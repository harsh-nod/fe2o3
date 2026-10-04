//! Exact live-at-record resource joins over actual allocator reuse.
use super::*;

fn allocation_options(limits: RuntimeAllocationCaptureLimitsV1) -> RuntimeObservationOptionsV1 {
    RuntimeObservationOptionsV1::new(
        options().origins(),
        options().frames(),
        RuntimeAllocationCaptureModeV1::Enabled(limits),
        Some(SimulationAllocationReuseV1::exact_private_and_workgroup(8192).unwrap()),
    )
    .unwrap()
}
fn full_allocation_options() -> RuntimeObservationOptionsV1 {
    allocation_options(RuntimeAllocationCaptureLimitsV1::new(4096, 64, 1024 * 1024).unwrap())
}

#[test]
fn real_slot_reuse_preserves_historical_incarnations_and_resets_snapshot_initialization() {
    let (module, request) = fixtures::private_allocations();
    let (_, owner) = run(&module, &request, full_allocation_options()).into_parts();
    assert_eq!(owner.allocation_coverage(), Coverage::Complete);
    let final_index = owner.legacy().records().len() - 1;
    let transitions = owner.allocations_at(final_index).unwrap().transitions();
    assert_eq!(transitions.len(), 4);
    let first = transitions[0].descriptor().identity();
    let second = transitions[2].descriptor().identity();
    assert_ne!(first.allocation(), second.allocation());
    assert_eq!(first.storage_slot(), second.storage_slot());
    assert_eq!(first.generation().checked_add(1), Some(second.generation()));
    assert!(
        matches!(transitions[2].kind(), SimulationAllocationTransitionKindV1::Create {
        previous_allocation: Some(previous) } if previous == first.allocation())
    );
    let find_snapshot = |allocation| {
        owner
            .legacy()
            .records()
            .iter()
            .enumerate()
            .find_map(|(index, record)| {
                let SimulationDebugRecordKindV1::Checkpoint {
                    memory: SimulationDebugCollectionV1::Captured(memory),
                    ..
                } = &record.kind
                else {
                    return None;
                };
                memory
                    .iter()
                    .find(|value| value.allocation == allocation)
                    .map(|value| (index, value))
            })
            .unwrap()
    };
    let (first_index, _) = find_snapshot(first.allocation());
    let (second_index, snapshot) = find_snapshot(second.allocation());
    assert_eq!(snapshot.bytes, [0; 4]);
    assert_eq!(snapshot.initialized, [false; 4]);
    let old = owner.allocations_at(first_index).unwrap();
    let new = owner.allocations_at(second_index).unwrap();
    assert_eq!(
        old.descriptor(first.allocation(), &mut work())
            .unwrap()
            .identity(),
        first
    );
    assert_eq!(
        old.descriptor(second.allocation(), &mut work()),
        Err(RuntimeAllocationMissingV1::NotLive)
    );
    assert_eq!(
        new.descriptor(second.allocation(), &mut work())
            .unwrap()
            .identity(),
        second
    );
    assert_eq!(
        new.descriptor(first.allocation(), &mut work()),
        Err(RuntimeAllocationMissingV1::NotLive)
    );
    assert_eq!(
        new.descriptor(
            second.allocation(),
            &mut RuntimeReplayWorkV1::new(0).unwrap()
        ),
        Err(RuntimeAllocationMissingV1::WorkLimit)
    );
    let final_state = owner.allocations_at(final_index).unwrap();
    assert_eq!(
        final_state.descriptor(first.allocation(), &mut work()),
        Err(RuntimeAllocationMissingV1::NotLive)
    );
    assert_eq!(
        final_state.descriptor(second.allocation(), &mut work()),
        Err(RuntimeAllocationMissingV1::NotLive)
    );

    let mut session = owner.into_session();
    session.seek_record(first_index, &mut work()).unwrap();
    session
        .add_watchpoint(
            crate::DebugWatchpointV1 {
                id: 1,
                allocation: first.allocation(),
                byte_offset: 0,
                byte_len: 4,
                access: crate::DebugWatchAccessV1::Write,
                scope: crate::DebugScopeSelectorV1::Dispatch,
                value_equals: None,
                enabled: true,
            },
            &mut work(),
        )
        .unwrap();
    assert!(
        matches!(session.continue_to_stop(RuntimeNavigationDirectionV1::Forward, &mut work()).unwrap(),
        DebugNavigationV1::Stopped(stop) if stop.reason == DebugStopReasonV1::Watchpoint(1))
    );
    assert!(matches!(session.legacy().current().unwrap().kind,
        SimulationDebugRecordKindV1::Memory { allocation, .. } if allocation == first.allocation()));
    assert_eq!(
        session.current_frames().unwrap_err(),
        RuntimeFrameMissingV1::RuntimeUnavailable(
            SimulationDebugFrameOriginUnavailableV1::NotCheckpoint
        )
    );
    assert!(session.current_origin().is_ok());
    assert_eq!(
        session
            .continue_to_stop(RuntimeNavigationDirectionV1::Forward, &mut work())
            .unwrap(),
        DebugNavigationV1::End
    );
    assert_eq!(
        session.legacy().watchpoint_hit_count(1),
        Some(1),
        "old watch must not follow reused storage"
    );
    session.seek_record(first_index, &mut work()).unwrap();
    assert_eq!(
        session
            .current_allocations()
            .unwrap()
            .descriptor(first.allocation(), &mut work())
            .unwrap()
            .identity(),
        first
    );
    session.seek_record(second_index, &mut work()).unwrap();
    assert_eq!(
        session
            .current_allocations()
            .unwrap()
            .descriptor(second.allocation(), &mut work())
            .unwrap()
            .identity(),
        second
    );
}

