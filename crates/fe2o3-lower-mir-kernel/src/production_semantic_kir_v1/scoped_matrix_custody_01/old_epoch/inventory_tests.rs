use super::super::inventory::Index;
use super::*;

// Reuse the existing rejection-only body fixtures and real SSA/Graph. The old
// complete-source loop is retained under cfg(test), not as a production path.
fn differential(body: &SemanticFunctionDeclV1, declarations: &[SemanticTypeDeclV1]) {
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        body,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut graph = Graph::new(body, plan.plan(), 1_000_000).unwrap();
    let mut index = Index::new(declarations, body, &mut |n| graph.charge(n)).unwrap();
    for seeds in [
        vec![id(2)],
        vec![id(1)],
        vec![id(8)],
        vec![id(2), id(1)],
        vec![id(2)],
    ] {
        for after in 0..body.blocks().len() as u32 {
            let mut reference = Graph::new(body, plan.plan(), 1_000_000).unwrap();
            let expected = first_use(declarations, &mut reference, after, &seeds).unwrap();
            assert_eq!(
                index
                    .first_use(declarations, &mut graph, after, &seeds)
                    .unwrap(),
                expected,
                "after={after} seeds={seeds:?}"
            );
        }
    }
}

#[test]
fn inventory_old_epoch_preserves_source_order_dead_paths_and_backedges() {
    for backedge in [false, true] {
        let value = body(vec![
            block(0, vec![assign(2, 2)], Some(1)),
            block(1, vec![assign(1, 1)], backedge.then_some(0)),
            block(2, vec![assign(2, 2), assign(1, 1)], Some(0)),
        ]);
        differential(&value, &declarations());
    }
}

#[test]
fn inventory_old_epoch_keeps_all_sibling_types_and_exact_earliest_statement() {
    let value = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(
            1,
            vec![
                assign(1, 1),
                statement(SemanticStatementKindV1::Assume(constant(2))),
                assign(2, 2),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(2),
                )),
            ],
            None,
        ),
    ]);
    differential(&value, &declarations());
    assert_eq!(scan(&value, 1), Some((1, Some(1))));
}

#[test]
fn inventory_old_epoch_query_seeds_are_not_cached_across_scopes() {
    let value = body(vec![
        block(0, vec![], Some(1)),
        block(1, vec![assign(1, 1)], None),
    ]);
    differential(&value, &declarations());
}

#[test]
fn inventory_old_epoch_keeps_assert_message_dependencies_and_terminator_order() {
    let original = block(1, vec![assign(1, 1)], Some(2));
    let assertion = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        original.statements().to_vec(),
        SemanticTerminatorV1::new(
            original.source(),
            SemanticTerminatorKindV1::Assert {
                condition: constant(1),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: constant(1),
                    index: constant(2),
                },
                target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::AssertSuccess,
                    SemanticBlockIdV1::from_index(2),
                ),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
    )
    .unwrap();
    let value = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        assertion,
        block(2, vec![], None),
    ]);
    differential(&value, &declarations());
    assert_eq!(scan(&value, 1), Some((1, None)));
}

#[test]
fn inventory_old_epoch_projected_scalar_keeps_its_declared_carrier_type() {
    let declarations = declarations();
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(1)).unwrap()],
        id(1),
    )
    .unwrap();
    let value = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(
            1,
            vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    place(1, 1),
                    SemanticRvalueV1::new(
                        id(1),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
                    ),
                ),
            ))],
            None,
        ),
    ]);
    // Like the existing projected-scalar test, this is rejection-only source
    // dependency coverage, not semantic layout or capability admission.
    differential(&value, &declarations);
    assert_eq!(scan(&value, 1), Some((1, Some(0))));
}

#[test]
fn inventory_old_epoch_rejects_changed_body_type_table_and_invalid_seed() {
    let declarations = declarations();
    let value = body(vec![block(0, vec![assign(1, 1)], None)]);
    let other = body(vec![block(0, vec![assign(2, 2)], None)]);
    let mut index = Index::new(&declarations, &value, &mut |_| Ok(())).unwrap();
    for (body, types, seeds) in [
        (&other, declarations.as_slice(), vec![id(2)]),
        (&value, declarations.as_slice(), vec![]),
        (&value, declarations.as_slice(), vec![id(99)]),
    ] {
        let plan = plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            body,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut graph = Graph::new(body, plan.plan(), 1_000_000).unwrap();
        assert!(matches!(
            index.first_use(types, &mut graph, 0, &seeds),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        &value,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut graph = Graph::new(&value, plan.plan(), 1_000_000).unwrap();
    let different_types = declarations.to_vec();
    assert!(matches!(
        index.first_use(&different_types, &mut graph, 0, &[id(2)]),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(matches!(
        index.first_use(&declarations, &mut graph, 99, &[id(2)]),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn inventory_old_epoch_malformed_sibling_never_becomes_absence() {
    let value = body(vec![block(
        0,
        vec![
            assign(2, 2),
            statement(SemanticStatementKindV1::Assume(constant(99))),
        ],
        None,
    )]);
    // Census has to validate this dependency even though another sibling type
    // is already an old-epoch candidate. No partial inventory is published.
    assert!(matches!(
        Index::new(&declarations(), &value, &mut |_| Ok(())),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn inventory_old_epoch_amortizes_repeated_scans_under_one_unchanged_graph_budget() {
    let declarations = declarations();
    let value = body(vec![
        block(0, vec![assign(2, 2)], Some(1)),
        block(1, (0..128).map(|_| assign(1, 1)).collect(), None),
    ]);
    let plan = plan_semantic_function_ssa_v1(
        SemanticFunctionIdV1::from_index(0),
        &value,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let run = |indexed: bool, limit: usize, prefix: usize| -> Result<()> {
        let mut graph = Graph::new(&value, plan.plan(), limit)?;
        graph.charge(prefix)?;
        let mut index = if indexed {
            Some(Index::new(&declarations, &value, &mut |n| graph.charge(n))?)
        } else {
            None
        };
        for _ in 0..24 {
            let found = match &mut index {
                Some(index) => index.first_use(&declarations, &mut graph, 1, &[id(2)])?,
                None => first_use(&declarations, &mut graph, 1, &[id(2)])?,
            };
            assert_eq!(found, None);
        }
        Ok(())
    };
    let minimum = |indexed, prefix| {
        let mut low = 0;
        let mut high = 1_000_000;
        run(indexed, high, prefix).unwrap();
        while low < high {
            let mid = low + (high - low) / 2;
            match run(indexed, mid, prefix) {
                Ok(()) => high = mid,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    limit,
                    ..
                }) if limit == mid => low = mid + 1,
                other => panic!("unexpected bound result: {other:?}"),
            }
        }
        low
    };
    let indexed = minimum(true, 0);
    let repeated = minimum(false, 0);
    assert!(indexed < repeated, "indexed={indexed} repeated={repeated}");
    assert_eq!(minimum(true, 17), indexed + 17);
    run(true, indexed, 0).unwrap();
    assert!(matches!(run(true, indexed - 1, 0),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork, limit, ..
        }) if limit == indexed - 1));
}
