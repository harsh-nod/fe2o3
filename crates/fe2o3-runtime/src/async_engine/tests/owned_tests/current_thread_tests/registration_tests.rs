use super::*;

#[derive(Clone, Copy)]
enum Kind {
    Event,
    Stream,
    Pair,
}
const KINDS: [Kind; 3] = [Kind::Event, Kind::Stream, Kind::Pair];

enum Observer {
    Event(RuntimeEventFutureV1<MockError>),
    Stream(RuntimeAsyncProgressRegistrationV1<MockError>),
    Pair(RuntimeAsyncProgressEventFutureV1<MockError>),
}

#[derive(Debug, PartialEq)]
enum Rejection {
    Event(RuntimeAsyncEventRegistrationErrorV1),
    Stream(RuntimeAsyncProgressRegistrationErrorV1),
    Pair(RuntimeAsyncProgressEventRegistrationErrorV1),
}
type Admission = Pin<
    Box<
        dyn Future<Output = Result<Result<Observer, Rejection>, RuntimeAsyncEngineCallErrorV1>>
            + Send,
    >,
>;

fn boxed<T: Unpin + Send + 'static, E: Send + 'static>(
    future: RuntimeAsyncRegistrationFutureV1<T, E>,
    observer: fn(T) -> Observer,
    rejection: fn(E) -> Rejection,
) -> Admission {
    Box::pin(async move {
        future
            .await
            .map(|value| value.map(observer).map_err(rejection))
    })
}

fn enroll(
    kind: Kind,
    handle: &Handle,
    stream: RuntimeStreamIdV1,
    event: RuntimeEventIdV1,
) -> Result<Admission, RuntimeAsyncEngineCallErrorV1> {
    Ok(match kind {
        Kind::Event => boxed(
            handle.observer().enqueue_event_registration(event)?,
            Observer::Event,
            Rejection::Event,
        ),
        Kind::Stream => boxed(
            handle.enqueue_stream_registration(stream)?,
            Observer::Stream,
            Rejection::Stream,
        ),
        Kind::Pair => boxed(
            handle.enqueue_event_registration_with_progress(stream, event)?,
            Observer::Pair,
            Rejection::Pair,
        ),
    })
}

struct Fixture {
    engine: Engine,
    handle: Handle,
    state: Arc<Mutex<MockState>>,
    trace: Arc<Mutex<OwnerTrace>>,
    records: Vec<(RuntimeStreamIdV1, RuntimeEventIdV1)>,
}

impl Fixture {
    fn new(waiters: usize, commands: usize, replies: usize) -> Self {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let config =
            RuntimeAsyncEngineConfigV1::new(commands, waiters, 1, 1, Duration::from_millis(1))
                .unwrap()
                .with_reply_capacity(replies)
                .unwrap();
        let (mut engine, handle) = start_current_state(state.clone(), trace.clone(), config);
        let records = drive(
            &mut engine,
            handle
                .observer()
                .enqueue_with_context(|context| {
                    let device = context.devices()[0].id();
                    let module = context.load_module(device, &[1]).unwrap();
                    let kernel = context
                        .resolve_kernel::<EmptyArgs>(module, "empty")
                        .unwrap();
                    (0..5)
                        .map(|_| {
                            let stream = context.create_stream(device).unwrap();
                            let submission = context
                                .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                                .unwrap();
                            (stream, context.record_event(&submission).unwrap())
                        })
                        .collect()
                })
                .unwrap(),
        )
        .unwrap();
        Self {
            engine,
            handle,
            state,
            trace,
            records,
        }
    }

    fn pending(&self, kind: Kind, index: usize) -> Admission {
        let (stream, event) = self.records[index];
        enroll(kind, &self.handle, stream, event).unwrap()
    }

    fn admitted(&mut self, kind: Kind, index: usize) -> Observer {
        let future = self.pending(kind, index);
        drive(&mut self.engine, future).unwrap().unwrap()
    }

    fn succeed(&self) {
        for status in self.state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
    }

