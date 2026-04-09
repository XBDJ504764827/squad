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
use futures::{SinkExt, StreamExt};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

fn make_lazy_db() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://squad:squad@127.0.0.1:5432/squad")
        .expect("lazy pool should be constructed")
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

async fn register_agent(
    url: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (mut socket, _) = connect_async(url)
        .await
        .expect("websocket upgrade should succeed");

    let registration = AgentClientMessage::Register(AgentRegistration {
        agent_id: "agent-1".to_string(),
        token: "test-token".to_string(),
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
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(make_lazy_db(), registry.clone());
    let http_app = build_app_with_registry(make_lazy_db(), registry);
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url).await;

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
                .uri("/api/agents/agent-1/files/tree?path=%2Fgame-root")
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
    server.abort();
}

#[tokio::test]
async fn http_file_read_route_bridges_to_agent_command() {
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(make_lazy_db(), registry.clone());
    let http_app = build_app_with_registry(make_lazy_db(), registry);
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url).await;

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
                .uri("/api/agents/agent-1/files/content?path=%2Fgame-root%2Fserver.cfg")
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
    server.abort();
}

#[tokio::test]
async fn http_file_write_route_bridges_to_agent_command() {
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(make_lazy_db(), registry.clone());
    let http_app = build_app_with_registry(make_lazy_db(), registry);
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url).await;

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
                .uri("/api/agents/agent-1/files/content")
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
    server.abort();
}

#[tokio::test]
async fn http_file_bridge_maps_agent_business_error_to_bad_request() {
    let registry = AgentRegistry::default();
    let ws_app = build_app_with_registry(make_lazy_db(), registry.clone());
    let http_app = build_app_with_registry(make_lazy_db(), registry);
    let (url, server) = spawn_ws_app(ws_app).await;
    let mut socket = register_agent(&url).await;

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
                .uri("/api/agents/agent-1/files/content?path=%2Fgame-root%2Fserver.cfg")
                .method("GET")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should return");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    agent.await.expect("agent task should finish");
    server.abort();
}
