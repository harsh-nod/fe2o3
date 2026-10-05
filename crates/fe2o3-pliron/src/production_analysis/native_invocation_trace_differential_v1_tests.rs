use super::tests::{STORAGE, WORK, memory, noop, owner, with_checked};
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CheckedBinaryOperator,
    ComparePredicate, Constant, IntegerSwitchCase, Operation, OperationKind, ScalarType,
    Terminator, Type, ValueDef, ValueId,
};

fn value_case(lhs: Constant, rhs: Constant, operator: BinaryOp) -> fe2o3_kernel_ir::Module {
    let mut module = noop();
    let ty = lhs.ty();
    let mut results = vec![ValueDef::new(ValueId(3), ty.clone())];
    if matches!(operator, BinaryOp::Checked(_)) {
        results.push(ValueDef::new(ValueId(4), Type::BOOL));
    }
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), ty.clone()),
            OperationKind::Constant(lhs),
        ),
        Operation::effect_free(ValueDef::new(ValueId(2), ty), OperationKind::Constant(rhs)),
        Operation::new(
            results,
            OperationKind::Binary {
                op: operator,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    ];
    module
}

#[test]
fn actual_folder_preserves_value_and_overflow_results() {
    use super::super::pliron_invocation_trace::native_values_v1::NativeTraceStateV1;
    // Literal finite-width results, independently derived from each operand pair.
    for (lhs, rhs, operator, value, overflow) in [
        (
            Constant::U8(255),
            Constant::U8(1),
            CheckedBinaryOperator::Add,
            0,
            1,
        ),
        (
            Constant::I8(-128),
            Constant::I8(1),
            CheckedBinaryOperator::Subtract,
            127,
            1,
        ),
        (
            Constant::U8(16),
            Constant::U8(16),
            CheckedBinaryOperator::Multiply,
            0,
            1,
        ),
        (
            Constant::I8(-3),
            Constant::I8(7),
            CheckedBinaryOperator::Multiply,
            235,
            0,
        ),
    ] {
        let (owner, _) = owner(&value_case(lhs, rhs, BinaryOp::Checked(operator)));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let projection = Projection::import(&owner, &mut budget).unwrap();
        projection
            .with_function(0, &mut budget, |context, _, row, budget| {
                let input =
                    NativeTraceInputV1::derive(&owner, context, row, 0, projection.epoch(), budget)
                        .unwrap();
                let mut state = NativeTraceStateV1::new(&input, budget, 0).unwrap();
                let mut visits = 0;
                for (id, expected) in [(ValueId(3), value), (ValueId(4), overflow)] {
                    let native = *row
                        .values
                        .iter()
                        .find(|(_, (candidate, _))| *candidate == id)
                        .unwrap()
                        .0;
                    assert_eq!(
                        state
                            .evaluate(context, native, &mut visits)
                            .unwrap()
                            .unwrap()
                            .bits,
                        expected
                    );
                }
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn overflow_division_by_zero_and_invalid_shift_refuse_normal_completion() {
    for (lhs, rhs, operator) in [
        (Constant::U8(255), Constant::U8(1), BinaryOp::Add),
        (Constant::I8(-128), Constant::I8(-1), BinaryOp::Divide),
        (Constant::U8(7), Constant::U8(0), BinaryOp::Divide),
        (Constant::U8(1), Constant::U8(8), BinaryOp::ShiftLeft),
    ] {
        with_checked(&value_case(lhs, rhs, operator), |checked, budget| {
            with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
                assert_eq!(
                    view.attempt(0, budget)?,
                    CanonicalInvocationTraceAttemptV1::Unsupported {
                        block: 0,
                        operation: 2,
                        reason: NativeTraceRefusalV1::UnsupportedValue
                    }
                );
                Ok(())
            })
            .unwrap();
        });
    }
}

#[test]
fn signed_and_unsigned_switches_preserve_actual_keys_and_repeated_target_payload_slots() {
    for (selector, case, expected_block) in [
        (Constant::I8(-1), Constant::I8(-1), 2),
        (Constant::I8(2), Constant::I8(-1), 3),
        (Constant::U8(255), Constant::U8(255), 2),
        (Constant::U8(2), Constant::U8(255), 3),
    ] {
        let mut module = noop();
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[0].operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(1), selector.ty()),
                OperationKind::Constant(selector),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U8)),
                OperationKind::Constant(Constant::U8(7)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U8)),
                OperationKind::Constant(Constant::U8(9)),
            ),
        ];
        body.blocks[0].terminator = Some(Terminator::IntegerSwitch {
            selector: ValueId(1),
            cases: vec![IntegerSwitchCase {
                value: case,
                target: BlockId(10),
                arguments: vec![ValueId(2), ValueId(3)],
            }],
            default_target: BlockId(10),
            default_arguments: vec![ValueId(3), ValueId(2)],
        });
        let mut join = BasicBlock::new(BlockId(10));
        join.parameters = vec![
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U8)),
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U8)),
        ];
        join.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ));
        join.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(6),
            then_target: BlockId(11),
            then_arguments: vec![],
            else_target: BlockId(12),
            else_arguments: vec![],
        });
        body.blocks.push(join);
        for id in [11, 12] {
            let mut block = BasicBlock::new(BlockId(id));
            block.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks.push(block);
        }
        with_checked(&module, |checked, budget| {
            with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
                assert_eq!(
                    view.attempt(0, budget)?,
                    CanonicalInvocationTraceAttemptV1::Complete {
                        invocations: 1,
                        events: 1
                    }
                );
                assert_eq!(view.event(0, 0, 0, budget)?.block, expected_block);
                Ok(())
            })
            .unwrap();
        });
    }
}

