use super::*;
use crate::production_semantic_kir_v1::scoped_slot_uses_v29;

fn stop_after_uses(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    assert!(receipt.ledger == budget.work_ledger_identity_v1());
    assert!(receipt.source == ExecutionCallSourceV29::from_instances(instances, budget)?);
    assert_eq!(receipt.instances.len(), emitted.len());
    let actual_allocations = emitted
        .iter()
        .flatten()
        .flat_map(|lowered| &lowered.function.body.as_ref().unwrap().blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Alloca { .. }))
        .count();
    assert_eq!(receipt.slots.len(), actual_allocations);
    for slot in &receipt.slots {
        let source = instances.instance(slot.instance).unwrap();
        assert_eq!(
            source.declaration().locals()[slot.origin.local as usize].ty(),
            slot.origin.semantic_type
        );
        let body = emitted[slot.instance.index()]
            .as_ref()
            .unwrap()
            .function
            .body
            .as_ref()
            .unwrap();
        let block = &body.blocks[slot.allocation.block_ordinal];
        assert_eq!(block.id, slot.allocation.block);
        let allocation = &block.operations[slot.allocation.operation];
        assert_eq!(allocation.results[0].id, slot.origin.pointer);
        assert!(matches!(
            allocation.kind,
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            }
        ));
    }
    let floor = budget.storage();
    scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, receipt, 1024, budget,
    )?;
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn real_source_scalar_array_diamond_and_loop_slot_uses_pass() {
    for fixture in [
        ScopedFixture::Arrays,
        ScopedFixture::RepeatedSlots,
        ScopedFixture::Initialization(InitializationFixtureV29::default()),
        ScopedFixture::Initialization(InitializationFixtureV29 {
            looping: true,
            ..InitializationFixtureV29::default()
        }),
        ScopedFixture::InitializationArray(true),
    ] {
        let (result, _, _) = run(false, fixture, stop_after_uses, 10_000_000, 10_000_000);
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
    }
}

fn observe_source_alias(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = 0;
    let mut pointers = BTreeSet::new();
    for (index, output) in emitted.iter().enumerate() {
        let id = instances.id_at(index).unwrap();
        let instance = instances.instance(id).unwrap();
        if instance.function() != SemanticFunctionIdV1::from_index(3) {
            continue;
        }
        helpers += 1;
        let source = instance.declaration();
        let statements = source.blocks()[3].statements();
        assert_eq!(statements.len(), 2);
        let SemanticStatementKindV1::Assign(address) = statements[0].kind() else {
            panic!("expected source address assignment");
        };
        assert_eq!(address.destination(), &place(5, source.locals()[5].ty()));
        assert_eq!(
            address.value().kind(),
            &SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(2, U32),
            }
        );
        let SemanticStatementKindV1::Assign(read) = statements[1].kind() else {
            panic!("expected source load assignment");
        };
        assert_eq!(read.destination(), &place(0, U32));
        let SemanticRvalueKindV1::Load(load) = read.value().kind() else {
            panic!("expected source memory load");
        };
        assert_eq!(
            load.source(),
            &SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(5),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()
                ],
                U32,
            )
            .unwrap()
        );
        let output = output.as_ref().unwrap();
        let initialization = output.scoped_initialization.as_ref().unwrap();
        let entry = initialization
            .blocks
            .iter()
            .find(|row| row.block.index() == 3)
            .unwrap();
        assert!(initialization.initialized_locals[entry.initialized.clone()].contains(&2));
        let block_id = output
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
            .find(|block| block.id == block_id)
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
        let slots: Vec<_> = receipt
            .slots
            .iter()
            .filter(|slot| slot.instance == id)
            .collect();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].origin.local, 2);
        assert!(pointers.insert(slots[0].origin.pointer));
        assert_eq!(loads[0].0, slots[0].origin.pointer);
        assert_eq!(loads[0].1.address_space, AddressSpace::Private);
        assert_eq!(loads[0].1.alignment, 4);
        assert_eq!(
            loads[0].1.volatile,
            load.volatility() == SemanticVolatilityV1::Volatile
        );
    }
    assert_eq!(helpers, 2);
    stop_after_uses(source, instances, emitted, receipt, budget)
}

#[test]
fn real_source_initialized_address_of_load_reuses_each_instance_slot() {
    // This Semantic-MIR fixture checks alias lowering, not kill-sensitive initialization.
    for looping in [false, true] {
        for volatile in [false, true] {
            let fixture = ScopedFixture::Initialization(InitializationFixtureV29 {
                looping,
                address_read: true,
                volatile,
                ..InitializationFixtureV29::default()
            });
            let (result, _, _) = run(false, fixture, observe_source_alias, 10_000_000, 10_000_000);
            assert!(is_stopped(&result), "{fixture:?}: {result:?}");
            assert_eq!(OBSERVED.get(), 1);
        }
    }
}

