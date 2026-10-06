use super::tests::{STORAGE, WORK, memory, noop, owner, with_checked};
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Operation, OperationKind,
    Signature, Terminator, Type, ValueDef, ValueId,
};

#[test]
fn omitted_reordered_and_reindexed_occurrences_are_rejected() {
    let (owner, _) = owner(&memory());
    for mode in 0..10 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let mut projection = Projection::import(&owner, &mut budget).unwrap();
        projection.test_corrupt_occurrences(mode);
        assert!(matches!(
            projection.check(&mut budget),
            Err(Failure::ExactGraph)
        ));
    }
}

#[test]
fn a_same_value_attribute_write_stales_the_projection_epoch() {
    let (owner, _) = owner(&memory());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let mut projection = Projection::import(&owner, &mut budget).unwrap();
    projection.test_live(|context, root| {
        let attrs = root.deref(context).attributes.clone();
        root.deref_mut(context).attributes = attrs;
    });
    assert!(matches!(
        projection.check(&mut budget),
        Err(Failure::Mutation)
    ));
}

#[test]
fn identical_bytes_from_a_foreign_owner_do_not_authenticate_input() {
    let (owner, _) = owner(&memory());
    let (foreign, _) = tests::owner(&memory());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let projection = Projection::import(&owner, &mut budget).unwrap();
    projection
        .with_function(0, &mut budget, |context, _, row, budget| {
            assert!(matches!(
                NativeTraceInputV1::derive(&foreign, context, row, 0, projection.epoch(), budget),
                Err(TraceFailure::Native {
                    reason: NativeTraceRefusalV1::Context,
                    ..
                })
            ));
            Ok(())
        })
        .unwrap();
}

