use super::*;

#[test]
fn actual_scalar_cell_payloads_bind_the_original_dereference_in_both_helper_instances() {
    let completed = std::cell::Cell::new(false);
    let result = with_cell_source_lowered(
        writeback_reader_owner(),
        |plan, emission, emitted, budget| {
            assert_eq!(plan.cells.rows.len(), 2);
            let mut helper_instances = BTreeSet::new();
            let mut helper_pointers = BTreeSet::new();
            for (cell_index, cell) in plan.cells.rows.iter().enumerate() {
                assert!(plan.loans.iter().enumerate().any(|(loan, row)| {
                    row.site.instance == cell.instance
                        && matches!(plan.cells.strategies[loan],
                            SourceReferenceCellStrategyV29::Scalar(index) if index == cell_index)
                }));
                let worker = &emitted.iter().find(|(id, _)| *id == cell.instance).unwrap().1;
                let worker_source = plan.instances.instance(cell.instance).unwrap().declaration();
                let worker_occurrences = plan.instances.occurrences(cell.instance).unwrap();
                let slot = worker.scoped_slot_origins.as_ref().unwrap().iter()
                    .find(|slot| slot.legacy_local().unwrap() == cell.local.index()).unwrap();
                let entry_rows: Vec<_> = worker.scoped_memory_anchors.as_ref().unwrap().rows.iter()
                    .filter(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::Access {
                        pointer,
                        payload: Some(ScopedMemoryPayloadV29::Store {
                            source: ScopedMemoryStoreSourceV29::EntryArgument { local, ty }, ..
                        }),
                    } if pointer == slot.pointer && local == cell.local && ty == WORD))
                    .collect();
                assert_eq!(entry_rows.len(), 1);
                let entry = entry_rows[0];
                assert!(entry.source.is_none());
                let worker_block = worker.function.body.as_ref().unwrap().blocks.iter()
                    .find(|block| block.id == entry.block).unwrap();
                check_scoped_payload_v29(worker_source, &worker_occurrences, entry,
                    &worker_block.operations[entry.position], budget)?;

                let child = plan.instances.calls(cell.instance).unwrap()[0].child().unwrap();
                assert!(helper_instances.insert(child.index()));
                let helper = &emitted.iter().find(|(id, _)| *id == child).unwrap().1;
                assert_eq!(helper.source_call_instance, Some(child));
                let source = plan.instances.instance(child).unwrap().declaration();
                let occurrences = plan.instances.occurrences(child).unwrap();
                let body = helper.function.body.as_ref().unwrap();
                assert_eq!(body.parameters.len(), 1);
                let pointer = body.parameters[0];
                assert!(helper_pointers.insert(pointer));
                let site = ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(0),
                    statement: 1,
                };
                let SemanticStatementKindV1::Assign(assignment) =
                    source.blocks()[0].statements()[1].kind()
                else {
                    panic!("original scalar-cell read assignment missing");
                };
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(original)) =
                    assignment.value().kind()
                else {
                    panic!("original scalar-cell read is not a Copy");
                };
                assert_eq!(original.local().index(), 2);
                assert_eq!(original.projections().len(), 1);
                assert_eq!(original.projections()[0].kind(), SemanticProjectionKindV1::Dereference);
                assert_eq!(original.ty(), WORD);
                assert_eq!(body.blocks.iter().flat_map(|block| &block.operations).filter(|operation|
                    matches!(operation.kind, OperationKind::Load { pointer: actual, .. }
                        if actual == pointer)).count(), 1);
                let mut reads = 0;
                for row in &helper.scoped_memory_anchors.as_ref().unwrap().rows {
                    if !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { pointer: actual, .. }
                        if actual == pointer)
                    {
                        continue;
                    }
                    let block = body.blocks.iter().find(|block| block.id == row.block).unwrap();
                    let Some(operation) = block.operations.get(row.position) else {
                        continue;
                    };
                    if !matches!(operation.kind, OperationKind::Load { pointer: actual, .. }
                        if actual == pointer)
                    {
                        continue;
                    }
                    let ScopedMemoryAnchorKindV29::Access {
                        pointer: actual,
                        payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
                    } = row.kind else {
                        panic!("actual scalar-cell Load lacks its complete source payload");
                    };
                    reads += 1;
                    assert_eq!(actual, pointer);
                    assert_eq!(read.site, site);
                    assert_eq!(read.role, ExecutionOperandV29::RvalueOperand(0));
                    assert_eq!(read.prefix, 1);
                    assert_eq!(read.ty, WORD);
                    assert!(std::ptr::eq(
                        scoped_payload_place_v29(source, read.site, read.role).unwrap(), original));
                    let ScopedMemoryOccurrenceV29::Promoted { event, definition } = read.occurrence else {
                        panic!("original reference holder must use its promoted SSA definition");
                    };
                    let occurrence = &occurrences.events()[event];
                    assert!(occurrence.is_reachable() && occurrence.is_promoted());
                    assert_eq!(occurrence.site(), site);
                    assert_eq!(occurrence.operand(), read.role);
                    assert_eq!(occurrence.role(), ExecutionEventV29::BaseUse);
                    assert_eq!(occurrence.event().variable().get(), 2);
                    assert_eq!(occurrence.resolved(), Some(SsaResolvedEventV1::Use {
                        variable: fe2o3_mir_model::SsaVariableIdV1::new(2),
                        value: definition,
                    }));
                    assert_eq!(operation.results.len(), 1);
                    assert_eq!(operation.results[0].id, result);
                    check_scoped_payload_v29(source, &occurrences, row, operation, budget)?;
                    assert!(plan.accesses.iter().zip(&emission.cell_accesses).any(|(access, claim)|
                        access.key.site.instance == child
                            && access.key.access == SourceReferenceAccessV29::Read
                            && claim.get().is_some_and(|claim|
                                claim.cell == cell_index && claim.pointer == pointer)));
                }
                assert_eq!(reads, 1);
            }
            assert_eq!(helper_instances.len(), 2);
            assert_eq!(helper_pointers.len(), 2);
            plan.check_owner(plan.instances, budget)?;
            verify_component(&emitted);
            completed.set(true);
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(completed.get(), "all scalar-cell payload assertions must finish");
}

