use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub http_addr: String,
    pub grpc_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub pki_dir: PathBuf,
    pub public_addrs: Vec<String>,
    /// 可选 Redis URL（如 `redis://:pw@127.0.0.1:6379/0`），仅用于轻量级缓存（probe 防抖等）。
    /// 留空时 master 完全不连 Redis，对应功能退化为无缓存。
    pub redis_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let public_addrs = std::env::var("MASTER_PUBLIC_ADDR")
            .map_err(|_| {
                anyhow::anyhow!(
                    "MASTER_PUBLIC_ADDR is required (comma-separated DNS names or IPs the \
                     master is reachable at; used as TLS SAN). Refusing to start."
                )
            })?
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if public_addrs.is_empty() {
            anyhow::bail!("MASTER_PUBLIC_ADDR contained no usable entries");
        }
        Ok(Self {
            http_addr: std::env::var("MASTER_HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:7080".into()),
            grpc_addr: std::env::var("MASTER_GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:7443".into()),
            database_url: std::env::var("MASTER_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .map_err(|_| anyhow::anyhow!("MASTER_DATABASE_URL is required"))?,
            jwt_secret: std::env::var("MASTER_JWT_SECRET")
                .unwrap_or_else(|_| "dev-only-not-for-prod-change-me".into()),
            pki_dir: std::env::var("MASTER_PKI_DIR")
                .unwrap_or_else(|_| "/var/lib/relay-master/pki".into())
                .into(),
            public_addrs,
            redis_url: std::env::var("MASTER_REDIS_URL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        })
    }
}
