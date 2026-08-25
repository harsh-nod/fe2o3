use std::ops::Range;

use dialect_kernel::{
    AccessKindAttr, MemorySpaceAttr, OwnershipCoverageAttr, OwnershipPartitionAttr,
};
use dialect_mir::pliron::MirProductionPlironLimitsV1;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    COMPLETE_GPU_HIERARCHY_V1, FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
    FunctionalRefinementSubjectsV2, MirPlironSemanticContractV1, ParallelNumericalPolicyV1,
    ParallelOutputRelationV1, ParallelReferenceContractV1, ParallelScheduleRelationV1,
    SafeReferenceKindV2, SemanticFiniteDomainV1, SemanticFiniteExtentV1, SemanticLoopContractV1,
    SemanticLoopDirectionV1, SemanticNumericalPolicyV1, SemanticOutputContractV1,
    SemanticScalarTypeV1, SemanticTypedRootV1, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    InertProductionMiddleEndEvidenceV4, InertProductionMiddleEndEvidenceV5,
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4, MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
    PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4, PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4,
    PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5, PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4,
    ProductionConstructionV1, ProductionEffectRefinementContractV2,
    ProductionFunctionalRefinementTrustPolicyV2, ProductionGpuWriteSiteV2,
    ProductionMiddleEndAssuranceV4, ProductionMiddleEndEvidenceCodecErrorV4,
    ProductionMiddleEndEvidencePassV4, ProductionMiddleEndEvidenceV4,
    ProductionMiddleEndEvidenceV5, ProductionNumericalContractV2, ProductionRankedBlockV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionReferenceOutputSiteV2, ProductionReferenceProofV2, ProductionSemanticExpressionV2,
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticScalarTypeV2,
    ProductionSessionLimitsV1, ShellLimits, compile_ranked_kernel_for_lowering_v1,
    compile_ranked_kernel_for_lowering_v2, normalized_effect_refinement_hash_for_kernel_v2,
    production_effect_contract_identity_v1, production_loop_transition_identity_v1,
    production_loop_variant_identity_v1, production_ranked_value_identity_v1,
    require_mir_pliron_semantic_contract_v1, require_parallel_reference_contract_v1,
    require_total_output_refinement_v2, verify_ranked_kernel_against_safe_reference_mir_v1,
};
use fe2o3_proof_contracts::DigestV1;

const RANKED_IR: &str = "func @static_copy {\n  kernel.return\n}\n";

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn proof_digest(tag: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes(bytes(tag))
}

fn functional_subjects() -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        proof_digest(31),
        DigestV1::ZERO,
        proof_digest(32),
        proof_digest(33),
        proof_digest(34),
    )
    .unwrap()
}

fn imported_reference(
    obligation: DigestV1,
) -> (
    ProductionReferenceProofV2,
    fe2o3_functional_proof::ImportedFunctionalRefinementProofV2,
    ProductionFunctionalRefinementTrustPolicyV2,
) {
    let signing = SigningKey::from_bytes(&[91; 32]);
    let toolchain = VerusToolchainIdentityV2::new(
        proof_digest(40),
        proof_digest(41),
        proof_digest(42),
        proof_digest(43),
        proof_digest(44),
    )
    .unwrap();
    let binding =
        FunctionalRefinementBindingV2::from_subjects(functional_subjects(), obligation).unwrap();
    let import_policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    let signer_identity = import_policy.signer_identity();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        signer_identity,
        binding,
        toolchain,
        proof_digest(45),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    )
    .unwrap();
    let wire = unsigned
        .clone()
        .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
    let mut importer = FunctionalRefinementReceiptImporterV2::new(import_policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let production_policy =
        ProductionFunctionalRefinementTrustPolicyV2::new([signer_identity], toolchain).unwrap();
    (
        ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
        imported,
        production_policy,
    )
}

fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
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
    )
}

fn semantic_function() -> SemanticFunctionDeclV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(10)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        )],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"middle_end_evidence_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5)),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ))
}

fn semantic_owner() -> ProductionSemanticMirOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        vec![semantic_function()],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(
        admitted,
        ProductionSemanticMirLimitsV1::new(
            ShellLimits::default(),
            MirProductionPlironLimitsV1::default(),
        ),
    )
    .unwrap()
}

fn ranked_input(index: u64) -> ProductionRankedKernelLoweringInputV1 {
    ranked_input_with_domain(index, true)
}

fn ranked_input_with_domain(
    index: u64,
    full_physical_workgroups: bool,
) -> ProductionRankedKernelLoweringInputV1 {
    let view = ProductionRankedValueIdV1::new(0);
    let coordinate = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "static_copy",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups,
                },
                ProductionRankedOperationV1::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: coordinate,
                    value: index,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(coordinate)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("middle_end_evidence", kernel).unwrap();
    compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
        .unwrap()
}

