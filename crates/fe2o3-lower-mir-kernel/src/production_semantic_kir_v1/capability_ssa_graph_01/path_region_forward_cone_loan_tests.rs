#[test]
fn capability_path_forward_cone_production_chain_hits_and_alternating_starts_have_exact_costs() {
    let count = 1536usize;
    let edges: Vec<Vec<u32>> = (0..count)
        .map(|v| {
            if v + 1 < count {
                vec![v as u32 + 1]
            } else {
                vec![]
            }
        })
        .collect();
    let original = graph_body(&edges);
    let blocks = original
        .blocks()
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let mut identity = *block.identity().as_bytes();
            identity[..4].copy_from_slice(&u32::try_from(index).unwrap().to_be_bytes());
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(identity),
                block.source(),
                block.statements().to_vec(),
                block.terminator().clone(),
            )
            .unwrap()
        })
        .collect();
    let owner = indexed_owner(blocks);
    owner.verify_replay().unwrap();
    let source = endpoint_query(&owner);
    let vector_header = std::mem::size_of::<Vec<u32>>().div_ceil(std::mem::size_of::<usize>());
    for attached in [false, true] {
        let mut graph =
            CapabilitySsaGraphV1::new(source.function(), source.plan().plan(), 100_000).unwrap();
        if attached {
            graph = graph.with_definition_source(&source).unwrap();
        }
        let mut completed_from = None;
        let mut epoch = 0;
        let mut previous_geometry = None;
        for (rows, (from, to)) in [
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 2),
            (0, 4),
            (1, 3),
            (0, 5),
            (0, 6),
        ]
        .into_iter()
        .enumerate()
        {
            let included = to - from + 1;
            let forward = count - from;
            let hit = completed_from == Some(from);
            let cold = completed_from.is_none();
            let mut path = count + 5 + 2 * included - 1;
            if !hit {
                path += 3 + 2 * forward - 1;
                if cold {
                    path += cone_allocation_work(count) + count - 1;
                }
                epoch += 1;
            } else {
                let previous_warm_path = count + 3 + 2 * forward - 1 + 2 * included - 1;
                assert_eq!(previous_warm_path - path, 2 * forward - 3);
                if to == 2 {
                    assert_eq!((previous_warm_path, path), (4615, 1546));
                }
            }
            let geometry = if attached {
                let build = if cold {
                    2 + 2 * vector_header + 4 * count + 6 * count + 3 * (count - 1) + 2 * count + 4
                } else {
                    0
                };
                5 + 1 + count + included + build
            } else {
                let outgoing = (from..=to).filter(|&v| v + 1 < count).count();
                4 + 3 * count
                    + included
                    + 2 * outgoing
                    + usize::from(cold) * (2 * vector_header + 2 * count)
            };
            let required = endpoint_fixture_lookup(rows)
                + 2
                + path
                + geometry
                + endpoint_fixture_publication(rows);
            assert!(required < 100_000);
            graph.remaining = required;
            let actual = graph.loan_region(from as u32, to as u32).unwrap();
            assert_eq!(graph.remaining, 0);
            assert_eq!(
                actual.blocks,
                (0..count).map(|v| from <= v && v <= to).collect::<Vec<_>>()
            );
            assert!(actual.acyclic);
            assert_eq!(graph.reuse.loan_regions.len(), rows + 1);
            assert!(Arc::ptr_eq(
                &actual,
                graph
                    .reuse
                    .loan_regions
                    .get(&(from as u32, to as u32))
                    .unwrap()
            ));
            if let Some(previous) = &previous_geometry {
                assert!(!Arc::ptr_eq(previous, &actual));
            }
            previous_geometry = Some(actual);
            let scratch = graph.reuse.path_region_scratch.as_ref().unwrap();
            assert_eq!(scratch.epoch, epoch);
            assert_eq!(scratch.completed_from, Some(from as u32));
            assert!(scratch.pending.is_empty());
            assert_eq!(scratch.predecessors[0].capacity(), 0);
            assert!(
                scratch.predecessors[1..]
                    .iter()
                    .all(|row| row.capacity() == 1)
            );
            assert_eq!(graph.reuse.endpoint_scc.is_some(), attached);
            endpoint_assert_no_loan(&graph);
            completed_from = Some(from);
        }
    }
}

