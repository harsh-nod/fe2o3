#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotSelectorCase {
    Mask,
    Remainder,
    Unbounded,
    Guard,
    Loop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotSelectorFault {
    None,
    Alignment,
    Offset,
    GuardBypass,
}

thread_local! {
    static SLOT_SELECTOR_FAULT: std::cell::Cell<SlotSelectorFault> = const { std::cell::Cell::new(SlotSelectorFault::None) };
    static SLOT_SELECTOR_SEEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SLOT_SELECTOR_CHANGED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SLOT_SELECTOR_STARTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SLOT_SELECTOR_COMPLETED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SLOT_SELECTOR_RUN_COMPLETED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct SlotSelectorObserver(Option<ScopedSlotObserverV29>);

impl SlotSelectorObserver {
    fn install(fault: SlotSelectorFault) -> Self {
        SLOT_SELECTOR_FAULT.set(fault);
        SLOT_SELECTOR_SEEN.set(0);
        SLOT_SELECTOR_CHANGED.set(0);
        SLOT_SELECTOR_STARTED.set(0);
        SLOT_SELECTOR_COMPLETED.set(0);
        SLOT_SELECTOR_RUN_COMPLETED.set(false);
        Self(SCOPED_SLOT_OBSERVER_V29.replace(Some(slot_selector_observer)))
    }
}

impl Drop for SlotSelectorObserver {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.0);
    }
}

fn slot_selector_observer(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    SLOT_SELECTOR_STARTED.set(SLOT_SELECTOR_STARTED.get() + 1);
    let fault = SLOT_SELECTOR_FAULT.get();
    for lowered in emitted.iter_mut().flatten() {
        let Some(effect) = lowered
            .private_arrays
            .effects
            .iter()
            .find(|effect| matches!(effect.original_index, PrivateArrayIndexV1::Local { .. }))
            .cloned()
        else {
            continue;
        };
        SLOT_SELECTOR_SEEN.set(SLOT_SELECTOR_SEEN.get() + 1);
        let body = lowered.function.body.as_mut().unwrap();
        let block = body
            .blocks
            .iter_mut()
            .find(|block| block.id == effect.gep_location.block)
            .unwrap();
        let gep = &mut block.operations[effect.gep_location.operation];
        assert!(matches!(gep.kind, OperationKind::GetElementPointer { .. }));
        if SLOT_SELECTOR_CHANGED.get() != 0 {
            continue;
        }
        match fault {
            SlotSelectorFault::None => {}
            SlotSelectorFault::Offset => {
                let OperationKind::GetElementPointer { base, offset } = &mut gep.kind else {
                    unreachable!();
                };
                *offset = *base;
                SLOT_SELECTOR_CHANGED.set(1);
            }
            SlotSelectorFault::Alignment => {
                let pointer = gep.results[0].id;
                let operation = block
                    .operations
                    .iter_mut()
                    .find(|operation| {
                        matches!(operation.kind, OperationKind::Store { pointer: actual, .. }
                        | OperationKind::Load { pointer: actual, .. } if actual == pointer)
                    })
                    .unwrap();
                let access = match &mut operation.kind {
                    OperationKind::Store { access, .. } | OperationKind::Load { access, .. } => {
                        access
                    }
                    _ => unreachable!(),
                };
                access.alignment = 8;
                SLOT_SELECTOR_CHANGED.set(1);
            }
            SlotSelectorFault::GuardBypass => {
                for block in &mut body.blocks {
                    let Some(Terminator::ConditionalBranch { then_target, .. }) =
                        block.terminator.as_ref()
                    else {
                        continue;
                    };
                    block.terminator = Some(Terminator::Branch {
                        target: *then_target,
                        arguments: vec![],
                    });
                    SLOT_SELECTOR_CHANGED.set(1);
                    break;
                }
            }
        }
    }
    SLOT_SELECTOR_COMPLETED.set(SLOT_SELECTOR_COMPLETED.get() + 1);
    Ok(())
}

