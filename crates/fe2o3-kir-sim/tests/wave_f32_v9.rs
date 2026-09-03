use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, Constant, Convergence,
    DiagnosticCode, Function, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    SynchronizationScope, TargetCapability, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV9, VerifiedCanonicalKernelIrV10, WaveF32ReductionKindV1,
    WaveOperation, WaveOperationKind, WaveWidth,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, PersistedSimulationScheduleArtifactV1,
    PersistedSimulationScheduleBindingV1, PersistedSimulationScheduleDocumentV1, ScalarBitsV1,
    SimulationArgumentV1, SimulationDebugCaptureLimitsV1, SimulationDebugCollectionV1,
    SimulationDebugRecordKindV1, SimulationDebugRecordV1, SimulationDebugSinkControlV1,
    SimulationDebugSinkV1, SimulationDebugValueV1, SimulationErrorV1,
    SimulationExecutionErrorKindV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationScheduleRequestV1, SimulationTargetV1,
};

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

#[derive(Clone, Copy, Debug)]
enum KirVersion {
    V9,
    V10,
}

fn effect(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn wave_capabilities(width: WaveWidth) -> BTreeSet<TargetCapability> {
    BTreeSet::from([
        TargetCapability::Subgroups,
        TargetCapability::SubgroupSize(width.lanes()),
        TargetCapability::WaveWidth(width),
    ])
}

fn wave_f32_module(width: WaveWidth, tile_width: u32, kind: WaveF32ReductionKindV1) -> Module {
    let scalar = Type::Scalar(ScalarType::F32);
    let input = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        effect(
            4,
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
            5,
            input.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(4),
            },
        ),
        effect(
            6,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        effect(
            7,
            scalar.clone(),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value: ValueId(6),
                    tile_width,
                    kind,
                },
                width,
            )),
        ),
        effect(
            8,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(tile_width - 1)),
        ),
        effect(
            9,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(3),
                rhs: ValueId(8),
            },
        ),
        effect(
            10,
            scalar.clone(),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::BroadcastF32 {
                    value: ValueId(6),
                    source_lane: ValueId(9),
                    tile_width,
                },
                width,
            )),
        ),
        effect(
            11,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(4),
            },
        ),
        effect(
            12,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(11),
                value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let capabilities = wave_capabilities(width);
    let mut entry = Function::kernel_entry(
        "wave_f32_impl",
        Signature::new(
            vec![input, output.clone(), output, Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    entry.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "wave_f32",
        "wave_f32_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::wave-f32-v9");
    module.required_capabilities = capabilities;
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

fn admitted(version: KirVersion, module: Module) -> AdmittedSimulationModuleV1 {
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

fn scalar(ty: ScalarType, bits: u128) -> ScalarBitsV1 {
    ScalarBitsV1::new(ty, bits, TARGET).unwrap()
}

fn f32_buffer(bits: &[u32], access: AccessMode) -> BufferArgumentV1 {
    let values = bits
        .iter()
        .map(|bits| scalar(ScalarType::F32, u128::from(*bits)))
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(access, 4, &values, TARGET).unwrap()
}

fn request(input: &[u32], source_lane: u32, grid: u64, workgroup: u32) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "wave_f32",
        [grid, 1, 1],
        [workgroup, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(f32_buffer(input, AccessMode::ReadOnly)),
            SimulationArgumentV1::Buffer(f32_buffer(&vec![0; input.len()], AccessMode::ReadWrite)),
            SimulationArgumentV1::Buffer(f32_buffer(&vec![0; input.len()], AccessMode::ReadWrite)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(source_lane)),
        ],
    )
}

fn buffer_bits(buffer: &BufferArgumentV1) -> Vec<u32> {
    buffer
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

fn exact_integer_f32(value: u32) -> u32 {
    match value {
        1 => 0x3f80_0000,
        2 => 0x4000_0000,
        3 => 0x4040_0000,
        4 => 0x4080_0000,
        5 => 0x40a0_0000,
        6 => 0x40c0_0000,
        7 => 0x40e0_0000,
        8 => 0x4100_0000,
        16 => 0x4180_0000,
        32 => 0x4200_0000,
        36 => 0x4210_0000,
        64 => 0x4280_0000,
        _ => panic!("test uses only exact listed f32 integers"),
    }
}

fn artifact(version: KirVersion) -> PersistedSimulationScheduleArtifactV1 {
    match version {
        KirVersion::V9 => PersistedSimulationScheduleArtifactV1::CanonicalKirV9,
        KirVersion::V10 => PersistedSimulationScheduleArtifactV1::CanonicalKirV10,
    }
}

#[test]
fn v9_and_v10_wave32_and_wave64_execute_multiwave_multiworkgroup_and_replay() {
    for version in [KirVersion::V9, KirVersion::V10] {
        for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
            let wave = width.lanes() as usize;
            let workgroup = width.lanes() * 2;
            let lanes = workgroup as usize * 2;
            let input = (0..lanes)
                .map(|lane| exact_integer_f32((lane % 8 + 1) as u32))
                .collect::<Vec<_>>();
            let request = request(&input, 3, lanes as u64, workgroup);
            let module = admitted(
                version,
                wave_f32_module(width, 8, WaveF32ReductionKindV1::Sum),
            );
            let execution = module
                .simulate_scheduled(
                    &request,
                    TARGET,
                    SimulationLimitsV1::default(),
                    SimulationScheduleRequestV1::RecordSeeded {
                        seed: 0x216,
                        max_decisions: 8_192,
                    },
                )
                .unwrap();
            assert_eq!(
                execution.identity().wire_version(),
                match version {
                    KirVersion::V9 => 9,
                    KirVersion::V10 => 10,
                }
            );
            assert_eq!(execution.workgroups_visited(), 2);
            assert_eq!(execution.invocations_executed(), lanes as u64);
            assert_eq!(
                buffer_bits(execution.buffer(1).unwrap()),
                vec![exact_integer_f32(36); lanes]
            );
            assert_eq!(
                buffer_bits(execution.buffer(2).unwrap()),
                vec![exact_integer_f32(4); lanes]
            );
            assert_eq!(wave * 4, lanes);

            let record = execution.schedule_record().unwrap().clone();
            let replay = module
                .simulate_scheduled(
                    &request,
                    TARGET,
                    SimulationLimitsV1::default(),
                    SimulationScheduleRequestV1::Replay(&record),
                )
                .unwrap();
            assert_eq!(replay.arguments(), execution.arguments());
            assert_eq!(
                replay.schedule_transcript_identity(),
                execution.schedule_transcript_identity()
            );

            let binding = PersistedSimulationScheduleBindingV1::new(
                artifact(version),
                *module.identity(),
                [0x21; 32],
                1,
                TARGET,
                SimulationLimitsV1::default(),
            );
            assert_eq!(
                binding.kir_wire_version(),
                execution.identity().wire_version()
            );
            let persisted = PersistedSimulationScheduleDocumentV1::new(binding, record).unwrap();
            let bytes = persisted.to_canonical_bytes().unwrap();
            let decoded =
                PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&bytes).unwrap();
            assert_eq!(decoded, persisted);
            assert_eq!(decoded.binding().artifact(), artifact(version));
        }
    }
}

#[test]
fn maximum_preserves_the_lowered_unordered_and_equal_left_operand_contract() {
    const NAN_PAYLOAD: u32 = 0xffc0_0042;
    let tile = [0x3f80_0000, NAN_PAYLOAD, 0x4000_0000, 0xbf80_0000];
    let zero_tile = [0, 0x8000_0000, 0, 0x8000_0000];
    let input = tile
        .into_iter()
        .chain(zero_tile)
        .cycle()
        .take(32)
        .collect::<Vec<_>>();
    let expected = [0x4000_0000, NAN_PAYLOAD, 0x4000_0000, 0x4000_0000]
        .into_iter()
        .chain(zero_tile)
        .cycle()
        .take(32)
        .collect::<Vec<_>>();
    for version in [KirVersion::V9, KirVersion::V10] {
        let execution = admitted(
            version,
            wave_f32_module(WaveWidth::Wave32, 4, WaveF32ReductionKindV1::Maximum),
        )
        .simulate(
            &request(&input, 1, 32, 32),
            TARGET,
            SimulationLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(buffer_bits(execution.buffer(1).unwrap()), expected);
        let broadcast = buffer_bits(execution.buffer(2).unwrap());
        for (tile_index, values) in input.chunks_exact(4).enumerate() {
            assert_eq!(
                &broadcast[tile_index * 4..tile_index * 4 + 4],
                &[values[1]; 4]
            );
        }
    }
}

#[test]
fn sum_handles_infinities_nan_classification_and_signed_zero_without_host_fp() {
    let input = [
        0x7f80_0000,
        0x3f80_0000,
        0x4000_0000,
        0x4040_0000,
        0xff80_0000,
        0xbf80_0000,
        0xc000_0000,
        0xc040_0000,
        0x7f80_0000,
        0xff80_0000,
        0x3f80_0000,
        0x4000_0000,
        0x8000_0000,
        0x8000_0000,
        0x8000_0000,
        0x8000_0000,
    ]
    .into_iter()
    .cycle()
    .take(64)
    .collect::<Vec<_>>();
    for version in [KirVersion::V9, KirVersion::V10] {
        let execution = admitted(
            version,
            wave_f32_module(WaveWidth::Wave64, 4, WaveF32ReductionKindV1::Sum),
        )
        .simulate(
            &request(&input, 0, 64, 64),
            TARGET,
            SimulationLimitsV1::default(),
        )
        .unwrap();
        let output = buffer_bits(execution.buffer(1).unwrap());
        for group in output.chunks_exact(16) {
            assert_eq!(&group[0..4], &[0x7f80_0000; 4]);
            assert_eq!(&group[4..8], &[0xff80_0000; 4]);
            assert!(
                group[8..12]
                    .iter()
                    .all(|bits| bits & 0x7f80_0000 == 0x7f80_0000 && bits & 0x007f_ffff != 0)
            );
            assert_eq!(&group[12..16], &[0x8000_0000; 4]);
        }
    }
}

#[test]
fn every_power_of_two_tile_width_has_exact_sum_and_broadcast_boundaries() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile_width in [1, 2, 4, 8, 16, 32, 64]
            .into_iter()
            .filter(|tile| *tile <= width.lanes())
        {
            let lanes = width.lanes() as usize;
            let input = vec![exact_integer_f32(1); lanes];
            let source_lane = tile_width - 1;
            let execution = admitted(
                KirVersion::V9,
                wave_f32_module(width, tile_width, WaveF32ReductionKindV1::Sum),
            )
            .simulate(
                &request(&input, source_lane, lanes as u64, width.lanes()),
                TARGET,
                SimulationLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(
                buffer_bits(execution.buffer(1).unwrap()),
                vec![exact_integer_f32(tile_width); lanes]
            );
            assert_eq!(buffer_bits(execution.buffer(2).unwrap()), input);
        }
    }
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
fn debugger_snapshots_retain_exact_wave_results() {
    let input = (0..32)
        .map(|lane| exact_integer_f32((lane % 8 + 1) as u32))
        .collect::<Vec<_>>();
    let module = admitted(
        KirVersion::V9,
        wave_f32_module(WaveWidth::Wave32, 8, WaveF32ReductionKindV1::Sum),
    );
    let mut records = DebugRecords::default();
    module
        .simulate_debugged_scheduled_with_sink(
            &request(&input, 0, 32, 32),
            TARGET,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordCanonical {
                max_decisions: 1_024,
            },
            SimulationDebugCaptureLimitsV1::new(32, 512, 64, 4_096).unwrap(),
            &mut records,
        )
        .unwrap();
    assert!(records.0.iter().any(|record| matches!(
        &record.kind,
        SimulationDebugRecordKindV1::Checkpoint {
            stack: SimulationDebugCollectionV1::Captured(frames),
            ..
        } if frames.iter().any(|frame| matches!(
            &frame.values,
            SimulationDebugCollectionV1::Captured(values)
                if values.iter().any(|binding| matches!(
                    binding.observed,
                    SimulationDebugValueV1::Scalar(value)
                        if value.ty() == ScalarType::F32
                            && value.bits() == u128::from(exact_integer_f32(36))
                ))
        ))
    )));
}

#[test]
fn malformed_wave_contracts_fail_before_admission() {
    for tile_width in [0, 3, 128] {
        let mut module = wave_f32_module(WaveWidth::Wave64, 4, WaveF32ReductionKindV1::Sum);
        let OperationKind::Wave(wave) =
            &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
        else {
            unreachable!()
        };
        let WaveOperationKind::ReduceF32 {
            tile_width: actual, ..
        } = &mut wave.kind
        else {
            unreachable!()
        };
        *actual = tile_width;
        let error = VerifiedCanonicalKernelIrV9::from_module(module).unwrap_err();
        assert!(
            matches!(error, fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV9::Verification(errors)
            if errors.contains(DiagnosticCode::InvalidWaveOperation))
        );
    }

    let mut inactive = wave_f32_module(WaveWidth::Wave32, 4, WaveF32ReductionKindV1::Sum);
    let OperationKind::Wave(wave) =
        &mut inactive.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    wave.active_lanes = 31;
    assert!(VerifiedCanonicalKernelIrV9::from_module(inactive).is_err());

    let mut nonuniform = wave_f32_module(WaveWidth::Wave32, 4, WaveF32ReductionKindV1::Sum);
    let OperationKind::Wave(wave) =
        &mut nonuniform.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    wave.convergence = Convergence::uniform(SynchronizationScope::Workgroup);
    assert!(VerifiedCanonicalKernelIrV9::from_module(nonuniform).is_err());

    let mut unbounded = wave_f32_module(WaveWidth::Wave32, 4, WaveF32ReductionKindV1::Sum);
    let OperationKind::Wave(wave) =
        &mut unbounded.functions[0].body.as_mut().unwrap().blocks[0].operations[6].kind
    else {
        unreachable!()
    };
    let WaveOperationKind::BroadcastF32 { source_lane, .. } = &mut wave.kind else {
        unreachable!()
    };
    *source_lane = ValueId(3);
    assert!(VerifiedCanonicalKernelIrV9::from_module(unbounded).is_err());
}

#[test]
fn partial_waves_and_step_limits_remain_precise_typed_failures() {
    let module = admitted(
        KirVersion::V9,
        wave_f32_module(WaveWidth::Wave32, 4, WaveF32ReductionKindV1::Sum),
    );
    let input = vec![exact_integer_f32(1); 31];
    let error = module
        .simulate(
            &request(&input, 0, 31, 32),
            TARGET,
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    let SimulationErrorV1::Execution(error) = error else {
        panic!("partial wave is a dynamic execution failure")
    };
    let SimulationExecutionErrorKindV1::IncompleteWave(detail) = error.kind else {
        panic!("partial wave carries exact mask evidence")
    };
    assert_eq!(detail.width, WaveWidth::Wave32);
    assert_eq!(detail.active_mask, (1_u64 << 31) - 1);
    assert_eq!(detail.required_mask, u64::from(u32::MAX));

    let limits = SimulationLimitsV1 {
        max_steps: 1,
        ..SimulationLimitsV1::default()
    };
    let input = vec![exact_integer_f32(1); 32];
    let error = module
        .simulate(&request(&input, 0, 32, 32), TARGET, limits)
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { .. },
            ..
        })
    ));
}

#[test]
fn persisted_artifact_kind_must_match_the_admitted_wire_identity() {
    let module = admitted(
        KirVersion::V9,
        wave_f32_module(WaveWidth::Wave32, 4, WaveF32ReductionKindV1::Sum),
    );
    let input = vec![exact_integer_f32(1); 32];
    let request = request(&input, 0, 32, 32);
    let execution = module
        .simulate_scheduled(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordCanonical {
                max_decisions: 1_024,
            },
        )
        .unwrap();
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV10,
        *module.identity(),
        [1; 32],
        1,
        TARGET,
        SimulationLimitsV1::default(),
    );
    assert!(
        PersistedSimulationScheduleDocumentV1::new(
            binding,
            execution.schedule_record().unwrap().clone()
        )
        .is_err()
    );

    let bundle_binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::SimulationBundleV1 {
            bundle_sha256: [2; 32],
            subject_sha256: [3; 32],
        },
        *module.identity(),
        [1; 32],
        1,
        TARGET,
        SimulationLimitsV1::default(),
    );
    assert!(
        PersistedSimulationScheduleDocumentV1::new(
            bundle_binding,
            execution.schedule_record().unwrap().clone(),
        )
        .is_err()
    );
}
