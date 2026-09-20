use super::*;
use crate::production_semantic_kir_v1::scoped_slot_uses_v29;

#[path = "production_scoped_defined_calls_v29_tests.rs"]
mod defined_call_phases;

const LIMIT: usize = 10_000_000;

fn check_anchors(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        check_scoped_defined_call_phases_v29(instances, emitted, budget)?;
        for item in &slots.instances {
            check_scoped_memory_anchors_v29(
                instances,
                item,
                emitted[item.instance.index()].as_ref().unwrap(),
                &slots.slots[item.slots.clone()],
                budget,
            )?;
        }
        Ok(())
    })
}

fn observe(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let callbacks: Vec<_> = slots
        .instances
        .iter()
        .filter(|row| row.function == CALLBACK)
        .collect();
    assert_eq!(callbacks.len(), 1);
    let item = callbacks[0];
    let local_slots = &slots.slots[item.slots.clone()];
    let pointer = |local| {
        local_slots
            .iter()
            .find(|row| row.origin.local == local)
            .unwrap()
            .origin
            .pointer
    };
    let indexed = local_slots.iter().any(|row| row.origin.local == 5);
    let lowered = emitted[item.instance.index()].as_ref().unwrap();
    let rows = &lowered.scoped_memory_anchors.as_ref().unwrap().rows;
    let results: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.source
                .is_some_and(|frame| frame.role == Some(ScopedMemoryRoleV29::CallResult))
        })
        .collect();
    assert_eq!(results.len(), 3);
    for (index, row) in results.iter().enumerate() {
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == row.block)
            .unwrap();
        let expected = if index == 0 && indexed {
            let gep = block.operations[..row.position].iter().rev().find(|op|
                matches!(op.kind, OperationKind::GetElementPointer { base, .. } if base == pointer(4))
            ).unwrap();
            assert_eq!(gep.results.len(), 1);
            gep.results[0].id
        } else {
            pointer(if index == 1 { 0 } else { 3 })
        };
        assert_eq!(
            row.source.unwrap().site,
            execution_site_v29(SemanticBlockIdV1::from_index(index as u32), None)
        );
        assert_eq!(
            row.kind,
            ScopedMemoryAnchorKindV29::Access { pointer: expected }
        );
        assert!(matches!(block.operations[row.position].kind,
            OperationKind::Store { pointer, .. } if pointer == expected));
        if index < 2 {
            assert!(
                block.operations[..row.position]
                    .iter()
                    .any(|op| matches!(op.kind, OperationKind::Call { .. }))
            );
        } else {
            assert!(
                !block
                    .operations
                    .iter()
                    .any(|op| matches!(op.kind, OperationKind::Call { .. }))
            );
        }
    }
    let addresses: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.source.is_some_and(|frame| {
                frame.role
                    == Some(ScopedMemoryRoleV29::Operand(
                        ExecutionOperandV29::CallDestinationAddress,
                    ))
            })
        })
        .collect();
    assert_eq!(addresses.len(), usize::from(indexed));
    if let Some(row) = addresses.first() {
        assert_eq!(
            row.kind,
            ScopedMemoryAnchorKindV29::Access {
                pointer: pointer(5)
            }
        );
        assert_eq!(
            row.source.unwrap().site,
            execution_site_v29(SemanticBlockIdV1::from_index(0), None)
        );
        assert!(row.position < results[0].position);
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == row.block)
            .unwrap();
        let call = block
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Call { .. }))
            .unwrap();
        assert!(row.position < call);
    }
    let moved: Vec<_> = rows
        .iter()
        .filter(|row| {
            matches!(
                row.kind,
                ScopedMemoryAnchorKindV29::Kill {
                    local: 3,
                    cause: ScopedMemoryKillV29::Move,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(moved.len(), 1);
    assert_eq!(
        moved[0].source.unwrap().role,
        Some(ScopedMemoryRoleV29::Operand(
            ExecutionOperandV29::CallArgument(0)
        ))
    );
    assert!(moved[0].position < results[1].position);
    let floor = budget.storage();
    check_anchors(instances, emitted, slots, budget)?;
    assert_eq!(budget.storage(), floor);
    let result = scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    );
    assert_eq!(budget.storage(), floor);
    if indexed {
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped slot cell requires an exact unsigned constant offset",
                    ..
                })
            ),
            "{result:?}"
        );
    } else {
        result?;
    }
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn defined_intrinsic_and_projected_call_results_keep_distinct_source_phases() {
    for (projected, indexed) in [(false, false), (true, false), (false, true)] {
        let fixture = ScopedFixture::CallDestinations {
            projected,
            retained_address: false,
            indexed,
        };
        let (result, _, _) = run(false, fixture, observe, LIMIT, LIMIT);
        assert_eq!(OBSERVED.get(), 1, "{fixture:?}: {result:?}");
        assert!(is_stopped(&result), "{fixture:?}: {result:?}");
    }
}

fn mutate(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    check_anchors(instances, emitted, slots, budget)?;
    let item = slots
        .instances
        .iter()
        .find(|row| row.function == CALLBACK)
        .unwrap();
    for address in [false, true] {
        let rows = &mut emitted[item.instance.index()]
            .as_mut()
            .unwrap()
            .scoped_memory_anchors
            .as_mut()
            .unwrap()
            .rows;
        let index = rows
            .iter()
            .position(|row| {
                row.source.is_some_and(|frame| {
                    frame.role
                        == Some(if address {
                            ScopedMemoryRoleV29::Operand(
                                ExecutionOperandV29::CallDestinationAddress,
                            )
                        } else {
                            ScopedMemoryRoleV29::CallResult
                        })
                })
            })
            .unwrap();
        let original = rows[index];
        rows[index].source.as_mut().unwrap().role = Some(if address {
            ScopedMemoryRoleV29::CallResult
        } else {
            ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallDestinationAddress)
        });
        assert!(matches!(
            check_anchors(instances, emitted, slots, budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "scoped memory anchors differ from their source instance",
            })
        ));
        emitted[item.instance.index()]
            .as_mut()
            .unwrap()
            .scoped_memory_anchors
            .as_mut()
            .unwrap()
            .rows[index] = original;
    }
    check_anchors(instances, emitted, slots, budget)?;
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn result_and_address_phases_cannot_be_swapped_on_actual_memory_operations() {
    let fixture = ScopedFixture::CallDestinations {
        projected: false,
        retained_address: false,
        indexed: true,
    };
    let (result, _, _) = run(false, fixture, mutate, LIMIT, LIMIT);
    assert_eq!(OBSERVED.get(), 1, "{result:?}");
    assert!(is_stopped(&result), "{result:?}");
}

#[test]
fn retaining_a_private_address_still_requires_representation_refinement() {
    let fixture = ScopedFixture::CallDestinations {
        projected: true,
        retained_address: true,
        indexed: false,
    };
    let (result, _, _) = run(false, fixture, observe, LIMIT, LIMIT);
    assert_eq!(OBSERVED.get(), 0);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 2,
                block: Some(0),
                statement: Some(2),
                detail: "retained-local value type differs from its private slot",
            })
        ),
        "{result:?}"
    );
}
