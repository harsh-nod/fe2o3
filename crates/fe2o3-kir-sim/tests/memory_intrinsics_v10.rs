use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Constant, CopyNonOverlappingContract, Function,
    Kernel, LaunchDomain, LaunchExtent, MemoryElementType, MemoryIntrinsicOperation, MemoryLayout,
    Module, Operation, OperationKind, PointerDistanceContract, PointerDistanceKind,
    PointerDistanceUnit, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV10, VolatileAccessContract, WorkgroupMemory, WorkgroupMemoryExtent,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    IndexWidthV1, PersistedSimulationScheduleArtifactV1, PersistedSimulationScheduleBindingV1,
    PersistedSimulationScheduleDocumentV1, ScalarBitsV1, SharedBufferV1, SimulationArgumentV1,
    SimulationErrorV1, SimulationExecutionErrorKindV1, SimulationLimitsV1,
    SimulationPreflightErrorV1, SimulationRequestV1, SimulationScheduleRequestV1,
    SimulationTargetV1, UnsupportedFeatureV1,
};

fn dynamic_domain_1d() -> LaunchDomain {
    LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    }
}

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn u32_buffer(values: &[u32], access: AccessMode) -> BufferArgumentV1 {
    BufferArgumentV1::from_scalars(
        access,
        4,
        &values
            .iter()
            .copied()
            .map(ScalarBitsV1::u32)
            .collect::<Vec<_>>(),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

fn i64_buffer(values: &[i64]) -> BufferArgumentV1 {
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        8,
        &values
            .iter()
            .copied()
            .map(|value| {
                ScalarBitsV1::new(
                    ScalarType::I64,
                    u128::from(value as u64),
                    SimulationTargetV1::amdgpu_64(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

fn signed_words(bytes: &[u8]) -> Vec<i64> {
    bytes
        .chunks_exact(8)
        .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

fn admit_v10(module: Module) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV10::from_module(module).unwrap();
    AdmittedSimulationModuleV1::admit_v10(canonical, SimulationLimitsV1::default()).unwrap()
}

fn scalar_buffer(
    ty: ScalarType,
    bits: u128,
    access: AccessMode,
    target: SimulationTargetV1,
) -> BufferArgumentV1 {
    let value = ScalarBitsV1::new(ty, bits, target).unwrap();
    let layout = MemoryElementType::Scalar(ty).expected_layout();
    BufferArgumentV1::from_scalars(access, layout.alignment_bytes, &[value], target).unwrap()
}

fn pointer_distance_module(kind: PointerDistanceKind, reverse: bool) -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let source = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let result_type = match kind {
        PointerDistanceKind::Signed => Type::Scalar(ScalarType::I64),
        PointerDistanceKind::Unsigned => Type::INDEX,
    };
    let output = Type::pointer(
        result_type.clone(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(2, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(4))),
        op(
            4,
            source.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: if reverse { ValueId(3) } else { ValueId(2) },
            },
        ),
        op(
            5,
            source,
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: if reverse { ValueId(2) } else { ValueId(3) },
            },
        ),
        op(
            6,
            result_type,
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
                pointer: ValueId(5),
                origin: ValueId(4),
                kind,
                unit: PointerDistanceUnit::Elements,
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: PointerDistanceContract::supported_rust(kind),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(6),
                access: fe2o3_kernel_ir::MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "distance_impl",
        Signature::new(
            vec![
                Type::pointer(element, AddressSpace::Global, AccessMode::ReadOnly),
                output,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::memory-distance-v10");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "distance",
        "distance_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn v10_pointer_distance_executes_signed_elements_and_rejects_unsigned_reverse() {
    let request = SimulationRequestV1::new(
        "distance",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[0; 5], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(i64_buffer(&[0])),
        ],
    );
    let execution = admit_v10(pointer_distance_module(PointerDistanceKind::Signed, true))
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(execution.identity().wire_version(), 10);
    assert_eq!(signed_words(execution.buffer(1).unwrap().bytes()), vec![-3]);

    let unsigned_request = SimulationRequestV1::new(
        "distance",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[0; 5], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::from_scalars(
                    AccessMode::ReadWrite,
                    8,
                    &[ScalarBitsV1::index(0, SimulationTargetV1::amdgpu_64()).unwrap()],
                    SimulationTargetV1::amdgpu_64(),
                )
                .unwrap(),
            ),
        ],
    );
    let error = admit_v10(pointer_distance_module(PointerDistanceKind::Unsigned, true))
        .simulate(
            &unsigned_request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::PointerDistanceNegativeUnsigned,
            ..
        })
    ));
}

fn cross_allocation_distance_module() -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(element, AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(op(
        2,
        Type::Scalar(ScalarType::I64),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
            pointer: ValueId(0),
            origin: ValueId(1),
            kind: PointerDistanceKind::Signed,
            unit: PointerDistanceUnit::Bytes,
            element: MemoryElementType::Scalar(ScalarType::U32),
            address_space: AddressSpace::Global,
            layout: MemoryLayout::new(4, 4),
            contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "cross_allocation_impl",
        Signature::new(vec![pointer.clone(), pointer], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::cross-allocation-distance-v10");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "cross_allocation_distance",
        "cross_allocation_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn v10_pointer_distance_fails_closed_for_distinct_logical_allocations() {
    let request = SimulationRequestV1::new(
        "cross_allocation_distance",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[1], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[1], AccessMode::ReadOnly)),
        ],
    );
    let error = admit_v10(cross_allocation_distance_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::PointerDistanceDifferentAllocation { .. },
            ..
        })
    ));
}

