use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, Constant,
    ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1, ExecutionCapabilityProvenanceV1,
    ExecutionCapabilityRequirementV1, ExecutionCapabilityRoleV1, ExecutionCapabilitySignatureV1,
    ExecutionCapabilitySourceV1, ExecutionCapabilityTypeV1, ExecutionSafetyObligationsV1,
    ExecutionTypeIdentityV1, Function, FunctionId, IndexKind, IntrinsicKind, IntrinsicOperation,
    Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain, LaunchExtent,
    MatrixElement, MatrixOperation, MemoryAccess, Module, NumericalModeV1, Operation,
    OperationKind, ScalarType, Signature, TargetCapability, TensorFragmentLayoutV1,
    TensorLayoutContractV1, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13,
    WorkgroupSize, required_execution_obligations_v1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationAdmissionErrorV1,
    SimulationArgumentV1, SimulationExecutionCapabilityFamilyV13, SimulationLimitsV1,
    SimulationPreflightErrorV1, SimulationRequestV1, SimulationTargetV1,
};

fn identity(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn emit(block: &mut BasicBlock, next: &mut u32, ty: Type, kind: OperationKind) -> ValueId {
    let value = ValueId(*next);
    *next += 1;
    block
        .operations
        .push(Operation::effect_free(ValueDef::new(value, ty), kind));
    value
}

fn index_constant(block: &mut BasicBlock, next: &mut u32, value: u64) -> ValueId {
    emit(
        block,
        next,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
    )
}

fn index_binary(
    block: &mut BasicBlock,
    next: &mut u32,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
) -> ValueId {
    emit(
        block,
        next,
        Type::INDEX,
        OperationKind::Binary { op, lhs, rhs },
    )
}

fn scalar_pointer(scalar: ScalarType, access: AccessMode) -> Type {
    Type::pointer(Type::Scalar(scalar), AddressSpace::Global, access)
}

fn execution_capability_type(
    source_type: ExecutionTypeIdentityV1,
    role: ExecutionCapabilityRoleV1,
) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance(),
        workgroup_brand: Some([14; 32]),
        epoch: Some([15; 32]),
        role,
    })
}

fn execution_capability_operation(
    result: ValueDef,
    operands: Vec<ValueId>,
    operation: ExecutionCapabilityOperationV1,
    source: u8,
) -> Operation {
    let (arguments, output) = match &operation {
        ExecutionCapabilityOperationV1::WorkgroupDerive { context, workgroup } => {
            (vec![*context], *workgroup)
        }
        ExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup,
            subgroup,
            ..
        } => (vec![*workgroup], *subgroup),
        ExecutionCapabilityOperationV1::MatrixAccess {
            subgroup,
            epoch,
            matrix,
            ..
        } => (vec![*subgroup, *epoch], *matrix),
        _ => unreachable!("matrix fixture constructs only its authority chain"),
    };
    Operation::new(
        vec![result],
        OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operands,
            signature: ExecutionCapabilitySignatureV1::new(&arguments, output).unwrap(),
            provenance: provenance(),
            workgroup_brand: Some([14; 32]),
            epoch_before: Some([15; 32]),
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [16; 32],
                operation: [source; 32],
                block: 0,
            },
            operation,
        }),
    )
}

fn matrix_capability_operations() -> Vec<Operation> {
    let context = KernelContextTypeV1::new("entry", [3; 32], [4; 32], [5; 32]);
    let workgroup = ExecutionCapabilityOperationV1::WorkgroupDerive {
        context: identity(8),
        workgroup: identity(9),
    };
    let subgroup = ExecutionCapabilityOperationV1::SubgroupDerive {
        workgroup: identity(9),
        subgroup: identity(10),
        width: 64,
    };
    let matrix = ExecutionCapabilityOperationV1::MatrixAccess {
        subgroup: identity(10),
        epoch: identity(11),
        matrix: identity(12),
        subgroup_brand: [13; 32],
        width: 64,
    };
    vec![
        Operation::kernel_context_issue(
            ValueId(4),
            context,
            KernelContextSourceIdentityV1::new([20; 32], [21; 32], [22; 32], [23; 32]),
        ),
        execution_capability_operation(
            ValueDef::new(
                ValueId(5),
                execution_capability_type(identity(9), ExecutionCapabilityRoleV1::Workgroup),
            ),
            vec![ValueId(4)],
            workgroup,
            17,
        ),
        execution_capability_operation(
            ValueDef::new(
                ValueId(6),
                execution_capability_type(
                    identity(10),
                    ExecutionCapabilityRoleV1::Subgroup { width: 64 },
                ),
            ),
            vec![ValueId(5)],
            subgroup,
            18,
        ),
        execution_capability_operation(
            ValueDef::new(
                ValueId(7),
                execution_capability_type(
                    identity(12),
                    ExecutionCapabilityRoleV1::Matrix {
                        subgroup_brand: [13; 32],
                        width: 64,
                    },
                ),
            ),
            vec![ValueId(6)],
            matrix,
            19,
        ),
    ]
}