#[test]
fn capability_path_forward_cone_cold_chain_still_fits_original_twenty_thousand_limit() {
    let count = 1536usize;
    let edges: Vec<Vec<u32>> = (0..count)
        .map(|v| {
            if v + 1 < count {
                vec![v as u32 + 1]
            } else {
                vec![]
            }
        })
        .collect();
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 20_000).unwrap();
    let setup = query_reuse::header_words() + count + body.locals().len();
    let path = count + 8 + cone_allocation_work(count) + count + count - 1 + 3 + count - 1;
    let expected = setup + path;
    #[cfg(target_pointer_width = "64")]
    assert_eq!(expected, 18_495);
    assert!(expected < 20_000);
    assert_eq!(
        graph.path_region(0, 1).unwrap(),
        (0..count).map(|v| v <= 1).collect::<Vec<_>>()
    );
    assert_eq!(graph.remaining, 20_000 - expected);
    graph.remaining = count + 5 + 5;
    assert_eq!(
        graph.path_region(0, 2).unwrap(),
        (0..count).map(|v| v <= 2).collect::<Vec<_>>()
    );
    assert_eq!(graph.remaining, 0);
}

#[test]
fn capability_path_forward_cone_new_geometry_hit_prefixes_never_publish_partial_success() {
    let edges = [vec![1], vec![2], vec![]];
    let owner = endpoint_owner(&edges);
    let source = endpoint_query(&owner);
    for attached in [false, true] {
        let prepare = || {
            let mut graph =
                CapabilitySsaGraphV1::new(source.function(), source.plan().plan(), 100_000)
                    .unwrap();
            if attached {
                graph = graph.with_definition_source(&source).unwrap();
            }
            graph.loan_region(0, 1).unwrap();
            graph
        };
        let path = edges.len() + 5 + 5;
        let dispatch = endpoint_fixture_lookup(1) + 2 + 5 * usize::from(attached);
        let geometry = if attached {
            1 + edges.len() + 3
        } else {
            acyclic_scratch_fixture_cost(&edges, &[true; 3], false)
        };
        let required = dispatch + path + geometry + endpoint_fixture_publication(1);
        for available in 0..=required {
            let mut graph = prepare();
            let old = Arc::clone(graph.reuse.loan_regions.get(&(0, 1)).unwrap());
            graph.remaining = available;
            let result = graph.loan_region(0, 2);
            if available < required {
                cone_assert_work_error(result, 100_000);
                assert_eq!(graph.reuse.loan_regions.len(), 1);
                assert!(!graph.reuse.loan_regions.contains_key(&(0, 2)));
                assert_eq!(
                    graph.reuse.path_region_scratch.is_some(),
                    available < dispatch + edges.len() + 2 || available >= dispatch + path
                );
            } else {
                let actual = result.unwrap();
                assert_eq!(actual.blocks, [true; 3]);
                assert!(actual.acyclic);
                assert_eq!(graph.remaining, 0);
                assert_eq!(graph.reuse.loan_regions.len(), 2);
                assert!(!Arc::ptr_eq(&old, &actual));
            }
            assert!(Arc::ptr_eq(
                &old,
                graph.reuse.loan_regions.get(&(0, 1)).unwrap()
            ));
            endpoint_assert_no_loan(&graph);
        }
    }
}

fn cone_invalidation_body(invalidation: SemanticStatementV1) -> SemanticFunctionDeclV1 {
    body(vec![
        block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            ],
            Some(1),
        ),
        block(1, vec![invalidation], Some(2)),
        block(
            2,
            vec![assign(3, Some(SemanticOperandV1::Copy(place(2))))],
            None,
        ),
    ])
}