fn evidence(index: u64, ranked_ir: &str) -> ProductionMiddleEndEvidenceV4 {
    ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &ranked_input(index), ranked_ir)
        .unwrap()
}

fn total_coverage_ranked_input() -> ProductionRankedKernelLoweringInputV1 {
    let view = ProductionRankedValueIdV1::new(0);
    let coordinate = ProductionRankedValueIdV1::new(1);
    let local = |value| ProductionRankedValueV1::Local(value);
    let kernel = ProductionRankedKernelV1::new(
        "total_coverage_evidence",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 3,
                    global_extents: [4, 1, 1],
                    workgroup_extents: [4, 1, 1],
                    subgroup_size: 4,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width: 32,
                    writable: true,
                    shape: vec![4],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                    memory_space: MemorySpaceAttr::Global,
                },
                ProductionRankedOperationV1::OwnershipContract {
                    view: local(view),
                    coverage: OwnershipCoverageAttr::TotalView,
                    partition: OwnershipPartitionAttr::DenseRectangles,
                },
                ProductionRankedOperationV1::InvocationIndex {
                    result: coordinate,
                    dimension: 0,
                    launch_extent: 4,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(view),
                    indices: vec![local(coordinate)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("total_coverage_evidence", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap()
}

fn total_output_refinement_input() -> ProductionRankedKernelLoweringInputV1 {
    let local = |identity| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity));
    let scalar_u32 = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let scalar_u64 = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 64,
    };
    let scalar_bool = ProductionSemanticScalarTypeV2::Bool;
    let contract = ProductionEffectRefinementContractV2::new(
        73,
        ProductionGpuWriteSiteV2::new(0, 8),
        ProductionReferenceOutputSiteV2::new(0, 0, 0),
        local(0),
        vec![local(1)],
        vec![local(5)],
        vec![local(5)],
        local(4),
        local(4),
        local(4),
        local(4),
        local(2),
        local(3),
    )
    .unwrap();
    let skeleton = ProductionRankedKernelV1::new(
        "total_output_refinement",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 9,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 9,
                    noalias_class: 9,
                    memory_space: MemorySpaceAttr::Global,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: 0,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(2),
                    expression: ProductionSemanticExpressionV2::Constant {
                        scalar: scalar_u32,
                        bits: 7,
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(3),
                    expression: ProductionSemanticExpressionV2::Constant {
                        scalar: scalar_u32,
                        bits: 7,
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(4),
                    expression: ProductionSemanticExpressionV2::Constant {
                        scalar: scalar_bool,
                        bits: 1,
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(5),
                    expression: ProductionSemanticExpressionV2::Constant {
                        scalar: scalar_u64,
                        bits: 0,
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::OwnershipContract {
                    view: local(0),
                    coverage: OwnershipCoverageAttr::TotalView,
                    partition: OwnershipPartitionAttr::ExactSets,
                },
                ProductionRankedOperationV1::ValueAccess {
                    kind: AccessKindAttr::Write,
                    view: local(0),
                    indices: vec![local(1)],
                    value: local(2),
                },
                ProductionRankedOperationV1::RequestEffectRefinement {
                    contract,
                    subjects: functional_subjects(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
        &skeleton.blocks()[0].operations()[9]
    else {
        unreachable!()
    };
    let obligation = normalized_effect_refinement_hash_for_kernel_v2(
        &skeleton,
        0,
        9,
        contract,
        functional_subjects(),
    )
    .unwrap();
    let (proof, imported, policy) = imported_reference(obligation);
    let bound = skeleton
        .bind_functional_refinement_request_v2(0, 9, proof)
        .unwrap();
    compile_ranked_kernel_for_lowering_v2(
        ProductionConstructionV1::ranked_kernel("total_output_refinement", bound).unwrap(),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap()
}

fn loop_total_output_refinement_input() -> ProductionRankedKernelLoweringInputV1 {
    let local = |identity| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity));
    let scalar_u32 = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let scalar_u64 = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 64,
    };
    let scalar_bool = ProductionSemanticScalarTypeV2::Bool;
    let contract = ProductionEffectRefinementContractV2::new(
        73,
        ProductionGpuWriteSiteV2::new(3, 0),
        ProductionReferenceOutputSiteV2::new(0, 0, 0),
        local(0),
        vec![local(1)],
        vec![local(5)],
        vec![local(5)],
        local(4),
        local(4),
        local(4),
        local(4),
        local(2),
        local(3),
    )
    .unwrap();
    let skeleton = ProductionRankedKernelV1::new(
        "loop_total_output_refinement",
        0,
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 9,
                        global_extents: [1, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::ViewInSpace {
                        result: ProductionRankedValueIdV1::new(0),
                        element_width: 32,
                        writable: true,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin: 9,
                        noalias_class: 9,
                        memory_space: MemorySpaceAttr::Global,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(1),
                        value: 0,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(2),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 7,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(3),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u32,
                            bits: 7,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(4),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_bool,
                            bits: 1,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: ProductionRankedValueIdV1::new(5),
                        expression: ProductionSemanticExpressionV2::Constant {
                            scalar: scalar_u64,
                            bits: 0,
                        },
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(6),
                        value: 0,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(7),
                        value: 4,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(8),
                        value: 1,
                    },
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(0),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                ],
                ProductionRankedTerminatorV1::BranchArgs {
                    arguments: vec![local(6)],
                    target: 1,
                },
            ),
            ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                ProductionRankedTerminatorV1::IndexLessThanArgs {
                    lhs: ProductionRankedValueV1::BlockArgument {
                        block: 1,
                        argument: 0,
                    },
                    rhs: local(7),
                    true_arguments: vec![ProductionRankedValueV1::BlockArgument {
                        block: 1,
                        argument: 0,
                    }],
                    false_arguments: vec![],
                    true_block: 2,
                    false_block: 3,
                },
            ),
            ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                ProductionRankedTerminatorV1::BranchArgsAdd {
                    value: ProductionRankedValueV1::BlockArgument {
                        block: 2,
                        argument: 0,
                    },
                    step: local(8),
                    target: 1,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(0),
                        indices: vec![local(1)],
                        value: local(2),
                    },
                    ProductionRankedOperationV1::RequestEffectRefinement {
                        contract,
                        subjects: functional_subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            ),
        ],
    )
    .unwrap();
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
        &skeleton.blocks()[3].operations()[1]
    else {
        unreachable!()
    };
    let obligation = normalized_effect_refinement_hash_for_kernel_v2(
        &skeleton,
        3,
        1,
        contract,
        functional_subjects(),
    )
    .unwrap();
    let (proof, imported, policy) = imported_reference(obligation);
    let bound = skeleton
        .bind_functional_refinement_request_v2(3, 1, proof)
        .unwrap();
    compile_ranked_kernel_for_lowering_v2(
        ProductionConstructionV1::ranked_kernel("loop_total_output_refinement", bound).unwrap(),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap()
}

#[test]
fn live_v5_evidence_uses_the_eight_pass_pipeline_without_vacuous_coverage_claims() {
    let input = ranked_input(7);
    let live =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let decoded = InertProductionMiddleEndEvidenceV5::decode(live.canonical_bytes()).unwrap();
    assert_eq!(
        decoded.pass_successes().map(|success| success.pass()),
        PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5
    );
    assert!(
        !decoded
            .coverage_summary()
            .has_non_vacuous_total_view_proof()
    );
    assert!(
        !decoded
            .coverage_summary()
            .has_non_vacuous_collective_contribution_proof()
    );
    assert_eq!(decoded.typed_semantic_summary().expression_roots, 0);
    assert!(decoded.typed_semantic_reconciliation().is_exact());
    assert!(!decoded.claims_full_arithmetic_correctness());
    assert!(!decoded.grants_target_value_authority());
}

#[test]
fn live_v5_evidence_binds_non_vacuous_total_view_counts() {
    let input = total_coverage_ranked_input();
    let live =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let coverage = live.coverage_summary();
    assert_eq!(coverage.total_view_declared(), 1);
    assert_eq!(coverage.total_view_proved(), 1);
    assert!(coverage.has_non_vacuous_total_view_proof());
    assert!(!coverage.has_non_vacuous_collective_contribution_proof());
    assert!(!live.claims_verus_verification());
    assert!(!live.claims_full_arithmetic_correctness());
}

#[test]
fn live_v5_and_aggregate_gate_prove_one_total_typed_output_at_the_mir_boundary() {
    let input = total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let report = require_total_output_refinement_v2(&input, &evidence).unwrap();
    assert_eq!(report.total_view_contracts(), 1);
    assert_eq!(report.effect_contracts(), 1);
    assert_eq!(report.typed_expression_roots(), 4);
    assert_eq!(report.retained_receipts(), 1);
    assert!(report.proves_total_output_refinement_at_mir_boundary());
    assert!(!report.grants_source_to_mir_authority());
    assert!(!report.grants_lowering_or_machine_code_authority());
    assert!(!report.grants_artifact_load_launch_or_hardware_authority());
}

fn semantic_contract_for_total_output(
    input: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    view: ProductionRankedValueV1,
    boolean_scalar: SemanticScalarTypeV1,
) -> MirPlironSemanticContractV1 {
    let local = |identity| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity));
    let domain = proof_digest(61);
    let typed_root = |operation: usize, value, scalar| {
        let ProductionRankedOperationV1::SemanticExpression {
            expression,
            numerical_contract,
            ..
        } = &input.kernel().blocks()[0].operations()[operation]
        else {
            unreachable!()
        };
        SemanticTypedRootV1::new(
            production_ranked_value_identity_v1(value),
            DigestV1::from_untrusted_bytes(
                expression.canonical_transcript_sha256(*numerical_contract),
            ),
            domain,
            scalar,
            SemanticNumericalPolicyV1::ExactBitVector,
        )
        .unwrap()
    };
    MirPlironSemanticContractV1::new(
        functional_subjects().safe_reference_mir_hash(),
        functional_subjects().kernel_mir_hash(),
        DigestV1::from_untrusted_bytes(*evidence.identity().sha256()),
        vec![SemanticFiniteDomainV1::new(domain, vec![SemanticFiniteExtentV1::Static(1)]).unwrap()],
        vec![
            typed_root(3, local(2), SemanticScalarTypeV1::Unsigned(32)),
            typed_root(4, local(3), SemanticScalarTypeV1::Unsigned(32)),
            typed_root(5, local(4), boolean_scalar),
            typed_root(6, local(5), SemanticScalarTypeV1::Unsigned(64)),
        ],
        vec![],
        vec![],
        vec![
            SemanticOutputContractV1::new(
                production_effect_contract_identity_v1(73),
                production_ranked_value_identity_v1(view),
                domain,
                production_ranked_value_identity_v1(local(2)),
                production_ranked_value_identity_v1(local(3)),
                [local(5), local(5), local(4), local(4), local(4), local(4)]
                    .into_iter()
                    .map(production_ranked_value_identity_v1)
                    .collect(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn semantic_contract_for_loop_total_output(
    input: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    wrong_transition: bool,
    loop_extent: SemanticFiniteExtentV1,
) -> MirPlironSemanticContractV1 {
    let local = |identity| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity));
    let output_domain = proof_digest(61);
    let loop_domain = proof_digest(62);
    let typed_root = |operation: usize, value, scalar| {
        let ProductionRankedOperationV1::SemanticExpression {
            expression,
            numerical_contract,
            ..
        } = &input.kernel().blocks()[0].operations()[operation]
        else {
            unreachable!()
        };
        SemanticTypedRootV1::new(
            production_ranked_value_identity_v1(value),
            DigestV1::from_untrusted_bytes(
                expression.canonical_transcript_sha256(*numerical_contract),
            ),
            output_domain,
            scalar,
            SemanticNumericalPolicyV1::ExactBitVector,
        )
        .unwrap()
    };
    let induction = production_ranked_value_identity_v1(ProductionRankedValueV1::BlockArgument {
        block: 1,
        argument: 0,
    });
    let lower = production_ranked_value_identity_v1(local(6));
    let upper = production_ranked_value_identity_v1(local(7));
    let step = production_ranked_value_identity_v1(local(8));
    let live_transition = production_loop_transition_identity_v1(input, 1, 2, 3).unwrap();
    let transition = if wrong_transition {
        proof_digest(99)
    } else {
        live_transition
    };
    let variant = production_loop_variant_identity_v1(
        1,
        2,
        3,
        induction,
        lower,
        upper,
        step,
        transition,
        SemanticLoopDirectionV1::Increasing,
    );
    MirPlironSemanticContractV1::new(
        functional_subjects().safe_reference_mir_hash(),
        functional_subjects().kernel_mir_hash(),
        DigestV1::from_untrusted_bytes(*evidence.identity().sha256()),
        vec![
            SemanticFiniteDomainV1::new(output_domain, vec![SemanticFiniteExtentV1::Static(1)])
                .unwrap(),
            SemanticFiniteDomainV1::new(loop_domain, vec![loop_extent]).unwrap(),
        ],
        vec![
            typed_root(3, local(2), SemanticScalarTypeV1::Unsigned(32)),
            typed_root(4, local(3), SemanticScalarTypeV1::Unsigned(32)),
            typed_root(5, local(4), SemanticScalarTypeV1::Boolean),
            typed_root(6, local(5), SemanticScalarTypeV1::Unsigned(64)),
        ],
        vec![
            SemanticLoopContractV1::new(
                proof_digest(70),
                1,
                2,
                3,
                loop_domain,
                induction,
                lower,
                upper,
                step,
                transition,
                variant,
                SemanticLoopDirectionV1::Increasing,
                loop_extent.inclusive_upper_bound(),
            )
            .unwrap(),
        ],
        vec![],
        vec![
            SemanticOutputContractV1::new(
                production_effect_contract_identity_v1(73),
                production_ranked_value_identity_v1(local(0)),
                output_domain,
                production_ranked_value_identity_v1(local(2)),
                production_ranked_value_identity_v1(local(3)),
                [local(5), local(5), local(4), local(4), local(4), local(4)]
                    .into_iter()
                    .map(production_ranked_value_identity_v1)
                    .collect(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn exact_mir_pliron_contract_joins_live_typed_effect_evidence() {
    let input = total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let contract = semantic_contract_for_total_output(
        &input,
        &evidence,
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        SemanticScalarTypeV1::Boolean,
    );
    let verified =
        verify_ranked_kernel_against_safe_reference_mir_v1(input, evidence, &contract).unwrap();
    let report = verified.semantic_contract_report();
    assert_eq!(report.typed_roots(), 4);
    assert_eq!(report.total_outputs(), 1);
    assert_eq!(report.finite_collectives(), 0);
    assert!(report.binds_safe_reference_mir_to_live_pliron());
    assert!(!report.proves_the_compiler_implementation_sound());
    assert!(!report.grants_llvm_or_later_authority());
    assert!(verified.establishes_total_output_refinement_at_mir_pliron_boundary());
    assert!(verified.compiler_projection_and_pass_soundness_remain_trusted());
    assert!(!verified.grants_llvm_or_later_authority());
}

#[test]
fn production_parallel_relation_is_derived_from_live_output_and_hierarchy_facts() {
    let input = total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let contract = semantic_contract_for_total_output(
        &input,
        &evidence,
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        SemanticScalarTypeV1::Boolean,
    );
    let total = require_total_output_refinement_v2(&input, &evidence).unwrap();
    let semantics =
        require_mir_pliron_semantic_contract_v1(&input, &evidence, total, &contract).unwrap();
    let receipt = input.retained_functional_refinement_receipts()[0]
        .receipt_identity()
        .digest();
    let output = &contract.outputs()[0];
    let relation = ParallelOutputRelationV1::new(
        proof_digest(90),
        output.identity(),
        output.output_domain(),
        ParallelScheduleRelationV1::PointwiseBijection,
        ParallelNumericalPolicyV1::ExactBitVector,
        COMPLETE_GPU_HIERARCHY_V1.to_vec(),
        vec![],
        receipt,
    )
    .unwrap();
    let expected =
        ParallelReferenceContractV1::new(contract.canonical_sha256(), vec![relation], vec![])
            .unwrap();
    let report =
        require_parallel_reference_contract_v1(&input, &evidence, semantics, &contract, &expected)
            .unwrap();
    assert_eq!(report.output_relations(), 1);
    assert_eq!(report.pointwise_relations(), 1);
    assert!(report.binds_reference_domains_to_complete_gpu_hierarchy());
    assert!(!report.grants_llvm_or_later_authority());

    let wrong = ParallelOutputRelationV1::new(
        proof_digest(91),
        output.identity(),
        output.output_domain(),
        ParallelScheduleRelationV1::PointwiseBijection,
        ParallelNumericalPolicyV1::ExactBitVector,
        COMPLETE_GPU_HIERARCHY_V1.to_vec(),
        vec![],
        proof_digest(92),
    )
    .unwrap();
    let wrong =
        ParallelReferenceContractV1::new(contract.canonical_sha256(), vec![wrong], vec![]).unwrap();
    let error =
        require_parallel_reference_contract_v1(&input, &evidence, semantics, &contract, &wrong)
            .unwrap_err();
    assert!(error.is_incomplete());
    assert!(error.to_string().contains("no retained authenticated"));
}

#[test]
fn semantic_contract_rejects_view_and_typed_root_substitution() {
    let input = total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let total = require_total_output_refinement_v2(&input, &evidence).unwrap();
    let wrong_view = semantic_contract_for_total_output(
        &input,
        &evidence,
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
        SemanticScalarTypeV1::Boolean,
    );
    assert!(matches!(
        require_mir_pliron_semantic_contract_v1(&input, &evidence, total, &wrong_view),
        Err(fe2o3_pliron::ProductionMirPlironSemanticContractErrorV1::OutputMismatch { index: 0 })
    ));

    let wrong_type = semantic_contract_for_total_output(
        &input,
        &evidence,
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        SemanticScalarTypeV1::Unsigned(32),
    );
    assert!(matches!(
        require_mir_pliron_semantic_contract_v1(&input, &evidence, total, &wrong_type),
        Err(fe2o3_pliron::ProductionMirPlironSemanticContractErrorV1::TypedRootMismatch)
    ));
}

#[test]
fn canonical_cfg_loop_is_bound_and_a_transition_substitution_is_rejected() {
    let input = loop_total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let contract = semantic_contract_for_loop_total_output(
        &input,
        &evidence,
        false,
        SemanticFiniteExtentV1::Static(4),
    );
    let verified =
        verify_ranked_kernel_against_safe_reference_mir_v1(input, evidence, &contract).unwrap();
    assert_eq!(verified.semantic_contract_report().bounded_loops(), 1);

    let input = loop_total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let wrong = semantic_contract_for_loop_total_output(
        &input,
        &evidence,
        true,
        SemanticFiniteExtentV1::Static(4),
    );
    let total = require_total_output_refinement_v2(&input, &evidence).unwrap();
    assert!(matches!(
        require_mir_pliron_semantic_contract_v1(&input, &evidence, total, &wrong),
        Err(
            fe2o3_pliron::ProductionMirPlironSemanticContractErrorV1::LoopValueMismatch {
                header: 1,
                latch: 2,
            }
        )
    ));
}

#[test]
fn dynamic_loop_rejects_an_unproved_narrow_bound() {
    let input = loop_total_output_refinement_input();
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(&semantic_owner(), &input, RANKED_IR).unwrap();
    let contract = semantic_contract_for_loop_total_output(
        &input,
        &evidence,
        false,
        SemanticFiniteExtentV1::Dynamic {
            symbol: 17,
            inclusive_upper_bound: 4,
        },
    );
    let total = require_total_output_refinement_v2(&input, &evidence).unwrap();
    let error =
        require_mir_pliron_semantic_contract_v1(&input, &evidence, total, &contract).unwrap_err();
    assert!(matches!(
        error,
        fe2o3_pliron::ProductionMirPlironSemanticContractErrorV1::DynamicLoopBoundUnproved {
            header: 1,
            latch: 2,
            inclusive_upper_bound: 4,
        }
    ));
    assert!(error.to_string().contains("no production range receipt"));
}

#[derive(Debug)]
struct Layout {
    domain: Range<usize>,
    policy: Range<usize>,
    assurance: usize,
    equivalence: usize,
    source_identity: Range<usize>,
    kernel_identity: Range<usize>,
    ranked_ir_len: usize,
    ranked_ir: Range<usize>,
    pass_count: usize,
    pass_records: Range<usize>,
    identity: Range<usize>,
}

fn u16_at(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn layout(bytes: &[u8]) -> Layout {
    let mut offset = 8 + 2 + 2 + 8 + 4;
    let domain_len = u16_at(bytes, offset);
    offset += 2;
    let domain = offset..offset + domain_len;
    offset = domain.end;
    let policy_len = u16_at(bytes, offset);
    offset += 2;
    let policy = offset..offset + policy_len;
    offset = policy.end;
    let assurance = offset;
    offset += 1;
    let equivalence = offset;
    offset += 1 + 2;
    let source_identity = offset..offset + 32;
    offset = source_identity.end;
    let kernel_identity = offset..offset + 32;
    offset = kernel_identity.end;
    let ranked_ir_len = offset;
    let ranked_len = u32_at(bytes, offset);
    offset += 4;
    let ranked_ir = offset..offset + ranked_len;
    offset = ranked_ir.end;
    let pass_count = offset;
    let passes = usize::from(bytes[pass_count]);
    offset += 1;
    let pass_records = offset..offset + passes * 10;
    offset = pass_records.end;
    let identity = offset..offset + 32;
    assert_eq!(identity.end, bytes.len());
    Layout {
        domain,
        policy,
        assurance,
        equivalence,
        source_identity,
        kernel_identity,
        ranked_ir_len,
        ranked_ir,
        pass_count,
        pass_records,
        identity,
    }
}

#[test]
fn live_evidence_round_trips_with_exact_internal_success_facts() {
    let live = evidence(7, RANKED_IR);
    let decoded = InertProductionMiddleEndEvidenceV4::decode(live.canonical_bytes()).unwrap();
    let wire = layout(live.canonical_bytes());

    assert_eq!(decoded.canonical_bytes(), live.canonical_bytes());
    assert_eq!(&live.canonical_bytes()[8..10], &4_u16.to_le_bytes());
    assert_eq!(live.canonical_bytes()[wire.pass_count], 7);
    assert_eq!(
        &live.canonical_bytes()[wire.domain],
        PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4
    );
    assert_eq!(decoded.identity(), live.identity());
    assert!(
        decoded
            .identity()
            .matches_canonical_bytes(decoded.canonical_bytes())
    );
    assert_ne!(*decoded.identity().sha256(), [0; 32]);
    assert_ne!(*decoded.source_semantic_identity(), [0; 32]);
    assert_ne!(*decoded.ranked_kernel_identity(), [0; 32]);
    assert_eq!(decoded.ranked_ir(), RANKED_IR);
    assert_eq!(decoded.policy(), PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4);
    assert_eq!(
        decoded.assurance(),
        ProductionMiddleEndAssuranceV4::InternalChecksOnly
    );
    assert_eq!(
        decoded.pass_successes().map(|success| success.pass()),
        PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4
    );
    assert_eq!(
        decoded.pass_successes()[0].pass(),
        ProductionMiddleEndEvidencePassV4::TensorLayout
    );
    for success in decoded.pass_successes() {
        assert!(success.is_clean());
        assert_eq!(success.finding_count(), 0);
        assert!(!success.grants_compiler_refinement_authority());
        assert!(!success.grants_artifact_or_launch_authority());
    }
    assert!(!live.authenticates_producer());
    assert!(!live.claims_verus_verification());
    assert!(!live.grants_compiler_refinement_authority());
    assert!(!live.grants_artifact_or_launch_authority());
    assert!(!live.grants_publication_authority());
    assert!(!live.grants_load_authority());
    assert!(!decoded.authenticates_producer());
    assert!(!decoded.claims_verus_verification());
    assert!(!decoded.grants_compiler_refinement_authority());
    assert!(!decoded.grants_artifact_or_launch_authority());
    assert!(!decoded.grants_publication_authority());
    assert!(!decoded.grants_load_authority());
    for forbidden_debug_label in [
        &b"RankedBoundsReportV1"[..],
        &b"RankedRaceReportV1"[..],
        &b"PlironBarrierReportV1"[..],
        &b"PlironWorkgroupMemoryReportV1"[..],
        &b"PlironSemanticRefinementReportV1"[..],
        &b"InternalChecksOnly"[..],
        &b"MemoryBounds"[..],
        &b"Clean"[..],
    ] {
        assert!(
            !live
                .canonical_bytes()
                .windows(forbidden_debug_label.len())
                .any(|window| window == forbidden_debug_label)
        );
    }
}

#[test]
fn construction_is_deterministic_and_binds_typed_kernel_and_ranked_ir() {
    let first = evidence(7, RANKED_IR);
    let repeated = evidence(7, RANKED_IR);
    assert_eq!(first.canonical_bytes(), repeated.canonical_bytes());
    assert_eq!(first.identity(), repeated.identity());
    assert_eq!(
        *first.identity().sha256(),
        [
            38, 203, 3, 120, 5, 159, 188, 183, 165, 252, 147, 76, 122, 85, 205, 194, 153, 68, 67,
            119, 181, 133, 83, 188, 125, 180, 90, 180, 110, 153, 164, 204,
        ]
    );

    let changed_kernel = evidence(8, RANKED_IR);
    assert_eq!(
        first.source_semantic_identity(),
        changed_kernel.source_semantic_identity()
    );
    assert_ne!(
        first.ranked_kernel_identity(),
        changed_kernel.ranked_kernel_identity()
    );
    assert_ne!(first.identity(), changed_kernel.identity());

    let changed_ir = evidence(7, "func @static_copy {\n  kernel.return /* v2 */\n}\n");
    assert_eq!(
        first.ranked_kernel_identity(),
        changed_ir.ranked_kernel_identity()
    );
    assert_ne!(first.identity(), changed_ir.identity());
}

#[test]
fn participant_domain_changes_ranked_and_evidence_identity() {
    let full = evidence(7, RANKED_IR);
    let partial_input = ranked_input_with_domain(7, false);
    let partial =
        ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &partial_input, RANKED_IR)
            .unwrap();
    assert_ne!(
        full.ranked_kernel_identity(),
        partial.ranked_kernel_identity()
    );
    assert_ne!(full.identity(), partial.identity());
}

#[test]
fn strict_decoder_rejects_schema_policy_success_and_authority_mutations() {
    let canonical = evidence(7, RANKED_IR).canonical_bytes().to_vec();
    let layout = layout(&canonical);

    let mut mutation = canonical.clone();
    mutation[0] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidMagic)
    );

    let mut mutation = canonical.clone();
    mutation[8..10].copy_from_slice(&5_u16.to_le_bytes());
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::UnsupportedVersion(
            5
        ))
    );

    let mut mutation = canonical.clone();
    mutation[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::UnsupportedFlags(1))
    );

    let mut mutation = canonical.clone();
    mutation[20] = 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved)
    );

    let mut mutation = canonical.clone();
    mutation[layout.domain.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidDomain)
    );

    let mut mutation = canonical.clone();
    mutation[layout.policy.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPolicy)
    );

    let mut mutation = canonical.clone();
    mutation[layout.assurance] = 2;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidAssurance(2))
    );

    let mut mutation = canonical.clone();
    mutation[layout.equivalence] = 0;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::SemanticEquivalenceNotEstablished)
    );

    let mut mutation = canonical.clone();
    mutation[layout.source_identity.clone()].fill(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroSemanticIdentity)
    );

    let mut mutation = canonical.clone();
    mutation[layout.kernel_identity.clone()].fill(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroRankedKernelIdentity)
    );

    let mut mutation = canonical.clone();
    mutation[layout.pass_count] = 4;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassCount(4))
    );

    let first_pass = layout.pass_records.start;
    let mut mutation = canonical.clone();
    mutation[first_pass] = 2;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassOrder {
            index: 0,
            expected: ProductionMiddleEndEvidencePassV4::TensorLayout,
            actual: 2,
        })
    );

    let mut mutation = canonical.clone();
    mutation[first_pass + 1] = 0;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassStatus {
            pass: ProductionMiddleEndEvidencePassV4::TensorLayout,
            actual: 0,
        })
    );

    let mut mutation = canonical.clone();
    mutation[first_pass + 2] = 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroFindings {
            pass: ProductionMiddleEndEvidencePassV4::TensorLayout,
            actual: 1,
        })
    );

    for authority_offset in [first_pass + 6, first_pass + 7] {
        let mut mutation = canonical.clone();
        mutation[authority_offset] = 1;
        assert_eq!(
            InertProductionMiddleEndEvidenceV4::decode(&mutation),
            Err(
                ProductionMiddleEndEvidenceCodecErrorV4::AuthorityClaimInEncoding {
                    pass: ProductionMiddleEndEvidencePassV4::TensorLayout,
                }
            )
        );
    }

    let mut mutation = canonical;
    mutation[first_pass + 8] = 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved)
    );
}

