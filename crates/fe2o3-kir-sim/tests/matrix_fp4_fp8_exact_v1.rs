//! Inert canonical graphs only: not Rust source or hardware qualification.
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;
use fixture::*;
fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 1024 * 1024,
        max_reachable_functions: 4,
        max_reachable_operations: 4096,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 2_000_000,
        max_call_depth: 4,
        max_ssa_values: 256,
        max_allocations: 8,
        max_allocation_bytes: 4096,
        max_total_bytes: 16_384,
        max_resident_bytes: 16 * 1024 * 1024,
        max_events: 100_000,
        max_memory_access_records: 8192,
    }
}
fn kind(error: SimulationErrorV1) -> SimulationExecutionErrorKindV1 {
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected execution refusal, got {error:?}")
    };
    error.kind
}
fn dense() -> ([u8; 2048], [u8; 2048], [i64; 256]) {
    let codes = [
        0, 0x28, 0x30, 0x34, 0x38, 0x3a, 0x3c, 0x40, 0x44, 0x48, 0x50, 0x58, 0xa8, 0xb8, 0xd8,
    ];
    (
        std::array::from_fn(|n| {
            [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15][(n * 7 + n / 128) % 15]
        }),
        std::array::from_fn(|n| codes[(n * 11 + n / 16) % 15]),
        std::array::from_fn(|n| (n as i64 - 128) * 17 + 1),
    )
}
#[test]
fn dense_fractional_signed_operands_execute_all_outputs_and_canaries() {
    let (a, b, c) = dense();
    let request = request(&a, &b, &c, 1, 64);
    let original = request.arguments.clone();
    let run = admit(module(64))
        .simulate(&request, TARGET, limits())
        .unwrap();
    check_output(&run, &oracle(&a, &b, &c), 1);
    assert_eq!(request.arguments, original);
    for argument in 0..3 {
        let SimulationArgumentV1::Buffer(input) = &request.arguments[argument] else {
            unreachable!()
        };
        assert_eq!(run.buffer(argument).unwrap().bytes(), input.bytes());
    }
}
#[test]
fn every_reduction_coordinate_and_all_row_column_lanes_have_independent_impulses() {
    let sim = admit(module(64));
    // All 128 k positions, including every byte/dword/split-K/lane-group boundary.
    // Distinct row/column markers cover all 256 outputs, not a scalar checksum.
    for k in 0..128 {
        let mut a = [0; 2048];
        let mut b = [0; 2048];
        let codes = [
            0x28, 0x30, 0x34, 0x38, 0x3a, 0x3c, 0x40, 0x44, 0x48, 0x50, 0x58, 0xa8, 0xb8, 0xd8,
            0x28, 0x34,
        ];
        for row in 0..16 {
            a[row * 128 + k] = [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 1, 3][row];
        }
        for col in 0..16 {
            b[k * 16 + col] = codes[15 - col];
        }
        let run = sim
            .simulate(&request(&a, &b, &[0; 256], 1, 64), TARGET, limits())
            .unwrap();
        check_output(&run, &oracle(&a, &b, &[0; 256]), 1);
    }
}
#[test]
fn exact_extremes_sixteenths_and_cancellation_remain_exact() {
    let sim = admit(module(64));
    for (a, b, c) in [
        ([7; 2048], [0x58; 2048], [4_194_304; 256]),
        ([15; 2048], [0x58; 2048], [-4_194_304; 256]),
        ([1; 2048], [0x28; 2048], [-256; 256]),
        ([0; 2048], [0xd8; 2048], [1; 256]),
    ] {
        let run = sim
            .simulate(&request(&a, &b, &c, 1, 64), TARGET, limits())
            .unwrap();
        check_output(&run, &oracle(&a, &b, &c), 1);
    }
}
#[test]
fn two_waves_seeded_and_replay_keep_distinct_operands_and_outputs() {
    let (a, b, c) = dense();
    for workgroup in [64, 128] {
        let sim = admit(module(workgroup));
        let mut request = request(&a, &b, &c, 2, workgroup);
        let SimulationArgumentV1::Buffer(old) = &request.arguments[0] else {
            unreachable!()
        };
        let mut bytes = old.bytes().to_vec();
        for lane in 0..64 {
            for word in 0..4 {
                let offset = (512 + lane * 8 + word) * 4;
                let mut bits = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                for nibble in 0..8 {
                    if (bits >> (4 * nibble)) & 7 != 0 {
                        bits ^= 8 << (4 * nibble);
                    }
                }
                bytes[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
            }
        }
        request.arguments[0] = SimulationArgumentV1::Buffer(
            BufferArgumentV1::new(
                old.element(),
                old.access(),
                old.alignment(),
                bytes,
                old.initialized().to_vec(),
                TARGET,
            )
            .unwrap(),
        );
        let expected = [
            oracle(&a, &b, &c),
            oracle(&a.map(|code| if code == 0 { 0 } else { code ^ 8 }), &b, &c),
        ];
        let canonical = sim
            .simulate(&request, TARGET, limits())
            .unwrap_or_else(|error| panic!("workgroup {workgroup}: {error:?}"));
        let seeded = sim
            .simulate_scheduled(
                &request,
                TARGET,
                limits(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed: 0xf008_1234,
                    max_decisions: 256,
                },
            )
            .unwrap();
        assert_eq!(seeded.schedule_record().unwrap().decisions().len(), 256);
        let one_short = sim
            .simulate_scheduled(
                &request,
                TARGET,
                limits(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed: 0xf008_1234,
                    max_decisions: 255,
                },
            )
            .unwrap_err();
        assert!(matches!(
            kind(one_short),
            SimulationExecutionErrorKindV1::ScheduleDecisionLimit {
                actual: 256,
                limit: 255
            }
        ));
        let replay = sim
            .simulate_scheduled(
                &request,
                TARGET,
                limits(),
                SimulationScheduleRequestV1::Replay(seeded.schedule_record().unwrap()),
            )
            .unwrap();
        for run in [&canonical, &seeded, &replay] {
            for (wave, expected) in expected.iter().enumerate() {
                check_wave_output(run, expected, wave, 2);
            }
        }
        assert_eq!(canonical.arguments(), seeded.arguments());
        assert_eq!(seeded.arguments(), replay.arguments());
        assert!(replay.schedule_record().is_none());
    }
}
#[derive(Default)]
struct MatrixEvents {
    operation: u32,
    arrived: usize,
    completed: usize,
    writes: usize,
}
impl SimulationEventSinkV1 for MatrixEvents {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        if event.site.operation == Some(self.operation)
            && matches!(event.kind, SimulationEventKindV1::OperationBegin)
        {
            self.arrived += 1;
        }
        if event.site.operation == Some(self.operation)
            && matches!(
                event.kind,
                SimulationEventKindV1::OperationEnd {
                    outcome: SimulationExecutionOutcomeV1::Completed
                }
            )
        {
            self.completed += 1;
        }
        if matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }) {
            self.writes += 1;
        }
        Ok(())
    }
}
fn matrix_position(module: &Module) -> usize {
    module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .position(|operation| matches!(operation.kind, OperationKind::Matrix(_)))
        .unwrap()
}

