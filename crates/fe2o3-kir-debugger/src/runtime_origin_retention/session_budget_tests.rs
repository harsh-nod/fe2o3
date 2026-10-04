//! Negative/budget controls are synthetic, not source qualification.
use super::session::*;
use super::session_tests::*;
use super::*;
use crate::*;
use fe2o3_kir_sim::SimulationDebugRecordKindV1 as Kind;

#[test]
fn zero_work_and_failed_registration_or_seek_preserve_semantic_state() {
    assert!(ReplayWork::new(MAX_WORK + 1).is_err());
    let mut session = memory();
    let record = &session.view().transcript().records()[0];
    let focus = record.invocation;
    let bp = breakpoint(1, record.site, focus);
    let rejected = session
        .add_breakpoint(bp.clone(), &mut ReplayWork::new(0).unwrap())
        .unwrap_err();
    assert_eq!(rejected.error(), &SessionError::WorkLimit);
    let (_, returned) = rejected.into_parts();
    assert_eq!(returned, bp);
    assert!(session.view().breakpoints().is_empty());
    assert_eq!(session.view().cursor_record_index(), None);
    session.add_breakpoint(bp, &mut work()).unwrap();
    assert_eq!(
        session.seek_record(1, &mut ReplayWork::new(1).unwrap()),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), None);
    assert_eq!(session.view().breakpoint_hit_count(1), Some(0));
    assert_eq!(
        session.step_into(Direction::Forward, focus, &mut ReplayWork::new(0).unwrap()),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), None);
    session.seek_record(0, &mut work()).unwrap();
    assert_eq!(
        session.step_over(Direction::Forward, focus, &mut ReplayWork::new(1).unwrap()),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), Some(0));
    assert_eq!(session.view().breakpoint_hit_count(1), Some(1));
}

#[test]
fn actual_seek_cost_includes_crossed_predicates_and_budget_is_cumulative() {
    let mut session = memory();
    let first = &session.view().transcript().records()[0];
    let focus = first.invocation;
    let bp = breakpoint(1, first.site, focus);
    session.add_breakpoint(bp, &mut work()).unwrap();
    let cost = 2 + session
        .filter_work(&session.view().transcript().records()[0])
        .unwrap();
    assert!(cost > 2);
    let mut budget = ReplayWork::new(cost - 1).unwrap();
    assert_eq!(
        session.seek_record(0, &mut budget),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), None);
    let mut exact = ReplayWork::new(cost).unwrap();
    session.seek_record(0, &mut exact).unwrap();
    assert_eq!(exact.remaining(), 0);
    assert_eq!(session.seek_entry(&mut exact), Err(SessionError::WorkLimit));
    assert_eq!(session.view().cursor_record_index(), Some(0));
    let before = session.view().breakpoints().len();
    assert_eq!(
        session.remove_breakpoint(1, &mut exact),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().breakpoints().len(), before);
}

#[test]
fn bounded_continue_stops_at_last_committed_cursor_without_false_breakpoint() {
    let mut session = memory();
    // No filters: charge scan1 + stop eligibility2 + seek1 + crossed row1.
    let mut budget = ReplayWork::new(5).unwrap();
    assert_eq!(
        session.continue_to_stop(Direction::Forward, &mut budget),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), Some(0));
    assert_eq!(budget.remaining(), 0);
    assert!(session.view().breakpoints().is_empty());
    let origin = *session.current_origin().unwrap().row;
    let mut zero = ReplayWork::new(0).unwrap();
    assert_eq!(
        session.continue_to_stop(Direction::Reverse, &mut zero),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(*session.current_origin().unwrap().row, origin);
}

