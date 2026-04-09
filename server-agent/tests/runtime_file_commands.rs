use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use server_agent::{
    runtime::RuntimeCommandHandler, AgentCommand, AgentCommandHandler, FileReadRequest,
    FileReadResult, FileService, FileServiceConfig, FileTreeEntry, FileTreeRequest, FileTreeResult,
    FileWriteRequest, FileWriteResult, PathPolicy, WorkspaceRootConfig,
};

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("server-agent-{prefix}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn build_service(root_name: &str, root_path: PathBuf) -> FileService {
    let policy = PathPolicy::new(&[WorkspaceRootConfig {
        name: root_name.to_string(),
        local_root: root_path,
    }])
    .expect("path policy");
    FileService::new(
        policy,
        FileServiceConfig {
            max_file_size: 1024 * 1024,
            allowed_extensions: Some(vec![".txt".to_string(), ".cfg".to_string()]),
        },
    )
}

#[test]
fn file_tree_command_returns_entries_from_file_service() {
    let tmp = make_temp_dir("runtime-handler-tree");
    let workspace_root = tmp.join("workspace");
    let docs = workspace_root.join("docs");
    fs::create_dir_all(&docs).expect("mkdir docs");
    fs::write(docs.join("readme.txt"), "hello\n").expect("write readme");

    let handler = RuntimeCommandHandler::new(build_service("workspace", workspace_root));
    let payload = handler
        .handle_command(AgentCommand::FileTree(FileTreeRequest {
            logical_path: "/workspace".to_string(),
        }))
        .expect("command should succeed")
        .expect("payload should exist");
    let result: FileTreeResult = serde_json::from_value(payload).expect("payload json");

    assert_eq!(
        result.entries,
        vec![
            FileTreeEntry {
                logical_path: "/workspace/docs".to_string(),
                is_dir: true,
                size: None,
            },
            FileTreeEntry {
                logical_path: "/workspace/docs/readme.txt".to_string(),
                is_dir: false,
                size: Some(6),
            },
        ]
    );
}

#[test]
fn file_read_command_returns_snapshot_from_file_service() {
    let tmp = make_temp_dir("runtime-handler-read");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    fs::write(workspace_root.join("server.cfg"), "hostname=test\n").expect("write");

    let handler = RuntimeCommandHandler::new(build_service("workspace", workspace_root));
    let payload = handler
        .handle_command(AgentCommand::FileRead(FileReadRequest {
            logical_path: "/workspace/server.cfg".to_string(),
        }))
        .expect("command should succeed")
        .expect("payload should exist");
    let result: FileReadResult = serde_json::from_value(payload).expect("payload json");

    assert_eq!(result.logical_path, "/workspace/server.cfg");
    assert_eq!(result.content, "hostname=test\n");
    assert!(!result.version.is_empty());
}

#[test]
fn file_write_command_updates_file_and_returns_new_version() {
    let tmp = make_temp_dir("runtime-handler-write");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    let file_path = workspace_root.join("server.cfg");
    fs::write(&file_path, "hostname=old\n").expect("write");

    let service = build_service("workspace", workspace_root.clone());
    let snapshot = service
        .read_text_file("/workspace/server.cfg")
        .expect("initial read");

    let handler = RuntimeCommandHandler::new(service);
    let payload = handler
        .handle_command(AgentCommand::FileWrite(FileWriteRequest {
            logical_path: "/workspace/server.cfg".to_string(),
            content: "hostname=new\n".to_string(),
            expected_version: Some(snapshot.version),
        }))
        .expect("command should succeed")
        .expect("payload should exist");
    let result: FileWriteResult = serde_json::from_value(payload).expect("payload json");

    assert_eq!(result.logical_path, "/workspace/server.cfg");
    assert!(!result.version.is_empty());
    assert_eq!(
        fs::read_to_string(&file_path).expect("read file"),
        "hostname=new\n"
    );
}
