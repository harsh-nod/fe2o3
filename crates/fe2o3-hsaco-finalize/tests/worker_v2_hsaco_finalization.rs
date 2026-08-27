#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, CompilerModuleHandoffSlotV2, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_in_slot_v2,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_in_slot_v2,
    publish_compiler_module_handoff_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1,
    DEVICE_DESCRIPTOR_SECTION_ALIGNMENT, DescriptorSourceEvidenceRequirementV1, FinalizationError,
    LinkOptionV1, MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2, PinnedWorkerV1,
    ProtectedWorkerV2FinalizerLineageDecodeErrorV2, ProtectedWorkerV2FinalizerLineageV2,
    RowSoftmaxV1StructuralArtifactErrorV1, TiledGemmV1StructuralArtifactErrorV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerV2HsacoFinalizationError, WorkerV2RawHsacoInspectionError,
    derive_unfinalized_hsaco_from_finalized_v1,
    execute_protected_reproducible_first_build_worker_v2,
    execute_reproducible_first_build_worker_v2, finalize_inspected_protected_worker_v2_hsaco_v2,
    finalize_inspected_worker_v2_hsaco_v1, finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
    finalize_tiled_gemm_v1_structural_worker_v2_hsaco_v1, inspect_protected_worker_v2_raw_hsaco_v1,
    inspect_row_softmax_v1_structural_worker_v2_hsaco_v1,
    inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1, inspect_unfinalized,
    inspect_worker_v2_raw_hsaco_v1, verify_finalized,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    ProducerIdentityV1, RowSoftmaxV1StructuralDescriptorErrorV1,
    RowSoftmaxV1StructuralDescriptorExpectationV1, ScalarTypeV1, SourceTypeDescriptorV1,
    SourceTypeRecordV1, Text, TiledGemmV1StructuralDescriptorErrorV1,
    TiledGemmV1StructuralDescriptorExpectationV1, ValidName, decode_device_descriptor_table_v1,
    encode_device_descriptor_table_v1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};
use sha2::{Digest, Sha256};

include!("fixtures/worker_v2_hsaco_test_support.rs");

const GFX942_TARGET: &str = "gfx942:xnack-";

fn canonical_fixture_options() -> FixtureOptions<'static> {
    FixtureOptions {
        target: GFX942_TARGET,
        ..FixtureOptions::valid()
    }
}

#[test]
fn canonical_fixture_controls_required_workgroup_metadata_presence() {
    let present = fixture(canonical_fixture_options());
    assert_eq!(
        fe2o3_hsaco::inspect(&present.bytes)
            .unwrap()
            .target()
            .to_string(),
        GFX942_TARGET
    );
    assert_eq!(
        fe2o3_hsaco::inspect(&present.bytes).unwrap().kernels()[0].required_workgroup_size(),
        Some([256, 1, 1])
    );

    let mut options = canonical_fixture_options();
    options.include_required_workgroup_size = false;
    let omitted = fixture(options);
    assert_eq!(
        fe2o3_hsaco::inspect(&omitted.bytes).unwrap().kernels()[0].required_workgroup_size(),
        None
    );
}

#[test]
fn missing_descriptor_source_returns_an_owning_fail_closed_blocker() {
    let fixture = fixture(canonical_fixture_options());
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, GFX942_TARGET, 0x41, 0x51)).unwrap();
    let raw_identity = raw.identity();
    let source_identity = raw.source_evidence_identity();
    let output_identity = raw.linked_output_identity();
    let policy_identity = raw.policy().identity();
    let attempt = raw.attempt();

    let blocker = match finalize_inspected_worker_v2_hsaco_v1(raw) {
        Err(WorkerV2HsacoFinalizationError::MissingAuthenticatedDescriptorSourceEvidence(
            blocker,
        )) => blocker,
        result => panic!("expected missing descriptor-source blocker, found {result:?}"),
    };

    assert_eq!(
        blocker.requirement(),
        DescriptorSourceEvidenceRequirementV1::AuthenticatedCanonicalDescriptorTableV1
    );
    assert_eq!(blocker.raw_inspection_identity(), raw_identity);
    assert_eq!(blocker.source_evidence_identity(), source_identity);
    assert_eq!(blocker.raw_output_identity(), output_identity);
    assert_eq!(blocker.policy_identity(), policy_identity);
    assert_eq!(blocker.attempt(), attempt);
    assert_eq!(blocker.target().to_string(), GFX942_TARGET);
    assert_eq!(blocker.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(blocker.observed_kernels().len(), 1);
    assert_eq!(blocker.observed_kernels()[0].entry(), "vecadd");
    assert_eq!(
        blocker.canonical_descriptor_section(),
        CanonicalDescriptorSectionObservationV1::Missing
    );
    assert!(!blocker.may_infer_descriptor_claims_from_executable_metadata());
    assert!(!blocker.grants_publication_authority());
    assert!(!blocker.grants_load_authority());
    assert!(!blocker.grants_launch_authority());
}

#[test]
fn structurally_finalizes_and_retains_raw_and_finalized_lineage() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let unfinalized = inspect_unfinalized(&raw_bytes).unwrap();
    let digest_offset = unfinalized.location().digest_offset();
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(raw_bytes.clone(), GFX942_TARGET, 0x42, 0x52))
            .unwrap();
    let raw_identity = raw.identity();
    let source_identity = raw.source_evidence_identity();
    let raw_output = raw.linked_output_identity();
    let policy = raw.policy().identity();

    let prepared = finalize_inspected_worker_v2_hsaco_v1(raw).unwrap();
    let finalized = prepared.exact_finalized_bytes();
    let verified = verify_finalized(finalized).unwrap();

    assert_eq!(prepared.raw_inspection_identity(), raw_identity);
    assert_eq!(prepared.source_evidence_identity(), source_identity);
    assert_eq!(prepared.raw_output_identity(), raw_output);
    assert_eq!(prepared.policy_identity(), policy);
    assert!(raw_output.matches(&raw_bytes));
    assert!(prepared.finalized_output_identity().matches(finalized));
    assert_ne!(
        prepared.raw_output_identity(),
        prepared.finalized_output_identity()
    );
    assert_eq!(prepared.canonical_digest(), verified.digest());
    assert_eq!(prepared.target().to_string(), GFX942_TARGET);
    assert_eq!(prepared.code_object_version(), CodeObjectVersion::V6);
    assert!(prepared.canonical_descriptor_finalization_ran());
    assert!(!prepared.has_authenticated_descriptor_source_evidence());
    assert!(prepared.is_structural_only());
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.proves_verus_verification());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());
    assert_ne!(prepared.identity().as_bytes(), &[0; 32]);

    for (index, (before, after)) in raw_bytes.iter().zip(finalized).enumerate() {
        if !(digest_offset..digest_offset + 32).contains(&index) {
            assert_eq!(before, after, "byte {index} outside digest slot changed");
        }
    }
}

#[test]
fn descriptor_and_finalized_byte_tampering_fail_closed() {
    let mut table = descriptor_table(GFX942_TARGET);
    table[16] = 1;
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, GFX942_TARGET, 0x43, 0x53)).unwrap();
    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::ExpectedZeroDigest
        ))
    ));

    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let text_offset = fixture.text_offset;
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, GFX942_TARGET, 0x44, 0x54)).unwrap();
    let prepared = finalize_inspected_worker_v2_hsaco_v1(raw).unwrap();
    let mut tampered = prepared.exact_finalized_bytes().to_vec();
    tampered[text_offset] ^= 1;
    assert!(matches!(
        verify_finalized(&tampered),
        Err(FinalizationError::CanonicalDigestMismatch { .. })
    ));
}

#[test]
fn rejects_double_finalization() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, GFX942_TARGET, 0x45, 0x55)).unwrap();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .unwrap()
        .exact_finalized_bytes()
        .to_vec();

    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(finalized, GFX942_TARGET, 0x46, 0x56)).unwrap();
    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::ExpectedZeroDigest
        ))
    ));
}

#[test]
fn rejects_descriptor_target_mismatch_without_weakening_raw_target_policy() {
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, GFX942_TARGET, 0x47, 0x57)).unwrap();
    assert_eq!(raw.target().to_string(), GFX942_TARGET);
    assert_eq!(
        decode_device_descriptor_table_v1(&table)
            .unwrap()
            .device_target()
            .to_string(),
        "gfx942"
    );

    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::DeviceTargetMismatch
        ))
    ));
}

#[test]
fn finalization_identity_binds_lineage_separately_from_finalized_content() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let first = prepare(fixture.bytes.clone(), GFX942_TARGET, 0x48, 0x58);
    let other_lineage = prepare(fixture.bytes.clone(), GFX942_TARGET, 0x49, 0x59);

    assert_eq!(
        first.finalized_output_identity(),
        other_lineage.finalized_output_identity()
    );
    assert_eq!(first.canonical_digest(), other_lineage.canonical_digest());
    assert_ne!(
        first.raw_inspection_identity(),
        other_lineage.raw_inspection_identity()
    );
    assert_ne!(first.identity(), other_lineage.identity());

    let mut changed = fixture.bytes;
    changed[fixture.text_offset] ^= 1;
    let changed = prepare(changed, GFX942_TARGET, 0x4a, 0x5a);
    assert_ne!(first.raw_output_identity(), changed.raw_output_identity());
    assert_ne!(
        first.finalized_output_identity(),
        changed.finalized_output_identity()
    );
    assert_ne!(first.canonical_digest(), changed.canonical_digest());
    assert_ne!(first.identity(), changed.identity());
}

#[test]
fn protected_missing_descriptor_retains_exact_v2_lineage() {
    let fixture = fixture(canonical_fixture_options());
    let closure = compiler_closure(0x31);
    let slot = CompilerModuleHandoffSlotV2::GeneralGemmReference;
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x71,
        0x81,
        closure,
        slot,
    ))
    .unwrap();
    let attempt = raw.attempt();
    let handoff = raw.handoff_identity();
    let inspection = raw.identity();

    let blocker = match finalize_inspected_protected_worker_v2_hsaco_v2(raw) {
        Err(
            WorkerV2HsacoFinalizationError::MissingAuthenticatedProtectedDescriptorSourceEvidence(
                blocker,
            ),
        ) => blocker,
        result => panic!("expected protected descriptor-source blocker, found {result:?}"),
    };
    assert_eq!(blocker.attempt(), attempt);
    assert_eq!(blocker.handoff_slot(), slot);
    assert_eq!(blocker.handoff_identity(), handoff);
    assert_eq!(blocker.compiler_closure(), closure);
    assert_eq!(blocker.raw_inspection_identity(), inspection);
    assert_eq!(
        blocker.requirement(),
        DescriptorSourceEvidenceRequirementV1::AuthenticatedCanonicalDescriptorTableV1
    );
    assert_eq!(
        blocker.canonical_descriptor_section(),
        CanonicalDescriptorSectionObservationV1::Missing
    );
    assert!(!blocker.may_infer_descriptor_claims_from_executable_metadata());
    assert!(!blocker.grants_publication_authority());
}

