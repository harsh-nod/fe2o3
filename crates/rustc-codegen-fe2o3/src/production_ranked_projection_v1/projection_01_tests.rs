fn ordinary_test_lowering_mut(
    root: &mut ProductionRankedRootProgramV1,
) -> &mut ProductionRankedKernelLoweringInputV1 {
    let crate::production_reference_effect_join_v2::conditional::ReferenceRootV1::Ordinary {
        lowering,
        ..
    } = &mut root.verification
    else {
        panic!("ordinary mutation fixture unexpectedly became conditional");
    };
    lowering
}

// Fixtures exercise the same source roster and executable stage as production.
// Invalid source launches remain typed errors; a lowerer failure is a fixture
// failure, never a fallback to the old source-only projector.
fn materialize_ranked_fixture_v1(
    ssa: ProductionSemanticSsaOwnerV1,
    inputs: &[ProductionRankedRootInputV1],
) -> Result<fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1, ProductionRankedProjectionErrorV1>
{
    let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, inputs)
        .map_err(source_launch_projection_error_v1)?;
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let materialized =
        fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .expect("the source-ranked fixture must also materialize its executable graph");
    let retained = materialized.retained_analysis_storage_v1();
    budget.reserve_storage(retained).unwrap();
    Ok(materialized)
}

fn materialized_ranked_fixture_receipt_v1(
    materialized: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    root: ProductionRankedRootProgramV1,
) -> fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1 {
    fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            materialized,
            vec![
                fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1::new(
                    root.semantic_root,
                    root.source_rank,
                    root.verification.into_ordinary().expect("ordinary test root").0,
                    root.ranked_ir,
                    root.access_sources,
                    root.executable_effect_sources,
                ),
            ],
        )
        .expect("the fixture retains structurally valid source/ranked correspondence")
}

fn project_ranked_fixture_v1(
    semantic_ssa: ProductionSemanticSsaOwnerV1,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    let materialized = materialize_ranked_fixture_v1(semantic_ssa, inputs)?;
    project_and_verify_ranked_materialized_semantic_mir_v1(materialized, inputs, references)
}

#[path = "conditional_generated_attribution_v1_tests.rs"]
mod conditional_generated_attribution_v1_tests;
#[path = "reference_continuation_v1_tests.rs"]
mod reference_continuation_v1_tests;
use super::*;
use fe2o3_mir_model::SemanticOptionProducerV1;
use fe2o3_mir_model::semantic_mir_v1::*;

const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const ARRAY_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const ENUM_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const BOOL_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const U8_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const I32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const U64_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const CHECKED_U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const CHECKED_U64_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const F64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const U128_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const U16_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const CHAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
const F32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
const VALIDITY_U32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);

#[test]
fn grid_exclusive_runtime_indices_require_exact_unsigned_64_bit_types() {
    let types = assertion_proof_types();
    assert_eq!(unsigned_index_bits_v1(&types, U64_TYPE), Some(64));
    for hostile in [SCALAR_TYPE, U8_TYPE, I32_TYPE, U128_TYPE, BOOL_TYPE] {
        assert_ne!(unsigned_index_bits_v1(&types, hostile), Some(64));
    }
}

#[test]
fn blocked_launch_bound_checks_the_last_thread_and_component_without_wrapping() {
    assert!(blocked_mapping_fits_launch_v1(Some(64), 16, 4));
    assert!(!blocked_mapping_fits_launch_v1(None, 16, 4));
    assert!(!blocked_mapping_fits_launch_v1(None, 1, 2));
    assert!(!blocked_mapping_fits_launch_v1(Some(64), 0, 4));
    assert!(!blocked_mapping_fits_launch_v1(Some(64), 16, 0));
    assert!(!blocked_mapping_fits_launch_v1(Some(64), u64::MAX, 2));
    assert!(blocked_mapping_fits_launch_v1(Some(64), 1, 2));
    assert!(blocked_mapping_fits_launch_v1(Some(1_u64 << 63), 1, 2));
    assert!(!blocked_mapping_fits_launch_v1(
        Some((1_u64 << 63) + 1),
        1,
        2,
    ));
    assert!(blocked_mapping_fits_launch_v1(Some(1_u64 << 62), 16, 4));
    assert!(!blocked_mapping_fits_launch_v1(
        Some((1_u64 << 62) + 1),
        16,
        4,
    ));
}