#[test]
fn every_input_role_refuses_late_bad_values_before_any_completion_or_store() {
    let graph = module(64);
    let position = matrix_position(&graph) as u32;
    let sim = admit(graph);
    for argument in 0..3 {
        let role = [
            MatrixInputRoleV1::A,
            MatrixInputRoleV1::B,
            MatrixInputRoleV1::Accumulator,
        ][argument];
        let cases = if argument == 2 {
            vec![
                (255, 3, 0x8000_0000),
                (255, 3, 0x3d00_0000),
                (255, 3, 0x4880_0001),
                (255, 3, 0x7fc0_0000),
                (255, 3, 0x7f80_0000),
            ]
        } else if argument == 0 {
            let mut cases = Vec::new();
            for word in 0..4 {
                cases.push((63 * 8 + word, (word * 8 + 7) as u8, 0x8111_1111));
            }
            for word in 4..8 {
                cases.push((63 * 8 + word, (28 + word) as u8, 1));
            }
            cases
        } else {
            let mut cases = Vec::new();
            for word in 0..8 {
                for bad in [0x80_u32, 0x01, 0x24, 0x59, 0x7f, 0xff] {
                    cases.push((
                        63 * 8 + word,
                        (word * 4 + 3) as u8,
                        0x0028_2828 | (bad << 24),
                    ));
                }
            }
            cases
        };
        for (word, component, bits) in cases {
            let mut request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
            alter(
                &mut request,
                argument,
                word * 4,
                &u32::to_le_bytes(bits),
                true,
            );
            let mut events = MatrixEvents {
                operation: position,
                ..Default::default()
            };
            let err = sim
                .simulate_observed_with_sink(&request, TARGET, limits(), &mut events)
                .unwrap_err();
            assert_eq!(
                kind(err),
                SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
                    role,
                    lane: 63,
                    component
                }
            );
            assert_eq!(events.completed, 0);
            assert_eq!(events.writes, 0);
        }
    }
}
#[test]
fn ordinary_memory_initialization_and_bounds_checks_still_run() {
    let sim = admit(module(64));
    let mut request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
    alter(&mut request, 0, 0, &0x1111_1111_u32.to_le_bytes(), false);
    assert!(matches!(
        kind(sim.simulate(&request, TARGET, limits()).unwrap_err()),
        SimulationExecutionErrorKindV1::UninitializedRead { .. }
    ));
    let mut short = fixture::request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
    let SimulationArgumentV1::Buffer(old) = &short.arguments[1] else {
        unreachable!()
    };
    short.arguments[1] = SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            old.element(),
            old.access(),
            old.alignment(),
            old.bytes()[..2044].to_vec(),
            old.initialized()[..2044].to_vec(),
            TARGET,
        )
        .unwrap(),
    );
    assert!(matches!(
        kind(sim.simulate(&short, TARGET, limits()).unwrap_err()),
        SimulationExecutionErrorKindV1::OutOfBounds { .. }
    ));
}
#[test]
fn partial_waves_are_not_zero_padded() {
    let sim = admit(module(64));
    for grid in [63, 65] {
        let mut request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 2, 64);
        request.grid = GridShapeV1([grid, 1, 1]);
        assert!(matches!(
            kind(sim.simulate(&request, TARGET, limits()).unwrap_err()),
            SimulationExecutionErrorKindV1::IncompleteWave(_)
        ));
    }
}
#[test]
fn numerical_work_is_prepaid_before_any_result_completion() {
    let graph = module(64);
    let position = matrix_position(&graph) as u32;
    let sim = admit(graph);
    let request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
    let before = 64 * (u64::from(position) + 1 + 40);
    let denied = SimulationLimitsV1 {
        max_steps: before + 66 * 64 + 333_952 - 1,
        ..limits()
    };
    let mut events = MatrixEvents {
        operation: position,
        ..Default::default()
    };
    assert!(matches!(
        kind(
            sim.simulate_observed_with_sink(&request, TARGET, denied, &mut events)
                .unwrap_err()
        ),
        SimulationExecutionErrorKindV1::StepLimit { .. }
    ));
    assert_eq!(events.arrived, 64);
    assert_eq!(events.completed, 0);
    assert_eq!(events.writes, 0);
}
fn branched(mismatched: bool) -> Module {
    let mut graph = module(64);
    let position = matrix_position(&graph);
    let body = graph.functions[0].body.as_mut().unwrap();
    let tail = body.blocks[0].operations.split_off(position);
    body.blocks[0].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2000), Type::INDEX),
            OperationKind::Constant(Constant::Index(32)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2001), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(4),
                rhs: ValueId(2000),
            },
        ),
    ]);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2001),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    let mut else_block = BasicBlock::new(BlockId(2));
    if mismatched {
        let mut other = tail[0].clone();
        for result in &mut other.results {
            result.id = ValueId(result.id.0 + 200);
        }
        else_block.operations.push(other);
    }
    then_block.operations = tail;
    then_block.terminator = Some(Terminator::Return { values: vec![] });
    else_block.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([then_block, else_block]);
    let caps = graph.functions[0].derived_capabilities();
    graph.functions[0].required_capabilities = caps.clone();
    graph.kernels[0].required_capabilities = caps.clone();
    graph.required_capabilities = caps;
    graph
}
#[test]
fn divergent_or_different_actual_matrix_site_cannot_complete() {
    let request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
    for mismatched in [false, true] {
        let refusal = kind(
            admit(branched(mismatched))
                .simulate(&request, TARGET, limits())
                .unwrap_err(),
        );
        if mismatched {
            assert!(matches!(
                refusal,
                SimulationExecutionErrorKindV1::MismatchedWave(_)
            ));
        } else {
            assert!(matches!(
                refusal,
                SimulationExecutionErrorKindV1::DivergentWave(_)
            ));
        }
    }
}

