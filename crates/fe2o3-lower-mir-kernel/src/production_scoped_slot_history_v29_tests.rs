use super::super::tests as kir;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

const LIMIT: usize = 10_000_000;
const FLOOR: usize = 31;
const CELL: Cell = Cell { slot: 19, index: 0 };
const R: EventKind = EventKind::Read;
const W: EventKind = EventKind::Set(true);
const K: EventKind = EventKind::Set(false);
const G: EventKind = EventKind::Preserve;

struct Program {
    blocks: Vec<HistoryBlock>,
    successors: Vec<usize>,
    events: Vec<Event>,
    cells: Vec<Cell>,
}

fn program(spec: &[(&[EventKind], &[usize])]) -> Program {
    let mut output = Program {
        blocks: vec![],
        successors: vec![],
        events: vec![],
        cells: vec![],
    };
    for (kinds, successors) in spec {
        let start = output.events.len();
        let edges = output.successors.len();
        output
            .events
            .extend(kinds.iter().enumerate().map(|(operation, &kind)| Event {
                cell: CELL,
                kind,
                operation,
            }));
        output.successors.extend_from_slice(successors);
        output.blocks.push(HistoryBlock {
            events: start..output.events.len(),
            successors: edges..output.successors.len(),
        });
    }
    if !output.events.is_empty() {
        output.cells.push(CELL);
    }
    output
}

fn evaluate(program: &Program, work: usize, storage: usize) -> (UseResult<()>, usize, usize) {
    let mut ledger = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut ledger, storage);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        check_history(
            &program.blocks,
            &program.successors,
            &program.events,
            &program.cells,
            0,
            budget,
        )
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

// Independent operational exploration, including both outcomes of guarded writes.
// Set(false) is an inert source-kill model here, not a production source anchor.
fn oracle(program: &Program) -> bool {
    for &cell in &program.cells {
        let mut pending = vec![(0, 0, false)];
        let mut seen = BTreeSet::new();
        while let Some((block, at, initialized)) = pending.pop() {
            if !seen.insert((block, at, initialized)) {
                continue;
            }
            let row = &program.blocks[block];
            if at == row.events.len() {
                for &target in &program.successors[row.successors.clone()] {
                    pending.push((target, 0, initialized));
                }
                continue;
            }
            let event = program.events[row.events.start + at];
            let mut next = initialized;
            if event.cell == cell {
                match event.kind {
                    EventKind::Read if !initialized => return false,
                    EventKind::Read => {}
                    EventKind::Set(value) => next = value,
                    EventKind::Preserve => pending.push((block, at + 1, true)),
                }
            }
            pending.push((block, at + 1, next));
        }
    }
    true
}

fn all_reachable(program: &Program) -> bool {
    let mut seen = BTreeSet::new();
    let mut pending = vec![0];
    while let Some(block) = pending.pop() {
        if seen.insert(block) {
            pending
                .extend_from_slice(&program.successors[program.blocks[block].successors.clone()]);
        }
    }
    seen.len() == program.blocks.len()
}

fn sequence(mut index: usize) -> Vec<EventKind> {
    let alphabet = [R, W, K, G];
    if index == 0 {
        return vec![];
    }
    index -= 1;
    if index < 4 {
        return vec![alphabet[index]];
    }
    index -= 4;
    vec![alphabet[index / 4], alphabet[index % 4]]
}

