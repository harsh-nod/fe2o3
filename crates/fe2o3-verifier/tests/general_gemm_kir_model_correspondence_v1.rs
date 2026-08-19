use fe2o3_kernel_ir::{
    GeneralGemmKirV1, GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotV1, GeneralGemmPropertyV1,
    GeneralGemmSemanticMutationV1, GeneralGemmVerificationStageV1,
    general_gemm_semantic_mutation_kir_v1,
};
use fe2o3_verifier::{
    GENERAL_GEMM_KIR_MODEL_PROPERTY_COUNT_V1, GeneralGemmEvidenceIdentityV1,
    GeneralGemmKirModelCorrespondenceClaimV1, GeneralGemmKirModelCorrespondenceErrorV1,
    GeneralGemmKirModelCorrespondenceFieldV1, GeneralGemmKirModelPropertyScopeV1,
    GeneralGemmProofPropertyV1, GeneralGemmProofRequestV1, GeneralGemmProofScheduleV1,
    check_general_gemm_kir_model_correspondence_v1,
    derive_general_gemm_kir_model_correspondence_claim_v1,
};

fn identity(seed: u8) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([seed; 32])
}

fn request(schedule: GeneralGemmProofScheduleV1, seed: u8) -> GeneralGemmProofRequestV1 {
    GeneralGemmProofRequestV1::checked(
        schedule,
        identity(seed),
        identity(seed + 1),
        identity(seed + 2),
        identity(seed + 3),
        identity(seed + 4),
        identity(seed + 5),
        identity(seed + 6),
        identity(seed + 7),
        identity(seed + 8),
        identity(seed + 9),
        identity(seed + 10),
    )
    .unwrap()
}

fn plan() -> GeneralGemmPlanFieldsV1 {
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [33, 35, 19],
        strides: [23, 41, 43],
        storage_elements: [755, 773, 1411],
        block_counts: [3, 3, 1],
        aql_grid_work_items: [192, 3, 1],
        reduction_phases: 2,
        alpha_bits: 1.25_f32.to_bits(),
        beta_bits: (-0.5_f32).to_bits(),
    })
    .unwrap()
}

fn canonical() -> GeneralGemmKirV1 {
    GeneralGemmKirV1::canonical(plan())
}

fn flip(identity: GeneralGemmEvidenceIdentityV1) -> GeneralGemmEvidenceIdentityV1 {
    let mut bytes = *identity.as_bytes();
    bytes[0] ^= 0x80;
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(bytes)
}

fn mismatch_field(
    result: Result<
        fe2o3_verifier::GeneralGemmKirModelCorrespondenceV1,
        GeneralGemmKirModelCorrespondenceErrorV1,
    >,
) -> GeneralGemmKirModelCorrespondenceFieldV1 {
    match result.unwrap_err() {
        GeneralGemmKirModelCorrespondenceErrorV1::FieldMismatch(field) => field,
        other => panic!("expected field mismatch, got {other:?}"),
    }
}

type ClaimMutation = fn(&mut GeneralGemmKirModelCorrespondenceClaimV1);

#[test]
fn both_schedules_bind_exact_kir_plan_properties_and_verus_inputs() {
    let kir = canonical();
    let reference_request = request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1, 1);
    let vector_request = request(
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
        32,
    );
    let reference_claim =
        derive_general_gemm_kir_model_correspondence_claim_v1(&kir, reference_request).unwrap();
    let vector_claim =
        derive_general_gemm_kir_model_correspondence_claim_v1(&kir, vector_request).unwrap();

    assert_eq!(reference_claim.dimensions, [33, 35, 19]);
    assert_eq!(reference_claim.strides, [23, 41, 43]);
    assert_eq!(reference_claim.storage_elements, [755, 773, 1411]);
    assert_eq!(reference_claim.block_counts, [3, 3, 1]);
    assert_eq!(reference_claim.aql_grid_work_items, [192, 3, 1]);
    assert_eq!(reference_claim.reduction_phases, 2);
    assert_eq!(reference_claim.tail_shape, [1, 3, 3]);
    assert_eq!(reference_claim.a_global_transfer_width, 1);
    assert!(!reference_claim.a_scalar_tail_fallback);
    assert_eq!(vector_claim.a_global_transfer_width, 4);
    assert!(vector_claim.a_scalar_tail_fallback);
    assert_eq!(reference_claim.model_identity, vector_claim.model_identity);
    assert_ne!(
        reference_claim.positive_source_identity,
        vector_claim.positive_source_identity
    );
    assert_ne!(
        reference_claim.theorem_set_identity,
        vector_claim.theorem_set_identity
    );
    assert_ne!(
        reference_claim.source_closure_identity,
        vector_claim.source_closure_identity
    );
    assert_eq!(
        reference_claim.properties.len(),
        GENERAL_GEMM_KIR_MODEL_PROPERTY_COUNT_V1
    );
    assert_eq!(
        reference_claim.properties[10].scope,
        GeneralGemmKirModelPropertyScopeV1::ExactRealModelOnly
    );
    assert_eq!(
        reference_claim.properties[11].scope,
        GeneralGemmKirModelPropertyScopeV1::MachineRefinementOpen
    );

    for (proof_request, claim) in [
        (reference_request, reference_claim),
        (vector_request, vector_claim),
    ] {
        let correspondence =
            check_general_gemm_kir_model_correspondence_v1(&kir, proof_request, claim).unwrap();
        assert_eq!(correspondence.claim(), claim);
        assert_eq!(correspondence.proof_request(), proof_request);
        assert!(!correspondence.can_enter_compiler_proof_gate());
        assert!(!correspondence.grants_artifact_or_runtime_authority());
        assert_ne!(correspondence.identity().as_bytes(), &[0; 32]);
    }
}

