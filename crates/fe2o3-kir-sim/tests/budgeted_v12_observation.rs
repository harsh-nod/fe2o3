//! Inert V12 owners only: numerical/source custody is not inferred from these tests.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kir_sim::*;
#[path = "matrix_bf16_exact/fixture.rs"]
mod fixture;
use std::cell::Cell;
struct Drain;
impl SimulationDebugSinkV1 for Drain {
    fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        SimulationDebugSinkControlV1::Continue
    }
}

fn prepared() -> (
    Owner,
    AdmittedSimulationModuleV1,
    usize,
    SimulationRequestV1,
) {
    prepared_module(fixture::module(64))
}
fn prepared_module(
    graph: fe2o3_kernel_ir::Module,
) -> (
    Owner,
    AdmittedSimulationModuleV1,
    usize,
    SimulationRequestV1,
) {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 32 * 1024 * 1024);
    let (owner, source) =
        Owner::from_module_ref_with_verification_budget_v12(&graph, &mut budget).unwrap();
    budget.reserve_storage(source.retained_storage()).unwrap();
    let (view, receipt) = AdmittedSimulationModuleV1::admit_v12_with_verification_budget(
        &owner,
        V12CpuObservationOptionsV1::default().simulation_limits(),
        &mut budget,
    )
    .unwrap();
    let floor = source.retained_storage() + receipt.retained_storage() + 8192;
    (
        owner,
        view,
        floor,
        fixture::request(&[1; 256], &[1; 256], &[0; 256], 1, 64),
    )
}
fn observe(
    owner: &Owner,
    view: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    budget: &mut Budget<'_>,
    options: V12CpuObservationOptionsV1,
) -> Result<bool, V12CpuObservationErrorV1<()>> {
    view.with_v12_cpu_observation_v1(
        V12CpuObservationInputV1::new(owner, request),
        options,
        budget,
        (&mut NoopSimulationEventSinkV1, &mut Drain),
        |result, _| Ok(result.is_ok()),
    )
}
#[test]
fn original_ledger_borrows_actual_output_without_new_execution_owner() {
    let (owner, view, floor, request) = prepared();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(23).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let expected = fixture::oracle(&[1; 256], &[1; 256], &[0; 256]);
    let result = view
        .with_v12_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            V12CpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Drain),
            |result, meter| {
                let run = result.unwrap();
                fixture::check_output(run, &expected, 1);
                assert!(meter.storage() > floor);
                assert!(meter.work_ledger_identity_v1() == ledger);
                Ok::<_, ()>(run.invocations_executed())
            },
        )
        .unwrap();
    assert_eq!(result, 64);
    let legacy = fixture::admit(fixture::module(64))
        .simulate(
            &request,
            fixture::TARGET,
            V12CpuObservationOptionsV1::default().simulation_limits(),
        )
        .unwrap();
    fixture::check_output(&legacy, &expected, 1);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 23);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