fn volatile_module(contract: VolatileAccessContract) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(0),
                value: ValueId(1),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract,
            }),
        ),
        op(
            2,
            scalar.clone(),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(0),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: if contract == VolatileAccessContract::external_mmio_store() {
                    VolatileAccessContract::external_mmio_load()
                } else {
                    VolatileAccessContract::rust_allocation_load()
                },
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "volatile_impl",
        Signature::new(vec![pointer, scalar], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::volatile-v10");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "volatile",
        "volatile_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn scalar_memory_module(ty: ScalarType) -> Module {
    let scalar = Type::Scalar(ty);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let layout = MemoryElementType::Scalar(ty).expected_layout();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(0),
                value: ValueId(2),
                element: MemoryElementType::Scalar(ty),
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
        ),
        op(
            4,
            scalar.clone(),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(0),
                element: MemoryElementType::Scalar(ty),
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(0),
                destination: ValueId(1),
                count: ValueId(3),
                element: MemoryElementType::Scalar(ty),
                source_address_space: AddressSpace::Global,
                destination_address_space: AddressSpace::Global,
                layout,
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "scalar_memory_impl",
        Signature::new(vec![pointer.clone(), pointer, scalar, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    let mut module = Module::new(format!("sim-tests::scalar-memory-v10::{ty:?}"));
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "scalar_memory",
        "scalar_memory_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn v10_volatile_and_copy_execute_every_scalar_layout_on_index64() {
    let target = SimulationTargetV1::amdgpu_64();
    for ty in [
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::Index,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        let value = ScalarBitsV1::new(ty, 1, target).unwrap();
        let request = SimulationRequestV1::new(
            "scalar_memory",
            [1, 1, 1],
            [1, 1, 1],
            vec![
                SimulationArgumentV1::Buffer(scalar_buffer(ty, 0, AccessMode::ReadWrite, target)),
                SimulationArgumentV1::Buffer(scalar_buffer(ty, 0, AccessMode::ReadWrite, target)),
                SimulationArgumentV1::Scalar(value),
                SimulationArgumentV1::Scalar(ScalarBitsV1::index(1, target).unwrap()),
            ],
        );
        let execution = admit_v10(scalar_memory_module(ty))
            .simulate(&request, target, SimulationLimitsV1::default())
            .unwrap_or_else(|error| panic!("{ty:?} failed: {error:?}"));
        let width =
            usize::try_from(MemoryElementType::Scalar(ty).expected_layout().size_bytes).unwrap();
        assert_eq!(
            execution.buffer(1).unwrap().bytes(),
            &value.bits().to_le_bytes()[..width],
            "{ty:?}",
        );
    }
}

#[test]
fn v10_volatile_rust_allocation_executes_and_external_mmio_fails_preflight() {
    let request = SimulationRequestV1::new(
        "volatile",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[1], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(99)),
        ],
    );
    let execution = admit_v10(volatile_module(
        VolatileAccessContract::rust_allocation_store(),
    ))
    .simulate(
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(words(execution.buffer(0).unwrap().bytes()), vec![99]);

    let error = admit_v10(volatile_module(
        VolatileAccessContract::external_mmio_store(),
    ))
    .preflight(
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("expected typed unsupported report")
    };
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.feature == UnsupportedFeatureV1::ExternalVolatileMemory)
    );
}

