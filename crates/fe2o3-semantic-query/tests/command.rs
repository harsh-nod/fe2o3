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
fn stdin_only_cli_emits_agent_native_capture_plan_and_diagnosis_status() {
    let input = encoded_trace(8);
    let plan = run(
        &["plan-next-capture", "--goal", "performance_hotspot"],
        &input,
    );
    assert!(plan.status.success());
    assert!(plan.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(json["response"], "plan_next_capture");
    assert_eq!(json["plan"]["goal"], "performance_hotspot");
    assert_eq!(json["plan"]["limitations"][0], "no_diagnosis_claim");
    assert_eq!(
        json["plan"]["steps"][0]["compute_unit_selection"],
        "unspecified_not_represented_by_trace_v1"
    );
    assert!(json["plan"]["steps"][0]["storage"].is_string());

    let diagnosis = run(
        &["diagnosis-status", "--goal", "correctness_mismatch"],
        &input,
    );
    assert!(diagnosis.status.success());
    let json: serde_json::Value = serde_json::from_slice(&diagnosis.stdout).unwrap();
    assert_eq!(json["response"], "diagnosis_status");
    assert_eq!(json["status"]["diagnosis_reached"], false);
    assert!(json["status"]["missing_facts"].is_array());
}

#[test]
fn capture_plan_goal_parser_rejects_unknown_duplicate_and_path_arguments() {
    for arguments in [
        vec!["plan-next-capture", "--goal", "latency"],
        vec![
            "plan-next-capture",
            "--goal",
            "memory_fault",
            "--goal",
            "barrier_divergence",
        ],
        vec!["plan-next-capture", "--goal", "memory_fault", "/dev/zero"],
    ] {
        let output = run(&arguments, &encoded_trace(8));
        assert!(!output.status.success(), "{arguments:?}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stderr).unwrap()["code"],
            "arguments"
        );
    }
}

#[test]
fn capture_plan_cli_is_byte_deterministic() {
    let input = encoded_trace(8);
    let arguments = ["plan-next-capture", "--goal", "barrier_divergence"];
    let first = run(&arguments, &input);
    let second = run(&arguments, &input);
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
