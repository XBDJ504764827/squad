use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use server_agent::{AgentError, PathPolicy, WorkspaceRootConfig};

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("server-agent-{prefix}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn build_policy(root_name: &str, root_path: PathBuf) -> PathPolicy {
    PathPolicy::new(&[WorkspaceRootConfig {
        name: root_name.to_string(),
        local_root: root_path,
    }])
    .expect("path policy should be created")
}

#[test]
fn normalizes_windows_style_logical_path() {
    let normalized =
        PathPolicy::normalize_logical_path(r"\workspace\docs\readme.txt").expect("normalize");
    assert_eq!(normalized, "/workspace/docs/readme.txt");
}

#[test]
fn maps_logical_path_to_local_path_under_whitelist_root() {
    let tmp = make_temp_dir("path-policy-map");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    let policy = build_policy("workspace", workspace_root.clone());

    let local = policy
        .logical_to_local("/workspace/src/main.rs")
        .expect("mapping should succeed");

    assert_eq!(local, workspace_root.join("src").join("main.rs"));
}

#[test]
fn rejects_parent_traversal_segments() {
    let err = PathPolicy::normalize_logical_path("/workspace/../secret.txt").expect_err("error");
    assert!(matches!(err, AgentError::AccessDenied(_)));
}

#[test]
fn rejects_unknown_root_access() {
    let tmp = make_temp_dir("path-policy-unknown-root");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    let policy = build_policy("workspace", workspace_root);

    let err = policy
        .logical_to_local("/other/readme.md")
        .expect_err("unknown root should fail");
    assert!(matches!(err, AgentError::UnknownRoot(_)));
}

#[test]
fn converts_local_path_back_to_logical_path_and_blocks_outside_path() {
    let tmp = make_temp_dir("path-policy-local-to-logical");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    let policy = build_policy("workspace", workspace_root.clone());

    let inside = workspace_root.join("src").join("lib.rs");
    let logical = policy.local_to_logical(&inside).expect("inside root");
    assert_eq!(logical, "/workspace/src/lib.rs");

    let outside = tmp.join("other").join("lib.rs");
    let err = policy
        .local_to_logical(&outside)
        .expect_err("outside root should fail");
    assert!(matches!(err, AgentError::AccessDenied(_)));
}

#[test]
fn rejects_relative_root_path_in_config() {
    let err = PathPolicy::new(&[WorkspaceRootConfig {
        name: "workspace".to_string(),
        local_root: PathBuf::from("relative/workspace"),
    }])
    .expect_err("relative root should fail");
    assert!(matches!(err, AgentError::InvalidConfig(_)));
}

#[test]
fn canonicalizes_root_path_to_avoid_alias_boundary_mismatch() {
    let tmp = make_temp_dir("path-policy-canonical-root");
    let nested = tmp.join("nested");
    let workspace_root = nested.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    fs::create_dir_all(nested.join("subdir")).expect("mkdir subdir");
    let alias_root = nested.join("subdir").join("..").join("workspace");
    let policy = build_policy("workspace", alias_root);

    let local = policy
        .logical_to_local("/workspace/readme.md")
        .expect("mapping should succeed");
    assert_eq!(local, workspace_root.join("readme.md"));
}

#[test]
fn rejects_inaccessible_root_name_rules() {
    let tmp = make_temp_dir("path-policy-invalid-root-name");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");

    let err = PathPolicy::new(&[WorkspaceRootConfig {
        name: "..".to_string(),
        local_root: workspace_root,
    }])
    .expect_err("invalid root name should fail");
    assert!(matches!(err, AgentError::InvalidConfig(_)));
}

#[cfg(unix)]
#[test]
fn local_to_logical_rejects_non_utf8_component() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let tmp = make_temp_dir("path-policy-non-utf8");
    let workspace_root = tmp.join("workspace");
    fs::create_dir_all(&workspace_root).expect("mkdir workspace");
    let policy = build_policy("workspace", workspace_root.clone());

    let bad_component = OsStr::from_bytes(b"\xFFbad");
    let bad_path = workspace_root.join(bad_component);

    let err = policy
        .local_to_logical(&bad_path)
        .expect_err("non utf-8 component should fail");
    assert!(matches!(err, AgentError::PathEncoding(_)));
}
