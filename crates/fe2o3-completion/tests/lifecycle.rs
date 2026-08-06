use fe2o3_completion::{
    CancelRequestError, NotificationError, OperationLifecycle, OperationState, ReclaimError,
    TerminalState, TransitionError,
};
use std::cell::Cell;
use std::rc::Rc;

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
    assert_eq!(completed.complete(), Ok(()));
    assert_eq!(completed.state(), &OperationState::Completed);

    let (mut failed, _) = lifecycle();
    assert_eq!(failed.fail("kernel"), Ok(()));
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
    assert_eq!(completed.complete(), Ok(()));

    let (mut failed, _) = lifecycle();
    failed.request_cancel_with(|| Ok::<_, ()>(())).unwrap();
    assert_eq!(failed.fail("cancelled"), Ok(()));
    assert_eq!(failed.state(), &OperationState::Failed("cancelled"));
}

#[test]
fn terminal_states_reject_every_transition_without_callbacks() {
    for terminal in [TerminalState::Completed, TerminalState::Failed] {
        let (mut operation, _) = lifecycle();
        match terminal {
            TerminalState::Completed => operation.complete().unwrap(),
            TerminalState::Failed => operation.fail("kernel").unwrap(),
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
        assert_eq!(operation.complete(), Err(TransitionError { terminal }));
        assert_eq!(operation.fail("later"), Err(TransitionError { terminal }));

        let notify_called = Cell::new(false);
        assert_eq!(
            operation.complete_with_notification(|_| {
                notify_called.set(true);
                Ok::<_, ()>(())
            }),
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

    operation.complete().unwrap();
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
    completed.complete().unwrap();
    drop(completed);
    assert_eq!(completed_drops.get(), 1);

    let (mut failed, failed_drops) = lifecycle();
    failed.fail("kernel").unwrap();
    drop(failed);
    assert_eq!(failed_drops.get(), 1);
}

#[test]
fn callback_errors_preserve_terminal_state_and_reclamation() {
    let (mut completed, completed_drops) = lifecycle();
    assert_eq!(
        completed.complete_with_notification(|state| {
            assert_eq!(state, &OperationState::Completed);
            Err("callback")
        }),
        Err(NotificationError::Callback("callback"))
    );
    assert_eq!(completed.state(), &OperationState::Completed);
    drop(completed);
    assert_eq!(completed_drops.get(), 1);

    let (mut failed, failed_drops) = lifecycle();
    assert_eq!(
        failed.fail_with_notification("kernel", |state| {
            assert_eq!(state, &OperationState::Failed("kernel"));
            Err("callback")
        }),
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
                let _ = operation.fail_with_notification("kernel", |_| -> Result<(), ()> {
                    panic!("notification panic")
                });
            } else {
                let _ = operation.complete_with_notification(|_| -> Result<(), ()> {
                    panic!("notification panic")
                });
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
            operation.fail("kernel").unwrap();
        } else {
            operation.complete().unwrap();
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
