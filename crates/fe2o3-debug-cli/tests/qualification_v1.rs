use std::process::Command;

use serde_json::Value;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fe2o3-debug-protocol/tests/fixtures/mi300x-qualification-v1.json"
);

#[test]
fn checked_in_manifest_has_one_agent_readable_incomplete_assessment() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["qualification", "--manifest", FIXTURE])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "qualification failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let assessment: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        assessment["schema"],
        "fe2o3-debug-qualification-assessment-v1"
    );
    assert_eq!(assessment["disposition"], "incomplete");
    assert_eq!(assessment["observation_authority"], false);
    assert_eq!(assessment["qualification_authority"], false);
    assert_eq!(
        assessment["manifest"]["components"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        assessment["overhead_assessments"].as_array().unwrap().len(),
        6
    );
    assert_eq!(
        assessment["manifest"]["components"][1]["capabilities"]["live_gpu_state"]["limitations"],
        "KFD declaration/publication were admitted, but ROCgdb 16.3 returned gpu_stopped_state_unavailable; raw MI reported a global#0 read failure with the KFD-only r_debug=0 runtime."
    );
}

#[test]
fn malformed_and_relative_inputs_fail_closed_without_stdout() {
    let relative = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["qualification", "--manifest", "relative.json"])
        .output()
        .unwrap();
    assert!(!relative.status.success());
    assert!(relative.stdout.is_empty());
    let error: Value = serde_json::from_slice(&relative.stderr).unwrap();
    assert_eq!(error["schema"], "fe2o3-debug-bootstrap-error-v1");
    assert_eq!(error["stage"], "arguments");
    assert_eq!(error["code"], "invalid_qualification_command_line");

    let malformed = std::env::temp_dir().join(format!(
        "fe2o3-qualification-malformed-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    std::fs::write(&malformed, b"{}\n").unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .arg("qualification")
        .arg("--manifest")
        .arg(&malformed)
        .output()
        .unwrap();
    std::fs::remove_file(malformed).unwrap();
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let error: Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(error["stage"], "input");
    assert_eq!(error["code"], "qualification_manifest_rejected");
}
