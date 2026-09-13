use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1,
    CastKind, CheckedBinaryOperator, ComparePredicate, Constant, Function, IntegerSwitchCase,
    MemoryAccess, Module, Operation, OperationKind, Signature, Type, UnaryOp, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12,
};

fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}
fn inventory(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    floor: usize,
) -> (CanonicalKirInventoryV1<'_>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, floor + 10_000_000);
    budget.reserve_storage(floor).unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(owner, &mut budget).unwrap();
    (inventory, storage.retained_storage())
}
fn inspect(
    module: Module,
    check: impl FnOnce(&CanonicalKirInventoryV1<'_>, &CanonicalKirSparseV1<'_, '_>),
) {
    let (owner, owner_storage) = admit(&module);
    let (inventory, inventory_storage) = inventory(&owner, owner_storage);
    let floor = owner_storage + inventory_storage + 11;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, floor + 10_000_000);
    budget.reserve_storage(floor).unwrap();
    let (report, storage) = CanonicalKirSparseV1::derive(
        &inventory,
        CanonicalKirSparseLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(report.belongs_to(&inventory));
    assert!(std::ptr::eq(report.inventory().owner(), &owner));
    check(&inventory, &report);
    drop(report);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}
fn result(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn literal(id: u32, value: Constant) -> Operation {
    result(id, value.ty(), OperationKind::Constant(value))
}
fn binary(id: u32, ty: ScalarType, op: BinaryOp, a: u32, b: u32) -> Operation {
    result(
        id,
        Type::Scalar(ty),
        OperationKind::Binary {
            op,
            lhs: ValueId(a),
            rhs: ValueId(b),
        },
    )
}
fn function_module(
    name: &str,
    signature: Signature,
    parameters: Vec<ValueId>,
    blocks: Vec<BasicBlock>,
) -> Module {
    let mut module = Module::new(name);
    module.functions.push(Function::internal_helper(
        name, signature, parameters, blocks,
    ));
    module
}
fn fact(
    inventory: &CanonicalKirInventoryV1<'_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    function: u32,
    value: u32,
) -> Value {
    let index = inventory
        .definitions()
        .iter()
        .position(|row| {
            let owner = match row.coordinate {
                Definition::FunctionArgument { function, .. } => function,
                Definition::BlockArgument { block, .. } => block.function,
                Definition::Result { operation, .. } => operation.block.function,
            };
            owner.0 == function && row.value == Some(ValueId(value))
        })
        .unwrap();
    report.value(index).unwrap()
}
fn constant(ty: ScalarType, bits: u128) -> Value {
    Value::Constant(CanonicalKirSparseConstantV1 { ty, bits })
}

fn diamond(dynamic: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(4_000_000_000));
    entry.operations = vec![
        literal(9, Constant::Bool(true)),
        literal(10, Constant::U32(11)),
        literal(20, Constant::U32(22)),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(if dynamic { 4_000_000_000 } else { 9 }),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(10)],
        else_target: BlockId(7),
        else_arguments: vec![ValueId(20)],
    });
    let mut join = BasicBlock::new(BlockId(7));
    join.parameters
        .push(ValueDef::new(ValueId(222), Type::Scalar(ScalarType::U32)));
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(222)],
    });
    let mut dormant = BasicBlock::new(BlockId(99));
    dormant
        .operations
        .push(literal(u32::MAX, Constant::U32(99)));
    dormant.terminator = Some(Terminator::Return {
        values: vec![ValueId(u32::MAX)],
    });
    function_module(
        "diamond",
        Signature::new(vec![Type::BOOL], vec![Type::Scalar(ScalarType::U32)]),
        vec![ValueId(4_000_000_000)],
        vec![entry, join, dormant],
    )
}

fn use_query_terminator(function: u32, block: u32, operand: u32) -> Use {
    Use::TerminatorOperand {
        block: Block {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function),
            block,
        },
        operand,
    }
}

fn use_query_operation(function: u32, block: u32, operation: u32, operand: u32) -> Use {
    Use::OperationOperand {
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: Block {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function),
                block,
            },
            operation,
        },
        operand,
    }
}

