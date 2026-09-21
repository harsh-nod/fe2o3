//! Actual interpreter transitions over synthetic KIR controls. These tests do
//! not establish ordinary-source lowering, source ownership, or GPU behavior.

use fe2o3_kernel_ir::BlockId;
use fe2o3_kir_sim::*;
use std::collections::{BTreeMap, BTreeSet};

#[path = "debug_runtime_origin/fixtures.rs"]
mod fixtures;
use fixtures::*;

const MAX_ROWS: usize = 16_384;

struct Capture {
    requested: bool,
    rows: Vec<(SimulationDebugRecordV1, SimulationDebugOriginContextV1)>,
    stop: Option<SimulationDebugSinkControlV1>,
}

impl Capture {
    fn new(requested: bool) -> Self {
        Self {
            requested,
            rows: Vec::new(),
            stop: None,
        }
    }

    fn retain(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: SimulationDebugOriginContextV1,
    ) -> SimulationDebugSinkControlV1 {
        assert!(self.rows.len() < MAX_ROWS, "bounded synthetic test capture");
        self.rows.push((record, origin));
        self.stop.unwrap_or(SimulationDebugSinkControlV1::Continue)
    }
}

impl SimulationDebugSinkV1 for Capture {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(
            !self.requested,
            "opt-in runtime must use contextual callback"
        );
        self.retain(
            record,
            SimulationDebugOriginContextV1::Unavailable(
                SimulationDebugOriginUnavailableV1::NotRequested,
            ),
        )
    }

    fn wants_operation_origin_v1(&self) -> bool {
        self.requested
    }

    fn record_with_operation_origin_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: SimulationDebugOriginContextV1,
    ) -> SimulationDebugSinkControlV1 {
        assert!(self.requested);
        self.retain(record, origin)
    }
}

fn capture_limits() -> SimulationDebugCaptureLimitsV1 {
    // Each record has bounded snapshot storage; the collector separately caps
    // record/context count. V1 value/byte limits do not charge origin contexts.
    SimulationDebugCaptureLimitsV1::new(8, 32, 4, 64).unwrap()
}

fn run(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    capture: &mut impl SimulationDebugSinkV1,
) -> SimulationExecutionV1 {
    module
        .simulate_debugged_with_sink(
            request,
            TARGET,
            SimulationLimitsV1::default(),
            capture_limits(),
            capture,
        )
        .unwrap()
}

fn available(origin: SimulationDebugOriginContextV1) -> SimulationDebugOperationOriginV1 {
    match origin {
        SimulationDebugOriginContextV1::Available(origin) => origin,
        other => panic!("one actual operation expected: {other:?}"),
    }
}

fn phase(record: &SimulationDebugRecordV1) -> Option<SimulationDebugCheckpointPhaseV1> {
    match record.kind {
        SimulationDebugRecordKindV1::Checkpoint { phase, .. } => Some(phase),
        _ => None,
    }
}

fn assert_complete_attempts(capture: &Capture) {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    let mut last_attempt = BTreeMap::new();
    for (record, context) in &capture.rows {
        if matches!(
            record.kind,
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action: SimulationDebugBarrierActionV1::Release,
                ..
            }
        ) {
            assert_eq!(
                *context,
                SimulationDebugOriginContextV1::Unavailable(
                    SimulationDebugOriginUnavailableV1::AggregateRecord
                )
            );
            continue;
        }
        let origin = available(*context);
        assert_eq!(origin.invocation(), record.invocation);
        assert_eq!(origin.site(), record.site);
        assert!(origin.activation() > 0 && origin.attempt() > 0);
        if phase(record) == Some(SimulationDebugCheckpointPhaseV1::BeforeOperation) {
            let prior = last_attempt
                .entry((origin.invocation(), origin.activation()))
                .or_insert(0);
            assert_eq!(origin.attempt(), *prior + 1);
            *prior = origin.attempt();
        }
        groups
            .entry((origin.invocation(), origin.activation(), origin.attempt()))
            .or_default()
            .push(record);
    }
    assert!(!groups.is_empty());
    for records in groups.values() {
        assert_eq!(
            phase(records[0]),
            Some(SimulationDebugCheckpointPhaseV1::BeforeOperation)
        );
        assert_eq!(
            phase(records[records.len() - 1]),
            Some(SimulationDebugCheckpointPhaseV1::AfterOperation)
        );
        assert_eq!(
            records
                .iter()
                .filter(|record| phase(record).is_some())
                .count(),
            2
        );
        assert!(records.iter().all(|record| record.site == records[0].site));
    }
}

