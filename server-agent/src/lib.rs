pub mod config;
pub mod file_service;
pub mod file_watcher;
pub mod log_parser;
pub mod log_tail;
pub mod models;
pub mod path_policy;
pub mod runtime;
pub mod transport;

pub use config::{AgentConfig, FileServiceConfig, WorkspaceConfig, WorkspaceRootConfig};
pub use file_service::FileService;
pub use log_parser::LogParser;
pub use log_tail::LogTailer;
pub use models::{
    AgentClientMessage, AgentCommand, AgentCommandEnvelope, AgentCommandResult, AgentError,
    AgentHeartbeat, AgentPlatform, AgentRegistered, AgentRegistration, AgentServerMessage,
    FileTreeEntry, LogEnvelope, LogSourceConfig, LogStartPosition, ParseRule, ParseRuleKind,
    ParsedLogEvent, ReadFileResult, WorkspaceRootSummary, WriteFileResult,
};
pub use path_policy::PathPolicy;
pub use transport::{AgentCommandHandler, AgentConnection, Transport};
