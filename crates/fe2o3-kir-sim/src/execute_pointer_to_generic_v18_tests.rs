use super::storage_tests_v1::*;
use super::storage_views_tests_v1::{invocation, memory, variant_module};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    StorageLayoutLimitsV1, VerifiedCanonicalKernelIrModuleV18,
};

fn admit(graph: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        graph,
        StorageLayoutLimitsV1 { rows: 64, edges: 256, containment_depth: 32, object_bytes: 4096 },
        &mut budget,
    ).unwrap().0
}

fn execute(graph: &Module) -> Result<crate::SimulationExecutionV18, SimulationErrorV1> {
    crate::simulate_canonical_storage_v18(
        &admit(graph), &request(), SimulationTargetV1::amdgpu_64(), SimulationLimitsV1::default(),
    )
}

fn exposed(id: u32, source: u32, pointee: Type) -> Operation {
    let to = Type::pointer(pointee, AddressSpace::Generic, AccessMode::ReadWrite);
    value(id, to.clone(), OperationKind::Cast {
        kind: CastKind::PointerToGeneric, value: ValueId(source), to,
    })
}

fn bits(result: &crate::SimulationExecutionV18) -> u32 {
    u32::from_le_bytes(result.buffer(0).unwrap().bytes().try_into().unwrap())
}

#[test]
fn exposure_changes_static_view_not_allocation_or_access_authority() {
    for space in [AddressSpace::Global, AddressSpace::Constant, AddressSpace::Private, AddressSpace::Workgroup] {
        let (mut memory, address) = memory(vec![0; 4], vec![true; 4], space);
        let mut pointer = address.pointer;
        let access = MemoryAccess::new(AddressSpace::Generic, 4);
        assert!(matches!(validate_access(memory.allocation(&pointer).unwrap(), &pointer, access, 4, false),
            Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch)));
        pointer.generic_exposed = true;
        if space == AddressSpace::Constant { pointer.access = AccessMode::ReadOnly; }
        assert_eq!(pointer.visible_address_space(), AddressSpace::Generic);
        assert_eq!(memory.allocation(&pointer).unwrap().address_space, space);
        validate_access(memory.allocation(&pointer).unwrap(), &pointer, access, 4, false).unwrap();
        assert!(matches!(validate_access(memory.allocation(&pointer).unwrap(), &pointer,
            MemoryAccess::new(space, 4), 4, false), Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch)));
        pointer.byte_offset = 4;
        assert!(validate_access(memory.allocation(&pointer).unwrap(), &pointer, access, 4, false).is_err());
        pointer.byte_offset = 0;
        if space == AddressSpace::Constant {
            pointer.access = AccessMode::ReadWrite;
            assert!(matches!(validate_access(memory.allocation(&pointer).unwrap(), &pointer, access, 4, true),
                Err(SimulationExecutionErrorKindV1::ReadOnlyWrite)));
        }
        memory.allocations.remove(&pointer.allocation);
        assert!(matches!(memory.allocation(&pointer), Err(SimulationExecutionErrorKindV1::DanglingPointer { .. })));
    }
}

#[test]
fn symbolic_pointer_relocations_require_exposure_and_keep_concrete_lifetime() {
    for space in [AddressSpace::Global, AddressSpace::Private, AddressSpace::Workgroup, AddressSpace::Constant] {
        let (mut memory, address) = memory(vec![0; 4], vec![true; 4], space);
        let mut pointer = address.pointer;
        pointer.element = ScalarType::U32;
        pointer.access = AccessMode::ReadOnly;
        let representation = StoragePointerV1 {
            pointee: StorageLayoutIdV1(0), value_space: AddressSpace::Generic,
            encoded_space: AddressSpace::Generic, access: AccessMode::ReadOnly, stored_bits: 64,
        };
        let validate = |memory: &Memory, pointer: PointerValue| {
            memory.storage_validate_pointer_v1(&StoragePointerPayloadV1::Scalar(pointer), representation, invocation())
        };
        assert!(validate(&memory, pointer.clone()).is_err());
        pointer.generic_exposed = true;
        validate(&memory, pointer.clone()).unwrap();
        let allocation = memory.allocations.get_mut(&pointer.allocation).unwrap();
        allocation.address_space = AddressSpace::Generic;
        assert!(validate(&memory, pointer.clone()).is_err());
        memory.allocations.remove(&pointer.allocation);
        assert!(validate(&memory, pointer).is_err());
    }
}

