use super::*;

pub(super) fn with_source_lowered_cursor(
    owner: ProductionSemanticSsaOwnerV1,
    mut before: impl FnMut(&mut ExecutionAvailabilityV29<'_>),
    inspect: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &[(ProductionCallInstanceIdV1, LoweredFunctionResultV1)],
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
    run_owner(owner, |references, budget| {
        let emission = SourceReferenceEmissionV29::new(references, budget)?;
        let instances = references.instances;
        let semantic = instances.owner().source_semantic();
        with_execution_call_scope_v29(budget, |scope, budget| {
            let mut signatures = BTreeMap::new();
            for index in 1..instances.instances().len() {
                let instance = instances.id_at(index).unwrap();
                let function = instances.instance(instance).unwrap().function();
                let signature = execution_function_signature_with_references_v29(
                    instances,
                    instance,
                    Some(references),
                    budget,
                )?;
                if let Some(previous) = signatures.get(&function) {
                    source_reference_same_signature_v29(previous, &signature, budget)?;
                } else {
                    signatures.insert(function, signature);
                }
            }
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
                    lower_one_source_function_with_calls_v29(
                        semantic,
                        &root,
                        instances.instance(instances.root()).unwrap().ssa(),
                        &BTreeMap::new(),
                        &signatures,
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
                let plan = execution_instance_plan_with_references_v29(
                    instances,
                    child,
                    pending.kernel_ir_function,
                    position,
                    Some(references),
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
                        lower_one_source_function_with_calls_v29(
                            semantic,
                            &plan,
                            instances.instance(child).unwrap().ssa(),
                            &BTreeMap::new(),
                            &signatures,
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
            inspect(references, &emitted, budget)
        })
    })
}

fn explicit_load_owner(
    case: Case,
    aggregate: bool,
    count: usize,
    volatility: SemanticVolatilityV1,
    atomic: Option<SemanticAtomicAccessV1>,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with(case, |_, functions| {
        let original = &functions[2];
        let old = original.blocks()[0].statements();
        assert_eq!(old.len(), 3 + usize::from(case == Case::Writeback));
        assert!(
            matches!(old[1].kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_))))
        );
        let source = if aggregate {
            projected(
                1,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            )
        } else {
            projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)])
        };
        let mut statements = vec![old[0].clone()];
        for _ in 0..count {
            statements.push(assign(
                place(3, WORD),
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    source.clone(),
                    volatility,
                    atomic,
                )),
            ));
        }
        statements.extend_from_slice(&old[2..]);
        functions[2] = function(
            30,
            false,
            CAPTURE,
            original.locals().to_vec(),
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

#[test]
fn explicit_reference_loads_consume_each_source_occurrence_in_each_instance() {
    for case in [Case::Shared, Case::UniqueRead] {
        for aggregate in [false, true] {
            for count in [1, 2] {
                let mut checked = 0;
                with_source_lowered(
                    explicit_load_owner(case, aggregate, count, SemanticVolatilityV1::NonVolatile, None),
                    |plan, emitted, _| {
                        for (instance, lowered) in emitted {
                            let source = plan.instances.instance(*instance).unwrap();
                            if source.function().index() != 2 {
                                continue;
                            }
                            checked += 1;
                            let body = lowered.function.body.as_ref().unwrap();
                            let observation = lowered.execution_observation.as_ref().unwrap();
                            assert_eq!(
                                observation.locals[3].as_ref().unwrap().value().unwrap(),
                                (body.parameters[0], Type::Scalar(ScalarType::U64))
                            );
                            assert!(!body.blocks.iter().flat_map(|block| &block.operations)
                                .any(|operation| matches!(operation.kind, OperationKind::Load { .. })));
                            assert_eq!(source.declaration().blocks()[0].statements().iter()
                                .filter(|statement| matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(_))))
                                .count(), count);
                            let definitions: Vec<_> = source.ssa().plan()
                                .resolved_events(SsaBlockIdV1::new(0)).unwrap().iter()
                                .filter_map(|(_, event)| match event {
                                    SsaResolvedEventV1::Define { variable, value }
                                        if variable.get() == 3 => Some(*value),
                                    _ => None,
                                }).collect();
                            assert_eq!(definitions.len(), count);
                            assert!(definitions.windows(2).all(|pair| pair[0] != pair[1]));
                        }
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(checked, 2, "both original helper instances must be emitted");
            }
        }
    }
}

#[test]
fn explicit_reference_loads_reject_missing_duplicate_and_unindexed_uses() {
    with_source_lowered(
        explicit_load_owner(
            Case::Shared,
            false,
            2,
            SemanticVolatilityV1::NonVolatile,
            None,
        ),
        |_, _, _| Ok(()),
    )
    .unwrap();
    for ordinal in 0..2 {
        for mutation in 0..3 {
            let entered = std::cell::Cell::new(false);
            let result = with_source_lowered_cursor(
                explicit_load_owner(
                    Case::Shared,
                    false,
                    2,
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
                |cursor| {
                    if cursor.function_id.index() != 2 {
                        return;
                    }
                    let events: Vec<_> = cursor
                        .occurrences
                        .events()
                        .iter()
                        .enumerate()
                        .filter(|(_, event)| {
                            event.operand() == ExecutionOperandV29::RvaluePlace
                                && event.role() == ExecutionEventV29::BaseUse
                        })
                        .map(|(index, _)| index)
                        .collect();
                    assert_eq!(events.len(), 2);
                    let event = events[ordinal];
                    match mutation {
                        0 => cursor.skipped_event = Some(event),
                        1 => cursor.claimed[event] = true,
                        2 => {
                            let before = cursor.index.len();
                            cursor.index.retain(|row| row.index != event);
                            assert_eq!(cursor.index.len() + 1, before);
                        }
                        _ => unreachable!(),
                    }
                    entered.set(true);
                },
                |_, _, _| panic!("invalid load event reached completed emission"),
            );
            assert!(entered.get());
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "execution availability differs from its source SSA instance"),
                "load {ordinal}, mutation {mutation}: {result:?}"
            );
        }
    }
}

