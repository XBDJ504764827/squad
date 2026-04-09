use std::{
    env,
    io,
    path::Path,
    sync::{Arc, Mutex},
    sync::atomic::{AtomicU16, Ordering},
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use backend::{
    build_app,
    models::{
        AgentPlatform, AgentRegistration, ManagedServer, ManagedServerDetailResponse, OnlineAgent,
        WorkspaceRootSummary,
    },
};
use dotenvy::from_path_override;
use serde::{Deserialize, de::DeserializeOwned};
use sqlx::postgres::PgPoolOptions;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerFeatureFlagsResponse {
    server_uuid: String,
    disable_vehicle_claiming: bool,
    force_all_vehicle_availability: bool,
    force_all_deployable_availability: bool,
    force_all_role_availability: bool,
    disable_vehicle_team_requirement: bool,
    disable_vehicle_kit_requirement: bool,
    no_respawn_timer: bool,
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS server_feature_flags (
            server_uuid TEXT PRIMARY KEY,
            disable_vehicle_claiming BOOLEAN NOT NULL DEFAULT FALSE,
            force_all_vehicle_availability BOOLEAN NOT NULL DEFAULT FALSE,
            force_all_deployable_availability BOOLEAN NOT NULL DEFAULT FALSE,
            force_all_role_availability BOOLEAN NOT NULL DEFAULT FALSE,
            disable_vehicle_team_requirement BOOLEAN NOT NULL DEFAULT FALSE,
            disable_vehicle_kit_requirement BOOLEAN NOT NULL DEFAULT FALSE,
            no_respawn_timer BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(db)
    .await
    .expect("server_feature_flags table should exist");
}

fn next_test_rcon_port() -> i32 {
    i32::from(NEXT_TEST_RCON_PORT.fetch_add(1, Ordering::Relaxed))
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
    .bind(next_test_rcon_port())
    .bind("secret")
    .bind(server_uuid)
    .execute(db)
    .await
    .expect("managed server fixture should insert");
}

async fn cleanup_server_fixture(db: &sqlx::PgPool, server_uuid: &str) {
    let _ = sqlx::query("DELETE FROM server_feature_flags WHERE server_uuid = $1")
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

async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should read");
    serde_json::from_slice(&body).expect("response body should be valid json")
}

async fn write_rcon_packet(
    stream: &mut tokio::net::TcpStream,
    request_id: i32,
    packet_type: i32,
    body: &str,
) -> io::Result<()> {
    let body_bytes = body.as_bytes();
    let packet_size = 4 + 4 + body_bytes.len() + 2;
    let packet_size =
        i32::try_from(packet_size).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "packet too large"))?;

    stream.write_i32_le(packet_size).await?;
    stream.write_i32_le(request_id).await?;
    stream.write_i32_le(packet_type).await?;
    stream.write_all(body_bytes).await?;
    stream.write_all(&[0, 0]).await?;
    stream.flush().await?;

    Ok(())
}

async fn read_rcon_packet(stream: &mut tokio::net::TcpStream) -> io::Result<(i32, i32, String)> {
    let packet_size = stream.read_i32_le().await?;
    let request_id = stream.read_i32_le().await?;
    let packet_type = stream.read_i32_le().await?;
    let body_size = usize::try_from(packet_size - 10)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid body size"))?;
    let mut body = vec![0_u8; body_size];
    stream.read_exact(&mut body).await?;
    let mut terminator = [0_u8; 2];
    stream.read_exact(&mut terminator).await?;

    Ok((request_id, packet_type, String::from_utf8_lossy(&body).to_string()))
}