#[test]
fn authenticated_ranked_projection_exposes_v5_and_inert_induction_evidence() {
    let middle_end_accessor: for<'a> fn(
        &'a AuthenticatedRankedVerificationV5,
    ) -> &'a fe2o3_pliron::ProductionMiddleEndEvidenceV5 =
        AuthenticatedRankedVerificationV5::middle_end_evidence;
    let induction_accessor: for<'a> fn(
            &'a AuthenticatedRankedVerificationV5,
        ) -> &'a fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1 =
            AuthenticatedRankedVerificationV5::semantic_u32_induction;
    let _ = (middle_end_accessor, induction_accessor);
}

#[test]
fn ranked_roster_receipt_source_preserves_linear_stage_boundaries() {
    let source = include_str!("../production_ranked_projection_v1.rs");
    let receipt = source
        .split("pub struct ProductionRankedSemanticProjectionRosterReceiptV1 {")
        .nth(1)
        .expect("ranked roster receipt declaration")
        .split('}')
        .next()
        .expect("ranked roster receipt fields");
    for retained in [
        "materialized: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1",
        "source_order_roots: Box<[ProductionRankedVerifiedRootCandidateV1]>",
        "canonical_kernel_order: Box<[usize]>",
        "canonical_roster_identity: ProductionRankedKernelRosterIdentityV1",
        "phase: retained_phase_v1::RetainedProjectionPhaseV1",
    ] {
        assert!(
            receipt.contains(retained),
            "missing custody field: {retained}"
        );
    }
    assert!(!receipt.contains("pub ") && !receipt.contains("pub(crate)"));

    let verified_root = source
        .split("pub(crate) struct ProductionRankedVerifiedRootCandidateV1 {")
        .nth(1)
        .expect("verified ranked root declaration")
        .split('}')
        .next()
        .expect("verified ranked root fields");
    assert!(verified_root.contains("verification: AuthenticatedRankedVerificationV5"));
    assert!(!verified_root.contains("semantic_u32_induction:"));
    let verifier = source
        .split("fn authenticate_ranked_root_v5(")
        .nth(1)
        .expect("ranked root verifier")
        .split("fn ranked_roster_identity_records_v1")
        .next()
        .expect("bounded ranked root verifier");
    assert!(verifier.contains("semantic_u32_induction,"));

    let module = source
        .split("pub(crate) fn into_module_verified_receipt(")
        .nth(1)
        .expect("complete module transition")
        .split("impl ProductionRankedSemanticProgramV1")
        .next()
        .expect("bounded complete module transition");
    assert!(module.contains("for root in source_order_roots.into_vec()"));
    assert!(module.contains("from_unvalidated_projection_roster_candidate"));
    assert!(module.contains("AuthenticatedRankedVerificationRosterV1"));
    assert!(!module.contains("into_singleton_verified_receipt"));
    assert!(!module.contains("try_lower_after_ranked_checks"));
    for forbidden in ["artifact", "publication", "load", "launch"] {
        assert!(
            !receipt.contains(forbidden),
            "ranked roster receipt gained downstream {forbidden} custody",
        );
    }
}

#[test]
fn value_carrying_accesses_retain_exact_semantic_correspondence() {
    let view = ProductionRankedValueV1::Argument(0);
    let value = ProductionRankedValueV1::Argument(1);
    let blocks = [ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::ValueAccess {
                kind: AccessKindAttr::Write,
                view,
                indices: vec![],
                value,
            },
            ProductionRankedOperationV1::AtomicValueAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::AcquireRelease,
                scope: AtomicScopeAttr::Device,
                view,
                indices: vec![],
                value,
            },
        ],
        ProductionRankedTerminatorV1::Return,
    )];
    let sites = [
        ProjectedAccessSourceV1 {
            block: 0,
            operation: 0,
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            output_extent: None,
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 7,
                statement: None,
            }),
        },
        ProjectedAccessSourceV1 {
            block: 0,
            operation: 1,
            access: AccessKindAttr::AtomicReadModifyWrite,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            output_extent: None,
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 7,
                statement: None,
            }),
        },
    ];

    // Existing inert non-private rows do not query the source or its ledger.
    let retained = production_access_sources(
        &projection_types(),
        &projection_function(vec![block(0, vec![], SemanticTerminatorKindV1::Return)]),
        &blocks,
        &sites,
        &mut ComponentDynamicAssertionFactsV1,
    )
    .unwrap();

    assert_eq!(retained.len(), 2);
    assert_eq!(
        (
            retained[0].semantic_block(),
            retained[0].semantic_statement(),
            retained[0].semantic_access_ordinal(),
            retained[0].ranked_block(),
            retained[0].ranked_operation(),
        ),
        (7, None, 0, 0, 0),
    );
    assert_eq!(
        (
            retained[1].semantic_block(),
            retained[1].semantic_access_ordinal(),
            retained[1].ranked_operation(),
        ),
        (7, 1, 1),
    );
}

