use fe2o3_kernel_ir::*;

fn ptr(pointee: Type, space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(pointee, space, access)
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn kernel(parameters: Vec<Type>, mut blocks: Vec<BasicBlock>) -> Module {
    for block in &mut blocks {
        if block.terminator.is_none() {
            block.terminator = Some(Terminator::Return { values: vec![] });
        }
    }
    let values = (0..parameters.len() as u32).map(ValueId).collect();
    let mut module = Module::new("generic-formal");
    module.functions.push(Function::kernel_entry(
        "entry", Signature::new(parameters, vec![]), values, blocks,
    ));
    module.kernels.push(Kernel::new("kernel", "entry", LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    }));
    module
}
fn analyze(module: &Module) -> FormalMemoryObligationAnalysis {
    derive_kernel_memory_obligations(module, &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(1), FormalIndexWidth::Bits64).unwrap()
}

#[test]
fn checked_exposure_preserves_the_concrete_formal_allocation_and_receipt_plane() {
    for space in [AddressSpace::Global, AddressSpace::Constant, AddressSpace::Private, AddressSpace::Workgroup] {
        let mut block = BasicBlock::new(BlockId(0));
        let generic = ptr(Type::F32, AddressSpace::Generic, AccessMode::ReadOnly);
        block.operations = vec![
            op(1, generic.clone(), OperationKind::Cast {
                kind: CastKind::PointerToGeneric, value: ValueId(0), to: generic,
            }),
            op(2, Type::F32, OperationKind::Load {
                pointer: ValueId(1), access: MemoryAccess::new(AddressSpace::Generic, 4),
            }),
        ];
        let module = kernel(vec![ptr(Type::F32, space, AccessMode::ReadOnly)], vec![block]);
        let report = analyze(&module);
        assert!(report.is_complete(), "{report:?}");
        let rows = report.obligations().accesses();
        if space == AddressSpace::Private {
            assert!(rows.is_empty());
            continue;
        }
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].allocation().parameter_index(), 0);
        assert_eq!(rows[0].address_space(), space);
        let receipt = InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(report.obligations()).unwrap();
        receipt.revalidate().unwrap();
        let replay = InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(receipt.canonical_bytes().to_vec()).unwrap();
        assert_eq!(replay.canonical_bytes(), receipt.canonical_bytes());
    }
}

#[test]
fn generic_formal_parameter_is_not_reclassified_as_global() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(op(1, Type::F32, OperationKind::Load {
        pointer: ValueId(0), access: MemoryAccess::new(AddressSpace::Generic, 4),
    }));
    let module = kernel(vec![ptr(Type::F32, AddressSpace::Generic, AccessMode::ReadOnly)], vec![block]);
    let report = analyze(&module);
    assert!(report.is_complete());
    assert_eq!(report.obligations().accesses()[0].address_space(), AddressSpace::Generic);
}

#[test]
fn generic_private_slot_keeps_the_stored_global_pointer_origin() {
    let global = ptr(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let private = ptr(global.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let generic = ptr(global.clone(), AddressSpace::Generic, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(1, private, OperationKind::Alloca {
            element: global.clone(), count: None, address_space: AddressSpace::Private, alignment: 8,
        }),
        op(2, generic.clone(), OperationKind::Cast {
            kind: CastKind::PointerToGeneric, value: ValueId(1), to: generic,
        }),
        Operation::new(vec![], OperationKind::Store {
            pointer: ValueId(2), value: ValueId(0), access: MemoryAccess::new(AddressSpace::Generic, 8),
        }),
        op(3, global.clone(), OperationKind::Load {
            pointer: ValueId(2), access: MemoryAccess::new(AddressSpace::Generic, 8),
        }),
        op(4, Type::F32, OperationKind::Load {
            pointer: ValueId(3), access: MemoryAccess::new(AddressSpace::Global, 4),
        }),
    ];
    let report = analyze(&kernel(vec![global], vec![block]));
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(report.obligations().accesses().len(), 1);
    assert_eq!(report.obligations().accesses()[0].allocation().parameter_index(), 0);
}

fn guarded(before_gep: bool, bound: bool) -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let global = ptr(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let generic = ptr(element.clone(), AddressSpace::Generic, AccessMode::ReadOnly);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(3, Type::INDEX, OperationKind::SliceLength { slice: ValueId(0) }),
        op(4, Type::BOOL, OperationKind::Compare {
            predicate: ComparePredicate::LessThan, lhs: ValueId(1), rhs: ValueId(3),
        }),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(if bound { 4 } else { 2 }),
        then_target: BlockId(1), then_arguments: vec![],
        else_target: BlockId(2), else_arguments: vec![],
    });
    let mut read = BasicBlock::new(BlockId(1));
    read.operations.push(op(10, global.clone(), OperationKind::SliceData { slice: ValueId(0) }));
    if before_gep {
        read.operations.push(op(11, generic.clone(), OperationKind::Cast {
            kind: CastKind::PointerToGeneric, value: ValueId(10), to: generic.clone(),
        }));
        read.operations.push(op(12, generic, OperationKind::GetElementPointer {
            base: ValueId(11), offset: ValueId(1),
        }));
    } else {
        read.operations.push(op(11, global, OperationKind::GetElementPointer {
            base: ValueId(10), offset: ValueId(1),
        }));
        read.operations.push(op(12, generic.clone(), OperationKind::Cast {
            kind: CastKind::PointerToGeneric, value: ValueId(11), to: generic,
        }));
    }
    read.operations.push(op(13, element.clone(), OperationKind::Load {
        pointer: ValueId(12), access: MemoryAccess::new(AddressSpace::Generic, 4),
    }));
    kernel(vec![Type::slice(element, AddressSpace::Global, AccessMode::ReadOnly), Type::INDEX, Type::BOOL],
        vec![entry, read, BasicBlock::new(BlockId(2))])
}

