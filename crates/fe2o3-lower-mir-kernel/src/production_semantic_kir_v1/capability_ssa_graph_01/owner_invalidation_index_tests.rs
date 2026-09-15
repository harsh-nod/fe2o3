#[test]
fn capability_owner_index_endpoint_extremes_match_graph87() {
    let body = aggregate_budget_body(8, Some(7), 3);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    for start in [None, Some(0), Some(1), Some(2), Some(4), Some(u32::MAX)] {
        for end in [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(u32::MAX),
        ] {
            let mut borrowed = loan(owner, 1);
            borrowed.borrow.statement = start;
            let consumer = reuse_consumer(0, end);
            reuse_assert_same(
                graph.loan_live(borrowed, consumer),
                reference.loan_live_before_owner_index(borrowed, consumer),
            );
        }
    }
}

#[test]
fn capability_owner_index_keeps_branch_invalidations_and_dead_cycles() {
    for edges in [
        vec![vec![1, 2], vec![3], vec![3], vec![]],
        vec![vec![1, 2], vec![], vec![2], vec![]],
    ] {
        let original = reuse_body(&edges);
        let mut blocks = original.blocks().to_vec();
        blocks[2] = SemanticBasicBlockV1::new(
            blocks[2].identity(),
            blocks[2].source(),
            vec![statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(1),
            ))],
            blocks[2].terminator().clone(),
        )
        .unwrap();
        let body = body(blocks);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        graph
            .loan_live(loan(owner, 1), reuse_consumer(1, None))
            .unwrap();
        let index = graph.owner_statement_index(1).unwrap();
        owner_index_assert_layout(&index, &body, 1);
        assert_eq!(index.offsets, [0, 1, 1, 2, 2]);
        assert_eq!(index.positions, [0, 0]);
        for target in [1, 2, 3] {
            reuse_assert_same(
                graph.loan_live(loan(owner, 1), reuse_consumer(target, None)),
                reference
                    .loan_live_before_owner_index(loan(owner, 1), reuse_consumer(target, None)),
            );
        }
    }
}

#[test]
fn capability_owner_index_full_loan_debits_match_graph87_delta_and_fail_closed() {
    for operands in [0, 1, 8, 257] {
        for warm in [false, true] {
            let body = aggregate_budget_body(operands, None, 3);
            let plan = plan(&body);
            let borrowed = {
                let mut seed = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
                loan(seed.use_value(0, 1).unwrap(), 1)
            };
            let short_window = reuse_consumer(0, Some(2));
            let consumer = reuse_consumer(0, Some(3));
            let prepare = || {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
                assert_eq!(graph.use_value(0, 1).unwrap(), borrowed.owner_value);
                if warm {
                    graph.loan_live(borrowed, short_window).unwrap();
                }
                graph
            };
            let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            reference.use_value(0, 1).unwrap();
            if warm {
                reference
                    .loan_live_before_owner_index(borrowed, short_window)
                    .unwrap();
            }
            let before = reference.remaining;
            reference
                .loan_live_before_owner_index(borrowed, consumer)
                .unwrap();
            let old_work = before - reference.remaining;
            let index_work = if warm {
                lookup_work(1) + 1
            } else {
                owner_index_expected_build_cost(&body, 1, 0)
            };
            // Graph87 scans four statements plus N aggregate operands. The new
            // benign row contains only position zero, so its window query costs 5.
            let expected = old_work - 4 - operands + index_work + 5;
            for available in 0..expected {
                let mut graph = prepare();
                graph.remaining = available;
                assert!(matches!(
                    graph.loan_live(borrowed, consumer),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
                ));
                assert!(graph.remaining <= available);
                assert_eq!(graph.reuse.loans.len(), usize::from(warm));
                assert!(!graph.reuse.loans.contains_key(&(borrowed, consumer)));
            }
            let mut graph = prepare();
            graph.remaining = expected;
            graph.loan_live(borrowed, consumer).unwrap();
            assert_eq!(graph.remaining, 0);
        }
    }
}

fn owner_index_expected_publication_cost(rows: usize) -> usize {
    insertion_work::<u32, Arc<CapabilityOwnerInvalidationsV1>>(rows)
        + 2 * std::mem::size_of::<Vec<usize>>().div_ceil(std::mem::size_of::<usize>())
        + 2
}

