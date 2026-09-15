use super::super::state::{Path, PathElement, Resource};
use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Reference = BTreeMap<u32, BTreeSet<Path>>;

#[derive(Clone, Debug)]
enum Op {
    Mark(u32, Path),
    Clear(u32),
    Initialize(u32, Path),
    Read(u32, Path),
}

#[derive(Clone, Debug)]
struct Edge {
    target: usize,
    initialize: Option<(u32, Path)>,
}

#[derive(Clone, Debug, Default)]
struct Block {
    ops: Vec<Op>,
    edges: Vec<Edge>,
}

fn edge(target: usize) -> Edge {
    Edge {
        target,
        initialize: None,
    }
}

fn field(index: u32) -> Path {
    vec![PathElement::Field(index)]
}

fn mark(reference: &mut Reference, local: u32, path: Path) {
    let paths = reference.entry(local).or_default();
    if path.is_empty() {
        paths.clear();
    } else if paths.contains(&Vec::new()) {
        return;
    }
    paths.insert(path);
}

fn initialize(reference: &mut Reference, local: u32, path: &[PathElement]) -> bool {
    if path.is_empty() {
        reference.remove(&local);
        return true;
    }
    if let Some(paths) = reference.get_mut(&local) {
        if paths
            .iter()
            .any(|moved| moved.len() < path.len() && path.starts_with(moved))
        {
            return false;
        }
        paths.retain(|moved| !moved.starts_with(path));
        if paths.is_empty() {
            reference.remove(&local);
        }
    }
    true
}

fn merge(reference: &mut Reference, source: &Reference) -> bool {
    let before = reference.clone();
    for (local, paths) in source {
        for path in paths {
            mark(reference, *local, path.clone());
        }
    }
    *reference != before
}

fn readable(reference: &Reference, local: u32, path: &[PathElement]) -> bool {
    !reference.get(&local).is_some_and(|paths| {
        paths
            .iter()
            .any(|moved| moved.starts_with(path) || path.starts_with(moved))
    })
}

fn apply_reference(state: &mut Reference, op: &Op) -> bool {
    match op {
        Op::Mark(local, path) => {
            mark(state, *local, path.clone());
            true
        }
        Op::Clear(local) => {
            state.remove(local);
            true
        }
        Op::Initialize(local, path) => initialize(state, *local, path),
        Op::Read(local, path) => readable(state, *local, path),
    }
}

