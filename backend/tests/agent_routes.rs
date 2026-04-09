use axum::Router;
use backend::{
    agent_registry::AgentRegistry,
    build_app_with_registry,
    models::{
        AgentClientMessage, AgentCommand, AgentCommandResult, AgentHeartbeat, AgentPlatform,
        AgentRegistration, AgentServerMessage, WorkspaceRootSummary,
    },
};
use futures::{SinkExt, StreamExt};
use serde_json::json;
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
        AgentServerMessage::Command(_) => panic!("unexpected command before dispatch"),
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

#[tokio::test]
async fn heartbeat_updates_agent_last_seen_timestamp() {
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
    let _ = socket
        .next()
        .await
        .expect("ack frame")
        .expect("ack readable");

    let connected = registry
        .get("agent-1")
        .await
        .expect("agent should be registered");

    socket
        .send(Message::Text(
            serde_json::to_string(&AgentClientMessage::Heartbeat(AgentHeartbeat {}))
                .expect("heartbeat json")
                .into(),
        ))
        .await
        .expect("heartbeat should send");

    let updated = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let online = registry
                .get("agent-1")
                .await
                .expect("agent should stay registered");
            if online.last_heartbeat_at_ms > connected.last_heartbeat_at_ms {
                break online;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("heartbeat timestamp should update");

    assert!(updated.last_heartbeat_at_ms >= updated.connected_at_ms);

    server.abort();
}

#[tokio::test]
async fn dispatch_command_bridges_response_between_backend_and_agent() {
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
    let _ = socket
        .next()
        .await
        .expect("ack frame")
        .expect("ack readable");

    let registry_clone = registry.clone();
    let dispatch = tokio::spawn(async move {
        registry_clone
            .dispatch_command("agent-1", AgentCommand::Ping)
            .await
            .expect("command should complete")
    });

    let command_frame = socket
        .next()
        .await
        .expect("command frame should exist")
        .expect("command frame should be readable");
    let command = serde_json::from_str::<AgentServerMessage>(
        command_frame
            .into_text()
            .expect("command should be text")
            .as_ref(),
    )
    .expect("command should be valid json");

    let request_id = match command {
        AgentServerMessage::Command(payload) => {
            assert_eq!(payload.command, AgentCommand::Ping);
            payload.request_id
        }
        other => panic!("unexpected server message: {other:?}"),
    };

    socket
        .send(Message::Text(
            serde_json::to_string(&AgentClientMessage::CommandResult(AgentCommandResult {
                request_id: request_id.clone(),
                success: true,
                payload: Some(json!({ "pong": true })),
                error: None,
            }))
            .expect("response json")
            .into(),
        ))
        .await
        .expect("response should send");

    let result = dispatch.await.expect("dispatch task should finish");
    assert_eq!(result.request_id, request_id);
    assert_eq!(result.payload, Some(json!({ "pong": true })));

    server.abort();
}
