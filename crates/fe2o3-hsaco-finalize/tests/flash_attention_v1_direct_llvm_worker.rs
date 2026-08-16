#![cfg(target_os = "linux")]

use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{CompilerModuleHandoffV2, CompilerModuleKindV1};
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizedFlashAttentionV1ReceiptV1,
    FlashAttentionV1FinalizationExpectationV1, FlashAttentionV1OcmlProviderPinsV1,
    FlashAttentionV1WorkerPinsV1, InertFirstBuildWorkerV2EvidenceV1, LinkOptionV1, PinnedWorkerV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    execute_reproducible_first_build_worker_v2, finalize_flash_attention_v1_worker_v2_hsaco_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;

const WORKER_ENV: &str = "FE2O3_FLASH_ATTENTION_V1_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_FLASH_ATTENTION_V1_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_FLASH_ATTENTION_V1_LLVM_BUILD_ID";
const HANDOFF_ENV: &str = "FE2O3_FLASH_ATTENTION_V1_HANDOFF";
const RAW_OUTPUT_ENV: &str = "FE2O3_FLASH_ATTENTION_V1_RAW_OUTPUT";
const TRANSCRIPT_OUTPUT_ENV: &str = "FE2O3_FLASH_ATTENTION_V1_TRANSCRIPT_OUTPUT";
const OCML_FILE_SHA256: [&str; 4] = [
    "cfe97fe9ee29379f522e5f20ae55aae1cdb96eb41d6aa250ea11c4941c54e019",
    "580d540cc738c0f9554c8710575bbc9b51ebacdcbc29aa0074ed05d3691dea1d",
    "22c799b9154389f050f8f3368762636b9954a2ea25622199c359366bbd84657f",
    "f3138eeee65c1d83234260728d124f635f021abb37c495f4ed027dfe92bcb1dd",
];
const OCML_MANIFEST_SHA256: &str =
    "e7a3924a5bda6eb5b62aca826d6133766962fc9f6d758fa961dfee674e31d7f9";

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required FlashAttention worker pin {name} is absent"))
}

fn sha256(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut output = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    output
}

fn nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("noncanonical SHA-256 pin"),
    }
}

fn link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed FlashAttention option"))
    .collect()
}

fn worker_pins(
    executable: ContentIdentityV1,
    worker_build_identity: &str,
    llvm_build_identity: &str,
) -> FlashAttentionV1WorkerPinsV1 {
    let provider = FlashAttentionV1OcmlProviderPinsV1::new(
        OCML_FILE_SHA256.map(sha256),
        sha256(OCML_MANIFEST_SHA256),
    )
    .expect("independently digest-pinned OCML closure");
    FlashAttentionV1WorkerPinsV1::new(
        executable,
        worker_build_identity,
        llvm_build_identity,
        provider,
    )
    .expect("independently pinned direct LLVM/LLD worker")
}

fn execute(
    worker: &PinnedWorkerV1,
    handoff: &CompilerModuleHandoffV2,
) -> InertFirstBuildWorkerV2EvidenceV1 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let transaction = env::temp_dir().join(format!(
        "fe2o3-flash-attention-v1-worker-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&transaction).expect("create handoff transaction");
    let producer = ProducerIdentity::from_codegen(
        "flash_attention_v1_direct_llvm_worker",
        Some(Path::new("tests/flash_attention_v1_direct_llvm_worker.rs")),
    )
    .expect("test producer");
    let attempt = begin_build_attempt(
        &transaction,
        &producer,
        BuildInvocation::from_bytes([0xfa; 32]),
        BuildSession::from_bytes(u128::from(sequence).to_le_bytes()),
    )
    .expect("begin handoff transaction");
    publish_compiler_module_handoff_v1(&transaction, &producer, attempt, handoff.canonical_bytes())
        .expect("publish compiler handoff");
    let consumed = consume_compiler_module_handoff_v1(&transaction, &producer, attempt)
        .expect("consume compiler handoff");
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed,
        worker,
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64).expect("bounded output"),
        WorkerExecutionLimitsV1::default(),
    )
    .expect("exact direct LLVM/LLD FlashAttention production");
    fs::remove_dir_all(transaction).expect("remove handoff transaction");
    evidence
}

