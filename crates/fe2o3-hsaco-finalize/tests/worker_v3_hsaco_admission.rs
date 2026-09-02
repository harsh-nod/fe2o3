#![cfg(target_os = "linux")]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use dialect_amdgcn::{
    CanonicalProductionKirToLlvmReplayEvidenceV1, ProductionReplayKernelIrVersionV1,
    bind_production_llvm22_worker_layout_v1, bind_production_target_v1,
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1,
};
use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
    ProductionAmdTargetProfileV1,
};
use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationOutcomeV3, BuildInvocation, BuildSession,
    CompilerModuleHandoffReceiptV3, CompilerModuleHandoffSlotV3, ConsumedCompilerModuleHandoffV3,
    DurablePublishedHsacoClaimV3, ProducerIdentity, WorkerV3ExternalProviderPayloadsV1,
    WorkerV3PublicationIntentOutcomeV1, begin_build_attempt,
    consume_compiler_module_handoff_in_slot_v3, finish_build_attempt,
    publish_compiler_module_handoff_in_slot_v3, reacquire_current_hsaco_publication_lease_v3,
};
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CodeObjectVersion, CompilerDescriptorSourceV1,
    CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3, INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3,
    INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3, INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3, InertFinalCompilerModuleCommitmentV3,
    InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_compiler_lineage::{
    DataLayoutTranscriptInputsV3, DataLayoutTranscriptV3, InertAbiReceiptV3,
    InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirReceiptV3, InertDataLayoutReceiptV3,
    InertExportManifestReceiptV3, InertFinalCompilerModuleCommitmentReceiptV3,
    InertFormalMemoryReceiptV3, InertKernelIrReceiptV3, InertLineageContentIdentityV3,
    InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProductionSemanticCapsuleV3, InertProofBindingAssociationInputsV4,
    InertProofBindingAssociationV4, InertProofBindingReceiptV3,
    InertRustcIdentityInventoryReceiptV3, InertRustcPreflightPlanReceiptV3,
    InertSemanticToLlvmAssociationInputsV3, InertSemanticToLlvmAssociationV3,
    InertSemanticToLlvmContentIdentityV3, InertSemanticToLlvmReceiptV3,
    InertTargetBindingReceiptV3, OrderedInertSemanticLineageReceiptsV3,
    SemanticToLlvmAssociationInputsV3, SemanticToLlvmAssociationTranscriptV3,
    TargetBindingTranscriptInputsV3, TargetBindingTranscriptV3, TargetLineageIdentityV3,
    derive_semantic_target_layout_identity_v1,
};
use fe2o3_hsaco_finalize::{
    CompilerClosureV2, ContentIdentityV1, FinalizedSemanticDebugMapAdmissionStatusV1,
    FinalizedSemanticDebugMapErrorV1, InertProductionKirV7StructuralBridgeV1,
    InertProductionSourceIsaCatalogV1, InertProtectedFirstBuildWorkerV3EvidenceV1,
    InspectedProtectedWorkerV3HsacoV1, LinkOptionV1, PinnedWorkerV1,
    ProductionFinalizedSemanticDebugAdmissionV1, ProductionIsaPointV1,
    ProductionKirV7BridgeAdmissionV1, ProductionKirV7BridgeCatalogQueryUnavailableV1,
    ProductionKirV7BridgeErrorV1, ProductionKirV7BridgeSiteV1, ProductionSemanticAnchorAdmissionV1,
    ProductionSemanticAnchorErrorV1, ProductionSourceIsaAcceptanceSummaryAdmissionV1,
    ProductionSourceIsaCatalogAdmissionV1, ProductionSourceIsaCatalogErrorV1,
    ProductionSourceIsaCatalogPointV1,
    ProductionSourceIsaCatalogRecordKindV1, ProductionSourceIsaCatalogTargetV1,
    ProductionSourceIsaCatalogTransformationV1, ProductionSourceIsaCorrelationAdmissionV1,
    ProductionSourceIsaCorrelationErrorV1, ProductionSourceIsaCorrelationUnavailableV1,
    ProductionSourceIsaRecordKindV1, ProtectedWorkerV3CompactFinalizerReplayV2,
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, WorkerV3HsacoFinalizationError, WorkerV3HsacoInspectionError,
    WorkerV3HsacoPublicationErrorV1, admit_production_kir_v7_structural_bridge_v1,
    execute_protected_reproducible_first_build_worker_v3, finalize_protected_worker_v3_hsaco_v1,
    inspect_protected_worker_v3_hsaco_v1, inspect_unfinalized,
    persist_prepared_protected_worker_v3_hsaco_publication_v1,
    prepare_protected_worker_v3_compact_finalizer_replay_v2,
    prepare_protected_worker_v3_hsaco_publication_v1,
    publish_recovered_protected_worker_v3_hsaco_v1,
    recover_protected_worker_v3_hsaco_publication_v1,
    revalidate_protected_worker_v3_finalizer_derivation_v1,
};
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest,
    CodeObjectVersion as DescriptorCodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1,
    DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest,
    EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1,
    LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1,
    SourceTypeRecordV1, Text, ValidName, encode_device_descriptor_table_v1,
};
use fe2o3_kernel_ir::{
    BinaryOp, CheckedBinaryOperator, DebugSourceMapBindingV1, DebugSourceMapDocumentV2,
    OperationKind, ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugCarrierV1,
    ProductionSemanticDebugFragmentErrorV1,
    ProductionSemanticDebugFragmentV1, ProductionSemanticDebugProducerCapabilityV1,
    ProductionSemanticDebugProducerGapV1, ProductionSemanticDebugReceiptExtensionV1,
    SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1, SemanticDebugBoundaryV1,
    SemanticDebugContentIdentityV1, SemanticDebugLayerV1, SemanticDebugLocationV1,
    SemanticDebugMapBindingV1, SemanticDebugMapDocumentV1, SemanticDebugMapErrorV1,
    SemanticDebugMappingOutputV1, SemanticDebugMappingV1, SemanticDebugNodeV1,
    SemanticDebugTransformationV1, SemanticDebugUnavailableReasonV1, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8, VerifiedCanonicalKernelIrV9,
};
use fe2o3_verifier::{
    CompilerTargetLineageValidationErrorV1, ValidatedCompilerTargetLineageV1,
    validate_compiler_proof_inputs_v4, validate_compiler_target_lineage_v1,
};
use object::{Object as _, ObjectSection as _};
use sha2::{Digest, Sha256};

#[path = "../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;
#[path = "fixtures/worker_v3_hsaco_test_support.rs"]
mod hsaco_fixture;
#[path = "../../../tests/support/production_semantic_debug_fixture_v1.rs"]
mod production_semantic_debug_fixture_v1;

use compiler_proof_inputs_v3::{
    ProductionSourceIsaKernelFamilyV1, canonical_compiler_proof_inputs_v4,
    canonical_compiler_proof_inputs_v4_with_sourceful_family,
    canonical_compiler_proof_inputs_v4_with_sourceful_induction,
    canonical_verus_execution_evidence_v1,
};
use hsaco_fixture::{
    ScalarAddFixtureMutation, scalar_add_fixture_with, slice_fixture_with_descriptor_table,
    slice_fixture_with_descriptor_table_and_workgroup,
    synthetic_two_kernel_slice_fixture_with_descriptor_table,
};
use production_semantic_debug_fixture_v1::{
    exact_source_mir_kir_carrier_v1, exact_source_mir_kir_carrier_with_projection_v1,
};

const TARGET: &str = "gfx942:xnack-";
// Measured for this test's one explicit i32, COV6, ROCm 7.2.4 LLVM 22 Worker signature.
const SCALAR_COV6_KERNARG_SEGMENT_BYTES: u32 = 264;
const WORKER_BUILD_ID: &str = "fixture-worker-v3-hsaco-v1";
const RAW_HSACO_MARKER: &[u8] = b"; FE2O3/TEST-HSACO-PAYLOAD/V2-HEX:";
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

#[derive(Clone, Copy)]
struct EvidenceConfig {
    attempt_seed: u8,
    slot: CompilerModuleHandoffSlotV3,
    invocation_seed: u8,
    module_seed: u8,
    optimization: &'static str,
    llvm_build_identity: &'static str,
    lineage_mutation: DescriptorLineageMutation,
}

#[derive(Clone, Copy)]
enum DescriptorLineageMutation {
    Exact,
    DifferentCanonicalSource,
    DifferentExportManifest,
}

impl EvidenceConfig {
    const BASE: Self = Self {
        attempt_seed: 0x61,
        slot: CompilerModuleHandoffSlotV3::Production,
        invocation_seed: 0x20,
        module_seed: 0x11,
        optimization: "2",
        llvm_build_identity: "upstream-llvm-test-build-a",
        lineage_mutation: DescriptorLineageMutation::Exact,
    };
}

#[allow(dead_code)]
pub(crate) struct PublishedWorkerV3Fixture {
    pub(crate) directory: TestDirectory,
    pub(crate) producer: ProducerIdentity,
    pub(crate) attempt: fe2o3_artifact_transaction::BuildAttempt,
    pub(crate) published: fe2o3_hsaco_finalize::PublishedProtectedWorkerV3HsacoV1,
}

#[allow(dead_code)]
pub(crate) struct PublishedWorkerV3InDirectory {
    pub(crate) producer: ProducerIdentity,
    pub(crate) attempt: fe2o3_artifact_transaction::BuildAttempt,
    pub(crate) published: fe2o3_hsaco_finalize::PublishedProtectedWorkerV3HsacoV1,
}

#[allow(dead_code)]
pub(crate) fn published_worker_v3_fixture() -> PublishedWorkerV3Fixture {
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    published_worker_v3_fixture_from_raw_hsaco(fixture.bytes, "vecadd", "vecadd.kd")
}

#[allow(dead_code)]
pub(crate) fn published_worker_v3_fixture_with_llvm_build_identity(
    llvm_build_identity: &'static str,
) -> PublishedWorkerV3Fixture {
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    published_worker_v3_fixture_from_raw_hsaco_for_kernels_with_config(
        fixture.bytes,
        &[("vecadd", "vecadd.kd")],
        EvidenceConfig {
            llvm_build_identity,
            ..EvidenceConfig::BASE
        },
    )
}

#[allow(dead_code)]
/// Publishes a hand-authored two-entry fixture; this is not compiler-produced provenance.
pub(crate) fn published_synthetic_two_kernel_worker_v3_fixture() -> PublishedWorkerV3Fixture {
    published_synthetic_two_kernel_worker_v3_fixture_with_llvm_build_identity(
        EvidenceConfig::BASE.llvm_build_identity,
    )
}

#[allow(dead_code)]
pub(crate) fn published_synthetic_two_kernel_worker_v3_fixture_with_llvm_build_identity(
    llvm_build_identity: &'static str,
) -> PublishedWorkerV3Fixture {
    let fixture = synthetic_two_kernel_slice_fixture_with_descriptor_table(
        &synthetic_two_kernel_slice_descriptor_table(),
    );
    published_worker_v3_fixture_from_raw_hsaco_for_kernels_with_config(
        fixture.bytes,
        &[
            ("synthetic_first_transform", "synthetic_first_transform.kd"),
            (
                "synthetic_second_transform",
                "synthetic_second_transform.kd",
            ),
        ],
        EvidenceConfig {
            llvm_build_identity,
            ..EvidenceConfig::BASE
        },
    )
}

#[allow(dead_code)]
pub(crate) fn published_worker_v3_fixture_from_raw_hsaco(
    raw_hsaco: Vec<u8>,
    entry_symbol: &str,
    descriptor_symbol: &str,
) -> PublishedWorkerV3Fixture {
    published_worker_v3_fixture_from_raw_hsaco_for_kernels_with_config(
        raw_hsaco,
        &[(entry_symbol, descriptor_symbol)],
        EvidenceConfig::BASE,
    )
}

fn published_worker_v3_fixture_from_raw_hsaco_for_kernels_with_config(
    raw_hsaco: Vec<u8>,
    kernel_symbols: &[(&str, &str)],
    config: EvidenceConfig,
) -> PublishedWorkerV3Fixture {
    let directory = TestDirectory::new();
    let staged = publish_worker_v3_fixture_in_directory_for_kernels_with_config(
        &directory,
        raw_hsaco,
        kernel_symbols,
        config,
    );
    PublishedWorkerV3Fixture {
        directory,
        producer: staged.producer,
        attempt: staged.attempt,
        published: staged.published,
    }
}

#[allow(dead_code)]
pub(crate) fn publish_worker_v3_fixture_in_directory(
    directory: &TestDirectory,
    attempt_seed: u8,
) -> PublishedWorkerV3InDirectory {
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    publish_worker_v3_fixture_in_directory_for_kernels_with_config(
        directory,
        fixture.bytes,
        &[("vecadd", "vecadd.kd")],
        EvidenceConfig {
            attempt_seed,
            ..EvidenceConfig::BASE
        },
    )
}

fn publish_worker_v3_fixture_in_directory_for_kernels_with_config(
    directory: &TestDirectory,
    raw_hsaco: Vec<u8>,
    kernel_symbols: &[(&str, &str)],
    config: EvidenceConfig,
) -> PublishedWorkerV3InDirectory {
    let producer = producer();
    let (attempt, source) = evidence_in_directory_for_kernels_and_providers(
        directory,
        raw_hsaco,
        config,
        kernel_symbols,
        Vec::new(),
    );
    let inspected = inspect_protected_worker_v3_hsaco_v1(source).unwrap();
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).unwrap();
    let prepared = prepare_protected_worker_v3_hsaco_publication_v1(&producer, finalized).unwrap();
    let persisted = persist_prepared_protected_worker_v3_hsaco_publication_v1(
        &directory.0,
        &producer,
        prepared,
    )
    .unwrap();
    let compiler_closure = persisted
        .finalized_evidence()
        .binding_expectation()
        .compiler_closure();
    let published = publish_recovered_protected_worker_v3_hsaco_v1(
        &directory.0,
        &producer,
        compiler_closure,
        persisted,
    )
    .unwrap();
    PublishedWorkerV3InDirectory {
        producer,
        attempt,
        published,
    }
}

#[test]
fn native_v3_inspection_retains_every_boundary_axis_without_authority() {
    let fixture = scalar_add_fixture_with(ScalarAddFixtureMutation::RequiredWorkgroup);
    let exact = fixture.bytes.clone();
    let evidence = evidence(fixture.bytes, EvidenceConfig::BASE);
    let source_identity = evidence.identity();
    let binding = evidence.binding();
    let expected = binding.expectation();
    let plan = evidence.plan().identity();

    let inspected = inspect_protected_worker_v3_hsaco_v1(evidence).unwrap();
    require_v3_inspection(&inspected);
    assert_eq!(inspected.source_evidence_identity(), source_identity);
    assert_eq!(inspected.binding_identity(), binding.identity());
    assert_eq!(inspected.binding_expectation(), expected);
    assert_eq!(inspected.attempt(), expected.attempt());
    assert_eq!(inspected.handoff_slot(), expected.slot());
    assert_eq!(
        inspected.transaction_identity(),
        expected.transaction_identity()
    );
    assert_eq!(
        inspected.outer_handoff_identity(),
        expected.outer_handoff_identity()
    );
    assert_eq!(
        inspected.outer_handoff().identity(),
        expected.outer_handoff_identity()
    );
    assert_eq!(inspected.compiler_closure(), expected.compiler_closure());
    assert_eq!(inspected.link_plan_identity(), plan);
    assert_eq!(inspected.exact_bytes(), exact);
    assert_eq!(
        inspected.raw_hsaco_identity(),
        ContentIdentityV1::calculate(&exact)
    );
    assert_eq!(
        inspected.linked_output_identity(),
        inspected.raw_hsaco_identity()
    );
    assert_eq!(inspected.target().to_string(), TARGET);
    assert_eq!(
        inspected.code_object_version(),
        fe2o3_kernel_descriptor::CodeObjectVersion::V6
    );
    assert_eq!(
        inspected.policy().launch().required_workgroup_size(),
        [64, 1, 1]
    );
    assert_eq!(inspected.policy().launch().wavefront_size(), 64);
    assert!(!inspected.descriptor_observation_preimage().is_empty());
    assert!(!inspected.abi_observation_preimage().is_empty());
    assert!(!inspected.resource_observation_preimage().is_empty());
    assert_eq!(inspected.source_evidence().identity(), source_identity);
    assert!(!inspected.canonical_descriptor_finalization_ran());
    assert!(!inspected.authenticates_compiler_origin());
    assert!(!inspected.proves_semantic_correctness());
    assert!(!inspected.grants_compiler_authority());
    assert!(!inspected.grants_link_authority());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
}

