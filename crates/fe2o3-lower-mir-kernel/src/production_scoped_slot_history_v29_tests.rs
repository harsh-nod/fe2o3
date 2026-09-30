use super::super::tests as kir;

#[test]
fn whole_slot_kills_reset_sparse_cells_and_keep_same_gap_order() {
    let mut p = program(&[(&[W, EventKind::KillSlot, W, R], &[])]);
    p.cells.push(Cell {
        slot: CELL.slot,
        index: CellIndex::Literal(1 << 40),
    });
    for index in [0, 2, 3] {
        p.events[index].cell = p.cells[1];
    }
    p.events[1].operation = 1;
    p.events[1].sequence = 0;
    p.events[2].operation = 1;
    p.events[2].sequence = usize::MAX;
    assert!(oracle(&p));
    assert!(evaluate(&p, LIMIT, LIMIT).0.is_ok());
    p.events[2].kind = G;
    assert!(!oracle(&p));
    assert!(evaluate(&p, LIMIT, LIMIT).0.is_err());
    p.events[1].cell.slot += 1;
    p.cells.push(p.events[1].cell);
    p.cells.sort_unstable();
    assert!(oracle(&p));
    assert!(evaluate(&p, LIMIT, LIMIT).0.is_ok());
}
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

const LIMIT: usize = 10_000_000;
const FLOOR: usize = 31;
const CELL: Cell = Cell {
    slot: 19,
    index: CellIndex::Literal(0),
};
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
                sequence: usize::MAX,
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
        let mut pending = vec![(0, 0, false, None)];
        let mut seen = BTreeSet::new();
        while let Some((block, at, initialized, failure)) = pending.pop() {
            if !seen.insert((block, at, initialized, failure)) {
                continue;
            }
            let row = &program.blocks[block];
            if at == row.events.len() {
                for &target in &program.successors[row.successors.clone()] {
                    pending.push((target, 0, initialized, None));
                }
                continue;
            }
            let event = program.events[row.events.start + at];
            let mut next = initialized;
            let mut failure_next = failure;
            match event.kind {
                EventKind::FailureRead | EventKind::FailureKillSlot => {
                    // Every diagnostic event begins the same failure branch,
                    // even when this particular event addresses another cell.
                    let mut shadow = failure.unwrap_or(initialized);
                    if event.kind == EventKind::FailureRead && event.cell == cell && !shadow {
                        return false;
                    }
                    if event.kind == EventKind::FailureKillSlot && event.cell.slot == cell.slot {
                        shadow = false;
                    }
                    failure_next = Some(shadow);
                }
                EventKind::Read if event.cell == cell && !initialized => return false,
                EventKind::Read => {}
                EventKind::Set(value) if event.cell == cell => next = value,
                EventKind::Set(_) => {}
                EventKind::KillSlot if event.cell.slot == cell.slot => next = false,
                EventKind::KillSlot => {}
                EventKind::Preserve if event.cell == cell => {
                    pending.push((block, at + 1, true, failure));
                }
                EventKind::Preserve => {}
                EventKind::ForgetSelector => {
                    unreachable!("symbolic events are outside the original literal oracle")
                }
            }
            pending.push((block, at + 1, next, failure_next));
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
        index: CellIndex::Literal(1 << 40),
    };
    for second in [
        Cell {
            slot: 100,
            index: CellIndex::Literal((1 << 40) + 1),
        },
        Cell {
            slot: 101,
            index: CellIndex::Literal(1 << 40),
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
        + 2 * std::mem::size_of::<usize>()
        + std::mem::size_of::<Option<usize>>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<bool>();
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
    let ScopedSlotRepresentationV29::ScalarArray(scalar) = &mut slots[0].representation else {
        panic!("array fixture");
    };
    scalar.length = index + 1;
    scalar.bytes = (index + 1) * 4;
    scalar.count = Some((
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

const FR: EventKind = EventKind::FailureRead;
const FK: EventKind = EventKind::FailureKillSlot;

fn differential_failure_history(program: &Program, expected: bool) {
    assert_eq!(oracle(program), expected);
    let result = evaluate(program, LIMIT, LIMIT).0;
    if expected {
        result.unwrap();
    } else {
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped slot read is not initialized in its fresh physical activation",
                    ..
                })
            ),
            "{result:?}"
        );
    }
}

#[test]
fn diagnostic_history_reads_are_ordered_and_do_not_kill_success_values() {
    for (program, expected) in [
        (program(&[(&[FR], &[])]), false),
        (program(&[(&[W, FR, FK], &[1]), (&[R], &[])]), true),
        (program(&[(&[W, FK, FR], &[1]), (&[R], &[])]), false),
        (program(&[(&[W, EventKind::KillSlot, FR], &[])]), false),
        (program(&[(&[W, FR, FK, FR], &[])]), false),
        (program(&[(&[W, FK, FK], &[1]), (&[FR], &[])]), true),
        (program(&[(&[G, FR], &[])]), false),
        (program(&[(&[W, G, FR, FK], &[1]), (&[R], &[])]), true),
    ] {
        differential_failure_history(&program, expected);
    }
}

