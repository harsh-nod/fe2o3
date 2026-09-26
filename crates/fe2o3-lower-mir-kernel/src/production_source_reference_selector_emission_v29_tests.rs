#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectorFault {
    None,
    Gep,
    Missing,
    Pending,
}

thread_local! {
    static SELECTOR_FAULT: std::cell::Cell<SelectorFault> = const { std::cell::Cell::new(SelectorFault::None) };
    static SELECTOR_REACHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SELECTOR_RUNTIME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SELECTOR_TREES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SELECTOR_SIGNED_CAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SELECTOR_CAST_UNKNOWN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct SelectorObservers {
    slot: Option<ScopedSlotObserverV29>,
    postflight: Option<SourceReferencePostflightObserverV29>,
}

impl SelectorObservers {
    fn install(fault: SelectorFault) -> Self {
        SELECTOR_FAULT.set(fault);
        SELECTOR_REACHED.set(0);
        SELECTOR_TREES.set(0);
        SELECTOR_SIGNED_CAST.set(false);
        SELECTOR_CAST_UNKNOWN.set(0);
        Self {
            slot: SCOPED_SLOT_OBSERVER_V29.replace(Some(selector_emitted_observer)),
            postflight: SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29
                .replace(Some(selector_claim_observer)),
        }
    }
}

impl Drop for SelectorObservers {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.slot);
        SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29.set(self.postflight);
    }
}

fn selector_emitted_observer(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for lowered in emitted.iter_mut().flatten() {
        if SELECTOR_SIGNED_CAST.get() {
            for operation in lowered.function.body.as_ref().unwrap().blocks.iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(operation.kind, OperationKind::Cast {
                    kind: CastKind::SignExtend, ref to, ..
                } if *to == Type::Scalar(ScalarType::U64)))
            {
                assert_eq!(operation.results.len(), 1);
                let output = &operation.results[0];
                let storage = budget.storage();
                let fact = source_selector_unsigned_constant_v29(
                    output.id, ScalarType::U64, 1, budget,
                    |value, budget| {
                        budget.charge_work(1)?;
                        assert_eq!(value, output.id);
                        Ok((&output.ty, Some(operation)))
                    },
                )?;
                assert_eq!(fact, None, "the actual signed cast is not an unsigned constant fact");
                assert_eq!(budget.storage(), storage);
                SELECTOR_CAST_UNKNOWN.set(SELECTOR_CAST_UNKNOWN.get() + 1);
            }
        }
        let Some(effect) = lowered
            .private_arrays
            .effects
            .iter()
            .find(|effect| matches!(effect.original_index, PrivateArrayIndexV1::Local { .. }))
        else {
            continue;
        };
        let PrivateArrayIndexV1::Local { original, .. } = effect.original_index else { unreachable!(); };
        if SELECTOR_RUNTIME.get() {
            let definition = lowered.function.body.as_ref().unwrap().blocks.iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.results.iter().any(|value| value.id == original)).unwrap();
            assert!(!matches!(definition.kind, OperationKind::Constant(_)));
        }
        if SELECTOR_FAULT.get() != SelectorFault::Gep { continue; }
        let location = effect.gep_location;
        let block = lowered
            .function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .unwrap();
        let OperationKind::GetElementPointer { base, offset } =
            &mut block.operations[location.operation].kind
        else {
            panic!("recorded selector GEP");
        };
        *offset = *base;
        SELECTOR_REACHED.set(SELECTOR_REACHED.get() + 1);
        return Ok(());
    }
    Ok(())
}

