//! Synthetic scope custody controls; these hooks do not create native DATA.

use super::*;

fn retain_then_dispose(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    prepared: &mut RuntimeGfx942PreparedV1<Borrowed<'_>>,
    _: &GeneratedHostRosterV1,
    hold: &ContextUnpublishedHoldV1,
) -> Result<bool, NativeError> {
    context.validate_unpublished_hold_v1(hold)?;
    if prepared.value().ticks.replace(1) == 0 {
        return Ok(false);
    }
    context.release_unpublished_hold_v1(hold)?;
    Ok(true)
}

#[test]
fn retained_producer_keeps_carrier_observer_and_hold_until_terminal_disposal() {
    for abandon in [false, true] {
        let mut context = context();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[0]);
        scope.hooks.complete = retain_then_dispose;
        let capacity = (scope.slots.capacity(), scope.copies.capacity());
        let mut observer = scope.completion_future_v1(&tickets[0]).unwrap();
        for _ in 0..3 {
            scope.progress_v1().unwrap();
        }
        assert_eq!(scope.slots[0].lifecycle.phase, Phase::RetainedProducer);
        assert_eq!(
            (scope.pending_v1(), decoded.get(), dropped.get()),
            (1, 0, 0)
        );
        assert!(scope.slots[0].lifecycle.value.is_some());
        assert!(scope.slots[0].lifecycle.outcome.is_none());
        assert!(scope.context.has_unpublished_holds_v1());
        assert!(poll(&mut observer, Waker::noop()).is_pending());
        assert!(scope.completion_v1(&tickets[0]).unwrap().is_none());
        assert_eq!(
            scope.cancel_before_adoption_v1(&tickets[0]).unwrap(),
            RuntimeGfx942ScopedCancelResultV1::NotCancellable
        );
        assert_eq!(
            scope.cancel_before_publication_v1(&tickets[0]).unwrap(),
            RuntimeGfx942ScopedCancelResultV1::NotCancellable
        );
        if abandon {
            drop(observer);
        } else {
            scope.progress_v1().unwrap();
            assert!(matches!(
                poll(&mut observer, Waker::noop()),
                Poll::Ready(Ok(()))
            ));
            drop(observer);
        }
        scope.drain_v1().unwrap();
        assert_eq!((decoded.get(), dropped.get()), (1, 1));
        assert_eq!(capacity, (scope.slots.capacity(), scope.copies.capacity()));
        assert!(!scope.context.has_unpublished_holds_v1());
        assert_eq!(scope.progress_v1().unwrap(), 0);
        drop((tickets, scope));
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn retained_producer_drop_forget_and_phase_failures_keep_original_custody() {
    const CHILD: &str = "FE2O3_RETAINED_PRODUCER_SCOPE_FAILURE";
    const TEST: &str = "context::generated_scope::tests::futures::retained_producer::retained_producer_drop_forget_and_phase_failures_keep_original_custody";
    const MARKER: &str = "RETAINED_PRODUCER_ORIGINALS_AND_HOLD_ROOTED";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut context = context();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[0]);
        scope.hooks.complete = retain_then_dispose;
        for _ in 0..2 {
            scope.progress_v1().unwrap();
        }
        if mode == "detach-refusal" || mode == "detach-unwind" {
            scope.hooks.complete = if mode == "detach-refusal" {
                |_, _, _, _| Err(RuntimeValidationErrorV1::ContextTerminal.into())
            } else {
                |_, _, _, _| panic!("injected original detach unwind")
            };
        } else {
            scope.progress_v1().unwrap();
            assert_eq!(scope.slots[0].lifecycle.phase, Phase::RetainedProducer);
            if mode == "dispose-refusal" || mode == "dispose-unwind" {
                scope.hooks.complete = if mode == "dispose-refusal" {
                    |_, _, _, _| Err(RuntimeValidationErrorV1::ContextTerminal.into())
                } else {
                    |_, _, _, _| panic!("injected original disposal unwind")
                };
            }
        }
        if mode != "drop" && mode != "forget" {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scope.progress_v1()));
            if mode == "detach-unwind" || mode == "dispose-unwind" {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
            assert!(matches!(
                scope.progress_v1(),
                Err(RuntimeGfx942ScopeErrorV1::Unknown)
            ));
        }
        assert_eq!(
            (decoded.get(), dropped.get(), scope.pending_v1()),
            (0, 0, 1)
        );
        assert!(scope.slots[0].lifecycle.value.is_some());
        assert!(scope.slots[0].lifecycle.outcome.is_none());
        assert!(scope.context.has_unpublished_holds_v1());
        drop(tickets);
        eprintln!("{MARKER}");
        if mode == "forget" {
            std::mem::forget(scope);
            assert!(!context.cleanup().is_complete());
            assert_eq!((decoded.get(), dropped.get()), (0, 0));
            drop(context);
        } else {
            drop(scope);
        }
        panic!("retained producer returned after releasing owners");
    }
    for mode in [
        "drop",
        "forget",
        "detach-refusal",
        "detach-unwind",
        "dispose-refusal",
        "dispose-unwind",
    ] {
        super::super::unpublished::abort_child(TEST, CHILD, mode, MARKER);
    }
}
