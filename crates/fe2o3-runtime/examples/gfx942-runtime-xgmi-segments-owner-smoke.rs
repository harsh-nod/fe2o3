//! Native owner-thread correctness qualification, not a performance benchmark.
//! Requires fresh identity/activity admission of both endpoints by the runner.

#![forbid(unsafe_code)]

use fe2o3_runtime::*;
use std::{
    error::Error,
    future::Future,
    sync::{Arc, mpsc},
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};

type ResultV1<T> = Result<T, Box<dyn Error + Send + Sync>>;
type GpuContext = RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>;
const BYTES: usize = 65_536;
const TIMEOUT: Duration = Duration::from_secs(60);

struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn await_bounded<F: Future>(future: F) -> ResultV1<F::Output> {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return Ok(result);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("owner observation deadline".into());
        }
        thread::park_timeout(remaining);
    }
}

fn wait_phase(
    control: &RuntimeAsyncOperationControlV1,
    expected: RuntimeAsyncOperationPhaseV1,
) -> ResultV1<()> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let phase = control.phase();
        if phase == expected {
            return Ok(());
        }
        if Instant::now() >= deadline
            || matches!(
                phase,
                RuntimeAsyncOperationPhaseV1::CancelledBeforeSubmission
                    | RuntimeAsyncOperationPhaseV1::ObservationFinished
                    | RuntimeAsyncOperationPhaseV1::StoppedBeforeSubmission
                    | RuntimeAsyncOperationPhaseV1::StoppedAfterSubmission
            )
        {
            return Err(format!("expected {expected:?}, observed {phase:?}").into());
        }
        // No Context operation, event poll or GPU progress is driven here.
        thread::sleep(Duration::from_millis(1));
    }
}

fn ids(arguments: &[String]) -> ResultV1<[u64; 2]> {
    if arguments.len() != 2 {
        return Err("expected two unique IDs".into());
    }
    let parse = |text: &str| -> ResultV1<u64> {
        let value = match text.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16)?,
            None => text.parse()?,
        };
        if value == 0 {
            return Err("zero unique ID".into());
        }
        Ok(value)
    };
    let values = [parse(&arguments[0])?, parse(&arguments[1])?];
    if values[0] == values[1] {
        return Err("distinct GPUs required".into());
    }
    Ok(values)
}

fn descriptors() -> Vec<RuntimePeerCopySegmentV1> {
    let a = RuntimePeerCopySegmentV1 {
        source_offset: 0,
        destination_offset: 0,
        byte_len: 64,
    };
    let b = RuntimePeerCopySegmentV1 {
        source_offset: 128,
        destination_offset: 32,
        byte_len: 64,
    };
    let mut segments = vec![a, b, a];
    segments.extend((0..62).map(|i| RuntimePeerCopySegmentV1 {
        source_offset: 512 + i * 31,
        destination_offset: 512 + i * 29,
        byte_len: 19,
    }));
    segments
}

fn input() -> Vec<u8> {
    (0..BYTES)
        .map(|i| ((i * 29 + i / 257) % 251) as u8)
        .collect()
}

fn expected(segments: &[RuntimePeerCopySegmentV1]) -> Vec<u8> {
    let input = input();
    let mut output = vec![0xa5; BYTES];
    for segment in segments {
        let from = 32 + segment.source_offset as usize;
        let to = 64 + segment.destination_offset as usize;
        let bytes = segment.byte_len as usize;
        output[to..to + bytes].copy_from_slice(&input[from..from + bytes]);
    }
    output
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: u64,
    bytes: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset,
        byte_len: bytes,
    }
}

#[derive(Clone, Copy)]
struct Fixture {
    seed: RuntimeAllocationIdV1,
    staging: RuntimeAllocationIdV1,
    tracked_destination: RuntimeAllocationIdV1,
    untracked_destination: RuntimeAllocationIdV1,
    producer_stream: RuntimeStreamIdV1,
    tracked_stream: RuntimeStreamIdV1,
    untracked_stream: RuntimeStreamIdV1,
    producer_event: RuntimeEventIdV1,
}

fn setup(context: &mut GpuContext) -> ResultV1<Fixture> {
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let mut allocate =
        |device| context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 4096);
    let seed = allocate(devices[1])?;
    let staging = allocate(devices[0])?;
    let tracked_destination = allocate(devices[1])?;
    let untracked_destination = allocate(devices[0])?;
    context.write_allocation(seed, 0, &input())?;
    for allocation in [staging, tracked_destination, untracked_destination] {
        context.write_allocation(allocation, 0, &vec![0xa5; BYTES])?;
    }
    let producer_stream = context.create_stream(devices[0])?;
    let tracked_stream = context.create_stream(devices[1])?;
    let untracked_stream = context.create_stream(devices[0])?;
    let producer = context.peer_copy(
        producer_stream,
        region(seed, RuntimeAccessV1::Read, 0, BYTES as u64),
        region(staging, RuntimeAccessV1::Write, 0, BYTES as u64),
        &[],
    )?;
    let producer_event = context.record_event(&producer)?;
    // Deliberately retain all resources in Context until owner shutdown.
    Ok(Fixture {
        seed,
        staging,
        tracked_destination,
        untracked_destination,
        producer_stream,
        tracked_stream,
        untracked_stream,
        producer_event,
    })
}