#[test]
fn protected_finalization_preserves_closure_handoff_and_exact_bytes() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let digest_offset = inspect_unfinalized(&raw_bytes)
        .unwrap()
        .location()
        .digest_offset();
    let closure = compiler_closure(0x41);
    let slot = CompilerModuleHandoffSlotV2::GeneralGemmVectorizedAOnly;
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x72,
        0x82,
        closure,
        slot,
    ))
    .unwrap();
    let attempt = raw.attempt();
    let handoff = raw.handoff_identity();
    let inspection = raw.identity();

    let finalized = finalize_inspected_protected_worker_v2_hsaco_v2(raw).unwrap();
    assert_eq!(finalized.attempt(), attempt);
    assert_eq!(finalized.handoff_slot(), slot);
    assert_eq!(finalized.handoff_identity(), handoff);
    assert_eq!(finalized.compiler_closure(), closure);
    assert_eq!(finalized.raw_inspection_identity(), inspection);
    assert!(finalized.raw_output_identity().matches(&raw_bytes));
    assert!(
        finalized
            .finalized_output_identity()
            .matches(finalized.exact_finalized_bytes())
    );
    assert_eq!(
        finalized.canonical_digest(),
        verify_finalized(finalized.exact_finalized_bytes())
            .unwrap()
            .digest()
    );
    assert_eq!(
        derive_unfinalized_hsaco_from_finalized_v1(finalized.exact_finalized_bytes()).unwrap(),
        raw_bytes
    );
    assert_eq!(
        derive_unfinalized_hsaco_from_finalized_v1(&raw_bytes).unwrap_err(),
        FinalizationError::ExpectedFinalizedDigest
    );
    let mut mutated_finalized = finalized.exact_finalized_bytes().to_vec();
    mutated_finalized[0x80] ^= 1;
    assert!(derive_unfinalized_hsaco_from_finalized_v1(&mutated_finalized).is_err());
    for (index, (before, after)) in raw_bytes
        .iter()
        .zip(finalized.exact_finalized_bytes())
        .enumerate()
    {
        if !(digest_offset..digest_offset + 32).contains(&index) {
            assert_eq!(before, after, "byte {index} outside digest slot changed");
        }
    }
}

#[test]
fn typed_protected_construction_and_wire_replay_share_canonical_validation() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x74,
        0x84,
        compiler_closure(0x61),
        CompilerModuleHandoffSlotV2::GeneralGemmReference,
    ))
    .unwrap();
    let raw_transcript = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw).unwrap();
    let raw_wire = raw_transcript.canonical_bytes();
    let decoded_raw =
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&raw_wire, &exact_raw, &exact_raw)
            .unwrap();
    assert_eq!(decoded_raw, raw_transcript);
    assert!(decoded_raw.matches_inspected_source(&raw));
    assert!(!decoded_raw.independently_rederives_transaction_handoff_identity());
    assert!(!decoded_raw.grants_compiler_authority());
    assert!(!decoded_raw.grants_publication_authority());
    assert!(!decoded_raw.grants_load_authority());
    assert!(!decoded_raw.grants_launch_authority());

    let finalized = finalize_inspected_protected_worker_v2_hsaco_v2(raw).unwrap();
    let exact_final = finalized.exact_finalized_bytes();
    let final_transcript = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&finalized).unwrap();
    let final_wire = final_transcript.canonical_bytes();
    let decoded_final =
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&final_wire, &exact_raw, exact_final)
            .unwrap();
    assert_eq!(decoded_final, final_transcript);
    assert!(decoded_final.matches_finalized_source(&finalized));
    decoded_final
        .validate_descriptor_table(verify_finalized(exact_final).unwrap().descriptor_table())
        .unwrap();
}

#[test]
fn protected_finalizer_lineage_requires_both_exact_worker_outputs() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x21,
        0x31,
        compiler_closure(0x41),
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let canonical = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw)
        .unwrap()
        .canonical_bytes();

    for (response_segment, expected_field) in [
        (4, "missing bootstrap worker output"),
        (6, "missing authorized worker output"),
    ] {
        let mut wire = canonical.clone();
        let response = lineage_segment(&wire, response_segment).to_vec();
        replace_lineage_segment(
            &mut wire,
            response_segment,
            &worker_response_without_output(&response),
        );
        match ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&wire, &exact_raw, &exact_raw) {
            Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(field)) => {
                assert_eq!(field, expected_field);
            }
            result => panic!("expected missing-output rejection, found {result:?}"),
        }
    }
}

#[test]
fn protected_finalizer_lineage_rejects_canonical_manifest_role_disagreement() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x22,
        0x32,
        compiler_closure(0x42),
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let mut wire = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw)
        .unwrap()
        .canonical_bytes();
    let alternate_manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "vecadd"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "vecadd.kd"),
        (
            CompilerModuleSymbolRoleV1::DeviceFfiExport,
            "self_consistent_forged_export",
        ),
    ])
    .unwrap();
    replace_lineage_segment(&mut wire, 1, alternate_manifest.canonical_bytes());

    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&wire, &exact_raw, &exact_raw,),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
            "compiler envelope and symbol manifest roles"
        ))
    ));
}

#[test]
fn protected_finalizer_lineage_rejects_coordinated_module_and_exchange_substitution() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let closure = compiler_closure(0x43);
    let original = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes.clone(),
        GFX942_TARGET,
        0x23,
        0x33,
        closure,
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let alternate =
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence_with_module_prefix(
            fixture.bytes,
            GFX942_TARGET,
            0x23,
            0x33,
            closure,
            CompilerModuleHandoffSlotV2::Default,
            b"different-valid-compiler-module-prefix",
        ))
        .unwrap();
    assert_ne!(original.handoff_identity(), alternate.handoff_identity());

    let mut wire = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&original)
        .unwrap()
        .canonical_bytes();
    let alternate_wire = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&alternate)
        .unwrap()
        .canonical_bytes();
    for segment in 3..=6 {
        replace_lineage_segment(
            &mut wire,
            segment,
            lineage_segment(&alternate_wire, segment),
        );
    }
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&wire, &exact_raw, &exact_raw),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
            "protected bootstrap request identity"
        ))
    ));

    let decoded_alternate = ProtectedWorkerV2FinalizerLineageV2::decode_canonical(
        &alternate_wire,
        &exact_raw,
        &exact_raw,
    )
    .unwrap();
    assert!(!decoded_alternate.independently_rederives_transaction_handoff_identity());
    assert!(!decoded_alternate.grants_compiler_authority());
}

#[test]
fn protected_finalizer_lineage_enforces_the_aggregate_wire_maximum() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x31,
        0x41,
        compiler_closure(0x51),
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let mut wire = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw)
        .unwrap()
        .canonical_bytes();
    let padding_bytes = MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2 - wire.len();
    let mut padded_observation = lineage_segment(&wire, 7).to_vec();
    padded_observation.resize(padded_observation.len() + padding_bytes, 0);
    replace_lineage_segment(&mut wire, 7, &padded_observation);
    assert_eq!(
        wire.len(),
        MAX_PROTECTED_WORKER_V2_FINALIZER_LINEAGE_BYTES_V2
    );
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&wire, &exact_raw, &exact_raw),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
            "policy or parsed observation"
        ))
    ));
    wire.push(0);
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&wire, &exact_raw, &exact_raw),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)
    ));
}

#[test]
fn protected_finalizer_lineage_rejects_resealed_substitutions_and_bad_bounds() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let first = finalize_inspected_protected_worker_v2_hsaco_v2(
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
            fixture.bytes.clone(),
            GFX942_TARGET,
            0x75,
            0x85,
            compiler_closure(0x71),
            CompilerModuleHandoffSlotV2::Default,
        ))
        .unwrap(),
    )
    .unwrap();
    let second = finalize_inspected_protected_worker_v2_hsaco_v2(
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
            fixture.bytes,
            GFX942_TARGET,
            0x75,
            0x85,
            compiler_closure(0x81),
            CompilerModuleHandoffSlotV2::Default,
        ))
        .unwrap(),
    )
    .unwrap();
    let exact_final = first.exact_finalized_bytes();
    assert_eq!(exact_final, second.exact_finalized_bytes());
    let mut substituted = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&first)
        .unwrap()
        .canonical_bytes();
    let donor = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&second)
        .unwrap()
        .canonical_bytes();
    substituted[11..43].copy_from_slice(&donor[11..43]);
    substituted[43..75].copy_from_slice(&donor[43..75]);
    substituted[76..108].copy_from_slice(&donor[76..108]);
    let closure_offset = lineage_closure_offset(&substituted);
    let donor_closure_offset = lineage_closure_offset(&donor);
    substituted[closure_offset - 32..closure_offset + 226]
        .copy_from_slice(&donor[donor_closure_offset - 32..donor_closure_offset + 226]);
    reseal_lineage_wire(&mut substituted);
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(
            &substituted,
            &exact_raw,
            exact_final,
        ),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::RawLineage(
            "protected bootstrap request identity"
        ))
    ));

    let canonical = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&first)
        .unwrap()
        .canonical_bytes();
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(
            &canonical[..canonical.len() - 1],
            &exact_raw,
            exact_final,
        ),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Checksum)
            | Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)
    ));

    let mut trailing = canonical.clone();
    trailing.insert(trailing.len() - 32, 0);
    reseal_lineage_wire(&mut trailing);
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&trailing, &exact_raw, exact_final,),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::TrailingBytes)
    ));

    let mut oversized_segment = canonical;
    let segment_offset = lineage_first_segment_offset(&oversized_segment);
    oversized_segment[segment_offset..segment_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    reseal_lineage_wire(&mut oversized_segment);
    assert!(matches!(
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(
            &oversized_segment,
            &exact_raw,
            exact_final,
        ),
        Err(ProtectedWorkerV2FinalizerLineageDecodeErrorV2::Length)
    ));
}

#[test]
fn protected_finalizer_lineage_requires_the_exact_canonical_descriptor_table() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x76,
        0x86,
        compiler_closure(0x91),
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let transcript = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw).unwrap();
    let correct = decode_device_descriptor_table_v1(&table).unwrap();
    transcript.validate_descriptor_table(&correct).unwrap();

    let substituted = coordinated_semantic_substitution_table(GFX942_TARGET);
    let correct_kernel = &correct.kernels()[0];
    let substituted_kernel = &substituted.kernels()[0];
    assert_eq!(
        substituted.canonical_code_object_digest(),
        correct.canonical_code_object_digest()
    );
    assert_eq!(substituted.device_target(), correct.device_target());
    assert_eq!(
        substituted.code_object_version(),
        correct.code_object_version()
    );
    assert_eq!(substituted_kernel.entry_name(), correct_kernel.entry_name());
    assert_eq!(
        substituted_kernel.descriptor_symbol(),
        correct_kernel.descriptor_symbol()
    );
    assert_eq!(
        substituted_kernel.abi_layout().kernarg_segment_size(),
        correct_kernel.abi_layout().kernarg_segment_size()
    );
    assert_eq!(
        substituted_kernel.abi_layout().kernarg_segment_alignment(),
        correct_kernel.abi_layout().kernarg_segment_alignment()
    );
    assert_eq!(
        substituted_kernel.launch().block_size(),
        correct_kernel.launch().block_size()
    );
    assert_eq!(
        substituted_kernel.launch().max_flat_workgroup_size(),
        correct_kernel.launch().max_flat_workgroup_size()
    );
    assert_eq!(
        substituted_kernel.launch().static_shared_memory_bytes(),
        correct_kernel.launch().static_shared_memory_bytes()
    );
    assert_ne!(substituted, correct);
    assert!(matches!(
        transcript.validate_descriptor_table(&substituted),
        Err(
            ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                "canonical descriptor table"
            )
        )
    ));
}