#[test]
fn every_two_block_history_matches_independent_concrete_paths() {
    let mut cases = 0;
    for count in 1_usize..=2 {
        for sequences in 0..21_usize.pow(count as u32) {
            let mut encoded = sequences;
            let kinds: Vec<_> = (0..count)
                .map(|_| {
                    let result = sequence(encoded % 21);
                    encoded /= 21;
                    result
                })
                .collect();
            for bits in 0..(1_usize << (count * count)) {
                let targets: Vec<Vec<_>> = (0..count)
                    .map(|source| {
                        (0..count)
                            .filter(|target| bits & (1 << (source * count + target)) != 0)
                            .collect()
                    })
                    .collect();
                let spec: Vec<_> = kinds
                    .iter()
                    .zip(&targets)
                    .map(|(k, t)| (k.as_slice(), t.as_slice()))
                    .collect();
                let program = program(&spec);
                let actual = evaluate(&program, LIMIT, LIMIT).0.is_ok();
                let expected = oracle(&program);
                assert!(!actual || expected, "{count}/{sequences}/{bits}");
                if all_reachable(&program) {
                    assert_eq!(actual, expected, "{count}/{sequences}/{bits}");
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 7098);
}

#[test]
fn ordered_resets_and_guarded_writes_do_not_hide_reads() {
    for (events, expected) in [
        (vec![R], false),
        (vec![W, R], true),
        (vec![R, W], false),
        (vec![W, K, R], false),
        (vec![W, K, W, R], true),
        (vec![G, R], false),
        (vec![W, G, R], true),
    ] {
        let program = program(&[(&events, &[])]);
        assert_eq!(oracle(&program), expected);
        assert_eq!(
            evaluate(&program, LIMIT, LIMIT).0.is_ok(),
            expected,
            "{events:?}"
        );
    }
}

#[test]
fn diamonds_empty_paths_and_later_loop_iterations_are_checked() {
    for (program, expected) in [
        (
            program(&[(&[], &[1, 2]), (&[W], &[3]), (&[W], &[3]), (&[R], &[])]),
            true,
        ),
        (
            program(&[(&[], &[1, 2]), (&[W], &[3]), (&[], &[3]), (&[R], &[])]),
            false,
        ),
        (
            program(&[(&[W], &[1, 2]), (&[], &[3]), (&[K], &[3]), (&[R], &[])]),
            false,
        ),
        (program(&[(&[], &[1]), (&[R], &[2]), (&[W], &[1])]), false),
        (program(&[(&[W], &[1]), (&[R], &[2]), (&[K], &[1])]), false),
        (program(&[(&[], &[1]), (&[W, R], &[2]), (&[K], &[1])]), true),
    ] {
        assert_eq!(oracle(&program), expected);
        assert_eq!(evaluate(&program, LIMIT, LIMIT).0.is_ok(), expected);
    }
}

#[test]
fn sparse_cells_and_slots_do_not_share_initialization() {
    let first = Cell {
        slot: 100,
        index: 1 << 40,
    };
    for second in [
        Cell {
            slot: 100,
            index: (1 << 40) + 1,
        },
        Cell {
            slot: 101,
            index: 1 << 40,
        },
    ] {
        let mut program = program(&[(&[W, R], &[])]);
        program.events[0].cell = first;
        program.events[1].cell = second;
        program.cells = vec![first, second];
        assert!(!oracle(&program));
        assert!(evaluate(&program, LIMIT, LIMIT).0.is_err());
    }
    let low = program(&[(&[W, R], &[])]);
    let mut high = program(&[(&[W, R], &[])]);
    high.cells[0] = first;
    for event in &mut high.events {
        event.cell = first;
    }
    let a = evaluate(&low, LIMIT, LIMIT);
    let b = evaluate(&high, LIMIT, LIMIT);
    a.0.unwrap();
    b.0.unwrap();
    assert_eq!((a.1, a.2), (b.1, b.2));
}

#[test]
fn every_activation_starts_fresh_even_on_the_same_ledger() {
    let initialized = program(&[(&[W, R], &[])]);
    let uninitialized = program(&[(&[R], &[])]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    for (program, expected) in [
        (&initialized, true),
        (&uninitialized, false),
        (&initialized, true),
    ] {
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            check_history(
                &program.blocks,
                &program.successors,
                &program.events,
                &program.cells,
                0,
                budget,
            )
        });
        assert_eq!(result.is_ok(), expected);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn history_rosters_and_physical_event_order_are_checked() {
    for mutation in 0..7 {
        let mut p = program(&[(&[W, R], &[])]);
        match mutation {
            0 => p.cells.push(CELL),
            1 => p.cells.clear(),
            2 => p.blocks[0].events.start = 1,
            3 => p.blocks[0].events.end = 3,
            4 => {
                p.successors.push(99);
                p.blocks[0].successors.end = 1;
            }
            5 => p.events[1].operation = p.events[0].operation,
            6 => {
                p.events[0].operation = 9;
                p.events[1].operation = 2;
            }
            _ => unreachable!(),
        }
        assert!(evaluate(&p, LIMIT, LIMIT).0.is_err(), "{mutation}");
    }
    evaluate(&program(&[(&[W], &[1]), (&[R], &[])]), LIMIT, LIMIT)
        .0
        .unwrap();
}

fn resource(result: UseResult<()>, work: bool) {
    assert!(
        matches!(
            (&result, work),
            (
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                ),
                true
            ) | (
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ),
                false
            )
        ),
        "{result:?}"
    );
}

#[test]
fn independently_counted_history_limits_preserve_the_caller_floor() {
    let program = program(&[(&[W, R], &[])]);
    let bytes = FLOOR
        + 3 * (std::mem::size_of::<OriginStateV1<bool>>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<bool>())
        + 2 * std::mem::size_of::<usize>();
    // Two scratch scopes4 + roster5 + ordering2 + membership4 + event scans6
    // + graph construction3 + independently counted origin-engine work34.
    let exact = evaluate(&program, 58, bytes);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (58, bytes));
    for work in 0..58 {
        resource(evaluate(&program, work, bytes).0, true);
    }
    resource(evaluate(&program, 58, bytes - 1).0, false);
}

fn raw_run(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    work_limit: usize,
    storage_limit: usize,
) -> (UseResult<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = checked_graph(function, slots, 19, budget)?;
        check(function, &graph, slots, 19, budget)
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

fn raw(function: &Function) -> UseResult<()> {
    kir::verified(function);
    raw_run(function, &kir::candidates(function), LIMIT, LIMIT).0
}

#[test]
fn actual_private_loads_require_a_definite_prior_store() {
    let mut function = kir::fixture();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(kir::load(12, kir::P, kir::scalar()));
    raw(&function).unwrap();
    let mut missing = function.clone();
    missing.body.as_mut().unwrap().blocks[0]
        .operations
        .remove(3);
    assert!(raw(&missing).is_err());
    let mut late = function.clone();
    late.body.as_mut().unwrap().blocks[0].operations.swap(3, 4);
    assert!(raw(&late).is_err());
    let mut guarded = function.clone();
    guarded.body.as_mut().unwrap().blocks[0].operations[3] = Operation::new(
        vec![],
        OperationKind::GuardedStore {
            pointer: kir::P,
            value: ValueId(1),
            predicate: ValueId(0),
            access: kir::access(),
        },
    );
    assert!(raw(&guarded).is_err());
    let mut fallback = missing;
    *fallback.body.as_mut().unwrap().blocks[0]
        .operations
        .last_mut()
        .unwrap() = kir::result(
        12,
        kir::scalar(),
        OperationKind::GuardedLoad {
            pointer: kir::P,
            predicate: ValueId(0),
            fallback: ValueId(1),
            access: kir::access(),
        },
    );
    assert!(raw(&fallback).is_err());
}

#[test]
fn actual_loop_aliases_and_nonfirst_sorted_entry_keep_physical_history() {
    let mut function = kir::loop_fixture(kir::P);
    let body = function.body.as_mut().unwrap();
    body.blocks[2].id = BlockId(5);
    let Some(Terminator::ConditionalBranch { else_target, .. }) = &mut body.blocks[1].terminator
    else {
        unreachable!()
    };
    *else_target = BlockId(5);
    raw(&function).unwrap();
    let mut missing = function.clone();
    missing.body.as_mut().unwrap().blocks[0]
        .operations
        .remove(3);
    assert!(raw(&missing).is_err());
    let mut reentered = kir::fixture();
    reentered.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(77),
        arguments: vec![],
    });
    assert!(raw(&reentered).is_err());
}

