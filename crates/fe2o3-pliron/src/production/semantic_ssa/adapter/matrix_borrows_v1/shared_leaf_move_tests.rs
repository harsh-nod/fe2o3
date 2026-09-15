use super::*;
use crate::production::{
    ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1, ProductionSemanticSsaSourceSiteV1,
    SemanticPartialMoveViolationV1,
};

fn owner(
    mutation: CarrierMutation,
) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    let mir =
        ProductionSemanticMirOwnerV1::try_new(tuple_carrier::source(mutation), Default::default())
            .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(mir, Default::default())
}

#[test]
fn exact_matrix_shared_leaf_move_has_original_borrow_use_and_partial_move_certificate() {
    assert_original_borrow_and_move_certificate(CarrierMutation::ProjectedMove, 1);
}

#[test]
fn disjoint_sibling_move_preserves_only_the_leaf_in_either_source_order() {
    for mutation in [
        CarrierMutation::SiblingMoveAfterLeafMove,
        CarrierMutation::SiblingMoveBeforeLeafMove,
        CarrierMutation::SiblingMovePolicyReference,
    ] {
        assert_original_borrow_and_move_certificate(mutation, 2);
    }
}

fn assert_original_borrow_and_move_certificate(mutation: CarrierMutation, moves: usize) {
    let owner = owner(mutation).unwrap();
    owner.verify_replay().unwrap();
    assert!(!owner.grants_proof_or_artifact_authority());
    let root = owner.source_semantic().roots()[0];
    let view = owner.execution_view_for_root(root).unwrap();
    let (site, local) = matrix_borrow(view);
    let plan = owner.execution_plan_for_root(root).unwrap();
    assert!(plan.partial_move_certificate().projected_moves() >= moves);
    assert!(
        plan.plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(local))
    );
    let SemanticStatementKindV1::Assign(a) =
        view.body().blocks()[site.block as usize].statements()[site.statement as usize].kind()
    else {
        panic!("retained assignment")
    };
    let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() else {
        panic!("retained borrow")
    };
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    query
        .borrow_place_use(
            ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(site.block),
                Some(site.statement),
            ),
            place,
            &mut || true,
        )
        .unwrap();

    let sites = super::super::super::super::borrowed_workgroup_v1::execution_sites(
        owner.source_semantic(),
        owner.execution_expansion(),
        view,
        65_536,
    )
    .unwrap();
    let input = |sites: &BTreeSet<SemanticTransparentBorrowSiteV1>| {
        super::super::super::super::semantic_function_ssa_input_v1(
            view.body(),
            Some(owner.source_semantic().types()),
            owner.source_semantic().callables(),
            sites,
        )
        .0
    };
    assert_eq!(
        input(&sites).blocks(),
        input(&BTreeSet::new()).blocks(),
        "transparency must not rewrite the original Copy/Move/Kill events"
    );
}

fn cause(mut error: &ProductionSemanticSsaErrorV1) -> &ProductionSemanticSsaErrorV1 {
    while let ProductionSemanticSsaErrorV1::ExpandedExecution { error: inner, .. } = error {
        error = inner;
    }
    error
}

#[test]
fn later_leaf_copy_whole_carrier_copy_and_double_move_reject_in_real_owner() {
    for mutation in [
        CarrierMutation::MoveThenLeafCopy,
        CarrierMutation::MoveThenWholeCopy,
        CarrierMutation::DoubleLeafMove,
    ] {
        let error = match owner(mutation) {
            Ok(_) => panic!("{mutation:?} must not publish an owner"),
            Err(error) => error,
        };
        assert!(
            matches!(
                cause(&error),
                ProductionSemanticSsaErrorV1::PartialMove {
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                }
            ),
            "{mutation:?}: {error:?}"
        );
    }
}

#[test]
fn moved_sibling_copy_whole_carrier_copy_and_double_move_reject_in_real_owner() {
    // The sibling is a ZST, but its moved path still requires initialization.
    for mutation in [
        CarrierMutation::SiblingMoveThenSiblingCopy,
        CarrierMutation::SiblingMoveThenWholeCopy,
        CarrierMutation::DoubleSiblingMove,
        CarrierMutation::UninitializedBeforeSiblingMove,
    ] {
        let error = match owner(mutation) {
            Ok(_) => panic!("{mutation:?} must not publish an owner"),
            Err(error) => error,
        };
        assert!(
            matches!(
                cause(&error),
                ProductionSemanticSsaErrorV1::PartialMove {
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                }
            ),
            "{mutation:?}: {error:?}"
        );
    }
}

#[test]
fn dead_or_uninitialized_carrier_cannot_publish_a_leaf_move_owner() {
    for mutation in [
        CarrierMutation::DeadBeforeLeafMove,
        CarrierMutation::UninitializedBeforeLeafMove,
        CarrierMutation::DeadBeforeSiblingMove,
    ] {
        let error = match owner(mutation) {
            Ok(_) => panic!("{mutation:?} must not publish an owner"),
            Err(error) => error,
        };
        assert!(
            matches!(
                cause(&error),
                ProductionSemanticSsaErrorV1::PartialMove {
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                } | ProductionSemanticSsaErrorV1::Planner {
                    error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { .. },
                    ..
                }
            ),
            "{mutation:?}: {error:?}"
        );
    }
}
