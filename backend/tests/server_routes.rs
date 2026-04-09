use std::{
    env,
    path::Path,
    sync::atomic::{AtomicU16, Ordering},
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use backend::{
    agent_registry::AgentRegistry,
    build_app,
    build_app_with_registry,
    models::{
        AgentPlatform, AgentRegistration, ManagedServer, ManagedServerDetailResponse, OnlineAgent,
        ServerAgentBindingResponse, WorkspaceRootSummary,
    },
};
use dotenvy::from_path_override;
use serde::{Deserialize, de::DeserializeOwned};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

static NEXT_TEST_RCON_PORT: AtomicU16 = AtomicU16::new(31000);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerAgentAuthResponse {
    server_uuid: String,
    has_key: bool,
    key_preview: Option<String>,
    plain_key: Option<String>,
    rotated_at: Option<u64>,
    agent_online: bool,
    agent_id: Option<String>,
    last_heartbeat_at: Option<u64>,
    workspace_roots: Vec<WorkspaceRootSummary>,
    primary_log_path: String,
}

fn make_lazy_db() -> sqlx::PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
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

async fn ensure_binding_tables(db: &sqlx::PgPool) {
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
        CREATE TABLE IF NOT EXISTS server_agent_bindings (
            server_uuid TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL UNIQUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(db)
    .await
    .expect("server_agent_bindings table should exist");

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

fn next_test_rcon_port() -> i32 {
    i32::from(NEXT_TEST_RCON_PORT.fetch_add(1, Ordering::Relaxed))
}

async fn insert_server_fixture(db: &sqlx::PgPool, server_uuid: &str) {
    ensure_binding_tables(db).await;

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
    .bind(next_test_rcon_port())
    .bind("secret")
    .bind(server_uuid)
    .execute(db)
    .await
    .expect("managed server fixture should insert");
}

async fn insert_binding_fixture(db: &sqlx::PgPool, server_uuid: &str, agent_id: &str) {
    insert_server_fixture(db, server_uuid).await;

    sqlx::query(
        r#"
        INSERT INTO server_agent_bindings (server_uuid, agent_id)
        VALUES ($1, $2)
        ON CONFLICT (server_uuid) DO UPDATE
        SET agent_id = EXCLUDED.agent_id,
            updated_at = NOW()
        "#,
    )
    .bind(server_uuid)
    .bind(agent_id)
    .execute(db)
    .await
    .expect("binding fixture should insert");
}

async fn cleanup_server_fixture(db: &sqlx::PgPool, server_uuid: &str) {
    let _ = sqlx::query("DELETE FROM server_agent_auth WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
    let _ = sqlx::query("DELETE FROM server_agent_bindings WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
    let _ = sqlx::query("DELETE FROM managed_servers WHERE server_uuid = $1")
        .bind(server_uuid)
        .execute(db)
        .await;
}

async fn register_online_agent(
    registry: &AgentRegistry,
    server_uuid: &str,
    agent_id: &str,
    primary_log_path: &str,
) -> OnlineAgent {
    let (outbound_tx, _outbound_rx) = mpsc::unbounded_channel();
    registry
        .register(
            AgentRegistration {
                server_uuid: server_uuid.to_string(),
                agent_id: agent_id.to_string(),
                auth_key: "test-auth-key".to_string(),
                platform: AgentPlatform::Linux,
                version: "0.1.0".to_string(),
                workspace_roots: vec![WorkspaceRootSummary {
                    name: "game-root".to_string(),
                    logical_path: "/game-root".to_string(),
                }],
                primary_log_path: primary_log_path.to_string(),
            },
            outbound_tx,
        )
        .await;

    registry
        .get(agent_id)
        .await
        .expect("agent should be registered")
}

async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should read");
    serde_json::from_slice(&body).expect("response body should be valid json")
}

#[tokio::test]
async fn server_detail_route_exists() {
    let db = make_lazy_db();
    let app = build_app(db);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_update_route_exists() {
    let db = make_lazy_db();
    let app = build_app(db);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid")
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"edited","ip":"127.0.0.1","rconPort":25575,"rconPassword":"secret"}"#,
                ))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_delete_route_exists() {
    let db = make_lazy_db();
    let app = build_app(db);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid")
                .method("DELETE")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_agent_binding_routes_exist() {
    let db = make_lazy_db();
    let app = build_app(db);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/agent-binding")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(get_response.status(), StatusCode::NOT_FOUND);

    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/agent-binding")
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"agentId":"agent-1"}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(put_response.status(), StatusCode::NOT_FOUND);

    let delete_response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/agent-binding")
                .method("DELETE")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(delete_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_agent_auth_routes_exist() {
    let db = make_lazy_db();
    let app = build_app(db);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/agent-auth")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(get_response.status(), StatusCode::NOT_FOUND);

    let post_response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/agent-auth-key")
                .method("POST")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(post_response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn server_detail_response_includes_agent_metadata_from_online_agent() {
    let server_uuid = "server-1".to_string();
    let server = ManagedServer {
        name: "测试服务器".to_string(),
        ip: "127.0.0.1".to_string(),
        rcon_port: 25575,
        server_uuid: server_uuid.clone(),
        rcon_password: "secret".to_string(),
    };
    let registration = AgentRegistration {
        server_uuid: "server-1".to_string(),
        agent_id: server_uuid.clone(),
        auth_key: "test-auth-key".to_string(),
        platform: AgentPlatform::Linux,
        version: "0.1.0".to_string(),
        workspace_roots: vec![WorkspaceRootSummary {
            name: "game-root".to_string(),
            logical_path: "/game-root".to_string(),
        }],
        primary_log_path: "/srv/game/server.log".to_string(),
    };
    let detail = ManagedServerDetailResponse::from_server(
        &server,
        Some(&server_uuid),
        Some(&OnlineAgent {
            session_id: "session-1".to_string(),
            connected_at_ms: 1,
            last_heartbeat_at_ms: 2,
            registration,
        }),
    );

    assert_eq!(detail.agent_id, Some(server_uuid));
    assert!(detail.agent_online);
    assert_eq!(detail.workspace_roots.len(), 1);
    assert_eq!(detail.workspace_roots[0].logical_path, "/game-root");
    assert_eq!(detail.primary_log_path, "/srv/game/server.log");
}

#[test]
fn server_detail_response_defaults_to_unbound_when_binding_is_missing() {
    let server = ManagedServer {
        name: "测试服务器".to_string(),
        ip: "127.0.0.1".to_string(),
        rcon_port: 25575,
        server_uuid: "server-1".to_string(),
        rcon_password: "secret".to_string(),
    };

    let detail = ManagedServerDetailResponse::from_server(&server, None, None);

    assert_eq!(detail.server_uuid, "server-1");
    assert_eq!(detail.agent_id, None);
    assert!(!detail.agent_online);
    assert!(detail.workspace_roots.is_empty());
    assert_eq!(detail.primary_log_path, "");
}

#[test]
fn server_detail_response_keeps_binding_when_agent_is_offline() {
    let server = ManagedServer {
        name: "测试服务器".to_string(),
        ip: "127.0.0.1".to_string(),
        rcon_port: 25575,
        server_uuid: "server-2".to_string(),
        rcon_password: "secret".to_string(),
    };

    let detail = ManagedServerDetailResponse::from_server(&server, Some("agent-9"), None);

    assert_eq!(detail.agent_id, Some("agent-9".to_string()));
    assert!(!detail.agent_online);
    assert!(detail.workspace_roots.is_empty());
    assert_eq!(detail.primary_log_path, "");
}

#[test]
fn server_agent_binding_response_includes_binding_and_online_agent_metadata() {
    let online_agent = OnlineAgent {
        session_id: "session-1".to_string(),
        connected_at_ms: 1,
        last_heartbeat_at_ms: 99,
        registration: AgentRegistration {
            server_uuid: "server-1".to_string(),
            agent_id: "agent-7".to_string(),
            auth_key: "test-auth-key".to_string(),
            platform: AgentPlatform::Linux,
            version: "0.1.0".to_string(),
            workspace_roots: vec![WorkspaceRootSummary {
                name: "game-root".to_string(),
                logical_path: "/game-root".to_string(),
            }],
            primary_log_path: "/srv/game/server.log".to_string(),
        },
    };

    let response =
        ServerAgentBindingResponse::from_binding("server-1", Some("agent-7"), Some(&online_agent));

    assert_eq!(response.server_uuid, "server-1");
    assert_eq!(response.agent_id, Some("agent-7".to_string()));
    assert!(response.agent_online);
    assert_eq!(response.workspace_roots.len(), 1);
    assert_eq!(response.primary_log_path, "/srv/game/server.log");
    assert_eq!(response.last_heartbeat_at, Some(99));
}

#[tokio::test]
async fn binding_routes_return_404_when_server_is_missing() {
    let db = make_test_db().await;
    ensure_binding_tables(&db).await;
    let app = build_app(db);
    let missing_server_uuid = format!("missing-{}", Uuid::new_v4());

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/servers/{missing_server_uuid}/agent-binding"
                ))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);

    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/servers/{missing_server_uuid}/agent-binding"
                ))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"agentId":"agent-missing"}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(put_response.status(), StatusCode::NOT_FOUND);

    let delete_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/servers/{missing_server_uuid}/agent-binding"
                ))
                .method("DELETE")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(delete_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn put_server_agent_binding_rejects_empty_agent_id() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;
    let app = build_app(db.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-binding"))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"agentId":"   "}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_server_fixture(&db, &server_uuid).await;
}

#[tokio::test]
async fn put_server_agent_binding_persists_binding_and_reads_online_state() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;

    let registry = AgentRegistry::default();
    let online_agent = register_online_agent(
        &registry,
        &server_uuid,
        "agent-online",
        "/srv/game/server.log",
    )
    .await;
    let app = build_app_with_registry(db.clone(), registry);

    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-binding"))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"agentId":"agent-online"}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(put_response.status(), StatusCode::OK);
    let put_body: ServerAgentBindingResponse = read_json(put_response).await;
    assert_eq!(put_body.agent_id, Some("agent-online".to_string()));
    assert!(put_body.agent_online);
    assert_eq!(put_body.workspace_roots, online_agent.registration.workspace_roots);
    assert_eq!(put_body.primary_log_path, "/srv/game/server.log");
    assert_eq!(
        put_body.last_heartbeat_at,
        Some(online_agent.last_heartbeat_at_ms)
    );

    let get_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-binding"))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(get_binding_response.status(), StatusCode::OK);
    let binding_body: ServerAgentBindingResponse = read_json(get_binding_response).await;
    assert_eq!(binding_body.agent_id, Some("agent-online".to_string()));
    assert!(binding_body.agent_online);

    cleanup_server_fixture(&db, &server_uuid).await;
}

