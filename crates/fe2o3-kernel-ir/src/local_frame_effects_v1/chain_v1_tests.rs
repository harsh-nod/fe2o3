use super::super::tests::{
    FLOOR, capacity_bytes, constant, function, header_bytes, module, pointer, scalar,
};
use super::*;

#[path = "chain_ledger_swap_v1_tests.rs"]
mod ledger_swap_tests;

use crate::{
    CanonicalKernelIrWorkBudgetV1, FunctionId, MemoryAccess, Signature, ValueDef,
    analyze_interprocedural_effects_v1, verify_module_ref,
};

fn chain_header_bytes() -> usize {
    size_of::<ChainWorkspace<'_>>() + size_of::<CheckedLocalFrameChainV1<'_, '_>>()
}

fn chain_rejected(module: &Module, operation: Option<usize>, reason: LocalFrameRefusalReasonV1) {
    let verified =
        verify_module_ref(module).expect("negative chain candidate must independently verify");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let mut entered = false;
    let actual = with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |_, _| {
        entered = true;
        Ok(())
    });
    assert_eq!(actual, Err(refusal(0, operation, reason)));
    assert!(!entered);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn old_entry_keeps_its_single_block_refusal_and_precharge() {
    let module = empty_chain();
    let verified = verify_module_ref(&module).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
    let mut budget = Budget::new(&mut work, FLOOR + header_bytes());
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| {
        panic!("legacy chain admission")
    });
    assert_eq!(
        result,
        Err(refusal(0, None, LocalFrameRefusalReasonV1::ControlFlow))
    );
    assert_eq!(budget.work(), 12);
    assert_eq!(budget.peak_storage(), FLOOR + header_bytes());
    assert_eq!(budget.storage(), FLOOR);
}

// These are verified physical KIR fixtures, not admitted Rust/source relations.
fn assert_chain(index_mode: u8) -> Module {
    let mut entry = BasicBlock::new(BlockId(7));
    entry.operations.push(constant(10, Constant::Index(8)));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(11), pointer(scalar())),
        OperationKind::Alloca {
            element: scalar(),
            count: Some(ValueId(10)),
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    for (cell, value) in [11, 13, 17, 19, 23, 29, 31, 37].into_iter().enumerate() {
        let cell = cell as u32;
        entry.operations.extend([
            constant(100 + cell, Constant::U32(value)),
            constant(120 + cell, Constant::Index(u64::from(cell))),
            Operation::effect_free(
                ValueDef::new(ValueId(140 + cell), pointer(scalar())),
                OperationKind::GetElementPointer {
                    base: ValueId(11),
                    offset: ValueId(120 + cell),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(140 + cell),
                    value: ValueId(100 + cell),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ]);
    }
    entry.operations.push(constant(20, Constant::U64(8)));
    if index_mode == 1 {
        entry.operations.extend([
            constant(24, Constant::U32(0)),
            Operation::effect_free(
                ValueDef::new(ValueId(25), pointer(scalar())),
                OperationKind::Alloca {
                    element: scalar(),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(25),
                    value: ValueId(24),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ]);
    }
    append_index(&mut entry, index_mode, 21, 26);
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(22), Type::INDEX),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(21),
            to: Type::INDEX,
        },
    ));
    append_condition(&mut entry, 23, 21, BlockId(19));

    let mut write = BasicBlock::new(BlockId(19));
    write.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(30), pointer(scalar())),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(22),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(30),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ]);
    append_index(&mut write, index_mode, 31, 34);
    write.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(32), Type::INDEX),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(31),
            to: Type::INDEX,
        },
    ));
    append_condition(&mut write, 33, 31, BlockId(3));

    let mut read = BasicBlock::new(BlockId(3));
    read.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(40), pointer(scalar())),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(32),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(41), scalar()),
            OperationKind::Load {
                pointer: ValueId(40),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        constant(42, Constant::U64(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(43), Type::INDEX),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(42),
                to: Type::INDEX,
            },
        ),
    ]);
    append_condition(&mut read, 44, 42, BlockId(42));

    let mut exit = BasicBlock::new(BlockId(42));
    exit.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(50), pointer(scalar())),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(43),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(50),
                value: ValueId(41),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ]);
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut sink = BasicBlock::new(BlockId(99));
    sink.operations
        .push(AmdGpuDiagnosticOperation::Trap.operation(None));
    sink.terminator = Some(Terminator::Unreachable);
    let parameters = if index_mode == 2 {
        vec![ValueId(0), ValueId(1)]
    } else {
        vec![ValueId(0)]
    };
    let mut helper = Function::definition(
        "assert_chain",
        Signature::new(vec![scalar(); parameters.len()], vec![]),
        parameters,
        vec![entry, exit, read, write, sink],
    );
    helper.required_capabilities = AmdGpuDiagnosticOperation::Trap.required_capabilities();
    module(vec![helper, AmdGpuDiagnosticOperation::Trap.declaration()])
}

