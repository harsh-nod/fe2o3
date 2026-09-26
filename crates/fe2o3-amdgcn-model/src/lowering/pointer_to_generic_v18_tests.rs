use super::*;
use super::storage_native_v18::tests::*;

fn expose(id: u32, source: u32, pointee: Type, access: AccessMode) -> Operation {
    let to = Type::pointer(pointee, Space::Generic, access);
    value(id, to.clone(), OperationKind::Cast {
        kind: CastKind::PointerToGeneric, value: ValueId(source), to,
    })
}

#[test]
fn scalar_and_object_casts_emit_actual_addrspacecast_for_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for object_value in [false, true] {
            for space in [Space::Private, Space::Workgroup, Space::Global] {
                let pointee = if object_value { object(0) } else { Type::Scalar(ScalarType::U32) };
                let source_ty = Type::pointer(pointee.clone(), space, AccessMode::ReadWrite);
                let mut operations = vec![];
                if space != Space::Global {
                    operations.push(value(0, source_ty.clone(), if space == Space::Private {
                        OperationKind::Alloca { element: pointee.clone(), count: None, address_space: space, alignment: 4 }
                    } else {
                        OperationKind::WorkgroupMemory(WorkgroupMemory {
                            element: pointee.clone(), extent: WorkgroupMemoryExtent::Static(1), alignment: 4,
                        })
                    }));
                }
                operations.push(expose(1, 0, pointee, AccessMode::ReadWrite));
                operations.push(constant(2, 17));
                operations.push(if object_value {
                    write(1, 2, Space::Generic, 4)
                } else {
                    Operation::new(vec![], OperationKind::Store {
                        pointer: ValueId(1), value: ValueId(2), access: MemoryAccess::new(Space::Generic, 4),
                    })
                });
                let mut source = module(vec![scalar()], operations, profile);
                if space == Space::Global {
                    source.functions[0].signature.parameters.push(source_ty);
                    source.functions[0].body.as_mut().unwrap().parameters.push(ValueId(0));
                }
                let text = emit(&source, profile);
                let number = match space { Space::Private => 5, Space::Workgroup => 3, Space::Global => 1, _ => unreachable!() };
                let input = if space == Space::Global { "%arg0" } else { "%v0" };
                assert!(text.contains(&format!("%v1 = addrspacecast ptr addrspace({number}) {input} to ptr addrspace(0)")));
                assert!(text.contains("store i32 17, ptr addrspace(0) %v1, align 4"));
                assert!(!text.contains("ptrtoint"));
                assert!(!text.contains("inttoptr"));
            }
        }
    }
}

#[test]
fn constant_internal_helper_keeps_read_only_access_and_real_space_four_conversion() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let element = Type::Scalar(ScalarType::U32);
        let mut source = module(vec![scalar()], vec![], profile);
        let mut body = BasicBlock::new(BlockId(0));
        body.operations = vec![
            expose(1, 0, element.clone(), AccessMode::ReadOnly),
            value(2, element.clone(), OperationKind::Load {
                pointer: ValueId(1), access: MemoryAccess::new(Space::Generic, 4),
            }),
        ];
        body.terminator = Some(Terminator::Return { values: vec![ValueId(2)] });
        source.functions.push(Function::internal_helper("constant_read",
            Signature::new(vec![Type::pointer(element.clone(), Space::Constant, AccessMode::ReadOnly)], vec![element]),
            vec![ValueId(0)], vec![body]));
        bind(&mut source, profile);
        let text = emit(&source, profile);
        assert!(text.contains("addrspacecast ptr addrspace(4) %arg0 to ptr addrspace(0)"));
        assert!(text.contains("load i32, ptr addrspace(0) %v1, align 4"));
    }
}

