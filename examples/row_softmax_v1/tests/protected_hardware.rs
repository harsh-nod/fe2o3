#![cfg(target_os = "linux")]

use fe2o3_artifact_transaction::{
    BuildAttempt, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    consume_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{CodeObjectVersion as CompilerCodeObjectVersion, CompilerModuleHandoffV2};
use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedProtectedRowSoftmaxV1HostAdapterV1, ObservedContext, join_protected_row_softmax_v1,
    prepare_protected_row_softmax_v1_host_token_v1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT,
    RowSoftmaxV1AuthorityPolicyV1, RowSoftmaxV1CompilerClosurePolicyV1,
    RowSoftmaxV1DirectWorkerExpectationV1, RowSoftmaxV1DirectWorkerPinsV1,
    RowSoftmaxV1OcmlProviderPinsV1, RowSoftmaxV1ProviderManifestV1, WorkerExecutionLimitsV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    inspect_row_softmax_v1_direct_worker_hsaco_v1, prepare_protected_row_softmax_v1_admission_v1,
};
use fe2o3_row_softmax_v1::{
    GFX942_OCML_COMPARISON_POLICY_V1, ROW_SOFTMAX_VERIFICATION_MANIFEST_V1, compare_row_softmax_v1,
    row_softmax_oracle_v1, validate_row_softmax_verification_manifest_v1,
};
use fe2o3_verifier::{
    RowSoftmaxVerificationCertificateObservationV1, RowSoftmaxVerificationFileObservationV1,
    authenticate_row_softmax_verification_certificate_v1,
};
use sha2::{Digest, Sha256};
use std::{env, error::Error, path::Path, path::PathBuf, time::Duration};

const WORKER_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_WORKER";
const WORKER_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_WORKER_SHA256";
const WORKER_BYTES_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_WORKER_BYTES";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_LLVM_BUILD_ID";
const HANDOFF_ROOT_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF_ROOT";
const HANDOFF_PRODUCER_CRATE_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF_PRODUCER_CRATE";
const HANDOFF_PRODUCER_SOURCE_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF_PRODUCER_SOURCE";
const HANDOFF_ATTEMPT_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF_ATTEMPT";
const HANDOFF_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF_SHA256";
const FRONTEND_AUTHORITY_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_FRONTEND_AUTHORITY_SHA256";
const BROKER_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_BROKER_SHA256";
const CARGO_EXECUTABLE_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_CARGO_EXECUTABLE_SHA256";
const RUSTC_EXECUTABLE_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_RUSTC_EXECUTABLE_SHA256";
const RUSTC_RUNTIME_TREE_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_RUSTC_RUNTIME_TREE_SHA256";
const CODEGEN_BACKEND_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_CODEGEN_BACKEND_SHA256";
const PROVIDER_STABLE_CRATE_ID_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_STABLE_CRATE_ID";
const PROVIDER_CRATE_HASH_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_CRATE_HASH";
const PROVIDER_DEFINITIONS_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_DEFINITION_IDENTITIES";
const PROVIDER_SOURCES_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_SOURCE_IDENTITIES";
const OCML_SHA256_ENVS: [&str; 4] = [
    "FE2O3_ROW_SOFTMAX_V1_OCML_SHA256",
    "FE2O3_ROW_SOFTMAX_V1_ISA942_SHA256",
    "FE2O3_ROW_SOFTMAX_V1_UNSAFE_MATH_OFF_SHA256",
    "FE2O3_ROW_SOFTMAX_V1_FINITE_ONLY_OFF_SHA256",
];
const PROVIDER_MANIFEST_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_MANIFEST_SHA256";
const OPT_IN_ENV: &str = "FE2O3_RUN_PROTECTED_ROW_SOFTMAX_V1_HARDWARE";
const TARGET: &str = "gfx942:xnack-";
const ELEMENTS: usize = 64;
const GUARD: usize = 32;
const INPUT_PREFIX: f32 = f32::from_bits(0x7fc0_a001);
const INPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_a002);
const OUTPUT_PREFIX: f32 = f32::from_bits(0x7fc0_d001);
const OUTPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_d002);
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_d0ff);
const SUCCESS_MARKER: &str = "FE2O3_PROTECTED_ROW_SOFTMAX_V1_OK";

