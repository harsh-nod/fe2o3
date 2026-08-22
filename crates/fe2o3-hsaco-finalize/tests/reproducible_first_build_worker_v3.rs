#![cfg(target_os = "linux")]

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, CompilerModuleHandoffErrorV3,
    CompilerModuleHandoffReceiptV3, CompilerModuleHandoffSlotV3, ConsumedCompilerModuleHandoffV3,
    ProducerIdentity, begin_build_attempt, consume_compiler_module_handoff_in_slot_v3,
    publish_compiler_module_handoff_in_slot_v3, publish_compiler_module_handoff_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3, INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3,
    INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3, INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3, InertFinalCompilerModuleCommitmentV3,
    InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkOptionV1, PinnedWorkerV1, ProtectedCompilerHandoffBindingErrorV3,
    ProtectedFirstBuildWorkerV3Error, WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1,
    execute_protected_reproducible_first_build_worker_v3,
};
use fe2o3_kernel_descriptor::DeviceTargetV1;
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const WORKER_ID: &str = "fixture-worker-v1";
const LLVM_ID: &str = "fixture-llvm-v1";
const OUTPUT: &[u8] = b"fixture-output";
const PROVIDER: &[u8] = b"exact V3 external provider";
const NATIVE_WORKER_ENV: &str = "FE2O3_SCALAR_GEMM_V1_WORKER";
const NATIVE_WORKER_BUILD_ID_ENV: &str = "FE2O3_SCALAR_GEMM_V1_WORKER_BUILD_ID";
const NATIVE_LLVM_BUILD_ID_ENV: &str = "FE2O3_SCALAR_GEMM_V1_LLVM_BUILD_ID";
const NATIVE_HANDOFF_ENV: &str = "FE2O3_SCALAR_GEMM_V1_HANDOFF";

const CAPSULE_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V3\0";
const PAIR_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V3\0";
const OUTER_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V3\0";
const INVOCATION_DIGEST_DOMAIN_V3: &[u8] = b"FE2O3/RUSTC-BUILD-INVOCATION/V3\0";
const CAPSULE_MAGIC_V3: &[u8; 8] = b"F2O3ISV3";
const CAPSULE_VERSION_V3: u16 = 3;

