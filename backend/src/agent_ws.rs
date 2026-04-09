use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::{
    agent_registry::AgentRegistry,
    models::{AgentClientMessage, AgentServerMessage},
};

pub async fn serve(mut socket: WebSocket, registry: AgentRegistry, db: PgPool) {
    let registration = match read_registration(&mut socket).await {
        Ok(registration) => registration,
        Err(message) => {
            let _ = socket.send(Message::Text(message.into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    if let Err(message) = crate::verify_agent_registration_auth(&db, &registration).await {
        let _ = socket.send(Message::Text(message.into())).await;
        let _ = socket.close().await;
        return;
    }

    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
    let registered = registry.register(registration.clone(), outbound_tx).await;
    let session_id = registered.session_id.clone();
    let agent_id = registered.agent_id.clone();
    let ack = match serde_json::to_string(&AgentServerMessage::Registered(registered)) {
        Ok(payload) => payload,
        Err(_) => {
            let _ = socket.close().await;
            return;
        }
    };

    if socket.send(Message::Text(ack.into())).await.is_err() {
        registry.remove_session(&agent_id, &session_id).await;
        let _ = socket.close().await;
        return;
    }

    loop {
        tokio::select! {
            outbound = outbound_rx.recv() => {
                match outbound {
                    Some(payload) => {
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            inbound = socket.next() => {
                let Some(message) = inbound else {
                    break;
                };

                match message {
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(payload)) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {}
                    Ok(Message::Text(text)) => {
                        let message = match serde_json::from_str::<AgentClientMessage>(text.as_ref()) {
                            Ok(message) => message,
                            Err(_) => break,
                        };

                        match message {
                            AgentClientMessage::Heartbeat(_) => {
                                registry.record_heartbeat(&agent_id, &session_id).await;
                            }
                            AgentClientMessage::CommandResult(payload) => {
                                registry
                                    .resolve_command_result(&agent_id, &session_id, payload)
                                    .await;
                            }
                            AgentClientMessage::LogChunk(payload) => {
                                registry
                                    .broadcast_event(
                                        &agent_id,
                                        &session_id,
                                        crate::models::AgentStreamEvent::LogChunk(payload),
                                    )
                                    .await;
                            }
                            AgentClientMessage::FileChanged(payload) => {
                                registry
                                    .broadcast_event(
                                        &agent_id,
                                        &session_id,
                                        crate::models::AgentStreamEvent::FileChanged(payload),
                                    )
                                    .await;
                            }
                            AgentClientMessage::Register(_) => break,
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
    }

    registry.remove_session(&agent_id, &session_id).await;
}

async fn read_registration(
    socket: &mut WebSocket,
) -> Result<crate::models::AgentRegistration, String> {
    let frame = socket
        .next()
        .await
        .ok_or_else(|| "missing registration frame".to_string())?
        .map_err(|err| format!("failed to read registration frame: {err}"))?;

    let text = match frame {
        Message::Text(text) => text,
        _ => return Err("registration frame must be text".to_string()),
    };

    let message = serde_json::from_str::<AgentClientMessage>(text.as_ref())
        .map_err(|err| format!("invalid registration payload: {err}"))?;
    let registration = match message {
        AgentClientMessage::Register(payload) => payload,
        AgentClientMessage::Heartbeat(_)
        | AgentClientMessage::CommandResult(_)
        | AgentClientMessage::LogChunk(_)
        | AgentClientMessage::FileChanged(_) => {
            return Err("first frame must be agent.register".to_string());
        }
    };

    if registration.server_uuid.trim().is_empty() {
        return Err("server_uuid is required".to_string());
    }
    if registration.agent_id.trim().is_empty() {
        return Err("agent_id is required".to_string());
    }
    if registration.auth_key.trim().is_empty() {
        return Err("auth_key is required".to_string());
    }

    Ok(registration)
}
