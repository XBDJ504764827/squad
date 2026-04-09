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
    ErrorResponse, FilePathQuery, FileReadRequest, FileReadResult, FileTreeRequest,
    FileTreeResult, FileWriteRequest, FileWriteRequestBody, FileWriteResult, HealthResponse,
    ManagedServer, ManagedServerDetailResponse, OnlineAgent, OnlineAgentSummary, ParseRule,
    ParseRuleKind, ParsedEventQuery, ParsedLogEvent, ReplaceParseRulesRequest,
    ServerAgentAuthResponse, ServerParsedEventsResponse, ServerParseRulesResponse,
    UpdateServerParseRulesRequest, UpdateServerRequest,
};
use regex::Regex;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions, types::Json as SqlxJson};
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
    let reaper_registry = agent_registry.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let _ = reaper_registry.reap_stale_sessions().await;
        }
    });

    Router::new()
        .route("/api/health", get(health))
        .route("/api/dashboard", get(dashboard))
        .route("/api/servers", post(add_server))
        .route("/api/agents/online", get(list_online_agents))
        .route("/api/agents/connect", get(connect_agent_ws))
        .route("/api/agents/{agent_id}/events", get(stream_agent_events))
        .route("/api/servers/{server_uuid}/events", get(stream_server_events))
        .route(
            "/api/agents/{agent_id}/files/tree",
            get(get_agent_file_tree),
        )
        .route(
            "/api/agents/{agent_id}/files/content",
            get(get_agent_file_content).put(put_agent_file_content),
        )
        .route(
            "/api/servers/{server_uuid}/files/tree",
            get(get_server_file_tree),
        )
        .route(
            "/api/servers/{server_uuid}/files/content",
            get(get_server_file_content).put(put_server_file_content),
        )
        .route(
            "/api/servers/{server_uuid}",
            get(get_server).put(update_server).delete(delete_server),
        )
        .route("/api/servers/{server_uuid}/agent-auth", get(get_server_agent_auth))
        .route(
            "/api/servers/{server_uuid}/agent-auth-key",
            post(post_server_agent_auth_key),
        )
        .route(
            "/api/servers/{server_uuid}/parse-rules",
            get(get_server_parse_rules).put(put_server_parse_rules),
        )
        .route(
            "/api/servers/{server_uuid}/parsed-events",
            get(get_server_parsed_events),
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
    ws.on_upgrade(move |socket| agent_ws::serve(socket, state.agent_registry, state.db))
}

