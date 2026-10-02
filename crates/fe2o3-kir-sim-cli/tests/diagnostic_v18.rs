//! Executable synthetic V18 ingress controls; no source or hardware claim.
#![cfg(target_os = "linux")]
#[path = "fixtures/diagnostic_kir_v18.rs"]
mod fixture;
use serde_json::Value;
use std::process::{Command, Output};

fn command(files: &fixture::Files) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"));
    command
        .env_clear()
        .arg("--diagnostic-kir-v18")
        .arg(&files.kir)
        .arg("--request")
        .arg(&files.request);
    command
}
fn error(output: Output, stage: &str, kind: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema"], "fe2o3-simulation-error-v1");
    assert_eq!(error["stage"], stage);
    assert_eq!(error["kind"], kind);
}
#[test]
fn canonical_v18_binary_executes_generic_memory_and_preserves_canaries() {
    for value in [37, u32::MAX] {
        let files = fixture::Files::new(
            &fixture::bytes(&fixture::module()),
            &fixture::request(value),
        );
        let output = command(&files).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["simulated"], true);
        for flag in [
            "hardware_observed",
            "hardware_validation",
            "performance_prediction",
        ] {
            assert_eq!(result[flag], false);
        }
        assert_eq!(result["counts"]["invocations_executed"], 64);
        let expected = fixture::output(value)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            result["arguments"][0]["value"]["bytes"],
            format!("0x{expected}")
        );
        assert_eq!(
            result["arguments"][0]["value"]["initialized"],
            format!("0x{}00", "ff".repeat(32))
        );
    }
}
#[test]
fn forbidden_schedule_and_wrong_version_leave_output_unpublished() {
    let files = fixture::Files::new(&fixture::bytes(&fixture::module()), &fixture::request(37));
    let output = files.path("output.json");
    let schedule = files.path("schedule.json");
    error(
        command(&files)
            .arg("--output")
            .arg(&output)
            .arg("--record-canonical-schedule")
            .arg(&schedule)
            .output()
            .unwrap(),
        "arguments",
        "schedule_input_unsupported",
    );
    assert!(!output.exists() && !schedule.exists());
    let mut bytes = fixture::bytes(&fixture::module());
    bytes[8..10].copy_from_slice(&17_u16.to_le_bytes());
    std::fs::write(&files.kir, bytes).unwrap();
    error(
        command(&files)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap(),
        "kir_admission",
        "kir_v18_wrong_version",
    );
    assert!(!output.exists());
}
#[test]
fn storage_execution_is_rejected_instead_of_discarding_layouts() {
    let files = fixture::Files::new(
        &fixture::bytes(&fixture::storage_module(true)),
        &fixture::storage_request(),
    );
    error(
        command(&files).output().unwrap(),
        "preflight",
        "preflight_unsupported",
    );
}
