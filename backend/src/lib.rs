pub mod agent_registry;
pub mod agent_ws;
pub mod models;
pub mod rcon;

use std::{convert::Infallible, env, net::SocketAddr, path::Path};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use dotenvy::from_path_override;
use futures::stream;
use models::{
    ActionResponse, AddServerRequest, AgentCommand, AgentStreamEvent, DashboardResponse,
    ErrorResponse, FilePathQuery, FileReadRequest, FileReadResult, FileTreeRequest, FileTreeResult,
    FileWriteRequest, FileWriteRequestBody, FileWriteResult, HealthResponse, ManagedServer,
    ManagedServerDetailResponse, UpdateServerRequest,
};
use serde::de::DeserializeOwned;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use crate::agent_registry::AgentRegistry;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    agent_registry: AgentRegistry,
}

struct AppConfig {
    database_url: String,
    port: u16,
    database_max_connections: u32,
}

pub async fn run() {
    let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    from_path_override(&env_path).ok();

    let config = AppConfig::from_env();

    let db = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to PostgreSQL");

    initialize_database(&db)
        .await
        .expect("failed to initialize PostgreSQL schema");

    let app = build_app(db);

    let address = SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind TCP listener");

    println!("Rust backend listening on http://{}", address);

    axum::serve(listener, app)
        .await
        .expect("backend server exited unexpectedly");
}

pub fn build_app(db: PgPool) -> Router {
    build_app_with_registry(db, AgentRegistry::default())
}

pub fn build_app_with_registry(db: PgPool, agent_registry: AgentRegistry) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/dashboard", get(dashboard))
        .route("/api/servers", post(add_server))
        .route("/api/agents/connect", get(connect_agent_ws))
        .route("/api/agents/{agent_id}/events", get(stream_agent_events))
        .route(
            "/api/agents/{agent_id}/files/tree",
            get(get_agent_file_tree),
        )
        .route(
            "/api/agents/{agent_id}/files/content",
            get(get_agent_file_content).put(put_agent_file_content),
        )
        .route(
            "/api/servers/{server_uuid}",
            get(get_server).put(update_server).delete(delete_server),
        )
        .with_state(AppState { db, agent_registry })
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

async fn connect_agent_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| agent_ws::serve(socket, state.agent_registry))
}