async fn spawn_mock_rcon_server() -> (u16, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock rcon listener should bind");
    let port = listener
        .local_addr()
        .expect("mock rcon listener should have local addr")
        .port();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let commands_for_task = Arc::clone(&commands);

    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let commands = Arc::clone(&commands_for_task);

            tokio::spawn(async move {
                let Ok((request_id, packet_type, body)) = read_rcon_packet(&mut stream).await else {
                    return;
                };

                if packet_type != 3 || body != "secret" {
                    let _ = write_rcon_packet(&mut stream, -1, 2, "").await;
                    return;
                }

                if write_rcon_packet(&mut stream, request_id, 2, "").await.is_err() {
                    return;
                }

                while let Ok((request_id, packet_type, body)) = read_rcon_packet(&mut stream).await {
                    if packet_type != 2 {
                        continue;
                    }

                    commands.lock().expect("commands lock").push(body.clone());
                    let _ = write_rcon_packet(&mut stream, request_id, 0, "OK").await;
                }
            });
        }
    });

    (port, commands, task)
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
async fn server_agent_binding_routes_are_removed() {
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
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);

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
    assert_eq!(put_response.status(), StatusCode::NOT_FOUND);

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
    assert_eq!(delete_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_file_routes_exist() {
    let db = make_lazy_db();
    let app = build_app(db);

    let tree_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/files/tree?path=%2Fgame-root")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(tree_response.status(), StatusCode::NOT_FOUND);

    let content_response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/files/content?path=%2Fgame-root%2Fserver.cfg")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(content_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_event_route_exists() {
    let db = make_lazy_db();
    let app = build_app(db);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/events")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
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

#[tokio::test]
async fn server_feature_flag_routes_exist() {
    let db = make_lazy_db();
    let app = build_app(db);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/feature-flags")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(get_response.status(), StatusCode::NOT_FOUND);

    let put_response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/feature-flags/disableVehicleClaiming")
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"enabled":true}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(put_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_parse_rules_routes_exist() {
    let db = make_lazy_db();
    let app = build_app(db);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/parse-rules")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(get_response.status(), StatusCode::NOT_FOUND);

    let put_response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/parse-rules")
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"rules":[]}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");
    assert_ne!(put_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_parsed_events_route_exists() {
    let db = make_lazy_db();
    let app = build_app(db);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/servers/test-server-uuid/parsed-events?eventType=chat&limit=50")
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
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

#[tokio::test]
async fn get_server_feature_flags_returns_all_false_when_record_missing() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    let (port, commands, task) = spawn_mock_rcon_server().await;
    ensure_server_tables(&db).await;

    sqlx::query(
        r#"
        INSERT INTO managed_servers (name, ip, rcon_port, rcon_password, server_uuid)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(format!("feature-server-{server_uuid}"))
    .bind("127.0.0.1")
    .bind(i32::from(port))
    .bind("secret")
    .bind(&server_uuid)
    .execute(&db)
    .await
    .expect("managed server fixture should insert");

    let app = build_app(db.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/feature-flags"))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_eq!(response.status(), StatusCode::OK);
    let body: ServerFeatureFlagsResponse = read_json(response).await;
    assert_eq!(body.server_uuid, server_uuid);
    assert!(!body.disable_vehicle_claiming);
    assert!(!body.force_all_vehicle_availability);
    assert!(!body.force_all_deployable_availability);
    assert!(!body.force_all_role_availability);
    assert!(!body.disable_vehicle_team_requirement);
    assert!(!body.disable_vehicle_kit_requirement);
    assert!(!body.no_respawn_timer);

    let recorded_commands = commands.lock().expect("commands lock").clone();
    assert_eq!(recorded_commands.len(), 7);
    assert!(recorded_commands.contains(&"AdminDisableVehicleClaiming 0".to_string()));
    assert!(recorded_commands.contains(&"AdminNoRespawnTimer 0".to_string()));

    cleanup_server_fixture(&db, &server_uuid).await;
    task.abort();
}

#[tokio::test]
async fn put_server_feature_flag_executes_rcon_and_persists_state() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    let (port, commands, task) = spawn_mock_rcon_server().await;
    ensure_server_tables(&db).await;

    sqlx::query(
        r#"
        INSERT INTO managed_servers (name, ip, rcon_port, rcon_password, server_uuid)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(format!("feature-server-{server_uuid}"))
    .bind("127.0.0.1")
    .bind(i32::from(port))
    .bind("secret")
    .bind(&server_uuid)
    .execute(&db)
    .await
    .expect("managed server fixture should insert");

    let app = build_app(db.clone());
    let put_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/feature-flags/disableVehicleClaiming"))
                .method("PUT")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"enabled":true}"#))
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_eq!(put_response.status(), StatusCode::OK);
    let put_body: ServerFeatureFlagsResponse = read_json(put_response).await;
    assert!(put_body.disable_vehicle_claiming);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/feature-flags"))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body: ServerFeatureFlagsResponse = read_json(get_response).await;
    assert!(get_body.disable_vehicle_claiming);

    let recorded_commands = commands.lock().expect("commands lock").clone();
    assert!(recorded_commands.iter().any(|command| command == "AdminDisableVehicleClaiming 1"));

    cleanup_server_fixture(&db, &server_uuid).await;
    task.abort();
}

#[tokio::test]
async fn get_server_feature_flags_replays_all_expected_rcon_commands() {
    let db = make_test_db().await;
    let server_uuid = format!("server-{}", Uuid::new_v4());
    let (port, commands, task) = spawn_mock_rcon_server().await;
    ensure_server_tables(&db).await;

    sqlx::query(
        r#"
        INSERT INTO managed_servers (name, ip, rcon_port, rcon_password, server_uuid)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(format!("feature-server-{server_uuid}"))
    .bind("127.0.0.1")
    .bind(i32::from(port))
    .bind("secret")
    .bind(&server_uuid)
    .execute(&db)
    .await
    .expect("managed server fixture should insert");

    sqlx::query(
        r#"
        INSERT INTO server_feature_flags (
            server_uuid,
            disable_vehicle_claiming,
            force_all_vehicle_availability,
            force_all_deployable_availability,
            force_all_role_availability,
            disable_vehicle_team_requirement,
            disable_vehicle_kit_requirement,
            no_respawn_timer
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(&server_uuid)
    .bind(true)
    .bind(false)
    .bind(true)
    .bind(false)
    .bind(true)
    .bind(false)
    .bind(true)
    .execute(&db)
    .await
    .expect("feature flag fixture should insert");

    let app = build_app(db.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/servers/{server_uuid}/feature-flags"))
                .method("GET")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("response should be returned");

    assert_eq!(response.status(), StatusCode::OK);
    let body: ServerFeatureFlagsResponse = read_json(response).await;
    assert!(body.disable_vehicle_claiming);
    assert!(!body.force_all_vehicle_availability);
    assert!(body.force_all_deployable_availability);
    assert!(!body.force_all_role_availability);
    assert!(body.disable_vehicle_team_requirement);
    assert!(!body.disable_vehicle_kit_requirement);
    assert!(body.no_respawn_timer);

    let recorded_commands = commands.lock().expect("commands lock").clone();
    assert_eq!(recorded_commands.len(), 7);
    assert!(recorded_commands.contains(&"AdminDisableVehicleClaiming 1".to_string()));
    assert!(recorded_commands.contains(&"AdminForceAllVehicleAvailability 0".to_string()));
    assert!(recorded_commands.contains(&"AdminForceAllDeployableAvailability 1".to_string()));
    assert!(recorded_commands.contains(&"AdminForceAllRoleAvailability 0".to_string()));
    assert!(recorded_commands.contains(&"AdminDisableVehicleTeamRequirement 1".to_string()));
    assert!(recorded_commands.contains(&"AdminDisableVehicleKitRequirement 0".to_string()));
    assert!(recorded_commands.contains(&"AdminNoRespawnTimer 1".to_string()));

    cleanup_server_fixture(&db, &server_uuid).await;
    task.abort();
}
