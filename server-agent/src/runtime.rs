use anyhow::Result;
use tracing::info;

use crate::{AgentConfig, FileService, LogParser, PathPolicy};

pub async fn run(config: AgentConfig) -> Result<()> {
    let path_policy = PathPolicy::new(&config.workspace_roots())?;
    let _file_service = FileService::new(path_policy, config.file_service_config());
    let _parser = LogParser::new(config.parse_rules.clone())?;

    info!(
        agent_id = %config.agent_id,
        backend_ws_url = %config.backend_ws_url,
        roots = config.workspace.roots.len(),
        "server-agent runtime initialized"
    );

    Ok(())
}
