#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, ContentIdentityV1,
    DescriptorSourceEvidenceRequirementV1, FinalizationError, LinkOptionV1, PinnedWorkerV1,
    RowSoftmaxV1StructuralArtifactErrorV1, TiledGemmV1StructuralArtifactErrorV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerV2HsacoFinalizationError, WorkerV2RawHsacoInspectionError,
    execute_reproducible_first_build_worker_v2, finalize_inspected_worker_v2_hsaco_v1,
    finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
    finalize_tiled_gemm_v1_structural_worker_v2_hsaco_v1,
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
    TiledGemmV1StructuralDescriptorExpectationV1, ValidName, encode_device_descriptor_table_v1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};

include!("fixtures/worker_v2_hsaco_test_support.rs");

#[test]
fn canonical_fixture_controls_required_workgroup_metadata_presence() {
    let present = fixture(FixtureOptions::valid());
    assert_eq!(
        fe2o3_hsaco::inspect(&present.bytes).unwrap().kernels()[0].required_workgroup_size(),
        Some([256, 1, 1])
    );

    let mut options = FixtureOptions::valid();
    options.include_required_workgroup_size = false;
    let omitted = fixture(options);
    assert_eq!(
        fe2o3_hsaco::inspect(&omitted.bytes).unwrap().kernels()[0].required_workgroup_size(),
        None
    );
}

#[test]
fn missing_descriptor_source_returns_an_owning_fail_closed_blocker() {
    let fixture = fixture(FixtureOptions::valid());
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x41, 0x51)).unwrap();
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
    assert_eq!(blocker.target().to_string(), "gfx942");
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
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw_bytes = fixture.bytes.clone();
    let unfinalized = inspect_unfinalized(&raw_bytes).unwrap();
    let digest_offset = unfinalized.location().digest_offset();
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(raw_bytes.clone(), "gfx942", 0x42, 0x52)).unwrap();
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
    assert_eq!(prepared.target().to_string(), "gfx942");
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
    let mut table = descriptor_table("gfx942");
    table[16] = 1;
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x43, 0x53)).unwrap();
    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::ExpectedZeroDigest
        ))
    ));

    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let text_offset = fixture.text_offset;
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x44, 0x54)).unwrap();
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
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x45, 0x55)).unwrap();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .unwrap()
        .exact_finalized_bytes()
        .to_vec();

    let raw = inspect_worker_v2_raw_hsaco_v1(evidence(finalized, "gfx942", 0x46, 0x56)).unwrap();
    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::ExpectedZeroDigest
        ))
    ));
}

#[test]
fn rejects_descriptor_target_mismatch_without_weakening_raw_target_policy() {
    let table = descriptor_table("gfx942:xnack-");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let raw =
        inspect_worker_v2_raw_hsaco_v1(evidence(fixture.bytes, "gfx942", 0x47, 0x57)).unwrap();

    assert!(matches!(
        finalize_inspected_worker_v2_hsaco_v1(raw),
        Err(WorkerV2HsacoFinalizationError::CanonicalFinalization(
            FinalizationError::DeviceTargetMismatch
        ))
    ));
}

#[test]
fn finalization_identity_binds_lineage_separately_from_finalized_content() {
    let table = descriptor_table("gfx942");
    let fixture = fixture_with_descriptor_table(FixtureOptions::valid(), Some(&table));
    let first = prepare(fixture.bytes.clone(), "gfx942", 0x48, 0x58);
    let other_lineage = prepare(fixture.bytes.clone(), "gfx942", 0x49, 0x59);

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
    let changed = prepare(changed, "gfx942", 0x4a, 0x5a);
    assert_ne!(first.raw_output_identity(), changed.raw_output_identity());
    assert_ne!(
        first.finalized_output_identity(),
        changed.finalized_output_identity()
    );
    assert_ne!(first.canonical_digest(), changed.canonical_digest());
    assert_ne!(first.identity(), changed.identity());
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
    assert_eq!(kernel.max_workgroups(), [Some(1), Some(1), Some(1)]);
    assert_eq!(kernel.cluster_dims(), None);
    assert_eq!(kernel.kind(), fe2o3_hsaco::KernelKind::Normal);
    assert!(!kernel.uses_dynamic_stack());
    assert_eq!(kernel.max_flat_workgroup_size(), 64);
    assert_eq!(kernel.group_segment_fixed_size(), 0);
    assert_eq!(kernel.kernarg_segment_size(), 288);
    assert_eq!(kernel.implicit_argument_offset(), Some(32));
    assert_eq!(kernel.implicit_argument_size(), 256);
    assert_eq!(kernel.explicit_arguments().len(), 4);
    assert_eq!(kernel.hidden_arguments().len(), 13);
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
            max_workgroups: [None; 3],
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
        assert!(matches!(
            inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
                evidence_for(
                    fixture.bytes,
                    "gfx942:xnack-",
                    0xa6,
                    0xb6,
                    "row_softmax_v1",
                    "row_softmax_v1.kd",
                ),
                row_softmax_expectation(),
            ),
            Err(RowSoftmaxV1StructuralArtifactErrorV1::DescriptorPolicy(
                RowSoftmaxV1StructuralDescriptorErrorV1::CapabilityProvenance
            ))
        ));
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
fn row_softmax_accepts_only_absent_or_exact_unit_cluster_dimensions() {
    let table = row_softmax_descriptor_table(row_softmax_capabilities());
    for (cluster_dims, invocation, semantic) in [(None, 0xa8, 0xb8), (Some([1, 1, 1]), 0xa9, 0xb9)]
    {
        let fixture = fixture_with_descriptor_table(
            FixtureOptions {
                cluster_dims,
                ..row_softmax_options()
            },
            Some(&table),
        );
        assert_eq!(
            fe2o3_hsaco::inspect(&fixture.bytes).unwrap().kernels()[0].cluster_dims(),
            cluster_dims
        );
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
        finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(inspected).unwrap();
    }
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
                uses_dynamic_stack: true,
                ..row_softmax_options()
            },
            "dynamic stack",
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
        assert!(
            matches!(
                &error,
                RowSoftmaxV1StructuralArtifactErrorV1::ArtifactProfile(actual) if *actual == field
            ),
            "substitution {index} expected {field}, found {error:?}"
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
        include_explicit_argument_alignments: true,
        max_workgroups: [Some(1), Some(1), Some(1)],
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
        KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            256,
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
    const PAYLOAD_MARKER: &[u8] = b"FE2O3/TEST-HSACO-PAYLOAD/V1\0";
    let target = CompilerDeviceTargetV1::parse(target).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, entry),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, descriptor),
        (CompilerModuleSymbolRoleV1::DeviceFfiExport, "ffi_export"),
    ])
    .unwrap();
    let mut envelope =
        CompilerFfiEnvelopeBuilderV1::new(target, CompilerCodeObjectVersion::V6, 1).unwrap();
    envelope
        .push(compiler_contract(target, semantic_seed))
        .unwrap();
    let mut module = PAYLOAD_MARKER.to_vec();
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