#[test]
fn exact_steps_and_resident_floor_are_enforced_without_cap_increases() {
    let sim = admit(module(64));
    let request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
    let baseline = sim.simulate(&request, TARGET, limits()).unwrap();
    let peak = sim
        .preflight(&request, TARGET, limits())
        .unwrap()
        .resident_bytes();
    let exact = SimulationLimitsV1 {
        max_steps: baseline.steps_executed(),
        max_resident_bytes: peak,
        ..limits()
    };
    assert!(sim.simulate(&request, TARGET, exact).is_ok());
    let short_work = SimulationLimitsV1 {
        max_steps: exact.max_steps - 1,
        ..exact
    };
    assert!(matches!(
        kind(sim.simulate(&request, TARGET, short_work).unwrap_err()),
        SimulationExecutionErrorKindV1::StepLimit { .. }
    ));
    let short_storage = SimulationLimitsV1 {
        max_resident_bytes: peak - 1,
        ..exact
    };
    assert!(matches!(
        sim.preflight(&request, TARGET, short_storage),
        Err(SimulationPreflightErrorV1::ResourceLimit { .. })
    ));
}

#[test]
fn layoutless_matrix_is_refused_before_simulator_admission() {
    let mut graph = module(64);
    let position = matrix_position(&graph);
    let OperationKind::Matrix(matrix) =
        &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations[position].kind
    else {
        unreachable!()
    };
    matrix.tensor_layout = None;
    // Existing semantic verification rejects this before simulator admission.
    // Do not bypass that stronger invariant just to reach numerical preflight.
    let error = VerifiedCanonicalKernelIrV12::from_module(graph).unwrap_err();
    let VerifiedCanonicalKernelIrErrorV12::Verification(errors) = error else {
        panic!("expected missing-layout semantic verification refusal");
    };
    assert_eq!(errors.diagnostics().len(), 1);
    let diagnostic = &errors.diagnostics()[0];
    assert_eq!(diagnostic.code, DiagnosticCode::InvalidSemanticOperation);
    assert_eq!(diagnostic.location.operation, Some(position));
    assert_eq!(
        diagnostic.message,
        "scaled matrix multiply requires an explicit tensor layout contract"
    );
}

