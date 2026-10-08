//! Independent row-major oracle and inert canonical load/FP4-MFMA/store graph.
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;
pub const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
pub const CANARY: u32 = 0x7f12_3456;
fn emit(ops: &mut Vec<Operation>, next: &mut u32, ty: Type, kind: OperationKind) -> ValueId {
    // Only preceding, typed INDEX definitions in this single block participate.
    // No loads, pointers, floating arithmetic, reassociation or commutation.
    if ty == Type::INDEX
        && let OperationKind::Binary { op, lhs, rhs } = &kind
    {
        let defined_index = |id: ValueId| {
            ops.iter().any(|operation| {
                operation
                    .results
                    .iter()
                    .any(|result| result.id == id && result.ty == Type::INDEX)
            })
        };
        if defined_index(*lhs) && defined_index(*rhs) {
            if *op == BinaryOp::Add
                && ops.iter().any(|operation| {
                    matches!((&operation.kind, operation.results.as_slice()),
                    (OperationKind::Constant(Constant::Index(0)), [result])
                    if result.id == *rhs && result.ty == Type::INDEX)
                })
            {
                return *lhs;
            }
            if let Some(id) = ops.iter().find_map(|operation| {
                match (&operation.kind, operation.results.as_slice()) {
                    (
                        OperationKind::Binary {
                            op: prior,
                            lhs: a,
                            rhs: b,
                        },
                        [result],
                    ) if prior == op && a == lhs && b == rhs && result.ty == Type::INDEX => {
                        Some(result.id)
                    }
                    _ => None,
                }
            }) {
                return id;
            }
        }
    }
    let id = ValueId(*next);
    *next += 1;
    ops.push(Operation::effect_free(ValueDef::new(id, ty), kind));
    id
}
fn constant(ops: &mut Vec<Operation>, next: &mut u32, n: u64) -> ValueId {
    // This builder emits one straight-line block. Reuse only a matching typed
    // constant already emitted in that prefix, so it dominates every new use.
    if let Some(id) =
        ops.iter().find_map(
            |operation| match (&operation.kind, operation.results.as_slice()) {
                (OperationKind::Constant(Constant::Index(value)), [result])
                    if *value == n && result.ty == Type::INDEX =>
                {
                    Some(result.id)
                }
                _ => None,
            },
        )
    {
        return id;
    }
    emit(
        ops,
        next,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(n)),
    )
}
pub fn module(workgroup: u32) -> Module {
    let u = Type::Scalar(ScalarType::U32);
    let f = Type::Scalar(ScalarType::F32);
    let a = Type::pointer(u.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let c = Type::pointer(f.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let out = Type::pointer(f.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut ops = Vec::new();
    let mut next = 4;
    let gid = emit(
        &mut ops,
        &mut next,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    );
    let eight = constant(&mut ops, &mut next, 8);
    let four = constant(&mut ops, &mut next, 4);
    let two = constant(&mut ops, &mut next, 2);
    let packed_base = emit(
        &mut ops,
        &mut next,
        Type::INDEX,
        OperationKind::Binary {
            op: BinaryOp::Multiply,
            lhs: gid,
            rhs: eight,
        },
    );
    let c_base = emit(
        &mut ops,
        &mut next,
        Type::INDEX,
        OperationKind::Binary {
            op: BinaryOp::Multiply,
            lhs: gid,
            rhs: four,
        },
    );
    let out_base = emit(
        &mut ops,
        &mut next,
        Type::INDEX,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: c_base,
            rhs: two,
        },
    );
    let mut values = Vec::new();
    for (arg, count, base, ptr, ty) in [
        (0, 8, packed_base, &a, &u),
        (1, 8, packed_base, &a, &u),
        (2, 4, c_base, &c, &f),
    ] {
        let mut group = Vec::new();
        for component in 0..count {
            let offset = constant(&mut ops, &mut next, component);
            let index = emit(
                &mut ops,
                &mut next,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: base,
                    rhs: offset,
                },
            );
            let pointer = emit(
                &mut ops,
                &mut next,
                ptr.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(arg),
                    offset: index,
                },
            );
            group.push(emit(
                &mut ops,
                &mut next,
                ty.clone(),
                OperationKind::Load {
                    pointer,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ));
        }
        values.push(group);
    }
    let result_ids = std::array::from_fn::<_, 4, _>(|i| ValueId(next + i as u32));
    next += 4;
    ops.push(Operation::new(
        result_ids
            .iter()
            .map(|id| ValueDef::new(*id, f.clone()))
            .collect(),
        OperationKind::Matrix(
            MatrixOperation::scaled_multiply_accumulate_fp4_e2m1(
                values[0].clone().try_into().unwrap(),
                values[1].clone().try_into().unwrap(),
                values[2].clone().try_into().unwrap(),
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
            ),
        ),
    ));
    for (component, value) in result_ids.into_iter().enumerate() {
        let offset = constant(&mut ops, &mut next, component as u64);
        let index = emit(
            &mut ops,
            &mut next,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: out_base,
                rhs: offset,
            },
        );
        let pointer = emit(
            &mut ops,
            &mut next,
            out.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: index,
            },
        );
        ops.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer,
                value,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = ops;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
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
    let caps = entry.derived_capabilities();
    entry.required_capabilities = caps.clone();
    kernel.required_capabilities = caps.clone();
    let mut module = Module::new("inert::fp4-exact-v1");
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
/// Test-only independent numeric oracle; never calls the implementation decoder.
pub fn decode(code: u8) -> f64 {
    assert!(code < 16 && code != 8);
    let magnitude = match code & 7 {
        0 => 0.0,
        1 => 0.5,
        2 => 1.0,
        3 => 1.5,
        4 => 2.0,
        5 => 3.0,
        6 => 4.0,
        7 => 6.0,
        _ => unreachable!(),
    };
    if code & 8 == 0 { magnitude } else { -magnitude }
}
pub fn oracle(a: &[u8; 2048], b: &[u8; 2048], c: &[i64; 256]) -> [u32; 256] {
    std::array::from_fn(|n| {
        let mut sum = c[n] as f64 / 4.0;
        for k in 0..128 {
            sum += decode(a[(n / 16) * 128 + k]) * decode(b[k * 16 + n % 16]);
        }
        (sum as f32).to_bits()
    })
}
fn buffer(ty: ScalarType, access: AccessMode, words: Vec<u32>) -> SimulationArgumentV1 {
    let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            ty,
            access,
            4,
            bytes.clone(),
            vec![true; bytes.len()],
            TARGET,
        )
        .unwrap(),
    )
}
pub fn request(
    a: &[u8; 2048],
    b: &[u8; 2048],
    c: &[i64; 256],
    waves: usize,
    workgroup: u32,
) -> SimulationRequestV1 {
    let mut aa = vec![0_u32; waves * 512];
    let mut bb = aa.clone();
    let mut cc = vec![0_u32; waves * 256];
    // Invert the dense row-major operand mapping, independent of production helpers.
    for wave in 0..waves {
        for row in 0..16 {
            for k in 0..128 {
                let lane = row + 16 * (k / 32);
                let component = k % 32;
                aa[wave * 512 + lane * 8 + component / 8] |=
                    u32::from(a[row * 128 + k]) << (4 * (component % 8));
            }
        }
        for k in 0..128 {
            for col in 0..16 {
                let lane = col + 16 * (k / 32);
                let component = k % 32;
                bb[wave * 512 + lane * 8 + component / 8] |=
                    u32::from(b[k * 16 + col]) << (4 * (component % 8));
            }
        }
        for row in 0..16 {
            for col in 0..16 {
                cc[wave * 256 + (col + 16 * (row / 4)) * 4 + row % 4] =
                    ((c[row * 16 + col] as f64 / 4.0) as f32).to_bits();
            }
        }
    }
    SimulationRequestV1::new(
        "mfma",
        [(waves * 64) as u64, 1, 1],
        [workgroup, 1, 1],
        vec![
            buffer(ScalarType::U32, AccessMode::ReadOnly, aa),
            buffer(ScalarType::U32, AccessMode::ReadOnly, bb),
            buffer(ScalarType::F32, AccessMode::ReadOnly, cc),
            buffer(
                ScalarType::F32,
                AccessMode::ReadWrite,
                vec![CANARY; waves * 256 + 4],
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
