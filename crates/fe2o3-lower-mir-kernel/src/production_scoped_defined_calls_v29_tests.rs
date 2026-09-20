use super::*;

#[derive(Clone, Debug, PartialEq)]
struct Donor {
    function: Function,
    blocks: Vec<SemanticKirBlockCorrespondenceV1>,
    statements: Vec<SemanticKirStatementOperationSpanV1>,
    terminators: Vec<SemanticKirTerminatorOperationSpanV1>,
    calls: Vec<SemanticKirCallReturnV1>,
    components: Vec<CallResultComponentV1>,
    memory: Vec<ScopedMemoryAnchorV29>,
    source: Option<ProductionCallInstanceIdV1>,
    slots: Option<Vec<ScopedSlotOriginV29>>,
}

impl Donor {
    fn capture(lowered: &LoweredFunctionResultV1) -> Self {
        Self {
            function: lowered.function.clone(),
            blocks: lowered.blocks.clone(),
            statements: lowered.statement_operation_spans.clone(),
            terminators: lowered.terminator_operation_spans.clone(),
            calls: lowered.call_returns.sites.rows.clone(),
            components: lowered.call_returns.components.rows.clone(),
            memory: lowered.scoped_memory_anchors.as_ref().unwrap().rows.clone(),
            source: lowered.source_call_instance,
            slots: lowered.scoped_slot_origins.clone(),
        }
    }

    fn swap_with(&mut self, lowered: &mut LoweredFunctionResultV1) {
        std::mem::swap(&mut lowered.function, &mut self.function);
        std::mem::swap(&mut lowered.blocks, &mut self.blocks);
        std::mem::swap(&mut lowered.statement_operation_spans, &mut self.statements);
        std::mem::swap(
            &mut lowered.terminator_operation_spans,
            &mut self.terminators,
        );
        std::mem::swap(&mut lowered.call_returns.sites.rows, &mut self.calls);
        std::mem::swap(
            &mut lowered.call_returns.components.rows,
            &mut self.components,
        );
        std::mem::swap(
            &mut lowered.scoped_memory_anchors.as_mut().unwrap().rows,
            &mut self.memory,
        );
        std::mem::swap(&mut lowered.source_call_instance, &mut self.source);
        std::mem::swap(&mut lowered.scoped_slot_origins, &mut self.slots);
    }
}

fn donors(emitted: &[Option<LoweredFunctionResultV1>]) -> Vec<Donor> {
    emitted
        .iter()
        .map(|row| Donor::capture(row.as_ref().unwrap()))
        .collect()
}

fn checked(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let before = donors(emitted);
    let source_slots = slots.slots.clone();
    let floor = budget.storage();
    let result = check_scoped_defined_call_phases_v29(instances, emitted, budget);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        donors(emitted),
        before,
        "validation changed donor functions"
    );
    assert_eq!(slots.slots, source_slots, "validation changed source slots");
    result
}

#[derive(Clone, Copy, Debug)]
enum Refusal {
    Correspondence,
    Memory,
    SourceRows,
}

fn assert_refusal(
    label: &str,
    result: Result<(), ProductionSemanticKirErrorV1>,
    expected: Refusal,
) {
    assert!(
        matches!(
            (&result, expected),
            (
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                Refusal::Correspondence
            ) | (
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "scoped memory anchors differ from their source instance",
                }),
                Refusal::Memory
            ) | (
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution call parameters differ from their source instance",
                }),
                Refusal::SourceRows
            )
        ),
        "{label}: expected {expected:?}, got {result:?}"
    );
}

#[allow(clippy::too_many_arguments)]
fn reject(
    label: &str,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    index: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    expected: Refusal,
    mutation: impl FnOnce(&mut LoweredFunctionResultV1),
) {
    let mut saved = Donor::capture(emitted[index].as_ref().unwrap());
    // Mutate clones so the actual donor allocations survive every hostile control.
    saved.swap_with(emitted[index].as_mut().unwrap());
    mutation(emitted[index].as_mut().unwrap());
    assert_refusal(label, checked(instances, emitted, slots, budget), expected);
    saved.swap_with(emitted[index].as_mut().unwrap());
    checked(instances, emitted, slots, budget).unwrap();
}

fn callback(slots: &OwnedScopedSourceSlotsV29) -> usize {
    let mut rows = slots
        .instances
        .iter()
        .filter(|row| row.function == CALLBACK);
    let index = rows.next().unwrap().instance.index();
    assert!(rows.next().is_none());
    index
}