fn slot_selector_owner(case: SlotSelectorCase) -> ProductionSemanticSsaOwnerV1 {
    let original = runtime_selector_module_owner(case == SlotSelectorCase::Guard);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    for function_index in [2, 3] {
        let old = &functions[function_index];
        let mut blocks = old.blocks().to_vec();
        let mut locals = old.locals().to_vec();
        if case == SlotSelectorCase::Guard && function_index == 3 {
            let SemanticStatementKindV1::Assign(read) = blocks[1].statements()[0].kind() else {
                panic!("actual guarded read");
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(selected)) = read.value().kind()
            else {
                panic!("actual selected place");
            };
            let mut statements = vec![assign(
                selected.clone(),
                SemanticRvalueKindV1::Use(literal(99)),
            )];
            statements.extend_from_slice(blocks[1].statements());
            blocks[1] = block(221, statements, SemanticTerminatorKindV1::Return);
        } else {
            let mut statements = blocks[0].statements().to_vec();
            let SemanticStatementKindV1::Assign(index) = statements[0].kind() else {
                panic!("current source index");
            };
            let destination = index.destination().clone();
            let index_value = match case {
                SlotSelectorCase::Remainder => SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Remainder,
                    left: SemanticOperandV1::Copy(place(2, U32)),
                    right: literal(4),
                },
                SlotSelectorCase::Unbounded => SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitAnd,
                    left: SemanticOperandV1::Copy(place(2, U32)),
                    right: literal(7),
                },
                _ => index.value().kind().clone(),
            };
            statements[0] = assign(destination.clone(), index_value);
            if case == SlotSelectorCase::Loop && function_index == 3 {
                let SemanticStatementKindV1::Assign(store) = statements[2].kind() else {
                    panic!("selected write");
                };
                let selected = store.destination().clone();
                let boolean = semantic
                    .types()
                    .iter()
                    .position(|ty| {
                        matches!(
                            ty.shape(),
                            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                        )
                    })
                    .unwrap();
                let boolean = SemanticTypeIdV1::from_index(boolean as u32);
                let condition = locals.len() as u32;
                locals.push(local(224, boolean, SemanticLocalRoleV1::Temporary));
                let read = locals.len() as u32;
                locals.push(local(225, U32, SemanticLocalRoleV1::Temporary));
                let edge = |role, target| {
                    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
                };
                blocks = vec![
                    block(
                        115,
                        statements,
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                    ),
                    block(
                        221,
                        vec![assign(
                            place(condition, boolean),
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: SemanticOperandV1::Copy(destination.clone()),
                                right: literal(4),
                            },
                        )],
                        SemanticTerminatorKindV1::Assert {
                            condition: SemanticOperandV1::Copy(place(condition, boolean)),
                            expected: true,
                            message: SemanticAssertMessageV1::BoundsCheck {
                                length: literal(4),
                                index: SemanticOperandV1::Copy(destination.clone()),
                            },
                            target: edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(
                        222,
                        vec![
                            assign(selected.clone(), SemanticRvalueKindV1::Use(literal(99))),
                            assign(
                                place(read, U32),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(selected)),
                            ),
                            assign(
                                destination.clone(),
                                SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::BitAnd,
                                    left: SemanticOperandV1::Copy(destination),
                                    right: literal(3),
                                },
                            ),
                        ],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(place(2, U32)),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                )],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                            )
                            .unwrap(),
                        },
                    ),
                    block(223, vec![], SemanticTerminatorKindV1::Return),
                ];
            } else {
                blocks[0] = block(
                    if function_index == 2 { 90 } else { 115 },
                    statements,
                    blocks[0].terminator().kind().clone(),
                );
            }
        }
        functions[function_index] = function(
            if function_index == 2 { 100 } else { 110 },
            old.role(),
            old.abi().clone(),
            locals,
            blocks,
        );
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn run_slot_selector_module(
    case: SlotSelectorCase,
    fault: SlotSelectorFault,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ScopedModuleErrorV29>, usize, usize) {
    let _observer = SlotSelectorObserver::install(fault);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let owner = slot_selector_owner(case);
        let (input, launch) = with_module_fixture_view(
            &owner,
            ModuleFixture::Array,
            &mut budget,
            |source, budget| OwnedExecutionInputV29::capture(source, budget),
        )?;
        let mut donor = Some(ScopedSourceInputsV29 {
            owner,
            launch,
            input: input?,
        });
        let owner = SourceOwnedScopedModuleV29::try_new(
            &mut donor,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        assert!(donor.is_none());
        assert!(
            SLOT_SELECTOR_SEEN.get() > 0,
            "must inspect actual indexed physical emission"
        );
        let floor = budget.storage();
        let identity = *owner.pending.graph.identity();
        let replay = owner.replay(&mut budget);
        assert_eq!(owner.pending.graph.identity(), &identity);
        assert_eq!(budget.storage(), floor);
        let retained = owner.retained_storage;
        drop(owner);
        budget.release_storage(retained)?;
        replay
    })();
    assert_eq!(
        budget.storage(),
        MODULE_FLOOR,
        "{case:?} {fault:?}: {result:?}"
    );
    SLOT_SELECTOR_RUN_COMPLETED.set(true);
    (result, budget.work(), budget.peak_storage())
}

