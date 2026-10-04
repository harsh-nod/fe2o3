//! Inert canonical Call/Return controls, not Rust source or native authority.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Owner, *,
};
use fe2o3_kir_sim::*;
use std::cell::Cell;
#[path = "matrix_bf16_exact/fixture.rs"]
mod fixture;

#[derive(Clone, Copy)]
enum Shape {
    Identity,
    Swap01,
    Branches,
    WrongOperand,
    DuplicateReturn,
    Cycle,
    ExtraBlock,
    Divergent,
}
fn graph(shape: Shape) -> Module {
    // Reuse the independently-oracled memory fixture, but retain a real separate
    // helper Function and actual Call/Return; no executor-side graph rewriting.
    let mut graph = fixture::module(64);
    let root = &mut graph.functions[0];
    let ops = &mut root.body.as_mut().unwrap().blocks[0].operations;
    let at = ops
        .iter()
        .position(|op| matches!(op.kind, OperationKind::Matrix(_)))
        .unwrap();
    let OperationKind::Matrix(old) = &ops[at].kind else {
        unreachable!()
    };
    let MatrixOperationKind::MultiplyAccumulate {
        lhs,
        rhs,
        accumulator,
        ..
    } = &old.kind
    else {
        unreachable!()
    };
    let arguments = lhs.iter().chain(rhs).chain(accumulator).copied().collect();
    ops[at].kind = OperationKind::Call {
        callee: "real_helper".into(),
        arguments,
    };
    let mut params = vec![Type::Scalar(ScalarType::Bf16); 8];
    params.extend(vec![Type::F32; 4]);
    let mut helper = BasicBlock::new(BlockId(0));
    let mut lhs = [ValueId(0), ValueId(1), ValueId(2), ValueId(3)];
    if matches!(shape, Shape::WrongOperand) {
        lhs.swap(0, 1);
    }
    helper.operations.push(Operation::new(
        (12..16)
            .map(|id| ValueDef::new(ValueId(id), Type::F32))
            .collect(),
        OperationKind::Matrix(
            MatrixOperation::multiply_accumulate(
                lhs,
                [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            ),
        ),
    ));
    let mut returned = vec![ValueId(12), ValueId(13), ValueId(14), ValueId(15)];
    if matches!(shape, Shape::Swap01) {
        returned.swap(0, 1);
    }
    if matches!(shape, Shape::DuplicateReturn) {
        returned[1] = returned[0];
    }
    helper.terminator = Some(Terminator::Return { values: returned });
    let mut blocks = vec![helper];
    if matches!(shape, Shape::Branches) {
        // One predecessor identity edges on BOTH sides of the actual Matrix.
        let mut entry = BasicBlock::new(BlockId(0));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: (0..12).map(ValueId).collect(),
        });
        let mut middle = blocks.remove(0);
        middle.id = BlockId(1);
        middle.parameters = params
            .iter()
            .enumerate()
            .map(|(i, ty)| ValueDef::new(ValueId(20 + i as u32), ty.clone()))
            .collect();
        middle.operations[0].kind = OperationKind::Matrix(
            MatrixOperation::multiply_accumulate(
                [ValueId(20), ValueId(21), ValueId(22), ValueId(23)],
                [ValueId(24), ValueId(25), ValueId(26), ValueId(27)],
                [ValueId(28), ValueId(29), ValueId(30), ValueId(31)],
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            ),
        );
        middle.terminator = Some(Terminator::Branch {
            target: BlockId(2),
            arguments: (12..16).map(ValueId).collect(),
        });
        let mut end = BasicBlock::new(BlockId(2));
        end.parameters = (32..36)
            .map(|id| ValueDef::new(ValueId(id), Type::F32))
            .collect();
        end.terminator = Some(Terminator::Return {
            values: (32..36).map(ValueId).collect(),
        });
        blocks = vec![entry, middle, end];
    }
    if matches!(shape, Shape::Cycle) {
        blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(0),
            arguments: vec![],
        });
    }
    if matches!(shape, Shape::ExtraBlock) {
        let mut extra = BasicBlock::new(BlockId(1));
        extra.terminator = Some(Terminator::Return {
            values: (8..12).map(ValueId).collect(),
        });
        blocks.push(extra);
    }
    if matches!(shape, Shape::Divergent) {
        let body = graph.functions[0].body.as_mut().unwrap();
        let at = body.blocks[0]
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Call { .. }))
            .unwrap();
        let tail = body.blocks[0].operations.split_off(at);
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(200), Type::INDEX),
            OperationKind::Constant(Constant::Index(32)),
        ));
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(201), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(4),
                rhs: ValueId(200),
            },
        ));
        body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(201),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut active = BasicBlock::new(BlockId(1));
        active.operations = tail;
        active.terminator = Some(Terminator::Return { values: vec![] });
        let mut inactive = BasicBlock::new(BlockId(2));
        inactive.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.extend([active, inactive]);
    }
    graph.functions.push(Function::internal_helper(
        "real_helper",
        Signature::new(params, vec![Type::F32; 4]),
        (0..12).map(ValueId).collect(),
        blocks,
    ));
    let mut caps = std::collections::BTreeSet::new();
    for f in &mut graph.functions {
        f.required_capabilities = f.derived_capabilities();
        caps.extend(f.required_capabilities.iter().cloned());
    }
    graph.kernels[0].required_capabilities = caps.clone();
    graph.required_capabilities = caps;
    graph
}
fn prepared(shape: Shape) -> (Owner, AdmittedSimulationModuleV1, usize) {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 32 * 1024 * 1024);
    let (owner, original) =
        Owner::from_module_ref_with_verification_budget_v12(&graph(shape), &mut budget).unwrap();
    budget.reserve_storage(original.retained_storage()).unwrap();
    let (view, copy) = AdmittedSimulationModuleV1::admit_v12_with_verification_budget(
        &owner,
        Bf16CallCpuObservationOptionsV1::default().simulation_limits(),
        &mut budget,
    )
    .unwrap();
    (
        owner,
        view,
        original.retained_storage() + copy.retained_storage() + 8192,
    )
}
struct Drain;
impl SimulationDebugSinkV1 for Drain {
    fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        SimulationDebugSinkControlV1::Continue
    }
}
fn request() -> (SimulationRequestV1, [u32; 256]) {
    let a = std::array::from_fn(|i| (i % 7) as i64 - 3);
    let b = std::array::from_fn(|i| (i % 5) as i64 - 2);
    let c = std::array::from_fn(|i| (i % 11) as i64 - 5);
    (
        fixture::request(&a, &b, &c, 1, 64),
        fixture::oracle(&a, &b, &c),
    )
}
fn observe(
    owner: &Owner,
    view: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    budget: &mut Budget<'_>,
    options: Bf16CallCpuObservationOptionsV1,
) -> Result<bool, V12CpuObservationErrorV1<()>> {
    view.with_bf16_call_cpu_observation_v1(
        V12CpuObservationInputV1::new(owner, request),
        options,
        budget,
        (&mut NoopSimulationEventSinkV1, &mut Drain),
        |result, _| Ok(result.is_ok()),
    )
}

