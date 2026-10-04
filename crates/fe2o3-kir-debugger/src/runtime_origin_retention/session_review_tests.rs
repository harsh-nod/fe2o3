//! Model-only review regressions; no source, lifecycle-reuse or public API claim.
use super::session::*;
use super::session_tests::{before, breakpoint, memory, stopped, watchpoint, work};
use super::*;
use crate::*;
use fe2o3_kernel_ir::ValueId;
use fe2o3_kir_sim::{SimulationDebugRecordKindV1 as Kind, SimulationErrorV1, SimulationLimitsV1};

fn same_cursor_and_counts(session: &ObservedSession, legacy: &DebugSessionV1) {
    assert_eq!(
        session.view().cursor_record_index(),
        legacy.cursor_record_index()
    );
    assert_eq!(
        session.view().breakpoint_hit_count(1),
        legacy.breakpoint_hit_count(1)
    );
    assert_eq!(
        session.view().watchpoint_hit_count(2),
        legacy.watchpoint_hit_count(2)
    );
}

#[test]
fn real_terminal_fault_is_reported_once_then_end_and_again_only_after_reverse_revisit() {
    let (module, request) = fixtures::memory();
    let limits = SimulationLimitsV1 {
        max_steps: 1,
        ..fixtures::simulation_limits()
    };
    let (result, observed) = fixtures::observed(
        &module,
        &request,
        limits,
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    let (expected, transcript) = fixtures::legacy(&module, &request, limits, false, 4096);
    fixtures::assert_result_eq(&result, &expected);
    assert!(matches!(result, Err(SimulationErrorV1::Execution(_))));
    assert_eq!(observed.transcript, transcript);
    assert!(!transcript.records().is_empty());
    let count = transcript.records().len();
    let last = count - 1;
    assert!(before(&transcript.records()[last]));
    let focus = transcript.records()[last].invocation;
    let fault = transcript
        .terminal_fault()
        .cloned()
        .expect("actual simulator fault");
    let detail = transcript.terminal_detail_v2().clone();
    let bp = breakpoint(1, transcript.records()[0].site, focus);
    // This synthetic fixture has exactly one ordinary input buffer allocation.
    let wp = watchpoint(2, 1, 0, focus);
    let mut legacy = DebugSessionV1::new(transcript);
    let mut session = observed.into_session();
    legacy.add_breakpoint(bp.clone()).unwrap();
    legacy.add_watchpoint(wp.clone()).unwrap();
    session.add_breakpoint(bp, &mut work()).unwrap();
    session.add_watchpoint(wp, &mut work()).unwrap();
    assert_eq!(
        session.seek_record(last, &mut work()).unwrap(),
        legacy.seek_record_index(last),
    );
    assert_eq!(
        session.step_over(Direction::Forward, focus, &mut work()),
        Err(SessionError::MissingPair),
    );
    same_cursor_and_counts(&session, &legacy);

    for visit in 0..2 {
        let actual = session
            .continue_to_stop(Direction::Forward, &mut work())
            .unwrap();
        assert_eq!(actual, legacy.continue_forward_bounded(4096));
        assert_eq!(stopped(actual, DebugStopReasonV1::Fault), count);
        same_cursor_and_counts(&session, &legacy);
        assert_eq!(session.view().transcript().terminal_fault(), Some(&fault));
        assert_eq!(session.view().transcript().terminal_detail_v2(), &detail);
        assert_eq!(
            session
                .continue_to_stop(Direction::Forward, &mut work())
                .unwrap(),
            DebugNavigationV1::End,
        );
        assert_eq!(
            legacy.continue_forward_bounded(4096),
            DebugNavigationV1::End
        );
        same_cursor_and_counts(&session, &legacy);
        if visit == 0 {
            let actual = session
                .step_into(Direction::Reverse, focus, &mut work())
                .unwrap();
            let expected = legacy.reverse_step(&DebugScopeSelectorV1::GlobalWorkitem(focus.global));
            assert_eq!(actual, expected);
            assert_eq!(stopped(actual, DebugStopReasonV1::Step), last);
            same_cursor_and_counts(&session, &legacy);
        }
    }
}

#[test]
fn identical_fresh_tokens_stay_borrowed_from_each_owning_capture() {
    let (module, request) = fixtures::memory();
    let capture = || {
        let (result, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            false,
            Some(fixtures::origin_limits(64)),
            4096,
        );
        assert!(result.is_ok());
        assert!(!observed.transcript.records().is_empty());
        observed
    };
    let first = capture();
    let second = capture();
    let first_storage = first.transcript.records().as_ptr();
    let second_storage = second.transcript.records().as_ptr();
    assert_ne!(first_storage, second_storage);
    assert_eq!(
        first.origin_at(0).unwrap().row,
        second.origin_at(0).unwrap().row
    );
    let mut first = first.into_session();
    let mut second = second.into_session();
    first.seek_record(0, &mut work()).unwrap();
    second.seek_record(0, &mut work()).unwrap();
    assert_eq!(
        first.current_origin().unwrap().record as *const _,
        first_storage
    );
    assert_eq!(
        second.current_origin().unwrap().record as *const _,
        second_storage
    );
    let focus = first.current_origin().unwrap().record.invocation;
    first
        .step_over(Direction::Forward, focus, &mut work())
        .unwrap();
    assert_eq!(first.view().cursor_record_index(), Some(1));
    assert_eq!(second.view().cursor_record_index(), Some(0));
    assert!(std::ptr::eq(
        first.current_origin().unwrap().record,
        first.view().current().unwrap(),
    ));
    assert!(std::ptr::eq(
        second.current_origin().unwrap().record,
        second.view().current().unwrap(),
    ));
    // The raw row/transcript join is leaf-private. Only owning-self lookup
    // methods are visible here; there is no detached-observation input API.
}

#[test]
fn reverse_hit_conditions_use_restored_inclusive_prefix_counts() {
    for (condition, expected_hits) in [
        (DebugHitConditionV1::Equal(2), vec![2]),
        (DebugHitConditionV1::AtLeast(2), vec![3, 2]),
        (DebugHitConditionV1::Multiple(2), vec![2]),
    ] {
        let (module, request) = fixtures::loops();
        let (result, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            false,
            Some(fixtures::origin_limits(4096)),
            4096,
        );
        assert!(result.is_ok());
        let mut session = observed.into_session();
        let initial = session
            .view()
            .transcript()
            .records()
            .iter()
            .find(|record| {
                before(record)
                    && record.site.function_ordinal == 0
                    && record.site.block.0 == 1
                    && record.site.operation == 0
            })
            .unwrap();
        let focus = initial.invocation;
        let site = initial.site;
        let calls: Vec<_> = session
            .view()
            .transcript()
            .records()
            .iter()
            .enumerate()
            .filter(|(_, record)| {
                before(record) && record.invocation == focus && record.site == site
            })
            .map(|(index, _)| index)
            .collect();
        assert_eq!(calls.len(), 3);
        let mut bp = breakpoint(1, site, focus);
        bp.hit_condition = Some(condition);
        session.add_breakpoint(bp, &mut work()).unwrap();
        let last = session.view().transcript().records().len() - 1;
        session.seek_record(last, &mut work()).unwrap();
        assert_eq!(session.view().breakpoint_hit_count(1), Some(3));
        for hits in expected_hits {
            let result = session
                .continue_to_stop(Direction::Reverse, &mut work())
                .unwrap();
            assert_eq!(
                stopped(result, DebugStopReasonV1::Breakpoint(1)),
                calls[hits - 1]
            );
            assert_eq!(session.view().breakpoint_hit_count(1), Some(hits as u64));
        }
        assert_eq!(
            session
                .continue_to_stop(Direction::Reverse, &mut work())
                .unwrap(),
            DebugNavigationV1::Beginning,
        );
        assert_eq!(session.view().breakpoint_hit_count(1), Some(0));
        let result = session
            .continue_to_stop(Direction::Forward, &mut work())
            .unwrap();
        assert_eq!(stopped(result, DebugStopReasonV1::Breakpoint(1)), calls[1]);
        assert_eq!(session.view().breakpoint_hit_count(1), Some(2));
    }
}

#[test]
fn late_watch_registration_and_asymmetric_reverse_seek_charge_crossed_records() {
    let mut session = memory();
    let records = session.view().transcript().records();
    let (write, allocation, offset) = records
        .iter()
        .enumerate()
        .find_map(|(index, record)| match record.kind {
            Kind::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                ..
            } => Some((index, allocation, byte_offset)),
            _ => None,
        })
        .unwrap();
    assert!(write > 0 && write + 1 < records.len());
    let after = write + 1;
    let before_store = write - 1;
    assert!(before(&records[before_store]));
    assert!(matches!(
        records[after].kind,
        Kind::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            ..
        },
    ));
    let focus = records[write].invocation;
    let mut bp = breakpoint(1, records[before_store].site, focus);
    bp.predicate = DebugPredicateV1::ScalarEquals {
        frame_depth: 0,
        value: ValueId(1),
        expected: ScalarBitsV1::u32(42),
    };
    session.seek_record(after, &mut work()).unwrap();
    session.add_breakpoint(bp, &mut work()).unwrap();
    session
        .add_watchpoint(watchpoint(2, allocation, offset, focus), &mut work())
        .unwrap();
    assert_eq!(session.view().breakpoint_hit_count(1), Some(1));
    assert_eq!(session.view().watchpoint_hit_count(2), Some(1));

    let removed = session
        .filter_work(&session.view().transcript().records()[after])
        .unwrap();
    let destination = session
        .filter_work(&session.view().transcript().records()[write])
        .unwrap();
    assert!(removed > destination); // Checkpoint frame scan versus Memory.
    let mut too_small = ReplayWork::new(2 + destination).unwrap();
    assert_eq!(
        session.seek_record(write, &mut too_small),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), Some(after));
    assert_eq!(session.view().watchpoint_hit_count(2), Some(1));
    let mut exact = ReplayWork::new(2 + removed).unwrap();
    assert_eq!(
        stopped(
            session.seek_record(write, &mut exact).unwrap(),
            DebugStopReasonV1::Step
        ),
        write,
    );
    assert_eq!(exact.remaining(), 0);
    assert_eq!(session.view().watchpoint_hit_count(2), Some(1));

    let removed = session
        .filter_work(&session.view().transcript().records()[write])
        .unwrap();
    let destination = session
        .filter_work(&session.view().transcript().records()[before_store])
        .unwrap();
    assert!(removed < destination);
    let mut exact = ReplayWork::new(2 + removed).unwrap();
    session.seek_record(before_store, &mut exact).unwrap();
    assert_eq!(exact.remaining(), 0);
    assert_eq!(session.view().watchpoint_hit_count(2), Some(0));
    session
        .add_watchpoint(watchpoint(3, allocation, offset, focus), &mut work())
        .unwrap();
    assert_eq!(session.view().watchpoint_hit_count(3), Some(0));
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work())
                .unwrap(),
            DebugStopReasonV1::Watchpoint(2),
        ),
        write,
    );
    assert_eq!(session.view().watchpoint_hit_count(2), Some(1));
    assert_eq!(session.view().watchpoint_hit_count(3), Some(1));
}
