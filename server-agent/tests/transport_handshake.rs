use futures::{SinkExt, StreamExt};
use server_agent::{
    AgentClientMessage, AgentPlatform, AgentRegistered, AgentRegistration, Transport,
    WorkspaceRootSummary,
};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

fn make_registration() -> AgentRegistration {
    AgentRegistration {
        agent_id: "agent-1".to_string(),
        token: "test-token".to_string(),
        platform: AgentPlatform::Linux,
        version: "0.1.0".to_string(),
        workspace_roots: vec![WorkspaceRootSummary {
            name: "game-root".to_string(),
            logical_path: "/game-root".to_string(),
        }],
        primary_log_path: "/srv/game/server.log".to_string(),
    }
}

#[tokio::test]
async fn connect_sends_registration_and_waits_for_registered_ack() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut socket = accept_async(stream).await.expect("upgrade");

        let frame = socket
            .next()
            .await
            .expect("register frame")
            .expect("register frame readable");
        let register = serde_json::from_str::<AgentClientMessage>(
            frame.into_text().expect("register text").as_ref(),
        )
        .expect("register json");

        match register {
            AgentClientMessage::Register(payload) => {
                assert_eq!(payload.agent_id, "agent-1");
                assert_eq!(payload.token, "test-token");
                assert_eq!(payload.workspace_roots[0].logical_path, "/game-root");
            }
        }

        let ack = AgentRegistered {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
        };
        socket
            .send(Message::Text(
                serde_json::to_string(&server_agent::AgentServerMessage::Registered(ack))
                    .expect("ack json")
                    .into(),
            ))
            .await
            .expect("ack should send");
    });

    let transport = Transport::new(format!("ws://{address}"));
    let registered = transport
        .connect(make_registration())
        .await
        .expect("handshake should succeed");

    assert_eq!(registered.agent_id, "agent-1");
    assert_eq!(registered.session_id, "session-1");

    server.await.expect("server should finish");
}