fn append_index(block: &mut BasicBlock, mode: u8, result: u32, loaded: u32) {
    if mode == 0 {
        block.operations.push(constant(result, Constant::U64(0)));
    } else {
        let value = if mode == 1 {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(loaded), scalar()),
                OperationKind::Load {
                    pointer: ValueId(25),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ));
            ValueId(loaded)
        } else {
            ValueId(1)
        };
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U64)),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value,
                to: Type::Scalar(ScalarType::U64),
            },
        ));
    }
}

fn append_condition(block: &mut BasicBlock, result: u32, index: u32, target: BlockId) {
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(result), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(index),
            rhs: ValueId(20),
        },
    ));
    block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(result),
        then_target: target,
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
}

fn chain_body(module: &mut Module, id: BlockId) -> &mut BasicBlock {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == id)
        .unwrap()
}

#[test]
fn verified_literal_and_slot_cast_assert_chains_retain_control_and_latest_store() {
    for mode in [0, 1] {
        let module = assert_chain(mode);
        let original = module.clone();
        let verified = verify_module_ref(&module).expect("physical A/B-shaped chain must verify");
        let raw = analyze_interprocedural_effects_v1(&module).unwrap();
        assert!(
            !raw.function(&FunctionId::new("assert_chain"))
                .unwrap()
                .is_complete_and_pure()
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
            assert!(std::ptr::eq(checked.module(), &module));
            assert_eq!(
                checked.allocations(budget)?.len(),
                1 + usize::from(mode == 1)
            );
            let control = checked.control(budget)?;
            assert_eq!(control.len(), 5);
            assert_eq!(
                control
                    .iter()
                    .filter(|row| matches!(
                        row.kind(),
                        LocalFrameControlKindV1::Selected {
                            value: true,
                            successor: 0,
                            inactive: BlockId(99),
                            ..
                        }
                    ))
                    .count(),
                3
            );
            assert_eq!(
                control
                    .iter()
                    .filter(|row| matches!(row.kind(), LocalFrameControlKindV1::InactiveTrap))
                    .count(),
                1
            );
            assert!(control.iter().all(|row| row.function_ordinal() == 0));
            assert!(checked.edge_bindings(budget)?.is_empty());
            let accesses = checked.accesses(budget)?;
            assert_eq!(accesses.len(), 11 + 3 * usize::from(mode == 1));
            let read = accesses
                .iter()
                .find(|row| row.location().block() == BlockId(3))
                .unwrap();
            assert_eq!(read.kind(), LocalFrameAccessKindV1::Read);
            assert_eq!(read.cell(), 0);
            assert_eq!(read.value(), ValueId(41));
            assert_eq!(
                read.initializing_store(),
                Some(LocalFrameLocationV1 {
                    function_ordinal: 0,
                    block: BlockId(19),
                    operation: 1
                })
            );
            let final_write = accesses.last().unwrap();
            assert_eq!(
                (
                    final_write.location().block(),
                    final_write.cell(),
                    final_write.value()
                ),
                (BlockId(42), 1, ValueId(41))
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(module, original);
        assert_eq!(analyze_interprocedural_effects_v1(&module).unwrap(), raw);
    }
}

#[test]
fn unknown_predicates_and_hostile_bounds_edges_and_sink_bodies_refuse() {
    chain_rejected(
        &assert_chain(2),
        None,
        LocalFrameRefusalReasonV1::ControlFlow,
    );
    for mutation in 0..7 {
        let mut module = assert_chain(0);
        match mutation {
            0 => {
                let entry = chain_body(&mut module, BlockId(7));
                let OperationKind::Compare { predicate, .. } =
                    &mut entry.operations.last_mut().unwrap().kind
                else {
                    unreachable!()
                };
                *predicate = ComparePredicate::GreaterThan;
            }
            1 => {
                let entry = chain_body(&mut module, BlockId(7));
                let op = entry
                    .operations
                    .iter_mut()
                    .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(20)))
                    .unwrap();
                op.kind = OperationKind::Constant(Constant::U64(0));
            }
            2 => {
                let entry = chain_body(&mut module, BlockId(7));
                let op = entry
                    .operations
                    .iter_mut()
                    .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(21)))
                    .unwrap();
                op.kind = OperationKind::Constant(Constant::U64(8));
            }
            3 => {
                let Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                } = chain_body(&mut module, BlockId(7))
                    .terminator
                    .as_mut()
                    .unwrap()
                else {
                    unreachable!()
                };
                std::mem::swap(then_target, else_target);
            }
            4 => {
                let sink = chain_body(&mut module, BlockId(99));
                sink.operations.clear();
                sink.terminator = Some(Terminator::Return { values: vec![] });
            }
            5 => chain_body(&mut module, BlockId(99))
                .operations
                .insert(0, constant(300, Constant::U32(7))),
            6 => {
                let mut extra = BasicBlock::new(BlockId(1000));
                extra.terminator = Some(Terminator::Return { values: vec![] });
                module.functions[0]
                    .body
                    .as_mut()
                    .unwrap()
                    .blocks
                    .push(extra);
            }
            _ => unreachable!(),
        }
        chain_rejected(&module, None, LocalFrameRefusalReasonV1::ControlFlow);
    }
}

