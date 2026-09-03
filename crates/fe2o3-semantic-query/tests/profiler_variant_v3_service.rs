use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_semantic_query::validate_agent_profiler_variant_response_line_v3;
use serde_json::Value;

const DISCOVER: &[u8] = br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-variant-request-v3","request_id":1,"expected_revision":0}
"#;

fn run(input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-service"))
        .arg("variant-v3-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn variant_v3_binary_discovers_authority_free_archive_replay() {
    let output = run(DISCOVER);
    assert!(output.status.success(), "{:?}", output.stderr);
    let lines = output
        .stdout
        .split_inclusive(|byte| *byte == b'\n')
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    validate_agent_profiler_variant_response_line_v3(lines[0]).unwrap();

    let value: Value = serde_json::from_slice(lines[0]).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(value["value"]["result"], "capabilities");
    assert_eq!(
        value["value"]["capabilities"]["external_provenance"],
        "not_authenticated_by_this_archive_or_service"
    );
    assert_eq!(value["value"]["capabilities"]["maximum_open_archives"], 2);
}

#[test]
fn variant_v3_binary_rejects_unknown_fields_with_authenticated_terminal_error() {
    let input = br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-variant-request-v3","request_id":1,"expected_revision":0,"unknown":true}
"#;
    let output = run(input);
    assert_eq!(output.status.code(), Some(1));
    let lines = output
        .stdout
        .split_inclusive(|byte| *byte == b'\n')
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    validate_agent_profiler_variant_response_line_v3(lines[0]).unwrap();

    let value: Value = serde_json::from_slice(lines[0]).unwrap();
    assert_eq!(value["status"], "error");
    assert_eq!(value["code"], "invalid_request");
    assert_eq!(value["terminal"], true);
}