#[test]
fn exact_and_one_short_resources_restore_nonzero_floor_and_deny_before_callback() {
    let (owner, view, floor, request) = prepared();
    let run = |w, s| {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(23).unwrap();
        let reached = Cell::new(false);
        let outcome = view.with_v12_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            V12CpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Drain),
            |result, _| {
                reached.set(true);
                Ok::<_, ()>(result.is_ok())
            },
        );
        assert_eq!(budget.storage(), floor);
        let used = budget.work();
        let peak = budget.peak_storage();
        let failed_storage = budget.failed_storage();
        (
            outcome,
            reached.get(),
            used,
            peak,
            failed_storage,
            work.failed_work(),
        )
    };
    let good = run(1usize << 54, 1usize << 31);
    assert_eq!(good.0, Ok(true));
    assert_eq!(run(good.2, good.3).0, Ok(true));
    let short_work = run(good.2 - 1, good.3);
    assert!(short_work.0.is_err() && !short_work.1 && short_work.5.is_some());
    let short_storage = run(good.2, good.3 - 1);
    assert!(short_storage.0.is_err() && !short_storage.1 && short_storage.4.is_some());
}
#[test]
fn domain_failure_stays_borrowed_and_is_not_successful_numerical_evidence() {
    let (owner, view, floor, mut request) = prepared();
    fixture::alter(&mut request, 0, 510, &0x8000u16.to_le_bytes(), true);
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    let ok = view.with_v12_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request), V12CpuObservationOptionsV1::default(),
        &mut budget, (&mut NoopSimulationEventSinkV1, &mut Drain),
        |result, _| {
            assert!(matches!(result, Err(SimulationErrorV1::Execution(e))
                if matches!(e.kind, SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain { .. })));
            Ok::<_, ()>(false)
        },
    ).unwrap();
    assert!(!ok);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn sink_stop_and_sink_panic_are_not_successful_complete_observations() {
    struct Stop;
    impl SimulationDebugSinkV1 for Stop {
        fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
            SimulationDebugSinkControlV1::Stop
        }
    }
    struct Panic;
    impl SimulationDebugSinkV1 for Panic {
        fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
            panic!("inert debug sink panic")
        }
    }
    let (owner, view, floor, request) = prepared();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    let result = view.with_v12_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        V12CpuObservationOptionsV1::default(),
        &mut budget,
        (&mut NoopSimulationEventSinkV1, &mut Stop),
        |_, _| -> Result<(), ()> { panic!("stopped sink must not reach observer") },
    );
    assert_eq!(result, Err(V12CpuObservationErrorV1::IncompleteObservation));
    assert_eq!(budget.storage(), floor);
    let before = budget.work();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), V12CpuObservationErrorV1<()>> = view.with_v12_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            V12CpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Panic),
            |_, _| -> Result<(), ()> { panic!("panicked sink must not reach observer") },
        );
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > before);
}
#[test]
fn caller_error_and_unwind_drop_charged_result_before_restoring_floor() {
    let (owner, view, floor, request) = prepared();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    let result = view.with_v12_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        V12CpuObservationOptionsV1::default(),
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
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), V12CpuObservationErrorV1<()>> = view.with_v12_cpu_observation_v1(
            V12CpuObservationInputV1::new(&owner, &request),
            V12CpuObservationOptionsV1::default(),
            &mut budget,
            (&mut NoopSimulationEventSinkV1, &mut Drain),
            |result, meter| {
                assert!(result.is_ok());
                meter.charge_work(17).unwrap();
                panic!("inert observation control")
            },
        );
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > before);
}
#[test]
fn bounded_debug_cap_refuses_without_calling_success_continuation() {
    let (owner, view, floor, request) = prepared();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    let result = view.with_v12_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        V12CpuObservationOptionsV1::default()
            .with_record_limit(1)
            .unwrap(),
        &mut budget,
        (&mut NoopSimulationEventSinkV1, &mut Drain),
        |_, _| -> Result<(), ()> { panic!("incomplete capture must not reach observer") },
    );
    assert_eq!(result, Err(V12CpuObservationErrorV1::IncompleteObservation));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn prior_denial_and_foreign_owner_never_reset_original_ledger() {
    let (owner, view, floor, request) = prepared();
    let mut graph = fixture::module(64);
    graph.id = "different_inert_owner".into();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    let (foreign, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(&graph, &mut budget).unwrap();
    budget
        .reserve_storage(floor + receipt.retained_storage())
        .unwrap();
    assert_eq!(
        observe(
            &foreign,
            &view,
            &request,
            &mut budget,
            V12CpuObservationOptionsV1::default()
        ),
        Err(V12CpuObservationErrorV1::Profile(
            V12CpuObservationProfileErrorV1::OwnerMismatch
        ))
    );
    assert!(budget.charge_work(usize::MAX).is_err());
    let before = budget.work();
    assert_eq!(
        observe(
            &owner,
            &view,
            &request,
            &mut budget,
            V12CpuObservationOptionsV1::default()
        ),
        Err(V12CpuObservationErrorV1::Profile(
            V12CpuObservationProfileErrorV1::Accounting
        ))
    );
    assert_eq!(budget.work(), before);
    assert_eq!(budget.failed_work(), Some(usize::MAX));
}
#[test]
fn reduced_steps_and_launch_mismatch_remain_distinct_refusals() {
    let (owner, view, floor, mut request) = prepared();
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        observe(
            &owner,
            &view,
            &request,
            &mut budget,
            V12CpuObservationOptionsV1::default()
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
            V12CpuObservationOptionsV1::default()
        ),
        Err(V12CpuObservationErrorV1::Profile(
            V12CpuObservationProfileErrorV1::Launch
        ))
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn full_wave_lane_id_uses_existing_collective_and_exact_before_after_debug_values() {
    use fe2o3_kernel_ir::{
        BlockId, Operation, OperationKind, ScalarType, Type, ValueDef, ValueId, WaveOperation,
        WaveOperationKind, WaveWidth,
    };
    struct LaneCheck {
        before: u64,
        after: u64,
    }
    impl SimulationDebugSinkV1 for LaneCheck {
        fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
            if record.site.function_ordinal != 0
                || record.site.block != BlockId(0)
                || record.site.operation != 0
            {
                return SimulationDebugSinkControlV1::Continue;
            }
            let SimulationDebugRecordKindV1::Checkpoint { phase, stack, .. } = record.kind else {
                panic!("LaneId must emit checkpoints, not a memory/barrier event");
            };
            let SimulationDebugCollectionV1::Captured(frames) = stack else {
                panic!("LaneId stack must be present");
            };
            let [frame] = frames.as_slice() else {
                panic!("one actual root frame");
            };
            let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
                panic!("LaneId bindings must be present");
            };
            let lane = record.invocation.local[0];
            assert!(lane < 64);
            let bit = 1u64 << lane;
            let mut matching = values
                .iter()
                .filter(|binding| binding.value == ValueId(200));
            match phase {
                SimulationDebugCheckpointPhaseV1::BeforeOperation => {
                    assert!(matching.next().is_none());
                    assert_eq!(self.before & bit, 0);
                    self.before |= bit;
                }
                SimulationDebugCheckpointPhaseV1::AfterOperation => {
                    let binding = matching.next().expect("actual LaneId result");
                    assert_eq!(
                        binding.observed,
                        SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(lane)),
                    );
                    assert!(matching.next().is_none());
                    assert_ne!(self.before & bit, 0);
                    assert_eq!(self.after & bit, 0);
                    self.after |= bit;
                }
            }
            SimulationDebugSinkControlV1::Continue
        }
    }
    let mut graph = fixture::module(64);
    graph.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(200), Type::Scalar(ScalarType::U32)),
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::LaneId,
                    WaveWidth::Wave64,
                )),
            ),
        );
    let caps = graph.functions[0].derived_capabilities();
    graph.functions[0].required_capabilities = caps.clone();
    graph.kernels[0].required_capabilities = caps.clone();
    graph.required_capabilities = caps;
    let (owner, view, floor, request) = prepared_module(graph);
    let mut work = Work::new(1usize << 54);
    let mut budget = Budget::new(&mut work, 1usize << 31);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(23).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let expected = fixture::oracle(&[1; 256], &[1; 256], &[0; 256]);
    let mut lanes = LaneCheck {
        before: 0,
        after: 0,
    };
    view.with_v12_cpu_observation_v1(
        V12CpuObservationInputV1::new(&owner, &request),
        V12CpuObservationOptionsV1::default(),
        &mut budget,
        (&mut NoopSimulationEventSinkV1, &mut lanes),
        |result, meter| {
            fixture::check_output(result.unwrap(), &expected, 1);
            assert!(meter.work_ledger_identity_v1() == identity);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(lanes.before, u64::MAX);
    assert_eq!(lanes.after, u64::MAX);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 23);
    assert!(budget.work_ledger_identity_v1() == identity);
}