#[test]
fn branch_proofs_do_not_replace_pointer_bounds_initialization_or_live_effect_checks() {
    let mut wrong_address = assert_chain(0);
    let op = chain_body(&mut wrong_address, BlockId(7))
        .operations
        .iter_mut()
        .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(22)))
        .unwrap();
    op.kind = OperationKind::Constant(Constant::Index(8));
    chain_rejected(&wrong_address, Some(0), LocalFrameRefusalReasonV1::Index);

    let mut uninitialized = assert_chain(0);
    chain_body(&mut uninitialized, BlockId(7))
        .operations
        .retain(|op| {
            !matches!(
                op.kind,
                OperationKind::Store {
                    pointer: ValueId(140),
                    ..
                }
            )
        });
    chain_body(&mut uninitialized, BlockId(19))
        .operations
        .remove(1);
    chain_rejected(
        &uninitialized,
        Some(1),
        LocalFrameRefusalReasonV1::UninitializedRead,
    );

    let mut live_call = assert_chain(0);
    chain_body(&mut live_call, BlockId(19)).operations.insert(
        0,
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("pure_leaf"),
                arguments: vec![],
            },
        ),
    );
    live_call
        .functions
        .push(function("pure_leaf", vec![], None));
    chain_rejected(&live_call, Some(0), LocalFrameRefusalReasonV1::Operation);

    let mut overwritten = assert_chain(0);
    chain_body(&mut overwritten, BlockId(19)).operations.insert(
        2,
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(30),
                value: ValueId(101),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    );
    let verified = verify_module_ref(&overwritten).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
        let read = checked
            .accesses(budget)?
            .iter()
            .find(|row| row.location().block() == BlockId(3))
            .copied()
            .unwrap();
        assert_eq!(
            read.initializing_store(),
            Some(LocalFrameLocationV1 {
                function_ordinal: 0,
                block: BlockId(19),
                operation: 2
            })
        );
        Ok(())
    })
    .unwrap();
}

