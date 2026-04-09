use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use crate::models::{
    AgentClientMessage, AgentError, AgentRegistered, AgentRegistration, AgentServerMessage,
};

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

    pub async fn connect(
        &self,
        registration: AgentRegistration,
    ) -> Result<AgentRegistered, AgentError> {
        if self.backend_ws_url.trim().is_empty() {
            return Err(AgentError::Transport(
                "backend websocket url is empty".to_string(),
            ));
        }

        let (mut socket, _) = tokio_tungstenite::connect_async(&self.backend_ws_url)
            .await
            .map_err(|err| {
                AgentError::Transport(format!("failed to connect backend websocket: {err}"))
            })?;

        let register = AgentClientMessage::Register(registration);
        socket
            .send(Message::Text(
                serde_json::to_string(&register)
                    .map_err(|err| {
                        AgentError::Transport(format!(
                            "failed to serialize register message: {err}"
                        ))
                    })?
                    .into(),
            ))
            .await
            .map_err(|err| {
                AgentError::Transport(format!("failed to send register message: {err}"))
            })?;

        let frame = socket
            .next()
            .await
            .ok_or_else(|| {
                AgentError::Transport(
                    "backend websocket closed before registration ack".to_string(),
                )
            })?
            .map_err(|err| {
                AgentError::Transport(format!("failed to read backend message: {err}"))
            })?;

        let message = match frame {
            Message::Text(text) => serde_json::from_str::<AgentServerMessage>(text.as_ref())
                .map_err(|err| {
                    AgentError::Transport(format!("failed to parse backend message: {err}"))
                })?,
            other => {
                return Err(AgentError::Transport(format!(
                    "unexpected backend frame during registration: {other:?}"
                )));
            }
        };

        match message {
            AgentServerMessage::Registered(payload) => Ok(payload),
        }
    }
}
