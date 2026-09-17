#![cfg(target_os = "linux")]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::*;
use sha2::{Digest, Sha256};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-debug-v6-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

// Deliberately test-authored custody, not evidence of production source export.
fn fixture() -> VerifiedSimulationBundleV6 {
    let kir =
        fs::read(root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir")).unwrap();
    let canonical =
        VerifiedCanonicalKernelIrV11::from_module(decode_module_v7(&kir).unwrap()).unwrap();
    let production = SimulationProductionKirIdentityV6::new(
        11,
        *canonical.identity().digest(),
        canonical.identity().canonical_length(),
    )
    .unwrap();
    let prepared = PreparedSimulationBundleV6::new(
        SimulationSourceLineageV1::new([0x61; 32], 101, [0x62; 32], 102).unwrap(),
        production,
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let original = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json")).unwrap(),
    )
    .unwrap();
    let source = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        original.files().to_vec(),
        original.sites().to_vec(),
        original.eliminated().to_vec(),
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"explicitly-test-authored-v6-semantic-fixture".to_vec();
    let storage = SemanticStorageMapV6::new(
        *prepared.subject_identity(),
        1,
        Sha256::digest(&semantic).into(),
        semantic.len() as u64,
        [0x63; 32],
        *prepared.canonical_kir_v11_digest(),
        prepared.canonical_kir_v11_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV6::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v11_digest(),
        prepared.canonical_kir_v11_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    prepared
        .finalize(source, semantic, storage, aggregate)
        .unwrap()
}

fn command(path: &Path, input: &str, extra: &[&str]) -> Output {
    let request = root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["sim", "--bundle-v6"])
        .arg(path)
        .arg("--request")
        .arg(request)
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn exact_v6_route_retains_the_embedded_source_map() {
    let directory = Directory::new();
    let path = directory.0.join("fill.fe2sim");
    let bundle = fixture();
    let expected_map = bundle
        .debug_map_identity()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    fs::write(&path, bundle.canonical_bytes()).unwrap();
    let request = root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&path, &request).unwrap();
    let evidence = admitted.input().simulation_bundle_evidence().unwrap();
    assert_eq!(evidence.envelope_version, 6);
    assert_eq!(evidence.envelope_identity, *bundle.identity().as_bytes());
    assert_eq!(evidence.subject_identity, *bundle.subject_identity());
    assert_eq!(admitted.input().module.identity().wire_version(), 11);
    assert_eq!(
        admitted.input().kir_sha256,
        *bundle.canonical_kir_v11_digest()
    );
    assert_eq!(
        evidence.production_kir_bytes,
        bundle.canonical_kir_v11_length()
    );
    assert!(!admitted.grants_compiler_authority());
    assert!(!admitted.grants_launch_authority());
    assert!(fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(&path, &request).is_err());
    let output = command(
        &path,
        concat!(
            "{\"operation\":\"step\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":1,\"expected_revision\":0,\"direction\":\"forward\",\"granularity\":\"operation\",\"count\":1}\n",
            "{\"operation\":\"inspect_values\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":2,\"expected_revision\":1,\"scope\":{\"level\":\"dispatch\"},\"selector\":{\"selector\":\"all\"},\"page\":{\"limit\":16}}\n"
        ),
        &[],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let lines = String::from_utf8(output.stdout).unwrap();
    let responses: Vec<serde_json::Value> = lines
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["status"], "ok");
    assert_eq!(responses[1]["status"], "ok", "{responses:?}");
    assert_eq!(responses[1]["session"]["hardware_observed"], false);
    let snapshot = &responses[1]["result"]["snapshot"];
    assert_eq!(snapshot["site"]["source"]["status"], "resolved");
    assert_eq!(
        snapshot["site"]["source"]["location"]["map_identity"],
        expected_map
    );
    assert_eq!(
        snapshot["site"]["source"]["location"]["provenance"],
        "compiler_bundle_bound"
    );
}

#[test]
fn corrupt_v6_is_rejected_before_any_protocol_output() {
    let directory = Directory::new();
    let path = directory.0.join("corrupt.fe2sim");
    let mut bytes = fixture().canonical_bytes().to_vec();
    bytes[24] ^= 1;
    fs::write(&path, bytes).unwrap();
    let output = command(&path, "", &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["stage"], "input");
    assert_eq!(error["code"], "simulation_bundle_rejected");
}

#[test]
fn bundle_inputs_cannot_be_duplicated_or_overridden() {
    let directory = Directory::new();
    let path = directory.0.join("fill.fe2sim");
    fs::write(&path, fixture().canonical_bytes()).unwrap();
    for extra in [
        vec!["--bundle-v6", "another"],
        vec!["--bundle-v5", "another"],
        vec![
            "--source-map",
            "another",
            "--source-bundle-subject",
            "6161616161616161616161616161616161616161616161616161616161616161",
        ],
    ] {
        let output = command(&path, "", &extra);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["stage"], "arguments");
        assert_eq!(error["code"], "invalid_command_line");
    }
}
