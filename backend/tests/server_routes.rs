use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::models::{
    AgentPlatform, AgentRegistration, ManagedServer, ManagedServerDetailResponse, OnlineAgent,
    WorkspaceRootSummary,
};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

fn make_lazy_db() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed")
}

#[tokio::test]
async fn server_detail_route_exists() {
    let db = PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed");
    let app = backend::build_app(db);

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
    let db = PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed");
    let app = backend::build_app(db);

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
    let app = backend::build_app(db);

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
        agent_id: server_uuid.clone(),
        token: "test-token".to_string(),
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
        Some(&OnlineAgent {
            session_id: "session-1".to_string(),
            connected_at_ms: 1,
            last_heartbeat_at_ms: 2,
            registration,
        }),
    );

    assert_eq!(detail.agent_id, server_uuid);
    assert_eq!(detail.workspace_roots.len(), 1);
    assert_eq!(detail.workspace_roots[0].logical_path, "/game-root");
    assert_eq!(detail.primary_log_path, "/srv/game/server.log");
}