// Count reference hits and growth without calling the production predicate or
// observing its debits. Exact allocator capacities are asserted separately.
fn owner_index_expected_build_charges(
    body: &SemanticFunctionDeclV1,
    owner: u32,
    rows: usize,
) -> Vec<usize> {
    let header = std::mem::size_of::<Vec<usize>>().div_ceil(std::mem::size_of::<usize>());
    let mut charges = vec![lookup_work(rows) + 1, 2 * header, body.blocks().len() + 1, 0];
    let mut count = 0usize;
    let mut capacity = 0usize;
    for block in body.blocks() {
        charges.extend([1, block.statements().len()]);
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if assignment.destination().local().index() != owner {
                    if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                        charges.push(aggregate.operands().len());
                    }
                }
            }
            if reference_invalidates(statement.kind(), owner) {
                charges.push(2);
                if count == capacity {
                    capacity = capacity.checked_mul(2).unwrap().max(4);
                    charges.extend([capacity + count, 0]);
                }
                count += 1;
            }
        }
    }
    charges.push(owner_index_expected_publication_cost(rows));
    charges
}

fn owner_index_expected_build_cost(
    body: &SemanticFunctionDeclV1,
    owner: u32,
    rows: usize,
) -> usize {
    owner_index_expected_build_charges(body, owner, rows)
        .into_iter()
        .sum()
}

fn owner_index_expected_capacity(count: usize) -> usize {
    if count == 0 {
        0
    } else {
        count.next_power_of_two().max(4)
    }
}

fn owner_index_expected_positions(
    body: &SemanticFunctionDeclV1,
    owner: u32,
) -> Vec<usize> {
    body.blocks()
        .iter()
        .flat_map(|block| {
            block
                .statements()
                .iter()
                .enumerate()
                .filter_map(move |(position, statement)| {
                    reference_invalidates(statement.kind(), owner).then_some(position)
                })
        })
        .collect()
}

fn owner_index_assert_layout(
    index: &CapabilityOwnerInvalidationsV1,
    body: &SemanticFunctionDeclV1,
    owner: u32,
) {
    assert_eq!(index.offsets.len(), body.blocks().len() + 1);
    assert_eq!(index.offsets.capacity(), body.blocks().len() + 1);
    assert_eq!(index.offsets.first(), Some(&0));
    assert_eq!(index.offsets.last(), Some(&index.positions.len()));
    assert!(index.offsets.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(index.positions, owner_index_expected_positions(body, owner));
    assert_eq!(
        index.positions.capacity(),
        owner_index_expected_capacity(index.positions.len())
    );
    let mut covered = 0;
    for (b, block) in body.blocks().iter().enumerate() {
        let expected: Vec<_> = block
            .statements()
            .iter()
            .enumerate()
            .filter_map(|(s, statement)| {
                reference_invalidates(statement.kind(), owner).then_some(s)
            })
            .collect();
        let row = index.row(b).unwrap();
        assert_eq!(row, expected.as_slice());
        assert!(row.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(index.offsets[b], covered);
        covered += row.len();
        assert_eq!(index.offsets[b + 1], covered);
    }
    assert_eq!(covered, index.positions.len());
    assert!(index.row(body.blocks().len()).is_none());
    assert!(index.row(usize::MAX).is_none());
}

#[test]
fn capability_owner_index_matches_independent_predicate_and_all_windows() {
    let statements: Vec<_> = reuse_invalidation_rows()
        .into_iter()
        .map(|(kind, _)| statement(kind))
        .collect();
    let scaffold = graph_body(&[vec![], vec![]]);
    let plan = plan(&scaffold);
    let body = body(vec![
        block(0, statements, None),
        block(
            1,
            vec![statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            ))],
            None,
        ),
    ]);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    for owner in 0..4 {
        let rows_before = graph.reuse.owner_invalidations.len();
        let before = graph.remaining;
        let index = graph.owner_statement_index(owner).unwrap();
        assert_eq!(
            before - graph.remaining,
            owner_index_expected_build_cost(&body, owner, rows_before)
        );
        owner_index_assert_layout(&index, &body, owner);
        for (b, block) in body.blocks().iter().enumerate() {
            let source = block.statements();
            for first in 0..=source.len() + 1 {
                for end in 0..=source.len() + 1 {
                    let expected = source.iter().enumerate().any(|(i, s)| {
                        first <= i && i < end && reference_invalidates(s.kind(), owner)
                    });
                    assert_eq!(
                        graph
                            .indexed_statement_invalidated(&index, b as u32, first, end)
                            .unwrap(),
                        expected
                    );
                }
            }
            assert!(
                !graph
                    .indexed_statement_invalidated(&index, b as u32, usize::MAX, usize::MAX)
                    .unwrap()
            );
        }
    }
    assert!(graph.reuse.loans.is_empty());
}

