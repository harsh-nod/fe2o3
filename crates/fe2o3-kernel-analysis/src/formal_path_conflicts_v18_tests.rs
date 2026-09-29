use super::*;

#[path = "formal_path_exclusions_v19_tests.rs"]
mod full_coordinate_tests;
#[path = "formal_path_paid_v20_tests.rs"]
mod paid_source_tests;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalOwnerFormalScopeV18, IntrinsicOperation,
    Kernel, LaunchExtent, MemoryAccess, Signature, StorageLayoutKindV1, StorageLayoutLimitsV1,
    StorageLayoutV1, ValueDef,
};

fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn fixture(bound: u64) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        value(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        value(
            3,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(bound)),
        ),
        value(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        value(
            5,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value(6, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        value(
            7,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut store = BasicBlock::new(BlockId(1));
    store.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(7),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    store.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("nominal-owner-path");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, store, exit],
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
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 20_000_000);
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
    // Inert analysis is deliberately NOT charged to this owner-admission ledger.
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
    result
}
fn launch(extent: u64) -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [extent, 1, 1],
    }
}
fn derive<'a>(
    owner: &'a VerifiedCanonicalKernelIrModuleV18,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
) -> FormalPathConflictsV18<'a> {
    let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
    FormalPathConflictsV18::from_formal(
        scope.derive(&KernelId::new("root"), launch, width).unwrap(),
        Default::default(),
    )
    .unwrap()
}

#[test]
fn actual_owner_singleton_path_preserves_the_full_witness_report() {
    with_owner(&fixture(1), |owner| {
        let result = derive(owner, launch(64), FormalIndexWidth::Bits64);
        assert!(result.belongs_to(owner));
        assert!(std::ptr::eq(result.owner(), owner));
        assert_eq!(result.formal().launch(), launch(64));
        let formal = result.formal().analysis();
        assert!(formal.is_complete());
        assert_eq!(formal.obligations().kernel(), &KernelId::new("root"));
        assert_eq!(
            formal.obligations().entry(),
            &owner.module().functions[0].id
        );
        assert_eq!(formal.obligations().index_width(), FormalIndexWidth::Bits64);
        assert_eq!(formal.obligations().inter_invocation_conflicts().len(), 1);
        assert_eq!(result.decisions(), &[Decision::Disjoint]);
        assert_eq!(result.queries(), 2);
        assert_eq!(owner.module().storage_layouts.len(), 1);
        let mut fresh = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
        let replay = fresh
            .derive(&KernelId::new("root"), launch(64), FormalIndexWidth::Bits64)
            .unwrap();
        assert_eq!(formal, replay.analysis());
    });
}

#[test]
fn changed_and_equal_byte_owners_require_distinct_fresh_compositions() {
    with_owner(&fixture(1), |original| {
        let original_result = derive(original, launch(64), FormalIndexWidth::Bits64);
        with_owner(&fixture(1), |equal_bytes| {
            assert_eq!(original.module(), equal_bytes.module());
            assert!(!original_result.belongs_to(equal_bytes));
            let fresh = derive(equal_bytes, launch(64), FormalIndexWidth::Bits64);
            assert!(fresh.belongs_to(equal_bytes));
            assert!(!fresh.belongs_to(original));
            assert_eq!(fresh.decisions(), &[Decision::Disjoint]);
        });
        with_owner(&fixture(2), |changed| {
            assert!(!original_result.belongs_to(changed));
            let fresh = derive(changed, launch(64), FormalIndexWidth::Bits64);
            assert!(matches!(
                fresh.decisions(),
                [Decision::PossibleOverlap { .. }]
            ));
            assert_eq!(original_result.decisions(), &[Decision::Disjoint]);
        });
    });
}

