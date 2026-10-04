use super::*;
use crate::{
    BasicBlock, IntrinsicOperation, Kernel, Signature, StorageLayoutKindV1, StorageLayoutLimitsV1,
    StorageLayoutV1, ValueDef,
};

const FLOOR: usize = 17;

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn constant(id: u32, value: u64) -> Operation {
    op(
        id,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
    )
}
fn binary(id: u32, kind: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    op(
        id,
        Type::INDEX,
        OperationKind::Binary {
            op: kind,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn fixture() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("affine_source_owner");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    // An authentic storage-aware owner, deliberately not a legacy V1 token.
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    module
}
fn arithmetic() -> Module {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        op(
            100,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        constant(2, 3),
        constant(90, 2),
        binary(7, BinaryOp::Multiply, 90, 100),
        binary(8, BinaryOp::Add, 7, 2),
        binary(9, BinaryOp::Multiply, 100, 100),
        constant(10, u64::MAX),
        binary(11, BinaryOp::Add, 10, 2),
    ];
    module
}
fn with_owner<R>(module: &Module, run: impl FnOnce(&VerifiedCanonicalKernelIrModuleV18) -> R) -> R {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let original = (budget.work(), budget.storage(), budget.peak_storage());
    let returned = run(&owner);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        original
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    returned
}
fn expected(constant: u64, coefficient: u64) -> Expression {
    Ok(AffineExpression {
        constant,
        invocation_coefficient: coefficient,
    })
}

#[test]
fn actual_owner_affine_uses_original_sparse_definitions_and_exact_grammar() {
    let module = arithmetic();
    assert!(verify_module_ref(&module).is_err());
    with_owner(&module, |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        assert!(std::ptr::eq(paid.owner, owner));
        assert!(std::ptr::eq(paid.source, &owner.module().functions[0]));
        assert!(std::ptr::eq(paid.root, &owner.module().kernels[0]));
        let (legacy, _) = collect_definitions(paid.source).unwrap();
        for (id, result) in [
            (2, expected(3, 0)),
            (7, expected(0, 2)),
            (8, expected(3, 2)),
            (9, Err(IndexExpressionError::Unsupported)),
            (11, Err(IndexExpressionError::Overflow)),
            (100, expected(0, 1)),
            (u32::MAX, Err(IndexExpressionError::Unsupported)),
        ] {
            assert_eq!(
                paid.expression(owner, 0, ValueId(id), &mut budget).unwrap(),
                result
            );
            assert_eq!(derive_affine_index(ValueId(id), &legacy), result);
        }
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(owner.module(), &module);
    });
}

#[test]
fn actual_owner_affine_u64_literals_do_not_admit_dynamic_or_fixed_width_arithmetic() {
    let mut module = fixture();
    let source = &mut module.functions[0];
    source.signature.parameters = vec![Type::Scalar(ScalarType::U64)];
    let body = source.body.as_mut().unwrap();
    body.parameters = vec![ValueId(0)];
    body.blocks[0].operations = vec![
        op(
            1,
            Type::Scalar(ScalarType::U64),
            OperationKind::Constant(Constant::U64(7)),
        ),
        op(
            2,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(1),
                to: Type::INDEX,
            },
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: Type::INDEX,
            },
        ),
        op(
            4,
            Type::Scalar(ScalarType::U64),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(1),
            },
        ),
        op(
            5,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(4),
                to: Type::INDEX,
            },
        ),
    ];
    with_owner(&module, |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        for id in 0..=5 {
            assert_eq!(
                paid.expression(owner, 0, ValueId(id), &mut budget).unwrap(),
                if matches!(id, 1 | 2) {
                    expected(7, 0)
                } else {
                    Err(IndexExpressionError::Unsupported)
                }
            );
        }
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

fn phi_fixture(recurrence: bool) -> Module {
    let mut module = fixture();
    let source = &mut module.functions[0];
    source.signature.parameters = vec![Type::BOOL];
    let body = source.body.as_mut().unwrap();
    body.parameters = vec![ValueId(0)];
    body.blocks[0].operations = vec![constant(10, 7)];
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(10)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = vec![ValueDef::new(ValueId(20), Type::INDEX)];
    if recurrence {
        header.operations.push(binary(21, BinaryOp::Add, 20, 10));
    }
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(if recurrence { 21 } else { 20 })],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(20)],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.parameters = vec![ValueDef::new(ValueId(30), Type::INDEX)];
    exit.operations.push(binary(31, BinaryOp::Add, 30, 10));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut dead = BasicBlock::new(BlockId(3));
    dead.operations.push(constant(40, 9));
    dead.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([header, exit, dead]);
    module
}