const RECEIPTS: [(&str, &[u8]); 14] = [
    (
        "inventory",
        b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-IDENTITY-INVENTORY/V3\0",
    ),
    (
        "preflight",
        b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-PREFLIGHT-PLAN/V3\0",
    ),
    (
        "mir",
        b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-SEMANTIC-MIR/V3\0",
    ),
    (
        "middle",
        b"FE2O3/INERT-LINEAGE-CONTENT/MIDDLE-END-PASS-CHAIN/V3\0",
    ),
    (
        "kir",
        b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-KERNEL-IR/V3\0",
    ),
    (
        "correspondence",
        b"FE2O3/INERT-LINEAGE-CONTENT/MIR-TO-KIR-CORRESPONDENCE/V3\0",
    ),
    (
        "memory",
        b"FE2O3/INERT-LINEAGE-CONTENT/FORMAL-MEMORY-OBLIGATIONS/V3\0",
    ),
    (
        "proof",
        b"FE2O3/INERT-LINEAGE-CONTENT/PROOF-BINDING-SET/V3\0",
    ),
    ("target", b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-BINDING/V3\0"),
    (
        "layout",
        b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-DATA-LAYOUT/V3\0",
    ),
    ("abi", b"FE2O3/INERT-LINEAGE-CONTENT/ABI/V3\0"),
    (
        "exports",
        b"FE2O3/INERT-LINEAGE-CONTENT/EXPORT-MANIFEST/V3\0",
    ),
    (
        "lowering",
        b"FE2O3/INERT-LINEAGE-CONTENT/AMDGPU-LOWERING/V3\0",
    ),
    (
        "semantic-llvm",
        b"FE2O3/INERT-LINEAGE-CONTENT/SEMANTIC-TO-LLVM/V3\0",
    ),
];
const FINAL_RECEIPT_DOMAIN_V3: &[u8] =
    b"FE2O3/INERT-LINEAGE-CONTENT/FINAL-COMPILER-MODULE-COMMITMENT/V3\0";

const INVOCATION_20_HEX: &str = "4645324f33524900030000007c02000000000000010021212121212121212121212121212121212121212121212121212121212121212222222222222222222222222222222222222222222222222222222222222222232323232323232323232323232323232323232323232323232323232323232324242424242424242424242424242424242424242424242424242424242424242525252525252525252525252525252525252525252525252525252525252525262626262626262626262626262626262626262626262626262626262626262624242424242424242424242424242424242424242424242424242424242424242626262626262626262626262626262626262626262626262626262626262626100000002f776f726b73706163652f6665326f3307000000100000002f6f70742f6665326f332f72757374630c0000002d2d63726174652d6e616d650c000000776f726b65725f76335f3230230000006372617465732f776f726b65722d76332d666978747572652f7372632f6c69622e7273100000002d2d63726174652d747970653d6c69620e0000002d2d65646974696f6e3d32303234360000002d5a636f646567656e2d6261636b656e643d2f6f70742f6665326f332f6c696272757374635f636f646567656e5f6665326f332e736f040000001500434152474f5f4346475f5441524745545f4152434806000000616d6467636e0f004645324f335f485341434f5f4449521d0000002f776f726b73706163652f6665326f332f7461726765742f6665326f330c004645324f335f5441524745540d0000006766783934323a786e61636b2d16004645324f335f5645524946595f4b45524e454c5f49520100000031";
const INVOCATION_40_HEX: &str = "4645324f33524900030000007c02000000000000010041414141414141414141414141414141414141414141414141414141414141414242424242424242424242424242424242424242424242424242424242424242434343434343434343434343434343434343434343434343434343434343434344444444444444444444444444444444444444444444444444444444444444444545454545454545454545454545454545454545454545454545454545454545464646464646464646464646464646464646464646464646464646464646464644444444444444444444444444444444444444444444444444444444444444444646464646464646464646464646464646464646464646464646464646464646100000002f776f726b73706163652f6665326f3307000000100000002f6f70742f6665326f332f72757374630c0000002d2d63726174652d6e616d650c000000776f726b65725f76335f3430230000006372617465732f776f726b65722d76332d666978747572652f7372632f6c69622e7273100000002d2d63726174652d747970653d6c69620e0000002d2d65646974696f6e3d32303234360000002d5a636f646567656e2d6261636b656e643d2f6f70742f6665326f332f6c696272757374635f636f646567656e5f6665326f332e736f040000001500434152474f5f4346475f5441524745545f4152434806000000616d6467636e0f004645324f335f485341434f5f4449521d0000002f776f726b73706163652f6665326f332f7461726765742f6665326f330c004645324f335f5441524745540d0000006766783934323a786e61636b2d16004645324f335f5645524946595f4b45524e454c5f49520100000031";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v3-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(TARGET).unwrap()
}

fn worker_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-executor-fixture"))
}

fn pinned() -> PinnedWorkerV1 {
    let bytes = fs::read(worker_path()).unwrap();
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap();
    PinnedWorkerV1::open(worker_path(), measurement).unwrap()
}

fn limits() -> WorkerExecutionLimitsV1 {
    WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 1024).unwrap()
}

fn options() -> Vec<LinkOptionV1> {
    [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", "2"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect()
}

fn provider() -> WorkerInputV1 {
    WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, PROVIDER.to_vec()).unwrap()
}

fn module_handoff(seed: u8) -> CompilerModuleHandoffV2 {
    let module = format!(
        "; ModuleID = 'worker-v3-{seed:02x}'\ndefine amdgpu_kernel void @workflow_kernel() {{ ret void }}\n"
    );
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target(), CodeObjectVersion::V6)
            .unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "workflow_kernel"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "workflow_kernel.kd",
        ),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target(),
        CodeObjectVersion::V6,
        envelope,
        manifest,
        module.as_bytes(),
    )
    .unwrap()
}

fn receipt_payload(label: &str, seed: u8) -> Vec<u8> {
    format!("worker-v3/receipt/{label}/{seed:02x}").into_bytes()
}