#[test]
fn failure_shadows_are_discarded_at_joins_and_every_loop_iteration() {
    for (program, expected) in [
        (program(&[(&[W], &[1]), (&[FR, FK], &[1])]), true),
        (program(&[(&[], &[1]), (&[W, FR, FK], &[1])]), true),
        (
            program(&[(&[W], &[1]), (&[FR, FK], &[2]), (&[K], &[1])]),
            false,
        ),
        (
            program(&[
                (&[], &[1, 2]),
                (&[W, FR, FK], &[3]),
                (&[W, FK], &[3]),
                (&[R, FR, FK], &[]),
            ]),
            true,
        ),
        (
            program(&[
                (&[], &[1, 2]),
                (&[W, FR, FK], &[3]),
                (&[FK], &[3]),
                (&[R], &[]),
            ]),
            false,
        ),
        (program(&[(&[W, FK], &[1, 1]), (&[FR, FK], &[])]), true),
    ] {
        differential_failure_history(&program, expected);
    }
}

#[test]
fn failure_slot_kills_cover_sparse_siblings_but_preserve_success_and_other_slots() {
    for high in [1, 1_u64 << 40] {
        let first = Cell {
            slot: 40,
            index: CellIndex::Literal(0),
        };
        let second = Cell {
            slot: 40,
            index: CellIndex::Literal(high),
        };
        let other = Cell {
            slot: 41,
            index: CellIndex::Literal(high),
        };
        let mut p = program(&[(&[W, W, FR, FK], &[1]), (&[R, R, FR, FK], &[1])]);
        p.cells = vec![first, second];
        for (event, cell) in p
            .events
            .iter_mut()
            .zip([first, second, first, first, first, second, second, first])
        {
            event.cell = cell;
        }
        // Both failure events share a real operation gap but retain source order.
        p.events[2].operation = 2;
        p.events[2].sequence = 0;
        p.events[3].operation = 2;
        p.events[3].sequence = 1;
        differential_failure_history(&p, true);
        p.events[2].kind = FK;
        p.events[3].kind = FR;
        p.events[3].cell = second;
        differential_failure_history(&p, false);
        p.events[2].cell = other;
        p.cells.push(other);
        differential_failure_history(&p, true);
    }
}

#[test]
fn every_two_block_failure_tail_matches_concrete_shadow_execution() {
    let prefixes: [&[EventKind]; 4] = [&[], &[W], &[K], &[G]];
    let tails: [&[EventKind]; 7] = [
        &[],
        &[FR],
        &[FK],
        &[FR, FR],
        &[FR, FK],
        &[FK, FR],
        &[FK, FK],
    ];
    let sequences: Vec<Vec<EventKind>> = prefixes
        .iter()
        .flat_map(|prefix| {
            tails
                .iter()
                .map(move |tail| prefix.iter().chain(tail.iter()).copied().collect())
        })
        .collect();
    let mut cases = 0;
    for count in 1_usize..=2 {
        for encoded in 0..28_usize.pow(count as u32) {
            let mut remaining = encoded;
            let kinds: Vec<_> = (0..count)
                .map(|_| {
                    let row = &sequences[remaining % 28];
                    remaining /= 28;
                    row
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
                    .map(|(kind, targets)| (kind.as_slice(), targets.as_slice()))
                    .collect();
                let p = program(&spec);
                let actual = evaluate(&p, LIMIT, LIMIT).0.is_ok();
                let expected = oracle(&p);
                assert!(!actual || expected, "{count}/{encoded}/{bits}");
                if all_reachable(&p) {
                    assert_eq!(actual, expected, "{count}/{encoded}/{bits}");
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 12_600);
}

#[test]
fn independently_counted_failure_history_limits_preserve_the_caller_floor() {
    let p = program(&[(&[W, FR, FK], &[1]), (&[R], &[])]);
    assert!(oracle(&p));
    // Six nodes, three links, and the three named failure-branch scratch fields.
    let bytes = FLOOR
        + 6 * (std::mem::size_of::<OriginStateV1<bool>>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<bool>())
        + 6 * std::mem::size_of::<usize>()
        + std::mem::size_of::<Option<usize>>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<bool>();
    // Scratch4 + roster7 + order4 + membership8 + scans12 + graph7 + engine65.
    let exact = evaluate(&p, 107, bytes);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (107, bytes));
    for work in 0..107 {
        resource(evaluate(&p, work, bytes).0, true);
    }
    resource(evaluate(&p, 107, bytes - 1).0, false);
}
