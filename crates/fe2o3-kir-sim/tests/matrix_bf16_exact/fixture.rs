//! Inert canonical graph only; never a live Rust source owner.
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;
pub const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
pub const CANARY: u32 = 0x7f12_3456;

fn one(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn index(id: u32, n: u64) -> Operation {
    one(id, Type::INDEX, OperationKind::Constant(Constant::Index(n)))
}

pub fn module(workgroup: u32) -> Module {
    let bf = Type::Scalar(ScalarType::Bf16);
    let fp = Type::Scalar(ScalarType::F32);
    let a = Type::pointer(bf.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let c = Type::pointer(fp.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let out = Type::pointer(fp.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut ops = vec![
        one(
            4,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        index(5, 4),
        index(6, 2),
        one(
            7,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
        one(
            8,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(7),
                rhs: ValueId(6),
            },
        ),
    ];
    for component in 0..4 {
        ops.push(index(10 + component, u64::from(component)));
        ops.push(one(
            14 + component,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(7),
                rhs: ValueId(10 + component),
            },
        ));
        for (arg, base, ty, pointee, alignment) in [
            (0, 20, &a, &bf, 2),
            (1, 30, &a, &bf, 2),
            (2, 40, &c, &fp, 4),
        ] {
            ops.push(one(
                base + component,
                ty.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(arg),
                    offset: ValueId(14 + component),
                },
            ));
            ops.push(one(
                base + 40 + component,
                pointee.clone(),
                OperationKind::Load {
                    pointer: ValueId(base + component),
                    access: MemoryAccess::new(AddressSpace::Global, alignment),
                },
            ));
        }
    }
    ops.push(Operation::new(
        (90..94)
            .map(|id| ValueDef::new(ValueId(id), fp.clone()))
            .collect(),
        OperationKind::Matrix(
            MatrixOperation::multiply_accumulate(
                [ValueId(60), ValueId(61), ValueId(62), ValueId(63)],
                [ValueId(70), ValueId(71), ValueId(72), ValueId(73)],
                [ValueId(80), ValueId(81), ValueId(82), ValueId(83)],
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            ),
        ),
    ));
    for component in 0..4 {
        ops.push(one(
            100 + component,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(8),
                rhs: ValueId(10 + component),
            },
        ));
        ops.push(one(
            110 + component,
            out.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: ValueId(100 + component),
            },
        ));
        ops.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(110 + component),
                value: ValueId(90 + component),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = ops;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "mfma_impl",
        Signature::new(vec![a.clone(), a, c, out], vec![]),
        (0..4).map(ValueId).collect(),
        vec![block],
    );
    let mut kernel = Kernel::new(
        "mfma",
        "mfma_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(workgroup, 1, 1));
    let mut module = Module::new("inert::bf16-exact-v1");
    let caps = entry.derived_capabilities();
    let mut entry = entry;
    entry.required_capabilities = caps.clone();
    kernel.required_capabilities = caps.clone();
    module.required_capabilities = caps;
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

pub fn admit(module: Module) -> AdmittedSimulationModuleV1 {
    AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_module(module).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap()
}

/// Independent integer-to-F32 oracle, not APFloat or a production mapping helper.
pub fn integer_bits(value: i64) -> u32 {
    if value == 0 {
        return 0;
    }
    let magnitude = value.unsigned_abs();
    assert!(magnitude < (1 << 24));
    let exponent = 63 - magnitude.leading_zeros();
    let fraction = ((magnitude - (1 << exponent)) << (23 - exponent)) as u32;
    (u32::from(value < 0) << 31) | ((exponent + 127) << 23) | fraction
}
pub fn bf16_bits(value: i64) -> u16 {
    assert!((-16..=16).contains(&value));
    let bits = integer_bits(value);
    assert_eq!(bits & 0xffff, 0);
    (bits >> 16) as u16
}
pub fn oracle(a: &[i64; 256], b: &[i64; 256], c: &[i64; 256]) -> [u32; 256] {
    let mut out = [0; 256];
    for (index, output) in out.iter_mut().enumerate() {
        let row = index / 16;
        let col = index % 16;
        let mut result = c[row * 16 + col];
        for k in 0..16 {
            result += a[row * 16 + k] * b[k * 16 + col];
        }
        *output = integer_bits(result);
    }
    out
}
fn buffer(ty: ScalarType, access: AccessMode, bytes: Vec<u8>) -> SimulationArgumentV1 {
    let initialized = vec![true; bytes.len()];
    SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            ty,
            access,
            ty.bit_width().unwrap() as u32 / 8,
            bytes,
            initialized,
            TARGET,
        )
        .unwrap(),
    )
}
pub fn request(
    a: &[i64; 256],
    b: &[i64; 256],
    c: &[i64; 256],
    waves: usize,
    workgroup: u32,
) -> SimulationRequestV1 {
    let mut aa = vec![0_u16; waves * 256];
    let mut bb = aa.clone();
    let mut cc = vec![0_u32; waves * 256];
    // Independent dense row/column input construction; no production map calls.
    for wave in 0..waves {
        for row in 0..16 {
            for col in 0..16 {
                let a_slot = wave * 256 + (row + (col / 4) * 16) * 4 + col % 4;
                let bc_slot = wave * 256 + (col + (row / 4) * 16) * 4 + row % 4;
                aa[a_slot] = bf16_bits(a[row * 16 + col]);
                bb[bc_slot] = bf16_bits(b[row * 16 + col]);
                cc[bc_slot] = integer_bits(c[row * 16 + col]);
            }
        }
    }
    let out = vec![CANARY; waves * 256 + 4];
    SimulationRequestV1::new(
        "mfma",
        [(waves * 64) as u64, 1, 1],
        [workgroup, 1, 1],
        vec![
            buffer(
                ScalarType::Bf16,
                AccessMode::ReadOnly,
                aa.into_iter().flat_map(u16::to_le_bytes).collect(),
            ),
            buffer(
                ScalarType::Bf16,
                AccessMode::ReadOnly,
                bb.into_iter().flat_map(u16::to_le_bytes).collect(),
            ),
            buffer(
                ScalarType::F32,
                AccessMode::ReadOnly,
                cc.into_iter().flat_map(u32::to_le_bytes).collect(),
            ),
            buffer(
                ScalarType::F32,
                AccessMode::ReadWrite,
                out.into_iter().flat_map(u32::to_le_bytes).collect(),
            ),
        ],
    )
}
pub fn check_output(run: &SimulationExecutionV1, expected: &[u32; 256], waves: usize) {
    for wave in 0..waves {
        check_wave_output(run, expected, wave, waves);
    }
}
pub fn check_wave_output(
    run: &SimulationExecutionV1,
    expected: &[u32; 256],
    wave: usize,
    waves: usize,
) {
    let output: Vec<u32> = run
        .buffer(3)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    assert_eq!(output.len(), waves * 256 + 4);
    assert_eq!(&output[..2], &[CANARY; 2]);
    assert_eq!(&output[output.len() - 2..], &[CANARY; 2]);
    for row in 0..16 {
        for col in 0..16 {
            let slot = 2 + wave * 256 + (col + (row / 4) * 16) * 4 + row % 4;
            assert_eq!(
                output[slot],
                expected[row * 16 + col],
                "wave{wave} ({row},{col})"
            );
        }
    }
    assert!(!run.grants_execution_authority());
}
pub fn alter(
    request: &mut SimulationRequestV1,
    argument: usize,
    offset: usize,
    bits: &[u8],
    initialized: bool,
) {
    let SimulationArgumentV1::Buffer(old) = &request.arguments[argument] else {
        panic!("buffer")
    };
    let mut bytes = old.bytes().to_vec();
    bytes[offset..offset + bits.len()].copy_from_slice(bits);
    let mut init = old.initialized().to_vec();
    init[offset..offset + bits.len()].fill(initialized);
    request.arguments[argument] = SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            old.element(),
            old.access(),
            old.alignment(),
            bytes,
            init,
            TARGET,
        )
        .unwrap(),
    );
}