#[test]
fn ranked_and_native_one_element_read_write_rosters_agree() {
    use dialect_kernel::{
        AccessKindAttr, IndexConstantOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp,
        RankedViewType, ReturnOp,
    };
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let function_type = FunctionType::get(&context, vec![], vec![]);
    let function = FuncOp::new(&mut context, "paired".try_into().unwrap(), function_type);
    let ty = RankedViewType::new(&context, 32, true, vec![1]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        &mut context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        11,
        101,
    )
    .unwrap();
    let zero = IndexConstantOp::new(&mut context, 0);
    let view_value = view.result(&context);
    let zero_value = zero.result(&context);
    let read = RankedAccessOp::new(
        &mut context,
        AccessKindAttr::Read,
        view_value,
        vec![zero_value],
    )
    .unwrap();
    let write = RankedAccessOp::new(
        &mut context,
        AccessKindAttr::Write,
        view_value,
        vec![zero_value],
    )
    .unwrap();
    let end = ReturnOp::new(&mut context);
    for pointer in [
        view.get_operation(),
        zero.get_operation(),
        read.get_operation(),
        write.get_operation(),
        end.get_operation(),
    ] {
        pointer.insert_at_back(function.get_entry_block(&context), &context);
    }
    let mut manager = Manager::new(&function);
    manager.prepare_exact_trace(&context, &function);
    let ranked = manager.exact_trace().unwrap();
    let normalized: Vec<_> = ranked[0]
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Memory {
                access, indices, ..
            } => {
                assert_eq!(indices, &[Some(0)]);
                Some(*access == AccessKindAttr::Write)
            }
            _ => None,
        })
        .collect();
    assert_eq!(normalized, [false, true]);
    with_checked(&memory(), |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            let mut native = Vec::new();
            for ordinal in 0..view.event_count(0, 0, budget)? {
                let event = view.event(0, 0, ordinal, budget)?;
                if matches!(
                    event.kind,
                    Some(NativeEventKindV1::Load | NativeEventKindV1::Store)
                ) {
                    assert_eq!(event.address.unwrap().byte_offset, Some(0));
                    native.push(event.kind == Some(NativeEventKindV1::Store));
                }
            }
            assert_eq!(native, [false, true]);
            assert_eq!(native, normalized);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn bounded_loop_keeps_every_store_barrier_and_simultaneous_payload_swap() {
    use fe2o3_kernel_ir::{
        AddressSpace, Barrier, BarrierSemantics, MemoryOrdering, SynchronizationScope,
        WorkgroupSize,
    };
    let mut module = memory();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[0].operations.pop().unwrap();
    let u8_ty = Type::Scalar(ScalarType::U8);
    for (id, value) in [(3, 0), (4, 1), (5, 3), (11, 7), (12, 9)] {
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), u8_ty.clone()),
            OperationKind::Constant(Constant::U8(value)),
        ));
    }
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(10),
        arguments: vec![ValueId(3), ValueId(11), ValueId(12)],
    });
    let mut header = BasicBlock::new(BlockId(10));
    header.parameters = [6, 7, 8]
        .map(|id| ValueDef::new(ValueId(id), u8_ty.clone()))
        .to_vec();
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(13), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(6),
            rhs: ValueId(5),
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(13),
        then_target: BlockId(11),
        then_arguments: vec![],
        else_target: BlockId(12),
        else_arguments: vec![ValueId(7), ValueId(8)],
    });
    let mut step = BasicBlock::new(BlockId(11));
    step.operations = vec![
        store,
        Operation::new(
            vec![],
            OperationKind::Barrier(Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Global],
                ),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), u8_ty.clone()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(6),
                rhs: ValueId(4),
            },
        ),
    ];
    step.terminator = Some(Terminator::Branch {
        target: BlockId(10),
        arguments: vec![ValueId(14), ValueId(8), ValueId(7)],
    });
    let mut exit = BasicBlock::new(BlockId(12));
    exit.parameters = [15, 16]
        .map(|id| ValueDef::new(ValueId(id), u8_ty.clone()))
        .to_vec();
    exit.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(17), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(15),
            rhs: ValueId(16),
        },
    ));
    exit.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(17),
        then_target: BlockId(18),
        then_arguments: vec![],
        else_target: BlockId(19),
        else_arguments: vec![],
    });
    body.blocks.extend([header, step, exit]);
    for id in [18, 19] {
        let mut end = BasicBlock::new(BlockId(id));
        end.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(end);
    }
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 1,
                    events: 8
                }
            );
            let expected = [
                (0, 0, Some(NativeEventKindV1::Load)),
                (2, 0, Some(NativeEventKindV1::Store)),
                (2, 1, None),
                (2, 0, Some(NativeEventKindV1::Store)),
                (2, 1, None),
                (2, 0, Some(NativeEventKindV1::Store)),
                (2, 1, None),
                (5, 0, Some(NativeEventKindV1::Return)),
            ];
            for (ordinal, (block, operation, kind)) in expected.into_iter().enumerate() {
                let event = view.event(0, 0, ordinal, budget)?;
                assert_eq!(
                    (event.block, event.operation, event.kind),
                    (block, operation, kind)
                );
                if kind.is_none() {
                    assert_eq!(event.synchronization_address_spaces, Some(4));
                }
            }
            assert_eq!(view.barrier_divergent(0, budget)?, Some(false));
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn immutable_folder_widening_and_truncation_match_literal_bit_patterns() {
    use super::super::pliron_invocation_trace::native_values_v1::NativeTraceStateV1;
    use fe2o3_kernel_ir::CastKind;
    for (source, kind, target, expected) in [
        (
            Constant::I8(-1),
            CastKind::SignExtend,
            ScalarType::I16,
            65535_u128,
        ),
        (
            Constant::U8(255),
            CastKind::ZeroExtend,
            ScalarType::U16,
            255,
        ),
        (Constant::U16(511), CastKind::Truncate, ScalarType::U8, 255),
        (
            Constant::I8(-128),
            CastKind::SignExtend,
            ScalarType::I128,
            u128::MAX - 127,
        ),
    ] {
        let mut module = noop();
        let target = Type::Scalar(target);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(1), source.ty()),
                OperationKind::Constant(source),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), target.clone()),
                OperationKind::Cast {
                    kind,
                    value: ValueId(1),
                    to: target,
                },
            ),
        ];
        let (owner, _) = owner(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let projection = Projection::import(&owner, &mut budget).unwrap();
        projection
            .with_function(0, &mut budget, |context, _, row, budget| {
                let input =
                    NativeTraceInputV1::derive(&owner, context, row, 0, projection.epoch(), budget)
                        .unwrap();
                let mut state = NativeTraceStateV1::new(&input, budget, 0).unwrap();
                let value = row.occurrences[1].pointer.deref(context).get_result(0);
                assert_eq!(
                    state
                        .evaluate(context, value, &mut 0)
                        .unwrap()
                        .unwrap()
                        .bits,
                    expected
                );
                Ok(())
            })
            .unwrap();
    }
}