#[test]
fn protected_finalizer_lineage_fails_closed_without_a_descriptor_section() {
    let fixture = fixture(canonical_fixture_options());
    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x77,
        0x87,
        compiler_closure(0x97),
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let transcript = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw).unwrap();
    let candidate = decode_device_descriptor_table_v1(&descriptor_table(GFX942_TARGET)).unwrap();
    assert!(matches!(
        transcript.validate_descriptor_table(&candidate),
        Err(
            ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                "canonical descriptor section is missing"
            )
        )
    ));
}

#[test]
fn protected_finalizer_lineage_preserves_cov6_explicit_only_kernarg_reconciliation() {
    let explicit_size = 40;
    let total_size = 296;
    let table = cov6_explicit_only_descriptor_table(explicit_size, total_size);
    let mut options = canonical_fixture_options();
    options.kernarg_segment_size_override = Some(u64::from(explicit_size));
    let mut fixture = fixture_with_descriptor_table(options, Some(&table));
    replace_fixture_metadata_with_explicit_only_cov6(&mut fixture.bytes, options, explicit_size);
    let exact_raw = fixture.bytes.clone();

    let unfinalized = inspect_unfinalized(&exact_raw).unwrap();
    assert_eq!(
        unfinalized.descriptor_table().kernels()[0]
            .abi_layout()
            .explicit_argument_size(),
        explicit_size
    );
    assert_eq!(
        unfinalized.descriptor_table().kernels()[0]
            .abi_layout()
            .kernarg_segment_size(),
        total_size
    );
    assert_eq!(
        unfinalized.hsaco().kernels()[0].kernarg_segment_size(),
        u64::from(explicit_size)
    );

    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
        fixture.bytes,
        GFX942_TARGET,
        0x78,
        0x88,
        compiler_closure(0x98),
        CompilerModuleHandoffSlotV2::Default,
    ))
    .unwrap();
    let raw_lineage = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw).unwrap();
    raw_lineage
        .validate_descriptor_table(unfinalized.descriptor_table())
        .unwrap();

    let finalized = finalize_inspected_protected_worker_v2_hsaco_v2(raw).unwrap();
    let final_inspection = verify_finalized(finalized.exact_finalized_bytes()).unwrap();
    let final_lineage = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&finalized).unwrap();
    final_lineage
        .validate_descriptor_table(final_inspection.descriptor_table())
        .unwrap();
    let wire = final_lineage.canonical_bytes();
    ProtectedWorkerV2FinalizerLineageV2::decode_canonical(
        &wire,
        &exact_raw,
        finalized.exact_finalized_bytes(),
    )
    .unwrap();
}

#[test]
fn protected_finalizer_lineage_derives_and_joins_an_exact_two_kernel_table() {
    let kernels = [("alpha", "alpha.kd"), ("zeta", "zeta.kd")];
    let raw_table =
        two_kernel_descriptor_table(CanonicalCodeObjectDigest::from_bytes([0; 32]), false);
    let table_bytes = encode_device_descriptor_table_v1(&raw_table).unwrap();
    let metadata = two_kernel_metadata(&kernels);
    let fixture = two_kernel_fixture(&table_bytes, &metadata, &kernels);
    let exact_raw = fixture.bytes.clone();
    let unfinalized = inspect_unfinalized(&exact_raw).unwrap();
    assert_eq!(unfinalized.descriptor_table(), &raw_table);
    assert_eq!(unfinalized.descriptor_table().kernels().len(), 2);

    let raw = inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence_for_kernels(
        fixture.bytes,
        GFX942_TARGET,
        0x79,
        0x89,
        compiler_closure(0x99),
        CompilerModuleHandoffSlotV2::Default,
        &kernels,
    ))
    .unwrap();
    let raw_lineage = ProtectedWorkerV2FinalizerLineageV2::from_inspected(&raw).unwrap();
    raw_lineage.validate_descriptor_table(&raw_table).unwrap();
    let raw_wire = raw_lineage.canonical_bytes();
    let decoded_raw =
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&raw_wire, &exact_raw, &exact_raw)
            .unwrap();
    decoded_raw.validate_descriptor_table(&raw_table).unwrap();

    let finalized = finalize_inspected_protected_worker_v2_hsaco_v2(raw).unwrap();
    let exact_final = finalized.exact_finalized_bytes();
    let verified = verify_finalized(exact_final).unwrap();
    assert_eq!(verified.descriptor_table().kernels().len(), 2);
    let final_lineage = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&finalized).unwrap();
    final_lineage
        .validate_descriptor_table(verified.descriptor_table())
        .unwrap();
    let final_wire = final_lineage.canonical_bytes();
    let decoded_final =
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(&final_wire, &exact_raw, exact_final)
            .unwrap();
    decoded_final
        .validate_descriptor_table(verified.descriptor_table())
        .unwrap();

    let substituted = two_kernel_descriptor_table(verified.digest(), true);
    assert_eq!(
        &substituted.kernels()[0],
        &verified.descriptor_table().kernels()[0],
        "the non-substituted kernel must remain exactly unchanged"
    );
    assert_ne!(
        &substituted.kernels()[1],
        &verified.descriptor_table().kernels()[1]
    );
    assert!(matches!(
        decoded_final.validate_descriptor_table(&substituted),
        Err(
            ProtectedWorkerV2FinalizerLineageDecodeErrorV2::DescriptorJoin(
                "canonical descriptor table"
            )
        )
    ));
}

#[test]
fn every_closure_role_mutation_changes_only_protected_finalization_lineage() {
    let table = descriptor_table(GFX942_TARGET);
    let fixture = fixture_with_descriptor_table(canonical_fixture_options(), Some(&table));
    let exact_raw = fixture.bytes.clone();
    let base = finalize_inspected_protected_worker_v2_hsaco_v2(
        inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
            fixture.bytes.clone(),
            GFX942_TARGET,
            0x73,
            0x83,
            compiler_closure(0x51),
            CompilerModuleHandoffSlotV2::Default,
        ))
        .unwrap(),
    )
    .unwrap();
    let base_transcript = ProtectedWorkerV2FinalizerLineageV2::from_finalized(&base).unwrap();

    for role in 0..6 {
        let changed_closure = compiler_closure_with_mutated_role(0x51, role);
        let changed = finalize_inspected_protected_worker_v2_hsaco_v2(
            inspect_protected_worker_v2_raw_hsaco_v1(protected_evidence(
                fixture.bytes.clone(),
                GFX942_TARGET,
                0x73,
                0x83,
                changed_closure,
                CompilerModuleHandoffSlotV2::Default,
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            base.exact_finalized_bytes(),
            changed.exact_finalized_bytes()
        );
        assert_eq!(
            base.finalized_output_identity(),
            changed.finalized_output_identity()
        );
        assert_ne!(base.compiler_closure(), changed.compiler_closure());
        assert_ne!(base.identity(), changed.identity());
        let changed_transcript =
            ProtectedWorkerV2FinalizerLineageV2::from_finalized(&changed).unwrap();
        assert_ne!(base_transcript.identity(), changed_transcript.identity());
        ProtectedWorkerV2FinalizerLineageV2::decode_canonical(
            &changed_transcript.canonical_bytes(),
            &exact_raw,
            changed.exact_finalized_bytes(),
        )
        .unwrap();
    }
}

#[test]
fn structural_tiled_gemm_v1_finalizes_as_four_slices_and_320_bytes() {
    let options = tiled_options();
    let table = tiled_descriptor_table(tiled_capabilities());
    let fixture = fixture_with_descriptor_table(options, Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let inspected = inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
        evidence_for(
            fixture.bytes,
            "gfx942:xnack-",
            0x81,
            0x91,
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
        tiled_expectation(),
    )
    .unwrap();

    assert_eq!(inspected.target().to_string(), "gfx942:xnack-");
    assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        inspected.descriptor_admission().workgroup_size(),
        [64, 1, 1]
    );
    assert_eq!(
        inspected.descriptor_admission().explicit_kernarg_bytes(),
        64
    );
    assert_eq!(inspected.descriptor_admission().total_kernarg_bytes(), 320);
    assert!(!inspected.authenticates_compiler_origin());
    assert!(!inspected.validates_kernel_body());
    assert!(!inspected.proves_bf16_isa_semantics());
    assert!(!inspected.proves_mfma_isa_semantics());
    assert!(!inspected.grants_launch_authority());

    let finalized = finalize_tiled_gemm_v1_structural_worker_v2_hsaco_v1(inspected).unwrap();
    let verified = verify_finalized(finalized.exact_finalized_bytes()).unwrap();
    let kernel = &verified.hsaco().kernels()[0];
    assert_eq!(kernel.kernarg_segment_size(), 320);
    assert_eq!(kernel.implicit_argument_offset(), Some(64));
    assert_eq!(kernel.implicit_argument_size(), 256);
    assert_eq!(kernel.explicit_arguments().len(), 8);
    for (index, argument) in kernel.explicit_arguments().iter().enumerate() {
        assert_eq!(argument.offset(), u64::try_from(index).unwrap() * 8);
    }
    assert_eq!(kernel.required_workgroup_size(), Some([64, 1, 1]));
    assert_eq!(kernel.max_flat_workgroup_size(), 64);
    assert_eq!(kernel.group_segment_fixed_size(), 0);
    assert!(finalized.raw_output_identity().matches(&raw_bytes));
    assert!(
        finalized
            .finalized_output_identity()
            .matches(finalized.exact_finalized_bytes())
    );
    assert!(finalized.canonical_descriptor_finalization_ran());
    assert!(!finalized.validates_kernel_body());
    assert!(!finalized.proves_bf16_isa_semantics());
    assert!(!finalized.proves_mfma_isa_semantics());
    assert!(!finalized.proves_verus_verification());
    assert!(!finalized.grants_publication_authority());
    assert!(!finalized.grants_load_authority());
    assert!(!finalized.grants_launch_authority());
}

#[test]
fn structural_tiled_gemm_admission_accepts_arbitrary_text_without_body_authority() {
    let table = tiled_descriptor_table(tiled_capabilities());
    for (fill, source_identity, executable_identity) in [(0x00, 0x86, 0x96), (0xff, 0x87, 0x97)] {
        let mut fixture = fixture_with_descriptor_table(tiled_options(), Some(&table));
        fixture.bytes[fixture.text_offset..fixture.text_offset + 64].fill(fill);
        let inspected = inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                source_identity,
                executable_identity,
                "tiled_gemm_v1",
                "tiled_gemm_v1.kd",
            ),
            tiled_expectation(),
        )
        .unwrap();

        assert!(!inspected.validates_kernel_body());
        assert!(!inspected.proves_bf16_isa_semantics());
        assert!(!inspected.proves_mfma_isa_semantics());
        assert!(!inspected.grants_launch_authority());
    }
}

