use super::*;
use crate::production_analysis::{
    LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
    ProductionAnalysisResourceLimitsV1 as Limits, derive_pliron_ir_structural_identity_v1,
    run_pliron_hierarchical_ownership_check_v1, run_pliron_ranked_bounds_check_v1,
    run_pliron_ranked_race_check_v1,
};

fn stored_kernel(rhs: Option<u32>) -> ProductionRankedKernelV1 {
    let mut recipe = kernel(Mode::UnorderedNonVolatile);
    let O::ViewInSpace { writable, .. } = &mut recipe.blocks[0].operations[1] else {
        unreachable!()
    };
    *writable = true;
    let expression = X::Constant {
        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        bits: 0,
    };
    recipe.blocks[0].operations.extend([
        O::SemanticExpression {
            result: ProductionRankedValueIdV1::new(3),
            numerical_contract: ProductionNumericalContractV2::exact_for_expression(&expression),
            expression,
        },
        if let Some(value) = rhs {
            O::ValueAccess {
                kind: AccessKindAttr::Write,
                view: local(0),
                indices: vec![local(1)],
                value: local(value),
            }
        } else {
            O::Access {
                kind: AccessKindAttr::Write,
                view: local(0),
                indices: vec![local(1)],
            }
        },
        O::OwnershipContract {
            view: local(0),
            coverage: OwnershipCoverageAttr::ExactView,
            partition: OwnershipPartitionAttr::ExactSets,
        },
    ]);
    ProductionRankedKernelV1::new("stored_read", 0, recipe.blocks).unwrap()
}

fn store_and_roots(
    session: &ProductionPlironSessionV1,
    stage: &Stage,
) -> (RankedAccessOp, Vec<Value>) {
    let context = &session.inner.context;
    let ops = operations(session, stage);
    let store = ops
        .iter()
        .filter(|op| Operation::is_op::<RankedAccessOp>(**op, context))
        .map(|op| RankedAccessOp::from_operation(*op))
        .find(|access| access.kind(context) == Some(AccessKindAttr::Write))
        .unwrap();
    let roots = ops
        .iter()
        .filter(|op| Operation::is_op::<SemanticTypedExpressionRootOp>(**op, context))
        .map(|op| op.deref(context).get_result(0))
        .collect();
    (store, roots)
}

#[test]
fn live_store_identity_and_census_retain_the_selected_rhs() {
    let mut identities = Vec::new();
    let mut censuses = Vec::new();
    for rhs in [None, Some(2), Some(3), Some(2)] {
        let (session, stage, _) = construct(stored_kernel(rhs)).unwrap();
        let context = &session.inner.context;
        let function = function(&session, &stage);
        let (store, roots) = store_and_roots(&session, &stage);
        assert_eq!(roots.len(), 2);
        assert_eq!(
            store.stored_value(context),
            rhs.map(|id| roots[id as usize - 2])
        );
        assert_eq!(store.indices(context).len(), 1);
        let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        censuses.push(
            provider
                .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
                .unwrap_or_else(|_| panic!("owner graph capture"))
                .input_census,
        );
        identities.push(derive_pliron_ir_structural_identity_v1(context, &function).unwrap());
    }
    assert_ne!(identities[0], identities[1]);
    assert_ne!(identities[1], identities[2]);
    assert_eq!(identities[1], identities[3]);
    assert_eq!(censuses[1], censuses[3]);
    assert_eq!(censuses[0].operations, censuses[1].operations);
    assert_eq!(censuses[0].results, censuses[1].results);
    assert_eq!(censuses[0].operands + 1, censuses[1].operands);
    assert_eq!(censuses[1].ranked_accesses, 2);
    assert_eq!(censuses[1].ownership_contracts, 1);
}

#[test]
fn load_derived_store_keeps_memory_checks_without_proving_read_value_semantics() {
    let (mut session, stage, root) = construct(stored_kernel(Some(2))).unwrap();
    let context = &session.inner.context;
    let function = function(&session, &stage);
    assert!(run_pliron_ranked_bounds_check_v1(context, &function).is_clean());
    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
    let ownership = run_pliron_hierarchical_ownership_check_v1(context, &function);
    assert!(ownership.is_clean(), "{ownership:?}");
    assert!(!ownership.regions().is_empty());
    let Err(ProductionSessionErrorV1::RankedSemantic(error)) =
        session.verify_production_ranked_kernel_pipeline(stage, root)
    else {
        panic!("storing a load-derived result must not prove its semantics");
    };
    assert!(error.report().findings().iter().any(|finding| matches!(
        finding,
        crate::PlironSemanticRefinementFindingV1::TypedExpressionRejected { .. }
    )));
}

#[test]
fn owner_rejects_changed_or_removed_store_values_before_analysis() {
    for remove in [false, true] {
        let (mut session, stage, root) = construct(stored_kernel(Some(2))).unwrap();
        let (store, roots) = store_and_roots(&session, &stage);
        let context = &mut session.inner.context;
        if remove {
            Operation::remove_operand(store.get_operation(), context, 2);
        } else {
            Operation::replace_operand(store.get_operation(), context, 2, roots[1]);
        }
        // Both mutations remain locally well-formed, but lose owner custody.
        pliron::op::verify_op(&store, context).unwrap();
        crate::production_analysis::panic_next_production_analysis_for_test_v1();
        assert!(matches!(
            session.verify_production_ranked_kernel_pipeline(stage, root),
            Err(ProductionSessionErrorV1::Operation(
                OperationHandleError::OperationGraphChangedOutsideTransaction
            ))
        ));
        assert!(session.is_poisoned());
        let (mut clean, stage, root) = construct(stored_kernel(Some(2))).unwrap();
        assert!(matches!(
            clean.verify_production_ranked_kernel_pipeline(stage, root),
            Err(ProductionSessionErrorV1::Operation(
                OperationHandleError::UpstreamPanicked
            ))
        ));
        assert!(clean.is_poisoned());
    }
}

#[test]
fn restored_pre_analysis_rhs_restores_bytes_but_not_mutation_history() {
    let (mut session, stage, root) = construct(stored_kernel(Some(2))).unwrap();
    let (store, roots) = store_and_roots(&session, &stage);
    let function = function(&session, &stage);
    let context = &mut session.inner.context;
    let original = derive_pliron_ir_structural_identity_v1(context, &function).unwrap();
    let before = context.ir_mutation_attempt_epoch().unwrap().value();
    Operation::replace_operand(store.get_operation(), context, 2, roots[1]);
    assert_ne!(
        original,
        derive_pliron_ir_structural_identity_v1(context, &function).unwrap()
    );
    Operation::replace_operand(store.get_operation(), context, 2, roots[0]);
    assert_eq!(
        original,
        derive_pliron_ir_structural_identity_v1(context, &function).unwrap()
    );
    assert!(context.ir_mutation_attempt_epoch().unwrap().value() >= before + 2);
    // A pre-analysis snapshot checks current bytes, not historical attempts.
    assert!(session.root_shape(&stage, &root).is_ok());
}
