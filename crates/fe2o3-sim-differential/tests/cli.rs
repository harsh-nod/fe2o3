use std::process::Command;

use serde_json::Value;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fe2o3-sim-differential"))
}

#[test]
fn command_emits_bounded_machine_readable_agreement() {
    let output = command()
        .args([
            "--seed-start",
            "7",
            "--cases",
            "4",
            "--inputs",
            "1",
            "--work-items",
            "4",
            "--max-nodes",
            "7",
            "--max-depth",
            "4",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() < 1_024);

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "fe2o3-sim-scalar-differential-v1");
    assert_eq!(report["status"], "agreement");
    assert_eq!(report["evidence_origin"], "differential_model_agreement");
    assert_eq!(report["authority"], "none");
    assert_eq!(report["cases"], 4);
    assert_eq!(report["lanes_compared"], 16);
    assert_eq!(report["kir_version"], 7);
    assert_eq!(report["simulation_target"], "amdgpu64-target-neutral");
    assert_eq!(report["workgroup_size"], serde_json::json!([64, 1, 1]));
    assert_eq!(report["hardware_observed"], false);
    assert_eq!(report["performance_prediction"], false);
    assert!(report["suite_sha256"].as_str().unwrap().starts_with("0x"));
}

#[test]
fn command_rejects_an_out_of_bounds_case_count_as_typed_json() {
    let output = command().args(["--cases", "0"]).output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.len() < 1_024);

    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(
        report["schema"],
        "fe2o3-sim-scalar-differential-command-error-v1"
    );
    assert_eq!(report["status"], "error");
    assert_eq!(report["code"], "invalid_command_line");
    assert_eq!(report["evidence_origin"], "command_validation");
    assert_eq!(report["authority"], "none");
    assert_eq!(report["hardware_observed"], false);
    assert_eq!(report["performance_prediction"], false);
}