fn bf16_matrix_module() -> Module {
    let bf16_slice = Type::slice(
        Type::Scalar(ScalarType::Bf16),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let f32_read_slice = Type::slice(
        Type::Scalar(ScalarType::F32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let f32_write_slice = Type::slice(
        Type::Scalar(ScalarType::F32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend(matrix_capability_operations());
    let mut next = 8_u32;
    let lhs_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::Bf16, AccessMode::ReadOnly),
        OperationKind::SliceData { slice: ValueId(0) },
    );
    let rhs_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::Bf16, AccessMode::ReadOnly),
        OperationKind::SliceData { slice: ValueId(1) },
    );
    let accumulator_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::F32, AccessMode::ReadOnly),
        OperationKind::SliceData { slice: ValueId(2) },
    );
    let output_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::F32, AccessMode::WriteOnly),
        OperationKind::SliceData { slice: ValueId(3) },
    );
    let lane = emit(
        &mut block,
        &mut next,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    );
    let sixteen = index_constant(&mut block, &mut next, 16);
    let four = index_constant(&mut block, &mut next, 4);
    let lane_column = index_binary(&mut block, &mut next, BinaryOp::Remainder, lane, sixteen);
    let lane_group = index_binary(&mut block, &mut next, BinaryOp::Divide, lane, sixteen);
    let component_base = index_binary(&mut block, &mut next, BinaryOp::Multiply, lane_group, four);
    let lhs_row = index_binary(
        &mut block,
        &mut next,
        BinaryOp::Multiply,
        lane_column,
        sixteen,
    );
    let mut lhs = [ValueId(0); 4];
    let mut rhs = [ValueId(0); 4];
    let mut accumulator = [ValueId(0); 4];
    let mut output_indices = [ValueId(0); 4];
    for component_index in 0..4 {
        let component = index_constant(&mut block, &mut next, component_index as u64);
        let depth = index_binary(
            &mut block,
            &mut next,
            BinaryOp::Add,
            component_base,
            component,
        );
        let lhs_index = index_binary(&mut block, &mut next, BinaryOp::Add, lhs_row, depth);
        let rhs_row = index_binary(&mut block, &mut next, BinaryOp::Multiply, depth, sixteen);
        let rhs_index = index_binary(&mut block, &mut next, BinaryOp::Add, rhs_row, lane_column);
        let output_row = index_binary(&mut block, &mut next, BinaryOp::Multiply, depth, sixteen);
        let output_index = index_binary(
            &mut block,
            &mut next,
            BinaryOp::Add,
            output_row,
            lane_column,
        );
        output_indices[component_index] = output_index;
        lhs[component_index] =
            load_scalar(&mut block, &mut next, lhs_base, lhs_index, ScalarType::Bf16);
        rhs[component_index] =
            load_scalar(&mut block, &mut next, rhs_base, rhs_index, ScalarType::Bf16);
        accumulator[component_index] = load_scalar(
            &mut block,
            &mut next,
            accumulator_base,
            output_index,
            ScalarType::F32,
        );
    }
    let results: [ValueDef; 4] = std::array::from_fn(|_| {
        let value = ValueId(next);
        next += 1;
        ValueDef::new(value, Type::Scalar(ScalarType::F32))
    });
    let matrix = MatrixOperation::multiply_accumulate(lhs, rhs, accumulator)
        .with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        );
    block.operations.push(Operation::new(
        results.clone().into_iter().collect(),
        OperationKind::Matrix(matrix),
    ));
    for (result, index) in results.into_iter().zip(output_indices) {
        let pointer = emit(
            &mut block,
            &mut next,
            scalar_pointer(ScalarType::F32, AccessMode::WriteOnly),
            OperationKind::GetElementPointer {
                base: output_base,
                offset: index,
            },
        );
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer,
                value: result.id,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: Vec::new() });

    let mut requirements = block
        .operations
        .iter()
        .flat_map(Operation::required_capabilities)
        .collect::<BTreeSet<_>>();
    requirements.insert(TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::Matrix {
            m: 16,
            n: 16,
            k: 16,
            input_type: ScalarType::Bf16,
            accumulator_type: ScalarType::F32,
        },
    ));
    requirements.insert(TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::Numerical {
            value_type: ScalarType::F32,
            mode: NumericalModeV1::StrictIeee,
        },
    ));
    let mut function = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                bf16_slice.clone(),
                bf16_slice,
                f32_read_slice,
                f32_write_slice,
            ],
            Vec::new(),
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "matrix-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = requirements.clone();
    let mut module = Module::new("v13-bf16-matrix-numerical");
    module.functions.push(function);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