#[test]
fn tiled_gemm_rejects_wg256_and_288_byte_frontend_probe_substitution() {
    let table = tiled_descriptor_table(tiled_capabilities());
    let wrong_required = fixture_with_descriptor_table(
        FixtureOptions {
            required_workgroup_size: [256, 1, 1],
            max_flat_workgroup_size: 256,
            ..tiled_options()
        },
        Some(&table),
    );
    let wrong_required = inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
        evidence_for(
            wrong_required.bytes,
            "gfx942:xnack-",
            0x82,
            0x92,
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
        tiled_expectation(),
    )
    .unwrap_err();
    let TiledGemmV1StructuralArtifactErrorV1::RawInspection(wrong_required) = wrong_required else {
        panic!("tiled required-workgroup mismatch did not retain raw inspection error");
    };
    assert_eq!(
        wrong_required,
        WorkerV2RawHsacoInspectionError::TiledGemmV1RequiredWorkgroupSizeMismatch {
            kernel: "tiled_gemm_v1".to_owned(),
            actual: Some([256, 1, 1]),
            expected: [64, 1, 1],
        }
    );
    assert_eq!(
        wrong_required.to_string(),
        "tiled GEMM V1 kernel tiled_gemm_v1 requires Some([256, 1, 1]), expected [64, 1, 1]"
    );

    let wrong_max = fixture_with_descriptor_table(
        FixtureOptions {
            max_flat_workgroup_size: 256,
            ..tiled_options()
        },
        Some(&table),
    );
    let wrong_max = inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
        evidence_for(
            wrong_max.bytes,
            "gfx942:xnack-",
            0x82,
            0x92,
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
        tiled_expectation(),
    )
    .unwrap_err();
    let TiledGemmV1StructuralArtifactErrorV1::RawInspection(wrong_max) = wrong_max else {
        panic!("tiled max-flat-workgroup mismatch did not retain raw inspection error");
    };
    assert_eq!(
        wrong_max,
        WorkerV2RawHsacoInspectionError::TiledGemmV1MaxFlatWorkgroupSizeMismatch {
            kernel: "tiled_gemm_v1".to_owned(),
            actual: 256,
            expected: 64,
        }
    );
    assert_eq!(
        wrong_max.to_string(),
        "tiled GEMM V1 kernel tiled_gemm_v1 max flat workgroup is 256, expected 64"
    );

    let fragment_probe_span = fixture_with_descriptor_table(
        FixtureOptions {
            kernarg_segment_size_override: Some(288),
            ..tiled_options()
        },
        Some(&table),
    );
    assert!(matches!(
        inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fragment_probe_span.bytes,
                "gfx942:xnack-",
                0x83,
                0x93,
                "tiled_gemm_v1",
                "tiled_gemm_v1.kd",
            ),
            tiled_expectation(),
        ),
        Err(TiledGemmV1StructuralArtifactErrorV1::RawInspection(_))
    ));
}

#[test]
fn tiled_gemm_rejects_capability_target_symbol_offset_and_lds_drift() {
    for capabilities in [
        vec![
            CapabilityV1::Subgroup,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
        ],
        vec![
            CapabilityV1::Subgroup,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
            CapabilityV1::AmdWmma,
        ],
    ] {
        let table = tiled_descriptor_table(capabilities);
        let fixture = fixture_with_descriptor_table(tiled_options(), Some(&table));
        assert!(matches!(
            inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
                evidence_for(
                    fixture.bytes,
                    "gfx942:xnack-",
                    0x84,
                    0x94,
                    "tiled_gemm_v1",
                    "tiled_gemm_v1.kd",
                ),
                tiled_expectation(),
            ),
            Err(TiledGemmV1StructuralArtifactErrorV1::DescriptorPolicy(
                TiledGemmV1StructuralDescriptorErrorV1::CapabilityProvenance
            ))
        ));
    }

    let table = tiled_descriptor_table(tiled_capabilities());
    for (options, target, entry, descriptor) in [
        (
            FixtureOptions {
                target: "gfx942",
                ..tiled_options()
            },
            "gfx942",
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
        (
            FixtureOptions {
                entry: "tiled_gemm_v1_alias",
                descriptor: "tiled_gemm_v1_alias.kd",
                ..tiled_options()
            },
            "gfx942:xnack-",
            "tiled_gemm_v1_alias",
            "tiled_gemm_v1_alias.kd",
        ),
        (
            FixtureOptions {
                tiled_first_argument_offset: 1,
                ..tiled_options()
            },
            "gfx942:xnack-",
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
        (
            FixtureOptions {
                group_segment_fixed_size: 1024,
                ..tiled_options()
            },
            "gfx942:xnack-",
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
        (
            FixtureOptions {
                wavefront_size: 32,
                descriptor_wavefront_size: 32,
                ..tiled_options()
            },
            "gfx942:xnack-",
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
    ] {
        let fixture = fixture_with_descriptor_table(options, Some(&table));
        assert!(
            inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1(
                evidence_for(fixture.bytes, target, 0x85, 0x95, entry, descriptor,),
                tiled_expectation(),
            )
            .is_err()
        );
    }
}

#[test]
fn structural_row_softmax_v1_finalizes_as_two_f32_slices_and_288_bytes() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    let fixture = fixture_with_descriptor_table(row_softmax_options(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let inspected = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
        evidence_for(
            fixture.bytes,
            "gfx942:xnack-",
            0xa1,
            0xb1,
            "row_softmax_v1",
            "row_softmax_v1.kd",
        ),
        row_softmax_expectation(),
    )
    .unwrap();

    assert_eq!(inspected.target().to_string(), "gfx942:xnack-");
    assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
    assert!(
        inspected
            .raw_inspection_identity()
            .as_bytes()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert_eq!(inspected.exact_bytes(), raw_bytes);
    let admitted = inspected.descriptor_admission();
    assert_eq!(admitted.workgroup_size(), [64, 1, 1]);
    assert_eq!(admitted.max_grid_size(), [1, 1, 1]);
    assert_eq!(admitted.explicit_kernarg_bytes(), 32);
    assert_eq!(admitted.total_kernarg_bytes(), 288);
    assert!(!inspected.authenticates_source_origin());
    assert!(!inspected.authenticates_compiler_origin());
    assert!(!inspected.validates_runtime_slice_lengths());
    assert!(!inspected.validates_kernel_body());
    assert!(!inspected.proves_functional_softmax());
    assert!(!inspected.proves_exp_implementation());
    assert!(!inspected.proves_numerical_contract());
    assert!(!inspected.proves_race_freedom());
    assert!(!inspected.proves_verus_verification());
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());

    let finalized = finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(inspected).unwrap();
    let verified = verify_finalized(finalized.exact_finalized_bytes()).unwrap();
    let kernel = &verified.hsaco().kernels()[0];
    assert_eq!(kernel.name(), "row_softmax_v1");
    assert_eq!(kernel.symbol(), "row_softmax_v1.kd");
    assert_eq!(kernel.required_workgroup_size(), Some([64, 1, 1]));
    assert_eq!(kernel.max_workgroups(), [None; 3]);
    assert_eq!(kernel.cluster_dims(), None);
    assert_eq!(kernel.kind(), fe2o3_hsaco::KernelKind::Normal);
    assert!(!kernel.uses_dynamic_stack());
    assert_eq!(kernel.max_flat_workgroup_size(), 64);
    assert_eq!(kernel.group_segment_fixed_size(), 0);
    assert_eq!(kernel.kernarg_segment_size(), 288);
    assert_eq!(kernel.implicit_argument_offset(), Some(32));
    assert_eq!(kernel.implicit_argument_size(), 256);
    assert_eq!(kernel.explicit_arguments().len(), 4);
    assert_eq!(kernel.hidden_arguments().len(), 19);
    assert!(
        kernel
            .hidden_arguments()
            .iter()
            .all(|argument| argument.value_kind() != fe2o3_hsaco::HiddenValueKind::DynamicLdsSize)
    );
    assert!(finalized.raw_output_identity().matches(&raw_bytes));
    assert!(
        finalized
            .finalized_output_identity()
            .matches(finalized.exact_finalized_bytes())
    );
    assert!(finalized.canonical_descriptor_finalization_ran());
    assert!(!finalized.authenticates_source_origin());
    assert!(!finalized.authenticates_compiler_origin());
    assert!(!finalized.validates_runtime_slice_lengths());
    assert!(!finalized.validates_kernel_body());
    assert!(!finalized.proves_functional_softmax());
    assert!(!finalized.proves_exp_implementation());
    assert!(!finalized.proves_numerical_contract());
    assert!(!finalized.proves_race_freedom());
    assert!(!finalized.proves_verus_verification());
    assert!(!finalized.grants_publication_authority());
    assert!(!finalized.grants_load_authority());
    assert!(!finalized.grants_launch_authority());
}

#[test]
fn row_softmax_structural_admission_accepts_arbitrary_text_without_semantic_authority() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    for (fill, invocation, semantic) in [(0x00, 0xa2, 0xb2), (0xff, 0xa3, 0xb3)] {
        let mut fixture = fixture_with_descriptor_table(row_softmax_options(), Some(&table));
        fixture.bytes[fixture.text_offset..fixture.text_offset + 64].fill(fill);
        let inspected = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                invocation,
                semantic,
                "row_softmax_v1",
                "row_softmax_v1.kd",
            ),
            row_softmax_expectation(),
        )
        .unwrap();

        assert!(!inspected.validates_kernel_body());
        assert!(!inspected.proves_functional_softmax());
        assert!(!inspected.proves_exp_implementation());
        assert!(!inspected.proves_numerical_contract());
        assert!(!inspected.grants_launch_authority());
    }
}

#[test]
fn row_softmax_rejects_workgroup_wave_grid_and_tiled_abi_substitution() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    let wrong_required = fixture_with_descriptor_table(
        FixtureOptions {
            required_workgroup_size: [256, 1, 1],
            max_flat_workgroup_size: 256,
            ..row_softmax_options()
        },
        Some(&table),
    );
    let error = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
        evidence_for(
            wrong_required.bytes,
            "gfx942:xnack-",
            0xa4,
            0xb4,
            "row_softmax_v1",
            "row_softmax_v1.kd",
        ),
        row_softmax_expectation(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RowSoftmaxV1StructuralArtifactErrorV1::RawInspection(
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1RequiredWorkgroupSizeMismatch {
                actual: Some([256, 1, 1]),
                expected: [64, 1, 1],
                ..
            }
        )
    ));

    for options in [
        FixtureOptions {
            wavefront_size: 32,
            descriptor_wavefront_size: 32,
            ..row_softmax_options()
        },
        FixtureOptions {
            max_workgroups: [Some(1), Some(1), Some(1)],
            ..row_softmax_options()
        },
        FixtureOptions {
            max_workgroups: [Some(2), Some(1), Some(1)],
            ..row_softmax_options()
        },
        FixtureOptions {
            kernarg_segment_size_override: Some(320),
            ..row_softmax_options()
        },
        FixtureOptions {
            row_softmax_first_argument_offset: 1,
            ..row_softmax_options()
        },
        FixtureOptions {
            group_segment_fixed_size: 256,
            ..row_softmax_options()
        },
    ] {
        let fixture = fixture_with_descriptor_table(options, Some(&table));
        assert!(
            inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
                evidence_for(
                    fixture.bytes,
                    "gfx942:xnack-",
                    0xa5,
                    0xb5,
                    "row_softmax_v1",
                    "row_softmax_v1.kd",
                ),
                row_softmax_expectation(),
            )
            .is_err()
        );
    }
}

