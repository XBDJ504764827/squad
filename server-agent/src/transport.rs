use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    net::TcpStream,
    time::{interval, sleep, MissedTickBehavior},
};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::warn;

use crate::models::{
    AgentClientMessage, AgentCommand, AgentCommandResult, AgentError, AgentFileChanged,
    AgentHeartbeat, AgentLogChunk, AgentRegistered, AgentRegistration, AgentServerMessage,
    LogEnvelope,
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const RECONNECT_DELAY: Duration = Duration::from_secs(1);

pub trait AgentCommandHandler {
    fn handle_command(&self, command: AgentCommand) -> Result<Option<Value>, AgentError>;

    fn drain_log_entries(&self) -> Result<Vec<LogEnvelope>, AgentError> {
        Ok(Vec::new())
    }

    fn drain_file_changes(&self) -> Result<Vec<AgentFileChanged>, AgentError> {
        Ok(Vec::new())
    }
}

pub struct AgentConnection {
    registered: AgentRegistered,
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

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
    ) -> Result<AgentConnection, AgentError> {
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
        send_client_message(&mut socket, &register).await?;

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
            AgentServerMessage::Registered(payload) => Ok(AgentConnection {
                registered: payload,
                socket,
            }),
            AgentServerMessage::Command(_) => Err(AgentError::Transport(
                "backend sent command before registration completed".to_string(),
            )),
        }
    }

    pub async fn run<H>(
        &self,
        registration: AgentRegistration,
        handler: &H,
    ) -> Result<(), AgentError>
    where
        H: AgentCommandHandler,
    {
        if self.backend_ws_url.trim().is_empty() {
            return Err(AgentError::Transport(
                "backend websocket url is empty".to_string(),
            ));
        }

        loop {
            match self.connect(registration.clone()).await {
                Ok(mut connection) => {
                    if let Err(err) = self.run_session(&mut connection, handler).await {
                        warn!(error = %err, "transport session ended with error");
                    }
                }
                Err(err) => {
                    warn!(error = %err, "transport connection attempt failed");
                }
            }

            sleep(RECONNECT_DELAY).await;
        }
    }

    async fn run_session<H>(
        &self,
        connection: &mut AgentConnection,
        handler: &H,
    ) -> Result<(), AgentError>
    where
        H: AgentCommandHandler,
    {
        let mut ticker = interval(HEARTBEAT_INTERVAL);
        let mut event_ticker = interval(EVENT_POLL_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        event_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        event_ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    connection.send_heartbeat().await?;
                }
                _ = event_ticker.tick() => {
                    let log_entries = handler.drain_log_entries()?;
                    if !log_entries.is_empty() {
                        connection.send_log_chunk(AgentLogChunk { entries: log_entries }).await?;
                    }

                    for change in handler.drain_file_changes()? {
                        connection.send_file_changed(change).await?;
                    }
                }
                message = connection.next_server_message() => {
                    match message? {
                        Some(AgentServerMessage::Command(command)) => {
                            let result = match handler.handle_command(command.command) {
                                Ok(payload) => AgentCommandResult {
                                    request_id: command.request_id,
                                    success: true,
                                    payload,
                                    error: None,
                                },
                                Err(err) => AgentCommandResult {
                                    request_id: command.request_id,
                                    success: false,
                                    payload: None,
                                    error: Some(err.to_string()),
                                },
                            };

                            connection.send_command_result(result).await?;
                        }
                        Some(AgentServerMessage::Registered(_)) => {
                            return Err(AgentError::Transport(
                                "unexpected registered message during active session".to_string(),
                            ));
                        }
                        None => return Ok(()),
                    }
                }
            }
        }
    }
}

impl AgentConnection {
    pub fn registered(&self) -> &AgentRegistered {
        &self.registered
    }

    pub async fn send_heartbeat(&mut self) -> Result<(), AgentError> {
        self.send_message(&AgentClientMessage::Heartbeat(AgentHeartbeat {}))
            .await
    }

    pub async fn send_command_result(
        &mut self,
        result: AgentCommandResult,
    ) -> Result<(), AgentError> {
        self.send_message(&AgentClientMessage::CommandResult(result))
            .await
    }

    pub async fn send_log_chunk(&mut self, payload: AgentLogChunk) -> Result<(), AgentError> {
        self.send_message(&AgentClientMessage::LogChunk(payload))
            .await
    }

    pub async fn send_file_changed(&mut self, payload: AgentFileChanged) -> Result<(), AgentError> {
        self.send_message(&AgentClientMessage::FileChanged(payload))
            .await
    }

    pub async fn next_server_message(&mut self) -> Result<Option<AgentServerMessage>, AgentError> {
        loop {
            let frame = match self.socket.next().await {
                Some(frame) => frame.map_err(|err| {
                    AgentError::Transport(format!("failed to read backend message: {err}"))
                })?,
                None => return Ok(None),
            };

            match frame {
                Message::Text(text) => {
                    let message = serde_json::from_str::<AgentServerMessage>(text.as_ref())
                        .map_err(|err| {
                            AgentError::Transport(format!("failed to parse backend message: {err}"))
                        })?;
                    return Ok(Some(message));
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|err| {
                            AgentError::Transport(format!("failed to send websocket pong: {err}"))
                        })?;
                }
                Message::Pong(_) => {}
                Message::Close(_) => return Ok(None),
                other => {
                    return Err(AgentError::Transport(format!(
                        "unexpected backend frame during session: {other:?}"
                    )));
                }
            }
        }
    }

    async fn send_message(&mut self, message: &AgentClientMessage) -> Result<(), AgentError> {
        send_client_message(&mut self.socket, message).await
    }
}

async fn send_client_message(
    socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    message: &AgentClientMessage,
) -> Result<(), AgentError> {
    let payload = serde_json::to_string(message)
        .map_err(|err| AgentError::Transport(format!("failed to serialize message: {err}")))?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|err| AgentError::Transport(format!("failed to send message: {err}")))
}

pub(crate) fn default_ping_response() -> Option<Value> {
    Some(json!({ "pong": true }))
}
