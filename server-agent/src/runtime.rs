use anyhow::Result;
use serde_json::Value;
use tracing::info;

use crate::{
    AgentCommand, AgentCommandHandler, AgentConfig, AgentError, AgentRegistration, FileService,
    LogParser, PathPolicy, Transport, WorkspaceRootSummary,
};

pub async fn run(config: AgentConfig) -> Result<()> {
    let path_policy = PathPolicy::new(&config.workspace_roots())?;
    let file_service = FileService::new(path_policy, config.file_service_config());
    let parser = LogParser::new(config.parse_rules.clone())?;

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
    let handler = RuntimeCommandHandler {
        _file_service: file_service,
        _parser: parser,
    };
    transport.run(registration, &handler).await?;

    Ok(())
}

struct RuntimeCommandHandler {
    _file_service: FileService,
    _parser: LogParser,
}

impl AgentCommandHandler for RuntimeCommandHandler {
    fn handle_command(&self, command: AgentCommand) -> Result<Option<Value>, AgentError> {
        match command {
            AgentCommand::Ping => Ok(crate::transport::default_ping_response()),
        }
    }
}