#[test]
fn row_softmax_rejects_capability_target_symbol_and_descriptor_substitution() {
    for capabilities in [vec![], vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave]] {
        let table = row_softmax_descriptor_table(capabilities);
        let fixture = fixture_with_descriptor_table(row_softmax_options(), Some(&table));
        let error = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                0xa6,
                0xb6,
                "row_softmax_v1",
                "row_softmax_v1.kd",
            ),
            row_softmax_expectation(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                RowSoftmaxV1StructuralArtifactErrorV1::DescriptorPolicy(
                    RowSoftmaxV1StructuralDescriptorErrorV1::CapabilityProvenance
                )
            ),
            "unexpected capability-substitution error: {error:?}"
        );
    }

    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    for (options, target, entry, descriptor) in [
        (
            FixtureOptions {
                target: "gfx942",
                ..row_softmax_options()
            },
            "gfx942",
            "row_softmax_v1",
            "row_softmax_v1.kd",
        ),
        (
            FixtureOptions {
                entry: "tiled_gemm_v1",
                descriptor: "tiled_gemm_v1.kd",
                ..row_softmax_options()
            },
            "gfx942:xnack-",
            "tiled_gemm_v1",
            "tiled_gemm_v1.kd",
        ),
    ] {
        let fixture = fixture_with_descriptor_table(options, Some(&table));
        assert!(
            inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
                evidence_for(fixture.bytes, target, 0xa7, 0xb7, entry, descriptor,),
                row_softmax_expectation(),
            )
            .is_err()
        );
    }
}

#[test]
fn row_softmax_rejects_even_unit_cluster_dimensions_absent_from_llvm22_output() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    let fixture = fixture_with_descriptor_table(
        FixtureOptions {
            cluster_dims: Some([1, 1, 1]),
            ..row_softmax_options()
        },
        Some(&table),
    );
    assert!(
        inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                0xa9,
                0xb9,
                "row_softmax_v1",
                "row_softmax_v1.kd",
            ),
            row_softmax_expectation(),
        )
        .is_err()
    );
}

#[test]
fn row_softmax_rejects_lifecycle_cluster_stack_and_hidden_launch_substitutions() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    let substitutions = [
        (
            FixtureOptions {
                kernel_kind: Some("init"),
                ..row_softmax_options()
            },
            "kernel kind",
        ),
        (
            FixtureOptions {
                cluster_dims: Some([2, 1, 1]),
                ..row_softmax_options()
            },
            "cluster dimensions",
        ),
        (
            FixtureOptions {
                uses_dynamic_stack: Some(true),
                ..row_softmax_options()
            },
            "dynamic stack declaration",
        ),
        (
            FixtureOptions {
                include_dynamic_lds_size: true,
                ..row_softmax_options()
            },
            "hidden argument profile",
        ),
        (
            FixtureOptions {
                optional_hidden_argument: Some((88, 8, "hidden_multigrid_sync_arg")),
                ..row_softmax_options()
            },
            "hidden argument profile",
        ),
        (
            FixtureOptions {
                optional_hidden_argument: Some((200, 8, "hidden_queue_ptr")),
                ..row_softmax_options()
            },
            "hidden argument profile",
        ),
    ];

    for (index, (options, field)) in substitutions.into_iter().enumerate() {
        let fixture = fixture_with_descriptor_table(options, Some(&table));
        let error = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                0xaa_u8.wrapping_add(u8::try_from(index).unwrap()),
                0xba_u8.wrapping_add(u8::try_from(index).unwrap()),
                "row_softmax_v1",
                "row_softmax_v1.kd",
            ),
            row_softmax_expectation(),
        )
        .unwrap_err();
        if field != "hidden argument profile" {
            assert!(
                matches!(
                    &error,
                    RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(actual) if *actual == field
                ),
                "substitution {index} expected {field}, found {error:?}"
            );
        }
    }
}

fn assert_row_softmax_rejects_unmeasured_metadata(options: FixtureOptions<'static>, case: &str) {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    let fixture = fixture_with_descriptor_table(options, Some(&table));
    assert!(
        inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                0xd0,
                0xe0,
                "row_softmax_v1",
                "row_softmax_v1.kd",
            ),
            row_softmax_expectation(),
        )
        .is_err(),
        "unmeasured metadata case {case} was accepted",
    );
}

macro_rules! unmeasured_kernel_metadata_case {
    ($name:ident, $options:expr) => {
        #[test]
        fn $name() {
            assert_row_softmax_rejects_unmeasured_metadata($options, stringify!($name));
        }
    };
}

unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_kernel_kind,
    FixtureOptions {
        kernel_kind: Some("normal"),
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_dynamic_stack,
    FixtureOptions {
        uses_dynamic_stack: None,
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_uniform_workgroup,
    FixtureOptions {
        uniform_work_group_size: Some(0),
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_workgroup_processor_mode,
    FixtureOptions {
        workgroup_processor_mode: Some(false),
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_gfx1250_revision,
    FixtureOptions {
        gfx1250_revision: Some("B0"),
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_device_enqueue_symbol,
    FixtureOptions {
        device_enqueue_symbol: Some("queue_entry"),
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_workgroup_size_hint,
    FixtureOptions {
        include_workgroup_size_hint: true,
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_vector_type_hint,
    FixtureOptions {
        include_vector_type_hint: true,
        ..row_softmax_options()
    }
);
unmeasured_kernel_metadata_case!(
    row_softmax_rejects_unmeasured_printf_metadata,
    FixtureOptions {
        include_printf_metadata: true,
        ..row_softmax_options()
    }
);

macro_rules! unmeasured_argument_metadata_case {
    ($name:ident, $field:expr, $value:expr) => {
        #[test]
        fn $name() {
            for argument_index in [0, 1, 4] {
                assert_row_softmax_rejects_unmeasured_metadata(
                    FixtureOptions {
                        argument_extra: Some((argument_index, $field, $value)),
                        ..row_softmax_options()
                    },
                    stringify!($name),
                );
            }
        }
    };
}

unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_type_name,
    ".type_name",
    FixtureMetadataValue::String("float*")
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_align,
    ".align",
    FixtureMetadataValue::Unsigned(8)
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_value_type,
    ".value_type",
    FixtureMetadataValue::String("f32")
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_access,
    ".access",
    FixtureMetadataValue::String("read_only")
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_actual_access,
    ".actual_access",
    FixtureMetadataValue::String("read_only")
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_pointee_align,
    ".pointee_align",
    FixtureMetadataValue::Unsigned(4)
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_is_const,
    ".is_const",
    FixtureMetadataValue::Boolean(false)
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_is_restrict,
    ".is_restrict",
    FixtureMetadataValue::Boolean(false)
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_is_volatile,
    ".is_volatile",
    FixtureMetadataValue::Boolean(false)
);
unmeasured_argument_metadata_case!(
    row_softmax_rejects_unmeasured_argument_is_pipe,
    ".is_pipe",
    FixtureMetadataValue::Boolean(false)
);

macro_rules! unmeasured_single_argument_metadata_case {
    ($name:ident, $index:expr, $field:expr, $value:expr) => {
        #[test]
        fn $name() {
            assert_row_softmax_rejects_unmeasured_metadata(
                FixtureOptions {
                    argument_extra: Some(($index, $field, $value)),
                    ..row_softmax_options()
                },
                stringify!($name),
            );
        }
    };
}

unmeasured_single_argument_metadata_case!(
    row_softmax_rejects_unmeasured_value_address_space,
    1,
    ".address_space",
    FixtureMetadataValue::String("global")
);
unmeasured_single_argument_metadata_case!(
    row_softmax_rejects_unmeasured_hidden_name,
    4,
    ".name",
    FixtureMetadataValue::String("hidden")
);
unmeasured_single_argument_metadata_case!(
    row_softmax_rejects_unmeasured_hidden_address_space,
    4,
    ".address_space",
    FixtureMetadataValue::String("global")
);

#[test]
fn row_softmax_rejects_source_and_register_metadata_drift() {
    #[derive(Clone, Copy)]
    enum ExpectedFailure {
        ArtifactProfile(&'static str),
        DescriptorBinding(&'static str),
    }

    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    let substitutions = [
        (
            FixtureOptions {
                source_language: None,
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("source metadata"),
        ),
        (
            FixtureOptions {
                source_language: Some("HIP"),
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("source metadata"),
        ),
        (
            FixtureOptions {
                source_language_version: None,
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("source metadata"),
        ),
        (
            FixtureOptions {
                source_language_version: Some([2, 1]),
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("source metadata"),
        ),
        (
            FixtureOptions {
                sgpr_count: 41,
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("register metadata"),
        ),
        (
            FixtureOptions {
                vgpr_count: 87,
                ..row_softmax_options()
            },
            ExpectedFailure::DescriptorBinding(".vgpr_count"),
        ),
        (
            FixtureOptions {
                agpr_count: None,
                ..row_softmax_options()
            },
            ExpectedFailure::DescriptorBinding(".agpr_count"),
        ),
        (
            FixtureOptions {
                agpr_count: Some(43),
                ..row_softmax_options()
            },
            ExpectedFailure::DescriptorBinding(".vgpr_count"),
        ),
        (
            FixtureOptions {
                sgpr_spill_count: None,
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("register metadata"),
        ),
        (
            FixtureOptions {
                sgpr_spill_count: Some(43),
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("register metadata"),
        ),
        (
            FixtureOptions {
                vgpr_spill_count: None,
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("register metadata"),
        ),
        (
            FixtureOptions {
                vgpr_spill_count: Some(27),
                ..row_softmax_options()
            },
            ExpectedFailure::ArtifactProfile("register metadata"),
        ),
    ];

    for (index, (options, expected_failure)) in substitutions.into_iter().enumerate() {
        let fixture = fixture_with_descriptor_table(options, Some(&table));
        let error = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
            evidence_for(
                fixture.bytes,
                "gfx942:xnack-",
                0x20_u8.wrapping_add(u8::try_from(index).unwrap()),
                0x40_u8.wrapping_add(u8::try_from(index).unwrap()),
                "row_softmax_v1",
                "row_softmax_v1.kd",
            ),
            row_softmax_expectation(),
        )
        .unwrap_err();
        match expected_failure {
            ExpectedFailure::ArtifactProfile(expected_field) => assert!(
                matches!(
                    &error,
                    RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(actual)
                        if *actual == expected_field
                ),
                "substitution {index} expected artifact field {expected_field}, found {error:?}",
            ),
            ExpectedFailure::DescriptorBinding(expected_field) => assert!(
                matches!(
                    &error,
                    RowSoftmaxV1StructuralArtifactErrorV1::RawInspection(
                        WorkerV2RawHsacoInspectionError::HsacoBinding(
                            fe2o3_hsaco::KernelBindingError::MetadataMismatch(actual)
                        )
                    ) if *actual == expected_field
                ),
                "substitution {index} expected binding field {expected_field}, found {error:?}",
            ),
        }
    }
}

macro_rules! hidden_argument_omission_case {
    ($name:ident, $index:expr) => {
        #[test]
        fn $name() {
            let table = row_softmax_descriptor_table(row_softmax_capabilities());
            let fixture = fixture_with_descriptor_table(
                FixtureOptions {
                    omitted_hidden_argument: Some($index),
                    ..row_softmax_options()
                },
                Some(&table),
            );
            assert!(
                inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
                    evidence_for(
                        fixture.bytes,
                        "gfx942:xnack-",
                        0xc1,
                        0xd1,
                        "row_softmax_v1",
                        "row_softmax_v1.kd",
                    ),
                    row_softmax_expectation(),
                )
                .is_err(),
                "hidden argument {} accepted an omission",
                $index,
            );
        }
    };
}

hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_block_count_x, 0);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_block_count_y, 1);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_block_count_z, 2);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_group_size_x, 3);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_group_size_y, 4);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_group_size_z, 5);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_remainder_x, 6);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_remainder_y, 7);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_remainder_z, 8);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_global_offset_x, 9);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_global_offset_y, 10);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_global_offset_z, 11);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_grid_dims, 12);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_hostcall_buffer, 13);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_multigrid_sync_arg, 14);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_heap_v1, 15);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_default_queue, 16);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_completion_action, 17);
hidden_argument_omission_case!(row_softmax_rejects_omitted_hidden_queue_ptr, 18);