#[test]
fn exposed_workgroup_ranges_still_require_publication() {
    let (mut memory, address) = memory(vec![0; 4], vec![true; 4], AddressSpace::Workgroup);
    let mut pointer = address.pointer;
    pointer.generic_exposed = true;
    assert!(matches!(memory.validate_range_read(&pointer, 4, 4, invocation()),
        Err(SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish { .. })));
    memory.allocations.get_mut(&pointer.allocation).unwrap().workgroup_published.fill(true);
    memory.validate_range_read(&pointer, 4, 4, invocation()).unwrap();
    memory.validate_range_write(&pointer, 4, 4).unwrap();
}

#[test]
fn actual_v18_storage_projects_and_checks_bounds_after_exposure() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        for index in [1, 2] {
            let graph = module(rows(), vec![
                allocate(1, 2, space), exposed(2, 1, object(2)), constant(3, 91),
                value(4, Type::INDEX, OperationKind::Constant(Constant::Index(index))),
                project(5, 0, 2, StorageProjectionV1::ArrayIndex(ValueId(4)), AddressSpace::Generic),
                write(5, 3, AddressSpace::Generic), read(6, 5, AddressSpace::Generic), output(6),
            ]);
            let result = execute(&graph);
            if index == 1 { assert_eq!(bits(&result.unwrap()), 91); }
            else { assert!(result.is_err()); }
        }
    }
}

#[test]
fn exposure_preserves_variant_guard_generation_and_uninitialized_rejection() {
    for (stale, uninitialized) in [(false, false), (true, false), (false, true)] {
        let mut graph = variant_module(stale, uninitialized);
        let operations = &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations;
        let read_position = operations.iter().position(|operation| operation.results.first().is_some_and(|result| result.id == ValueId(8))).unwrap();
        operations.insert(read_position, exposed(20, 6, object(0)));
        operations[read_position + 1] = read(8, 20, AddressSpace::Generic);
        let result = execute(&graph);
        if stale || uninitialized { assert!(result.is_err()); }
        else { assert_eq!(bits(&result.unwrap()), 99); }
    }
}

#[test]
fn generic_pointer_descriptor_copy_roundtrip_keeps_symbolic_backing() {
    let mut layouts = vec![scalar_row()];
    layouts.push(StorageLayoutV1 {
        size: 8, alignment: 8, kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(0), value_space: AddressSpace::Generic,
            encoded_space: AddressSpace::Generic, access: AccessMode::ReadWrite, stored_bits: 64,
        }),
    });
    let scalar_pointer = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite);
    let slot = |id| value(id, address(1, AddressSpace::Private), OperationKind::Alloca {
        element: object(1), count: None, address_space: AddressSpace::Private, alignment: 8,
    });
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    let graph = module(layouts, vec![
        exposed(1, 0, Type::Scalar(ScalarType::U32)), slot(2), slot(3),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(2), value: ValueId(1), access,
        })),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::CopyObject {
            destination: ValueId(3), source: ValueId(2),
            source_access: access, destination_access: access, overlap: StorageCopyOverlapV1::NonOverlapping,
        })),
        value(4, scalar_pointer, OperationKind::Storage(StorageOperationV1::ReadValue { address: ValueId(3), access })),
        constant(5, 71), Operation::new(vec![], OperationKind::Store {
            pointer: ValueId(4), value: ValueId(5), access: MemoryAccess::new(AddressSpace::Generic, 4),
        }),
    ]);
    assert_eq!(bits(&execute(&graph).unwrap()), 71);
}

#[test]
fn generic_slice_descriptor_and_slice_data_preserve_exposure() {
    let slice = Type::slice(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite);
    let layouts = vec![scalar_row(),
        StorageLayoutV1 { size: 8, alignment: 8, kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(0), value_space: AddressSpace::Generic,
            encoded_space: AddressSpace::Generic, access: AccessMode::ReadWrite, stored_bits: 64,
        }) },
        StorageLayoutV1 { size: 8, alignment: 8, kind: StorageLayoutKindV1::Scalar(ScalarType::Index) },
        StorageLayoutV1 { size: 16, alignment: 8, kind: StorageLayoutKindV1::Slice {
            element: StorageLayoutIdV1(0), value_space: AddressSpace::Generic, access: AccessMode::ReadWrite,
            data: StorageFieldV1 { offset: 0, layout: StorageLayoutIdV1(1) },
            length: StorageFieldV1 { offset: 8, layout: StorageLayoutIdV1(2) },
        } },
    ];
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    let graph = module(layouts, vec![
        exposed(1, 0, Type::Scalar(ScalarType::U32)),
        value(2, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        value(10, address(3, AddressSpace::Private), OperationKind::Alloca {
            element: object(3), count: None, address_space: AddressSpace::Private, alignment: 8,
        }),
        project(11, 1, 10, StorageProjectionV1::Field(0), AddressSpace::Private),
        project(12, 2, 10, StorageProjectionV1::Field(1), AddressSpace::Private),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue { address: ValueId(11), value: ValueId(1), access })),
        Operation::new(vec![], OperationKind::Storage(StorageOperationV1::WriteValue { address: ValueId(12), value: ValueId(2), access })),
        value(3, slice, OperationKind::Storage(StorageOperationV1::ReadValue { address: ValueId(10), access })),
        value(4, Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite),
            OperationKind::SliceData { slice: ValueId(3) }),
        constant(5, 81), Operation::new(vec![], OperationKind::Store {
            pointer: ValueId(4), value: ValueId(5), access: MemoryAccess::new(AddressSpace::Generic, 4),
        }),
    ]);
    assert_eq!(bits(&execute(&graph).unwrap()), 81);
}