#[test]
fn borrowed_finalizer_revalidation_retains_exact_stage_custody_without_payloads_or_authority() {
    let directory = TestDirectory::new();
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let (attempt, evidence) = evidence_in_directory_for_kernel(
        &directory,
        fixture.bytes,
        EvidenceConfig::BASE,
        "vecadd",
        "vecadd.kd",
    );
    let source = evidence.identity();
    let binding = evidence.binding().identity();
    let worker = evidence.worker_measurement().clone();
    let compiler_module =
        ContentIdentityV1::calculate(evidence.handoff().module_handoff().module_bytes());
    let link_plan = evidence.plan().identity();
    let derivation = evidence.derivation_evidence().clone();
    let raw_hsaco = evidence.output_identity();
    let inspected = inspect_protected_worker_v3_hsaco_v1(evidence).unwrap();
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).unwrap();
    let finalization = finalized.identity();
    let finalized_hsaco = finalized.finalized_output_identity();
    let prepared = prepare_protected_worker_v3_compact_finalizer_replay_v2(finalized).unwrap();
    let parts = prepared.into_parts();
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&parts.transcript).unwrap();

    let revalidated = revalidate_protected_worker_v3_finalizer_derivation_v1(
        attempt,
        &parts.outer_handoff,
        parts.external_provider_payloads.iter().map(Vec::as_slice),
        &parts.transcript,
        &parts.finalized_hsaco,
    )
    .unwrap();
    assert_eq!(revalidated.transcript_identity(), transcript.identity());
    assert_eq!(revalidated.source_evidence_identity(), source);
    assert_eq!(revalidated.binding_identity(), binding);
    assert_eq!(revalidated.worker_measurement(), &worker);
    assert_eq!(revalidated.compiler_module_identity(), compiler_module);
    assert_eq!(revalidated.link_plan_identity(), link_plan);
    assert_eq!(revalidated.derivation_evidence(), &derivation);
    assert_eq!(revalidated.raw_hsaco_identity(), raw_hsaco);
    assert_eq!(revalidated.finalization_identity(), finalization);
    assert_eq!(revalidated.finalized_hsaco_identity(), finalized_hsaco);
    assert!(!revalidated.proves_llvm_to_machine_semantic_refinement());
    assert!(!revalidated.grants_compiler_authority());
    assert!(!revalidated.grants_publication_authority());
    assert!(!revalidated.grants_load_authority());
    assert!(!revalidated.grants_launch_authority());

    let mut corrupt_transcript = parts.transcript.clone();
    *corrupt_transcript.last_mut().unwrap() ^= 1;
    assert!(matches!(
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            attempt,
            &parts.outer_handoff,
            parts.external_provider_payloads.iter().map(Vec::as_slice),
            &corrupt_transcript,
            &parts.finalized_hsaco,
        ),
        Err(WorkerV3HsacoPublicationErrorV1::CompactReplay(_))
    ));

    let mut corrupt_outer = parts.outer_handoff.clone();
    corrupt_outer[0] ^= 1;
    assert!(matches!(
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            attempt,
            &corrupt_outer,
            parts.external_provider_payloads.iter().map(Vec::as_slice),
            &parts.transcript,
            &parts.finalized_hsaco,
        ),
        Err(WorkerV3HsacoPublicationErrorV1::OuterHandoff(_))
    ));

    let mut corrupt_finalized = parts.finalized_hsaco.clone();
    corrupt_finalized[0] ^= 1;
    assert!(
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            attempt,
            &parts.outer_handoff,
            parts.external_provider_payloads.iter().map(Vec::as_slice),
            &parts.transcript,
            &corrupt_finalized,
        )
        .is_err()
    );
}

#[test]
fn strict_v3_inspection_derives_wg256_from_the_bound_descriptor() {
    let directory = TestDirectory::new();
    let descriptor = slice_descriptor_table_with_workgroup(256);
    let fixture = slice_fixture_with_descriptor_table_and_workgroup(&descriptor, 256);
    let (_, source) = evidence_in_directory_for_kernel(
        &directory,
        fixture.bytes,
        EvidenceConfig::BASE,
        "vecadd",
        "vecadd.kd",
    );

    let inspected = inspect_protected_worker_v3_hsaco_v1(source).unwrap();

    assert_eq!(
        inspected.policy().launch().required_workgroup_size(),
        [256, 1, 1]
    );
    assert_eq!(inspected.policy().launch().max_flat_workgroup_size(), 256);
}

#[test]
fn native_v3_finalization_fails_closed_without_descriptor_source_evidence() {
    let raw = inspected(
        scalar_add_fixture_with(ScalarAddFixtureMutation::RequiredWorkgroup).bytes,
        EvidenceConfig::BASE,
    );
    let raw_identity = raw.identity();
    let source_identity = raw.source_evidence_identity();
    let binding = raw.binding_identity();
    let expected = raw.binding_expectation();
    let raw_output = raw.raw_hsaco_identity();
    let blocker = match finalize_protected_worker_v3_hsaco_v1(raw) {
        Err(
            WorkerV3HsacoFinalizationError::MissingAuthenticatedProtectedDescriptorSourceEvidenceV3(
                blocker,
            ),
        ) => blocker,
        result => panic!("expected native V3 descriptor-source blocker, found {result:?}"),
    };

    assert_eq!(blocker.raw_inspection_identity(), raw_identity);
    assert_eq!(blocker.source_evidence_identity(), source_identity);
    assert_eq!(blocker.binding_identity(), binding);
    assert_eq!(blocker.binding_expectation(), expected);
    assert_eq!(blocker.attempt(), expected.attempt());
    assert_eq!(blocker.handoff_slot(), expected.slot());
    assert_eq!(
        blocker.transaction_identity(),
        expected.transaction_identity()
    );
    assert_eq!(
        blocker.outer_handoff_identity(),
        expected.outer_handoff_identity()
    );
    assert_eq!(blocker.compiler_closure(), expected.compiler_closure());
    assert_eq!(blocker.raw_output_identity(), raw_output);
    assert!(!blocker.may_infer_descriptor_claims_from_executable_metadata());
    assert!(!blocker.grants_publication_authority());
    assert!(!blocker.grants_load_authority());
    assert!(!blocker.grants_launch_authority());
}

#[test]
fn native_v3_finalization_rejects_a_different_canonical_descriptor_source() {
    let directory = TestDirectory::new();
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let (_, source) = evidence_in_directory_for_kernel(
        &directory,
        fixture.bytes,
        EvidenceConfig {
            lineage_mutation: DescriptorLineageMutation::DifferentCanonicalSource,
            ..EvidenceConfig::BASE
        },
        "vecadd",
        "vecadd.kd",
    );
    let raw = inspect_protected_worker_v3_hsaco_v1(source).unwrap();
    assert!(matches!(
        finalize_protected_worker_v3_hsaco_v1(raw),
        Err(WorkerV3HsacoFinalizationError::CompilerDescriptorSourceMismatch)
    ));
}

#[test]
fn native_v3_finalization_rejects_a_different_export_manifest_receipt() {
    let directory = TestDirectory::new();
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let (_, source) = evidence_in_directory_for_kernel(
        &directory,
        fixture.bytes,
        EvidenceConfig {
            lineage_mutation: DescriptorLineageMutation::DifferentExportManifest,
            ..EvidenceConfig::BASE
        },
        "vecadd",
        "vecadd.kd",
    );
    let raw = inspect_protected_worker_v3_hsaco_v1(source).unwrap();
    assert!(matches!(
        finalize_protected_worker_v3_hsaco_v1(raw),
        Err(WorkerV3HsacoFinalizationError::ExportManifestMismatch)
    ));
}

#[test]
fn native_v3_finalizer_admits_only_the_exact_artifact_and_bounded_isa_interval() {
    let directory = TestDirectory::new();
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let (_, source) = evidence_in_directory_for_kernel(
        &directory,
        fixture.bytes,
        EvidenceConfig::BASE,
        "vecadd",
        "vecadd.kd",
    );
    let inspected = inspect_protected_worker_v3_hsaco_v1(source).unwrap();
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).unwrap();
    let exact_artifact = finalized.exact_finalized_bytes();

    let map = finalizer_semantic_map(exact_artifact, 4);
    let admitted = finalized.admit_semantic_debug_map_v1(&map).unwrap();
    assert_eq!(
        admitted.artifact_identity(),
        finalized.finalized_output_identity()
    );
    assert_eq!(
        admitted.admission_status(),
        FinalizedSemanticDebugMapAdmissionStatusV1::ArtifactOnly
    );
    assert!(!admitted.validates_all_input_axes());
    assert!(!admitted.authenticates_compiler_execution());
    assert!(!admitted.grants_publication_authority());

    let stale = finalizer_semantic_map(b"substituted-hsaco", 4);
    assert!(matches!(
        finalized.admit_semantic_debug_map_v1(&stale),
        Err(
            fe2o3_hsaco_finalize::FinalizedSemanticDebugMapErrorV1::SemanticMap(
                SemanticDebugMapErrorV1::ArtifactBindingMismatch
            )
        )
    ));
    let outside_entry = finalizer_semantic_map(exact_artifact, 1_u64 << 32);
    assert!(matches!(
        finalized.admit_semantic_debug_map_v1(&outside_entry),
        Err(
            fe2o3_hsaco_finalize::FinalizedSemanticDebugMapErrorV1::SemanticMap(
                SemanticDebugMapErrorV1::InvalidIsaInterval
            )
        )
    ));
}

#[test]
fn production_semantic_debug_legacy_and_unavailable_states_are_typed() {
    let raw = slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes;
    let legacy = finalized_with_optional_semantic_debug(
        raw.clone(),
        OptionalSemanticDebugFixture::LegacyBare,
    );
    assert!(matches!(
        legacy.admit_production_semantic_debug_map_v1().unwrap(),
        ProductionFinalizedSemanticDebugAdmissionV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::LegacyBareAssociationNoAttachment
        )
    ));

    let unavailable = finalized_with_optional_semantic_debug(
        raw,
        OptionalSemanticDebugFixture::Unavailable(
            ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable,
        ),
    );
    assert!(matches!(
        unavailable
            .admit_production_semantic_debug_map_v1()
            .unwrap(),
        ProductionFinalizedSemanticDebugAdmissionV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable
        )
    ));

    let available = finalized_with_optional_semantic_debug(
        slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes,
        OptionalSemanticDebugFixture::Available,
    );
    let admitted = match available.admit_production_semantic_debug_map_v1().unwrap() {
        ProductionFinalizedSemanticDebugAdmissionV1::Admitted(admitted) => admitted,
        ProductionFinalizedSemanticDebugAdmissionV1::Unavailable(gap) => {
            panic!("sourceful exact fixture became unavailable: {gap:?}")
        }
    };
    assert_eq!(
        admitted.admission_status(),
        FinalizedSemanticDebugMapAdmissionStatusV1::ExactInputsAndArtifact
    );
    assert!(admitted.validates_all_input_axes());
    assert!(
        admitted
            .artifact_identity()
            .matches(available.exact_finalized_bytes())
    );
    assert_eq!(
        available.llvm_to_hsaco_derivation_evidence().hsaco(),
        available.raw_output_identity()
    );
    let outer = available.outer_handoff();
    let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
        outer
            .capsule()
            .receipts()
            .semantic_to_llvm()
            .canonical_preimage(),
    )
    .unwrap();
    let ProductionSemanticDebugAvailabilityV1::Available(fragment) =
        extension.carrier_v1().availability()
    else {
        panic!("available fixture lost its debug fragment")
    };
    assert!(fragment.producer_capabilities().contains(
        &ProductionSemanticDebugProducerCapabilityV1::ExactCanonicalKirV7DebugProjection
    ));
    let (_, exact_v8) = VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(
        outer
            .capsule()
            .receipts()
            .kernel_ir()
            .canonical_preimage()
            .to_vec(),
    )
    .unwrap();
    let exact_v7 =
        VerifiedCanonicalKernelIrV7::from_canonical_bytes(fragment.canonical_kir_v7().to_vec())
            .unwrap();
    assert_eq!(
        fe2o3_kernel_ir::decode_module_v7(exact_v7.canonical_bytes()).unwrap(),
        exact_v8
    );
}

#[test]
fn production_semantic_debug_rejects_canonically_resealed_correspondence_substitution() {
    for fixture in [
        OptionalSemanticDebugFixture::AvailableDeleteEliminated,
        OptionalSemanticDebugFixture::AvailableRetypeEliminated,
        OptionalSemanticDebugFixture::AvailableRepointEliminated,
    ] {
        let finalized = finalized_with_optional_semantic_debug(
            slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes,
            fixture,
        );
        assert!(matches!(
            finalized.admit_production_semantic_debug_map_v1(),
            Err(fe2o3_hsaco_finalize::FinalizedSemanticDebugMapErrorV1::InvalidSemanticCorrespondence)
        ));
    }
}

#[test]
fn production_semantic_debug_rejects_changed_v7_projection_and_replay_target() {
    let changed_projection = finalized_with_optional_semantic_debug(
        slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes,
        OptionalSemanticDebugFixture::AvailableDifferentKirV7Module,
    );
    assert!(matches!(
        changed_projection.admit_production_semantic_debug_map_v1(),
        Err(fe2o3_hsaco_finalize::FinalizedSemanticDebugMapErrorV1::CanonicalKirProjectionMismatch)
    ));

    let changed_replay_target = finalized_with_optional_semantic_debug(
        slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes,
        OptionalSemanticDebugFixture::AvailableGfx950Replay,
    );
    assert!(matches!(
        changed_replay_target.admit_production_semantic_debug_map_v1(),
        Err(fe2o3_hsaco_finalize::FinalizedSemanticDebugMapErrorV1::KirToLlvmReplayTargetMismatch)
    ));
}

fn finalizer_semantic_map(artifact: &[u8], isa_end: u64) -> Vec<u8> {
    let content = |bytes: &[u8]| SemanticDebugContentIdentityV1::calculate(bytes).unwrap();
    let binding = SemanticDebugMapBindingV1::new(
        content(b"source-map-v2"),
        content(b"semantic-mir"),
        content(b"canonical-kir"),
        content(b"schedule"),
        content(b"llvm-module"),
        content(artifact),
    )
    .unwrap();
    let llvm = SemanticDebugNodeV1::new(
        [0x91; 32],
        SemanticDebugLocationV1::Llvm {
            function_ordinal: 0,
            block_ordinal: 0,
            instruction_ordinal: 0,
        },
    )
    .unwrap();
    let isa = SemanticDebugNodeV1::new(
        [0x92; 32],
        SemanticDebugLocationV1::Isa {
            kernel_ordinal: 0,
            byte_start: 0,
            byte_end: isa_end,
        },
    )
    .unwrap();
    let mapping = SemanticDebugMappingV1::new(
        [0x93; 32],
        SemanticDebugLayerV1::Llvm,
        SemanticDebugLayerV1::Isa,
        SemanticDebugTransformationV1::Preserved,
        vec![[0x91; 32]],
        SemanticDebugMappingOutputV1::available(vec![[0x92; 32]]),
    )
    .unwrap();
    SemanticDebugMapDocumentV1::new_partial(
        binding,
        vec![llvm, isa],
        vec![mapping],
        vec![
            SemanticDebugBoundaryV1::new(
                [0x91; 32],
                SemanticDebugBoundaryDirectionV1::PredecessorUnavailable,
                SemanticDebugBoundaryReasonV1::ProducerBoundary,
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .to_canonical_json_bytes()
    .unwrap()
}

#[test]
fn native_v3_publication_persists_and_reconstructs_exact_lineage_after_restart() {
    let directory = TestDirectory::new();
    let config = EvidenceConfig::BASE;
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let exact_raw = fixture.bytes.clone();
    let (attempt, source) = evidence_in_directory_for_kernel_and_providers(
        &directory,
        fixture.bytes,
        config,
        "vecadd",
        "vecadd.kd",
        Vec::new(),
    );
    let inspected = inspect_protected_worker_v3_hsaco_v1(source).unwrap();
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).unwrap();
    let exact_finalized = finalized.exact_finalized_bytes().to_vec();
    let prepared =
        prepare_protected_worker_v3_hsaco_publication_v1(&producer(), finalized).unwrap();
    assert_eq!(prepared.attempt(), attempt);
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());

    let persisted = persist_prepared_protected_worker_v3_hsaco_publication_v1(
        &directory.0,
        &producer(),
        prepared,
    )
    .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    let compiler_closure = persisted
        .finalized_evidence()
        .binding_expectation()
        .compiler_closure();
    let binding = persisted.publication_binding(compiler_closure).unwrap();
    assert_eq!(
        binding.publication_intent_record_identity(),
        persisted.storage_record().identity().as_bytes()
    );
    assert_eq!(
        binding.finalization_identity(),
        *persisted
            .publication_intent()
            .finalization_identity()
            .as_bytes()
    );
    assert_eq!(
        binding.finalized_output_length(),
        exact_finalized.len() as u64
    );
    assert!(!binding.grants_publication_authority());
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
    let mismatched_closure =
        CompilerClosureV2::new([21; 32], [22; 32], [23; 32], [24; 32], [25; 32], [26; 32]).unwrap();
    assert!(matches!(
        persisted.publication_binding(mismatched_closure),
        Err(WorkerV3HsacoPublicationErrorV1::CompilerClosureMismatch)
    ));
    let published = publish_recovered_protected_worker_v3_hsaco_v1(
        &directory.0,
        &producer(),
        compiler_closure,
        persisted,
    )
    .unwrap();
    let publication = published.publication_result();
    assert_eq!(publication.publication_binding(), binding);
    let encoded_claim = publication.published_claim().encode_canonical().unwrap();
    let decoded_claim = DurablePublishedHsacoClaimV3::decode_canonical(&encoded_claim).unwrap();
    assert_eq!(&decoded_claim, publication.published_claim());
    let lease = reacquire_current_hsaco_publication_lease_v3(&directory.0, &decoded_claim).unwrap();
    assert_eq!(lease.exact_artifact_bytes(), exact_finalized);
    drop(lease);
    assert_eq!(
        published.recovered_evidence().exact_finalized_hsaco(),
        exact_finalized
    );
    assert_eq!(
        fe2o3_hsaco_finalize::derive_unfinalized_hsaco_from_finalized_v1(
            published.recovered_evidence().exact_finalized_hsaco()
        )
        .unwrap(),
        exact_raw
    );
    let compiler_subject = published.compiler_execution_subject_v1().unwrap();
    let finalized = published.recovered_evidence().finalized_evidence();
    assert_eq!(compiler_subject.attempt(), finalized.attempt());
    assert_eq!(compiler_subject.slot(), finalized.handoff_slot());
    assert_eq!(
        compiler_subject.transaction_identity(),
        finalized.transaction_identity()
    );
    assert_eq!(
        compiler_subject.outer_handoff().sha256(),
        finalized.outer_handoff().identity().sha256()
    );
    let expected_intent = published.recovered_evidence().publication_intent();
    drop(published);
    finish_build_attempt(&directory.0, &producer(), attempt).unwrap();

    let recovered =
        recover_protected_worker_v3_hsaco_publication_v1(&directory.0, &producer(), attempt)
            .unwrap();
    assert_eq!(
        recovered.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Recovered
    );
    assert_eq!(recovered.exact_finalized_hsaco(), exact_finalized);
    assert_eq!(recovered.publication_intent(), expected_intent);
    assert_eq!(
        recovered.compiler_execution_subject_v1().unwrap(),
        compiler_subject
    );
    assert!(!recovered.grants_publication_authority());
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let reconstructed = publish_recovered_protected_worker_v3_hsaco_v1(
        &directory.0,
        &producer(),
        compiler_closure,
        recovered,
    )
    .unwrap();
    assert_eq!(
        reconstructed.publication_result().outcome(),
        AttemptScopedHsacoPublicationOutcomeV3::RecoveredCommittedPublication
    );
    assert_eq!(
        reconstructed.recovered_evidence().exact_finalized_hsaco(),
        exact_finalized
    );
    assert_eq!(
        reconstructed.compiler_execution_subject_v1().unwrap(),
        compiler_subject
    );
    let binding = reconstructed.publication_result().publication_binding();
    let (replay, record, claim, lease) = reconstructed
        .into_load_envelope_parts_v1()
        .expect("completed V3 publication must transfer into load-envelope custody")
        .into_parts();
    assert_eq!(
        record.identity().as_bytes(),
        binding.publication_intent_record_identity()
    );
    assert_eq!(record.plan(), claim.plan());
    assert_eq!(claim.worker_v3_binding(), binding);
    assert_eq!(replay.finalized_hsaco, exact_finalized);
    assert_eq!(lease.exact_artifact_bytes(), exact_finalized);
    assert!(replay.external_provider_payloads.is_empty());
    let providers =
        WorkerV3ExternalProviderPayloadsV1::new(replay.external_provider_payloads.clone()).unwrap();
    assert_eq!(
        providers.canonical_sha256(),
        record.external_provider_archive_sha256()
    );
    assert_eq!(
        providers.canonical_length(),
        record.external_provider_archive_length()
    );
    assert_eq!(
        providers.payload_length(),
        record.external_provider_payload_length()
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&replay.outer_handoff)),
        record.outer_handoff_sha256()
    );
    assert_eq!(replay.outer_handoff.len(), record.outer_handoff_length());
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&replay.transcript)),
        record.transcript_sha256()
    );
    assert_eq!(replay.transcript.len(), record.transcript_length());
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&replay.finalized_hsaco)),
        record.output_sha256()
    );
    assert_eq!(replay.finalized_hsaco.len(), record.output_length());
    let current = lease.acquire_current_token().unwrap();
    lease.validate_current_token(&current).unwrap();
    assert_eq!(current.exact_artifact_bytes(), exact_finalized);
    current.revalidate_locked_currentness().unwrap();
    drop(current);
    let outer = InertSemanticCompilerModuleHandoffV3::decode(&replay.outer_handoff).unwrap();
    assert_eq!(
        *outer.capsule().compiler_closure(),
        binding.compiler_closure()
    );
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&replay.transcript).unwrap();
    assert_eq!(
        transcript.expected_finalization_identity(),
        &binding.finalization_identity()
    );
    assert_eq!(
        transcript.source_evidence_identity(),
        &binding.source_evidence_identity()
    );
}

