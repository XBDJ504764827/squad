use anyhow::Result;
use serde_json::Value;
use std::sync::Mutex;
use tracing::info;

use crate::file_watcher::FileWatcher;
use crate::{
    AgentCommand, AgentCommandHandler, AgentConfig, AgentError, AgentFileChanged,
    AgentRegistration, FileReadResult, FileService, FileTreeResult, FileWriteResult, LogEnvelope,
    LogParser, LogTailer, ParseRule, ParsedLogEvent, PathPolicy, ReplaceParseRulesResult,
    Transport, WorkspaceRootSummary,
};

pub async fn run(config: AgentConfig) -> Result<()> {
    let workspace_roots = config.workspace_roots();
    let path_policy = PathPolicy::new(&workspace_roots)?;
    let file_service = FileService::new(path_policy.clone(), config.file_service_config());
    let log_tailer = LogTailer::new(config.agent_id.clone(), "server", config.log_source.clone())?;
    let file_watcher = FileWatcher::new(path_policy, &workspace_roots)?;

    info!(
        agent_id = %config.agent_id,
        backend_ws_url = %config.backend_ws_url,
        roots = config.workspace.roots.len(),
        "server-agent runtime initialized"
    );

    let transport = Transport::new(config.backend_ws_url.clone());
    let registration = AgentRegistration {
        server_uuid: config.server_uuid.clone(),
        agent_id: config.agent_id.clone(),
        auth_key: config.auth_key.clone(),
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
    let handler = RuntimeCommandHandler::with_streaming(
        file_service,
        config.parse_rules.clone(),
        log_tailer,
        file_watcher,
    )?;
    transport.run(registration, &handler).await?;

    Ok(())
}

pub struct RuntimeCommandHandler {
    file_service: FileService,
    log_parser: Mutex<LogParser>,
    parsed_events: Mutex<Vec<ParsedLogEvent>>,
    log_tailer: Option<Mutex<LogTailer>>,
    file_watcher: Option<Mutex<FileWatcher>>,
}

impl RuntimeCommandHandler {
    pub fn new(file_service: FileService) -> Self {
        Self {
            file_service,
            log_parser: Mutex::new(LogParser::new(Vec::new()).expect("empty parse rules should compile")),
            parsed_events: Mutex::new(Vec::new()),
            log_tailer: None,
            file_watcher: None,
        }
    }

    pub fn with_parser(file_service: FileService, parse_rules: Vec<ParseRule>) -> Result<Self, AgentError> {
        Ok(Self {
            file_service,
            log_parser: Mutex::new(LogParser::new(parse_rules)?),
            parsed_events: Mutex::new(Vec::new()),
            log_tailer: None,
            file_watcher: None,
        })
    }

    pub fn with_streaming(
        file_service: FileService,
        parse_rules: Vec<ParseRule>,
        log_tailer: LogTailer,
        file_watcher: FileWatcher,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            file_service,
            log_parser: Mutex::new(LogParser::new(parse_rules)?),
            parsed_events: Mutex::new(Vec::new()),
            log_tailer: Some(Mutex::new(log_tailer)),
            file_watcher: Some(Mutex::new(file_watcher)),
        })
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
            AgentCommand::ReplaceParseRules(request) => {
                let mut parser = self
                    .log_parser
                    .lock()
                    .map_err(|_| AgentError::Runtime("failed to lock log parser".to_string()))?;
                parser.replace_rules(request.rules.clone())?;
                serde_json::to_value(ReplaceParseRulesResult {
                    version: request.version,
                    rule_count: request.rules.len(),
                })
                .map(Some)
                .map_err(|err| {
                    AgentError::Runtime(format!("failed to serialize parse rule result: {err}"))
                })
            }
        }
    }

    fn drain_log_entries(&self) -> Result<Vec<LogEnvelope>, AgentError> {
        let Some(log_tailer) = &self.log_tailer else {
            return Ok(Vec::new());
        };

        let entries = log_tailer
            .lock()
            .map_err(|_| AgentError::Runtime("failed to lock log tailer".to_string()))?
            .poll()?;

        if entries.is_empty() {
            return Ok(entries);
        }

        let parser = self
            .log_parser
            .lock()
            .map_err(|_| AgentError::Runtime("failed to lock log parser".to_string()))?;
        let parsed = entries
            .iter()
            .filter_map(|entry| parser.parse(entry))
            .collect::<Vec<_>>();
        drop(parser);

        if !parsed.is_empty() {
            self.parsed_events
                .lock()
                .map_err(|_| AgentError::Runtime("failed to lock parsed events".to_string()))?
                .extend(parsed);
        }

        Ok(entries)
    }

    fn drain_file_changes(&self) -> Result<Vec<AgentFileChanged>, AgentError> {
        let Some(file_watcher) = &self.file_watcher else {
            return Ok(Vec::new());
        };

        file_watcher
            .lock()
            .map_err(|_| AgentError::Runtime("failed to lock file watcher".to_string()))?
            .poll_changes()
    }

    fn drain_parsed_events(&self) -> Result<Vec<ParsedLogEvent>, AgentError> {
        let mut guard = self
            .parsed_events
            .lock()
            .map_err(|_| AgentError::Runtime("failed to lock parsed events".to_string()))?;
        Ok(std::mem::take(&mut *guard))
    }
}