#[test]
fn exposure_survives_select_gep_call_return_block_argument_and_restriction() {
    let element = Type::Scalar(ScalarType::U32);
    let generic = Type::pointer(element.clone(), AddressSpace::Generic, AccessMode::ReadWrite);
    let read_only = Type::pointer(element.clone(), AddressSpace::Generic, AccessMode::ReadOnly);
    let mut graph = module(vec![], vec![
        constant(1, 47), output(1), exposed(2, 0, element.clone()),
        value(3, Type::BOOL, OperationKind::Constant(Constant::Bool(true))),
        value(4, generic.clone(), OperationKind::Select { condition: ValueId(3), true_value: ValueId(2), false_value: ValueId(2) }),
        value(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        value(6, generic.clone(), OperationKind::GetElementPointer { base: ValueId(4), offset: ValueId(5) }),
        value(7, generic.clone(), OperationKind::Call { callee: fe2o3_kernel_ir::FunctionId::new("identity"), arguments: vec![ValueId(6)] }),
    ]);
    let body = graph.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch { target: BlockId(1), arguments: vec![ValueId(7)] });
    let mut next = BasicBlock::new(BlockId(1));
    next.parameters.push(ValueDef::new(ValueId(8), generic.clone()));
    next.operations = vec![
        value(9, read_only.clone(), OperationKind::Cast { kind: CastKind::RestrictPointerAccess, value: ValueId(8), to: read_only }),
        value(10, element, OperationKind::Load { pointer: ValueId(9), access: MemoryAccess::new(AddressSpace::Generic, 4) }),
        output(10),
    ];
    next.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(next);
    let mut helper = BasicBlock::new(BlockId(0));
    helper.terminator = Some(Terminator::Return { values: vec![ValueId(0)] });
    graph.functions.push(Function::internal_helper("identity", fe2o3_kernel_ir::Signature::new(vec![generic.clone()], vec![generic]), vec![ValueId(0)], vec![helper]));
    assert_eq!(bits(&execute(&graph).unwrap()), 47);
}

#[test]
fn exposed_pointer_returned_after_its_private_frame_ends_is_dangling() {
    let element = Type::Scalar(ScalarType::U32);
    let generic = Type::pointer(element.clone(), AddressSpace::Generic, AccessMode::ReadWrite);
    let mut graph = module(vec![], vec![
        value(1, generic.clone(), OperationKind::Call { callee: fe2o3_kernel_ir::FunctionId::new("expired"), arguments: vec![] }),
        value(2, element.clone(), OperationKind::Load { pointer: ValueId(1), access: MemoryAccess::new(AddressSpace::Generic, 4) }), output(2),
    ]);
    let mut helper = BasicBlock::new(BlockId(0));
    helper.operations = vec![
        value(0, Type::pointer(element.clone(), AddressSpace::Private, AccessMode::ReadWrite), OperationKind::Alloca {
            element: element.clone(), count: None, address_space: AddressSpace::Private, alignment: 4,
        }), constant(1, 12),
        Operation::new(vec![], OperationKind::Store { pointer: ValueId(0), value: ValueId(1), access: MemoryAccess::new(AddressSpace::Private, 4) }),
        exposed(2, 0, element),
    ];
    helper.terminator = Some(Terminator::Return { values: vec![ValueId(2)] });
    graph.functions.push(Function::internal_helper("expired", fe2o3_kernel_ir::Signature::new(vec![], vec![generic]), vec![], vec![helper]));
    let error = execute(&graph).unwrap_err();
    assert!(format!("{error:?}").contains("DanglingPointer"), "{error:?}");
}