fn produce(
    worker: &PinnedWorkerV1,
    handoff: &CompilerModuleHandoffV2,
    expectation: FlashAttentionV1FinalizationExpectationV1,
    raw_output: Option<&Path>,
    transcript_output: Option<&Path>,
) -> FinalizedFlashAttentionV1ReceiptV1 {
    let evidence = execute(worker, handoff);
    if let Some(path) = raw_output {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("create private raw FlashAttention output");
        std::io::Write::write_all(&mut file, evidence.output_bytes())
            .expect("write raw FlashAttention output");
    }
    if let Some(path) = transcript_output {
        let response = evidence.exact_replay().response();
        let mut transcript = String::new();
        writeln!(
            transcript,
            "llvm_build_identity={}",
            evidence.worker_measurement().llvm_build_identity()
        )
        .expect("write LLVM identity to transcript");
        writeln!(
            transcript,
            "worker_build_identity={}",
            response.worker_build_identity()
        )
        .expect("write worker identity to transcript");
        for diagnostic in response.diagnostics() {
            writeln!(transcript, "{diagnostic}").expect("write worker diagnostic to transcript");
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("create private FlashAttention transcript");
        std::io::Write::write_all(&mut file, transcript.as_bytes())
            .expect("write FlashAttention transcript");
    }
    finalize_flash_attention_v1_worker_v2_hsaco_v1(evidence, expectation)
        .expect("opaque exact FlashAttention finalization")
}

#[test]
#[ignore = "requires the measured direct LLVM/LLD worker built for gfx942"]
fn real_worker_produces_reproducible_opaque_flash_attention_v1_receipt() {
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let executable =
        ContentIdentityV1::calculate(&fs::read(&worker_path).expect("read worker executable"));
    let worker_build_identity = required_env(WORKER_BUILD_ID_ENV);
    let llvm_build_identity = required_env(LLVM_BUILD_ID_ENV);
    let measurement = WorkerMeasurementV1::new(
        executable,
        worker_build_identity.clone(),
        llvm_build_identity.clone(),
    )
    .expect("exact FlashAttention worker measurement");
    let worker = PinnedWorkerV1::open(&worker_path, measurement).expect("open measured worker");
    let pins = worker_pins(executable, &worker_build_identity, &llvm_build_identity);

    let handoff_bytes = fs::read(required_env(HANDOFF_ENV)).expect("read compiler handoff");
    let handoff = CompilerModuleHandoffV2::decode(&handoff_bytes).expect("decode compiler handoff");
    assert_eq!(handoff.canonical_bytes(), handoff_bytes);
    let expectation =
        FlashAttentionV1FinalizationExpectationV1::from_authenticated_compiler_handoff(
            &handoff, pins,
        )
        .expect("consume exact authenticated compiler authority");

    let mut substituted_module = handoff.module_bytes().to_vec();
    substituted_module[0] ^= 1;
    let substituted = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        handoff.target(),
        handoff.code_object_version(),
        handoff.envelope().clone(),
        handoff.symbol_manifest().clone(),
        &substituted_module,
    )
    .expect("construct authority-free substituted handoff");
    assert!(
        FlashAttentionV1FinalizationExpectationV1::from_authenticated_compiler_handoff(
            &substituted,
            pins,
        )
        .is_err(),
        "module/source substitution retained exact compiler authority"
    );

    let mut substituted_provider_files = OCML_FILE_SHA256.map(sha256);
    substituted_provider_files[0][0] ^= 1;
    let substituted_provider = FlashAttentionV1OcmlProviderPinsV1::new(
        substituted_provider_files,
        sha256(OCML_MANIFEST_SHA256),
    )
    .expect("nonzero substituted OCML provider pins");
    let substituted_worker_pins = FlashAttentionV1WorkerPinsV1::new(
        executable,
        &worker_build_identity,
        &llvm_build_identity,
        substituted_provider,
    )
    .expect("substituted direct worker pins");
    let substituted_expectation =
        FlashAttentionV1FinalizationExpectationV1::from_authenticated_compiler_handoff(
            &handoff,
            substituted_worker_pins,
        )
        .expect("exact handoff with independently supplied provider pins");
    assert!(
        finalize_flash_attention_v1_worker_v2_hsaco_v1(
            execute(&worker, &handoff),
            substituted_expectation,
        )
        .is_err(),
        "OCML provider substitution retained finalization authority"
    );

    let raw_output = env::var_os(RAW_OUTPUT_ENV).map(PathBuf::from);
    let transcript_output = env::var_os(TRANSCRIPT_OUTPUT_ENV).map(PathBuf::from);
    let first = produce(
        &worker,
        &handoff,
        expectation.clone(),
        raw_output.as_deref(),
        transcript_output.as_deref(),
    );
    let second = produce(&worker, &handoff, expectation, None, None);
    assert_eq!(first.target().to_string(), "gfx942:xnack-");
    assert_eq!(first.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        first.finalized_output_identity(),
        second.finalized_output_identity()
    );
    assert_eq!(first.raw_output_identity(), second.raw_output_identity());
    assert_ne!(
        first.raw_output_identity(),
        first.finalized_output_identity()
    );
    assert!(first.canonical_descriptor_finalization_ran());
    assert!(first.exact_authenticated_compiler_handoff_was_checked());
    assert!(first.exact_machine_identity_was_checked());
    assert!(first.measured_ocml_provider_closure_was_checked());
    assert!(!first.proves_compiler_refinement());
    assert!(!first.proves_ocml_or_exponential_semantics());
    assert!(!first.proves_ieee_fp32_refinement());
    assert!(!first.proves_functional_flash_attention());
    assert!(!first.proves_gpu_execution_or_numerical_results());
    assert!(!first.proves_performance());
    assert!(!first.proves_no_comgr_linkage());
    assert!(!first.grants_publication_authority());
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());
}
