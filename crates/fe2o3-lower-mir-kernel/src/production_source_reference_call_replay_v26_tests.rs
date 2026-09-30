fn run_reference_call_replay_v26(
    fault: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
    use fe2o3_kernel_ir::{Function as KirFunction, Signature};
    let owner = global_expression_helper_owner_v23(false);
    let semantic = owner.source_semantic();
    let caller = &semantic.functions()[0];
    let callee = &semantic.functions()[1];
    let SemanticTerminatorKindV1::Call(call) = caller.blocks()[3].terminator().kind() else {
        panic!("original helper call");
    };
    let foreign = call.clone();
    let occurrence = ProductionCallOccurrenceV1 {
        caller: ProductionCallInstanceIdV1(0),
        block: SemanticBlockIdV1::from_index(3),
    };
    let generic = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    );
    let global = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut input_type = global.clone();
    if let Type::Pointer(pointer) = &mut input_type {
        if fault == 2 {
            pointer.pointee = Box::new(Type::Scalar(ScalarType::U64));
        }
        if fault == 3 {
            pointer.access = AccessMode::ReadOnly;
        }
    }
    let target = KirFunction::internal_helper(
        "reference-target",
        Signature::new(vec![generic.clone(), Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![],
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(
            ValueId(if fault == 11 { 4 } else { 3 }),
            generic.clone(),
        )],
        OperationKind::Cast {
            kind: if fault == 4 {
                CastKind::Bitcast
            } else {
                CastKind::PointerToGeneric
            },
            value: ValueId(if fault == 1 { 1 } else { 0 }),
            to: if fault == 8 {
                Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Generic,
                    AccessMode::ReadWrite,
                )
            } else {
                generic
            },
        },
    ));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: if fault == 12 {
                "foreign-target".into()
            } else {
                target.id.clone()
            },
            arguments: vec![ValueId(if fault == 7 { 1 } else { 3 }), ValueId(2)],
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let emitted = KirFunction::internal_helper(
        "reference-caller",
        Signature::new(
            vec![input_type, global, Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut inputs = [
        InvocationInputRowV1 {
            local: 1,
            ty: callee.locals()[1].ty(),
            source_argument: 0,
            tuple_field: None,
            first_parameter: if fault == 10 { 1 } else { 0 },
            parameter_count: 1,
            reference_call_transport: Some(ReferenceCallTransportV26 {
                occurrence: ProductionCallOccurrenceV1 {
                    caller: ProductionCallInstanceIdV1(usize::from(fault == 6)),
                    block: occurrence.block,
                },
                source_call: if fault == 9 {
                    0
                } else {
                    std::ptr::from_ref(call) as usize
                },
                input: ValueId(0),
                output: ValueId(3),
            }),
        },
        InvocationInputRowV1 {
            local: 2,
            ty: callee.locals()[2].ty(),
            source_argument: 1,
            tuple_field: None,
            first_parameter: 1,
            parameter_count: 1,
            reference_call_transport: None,
        },
    ];
    if fault == 13 {
        inputs[0].source_argument = 1;
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let result = (|| {
        let values = CallFunctionIndexV1::new(&emitted, &mut budget)?;
        check_reference_call_replay_v26(
            semantic.types(),
            caller,
            callee,
            occurrence,
            if fault == 5 { &foreign } else { call },
            &target,
            &inputs,
            &emitted.body.as_ref().unwrap().blocks[0],
            0,
            1,
            &values,
            &mut budget,
        )
    })();
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn original_reference_call_replay_rejects_same_count_source_cast_and_parameter_substitutions() {
    run_reference_call_replay_v26(0, usize::MAX, usize::MAX)
        .0
        .unwrap();
    for fault in 1..=13 {
        let result = run_reference_call_replay_v26(fault, usize::MAX, usize::MAX).0;
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn original_reference_call_replay_has_exact_and_one_short_cumulative_limits() {
    let (result, work, retained, peak) = run_reference_call_replay_v26(0, usize::MAX, usize::MAX);
    result.unwrap();
    let (exact, used, held, high) = run_reference_call_replay_v26(0, work, peak);
    exact.unwrap();
    assert_eq!((used, held, high), (work, retained, peak));
    let short = run_reference_call_replay_v26(0, work - 1, peak).0;
    assert!(matches!(
        short,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    let short = run_reference_call_replay_v26(0, work, peak - 1).0;
    assert!(matches!(
        short,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

thread_local! {
    static ACTUAL_REFERENCE_CALL_CAPTURES_V26: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_actual_reference_call_capture_v26(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut captures = 0;
    for target in emitted.iter().flatten() {
        let Some(inputs) = &target.direct_call_inputs else {
            continue;
        };
        assert!(target.invocation_entry.is_none());
        for input in inputs {
            let Some(transport) = input.reference_call_transport else {
                continue;
            };
            let child = target.source_call_instance.unwrap();
            let incoming = instances.incoming(child).unwrap();
            assert_eq!(transport.occurrence, incoming.occurrence());
            assert_eq!(
                transport.source_call,
                std::ptr::from_ref(incoming.source()) as usize
            );
            assert_eq!(input.parameter_count, 1);
            let caller = emitted[transport.occurrence.caller.index()]
                .as_ref()
                .unwrap();
            let physical = caller
                .blocks
                .iter()
                .find(|row| row.semantic_block == transport.occurrence.block)
                .unwrap()
                .kernel_ir_block;
            let block = caller
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == physical)
                .unwrap();
            let cast = block
                .operations
                .iter()
                .find(|operation| {
                    operation
                        .results
                        .first()
                        .is_some_and(|result| result.id == transport.output)
                })
                .unwrap();
            assert!(
                matches!(cast.kind, OperationKind::Cast { kind: CastKind::PointerToGeneric, value, .. } if value == transport.input)
            );
            let call = block
                .operations
                .iter()
                .find_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, arguments } if callee == &target.function.id => {
                        Some(arguments)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(call[input.first_parameter], transport.output);
            captures += 1;
        }
    }
    assert_eq!(captures, 1);
    ACTUAL_REFERENCE_CALL_CAPTURES_V26.set(captures);
    Ok(())
}

thread_local! {
    static ACTUAL_DIRECT_CALL_INPUT_FAULT_V26: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

fn mutate_actual_direct_call_inputs_v26(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    observe_actual_reference_call_capture_v26(source, instances, emitted, slots, budget)?;
    // Mutate only the first candidate, never its independent reconstruction.
    SCOPED_SLOT_OBSERVER_V29.set(None);
    let target = emitted
        .iter_mut()
        .flatten()
        .find(|target| {
            target.direct_call_inputs.as_ref().is_some_and(|inputs| {
                inputs
                    .iter()
                    .any(|input| input.reference_call_transport.is_some())
            })
        })
        .unwrap();
    match ACTUAL_DIRECT_CALL_INPUT_FAULT_V26.get() {
        1 => target.direct_call_inputs = None,
        2 => {
            let input = target
                .direct_call_inputs
                .as_mut()
                .unwrap()
                .iter_mut()
                .find(|input| input.reference_call_transport.is_some())
                .unwrap();
            input.source_argument ^= 1;
        }
        3 => {
            let input = target
                .direct_call_inputs
                .as_mut()
                .unwrap()
                .iter_mut()
                .find(|input| input.reference_call_transport.is_some())
                .unwrap();
            let transport = input.reference_call_transport.as_mut().unwrap();
            transport.input = transport.output;
        }
        4 => {
            let input = target
                .direct_call_inputs
                .as_mut()
                .unwrap()
                .iter_mut()
                .find(|input| input.reference_call_transport.is_some())
                .unwrap();
            input.reference_call_transport = None;
        }
        5 => target.direct_call_inputs.as_mut().unwrap().clear(),
        _ => panic!("unexpected direct-call input mutation"),
    }
    Ok(())
}

#[test]
fn original_straight_line_reference_call_inputs_reject_omission_and_same_count_rebinding() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
            ACTUAL_DIRECT_CALL_INPUT_FAULT_V26.set(0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(mutate_actual_direct_call_inputs_v26)));
    for nested in [false, true] {
        for fault in 1..=5 {
            SCOPED_SLOT_OBSERVER_V29.set(Some(mutate_actual_direct_call_inputs_v26));
            ACTUAL_DIRECT_CALL_INPUT_FAULT_V26.set(fault);
            let owner = global_expression_helper_owner_v23(nested);
            let abi = issued_descriptor_role_abi_v18(&owner);
            let completed = std::cell::Cell::new(false);
            let result = run_descriptor_role_owner_with_abi_v18(
                owner,
                abi,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |_, _, _| {
                    completed.set(true);
                    Ok(())
                },
            )
            .0;
            assert!(result.is_err(), "nested={nested}, fault={fault}");
            assert!(
                !completed.get(),
                "mutated input table reached source consumer"
            );
            assert_eq!(ACTUAL_REFERENCE_CALL_CAPTURES_V26.get(), 1);
        }
    }
}

#[test]
fn original_reference_argument_emission_retains_the_actual_cast_for_independent_replay() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_actual_reference_call_capture_v26)));
    for nested in [false, true] {
        ACTUAL_REFERENCE_CALL_CAPTURES_V26.set(0);
        let owner = global_expression_helper_owner_v23(nested);
        let abi = issued_descriptor_role_abi_v18(&owner);
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |_, _, _| {
                completed.set(true);
                Ok(())
            },
        )
        .0;
        assert!(result.is_ok(), "nested={nested}: {result:?}");
        assert!(completed.get());
        assert_eq!(ACTUAL_REFERENCE_CALL_CAPTURES_V26.get(), 1);
    }
}

include!("production_source_issued_call_owner_v26_tests.rs");
