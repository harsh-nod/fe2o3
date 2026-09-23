//! Independent cutoffs, accepted-row joins and bounded controls.
use super::*;

#[test]
fn checked_limits_refuse_expansion_and_frames_require_origins() {
    assert!(RuntimeOriginCaptureLimitsV1::new(0, 4096).is_err());
    assert!(RuntimeOriginCaptureLimitsV1::new(MAX_ROWS + 1, 4096).is_err());
    assert!(RuntimeFrameCaptureLimitsV1::new(1, MAX_ROWS + 1, 4096).is_err());
    assert!(RuntimeAllocationCaptureLimitsV1::new(1, 65_537, 4096).is_err());
    assert!(RuntimeAllocationCaptureLimitsV1::new(1, 1, 16 * 1024 * 1024 + 1).is_err());
    assert!(
        RuntimeAllocationCaptureLimitsV1::new_with_validation_work(1, 1, 4096, 1_000_001).is_err()
    );
    assert_eq!(
        RuntimeAllocationCaptureLimitsV1::new(1, 1, 4096)
            .unwrap()
            .max_validation_work(),
        1_000_000
    );
    assert!(matches!(
        RuntimeObservationOptionsV1::new(
            RuntimeOriginCaptureModeV1::Disabled,
            options().frames(),
            RuntimeAllocationCaptureModeV1::Disabled,
            None
        ),
        Err(RuntimeObservationConfigErrorV1::FramesRequireOrigins)
    ));
}

#[test]
fn legacy_drop_accepts_no_orphan_origin_frame_or_watermark_row() {
    let (module, request) = fixtures::loops();
    let run = capture_debugger_observed_run_v1(
        &module,
        &request,
        fixtures::TARGET,
        fixtures::simulation_limits(),
        capture_limits(),
        fixtures::debugger_limits(1),
        DebugWaveWidthV1::Wave64,
        options(),
    )
    .unwrap();
    assert!(run.execution().is_ok());
    let owner = run.transcript();
    assert_eq!(owner.legacy().records().len(), 1);
    assert!(matches!(
        owner.legacy().completeness(),
        DebugTranscriptCompletenessV1::Truncated(_)
    ));
    assert_eq!(owner.origin_metadata_usage().retained_rows, 1);
    assert_eq!(owner.frame_metadata_usage().retained_records, 1);
    assert_eq!(owner.frame_metadata_usage().retained_frames, 1);
    assert_eq!(owner.origin_coverage(), Coverage::Complete);
    assert_eq!(owner.frame_coverage(), Coverage::Complete);
    assert_eq!(owner.origin_at(1).unwrap_err(), Missing::NoSuchRecord);
    assert_eq!(
        owner.frames_at(1).unwrap_err(),
        RuntimeFrameMissingV1::NoSuchRecord
    );
}

#[test]
fn frame_roster_cutoff_is_all_or_none_and_origin_continues() {
    let (module, request) = fixtures::loops();
    let config = RuntimeObservationOptionsV1::new(
        options().origins(),
        RuntimeFrameCaptureModeV1::Enabled(
            RuntimeFrameCaptureLimitsV1::new(4096, 1, 256 * 1024).unwrap(),
        ),
        RuntimeAllocationCaptureModeV1::Disabled,
        None,
    )
    .unwrap();
    let run = run(&module, &request, config);
    let owner = run.transcript();
    assert_eq!(
        owner.frame_coverage(),
        Coverage::PrefixTruncated(Cutoff::FrameRowLimit)
    );
    assert_eq!(owner.frame_metadata_usage().retained_records, 1);
    assert_eq!(owner.frame_metadata_usage().retained_frames, 1);
    assert_eq!(owner.frames_at(0).unwrap().len(), 1);
    assert_eq!(
        owner.frames_at(1).unwrap_err(),
        RuntimeFrameMissingV1::PrefixTruncated(Cutoff::FrameRowLimit)
    );
    assert_eq!(owner.origin_coverage(), Coverage::Complete);
    assert!(owner.origin_at(owner.legacy().records().len() - 1).is_ok());
}

#[test]
fn frame_byte_cutoff_and_origin_row_cutoff_have_distinct_coverage() {
    let (module, request) = fixtures::memory();
    let config = RuntimeObservationOptionsV1::new(
        RuntimeOriginCaptureModeV1::Enabled(RuntimeOriginCaptureLimitsV1::new(1, 4096).unwrap()),
        RuntimeFrameCaptureModeV1::Enabled(
            RuntimeFrameCaptureLimitsV1::new(4096, 4096, frames::minimum_metadata_bytes()).unwrap(),
        ),
        RuntimeAllocationCaptureModeV1::Disabled,
        None,
    )
    .unwrap();
    let run = run(&module, &request, config);
    let owner = run.transcript();
    assert_eq!(
        owner.origin_coverage(),
        Coverage::PrefixTruncated(Cutoff::RowLimit)
    );
    assert_eq!(
        owner.frame_coverage(),
        Coverage::PrefixTruncated(Cutoff::ByteLimit)
    );
    assert_eq!(owner.origin_metadata_usage().retained_rows, 1);
    assert_eq!(owner.frame_metadata_usage().retained_records, 1);
    assert_eq!(
        owner.frame_metadata_usage().metadata_bytes,
        frames::minimum_metadata_bytes()
    );
    assert_eq!(
        owner.origin_at(1).unwrap_err(),
        Missing::PrefixTruncated(Cutoff::RowLimit)
    );
}