fn selector_claim_observer(
    references: Option<&mut SourceReferenceEmissionV29<'_, '_>>,
    _: &ExecutionInstancesV29<'_>,
    _: &mut ArgumentBudgetV1<'_>,
    success: bool,
) {
    let Some(references) = references.filter(|_| success) else {
        return;
    };
    if references.selectors.is_empty() {
        return;
    }
    let cell = &references.selectors[0];
    SELECTOR_TREES.set(SELECTOR_TREES.get() + references.selectors.iter().filter(|cell|
        cell.get().is_some_and(|claim| matches!(claim.producer, SourceReferenceSelectorProducerV29::ValueTree { .. }))).count());
    let mut claim = cell
        .get()
        .expect("actual source index producer was consumed");
    assert!(!matches!(
        claim.producer,
        SourceReferenceSelectorProducerV29::Pending
    ));
    match SELECTOR_FAULT.get() {
        SelectorFault::Missing => {
            cell.set(None);
        }
        SelectorFault::Pending => {
            claim.producer = SourceReferenceSelectorProducerV29::Pending;
            cell.set(Some(claim));
        }
        _ => {}
    }
    SELECTOR_REACHED.set(SELECTOR_REACHED.get() + references.selectors.len());
}

fn runtime_selector_module_owner(guarded_read: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = module_fixture_owner(ModuleFixture::Array);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let SemanticStatementKindV1::Assign(original_initializer) =
        semantic.functions()[2].blocks()[0].statements()[1].kind()
    else {
        panic!("original array initializer");
    };
    let array = original_initializer.destination().ty();
    assert!(matches!(types[array.index() as usize].shape(),
        SemanticTypeShapeV1::Array { element, length: 1 } if *element == U32));
    // Replace the original array row coherently; appending a row leaves the
    // one-element declaration outside the exact reachable source type closure.
    types[array.index() as usize] = SemanticTypeDeclV1::new(
        types[array.index() as usize].identity(),
        types[array.index() as usize].layout_identity(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16, 4, SemanticFieldsShapeV1::array(4, 4),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true), None, false, None, 16, 0,
            SemanticTypeLayoutDetailsV1::None,
        ).unwrap(), SemanticTypeShapeV1::Array { element: U32, length: 4 });
    let mut functions = semantic.functions().to_vec();
    for function_index in [2, 3] {
        let old = &functions[function_index];
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(index) = statements[0].kind() else {
            panic!("index producer");
        };
        let destination = index.destination().clone();
        // Nonconstant emitted SSA use spanning all four elements. The exact
        // source mask keeps the test in bounds without a compiler special case.
        statements[0] = assign(
            destination,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left: SemanticOperandV1::Copy(place(2, U32)),
                right: literal(3),
            },
        );
        let SemanticStatementKindV1::Assign(initializer) = statements[1].kind() else { panic!("array initializer"); };
        let array_local = initializer.destination().local();
        statements[1] = assign(place(array_local.index(), array), SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array,
                vec![literal(11), literal(12), literal(13), literal(14)]).unwrap()));
        let mut locals = old.locals().to_vec();
        locals[array_local.index() as usize] = local(140, array, SemanticLocalRoleV1::Temporary);
        if guarded_read && function_index == 3 {
            let boolean = types.iter().position(|ty| matches!(ty.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))).unwrap();
            let boolean = SemanticTypeIdV1::from_index(boolean as u32);
            let condition = locals.len() as u32;
            locals.push(local(220, boolean, SemanticLocalRoleV1::Temporary));
            let SemanticStatementKindV1::Assign(index_assignment) = statements[0].kind() else { panic!("index"); };
            let index_place = index_assignment.destination().clone();
            let SemanticStatementKindV1::Assign(store) = statements[2].kind() else { panic!("source selector"); };
            let selected = store.destination().clone();
            statements[0] = assign(index_place.clone(), SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place(2, U32))));
            statements.truncate(2);
            statements.push(assign(place(condition, boolean), SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: SemanticOperandV1::Copy(index_place.clone()), right: literal(4),
            }));
            blocks = vec![block(115, statements, SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(place(condition, boolean)), expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: literal(4), index: SemanticOperandV1::Copy(index_place),
                },
                target: SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::AssertSuccess,
                    SemanticBlockIdV1::from_index(1)),
                unwind: SemanticUnwindActionV1::Unreachable,
            }), block(221, vec![assign(place(0, U32), SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(selected)))], SemanticTerminatorKindV1::Return)];
            functions[function_index] = function(110, old.role(), old.abi().clone(), locals, blocks);
            continue;
        }
        blocks[0] = block(
            if function_index == 2 { 90 } else { 115 },
            statements,
            blocks[0].terminator().kind().clone(),
        );
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
        types,
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

