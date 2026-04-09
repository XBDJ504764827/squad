use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{AgentRegistered, AgentRegistration, OnlineAgent};

#[derive(Clone, Default)]
pub struct AgentRegistry {
    inner: Arc<RwLock<HashMap<String, OnlineAgent>>>,
}

impl AgentRegistry {
    pub async fn register(&self, registration: AgentRegistration) -> AgentRegistered {
        let session_id = Uuid::new_v4().to_string();
        let online_agent = OnlineAgent {
            session_id: session_id.clone(),
            connected_at_ms: now_millis(),
            registration: registration.clone(),
        };

        self.inner
            .write()
            .await
            .insert(registration.agent_id.clone(), online_agent);

        AgentRegistered {
            agent_id: registration.agent_id,
            session_id,
        }
    }

    pub async fn get(&self, agent_id: &str) -> Option<OnlineAgent> {
        self.inner.read().await.get(agent_id).cloned()
    }

    pub async fn remove_session(&self, agent_id: &str, session_id: &str) {
        let mut guard = self.inner.write().await;
        let should_remove = guard
            .get(agent_id)
            .map(|agent| agent.session_id == session_id)
            .unwrap_or(false);
        if should_remove {
            guard.remove(agent_id);
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