#[test]
fn unavailable_legacy_stack_is_not_replaced_by_identity_only_frames() {
    let (module, request) = fixtures::loops();
    let run = capture_debugger_observed_run_v1(
        &module,
        &request,
        fixtures::TARGET,
        fixtures::simulation_limits(),
        SimulationDebugCaptureLimitsV1::new(1, 64, 8, 256).unwrap(),
        fixtures::debugger_limits(4096),
        DebugWaveWidthV1::Wave64,
        options(),
    )
    .unwrap();
    assert!(run.execution().is_ok());
    assert_eq!(run.transcript().frame_coverage(), Coverage::Complete);
    let index = run
        .transcript()
        .legacy()
        .records()
        .iter()
        .position(|record| record.site.function_ordinal == 1)
        .unwrap();
    assert_eq!(
        run.transcript().frames_at(index).unwrap_err(),
        RuntimeFrameMissingV1::RuntimeUnavailable(
            SimulationDebugFrameOriginUnavailableV1::LegacyStackUnavailable {
                reason: SimulationDebugUnavailableReasonV1::FrameLimit,
                required: 2,
            }
        )
    );
    assert!(run.transcript().origin_at(index).is_ok());
}

#[test]
fn failed_and_invalid_origin_reservations_never_retain_unaccounted_capacity() {
    let limits = Some(Limits::new(8, 4096).unwrap());
    let failed = Retention::with_reservation(limits, |_| Err(()));
    assert_eq!(
        failed.coverage,
        Coverage::PrefixTruncated(Cutoff::AllocationFailure)
    );
    assert_eq!(failed.rows.capacity(), 0);
    let invalid = Retention::with_reservation(limits, |_| Ok(Vec::new()));
    assert_eq!(
        invalid.coverage,
        Coverage::PrefixTruncated(Cutoff::InvalidCapacity)
    );
    assert_eq!(invalid.rows.capacity(), 0);
}

#[test]
fn exact_pair_and_replay_work_are_cumulative_and_failure_preserves_cursor() {
    let (module, request) = fixtures::memory();
    let (_, owner) = run(&module, &request, options()).into_parts();
    let mut scan = RuntimeOriginScanWorkV1::new(4).unwrap();
    assert_eq!(owner.paired_after(0, &mut scan), Ok(1));
    assert_eq!(owner.paired_after(0, &mut scan), Ok(1));
    assert_eq!(scan.remaining(), 0);
    assert_eq!(
        owner.paired_after(0, &mut scan),
        Err(RuntimeOriginPairErrorV1::WorkLimit)
    );
    let mut session = owner.into_session();
    let mut limited = RuntimeReplayWorkV1::new(2).unwrap();
    assert!(session.seek_record(0, &mut limited).is_err());
    assert_eq!(session.legacy().cursor_record_index(), None);
    let mut work = work();
    let first = session
        .continue_to_stop_bounded(RuntimeNavigationDirectionV1::Forward, 1, &mut work)
        .unwrap();
    assert!(matches!(first, DebugNavigationV1::BudgetExhausted(stop) if stop.record_index == 0));
    assert!(
        matches!(session.continue_to_stop_bounded(RuntimeNavigationDirectionV1::Forward, 0, &mut work).unwrap(),
        DebugNavigationV1::BudgetExhausted(stop) if stop.record_index == 0)
    );
}

fn breakpoint(id: u64) -> crate::DebugBreakpointV1 {
    crate::DebugBreakpointV1 {
        id,
        site: crate::DebugSiteSelectorV1 {
            function_ordinal: None,
            block: None,
            operation: None,
            phase: Some(SimulationDebugCheckpointPhaseV1::BeforeOperation),
        },
        scope: crate::DebugScopeSelectorV1::Dispatch,
        predicate: crate::DebugPredicateV1::True,
        hit_condition: None,
        enabled: true,
    }
}
#[test]
fn atomic_filters_validate_before_mutation_and_keep_the_eight_filter_bound() {
    let (module, request) = fixtures::memory();
    let (_, owner) = run(&module, &request, options()).into_parts();
    let mut session = owner.into_session();
    let error = session
        .add_breakpoints_atomic(vec![breakpoint(1), breakpoint(1)], &mut work())
        .unwrap_err();
    assert!(matches!(error.error(), RuntimeSessionErrorV1::Debugger(_)));
    assert!(session.legacy().breakpoints().is_empty());
    assert_eq!(error.into_parts().1.len(), 2);
    session
        .add_breakpoints_atomic((1..=8).map(breakpoint).collect(), &mut work())
        .unwrap();
    let error = session
        .add_breakpoint(breakpoint(9), &mut work())
        .unwrap_err();
    assert_eq!(error.error(), &RuntimeSessionErrorV1::FilterLimit);
    assert!(format!("{error:?}").len() < 200);
    assert_eq!(session.legacy().breakpoints().len(), 8);
    let navigation = session
        .continue_to_stop(RuntimeNavigationDirectionV1::Forward, &mut work())
        .unwrap();
    assert!(
        matches!(navigation, DebugNavigationV1::Stopped(stop) if stop.reason == DebugStopReasonV1::Breakpoint(1))
    );
    assert_eq!(session.legacy().breakpoint_hit_count(1), Some(1));
    assert!(session.remove_breakpoint(1, &mut work()).unwrap());
    assert_eq!(session.legacy().breakpoints().len(), 7);
}