#[test]
fn every_top_level_claim_field_substitution_fails_closed() {
    use GeneralGemmKirModelCorrespondenceFieldV1 as Field;

    let kir = canonical();
    let proof_request = request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1, 1);
    let exact = derive_general_gemm_kir_model_correspondence_claim_v1(&kir, proof_request).unwrap();
    let cases: [(Field, ClaimMutation); 26] = [
        (Field::KirIdentity, |claim| {
            claim.kir_identity = flip(claim.kir_identity)
        }),
        (Field::Properties, |claim| {
            claim.properties[0].diagnostic_code ^= 1
        }),
        (Field::Dimensions, |claim| claim.dimensions[0] ^= 1),
        (Field::Strides, |claim| claim.strides[0] ^= 1),
        (Field::StorageElements, |claim| {
            claim.storage_elements[0] ^= 1
        }),
        (Field::BlockCounts, |claim| claim.block_counts[0] ^= 1),
        (Field::AqlGridWorkItems, |claim| {
            claim.aql_grid_work_items[0] ^= 1
        }),
        (Field::ReductionPhases, |claim| claim.reduction_phases ^= 1),
        (Field::AlphaBits, |claim| claim.alpha_bits ^= 1),
        (Field::BetaBits, |claim| claim.beta_bits ^= 1),
        (Field::TailShape, |claim| claim.tail_shape[0] ^= 1),
        (Field::RequiresDispatch, |claim| {
            claim.requires_dispatch = !claim.requires_dispatch
        }),
        (Field::Schedule, |claim| {
            claim.schedule = GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1;
        }),
        (Field::TileExtent, |claim| claim.tile_extent ^= 1),
        (Field::WaveLanes, |claim| claim.wave_lanes ^= 1),
        (Field::ComponentsPerLane, |claim| {
            claim.components_per_lane ^= 1
        }),
        (Field::LdsElementsPerOperand, |claim| {
            claim.lds_elements_per_operand ^= 1
        }),
        (Field::AGlobalTransferWidth, |claim| {
            claim.a_global_transfer_width ^= 1
        }),
        (Field::BGlobalTransferWidth, |claim| {
            claim.b_global_transfer_width ^= 1
        }),
        (Field::AScalarTailFallback, |claim| {
            claim.a_scalar_tail_fallback = !claim.a_scalar_tail_fallback;
        }),
        (Field::SingleBufferedLds, |claim| {
            claim.single_buffered_lds = !claim.single_buffered_lds;
        }),
        (Field::ProofRequestIdentity, |claim| {
            claim.proof_request_identity = flip(claim.proof_request_identity);
        }),
        (Field::ModelIdentity, |claim| {
            claim.model_identity = flip(claim.model_identity)
        }),
        (Field::PositiveSourceIdentity, |claim| {
            claim.positive_source_identity = flip(claim.positive_source_identity);
        }),
        (Field::TheoremSetIdentity, |claim| {
            claim.theorem_set_identity = flip(claim.theorem_set_identity);
        }),
        (Field::SourceClosureIdentity, |claim| {
            claim.source_closure_identity = flip(claim.source_closure_identity);
        }),
    ];

    for (expected_field, mutate) in cases {
        let mut hostile = exact;
        mutate(&mut hostile);
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                proof_request,
                hostile,
            )),
            expected_field
        );
    }
}

