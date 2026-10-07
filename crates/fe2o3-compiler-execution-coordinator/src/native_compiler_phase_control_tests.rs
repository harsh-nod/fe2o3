//! Scheduler controls substitute effects, never original protected authority.
use super::*;

fn completed_fake() -> Fake {
    let mut fake = Fake::new();
    fake.intake = true;
    fake.signal_at = usize::MAX;
    fake.foreground = true;
    fake.foreground_pending = 1;
    fake.busy = 1;
    fake
}

#[test]
fn continuation_retains_original_frame_after_full_phase_retirement() {
    let fake = completed_fake();
    let trace = fake.trace.clone();
    let mut request = Account::new(Work::new(usize::MAX), FRAME + RETAINED + 13);
    request.with_budget(|b| {
        b.reserve_storage(13).unwrap();
        let ledger = b.work_ledger_identity_v1();
        run_scoped_with(
            b,
            3,
            3,
            || Ok(fake),
            |runtime, outcome, b| {
                assert!(outcome == MonitorOutcome::CompletionReady);
                assert!(ledger == b.work_ledger_identity_v1());
                assert_eq!(b.storage(), FRAME + RETAINED + 13);
                assert_eq!(runtime.foreground_pending, 0);
                assert_eq!(runtime.busy, 0);
                assert_eq!(runtime.shutdowns, 2);
                assert_eq!(trace.borrow().events.last(), Some(&"restore"));
                assert!(!trace.borrow().events.contains(&"drop"));
                runtime.request("continuation", b)?;
                Ok(())
            },
        )
        .unwrap();
    });
    assert_eq!(request.storage(), 13);
    let events = &trace.borrow().events;
    assert_eq!(
        &events[events.len() - 3..],
        &["restore", "continuation", "drop"]
    );
    assert!(
        events.iter().position(|event| *event == "cancel").unwrap()
            < events
                .iter()
                .position(|event| *event == "foreground")
                .unwrap()
    );
    assert!(!events.contains(&"complete"));
}

#[test]
fn refused_continuation_does_not_publish_or_replace_original_account() {
    let fake = completed_fake();
    let trace = fake.trace.clone();
    let mut request = Account::new(Work::new(usize::MAX), FRAME + RETAINED);
    let result: Result<()> = request.with_budget(|b| {
        let ledger = b.work_ledger_identity_v1();
        run_scoped_with(
            b,
            3,
            3,
            || Ok(fake),
            |runtime, _, b| {
                assert!(ledger == b.work_ledger_identity_v1());
                assert_eq!(b.storage(), FRAME + RETAINED);
                runtime.request("continuation", b)?;
                Err(root::invalid("continuation", "injected refusal"))
            },
        )
    });
    assert!(matches!(
        result,
        Err(Failure::Invalid {
            role: "continuation",
            ..
        })
    ));
    assert_eq!(request.storage(), 0);
    assert!(!trace.borrow().events.contains(&"complete"));
    assert_eq!(trace.borrow().events.last(), Some(&"drop"));
}

#[test]
fn cleanup_failure_or_unwind_never_enters_application_continuation() {
    for stage in ["foreground", "shutdown", "restore"] {
        for unwind in [false, true] {
            let mut fake = completed_fake();
            if unwind {
                fake.panic = Some(stage);
            } else {
                fake.fail = Some(stage);
            }
            let trace = fake.trace.clone();
            let mut request = Account::new(Work::new(usize::MAX), FRAME + RETAINED);
            let entered = std::cell::Cell::new(false);
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                request.with_budget(|b| {
                    run_scoped_with(
                        b,
                        3,
                        3,
                        || Ok(fake),
                        |_, _, _| -> Result<()> {
                            entered.set(true);
                            Ok(())
                        },
                    )
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(!entered.get());
            assert!(!trace.borrow().events.contains(&"complete"));
            assert_eq!(trace.borrow().events.last(), Some(&"drop"));
            assert_eq!(request.storage(), 0);
        }
    }
}
