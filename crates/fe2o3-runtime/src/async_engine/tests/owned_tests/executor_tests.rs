use super::*;
use crate::{
    RuntimeAsyncLaunchRequestV1, RuntimeAsyncOperationPhaseV1, RuntimeAsyncTimeoutResultV1,
};
use std::future::poll_fn;
use tokio::sync::oneshot;

async fn pending_notification<F: Future>(future: F, sender: oneshot::Sender<()>) -> F::Output {
    let mut future = Box::pin(future);
    let mut sender = Some(sender);
    poll_fn(move |cx| {
        let result = future.as_mut().poll(cx);
        if result.is_pending()
            && let Some(sender) = sender.take()
        {
            sender.send(()).unwrap();
        }
        result
    })
    .await
}

fn tokio_wakeup(multithread: bool, abandon: bool) {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let (stream, kernel) = launch_fixture(&handle);
    let request =
        RuntimeAsyncLaunchRequestV1::new(stream, kernel, &EmptyArgs, geometry(), Vec::new())
            .unwrap();
    let operation = handle.enqueue_launch_tracked(request).unwrap();
    let control = operation.control();
    wait_until(|| state.lock().unwrap().issues.len() == 1);
    let raw = state.lock().unwrap().issues[0].1;
    let mut builder = if multithread {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.worker_threads(2);
        builder
    } else {
        tokio::runtime::Builder::new_current_thread()
    };
    let runtime = builder.enable_time().build().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            let (sent, received) = oneshot::channel();
            // Spawning also checks Send future semantics with the !Send backend.
            let task = tokio::spawn(pending_notification(operation, sent));
            received.await.unwrap();
            if abandon {
                task.abort();
                assert!(matches!(task.await, Err(error) if error.is_cancelled()));
                state
                    .lock()
                    .unwrap()
                    .statuses
                    .insert(raw, BackendPollV1::Succeeded);
                while control.phase() != RuntimeAsyncOperationPhaseV1::ObservationFinished {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            } else {
                state
                    .lock()
                    .unwrap()
                    .statuses
                    .insert(raw, BackendPollV1::Succeeded);
                let result = task.await.unwrap().unwrap();
                assert_eq!(
                    result.observation.unwrap(),
                    RuntimeCompletionStatusV1::Succeeded
                );
            }
        })
        .await
        .expect("executor must receive the owner wakeup");
    });
    assert_eq!(state.lock().unwrap().issues.len(), 1);
    assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert!(shutdown.cleanup.unwrap().is_complete());
    let trace = trace.lock().unwrap();
    assert!(
        trace
            .calls
            .iter()
            .all(|(_, thread)| *thread == trace.calls[0].1)
    );
}

#[test]
fn r64_tokio_current_thread_uses_real_pending_wakeup() {
    tokio_wakeup(false, false);
}
#[test]
fn r64_tokio_multithread_uses_real_pending_wakeup() {
    tokio_wakeup(true, false);
}
#[test]
fn r64_tokio_aborted_observer_does_not_cancel_owner_progress() {
    tokio_wakeup(true, true);
}

#[test]
fn r64_local_pool_drives_non_send_observer_with_owner_wakeup() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let (stream, kernel) = launch_fixture(&handle);
    let operation = handle
        .enqueue_launch(
            RuntimeAsyncLaunchRequestV1::new(stream, kernel, &EmptyArgs, geometry(), Vec::new())
                .unwrap(),
        )
        .unwrap();
    wait_until(|| state.lock().unwrap().issues.len() == 1);
    let raw = state.lock().unwrap().issues[0].1;
    let (sent, received) = oneshot::channel();
    let local = Rc::new(Cell::new(false));
    let observer = async {
        let result = pending_notification(operation, sent).await;
        local.set(true);
        result
    };
    let producer = async {
        received.await.unwrap();
        state
            .lock()
            .unwrap()
            .statuses
            .insert(raw, BackendPollV1::Succeeded);
    };
    let (cancel, cancelled) = sync_channel::<()>(1);
    let (expired, timeout) = oneshot::channel();
    let timed_out = Arc::new(AtomicBool::new(false));
    let watchdog_timeout = timed_out.clone();
    let watchdog = thread::spawn(move || {
        if matches!(
            cancelled.recv_timeout(Duration::from_secs(5)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            watchdog_timeout.store(true, Ordering::Release);
            let _ = expired.send(());
        }
    });
    let mut executor = futures_executor::LocalPool::new();
    let outcome = executor.run_until(futures_util::future::select(
        Box::pin(futures_util::future::join(observer, producer)),
        Box::pin(timeout),
    ));
    drop(cancel);
    watchdog.join().unwrap();
    assert!(
        !timed_out.load(Ordering::Acquire),
        "watchdog must not rescue a lost owner wakeup"
    );
    let futures_util::future::Either::Left(((result, ()), _)) = outcome else {
        panic!("LocalPool missed owner wakeup")
    };
    assert_eq!(
        result.unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert!(local.get());
    assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
    let shutdown = engine.shutdown().unwrap();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert!(shutdown.cleanup.unwrap().is_complete());
}

struct BytesArgs;
impl RuntimeArgumentsV1 for BytesArgs {
    const SIGNATURE_V1: [u8; 32] = [19; 32];
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; 8]
    }
    fn bindings_v1(&self) -> Vec<crate::RuntimeBindingV1> {
        Vec::new()
    }
}

#[test]
fn r64_tokio_timer_recovers_same_budgeted_operation_without_cancellation() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let (stream, kernel) = handle
        .observer()
        .try_with_context(|context| {
            let device = context.devices()[0].id();
            let stream = context.create_stream(device).unwrap();
            let module = context.load_module(device, &[1]).unwrap();
            (
                stream,
                Arc::new(
                    context
                        .resolve_kernel::<BytesArgs>(module, "bytes")
                        .unwrap(),
                ),
            )
        })
        .unwrap();
    let (entered, waiting) = sync_channel(1);
    let (release, gate) = sync_channel(1);
    let held = handle
        .observer()
        .enqueue_with_context(move |_| {
            entered.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
        })
        .unwrap();
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    let request = || {
        RuntimeAsyncLaunchRequestV1::new(stream, kernel.clone(), &BytesArgs, geometry(), Vec::new())
            .unwrap()
    };
    let cancelled = handle.enqueue_launch_tracked(request()).unwrap();
    assert_eq!(
        cancelled.control().cancel_before_submission(),
        crate::RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
    );
    let operation = handle.enqueue_launch_tracked(request()).unwrap();
    let identity = operation.control();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            let RuntimeAsyncTimeoutResultV1::TimedOut { operation } = operation
                .observe_with_timeout(tokio::time::sleep(Duration::from_millis(10)))
                .await
            else {
                panic!("held owner cannot finish")
            };
            assert!(identity.same_operation(&operation.control()));
            assert_eq!(identity.phase(), RuntimeAsyncOperationPhaseV1::Queued);
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 16);
            release.send(()).unwrap();
            held.await.unwrap();
            assert!(matches!(
                cancelled.await,
                Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
            ));
            while state.lock().unwrap().issues.is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            for status in state.lock().unwrap().statuses.values_mut() {
                *status = BackendPollV1::Succeeded;
            }
            assert_eq!(
                operation.await.unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        })
        .await
        .unwrap();
    });
    assert_eq!(state.lock().unwrap().issues.len(), 1);
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}
