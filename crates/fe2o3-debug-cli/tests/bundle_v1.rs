#![cfg(target_os = "linux")]

use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_debug_cli::{
    CompilerBundleDebugRunErrorV1, run_admitted_jsonl_with_compiler_source_map_v1,
};
use fe2o3_debug_protocol::{
    CapabilityAvailabilityV1, DebugCapabilityNameV1, DebugResponseV1, DebugResultV1,
    OpaqueIdentityV1, ProtocolLimitsV1, SourceMapProvenanceV1, SourceSiteAvailabilityV1,
    decode_response_line_v1,
};
use fe2o3_kernel_ir::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV1, SimulationCompilerExecutionBindingV1,
    SimulationDebugMapV1, SimulationProductionKirIdentityV1, SimulationSourceLineageV1,
    VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV1,
    decode_module_v7,
};
use fe2o3_kir_debugger::DebugWaveWidthV1;
use serde_json::Value;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

struct ClosedWriter;

impl Write for ClosedWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
    }
}

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-debug-bundle-v1-{}-{sequence}",
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

fn build_bundle(debug_map: Option<Vec<u8>>) -> VerifiedSimulationBundleV1 {
    build_bundle_for_target(debug_map, "gfx942:xnack-")
}

fn build_bundle_for_target(debug_map: Option<Vec<u8>>, target: &str) -> VerifiedSimulationBundleV1 {
    try_build_bundle_for_target(debug_map, target).unwrap()
}

fn try_build_bundle_for_target(
    debug_map: Option<Vec<u8>>,
    target: &str,
) -> Result<VerifiedSimulationBundleV1, fe2o3_kernel_ir::SimulationBundleErrorV1> {
    let kir = fill_kir();
    let canonical = VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir.clone()).unwrap();
    let module = decode_module_v7(&kir).unwrap();
    let production = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
    VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([3; 32], 33, [4; 32], 44).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production.identity().digest(),
            production.identity().canonical_length(),
        )
        .unwrap(),
        target,
        canonical,
        debug_map
            .map(|bytes| SimulationDebugMapV1::from_unverified_canonical_bytes(bytes).unwrap()),
    )
}

fn source_map_for(subject: [u8; 32], stale_kir: bool, stale_subject: bool) -> Vec<u8> {
    let root = workspace_root();
    let document = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(root.join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json")).unwrap(),
    )
    .unwrap();
    let kir = VerifiedCanonicalKernelIrV7::from_canonical_bytes(fill_kir()).unwrap();
    let mut subject = subject;
    if stale_subject {
        subject[0] ^= 1;
    }
    let mut digest = *kir.identity().digest();
    if stale_kir {
        digest[0] ^= 1;
    }
    DebugSourceMapDocumentV1::new(
        DebugSourceMapBindingV1::new(subject, digest, kir.identity().canonical_length()).unwrap(),
        document.files().to_vec(),
        document.sites().to_vec(),
        document.eliminated().to_vec(),
    )
    .unwrap()
    .to_canonical_json_bytes()
    .unwrap()
}

fn bundle_with_map(stale_kir: bool, stale_subject: bool) -> VerifiedSimulationBundleV1 {
    let subject = *build_bundle(None).subject_identity();
    build_bundle(Some(source_map_for(subject, stale_kir, stale_subject)))
}

fn run_debug(
    bundle: &Path,
    request: &Path,
    wave_width: &str,
    requests: &[u8],
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .arg("sim")
        .arg("--bundle")
        .arg(bundle)
        .arg("--request")
        .arg(request)
        .args(["--protocol", "jsonl", "--wave-width", wave_width])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(requests).unwrap();
    child.wait_with_output().unwrap()
}

fn decode_lines(bytes: &[u8]) -> Vec<DebugResponseV1> {
    bytes
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| decode_response_line_v1(line, ProtocolLimitsV1::default()).unwrap())
        .collect()
}