// Real semantic-to-KIR emission, distinct from the unchanged rustc source gates.
pub(super) fn with_cell_source_lowered(
    owner: ProductionSemanticSsaOwnerV1,
    inspect: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &SourceReferenceEmissionV29<'_, '_>,
        Vec<(ProductionCallInstanceIdV1, LoweredFunctionResultV1)>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_cell_source_lowered_cursor(owner, |_| {}, inspect)
}

pub(super) fn with_cell_source_lowered_cursor(
    owner: ProductionSemanticSsaOwnerV1,
    mut before: impl FnMut(&mut ExecutionAvailabilityV29<'_>),
    inspect: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &SourceReferenceEmissionV29<'_, '_>,
        Vec<(ProductionCallInstanceIdV1, LoweredFunctionResultV1)>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let next_placement = |lowered: &LoweredFunctionResultV1| SemanticEmissionPlacementV1 {
        first_block: lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .map(|block| block.id.0)
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
        first_value: lowered.next_value,
    };
    cells_tests::run_cells(owner, |references, budget| {
        let emission = SourceReferenceEmissionV29::new(references, budget)?;
        let instances = references.instances;
        let semantic = instances.owner().source_semantic();
        with_execution_instance_layouts_v29(references, budget, |signatures, budget| {
        with_execution_call_scope_v29(budget, |scope, budget| {
            let mut closure = ReachableClosureBudgetV1::new(16_384);
            let root = kernel_entry_plan_v1(
                semantic,
                ROOT,
                ROOT,
                FunctionId::new("source_reference_component"),
                16_384,
                &mut closure,
            )?;
            let mut private = PrivateArrayLazyBudgetV1::new(1, 16_384);
            let mut sink = ExecutionDefinedCallSinkV29::new(scope, instances, budget)?;
            let lowered = with_source_reference_availability_v29(
                instances,
                instances.root(),
                Some(&emission),
                budget,
                |mut cursor, budget| {
                    before(&mut cursor);
                    lower_one_semantic_function_with_calls_v29(
                        semantic,
                        &root,
                        instances.instance(instances.root()).unwrap().ssa(),
                        &BTreeMap::new(),
                        signatures,
                        None,
                        BTreeSet::new(),
                        1,
                        true,
                        16_384,
                        None,
                        &mut private,
                        None,
                        budget,
                        SemanticEmissionPlacementV1::default(),
                        Some(cursor),
                        Some(&mut sink),
                        None,
                    )
                },
            )?;
            let mut position = next_placement(&lowered);
            let mut emitted = vec![(instances.root(), lowered)];
            while let Some(pending) = sink.pop_pending() {
                let child = pending.child;
                let plan = signatures.instance_plan_v29(
                    instances,
                    child,
                    pending.kernel_ir_function,
                    position,
                    budget,
                )?;
                let (arguments, parameters) = prepare_execution_parameters_with_references_v29(
                    instances,
                    child,
                    pending.arguments,
                    &plan,
                    Some(&emission),
                    budget,
                )?;
                let incoming = instances.incoming(child).unwrap();
                let caller = emitted
                    .iter()
                    .find(|(id, _)| *id == incoming.occurrence().caller)
                    .expect("original caller was emitted first");
                let original_call = incoming.source();
                assert_eq!(original_call.arguments().len(), 1);
                let calls: Vec<_> = caller
                    .1
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match &operation.kind {
                        OperationKind::Call { callee, arguments }
                            if callee == &plan.kernel_ir_function =>
                        {
                            Some(arguments)
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(calls, [&arguments]);
                let lowered = with_source_reference_availability_v29(
                    instances,
                    child,
                    Some(&emission),
                    budget,
                    |mut cursor, budget| {
                        before(&mut cursor);
                        lower_one_semantic_function_with_calls_v29(
                            semantic,
                            &plan,
                            instances.instance(child).unwrap().ssa(),
                            &BTreeMap::new(),
                            signatures,
                            None,
                            BTreeSet::new(),
                            1,
                            false,
                            16_384,
                            None,
                            &mut private,
                            None,
                            budget,
                            position,
                            Some(cursor.with_call_parameters_v29(parameters)?),
                            Some(&mut sink),
                            None,
                        )
                    },
                )?;
                assert_eq!(lowered.source_call_instance, Some(child));
                position = next_placement(&lowered);
                emitted.push((child, lowered));
            }
            sink.finish(emitted.iter().map(|(_, row)| row), instances, budget)?;
            emission.finish(budget)?;
            assert_eq!(emitted.len(), instances.instances().len());
            inspect(references, &emission, emitted, budget)
        })
        })
    })
}

fn writeback_reader_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        let old = &functions[1];
        let mut locals = old.locals().to_vec();
        locals.push(local(241, WORD, SemanticLocalRoleV1::Temporary));
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                old.blocks()[0].clone(),
                block(
                    21,
                    vec![assign(
                        place(5, WORD),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    })
}

fn verify_component(emitted: &[(ProductionCallInstanceIdV1, LoweredFunctionResultV1)]) {
    let mut module = Module::new("source_reference_cells_component");
    module.functions = emitted
        .iter()
        .map(|(_, row)| row.function.clone())
        .collect();
    module.kernels.push(Kernel::new(
        "source_reference_component",
        "source_reference_component",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    verify_module(&module).unwrap();
}

#[test]
fn actual_scalar_cells_keep_one_backing_through_helper_write_and_caller_read() {
    with_cell_source_lowered(writeback_reader_owner(), |plan, emission, emitted, budget| {
        assert_eq!(plan.cells.rows.len(), 2);
        let mut distinct = BTreeSet::new();
        for (cell_index, cell) in plan.cells.rows.iter().enumerate() {
            let source = plan.instances.instance(cell.instance).unwrap();
            assert_eq!(source.declaration().locals()[cell.local.index() as usize].ty(), WORD);
            assert_eq!(source.declaration().locals()[cell.local.index() as usize].role(), SemanticLocalRoleV1::Argument(0));
            let worker = &emitted.iter().find(|(id, _)| *id == cell.instance).unwrap().1;
            let slot = worker.scoped_slot_origins.as_ref().unwrap().iter()
                .find(|slot| slot.legacy_local().unwrap() == cell.local.index()).unwrap();
            assert_eq!(slot.semantic_type, cell.ty);
            assert!(distinct.insert(slot.pointer));
            let child = plan.instances.calls(cell.instance).unwrap()[0].child().unwrap();
            let helper = &emitted.iter().find(|(id, _)| *id == child).unwrap().1;
            let pointer_type = Type::pointer(Type::Scalar(ScalarType::U64), AddressSpace::Private, AccessMode::ReadWrite);
            assert_eq!(helper.function.signature.parameters, [pointer_type]);
            let helper_body = helper.function.body.as_ref().unwrap();
            assert_eq!(helper_body.parameters.len(), 1);
            let pointer = helper_body.parameters[0];
            let helper_operations: Vec<_> = helper_body.blocks.iter().flat_map(|block| &block.operations).collect();
            assert_eq!(helper_operations.iter().filter(|op| matches!(op.kind, OperationKind::Load { pointer: actual, .. } if actual == pointer)).count(), 1);
            assert_eq!(helper_operations.iter().filter(|op| matches!(op.kind, OperationKind::Store { pointer: actual, .. } if actual == pointer)).count(), 1);
            assert!(helper_operations.iter().all(|op| !matches!(op.kind, OperationKind::Alloca { .. })));
            let body = worker.function.body.as_ref().unwrap();
            let call = body.blocks.iter().flat_map(|block| &block.operations).find(|op|
                matches!(&op.kind, OperationKind::Call { callee, .. } if callee == &helper.function.id)).unwrap();
            assert!(matches!(&call.kind, OperationKind::Call { arguments, .. } if arguments == &[slot.pointer]));
            let read = worker.statement_operation_spans.iter().find(|span|
                span.semantic_block.index() == 1 && span.statement_ordinal == 0).unwrap();
            let block = body.blocks.iter().find(|block| block.id == read.kernel_ir_block).unwrap();
            let operations = &block.operations[read.first_operation_ordinal as usize..
                (read.first_operation_ordinal + read.operation_count) as usize];
            assert_eq!(operations.iter().filter(|operation|
                matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == slot.pointer)).count(), 1);
            let claims: Vec<_> = plan.accesses.iter().zip(&emission.cell_accesses)
                .filter_map(|(row, claim)| claim.get().filter(|claim| claim.cell == cell_index).map(|claim| (row, claim))).collect();
            assert!(claims.iter().any(|(row, claim)| row.key.access == SourceReferenceAccessV29::Write
                && claim.pointer == pointer && row.key.site.instance == child));
            assert!(claims.iter().any(|(row, claim)| matches!(row.key.access, SourceReferenceAccessV29::Borrow(_))
                && claim.pointer == slot.pointer && claim.operation.is_none()));
            plan.check_owner(plan.instances, budget)?;
        }
        verify_component(&emitted);
        Ok(())
    }).unwrap();
}

#[test]
fn actual_later_mutating_instance_uses_its_own_abi_without_sharing_storage() {
    with_cell_source_lowered(cells_tests::mixed_strategy_owner(), |plan, _, emitted, budget| {
        assert_eq!(plan.cells.rows.len(), 1);
        assert_eq!(plan.loans.len(), 2);
        let mut origins = BTreeSet::new();
        let mut helpers = Vec::new();
        for (loan, row) in plan.loans.iter().enumerate() {
            let origin = &plan.origins[row.origin];
            assert!(origins.insert((origin.instance.index(), origin.local.index(), origin.generation)));
            let worker = &emitted.iter().find(|(id, _)| *id == row.site.instance).unwrap().1;
            let child = plan.instances.calls(row.site.instance).unwrap()[0].child().unwrap();
            let helper = &emitted.iter().find(|(id, _)| *id == child).unwrap().1;
            assert_eq!(helper.source_call_instance, Some(child));
            helpers.push(&helper.function.signature);
            let calls: Vec<_> = worker.function.body.as_ref().unwrap().blocks.iter()
                .flat_map(|block| &block.operations).filter_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, arguments } if callee == &helper.function.id => Some(arguments),
                    _ => None,
                }).collect();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].len(), 1);
            if let Some((_, cell)) = plan.scalar_cell(loan, budget)? {
                let slot = worker.scoped_slot_origins.as_ref().unwrap().iter()
                    .find(|slot| slot.legacy_local().unwrap() == cell.local.index()).unwrap();
                assert_eq!(calls[0].as_slice(), [slot.pointer]);
                assert!(row.effects.referent_writes > 0);
            } else {
                plan.require_promoted(loan, budget)?;
                assert_eq!(row.effects.referent_writes, 0);
                assert!(worker.execution_observation.as_ref().unwrap().bindings.values().any(|binding|
                    matches!(binding, SemanticValueBindingV1::SourceReference(binding)
                        if binding.origin == SourceReferenceBindingOriginV29::SingleLoan(loan) && binding.values.len() == 1
                        && binding.values[0].id == calls[0][0]
                        && binding.values[0].ty == Type::Scalar(ScalarType::U64))));
            }
            plan.check_owner(plan.instances, budget)?;
        }
        assert_eq!(helpers[0].parameters, [Type::Scalar(ScalarType::U64)]);
        assert_eq!(helpers[1].parameters, [Type::pointer(Type::Scalar(ScalarType::U64),
            AddressSpace::Private, AccessMode::ReadWrite)]);
        assert_eq!(helpers[0].results, helpers[1].results);
        verify_component(&emitted);
        Ok(())
    }).unwrap();
}

