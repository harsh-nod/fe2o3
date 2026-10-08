#![cfg(test)]
use super::*;

struct Returned<'a>(&'a Cell<usize>);
impl Drop for Returned<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn closing_deadline_terminal_and_original_hold_refuse_callback_value() {
    for mode in 0..3 {
        let mut context = context();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let values = Cell::new(0);
        let mut held = None;
        let result = context.with_generated_gfx942_scope_settled_v1::<Borrowed<'_>, _>(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                match mode {
                    0 => scope.deadline = Instant::now(),
                    1 => scope.context.quarantine_after_async_command_panic_v1(),
                    _ => {
                        let _permit = scope.epoch.enter().unwrap();
                        held = Some(scope.context.hold_unpublished_stream_v1(stream).unwrap());
                    }
                }
                Returned(&values)
            },
        );
        assert!(result.is_err());
        assert_eq!(values.get(), 1);
        assert!(!context.scope_epoch.active());
        if let Some(hold) = held {
            context.release_unpublished_hold_v1(&hold).unwrap();
        }
        assert_eq!(context.is_terminal(), mode == 1);
        if mode != 1 {
            assert!(context.cleanup().is_complete());
        }
    }
}

#[test]
fn callback_unwind_after_actual_settlement_closes_epoch_without_returning_value() {
    let mut context = context();
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = context.with_generated_gfx942_scope_settled_v1(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.admit(prepared, stream).unwrap();
                scope.drain_v1().unwrap();
                panic!("callback unwind after original closure");
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    assert!(!context.scope_epoch.active());
    assert!(context.cleanup().is_complete());
}

#[test]
fn unknown_progress_or_unwind_never_returns_or_drops_callback_value_before_fail_stop() {
    const ENV: &str = "FE2O3_SETTLED_EXIT_UNKNOWN";
    const TEST: &str = "context::generated_scope::tests::settled_exit::faults::unknown_progress_or_unwind_never_returns_or_drops_callback_value_before_fail_stop";
    if let Some(mode) = std::env::var_os(ENV) {
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
        struct CannotDrop;
        impl Drop for CannotDrop {
            fn drop(&mut self) {
                std::process::exit(92);
            }
        }
        let mut context = context();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
        let _ = context.with_generated_gfx942_scope_settled_v1(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.hooks.complete = |context, prepared, _, hold| {
                    context.validate_unpublished_hold_v1(hold)?;
                    assert_eq!(prepared.value().dropped.get(), 0);
                    context.quarantine_after_async_command_panic_v1();
                    Err(RuntimeValidationErrorV1::InvalidBackendDescription.into())
                };
                scope.admit(prepared, stream).unwrap();
                if mode == "callback-panic" {
                    eprintln!("SETTLED_EXIT_ORIGINALS_RETAINED");
                    panic!("callback still owns original preparation");
                }
                if mode == "hook-panic" {
                    scope.hooks.complete = |_, _, _, _| panic!("original release unwind");
                }
                let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    for _ in 0..16 {
                        scope.progress_v1()?;
                    }
                    Ok::<_, RuntimeGfx942ScopeErrorV1>(())
                }));
                if mode == "hook-panic" {
                    assert!(error.is_err());
                } else {
                    assert!(error.unwrap().is_err());
                }
                assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
                assert!(scope.slots[0].lifecycle.value.is_some());
                assert!(scope.context.has_unpublished_holds_v1());
                assert_eq!((decoded.get(), dropped.get()), (0, 0));
                eprintln!("SETTLED_EXIT_ORIGINALS_RETAINED");
                CannotDrop
            },
        );
        panic!("returned after releasing owners");
    }
    for mode in ["error", "hook-panic", "callback-panic"] {
        unpublished::abort_child(TEST, ENV, mode, "SETTLED_EXIT_ORIGINALS_RETAINED");
    }
}

#[test]
fn polled_async_drop_and_forget_preserve_original_epoch_until_fail_stop() {
    const ENV: &str = "FE2O3_SETTLED_EXIT_ASYNC_CUSTODY";
    const TEST: &str = "context::generated_scope::tests::settled_exit::faults::polled_async_drop_and_forget_preserve_original_epoch_until_fail_stop";
    if let Some(mode) = std::env::var_os(ENV) {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };
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
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
        struct CannotDrop;
        impl Drop for CannotDrop {
            fn drop(&mut self) {
                std::process::exit(92);
            }
        }
        let draining = mode == "draining";
        let mut future = Box::pin(context.with_generated_gfx942_scope_settled_async_v1(
            1,
            Instant::now() + Duration::from_secs(30),
            |_| std::future::ready(()),
            async |scope| {
                scope.hooks = hooks();
                scope.admit(prepared, stream).unwrap();
                if !draining {
                    std::future::pending::<()>().await;
                }
                CannotDrop
            },
        ));
        assert!(matches!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        eprintln!("SETTLED_ASYNC_ORIGINALS_RETAINED");
        if mode == "drop" || draining {
            drop(future);
        } else {
            std::mem::forget(future);
            assert!(context.scope_epoch.active());
            assert!(!context.cleanup().is_complete());
            drop(context);
        }
        panic!("returned after releasing owners");
    }
    for mode in ["drop", "forget", "draining"] {
        unpublished::abort_child(TEST, ENV, mode, "SETTLED_ASYNC_ORIGINALS_RETAINED");
    }
}
