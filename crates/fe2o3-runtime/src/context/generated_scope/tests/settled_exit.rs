//! CPU composition controls; synthetic hooks never qualify native fault isolation.
#![cfg(test)]
use super::*;
use std::sync::Arc;

mod copies;
mod faults;
mod graph;

fn borrowed<'a>(decoded: &'a Cell<usize>, dropped: &'a Cell<usize>) -> Borrowed<'a> {
    Borrowed {
        ticks: Cell::new(2),
        decoded,
        dropped,
        domain: Arc::new(()),
        completion_order: None,
    }
}

#[test]
fn cold_local_failure_preserves_callback_manifest_only_after_other_child_and_epoch_close() {
    let mut context =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let uid = context.devices()[0].backend_device;
    let streams = devices.map(|d| context.create_stream(d).unwrap());
    let decoded = [Cell::new(0), Cell::new(0)];
    let dropped = [Cell::new(0), Cell::new(0)];
    let prepared = devices
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            context.bound_multi_preparation_for_test_v1(d, borrowed(&decoded[i], &dropped[i]))
        })
        .collect::<Vec<_>>();
    let original_manifest = Rc::new(Cell::new(false));
    let returned = context
        .with_generated_gfx942_scope_settled_v1(
            2,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = cold_device::cold_hooks();
                for (p, stream) in prepared.into_iter().zip(streams) {
                    scope.admit(p, stream).unwrap();
                }
                scope.progress_v1().unwrap();
                assert_eq!(
                    scope.cold_device_failures_v1().collect::<Vec<_>>(),
                    [(0, uid)]
                );
                assert_eq!(scope.pending_v1(), 1);
                assert!(scope.context.scope_epoch.active());
                (Rc::clone(&original_manifest), [(0, uid)])
            },
        )
        .unwrap();
    assert_eq!(
        returned.completion_v1(),
        Err(&RuntimeGfx942SettledFailureV1::DeviceUnavailableBeforeActivation { device_uid: uid })
    );
    assert!(!context.scope_epoch.active());
    assert!(!context.has_unpublished_holds_v1());
    assert_eq!(decoded.each_ref().map(Cell::get), [0, 1]);
    assert_eq!(dropped.each_ref().map(Cell::get), [1, 1]);
    let ((value, manifest), completion) = returned.into_parts_v1();
    assert!(Rc::ptr_eq(&value, &original_manifest));
    assert_eq!(manifest, [(0, uid)]);
    assert!(completion.is_err());
    value.set(true);
    assert!(context.cleanup().is_complete());
}

