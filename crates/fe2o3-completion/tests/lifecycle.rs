use fe2o3_completion::{
    BoundedQuarantine, CancelRequestError, ConcurrentOperationLifecycle, LeakReason,
    MAX_QUARANTINED_OPERATIONS, NotificationError, OperationLifecycle, OperationState,
    PoisonedLifecycle, ReclaimError, RetentionOutcome, SynchronizedLifecycleError, TerminalState,
    TransitionError,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

#[derive(Debug)]
struct DropCounter(Rc<Cell<usize>>);

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn lifecycle() -> (
    OperationLifecycle<DropCounter, &'static str>,
    Rc<Cell<usize>>,
) {
    let drops = Rc::new(Cell::new(0));
    (
        OperationLifecycle::submitted(DropCounter(drops.clone())),
        drops,
    )
}

#[test]
fn submitted_allows_each_nonterminal_transition() {
    let (mut cancel, _) = lifecycle();
    assert_eq!(cancel.request_cancel_with(|| Ok::<_, ()>(())), Ok(true));
    assert_eq!(cancel.state(), &OperationState::CancelRequested);

    let (mut completed, _) = lifecycle();
    // SAFETY: No backend work exists in this lifecycle unit test.
    assert_eq!(unsafe { completed.complete() }, Ok(()));
    assert_eq!(completed.state(), &OperationState::Completed);

    let (mut failed, _) = lifecycle();
    // SAFETY: No backend work exists in this lifecycle unit test.
    assert_eq!(unsafe { failed.fail("kernel") }, Ok(()));
    assert_eq!(failed.state(), &OperationState::Failed("kernel"));
}

#[test]
fn cancel_requested_can_only_become_terminal() {
    let requests = Cell::new(0);
    let (mut completed, _) = lifecycle();
    completed
        .request_cancel_with(|| {
            requests.set(requests.get() + 1);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        completed.request_cancel_with(|| {
            requests.set(requests.get() + 1);
            Ok::<_, ()>(())
        }),
        Ok(false)
    );
    assert_eq!(requests.get(), 1);
    // SAFETY: No backend work exists in this lifecycle unit test.
    assert_eq!(unsafe { completed.complete() }, Ok(()));

    let (mut failed, _) = lifecycle();
    failed.request_cancel_with(|| Ok::<_, ()>(())).unwrap();
    // SAFETY: No backend work exists in this lifecycle unit test.
    assert_eq!(unsafe { failed.fail("cancelled") }, Ok(()));
    assert_eq!(failed.state(), &OperationState::Failed("cancelled"));
}

#[test]
fn terminal_states_reject_every_transition_without_callbacks() {
    for terminal in [
        TerminalState::Completed,
        TerminalState::Failed,
        TerminalState::StreamQuiesced,
        TerminalState::Quarantined,
        TerminalState::Leaked,
    ] {
        let (mut operation, _) = lifecycle();
        let mut quarantine = BoundedQuarantine::new(if terminal == TerminalState::Leaked {
            0
        } else {
            1
        })
        .unwrap();
        // SAFETY: No backend work exists in this lifecycle unit test.
        unsafe {
            match terminal {
                TerminalState::Completed => operation.complete().unwrap(),
                TerminalState::Failed => operation.fail("kernel").unwrap(),
                TerminalState::StreamQuiesced => operation.mark_stream_quiesced().unwrap(),
                TerminalState::Quarantined | TerminalState::Leaked => {
                    operation.abandon_to_quarantine(&mut quarantine).unwrap();
                }
            }
        }

        let cancel_called = Cell::new(false);
        assert_eq!(
            operation.request_cancel_with(|| {
                cancel_called.set(true);
                Ok::<_, ()>(())
            }),
            Err(CancelRequestError::Terminal(terminal))
        );
        assert!(!cancel_called.get());
        // SAFETY: The operation is already terminal, so these calls cannot release resources.
        unsafe {
            assert_eq!(operation.complete(), Err(TransitionError { terminal }));
            assert_eq!(operation.fail("later"), Err(TransitionError { terminal }));
        }

        let notify_called = Cell::new(false);
        // SAFETY: The operation is already terminal, so this call cannot release resources.
        assert_eq!(
            unsafe {
                operation.complete_with_notification(|_| {
                    notify_called.set(true);
                    Ok::<_, ()>(())
                })
            },
            Err(NotificationError::Transition(TransitionError { terminal }))
        );
        assert!(!notify_called.get());
    }
}

#[test]
fn cancellation_never_authorizes_reclamation() {
    let (mut operation, drops) = lifecycle();
    operation.request_cancel_with(|| Ok::<_, ()>(())).unwrap();

    assert!(matches!(
        operation.reclaim(),
        Err(ReclaimError::NotQuiescent)
    ));
    assert_eq!(drops.get(), 0);

    // SAFETY: No backend work exists in this lifecycle unit test.
    unsafe { operation.complete() }.unwrap();
    drop(operation.reclaim().unwrap());
    assert_eq!(drops.get(), 1);
    assert!(matches!(
        operation.reclaim(),
        Err(ReclaimError::AlreadyReclaimed)
    ));
    drop(operation);
    assert_eq!(drops.get(), 1);
}

#[test]
fn cancellation_request_failure_and_panic_leave_work_submitted() {
    let (mut failed, failed_drops) = lifecycle();
    assert_eq!(
        failed.request_cancel_with(|| Err("request")),
        Err(CancelRequestError::Request("request"))
    );
    assert_eq!(failed.state(), &OperationState::Submitted);
    drop(failed);
    assert_eq!(failed_drops.get(), 0);

    let panic_drops = Rc::new(Cell::new(0));
    let panic_resource = panic_drops.clone();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let mut operation =
            OperationLifecycle::<_, &'static str>::submitted(DropCounter(panic_resource));
        let _ = operation.request_cancel_with(|| -> Result<(), ()> { panic!("cancel panic") });
    }));
    assert!(panic.is_err());
    assert_eq!(panic_drops.get(), 0);
}