#[test]
fn actual_owner_affine_original_phi_scc_and_unreachable_code_remain_conservative() {
    for recurrence in [false, true] {
        with_owner(&phi_fixture(recurrence), |owner| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let (legacy, _) = collect_definitions(paid.source).unwrap();
            for id in [0, 10, 20, 21, 30, 31, 40, u32::MAX] {
                assert_eq!(
                    paid.expression(owner, 0, ValueId(id), &mut budget).unwrap(),
                    derive_affine_index(ValueId(id), &legacy)
                );
            }
            assert_eq!(
                paid.expression(owner, 0, ValueId(31), &mut budget).unwrap(),
                if recurrence {
                    Err(IndexExpressionError::Unsupported)
                } else {
                    expected(14, 0)
                }
            );
            assert_eq!(
                paid.expression(owner, 0, ValueId(40), &mut budget).unwrap(),
                Err(IndexExpressionError::Unsupported)
            );
            paid.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        });
    }
}

#[test]
fn actual_owner_affine_deep_reverse_id_graph_uses_paid_stack_growth() {
    let mut module = fixture();
    let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    ops.push(constant(600, 1));
    for id in (1..600).rev() {
        ops.push(binary(id, BinaryOp::Add, id + 1, 600));
    }
    with_owner(&module, |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        assert_eq!(
            paid.expression(owner, 0, ValueId(1), &mut budget).unwrap(),
            expected(600, 0)
        );
        assert!(
            budget.peak_storage() > budget.storage(),
            "completed traversal scratch must retire"
        );
        assert!(paid.rows.iter().all(|row| !row.visiting));
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn actual_owner_affine_bare_work_has_an_independent_exact_boundary() {
    // Entry 1 + exact function-name join 11 + context 163 + definition walk 1
    // + empty dense vector allocation 1 + final root attempt 1.
    const EXACT: usize = 178;
    with_owner(&fixture(), |owner| {
        for limit in [EXACT - 1, EXACT] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            match ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget) {
                Ok(paid) => {
                    assert_eq!(limit, EXACT);
                    assert_eq!(budget.work(), EXACT);
                    paid.release(&mut budget).unwrap();
                }
                Err(error) => {
                    assert_eq!(limit, EXACT - 1);
                    assert!(matches!(error, Failure::Resource(ResourceError::Work(_))));
                    assert_eq!(budget.failed_work(), Some(EXACT));
                }
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    });
}

#[test]
fn actual_owner_affine_full_phase_exact_and_short_budgets_preserve_denial_and_floor() {
    with_owner(&arithmetic(), |owner| {
        let (exact_work, exact_storage) = {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let observed = (budget.work(), budget.peak_storage());
            paid.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            observed
        };
        // These whole-constructor cuts are measured, not an independent formula.
        for (work_limit, storage_limit, succeeds) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
            (0, exact_storage, false),
            (exact_work, FLOOR + frame_bytes_v18() - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            match ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget) {
                Ok(paid) => {
                    assert!(succeeds);
                    paid.release(&mut budget).unwrap();
                }
                Err(error) => {
                    assert!(!succeeds);
                    assert!(matches!(
                        error,
                        Failure::Resource(ResourceError::Work(_) | ResourceError::Storage { .. })
                    ));
                    assert_eq!(
                        ActualOwnerAffineV18::build(
                            owner,
                            0,
                            ControlFlowLimits::DEFAULT,
                            &mut budget
                        )
                        .err()
                        .unwrap(),
                        error
                    );
                }
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    });
}

#[test]
fn actual_owner_affine_equal_or_changed_owners_do_not_replace_live_custody() {
    let module = arithmetic();
    let mut changed = module.clone();
    changed.functions[0].body.as_mut().unwrap().blocks[0].operations[1] = constant(2, 5);
    with_owner(&module, |owner| {
        with_owner(&module, |equal| {
            with_owner(&changed, |different| {
                assert_eq!(owner.canonical_bytes(), equal.canonical_bytes());
                for foreign in [equal, different] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                    let mut budget = Budget::new(&mut work, usize::MAX);
                    let mut paid = ActualOwnerAffineV18::build(
                        owner,
                        0,
                        ControlFlowLimits::DEFAULT,
                        &mut budget,
                    )
                    .unwrap();
                    let error = paid
                        .expression(foreign, 0, ValueId(8), &mut budget)
                        .unwrap_err();
                    assert_eq!(error, Failure::Resource(ResourceError::Accounting));
                    assert_eq!(
                        paid.expression(owner, 0, ValueId(8), &mut budget)
                            .unwrap_err(),
                        error
                    );
                    paid.release(&mut budget).unwrap();
                    assert_eq!(budget.storage(), 0);
                }
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = Budget::new(&mut work, usize::MAX);
                let mut paid = ActualOwnerAffineV18::build(
                    different,
                    0,
                    ControlFlowLimits::DEFAULT,
                    &mut budget,
                )
                .unwrap();
                assert_eq!(
                    paid.expression(different, 0, ValueId(8), &mut budget)
                        .unwrap(),
                    expected(5, 2)
                );
                paid.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), 0);
            })
        })
    });
}