fn empty_chain() -> Module {
    let mut first = BasicBlock::new(BlockId(7));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![],
    });
    let mut last = BasicBlock::new(BlockId(9));
    last.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![Function::definition(
        "empty_chain",
        Signature::new(vec![], vec![]),
        vec![],
        vec![first, last],
    )])
}

fn comparison_chain(left: Constant, right: Constant, predicate: ComparePredicate) -> Module {
    let mut module = empty_chain();
    let entry = chain_body(&mut module, BlockId(7));
    entry.operations = vec![
        constant(0, left),
        constant(1, right),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::BOOL),
            OperationKind::Compare {
                predicate,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(9),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    let mut sink = BasicBlock::new(BlockId(99));
    sink.operations
        .push(AmdGpuDiagnosticOperation::Trap.operation(None));
    sink.terminator = Some(Terminator::Unreachable);
    module.functions[0].body.as_mut().unwrap().blocks.push(sink);
    module.functions[0].required_capabilities =
        AmdGpuDiagnosticOperation::Trap.required_capabilities();
    module
        .functions
        .push(AmdGpuDiagnosticOperation::Trap.declaration());
    module
}

#[test]
fn compare_selection_uses_exact_unsigned_types_and_full_width_values() {
    let cases = [
        (
            Constant::U8(254),
            Constant::U8(255),
            ComparePredicate::LessThan,
        ),
        (
            Constant::U16(65535),
            Constant::U16(65535),
            ComparePredicate::Equal,
        ),
        (
            Constant::U32(u32::MAX),
            Constant::U32(0),
            ComparePredicate::GreaterThan,
        ),
        (
            Constant::U64(u64::MAX),
            Constant::U64(u64::MAX - 1),
            ComparePredicate::GreaterThan,
        ),
        (
            Constant::Index(u64::MAX),
            Constant::Index(u64::MAX),
            ComparePredicate::LessThanOrEqual,
        ),
        (
            Constant::Bool(false),
            Constant::Bool(true),
            ComparePredicate::NotEqual,
        ),
    ];
    for (left, right, predicate) in cases {
        let module = comparison_chain(left, right, predicate);
        let verified = verify_module_ref(&module).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
            assert!(checked.control(budget)?.iter().any(|row| matches!(
                row.kind(),
                LocalFrameControlKindV1::Selected {
                    value: true,
                    successor: 0,
                    ..
                }
            )));
            Ok(())
        })
        .unwrap();
    }
    chain_rejected(
        &comparison_chain(
            Constant::U64(u64::MAX),
            Constant::U64(0),
            ComparePredicate::LessThan,
        ),
        None,
        LocalFrameRefusalReasonV1::ControlFlow,
    );
    chain_rejected(
        &comparison_chain(
            Constant::I64(0),
            Constant::I64(8),
            ComparePredicate::LessThan,
        ),
        None,
        LocalFrameRefusalReasonV1::ControlFlow,
    );
}

#[test]
fn selected_false_edge_is_not_confused_with_a_true_expected_polarity() {
    let mut module = comparison_chain(
        Constant::U64(0),
        Constant::U64(8),
        ComparePredicate::GreaterThan,
    );
    let Terminator::ConditionalBranch {
        then_target,
        else_target,
        ..
    } = chain_body(&mut module, BlockId(7))
        .terminator
        .as_mut()
        .unwrap()
    else {
        unreachable!()
    };
    std::mem::swap(then_target, else_target);
    let verified = verify_module_ref(&module).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
        assert!(checked.control(budget)?.iter().any(|row| matches!(
            row.kind(),
            LocalFrameControlKindV1::Selected {
                value: false,
                successor: 1,
                target: BlockId(9),
                inactive: BlockId(99),
                ..
            }
        )));
        Ok(())
    })
    .unwrap();
}