#[test]
fn unpublished_rejection_and_decoder_failure_are_distinct_settled_outcomes() {
    for reject in [true, false] {
        let mut context = context();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
        let returned = context
            .with_generated_gfx942_scope_settled_v1(
                1,
                Instant::now() + Duration::from_secs(30),
                |scope| {
                    scope.hooks = hooks();
                    if reject {
                        scope.hooks.rejected = |context, hold| {
                            context.validate_unpublished_hold_v1(hold)?;
                            Ok(true)
                        };
                        scope.hooks.retire_rejected = |context, prepared, _, hold| {
                            context.validate_unpublished_hold_v1(hold)?;
                            assert_eq!(prepared.value().dropped.get(), 0);
                            Ok(())
                        };
                    } else {
                        scope.hooks.decode = |prepared| {
                            drop(prepared);
                            Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
                        };
                    }
                    scope.admit(prepared, stream).unwrap();
                    // Even an application-level error remains the exact R, rather
                    // than being confused with the scope's structural error channel.
                    Err::<(), _>(17)
                },
            )
            .unwrap();
        let (value, completion) = returned.into_parts_v1();
        assert_eq!(value, Err(17));
        assert_eq!(
            completion,
            Err(if reject {
                RuntimeGfx942SettledFailureV1::RejectedBeforePublication
            } else {
                RuntimeGfx942SettledFailureV1::Readback(
                    RuntimeGfx942ReadbackErrorV1::InvalidStorage,
                )
            })
        );
        assert_eq!((decoded.get(), dropped.get()), (0, 1));
        assert!(!context.scope_epoch.active());
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn strict_scope_still_discards_value_on_the_same_settled_local_failure() {
    struct Value<'a>(&'a Cell<usize>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let mut context = context();
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let values = Cell::new(0);
    let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
    let result = context.with_generated_gfx942_scope_v1(
        1,
        Instant::now() + Duration::from_secs(30),
        |scope| {
            scope.hooks = hooks();
            scope.hooks.decode = |prepared| {
                drop(prepared);
                Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
            };
            scope.admit(prepared, stream).unwrap();
            Value(&values)
        },
    );
    assert!(matches!(
        result,
        Err(RuntimeGfx942ScopeErrorV1::Readback(_))
    ));
    assert_eq!((dropped.get(), values.get()), (1, 1));
    assert!(context.cleanup().is_complete());
}

#[test]
fn actual_async_api_inside_tokio_preserves_value_after_original_driver_closes() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            for early in [false, true] {
                let mut context = context();
                let stream = context.create_stream(context.devices()[0].id()).unwrap();
                let decoded = Cell::new(0);
                let dropped = Cell::new(0);
                let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
                let returned = context
                    .with_generated_gfx942_scope_settled_async_v1(
                        1,
                        Instant::now() + Duration::from_secs(30),
                        |_| std::future::ready(()),
                        async |scope| {
                            scope.hooks = hooks();
                            scope.hooks.decode = |prepared| {
                                drop(prepared);
                                Err(RuntimeGfx942ReadbackErrorV1::AlreadyReserved)
                            };
                            let ticket = scope.admit(prepared, stream).unwrap();
                            if early {
                                return 47;
                            }
                            let observer = scope.completion_future_v1(&ticket).unwrap();
                            let (drive, observation) = futures_util::future::join(
                                scope.drive_with_wake_v1(|_| std::future::ready(())),
                                observer,
                            )
                            .await;
                            assert!(matches!(drive, Err(RuntimeGfx942ScopeErrorV1::Readback(_))));
                            assert!(matches!(
                                observation,
                                Err(RuntimeGfx942ScopeErrorV1::Readback(_))
                            ));
                            47
                        },
                    )
                    .await
                    .unwrap();
                assert!(!context.scope_epoch.active());
                assert_eq!(
                    returned.into_parts_v1(),
                    (
                        47,
                        Err(RuntimeGfx942SettledFailureV1::Readback(
                            RuntimeGfx942ReadbackErrorV1::AlreadyReserved
                        ))
                    )
                );
                assert_eq!(dropped.get(), 1);
                assert!(context.cleanup().is_complete());
            }
        });
}

#[test]
fn cancellation_stays_per_ticket_without_minting_local_failure_or_decoder_success() {
    for adopted in [false, true] {
        let mut context = context();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_preparation_for_test_v1(borrowed(&decoded, &dropped));
        let returned = context
            .with_generated_gfx942_scope_settled_v1(
                1,
                Instant::now() + Duration::from_secs(30),
                |scope| {
                    scope.hooks = hooks();
                    let ticket = scope.admit(prepared, stream).unwrap();
                    if adopted {
                        scope.progress_v1().unwrap();
                    }
                    let cancellation = if adopted {
                        scope.cancel_before_publication_v1(&ticket)
                    } else {
                        scope.cancel_before_adoption_v1(&ticket)
                    };
                    assert_eq!(
                        cancellation.unwrap(),
                        if adopted {
                            RuntimeGfx942ScopedCancelResultV1::CancelledBeforePublication
                        } else {
                            RuntimeGfx942ScopedCancelResultV1::CancelledBeforeSubmission
                        }
                    );
                    assert!(matches!(
                        scope.completion_v1(&ticket),
                        Err(RuntimeGfx942ScopeErrorV1::CancelledBeforeSubmission
                            | RuntimeGfx942ScopeErrorV1::CancelledBeforePublication)
                    ));
                    adopted
                },
            )
            .unwrap();
        assert_eq!(returned.into_parts_v1(), (adopted, Ok(())));
        assert_eq!((decoded.get(), dropped.get()), (0, 1));
        assert!(context.cleanup().is_complete());
    }
}