    fn finish(self) {
        self.succeed();
        assert_eq!(
            self.engine.shutdown().disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
    }
}

#[test]
fn observer_registration_all_three_owner_apis_progress_without_blocking() {
    for kind in KINDS {
        let mut f = Fixture::new(4, 4, 8);
        let (stream, event) = f.records[0];
        assert!(matches!(
            f.handle.observer().event_future(event),
            Err(RuntimeAsyncEventRegistrationErrorV1::ReentrantCall)
        ));
        assert!(matches!(
            f.handle.register_stream(stream),
            Err(RuntimeAsyncProgressRegistrationErrorV1::ReentrantCall)
        ));
        assert!(matches!(
            f.handle.event_future_with_progress(stream, event),
            Err(RuntimeAsyncProgressEventRegistrationErrorV1::ReentrantCall)
        ));
        let mut observer = f.admitted(kind, 0);
        f.succeed();
        for _ in 0..4 {
            f.engine.tick().unwrap();
        }
        match &mut observer {
            Observer::Event(future) => assert!(matches!(
                poll_once(future),
                Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
            )),
            Observer::Stream(guard) => {
                assert_eq!(guard.stream(), stream);
                assert!(!guard.is_stopped());
                assert!(!f.state.lock().unwrap().flush_calls.is_empty());
            }
            Observer::Pair(future) => {
                assert!(matches!(
                    poll_once(future),
                    Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
                ));
                assert!(future.is_progress_stopped());
            }
        }
        drop(observer);
        assert_eq!(f.handle.observer().reply_cells_in_use(), 0);
        f.finish();
    }
}

#[test]
fn observer_registration_reply_and_queue_exhaustion_restore_exact_credits() {
    for kind in KINDS {
        for replies in [1, 8] {
            let f = Fixture::new(4, 1, replies);
            let first = f.pending(kind, 0);
            let (stream, event) = f.records[1];
            let expected = if replies == 1 {
                RuntimeAsyncEngineCallErrorV1::ReplyCapacity
            } else {
                RuntimeAsyncEngineCallErrorV1::CommandQueueFull
            };
            assert!(
                matches!(enroll(kind, &f.handle, stream, event), Err(error) if error == expected)
            );
            assert_eq!(f.handle.observer().reply_cells_in_use(), 1);
            drop(first);
            // The accepted command still owns its reply until Stop drops it.
            assert_eq!(f.handle.observer().reply_cells_in_use(), 1);
            let handle = f.handle.clone();
            f.finish();
            assert_eq!(handle.observer().reply_cells_in_use(), 0);
        }
    }
}

#[test]
fn observer_registration_pre_ack_drop_never_polls_or_flushes_and_allows_reuse() {
    for kind in KINDS {
        let mut f = Fixture::new(1, 4, 8);
        drop(f.pending(kind, 0));
        f.engine.tick().unwrap();
        assert_eq!(count(&f.trace, "poll_v1"), 0);
        assert_eq!(count(&f.trace, "flush"), 0);
        assert_eq!(f.handle.observer().reply_cells_in_use(), 0);
        let observer = f.admitted(kind, 0);
        drop(observer);
        f.finish();
    }
}

struct DropDuringAck(Mutex<Option<Admission>>);
impl Wake for DropDuringAck {
    fn wake(self: Arc<Self>) {
        drop(self.0.lock().unwrap().take());
    }
}

#[test]
fn observer_registration_drop_in_ack_wake_cannot_leave_live_orphan() {
    for kind in KINDS {
        let mut f = Fixture::new(1, 4, 8);
        let owner = Arc::new(DropDuringAck(Mutex::new(Some(f.pending(kind, 0)))));
        let waker = Waker::from(owner.clone());
        assert!(
            owner
                .0
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        f.engine.tick().unwrap();
        assert!(owner.0.lock().unwrap().is_none());
        assert_eq!(count(&f.trace, "poll_v1"), 0);
        assert_eq!(count(&f.trace, "flush"), 0);
        assert_eq!(f.handle.observer().reply_cells_in_use(), 0);
        drop(f.admitted(kind, 0));
        f.finish();
    }
}

#[test]
fn observer_registration_stop_distinguishes_queued_from_admitted_original_observers() {
    for kind in KINDS {
        for admitted in [false, true] {
            let mut f = Fixture::new(4, 4, 8);
            let mut pending = f.pending(kind, 0);
            if admitted {
                f.engine.tick().unwrap();
            }
            let handle = f.handle.clone();
            f.finish();
            let result = pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()));
            if admitted {
                let Poll::Ready(Ok(Ok(mut observer))) = result else {
                    panic!("original acknowledgment missing");
                };
                match &mut observer {
                    Observer::Event(future) => assert!(matches!(
                        poll_once(future),
                        Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
                    )),
                    Observer::Stream(guard) => assert!(guard.is_stopped()),
                    Observer::Pair(future) => {
                        assert!(future.is_progress_stopped());
                        assert!(matches!(
                            poll_once(future),
                            Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
                        ));
                    }
                }
            } else {
                assert!(matches!(
                    result,
                    Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
                ));
            }
            drop(pending);
            assert_eq!(handle.observer().reply_cells_in_use(), 0);
        }
    }
}

