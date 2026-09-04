use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate,
    Constant, F32MathFunction, FloatOperation, Function, FunctionId, IndexKind, IntrinsicKind,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, UnaryOp, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationDebugCaptureLimitsV1, SimulationDebugCollectionV1, SimulationDebugMemoryAccessV1,
    SimulationDebugRecordKindV1, SimulationDebugRecordV1, SimulationDebugSinkControlV1,
    SimulationDebugSinkV1, SimulationDebugValueV1, SimulationLimitsV1, SimulationPreflightErrorV1,
    SimulationRequestV1, SimulationScheduleRequestV1, SimulationTargetV1, UnsupportedFeatureV1,
};

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

fn value(ty: ScalarType, bits: u128) -> ScalarBitsV1 {
    ScalarBitsV1::new(ty, bits, TARGET).unwrap()
}

fn buffer(access: AccessMode, values: &[ScalarBitsV1]) -> BufferArgumentV1 {
    let alignment = u32::from(values[0].ty().bit_width().unwrap().div_ceil(8));
    BufferArgumentV1::from_scalars(access, alignment, values, TARGET).unwrap()
}

fn buffer_bits(buffer: &BufferArgumentV1) -> Vec<u128> {
    let bytes = usize::from(buffer.element().bit_width().unwrap().div_ceil(8));
    buffer
        .bytes()
        .chunks_exact(bytes)
        .map(|chunk| {
            let mut raw = [0_u8; 16];
            raw[..bytes].copy_from_slice(chunk);
            u128::from_le_bytes(raw)
        })
        .collect()
}

fn admitted(module: Module) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
    AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default()).unwrap()
}

fn domain() -> LaunchDomain {
    LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    }
}