const DISCOVER_AND_RESOLVE: &[u8] = concat!(
    "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":1,\"expected_revision\":0}\n",
    "{\"operation\":\"resolve_source\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":2,\"expected_revision\":0,\"site\":{\"function_ordinal\":0,\"block_ordinal\":0,\"point\":{\"kind\":\"operation\",\"operation_ordinal\":2}}}\n"
)
.as_bytes();

#[test]
fn embedded_map_is_bundle_bound_and_reusable_across_requests_and_wave_widths() {
    let directory = TestDirectory::new();
    let bundle = directory.0.join("fill.fe2sim");
    let verified = bundle_with_map(false, false);
    let expected_map_identity = verified.debug_map_identity().unwrap();
    fs::write(&bundle, verified.canonical_bytes()).unwrap();
    let base_request =
        fs::read(workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"))
            .unwrap();
    let requests = [
        ("32", base_request.clone()),
        ("64", [b" \n".as_slice(), base_request.as_slice()].concat()),
    ];

    for (index, (wave_width, request_bytes)) in requests.into_iter().enumerate() {
        let request = directory.0.join(format!("request-{index}.json"));
        fs::write(&request, request_bytes).unwrap();
        let output = run_debug(&bundle, &request, wave_width, DISCOVER_AND_RESOLVE);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let responses = decode_lines(&output.stdout);
        let DebugResponseV1::Ok { result, .. } = &responses[0] else {
            panic!("capabilities failed")
        };
        let DebugResultV1::Capabilities { capabilities } = result.as_ref() else {
            panic!("wrong capabilities result")
        };
        assert!(capabilities.iter().any(|capability| {
            capability.name == DebugCapabilityNameV1::SourceSites
                && capability.availability == CapabilityAvailabilityV1::Available
        }));
        let DebugResponseV1::Ok { result, .. } = &responses[1] else {
            panic!("source resolution failed")
        };
        let DebugResultV1::Source { site } = result.as_ref() else {
            panic!("wrong source result")
        };
        assert!(matches!(
            site.source,
            SourceSiteAvailabilityV1::Resolved { location }
                if location.provenance == SourceMapProvenanceV1::CompilerBundleBound
                    && location.map_identity.as_bytes() == expected_map_identity
        ));
    }
}

#[test]
fn exact_bundle_target_is_bound_into_the_debug_session_identity() {
    let directory = TestDirectory::new();
    let request = workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    let mut configurations = Vec::new();
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let bundle = directory.0.join(format!("{}.fe2sim", &target[..6]));
        fs::write(
            &bundle,
            build_bundle_for_target(None, target).canonical_bytes(),
        )
        .unwrap();
        let output = run_debug(&bundle, &request, "64", DISCOVER_AND_RESOLVE);
        assert!(output.status.success());
        let responses = decode_lines(&output.stdout);
        let DebugResponseV1::Ok { session, .. } = &responses[0] else {
            panic!("capabilities failed")
        };
        assert!(session.simulated);
        assert!(!session.hardware_observed);
        assert!(!session.performance_prediction);
        configurations.push(session.configuration_identity);
    }
    assert_ne!(configurations[0], configurations[1]);
}

