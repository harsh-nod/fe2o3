//! Algorithm-level fixtures; owner-backed V18 tests separately check custody.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn coordinate(index: usize) -> Coordinate {
    Coordinate::Operation {
        function: 0,
        block: 0,
        operation: index as u32,
    }
}

fn operation(index: usize, source: bool) -> Node {
    Node {
        kind: Kind::Operation,
        input: source.then(|| Endpoint::Operation(coordinate(index))),
        result_index: None,
    }
}

fn replacement(from: usize, to: usize) -> Event {
    Event {
        pass: 0,
        change: Change::Replace(from as u32, to as u32),
    }
}

fn metered(
    nodes: &[Node],
    events: &[Event],
    terminal: &[Option<Endpoint>],
    targets: usize,
    budget: &mut Budget<'_>,
    counts: &mut RelationTraversalCountsV12,
) -> Result<(
    Vec<KirOptimizationRelationV12>,
    Vec<Endpoint>,
    Vec<Coordinate>,
)> {
    derive_relations_with_work_v18(nodes, events, terminal, targets, counts, &mut |units| {
        budget.charge_work(
            units
                .checked_mul(128)
                .ok_or(KirOptimizationMapErrorV12::Arithmetic)?,
        )?;
        Ok(())
    })
}

#[test]
fn singleton_chain_meter_scales_with_materialized_rows_not_source_times_history() {
    for log in [0, 5, 6, 7, 10] {
        let n = 1usize << log;
        let nodes = (0..n).map(|i| operation(i, true)).collect::<Vec<_>>();
        let events = (0..n - 1)
            .map(|i| replacement(i, i + 1))
            .collect::<Vec<_>>();
        let mut terminal = vec![None; n];
        terminal[n - 1] = Some(Endpoint::Operation(coordinate(n - 1)));
        // 24N-7 fixed DAG rows, 3N search/target/sort rows, N(logN+1)
        // source sort, and 8N(logN+2) destination-tree work.
        let units = n * (44 + 9 * log) - 7;
        let mut work = Work::new(7 + 128 * units);
        work.charge_work(7).unwrap();
        let mut budget = Budget::new(&mut work, 13);
        budget.reserve_storage(13).unwrap();
        let mut counts = RelationTraversalCountsV12::default();
        let actual = metered(&nodes, &events, &terminal, n, &mut budget, &mut counts).unwrap();
        assert_eq!(
            actual,
            derive_relations(&nodes, &events, &terminal, n).unwrap()
        );
        assert_eq!(budget.work(), 7 + 128 * units);
        assert_eq!(budget.storage(), 13);
        assert_eq!((counts.suffix_nodes, counts.suffix_edges), (n, n - 1));
        assert_eq!((counts.search_nodes, counts.search_edges), (n, 0));
    }
}

#[test]
fn singleton_exact_and_one_short_denials_preserve_prefix_and_floor() {
    let nodes = [operation(0, true)];
    let terminal = [Some(Endpoint::Operation(coordinate(0)))];
    // One node, no events, one target: 512+64+4096 scratch bytes.
    // Logical meter: 17 initial + 1 pop + 1 target + 1 target sort
    // + 1 source sort + 16 destination units = 37 units.
    for (allowance, scratch) in [(4735, 4672), (4736, 4671), (4736, 4672)] {
        let mut work = Work::new(7 + allowance);
        work.charge_work(7).unwrap();
        let mut budget = Budget::new(&mut work, 13 + scratch);
        budget.reserve_storage(13).unwrap();
        let reserve = budget.reserve_storage(4672);
        if scratch == 4671 {
            assert!(matches!(reserve, Err(ResourceError::Storage(error))
                if error.actual() == 4685 && error.limit() == 4684));
            assert_eq!(budget.work(), 7);
        } else {
            reserve.unwrap();
            let result = metered(
                &nodes,
                &[],
                &terminal,
                1,
                &mut budget,
                &mut RelationTraversalCountsV12::default(),
            );
            if allowance == 4735 {
                assert!(
                    matches!(result, Err(KirOptimizationMapErrorV12::Resources(ResourceError::Work(error)))
                    if error.actual() == 4743 && error.limit() == 4742)
                );
                assert_eq!(budget.work(), 7 + 128 * 21);
            } else {
                result.unwrap();
                assert_eq!(budget.work(), 4743);
            }
            budget.release_storage(4672).unwrap();
        }
        assert_eq!(budget.storage(), 13);
    }
}