fn array_fixture(index: u64) -> (Function, Vec<ScopedSourceSlotV29>) {
    let mut function = kir::fixture();
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        0,
        kir::result(
            5,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(index + 1)),
        ),
    );
    let OperationKind::Alloca { count, .. } = &mut body.blocks[0].operations[1].kind else {
        unreachable!()
    };
    *count = Some(ValueId(5));
    body.blocks[0].operations.extend([
        kir::result(
            6,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(index)),
        ),
        kir::result(
            7,
            kir::pointer(AccessMode::ReadWrite),
            OperationKind::GetElementPointer {
                base: kir::P,
                offset: ValueId(6),
            },
        ),
        kir::store(ValueId(7), ValueId(1)),
        kir::load(8, ValueId(7), kir::scalar()),
    ]);
    let mut slots = kir::candidates(&function);
    slots[0].length = index + 1;
    slots[0].bytes = (index + 1) * 4;
    slots[0].count = Some((
        ValueId(5),
        PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: BlockId(77),
            operation: 0,
        },
    ));
    kir::verified(&function);
    (function, slots)
}

#[test]
fn actual_array_cells_are_sparse_and_distinct() {
    let (low, low_slots) = array_fixture(1);
    let (high, high_slots) = array_fixture(1 << 40);
    let a = raw_run(&low, &low_slots, LIMIT, LIMIT);
    let b = raw_run(&high, &high_slots, LIMIT, LIMIT);
    a.0.unwrap();
    b.0.unwrap();
    assert_eq!((a.1, a.2), (b.1, b.2));
    let mut missing = low.clone();
    missing.body.as_mut().unwrap().blocks[0]
        .operations
        .remove(7);
    kir::verified(&missing);
    assert!(matches!(
        raw_run(&missing, &low_slots, LIMIT, LIMIT).0,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "scoped slot read is not initialized in its fresh physical activation",
            ..
        })
    ));
    let mut mixed = low.clone();
    mixed.body.as_mut().unwrap().blocks[0].operations.extend([
        kir::result(
            10,
            kir::pointer(AccessMode::ReadWrite),
            OperationKind::Select {
                condition: ValueId(0),
                true_value: kir::P,
                false_value: ValueId(7),
            },
        ),
        kir::load(11, ValueId(10), kir::scalar()),
    ]);
    kir::verified(&mixed);
    assert!(raw_run(&mixed, &low_slots, LIMIT, LIMIT).0.is_err());
}

