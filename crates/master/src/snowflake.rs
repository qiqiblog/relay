//! 标准 Snowflake ID 生成器（单 master，worker_id = 0）。
//!
//! 布局（高位到低位，共 64 位）：
//!   1  位 — 符号位，恒为 0
//!   41 位 — 自定义纪元毫秒时间戳（支持约 69 年）
//!   10 位 — worker ID（固定 0）
//!   12 位 — 毫秒内序列号（0–4095）

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 自定义纪元：2024-01-01 00:00:00 UTC
const EPOCH_MS: u64 = 1_704_067_200_000;
const WORKER_ID: u64 = 0;
const WORKER_SHIFT: u64 = 12;
const TS_SHIFT: u64 = 22;

static STATE: AtomicU64 = AtomicU64::new(0);

/// 生成一个单调递增的 Snowflake ID。
/// Snowflake 值的符号位恒为 0，可安全 as i64。
pub fn next_id() -> i64 {
    loop {
        let now_ms = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64)
            .saturating_sub(EPOCH_MS);

        let old = STATE.load(Ordering::Relaxed);
        let old_ts = old >> TS_SHIFT;
        let old_seq = old & 0xFFF;

        let (ts, seq) = if now_ms > old_ts {
            (now_ms, 0u64)
        } else {
            let next_seq = (old_seq + 1) & 0xFFF;
            if next_seq == 0 {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            (old_ts, next_seq)
        };

        let new_state = (ts << TS_SHIFT) | seq;
        if STATE
            .compare_exchange(old, new_state, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            return ((ts << TS_SHIFT) | (WORKER_ID << WORKER_SHIFT) | seq) as i64;
        }
    }
}

/// serde 辅助模块：将 i64 ID 序列化为 JSON 字符串，避免 JS 精度丢失。
pub mod as_str {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// serde 辅助模块：将 Option<i64> ID 序列化为 JSON 字符串或 null。
pub mod as_str_opt {
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(dead_code)]
    pub fn serialize<S: Serializer>(v: &Option<i64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(id) => s.serialize_some(&id.to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
        let opt = Option::<String>::deserialize(d)?;
        match opt {
            Some(s) if !s.is_empty() => {
                s.parse::<i64>().map(Some).map_err(serde::de::Error::custom)
            }
            _ => Ok(None),
        }
    }
}
