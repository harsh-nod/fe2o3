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

#[test]
fn protected_qualification_enumerates_exact_unavailable_authority() {
    let output = command()
        .arg("protected-physical-qualification-v2")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() < 16 * 1024);
    let qualification: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        qualification["schema"],
        "fe2o3-simulator-direct-kfd-differential-qualification-v2"
    );
    assert_eq!(qualification["production_ready"], false);
    assert_eq!(qualification["hardware_observed"], false);
    assert_eq!(qualification["parity_observed"], false);
    assert_eq!(qualification["grants_authority"], false);
    assert_eq!(qualification["synthetic_verifier_admitted"], false);
    assert_eq!(qualification["execution_failure_can_mint_report"], false);
    assert_eq!(qualification["stale_publication_can_mint_report"], false);
    assert_eq!(qualification["stale_device_can_mint_report"], false);
    assert_eq!(qualification["ambiguous_completion_can_mint_report"], false);
    assert_eq!(
        qualification["available_boundary"],
        "authenticated_generated_invocation_to_single_use_execute_and_compare"
    );
    assert_eq!(
        qualification["current_blocker"],
        "concrete_backend_not_implemented"
    );
    assert!(qualification.get("hardware_passes").is_none());
    assert!(qualification.get("parity_passes").is_none());

    let prerequisites = qualification["prerequisites"].as_array().unwrap();
    assert_eq!(prerequisites.len(), 14);
    assert!(prerequisites.iter().any(|record| {
        record["prerequisite"] == "sealed_protected_verifier_adapter"
            && record["status"] == "implemented_mechanism"
    }));
    assert!(prerequisites.iter().any(|record| {
        record["prerequisite"] == "proof_to_executable_machine_refinement"
            && record["unavailable_reason"] == "machine_refinement_receipt_producer_not_implemented"
    }));
}
