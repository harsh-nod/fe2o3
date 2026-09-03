use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, BasicBlock, BinaryOp, BlockId, Constant,
    Convergence, Function, Gfx950LdsTransposeFormatV1, Gfx950LdsTransposeOperationKindV1,
    Gfx950LdsTransposeOperationV1, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MatrixElement, MatrixOperation, MemoryAccess, MemoryOrdering,
    Module, Operation, OperationKind, ScalarType, Signature, SynchronizationScope, Terminator,
    Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV9, VerifiedCanonicalKernelIrV10,
    WorkgroupBarrier, WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationDebugBarrierActionV1, SimulationDebugCaptureLimitsV1, SimulationDebugRecordKindV1,
    SimulationDebugRecordV1, SimulationDebugSinkControlV1, SimulationDebugSinkV1,
    SimulationExecutionErrorKindV1, SimulationLimitsV1, SimulationPreflightErrorV1,
    SimulationRequestV1, SimulationScheduleRequestV1, SimulationTargetV1,
};

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

#[derive(Clone, Copy)]
enum KirVersion {
    V9,
    V10,
}

fn one(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn index_constant(result: u32, value: u64) -> Operation {
    one(
        result,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
    )
}

fn finalize_module(mut module: Module) -> Module {
    let capabilities = module.functions[0].derived_capabilities();
    module.functions[0].required_capabilities = capabilities.clone();
    module.kernels[0].required_capabilities = capabilities.clone();
    module.required_capabilities = capabilities;
    module
}

fn admit(version: KirVersion, module: Module) -> AdmittedSimulationModuleV1 {
    match version {
        KirVersion::V9 => AdmittedSimulationModuleV1::admit_v9(
            VerifiedCanonicalKernelIrV9::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap(),
        KirVersion::V10 => AdmittedSimulationModuleV1::admit_v10(
            VerifiedCanonicalKernelIrV10::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap(),
    }
}

fn matrix_lds_module() -> Module {
    let f32_ty = Type::Scalar(ScalarType::F32);
    let input_ty = Type::pointer(f32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output_ty = Type::pointer(f32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let lds_ty = Type::pointer(
        f32_ty.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut operations = vec![
        one(
            2,
            lds_ty,
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: f32_ty.clone(),
                extent: WorkgroupMemoryExtent::Static(256),
                alignment: 4,
            }),
        ),
        one(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        index_constant(4, 4),
        one(
            5,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ),
        one(
            6,
            input_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(5),
            },
        ),
    ];
    for component in 0..4_u32 {
        let pointer = 10 + component;
        if component == 0 {
            operations.push(one(
                pointer,
                input_ty.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(6),
                    offset: ValueId(40),
                },
            ));
        } else {
            operations.push(index_constant(40 + component, u64::from(component)));
            operations.push(one(
                pointer,
                input_ty.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(6),
                    offset: ValueId(40 + component),
                },
            ));
        }
        operations.push(one(
            20 + component,
            f32_ty.clone(),
            OperationKind::Load {
                pointer: ValueId(pointer),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    operations.insert(5, index_constant(40, 0));
    operations.push(Operation::new(
        vec![],
        OperationKind::Matrix(MatrixOperation::lds_store(
            ValueId(2),
            [ValueId(20), ValueId(21), ValueId(22), ValueId(23)],
            MatrixElement::F32,
        )),
    ));
    operations.push(Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ));
    operations.push(Operation::new(
        (30..34)
            .map(|id| ValueDef::new(ValueId(id), f32_ty.clone()))
            .collect(),
        OperationKind::Matrix(MatrixOperation::lds_load(ValueId(2), MatrixElement::F32)),
    ));
    operations.extend([
        one(
            34,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            35,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(34),
                rhs: ValueId(4),
            },
        ),
        one(
            36,
            output_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(35),
            },
        ),
    ]);
    for component in 0..4_u32 {
        operations.push(one(
            50 + component,
            output_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(36),
                offset: ValueId(40 + component),
            },
        ));
        operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(50 + component),
                value: ValueId(30 + component),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "matrix_lds_impl",
        Signature::new(vec![input_ty, output_ty], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "matrix_lds",
        "matrix_lds_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("sim-tests::matrix-lds-v9");
    module.functions.push(entry);
    module.kernels.push(kernel);
    finalize_module(module)
}

fn lds_gemm_module(include_barrier: bool) -> Module {
    let f32_ty = Type::Scalar(ScalarType::F32);
    let input_ty = Type::pointer(f32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output_ty = Type::pointer(f32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let lds_ty = Type::pointer(
        f32_ty.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut operations = vec![
        one(
            3,
            lds_ty.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: f32_ty.clone(),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 4,
            }),
        ),
        one(
            4,
            lds_ty.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: f32_ty.clone(),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 4,
            }),
        ),
        one(
            5,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        one(
            6,
            input_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(5),
            },
        ),
        one(
            7,
            f32_ty.clone(),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        one(
            8,
            input_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(5),
            },
        ),
        one(
            9,
            f32_ty.clone(),
            OperationKind::Load {
                pointer: ValueId(8),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        one(
            10,
            lds_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: ValueId(5),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
        one(
            11,
            lds_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(5),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(11),
                value: ValueId(9),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
    ];
    if include_barrier {
        operations.push(Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ));
    }
    operations.extend([
        index_constant(12, 3),
        one(
            13,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::ShiftRight,
                lhs: ValueId(5),
                rhs: ValueId(12),
            },
        ),
        index_constant(14, 7),
        one(
            15,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(5),
                rhs: ValueId(14),
            },
        ),
        index_constant(16, 8),
        one(
            17,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(13),
                rhs: ValueId(16),
            },
        ),
        one(
            18,
            f32_ty.clone(),
            OperationKind::Constant(Constant::F32Bits(0)),
        ),
    ]);
    let mut next = 100_u32;
    let mut accumulator = ValueId(18);
    for k in 0..8_u64 {
        let k_value = ValueId(next);
        operations.push(index_constant(next, k));
        next += 1;
        let a_index = ValueId(next);
        operations.push(one(
            next,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(17),
                rhs: k_value,
            },
        ));
        next += 1;
        let a_pointer = ValueId(next);
        operations.push(one(
            next,
            lds_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: a_index,
            },
        ));
        next += 1;
        let a_value = ValueId(next);
        operations.push(one(
            next,
            f32_ty.clone(),
            OperationKind::Load {
                pointer: a_pointer,
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ));
        next += 1;
        let k_base = ValueId(next);
        operations.push(one(
            next,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: k_value,
                rhs: ValueId(16),
            },
        ));
        next += 1;
        let b_index = ValueId(next);
        operations.push(one(
            next,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: k_base,
                rhs: ValueId(15),
            },
        ));
        next += 1;
        let b_pointer = ValueId(next);
        operations.push(one(
            next,
            lds_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: b_index,
            },
        ));
        next += 1;
        let b_value = ValueId(next);
        operations.push(one(
            next,
            f32_ty.clone(),
            OperationKind::Load {
                pointer: b_pointer,
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ));
        next += 1;
        let product = ValueId(next);
        operations.push(one(
            next,
            f32_ty.clone(),
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: a_value,
                rhs: b_value,
            },
        ));
        next += 1;
        let sum = ValueId(next);
        operations.push(one(
            next,
            f32_ty.clone(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: accumulator,
                rhs: product,
            },
        ));
        next += 1;
        accumulator = sum;
    }
    operations.extend([
        one(
            next,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        one(
            next + 1,
            output_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(next),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(next + 1),
                value: accumulator,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "lds_gemm_impl",
        Signature::new(vec![input_ty.clone(), input_ty, output_ty], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "lds_gemm",
        "lds_gemm_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("sim-tests::static-lds-gemm");
    module.functions.push(entry);
    module.kernels.push(kernel);
    finalize_module(module)
}

fn transpose_module(format: Gfx950LdsTransposeFormatV1) -> Module {
    let source_ty = Type::slice(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let output_ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let storage_ty = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut operations = vec![
        one(
            10,
            storage_ty.clone(),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Current { format },
            )),
        ),
        one(
            11,
            storage_ty.clone(),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Stage {
                    format,
                    storage: ValueId(10),
                    source_slice: ValueId(0),
                    offset: ValueId(2),
                    rows: ValueId(3),
                    columns: ValueId(4),
                    stride: ValueId(5),
                    token_base: ValueId(6),
                    reduction_base: ValueId(7),
                },
            )),
        ),
        one(
            12,
            storage_ty,
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Publish {
                    format,
                    storage: ValueId(11),
                },
            )),
        ),
        Operation::new(
            (13..21)
                .map(|id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)))
                .collect(),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Read {
                    format,
                    storage: ValueId(12),
                },
            )),
        ),
        one(
            21,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        index_constant(22, 8),
        one(
            23,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(21),
                rhs: ValueId(22),
            },
        ),
        one(
            24,
            output_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(23),
            },
        ),
        index_constant(40, 0),
    ];
    for result in 0..8_u32 {
        if result != 0 {
            operations.push(index_constant(40 + result, u64::from(result)));
        }
        operations.push(one(
            50 + result,
            output_ty.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(24),
                offset: ValueId(40 + result),
            },
        ));
        operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(50 + result),
                value: ValueId(13 + result),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let parameter_types = vec![
        source_ty,
        output_ty,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
    ];
    let entry = Function::kernel_entry(
        "transpose_impl",
        Signature::new(parameter_types, vec![]),
        (0..8).map(ValueId).collect(),
        vec![block],
    );
    let mut kernel = Kernel::new(
        "transpose",
        "transpose_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("sim-tests::gfx950-transpose-v9");
    module.functions.push(entry);
    module.kernels.push(kernel);
    finalize_module(module)
}