fn invocation_bytes(seed: u8) -> Vec<u8> {
    let encoded = match seed {
        0x20 => INVOCATION_20_HEX,
        0x40 => INVOCATION_40_HEX,
        _ => panic!("unsupported strict invocation fixture seed {seed:#x}"),
    };
    assert_eq!(encoded.len() % 2, 0);
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn capsule_bytes(seed: u8, handoff: &CompilerModuleHandoffV2) -> Vec<u8> {
    let invocation = invocation_bytes(seed);
    let final_commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(handoff).unwrap();
    let mut receipts = RECEIPTS
        .iter()
        .map(|(label, domain)| (receipt_payload(label, seed), *domain))
        .collect::<Vec<_>>();
    receipts.push((
        final_commitment.canonical_bytes().to_vec(),
        FINAL_RECEIPT_DOMAIN_V3,
    ));

    let total_len = 24
        + 4
        + invocation.len()
        + 32
        + 2
        + TARGET.len()
        + receipts
            .iter()
            .map(|(payload, _)| 4 + payload.len() + 32)
            .sum::<usize>()
        + 32;
    let mut capsule = Vec::with_capacity(total_len);
    capsule.extend_from_slice(CAPSULE_MAGIC_V3);
    capsule.extend_from_slice(&CAPSULE_VERSION_V3.to_le_bytes());
    capsule.extend_from_slice(&0_u16.to_le_bytes());
    capsule.extend_from_slice(&(total_len as u64).to_le_bytes());
    capsule.extend_from_slice(&0_u32.to_le_bytes());
    push_blob(&mut capsule, &invocation);
    capsule.extend_from_slice(&identity(INVOCATION_DIGEST_DOMAIN_V3, &invocation));
    capsule.extend_from_slice(&(TARGET.len() as u16).to_le_bytes());
    capsule.extend_from_slice(TARGET.as_bytes());
    for (payload, domain) in receipts {
        push_blob(&mut capsule, &payload);
        capsule.extend_from_slice(&identity(domain, &payload));
    }
    let capsule_identity = identity(CAPSULE_IDENTITY_DOMAIN_V3, &capsule);
    capsule.extend_from_slice(&capsule_identity);
    assert_eq!(capsule.len(), total_len);
    capsule
}

fn outer(seed: u8, module_seed: u8) -> InertSemanticCompilerModuleHandoffV3 {
    let handoff = module_handoff(module_seed);
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes(seed, &handoff),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

fn push_blob(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("non-canonical fixture hex"),
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required native Worker V3 pin {name} is absent"))
}

fn raw_outer(capsule: &[u8], handoff: &[u8]) -> Vec<u8> {
    let capsule_sha256: [u8; 32] = capsule[capsule.len() - 32..].try_into().unwrap();
    let handoff_sha256: [u8; 32] = Sha256::digest(handoff).into();
    let mut pair = Vec::with_capacity(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);
    pair.extend_from_slice(&INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3);
    pair.extend_from_slice(&INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3.to_le_bytes());
    pair.extend_from_slice(&0_u16.to_le_bytes());
    pair.extend_from_slice(&(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 as u32).to_le_bytes());
    pair.extend_from_slice(&0_u32.to_le_bytes());
    pair.extend_from_slice(&capsule_sha256);
    pair.extend_from_slice(&(capsule.len() as u64).to_le_bytes());
    pair.extend_from_slice(&handoff_sha256);
    pair.extend_from_slice(&(handoff.len() as u64).to_le_bytes());
    let pair_identity = identity(PAIR_IDENTITY_DOMAIN_V3, &pair);
    pair.extend_from_slice(&pair_identity);
    assert_eq!(pair.len(), INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);

    let total_len = 40 + capsule.len() + handoff.len() + pair.len() + 32;
    let mut outer = Vec::with_capacity(total_len);
    outer.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3);
    outer.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3.to_le_bytes());
    outer.extend_from_slice(&0_u16.to_le_bytes());
    outer.extend_from_slice(&(total_len as u64).to_le_bytes());
    outer.extend_from_slice(&0_u32.to_le_bytes());
    outer.extend_from_slice(&(capsule.len() as u64).to_le_bytes());
    outer.extend_from_slice(&(handoff.len() as u64).to_le_bytes());
    outer.extend_from_slice(capsule);
    outer.extend_from_slice(handoff);
    outer.extend_from_slice(&pair);
    let outer_identity = identity(OUTER_IDENTITY_DOMAIN_V3, &outer);
    outer.extend_from_slice(&outer_identity);
    assert_eq!(outer.len(), total_len);
    outer
}

fn producer() -> ProducerIdentity {
    ProducerIdentity::from_codegen(
        "worker_v3_fixture",
        Some(Path::new("src/worker_v3_fixture.rs")),
    )
    .unwrap()
}