#[test]
fn row_softmax_rejects_hidden_argument_size_and_kind_drift() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    for override_value in [
        (10, 48, 9, "hidden_global_offset_y"),
        (10, 48, 8, "hidden_none"),
    ] {
        let fixture = fixture_with_descriptor_table(
            FixtureOptions {
                hidden_argument_override: Some(override_value),
                ..row_softmax_options()
            },
            Some(&table),
        );
        assert!(
            inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
                evidence_for(
                    fixture.bytes,
                    "gfx942:xnack-",
                    0xc1,
                    0xd1,
                    "row_softmax_v1",
                    "row_softmax_v1.kd",
                ),
                row_softmax_expectation(),
            )
            .is_err(),
            "hidden argument accepted size or kind drift",
        );
    }
}

fn tiled_options() -> FixtureOptions<'static> {
    FixtureOptions {
        target: "gfx942:xnack-",
        // ELF OSABI byte 4 encodes AMDGPU HSA code object V6.
        code_object_version: 4,
        entry: "tiled_gemm_v1",
        descriptor: "tiled_gemm_v1.kd",
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        include_explicit_argument_alignments: true,
        abi: FixtureAbi::TiledGemmV1,
        ..FixtureOptions::valid()
    }
}

fn tiled_capabilities() -> Vec<CapabilityV1> {
    vec![
        CapabilityV1::Subgroup,
        CapabilityV1::MatrixMultiply,
        CapabilityV1::AmdWave,
        CapabilityV1::AmdMfma,
    ]
}

fn tiled_expectation() -> TiledGemmV1StructuralDescriptorExpectationV1 {
    TiledGemmV1StructuralDescriptorExpectationV1::new(
        KernelId::from_bytes([0x71; 32]),
        build_evidence(0x72, 0x73),
        build_evidence(0x74, 0x75),
    )
    .unwrap()
}

fn tiled_descriptor_table(capabilities: Vec<CapabilityV1>) -> Vec<u8> {
    let u16_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16));
    let u16_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16));
    let f32_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let f32_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let output_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let output_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x71; 32]),
        name("tiled_gemm_v1"),
        name("tiled_gemm_v1"),
        name("tiled_gemm_v1.kd"),
        build_evidence(0x72, 0x73),
        build_evidence(0x74, 0x75),
        capabilities,
        KernelAbiLayoutV1::new(64, 320, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        vec![
            LogicalArgumentV1::shared_slice(0, name("a"), &u16_source, &u16_layout, 0).unwrap(),
            LogicalArgumentV1::shared_slice(1, name("b"), &u16_source, &u16_layout, 16).unwrap(),
            LogicalArgumentV1::shared_slice(2, name("c"), &f32_source, &f32_layout, 32).unwrap(),
            LogicalArgumentV1::disjoint_slice(
                3,
                name("d"),
                &output_source,
                &output_layout,
                AccessMode::ReadWrite,
                48,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x76; 20]),
        ProducerIdentityV1::new(text("rustc-codegen-fe2o3-worker-v2"), text("test")),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        vec![u16_source, f32_source, output_source],
        vec![u16_layout, f32_layout, output_layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn row_softmax_options() -> FixtureOptions<'static> {
    FixtureOptions {
        target: "gfx942:xnack-",
        // ELF OSABI byte 4 encodes AMDGPU HSA code object V6.
        code_object_version: 4,
        entry: "row_softmax_v1",
        descriptor: "row_softmax_v1.kd",
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        include_explicit_argument_alignments: false,
        include_exact_row_llvm22_hidden_arguments: true,
        max_workgroups: [None; 3],
        uses_dynamic_stack: Some(false),
        source_language: Some("OpenCL C"),
        source_language_version: Some([2, 0]),
        sgpr_count: 42,
        vgpr_count: 88,
        agpr_count: Some(44),
        sgpr_spill_count: Some(44),
        vgpr_spill_count: Some(28),
        abi: FixtureAbi::RowSoftmaxV1,
        ..FixtureOptions::valid()
    }
}

fn row_softmax_capabilities() -> Vec<CapabilityV1> {
    vec![CapabilityV1::AmdWave]
}

fn row_softmax_expectation() -> RowSoftmaxV1StructuralDescriptorExpectationV1 {
    RowSoftmaxV1StructuralDescriptorExpectationV1::new(
        KernelId::from_bytes([0x81; 32]),
        build_evidence(0x82, 0x83),
        build_evidence(0x84, 0x85),
    )
    .unwrap()
}

fn row_softmax_descriptor_table(capabilities: Vec<CapabilityV1>) -> Vec<u8> {
    let input_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let input_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let output_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let output_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x81; 32]),
        name("row_softmax_v1"),
        name("row_softmax_v1"),
        name("row_softmax_v1.kd"),
        build_evidence(0x82, 0x83),
        build_evidence(0x84, 0x85),
        capabilities,
        KernelAbiLayoutV1::new(32, 288, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(1, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        vec![
            LogicalArgumentV1::shared_slice(0, name("input"), &input_source, &input_layout, 0)
                .unwrap(),
            LogicalArgumentV1::disjoint_slice(
                1,
                name("output"),
                &output_source,
                &output_layout,
                AccessMode::ReadWrite,
                16,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x86; 20]),
        ProducerIdentityV1::new(text("rustc-codegen-fe2o3-worker-v2"), text("test")),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        vec![input_source, output_source],
        vec![input_layout, output_layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn prepare(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
) -> fe2o3_hsaco_finalize::PreparedFinalizedWorkerV2HsacoV1 {
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(bytes, target, invocation_seed, semantic_seed))
            .unwrap();
    finalize_inspected_worker_v2_hsaco_v1(raw).unwrap()
}

fn descriptor_table(target: &str) -> Vec<u8> {
    descriptor_table_with_launch(target, 272, 256)
}

fn descriptor_table_with_launch(
    target: &str,
    kernarg_segment_size: u32,
    max_flat_workgroup_size: u32,
) -> Vec<u8> {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x61; 32]),
        name("vecadd"),
        name("vecadd"),
        name("vecadd.kd"),
        build_evidence(0x62, 0x63),
        build_evidence(0x64, 0x65),
        Vec::new(),
        KernelAbiLayoutV1::new(16, kernarg_segment_size, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            max_flat_workgroup_size,
            0,
            64 * 1024,
        )
        .unwrap(),
        vec![LogicalArgumentV1::shared_slice(0, name("values"), &source, &layout, 0).unwrap()],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc"), text("unauthenticated-test"), [0x66; 20]),
        ProducerIdentityV1::new(text("fe2o3-test"), text("unauthenticated-test")),
        DeviceTargetV1::parse(target).unwrap(),
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn coordinated_semantic_substitution_table(target: &str) -> DeviceDescriptorTableV1 {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0xa1; 32]),
        name("coordinated_vecadd"),
        name("vecadd"),
        name("vecadd.kd"),
        build_evidence(0xa2, 0xa3),
        build_evidence(0xa4, 0xa5),
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(4, 272, 8).unwrap(),
        LaunchConstraintsV1::new(
            2,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 7, 1).unwrap(),
            256,
            0,
            1_024,
        )
        .unwrap(),
        vec![LogicalArgumentV1::scalar(0, name("element"), &source, &layout, 0).unwrap()],
    )
    .unwrap();
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("alternate-rustc"), text("substituted"), [0xa6; 20]),
        ProducerIdentityV1::new(text("alternate-producer"), text("substituted")),
        DeviceTargetV1::parse(target).unwrap(),
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap()
}

fn cov6_explicit_only_descriptor_table(explicit_size: u32, total_size: u32) -> Vec<u8> {
    let scalar_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0xb1; 32]),
        name("vecadd"),
        name("vecadd"),
        name("vecadd.kd"),
        build_evidence(0xb2, 0xb3),
        build_evidence(0xb4, 0xb5),
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(explicit_size, total_size, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            256,
            0,
            64 * 1024,
        )
        .unwrap(),
        vec![
            LogicalArgumentV1::scalar(0, name("scale"), &scalar_source, &scalar_layout, 0).unwrap(),
            LogicalArgumentV1::shared_slice(1, name("input"), &shared_source, &shared_layout, 8)
                .unwrap(),
            LogicalArgumentV1::disjoint_slice(
                2,
                name("output"),
                &disjoint_source,
                &disjoint_layout,
                AccessMode::ReadWrite,
                24,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0xb6; 20]),
        ProducerIdentityV1::new(
            text("rustc-codegen-fe2o3-worker-v2"),
            text("typed-general-gfx942-cov6-v1"),
        ),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        vec![scalar_source, shared_source, disjoint_source],
        vec![scalar_layout, shared_layout, disjoint_layout],
        vec![kernel],
    )
    .unwrap();
    encode_device_descriptor_table_v1(&table).unwrap()
}

fn replace_fixture_metadata_with_explicit_only_cov6(
    bytes: &mut [u8],
    options: FixtureOptions<'_>,
    explicit_size: u32,
) {
    let owner_offset = bytes
        .windows(b"AMDGPU\0".len())
        .position(|window| window == b"AMDGPU\0")
        .unwrap();
    let note_offset = owner_offset - 12;
    let metadata_len =
        u32::from_le_bytes(bytes[note_offset + 4..note_offset + 8].try_into().unwrap()) as usize;
    let metadata_offset = owner_offset + 8;
    let metadata = (0..=metadata_len)
        .find_map(|padding| {
            let candidate = explicit_only_cov6_metadata(options, explicit_size, padding);
            (candidate.len() == metadata_len).then_some(candidate)
        })
        .expect("a language field can pad explicit-only metadata to the original note size");
    bytes[metadata_offset..metadata_offset + metadata_len].copy_from_slice(&metadata);
}

fn explicit_only_cov6_metadata(
    options: FixtureOptions<'_>,
    explicit_size: u32,
    language_padding: usize,
) -> Vec<u8> {
    use msgpack::Value;

    let arguments = vec![
        explicit_argument(Some("scale"), 0, 4, None, "by_value", None),
        explicit_pointer_argument(
            Some("input_ptr"),
            8,
            8,
            None,
            "global_buffer",
            Some("global"),
            None,
        ),
        explicit_argument(Some("input_len"), 16, 8, None, "by_value", None),
        explicit_pointer_argument(
            Some("output_ptr"),
            24,
            8,
            None,
            "global_buffer",
            Some("global"),
            None,
        ),
        explicit_argument(Some("output_len"), 32, 8, None, "by_value", None),
    ];
    let kernel = Value::Map(vec![
        (Value::from(".name"), Value::from(options.entry)),
        (Value::from(".symbol"), Value::from(options.descriptor)),
        (Value::from(".args"), Value::Array(arguments)),
        (
            Value::from(".kernarg_segment_size"),
            Value::from(explicit_size),
        ),
        (Value::from(".kernarg_segment_align"), Value::from(8)),
        (Value::from(".group_segment_fixed_size"), Value::from(0)),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (Value::from(".wavefront_size"), Value::from(64)),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(11)),
        (Value::from(".agpr_count"), Value::from(3)),
        (Value::from(".sgpr_spill_count"), Value::from(2)),
        (Value::from(".vgpr_spill_count"), Value::from(4)),
        (Value::from(".max_flat_workgroup_size"), Value::from(256)),
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(vec![Value::from(256), Value::from(1), Value::from(1)]),
        ),
        (
            Value::from(".language"),
            Value::from("R".repeat(language_padding)),
        ),
    ]);
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from(format!("amdgcn-amd-amdhsa--{}", options.target)),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(vec![kernel])),
    ]);
    let mut encoded = Vec::new();
    msgpack::write_value(&mut encoded, &root).unwrap();
    encoded
}