#[test]
fn use_queries_resolve_operands_and_both_checked_result_components() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        literal(1, Constant::U8(255)),
        literal(2, Constant::U8(1)),
        Operation::checked_binary(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U8)),
            ValueDef::new(ValueId(4), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3), ValueId(4)],
    });
    inspect(
        function_module(
            "checked_uses",
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U8), Type::BOOL]),
            vec![],
            vec![block],
        ),
        |_, report| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(32);
            let mut budget = Budget::new(&mut work, 0);
            for (coordinate, expected) in [
                (
                    use_query_operation(0, 0, 2, 0),
                    constant(ScalarType::U8, 255),
                ),
                (use_query_operation(0, 0, 2, 1), constant(ScalarType::U8, 1)),
                (use_query_terminator(0, 0, 0), constant(ScalarType::U8, 0)),
                (use_query_terminator(0, 0, 1), constant(ScalarType::Bool, 1)),
            ] {
                assert_eq!(report.value_at_use(coordinate, &mut budget), Ok(expected));
            }
            assert_eq!(budget.work(), 32);
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.peak_storage(), 0);
        },
    );
}

#[test]
fn use_queries_keep_duplicate_edge_payloads_and_lattice_states_distinct() {
    for dynamic in [false, true] {
        inspect(diamond(dynamic), |inventory, report| {
            assert_eq!(inventory.edges()[0].target, inventory.edges()[1].target);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(40);
            let mut budget = Budget::new(&mut work, 0);
            for (coordinate, expected) in [
                (
                    use_query_terminator(0, 0, 0),
                    if dynamic {
                        Value::Dynamic
                    } else {
                        constant(ScalarType::Bool, 1)
                    },
                ),
                (use_query_terminator(0, 0, 1), constant(ScalarType::U32, 11)),
                (use_query_terminator(0, 0, 2), constant(ScalarType::U32, 22)),
                (
                    use_query_terminator(0, 1, 0),
                    if dynamic {
                        Value::Dynamic
                    } else {
                        constant(ScalarType::U32, 11)
                    },
                ),
                (use_query_terminator(0, 2, 0), Value::Unreachable),
            ] {
                assert_eq!(report.value_at_use(coordinate, &mut budget), Ok(expected));
            }
            // The untaken edge's argument still names a constant definition.
            assert_eq!(report.edge_executable(1), Some(dynamic));
            assert_eq!(budget.work(), 40);
        });
    }
}