fn scalar(ty: ScalarType, bits: u128) -> ScalarBitsV1 {
    ScalarBitsV1::new(ty, bits, TARGET).unwrap()
}

fn buffer(ty: ScalarType, access: AccessMode, values: &[u128]) -> BufferArgumentV1 {
    let values = values
        .iter()
        .map(|bits| scalar(ty, *bits))
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(access, ty.bit_width().unwrap() as u32 / 8, &values, TARGET)
        .unwrap()
}

fn index(value: u64) -> SimulationArgumentV1 {
    SimulationArgumentV1::Scalar(ScalarBitsV1::index(value, TARGET).unwrap())
}

fn output_u32(execution: &fe2o3_kir_sim::SimulationExecutionV1, ordinal: usize) -> Vec<u32> {
    execution
        .buffer(ordinal)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

#[test]
fn matrix_lds_round_trips_exact_xor4_fragments_in_v9_and_v10() {
    let input = (0..512_u32)
        .map(|value| u128::from(value.rotate_left(value & 15)))
        .collect::<Vec<_>>();
    let request = SimulationRequestV1::new(
        "matrix_lds",
        [128, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadOnly, &input)),
            SimulationArgumentV1::Buffer(buffer(
                ScalarType::F32,
                AccessMode::ReadWrite,
                &vec![0; input.len()],
            )),
        ],
    );
    for version in [KirVersion::V9, KirVersion::V10] {
        let module = admit(version, matrix_lds_module());
        let recorded = module
            .simulate_scheduled(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed: 0x0002_16c3,
                    max_decisions: 4_096,
                },
            )
            .unwrap();
        assert_eq!(
            output_u32(&recorded, 1),
            input.iter().map(|value| *value as u32).collect::<Vec<_>>()
        );
        let replayed = module
            .simulate_scheduled(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::Replay(recorded.schedule_record().unwrap()),
            )
            .unwrap();
        assert_eq!(replayed.arguments(), recorded.arguments());
    }
}

