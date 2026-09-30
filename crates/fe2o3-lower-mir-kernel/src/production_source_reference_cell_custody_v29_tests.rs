use super::*;

// This supplies only the physical index to the private pointer-census component.
// It deliberately has no lifecycle receipt and cannot pass the complete scoped
// slot constructor. Genuine source tests exercise that independent boundary.
fn pointer_component_subject(
    plan: &SourceReferencePlanV29<'_, '_>,
    emitted: Vec<(ProductionCallInstanceIdV1, LoweredFunctionResultV1)>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        Vec<Option<LoweredFunctionResultV1>>,
        OwnedScopedSourceSlotsV29,
    ),
    ProductionSemanticKirErrorV1,
> {
    let mut dense: Vec<_> = (0..plan.instances.instances().len())
        .map(|_| None)
        .collect();
    for (instance, lowered) in emitted {
        assert!(dense[instance.index()].replace(lowered).is_none());
    }
    let mut slots = Vec::new();
    for (index, lowered) in dense.iter().enumerate() {
        let lowered = lowered.as_ref().unwrap();
        let instance = plan.instances.id_at(index).unwrap();
        let source = plan.instances.instance(instance).unwrap();
        let body = lowered.function.body.as_ref().unwrap();
        let original = scoped_slot_candidates_v29(source.declaration(), source.ssa(), budget)?;
        let mut expected = Vec::new();
        for (local, flag) in original.iter().enumerate() {
            if *flag == 2
                && !source_reference_existing_value_local_v29(plan, instance, local as u32, budget)?
            {
                expected.push(local as u32);
            }
        }
        let origins = lowered.scoped_slot_origins.as_ref().unwrap();
        assert_eq!(
            origins
                .iter()
                .map(|row| row.legacy_local().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        for &origin in origins {
            assert_eq!(
                source.declaration().locals()[origin.legacy_local().unwrap() as usize].ty(),
                origin.semantic_type
            );
            let element = private_retained_slot_facts_v1(
                plan.instances.owner().source_semantic().types(),
                origin.semantic_type,
                budget,
            )?
            .unwrap();
            let allocations: Vec<_> = body
                .blocks
                .iter()
                .enumerate()
                .flat_map(|(block_ordinal, block)| {
                    block
                        .operations
                        .iter()
                        .enumerate()
                        .filter_map(move |(operation, value)| {
                            (value
                                .results
                                .first()
                                .is_some_and(|value| value.id == origin.pointer)
                                && matches!(value.kind, OperationKind::Alloca { .. }))
                            .then_some((
                                PrivateArrayPhysicalLocationV1 {
                                    block_ordinal,
                                    block: block.id,
                                    operation,
                                },
                                value,
                            ))
                        })
                })
                .collect();
            assert_eq!(allocations.len(), 1);
            private_retained_check_allocation_operation_v1(
                allocations[0].1,
                origin.pointer,
                None,
                element,
                budget,
            )
            .map_err(scoped_slot_relation_error_v29)?;
            slots.push(ScopedSourceSlotV29 {
                instance,
                origin,
                representation: ScopedSlotRepresentationV29::ScalarArray(
                    ScopedScalarArraySlotV29 {
                        element_type: origin.semantic_type,
                        element,
                        length: 1,
                        bytes: element.size,
                        count: None,
                    },
                ),
                allocation: allocations[0].0,
            });
        }
    }
    Ok((
        dense,
        OwnedScopedSourceSlotsV29 {
            source: ExecutionCallSourceV29::from_instances(plan.instances, budget)?,
            ledger: budget.work_ledger_identity_v1(),
            instances: Vec::new(),
            slots,
            pending_memory: None,
            retained_storage: 0,
        },
    ))
}

#[test]
fn actual_emitted_cell_pointer_census_joins_all_calls_and_original_accesses() {
    cell_emission_tests::with_cell_source_lowered(
        owner(Case::Writeback),
        |plan, references, emitted, budget| {
            let (emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            let proof =
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            assert_eq!(proof.cell_slots.len(), 2);
            for (index, cell) in plan.cells.rows.iter().enumerate() {
                let slot = &slots.slots[proof.cell_slots[index].expect("actual scalar cell proof")];
                assert_eq!(
                    (slot.instance, slot.legacy_local().unwrap()),
                    (cell.instance, cell.local.index())
                );
                proof.require_cell(index, (cell.instance.index(), slot.origin.pointer), budget)?;
                let lowered = emitted[cell.instance.index()].as_ref().unwrap();
                let store = &lowered.function.body.as_ref().unwrap().blocks
                    [slot.allocation.block_ordinal]
                    .operations[slot.allocation.operation + 1];
                source_reference_cell_entry_store_v29(
                    plan,
                    cell.instance,
                    slot.origin,
                    lowered,
                    store,
                    budget,
                )?;
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn absent_scalar_cell_proof_cannot_borrow_another_rows_authority() {
    let reached = std::cell::Cell::new(false);
    cell_emission_tests::with_cell_source_lowered(
        owner(Case::Writeback),
        |plan, references, emitted, budget| {
            let (emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            let mut proof =
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            assert_eq!(proof.cell_slots.len(), plan.cells.rows.len());
            assert!(proof.cell_slots.len() >= 2);
            let cell = plan.cells.rows[0];
            let slot = &slots.slots[proof.cell_slots[0].unwrap()];
            proof.require_cell(0, (cell.instance.index(), slot.origin.pointer), budget)?;
            let sibling = proof.cell_slots[1];
            proof.cell_slots[0] = None;
            assert!(
                proof
                    .require_cell(0, (cell.instance.index(), slot.origin.pointer), budget)
                    .is_err()
            );
            assert_eq!(proof.cell_slots[1], sibling);
            reached.set(true);
            Ok(())
        },
    )
    .unwrap();
    assert!(reached.get());
}

#[test]
fn entry_cell_initialization_rejects_a_substituted_same_type_value() {
    let entered = std::cell::Cell::new(false);
    let result = cell_emission_tests::with_cell_source_lowered(
        owner(Case::Writeback),
        |plan, references, emitted, budget| {
            let (mut emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            let proof =
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            drop(proof);
            let cell = plan.cells.rows[0];
            let slot = slots
                .slots
                .iter()
                .find(|slot| {
                    slot.instance == cell.instance
                        && slot.legacy_local().unwrap() == cell.local.index()
                })
                .unwrap();
            let lowered = emitted[cell.instance.index()].as_mut().unwrap();
            let body = lowered.function.body.as_mut().unwrap();
            let replacement = ValueId(lowered.next_value);
            lowered.next_value += 1;
            let operations = &mut body.blocks[slot.allocation.block_ordinal].operations;
            operations.insert(
                slot.allocation.operation + 1,
                Operation::effect_free(
                    ValueDef::new(replacement, Type::Scalar(ScalarType::U64)),
                    OperationKind::Constant(Constant::U64(19)),
                ),
            );
            let operation = &mut operations[slot.allocation.operation + 2];
            let OperationKind::Store { value, .. } = &mut operation.kind else {
                panic!("expected real entry store")
            };
            *value = replacement;
            // Signature and parameter identity remain exact; only the initializer
            // changed to another dominating U64 value.
            entered.set(true);
            source_reference_cell_entry_store_v29(
                plan,
                cell.instance,
                slot.origin,
                lowered,
                &lowered.function.body.as_ref().unwrap().blocks[slot.allocation.block_ordinal]
                    .operations[slot.allocation.operation + 2],
                budget,
            )
        },
    );
    assert!(entered.get());
    assert!(result.is_err());
}

#[test]
fn complete_cell_census_rejects_unclaimed_memory_and_call_substitution() {
    for extra_read in [false, true] {
        let entered = std::cell::Cell::new(false);
        let result = cell_emission_tests::with_cell_source_lowered(
            owner(Case::Writeback),
            |plan, references, emitted, budget| {
                let (mut emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
                let proof = checked_source_reference_cell_pointers_v29(
                    references, &emitted, &slots, budget,
                )?;
                drop(proof);
                let cell = plan.cells.rows[0];
                let slot = slots
                    .slots
                    .iter()
                    .find(|slot| {
                        slot.instance == cell.instance
                            && slot.legacy_local().unwrap() == cell.local.index()
                    })
                    .unwrap();
                let child = plan.instances.calls(cell.instance).unwrap()[0]
                    .child()
                    .unwrap();
                if extra_read {
                    let helper = emitted[child.index()].as_mut().unwrap();
                    let pointer = helper.function.body.as_ref().unwrap().parameters[0];
                    let value = ValueId(helper.next_value);
                    helper.next_value += 1;
                    helper
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .last_mut()
                        .unwrap()
                        .operations
                        .push(Operation::effect_free(
                            ValueDef::new(value, Type::Scalar(ScalarType::U64)),
                            OperationKind::Load {
                                pointer,
                                access: MemoryAccess::new(AddressSpace::Private, 8),
                            },
                        ));
                } else {
                    let callee = emitted[child.index()].as_ref().unwrap().function.id.clone();
                    let worker = emitted[cell.instance.index()].as_mut().unwrap();
                    let body = worker.function.body.as_mut().unwrap();
                    let extra = ValueId(worker.next_value);
                    worker.next_value += 1;
                    let (call_block, call_ordinal) = body
                        .blocks
                        .iter()
                        .enumerate()
                        .find_map(|(block, row)| {
                            row.operations
                                .iter()
                                .position(|operation| {
                                    matches!(&operation.kind,
                        OperationKind::Call { callee: actual, .. } if actual == &callee)
                                })
                                .map(|ordinal| (block, ordinal))
                        })
                        .unwrap();
                    // A distinct same-type allocation dominates the substituted
                    // call input. Keep its source call span on the actual call.
                    body.blocks[call_block].operations.insert(
                        call_ordinal,
                        Operation::effect_free(
                            ValueDef::new(
                                extra,
                                Type::pointer(
                                    Type::Scalar(ScalarType::U64),
                                    AddressSpace::Private,
                                    AccessMode::ReadWrite,
                                ),
                            ),
                            OperationKind::Alloca {
                                element: Type::Scalar(ScalarType::U64),
                                count: None,
                                address_space: AddressSpace::Private,
                                alignment: 8,
                            },
                        ),
                    );
                    let call_block_id = body.blocks[call_block].id;
                    let span = worker
                        .terminator_operation_spans
                        .iter_mut()
                        .find(|span| {
                            span.kernel_ir_block == call_block_id
                                && call_ordinal >= span.first_operation_ordinal as usize
                                && call_ordinal
                                    < (span.first_operation_ordinal + span.operation_count) as usize
                        })
                        .unwrap();
                    span.operation_count += 1;
                    let call = body.blocks.iter_mut().flat_map(|block| &mut block.operations)
                    .find(|operation| matches!(&operation.kind, OperationKind::Call { callee: actual, .. } if actual == &callee)).unwrap();
                    let OperationKind::Call { arguments, .. } = &mut call.kind else {
                        unreachable!()
                    };
                    assert_eq!(arguments, &[slot.origin.pointer]);
                    arguments[0] = extra;
                }
                entered.set(true);
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)
                    .map(drop)
            },
        );
        assert!(entered.get());
        assert!(result.is_err());
    }
}

// These inert, empty subjects are only passed to custody/header rejection paths.
// No positive allocation or source-census authority is manufactured here.
fn empty_subject(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> OwnedScopedSourceSlotsV29 {
    OwnedScopedSourceSlotsV29 {
        source: ExecutionCallSourceV29::from_instances(plan.instances, budget).unwrap(),
        ledger: budget.work_ledger_identity_v1(),
        instances: Vec::new(),
        slots: Vec::new(),
        pending_memory: None,
        retained_storage: 0,
    }
}

#[test]
fn cell_pointer_constructor_header_denials_are_reached_and_sticky() {
    use std::mem::size_of;
    let header = size_of::<SourceReferenceCellPointerProofV29<'_, '_, '_>>()
        + 2 * size_of::<
            Result<SourceReferenceCellPointerProofV29<'_, '_, '_>, ProductionSemanticKirErrorV1>,
        >();
    assert_eq!(
        source_reference_emission_headers_v29::<SourceReferenceCellPointerProofV29<'_, '_, '_>>()
            .unwrap(),
        header
    );
    for free in [header - 1, header] {
        let entered = std::cell::Cell::new(false);
        let result = cells_tests::run_cells(owner(Case::Writeback), |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let slots = empty_subject(plan, budget);
            let padding = usize::MAX - budget.storage() - free;
            budget.reserve_storage(padding)?;
            let before = budget.storage();
            entered.set(true);
            let Err(error) =
                checked_source_reference_cell_pointers_v29(&references, &[], &slots, budget)
            else {
                panic!("the exact constructor header boundary was denied");
            };
            let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
                panic!("denial must precede empty subject validation");
            };
            assert!(matches!(first, ArgumentResourceV1::Storage(_)));
            assert_eq!(
                budget.storage() - before,
                if free == header { header } else { 0 }
            );
            budget.release_storage(padding)?;
            assert_eq!(plan.failure.get(), Some(first));
            let retry_work = budget.work();
            assert!(matches!(references.check(budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
            assert_eq!(budget.work(), retry_work);
            Ok(())
        });
        assert!(entered.get());
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ));
    }
}

#[test]
fn cell_consumer_rejects_foreign_ledger_before_work_or_storage_debits() {
    for consumer in 0..3 {
        let result = cells_tests::run_cells(owner(Case::Writeback), |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let slots = empty_subject(plan, budget);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let before = (foreign.work(), foreign.storage());
            let result = match consumer {
                0 => checked_source_reference_cell_pointers_v29(
                    &references,
                    &[],
                    &slots,
                    &mut foreign,
                )
                .map(|_| ()),
                1 => scoped_slot_uses_v29::check_scoped_source_slot_uses_with_references_v29(
                    plan.instances,
                    &[],
                    &slots,
                    1,
                    Some(&references),
                    &mut foreign,
                ),
                2 => check_scoped_defined_call_phases_with_references_v29(
                    plan.instances,
                    &[],
                    Some(plan),
                    &mut foreign,
                ),
                _ => unreachable!(),
            };
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!((foreign.work(), foreign.storage()), before);
            assert_eq!(plan.failure.get(), Some(ArgumentResourceV1::Accounting));
            let retry = (budget.work(), budget.storage());
            assert!(references.check(budget).is_err());
            assert_eq!((budget.work(), budget.storage()), retry);
            Ok(())
        });
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
    }
}

#[test]
fn cell_strategy_denial_is_recorded_after_exact_owner_check() {
    let entered = std::cell::Cell::new(false);
    let result = cells_tests::run_cells(owner(Case::Writeback), |plan, budget| {
        let padding = usize::MAX - budget.work() - 5;
        budget.charge_work(padding)?;
        let before = budget.work();
        entered.set(true);
        let Err(error) = plan.scalar_cell(0, budget) else {
            panic!("cell debit must be denied");
        };
        assert_eq!(
            budget.work() - before,
            5,
            "the concrete owner was checked before the cell debit"
        );
        let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
            panic!("work refusal");
        };
        assert!(matches!(first, ArgumentResourceV1::Work(_)));
        assert_eq!(plan.failure.get(), Some(first));
        assert!(matches!(plan.scalar_cell(0, budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
        Ok(())
    });
    assert!(entered.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
}

#[test]
fn exact_cell_access_census_rejects_missing_stale_and_changed_memory_claims() {
    for mutation in 0..7 {
        let entered = std::cell::Cell::new(false);
        let result = cell_emission_tests::with_cell_source_lowered(
            owner(Case::Writeback),
            |plan, references, emitted, budget| {
                let (mut emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
                let (index, row) = plan
                    .accesses
                    .iter()
                    .enumerate()
                    .find(|(index, row)| {
                        row.key.access == SourceReferenceAccessV29::Read
                            && references.cell_accesses[*index].get().is_some()
                    })
                    .unwrap();
                let mut used = references.cell_accesses[index].get().unwrap();
                match mutation {
                    0 => references.cell_accesses[index].set(None),
                    1 => {
                        used.operation = Some(used.operation.unwrap() + 1);
                        references.cell_accesses[index].set(Some(used));
                    }
                    2 => {
                        assert_eq!(plan.cells.rows.len(), 2);
                        used.cell = (used.cell + 1) % 2;
                        references.cell_accesses[index].set(Some(used));
                    }
                    _ => {
                        let body = emitted[row.key.site.instance.index()]
                            .as_mut()
                            .unwrap()
                            .function
                            .body
                            .as_mut()
                            .unwrap();
                        let operation = &mut body
                            .blocks
                            .iter_mut()
                            .find(|block| block.id == used.block)
                            .unwrap()
                            .operations[used.operation.unwrap()];
                        let OperationKind::Load { pointer, access } = &mut operation.kind else {
                            panic!("genuine read claim must name its Load");
                        };
                        match mutation {
                            3 => access.alignment = 1,
                            4 => access.volatile = true,
                            5 => access.address_space = AddressSpace::Global,
                            6 => *pointer = ValueId(u32::MAX),
                            _ => unreachable!(),
                        }
                    }
                }
                entered.set(true);
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)
                    .map(drop)
            },
        );
        assert!(
            entered.get(),
            "mutation {mutation} must reach the valid owner"
        );
        assert!(result.is_err(), "mutation {mutation} must be refused");
    }
}

#[test]
fn a_second_claim_for_the_same_original_cell_read_is_refused() {
    let entered = std::cell::Cell::new(false);
    let result = cell_emission_tests::with_cell_source_lowered(
        owner(Case::Writeback),
        |plan, references, emitted, budget| {
            let (emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            let (index, row) = plan
                .accesses
                .iter()
                .enumerate()
                .find(|(index, row)| {
                    row.key.access == SourceReferenceAccessV29::Read
                        && references.cell_accesses[*index].get().is_some()
                })
                .unwrap();
            let source = plan.instances.instance(row.key.site.instance).unwrap();
            let statement = &source.declaration().blocks()[row.key.site.block.index() as usize]
                .statements()[row.key.site.statement.unwrap()];
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!("original helper read assignment");
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =
                assignment.value().kind()
            else {
                panic!("original helper Copy read");
            };
            assert_eq!(row.key.source, place as *const SemanticPlaceV1 as usize);
            entered.set(true);
            references.claim_cell_access(
                row.key.site,
                place,
                SourceReferenceAccessV29::Read,
                references.cell_accesses[index].get().unwrap(),
                budget,
            )
        },
    );
    assert!(entered.get());
    assert!(result.is_err());
}

#[test]
fn unused_substituted_cell_return_is_still_checked() {
    let entered = std::cell::Cell::new(false);
    let result = cell_emission_tests::with_cell_source_lowered(
        cells_tests::return_alias_owner(false),
        |plan, references, emitted, budget| {
            let (mut emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            let cell = plan.cells.rows[0];
            let child = plan.instances.calls(cell.instance).unwrap()[0]
                .child()
                .unwrap();
            let helper = emitted[child.index()].as_mut().unwrap();
            let result_type = helper.function.signature.results[0].clone();
            let extra = ValueId(helper.next_value);
            helper.next_value += 1;
            let block = helper
                .function
                .body
                .as_mut()
                .unwrap()
                .blocks
                .last_mut()
                .unwrap();
            block.operations.push(Operation::effect_free(
                ValueDef::new(extra, result_type),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U64),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 8,
                },
            ));
            let Some(Terminator::Return { values }) = &mut block.terminator else {
                panic!("actual ancestor return");
            };
            values[0] = extra;
            // Remove all caller reads so result-use liveness cannot be the
            // reason this substituted result is discovered.
            let worker = emitted[cell.instance.index()].as_mut().unwrap();
            for block in &mut worker.function.body.as_mut().unwrap().blocks {
                block
                    .operations
                    .retain(|operation| !matches!(operation.kind, OperationKind::Load { .. }));
            }
            entered.set(true);
            let mut proof =
                source_reference_cell_pointer_proof_v29(references, &emitted, &slots, budget)?;
            proof.check_payloads(&emitted, budget)
        },
    );
    assert!(entered.get());
    assert!(result.is_err());
}

#[test]
fn a_grounded_cell_input_does_not_hide_an_ungrounded_cfg_alternative() {
    let entered = std::cell::Cell::new(false);
    let result = cell_emission_tests::with_cell_source_lowered(
        cells_tests::return_alias_owner(false),
        |plan, references, emitted, budget| {
            let (mut emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            let cell = plan.cells.rows[0];
            let child = plan.instances.calls(cell.instance).unwrap()[0]
                .child()
                .unwrap();
            let helper = emitted[child.index()].as_mut().unwrap();
            let pointer_type = helper.function.signature.results[0].clone();
            let joined = ValueId(helper.next_value);
            let ungrounded = ValueId(helper.next_value + 1);
            let condition = ValueId(helper.next_value + 2);
            helper.next_value += 3;
            let body = helper.function.body.as_mut().unwrap();
            let first = body.blocks.iter().map(|block| block.id.0).max().unwrap() + 1;
            let join_id = BlockId(first);
            let cycle_id = BlockId(first + 1);
            let original = body.blocks.last_mut().unwrap();
            let Some(Terminator::Return { values }) = original.terminator.take() else {
                panic!("actual ancestor return");
            };
            assert_eq!(values.len(), 1);
            original.terminator = Some(Terminator::Branch {
                target: join_id,
                arguments: values,
            });
            let mut join = BasicBlock::new(join_id);
            join.parameters
                .push(ValueDef::new(joined, pointer_type.clone()));
            join.terminator = Some(Terminator::Return {
                values: vec![joined],
            });
            let mut cycle = BasicBlock::new(cycle_id);
            cycle
                .parameters
                .push(ValueDef::new(ungrounded, pointer_type));
            cycle.operations.push(Operation::effect_free(
                ValueDef::new(condition, Type::BOOL),
                OperationKind::Constant(Constant::Bool(true)),
            ));
            cycle.terminator = Some(Terminator::ConditionalBranch {
                condition,
                then_target: cycle_id,
                then_arguments: vec![ungrounded],
                else_target: join_id,
                else_arguments: vec![ungrounded],
            });
            body.blocks.extend([join, cycle]);
            entered.set(true);
            let mut proof =
                source_reference_cell_pointer_proof_v29(references, &emitted, &slots, budget)?;
            proof.check_payloads(&emitted, budget)
        },
    );
    assert!(entered.get());
    assert!(result.is_err());
}

fn cell_assert_owner() -> ProductionSemanticSsaOwnerV1 {
    cell_assert_owner_with_direct(false)
}

fn cell_assert_owner_with_direct(direct: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |types, functions| {
        let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([211; 32]),
            SemanticLayoutIdentityV1::from_sha256([211; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ));
        let old = &functions[2];
        let diagnostic = || {
            if direct {
                return SemanticOperandV1::Copy(place(3, WORD));
            }
            SemanticOperandV1::Copy(projected(
                2,
                &[(SemanticProjectionKindV1::Dereference, WORD)],
            ))
        };
        let mut statements = old.blocks()[0].statements().to_vec();
        if direct {
            statements.extend([
                assign(
                    place(4, REFERENCE),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(3, WORD),
                    },
                ),
                assign(
                    projected(4, &[(SemanticProjectionKindV1::Dereference, WORD)]),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            WORD,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(23, 8).unwrap(),
                            ),
                        ),
                    )),
                ),
                dead(4),
            ]);
        }
        functions[2] = function(
            30,
            false,
            CAPTURE,
            old.locals().to_vec(),
            vec![
                block(
                    30,
                    statements,
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            boolean,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 1).unwrap(),
                            ),
                        )),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: diagnostic(),
                            index: diagnostic(),
                        },
                        target: SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::AssertSuccess,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    31,
                    vec![assign(
                        place(3, WORD),
                        SemanticRvalueKindV1::Use(diagnostic()),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    })
}

#[test]
fn distinct_identical_assert_operands_have_distinct_failure_only_cell_receipts() {
    cell_emission_tests::with_cell_source_lowered(
        cell_assert_owner(),
        |plan, references, emitted, budget| {
            let (emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            let mut original_places = BTreeSet::new();
            let mut events = BTreeSet::new();
            for (index, receipt) in references.cell_failure_reads.iter().enumerate() {
                let Some(receipt) = receipt.get() else {
                    continue;
                };
                let row = &plan.accesses[index];
                assert_eq!(row.key.access, SourceReferenceAccessV29::Read);
                assert!(references.cell_accesses[index].get().is_none());
                assert!(original_places.insert((row.key.site.instance.index(), row.key.source)));
                let ScopedMemoryAnchorKindV29::FailureRead { event, .. } = receipt.kind else {
                    panic!("no pointer access receipt");
                };
                assert!(events.insert((row.key.site.instance.index(), event)));
            }
            assert_eq!(
                events.len(),
                4,
                "two original message operands in each helper instance"
            );
            assert_eq!(original_places.len(), events.len());
            Ok(())
        },
    )
    .unwrap();
}

fn with_direct_failure_claim_plan_v29(
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let owner = cell_assert_owner_with_direct(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(73)?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let credit = layouts.capture_emission_credit(&owner, budget).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                    &mut layouts,
                    instances,
                    None,
                    lens,
                    budget,
                    |plan, _, budget| consume(plan, budget).map_err(Into::into),
                )
            }));
            let cleanup = credit
                .into_root_credit(&layouts, budget)
                .ok_or_else(|| ProductionSemanticKirErrorV1::from(ArgumentResourceV1::Accounting))
                .and_then(|scratch| {
                    if !layouts.permits_root_emission_refund(&owner, scratch, budget) {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    budget.release_storage(scratch).map_err(Into::into)
                });
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                result.map(|result| result.and(cleanup)),
            )
        },
    )
    .unwrap();
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    assert_eq!(budget.storage(), 73);
    result
        .expect("source callback panic must not become a successful claim")
        .and(cleanup)
        .and(demand_cleanup)
}

#[test]
fn demanded_direct_failure_occurrences_do_not_acquire_scalar_reference_cell_claims() {
    for fault in 0..4 {
        let completed = std::cell::Cell::new(false);
        let result = with_direct_failure_claim_plan_v29(|plan, budget| {
            assert!(plan.has_storage_demands);
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let mut occurrences_seen = BTreeSet::new();
            let mut places_seen = BTreeSet::new();
            for ordinal in 0..plan.instances.instances().len() {
                let instance = plan.instances.id_at(ordinal).unwrap();
                let function = plan.instances.instance(instance).unwrap().declaration();
                let occurrences = plan.instances.occurrences(instance).unwrap();
                for (event, original) in occurrences.events().iter().enumerate() {
                    let ExecutionOperandV29::AssertMessage(_) = original.operand() else {
                        continue;
                    };
                    if original.role() != ExecutionEventV29::BaseUse {
                        continue;
                    }
                    let Some(SemanticOperandV1::Copy(source)) =
                        scoped_source_operand_v29(function, original.site(), original.operand())
                    else {
                        unreachable!()
                    };
                    let ExecutionSiteV29::Terminator { block } = original.site() else {
                        unreachable!()
                    };
                    let mut site = SourceReferenceSiteV29 {
                        instance,
                        block: SemanticBlockIdV1::from_index(block.get()),
                        statement: None,
                    };
                    let index = *plan
                        .access_sites
                        .get(&source_reference_access_key_v29(
                            site,
                            source,
                            SourceReferenceAccessV29::Read,
                        ))
                        .expect("real demanded direct access");
                    assert!(plan.accesses[index].loan.is_none());
                    let mut receipt = ScopedMemoryAnchorV29 {
                        block: BlockId(block.get()),
                        position: 0,
                        source: Some(ScopedMemoryFrameV29::operand(
                            original.site(),
                            Some(original.operand()),
                        )),
                        kind: ScopedMemoryAnchorKindV29::FailureRead {
                            event,
                            local: source.local().index(),
                        },
                    };
                    // These are original source-receipt component checks, not
                    // emitted physical coordinates or completed memory admission.
                    let cloned = source.clone();
                    let queried = if fault == 1 { &cloned } else { source };
                    if fault == 2 {
                        receipt.kind = ScopedMemoryAnchorKindV29::FailureRead {
                            event: usize::MAX,
                            local: source.local().index(),
                        };
                    }
                    if fault == 3 {
                        site.statement = Some(0);
                    }
                    let result = references.claim_cell_failure_read(site, queried, receipt, budget);
                    assert!(
                        references
                            .cell_accesses
                            .iter()
                            .all(|row| row.get().is_none())
                    );
                    assert!(
                        references
                            .cell_failure_reads
                            .iter()
                            .all(|row| row.get().is_none())
                    );
                    if fault != 0 {
                        let error =
                            result.expect_err("untrusted source failure receipt must refuse");
                        let expected = if fault == 2 {
                            "scoped memory anchors differ from their source instance"
                        } else {
                            "execution call parameters differ from their source instance"
                        };
                        assert!(
                            matches!(&error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
                            if *detail == expected)
                        );
                        completed.set(true);
                        return Err(error);
                    }
                    result?;
                    assert!(occurrences_seen.insert((instance.index(), event)));
                    assert!(
                        places_seen
                            .insert((instance.index(), source as *const SemanticPlaceV1 as usize))
                    );
                }
            }
            assert_eq!(occurrences_seen.len(), 4);
            assert_eq!(places_seen.len(), 4);
            completed.set(true);
            Ok(())
        });
        assert!(completed.get(), "fault {fault}: {result:?}");
        if fault == 0 {
            result.unwrap();
        } else {
            let expected = if fault == 2 {
                "scoped memory anchors differ from their source instance"
            } else {
                "execution call parameters differ from their source instance"
            };
            assert!(
                matches!(&result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if *detail == expected)
            );
        }
    }
}

#[test]
fn borrowed_failure_receipts_still_require_unique_scalar_reference_claims() {
    let completed = std::cell::Cell::new(false);
    let result = cell_emission_tests::with_cell_source_lowered(
        cell_assert_owner(),
        |plan, references, emitted, budget| {
            let (emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
            checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
            let (index, receipt) = references
                .cell_failure_reads
                .iter()
                .enumerate()
                .find_map(|(index, row)| row.get().map(|receipt| (index, receipt)))
                .unwrap();
            let row = &plan.accesses[index];
            assert!(plan.scalar_cell(row.loan.unwrap(), budget)?.is_some());
            let instance = plan.instances.instance(row.key.site.instance).unwrap();
            let occurrences = plan.instances.occurrences(row.key.site.instance).unwrap();
            let source = checked_scoped_failure_read_v29(
                instance.declaration(),
                &occurrences,
                &receipt,
                budget,
            )?;
            let error = references
                .claim_cell_failure_read(row.key.site, source, receipt, budget)
                .expect_err("borrowed failure receipt cannot be claimed twice");
            assert!(
                matches!(&error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
                if *detail == "source reference failure read was claimed twice")
            );
            completed.set(true);
            Err(error)
        },
    );
    assert!(completed.get(), "{result:?}");
    assert!(
        matches!(&result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
        if *detail == "source reference failure read was claimed twice")
    );
}

#[test]
fn failure_receipts_cannot_replace_each_other_or_a_real_memory_access() {
    for mutation in 0..5 {
        let entered = std::cell::Cell::new(false);
        let result = cell_emission_tests::with_cell_source_lowered(
            cell_assert_owner(),
            |plan, references, emitted, budget| {
                let (mut emitted, slots) = pointer_component_subject(plan, emitted, budget)?;
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)?;
                let indices: Vec<_> = references
                    .cell_failure_reads
                    .iter()
                    .enumerate()
                    .filter_map(|(index, receipt)| receipt.get().map(|_| index))
                    .collect();
                assert_eq!(indices.len(), 4);
                let first = indices[0];
                let mut receipt = references.cell_failure_reads[first].get().unwrap();
                match mutation {
                    0 => references.cell_failure_reads[first].set(None),
                    1 => {
                        references.cell_failure_reads[indices[1]].set(Some(receipt));
                    }
                    2 => {
                        let instance = plan.accesses[first].key.site.instance.index();
                        let rows = &mut emitted[instance]
                            .as_mut()
                            .unwrap()
                            .scoped_memory_anchors
                            .as_mut()
                            .unwrap()
                            .rows;
                        let previous = rows.len();
                        rows.retain(|row| *row != receipt);
                        assert_eq!(previous, rows.len() + 1);
                    }
                    3 => {
                        receipt.position = usize::MAX;
                        references.cell_failure_reads[first].set(Some(receipt));
                    }
                    4 => {
                        let actual = references
                            .cell_accesses
                            .iter()
                            .enumerate()
                            .find(|(index, claim)| {
                                claim.get().is_some()
                                    && plan.accesses[*index].key.access
                                        == SourceReferenceAccessV29::Read
                            })
                            .map(|(index, _)| index)
                            .unwrap();
                        references.cell_failure_reads[actual].set(Some(receipt));
                    }
                    _ => unreachable!(),
                }
                entered.set(true);
                checked_source_reference_cell_pointers_v29(references, &emitted, &slots, budget)
                    .map(drop)
            },
        );
        assert!(entered.get());
        assert!(result.is_err(), "receipt mutation {mutation}");
    }
}

struct CleanupPayload {
    drops: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    remaining: usize,
}

impl Drop for CleanupPayload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.remaining != 0 {
            std::panic::panic_any(Self {
                drops: self.drops.clone(),
                remaining: self.remaining - 1,
            });
        }
    }
}