fn block_mut(lowered: &mut LoweredFunctionResultV1, source: u32) -> &mut BasicBlock {
    let id = lowered
        .blocks
        .iter()
        .find(|row| row.semantic_block == SemanticBlockIdV1::from_index(source))
        .unwrap()
        .kernel_ir_block;
    lowered
        .function
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == id)
        .unwrap()
}

fn call_row(lowered: &LoweredFunctionResultV1, source: u32) -> usize {
    lowered
        .call_returns
        .sites
        .rows
        .iter()
        .position(|row| row.semantic_block == SemanticBlockIdV1::from_index(source))
        .unwrap()
}

fn observe_repeated(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    checked(instances, emitted, slots, budget)?;
    let index = callback(slots);
    let id = instances.id_at(index).unwrap();
    let calls = instances.calls(id).unwrap();
    assert_eq!(calls.len(), 3);
    let first = calls[0].child().unwrap();
    let second = calls[1].child().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        instances.instance(first).unwrap().function(),
        instances.instance(second).unwrap().function()
    );
    assert!(
        calls[2].child().is_none(),
        "third call must remain intrinsic"
    );
    let a = emitted[first.index()].as_ref().unwrap();
    let b = emitted[second.index()].as_ref().unwrap();
    assert_ne!(a.function.id, b.function.id);
    assert_ne!(
        a.lifecycle_events.as_ref().unwrap().placement,
        b.lifecycle_events.as_ref().unwrap().placement
    );
    let lowered = emitted[index].as_ref().unwrap();
    for source_block in 0..4 {
        let row = lowered
            .blocks
            .iter()
            .find(|row| row.semantic_block == SemanticBlockIdV1::from_index(source_block))
            .unwrap();
        assert_ne!(row.kernel_ir_block, BlockId(source_block));
    }
    let intrinsic_block = block_mut(emitted[index].as_mut().unwrap(), 2);
    assert!(
        !intrinsic_block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Call { .. }))
    );
    assert!(
        intrinsic_block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Store { .. }))
    );
    for (index, lowered) in emitted.iter().enumerate() {
        let id = instances.id_at(index).unwrap();
        assert_eq!(
            lowered
                .as_ref()
                .unwrap()
                .call_returns
                .sites
                .rows
                .iter()
                .filter(|row| matches!(row.kind, SemanticKirCallReturnKindV1::Return { .. }))
                .count(),
            instances.returns(id).unwrap().count()
        );
    }
    checked(instances, emitted, slots, budget)?;
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn repeated_defined_calls_keep_distinct_shifted_instances_and_intrinsic_result() {
    for (projected, indexed) in [(false, false), (true, false), (false, true)] {
        let fixture = ScopedFixture::CallDestinations {
            projected,
            indexed,
            retained_address: false,
        };
        let (result, _, _) = run(false, fixture, observe_repeated, LIMIT, LIMIT);
        assert_eq!(OBSERVED.get(), 1, "{fixture:?}: {result:?}");
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
    }
}

#[derive(Clone, Copy, Debug)]
enum PhaseFault {
    MissingSource,
    MissingRole,
    RelabelledRole,
    WrongSite,
    Omitted,
    Duplicate,
}

fn observe_phase_faults(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    checked(instances, emitted, slots, budget)?;
    let index = callback(slots);
    let phases = [
        (
            0,
            ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallDestinationAddress),
        ),
        (
            1,
            ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallArgument(0)),
        ),
        (0, ScopedMemoryRoleV29::CallResult),
        (1, ScopedMemoryRoleV29::CallResult),
    ];
    for (source_block, role) in phases {
        let source_site = execution_site_v29(SemanticBlockIdV1::from_index(source_block), None);
        let row = emitted[index]
            .as_ref()
            .unwrap()
            .scoped_memory_anchors
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .position(|row| {
                matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. })
                    && row
                        .source
                        .is_some_and(|frame| frame.site == source_site && frame.role == Some(role))
            })
            .expect("actual address, argument, or result memory access");
        for fault in [
            PhaseFault::MissingSource,
            PhaseFault::MissingRole,
            PhaseFault::RelabelledRole,
            PhaseFault::WrongSite,
            PhaseFault::Omitted,
            PhaseFault::Duplicate,
        ] {
            let label = format!("{role:?} in block {source_block}: {fault:?}");
            reject(
                &label,
                instances,
                emitted,
                slots,
                index,
                budget,
                Refusal::Memory,
                |lowered| {
                    let rows = &mut lowered.scoped_memory_anchors.as_mut().unwrap().rows;
                    match fault {
                        PhaseFault::MissingSource => rows[row].source = None,
                        PhaseFault::MissingRole => rows[row].source.as_mut().unwrap().role = None,
                        PhaseFault::RelabelledRole => {
                            rows[row].source.as_mut().unwrap().role =
                                Some(if role == ScopedMemoryRoleV29::CallResult {
                                    ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallArgument(
                                        0,
                                    ))
                                } else {
                                    ScopedMemoryRoleV29::CallResult
                                });
                        }
                        PhaseFault::WrongSite => {
                            rows[row].source.as_mut().unwrap().site =
                                execution_site_v29(SemanticBlockIdV1::from_index(2), None)
                        }
                        PhaseFault::Omitted => {
                            rows.remove(row);
                        }
                        PhaseFault::Duplicate => rows.insert(row, rows[row]),
                    }
                },
            );
        }
    }
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn each_defined_call_memory_phase_requires_one_exact_source_role() {
    let (result, _, _) = run(
        false,
        ScopedFixture::CallDestinations {
            projected: false,
            retained_address: false,
            indexed: true,
        },
        observe_phase_faults,
        LIMIT,
        LIMIT,
    );
    assert_eq!(OBSERVED.get(), 1, "{result:?}");
    assert!(is_stopped(&result), "{result:?}");
}