const REQUIRED_ENVIRONMENT: [&str; 25] = [
    WORKER_ENV,
    WORKER_SHA256_ENV,
    WORKER_BYTES_ENV,
    WORKER_BUILD_ID_ENV,
    LLVM_BUILD_ID_ENV,
    HANDOFF_ROOT_ENV,
    HANDOFF_PRODUCER_CRATE_ENV,
    HANDOFF_PRODUCER_SOURCE_ENV,
    HANDOFF_ATTEMPT_ENV,
    HANDOFF_SHA256_ENV,
    FRONTEND_AUTHORITY_ENV,
    BROKER_SHA256_ENV,
    CARGO_EXECUTABLE_SHA256_ENV,
    RUSTC_EXECUTABLE_SHA256_ENV,
    RUSTC_RUNTIME_TREE_SHA256_ENV,
    CODEGEN_BACKEND_SHA256_ENV,
    PROVIDER_STABLE_CRATE_ID_ENV,
    PROVIDER_CRATE_HASH_ENV,
    PROVIDER_DEFINITIONS_ENV,
    PROVIDER_SOURCES_ENV,
    OCML_SHA256_ENVS[0],
    OCML_SHA256_ENVS[1],
    OCML_SHA256_ENVS[2],
    OCML_SHA256_ENVS[3],
    PROVIDER_MANIFEST_ENV,
];

type BoxError = Box<dyn Error>;

fn required_env(name: &str) -> Result<String, BoxError> {
    env::var(name).map_err(|_| format!("missing exact protected pin {name}").into())
}

fn required_hex<const N: usize>(name: &str, value: &str) -> Result<[u8; N], BoxError> {
    if value.len() != N * 2 {
        return Err(format!("{name} must contain exactly {} lowercase hex digits", N * 2).into());
    }
    let mut decoded = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_nibble(name, pair[0])? << 4) | hex_nibble(name, pair[1])?;
    }
    Ok(decoded)
}

fn hex_nibble(name: &str, value: u8) -> Result<u8, BoxError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(format!("{name} must use lowercase hexadecimal").into()),
    }
}

fn required_sha256(name: &str) -> Result<[u8; 32], BoxError> {
    let value = required_hex(name, &required_env(name)?)?;
    if value == [0; 32] {
        return Err(format!("{name} must not be zero").into());
    }
    Ok(value)
}

fn required_u64(name: &str) -> Result<u64, BoxError> {
    let value = required_env(name)?;
    let decoded = value.parse::<u64>()?;
    if decoded == 0 || decoded.to_string() != value {
        return Err(format!("{name} must be a canonical positive integer").into());
    }
    Ok(decoded)
}

fn required_identity_list<const N: usize, const WIDTH: usize>(
    name: &str,
) -> Result<[[u8; WIDTH]; N], BoxError> {
    let value = required_env(name)?;
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != N {
        return Err(format!("{name} must contain exactly {N} identities").into());
    }
    let mut result = [[0; WIDTH]; N];
    for (index, field) in fields.into_iter().enumerate() {
        result[index] = required_hex(name, field)?;
    }
    Ok(result)
}

fn require_complete_environment() -> Result<(), BoxError> {
    let present = REQUIRED_ENVIRONMENT
        .iter()
        .filter(|name| env::var_os(name).is_some())
        .count();
    if present != REQUIRED_ENVIRONMENT.len() {
        return Err(format!(
            "protected hardware execution requires all {} exact compiler/worker/OCML pins; found {present}",
            REQUIRED_ENVIRONMENT.len()
        )
        .into());
    }
    Ok(())
}

fn worker_pins() -> Result<RowSoftmaxV1DirectWorkerPinsV1, BoxError> {
    Ok(RowSoftmaxV1DirectWorkerPinsV1::new(
        ContentIdentityV1::from_parts(
            required_sha256(WORKER_SHA256_ENV)?,
            required_u64(WORKER_BYTES_ENV)?,
        ),
        &required_env(WORKER_BUILD_ID_ENV)?,
        &required_env(LLVM_BUILD_ID_ENV)?,
        RowSoftmaxV1OcmlProviderPinsV1::new(
            [
                required_sha256(OCML_SHA256_ENVS[0])?,
                required_sha256(OCML_SHA256_ENVS[1])?,
                required_sha256(OCML_SHA256_ENVS[2])?,
                required_sha256(OCML_SHA256_ENVS[3])?,
            ],
            required_sha256(PROVIDER_MANIFEST_ENV)?,
        )?,
    )?)
}

