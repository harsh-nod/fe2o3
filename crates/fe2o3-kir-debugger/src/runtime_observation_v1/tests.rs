//! Interpreter-backed tests over synthetic KIR, not ordinary-source acceptance.
use super::*;
use crate::{
    DebugNavigationV1, DebugStopReasonV1, DebugTranscriptCompletenessV1, DebugWaveWidthV1,
    DebuggerLimitsV1,
};
use fe2o3_kir_sim::*;
use std::collections::{BTreeMap, BTreeSet};
#[path = "allocation_tests.rs"]
mod allocation_tests;
#[path = "fixtures_tests.rs"]
mod fixtures;
#[path = "retention_tests.rs"]
mod retention_tests;

fn options() -> RuntimeObservationOptionsV1 {
    RuntimeObservationOptionsV1::new(
        RuntimeOriginCaptureModeV1::Enabled(
            RuntimeOriginCaptureLimitsV1::new(4096, 256 * 1024).unwrap(),
        ),
        RuntimeFrameCaptureModeV1::Enabled(
            RuntimeFrameCaptureLimitsV1::new(4096, 16_384, 3 * 1024 * 1024).unwrap(),
        ),
        RuntimeAllocationCaptureModeV1::Disabled,
        None,
    )
    .unwrap()
}
fn capture_limits() -> SimulationDebugCaptureLimitsV1 {
    SimulationDebugCaptureLimitsV1::new(8, 64, 8, 256).unwrap()
}
fn run(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    config: RuntimeObservationOptionsV1,
) -> DebugObservedRunV1 {
    capture_debugger_observed_run_v1(
        module,
        request,
        fixtures::TARGET,
        fixtures::simulation_limits(),
        capture_limits(),
        fixtures::debugger_limits(4096),
        DebugWaveWidthV1::Wave64,
        config,
    )
    .unwrap()
}
fn work() -> RuntimeReplayWorkV1 {
    RuntimeReplayWorkV1::new(1_000_000_000).unwrap()
}
fn before(record: &SimulationDebugRecordV1) -> bool {
    matches!(
        record.kind,
        SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::BeforeOperation,
            ..
        }
    )
}
fn after(record: &SimulationDebugRecordV1) -> bool {
    matches!(
        record.kind,
        SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            ..
        }
    )
}

#[test]
fn real_nested_reused_and_empty_helpers_have_complete_actual_rosters() {
    let (module, request) = fixtures::loops();
    let run = run(&module, &request, options());
    assert!(run.execution().is_ok());
    let owner = run.transcript();
    assert_eq!(owner.origin_coverage(), Coverage::Complete);
    assert_eq!(owner.frame_coverage(), Coverage::Complete);
    let mut activations = BTreeMap::<_, BTreeSet<_>>::new();
    let mut saw_nested = false;
    let mut saw_after_next_difference = false;
    let mut caller_values = BTreeMap::new();
    for (index, record) in owner.legacy().records().iter().enumerate() {
        let frames = owner.frames_at(index).unwrap();
        assert_eq!(frames.record(), record);
        let origin = owner.origin_at(index).unwrap();
        let root = frames.get(0).unwrap();
        assert_eq!(root.activation(), 1);
        assert_eq!(root.parent(), SimulationDebugFrameParentV1::Root);
        let top = frames.get(frames.len() - 1).unwrap();
        assert_eq!(top.activation(), origin.activation());
        assert!(
            matches!(top.operation_state(), SimulationDebugFrameOperationV1::ActiveOperation { attempt, site }
            if attempt == origin.attempt() && site == record.site)
        );
        if after(record) && top.legacy().next_operation != Some(record.site.operation) {
            saw_after_next_difference = true;
        }
        if before(record) && record.site.function_ordinal == 1 {
            activations
                .entry(record.invocation.global)
                .or_default()
                .insert(top.activation());
        }
        for depth in 1..frames.len() {
            let caller = frames.get(depth - 1).unwrap();
            let child = frames.get(depth).unwrap();
            assert!(
                matches!(child.parent(), SimulationDebugFrameParentV1::Caller { activation, attempt, call_site }
                if activation == caller.activation() && matches!(caller.operation_state(),
                    SimulationDebugFrameOperationV1::Suspended { attempt: pending, site }
                    if pending == attempt && site == call_site))
            );
        }
        if frames.len() == 3 {
            saw_nested = true;
            let caller = frames.get(0).unwrap();
            let key = (record.invocation.global, top.activation());
            match caller_values.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(caller.legacy().values.clone());
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    assert_eq!(entry.get(), &caller.legacy().values)
                }
            }
        }
    }
    assert!(saw_nested && saw_after_next_difference);
    assert_eq!(activations.len(), 2);
    for values in activations.values() {
        // Empty helpers consume 5/9/13; they do not invent checkpoint frames.
        assert_eq!(values.iter().copied().collect::<Vec<_>>(), [2, 6, 10]);
    }
}

