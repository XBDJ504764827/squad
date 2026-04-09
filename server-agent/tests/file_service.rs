use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use server_agent::{AgentError, FileService, FileServiceConfig, PathPolicy, WorkspaceRootConfig};

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("server-agent-{prefix}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn build_service(
    root_name: &str,
    root_path: PathBuf,
    max_file_size: u64,
    allowed_extensions: Option<Vec<String>>,
) -> FileService {
    let policy = PathPolicy::new(&[WorkspaceRootConfig {
        name: root_name.to_string(),
        local_root: root_path,
    }])
    .expect("path policy");
    FileService::new(
        policy,
        FileServiceConfig {
            max_file_size,
            allowed_extensions,
        },
    )
}

#[test]
fn lists_tree_recursively_for_current_behavior() {
    let tmp = make_temp_dir("file-service-list-tree");
    let workspace_root = tmp.join("workspace");
    let docs = workspace_root.join("docs");
    fs::create_dir_all(&docs).expect("mkdir docs");
    fs::write(workspace_root.join("main.rs"), "fn main() {}\n").expect("write main.rs");
    fs::write(docs.join("readme.md"), "# docs\n").expect("write readme");

    let service = build_service("workspace", workspace_root, 1024 * 1024, None);
    let entries = service.list_tree("/workspace").expect("list tree");

    let logical_paths: Vec<String> = entries.iter().map(|v| v.logical_path.clone()).collect();
    assert!(logical_paths.contains(&"/workspace/docs".to_string()));
    assert!(logical_paths.contains(&"/workspace/docs/readme.md".to_string()));
    assert!(logical_paths.contains(&"/workspace/main.rs".to_string()));
}

#[test]
fn reads_text_with_stable_version_when_file_unchanged() {
    let tmp = make_temp_dir("file-service-read-version");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    fs::write(workspace_root.join("note.txt"), "hello\nworld\n").expect("write");

    let service = build_service("workspace", workspace_root, 1024 * 1024, None);
    let first = service
        .read_text_file("/workspace/note.txt")
        .expect("first read");
    let second = service
        .read_text_file("/workspace/note.txt")
        .expect("second read");

    assert_eq!(first.content, "hello\nworld\n");
    assert_eq!(first.version, second.version);
}

#[test]
fn rejects_reading_file_beyond_max_size_limit() {
    let tmp = make_temp_dir("file-service-size-limit");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    fs::write(workspace_root.join("big.txt"), "123456").expect("write");

    let service = build_service("workspace", workspace_root, 4, None);
    let err = service
        .read_text_file("/workspace/big.txt")
        .expect_err("size limit should fail");
    assert!(matches!(err, AgentError::FileTooLarge { .. }));
}

#[test]
fn rejects_extension_not_in_allow_list() {
    let tmp = make_temp_dir("file-service-extension");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    fs::write(workspace_root.join("main.rs"), "fn main() {}\n").expect("write");

    let service = build_service(
        "workspace",
        workspace_root,
        1024 * 1024,
        Some(vec![".txt".to_string()]),
    );
    let err = service
        .read_text_file("/workspace/main.rs")
        .expect_err("extension should fail");
    assert!(matches!(err, AgentError::ExtensionNotAllowed { .. }));
}

#[test]
fn detects_conflict_when_expected_version_is_stale() {
    let tmp = make_temp_dir("file-service-version-conflict");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    let file_path = workspace_root.join("note.txt");
    fs::write(&file_path, "v1\n").expect("write v1");

    let service = build_service("workspace", workspace_root, 1024 * 1024, None);
    let snapshot = service
        .read_text_file("/workspace/note.txt")
        .expect("initial read");

    fs::write(&file_path, "v2\n").expect("write v2");

    let err = service
        .write_text_file("/workspace/note.txt", "v3\n", Some(&snapshot.version))
        .expect_err("stale version should fail");
    assert!(matches!(err, AgentError::VersionConflict { .. }));
}

#[test]
fn write_keeps_existing_crlf_newline_style() {
    let tmp = make_temp_dir("file-service-newline-style");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir");
    let file_path = workspace_root.join("note.txt");
    fs::write(&file_path, "a\r\nb\r\n").expect("write crlf");

    let service = build_service("workspace", workspace_root, 1024 * 1024, None);
    let _ = service
        .write_text_file("/workspace/note.txt", "x\ny\n", None)
        .expect("write");

    let raw = fs::read(&file_path).expect("read");
    assert_eq!(raw, b"x\r\ny\r\n");
}

#[cfg(unix)]
#[test]
fn rejects_reading_symlink_target_outside_whitelist_root() {
    use std::os::unix::fs::symlink;

    let tmp = make_temp_dir("file-service-read-symlink-escape");
    let workspace_root = tmp.join("workspace");
    let outside_root = tmp.join("outside");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    fs::create_dir_all(&outside_root).expect("mkdir outside");
    let outside_file = outside_root.join("secret.txt");
    fs::write(&outside_file, "secret\n").expect("write outside file");

    let link = workspace_root.join("link.txt");
    symlink(&outside_file, &link).expect("create symlink");

    let service = build_service("workspace", workspace_root, 1024 * 1024, None);
    let err = service
        .read_text_file("/workspace/link.txt")
        .expect_err("symlink escape should fail");
    assert!(matches!(err, AgentError::AccessDenied(_)));
}

#[cfg(unix)]
#[test]
fn rejects_writing_symlink_target_outside_whitelist_root() {
    use std::os::unix::fs::symlink;

    let tmp = make_temp_dir("file-service-write-symlink-escape");
    let workspace_root = tmp.join("workspace");
    let outside_root = tmp.join("outside");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    fs::create_dir_all(&outside_root).expect("mkdir outside");
    let outside_file = outside_root.join("secret.txt");
    fs::write(&outside_file, "secret\n").expect("write outside file");

    let link = workspace_root.join("link.txt");
    symlink(&outside_file, &link).expect("create symlink");

    let service = build_service("workspace", workspace_root, 1024 * 1024, None);
    let err = service
        .write_text_file("/workspace/link.txt", "overwrite\n", None)
        .expect_err("symlink escape should fail");
    assert!(matches!(err, AgentError::AccessDenied(_)));
}
