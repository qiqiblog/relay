use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelSpec {
    pub id: String,
    pub protocol: Protocol,
    pub listen_addr: String,
    pub upstream_addrs: Vec<String>,
    pub lb_strategy: String,
    pub max_connections: u32,
    pub allow_cidrs: Vec<String>,
    pub deny_cidrs: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub hostname: String,
    pub version: String,
    pub tags: Vec<String>,
}