fn run_source_selector_module(
    runtime: bool,
    guarded_read: bool,
    fault: SelectorFault,
) -> Result<(), ScopedModuleErrorV29> {
    let source_owner = if runtime {
        runtime_selector_module_owner(guarded_read)
    } else {
        module_fixture_owner(ModuleFixture::Array)
    };
    run_source_selector_owner(source_owner, runtime, guarded_read, false, fault)
}

fn run_source_selector_owner(
    source_owner: ProductionSemanticSsaOwnerV1,
    runtime: bool,
    guarded_read: bool,
    signed_cast: bool,
    fault: SelectorFault,
) -> Result<(), ScopedModuleErrorV29> {
    let _observers = SelectorObservers::install(fault);
    SELECTOR_RUNTIME.set(runtime);
    SELECTOR_SIGNED_CAST.set(signed_cast);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR)?;
    let (input, launch) = with_module_fixture_view(
        &source_owner,
        ModuleFixture::Array,
        &mut budget,
        |source, budget| OwnedExecutionInputV29::capture(source, budget),
    )?;
    let mut donor = Some(ScopedSourceInputsV29 {
        owner: source_owner,
        launch,
        input: input?,
    });
    let owner = SourceOwnedScopedModuleV29::try_new(
        &mut donor,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )?;
    assert!(donor.is_none());
    assert!(SELECTOR_REACHED.get() >= 2);
    if guarded_read { assert!(SELECTOR_TREES.get() > 0); }
    if signed_cast { assert!(SELECTOR_CAST_UNKNOWN.get() > 0); }
    let graph_identity = *owner.pending.graph.identity();
    let floor = budget.storage();
    owner.replay(&mut budget)?;
    assert_eq!(owner.pending.graph.identity(), &graph_identity);
    assert_eq!(budget.storage(), floor);
    let retained = owner.retained_storage;
    drop(owner);
    budget.release_storage(retained)?;
    assert_eq!(budget.storage(), MODULE_FLOOR);
    Ok(())
}

#[test]
fn fixed_array_source_selectors_reach_actual_module_v18_and_replay() {
    run_source_selector_module(false, false, SelectorFault::None).unwrap();
    run_source_selector_module(true, false, SelectorFault::None).unwrap();
    run_source_selector_module(true, true, SelectorFault::None).unwrap();
}

#[derive(Clone, Copy)]
enum CastSelectorSourceV29 {
    Literal(u64),
    GuardedSignExtend,
}

fn selector_integer_type_v29(
    types: &mut Vec<SemanticTypeDeclV1>,
    signed: bool,
    bits: u16,
) -> SemanticTypeIdV1 {
    let scalar = SemanticScalarTypeV1::Integer { signed, bits };
    if let Some(index) = types.iter().position(|ty|
        ty.shape() == &SemanticTypeShapeV1::Scalar(scalar))
    {
        return SemanticTypeIdV1::from_index(index as u32);
    }
    let bytes = u64::from(bits / 8);
    declaration(
        types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes), bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
            )),
            false,
        ).unwrap(),
        SemanticTypeShapeV1::Scalar(scalar),
        None,
    )
}

