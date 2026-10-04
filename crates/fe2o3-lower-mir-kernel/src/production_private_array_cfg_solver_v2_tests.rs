use super::*;

#[test]
fn private_array_assert_sink_decoder_has_independent_exact_work_and_no_byte_allocation() {
    let mut block = BasicBlock::new(BlockId(3));
    block
        .operations
        .push(AmdGpuDiagnosticOperation::Trap.operation(None));
    block.terminator = Some(Terminator::Unreachable);
    let OperationKind::Call { callee, .. } = &block.operations[0].kind else {
        panic!("trap call");
    };
    // Eight structural checks, then eight complete fixed descriptor comparisons.
    let exact = 8 + 8 * (callee.as_str().len() + 2);
    for limit in [exact, exact - 1] {
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
        let result = check_assert_failure_sink(
            &block,
            &mut Work {
                budget: &mut budget,
            },
        );
        if limit == exact {
            assert!(result.is_ok());
            assert_eq!(budget.remaining, 0);
        } else {
            assert!(matches!(result, Err(Error::ResourceLimit)));
        }
    }
}

// This is a concrete finite-product path interpreter, not the production
// may-uninitialized propagation algorithm. Both initial states at a join are
// retained independently, so a single uninitialized path remains observable.
fn concrete(cfg: &Cfg, transfer: &[Transfer]) -> (Vec<bool>, Vec<bool>) {
    let mut seen = vec![[false; 2]; transfer.len()];
    let mut pending = vec![(cfg.entry, false)];
    let mut incoming = vec![false; transfer.len()];
    let mut outgoing = incoming.clone();
    while let Some((block, initialized)) = pending.pop() {
        if seen[block][usize::from(initialized)] {
            continue;
        }
        seen[block][usize::from(initialized)] = true;
        incoming[block] |= !initialized;
        let initialized = match transfer[block] {
            Transfer::Identity => initialized,
            Transfer::Initialized => true,
            Transfer::Uninitialized => false,
        };
        outgoing[block] |= !initialized;
        for edge in &cfg.edges {
            if edge[0] == block {
                pending.push((edge[1], initialized));
            }
        }
    }
    (incoming, outgoing)
}

fn graph(count: usize, mut edges: Vec<[usize; 2]>, entry: usize) -> Cfg {
    edges.sort_unstable();
    // Deliberately retain duplicate edges: the production solve is robust even
    // before the source-normalization step removes them.
    let mut ranges = vec![(0, 0); count];
    let mut cursor = 0;
    for (block, range) in ranges.iter_mut().enumerate() {
        let start = cursor;
        while edges.get(cursor).is_some_and(|edge| edge[0] == block) {
            cursor += 1;
        }
        *range = (start, cursor);
    }
    let mut reachable = vec![false; count];
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if std::mem::replace(&mut reachable[block], true) {
            continue;
        }
        for edge in &edges {
            if edge[0] == block {
                pending.push(edge[1]);
            }
        }
    }
    Cfg {
        edges,
        ranges,
        reachable,
        entry,
    }
}

fn compare(cfg: &Cfg, transfer: &[Transfer]) {
    let expected = concrete(cfg, transfer);
    let count = transfer.len();
    let mut actual_in = vec![true; count];
    let mut actual_out = vec![true; count];
    let mut queue = Vec::with_capacity(count);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 {
        remaining: usize::MAX,
    };
    solve(
        cfg,
        transfer,
        &mut actual_in,
        &mut actual_out,
        &mut queue,
        &mut Work {
            budget: &mut budget,
        },
    )
    .unwrap();
    assert_eq!((actual_in, actual_out), expected);
    assert!(queue.len() <= count);
    let mut unique = queue.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), queue.len());
}

#[test]
fn private_array_cfg_actual_solver_matches_all_small_concrete_path_states() {
    let mut cases = 0usize;
    for count in 1..=3usize {
        for mask in 0..(1usize << (count * count)) {
            let edges = (0..count * count)
                .filter(|bit| mask & (1 << bit) != 0)
                .map(|bit| [bit / count, bit % count])
                .collect::<Vec<_>>();
            for encoding in 0..3usize.pow(count as u32) {
                let mut digits = encoding;
                let transfer = (0..count)
                    .map(|_| {
                        let value = match digits % 3 {
                            0 => Transfer::Identity,
                            1 => Transfer::Initialized,
                            _ => Transfer::Uninitialized,
                        };
                        digits /= 3;
                        value
                    })
                    .collect::<Vec<_>>();
                for entry in 0..count {
                    compare(&graph(count, edges.clone(), entry), &transfer);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 41_766);
}

#[test]
fn private_array_cfg_actual_solver_matches_seeded_duplicate_edge_and_backedge_paths() {
    let mut state = 0x1f3e2a17u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    for _ in 0..10_000 {
        let count = 4 + next() as usize % 9;
        let entry = next() as usize % count;
        let mut edges = Vec::new();
        for from in 0..count {
            for to in 0..count {
                if next() % 5 == 0 {
                    edges.push([from, to]);
                    if next() % 3 == 0 {
                        edges.push([from, to]);
                    }
                }
            }
        }
        let transfer = (0..count)
            .map(|_| match next() % 3 {
                0 => Transfer::Identity,
                1 => Transfer::Initialized,
                _ => Transfer::Uninitialized,
            })
            .collect::<Vec<_>>();
        compare(&graph(count, edges, entry), &transfer);
    }
}

#[test]
fn private_array_cfg_solver_entry_backedge_unreachable_kills_and_exact_work() {
    // B=4; entry initializes, block 1 preserves, 2 kills on a backedge,
    // and the unreachable block 3 kills. Bad output visits exactly 2, 1;
    // their three edges contribute nine units. Total: 5B + Q + 3E_bad = 31.
    let cfg = graph(4, vec![[0, 1], [1, 0], [1, 2], [2, 1], [3, 0]], 0);
    let transfer = [
        Transfer::Initialized,
        Transfer::Identity,
        Transfer::Uninitialized,
        Transfer::Uninitialized,
    ];
    compare(&cfg, &transfer);
    for capacity in [31, 30] {
        let mut incoming = vec![false; 4];
        let mut outgoing = incoming.clone();
        let mut queue = Vec::with_capacity(4);
        let mut budget = UnsupportedIndexCorrelationBudgetV1 {
            remaining: capacity,
        };
        let result = solve(
            &cfg,
            &transfer,
            &mut incoming,
            &mut outgoing,
            &mut queue,
            &mut Work {
                budget: &mut budget,
            },
        );
        if capacity == 31 {
            result.unwrap();
            assert_eq!(budget.remaining, 0);
            assert_eq!(incoming, [true, true, true, false]);
            assert_eq!(outgoing, [false, true, true, false]);
        } else {
            assert!(matches!(result, Err(Error::ResourceLimit)));
        }
    }
    // A backedge into entry cannot erase the initial uninitialized state.
    compare(
        &graph(2, vec![[0, 1], [1, 0]], 0),
        &[Transfer::Identity, Transfer::Initialized],
    );
    // An unreachable kill predecessor cannot poison the reachable join.
    let cfg = graph(3, vec![[0, 1], [2, 1]], 0);
    let transfer = [
        Transfer::Initialized,
        Transfer::Identity,
        Transfer::Uninitialized,
    ];
    assert_eq!(
        concrete(&cfg, &transfer),
        (vec![true, false, false], vec![false; 3])
    );
    compare(&cfg, &transfer);
}