#[test]
fn strict_decoder_rejects_ranked_ir_identity_length_and_truncation_mutations() {
    let canonical = evidence(7, RANKED_IR).canonical_bytes().to_vec();
    let layout = layout(&canonical);

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::IdentityMismatch)
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.start] = 0;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset: 0 })
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.start] = 0xff;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidRankedIrUtf8)
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.end - 1] = b' ';
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrMissingFinalNewline)
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir_len..layout.ranked_ir_len + 4].copy_from_slice(
        &u32::try_from(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 + 1)
            .unwrap()
            .to_le_bytes(),
    );
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
            actual: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 + 1,
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
        })
    );

    let mut mutation = canonical.clone();
    mutation[layout.identity.clone()].fill(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroIdentity)
    );

    let mut mutation = canonical.clone();
    mutation[layout.identity.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::IdentityMismatch)
    );

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&trailing),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::TrailingBytes)
    );

    for length in 0..canonical.len() {
        assert!(InertProductionMiddleEndEvidenceV4::decode(&canonical[..length]).is_err());
    }
}

#[test]
fn constructor_and_decoder_enforce_aggregate_bounds_before_copying() {
    let maximum_ir = format!(
        "{}\n",
        "x".repeat(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 - 1)
    );
    let maximum = evidence(7, &maximum_ir);
    assert_eq!(
        maximum.canonical_bytes().len(),
        MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4
    );
    assert!(InertProductionMiddleEndEvidenceV4::decode(maximum.canonical_bytes()).is_ok());

    let too_large_ir = format!(
        "{}\n",
        "x".repeat(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4)
    );
    assert_eq!(
        ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &ranked_input(7), &too_large_ir,)
            .unwrap_err(),
        ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
            actual: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 + 1,
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
        }
    );

    let oversized = vec![0; MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 + 1];
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&oversized),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::TooLarge {
            actual: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 + 1,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
        })
    );
}

#[test]
fn constructor_rejects_noncanonical_ranked_ir() {
    for (ranked_ir, expected) in [
        ("", ProductionMiddleEndEvidenceCodecErrorV4::EmptyRankedIr),
        (
            "func @kernel {}",
            ProductionMiddleEndEvidenceCodecErrorV4::RankedIrMissingFinalNewline,
        ),
        (
            "func @kernel {\r\n}\n",
            ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset: 14 },
        ),
    ] {
        assert_eq!(
            ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &ranked_input(7), ranked_ir,)
                .unwrap_err(),
            expected
        );
    }
}
