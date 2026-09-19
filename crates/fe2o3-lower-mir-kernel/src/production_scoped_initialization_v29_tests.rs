use super::*;

fn config() -> InitializationFixtureV29 {
    InitializationFixtureV29::default()
}

fn init_run(
    config: InitializationFixtureV29,
    observer: ScopedSlotObserverV29,
) -> Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1> {
    run(
        false,
        ScopedFixture::Initialization(config),
        observer,
        10_000_000,
        10_000_000,
    )
    .0
}

fn initialized_rows(summary: &ScopedRetainedInitializationV29) -> Vec<(u32, Vec<u32>)> {
    let mut rows: Vec<_> = summary
        .blocks
        .iter()
        .map(|row| {
            (
                row.block.index(),
                summary.initialized_locals[row.initialized.clone()].to_vec(),
            )
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
}

fn observe_initialization(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let mut helpers = 0;
    for (index, output) in emitted.iter().enumerate() {
        let output = output.as_ref().unwrap();
        let id = instances.id_at(index).unwrap();
        let instance = instances.instance(id).unwrap();
        let summary = output.scoped_initialization.as_ref().unwrap();
        assert!(summary.subject.source == receipt.source);
        assert!(summary.subject.ledger == budget.work_ledger_identity_v1());
        assert_eq!(summary.subject.instance, id);
        assert_eq!(summary.subject.function, instance.function());
        assert_eq!(
            summary
                .blocks
                .iter()
                .map(|row| row.block.index())
                .collect::<Vec<_>>(),
            instance
                .ssa()
                .plan()
                .reverse_postorder()
                .iter()
                .map(|block| block.get())
                .collect::<Vec<_>>()
        );
        summary.check_custody(
            instances,
            id,
            output.lifecycle_events.as_ref().unwrap(),
            output.scoped_slot_origins.as_ref().unwrap(),
            budget,
        )?;
        if instance.function() == SemanticFunctionIdV1::from_index(3) {
            helpers += 1;
            assert_eq!(
                initialized_rows(summary),
                vec![(0, vec![]), (1, vec![2]), (2, vec![2]), (3, vec![2]),]
            );
            let source = instance.declaration();
            let read = match source.blocks()[3].statements()[0].kind() {
                SemanticStatementKindV1::Assign(assignment) => assignment.value().kind(),
                _ => unreachable!(),
            };
            let volatile = matches!(read, SemanticRvalueKindV1::Load(load)
                if load.volatility() == SemanticVolatilityV1::Volatile);
            let source_block = output
                .lifecycle_events
                .as_ref()
                .unwrap()
                .placement
                .block(3)?;
            let block = output
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == source_block)
                .unwrap();
            let loads: Vec<_> = block
                .operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Load { pointer, access } => Some((*pointer, access)),
                    _ => None,
                })
                .collect();
            assert_eq!(loads.len(), 1);
            assert_eq!(
                loads[0].0,
                output.scoped_slot_origins.as_ref().unwrap()[0].pointer
            );
            assert_eq!(loads[0].1.volatile, volatile);
            assert_eq!(loads[0].1.address_space, AddressSpace::Private);
        } else {
            assert!(summary.initialized_locals.is_empty());
            assert!(summary.blocks.iter().all(|row| row.initialized.is_empty()));
        }
    }
    assert_eq!(helpers, 2);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn source_initialization_retains_literal_diamond_and_loop_states_per_instance() {
    for looping in [false, true] {
        for (copy_read, volatile) in [(false, false), (false, true), (true, false)] {
            let result = init_run(
                InitializationFixtureV29 {
                    looping,
                    copy_read,
                    volatile,
                    ..config()
                },
                observe_initialization,
            );
            assert!(
                is_stopped(&result),
                "{looping}/{copy_read}/{volatile}: {result:?}"
            );
            assert_eq!(OBSERVED.get(), 1);
        }
    }
}

#[test]
fn direct_slot_loads_reject_kills_on_joins_and_loop_backedges_before_capture() {
    for looping in [false, true] {
        for kill in [
            InitializationKillV29::StorageLive,
            InitializationKillV29::StorageDead,
            InitializationKillV29::Deinitialize,
            InitializationKillV29::Move,
        ] {
            for (copy_read, volatile) in [(false, false), (false, true), (true, false)] {
                let result = init_run(
                    InitializationFixtureV29 {
                        looping,
                        kill: Some(kill),
                        copy_read,
                        volatile,
                        ..config()
                    },
                    observe_initialization,
                );
                assert!(
                    matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                            function: 3,
                            block: 3,
                            statement: Some(0),
                            local: 2,
                        })
                    ),
                    "{looping}/{kill:?}/{copy_read}/{volatile}: {result:?}"
                );
                assert_eq!(OBSERVED.get(), 0);
            }
        }
    }
}

#[test]
fn reinitialization_restores_direct_reads_after_each_source_kill() {
    for looping in [false, true] {
        for kill in [
            InitializationKillV29::StorageLive,
            InitializationKillV29::StorageDead,
            InitializationKillV29::Deinitialize,
            InitializationKillV29::Move,
        ] {
            let result = init_run(
                InitializationFixtureV29 {
                    looping,
                    kill: Some(kill),
                    reinitialize: true,
                    ..config()
                },
                observe_initialization,
            );
            assert!(is_stopped(&result), "{looping}/{kill:?}: {result:?}");
            assert_eq!(OBSERVED.get(), 1);
        }
    }
}

