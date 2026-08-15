#![cfg(target_os = "linux")]

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, Wave64CollectivesV1CompilerPinsV1,
    Wave64CollectivesV1DirectWorkerExpectationV1, Wave64CollectivesV1DirectWorkerPinsV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    construct_inert_wave64_collectives_v1_compiler_handoff_v1,
    execute_reproducible_first_build_worker_v2, finalize_wave64_collectives_v1_worker_v2_hsaco_v1,
    inspect_wave64_collectives_v1_worker_v2_hsaco_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;

const WORKER_ENV: &str = "FE2O3_WAVE64_COLLECTIVES_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_WAVE64_COLLECTIVES_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_WAVE64_COLLECTIVES_V1_LLVM_BUILD_ID";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-wave64-collectives-v1-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create Wave64 handoff directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required Wave64 worker pin {name} is absent"))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
) -> ConsumedCompilerModuleHandoffV1 {
    let producer = ProducerIdentity::from_codegen(
        "wave64_collectives_v1_direct_llvm_worker",
        Some(Path::new(
            "tests/wave64_collectives_v1_direct_llvm_worker.rs",
        )),
    )
    .expect("Wave64 test producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x64; 32]),
        BuildSession::from_bytes([0x94; 16]),
    )
    .expect("begin Wave64 handoff attempt");
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .expect("publish Wave64 handoff");
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume Wave64 handoff")
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed Wave64 option"))
    .collect()
}

fn produce(
    worker: &PinnedWorkerV1,
    handoff: &CompilerModuleHandoffV2,
    expectation: Wave64CollectivesV1DirectWorkerExpectationV1,
) -> ([u8; 32], ContentIdentityV1, ContentIdentityV1) {
    let directory = TestDirectory::new();
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff(&directory, handoff),
        worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64).expect("bounded Wave64 output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("direct LLVM/LLD Wave64 production");
    let diagnostics = evidence.authorized().response().diagnostics().to_vec();
    let inspected = inspect_wave64_collectives_v1_worker_v2_hsaco_v1(evidence, expectation)
        .unwrap_or_else(|error| {
            panic!("exact Wave64 Worker V2 inspection: {error:?}; diagnostics={diagnostics:?}")
        });
    assert_eq!(inspected.target().to_string(), "gfx942:xnack-");
    assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
    assert!(!inspected.authenticates_compiler_origin());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());

    let finalized = finalize_wave64_collectives_v1_worker_v2_hsaco_v1(inspected)
        .expect("opaque canonical Wave64 finalization");
    assert!(finalized.exact_profile_descriptor_source_was_checked());
    assert!(finalized.exact_five_argument_abi_was_checked());
    assert!(finalized.direct_upstream_llvm_worker_exchange_was_checked());
    assert!(!finalized.proves_compiler_refinement());
    assert!(!finalized.proves_functional_collectives());
    assert!(!finalized.proves_no_comgr_linkage());
    assert!(!finalized.grants_publication_authority());
    assert!(!finalized.grants_load_authority());
    assert!(!finalized.grants_launch_authority());
    (
        *finalized.identity().as_bytes(),
        finalized.raw_output_identity(),
        finalized.finalized_output_identity(),
    )
}

#[test]
#[ignore = "requires the measured direct LLVM/LLD worker built for gfx942"]
fn real_worker_produces_reproducible_opaque_wave64_collectives_v1_cov6_admission() {
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let worker_bytes = fs::read(&worker_path).expect("read Wave64 worker executable");
    let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
    let worker_build_identity = required_env(WORKER_BUILD_ID_ENV);
    let llvm_build_identity = required_env(LLVM_BUILD_ID_ENV);
    let measurement = WorkerMeasurementV1::new(
        worker_identity,
        worker_build_identity.clone(),
        llvm_build_identity.clone(),
    )
    .expect("exact Wave64 worker measurement");
    let worker =
        PinnedWorkerV1::open(&worker_path, measurement).expect("open measured Wave64 worker");

    let compiler_pins = Wave64CollectivesV1CompilerPinsV1::new([0xa5; 32])
        .expect("non-authoritative Wave64 compiler pin");
    let handoff = construct_inert_wave64_collectives_v1_compiler_handoff_v1(compiler_pins)
        .expect("construct exact inert Wave64 compiler handoff");
    let direct_worker_pins = Wave64CollectivesV1DirectWorkerPinsV1::new(
        worker_identity,
        &worker_build_identity,
        &llvm_build_identity,
    )
    .expect("bind direct LLVM worker pins");
    let expectation = Wave64CollectivesV1DirectWorkerExpectationV1::from_pinned_handoff(
        &handoff,
        *handoff.identity().sha256(),
        compiler_pins,
        direct_worker_pins,
    )
    .expect("bind exact Wave64 handoff expectation");

    eprintln!("worker_sha256={}", hex(worker_identity.sha256()));
    eprintln!("worker_build_identity={worker_build_identity}");
    eprintln!("llvm_build_identity={llvm_build_identity}");
    eprintln!("handoff_sha256={}", hex(handoff.identity().sha256()));

    let first = produce(&worker, &handoff, expectation);
    let second = produce(&worker, &handoff, expectation);
    assert_eq!(first, second, "repeated Wave64 production changed identity");
    eprintln!("admission_sha256={}", hex(&first.0));
    eprintln!("raw_output_sha256={}", hex(first.1.sha256()));
    eprintln!("finalized_output_sha256={}", hex(first.2.sha256()));
}
