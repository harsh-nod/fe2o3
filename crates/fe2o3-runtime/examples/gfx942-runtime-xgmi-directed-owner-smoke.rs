//! Native directed dependency-diamond qualification, not a performance benchmark.
//! Requires fresh identity/activity admission of both endpoints by the runner.

#![forbid(unsafe_code)]

use fe2o3_runtime::*;
use std::{
    error::Error,
    fmt,
    future::Future,
    sync::{Arc, mpsc},
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};

type ResultV1<T> = Result<T, Box<dyn Error + Send + Sync>>;
type GpuContext = RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdNativeXgmiRuntimeBackendV1>;
type Submission = RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;
const BYTES: usize = 65_536;
const COPY_BYTES: usize = 32_768;
const TIMEOUT: Duration = Duration::from_secs(60);
const HOMES: [usize; 5] = [0, 1, 0, 0, 1];
const OFFSETS: [usize; 5] = [4096, 8192, 12288, 16384, 20480];
const CANARIES: [u8; 4] = [0xa1, 0xb2, 0xc3, 0xd4];
const EDGES: [(usize, usize); 4] = [(0, 1), (1, 2), (1, 3), (2, 4)];

struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn remaining(deadline: Instant) -> ResultV1<Duration> {
    let duration = deadline.saturating_duration_since(Instant::now());
    if duration.is_zero() {
        Err("owner observation deadline".into())
    } else {
        Ok(duration)
    }
}

fn await_until<F: Future>(deadline: Instant, future: F) -> ResultV1<F::Output> {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    loop {
        remaining(deadline)?;
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return Ok(result);
        }
        thread::park_timeout(remaining(deadline)?);
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

fn input_byte(index: usize) -> u8 {
    ((index * 29 + index / 257) % 127) as u8
}

fn expected(allocation: usize) -> Vec<u8> {
    if allocation == 0 {
        return (0..BYTES).map(input_byte).collect();
    }
    // Independent oracle: derive every output from the original seed, not from
    // a prior readback or a replay of the copy edges.
    let mut output = vec![CANARIES[allocation - 1]; BYTES];
    for index in 0..COPY_BYTES {
        output[OFFSETS[allocation] + index] = input_byte(4096 + index);
    }
    output
}

fn check_bytes(allocation: usize, actual: &[u8]) -> ResultV1<()> {
    if actual != expected(allocation) {
        return Err(format!("allocation {allocation}: payload, source or guards differ").into());
    }
    Ok(())
}

fn region(
    allocations: &[RuntimeAllocationIdV1; 5],
    index: usize,
    write: bool,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation: allocations[index],
        access: if write {
            RuntimeAccessV1::Write
        } else {
            RuntimeAccessV1::Read
        },
        byte_offset: OFFSETS[index] as u64,
        byte_len: COPY_BYTES as u64,
    }
}

struct Fixture {
    allocations: [RuntimeAllocationIdV1; 5],
    streams: [RuntimeStreamIdV1; 4],
    producers: [Submission; 3],
    events: [RuntimeEventIdV1; 3],
    owner: thread::ThreadId,
}

fn check_owner(owner: thread::ThreadId) -> ResultV1<()> {
    if thread::current().id() != owner {
        return Err("owner thread changed".into());
    }
    Ok(())
}

fn check_stream(context: &GpuContext, stream: RuntimeStreamIdV1, pending: bool) -> ResultV1<()> {
    let status = context.query_stream(stream)?;
    let expected = RuntimeStreamObservationV1 {
        total_submissions: 1,
        pending: usize::from(pending),
        succeeded: usize::from(!pending),
        ..RuntimeStreamObservationV1::default()
    };
    if status != expected {
        return Err(format!("unexpected stream accounting: {status:?}").into());
    }
    Ok(())
}

fn setup(context: &mut GpuContext) -> ResultV1<Fixture> {
    if context.devices().len() != 2 {
        return Err("exactly two native endpoints required".into());
    }
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let mut allocate = |index| {
        context.allocate(
            devices[index],
            RuntimeMemoryKindV1::DeviceLocal,
            BYTES as u64,
            4096,
        )
    };
    let allocations = [
        allocate(HOMES[0])?,
        allocate(HOMES[1])?,
        allocate(HOMES[2])?,
        allocate(HOMES[3])?,
        allocate(HOMES[4])?,
    ];
    context.write_allocation(allocations[0], 0, &expected(0))?;
    for index in 1..5 {
        context.write_allocation(allocations[index], 0, &vec![CANARIES[index - 1]; BYTES])?;
    }
    let streams = [
        context.create_stream(devices[1])?,
        context.create_stream(devices[0])?,
        context.create_stream(devices[0])?,
        context.create_stream(devices[1])?,
    ];
    let mut submit = |index: usize, dependencies: &[RuntimeEventIdV1]| -> ResultV1<_> {
        let (from, to) = EDGES[index];
        let submission = context.directed_peer_copy_v1(
            streams[index],
            region(&allocations, from, false),
            region(&allocations, to, true),
            dependencies,
        )?;
        let event = context.record_event(&submission)?;
        Ok((submission, event))
    };
    let (root, root_event) = submit(0, &[])?;
    let (left, left_event) = submit(1, &[root_event])?;
    let (right, right_event) = submit(2, &[root_event])?;
    let producers = [root, left, right];
    for (index, producer) in producers.iter().enumerate() {
        check_stream(context, streams[index], true)?;
        if context.query_submission(producer)? != RuntimeCompletionStatusV1::Pending {
            return Err("producer progressed during setup".into());
        }
    }
    if context.query_stream(streams[3])? != RuntimeStreamObservationV1::default()
        || context.version_journal_read_records_v1() != Some(3)
    {
        return Err("unexpected setup journal or join state".into());
    }
    Ok(Fixture {
        allocations,
        streams,
        producers,
        events: [root_event, left_event, right_event],
        owner: thread::current().id(),
    })
}

