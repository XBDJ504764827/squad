use anyhow::Result;
use serde_json::Value;
use tracing::info;

use crate::{
    AgentCommand, AgentCommandHandler, AgentConfig, AgentError, AgentRegistration, FileReadResult,
    FileService, FileTreeResult, FileWriteResult, LogParser, PathPolicy, Transport,
    WorkspaceRootSummary,
};

pub async fn run(config: AgentConfig) -> Result<()> {
    let path_policy = PathPolicy::new(&config.workspace_roots())?;
    let file_service = FileService::new(path_policy, config.file_service_config());
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
    let handler = RuntimeCommandHandler::new(file_service);
    transport.run(registration, &handler).await?;

    Ok(())
}

pub struct RuntimeCommandHandler {
    file_service: FileService,
}

impl RuntimeCommandHandler {
    pub fn new(file_service: FileService) -> Self {
        Self { file_service }
    }
}

impl AgentCommandHandler for RuntimeCommandHandler {
    fn handle_command(&self, command: AgentCommand) -> Result<Option<Value>, AgentError> {
        match command {
            AgentCommand::Ping => Ok(crate::transport::default_ping_response()),
            AgentCommand::FileTree(request) => {
                let entries = self.file_service.list_tree(&request.logical_path)?;
                serde_json::to_value(FileTreeResult { entries })
                    .map(Some)
                    .map_err(|err| {
                        AgentError::Runtime(format!("failed to serialize file tree result: {err}"))
                    })
            }
            AgentCommand::FileRead(request) => {
                let result = self.file_service.read_text_file(&request.logical_path)?;
                serde_json::to_value(FileReadResult {
                    logical_path: result.logical_path,
                    content: result.content,
                    version: result.version,
                })
                .map(Some)
                .map_err(|err| {
                    AgentError::Runtime(format!("failed to serialize file read result: {err}"))
                })
            }
            AgentCommand::FileWrite(request) => {
                let result = self.file_service.write_text_file(
                    &request.logical_path,
                    &request.content,
                    request.expected_version.as_deref(),
                )?;
                serde_json::to_value(FileWriteResult {
                    logical_path: result.logical_path,
                    version: result.version,
                })
                .map(Some)
                .map_err(|err| {
                    AgentError::Runtime(format!("failed to serialize file write result: {err}"))
                })
            }
        }
    }
}
