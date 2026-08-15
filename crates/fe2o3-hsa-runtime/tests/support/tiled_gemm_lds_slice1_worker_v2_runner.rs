use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedLdsGemmSlice1HostAdapterV1, ObservedContext, join_exact_lds_gemm_slice1_v1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    finalize_exact_lds_gemm_compiler_import_v1,
};

const TILE: usize = 16;
const ELEMENTS: usize = TILE * TILE;
const GUARD_ELEMENTS: usize = 32;
const A_PREFIX: u16 = 0x7fc1;
const A_SUFFIX: u16 = 0x7fc2;
const B_PREFIX: u16 = 0x7fd1;
const B_SUFFIX: u16 = 0x7fd2;
const C_PREFIX: f32 = f32::from_bits(0x7fc0_c001);
const C_SUFFIX: f32 = f32::from_bits(0x7fc0_c002);
const C_POISON: f32 = f32::from_bits(0x7fc0_c0ff);
const SUCCESS_MARKER: &str = "FE2O3_PROTECTED_SLICE1_WORKER_V2_OK";

type BoxError = Box<dyn std::error::Error>;

#[allow(dead_code, unused_imports)]
mod registry_fixture {
    include!(concat!(
        env!("FE2O3_WORKSPACE_ROOT"),
        "/crates/fe2o3-hsaco-finalize/tests/lds_gemm_profile_registry.rs"
    ));

    pub(super) fn exact_import_and_handoff() -> (
        fe2o3_hsaco_finalize::InspectedExactLdsGemmCompilerImportV1,
        fe2o3_compiler_ffi::CompilerModuleHandoffV2,
    ) {
        let fixture = Slice1Fixture::canonical();
        let handoff = fixture.handoff();
        let import = fe2o3_hsaco_finalize::inspect_exact_lds_gemm_compiler_import_v1(
            fixture.pins(),
            fixture.handoff(),
        )
        .expect("canonical exact Slice1 compiler import");
        (import, handoff)
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Result<Self, BoxError> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-protected-slice1-handoff-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn required_env(variable: &str) -> Result<String, BoxError> {
    env::var(variable)
        .map_err(|_| format!("missing required environment variable {variable}").into())
}

fn measured_worker() -> Result<PinnedWorkerV1, BoxError> {
    let worker_path = PathBuf::from(required_env("FE2O3_LDS_GEMM_V1_WORKER")?);
    let worker_bytes = fs::read(&worker_path)?;
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&worker_bytes),
        required_env("FE2O3_LDS_GEMM_V1_WORKER_BUILD_ID")?,
        required_env("FE2O3_LDS_GEMM_V1_LLVM_BUILD_ID")?,
    )?;
    Ok(PinnedWorkerV1::open(worker_path, measurement)?)
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &fe2o3_compiler_ffi::CompilerModuleHandoffV2,
) -> Result<fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV1, BoxError> {
    let producer = ProducerIdentity::from_codegen(
        "protected_slice1_worker_v2_hardware",
        Some(Path::new(
            "tests/support/tiled_gemm_lds_slice1_worker_v2_runner.rs",
        )),
    )?;
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x97; 32]),
        BuildSession::from_bytes([0x10; 16]),
    )?;
    publish_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
        handoff.canonical_bytes(),
    )?;
    Ok(consume_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
    )?)
}

fn bf16_bits(value: f32) -> u16 {
    (value.to_bits() >> 16) as u16
}

fn bf16_value(bits: u16) -> f32 {
    f32::from_bits(u32::from(bits) << 16)
}

fn inputs() -> (Vec<u16>, Vec<u16>) {
    let mut a = Vec::with_capacity(ELEMENTS);
    let mut b = Vec::with_capacity(ELEMENTS);
    for row in 0..TILE {
        for depth in 0..TILE {
            let value = ((row * 3 + depth * 5) % 7) as i32 - 3;
            a.push(bf16_bits(value as f32));
        }
    }
    for depth in 0..TILE {
        for column in 0..TILE {
            let value = ((depth * 2 + column * 3) % 5) as i32 - 2;
            b.push(bf16_bits(value as f32));
        }
    }
    (a, b)
}

fn cpu_reference(a: &[u16], b: &[u16]) -> Vec<f32> {
    let mut output = vec![0.0; ELEMENTS];
    for row in 0..TILE {
        for column in 0..TILE {
            let mut sum = 0.0;
            for depth in 0..TILE {
                sum += bf16_value(a[row * TILE + depth]) * bf16_value(b[depth * TILE + column]);
            }
            output[row * TILE + column] = sum;
        }
    }
    output
}

fn guarded<T: Copy>(body: &[T], prefix: T, suffix: T) -> Vec<T> {
    let mut allocation = vec![prefix; GUARD_ELEMENTS];
    allocation.extend_from_slice(body);
    allocation.resize(GUARD_ELEMENTS + ELEMENTS + GUARD_ELEMENTS, suffix);
    allocation
}