fn reject_custody_mutation(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    index: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    mutate: impl FnOnce(&mut ScopedRetainedInitializationV29),
) {
    let output = emitted[index].as_mut().unwrap();
    let summary = output.scoped_initialization.as_mut().unwrap();
    let subject = summary.subject;
    let blocks = summary.blocks.clone();
    let locals = summary.initialized_locals.clone();
    let storage = summary.retained_storage;
    let floor = budget.storage();
    mutate(summary);
    let error = summary
        .check_custody(
            instances,
            instances.id_at(index).unwrap(),
            output.lifecycle_events.as_ref().unwrap(),
            output.scoped_slot_origins.as_ref().unwrap(),
            budget,
        )
        .err()
        .unwrap();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "retained initialization differs from its scoped source instance",
                ..
            } | ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), floor);
    summary.subject = subject;
    summary.blocks = blocks;
    summary.initialized_locals = locals;
    summary.retained_storage = storage;
}

#[test]
fn retained_initialization_custody_rejects_foreign_headers_and_malformed_rows() {
    fn observe(
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        assert!(is_stopped(&observe_initialization(
            instances, emitted, receipt, budget
        )));
        let indices: Vec<_> = receipt
            .slots
            .iter()
            .map(|slot| slot.instance.index())
            .collect();
        let index = indices[0];
        let other = emitted[indices[1]]
            .as_ref()
            .unwrap()
            .scoped_initialization
            .as_ref()
            .unwrap()
            .subject;
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.subject = other
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.subject.source.semantic[0] ^= 1
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.subject.source.root = HELPER
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.subject.function = ROOT
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.blocks.pop();
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.blocks.swap(1, 2)
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.blocks[1].block = SemanticBlockIdV1::from_index(99)
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.blocks[1].initialized.start += 1
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.blocks[1].initialized.end = usize::MAX
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.initialized_locals[0] = 1
        });
        reject_custody_mutation(instances, emitted, index, budget, |summary| {
            summary.retained_storage += 1
        });
        let output = emitted[index].as_ref().unwrap();
        let summary = output.scoped_initialization.as_ref().unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut foreign = ArgumentBudgetV1::new(&mut work, 1000);
        foreign.reserve_storage(37)?;
        assert!(matches!(
            summary.check_custody(
                instances,
                instances.id_at(index).unwrap(),
                output.lifecycle_events.as_ref().unwrap(),
                output.scoped_slot_origins.as_ref().unwrap(),
                &mut foreign
            ),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), 37);
        assert!(emitted.iter().all(Option::is_some));
        Err(unsupported(0, None, None, STOP))
    }
    assert!(is_stopped(&init_run(config(), observe)));
}

#[test]
fn retained_initialization_capture_has_independent_exact_resource_limits() {
    fn observe(
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        assert!(is_stopped(&observe_initialization(
            instances, emitted, receipt, budget
        )));
        let slot = receipt.slots[0];
        let output = emitted[slot.instance.index()].as_ref().unwrap();
        let summary = output.scoped_initialization.as_ref().unwrap();
        let instance = instances.instance(slot.instance).unwrap();
        let slots = BTreeMap::from([(
            2,
            SemanticRetainedLocalSlotV1 {
                pointer: slot.origin.pointer,
                semantic_type: U32,
                kernel_type: Type::Scalar(ScalarType::U32),
                alignment: 4,
                array: None,
            },
        )]);
        // Literal source expectations, not another invocation of the analysis.
        let entries = BTreeMap::from([
            (0, BTreeSet::new()),
            (1, BTreeSet::from([2])),
            (2, BTreeSet::from([2])),
            (3, BTreeSet::from([2])),
        ]);
        let capture = |budget: &mut ArgumentBudgetV1<'_>| {
            capture_scoped_initialization_v29(
                summary.subject,
                instance.declaration(),
                instance.ssa(),
                &slots,
                &entries,
                budget,
            )
        };
        let floor = budget.storage();
        let available = budget.storage_limit() - floor;
        let filler = available - 1_000_000;
        let old_peak = budget.peak_storage();
        budget.reserve_storage(filler)?;
        let entry = budget.storage();
        assert!(entry > old_peak);
        let before = budget.work();
        let replay = capture(budget)?;
        let work = budget.work() - before;
        let peak = budget.peak_storage() - entry;
        assert_eq!(replay.blocks, summary.blocks);
        assert_eq!(replay.initialized_locals, summary.initialized_locals);
        let bytes = replay.retained_storage;
        drop(replay);
        budget.release_storage(bytes + filler)?;
        for (allowance, success) in [(peak, true), (peak - 1, false)] {
            let filler = available - allowance;
            budget.reserve_storage(filler)?;
            match capture(budget) {
                Ok(replay) => {
                    assert!(success);
                    let bytes = replay.retained_storage;
                    drop(replay);
                    budget.release_storage(bytes)?;
                }
                Err(error) => {
                    assert!(!success);
                    assert_resource(&error, false);
                }
            }
            assert_eq!(budget.storage(), floor + filler);
            budget.release_storage(filler)?;
        }
        budget.charge_work(10_000_000 - budget.work() - (work - 1))?;
        assert_resource(&capture(budget).err().unwrap(), true);
        assert_eq!(budget.storage(), floor);
        Err(unsupported(0, None, None, STOP))
    }
    assert!(is_stopped(&init_run(config(), observe)));
}
