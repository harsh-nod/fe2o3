use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_core::GpuContext;
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkOptionV1,
    PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerInputKindV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    execute_reproducible_first_build_worker_v2,
};
use fe2o3_pliron_scalar_add_v1::{
    CanonicalSourceObservationV1, RuntimeEvidenceV1, canonical_prepared_scalar_add_v1,
    canonical_source_observation_v1, execute_repository_scalar_add_v1_on_mi300x,
    finalize_repository_scalar_add_v1, repository_profile_v1,
};
use fe2o3_pliron_worker_v2::construct_scalar_add_worker_request_v2;

const RUN_ENV: &str = "FE2O3_RUN_REPOSITORY_SCALAR_ADD_V1_MI300X";
const WORKER_ENV: &str = "FE2O3_PLIRON_SCALAR_ADD_V1_WORKER";
const WORKER_BUILD_MANIFEST_ENV: &str = "FE2O3_PLIRON_SCALAR_ADD_V1_OBSERVED_WORKER_BUILD_ID_FILE";
const LLVM_BUILD_MANIFEST_ENV: &str = "FE2O3_PLIRON_SCALAR_ADD_V1_OBSERVED_LLVM_BUILD_ID_FILE";
const SUCCESS_MARKER: &str = "FE2O3_REPOSITORY_SCALAR_ADD_V1_MI300X_OK";
const SOURCE_OBSERVATION_MARKER: &str =
    "FE2O3_PLIRON_SCALAR_ADD_V1_NON_AUTHORITATIVE_SOURCE_OBSERVATION_V1";
const MAX_HSACO_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BUILD_ID_BYTES: usize = 512;

type BoxError = Box<dyn std::error::Error>;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Result<Self, BoxError> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-repository-scalar-add-v1-{}-{}",
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
    env::var(name).map_err(|_| format!("missing required environment variable {name}").into())
}

fn canonical_manifest_value(name: &str) -> Result<String, BoxError> {
    let path = PathBuf::from(required_env(name)?);
    require(path.is_absolute(), format!("{name} path must be absolute"))?;
    let metadata = fs::symlink_metadata(&path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        format!("{name} must be a regular non-symlink file"),
    )?;
    require(
        fs::canonicalize(&path)? == path,
        format!("{name} path must already be canonical"),
    )?;
    let bytes = fs::read(path)?;
    require(
        !bytes.is_empty() && bytes.len() <= MAX_BUILD_ID_BYTES,
        format!("{name} must be a bounded nonempty manifest"),
    )?;
    let text = std::str::from_utf8(&bytes)?;
    let value = text.strip_suffix('\n').unwrap_or(text);
    require(
        !value.is_empty()
            && !value.contains(['\r', '\n', '\0'])
            && value.bytes().all(|byte| byte.is_ascii_graphic()),
        format!("{name} must contain one canonical identity line"),
    )?;
    Ok(value.to_owned())
}

fn measured_worker() -> Result<PinnedWorkerV1, BoxError> {
    let worker_path = PathBuf::from(required_env(WORKER_ENV)?);
    require(worker_path.is_absolute(), "worker path must be absolute")?;
    let metadata = fs::symlink_metadata(&worker_path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "worker must be a regular non-symlink file",
    )?;
    require(
        fs::canonicalize(&worker_path)? == worker_path,
        "worker path must already be canonical",
    )?;
    let worker_build_identity = canonical_manifest_value(WORKER_BUILD_MANIFEST_ENV)?;
    let llvm_build_identity = canonical_manifest_value(LLVM_BUILD_MANIFEST_ENV)?;
    require(
        llvm_build_identity == PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY,
        "worker manifest does not name pinned upstream LLVM 22.1.8",
    )?;
    let bytes = fs::read(&worker_path)?;
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&bytes),
        worker_build_identity,
        llvm_build_identity,
    )?;
    Ok(PinnedWorkerV1::open(worker_path, measurement)?)
}

fn publish_and_consume(
    directory: &TestDirectory,
    bytes: &[u8],
) -> Result<ConsumedCompilerModuleHandoffV1, BoxError> {
    let producer = ProducerIdentity::from_codegen(
        "gfx942_repository_scalar_add_v1_hardware",
        Some(Path::new(
            "tests/support/gfx942_repository_scalar_add_v1_runner.rs",
        )),
    )?;
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x34; 32]),
        BuildSession::from_bytes([0x43; 16]),
    )?;
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, bytes)?;
    Ok(consume_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
    )?)
}