fn workload(handle: &Handle, deadline: Instant) -> ResultV1<()> {
    let fixture = await_until(deadline, handle.observer().enqueue_with_context(setup)?)???;
    let Fixture {
        allocations,
        streams,
        events,
        owner,
        ..
    } = fixture;
    if owner == thread::current().id() {
        return Err("backend is not on a separate owner thread".into());
    }

    // One command and one operation advance per tick separate join admission
    // from its first progress. Dropping release on any error unblocks the gate.
    let (release, gate) = mpsc::sync_channel(1);
    let (entered, entry) = mpsc::sync_channel(1);
    let held = handle
        .observer()
        .enqueue_with_context(move |_| -> ResultV1<()> {
            check_owner(owner)?;
            entered.send(())?;
            gate.recv_timeout(remaining(deadline)?)?;
            Ok(())
        })?;
    entry.recv_timeout(remaining(deadline)?)?;
    let (from, to) = EDGES[3];
    let join = handle.directed_peer_copy_tracked(
        streams[3],
        region(&allocations, from, false),
        region(&allocations, to, true),
        vec![events[1], events[2]],
    )?;
    let control = join.control();
    if control.phase() != RuntimeAsyncOperationPhaseV1::Queued
        || handle.observer().snapshot_bytes_in_use() != 2 * size_of::<RuntimeEventIdV1>()
    {
        return Err("gated join identity or snapshot charge mismatch".into());
    }
    let released = handle
        .observer()
        .enqueue_with_context(move |context| -> ResultV1<()> {
            check_owner(owner)?;
            for stream in streams {
                check_stream(context, stream, true)?;
            }
            if context.version_journal_read_records_v1() != Some(4) {
                return Err("join was not admitted before event release".into());
            }
            for event in events {
                context.release_event(event)?;
            }
            Ok(())
        })?;
    release.send(())?;
    await_until(deadline, held)???;
    await_until(deadline, released)???;
    let result = await_until(deadline, join)??;
    if result.observation? != RuntimeCompletionStatusV1::Succeeded
        || result.rejected_observations != 0
        || result.last_rejected_observation.is_some()
        || control.phase() != RuntimeAsyncOperationPhaseV1::ObservationFinished
    {
        return Err("join did not complete without rejected observations".into());
    }
    let join = result.submission.ok_or("join submission missing")?;
    if join.stream() != streams[3] || handle.observer().snapshot_bytes_in_use() != 0 {
        return Err("join stream identity or snapshot settlement mismatch".into());
    }
    await_until(
        deadline,
        handle
            .observer()
            .enqueue_with_context(move |context| -> ResultV1<()> {
                check_owner(owner)?;
                for submission in fixture.producers.iter().chain(std::iter::once(&join)) {
                    if context.query_submission(submission)? != RuntimeCompletionStatusV1::Succeeded
                    {
                        return Err("exact diamond submission did not succeed".into());
                    }
                }
                for stream in streams {
                    check_stream(context, stream, false)?;
                }
                if context.version_journal_read_records_v1() != Some(0) {
                    return Err("terminal diamond retained journal inputs".into());
                }
                let mut observed = vec![0; BYTES];
                for (index, allocation) in allocations.into_iter().enumerate() {
                    context.read_allocation(allocation, 0, &mut observed)?;
                    check_bytes(index, &observed)?;
                }
                // Context retains all native resources until explicit owner shutdown.
                Ok(())
            })?,
    )???;
    Ok(())
}

#[derive(Debug)]
struct WorkloadAndCleanupError {
    primary: Box<dyn Error + Send + Sync>,
    cleanup: Box<dyn Error + Send + Sync>,
}
impl fmt::Display for WorkloadAndCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}; secondary owner cleanup failure: {}",
            self.primary, self.cleanup
        )
    }
}
impl Error for WorkloadAndCleanupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary.as_ref())
    }
}

