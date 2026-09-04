use alloc::vec::Vec;

use super::*;

fn config(capacity: usize, poll_budget: usize, flush_budget: usize) -> R24PortableProgressConfigV1 {
    R24PortableProgressConfigV1 {
        capacity,
        poll_budget,
        flush_budget,
    }
}

fn key(id: u64) -> R24PortableProgressKeyV1 {
    R24PortableProgressKeyV1 {
        context_generation: 24,
        event_id: 100 + id,
        stream_id: 200 + id,
    }
}

fn request(id: u64, total_packets: u16) -> R24PortableRegistrationRequestV1 {
    R24PortableRegistrationRequestV1 {
        key: key(id),
        total_packets,
    }
}

fn pending(key: R24PortableProgressKeyV1) -> R24PollStepV1 {
    R24PollStepV1 {
        key,
        disposition: R24PollDispositionV1::Pending,
    }
}

#[test]
fn configuration_and_registration_bounds_fail_atomically() {
    for invalid in [
        config(0, 1, 1),
        config(R24_MAX_REGISTRATIONS_V1 + 1, 1, 1),
        config(1, 0, 1),
        config(1, 1, R24_MAX_PROGRESS_BUDGET_V1 + 1),
    ] {
        assert!(matches!(
            R24PortableProgressModelV1::new_model_only(invalid),
            Err(R24PortableProgressErrorV1::InvalidConfiguration)
        ));
    }
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    let before = model.snapshot();
    assert_eq!(
        model.register_model_only(request(1, 0)),
        Err(R24PortableProgressErrorV1::InvalidRegistration)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn event_and_stream_registration_is_atomic_with_no_half_install() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(2, 1, 1)).unwrap();
    let before = model.snapshot();
    assert_eq!(
        model.register_with_disposition_model_only(
            request(1, 65),
            R24RegistrationDispositionV1::RejectAfterEventPreflight,
        ),
        Err(R24PortableProgressErrorV1::InvalidRegistration)
    );
    assert_eq!(model.snapshot(), before);
    model.register_model_only(request(1, 65)).unwrap();
    let registration = model.snapshot().registrations[0];
    assert!(registration.event_installed);
    assert!(registration.stream_installed);
    assert!(registration.custody_retained);
}

#[test]
fn duplicate_and_capacity_rejection_preserve_exact_registry() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    model.register_model_only(request(1, 1)).unwrap();
    let installed = model.snapshot();
    assert_eq!(
        model.register_model_only(request(1, 1)),
        Err(R24PortableProgressErrorV1::DuplicateEvent)
    );
    assert_eq!(model.snapshot(), installed);
    assert_eq!(
        model.register_model_only(request(2, 1)),
        Err(R24PortableProgressErrorV1::CapacityExceeded)
    );
    assert_eq!(model.snapshot(), installed);
}

#[test]
fn active_event_and_stream_duplicates_are_rejected_independently() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(3, 1, 1)).unwrap();
    model.register_model_only(request(1, 1)).unwrap();
    let installed = model.snapshot();
    let duplicate_event = R24PortableRegistrationRequestV1 {
        key: R24PortableProgressKeyV1 {
            stream_id: key(2).stream_id,
            ..key(1)
        },
        total_packets: 1,
    };
    assert_eq!(
        model.register_model_only(duplicate_event),
        Err(R24PortableProgressErrorV1::DuplicateEvent)
    );
    assert_eq!(model.snapshot(), installed);

    let duplicate_stream = R24PortableRegistrationRequestV1 {
        key: R24PortableProgressKeyV1 {
            stream_id: key(1).stream_id,
            ..key(2)
        },
        total_packets: 1,
    };
    assert_eq!(
        model.register_model_only(duplicate_stream),
        Err(R24PortableProgressErrorV1::DuplicateStream)
    );
    assert_eq!(model.snapshot(), installed);
}