fn copy_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let source = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let destination = Type::pointer(scalar, AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
            source: ValueId(0),
            destination: ValueId(1),
            count: ValueId(2),
            element: MemoryElementType::Scalar(ScalarType::U32),
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Global,
            layout: MemoryLayout::new(4, 4),
            contract: CopyNonOverlappingContract::supported_rust(),
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "copy_impl",
        Signature::new(vec![source, destination, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::copy-v10");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("copy", "copy_impl", dynamic_domain_1d()));
    module
}

fn internal_address_space_module(address_space: AddressSpace) -> Module {
    assert!(matches!(
        address_space,
        AddressSpace::Private | AddressSpace::Workgroup
    ));
    let scalar = Type::Scalar(ScalarType::U32);
    let internal = Type::pointer(scalar.clone(), address_space, AccessMode::ReadWrite);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let allocation = |result| match address_space {
        AddressSpace::Private => op(
            result,
            internal.clone(),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: Some(ValueId(1)),
                address_space,
                alignment: 4,
            },
        ),
        AddressSpace::Workgroup => op(
            result,
            internal.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar.clone(),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            }),
        ),
        _ => unreachable!(),
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(1, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        allocation(2),
        allocation(3),
        op(
            4,
            scalar.clone(),
            OperationKind::Constant(Constant::U32(77)),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(2),
                value: ValueId(4),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
        ),
        op(
            5,
            scalar.clone(),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(2),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(2),
                destination: ValueId(3),
                count: ValueId(1),
                element: MemoryElementType::Scalar(ScalarType::U32),
                source_address_space: address_space,
                destination_address_space: address_space,
                layout: MemoryLayout::new(4, 4),
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
        op(
            6,
            scalar,
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(3),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(6),
                access: fe2o3_kernel_ir::MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "internal_memory_impl",
        Signature::new(vec![output], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new(format!("sim-tests::internal-memory-v10::{address_space:?}"));
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "internal_memory",
        "internal_memory_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn unsupported_source_address_space_module(source_address_space: AddressSpace) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let source = Type::pointer(scalar.clone(), source_address_space, AccessMode::ReadOnly);
    let destination = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(2, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            3,
            scalar,
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(0),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: source_address_space,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(0),
                destination: ValueId(1),
                count: ValueId(2),
                element: MemoryElementType::Scalar(ScalarType::U32),
                source_address_space,
                destination_address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "constant_source_impl",
        Signature::new(vec![source, destination], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new(format!(
        "sim-tests::unsupported-source-v10::{source_address_space:?}"
    ));
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "constant_source",
        "constant_source_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn v10_memory_intrinsics_cover_private_workgroup_and_reject_unmodeled_spaces() {
    let target = SimulationTargetV1::amdgpu_64();
    for address_space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let request = SimulationRequestV1::new(
            "internal_memory",
            [1, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Buffer(u32_buffer(
                &[0],
                AccessMode::ReadWrite,
            ))],
        );
        let execution = admit_v10(internal_address_space_module(address_space))
            .simulate(&request, target, SimulationLimitsV1::default())
            .unwrap_or_else(|error| panic!("{address_space:?} failed: {error:?}"));
        assert_eq!(words(execution.buffer(0).unwrap().bytes()), vec![77]);
    }

    let request = SimulationRequestV1::new(
        "constant_source",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[91], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[0], AccessMode::ReadWrite)),
        ],
    );
    for address_space in [AddressSpace::Constant, AddressSpace::Generic] {
        let error = admit_v10(unsupported_source_address_space_module(address_space))
            .preflight(&request, target, SimulationLimitsV1::default())
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("expected {address_space:?} rejection")
        };
        assert!(report.findings().iter().any(|finding| {
            finding.feature == UnsupportedFeatureV1::UnsupportedAddressSpace(address_space)
        }));
    }
}

#[test]
fn v10_copy_executes_exact_bytes_and_rejects_overlap_and_uninitialized_source() {
    let target = SimulationTargetV1::amdgpu_64();
    let request = SimulationRequestV1::new(
        "copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[7, 8, 9], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[0, 0, 0], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(3, target).unwrap()),
        ],
    );
    let execution = admit_v10(copy_module())
        .simulate(&request, target, SimulationLimitsV1::default())
        .unwrap();
    assert_eq!(words(execution.buffer(1).unwrap().bytes()), vec![7, 8, 9]);

    let backing = BufferBackingIdV1(17);
    let view = |offset| {
        BufferViewArgumentV1::new(
            backing,
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            offset,
            2,
            target,
        )
        .unwrap()
    };
    let overlap = SimulationRequestV1::new(
        "copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(view(0)),
            SimulationArgumentV1::BufferView(view(4)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(2, target).unwrap()),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: backing,
        buffer: u32_buffer(&[1, 2, 3], AccessMode::ReadWrite),
    }]);
    let error = admit_v10(copy_module())
        .simulate(&overlap, target, SimulationLimitsV1::default())
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::CopyRangesOverlap { bytes: 8, .. },
            ..
        })
    ));

    let mut uninitialized = u32_buffer(&[7], AccessMode::ReadOnly);
    uninitialized = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        uninitialized.bytes().to_vec(),
        vec![false; 4],
        target,
    )
    .unwrap();
    let uninitialized_request = SimulationRequestV1::new(
        "copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(uninitialized),
            SimulationArgumentV1::Buffer(u32_buffer(&[0], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(1, target).unwrap()),
        ],
    );
    let error = admit_v10(copy_module())
        .simulate(
            &uninitialized_request,
            target,
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::UninitializedRead { bytes: 4, .. },
            ..
        })
    ));
}