#[test]
fn repeated_blocks_pointer_edge_parameters_and_nontrap_sink_calls_refuse() {
    let mut looped = empty_chain();
    chain_body(&mut looped, BlockId(9)).terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    chain_rejected(&looped, None, LocalFrameRefusalReasonV1::ControlFlow);

    let mut pointer_edge = empty_chain();
    let first = chain_body(&mut pointer_edge, BlockId(7));
    first.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), pointer(scalar())),
        OperationKind::Alloca {
            element: scalar(),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![ValueId(0)],
    });
    chain_body(&mut pointer_edge, BlockId(9))
        .parameters
        .push(ValueDef::new(ValueId(1), pointer(scalar())));
    chain_rejected(&pointer_edge, None, LocalFrameRefusalReasonV1::PointerUse);

    let mut debug_sink = assert_chain(0);
    chain_body(&mut debug_sink, BlockId(99)).operations =
        vec![AmdGpuDiagnosticOperation::DebugTrap.operation(None)];
    debug_sink
        .functions
        .push(AmdGpuDiagnosticOperation::DebugTrap.declaration());
    debug_sink.functions[0]
        .required_capabilities
        .extend(AmdGpuDiagnosticOperation::DebugTrap.required_capabilities());
    chain_rejected(&debug_sink, Some(0), LocalFrameRefusalReasonV1::Operation);
}

#[test]
fn scalar_edge_arguments_are_simultaneously_substituted_and_retained() {
    let mut module = comparison_chain(Constant::U64(1), Constant::U64(0), ComparePredicate::Equal);
    let first = chain_body(&mut module, BlockId(7));
    first.operations.pop();
    first.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![ValueId(1), ValueId(0)],
    });
    let second = chain_body(&mut module, BlockId(9));
    second.parameters = vec![
        ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U64)),
        ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U64)),
    ];
    second.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(12), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(10),
            rhs: ValueId(11),
        },
    ));
    second.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(13),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    let mut last = BasicBlock::new(BlockId(13));
    last.terminator = Some(Terminator::Return { values: vec![] });
    module.functions[0].body.as_mut().unwrap().blocks.push(last);
    let verified = verify_module_ref(&module).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
        let rows = checked.edge_bindings(budget)?;
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0].argument(),
                rows[0].parameter(),
                rows[0].known_unsigned()
            ),
            (ValueId(1), ValueId(10), Some(0))
        );
        assert_eq!(
            (
                rows[1].argument(),
                rows[1].parameter(),
                rows[1].known_unsigned()
            ),
            (ValueId(0), ValueId(11), Some(1))
        );
        assert!(rows.iter().all(|row| row.source() == BlockId(7)
            && row.target() == BlockId(9)
            && row.ty() == ScalarType::U64));
        Ok(())
    })
    .unwrap();
    let Terminator::Branch { arguments, .. } = chain_body(&mut module, BlockId(7))
        .terminator
        .as_mut()
        .unwrap()
    else {
        unreachable!()
    };
    *arguments = vec![ValueId(1), ValueId(1)];
    chain_rejected(&module, None, LocalFrameRefusalReasonV1::ControlFlow);
}