#[test]
fn nonterminal_drop_retains_resources_but_terminal_drop_reclaims_once() {
    let (submitted, submitted_drops) = lifecycle();
    drop(submitted);
    assert_eq!(submitted_drops.get(), 0);

    let (mut cancelled, cancelled_drops) = lifecycle();
    cancelled.request_cancel_with(|| Ok::<_, ()>(())).unwrap();
    drop(cancelled);
    assert_eq!(cancelled_drops.get(), 0);

    let (mut completed, completed_drops) = lifecycle();
    // SAFETY: No backend work exists in this lifecycle unit test.
    unsafe { completed.complete() }.unwrap();
    drop(completed);
    assert_eq!(completed_drops.get(), 1);

    let (mut failed, failed_drops) = lifecycle();
    // SAFETY: No backend work exists in this lifecycle unit test.
    unsafe { failed.fail("kernel") }.unwrap();
    drop(failed);
    assert_eq!(failed_drops.get(), 1);
}

#[test]
fn callback_errors_preserve_terminal_state_and_reclamation() {
    let (mut completed, completed_drops) = lifecycle();
    assert_eq!(
        unsafe {
            // SAFETY: No backend work exists in this lifecycle unit test.
            completed.complete_with_notification(|state| {
                assert_eq!(state, &OperationState::Completed);
                Err("callback")
            })
        },
        Err(NotificationError::Callback("callback"))
    );
    assert_eq!(completed.state(), &OperationState::Completed);
    drop(completed);
    assert_eq!(completed_drops.get(), 1);

    let (mut failed, failed_drops) = lifecycle();
    assert_eq!(
        unsafe {
            // SAFETY: No backend work exists in this lifecycle unit test.
            failed.fail_with_notification("kernel", |state| {
                assert_eq!(state, &OperationState::Failed("kernel"));
                Err("callback")
            })
        },
        Err(NotificationError::Callback("callback"))
    );
    assert_eq!(failed.state(), &OperationState::Failed("kernel"));
    drop(failed);
    assert_eq!(failed_drops.get(), 1);
}

