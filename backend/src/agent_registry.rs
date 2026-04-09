use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    sync::{RwLock, broadcast, mpsc, oneshot},
    time::{Duration, timeout},
};
use uuid::Uuid;

use crate::models::{
    AgentCommand, AgentCommandEnvelope, AgentCommandResult, AgentRegistered, AgentRegistration,
    AgentServerMessage, AgentStreamEvent, OnlineAgent,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone, Default)]
pub struct AgentRegistry {
    sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    pending_commands: Arc<RwLock<HashMap<String, PendingCommand>>>,
    event_channels: Arc<RwLock<HashMap<String, broadcast::Sender<AgentStreamEvent>>>>,
}

#[derive(Clone)]
struct AgentSession {
    online_agent: OnlineAgent,
    outbound_tx: mpsc::UnboundedSender<String>,
}

struct PendingCommand {
    agent_id: String,
    session_id: String,
    response_tx: oneshot::Sender<AgentCommandResult>,
}

impl AgentRegistry {
    pub async fn register(
        &self,
        registration: AgentRegistration,
        outbound_tx: mpsc::UnboundedSender<String>,
    ) -> AgentRegistered {
        let session_id = Uuid::new_v4().to_string();
        let now = now_millis();
        let online_agent = OnlineAgent {
            session_id: session_id.clone(),
            connected_at_ms: now,
            last_heartbeat_at_ms: now,
            registration: registration.clone(),
        };

        let previous = self.sessions.write().await.insert(
            registration.agent_id.clone(),
            AgentSession {
                online_agent,
                outbound_tx,
            },
        );

        if let Some(previous) = previous {
            self.fail_pending_for_session(
                &registration.agent_id,
                &previous.online_agent.session_id,
            )
            .await;
        }

        AgentRegistered {
            agent_id: registration.agent_id,
            session_id,
        }
    }

    pub async fn get(&self, agent_id: &str) -> Option<OnlineAgent> {
        self.sessions
            .read()
            .await
            .get(agent_id)
            .map(|session| session.online_agent.clone())
    }

    pub async fn list(&self) -> Vec<OnlineAgent> {
        self.sessions
            .read()
            .await
            .values()
            .map(|session| session.online_agent.clone())
            .collect()
    }

    pub async fn record_heartbeat(&self, agent_id: &str, session_id: &str) {
        let mut guard = self.sessions.write().await;
        if let Some(session) = guard.get_mut(agent_id) {
            if session.online_agent.session_id == session_id {
                session.online_agent.last_heartbeat_at_ms =
                    now_millis().max(session.online_agent.last_heartbeat_at_ms.saturating_add(1));
            }
        }
    }

    pub async fn dispatch_command(
        &self,
        agent_id: &str,
        command: AgentCommand,
    ) -> Result<AgentCommandResult, String> {
        let (session_id, outbound_tx) = {
            let guard = self.sessions.read().await;
            let session = guard
                .get(agent_id)
                .ok_or_else(|| format!("agent `{agent_id}` is offline"))?;
            (
                session.online_agent.session_id.clone(),
                session.outbound_tx.clone(),
            )
        };

        let request_id = Uuid::new_v4().to_string();
        let payload = serde_json::to_string(&AgentServerMessage::Command(AgentCommandEnvelope {
            request_id: request_id.clone(),
            command,
        }))
        .map_err(|err| format!("failed to serialize command: {err}"))?;

        let (response_tx, response_rx) = oneshot::channel();
        self.pending_commands.write().await.insert(
            request_id.clone(),
            PendingCommand {
                agent_id: agent_id.to_string(),
                session_id: session_id.clone(),
                response_tx,
            },
        );

        if outbound_tx.send(payload).is_err() {
            self.pending_commands.write().await.remove(&request_id);
            return Err(format!("agent `{agent_id}` connection is not writable"));
        }

        match timeout(COMMAND_TIMEOUT, response_rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(format!("agent `{agent_id}` disconnected before response")),
            Err(_) => {
                self.pending_commands.write().await.remove(&request_id);
                Err(format!("agent `{agent_id}` response timed out"))
            }
        }
    }

    pub async fn resolve_command_result(
        &self,
        agent_id: &str,
        session_id: &str,
        result: AgentCommandResult,
    ) {
        let pending = self
            .pending_commands
            .write()
            .await
            .remove(&result.request_id);

        if let Some(pending) = pending {
            if pending.agent_id == agent_id && pending.session_id == session_id {
                let _ = pending.response_tx.send(result);
            }
        }
    }

    pub async fn subscribe_events(&self, agent_id: &str) -> broadcast::Receiver<AgentStreamEvent> {
        self.get_or_create_event_sender(agent_id).await.subscribe()
    }

    pub async fn broadcast_event(&self, agent_id: &str, session_id: &str, event: AgentStreamEvent) {
        let is_active_session = self
            .sessions
            .read()
            .await
            .get(agent_id)
            .map(|session| session.online_agent.session_id == session_id)
            .unwrap_or(false);

        if !is_active_session {
            return;
        }

        let sender = self.get_or_create_event_sender(agent_id).await;
        let _ = sender.send(event);
    }

    pub async fn remove_session(&self, agent_id: &str, session_id: &str) {
        let mut guard = self.sessions.write().await;
        let should_remove = guard
            .get(agent_id)
            .map(|session| session.online_agent.session_id == session_id)
            .unwrap_or(false);

        if should_remove {
            guard.remove(agent_id);
        }

        drop(guard);

        if should_remove {
            self.fail_pending_for_session(agent_id, session_id).await;
        }
    }

    async fn fail_pending_for_session(&self, agent_id: &str, session_id: &str) {
        let request_ids = {
            let guard = self.pending_commands.read().await;
            guard
                .iter()
                .filter(|(_, pending)| {
                    pending.agent_id == agent_id && pending.session_id == session_id
                })
                .map(|(request_id, _)| request_id.clone())
                .collect::<Vec<_>>()
        };

        let mut guard = self.pending_commands.write().await;
        for request_id in request_ids {
            guard.remove(&request_id);
        }
    }

    async fn get_or_create_event_sender(
        &self,
        agent_id: &str,
    ) -> broadcast::Sender<AgentStreamEvent> {
        if let Some(sender) = self.event_channels.read().await.get(agent_id).cloned() {
            return sender;
        }

        let mut guard = self.event_channels.write().await;
        guard
            .entry(agent_id.to_string())
            .or_insert_with(|| {
                let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
                sender
            })
            .clone()
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
