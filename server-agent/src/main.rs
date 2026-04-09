use std::path::PathBuf;

use anyhow::Result;
use server_agent::{runtime, AgentConfig};

#[tokio::main]
async fn main() -> Result<()> {
    init_rustls_crypto_provider();
    init_tracing();

    let config_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("agent.toml"));
    let config = AgentConfig::load_from_path(&config_path)?;

    runtime::run(config).await
}

fn init_rustls_crypto_provider() {
    // WSS 连接依赖 rustls 的进程级默认 CryptoProvider。
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_rustls_crypto_provider_installs_default_provider() {
        init_rustls_crypto_provider();

        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}