#[test]
fn lifecycle_capture_without_reuse_reports_policy_disabled_not_fake_generation() {
    let (module, request) = fixtures::memory();
    let config = RuntimeObservationOptionsV1::new(
        options().origins(),
        options().frames(),
        RuntimeAllocationCaptureModeV1::Enabled(
            RuntimeAllocationCaptureLimitsV1::new(4096, 64, 1024 * 1024).unwrap(),
        ),
        None,
    )
    .unwrap();
    let run = run(&module, &request, config);
    assert!(run.execution().is_ok());
    assert_eq!(run.transcript().allocation_coverage(), Coverage::Complete);
    assert_eq!(
        run.transcript()
            .allocation_metadata_usage()
            .retained_transitions,
        0
    );
    assert_eq!(
        run.transcript().allocations_at(0).unwrap_err(),
        RuntimeAllocationMissingV1::RuntimeUnavailable(
            SimulationAllocationWatermarkV1::PolicyDisabled
        )
    );
}

#[test]
fn actual_preexisting_buffer_is_visible_without_claiming_dispatch_teardown() {
    let (module, request) = fixtures::memory();
    let run = run(&module, &request, full_allocation_options());
    assert!(run.execution().is_ok());
    let owner = run.transcript();
    let last = owner
        .allocations_at(owner.legacy().records().len() - 1)
        .unwrap();
    assert_eq!(last.transitions().len(), 1);
    let transition = last.transitions()[0];
    assert_eq!(
        transition.kind(),
        SimulationAllocationTransitionKindV1::Preexisting
    );
    assert_eq!(transition.descriptor().creation_site(), None);
    assert_eq!(
        transition.descriptor().scope(),
        SimulationAllocationScopeV1::Dispatch
    );
    assert!(
        last.descriptor(transition.descriptor().identity().allocation(), &mut work())
            .is_ok()
    );
}

#[test]
fn lifecycle_transition_and_validation_work_cutoffs_preserve_only_the_true_prefix() {
    let (module, request) = fixtures::private_allocations();
    for (limits, expected) in [
        (
            RuntimeAllocationCaptureLimitsV1::new(4096, 1, 1024 * 1024).unwrap(),
            Cutoff::TransitionLimit,
        ),
        (
            RuntimeAllocationCaptureLimitsV1::new_with_validation_work(4096, 64, 1024 * 1024, 1)
                .unwrap(),
            Cutoff::ValidationWorkLimit,
        ),
    ] {
        let run = run(&module, &request, allocation_options(limits));
        assert!(run.execution().is_ok());
        let owner = run.transcript();
        assert_eq!(
            owner.allocation_coverage(),
            Coverage::PrefixTruncated(expected)
        );
        assert_eq!(owner.allocation_metadata_usage().retained_transitions, 1);
        assert_eq!(owner.allocation_metadata_usage().validation_work_used, 1);
        assert!(
            owner.allocations_at(0).is_ok(),
            "valid earlier empty prefix remains usable"
        );
        let last = owner.legacy().records().len() - 1;
        assert_eq!(
            owner.allocations_at(last).unwrap_err(),
            RuntimeAllocationMissingV1::PrefixTruncated(expected)
        );
        assert!(owner.origin_at(last).is_ok());
        assert!(owner.frames_at(last).is_ok());
        assert_eq!(
            owner.legacy().completeness(),
            DebugTranscriptCompletenessV1::Complete
        );
    }
}
