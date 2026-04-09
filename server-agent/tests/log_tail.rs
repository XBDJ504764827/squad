use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use server_agent::{LogSourceConfig, LogStartPosition, LogTailer};

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("server-agent-{prefix}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn make_log_config(primary_path: &Path, start_position: LogStartPosition) -> LogSourceConfig {
    LogSourceConfig {
        primary_path: primary_path.to_path_buf(),
        glob: None,
        start_position,
    }
}

#[test]
fn starts_from_end_without_replaying_existing_lines() {
    let tmp = make_temp_dir("log-tail-start-end");
    let log_path = tmp.join("server.log");
    fs::write(&log_path, "old-1\nold-2\n").expect("write old lines");

    let mut tailer = LogTailer::new(
        "agent-1",
        "server",
        make_log_config(&log_path, LogStartPosition::End),
    )
    .expect("create tailer");

    assert!(tailer.poll().expect("first poll").is_empty());

    fs::write(&log_path, "old-1\nold-2\nnew-1\n").expect("append new line");

    let lines = tailer.poll().expect("second poll");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].raw_line, "new-1");
    assert_eq!(lines[0].line_number, 1);
}

#[test]
fn reads_multiple_appended_lines_in_stable_order() {
    let tmp = make_temp_dir("log-tail-order");
    let log_path = tmp.join("server.log");
    fs::write(&log_path, "").expect("write empty file");

    let mut tailer = LogTailer::new(
        "agent-1",
        "server",
        make_log_config(&log_path, LogStartPosition::End),
    )
    .expect("create tailer");

    assert!(tailer.poll().expect("initial poll").is_empty());

    fs::write(&log_path, "alpha\nbeta\ngamma\n").expect("append lines");

    let lines = tailer.poll().expect("read appended lines");
    let raw_lines: Vec<&str> = lines.iter().map(|line| line.raw_line.as_str()).collect();
    let line_numbers: Vec<u64> = lines.iter().map(|line| line.line_number).collect();

    assert_eq!(raw_lines, vec!["alpha", "beta", "gamma"]);
    assert_eq!(line_numbers, vec![1, 2, 3]);
}

#[test]
fn continues_after_rotation_and_switches_to_new_primary_file() {
    let tmp = make_temp_dir("log-tail-rotation");
    let log_path = tmp.join("server.log");
    let rotated_path = tmp.join("server.log.1");
    fs::write(&log_path, "old\n").expect("write initial line");

    let mut tailer = LogTailer::new(
        "agent-1",
        "server",
        make_log_config(&log_path, LogStartPosition::End),
    )
    .expect("create tailer");

    assert!(tailer.poll().expect("initial poll").is_empty());

    fs::write(&log_path, "old\nlive-1\n").expect("append live line");
    let first_batch = tailer.poll().expect("read first batch");
    assert_eq!(first_batch.len(), 1);
    assert_eq!(first_batch[0].raw_line, "live-1");

    fs::rename(&log_path, &rotated_path).expect("rotate file");
    fs::write(&rotated_path, "old\nlive-1\nrotated-tail\n").expect("write rotated tail");
    fs::write(&log_path, "new-live-1\n").expect("create new primary");

    let second_batch = tailer.poll().expect("read after rotation");
    let second_lines: Vec<&str> = second_batch
        .iter()
        .map(|entry| entry.raw_line.as_str())
        .collect();
    assert_eq!(second_lines, vec!["rotated-tail", "new-live-1"]);

    fs::write(&log_path, "new-live-1\nnew-live-2\n").expect("append second primary line");

    let third_batch = tailer.poll().expect("read new primary");
    let third_lines: Vec<&str> = third_batch
        .iter()
        .map(|entry| entry.raw_line.as_str())
        .collect();
    assert_eq!(third_lines, vec!["new-live-2"]);
}
