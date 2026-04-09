use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::models::{AgentError, LogSourceConfig, ParseRule};

const DEFAULT_MAX_FILE_SIZE: u64 = 1024 * 1024;
const DEFAULT_ENCODING: &str = "utf-8";

#[derive(Debug, Clone)]
pub struct WorkspaceRootConfig {
    pub name: String,
    pub local_root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceConfig {
    pub roots: Vec<WorkspaceRootConfig>,
    #[serde(default)]
    pub read_only_roots: Vec<String>,
    #[serde(default)]
    pub allowed_extensions: Option<Vec<String>>,
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileServiceConfig {
    #[serde(default = "default_max_file_size", alias = "max_file_size_bytes")]
    pub max_file_size: u64,
    #[serde(default)]
    pub allowed_extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub agent_id: String,
    pub backend_ws_url: String,
    pub backend_token: String,
    #[serde(default = "default_encoding")]
    pub default_encoding: String,
    pub log_source: LogSourceConfig,
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub parse_rules: Vec<ParseRule>,
}

impl WorkspaceConfig {
    pub fn file_service_config(&self) -> FileServiceConfig {
        FileServiceConfig {
            max_file_size: self.max_file_size_bytes,
            allowed_extensions: self.allowed_extensions.clone(),
        }
    }

    pub fn workspace_roots(&self) -> Vec<WorkspaceRootConfig> {
        self.roots.clone()
    }
}

impl AgentConfig {
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, AgentError> {
        let content = fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&content)
            .map_err(|err| AgentError::InvalidConfig(format!("failed to parse config: {err}")))?;
        config.normalize()?;
        Ok(config)
    }

    pub fn file_service_config(&self) -> FileServiceConfig {
        self.workspace.file_service_config()
    }

    pub fn workspace_roots(&self) -> Vec<WorkspaceRootConfig> {
        self.workspace.workspace_roots()
    }

    fn normalize(&mut self) -> Result<(), AgentError> {
        self.agent_id = self.agent_id.trim().to_string();
        self.backend_ws_url = self.backend_ws_url.trim().to_string();
        self.backend_token = self.backend_token.trim().to_string();
        self.default_encoding = self.default_encoding.trim().to_string();

        if self.agent_id.is_empty() {
            return Err(AgentError::InvalidConfig("agent_id is required".to_string()));
        }
        if self.backend_ws_url.is_empty() {
            return Err(AgentError::InvalidConfig(
                "backend_ws_url is required".to_string(),
            ));
        }
        if self.workspace.roots.is_empty() {
            return Err(AgentError::InvalidConfig(
                "workspace.roots is required".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'de> Deserialize<'de> for WorkspaceRootConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawWorkspaceRootConfig {
            name: String,
            #[serde(alias = "path")]
            local_root: PathBuf,
        }

        let raw = RawWorkspaceRootConfig::deserialize(deserializer)?;
        Ok(Self {
            name: raw.name,
            local_root: raw.local_root,
        })
    }
}

fn default_max_file_size() -> u64 {
    DEFAULT_MAX_FILE_SIZE
}

fn default_encoding() -> String {
    DEFAULT_ENCODING.to_string()
}