#[test]
fn observer_registration_capacity_failures_never_commit_half_a_pair() {
    // Event capacity rejects without reserving the paired stream.
    let mut f = Fixture::new(1, 8, 16);
    let event = f.admitted(Kind::Event, 0);
    let pending = f.pending(Kind::Pair, 1);
    assert!(matches!(
        drive(&mut f.engine, pending),
        Ok(Err(Rejection::Pair(
            RuntimeAsyncProgressEventRegistrationErrorV1::EventCapacity
        )))
    ));
    drop(f.admitted(Kind::Stream, 1));
    drop(event);
    f.finish();
    // All four progress slots are occupied; a rejected pair must not reserve its event.
    let mut f = Fixture::new(4, 8, 16);
    let streams: Vec<_> = (0..4).map(|i| f.admitted(Kind::Stream, i)).collect();
    let pending = f.pending(Kind::Pair, 4);
    assert!(matches!(
        drive(&mut f.engine, pending),
        Ok(Err(Rejection::Pair(
            RuntimeAsyncProgressEventRegistrationErrorV1::ProgressCapacity
        )))
    ));
    drop(f.admitted(Kind::Event, 4));
    drop(streams);
    f.finish();
}

#[test]
fn observer_registration_validation_and_duplicate_precedence_are_unchanged() {
    let mut f = Fixture::new(4, 8, 16);
    let (stream, event) = f.records[0];
    let (other, _) = f.records[1];
    let mismatch = enroll(Kind::Pair, &f.handle, other, event).unwrap();
    assert!(matches!(
        drive(&mut f.engine, mismatch),
        Ok(Err(Rejection::Pair(
            RuntimeAsyncProgressEventRegistrationErrorV1::EventStreamMismatch
        )))
    ));
    let original = f.admitted(Kind::Event, 0);
    let duplicate = enroll(Kind::Pair, &f.handle, other, event).unwrap();
    assert!(matches!(
        drive(&mut f.engine, duplicate),
        Ok(Err(Rejection::Pair(
            RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateEvent
        )))
    ));
    let original_stream = f.admitted(Kind::Stream, 0);
    drop(original);
    let duplicate = enroll(Kind::Pair, &f.handle, stream, event).unwrap();
    assert!(matches!(
        drive(&mut f.engine, duplicate),
        Ok(Err(Rejection::Pair(
            RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateStream
        )))
    ));
    drop(original_stream);
    drive(
        &mut f.engine,
        f.handle
            .observer()
            .enqueue_with_context(move |context| context.release_event(event).unwrap())
            .unwrap(),
    )
    .unwrap();
    let invalid = f.pending(Kind::Event, 0);
    assert!(matches!(
        drive(&mut f.engine, invalid),
        Ok(Err(Rejection::Event(
            RuntimeAsyncEventRegistrationErrorV1::InvalidEvent(_)
        )))
    ));
    f.finish();
}

