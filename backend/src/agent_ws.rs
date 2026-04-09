use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};

use crate::{
    agent_registry::AgentRegistry,
    models::{AgentClientMessage, AgentServerMessage},
};

pub async fn serve(mut socket: WebSocket, registry: AgentRegistry) {
    let registration = match read_registration(&mut socket).await {
        Ok(registration) => registration,
        Err(message) => {
            let _ = socket.send(Message::Text(message.into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    let registered = registry.register(registration.clone()).await;
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

    while let Some(message) = socket.next().await {
        match message {
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(payload)) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => break,
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
    };

    if registration.agent_id.trim().is_empty() {
        return Err("agent_id is required".to_string());
    }
    if registration.token.trim().is_empty() {
        return Err("token is required".to_string());
    }

    Ok(registration)
}