fn inject_observation(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let slot = receipt.slots[0];
    let lowered = emitted[slot.instance.index()].as_mut().unwrap();
    let function = &mut lowered.function;
    assert!(
        !function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(|op| &op.results)
            .any(|result| result.id == ValueId(u32::MAX))
    );
    let block = &mut function.body.as_mut().unwrap().blocks[0];
    let block_id = block.id;
    let first = u32::try_from(block.operations.len()).unwrap();
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(u32::MAX), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::Equal,
            lhs: slot.origin.pointer,
            rhs: slot.origin.pointer,
        },
    ));
    // Keep source-span coverage coherent so this reaches the pointer-use checker.
    let terminator = lowered
        .terminator_operation_spans
        .iter_mut()
        .find(|span| span.kernel_ir_block == block_id)
        .unwrap();
    assert_eq!(terminator.first_operation_ordinal, first);
    assert_eq!(terminator.operation_count, 0);
    let statement = lowered
        .statement_operation_spans
        .iter_mut()
        .rev()
        .find(|span| span.kernel_ir_block == block_id)
        .unwrap();
    assert_eq!(
        statement.first_operation_ordinal + statement.operation_count,
        first
    );
    statement.operation_count += 1;
    terminator.first_operation_ordinal += 1;
    check_scoped_defined_call_phases_v29(instances, emitted, budget)?;
    Ok(())
}

#[test]
fn production_pre_splice_hook_rejects_slot_identity_observation_after_capture() {
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedSlots,
        inject_observation,
        10_000_000,
        10_000_000,
    );
    assert_eq!(OBSERVED.get(), 1);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 3,
                detail: "scoped source-slot address escapes through an unsupported operand",
                ..
            })
        ),
        "{result:?}"
    );
}

fn replace_initial_store_with_read(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let slot = receipt.slots[0];
    let body = emitted[slot.instance.index()]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap();
    assert!(
        !body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(|operation| &operation.results)
            .any(|result| result.id == ValueId(u32::MAX))
    );
    let operation = body.blocks.iter_mut().flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::Store { pointer, .. } if pointer == slot.origin.pointer)).unwrap();
    let OperationKind::Store { access, .. } = &operation.kind else {
        unreachable!()
    };
    *operation = Operation::effect_free(
        ValueDef::new(ValueId(u32::MAX), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: slot.origin.pointer,
            access: *access,
        },
    );
    Ok(())
}

#[test]
fn production_physical_history_does_not_trust_source_initialization_summaries() {
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedSlots,
        replace_initial_store_with_read,
        10_000_000,
        10_000_000,
    );
    assert_eq!(OBSERVED.get(), 1);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 3,
                detail: "scoped slot read is not initialized in its fresh physical activation",
                ..
            })
        ),
        "{result:?}"
    );
}

fn mutate_allocation(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let slot = receipt.slots[0];
    let body = emitted[slot.instance.index()]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap();
    let operation =
        &mut body.blocks[slot.allocation.block_ordinal].operations[slot.allocation.operation];
    let OperationKind::Alloca { alignment, .. } = &mut operation.kind else {
        unreachable!()
    };
    *alignment = 0;
    Ok(())
}

#[test]
fn slot_use_check_revalidates_live_allocations_after_the_original_census() {
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedSlots,
        mutate_allocation,
        10_000_000,
        10_000_000,
    );
    assert_eq!(OBSERVED.get(), 1);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "scoped source-slot allocation census is incomplete or mismatched",
                ..
            })
        ),
        "{result:?}"
    );
}

fn check_foreign_ledger(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_receipt(instances, emitted, receipt, budget);
    let floor = budget.storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut foreign = ArgumentBudgetV1::new(&mut work, 10_000_000);
    foreign.reserve_storage(43)?;
    assert!(matches!(
        scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
            instances,
            emitted,
            receipt,
            1024,
            &mut foreign
        ),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(foreign.storage(), 43);
    assert_eq!(foreign.work(), 0);
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn source_slot_use_checks_reject_foreign_ledgers_and_obey_preassembly_limits() {
    assert!(is_stopped(
        &run(
            false,
            ScopedFixture::RepeatedSlots,
            check_foreign_ledger,
            10_000_000,
            10_000_000
        )
        .0
    ));
    let (result, work, storage) = run(
        false,
        ScopedFixture::RepeatedSlots,
        stop_after_uses,
        10_000_000,
        10_000_000,
    );
    assert!(is_stopped(&result));
    assert!(is_stopped(
        &run(
            false,
            ScopedFixture::RepeatedSlots,
            stop_after_uses,
            work,
            storage
        )
        .0
    ));
    for (work, storage, work_failure) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let result = run(
            false,
            ScopedFixture::RepeatedSlots,
            stop_after_uses,
            work,
            storage,
        )
        .0;
        assert_resource(result.as_ref().err().unwrap(), work_failure);
    }
}