#[test]
fn explicit_ordered_reference_loads_keep_the_addressable_effect_refusal() {
    for aggregate in [false, true] {
        for atomic in [false, true] {
            let owner = explicit_load_owner(
                Case::Shared,
                aggregate,
                1,
                if atomic {
                    SemanticVolatilityV1::NonVolatile
                } else {
                    SemanticVolatilityV1::Volatile
                },
                atomic.then_some(SemanticAtomicAccessV1::new(
                    SemanticAtomicOrderingV1::Acquire,
                    SemanticAtomicScopeV1::Device,
                )),
            );
            assert!(matches!(
                with_source_lowered(owner, |_, _, _| panic!(
                    "ordered load reached stable payload emission"
                )),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference ordered load requires checked addressable effects",
                    ..
                })
            ));
        }
    }
}

#[test]
fn explicit_cell_loads_claim_one_physical_load_per_original_source_use() {
    for aggregate in [false, true] {
        for count in [1, 2] {
            cell_emission_tests::with_cell_source_lowered(
                explicit_load_owner(
                    Case::Writeback,
                    aggregate,
                    count,
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
                |plan, emission, emitted, budget| {
                    assert_eq!(plan.cells.rows.len(), 2);
                    let mut backing = BTreeSet::new();
                    for (cell_index, cell) in plan.cells.rows.iter().enumerate() {
                        let worker = &emitted
                            .iter()
                            .find(|(id, _)| *id == cell.instance)
                            .unwrap()
                            .1;
                        let slot = worker
                            .scoped_slot_origins
                            .as_ref()
                            .unwrap()
                            .iter()
                            .find(|slot| slot.legacy_local().unwrap() == cell.local.index())
                            .unwrap();
                        assert!(backing.insert(slot.pointer));
                        let child = plan.instances.calls(cell.instance).unwrap()[0]
                            .child()
                            .unwrap();
                        let helper = &emitted.iter().find(|(id, _)| *id == child).unwrap().1;
                        let body = helper.function.body.as_ref().unwrap();
                        assert_eq!(body.parameters.len(), 1);
                        let pointer = body.parameters[0];
                        let operations: Vec<_> = body
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .collect();
                        assert_eq!(
                            operations
                                .iter()
                                .filter(|operation| matches!(
                                    operation.kind,
                                    OperationKind::Load { .. }
                                ))
                                .count(),
                            count
                        );
                        assert_eq!(operations.iter().filter(|operation|
                            matches!(operation.kind, OperationKind::Load { pointer: actual, .. }
                                if actual == pointer)).count(), count);
                        assert_eq!(operations.iter().filter(|operation|
                            matches!(operation.kind, OperationKind::Store { pointer: actual, .. }
                                if actual == pointer)).count(), 1);
                        assert!(!operations.iter().any(|operation| matches!(
                            operation.kind,
                            OperationKind::Alloca { .. }
                        )));
                        let caller = worker.function.body.as_ref().unwrap();
                        let call = caller
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| {
                                matches!(&operation.kind, OperationKind::Call { callee, .. }
                                if callee == &helper.function.id)
                            })
                            .unwrap();
                        assert!(matches!(&call.kind, OperationKind::Call { arguments, .. }
                            if arguments == &[slot.pointer]));
                        let claims: Vec<_> = plan
                            .accesses
                            .iter()
                            .zip(&emission.cell_accesses)
                            .filter_map(|(source, claim)| {
                                (source.key.site.instance == child
                                    && source.key.access == SourceReferenceAccessV29::Read)
                                    .then(|| {
                                        (
                                            source,
                                            claim
                                                .get()
                                                .expect("every original Load has a physical claim"),
                                        )
                                    })
                            })
                            .collect();
                        assert_eq!(claims.len(), count);
                        let mut physical = BTreeSet::new();
                        for (ordinal, (source, claim)) in claims.iter().enumerate() {
                            assert_eq!(source.key.site.statement, Some(ordinal + 1));
                            assert_eq!(claim.cell, cell_index);
                            assert_eq!(claim.pointer, pointer);
                            let operation =
                                claim.operation.expect("claimed original Load operation");
                            assert!(physical.insert((claim.block.0, operation)));
                            let emitted_block = body
                                .blocks
                                .iter()
                                .find(|block| block.id == claim.block)
                                .unwrap();
                            assert!(matches!(emitted_block.operations[operation].kind,
                                OperationKind::Load { pointer: actual, .. } if actual == pointer));
                        }
                    }
                    plan.check_owner(plan.instances, budget)?;
                    let mut module = Module::new("explicit_source_reference_loads");
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
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}