#[test]
fn strict_v3_gfx942_no_ffi_rejects_external_provider_input() {
    let directory = TestDirectory::new();
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let provider = WorkerInputV1::new(
        WorkerInputKindV1::AmdGpuRelocatable,
        b"unadmitted-external-provider".to_vec(),
    )
    .unwrap();
    let (_, source) = evidence_in_directory_for_kernel_and_providers(
        &directory,
        fixture.bytes,
        EvidenceConfig::BASE,
        "vecadd",
        "vecadd.kd",
        vec![provider],
    );
    assert_eq!(
        inspect_protected_worker_v3_hsaco_v1(source).unwrap_err(),
        WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch
    );
}

#[test]
fn native_v3_publication_rejects_a_different_producer() {
    let directory = TestDirectory::new();
    let fixture = slice_fixture_with_descriptor_table(&slice_descriptor_table());
    let (_, source) = evidence_in_directory_for_kernel(
        &directory,
        fixture.bytes,
        EvidenceConfig::BASE,
        "vecadd",
        "vecadd.kd",
    );
    let inspected = inspect_protected_worker_v3_hsaco_v1(source).unwrap();
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).unwrap();
    let prepared =
        prepare_protected_worker_v3_hsaco_publication_v1(&producer(), finalized).unwrap();
    let other = ProducerIdentity::from_codegen(
        "worker_v3_hsaco_admission_other",
        Some(Path::new("tests/worker_v3_hsaco_admission_other.rs")),
    )
    .unwrap();

    assert!(matches!(
        persist_prepared_protected_worker_v3_hsaco_publication_v1(&directory.0, &other, prepared,),
        Err(fe2o3_hsaco_finalize::WorkerV3HsacoPublicationErrorV1::ProducerIdentityMismatch)
    ));
}

#[test]
fn invocation_closure_transaction_plan_and_worker_axes_cannot_be_dropped() {
    let fixture = || scalar_add_fixture_with(ScalarAddFixtureMutation::RequiredWorkgroup).bytes;
    let base = inspected(fixture(), EvidenceConfig::BASE);
    let changed_attempt = inspected(
        fixture(),
        EvidenceConfig {
            attempt_seed: 0x62,
            ..EvidenceConfig::BASE
        },
    );
    let changed_invocation = inspected(
        fixture(),
        EvidenceConfig {
            invocation_seed: 0x40,
            ..EvidenceConfig::BASE
        },
    );
    let changed_module = inspected(
        fixture(),
        EvidenceConfig {
            module_seed: 0x12,
            ..EvidenceConfig::BASE
        },
    );
    let changed_plan = inspected(
        fixture(),
        EvidenceConfig {
            optimization: "3",
            ..EvidenceConfig::BASE
        },
    );
    let changed_worker = inspected(
        fixture(),
        EvidenceConfig {
            llvm_build_identity: "upstream-llvm-test-build-b",
            ..EvidenceConfig::BASE
        },
    );

    for changed in [
        &changed_attempt,
        &changed_invocation,
        &changed_module,
        &changed_plan,
        &changed_worker,
    ] {
        assert_eq!(base.exact_bytes(), changed.exact_bytes());
        assert_eq!(base.raw_hsaco_identity(), changed.raw_hsaco_identity());
        assert_ne!(base.identity(), changed.identity());
    }
    assert_ne!(base.attempt(), changed_attempt.attempt());
    assert_ne!(
        base.transaction_identity(),
        changed_attempt.transaction_identity()
    );
    assert_ne!(
        base.binding_expectation().invocation_digest(),
        changed_invocation.binding_expectation().invocation_digest()
    );
    assert_ne!(
        base.compiler_closure(),
        changed_invocation.compiler_closure()
    );
    assert_ne!(
        base.outer_handoff_identity(),
        changed_module.outer_handoff_identity()
    );
    assert_ne!(
        base.link_plan_identity(),
        changed_module.link_plan_identity()
    );
    assert_ne!(base.link_plan_identity(), changed_plan.link_plan_identity());
    assert_ne!(
        base.worker_measurement().llvm_build_identity(),
        changed_worker.worker_measurement().llvm_build_identity()
    );
}

#[test]
fn raw_bytes_and_every_structural_hsaco_axis_are_checked() {
    let valid = inspected(
        scalar_add_fixture_with(ScalarAddFixtureMutation::RequiredWorkgroup).bytes,
        EvidenceConfig::BASE,
    );
    let mut changed_fixture = scalar_add_fixture_with(ScalarAddFixtureMutation::RequiredWorkgroup);
    changed_fixture.bytes[changed_fixture.text_offset] ^= 1;
    let changed_bytes = inspected(
        changed_fixture.bytes,
        EvidenceConfig {
            attempt_seed: 0x63,
            ..EvidenceConfig::BASE
        },
    );
    assert_ne!(valid.exact_bytes(), changed_bytes.exact_bytes());
    assert_ne!(
        valid.raw_hsaco_identity(),
        changed_bytes.raw_hsaco_identity()
    );
    assert_ne!(valid.identity(), changed_bytes.identity());

    for (attempt_seed, mutation) in [
        (0x70, ScalarAddFixtureMutation::Target),
        (0x71, ScalarAddFixtureMutation::CodeObjectVersion),
        (0x72, ScalarAddFixtureMutation::EntrySymbol),
        (0x74, ScalarAddFixtureMutation::None),
        (0x77, ScalarAddFixtureMutation::DescriptorComputePgmRsrc1),
        (0x78, ScalarAddFixtureMutation::TruncatedHeader),
    ] {
        let evidence = evidence(
            scalar_add_fixture_with(mutation).bytes,
            EvidenceConfig {
                attempt_seed,
                ..EvidenceConfig::BASE
            },
        );
        assert!(inspect_protected_worker_v3_hsaco_v1(evidence).is_err());
    }
}

fn inspected(bytes: Vec<u8>, config: EvidenceConfig) -> InspectedProtectedWorkerV3HsacoV1 {
    inspect_protected_worker_v3_hsaco_v1(evidence(bytes, config)).unwrap()
}

fn require_v3_inspection(_: &InspectedProtectedWorkerV3HsacoV1) {}

fn evidence(hsaco: Vec<u8>, config: EvidenceConfig) -> InertProtectedFirstBuildWorkerV3EvidenceV1 {
    let directory = TestDirectory::new();
    evidence_in_directory(&directory, hsaco, config).1
}

fn evidence_in_directory(
    directory: &TestDirectory,
    hsaco: Vec<u8>,
    config: EvidenceConfig,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    InertProtectedFirstBuildWorkerV3EvidenceV1,
) {
    evidence_in_directory_for_kernel(directory, hsaco, config, "scalar_add", "scalar_add.kd")
}

fn evidence_in_directory_for_kernel(
    directory: &TestDirectory,
    hsaco: Vec<u8>,
    config: EvidenceConfig,
    entry_symbol: &str,
    descriptor_symbol: &str,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    InertProtectedFirstBuildWorkerV3EvidenceV1,
) {
    evidence_in_directory_for_kernel_and_providers(
        directory,
        hsaco,
        config,
        entry_symbol,
        descriptor_symbol,
        Vec::new(),
    )
}

fn evidence_in_directory_for_kernel_and_providers(
    directory: &TestDirectory,
    hsaco: Vec<u8>,
    config: EvidenceConfig,
    entry_symbol: &str,
    descriptor_symbol: &str,
    external_providers: Vec<WorkerInputV1>,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    InertProtectedFirstBuildWorkerV3EvidenceV1,
) {
    evidence_in_directory_for_kernels_and_providers(
        directory,
        hsaco,
        config,
        &[(entry_symbol, descriptor_symbol)],
        external_providers,
    )
}

fn evidence_in_directory_for_kernels_and_providers(
    directory: &TestDirectory,
    hsaco: Vec<u8>,
    config: EvidenceConfig,
    kernel_symbols: &[(&str, &str)],
    external_providers: Vec<WorkerInputV1>,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    InertProtectedFirstBuildWorkerV3EvidenceV1,
) {
    let attempt = begin_build_attempt(
        &directory.0,
        &producer(),
        BuildInvocation::from_bytes([config.attempt_seed; 32]),
        BuildSession::from_bytes([config.attempt_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let handoff = outer_for_kernels(
        config.invocation_seed,
        config.module_seed,
        &hsaco,
        kernel_symbols,
        config.lineage_mutation,
    );
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        &handoff,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        handoff.identity(),
    )
    .unwrap();
    let worker = pinned(directory, config.llvm_build_identity);
    let evidence = execute(config, receipt, consumed, &worker, external_providers);
    (attempt, evidence)
}

fn execute(
    config: EvidenceConfig,
    receipt: CompilerModuleHandoffReceiptV3,
    consumed: ConsumedCompilerModuleHandoffV3,
    worker: &PinnedWorkerV1,
    external_providers: Vec<WorkerInputV1>,
) -> InertProtectedFirstBuildWorkerV3EvidenceV1 {
    let closure = *consumed.handoff().capsule().compiler_closure();
    execute_protected_reproducible_first_build_worker_v3(
        consumed,
        receipt,
        closure,
        worker,
        external_providers,
        options(config.optimization),
        WorkerOutputConstraintsV1::new(1024 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(3), 2 * 1024 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap()
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(TARGET).unwrap()
}

fn slice_descriptor_table() -> Vec<u8> {
    slice_descriptor_table_with_workgroup(64)
}

fn slice_descriptor_table_with_workgroup(workgroup_size: u32) -> Vec<u8> {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let kernel = slice_kernel_descriptor(
        0xa1,
        "vecadd",
        "vecadd",
        "vecadd.kd",
        &source,
        &layout,
        workgroup_size,
    );
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        DescriptorCodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3").unwrap(),
            Text::new("test").unwrap(),
            [0xa6; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3-worker-v3").unwrap(),
            Text::new("test").unwrap(),
        ),
        target(),
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn synthetic_two_kernel_slice_descriptor_table() -> Vec<u8> {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let kernels = vec![
        slice_kernel_descriptor(
            0xc1,
            "synthetic_first_transform",
            "synthetic_first_transform",
            "synthetic_first_transform.kd",
            &source,
            &layout,
            64,
        ),
        slice_kernel_descriptor(
            0xb1,
            "synthetic_second_transform",
            "synthetic_second_transform",
            "synthetic_second_transform.kd",
            &source,
            &layout,
            64,
        ),
    ];
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        DescriptorCodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3").unwrap(),
            Text::new("test").unwrap(),
            [0xa6; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3-worker-v3").unwrap(),
            Text::new("test").unwrap(),
        ),
        target(),
        vec![source],
        vec![layout],
        kernels,
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn slice_kernel_descriptor(
    identity_seed: u8,
    logical_name: &str,
    entry_name: &str,
    descriptor_symbol: &str,
    source: &SourceTypeRecordV1,
    layout: &DeviceLayoutRecordV1,
    workgroup_size: u32,
) -> KernelDescriptorV1 {
    KernelDescriptorV1::new(
        KernelId::from_bytes([identity_seed; 32]),
        ValidName::new(logical_name).unwrap(),
        ValidName::new(entry_name).unwrap(),
        ValidName::new(descriptor_symbol).unwrap(),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([identity_seed.wrapping_add(1); 32]),
            EvidenceDigest::from_sha256_bytes([identity_seed.wrapping_add(2); 32]),
        ),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([identity_seed.wrapping_add(3); 32]),
            EvidenceDigest::from_sha256_bytes([identity_seed.wrapping_add(4); 32]),
        ),
        Vec::new(),
        KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(workgroup_size, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            workgroup_size,
            0,
            64 * 1024,
        )
        .unwrap(),
        vec![
            LogicalArgumentV1::shared_slice(
                0,
                ValidName::new("values").unwrap(),
                source,
                layout,
                0,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn worker_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-v3-hsaco-fixture"))
}

fn pinned(directory: &TestDirectory, llvm_build_identity: &str) -> PinnedWorkerV1 {
    let private_worker = directory.0.join("worker");
    fs::copy(worker_path(), &private_worker).unwrap();
    let bytes = fs::read(&private_worker).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&bytes),
        WORKER_BUILD_ID,
        llvm_build_identity,
    )
    .unwrap();
    PinnedWorkerV1::open(private_worker, measurement).unwrap()
}

fn pinned_external(
    directory: &TestDirectory,
    worker_path: &Path,
    worker_build_identity: &str,
    llvm_build_identity: &str,
) -> PinnedWorkerV1 {
    let private_worker = directory.0.join("real-worker");
    fs::copy(worker_path, &private_worker).unwrap();
    let bytes = fs::read(&private_worker).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&bytes),
        worker_build_identity,
        llvm_build_identity,
    )
    .unwrap();
    PinnedWorkerV1::open(private_worker, measurement).unwrap()
}

fn scalar_descriptor_source_for_target(
    entry: &str,
    target: DeviceTargetV1,
) -> CompilerDescriptorSourceV1 {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0xd1; 32]),
        ValidName::new(entry).unwrap(),
        ValidName::new(entry).unwrap(),
        ValidName::new(format!("{entry}.kd")).unwrap(),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([0xd2; 32]),
            EvidenceDigest::from_sha256_bytes([0xd3; 32]),
        ),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([0xd4; 32]),
            EvidenceDigest::from_sha256_bytes([0xd5; 32]),
        ),
        Vec::new(),
        KernelAbiLayoutV1::new(4, SCALAR_COV6_KERNARG_SEGMENT_BYTES, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            64,
            0,
            64 * 1024,
        )
        .unwrap(),
        vec![
            LogicalArgumentV1::scalar(0, ValidName::new("value").unwrap(), &source, &layout, 0)
                .unwrap(),
        ],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        DescriptorCodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3").unwrap(),
            Text::new("real-worker-semantic-anchor-test").unwrap(),
            [0xd6; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3-worker-v3").unwrap(),
            Text::new("real-worker-semantic-anchor-test").unwrap(),
        ),
        target,
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap();
    CompilerDescriptorSourceV1::new(table).unwrap()
}

fn append_descriptor_source_assembly(llvm: &mut String, bytes: &[u8]) {
    llvm.push_str("\nmodule asm \".section ");
    llvm.push_str(COMPILER_DESCRIPTOR_SECTION_NAME_V1);
    llvm.push_str(",\\22\\22,@progbits\"\n");
    llvm.push_str("module asm \".balign 8\"\n");
    for chunk in bytes.chunks(16) {
        llvm.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                llvm.push_str(", ");
            }
            llvm.push_str(&format!("0x{byte:02x}"));
        }
        llvm.push_str("\"\n");
    }
}