fn slot_selector_assertions_completed(require_observer: bool) {
    assert!(SLOT_SELECTOR_RUN_COMPLETED.get());
    assert_eq!(
        SLOT_SELECTOR_STARTED.get(),
        SLOT_SELECTOR_COMPLETED.get(),
        "no assertion panic inside production catches may qualify a refusal"
    );
    if require_observer {
        assert!(SLOT_SELECTOR_COMPLETED.get() > 0 && SLOT_SELECTOR_SEEN.get() > 0);
    }
}

#[test]
fn symbolic_slot_mask_remainder_and_original_guard_reach_actual_v18_replay() {
    for case in [
        SlotSelectorCase::Mask,
        SlotSelectorCase::Remainder,
        SlotSelectorCase::Guard,
    ] {
        let result =
            run_slot_selector_module(case, SlotSelectorFault::None, MODULE_LIMIT, MODULE_LIMIT).0;
        slot_selector_assertions_completed(true);
        result.unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
    }
}

#[test]
fn symbolic_slot_current_index_phi_and_reevaluation_reach_actual_v18_replay() {
    let result = run_slot_selector_module(
        SlotSelectorCase::Loop,
        SlotSelectorFault::None,
        MODULE_LIMIT,
        MODULE_LIMIT,
    )
    .0;
    slot_selector_assertions_completed(true);
    result.unwrap();
}

#[test]
fn symbolic_slot_actual_unbounded_access_has_no_permission_from_selector_identity() {
    let result = run_slot_selector_module(
        SlotSelectorCase::Unbounded,
        SlotSelectorFault::None,
        MODULE_LIMIT,
        MODULE_LIMIT,
    )
    .0;
    slot_selector_assertions_completed(true);
    assert!(
        matches!(
            result,
            Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported { .. }
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn symbolic_slot_actual_access_rejects_changed_offset_alignment_and_guard() {
    for (case, fault) in [
        (SlotSelectorCase::Mask, SlotSelectorFault::Offset),
        (SlotSelectorCase::Mask, SlotSelectorFault::Alignment),
        (SlotSelectorCase::Guard, SlotSelectorFault::GuardBypass),
    ] {
        let result = run_slot_selector_module(case, fault, MODULE_LIMIT, MODULE_LIMIT).0;
        slot_selector_assertions_completed(true);
        assert!(SLOT_SELECTOR_CHANGED.get() > 0);
        assert!(
            matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported { .. }
                ))
            ),
            "{case:?} {fault:?}: {result:?}"
        );
    }
}
