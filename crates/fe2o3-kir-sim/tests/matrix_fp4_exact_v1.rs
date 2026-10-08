//! Inert canonical graphs only: not Rust source or hardware qualification.
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;
#[path = "matrix_fp4_exact/fixture.rs"]
mod fixture;
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
    let codes = [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15];
    (
        std::array::from_fn(|n| codes[(n * 7 + n / 128) % 15]),
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
    // All 128 k positions, including every dword/nibble/lane-group boundary.
    // Distinct row/column markers cover all 256 outputs, not a scalar checksum.
    for k in 0..128 {
        let mut a = [0; 2048];
        let mut b = [0; 2048];
        let codes = [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 1, 3];
        for row in 0..16 {
            a[row * 128 + k] = codes[row];
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
fn exact_extremes_quarters_and_cancellation_remain_exact() {
    let sim = admit(module(64));
    for (a, b, c) in [
        ([7; 2048], [7; 2048], [4_194_304; 256]),
        ([15; 2048], [7; 2048], [-4_194_304; 256]),
        ([1; 2048], [1; 2048], [-128; 256]),
        ([0; 2048], [15; 2048], [1; 256]),
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
                    seed: 0xf004_1234,
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
                    seed: 0xf004_1234,
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
                (255, 3, 0x3e00_0000),
                (255, 3, 0x4980_0001),
                (255, 3, 0x7fc0_0000),
                (255, 3, 0x7f80_0000),
            ]
        } else {
            let mut cases = vec![(63 * 8 + 3, 31, 0x8111_1111)];
            for word in 4..8 {
                cases.push((63 * 8 + word, (28 + word) as u8, 1));
            }
            cases
        };
        for (word, component, bits) in cases {
            let mut request = request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
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
    let mut request = request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
    alter(&mut request, 0, 0, &0x1111_1111_u32.to_le_bytes(), false);
    assert!(matches!(
        kind(sim.simulate(&request, TARGET, limits()).unwrap_err()),
        SimulationExecutionErrorKindV1::UninitializedRead { .. }
    ));
    let mut short = fixture::request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
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
        let mut request = request(&[1; 2048], &[1; 2048], &[0; 256], 2, 64);
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
    let request = request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
    let before = 64 * (u64::from(position) + 1 + 40);
    let denied = SimulationLimitsV1 {
        max_steps: before + 66 * 64 + 268_672 - 1,
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
    let request = request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
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
    let request = request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
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
    let request = request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64);
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
fn mixed_layout_refuses_while_exact_fp8_profile_admits() {
    for layout in [
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
    ] {
        let mut graph = module(64);
        let position = matrix_position(&graph);
        let OperationKind::Matrix(matrix) =
            &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations[position].kind
        else {
            unreachable!()
        };
        if layout == TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64() {
            let MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } = &mut matrix.kind
            else {
                unreachable!()
            };
            *profile = MatrixMultiplyProfile::fp8_e4m3_f32_m16n16k128_wave64();
        }
        matrix.tensor_layout = Some(layout);
        let caps = graph.functions[0].derived_capabilities();
        graph.functions[0].required_capabilities = caps.clone();
        graph.kernels[0].required_capabilities = caps.clone();
        graph.required_capabilities = caps;
        let sim = admit(graph);
        if layout == TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64() {
            assert!(
                sim.preflight(
                    &request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64),
                    TARGET,
                    limits(),
                )
                .is_ok()
            );
            continue;
        }
        let err = sim
            .preflight(
                &request(&[1; 2048], &[1; 2048], &[0; 256], 1, 64),
                TARGET,
                limits(),
            )
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = err else {
            panic!("expected numerical refusal");
        };
        assert_eq!(report.total_findings(), 1);
        assert_eq!(
            report.findings()[0].feature,
            UnsupportedFeatureV1::UnsupportedNumericalContract
        );
        assert_eq!(
            report.findings()[0].operation,
            Some(u32::try_from(position).unwrap())
        );
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
