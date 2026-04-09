use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use backend::{
    agent_registry::AgentRegistry,
    build_app_with_registry,
    models::{
        AgentClientMessage, AgentCommand, AgentCommandResult, AgentPlatform, AgentRegistration,
        AgentServerMessage, FileReadRequest, FileReadResult, FileTreeEntry, FileTreeResult,
        FileWriteRequestBody, FileWriteResult, WorkspaceRootSummary,
    },
};
use dotenvy::from_path_override;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{env, path::Path};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

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
    .bind(32001_i32)
    .bind("secret")
    .bind(server_uuid)
    .execute(db)
    .await
    .expect("managed server fixture should insert");
}

async fn cleanup_server_fixture(db: &sqlx::PgPool, server_uuid: &str) {
    let _ = sqlx::query("DELETE FROM server_parse_rules WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
    let _ = sqlx::query("DELETE FROM server_agent_auth WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
    let _ = sqlx::query("DELETE FROM managed_servers WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
}

async fn spawn_ws_app(app: Router) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    (format!("ws://{address}/api/agents/connect"), server)
}

async fn provision_agent_auth_key(app: &Router, server_uuid: &str) -> String {
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
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("response body json");
    payload["plainKey"]
        .as_str()
        .expect("plain key should exist")
        .to_string()
}

async fn register_agent(
    url: &str,
    auth_key: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (mut socket, _) = connect_async(url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        server_uuid: "server-1".to_string(),
        agent_id: "agent-1".to_string(),
        auth_key: auth_key.to_string(),
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

    let _ = socket.next().await.expect("ack").expect("ack readable");
    socket
}

#[tokio::test]
async fn http_file_tree_route_bridges_to_agent_command() {
    let db = make_test_db().await;
    insert_server_fixture(&db, "server-1").await;
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(db.clone(), registry.clone());
    let http_app = build_app_with_registry(db.clone(), registry);
    let auth_key = provision_agent_auth_key(&http_app, "server-1").await;
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url, &auth_key).await;

    let agent = tokio::spawn(async move {
        let frame = socket
            .next()
            .await
            .expect("command")
            .expect("command readable");
        let command =
            serde_json::from_str::<AgentServerMessage>(frame.into_text().expect("text").as_ref())
                .expect("command json");
        let request_id = match command {
            AgentServerMessage::Command(payload) => match payload.command {
                AgentCommand::FileTree(request) => {
                    assert_eq!(request.logical_path, "/game-root");
                    payload.request_id
                }
                other => panic!("unexpected command: {other:?}"),
            },
            other => panic!("unexpected message: {other:?}"),
        };

        socket
            .send(Message::Text(
                serde_json::to_string(&AgentClientMessage::CommandResult(AgentCommandResult {
                    request_id,
                    success: true,
                    payload: Some(json!(FileTreeResult {
                        entries: vec![FileTreeEntry {
                            logical_path: "/game-root/server.cfg".to_string(),
                            is_dir: false,
                            size: Some(12),
                        }],
                    })),
                    error: None,
                }))
                .expect("response json")
                .into(),
            ))
            .await
            .expect("response should send");
    });

    let response = http_app
        .oneshot(
            Request::builder()
                .uri("/api/servers/server-1/files/tree?path=%2Fgame-root")
                .method("GET")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should return");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let result: FileTreeResult = serde_json::from_slice(&body).expect("body json");
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].logical_path, "/game-root/server.cfg");

    agent.await.expect("agent task should finish");
    cleanup_server_fixture(&db, "server-1").await;
    server.abort();
}

#[tokio::test]
async fn http_file_read_route_bridges_to_agent_command() {
    let db = make_test_db().await;
    insert_server_fixture(&db, "server-1").await;
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(db.clone(), registry.clone());
    let http_app = build_app_with_registry(db.clone(), registry);
    let auth_key = provision_agent_auth_key(&http_app, "server-1").await;
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url, &auth_key).await;

    let agent = tokio::spawn(async move {
        let frame = socket
            .next()
            .await
            .expect("command")
            .expect("command readable");
        let command =
            serde_json::from_str::<AgentServerMessage>(frame.into_text().expect("text").as_ref())
                .expect("command json");
        let request_id = match command {
            AgentServerMessage::Command(payload) => match payload.command {
                AgentCommand::FileRead(FileReadRequest { logical_path }) => {
                    assert_eq!(logical_path, "/game-root/server.cfg");
                    payload.request_id
                }
                other => panic!("unexpected command: {other:?}"),
            },
            other => panic!("unexpected message: {other:?}"),
        };

        socket
            .send(Message::Text(
                serde_json::to_string(&AgentClientMessage::CommandResult(AgentCommandResult {
                    request_id,
                    success: true,
                    payload: Some(json!(FileReadResult {
                        logical_path: "/game-root/server.cfg".to_string(),
                        content: "hostname=test\n".to_string(),
                        version: "v1".to_string(),
                    })),
                    error: None,
                }))
                .expect("response json")
                .into(),
            ))
            .await
            .expect("response should send");
    });

    let response = http_app
        .oneshot(
            Request::builder()
                .uri("/api/servers/server-1/files/content?path=%2Fgame-root%2Fserver.cfg")
                .method("GET")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should return");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let result: FileReadResult = serde_json::from_slice(&body).expect("body json");
    assert_eq!(result.content, "hostname=test\n");
    assert_eq!(result.version, "v1");

    agent.await.expect("agent task should finish");
    cleanup_server_fixture(&db, "server-1").await;
    server.abort();
}

#[tokio::test]
async fn http_file_write_route_bridges_to_agent_command() {
    let db = make_test_db().await;
    insert_server_fixture(&db, "server-1").await;
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(db.clone(), registry.clone());
    let http_app = build_app_with_registry(db.clone(), registry);
    let auth_key = provision_agent_auth_key(&http_app, "server-1").await;
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url, &auth_key).await;

    let agent = tokio::spawn(async move {
        let frame = socket
            .next()
            .await
            .expect("command")
            .expect("command readable");
        let command =
            serde_json::from_str::<AgentServerMessage>(frame.into_text().expect("text").as_ref())
                .expect("command json");
        let request_id = match command {
            AgentServerMessage::Command(payload) => match payload.command {
                AgentCommand::FileWrite(request) => {
                    assert_eq!(request.logical_path, "/game-root/server.cfg");
                    assert_eq!(request.content, "hostname=new\n");
                    assert_eq!(request.expected_version.as_deref(), Some("v1"));
                    payload.request_id
                }
                other => panic!("unexpected command: {other:?}"),
            },
            other => panic!("unexpected message: {other:?}"),
        };

        socket
            .send(Message::Text(
                serde_json::to_string(&AgentClientMessage::CommandResult(AgentCommandResult {
                    request_id,
                    success: true,
                    payload: Some(json!(FileWriteResult {
                        logical_path: "/game-root/server.cfg".to_string(),
                        version: "v2".to_string(),
                    })),
                    error: None,
                }))
                .expect("response json")
                .into(),
            ))
            .await
            .expect("response should send");
    });

    let response = http_app
        .oneshot(
            Request::builder()
                .uri("/api/servers/server-1/files/content")
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&FileWriteRequestBody {
                        logical_path: "/game-root/server.cfg".to_string(),
                        content: "hostname=new\n".to_string(),
                        expected_version: Some("v1".to_string()),
                    })
                    .expect("request json"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("response should return");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let result: FileWriteResult = serde_json::from_slice(&body).expect("body json");
    assert_eq!(result.version, "v2");

    agent.await.expect("agent task should finish");
    cleanup_server_fixture(&db, "server-1").await;
    server.abort();
}

#[tokio::test]
async fn http_file_bridge_maps_agent_business_error_to_bad_request() {
    let db = make_test_db().await;
    insert_server_fixture(&db, "server-1").await;
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(db.clone(), registry.clone());
    let http_app = build_app_with_registry(db.clone(), registry);
    let auth_key = provision_agent_auth_key(&http_app, "server-1").await;
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url, &auth_key).await;

    let agent = tokio::spawn(async move {
        let frame = socket
            .next()
            .await
            .expect("command")
            .expect("command readable");
        let command =
            serde_json::from_str::<AgentServerMessage>(frame.into_text().expect("text").as_ref())
                .expect("command json");
        let request_id = match command {
            AgentServerMessage::Command(payload) => payload.request_id,
            other => panic!("unexpected message: {other:?}"),
        };

        socket
            .send(Message::Text(
                serde_json::to_string(&AgentClientMessage::CommandResult(AgentCommandResult {
                    request_id,
                    success: false,
                    payload: None,
                    error: Some("version conflict".to_string()),
                }))
                .expect("response json")
                .into(),
            ))
            .await
            .expect("response should send");
    });

    let response = http_app
        .oneshot(
            Request::builder()
                .uri("/api/servers/server-1/files/content?path=%2Fgame-root%2Fserver.cfg")
                .method("GET")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should return");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    agent.await.expect("agent task should finish");
    cleanup_server_fixture(&db, "server-1").await;
    server.abort();
}
