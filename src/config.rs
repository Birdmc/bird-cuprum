use cubic_protocol::packet_default::StatusResponseObject;
use serde::{Serialize, Deserialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CuprumConfig {
    pub servers: Vec<CuprumServerConfig>,
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
}