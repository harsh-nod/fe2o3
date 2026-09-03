use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_semantic_query::{
    AGENT_PC_SOURCE_ISA_REQUEST_SCHEMA_V1, AGENT_PC_SOURCE_ISA_RESPONSE_SCHEMA_V1,
    AgentPcSourceIsaRequestV1,
};

#[test]
fn profiler_service_exposes_the_pc_source_isa_jsonl_route() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-service"))
        .arg("pc-source-isa-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = AgentPcSourceIsaRequestV1::DiscoverCapabilities {
        schema: AGENT_PC_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 1,
        expected_revision: 0,
    };
    let mut encoded = serde_json::to_vec(&request).unwrap();
    encoded.push(b'\n');
    child.stdin.take().unwrap().write_all(&encoded).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "capabilities");
    assert_eq!(response["schema"], AGENT_PC_SOURCE_ISA_RESPONSE_SCHEMA_V1);
    assert_eq!(response["operations"][3], "lookup_sample");
    assert_eq!(response["max_page_items"], 64);
}

#[test]
fn profiler_service_rejects_noncanonical_requests() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-service"))
        .arg("pc-source-isa-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{ \"operation\": \"discover_capabilities\" }\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "error");
    assert_eq!(response["code"], "invalid_request");
    assert_eq!(response["terminal"], false);
}