#[test]
fn conservative_allocation_effect_retains_exact_semantic_correspondence() {
    let blocks = [ProductionRankedBlockV1::new(
        vec![ProductionRankedOperationV1::AllocationEffect {
            kind: AccessKindAttr::Read,
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        }],
        ProductionRankedTerminatorV1::Return,
    )];
    let sites = [ProjectedAccessSourceV1 {
        block: 0,
        operation: 0,
        access: AccessKindAttr::Read,
        memory_space: MemorySpaceAttr::Global,
        source: SemanticSourceProvenanceV1::unavailable(),
        output_extent: None,
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block: 9,
            statement: None,
        }),
    }];

    // Existing inert non-private rows do not query the source or its ledger.
    let retained = production_access_sources(
        &projection_types(),
        &projection_function(vec![block(0, vec![], SemanticTerminatorKindV1::Return)]),
        &blocks,
        &sites,
        &mut ComponentDynamicAssertionFactsV1,
    )
    .unwrap();

    assert_eq!(retained.len(), 1);
    assert_eq!(
        (
            retained[0].semantic_block(),
            retained[0].semantic_statement(),
            retained[0].semantic_access_ordinal(),
            retained[0].ranked_block(),
            retained[0].ranked_operation(),
        ),
        (9, None, 0, 0, 0),
    );
}

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn ranked_root_input(name: &str, binding: u8, rank: u8) -> ProductionRankedRootInputV1 {
    let workgroup = match rank {
        1 => [64, 1, 1],
        2 => [8, 8, 1],
        3 => [4, 4, 4],
        _ => unreachable!(),
    };
    let launch = LaunchContract::new(
        rank,
        BlockSize::Exact(
            fe2o3_artifacts::Dimensions::new(workgroup[0], workgroup[1], workgroup[2]).unwrap(),
        ),
        fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    ProductionRankedRootInputV1::new(name, bytes(binding), &launch)
}

fn ranked_root_input_1d(name: &str, binding: u8, size: u32) -> ProductionRankedRootInputV1 {
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(fe2o3_artifacts::Dimensions::new(size, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    ProductionRankedRootInputV1::new(name, bytes(binding), &launch)
}

const NEUTRAL_UNIT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const NEUTRAL_LDS_SCOPE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const NEUTRAL_LDS_SCOPE_REFERENCE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const NEUTRAL_ELEMENT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const NEUTRAL_ELEMENT_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const NEUTRAL_U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const NEUTRAL_DYNAMIC_LDS_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const NEUTRAL_CONTEXT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const NEUTRAL_CONTEXT_REFERENCE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

fn neutral_scalar_backend_v1(
    primitive: SemanticBackendPrimitiveV1,
    maximum: u128,
) -> SemanticBackendReprV1 {
    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, maximum),
    ))
}

fn neutral_pointer_type_v1(
    tag: u8,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    validity_start: u128,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(validity_start, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn neutral_reference_type_v1(
    tag: u8,
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
    pointee_kind: SemanticAbiPointeeKindV1,
) -> SemanticTypeDeclV1 {
    neutral_pointer_type_v1(
        tag,
        pointee,
        SemanticPointerKindV1::Reference,
        mutability,
        1,
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(pointee_kind, 0, 1).unwrap()),
            None,
        ),
    )
}

fn neutral_semantic_types_v1() -> Vec<SemanticTypeDeclV1> {
    let unit = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(220)),
        SemanticLayoutIdentityV1::from_sha256(bytes(220)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(Vec::new(), Vec::new()).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let scope = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(221)),
        SemanticLayoutIdentityV1::from_sha256(bytes(221)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0], Vec::new()).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![NEUTRAL_UNIT_TYPE]).unwrap(),
        ),
    );
    let scope_reference = neutral_reference_type_v1(
        222,
        NEUTRAL_LDS_SCOPE_TYPE,
        SemanticMutabilityV1::Mutable,
        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
    );
    let element = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(223)),
        SemanticLayoutIdentityV1::from_sha256(bytes(223)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                u32::MAX.into(),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let element_pointer = neutral_pointer_type_v1(
        224,
        NEUTRAL_ELEMENT_TYPE,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        0,
    );
    let u64_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(225)),
        SemanticLayoutIdentityV1::from_sha256(bytes(225)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                u64::MAX.into(),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let dynamic_lds = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(226)),
        SemanticLayoutIdentityV1::from_sha256(bytes(226)),
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16, 24, 24, 24], Vec::new()).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                NEUTRAL_ELEMENT_POINTER_TYPE,
                NEUTRAL_U64_TYPE,
                NEUTRAL_U64_TYPE,
                NEUTRAL_UNIT_TYPE,
                NEUTRAL_UNIT_TYPE,
                NEUTRAL_UNIT_TYPE,
            ])
            .unwrap(),
        ),
    );
    let context = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(227)),
        SemanticLayoutIdentityV1::from_sha256(bytes(227)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let context_reference = neutral_reference_type_v1(
        228,
        NEUTRAL_CONTEXT_TYPE,
        SemanticMutabilityV1::Immutable,
        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
    );
    vec![
        unit,
        scope,
        scope_reference,
        element,
        element_pointer,
        u64_type,
        dynamic_lds,
        context,
        context_reference,
    ]
}