fn finish(work: ResultV1<()>, cleanup: ResultV1<()>) -> ResultV1<()> {
    match (work, cleanup) {
        (Ok(()), cleanup) => cleanup,
        (Err(primary), Ok(())) => Err(primary),
        (Err(primary), Err(cleanup)) => Err(Box::new(WorkloadAndCleanupError { primary, cleanup })),
    }
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
        RuntimeAsyncEngineConfigV1::new(16, 16, 1, 1, Duration::from_millis(1))?,
        RuntimeAsyncProgressConfigV1::new(16, 1)?,
    )
    .map_err(|error| format!("owner initialization: {error}"))?;
    let work = workload(&handle, Instant::now() + TIMEOUT);
    let cleanup = (|| -> ResultV1<()> {
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
        Ok(())
    })();
    finish(work, cleanup)?;
    println!(
        "PASS schema=fe2o3.runtime.xgmi-directed-owner.v1 uid0={:016x} uid1={:016x} owner_threads=1 allocations=5 streams=4 directed_copies=4 dependency_edges=4 max_depth=3 routes=forward-reverse-reverse-forward journal=enabled pending_dataflow=supported adapter=join-tracked events=released-before-progress statuses=4-succeeded rejected=0 checked_bytes=327680 payload_bytes=131072 guard_bytes=131072 source_bytes=65536 canaries=complete source=unchanged cleanup=complete native_execution=true exclusive_reservation=false performance_claim=false formal_refinement=false",
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
    fn plan_has_exact_routes_ranges_and_byte_accounting() {
        assert_eq!(EDGES, [(0, 1), (1, 2), (1, 3), (2, 4)]);
        assert_eq!(OFFSETS, [4096, 8192, 12288, 16384, 20480]);
        assert_eq!(
            EDGES.map(|(from, to)| (HOMES[from], HOMES[to])),
            [(0, 1), (1, 0), (1, 0), (0, 1)]
        );
        assert!(
            OFFSETS
                .iter()
                .all(|offset| *offset > 0 && offset + COPY_BYTES < BYTES)
        );
        assert_eq!(5 * BYTES, 327_680);
        assert_eq!(4 * COPY_BYTES, 131_072);
        assert_eq!(4 * (BYTES - COPY_BYTES), 131_072);
    }

    #[test]
    fn oracle_detects_payload_guard_and_source_boundary_mutations() {
        for allocation in 0..5 {
            let oracle = expected(allocation);
            check_bytes(allocation, &oracle).unwrap();
            // Every position contributes to the exact equality, including the
            // right branch that the join orders but does not use as data input.
            for index in [
                0,
                OFFSETS[allocation] - 1,
                OFFSETS[allocation],
                OFFSETS[allocation] + COPY_BYTES - 1,
                OFFSETS[allocation] + COPY_BYTES,
                BYTES - 1,
            ] {
                let mut bad = oracle.clone();
                bad[index] ^= 1;
                assert!(check_bytes(allocation, &bad).is_err());
            }
            assert!(check_bytes(allocation, &oracle[..BYTES - 1]).is_err());
            if allocation != 0 {
                let start = OFFSETS[allocation];
                assert!(
                    oracle[start..start + COPY_BYTES]
                        .iter()
                        .all(|byte| *byte < 127)
                );
                assert!(
                    oracle[..start]
                        .iter()
                        .chain(&oracle[start + COPY_BYTES..])
                        .all(|byte| *byte == CANARIES[allocation - 1])
                );
                assert!(check_bytes(allocation, &vec![CANARIES[allocation - 1]; BYTES]).is_err());
            }
        }
    }

    #[test]
    fn expired_deadline_rejects_even_ready_observation() {
        assert!(await_until(Instant::now(), std::future::ready(())).is_err());
        assert!(await_until(Instant::now() + TIMEOUT, std::future::ready(())).is_ok());
    }

    #[test]
    fn dropped_gate_sender_unblocks_receiver() {
        let (release, gate) = mpsc::sync_channel::<()>(1);
        drop(release);
        assert_eq!(
            gate.recv_timeout(TIMEOUT),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn cleanup_failure_does_not_replace_original_workload_error() {
        let both = finish(Err("workload".into()), Err("cleanup".into())).unwrap_err();
        assert_eq!(both.source().unwrap().to_string(), "workload");
        assert_eq!(
            both.to_string(),
            "workload; secondary owner cleanup failure: cleanup"
        );
        assert_eq!(
            finish(Err("workload".into()), Ok(()))
                .unwrap_err()
                .to_string(),
            "workload"
        );
        assert_eq!(
            finish(Ok(()), Err("cleanup".into()))
                .unwrap_err()
                .to_string(),
            "cleanup"
        );
        assert!(finish(Ok(()), Ok(())).is_ok());
    }
}
