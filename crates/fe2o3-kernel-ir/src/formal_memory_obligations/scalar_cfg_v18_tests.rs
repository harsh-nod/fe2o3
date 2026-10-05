use super::*;
use crate::{
    BasicBlock, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Kernel, Signature, StorageFieldV1, StorageLayoutIdV1,
    StorageLayoutLimitsV1, StorageLayoutV1, ValueDef, WorkgroupSize,
};

fn module(looping: bool) -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(0));
    let blocks = if looping {
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(1)],
        });
        let mut body = BasicBlock::new(BlockId(1));
        body.parameters
            .push(ValueDef::new(ValueId(2), u32_ty.clone()));
        body.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(1)),
        ));
        body.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(4), u32_ty.clone()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ));
        body.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_arguments: vec![ValueId(4)],
            else_target: BlockId(2),
            else_arguments: vec![ValueId(4)],
        });
        let mut exit = BasicBlock::new(BlockId(2));
        exit.parameters
            .push(ValueDef::new(ValueId(5), u32_ty.clone()));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        vec![entry, body, exit]
    } else {
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut left = BasicBlock::new(BlockId(1));
        left.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(2), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(7)),
        ));
        left.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![ValueId(2)],
        });
        let mut right = BasicBlock::new(BlockId(2));
        right.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(0)),
        ));
        right.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(4), u32_ty.clone()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(3),
            },
        ));
        right.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![ValueId(4)],
        });
        let mut join = BasicBlock::new(BlockId(3));
        join.parameters
            .push(ValueDef::new(ValueId(5), u32_ty.clone()));
        join.terminator = Some(Terminator::Return { values: vec![] });
        vec![entry, left, right, join]
    };
    let mut module = Module::new("scalar_cfg");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::Bool), u32_ty], vec![]),
        vec![ValueId(0), ValueId(1)],
        blocks,
    ));
    let mut kernel = Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn with_owner<T>(module: &Module, run: impl FnOnce(&VerifiedCanonicalKernelIrModuleV18) -> T) -> T {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(37).unwrap();
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
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let result = run(&owner);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
    result
}

fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [64, 1, 1],
    }
}

#[test]
fn scalar_cfg_formal_actual_branch_join_and_loop_reuse_complete_engine() {
    for looping in [false, true] {
        let mut module = module(looping);
        let expected = derive_kernel_memory_obligations_for_launch(
            &module,
            &KernelId::from("root"),
            launch(),
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(expected.is_complete());
        module.storage_layouts.push(StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        });
        assert!(
            verify_module_ref(&module).is_err(),
            "do not erase V18 storage metadata for a legacy token"
        );
        with_owner(&module, |owner| {
            let mut scope = CanonicalScalarCfgFormalScopeV18::new(owner).unwrap();
            assert!(std::ptr::eq(scope.owner(), owner));
            let actual = scope
                .derive(&KernelId::from("root"), launch(), FormalIndexWidth::Bits64)
                .unwrap();
            assert_eq!(actual, expected);
            assert!(actual.obligations().accesses().is_empty());
            assert!(actual.obligations().allocations().is_empty());
            assert!(actual.obligations().bounds_requirements().is_empty());
            assert!(actual.obligations().runtime_alias_requirements().is_empty());
            assert!(actual.obligations().inter_invocation_conflicts().is_empty());
            assert_eq!(owner.module(), &module);
        });
    }
}

#[test]
fn scalar_cfg_formal_unknown_launch_and_wrong_index_remain_incomplete() {
    for (extent, width) in [
        (ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64),
        (launch(), FormalIndexWidth::Bits32),
    ] {
        let mut module = module(false);
        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        };
        with_owner(&module, |owner| {
            let mut scope = CanonicalScalarCfgFormalScopeV18::new(owner).unwrap();
            let result = scope
                .derive(&KernelId::from("root"), extent, width)
                .unwrap();
            assert!(!result.is_complete());
            assert!(!result.incomplete_reasons().is_empty());
        });
    }
}

