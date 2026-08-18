use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    GENERAL_GEMM_KIR_LDS_ELEMENTS_V1, GENERAL_GEMM_KIR_SCHEMA_V1,
    GENERAL_GEMM_MUTATION_EXPECTATIONS_V1, GENERAL_GEMM_SEMANTIC_MUTATIONS_V1, GemmOperandV1,
    GeneralGemmAsyncWaitEventV1, GeneralGemmKirBuildErrorV1, GeneralGemmKirV1,
    GeneralGemmPhaseEventV1, GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotErrorV1,
    GeneralGemmPlanSnapshotV1, GeneralGemmPropertyV1, GeneralGemmRegionScopeV1,
    GeneralGemmRegionV1, GeneralGemmStageCompletionV1, GeneralGemmVerificationStageV1,
    MAX_GENERAL_GEMM_PHASE_EVENTS_V1, encode_general_gemm_kir_canonical_v1,
    general_gemm_kir_identity_v1, general_gemm_semantic_mutation_kir_v1,
    verify_general_gemm_kir_v1,
};

fn plan() -> GeneralGemmPlanFieldsV1 {
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 18],
        strides: [23, 29, 31],
        storage_elements: [386, 512, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 2,
        alpha_bits: 2.0_f32.to_bits(),
        beta_bits: (-1.0_f32).to_bits(),
    })
    .unwrap()
}

#[test]
fn checked_plan_fields_drive_regions_geometry_tails_and_scalars() {
    assert_eq!(GENERAL_GEMM_KIR_SCHEMA_V1, "fe2o3-general-gemm-kir-v1");
    let plan = plan();
    assert_eq!(plan.dimensions(), [17, 19, 18]);
    assert_eq!(plan.strides(), [23, 29, 31]);
    assert_eq!(plan.storage_elements(), [386, 512, 515]);
    assert_eq!(plan.block_counts(), [2, 2, 1]);
    assert_eq!(plan.aql_grid_work_items(), [128, 2, 1]);
    assert_eq!(plan.reduction_phases(), 2);
    assert_eq!((plan.tails().m, plan.tails().n, plan.tails().k), (1, 3, 2));
    assert_eq!(plan.alpha_bits(), 2.0_f32.to_bits());
    assert_eq!(plan.beta_bits(), (-1.0_f32).to_bits());

    let kir = GeneralGemmKirV1::canonical(plan);
    assert_eq!(kir.regions().len(), 5);
    assert_eq!(kir.regions()[0].region, GeneralGemmRegionV1::GlobalA);
    assert_eq!(kir.regions()[0].elements, 386);
    assert_eq!(kir.regions()[0].row_stride, 23);
    assert_eq!(kir.regions()[3].region, GeneralGemmRegionV1::LdsA);
    assert_eq!(kir.regions()[3].scope, GeneralGemmRegionScopeV1::Workgroup);
    assert_eq!(kir.regions()[3].elements, GENERAL_GEMM_KIR_LDS_ELEMENTS_V1);
    assert_eq!(kir.regions()[4].region, GeneralGemmRegionV1::LdsB);
    assert!(verify_general_gemm_kir_v1(&kir).is_ok());
}

#[test]
fn plan_projection_rejects_inconsistent_checked_fields() {
    let baseline = GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 18],
        strides: [23, 29, 31],
        storage_elements: [386, 512, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 2,
        alpha_bits: 0,
        beta_bits: 0,
    };

    let mut mutation = baseline;
    mutation.storage_elements[0] += 1;
    assert_eq!(
        GeneralGemmPlanFieldsV1::checked(mutation),
        Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
            field: "storage_elements"
        })
    );
    let mut mutation = baseline;
    mutation.block_counts[0] = 1;
    assert_eq!(
        GeneralGemmPlanFieldsV1::checked(mutation),
        Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
            field: "block_counts"
        })
    );
    let mut mutation = baseline;
    mutation.aql_grid_work_items[0] = 64;
    assert_eq!(
        GeneralGemmPlanFieldsV1::checked(mutation),
        Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
            field: "aql_grid_work_items"
        })
    );
    let mut mutation = baseline;
    mutation.reduction_phases = 1;
    assert_eq!(
        GeneralGemmPlanFieldsV1::checked(mutation),
        Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
            field: "reduction_phases"
        })
    );
    let mut mutation = baseline;
    mutation.strides[2] = 18;
    assert_eq!(
        GeneralGemmPlanFieldsV1::checked(mutation),
        Err(GeneralGemmPlanSnapshotErrorV1::StrideTooSmall {
            operand: GemmOperandV1::C,
            minimum: 19,
            actual: 18,
        })
    );
}

#[test]
fn zero_k_and_empty_output_match_host_planner_conventions() {
    let zero_k = GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 0],
        strides: [0, 0, 31],
        storage_elements: [0, 0, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 0,
        alpha_bits: 1.0_f32.to_bits(),
        beta_bits: 0.0_f32.to_bits(),
    })
    .unwrap();
    assert_eq!(zero_k.tails().k, 0);
    assert!(zero_k.requires_dispatch());
    assert!(verify_general_gemm_kir_v1(&GeneralGemmKirV1::canonical(zero_k)).is_ok());

    let empty = GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [0, 19, 18],
        strides: [0, 0, 0],
        storage_elements: [0, 0, 0],
        block_counts: [0, 0, 0],
        aql_grid_work_items: [0, 0, 0],
        reduction_phases: 0,
        alpha_bits: f32::NAN.to_bits(),
        beta_bits: (-0.0_f32).to_bits(),
    })
    .unwrap();
    assert!(!empty.requires_dispatch());
    assert!(verify_general_gemm_kir_v1(&GeneralGemmKirV1::canonical(empty)).is_ok());
}

