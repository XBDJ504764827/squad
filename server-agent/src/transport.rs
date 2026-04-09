use crate::models::AgentError;

#[derive(Debug, Clone)]
pub struct Transport {
    backend_ws_url: String,
}

impl Transport {
    pub fn new(backend_ws_url: impl Into<String>) -> Self {
        Self {
            backend_ws_url: backend_ws_url.into(),
        }
    }

    pub async fn connect(&self) -> Result<(), AgentError> {
        if self.backend_ws_url.trim().is_empty() {
            return Err(AgentError::Transport(
                "backend websocket url is empty".to_string(),
            ));
        }
        Ok(())
    }
}
