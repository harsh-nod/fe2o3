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
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, LinkOptionV1, PinnedWorkerV1,
    RowSoftmaxV1DirectWorkerExpectationV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
    finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
    inspect_row_softmax_v1_direct_worker_hsaco_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use object::{Object, ObjectSymbol};

const WORKER_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_LLVM_BUILD_ID";
const HANDOFF_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF";
const HANDOFF_SHA256_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_HANDOFF_SHA256";
const FRONTEND_AUTHORITY_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_FRONTEND_AUTHORITY_SHA256";
const OUTPUT_ENV: &str = "FE2O3_ROW_SOFTMAX_V1_OUTPUT";
const TARGET: &str = "gfx942:xnack-";

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

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-row-softmax-v1-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create row-softmax handoff directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn canonical_handoff() -> (
    CompilerModuleHandoffV2,
    RowSoftmaxV1DirectWorkerExpectationV1,
) {
    let path = PathBuf::from(required_env(HANDOFF_ENV));
    let bytes = fs::read(&path).expect("read rustc-produced row-softmax Worker V2 handoff");
    let handoff = CompilerModuleHandoffV2::decode(&bytes)
        .expect("strictly decode rustc-produced row-softmax Worker V2 handoff");
    assert_eq!(handoff.target().to_string(), TARGET);
    assert_eq!(handoff.code_object_version(), CompilerCodeObjectVersion::V6);
    assert_eq!(handoff.canonical_bytes(), bytes);
    let expectation = RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
        &handoff,
        required_sha256(HANDOFF_SHA256_ENV),
        required_sha256(FRONTEND_AUTHORITY_ENV),
    )
    .expect("admit exact pinned row-softmax rustc handoff");
    (handoff, expectation)
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
) -> ConsumedCompilerModuleHandoffV1 {
    let producer = ProducerIdentity::from_codegen(
        "row_softmax_v1_direct_llvm_worker",
        Some(Path::new("tests/row_softmax_v1_direct_llvm_worker.rs")),
    )
    .expect("row-softmax test producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x52; 32]),
        BuildSession::from_bytes([0x94; 16]),
    )
    .expect("begin row-softmax handoff attempt");
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .expect("publish row-softmax handoff");
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume row-softmax handoff")
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

fn produce_inspect_and_finalize(worker: &PinnedWorkerV1) -> ProducedRowSoftmax {
    let (handoff, expectation) = canonical_handoff();
    let directory = TestDirectory::new();
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed_handoff(&directory, &handoff),
        worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64).expect("bounded row-softmax output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("direct upstream LLVM/LLD row-softmax production");
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
            .embedded_frontend_authority_commitment(),
        expectation.frontend_authority_commitment()
    );
    assert!(!inspected.proves_exp_math_accuracy());
    assert!(!inspected.proves_functional_softmax());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());

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
    for forbidden in [b"amd_comgr".as_slice(), b"libamd_comgr".as_slice()] {
        assert!(
            !bytes
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "row-softmax HSACO contains forbidden COMGR reference"
        );
    }

    ProducedRowSoftmax {
        bytes,
        finalized_identity: finalized.identity(),
        finalized_output_identity: finalized.finalized_output_identity(),
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
#[ignore = "requires the measured upstream LLVM/LLD worker and pinned gfx942 OCML closure"]
fn real_worker_produces_deterministic_finalized_row_softmax_v1_cov6_hsaco() {
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let worker_bytes = fs::read(&worker_path).expect("read row-softmax worker executable");
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&worker_bytes),
        required_env(WORKER_BUILD_ID_ENV),
        required_env(LLVM_BUILD_ID_ENV),
    )
    .expect("exact row-softmax worker measurement");
    let worker =
        PinnedWorkerV1::open(&worker_path, measurement).expect("open measured row-softmax worker");

    let first = produce_inspect_and_finalize(&worker);
    let second = produce_inspect_and_finalize(&worker);
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