#[test]
fn exact_profile_does_not_admit_a_32_bit_target_layout() {
    let sim = admit(module(64));
    let request = request(&[1; 2048], &[0x28; 2048], &[0; 256], 1, 64);
    assert!(sim.preflight(&request, TARGET, limits()).is_ok());
    assert!(
        sim.preflight(
            &request,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            limits()
        )
        .is_err()
    );
}

#[test]
fn reversed_operand_layout_and_wrong_profile_are_not_silently_reinterpreted() {
    for reverse in [false, true] {
        let mut graph = module(64);
        let position = matrix_position(&graph);
        let OperationKind::Matrix(matrix) =
            &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations[position].kind
        else {
            unreachable!()
        };
        if reverse {
            let layout = matrix.tensor_layout.as_mut().unwrap();
            std::mem::swap(&mut layout.a, &mut layout.b);
        } else {
            let MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } = &mut matrix.kind
            else {
                unreachable!()
            };
            *profile = MatrixMultiplyProfile::fp8_e4m3_f32_m16n16k128_wave64();
        }
        // Both are invalid descriptors under the existing canonical verifier.
        assert!(VerifiedCanonicalKernelIrV12::from_module(graph).is_err());
    }
}

#[test]
fn compact_fixture_preserves_all_memory_and_matrix_operations() {
    let graph = module(128);
    let body = graph.functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    let operations = &body.blocks[0].operations;
    let constants: Vec<_> = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::Constant(Constant::Index(value)) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(constants.len(), 9);
    let mut unique = constants;
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique, (0..=8).collect::<Vec<_>>());
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Load { .. }))
            .count(),
        20
    );
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Store { .. }))
            .count(),
        4
    );
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Matrix(_)))
            .count(),
        1
    );
    assert_eq!(
        body.parameters.len() + operations.iter().map(|op| op.results.len()).sum::<usize>(),
        77
    );
    assert_eq!(operations[0].results[0].id, ValueId(4));
    assert_eq!(limits().max_resident_bytes, 16 * 1024 * 1024);
    assert_eq!(limits().max_memory_access_records, 8192);
}