fn authority_policy(attempt: BuildAttempt) -> Result<RowSoftmaxV1AuthorityPolicyV1, BoxError> {
    let provider = RowSoftmaxV1ProviderManifestV1::new(
        required_u64(PROVIDER_STABLE_CRATE_ID_ENV)?,
        required_hex(
            PROVIDER_CRATE_HASH_ENV,
            &required_env(PROVIDER_CRATE_HASH_ENV)?,
        )?,
        required_identity_list::<ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT, 16>(PROVIDER_DEFINITIONS_ENV)?,
        required_identity_list::<ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT, 32>(PROVIDER_SOURCES_ENV)?,
    )?;
    let compiler = RowSoftmaxV1CompilerClosurePolicyV1::new(
        required_sha256(CARGO_EXECUTABLE_SHA256_ENV)?,
        required_sha256(RUSTC_EXECUTABLE_SHA256_ENV)?,
        required_sha256(RUSTC_RUNTIME_TREE_SHA256_ENV)?,
        required_sha256(CODEGEN_BACKEND_SHA256_ENV)?,
    )?;
    Ok(RowSoftmaxV1AuthorityPolicyV1::new(
        provider,
        attempt,
        required_sha256(BROKER_SHA256_ENV)?,
        compiler,
    )?)
}

fn production_handoff(
    worker: RowSoftmaxV1DirectWorkerPinsV1,
) -> Result<
    (
        ConsumedCompilerModuleHandoffV1,
        RowSoftmaxV1DirectWorkerExpectationV1,
    ),
    BoxError,
> {
    let source = required_env(HANDOFF_PRODUCER_SOURCE_ENV)?;
    let producer = ProducerIdentity::from_codegen(
        &required_env(HANDOFF_PRODUCER_CRATE_ENV)?,
        (source != "-").then(|| Path::new(&source)),
    )?;
    let attempt = BuildAttempt::from_env_value(&required_env(HANDOFF_ATTEMPT_ENV)?)?;
    let consumed = consume_compiler_module_handoff_v1(
        Path::new(&required_env(HANDOFF_ROOT_ENV)?),
        &producer,
        attempt,
    )?;
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())?;
    if handoff.target().to_string() != TARGET
        || handoff.code_object_version() != CompilerCodeObjectVersion::V6
        || handoff.canonical_bytes() != consumed.bytes()
    {
        return Err("rustc handoff does not match exact row-softmax profile".into());
    }
    let expectation = RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
        &handoff,
        required_sha256(HANDOFF_SHA256_ENV)?,
        required_sha256(FRONTEND_AUTHORITY_ENV)?,
        authority_policy(attempt)?,
        worker,
    )?;
    Ok((consumed, expectation))
}

fn link_options() -> Result<Vec<LinkOptionV1>, BoxError> {
    [
        ("code-object-version", "6"),
        ("opt-level", "0"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).map_err(Into::into))
    .collect()
}

fn certificate()
-> Result<fe2o3_verifier::AuthenticatedRowSoftmaxVerificationCertificateV1, BoxError> {
    let inert = validate_row_softmax_verification_manifest_v1(ROW_SOFTMAX_VERIFICATION_MANIFEST_V1)
        .map_err(|error| format!("row-softmax verification manifest mismatch: {error:?}"))?;
    let manifest = inert.canonical_manifest_bytes();
    let evidence = [
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs",
            include_bytes!(
                "../../../crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs"
            ),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/src/numerical_contract.rs",
            include_bytes!("../src/numerical_contract.rs"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/verus/row_softmax_v1.rs",
            include_bytes!("../verus/row_softmax_v1.rs"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST",
            include_bytes!("../verus/VERUS_CLOSURE_MANIFEST"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY",
            include_bytes!("../verus/VERUS_TRUST_VOCABULARY"),
        )),
    ];
    Ok(authenticate_row_softmax_verification_certificate_v1(
        RowSoftmaxVerificationCertificateObservationV1::new(
            manifest,
            Sha256::digest(manifest).into(),
            evidence,
        ),
    )?)
}

fn protected_token() -> Result<fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1, BoxError> {
    let pins = worker_pins()?;
    let measurement = WorkerMeasurementV1::new(
        pins.executable(),
        required_env(WORKER_BUILD_ID_ENV)?,
        required_env(LLVM_BUILD_ID_ENV)?,
    )?;
    let worker = PinnedWorkerV1::open(PathBuf::from(required_env(WORKER_ENV)?), measurement)?;
    let (handoff, expectation) = production_handoff(pins)?;
    let evidence = execute_reproducible_first_build_worker_v2(
        handoff,
        &worker,
        Vec::new(),
        link_options()?,
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)?,
        WorkerExecutionLimitsV1::default(),
    )?;
    let inspected = inspect_row_softmax_v1_direct_worker_hsaco_v1(evidence, expectation)?;
    let admission = prepare_protected_row_softmax_v1_admission_v1(certificate()?, inspected)?;
    Ok(prepare_protected_row_softmax_v1_host_token_v1(admission)?)
}

