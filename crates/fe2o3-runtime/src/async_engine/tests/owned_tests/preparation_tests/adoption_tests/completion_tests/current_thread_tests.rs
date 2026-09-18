use super::super::super::super::current_thread_tests::{drive, start_current};
use super::*;

#[test]
fn current_thread_generated_receipt_survives_owner_join_rejection_and_deadline() {
    let (mut engine, handle) = start_current(
        Arc::new(Mutex::new(OwnerTrace::default())),
        RuntimeAsyncEngineConfigV1::default(),
    );
    let (stream, state) = drive(
        &mut engine,
        handle
            .observer()
            .enqueue_with_context(|context| {
                (
                    context.create_stream(context.devices()[0].id()).unwrap(),
                    context.backend().inner.state.clone(),
                )
            })
            .unwrap(),
    )
    .unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let domain = Arc::new(());
    let foreign = Arc::new(());
    let prepared = drive(
        &mut engine,
        preparation_with_domain(
            &handle,
            state.clone(),
            drops.clone(),
            0,
            Some(completion_hooks::<ThreadBoundBackend>()),
            domain.clone(),
        ),
    )
    .unwrap()
    .unwrap();
    let reserved = drive(
        &mut engine,
        handle.try_reserve_prepared_v1(prepared).unwrap(),
    )
    .unwrap()
    .unwrap();
    let completion = drive(
        &mut engine,
        handle
            .enqueue_reserved_activation_v1(reserved, stream, true)
            .unwrap(),
    )
    .unwrap()
    .unwrap();
    let probe = completion.result_probe_for_test_v1();
    assert_eq!(probe(), None);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    let order = state.lock().unwrap().adoption_order.clone();
    let owners = Arc::strong_count(&domain);
    let failure = completion
        .try_join()
        .expect_err("owner must not block on itself");
    assert_eq!(failure.error, RuntimeAsyncEngineCallErrorV1::ReentrantCall);
    assert!(failure.completion.matches_owner(&domain));
    assert!(!failure.completion.matches_owner(&foreign));
    assert_eq!(probe(), None);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(Arc::strong_count(&domain), owners);
    assert_eq!(state.lock().unwrap().adoption_order, order);
    assert_eq!(handle.observer().reply_cells_in_use(), 1);
    let mut completion = Box::pin(failure.completion);
    assert!(matches!(
        engine.drive_until_ready(completion.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    ));
    assert_eq!(probe(), None);
    assert_eq!(state.lock().unwrap().adoption_order, order);
    for _ in 0..8 {
        if probe().is_some() {
            break;
        }
        engine.tick().unwrap();
    }
    assert_eq!(probe(), Some(Ok(Ok(()))));
    let receipt = engine
        .drive_until_ready(completion.as_mut(), Instant::now())
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(receipt.matches_owner(&domain));
    assert!(!receipt.matches_owner(&foreign));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        state.lock().unwrap().adoption_order,
        [
            "preflight",
            "adopt",
            "issue",
            "poll",
            "copy",
            "postcheck",
            "native_retire",
            "retire",
            "records_retire",
            "hold_release",
            "hold_released",
            "decode",
            "gate_commit",
            "payload_drop"
        ]
    );
    drop(completion);
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
    assert!(
        drive(
            &mut engine,
            handle
                .observer()
                .enqueue_with_context(move |context| context.destroy_stream(stream))
                .unwrap()
        )
        .unwrap()
        .is_ok()
    );
    assert_eq!(
        engine.shutdown().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}
