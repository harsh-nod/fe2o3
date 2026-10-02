use super::*;
use crate::{
    RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDriveErrorV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1,
};
use fe2o3_runtime_model::ContextAllocationStateV1;

type Calls = Arc<Mutex<Vec<(usize, RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>;
type Producer = RuntimeSubmissionV1<MixedArguments>;

struct Handles {
    compute_stream: RuntimeStreamIdV1,
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    kernel: TypedRuntimeKernelV1<MixedArguments>,
}

fn setup() -> (Context, Handles, Producer, RuntimeEventIdV1) {
    let Fixture {
        context,
        compute_stream,
        peer_stream,
        readback_stream,
        source,
        destination,
        host,
        kernel,
        producer,
        event,
    } = Fixture::new();
    (
        context,
        Handles {
            compute_stream,
            peer_stream,
            readback_stream,
            source,
            destination,
            host,
            kernel,
        },
        producer,
        event,
    )
}

fn callback<A>(
    context: &mut Context,
    submission: &RuntimeSubmissionV1<A>,
    batch: usize,
    calls: &Calls,
) {
    let calls = Arc::clone(calls);
    let id = submission.id;
    context
        .on_completion(submission, move |status| {
            calls.lock().unwrap().push((batch, id, status));
        })
        .unwrap();
}

struct Report {
    released_event: RuntimeEventIdV1,
    states: [ContextAllocationStateV1; 3],
    bytes: [u8; 64],
}

fn batch(
    context: &mut Context,
    handles: &Handles,
    initial: Option<(Producer, RuntimeEventIdV1)>,
    ordinal: usize,
    stale_event: Option<RuntimeEventIdV1>,
    calls: &Calls,
) -> Report {
    let allocations = [handles.source, handles.destination, handles.host];
    let (mut producer, event) = if let Some(initial) = initial {
        initial
    } else {
        assert!(context.submissions.is_empty());
        assert!(context.events.is_empty());
        assert!(context.producer_launches.is_empty());
        assert!(context.scalar_peer_copies.is_empty());
        assert!(context.same_device_copies.is_empty());
        assert_eq!(context.version_journal_read_records_v1(), Some(0));
        assert_eq!(context.version_journal_writer_records_v1(), Some(0));
        for (index, allocation) in allocations.into_iter().enumerate() {
            let before = state(context, allocation);
            let fresh = [0x41 + index as u8; 64];
            context.write_allocation(allocation, 0, &fresh).unwrap();
            let after = state(context, allocation);
            assert_eq!(after.attempt_epoch, before.attempt_epoch + 1);
            assert_eq!(after.content_lineage, before.content_lineage + 1);
            let mut observed = [0; 64];
            context
                .read_allocation(allocation, 0, &mut observed)
                .unwrap();
            assert_eq!(observed, fresh);
        }
        let producer = context
            .launch_producer_aware_v1(
                handles.compute_stream,
                &handles.kernel,
                &MixedArguments(vec![span(handles.source, RuntimeAccessV1::Write, 0, 64)]),
                geometry(),
                &[],
            )
            .unwrap();
        let event = context.record_event(&producer).unwrap();
        (producer, event)
    };
    if let Some(stale) = stale_event {
        let before = context.backend.copy_call_count;
        validation(
            context.peer_copy(
                handles.peer_stream,
                span(handles.source, RuntimeAccessV1::Read, 0, 64),
                span(handles.destination, RuntimeAccessV1::Write, 0, 64),
                &[stale],
            ),
            RuntimeValidationErrorV1::UnknownEvent,
        );
        assert_eq!(context.backend.copy_call_count, before);
    }
    let peer = context
        .peer_copy(
            handles.peer_stream,
            span(handles.source, RuntimeAccessV1::Read, 0, 64),
            span(handles.destination, RuntimeAccessV1::Write, 0, 64),
            &[event],
        )
        .unwrap();
    let peer_event = context.record_event(&peer).unwrap();
    let mut readback = context
        .copy_async(
            handles.readback_stream,
            span(handles.destination, RuntimeAccessV1::Read, 0, 64),
            span(handles.host, RuntimeAccessV1::Write, 0, 64),
            &[peer_event],
        )
        .unwrap();
    context.release_event(peer_event).unwrap();
    context.release_event(event).unwrap();
    let before = allocations.map(|allocation| state(context, allocation));
    let marker = context.submissions[&peer.id].journal_producer_read.unwrap();
    let lease = context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .lookup_producer_read(marker.active_first_for_test().unwrap())
        .unwrap();
    assert_eq!(
        lease.producer,
        context.submissions[&producer.id].journal_writer.unwrap()
    );
    assert_eq!(lease.read.attempt_epoch, before[0].attempt_epoch);
    assert_eq!(lease.read.content_lineage, before[0].content_lineage);
    for allocation in allocations {
        validation(
            context.write_allocation(allocation, 0, &[0xff; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            context.release_allocation(allocation),
            RuntimeValidationErrorV1::ContextReserved,
        );
    }
    let failure = context.release_submission(producer).unwrap_err();
    assert!(matches!(
        failure.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
    ));
    (producer, _) = failure.into_parts();
    assert_eq!(
        allocations.map(|allocation| state(context, allocation)),
        before
    );
    callback(context, &producer, ordinal, calls);
    callback(context, &peer, ordinal, calls);
    callback(context, &readback, ordinal, calls);
    let ids = [producer.id, peer.id, readback.id];
    let expected = [(producer.backend_submission as u8).wrapping_add(37); 64];
    // Invoke the existing mock operations, including their real byte writes;
    // Context success and journal settlement still require normal observation.
    for id in [
        producer.backend_submission,
        peer.backend_submission,
        readback.backend_submission,
    ] {
        context.backend.finish_submission(id, true);
        context.backend.polls.insert(id, 1);
    }
    for _ in 0..8 {
        if context.poll(&mut readback).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
    }
    for id in ids {
        let record = &context.submissions[&id];
        assert_eq!(record.status, RuntimeCompletionStatusV1::Succeeded);
        assert!(record.quiescent);
        assert_eq!(record.dependency_retains, 0);
    }
    assert_eq!(
        context.poll(&mut readback).unwrap(),
        RuntimePollV1::Succeeded
    );
    let observed: Vec<_> = calls
        .lock()
        .unwrap()
        .iter()
        .copied()
        .filter(|(batch, _, _)| *batch == ordinal)
        .collect();
    assert_eq!(
        observed,
        ids.map(|id| (ordinal, id, RuntimeCompletionStatusV1::Succeeded))
    );
    let states = allocations.map(|allocation| state(context, allocation));
    for (before, after) in before.into_iter().zip(states) {
        assert_eq!(after.attempt_epoch, before.attempt_epoch);
        assert_eq!(after.content_lineage, before.content_lineage + 1);
    }
    let mut bytes = [0; 64];
    context
        .read_allocation(handles.host, 0, &mut bytes)
        .unwrap();
    assert_eq!(bytes, expected);
    context.release_submission(readback).unwrap();
    context.release_submission(peer).unwrap();
    context.release_submission(producer).unwrap();
    assert!(context.submissions.is_empty());
    assert!(context.events.is_empty());
    assert!(context.producer_launches.is_empty());
    assert!(context.scalar_peer_copies.is_empty());
    assert!(context.same_device_copies.is_empty());
    assert_eq!(context.version_journal_read_records_v1(), Some(0));
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(context.completion_callback_count, 0);
    Report {
        released_event: event,
        states,
        bytes,
    }
}

fn changed(first: &Report, second: &Report, calls: &Calls) {
    assert_ne!(first.bytes, second.bytes);
    assert_ne!(first.released_event, second.released_event);
    for (first, second) in first.states.into_iter().zip(second.states) {
        assert_eq!(second.content_lineage, first.content_lineage + 2);
        assert_eq!(second.attempt_epoch, first.attempt_epoch + 2);
    }
    assert_eq!(calls.lock().unwrap().len(), 6);
}

#[test]
fn compute_peer_two_changed_batches_reuse_context_handles_without_stale_leases() {
    let (mut context, handles, producer, event) = setup();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let first = batch(
        &mut context,
        &handles,
        Some((producer, event)),
        0,
        None,
        &calls,
    );
    let second = batch(
        &mut context,
        &handles,
        None,
        1,
        Some(first.released_event),
        &calls,
    );
    changed(&first, &second, &calls);
    assert_eq!(context.backend.allocation_calls, 3);
    assert_eq!(context.backend.enumeration_calls, 1);
    assert!(context.cleanup().is_complete());
}

#[test]
fn compute_peer_repeat_owner_commands_resume_deadline_and_dispose_completed_reply_credit() {
    let (context, handles, producer, event) = setup();
    let handles = Arc::new(handles);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let config = RuntimeAsyncEngineConfigV1::new(4, 4, 4, 4, Duration::from_millis(1))
        .unwrap()
        .with_reply_capacity(1)
        .unwrap();
    let progress = RuntimeAsyncProgressConfigV1::new(4, 4).unwrap();
    let (mut owner, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        move || Ok::<_, MockError>(context),
        config,
        progress,
    )
    .unwrap();
    let mut initial = Some((producer, event));
    let mut previous: Option<Report> = None;
    for ordinal in 0..2 {
        let command_handles = Arc::clone(&handles);
        let command_calls = Arc::clone(&calls);
        let initial = initial.take();
        let stale = previous.as_ref().map(|report| report.released_event);
        let mut reply = handle
            .observer()
            .enqueue_with_context(move |context| {
                batch(
                    context,
                    &command_handles,
                    initial,
                    ordinal,
                    stale,
                    &command_calls,
                )
            })
            .unwrap();
        assert_eq!(handle.observer().reply_cells_in_use(), 1);
        assert!(matches!(
            owner.drive_until_ready(std::pin::Pin::new(&mut reply), Instant::now()),
            Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
        ));
        assert_eq!(calls.lock().unwrap().len(), ordinal * 3);
        let report = owner
            .drive_until_ready(
                std::pin::Pin::new(&mut reply),
                Instant::now() + Duration::from_secs(1),
            )
            .unwrap()
            .unwrap();
        assert_eq!(handle.observer().reply_cells_in_use(), 1);
        assert!(handle.observer().enqueue_with_context(|_| ()).is_err());
        drop(reply);
        assert_eq!(handle.observer().reply_cells_in_use(), 0);
        if let Some(first) = &previous {
            changed(first, &report, &calls);
        }
        previous = Some(report);
    }
    drop(handle);
    let shutdown = owner.shutdown();
    assert_eq!(
        shutdown.disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert!(shutdown.cleanup.unwrap().is_complete());
    assert!(shutdown.native_failure.is_none());
    assert!(!shutdown.worker_panicked);
}