#[test]
fn every_property_mapping_subfield_substitution_fails_closed() {
    let kir = canonical();
    let proof_request = request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1, 1);
    let exact = derive_general_gemm_kir_model_correspondence_claim_v1(&kir, proof_request).unwrap();

    for index in 0..GENERAL_GEMM_KIR_MODEL_PROPERTY_COUNT_V1 {
        let mut hostile = exact;
        hostile.properties[index].kir_property = GeneralGemmPropertyV1::MemorySafe;
        if hostile.properties[index] == exact.properties[index] {
            hostile.properties[index].kir_property = GeneralGemmPropertyV1::BoundsSafe;
        }
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                proof_request,
                hostile,
            )),
            GeneralGemmKirModelCorrespondenceFieldV1::Properties
        );

        let mut hostile = exact;
        hostile.properties[index].proof_property = GeneralGemmProofPropertyV1::MemorySafe;
        if hostile.properties[index] == exact.properties[index] {
            hostile.properties[index].proof_property = GeneralGemmProofPropertyV1::BoundsSafe;
        }
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                proof_request,
                hostile,
            )),
            GeneralGemmKirModelCorrespondenceFieldV1::Properties
        );

        let mut hostile = exact;
        hostile.properties[index].verification_stage = GeneralGemmVerificationStageV1::Kernel;
        if hostile.properties[index] == exact.properties[index] {
            hostile.properties[index].verification_stage = GeneralGemmVerificationStageV1::Amdgcn;
        }
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                proof_request,
                hostile,
            )),
            GeneralGemmKirModelCorrespondenceFieldV1::Properties
        );

        let mut hostile = exact;
        hostile.properties[index].diagnostic_code ^= 1;
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                proof_request,
                hostile,
            )),
            GeneralGemmKirModelCorrespondenceFieldV1::Properties
        );

        let mut hostile = exact;
        hostile.properties[index].scope = GeneralGemmKirModelPropertyScopeV1::MachineRefinementOpen;
        if hostile.properties[index] == exact.properties[index] {
            hostile.properties[index].scope =
                GeneralGemmKirModelPropertyScopeV1::StructuralModelCorrespondence;
        }
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                proof_request,
                hostile,
            )),
            GeneralGemmKirModelCorrespondenceFieldV1::Properties
        );
    }
}

#[test]
fn stale_model_source_and_proof_request_substitutions_are_rejected() {
    let kir = canonical();
    let exact_request = request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1, 1);
    let exact = derive_general_gemm_kir_model_correspondence_claim_v1(&kir, exact_request).unwrap();

    for (expected, mutate) in [
        (
            GeneralGemmKirModelCorrespondenceFieldV1::ModelIdentity,
            (|claim: &mut GeneralGemmKirModelCorrespondenceClaimV1| {
                claim.model_identity = flip(claim.model_identity);
            }) as fn(&mut GeneralGemmKirModelCorrespondenceClaimV1),
        ),
        (
            GeneralGemmKirModelCorrespondenceFieldV1::PositiveSourceIdentity,
            |claim| claim.positive_source_identity = flip(claim.positive_source_identity),
        ),
        (
            GeneralGemmKirModelCorrespondenceFieldV1::TheoremSetIdentity,
            |claim| claim.theorem_set_identity = flip(claim.theorem_set_identity),
        ),
        (
            GeneralGemmKirModelCorrespondenceFieldV1::SourceClosureIdentity,
            |claim| claim.source_closure_identity = flip(claim.source_closure_identity),
        ),
    ] {
        let mut stale = exact;
        mutate(&mut stale);
        assert_eq!(
            mismatch_field(check_general_gemm_kir_model_correspondence_v1(
                &kir,
                exact_request,
                stale,
            )),
            expected
        );
    }

    assert_eq!(
        mismatch_field(check_general_gemm_kir_model_correspondence_v1(
            &kir,
            request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1, 64),
            exact,
        )),
        GeneralGemmKirModelCorrespondenceFieldV1::ProofRequestIdentity
    );
}

#[test]
fn noncanonical_kir_and_record_authority_fail_closed() {
    let hostile = general_gemm_semantic_mutation_kir_v1(
        plan(),
        GeneralGemmSemanticMutationV1::DivergentBarrier,
    );
    let proof_request = request(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1, 1);
    assert!(matches!(
        derive_general_gemm_kir_model_correspondence_claim_v1(&hostile, proof_request),
        Err(GeneralGemmKirModelCorrespondenceErrorV1::NonCanonicalKir)
    ));

    let kir = canonical();
    let claim = derive_general_gemm_kir_model_correspondence_claim_v1(&kir, proof_request).unwrap();
    let correspondence =
        check_general_gemm_kir_model_correspondence_v1(&kir, proof_request, claim).unwrap();
    assert!(!correspondence.can_enter_compiler_proof_gate());
    assert!(!correspondence.grants_artifact_or_runtime_authority());
    assert_eq!(
        correspondence.claim().properties[10].scope,
        GeneralGemmKirModelPropertyScopeV1::ExactRealModelOnly
    );
    assert_eq!(
        correspondence.claim().properties[11].scope,
        GeneralGemmKirModelPropertyScopeV1::MachineRefinementOpen
    );
}