fn exact_small_u32_f32(value: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    let exponent = 31 - value.leading_zeros();
    let fraction = (value - (1 << exponent)) << (23 - exponent);
    ((127 + exponent) << 23) | fraction
}

fn lds_gemm_request() -> SimulationRequestV1 {
    let a = (0..8_u32)
        .flat_map(|row| (0..8_u32).map(move |column| u128::from(exact_small_u32_f32(row + column))))
        .collect::<Vec<_>>();
    let b = (0..8_u32)
        .flat_map(|row| {
            (0..8_u32).map(move |column| u128::from(exact_small_u32_f32((row == column) as u32)))
        })
        .collect::<Vec<_>>();
    SimulationRequestV1::new(
        "lds_gemm",
        [128, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadOnly, &a)),
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadOnly, &b)),
            SimulationArgumentV1::Buffer(buffer(
                ScalarType::F32,
                AccessMode::ReadWrite,
                &vec![0; 128],
            )),
        ],
    )
}

#[test]
fn static_lds_gemm_profile_is_exact_replayable_and_requires_publish() {
    let request = lds_gemm_request();
    let expected = (0..128_u32)
        .map(|lane| {
            let local = lane % 64;
            exact_small_u32_f32(local / 8 + local % 8)
        })
        .collect::<Vec<_>>();
    for version in [KirVersion::V9, KirVersion::V10] {
        let module = admit(version, lds_gemm_module(true));
        let recorded = module
            .simulate_scheduled(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed: 0x0000_0888,
                    max_decisions: 4_096,
                },
            )
            .unwrap();
        assert_eq!(output_u32(&recorded, 2), expected);
        let replayed = module
            .simulate_scheduled(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::Replay(recorded.schedule_record().unwrap()),
            )
            .unwrap();
        assert_eq!(replayed.arguments(), recorded.arguments());
    }

    let error = admit(KirVersion::V9, lds_gemm_module(false))
        .simulate_scheduled(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 0x0000_0888,
                max_decisions: 4_096,
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        fe2o3_kir_sim::SimulationErrorV1::Execution(error)
            if matches!(
                error.kind,
                SimulationExecutionErrorKindV1::UninitializedRead { .. }
                    | SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish { .. }
            )
    ));
}