fn effect(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn float_pipeline_module() -> Module {
    let scalar = Type::Scalar(ScalarType::F32);
    let input = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        effect(
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
        effect(
            4,
            input.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
        effect(
            5,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(3),
            },
        ),
        effect(
            6,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        effect(
            7,
            scalar.clone(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(6),
                rhs: ValueId(2),
            },
        ),
        effect(
            8,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(6),
                rhs: ValueId(6),
            },
        ),
        effect(
            9,
            scalar.clone(),
            OperationKind::Unary {
                op: UnaryOp::Negate,
                operand: ValueId(7),
            },
        ),
        effect(
            10,
            scalar,
            OperationKind::Select {
                condition: ValueId(8),
                true_value: ValueId(7),
                false_value: ValueId(9),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(5),
                value: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "float_pipeline_impl",
        Signature::new(vec![input, output, Type::Scalar(ScalarType::F32)], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::float-pipeline");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "float_pipeline",
        "float_pipeline_impl",
        domain(),
    ));
    module
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
fn float_memory_debug_and_seeded_replay_cover_partial_multi_workgroups() {
    let input = [
        value(ScalarType::F32, 0),
        value(ScalarType::F32, 0x8000_0000),
        value(ScalarType::F32, 1),
        value(ScalarType::F32, 0x3f80_0000),
        value(ScalarType::F32, 0x7fc0_0042),
    ];
    let output = [value(ScalarType::F32, 0); 5];
    let request = SimulationRequestV1::new(
        "float_pipeline",
        [5, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(AccessMode::ReadOnly, &input)),
            SimulationArgumentV1::Buffer(buffer(AccessMode::ReadWrite, &output)),
            SimulationArgumentV1::Scalar(value(ScalarType::F32, 0x3f80_0000)),
        ],
    );
    let module = admitted(float_pipeline_module());
    let mut debug = DebugRecords::default();
    let first = module
        .simulate_debugged_scheduled_with_sink(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 0x5eed,
                max_decisions: 16,
            },
            SimulationDebugCaptureLimitsV1::new(16, 256, 16, 1_024).unwrap(),
            &mut debug,
        )
        .unwrap();
    assert_eq!(first.workgroups_visited(), 2);
    assert_eq!(first.invocations_executed(), 5);
    assert_eq!(
        buffer_bits(first.buffer(1).unwrap()),
        vec![
            0x3f80_0000,
            0x3f80_0000,
            0x3f80_0000,
            0x4000_0000,
            0xffc0_0042,
        ]
    );
    assert!(debug.0.iter().any(|record| matches!(
        &record.kind,
        SimulationDebugRecordKindV1::Checkpoint {
            stack: SimulationDebugCollectionV1::Captured(frames),
            ..
        } if frames.iter().any(|frame| matches!(
            &frame.values,
            SimulationDebugCollectionV1::Captured(values)
                if values.iter().any(|binding| matches!(
                    binding.observed,
                    SimulationDebugValueV1::Scalar(scalar)
                        if scalar.ty() == ScalarType::F32
                            && scalar.bits() == 0x4000_0000
                ))
        ))
    )));
    assert_eq!(
        debug
            .0
            .iter()
            .filter(|record| matches!(
                record.kind,
                SimulationDebugRecordKindV1::Memory {
                    access: SimulationDebugMemoryAccessV1::Read,
                    ..
                }
            ))
            .count(),
        5
    );
    assert_eq!(
        debug
            .0
            .iter()
            .filter(|record| matches!(
                record.kind,
                SimulationDebugRecordKindV1::Memory {
                    access: SimulationDebugMemoryAccessV1::WriteCommitted,
                    ..
                }
            ))
            .count(),
        5
    );

    let replay = module
        .simulate_scheduled(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::Replay(first.schedule_record().unwrap()),
        )
        .unwrap();
    assert_eq!(replay.arguments(), first.arguments());
    assert_eq!(
        replay.schedule_transcript_identity(),
        first.schedule_transcript_identity()
    );
}

fn constant_for(ty: ScalarType, bits: u128) -> Constant {
    match ty {
        ScalarType::F16 => Constant::F16Bits(bits as u16),
        ScalarType::Bf16 => Constant::Bf16Bits(bits as u16),
        ScalarType::F32 => Constant::F32Bits(bits as u32),
        ScalarType::F64 => Constant::F64Bits(bits as u64),
        _ => panic!("float constant type"),
    }
}

fn constant_module(ty: ScalarType, bits: u128) -> Module {
    let scalar = Type::Scalar(ty);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        effect(
            1,
            scalar.clone(),
            OperationKind::Constant(constant_for(ty, bits)),
        ),
        effect(
            2,
            scalar,
            OperationKind::Unary {
                op: UnaryOp::Negate,
                operand: ValueId(1),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(2),
                access: MemoryAccess::new(
                    AddressSpace::Global,
                    u32::from(ty.bit_width().unwrap().div_ceil(8)),
                ),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "float_constant_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::float-constant");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "float_constant",
        "float_constant_impl",
        domain(),
    ));
    module
}

#[test]
fn every_float_constant_preserves_nan_payload_and_sign_bits() {
    for (ty, bits, expected) in [
        (ScalarType::F16, 0x7e42, 0xfe42),
        (ScalarType::Bf16, 0x7fc2, 0xffc2),
        (ScalarType::F32, 0x7fc0_0042, 0xffc0_0042),
        (
            ScalarType::F64,
            0x7ff8_0000_0000_0042,
            0xfff8_0000_0000_0042,
        ),
    ] {
        let request = SimulationRequestV1::new(
            "float_constant",
            [1, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Buffer(buffer(
                AccessMode::ReadWrite,
                &[value(ty, 0)],
            ))],
        );
        let execution = admitted(constant_module(ty, bits))
            .simulate(&request, TARGET, SimulationLimitsV1::default())
            .unwrap();
        assert_eq!(buffer_bits(execution.buffer(0).unwrap()), vec![expected]);
    }
}

fn call_module(name: &str) -> Module {
    let float = FloatOperation::from_intrinsic_id(&FunctionId::new(name)).unwrap();
    let result = float.result_type();
    let Type::Scalar(result_scalar) = result.clone() else {
        panic!("float operation scalar result")
    };
    let pointer = Type::pointer(result.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let parameters = float.parameter_types();
    let arguments = (0..parameters.len())
        .map(|index| ValueId(u32::try_from(index + 1).unwrap()))
        .collect::<Vec<_>>();
    let result_id = ValueId(u32::try_from(parameters.len() + 1).unwrap());
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(result_id, result),
        OperationKind::Call {
            callee: float.intrinsic_function_id(),
            arguments,
        },
    ));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: result_id,
            access: MemoryAccess::new(
                AddressSpace::Global,
                u32::from(result_scalar.bit_width().unwrap().div_ceil(8)),
            ),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut signature = vec![pointer];
    signature.extend(parameters.iter().cloned());
    let parameter_ids = (0..signature.len())
        .map(|index| ValueId(u32::try_from(index).unwrap()))
        .collect::<Vec<_>>();
    let capabilities = float.required_capabilities();
    let mut entry = Function::kernel_entry(
        "float_call_impl",
        Signature::new(signature, vec![]),
        parameter_ids,
        vec![block],
    );
    entry.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new("float_call", "float_call_impl", domain());
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::float-call");
    module.required_capabilities = capabilities;
    module.functions.push(entry);
    module.functions.push(float.declaration());
    module.kernels.push(kernel);
    module
}

