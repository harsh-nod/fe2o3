#![cfg(target_os = "linux")]

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedWave64CollectivesV1HostAdapterV1, ObservedContext, join_wave64_collectives_v1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, PreparedFinalizedWave64CollectivesV1HsacoV1,
    Wave64CollectivesV1CompilerPinsV1, Wave64CollectivesV1DirectWorkerExpectationV1,
    Wave64CollectivesV1DirectWorkerPinsV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, construct_inert_wave64_collectives_v1_compiler_handoff_v1,
    execute_reproducible_first_build_worker_v2, finalize_wave64_collectives_v1_worker_v2_hsaco_v1,
    inspect_wave64_collectives_v1_worker_v2_hsaco_v1,
};
use fe2o3_wave64_collectives_v1::{
    WAVE64_LANES_V1, compare_wave64_collectives_v1, wave64_collectives_oracle_v1,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const OPT_IN: &str = "FE2O3_RUN_GFX942_WAVE64_COLLECTIVES_V1_HARDWARE";
const WORKER: &str = "FE2O3_WAVE64_COLLECTIVES_V1_WORKER";
const WORKER_SHA256: &str = "FE2O3_WAVE64_COLLECTIVES_V1_WORKER_SHA256";
const WORKER_BUILD_ID: &str = "FE2O3_WAVE64_COLLECTIVES_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID: &str = "FE2O3_WAVE64_COLLECTIVES_V1_LLVM_BUILD_ID";
const SOURCE_AUTHORITY: &str = "FE2O3_WAVE64_COLLECTIVES_V1_SOURCE_AUTHORITY_SHA256";
const HANDOFF_SHA256: &str = "FE2O3_WAVE64_COLLECTIVES_V1_HANDOFF_SHA256";
const FINALIZED_OUTPUT_SHA256: &str = "FE2O3_WAVE64_COLLECTIVES_V1_FINALIZED_OUTPUT_SHA256";
const ADMISSION_SHA256: &str = "FE2O3_WAVE64_COLLECTIVES_V1_ADMISSION_SHA256";
const SUCCESS_MARKER: &str = "FE2O3_PROTECTED_WAVE64_COLLECTIVES_V1_GFX942_OK";
const GUARD: usize = 16;
const INPUT_PREFIX: f32 = f32::from_bits(0x7fc1_0001);
const INPUT_SUFFIX: f32 = f32::from_bits(0x7fc1_0002);
const REDUCTION_PREFIX: f32 = f32::from_bits(0x7fc2_0001);
const REDUCTION_SUFFIX: f32 = f32::from_bits(0x7fc2_0002);
const INCLUSIVE_PREFIX: f32 = f32::from_bits(0x7fc3_0001);
const INCLUSIVE_SUFFIX: f32 = f32::from_bits(0x7fc3_0002);
const EXCLUSIVE_PREFIX: f32 = f32::from_bits(0x7fc4_0001);
const EXCLUSIVE_SUFFIX: f32 = f32::from_bits(0x7fc4_0002);
const OUTPUT_POISON: f32 = f32::from_bits(0x7fcf_00ff);

type BoxError = Box<dyn std::error::Error>;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Result<Self, BoxError> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-wave64-hardware-{}-{}",
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

fn required_env(name: &str) -> Result<String, BoxError> {
    env::var(name).map_err(|_| format!("missing required protected-harness pin {name}").into())
}

fn decode_sha256(name: &str) -> Result<[u8; 32], BoxError> {
    let encoded = required_env(name)?;
    require(
        encoded.len() == 64,
        format!("{name} must contain 64 lowercase hex digits"),
    )?;
    let mut bytes = [0_u8; 32];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        require(
            pair.iter().all(u8::is_ascii_hexdigit),
            format!("{name} is not hexadecimal"),
        )?;
        let text = std::str::from_utf8(pair)?;
        bytes[index] = u8::from_str_radix(text, 16)?;
    }
    require(bytes != [0; 32], format!("{name} must be nonzero"))?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require_environment() -> Result<(), BoxError> {
    require(
        env::var(OPT_IN).as_deref() == Ok("1"),
        format!("set {OPT_IN}=1 to opt in"),
    )?;
    for (name, expected) in [
        ("HSA_XNACK", "0"),
        ("HIP_VISIBLE_DEVICES", "0"),
        ("ROCR_VISIBLE_DEVICES", "0"),
    ] {
        require(
            env::var(name).as_deref() == Ok(expected),
            format!("set {name}={expected}"),
        )?;
    }
    for forbidden in ["FE2O3_LLC", "FE2O3_LLD", "FE2O3_COMGR"] {
        require(
            env::var_os(forbidden).is_none(),
            format!("unset forbidden path {forbidden}"),
        )?;
    }
    for name in [WORKER, WORKER_BUILD_ID, LLVM_BUILD_ID] {
        let value = required_env(name)?;
        require(!value.trim().is_empty(), format!("{name} must be nonempty"))?;
    }
    for name in [
        WORKER_SHA256,
        SOURCE_AUTHORITY,
        HANDOFF_SHA256,
        FINALIZED_OUTPUT_SHA256,
        ADMISSION_SHA256,
    ] {
        decode_sha256(name)?;
    }
    Ok(())
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
) -> Result<ConsumedCompilerModuleHandoffV1, BoxError> {
    let producer = ProducerIdentity::from_codegen(
        "wave64_collectives_v1_protected_hardware",
        Some(Path::new("tests/gfx942_hardware.rs")),
    )?;
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x64; 32]),
        BuildSession::from_bytes([0x94; 16]),
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

