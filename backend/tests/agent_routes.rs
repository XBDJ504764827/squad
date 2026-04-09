use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    Router,
};
use backend::{
    agent_registry::AgentRegistry,
    build_app_with_registry,
    models::{
        AgentClientMessage, AgentCommand, AgentCommandResult, AgentFileChanged, AgentHeartbeat,
        AgentLogChunk, AgentPlatform, AgentRegistration, AgentServerMessage, AgentStreamEvent,
        LogEnvelope, OnlineAgentSummary, WorkspaceRootSummary,
    },
};
use dotenvy::from_path_override;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{env, path::Path};
use tower::ServiceExt;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerAgentAuthResponse {
    server_uuid: String,
    has_key: bool,
    key_preview: Option<String>,
    plain_key: Option<String>,
}

fn make_lazy_db() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed")
}

async fn make_test_db() -> sqlx::PgPool {
    let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    from_path_override(&env_path).ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL should exist");

    PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("test db should connect")
}

async fn ensure_server_tables(db: &sqlx::PgPool) {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS managed_servers (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            ip TEXT NOT NULL,
            rcon_port INTEGER NOT NULL,
            rcon_password TEXT NOT NULL,
            server_uuid TEXT NOT NULL UNIQUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE (ip, rcon_port)
        )
        "#,
    )
    .execute(db)
    .await
    .expect("managed_servers table should exist");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS server_agent_auth (
            server_uuid TEXT PRIMARY KEY,
            key_hash TEXT NOT NULL,
            key_preview TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            rotated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(db)
    .await
    .expect("server_agent_auth table should exist");
}

async fn insert_server_fixture(db: &sqlx::PgPool, server_uuid: &str) {
    ensure_server_tables(db).await;
    let rcon_port = 20000 + (server_uuid.bytes().fold(0_u16, |acc, byte| {
        acc.wrapping_add(byte as u16)
    }) % 20000);

    sqlx::query(
        r#"
        INSERT INTO managed_servers (name, ip, rcon_port, rcon_password, server_uuid)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (server_uuid) DO UPDATE
        SET name = EXCLUDED.name,
            ip = EXCLUDED.ip,
            rcon_port = EXCLUDED.rcon_port,
            rcon_password = EXCLUDED.rcon_password
        "#,
    )
    .bind(format!("test-server-{server_uuid}"))
    .bind("127.0.0.1")
    .bind(i32::from(rcon_port))
    .bind("secret")
    .bind(server_uuid)
    .execute(db)
    .await
    .expect("managed server fixture should insert");
}