fn zero_sized_summary_helper_v1(
    tag: u8,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        Vec::new(),
        SemanticAbiValueV1::new(NEUTRAL_LDS_SCOPE_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(tag.wrapping_add(1))),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag.wrapping_add(2))),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag.wrapping_add(3))),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag.wrapping_add(4))),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag.wrapping_add(5))),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            local(
                tag.wrapping_add(6),
                NEUTRAL_LDS_SCOPE_TYPE,
                SemanticLocalRoleV1::Return,
            ),
            local(
                tag.wrapping_add(7),
                NEUTRAL_UNIT_TYPE,
                SemanticLocalRoleV1::Temporary,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn exact_zero_sized_summary_helper_v1(tag: u8) -> SemanticFunctionDeclV1 {
    zero_sized_summary_helper_v1(
        tag,
        vec![block(
            tag,
            vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    neutral_test_place_v1(0, NEUTRAL_LDS_SCOPE_TYPE),
                    SemanticRvalueV1::new(
                        NEUTRAL_LDS_SCOPE_TYPE,
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Aggregate,
                                vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    NEUTRAL_UNIT_TYPE,
                                    SemanticConstantValueV1::ZeroSized,
                                ))],
                            )
                            .unwrap(),
                        ),
                    ),
                ),
            ))],
            SemanticTerminatorKindV1::Return,
        )],
    )
}

fn neutral_plain_direct_abi_value_v1(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn neutral_reference_abi_value_v1(ty: SemanticTypeIdV1, shared: bool) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    shared,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn neutral_dynamic_lds_abi_value_v1() -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        NEUTRAL_DYNAMIC_LDS_TYPE,
        SemanticAbiPassModeV1::Indirect {
            attributes: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesNone),
                    true,
                    false,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                24,
                Some(8),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        },
    )
}

fn neutral_compiler_intrinsic_callable_v1(
    tag: u8,
    inputs: Vec<SemanticAbiValueV1>,
    output: SemanticAbiValueV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let arguments = inputs
        .into_iter()
        .map(SemanticAbiArgumentV1::source)
        .collect::<Vec<_>>();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        u32::try_from(arguments.len()).unwrap(),
        arguments,
        output,
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
    }
}

fn neutral_test_place_v1(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), Vec::new(), ty).unwrap()
}

