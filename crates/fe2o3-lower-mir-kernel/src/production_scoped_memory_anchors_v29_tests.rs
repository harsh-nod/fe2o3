use super::*;
use crate::production_semantic_kir_v1::scoped_slot_uses_v29;

const LIMIT: usize = 10_000_000;

fn inspect_array_move(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        helpers += 1;
        let source = instances.instance(item.instance).unwrap().declaration();
        let SemanticStatementKindV1::Assign(assignment) =
            source.blocks()[0].statements().last().unwrap().kind()
        else {
            panic!("expected move");
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) = assignment.value().kind()
        else {
            panic!("expected move operand");
        };
        let projected = !place.projections().is_empty();
        let lowered = emitted[item.instance.index()].as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let kills: Vec<_> = anchors
            .rows
            .iter()
            .filter(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Kill { .. }))
            .collect();
        assert_eq!(kills.len(), 1);
        let ScopedMemoryAnchorKindV29::Kill {
            event,
            local,
            cause,
        } = kills[0].kind
        else {
            unreachable!()
        };
        assert_eq!(local, 2);
        assert_eq!(
            cause,
            if projected {
                ScopedMemoryKillV29::ProjectedArrayMove
            } else {
                ScopedMemoryKillV29::Move
            }
        );
        let original = instances.occurrences(item.instance).unwrap();
        assert_eq!(
            original.events()[event].role(),
            if projected {
                ExecutionEventV29::BaseUse
            } else {
                ExecutionEventV29::MoveKill
            }
        );
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == kills[0].block)
            .unwrap();
        assert!(matches!(
            block.operations[kills[0].position - 1].kind,
            OperationKind::Load { .. }
        ));
    }
    assert_eq!(helpers, 2);
    observe(instances, emitted, slots, budget)
}

#[test]
fn whole_and_projected_array_moves_keep_distinct_original_ssa_occurrences() {
    for projected in [false, true] {
        let (result, _, _) = run(
            false,
            ScopedFixture::InitializationArrayMove(projected),
            inspect_array_move,
            LIMIT,
            LIMIT,
        );
        assert!(is_stopped(&result), "{projected}: {result:?}");
    }
}

fn observe(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let floor = budget.storage();
    let result = scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    );
    assert_eq!(budget.storage(), floor);
    result?;
    Err(unsupported(0, None, None, STOP))
}

fn fixture(
    kill: Option<InitializationKillV29>,
    looping: bool,
    reinitialize: bool,
) -> ScopedFixture {
    ScopedFixture::Initialization(InitializationFixtureV29 {
        kill,
        looping,
        reinitialize,
        address_read: true,
        ..InitializationFixtureV29::default()
    })
}

#[test]
fn source_kills_reject_alias_reads_after_zero_operation_invalidations_and_moves() {
    for looping in [false, true] {
        for kill in [
            InitializationKillV29::StorageLive,
            InitializationKillV29::StorageDead,
            InitializationKillV29::Deinitialize,
            InitializationKillV29::Move,
            InitializationKillV29::StorageDeadLive,
        ] {
            let (result, _, _) = run(
                false,
                fixture(Some(kill), looping, false),
                observe,
                LIMIT,
                LIMIT,
            );
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        function: 3,
                        detail: "scoped slot read is not initialized in its fresh physical activation",
                        ..
                    })
                ),
                "{looping}/{kill:?}: {result:?}"
            );
            assert_eq!(OBSERVED.get(), 1);
            let (result, _, _) = run(
                false,
                fixture(Some(kill), looping, true),
                observe,
                LIMIT,
                LIMIT,
            );
            assert!(is_stopped(&result), "{looping}/{kill:?}: {result:?}");
        }
    }
}

fn inspect_self_move(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut pointers = BTreeSet::new();
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        let slot = slots.slots[item.slots.start];
        assert!(pointers.insert(slot.origin.pointer));
        let lowered = emitted[item.instance.index()].as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        assert_eq!(anchors.subject.instance, item.instance);
        let kills: Vec<_> = anchors
            .rows
            .iter()
            .filter(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Kill { .. }))
            .collect();
        assert_eq!(kills.len(), 1);
        let kill = kills[0];
        assert!(matches!(
            kill.kind,
            ScopedMemoryAnchorKindV29::Kill {
                local: 2,
                cause: ScopedMemoryKillV29::Move,
                ..
            }
        ));
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == kill.block)
            .unwrap();
        assert!(matches!(block.operations[kill.position - 1].kind,
            OperationKind::Load { pointer, .. } if pointer == slot.origin.pointer));
        assert!(matches!(block.operations[kill.position].kind,
            OperationKind::Store { pointer, .. } if pointer == slot.origin.pointer));
        let rows = &anchors.rows;
        let index = rows.iter().position(|row| std::ptr::eq(row, kill)).unwrap();
        assert_eq!(rows[index - 1].position + 1, kill.position);
        assert_eq!(rows[index + 1].position, kill.position);
        assert_eq!(
            rows[index - 1].source.unwrap().role,
            Some(ExecutionOperandV29::RvalueOperand(0))
        );
        assert_eq!(
            rows[index + 1].source.unwrap().role,
            Some(ExecutionOperandV29::Destination)
        );
    }
    assert_eq!(pointers.len(), 2);
    observe(instances, emitted, slots, budget)
}

#[test]
fn self_move_reads_then_invalidates_then_reinitializes_each_instance() {
    for looping in [false, true] {
        let (result, _, _) = run(
            false,
            fixture(Some(InitializationKillV29::SelfMove), looping, false),
            inspect_self_move,
            LIMIT,
            LIMIT,
        );
        assert!(is_stopped(&result), "{looping}: {result:?}");
    }
}