#[test]
fn physical_cell_bounds_alignment_and_unsupported_offsets_refuse() {
    let (function, slots) = array_fixture(1);
    for mutation in 0..4 {
        let mut changed = function.clone();
        let ops = &mut changed.body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => ops[5].kind = OperationKind::Constant(Constant::Index(2)),
            1 => {
                let OperationKind::Load { access, .. } = &mut ops[8].kind else {
                    unreachable!()
                };
                access.alignment = 16;
            }
            2 => {
                ops[6].kind = OperationKind::GetElementPointer {
                    base: kir::P,
                    offset: ValueId(1),
                }
            }
            3 => {
                ops.push(kir::result(
                    20,
                    kir::pointer(AccessMode::ReadWrite),
                    OperationKind::GetElementPointer {
                        base: ValueId(7),
                        offset: ValueId(3),
                    },
                ));
                ops.push(kir::load(21, ValueId(20), kir::scalar()));
            }
            _ => unreachable!(),
        }
        kir::verified(&changed);
        assert!(
            raw_run(&changed, &slots, LIMIT, LIMIT).0.is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn physical_history_adapter_restores_scratch_at_exact_limits() {
    let (function, slots) = array_fixture(1);
    let baseline = raw_run(&function, &slots, LIMIT, LIMIT);
    baseline.0.unwrap();
    raw_run(&function, &slots, baseline.1, baseline.2)
        .0
        .unwrap();
    resource(
        raw_run(&function, &slots, baseline.1 - 1, baseline.2).0,
        true,
    );
    resource(
        raw_run(&function, &slots, baseline.1, baseline.2 - 1).0,
        false,
    );
}