#[test]
fn use_queries_validate_nested_ordinals_instead_of_raw_sparse_ids() {
    let mut module = diamond(false);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(result(
            4_000_000_001,
            Type::BOOL,
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(9),
            },
        ));
    let mut prefix = BasicBlock::new(BlockId(70));
    prefix.operations.push(literal(9, Constant::Bool(false)));
    prefix.terminator = Some(Terminator::Return {
        values: vec![ValueId(9)],
    });
    module.functions.insert(
        0,
        Function::internal_helper(
            "prefix",
            Signature::new(vec![], vec![Type::BOOL]),
            vec![],
            vec![prefix],
        ),
    );
    module.functions.insert(
        1,
        Function::external_import("declaration", Signature::new(vec![Type::BOOL], vec![])),
    );
    inspect(module, |_, report| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
        let mut budget = Budget::new(&mut work, 0);
        assert_eq!(
            report.value_at_use(use_query_terminator(0, 0, 0), &mut budget),
            Ok(constant(ScalarType::Bool, 0)),
        );
        assert_eq!(
            report.value_at_use(use_query_operation(2, 0, 3, 0), &mut budget),
            Ok(constant(ScalarType::Bool, 1)),
        );
        assert_eq!(
            report.value_at_use(use_query_terminator(2, 0, 1), &mut budget),
            Ok(constant(ScalarType::U32, 11)),
        );
        for coordinate in [
            use_query_terminator(u32::MAX, 0, 0),
            use_query_terminator(1, 0, 0), // Declaration's empty range cannot spill.
            use_query_terminator(0, 1, 0), // Nor can the preceding function's range.
            use_query_terminator(2, 3, 0),
            use_query_terminator(2, 4_000_000_000, 0),
            use_query_terminator(2, 0, 3), // Next block has a use at this dense offset.
            use_query_terminator(2, 0, u32::MAX),
            use_query_operation(2, 0, 0, 0), // Constant has no operands.
            use_query_operation(2, 0, 3, 1), // Would spill into terminator operands.
            use_query_operation(2, 0, 4, 0),
            use_query_operation(2, 0, u32::MAX, 0),
        ] {
            assert_eq!(
                report.value_at_use(coordinate, &mut budget),
                Err(CanonicalKirSparseErrorV1::InvalidUseCoordinate { coordinate }),
            );
        }
        assert_eq!(budget.work(), 14 * 8);
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn use_query_budget_is_fixed_upfront_and_preserves_owner_floor_and_history() {
    let (owner, owner_storage) = admit(&diamond(false));
    let (inventory, inventory_storage) = inventory(&owner, owner_storage);
    let base = owner_storage + inventory_storage + 13;
    let (report, report_storage) = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, base + 1_000_000);
        budget.reserve_storage(base).unwrap();
        CanonicalKirSparseV1::derive(
            &inventory,
            CanonicalKirSparseLimitsV1::default(),
            &mut budget,
        )
        .unwrap()
    };
    let floor = base + report_storage.retained_storage();
    let coordinate = use_query_terminator(0, 0, 0);
    for allowance in [8, 7] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3 + allowance);
        {
            let mut budget = Budget::new(&mut work, floor + 5);
            budget.reserve_storage(floor + 5).unwrap();
            budget.release_storage(5).unwrap();
            assert!(budget.reserve_storage(6).is_err());
            budget.charge_work(3).unwrap();
            assert!(budget.charge_work(100).is_err());
            let actual = report.value_at_use(coordinate, &mut budget);
            if allowance == 8 {
                assert_eq!(actual, Ok(constant(ScalarType::Bool, 1)));
            } else {
                assert!(matches!(
                    actual,
                    Err(CanonicalKirSparseErrorV1::Resource(Resource::Work(error)))
                        if error.actual() == 11 && error.limit() == 10
                ));
            }
            assert_eq!(budget.work(), if allowance == 8 { 11 } else { 3 });
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + 5);
            assert_eq!(budget.failed_storage(), Some(floor + 6));
        }
        assert_eq!(work.failed_work(), Some(103));
    }
    // Exactly the live owner floor is sufficient: the query allocates nothing.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
    let mut budget = Budget::new(&mut work, floor);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        report.value_at_use(coordinate, &mut budget),
        Ok(constant(ScalarType::Bool, 1)),
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor);
    assert!(report.belongs_to(&inventory));
    assert!(inventory.belongs_to(&owner));
    assert_eq!(
        owner.module().functions[0].body.as_ref().unwrap().blocks[0].id,
        BlockId(4_000_000_000)
    );
}

#[test]
fn invalid_use_queries_observe_the_same_exact_and_one_under_work_bound() {
    inspect(diamond(false), |_, report| {
        let coordinate = use_query_terminator(u32::MAX, u32::MAX, u32::MAX);
        for allowance in [8, 7] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(allowance);
            {
                let mut budget = Budget::new(&mut work, 0);
                let actual = report.value_at_use(coordinate, &mut budget);
                if allowance == 8 {
                    assert_eq!(
                        actual,
                        Err(CanonicalKirSparseErrorV1::InvalidUseCoordinate { coordinate }),
                    );
                    assert_eq!(budget.work(), 8);
                } else {
                    assert!(matches!(
                        actual,
                        Err(CanonicalKirSparseErrorV1::Resource(Resource::Work(error)))
                            if error.actual() == 8 && error.limit() == 7
                    ));
                    assert_eq!(budget.work(), 0);
                }
                assert_eq!(budget.storage(), 0);
            }
            assert_eq!(work.failed_work(), (allowance == 7).then_some(8));
        }
    });
}

#[test]
fn use_query_range_checks_reject_overflow_inverted_and_spilling_ranges() {
    assert_eq!(
        use_query_index(&(usize::MAX - 1..usize::MAX), 2, usize::MAX),
        None
    );
    let reversed = std::ops::Range { start: 2, end: 1 };
    assert_eq!(use_query_index(&reversed, 0, 3), None);
    assert_eq!(use_query_index(&(1..3), 0, 2), None);
    assert_eq!(use_query_index(&(1..2), 1, 3), None);
    assert_eq!(use_query_index(&(1..2), 0, 3), Some(1));
    assert!(!use_query_subrange(&(1..3), &(0..2), 3));
    assert!(!use_query_subrange(&(1..3), &(2..4), 4));
    assert!(!use_query_subrange(&(1..3), &reversed, 3));
    assert!(!use_query_subrange(&(1..3), &(1..2), 2));
}