fn neutral_test_call_v1(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination_local: u32,
    destination_ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                neutral_test_place_v1(destination_local, destination_ty),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn neutral_ranked_source_for_operation_v1(
    operation: SemanticCompilerIntrinsicOperationV1,
    elements: u32,
) -> ProductionSemanticSsaOwnerV1 {
    let scope_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        neutral_test_place_v1(2, NEUTRAL_LDS_SCOPE_REFERENCE_TYPE),
        SemanticRvalueV1::new(
            NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: neutral_test_place_v1(1, NEUTRAL_LDS_SCOPE_TYPE),
            },
        ),
    )));
    let context_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        neutral_test_place_v1(5, NEUTRAL_CONTEXT_REFERENCE_TYPE),
        SemanticRvalueV1::new(
            NEUTRAL_CONTEXT_REFERENCE_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: neutral_test_place_v1(4, NEUTRAL_CONTEXT_TYPE),
            },
        ),
    )));
    let blocks = vec![
        block(
            230,
            vec![scope_borrow],
            neutral_test_call_v1(
                1,
                vec![SemanticOperandV1::Copy(neutral_test_place_v1(
                    2,
                    NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                ))],
                3,
                NEUTRAL_DYNAMIC_LDS_TYPE,
                1,
            ),
        ),
        block(
            231,
            Vec::new(),
            neutral_test_call_v1(2, Vec::new(), 4, NEUTRAL_CONTEXT_TYPE, 2),
        ),
        block(
            232,
            vec![context_borrow],
            neutral_test_call_v1(
                3,
                vec![
                    SemanticOperandV1::Copy(neutral_test_place_v1(
                        5,
                        NEUTRAL_CONTEXT_REFERENCE_TYPE,
                    )),
                    // Match the post-borrow-check encoding produced by
                    // ordinary Rust for this non-Copy, no-drop value.
                    SemanticOperandV1::Copy(neutral_test_place_v1(3, NEUTRAL_DYNAMIC_LDS_TYPE)),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        NEUTRAL_ELEMENT_TYPE,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
                    )),
                ],
                6,
                NEUTRAL_ELEMENT_TYPE,
                3,
            ),
        ),
        block(233, Vec::new(), SemanticTerminatorKindV1::Return),
    ];
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(234)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        Vec::new(),
        SemanticAbiValueV1::new(NEUTRAL_UNIT_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [
        (NEUTRAL_UNIT_TYPE, SemanticLocalRoleV1::Return),
        (NEUTRAL_LDS_SCOPE_TYPE, SemanticLocalRoleV1::Temporary),
        (
            NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
            SemanticLocalRoleV1::Temporary,
        ),
        (NEUTRAL_DYNAMIC_LDS_TYPE, SemanticLocalRoleV1::Temporary),
        (NEUTRAL_CONTEXT_TYPE, SemanticLocalRoleV1::Temporary),
        (
            NEUTRAL_CONTEXT_REFERENCE_TYPE,
            SemanticLocalRoleV1::Temporary,
        ),
        (NEUTRAL_ELEMENT_TYPE, SemanticLocalRoleV1::Temporary),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (ty, role))| local(235 + index as u8, ty, role))
    .collect::<Vec<_>>();
    let dimensions = SemanticWorkgroupDimensionsV1::new([elements, 1, 1]).unwrap();
    let source_contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(242)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(243)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(244)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(245)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(246)),
        SemanticSourceProvenanceV1::unavailable(),
        root_abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"neutral_generated_hostile".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(247)),
        source_contract,
    ));
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        neutral_compiler_intrinsic_callable_v1(
            248,
            vec![neutral_reference_abi_value_v1(
                NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                false,
            )],
            neutral_dynamic_lds_abi_value_v1(),
            SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                scope: NEUTRAL_LDS_SCOPE_TYPE,
                dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
                element_storage: NEUTRAL_ELEMENT_TYPE,
                elements: u64::from(elements),
            },
        ),
        neutral_compiler_intrinsic_callable_v1(
            249,
            Vec::new(),
            SemanticAbiValueV1::new(NEUTRAL_CONTEXT_TYPE, SemanticAbiPassModeV1::Ignore),
            SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent {
                context: NEUTRAL_CONTEXT_TYPE,
            },
        ),
        neutral_compiler_intrinsic_callable_v1(
            250,
            vec![
                neutral_reference_abi_value_v1(NEUTRAL_CONTEXT_REFERENCE_TYPE, true),
                neutral_dynamic_lds_abi_value_v1(),
                neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
            ],
            neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
            operation,
        ),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        neutral_semantic_types_v1(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn neutral_ranked_program_for_operation_v1(
    operation: SemanticCompilerIntrinsicOperationV1,
    elements: u32,
) -> ProductionRankedSemanticProgramV1 {
    let owner = neutral_ranked_source_for_operation_v1(operation, elements);
    project_ranked_fixture_v1(
        owner,
        &[ranked_root_input_1d(
            "neutral_generated_hostile",
            247,
            elements,
        )],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap()
}

fn neutral_ranked_program_v1() -> ProductionRankedSemanticProgramV1 {
    neutral_ranked_program_for_operation_v1(
        SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
            context: NEUTRAL_CONTEXT_TYPE,
            dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
            element_storage: NEUTRAL_ELEMENT_TYPE,
            element: NEUTRAL_ELEMENT_TYPE,
        },
        64,
    )
}

fn neutral_scan_ranked_program_v1(
    kind: SemanticWorkgroupScanKindV1,
    elements: u32,
) -> ProductionRankedSemanticProgramV1 {
    neutral_ranked_program_for_operation_v1(
        SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
            context: NEUTRAL_CONTEXT_TYPE,
            dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
            element_storage: NEUTRAL_ELEMENT_TYPE,
            element: NEUTRAL_ELEMENT_TYPE,
            kind,
        },
        elements,
    )
}