fn guarded(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut result = vec![prefix; GUARD];
    result.extend_from_slice(body);
    result.resize(GUARD + ELEMENTS + GUARD, suffix);
    result
}

fn verify_exact(actual: &[f32], expected: &[f32], role: &str) -> Result<(), BoxError> {
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
    {
        return Err(format!("{role} changed").into());
    }
    Ok(())
}

#[test]
#[ignore = "requires complete exact compiler/worker/OCML pins and gfx942:xnack- hardware"]
fn exact_protected_lifecycle_executes_one_guarded_unmasked_row() -> Result<(), BoxError> {
    if env::var(OPT_IN_ENV).as_deref() != Ok("1") {
        eprintln!("skipping protected hardware run: set {OPT_IN_ENV}=1");
        return Ok(());
    }
    require_complete_environment()?;
    let token = protected_token()?;
    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    if observed.device().target() != TARGET {
        return Err(format!(
            "protected run requires {TARGET}, found {}",
            observed.device().target()
        )
        .into());
    }

    let input_body: Vec<f32> = (0..ELEMENTS)
        .map(|index| ((index * 17 + 3) % 29) as f32 * 0.25 - 3.5)
        .collect();
    let mut expected = vec![0.0; ELEMENTS];
    row_softmax_oracle_v1(&input_body, None, &mut expected)
        .map_err(|error| format!("row-softmax oracle rejected the exact input: {error:?}"))?;
    let input_initial = guarded(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
    let output_initial = guarded(&[OUTPUT_POISON; ELEMENTS], OUTPUT_PREFIX, OUTPUT_SUFFIX);
    let stream = context.create_stream()?;
    let input = DeviceBuffer::from_host(&stream, &input_initial)?;
    let mut output = DeviceBuffer::from_host(&stream, &output_initial)?;
    let body = GUARD..GUARD + ELEMENTS;
    let host = GeneratedProtectedRowSoftmaxV1HostAdapterV1::prepare(
        &observed,
        input.view(body.clone())?,
        output.view_mut(body.clone())?,
    )?;
    let joined = join_protected_row_softmax_v1(token, host)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    if adapter.completion_timeout_v1() != Duration::from_secs(5) {
        return Err("protected runtime completion deadline drifted".into());
    }
    let completed = joined.load(adapter)?.dispatch_and_wait()?;
    let unloaded = completed.unload();
    if unloaded.unload_identity().as_bytes() == &[0; 32] || unloaded.proves_masked_execution() {
        return Err("terminal receipt drifted or overclaimed masked execution".into());
    }

    let input_after = input.to_host_vec(&stream)?;
    let output_after = output.to_host_vec(&stream)?;
    verify_exact(&input_after, &input_initial, "guarded immutable input")?;
    verify_exact(
        &output_after[..GUARD],
        &output_initial[..GUARD],
        "output prefix canary",
    )?;
    verify_exact(
        &output_after[GUARD + ELEMENTS..],
        &output_initial[GUARD + ELEMENTS..],
        "output suffix canary",
    )?;
    compare_row_softmax_v1(
        &expected,
        &output_after[body],
        None,
        GFX942_OCML_COMPARISON_POLICY_V1,
    )
    .map_err(|error| format!("row-softmax hardware output mismatch: {error:?}"))?;
    println!(
        "{SUCCESS_MARKER} outputs={ELEMENTS} timeout_ms={} unload={}",
        Duration::from_secs(5).as_millis(),
        hex(unloaded.unload_identity().as_bytes())
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