fn begin(directory: &TestDirectory, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        &directory.0,
        &producer(),
        BuildInvocation::from_bytes([seed; 32]),
        BuildSession::from_bytes([seed.wrapping_add(1); 16]),
    )
    .unwrap()
}

fn consumed(
    directory: &TestDirectory,
    attempt_seed: u8,
    slot: CompilerModuleHandoffSlotV3,
    invocation_seed: u8,
    module_seed: u8,
) -> (
    BuildAttempt,
    CompilerModuleHandoffReceiptV3,
    ConsumedCompilerModuleHandoffV3,
) {
    let attempt = begin(directory, attempt_seed);
    let handoff = outer(invocation_seed, module_seed);
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        slot,
        &handoff,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        slot,
        handoff.identity(),
    )
    .unwrap();
    (attempt, receipt, consumed)
}

#[test]
fn consumed_v3_executes_natively_and_retains_every_exact_axis() {
    let directory = TestDirectory::new();
    let (attempt, receipt, consumed) = consumed(
        &directory,
        0x61,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x11,
    );
    let transaction = consumed.transaction_identity();
    let outer_identity = consumed.handoff_identity();
    let receipt_byte_len = receipt.length() as u64;
    let parent_closure = *consumed.handoff().capsule().compiler_closure();
    let execution_limits = limits();
    let evidence = execute_protected_reproducible_first_build_worker_v3(
        consumed,
        receipt,
        parent_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        execution_limits,
    )
    .unwrap();

    let expected = evidence.binding().expectation();
    assert_eq!(expected.attempt(), attempt);
    assert_eq!(expected.slot(), CompilerModuleHandoffSlotV3::Default);
    assert_eq!(expected.transaction_identity(), transaction);
    assert_eq!(expected.receipt_byte_len(), receipt_byte_len);
    assert_eq!(expected.outer_handoff_identity(), outer_identity);
    let capsule_identity = evidence.handoff().capsule().identity();
    assert_eq!(expected.capsule_sha256(), *capsule_identity.sha256());
    assert_eq!(expected.capsule_byte_len(), capsule_identity.byte_len());
    assert_eq!(
        expected.nested_handoff_identity(),
        evidence.handoff().module_handoff().identity()
    );
    assert_eq!(
        expected.invocation_digest(),
        *evidence.handoff().capsule().invocation_digest().as_bytes()
    );
    let pair_identity = evidence.handoff().pair_binding().identity();
    assert_eq!(expected.pair_binding_sha256(), *pair_identity.sha256());
    assert_eq!(expected.pair_binding_byte_len(), pair_identity.byte_len());
    let final_receipt = evidence
        .handoff()
        .capsule()
        .receipts()
        .final_compiler_module_commitment();
    let final_receipt_identity = final_receipt.identity();
    assert_eq!(
        expected.final_commitment_receipt_sha256(),
        *final_receipt_identity.sha256()
    );
    assert_eq!(
        expected.final_commitment_receipt_byte_len(),
        final_receipt_identity.byte_len()
    );
    let final_commitment =
        InertFinalCompilerModuleCommitmentV3::decode(final_receipt.canonical_preimage()).unwrap();
    assert_eq!(
        expected.final_commitment_sha256(),
        *final_commitment.identity().sha256()
    );
    assert_eq!(
        expected.final_commitment_byte_len(),
        final_commitment.identity().byte_len()
    );
    assert_eq!(
        expected.compiler_closure(),
        *evidence.handoff().capsule().compiler_closure()
    );
    assert_eq!(evidence.worker_measurement().llvm_build_identity(), LLVM_ID);
    assert_eq!(evidence.execution_limits(), execution_limits);
    assert_eq!(&evidence.bootstrap_request_bytes()[..8], b"F3LREQ02");
    assert_eq!(&evidence.exact_replay_request_bytes()[..8], b"F3LREQ02");
    assert_ne!(
        evidence.bootstrap().response().request_id(),
        evidence.exact_replay().response().request_id()
    );
    assert_eq!(evidence.output_bytes(), OUTPUT);
    assert_eq!(
        evidence.output_identity(),
        ContentIdentityV1::calculate(OUTPUT)
    );
    assert!(!evidence.authenticates_compiler_origin());
    assert!(!evidence.grants_compiler_authority());
    assert!(!evidence.grants_link_authority());
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
    for exact_bytes in [
        evidence.bootstrap_request_bytes(),
        evidence.exact_replay_request_bytes(),
        evidence.bootstrap().response().canonical_bytes(),
        evidence.exact_replay().response().canonical_bytes(),
    ] {
        assert!(!exact_bytes.windows(5).any(|window| window == b"comgr"));
        assert!(!exact_bytes.windows(5).any(|window| window == b"COMGR"));
    }
}