fn verify_u16_allocation(
    role: &str,
    actual: &[u16],
    body: &[u16],
    prefix: u16,
    suffix: u16,
) -> Result<(), BoxError> {
    require(
        actual.len() == GUARD_ELEMENTS + ELEMENTS + GUARD_ELEMENTS,
        format!("{role} allocation length changed"),
    )?;
    require(
        actual[..GUARD_ELEMENTS]
            .iter()
            .all(|value| *value == prefix),
        format!("{role} prefix guard changed"),
    )?;
    require(
        actual[GUARD_ELEMENTS..GUARD_ELEMENTS + ELEMENTS] == *body,
        format!("{role} input body changed"),
    )?;
    require(
        actual[GUARD_ELEMENTS + ELEMENTS..]
            .iter()
            .all(|value| *value == suffix),
        format!("{role} suffix guard changed"),
    )
}

fn verify_f32_allocation(actual: &[f32], expected: &[f32]) -> Result<f32, BoxError> {
    require(
        actual.len() == GUARD_ELEMENTS + ELEMENTS + GUARD_ELEMENTS,
        "C allocation length changed",
    )?;
    require(
        actual[..GUARD_ELEMENTS]
            .iter()
            .all(|value| value.to_bits() == C_PREFIX.to_bits()),
        "C prefix guard changed",
    )?;
    require(
        actual[GUARD_ELEMENTS + ELEMENTS..]
            .iter()
            .all(|value| value.to_bits() == C_SUFFIX.to_bits()),
        "C suffix guard changed",
    )?;
    let mut max_abs_error = 0.0_f32;
    for (index, (actual, expected)) in actual[GUARD_ELEMENTS..GUARD_ELEMENTS + ELEMENTS]
        .iter()
        .zip(expected)
        .enumerate()
    {
        let error = (*actual - *expected).abs();
        max_abs_error = max_abs_error.max(error);
        require(
            actual.to_bits() == expected.to_bits(),
            format!(
                "C[{index}] differs: expected {expected} ({:#010x}), got {actual} ({:#010x})",
                expected.to_bits(),
                actual.to_bits()
            ),
        )?;
    }
    Ok(max_abs_error)
}

fn run() -> Result<(), BoxError> {
    let worker = measured_worker()?;
    let transaction = TestDirectory::new()?;
    let (compiler_import, handoff) = registry_fixture::exact_import_and_handoff();

    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    let stream = context.create_stream()?;
    let (a_body, b_body) = inputs();
    let expected = cpu_reference(&a_body, &b_body);
    let a_initial = guarded(&a_body, A_PREFIX, A_SUFFIX);
    let b_initial = guarded(&b_body, B_PREFIX, B_SUFFIX);
    let c_initial = guarded(&[C_POISON; ELEMENTS], C_PREFIX, C_SUFFIX);
    let a = DeviceBuffer::from_host(&stream, &a_initial)?;
    let b = DeviceBuffer::from_host(&stream, &b_initial)?;
    let mut c = DeviceBuffer::from_host(&stream, &c_initial)?;

    let body = GUARD_ELEMENTS..GUARD_ELEMENTS + ELEMENTS;
    let host = GeneratedLdsGemmSlice1HostAdapterV1::prepare(
        &observed,
        &compiler_import,
        a.view(body.clone())?,
        b.view(body.clone())?,
        c.view_mut(body.clone())?,
    )?;
    let artifact = finalize_exact_lds_gemm_compiler_import_v1(
        compiler_import,
        consumed_handoff(&transaction, &handoff)?,
        &worker,
        WorkerExecutionLimitsV1::default(),
    )?;
    let finalizer_identity = artifact.identity();
    let import_identity = artifact.import_identity();
    let profile_identity = artifact.profile_identity();

    let joined = join_exact_lds_gemm_slice1_v1(artifact, host)?;
    require(
        joined.finalizer_identity() == finalizer_identity
            && joined.import_identity() == import_identity
            && joined.profile_identity() == profile_identity,
        "protected join receipt identities changed",
    )?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    let completed = joined.load(adapter)?.dispatch_and_wait()?;
    require(
        completed.finalizer_identity() == finalizer_identity
            && completed.import_identity() == import_identity
            && completed.profile_identity() == profile_identity,
        "protected completion receipt identities changed",
    )?;
    let unloaded = completed.unload();
    require(
        unloaded.finalizer_identity() == finalizer_identity
            && unloaded.import_identity() == import_identity
            && unloaded.profile_identity() == profile_identity
            && unloaded.unload_identity().as_bytes() != &[0; 32],
        "protected unload receipt identities changed",
    )?;

    let a_after = a.to_host_vec(&stream)?;
    let b_after = b.to_host_vec(&stream)?;
    let c_after = c.to_host_vec(&stream)?;
    verify_u16_allocation("A", &a_after, &a_body, A_PREFIX, A_SUFFIX)?;
    verify_u16_allocation("B", &b_after, &b_body, B_PREFIX, B_SUFFIX)?;
    let max_abs_error = verify_f32_allocation(&c_after, &expected)?;
    println!(
        "{SUCCESS_MARKER} outputs={ELEMENTS} max_abs_error={max_abs_error} finalizer={} unload={}",
        hex(finalizer_identity.as_bytes()),
        hex(unloaded.unload_identity().as_bytes())
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() {
    if let Err(error) = run() {
        eprintln!("protected Slice1 Worker V2 route failed: {error}");
        std::process::exit(1);
    }
}
