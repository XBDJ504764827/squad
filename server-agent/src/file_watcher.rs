use crate::models::AgentError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChangeEvent {
    pub logical_path: String,
}

#[derive(Debug, Default)]
pub struct FileWatcher;

impl FileWatcher {
    pub fn new() -> Result<Self, AgentError> {
        Ok(Self)
    }

    pub fn poll_changes(&mut self) -> Result<Vec<FileChangeEvent>, AgentError> {
        Ok(Vec::new())
    }
}
