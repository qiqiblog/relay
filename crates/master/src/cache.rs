//! 轻量级 Redis 缓存工具。
//!
//! 设计原则：缓存层永远不应让请求变慢。所有 Redis 操作都包了 100~200ms
//! 超时；任何错误（超时、连接断、反序列化失败、Redis 未配置）一律 fail-open
//! 跳过缓存继续走业务逻辑。

use std::time::Duration;

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::de::DeserializeOwned;
use serde::Serialize;

const GET_TIMEOUT: Duration = Duration::from_millis(150);
const SET_TIMEOUT: Duration = Duration::from_millis(150);

/// 取 JSON 缓存值；任何错误（含未配置 Redis）返回 None。
pub async fn get_json<T: DeserializeOwned>(
    redis: &Option<ConnectionManager>,
    key: &str,
) -> Option<T> {
    let mut conn = redis.as_ref()?.clone();
    let raw: Option<String> =
        match tokio::time::timeout(GET_TIMEOUT, conn.get::<_, Option<String>>(key)).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                tracing::debug!(key, error = %e, "redis cache get failed");
                return None;
            }
            Err(_) => {
                tracing::debug!(key, "redis cache get timed out");
                return None;
            }
        };
    let raw = raw?;
    match serde_json::from_str::<T>(&raw) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::debug!(key, error = %e, "redis cache decode failed");
            None
        }
    }
}

/// best-effort 写入；错误吞掉只 log。
pub async fn set_json<T: Serialize>(
    redis: &Option<ConnectionManager>,
    key: &str,
    value: &T,
    ttl_secs: u64,
) {
    let Some(conn) = redis.as_ref() else { return };
    let raw = match serde_json::to_string(value) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(key, error = %e, "redis cache encode failed");
            return;
        }
    };
    let mut conn = conn.clone();
    let res: Result<Result<(), redis::RedisError>, tokio::time::error::Elapsed> =
        tokio::time::timeout(SET_TIMEOUT, conn.set_ex::<_, _, ()>(key, raw, ttl_secs)).await;
    match res {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::debug!(key, error = %e, "redis cache set failed"),
        Err(_) => tracing::debug!(key, "redis cache set timed out"),
    }
}

/// 节点心跳缓存：作为 master 重启时的 L1 warmup 来源。
///
/// 热路径只读 in-process L1（见 `AppState.node_runtime`）。Redis 承担两件事：
/// 1) 写心跳时同步 mirror（best-effort）；2) master 启动时一次性 warmup。
pub mod node {
    use std::time::Duration;

    use chrono::{DateTime, Utc};
    use redis::aio::ConnectionManager;
    use redis::AsyncCommands;
    use serde::{Deserialize, Serialize};

    /// L2 TTL：远大于 15s 在线阈值，给单 master 重启 / 短暂断连留余量。
    pub const RUNTIME_TTL_SECS: u64 = 90;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct RuntimePayload {
        pub last_heartbeat: serde_json::Value,
        pub last_seen_at: DateTime<Utc>,
        pub version: String,
        pub protocol_version: i32,
        #[serde(default)]
        pub capabilities: Vec<String>,
    }

    fn key(id: &str) -> String {
        format!("node:hb:{id}")
    }

    pub async fn write(redis: &Option<ConnectionManager>, id: &str, p: &RuntimePayload) {
        super::set_json(redis, &key(id), p, RUNTIME_TTL_SECS).await;
    }

    pub async fn delete(redis: &Option<ConnectionManager>, id: &str) {
        let Some(conn) = redis.as_ref() else { return };
        let mut conn = conn.clone();
        let k = key(id);
        let res = tokio::time::timeout(Duration::from_millis(150), conn.del::<_, ()>(&k)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::debug!(key = %k, error = %e, "redis node hb delete failed"),
            Err(_) => tracing::debug!(key = %k, "redis node hb delete timed out"),
        }
    }

    /// master 启动时一次性灌满 in-process L1。失败只 warn，不阻挡启动。
    pub async fn warmup(redis: &Option<ConnectionManager>) -> Vec<(String, RuntimePayload)> {
        let Some(conn) = redis.as_ref() else {
            return vec![];
        };
        let mut conn = conn.clone();

        // 1) SCAN 收集 keys（带总预算 2s + 上限 5000 防止无界扫描）
        let mut keys: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;
        let scan_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            let mut cmd = redis::cmd("SCAN");
            cmd.arg(cursor)
                .arg("MATCH")
                .arg("node:hb:*")
                .arg("COUNT")
                .arg(200);
            let fut = cmd.query_async::<(u64, Vec<String>)>(&mut conn);
            let remaining = scan_deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                tracing::warn!("redis node hb warmup: scan deadline exceeded");
                break;
            }
            match tokio::time::timeout(remaining, fut).await {
                Ok(Ok((next, batch))) => {
                    keys.extend(batch);
                    if next == 0 {
                        break;
                    }
                    cursor = next;
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "redis node hb warmup: SCAN failed");
                    return vec![];
                }
                Err(_) => {
                    tracing::warn!("redis node hb warmup: SCAN timed out");
                    return vec![];
                }
            }
            if keys.len() > 5_000 {
                tracing::warn!(
                    count = keys.len(),
                    "redis node hb warmup: too many keys, truncating"
                );
                break;
            }
        }

        // 2) 逐个 GET（小规模够用；大规模可后续改 MGET）
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            let fut = conn.get::<_, Option<String>>(&k);
            match tokio::time::timeout(Duration::from_millis(150), fut).await {
                Ok(Ok(Some(s))) => {
                    if let Ok(p) = serde_json::from_str::<RuntimePayload>(&s) {
                        if let Some(id) = k.strip_prefix("node:hb:") {
                            out.push((id.to_string(), p));
                        }
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => {
                    tracing::debug!(key = %k, error = %e, "redis node hb warmup get failed")
                }
                Err(_) => tracing::debug!(key = %k, "redis node hb warmup get timed out"),
            }
        }
        out
    }
}