#[test]
fn capability_path_forward_cone_hits_keep_all_ordered_owner_invalidations() {
    for invalidation in [
        assign(3, Some(SemanticOperandV1::Move(place(1)))),
        assign(1, None),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
    ] {
        let body = cone_invalidation_body(invalidation);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let value = graph.use_value(0, 1).unwrap();
        let borrowed = loan(value, 1);
        graph
            .loan_live(borrowed, reuse_consumer(1, Some(0)))
            .unwrap();
        let epoch = graph.reuse.path_region_scratch.as_ref().unwrap().epoch;
        for consumer in [
            reuse_consumer(1, None),
            reuse_consumer(2, Some(0)),
            reuse_consumer(2, None),
        ] {
            let result = graph.loan_live(borrowed, consumer);
            assert!(result.is_err());
            reuse_assert_same(
                result,
                old.loan_live_before_path_scratch(borrowed, consumer),
            );
            assert!(!graph.reuse.loans.contains_key(&(borrowed, consumer)));
            assert_eq!(
                graph.reuse.path_region_scratch.as_ref().unwrap().epoch,
                epoch
            );
        }
        assert_eq!(graph.reuse.loan_regions.len(), 2);
    }
}

#[test]
fn capability_path_forward_cone_hits_cannot_promote_storage_observable_owners() {
    for invalidation in [
        statement(SemanticStatementKindV1::Deinitialize(place(1))),
        statement(SemanticStatementKindV1::SetDiscriminant {
            place: place(1),
            variant_index: 0,
        }),
    ] {
        let body = cone_invalidation_body(invalidation);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        graph.path_region(0, 1).unwrap();
        let epoch = graph.reuse.path_region_scratch.as_ref().unwrap().epoch;
        for to in [1, 2, 1] {
            assert_eq!(graph.path_region(0, to).unwrap(), [true, true, to == 2]);
            assert_eq!(
                graph.reuse.path_region_scratch.as_ref().unwrap().epoch,
                epoch
            );
            // These original operations make storage observable before loan
            // construction. A completed geometry cannot create a missing use.
            assert!(matches!(graph.use_value(0, 1),
                Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "capability reference has no exact SSA use"));
            assert!(graph.reuse.uses.is_empty());
            assert!(graph.reuse.loans.is_empty());
        }
        let index = graph.owner_statement_index(1).unwrap();
        assert!(
            graph
                .indexed_statement_invalidated(&index, 1, 0, 1)
                .unwrap()
        );
        assert!(
            !graph
                .indexed_statement_invalidated(&index, 1, 0, 0)
                .unwrap()
        );
        assert!(
            !graph
                .indexed_statement_invalidated(&index, 1, 1, 1)
                .unwrap()
        );
    }
}