async fn stream_agent_events(
    AxumPath(agent_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    stream_events_for_agent(state.agent_registry.clone(), agent_id).await
}

async fn stream_server_events(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)>
{
    let online_agent = require_online_agent_for_server(&state, &server_uuid).await?;
    Ok(stream_events_for_agent(
        state.agent_registry.clone(),
        online_agent.registration.agent_id.clone(),
    )
    .await)
}

async fn stream_events_for_agent(
    registry: AgentRegistry,
    agent_id: String,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let receiver = registry.subscribe_events(&agent_id).await;

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

async fn get_server_file_tree(
    AxumPath(server_uuid): AxumPath<String>,
    Query(query): Query<FilePathQuery>,
    State(state): State<AppState>,
) -> Result<Json<FileTreeResult>, (StatusCode, Json<ErrorResponse>)> {
    let online_agent = require_online_agent_for_server(&state, &server_uuid).await?;
    let result = dispatch_agent_command::<FileTreeResult>(
        &state.agent_registry,
        &online_agent.registration.agent_id,
        AgentCommand::FileTree(FileTreeRequest {
            logical_path: query.path,
        }),
    )
    .await?;

    Ok(Json(result))
}

async fn get_server_file_content(
    AxumPath(server_uuid): AxumPath<String>,
    Query(query): Query<FilePathQuery>,
    State(state): State<AppState>,
) -> Result<Json<FileReadResult>, (StatusCode, Json<ErrorResponse>)> {
    let online_agent = require_online_agent_for_server(&state, &server_uuid).await?;
    let result = dispatch_agent_command::<FileReadResult>(
        &state.agent_registry,
        &online_agent.registration.agent_id,
        AgentCommand::FileRead(FileReadRequest {
            logical_path: query.path,
        }),
    )
    .await?;

    Ok(Json(result))
}

async fn put_server_file_content(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
    Json(payload): Json<FileWriteRequestBody>,
) -> Result<Json<FileWriteResult>, (StatusCode, Json<ErrorResponse>)> {
    let online_agent = require_online_agent_for_server(&state, &server_uuid).await?;
    let result = dispatch_agent_command::<FileWriteResult>(
        &state.agent_registry,
        &online_agent.registration.agent_id,
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
    let mut items = state
        .agent_registry
        .list()
        .await
        .into_iter()
        .map(|agent| {
            OnlineAgentSummary {
                agent_id: agent.registration.agent_id.clone(),
                server_uuid: agent.registration.server_uuid.clone(),
                platform: agent.registration.platform.clone(),
                version: agent.registration.version.clone(),
                workspace_roots: agent.registration.workspace_roots.clone(),
                primary_log_path: agent.registration.primary_log_path.clone(),
                connected_at: agent.connected_at_ms,
                last_heartbeat_at: agent.last_heartbeat_at_ms,
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

    let online_agent = state.agent_registry.get_by_server_uuid(&server_uuid).await;

    Ok(Json(ManagedServerDetailResponse::from_server(
        &server,
        online_agent
            .as_ref()
            .map(|agent| agent.registration.agent_id.as_str()),
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

async fn get_server_agent_auth(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ServerAgentAuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let auth = fetch_server_agent_auth_by_server_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取 agent 鉴权状态失败"))?;
    let online_agent = state.agent_registry.get_by_server_uuid(&server_uuid).await;

    Ok(Json(ServerAgentAuthResponse::from_auth(
        &server_uuid,
        auth.as_ref().map(|record| record.key_preview.clone()),
        auth.as_ref().map(|record| record.rotated_at_ms as u64),
        None,
        online_agent.as_ref(),
    )))
}

async fn post_server_agent_auth_key(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ServerAgentAuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let plain_key = generate_agent_auth_key();
    let key_preview = build_agent_auth_key_preview(&plain_key);
    let key_hash = hash_agent_auth_key(&plain_key);
    let auth = upsert_server_agent_auth(&state.db, &server_uuid, &key_hash, &key_preview)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入 agent key 失败"))?;
    let online_agent = state.agent_registry.get_by_server_uuid(&server_uuid).await;

    Ok(Json(ServerAgentAuthResponse::from_auth(
        &server_uuid,
        Some(auth.key_preview),
        Some(auth.rotated_at_ms as u64),
        Some(plain_key),
        online_agent.as_ref(),
    )))
}

async fn get_server_parse_rules(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<ServerParseRulesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let ruleset = fetch_server_parse_rules_by_server_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取解析规则失败"))?;
    let online_agent = state.agent_registry.get_by_server_uuid(&server_uuid).await;

    Ok(Json(ServerParseRulesResponse::from_rules(
        &server_uuid,
        ruleset.as_ref().map(|record| record.version as u64),
        ruleset.map(|record| record.rules_json.0).unwrap_or_default(),
        online_agent.as_ref(),
        false,
        "解析规则已读取",
    )))
}

async fn put_server_parse_rules(
    AxumPath(server_uuid): AxumPath<String>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateServerParseRulesRequest>,
) -> Result<Json<ServerParseRulesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    validate_parse_rules(&payload.rules)
        .map_err(|message| error_response(StatusCode::BAD_REQUEST, &message))?;

    let existing_ruleset = fetch_server_parse_rules_by_server_uuid(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取解析规则失败"))?;
    let next_version = existing_ruleset
        .as_ref()
        .map(|record| record.version as u64 + 1)
        .unwrap_or(1);

    upsert_server_parse_rules(&state.db, &server_uuid, next_version, &payload.rules)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入解析规则失败"))?;

    let online_agent = state.agent_registry.get_by_server_uuid(&server_uuid).await;
    let (applied, message) = if let Some(agent) = online_agent.as_ref() {
        match dispatch_agent_command::<serde_json::Value>(
            &state.agent_registry,
            &agent.registration.agent_id,
            AgentCommand::ReplaceParseRules(ReplaceParseRulesRequest {
                version: next_version,
                rules: payload.rules.clone(),
            }),
        )
        .await
        {
            Ok(_) => (true, "解析规则已保存并下发到在线 Agent".to_string()),
            Err((status, error)) => {
                if status == StatusCode::SERVICE_UNAVAILABLE || status == StatusCode::CONFLICT {
                    (false, "解析规则已保存，当前 Agent 不在线".to_string())
                } else {
                    (
                        false,
                        format!("解析规则已保存，但热更新下发失败：{}", error.message),
                    )
                }
            }
        }
    } else {
        (false, "解析规则已保存，等待 Agent 下次连接时同步".to_string())
    };

    Ok(Json(ServerParseRulesResponse::from_rules(
        &server_uuid,
        Some(next_version),
        payload.rules,
        online_agent.as_ref(),
        applied,
        message,
    )))
}

async fn get_server_parsed_events(
    AxumPath(server_uuid): AxumPath<String>,
    Query(query): Query<ParsedEventQuery>,
    State(state): State<AppState>,
) -> Result<Json<ServerParsedEventsResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, &server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    let limit = query.limit.unwrap_or(100).clamp(1, 200) as i64;
    let items = fetch_server_parsed_events(
        &state.db,
        &server_uuid,
        query.event_type.as_deref(),
        query.before,
        limit,
    )
    .await
    .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取结构化事件失败"))?;

    Ok(Json(ServerParsedEventsResponse::from_items(
        &server_uuid,
        query.event_type,
        items,
    )))
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

async fn require_online_agent_for_server(
    state: &AppState,
    server_uuid: &str,
) -> Result<OnlineAgent, (StatusCode, Json<ErrorResponse>)> {
    if !server_exists(&state.db, server_uuid)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取服务器失败"))?
    {
        return Err(error_response(StatusCode::NOT_FOUND, "服务器不存在"));
    }

    state
        .agent_registry
        .get_by_server_uuid(server_uuid)
        .await
        .ok_or_else(|| error_response(StatusCode::SERVICE_UNAVAILABLE, "agent 不在线"))
}

fn validate_parse_rules(rules: &[ParseRule]) -> Result<(), String> {
    for rule in rules {
        if rule.id.trim().is_empty() {
            return Err("parse rule id is required".to_string());
        }
        if rule.event_type.trim().is_empty() {
            return Err(format!("parse rule `{}` eventType is required", rule.id));
        }
        if rule.severity.trim().is_empty() {
            return Err(format!("parse rule `{}` severity is required", rule.id));
        }

        match rule.kind {
            ParseRuleKind::Regex => {
                Regex::new(&rule.pattern)
                    .map_err(|err| format!("invalid parse rule `{}`: {err}", rule.id))?;
            }
        }
    }

    Ok(())
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
        AgentStreamEvent::ParsedEvents(payload) => Event::default()
            .event("agent.parsedEvents")
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
    .await?;

    sqlx::query(
        r#"
        DELETE FROM server_agent_auth
        WHERE server_uuid NOT IN (
            SELECT server_uuid
            FROM managed_servers
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
                WHERE conname = 'server_agent_auth_server_uuid_fkey'
                  AND conrelid = 'server_agent_auth'::regclass
            ) THEN
                ALTER TABLE server_agent_auth
                ADD CONSTRAINT server_agent_auth_server_uuid_fkey
                FOREIGN KEY (server_uuid)
                REFERENCES managed_servers (server_uuid)
                ON DELETE CASCADE;
            END IF;
        END $$;
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS server_parse_rules (
            server_uuid TEXT PRIMARY KEY,
            rules_json JSONB NOT NULL,
            version BIGINT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM server_parse_rules
        WHERE server_uuid NOT IN (
            SELECT server_uuid
            FROM managed_servers
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
                WHERE conname = 'server_parse_rules_server_uuid_fkey'
                  AND conrelid = 'server_parse_rules'::regclass
            ) THEN
                ALTER TABLE server_parse_rules
                ADD CONSTRAINT server_parse_rules_server_uuid_fkey
                FOREIGN KEY (server_uuid)
                REFERENCES managed_servers (server_uuid)
                ON DELETE CASCADE;
            END IF;
        END $$;
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS server_parsed_events (
            id BIGSERIAL PRIMARY KEY,
            server_uuid TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            rule_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            source TEXT NOT NULL,
            cursor TEXT NOT NULL,
            line_number BIGINT NOT NULL,
            raw_line TEXT NOT NULL,
            observed_at_ms BIGINT NOT NULL,
            payload_json JSONB NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM server_parsed_events
        WHERE server_uuid NOT IN (
            SELECT server_uuid
            FROM managed_servers
        )
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS server_parsed_events_server_uuid_observed_idx
        ON server_parsed_events (server_uuid, observed_at_ms DESC, id DESC)
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS server_parsed_events_server_uuid_event_type_observed_idx
        ON server_parsed_events (server_uuid, event_type, observed_at_ms DESC, id DESC)
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
                WHERE conname = 'server_parsed_events_server_uuid_fkey'
                  AND conrelid = 'server_parsed_events'::regclass
            ) THEN
                ALTER TABLE server_parsed_events
                ADD CONSTRAINT server_parsed_events_server_uuid_fkey
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
struct ServerAgentAuthRecord {
    _server_uuid: String,
    key_hash: String,
    key_preview: String,
    rotated_at_ms: i64,
}

#[derive(Clone, FromRow)]
pub(crate) struct ServerParseRulesRecord {
    _server_uuid: String,
    rules_json: SqlxJson<Vec<ParseRule>>,
    version: i64,
}

#[derive(Clone, FromRow)]
struct ServerParsedEventRow {
    agent_id: String,
    rule_id: String,
    event_type: String,
    severity: String,
    source: String,
    cursor: String,
    line_number: i64,
    raw_line: String,
    observed_at_ms: i64,
    payload_json: SqlxJson<std::collections::BTreeMap<String, String>>,
}

async fn fetch_server_agent_auth_by_server_uuid(
    db: &PgPool,
    server_uuid: &str,
) -> Result<Option<ServerAgentAuthRecord>, sqlx::Error> {
    sqlx::query_as::<_, ServerAgentAuthRecord>(
        r#"
        SELECT
            server_uuid AS _server_uuid,
            key_hash,
            key_preview,
            CAST(EXTRACT(EPOCH FROM rotated_at) * 1000 AS BIGINT) AS rotated_at_ms
        FROM server_agent_auth
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .fetch_optional(db)
    .await
}

pub(crate) async fn fetch_server_parse_rules_by_server_uuid(
    db: &PgPool,
    server_uuid: &str,
) -> Result<Option<ServerParseRulesRecord>, sqlx::Error> {
    sqlx::query_as::<_, ServerParseRulesRecord>(
        r#"
        SELECT
            server_uuid AS _server_uuid,
            rules_json,
            version
        FROM server_parse_rules
        WHERE server_uuid = $1
        "#,
    )
    .bind(server_uuid)
    .fetch_optional(db)
    .await
}

async fn upsert_server_parse_rules(
    db: &PgPool,
    server_uuid: &str,
    version: u64,
    rules: &[ParseRule],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO server_parse_rules (server_uuid, rules_json, version)
        VALUES ($1, $2, $3)
        ON CONFLICT (server_uuid) DO UPDATE
        SET rules_json = EXCLUDED.rules_json,
            version = EXCLUDED.version,
            updated_at = NOW()
        "#,
    )
    .bind(server_uuid)
    .bind(SqlxJson(rules.to_vec()))
    .bind(version as i64)
    .execute(db)
    .await?;

    Ok(())
}

async fn insert_server_parsed_events(
    db: &PgPool,
    server_uuid: &str,
    events: &[ParsedLogEvent],
) -> Result<(), sqlx::Error> {
    for event in events {
        let observed_at_ms = event.observed_at.parse::<i64>().unwrap_or_default();
        sqlx::query(
            r#"
            INSERT INTO server_parsed_events (
                server_uuid,
                agent_id,
                rule_id,
                event_type,
                severity,
                source,
                cursor,
                line_number,
                raw_line,
                observed_at_ms,
                payload_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(server_uuid)
        .bind(&event.agent_id)
        .bind(&event.rule_id)
        .bind(&event.event_type)
        .bind(&event.severity)
        .bind(&event.source)
        .bind(&event.cursor)
        .bind(event.line_number as i64)
        .bind(&event.raw_line)
        .bind(observed_at_ms)
        .bind(SqlxJson(event.payload.clone()))
        .execute(db)
        .await?;
    }

    Ok(())
}

async fn fetch_server_parsed_events(
    db: &PgPool,
    server_uuid: &str,
    event_type: Option<&str>,
    before: Option<u64>,
    limit: i64,
) -> Result<Vec<ParsedLogEvent>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ServerParsedEventRow>(
        r#"
        SELECT
            agent_id,
            rule_id,
            event_type,
            severity,
            source,
            cursor,
            line_number,
            raw_line,
            observed_at_ms,
            payload_json
        FROM server_parsed_events
        WHERE server_uuid = $1
          AND ($2::TEXT IS NULL OR event_type = $2)
          AND ($3::BIGINT IS NULL OR observed_at_ms < $3)
        ORDER BY observed_at_ms DESC, id DESC
        LIMIT $4
        "#,
    )
    .bind(server_uuid)
    .bind(event_type)
    .bind(before.map(|value| value as i64))
    .bind(limit)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ParsedLogEvent {
            agent_id: row.agent_id,
            rule_id: row.rule_id,
            event_type: row.event_type,
            severity: row.severity,
            source: row.source,
            cursor: row.cursor,
            line_number: row.line_number as u64,
            raw_line: row.raw_line,
            observed_at: row.observed_at_ms.to_string(),
            payload: row.payload_json.0,
        })
        .collect())
}

async fn upsert_server_agent_auth(
    db: &PgPool,
    server_uuid: &str,
    key_hash: &str,
    key_preview: &str,
) -> Result<ServerAgentAuthRecord, sqlx::Error> {
    sqlx::query_as::<_, ServerAgentAuthRecord>(
        r#"
        INSERT INTO server_agent_auth (server_uuid, key_hash, key_preview)
        VALUES ($1, $2, $3)
        ON CONFLICT (server_uuid) DO UPDATE
        SET key_hash = EXCLUDED.key_hash,
            key_preview = EXCLUDED.key_preview,
            rotated_at = NOW()
        RETURNING
            server_uuid AS _server_uuid,
            key_hash,
            key_preview,
            CAST(EXTRACT(EPOCH FROM rotated_at) * 1000 AS BIGINT) AS rotated_at_ms
        "#,
    )
    .bind(server_uuid)
    .bind(key_hash)
    .bind(key_preview)
    .fetch_one(db)
    .await
}

pub(crate) async fn verify_agent_registration_auth(
    db: &PgPool,
    registration: &crate::models::AgentRegistration,
) -> Result<(), String> {
    let exists = server_exists(db, &registration.server_uuid)
        .await
        .map_err(|err| format!("failed to read server: {err}"))?;
    if !exists {
        return Err("server not found".to_string());
    }

    let Some(auth) = fetch_server_agent_auth_by_server_uuid(db, &registration.server_uuid)
        .await
        .map_err(|err| format!("failed to read agent auth key: {err}"))?
    else {
        return Err("agent auth key is not provisioned".to_string());
    };

    if hash_agent_auth_key(&registration.auth_key) != auth.key_hash {
        return Err("agent auth key is invalid".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ParseRule, ParseRuleKind};

    #[tokio::test]
    async fn initialize_database_does_not_create_server_agent_bindings_table() {
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

        assert_eq!(table_exists, None);
    }

    #[tokio::test]
    async fn initialize_database_removes_orphaned_server_agent_auth_rows_before_adding_fk() {
        let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
        from_path_override(&env_path).ok();

        let config = AppConfig::from_env();
        let db = PgPoolOptions::new()
            .max_connections(1)
            .connect(&config.database_url)
            .await
            .expect("failed to connect to PostgreSQL");

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
        .execute(&db)
        .await
        .expect("failed to create managed_servers");

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
        .execute(&db)
        .await
        .expect("failed to create server_agent_auth");

        sqlx::query(
            r#"
            ALTER TABLE server_agent_auth
            DROP CONSTRAINT IF EXISTS server_agent_auth_server_uuid_fkey
            "#,
        )
        .execute(&db)
        .await
        .expect("failed to drop server_agent_auth foreign key");

        let orphan_server_uuid = format!("orphan-{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO server_agent_auth (server_uuid, key_hash, key_preview)
            VALUES ($1, $2, $3)
            ON CONFLICT (server_uuid) DO UPDATE
            SET key_hash = EXCLUDED.key_hash,
                key_preview = EXCLUDED.key_preview,
                rotated_at = NOW()
            "#,
        )
        .bind(&orphan_server_uuid)
        .bind("orphan-hash")
        .bind("orph...uuid")
        .execute(&db)
        .await
        .expect("failed to insert orphan auth row");

        initialize_database(&db)
            .await
            .expect("failed to initialize PostgreSQL schema");

        let orphan_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM server_agent_auth
            WHERE server_uuid = $1
            "#,
        )
        .bind(&orphan_server_uuid)
        .fetch_one(&db)
        .await
        .expect("failed to query orphan auth row");

        assert_eq!(orphan_count, 0);
    }

    #[test]
    fn validate_parse_rules_rejects_invalid_regex() {
        let result = validate_parse_rules(&[ParseRule {
            id: "broken".to_string(),
            kind: ParseRuleKind::Regex,
            pattern: "(".to_string(),
            event_type: "chat".to_string(),
            severity: "info".to_string(),
        }]);

        assert!(result.is_err());
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

fn generate_agent_auth_key() -> String {
    format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn build_agent_auth_key_preview(key: &str) -> String {
    if key.len() <= 8 {
        return key.to_string();
    }

    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

fn hash_agent_auth_key(key: &str) -> String {
    let pepper = env::var("AGENT_AUTH_PEPPER")
        .unwrap_or_else(|_| "squad-dev-agent-auth-pepper".to_string());
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
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