#[test]
fn duplicate_targets_remain_distinct_executable_occurrences_and_merge_inputs() {
    for dynamic in [false, true] {
        inspect(diamond(dynamic), |inventory, report| {
            assert_eq!(report.edge_executable(0), Some(true));
            assert_eq!(report.edge_executable(1), Some(dynamic));
            assert_eq!(inventory.edges()[0].target, inventory.edges()[1].target);
            assert_eq!(
                fact(inventory, report, 0, 222),
                if dynamic {
                    Value::Dynamic
                } else {
                    constant(ScalarType::U32, 11)
                }
            );
            assert_eq!(fact(inventory, report, 0, 4_000_000_000), Value::Dynamic);
            assert_eq!(fact(inventory, report, 0, u32::MAX), Value::Unreachable);
            assert_eq!(report.block_executable(2), Some(false));
        });
    }
}

#[test]
fn integer_switch_prunes_by_typed_value_not_shared_target_identity() {
    let mut entry = BasicBlock::new(BlockId(8));
    entry.operations = vec![
        literal(1, Constant::I8(-1)),
        literal(2, Constant::U32(11)),
        literal(3, Constant::U32(22)),
        literal(4, Constant::U32(33)),
    ];
    entry.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(1),
        cases: vec![
            IntegerSwitchCase {
                value: Constant::I8(-1),
                target: BlockId(7),
                arguments: vec![ValueId(2)],
            },
            IntegerSwitchCase {
                value: Constant::I8(0),
                target: BlockId(7),
                arguments: vec![ValueId(3)],
            },
        ],
        default_target: BlockId(7),
        default_arguments: vec![ValueId(4)],
    });
    let mut join = BasicBlock::new(BlockId(7));
    join.parameters
        .push(ValueDef::new(ValueId(9), Type::Scalar(ScalarType::U32)));
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(9)],
    });
    inspect(
        function_module(
            "switch",
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
            vec![],
            vec![entry, join],
        ),
        |inventory, report| {
            assert_eq!(
                (0..3)
                    .map(|edge| report.edge_executable(edge).unwrap())
                    .collect::<Vec<_>>(),
                [true, false, false]
            );
            assert_eq!(fact(inventory, report, 0, 9), constant(ScalarType::U32, 11));
        },
    );
}

#[test]
fn changing_loop_argument_reaches_dynamic_and_opens_the_exit_without_unrolling() {
    let mut entry = BasicBlock::new(BlockId(90));
    entry.operations = vec![
        literal(1, Constant::U8(0)),
        literal(2, Constant::U8(1)),
        literal(3, Constant::U8(2)),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(91),
        arguments: vec![ValueId(1)],
    });
    let mut body = BasicBlock::new(BlockId(91));
    body.parameters
        .push(ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U8)));
    body.operations = vec![
        result(
            11,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(3),
            },
        ),
        binary(12, ScalarType::U8, BinaryOp::Add, 10, 2),
    ];
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(91),
        then_arguments: vec![ValueId(12)],
        else_target: BlockId(92),
        else_arguments: vec![ValueId(10)],
    });
    let mut exit = BasicBlock::new(BlockId(92));
    exit.parameters
        .push(ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U8)));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });
    inspect(
        function_module(
            "loop",
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U8)]),
            vec![],
            vec![entry, body, exit],
        ),
        |inventory, report| {
            assert!((0..3).all(|edge| report.edge_executable(edge) == Some(true)));
            assert_eq!(fact(inventory, report, 0, 10), Value::Dynamic);
            assert_eq!(fact(inventory, report, 0, 20), Value::Dynamic);
            assert_eq!(fact(inventory, report, 0, 2), constant(ScalarType::U8, 1));
            assert!(!report.values().contains(&Value::Unknown));
        },
    );
}

