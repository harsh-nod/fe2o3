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

#[test]
fn semantic_commands_emit_bounded_agent_readable_capabilities_and_evidence() {
    let capabilities = command().arg("semantic-capabilities-v2").output().unwrap();
    assert!(capabilities.status.success());
    assert!(capabilities.stderr.is_empty());
    assert!(capabilities.stdout.len() < 16 * 1024);
    let capabilities: Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    assert_eq!(
        capabilities["schema"],
        "fe2o3-sim-semantic-differential-capabilities-v2"
    );
    assert_eq!(capabilities["authority"], "none");
    assert_eq!(capabilities["case_limit"], 23);
    assert_eq!(capabilities["hardware_observed"], false);
    assert_eq!(capabilities["performance_prediction"], false);

    let output = command()
        .args(["semantic-run-v2", "--seed", "19"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() < 128 * 1024);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "fe2o3-sim-semantic-differential-v2");
    assert_eq!(report["status"], "agreement");
    assert_eq!(report["authority"], "none");
    assert_eq!(report["agreement_cases"], 19);
    assert_eq!(report["expected_rejections"], 4);
    assert_eq!(report["lanes_compared"], 152);
    assert_eq!(report["cases"].as_array().unwrap().len(), 23);
    assert_eq!(report["hardware_observed"], false);
    assert_eq!(report["performance_prediction"], false);
    assert_eq!(report["capability_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(report["suite_sha256"].as_str().unwrap().len(), 66);

    let case = &report["cases"][0];
    let replay = command()
        .args([
            "semantic-replay-v2",
            "--seed",
            "19",
            "--case",
            case["case_id"].as_str().unwrap(),
            "--kir-sha256",
            case["kir_sha256"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay.status.success());
    let replay: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(
        replay["schema"],
        "fe2o3-sim-semantic-differential-replay-v2"
    );
    assert_eq!(replay["status"], "reproduced");
    assert_eq!(replay["case"], *case);
}

#[test]
fn semantic_replay_fails_closed_on_identity_and_argument_substitution() {
    let output = command()
        .args([
            "semantic-replay-v2",
            "--seed",
            "0",
            "--case",
            "integer-i8",
            "--kir-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["code"], "replay_rejected");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("identity mismatch")
    );

    for arguments in [
        vec!["semantic-run-v2", "--seed", "1", "--seed", "1"],
        vec!["semantic-capabilities-v2", "unexpected"],
        vec![
            "semantic-replay-v2",
            "--seed",
            "0",
            "--case",
            "integer-i8",
            "--kir-sha256",
            "ABCDEF",
        ],
    ] {
        let output = command().args(arguments).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["status"], "error");
    }
}

#[test]
fn f32_commands_emit_bounded_exact_bit_matrix_and_replay_evidence() {
    let capabilities = command().arg("f32-capabilities-v3").output().unwrap();
    assert!(capabilities.status.success());
    assert!(capabilities.stderr.is_empty());
    assert!(capabilities.stdout.len() < 16 * 1024);
    let capabilities: Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    assert_eq!(
        capabilities["schema"],
        "fe2o3-sim-f32-differential-capabilities-v3"
    );
    assert_eq!(capabilities["authority"], "none");
    assert_eq!(capabilities["case_limit"], 17);
    assert_eq!(capabilities["maximum_rows_per_case"], 10);
    assert_eq!(capabilities["hardware_observed"], false);
    assert_eq!(capabilities["performance_prediction"], false);

    let output = command().arg("f32-run-v3").output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() < 128 * 1024);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "fe2o3-sim-f32-differential-v3");
    assert_eq!(report["status"], "agreement");
    assert_eq!(
        report["evidence_origin"],
        "independent_exact_bit_table_agreement"
    );
    assert_eq!(report["authority"], "none");
    assert_eq!(report["operation_cases"], 17);
    assert_eq!(report["rows_compared"], 149);
    assert_eq!(report["cases"].as_array().unwrap().len(), 17);
    assert_eq!(report["hardware_observed"], false);
    assert_eq!(report["performance_prediction"], false);

    for case in report["cases"].as_array().unwrap() {
        assert_eq!(case["oracle_sha256"], case["observed_sha256"]);
        assert_eq!(case["oracle_corpus_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(
            case["rows"].as_u64().unwrap(),
            case["row_ids"].as_array().unwrap().len() as u64
        );
    }
    let case = &report["cases"][12];
    let replay = command()
        .args([
            "f32-replay-v3",
            "--case",
            case["case_id"].as_str().unwrap(),
            "--kir-sha256",
            case["kir_sha256"].as_str().unwrap(),
            "--oracle-corpus-sha256",
            case["oracle_corpus_sha256"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay.status.success());
    assert!(replay.stderr.is_empty());
    let replay: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replay["schema"], "fe2o3-sim-f32-differential-replay-v3");
    assert_eq!(replay["status"], "reproduced");
    assert_eq!(replay["case"], *case);
}

#[test]
fn f32_replay_fails_closed_on_identity_and_argument_substitution() {
    let report = command().arg("f32-run-v3").output().unwrap();
    assert!(report.status.success());
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    let case = report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_id"] == "f32-add")
        .unwrap();
    let output = command()
        .args([
            "f32-replay-v3",
            "--case",
            "f32-add",
            "--kir-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--oracle-corpus-sha256",
            case["oracle_corpus_sha256"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["code"], "replay_rejected");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("identity mismatch")
    );

    let output = command()
        .args([
            "f32-replay-v3",
            "--case",
            "f32-add",
            "--kir-sha256",
            case["kir_sha256"].as_str().unwrap(),
            "--oracle-corpus-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["code"], "replay_rejected");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("oracle corpus identity mismatch")
    );

    for arguments in [
        vec!["f32-run-v3", "unexpected"],
        vec!["f32-capabilities-v3", "unexpected"],
        vec![
            "f32-replay-v3",
            "--case",
            "f32-add",
            "--kir-sha256",
            "ABCDEF",
            "--oracle-corpus-sha256",
            case["oracle_corpus_sha256"].as_str().unwrap(),
        ],
        vec![
            "f32-replay-v3",
            "--case",
            "f32-add",
            "--case",
            "f32-add",
            "--kir-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--oracle-corpus-sha256",
            case["oracle_corpus_sha256"].as_str().unwrap(),
        ],
        vec![
            "f32-replay-v3",
            "--case",
            "f32-add",
            "--kir-sha256",
            case["kir_sha256"].as_str().unwrap(),
            "--oracle-corpus-sha256",
            "ABCDEF",
        ],
    ] {
        let output = command().args(arguments).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["status"], "error");
    }
}