fn run_call(name: &str, operands: &[ScalarBitsV1]) -> u128 {
    let float = FloatOperation::from_intrinsic_id(&FunctionId::new(name)).unwrap();
    let Type::Scalar(result) = float.result_type() else {
        panic!("float operation scalar result")
    };
    let mut arguments = vec![SimulationArgumentV1::Buffer(buffer(
        AccessMode::ReadWrite,
        &[value(result, 0)],
    ))];
    arguments.extend(operands.iter().copied().map(SimulationArgumentV1::Scalar));
    let request = SimulationRequestV1::new("float_call", [1, 1, 1], [1, 1, 1], arguments);
    let execution = admitted(call_module(name))
        .simulate(&request, TARGET, SimulationLimitsV1::default())
        .unwrap();
    buffer_bits(execution.buffer(0).unwrap())[0]
}

#[test]
fn fabs_f32_clears_only_the_sign_bit_for_ieee_values() {
    const FABS_F32: &str = "__fe2o3_ir_float_v1_fabs_f32";
    for (input, expected) in [
        (0x0000_0000, 0x0000_0000),
        (0x8000_0000, 0x0000_0000),
        (0x3fc0_0000, 0x3fc0_0000),
        (0xbfc0_0000, 0x3fc0_0000),
        (0x7f80_0000, 0x7f80_0000),
        (0xff80_0000, 0x7f80_0000),
        (0x7fc0_0042, 0x7fc0_0042),
        (0xffc0_0042, 0x7fc0_0042),
    ] {
        assert_eq!(
            run_call(FABS_F32, &[value(ScalarType::F32, input)]),
            expected,
            "input bits {input:#010x}"
        );
    }
}

#[test]
fn fabs_f32_rejects_wrong_operand_arity_and_type_before_execution() {
    const FABS_F32: &str = "__fe2o3_ir_float_v1_fabs_f32";
    let module = admitted(call_module(FABS_F32));
    let output =
        SimulationArgumentV1::Buffer(buffer(AccessMode::ReadWrite, &[value(ScalarType::F32, 0)]));

    let missing =
        SimulationRequestV1::new("float_call", [1, 1, 1], [1, 1, 1], vec![output.clone()]);
    assert_eq!(
        module
            .preflight(&missing, TARGET, SimulationLimitsV1::default())
            .unwrap_err(),
        SimulationPreflightErrorV1::ArgumentCount {
            expected: 2,
            actual: 1,
        }
    );

    let wrong_type = SimulationRequestV1::new(
        "float_call",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            output,
            SimulationArgumentV1::Scalar(value(ScalarType::U32, 0xbf80_0000)),
        ],
    );
    assert_eq!(
        module
            .preflight(&wrong_type, TARGET, SimulationLimitsV1::default())
            .unwrap_err(),
        SimulationPreflightErrorV1::ArgumentType {
            argument: 1,
            expected: Type::Scalar(ScalarType::F32),
        }
    );
}