#[test]
fn synthetic_loops_nested_calls_empty_helpers_and_reused_slots_have_runtime_origins() {
    let module = loops_and_helpers();
    let request = SimulationRequestV1::new("kernel", [2, 1, 1], [1, 1, 1], vec![]);
    let mut capture = Capture::new(true);
    let result = run(&module, &request, &mut capture);
    assert_eq!(result.invocations_executed(), 2);
    assert_complete_attempts(&capture);
    for x in 0..2 {
        let before = capture.rows.iter().filter(|(record, _)| {
            record.invocation.global[0] == x
                && phase(record) == Some(SimulationDebugCheckpointPhaseV1::BeforeOperation)
        });
        let mut activations = BTreeMap::<usize, BTreeSet<u64>>::new();
        let mut outer_calls = vec![];
        for (record, context) in before {
            let origin = available(*context);
            activations
                .entry(record.site.function_ordinal)
                .or_default()
                .insert(origin.activation());
            if record.site.function_ordinal == 0
                && record.site.block == BlockId(1)
                && record.site.operation == 0
            {
                outer_calls.push(origin.attempt());
            }
        }
        assert_eq!(activations[&0], BTreeSet::from([1]));
        assert_eq!(activations[&1], BTreeSet::from([2, 6, 10]));
        assert_eq!(activations[&2], BTreeSet::from([3, 4, 7, 8, 11, 12]));
        // Empty helpers consume real activations 5/9/13 without fabricated records.
        assert!(!activations.contains_key(&3));
        assert_eq!(outer_calls, vec![4, 8, 12]);
    }
}

#[test]
fn synthetic_memory_and_fence_share_before_after_attempts() {
    let (module, request) = memory_and_fence();
    let mut capture = Capture::new(true);
    run(&module, &request, &mut capture);
    assert_complete_attempts(&capture);
    assert_eq!(capture.rows.len(), 11);
    assert_eq!(
        capture
            .rows
            .iter()
            .filter(|(record, _)| matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. }))
            .count(),
        2
    );
    assert_eq!(
        capture
            .rows
            .iter()
            .filter(|(record, _)| matches!(record.kind, SimulationDebugRecordKindV1::Fence { .. }))
            .count(),
        1
    );
}

