pub(super) use super::storage_tests_v1::{Events, constant, output, value};
use super::*;
pub(super) use crate::storage_inputs_v29::*;
pub(super) use fe2o3_kernel_ir::{
    StorageCopyOverlapV1, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1,
    StorageOperationV1, StoragePointerV1, StorageProjectionV1, StorageVariantEncodingV1,
    StorageVariantV1, VerifiedCanonicalKernelIrModuleV18,
};

pub(super) fn targets() -> [SimulationTargetV1; 2] {
    [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ]
    .map(SimulationTargetV1::amdgpu_profile)
}

pub(super) fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_call_depth: 8,
        max_ssa_values: 64,
        max_memory_access_records: 128,
        ..SimulationLimitsV1::default()
    }
}

pub(super) fn scalar() -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    }
}

pub(super) fn pointer(pointee: u32) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(pointee),
            value_space: AddressSpace::Global,
            encoded_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            stored_bits: 64,
        }),
    }
}

pub(super) fn object_pointer(id: u32, space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(Type::StorageObject(StorageLayoutIdV1(id)), space, access)
}

pub(super) fn inline_type(id: u32) -> Type {
    object_pointer(id, AddressSpace::Constant, AccessMode::ReadOnly)
}

pub(super) fn graph(
    rows: Vec<StorageLayoutV1>,
    parameters: Vec<Type>,
    operations: Vec<Operation>,
) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("explicit-storage-inputs");
    module.storage_layouts = rows;
    let values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    module.functions.push(Function::kernel_entry(
        "entry",
        fe2o3_kernel_ir::Signature::new(parameters, vec![]),
        values,
        vec![block],
    ));
    let mut kernel = fe2o3_kernel_ir::Kernel::new(
        "entry",
        "entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    module
}

pub(super) fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module,
        fe2o3_kernel_ir::StorageLayoutLimitsV1 {
            rows: 256,
            edges: 1024,
            containment_depth: 128,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap()
    .0
}

pub(super) fn request(arguments: Vec<SimulationStorageArgumentV29>) -> SimulationStorageRequestV29 {
    SimulationStorageRequestV29 {
        kernel: "entry".into(),
        grid: crate::GridShapeV1([1, 1, 1]),
        workgroup: crate::WorkgroupShapeV1([1, 1, 1]),
        arguments,
        shared_storage: vec![],
        events: EventPolicyV1::Enabled,
    }
}

pub(super) fn image(
    layout: u32,
    alignment: u32,
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    relocations: Vec<SimulationObjectRelocationV29>,
) -> SimulationObjectImageV29 {
    SimulationObjectImageV29::new(
        StorageLayoutIdV1(layout),
        alignment,
        bytes,
        initialized,
        relocations,
    )
    .unwrap()
}

pub(super) fn view(
    backing: u32,
    layout: u32,
    path: Vec<SimulationObjectComponentV29>,
) -> SimulationObjectViewV29 {
    SimulationObjectViewV29 {
        origin: SimulationInputOriginV29::Backing(BufferBackingIdV1(backing)),
        path,
        layout: StorageLayoutIdV1(layout),
        range: None,
        access: AccessMode::ReadWrite,
    }
}

pub(super) fn relocation(
    pointer_layout: u32,
    path: Vec<SimulationObjectComponentV29>,
    referent: SimulationObjectViewV29,
) -> SimulationObjectRelocationV29 {
    SimulationObjectRelocationV29 {
        path,
        pointer_layout: StorageLayoutIdV1(pointer_layout),
        referent,
    }
}

pub(super) fn buffer(bits: u32, target: SimulationTargetV1) -> BufferArgumentV1 {
    BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        bits.to_le_bytes().to_vec(),
        vec![true; 4],
        target,
    )
    .unwrap()
}

pub(super) fn output_type() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )
}

