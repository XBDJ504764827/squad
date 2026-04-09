use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use server_agent::file_watcher::FileWatcher;
use server_agent::{
    runtime::RuntimeCommandHandler, AgentCommand, AgentCommandHandler, FileReadRequest,
    FileReadResult, FileService, FileServiceConfig, FileTreeEntry, FileTreeRequest, FileTreeResult,
    FileWriteRequest, FileWriteResult, LogSourceConfig, LogStartPosition, LogTailer, ParseRule,
    ParseRuleKind, PathPolicy, ReplaceParseRulesRequest, ReplaceParseRulesResult,
    WorkspaceRootConfig,
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

fn make_rule(id: &str, pattern: &str, event_type: &str, severity: &str) -> ParseRule {
    ParseRule {
        id: id.to_string(),
        kind: ParseRuleKind::Regex,
        pattern: pattern.to_string(),
        event_type: event_type.to_string(),
        severity: severity.to_string(),
    }
}

fn make_log_config(primary_path: &std::path::Path) -> LogSourceConfig {
    LogSourceConfig {
        primary_path: primary_path.to_path_buf(),
        glob: None,
        start_position: LogStartPosition::End,
    }
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

#[test]
fn replace_parse_rules_command_updates_runtime_parser() {
    let tmp = make_temp_dir("runtime-rules");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");

    let handler = RuntimeCommandHandler::with_parser(
        build_service("workspace", workspace_root),
        vec![make_rule(
            "chat",
            r"^\[CHAT\] (?P<player>[^:]+): (?P<message>.+)$",
            "chat",
            "info",
        )],
    )
    .expect("handler should build");

    let payload = handler
        .handle_command(AgentCommand::ReplaceParseRules(ReplaceParseRulesRequest {
            version: 2,
            rules: vec![make_rule("kill", r"^\[KILL\] (?P<killer>.+)$", "kill", "warn")],
        }))
        .expect("command should succeed")
        .expect("payload should exist");
    let result: ReplaceParseRulesResult = serde_json::from_value(payload).expect("payload json");

    assert_eq!(result.version, 2);
    assert_eq!(result.rule_count, 1);
}

#[test]
fn invalid_replace_parse_rules_keeps_previous_rules_active() {
    let tmp = make_temp_dir("runtime-rules-invalid");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");

    let handler = RuntimeCommandHandler::with_parser(
        build_service("workspace", workspace_root),
        vec![make_rule(
            "chat",
            r"^\[CHAT\] (?P<player>[^:]+): (?P<message>.+)$",
            "chat",
            "info",
        )],
    )
    .expect("handler should build");

    let result = handler.handle_command(AgentCommand::ReplaceParseRules(ReplaceParseRulesRequest {
        version: 2,
        rules: vec![make_rule("broken", "(", "chat", "info")],
    }));

    assert!(result.is_err());
}

#[test]
fn drain_log_entries_collects_and_clears_parsed_events() {
    let tmp = make_temp_dir("runtime-parsed-events");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    let log_path = workspace_root.join("server.log");
    fs::write(&log_path, "").expect("write empty log");

    let roots = vec![WorkspaceRootConfig {
        name: "workspace".to_string(),
        local_root: workspace_root.clone(),
    }];
    let path_policy = PathPolicy::new(&roots).expect("path policy");
    let file_service = FileService::new(
        path_policy.clone(),
        FileServiceConfig {
            max_file_size: 1024 * 1024,
            allowed_extensions: Some(vec![".txt".to_string(), ".cfg".to_string()]),
        },
    );
    let log_tailer =
        LogTailer::new("agent-1", "server", make_log_config(&log_path)).expect("create tailer");
    let file_watcher = FileWatcher::new(path_policy, &roots).expect("create file watcher");
    let handler = RuntimeCommandHandler::with_streaming(
        file_service,
        vec![make_rule(
            "chat",
            r"^\[CHAT\] (?P<player>[^:]+): (?P<message>.+)$",
            "chat",
            "info",
        )],
        log_tailer,
        file_watcher,
    )
    .expect("handler should build");

    assert!(handler
        .drain_log_entries()
        .expect("initial drain should succeed")
        .is_empty());

    fs::write(&log_path, "[CHAT] RiverFox: hello squad\n").expect("append log line");

    let log_entries = handler
        .drain_log_entries()
        .expect("drain log entries should succeed");
    assert_eq!(log_entries.len(), 1);
    assert_eq!(log_entries[0].raw_line, "[CHAT] RiverFox: hello squad");

    let parsed_events = handler
        .drain_parsed_events()
        .expect("drain parsed events should succeed");
    assert_eq!(parsed_events.len(), 1);
    assert_eq!(parsed_events[0].event_type, "chat");
    assert_eq!(parsed_events[0].payload.get("player"), Some(&"RiverFox".to_string()));
    assert_eq!(
        parsed_events[0].payload.get("message"),
        Some(&"hello squad".to_string())
    );

    assert!(handler
        .drain_parsed_events()
        .expect("second drain should succeed")
        .is_empty());
}