fn source_data() -> Vec<u128> {
    (0..16_u32)
        .flat_map(|row| (0..128_u32).map(move |column| u128::from((row * 17 + column * 3) as u8)))
        .collect()
}

fn expected_transpose(format: Gfx950LdsTransposeFormatV1, source: &[u128]) -> Vec<u32> {
    let mut output = Vec::with_capacity(128 * 8);
    for global_lane in 0..128_u32 {
        let lane = global_lane % 64;
        let row = lane % 16;
        let group = lane / 16;
        match format {
            Gfx950LdsTransposeFormatV1::Fp8E4M3 => {
                for word in 0..8_u32 {
                    let mut bytes = [0_u8; 4];
                    for (within, byte) in bytes.iter_mut().enumerate() {
                        let component = word * 4 + within as u32;
                        let column = group * 16 + component % 16 + (component / 16) * 64;
                        *byte = source[(row * 128 + column) as usize] as u8;
                    }
                    output.push(u32::from_le_bytes(bytes));
                }
            }
            Gfx950LdsTransposeFormatV1::Fp4E2M1 => {
                for word in 0..4_u32 {
                    let mut packed = 0_u32;
                    for within in 0..8_u32 {
                        let component = word * 8 + within;
                        let column = group * 32 + component;
                        let nibble = source[(row * 128 + column) as usize] as u32 & 0x0f;
                        packed |= nibble << (within * 4);
                    }
                    output.push(packed);
                }
                output.extend([0; 4]);
            }
        }
    }
    output
}

fn transpose_request(source: &[u128]) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "transpose",
        [128, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(ScalarType::U8, AccessMode::ReadOnly, source)),
            SimulationArgumentV1::Buffer(buffer(
                ScalarType::U32,
                AccessMode::ReadWrite,
                &vec![0; 128 * 8],
            )),
            index(0),
            index(16),
            index(128),
            index(128),
            index(0),
            index(0),
        ],
    )
}

#[test]
fn gfx950_fp4_and_fp8_transpose_execute_exact_packed_layout_and_replay() {
    let source = source_data();
    let request = transpose_request(&source);
    for version in [KirVersion::V9, KirVersion::V10] {
        for format in [
            Gfx950LdsTransposeFormatV1::Fp4E2M1,
            Gfx950LdsTransposeFormatV1::Fp8E4M3,
        ] {
            let module = admit(version, transpose_module(format));
            let recorded = module
                .simulate_scheduled(
                    &request,
                    TARGET,
                    SimulationLimitsV1::default(),
                    SimulationScheduleRequestV1::RecordSeeded {
                        seed: 0x950,
                        max_decisions: 4_096,
                    },
                )
                .unwrap();
            assert_eq!(
                output_u32(&recorded, 1),
                expected_transpose(format, &source)
            );
            let replayed = module
                .simulate_scheduled(
                    &request,
                    TARGET,
                    SimulationLimitsV1::default(),
                    SimulationScheduleRequestV1::Replay(recorded.schedule_record().unwrap()),
                )
                .unwrap();
            assert_eq!(replayed.arguments(), recorded.arguments());
        }
    }
}