async fn stream_agent_events(
    AxumPath(agent_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.agent_registry.subscribe_events(&agent_id).await;

    let stream = stream::unfold(receiver, |mut receiver| async move {
        let next_event = match receiver.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                let warning = serde_json::json!({
                    "message": "agent event stream lagged",
                    "skipped": skipped,
                });
                let event = Event::default()
                    .event("agent.warning")
                    .data(warning.to_string());
                return Some((Ok(event), receiver));
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
        };

        let event = Ok(agent_stream_event_to_sse(next_event));
        Some((event, receiver))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn get_agent_file_tree(
    AxumPath(agent_id): AxumPath<String>,
    Query(query): Query<FilePathQuery>,
    State(state): State<AppState>,
) -> Result<Json<FileTreeResult>, (StatusCode, Json<ErrorResponse>)> {
    let result = dispatch_agent_command::<FileTreeResult>(
        &state.agent_registry,
        &agent_id,
        AgentCommand::FileTree(FileTreeRequest {
            logical_path: query.path,
        }),
    )
    .await?;

    Ok(Json(result))
}

async fn get_agent_file_content(
    AxumPath(agent_id): AxumPath<String>,
    Query(query): Query<FilePathQuery>,
    State(state): State<AppState>,
) -> Result<Json<FileReadResult>, (StatusCode, Json<ErrorResponse>)> {
    let result = dispatch_agent_command::<FileReadResult>(
        &state.agent_registry,
        &agent_id,
        AgentCommand::FileRead(FileReadRequest {
            logical_path: query.path,
        }),
    )
    .await?;

    Ok(Json(result))
}

async fn put_agent_file_content(
    AxumPath(agent_id): AxumPath<String>,
    State(state): State<AppState>,
    Json(payload): Json<FileWriteRequestBody>,
) -> Result<Json<FileWriteResult>, (StatusCode, Json<ErrorResponse>)> {
    let result = dispatch_agent_command::<FileWriteResult>(
        &state.agent_registry,
        &agent_id,
        AgentCommand::FileWrite(FileWriteRequest {
            logical_path: payload.logical_path,
            content: payload.content,
            expected_version: payload.expected_version,
        }),
    )
    .await?;

    Ok(Json(result))
}

impl AppConfig {
    fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://squad:squad@127.0.0.1:5432/squad".to_string()),
            port: read_env_or_default("PORT", 3000),
            database_max_connections: read_env_or_default("DATABASE_MAX_CONNECTIONS", 10),
        }
    }
}

fn read_env_or_default<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + Copy,
{
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn dashboard(
    State(state): State<AppState>,
) -> Result<Json<DashboardResponse>, (StatusCode, Json<ErrorResponse>)> {
    let servers = fetch_servers(&state.db)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器列表失败"))?;

    Ok(Json(DashboardResponse::from_servers(&servers)))
}

async fn get_server(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ManagedServerDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    let server = fetch_server_by_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器详情失败"))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "服务器不存在"))?;

    let online_agent = state.agent_registry.get(&server_uuid).await;

    Ok(Json(ManagedServerDetailResponse::from_server(
        &server,
        online_agent.as_ref(),
    )))
}

async fn add_server(
    State(state): State<AppState>,
    Json(payload): Json<AddServerRequest>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim().to_string();
    let ip = payload.ip.trim().to_string();
    let password = payload.rcon_password.trim().to_string();

    validate_server_payload(&name, &ip, payload.rcon_port, &password)?;

    let duplicate_exists = server_exists_for_other(&state.db, &name, &ip, payload.rcon_port, None)
        .await
        .map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "检查服务器是否重复时失败",
            )
        })?;

    if duplicate_exists {
        return Err(error_response(
            StatusCode::CONFLICT,
            "服务器名称或服务器地址已存在，请勿重复添加",
        ));
    }

    rcon::validate_rcon_credentials(&ip, payload.rcon_port, &password)
        .await
        .map_err(|message| error_response(StatusCode::BAD_REQUEST, &message))?;

    let server_uuid = insert_server(&state.db, &name, &ip, payload.rcon_port, &password)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器写入数据库失败"))?;

    Ok(Json(ActionResponse {
        message: "服务器添加成功，RCON 验证已通过".to_string(),
        server_uuid,
    }))
}

async fn update_server(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateServerRequest>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim().to_string();
    let ip = payload.ip.trim().to_string();
    let password = payload.rcon_password.trim().to_string();

    validate_server_payload(&name, &ip, payload.rcon_port, &password)?;

    let existing_server = fetch_server_by_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器详情失败"))?;

    if existing_server.is_none() {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let duplicate_exists =
        server_exists_for_other(&state.db, &name, &ip, payload.rcon_port, Some(&server_uuid))
            .await
            .map_err(|_| {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "检查服务器是否重复时失败",
                )
            })?;

    if duplicate_exists {
        return Err(error_response(
            StatusCode::CONFLICT,
            "服务器名称或服务器地址已存在，请勿重复修改",
        ));
    }

    rcon::validate_rcon_credentials(&ip, payload.rcon_port, &password)
        .await
        .map_err(|message| error_response(StatusCode::BAD_REQUEST, &message))?;

    let updated = update_server_record(
        &state.db,
        &server_uuid,
        &name,
        &ip,
        payload.rcon_port,
        &password,
    )
    .await
    .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器更新失败"))?;

    if !updated {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    Ok(Json(ActionResponse {
        message: "服务器更新成功，RCON 验证已通过".to_string(),
        server_uuid,
    }))
}

async fn delete_server(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let deleted = delete_server_record(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "删除服务器失败"))?;

    if !deleted {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    Ok(Json(ActionResponse {
        message: "服务器删除成功".to_string(),
        server_uuid,
    }))
}

async fn dispatch_agent_command<T>(
    registry: &AgentRegistry,
    agent_id: &str,
    command: AgentCommand,
) -> Result<T, (StatusCode, Json<ErrorResponse>)>
where
    T: DeserializeOwned,
{
    let result = registry
        .dispatch_command(agent_id, command)
        .await
        .map_err(|message| map_dispatch_error(&message))?;

    if !result.success {
        let message = result
            .error
            .unwrap_or_else(|| "agent command failed".to_string());
        return Err(error_response(StatusCode::BAD_REQUEST, &message));
    }

    let payload = result.payload.ok_or_else(|| {
        error_response(
            StatusCode::BAD_GATEWAY,
            "agent command succeeded but payload is missing",
        )
    })?;

    serde_json::from_value(payload).map_err(|err| {
        error_response(
            StatusCode::BAD_GATEWAY,
            &format!("agent returned invalid payload: {err}"),
        )
    })
}

fn validate_server_payload(
    name: &str,
    ip: &str,
    rcon_port: u16,
    password: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if name.is_empty() || ip.is_empty() || password.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "服务器名称、IP、RCON 端口和 RCON 密码不能为空",
        ));
    }

    if rcon_port == 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "RCON 端口必须大于 0",
        ));
    }

    Ok(())
}