#[test]
fn incomplete_reports_keep_all_reasons_and_never_discharge_conflicts() {
    with_owner(&fixture(1), |owner| {
        for (launch, width) in [
            (ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64),
            (launch(64), FormalIndexWidth::Bits32),
        ] {
            let result = derive(owner, launch, width);
            let formal = result.formal().analysis();
            assert!(!formal.is_complete());
            assert!(!formal.incomplete_reasons().is_empty());
            assert_eq!(result.formal().launch(), launch);
            assert_eq!(formal.obligations().index_width(), width);
            assert!(
                result
                    .decisions()
                    .iter()
                    .all(|row| *row == Decision::IncompleteFormalReport)
            );
            assert_eq!(result.queries(), 0);
            assert_eq!(result.construction_steps(), 0);
            let mut fresh = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
            assert_eq!(
                formal,
                fresh
                    .derive(&KernelId::new("root"), launch, width)
                    .unwrap()
                    .analysis()
            );
        }
    });
}

#[test]
fn unavailable_call_effects_keep_existing_conflicts_incomplete() {
    let mut module = fixture(1);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "external".into(),
                    arguments: vec![],
                },
            ),
        );
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_owner(&module, |owner| {
        let result = derive(owner, launch(64), FormalIndexWidth::Bits64);
        let report = result.formal().analysis();
        assert_eq!(
            report.incomplete_reasons(),
            &[
                fe2o3_kernel_ir::FormalMemoryIncompleteReason::CallEffectsUnavailable {
                    location: fe2o3_kernel_ir::FunctionOperationLocation::new(BlockId(0), 0),
                    callee: "external".into(),
                }
            ]
        );
        assert_eq!(report.obligations().inter_invocation_conflicts().len(), 1);
        assert_eq!(result.decisions(), &[Decision::IncompleteFormalReport]);
        assert_eq!(result.queries(), 0);
        assert_eq!(result.construction_steps(), 0);
    });
}

#[test]
fn conflict_order_and_selected_root_survive_composition() {
    let mut module = fixture(1);
    let store = module.functions[0].body.as_ref().unwrap().blocks[1].operations[0].clone();
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(store);
    module.kernels.push(Kernel::new(
        "other_root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    with_owner(&module, |owner| {
        let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
        let formal = scope
            .derive(
                &KernelId::new("other_root"),
                launch(64),
                FormalIndexWidth::Bits64,
            )
            .unwrap();
        let before = formal.analysis().clone();
        let result = FormalPathConflictsV18::from_formal(formal, Default::default()).unwrap();
        assert_eq!(result.formal().analysis(), &before);
        assert_eq!(before.obligations().kernel(), &KernelId::new("other_root"));
        assert_eq!(before.obligations().inter_invocation_conflicts().len(), 3);
        assert_eq!(
            result.decisions(),
            &[Decision::Disjoint, Decision::Disjoint, Decision::Disjoint]
        );
        assert_eq!(result.queries(), 6);
    });
}

#[test]
fn original_launch_and_path_limits_cannot_be_substituted_after_formal_extraction() {
    with_owner(&fixture(2), |owner| {
        let one = derive(owner, launch(1), FormalIndexWidth::Bits64);
        let two = derive(owner, launch(2), FormalIndexWidth::Bits64);
        assert_eq!(one.formal().launch(), launch(1));
        assert!(
            one.formal()
                .analysis()
                .obligations()
                .inter_invocation_conflicts()
                .is_empty()
        );
        assert_eq!(two.formal().launch(), launch(2));
        assert!(matches!(
            two.decisions(),
            [Decision::PossibleOverlap { .. }]
        ));
        let mut scope = CanonicalOwnerFormalScopeV18::new(owner, Default::default()).unwrap();
        let formal = scope
            .derive(&KernelId::new("root"), launch(2), FormalIndexWidth::Bits64)
            .unwrap();
        assert!(matches!(
            FormalPathConflictsV18::from_formal(
                formal,
                Limits {
                    conflicts: 0,
                    ..Default::default()
                }
            ),
            Err(Error::Limit {
                resource: Resource::Conflicts,
                actual: 1,
                limit: 0
            })
        ));
        let formal = scope
            .derive(&KernelId::new("root"), launch(2), FormalIndexWidth::Bits64)
            .unwrap();
        assert!(matches!(
            FormalPathConflictsV18::from_formal(
                formal,
                Limits {
                    source_items: 0,
                    ..Default::default()
                }
            ),
            Err(Error::Limit {
                resource: Resource::SourceItems,
                actual: 1,
                limit: 0
            })
        ));
    });
}
