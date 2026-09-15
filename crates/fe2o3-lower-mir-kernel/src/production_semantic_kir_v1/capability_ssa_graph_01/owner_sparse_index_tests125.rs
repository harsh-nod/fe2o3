#[test]
fn capability_owner_sparse_many_empty_blocks_retain_compressed_offset_storage125() {
    for populated in [false, true] {
        let mut fixed_work = None;
        for block_count in [1usize, 8, 64, 128] {
            let body = body(
                (0..block_count)
                    .map(|b| {
                        block(
                            b as u8,
                            if populated && b == block_count / 2 {
                                vec![assign(1, None)]
                            } else {
                                vec![]
                            },
                            (b + 1 < block_count).then_some((b + 1) as u32),
                        )
                    })
                    .collect(),
            );
            let plan = plan(&body);
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let work = owner_index_expected_build_cost(&body, 1, 0);
            graph.remaining = work;
            let index = graph.owner_statement_index(1).unwrap();
            assert_eq!(graph.remaining, 0);
            owner_index_assert_layout(&index, &body, 1);
            assert_eq!(index.positions.len(), usize::from(populated));
            assert_eq!(index.positions.capacity(), if populated { 4 } else { 0 });
            assert_eq!(
                std::mem::size_of_val(index.as_ref()),
                6 * std::mem::size_of::<usize>()
            );
            let storage_words = 6 + 2 + index.offsets.capacity() + index.positions.capacity();
            assert_eq!(
                storage_words,
                block_count + 1 + 8 + if populated { 4 } else { 0 }
            );
            // Each empty block adds one offset word and one visit, with no row allocation.
            assert_eq!(
                work - 2 * block_count,
                *fixed_work.get_or_insert(work - 2 * block_count)
            );
            assert!(graph.reuse.loans.is_empty());
        }
    }
}