fn offset_copy_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let source = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let destination = Type::pointer(scalar, AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(2))),
        op(
            4,
            source.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
        op(
            5,
            destination.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(4),
                destination: ValueId(5),
                count: ValueId(2),
                element: MemoryElementType::Scalar(ScalarType::U32),
                source_address_space: AddressSpace::Global,
                destination_address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "offset_copy_impl",
        Signature::new(vec![source, destination, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::offset-copy-v10");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "offset_copy",
        "offset_copy_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn v10_zero_count_copy_checks_identity_and_alignment_but_not_range() {
    let target = SimulationTargetV1::amdgpu_64();
    let request = SimulationRequestV1::new(
        "offset_copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[7], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[11], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(0, target).unwrap()),
        ],
    );
    let execution = admit_v10(offset_copy_module())
        .simulate(&request, target, SimulationLimitsV1::default())
        .unwrap();
    assert_eq!(words(execution.buffer(1).unwrap().bytes()), vec![11]);

    let positive = SimulationRequestV1::new(
        "offset_copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[7], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[11], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(1, target).unwrap()),
        ],
    );
    let error = admit_v10(offset_copy_module())
        .simulate(&positive, target, SimulationLimitsV1::default())
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
            ..
        })
    ));
}

#[test]
fn v10_copy_work_is_bounded_by_the_global_step_limit() {
    let target = SimulationTargetV1::amdgpu_64();
    let request = SimulationRequestV1::new(
        "copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[7], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[0], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(1, target).unwrap()),
        ],
    );
    let limits = SimulationLimitsV1 {
        max_steps: 4,
        ..SimulationLimitsV1::default()
    };
    let error = admit_v10(copy_module())
        .simulate(&request, target, limits)
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { limit: 4 },
            ..
        })
    ));
}

#[test]
fn v10_copy_byte_count_uses_target_usize_and_rejects_overflow() {
    let target = SimulationTargetV1::little_endian(IndexWidthV1::Bits32);
    let request = SimulationRequestV1::new(
        "copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[7], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[0], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(u64::from(u32::MAX), target).unwrap()),
        ],
    );
    let error = admit_v10(copy_module())
        .simulate(&request, target, SimulationLimitsV1::default())
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::MemoryIntrinsicByteCountOverflow,
            ..
        })
    ));
}

#[test]
fn v10_copy_schedule_records_round_trip_with_explicit_v10_custody() {
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let request = SimulationRequestV1::new(
        "copy",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&[7], AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(u32_buffer(&[0], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(1, target).unwrap()),
        ],
    );
    let admitted = admit_v10(copy_module());
    let execution = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 16 },
        )
        .unwrap();
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV10,
        *admitted.identity(),
        [0x31; 32],
        101,
        target,
        limits,
    );
    let encoded = PersistedSimulationScheduleDocumentV1::encode_record(
        binding,
        execution.schedule_record().unwrap(),
    )
    .unwrap();
    assert!(
        std::str::from_utf8(&encoded)
            .unwrap()
            .contains("\"kind\":\"canonical_kir_v10\"")
    );
    let decoded = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&encoded).unwrap();
    assert_eq!(decoded.binding(), binding);
    assert_eq!(decoded.record(), execution.schedule_record().unwrap());
    admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(decoded.record()),
        )
        .unwrap();
}

#[test]
fn v10_index_memory_layout_fails_closed_on_index32_target() {
    let scalar = Type::Scalar(ScalarType::Index);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(op(
        1,
        scalar,
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
            pointer: ValueId(0),
            element: MemoryElementType::Scalar(ScalarType::Index),
            address_space: AddressSpace::Global,
            layout: MemoryLayout::new(8, 8),
            contract: VolatileAccessContract::rust_allocation_load(),
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "index_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::index-layout-v10");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("index", "index_impl", dynamic_domain_1d()));
    let target = SimulationTargetV1::little_endian(IndexWidthV1::Bits32);
    let request = SimulationRequestV1::new(
        "index",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(
                AccessMode::ReadOnly,
                4,
                &[ScalarBitsV1::index(1, target).unwrap()],
                target,
            )
            .unwrap(),
        )],
    );
    let error = admit_v10(module)
        .preflight(&request, target, SimulationLimitsV1::default())
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("expected unsupported layout")
    };
    assert!(
        report.findings().iter().any(|finding| {
            finding.feature == UnsupportedFeatureV1::MemoryIntrinsicTargetLayout
        })
    );
}
