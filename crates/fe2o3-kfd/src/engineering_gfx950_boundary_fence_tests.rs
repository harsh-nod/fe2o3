use super::*;

#[test]
fn closed_program_retains_native_lifecycle_and_every_signal_at_all_boundaries() {
    for count in [1, 2, 63, 64, 65, 652, 1024] {
        let mut system = Fake {
            native_program: true,
            pending_polls: 1,
            ..Default::default()
        };
        let mut closed = Fake {
            boundary_fences: true,
            pending_polls: 1,
            ..Default::default()
        };
        run_ordered_batch_deadline_bounded(&mut system, count, 600_000, 1024, None).unwrap();
        run_ordered_batch_deadline_bounded(&mut closed, count, 600_000, 1024, None).unwrap();
        assert_eq!(closed.events, system.events);
        assert!(closed.completed && !closed.poisoned);
        assert_eq!(closed.retained, count);
        assert_eq!(closed.closed_headers.len(), count);
        if count == 1 {
            assert_eq!(closed.closed_headers, [0x1502]);
        } else {
            assert_eq!(closed.closed_headers[0], 0x0d02);
            assert_eq!(closed.closed_headers[count - 1], 0x1302);
            assert!(
                closed.closed_headers[1..count - 1]
                    .iter()
                    .all(|header| *header == 0x0b02)
            );
        }
        let final_poll = closed
            .events
            .iter()
            .rposition(|e| e == "poll_final_identity_counters_exception")
            .unwrap();
        let first_retire = closed
            .events
            .iter()
            .position(|e| e == "validate_signal:0")
            .unwrap();
        assert!(first_retire > final_poll);
        assert_eq!(
            closed
                .events
                .iter()
                .filter(|e| e.starts_with("validate_signal:"))
                .count(),
            count
        );
    }
}

#[test]
fn closed_program_every_preparation_publication_and_retirement_fault_is_terminal() {
    let count = 65;
    let mut success = Fake {
        boundary_fences: true,
        ..Default::default()
    };
    run_ordered_batch_deadline_bounded(&mut success, count, 600_000, 1024, None).unwrap();
    let completion_step = success
        .events
        .iter()
        .position(|event| event == "complete_frontier")
        .unwrap();
    for fail_at in 0..success.events.len() {
        let mut failed = Fake {
            boundary_fences: true,
            fail_at: Some(fail_at),
            ..Default::default()
        };
        assert!(
            run_ordered_batch_deadline_bounded(&mut failed, count, 600_000, 1024, None).is_err()
        );
        assert!(failed.poisoned);
        assert_eq!(failed.completed, fail_at > completion_step);
        assert_eq!(failed.events, success.events[..=fail_at]);
        if failed
            .events
            .iter()
            .any(|event| event == "write_reservation")
        {
            assert_eq!(failed.retained, count);
        }
        assert!(
            run_ordered_batch_deadline_bounded(&mut failed, count, 600_000, 1024, None).is_err()
        );
        assert_eq!(failed.events.len(), fail_at + 1);
    }
}

#[test]
fn closed_program_late_invalid_dispatch_expiry_and_count_publish_nothing() {
    let mut failed = Fake {
        boundary_fences: true,
        fault_after_prepare: Some((651, PreparationFault::Frontier)),
        ..Default::default()
    };
    assert!(run_ordered_batch_deadline_bounded(&mut failed, 652, 600_000, 1024, None).is_err());
    assert!(failed.poisoned && !failed.completed);
    assert_eq!(failed.retained, 0);
    assert!(!failed.events.iter().any(|event| event.starts_with("body:")));
    for count in [0, 1025] {
        let mut failed = Fake {
            boundary_fences: true,
            ..Default::default()
        };
        assert!(
            run_ordered_batch_deadline_bounded(&mut failed, count, 600_000, 1024, None).is_err()
        );
        assert!(failed.poisoned && failed.events.is_empty());
    }
    let mut expired = Fake {
        boundary_fences: true,
        ..Default::default()
    };
    assert!(
        run_ordered_batch_deadline_bounded(&mut expired, 652, 600_000, 1024, Some(Instant::now()))
            .is_err()
    );
    assert!(expired.poisoned && !expired.completed);
    assert_eq!(expired.retained, 0);
}