#[test]
fn actual_ancestor_return_carries_the_same_cell_pointer_back_to_caller() {
    with_cell_source_lowered(cells_tests::return_alias_owner(false), |plan, _, emitted, _| {
        assert_eq!(plan.cells.rows.len(), 2);
        for cell in &plan.cells.rows {
            let worker = &emitted.iter().find(|(id, _)| *id == cell.instance).unwrap().1;
            let child = plan.instances.calls(cell.instance).unwrap()[0].child().unwrap();
            let helper = &emitted.iter().find(|(id, _)| *id == child).unwrap().1;
            let body = helper.function.body.as_ref().unwrap();
            assert_eq!(helper.function.signature.results, helper.function.signature.parameters);
            assert!(body.blocks.iter().any(|block| matches!(&block.terminator,
                Some(Terminator::Return { values }) if values == &body.parameters)));
            let call = worker.function.body.as_ref().unwrap().blocks.iter().flat_map(|block| &block.operations)
                .find(|operation| matches!(&operation.kind, OperationKind::Call { callee, .. } if callee == &helper.function.id)).unwrap();
            assert_eq!(call.results.len(), 1);
            assert!(worker.function.body.as_ref().unwrap().blocks.iter().flat_map(|block| &block.operations).any(|operation|
                matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == call.results[0].id)));
        }
        verify_component(&emitted);
        Ok(())
    }).unwrap();
}

