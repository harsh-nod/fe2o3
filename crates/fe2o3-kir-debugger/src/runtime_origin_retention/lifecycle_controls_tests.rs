//! Bounded negative controls. Fabricated events are explicitly model-only.
use super::super::session::Direction;
use super::*;
use crate::DebugSessionV1;
use fe2o3_kir_sim::*;

fn event(kind: SimulationEventKindV1) -> SimulationEventV1 {
    let (_, owner) = {
        let (m, r) = fixtures::private(false, false);
        capture(&m, &r, fixtures::schedule(false), fixtures::config(4096))
    };
    let record = &owner.view().transcript().records()[0];
    SimulationEventV1 {
        invocation: record.invocation,
        site: SimulationEventSiteV1 {
            function_ordinal: record.site.function_ordinal,
            block: record.site.block,
            operation: Some(record.site.operation),
        },
        kind,
    }
}
fn created() -> SimulationEventV1 {
    event(SimulationEventKindV1::AllocationCreated {
        allocation: 7,
        address_space: AddressSpace::Private,
        bytes: 4,
    })
}
fn release() -> SimulationEventV1 {
    event(SimulationEventKindV1::AllocationReleased { allocation: 7 })
}

#[test]
fn synthetic_unknown_duplicate_release_reused_id_and_reversed_boundary_refuse() {
    let create = created();
    let released = release();
    for events in [
        vec![(0, released.clone())],
        vec![
            (0, create.clone()),
            (1, released.clone()),
            (2, released.clone()),
        ],
        vec![
            (0, create.clone()),
            (1, released.clone()),
            (2, create.clone()),
        ],
        vec![(2, create.clone()), (1, released.clone())],
    ] {
        let mut ledger = Ledger::new(fixtures::limits(), true);
        for (boundary, event) in events {
            capture::synthetic_event(&mut ledger, boundary, &event);
        }
        assert_eq!(ledger.gap.unwrap().1, Gap::InvalidProducer);
    }
    let mut zero = create;
    zero.kind = SimulationEventKindV1::AllocationCreated {
        allocation: 0,
        address_space: AddressSpace::Private,
        bytes: 4,
    };
    let mut ledger = Ledger::new(fixtures::limits(), true);
    assert_eq!(
        capture::synthetic_event(&mut ledger, 0, &zero),
        SimulationEventSinkControlV1::DropAndStop
    );
    assert!(ledger.rows.is_empty());
}

#[test]
fn synthetic_exact_and_one_short_rows_bytes_work_preserve_retained_prefix() {
    let create = created();
    let released = release();
    let bytes = |rows| size_of::<Ledger>() + rows * size_of::<Transition>();
    let mut exact = Ledger::new(LifecycleLimits::new(2, bytes(2), 3).unwrap(), true);
    capture::synthetic_event(&mut exact, 0, &create);
    capture::synthetic_event(&mut exact, 1, &released);
    assert!(exact.gap.is_none());
    assert_eq!(exact.rows.len(), 2);
    assert_eq!(exact.work_left, 0);
    assert!(exact.metadata_bytes() <= bytes(2));
    for (limits, gap) in [
        (
            LifecycleLimits::new(1, bytes(1), 100).unwrap(),
            Gap::RowLimit,
        ),
        (
            LifecycleLimits::new(2, bytes(1), 100).unwrap(),
            Gap::ByteLimit,
        ),
        (
            LifecycleLimits::new(2, bytes(2), 2).unwrap(),
            Gap::WorkLimit,
        ),
    ] {
        let mut ledger = Ledger::new(limits, true);
        capture::synthetic_event(&mut ledger, 0, &create);
        assert_eq!(
            capture::synthetic_event(&mut ledger, 1, &released),
            SimulationEventSinkControlV1::DropAndStop
        );
        assert_eq!(ledger.gap, Some((1, gap)));
        assert_eq!(ledger.rows.len(), 1);
        assert_eq!(ledger.rows[0].action, Action::Create);
    }
}