#[test]
fn absent_map_is_typed_unavailable_and_external_override_is_rejected() {
    let directory = TestDirectory::new();
    let bundle = directory.0.join("fill.fe2sim");
    fs::write(&bundle, build_bundle(None).canonical_bytes()).unwrap();
    let request = workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    let output = run_debug(&bundle, &request, "64", DISCOVER_AND_RESOLVE);
    assert!(output.status.success());
    let responses = decode_lines(&output.stdout);
    let DebugResponseV1::Ok { result, .. } = &responses[0] else {
        panic!("capabilities failed")
    };
    let DebugResultV1::Capabilities { capabilities } = result.as_ref() else {
        panic!("wrong capabilities result")
    };
    assert!(capabilities.iter().any(|capability| {
        capability.name == DebugCapabilityNameV1::SourceSites
            && capability.availability == CapabilityAvailabilityV1::Unavailable
    }));
    let DebugResponseV1::Ok { result, .. } = &responses[1] else {
        panic!("source inspection did not return a typed result")
    };
    let DebugResultV1::Source { site } = result.as_ref() else {
        panic!("wrong source result")
    };
    assert!(matches!(
        site.source,
        SourceSiteAvailabilityV1::Unavailable { .. }
    ));

    let override_attempt = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .arg("sim")
        .arg("--bundle")
        .arg(bundle)
        .arg("--request")
        .arg(request)
        .arg("--source-map")
        .arg(workspace_root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json"))
        .args(["--source-bundle-subject", &"11".repeat(32)])
        .output()
        .unwrap();
    assert!(!override_attempt.status.success());
    let error: Value = serde_json::from_slice(&override_attempt.stderr).unwrap();
    assert_eq!(error["stage"], "arguments");
    assert_eq!(error["code"], "invalid_command_line");
}

#[test]
fn stale_map_bindings_and_committed_map_substitution_fail_closed() {
    let directory = TestDirectory::new();
    let request = workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    for (stale_kir, stale_subject) in [(true, false), (false, true)] {
        let subject = *build_bundle(None).subject_identity();
        let stale = source_map_for(subject, stale_kir, stale_subject);
        assert!(matches!(
            try_build_bundle_for_target(Some(stale), "gfx942:xnack-"),
            Err(fe2o3_kernel_ir::SimulationBundleErrorV1::DebugMapBindingMismatch)
        ));
    }

    let mut substituted = bundle_with_map(false, false).into_canonical_bytes();
    *substituted.last_mut().unwrap() ^= 1;
    let bundle = directory.0.join("substituted-map.fe2sim");
    fs::write(&bundle, substituted).unwrap();
    let output = run_debug(&bundle, &request, "64", DISCOVER_AND_RESOLVE);
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["stage"], "input");
    assert_eq!(error["code"], "simulation_bundle_rejected");

    let verified = bundle_with_map(false, false);
    let bundle_path = directory.0.join("wrong-handoff-subject.fe2sim");
    fs::write(&bundle_path, verified.canonical_bytes()).unwrap();
    let admitted =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(&bundle_path, &request).unwrap();
    let (input, bundle) = admitted.into_parts();
    let mut reader = Cursor::new(Vec::<u8>::new());
    let mut writer = Vec::new();
    let wrong_subject = OpaqueIdentityV1::new([9; 32]).unwrap();
    let map_identity = OpaqueIdentityV1::new(bundle.debug_map_identity().unwrap()).unwrap();
    let error = run_admitted_jsonl_with_compiler_source_map_v1(
        input,
        DebugWaveWidthV1::Wave64,
        bundle.debug_map().unwrap(),
        wrong_subject,
        map_identity,
        &mut reader,
        &mut writer,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("not retained by the admitted bundle input")
    );
    assert!(writer.is_empty());
}

#[test]
fn compiler_bundle_runner_keeps_protocol_io_distinct_from_map_rejection() {
    let directory = TestDirectory::new();
    let request = workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json");
    let verified = bundle_with_map(false, false);
    let bundle_path = directory.0.join("closed-output.fe2sim");
    fs::write(&bundle_path, verified.canonical_bytes()).unwrap();
    let admitted =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(&bundle_path, &request).unwrap();
    let (input, bundle) = admitted.into_parts();
    let mut reader = Cursor::new(DISCOVER_AND_RESOLVE);
    let subject = OpaqueIdentityV1::new(*bundle.subject_identity()).unwrap();
    let map_identity = OpaqueIdentityV1::new(bundle.debug_map_identity().unwrap()).unwrap();
    let error = run_admitted_jsonl_with_compiler_source_map_v1(
        input,
        DebugWaveWidthV1::Wave64,
        bundle.debug_map().unwrap(),
        subject,
        map_identity,
        &mut reader,
        &mut ClosedWriter,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CompilerBundleDebugRunErrorV1::ProtocolStream(_)
    ));
}

#[test]
fn debug_bundle_and_raw_kir_are_mutually_exclusive() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["sim", "--bundle", "one.fe2sim", "--kir-v7", "two.kir"])
        .args(["--request", "request.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["stage"], "arguments");
    assert_eq!(error["code"], "invalid_command_line");
}