#[test]
fn fresh_owners_keep_equal_runtime_numbers_but_never_share_owner_tags() {
    let (module, request) = fixtures::loops();
    let (_, first) = run(&module, &request, options()).into_parts();
    let (_, second) = run(&module, &request, options()).into_parts();
    assert_ne!(first.capture_instance(), second.capture_instance());
    assert_eq!(first.legacy(), second.legacy());
    assert_eq!(
        first.origin_at(0).unwrap().activation(),
        second.origin_at(0).unwrap().activation()
    );
    let owner = first.capture_instance();
    assert_eq!(first.into_session().capture_instance(), owner);
}

#[test]
fn disabled_and_origin_only_match_legacy_capture_and_old_origin_callback() {
    let (module, request) = fixtures::loops();
    let legacy = crate::capture_debugger_run_v1(
        &module,
        &request,
        fixtures::TARGET,
        fixtures::simulation_limits(),
        capture_limits(),
        fixtures::debugger_limits(4096),
        DebugWaveWidthV1::Wave64,
    );
    let disabled = run(&module, &request, RuntimeObservationOptionsV1::disabled());
    fixtures::assert_result_eq(disabled.execution(), &legacy.execution);
    assert_eq!(disabled.transcript().legacy(), &legacy.transcript);
    assert_eq!(
        disabled.transcript().origin_metadata_usage().capacity_rows,
        0
    );
    assert_eq!(
        disabled.transcript().frame_metadata_usage().frame_capacity,
        0
    );
    assert_eq!(
        disabled
            .transcript()
            .allocation_metadata_usage()
            .transition_capacity,
        0
    );
    assert_eq!(
        disabled.transcript().origin_at(0).unwrap_err(),
        Missing::Disabled
    );
    let origin_only = RuntimeObservationOptionsV1::new(
        options().origins(),
        RuntimeFrameCaptureModeV1::Disabled,
        RuntimeAllocationCaptureModeV1::Disabled,
        None,
    )
    .unwrap();
    let observed = run(&module, &request, origin_only);
    fixtures::assert_result_eq(observed.execution(), &legacy.execution);
    assert_eq!(observed.transcript().legacy(), &legacy.transcript);

    #[derive(Default)]
    struct OldSink {
        records: Vec<SimulationDebugRecordV1>,
        origins: Vec<SimulationDebugOriginContextV1>,
    }
    impl SimulationDebugSinkV1 for OldSink {
        fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
            panic!("old opt-in route lost")
        }
        fn wants_operation_origin_v1(&self) -> bool {
            true
        }
        fn record_with_operation_origin_v1(
            &mut self,
            record: SimulationDebugRecordV1,
            origin: SimulationDebugOriginContextV1,
        ) -> SimulationDebugSinkControlV1 {
            self.records.push(record);
            self.origins.push(origin);
            SimulationDebugSinkControlV1::Continue
        }
    }
    let mut sink = OldSink::default();
    let result = module.simulate_debugged_with_sink(
        &request,
        fixtures::TARGET,
        fixtures::simulation_limits(),
        capture_limits(),
        &mut sink,
    );
    fixtures::assert_result_eq(&result, &legacy.execution);
    assert_eq!(sink.records, legacy.transcript.records());
    for (index, context) in sink.origins.iter().enumerate() {
        let SimulationDebugOriginContextV1::Available(origin) = context else {
            panic!("expected operation");
        };
        let retained = observed.transcript().origin_at(index).unwrap();
        assert_eq!(origin.activation(), retained.activation());
        assert_eq!(origin.attempt(), retained.attempt());
    }
}

