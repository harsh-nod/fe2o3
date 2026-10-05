use super::*;

const LIMIT: usize = 1_000_000;
const FLOOR: usize = 47;
const SLOT: usize = 7;
const S: CellIndex = CellIndex::Selected(0);
const T: CellIndex = CellIndex::Selected(1);
const W: EventKind = EventKind::Set(true);
const R: EventKind = EventKind::Read;
const F: EventKind = EventKind::ForgetSelector;

struct Program {
    blocks: Vec<HistoryBlock>,
    successors: Vec<usize>,
    events: Vec<Event>,
    cells: Vec<Cell>,
}

fn program(spec: &[(&[(CellIndex, EventKind)], &[usize])]) -> Program {
    let mut p = Program {
        blocks: vec![],
        successors: vec![],
        events: vec![],
        cells: vec![],
    };
    for (events, successors) in spec {
        let first = p.events.len();
        let edges = p.successors.len();
        for (operation, &(index, kind)) in events.iter().enumerate() {
            let cell = Cell { slot: SLOT, index };
            p.cells.push(cell);
            p.events.push(Event {
                cell,
                kind,
                operation,
                sequence: 0,
            });
        }
        p.successors.extend_from_slice(successors);
        p.blocks.push(HistoryBlock {
            events: first..p.events.len(),
            successors: edges..p.successors.len(),
        });
    }
    p.cells.sort_unstable();
    p.cells.dedup();
    p
}