fn scan_rounds_v1(elements: u32) -> u32 {
    if elements <= 1 {
        0
    } else {
        u32::BITS - (elements - 1).leading_zeros()
    }
}

#[test]
fn actual_ranked_roster_keeps_original_phase_through_both_consuming_moves() {
    use retained_phase_v1::observation::{self, Event};
    let (_, observed) = observation::observe(|| {
        // This is the existing real MIR/SSA/KIR/Pliron fixture, not a forged
        // receipt or a BF16 rustc-source qualification.
        let mut program = neutral_ranked_program_v1();
        let (identity, storage, work) = program
            .phase
            .with_budget(|budget| {
                Ok((
                    budget.work_ledger_identity_v1(),
                    budget.storage(),
                    budget.work(),
                ))
            })
            .unwrap();
        let mut roster = program.into_verified_roster_receipt().unwrap();
        roster
            .phase
            .with_budget(|budget| {
                assert!(budget.work_ledger_identity_v1() == identity);
                assert_eq!(budget.storage(), storage);
                assert!(budget.work() > work);
                budget
                    .check_prior_denials_v1()
                    .map_err(ProductionRankedVerificationErrorV1::ConditionalResource)
            })
            .unwrap();
        let (module, mut authenticated) = roster.into_module_verified_receipt().unwrap();
        assert_eq!(module.root_count(), authenticated.root_count());
        authenticated
            .phase
            .with_budget(|budget| {
                assert!(budget.work_ledger_identity_v1() == identity);
                assert_eq!(budget.storage(), storage);
                assert!(budget.work() > work);
                budget
                    .check_prior_denials_v1()
                    .map_err(ProductionRankedVerificationErrorV1::ConditionalResource)
            })
            .unwrap();
        // Actual graph/source custody dies before the proof roster/account.
        drop(module);
        drop(authenticated);
    });
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseRetained(_)))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseDropped { .. }))
            .count(),
        1
    );
    assert!(matches!(observed.events.last(),
            Some(Event::PhaseDropped { after, poisoned: false, .. }) if after.storage == 0));
    assert!(
        !observed.events[..observed.events.len() - 1]
            .iter()
            .any(|e| matches!(e, Event::PhaseDropped { .. }))
    );
}

#[test]
fn actual_ranked_roster_refusal_drops_original_account_without_replacement() {
    use retained_phase_v1::observation::{self, Event};
    let (_, observed) = observation::observe(|| {
        let mut program = neutral_ranked_program_v1();
        program.roots[0].semantic_root = SemanticFunctionIdV1::from_index(u32::MAX);
        assert!(matches!(
            program.into_verified_roster_receipt(),
            Err(ProductionRankedVerificationErrorV1::RosterMetadata(_))
        ));
    });
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseRetained(_)))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseDropped { .. }))
            .count(),
        1
    );
    assert!(matches!(observed.events.last(),
            Some(Event::PhaseDropped { after, poisoned: false, .. }) if after.storage == 0));
}

#[test]
fn actual_ranked_roster_conversion_does_not_hide_original_sticky_work_denial() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use retained_phase_v1::observation::{self, Event};
    let (_, observed) = observation::observe(|| {
        let mut program = neutral_ranked_program_v1();
        program
            .phase
            .with_budget(|budget| {
                assert!(budget.charge_work(usize::MAX).is_err());
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            program.into_verified_roster_receipt(),
            Err(ProductionRankedVerificationErrorV1::ConditionalResource(
                Resource::Work(_)
            ))
        ));
    });
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseRetained(_)))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseDropped { .. }))
            .count(),
        1
    );
    assert!(matches!(observed.events.last(),
            Some(Event::PhaseDropped { before, after, poisoned: false })
                if before.failed_work == Some(usize::MAX)
                    && after.failed_work == before.failed_work && after.storage == 0));
}