#[test]
fn callback_panics_still_reclaim_terminal_resources_exactly_once() {
    for fail in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let resource_drops = drops.clone();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let mut operation = OperationLifecycle::submitted(DropCounter(resource_drops));
            if fail {
                // SAFETY: No backend work exists in this lifecycle unit test.
                let _ = unsafe {
                    operation.fail_with_notification("kernel", |_| -> Result<(), ()> {
                        panic!("notification panic")
                    })
                };
            } else {
                // SAFETY: No backend work exists in this lifecycle unit test.
                let _ = unsafe {
                    operation.complete_with_notification(|_| -> Result<(), ()> {
                        panic!("notification panic")
                    })
                };
            }
        }));
        assert!(panic.is_err());
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn explicit_reclamation_is_exactly_once_for_both_terminal_states() {
    for fail in [false, true] {
        let (mut operation, drops) = lifecycle();
        if fail {
            // SAFETY: No backend work exists in this lifecycle unit test.
            unsafe { operation.fail("kernel") }.unwrap();
        } else {
            // SAFETY: No backend work exists in this lifecycle unit test.
            unsafe { operation.complete() }.unwrap();
        }

        let resources = operation.reclaim().unwrap();
        assert!(matches!(
            operation.reclaim(),
            Err(ReclaimError::AlreadyReclaimed)
        ));
        assert_eq!(drops.get(), 0);
        drop(resources);
        assert_eq!(drops.get(), 1);
        drop(operation);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn stream_quiescence_is_stronger_than_cancellation_and_authorizes_reclamation() {
    let (mut operation, drops) = lifecycle();
    operation.request_cancel_with(|| Ok::<_, ()>(())).unwrap();
    assert!(matches!(
        operation.reclaim(),
        Err(ReclaimError::NotQuiescent)
    ));

    // SAFETY: No backend work exists in this lifecycle unit test.
    unsafe { operation.mark_stream_quiesced() }.unwrap();
    assert_eq!(operation.state(), &OperationState::StreamQuiesced);
    drop(operation.reclaim().unwrap());
    assert_eq!(drops.get(), 1);
}

#[test]
fn stream_notification_failure_preserves_quiescence() {
    let (mut operation, drops) = lifecycle();
    assert_eq!(
        // SAFETY: No backend work exists in this lifecycle unit test.
        unsafe {
            operation.stream_quiesced_with_notification(|state| {
                assert_eq!(state, &OperationState::StreamQuiesced);
                Err("callback")
            })
        },
        Err(NotificationError::Callback("callback"))
    );
    drop(operation);
    assert_eq!(drops.get(), 1);
}

#[test]
fn bounded_quarantine_records_transfer_and_overflow_leak() {
    let mut quarantine = BoundedQuarantine::new(1).unwrap();
    let (mut quarantined, quarantined_drops) = lifecycle();
    quarantined.request_cancel_with(|| Ok::<_, ()>(())).unwrap();
    let ticket = match quarantined.abandon_to_quarantine(&mut quarantine).unwrap() {
        RetentionOutcome::Quarantined(ticket) => ticket,
        RetentionOutcome::Leaked(reason) => panic!("unexpected leak: {reason:?}"),
    };
    assert_eq!(ticket.get(), 0);
    assert_eq!(quarantine.len(), 1);
    assert!(matches!(
        quarantined.reclaim(),
        Err(ReclaimError::Quarantined(observed)) if observed == ticket
    ));
    drop(quarantined);
    assert_eq!(quarantined_drops.get(), 0);

    let (mut leaked, leaked_drops) = lifecycle();
    assert_eq!(
        leaked.abandon_to_quarantine(&mut quarantine),
        Ok(RetentionOutcome::Leaked(LeakReason::QuarantineFull))
    );
    assert_eq!(
        leaked.state(),
        &OperationState::Leaked(LeakReason::QuarantineFull)
    );
    assert!(matches!(
        leaked.reclaim(),
        Err(ReclaimError::Leaked(LeakReason::QuarantineFull))
    ));
    drop(leaked);
    assert_eq!(leaked_drops.get(), 0);

    // SAFETY: No backend work exists in this lifecycle unit test.
    let resources = unsafe { quarantine.release_after_stream_quiescence() };
    assert!(quarantine.is_empty());
    drop(resources);
    assert_eq!(quarantined_drops.get(), 1);
}

#[test]
fn dropping_quarantine_leaks_and_capacity_is_strictly_bounded() {
    assert_eq!(
        BoundedQuarantine::<DropCounter>::new(MAX_QUARANTINED_OPERATIONS + 1)
            .unwrap_err()
            .maximum,
        MAX_QUARANTINED_OPERATIONS
    );

    let (mut operation, drops) = lifecycle();
    let mut quarantine = BoundedQuarantine::new(1).unwrap();
    operation.abandon_to_quarantine(&mut quarantine).unwrap();
    drop(operation);
    drop(quarantine);
    assert_eq!(drops.get(), 0);
}

#[test]
fn terminal_sequence_is_first_observation_wins() {
    let (mut operation, _) = lifecycle();
    operation.request_cancel_with(|| Ok::<_, ()>(())).unwrap();
    // SAFETY: No backend work exists in this lifecycle unit test.
    unsafe { operation.fail("cancelled") }.unwrap();

    assert_eq!(
        // SAFETY: The lifecycle is already terminal and cannot release resources twice.
        unsafe { operation.mark_stream_quiesced() },
        Err(TransitionError {
            terminal: TerminalState::Failed
        })
    );
    assert_eq!(
        operation.request_cancel_with(|| panic!("terminal callback ran")),
        Err(CancelRequestError::<()>::Terminal(TerminalState::Failed))
    );
    assert_eq!(operation.state(), &OperationState::Failed("cancelled"));
}

#[derive(Debug)]
struct ThreadDropCounter(Arc<AtomicUsize>);

impl Drop for ThreadDropCounter {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn concurrent_lifecycle() -> (
    ConcurrentOperationLifecycle<ThreadDropCounter, &'static str>,
    Arc<AtomicUsize>,
) {
    let drops = Arc::new(AtomicUsize::new(0));
    (
        ConcurrentOperationLifecycle::submitted(ThreadDropCounter(Arc::clone(&drops))),
        drops,
    )
}

#[test]
fn concurrent_cancellation_callback_runs_at_most_once() {
    let (operation, drops) = concurrent_lifecycle();
    let calls = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(17));
    let mut threads = Vec::new();

    for _ in 0..16 {
        let operation = operation.clone();
        let calls = Arc::clone(&calls);
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            operation.request_cancel_with(|| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(())
            })
        }));
    }
    barrier.wait();

    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|accepted| **accepted).count(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(operation.state(), Ok(OperationState::CancelRequested));
    drop(operation);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn racing_terminal_notifications_have_one_winner_and_one_callback() {
    let (operation, drops) = concurrent_lifecycle();
    let calls = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(17));
    let mut threads = Vec::new();

    for _ in 0..16 {
        let operation = operation.clone();
        let calls = Arc::clone(&calls);
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            // SAFETY: No backend work exists in this lifecycle unit test.
            unsafe {
                operation.complete_with_notification(|state| {
                    assert_eq!(state, &OperationState::Completed);
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, ()>(())
                })
            }
        }));
    }
    barrier.wait();

    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    for result in results.into_iter().filter(|result| result.is_err()) {
        assert_eq!(
            result,
            Err(SynchronizedLifecycleError::Lifecycle(
                NotificationError::Transition(TransitionError {
                    terminal: TerminalState::Completed
                })
            ))
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(operation.reclaim().unwrap());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn racing_success_and_failure_are_linearizable() {
    let (operation, drops) = concurrent_lifecycle();
    let barrier = Arc::new(Barrier::new(3));

    let complete = {
        let operation = operation.clone();
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            // SAFETY: No backend work exists in this lifecycle unit test.
            unsafe { operation.complete() }
        })
    };
    let fail = {
        let operation = operation.clone();
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            // SAFETY: No backend work exists in this lifecycle unit test.
            unsafe { operation.fail("kernel") }
        })
    };
    barrier.wait();

    let complete = complete.join().unwrap();
    let fail = fail.join().unwrap();
    assert_ne!(complete.is_ok(), fail.is_ok());
    let state = operation.state().unwrap();
    let terminal = state.terminal_state().unwrap();
    assert!(matches!(
        terminal,
        TerminalState::Completed | TerminalState::Failed
    ));
    let loser_terminal = if let Err(complete_error) = complete {
        match complete_error {
            SynchronizedLifecycleError::Lifecycle(error) => error.terminal,
            SynchronizedLifecycleError::Poisoned => panic!("unexpected poison"),
        }
    } else {
        match fail.unwrap_err() {
            SynchronizedLifecycleError::Lifecycle(error) => error.terminal,
            SynchronizedLifecycleError::Poisoned => panic!("unexpected poison"),
        }
    };
    assert_eq!(loser_terminal, terminal);
    drop(operation.reclaim().unwrap());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn racing_reclamation_returns_resources_exactly_once() {
    let (operation, drops) = concurrent_lifecycle();
    // SAFETY: No backend work exists in this lifecycle unit test.
    unsafe { operation.mark_stream_quiesced() }.unwrap();

    let barrier = Arc::new(Barrier::new(9));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let operation = operation.clone();
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            operation.reclaim()
        }));
    }
    barrier.wait();

    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    for resource in results.into_iter().flatten() {
        drop(resource);
    }
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn poisoned_nonterminal_lifecycle_fails_closed_and_leaks() {
    let (operation, drops) = concurrent_lifecycle();
    let poisoner = operation.clone();
    let panic = std::panic::catch_unwind(move || {
        let _ = poisoner.inspect(|_| panic!("poison lifecycle mutex"));
    });
    assert!(panic.is_err());
    assert_eq!(operation.state(), Err(PoisonedLifecycle));
    assert!(matches!(
        operation.reclaim(),
        Err(SynchronizedLifecycleError::Poisoned)
    ));
    drop(operation);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn terminal_callback_panic_runs_outside_lock_and_preserves_reclamation() {
    let (operation, drops) = concurrent_lifecycle();
    let notifier = operation.clone();
    let panic = std::panic::catch_unwind(move || {
        // SAFETY: No backend work exists in this lifecycle unit test.
        let _ = unsafe {
            notifier
                .complete_with_notification(|_| -> Result<(), ()> { panic!("notification panic") })
        };
    });
    assert!(panic.is_err());
    assert_eq!(operation.state(), Ok(OperationState::Completed));
    drop(operation.reclaim().unwrap());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn cancellation_callback_panic_poison_fails_closed() {
    let (operation, drops) = concurrent_lifecycle();
    let requester = operation.clone();
    let panic = std::panic::catch_unwind(move || {
        let _ = requester
            .request_cancel_with(|| -> Result<(), ()> { panic!("cancellation callback panic") });
    });
    assert!(panic.is_err());
    assert_eq!(operation.state(), Err(PoisonedLifecycle));
    assert!(matches!(
        operation.reclaim(),
        Err(SynchronizedLifecycleError::Poisoned)
    ));
    drop(operation);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}