#[tokio::test]
async fn put_server_agent_binding_rejects_agent_already_bound_to_other_server() {
    let db = make_test_db().await;
    let first_server_uuid = format!("server-{}", Uuid::new_v4());
    let second_server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &first_server_uuid).await;
    insert_server_fixture(&db, &second_server_uuid).await;
    let app = build_app(db.clone());

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{first_server_uuid}/agent-binding"))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"agentId":"agent-conflict"}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(first_response.status(), StatusCode::OK);

    let second_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{second_server_uuid}/agent-binding"))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"agentId":"agent-conflict"}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(second_response.status(), StatusCode::CONFLICT);

    cleanup_server_fixture(&db, &first_server_uuid).await;
    cleanup_server_fixture(&db, &second_server_uuid).await;
}

#[tokio::test]
async fn delete_server_agent_binding_is_idempotent() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_binding_fixture(&db, &server_uuid, "agent-to-delete").await;
    let app = build_app(db.clone());

    let first_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-binding"))
                .method("DELETE")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(first_delete_response.status(), StatusCode::OK);

    let second_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-binding"))
                .method("DELETE")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(second_delete_response.status(), StatusCode::OK);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-binding"))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(get_response.status(), StatusCode::OK);
    let body: ServerAgentBindingResponse = read_json(get_response).await;
    assert_eq!(body.agent_id, None);
    assert!(!body.agent_online);

    cleanup_server_fixture(&db, &server_uuid).await;
}