fn exact_link_options() -> Result<Vec<LinkOptionV1>, BoxError> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).map_err(Into::into))
    .collect()
}

pub(crate) fn run() -> Result<(), BoxError> {
    require(
        env::var(RUN_ENV).as_deref() == Ok("1"),
        format!("set {RUN_ENV}=1 to run the isolated MI300X slice"),
    )?;
    // The worker build/LLVM manifests are observation inputs only. They cannot
    // construct approval; finalization receives only repository_profile_v1().
    let worker = measured_worker()?;
    let transaction = TestDirectory::new()?;
    let prepared = canonical_prepared_scalar_add_v1()?.into_prepared();
    let consumed =
        publish_and_consume(&transaction, prepared.compiler_handoff().canonical_bytes())?;
    let first_build = execute_reproducible_first_build_worker_v2(
        consumed.clone(),
        &worker,
        Vec::new(),
        exact_link_options()?,
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES)?,
        WorkerExecutionLimitsV1::default(),
    )?;
    let input_kinds =
        LinkInputKindClosureV1::new(first_build.plan(), vec![WorkerInputKindV1::LlvmTextIr])?;
    let request = construct_scalar_add_worker_request_v2(
        prepared,
        first_build.plan(),
        &worker,
        consumed,
        &input_kinds,
        WorkerOutputConstraintsV1::new(first_build.output_identity().byte_len())?,
    )?;
    let authorized_execution = first_build.into_authorized_execution();
    let finalized =
        finalize_repository_scalar_add_v1(request, authorized_execution, repository_profile_v1()?)?;
    let evidence = execute_repository_scalar_add_v1_on_mi300x(finalized)?;
    require(
        evidence.output_bits() == 3.75_f32.to_bits()
            && !evidence.claims_general_memory_safety()
            && !evidence.claims_general_race_freedom()
            && !evidence.claims_cuda_oxide_parity(),
        "bounded runtime evidence changed its scope or result",
    )?;
    let marker = evidence.success_marker_v1();
    require(
        marker.starts_with(SUCCESS_MARKER),
        "success marker prefix changed",
    )?;
    require(
        RuntimeEvidenceV1::parse_success_marker_v1(&marker)? == evidence.identity(),
        "success marker did not parse to the aggregate evidence identity",
    )?;
    println!("{marker}");
    Ok(())
}

pub(crate) fn observe_canonical_source() -> Result<CanonicalSourceObservationV1, BoxError> {
    let observation = canonical_source_observation_v1()?;
    println!(
        "{SOURCE_OBSERVATION_MARKER} authority=none qualification=none hsa_touched=false source_sha256={} source_length={} source_manifest_sha256={} source_manifest_length={} origin_identity_sha256={} semantic_identity_sha256={} schedule_identity_sha256={} target_plan_identity_sha256={} v2_handoff_identity_sha256={} assembly_sha256={} assembly_length={} compiler_handoff_sha256={} compiler_handoff_length={} symbol_manifest_sha256={} symbol_manifest_length={}",
        hex(observation.source_identity().sha256()),
        observation.source_identity().byte_len(),
        hex(observation.source_manifest_identity().sha256()),
        observation.source_manifest_identity().byte_len(),
        hex(observation.origin_identity()),
        hex(observation.semantic_identity()),
        hex(observation.schedule_identity()),
        hex(observation.target_plan_identity()),
        hex(observation.v2_handoff_identity()),
        hex(observation.assembly_identity().sha256()),
        observation.assembly_identity().byte_len(),
        hex(observation.compiler_handoff_identity().sha256()),
        observation.compiler_handoff_identity().byte_len(),
        hex(observation.symbol_manifest_identity().sha256()),
        observation.symbol_manifest_identity().byte_len(),
    );
    Ok(observation)
}

pub(crate) fn observe_runtime_environment() -> Result<(), BoxError> {
    let context = GpuContext::new(0)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    let environment = adapter.environment();
    println!(
        "FE2O3_PLIRON_SCALAR_ADD_V1_NON_AUTHORITATIVE_RUNTIME_OBSERVATION_V1 authority=none implementation={} version={} image_sha256={} runtime={} device={} target={} agent={:016x}",
        environment.runtime().implementation().replace(' ', "_"),
        environment.runtime().version(),
        hex(environment.runtime().image_digest().bytes().as_bytes()),
        hex(&environment.runtime().instance()),
        hex(&environment.physical_device().uuid()),
        environment.physical_device().target(),
        environment.agent().agent_handle(),
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