fn semantic_anchor_handoff(
    profile: ProductionAmdTargetProfileV1,
) -> (CompilerModuleHandoffV2, CompilerDescriptorSourceV1, String) {
    semantic_anchor_handoff_with_version(profile, ProductionReplayKernelIrVersionV1::V8)
}

fn semantic_anchor_handoff_for_family(
    profile: ProductionAmdTargetProfileV1,
    family: ProductionSourceIsaKernelFamilyV1,
) -> (CompilerModuleHandoffV2, CompilerDescriptorSourceV1, String) {
    semantic_anchor_handoff_with_version_and_family(
        profile,
        ProductionReplayKernelIrVersionV1::V8,
        family,
    )
}

fn semantic_anchor_handoff_with_version(
    profile: ProductionAmdTargetProfileV1,
    version: ProductionReplayKernelIrVersionV1,
) -> (CompilerModuleHandoffV2, CompilerDescriptorSourceV1, String) {
    semantic_anchor_handoff_with_version_and_family(
        profile,
        version,
        ProductionSourceIsaKernelFamilyV1::Elementwise,
    )
}

fn semantic_anchor_handoff_with_version_and_family(
    profile: ProductionAmdTargetProfileV1,
    version: ProductionReplayKernelIrVersionV1,
    family: ProductionSourceIsaKernelFamilyV1,
) -> (CompilerModuleHandoffV2, CompilerDescriptorSourceV1, String) {
    let proof_inputs = canonical_compiler_proof_inputs_v4_with_sourceful_family(0x20, family);
    let neutral_bytes = replay_kernel_ir_bytes(proof_inputs.kernel_ir(), version);
    let neutral_module = match version {
        ProductionReplayKernelIrVersionV1::V8 => {
            VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(neutral_bytes)
                .unwrap()
                .1
        }
        ProductionReplayKernelIrVersionV1::V9 => {
            VerifiedCanonicalKernelIrV9::from_canonical_bytes_with_module(neutral_bytes)
                .unwrap()
                .1
        }
    };
    let target_bound = bind_production_target_v1(&neutral_module, profile).unwrap();
    let anchor_identity = match version {
        ProductionReplayKernelIrVersionV1::V8 => {
            let owner =
                VerifiedCanonicalKernelIrV8::from_module(target_bound.module().clone()).unwrap();
            dialect_amdgcn::ProductionSemanticAnchorKirIdentityV1::from_v8(&owner)
        }
        ProductionReplayKernelIrVersionV1::V9 => {
            let owner =
                VerifiedCanonicalKernelIrV9::from_module(target_bound.module().clone()).unwrap();
            dialect_amdgcn::ProductionSemanticAnchorKirIdentityV1::from_v9(&owner)
        }
    };
    let dialect = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => {
            lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                target_bound.module(),
                anchor_identity,
            )
        }
        ProductionAmdTargetProfileV1::Gfx950 => {
            lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                target_bound.module(),
                anchor_identity,
            )
        }
    }
    .unwrap();
    let mut llvm = bind_production_llvm22_worker_layout_v1(&dialect).unwrap();
    let [kernel_id] = target_bound.kernel_ids() else {
        panic!("semantic-anchor fixture must contain one kernel");
    };
    let entry = kernel_id.as_str().to_owned();
    let target = DeviceTargetV1::parse(profile.device_target()).unwrap();
    let descriptor_source = scalar_descriptor_source_for_target(&entry, target);
    append_descriptor_source_assembly(&mut llvm, descriptor_source.canonical_bytes());
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    let descriptor = format!("{entry}.kd");
    let manifest = CompilerModuleSymbolManifestV1::new(vec![
        (CompilerModuleSymbolRoleV1::KernelEntry, entry.as_str()),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            descriptor.as_str(),
        ),
    ])
    .unwrap();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        llvm.as_bytes(),
    )
    .unwrap();
    (handoff, descriptor_source, entry)
}

