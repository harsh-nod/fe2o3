use super::*;
use generated_operation::adoption::IssueHooksV1;

fn issue_hooks<B: RetireBackend>() -> AdoptionHooksV1<B, Payload> {
    let mut hooks: AdoptionHooksV1<B, Payload> = hooks();
    hooks.issue = Some(IssueHooksV1 {
        progress: |_, payload: &mut Payload, _, _| {
            payload.issue_advances += 1;
            payload
                .state
                .lock()
                .unwrap()
                .adoption_order
                .push(if payload.issue_advances == 1 {
                    "issue"
                } else {
                    "poll"
                });
            match payload.mode {
                10 => Err(RuntimeValidationErrorV1::Unsupported.into()),
                11 => panic!("issue hook panic after retained attempt"),
                12 => Ok(false),
                _ => Ok(payload.issue_advances == 2),
            }
        },
        retire_stopped: |context, hold| {
            if !context.backend().adoption_ready()? {
                return Ok(false);
            }
            context
                .backend_mut_for_test_v1()
                .retire_adoption(hold.stream())?;
            Ok(true)
        },
    });
    hooks
}

fn reserved_issue(
    h: &mut Harness,
    drops: Arc<AtomicUsize>,
    mode: u8,
) -> (RuntimeAsyncReservedTicketV1, RuntimeStreamIdV1) {
    let stream = h
        .context
        .create_stream(h.context.devices()[0].id())
        .unwrap();
    let future =
        preparation_with_hooks(&h.handle, h.state.clone(), drops, mode, Some(issue_hooks()));
    h.command();
    h.advance();
    let ticket = ready(future).unwrap().unwrap();
    let future = h.handle.try_reserve_prepared_v1(ticket).unwrap();
    h.command();
    (ready(future).unwrap().unwrap(), stream)
}

#[test]
fn issued_and_physically_settled_work_remains_owned_during_drain() {
    for mode in [0, 12] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved_issue(&mut h, drops.clone(), mode);
        activate(&mut h, ticket, stream);
        h.advance(); // Adopt.
        for _ in 0..6 {
            h.advance();
            h.registry.retire_unpublished_v1(&mut h.context, 1);
        }
        assert_eq!(h.registry.active_len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        assert!(h.context.destroy_stream(stream).is_err());
        let state = h.state.lock().unwrap();
        assert_eq!(state.adoption_retire_calls, 0);
        assert_eq!(
            state
                .adoption_order
                .iter()
                .filter(|v| **v == "issue")
                .count(),
            1
        );
        if mode == 0 {
            assert_eq!(
                state
                    .adoption_order
                    .iter()
                    .filter(|v| **v == "poll")
                    .count(),
                1
            );
        }
        drop(state);
        assert!(!h.registry.stop_observations());
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert_eq!(h.registry.active_len(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(h.state.lock().unwrap().adoption_retire_calls, 1);
        h.context.destroy_stream(stream).unwrap();
    }
}

#[test]
fn issued_stop_pending_retains_carrier_hold_and_never_reissues() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserved_issue(&mut h, drops.clone(), 12);
    activate(&mut h, ticket, stream);
    h.advance();
    h.advance();
    h.state.lock().unwrap().adoption_ready_mode = 1;
    assert!(!h.registry.stop_observations());
    for _ in 0..3 {
        h.registry.retire_unpublished_v1(&mut h.context, 1);
    }
    assert_eq!(h.registry.active_len(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(h.context.destroy_stream(stream).is_err());
    let state = h.state.lock().unwrap();
    assert_eq!(state.adoption_order, ["preflight", "adopt", "issue"]);
    assert_eq!(state.adoption_retire_calls, 0);
    drop(state);
    core::mem::forget(h.registry);
}

#[test]
fn issue_and_stopped_retirement_faults_never_retry_or_drop() {
    for (mode, retire_mode) in [(10, 0), (11, 0), (0, 1), (0, 2)] {
        let mut h = Harness::new(1, 2, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserved_issue(&mut h, drops.clone(), mode);
        activate(&mut h, ticket, stream);
        h.advance();
        h.advance();
        h.state.lock().unwrap().adoption_retire_mode = retire_mode;
        h.registry.stop_observations();
        h.registry.retire_unpublished_v1(&mut h.context, 1);
        assert!(h.context.is_terminal());
        let order = h.state.lock().unwrap().adoption_order.clone();
        for _ in 0..3 {
            h.advance();
            h.registry.retire_unpublished_v1(&mut h.context, 1);
        }
        assert_eq!(h.state.lock().unwrap().adoption_order, order);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.registry.active_len(), 1);
        core::mem::forget(h.registry);
    }
}

#[test]
fn transferable_engine_cannot_activate_generated_issue() {
    let mut h = Harness::new(1, 2, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserved_issue(&mut h, drops.clone(), 0);
    let key = ticket.key.clone();
    let mut nonowned = operation::OperationRegistryV1::new(1, false);
    let mut ticket = Some(ticket);
    let failure = nonowned
        .activate_reserved(&mut h.context, &mut ticket, stream)
        .unwrap_err();
    assert!(matches!(
        failure,
        ActivationErrorV1::Engine(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert!(Arc::ptr_eq(&key, &ticket.as_ref().unwrap().key));
    assert!(h.state.lock().unwrap().adoption_order.is_empty());
    h.context.destroy_stream(stream).unwrap();
    assert!(h.registry.discard_reserved(&key));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn owned_generated_drain_does_not_cancel_a_physically_settled_result() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let drops = Arc::new(AtomicUsize::new(0));
    let (engine, handle) = start(state.clone(), trace);
    let stream = handle
        .observer
        .try_with_context(|c| c.create_stream(c.devices()[0].id()).unwrap())
        .unwrap();
    let ticket = join(preparation_with_hooks(
        &handle,
        state.clone(),
        drops.clone(),
        0,
        Some(issue_hooks()),
    ))
    .unwrap()
    .unwrap();
    let ticket = join(handle.try_reserve_prepared_v1(ticket).unwrap())
        .unwrap()
        .unwrap();
    join(handle.try_activate_reserved_v1(ticket, stream).unwrap())
        .unwrap()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if state.lock().unwrap().adoption_order.contains(&"poll") {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let report = join(handle.begin_drain(8).unwrap()).unwrap();
    assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::BudgetExhausted);
    assert_eq!(report.operations_remaining, 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(state.lock().unwrap().adoption_retire_calls, 0);
    // Exhausted drain already seals the Context; Stop cannot reopen it.
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
}

#[test]
fn owned_stop_retires_settled_work_without_claiming_successful_output() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let drops = Arc::new(AtomicUsize::new(0));
    let (engine, handle) = start(state.clone(), trace);
    let stream = handle
        .observer
        .try_with_context(|c| c.create_stream(c.devices()[0].id()).unwrap())
        .unwrap();
    let ticket = join(preparation_with_hooks(
        &handle,
        state.clone(),
        drops.clone(),
        0,
        Some(issue_hooks()),
    ))
    .unwrap()
    .unwrap();
    let ticket = join(handle.try_reserve_prepared_v1(ticket).unwrap())
        .unwrap()
        .unwrap();
    join(handle.try_activate_reserved_v1(ticket, stream).unwrap())
        .unwrap()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if state.lock().unwrap().adoption_order.contains(&"poll") {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(state.lock().unwrap().adoption_retire_calls, 1);
}