async fn cleanup_server_fixture(db: &sqlx::PgPool, server_uuid: &str) {
    let _ = sqlx::query("DELETE FROM server_agent_auth WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
    let _ = sqlx::query("DELETE FROM managed_servers WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
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

async fn provision_agent_auth_key(app: &Router, server_uuid: &str) -> ServerAgentAuthResponse {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-auth-key"))
                .method("POST")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should read");
    serde_json::from_slice(&body).expect("response body should be valid json")
}

#[tokio::test]
async fn agent_websocket_route_accepts_upgrade() {
    let registry = AgentRegistry::default();
    let app = build_app_with_registry(make_lazy_db(), registry);
    let (url, server) = spawn_app(app.clone()).await;

    let result = connect_async(&url).await;

    server.abort();
    assert!(result.is_ok(), "websocket upgrade should succeed");
}

#[tokio::test]
async fn registration_is_rejected_when_server_key_is_not_provisioned() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;

    let registry = AgentRegistry::default();
    let app = build_app_with_registry(db.clone(), registry.clone());
    let (url, server) = spawn_app(app).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    socket
        .send(Message::Text(
            json!({
                "type": "agent.register",
                "payload": {
                    "serverUuid": server_uuid,
                    "agentId": "agent-1",
                    "authKey": "missing-key",
                    "platform": "linux",
                    "version": "0.1.0",
                    "workspaceRoots": [],
                    "primaryLogPath": "/srv/game/server.log"
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("registration should send");

    let reply = socket
        .next()
        .await
        .expect("rejection frame should exist")
        .expect("rejection frame should be readable");
    let text = reply.into_text().expect("rejection should be text");
    assert!(text.contains("agent auth key is not provisioned"));
    assert!(registry.get("agent-1").await.is_none());

    cleanup_server_fixture(&db, &server_uuid).await;
    server.abort();
}

#[tokio::test]
async fn registration_is_rejected_when_server_key_is_invalid() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;

    let registry = AgentRegistry::default();
    let app = build_app_with_registry(db.clone(), registry.clone());
    let provisioned = provision_agent_auth_key(&app, &server_uuid).await;
    assert_eq!(provisioned.server_uuid, server_uuid);
    assert!(provisioned.has_key);
    assert!(provisioned.key_preview.is_some());

    let (url, server) = spawn_app(app).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    socket
        .send(Message::Text(
            json!({
                "type": "agent.register",
                "payload": {
                    "serverUuid": server_uuid,
                    "agentId": "agent-1",
                    "authKey": "wrong-key",
                    "platform": "linux",
                    "version": "0.1.0",
                    "workspaceRoots": [],
                    "primaryLogPath": "/srv/game/server.log"
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("registration should send");

    let reply = socket
        .next()
        .await
        .expect("rejection frame should exist")
        .expect("rejection frame should be readable");
    let text = reply.into_text().expect("rejection should be text");
    assert!(text.contains("agent auth key is invalid"));
    assert!(registry.get("agent-1").await.is_none());

    cleanup_server_fixture(&db, &server_uuid).await;
    server.abort();
}

#[tokio::test]
async fn valid_registration_marks_agent_online_and_returns_ack() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;

    let registry = AgentRegistry::default();
    let app = build_app_with_registry(db.clone(), registry.clone());
    let provisioned = provision_agent_auth_key(&app, &server_uuid).await;
    let (url, server) = spawn_app(app.clone()).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        server_uuid: server_uuid.clone(),
        agent_id: "agent-1".to_string(),
        auth_key: provisioned
            .plain_key
            .clone()
            .expect("plain key should be returned"),
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

    cleanup_server_fixture(&db, &server_uuid).await;
    server.abort();
}

#[tokio::test]
async fn heartbeat_updates_agent_last_seen_timestamp() {
    let registry = AgentRegistry::default();
    let app = build_app_with_registry(make_lazy_db(), registry.clone());
    let (url, server) = spawn_app(app.clone()).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        server_uuid: "server-1".to_string(),
        agent_id: "agent-1".to_string(),
        auth_key: "test-auth-key".to_string(),
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
    let (url, server) = spawn_app(app.clone()).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        server_uuid: "server-1".to_string(),
        agent_id: "agent-1".to_string(),
        auth_key: "test-auth-key".to_string(),
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

#[tokio::test]
async fn log_chunk_is_broadcast_to_agent_event_subscribers() {
    let registry = AgentRegistry::default();
    let app = build_app_with_registry(make_lazy_db(), registry.clone());
    let (url, server) = spawn_app(app.clone()).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        server_uuid: "server-1".to_string(),
        agent_id: "agent-1".to_string(),
        auth_key: "test-auth-key".to_string(),
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

    let mut receiver = registry.subscribe_events("agent-1").await;

    socket
        .send(Message::Text(
            serde_json::to_string(&AgentClientMessage::LogChunk(AgentLogChunk {
                entries: vec![LogEnvelope {
                    agent_id: "agent-1".to_string(),
                    source: "server".to_string(),
                    cursor: "1".to_string(),
                    line_number: 1,
                    raw_line: "server started".to_string(),
                    observed_at: "1".to_string(),
                }],
            }))
            .expect("log chunk json")
            .into(),
        ))
        .await
        .expect("log chunk should send");

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .expect("event should arrive")
        .expect("receiver should stay open");

    match event {
        AgentStreamEvent::LogChunk(payload) => {
            assert_eq!(payload.entries.len(), 1);
            assert_eq!(payload.entries[0].raw_line, "server started");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    server.abort();
}

#[tokio::test]
async fn file_change_is_broadcast_to_agent_event_subscribers() {
    let registry = AgentRegistry::default();
    let app = build_app_with_registry(make_lazy_db(), registry.clone());
    let (url, server) = spawn_app(app).await;
    let (mut socket, _) = connect_async(&url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        server_uuid: "server-1".to_string(),
        agent_id: "agent-1".to_string(),
        auth_key: "test-auth-key".to_string(),
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

    let mut receiver = registry.subscribe_events("agent-1").await;

    socket
        .send(Message::Text(
            serde_json::to_string(&AgentClientMessage::FileChanged(AgentFileChanged {
                logical_path: "/game-root/server.cfg".to_string(),
            }))
            .expect("file changed json")
            .into(),
        ))
        .await
        .expect("file changed should send");

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .expect("event should arrive")
        .expect("receiver should stay open");

    match event {
        AgentStreamEvent::FileChanged(payload) => {
            assert_eq!(payload.logical_path, "/game-root/server.cfg");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    server.abort();
}

#[tokio::test]
async fn online_agents_route_returns_binding_state_for_registered_agents() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    let second_server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;
    insert_server_fixture(&db, &second_server_uuid).await;

    let registry = AgentRegistry::default();
    let app = build_app_with_registry(db.clone(), registry.clone());
    let first_auth = provision_agent_auth_key(&app, &server_uuid).await;
    let second_auth = provision_agent_auth_key(&app, &second_server_uuid).await;
    let (url, server) = spawn_app(app.clone()).await;
    let (mut socket_1, _) = connect_async(&url).await.expect("ws should connect");
    let (mut socket_2, _) = connect_async(&url).await.expect("ws should connect");

    let registration_1 = AgentClientMessage::Register(AgentRegistration {
        server_uuid: server_uuid.clone(),
        agent_id: "agent-1".to_string(),
        auth_key: first_auth.plain_key.expect("first plain key"),
        platform: AgentPlatform::Linux,
        version: "0.1.0".to_string(),
        workspace_roots: vec![],
        primary_log_path: "/srv/game/server.log".to_string(),
    });
    let registration_2 = AgentClientMessage::Register(AgentRegistration {
        server_uuid: second_server_uuid.clone(),
        agent_id: "agent-2".to_string(),
        auth_key: second_auth.plain_key.expect("second plain key"),
        platform: AgentPlatform::Linux,
        version: "0.1.0".to_string(),
        workspace_roots: vec![],
        primary_log_path: "/srv/game/second.log".to_string(),
    });

    socket_1
        .send(Message::Text(
            serde_json::to_string(&registration_1)
                .expect("registration json")
                .into(),
        ))
        .await
        .expect("registration should send");
    let ack_1 = socket_1
        .next()
        .await
        .expect("ack frame should exist")
        .expect("ack frame should be readable");
    let ack_1 = serde_json::from_str::<AgentServerMessage>(
        ack_1.into_text().expect("ack should be text").as_ref(),
    )
    .expect("ack should be valid json");
    match ack_1 {
        AgentServerMessage::Registered(payload) => {
            assert_eq!(payload.agent_id, "agent-1");
            assert!(!payload.session_id.is_empty());
        }
        AgentServerMessage::Command(_) => panic!("unexpected command before dispatch"),
    }

    socket_2
        .send(Message::Text(
            serde_json::to_string(&registration_2)
                .expect("registration json")
                .into(),
        ))
        .await
        .expect("registration should send");
    let ack_2 = socket_2
        .next()
        .await
        .expect("ack frame should exist")
        .expect("ack frame should be readable");
    let ack_2 = serde_json::from_str::<AgentServerMessage>(
        ack_2.into_text().expect("ack should be text").as_ref(),
    )
    .expect("ack should be valid json");
    match ack_2 {
        AgentServerMessage::Registered(payload) => {
            assert_eq!(payload.agent_id, "agent-2");
            assert!(!payload.session_id.is_empty());
        }
        AgentServerMessage::Command(_) => panic!("unexpected command before dispatch"),
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/agents/online")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should read");
    let agents: Vec<OnlineAgentSummary> =
        serde_json::from_slice(&body).expect("response body should be valid json");

    assert_eq!(agents.len(), 2);

    let agent_1 = agents
        .iter()
        .find(|agent| agent.agent_id == "agent-1")
        .expect("agent-1 should exist");
    assert_eq!(agent_1.platform, AgentPlatform::Linux);
    assert_eq!(agent_1.server_uuid, server_uuid);
    assert_eq!(agent_1.version, "0.1.0");
    assert_eq!(agent_1.primary_log_path, "/srv/game/server.log");

    let agent_2 = agents
        .iter()
        .find(|agent| agent.agent_id == "agent-2")
        .expect("agent-2 should exist");
    assert_eq!(agent_2.platform, AgentPlatform::Linux);
    assert_eq!(agent_2.server_uuid, second_server_uuid);
    assert_eq!(agent_2.version, "0.1.0");
    assert_eq!(agent_2.primary_log_path, "/srv/game/second.log");

    cleanup_server_fixture(&db, &server_uuid).await;
    cleanup_server_fixture(&db, &second_server_uuid).await;
    server.abort();
}