fn inspect_same_gap(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        helpers += 1;
        let lowered = emitted[item.instance.index()].as_ref().unwrap();
        let rows: Vec<_> = lowered
            .scoped_memory_anchors
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .filter(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Kill { .. }))
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].block, rows[1].block);
        assert_eq!((rows[0].position, rows[1].position), (0, 0));
        assert!(matches!(
            rows[0].kind,
            ScopedMemoryAnchorKindV29::Kill {
                cause: ScopedMemoryKillV29::StorageDead,
                ..
            }
        ));
        assert!(matches!(
            rows[1].kind,
            ScopedMemoryAnchorKindV29::Kill {
                cause: ScopedMemoryKillV29::StorageLive,
                ..
            }
        ));
        assert_eq!(
            scoped_memory_site_key_v29(rows[0].source.unwrap().site),
            (2, Some(0))
        );
        assert_eq!(
            scoped_memory_site_key_v29(rows[1].source.unwrap().site),
            (2, Some(1))
        );
    }
    assert_eq!(helpers, 2);
    observe(instances, emitted, slots, budget)
}

#[test]
fn separate_source_kills_at_one_gap_are_not_collapsed() {
    let (result, _, _) = run(
        false,
        fixture(Some(InitializationKillV29::StorageDeadLive), false, true),
        inspect_same_gap,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
}

fn inspect_alias_move(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        helpers += 1;
        let lowered = emitted[item.instance.index()].as_ref().unwrap();
        let rows = &lowered.scoped_memory_anchors.as_ref().unwrap().rows;
        assert!(
            rows.iter()
                .all(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }))
        );
        let exit = item.placement.block(3)?;
        assert_eq!(rows.iter().filter(|row| row.block == exit).count(), 2);
    }
    assert_eq!(helpers, 2);
    observe(instances, emitted, slots, budget)
}

#[test]
fn moving_through_alias_does_not_invalidate_the_alias_or_pointee_slot() {
    let source = ScopedFixture::Initialization(InitializationFixtureV29 {
        address_read: true,
        alias_move: true,
        ..InitializationFixtureV29::default()
    });
    let (result, _, _) = run(false, source, inspect_alias_move, LIMIT, LIMIT);
    assert!(is_stopped(&result), "{result:?}");
}

thread_local! {
    static MUTATION: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

fn mutate(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let item = slots
        .instances
        .iter()
        .find(|item| item.function.index() == 3)
        .unwrap();
    let lowered = emitted[item.instance.index()].as_mut().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
    let kill = anchors
        .rows
        .iter()
        .position(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Kill { .. }))
        .unwrap();
    match MUTATION.get() {
        0 => {
            anchors.rows.remove(kill);
        }
        1 => {
            // Keep capacity unchanged; this mutation tests duplicate coverage, not allocation.
            let other = kill + 1;
            assert!(matches!(
                anchors.rows[other].kind,
                ScopedMemoryAnchorKindV29::Kill { .. }
            ));
            anchors.rows[other] = anchors.rows[kill];
        }
        2 => {
            anchors.rows[kill].position = usize::MAX;
        }
        3 => {
            anchors.placement.first_block += 1;
        }
        4 => {
            anchors.subject.instance = instances.root();
        }
        5 => {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
            anchors.subject.ledger = foreign.work_ledger_identity_v1();
        }
        6 => {
            let index = anchors
                .rows
                .iter()
                .position(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }))
                .unwrap();
            anchors.rows.remove(index);
        }
        7 => {
            if let ScopedMemoryAnchorKindV29::Kill { local, .. } = &mut anchors.rows[kill].kind {
                *local = 999;
            }
        }
        8 => {
            anchors.rows[kill].source = None;
        }
        9 => {
            lowered.statement_operation_spans[0].semantic_function = ROOT;
        }
        10 => {
            lowered.terminator_operation_spans[0].correspondence_owner =
                SemanticFunctionIdV1::from_index(999);
        }
        _ => unreachable!(),
    }
    observe(instances, emitted, slots, budget)
}

#[test]
fn source_census_rejects_missing_duplicate_foreign_or_displaced_anchors() {
    for mutation in 0..11 {
        MUTATION.set(mutation);
        let (result, _, _) = run(
            false,
            fixture(
                Some(if mutation == 1 {
                    InitializationKillV29::StorageDeadLive
                } else {
                    InitializationKillV29::SelfMove
                }),
                false,
                mutation == 1,
            ),
            mutate,
            LIMIT,
            LIMIT,
        );
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped memory anchors differ from their source instance",
                    ..
                }) | Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ),
            "{mutation}: {result:?}"
        );
        assert_eq!(OBSERVED.get(), 1);
    }
}

#[test]
fn composed_source_and_physical_history_obeys_exact_shared_limits() {
    let source = fixture(Some(InitializationKillV29::SelfMove), true, false);
    let (result, work, peak) = run(false, source, observe, LIMIT, LIMIT);
    assert!(is_stopped(&result), "{result:?}");
    assert!(is_stopped(&run(false, source, observe, work, peak).0));
    let short_work = run(false, source, observe, work - 1, peak).0;
    let short_storage = run(false, source, observe, work, peak - 1).0;
    assert!(
        matches!(
            short_work,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ),
        "{short_work:?}"
    );
    assert!(
        matches!(
            short_storage,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ),
        "{short_storage:?}"
    );
}
