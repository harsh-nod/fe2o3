//! Matching inert recipe and the pending graph without a reference contract.

use super::*;

pub(super) fn local(id: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(fe2o3_pliron::ProductionRankedValueIdV1::new(id))
}

fn reference_subjects() -> fe2o3_pliron::ProductionConditionalReferenceSubjectsV1 {
    use fe2o3_proof_contracts::DigestV1;
    fe2o3_pliron::ProductionConditionalReferenceSubjectsV1::new(
        fe2o3_functional_proof::SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes([1; 32]),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes([2; 32]),
        DigestV1::from_untrusted_bytes([3; 32]),
        DigestV1::from_untrusted_bytes([4; 32]),
    )
    .unwrap()
}

pub(super) fn output_recipe(
    source: &ProductionPreRankedKirOwnerV1,
    request: bool,
) -> fe2o3_pliron::ProductionRankedKernelV1 {
    use fe2o3_pliron::{
        ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2,
        ProductionNumericalContractV2, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1 as Term, ProductionRankedValueIdV1 as Id,
        ProductionReferenceOutputSiteV2, ProductionSemanticExpressionV2 as Expr,
        ProductionSemanticScalarTypeV2 as Scalar,
    };
    let extent = ProductionRankedValueV1::Argument(0);
    let layout = source.source_launch().roots()[0].layout();
    let scalar = Scalar::Integer {
        signed: false,
        bits: 32,
    };
    let mut body = vec![ProductionRankedOperationV1::ValueAccess {
        kind: dialect_kernel::AccessKindAttr::Write,
        view: local(1),
        indices: vec![local(0)],
        value: local(2),
    }];
    if request {
        body.push(ProductionRankedOperationV1::RequestEffectRefinement {
            contract: ProductionEffectRefinementContractV2::new(
                1,
                ProductionGpuWriteSiteV2::new(1, 0),
                ProductionReferenceOutputSiteV2::new(0, 0, 0),
                local(1),
                vec![local(0)],
                vec![local(4)],
                vec![local(4)],
                local(3),
                local(3),
                local(3),
                local(3),
                local(2),
                local(2),
            )
            .unwrap(),
            subjects: reference_subjects(),
        });
    }
    ProductionRankedKernelV1::new(
        "conditional_output",
        1,
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: layout.grid_identity(),
                        global_extents: layout.global_extents(),
                        workgroup_extents: layout.workgroup_extents(),
                        subgroup_size: layout.subgroup_size(),
                        full_physical_workgroups: layout.full_physical_workgroups(),
                    },
                    ProductionRankedOperationV1::InvocationIndex {
                        result: Id::new(0),
                        dimension: 0,
                        launch_extent: 0,
                    },
                    ProductionRankedOperationV1::View {
                        result: Id::new(1),
                        element_width: 32,
                        writable: true,
                        shape: vec![dialect_kernel::DYNAMIC_EXTENT],
                        dynamic_extents: vec![extent],
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: Id::new(2),
                        expression: Expr::Constant { scalar, bits: 7 },
                        numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
                    },
                    ProductionRankedOperationV1::SemanticConstant {
                        result: Id::new(3),
                        value: 1,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: Id::new(4),
                        expression: Expr::Symbol { symbol: 0, scalar },
                        numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
                    },
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(1),
                        coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
                        partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
                    },
                ],
                Term::IndexLessThan {
                    lhs: local(0),
                    rhs: extent,
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(body, Term::Branch { target: 2 }),
            ProductionRankedBlockV1::new(vec![], Term::Return),
        ],
    )
    .unwrap()
}

pub(super) fn pending(
    recipe: fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<
    fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
    fe2o3_pliron::ProductionSessionErrorV1,
> {
    use fe2o3_pliron::{
        ProductionConditionalOwnershipSiteV1, ProductionConstructionV1, ProductionPlironSessionV1,
        ProductionSessionLimitsV1,
    };
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    let registered = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("conditional_output", recipe).unwrap(),
        )
        .unwrap();
    let (stage, root) = session.construct_registered(registered)?;
    session.prepare_conditional_ranked_analysis_v1(
        stage,
        root,
        &[ProductionConditionalOwnershipSiteV1 {
            block: 0,
            operation: 6,
            view: local(1),
        }],
    )
}

pub(super) fn output_source_row() -> ProductionRankedAccessSourceV1 {
    ProductionRankedAccessSourceV1::new(2, Some(0), 0, 1, 0).with_output_extent(
        crate::ProductionRankedOutputExtentSourceV1::new(
            0,
            local(1),
            ProductionRankedValueV1::Argument(0),
            local(0),
        ),
    )
}

pub(super) fn output_input(
    source: &ProductionPreRankedKirOwnerV1,
) -> ProductionConditionalRootInputV1 {
    ProductionConditionalRootInputV1 {
        pending: pending(output_recipe(source, false)).unwrap(),
        semantic_root: 0,
        launch_rank: 1,
        access_sources: vec![output_source_row()],
        executable_effect_sources: vec![],
        ranked_ir: String::new(),
        reference_subjects: reference_subjects(),
    }
}