#[test]
fn exact_scalar_folding_keeps_checked_pairs_and_exceptional_operations_distinct() {
    let mut block = BasicBlock::new(BlockId(71));
    block.operations = vec![
        literal(1, Constant::U8(255)),
        literal(2, Constant::U8(1)),
        binary(3, ScalarType::U8, BinaryOp::Add, 1, 2),
        Operation::checked_binary(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U8)),
            ValueDef::new(ValueId(5), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
        literal(6, Constant::I8(-1)),
        literal(7, Constant::I8(1)),
        result(
            8,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(6),
                rhs: ValueId(7),
            },
        ),
        result(
            9,
            Type::Scalar(ScalarType::U16),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(1),
                to: Type::Scalar(ScalarType::U16),
            },
        ),
        result(
            10,
            Type::Scalar(ScalarType::I128),
            OperationKind::Cast {
                kind: CastKind::SignExtend,
                value: ValueId(6),
                to: Type::Scalar(ScalarType::I128),
            },
        ),
        result(
            11,
            Type::Scalar(ScalarType::U8),
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: ValueId(10),
                to: Type::Scalar(ScalarType::U8),
            },
        ),
        result(
            12,
            Type::Scalar(ScalarType::I8),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(1),
                to: Type::Scalar(ScalarType::I8),
            },
        ),
        literal(13, Constant::I8(-128)),
        binary(14, ScalarType::I8, BinaryOp::Divide, 13, 6),
        literal(15, Constant::U8(0)),
        binary(16, ScalarType::U8, BinaryOp::Divide, 2, 15),
        result(
            17,
            Type::Scalar(ScalarType::U128),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(2),
                to: Type::Scalar(ScalarType::U128),
            },
        ),
        result(
            18,
            Type::Scalar(ScalarType::U128),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(17),
            },
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(19), Type::Scalar(ScalarType::U128)),
            ValueDef::new(ValueId(20), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(18),
            ValueId(17),
        ),
        binary(21, ScalarType::U128, BinaryOp::Add, 19, 17),
        binary(22, ScalarType::I8, BinaryOp::ShiftRight, 13, 2),
        binary(23, ScalarType::U8, BinaryOp::ShiftLeft, 2, 1),
        literal(24, Constant::Index(6)),
        literal(25, Constant::Index(1)),
        binary(26, ScalarType::Index, BinaryOp::Add, 24, 25),
        result(
            27,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(24),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
        literal(28, Constant::U32(2)),
        result(
            29,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(28),
                to: Type::INDEX,
            },
        ),
        result(
            30,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(24),
                rhs: ValueId(25),
            },
        ),
        result(
            31,
            Type::INDEX,
            OperationKind::Select {
                condition: ValueId(5),
                true_value: ValueId(24),
                false_value: ValueId(25),
            },
        ),
        result(
            32,
            Type::Scalar(ScalarType::U8),
            OperationKind::Call {
                callee: "external".into(),
                arguments: vec![],
            },
        ),
        result(
            33,
            Type::pointer(
                Type::Scalar(ScalarType::U8),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U8),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 1,
            },
        ),
        result(
            34,
            Type::Scalar(ScalarType::U8),
            OperationKind::Load {
                pointer: ValueId(33),
                access: MemoryAccess::new(AddressSpace::Private, 1),
            },
        ),
        literal(35, Constant::F32Bits(1.0_f32.to_bits())),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = function_module(
        "scalar",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U8)]),
    ));
    inspect(module, |inventory, report| {
        for (id, ty, bits) in [
            (3, ScalarType::U8, 0),
            (4, ScalarType::U8, 0),
            (5, ScalarType::Bool, 1),
            (8, ScalarType::Bool, 1),
            (9, ScalarType::U16, 255),
            (10, ScalarType::I128, u128::MAX),
            (11, ScalarType::U8, 255),
            (12, ScalarType::I8, 255),
            (17, ScalarType::U128, 1),
            (18, ScalarType::U128, u128::MAX - 1),
            (19, ScalarType::U128, u128::MAX),
            (20, ScalarType::Bool, 0),
            (21, ScalarType::U128, 0),
            (22, ScalarType::I8, 192),
            (31, ScalarType::Index, 6),
        ] {
            assert_eq!(
                fact(inventory, report, 0, id),
                constant(ty, bits),
                "value {id}"
            );
        }
        for id in [14, 16, 23, 26, 27, 29, 30, 32, 33, 34, 35] {
            assert_eq!(fact(inventory, report, 0, id), Value::Dynamic, "value {id}");
        }
        for id in [14, 16, 23] {
            let operation = inventory
                .operations()
                .iter()
                .position(|row| {
                    inventory.definitions()[row.results.start].value == Some(ValueId(id))
                })
                .unwrap();
            assert_eq!(
                report.exception(operation),
                Some(CanonicalKirSparseExceptionV1::ExceptionalOperandsObserved)
            );
        }
        assert_eq!(report.block_executable(0), Some(true)); // Never prune on exceptional operands.
    });
}

