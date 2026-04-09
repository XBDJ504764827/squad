use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use server_agent::{
    AgentClientMessage, AgentCommand, AgentCommandHandler, AgentCommandResult, AgentHeartbeat,
    AgentPlatform, AgentRegistered, AgentRegistration, AgentServerMessage, Transport,
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
            AgentClientMessage::Heartbeat(_) | AgentClientMessage::CommandResult(_) => {
                panic!("unexpected message before registration ack")
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
    let connection = transport
        .connect(make_registration())
        .await
        .expect("handshake should succeed");

    assert_eq!(connection.registered().agent_id, "agent-1");
    assert_eq!(connection.registered().session_id, "session-1");

    server.await.expect("server should finish");
}

#[tokio::test]
async fn connection_can_send_heartbeat_after_registration() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut socket = accept_async(stream).await.expect("upgrade");

        let _ = socket
            .next()
            .await
            .expect("register frame")
            .expect("register frame readable");

        let ack = AgentRegistered {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
        };
        socket
            .send(Message::Text(
                serde_json::to_string(&AgentServerMessage::Registered(ack))
                    .expect("ack json")
                    .into(),
            ))
            .await
            .expect("ack should send");

        let heartbeat = socket
            .next()
            .await
            .expect("heartbeat frame")
            .expect("heartbeat readable");
        let message = serde_json::from_str::<AgentClientMessage>(
            heartbeat.into_text().expect("heartbeat text").as_ref(),
        )
        .expect("heartbeat json");

        assert!(matches!(
            message,
            AgentClientMessage::Heartbeat(AgentHeartbeat {})
        ));
    });

    let transport = Transport::new(format!("ws://{address}"));
    let mut connection = transport
        .connect(make_registration())
        .await
        .expect("handshake should succeed");
    connection
        .send_heartbeat()
        .await
        .expect("heartbeat should send");

    server.await.expect("server should finish");
}

#[tokio::test]
async fn connection_receives_command_and_sends_command_result() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut socket = accept_async(stream).await.expect("upgrade");

        let _ = socket
            .next()
            .await
            .expect("register frame")
            .expect("register frame readable");

        let ack = AgentRegistered {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
        };
        socket
            .send(Message::Text(
                serde_json::to_string(&AgentServerMessage::Registered(ack))
                    .expect("ack json")
                    .into(),
            ))
            .await
            .expect("ack should send");

        socket
            .send(Message::Text(
                serde_json::to_string(&AgentServerMessage::Command(
                    server_agent::AgentCommandEnvelope {
                        request_id: "req-1".to_string(),
                        command: AgentCommand::Ping,
                    },
                ))
                .expect("command json")
                .into(),
            ))
            .await
            .expect("command should send");

        let response = socket
            .next()
            .await
            .expect("response frame")
            .expect("response readable");
        let message = serde_json::from_str::<AgentClientMessage>(
            response.into_text().expect("response text").as_ref(),
        )
        .expect("response json");

        match message {
            AgentClientMessage::CommandResult(payload) => {
                assert_eq!(payload.request_id, "req-1");
                assert_eq!(payload.payload, Some(json!({ "pong": true })));
            }
            other => panic!("unexpected client message: {other:?}"),
        }
    });

    let transport = Transport::new(format!("ws://{address}"));
    let mut connection = transport
        .connect(make_registration())
        .await
        .expect("handshake should succeed");
    let command = connection
        .next_server_message()
        .await
        .expect("reading command should succeed")
        .expect("command should exist");

    let request_id = match command {
        AgentServerMessage::Command(payload) => {
            assert_eq!(payload.command, AgentCommand::Ping);
            payload.request_id
        }
        other => panic!("unexpected server message: {other:?}"),
    };

    connection
        .send_command_result(AgentCommandResult {
            request_id,
            success: true,
            payload: Some(json!({ "pong": true })),
            error: None,
        })
        .await
        .expect("response should send");

    server.await.expect("server should finish");
}

#[derive(Default)]
struct TestCommandHandler;

impl AgentCommandHandler for TestCommandHandler {
    fn handle_command(
        &self,
        command: AgentCommand,
    ) -> Result<Option<Value>, server_agent::AgentError> {
        match command {
            AgentCommand::Ping => Ok(Some(json!({ "pong": true }))),
        }
    }
}

#[tokio::test]
async fn run_reconnects_after_disconnect_and_registers_again() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
        for expected_session in ["session-1", "session-2"] {
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
            assert!(matches!(register, AgentClientMessage::Register(_)));

            socket
                .send(Message::Text(
                    serde_json::to_string(&AgentServerMessage::Registered(AgentRegistered {
                        agent_id: "agent-1".to_string(),
                        session_id: expected_session.to_string(),
                    }))
                    .expect("ack json")
                    .into(),
                ))
                .await
                .expect("ack should send");

            let _ = socket.close(None).await;
        }
    });

    let transport = Transport::new(format!("ws://{address}"));
    let task = tokio::spawn(async move {
        transport
            .run(make_registration(), &TestCommandHandler::default())
            .await
            .expect("transport loop should keep reconnecting");
    });

    tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .expect("server should observe two registrations")
        .expect("server task should finish");

    task.abort();
}