#[tokio::test]
async fn post_server_agent_auth_key_rotates_key_and_get_reads_status() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    insert_server_fixture(&db, &server_uuid).await;
    let app = build_app(db.clone());

    let first_response = app
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
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body: ServerAgentAuthResponse = read_json(first_response).await;
    assert_eq!(first_body.server_uuid, server_uuid);
    assert!(first_body.has_key);
    assert!(first_body.plain_key.as_deref().unwrap_or_default().len() >= 16);
    assert!(!first_body.key_preview.as_deref().unwrap_or_default().is_empty());
    assert!(first_body.rotated_at.is_some());
    assert!(!first_body.agent_online);
    assert_eq!(first_body.agent_id, None);
    assert_eq!(first_body.last_heartbeat_at, None);
    assert!(first_body.workspace_roots.is_empty());
    assert_eq!(first_body.primary_log_path, "");

    let second_response = app
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
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body: ServerAgentAuthResponse = read_json(second_response).await;
    assert_ne!(first_body.plain_key, second_body.plain_key);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/agent-auth"))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body: ServerAgentAuthResponse = read_json(get_response).await;
    assert_eq!(get_body.server_uuid, server_uuid);
    assert!(get_body.has_key);
    assert_eq!(get_body.plain_key, None);
    assert_eq!(get_body.key_preview, second_body.key_preview);

    cleanup_server_fixture(&db, &server_uuid).await;
}