#[test]
fn private_nominal_module_conversion_refuses_actual_raw_empty_and_drops_once() {
    use retained_phase_v1::observation::{self, Event};
    let (before, observed) = observation::observe(|| {
        // Existing real semantic/SSA/KIR/Pliron fixture, not BF16 source authority.
        let mut roster = neutral_ranked_program_v1()
            .into_verified_roster_receipt()
            .unwrap();
        assert_eq!(
            roster.materialized.helper_source_policy_v1(),
            fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
        );
        let before = roster
            .phase
            .with_budget(|budget| {
                Ok((
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                ))
            })
            .unwrap();
        assert!(
            matches!(roster.into_private_bf16_module_verified_receipt_v1(),
            Err(ProductionRankedVerificationErrorV1::Custody(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::
                    LocalHelperSourceConsumerUnavailable {
                        consumer: "private nominal module conversion",
                    },
            )))
        );
        before
    });
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseRetained(_)))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseDropped { .. }))
            .count(),
        1
    );
    let Some(Event::PhaseDropped {
        before: last,
        after,
        poisoned: false,
    }) = observed.events.last()
    else {
        panic!("original phase was not the final clean drop");
    };
    // require_clean, the same phase loan and the policy check: exactly 3 units.
    assert_eq!(last.work, before.0.checked_add(3).unwrap());
    assert_eq!(
        (
            last.storage,
            last.peak_storage,
            last.failed_work,
            last.failed_storage
        ),
        (before.1, before.2, before.3, before.4)
    );
    assert_eq!(after.storage, 0);
    assert_eq!(
        (
            after.work,
            after.peak_storage,
            after.failed_work,
            after.failed_storage
        ),
        (
            last.work,
            last.peak_storage,
            last.failed_work,
            last.failed_storage
        )
    );
}

#[test]
fn private_nominal_module_conversion_preserves_original_denials_before_policy() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use retained_phase_v1::observation::{self, Event};
    for storage_failure in [false, true] {
        let (before, observed) = observation::observe(|| {
            let mut roster = neutral_ranked_program_v1()
                .into_verified_roster_receipt()
                .unwrap();
            let before = roster
                .phase
                .with_budget(|budget| {
                    if storage_failure {
                        assert!(budget.reserve_storage(usize::MAX).is_err());
                    } else {
                        assert!(budget.charge_work(usize::MAX).is_err());
                    }
                    Ok((
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_work(),
                        budget.failed_storage(),
                    ))
                })
                .unwrap();
            let result = roster.into_private_bf16_module_verified_receipt_v1();
            if storage_failure {
                assert!(matches!(
                    result,
                    Err(ProductionRankedVerificationErrorV1::ConditionalResource(
                        Resource::Storage(_)
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionRankedVerificationErrorV1::ConditionalResource(
                        Resource::Work(_)
                    ))
                ));
            }
            before
        });
        assert_eq!(
            observed
                .events
                .iter()
                .filter(|e| matches!(e, Event::PhaseRetained(_)))
                .count(),
            1
        );
        assert_eq!(
            observed
                .events
                .iter()
                .filter(|e| matches!(e, Event::PhaseDropped { .. }))
                .count(),
            1
        );
        let Some(Event::PhaseDropped {
            before: last,
            after,
            poisoned: false,
        }) = observed.events.last()
        else {
            panic!("original denied phase was not the final drop");
        };
        assert_eq!(
            (
                last.work,
                last.storage,
                last.peak_storage,
                last.failed_work,
                last.failed_storage
            ),
            before
        );
        assert_eq!(after.storage, 0);
        assert_eq!(
            (
                after.work,
                after.peak_storage,
                after.failed_work,
                after.failed_storage
            ),
            (
                last.work,
                last.peak_storage,
                last.failed_work,
                last.failed_storage
            )
        );
    }
}

#[test]
fn private_nominal_lowerer_module_constructor_refuses_actual_raw_empty_on_same_phase() {
    let program = neutral_ranked_program_v1();
    // The actual projection account outlives both moved source and test roots.
    let mut phase = program.phase;
    let materialized = program.materialized;
    drop(program.roots);
    phase
        .with_budget(|budget| {
            let ledger = budget.work_ledger_identity_v1();
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            let result = fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1::
            from_private_bf16_projection_roster_with_budget_v1(
                materialized, Vec::new().into_boxed_slice(), budget,
            );
            assert!(
                matches!(result, Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::
            LocalHelperSourceConsumerUnavailable { consumer: "private nominal module receipt" }))
            );
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (before.0.checked_add(1).unwrap(), before.1, before.2)
            );
            assert!(budget.check_prior_denials_v1().is_ok());
            Ok(())
        })
        .unwrap();
    drop(phase);
}

#[test]
fn private_nominal_module_revalidation_is_read_only_and_keeps_legacy_closed() {
    let roster = neutral_ranked_program_v1()
        .into_verified_roster_receipt()
        .unwrap();
    let (receipt, mut verification) = roster.into_module_verified_receipt().unwrap();
    let roots = receipt.root_count();
    verification
        .phase
        .with_budget(|budget| {
            let ledger = budget.work_ledger_identity_v1();
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            assert!(
                matches!(receipt.verify_private_bf16_module_roster_with_budget_v1(
            &[0; 32], SemanticFunctionIdV1::from_index(0), [0, 1, 2, 3], budget,
        ), Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::
            LocalHelperSourceConsumerUnavailable { consumer: "private nominal module receipt" }))
            );
            assert_eq!(receipt.root_count(), roots);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (before.0.checked_add(1).unwrap(), before.1, before.2)
            );
            assert!(budget.check_prior_denials_v1().is_ok());
            Ok(())
        })
        .unwrap();
    drop(receipt);
    drop(verification);
}