#[test]
fn actual_owner_affine_foreign_ledger_root_and_restored_floor_checks_are_exact() {
    with_owner(&arithmetic(), |owner| {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            if mode == 0 {
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut other = Budget::new(&mut other_work, usize::MAX);
                assert_eq!(
                    paid.expression(owner, 0, ValueId(8), &mut other)
                        .unwrap_err(),
                    Failure::Resource(ResourceError::Accounting)
                );
                assert_eq!((other.work(), other.storage()), (0, 0));
                assert_eq!(
                    paid.expression(owner, 0, ValueId(8), &mut budget).unwrap(),
                    expected(3, 2)
                );
            } else {
                if mode == 1 {
                    budget.release_storage(1).unwrap();
                }
                let error = paid
                    .expression(
                        owner,
                        if mode == 2 { 1 } else { 0 },
                        ValueId(8),
                        &mut budget,
                    )
                    .unwrap_err();
                assert_eq!(error, Failure::Resource(ResourceError::Accounting));
                if mode == 1 {
                    budget.reserve_storage(1).unwrap();
                }
                assert_eq!(
                    paid.expression(owner, 0, ValueId(8), &mut budget)
                        .unwrap_err(),
                    error
                );
            }
            budget.reserve_storage(29).unwrap();
            paid.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR + 29);
        }
    });
}

#[test]
fn actual_owner_affine_original_resource_denial_precedes_floor_undercut() {
    with_owner(&arithmetic(), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        budget.charge_work(1_000_000 - budget.work()).unwrap();
        let refusal = Failure::from(budget.charge_work(1).unwrap_err());
        budget.release_storage(1).unwrap();
        assert_eq!(
            paid.expression(owner, 0, ValueId(8), &mut budget)
                .unwrap_err(),
            refusal
        );
        budget.reserve_storage(1).unwrap();
        assert_eq!(
            paid.expression(owner, 0, ValueId(8), &mut budget)
                .unwrap_err(),
            refusal
        );
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn actual_owner_affine_invalid_root_refuses_without_fabricating_context() {
    with_owner(&fixture(), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(
            ActualOwnerAffineV18::build(owner, 1, ControlFlowLimits::DEFAULT, &mut budget)
                .err()
                .unwrap(),
            Failure::Resource(ResourceError::Accounting)
        );
        assert_eq!(budget.storage(), FLOOR);
        let paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    });
}

#[test]
fn actual_owner_affine_does_not_edit_original_effects_reasons_or_report_rows() {
    let mut module = arithmetic();
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let source = &mut module.functions[0];
    source.signature.parameters = vec![pointer.clone(), Type::Scalar(ScalarType::U32)];
    let body = source.body.as_mut().unwrap();
    body.parameters = vec![ValueId(0), ValueId(1)];
    body.blocks[0].operations.push(op(
        110,
        pointer,
        OperationKind::GetElementPointer {
            base: ValueId(0),
            offset: ValueId(8),
        },
    ));
    body.blocks[0].operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(110),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    with_owner(&module, |owner| {
        let launch = ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        };
        let mut formal = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
        let original = formal
            .derive(&KernelId::new("root"), launch, FormalIndexWidth::Bits64)
            .unwrap();
        assert_eq!(original.analysis().obligations().accesses().len(), 1);
        let bytes = owner.canonical_bytes().to_vec();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        assert_eq!(
            paid.expression(owner, 0, ValueId(8), &mut budget).unwrap(),
            expected(3, 2)
        );
        paid.release(&mut budget).unwrap();
        let fresh = formal
            .derive(&KernelId::new("root"), launch, FormalIndexWidth::Bits64)
            .unwrap();
        assert_eq!(fresh.analysis(), original.analysis());
        assert!(std::ptr::eq(fresh.owner(), original.owner()));
        assert_eq!(fresh.launch(), launch);
        assert_eq!(owner.canonical_bytes(), bytes);
        assert_eq!(budget.storage(), 0);
    });
}
