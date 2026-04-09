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
    AgentError, AgentPlatform, FileTreeEntry, LogEnvelope, LogSourceConfig, LogStartPosition,
    ParseRule, ParseRuleKind, ParsedLogEvent, ReadFileResult, WriteFileResult,
};
pub use path_policy::PathPolicy;