#[derive(Clone, Copy, Debug)]
enum ScaledCase {
    Fp8,
    Fp4,
    MixedFp4Fp8,
}

impl ScaledCase {
    fn layout(self) -> TensorLayoutContractV1 {
        match self {
            Self::Fp8 => {
                TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64()
            }
            Self::Fp4 => {
                TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64()
            }
            Self::MixedFp4Fp8 => {
                TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64()
            }
        }
    }

    fn operation(
        self,
        lhs: [ValueId; 8],
        rhs: [ValueId; 8],
        accumulator: [ValueId; 4],
    ) -> MatrixOperation {
        let operation = match self {
            Self::Fp8 => {
                MatrixOperation::scaled_multiply_accumulate_fp8_e4m3(lhs, rhs, accumulator)
            }
            Self::Fp4 | Self::MixedFp4Fp8 => {
                MatrixOperation::scaled_multiply_accumulate_fp4_e2m1(lhs, rhs, accumulator)
            }
        };
        operation.with_declared_tensor_layout(self.layout())
    }
}

fn scaled_matrix_module(case: ScaledCase) -> Module {
    let packed_slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let f32_read_slice = Type::slice(
        Type::Scalar(ScalarType::F32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let f32_write_slice = Type::slice(
        Type::Scalar(ScalarType::F32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend(matrix_capability_operations());
    let mut next = 8_u32;
    let lhs_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::U32, AccessMode::ReadOnly),
        OperationKind::SliceData { slice: ValueId(0) },
    );
    let rhs_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::U32, AccessMode::ReadOnly),
        OperationKind::SliceData { slice: ValueId(1) },
    );
    let accumulator_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::F32, AccessMode::ReadOnly),
        OperationKind::SliceData { slice: ValueId(2) },
    );
    let output_base = emit(
        &mut block,
        &mut next,
        scalar_pointer(ScalarType::F32, AccessMode::WriteOnly),
        OperationKind::SliceData { slice: ValueId(3) },
    );
    let lane = emit(
        &mut block,
        &mut next,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    );
    let sixteen = index_constant(&mut block, &mut next, 16);
    let eight = index_constant(&mut block, &mut next, 8);
    let four = index_constant(&mut block, &mut next, 4);
    let packed_base = index_binary(&mut block, &mut next, BinaryOp::Multiply, lane, eight);
    let mut lhs = [ValueId(0); 8];
    let mut rhs = [ValueId(0); 8];
    for word in 0..8 {
        let word_value = index_constant(&mut block, &mut next, word as u64);
        let index = index_binary(
            &mut block,
            &mut next,
            BinaryOp::Add,
            packed_base,
            word_value,
        );
        lhs[word] = load_scalar(&mut block, &mut next, lhs_base, index, ScalarType::U32);
        rhs[word] = load_scalar(&mut block, &mut next, rhs_base, index, ScalarType::U32);
    }
    let lane_column = index_binary(&mut block, &mut next, BinaryOp::Remainder, lane, sixteen);
    let lane_group = index_binary(&mut block, &mut next, BinaryOp::Divide, lane, sixteen);
    let component_base = index_binary(&mut block, &mut next, BinaryOp::Multiply, lane_group, four);
    let mut accumulator = [ValueId(0); 4];
    let mut output_indices = [ValueId(0); 4];
    for component_index in 0..4 {
        let component = index_constant(&mut block, &mut next, component_index as u64);
        let row = index_binary(
            &mut block,
            &mut next,
            BinaryOp::Add,
            component_base,
            component,
        );
        let row_base = index_binary(&mut block, &mut next, BinaryOp::Multiply, row, sixteen);
        let output_index =
            index_binary(&mut block, &mut next, BinaryOp::Add, row_base, lane_column);
        output_indices[component_index] = output_index;
        accumulator[component_index] = load_scalar(
            &mut block,
            &mut next,
            accumulator_base,
            output_index,
            ScalarType::F32,
        );
    }
    let results: [ValueDef; 4] = std::array::from_fn(|_| {
        let value = ValueId(next);
        next += 1;
        ValueDef::new(value, Type::Scalar(ScalarType::F32))
    });
    block.operations.push(Operation::new(
        results.clone().into_iter().collect(),
        OperationKind::Matrix(case.operation(lhs, rhs, accumulator)),
    ));
    for (result, index) in results.into_iter().zip(output_indices) {
        let pointer = emit(
            &mut block,
            &mut next,
            scalar_pointer(ScalarType::F32, AccessMode::WriteOnly),
            OperationKind::GetElementPointer {
                base: output_base,
                offset: index,
            },
        );
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer,
                value: result.id,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    let mut requirements = block
        .operations
        .iter()
        .flat_map(Operation::required_capabilities)
        .collect::<BTreeSet<_>>();
    requirements.insert(TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::Matrix {
            m: 16,
            n: 16,
            k: 128,
            input_type: ScalarType::U8,
            accumulator_type: ScalarType::F32,
        },
    ));
    requirements.insert(TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::Numerical {
            value_type: ScalarType::F32,
            mode: NumericalModeV1::StrictIeee,
        },
    ));
    let mut function = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                packed_slice.clone(),
                packed_slice,
                f32_read_slice,
                f32_write_slice,
            ],
            Vec::new(),
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "scaled-matrix-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = requirements.clone();
    let mut module = Module::new("v13-scaled-matrix-numerical");
    module.functions.push(function);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn load_scalar(
    block: &mut BasicBlock,
    next: &mut u32,
    base: ValueId,
    index: ValueId,
    scalar: ScalarType,
) -> ValueId {
    let pointer = emit(
        block,
        next,
        scalar_pointer(scalar, AccessMode::ReadOnly),
        OperationKind::GetElementPointer {
            base,
            offset: index,
        },
    );
    emit(
        block,
        next,
        Type::Scalar(scalar),
        OperationKind::Load {
            pointer,
            access: MemoryAccess::new(
                AddressSpace::Global,
                u32::from(scalar.bit_width().unwrap() / 8),
            ),
        },
    )
}