#[test]
fn every_supported_canonical_float_operation_executes_as_one_scalar_operation() {
    for (name, input_ty, input, expected) in [
        (
            "__fe2o3_ir_float_v1_f16_to_f32",
            ScalarType::F16,
            0x3c00,
            0x3f80_0000,
        ),
        (
            "__fe2o3_ir_float_v1_f32_to_f16_rne",
            ScalarType::F32,
            0x3f80_0000,
            0x3c00,
        ),
        (
            "__fe2o3_ir_float_v1_bf16_to_f32",
            ScalarType::Bf16,
            0x3f80,
            0x3f80_0000,
        ),
        (
            "__fe2o3_ir_float_v1_f32_to_bf16_rne",
            ScalarType::F32,
            0x3f80_0000,
            0x3f80,
        ),
    ] {
        assert_eq!(
            run_call(name, &[value(input_ty, input)]),
            expected,
            "{name}"
        );
    }

    for (prefix, ty, one, two, three, four) in [
        ("f16", ScalarType::F16, 0x3c00, 0x4000, 0x4200, 0x4400),
        ("bf16", ScalarType::Bf16, 0x3f80, 0x4000, 0x4040, 0x4080),
    ] {
        for (op, lhs, rhs, expected) in [
            ("add", one, two, three),
            ("sub", two, one, one),
            ("mul", two, two, four),
            ("div", two, two, one),
        ] {
            let name = format!("__fe2o3_ir_float_v1_{prefix}_{op}_widened_rne");
            assert_eq!(
                run_call(&name, &[value(ty, lhs), value(ty, rhs)]),
                expected,
                "{name}"
            );
        }
    }

    assert_eq!(
        run_call(
            "__fe2o3_ir_float_v1_fma_f32",
            &[
                value(ScalarType::F32, 0x4168_0000),
                value(ScalarType::F32, 0xc168_0000),
                value(ScalarType::F32, 0x4361_0000),
            ],
        ),
        0x416c_0000
    );
    for (name, input, expected) in [
        ("__fe2o3_ir_float_v1_floor_f32", 0xbfc0_0000, 0xc000_0000),
        ("__fe2o3_ir_float_v1_ceil_f32", 0xbfc0_0000, 0xbf80_0000),
        ("__fe2o3_ir_float_v1_trunc_f32", 0xbfc0_0000, 0xbf80_0000),
        (
            "__fe2o3_ir_float_v1_roundeven_f32",
            0x4020_0000,
            0x4000_0000,
        ),
    ] {
        assert_eq!(
            run_call(name, &[value(ScalarType::F32, input)]),
            expected,
            "{name}"
        );
    }
    assert_eq!(
        run_call(
            "__fe2o3_ir_float_v1_fma_bf16x2",
            &[
                value(ScalarType::U32, 0x3f80_3f80),
                value(ScalarType::U32, 0x4000_4000),
                value(ScalarType::U32, 0x3f80_3f80),
            ],
        ),
        0x4040_4040
    );
}