#[test]
fn invocation_tamper_and_final_commitment_splice_fail_before_worker_execution() {
    let handoff = module_handoff(0x31);
    let mut capsule = capsule_bytes(0x20, &handoff);
    let marker = b"worker_v3_20";
    let marker_offset = capsule
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap();
    capsule[marker_offset + marker.len() - 1] ^= 1;
    let capsule_terminal = capsule.len() - 32;
    let changed_capsule_identity =
        identity(CAPSULE_IDENTITY_DOMAIN_V3, &capsule[..capsule_terminal]);
    capsule[capsule_terminal..].copy_from_slice(&changed_capsule_identity);
    assert!(
        InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
            &capsule,
            handoff.canonical_bytes()
        ))
        .is_err()
    );

    let stale_capsule = capsule_bytes(0x20, &module_handoff(0x41));
    let replacement = module_handoff(0x42);
    assert!(
        InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
            &stale_capsule,
            replacement.canonical_bytes()
        ))
        .is_err()
    );
}

#[test]
fn attempt_slot_transaction_capsule_closure_and_nested_module_change_all_v3_identities() {
    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let (_, first_receipt, first) = consumed(
        &first_directory,
        0x71,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x51,
    );
    let (_, second_receipt, second) = consumed(
        &second_directory,
        0x72,
        CompilerModuleHandoffSlotV3::GeneralGemmReference,
        0x40,
        0x52,
    );
    let first_closure = *first.handoff().capsule().compiler_closure();
    let second_closure = *second.handoff().capsule().compiler_closure();
    let first = execute_protected_reproducible_first_build_worker_v3(
        first,
        first_receipt,
        first_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();
    let second = execute_protected_reproducible_first_build_worker_v3(
        second,
        second_receipt,
        second_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    let first_expected = first.binding().expectation();
    let second_expected = second.binding().expectation();
    assert_ne!(first_expected.attempt(), second_expected.attempt());
    assert_ne!(first_expected.slot(), second_expected.slot());
    assert_ne!(
        first_expected.transaction_identity(),
        second_expected.transaction_identity()
    );
    assert_ne!(
        first_expected.outer_handoff_identity(),
        second_expected.outer_handoff_identity()
    );
    assert_ne!(
        first_expected.capsule_sha256(),
        second_expected.capsule_sha256()
    );
    assert_ne!(
        first_expected.invocation_digest(),
        second_expected.invocation_digest()
    );
    assert_ne!(
        first_expected.compiler_closure(),
        second_expected.compiler_closure()
    );
    assert_ne!(
        first_expected.nested_handoff_identity(),
        second_expected.nested_handoff_identity()
    );
    assert_ne!(first.binding().identity(), second.binding().identity());
    assert_ne!(first.identity(), second.identity());
    assert_ne!(
        first.bootstrap().response().request_id(),
        second.bootstrap().response().request_id()
    );
    assert_ne!(
        first.exact_replay().response().request_id(),
        second.exact_replay().response().request_id()
    );
}

#[test]
fn parent_attempt_slot_transaction_outer_and_closure_substitutions_fail_closed() {
    let expected_directory = TestDirectory::new();
    let (_, expected_receipt, expected_consumed) = consumed(
        &expected_directory,
        0x73,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x53,
    );
    let expected_closure = *expected_consumed.handoff().capsule().compiler_closure();

    let attempt_directory = TestDirectory::new();
    let (_, _, attempt_substitution) = consumed(
        &attempt_directory,
        0x74,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x53,
    );
    assert!(matches!(
        execute_protected_reproducible_first_build_worker_v3(
            attempt_substitution,
            expected_receipt,
            expected_closure,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(ProtectedFirstBuildWorkerV3Error::Binding(
            ProtectedCompilerHandoffBindingErrorV3::RelationshipMismatch {
                field: "parent build attempt"
            }
        ))
    ));

    let slot_directory = TestDirectory::new();
    let (_, _, slot_substitution) = consumed(
        &slot_directory,
        0x73,
        CompilerModuleHandoffSlotV3::GeneralGemmReference,
        0x20,
        0x53,
    );
    assert!(matches!(
        execute_protected_reproducible_first_build_worker_v3(
            slot_substitution,
            expected_receipt,
            expected_closure,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(ProtectedFirstBuildWorkerV3Error::Binding(
            ProtectedCompilerHandoffBindingErrorV3::RelationshipMismatch {
                field: "parent V3 slot"
            }
        ))
    ));

    let outer_directory = TestDirectory::new();
    let (_, outer_receipt, outer_substitution) = consumed(
        &outer_directory,
        0x73,
        CompilerModuleHandoffSlotV3::Default,
        0x40,
        0x54,
    );
    let outer_closure = *outer_substitution.handoff().capsule().compiler_closure();
    assert!(matches!(
        execute_protected_reproducible_first_build_worker_v3(
            outer_substitution,
            expected_receipt,
            outer_closure,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(ProtectedFirstBuildWorkerV3Error::Binding(
            ProtectedCompilerHandoffBindingErrorV3::RelationshipMismatch {
                field: "parent V3 transaction identity"
            }
        ))
    ));

    let closure_directory = TestDirectory::new();
    let (_, _, closure_substitution) = consumed(
        &closure_directory,
        0x73,
        CompilerModuleHandoffSlotV3::Default,
        0x40,
        0x54,
    );
    assert!(matches!(
        execute_protected_reproducible_first_build_worker_v3(
            closure_substitution,
            outer_receipt,
            expected_closure,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(ProtectedFirstBuildWorkerV3Error::Binding(
            ProtectedCompilerHandoffBindingErrorV3::RelationshipMismatch {
                field: "parent compiler closure"
            }
        ))
    ));
}

#[test]
fn identical_v3_inputs_produce_deterministic_execution_evidence() {
    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let (_, first_receipt, first) = consumed(
        &first_directory,
        0x75,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x55,
    );
    let (_, second_receipt, second) = consumed(
        &second_directory,
        0x75,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x55,
    );
    let first_closure = *first.handoff().capsule().compiler_closure();
    let second_closure = *second.handoff().capsule().compiler_closure();
    let first = execute_protected_reproducible_first_build_worker_v3(
        first,
        first_receipt,
        first_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();
    let second = execute_protected_reproducible_first_build_worker_v3(
        second,
        second_receipt,
        second_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    assert_eq!(first.binding(), second.binding());
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.bootstrap_request_bytes(),
        second.bootstrap_request_bytes()
    );
    assert_eq!(
        first.exact_replay_request_bytes(),
        second.exact_replay_request_bytes()
    );
    assert_eq!(
        first.bootstrap().response().canonical_bytes(),
        second.bootstrap().response().canonical_bytes()
    );
    assert_eq!(
        first.exact_replay().response().canonical_bytes(),
        second.exact_replay().response().canonical_bytes()
    );
}

#[test]
fn execution_limits_are_retained_and_change_only_the_complete_evidence_identity() {
    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let (_, first_receipt, first) = consumed(
        &first_directory,
        0x7a,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x5a,
    );
    let (_, second_receipt, second) = consumed(
        &second_directory,
        0x7a,
        CompilerModuleHandoffSlotV3::Default,
        0x20,
        0x5a,
    );
    let first_closure = *first.handoff().capsule().compiler_closure();
    let second_closure = *second.handoff().capsule().compiler_closure();
    let first_limits = limits();
    let second_limits =
        WorkerExecutionLimitsV1::new(Duration::from_secs(3), 16 * 1024, 1024).unwrap();
    let first = execute_protected_reproducible_first_build_worker_v3(
        first,
        first_receipt,
        first_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        first_limits,
    )
    .unwrap();
    let second = execute_protected_reproducible_first_build_worker_v3(
        second,
        second_receipt,
        second_closure,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        second_limits,
    )
    .unwrap();

    assert_eq!(first.binding(), second.binding());
    assert_eq!(first.plan(), second.plan());
    assert_eq!(first.output_bytes(), second.output_bytes());
    assert_eq!(first.execution_limits(), first_limits);
    assert_eq!(second.execution_limits(), second_limits);
    assert_ne!(first.identity(), second.identity());
}

#[test]
fn consumed_transaction_cannot_be_replayed_and_v3_entry_has_no_legacy_parameter() {
    let directory = TestDirectory::new();
    let attempt = begin(&directory, 0x7a);
    let handoff = outer(0x20, 0x61);
    publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        CompilerModuleHandoffSlotV3::Default,
        &handoff,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        CompilerModuleHandoffSlotV3::Default,
        handoff.identity(),
    )
    .unwrap();
    assert_eq!(consumed.attempt(), attempt);
    assert!(matches!(
        consume_compiler_module_handoff_in_slot_v3(
            &directory.0,
            &producer(),
            attempt,
            CompilerModuleHandoffSlotV3::Default,
            handoff.identity(),
        ),
        Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
    ));

    let v2_directory = TestDirectory::new();
    let v2_attempt = begin(&v2_directory, 0x7b);
    let v2_handoff = module_handoff(0x62);
    let closure = *handoff.capsule().compiler_closure();
    publish_compiler_module_handoff_v2(
        &v2_directory.0,
        &producer(),
        v2_attempt,
        closure,
        v2_handoff.canonical_bytes(),
    )
    .unwrap();
    assert!(matches!(
        consume_compiler_module_handoff_in_slot_v3(
            &v2_directory.0,
            &producer(),
            v2_attempt,
            CompilerModuleHandoffSlotV3::Default,
            handoff.identity(),
        ),
        Err(CompilerModuleHandoffErrorV3::NotPublished)
    ));

    #[allow(clippy::type_complexity)]
    let _entry: fn(
        ConsumedCompilerModuleHandoffV3,
        CompilerModuleHandoffReceiptV3,
        CompilerClosureV2,
        &PinnedWorkerV1,
        Vec<WorkerInputV1>,
        Vec<LinkOptionV1>,
        WorkerOutputConstraintsV1,
        WorkerExecutionLimitsV1,
    ) -> Result<_, _> = execute_protected_reproducible_first_build_worker_v3;
}

#[test]
#[ignore = "requires the measured upstream LLVM/LLD worker and rustc-produced gfx942 handoff"]
fn configured_upstream_llvm_worker_executes_the_native_v3_path() {
    let nested_bytes = fs::read(required_env(NATIVE_HANDOFF_ENV)).unwrap();
    let nested = CompilerModuleHandoffV2::decode(&nested_bytes).unwrap();
    assert_eq!(nested.canonical_bytes(), nested_bytes);
    assert_eq!(nested.target().to_string(), TARGET);

    let worker_path = PathBuf::from(required_env(NATIVE_WORKER_ENV));
    let worker_bytes = fs::read(&worker_path).unwrap();
    let llvm_build_identity = required_env(NATIVE_LLVM_BUILD_ID_ENV);
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&worker_bytes),
        required_env(NATIVE_WORKER_BUILD_ID_ENV),
        llvm_build_identity.clone(),
    )
    .unwrap();
    let worker = PinnedWorkerV1::open(worker_path, measurement).unwrap();

    let directory = TestDirectory::new();
    let attempt = begin(&directory, 0x7c);
    let handoff = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes(0x20, &nested),
        nested.canonical_bytes(),
    ))
    .unwrap();
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        CompilerModuleHandoffSlotV3::Default,
        &handoff,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        CompilerModuleHandoffSlotV3::Default,
        handoff.identity(),
    )
    .unwrap();
    let closure = *consumed.handoff().capsule().compiler_closure();
    let evidence = execute_protected_reproducible_first_build_worker_v3(
        consumed,
        receipt,
        closure,
        &worker,
        Vec::new(),
        options(),
        WorkerOutputConstraintsV1::new(64 * 1024 * 1024).unwrap(),
        WorkerExecutionLimitsV1::default(),
    )
    .unwrap();

    assert_eq!(
        evidence.worker_measurement().llvm_build_identity(),
        llvm_build_identity
    );
    assert_eq!(&evidence.output_bytes()[..4], b"\x7fELF");
    assert_eq!(
        evidence.bootstrap().response().output(),
        evidence.exact_replay().response().output()
    );
    assert!(evidence.output_identity().matches(evidence.output_bytes()));
}

fn identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    digest.finalize().into()
}