pub(super) fn output_bits(result: &SimulationStorageExecutionV29) -> u32 {
    let SimulationStorageArgumentObservationV29::Existing(SimulationArgumentV1::Buffer(buffer)) =
        &result.arguments()[0]
    else {
        panic!("actual scalar output")
    };
    assert_eq!(buffer.initialized(), &[true; 4]);
    u32::from_le_bytes(buffer.bytes().try_into().unwrap())
}

pub(super) fn read(
    id: u32,
    source: u32,
    ty: Type,
    space: AddressSpace,
    alignment: u32,
) -> Operation {
    value(
        id,
        ty,
        OperationKind::Storage(StorageOperationV1::ReadValue {
            address: ValueId(source),
            access: MemoryAccess::new(space, alignment),
        }),
    )
}

pub(super) fn project(
    id: u32,
    base: u32,
    row: u32,
    step: StorageProjectionV1,
    space: AddressSpace,
    access: AccessMode,
) -> Operation {
    value(
        id,
        object_pointer(row, space, access),
        OperationKind::Storage(StorageOperationV1::Project {
            base: ValueId(base),
            step,
        }),
    )
}

pub(super) fn assert_violation(
    result: Result<SimulationStorageExecutionV29, SimulationErrorV1>,
    expected: &'static str,
) {
    assert!(
        matches!(result, Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
        kind: SimulationExecutionErrorKindV1::StorageViolation { reason }, ..
    })) if reason == expected),
        "expected {expected}"
    );
}

#[test]
fn image_model_preserves_padding_masks_and_rejects_only_local_shape() {
    assert_eq!(
        SimulationObjectImageV29::new(StorageLayoutIdV1(7), 3, vec![], vec![], vec![]),
        Err(SimulationObjectImageErrorV29::InvalidAlignment)
    );
    assert_eq!(
        SimulationObjectImageV29::new(StorageLayoutIdV1(7), 4, vec![0], vec![], vec![]),
        Err(SimulationObjectImageErrorV29::InitializationLength)
    );
    let image = image(
        7,
        4,
        vec![7, 0, 0, 0],
        vec![true, false, false, false],
        vec![],
    );
    assert_eq!(image.layout(), StorageLayoutIdV1(7));
    assert_eq!(image.initialized(), &[true, false, false, false]);
    // No constructor fabricates a module owner, initialized padding, or a pointer.
    assert_eq!(image.bytes(), &[7, 0, 0, 0]);
    assert!(image.relocations().is_empty());
}

#[test]
fn closed_request_views_preserve_legacy_counts_order_and_original_borrows() {
    let old = super::storage_tests_v1::request();
    let view = SimulationRequestRefV29::Legacy(&old);
    assert!(std::ptr::eq(view.legacy().unwrap(), &old));
    assert!(std::ptr::eq(view.kernel(), &old.kernel));
    assert_eq!(view.argument_count(), 1);
    assert_eq!(view.backing_count(), 0);
    assert_eq!(
        view.allocations()
            .map(crate::storage_request_view_v29::SimulationBackingRefV29::bytes)
            .collect::<Vec<_>>(),
        [4]
    );
    let current = request(vec![SimulationStorageArgumentV29::Existing(
        old.arguments[0].clone(),
    )]);
    assert!(
        SimulationRequestRefV29::Storage(&current)
            .legacy()
            .is_none()
    );
}

#[test]
fn empty_and_scalar_only_explicit_profiles_execute_actual_owner_without_legacy_replay() {
    for target in targets() {
        for scalar_argument in [false, true] {
            let owner = admit(&graph(
                vec![],
                if scalar_argument {
                    vec![Type::Scalar(ScalarType::U32)]
                } else {
                    vec![]
                },
                vec![],
            ));
            let request = request(if scalar_argument {
                vec![SimulationStorageArgumentV29::Existing(
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(ScalarType::U32, 42, target).unwrap(),
                    ),
                )]
            } else {
                vec![]
            });
            let result =
                simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
            assert_eq!(result.identity(), owner.identity());
            assert_eq!(result.invocations_executed(), 1);
            assert!(!result.grants_execution_authority());
            assert!(result.schedule_coverage().is_complete());
        }
    }
}