fn two_kernel_descriptor_table(
    digest: CanonicalCodeObjectDigest,
    substitute_zeta: bool,
) -> DeviceDescriptorTableV1 {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let kernels = [("alpha", "alpha.kd", 0xc1), ("zeta", "zeta.kd", 0xd1)]
        .into_iter()
        .map(|(entry, descriptor, seed)| {
            let substituted = substitute_zeta && entry == "zeta";
            KernelDescriptorV1::new(
                KernelId::from_bytes([if substituted { 0xe1 } else { seed }; 32]),
                name(if substituted {
                    "zeta_substituted"
                } else {
                    entry
                }),
                name(entry),
                name(descriptor),
                build_evidence(
                    if substituted {
                        0xe2
                    } else {
                        seed.wrapping_add(1)
                    },
                    if substituted {
                        0xe3
                    } else {
                        seed.wrapping_add(2)
                    },
                ),
                build_evidence(
                    if substituted {
                        0xe4
                    } else {
                        seed.wrapping_add(3)
                    },
                    if substituted {
                        0xe5
                    } else {
                        seed.wrapping_add(4)
                    },
                ),
                if substituted {
                    vec![CapabilityV1::AmdWave]
                } else {
                    Vec::new()
                },
                KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
                LaunchConstraintsV1::new(
                    1,
                    BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
                    DimensionsV1::new(if substituted { 17 } else { u32::MAX }, 1, 1).unwrap(),
                    256,
                    0,
                    if substituted { 2_048 } else { 64 * 1024 },
                )
                .unwrap(),
                vec![
                    LogicalArgumentV1::shared_slice(
                        0,
                        name(if substituted {
                            "substituted_values"
                        } else {
                            "values"
                        }),
                        &source,
                        &layout,
                        0,
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        })
        .collect();
    DeviceDescriptorTableV1::new(
        digest,
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc"), text("two-kernel-test"), [0xc6; 20]),
        ProducerIdentityV1::new(text("fe2o3-test"), text("two-kernel-test")),
        DeviceTargetV1::parse(GFX942_TARGET).unwrap(),
        vec![source],
        vec![layout],
        kernels,
    )
    .unwrap()
}

fn two_kernel_metadata(kernels: &[(&str, &str)]) -> Vec<u8> {
    use msgpack::Value;

    let kernels = kernels
        .iter()
        .map(|(entry, descriptor)| {
            let mut arguments = vec![
                argument(Some("values_ptr"), 0, 8, "global_buffer", Some("global")),
                argument(Some("values_len"), 8, 8, "by_value", None),
            ];
            arguments.extend(v5_hidden_arguments(16));
            Value::Map(vec![
                (Value::from(".name"), Value::from(*entry)),
                (Value::from(".symbol"), Value::from(*descriptor)),
                (Value::from(".args"), Value::Array(arguments)),
                (Value::from(".kernarg_segment_size"), Value::from(272)),
                (Value::from(".kernarg_segment_align"), Value::from(8)),
                (Value::from(".group_segment_fixed_size"), Value::from(0)),
                (Value::from(".private_segment_fixed_size"), Value::from(0)),
                (Value::from(".wavefront_size"), Value::from(64)),
                (Value::from(".sgpr_count"), Value::from(14)),
                (Value::from(".vgpr_count"), Value::from(11)),
                (Value::from(".agpr_count"), Value::from(3)),
                (Value::from(".sgpr_spill_count"), Value::from(2)),
                (Value::from(".vgpr_spill_count"), Value::from(4)),
                (Value::from(".max_flat_workgroup_size"), Value::from(256)),
                (
                    Value::from(".reqd_workgroup_size"),
                    Value::Array(vec![Value::from(256), Value::from(1), Value::from(1)]),
                ),
            ])
        })
        .collect();
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from("amdgcn-amd-amdhsa--gfx942:xnack-"),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(kernels)),
    ]);
    let mut encoded = Vec::new();
    msgpack::write_value(&mut encoded, &root).unwrap();
    encoded
}

fn two_kernel_fixture(table: &[u8], metadata: &[u8], kernels: &[(&str, &str)]) -> Fixture {
    const PROGRAM_COUNT: usize = 3;
    const SHSTRTAB_INDEX: usize = 7;

    let note = metadata_note(metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let descriptor_offsets = kernels
        .iter()
        .map(|_| {
            align(&mut bytes, 64);
            let offset = bytes.len();
            bytes.resize(offset + 64, 0);
            offset
        })
        .collect::<Vec<_>>();
    let rodata_end = bytes.len();

    let entry_offsets = kernels
        .iter()
        .map(|_| {
            align(&mut bytes, 256);
            let offset = bytes.len();
            bytes.resize(offset + 64, 0xbf);
            offset
        })
        .collect::<Vec<_>>();
    align(&mut bytes, 256);
    let export_offset = bytes.len();
    bytes.resize(export_offset + 64, 0xbe);
    let text_offset = entry_offsets[0];
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let symbol_names = kernels
        .iter()
        .map(|(entry, descriptor)| {
            (
                push_name(&mut strtab, entry),
                push_name(&mut strtab, descriptor),
            )
        })
        .collect::<Vec<_>>();
    let export_name = push_name(&mut strtab, "ffi_export");
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);
    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 2 + kernels.len() * 2;
    bytes.resize(symtab_offset + symbol_count * 24, 0);

    for (index, ((entry_name, descriptor_name), descriptor_offset)) in
        symbol_names.iter().zip(&descriptor_offsets).enumerate()
    {
        let entry_symbol = symtab_offset + (1 + index * 2) * 24;
        write_u32(&mut bytes, entry_symbol, *entry_name);
        bytes[entry_symbol + 4] = 0x12;
        bytes[entry_symbol + 5] = 3;
        write_u16(&mut bytes, entry_symbol + 6, TEXT_SECTION_INDEX as u16);
        let entry_address = (entry_offsets[index] + 0x1000) as u64;
        write_u64(&mut bytes, entry_symbol + 8, entry_address);
        write_u64(&mut bytes, entry_symbol + 16, 64);

        let descriptor_symbol = symtab_offset + (2 + index * 2) * 24;
        write_u32(&mut bytes, descriptor_symbol, *descriptor_name);
        bytes[descriptor_symbol + 4] = 0x11;
        write_u16(
            &mut bytes,
            descriptor_symbol + 6,
            RODATA_SECTION_INDEX as u16,
        );
        write_u64(&mut bytes, descriptor_symbol + 8, *descriptor_offset as u64);
        write_u64(&mut bytes, descriptor_symbol + 16, 64);

        write_u32(&mut bytes, *descriptor_offset + 8, 272);
        write_i64(
            &mut bytes,
            *descriptor_offset + 16,
            i64::try_from(entry_address - *descriptor_offset as u64).unwrap(),
        );
        write_u32(&mut bytes, *descriptor_offset + 44, 1);
        write_u32(&mut bytes, *descriptor_offset + 48, 0x00af_0081);
        write_u32(&mut bytes, *descriptor_offset + 52, 0x1390);
        write_u16(&mut bytes, *descriptor_offset + 56, 0x001e);
    }
    let export_symbol = symtab_offset + (symbol_count - 1) * 24;
    write_u32(&mut bytes, export_symbol, export_name);
    bytes[export_symbol + 4] = 0x12;
    bytes[export_symbol + 5] = 3;
    write_u16(&mut bytes, export_symbol + 6, TEXT_SECTION_INDEX as u16);
    write_u64(
        &mut bytes,
        export_symbol + 8,
        (export_offset + 0x1000) as u64,
    );
    write_u64(&mut bytes, export_symbol + 16, 64);

    align(
        &mut bytes,
        usize::try_from(DEVICE_DESCRIPTOR_SECTION_ALIGNMENT).unwrap(),
    );
    let table_offset = bytes.len();
    bytes.extend_from_slice(table);

    let mut shstr = vec![0];
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let descriptor_name = push_name(&mut shstr, DEVICE_DESCRIPTOR_SECTION_NAME);
    let shstr_name = push_name(&mut shstr, ".shstrtab");
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    bytes.resize(section_table_offset + 8 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, target_flags(GFX942_TARGET));
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, 8);
    write_u16(&mut bytes, 62, SHSTRTAB_INDEX as u16);

    let read_program = ELF_HEADER_BYTES;
    write_u32(&mut bytes, read_program, 1);
    write_u32(&mut bytes, read_program + 4, 4);
    write_u64(&mut bytes, read_program + 32, rodata_end as u64);
    write_u64(&mut bytes, read_program + 40, rodata_end as u64);
    write_u64(&mut bytes, read_program + 48, 0x1000);
    let execute_program = read_program + PROGRAM_HEADER_BYTES;
    write_u32(&mut bytes, execute_program, 1);
    write_u32(&mut bytes, execute_program + 4, 5);
    write_u64(&mut bytes, execute_program + 8, text_offset as u64);
    write_u64(
        &mut bytes,
        execute_program + 16,
        (text_offset + 0x1000) as u64,
    );
    write_u64(
        &mut bytes,
        execute_program + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(
        &mut bytes,
        execute_program + 40,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, execute_program + 48, 0x1000);

    write_test_section_header(
        &mut bytes,
        section_table_offset,
        NOTE_SECTION_INDEX,
        note_name,
        7,
        2,
        0,
        note_offset,
        note.len(),
        0,
        0,
        4,
        0,
    );
    write_test_section_header(
        &mut bytes,
        section_table_offset,
        RODATA_SECTION_INDEX,
        rodata_name,
        1,
        2,
        rodata_offset,
        rodata_offset,
        rodata_end - rodata_offset,
        0,
        0,
        64,
        0,
    );
    write_test_section_header(
        &mut bytes,
        section_table_offset,
        TEXT_SECTION_INDEX,
        text_name,
        1,
        6,
        text_offset + 0x1000,
        text_offset,
        text_end - text_offset,
        0,
        0,
        256,
        0,
    );
    write_test_section_header(
        &mut bytes,
        section_table_offset,
        STRTAB_SECTION_INDEX,
        strtab_name,
        3,
        0,
        0,
        strtab_offset,
        strtab.len(),
        0,
        0,
        1,
        0,
    );
    write_test_section_header(
        &mut bytes,
        section_table_offset,
        SYMTAB_SECTION_INDEX,
        symtab_name,
        2,
        0,
        0,
        symtab_offset,
        symbol_count * 24,
        STRTAB_SECTION_INDEX,
        1,
        8,
        24,
    );
    write_test_section_header(
        &mut bytes,
        section_table_offset,
        CANONICAL_DESCRIPTOR_SECTION_INDEX,
        descriptor_name,
        1,
        0,
        0,
        table_offset,
        table.len(),
        0,
        0,
        usize::try_from(DEVICE_DESCRIPTOR_SECTION_ALIGNMENT).unwrap(),
        0,
    );
    write_test_section_header(
        &mut bytes,
        section_table_offset,
        SHSTRTAB_INDEX,
        shstr_name,
        3,
        0,
        0,
        shstr_offset,
        shstr.len(),
        0,
        0,
        1,
        0,
    );

    Fixture {
        bytes,
        text_offset,
        descriptor_offset: descriptor_offsets[0],
    }
}