#[test]
fn load_dependent_boolean_control_has_no_successful_trace_cache() {
    let mut module = memory();
    module.functions[0].signature.parameters[0] = Type::pointer(
        Type::BOOL,
        fe2o3_kernel_ir::AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::ReadWrite,
    );
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.truncate(1);
    body.blocks[0].operations[0].results[0].ty = Type::BOOL;
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(10),
        then_arguments: vec![],
        else_target: BlockId(11),
        else_arguments: vec![],
    });
    for id in [10, 11] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(block);
    }
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::UnresolvedControl { block: 0 }
            );
            assert!(
                !view.rows[0]
                    .manager
                    .as_ref()
                    .unwrap()
                    .has_successful_exact_trace_v1()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn a_call_is_located_and_cannot_become_an_empty_helper_trace() {
    let mut module = noop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![],
            },
        ));
    let mut block = BasicBlock::new(BlockId(17));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Unsupported {
                    block: 0,
                    operation: 0,
                    reason: NativeTraceRefusalV1::UnsupportedCall
                }
            );
            assert_eq!(view.function_count(budget)?, 2);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn an_unselected_call_block_is_still_in_the_total_unsupported_census() {
    let mut module = noop();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::BOOL),
        OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(false)),
    ));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(10),
        then_arguments: vec![],
        else_target: BlockId(11),
        else_arguments: vec![],
    });
    for id in [10, 11] {
        let mut block = BasicBlock::new(BlockId(id));
        if id == 10 {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "helper".into(),
                    arguments: vec![],
                },
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(block);
    }
    let mut helper = BasicBlock::new(BlockId(20));
    helper.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper],
    ));
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Unsupported {
                    block: 1,
                    operation: 0,
                    reason: NativeTraceRefusalV1::UnsupportedCall,
                }
            );
            assert_eq!(
                view.function_census(0, budget)?,
                CanonicalNativeFunctionCensusV1::Unsupported {
                    block: 1,
                    operation: 0,
                    reason: NativeTraceRefusalV1::UnsupportedCall,
                }
            );
            assert!(view.rows[0].manager.is_none());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn unbound_index_width_is_not_a_u64_constant() {
    let mut module = noop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(0)),
        ));
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Unsupported {
                    block: 0,
                    operation: 0,
                    reason: NativeTraceRefusalV1::UnsupportedValue
                }
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn query_error_survives_a_later_panic_and_the_next_scope_recovers() {
    with_checked(&noop(), |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_invocation_traces_v1::<()>(checked, budget, |view, budget| {
            let _ = view.attempt(8, budget);
            panic!("after query failure");
        })
        .unwrap_err();
        assert!(matches!(
            error.failure,
            Failure::InvalidQuery { function: 8 }
        ));
        assert_eq!(budget.storage(), floor);
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(view.root_count(budget)?, 1);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn foreign_budget_cannot_query_a_borrowed_trace() {
    with_checked(&noop(), |checked, budget| {
        let error = with_canonical_invocation_traces_v1::<()>(checked, budget, |view, _| {
            let mut work = Work::new(WORK);
            let mut foreign = Budget::new(&mut work, STORAGE);
            view.root_count(&mut foreign)?;
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(
            error.failure,
            Failure::Resource(Resource::Accounting)
        ));
    });
}

#[test]
fn atomic_compare_exchange_outcome_cannot_select_a_success_branch() {
    let mut module = tests::atomic(fe2o3_kernel_ir::AtomicKind::CompareExchange);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(5),
        then_target: BlockId(10),
        then_arguments: vec![],
        else_target: BlockId(11),
        else_arguments: vec![],
    });
    for id in [10, 11] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(block);
    }
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::UnresolvedControl { block: 0 }
            );
            assert!(
                !view.rows[0]
                    .manager
                    .as_ref()
                    .unwrap()
                    .has_successful_exact_trace_v1()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn unknown_slice_length_stays_unknown_in_the_actual_value_adapter() {
    use super::super::pliron_invocation_trace::native_values_v1::NativeTraceStateV1;
    let mut module = noop();
    module.functions[0].signature.parameters = vec![Type::slice(
        Type::BOOL,
        fe2o3_kernel_ir::AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::ReadOnly,
    )];
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters = vec![ValueId(1)];
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::INDEX),
        OperationKind::SliceLength { slice: ValueId(1) },
    ));
    let (owner, _) = owner(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let projection = Projection::import(&owner, &mut budget).unwrap();
    projection
        .with_function(0, &mut budget, |context, _, row, budget| {
            let input =
                NativeTraceInputV1::derive(&owner, context, row, 0, projection.epoch(), budget)
                    .unwrap();
            let mut state = NativeTraceStateV1::new(&input, budget, 1).unwrap();
            assert_eq!(
                state
                    .evaluate(
                        context,
                        row.occurrences[0].pointer.deref(context).get_result(0),
                        &mut 0
                    )
                    .unwrap(),
                None
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn overflowing_gep_keeps_the_load_event_but_not_a_fabricated_zero_offset() {
    use fe2o3_kernel_ir::{BinaryOp, CastKind, Constant, ScalarType};
    let mut module = memory();
    let pointer_type = module.functions[0].signature.parameters[0].clone();
    let body = module.functions[0].body.as_mut().unwrap();
    let mut load = body.blocks[0].operations[0].clone();
    load.results[0].id = ValueId(6);
    if let OperationKind::Load { pointer, .. } = &mut load.kind {
        *pointer = ValueId(5);
    }
    let i128_ty = Type::Scalar(ScalarType::I128);
    body.blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::I64)),
            OperationKind::Constant(Constant::I64(i64::MAX)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), i128_ty.clone()),
            OperationKind::Cast {
                kind: CastKind::SignExtend,
                value: ValueId(2),
                to: i128_ty.clone(),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), i128_ty),
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(3),
                rhs: ValueId(3),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer_type),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(4),
            },
        ),
        load,
    ];
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 1,
                    events: 2
                }
            );
            let event = view.event(0, 0, 0, budget)?;
            assert_eq!(event.kind, Some(NativeEventKindV1::Load));
            let address = event.address.unwrap();
            assert_eq!(address.base_subject, ValueId(1));
            assert_eq!(address.byte_offset, None);
            assert_eq!(address.pointee_bytes, Some(4));
            assert!(
                view.remaining_obligations(0, budget)?
                    .unwrap()
                    .byte_bounds_and_alignment
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn subgroup_barrier_without_subgroup_layout_is_an_explicit_refusal() {
    use fe2o3_kernel_ir::{
        AddressSpace, Barrier, BarrierSemantics, MemoryOrdering, SynchronizationScope,
    };
    let mut module = noop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Barrier(Barrier {
                execution_scope: SynchronizationScope::Subgroup,
                memory_scope: SynchronizationScope::Subgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
            }),
        ));
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Unsupported {
                    block: 0,
                    operation: 0,
                    reason: NativeTraceRefusalV1::MissingHierarchy
                }
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn foreign_context_and_post_derivation_epoch_change_refuse_before_cache_reuse() {
    use pliron::{context::Context, op::Op};
    let (owner, _) = owner(&memory());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let projection = Projection::import(&owner, &mut budget).unwrap();
    projection
        .with_function(0, &mut budget, |context, function, row, budget| {
            let foreign = Context::new();
            let foreign_epoch = foreign.ir_mutation_attempt_epoch().unwrap().value();
            assert!(matches!(
                NativeTraceInputV1::derive(&owner, &foreign, row, 0, foreign_epoch, budget),
                Err(TraceFailure::Native {
                    reason: NativeTraceRefusalV1::Context,
                    ..
                })
            ));
            let input =
                NativeTraceInputV1::derive(&owner, context, row, 0, projection.epoch(), budget)
                    .unwrap();
            let pointer = function.get_operation();
            let attrs = pointer.deref(context).attributes.clone();
            pointer.deref_mut(context).attributes = attrs;
            assert!(matches!(
                input.authenticate(context, function),
                Err(TraceFailure::Native {
                    reason: NativeTraceRefusalV1::Context,
                    ..
                })
            ));
            Ok(())
        })
        .unwrap();
}
