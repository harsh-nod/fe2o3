//! Actual original Grid and Invocation3D borrows must retain their source events.
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticBorrowKindV1, SemanticFunctionIdV1, SemanticRvalueKindV1,
    SemanticStatementKindV1,
};
use fe2o3_mir_model::{SemanticExpandedStatementOriginV1, SsaResolvedEventV1};
use fe2o3_pliron::{
    ProductionSemanticSsaOwnerV1, ProductionSemanticSsaSourceQueryErrorV1,
    ProductionSemanticSsaSourceSiteV1,
};

pub(super) fn assert_source_census(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
) {
    let view = owner.execution_view_for_root(root).unwrap();
    let plan = owner.execution_plan_for_root(root).unwrap();
    let [receipt] = plan.guarded_grid_leader_results() else {
        panic!("one original Grid receipt")
    };
    let record = receipt.contract();
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    let mut left = 100_000_usize;
    let mut charge = || match left.checked_sub(1) {
        Some(next) => {
            left = next;
            true
        }
        None => false,
    };
    let mut counts = [0; 2];
    for (bi, origin) in view.block_origins().iter().enumerate() {
        for (si, marker) in origin.statements().iter().enumerate() {
            if !matches!(marker, SemanticExpandedStatementOriginV1::Source { .. }) {
                continue;
            }
            let SemanticStatementKindV1::Assign(a) =
                view.body().blocks()[bi].statements()[si].kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = a.value().kind()
            else {
                continue;
            };
            if !place.projections().is_empty() {
                continue;
            }
            let which = if origin.function() == record.source().caller
                && place.ty() == record.types().grid
            {
                0
            } else if origin.function() == record.source().grid_current {
                1
            } else {
                continue;
            };
            counts[which] += 1;
            let site = ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(bi as u32),
                Some(si as u32),
            );
            let value = query
                .borrow_place_use(site, place, &mut charge)
                .expect("the original shared owner must have its exact retained SSA Use");
            assert!(matches!(
                query.borrow_place_use(site, &place.clone(), &mut charge),
                Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)
            ));
            let events = query.resolved_events_at(site, &mut charge).unwrap();
            assert_eq!(
                events.len(),
                2,
                "one original Borrow Use followed by its reference Define; no synthesized source events"
            );
            assert!(
                matches!(&events[0].1,SsaResolvedEventV1::Use {variable,value:actual}
                if variable.get()==place.local().index() && *actual==value)
            );
            assert!(
                matches!(&events[1].1,SsaResolvedEventV1::Define {variable,..}
                if variable.get()==a.destination().local().index())
            );
        }
    }
    assert_eq!(
        counts,
        [1, 2],
        "one Grid rank borrow and both original Invocation3D arithmetic receivers"
    );
}
