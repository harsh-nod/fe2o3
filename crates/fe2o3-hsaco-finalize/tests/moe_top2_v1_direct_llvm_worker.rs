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
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, MoeTop2V1DirectWorkerExpectationV1,
    MoeTop2V1DirectWorkerPinsV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    finalize_moe_top2_v1_worker_v2_hsaco_v1, inspect_moe_top2_v1_worker_v2_hsaco_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;

const MODULE_ENV: &str = "FE2O3_MOE_TOP2_V1_COMPILER_MODULE";
const WORKER_ENV: &str = "FE2O3_MOE_TOP2_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_MOE_TOP2_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_MOE_TOP2_V1_LLVM_BUILD_ID";
const KERNEL: &str = "moe_top2_route_f32_t8_e4_k2_c4_v1";
const DESCRIPTOR: &str = "moe_top2_route_f32_t8_e4_k2_c4_v1.kd";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-moe-top2-v1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create exact MoE handoff directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required exact MoE pin {name} is absent"))
}

fn exact_handoff() -> CompilerModuleHandoffV2 {
    let module = fs::read(required_env(MODULE_ENV)).expect("read compiler-produced MoE LLVM");
    let target = DeviceTargetV1::parse("gfx942:xnack-").expect("fixed target");
    let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
        target,
        fe2o3_compiler_ffi::CodeObjectVersion::V6,
    )
    .expect("exact no-FFI envelope");
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, KERNEL),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, DESCRIPTOR),
    ])
    .expect("exact MoE manifest");
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        fe2o3_compiler_ffi::CodeObjectVersion::V6,
        envelope,
        manifest,
        &module,
    )
    .expect("exact compiler-produced MoE handoff")
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
) -> ConsumedCompilerModuleHandoffV1 {
    let producer = ProducerIdentity::from_codegen(
        "moe_top2_v1",
        Some(Path::new("tests/moe_top2_v1_direct_llvm_worker.rs")),
    )
    .expect("exact MoE test producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x5e; 32]),
        BuildSession::from_bytes([0x94; 16]),
    )
    .expect("begin exact MoE handoff attempt");
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .expect("publish exact MoE handoff");
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume exact MoE handoff")
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed exact MoE option"))
    .collect()
}

fn produce(
    worker: &PinnedWorkerV1,
    handoff: &CompilerModuleHandoffV2,
    expectation: MoeTop2V1DirectWorkerExpectationV1,
) -> ([u8; 32], ContentIdentityV1, ContentIdentityV1) {
    let directory = TestDirectory::new();
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff(&directory, handoff),
        worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64).expect("bounded exact MoE output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("direct upstream LLVM/LLD exact MoE production");
    let diagnostics = evidence.authorized().response().diagnostics().to_vec();
    let inspected = inspect_moe_top2_v1_worker_v2_hsaco_v1(evidence, expectation)
        .unwrap_or_else(|error| panic!("exact MoE inspection: {error:?}; {diagnostics:?}"));
    assert_eq!(inspected.target().to_string(), "gfx942:xnack-");
    assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
    let finalized = finalize_moe_top2_v1_worker_v2_hsaco_v1(inspected)
        .expect("opaque exact MoE descriptor finalization");
    assert!(finalized.exact_source_kir_compiler_profile_was_checked());
    assert!(finalized.direct_upstream_llvm_lld_exchange_was_checked());
    assert!(!finalized.authenticates_compiler_origin());
    assert!(!finalized.proves_source_refinement());
    assert!(!finalized.proves_compiler_refinement());
    assert!(!finalized.proves_machine_refinement());
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
#[ignore = "requires the compiler-produced module and measured direct LLVM/LLD worker"]
fn real_worker_produces_reproducible_opaque_exact_moe_top2_v1_cov6_admission() {
    let handoff = exact_handoff();
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let worker_bytes = fs::read(&worker_path).expect("read exact MoE worker");
    let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
    let worker_build_identity = required_env(WORKER_BUILD_ID_ENV);
    let llvm_build_identity = required_env(LLVM_BUILD_ID_ENV);
    let worker_pins = MoeTop2V1DirectWorkerPinsV1::new(
        worker_identity,
        &worker_build_identity,
        &llvm_build_identity,
    )
    .expect("bind exact MoE worker pins");
    let expectation = MoeTop2V1DirectWorkerExpectationV1::from_exact_compiler_handoff(
        &handoff,
        *handoff.identity().sha256(),
        worker_pins,
    )
    .expect("bind compiler-produced exact MoE handoff");
    let measurement = WorkerMeasurementV1::new(
        worker_identity,
        worker_build_identity.clone(),
        llvm_build_identity.clone(),
    )
    .expect("exact MoE worker measurement");
    let worker =
        PinnedWorkerV1::open(&worker_path, measurement).expect("open measured exact MoE worker");
    let first = produce(&worker, &handoff, expectation);
    let second = produce(&worker, &handoff, expectation);
    assert_eq!(first, second, "repeated exact MoE production changed");
    eprintln!(
        "handoff_sha256={} worker_sha256={} worker_build_identity={} llvm_build_identity={} admission_sha256={} raw_output_sha256={} finalized_output_sha256={}",
        hex(handoff.identity().sha256()),
        hex(worker_identity.sha256()),
        worker_build_identity,
        llvm_build_identity,
        hex(&first.0),
        hex(first.1.sha256()),
        hex(first.2.sha256()),
    );
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
