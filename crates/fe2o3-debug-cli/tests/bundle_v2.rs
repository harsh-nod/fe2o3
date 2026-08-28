#![cfg(target_os = "linux")]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_debug_protocol::{
    CapturedValueV1, DebugResponseV1, DebugValueTypeV1, ProtocolLimitsV1, SourceVariableResponseV2,
    SourceVariableValueAvailabilityV2, ValueAvailabilityV1, decode_response_line_v1,
    decode_source_variable_response_line_v2,
};
use fe2o3_kernel_ir::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV1, DebugSourceMapDocumentV2,
    DebugSourceScopeV2, DebugSourceVariableBindingV2, DebugSourceVariableFallbackV2,
    DebugSourceVariableLocationV2, DebugSourceVariableV2, SimulationCompilerExecutionBindingV1,
    SimulationProductionKirIdentityV1, SimulationSourceLineageV1, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV1, VerifiedSimulationBundleV2,
    decode_module_v7,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-debug-bundle-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is in workspace/crates")
        .to_owned()
}

fn fill_kir() -> Vec<u8> {
    fs::read(workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir")).unwrap()
}

fn inner_bundle() -> VerifiedSimulationBundleV1 {
    let kir = fill_kir();
    let canonical = VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir.clone()).unwrap();
    let production =
        VerifiedCanonicalKernelIrV8::from_module(decode_module_v7(&kir).unwrap()).unwrap();
    VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([3; 32], 33, [4; 32], 44).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production.identity().digest(),
            production.identity().canonical_length(),
        )
        .unwrap(),
        "gfx942:xnack-",
        canonical,
        None,
    )
    .unwrap()
}

fn test_authored_source_map_v2(inner: &VerifiedSimulationBundleV1) -> DebugSourceMapDocumentV2 {
    // The fixture is compiler-shaped, but it is deliberately not evidence that
    // the production compiler emitted or authenticated source-variable data.
    let v1 = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(workspace_root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json"))
            .unwrap(),
    )
    .unwrap();
    let span = v1.sites()[0].spans()[0];
    let scope = DebugSourceScopeV2::new([0x31; 32], 0, None, 0, span).unwrap();
    let variable = DebugSourceVariableV2::new(
        [0x41; 32],
        "buffer".into(),
        0,
        scope.identity(),
        DebugSourceVariableFallbackV2::NotInScope,
        vec![
            DebugSourceVariableLocationV2::new(
                0,
                0,
                1,
                DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
            )
            .unwrap(),
        ],
    )
    .unwrap();
    DebugSourceMapDocumentV2::new(
        DebugSourceMapBindingV1::new(
            *inner.subject_identity(),
            *inner.canonical_kir_v7_identity().digest(),
            inner.canonical_kir_v7_identity().canonical_length(),
        )
        .unwrap(),
        v1.files().to_vec(),
        v1.sites().to_vec(),
        v1.eliminated().to_vec(),
        vec![scope],
        vec![variable],
    )
    .unwrap()
}

#[test]
fn explicit_v2_bundle_route_queries_a_real_checkpoint_without_authority() {
    let directory = TestDirectory::new();
    let inner = inner_bundle();
    let source_map = test_authored_source_map_v2(&inner);
    let bundle = VerifiedSimulationBundleV2::new(inner, source_map).unwrap();
    assert!(!bundle.authenticates_compiler_execution());
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_load_authority());
    assert!(!bundle.grants_launch_authority());

    let bundle_path = directory.0.join("fill-v2.fe2sim");
    fs::write(&bundle_path, bundle.canonical_bytes()).unwrap();
    let request = workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v2(&bundle_path, &request)
        .expect("admit exact V2 envelope");
    assert!(!admitted.authenticates_compiler_execution());
    assert!(!admitted.grants_proof_authority());
    assert!(!admitted.grants_artifact_authority());
    assert!(!admitted.grants_compiler_authority());
    assert!(!admitted.grants_hardware_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
    assert!(fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(&bundle_path, &request).is_err());

    let requests = concat!(
        "{\"operation\":\"step\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":1,\"expected_revision\":0,\"direction\":\"forward\",\"granularity\":\"operation\",\"count\":1}\n",
        "{\"operation\":\"inspect_source_variables\",\"schema\":\"fe2o3-debug-source-variable-request-v2\",\"request_id\":2,\"expected_revision\":1,\"scope\":{\"level\":\"dispatch\"},\"frame\":1,\"selector\":{\"selector\":\"identity\",\"variable_identity\":\"4141414141414141414141414141414141414141414141414141414141414141\"},\"page\":{\"limit\":1}}\n"
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .arg("sim")
        .arg("--bundle-v2")
        .arg(&bundle_path)
        .arg("--request")
        .arg(&request)
        .args(["--protocol", "jsonl", "--wave-width", "64"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(requests.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let lines: Vec<_> = output
        .stdout
        .split_inclusive(|byte| *byte == b'\n')
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(matches!(
        decode_response_line_v1(lines[0], ProtocolLimitsV1::default()).unwrap(),
        DebugResponseV1::Ok { .. }
    ));
    let response =
        decode_source_variable_response_line_v2(lines[1], ProtocolLimitsV1::default()).unwrap();
    assert!(matches!(
        response,
        SourceVariableResponseV2::Ok { values, .. }
            if matches!(
                values.as_slice(),
                [fe2o3_debug_protocol::SourceVariableValueV2 {
                    generation: 1,
                    availability: SourceVariableValueAvailabilityV2::Value {
                        value: ValueAvailabilityV1::Captured {
                            value_type: DebugValueTypeV1::Pointer { .. },
                            value: CapturedValueV1::AllocationRelativePointer { .. },
                            ..
                        }
                    },
                    ..
                }]
            )
    ));
}
