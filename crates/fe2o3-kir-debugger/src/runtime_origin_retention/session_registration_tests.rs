//! Bounded synthetic ownership regressions, not arbitrary-input or public API qualification.
use super::super::super::session_tests::{breakpoint, memory, watchpoint, work};
use super::*;
use crate::{
    DebugHitConditionV1, DebugNavigationV1, DebugWatchpointV1, MAX_DEBUGGER_PREDICATE_DEPTH_V1,
};

const FIXED: usize = 3 * (MAX_DEBUGGER_PREDICATE_NODES_V1 + 1) + MAX_FILTERS;
const CLEANUP_CAP: usize = MAX_DEBUGGER_PREDICATE_NODES_V1 + MAX_DEBUGGER_PREDICATE_DEPTH_V1 + 2;

#[derive(Debug, PartialEq)]
struct State {
    cursor: Option<usize>,
    breakpoints: Vec<DebugBreakpointV1>,
    breakpoint_hits: Vec<u64>,
    watchpoints: Vec<DebugWatchpointV1>,
    watchpoint_hits: Vec<u64>,
    predicate_nodes: [usize; MAX_FILTERS],
    transcript: DebugTranscriptV1,
    record_address: usize,
    rows: Vec<Row>,
    row_address: usize,
    row_capacity: usize,
    row_limits: Option<Limits>,
    capacity_limit: usize,
    coverage: Coverage,
}
fn state(session: &ObservedSession) -> State {
    assert!(session.session.transcript.records.len() <= 4096);
    assert!(session.origins.rows.len() <= 4096);
    State {
        cursor: session.session.cursor,
        breakpoints: session.session.breakpoints.clone(),
        breakpoint_hits: session.session.breakpoint_hits.clone(),
        watchpoints: session.session.watchpoints.clone(),
        watchpoint_hits: session.session.watchpoint_hits.clone(),
        predicate_nodes: session.predicate_nodes,
        transcript: session.session.transcript.clone(),
        record_address: session.session.transcript.records.as_ptr() as usize,
        rows: session.origins.rows.clone(),
        row_address: session.origins.rows.as_ptr() as usize,
        row_capacity: session.origins.rows.capacity(),
        row_limits: session.origins.limits,
        capacity_limit: session.origins.capacity_limit,
        coverage: session.origins.coverage,
    }
}
fn payload_address(predicate: &DebugPredicateV1) -> usize {
    match predicate {
        DebugPredicateV1::And(values) | DebugPredicateV1::Or(values) => values.as_ptr() as usize,
        DebugPredicateV1::Not(value) => &**value as *const DebugPredicateV1 as usize,
        _ => panic!("control needs heap-backed predicate"),
    }
}
fn candidate(session: &ObservedSession, id: u64, predicate: DebugPredicateV1) -> DebugBreakpointV1 {
    let record = &session.session.transcript.records[0];
    let mut result = breakpoint(id, record.site, record.invocation);
    result.predicate = predicate;
    result
}
fn deep() -> DebugPredicateV1 {
    let mut predicate = DebugPredicateV1::True;
    for _ in 0..MAX_DEBUGGER_PREDICATE_DEPTH_V1 {
        predicate = DebugPredicateV1::Not(Box::new(predicate));
    }
    predicate
}
fn wide() -> DebugPredicateV1 {
    DebugPredicateV1::And(vec![
        DebugPredicateV1::True;
        MAX_DEBUGGER_PREDICATE_NODES_V1
    ])
}
fn small() -> DebugPredicateV1 {
    DebugPredicateV1::And(vec![DebugPredicateV1::True, DebugPredicateV1::True])
}
// Caller-owned test cleanup only. Fixtures contain at most 1025 nodes and depth
// 33; no production promise is inferred about arbitrary recursive Drop, Vec
// spare capacity, allocator behavior, caller abandonment, panic or process exit.
fn caller_cleanup(breakpoint: DebugBreakpointV1) -> usize {
    let mut pending = Vec::with_capacity(CLEANUP_CAP);
    pending.push(breakpoint.predicate);
    let mut seen = 0;
    while let Some(predicate) = pending.pop() {
        seen += 1;
        assert!(seen <= CLEANUP_CAP);
        match predicate {
            DebugPredicateV1::Not(child) => pending.push(*child),
            DebugPredicateV1::And(children) | DebugPredicateV1::Or(children) => {
                assert!(pending.len() + children.len() <= CLEANUP_CAP);
                pending.extend(children);
            }
            _ => {}
        }
    }
    seen
}
fn rejected(
    session: &mut ObservedSession,
    payload: DebugBreakpointV1,
    budget: &mut ReplayWork,
    expected: SessionError,
) -> DebugBreakpointV1 {
    let before = state(session);
    let address = payload_address(&payload.predicate);
    let refusal = session.add_breakpoint(payload, budget).unwrap_err();
    assert_eq!(refusal.error(), &expected);
    let diagnostic = format!("{refusal:?}");
    assert!(
        diagnostic.len() <= 256 && !diagnostic.contains("And(") && !diagnostic.contains("Not(")
    );
    assert_eq!(
        state(session),
        before,
        "refusal changed replay/counter/sidecar state"
    );
    let (actual, returned) = refusal.into_parts();
    assert_eq!(actual, expected);
    assert_eq!(
        payload_address(&returned.predicate),
        address,
        "must return original owned allocation"
    );
    returned
}

#[test]
fn over_depth_returns_original_owned_tree_without_recursive_refusal_cleanup() {
    let mut session = memory();
    let payload = candidate(&session, 91, deep());
    let mut budget = ReplayWork::new(FIXED).unwrap();
    let returned = rejected(
        &mut session,
        payload,
        &mut budget,
        SessionError::Debugger(DebuggerErrorV1::PredicateLimit),
    );
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        caller_cleanup(returned),
        MAX_DEBUGGER_PREDICATE_DEPTH_V1 + 1
    );
}

