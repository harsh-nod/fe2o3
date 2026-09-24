//! Inert structural-owner corpus on the existing V17 simulator and call engine.
//! Not Rust-source custody, physical-register state or native/GPU qualification.
use fe2o3_kernel_ir as ordered_composition_fixture_ir;
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/ordered_composition_v1.rs"]
mod fixture;

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 64 * 1024,
        max_reachable_functions: 3,
        max_reachable_operations: 256,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 8192,
        max_call_depth: 2,
        max_ssa_values: 64,
        max_allocations: 3,
        max_allocation_bytes: 1024,
        max_total_bytes: 4096,
        max_resident_bytes: 32 * 1024 * 1024,
        max_events: 4096,
        max_memory_access_records: 2048,
    }
}
fn composition(
    module: &Module,
) -> (
    VerifiedOrderedProgramCompositionV1,
    CanonicalKernelIrOwnedVerificationResourceBudgetV1,
) {
    let mut ledger = CanonicalKernelIrOwnedVerificationResourceBudgetV1::new(
        CanonicalKernelIrWorkBudgetV1::new(100_000_000),
        64 * 1024 * 1024,
    );
    let (canonical, canonical_storage) = ledger
        .with_budget(|budget| {
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                module, budget,
            )
        })
        .unwrap();
    ledger
        .with_budget(|budget| budget.reserve_storage(canonical_storage.retained_storage()))
        .unwrap();
    let (owner, extra) = ledger
        .with_budget(|budget| {
            VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, budget)
        })
        .unwrap();
    ledger
        .with_budget(|budget| budget.reserve_storage(extra.retained_storage()))
        .unwrap();
    (owner, ledger)
}
fn indexed(mut module: Module) -> Module {
    let pointer = module.functions[0].signature.parameters[3].clone();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let mut store = block.operations.pop().unwrap();
    let OperationKind::Store {
        pointer: destination,
        ..
    } = &mut store.kind
    else {
        panic!()
    };
    *destination = ValueId(501);
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(500), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(501), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: ValueId(500),
            },
        ),
        store,
    ]);
    module
}
fn request(inputs: [u32; 3], invocations: u64, output_words: usize) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let bytes = output_words * 4 + 8;
    let mut arguments: Vec<_> = inputs
        .into_iter()
        .map(|value| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value)))
        .collect();
    arguments.push(SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0x5a; bytes],
            vec![false; bytes],
            target,
        )
        .unwrap(),
    ));
    SimulationRequestV1::new("kernel", [invocations, 1, 1], [64, 1, 1], arguments)
}
fn check_output(execution: &SimulationExecutionV1, words: usize, expected: u32) {
    let output = execution.buffer(3).unwrap();
    assert!(
        output.bytes()[..words * 4]
            .chunks_exact(4)
            .all(|word| word == expected.to_le_bytes())
    );
    assert_eq!(&output.bytes()[words * 4..], &[0x5a; 8]);
    assert!(output.initialized()[..words * 4].iter().all(|v| *v));
    assert_eq!(&output.initialized()[words * 4..], &[false; 8]);
    assert!(!execution.grants_execution_authority());
}
// A fixture-specific composition oracle delegates each region's semantics to the
// existing typed evaluator. It does not interpret instructions or generic KIR/CFG.
fn fixture_result(
    direct: usize,
    helpers: &[usize],
    calls: &[usize],
    steps: u8,
    [a, b, c]: [u32; 3],
) -> u32 {
    let mut value = a;
    for _ in 0..direct {
        value = fixture::program(steps, false).evaluate([value, b, c]);
    }
    for helper in calls {
        for _ in 0..helpers[*helper] {
            value = fixture::program(steps, (*helper + 1) % 2 == 1).evaluate([value, b, c]);
        }
    }
    value
}
#[test]
fn complete_existing_engine_corpus_covers_288_compositions_and_tails() {
    let shapes: [(usize, &[usize], &[usize]); 6] = [
        (1, &[], &[]),
        (2, &[], &[]),
        (0, &[1], &[0, 0]),
        (1, &[1, 2], &[0, 1, 0]),
        (1, &[0], &[0, 0]),
        (4, &[2], &[0, 0]),
    ];
    let inputs = [
        [0, 0, 0],
        [7, 11, 13],
        [u32::MAX, 1, 0],
        [0x8000_0000, u32::MAX, 1],
        [0xa5a5_a5a5, 0x5a5a_5a5a, 2],
        [u32::MAX; 3],
    ];
    let mut cases = 0;
    for (direct, helpers, calls) in shapes {
        for steps in [1, 3, 8, 16] {
            let module = indexed(fixture::module(direct, helpers, calls, steps));
            let (owner, ledger) = composition(&module);
            let incoming = (ledger.storage(), ledger.work(), ledger.peak_storage());
            // Existing V17 admission is separately bounded observational decode/reencode.
            // This does not mint source authority or replace the owner ledger.
            let admitted =
                AdmittedSimulationModuleV1::admit_v17(owner.canonical(), limits()).unwrap();
            assert_eq!(
                admitted.identity().digest(),
                owner.canonical().identity().digest()
            );
            assert_eq!(admitted.module(), owner.canonical().module());
            assert!(!admitted.grants_execution_authority());
            for values in inputs {
                for count in [64, 128] {
                    let request = request(values, count, count as usize);
                    let unchanged = request.clone();
                    let result = admitted
                        .simulate(&request, SimulationTargetV1::amdgpu_64(), limits())
                        .unwrap();
                    check_output(
                        &result,
                        count as usize,
                        fixture_result(direct, helpers, calls, steps, values),
                    );
                    assert_eq!(request, unchanged);
                    cases += 1;
                }
            }
            assert_eq!(
                (ledger.storage(), ledger.work(), ledger.peak_storage()),
                incoming
            );
        }
    }
    assert_eq!(cases, 288);
}
#[test]
fn two_distinct_algorithms_have_independent_mathematical_composition_oracle() {
    use Gfx942ProgramDestinationV1::Output;
    use Gfx942ProgramRoleV1::{Input0, Input1, Input2, Output as Out};
    let mut bitselect = [0; 16];
    bitselect[..3].copy_from_slice(&[0x0085, 0x0133, 0x019d]);
    let bitselect = Gfx942U32ProgramV1::from_descriptors(3, bitselect).unwrap();
    let mut mix = [0; 16];
    mix[0] = Gfx942ProgramInstructionV1::Move {
        destination: Output,
        source: Input0,
    }
    .descriptor();
    mix[1] = Gfx942ProgramInstructionV1::Binary {
        opcode: Gfx942ProgramBinaryOpcodeV1::Add,
        destination: Output,
        left: Out,
        right: Input1,
    }
    .descriptor();
    mix[2] = Gfx942ProgramInstructionV1::Binary {
        opcode: Gfx942ProgramBinaryOpcodeV1::Xor,
        destination: Output,
        left: Out,
        right: Input2,
    }
    .descriptor();
    let mix = Gfx942U32ProgramV1::from_descriptors(3, mix).unwrap();
    let mut module = indexed(fixture::module(1, &[1], &[0, 0], 3));
    for (function, f) in module.functions.iter_mut().enumerate() {
        for op in &mut f.body.as_mut().unwrap().blocks[0].operations {
            if let OperationKind::Gfx942OrderedProgram(old) = op.kind {
                op.kind = OperationKind::Gfx942OrderedProgram(
                    Gfx942OrderedProgramV1::new(
                        old.source(),
                        old.registers(),
                        *old.inputs(),
                        if function == 0 { bitselect } else { mix },
                    )
                    .unwrap(),
                );
            }
        }
    }
    let (owner, _ledger) = composition(&module);
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner.canonical(), limits()).unwrap();
    for [a, b, c] in [
        [7, 11, 13],
        [u32::MAX, 1, 0xffff_0000],
        [0, u32::MAX, u32::MAX],
    ] {
        let selected = b ^ ((a ^ b) & c);
        let expected = ((selected.wrapping_add(b) ^ c).wrapping_add(b)) ^ c;
        let result = admitted
            .simulate(
                &request([a, b, c], 64, 64),
                SimulationTargetV1::amdgpu_64(),
                limits(),
            )
            .unwrap();
        check_output(&result, 64, expected);
    }
}
#[test]
fn reused_physical_roles_do_not_overwrite_earlier_logical_ssa_result() {
    let mut module = indexed(fixture::module(2, &[], &[], 3));
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let OperationKind::Store { value, .. } = &mut block.operations.last_mut().unwrap().kind else {
        panic!()
    };
    *value = ValueId(100);
    let (owner, _ledger) = composition(&module);
    assert_eq!(owner.definitions().len(), 2);
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner.canonical(), limits()).unwrap();
    let values = [7, 11, 13];
    let first = fixture::program(3, false).evaluate(values);
    let second = fixture::program(3, false).evaluate([first, values[1], values[2]]);
    assert_ne!(first, second);
    let result = admitted
        .simulate(
            &request(values, 64, 64),
            SimulationTargetV1::amdgpu_64(),
            limits(),
        )
        .unwrap();
    check_output(&result, 64, first);
}
#[test]
fn surrounding_checked_wrapping_arithmetic_uses_existing_scalar_engine() {
    let mut module = indexed(fixture::module(0, &[1], &[0], 3));
    let block = &mut module.functions[1].body.as_mut().unwrap().blocks[0];
    block.operations.insert(
        0,
        Operation::new(
            vec![
                ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(91), Type::BOOL),
            ],
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
    );
    block.operations[1] = fixture::region(
        [ValueId(90), ValueId(11), ValueId(12)],
        ValueId(100),
        1,
        0,
        3,
    );
    let (owner, _ledger) = composition(&module);
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner.canonical(), limits()).unwrap();
    for [a, b, c] in [[u32::MAX, 1, 13], [7, u32::MAX, 0]] {
        let result = admitted
            .simulate(
                &request([a, b, c], 64, 64),
                SimulationTargetV1::amdgpu_64(),
                limits(),
            )
            .unwrap();
        check_output(
            &result,
            64,
            fixture::program(3, true).evaluate([a.wrapping_add(b), b, c]),
        );
    }
}
#[derive(Default)]
struct Records {
    received: usize,
    lane_zero: Vec<SimulationDebugRecordV1>,
}
impl SimulationDebugSinkV1 for Records {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.received += 1;
        assert!(self.received <= 4096);
        if record.invocation.global[0] == 0 {
            assert!(self.lane_zero.len() < 64);
            self.lane_zero.push(record);
        }
        SimulationDebugSinkControlV1::Continue
    }
}
#[test]
fn shared_helper_has_two_atomic_region_pairs_and_actual_distinct_frames() {
    let (owner, _ledger) = composition(&indexed(fixture::module(0, &[1], &[0, 0], 16)));
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner.canonical(), limits()).unwrap();
    let values = [7, 11, 13];
    let first = fixture::program(16, true).evaluate(values);
    let second = fixture::program(16, true).evaluate([first, values[1], values[2]]);
    let mut records = Records::default();
    let execution = admitted
        .simulate_debugged_with_sink(
            &request(values, 64, 64),
            SimulationTargetV1::amdgpu_64(),
            limits(),
            SimulationDebugCaptureLimitsV1::new(4, 32, 2, 1024).unwrap(),
            &mut records,
        )
        .unwrap();
    check_output(&execution, 64, second);
    let helper: Vec<_> = records
        .lane_zero
        .iter()
        .filter(|r| {
            r.site.function_ordinal == 1 && r.site.block == BlockId(73) && r.site.operation == 0
        })
        .collect();
    // Sixteen authored instructions remain one Before/After region, twice.
    assert_eq!(helper.len(), 4);
    for (call, expected) in [first, second].into_iter().enumerate() {
        let SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::BeforeOperation,
            ..
        } = &helper[call * 2].kind
        else {
            panic!("missing actual before");
        };
        let SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            stack: SimulationDebugCollectionV1::Captured(frames),
            ..
        } = &helper[call * 2 + 1].kind
        else {
            panic!("missing actual after");
        };
        let frame = frames
            .iter()
            .find(|frame| frame.function_ordinal == 1)
            .unwrap();
        let SimulationDebugCollectionV1::Captured(bindings) = &frame.values else {
            panic!()
        };
        assert_eq!(bindings.len(), 4);
        assert_eq!(
            bindings
                .iter()
                .find(|v| v.value == ValueId(100))
                .unwrap()
                .observed,
            SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(expected))
        );
        assert!(
            bindings
                .iter()
                .all(|v| [ValueId(10), ValueId(11), ValueId(12), ValueId(100)].contains(&v.value))
        );
        if call == 1 {
            let root = frames
                .iter()
                .find(|frame| frame.function_ordinal == 0)
                .unwrap();
            let SimulationDebugCollectionV1::Captured(bindings) = &root.values else {
                panic!()
            };
            assert_eq!(
                bindings
                    .iter()
                    .find(|v| v.value == ValueId(100))
                    .unwrap()
                    .observed,
                SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(first))
            );
        }
    }
}
#[test]
fn exact_existing_admission_and_step_limits_remain_transactional() {
    let (owner, ledger) = composition(&indexed(fixture::module(1, &[1], &[0, 0], 8)));
    let identity = *owner.canonical().identity();
    let mut exact = limits();
    exact.max_canonical_bytes = owner.canonical().canonical_bytes().len();
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner.canonical(), exact).unwrap();
    exact.max_canonical_bytes -= 1;
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v17(owner.canonical(), exact),
        Err(SimulationAdmissionErrorV1::CanonicalBytesLimit { .. })
    ));
    let request = request([7, 11, 13], 64, 64);
    let original = request.clone();
    let run = admitted
        .simulate(&request, SimulationTargetV1::amdgpu_64(), limits())
        .unwrap();
    let mut exact = limits();
    exact.max_steps = run.steps_executed();
    assert!(
        admitted
            .simulate(&request, SimulationTargetV1::amdgpu_64(), exact)
            .is_ok()
    );
    exact.max_steps -= 1;
    assert!(
        admitted
            .simulate(&request, SimulationTargetV1::amdgpu_64(), exact)
            .is_err()
    );
    assert_eq!(request, original);
    assert_eq!(*owner.canonical().identity(), identity);
    assert!(ledger.storage() > 0);
}
#[test]
fn partial_wave_and_insufficient_output_refuse_without_mutating_request() {
    let (owner, _ledger) = composition(&indexed(fixture::module(0, &[1], &[0, 0], 3)));
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner.canonical(), limits()).unwrap();
    let wrong = request([7, 11, 13], 63, 63);
    assert!(matches!(
        admitted.preflight(&wrong, SimulationTargetV1::amdgpu_64(), limits()),
        Err(SimulationPreflightErrorV1::Unsupported(_))
    ));
    let short = request([7, 11, 13], 64, 0);
    let original = short.clone();
    assert!(
        admitted
            .simulate(&short, SimulationTargetV1::amdgpu_64(), limits())
            .is_err()
    );
    assert_eq!(short, original);
}