#[test]
fn events_disabled_is_explicitly_unavailable_with_exact_legacy_behavior() {
    let (module, mut request) = fixtures::private(false, false);
    request.events = EventPolicyV1::Disabled;
    let (expected, legacy) = fixtures::baseline(&module, &request, false, fixtures::config(4096));
    let (result, mut owner) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    super::super::fixtures::assert_result_eq(&result, &expected);
    assert_eq!(owner.view().transcript(), &legacy);
    assert!(owner.transitions().is_empty());
    owner.seek_record(0, &mut fixtures::work()).unwrap();
    assert!(matches!(
        owner.current_allocation(1, &mut fixtures::work()),
        Err(QueryError::Gap(Gap::Disabled))
    ));
}

#[test]
fn lifecycle_cutoff_does_not_claim_absence_or_release_from_later_checkpoints() {
    let (module, request) = fixtures::private(false, false);
    let mut config = fixtures::config(4096);
    config.lifecycle =
        LifecycleLimits::new(1, size_of::<Ledger>() + size_of::<Transition>(), 100_000).unwrap();
    let (result, mut owner) = capture(&module, &request, fixtures::schedule(false), config);
    assert!(result.is_ok());
    assert_eq!(owner.transitions().len(), 1);
    assert_eq!(owner.transitions()[0].action, Action::Preexisting);
    owner
        .continue_to_stop(Direction::Forward, &mut fixtures::work())
        .unwrap();
    assert!(matches!(
        owner.current_allocation(2, &mut fixtures::work()),
        Err(QueryError::Gap(Gap::RowLimit))
    ));
}

#[test]
fn debug_prefix_cannot_become_a_completed_terminal_lifecycle_selection() {
    let (module, request) = fixtures::private(false, false);
    let (result, mut owner) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(1),
    );
    assert!(result.is_ok());
    assert_eq!(owner.view().transcript().records().len(), 1);
    assert_eq!(
        owner.continue_to_stop(Direction::Forward, &mut fixtures::work()),
        Err(SessionError::Incomplete)
    );
    assert!(owner.view().cursor_record_index().is_some_and(|i| i < 1));
}

#[test]
fn zero_query_work_preserves_cursor_counts_and_all_retained_storage() {
    let (module, request) = fixtures::private(false, false);
    let (_, mut owner) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    owner.seek_record(0, &mut fixtures::work()).unwrap();
    let index = owner.view().cursor_record_index();
    let storage = owner.transitions().as_ptr();
    let transcript = owner.view().transcript().records().as_ptr();
    let rows = owner.transitions().to_vec();
    assert!(matches!(
        owner.current_allocation(1, &mut ReplayWork::new(0).unwrap()),
        Err(QueryError::Work)
    ));
    assert_eq!(owner.view().cursor_record_index(), index);
    assert_eq!(owner.transitions(), rows);
    assert_eq!(owner.transitions().as_ptr(), storage);
    assert_eq!(owner.view().transcript().records().as_ptr(), transcript);
}

#[test]
fn equal_allocation_numbers_remain_borrowed_from_each_distinct_capture_owner() {
    let (module, request) = fixtures::private(false, false);
    let (_, mut first) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    let (_, mut second) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    first.seek_record(0, &mut fixtures::work()).unwrap();
    second.seek_record(0, &mut fixtures::work()).unwrap();
    {
        let left = first.current_allocation(1, &mut fixtures::work()).unwrap();
        let right = second.current_allocation(1, &mut fixtures::work()).unwrap();
        assert_eq!(left.latest, right.latest);
        assert!(!std::ptr::eq(left.latest.unwrap(), right.latest.unwrap()));
        assert!(!std::ptr::eq(
            left.checkpoint.unwrap(),
            right.checkpoint.unwrap()
        ));
        assert!(matches!(
            second.allocation_at_selection(&first.selection(), 1, &mut fixtures::work()),
            Err(QueryError::ForeignOwner)
        ));
        assert_eq!(
            first
                .allocation_at_selection(&first.selection(), 1, &mut fixtures::work())
                .unwrap()
                .state,
            State::PreexistingLive
        );
    }
    first
        .continue_to_stop(Direction::Forward, &mut fixtures::work())
        .unwrap();
    assert_eq!(second.view().cursor_record_index(), Some(0));
    // No session API accepts a detached lifecycle selection or raw sidecar.
}

