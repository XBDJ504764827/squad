use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

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
    let db = PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed");
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
