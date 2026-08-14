#![cfg(target_os = "linux")]

use std::{
    env,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use fe2o3_artifact_transaction::{
    BuildAttempt, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    consume_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{CodeObjectVersion as CompilerCodeObjectVersion, CompilerModuleHandoffV2};
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, LinkOptionV1, PinnedWorkerV1,
    RowSoftmaxV1DirectWorkerExpectationV1, RowSoftmaxV1DirectWorkerPinsV1,
    RowSoftmaxV1OcmlProviderPinsV1, WorkerDeviceLibraryProviderEvidenceV1, WorkerExecutionLimitsV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
    inspect_row_softmax_v1_direct_worker_hsaco_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use object::{Object, ObjectSymbol};

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
const OCML_SHA256_ENVS: [&str; 4] = [
    "FE2O3_ROW_SOFTMAX_V1_OCML_SHA256",
    "FE2O3_ROW_SOFTMAX_V1_ISA942_SHA256",
    "FE2O3_ROW_SOFTMAX_V1_UNSAFE_MATH_OFF_SHA256",
    "FE2O3_ROW_SOFTMAX_V1_FINITE_ONLY_OFF_SHA256",
];
const PROVIDER_MANIFEST_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_MANIFEST_SHA256";
const OUTPUT_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_OUTPUT";
const TARGET: &str = "gfx942:xnack-";
const OCML_PROVIDER_BASENAMES: [&str; 4] = [
    "ocml.bc",
    "oclc_isa_version_942.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
];

const REQUIRED_ENVIRONMENT: [&str; 17] = [
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
    OCML_SHA256_ENVS[0],
    OCML_SHA256_ENVS[1],
    OCML_SHA256_ENVS[2],
    OCML_SHA256_ENVS[3],
    PROVIDER_MANIFEST_ENV,
    OUTPUT_ENV,
];

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required row-softmax native pin {name} is absent"))
}

fn required_sha256(name: &str) -> [u8; 32] {
    let value = required_env(name);
    assert_eq!(value.len(), 64, "{name} must contain exactly 64 hex digits");
    let mut decoded = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_nibble(name, pair[0]) << 4) | hex_nibble(name, pair[1]);
    }
    assert_ne!(decoded, [0; 32], "{name} must not be zero");
    decoded
}

fn hex_nibble(name: &str, value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("{name} must use lowercase hexadecimal"),
    }
}

fn required_u64(name: &str) -> u64 {
    let value = required_env(name);
    let decoded = value
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{name} must contain a canonical positive decimal integer"));
    assert_ne!(decoded, 0, "{name} must not be zero");
    assert_eq!(decoded.to_string(), value, "{name} is not canonical");
    decoded
}

fn configured_environment_is_present() -> bool {
    let present = REQUIRED_ENVIRONMENT
        .iter()
        .filter(|name| env::var_os(name).is_some())
        .count();
    if present == 0 {
        eprintln!(
            "skipping configured row-softmax direct-worker integration: environment is absent"
        );
        return false;
    }
    assert_eq!(
        present,
        REQUIRED_ENVIRONMENT.len(),
        "partial row-softmax native configuration is forbidden; required variables: {}",
        REQUIRED_ENVIRONMENT.join(",")
    );
    true
}

fn worker_pins() -> RowSoftmaxV1DirectWorkerPinsV1 {
    RowSoftmaxV1DirectWorkerPinsV1::new(
        ContentIdentityV1::from_parts(
            required_sha256(WORKER_SHA256_ENV),
            required_u64(WORKER_BYTES_ENV),
        ),
        &required_env(WORKER_BUILD_ID_ENV),
        &required_env(LLVM_BUILD_ID_ENV),
        RowSoftmaxV1OcmlProviderPinsV1::new(
            OCML_SHA256_ENVS.map(required_sha256),
            required_sha256(PROVIDER_MANIFEST_ENV),
        )
        .expect("independently pinned gfx942 OCML provider closure"),
    )
    .expect("independently pinned row-softmax worker")
}