fn reject_nested(handle: &Handle, stream: RuntimeStreamIdV1, event: RuntimeEventIdV1) {
    let before = handle.observer().reply_cells_in_use();
    for kind in KINDS {
        assert!(matches!(
            enroll(kind, handle, stream, event),
            Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
        ));
    }
    assert_eq!(handle.observer().reply_cells_in_use(), before);
}
struct NestedWake {
    handle: Handle,
    stream: RuntimeStreamIdV1,
    event: RuntimeEventIdV1,
    wakes: AtomicUsize,
}
impl Wake for NestedWake {
    fn wake(self: Arc<Self>) {
        reject_nested(&self.handle, self.stream, self.event);
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn observer_registration_callbacks_and_ack_wakes_cannot_reenter() {
    let mut f = Fixture::new(4, 8, 16);
    let (stream, event) = f.records[0];
    let nested = f.handle.clone();
    drive(
        &mut f.engine,
        f.handle
            .observer()
            .enqueue_with_context(move |_| reject_nested(&nested, stream, event))
            .unwrap(),
    )
    .unwrap();
    let wake = Arc::new(NestedWake {
        handle: f.handle.clone(),
        stream,
        event,
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(wake.clone());
    let mut pending = f.pending(Kind::Pair, 0);
    assert!(
        pending
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    f.engine.tick().unwrap();
    assert_eq!(wake.wakes.load(Ordering::SeqCst), 1);
    drop(pending);
    f.finish();
}

#[test]
fn observer_registration_deadline_retains_original_ack_and_credit() {
    let mut f = Fixture::new(4, 8, 16);
    let (_, event) = f.records[0];
    let mut pending = Box::pin(
        f.handle
            .observer()
            .enqueue_event_registration(event)
            .unwrap(),
    );
    assert!(matches!(
        f.engine.drive_until_ready(pending.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    ));
    assert_eq!(f.handle.observer().reply_cells_in_use(), 1);
    let observer = f
        .engine
        .drive_until_ready(pending.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(observer.event(), event);
    assert_eq!(f.handle.observer().reply_cells_in_use(), 1);
    drop(pending);
    assert_eq!(f.handle.observer().reply_cells_in_use(), 0);
    drop(observer);
    f.finish();
}

#[test]
fn observer_registration_terminal_pair_does_not_flush_or_hold_registry_capacity() {
    let mut f = Fixture::new(1, 4, 8);
    let (_, event) = f.records[0];
    f.succeed();
    drive(
        &mut f.engine,
        f.handle
            .observer()
            .enqueue_with_context(move |context| context.poll_event(event).unwrap())
            .unwrap(),
    )
    .unwrap();
    let before = count(&f.trace, "poll_v1");
    let Observer::Pair(mut observer) = f.admitted(Kind::Pair, 0) else {
        unreachable!()
    };
    assert!(matches!(
        poll_once(&mut observer),
        Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
    ));
    assert!(observer.is_progress_stopped());
    assert_eq!(count(&f.trace, "poll_v1"), before);
    assert_eq!(count(&f.trace, "flush"), 0);
    drop(f.admitted(Kind::Pair, 0));
    f.finish();
}

#[test]
fn observer_registration_foreign_thread_enqueues_without_owning_context() {
    for kind in KINDS {
        let mut f = Fixture::new(4, 4, 8);
        let handle = f.handle.clone();
        let (stream, event) = f.records[0];
        let pending = thread::spawn(move || enroll(kind, &handle, stream, event).unwrap())
            .join()
            .unwrap();
        drop(drive(&mut f.engine, pending).unwrap().unwrap());
        f.finish();
    }
}

#[test]
fn observer_registration_background_owner_uses_the_same_async_commands() {
    for kind in KINDS {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (engine, handle) = start(state.clone(), trace);
        let (stream, event) = handle
            .observer()
            .try_with_context(|context| {
                let device = context.devices()[0].id();
                let stream = context.create_stream(device).unwrap();
                let module = context.load_module(device, &[1]).unwrap();
                let kernel = context
                    .resolve_kernel::<EmptyArgs>(module, "empty")
                    .unwrap();
                let submission = context
                    .launch(stream, &kernel, &EmptyArgs, geometry(), &[])
                    .unwrap();
                (stream, context.record_event(&submission).unwrap())
            })
            .unwrap();
        let pending = enroll(kind, &handle, stream, event).unwrap();
        drop(owned::join_observer_v1(pending).unwrap().unwrap());
        for status in state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
    }
}

#[test]
fn observer_registration_standalone_capacity_and_duplicate_errors_are_async() {
    let mut f = Fixture::new(1, 8, 16);
    let original = f.admitted(Kind::Event, 0);
    let duplicate = f.pending(Kind::Event, 0);
    assert!(matches!(
        drive(&mut f.engine, duplicate),
        Ok(Err(Rejection::Event(
            RuntimeAsyncEventRegistrationErrorV1::DuplicateEvent
        )))
    ));
    let capacity = f.pending(Kind::Event, 1);
    assert!(matches!(
        drive(&mut f.engine, capacity),
        Ok(Err(Rejection::Event(
            RuntimeAsyncEventRegistrationErrorV1::Capacity
        )))
    ));
    drop(original);
    drop(f.admitted(Kind::Event, 1));
    let streams: Vec<_> = (0..4).map(|i| f.admitted(Kind::Stream, i)).collect();
    let duplicate = f.pending(Kind::Stream, 0);
    assert!(matches!(
        drive(&mut f.engine, duplicate),
        Ok(Err(Rejection::Stream(
            RuntimeAsyncProgressRegistrationErrorV1::DuplicateStream
        )))
    ));
    let capacity = f.pending(Kind::Stream, 4);
    assert!(matches!(
        drive(&mut f.engine, capacity),
        Ok(Err(Rejection::Stream(
            RuntimeAsyncProgressRegistrationErrorV1::Capacity
        )))
    ));
    drop(streams);
    drop(f.admitted(Kind::Stream, 4));
    assert_eq!(f.handle.observer().reply_cells_in_use(), 0);
    f.finish();
}
