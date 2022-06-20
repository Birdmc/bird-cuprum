use cubic_protocol::packet_default::StatusResponseObject;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CuprumConfig {
    pub servers: Vec<CuprumServerConfig>,
}

#[derive(Serialize, Deserialize)]
pub struct CuprumServerConfig {
    pub host: String,
    pub listen: u16,
    pub server: String,
    pub offline_status: StatusResponseObject,
}