#[test]
fn over_nodes_returns_original_owned_vector_without_refusal_cleanup() {
    let mut session = memory();
    let payload = candidate(&session, 92, wide());
    let mut budget = ReplayWork::new(FIXED).unwrap();
    let returned = rejected(
        &mut session,
        payload,
        &mut budget,
        SessionError::Debugger(DebuggerErrorV1::PredicateLimit),
    );
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        caller_cleanup(returned),
        MAX_DEBUGGER_PREDICATE_NODES_V1 + 1
    );
}

#[test]
fn zero_work_returns_unadmitted_payload_without_charging_or_session_change() {
    let mut session = memory();
    let payload = candidate(&session, 93, wide());
    let mut budget = ReplayWork::new(0).unwrap();
    let returned = rejected(&mut session, payload, &mut budget, SessionError::WorkLimit);
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        caller_cleanup(returned),
        MAX_DEBUGGER_PREDICATE_NODES_V1 + 1
    );
}

#[test]
fn full_filter_cap_returns_unadmitted_payload_before_validation_or_work_charge() {
    let mut session = memory();
    for id in 1..=MAX_FILTERS as u64 {
        let payload = candidate(&session, id, DebugPredicateV1::True);
        session.add_breakpoint(payload, &mut work()).unwrap();
    }
    session.seek_record(0, &mut work()).unwrap();
    let payload = candidate(&session, 94, deep());
    let mut budget = ReplayWork::new(37).unwrap();
    let returned = rejected(
        &mut session,
        payload,
        &mut budget,
        SessionError::FilterLimit,
    );
    assert_eq!(budget.remaining(), 37);
    assert_eq!(
        caller_cleanup(returned),
        MAX_DEBUGGER_PREDICATE_DEPTH_V1 + 1
    );
}

#[test]
fn prefix_budget_refusal_keeps_original_payload_and_existing_hit_counts() {
    let mut session = memory();
    let first = &session.session.transcript.records[0];
    let (site, focus) = (first.site, first.invocation);
    session
        .add_breakpoint(breakpoint(1, site, focus), &mut work())
        .unwrap();
    session
        .add_watchpoint(watchpoint(2, 1, 0, focus), &mut work())
        .unwrap();
    session.seek_record(0, &mut work()).unwrap();
    assert_eq!(session.view().breakpoint_hit_count(1), Some(1));
    let payload = candidate(&session, 95, small());
    let prefix = predicate_work(&session.session.transcript.records[0], 3).unwrap();
    let mut budget = ReplayWork::new(FIXED + prefix - 1).unwrap();
    let returned = rejected(&mut session, payload, &mut budget, SessionError::WorkLimit);
    assert_eq!(
        budget.remaining(),
        prefix - 1,
        "fixed admission paid; refused scan not partially charged"
    );
    assert_eq!(caller_cleanup(returned), 3);
}

#[test]
fn duplicate_and_hit_condition_refusals_keep_payload_ownership() {
    let mut session = memory();
    session
        .add_breakpoint(candidate(&session, 1, DebugPredicateV1::True), &mut work())
        .unwrap();
    for duplicate in [true, false] {
        let mut payload = candidate(&session, if duplicate { 1 } else { 96 }, small());
        if !duplicate {
            payload.hit_condition = Some(DebugHitConditionV1::Equal(0));
        }
        let mut budget = ReplayWork::new(FIXED).unwrap();
        let error = if duplicate {
            DebuggerErrorV1::InvalidOrDuplicateIdentity
        } else {
            DebuggerErrorV1::InvalidHitCondition
        };
        let returned = rejected(
            &mut session,
            payload,
            &mut budget,
            SessionError::Debugger(error),
        );
        assert_eq!(budget.remaining(), 0);
        assert_eq!(caller_cleanup(returned), 3);
    }
}

#[test]
fn admitted_tree_moves_once_and_uses_existing_matcher_with_exact_cumulative_charge() {
    let mut session = memory();
    session.seek_record(0, &mut work()).unwrap();
    let mut legacy = DebugSessionV1::new(session.session.transcript.clone());
    legacy.seek_record_index(0);
    let payload = candidate(&session, 97, small());
    let address = payload_address(&payload.predicate);
    legacy.add_breakpoint(payload.clone()).unwrap();
    let prefix = predicate_work(&session.session.transcript.records[0], 3).unwrap();
    let mut budget = ReplayWork::new(FIXED + prefix).unwrap();
    session.add_breakpoint(payload, &mut budget).unwrap();
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        payload_address(&session.view().breakpoints()[0].predicate),
        address
    );
    assert_eq!(session.predicate_nodes[0], 3);
    assert_eq!(session.view().breakpoints(), legacy.breakpoints());
    assert_eq!(
        session.view().breakpoint_hit_count(97),
        legacy.breakpoint_hit_count(97)
    );
    assert_eq!(
        session.seek_entry(&mut work()).unwrap(),
        DebugNavigationV1::Beginning
    );
    legacy.seek_entry();
    assert_eq!(
        session.view().breakpoint_hit_count(97),
        legacy.breakpoint_hit_count(97)
    );
    session.seek_record(0, &mut work()).unwrap();
    legacy.seek_record_index(0);
    assert_eq!(
        session.view().breakpoint_hit_count(97),
        legacy.breakpoint_hit_count(97)
    );
    assert_eq!(
        session.seek_entry(&mut budget),
        Err(SessionError::WorkLimit)
    );
    assert_eq!(session.view().cursor_record_index(), Some(0));
}