#[test]
fn existing_break_watch_matchers_and_reverse_counts_remain_identical() {
    use super::super::session_tests::{breakpoint, watchpoint};
    let (module, request) = fixtures::private(false, false);
    let (_, legacy) = fixtures::baseline(&module, &request, false, fixtures::config(4096));
    let (_, mut owner) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    let selected = owner
        .view()
        .transcript()
        .records()
        .iter()
        .find(|r| r.site.function_ordinal == 1 && r.site.operation == 1)
        .unwrap();
    let bp = breakpoint(1, selected.site, selected.invocation);
    let wp = watchpoint(2, 1, 0, selected.invocation);
    let mut reference = DebugSessionV1::new(legacy);
    reference.add_breakpoint(bp.clone()).unwrap();
    reference.add_watchpoint(wp.clone()).unwrap();
    owner.add_breakpoint(bp, &mut fixtures::work()).unwrap();
    owner.add_watchpoint(wp, &mut fixtures::work()).unwrap();
    for direction in [Direction::Forward, Direction::Reverse] {
        for _ in 0..4 {
            let actual = owner
                .continue_to_stop(direction, &mut fixtures::work())
                .unwrap();
            let expected = if direction == Direction::Forward {
                reference.continue_forward_bounded(4096)
            } else {
                reference.continue_reverse()
            };
            assert_eq!(actual, expected);
            assert_eq!(
                owner.view().breakpoint_hit_count(1),
                reference.breakpoint_hit_count(1)
            );
            assert_eq!(
                owner.view().watchpoint_hit_count(2),
                reference.watchpoint_hit_count(2)
            );
        }
    }
    owner.seek_entry(&mut fixtures::work()).unwrap();
    assert_eq!(owner.view().cursor_record_index(), None);
    assert_eq!(owner.view().breakpoint_hit_count(1), Some(0));
    assert_eq!(owner.view().watchpoint_hit_count(2), Some(0));
}

#[test]
fn unchanged_hard_event_limit_remains_a_real_execution_failure() {
    let (module, request) = fixtures::private(false, false);
    let config = || {
        let mut c = fixtures::config(4096);
        c.simulation.max_events = 1;
        c
    };
    let (expected, legacy) = fixtures::baseline(&module, &request, false, config());
    let (result, owner) = capture(&module, &request, fixtures::schedule(false), config());
    super::super::fixtures::assert_result_eq(&result, &expected);
    assert_eq!(owner.view().transcript(), &legacy);
    assert!(matches!(result, Err(SimulationErrorV1::Execution(error))
        if matches!(error.kind, SimulationExecutionErrorKindV1::EventLimit { .. })));
}

#[test]
fn event_sink_failure_is_not_converted_into_semantic_negative_evidence() {
    struct Reject;
    impl SimulationEventSinkV1 for Reject {
        fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
            if matches!(event.kind, SimulationEventKindV1::AllocationCreated { .. }) {
                Err(SimulationEventSinkErrorV1 {
                    detail: "synthetic collector refusal".into(),
                })
            } else {
                Ok(())
            }
        }
    }
    let (module, request) = fixtures::private(false, false);
    let c = fixtures::config(4096);
    let result = module.simulate_debugged_scheduled_with_sinks(
        &request,
        c.target,
        c.simulation,
        fixtures::schedule(false),
        c.capture,
        &mut Reject,
        &mut NoopSimulationDebugSinkV1,
    );
    assert!(matches!(result, Err(SimulationErrorV1::Execution(error))
        if matches!(error.kind, SimulationExecutionErrorKindV1::EventSinkFailure(ref failure)
            if failure.detail == "synthetic collector refusal")));
}
