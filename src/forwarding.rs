use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::CuprumServerConfig;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum IpForwarding {
    None,
    IpAddr,
    TcpShieldRealIp,
}

impl Default for IpForwarding {
    fn default() -> Self {
        IpForwarding::None
    }
}

impl IpForwarding {
    pub fn handshake_string(&self, current: String, config: Arc<CuprumServerConfig>, addr: SocketAddr) -> String {
        match self {
            IpForwarding::None => current,
            IpForwarding::IpAddr => match config.server.rfind(':') {
                Some(index) => config.server[0..index].to_string(),
                None => config.server.clone()
            },
            IpForwarding::TcpShieldRealIp =>
                format!(
                    "{}///{}///{}",
                    current, addr,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                )
        }
    }
}