#[test]
fn explicit_async_staging_is_valid_only_with_waits_before_publish() {
    let plan = plan();
    let canonical = GeneralGemmKirV1::canonical(plan);
    let mut events = canonical.phase_events().to_vec();
    for event in &mut events {
        if let GeneralGemmPhaseEventV1::Stage(stage) = event {
            stage.completion = GeneralGemmStageCompletionV1::PendingAsync;
        }
    }
    events.insert(
        2,
        GeneralGemmPhaseEventV1::AsyncWait(GeneralGemmAsyncWaitEventV1 {
            operand: GemmOperandV1::A,
        }),
    );
    events.insert(
        3,
        GeneralGemmPhaseEventV1::AsyncWait(GeneralGemmAsyncWaitEventV1 {
            operand: GemmOperandV1::B,
        }),
    );
    let kir =
        GeneralGemmKirV1::checked_from_parts(plan, events.clone(), *canonical.epilogue()).unwrap();
    verify_general_gemm_kir_v1(&kir).unwrap();

    events.swap(3, 4);
    let invalid =
        GeneralGemmKirV1::checked_from_parts(plan, events, *canonical.epilogue()).unwrap();
    let error = verify_general_gemm_kir_v1(&invalid).unwrap_err();
    assert_eq!(error.property, GeneralGemmPropertyV1::Initialized);
    assert_eq!(error.stage, GeneralGemmVerificationStageV1::Gpu);
    assert_eq!(error.code, 0x4647_0103);
}

#[test]
fn all_fifteen_semantic_mutations_have_exact_property_stage_and_code() {
    assert_eq!(GENERAL_GEMM_SEMANTIC_MUTATIONS_V1.len(), 15);
    assert_eq!(GENERAL_GEMM_MUTATION_EXPECTATIONS_V1.len(), 15);
    for (&mutation, &expected) in GENERAL_GEMM_SEMANTIC_MUTATIONS_V1
        .iter()
        .zip(&GENERAL_GEMM_MUTATION_EXPECTATIONS_V1)
    {
        assert_eq!(expected.mutation, mutation);
        assert_eq!(expected, mutation.expectation());
        let kir = general_gemm_semantic_mutation_kir_v1(plan(), mutation);
        let error = match verify_general_gemm_kir_v1(&kir) {
            Ok(_) => panic!("{mutation:?} unexpectedly verified"),
            Err(error) => error,
        };
        assert_eq!(error.property, expected.property, "{mutation:?}");
        assert_eq!(error.stage, expected.stage, "{mutation:?}");
        assert_eq!(error.code, expected.code, "{mutation:?}");
        assert_eq!(
            expected.property.verification_stage(),
            expected.stage,
            "{mutation:?}"
        );
        assert_eq!(
            expected.property.diagnostic_code(),
            expected.code,
            "{mutation:?}"
        );
    }
}

#[test]
fn canonical_identity_is_deterministic_and_binds_all_semantic_substitutions() {
    let plan = plan();
    let canonical = GeneralGemmKirV1::canonical(plan);
    assert_eq!(
        canonical.encode_canonical(),
        encode_general_gemm_kir_canonical_v1(&canonical)
    );
    assert_eq!(
        canonical.identity(),
        general_gemm_kir_identity_v1(&canonical)
    );
    assert_eq!(canonical.identity(), canonical.clone().identity());

    let mut identities = BTreeSet::from([canonical.identity()]);
    for mutation in GENERAL_GEMM_SEMANTIC_MUTATIONS_V1 {
        let hostile = general_gemm_semantic_mutation_kir_v1(plan, mutation);
        assert_ne!(hostile.identity(), canonical.identity(), "{mutation:?}");
        assert!(identities.insert(hostile.identity()), "{mutation:?}");
    }

    let coefficient_substitution = GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 18],
        strides: [23, 29, 31],
        storage_elements: [386, 512, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 2,
        alpha_bits: 3.0_f32.to_bits(),
        beta_bits: (-1.0_f32).to_bits(),
    })
    .unwrap();
    assert_ne!(
        canonical.identity(),
        GeneralGemmKirV1::canonical(coefficient_substitution).identity()
    );

    let mut reordered_events = canonical.phase_events().to_vec();
    reordered_events.swap(0, 1);
    let reordered =
        GeneralGemmKirV1::checked_from_parts(plan, reordered_events, *canonical.epilogue())
            .unwrap();
    verify_general_gemm_kir_v1(&reordered).unwrap();
    assert_ne!(canonical.identity(), reordered.identity());

    assert_eq!(
        *canonical.identity().as_bytes(),
        [
            210, 198, 178, 58, 186, 107, 24, 11, 144, 90, 20, 102, 40, 155, 177, 197, 185, 237, 93,
            147, 124, 158, 95, 58, 164, 149, 97, 210, 168, 19, 152, 32,
        ]
    );
}

#[test]
fn unverified_program_construction_enforces_the_event_bound() {
    let canonical = GeneralGemmKirV1::canonical(plan());
    let events = vec![
        GeneralGemmPhaseEventV1::AsyncWait(GeneralGemmAsyncWaitEventV1 {
            operand: GemmOperandV1::A,
        });
        MAX_GENERAL_GEMM_PHASE_EVENTS_V1 + 1
    ];
    assert_eq!(
        GeneralGemmKirV1::checked_from_parts(plan(), events, *canonical.epilogue()),
        Err(GeneralGemmKirBuildErrorV1::TooManyPhaseEvents {
            actual: MAX_GENERAL_GEMM_PHASE_EVENTS_V1 + 1,
            maximum: MAX_GENERAL_GEMM_PHASE_EVENTS_V1,
        })
    );
}