#[allow(clippy::too_many_arguments)]
fn write_test_section_header(
    bytes: &mut [u8],
    section_table_offset: usize,
    index: usize,
    name: u32,
    section_type: u32,
    flags: u64,
    address: usize,
    offset: usize,
    size: usize,
    link: usize,
    info: usize,
    alignment: usize,
    entry_size: usize,
) {
    let header = section_table_offset + index * SECTION_HEADER_BYTES;
    write_u32(bytes, header, name);
    write_u32(bytes, header + 4, section_type);
    write_u64(bytes, header + 8, flags);
    write_u64(bytes, header + 16, address as u64);
    write_u64(bytes, header + 24, offset as u64);
    write_u64(bytes, header + 32, size as u64);
    write_u32(bytes, header + 40, link as u32);
    write_u32(bytes, header + 44, info as u32);
    write_u64(bytes, header + 48, alignment as u64);
    write_u64(bytes, header + 56, entry_size as u64);
}

fn lineage_closure_offset(bytes: &[u8]) -> usize {
    let mut cursor = 108;
    let attempt_len = usize::from(u16::from_le_bytes(
        bytes[cursor..cursor + 2].try_into().unwrap(),
    ));
    cursor += 2 + attempt_len;
    cursor + 1 + 32
}

fn lineage_first_segment_offset(bytes: &[u8]) -> usize {
    let mut cursor = lineage_closure_offset(bytes) + 226;
    let target_len = usize::from(u16::from_le_bytes(
        bytes[cursor..cursor + 2].try_into().unwrap(),
    ));
    cursor += 2 + target_len + 1 + 20 + 40;
    for _ in 0..2 {
        let text_len = usize::from(u16::from_le_bytes(
            bytes[cursor..cursor + 2].try_into().unwrap(),
        ));
        cursor += 2 + text_len;
    }
    cursor
}

fn lineage_segment(bytes: &[u8], index: usize) -> &[u8] {
    &bytes[lineage_segment_range(bytes, index)]
}

fn lineage_segment_range(bytes: &[u8], index: usize) -> std::ops::Range<usize> {
    let mut cursor = lineage_first_segment_offset(bytes);
    for current in 0..=index {
        let length = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let start = cursor + 4;
        let end = start + length;
        assert!(end <= bytes.len() - 32);
        if current == index {
            return start..end;
        }
        cursor = end;
    }
    unreachable!()
}

fn replace_lineage_segment(bytes: &mut Vec<u8>, index: usize, replacement: &[u8]) {
    let range = lineage_segment_range(bytes, index);
    let length_offset = range.start - 4;
    bytes.splice(range, replacement.iter().copied());
    bytes[length_offset..length_offset + 4]
        .copy_from_slice(&(replacement.len() as u32).to_le_bytes());
    reseal_lineage_wire(bytes);
}

fn worker_response_without_output(response: &[u8]) -> Vec<u8> {
    assert_eq!(&response[..8], b"F3LRSP02");
    let mut rewritten = response[..8].to_vec();
    let mut cursor = 8;
    for expected_tag in 1_u16..=7 {
        let tag = u16::from_le_bytes(response[cursor..cursor + 2].try_into().unwrap());
        let length =
            u32::from_le_bytes(response[cursor + 2..cursor + 6].try_into().unwrap()) as usize;
        assert_eq!(tag, expected_tag);
        let field = &response[cursor + 6..cursor + 6 + length];
        let replacement = match tag {
            5 => &[8][..],
            7 => &[0][..],
            _ => field,
        };
        rewritten.extend_from_slice(&tag.to_le_bytes());
        rewritten.extend_from_slice(&(replacement.len() as u32).to_le_bytes());
        rewritten.extend_from_slice(replacement);
        cursor += 6 + length;
    }
    assert_eq!(cursor, response.len());
    rewritten
}

fn reseal_lineage_wire(bytes: &mut [u8]) {
    const DOMAIN: &[u8] = b"FE2O3/FINALIZER/PROTECTED-WORKER-V2-LINEAGE-CHECKSUM/V2\0";
    let body_len = bytes.len() - 32;
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update(&bytes[..body_len]);
    bytes[body_len..].copy_from_slice(&hasher.finalize());
}

fn name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn build_evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v2-hsaco-finalization-{}-{}",
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

fn evidence(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    evidence_for(
        bytes,
        target,
        invocation_seed,
        semantic_seed,
        "vecadd",
        "vecadd.kd",
    )
}

fn evidence_for(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
    entry: &str,
    descriptor: &str,
) -> fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1 {
    let directory = TestDirectory::new();
    let producer = ProducerIdentity::from_codegen(
        "worker_v2_hsaco_finalization_fixture",
        Some(Path::new("tests/worker_v2_hsaco_finalization.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([invocation_seed; 32]),
        BuildSession::from_bytes([invocation_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let handoff = compiler_handoff(&bytes, target, semantic_seed, entry, descriptor);
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
    execute_reproducible_first_build_worker_v2(
        consumed,
        &pinned_worker(),
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(64 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn protected_evidence(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
    closure: CompilerClosureV2,
    slot: CompilerModuleHandoffSlotV2,
) -> fe2o3_hsaco_finalize::InertProtectedFirstBuildWorkerV2EvidenceV1 {
    protected_evidence_with_module_prefix(
        bytes,
        target,
        invocation_seed,
        semantic_seed,
        closure,
        slot,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn protected_evidence_with_module_prefix(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
    closure: CompilerClosureV2,
    slot: CompilerModuleHandoffSlotV2,
    module_prefix: &[u8],
) -> fe2o3_hsaco_finalize::InertProtectedFirstBuildWorkerV2EvidenceV1 {
    protected_evidence_for_kernels_with_module_prefix(
        bytes,
        target,
        invocation_seed,
        semantic_seed,
        closure,
        slot,
        &[("vecadd", "vecadd.kd")],
        module_prefix,
    )
}

#[allow(clippy::too_many_arguments)]
fn protected_evidence_for_kernels(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
    closure: CompilerClosureV2,
    slot: CompilerModuleHandoffSlotV2,
    kernels: &[(&str, &str)],
) -> fe2o3_hsaco_finalize::InertProtectedFirstBuildWorkerV2EvidenceV1 {
    protected_evidence_for_kernels_with_module_prefix(
        bytes,
        target,
        invocation_seed,
        semantic_seed,
        closure,
        slot,
        kernels,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn protected_evidence_for_kernels_with_module_prefix(
    bytes: Vec<u8>,
    target: &str,
    invocation_seed: u8,
    semantic_seed: u8,
    closure: CompilerClosureV2,
    slot: CompilerModuleHandoffSlotV2,
    kernels: &[(&str, &str)],
    module_prefix: &[u8],
) -> fe2o3_hsaco_finalize::InertProtectedFirstBuildWorkerV2EvidenceV1 {
    let directory = TestDirectory::new();
    let producer = ProducerIdentity::from_codegen(
        "protected_worker_v2_hsaco_finalization_fixture",
        Some(Path::new("tests/worker_v2_hsaco_finalization.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([invocation_seed; 32]),
        BuildSession::from_bytes([invocation_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let handoff = compiler_handoff_for_kernels_with_module_prefix(
        &bytes,
        target,
        semantic_seed,
        kernels,
        module_prefix,
    );
    publish_compiler_module_handoff_in_slot_v2(
        &directory.0,
        &producer,
        attempt,
        slot,
        closure,
        handoff.canonical_bytes(),
    )
    .unwrap();
    let consumed =
        consume_compiler_module_handoff_in_slot_v2(&directory.0, &producer, attempt, slot, closure)
            .unwrap();
    execute_protected_reproducible_first_build_worker_v2(
        consumed,
        &pinned_worker(),
        Vec::new(),
        link_options(),
        WorkerOutputConstraintsV1::new(64 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(
            Duration::from_secs(if module_prefix.is_empty() { 2 } else { 10 }),
            16 * 1024,
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap()
}

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
    )
    .unwrap()
}

fn compiler_closure_with_mutated_role(seed: u8, role: usize) -> CompilerClosureV2 {
    let mut pins = [
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
    ];
    pins[role][0] ^= 0xff;
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
}

fn pinned_worker() -> PinnedWorkerV1 {
    let path = Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-v2-hsaco-fixture"));
    let executable = fs::read(path).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&executable),
        "fixture-worker-v2-hsaco-v1",
        "fixture-llvm-v1",
    )
    .unwrap();
    PinnedWorkerV1::open(path, measurement).unwrap()
}

fn link_options() -> Vec<LinkOptionV1> {
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

fn compiler_handoff(
    bytes: &[u8],
    target: &str,
    semantic_seed: u8,
    entry: &str,
    descriptor: &str,
) -> CompilerModuleHandoffV2 {
    compiler_handoff_with_module_prefix(bytes, target, semantic_seed, entry, descriptor, &[])
}

fn compiler_handoff_with_module_prefix(
    bytes: &[u8],
    target: &str,
    semantic_seed: u8,
    entry: &str,
    descriptor: &str,
    module_prefix: &[u8],
) -> CompilerModuleHandoffV2 {
    compiler_handoff_for_kernels_with_module_prefix(
        bytes,
        target,
        semantic_seed,
        &[(entry, descriptor)],
        module_prefix,
    )
}

fn compiler_handoff_for_kernels_with_module_prefix(
    bytes: &[u8],
    target: &str,
    semantic_seed: u8,
    kernels: &[(&str, &str)],
    module_prefix: &[u8],
) -> CompilerModuleHandoffV2 {
    const PAYLOAD_MARKER: &[u8] = b"FE2O3/TEST-HSACO-PAYLOAD/V1\0";
    let target = CompilerDeviceTargetV1::parse(target).unwrap();
    let mut symbols = kernels
        .iter()
        .map(|(entry, _)| (CompilerModuleSymbolRoleV1::KernelEntry, (*entry).to_owned()))
        .chain(kernels.iter().map(|(_, descriptor)| {
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                (*descriptor).to_owned(),
            )
        }))
        .chain(std::iter::once((
            CompilerModuleSymbolRoleV1::DeviceFfiExport,
            "ffi_export".to_owned(),
        )))
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    let manifest = CompilerModuleSymbolManifestV1::new(symbols).unwrap();
    let mut envelope =
        CompilerFfiEnvelopeBuilderV1::new(target, CompilerCodeObjectVersion::V6, 1).unwrap();
    envelope
        .push(compiler_contract(target, semantic_seed))
        .unwrap();
    let mut module = module_prefix.to_vec();
    module.extend_from_slice(PAYLOAD_MARKER);
    module.extend_from_slice(bytes);
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmBitcode,
        target,
        CompilerCodeObjectVersion::V6,
        envelope.finish().unwrap(),
        manifest,
        &module,
    )
    .unwrap()
}

fn compiler_contract(target: CompilerDeviceTargetV1, semantic_seed: u8) -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic_identity = [semantic_seed; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let target_text = target.to_string();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "ffi_export",
        calling_convention: "C",
        code_object_version: 6,
        target: &target_text,
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        target,
        CompilerCodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "finalization_fixture",
            "finalization_fixture::ffi_export",
            [0x67; 16],
            "_RINvNtCs1234_20finalization_fixture10ffi_export",
        )
        .unwrap(),
        "ffi_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}
