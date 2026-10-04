//! Synthetic interpreter controls; the ignored source ladder is separate.
use super::session::*;
use super::*;
use crate::*;
use fe2o3_kir_sim::{
    SimulationDebugCheckpointPhaseV1 as Phase, SimulationDebugRecordKindV1 as Kind,
};

pub(super) fn work() -> ReplayWork {
    ReplayWork::new(MAX_WORK).unwrap()
}
pub(super) fn memory() -> ObservedSession {
    let (module, request) = fixtures::memory();
    let (result, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(4096)),
        4096,
    );
    assert!(result.is_ok());
    observed.into_session()
}
pub(super) fn before(record: &SimulationDebugRecordV1) -> bool {
    matches!(
        record.kind,
        Kind::Checkpoint {
            phase: Phase::BeforeOperation,
            ..
        }
    )
}
pub(super) fn breakpoint(
    id: u64,
    site: SimulationDebugSiteV1,
    focus: SimulationInvocationV1,
) -> DebugBreakpointV1 {
    DebugBreakpointV1 {
        id,
        site: DebugSiteSelectorV1 {
            function_ordinal: Some(site.function_ordinal),
            block: Some(site.block),
            operation: Some(site.operation),
            phase: Some(Phase::BeforeOperation),
        },
        scope: DebugScopeSelectorV1::GlobalWorkitem(focus.global),
        predicate: DebugPredicateV1::True,
        hit_condition: None,
        enabled: true,
    }
}
pub(super) fn watchpoint(
    id: u64,
    allocation: u64,
    offset: usize,
    focus: SimulationInvocationV1,
) -> DebugWatchpointV1 {
    DebugWatchpointV1 {
        id,
        allocation,
        byte_offset: offset,
        byte_len: 4,
        access: DebugWatchAccessV1::Write,
        scope: DebugScopeSelectorV1::GlobalWorkitem(focus.global),
        value_equals: None,
        enabled: true,
    }
}
pub(super) fn stopped(result: DebugNavigationV1, reason: DebugStopReasonV1) -> usize {
    let DebugNavigationV1::Stopped(stop) = result else {
        panic!("not stopped: {result:?}")
    };
    assert_eq!(stop.reason, reason);
    stop.record_index
}

#[test]
fn moved_owner_keeps_storage_and_legacy_counters_without_mutable_escape() {
    let (module, request) = fixtures::memory();
    let (_, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    let address = observed.transcript.records().as_ptr();
    let mut session = observed.into_session();
    assert_eq!(session.view().transcript().records().as_ptr(), address);
    let focus = session.view().transcript().records()[0].invocation;
    let site = session.view().transcript().records()[0].site;
    session
        .add_breakpoint(breakpoint(1, site, focus), &mut work())
        .unwrap();
    let index = stopped(
        session
            .continue_to_stop(Direction::Forward, &mut work())
            .unwrap(),
        DebugStopReasonV1::Breakpoint(1),
    );
    assert_eq!(index, 0);
    assert_eq!(session.view().breakpoint_hit_count(1), Some(1));
    assert_eq!(session.current_origin().unwrap().record.ordinal, 0);
    session.seek_entry(&mut work()).unwrap();
    assert_eq!(session.view().breakpoint_hit_count(1), Some(0));
    assert!(session.remove_breakpoint(1, &mut work()).unwrap());
    assert!(!session.remove_breakpoint(1, &mut work()).unwrap());
}

#[test]
fn into_is_full_invocation_pinned_and_over_is_exactly_reversible_through_helpers() {
    let (module, request) = fixtures::loops();
    for seeded in [false, true] {
        let (_, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            seeded,
            Some(fixtures::origin_limits(4096)),
            4096,
        );
        let mut session = observed.into_session();
        let focus = session.view().transcript().records()[0].invocation;
        let indices: Vec<_> = session
            .view()
            .transcript()
            .records()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.invocation == focus && matches!(r.kind, Kind::Checkpoint { .. }))
            .map(|(i, _)| i)
            .collect();
        let mut budget = work();
        for &index in &indices {
            assert_eq!(
                stopped(
                    session
                        .step_into(Direction::Forward, focus, &mut budget)
                        .unwrap(),
                    DebugStopReasonV1::Step
                ),
                index
            );
        }
        for &index in indices[..indices.len() - 1].iter().rev() {
            assert_eq!(
                stopped(
                    session
                        .step_into(Direction::Reverse, focus, &mut budget)
                        .unwrap(),
                    DebugStopReasonV1::Step
                ),
                index
            );
        }
        assert_eq!(
            session
                .step_into(Direction::Reverse, focus, &mut budget)
                .unwrap(),
            DebugNavigationV1::Beginning
        );
        let calls: Vec<_> = session
            .view()
            .transcript()
            .records()
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.invocation == focus
                    && r.site.function_ordinal == 0
                    && r.site.block.0 == 1
                    && r.site.operation == 0
                    && before(r)
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(calls.len(), 3);
        let mut attempts = std::collections::BTreeSet::new();
        for index in calls {
            session.seek_record(index, &mut budget).unwrap();
            let row = *session.current_origin().unwrap().row;
            assert!(attempts.insert(row.attempt));
            assert_eq!(row.activation, 1);
            let after = stopped(
                session
                    .step_over(Direction::Forward, focus, &mut budget)
                    .unwrap(),
                DebugStopReasonV1::Step,
            );
            assert!(after > index + 1);
            assert_eq!(*session.current_origin().unwrap().row, row);
            assert_eq!(
                stopped(
                    session
                        .step_over(Direction::Reverse, focus, &mut budget)
                        .unwrap(),
                    DebugStopReasonV1::Step
                ),
                index
            );
        }
    }
}