#[test]
fn generic_scalar_gep_and_guarded_memory_emit_verified_llvm_for_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let element = Type::Scalar(ScalarType::U32);
        let generic = Type::pointer(element.clone(), Space::Generic, AccessMode::ReadWrite);
        let operations = vec![
            expose(1, 0, element.clone(), AccessMode::ReadWrite),
            constant(2, 17),
            constant(3, 0),
            value(4, Type::BOOL, OperationKind::Constant(Constant::Bool(true))),
            value(5, generic, OperationKind::GetElementPointer {
                base: ValueId(1), offset: ValueId(3),
            }),
            Operation::new(vec![], OperationKind::GuardedStore {
                pointer: ValueId(5), predicate: ValueId(4), value: ValueId(2),
                access: MemoryAccess::new(Space::Generic, 4),
            }),
            value(6, element.clone(), OperationKind::GuardedLoad {
                pointer: ValueId(5), predicate: ValueId(4), fallback: ValueId(2),
                access: MemoryAccess::new(Space::Generic, 4),
            }),
            Operation::new(vec![], OperationKind::Store {
                pointer: ValueId(5), value: ValueId(6),
                access: MemoryAccess::new(Space::Generic, 4),
            }),
            value(7, element.clone(), OperationKind::Load {
                pointer: ValueId(5), access: MemoryAccess::new(Space::Generic, 4),
            }),
        ];
        let mut source = module(vec![scalar()], operations, profile);
        source.functions[0].signature.parameters.push(Type::pointer(
            element, Space::Global, AccessMode::ReadWrite,
        ));
        source.functions[0].body.as_mut().unwrap().parameters.push(ValueId(0));
        let text = emit(&source, profile);
        assert!(text.contains("%v5 = getelementptr i32, ptr addrspace(0) %v1, i32 0"));
        assert!(text.contains("load i32, ptr addrspace(0) %v5, align 4"));
        assert!(text.contains("store i32 17, ptr addrspace(0) %v5, align 4"));
        assert!(text.contains("store i32 %v6, ptr addrspace(0) %v5, align 4"));
        assert!(text.contains("%v6 = phi i32"));
        assert!(!text.contains("ptrtoint"));
        assert!(!text.contains("inttoptr"));
    }
}

#[test]
fn shared_native_validator_refuses_capability_and_representation_changes() {
    let private = Type::pointer(Type::F32, Space::Private, AccessMode::ReadWrite);
    let generic = Type::pointer(Type::F32, Space::Generic, AccessMode::ReadWrite);
    let location = LoweringLocation::module(&Module::new("legacy_generic_refusal"));
    for target in [LoweringTarget::Gfx942XnackMinusV1, LoweringTarget::Gfx950XnackMinusV1] {
        assert!(validate_pointer(&private, &location, target).is_ok());
        assert!(validate_pointer(&generic, &location, target).is_err());
        assert!(validate_cast(CastKind::PointerToGeneric, &private, &generic, target).is_ok());
        for (from, to) in [
            (generic.clone(), private.clone()), (generic.clone(), generic.clone()),
            (private.clone(), Type::pointer(Type::Scalar(ScalarType::U32), Space::Generic, AccessMode::ReadWrite)),
            (Type::pointer(Type::F32, Space::Constant, AccessMode::ReadWrite), generic.clone()),
            (Type::pointer(Type::F32, Space::Constant, AccessMode::WriteOnly), Type::pointer(Type::F32, Space::Generic, AccessMode::WriteOnly)),
        ] {
            assert!(validate_cast(CastKind::PointerToGeneric, &from, &to, target).is_err());
        }
    }
}

#[test]
fn actual_v18_native_exposure_has_exact_work_and_peak_boundaries() {
    use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget, VerifiedCanonicalKernelIrModuleV18 as Owner};
    let graph = module(vec![scalar()], vec![allocate(0, 0, Space::Private, 4),
        expose(1, 0, object(0), AccessMode::ReadWrite), constant(2, 7), write(1, 2, Space::Generic, 4)], Profile::Gfx942);
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let result = Owner::from_module_ref_with_verification_budget_v18(&graph,
            StorageLayoutLimitsV1 { rows: 64, edges: 256, containment_depth: 32, object_bytes: 4096 }, &mut budget);
        let success = match result {
            Ok((owner, receipt)) => {
                if budget.reserve_storage(receipt.retained_storage()).is_err() { false }
                else {
                    let floor = budget.storage();
                    let result = super::storage_native_v18::lower_canonical_storage_module_v18(&owner, Profile::Gfx942, &mut budget);
                    assert_eq!(budget.storage(), floor);
                    let success = result.is_ok();
                    drop((result, owner));
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    success
                }
            }
            Err(_) => false,
        };
        assert_eq!(budget.storage(), 17);
        let used = budget.work();
        let peak = budget.peak_storage();
        let denied_storage = budget.failed_storage().is_some();
        (success, used, peak, work.failed_work().is_some(), denied_storage)
    };
    let baseline = run(1_000_000_000, 1_000_000_000);
    assert!(baseline.0);
    let exact = run(baseline.1, baseline.2);
    assert!(exact.0);
    assert_eq!((exact.1, exact.2), (baseline.1, baseline.2));
    let work_short = run(baseline.1 - 1, baseline.2);
    assert!(!work_short.0 && work_short.3);
    let storage_short = run(baseline.1, baseline.2 - 1);
    assert!(!storage_short.0 && storage_short.4);
}