#[test]
fn scheduled_public_capture_preserves_canonical_seeded_and_exact_replay() {
    let (module, request) = fixtures::barrier();
    for seed in [None, Some(71)] {
        let schedule = match seed {
            None => SimulationScheduleRequestV1::RecordCanonical { max_decisions: 512 },
            Some(seed) => SimulationScheduleRequestV1::RecordSeeded {
                seed,
                max_decisions: 512,
            },
        };
        let run = capture_debugger_observed_scheduled_run_v1(
            &module,
            &request,
            fixtures::TARGET,
            fixtures::simulation_limits(),
            capture_limits(),
            fixtures::debugger_limits(4096),
            DebugWaveWidthV1::Wave64,
            options(),
            schedule,
        )
        .unwrap();
        let result = run.execution().as_ref().unwrap();
        let schedule = result.schedule_record().unwrap();
        let replay = capture_debugger_observed_scheduled_run_v1(
            &module,
            &request,
            fixtures::TARGET,
            fixtures::simulation_limits(),
            capture_limits(),
            fixtures::debugger_limits(4096),
            DebugWaveWidthV1::Wave64,
            options(),
            SimulationScheduleRequestV1::Replay(schedule),
        )
        .unwrap();
        // Recording owns a new schedule artifact; replay consumes the supplied
        // artifact and does not retain a second copy. Compare each route with
        // its matching legacy route, then compare every semantic execution field.
        let mut legacy_collector =
            crate::TranscriptCollectorV1::new(fixtures::debugger_limits(4096));
        let legacy_recorded = module.simulate_debugged_scheduled_with_sink(
            &request,
            fixtures::TARGET,
            fixtures::simulation_limits(),
            match seed {
                None => SimulationScheduleRequestV1::RecordCanonical { max_decisions: 512 },
                Some(seed) => SimulationScheduleRequestV1::RecordSeeded {
                    seed,
                    max_decisions: 512,
                },
            },
            capture_limits(),
            &mut legacy_collector,
        );
        fixtures::assert_result_eq(run.execution(), &legacy_recorded);
        let legacy_replayed = crate::capture_debugger_replayed_run_v1(
            &module,
            &request,
            fixtures::TARGET,
            fixtures::simulation_limits(),
            capture_limits(),
            fixtures::debugger_limits(4096),
            DebugWaveWidthV1::Wave64,
            schedule,
        );
        fixtures::assert_result_eq(replay.execution(), &legacy_replayed.execution);
        let replayed = replay.execution().as_ref().unwrap();
        assert!(result.schedule_record().is_some());
        assert!(replayed.schedule_record().is_none());
        assert_eq!(result.identity(), replayed.identity());
        assert_eq!(
            result.dynamic_workgroup_memory(),
            replayed.dynamic_workgroup_memory()
        );
        assert_eq!(result.arguments(), replayed.arguments());
        assert_eq!(result.shared_buffers(), replayed.shared_buffers());
        assert_eq!(
            result.invocations_executed(),
            replayed.invocations_executed()
        );
        assert_eq!(result.workgroups_visited(), replayed.workgroups_visited());
        assert_eq!(
            result.scheduled_slots_visited(),
            replayed.scheduled_slots_visited()
        );
        assert_eq!(result.steps_executed(), replayed.steps_executed());
        assert_eq!(result.events_emitted(), replayed.events_emitted());
        assert_eq!(result.schedule(), replayed.schedule());
        assert_eq!(
            result.schedule_transcript_identity(),
            replayed.schedule_transcript_identity()
        );
        assert_eq!(result.schedule_coverage(), replayed.schedule_coverage());
        assert_eq!(result.conflict_assessment(), replayed.conflict_assessment());
        assert_eq!(result.race_assessment(), replayed.race_assessment());
        assert_eq!(run.transcript().legacy(), replay.transcript().legacy());
        let mut releases = 0;
        for (index, record) in run.transcript().legacy().records().iter().enumerate() {
            if matches!(
                record.kind,
                SimulationDebugRecordKindV1::WorkgroupBarrier {
                    action: SimulationDebugBarrierActionV1::Release,
                    ..
                }
            ) {
                releases += 1;
                assert_eq!(
                    run.transcript().origin_at(index).unwrap_err(),
                    Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)
                );
                assert_eq!(
                    run.transcript().frames_at(index).unwrap_err(),
                    RuntimeFrameMissingV1::RuntimeUnavailable(
                        SimulationDebugFrameOriginUnavailableV1::NotCheckpoint
                    )
                );
            }
        }
        assert_eq!(releases, 2);
    }
}

#[test]
fn memory_watch_record_has_real_origin_but_never_borrows_a_checkpoint_roster() {
    let (module, request) = fixtures::memory();
    let run = run(&module, &request, options());
    let owner = run.transcript();
    let mut count = 0;
    for (index, record) in owner.legacy().records().iter().enumerate() {
        if matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. }) {
            count += 1;
            assert!(owner.origin_at(index).is_ok());
            assert_eq!(
                owner.frames_at(index).unwrap_err(),
                RuntimeFrameMissingV1::RuntimeUnavailable(
                    SimulationDebugFrameOriginUnavailableV1::NotCheckpoint
                )
            );
        }
    }
    assert_eq!(count, 2);
}