#[test]
fn invalid_focus_phase_metadata_and_pair_fail_without_moving_cursor() {
    let mut session = memory();
    let focus = session.view().transcript().records()[0].invocation;
    session.seek_record(0, &mut work()).unwrap();
    let mut wrong = focus;
    wrong.launch_extent[0] += 1; // Same global lane is not a full invocation.
    assert_eq!(
        session.step_over(Direction::Forward, wrong, &mut work()),
        Err(SessionError::FocusMismatch)
    );
    assert_eq!(
        session.step_over(Direction::Reverse, focus, &mut work()),
        Err(SessionError::WrongPhase)
    );
    assert_eq!(session.view().cursor_record_index(), Some(0));
    session.origins.rows[1].attempt += 1;
    assert_eq!(
        session.step_over(Direction::Forward, focus, &mut work()),
        Err(SessionError::InvalidSequence)
    );
    assert_eq!(session.view().cursor_record_index(), Some(0));
    session.origins.rows[1].status = Status::RuntimeUnavailable(RuntimeMissing::IdentityInvariant);
    assert_eq!(
        session.step_over(Direction::Forward, focus, &mut work()),
        Err(SessionError::Origin(Missing::RuntimeUnavailable(
            RuntimeMissing::IdentityInvariant
        )))
    );
    session.origins.rows.truncate(1);
    session.origins.coverage = Coverage::PrefixTruncated(Cutoff::RowLimit);
    assert_eq!(
        session.step_over(Direction::Forward, focus, &mut work()),
        Err(SessionError::Origin(Missing::PrefixTruncated(
            Cutoff::RowLimit
        )))
    );
    assert_eq!(session.view().cursor_record_index(), Some(0));
}

#[test]
fn record_coordinates_need_not_be_roster_positions_and_unknown_terminal_is_not_pair() {
    let mut session = memory();
    let focus = session.view().transcript().records()[0].invocation;
    // Synthetic adapter-domain control; no source/runtime claim for this edit.
    for record in &mut session.session.transcript.records {
        record.site.block = fe2o3_kernel_ir::BlockId(99);
    }
    session.seek_record(0, &mut work()).unwrap();
    assert_eq!(
        stopped(
            session
                .step_over(Direction::Forward, focus, &mut work())
                .unwrap(),
            DebugStopReasonV1::Step
        ),
        1
    );
    assert_eq!(session.current_origin().unwrap().record.site.block.0, 99);
    assert_eq!(
        stopped(
            session
                .step_over(Direction::Reverse, focus, &mut work())
                .unwrap(),
            DebugStopReasonV1::Step
        ),
        0
    );
    session.session.transcript.records.truncate(1);
    session.origins.rows.truncate(1);
    assert_eq!(
        session.step_over(Direction::Forward, focus, &mut work()),
        Err(SessionError::MissingPair)
    );
}

#[test]
fn filters_are_bounded_and_disabled_capture_does_not_gain_identity() {
    let mut session = memory();
    let record = &session.view().transcript().records()[0];
    let focus = record.invocation;
    let site = record.site;
    for id in 1..=MAX_FILTERS as u64 {
        let mut bp = breakpoint(id, site, focus);
        bp.enabled = false;
        session.add_breakpoint(bp, &mut work()).unwrap();
    }
    let rejected = session
        .add_breakpoint(breakpoint(99, site, focus), &mut work())
        .unwrap_err();
    assert_eq!(rejected.error(), &SessionError::FilterLimit);
    let (_, returned) = rejected.into_parts();
    assert_eq!(returned.id, 99);
    assert_eq!(
        session
            .continue_to_stop(Direction::Forward, &mut work())
            .unwrap(),
        DebugNavigationV1::End
    );
    assert_eq!(session.view().breakpoint_hit_count(1), Some(0));
    let (module, request) = fixtures::memory();
    let (actual, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        None,
        4096,
    );
    let (expected, legacy) = fixtures::legacy(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        4096,
    );
    fixtures::assert_result_eq(&actual, &expected);
    let mut disabled = observed.into_session();
    assert_eq!(disabled.view().transcript(), &legacy);
    assert_eq!(
        disabled.step_into(Direction::Forward, focus, &mut work()),
        Err(SessionError::Origin(Missing::Disabled))
    );
    assert_eq!(disabled.view().cursor_record_index(), None);
    assert_eq!(disabled.origins.rows.capacity(), 0);
    assert!(matches!(
        disabled.view().transcript().records()[0].kind,
        Kind::Checkpoint { .. }
    ));
}
