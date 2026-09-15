use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticCallExpansionLimitsV1;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "fixture.rs"]
mod fixture;
use fixture::{Mutation, ty};
include!("cold_oracle.rs");

fn budget(limit: usize) -> Budget {
    Budget {
        limit,
        remaining: limit,
        profile: FlowWorkProfile::default(),
    }
}
fn expand(source: &AdmittedInertSemanticMirV1) -> SemanticCallExpansionV1 {
    let expansion =
        SemanticCallExpansionV1::try_new(source, SemanticCallExpansionLimitsV1::default()).unwrap();
    expansion.verify_replay(source).unwrap();
    expansion
}
fn matrix<'a>(
    source: &'a AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    view: &'a SemanticExpandedRootV1,
) -> MatrixBorrowSitesV1<'a> {
    MatrixBorrowSitesV1::new(
        source,
        expansion,
        view,
        &expansion.defined_capability_bindings(source).unwrap(),
        MAX_FLOW_WORK,
    )
    .unwrap()
}
fn original_assignment<'a>(
    view: &'a SemanticExpandedRootV1,
    local: u32,
) -> &'a SemanticAssignmentV1 {
    view.body()
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .find_map(|statement| match statement.kind() {
            SemanticStatementKindV1::Assign(a)
                if view.local_origins()[a.destination().local().index() as usize]
                    .instance()
                    .index()
                    == 0
                    && view.local_origins()[a.destination().local().index() as usize]
                        .local()
                        .index()
                        == local =>
            {
                Some(a)
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn replayed_bind_is_the_only_policy_carrier_seed() {
    let source = fixture::source(Mutation::None);
    let expansion = expand(&source);
    let view = &expansion.roots()[0];
    let matrix = matrix(&source, &expansion, view);
    assert_eq!(
        matrix.policy_carrier_leaves(),
        &BTreeMap::from([(ty(6), ty(3))])
    );
    let routes = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &matrix,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert_eq!(routes.owned(ty(13)), Some(&ty(3)));
    assert_eq!(routes.reference(ty(13)), Some(ty(6)));
    let absent = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &MatrixBorrowSitesV1::default(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert!(
        absent.routes.is_empty(),
        "policy shape and issuance terminal alone cannot seed a carrier"
    );
    let foreign = fixture::source(Mutation::MoveSubcarrier);
    let foreign_expansion = expand(&foreign);
    assert!(matches!(
        MatrixBorrowSitesV1::new(
            &source,
            &foreign_expansion,
            view,
            &expansion.defined_capability_bindings(&source).unwrap(),
            MAX_FLOW_WORK
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn policy_selection_preserves_prior_routes_ambiguity_and_branded_wrapper_barriers() {
    let source = fixture::source(Mutation::None);
    let expansion = expand(&source);
    let view = &expansion.roots()[0];
    let matrix = matrix(&source, &expansion, view);
    let routes = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &matrix,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let original = Routes::from_pairs(
        view.body(),
        source.types(),
        matrix.carrier_leaves(),
        matrix.carrier_barriers(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    for (ty, prior) in &original.routes {
        assert_eq!(routes.routes.get(ty), Some(prior), "prior route {ty:?}");
    }
    for (t, reference, owned, path) in [
        (13, 6, 3, vec![0]),
        (14, 5, 2, vec![0]),
        (17, 5, 2, vec![2]),
        (18, 6, 3, vec![0, 0]),
    ] {
        let route = routes.routes.get(&ty(t)).unwrap();
        assert_eq!((route.reference, route.owned), (ty(reference), ty(owned)));
        assert_eq!(route.fields[..route.len], path);
    }
    for t in [7, 9, 15, 16] {
        assert!(
            !routes.routes.contains_key(&ty(t)),
            "ambiguous or branded type {t}"
        )
    }
    let mut primary = matrix.carrier_leaves().clone();
    primary.insert(ty(6), ty(3));
    let strict = Routes::from_pairs(
        view.body(),
        source.types(),
        &primary,
        matrix.carrier_barriers(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert!(
        !strict.routes.contains_key(&ty(14)),
        "the old single-leaf invariant is not silently changed"
    );
    assert_eq!(
        routes.reference(ty(14)),
        Some(ty(5)),
        "existing Matrix carrier remains Matrix, not policy"
    );
}

#[test]
fn policy_capture_nested_transport_and_exact_shared_leaf_extraction_connect() {
    let source = fixture::source(Mutation::None);
    let expansion = expand(&source);
    let view = &expansion.roots()[0];
    let facts = matrix(&source, &expansion, view);
    let routes = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &facts,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    for (destination, parent) in [(13, 6), (20, 13), (21, 20), (14, 21), (15, 14)] {
        let assignment = original_assignment(view, destination);
        let source = routes
            .source(assignment, &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .unwrap();
        assert_eq!(
            view.local_origins()[source.local().index() as usize]
                .local()
                .index(),
            parent
        );
    }
    let moved = fixture::source(Mutation::MoveSubcarrier);
    let moved_expansion = expand(&moved);
    let moved_view = &moved_expansion.roots()[0];
    let moved_matrix = matrix(&moved, &moved_expansion, moved_view);
    let routes = Routes::new_with_matrix(
        moved_view.body(),
        Some(moved.types()),
        moved.callables(),
        &moved_matrix,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert!(
        routes
            .source(
                original_assignment(moved_view, 14),
                &mut budget(MAX_FLOW_WORK)
            )
            .unwrap()
            .is_none(),
        "moved sub-carriers remain excluded"
    );
}

#[test]
fn absence_of_policy_preserves_cold_results_node_counts_work_and_exact_errors() {
    let source = fixture::source(Mutation::None);
    for pairs in [
        BTreeMap::new(),
        BTreeMap::from([(ty(5), ty(2))]),
        BTreeMap::from([(ty(5), ty(2)), (ty(6), ty(3))]),
    ] {
        for barriers in [BTreeSet::new(), BTreeSet::from([ty(7), ty(9)])] {
            for i in 0..source.types().len() {
                for limit in [0, 1, 2, 4, 8, 16, 32, 64, 128] {
                    for initial_nodes in [0, MAX_SHAPE_NODES - 1, MAX_SHAPE_NODES] {
                        let (mut old, mut new) = (budget(limit), budget(limit));
                        let (mut old_nodes, mut new_nodes) = (initial_nodes, initial_nodes);
                        let (mut old_route, mut new_route) = (None, None);
                        let (mut old_path, mut new_path) = ([0; MAX_FIELDS], [0; MAX_FIELDS]);
                        let a = cold_walk(
                            source.types(),
                            &pairs,
                            &barriers,
                            ty(i as u32),
                            &mut old_path,
                            0,
                            &mut old_nodes,
                            &mut old_route,
                            &mut old,
                        );
                        let b = walk(
                            source.types(),
                            &pairs,
                            &BTreeMap::new(),
                            &barriers,
                            ty(i as u32),
                            &mut new_path,
                            0,
                            &mut new_nodes,
                            &mut new_route,
                            &mut Selection::default(),
                            &mut new,
                        );
                        assert_eq!(
                            a.map_err(|e| format!("{e:?}")),
                            b.map_err(|e| format!("{e:?}"))
                        );
                        assert_eq!(
                            (old.remaining, old_nodes, old_route, old_path),
                            (new.remaining, new_nodes, new_route, new_path)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn secondary_policy_does_not_hide_a_tracked_primary_sibling() {
    let source = fixture::source(Mutation::None);
    let expansion = expand(&source);
    let view = &expansion.roots()[0];
    let matrix = matrix(&source, &expansion, view);
    let routes = Routes::new_with_matrix(
        view.body(),
        Some(source.types()),
        source.callables(),
        &matrix,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let assignment = SemanticAssignmentV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(16), vec![], ty(14)).unwrap(),
        SemanticRvalueV1::new(
            ty(14),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![
                        SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], ty(5))
                                .unwrap(),
                        ),
                        SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(6), vec![], ty(6))
                                .unwrap(),
                        ),
                    ],
                )
                .unwrap(),
            ),
        ),
    );
    let site = SemanticTransparentBorrowSiteV1 {
        block: 0,
        statement: 0,
    };
    let mut candidates = vec![SemanticBorrowCandidateV1 {
        site,
        source_local: 3,
        source_type: ty(3),
        source_reference: None,
        value_alias: false, source_kind: SemanticBorrowCandidateSourceV1::Direct,
        valid: true,
        consumers: 0,
        intrinsic_consumer: false,
    }];
    assert!(
        routes
            .source(&assignment, &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .is_some()
    );
    routes
        .invalidate_siblings(
            &assignment,
            &BTreeMap::from([(6, 0)]),
            &mut candidates,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
    assert!(
        !candidates[0].valid,
        "Matrix priority cannot hide another tracked policy operand"
    );
}

#[test]
fn policy_carrier_allocation_and_lookup_work_keep_the_exact_cap_boundary() {
    let source = fixture::source(Mutation::None);
    let expansion = expand(&source);
    let view = &expansion.roots()[0];
    let matrix = matrix(&source, &expansion, view);
    let run = |limit| {
        Routes::new_with_matrix(
            view.body(),
            Some(source.types()),
            source.callables(),
            &matrix,
            &mut budget(limit),
        )
        .map(|routes| routes.routes)
    };
    let expected = run(MAX_FLOW_WORK).unwrap();
    assert!(expected.contains_key(&ty(13)));
    let (mut low, mut high) = (0, MAX_FLOW_WORK);
    while low < high {
        let mid = low + (high - low) / 2;
        if run(mid).is_ok() {
            high = mid
        } else {
            low = mid + 1
        }
    }
    assert_eq!(run(low).unwrap(), expected);
    let error = run(low - 1).unwrap_err();
    let ProductionSemanticSsaErrorV1::BorrowFlowWork { error, .. } = error else {
        panic!("{error:?}")
    };
    assert!(
        matches!(*error,ProductionSemanticSsaErrorV1::AggregateResourceLimit {resource:SsaPlannerResourceV1::WorkUnits,required,limit} if required==low&&limit==low-1)
    );
}

#[test]
fn policy_selection_cannot_restart_shape_limits_or_ignore_a_later_barrier() {
    let source = fixture::source(Mutation::None);
    let primary = BTreeMap::from([(ty(5), ty(2))]);
    let policy = BTreeMap::from([(ty(6), ty(3))]);
    for (root, depth, initial, barriers) in [
        (18, 0, MAX_SHAPE_NODES - 1, BTreeSet::new()),
        (18, MAX_FIELDS - 1, 0, BTreeSet::new()),
        (14, 0, 0, BTreeSet::from([ty(6)])),
        (17, 0, 0, BTreeSet::from([ty(5)])),
    ] {
        let mut nodes = initial;
        let mut selected = None;
        let mut secondary = Selection::default();
        assert!(
            !walk(
                source.types(),
                &primary,
                &policy,
                &barriers,
                ty(root),
                &mut [0; MAX_FIELDS],
                depth,
                &mut nodes,
                &mut selected,
                &mut secondary,
                &mut budget(MAX_FLOW_WORK)
            )
            .unwrap()
        );
        assert!(nodes >= initial && nodes <= MAX_SHAPE_NODES);
    }
}

// These two tests use the actual mounted parent classifier and SSA event API.
// The cached standalone harness omits this module, never substitutes its logic.
#[path = "integrated_tests.rs"]
mod integrated_tests;
