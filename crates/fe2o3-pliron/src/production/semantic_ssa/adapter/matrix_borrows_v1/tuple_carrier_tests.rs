use super::canonical_fixture::tuple_carrier::{self, CarrierMutation};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaResolvedEventV1, SsaVariableIdV1, plan_ssa_v1};

fn matrix_borrow(view: &SemanticExpandedRootV1) -> (SemanticTransparentBorrowSiteV1, u32) {
    let mut matches = Vec::new();
    for (block, origin) in view.block_origins().iter().enumerate() {
        if origin.function() != view.root() {
            continue;
        }
        for (statement, node) in view.body().blocks()[block].statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) = node.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() else {
                continue;
            };
            if place.ty().index() == 2 && place.projections().is_empty() {
                matches.push((
                    SemanticTransparentBorrowSiteV1 {
                        block: block as u32,
                        statement: statement as u32,
                    },
                    place.local().index(),
                ));
            }
        }
    }
    assert_eq!(matches.len(), 1);
    matches[0]
}

#[test]
fn source_checked_matrix_leaf_crosses_tuple_move_and_exact_field_copy() {
    let source = tuple_carrier::source(CarrierMutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 65_536).unwrap();
    assert_eq!(
        facts
            .carrier_leaves()
            .iter()
            .map(|(r, o)| (r.index(), o.index()))
            .collect::<Vec<_>>(),
        vec![(5, 2)]
    );
    assert!(
        facts
            .carrier_barriers()
            .contains(&SemanticTypeIdV1::from_index(7))
    );
    assert!(
        facts
            .carrier_barriers()
            .contains(&SemanticTypeIdV1::from_index(9))
    );
    let sites = super::super::super::borrowed_workgroup_v1::execution_sites(
        &source, &expansion, view, 65_536,
    )
    .unwrap();
    let (site, local) = matrix_borrow(view);
    assert!(sites.contains(&site));
    let mut events = None;
    let (input, _, _) = super::super::super::semantic_function_ssa_input_with_event_origins_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        &sites,
        |block, statement, range| {
            if block == site.block && statement == Some(site.statement) {
                assert!(events.replace(range).is_none());
            }
        },
    );
    assert!(input.promotable()[local as usize]);
    let plan = plan_ssa_v1(&input).unwrap();
    assert!(
        plan.promoted_variables()
            .contains(&SsaVariableIdV1::new(local))
    );
    let range = events.unwrap();
    assert_eq!(plan.resolved_events(SsaBlockIdV1::new(site.block)).unwrap().iter()
        .filter(|(event, resolved)| range.contains(&(*event as usize))
            && matches!(resolved, SsaResolvedEventV1::Use { variable, .. } if variable.get() == local)).count(), 1);
}

#[test]
fn source_matrix_carrier_retains_complete_use_and_single_leaf_rejection() {
    for mutation in [
        CarrierMutation::DuplicateDefinition,
        CarrierMutation::BorrowedCarrier,
        CarrierMutation::PointeeObservation,
        CarrierMutation::TwoMatrixLeaves,
        CarrierMutation::BorrowAfterLeafMove,
    ] {
        let source = tuple_carrier::source(mutation);
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let sites = super::super::super::borrowed_workgroup_v1::execution_sites(
            &source, &expansion, view, 65_536,
        )
        .unwrap();
        let (site, local) = matrix_borrow(view);
        assert!(
            !sites.contains(&site),
            "{mutation:?} cannot preserve a partially checked component"
        );
        let (input, _, _) = super::super::super::semantic_function_ssa_input_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &sites,
        );
        assert!(
            !input.promotable()[local as usize],
            "{mutation:?}: shape does not authorize a value"
        );
    }
}

#[path = "shared_leaf_move_tests.rs"]
mod shared_leaf_move_tests;

#[test]
fn matrix_carrier_seed_requires_the_existing_replayed_record_inventory() {
    let source = tuple_carrier::source(CarrierMutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let empty = MatrixBorrowSitesV1::new(&source, &expansion, view, &[], 65_536).unwrap();
    assert!(empty.carrier_leaves().is_empty());
    assert!(empty.carrier_barriers().is_empty());
    assert_eq!(empty.work_units, 0);
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let foreign = canonical_expansion(&source);
    assert!(matches!(
        MatrixBorrowSitesV1::new(
            &source,
            &expansion,
            foreign.root(source.roots()[0]).unwrap(),
            &bindings,
            65_536
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}