#[test]
fn roster_observer_failed_consuming_conversion_creates_no_paid_output() {
    use retained_phase_v1::observation::{self, Event};
    let (before, observed) = observation::observe(|| {
        let mut program = neutral_ranked_program_v1();
        program.roots[0].kernel_binding[0] ^= 1;
        // Harness diagnostics are outside the selected paid observation domain.
        let before = program
            .phase
            .with_budget(|budget| {
                Ok((
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                ))
            })
            .unwrap();
        assert!(matches!(
            bf16_nominal_owned_projection_v1::refuse_roster_observation_conversion_fixture_v1(
                program
            ),
            Err(ProductionRankedVerificationErrorV1::RosterMetadata(
                "substituted ranked root identity metadata"
            ))
        ));
        before
    });
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseRetained(_)))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseDropped { .. }))
            .count(),
        1
    );
    let Some(Event::PhaseDropped {
        before: last,
        after,
        poisoned: false,
    }) = observed.events.last()
    else {
        panic!("original phase was not the final clean drop");
    };
    assert_eq!(last.work, before.0.checked_add(1).unwrap());
    assert_eq!(
        (
            last.storage,
            last.peak_storage,
            last.failed_work,
            last.failed_storage
        ),
        (before.1, before.2, before.3, before.4)
    );
    assert_eq!(after.storage, 0);
    assert_eq!(
        (
            after.work,
            after.peak_storage,
            after.failed_work,
            after.failed_storage
        ),
        (
            last.work,
            last.peak_storage,
            last.failed_work,
            last.failed_storage
        )
    );
}

#[test]
fn module_observer_failed_consuming_conversion_creates_no_paid_output() {
    use retained_phase_v1::observation::{self, Event};
    let (before, observed) = observation::observe(|| {
        let mut roster = neutral_ranked_program_v1()
            .into_verified_roster_receipt()
            .unwrap();
        let before = roster
            .phase
            .with_budget(|budget| {
                Ok((
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                ))
            })
            .unwrap();
        assert!(matches!(
            bf16_nominal_module_receipt_v1::refuse_module_observation_conversion_fixture_v1(roster),
            Err(ProductionRankedVerificationErrorV1::Custody(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::
                    LocalHelperSourceConsumerUnavailable {
                        consumer: "private nominal module conversion",
                    }
            ))
        ));
        before
    });
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseRetained(_)))
            .count(),
        1
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|e| matches!(e, Event::PhaseDropped { .. }))
            .count(),
        1
    );
    let Some(Event::PhaseDropped {
        before: last,
        after,
        poisoned: false,
    }) = observed.events.last()
    else {
        panic!("original phase was not the final clean drop");
    };
    assert_eq!(last.work, before.0.checked_add(3).unwrap());
    assert_eq!(
        (
            last.storage,
            last.peak_storage,
            last.failed_work,
            last.failed_storage
        ),
        (before.1, before.2, before.3, before.4)
    );
    assert_eq!(after.storage, 0);
    assert_eq!(
        (
            after.work,
            after.peak_storage,
            after.failed_work,
            after.failed_storage
        ),
        (
            last.work,
            last.peak_storage,
            last.failed_work,
            last.failed_storage
        )
    );
}

#[test]
fn no_carry_module_revalidation_keeps_legacy_receipt_closed() {
    let roster = neutral_ranked_program_v1()
        .into_verified_roster_receipt()
        .unwrap();
    let (receipt, mut verification) = roster.into_module_verified_receipt().unwrap();
    verification
        .phase
        .with_budget(|budget| {
            let ledger = budget.work_ledger_identity_v1();
            let before = (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            );
            assert!(
                matches!(receipt.verify_private_bf16_module_retained_source_with_budget_v1(
            SemanticFunctionIdV1::from_index(0), [0, 1, 2, 3], budget,
        ), Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::
            LocalHelperSourceConsumerUnavailable { consumer: "private nominal module receipt" }))
            );
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage()
                ),
                (
                    before.0.checked_add(1).unwrap(),
                    before.1,
                    before.2,
                    before.3,
                    before.4
                )
            );
            Ok(())
        })
        .unwrap();
    drop(receipt);
    drop(verification);
}