fn agent_stream_event_to_sse(event: AgentStreamEvent) -> Event {
    match event {
        AgentStreamEvent::LogChunk(payload) => Event::default()
            .event("agent.logChunk")
            .data(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())),
        AgentStreamEvent::FileChanged(payload) => Event::default()
            .event("agent.fileChanged")
            .data(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())),
    }
}

async fn initialize_database(db: &PgPool) -> Result<(), sqlx::Error> {
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
    .await?;

    sqlx::query(
        r#"
        ALTER TABLE managed_servers
        ADD COLUMN IF NOT EXISTS server_uuid TEXT
        "#,
    )
    .execute(db)
    .await?;

    let missing_uuid_server_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM managed_servers
        WHERE server_uuid IS NULL OR server_uuid = ''
        "#,
    )
    .fetch_all(db)
    .await?;

    for server_id in missing_uuid_server_ids {
        sqlx::query(
            r#"
            UPDATE managed_servers
            SET server_uuid = $1
            WHERE id = $2
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(server_id)
        .execute(db)
        .await?;
    }

    sqlx::query(
        r#"
        ALTER TABLE managed_servers
        ALTER COLUMN server_uuid SET NOT NULL
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS managed_servers_server_uuid_idx
        ON managed_servers (server_uuid)
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn fetch_servers(db: &PgPool) -> Result<Vec<ManagedServer>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ManagedServer>(
        r#"
        SELECT name, ip, rcon_port, server_uuid, rcon_password
        FROM managed_servers
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

async fn fetch_server_by_uuid(
    db: &PgPool,
    server_uuid: &str,
) -> Result<Option<ManagedServer>, sqlx::Error> {
    sqlx::query_as::<_, ManagedServer>(
        r#"
        SELECT name, ip, rcon_port, server_uuid, rcon_password
        FROM managed_servers
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .fetch_optional(db)
    .await
}

async fn server_exists_for_other(
    db: &PgPool,
    name: &str,
    ip: &str,
    rcon_port: u16,
    excluded_server_uuid: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let existing = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM managed_servers
        WHERE (name = $1 OR (ip = $2 AND rcon_port = $3))
          AND ($4::TEXT IS NULL OR server_uuid <> $4)
        "#,
    )
    .bind(name)
    .bind(ip)
    .bind(i32::from(rcon_port))
    .bind(excluded_server_uuid)
    .fetch_one(db)
    .await?;

    Ok(existing > 0)
}

async fn insert_server(
    db: &PgPool,
    name: &str,
    ip: &str,
    rcon_port: u16,
    rcon_password: &str,
) -> Result<String, sqlx::Error> {
    let server_uuid = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO managed_servers (name, ip, rcon_port, rcon_password, server_uuid)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(name)
    .bind(ip)
    .bind(i32::from(rcon_port))
    .bind(rcon_password)
    .bind(&server_uuid)
    .execute(db)
    .await?;

    Ok(server_uuid)
}

async fn update_server_record(
    db: &PgPool,
    server_uuid: &str,
    name: &str,
    ip: &str,
    rcon_port: u16,
    rcon_password: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE managed_servers
        SET name = $1,
            ip = $2,
            rcon_port = $3,
            rcon_password = $4
        WHERE server_uuid = $5
        "#,
    )
    .bind(name)
    .bind(ip)
    .bind(i32::from(rcon_port))
    .bind(rcon_password)
    .bind(server_uuid)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

async fn delete_server_record(db: &PgPool, server_uuid: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM managed_servers
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            message: message.to_string(),
        }),
    )
}

fn map_dispatch_error(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    if message.contains("offline") {
        return error_response(StatusCode::CONFLICT, message);
    }
    if message.contains("timed out") {
        return error_response(StatusCode::GATEWAY_TIMEOUT, message);
    }

    error_response(StatusCode::BAD_GATEWAY, message)
}