#[test]
fn synthetic_wave_and_barrier_yields_preserve_pending_helper_and_caller_attempts() {
    let module = yielding_helper();
    let request = SimulationRequestV1::new("kernel", [64, 1, 1], [64, 1, 1], vec![]);
    for schedule in [
        SimulationScheduleRequestV1::RecordCanonical {
            max_decisions: 4096,
        },
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 71,
            max_decisions: 4096,
        },
    ] {
        let mut capture = Capture::new(true);
        let mut legacy = Capture::new(false);
        let observed = module
            .simulate_debugged_scheduled_with_sink(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                schedule,
                capture_limits(),
                &mut capture,
            )
            .unwrap();
        let baseline = module
            .simulate_debugged_scheduled_with_sink(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                schedule,
                capture_limits(),
                &mut legacy,
            )
            .unwrap();
        assert_eq!(observed, baseline);
        assert_eq!(
            capture.rows.iter().map(|row| &row.0).collect::<Vec<_>>(),
            legacy.rows.iter().map(|row| &row.0).collect::<Vec<_>>()
        );
        assert_complete_attempts(&capture);
        let releases = capture
            .rows
            .iter()
            .filter(|(record, _)| {
                matches!(
                    record.kind,
                    SimulationDebugRecordKindV1::WorkgroupBarrier {
                        action: SimulationDebugBarrierActionV1::Release,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(releases, 2);
        for lane in 0..64 {
            let helper_activations = capture
                .rows
                .iter()
                .filter(|(record, _)| {
                    record.invocation.global[0] == lane
                        && record.site.function_ordinal == 1
                        && phase(record) == Some(SimulationDebugCheckpointPhaseV1::BeforeOperation)
                })
                .map(|(_, origin)| available(*origin).activation())
                .collect::<BTreeSet<_>>();
            assert_eq!(helper_activations, BTreeSet::from([2, 3]));
        }
    }
}

#[test]
fn synthetic_transpose_release_is_aggregate_even_at_the_representative_site() {
    let (module, request) = transpose();
    let mut capture = Capture::new(true);
    run(&module, &request, &mut capture);
    assert_complete_attempts(&capture);
    let releases = capture
        .rows
        .iter()
        .filter(|(record, _)| {
            matches!(
                record.kind,
                SimulationDebugRecordKindV1::WorkgroupBarrier {
                    action: SimulationDebugBarrierActionV1::Release,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(releases.len(), 1);
    let (release, context) = releases[0];
    assert_eq!(
        *context,
        SimulationDebugOriginContextV1::Unavailable(
            SimulationDebugOriginUnavailableV1::AggregateRecord
        )
    );
    assert_eq!(release.site.operation, 2);
    assert!(capture.rows.iter().any(|(record, context)| {
        record.invocation == release.invocation
            && record.site == release.site
            && matches!(context, SimulationDebugOriginContextV1::Available(_))
    }));
    // Cooperative stage/read memory belongs to each suspended lane's own
    // attempt, not the last runnable lane or representative publish record.
    assert!(
        capture
            .rows
            .iter()
            .any(|(record, _)| matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. }))
    );
}

#[test]
fn opting_in_does_not_change_legacy_records_results_or_sink_stop_semantics() {
    let (module, request) = memory_and_fence();
    let mut baseline = Capture::new(false);
    let expected = run(&module, &request, &mut baseline);
    for stop in [
        None,
        Some(SimulationDebugSinkControlV1::Stop),
        Some(SimulationDebugSinkControlV1::DropAndStop),
    ] {
        let mut capture = Capture::new(true);
        capture.stop = stop;
        assert_eq!(run(&module, &request, &mut capture), expected);
        let count = if stop.is_some() {
            1
        } else {
            baseline.rows.len()
        };
        assert_eq!(capture.rows.len(), count);
        assert_eq!(
            capture.rows.iter().map(|row| &row.0).collect::<Vec<_>>(),
            baseline.rows[..count]
                .iter()
                .map(|row| &row.0)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn default_context_adapter_calls_legacy_sink_exactly_once_and_disabled_capture_stays_disabled() {
    #[derive(Default)]
    struct Adapted(Vec<SimulationDebugRecordV1>);
    impl SimulationDebugSinkV1 for Adapted {
        fn wants_operation_origin_v1(&self) -> bool {
            true
        }
        fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
            assert!(self.0.len() < MAX_ROWS);
            self.0.push(record);
            SimulationDebugSinkControlV1::Continue
        }
    }
    let (module, request) = memory_and_fence();
    let mut baseline = Capture::new(false);
    let expected = run(&module, &request, &mut baseline);
    let mut adapted = Adapted::default();
    assert_eq!(run(&module, &request, &mut adapted), expected);
    assert_eq!(
        adapted.0,
        baseline
            .rows
            .into_iter()
            .map(|row| row.0)
            .collect::<Vec<_>>()
    );
    let mut disabled = Capture::new(true);
    assert_eq!(
        module
            .simulate_debugged_with_sink(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                SimulationDebugCaptureLimitsV1::disabled(),
                &mut disabled
            )
            .unwrap(),
        expected
    );
    assert!(disabled.rows.is_empty());
}

#[test]
fn attempted_operation_is_not_falsely_reported_as_completed_on_step_failure() {
    let module = loops_and_helpers();
    let request = SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], vec![]);
    let limits = SimulationLimitsV1 {
        max_steps: 2,
        ..SimulationLimitsV1::default()
    };
    let mut observed = Capture::new(true);
    let mut legacy = Capture::new(false);
    let a = module.simulate_debugged_with_sink(
        &request,
        TARGET,
        limits,
        capture_limits(),
        &mut observed,
    );
    let b =
        module.simulate_debugged_with_sink(&request, TARGET, limits, capture_limits(), &mut legacy);
    match (a, b) {
        (
            Err(SimulationErrorV1::Execution(observed)),
            Err(SimulationErrorV1::Execution(legacy)),
        ) => {
            assert_eq!(observed, legacy);
            assert!(matches!(
                observed.kind,
                SimulationExecutionErrorKindV1::StepLimit { limit: 2 }
            ));
        }
        other => panic!("same dynamic step refusal expected: {other:?}"),
    }
    assert_eq!(
        observed.rows.iter().map(|row| &row.0).collect::<Vec<_>>(),
        legacy.rows.iter().map(|row| &row.0).collect::<Vec<_>>()
    );
    let (last, origin) = observed.rows.last().expect("attempt before refusal");
    assert_eq!(
        phase(last),
        Some(SimulationDebugCheckpointPhaseV1::BeforeOperation)
    );
    assert_eq!(available(*origin).site(), last.site);
}

#[test]
fn runtime_context_has_fixed_size_and_tokens_are_only_live_run_local() {
    assert!(std::mem::size_of::<SimulationDebugOriginContextV1>() <= 256);
    assert!(!std::mem::needs_drop::<SimulationDebugOriginContextV1>());
    let module = loops_and_helpers();
    let request = SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], vec![]);
    let mut first = Capture::new(true);
    let mut second = Capture::new(true);
    run(&module, &request, &mut first);
    run(&module, &request, &mut second);
    // Equal bytes in fresh runs are deliberate: no global/persisted/run owner
    // is minted by this API and external transcript owners cannot be omitted.
    assert_eq!(first.rows, second.rows);
}