#[test]
fn interleaved_yields_and_aggregate_release_do_not_change_pair_identity() {
    let (module, request) = fixtures::barrier();
    let (_, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        true,
        Some(fixtures::origin_limits(4096)),
        4096,
    );
    let mut session = observed.into_session();
    let index = session
        .view()
        .transcript()
        .records()
        .iter()
        .position(|r| before(r) && r.site.function_ordinal == 0)
        .unwrap();
    let focus = session.view().transcript().records()[index].invocation;
    session.seek_record(index, &mut work()).unwrap();
    let after = stopped(
        session
            .step_over(Direction::Forward, focus, &mut work())
            .unwrap(),
        DebugStopReasonV1::Step,
    );
    assert!(
        session.view().transcript().records()[index + 1..after]
            .iter()
            .any(|r| r.invocation != focus)
    );
    assert_eq!(
        stopped(
            session
                .step_over(Direction::Reverse, focus, &mut work())
                .unwrap(),
            DebugStopReasonV1::Step
        ),
        index
    );
}

#[test]
fn break_watch_hit_counts_restore_in_both_directions_and_registration_sees_prefix() {
    let mut session = memory();
    let records = session.view().transcript().records();
    let focus = records[0].invocation;
    let site = records[0].site;
    let (memory_index, allocation, offset) = records
        .iter()
        .enumerate()
        .find_map(|(i, r)| match r.kind {
            Kind::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                ..
            } => Some((i, allocation, byte_offset)),
            _ => None,
        })
        .unwrap();
    session
        .add_breakpoint(breakpoint(1, site, focus), &mut work())
        .unwrap();
    session
        .add_watchpoint(watchpoint(2, allocation, offset, focus), &mut work())
        .unwrap();
    let start = stopped(
        session
            .continue_to_stop(Direction::Forward, &mut work())
            .unwrap(),
        DebugStopReasonV1::Breakpoint(1),
    );
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work())
                .unwrap(),
            DebugStopReasonV1::Watchpoint(2)
        ),
        memory_index
    );
    let origin = *session.current_origin().unwrap().row;
    assert_eq!(session.view().watchpoint_hit_count(2), Some(1));
    session
        .add_breakpoint(breakpoint(3, site, focus), &mut work())
        .unwrap();
    assert_eq!(session.view().breakpoint_hit_count(3), Some(1));
    assert!(session.remove_breakpoint(3, &mut work()).unwrap());
    assert_eq!(
        session
            .continue_to_stop(Direction::Forward, &mut work())
            .unwrap(),
        DebugNavigationV1::End
    );
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Reverse, &mut work())
                .unwrap(),
            DebugStopReasonV1::Watchpoint(2)
        ),
        memory_index
    );
    assert_eq!(*session.current_origin().unwrap().row, origin);
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Reverse, &mut work())
                .unwrap(),
            DebugStopReasonV1::Breakpoint(1)
        ),
        start
    );
    assert_eq!(session.view().watchpoint_hit_count(2), Some(0));
    assert_eq!(
        session
            .continue_to_stop(Direction::Reverse, &mut work())
            .unwrap(),
        DebugNavigationV1::Beginning
    );
    assert_eq!(session.view().breakpoint_hit_count(1), Some(0));
    assert!(session.remove_watchpoint(2, &mut work()).unwrap());
}

#[test]
fn hit_conditions_and_predicates_remain_the_existing_matcher_semantics() {
    let (module, request) = fixtures::loops();
    for condition in [
        DebugHitConditionV1::Equal(2),
        DebugHitConditionV1::AtLeast(2),
        DebugHitConditionV1::Multiple(2),
    ] {
        let (_, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            false,
            Some(fixtures::origin_limits(4096)),
            4096,
        );
        let mut session = observed.into_session();
        let record = session
            .view()
            .transcript()
            .records()
            .iter()
            .find(|r| {
                before(r)
                    && r.site.function_ordinal == 0
                    && r.site.block.0 == 1
                    && r.site.operation == 0
            })
            .unwrap();
        let mut bp = breakpoint(1, record.site, record.invocation);
        bp.hit_condition = Some(condition);
        // Root block parameter is a genuine captured scalar, not a source name.
        bp.predicate = DebugPredicateV1::ScalarNotEquals {
            frame_depth: 0,
            value: fe2o3_kernel_ir::ValueId(10),
            expected: ScalarBitsV1::index(99, fixtures::TARGET).unwrap(),
        };
        session.add_breakpoint(bp, &mut work()).unwrap();
        stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work())
                .unwrap(),
            DebugStopReasonV1::Breakpoint(1),
        );
        assert_eq!(session.view().breakpoint_hit_count(1), Some(2));
        session.seek_entry(&mut work()).unwrap();
        assert_eq!(session.view().breakpoint_hit_count(1), Some(0));
    }
}
