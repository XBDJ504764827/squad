use anyhow::Result;
use tracing::info;

use crate::{
    AgentConfig, AgentRegistration, FileService, LogParser, PathPolicy, Transport,
    WorkspaceRootSummary,
};

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

    let transport = Transport::new(config.backend_ws_url.clone());
    let registration = AgentRegistration {
        agent_id: config.agent_id.clone(),
        token: config.backend_token.clone(),
        platform: crate::AgentPlatform::current(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        workspace_roots: config
            .workspace_roots()
            .into_iter()
            .map(|root| {
                let name = root.name;
                WorkspaceRootSummary {
                    logical_path: format!("/{name}"),
                    name,
                }
            })
            .collect(),
        primary_log_path: config.log_source.primary_path.to_string_lossy().to_string(),
    };
    let registered = transport.connect(registration).await?;

    info!(
        agent_id = %registered.agent_id,
        session_id = %registered.session_id,
        "server-agent transport registered"
    );

    Ok(())
}