#[test]
fn unavailable_float_functions_are_typed_per_function_before_execution() {
    for (name, expected) in [
        ("__fe2o3_ir_float_v1_sqrt_f32", F32MathFunction::Sqrt),
        ("__fe2o3_ir_float_v1_sin_f32", F32MathFunction::Sin),
        ("__fe2o3_ir_float_v1_cos_f32", F32MathFunction::Cos),
        ("__fe2o3_ir_float_v1_exp_f32", F32MathFunction::Exp),
        ("__fe2o3_ir_float_v1_exp2_f32", F32MathFunction::Exp2),
        ("__fe2o3_ir_float_v1_log_f32", F32MathFunction::Ln),
        ("__fe2o3_ir_float_v1_log2_f32", F32MathFunction::Log2),
        ("__fe2o3_ir_float_v1_log10_f32", F32MathFunction::Log10),
    ] {
        let request = SimulationRequestV1::new(
            "float_call",
            [1, 1, 1],
            [1, 1, 1],
            vec![
                SimulationArgumentV1::Buffer(buffer(
                    AccessMode::ReadWrite,
                    &[value(ScalarType::F32, 0)],
                )),
                SimulationArgumentV1::Scalar(value(ScalarType::F32, 0x3f80_0000)),
            ],
        );
        let error = admitted(call_module(name))
            .preflight(&request, TARGET, SimulationLimitsV1::default())
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("expected unsupported float function, found {error:?}")
        };
        assert!(
            report.findings().iter().any(|finding| {
                finding.feature == UnsupportedFeatureV1::FloatFunction(expected)
            })
        );
    }
}

fn cast_module(kind: CastKind, from: ScalarType, to: ScalarType) -> Module {
    let result = Type::Scalar(to);
    let pointer = Type::pointer(result.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        effect(
            2,
            result,
            OperationKind::Cast {
                kind,
                value: ValueId(1),
                to: Type::Scalar(to),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(2),
                access: MemoryAccess::new(
                    AddressSpace::Global,
                    u32::from(to.bit_width().unwrap().div_ceil(8)),
                ),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "float_cast_impl",
        Signature::new(vec![pointer, Type::Scalar(from)], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::float-cast");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("float_cast", "float_cast_impl", domain()));
    module
}

#[test]
fn represented_cast_kinds_reach_the_software_float_path_and_invalid_ranges_are_typed() {
    for (kind, from, input, to, expected) in [
        (
            CastKind::FloatExtend,
            ScalarType::F16,
            0x3c01,
            ScalarType::F32,
            0x3f80_2000,
        ),
        (
            CastKind::FloatTruncate,
            ScalarType::F32,
            0x3f80_1000,
            ScalarType::F16,
            0x3c00,
        ),
        (
            CastKind::IntegerToFloat,
            ScalarType::I32,
            u128::from(u32::MAX - 2),
            ScalarType::F32,
            0xc040_0000,
        ),
        (
            CastKind::FloatToInteger,
            ScalarType::F32,
            0x3ff3_3333,
            ScalarType::U32,
            1,
        ),
    ] {
        let request = SimulationRequestV1::new(
            "float_cast",
            [1, 1, 1],
            [1, 1, 1],
            vec![
                SimulationArgumentV1::Buffer(buffer(AccessMode::ReadWrite, &[value(to, 0)])),
                SimulationArgumentV1::Scalar(value(from, input)),
            ],
        );
        let execution = admitted(cast_module(kind, from, to))
            .simulate(&request, TARGET, SimulationLimitsV1::default())
            .unwrap();
        assert_eq!(buffer_bits(execution.buffer(0).unwrap()), vec![expected]);
    }

    let request = SimulationRequestV1::new(
        "float_cast",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer(
                AccessMode::ReadWrite,
                &[value(ScalarType::I32, 0)],
            )),
            SimulationArgumentV1::Scalar(value(ScalarType::F32, 0x7fc0_0000)),
        ],
    );
    let error = admitted(cast_module(
        CastKind::FloatToInteger,
        ScalarType::F32,
        ScalarType::I32,
    ))
    .simulate(&request, TARGET, SimulationLimitsV1::default())
    .unwrap_err();
    assert!(matches!(
        error,
        fe2o3_kir_sim::SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: fe2o3_kir_sim::SimulationExecutionErrorKindV1::IntegerOutOfRange,
            ..
        })
    ));
}