#[test]
fn calls_and_declarations_are_conservative_even_for_a_shared_constant_helper() {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut helper = BasicBlock::new(BlockId(8));
    helper.operations.push(literal(7, Constant::U32(11)));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(7)],
    });
    let caller = |name: &str| {
        let mut block = BasicBlock::new(BlockId(9));
        block.operations = vec![
            result(
                20,
                scalar.clone(),
                OperationKind::Call {
                    callee: "shared".into(),
                    arguments: vec![],
                },
            ),
            result(
                21,
                scalar.clone(),
                OperationKind::Call {
                    callee: "external".into(),
                    arguments: vec![ValueId(20)],
                },
            ),
        ];
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(21)],
        });
        Function::internal_helper(
            name,
            Signature::new(vec![], vec![scalar.clone()]),
            vec![],
            vec![block],
        )
    };
    let mut module = Module::new("shared");
    module.functions = vec![
        caller("a"),
        Function::internal_helper(
            "shared",
            Signature::new(vec![], vec![scalar.clone()]),
            vec![],
            vec![helper],
        ),
        caller("b"),
        Function::external_import(
            "external",
            Signature::new(vec![scalar.clone()], vec![scalar]),
        ),
    ];
    inspect(module, |inventory, report| {
        assert_eq!(inventory.functions().len(), 4);
        assert_eq!(fact(inventory, report, 1, 7), constant(ScalarType::U32, 11));
        for function in [0, 2] {
            assert_eq!(fact(inventory, report, function, 20), Value::Dynamic);
            assert_eq!(fact(inventory, report, function, 21), Value::Dynamic);
        }
        let argument = inventory.functions()[3].definitions.start;
        assert!(inventory.definitions()[argument].value.is_none());
        assert_eq!(report.value(argument), Some(Value::Dynamic));
    });
}

#[test]
fn exact_inventory_binding_and_lattice_distinctions_are_not_hash_claims() {
    let (owner, owner_storage) = admit(&diamond(false));
    let (first, first_storage) = inventory(&owner, owner_storage);
    let (second, second_storage) = inventory(&owner, owner_storage + first_storage);
    let floor = owner_storage + first_storage + second_storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, floor + 1_000_000);
    budget.reserve_storage(floor).unwrap();
    let (report, _) =
        CanonicalKirSparseV1::derive(&first, CanonicalKirSparseLimitsV1::default(), &mut budget)
            .unwrap();
    assert!(report.belongs_to(&first));
    assert!(!report.belongs_to(&second));
    assert_eq!(Value::Unreachable.join(Value::Unknown), Value::Unknown);
    assert_eq!(
        Value::Unknown.join(constant(ScalarType::U8, 1)),
        constant(ScalarType::U8, 1)
    );
    assert_eq!(
        Value::Dynamic.join(constant(ScalarType::U8, 1)),
        Value::Dynamic
    );
    assert_eq!(
        constant(ScalarType::U8, 1).join(constant(ScalarType::U8, 2)),
        Value::Dynamic
    );
}