#[test]
fn scalar_cfg_formal_unused_aggregate_metadata_and_index_signatures_refuse() {
    let mut aggregate = module(false);
    aggregate.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    with_owner(&aggregate, |owner| {
        assert!(matches!(
            CanonicalScalarCfgFormalScopeV18::new(owner),
            Err(CanonicalScalarCfgFormalErrorV18::Unsupported(
                "non-scalar storage metadata"
            ))
        ))
    });
    let mut index = module(false);
    index.functions[0].signature.parameters.push(Type::INDEX);
    index.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(99));
    with_owner(&index, |owner| {
        assert!(matches!(
            CanonicalScalarCfgFormalScopeV18::new(owner),
            Err(CanonicalScalarCfgFormalErrorV18::Unsupported(
                "non-scalar or returned signature"
            ))
        ))
    });
}

#[test]
fn scalar_cfg_formal_replay_and_missing_root_keep_real_refusals() {
    with_owner(&module(false), |owner| {
        let mut scope = CanonicalScalarCfgFormalScopeV18::new(owner).unwrap();
        assert!(matches!(
            scope.derive(
                &KernelId::from("absent"),
                launch(),
                FormalIndexWidth::Bits64
            ),
            Err(CanonicalScalarCfgFormalErrorV18::Formal(
                FormalMemoryObligationError::MissingKernel { .. }
            ))
        ));
        assert!(matches!(
            scope.derive(&KernelId::from("root"), launch(), FormalIndexWidth::Bits64),
            Err(CanonicalScalarCfgFormalErrorV18::Unsupported(
                "formal root replay bound"
            ))
        ));
    });
}

#[test]
fn scalar_cfg_formal_census_bounds_fail_before_effects_and_do_not_wrap() {
    assert_eq!(work_v18(16, 31, 2).unwrap(), 6_096);
    for (nodes, names, roots) in [
        (usize::MAX, 0, 0),
        (16, usize::MAX, 2),
        (16, 31, usize::MAX),
        (1024, 0, 0),
    ] {
        assert!(matches!(
            work_v18(nodes, names, roots),
            Err(CanonicalScalarCfgFormalErrorV18::Unsupported(
                "formal module work bound"
            ))
        ));
    }
    let mut exact = MAX_NODES_V18 - 1;
    add_v18(&mut exact, 1).unwrap();
    assert!(add_v18(&mut exact, 1).is_err());
    // Eight legal identifiers reach the aggregate census without violating the
    // canonical per-identifier 4096-byte admission limit first.
    assert_eq!(crate::wire::MAX_TEXT_BYTES_V1, 4096);
    let name_len = crate::wire::MAX_TEXT_BYTES_V1 - 1;
    for over in [false, true] {
        let template = module(false);
        let mut boundary = Module::new("name_census_boundary");
        for ordinal in 0..8 {
            let mut function = template.functions[0].clone();
            function.id = FunctionId::from(format!("{ordinal}{}", "x".repeat(name_len - 1)));
            let mut kernel = template.kernels[0].clone();
            kernel.id = KernelId::from(if over && ordinal == 0 {
                "k00".to_owned()
            } else {
                format!("k{ordinal}")
            });
            kernel.entry = function.id.clone();
            boundary.functions.push(function);
            boundary.kernels.push(kernel);
        }
        let names = boundary
            .functions
            .iter()
            .map(|f| f.id.as_str().len())
            .sum::<usize>()
            + boundary
                .kernels
                .iter()
                .map(|k| k.id.as_str().len() + k.entry.as_str().len())
                .sum::<usize>();
        assert_eq!(names, MAX_NODES_V18 + usize::from(over));
        with_owner(&boundary, |owner| {
            let expected = if names == MAX_NODES_V18 {
                "formal module work bound"
            } else {
                "formal module node bound"
            };
            assert!(matches!(
                CanonicalScalarCfgFormalScopeV18::new(owner),
                Err(CanonicalScalarCfgFormalErrorV18::Unsupported(actual)) if actual == expected
            ));
        });
    }
}

#[test]
fn scalar_cfg_formal_memory_in_unreachable_block_still_refuses() {
    let mut module = module(false);
    let mut dead = BasicBlock::new(BlockId(99));
    dead.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(99),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    dead.terminator = Some(Terminator::Unreachable);
    module.functions[0].body.as_mut().unwrap().blocks.push(dead);
    with_owner(&module, |owner| {
        assert!(matches!(
            CanonicalScalarCfgFormalScopeV18::new(owner),
            Err(CanonicalScalarCfgFormalErrorV18::Unsupported(
                "non-scalar operation result"
            ))
        ));
    });
}