fn production_handoff(
    expected_worker: RowSoftmaxV1DirectWorkerPinsV1,
) -> (
    ConsumedCompilerModuleHandoffV1,
    CompilerModuleHandoffV2,
    RowSoftmaxV1DirectWorkerExpectationV1,
) {
    let producer_source = required_env(HANDOFF_PRODUCER_SOURCE_ENV);
    let producer = ProducerIdentity::from_codegen(
        &required_env(HANDOFF_PRODUCER_CRATE_ENV),
        (producer_source != "-").then(|| Path::new(&producer_source)),
    )
    .expect("reconstruct production rustc producer identity");
    let attempt = BuildAttempt::from_env_value(&required_env(HANDOFF_ATTEMPT_ENV))
        .expect("decode production rustc build attempt");
    let consumed = consume_compiler_module_handoff_v1(
        Path::new(&required_env(HANDOFF_ROOT_ENV)),
        &producer,
        attempt,
    )
    .expect("consume production rustc row-softmax handoff slot");
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .expect("strictly decode rustc-produced row-softmax Worker V2 handoff");
    assert_eq!(handoff.target().to_string(), TARGET);
    assert_eq!(handoff.code_object_version(), CompilerCodeObjectVersion::V6);
    assert_eq!(handoff.canonical_bytes(), consumed.bytes());
    let expectation = RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
        &handoff,
        required_sha256(HANDOFF_SHA256_ENV),
        required_sha256(FRONTEND_AUTHORITY_ENV),
        expected_worker,
    )
    .expect("admit exact pinned row-softmax rustc handoff");
    (consumed, handoff, expectation)
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "0"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed row-softmax option"))
    .collect()
}

struct ProducedRowSoftmax {
    bytes: Vec<u8>,
    finalized_identity: FinalizedWorkerV2HsacoIdentityV1,
    finalized_output_identity: ContentIdentityV1,
}

fn produce_inspect_and_finalize(
    worker: &PinnedWorkerV1,
    consumed_handoff: ConsumedCompilerModuleHandoffV1,
    expectation: RowSoftmaxV1DirectWorkerExpectationV1,
) -> ProducedRowSoftmax {
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff,
        worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64).expect("bounded row-softmax output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("direct upstream LLVM/LLD row-softmax production");
    for execution in [evidence.bootstrap(), evidence.exact_replay()] {
        let provider = execution
            .response()
            .device_library_provider()
            .expect("structured measured OCML provider evidence on both V2 executions");
        assert_measured_ocml_provider(provider, expectation.worker_pins().provider());
    }
    let diagnostics = evidence.authorized().response().diagnostics().to_vec();
    let inspected = inspect_row_softmax_v1_direct_worker_hsaco_v1(evidence, expectation)
        .unwrap_or_else(|error| {
            panic!("exact row-softmax Worker V2 inspection: {error:?}; diagnostics={diagnostics:?}")
        });

    assert_eq!(inspected.structural().target().to_string(), TARGET);
    assert_eq!(
        inspected.structural().code_object_version(),
        CodeObjectVersion::V6
    );
    assert_eq!(
        inspected.exchange().requested_ocml_import(),
        "__ocml_exp_f32"
    );
    assert!(
        inspected
            .exchange()
            .measured_gfx942_ocml_provider_closure_was_checked()
    );
    assert_eq!(inspected.exchange().measured_ocml_provider_file_count(), 4);
    assert_eq!(
        inspected
            .exchange()
            .measured_ocml_provider_manifest_identity(),
        expectation.worker_pins().provider().manifest_identity()
    );
    assert_eq!(
        inspected
            .exchange()
            .embedded_frontend_authority_commitment(),
        expectation.frontend_authority_commitment()
    );
    assert!(!inspected.proves_exp_math_accuracy());
    assert!(!inspected.proves_functional_softmax());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
    assert!(!inspected.exchange().proves_no_comgr_linkage());
    assert!(
        inspected
            .exchange()
            .no_comgr_requires_measured_worker_build_manifest()
    );

    let raw_bytes = inspected.structural().exact_bytes().to_vec();
    assert_eq!(
        inspected.exchange().linked_output_identity(),
        ContentIdentityV1::calculate(&raw_bytes)
    );
    assert_ocml_symbol_closure(&raw_bytes);

    let finalized =
        finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(inspected.into_structural())
            .expect("canonical row-softmax descriptor finalization");
    assert!(finalized.canonical_descriptor_finalization_ran());
    assert_ne!(
        finalized.raw_output_identity(),
        finalized.finalized_output_identity()
    );
    assert_ne!(finalized.raw_output_identity().sha256(), &[0; 32]);
    assert_ne!(finalized.finalized_output_identity().sha256(), &[0; 32]);
    assert!(!finalized.proves_exp_implementation());
    assert!(!finalized.proves_numerical_contract());
    assert!(!finalized.proves_functional_softmax());
    assert!(!finalized.grants_publication_authority());
    assert!(!finalized.grants_load_authority());
    assert!(!finalized.grants_launch_authority());

    let bytes = finalized.exact_finalized_bytes().to_vec();
    assert!(finalized.finalized_output_identity().matches(&bytes));
    assert_ocml_symbol_closure(&bytes);

    ProducedRowSoftmax {
        bytes,
        finalized_identity: finalized.identity(),
        finalized_output_identity: finalized.finalized_output_identity(),
    }
}

