use dialect_kernel::{
    AccessKindAttr, OwnershipCoverageAttr, OwnershipPartitionAttr, SemanticCoverageBindingAttr,
    SemanticEvaluationOrderAttr,
};
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_pliron::{
    ProductionCollectiveSemanticContractV1, ProductionCollectiveSemanticKindV1,
    ProductionConstructionV1, ProductionNumericalContractV2, ProductionRankedBlockV1,
    ProductionRankedKernelErrorV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_v1,
    normalized_functional_refinement_formula_hash_for_kernel_v2,
};
use fe2o3_proof_contracts::DigestV1;

fn local(identity: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity))
}

fn typed_constant(result: u32, bits: u64) -> ProductionRankedOperationV1 {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    ProductionRankedOperationV1::SemanticExpression {
        result: ProductionRankedValueIdV1::new(result),
        expression: ProductionSemanticExpressionV2::Constant { scalar, bits },
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
    }
}

fn contract() -> ProductionCollectiveSemanticContractV1 {
    ProductionCollectiveSemanticContractV1::new(
        ProductionCollectiveSemanticKindV1::FiniteFold,
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [5, 6, 7, 8],
        64,
        64,
        SemanticEvaluationOrderAttr::Ascending,
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        SemanticCoverageBindingAttr::TotalView,
    )
    .unwrap()
}

#[test]
fn ranked_recipe_retains_a_closed_typed_finite_fold_contract() {
    let kernel = ProductionRankedKernelV1::new(
        "finite_fold_recipe",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 17,
                    noalias_class: 17,
                },
                typed_constant(1, 7),
                typed_constant(2, 7),
                typed_constant(3, 0),
                typed_constant(4, 99),
                ProductionRankedOperationV1::OwnershipContract {
                    view: local(0),
                    coverage: OwnershipCoverageAttr::TotalView,
                    partition: OwnershipPartitionAttr::ExactSets,
                },
                ProductionRankedOperationV1::CollectiveSemantics {
                    contract: contract(),
                    view: local(0),
                    actual: local(1),
                    expected: local(2),
                    witness0: local(3),
                    witness1: local(4),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let ProductionRankedOperationV1::CollectiveSemantics { contract, .. } =
        &kernel.blocks()[0].operations()[6]
    else {
        panic!("collective contract was not retained")
    };
    assert_eq!(contract.domain_bound(), 64);
    assert_eq!(contract.step_bound(), 64);
    assert!(!contract.grants_gpu_implementation_refinement_authority());
}

#[test]
fn contract_constructor_rejects_unbounded_or_ambiguous_domains() {
    for (kind, source, target, domain, steps) in [
        (
            ProductionCollectiveSemanticKindV1::FiniteFold,
            [5, 6, 7, 8],
            [5, 6, 7, 8],
            0,
            0,
        ),
        (
            ProductionCollectiveSemanticKindV1::FiniteRecurrence,
            [5, 6, 7, 8],
            [9, 10, 11, 12],
            8,
            8,
        ),
        (
            ProductionCollectiveSemanticKindV1::PermutationGather,
            [5, 6, 7, 8],
            [5, 6, 7, 8],
            8,
            8,
        ),
    ] {
        assert!(matches!(
            ProductionCollectiveSemanticContractV1::new(
                kind,
                [1, 2, 3, 4],
                source,
                target,
                domain,
                steps,
                SemanticEvaluationOrderAttr::Explicit,
                ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                SemanticCoverageBindingAttr::TotalView,
            ),
            Err(ProductionRankedKernelErrorV1::InvalidCollectiveSemanticContract)
        ));
    }
}

#[test]
fn legacy_untyped_semantics_cannot_enter_a_production_collective_contract() {
    let result = ProductionRankedKernelV1::new(
        "untyped_fold_recipe",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 17,
                    noalias_class: 17,
                },
                ProductionRankedOperationV1::SemanticConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: 7,
                },
                typed_constant(2, 7),
                typed_constant(3, 0),
                typed_constant(4, 99),
                ProductionRankedOperationV1::CollectiveSemantics {
                    contract: contract(),
                    view: local(0),
                    actual: local(1),
                    expected: local(2),
                    witness0: local(3),
                    witness1: local(4),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    );
    assert!(matches!(
        result,
        Err(ProductionRankedKernelErrorV1::InvalidCollectiveSemanticContract)
    ));
}

#[test]
fn mandatory_production_pipeline_rejects_coverage_without_a_value_proof() {
    let kernel = ProductionRankedKernelV1::new(
        "unproved_finite_fold",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 17,
                    noalias_class: 17,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: 0,
                },
                typed_constant(2, 7),
                typed_constant(3, 7),
                typed_constant(4, 0),
                typed_constant(5, 99),
                ProductionRankedOperationV1::OwnershipContract {
                    view: local(0),
                    coverage: OwnershipCoverageAttr::TotalView,
                    partition: OwnershipPartitionAttr::ExactSets,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(0),
                    indices: vec![local(1)],
                },
                ProductionRankedOperationV1::CollectiveSemantics {
                    contract: contract(),
                    view: local(0),
                    actual: local(2),
                    expected: local(3),
                    witness0: local(4),
                    witness1: local(5),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("collective_module", kernel).unwrap();
    let error =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .unwrap_err();
    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains("error[FE2O3-SEMANTIC-005]"),
        "{error:?}: {diagnostic}"
    );
    assert!(diagnostic.contains("coverage never proves a final value"));
}

#[test]
fn authenticated_formula_identity_binds_the_complete_collective_contract() {
    fn subjects() -> FunctionalRefinementSubjectsV2 {
        let digest = |byte| DigestV1::from_untrusted_bytes([byte; 32]);
        FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            digest(1),
            DigestV1::ZERO,
            digest(2),
            digest(3),
            digest(4),
        )
        .unwrap()
    }
    fn kernel(contract_identity: [u64; 4]) -> ProductionRankedKernelV1 {
        let contract = ProductionCollectiveSemanticContractV1::new(
            ProductionCollectiveSemanticKindV1::FiniteFold,
            contract_identity,
            [5, 6, 7, 8],
            [5, 6, 7, 8],
            64,
            64,
            SemanticEvaluationOrderAttr::Ascending,
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
            SemanticCoverageBindingAttr::TotalView,
        )
        .unwrap();
        ProductionRankedKernelV1::new(
            "bound_collective_graph",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::View {
                        result: ProductionRankedValueIdV1::new(0),
                        element_width: 32,
                        writable: true,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin: 17,
                        noalias_class: 17,
                    },
                    typed_constant(1, 7),
                    typed_constant(2, 7),
                    typed_constant(3, 0),
                    typed_constant(4, 99),
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(0),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                    ProductionRankedOperationV1::CollectiveSemantics {
                        contract,
                        view: local(0),
                        actual: local(1),
                        expected: local(2),
                        witness0: local(3),
                        witness1: local(4),
                    },
                    ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                        actual: local(1),
                        expected: local(2),
                        subjects: subjects(),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }
    let first = kernel([1, 2, 3, 4]);
    let second = kernel([11, 12, 13, 14]);
    let hash = |kernel: &ProductionRankedKernelV1| {
        normalized_functional_refinement_formula_hash_for_kernel_v2(
            kernel,
            0,
            7,
            local(1),
            local(2),
            subjects(),
        )
        .unwrap()
    };
    assert_ne!(hash(&first), hash(&second));
}
