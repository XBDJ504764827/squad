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
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);

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
        self.reap_stale_sessions().await;
        self.sessions
            .read()
            .await
            .get(agent_id)
            .map(|session| session.online_agent.clone())
    }

    pub async fn list(&self) -> Vec<OnlineAgent> {
        self.reap_stale_sessions().await;
        self.sessions
            .read()
            .await
            .values()
            .map(|session| session.online_agent.clone())
            .collect()
    }

    pub async fn get_by_server_uuid(&self, server_uuid: &str) -> Option<OnlineAgent> {
        self.reap_stale_sessions().await;
        self.sessions
            .read()
            .await
            .values()
            .map(|session| session.online_agent.clone())
            .filter(|agent| agent.registration.server_uuid == server_uuid)
            .max_by_key(|agent| agent.connected_at_ms)
    }

    pub async fn reap_stale_sessions(&self) -> usize {
        let stale_sessions = {
            let guard = self.sessions.read().await;
            let now = now_millis();
            let timeout_ms = HEARTBEAT_TIMEOUT.as_millis() as u64;

            guard
                .iter()
                .filter_map(|(agent_id, session)| {
                    let last_seen = session.online_agent.last_heartbeat_at_ms;
                    let elapsed_ms = now.saturating_sub(last_seen);
                    (elapsed_ms > timeout_ms)
                        .then(|| (agent_id.clone(), session.online_agent.session_id.clone()))
                })
                .collect::<Vec<_>>()
        };

        for (agent_id, session_id) in &stale_sessions {
            self.remove_session(agent_id, session_id).await;
        }

        stale_sessions.len()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentPlatform, WorkspaceRootSummary};

    #[tokio::test]
    async fn reap_stale_sessions_removes_expired_agent() {
        let registry = AgentRegistry::default();
        let (outbound_tx, _outbound_rx) = mpsc::unbounded_channel();
        let registration = AgentRegistration {
            server_uuid: "server-1".to_string(),
            agent_id: "agent-1".to_string(),
            auth_key: "key".to_string(),
            platform: AgentPlatform::Linux,
            version: "0.1.0".to_string(),
            workspace_roots: vec![WorkspaceRootSummary {
                name: "game-root".to_string(),
                logical_path: "/game-root".to_string(),
            }],
            primary_log_path: "/srv/game/server.log".to_string(),
        };

        let registered = registry.register(registration, outbound_tx).await;
        {
            let mut guard = registry.sessions.write().await;
            let session = guard.get_mut("agent-1").expect("session should exist");
            session.online_agent.last_heartbeat_at_ms =
                now_millis().saturating_sub(HEARTBEAT_TIMEOUT.as_millis() as u64 + 1);
        }

        let removed = registry.reap_stale_sessions().await;

        assert_eq!(removed, 1);
        assert!(registry.get("agent-1").await.is_none());
        assert!(registry.get_by_server_uuid("server-1").await.is_none());
        assert_eq!(registered.agent_id, "agent-1");
    }
}