#[test]
fn multiple_terminal_paths_are_metered_and_target_cap_is_not_relaxed() {
    let nodes = (0..4).map(|i| operation(i, i == 0)).collect::<Vec<_>>();
    let events = [
        replacement(0, 1),
        replacement(0, 3),
        replacement(1, 2),
        replacement(2, 3),
    ];
    let terminal = [
        Some(Endpoint::Operation(coordinate(0))),
        None,
        None,
        Some(Endpoint::Operation(coordinate(3))),
    ];
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    let mut counts = RelationTraversalCountsV12::default();
    let actual = metered(&nodes, &events, &terminal, 2, &mut budget, &mut counts).unwrap();
    assert_eq!(
        actual,
        derive_relations(&nodes, &events, &terminal, 2).unwrap()
    );
    assert_eq!(actual.1, vec![terminal[0].unwrap(), terminal[3].unwrap()]);
    assert_eq!(counts.search_edges, 2);
    assert_eq!(
        metered(&nodes, &events, &terminal, 1, &mut budget, &mut counts),
        Err(KirOptimizationMapErrorV12::Limit)
    );
    assert_eq!(budget.storage(), 17);
}

#[test]
fn cyclic_replacement_is_rejected_after_only_its_prepaid_dag_scan() {
    let nodes = [operation(0, true), operation(1, true)];
    let events = [replacement(0, 1), replacement(1, 0)];
    let mut work = Work::new(7 + 128 * 49);
    work.charge_work(7).unwrap();
    let mut budget = Budget::new(&mut work, 0);
    let mut counts = RelationTraversalCountsV12::default();
    assert_eq!(
        metered(&nodes, &events, &[None, None], 2, &mut budget, &mut counts),
        Err(KirOptimizationMapErrorV12::Relation)
    );
    assert_eq!(budget.work(), 7 + 128 * 49);
    assert_eq!((counts.suffix_nodes, counts.search_nodes), (0, 0));
}

#[test]
fn genuine_empty_v18_map_uses_exact_actual_scratch_independent_of_metadata() {
    use crate::neutral_optimization_v1::storage_v18::tests::{SPACE, WORK, input, observe};
    // Include the leading "m" in the codec's 4096-byte module ID limit.
    for padding in [0, 4095] {
        let input = input(&Module::new(format!("m{}", "x".repeat(padding))));
        let mut fixture_work = Work::new(WORK);
        let mut fixture_budget = Budget::new(&mut fixture_work, SPACE);
        fixture_budget.reserve_storage(input.storage).unwrap();
        let observed = observe(&input, &mut fixture_budget);
        // Empty immutable census: 3 traversal units + 128 base + 128 DAG.
        // Actual scratch has no nodes/events/targets, only its 4096 header.
        for (work_allowance, scratch) in [(258, 4096), (259, 4095), (259, 4096)] {
            let mut work = Work::new(7 + work_allowance);
            work.charge_work(7).unwrap();
            let mut budget = Budget::new(&mut work, 13 + scratch);
            budget.reserve_storage(13).unwrap();
            let result = observed
                .map()
                .check_against(&input.owner, observed.owner(), &mut budget);
            match (work_allowance, scratch) {
                (258, _) => {
                    assert!(
                        matches!(result, Err(KirOptimizationMapErrorV12::Resources(ResourceError::Work(error)))
                        if error.actual() == 266 && error.limit() == 265)
                    );
                    assert_eq!(budget.work(), 138);
                }
                (_, 4095) => {
                    assert!(
                        matches!(result, Err(KirOptimizationMapErrorV12::Resources(ResourceError::Storage(error)))
                        if error.actual() == 4109 && error.limit() == 4108)
                    );
                    assert_eq!(budget.work(), 138);
                }
                _ => {
                    result.unwrap();
                    assert_eq!(budget.work(), 266);
                }
            }
            assert_eq!(budget.storage(), 13);
        }
    }
}

#[test]
fn actual_scratch_arithmetic_refuses_each_overflowing_dimension() {
    for limits in [
        CaptureLimitsV12 {
            nodes: usize::MAX,
            events: 0,
            targets: 0,
        },
        CaptureLimitsV12 {
            nodes: 0,
            events: usize::MAX,
            targets: 0,
        },
        CaptureLimitsV12 {
            nodes: 0,
            events: 0,
            targets: usize::MAX,
        },
    ] {
        assert_eq!(
            limits.storage(),
            Err(KirOptimizationMapErrorV12::Arithmetic)
        );
    }
}
