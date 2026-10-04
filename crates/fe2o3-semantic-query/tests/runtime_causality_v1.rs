use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn profiler_service_routes_runtime_causality_jsonl() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-service"))
        .arg("runtime-causality-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-runtime-causality-request-v1\",\"request_id\":1,\"revision\":0}\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], "ok");
    assert_eq!(
        response["schema"],
        "fe2o3-agent-runtime-causality-response-v1"
    );
    assert_eq!(response["result"]["result"], "capabilities");
    assert_eq!(
        response["result"]["dependency_events"]["state"],
        "unavailable"
    );
}