#[test]
fn retired_history_allows_exact_reregistration_and_targets_active_occurrence() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();

    model.register_model_only(request(1, 1)).unwrap();
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Retryable,
        }])
        .unwrap();
    model.register_model_only(request(1, 1)).unwrap();
    model.abandon_model_only(key(1)).unwrap();
    model.register_model_only(request(1, 1)).unwrap();
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }])
        .unwrap();
    model.register_model_only(request(1, 1)).unwrap();

    let snapshot = model.snapshot();
    assert_eq!(snapshot.registrations.len(), 4);
    assert_eq!(
        snapshot
            .registrations
            .iter()
            .filter(|entry| entry.observing)
            .count(),
        1
    );
    assert_eq!(
        snapshot
            .registrations
            .iter()
            .filter(|entry| entry.abandoned)
            .count(),
        1
    );
    assert_eq!(
        snapshot
            .registrations
            .iter()
            .filter(|entry| entry.phase.is_terminal())
            .count(),
        1
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn poll_and_flush_budgets_are_independent_and_bounded() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(3, 2, 1)).unwrap();
    for id in 1..=3 {
        model.register_model_only(request(id, 65)).unwrap();
    }
    assert_eq!(
        model.poll_budget_model_only(&[pending(key(1)), pending(key(2)), pending(key(3))]),
        Err(R24PortableProgressErrorV1::BudgetExceeded)
    );
    model
        .poll_budget_model_only(&[
            R24PollStepV1 {
                key: key(1),
                disposition: R24PollDispositionV1::Completed,
            },
            R24PollStepV1 {
                key: key(2),
                disposition: R24PollDispositionV1::Completed,
            },
        ])
        .unwrap();
    assert_eq!(
        model.flush_budget_model_only(&[
            R24FlushStepV1 {
                key: key(1),
                disposition: R24FlushDispositionV1::Retryable,
            },
            R24FlushStepV1 {
                key: key(2),
                disposition: R24FlushDispositionV1::Retryable,
            },
        ]),
        Err(R24PortableProgressErrorV1::BudgetExceeded)
    );
    let state = model.snapshot();
    assert_eq!(state.poll_visits, 2);
    assert_eq!(state.flush_visits, 0);
}

#[test]
fn poll_visitation_is_stable_and_cyclic() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(3, 2, 1)).unwrap();
    for id in 1..=3 {
        model.register_model_only(request(id, 1)).unwrap();
    }
    assert_eq!(
        model
            .poll_budget_model_only(&[pending(key(1)), pending(key(2))])
            .unwrap(),
        [key(1), key(2)]
    );
    assert_eq!(
        model
            .poll_budget_model_only(&[pending(key(3)), pending(key(1))])
            .unwrap(),
        [key(3), key(1)]
    );
    assert_eq!(model.snapshot().poll_cursor, 1);
}

#[test]
fn substituted_visit_roster_rejects_before_any_progress() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(2, 2, 1)).unwrap();
    model.register_model_only(request(1, 1)).unwrap();
    model.register_model_only(request(2, 1)).unwrap();
    let before = model.snapshot();
    assert_eq!(
        model.poll_budget_model_only(&[pending(key(2)), pending(key(1))]),
        Err(R24PortableProgressErrorV1::VisitSubstitution)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn sixty_three_plus_two_requires_poll_before_continuation_flush() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    model.register_model_only(request(1, 65)).unwrap();
    assert_eq!(
        model.snapshot().registrations[0].phase,
        R24ProgressPhaseV1::WindowPending {
            ordinal: 0,
            packet_count: 63,
        }
    );
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }])
        .unwrap();
    assert_eq!(
        model.snapshot().registrations[0].phase,
        R24ProgressPhaseV1::ContinuationReady {
            completed_packets: 63,
            next_packet_count: 2,
            polled_before_continuation: true,
        }
    );
    model
        .flush_budget_model_only(&[R24FlushStepV1 {
            key: key(1),
            disposition: R24FlushDispositionV1::Published,
        }])
        .unwrap();
    assert_eq!(
        model.snapshot().registrations[0].phase,
        R24ProgressPhaseV1::WindowPending {
            ordinal: 1,
            packet_count: 2,
        }
    );
}

#[test]
fn retryable_poll_retires_progress_but_preserves_resource_custody_and_phase() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    model.register_model_only(request(1, 65)).unwrap();
    let before = model.snapshot().registrations[0];
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Retryable,
        }])
        .unwrap();
    let after = model.snapshot().registrations[0];
    assert_eq!(after.phase, before.phase);
    assert_eq!(after.event_installed, before.event_installed);
    assert_eq!(after.stream_installed, before.stream_installed);
    assert_eq!(after.custody_retained, before.custody_retained);
    assert!(!after.observing);
    assert!(!after.abandoned);
}

#[test]
fn retryable_flush_preserves_registration_custody_and_continuation() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    model.register_model_only(request(1, 65)).unwrap();
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }])
        .unwrap();
    let before = model.snapshot().registrations[0];
    model
        .flush_budget_model_only(&[R24FlushStepV1 {
            key: key(1),
            disposition: R24FlushDispositionV1::Retryable,
        }])
        .unwrap();
    assert_eq!(model.snapshot().registrations[0], before);
}

