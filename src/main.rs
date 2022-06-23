use log::LevelFilter;
use simple_logger::SimpleLogger;

use crate::config::CuprumConfig;
use crate::server::run_server;

mod config;
mod server;
mod route;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();
    log::info!("Loading configuration");
    let file = std::fs::read_to_string("config.json")?;
    let config: CuprumConfig = serde_json::from_str(file.as_str())?;
    log::info!("Configuration loaded");
    for handle in run_server(config) {
        handle.await?;
    }
    Ok(())
}