fn assert_measured_ocml_provider(
    actual: &WorkerDeviceLibraryProviderEvidenceV1,
    expected: RowSoftmaxV1OcmlProviderPinsV1,
) {
    assert_eq!(actual.provider_identity(), "gfx942-ocml-v1");
    assert_eq!(actual.target().to_string(), TARGET);
    assert_eq!(actual.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(actual.import_symbols(), ["__ocml_exp_f32"]);
    assert_eq!(actual.manifest_identity(), expected.manifest_identity());
    assert_eq!(actual.files().len(), OCML_PROVIDER_BASENAMES.len());
    for (index, file) in actual.files().iter().enumerate() {
        assert_eq!(file.basename(), OCML_PROVIDER_BASENAMES[index]);
        assert_eq!(file.sha256(), &expected.file_sha256()[index]);
    }
}

fn assert_ocml_symbol_closure(bytes: &[u8]) {
    let file = object::File::parse(bytes).expect("parse row-softmax ELF symbol table");
    let mut exp_definition = false;
    for symbol in file.symbols().chain(file.dynamic_symbols()) {
        let Ok(name) = symbol.name() else {
            continue;
        };
        if name == "__ocml_exp_f32" && !symbol.is_undefined() {
            exp_definition = true;
        }
        assert!(
            !(name.starts_with("__ocml_") && symbol.is_undefined()),
            "unresolved OCML symbol escaped final output: {name}"
        );
    }
    assert!(
        exp_definition,
        "linked output omitted the requested OCML root"
    );
}

#[test]
fn real_worker_produces_deterministic_finalized_row_softmax_v1_cov6_hsaco() {
    if !configured_environment_is_present() {
        return;
    }
    let expected_worker = worker_pins();
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let measurement = WorkerMeasurementV1::new(
        expected_worker.executable(),
        required_env(WORKER_BUILD_ID_ENV),
        required_env(LLVM_BUILD_ID_ENV),
    )
    .expect("exact row-softmax worker measurement");
    let worker =
        PinnedWorkerV1::open(&worker_path, measurement).expect("open measured row-softmax worker");
    let (consumed, _handoff, expectation) = production_handoff(expected_worker);

    let first = produce_inspect_and_finalize(&worker, consumed.clone(), expectation);
    let second = produce_inspect_and_finalize(&worker, consumed, expectation);
    assert_eq!(
        first.bytes, second.bytes,
        "repeated row-softmax links changed bytes"
    );
    assert_eq!(
        first.finalized_identity, second.finalized_identity,
        "repeated row-softmax finalization changed its sealed identity"
    );
    assert_eq!(
        first.finalized_output_identity, second.finalized_output_identity,
        "repeated row-softmax finalization changed its content identity"
    );
    assert!(first.finalized_output_identity.matches(&first.bytes));

    let output = PathBuf::from(required_env(OUTPUT_ENV));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .expect("create fresh row-softmax HSACO output");
    file.write_all(&first.bytes)
        .expect("write row-softmax HSACO");
    file.sync_all().expect("sync row-softmax HSACO");
}