fn apply_state(state: &mut State, op: &Op, budget: &Budget) -> Result<bool> {
    match op {
        Op::Mark(local, path) => {
            state.mark(*local, path.clone(), budget)?;
            Ok(true)
        }
        Op::Clear(local) => {
            state.clear(*local, budget)?;
            Ok(true)
        }
        Op::Initialize(local, path) => state.initialize(*local, path, budget),
        Op::Read(local, path) => state.readable(*local, path, budget),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Observations {
    // Test-only maps do not retain any persistent State/Rc allocation.
    input: Vec<Option<Reference>>,
    checks: BTreeMap<(usize, usize), bool>,
}

fn solve(
    blocks: &[Block],
    consume: bool,
    budget: &Budget,
    observe: bool,
) -> Result<(Observations, usize)> {
    let retention = if consume {
        Some(InputRetention::new(blocks.len(), 0, budget, |record| {
            for (source, block) in blocks.iter().enumerate() {
                for edge in &block.edges {
                    record(source, edge.target)?;
                }
            }
            Ok(())
        })?)
    } else {
        None
    };
    let mut incoming = vec![None::<State>; blocks.len()];
    let mut reference = vec![None::<Reference>; blocks.len()];
    incoming[0] = Some(State::default());
    reference[0] = Some(Reference::new());
    let mut pending = VecDeque::from([0]);
    let mut queued = vec![false; blocks.len()];
    queued[0] = true;
    let mut checks = BTreeMap::new();
    let mut visited_inputs = vec![None; blocks.len()];
    let mut visits = 0;
    while let Some(block) = pending.pop_front() {
        budget.work(1)?;
        visits += 1;
        queued[block] = false;
        let mut state = match &retention {
            Some(retention) => retention.begin(block, &mut incoming).unwrap(),
            None => incoming[block].clone().unwrap(),
        };
        let mut model = if observe {
            if consume && retention.as_ref().unwrap().incoming_edges[block] == 1 {
                reference[block].take().unwrap_or_default()
            } else {
                reference[block].clone().unwrap_or_default()
            }
        } else {
            Reference::new()
        };
        if observe {
            // Input facts can only grow in the whole-local-dominates lattice.
            if let Some(previous) = &visited_inputs[block] {
                let mut joined: Reference = model.clone();
                assert!(!merge(&mut joined, previous));
            }
            visited_inputs[block] = Some(model.clone());
        }
        for (statement, op) in blocks[block].ops.iter().enumerate() {
            budget.work(1)?;
            let passed = apply_state(&mut state, op, budget)?;
            if observe {
                assert_eq!(passed, apply_reference(&mut model, op));
                // A prior failed read/initialization cannot become valid when
                // another predecessor contributes additional moved facts.
                assert!(checks.get(&(block, statement)).copied().unwrap_or(true) || !passed);
                checks.insert((block, statement), passed);
            }
        }
        for edge in &blocks[block].edges {
            budget.work(1)?;
            let mut edge_state = state.clone();
            let mut edge_model = if observe {
                model.clone()
            } else {
                Reference::new()
            };
            if let Some((local, path)) = &edge.initialize {
                let passed = edge_state.initialize(*local, path, budget)?;
                if observe {
                    assert_eq!(passed, initialize(&mut edge_model, *local, path));
                }
            }
            let first = incoming[edge.target].is_none();
            let changed = incoming[edge.target]
                .get_or_insert_with(State::default)
                .merge(&edge_state, budget)?;
            if observe {
                assert_eq!(
                    changed,
                    merge(
                        reference[edge.target].get_or_insert_with(Reference::new),
                        &edge_model
                    )
                );
            }
            if (first || changed) && !queued[edge.target] {
                queued[edge.target] = true;
                pending.push_back(edge.target);
            }
        }
    }
    Ok((
        Observations {
            input: visited_inputs,
            checks,
        },
        visits,
    ))
}

fn budget() -> Budget {
    Budget::new(0, 0, 2_097_152, 67_108_864).unwrap()
}

#[test]
fn exact_edge_multiplicity_and_external_entry_are_retained() {
    let budget = budget();
    let policy = InputRetention::new(6, 0, &budget, |record| {
        for (source, target) in [(0, 1), (1, 2), (1, 2), (2, 0), (3, 4), (4, 4), (4, 5)] {
            record(source, target)?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(&*policy.incoming_edges, &[2, 1, 2, 0, 2, 1]);
    let mut original = State::default();
    original.mark(7, field(0), &budget).unwrap();
    let mut inputs = vec![Some(original.clone()); 6];
    for block in 0..6 {
        let result = policy.begin(block, &mut inputs).unwrap();
        assert!(!result.readable(7, &field(0), &budget).unwrap());
        assert_eq!(inputs[block].is_none(), policy.incoming_edges[block] == 1);
    }
    assert!(!original.readable(7, &field(0), &budget).unwrap());
}

#[test]
fn missing_queued_input_does_not_synthesize_an_empty_state() {
    let budget = budget();
    let policy = InputRetention::new(2, 0, &budget, |record| record(0, 1)).unwrap();
    let mut inputs = vec![None, None];
    assert!(policy.begin(0, &mut inputs).is_none());
    assert!(policy.begin(1, &mut inputs).is_none());
}

#[test]
fn differential_cyclic_cfgs_preserve_exact_maps_and_checks() {
    let mut random = 0x6194_830f_a276_d10bu64;
    for case in 0..192 {
        let mut next = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            random
        };
        let count = 5 + (next() % 19) as usize;
        let mut blocks = vec![Block::default(); count];
        for (index, block) in blocks.iter_mut().enumerate() {
            if index + 1 < count {
                block.edges.push(edge(index + 1));
            }
            if next() % 3 == 0 {
                block.edges.push(edge(next() as usize % count));
            }
            for _ in 0..1 + next() % 4 {
                let local = (next() % 8) as u32;
                let path = match next() % 4 {
                    0 => vec![],
                    1 => field(0),
                    2 => field(1),
                    _ => vec![PathElement::Field(0), PathElement::Field(1)],
                };
                block.ops.push(match next() % 4 {
                    0 => Op::Mark(local, path),
                    1 => Op::Clear(local),
                    2 => Op::Initialize(local, path),
                    _ => Op::Read(local, path),
                });
            }
        }
        let old = solve(&blocks, false, &budget(), true).unwrap().0;
        let new = solve(&blocks, true, &budget(), true).unwrap().0;
        assert_eq!(new, old, "case {case}: {blocks:?}");
    }
}

#[test]
fn call_normal_return_initialization_cannot_erase_unwind_facts() {
    let blocks = vec![
        Block {
            ops: vec![Op::Mark(0, vec![])],
            edges: vec![
                Edge {
                    target: 1,
                    initialize: Some((0, vec![])),
                },
                edge(1),
            ],
        },
        Block {
            ops: vec![Op::Initialize(0, field(0)), Op::Read(0, field(0))],
            edges: vec![],
        },
    ];
    let old = solve(&blocks, false, &budget(), true).unwrap().0;
    let new = solve(&blocks, true, &budget(), true).unwrap().0;
    assert_eq!(old, new);
    assert_eq!(new.checks.get(&(1, 0)), Some(&false));
    assert_eq!(new.checks.get(&(1, 1)), Some(&false));
}

#[test]
fn sole_predecessor_loops_still_stop_at_external_entry_or_join() {
    for backedge in [0, 1] {
        let mut blocks = vec![Block::default(); 80];
        for (index, block) in blocks.iter_mut().enumerate() {
            block.ops.push(Op::Mark(index as u32, field(1)));
            block
                .edges
                .push(edge(if index == 79 { backedge } else { index + 1 }));
        }
        let old = solve(&blocks, false, &budget(), true).unwrap().0;
        let (new, visits) = solve(&blocks, true, &budget(), true).unwrap();
        assert_eq!(old, new);
        assert!(visits <= 240);
    }
}

fn dense_chain(count: usize) -> Vec<Block> {
    (0..count)
        .map(|index| Block {
            ops: (0..16)
                .map(|offset| Op::Mark((index * 16 + offset) as u32, vec![]))
                .collect(),
            edges: if index + 1 < count {
                vec![edge(index + 1)]
            } else {
                vec![]
            },
        })
        .collect()
}

#[test]
fn dense_frame_dead_facts_release_consumed_history_at_same_ceiling() {
    let blocks = dense_chain(4096);
    let old_budget = budget();
    let new_budget = budget();
    solve(&blocks, false, &old_budget, false).unwrap();
    solve(&blocks, true, &new_budget, false).unwrap();
    eprintln!(
        "synthetic dense chain: retained={} consumed={} additional peak words",
        old_budget.peak(),
        new_budget.peak()
    );
    assert!(new_budget.peak() * 8 < old_budget.peak());
    let base = 2_097_152 - new_budget.peak();
    solve(
        &blocks,
        true,
        &Budget::new(base, 0, 2_097_152, 67_108_864).unwrap(),
        false,
    )
    .unwrap();
    assert!(matches!(
        solve(
            &blocks,
            false,
            &Budget::new(base, 0, 2_097_152, 67_108_864).unwrap(),
            false
        ),
        Err(Error::Limit {
            resource: Resource::Storage,
            ..
        })
    ));
    assert!(matches!(
        solve(
            &blocks,
            true,
            &Budget::new(base + 1, 0, 2_097_152, 67_108_864).unwrap(),
            false
        ),
        Err(Error::Limit {
            resource: Resource::Storage,
            ..
        })
    ));
}

#[test]
fn retained_frontier_work_is_charged_and_not_exempted() {
    let blocks = dense_chain(24);
    let measurement = budget();
    solve(&blocks, true, &measurement, false).unwrap();
    let work = measurement.work_units();
    solve(
        &blocks,
        true,
        &Budget::new(0, 0, 2_097_152, work).unwrap(),
        false,
    )
    .unwrap();
    assert!(matches!(
        solve(
            &blocks,
            true,
            &Budget::new(0, 0, 2_097_152, work - 1).unwrap(),
            false
        ),
        Err(Error::Limit {
            resource: Resource::Work,
            ..
        })
    ));
}