struct Frames<'a> {
    owner: &'a Owner,
    expected: &'a [u32; 256],
    swap: bool,
    matrix: u64,
    call: u64,
}
impl SimulationDebugSinkV1 for Frames<'_> {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        let Some(function) = self
            .owner
            .module()
            .functions
            .get(record.site.function_ordinal)
        else {
            panic!("actual function");
        };
        let body = function.body.as_ref().unwrap();
        let block = body
            .blocks
            .iter()
            .find(|b| b.id == record.site.block)
            .unwrap();
        let Some(op) = block.operations.get(record.site.operation as usize) else {
            return SimulationDebugSinkControlV1::Continue;
        };
        let is_matrix = matches!(op.kind, OperationKind::Matrix(_));
        let is_call = matches!(&op.kind, OperationKind::Call { callee, .. } if callee.as_str() == "real_helper");
        if !is_matrix && !is_call {
            return SimulationDebugSinkControlV1::Continue;
        }
        let SimulationDebugRecordKindV1::Checkpoint { phase, stack, .. } = record.kind else {
            return SimulationDebugSinkControlV1::Continue;
        };
        if phase != SimulationDebugCheckpointPhaseV1::AfterOperation {
            return SimulationDebugSinkControlV1::Continue;
        }
        let SimulationDebugCollectionV1::Captured(frames) = stack else {
            panic!("complete frames");
        };
        assert_eq!(frames.len(), if is_matrix { 2 } else { 1 });
        assert_eq!(frames[0].function_ordinal, 0);
        assert_eq!(frames[0].depth, 0);
        let frame = frames.last().unwrap();
        assert_eq!(frame.function_ordinal, if is_matrix { 1 } else { 0 });
        assert_eq!(frame.depth, if is_matrix { 1 } else { 0 });
        let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
            panic!("complete values");
        };
        let lane = record.invocation.local[0] as usize;
        assert!(lane < 64);
        let seen = if is_matrix {
            &mut self.matrix
        } else {
            &mut self.call
        };
        assert_eq!(*seen & (1u64 << lane), 0);
        *seen |= 1u64 << lane;
        assert_eq!(op.results.len(), 4);
        for (component, def) in op.results.iter().enumerate() {
            let component = if is_call && self.swap && component < 2 {
                1 - component
            } else {
                component
            };
            let row = (lane / 16) * 4 + component;
            let col = lane % 16;
            let expected = self.expected[row * 16 + col];
            let mut matched = values.iter().filter(|v| v.value == def.id);
            let actual = matched.next().expect("actual result SSA");
            assert!(matched.next().is_none());
            assert_eq!(
                actual.observed,
                SimulationDebugValueV1::Scalar(
                    ScalarBitsV1::new(ScalarType::F32, u128::from(expected), fixture::TARGET)
                        .unwrap()
                )
            );
        }
        SimulationDebugSinkControlV1::Continue
    }
}
#[test]
fn actual_two_frames_call_return_permutation_and_all_values_match_independent_oracle() {
    for shape in [Shape::Identity, Shape::Swap01, Shape::Branches] {
        let (owner, view, floor) = prepared(shape);
        let (request, expected) = request();
        let swap = matches!(shape, Shape::Swap01);
        let mut frames = Frames {
            owner: &owner,
            expected: &expected,
            swap,
            matrix: 0,
            call: 0,
        };
        let mut work = Work::new(1usize << 54);
        let mut budget = Budget::new(&mut work, 1usize << 31);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(23).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        view.with_bf16_call_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            Bf16CallCpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut frames),
            |result, meter| {
                let actual = result.unwrap();
                let expected_output = std::array::from_fn(|i| {
                    let row = i / 16;
                    let col = i % 16;
                    let source_row = if swap && row % 4 < 2 { row ^ 1 } else { row };
                    expected[source_row * 16 + col]
                });
                fixture::check_output(actual, &expected_output, 1);
                assert!(actual.buffer(3).unwrap().initialized().iter().all(|v| *v));
                assert!(meter.work_ledger_identity_v1() == ledger);
                assert_eq!(actual.invocations_executed(), 64);
                Ok::<_, ()>(())
            },
        )
        .unwrap();
        assert_eq!((frames.matrix, frames.call), (u64::MAX, u64::MAX));
        let legacy = fixture::admit(graph(shape))
            .simulate(
                &request,
                fixture::TARGET,
                Bf16CallCpuObservationOptionsV1::default().simulation_limits(),
            )
            .unwrap();
        let expected_output = std::array::from_fn(|i| {
            let row = i / 16;
            let col = i % 16;
            let source_row = if swap && row % 4 < 2 { row ^ 1 } else { row };
            expected[source_row * 16 + col]
        });
        fixture::check_output(&legacy, &expected_output, 1);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        // The old finite profile remains closed, not silently generalized.
        assert_eq!(
            view.with_v12_cpu_observation_v1(
                V12CpuObservationInputV1::new(&owner, &request),
                V12CpuObservationOptionsV1::default(),
                &mut budget,
                (&mut NoopSimulationEventSinkV1, &mut Drain),
                |_, _| Ok::<_, ()>(())
            ),
            Err(V12CpuObservationErrorV1::Profile(
                V12CpuObservationProfileErrorV1::FunctionRoster
            ))
        );
    }
}
#[test]
fn actual_canonical_wrong_operand_return_cycle_or_unreachable_helper_refuse_before_callback() {
    for (shape, expected) in [
        (Shape::WrongOperand, V12CpuObservationProfileErrorV1::Matrix),
        (
            Shape::DuplicateReturn,
            V12CpuObservationProfileErrorV1::Operation,
        ),
        (Shape::Cycle, V12CpuObservationProfileErrorV1::Operation),
        (
            Shape::ExtraBlock,
            V12CpuObservationProfileErrorV1::Operation,
        ),
    ] {
        let (owner, view, floor) = prepared(shape);
        let (request, _) = request();
        let mut work = Work::new(1usize << 54);
        let mut budget = Budget::new(&mut work, 1usize << 31);
        budget.reserve_storage(floor).unwrap();
        let result = view.with_bf16_call_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            Bf16CallCpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Drain),
            |_, _| -> Result<(), ()> { panic!("invalid helper must not reach execution observer") },
        );
        assert_eq!(result, Err(V12CpuObservationErrorV1::Profile(expected)));
        assert_eq!(budget.storage(), floor);
    }
}
#[test]
fn exact_and_one_short_prepaid_resources_use_original_nonzero_floor() {
    let (owner, view, floor) = prepared(Shape::Identity);
    let (request, _) = request();
    let run = |w, s| {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(23).unwrap();
        let reached = Cell::new(false);
        let result = view.with_bf16_call_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            Bf16CallCpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Drain),
            |result, _| {
                reached.set(true);
                Ok::<_, ()>(result.is_ok())
            },
        );
        assert_eq!(budget.storage(), floor);
        (
            result,
            reached.get(),
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
            budget.failed_work(),
        )
    };
    let good = run(1usize << 54, 1usize << 31);
    assert_eq!(good.0, Ok(true));
    assert_eq!(run(good.2, good.3).0, Ok(true));
    let w = run(good.2 - 1, good.3);
    assert!(w.0.is_err() && !w.1 && w.5.is_some());
    let s = run(good.2, good.3 - 1);
    assert!(s.0.is_err() && !s.1 && s.4.is_some());
}
#[test]
fn domain_failure_does_not_bind_any_matrix_or_call_result() {
    let (owner, view, floor) = prepared(Shape::Identity);
    let (mut request, expected) = request();
    fixture::alter(&mut request, 1, 510, &0x8000u16.to_le_bytes(), true);
    let mut frames = Frames {
        owner: &owner,
        expected: &expected,
        swap: false,
        matrix: 0,
        call: 0,
    };
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    view.with_bf16_call_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request), Bf16CallCpuObservationOptionsV1::default(),
        &mut budget, (&mut NoopSimulationEventSinkV1, &mut frames), |result, _| {
            assert!(matches!(result, Err(SimulationErrorV1::Execution(e))
                if matches!(e.kind, SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain { .. })));
            Ok::<_, ()>(())
        }).unwrap();
    assert_eq!((frames.matrix, frames.call), (0, 0));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn error_panic_and_prior_denial_do_not_reset_or_leak_original_ledger() {
    let (owner, view, floor) = prepared(Shape::Identity);
    let (request, _) = request();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = view.with_bf16_call_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        Bf16CallCpuObservationOptionsV1::default(),
        &mut budget,
        (&mut NoopSimulationEventSinkV1, &mut Drain),
        |result, meter| {
            assert!(result.is_ok());
            meter.charge_work(19).unwrap();
            Err::<(), _>(7u8)
        },
    );
    assert_eq!(result, Err(V12CpuObservationErrorV1::Observer(7)));
    assert_eq!(budget.storage(), floor);
    let before = budget.work();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), V12CpuObservationErrorV1<()>> = view
                .with_bf16_call_cpu_observation_v1(
                    V12CpuObservationInputV1::new(&owner, &request),
                    Bf16CallCpuObservationOptionsV1::default(),
                    &mut budget,
                    (&mut NoopSimulationEventSinkV1, &mut Drain),
                    |result, meter| {
                        assert!(result.is_ok());
                        meter.charge_work(17).unwrap();
                        panic!("actual callback unwind")
                    },
                );
        }))
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > before && budget.work_ledger_identity_v1() == ledger);
    assert!(budget.charge_work(usize::MAX).is_err());
    let before = budget.work();
    assert_eq!(
        observe(
            &owner,
            &view,
            &request,
            &mut budget,
            Bf16CallCpuObservationOptionsV1::default()
        ),
        Err(V12CpuObservationErrorV1::Profile(
            V12CpuObservationProfileErrorV1::Accounting
        ))
    );
    assert_eq!(budget.work(), before);
}
#[test]
fn limits_reduce_only_and_incomplete_capture_never_reaches_observer() {
    assert!(
        Bf16CallCpuObservationOptionsV1::default()
            .with_step_limit(131073)
            .is_err()
    );
    assert!(
        Bf16CallCpuObservationOptionsV1::default()
            .with_record_limit(65537)
            .is_err()
    );
    let (owner, view, floor) = prepared(Shape::Identity);
    let (mut request, _) = request();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        view.with_bf16_call_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            Bf16CallCpuObservationOptionsV1::default()
                .with_record_limit(1)
                .unwrap(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Drain),
            |_, _| -> Result<(), ()> { panic!("incomplete") }
        ),
        Err(V12CpuObservationErrorV1::IncompleteObservation)
    );
    assert_eq!(
        observe(
            &owner,
            &view,
            &request,
            &mut budget,
            Bf16CallCpuObservationOptionsV1::default()
                .with_step_limit(1)
                .unwrap()
        ),
        Ok(false)
    );
    request.grid.0[0] = 63;
    assert_eq!(
        observe(
            &owner,
            &view,
            &request,
            &mut budget,
            Bf16CallCpuObservationOptionsV1::default()
        ),
        Err(V12CpuObservationErrorV1::Profile(
            V12CpuObservationProfileErrorV1::Launch
        ))
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn divergent_root_call_has_no_successful_helper_results() {
    let (owner, view, floor) = prepared(Shape::Divergent);
    let (request, expected) = request();
    let mut frames = Frames {
        owner: &owner,
        expected: &expected,
        swap: false,
        matrix: 0,
        call: 0,
    };
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    view.with_bf16_call_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        Bf16CallCpuObservationOptionsV1::default(),
        &mut budget,
        (&mut NoopSimulationEventSinkV1, &mut frames),
        |result, _| {
            assert!(matches!(result, Err(SimulationErrorV1::Execution(e))
                if matches!(e.kind, SimulationExecutionErrorKindV1::DivergentWave(_))));
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!((frames.matrix, frames.call), (0, 0));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn actual_stores_initialize_output_and_preserve_initialized_canaries() {
    let (owner, view, floor) = prepared(Shape::Identity);
    let (mut request, expected) = request();
    let SimulationArgumentV1::Buffer(old) = &request.arguments[3] else {
        panic!("output");
    };
    let bytes = old.bytes().to_vec();
    let mut init = old.initialized().to_vec();
    let end = init.len() - 8;
    init[8..end].fill(false);
    request.arguments[3] = SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            old.element(),
            old.access(),
            old.alignment(),
            bytes,
            init,
            fixture::TARGET,
        )
        .unwrap(),
    );
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    view.with_bf16_call_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        Bf16CallCpuObservationOptionsV1::default(),
        &mut budget,
        (&mut NoopSimulationEventSinkV1, &mut Drain),
        |result, _| {
            let actual = result.unwrap();
            fixture::check_output(actual, &expected, 1);
            assert!(actual.buffer(3).unwrap().initialized().iter().all(|v| *v));
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
}