#[test]
fn dynamic_step_out_over_reverse_and_repeated_selection_use_actual_keys() {
    let (module, request) = fixtures::loops();
    let (_, owner) = run(&module, &request, options()).into_parts();
    let first_leaf = owner
        .legacy()
        .records()
        .iter()
        .enumerate()
        .find_map(|(index, record)| {
            (before(record) && record.site.function_ordinal == 2).then_some(index)
        })
        .unwrap();
    let focus = owner.legacy().records()[first_leaf].invocation;
    let parent = owner
        .frames_at(first_leaf)
        .unwrap()
        .get(2)
        .unwrap()
        .parent();
    let SimulationDebugFrameParentV1::Caller {
        activation,
        attempt,
        call_site,
    } = parent
    else {
        panic!()
    };
    let parent_before = (0..first_leaf)
        .rev()
        .find(|index| {
            let row = owner.origin_at(*index).unwrap();
            before(row.record())
                && row.invocation() == focus
                && row.activation() == activation
                && row.attempt() == attempt
                && row.site() == call_site
        })
        .unwrap();
    let parent_after = owner
        .paired_after(
            parent_before,
            &mut RuntimeOriginScanWorkV1::new(4096).unwrap(),
        )
        .unwrap();
    let second_helper = owner
        .legacy()
        .records()
        .iter()
        .enumerate()
        .find_map(|(index, record)| {
            let origin = owner.origin_at(index).ok()?;
            (before(record)
                && record.invocation == focus
                && record.site.function_ordinal == 1
                && origin.activation() == 6)
                .then_some(index)
        })
        .unwrap();
    let mut session = owner.into_session();
    let mut work = work();
    session.seek_record(first_leaf, &mut work).unwrap();
    session
        .step_out(RuntimeNavigationDirectionV1::Forward, focus, &mut work)
        .unwrap();
    assert_eq!(session.legacy().cursor_record_index(), Some(parent_after));
    session.seek_record(first_leaf, &mut work).unwrap();
    session
        .step_out(RuntimeNavigationDirectionV1::Reverse, focus, &mut work)
        .unwrap();
    assert_eq!(session.legacy().cursor_record_index(), Some(parent_before));
    session
        .step_over(RuntimeNavigationDirectionV1::Forward, focus, &mut work)
        .unwrap();
    assert_eq!(session.legacy().cursor_record_index(), Some(parent_after));
    session
        .step_over(RuntimeNavigationDirectionV1::Reverse, focus, &mut work)
        .unwrap();
    assert_eq!(session.legacy().cursor_record_index(), Some(parent_before));
    session.seek_record(second_helper, &mut work).unwrap();
    assert!(session.current_frames().unwrap().by_activation(2).is_none());
    assert!(session.current_frames().unwrap().by_activation(6).is_some());
    session.seek_record(first_leaf, &mut work).unwrap();
    assert!(session.current_frames().unwrap().by_activation(2).is_some());
    let old_cursor = session.legacy().cursor_record_index();
    assert_eq!(
        session.step_out(
            RuntimeNavigationDirectionV1::Forward,
            focus,
            &mut RuntimeReplayWorkV1::new(0).unwrap()
        ),
        Err(RuntimeSessionErrorV1::WorkLimit)
    );
    assert_eq!(session.legacy().cursor_record_index(), old_cursor);
}

#[test]
fn terminal_fault_keeps_unpaired_attempt_and_current_queries_explicitly_absent() {
    let (module, request) = fixtures::memory();
    let limits = SimulationLimitsV1 {
        max_steps: 1,
        ..fixtures::simulation_limits()
    };
    let run = capture_debugger_observed_run_v1(
        &module,
        &request,
        fixtures::TARGET,
        limits,
        capture_limits(),
        fixtures::debugger_limits(4096),
        DebugWaveWidthV1::Wave64,
        options(),
    )
    .unwrap();
    assert!(run.execution().is_err());
    let (_, owner) = run.into_parts();
    let len = owner.legacy().records().len();
    assert!(owner.legacy().terminal_fault().is_some());
    assert_eq!(
        owner.paired_after(len - 1, &mut RuntimeOriginScanWorkV1::new(4096).unwrap()),
        Err(RuntimeOriginPairErrorV1::MissingAfter)
    );
    let focus = owner.legacy().records()[len - 1].invocation;
    let mut session = owner.into_session();
    let navigation = session.seek_record(len, &mut work()).unwrap();
    assert!(
        matches!(navigation, DebugNavigationV1::Stopped(stop) if stop.reason == DebugStopReasonV1::Fault)
    );
    assert_eq!(
        session.current_origin().unwrap_err(),
        Missing::NoCurrentRecord
    );
    assert_eq!(
        session.current_frames().unwrap_err(),
        RuntimeFrameMissingV1::NoCurrentRecord
    );
    session
        .step_into(RuntimeNavigationDirectionV1::Reverse, focus, &mut work())
        .unwrap();
    assert_eq!(session.legacy().cursor_record_index(), Some(len - 1));
    assert!(session.current_frames().is_ok());
}