#[test]
fn transpose_overflow_guards_zero_fill_and_current_is_resource_accounted() {
    let source = source_data();
    let mut request = transpose_request(&source);
    request.arguments[2] = index(u64::MAX);
    request.arguments[6] = index(u64::MAX);
    request.arguments[7] = index(u64::MAX);
    let module = admit(
        KirVersion::V9,
        transpose_module(Gfx950LdsTransposeFormatV1::Fp8E4M3),
    );
    let execution = module
        .simulate(&request, TARGET, SimulationLimitsV1::default())
        .unwrap();
    assert_eq!(output_u32(&execution, 1), vec![0; 128 * 8]);

    let minimal_request = SimulationRequestV1::new(
        "transpose",
        [64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(ScalarType::U8, AccessMode::ReadOnly, &[0])),
            SimulationArgumentV1::Buffer(buffer(ScalarType::U32, AccessMode::ReadWrite, &[0])),
            index(0),
            index(0),
            index(0),
            index(0),
            index(0),
            index(0),
        ],
    );
    assert!(matches!(
        module.preflight(
            &minimal_request,
            TARGET,
            SimulationLimitsV1 {
                max_allocation_bytes: 2_047,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "static workgroup allocation bytes",
            actual: 2_048,
            limit: 2_047,
        })
    ));
}

#[test]
fn matrix_lds_requires_a_complete_wave64_at_execution() {
    let mut module = matrix_lds_module();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
    module = finalize_module(module);
    let input = vec![0; 128];
    let request = SimulationRequestV1::new(
        "matrix_lds",
        [32, 1, 1],
        [32, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadOnly, &input)),
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadWrite, &input)),
        ],
    );
    let error = admit(KirVersion::V9, module)
        .simulate(&request, TARGET, SimulationLimitsV1::default())
        .unwrap_err();
    assert!(matches!(
        error,
        fe2o3_kir_sim::SimulationErrorV1::Execution(error)
            if matches!(
                error.kind,
                SimulationExecutionErrorKindV1::IncompleteWave(detail)
                    if detail.active_mask == u64::from(u32::MAX)
                        && detail.required_mask == u64::MAX
            )
    ));
}

#[derive(Default)]
struct DebugRecords(Vec<SimulationDebugRecordV1>);

impl SimulationDebugSinkV1 for DebugRecords {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn transpose_debugger_exposes_lds_bytes_and_publish_epoch() {
    let source = source_data();
    let mut records = DebugRecords::default();
    admit(
        KirVersion::V9,
        transpose_module(Gfx950LdsTransposeFormatV1::Fp8E4M3),
    )
    .simulate_debugged_scheduled_with_sink(
        &transpose_request(&source),
        TARGET,
        SimulationLimitsV1::default(),
        SimulationScheduleRequestV1::RecordCanonical {
            max_decisions: 4_096,
        },
        SimulationDebugCaptureLimitsV1::new(128, 4_096, 128, 65_536).unwrap(),
        &mut records,
    )
    .unwrap();
    assert!(records.0.iter().any(|record| matches!(
        record.kind,
        SimulationDebugRecordKindV1::Memory {
            address_space: AddressSpace::Workgroup,
            ..
        }
    )));
    assert!(records.0.iter().any(|record| matches!(
        record.kind,
        SimulationDebugRecordKindV1::WorkgroupBarrier {
            action: SimulationDebugBarrierActionV1::Release,
            participants: 64,
            ..
        }
    )));
}

#[test]
fn matrix_lds_uninitialized_reads_remain_typed() {
    let mut module = matrix_lds_module();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let store = operations
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                OperationKind::Matrix(MatrixOperation {
                    kind: fe2o3_kernel_ir::MatrixOperationKind::LdsStore { .. },
                    ..
                })
            )
        })
        .unwrap();
    operations.remove(store);
    operations.remove(store);
    module = finalize_module(module);
    let input = vec![0; 256];
    let request = SimulationRequestV1::new(
        "matrix_lds",
        [64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadOnly, &input)),
            SimulationArgumentV1::Buffer(buffer(ScalarType::F32, AccessMode::ReadWrite, &input)),
        ],
    );
    let error = admit(KirVersion::V9, module)
        .simulate(&request, TARGET, SimulationLimitsV1::default())
        .unwrap_err();
    assert!(matches!(
        error,
        fe2o3_kir_sim::SimulationErrorV1::Execution(error)
            if matches!(error.kind, SimulationExecutionErrorKindV1::UninitializedRead { .. })
    ));
}
