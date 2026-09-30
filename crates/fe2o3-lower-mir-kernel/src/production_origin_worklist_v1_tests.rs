use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

const LIMIT: usize = 10_000_000;
const FLOOR: usize = 17;
type State = OriginStateV1<u8>;

fn with_graph(
    seeds: &[State],
    edges: &[(usize, usize)],
    work_limit: usize,
    storage_limit: usize,
    consume: impl FnOnce(&[State], &mut Budget<'_>),
) -> (Result<()>, usize, usize) {
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut meter, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| {
        let mut graph = OriginWorkV1::new(seeds.len(), edges.len(), &mut budget)?;
        for &seed in seeds {
            graph.seed_next(seed, &mut budget)?;
        }
        for &(source, target) in edges {
            graph.add_link(source, target, &mut budget)?;
        }
        let solved = graph.solve(&mut budget)?;
        consume(&solved, &mut budget);
        Ok(())
    })();
    let release = budget.storage() - FLOOR;
    budget.release_storage(release).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

// Independent full scans, not the production queue or its join implementation.
fn reference(seeds: &[State], edges: &[(usize, usize)]) -> Vec<State> {
    let mut states = seeds.to_vec();
    for poison in [false, true] {
        if poison {
            for state in &mut states {
                if *state == State::Pending {
                    *state = State::Unknown;
                }
            }
        }
        let mut settled = false;
        for _ in 0..=2 * states.len() {
            let mut changed = false;
            for &(source, target) in edges {
                let next = match (states[source], states[target]) {
                    (State::Pending, target) => target,
                    (source, State::Pending) => source,
                    (State::Exact(left), State::Exact(right)) if left == right => {
                        State::Exact(left)
                    }
                    _ => State::Unknown,
                };
                changed |= next != states[target];
                states[target] = next;
            }
            if !changed {
                settled = true;
                break;
            }
        }
        assert!(settled, "finite-height reference failed to settle");
    }
    states
}

#[test]
fn every_three_node_graph_matches_independent_full_scans() {
    let choices = [
        State::Pending,
        State::Exact(0),
        State::Exact(1),
        State::Unknown,
    ];
    let mut cases = 0;
    for nodes in 0..=3 {
        for seed_code in 0..4_usize.pow(nodes as u32) {
            let seeds: Vec<_> = (0..nodes)
                .map(|index| choices[(seed_code / 4_usize.pow(index as u32)) % 4])
                .collect();
            for edge_mask in 0..(1_usize << (nodes * nodes)) {
                let edges: Vec<_> = (0..nodes)
                    .flat_map(|source| (0..nodes).map(move |target| (source, target)))
                    .filter(|&(source, target)| edge_mask & (1 << (source * nodes + target)) != 0)
                    .collect();
                let expected = reference(&seeds, &edges);
                with_graph(&seeds, &edges, LIMIT, LIMIT, |actual, _| {
                    assert_eq!(actual, expected, "seeds={seeds:?} edges={edges:?}");
                    assert!(!actual.contains(&State::Pending));
                })
                .0
                .unwrap();
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 33_033);
}

#[test]
fn duplicate_dependencies_and_reversed_order_do_not_hide_conflicts() {
    let seeds = [
        State::Exact(7),
        State::Exact(8),
        State::Pending,
        State::Pending,
        State::Pending,
    ];
    let mut edges = vec![(0, 2), (0, 2), (2, 3), (3, 2), (1, 3), (3, 4)];
    for _ in 0..2 {
        with_graph(&seeds, &edges, LIMIT, LIMIT, |actual, _| {
            assert_eq!(
                actual,
                [
                    State::Exact(7),
                    State::Exact(8),
                    State::Unknown,
                    State::Unknown,
                    State::Unknown
                ]
            );
        })
        .0
        .unwrap();
        edges.reverse();
    }
}

#[test]
fn absent_slot_label_is_exact_and_distinct_from_unknown() {
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut meter, LIMIT);
    let mut graph = OriginWorkV1::new(6, 4, &mut budget).unwrap();
    for state in [
        OriginStateV1::Exact(None),
        OriginStateV1::Exact(Some(0_usize)),
        OriginStateV1::Exact(Some(1)),
        OriginStateV1::Pending,
        OriginStateV1::Pending,
        OriginStateV1::Pending,
    ] {
        graph.seed_next(state, &mut budget).unwrap();
    }
    for (source, target) in [(0, 3), (1, 3), (1, 4), (2, 4)] {
        graph.add_link(source, target, &mut budget).unwrap();
    }
    let origins = graph.solve(&mut budget).unwrap();
    assert_eq!(
        origins,
        [
            OriginStateV1::Exact(None),
            OriginStateV1::Exact(Some(0)),
            OriginStateV1::Exact(Some(1)),
            OriginStateV1::Unknown,
            OriginStateV1::Unknown,
            OriginStateV1::Unknown,
        ]
    );
    drop(origins);
    budget.release_storage(budget.storage()).unwrap();
}

#[test]
fn exact_independent_work_and_storage_limits_preserve_the_caller_floor() {
    let seeds = [State::Exact(7), State::Pending, State::Pending];
    let edges = [(0, 1), (1, 2)];
    let bytes = 3 * (std::mem::size_of::<State>() + 2 * std::mem::size_of::<usize>() + 1)
        + 4 * std::mem::size_of::<usize>();
    // Five reservations; four initialization, one seed and two solve visits per node;
    // two links; three enqueues/pops and two traversed links.
    let work = 5 + 4 * 3 + 3 + 2 * 3 + 2 + 3 + 3 + 2;
    assert_eq!(work, 36);
    let (result, spent, peak) = with_graph(&seeds, &edges, work, FLOOR + bytes, |actual, _| {
        assert_eq!(actual, [State::Exact(7); 3]);
    });
    result.unwrap();
    assert_eq!(spent, work);
    assert_eq!(peak, FLOOR + bytes);
    let (result, _, _) = with_graph(&seeds, &edges, work - 1, FLOOR + bytes, |_, _| {
        panic!("one-short work reached consumer")
    });
    assert!(matches!(
        result,
        Err(OriginWorkErrorV1::Resource(Resource::Work(_)))
    ));
    let (result, _, _) = with_graph(&seeds, &edges, work, FLOOR + bytes - 1, |_, _| {
        panic!("one-short storage reached consumer")
    });
    assert!(matches!(
        result,
        Err(OriginWorkErrorV1::Resource(Resource::Storage(_)))
    ));
    for limit in 0..work {
        assert!(matches!(
            with_graph(&seeds, &edges, limit, FLOOR + bytes, |_, _| {
                panic!("exhausted prefix reached consumer")
            })
            .0,
            Err(OriginWorkErrorV1::Resource(Resource::Work(_)))
        ));
    }
}

#[test]
fn incomplete_or_overfilled_graph_rosters_and_invalid_indices_reject() {
    for case in 0..8 {
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut meter, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = (|| {
            let mut graph = OriginWorkV1::<u8>::new(2, 1, &mut budget)?;
            if case == 0 {
                return graph.solve(&mut budget).map(|_| ());
            }
            if case == 1 {
                return graph.add_link(0, 1, &mut budget);
            }
            graph.seed_next(State::Exact(0), &mut budget)?;
            if case == 2 {
                return graph.solve(&mut budget).map(|_| ());
            }
            graph.seed_next(State::Pending, &mut budget)?;
            match case {
                3 => graph.seed_next(State::Unknown, &mut budget),
                4 => graph.add_link(2, 1, &mut budget),
                5 => graph.add_link(0, usize::MAX, &mut budget),
                6 => graph.solve(&mut budget).map(|_| ()),
                7 => {
                    graph.add_link(0, 1, &mut budget)?;
                    graph.add_link(1, 0, &mut budget)
                }
                _ => unreachable!(),
            }
        })();
        assert_eq!(result, Err(OriginWorkErrorV1::Shape), "case={case}");
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    with_graph(&[], &[], 5, FLOOR, |actual, _| assert!(actual.is_empty()))
        .0
        .unwrap();
}

#[test]
fn foreign_live_ledgers_reject_before_charging_or_mutation() {
    for phase in 0..3 {
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign_meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut meter, LIMIT);
        let mut foreign = Budget::new(&mut foreign_meter, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        foreign.reserve_storage(29).unwrap();
        let mut graph = OriginWorkV1::<u8>::new(1, 1, &mut budget).unwrap();
        let result = match phase {
            0 => {
                let result = graph.seed_next(State::Exact(0), &mut foreign);
                assert_eq!(graph.seeded, 0);
                graph.seed_next(State::Exact(7), &mut budget).unwrap();
                graph.add_link(0, 0, &mut budget).unwrap();
                assert_eq!(graph.solve(&mut budget).unwrap(), [State::Exact(7)]);
                result
            }
            1 => {
                graph.seed_next(State::Exact(0), &mut budget).unwrap();
                let result = graph.add_link(0, 0, &mut foreign);
                assert!(graph.links.is_empty());
                graph.add_link(0, 0, &mut budget).unwrap();
                assert_eq!(graph.solve(&mut budget).unwrap(), [State::Exact(0)]);
                result
            }
            2 => {
                graph.seed_next(State::Exact(0), &mut budget).unwrap();
                graph.add_link(0, 0, &mut budget).unwrap();
                graph.solve(&mut foreign).map(|_| ())
            }
            _ => unreachable!(),
        };
        assert_eq!(
            result,
            Err(OriginWorkErrorV1::Resource(Resource::Accounting))
        );
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), 29);
        budget.release_storage(budget.storage() - FLOOR).unwrap();
    }
}

#[test]
fn same_work_ledger_cannot_use_a_graph_after_releasing_its_backing() {
    for phase in 0..3 {
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut meter, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut graph = OriginWorkV1::<u8>::new(1, 1, &mut budget).unwrap();
        if phase > 0 {
            graph.seed_next(State::Exact(0), &mut budget).unwrap();
        }
        if phase > 1 {
            graph.add_link(0, 0, &mut budget).unwrap();
        }
        let spent = budget.work();
        let reserved = budget.storage() - FLOOR;
        budget.release_storage(reserved).unwrap();
        let result = match phase {
            0 => graph.seed_next(State::Exact(0), &mut budget),
            1 => graph.add_link(0, 0, &mut budget),
            2 => graph.solve(&mut budget).map(|_| ()),
            _ => unreachable!(),
        };
        assert_eq!(
            result,
            Err(OriginWorkErrorV1::Resource(Resource::Accounting))
        );
        assert_eq!(budget.work(), spent);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn arithmetic_overflow_refuses_before_allocation() {
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut meter, LIMIT);
    assert!(matches!(
        OriginWorkV1::<u64>::new(usize::MAX, 0, &mut budget),
        Err(OriginWorkErrorV1::Resource(Resource::Arithmetic))
    ));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 1);
}

fn with_transfers(
    seeds: &[State],
    edges: &[(usize, usize, u8)],
    work_limit: usize,
    storage_limit: usize,
    consume: impl FnOnce(&[State]),
) -> (Result<()>, usize, usize) {
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut meter, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| {
        let mut graph = OriginWorkV1::<u8, u8>::new(seeds.len(), edges.len(), &mut budget)?;
        for &seed in seeds {
            graph.seed_next(seed, &mut budget)?;
        }
        for &(source, target, delta) in edges {
            graph.add_transfer(source, target, delta, &mut budget)?;
        }
        let solved = graph.solve_with(&mut budget, |delta, value, budget| {
            budget.charge_work(1)?;
            value.checked_add(delta).ok_or(OriginWorkErrorV1::Shape)
        })?;
        consume(&solved);
        Ok(())
    })();
    budget.release_storage(budget.storage() - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn exact_transfers_compose_without_grounding_uninitialized_cycles() {
    let seeds = [
        State::Exact(2),
        State::Pending,
        State::Pending,
        State::Pending,
        State::Pending,
    ];
    let mut edges = vec![(0, 1, 3), (1, 2, 4), (2, 3, 0), (3, 2, 0)];
    for _ in 0..2 {
        with_transfers(&seeds, &edges, LIMIT, LIMIT, |actual| {
            assert_eq!(
                actual,
                [
                    State::Exact(2),
                    State::Exact(5),
                    State::Exact(9),
                    State::Exact(9),
                    State::Unknown
                ]
            );
        })
        .0
        .unwrap();
        edges.reverse();
    }
    // A conflicting displacement, not merely a conflicting root, poisons a join.
    edges.push((0, 2, 8));
    with_transfers(&seeds, &edges, LIMIT, LIMIT, |actual| {
        assert_eq!(
            actual,
            [
                State::Exact(2),
                State::Exact(5),
                State::Unknown,
                State::Unknown,
                State::Unknown
            ]
        );
    })
    .0
    .unwrap();
    // A transfer label cannot manufacture a source for a pure incoming cycle.
    with_transfers(
        &[State::Exact(0), State::Pending, State::Pending],
        &[(1, 1, 1), (1, 2, 1), (0, 2, 0)],
        LIMIT,
        LIMIT,
        |actual| {
            assert_eq!(actual, [State::Exact(0), State::Unknown, State::Unknown]);
        },
    )
    .0
    .unwrap();
}

#[test]
fn nonzero_transfer_cycle_is_a_conflict_not_an_unbounded_iteration() {
    with_transfers(
        &[State::Exact(0), State::Pending],
        &[(0, 1, 1), (1, 1, 1)],
        LIMIT,
        LIMIT,
        |actual| {
            assert_eq!(actual, [State::Exact(0), State::Unknown]);
        },
    )
    .0
    .unwrap();
    let (result, _, _) = with_transfers(
        &[State::Exact(255), State::Pending],
        &[(0, 1, 1)],
        LIMIT,
        LIMIT,
        |_| panic!("overflow published"),
    );
    assert_eq!(result, Err(OriginWorkErrorV1::Shape));
}

#[test]
fn transferred_edges_preserve_exact_independent_resource_boundaries() {
    let seeds = [State::Exact(0), State::Pending, State::Pending];
    let edges = [(0, 1, 3), (1, 2, 4)];
    // The old 36-work graph plus two charged transfers. Each labelled link has
    // two usize indices and one byte label padded to machine-word alignment.
    let work = 38;
    let bytes = 3 * (std::mem::size_of::<State>() + 2 * std::mem::size_of::<usize>() + 1)
        + 6 * std::mem::size_of::<usize>();
    let (result, spent, peak) = with_transfers(&seeds, &edges, work, FLOOR + bytes, |actual| {
        assert_eq!(actual, [State::Exact(0), State::Exact(3), State::Exact(7)]);
    });
    result.unwrap();
    assert_eq!((spent, peak), (work, FLOOR + bytes));
    for (work_limit, storage_limit, is_work) in [
        (work - 1, FLOOR + bytes, true),
        (work, FLOOR + bytes - 1, false),
    ] {
        let (result, _, _) = with_transfers(&seeds, &edges, work_limit, storage_limit, |_| {
            panic!("short allowance published")
        });
        assert!(matches!(
            (result, is_work),
            (Err(OriginWorkErrorV1::Resource(Resource::Work(_))), true)
                | (
                    Err(OriginWorkErrorV1::Resource(Resource::Storage(_))),
                    false
                )
        ));
    }
}

#[test]
fn transfer_callbacks_cannot_publish_after_losing_graph_custody() {
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut meter, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut graph = OriginWorkV1::<u8, ()>::new(2, 1, &mut budget).unwrap();
    graph.seed_next(State::Exact(1), &mut budget).unwrap();
    graph.seed_next(State::Pending, &mut budget).unwrap();
    graph.add_transfer(0, 1, (), &mut budget).unwrap();
    let mut entered = false;
    let result = graph.solve_with(&mut budget, |(), value, budget| {
        entered = true;
        budget.release_storage(budget.storage() - FLOOR)?;
        Ok(value)
    });
    assert!(entered);
    assert_eq!(
        result,
        Err(OriginWorkErrorV1::Resource(Resource::Accounting))
    );
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn every_two_node_labelled_graph_matches_independent_full_scans() {
    fn expected(seeds: &[State], edges: &[(usize, usize, u8)]) -> Vec<State> {
        let mut states = seeds.to_vec();
        for poison in [false, true] {
            if poison {
                for state in &mut states {
                    if *state == State::Pending {
                        *state = State::Unknown;
                    }
                }
            }
            let mut settled = false;
            for _ in 0..=2 * states.len() {
                let mut changed = false;
                for &(source, target, toggle) in edges {
                    let incoming = match states[source] {
                        State::Exact(value) => State::Exact(value ^ toggle),
                        other => other,
                    };
                    let next = match (incoming, states[target]) {
                        (State::Pending, target) => target,
                        (source, State::Pending) => source,
                        (State::Exact(left), State::Exact(right)) if left == right => {
                            State::Exact(left)
                        }
                        _ => State::Unknown,
                    };
                    changed |= next != states[target];
                    states[target] = next;
                }
                if !changed {
                    settled = true;
                    break;
                }
            }
            assert!(settled, "finite-height transfer reference failed to settle");
        }
        states
    }

    let choices = [
        State::Pending,
        State::Exact(0),
        State::Exact(1),
        State::Unknown,
    ];
    let mut cases = 0;
    for nodes in 0..=2 {
        for seed_code in 0..4_usize.pow(nodes as u32) {
            let seeds: Vec<_> = (0..nodes)
                .map(|index| choices[(seed_code / 4_usize.pow(index as u32)) % 4])
                .collect();
            for edge_code in 0..3_usize.pow((nodes * nodes) as u32) {
                let edges: Vec<_> = (0..nodes * nodes)
                    .filter_map(|index| match (edge_code / 3_usize.pow(index as u32)) % 3 {
                        0 => None,
                        label => Some((index / nodes, index % nodes, (label - 1) as u8)),
                    })
                    .collect();
                let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = Budget::new(&mut meter, LIMIT);
                budget.reserve_storage(FLOOR).unwrap();
                let mut graph =
                    OriginWorkV1::<u8, u8>::new(nodes, edges.len(), &mut budget).unwrap();
                for &seed in &seeds {
                    graph.seed_next(seed, &mut budget).unwrap();
                }
                for &(source, target, toggle) in &edges {
                    graph
                        .add_transfer(source, target, toggle, &mut budget)
                        .unwrap();
                }
                let actual = graph
                    .solve_with(&mut budget, |toggle, value, budget| {
                        budget.charge_work(1)?;
                        Ok(value ^ toggle)
                    })
                    .unwrap();
                assert_eq!(
                    actual,
                    expected(&seeds, &edges),
                    "seeds={seeds:?} edges={edges:?}"
                );
                assert!(!actual.contains(&State::Pending));
                drop(actual);
                budget.release_storage(budget.storage() - FLOOR).unwrap();
                assert_eq!(budget.storage(), FLOOR);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 1_309);
}

#[test]
fn transfer_callbacks_only_observe_exact_values_and_propagate_refusals() {
    for seed in [State::Pending, State::Unknown, State::Exact(7)] {
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut meter, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut graph = OriginWorkV1::<u8, u8>::new(2, 1, &mut budget).unwrap();
        graph.seed_next(seed, &mut budget).unwrap();
        graph.seed_next(State::Pending, &mut budget).unwrap();
        graph.add_transfer(0, 1, 3, &mut budget).unwrap();
        let mut calls = 0;
        let result = graph.solve_with(&mut budget, |label, value, budget| {
            calls += 1;
            assert_eq!((label, value), (3, 7));
            budget.charge_work(1)?;
            Err(OriginWorkErrorV1::Shape)
        });
        if seed == State::Exact(7) {
            assert_eq!(calls, 1);
            assert_eq!(result, Err(OriginWorkErrorV1::Shape));
        } else {
            assert_eq!(calls, 0);
            assert_eq!(result.unwrap(), [State::Unknown; 2]);
        }
        let spent = budget.work();
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), spent);
    }
}