#[test]
fn actual_mutating_reference_loop_keeps_the_cell_and_real_backedge() {
    let owner = owner_with(Case::Writeback, |_, functions| {
        let old = &functions[1];
        let mut locals = old.locals().to_vec();
        locals.extend([
            local(240, WORD, SemanticLocalRoleV1::Temporary),
            local(241, WORD, SemanticLocalRoleV1::Temporary),
        ]);
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let mut start = vec![assign(
            place(5, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
        )];
        start.extend_from_slice(old.blocks()[0].statements());
        let referent = || {
            projected(
                3,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            )
        };
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                block(
                    20,
                    start,
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    21,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(5, WORD)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    22,
                    vec![
                        assign(
                            referent(),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    WORD,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(17, 8).unwrap(),
                                    ),
                                ),
                            )),
                        ),
                        assign(
                            place(6, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(referent())),
                        ),
                        assign(
                            place(5, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    WORD,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(0, 8).unwrap(),
                                    ),
                                ),
                            )),
                        ),
                    ],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    23,
                    vec![],
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 4),
                ),
                block(24, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    });
    with_cell_source_lowered(owner, |plan, emission, emitted, _| {
        assert_eq!(plan.cells.rows.len(), 2);
        for (cell_index, cell) in plan.cells.rows.iter().enumerate() {
            let worker = &emitted.iter().find(|(id, _)| *id == cell.instance).unwrap().1;
            let body = worker.function.body.as_ref().unwrap();
            let slot = worker.scoped_slot_origins.as_ref().unwrap().iter().find(|slot| slot.legacy_local().unwrap() == cell.local.index()).unwrap();
            let header = worker.blocks.iter().find(|row| row.semantic_block.index() == 1).unwrap().kernel_ir_block;
            let latch = worker.blocks.iter().find(|row| row.semantic_block.index() == 2).unwrap().kernel_ir_block;
            let latch = body.blocks.iter().find(|block| block.id == latch).unwrap();
            assert!(matches!(&latch.terminator, Some(Terminator::Branch { target, .. }) if *target == header));
            assert!(latch.operations.iter().any(|operation| matches!(operation.kind, OperationKind::Store { pointer, .. } if pointer == slot.pointer)));
            assert!(latch.operations.iter().any(|operation| matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == slot.pointer)));
            for access in [SourceReferenceAccessV29::Read, SourceReferenceAccessV29::Write] {
                assert!(plan.accesses.iter().zip(&emission.cell_accesses).any(|(row, claim)|
                    row.key.site.instance == cell.instance && row.key.site.block.index() == 2 && row.key.access == access
                    && claim.get().is_some_and(|claim| claim.cell == cell_index && claim.pointer == slot.pointer)));
            }
        }
        verify_component(&emitted);
        Ok(())
    }).unwrap();
}
