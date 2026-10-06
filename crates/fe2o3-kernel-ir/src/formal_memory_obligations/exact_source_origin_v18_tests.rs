use super::*;
use crate::{
    BasicBlock, Kernel, Signature, StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1,
    ValueDef,
};

fn cast(id: u32, kind: CastKind, value: u32, to: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), to.clone()),
        OperationKind::Cast {
            kind,
            value: ValueId(value),
            to,
        },
    )
}
fn fixture() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let ptr = |space, access| Type::pointer(scalar.clone(), space, access);
    let slice = |space| Type::slice(scalar.clone(), space, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                ptr(AddressSpace::Private, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        cast(
            11,
            CastKind::RestrictPointerAccess,
            10,
            ptr(AddressSpace::Private, AccessMode::ReadOnly),
        ),
        cast(
            12,
            CastKind::PointerToGeneric,
            11,
            ptr(AddressSpace::Generic, AccessMode::ReadOnly),
        ),
        cast(
            13,
            CastKind::PointerToGeneric,
            10,
            ptr(AddressSpace::Generic, AccessMode::ReadWrite),
        ),
        cast(
            14,
            CastKind::RestrictPointerAccess,
            13,
            ptr(AddressSpace::Generic, AccessMode::ReadOnly),
        ),
        cast(
            15,
            CastKind::RestrictPointerAccess,
            0,
            ptr(AddressSpace::Global, AccessMode::ReadOnly),
        ),
        cast(
            16,
            CastKind::PointerToGeneric,
            15,
            ptr(AddressSpace::Generic, AccessMode::ReadOnly),
        ),
        cast(
            17,
            CastKind::SliceToGeneric,
            1,
            slice(AddressSpace::Generic),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("exact_source_origin");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                ptr(AddressSpace::Global, AccessMode::ReadWrite),
                slice(AddressSpace::Global),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
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
    let result = run(&owner);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

#[test]
fn paid_exact_origin_preserves_original_pointer_slice_and_unknown_value_semantics() {
    with_owner(&fixture(), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        let (legacy, _) = collect_definitions(paid.source).unwrap();
        let types = collect_types(paid.source);
        let retained = budget.storage();
        for _ in 0..3 {
            for (id, origin) in [
                (0, 0),
                (1, 1),
                (2, 2),
                (10, 10),
                (11, 10),
                (12, 10),
                (13, 10),
                (14, 10),
                (15, 0),
                (16, 0),
                (17, 1),
                (u32::MAX, u32::MAX),
            ] {
                assert_eq!(
                    paid.exact_origin(owner, 0, ValueId(id), &mut budget)
                        .unwrap(),
                    Some(ValueId(origin))
                );
                assert_eq!(
                    legacy.exact_ssa_origin(ValueId(id), &types),
                    Some(ValueId(origin))
                );
                assert_eq!(budget.storage(), retained);
            }
        }
        assert_eq!(paid.next_origin_epoch, 36);
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn paid_exact_origin_uses_original_loop_phi_before_private_pointer_cast() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    let ty = body.blocks[0].operations[0].results[0].ty.clone();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(10)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = vec![ValueDef::new(ValueId(30), ty.clone())];
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(30)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(30)],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.parameters = vec![ValueDef::new(ValueId(31), ty)];
    exit.operations.push(cast(
        32,
        CastKind::PointerToGeneric,
        31,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadWrite,
        ),
    ));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([header, exit]);
    with_owner(&module, |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        for id in [30, 31, 32] {
            assert_eq!(
                paid.exact_origin(owner, 0, ValueId(id), &mut budget)
                    .unwrap(),
                Some(ValueId(10))
            );
        }
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn paid_exact_origin_preserves_ambiguous_pointer_phi_without_inventing_a_slot() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    let mut second = body.blocks[0].operations[0].clone();
    second.results[0].id = ValueId(18);
    let ty = second.results[0].ty.clone();
    body.blocks[0].operations.push(second);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(10)],
        else_target: BlockId(1),
        else_arguments: vec![ValueId(18)],
    });
    let mut merge = BasicBlock::new(BlockId(1));
    merge.parameters = vec![ValueDef::new(ValueId(30), ty)];
    merge.operations.push(cast(
        31,
        CastKind::PointerToGeneric,
        30,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadWrite,
        ),
    ));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(merge);
    with_owner(&module, |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        let (legacy, _) = collect_definitions(paid.source).unwrap();
        let types = collect_types(paid.source);
        for id in [30, 31, 10, 18, 31, 30] {
            let expected = matches!(id, 10 | 18).then_some(ValueId(id));
            assert_eq!(
                paid.exact_origin(owner, 0, ValueId(id), &mut budget)
                    .unwrap(),
                expected
            );
            assert_eq!(legacy.exact_ssa_origin(ValueId(id), &types), expected);
        }
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn paid_exact_origin_nested_type_comparison_is_metered_without_cloning() {
    for depth in [0, 1, 4, 16] {
        let mut nested = Type::Scalar(ScalarType::U32);
        for _ in 0..depth {
            nested = Type::pointer(nested, AddressSpace::Global, AccessMode::ReadOnly);
        }
        let from = Type::pointer(nested.clone(), AddressSpace::Private, AccessMode::ReadWrite);
        let to = Type::pointer(nested, AddressSpace::Generic, AccessMode::ReadWrite);
        let operation = cast(1, CastKind::PointerToGeneric, 0, to);
        struct Meter<'a, 'w>(&'a mut Budget<'w>);
        impl Compare for Meter<'_, '_> {
            type Error = VerificationResourceError;
            fn equal(
                &mut self,
                left: &Type,
                right: &Type,
            ) -> std::result::Result<bool, Self::Error> {
                verification_types_equal_v1(left, right, self.0)
            }
            fn step(&mut self) -> std::result::Result<(), Self::Error> {
                self.0.charge_work(1)
            }
        }
        // Dispatch 1, result/target type depth+2, pointee depth+1.
        let exact = 2 * depth + 4;
        for limit in [exact - 1, exact] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, 0);
            let result = exact_origin_v18::pointer(&operation, &from, &mut Meter(&mut budget));
            if limit == exact {
                assert_eq!(result.unwrap(), Some(ValueId(0)));
                assert_eq!(budget.work(), exact);
            } else {
                assert!(matches!(result, Err(VerificationResourceError::Work(_))));
                assert_eq!(budget.failed_work(), Some(exact));
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[test]
fn shared_exact_origin_cast_grammar_rejects_forged_result_element_and_space() {
    let from = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let valid = cast(
        1,
        CastKind::PointerToGeneric,
        0,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadWrite,
        ),
    );
    assert_eq!(
        checked_pointer_cast_source_v18(&valid, &from),
        Some(ValueId(0))
    );
    for mode in 0..5 {
        let mut operation = valid.clone();
        match mode {
            0 => operation.results[0].ty = Type::INDEX,
            1 => {
                let OperationKind::Cast { to, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *to = Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Generic,
                    AccessMode::ReadWrite,
                );
                operation.results[0].ty = to.clone();
            }
            2 => {
                let OperationKind::Cast { to, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *to = from.clone();
                operation.results[0].ty = to.clone();
            }
            3 => {
                let OperationKind::Cast { kind, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *kind = CastKind::Bitcast;
            }
            _ => operation.results.clear(),
        }
        assert_eq!(checked_pointer_cast_source_v18(&operation, &from), None);
    }
}

#[test]
fn paid_exact_origin_query_storage_is_exact_and_one_short_is_sticky() {
    with_owner(&fixture(), |owner| {
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let mut paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let independently_sized = 2 * size_of::<usize>()
                + size_of::<usize>()
                + size_of::<std::thread::Result<Result<Option<ValueId>>>>()
                + size_of::<Result<Option<ValueId>>>()
                + size_of::<(
                    &mut ActualOwnerAffineV18<'static, 'static>,
                    &mut Budget<'static>,
                    ValueId,
                )>();
            assert_eq!(query_frame_v18(), independently_sized);
            let padding = budget.storage_limit() - budget.storage() - independently_sized
                + usize::from(short);
            budget.reserve_storage(padding).unwrap();
            let before = budget.storage();
            let result = paid.exact_origin(owner, 0, ValueId(12), &mut budget);
            if short {
                let error = result.unwrap_err();
                assert!(matches!(
                    error,
                    Failure::Resource(ResourceError::Storage {
                        actual: 1_000_001,
                        limit: 1_000_000
                    })
                ));
                assert_eq!(
                    paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                        .unwrap_err(),
                    error
                );
            } else {
                assert_eq!(result.unwrap(), Some(ValueId(10)));
                assert_eq!(budget.peak_storage(), 1_000_000);
            }
            assert_eq!(budget.storage(), before);
            paid.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), padding);
        }
    });
}

#[test]
fn paid_exact_origin_cumulative_work_short_and_epoch_overflow_never_yield_origin() {
    with_owner(&fixture(), |owner| {
        let (build, query) = {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let before = budget.work();
            assert_eq!(
                paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                    .unwrap(),
                Some(ValueId(10))
            );
            let used = budget.work() - before;
            paid.release(&mut budget).unwrap();
            (before, used)
        };
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(build + query - usize::from(short));
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let floor = budget.storage();
            let result = paid.exact_origin(owner, 0, ValueId(12), &mut budget);
            if short {
                let error = result.unwrap_err();
                assert!(matches!(error, Failure::Resource(ResourceError::Work(_))));
                assert_eq!(
                    paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                        .unwrap_err(),
                    error
                );
            } else {
                assert_eq!(result.unwrap(), Some(ValueId(10)));
            }
            assert_eq!(budget.storage(), floor);
            paid.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut paid =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        paid.next_origin_epoch = usize::MAX;
        assert_eq!(
            paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                .unwrap_err(),
            Failure::Resource(ResourceError::Arithmetic)
        );
        paid.next_origin_epoch = 0;
        assert_eq!(
            paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                .unwrap_err(),
            Failure::Resource(ResourceError::Arithmetic)
        );
        paid.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn paid_exact_origin_keeps_actual_owner_and_original_ledger_identity() {
    let module = fixture();
    with_owner(&module, |owner| {
        with_owner(&module, |other_owner| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut paid =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut other = Budget::new(&mut other_work, usize::MAX);
            assert_eq!(
                paid.exact_origin(owner, 0, ValueId(12), &mut other)
                    .unwrap_err(),
                Failure::Resource(ResourceError::Accounting)
            );
            assert_eq!((other.work(), other.storage()), (0, 0));
            assert_eq!(
                paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                    .unwrap(),
                Some(ValueId(10))
            );
            assert_eq!(
                paid.exact_origin(other_owner, 0, ValueId(12), &mut budget)
                    .unwrap_err(),
                Failure::Resource(ResourceError::Accounting)
            );
            assert_eq!(
                paid.exact_origin(owner, 0, ValueId(12), &mut budget)
                    .unwrap_err(),
                Failure::Resource(ResourceError::Accounting)
            );
            paid.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        })
    });
}
