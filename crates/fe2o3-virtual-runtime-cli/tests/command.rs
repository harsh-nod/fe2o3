#![cfg(target_os = "linux")]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    BasicBlock, BlockId, DebugSourceMapDocumentV2, DebugSourceMapFileV1, DebugSourceMapSpanV1,
    Function, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
    LaunchExtent, Operation, PreparedSimulationBundleV1, PreparedSimulationBundleV7,
    PreparedSimulationBundleV8, SemanticAggregateStorageMapV7, SemanticAggregateStorageMapV8,
    SemanticKernelStorageV1, SemanticKernelStorageV2, SemanticStorageMapV7, SemanticStorageMapV8,
    Signature, SimulationCompilerExecutionBindingV1, SimulationProductionKirIdentityV1,
    SimulationProductionKirIdentityV7, SimulationProductionKirIdentityV8,
    SimulationSourceLineageV1, Terminator, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8, VerifiedCanonicalKernelIrV12, VerifiedCanonicalKernelIrV13,
};
use sha2::{Digest, Sha256};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").unwrap();
    }
    result
}

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

fn v7_bundle() -> fe2o3_kernel_ir::VerifiedSimulationBundleV7 {
    let context = KernelContextTypeV1::new("v7_entry", [0x81; 32], [0x82; 32], [0x83; 32]);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::kernel_context_issue(
        fe2o3_kernel_ir::ValueId(0),
        context,
        KernelContextSourceIdentityV1::new([0x84; 32], [0x85; 32], [0x86; 32], [0x87; 32]),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = fe2o3_kernel_ir::Module::new("virtual-runtime-cli-v7");
    module.functions.push(Function::kernel_entry(
        "v7_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "v7_kernel",
        "v7_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let canonical = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    let digest = *canonical.identity().digest();
    let length = canonical.identity().canonical_length();
    let prepared = PreparedSimulationBundleV7::new(
        SimulationSourceLineageV1::new([0x88; 32], 301, [0x89; 32], 302).unwrap(),
        SimulationProductionKirIdentityV7::new(12, digest, length).unwrap(),
        41,
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([0x8a; 32], 16, "virtual-v7.rs".into()).unwrap()],
        vec![],
        vec![DebugSourceMapSpanV1::new([0x8a; 32], 1, 2, 1, 2).unwrap()],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"virtual-runtime-cli-v7-semantic".to_vec();
    let storage = SemanticStorageMapV7::new(
        *prepared.subject_identity(),
        1,
        Sha256::digest(&semantic).into(),
        semantic.len() as u64,
        [0x8b; 32],
        *prepared.canonical_kir_v12_digest(),
        prepared.canonical_kir_v12_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV7::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v12_digest(),
        prepared.canonical_kir_v12_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    prepared
        .finalize(source_map, semantic, storage, aggregate)
        .unwrap()
}

fn v8_bundle() -> fe2o3_kernel_ir::VerifiedSimulationBundleV8 {
    let context = KernelContextTypeV1::new("v8_entry", [0x91; 32], [0x92; 32], [0x93; 32]);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::kernel_context_issue(
        fe2o3_kernel_ir::ValueId(0),
        context,
        KernelContextSourceIdentityV1::new([0x94; 32], [0x95; 32], [0x96; 32], [0x97; 32]),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = fe2o3_kernel_ir::Module::new("virtual-runtime-cli-v8");
    module.functions.push(Function::kernel_entry(
        "v8_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "v8_kernel",
        "v8_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let digest = *canonical.identity().digest();
    let length = canonical.identity().canonical_length();
    let prepared = PreparedSimulationBundleV8::new(
        SimulationSourceLineageV1::new([0x98; 32], 401, [0x99; 32], 402).unwrap(),
        SimulationProductionKirIdentityV8::new(13, digest, length).unwrap(),
        43,
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([0x9a; 32], 16, "virtual-v8.rs".into()).unwrap()],
        vec![],
        vec![DebugSourceMapSpanV1::new([0x9a; 32], 1, 2, 1, 2).unwrap()],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"virtual-runtime-cli-v8-semantic".to_vec();
    let storage = SemanticStorageMapV8::new(
        *prepared.subject_identity(),
        1,
        Sha256::digest(&semantic).into(),
        semantic.len() as u64,
        [0x9b; 32],
        *prepared.canonical_kir_v13_digest(),
        prepared.canonical_kir_v13_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV8::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v13_digest(),
        prepared.canonical_kir_v13_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    prepared
        .finalize(source_map, semantic, storage, aggregate)
        .unwrap()
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
    let bundle_runtime_identity = result["lifecycle"]["runtime_identity"].as_str().unwrap();

    let loose_exact = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--kir-v7")
        .arg(fixture("kernel.kir"))
        .arg("--request")
        .arg(fixture("request.json"))
        .arg("--target")
        .arg("gfx942:xnack-")
        .output()
        .unwrap();
    assert!(
        loose_exact.status.success(),
        "{}",
        String::from_utf8_lossy(&loose_exact.stderr)
    );
    let loose_exact: serde_json::Value = serde_json::from_slice(&loose_exact.stdout).unwrap();
    assert_ne!(
        bundle_runtime_identity,
        loose_exact["lifecycle"]["runtime_identity"]
            .as_str()
            .unwrap()
    );

    let loose_neutral = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--kir-v7")
        .arg(fixture("kernel.kir"))
        .arg("--request")
        .arg(fixture("request.json"))
        .output()
        .unwrap();
    assert!(
        loose_neutral.status.success(),
        "{}",
        String::from_utf8_lossy(&loose_neutral.stderr)
    );
    let loose_neutral: serde_json::Value = serde_json::from_slice(&loose_neutral.stdout).unwrap();
    assert_ne!(
        loose_exact["lifecycle"]["runtime_identity"],
        loose_neutral["lifecycle"]["runtime_identity"]
    );
}

#[test]
fn v7_bundle_executes_exact_v12_through_virtual_runtime() {
    let bundle = v7_bundle();
    let bundle_file = TempFile::new(bundle.canonical_bytes());
    let request = TempFile::new(
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"v7_kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[]}"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--bundle-v7")
        .arg(bundle_file.path())
        .arg("--request")
        .arg(request.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["target"], "gfx942:xnack-");
    assert_eq!(
        result["kir"]["canonical_bytes"],
        bundle.canonical_kir_v12_length()
    );
    assert_eq!(
        result["bundle_sha256"],
        format!("0x{}", hex(bundle.identity().as_bytes()))
    );
    assert_eq!(result["dispatches"].as_array().unwrap().len(), 1);
}

#[test]
fn v8_bundle_executes_exact_v13_through_virtual_runtime() {
    let bundle = v8_bundle();
    let bundle_file = TempFile::new(bundle.canonical_bytes());
    let request = TempFile::new(
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"v8_kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[]}"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-virtual-runtime"))
        .arg("--bundle-v8")
        .arg(bundle_file.path())
        .arg("--request")
        .arg(request.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema"], "fe2o3-virtual-runtime-result-v1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["target"], "gfx942:xnack-");
    assert_eq!(
        result["kir"]["canonical_bytes"],
        bundle.canonical_kir_v13_length()
    );
    assert_eq!(
        result["bundle_sha256"],
        format!("0x{}", hex(bundle.identity().as_bytes()))
    );
    assert_eq!(result["dispatches"].as_array().unwrap().len(), 1);
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
