pub mod agent_registry;
pub mod agent_ws;
pub mod models;
pub mod rcon;

use std::{collections::HashMap, convert::Infallible, env, net::SocketAddr, path::Path};

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
    ManagedServerDetailResponse, OnlineAgentSummary, ServerAgentBindingResponse,
    UpdateServerAgentBindingRequest, UpdateServerRequest,
};
use serde::de::DeserializeOwned;
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions};
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
        .route("/api/agents/online", get(list_online_agents))
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
        .route(
            "/api/servers/{server_uuid}/agent-binding",
            get(get_server_agent_binding)
                .put(put_server_agent_binding)
                .delete(delete_server_agent_binding),
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

async fn list_online_agents(
    State(state): State<AppState>,
) -> Result<Json<Vec<OnlineAgentSummary>>, (StatusCode, Json<ErrorResponse>)> {
    let binding_by_agent = fetch_all_server_agent_bindings(&state.db)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取 agent 绑定关系失败"))?
        .into_iter()
        .map(|binding| (binding.agent_id, binding.server_uuid))
        .collect::<HashMap<_, _>>();

    let mut items = state
        .agent_registry
        .list()
        .await
        .into_iter()
        .map(|agent| {
            let bound_server_uuid = binding_by_agent.get(&agent.registration.agent_id).cloned();
            OnlineAgentSummary {
                agent_id: agent.registration.agent_id.clone(),
                platform: agent.registration.platform.clone(),
                version: agent.registration.version.clone(),
                workspace_roots: agent.registration.workspace_roots.clone(),
                primary_log_path: agent.registration.primary_log_path.clone(),
                connected_at: agent.connected_at_ms,
                last_heartbeat_at: agent.last_heartbeat_at_ms,
                is_bound: bound_server_uuid.is_some(),
                bound_server_uuid,
            }
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));

    Ok(Json(items))
}

async fn get_server(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ManagedServerDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    let server = fetch_server_by_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器详情失败"))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "服务器不存在"))?;

    Ok(Json(ManagedServerDetailResponse::from_server(
        &server,
        None,
        None,
    )))
}

async fn get_server_agent_binding(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ServerAgentBindingResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let binding = fetch_server_agent_binding_by_server_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取绑定关系失败"))?;
    let online_agent = match binding.as_ref() {
        Some(binding) => state.agent_registry.get(&binding.agent_id).await,
        None => None,
    };

    Ok(Json(ServerAgentBindingResponse::from_binding(
        &server_uuid,
        binding.as_ref().map(|binding| binding.agent_id.as_str()),
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

async fn put_server_agent_binding(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateServerAgentBindingRequest>,
) -> Result<Json<ServerAgentBindingResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let agent_id = payload.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "agentId 不能为空"));
    }

    let existing_binding = fetch_server_agent_binding_by_agent_id(&state.db, &agent_id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取绑定关系失败"))?;
    if let Some(existing_binding) = existing_binding {
        if existing_binding.server_uuid != server_uuid {
            return Err(error_response(
                StatusCode::CONFLICT,
                "agent 已绑定到其他服务器",
            ));
        }
    }

    upsert_server_agent_binding(&state.db, &server_uuid, &agent_id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入绑定关系失败"))?;

    let online_agent = state.agent_registry.get(&agent_id).await;

    Ok(Json(ServerAgentBindingResponse::from_binding(
        &server_uuid,
        Some(&agent_id),
        online_agent.as_ref(),
    )))
}

async fn delete_server_agent_binding(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ServerAgentBindingResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    delete_server_agent_binding_record(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "删除绑定关系失败"))?;

    Ok(Json(ServerAgentBindingResponse::from_binding(
        &server_uuid,
        None,
        None,
    )))
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
    .await?;

    sqlx::query(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'server_agent_bindings_server_uuid_fkey'
                  AND conrelid = 'server_agent_bindings'::regclass
            ) THEN
                ALTER TABLE server_agent_bindings
                ADD CONSTRAINT server_agent_bindings_server_uuid_fkey
                FOREIGN KEY (server_uuid)
                REFERENCES managed_servers (server_uuid)
                ON DELETE CASCADE;
            END IF;
        END $$;
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

#[derive(Clone, FromRow)]
struct ServerAgentBindingRecord {
    server_uuid: String,
    agent_id: String,
}

async fn fetch_all_server_agent_bindings(
    db: &PgPool,
) -> Result<Vec<ServerAgentBindingRecord>, sqlx::Error> {
    sqlx::query_as::<_, ServerAgentBindingRecord>(
        r#"
        SELECT server_uuid, agent_id
        FROM server_agent_bindings
        "#,
    )
    .fetch_all(db)
    .await
}

async fn fetch_server_agent_binding_by_server_uuid(
    db: &PgPool,
    server_uuid: &str,
) -> Result<Option<ServerAgentBindingRecord>, sqlx::Error> {
    sqlx::query_as::<_, ServerAgentBindingRecord>(
        r#"
        SELECT server_uuid, agent_id
        FROM server_agent_bindings
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .fetch_optional(db)
    .await
}

async fn fetch_server_agent_binding_by_agent_id(
    db: &PgPool,
    agent_id: &str,
) -> Result<Option<ServerAgentBindingRecord>, sqlx::Error> {
    sqlx::query_as::<_, ServerAgentBindingRecord>(
        r#"
        SELECT server_uuid, agent_id
        FROM server_agent_bindings
        WHERE agent_id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_optional(db)
    .await
}

async fn upsert_server_agent_binding(
    db: &PgPool,
    server_uuid: &str,
    agent_id: &str,
) -> Result<(), sqlx::Error> {
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
    .await?;

    Ok(())
}

async fn delete_server_agent_binding_record(
    db: &PgPool,
    server_uuid: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM server_agent_bindings
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .execute(db)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initialize_database_creates_server_agent_bindings_table_and_fk() {
        let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
        from_path_override(&env_path).ok();

        let config = AppConfig::from_env();
        let db = PgPoolOptions::new()
            .max_connections(1)
            .connect(&config.database_url)
            .await
            .expect("failed to connect to PostgreSQL");

        initialize_database(&db)
            .await
            .expect("failed to initialize PostgreSQL schema");

        let table_exists = sqlx::query_scalar::<_, Option<String>>(
            "SELECT to_regclass('public.server_agent_bindings')::text",
        )
        .fetch_one(&db)
        .await
        .expect("failed to query server_agent_bindings table");

        assert!(table_exists.is_some());

        let constraint_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM pg_constraint
            WHERE conname = 'server_agent_bindings_server_uuid_fkey'
              AND conrelid = 'server_agent_bindings'::regclass
            "#,
        )
        .fetch_one(&db)
        .await
        .expect("failed to query server_agent_bindings foreign key");

        assert!(constraint_count > 0);
    }
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

async fn server_exists(db: &PgPool, server_uuid: &str) -> Result<bool, sqlx::Error> {
    let existing = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM managed_servers
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .fetch_one(db)
    .await?;

    Ok(existing > 0)
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