fn link_options() -> Result<Vec<LinkOptionV1>, BoxError> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| Ok(LinkOptionV1::new(name, value)?))
    .collect()
}

struct PinnedBuild {
    worker: PinnedWorkerV1,
    handoff: CompilerModuleHandoffV2,
    expectation: Wave64CollectivesV1DirectWorkerExpectationV1,
    expected_admission: [u8; 32],
    expected_finalized_output: [u8; 32],
}

fn pinned_build() -> Result<PinnedBuild, BoxError> {
    let worker_path = PathBuf::from(required_env(WORKER)?);
    let worker_bytes = fs::read(&worker_path)?;
    let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
    require(
        worker_identity.sha256() == &decode_sha256(WORKER_SHA256)?,
        "worker executable SHA-256 differs from pin",
    )?;
    let worker_build = required_env(WORKER_BUILD_ID)?;
    let llvm_build = required_env(LLVM_BUILD_ID)?;
    let measurement = WorkerMeasurementV1::new(worker_identity, &worker_build, &llvm_build)?;
    let worker = PinnedWorkerV1::open(worker_path, measurement)?;
    let compiler_pins = Wave64CollectivesV1CompilerPinsV1::new(decode_sha256(SOURCE_AUTHORITY)?)?;
    let handoff = construct_inert_wave64_collectives_v1_compiler_handoff_v1(compiler_pins)?;
    require(
        handoff.identity().sha256() == &decode_sha256(HANDOFF_SHA256)?,
        "compiler handoff SHA-256 differs from pin",
    )?;
    let worker_pins =
        Wave64CollectivesV1DirectWorkerPinsV1::new(worker_identity, &worker_build, &llvm_build)?;
    let expectation = Wave64CollectivesV1DirectWorkerExpectationV1::from_pinned_handoff(
        &handoff,
        *handoff.identity().sha256(),
        compiler_pins,
        worker_pins,
    )?;
    Ok(PinnedBuild {
        worker,
        handoff,
        expectation,
        expected_admission: decode_sha256(ADMISSION_SHA256)?,
        expected_finalized_output: decode_sha256(FINALIZED_OUTPUT_SHA256)?,
    })
}

fn produce(build: &PinnedBuild) -> Result<PreparedFinalizedWave64CollectivesV1HsacoV1, BoxError> {
    let directory = TestDirectory::new()?;
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff(&directory, &build.handoff)?,
        &build.worker,
        Vec::new(),
        link_options()?,
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)?,
        WorkerExecutionLimitsV1::default(),
    )?;
    let inspected = inspect_wave64_collectives_v1_worker_v2_hsaco_v1(evidence, build.expectation)?;
    let finalized = finalize_wave64_collectives_v1_worker_v2_hsaco_v1(inspected)?;
    require(
        finalized.identity().as_bytes() == &build.expected_admission,
        "finalizer admission identity differs from pin",
    )?;
    require(
        finalized.finalized_output_identity().sha256() == &build.expected_finalized_output,
        "finalized output identity differs from pin",
    )?;
    Ok(finalized)
}

fn input_body() -> [f32; WAVE64_LANES_V1] {
    core::array::from_fn(|lane| ((lane * 13 + 5) % 31) as f32 - 15.0)
}

fn guarded(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut values = vec![prefix; GUARD];
    values.extend_from_slice(body);
    values.resize(GUARD + WAVE64_LANES_V1 + GUARD, suffix);
    values
}

fn verify_allocation(
    role: &str,
    actual: &[f32],
    expected: &[f32; WAVE64_LANES_V1],
    prefix: f32,
    suffix: f32,
) -> Result<(), BoxError> {
    require(
        actual.len() == GUARD + WAVE64_LANES_V1 + GUARD,
        format!("{role} allocation length changed"),
    )?;
    require(
        actual[..GUARD]
            .iter()
            .all(|value| value.to_bits() == prefix.to_bits()),
        format!("{role} prefix canary changed"),
    )?;
    require(
        actual[GUARD + WAVE64_LANES_V1..]
            .iter()
            .all(|value| value.to_bits() == suffix.to_bits()),
        format!("{role} suffix canary changed"),
    )?;
    for (lane, (actual, expected)) in actual[GUARD..GUARD + WAVE64_LANES_V1]
        .iter()
        .zip(expected)
        .enumerate()
    {
        require(
            actual.to_bits() == expected.to_bits(),
            format!(
                "{role}[{lane}] differs: expected {:#010x}, got {:#010x}",
                expected.to_bits(),
                actual.to_bits()
            ),
        )?;
    }
    Ok(())
}