fn bf16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let bias = 0x7fff + ((bits >> 16) & 1);
    (bits.wrapping_add(bias) >> 16) as u16
}

fn bf16_scalar(bits: u16) -> ScalarBitsV1 {
    ScalarBitsV1::new(
        ScalarType::Bf16,
        u128::from(bits),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

fn f32_scalar(value: f32) -> ScalarBitsV1 {
    ScalarBitsV1::new(
        ScalarType::F32,
        u128::from(value.to_bits()),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

fn deterministic_inputs() -> (Vec<u16>, Vec<u16>, Vec<f32>) {
    const VALUES: [f32; 9] = [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];
    let mut lhs = (0..256)
        .map(|index| bf16_bits(VALUES[(index * 5 + index / 16) % VALUES.len()]))
        .collect::<Vec<_>>();
    let mut rhs = (0..256)
        .map(|index| bf16_bits(VALUES[(index * 7 + index / 16 + 2) % VALUES.len()]))
        .collect::<Vec<_>>();
    let mut accumulator = (0..256)
        .map(|index| VALUES[(index * 3 + 1) % VALUES.len()])
        .collect::<Vec<_>>();
    accumulator[0] = 16_777_216.0;
    for depth in 0..16 {
        lhs[depth] = bf16_bits(1.0);
        rhs[depth * 16] = bf16_bits(1.0);
    }
    (lhs, rhs, accumulator)
}

fn cpu_reference(lhs: &[u16], rhs: &[u16], accumulator: &[f32]) -> Vec<f32> {
    let mut output = accumulator.to_vec();
    for row in 0..16 {
        for column in 0..16 {
            for depth in 0..16 {
                let a = f32::from_bits(u32::from(lhs[row * 16 + depth]) << 16);
                let b = f32::from_bits(u32::from(rhs[depth * 16 + column]) << 16);
                let product = std::hint::black_box(a * b);
                output[row * 16 + column] =
                    std::hint::black_box(output[row * 16 + column] + product);
            }
        }
    }
    output
}

fn low_precision_code(element: MatrixElement, index: usize) -> u8 {
    const FP4: [u8; 5] = [0xa, 0x9, 0x0, 0x1, 0x2];
    const FP8: [u8; 5] = [0xb8, 0xb0, 0x00, 0x30, 0x38];
    match element {
        MatrixElement::Fp4E2M1 => FP4[index % FP4.len()],
        MatrixElement::Fp8E4M3 => FP8[index % FP8.len()],
        MatrixElement::Bf16 | MatrixElement::F32 => unreachable!(),
    }
}

fn pack_scaled_fragment(codes: &[u8], layout: TensorFragmentLayoutV1) -> Vec<u32> {
    let mut packed = vec![0_u32; 64 * 8];
    for lane in 0..64_u16 {
        for component in 0..layout.fragment_elements {
            let coordinate = layout.logical_coordinate(lane, component).unwrap();
            let row = usize::try_from(coordinate[0]).unwrap();
            let column = usize::try_from(coordinate[1]).unwrap();
            let code = codes[row * usize::from(layout.shape[1]) + column];
            let (word, shift) = match layout.element {
                MatrixElement::Fp8E4M3 => (usize::from(component) / 4, component % 4 * 8),
                MatrixElement::Fp4E2M1 => (usize::from(component) / 8, component % 8 * 4),
                MatrixElement::Bf16 | MatrixElement::F32 => unreachable!(),
            };
            packed[usize::from(lane) * 8 + word] |= u32::from(code) << shift;
        }
    }
    packed
}

fn decode_low_precision(element: MatrixElement, bits: u8) -> f32 {
    match element {
        MatrixElement::Fp4E2M1 => {
            const MAGNITUDES: [f32; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
            let magnitude = MAGNITUDES[usize::from(bits & 7)];
            if bits & 8 == 0 { magnitude } else { -magnitude }
        }
        MatrixElement::Fp8E4M3 => {
            let sign = if bits & 0x80 == 0 { 1.0 } else { -1.0 };
            let exponent = i32::from((bits >> 3) & 0xf);
            let mantissa = u32::from(bits & 7);
            if exponent == 15 && mantissa == 7 {
                return f32::NAN;
            }
            if exponent == 0 {
                sign * (mantissa as f32) * 2.0_f32.powi(-9)
            } else {
                sign * (1.0 + (mantissa as f32) / 8.0) * 2.0_f32.powi(exponent - 7)
            }
        }
        MatrixElement::Bf16 | MatrixElement::F32 => unreachable!(),
    }
}

fn scaled_inputs(case: ScaledCase) -> (Vec<u8>, Vec<u8>, Vec<u32>, Vec<u32>) {
    let layout = case.layout();
    let lhs = (0..16 * 128)
        .map(|index| low_precision_code(layout.a.element, index * 3 + index / 128))
        .collect::<Vec<_>>();
    let rhs = (0..128 * 16)
        .map(|index| low_precision_code(layout.b.element, index * 7 + index / 16 + 1))
        .collect::<Vec<_>>();
    let lhs_packed = pack_scaled_fragment(&lhs, layout.a);
    let rhs_packed = pack_scaled_fragment(&rhs, layout.b);
    (lhs, rhs, lhs_packed, rhs_packed)
}

fn scaled_cpu_reference(case: ScaledCase, lhs: &[u8], rhs: &[u8], accumulator: &[f32]) -> Vec<f32> {
    let layout = case.layout();
    let mut output = accumulator.to_vec();
    for row in 0..16 {
        for column in 0..16 {
            for depth in 0..128 {
                let a = decode_low_precision(layout.a.element, lhs[row * 128 + depth]);
                let b = decode_low_precision(layout.b.element, rhs[depth * 16 + column]);
                let product = std::hint::black_box(a * b);
                output[row * 16 + column] =
                    std::hint::black_box(output[row * 16 + column] + product);
            }
        }
    }
    output
}

fn u32_buffer(values: &[u32]) -> BufferArgumentV1 {
    BufferArgumentV1::from_scalars(
        AccessMode::ReadOnly,
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

fn scaled_request(lhs: &[u32], rhs: &[u32], accumulator: &[f32]) -> SimulationRequestV1 {
    let output = vec![f32::NAN; 256];
    SimulationRequestV1::new(
        "scaled-matrix-kernel",
        [64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(lhs)),
            SimulationArgumentV1::Buffer(u32_buffer(rhs)),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::from_scalars(
                    AccessMode::ReadOnly,
                    4,
                    &accumulator
                        .iter()
                        .copied()
                        .map(f32_scalar)
                        .collect::<Vec<_>>(),
                    SimulationTargetV1::amdgpu_64(),
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::from_scalars(
                    AccessMode::WriteOnly,
                    4,
                    &output.into_iter().map(f32_scalar).collect::<Vec<_>>(),
                    SimulationTargetV1::amdgpu_64(),
                )
                .unwrap(),
            ),
        ],
    )
}

#[test]
fn v13_bf16_matrix_matches_full_cpu_tile_and_replays() {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(bf16_matrix_module()).unwrap();
    let identity = *canonical.identity();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let (lhs, rhs, accumulator) = deterministic_inputs();
    let output = vec![f32::NAN; 256];
    let arguments = vec![
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(
                AccessMode::ReadOnly,
                2,
                &lhs.iter().copied().map(bf16_scalar).collect::<Vec<_>>(),
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        ),
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(
                AccessMode::ReadOnly,
                2,
                &rhs.iter().copied().map(bf16_scalar).collect::<Vec<_>>(),
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        ),
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(
                AccessMode::ReadOnly,
                4,
                &accumulator
                    .iter()
                    .copied()
                    .map(f32_scalar)
                    .collect::<Vec<_>>(),
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        ),
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(
                AccessMode::WriteOnly,
                4,
                &output.into_iter().map(f32_scalar).collect::<Vec<_>>(),
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        ),
    ];
    let request = SimulationRequestV1::new("matrix-kernel", [64, 1, 1], [64, 1, 1], arguments);
    let first = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let replay = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.identity().digest(), identity.digest());
    let actual = first
        .buffer(3)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap())))
        .collect::<Vec<_>>();
    let expected = cpu_reference(&lhs, &rhs, &accumulator);
    assert_eq!(
        actual
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(actual[0].to_bits(), 16_777_216.0_f32.to_bits());
}

#[test]
fn v13_scaled_fp8_fp4_and_mixed_matrix_profiles_match_cpu_tiles() {
    for case in [ScaledCase::Fp8, ScaledCase::Fp4, ScaledCase::MixedFp4Fp8] {
        let canonical =
            VerifiedCanonicalKernelIrV13::from_module(scaled_matrix_module(case)).unwrap();
        let admitted =
            AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default())
                .unwrap();
        let (lhs, rhs, lhs_packed, rhs_packed) = scaled_inputs(case);
        let accumulator = vec![0.0_f32; 256];
        let execution = admitted
            .simulate(
                &scaled_request(&lhs_packed, &rhs_packed, &accumulator),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        let actual = execution
            .buffer(3)
            .unwrap()
            .bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        let expected = scaled_cpu_reference(case, &lhs, &rhs, &accumulator)
            .into_iter()
            .map(f32::to_bits)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "profile {case:?}");
    }
}

#[test]
fn v13_fp4_matrix_rejects_nonzero_abi_padding() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(scaled_matrix_module(ScaledCase::Fp4)).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let (_, _, mut lhs, rhs) = scaled_inputs(ScaledCase::Fp4);
    lhs[4] = 1;
    let error = admitted
        .simulate(
            &scaled_request(&lhs, &rhs, &[0.0_f32; 256]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("zero high dwords"));
}

#[test]
fn v13_matrix_scratch_has_an_exact_resident_boundary() {
    let canonical =
        VerifiedCanonicalKernelIrV13::from_module(scaled_matrix_module(ScaledCase::Fp8)).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let (_, _, lhs, rhs) = scaled_inputs(ScaledCase::Fp8);
    let request = scaled_request(&lhs, &rhs, &[0.0_f32; 256]);
    let plan = admitted
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let resident = plan.resident_bytes();

    assert!(matches!(
        admitted.preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: resident - 1,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual,
            limit,
        }) if actual == resident as u64 && limit == (resident - 1) as u64
    ));
    assert_eq!(
        admitted
            .preflight(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1 {
                    max_resident_bytes: resident,
                    ..SimulationLimitsV1::default()
                },
            )
            .unwrap()
            .resident_bytes(),
        resident
    );
}