fn cast_selector_module_owner_v29(case: CastSelectorSourceV29) -> ProductionSemanticSsaOwnerV1 {
    let original = runtime_selector_module_owner(true);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let word = selector_integer_type_v29(&mut types, false, 64);
    let word_literal = |value: u64| SemanticOperandV1::Constant(SemanticConstantV1::new(
        word, SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(u128::from(value), 8).unwrap()),
    ));
    let mut functions = semantic.functions().to_vec();
    let old = &functions[3];
    let statements = old.blocks()[0].statements();
    let SemanticStatementKindV1::Assign(index_assignment) = statements[0].kind() else {
        panic!("existing guarded selector producer");
    };
    let index = index_assignment.destination().clone();
    let initializer = statements[1].clone();
    let mut locals = old.locals().to_vec();
    let blocks = match case {
        CastSelectorSourceV29::Literal(value) => {
            // The length-four array is read without a dynamic guard. The
            // original cast must supply a valid constant or an exact refusal.
            vec![block(115, vec![
                assign(index, SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer, operand: word_literal(value),
                }),
                initializer,
                old.blocks()[1].statements()[0].clone(),
            ], SemanticTerminatorKindV1::Return)]
        }
        CastSelectorSourceV29::GuardedSignExtend => {
            let signed = selector_integer_type_v29(&mut types, true, 32);
            let signed_local = locals.len() as u32;
            locals.push(local(222, signed, SemanticLocalRoleV1::Temporary));
            locals[index.local().index() as usize] =
                local(141, word, SemanticLocalRoleV1::Temporary);
            let index = place(index.local().index(), word);
            let SemanticStatementKindV1::Assign(condition_assignment) = statements[2].kind() else {
                panic!("existing bounds condition");
            };
            let condition = condition_assignment.destination().clone();
            vec![
                block(115, vec![
                    assign(place(signed_local, signed), SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: SemanticOperandV1::Copy(place(2, U32)),
                    }),
                    assign(index.clone(), SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: SemanticOperandV1::Copy(place(signed_local, signed)),
                    }),
                    initializer,
                    assign(condition.clone(), SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: SemanticOperandV1::Copy(index.clone()),
                        right: word_literal(4),
                    }),
                ], SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Copy(condition), expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: word_literal(4), index: SemanticOperandV1::Copy(index),
                    },
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess, SemanticBlockIdV1::from_index(1)),
                    unwind: SemanticUnwindActionV1::Unreachable,
                }),
                old.blocks()[1].clone(),
            ]
        }
    };
    functions[3] = function(110, old.role(), old.abi().clone(), locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), types, semantic.allocations().to_vec(), semantic.statics().to_vec(),
        semantic.vtables().to_vec(), functions, semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

#[test]
fn cast_literal_selectors_admit_last_element_and_refuse_exact_array_length() {
    run_source_selector_owner(
        cast_selector_module_owner_v29(CastSelectorSourceV29::Literal(3)),
        true, false, false, SelectorFault::None,
    ).unwrap();
    let error = run_source_selector_owner(
        cast_selector_module_owner_v29(CastSelectorSourceV29::Literal(4)),
        true, false, false, SelectorFault::None,
    ).expect_err("a casted index at the fixed extent cannot be admitted");
    assert!(matches!(error, ScopedModuleErrorV29::Source(
        ProductionSemanticKirErrorV1::FixedArrayIndexOutOfBounds {
            function: 3, block: 0, statement: Some(2), index: 4, length: 4,
            from_end: false, ..
        }
    )), "{error:?}");
}

#[test]
fn guarded_signed_cast_selector_keeps_unknown_fact_and_replays_actual_module() {
    run_source_selector_owner(
        cast_selector_module_owner_v29(CastSelectorSourceV29::GuardedSignExtend),
        true, true, true, SelectorFault::None,
    ).unwrap();
}

#[test]
fn actual_array_emission_refuses_changed_gep_and_missing_or_incomplete_index_receipts() {
    for runtime in [false, true] {
        for fault in [
            SelectorFault::Gep,
            SelectorFault::Missing,
            SelectorFault::Pending,
        ] {
            let result = run_source_selector_module(runtime, false, fault);
            assert!(
                SELECTOR_REACHED.get() > 0,
                "must reach actual emission: {result:?}"
            );
            assert!(result.is_err(), "fault {fault:?}");
        }
    }
}
fn literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}
