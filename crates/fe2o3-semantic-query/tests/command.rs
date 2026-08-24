mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::encoded_trace;

fn run(arguments: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-trace-query"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_only_cli_emits_bounded_json_summary() {
    let output = run(&["summary"], &encoded_trace(8));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["response"], "dispatch_summary");
    assert_eq!(json["context"]["event_count"], 13);
}

#[test]
fn path_like_argument_is_rejected_without_opening_it() {
    let output = run(&["capabilities", "/dev/zero"], b"");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(json["code"], "arguments");
}

#[test]
fn malformed_stdin_and_hostile_cursor_fail_closed() {
    let malformed = run(&["summary"], b"not-a-trace");
    assert!(!malformed.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&malformed.stderr).unwrap()["code"],
        "trace_open"
    );

    let cursor = run(
        &["lanes", "--cursor", "../../etc/passwd"],
        &encoded_trace(8),
    );
    assert!(!cursor.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&cursor.stderr).unwrap()["code"],
        "cursor"
    );
}

#[test]
fn repeated_cli_query_is_byte_deterministic() {
    let input = encoded_trace(8);
    let first = run(&["sites", "--limit", "3"], &input);
    let second = run(&["sites", "--limit", "3"], &input);
    assert!(first.status.success() && second.status.success());
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn duplicate_flags_and_noncanonical_cursor_hex_are_rejected() {
    let duplicate = run(
        &["lanes", "--limit", "1", "--limit", "2"],
        &encoded_trace(8),
    );
    assert!(!duplicate.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&duplicate.stderr).unwrap()["code"],
        "arguments"
    );

    let uppercase = format!("{}:0", "AB".repeat(32));
    let cursor = run(&["lanes", "--cursor", &uppercase], &encoded_trace(8));
    assert!(!cursor.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&cursor.stderr).unwrap()["code"],
        "cursor"
    );
}
