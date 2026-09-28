use crate::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, Signature, StorageFieldV1,
    StorageLayoutIdV1, StorageLayoutLimitsV1, StorageLayoutV1, ValueDef, ValueId, WorkgroupSize,
};

fn scalar_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("formal_v18");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![block],
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

fn extract(
    module: &Module,
) -> Result<FormalMemoryObligationAnalysis, CanonicalClosedScalarFormalErrorV18> {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(37).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let result = derive_canonical_closed_scalar_memory_obligations_v18(
        &owner,
        &KernelId::from("root"),
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        FormalIndexWidth::Bits64,
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before,
        "formal extraction keeps its existing separate engine policy"
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
    result
}

#[test]
fn closed_scalar_formal_v18_reuses_full_engine_without_legacy_token_or_table_removal() {
    let mut module = scalar_module();
    let legacy = derive_kernel_memory_obligations_for_launch(
        &module,
        &KernelId::from("root"),
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert_eq!(extract(&module).unwrap(), legacy);
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 0,
            alignment: 1,
            kind: StorageLayoutKindV1::Record(Box::new([])),
        },
    ];
    assert!(verify_module_ref(&module).is_err());
    let actual = extract(&module).unwrap();
    assert_eq!(actual, legacy);
    assert!(actual.is_complete());
    assert!(actual.obligations().accesses().is_empty());
    assert!(actual.obligations().allocations().is_empty());
    assert_eq!(module.storage_layouts.len(), 2);
}

#[test]
fn closed_scalar_formal_v18_refuses_unused_aggregate_metadata_and_dead_nonconstant_syntax() {
    let mut module = scalar_module();
    module.storage_layouts = vec![
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
    assert!(matches!(
        extract(&module),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "non-scalar storage metadata"
        ))
    ));
    module.storage_layouts.clear();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Unary {
            op: crate::UnaryOp::Not,
            operand: ValueId(0),
        };
    assert!(matches!(
        extract(&module),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "non-constant scalar operation"
        ))
    ));
}

#[test]
fn closed_scalar_formal_v18_keeps_real_missing_root_and_control_refusals() {
    let mut module = scalar_module();
    module.functions[0].body.as_mut().unwrap().blocks.push({
        let mut next = BasicBlock::new(BlockId(1));
        next.terminator = Some(Terminator::Return { values: vec![] });
        next
    });
    assert!(matches!(
        extract(&module),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "control flow"
        ))
    ));
    module.functions[0].body.as_mut().unwrap().blocks.pop();
    module.kernels[0].id = KernelId::from("different");
    assert!(
        matches!(extract(&module), Err(CanonicalClosedScalarFormalErrorV18::Formal(FormalMemoryObligationError::MissingKernel { kernel })) if kernel == KernelId::from("root"))
    );
}

include!("closed_scalar_bounded_v1765_tests.rs");
