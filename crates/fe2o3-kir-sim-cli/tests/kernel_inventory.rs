//! Public read-only metadata discovery; these synthetic cases do not execute.
#![cfg(target_os = "linux")]
#[path = "fixtures/diagnostic_kir_v18.rs"]
mod fixture;
use serde_json::Value;
use std::process::{Command, Output};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"));
    command.env_clear().arg("inspect");
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
fn inspect(files: &fixture::Files) -> Output {
    command()
        .arg("--diagnostic-kir-v18")
        .arg(&files.kir)
        .output()
        .unwrap()
}

#[test]
fn public_inspect_discovers_actual_ids_and_abi_without_request_or_execution() {
    let module = fixture::module();
    let files = fixture::Files::new(&fixture::bytes(&module), b"not a request");
    std::fs::remove_file(&files.request).unwrap();
    let output = inspect(&files);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let kernel = &module.kernels[0];
    let entry = module.function(&kernel.entry).unwrap();
    assert_eq!(report["schema"], "fe2o3-kernel-inventory-v1");
    assert_eq!(report["kir"]["wire_version"], 18);
    assert_eq!(report["storage_layouts"], module.storage_layouts.len());
    assert_eq!(report["kernels"][0]["id"], kernel.id.as_str());
    assert_eq!(report["kernels"][0]["entry"], entry.id.as_str());
    assert_eq!(
        report["kernels"][0]["parameters"].as_array().unwrap().len(),
        entry.signature.parameters.len()
    );
    assert_eq!(
        report["kernels"][0]["parameters"][0]["type"],
        serde_json::json!({"kind":"pointer","address_space":"global","access":"read_write",
            "pointee":{"kind":"scalar","type":"u32","bits":32}})
    );
    assert_eq!(
        report["kernels"][0]["parameters"][1]["type"],
        serde_json::json!({"kind":"scalar","type":"u32","bits":32})
    );
    assert_eq!(report["kernels"][0]["request_abi"], "entry_parameter_order");
    assert_eq!(report["simulator_admission"], "not_checked");
    assert_eq!(report["authority"], "observation_only");
    for flag in [
        "simulated",
        "source_authentication",
        "proof_authority",
        "compiler_execution_authority",
        "launch_authority",
        "hardware_observed",
        "performance_prediction",
    ] {
        assert_eq!(report[flag], false);
    }
}

#[test]
fn public_inspect_file_output_is_no_clobber_and_help_names_the_explicit_route() {
    let files = fixture::Files::new(&fixture::bytes(&fixture::module()), b"");
    let expected = inspect(&files);
    assert!(expected.status.success());
    let destination = files.path("inventory.json");
    let output = command()
        .arg("--diagnostic-kir-v18")
        .arg(&files.kir)
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read(&destination).unwrap(), expected.stdout);
    let output = command()
        .arg("--diagnostic-kir-v18")
        .arg(&files.kir)
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read(&destination).unwrap(), expected.stdout);
    let help = command().arg("--help").output().unwrap();
    assert!(help.status.success() && help.stderr.is_empty());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("inspect --diagnostic-kir-v18 PATH [--output PATH]")
    );
}

#[test]
fn public_inspect_rejects_execution_flags_and_wrong_versions_before_publication() {
    let files = fixture::Files::new(&fixture::bytes(&fixture::module()), b"");
    let destination = files.path("inventory.json");
    for extra in [
        "--request",
        "--record-canonical-schedule",
        "--kir-v7",
        "--bundle",
    ] {
        error(
            command()
                .arg("--diagnostic-kir-v18")
                .arg(&files.kir)
                .arg("--output")
                .arg(&destination)
                .arg(extra)
                .arg(&files.request)
                .output()
                .unwrap(),
            "arguments",
            "invalid_command_line",
        );
        assert!(!destination.exists());
    }
    let mut bytes = fixture::bytes(&fixture::module());
    bytes[8..10].copy_from_slice(&17_u16.to_le_bytes());
    std::fs::write(&files.kir, bytes).unwrap();
    error(
        command()
            .arg("--diagnostic-kir-v18")
            .arg(&files.kir)
            .arg("--output")
            .arg(&destination)
            .output()
            .unwrap(),
        "kir_admission",
        "kir_v18_wrong_version",
    );
    assert!(!destination.exists());
}