#[test]
fn chain_control_queries_and_callback_cleanup_preserve_live_ledger_precedence() {
    let module = assert_chain(1);
    let verified = verify_module_ref(&module).unwrap();
    for mode in 0..6 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_checked_local_frame_chain_function_v1(
                verified,
                0,
                &mut budget,
                |checked, budget| {
                    entered = true;
                    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                    let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
                    foreign.reserve_storage(budget.storage()).unwrap();
                    assert_eq!(
                        checked.control(&mut foreign),
                        Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
                    );
                    assert_eq!(
                        checked.edge_bindings(&mut foreign),
                        Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
                    );
                    assert_eq!(foreign.work(), 10);
                    assert_eq!(checked.control(budget)?.len(), 5);
                    if mode >= 3 {
                        budget.release_storage(1)?;
                        assert_eq!(
                            checked.control(budget),
                            Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
                        );
                    }
                    match mode % 3 {
                        0 => Ok(()),
                        1 => Err(refusal(0, None, LocalFrameRefusalReasonV1::Index)),
                        _ => std::panic::panic_any("chain callback payload"),
                    }
                },
            )
        }));
        assert!(entered);
        if mode >= 3 {
            assert_eq!(
                outcome.unwrap(),
                Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
            );
        } else if mode == 2 {
            assert_eq!(
                outcome.unwrap_err().downcast_ref::<&str>(),
                Some(&"chain callback payload")
            );
        } else {
            assert_eq!(
                outcome.unwrap(),
                if mode == 0 {
                    Ok(())
                } else {
                    Err(refusal(0, None, LocalFrameRefusalReasonV1::Index))
                }
            );
        }
        assert_eq!(budget.storage(), FLOOR);
        with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
            assert_eq!(checked.control(budget)?.len(), 5);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn chain_callback_cannot_recreate_released_incoming_storage() {
    let module = empty_chain();
    let verified = verify_module_ref(&module).unwrap();
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_checked_local_frame_chain_function_v1(
                verified,
                0,
                &mut budget,
                |checked, budget| {
                    budget.release_storage(budget.storage() - (FLOOR - 1))?;
                    assert_eq!(
                        checked.control(budget),
                        Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
                    );
                    match mode {
                        0 => Ok(()),
                        1 => Err(refusal(0, None, LocalFrameRefusalReasonV1::Index)),
                        _ => std::panic::panic_any("lost incoming reservation"),
                    }
                },
            )
        }));
        assert_eq!(
            outcome.unwrap(),
            Err(LocalFrameErrorV1::Resource(ResourceError::Accounting))
        );
        assert_eq!(budget.storage(), FLOOR - 1);
    }
}

