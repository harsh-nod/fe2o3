//! Bounded direct-KFD owner-thread copy qualification, not a performance claim.

use fe2o3_runtime::*;
use std::error::Error;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

const BODY: usize = 1024 * 1024;
const PAD: usize = 128;
const TOTAL: usize = BODY + 2 * PAD;
type ResultV1<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug)]
struct CopyOnly;
// SAFETY: no request can receive executable authority from this implementation.
unsafe impl KfdRuntimeLaunchAuthorityV1 for CopyOnly {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn await_bounded<F: Future>(future: F) -> ResultV1<F::Output> {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return Ok(result);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("async qualification deadline expired".into());
        }
        thread::park_timeout(remaining);
    }
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: usize,
    len: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset as u64,
        byte_len: len as u64,
    }
}

fn run(unique_id: u64, tracked: bool) -> ResultV1<()> {
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || -> ResultV1<_> {
            let backend = KfdRuntimeBackendV1::open_default(unique_id, CopyOnly)?;
            Ok(RuntimeContextV1::open(backend)?)
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .map_err(|error| format!("owner initialization: {error}"))?;
    let (stream, upload, device, download, owner_thread) =
        await_bounded(handle.observer().enqueue_with_context(|context| {
            let selected = context.devices()[0].id();
            let stream = context.create_stream(selected)?;
            let upload = context.allocate(
                selected,
                RuntimeMemoryKindV1::HostVisible,
                TOTAL as u64,
                4096,
            )?;
            let device = context.allocate(
                selected,
                RuntimeMemoryKindV1::DeviceLocal,
                TOTAL as u64,
                4096,
            )?;
            let download = context.allocate(
                selected,
                RuntimeMemoryKindV1::HostVisible,
                TOTAL as u64,
                4096,
            )?;
            let input: Vec<_> = (0..TOTAL)
                .map(|i| ((i * 29 + i / 257) % 251) as u8)
                .collect();
            context.write_allocation(upload, 0, &input)?;
            context.write_allocation(device, 0, &vec![0xa7; TOTAL])?;
            context.write_allocation(download, 0, &vec![0x3c; TOTAL])?;
            // Gives the external queue census an observation window; not timed work.
            thread::sleep(Duration::from_millis(50));
            Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>((
                stream,
                upload,
                device,
                download,
                thread::current().id(),
            ))
        })?)???;
    assert_ne!(owner_thread, thread::current().id());
    if tracked {
        // Test-only admission gate: no performance measurement or timer claim.
        // Receiver disconnection and the external guard bound every error path.
        let (release, gate) = std::sync::mpsc::sync_channel(1);
        let (entered, entry) = std::sync::mpsc::sync_channel(1);
        let held = handle
            .observer()
            .enqueue_with_context(move |_| -> ResultV1<()> {
                entered.send(())?;
                gate.recv_timeout(Duration::from_secs(20))?;
                Ok(())
            })?;
        entry.recv_timeout(Duration::from_secs(20))?;
        let cancelled = handle.copy_async_tracked(
            stream,
            region(upload, RuntimeAccessV1::Read, 0, TOTAL),
            region(device, RuntimeAccessV1::Write, 0, TOTAL),
            Vec::new(),
        )?;
        let cancelled_control = cancelled.control();
        if cancelled_control.cancel_before_submission()
            != RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
        {
            return Err("gated copy was not cancelled before submission".into());
        }
        let upload_future = handle.copy_async_tracked(
            stream,
            region(upload, RuntimeAccessV1::Read, PAD, BODY),
            region(device, RuntimeAccessV1::Write, PAD, BODY),
            Vec::new(),
        )?;
        let upload_control = upload_future.control();
        let recovered =
            match await_bounded(upload_future.observe_with_timeout(std::future::ready(())))? {
                RuntimeAsyncTimeoutResultV1::TimedOut { operation } => operation,
                RuntimeAsyncTimeoutResultV1::Completed(_) => {
                    return Err("gated upload unexpectedly completed".into());
                }
            };
        if !upload_control.same_operation(&recovered.control())
            || upload_control.phase() != RuntimeAsyncOperationPhaseV1::Queued
        {
            return Err("timeout lost exact queued operation identity".into());
        }
        drop(recovered);
        release.send(())?;
        await_bounded(held)???;
        if !matches!(
            await_bounded(cancelled)?,
            Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
        ) || cancelled_control.phase() != RuntimeAsyncOperationPhaseV1::CancelledBeforeSubmission
        {
            return Err("cancelled operation lost its non-submission disposition".into());
        }
    } else {
        drop(handle.copy_async(
            stream,
            region(upload, RuntimeAccessV1::Read, PAD, BODY),
            region(device, RuntimeAccessV1::Write, PAD, BODY),
            Vec::new(),
        )?);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = await_bounded(
            handle
                .observer()
                .enqueue_with_context(move |context| context.query_stream(stream))?,
        )???;
        if status.total_submissions == 1 && status.pending == 0 {
            if status.succeeded != 1 {
                return Err("abandoned upload did not succeed".into());
            }
            break;
        }
        if Instant::now() >= deadline {
            return Err("abandoned upload did not make progress".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
    let download_result = if tracked {
        await_bounded(handle.copy_async_tracked(
            stream,
            region(device, RuntimeAccessV1::Read, 0, TOTAL),
            region(download, RuntimeAccessV1::Write, 0, TOTAL),
            Vec::new(),
        )?)??
    } else {
        await_bounded(handle.copy_async(
            stream,
            region(device, RuntimeAccessV1::Read, 0, TOTAL),
            region(download, RuntimeAccessV1::Write, 0, TOTAL),
            Vec::new(),
        )?)??
    };
    if download_result.observation? != RuntimeCompletionStatusV1::Succeeded {
        return Err("download did not succeed".into());
    }
    await_bounded(
        handle
            .observer()
            .enqueue_with_context(move |context| -> ResultV1<()> {
                let mut input = vec![0; TOTAL];
                let mut output = vec![0; TOTAL];
                context.read_allocation(upload, 0, &mut input)?;
                context.read_allocation(download, 0, &mut output)?;
                for i in 0..TOTAL {
                    let expected_input = ((i * 29 + i / 257) % 251) as u8;
                    let expected_output = if (PAD..PAD + BODY).contains(&i) {
                        expected_input
                    } else {
                        0xa7
                    };
                    if input[i] != expected_input || output[i] != expected_output {
                        return Err(format!("canary mismatch at byte {i}").into());
                    }
                }
                Ok(())
            })?,
    )???;
    let shutdown = engine.shutdown()?;
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(format!("owner did not release all custody: {shutdown:?}").into());
    }
    if tracked {
        println!(
            "PASS schema=fe2o3.runtime.r62-async-control-copy.v1 bytes={TOTAL} owner_threads=1 cancelled_copy=not_submitted timeout_identity=retained abandoned_upload=completed canaries=complete cleanup=complete"
        );
    } else {
        println!(
            "PASS schema=fe2o3.runtime.r61-async-owner-copy.v1 bytes={TOTAL} owner_threads=1 abandoned_upload=completed canaries=complete cleanup=complete"
        );
    }
    Ok(())
}

pub fn main_for_profile(tracked: bool) -> ResultV1<()> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 2 {
        return Err("usage: owner-copy-qualifier <unique-id>".into());
    }
    let value = arguments[1]
        .strip_prefix("0x")
        .ok_or("unique-id must use 0x hexadecimal")?;
    let unique_id = u64::from_str_radix(value, 16)?;
    if unique_id == 0 {
        return Err("unique-id must be nonzero".into());
    }
    run(unique_id, tracked)
}
