use axum::Router;
use backend::{
    agent_registry::AgentRegistry,
    build_app_with_registry,
    models::{
        AgentClientMessage, AgentPlatform, AgentRegistration, AgentServerMessage,
        WorkspaceRootSummary,
    },
};
use futures::{SinkExt, StreamExt};
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn make_lazy_db() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed")
}

async fn spawn_app(app: Router) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    (format!("ws://{address}/api/agents/connect"), server)
}

#[tokio::test]
async fn agent_websocket_route_accepts_upgrade() {
    let registry = AgentRegistry::default();
    let app = build_app_with_registry(make_lazy_db(), registry);
    let (url, server) = spawn_app(app).await;

    let result = connect_async(&url).await;

    server.abort();
    assert!(result.is_ok(), "websocket upgrade should succeed");
}

#[tokio::test]
async fn valid_registration_marks_agent_online_and_returns_ack() {
    let registry = AgentRegistry::default();
    let app = build_app_with_registry(make_lazy_db(), registry.clone());
    let (url, server) = spawn_app(app).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        agent_id: "agent-1".to_string(),
        token: "test-token".to_string(),
        platform: AgentPlatform::Linux,
        version: "0.1.0".to_string(),
        workspace_roots: vec![WorkspaceRootSummary {
            name: "game-root".to_string(),
            logical_path: "/game-root".to_string(),
        }],
        primary_log_path: "/srv/game/server.log".to_string(),
    });

    socket
        .send(Message::Text(
            serde_json::to_string(&registration)
                .expect("registration json")
                .into(),
        ))
        .await
        .expect("registration should send");

    let reply = socket
        .next()
        .await
        .expect("ack frame should exist")
        .expect("ack frame should be readable");
    let ack = serde_json::from_str::<AgentServerMessage>(
        reply.into_text().expect("ack should be text").as_ref(),
    )
    .expect("ack should be valid json");

    match ack {
        AgentServerMessage::Registered(payload) => {
            assert_eq!(payload.agent_id, "agent-1");
            assert!(!payload.session_id.is_empty());
        }
    }

    let online_agent = registry
        .get("agent-1")
        .await
        .expect("agent should be registered");
    assert_eq!(
        online_agent.registration.primary_log_path,
        "/srv/game/server.log"
    );
    assert_eq!(online_agent.registration.workspace_roots.len(), 1);

    server.abort();
}