#[test]
fn both_source_reference_entries_prepay_exact_seven_work_before_headers() {
    assert_eq!(SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29, 4);
    assert_eq!(SOURCE_REFERENCE_ENTRY_WORK_V29, 7);
    cells_tests::run_cells(owner(Case::Shared), |plan, _| {
        for cells in [false, true] {
            for limit in [6, 7] {
                let entered = std::cell::Cell::new(false);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let callback = |_: &SourceReferencePlanV29<'_, '_>, _: &mut ArgumentBudgetV1<'_>| {
                    entered.set(true);
                    Ok(())
                };
                let result = if cells {
                    with_source_reference_storage_plan_v29(plan.instances, SourceReferenceStorageV29::ScalarCells, &mut budget, callback)
                } else {
                    with_source_reference_plan_v29(plan.instances, &mut budget, callback)
                };
                assert!(!entered.get());
                assert_eq!(budget.work(), if limit == 7 { 7 } else { 0 });
                assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error))
                    if if limit == 7 { matches!(error, ArgumentResourceV1::Storage(_)) }
                       else { matches!(error, ArgumentResourceV1::Work(_)) }));
            }
        }
        Ok(())
    }).unwrap();
}

#[test]
fn both_source_reference_entries_complete_finite_cleanup_before_refund() {
    cells_tests::run_cells(owner(Case::Shared), |plan, budget| {
        for cells in [false, true] {
            for remaining in 0..=4 {
                let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let floor = budget.storage();
                let callback = |_: &SourceReferencePlanV29<'_, '_>,
                                _: &mut ArgumentBudgetV1<'_>|
                 -> Result<(), ProductionSemanticKirErrorV1> {
                    std::panic::panic_any(CleanupPayload {
                        drops: drops.clone(),
                        remaining,
                    });
                };
                let result = if cells {
                    with_source_reference_storage_plan_v29(
                        plan.instances,
                        SourceReferenceStorageV29::ScalarCells,
                        budget,
                        callback,
                    )
                } else {
                    with_source_reference_plan_v29(plan.instances, budget, callback)
                };
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source reference callback panicked",
                        ..
                    })
                ));
                assert_eq!(
                    drops.load(std::sync::atomic::Ordering::SeqCst),
                    remaining + 1
                );
                assert_eq!(budget.storage(), floor);
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn first_source_reference_failure_survives_rejected_result_destructor_panic() {
    cells_tests::run_cells(owner(Case::Shared), |outer, budget| {
        for cells in [false, true] {
            let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let floor = budget.storage();
            let callback = |plan: &SourceReferencePlanV29<'_, '_>,
                            budget: &mut ArgumentBudgetV1<'_>| {
                budget.reserve_storage(37)?;
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
                assert!(plan.check_owner(plan.instances, &mut foreign).is_err());
                assert_eq!(plan.failure.get(), Some(ArgumentResourceV1::Accounting));
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                Ok(CleanupPayload {
                    drops: drops.clone(),
                    remaining: 1,
                })
            };
            let result = if cells {
                with_source_reference_storage_plan_v29(
                    outer.instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                    callback,
                )
            } else {
                with_source_reference_plan_v29(outer.instances, budget, callback)
            };
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 2);
            assert_eq!(budget.storage(), floor + 37);
            budget.release_storage(37)?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
#[cfg(target_os = "linux")]
fn bounded_source_reference_payload_exhaustion_aborts_only_its_child() {
    const MODE: &str = "FE2O3_SOURCE_REFERENCE_BOUNDED_CLEANUP_CHILD";
    const TEST: &str = "production_semantic_kir_v1::source_reference_lowering_v29_tests::cell_custody_tests::bounded_source_reference_payload_exhaustion_aborts_only_its_child";
    if let Some(mode) = std::env::var_os(MODE) {
        assert!(mode == "promoted" || mode == "cells");
        cells_tests::run_cells(owner(Case::Shared), |plan, budget| {
            let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let callback = |_: &SourceReferencePlanV29<'_, '_>,
                            _: &mut ArgumentBudgetV1<'_>|
             -> Result<(), ProductionSemanticKirErrorV1> {
                // Finite depth: an old unbounded implementation returns and
                // fails this test instead of hanging a shared machine.
                std::panic::panic_any(CleanupPayload {
                    drops,
                    remaining: 5,
                });
            };
            let _result = if mode == "cells" {
                with_source_reference_storage_plan_v29(
                    plan.instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                    callback,
                )
            } else {
                with_source_reference_plan_v29(plan.instances, budget, callback)
            };
            panic!("exhausted payload cleanup returned instead of aborting");
        })
        .unwrap();
        unreachable!();
    }
    use std::os::unix::process::ExitStatusExt;
    for mode in ["promoted", "cells"] {
        let output = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "ulimit -c 0 || exit 125; exec \"$@\"",
                "source-reference-cleanup-child",
            ])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(MODE, mode)
            .env("RUST_BACKTRACE", "0")
            .output()
            .unwrap();
        assert_eq!(
            output.status.signal(),
            Some(6),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
