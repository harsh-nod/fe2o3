//! CPU driver composition only. Synthetic hooks cannot construct a lower reset.
#![cfg(test)]
use super::*;
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    Pin::new(future).poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

pub(super) fn cold_hooks<'a>() -> Hooks<KfdMultiDeviceRuntimeBackendV1, Borrowed<'a>> {
    let mut hooks: Hooks<KfdMultiDeviceRuntimeBackendV1, Borrowed<'a>> = hooks();
    hooks.cold = Some((
        |context, prepared, _, hold| {
            context.validate_unpublished_hold_v1(hold)?;
            if context.streams[&hold.stream()].device == context.devices()[0].id() {
                context
                    .synthetic_cold_failure_for_test_v1(prepared, hold)
                    .map(Some)
            } else {
                Ok(None)
            }
        },
        |context, prepared, _, hold, failure| {
            assert_eq!(
                (
                    prepared.value().decoded.get(),
                    prepared.value().dropped.get()
                ),
                (0, 0)
            );
            context.validate_synthetic_cold_failure_for_test_v1(prepared, hold, failure)
        },
    ));
    hooks
}

#[test]
fn settled_cold_failure_disposes_original_without_decode_and_preserves_other_child() {
    let mut context =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let uid = context.devices()[0].backend_device;
    let streams = devices.map(|device| context.create_stream(device).unwrap());
    let decoded = [Cell::new(0), Cell::new(0)];
    let dropped = [Cell::new(0), Cell::new(0)];
    let prepared: Vec<_> = devices
        .iter()
        .enumerate()
        .map(|(i, &device)| {
            context.bound_multi_preparation_for_test_v1(
                device,
                Borrowed {
                    ticks: Cell::new(3),
                    decoded: &decoded[i],
                    dropped: &dropped[i],
                    domain: Arc::new(()),
                    completion_order: None,
                },
            )
        })
        .collect();
    let result = context.with_generated_gfx942_scope_v1(2, Instant::now() + Duration::from_secs(30), |scope| {
        scope.hooks = cold_hooks();
        let tickets: Vec<_> = prepared.into_iter().zip(streams).map(|(p,s)| scope.admit(p,s).unwrap()).collect();
        let mut failed = scope.completion_future_v1(&tickets[0]).unwrap();
        let mut healthy = scope.completion_future_v1(&tickets[1]).unwrap();
        assert_eq!(scope.progress_v1().unwrap(), 2);
        assert_eq!(scope.slots[0].lifecycle.phase, Phase::ColdDeviceFailed { device_uid: uid });
        assert!(scope.slots[0].lifecycle.value.is_none());
        assert!(scope.slots[0].lifecycle.outcome.is_none());
        assert_eq!((decoded[0].get(), dropped[0].get(), dropped[1].get()), (0,1,0));
        assert_eq!(scope.cold_device_failures_v1().collect::<Vec<_>>(), [(0,uid)]);
        assert!(matches!(poll(&mut failed), Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::DeviceUnavailableBeforeActivation { device_uid })) if device_uid == uid));
        assert!(poll(&mut healthy).is_pending());
        assert_eq!(scope.pending_v1(), 1);
        for _ in 0..16 {
            if scope.pending_v1() == 0 { break; }
            scope.progress_v1().unwrap();
        }
        assert_eq!(scope.pending_v1(), 0);
        assert!(matches!(poll(&mut healthy), Poll::Ready(Ok(()))));
        assert_eq!((decoded[0].get(), decoded[1].get(), dropped[1].get()), (0,1,1));
        assert!(!scope.context.has_unpublished_holds_v1());
        assert!(!scope.context.is_terminal());
    });
    assert!(
        matches!(result, Err(RuntimeGfx942ScopeErrorV1::DeviceUnavailableBeforeActivation { device_uid }) if device_uid == uid)
    );
    assert!(!context.is_terminal());
    assert!(context.cleanup().is_complete());
}

#[test]
fn cold_source_or_process_ambiguity_retains_all_children_and_fail_stops() {
    const ENV: &str = "FE2O3_COLD_SCOPE_AMBIGUITY";
    const TEST: &str = "context::generated_scope::tests::cold_device::cold_source_or_process_ambiguity_retains_all_children_and_fail_stops";
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
        let mut context =
            RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let streams = devices.map(|device| context.create_stream(device).unwrap());
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared: Vec<_> = devices
            .into_iter()
            .map(|device| {
                context.bound_multi_preparation_for_test_v1(
                    device,
                    Borrowed {
                        ticks: Cell::new(3),
                        decoded: &decoded,
                        dropped: &dropped,
                        domain: Arc::new(()),
                        completion_order: None,
                    },
                )
            })
            .collect();
        let _ =
            context.with_generated_gfx942_scope_v1(
                2,
                Instant::now() + Duration::from_secs(30),
                |scope| {
                    scope.hooks = cold_hooks();
                    let settle: ColdSettle<_, _> = if mode == "panic" {
                        |context, _, _, _, _| {
                            context.quarantine_after_async_command_panic_v1();
                            panic!("cold source check ambiguity")
                        }
                    } else {
                        |context, _, _, _, _| {
                            context.quarantine_after_async_command_panic_v1();
                            Err(RuntimeValidationErrorV1::InvalidBackendDescription.into())
                        }
                    };
                    scope.hooks.cold.as_mut().unwrap().1 = settle;
                    let tickets: Vec<_> = prepared
                        .into_iter()
                        .zip(streams)
                        .map(|(p, s)| scope.admit(p, s).unwrap())
                        .collect();
                    let mut observers: Vec<_> = tickets
                        .iter()
                        .map(|t| scope.completion_future_v1(t).unwrap())
                        .collect();
                    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        scope.progress_v1()
                    }));
                    if mode == "panic" {
                        assert!(failure.is_err());
                    } else {
                        assert!(failure.unwrap().is_err());
                    }
                    assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
                    assert!(
                        scope.slots.iter().all(|slot| slot.lifecycle.value.is_some()
                            && slot.lifecycle.outcome.is_none())
                    );
                    assert_eq!(
                        (decoded.get(), dropped.get(), scope.pending_v1()),
                        (0, 0, 2)
                    );
                    assert_eq!(scope.cold_device_failures_v1().count(), 0);
                    assert!(scope.context.is_terminal());
                    assert!(scope.context.has_unpublished_holds_v1());
                    assert!(matches!(
                        scope.progress_v1(),
                        Err(RuntimeGfx942ScopeErrorV1::Unknown)
                    ));
                    for observer in &mut observers {
                        assert!(matches!(
                            poll(observer),
                            Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown))
                        ));
                    }
                    eprintln!("COLD_SCOPE_ORIGINALS_RETAINED");
                },
            );
        panic!("returned after releasing original cold-scope owners");
    }
    for mode in ["error", "panic"] {
        unpublished::abort_child(TEST, ENV, mode, "COLD_SCOPE_ORIGINALS_RETAINED");
    }
}
