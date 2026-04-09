use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTreeEntry {
    pub logical_path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub logical_path: String,
    pub content: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteFileResult {
    pub logical_path: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentPlatform {
    Linux,
    Windows,
}

impl AgentPlatform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStartPosition {
    Beginning,
    End,
}

impl Default for LogStartPosition {
    fn default() -> Self {
        Self::End
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSourceConfig {
    pub primary_path: PathBuf,
    pub glob: Option<String>,
    #[serde(default)]
    pub start_position: LogStartPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseRuleKind {
    Regex,
}

impl Default for ParseRuleKind {
    fn default() -> Self {
        Self::Regex
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseRule {
    pub id: String,
    #[serde(default)]
    pub kind: ParseRuleKind,
    pub pattern: String,
    pub event_type: String,
    pub severity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEnvelope {
    pub agent_id: String,
    pub source: String,
    pub cursor: String,
    pub line_number: u64,
    pub raw_line: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedLogEvent {
    pub agent_id: String,
    pub rule_id: String,
    pub event_type: String,
    pub severity: String,
    pub source: String,
    pub cursor: String,
    pub line_number: u64,
    pub raw_line: String,
    pub observed_at: String,
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("invalid logical path: {0}")]
    InvalidLogicalPath(String),
    #[error("unknown root: {0}")]
    UnknownRoot(String),
    #[error("access denied: {0}")]
    AccessDenied(String),
    #[error("path encoding error: {0}")]
    PathEncoding(String),
    #[error("invalid parse rule `{rule_id}`: {message}")]
    InvalidParseRule { rule_id: String, message: String },
    #[error("file too large: path={path}, max_size={max_size}, actual_size={actual_size}")]
    FileTooLarge {
        path: String,
        max_size: u64,
        actual_size: u64,
    },
    #[error("extension not allowed: path={path}, extension={extension}")]
    ExtensionNotAllowed {
        path: String,
        extension: String,
    },
    #[error("file is not utf-8: {0}")]
    NotUtf8(String),
    #[error("version conflict: path={path}, expected={expected}, actual={actual}")]
    VersionConflict {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("unsupported parse rule kind: {0}")]
    UnsupportedParseRuleKind(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("notify error: {0}")]
    Notify(String),
    #[error("io error: {0}")]
    Io(std::io::Error),
}

impl From<std::io::Error> for AgentError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<notify::Error> for AgentError {
    fn from(value: notify::Error) -> Self {
        Self::Notify(value.to_string())
    }
}
