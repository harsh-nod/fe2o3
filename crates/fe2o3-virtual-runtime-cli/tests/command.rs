#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    PreparedSimulationBundleV1, SimulationCompilerExecutionBindingV1,
    SimulationProductionKirIdentityV1, SimulationSourceLineageV1, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-virtual-runtime-cli-input-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fe2o3-kir-sim-cli/tutorial/fill-v1")
        .join(name)
}

#[test]
fn admitted_kir_runs_two_serial_virtual_dispatches() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--kir-v7")
        .arg(fixture("kernel.kir"))
        .arg("--request")
        .arg(fixture("request.json"))
        .arg("--repeat")
        .arg("2")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema"], "fe2o3-virtual-runtime-result-v1");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["simulated"], true);
    assert_eq!(result["hardware_observed"], false);
    assert_eq!(result["performance_prediction"], false);
    assert_eq!(
        result["lifecycle"]["schema"],
        "fe2o3-virtual-runtime-lifecycle-v1"
    );
    assert_eq!(result["lifecycle"]["serial_dependency_edges"], 1);
    assert_eq!(result["lifecycle"]["completed_dispatches"], 2);
    assert!(
        result["lifecycle"]["runtime_identity"]
            .as_str()
            .unwrap()
            .starts_with("0x")
    );
    assert!(result["lifecycle"]["module"].as_u64().is_some());
    assert!(result["lifecycle"]["queue"].as_u64().is_some());
    assert_eq!(result["lifecycle"]["allocations"], 1);
    assert_eq!(result["lifecycle"]["terminal_buffer_state"], "released");
    assert_eq!(result["lifecycle"]["terminal_module_state"], "released");
    assert_eq!(result["lifecycle"]["terminal_queue_state"], "released");
    assert_eq!(result["dispatches"].as_array().unwrap().len(), 2);
    assert!(result["dispatches"][0]["depends_on"].is_null());
    assert_eq!(result["dispatches"][0]["state"], "completed");
    assert_eq!(result["dispatches"][1]["state"], "completed");
    assert_eq!(
        result["dispatches"][1]["depends_on"],
        result["dispatches"][0]["completion"]
    );
    assert_eq!(
        result["buffers"][0]["bytes"],
        "0x11000000110000001100000011000000"
    );
}

#[test]
fn invalid_bound_is_a_stable_typed_json_failure() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--kir-v7")
        .arg(fixture("kernel.kir"))
        .arg("--request")
        .arg(fixture("request.json"))
        .arg("--repeat")
        .arg("0")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema"], "fe2o3-virtual-runtime-error-v1");
    assert_eq!(error["stage"], "arguments");
    assert_eq!(error["code"], "invalid_command_line");
    assert_eq!(error["hardware_observed"], false);
    assert_eq!(error["performance_prediction"], false);
}

#[test]
fn verified_bundle_selects_its_exact_semantic_target() {
    let bytes = fs::read(fixture("kernel.kir")).unwrap();
    let (canonical_v7, module) =
        VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(bytes).unwrap();
    let production = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
    let production_identity = SimulationProductionKirIdentityV1::v8(
        *production.identity().digest(),
        production.identity().canonical_length(),
    )
    .unwrap();
    let bundle = PreparedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([1; 32], 1, [2; 32], 1).unwrap(),
        production_identity,
        "gfx942:xnack-",
        canonical_v7,
    )
    .unwrap()
    .finalize_without_source_map()
    .unwrap();
    let bundle = TempFile::new(bundle.canonical_bytes());
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--bundle")
        .arg(bundle.path())
        .arg("--request")
        .arg(fixture("request.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["target"], "gfx942:xnack-");
    assert!(result["bundle_sha256"].as_str().is_some());
}

#[test]
fn semantic_memory_fault_is_a_stable_typed_json_failure() {
    let request = fs::read_to_string(fixture("request.json"))
        .unwrap()
        .replace("\"grid\":[4,1,1]", "\"grid\":[5,1,1]");
    let request = TempFile::new(request.as_bytes());
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--kir-v7")
        .arg(fixture("kernel.kir"))
        .arg("--request")
        .arg(request.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema"], "fe2o3-virtual-runtime-error-v1");
    assert_eq!(error["stage"], "virtual_runtime");
    assert_eq!(error["code"], "simulation_failed");
    assert_eq!(error["authority"], "observation_only");
}

#[test]
fn early_release_is_a_stable_typed_lifecycle_failure() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--kir-v7")
        .arg(fixture("kernel.kir"))
        .arg("--request")
        .arg(fixture("request.json"))
        .arg("--fault")
        .arg("early-release")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema"], "fe2o3-virtual-runtime-error-v1");
    assert_eq!(error["stage"], "virtual_runtime");
    assert_eq!(error["code"], "resource_in_use");
    assert_eq!(error["authority"], "observation_only");
    assert_eq!(error["hardware_observed"], false);
    assert_eq!(error["performance_prediction"], false);
}