#[test]
fn terminal_success_and_failure_retire_progress_and_retain_resource_custody() {
    for disposition in [
        R24PollDispositionV1::Completed,
        R24PollDispositionV1::TerminalFailure,
    ] {
        let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
        model.register_model_only(request(1, 1)).unwrap();
        model
            .poll_budget_model_only(&[R24PollStepV1 {
                key: key(1),
                disposition,
            }])
            .unwrap();
        let terminal = model.snapshot().registrations[0];
        assert!(terminal.phase.is_terminal());
        assert!(!terminal.observing);
        assert!(terminal.event_installed && terminal.stream_installed && terminal.custody_retained);
        assert_eq!(
            model.poll_budget_model_only(&[pending(key(1))]),
            Err(R24PortableProgressErrorV1::BudgetExceeded)
        );
        assert_eq!(model.snapshot().registrations[0], terminal);
    }
}

#[test]
fn abandon_and_drop_are_observation_only() {
    for drop in [false, true] {
        let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
        model.register_model_only(request(1, 65)).unwrap();
        let before = model.snapshot().registrations[0];
        if drop {
            model.drop_observation_model_only(key(1)).unwrap();
        } else {
            model.abandon_model_only(key(1)).unwrap();
        }
        let after = model.snapshot().registrations[0];
        assert_eq!(after.phase, before.phase);
        assert_eq!(after.event_installed, before.event_installed);
        assert_eq!(after.stream_installed, before.stream_installed);
        assert_eq!(after.custody_retained, before.custody_retained);
        assert!(!after.observing);
        assert!(after.abandoned);
    }
}

#[test]
fn stop_performs_no_final_poll_or_flush_progress() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(2, 2, 2)).unwrap();
    model.register_model_only(request(1, 65)).unwrap();
    model.register_model_only(request(2, 1)).unwrap();
    let before = model.snapshot();
    model.stop_model_only();
    let stopped = model.snapshot();
    assert!(stopped.stopped);
    assert_eq!(stopped.poll_visits, before.poll_visits);
    assert_eq!(stopped.flush_visits, before.flush_visits);
    assert_eq!(
        stopped
            .registrations
            .iter()
            .map(|entry| entry.phase)
            .collect::<Vec<_>>(),
        before
            .registrations
            .iter()
            .map(|entry| entry.phase)
            .collect::<Vec<_>>()
    );
    assert!(stopped.registrations.iter().all(|entry| {
        !entry.observing
            && entry.event_installed
            && entry.stream_installed
            && entry.custody_retained
    }));
    assert_eq!(
        model.poll_budget_model_only(&[]),
        Err(R24PortableProgressErrorV1::EngineStopped)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn tail_poll_completes_without_an_extra_continuation() {
    let mut model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    model.register_model_only(request(1, 65)).unwrap();
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }])
        .unwrap();
    model
        .flush_budget_model_only(&[R24FlushStepV1 {
            key: key(1),
            disposition: R24FlushDispositionV1::Published,
        }])
        .unwrap();
    model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }])
        .unwrap();
    assert_eq!(
        model.snapshot().registrations[0].phase,
        R24ProgressPhaseV1::TerminalSucceeded
    );
    assert!(!model.snapshot().registrations[0].observing);
}

#[test]
fn visit_counter_overflow_is_failure_atomic_before_poll_or_flush_mutation() {
    let mut poll_model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    poll_model.register_model_only(request(1, 65)).unwrap();
    poll_model.set_visit_counts_for_test_model_only(u64::MAX, 0);
    let before_poll = poll_model.snapshot();
    assert_eq!(
        poll_model.poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }]),
        Err(R24PortableProgressErrorV1::InvariantViolation)
    );
    assert_eq!(poll_model.snapshot(), before_poll);

    let mut flush_model = R24PortableProgressModelV1::new_model_only(config(1, 1, 1)).unwrap();
    flush_model.register_model_only(request(1, 65)).unwrap();
    flush_model
        .poll_budget_model_only(&[R24PollStepV1 {
            key: key(1),
            disposition: R24PollDispositionV1::Completed,
        }])
        .unwrap();
    flush_model.set_visit_counts_for_test_model_only(1, u64::MAX);
    let before_flush = flush_model.snapshot();
    assert_eq!(
        flush_model.flush_budget_model_only(&[R24FlushStepV1 {
            key: key(1),
            disposition: R24FlushDispositionV1::Published,
        }]),
        Err(R24PortableProgressErrorV1::InvariantViolation)
    );
    assert_eq!(flush_model.snapshot(), before_flush);
}