fn run_mask(
    build: &PinnedBuild,
    context: &std::sync::Arc<GpuContext>,
    observed: &ObservedContext,
    mask: u64,
) -> Result<[u8; 32], BoxError> {
    let stream = context.create_stream()?;
    let input_body = input_body();
    let mut expected_reduction = [0.0; WAVE64_LANES_V1];
    let mut expected_inclusive = [0.0; WAVE64_LANES_V1];
    let mut expected_exclusive = [0.0; WAVE64_LANES_V1];
    wave64_collectives_oracle_v1(
        &input_body,
        mask,
        &mut expected_reduction,
        &mut expected_inclusive,
        &mut expected_exclusive,
    )?;

    let input_initial = guarded(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
    let output_initial = [OUTPUT_POISON; WAVE64_LANES_V1];
    let input = DeviceBuffer::from_host(&stream, &input_initial)?;
    let mut reduction = DeviceBuffer::from_host(
        &stream,
        &guarded(&output_initial, REDUCTION_PREFIX, REDUCTION_SUFFIX),
    )?;
    let mut inclusive = DeviceBuffer::from_host(
        &stream,
        &guarded(&output_initial, INCLUSIVE_PREFIX, INCLUSIVE_SUFFIX),
    )?;
    let mut exclusive = DeviceBuffer::from_host(
        &stream,
        &guarded(&output_initial, EXCLUSIVE_PREFIX, EXCLUSIVE_SUFFIX),
    )?;
    let body = GUARD..GUARD + WAVE64_LANES_V1;
    let host = GeneratedWave64CollectivesV1HostAdapterV1::prepare(
        observed,
        input.view(body.clone())?,
        mask,
        reduction.view_mut(body.clone())?,
        inclusive.view_mut(body.clone())?,
        exclusive.view_mut(body.clone())?,
    )?;
    let joined = join_wave64_collectives_v1(produce(build)?, host)?;
    let lineage = joined.lineage();
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    let completed = joined.load(adapter)?.dispatch_and_wait()?;
    require(completed.lineage() == lineage, "completion lineage changed")?;
    let unloaded = completed.unload();
    require(unloaded.lineage() == lineage, "unload lineage changed")?;
    require(
        unloaded.unload_identity().as_bytes() != &[0; 32],
        "zero unload identity",
    )?;
    require(
        !unloaded.proves_functional_collectives(),
        "runtime receipt widened functional authority",
    )?;

    let input_after = input.to_host_vec(&stream)?;
    let reduction_after = reduction.to_host_vec(&stream)?;
    let inclusive_after = inclusive.to_host_vec(&stream)?;
    let exclusive_after = exclusive.to_host_vec(&stream)?;
    verify_allocation(
        "input",
        &input_after,
        &input_body,
        INPUT_PREFIX,
        INPUT_SUFFIX,
    )?;
    verify_allocation(
        "reduction",
        &reduction_after,
        &expected_reduction,
        REDUCTION_PREFIX,
        REDUCTION_SUFFIX,
    )?;
    verify_allocation(
        "inclusive",
        &inclusive_after,
        &expected_inclusive,
        INCLUSIVE_PREFIX,
        INCLUSIVE_SUFFIX,
    )?;
    verify_allocation(
        "exclusive",
        &exclusive_after,
        &expected_exclusive,
        EXCLUSIVE_PREFIX,
        EXCLUSIVE_SUFFIX,
    )?;
    compare_wave64_collectives_v1(
        &input_body,
        mask,
        &reduction_after[body.clone()],
        &inclusive_after[body.clone()],
        &exclusive_after[body],
    )?;
    Ok(*unloaded.unload_identity().as_bytes())
}

/// Protected exact-profile execution on one visible gfx942:xnack- device.
///
/// The test fails closed unless every compiler/Worker/finalizer identity and
/// device-selection variable is supplied. The worker is the measured direct
/// upstream LLVM/LLD API worker; shell `llc`, shell linkers, COMGR, CUDA, and
/// HIP compiler paths are rejected.
#[test]
#[ignore = "requires all exact build pins and one isolated MI300X gfx942:xnack- device"]
fn gfx942_masked_wave64_collectives_v1_protected_hardware() -> Result<(), BoxError> {
    require_environment()?;
    let build = pinned_build()?;
    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    let masks = [
        u64::MAX,
        0,
        0xaaaa_aaaa_aaaa_aaaa,
        (1_u64 << 0) | (1_u64 << 1) | (1_u64 << 31) | (1_u64 << 32) | (1_u64 << 62) | (1_u64 << 63),
    ];
    let mut last_unload = [0; 32];
    for mask in masks {
        last_unload = run_mask(&build, &context, &observed, mask)?;
    }
    println!(
        "{SUCCESS_MARKER} masks={} lanes={} admission={} finalized_output={} last_unload={}",
        masks.len(),
        WAVE64_LANES_V1,
        hex(&build.expected_admission),
        hex(&build.expected_finalized_output),
        hex(&last_unload),
    );
    Ok(())
}