fn check_stream(
    context: &mut GpuContext,
    stream: RuntimeStreamIdV1,
    pending: bool,
) -> ResultV1<()> {
    let status = context.query_stream(stream)?;
    if status.total_submissions != 1
        || status.pending != pending as usize
        || status.succeeded != (!pending) as usize
    {
        return Err(format!("unexpected stream accounting: {status:?}").into());
    }
    Ok(())
}

fn main() -> ResultV1<()> {
    let ids = ids(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || -> ResultV1<_> {
            let backend = KfdNativeXgmiRuntimeBackendV1::open_default(ids[0], ids[1])?;
            Ok(
                RuntimeContextV1::open_with_version_journal_v1(backend, 16, 8)
                    .map_err(|error| format!("context initialization: {error:?}"))?,
            )
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .map_err(|error| format!("owner initialization: {error}"))?;
    let (fixture, owner) = await_bounded(handle.observer().enqueue_with_context(
        |context| -> ResultV1<_> { Ok((setup(context)?, thread::current().id())) },
    )?)???;
    if owner == thread::current().id() {
        return Err("backend is not on a separate owner thread".into());
    }

    // Pending writer -> future reader handoff is not implemented by the journal.
    // Record its fail-closed boundary instead of disabling version ownership.
    let refused = handle.peer_copy_segments_tracked(
        fixture.tracked_stream,
        region(fixture.staging, RuntimeAccessV1::Read, 32, 32_768),
        region(
            fixture.tracked_destination,
            RuntimeAccessV1::Write,
            64,
            49_152,
        ),
        descriptors(),
        vec![fixture.producer_event],
    )?;
    let refused_control = refused.control();
    let refused = await_bounded(refused)??;
    if refused.submission.is_some()
        || !matches!(
            refused.observation,
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        )
        || refused.rejected_observations != 0
        || refused.last_rejected_observation.is_some()
        || refused_control.phase() != RuntimeAsyncOperationPhaseV1::ObservationFinished
    {
        return Err("pending versioned input did not reject before submission".into());
    }
    await_bounded(
        handle
            .observer()
            .enqueue_with_context(move |context| -> ResultV1<()> {
                check_stream(context, fixture.producer_stream, true)?;
                let status = context.query_stream(fixture.tracked_stream)?;
                if status.total_submissions != 0 || status.pending != 0 || status.succeeded != 0 {
                    return Err("refused consumer reached Context submission".into());
                }
                let mut observed = vec![0; BYTES];
                context.read_allocation(fixture.tracked_destination, 0, &mut observed)?;
                if observed != vec![0xa5; BYTES] {
                    return Err("refused consumer changed destination".into());
                }
                Ok(())
            })?,
    )???;
    let mut producer = await_bounded(handle.enqueue_event_registration_with_progress(
        fixture.producer_stream,
        fixture.producer_event,
    )?)???;
    if await_bounded(&mut producer)?? != RuntimeCompletionStatusV1::Succeeded
        || producer.progress_failure_count() != 0
    {
        return Err("producer did not complete cleanly".into());
    }
    drop(producer);

    let (release, gate) = mpsc::sync_channel(1);
    let (entered, entry) = mpsc::sync_channel(1);
    let held = handle
        .observer()
        .enqueue_with_context(move |_| -> ResultV1<()> {
            entered.send(())?;
            gate.recv_timeout(TIMEOUT)?;
            Ok(())
        })?;
    entry.recv_timeout(TIMEOUT)?;
    let cancelled = handle.peer_copy_segments_tracked(
        fixture.tracked_stream,
        region(fixture.staging, RuntimeAccessV1::Read, 0, BYTES as u64),
        region(
            fixture.tracked_destination,
            RuntimeAccessV1::Write,
            0,
            BYTES as u64,
        ),
        vec![RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 0,
            byte_len: BYTES as u64,
        }],
        vec![fixture.producer_event],
    )?;
    let cancelled_control = cancelled.control();
    if cancelled_control.cancel_before_submission()
        != RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
    {
        return Err("gated cancellation did not win".into());
    }
    let tracked = handle.peer_copy_segments_tracked(
        fixture.tracked_stream,
        region(fixture.staging, RuntimeAccessV1::Read, 32, 32_768),
        region(
            fixture.tracked_destination,
            RuntimeAccessV1::Write,
            64,
            49_152,
        ),
        descriptors(),
        vec![fixture.producer_event],
    )?;
    let control = tracked.control();
    let recovered = match await_bounded(tracked.observe_with_timeout(std::future::ready(())))? {
        RuntimeAsyncTimeoutResultV1::TimedOut { operation } => operation,
        RuntimeAsyncTimeoutResultV1::Completed(_) => return Err("gated operation completed".into()),
    };
    if !control.same_operation(&recovered.control())
        || control.phase() != RuntimeAsyncOperationPhaseV1::Queued
    {
        return Err("timeout lost the queued operation identity".into());
    }
    drop(recovered);
    release.send(())?;
    await_bounded(held)???;
    if !matches!(
        await_bounded(cancelled)?,
        Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
    ) || cancelled_control.phase() != RuntimeAsyncOperationPhaseV1::CancelledBeforeSubmission
    {
        return Err("cancelled operation was submitted".into());
    }
    wait_phase(&control, RuntimeAsyncOperationPhaseV1::ObservationFinished)?;
    await_bounded(
        handle
            .observer()
            .enqueue_with_context(move |context| -> ResultV1<()> {
                check_stream(context, fixture.producer_stream, false)?;
                check_stream(context, fixture.tracked_stream, false)
            })?,
    )???;

    let untracked = await_bounded(handle.peer_copy_segments(
        fixture.untracked_stream,
        region(fixture.seed, RuntimeAccessV1::Read, 32, 32_768),
        region(
            fixture.untracked_destination,
            RuntimeAccessV1::Write,
            64,
            49_152,
        ),
        descriptors(),
        Vec::new(),
    )?)??;
    if untracked.submission.is_none()
        || untracked.observation? != RuntimeCompletionStatusV1::Succeeded
        || untracked.rejected_observations != 0
        || untracked.last_rejected_observation.is_some()
    {
        return Err("untracked whole-list completion failed".into());
    }
    await_bounded(
        handle
            .observer()
            .enqueue_with_context(move |context| -> ResultV1<()> {
                check_stream(context, fixture.untracked_stream, false)?;
                let mut observed = vec![0; BYTES];
                for allocation in [fixture.seed, fixture.staging] {
                    context.read_allocation(allocation, 0, &mut observed)?;
                    if observed != input() {
                        return Err("source or producer bytes changed".into());
                    }
                }
                for allocation in [fixture.tracked_destination, fixture.untracked_destination] {
                    context.read_allocation(allocation, 0, &mut observed)?;
                    if observed != expected(&descriptors()) {
                        return Err("ordered bytes or canaries differ".into());
                    }
                }
                Ok(())
            })?,
    )???;
    if handle.observer().snapshot_bytes_in_use() != 0 {
        return Err("snapshot credit retained after submission".into());
    }
    let shutdown = engine.shutdown()?;
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(format!("owner cleanup incomplete: {shutdown:?}").into());
    }
    println!(
        "PASS schema=fe2o3.runtime.xgmi-segments-owner.v1 uid0={:016x} uid1={:016x} owner_threads=1 allocations=4 streams=3 segments=65 ordered_lists=2 producer_copies=1 cancelled=not_submitted timeout_identity=retained dropped_observer=completed journal=enabled pending_dataflow=refused_before_submission dependency=completed_producer directions=both canaries=complete source=unchanged cleanup=complete performance_claim=false",
        ids[0], ids[1]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_reject_wrong_count_zero_duplicate_and_overflow() {
        let parse = |args: &[&str]| ids(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(parse(&["0xab", "172"]).unwrap(), [171, 172]);
        for args in [
            &[][..],
            &["1"][..],
            &["1", "2", "3"][..],
            &["0", "2"][..],
            &["1", "1"][..],
            &["0x", "2"][..],
            &["18446744073709551616", "2"][..],
        ] {
            assert!(parse(args).is_err());
        }
    }

    #[test]
    fn plan_fits_unequal_envelopes_and_exceeds_one_flush_budget() {
        let segments = descriptors();
        assert_eq!(segments.len(), 65);
        assert!(
            fe2o3_runtime_model::validate_ordered_peer_copy_segments_v1(
                32, 32_768, 64, 49_152, &segments
            )
            .is_ok()
        );
        let output = expected(&segments);
        assert!(output[..64].iter().all(|b| *b == 0xa5));
        assert!(output[64 + 49_152..].iter().all(|b| *b == 0xa5));
        assert_ne!(output, vec![0xa5; BYTES]);
    }

    #[test]
    fn duplicate_and_overlapping_order_are_byte_observable() {
        let plan = descriptors();
        let exact = expected(&plan);
        let mut without_duplicate = plan.clone();
        without_duplicate.remove(2);
        assert_ne!(exact, expected(&without_duplicate));
        let mut reordered = plan.clone();
        reordered.swap(1, 2);
        assert_ne!(exact, expected(&reordered));
        assert_ne!(exact, expected(&plan[..8]));
    }
}