fn replay_kernel_ir_bytes(
    canonical_v8: &[u8],
    version: ProductionReplayKernelIrVersionV1,
) -> Vec<u8> {
    let (_, module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(canonical_v8.to_vec())
            .unwrap();
    match version {
        ProductionReplayKernelIrVersionV1::V8 => canonical_v8.to_vec(),
        ProductionReplayKernelIrVersionV1::V9 => VerifiedCanonicalKernelIrV9::from_module(module)
            .unwrap()
            .canonical_bytes()
            .to_vec(),
    }
}

fn semantic_anchor_outer(
    handoff: &CompilerModuleHandoffV2,
    descriptor_source: &CompilerDescriptorSourceV1,
    profile: ProductionAmdTargetProfileV1,
) -> InertSemanticCompilerModuleHandoffV3 {
    let base = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            None,
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap();
    let association = association_from_outer(&base);
    let proof = canonical_compiler_proof_inputs_v4_with_sourceful_induction(0x20);
    let carrier = exact_source_mir_kir_carrier_v1(
        association.canonical_bytes(),
        proof.semantic_mir(),
        proof.kernel_ir(),
        proof.correspondence(),
        handoff.module_bytes(),
    );
    let extension =
        ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
            .unwrap();
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            Some(extension.canonical_bytes()),
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

fn semantic_anchor_outer_for_family(
    handoff: &CompilerModuleHandoffV2,
    descriptor_source: &CompilerDescriptorSourceV1,
    profile: ProductionAmdTargetProfileV1,
    family: ProductionSourceIsaKernelFamilyV1,
) -> InertSemanticCompilerModuleHandoffV3 {
    let base = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm_and_version_for_family(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            None,
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
            ProductionReplayKernelIrVersionV1::V8,
            Some(family),
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap();
    let association = association_from_outer(&base);
    let proof = canonical_compiler_proof_inputs_v4_with_sourceful_family(0x20, family);
    let carrier = exact_source_mir_kir_carrier_v1(
        association.canonical_bytes(),
        proof.semantic_mir(),
        proof.kernel_ir(),
        proof.correspondence(),
        handoff.module_bytes(),
    );
    let extension =
        ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
            .unwrap();
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm_and_version_for_family(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            Some(extension.canonical_bytes()),
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
            ProductionReplayKernelIrVersionV1::V8,
            Some(family),
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

fn semantic_anchor_outer_with_association_seed(
    handoff: &CompilerModuleHandoffV2,
    descriptor_source: &CompilerDescriptorSourceV1,
    profile: ProductionAmdTargetProfileV1,
    association_seed: u8,
) -> InertSemanticCompilerModuleHandoffV3 {
    let base = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm(
            association_seed,
            handoff,
            DescriptorLineageMutation::Exact,
            None,
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap();
    let association = association_from_outer(&base);
    let carrier = ProductionSemanticDebugCarrierV1::new(
        association.canonical_bytes(),
        ProductionSemanticDebugAvailabilityV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        ),
    )
    .unwrap();
    let extension =
        ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
            .unwrap();
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            Some(extension.canonical_bytes()),
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

#[derive(Clone, Copy)]
enum V9SourceCarrierFixture {
    ExactProjectionGap,
    OtherProducerGap,
    AvailableV8Substitution,
    StaleAssociation,
}

fn semantic_anchor_outer_v9(
    handoff: &CompilerModuleHandoffV2,
    descriptor_source: &CompilerDescriptorSourceV1,
    profile: ProductionAmdTargetProfileV1,
    fixture: V9SourceCarrierFixture,
) -> InertSemanticCompilerModuleHandoffV3 {
    let base = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm_and_version(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            None,
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
            ProductionReplayKernelIrVersionV1::V9,
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap();
    let association = match fixture {
        V9SourceCarrierFixture::StaleAssociation => {
            let other = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
                &capsule_bytes_with_semantic_to_llvm_and_version(
                    0x40,
                    handoff,
                    DescriptorLineageMutation::Exact,
                    None,
                    Some(profile),
                    Some(descriptor_source.canonical_bytes()),
                    ProductionReplayKernelIrVersionV1::V9,
                ),
                handoff.canonical_bytes(),
            ))
            .unwrap();
            association_from_outer(&other)
        }
        V9SourceCarrierFixture::ExactProjectionGap
        | V9SourceCarrierFixture::OtherProducerGap
        | V9SourceCarrierFixture::AvailableV8Substitution => association_from_outer(&base),
    };
    let availability = match fixture {
        V9SourceCarrierFixture::ExactProjectionGap | V9SourceCarrierFixture::StaleAssociation => {
            ProductionSemanticDebugAvailabilityV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable,
            )
        }
        V9SourceCarrierFixture::OtherProducerGap => {
            ProductionSemanticDebugAvailabilityV1::Unavailable(
                ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
            )
        }
        V9SourceCarrierFixture::AvailableV8Substitution => {
            let proof = canonical_compiler_proof_inputs_v4_with_sourceful_induction(0x20);
            exact_source_mir_kir_carrier_v1(
                association.canonical_bytes(),
                proof.semantic_mir(),
                proof.kernel_ir(),
                proof.correspondence(),
                handoff.module_bytes(),
            )
            .availability()
            .clone()
        }
    };
    let carrier =
        ProductionSemanticDebugCarrierV1::new(association.canonical_bytes(), availability).unwrap();
    let extension =
        ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
            .unwrap();
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm_and_version(
            0x20,
            handoff,
            DescriptorLineageMutation::Exact,
            Some(extension.canonical_bytes()),
            Some(profile),
            Some(descriptor_source.canonical_bytes()),
            ProductionReplayKernelIrVersionV1::V9,
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

fn execute_semantic_anchor_handoff(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
    descriptor_source: &CompilerDescriptorSourceV1,
    profile: ProductionAmdTargetProfileV1,
    worker: &PinnedWorkerV1,
    llvm_build_identity: &'static str,
) -> (
    fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1,
    Vec<u8>,
) {
    let outer = semantic_anchor_outer(handoff, descriptor_source, profile);
    execute_semantic_anchor_outer(directory, &outer, worker, llvm_build_identity)
}

fn execute_semantic_anchor_handoff_for_family(
    directory: &TestDirectory,
    handoff: &CompilerModuleHandoffV2,
    descriptor_source: &CompilerDescriptorSourceV1,
    profile: ProductionAmdTargetProfileV1,
    family: ProductionSourceIsaKernelFamilyV1,
    worker: &PinnedWorkerV1,
    llvm_build_identity: &'static str,
) -> (
    fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1,
    Vec<u8>,
) {
    let outer = semantic_anchor_outer_for_family(handoff, descriptor_source, profile, family);
    execute_semantic_anchor_outer(directory, &outer, worker, llvm_build_identity)
}

fn execute_semantic_anchor_outer(
    directory: &TestDirectory,
    outer: &InertSemanticCompilerModuleHandoffV3,
    worker: &PinnedWorkerV1,
    llvm_build_identity: &'static str,
) -> (
    fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1,
    Vec<u8>,
) {
    let config = EvidenceConfig {
        optimization: "3",
        llvm_build_identity,
        ..EvidenceConfig::BASE
    };
    let attempt = begin_build_attempt(
        &directory.0,
        &producer(),
        BuildInvocation::from_bytes([config.attempt_seed; 32]),
        BuildSession::from_bytes([config.attempt_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        outer,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        outer.identity(),
    )
    .unwrap();
    let evidence = execute(config, receipt, consumed, worker, Vec::new());
    let raw_worker_output = evidence.output_bytes().to_vec();
    let inspected = inspect_protected_worker_v3_hsaco_v1(evidence).unwrap();
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).unwrap();
    (finalized, raw_worker_output)
}

fn mutate_section_first_byte(bytes: &mut [u8], section_name: &str) {
    let (offset, size) = {
        let object = object::File::parse(&*bytes).unwrap();
        let mut sections = object
            .sections()
            .filter(|section| section.name() == Ok(section_name));
        let section = sections
            .next()
            .expect("actual Worker output retains section");
        assert!(sections.next().is_none());
        section.file_range().expect("section is file-backed")
    };
    assert!(size > 8);
    bytes[usize::try_from(offset).unwrap()] ^= 1;
}

fn handoff_with_raw_worker_output(
    base: &CompilerModuleHandoffV2,
    raw_worker_output: &[u8],
) -> CompilerModuleHandoffV2 {
    let mut module = base.module_bytes().to_vec();
    module.extend_from_slice(RAW_HSACO_MARKER);
    module.extend_from_slice(hex_encode(raw_worker_output).as_bytes());
    module.push(b'\n');
    handoff_with_module_bytes(base, &module)
}

fn handoff_with_module_bytes(
    base: &CompilerModuleHandoffV2,
    module: &[u8],
) -> CompilerModuleHandoffV2 {
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        base.target(),
        base.code_object_version(),
        base.envelope().clone(),
        base.symbol_manifest().clone(),
        module,
    )
    .unwrap()
}

fn handoff_with_diagnostic_probe_descriptor_name(
    base: &CompilerModuleHandoffV2,
) -> CompilerModuleHandoffV2 {
    let llvm = std::str::from_utf8(base.module_bytes()).unwrap();
    let named = llvm
        .lines()
        .find(|line| line.starts_with("!llvm.pseudo_probe_desc = !{!"))
        .unwrap();
    let reference = named
        .strip_prefix("!llvm.pseudo_probe_desc = !{!")
        .and_then(|line| line.strip_suffix('}'))
        .unwrap();
    let definition_prefix = format!("!{reference} = !{{i64 ");
    let mut changed = false;
    let mut output = String::new();
    for line in llvm.lines() {
        if line.starts_with(&definition_prefix) {
            let name_start = line.rfind("!\"").unwrap();
            output.push_str(&line[..name_start]);
            output.push_str("!\"diagnostic-only-name\"}");
            changed = true;
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    assert!(changed);
    handoff_with_module_bytes(base, output.as_bytes())
}

#[test]
fn semantic_anchor_admission_rejects_resealed_cross_spliced_association_by_default() {
    let finalized = finalized_with_optional_semantic_debug(
        slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes,
        OptionalSemanticDebugFixture::UnavailableCrossSpliced,
    );
    assert_eq!(
        finalized
            .admit_production_semantic_anchors_v1()
            .unwrap_err(),
        ProductionSemanticAnchorErrorV1::InvalidProductionAssociation
    );
}

#[test]
fn source_isa_correlation_preserves_exact_source_carrier_unavailability() {
    let finalized = finalized_with_optional_semantic_debug(
        slice_fixture_with_descriptor_table(&slice_descriptor_table()).bytes,
        OptionalSemanticDebugFixture::Unavailable(
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        ),
    );
    assert!(matches!(
        finalized
            .admit_production_source_isa_correlation_v1()
            .unwrap(),
        ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
            ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(
                ProductionSemanticDebugProducerGapV1::SourceMapUnavailable
            )
        )
    ));
    assert!(matches!(
        finalized
            .admit_production_source_isa_acceptance_summary_v1()
            .unwrap(),
        ProductionSourceIsaAcceptanceSummaryAdmissionV1::Unavailable(
            ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(
                ProductionSemanticDebugProducerGapV1::SourceMapUnavailable
            )
        )
    ));
}

fn production_bridge_inputs(
    finalized: &fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
        finalized
            .outer_handoff()
            .capsule()
            .receipts()
            .semantic_to_llvm()
            .canonical_preimage(),
    )
    .unwrap();
    let ProductionSemanticDebugAvailabilityV1::Available(fragment) =
        extension.carrier_v1().availability()
    else {
        panic!("production bridge fixture requires an available exact source carrier")
    };
    (
        fragment.canonical_kir_v7().to_vec(),
        finalized
            .outer_handoff()
            .capsule()
            .receipts()
            .kernel_ir()
            .canonical_preimage()
            .to_vec(),
        fragment.source_map_v2().to_vec(),
    )
}

fn reseal_bridge_claim(bytes: &mut [u8]) {
    const DOMAIN: &[u8] = b"FE2O3/PRODUCTION-KIR-V7-STRUCTURAL-BRIDGE/V1\0";
    let identity_offset = bytes.len() - 32;
    let mut digest = Sha256::new();
    digest.update(u32::try_from(DOMAIN.len()).unwrap().to_le_bytes());
    digest.update(DOMAIN);
    digest.update(&bytes[..identity_offset]);
    bytes[identity_offset..].copy_from_slice(&digest.finalize());
}

fn assert_production_source_isa_family_shape(family: ProductionSourceIsaKernelFamilyV1) {
    let proof = canonical_compiler_proof_inputs_v4_with_sourceful_family(0x20, family);
    let (_, module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(proof.kernel_ir().to_vec())
            .unwrap();
    let operations = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| body.blocks.iter())
        .flat_map(|block| block.operations.iter())
        .map(|operation| &operation.kind)
        .collect::<Vec<_>>();
    assert!(
        operations.iter().any(|operation| {
            matches!(
                operation,
                OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    ..
                }
            )
        }),
        "{family:?} lost the production u32 induction operation"
    );
    match family {
        ProductionSourceIsaKernelFamilyV1::Elementwise => {}
        ProductionSourceIsaKernelFamilyV1::WorkgroupCollective => assert!(
            operations
                .iter()
                .any(|operation| matches!(operation, OperationKind::WorkgroupBarrier(_))),
            "workgroup-collective fixture lost its uniform barrier"
        ),
        ProductionSourceIsaKernelFamilyV1::Tiled => {
            for expected in [BinaryOp::Divide, BinaryOp::Remainder] {
                assert!(
                    operations.iter().any(|operation| {
                        matches!(operation, OperationKind::Binary { op, .. } if *op == expected)
                    }),
                    "tiled fixture lost {expected:?} coordinate arithmetic"
                );
            }
        }
    }
}

#[test]
fn production_source_isa_kernel_family_fixtures_are_distinct_canonical_kir_v8() {
    let mut identities = Vec::new();
    for family in [
        ProductionSourceIsaKernelFamilyV1::Elementwise,
        ProductionSourceIsaKernelFamilyV1::WorkgroupCollective,
        ProductionSourceIsaKernelFamilyV1::Tiled,
    ] {
        assert_production_source_isa_family_shape(family);
        let proof = canonical_compiler_proof_inputs_v4_with_sourceful_family(0x20, family);
        let owner =
            VerifiedCanonicalKernelIrV8::from_canonical_bytes(proof.kernel_ir().to_vec()).unwrap();
        assert!(
            !identities.contains(owner.identity().digest()),
            "{family:?} reused another family's canonical KIR"
        );
        identities.push(*owner.identity().digest());
    }
}

#[test]
#[allow(clippy::too_many_lines)]
#[ignore = "requires FE2O3_TEST_REAL_WORKER, FE2O3_TEST_REAL_WORKER_BUILD_ID, and FE2O3_TEST_REAL_LLVM_BUILD_ID"]
fn production_source_isa_catalog_admits_real_worker_kernel_family_matrix() {
    let worker_path = PathBuf::from(std::env::var("FE2O3_TEST_REAL_WORKER").unwrap());
    let worker_build_identity = std::env::var("FE2O3_TEST_REAL_WORKER_BUILD_ID").unwrap();
    let llvm_build_identity: &'static str = Box::leak(
        std::env::var("FE2O3_TEST_REAL_LLVM_BUILD_ID")
            .unwrap()
            .into_boxed_str(),
    );
    let families = [
        ProductionSourceIsaKernelFamilyV1::Elementwise,
        ProductionSourceIsaKernelFamilyV1::WorkgroupCollective,
        ProductionSourceIsaKernelFamilyV1::Tiled,
    ];
    let profiles = [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ];
    let mut prior_catalogs = Vec::new();
    let mut saw_map_family_substitution = false;
    let mut saw_target_substitution = false;
    let mut saw_duplicated = false;
    let mut saw_coalesced = false;
    let mut saw_eliminated = false;

    for profile in profiles {
        for family in families {
            assert_production_source_isa_family_shape(family);
            let (handoff, descriptor_source, _) =
                semantic_anchor_handoff_for_family(profile, family);
            let directory = TestDirectory::new();
            let real_worker = pinned_external(
                &directory,
                &worker_path,
                &worker_build_identity,
                llvm_build_identity,
            );
            let (finalized, _) = execute_semantic_anchor_handoff_for_family(
                &directory,
                &handoff,
                &descriptor_source,
                profile,
                family,
                &real_worker,
                llvm_build_identity,
            );
            let correlation = match finalized
                .admit_production_source_isa_correlation_v1()
                .unwrap()
            {
                ProductionSourceIsaCorrelationAdmissionV1::Admitted(correlation) => correlation,
                ProductionSourceIsaCorrelationAdmissionV1::Unavailable(reason) => {
                    panic!("{profile:?} {family:?} correlation unavailable: {reason:?}")
                }
            };
            let catalog = match finalized.admit_production_source_isa_catalog_v1().unwrap() {
                ProductionSourceIsaCatalogAdmissionV1::Admitted(catalog) => catalog,
                ProductionSourceIsaCatalogAdmissionV1::Unavailable(reason) => {
                    panic!("{profile:?} {family:?} catalog unavailable: {reason:?}")
                }
            };
            assert_eq!(correlation.structural_binding().profile(), profile);
            assert_eq!(
                catalog.structural_binding().target(),
                match profile {
                    ProductionAmdTargetProfileV1::Gfx942 => {
                        ProductionSourceIsaCatalogTargetV1::Gfx942
                    }
                    ProductionAmdTargetProfileV1::Gfx950 => {
                        ProductionSourceIsaCatalogTargetV1::Gfx950
                    }
                }
            );
            assert_eq!(catalog.artifact_identity(), correlation.artifact_identity());
            assert_eq!(catalog.records().len(), correlation.records().len());

            let mut exact_round_trips = 0_usize;
            for record in catalog.records() {
                match record.transformation() {
                    Some(ProductionSourceIsaCatalogTransformationV1::Duplicated) => {
                        saw_duplicated = true;
                    }
                    Some(ProductionSourceIsaCatalogTransformationV1::Coalesced) => {
                        saw_coalesced = true;
                    }
                    Some(ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced) => {
                        saw_duplicated = true;
                        saw_coalesced = true;
                    }
                    Some(ProductionSourceIsaCatalogTransformationV1::Eliminated) => {
                        saw_eliminated = true;
                    }
                    Some(ProductionSourceIsaCatalogTransformationV1::Preserved) | None => {}
                }
                if record.kind() == ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir {
                    saw_eliminated = true;
                }
                if record.kind() != ProductionSourceIsaCatalogRecordKindV1::SourceAnchored
                    || record.isa().is_empty()
                {
                    continue;
                }

                let source_node = record.source_node_identity().unwrap();
                let source_span = record.source_span().unwrap();
                let mir_node = record.mir_node_identity().unwrap();
                let mir = record.mir().unwrap();
                let neutral_kir_node = record.neutral_kir_node_identity().unwrap();
                let neutral_kir = record.neutral_kir().unwrap();
                let target_kir = record.target_kir().unwrap();
                let semantic_operation = record.semantic_operation_id().unwrap();
                let llvm = record.compiler_handoff_llvm().unwrap();
                assert!(
                    catalog
                        .query_source_node(source_node)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(
                    catalog
                        .query_source_span(source_span)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(
                    catalog
                        .query_mir_node(mir_node)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(catalog.query_mir(mir).unwrap().any(|v| v == record));
                assert!(
                    catalog
                        .query_neutral_kir_node(neutral_kir_node)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(
                    catalog
                        .query_neutral_kir(neutral_kir)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(
                    catalog
                        .query_target_kir(target_kir)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(
                    catalog
                        .query_semantic_operation(semantic_operation)
                        .unwrap()
                        .any(|v| v == record)
                );
                assert!(
                    catalog
                        .query_compiler_handoff_llvm(llvm)
                        .unwrap()
                        .any(|v| v == record)
                );
                for interval in record.isa() {
                    assert!(
                        catalog
                            .query_isa_pc(ProductionSourceIsaCatalogPointV1::new(
                                interval.kernel_ordinal(),
                                interval.byte_start(),
                            ))
                            .unwrap()
                            .any(|v| v == record)
                    );
                }
                exact_round_trips += 1;
            }
            assert!(
                exact_round_trips > 0,
                "{profile:?} {family:?} has no exact Source-to-ISA-to-Source witness"
            );
            assert!(!catalog.proves_complete_machine_instruction_coverage());
            assert!(!catalog.proves_a_schedule());
            assert!(!catalog.proves_semantic_refinement());
            assert!(!catalog.proves_optimized_or_final_llvm_custody());
            assert!(!catalog.proves_live_program_counter_ownership());
            assert!(!catalog.grants_debugger_authority());
            assert!(!catalog.grants_profiler_authority());
            assert!(!catalog.grants_publication_authority());
            assert!(!catalog.grants_runtime_authority());

            let catalog_bytes = catalog.to_canonical_bytes().unwrap();
            let re_admitted =
                InertProductionSourceIsaCatalogV1::from_canonical_bytes(&catalog_bytes)
                    .unwrap()
                    .admit_exact_projection_v1(&correlation)
                    .unwrap();
            assert_eq!(re_admitted.records(), catalog.records());
            for (prior_profile, prior_family, prior_bytes) in &prior_catalogs {
                let wrong = InertProductionSourceIsaCatalogV1::from_canonical_bytes(prior_bytes)
                    .unwrap()
                    .admit_exact_projection_v1(&correlation);
                assert!(matches!(
                    wrong,
                    Err(ProductionSourceIsaCatalogErrorV1::ExactProjectionMismatch)
                ));
                saw_map_family_substitution |= *prior_profile == profile && *prior_family != family;
                saw_target_substitution |= *prior_profile != profile && *prior_family == family;
            }
            prior_catalogs.push((profile, family, catalog_bytes));
        }
    }
    assert!(saw_map_family_substitution);
    assert!(saw_target_substitution);
    assert!(saw_duplicated);
    assert!(saw_coalesced);
    assert!(saw_eliminated);
}

#[test]
#[ignore = "requires FE2O3_TEST_REAL_WORKER, FE2O3_TEST_REAL_WORKER_BUILD_ID, and FE2O3_TEST_REAL_LLVM_BUILD_ID"]
fn production_semantic_anchors_admit_real_worker_gfx942_and_gfx950() {
    let worker_path = PathBuf::from(std::env::var("FE2O3_TEST_REAL_WORKER").unwrap());
    let worker_build_identity = std::env::var("FE2O3_TEST_REAL_WORKER_BUILD_ID").unwrap();
    let llvm_build_identity: &'static str = Box::leak(
        std::env::var("FE2O3_TEST_REAL_LLVM_BUILD_ID")
            .unwrap()
            .into_boxed_str(),
    );
    let mut prior_catalog = None;
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let (handoff, descriptor_source, _) = semantic_anchor_handoff(profile);
        let directory = TestDirectory::new();
        let real_worker = pinned_external(
            &directory,
            &worker_path,
            &worker_build_identity,
            llvm_build_identity,
        );
        let (finalized, raw_worker_output) = execute_semantic_anchor_handoff(
            &directory,
            &handoff,
            &descriptor_source,
            profile,
            &real_worker,
            llvm_build_identity,
        );
        let admitted = match finalized.admit_production_semantic_anchors_v1().unwrap() {
            ProductionSemanticAnchorAdmissionV1::Admitted(admitted) => admitted,
            ProductionSemanticAnchorAdmissionV1::Unavailable(reason) => {
                panic!("real Worker anchors unexpectedly unavailable: {reason:?}")
            }
        };
        assert_eq!(admitted.target(), profile.device_target());
        assert!(!admitted.anchors().is_empty());
        assert!(
            admitted
                .anchors()
                .iter()
                .any(|anchor| !anchor.isa().is_empty())
        );
        assert!(!admitted.proves_general_executable_bytes_unchanged());
        assert!(!admitted.proves_general_resource_metadata_unchanged());
        assert!(!admitted.proves_zero_runtime_or_code_size_overhead());

        let correlation = match finalized
            .admit_production_source_isa_correlation_v1()
            .unwrap()
        {
            ProductionSourceIsaCorrelationAdmissionV1::Admitted(correlation) => correlation,
            ProductionSourceIsaCorrelationAdmissionV1::Unavailable(reason) => {
                panic!("real Worker source/ISA correlation unexpectedly unavailable: {reason:?}")
            }
        };
        let sourceful = correlation
            .records()
            .iter()
            .find(|record| {
                record.kind() == ProductionSourceIsaRecordKindV1::SourceAnchored
                    && !record.isa().is_empty()
            })
            .expect("real Worker has one sourceful sparse ISA anchor");
        let source_matches = correlation
            .query_source_node(sourceful.source_node_identity().unwrap())
            .unwrap()
            .collect::<Vec<_>>();
        assert!(
            source_matches.iter().any(|record| {
                record.semantic_operation_id() == sourceful.semantic_operation_id()
            })
        );
        let SemanticDebugLocationV1::Isa {
            kernel_ordinal,
            byte_start,
            ..
        } = sourceful.isa()[0]
        else {
            unreachable!()
        };
        let reverse = correlation
            .query_isa_pc(ProductionIsaPointV1::new(kernel_ordinal, byte_start))
            .unwrap()
            .collect::<Vec<_>>();
        assert!(reverse.iter().any(|record| {
            record.semantic_operation_id() == sourceful.semantic_operation_id()
                && record.source_node_identity() == sourceful.source_node_identity()
        }));
        assert!(correlation.records().iter().any(|record| {
            record.kind() == ProductionSourceIsaRecordKindV1::EliminatedBeforeKir
        }));
        assert!(!correlation.proves_complete_machine_instruction_coverage());
        assert!(!correlation.proves_a_schedule());
        assert!(!correlation.proves_semantic_refinement());
        assert!(!correlation.proves_optimized_or_final_llvm_custody());
        assert!(!correlation.proves_live_program_counter_ownership());
        assert!(!correlation.grants_runtime_authority());

        let catalog = match finalized.admit_production_source_isa_catalog_v1().unwrap() {
            ProductionSourceIsaCatalogAdmissionV1::Admitted(catalog) => catalog,
            ProductionSourceIsaCatalogAdmissionV1::Unavailable(reason) => {
                panic!("real Worker source/ISA catalog unexpectedly unavailable: {reason:?}")
            }
        };
        assert_eq!(catalog.correlation_identity(), correlation.identity());
        assert_eq!(
            catalog.semantic_map_identity(),
            correlation.semantic_map_identity()
        );
        assert_eq!(
            catalog.source_map_v2_identity().sha256(),
            correlation.source_map_v2_identity().sha256()
        );
        assert_eq!(
            catalog.source_map_v2_identity().byte_len(),
            correlation.source_map_v2_identity().byte_len()
        );
        assert_eq!(catalog.artifact_identity(), correlation.artifact_identity());
        assert_eq!(catalog.records().len(), correlation.records().len());
        let catalog_sourceful = catalog
            .records()
            .iter()
            .find(|record| {
                record.kind() == ProductionSourceIsaCatalogRecordKindV1::SourceAnchored
                    && !record.isa().is_empty()
            })
            .unwrap();
        assert!(
            catalog
                .query_semantic_operation(catalog_sourceful.semantic_operation_id().unwrap())
                .unwrap()
                .any(|record| record.source_node_identity()
                    == catalog_sourceful.source_node_identity())
        );
        let catalog_interval = catalog_sourceful.isa()[0];
        assert!(
            catalog
                .query_isa_pc(ProductionSourceIsaCatalogPointV1::new(
                    catalog_interval.kernel_ordinal(),
                    catalog_interval.byte_start(),
                ))
                .unwrap()
                .any(|record| record.semantic_operation_id()
                    == catalog_sourceful.semantic_operation_id())
        );
        let catalog_bytes = catalog.to_canonical_bytes().unwrap();
        let decoded_catalog =
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&catalog_bytes)
                .unwrap()
                .admit_exact_projection_v1(&correlation)
                .unwrap();
        assert_eq!(decoded_catalog.identity(), catalog.identity());
        assert_eq!(decoded_catalog.records(), catalog.records());
        assert!(!decoded_catalog.grants_debugger_authority());
        assert!(!decoded_catalog.grants_profiler_authority());

        let (canonical_v7, canonical_v8, source_map_v2) = production_bridge_inputs(&finalized);
        let bridge = match admit_production_kir_v7_structural_bridge_v1(
            &canonical_v7,
            &canonical_v8,
            &source_map_v2,
            finalized.exact_finalized_bytes(),
            &catalog,
        )
        .unwrap()
        {
            ProductionKirV7BridgeAdmissionV1::Admitted(bridge) => bridge,
            ProductionKirV7BridgeAdmissionV1::Unavailable(reason) => {
                panic!("exact real-Worker V7/V8 bridge unexpectedly unavailable: {reason:?}")
            }
        };
        assert_eq!(bridge.catalog_identity(), catalog.identity());
        assert_eq!(
            bridge.correlation_identity(),
            catalog.correlation_identity()
        );
        assert_eq!(
            bridge.semantic_map_identity(),
            catalog.semantic_map_identity()
        );
        assert_eq!(
            bridge.structural_identity(),
            &catalog.structural_binding().identity()
        );
        assert_eq!(
            bridge.artifact_identity().sha256(),
            *catalog.artifact_identity().sha256()
        );
        assert_eq!(
            bridge.artifact_identity().byte_len(),
            catalog.artifact_identity().byte_len()
        );
        for record in bridge.records() {
            assert_eq!(
                bridge.query_simulator_v7(record.simulator_v7()).unwrap(),
                *record
            );
            assert_eq!(
                bridge
                    .query_neutral_production(record.neutral_production())
                    .unwrap(),
                *record
            );
            assert_eq!(
                bridge
                    .query_target_production(record.target_production())
                    .unwrap(),
                *record
            );
        }
        let no_source = catalog
            .records()
            .iter()
            .find(|record| {
                record.kind() == ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance
                    && record.target_kir().is_some()
            })
            .expect("real Worker catalog retains one exact no-source target operation");
        let no_source_coordinate = no_source.target_kir().unwrap();
        let no_source_site = ProductionKirV7BridgeSiteV1::operation(
            no_source_coordinate.function_ordinal(),
            no_source_coordinate.block_ordinal(),
            no_source_coordinate.operation_ordinal(),
        );
        assert!(
            bridge
                .query_target_catalog(&catalog, no_source_site)
                .unwrap()
                .any(|record| {
                    record.kind() == ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance
                        && record.target_kir() == Some(no_source_coordinate)
                })
        );
        if let Some(stale_catalog) = prior_catalog.as_ref() {
            assert_eq!(
                bridge
                    .query_target_catalog(stale_catalog, no_source_site)
                    .unwrap_err(),
                ProductionKirV7BridgeCatalogQueryUnavailableV1::CatalogIdentityMismatch
            );
        }
        let block_entry = ProductionKirV7BridgeSiteV1::block_entry(
            no_source_coordinate.function_ordinal(),
            no_source_coordinate.block_ordinal(),
        );
        assert_eq!(
            bridge.query_target_catalog(&catalog, block_entry).unwrap_err(),
            ProductionKirV7BridgeCatalogQueryUnavailableV1::BlockEntryHasNoCatalogOperationCoordinate
        );
        let terminator = ProductionKirV7BridgeSiteV1::terminator(
            no_source_coordinate.function_ordinal(),
            no_source_coordinate.block_ordinal(),
        );
        assert_eq!(
            bridge.query_target_catalog(&catalog, terminator).unwrap_err(),
            ProductionKirV7BridgeCatalogQueryUnavailableV1::TerminatorHasNoCatalogOperationCoordinate
        );
        assert!(!bridge.proves_source_attribution_for_every_site());
        assert!(!bridge.proves_semantic_refinement());
        assert!(!bridge.grants_runtime_authority());

        let bridge_bytes = bridge.to_canonical_bytes().unwrap();
        let replayed = InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bridge_bytes)
            .unwrap()
            .admit_exact_projection_v1(
                &canonical_v7,
                &canonical_v8,
                &source_map_v2,
                finalized.exact_finalized_bytes(),
                &catalog,
            )
            .unwrap();
        assert_eq!(replayed.identity(), bridge.identity());
        assert_eq!(replayed.records(), bridge.records());

        let (_, mut stale_module) =
            VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(canonical_v8.clone())
                .unwrap();
        stale_module.id = "stale-production-bridge-module".into();
        let stale_v7 = VerifiedCanonicalKernelIrV7::from_module(stale_module.clone())
            .unwrap()
            .into_canonical_bytes();
        let stale_v8 = VerifiedCanonicalKernelIrV8::from_module(stale_module)
            .unwrap()
            .into_canonical_bytes();
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bridge_bytes)
                .unwrap()
                .admit_exact_projection_v1(
                    &stale_v7,
                    &canonical_v8,
                    &source_map_v2,
                    finalized.exact_finalized_bytes(),
                    &catalog,
                )
                .unwrap_err(),
            ProductionKirV7BridgeErrorV1::SourceMapV7IdentityMismatch
        );
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bridge_bytes)
                .unwrap()
                .admit_exact_projection_v1(
                    &canonical_v7,
                    &stale_v8,
                    &source_map_v2,
                    finalized.exact_finalized_bytes(),
                    &catalog,
                )
                .unwrap_err(),
            ProductionKirV7BridgeErrorV1::ProductionKirCatalogIdentityMismatch
        );

        let exact_source_map =
            DebugSourceMapDocumentV2::from_canonical_json_bytes(&source_map_v2).unwrap();
        let stale_source_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                [0x91; 32],
                exact_source_map.binding().canonical_kir().digest(),
                exact_source_map.binding().canonical_kir().canonical_bytes(),
            )
            .unwrap(),
            exact_source_map.files().to_vec(),
            exact_source_map.sites().to_vec(),
            exact_source_map.eliminated().to_vec(),
            exact_source_map.scopes().to_vec(),
            exact_source_map.variables().to_vec(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bridge_bytes)
                .unwrap()
                .admit_exact_projection_v1(
                    &canonical_v7,
                    &canonical_v8,
                    &stale_source_map,
                    finalized.exact_finalized_bytes(),
                    &catalog,
                )
                .unwrap_err(),
            ProductionKirV7BridgeErrorV1::SourceMapCatalogIdentityMismatch
        );
        let mut stale_artifact = finalized.exact_finalized_bytes().to_vec();
        let stale_artifact_last = stale_artifact.len() - 1;
        stale_artifact[stale_artifact_last] ^= 1;
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&bridge_bytes)
                .unwrap()
                .admit_exact_projection_v1(
                    &canonical_v7,
                    &canonical_v8,
                    &source_map_v2,
                    &stale_artifact,
                    &catalog,
                )
                .unwrap_err(),
            ProductionKirV7BridgeErrorV1::ArtifactCatalogIdentityMismatch
        );

        let mut substituted_target = bridge_bytes.clone();
        substituted_target[73] = match substituted_target[73] {
            1 => 2,
            2 => 1,
            _ => unreachable!("admitted bridge has one closed target tag"),
        };
        reseal_bridge_claim(&mut substituted_target);
        assert_eq!(
            InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&substituted_target)
                .unwrap()
                .admit_exact_projection_v1(
                    &canonical_v7,
                    &canonical_v8,
                    &source_map_v2,
                    finalized.exact_finalized_bytes(),
                    &catalog,
                )
                .unwrap_err(),
            ProductionKirV7BridgeErrorV1::ExactProjectionMismatch
        );
        for identity_offset in [32_usize, 80, 120, 160, 192, 232, 272, 304, 336] {
            let mut substituted_claim = bridge_bytes.clone();
            substituted_claim[identity_offset] ^= 1;
            reseal_bridge_claim(&mut substituted_claim);
            let inert =
                InertProductionKirV7StructuralBridgeV1::from_canonical_bytes(&substituted_claim)
                    .unwrap();
            assert_eq!(
                inert
                    .admit_exact_projection_v1(
                        &canonical_v7,
                        &canonical_v8,
                        &source_map_v2,
                        finalized.exact_finalized_bytes(),
                        &catalog,
                    )
                    .unwrap_err(),
                ProductionKirV7BridgeErrorV1::ExactProjectionMismatch
            );
        }
        prior_catalog = Some(catalog);

        let summary = match finalized
            .admit_production_source_isa_acceptance_summary_v1()
            .unwrap()
        {
            ProductionSourceIsaAcceptanceSummaryAdmissionV1::Admitted(summary) => summary,
            ProductionSourceIsaAcceptanceSummaryAdmissionV1::Unavailable(reason) => {
                panic!("real Worker source/ISA summary unexpectedly unavailable: {reason:?}")
            }
        };
        assert_eq!(summary.artifact_identity(), correlation.artifact_identity());
        assert_eq!(summary.correlation_identity(), correlation.identity());
        assert_eq!(
            summary.structural_binding(),
            correlation.structural_binding()
        );
        assert_eq!(summary.structural_binding().profile(), profile);
        assert_eq!(
            summary.structural_binding().version(),
            ProductionReplayKernelIrVersionV1::V8
        );
        let mut expected_source_nodes = BTreeMap::new();
        let mut expected_source_spans = BTreeMap::new();
        let mut expected_isa_points = BTreeMap::new();
        let mut expected_source_anchored = 0_u64;
        let mut expected_eliminated = 0_u64;
        let mut expected_no_source = 0_u64;
        let mut expected_source_without_isa = 0_u64;
        let mut expected_isa_references = 0_u64;
        for record in correlation.records() {
            match record.kind() {
                ProductionSourceIsaRecordKindV1::SourceAnchored => {
                    expected_source_anchored += 1;
                    expected_source_without_isa += u64::from(record.isa().is_empty());
                }
                ProductionSourceIsaRecordKindV1::EliminatedBeforeKir => expected_eliminated += 1,
                ProductionSourceIsaRecordKindV1::NoSourceProvenance => expected_no_source += 1,
            }
            if let Some(identity) = record.source_node_identity() {
                *expected_source_nodes.entry(identity).or_insert(0_u64) += 1;
            }
            if let Some(span) = record.source_span() {
                *expected_source_spans.entry(span).or_insert(0_u64) += 1;
            }
            for location in record.isa() {
                let SemanticDebugLocationV1::Isa {
                    kernel_ordinal,
                    byte_start,
                    ..
                } = *location
                else {
                    unreachable!()
                };
                *expected_isa_points
                    .entry(ProductionIsaPointV1::new(kernel_ordinal, byte_start))
                    .or_insert(0_u64) += 1;
                expected_isa_references += 1;
            }
        }
        fn maximum<K: Ord>(counts: &BTreeMap<K, u64>) -> u64 {
            counts.values().copied().max().unwrap_or(0)
        }
        let counts = summary.counts();
        assert_eq!(
            counts.records(),
            u64::try_from(correlation.records().len()).unwrap()
        );
        assert_eq!(counts.source_anchored_records(), expected_source_anchored);
        assert_eq!(counts.eliminated_before_kir_records(), expected_eliminated);
        assert_eq!(counts.no_source_provenance_records(), expected_no_source);
        assert_eq!(
            counts.source_anchored_without_isa_records(),
            expected_source_without_isa
        );
        assert_eq!(counts.isa_references(), expected_isa_references);
        assert_eq!(
            counts.distinct_source_node_queries(),
            u64::try_from(expected_source_nodes.len()).unwrap()
        );
        assert_eq!(
            counts.distinct_source_span_queries(),
            u64::try_from(expected_source_spans.len()).unwrap()
        );
        assert_eq!(
            counts.distinct_isa_point_queries(),
            u64::try_from(expected_isa_points.len()).unwrap()
        );
        assert_eq!(
            counts.maximum_source_node_query_matches(),
            maximum(&expected_source_nodes)
        );
        assert_eq!(
            counts.maximum_source_span_query_matches(),
            maximum(&expected_source_spans)
        );
        assert_eq!(
            counts.maximum_isa_point_query_matches(),
            maximum(&expected_isa_points)
        );
        assert!(counts.source_anchored_records() > 0);
        assert!(counts.isa_references() > 0);
        let witness = summary
            .round_trip_witness()
            .expect("real Worker source/ISA summary has one exact round-trip witness");
        assert_eq!(witness.isa_point().kernel_ordinal(), 0);
        assert!(witness.isa_point().symbol_relative_pc().is_multiple_of(4));
        assert!(witness.source_node_query_matches() > 0);
        assert!(witness.source_span_query_matches() > 0);
        assert!(witness.isa_point_query_matches() > 0);
        assert!(!summary.proves_complete_machine_instruction_coverage());
        assert!(!summary.proves_a_schedule());
        assert!(!summary.proves_semantic_refinement());
        assert!(!summary.proves_optimized_or_final_llvm_custody());
        assert!(!summary.proves_live_program_counter_ownership());
        assert!(!summary.retains_correlation_records());
        assert!(!summary.grants_publication_authority());
        assert!(!summary.grants_runtime_authority());

        let (v9_handoff, v9_descriptor_source, _) =
            semantic_anchor_handoff_with_version(profile, ProductionReplayKernelIrVersionV1::V9);
        let v9_outer = semantic_anchor_outer_v9(
            &v9_handoff,
            &v9_descriptor_source,
            profile,
            V9SourceCarrierFixture::ExactProjectionGap,
        );
        let v9_directory = TestDirectory::new();
        let v9_real_worker = pinned_external(
            &v9_directory,
            &worker_path,
            &worker_build_identity,
            llvm_build_identity,
        );
        let (v9_finalized, v9_raw_worker_output) = execute_semantic_anchor_outer(
            &v9_directory,
            &v9_outer,
            &v9_real_worker,
            llvm_build_identity,
        );
        assert!(matches!(
            v9_finalized
                .admit_production_source_isa_correlation_v1()
                .unwrap(),
            ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9
            )
        ));
        assert!(matches!(
            v9_finalized
                .admit_production_source_isa_acceptance_summary_v1()
                .unwrap(),
            ProductionSourceIsaAcceptanceSummaryAdmissionV1::Unavailable(
                ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9
            )
        ));
        assert!(matches!(
            v9_finalized
                .admit_production_source_isa_catalog_v1()
                .unwrap(),
            ProductionSourceIsaCatalogAdmissionV1::Unavailable(
                ProductionSourceIsaCorrelationUnavailableV1::SourceProjectionForKirV9
            )
        ));

        let v9_actual_handoff = handoff_with_raw_worker_output(&v9_handoff, &v9_raw_worker_output);
        let other_gap = semantic_anchor_outer_v9(
            &v9_actual_handoff,
            &v9_descriptor_source,
            profile,
            V9SourceCarrierFixture::OtherProducerGap,
        );
        let other_gap_directory = TestDirectory::new();
        let fixture_worker = pinned(
            &other_gap_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (other_gap_finalized, _) = execute_semantic_anchor_outer(
            &other_gap_directory,
            &other_gap,
            &fixture_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert!(matches!(
            other_gap_finalized
                .admit_production_source_isa_correlation_v1()
                .unwrap(),
            ProductionSourceIsaCorrelationAdmissionV1::Unavailable(
                ProductionSourceIsaCorrelationUnavailableV1::SemanticDebugCarrier(
                    ProductionSemanticDebugProducerGapV1::SourceMapUnavailable
                )
            )
        ));

        let substituted_carrier = semantic_anchor_outer_v9(
            &v9_actual_handoff,
            &v9_descriptor_source,
            profile,
            V9SourceCarrierFixture::AvailableV8Substitution,
        );
        let substituted_directory = TestDirectory::new();
        let fixture_worker = pinned(
            &substituted_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (substituted_finalized, _) = execute_semantic_anchor_outer(
            &substituted_directory,
            &substituted_carrier,
            &fixture_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert!(matches!(
            substituted_finalized.admit_production_source_isa_correlation_v1(),
            Err(ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(
                FinalizedSemanticDebugMapErrorV1::InvalidBoundCanonicalKirV8
            ))
        ));

        let stale_association = semantic_anchor_outer_v9(
            &v9_actual_handoff,
            &v9_descriptor_source,
            profile,
            V9SourceCarrierFixture::StaleAssociation,
        );
        let stale_directory = TestDirectory::new();
        let fixture_worker = pinned(&stale_directory, EvidenceConfig::BASE.llvm_build_identity);
        let (stale_finalized, _) = execute_semantic_anchor_outer(
            &stale_directory,
            &stale_association,
            &fixture_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert!(matches!(
            stale_finalized.admit_production_source_isa_correlation_v1(),
            Err(ProductionSourceIsaCorrelationErrorV1::SemanticDebugMap(
                FinalizedSemanticDebugMapErrorV1::ProductionAssociationMismatch
            ))
        ));

        let artifact_substitution = handoff_with_raw_worker_output(&v9_handoff, &raw_worker_output);
        let artifact_substitution = semantic_anchor_outer_v9(
            &artifact_substitution,
            &v9_descriptor_source,
            profile,
            V9SourceCarrierFixture::ExactProjectionGap,
        );
        let artifact_directory = TestDirectory::new();
        let fixture_worker = pinned(
            &artifact_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (artifact_finalized, _) = execute_semantic_anchor_outer(
            &artifact_directory,
            &artifact_substitution,
            &fixture_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert!(matches!(
            artifact_finalized.admit_production_source_isa_correlation_v1(),
            Err(ProductionSourceIsaCorrelationErrorV1::SemanticAnchors(_))
        ));

        let diagnostic_handoff = handoff_with_diagnostic_probe_descriptor_name(&handoff);
        let diagnostic_handoff =
            handoff_with_raw_worker_output(&diagnostic_handoff, &raw_worker_output);
        let diagnostic_directory = TestDirectory::new();
        let diagnostic_worker = pinned(
            &diagnostic_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (diagnostic_finalized, _) = execute_semantic_anchor_handoff(
            &diagnostic_directory,
            &diagnostic_handoff,
            &descriptor_source,
            profile,
            &diagnostic_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert!(matches!(
            diagnostic_finalized
                .admit_production_source_isa_correlation_v1()
                .unwrap(),
            ProductionSourceIsaCorrelationAdmissionV1::Admitted(_)
        ));

        let actual_handoff = handoff_with_raw_worker_output(&handoff, &raw_worker_output);
        let cross_spliced = semantic_anchor_outer_with_association_seed(
            &actual_handoff,
            &descriptor_source,
            profile,
            0x40,
        );
        let cross_splice_directory = TestDirectory::new();
        let cross_splice_worker = pinned(
            &cross_splice_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (cross_spliced_finalized, _) = execute_semantic_anchor_outer(
            &cross_splice_directory,
            &cross_spliced,
            &cross_splice_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert_eq!(
            cross_spliced_finalized
                .admit_production_semantic_anchors_v1()
                .unwrap_err(),
            ProductionSemanticAnchorErrorV1::InvalidProductionAssociation
        );

        for (section, expected_error) in [
            (
                ".pseudo_probe_desc",
                ProductionSemanticAnchorErrorV1::ProbeDescriptorMismatch,
            ),
            (
                ".pseudo_probe",
                ProductionSemanticAnchorErrorV1::InvalidProbeEncoding,
            ),
        ] {
            let mut mutated = raw_worker_output.clone();
            mutate_section_first_byte(&mut mutated, section);
            let mutated_handoff = handoff_with_raw_worker_output(&handoff, &mutated);
            let mutation_directory = TestDirectory::new();
            let fixture_worker = pinned(
                &mutation_directory,
                EvidenceConfig::BASE.llvm_build_identity,
            );
            let (mutated_finalized, _) = execute_semantic_anchor_handoff(
                &mutation_directory,
                &mutated_handoff,
                &descriptor_source,
                profile,
                &fixture_worker,
                EvidenceConfig::BASE.llvm_build_identity,
            );
            assert_eq!(
                mutated_finalized
                    .admit_production_semantic_anchors_v1()
                    .unwrap_err(),
                expected_error
            );
        }

        let compiler_handoff = std::str::from_utf8(handoff.module_bytes()).unwrap();
        let hostile = compiler_handoff.replacen(
            "!fe2o3.semantic_anchor.v1",
            "!fe2o3.semantic_anchor.hybrid.v1",
            1,
        );
        assert_ne!(hostile, compiler_handoff);
        let hostile = handoff_with_module_bytes(&handoff, hostile.as_bytes());
        let hostile = handoff_with_raw_worker_output(&hostile, &raw_worker_output);
        let mutation_directory = TestDirectory::new();
        let fixture_worker = pinned(
            &mutation_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (hostile_finalized, _) = execute_semantic_anchor_handoff(
            &mutation_directory,
            &hostile,
            &descriptor_source,
            profile,
            &fixture_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert_eq!(
            hostile_finalized
                .admit_production_semantic_anchors_v1()
                .unwrap_err(),
            ProductionSemanticAnchorErrorV1::ContradictoryLlvm
        );

        let compiler_handoff = std::str::from_utf8(handoff.module_bytes()).unwrap();
        let digest_start = compiler_handoff.find("sha256:").unwrap() + "sha256:".len();
        let mut stale = compiler_handoff.as_bytes().to_vec();
        stale[digest_start] = if stale[digest_start] == b'0' {
            b'1'
        } else {
            b'0'
        };
        let stale = handoff_with_module_bytes(&handoff, &stale);
        let stale = handoff_with_raw_worker_output(&stale, &raw_worker_output);
        let mutation_directory = TestDirectory::new();
        let fixture_worker = pinned(
            &mutation_directory,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        let (stale_finalized, _) = execute_semantic_anchor_handoff(
            &mutation_directory,
            &stale,
            &descriptor_source,
            profile,
            &fixture_worker,
            EvidenceConfig::BASE.llvm_build_identity,
        );
        assert_eq!(
            stale_finalized
                .admit_production_semantic_anchors_v1()
                .unwrap_err(),
            ProductionSemanticAnchorErrorV1::BindingMismatch
        );
    }
}

fn options(optimization: &str) -> Vec<LinkOptionV1> {
    [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", optimization),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect()
}

fn module_handoff_for_kernels(
    seed: u8,
    hsaco: &[u8],
    kernel_symbols: &[(&str, &str)],
) -> CompilerModuleHandoffV2 {
    let mut module = format!("; ModuleID = 'raw-hsaco-v3-{seed:02x}'\n").into_bytes();
    module.extend_from_slice(RAW_HSACO_MARKER);
    module.extend_from_slice(hex_encode(hsaco).as_bytes());
    module.push(b'\n');
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target(), CodeObjectVersion::V6)
            .unwrap();
    let mut symbols = kernel_symbols
        .iter()
        .flat_map(|(entry_symbol, descriptor_symbol)| {
            [
                (CompilerModuleSymbolRoleV1::KernelEntry, *entry_symbol),
                (
                    CompilerModuleSymbolRoleV1::KernelDescriptor,
                    *descriptor_symbol,
                ),
            ]
        })
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    let manifest = CompilerModuleSymbolManifestV1::new(symbols).unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target(),
        CodeObjectVersion::V6,
        envelope,
        manifest,
        &module,
    )
    .unwrap()
}

#[derive(Clone, Copy)]
enum OptionalSemanticDebugFixture {
    LegacyBare,
    Unavailable(ProductionSemanticDebugProducerGapV1),
    UnavailableCrossSpliced,
    Available,
    AvailableDeleteEliminated,
    AvailableRetypeEliminated,
    AvailableRepointEliminated,
    AvailableDifferentKirV7Module,
    AvailableGfx950Replay,
}

impl OptionalSemanticDebugFixture {
    const fn is_available(self) -> bool {
        matches!(
            self,
            Self::Available
                | Self::AvailableDeleteEliminated
                | Self::AvailableRetypeEliminated
                | Self::AvailableRepointEliminated
                | Self::AvailableDifferentKirV7Module
                | Self::AvailableGfx950Replay
        )
    }

    const fn replay_profile(self) -> Option<ProductionAmdTargetProfileV1> {
        if matches!(self, Self::AvailableGfx950Replay) {
            Some(ProductionAmdTargetProfileV1::Gfx950)
        } else if self.is_available() || matches!(self, Self::Unavailable(_)) {
            Some(ProductionAmdTargetProfileV1::Gfx942)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
enum AvailableMapMutation {
    Delete,
    Retype,
    Repoint,
}

fn finalized_with_optional_semantic_debug(
    raw_hsaco: Vec<u8>,
    fixture: OptionalSemanticDebugFixture,
) -> fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1 {
    let directory = TestDirectory::new();
    let config = EvidenceConfig::BASE;
    let attempt = begin_build_attempt(
        &directory.0,
        &producer(),
        BuildInvocation::from_bytes([config.attempt_seed; 32]),
        BuildSession::from_bytes([config.attempt_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let handoff = outer_for_kernels_with_optional_semantic_debug(
        config.invocation_seed,
        config.module_seed,
        &raw_hsaco,
        &[("vecadd", "vecadd.kd")],
        config.lineage_mutation,
        fixture,
    );
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        &handoff,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        handoff.identity(),
    )
    .unwrap();
    let worker = pinned(&directory, config.llvm_build_identity);
    let evidence = execute(config, receipt, consumed, &worker, Vec::new());
    let inspected = inspect_protected_worker_v3_hsaco_v1(evidence).unwrap();
    finalize_protected_worker_v3_hsaco_v1(inspected).unwrap()
}

fn outer_for_kernels_with_optional_semantic_debug(
    invocation_seed: u8,
    module_seed: u8,
    hsaco: &[u8],
    kernel_symbols: &[(&str, &str)],
    lineage_mutation: DescriptorLineageMutation,
    fixture: OptionalSemanticDebugFixture,
) -> InertSemanticCompilerModuleHandoffV3 {
    let handoff = module_handoff_for_kernels(module_seed, hsaco, kernel_symbols);
    let replay_profile = fixture.replay_profile();
    let base = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm(
            invocation_seed,
            &handoff,
            lineage_mutation,
            None,
            replay_profile,
            None,
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap();
    let association = association_from_outer(&base);
    let receipt = match fixture {
        OptionalSemanticDebugFixture::LegacyBare => association.canonical_bytes().to_vec(),
        OptionalSemanticDebugFixture::Unavailable(gap) => {
            let carrier = ProductionSemanticDebugCarrierV1::new(
                association.canonical_bytes(),
                ProductionSemanticDebugAvailabilityV1::Unavailable(gap),
            )
            .unwrap();
            ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
                .unwrap()
                .canonical_bytes()
                .to_vec()
        }
        OptionalSemanticDebugFixture::UnavailableCrossSpliced => {
            let other = InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
                &capsule_bytes_with_semantic_to_llvm(
                    0x40,
                    &handoff,
                    lineage_mutation,
                    None,
                    replay_profile,
                    None,
                ),
                handoff.canonical_bytes(),
            ))
            .unwrap();
            let other_association = association_from_outer(&other);
            let carrier = ProductionSemanticDebugCarrierV1::new(
                other_association.canonical_bytes(),
                ProductionSemanticDebugAvailabilityV1::Unavailable(
                    ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
                ),
            )
            .unwrap();
            ProductionSemanticDebugReceiptExtensionV1::new(
                other_association.canonical_bytes(),
                carrier,
            )
            .unwrap()
            .canonical_bytes()
            .to_vec()
        }
        OptionalSemanticDebugFixture::Available
        | OptionalSemanticDebugFixture::AvailableDeleteEliminated
        | OptionalSemanticDebugFixture::AvailableRetypeEliminated
        | OptionalSemanticDebugFixture::AvailableRepointEliminated
        | OptionalSemanticDebugFixture::AvailableDifferentKirV7Module
        | OptionalSemanticDebugFixture::AvailableGfx950Replay => {
            let proof =
                canonical_compiler_proof_inputs_v4_with_sourceful_induction(invocation_seed);
            let carrier = if matches!(
                fixture,
                OptionalSemanticDebugFixture::AvailableDifferentKirV7Module
            ) {
                let other = canonical_compiler_proof_inputs_v4_with_sourceful_induction(
                    invocation_seed.wrapping_add(1),
                );
                exact_source_mir_kir_carrier_with_projection_v1(
                    association.canonical_bytes(),
                    proof.semantic_mir(),
                    proof.kernel_ir(),
                    proof.correspondence(),
                    handoff.module_bytes(),
                    other.kernel_ir(),
                )
            } else {
                exact_source_mir_kir_carrier_v1(
                    association.canonical_bytes(),
                    proof.semantic_mir(),
                    proof.kernel_ir(),
                    proof.correspondence(),
                    handoff.module_bytes(),
                )
            };
            let carrier = match fixture {
                OptionalSemanticDebugFixture::AvailableDeleteEliminated => {
                    mutate_available_map(carrier, AvailableMapMutation::Delete)
                }
                OptionalSemanticDebugFixture::AvailableRetypeEliminated => {
                    mutate_available_map(carrier, AvailableMapMutation::Retype)
                }
                OptionalSemanticDebugFixture::AvailableRepointEliminated => {
                    mutate_available_map(carrier, AvailableMapMutation::Repoint)
                }
                _ => carrier,
            };
            ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
                .unwrap()
                .canonical_bytes()
                .to_vec()
        }
    };
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes_with_semantic_to_llvm(
            invocation_seed,
            &handoff,
            lineage_mutation,
            Some(&receipt),
            replay_profile,
            None,
        ),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

fn mutate_available_map(
    carrier: ProductionSemanticDebugCarrierV1,
    mutation: AvailableMapMutation,
) -> ProductionSemanticDebugCarrierV1 {
    let ProductionSemanticDebugAvailabilityV1::Available(fragment) = carrier.availability() else {
        panic!("map mutation requires an available fragment")
    };
    let map =
        SemanticDebugMapDocumentV1::from_canonical_json_bytes(fragment.pre_finalization_map())
            .unwrap();
    let mut mappings = map.mappings().to_vec();
    let mut boundaries = map.boundaries().to_vec();
    let eliminated = mappings
        .iter()
        .position(|mapping| {
            mapping.input_layer() == SemanticDebugLayerV1::Mir
                && mapping.output_layer() == SemanticDebugLayerV1::Kir
                && mapping.transformation() == SemanticDebugTransformationV1::Eliminated
        })
        .expect("sourceful fixture has one eliminated statement");
    let eliminated_input = mappings[eliminated].inputs()[0];
    match mutation {
        AvailableMapMutation::Delete => {
            mappings.remove(eliminated);
            boundaries.push(
                SemanticDebugBoundaryV1::new(
                    eliminated_input,
                    SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                    SemanticDebugBoundaryReasonV1::NotRepresented,
                )
                .unwrap(),
            );
        }
        AvailableMapMutation::Retype => {
            let mapping = &mappings[eliminated];
            mappings[eliminated] = SemanticDebugMappingV1::new(
                mapping.identity(),
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Unavailable,
                vec![eliminated_input],
                SemanticDebugMappingOutputV1::unavailable(
                    SemanticDebugUnavailableReasonV1::OptimizedOut,
                ),
            )
            .unwrap();
        }
        AvailableMapMutation::Repoint => {
            let available = mappings
                .iter()
                .position(|mapping| {
                    mapping.input_layer() == SemanticDebugLayerV1::Mir
                        && mapping.output_layer() == SemanticDebugLayerV1::Kir
                        && mapping.output().reason().is_none()
                })
                .expect("sourceful fixture has a retained statement");
            let available_mapping = mappings[available].clone();
            let available_input = available_mapping.inputs()[0];
            let eliminated_mapping = mappings[eliminated].clone();
            mappings[available] = SemanticDebugMappingV1::new(
                available_mapping.identity(),
                available_mapping.input_layer(),
                available_mapping.output_layer(),
                available_mapping.transformation(),
                vec![eliminated_input],
                available_mapping.output().clone(),
            )
            .unwrap();
            mappings[eliminated] = SemanticDebugMappingV1::new(
                eliminated_mapping.identity(),
                eliminated_mapping.input_layer(),
                eliminated_mapping.output_layer(),
                eliminated_mapping.transformation(),
                vec![available_input],
                eliminated_mapping.output().clone(),
            )
            .unwrap();
        }
    }
    let map = SemanticDebugMapDocumentV1::new_partial(
        map.binding(),
        map.nodes().to_vec(),
        mappings,
        boundaries,
    )
    .unwrap()
    .to_canonical_json_bytes()
    .unwrap();
    let fragment = ProductionSemanticDebugFragmentV1::new(
        fragment.source_map_v2().to_vec(),
        fragment.canonical_kir_v7().to_vec(),
        fragment.schedule_status().to_vec(),
        map,
    )
    .unwrap();
    ProductionSemanticDebugCarrierV1::new(
        carrier.association_v3(),
        ProductionSemanticDebugAvailabilityV1::Available(fragment),
    )
    .unwrap()
}

fn association_from_outer(
    outer: &InertSemanticCompilerModuleHandoffV3,
) -> InertSemanticToLlvmAssociationV3 {
    let receipts = outer.capsule().receipts();
    let module = outer.module_handoff().module_identity();
    let identity = |sha256: &[u8; 32], byte_len| {
        InertSemanticToLlvmContentIdentityV3::new(*sha256, byte_len).unwrap()
    };
    InertSemanticToLlvmAssociationV3::new(InertSemanticToLlvmAssociationInputsV3::new(
        identity(
            receipts.semantic_mir().identity().sha256(),
            receipts.semantic_mir().identity().byte_len(),
        ),
        identity(
            receipts.middle_end().identity().sha256(),
            receipts.middle_end().identity().byte_len(),
        ),
        identity(
            receipts.kernel_ir().identity().sha256(),
            receipts.kernel_ir().identity().byte_len(),
        ),
        identity(
            receipts.mir_to_kir_correspondence().identity().sha256(),
            receipts.mir_to_kir_correspondence().identity().byte_len(),
        ),
        identity(
            receipts.formal_memory().identity().sha256(),
            receipts.formal_memory().identity().byte_len(),
        ),
        identity(
            receipts.proof_binding().identity().sha256(),
            receipts.proof_binding().identity().byte_len(),
        ),
        identity(
            receipts.target_binding().identity().sha256(),
            receipts.target_binding().identity().byte_len(),
        ),
        identity(
            receipts.data_layout().identity().sha256(),
            receipts.data_layout().identity().byte_len(),
        ),
        identity(
            receipts.abi().identity().sha256(),
            receipts.abi().identity().byte_len(),
        ),
        identity(
            receipts.export_manifest().identity().sha256(),
            receipts.export_manifest().identity().byte_len(),
        ),
        identity(
            receipts.amdgpu_lowering().identity().sha256(),
            receipts.amdgpu_lowering().identity().byte_len(),
        ),
        identity(module.sha256(), module.byte_len()),
        identity(
            receipts
                .final_compiler_module_commitment()
                .identity()
                .sha256(),
            receipts
                .final_compiler_module_commitment()
                .identity()
                .byte_len(),
        ),
    ))
    .unwrap()
}

fn outer_for_kernels(
    invocation_seed: u8,
    module_seed: u8,
    hsaco: &[u8],
    kernel_symbols: &[(&str, &str)],
    lineage_mutation: DescriptorLineageMutation,
) -> InertSemanticCompilerModuleHandoffV3 {
    let handoff = module_handoff_for_kernels(module_seed, hsaco, kernel_symbols);
    InertSemanticCompilerModuleHandoffV3::decode(&raw_outer(
        &capsule_bytes(invocation_seed, &handoff, lineage_mutation),
        handoff.canonical_bytes(),
    ))
    .unwrap()
}

fn capsule_bytes(
    seed: u8,
    handoff: &CompilerModuleHandoffV2,
    lineage_mutation: DescriptorLineageMutation,
) -> Vec<u8> {
    capsule_bytes_with_semantic_to_llvm(seed, handoff, lineage_mutation, None, None, None)
}

fn capsule_bytes_with_semantic_to_llvm(
    seed: u8,
    handoff: &CompilerModuleHandoffV2,
    lineage_mutation: DescriptorLineageMutation,
    semantic_to_llvm: Option<&[u8]>,
    replay_profile: Option<ProductionAmdTargetProfileV1>,
    descriptor_source_override: Option<&[u8]>,
) -> Vec<u8> {
    capsule_bytes_with_semantic_to_llvm_and_version(
        seed,
        handoff,
        lineage_mutation,
        semantic_to_llvm,
        replay_profile,
        descriptor_source_override,
        ProductionReplayKernelIrVersionV1::V8,
    )
}

fn capsule_bytes_with_semantic_to_llvm_and_version(
    seed: u8,
    handoff: &CompilerModuleHandoffV2,
    lineage_mutation: DescriptorLineageMutation,
    semantic_to_llvm: Option<&[u8]>,
    replay_profile: Option<ProductionAmdTargetProfileV1>,
    descriptor_source_override: Option<&[u8]>,
    replay_version: ProductionReplayKernelIrVersionV1,
) -> Vec<u8> {
    capsule_bytes_with_semantic_to_llvm_and_version_for_family(
        seed,
        handoff,
        lineage_mutation,
        semantic_to_llvm,
        replay_profile,
        descriptor_source_override,
        replay_version,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn capsule_bytes_with_semantic_to_llvm_and_version_for_family(
    seed: u8,
    handoff: &CompilerModuleHandoffV2,
    lineage_mutation: DescriptorLineageMutation,
    semantic_to_llvm: Option<&[u8]>,
    replay_profile: Option<ProductionAmdTargetProfileV1>,
    descriptor_source_override: Option<&[u8]>,
    replay_version: ProductionReplayKernelIrVersionV1,
    family: Option<ProductionSourceIsaKernelFamilyV1>,
) -> Vec<u8> {
    let invocation = invocation_bytes_for_target(seed, &handoff.target().to_string());
    let final_commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(handoff).unwrap();
    let mut receipts = RECEIPTS
        .iter()
        .map(|(label, domain)| {
            (
                format!("worker-v3/receipt/{label}/{seed:02x}").into_bytes(),
                *domain,
            )
        })
        .collect::<Vec<_>>();
    let proof_inputs = if let Some(family) = family {
        canonical_compiler_proof_inputs_v4_with_sourceful_family(seed, family)
    } else if replay_profile.is_some() {
        canonical_compiler_proof_inputs_v4_with_sourceful_induction(seed)
    } else {
        canonical_compiler_proof_inputs_v4(seed)
    };
    receipts[2].0 = proof_inputs.semantic_mir().to_vec();
    receipts[3].0 = proof_inputs.middle_end().to_vec();
    receipts[4].0 = replay_kernel_ir_bytes(proof_inputs.kernel_ir(), replay_version);
    receipts[5].0 = proof_inputs.correspondence().to_vec();
    receipts[6].0 = proof_inputs.formal_memory().to_vec();
    let profile = replay_profile.unwrap_or(ProductionAmdTargetProfileV1::Gfx942);
    let neutral_module = match replay_version {
        ProductionReplayKernelIrVersionV1::V8 => {
            VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(receipts[4].0.clone())
                .unwrap()
                .1
        }
        ProductionReplayKernelIrVersionV1::V9 => {
            VerifiedCanonicalKernelIrV9::from_canonical_bytes_with_module(receipts[4].0.clone())
                .unwrap()
                .1
        }
    };
    let target_bound = bind_production_target_v1(&neutral_module, profile).unwrap();
    let anchor_identity = match replay_version {
        ProductionReplayKernelIrVersionV1::V8 => {
            let owner =
                VerifiedCanonicalKernelIrV8::from_module(target_bound.module().clone()).unwrap();
            dialect_amdgcn::ProductionSemanticAnchorKirIdentityV1::from_v8(&owner)
        }
        ProductionReplayKernelIrVersionV1::V9 => {
            let owner =
                VerifiedCanonicalKernelIrV9::from_module(target_bound.module().clone()).unwrap();
            dialect_amdgcn::ProductionSemanticAnchorKirIdentityV1::from_v9(&owner)
        }
    };
    let dialect_llvm = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => {
            lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                target_bound.module(),
                anchor_identity,
            )
        }
        ProductionAmdTargetProfileV1::Gfx950 => {
            lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                target_bound.module(),
                anchor_identity,
            )
        }
    }
    .unwrap();
    let pre_descriptor_llvm = bind_production_llvm22_worker_layout_v1(&dialect_llvm).unwrap();
    let lowering = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
        &receipts[4].0,
        target_bound.module(),
        profile,
        &pre_descriptor_llvm,
    )
    .unwrap();
    let neutral_kir = TargetLineageIdentityV3::new(
        lowering.neutral_kernel_ir_identity().sha256(),
        lowering.neutral_kernel_ir_identity().byte_len(),
    )
    .unwrap();
    let bound_kir = TargetLineageIdentityV3::new(
        lowering.target_bound_kernel_ir_identity().sha256(),
        lowering.target_bound_kernel_ir_identity().byte_len(),
    )
    .unwrap();
    receipts[12].0 = lowering.canonical_bytes().to_vec();
    receipts[8].0 = TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
        protected_rustc_invocation: TargetLineageIdentityV3::new(
            identity(INVOCATION_DIGEST_DOMAIN_V3, &invocation),
            invocation.len() as u64,
        )
        .unwrap(),
        semantic_mir: receipt_lineage_identity(&receipts, 2),
        target_neutral_kir: neutral_kir,
        target_bound_kir: bound_kir,
        configured_target: profile.device_target(),
        rustc_llvm_target: profile.rustc_target(),
        target_cpu: profile.cpu(),
        target_features: profile.rustc_features(),
        code_object_version: 6,
        wave_width_bits: 64,
        default_workgroup: [64, 1, 1],
    })
    .unwrap()
    .canonical_bytes()
    .to_vec();
    let semantic_layout = derive_semantic_target_layout_identity_v1(
        profile.rustc_target(),
        PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
        64,
        profile.cpu(),
        profile.rustc_features(),
    )
    .unwrap();
    receipts[9].0 = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
        semantic_mir: receipt_lineage_identity(&receipts, 2),
        target_binding: receipt_lineage_identity(&receipts, 8),
        semantic_layout,
        rustc_llvm_target: profile.rustc_target(),
        live_rustc_data_layout: PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
        final_llvm_target: profile.rustc_target(),
        final_llvm_data_layout: PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
        default_pointer_width_bits: 64,
    })
    .unwrap()
    .canonical_bytes()
    .to_vec();
    let hsaco = handoff
        .module_bytes()
        .windows(RAW_HSACO_MARKER.len())
        .position(|window| window == RAW_HSACO_MARKER)
        .and_then(|offset| {
            let encoded = &handoff.module_bytes()[offset + RAW_HSACO_MARKER.len()..];
            let line = encoded.split(|byte| *byte == b'\n').next()?;
            hex_decode(line)
        });
    let descriptor_source = descriptor_source_override
        .map(ToOwned::to_owned)
        .or_else(|| {
            hsaco
                .as_deref()
                .and_then(|bytes| inspect_unfinalized(bytes).ok())
                .and_then(|inspection| {
                    encode_device_descriptor_table_v1(inspection.descriptor_table()).ok()
                })
        });
    if let Some(descriptor_source) = descriptor_source {
        receipts[10].0 = descriptor_source;
    }
    let verus_execution = canonical_verus_execution_evidence_v1(&receipts[3].0, seed);
    receipts[7].0 = proof_binding_association_payload(&receipts, &verus_execution);
    receipts[11].0 = handoff.symbol_manifest().canonical_bytes().to_vec();
    match lineage_mutation {
        DescriptorLineageMutation::Exact => {}
        DescriptorLineageMutation::DifferentCanonicalSource => {
            let source = &mut receipts[10].0;
            let offset = source
                .windows(4)
                .position(|window| window == b"test")
                .expect("fixture descriptor has a test identity");
            source[offset] = b'b';
            CompilerDescriptorSourceV1::decode(source)
                .expect("hostile source remains canonical and zero-digest");
        }
        DescriptorLineageMutation::DifferentExportManifest => {
            receipts[11].0 = b"different canonical export manifest receipt".to_vec();
        }
    }
    receipts.push((
        final_commitment.canonical_bytes().to_vec(),
        FINAL_RECEIPT_DOMAIN_V3,
    ));
    if let Some(semantic_to_llvm) = semantic_to_llvm {
        receipts[13].0 = semantic_to_llvm.to_vec();
    } else {
        receipts[13].0 =
            SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                semantic_mir: receipt_lineage_identity(&receipts, 2),
                middle_end: receipt_lineage_identity(&receipts, 3),
                kernel_ir: receipt_lineage_identity(&receipts, 4),
                mir_to_kir_correspondence: receipt_lineage_identity(&receipts, 5),
                formal_memory: receipt_lineage_identity(&receipts, 6),
                proof_binding: receipt_lineage_identity(&receipts, 7),
                target_binding: receipt_lineage_identity(&receipts, 8),
                data_layout: receipt_lineage_identity(&receipts, 9),
                abi: receipt_lineage_identity(&receipts, 10),
                export_manifest: receipt_lineage_identity(&receipts, 11),
                amdgpu_lowering: receipt_lineage_identity(&receipts, 12),
                final_llvm: TargetLineageIdentityV3::new(
                    Sha256::digest(handoff.module_bytes()).into(),
                    handoff.module_bytes().len() as u64,
                )
                .unwrap(),
                final_compiler_module_commitment: receipt_lineage_identity(&receipts, 14),
            })
            .unwrap()
            .canonical_bytes()
            .to_vec();
    }
    let capsule_target = handoff.target().to_string();
    let total_len = 24
        + 4
        + invocation.len()
        + 32
        + 2
        + capsule_target.len()
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
    capsule.extend_from_slice(&(capsule_target.len() as u16).to_le_bytes());
    capsule.extend_from_slice(capsule_target.as_bytes());
    for (payload, domain) in receipts {
        push_blob(&mut capsule, &payload);
        capsule.extend_from_slice(&identity(domain, &payload));
    }
    let capsule_identity = identity(CAPSULE_IDENTITY_DOMAIN_V3, &capsule);
    capsule.extend_from_slice(&capsule_identity);
    assert_eq!(capsule.len(), total_len);
    capsule
}

fn receipt_lineage_identity(
    receipts: &[(Vec<u8>, &[u8])],
    index: usize,
) -> TargetLineageIdentityV3 {
    let (payload, domain) = &receipts[index];
    TargetLineageIdentityV3::new(identity(domain, payload), payload.len() as u64).unwrap()
}

#[derive(Clone, Copy)]
enum TargetLineageSubstitution {
    TargetBinding,
    AmdgpuLowering,
    SemanticToLlvm,
}

fn capsule_with_target_lineage_substitution(
    source: &InertProductionSemanticCapsuleV3,
    replacement: &InertProductionSemanticCapsuleV3,
    substitution: TargetLineageSubstitution,
) -> InertProductionSemanticCapsuleV3 {
    let source_receipts = source.receipts();
    let replacement_receipts = replacement.receipts();
    let target_binding = if matches!(substitution, TargetLineageSubstitution::TargetBinding) {
        replacement_receipts.target_binding()
    } else {
        source_receipts.target_binding()
    };
    let amdgpu_lowering = if matches!(substitution, TargetLineageSubstitution::AmdgpuLowering) {
        replacement_receipts.amdgpu_lowering()
    } else {
        source_receipts.amdgpu_lowering()
    };
    let semantic_to_llvm = if matches!(substitution, TargetLineageSubstitution::SemanticToLlvm) {
        replacement_receipts.semantic_to_llvm()
    } else {
        source_receipts.semantic_to_llvm()
    };
    let receipts = OrderedInertSemanticLineageReceiptsV3::new(
        InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
            source_receipts
                .rustc_identity_inventory()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
        InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
            source_receipts
                .rustc_preflight_plan()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
            source_receipts.semantic_mir().canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertMiddleEndReceiptV3::from_canonical_preimage(
            source_receipts.middle_end().canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertKernelIrReceiptV3::from_canonical_preimage(
            source_receipts.kernel_ir().canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
            source_receipts
                .mir_to_kir_correspondence()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
        InertFormalMemoryReceiptV3::from_canonical_preimage(
            source_receipts
                .formal_memory()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
        InertProofBindingReceiptV3::from_canonical_preimage(
            source_receipts
                .proof_binding()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
        InertTargetBindingReceiptV3::from_canonical_preimage(
            target_binding.canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertDataLayoutReceiptV3::from_canonical_preimage(
            source_receipts.data_layout().canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertAbiReceiptV3::from_canonical_preimage(
            source_receipts.abi().canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertExportManifestReceiptV3::from_canonical_preimage(
            source_receipts
                .export_manifest()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
        InertAmdgpuLoweringReceiptV3::from_canonical_preimage(
            amdgpu_lowering.canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertSemanticToLlvmReceiptV3::from_canonical_preimage(
            semantic_to_llvm.canonical_preimage().to_vec(),
        )
        .unwrap(),
        InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
            source_receipts
                .final_compiler_module_commitment()
                .canonical_preimage()
                .to_vec(),
        )
        .unwrap(),
    );
    InertProductionSemanticCapsuleV3::new(source.invocation().clone(), source.target(), receipts)
        .unwrap()
}

fn validate_fixture_target_lineage(
    capsule: &InertProductionSemanticCapsuleV3,
) -> Result<ValidatedCompilerTargetLineageV1, CompilerTargetLineageValidationErrorV1> {
    let receipts = capsule.receipts();
    let proof_inputs = validate_compiler_proof_inputs_v4(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .unwrap();
    validate_compiler_target_lineage_v1(capsule, &proof_inputs).map(|lineage| {
        assert!(lineage.has_exact_receipt_association());
        assert!(lineage.has_exact_kir_to_llvm_replay());
        assert!(!lineage.establishes_semantic_refinement());
        assert!(!lineage.establishes_llvm_to_machine_refinement());
        assert!(!lineage.authenticates_producer());
        assert!(!lineage.grants_runtime_authority());
        lineage
    })
}

#[test]
fn singleton_target_lineage_accepts_semantic_debug_receipt_extensions() {
    for fixture in [
        OptionalSemanticDebugFixture::Unavailable(
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        ),
        OptionalSemanticDebugFixture::Available,
    ] {
        let outer = outer_for_kernels_with_optional_semantic_debug(
            0x20,
            0x11,
            &[],
            &[("vecadd", "vecadd.kd")],
            DescriptorLineageMutation::Exact,
            fixture,
        );
        let capsule = outer.capsule();
        let receipt = capsule.receipts().semantic_to_llvm();
        let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
            receipt.canonical_preimage(),
        )
        .unwrap();
        let lineage = validate_fixture_target_lineage(capsule).unwrap();

        assert_eq!(
            lineage.semantic_to_llvm_association_bytes(),
            extension.association_v3()
        );
        assert_eq!(
            lineage.semantic_to_llvm().canonical_bytes(),
            extension.association_v3()
        );
        assert_eq!(
            lineage.semantic_to_llvm_receipt_identity().sha256(),
            *receipt.identity().sha256()
        );
        assert_eq!(
            lineage.semantic_to_llvm_receipt_identity().byte_len(),
            receipt.identity().byte_len()
        );
    }
}

#[test]
fn singleton_target_lineage_rejects_malformed_semantic_debug_receipt_extension() {
    let handoff = module_handoff_for_kernels(0x11, &[], &[("vecadd", "vecadd.kd")]);
    let base = InertProductionSemanticCapsuleV3::decode(&capsule_bytes_with_semantic_to_llvm(
        0x20,
        &handoff,
        DescriptorLineageMutation::Exact,
        None,
        Some(ProductionAmdTargetProfileV1::Gfx942),
        None,
    ))
    .unwrap();
    let association = SemanticToLlvmAssociationTranscriptV3::decode(
        base.receipts().semantic_to_llvm().canonical_preimage(),
    )
    .unwrap();
    let carrier = ProductionSemanticDebugCarrierV1::new(
        association.canonical_bytes(),
        ProductionSemanticDebugAvailabilityV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        ),
    )
    .unwrap();
    let mut malformed =
        ProductionSemanticDebugReceiptExtensionV1::new(association.canonical_bytes(), carrier)
            .unwrap()
            .canonical_bytes()
            .to_vec();
    malformed.push(0);
    let hostile = InertProductionSemanticCapsuleV3::decode(&capsule_bytes_with_semantic_to_llvm(
        0x20,
        &handoff,
        DescriptorLineageMutation::Exact,
        Some(&malformed),
        Some(ProductionAmdTargetProfileV1::Gfx942),
        None,
    ))
    .unwrap();

    assert!(matches!(
        validate_fixture_target_lineage(&hostile),
        Err(
            CompilerTargetLineageValidationErrorV1::SemanticDebugExtension(
                ProductionSemanticDebugFragmentErrorV1::InvalidEncoding
            )
        )
    ));
}

#[test]
fn singleton_target_lineage_rejects_cross_spliced_semantic_debug_receipt_extension() {
    let outer = outer_for_kernels_with_optional_semantic_debug(
        0x20,
        0x11,
        &[],
        &[("vecadd", "vecadd.kd")],
        DescriptorLineageMutation::Exact,
        OptionalSemanticDebugFixture::UnavailableCrossSpliced,
    );

    assert!(matches!(
        validate_fixture_target_lineage(outer.capsule()),
        Err(CompilerTargetLineageValidationErrorV1::IdentityMismatch {
            field: "semantic MIR"
        })
    ));
}

#[test]
fn singleton_target_lineage_rejects_rehashed_cross_compilation_splices() {
    let handoff = module_handoff_for_kernels(0x11, &[], &[("vecadd", "vecadd.kd")]);
    let first = InertProductionSemanticCapsuleV3::decode(&capsule_bytes(
        0x20,
        &handoff,
        DescriptorLineageMutation::Exact,
    ))
    .unwrap();
    let second = InertProductionSemanticCapsuleV3::decode(&capsule_bytes(
        0x40,
        &handoff,
        DescriptorLineageMutation::Exact,
    ))
    .unwrap();
    drop(validate_fixture_target_lineage(&first).unwrap());
    for substitution in [
        TargetLineageSubstitution::TargetBinding,
        TargetLineageSubstitution::AmdgpuLowering,
        TargetLineageSubstitution::SemanticToLlvm,
    ] {
        let hostile = capsule_with_target_lineage_substitution(&first, &second, substitution);
        assert!(validate_fixture_target_lineage(&hostile).is_err());
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn hex_decode(encoded: &[u8]) -> Option<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    encoded
        .chunks_exact(2)
        .map(|pair| Some((decode_hex_nibble(pair[0])? << 4) | decode_hex_nibble(pair[1])?))
        .collect()
}

const fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn proof_binding_association_payload(
    receipts: &[(Vec<u8>, &[u8])],
    verus_execution: &[u8],
) -> Vec<u8> {
    let mut identities = Vec::with_capacity(5);
    for (payload, domain) in receipts.iter().take(7).skip(2) {
        identities.push(
            InertLineageContentIdentityV3::new(identity(domain, payload), payload.len() as u64)
                .unwrap(),
        );
    }
    InertProofBindingAssociationV4::new(
        InertProofBindingAssociationInputsV4::new(
            identities[0],
            identities[1],
            identities[2],
            identities[3],
            identities[4],
        ),
        verus_execution,
    )
    .unwrap()
    .canonical_bytes()
    .to_vec()
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

fn invocation_bytes(seed: u8) -> Vec<u8> {
    let encoded = match seed {
        0x20 => INVOCATION_20_HEX,
        0x40 => INVOCATION_40_HEX,
        _ => panic!("unsupported strict invocation fixture seed {seed:#x}"),
    };
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn invocation_bytes_for_target(seed: u8, target: &str) -> Vec<u8> {
    let mut invocation = invocation_bytes(seed);
    if target != TARGET {
        assert_eq!(target.len(), TARGET.len());
        let offset = invocation
            .windows(TARGET.len())
            .position(|window| window == TARGET.as_bytes())
            .expect("strict invocation fixture contains its target");
        invocation[offset..offset + target.len()].copy_from_slice(target.as_bytes());
    }
    invocation
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("non-canonical fixture hex"),
    }
}

fn push_blob(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    digest.finalize().into()
}

fn producer() -> ProducerIdentity {
    ProducerIdentity::from_codegen(
        "worker_v3_hsaco_admission_fixture",
        Some(Path::new("tests/worker_v3_hsaco_admission.rs")),
    )
    .unwrap()
}

pub(crate) struct TestDirectory(pub(crate) PathBuf);

impl TestDirectory {
    pub(crate) fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v3-admission-{}-{}",
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