#[test]
fn capability_path_forward_cone_hits_do_not_confer_other_owner_authority() {
    let body = body(vec![
        block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            ],
            Some(1),
        ),
        block(
            1,
            vec![
                assign(3, Some(SemanticOperandV1::Copy(place(2)))),
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(2),
                )),
            ],
            Some(2),
        ),
        block(2, vec![statement(SemanticStatementKindV1::Nop)], None),
    ]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let first = graph.use_value(0, 1).unwrap();
    let second = graph.use_value(1, 2).unwrap();
    graph
        .loan_live(loan(first, 1), reuse_consumer(1, None))
        .unwrap();
    graph
        .loan_live(loan(first, 1), reuse_consumer(2, None))
        .unwrap();
    assert_eq!(graph.reuse.path_region_scratch.as_ref().unwrap().epoch, 1);
    let other = CapabilityLoanV1 {
        borrow: CapabilityDefinitionSiteV1 {
            block: 1,
            statement: Some(0),
            local: 3,
        },
        owner_local: 2,
        owner_value: second,
    };
    for consumer in [
        reuse_consumer(1, Some(1)),
        reuse_consumer(1, None),
        reuse_consumer(2, None),
    ] {
        let result = graph.loan_live(other, consumer);
        assert_eq!(result.is_ok(), consumer.statement.is_some());
        reuse_assert_same(result, old.loan_live_before_path_scratch(other, consumer));
    }
    let wrong_owner = CapabilityLoanV1 {
        owner_value: first,
        ..other
    };
    assert!(matches!(
        graph.loan_live(wrong_owner, reuse_consumer(2, None)),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(
        !graph
            .reuse
            .loans
            .contains_key(&(other, reuse_consumer(2, None)))
    );
}

#[test]
fn capability_path_forward_cone_hits_keep_call_drop_unwind_and_cycle_rejections() {
    let edges = [vec![1], vec![2, 3], vec![], vec![]];
    let scaffold = reuse_body(&edges);
    let scaffold_plan = plan(&scaffold);
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let call = |operand, destination| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand],
                Some(SemanticCallDestinationV1::new(
                    place(destination),
                    edge(SemanticEdgeRoleV1::CallReturn, 2),
                )),
                SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 3)),
            )
            .unwrap(),
        )
    };
    for (terminator, invalid) in [
        (call(SemanticOperandV1::Move(place(1)), 3), true),
        (call(SemanticOperandV1::Copy(place(1)), 1), true),
        (call(SemanticOperandV1::Copy(place(1)), 3), false),
        (
            SemanticTerminatorKindV1::Drop {
                place: place(1),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: edge(SemanticEdgeRoleV1::DropReturn, 2),
                unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::DropUnwind, 3)),
            },
            true,
        ),
    ] {
        let mut blocks = scaffold.blocks().to_vec();
        blocks[1] = SemanticBasicBlockV1::new(
            blocks[1].identity(),
            blocks[1].source(),
            blocks[1].statements().to_vec(),
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap();
        let body = body(blocks);
        let mut graph = CapabilitySsaGraphV1::new(&body, scaffold_plan.plan(), 100_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, scaffold_plan.plan(), 100_000).unwrap();
        let value = graph.use_value(0, 1).unwrap();
        graph
            .loan_live(loan(value, 1), reuse_consumer(1, Some(0)))
            .unwrap();
        let epoch = graph.reuse.path_region_scratch.as_ref().unwrap().epoch;
        for consumer in [
            reuse_consumer(1, None),
            reuse_consumer(2, None),
            reuse_consumer(3, None),
        ] {
            let result = graph.loan_live(loan(value, 1), consumer);
            assert_eq!(result.is_err(), invalid);
            reuse_assert_same(
                result,
                old.loan_live_before_path_scratch(loan(value, 1), consumer),
            );
            assert_eq!(
                graph.reuse.path_region_scratch.as_ref().unwrap().epoch,
                epoch
            );
            if invalid {
                assert!(!graph.reuse.loans.contains_key(&(loan(value, 1), consumer)));
            }
        }
        assert_eq!(graph.reuse.loan_regions.len(), 3);
    }
    for edges in [
        vec![vec![1], vec![1, 2], vec![]],
        vec![vec![1], vec![0, 2], vec![]],
        vec![vec![1], vec![2], vec![1]],
    ] {
        let body = reuse_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let value = graph.use_value(0, 1).unwrap();
        graph.loan_region(0, 1).unwrap();
        for target in [2, 1, 2] {
            let consumer = reuse_consumer(target, None);
            let result = graph.loan_live(loan(value, 1), consumer);
            assert!(
                matches!(&result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if *detail == "capability loan crosses a cycle without a proven storage generation")
            );
            reuse_assert_same(
                result,
                old.loan_live_before_path_scratch(loan(value, 1), consumer),
            );
            assert_eq!(graph.reuse.path_region_scratch.as_ref().unwrap().epoch, 1);
            assert!(graph.reuse.loans.is_empty());
        }
    }
}