#[test]
fn capability_owner_index_full_loan_matches_graph87_on_seeded_cfgs() {
    let mut seed = 0x8911_76a3_u32;
    for _ in 0..48 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 4).map(|_| next() % 8).collect())
            .collect();
        let original = reuse_body(&edges);
        let mut blocks = original.blocks().to_vec();
        for b in 1..blocks.len() {
            let mut statements = blocks[b].statements().to_vec();
            statements.push(if next() % 3 == 0 {
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(1),
                ))
            } else {
                statement(SemanticStatementKindV1::Nop)
            });
            blocks[b] = SemanticBasicBlockV1::new(
                blocks[b].identity(),
                blocks[b].source(),
                statements,
                blocks[b].terminator().clone(),
            )
            .unwrap();
        }
        let body = body(blocks);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        for target in 0..8 {
            for end in [Some(0), Some(1), Some(2), None] {
                for _ in 0..2 {
                    let consumer = reuse_consumer(target, end);
                    reuse_assert_same(
                        graph.loan_live(loan(owner, 1), consumer),
                        reference.loan_live_before_owner_index(loan(owner, 1), consumer),
                    );
                }
            }
        }
        assert!(reference.reuse.owner_invalidations.is_empty());
    }
}

#[test]
fn capability_owner_index_cold_budget_prefixes_never_publish_partial_indexes() {
    let body = aggregate_budget_body(8, Some(7), 3);
    let plan = plan(&body);
    let expected = owner_index_expected_build_cost(&body, 1, 0);
    for available in 0..expected {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        graph.remaining = available;
        for _ in 0..2 {
            let before = graph.remaining;
            assert!(matches!(
                graph.owner_statement_index(1),
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
            assert!(graph.remaining <= before);
            assert!(graph.reuse.owner_invalidations.is_empty());
            assert!(graph.reuse.loans.is_empty());
        }
    }
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    graph.remaining = expected;
    let index = graph.owner_statement_index(1).unwrap();
    assert_eq!(graph.remaining, 0);
    owner_index_assert_layout(&index, &body, 1);
}

#[test]
fn capability_owner_index_warm_lookup_and_window_queries_are_precharged() {
    let body = aggregate_budget_body(8, Some(7), 3);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let index = graph.owner_statement_index(1).unwrap();
    for available in 0..lookup_work(1) + 1 {
        graph.remaining = available;
        assert!(matches!(
            graph.owner_statement_index(1),
            Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
        ));
        assert_eq!(graph.remaining, available);
    }
    // Two sorted positions: two offset reads and four scalar comparisons.
    // Empty windows require only the offset reads.
    for (first, end, cost, expected) in [(2, 3, 6, true), (3, 2, 2, false)] {
        for available in 0..cost {
            graph.remaining = available;
            assert!(matches!(
                graph.indexed_statement_invalidated(&index, 0, first, end),
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
            assert_eq!(
                graph.remaining,
                available.checked_sub(2).unwrap_or(available)
            );
        }
        graph.remaining = cost;
        assert_eq!(
            graph
                .indexed_statement_invalidated(&index, 0, first, end)
                .unwrap(),
            expected
        );
        assert_eq!(graph.remaining, 0);
    }
    assert_eq!(graph.reuse.owner_invalidations.len(), 1);
    assert!(graph.reuse.loans.is_empty());
}

#[test]
fn capability_owner_index_is_local_to_exact_body_and_does_not_replace_owner_ssa() {
    let first = aggregate_budget_body(8, Some(7), 3);
    let second = aggregate_budget_body(8, None, 3);
    assert_eq!(first.identity(), second.identity());
    let first_plan = plan(&first);
    let second_plan = plan(&second);
    let mut a = CapabilitySsaGraphV1::new(&first, first_plan.plan(), 100_000).unwrap();
    let mut b = CapabilitySsaGraphV1::new(&second, second_plan.plan(), 100_000).unwrap();
    let left = a.owner_statement_index(1).unwrap();
    assert!(b.reuse.owner_invalidations.is_empty());
    let right = b.owner_statement_index(1).unwrap();
    assert!(!Arc::ptr_eq(&left, &right));
    owner_index_assert_layout(&left, &first, 1);
    owner_index_assert_layout(&right, &second, 1);
    assert_eq!(left.offsets, [0, 2]);
    assert_eq!(right.offsets, [0, 1]);
    assert_eq!(left.positions, [0, 2]);
    assert_eq!(right.positions, [0]);
    let other_owner = a.owner_statement_index(2).unwrap();
    owner_index_assert_layout(&other_owner, &first, 2);
    assert!(!Arc::ptr_eq(&left, &other_owner));
    let mut fresh = CapabilitySsaGraphV1::new(&first, first_plan.plan(), 100_000).unwrap();
    let wrong = loan(
        SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(999)),
        1,
    );
    assert!(matches!(
        fresh.loan_live(wrong, reuse_consumer(0, Some(3))),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(fresh.reuse.owner_invalidations.is_empty());
    assert!(fresh.reuse.loans.is_empty());
}

#[test]
fn capability_owner_index_rejected_loan_retains_data_not_authority() {
    let body = aggregate_budget_body(8, Some(7), 3);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    let bad = reuse_consumer(0, Some(3));
    assert!(matches!(
        graph.loan_live(loan(owner, 1), bad),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    assert_eq!(graph.reuse.owner_invalidations.len(), 1);
    assert!(graph.reuse.loans.is_empty());
    let before = graph.owner_statement_index(1).unwrap();
    graph
        .loan_live(loan(owner, 1), reuse_consumer(0, Some(2)))
        .unwrap();
    assert!(Arc::ptr_eq(
        &before,
        &graph.owner_statement_index(1).unwrap()
    ));
    assert!(!graph.reuse.loans.contains_key(&(loan(owner, 1), bad)));
}

#[test]
fn capability_owner_index_final_loan_commit_still_requires_budget() {
    let body = aggregate_budget_body(8, None, 3);
    let plan = plan(&body);
    let mut measured = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let owner = measured.use_value(0, 1).unwrap();
    let consumer = reuse_consumer(0, Some(3));
    measured.loan_live(loan(owner, 1), consumer).unwrap();
    let required = measured.limit - measured.remaining;
    let mut short = CapabilitySsaGraphV1::new(&body, plan.plan(), required - 1).unwrap();
    let owner = short.use_value(0, 1).unwrap();
    assert!(matches!(
        short.loan_live(loan(owner, 1), consumer),
        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
    ));
    assert!(short.reuse.loans.is_empty());
    assert_eq!(short.reuse.owner_invalidations.len(), 1);
    let mut exact = CapabilitySsaGraphV1::new(&body, plan.plan(), required).unwrap();
    let owner = exact.use_value(0, 1).unwrap();
    exact.loan_live(loan(owner, 1), consumer).unwrap();
    assert_eq!(exact.remaining, 0);
}

#[test]
fn capability_owner_index_many_windows_amortize_full_statement_scans() {
    let edges: Vec<Vec<u32>> = (0..12)
        .map(|b| if b == 11 { vec![] } else { vec![b + 1] })
        .collect();
    let original = reuse_body(&edges);
    let blocks = original
        .blocks()
        .iter()
        .enumerate()
        .map(|(b, source)| {
            let mut statements = if b == 0 {
                source.statements().to_vec()
            } else {
                Vec::new()
            };
            statements.extend((0..64).map(|_| statement(SemanticStatementKindV1::Nop)));
            SemanticBasicBlockV1::new(
                source.identity(),
                source.source(),
                statements,
                source.terminator().clone(),
            )
            .unwrap()
        })
        .collect();
    let body = body(blocks);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    assert_eq!(owner, reference.use_value(0, 1).unwrap());
    for end in 32..56 {
        let consumer = reuse_consumer(11, Some(end));
        graph.loan_live(loan(owner, 1), consumer).unwrap();
        reference
            .loan_live_before_owner_index(loan(owner, 1), consumer)
            .unwrap();
    }
    let indexed = graph.limit - graph.remaining;
    let scanned = reference.limit - reference.remaining;
    assert_eq!(graph.reuse.loans.len(), 24);
    assert_eq!(graph.reuse.owner_invalidations.len(), 1);
    assert!(reference.reuse.owner_invalidations.is_empty());
    assert!(
        indexed < scanned / 2,
        "indexed={indexed}, scanned={scanned}"
    );
}
