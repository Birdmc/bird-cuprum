use bird_protocol::packet_default::StatusResponseObject;
use serde::{Serialize, Deserialize};
use crate::forwarding::IpForwarding;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CuprumConfig {
    pub servers: Vec<CuprumServerConfig>,
    #[serde(flatten)]
    pub ex: CuprumConfigEx,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CuprumConfigEx {
    pub buffer_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CuprumServerConfig {
    #[serde(default = "String::new")]
    pub host: String,
    pub listen: u16,
    pub server: String,
    pub offline_status: StatusResponseObject,
    pub offline_kick: String,
    #[serde(default = "IpForwarding::default")]
    pub ip_forwarding: IpForwarding,
}