#[test]
fn capability_owner_sparse_cross_block_and_empty_windows_are_precharged125() {
    for populated in [false, true] {
        let positions = if populated {
            vec![(1, 3), (3, 0), (3, 2), (4, 1)]
        } else {
            vec![]
        };
        let body = body(
            [4usize, 4, 3, 3, 2, 2]
                .into_iter()
                .enumerate()
                .map(|(b, len)| {
                    block(
                        b as u8,
                        (0..len)
                            .map(|s| {
                                if positions.contains(&(b, s)) {
                                    assign(1, None)
                                } else {
                                    statement(SemanticStatementKindV1::Nop)
                                }
                            })
                            .collect(),
                        (b < 5).then_some((b + 1) as u32),
                    )
                })
                .collect(),
        );
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let index = graph.owner_statement_index(1).unwrap();
        owner_index_assert_layout(&index, &body, 1);
        if populated {
            assert_eq!(index.offsets, [0, 0, 1, 1, 3, 4, 4]);
            assert_eq!(index.positions, [3, 0, 2, 1]);
        } else {
            assert_eq!(index.offsets, [0; 7]);
            assert!(index.positions.is_empty());
        }
        assert_eq!(index.positions.capacity(), if populated { 4 } else { 0 });
        for (b, block) in body.blocks().iter().enumerate() {
            let row_len = block
                .statements()
                .iter()
                .filter(|statement| reference_invalidates(statement.kind(), 1))
                .count();
            for first in [0, 1, 2, 3, 4, 5, usize::MAX] {
                for end in [0, 1, 2, 3, 4, 5, usize::MAX] {
                    let expected = block.statements().iter().enumerate().any(|(s, statement)| {
                        first <= s && s < end && reference_invalidates(statement.kind(), 1)
                    });
                    let cost = if first >= end || row_len == 0 {
                        2
                    } else {
                        2 + 2 + (usize::BITS - row_len.leading_zeros()) as usize
                    };
                    for available in 0..cost {
                        graph.remaining = available;
                        assert!(matches!(
                            graph.indexed_statement_invalidated(&index, b as u32, first, end),
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
                            .indexed_statement_invalidated(&index, b as u32, first, end)
                            .unwrap(),
                        expected,
                        "block={b}, first={first}, end={end}, populated={populated}"
                    );
                    assert_eq!(graph.remaining, 0);
                }
            }
        }
        for b in [body.blocks().len() as u32, u32::MAX] {
            for (first, end) in [(0, 0), (1, 0), (usize::MAX, usize::MAX), (0, usize::MAX)] {
                for available in 0..2 {
                    graph.remaining = available;
                    assert!(matches!(
                        graph.indexed_statement_invalidated(&index, b, first, end),
                        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
                    ));
                    assert_eq!(graph.remaining, available);
                }
                graph.remaining = 2;
                assert!(matches!(
                    graph.indexed_statement_invalidated(&index, b, first, end),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert_eq!(graph.remaining, 0);
            }
        }
        assert_eq!(graph.reuse.owner_invalidations.len(), 1);
        assert!(graph.reuse.loans.is_empty());
    }
}

#[test]
fn capability_owner_sparse_capacity_boundaries_preserve_every_budget_prefix125() {
    for count in [0usize, 1, 3, 4, 5, 7, 8, 9, 15, 16, 17] {
        let body = body(
            (0..3)
                .map(|b| {
                    block(
                        b as u8,
                        (b..count)
                            .step_by(3)
                            .flat_map(|_| {
                                [statement(SemanticStatementKindV1::Nop), assign(1, None)]
                            })
                            .collect(),
                        (b < 2).then_some((b + 1) as u32),
                    )
                })
                .collect(),
        );
        let plan = plan(&body);
        for occupied in [false, true] {
            let prepare = || {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
                let retained = occupied.then(|| graph.owner_statement_index(3).unwrap());
                (graph, retained)
            };
            let rows = usize::from(occupied);
            let charges = owner_index_expected_build_charges(&body, 1, rows);
            let expected: usize = charges.iter().sum();
            for available in 0..expected {
                let (mut graph, retained) = prepare();
                graph.remaining = available;
                let mut remaining = available;
                for charge in &charges {
                    let Some(next) = remaining.checked_sub(*charge) else {
                        break;
                    };
                    remaining = next;
                }
                assert!(matches!(
                    graph.owner_statement_index(1),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
                ));
                assert_eq!(
                    graph.remaining, remaining,
                    "count={count}, available={available}, occupied={occupied}"
                );
                assert_eq!(graph.reuse.owner_invalidations.len(), rows);
                assert!(!graph.reuse.owner_invalidations.contains_key(&1));
                if let Some(retained) = retained {
                    assert!(Arc::ptr_eq(&retained, &graph.reuse.owner_invalidations[&3]));
                }
                assert!(graph.reuse.loans.is_empty());
            }
            let (mut graph, _) = prepare();
            graph.remaining = expected;
            let index = graph.owner_statement_index(1).unwrap();
            assert_eq!(graph.remaining, 0);
            owner_index_assert_layout(&index, &body, 1);
            assert_eq!(index.positions.len(), count);
            assert_eq!(
                index.positions.capacity(),
                owner_index_expected_capacity(count)
            );
            assert_eq!(graph.reuse.owner_invalidations.len(), rows + 1);
            let first_row_len = count.div_ceil(3);
            let query_cost = if first_row_len == 0 {
                2
            } else {
                2 + 2 + (usize::BITS - first_row_len.leading_zeros()) as usize
            };
            for available in 0..query_cost {
                graph.remaining = available;
                assert!(matches!(
                    graph.indexed_statement_invalidated(&index, 0, 0, usize::MAX),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
                ));
                assert_eq!(
                    graph.remaining,
                    available.checked_sub(2).unwrap_or(available)
                );
            }
            graph.remaining = query_cost;
            assert_eq!(
                graph
                    .indexed_statement_invalidated(&index, 0, 0, usize::MAX)
                    .unwrap(),
                count != 0
            );
            assert_eq!(graph.remaining, 0);
            assert!(graph.reuse.loans.is_empty());
        }
    }
}

// Recreate the old allocations with the independent predicate. Work includes
// the original full scan and publication; storage counts Arc and vector words.
fn owner_sparse_old_layout_measurement125(body: &SemanticFunctionDeclV1) -> (usize, usize) {
    let vector_words = std::mem::size_of::<Vec<usize>>().div_ceil(std::mem::size_of::<usize>());
    let mut by_block = Vec::with_capacity(body.blocks().len());
    let mut work = lookup_work(0) + 1 + vector_words + by_block.capacity() * vector_words;
    for block in body.blocks() {
        let mut positions = Vec::with_capacity(block.statements().len());
        work += 1 + positions.capacity() + block.statements().len();
        for (s, statement) in block.statements().iter().enumerate() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if assignment.destination().local().index() != 1 {
                    if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                        work += aggregate.operands().len();
                    }
                }
            }
            if reference_invalidates(statement.kind(), 1) {
                positions.push(s);
            }
        }
        by_block.push(positions);
    }
    work += insertion_work::<u32, Arc<Vec<Vec<usize>>>>(0) + vector_words + 2;
    let storage = vector_words
        + 2
        + by_block.capacity() * vector_words
        + by_block.iter().map(Vec::capacity).sum::<usize>();
    (storage, work)
}

#[test]
fn capability_owner_sparse_body_reduces_measured_dense_storage_and_build_work125() {
    let body = body(
        (0..64)
            .map(|b| {
                block(
                    b as u8,
                    (0..32)
                        .map(|s| {
                            if s == 0 && [0, 31, 63].contains(&b) {
                                assign(1, None)
                            } else {
                                statement(SemanticStatementKindV1::Nop)
                            }
                        })
                        .collect(),
                    (b < 63).then_some(b + 1),
                )
            })
            .collect(),
    );
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let (old_storage, old_work) = owner_sparse_old_layout_measurement125(&body);
    let before = graph.remaining;
    let index = graph.owner_statement_index(1).unwrap();
    let new_work = before - graph.remaining;
    let new_storage = std::mem::size_of_val(index.as_ref()).div_ceil(std::mem::size_of::<usize>())
        + 2
        + index.offsets.capacity()
        + index.positions.capacity();
    owner_index_assert_layout(&index, &body, 1);
    assert_eq!(index.offsets.capacity(), 65);
    assert_eq!(index.positions, [0, 0, 0]);
    assert_eq!(index.positions.capacity(), 4);
    assert_eq!(new_work, owner_index_expected_build_cost(&body, 1, 0));
    assert_eq!((old_storage, new_storage), (2245, 77));
    // This sparse fixture benefits; dense invalidation streams can cost more.
    assert!(
        new_work < old_work,
        "old_work={old_work}, new_work={new_work}"
    );
    assert!(graph.reuse.loans.is_empty());
}
