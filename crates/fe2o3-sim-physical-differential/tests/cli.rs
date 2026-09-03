use std::process::Command;

use serde_json::Value;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fe2o3-sim-physical-differential"))
}

#[test]
fn physical_capabilities_report_exact_blocker_and_zero_passes() {
    let output = command().arg("physical-capabilities-v1").output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() < 4 * 1024);
    let capabilities: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        capabilities["schema"],
        "fe2o3-simulator-direct-kfd-differential-capabilities-v1"
    );
    assert_eq!(
        capabilities["current_production_blocker"],
        "protected_verifier_unavailable"
    );
    assert_eq!(capabilities["hardware_passes"], 0);
    assert_eq!(capabilities["parity_passes"], 0);
    assert_eq!(capabilities["hardware_unavailable_counts_as_pass"], false);
    assert_eq!(capabilities["legacy_llvm_fixture_excluded"], true);
    assert_eq!(
        capabilities["executable_compare_surface"],
        "generated-host-library-api-only"
    );

    let hostile = command()
        .args(["physical-capabilities-v1", "unexpected"])
        .output()
        .unwrap();
    assert!(!hostile.status.success());
    assert!(hostile.stdout.is_empty());
    let error: Value = serde_json::from_slice(&hostile.stderr).unwrap();
    assert_eq!(error["code"], "invalid_command_line");
    assert_eq!(error["hardware_observed"], false);
}