#[test]
fn v13_matrix_layout_and_requirement_substitution_fail_closed() {
    let mut layout_mutation = bf16_matrix_module();
    let matrix = layout_mutation.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .unwrap();
    matrix.active_lanes = 32;
    assert!(VerifiedCanonicalKernelIrV13::from_module(layout_mutation).is_err());

    let mut unsupported_requirement = bf16_matrix_module();
    let unsupported = TargetCapability::Execution(ExecutionCapabilityRequirementV1::Matrix {
        m: 8,
        n: 16,
        k: 16,
        input_type: ScalarType::Bf16,
        accumulator_type: ScalarType::F32,
    });
    unsupported_requirement
        .required_capabilities
        .insert(unsupported.clone());
    unsupported_requirement.functions[0]
        .required_capabilities
        .insert(unsupported.clone());
    unsupported_requirement.kernels[0]
        .required_capabilities
        .insert(unsupported);
    let unsupported = VerifiedCanonicalKernelIrV13::from_module(unsupported_requirement).unwrap();
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v13(unsupported, SimulationLimitsV1::default()),
        Err(SimulationAdmissionErrorV1::UnsupportedExecutionCapability(
            ExecutionCapabilityRequirementV1::Matrix { m: 8, .. }
        ))
    ));
}

#[test]
fn v13_matrix_capability_receipt_is_exact_and_each_provenance_substitution_fails() {
    use SimulationExecutionCapabilityFamilyV13 as Family;

    let module = bf16_matrix_module();
    let coordinates = module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .enumerate()
        .filter_map(|(operation, definition)| match &definition.kind {
            OperationKind::ExecutionCapability(contract) => Some((
                operation,
                SimulationExecutionCapabilityFamilyV13::of(&contract.operation),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    for &(operation, family) in &coordinates {
        let mut substituted = module.clone();
        let OperationKind::ExecutionCapability(contract) =
            &mut substituted.functions[0].body.as_mut().unwrap().blocks[0].operations[operation]
                .kind
        else {
            unreachable!();
        };
        contract.provenance.kernel_binding[0] ^= 0x80;
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(substituted).is_err(),
            "{family:?} accepted a one-axis provenance substitution"
        );
    }

    let admitted = AdmittedSimulationModuleV1::admit_v13(
        VerifiedCanonicalKernelIrV13::from_module(module).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    let receipt = admitted.capability_projection_receipt_v13().unwrap();
    assert_eq!(
        receipt
            .coordinates()
            .iter()
            .filter_map(|coordinate| coordinate.execution_family())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            Family::WorkgroupDerive,
            Family::SubgroupDerive,
            Family::MatrixAccess,
        ])
    );
    assert!(receipt.definitions() != 0);
    assert!(receipt.uses() != 0);
}