#[test]
fn independent_packing_covers_contiguous_a_split_k_b_and_a_padding() {
    let (a, b, c) = dense();
    let input = request(&a, &b, &c, 1, 64);
    for argument in 0..2 {
        let SimulationArgumentV1::Buffer(buffer) = &input.arguments[argument] else {
            unreachable!()
        };
        for lane in 0..64 {
            for word in 0..8 {
                let expected = if argument == 0 {
                    if word >= 4 {
                        0
                    } else {
                        (0..8).fold(0_u32, |packed, nibble| {
                            let k = 32 * (lane / 16) + word * 8 + nibble;
                            packed | (u32::from(a[(lane % 16) * 128 + k]) << (4 * nibble))
                        })
                    }
                } else {
                    (0..4).fold(0_u32, |packed, byte| {
                        let item = 4 * word + byte;
                        let k = 16 * (lane / 16) + item % 16 + 64 * (item / 16);
                        packed | (u32::from(b[k * 16 + lane % 16]) << (8 * byte))
                    })
                };
                let offset = (lane * 8 + word) * 4;
                assert_eq!(&buffer.bytes()[offset..offset + 4], &expected.to_le_bytes());
            }
        }
    }
}

mod fixture {
    //! Independent row-major oracle and inert canonical load/mixed-MFMA/store graph.
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
                    TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
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
        let mut module = Module::new("inert::mixed-fp4-fp8-exact-v1");
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
    pub fn decode_fp4(code: u8) -> f64 {
        let magnitude = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0][usize::from(code & 7)];
        if code & 8 == 0 { magnitude } else { -magnitude }
    }
    pub fn decode(code: u8) -> f64 {
        let sign = if code & 0x80 == 0 { 1.0 } else { -1.0 };
        let exponent = i32::from((code >> 3) & 15);
        let mantissa = f64::from(code & 7);
        assert!(!(exponent == 15 && mantissa == 7.0));
        if exponent == 0 {
            return sign * mantissa * 2.0_f64.powi(-9);
        }
        sign * (1.0 + mantissa / 8.0) * 2.0_f64.powi(exponent - 7)
    }
    pub fn oracle(a: &[u8; 2048], b: &[u8; 2048], c: &[i64; 256]) -> [u32; 256] {
        std::array::from_fn(|n| {
            let mut sum = c[n] as f64 / 16.0;
            for k in 0..128 {
                sum += decode_fp4(a[(n / 16) * 128 + k]) * decode(b[k * 16 + n % 16]);
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
                    let lane = col + 16 * ((k % 64) / 16);
                    let component = (k % 16) + 16 * (k / 64);
                    bb[wave * 512 + lane * 8 + component / 4] |=
                        u32::from(b[k * 16 + col]) << (8 * (component % 4));
                }
            }
            for row in 0..16 {
                for col in 0..16 {
                    cc[wave * 256 + (col + 16 * (row / 4)) * 4 + row % 4] =
                        ((c[row * 16 + col] as f64 / 16.0) as f32).to_bits();
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
}
