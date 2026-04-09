use std::path::PathBuf;

use anyhow::Result;
use server_agent::{AgentConfig, runtime};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("agent.toml"));
    let config = AgentConfig::load_from_path(&config_path)?;

    runtime::run(config).await
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