fn evaluate(
    p: &Program,
    length: u64,
    work: usize,
    storage: usize,
) -> (UseResult<()>, usize, usize) {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        check_sparse_history(
            &p.blocks,
            &p.successors,
            &p.events,
            &p.cells,
            0,
            &[(SLOT, length)],
            budget,
        )
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

// This interpreter tracks concrete bytes and independently chosen runtime
// selector values. It does not call reset(), the bool engine or its lattice.
fn concrete(p: &Program, length: usize) -> bool {
    assert!((1..=3).contains(&length));
    let mut pending = Vec::new();
    for first in 0..length {
        for second in 0..length {
            pending.push((0, 0, [false; 3], [first, second], None::<[bool; 3]>));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    while let Some((block, at, initialized, values, failure)) = pending.pop() {
        if !seen.insert((block, at, initialized, values, failure)) {
            continue;
        }
        let row = &p.blocks[block];
        if at == row.events.len() {
            for &target in &p.successors[row.successors.clone()] {
                pending.push((target, 0, initialized, values, None));
            }
            continue;
        }
        let event = p.events[row.events.start + at];
        let mut memory = initialized;
        let mut shadow = failure;
        let index = match event.cell.index {
            CellIndex::Literal(index) => Some(usize::try_from(index).unwrap()),
            CellIndex::Selected(key) => Some(values[key]),
            CellIndex::Whole => None,
        };
        let read = |memory: &[bool; 3]| {
            index.map_or_else(|| memory[..length].iter().all(|&v| v), |i| memory[i])
        };
        match event.kind {
            EventKind::Read => {
                if !read(&memory) {
                    return false;
                }
            }
            EventKind::FailureRead => {
                let memory = shadow.unwrap_or(memory);
                if !read(&memory) {
                    return false;
                }
                shadow = Some(memory);
            }
            EventKind::Set(value) => memory[index.unwrap()] = value,
            EventKind::KillSlot => memory[..length].fill(false),
            EventKind::FailureKillSlot => {
                let mut memory = shadow.unwrap_or(memory);
                memory[..length].fill(false);
                shadow = Some(memory);
            }
            EventKind::FailureKillCell => {
                let mut memory = shadow.unwrap_or(memory);
                memory[index.expect("failure cell kill has an exact element")] = false;
                shadow = Some(memory);
            }
            EventKind::Preserve => {
                let mut written = memory;
                written[index.unwrap()] = true;
                pending.push((block, at + 1, written, values, shadow));
            }
            EventKind::ForgetSelector => {
                let CellIndex::Selected(key) = event.cell.index else {
                    panic!("original selector key");
                };
                for index in 0..length {
                    let mut next = values;
                    next[key] = index;
                    pending.push((block, at + 1, memory, next, shadow));
                }
                continue;
            }
        }
        pending.push((block, at + 1, memory, values, shadow));
    }
    true
}

fn expect(p: &Program, length: usize, accepted: bool) {
    assert_eq!(concrete(p, length), accepted);
    let result = evaluate(p, length as u64, LIMIT, LIMIT).0;
    assert_eq!(result.is_ok(), accepted, "{result:?}");
}

#[test]
fn selected_history_tracks_only_the_current_selected_element() {
    expect(&program(&[(&[(S, W), (S, R)], &[])]), 2, true);
    expect(&program(&[(&[(S, W), (T, R)], &[])]), 2, false);
    expect(
        &program(&[(&[(S, W), (CellIndex::Literal(0), R)], &[])]),
        2,
        false,
    );
    expect(
        &program(&[(&[(S, W), (CellIndex::Whole, EventKind::FailureRead)], &[])]),
        2,
        false,
    );
    expect(
        &program(&[(&[(S, W), (T, EventKind::Set(false)), (S, R)], &[])]),
        2,
        false,
    );
}

#[test]
fn selected_and_complete_literal_paths_join_before_the_read() {
    let p = program(&[
        (&[], &[1, 2]),
        (&[(S, W)], &[3]),
        (
            &[(CellIndex::Literal(0), W), (CellIndex::Literal(1), W)],
            &[3],
        ),
        (&[(S, R)], &[]),
    ]);
    expect(&p, 2, true);
    expect(
        &program(&[
            (&[], &[1, 2]),
            (&[(S, W)], &[3]),
            (&[(CellIndex::Literal(0), W)], &[3]),
            (&[(S, R)], &[]),
        ]),
        2,
        false,
    );
}

#[test]
fn original_definition_and_phi_reevaluation_forget_the_old_index() {
    for split in [false, true] {
        let stale = if split {
            program(&[(&[(S, W)], &[1]), (&[(S, F)], &[2]), (&[(S, R)], &[1])])
        } else {
            program(&[(&[(S, W)], &[1]), (&[(S, F), (S, R)], &[1])])
        };
        expect(&stale, 2, false);
        let rewritten = if split {
            program(&[
                (&[(S, W)], &[1]),
                (&[(S, F)], &[2]),
                (&[(S, W), (S, R)], &[1]),
            ])
        } else {
            program(&[(&[(S, W)], &[1]), (&[(S, F), (S, W), (S, R)], &[1])])
        };
        expect(&rewritten, 2, true);
    }
    expect(
        &program(&[
            (
                &[(CellIndex::Literal(0), W), (CellIndex::Literal(1), W)],
                &[1],
            ),
            (&[(S, F), (S, R)], &[1]),
        ]),
        2,
        true,
    );
}

#[test]
fn sparse_failure_whole_reads_and_kills_keep_success_state() {
    expect(
        &program(&[
            (
                &[
                    (CellIndex::Literal(0), W),
                    (CellIndex::Literal(1), W),
                    (CellIndex::Whole, EventKind::FailureRead),
                    (CellIndex::Literal(0), EventKind::FailureKillSlot),
                ],
                &[1],
            ),
            (&[(S, R)], &[]),
        ]),
        2,
        true,
    );
    expect(
        &program(&[(
            &[
                (CellIndex::Literal(0), W),
                (CellIndex::Literal(1), W),
                (CellIndex::Literal(0), EventKind::FailureKillSlot),
                (CellIndex::Whole, EventKind::FailureRead),
            ],
            &[],
        )]),
        2,
        false,
    );
    expect(
        &program(&[(
            &[(S, W), (CellIndex::Literal(0), EventKind::KillSlot), (S, R)],
            &[],
        )]),
        2,
        false,
    );
}

#[test]
fn sparse_failure_cell_kills_preserve_siblings_and_success_selectors() {
    for (read, accepted) in [
        (CellIndex::Literal(1), true),
        (CellIndex::Literal(0), false),
        (CellIndex::Whole, false),
    ] {
        expect(
            &program(&[
                (
                    &[
                        (CellIndex::Literal(0), W),
                        (CellIndex::Literal(1), W),
                        (CellIndex::Literal(0), EventKind::FailureKillCell),
                        (read, EventKind::FailureRead),
                    ],
                    &[1],
                ),
                (&[(S, R), (T, R), (S, EventKind::FailureRead)], &[1]),
            ]),
            2,
            accepted,
        );
    }
}

#[test]
fn all_small_symbolic_histories_are_sound_against_concrete_runtime_indices() {
    let choices = [
        (S, W),
        (T, W),
        (S, R),
        (T, R),
        (S, F),
        (T, F),
        (CellIndex::Literal(0), W),
        (CellIndex::Literal(1), W),
        (S, EventKind::Set(false)),
        (CellIndex::Literal(0), EventKind::KillSlot),
        (S, EventKind::Preserve),
        (CellIndex::Whole, EventKind::FailureRead),
    ];
    let mut accepted = 0;
    for &a in &choices {
        for &b in &choices {
            for &c in &choices {
                let p = program(&[(&[a, b], &[1]), (&[c], &[0])]);
                let result = evaluate(&p, 2, LIMIT, LIMIT).0;
                if result.is_ok() {
                    accepted += 1;
                    assert!(
                        concrete(&p, 2),
                        "accepted unsound concrete history: {a:?} {b:?} {c:?}"
                    );
                }
            }
        }
    }
    assert!(accepted > 0);
}

#[test]
fn huge_sparse_extents_do_not_expand_into_per_element_history() {
    let sparse = program(&[(&[(S, W), (S, R)], &[])]);
    let result = evaluate(&sparse, u64::MAX, 1000, 4096);
    result.0.unwrap();
    assert!(result.1 < 1000 && result.2 < 4096);
    let whole = program(&[(
        &[
            (CellIndex::Literal(0), W),
            (CellIndex::Whole, EventKind::FailureRead),
        ],
        &[],
    )]);
    assert!(matches!(
        evaluate(&whole, u64::MAX, 1000, 4096).0,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "scoped whole-slot read lacks complete physical initialization",
            ..
        })
    ));
}

#[test]
fn independently_counted_selected_history_resource_boundaries() {
    let p = program(&[(&[(S, W), (S, R)], &[])]);
    let bytes = FLOOR
        + std::mem::size_of::<Vec<bool>>()
        + std::mem::size_of::<Result<Vec<bool>, ProductionSemanticKirErrorV1>>()
        + 3 * std::mem::size_of::<bool>()
        + 3 * (std::mem::size_of::<OriginStateV1<bool>>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<bool>())
        + 2 * std::mem::size_of::<usize>()
        + std::mem::size_of::<Option<usize>>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<bool>();
    // Literal W/R costs58. Add extent/group6, coverage scope2, allocation/fill6,
    // two group scans2, and checked whole resets/entry queries8: exact82.
    let exact = evaluate(&p, 2, 82, bytes);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (82, bytes));
    for work in 0..82 {
        assert!(matches!(
            evaluate(&p, 2, work, bytes).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
    }
    assert!(matches!(
        evaluate(&p, 2, 82, bytes - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}
