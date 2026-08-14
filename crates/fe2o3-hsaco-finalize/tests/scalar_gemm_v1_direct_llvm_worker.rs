#![cfg(target_os = "linux")]

use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{CodeObjectVersion as CompilerCodeObjectVersion, CompilerModuleHandoffV2};
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    inspect_scalar_gemm_v1_worker_v2_hsaco_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use fe2o3_kernel_ir::SCALAR_GEMM_V1_KERNEL_ID;

const WORKER_ENV: &str = "FE2O3_SCALAR_GEMM_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_SCALAR_GEMM_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_SCALAR_GEMM_V1_LLVM_BUILD_ID";
const HANDOFF_ENV: &str = "FE2O3_SCALAR_GEMM_V1_HANDOFF";
const OUTPUT_ENV: &str = "FE2O3_SCALAR_GEMM_V1_OUTPUT";
const TARGET: &str = "gfx942:xnack-";

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required scalar GEMM native pin {name} is absent"))
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-scalar-gemm-v1-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create scalar GEMM handoff directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn canonical_handoff() -> CompilerModuleHandoffV2 {
    let path = PathBuf::from(required_env(HANDOFF_ENV));
    let bytes = fs::read(&path).expect("read rustc-produced scalar GEMM Worker V2 handoff");
    let handoff = CompilerModuleHandoffV2::decode(&bytes)
        .expect("strictly decode rustc-produced scalar GEMM Worker V2 handoff");
    assert_eq!(handoff.target().to_string(), TARGET);
    assert_eq!(handoff.code_object_version(), CompilerCodeObjectVersion::V6);
    assert_eq!(handoff.canonical_bytes(), bytes);
    handoff
}

fn consumed_handoff(directory: &TestDirectory) -> ConsumedCompilerModuleHandoffV1 {
    let producer = ProducerIdentity::from_codegen(
        "scalar_gemm_v1_direct_llvm_worker",
        Some(Path::new("tests/scalar_gemm_v1_direct_llvm_worker.rs")),
    )
    .expect("scalar GEMM test producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x47; 32]),
        BuildSession::from_bytes([0x94; 16]),
    )
    .expect("begin scalar GEMM handoff attempt");
    publish_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
        canonical_handoff().canonical_bytes(),
    )
    .expect("publish scalar GEMM handoff");
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume scalar GEMM handoff")
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "0"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed scalar GEMM option"))
    .collect()
}

fn produce_and_inspect(worker: &PinnedWorkerV1) -> Vec<u8> {
    let directory = TestDirectory::new();
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff(&directory),
        worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64).expect("bounded scalar GEMM output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("direct LLVM/LLD scalar GEMM production");
    let diagnostics = evidence.authorized().response().diagnostics().to_vec();
    let inspected = inspect_scalar_gemm_v1_worker_v2_hsaco_v1(evidence).unwrap_or_else(|error| {
        panic!("exact scalar GEMM Worker V2 inspection: {error:?}; diagnostics={diagnostics:?}")
    });

    assert_eq!(inspected.target().to_string(), TARGET);
    assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
    assert!(inspected.code_object_version_was_inspected());
    assert_eq!(
        inspected
            .raw_inspection()
            .policy()
            .expected_defined_symbols(),
        [SCALAR_GEMM_V1_KERNEL_ID, "scalar_gemm_v1.kd"]
    );
    let launch = inspected.raw_inspection().policy().launch();
    assert_eq!(launch.required_workgroup_size(), [256, 1, 1]);
    assert_eq!(launch.max_flat_workgroup_size(), 256);
    assert_eq!(launch.wavefront_size(), 64);
    assert!(!inspected.authenticates_frontend_origin());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());

    let bytes = inspected.exact_bytes().to_vec();
    let output_identity = ContentIdentityV1::calculate(&bytes);
    assert_eq!(
        inspected.exchange().linked_output_identity(),
        output_identity
    );
    assert_ne!(
        inspected
            .exchange()
            .embedded_frontend_authority_commitment(),
        &[0; 32]
    );
    assert!(output_identity.matches(&bytes));
    for forbidden in [b"amd_comgr".as_slice(), b"libamd_comgr".as_slice()] {
        assert!(
            !bytes
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "scalar GEMM HSACO contains forbidden COMGR reference"
        );
    }
    bytes
}

#[test]
#[ignore = "requires the measured upstream LLVM/LLD worker built for gfx942"]
fn real_worker_produces_deterministic_inspected_scalar_gemm_v1_cov6_hsaco() {
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let worker_bytes = fs::read(&worker_path).expect("read scalar GEMM worker executable");
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&worker_bytes),
        required_env(WORKER_BUILD_ID_ENV),
        required_env(LLVM_BUILD_ID_ENV),
    )
    .expect("exact scalar GEMM worker measurement");
    let worker =
        PinnedWorkerV1::open(&worker_path, measurement).expect("open measured scalar GEMM worker");

    let first = produce_and_inspect(&worker);
    let second = produce_and_inspect(&worker);
    assert_eq!(first, second, "repeated scalar GEMM links changed bytes");

    let output = PathBuf::from(required_env(OUTPUT_ENV));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .expect("create fresh scalar GEMM HSACO output");
    file.write_all(&first).expect("write scalar GEMM HSACO");
    file.sync_all().expect("sync scalar GEMM HSACO");
}