#[test]
fn chain_work_and_actual_capacity_limits_are_source_derived() {
    // Wrapper4 + common entry8 + chain6 + census10 + reservations14 + publish4
    // + sort8 + two loop visits12 + block lookups4+3+3 + dispatch8
    // + empty substitution3 + return1 + final census2 = 90.
    const EXACT: usize = 4 + 8 + 6 + 10 + 14 + 4 + 8 + 12 + 4 + 3 + 3 + 8 + 3 + 1 + 2;
    assert_eq!(EXACT, 90);
    let module = empty_chain();
    let verified = verify_module_ref(&module).unwrap();
    let bytes = chain_header_bytes()
        + capacity_bytes::<ChainBlock>(2)
        + capacity_bytes::<LocalFrameControlV1>(2);
    for limit in [EXACT - 1, EXACT] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, FLOOR + bytes);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result = with_checked_local_frame_chain_function_v1(
            verified,
            0,
            &mut budget,
            |checked, budget| {
                entered = true;
                assert_eq!(checked.control.len(), 2);
                assert_eq!(budget.storage(), FLOOR + bytes);
                Ok(())
            },
        );
        if limit == EXACT {
            result.unwrap();
            assert!(entered);
            assert_eq!(budget.work(), EXACT);
        } else {
            assert!(
                matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Work(error))) if error.actual() == EXACT && error.limit() == limit)
            );
            assert!(!entered);
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + bytes);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, FLOOR + bytes - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |_, _| {
        panic!("storage-denied callback")
    });
    assert!(
        matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Storage(error))) if error.actual() == FLOOR + bytes)
    );
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn control_row_query_has_an_exact_post_derivation_work_boundary() {
    let module = empty_chain();
    let verified = verify_module_ref(&module).unwrap();
    // Complete derivation90, live-ledger guard5, two retained control rows2.
    for limit in [96, 97] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result = with_checked_local_frame_chain_function_v1(
            verified,
            0,
            &mut budget,
            |checked, budget| {
                entered = true;
                assert_eq!(checked.control(budget)?.len(), 2);
                Ok(())
            },
        );
        assert!(entered);
        if limit == 97 {
            result.unwrap();
            assert_eq!(budget.work(), 97);
        } else {
            assert!(
                matches!(result, Err(LocalFrameErrorV1::Resource(ResourceError::Work(error))) if error.actual() == 97 && error.limit() == 96)
            );
            assert_eq!(budget.work(), 95);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn unknown_slot_overwrite_across_blocks_clears_then_reinitializes_the_predicate_fact() {
    for alias in [false, true] {
        let mut module = assert_chain(1);
        let pointer = if alias {
            let entry = chain_body(&mut module, BlockId(7));
            entry.operations.push(constant(61, Constant::Index(0)));
            entry.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(62), pointer(scalar())),
                OperationKind::GetElementPointer {
                    base: ValueId(25),
                    offset: ValueId(61),
                },
            ));
            ValueId(62)
        } else {
            ValueId(25)
        };
        chain_body(&mut module, BlockId(19)).operations.insert(
            2,
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer,
                    value: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        );
        chain_rejected(&module, None, LocalFrameRefusalReasonV1::ControlFlow);
        chain_body(&mut module, BlockId(19)).operations.insert(
            3,
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer,
                    value: ValueId(24),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        );
        let verified = verify_module_ref(&module).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |checked, budget| {
            assert_eq!(
                checked
                    .control(budget)?
                    .iter()
                    .filter(|row| matches!(
                        row.kind(),
                        LocalFrameControlKindV1::Selected { value: true, .. }
                    ))
                    .count(),
                3
            );
            let read = checked
                .accesses(budget)?
                .iter()
                .find(|row| {
                    row.value() == ValueId(34) && row.kind() == LocalFrameAccessKindV1::Read
                })
                .copied()
                .unwrap();
            assert_eq!(
                read.initializing_store(),
                Some(LocalFrameLocationV1 {
                    function_ordinal: 0,
                    block: BlockId(19),
                    operation: 3
                })
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn verified_execution_context_is_not_a_local_frame_operation() {
    use crate::{ExecutionOperationV15, ExecutionRoleV15, Kernel, LaunchDomain, LaunchExtent};

    let mut candidate = module(vec![Function::kernel_entry(
        "local_frame_execution",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations: vec![],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    )]);
    candidate.kernels.push(Kernel::new(
        "local_frame_execution",
        "local_frame_execution",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    for has_execution in [false, true] {
        if has_execution {
            candidate.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::effect_free(
                    ValueDef::new(ValueId(0), Type::Execution(ExecutionRoleV15::Context)),
                    OperationKind::Execution(ExecutionOperationV15::ContextIssue),
                ));
        }
        // A Context may remain live at exit under the genuine V15 lifecycle.
        let verified = verify_module_ref(&candidate).unwrap();
        for chain in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut entered = false;
            let result = if chain {
                with_checked_local_frame_chain_function_v1(verified, 0, &mut budget, |_, _| {
                    entered = true;
                    Ok(())
                })
            } else {
                with_checked_local_frame_function_v1(verified, 0, &mut budget, |_, _| {
                    entered = true;
                    Ok(())
                })
            };
            if has_execution {
                assert_eq!(
                    result,
                    Err(LocalFrameErrorV1::Unsupported {
                        function_ordinal: 0,
                        operation: Some(0),
                        reason: LocalFrameRefusalReasonV1::Operation,
                    })
                );
                assert!(!entered);
            } else {
                result.unwrap();
                assert!(entered);
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
