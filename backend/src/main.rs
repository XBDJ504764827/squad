mod models;
mod rcon;

use std::{env, net::SocketAddr};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use models::{
    ActionResponse, AddServerRequest, DashboardResponse, ErrorResponse, HealthResponse,
    ManagedServer,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[tokio::main]
async fn main() {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://squad:squad@192.168.0.62:5432/squad".to_string());

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to PostgreSQL");

    initialize_database(&db)
        .await
        .expect("failed to initialize PostgreSQL schema");

    let state = AppState { db };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/dashboard", get(dashboard))
        .route("/api/servers", post(add_server))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);

    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind TCP listener");

    println!("Rust backend listening on http://{}", address);

    axum::serve(listener, app)
        .await
        .expect("backend server exited unexpectedly");
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

async fn add_server(
    State(state): State<AppState>,
    Json(payload): Json<AddServerRequest>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let name = payload.name.trim().to_string();
    let ip = payload.ip.trim().to_string();
    let password = payload.rcon_password.trim().to_string();

    if name.is_empty() || ip.is_empty() || password.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "服务器名称、IP、RCON 端口和 RCON 密码不能为空",
        ));
    }

    let duplicate_exists = server_exists(&state.db, &name, &ip, payload.rcon_port)
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

    insert_server(&state.db, &name, &ip, payload.rcon_port, &password)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器写入数据库失败"))?;

    Ok(Json(ActionResponse {
        message: "服务器添加成功，RCON 验证已通过".to_string(),
    }))
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
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE (ip, rcon_port)
        )
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn fetch_servers(db: &PgPool) -> Result<Vec<ManagedServer>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ManagedServer>(
        r#"
        SELECT name, ip, rcon_port, rcon_password
        FROM managed_servers
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

async fn server_exists(
    db: &PgPool,
    name: &str,
    ip: &str,
    rcon_port: u16,
) -> Result<bool, sqlx::Error> {
    let existing = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM managed_servers
        WHERE name = $1 OR (ip = $2 AND rcon_port = $3)
        "#,
    )
    .bind(name)
    .bind(ip)
    .bind(i32::from(rcon_port))
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
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO managed_servers (name, ip, rcon_port, rcon_password)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(name)
    .bind(ip)
    .bind(i32::from(rcon_port))
    .bind(rcon_password)
    .execute(db)
    .await?;

    Ok(())
}

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            message: message.to_string(),
        }),
    )
}
