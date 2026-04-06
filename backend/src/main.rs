mod models;

use std::{env, net::SocketAddr};

use axum::{Json, Router, routing::get};
use models::{DashboardResponse, HealthResponse};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/dashboard", get(dashboard))
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

async fn dashboard() -> Json<DashboardResponse> {
    Json(DashboardResponse::empty())
}