#[test]
fn independent_empty_budget_boundaries_preserve_prefix_floor_and_history() {
    let (owner, owner_storage) = admit(&Module::new("empty"));
    let (inventory, inventory_storage) = inventory(&owner, owner_storage);
    let floor = owner_storage + inventory_storage + 7;
    let payload = size_of::<CanonicalKirSparseV1<'_, '_>>();
    // One inventory admission event + seven scalar input-limit checks.
    for (allowance, bytes, succeeds) in [
        (8, payload, true),
        (7, payload, false),
        (8, payload - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(13 + allowance);
        work.charge_work(13).unwrap();
        {
            let mut budget = Budget::new(&mut work, floor + bytes);
            budget.reserve_storage(floor).unwrap();
            let result = CanonicalKirSparseV1::derive(
                &inventory,
                CanonicalKirSparseLimitsV1::default(),
                &mut budget,
            );
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(budget.storage(), floor);
            match result {
                Ok((report, receipt)) => {
                    assert_eq!(budget.work(), 21);
                    assert_eq!(receipt.retained_storage(), payload);
                    assert_eq!(budget.peak_storage(), floor + payload);
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    drop(report);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                }
                Err(CanonicalKirSparseErrorV1::Resource(Resource::Work(error))) => {
                    assert_eq!(error.actual(), 21);
                    assert_eq!(budget.work(), 20);
                    assert_eq!(budget.peak_storage(), floor);
                }
                Err(CanonicalKirSparseErrorV1::Resource(Resource::Storage(error))) => {
                    assert_eq!(error.actual(), floor + payload);
                    assert_eq!(budget.work(), 21);
                    assert_eq!(budget.peak_storage(), floor);
                }
                other => panic!("unexpected empty-boundary result: {other:?}"),
            }
        }
    }
}

#[test]
fn independent_nonempty_work_boundary_drops_queue_scratch_before_transferring_output() {
    let mut block = BasicBlock::new(BlockId(8));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let module = function_module(
        "empty-body",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let (owner, owner_storage) = admit(&module);
    let identity = *owner.canonical().identity();
    let (inventory, inventory_storage) = inventory(&owner, owner_storage);
    let floor = owner_storage + inventory_storage + 7;
    let retained = size_of::<CanonicalKirSparseV1<'_, '_>>() + size_of::<u8>();
    let peak_payload = retained + size_of::<u8>() + size_of::<usize>();
    // Admission8 + three allocate/init pairs6 + function visit1 + block
    // activation1 + enqueue1 + dequeue1 + terminator transfer1 = 19.
    for allowance in [18, 19] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + allowance);
        work.charge_work(5).unwrap();
        let mut budget = Budget::new(&mut work, floor + peak_payload);
        budget.reserve_storage(floor).unwrap();
        let result = CanonicalKirSparseV1::derive(
            &inventory,
            CanonicalKirSparseLimitsV1::default(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor + peak_payload);
        match result {
            Ok((report, receipt)) => {
                assert_eq!(allowance, 19);
                assert_eq!(budget.work(), 24);
                assert_eq!(receipt.retained_storage(), retained);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(report.block_executable(0), Some(true));
                drop(report);
                budget.release_storage(receipt.retained_storage()).unwrap();
            }
            Err(CanonicalKirSparseErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(allowance, 18);
                assert_eq!(error.actual(), 24);
                assert_eq!(budget.work(), 23);
            }
            other => panic!("unexpected queue-boundary result: {other:?}"),
        }
        assert_eq!(*owner.canonical().identity(), identity);
    }
}

#[test]
fn input_limits_and_seeded_failure_history_are_preserved() {
    let (owner, owner_storage) = admit(&diamond(true));
    let (inventory, inventory_storage) = inventory(&owner, owner_storage);
    let floor = owner_storage + inventory_storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    {
        let mut budget = Budget::new(&mut work, floor + 1000);
        budget.reserve_storage(floor).unwrap();
        let limits = CanonicalKirSparseLimitsV1 {
            definitions: 0,
            ..CanonicalKirSparseLimitsV1::default()
        };
        assert!(matches!(
            CanonicalKirSparseV1::derive(&inventory, limits, &mut budget),
            Err(CanonicalKirSparseErrorV1::InputLimit {
                resource: CanonicalKirSparseResourceV1::Definitions,
                ..
            })
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let _ = CanonicalKirSparseV1::derive(
            &inventory,
            CanonicalKirSparseLimitsV1::default(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

#[test]
fn nonempty_exact_peak_accounts_for_all_inverse_and_closure_buffers() {
    let mut block = BasicBlock::new(BlockId(91));
    block.operations.push(literal(7, Constant::U8(13)));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(7)],
    });
    let module = function_module(
        "nonempty-peak",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U8)]),
        vec![],
        vec![block],
    );
    let (owner, owner_storage) = admit(&module);
    let identity = *owner.canonical().identity();
    let (inventory, inventory_storage) = inventory(&owner, owner_storage);
    assert_eq!(inventory.definitions().len(), 1);
    assert_eq!(inventory.uses().len(), 1);
    assert_eq!(inventory.blocks().len(), 1);
    assert_eq!(inventory.operations().len(), 1);
    assert!(inventory.edges().is_empty());

    let floor = owner_storage + inventory_storage + 7;
    let retained = size_of::<CanonicalKirSparseV1<'_, '_>>()
        + size_of::<Value>()
        + size_of::<u8>()
        + size_of::<CanonicalKirSparseExceptionV1>();
    // D=1, U=1, N=O+B=2: heads1 + next1 + queue2 + closure1;
    // two queued flags coexist with every retained output allocation.
    let scratch = 5 * size_of::<usize>() + 2 * size_of::<u8>();
    let peak_payload = retained + scratch;
    for available in [peak_payload - 1, peak_payload] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        work.charge_work(5).unwrap();
        let mut budget = Budget::new(&mut work, floor + available);
        budget.reserve_storage(floor).unwrap();
        let result = CanonicalKirSparseV1::derive(
            &inventory,
            CanonicalKirSparseLimitsV1::default(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        match result {
            Ok((report, receipt)) => {
                assert_eq!(available, peak_payload);
                // Admission8 + allocation/init18 + inverse1 + definition1
                // + function1 + activation5 + op6 + terminator2 + closure1.
                assert_eq!(budget.work(), 5 + 43);
                assert_eq!(budget.peak_storage(), floor + peak_payload);
                assert_eq!(receipt.retained_storage(), retained);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(
                    fact(&inventory, &report, 0, 7),
                    constant(ScalarType::U8, 13)
                );
                assert_eq!(
                    report.exception(0),
                    Some(CanonicalKirSparseExceptionV1::NotAnalyzed)
                );
                drop(report);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            Err(CanonicalKirSparseErrorV1::Resource(Resource::Storage(error))) => {
                assert_eq!(available, peak_payload - 1);
                // The final closure allocation is charged after all seven
                // preceding nonempty vectors, but before its single init visit.
                assert_eq!(budget.work(), 5 + 25);
                assert_eq!(error.actual(), floor + peak_payload);
                assert_eq!(budget.failed_storage(), Some(floor + peak_payload));
                assert_eq!(
                    budget.peak_storage(),
                    floor + peak_payload - size_of::<usize>()
                );
            }
            other => panic!("unexpected nonempty-peak result: {other:?}"),
        }
        assert_eq!(*owner.canonical().identity(), identity);
    }
}

#[test]
fn admitted_entry_parameter_cycle_closes_unknown_without_inventing_an_input() {
    // Canonical structural verification permits an entry block parameter.
    // Its initial value is not supplied by the function signature; separate
    // formal-memory admission explicitly cannot authenticate that initial edge.
    // This test claims only conservative sparse-analysis closure.
    let mut entry = BasicBlock::new(BlockId(4_000_000_000));
    entry
        .parameters
        .push(ValueDef::new(ValueId(77), Type::BOOL));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(77),
        then_target: entry.id,
        then_arguments: vec![ValueId(77)],
        else_target: BlockId(7),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(7));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    inspect(
        function_module(
            "entry-parameter-cycle",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry, exit],
        ),
        |inventory, report| {
            assert_eq!(inventory.definitions().len(), 1);
            assert!(matches!(
                inventory.definitions()[0].coordinate,
                Definition::BlockArgument { .. }
            ));
            assert_eq!(inventory.edges().len(), 2);
            assert_eq!(inventory.edge_arguments().len(), 1);
            assert_eq!(
                inventory.edge_arguments()[0].incoming_definition,
                inventory.edge_arguments()[0].target_definition
            );
            // Before unresolved closure, the only selector is Unknown and
            // neither edge can execute. No constant or signature input seeds it.
            assert_eq!(fact(inventory, report, 0, 77), Value::Dynamic);
            assert_eq!(report.edge_executable(0), Some(true));
            assert_eq!(report.edge_executable(1), Some(true));
            assert_eq!(report.block_executable(0), Some(true));
            assert_eq!(report.block_executable(1), Some(true));
            assert!(!report.values().contains(&Value::Unknown));
        },
    );
}