#[test]
fn actual_v18_guarded_queries_retain_original_pointer_and_bound_through_exposure() {
    for before in [false, true] {
        for bound in [false, true] {
            let module = guarded(before, bound);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
            let (owner, storage) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module, StorageLayoutLimitsV1 { rows: 8, edges: 16, containment_depth: 8, object_bytes: 1024 }, &mut budget,
            ).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            with_canonical_guarded_global_reads_v18(&owner, Default::default(), &mut budget, |view, budget| {
                assert_eq!(view.function_effects(CanonicalKirFunctionCoordinateV1(0), budget)?, (1, 0, 0));
                let outcome = view.read_at(CanonicalKirOperationCoordinateV1 {
                    block: CanonicalKirBlockCoordinateV1 { function: CanonicalKirFunctionCoordinateV1(0), block: 1 },
                    operation: 3,
                }, budget)?;
                match outcome {
                    CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) => {
                        assert!(bound);
                        assert_eq!(read.domain().pointer(), ValueId(12));
                        assert_eq!(read.domain().slice(), ValueId(0));
                        assert_eq!(read.domain().index(), ValueId(1));
                    }
                    CanonicalGuardedGlobalReadOutcomeV18::NotProved(reason) => {
                        assert!(!bound, "before_gep={before}, bound={bound}: {reason:?}");
                    }
                }
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn generic_effect_census_never_omits_unresolved_reads_or_writes() {
    let generic = ptr(Type::F32, AddressSpace::Generic, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(1, Type::F32, OperationKind::Load { pointer: ValueId(0), access: MemoryAccess::new(AddressSpace::Generic, 4) }),
        Operation::new(vec![], OperationKind::Store { pointer: ValueId(0), value: ValueId(1), access: MemoryAccess::new(AddressSpace::Generic, 4) }),
    ];
    let module = kernel(vec![generic], vec![block]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
    let (owner, storage) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module, StorageLayoutLimitsV1 { rows: 8, edges: 16, containment_depth: 8, object_bytes: 1024 }, &mut budget,
    ).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    with_canonical_guarded_global_reads_v18(&owner, Default::default(), &mut budget, |view, budget| {
        assert_eq!(view.function_effects(CanonicalKirFunctionCoordinateV1(0), budget)?, (1, 1, 0));
        assert!(matches!(view.read_at(CanonicalKirOperationCoordinateV1 {
            block: CanonicalKirBlockCoordinateV1 { function: CanonicalKirFunctionCoordinateV1(0), block: 0 }, operation: 0,
        }, budget)?, CanonicalGuardedGlobalReadOutcomeV18::NotProved(_)));
        Ok(())
    }).unwrap();
}

#[test]
fn same_typed_foreign_slice_does_not_inherit_another_slices_bound() {
    let mut module = guarded(true, true);
    let function = &mut module.functions[0];
    function.signature.parameters.push(function.signature.parameters[0].clone());
    function.body.as_mut().unwrap().parameters.push(ValueId(50));
    function.body.as_mut().unwrap().blocks[1].operations[0].kind = OperationKind::SliceData { slice: ValueId(50) };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
    let (owner, storage) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module, StorageLayoutLimitsV1 { rows: 8, edges: 16, containment_depth: 8, object_bytes: 1024 }, &mut budget,
    ).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    with_canonical_guarded_global_reads_v18(&owner, Default::default(), &mut budget, |view, budget| {
        assert_eq!(view.function_effects(CanonicalKirFunctionCoordinateV1(0), budget)?, (1, 0, 0));
        assert!(matches!(view.read_at(CanonicalKirOperationCoordinateV1 {
            block: CanonicalKirBlockCoordinateV1 { function: CanonicalKirFunctionCoordinateV1(0), block: 1 }, operation: 3,
        }, budget)?, CanonicalGuardedGlobalReadOutcomeV18::NotProved(_)));
        Ok(())
    }).unwrap();
}

#[test]
fn selected_different_concrete_roots_cannot_mint_one_exact_formal_origin() {
    let concrete = ptr(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let generic = ptr(Type::F32, AddressSpace::Generic, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(3, generic.clone(), OperationKind::Cast { kind: CastKind::PointerToGeneric, value: ValueId(0), to: generic.clone() }),
        op(4, generic.clone(), OperationKind::Cast { kind: CastKind::PointerToGeneric, value: ValueId(1), to: generic.clone() }),
        op(5, generic, OperationKind::Select { condition: ValueId(2), true_value: ValueId(3), false_value: ValueId(4) }),
        op(6, Type::F32, OperationKind::Load { pointer: ValueId(5), access: MemoryAccess::new(AddressSpace::Generic, 4) }),
    ];
    let report = analyze(&kernel(vec![concrete.clone(), concrete, Type::BOOL], vec![block]));
    assert!(!report.is_complete());
}