#[derive(Clone, Copy, Debug)]
enum CallFault {
    MissingAnchor,
    DuplicateAnchor,
    ArgumentsAfterCall,
    CallOutsideBlock,
    ResultBeforeCall,
    ResultPastSpan,
    WrongPointer,
    WrongValue,
    WrongAccess,
    WrongSuccessor,
    SwappedChild,
    MissingBounds,
    DuplicateBounds,
    WrongBounds,
    ExtraStatementCall,
    ExtraIntrinsicCall,
}

fn observe_call_faults(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    checked(instances, emitted, slots, budget)?;
    let index = callback(slots);
    let id = instances.id_at(index).unwrap();
    let calls = instances.calls(id).unwrap();
    let other_child = calls[1].child().unwrap();
    let other_target = emitted[other_child.index()]
        .as_ref()
        .unwrap()
        .function
        .id
        .clone();
    let first_child = calls[0].child().unwrap();
    let first_target = emitted[first_child.index()]
        .as_ref()
        .unwrap()
        .function
        .id
        .clone();
    let lowered = emitted[index].as_ref().unwrap();
    let first_row = call_row(lowered, 0);
    let SemanticKirCallReturnKindV1::Call { call_operation, .. } =
        lowered.call_returns.sites.rows[first_row].kind
    else {
        unreachable!()
    };
    let call_position = call_operation as usize;
    let other_pointer = slots.slots[slots.instances[index].slots.clone()]
        .iter()
        .find(|row| row.origin.local == 0)
        .unwrap()
        .origin
        .pointer;
    for fault in [
        CallFault::MissingAnchor,
        CallFault::DuplicateAnchor,
        CallFault::ArgumentsAfterCall,
        CallFault::CallOutsideBlock,
        CallFault::ResultBeforeCall,
        CallFault::ResultPastSpan,
        CallFault::WrongPointer,
        CallFault::WrongValue,
        CallFault::WrongAccess,
        CallFault::WrongSuccessor,
        CallFault::SwappedChild,
        CallFault::MissingBounds,
        CallFault::DuplicateBounds,
        CallFault::WrongBounds,
        CallFault::ExtraStatementCall,
        CallFault::ExtraIntrinsicCall,
    ] {
        let expected = match fault {
            CallFault::MissingBounds | CallFault::DuplicateBounds | CallFault::WrongBounds => {
                Refusal::SourceRows
            }
            _ => Refusal::Correspondence,
        };
        reject(
            &format!("{fault:?}"),
            instances,
            emitted,
            slots,
            index,
            budget,
            expected,
            |lowered| match fault {
                CallFault::MissingAnchor => {
                    lowered.call_returns.sites.rows.remove(first_row);
                }
                CallFault::DuplicateAnchor => {
                    let row = lowered.call_returns.sites.rows[first_row];
                    lowered.call_returns.sites.rows.insert(first_row, row);
                }
                CallFault::ArgumentsAfterCall
                | CallFault::CallOutsideBlock
                | CallFault::ResultBeforeCall
                | CallFault::ResultPastSpan => {
                    let SemanticKirCallReturnKindV1::Call {
                        arguments_first,
                        call_operation,
                        destination_end,
                        ..
                    } = &mut lowered.call_returns.sites.rows[first_row].kind
                    else {
                        unreachable!()
                    };
                    match fault {
                        CallFault::ArgumentsAfterCall => *arguments_first = *call_operation + 1,
                        CallFault::CallOutsideBlock => *call_operation = u32::MAX,
                        CallFault::ResultBeforeCall => *destination_end = *call_operation,
                        CallFault::ResultPastSpan => *destination_end = u32::MAX,
                        _ => unreachable!(),
                    }
                }
                CallFault::WrongPointer | CallFault::WrongValue | CallFault::WrongAccess => {
                    let block = block_mut(lowered, 0);
                    let OperationKind::Call { arguments, .. } =
                        &block.operations[call_position].kind
                    else {
                        unreachable!()
                    };
                    let wrong_value = arguments[0];
                    let result = block.operations[call_position].results[0].id;
                    assert_ne!(wrong_value, result);
                    let OperationKind::Store {
                        pointer,
                        value,
                        access,
                    } = &mut block.operations[call_position + 1].kind
                    else {
                        unreachable!()
                    };
                    match fault {
                        CallFault::WrongPointer => {
                            assert_ne!(*pointer, other_pointer);
                            *pointer = other_pointer;
                        }
                        CallFault::WrongValue => *value = wrong_value,
                        CallFault::WrongAccess => access.volatile = !access.volatile,
                        _ => unreachable!(),
                    }
                }
                CallFault::WrongSuccessor => {
                    let block = block_mut(lowered, 0);
                    let Some(Terminator::Branch { target, .. }) = &mut block.terminator else {
                        unreachable!()
                    };
                    assert_ne!(*target, block.id);
                    *target = block.id;
                }
                CallFault::SwappedChild => {
                    let block = block_mut(lowered, 0);
                    let OperationKind::Call { callee, .. } =
                        &mut block.operations[call_position].kind
                    else {
                        unreachable!()
                    };
                    assert_eq!(*callee, first_target);
                    *callee = other_target.clone();
                }
                CallFault::MissingBounds | CallFault::DuplicateBounds | CallFault::WrongBounds => {
                    let spans = &mut lowered.terminator_operation_spans;
                    let position = spans
                        .iter()
                        .position(|span| span.semantic_block == SemanticBlockIdV1::from_index(0))
                        .unwrap();
                    match fault {
                        CallFault::MissingBounds => {
                            spans.remove(position);
                        }
                        CallFault::DuplicateBounds => spans.insert(position, spans[position]),
                        CallFault::WrongBounds => spans[position].operation_count += 1,
                        _ => unreachable!(),
                    }
                }
                CallFault::ExtraStatementCall => {
                    let span = lowered
                        .statement_operation_spans
                        .iter()
                        .find(|span| {
                            span.semantic_block == SemanticBlockIdV1::from_index(0)
                                && span.operation_count > 0
                        })
                        .unwrap();
                    let position = span.first_operation_ordinal as usize;
                    let operation = &mut block_mut(lowered, 0).operations[position];
                    operation.kind = OperationKind::Call {
                        callee: first_target.clone(),
                        arguments: vec![],
                    };
                }
                CallFault::ExtraIntrinsicCall => {
                    let span = lowered
                        .terminator_operation_spans
                        .iter()
                        .find(|span| span.semantic_block == SemanticBlockIdV1::from_index(2))
                        .unwrap();
                    let position = span.first_operation_ordinal as usize;
                    block_mut(lowered, 2).operations[position].kind = OperationKind::Call {
                        callee: first_target.clone(),
                        arguments: vec![],
                    };
                }
            },
        );
    }
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn defined_call_census_bounds_targets_results_and_successors_are_exact() {
    for (projected, indexed) in [(false, false), (true, false), (false, true)] {
        let (result, _, _) = run(
            false,
            ScopedFixture::CallDestinations {
                projected,
                retained_address: false,
                indexed,
            },
            observe_call_faults,
            LIMIT,
            LIMIT,
        );
        assert_eq!(OBSERVED.get(), 1, "{result:?}");
        assert!(is_stopped(&result), "{result:?}");
    }
}

fn observe_returns(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    checked(instances, emitted, slots, budget)?;
    let mut checked_returns = 0;
    for index in 0..emitted.len() {
        let returned: Vec<_> = emitted[index]
            .as_ref()
            .unwrap()
            .call_returns
            .sites
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| match row.kind {
                SemanticKirCallReturnKindV1::Return { components } => {
                    Some((index, row.semantic_block, components))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            returned.len(),
            instances
                .returns(instances.id_at(index).unwrap())
                .unwrap()
                .count()
        );
        for (position, source_block, components) in returned {
            checked_returns += 1;
            reject(
                "missing return anchor",
                instances,
                emitted,
                slots,
                index,
                budget,
                Refusal::Correspondence,
                |lowered| {
                    lowered.call_returns.sites.rows.remove(position);
                    // A scalar helper has only this return component span. Removing both
                    // tests source return coverage independently of orphan-pool detection.
                    if lowered.call_returns.sites.rows.is_empty() {
                        lowered.call_returns.components.rows.clear();
                    }
                },
            );
            reject(
                "duplicate return anchor",
                instances,
                emitted,
                slots,
                index,
                budget,
                Refusal::Correspondence,
                |lowered| {
                    let row = lowered.call_returns.sites.rows[position];
                    lowered.call_returns.sites.rows.insert(position, row);
                },
            );
            reject(
                "return replaced by unreachable",
                instances,
                emitted,
                slots,
                index,
                budget,
                Refusal::Correspondence,
                |lowered| {
                    block_mut(lowered, source_block.index()).terminator =
                        Some(Terminator::Unreachable);
                },
            );
            if components.count > 0 {
                reject(
                    "missing physical return values",
                    instances,
                    emitted,
                    slots,
                    index,
                    budget,
                    Refusal::Correspondence,
                    |lowered| {
                        let Some(Terminator::Return { values }) =
                            &mut block_mut(lowered, source_block.index()).terminator
                        else {
                            unreachable!()
                        };
                        assert!(!values.is_empty());
                        values.clear();
                    },
                );
                reject(
                    "wrong return component",
                    instances,
                    emitted,
                    slots,
                    index,
                    budget,
                    Refusal::Correspondence,
                    |lowered| {
                        let parameter =
                            lowered.function.body.as_ref().unwrap().blocks[0].parameters[0].id;
                        let CallResultComponentV1::Return { input, .. } =
                            &mut lowered.call_returns.components.rows[components.first as usize]
                        else {
                            unreachable!()
                        };
                        assert_ne!(*input, parameter);
                        *input = parameter;
                    },
                );
            }
        }
    }
    assert_eq!(checked_returns, emitted.len());
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn every_scoped_instance_return_requires_complete_physical_and_source_coverage() {
    let (result, _, _) = run(
        false,
        ScopedFixture::CallDestinations {
            projected: false,
            retained_address: false,
            indexed: false,
        },
        observe_returns,
        LIMIT,
        LIMIT,
    );
    assert_eq!(OBSERVED.get(), 1, "{result:?}");
    assert!(is_stopped(&result), "{result:?}");
}

thread_local! {
    static SHORT_WORK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_resources(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let floor = budget.storage();
    assert!(floor > FLOOR);
    let available = budget.storage_limit() - floor;
    let filler = available - 1_000_000;
    let previous_peak = budget.peak_storage();
    budget.reserve_storage(filler)?;
    let entry = budget.storage();
    assert!(entry > previous_peak);
    let before = budget.work();
    checked(instances, emitted, slots, budget)?;
    let work = budget.work() - before;
    let peak = budget.peak_storage() - entry;
    assert!(work > 0 && peak > 0);
    budget.release_storage(filler)?;
    assert_eq!(budget.storage(), floor);
    for (allowance, succeeds) in [(peak, true), (peak - 1, false)] {
        let filler = available - allowance;
        budget.reserve_storage(filler)?;
        let before = budget.work();
        let result = checked(instances, emitted, slots, budget);
        if succeeds {
            result?;
            assert_eq!(budget.work() - before, work);
        } else {
            assert_resource(result.as_ref().unwrap_err(), false);
        }
        assert_eq!(budget.storage(), floor + filler);
        budget.release_storage(filler)?;
    }
    let short = SHORT_WORK.get();
    let allowance = work - usize::from(short);
    budget.charge_work(LIMIT - budget.work() - allowance)?;
    let before = budget.work();
    let result = checked(instances, emitted, slots, budget);
    if short {
        assert_resource(result.as_ref().unwrap_err(), true);
    } else {
        result?;
        assert_eq!(budget.work() - before, work);
        assert_eq!(budget.work(), LIMIT);
    }
    assert_eq!(budget.storage(), floor);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn defined_call_validation_obeys_exact_and_one_short_shared_work_and_scratch() {
    for short in [false, true] {
        SHORT_WORK.set(short);
        let (result, _, _) = run(
            false,
            ScopedFixture::CallDestinations {
                projected: false,
                retained_address: false,
                indexed: true,
            },
            observe_resources,
            LIMIT,
            LIMIT,
        );
        assert_eq!(OBSERVED.get(), 1, "{result:?}");
        assert!(is_stopped(&result), "{result:?}");
    }
    SHORT_WORK.set(false);
}
