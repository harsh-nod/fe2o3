#![cfg(all(
    feature = "hardware-qualification",
    target_os = "linux",
    target_arch = "x86_64"
))]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_runtime::{
    KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1, RuntimeAccessV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncEngineV1, RuntimeAsyncProgressConfigV1, RuntimeBinaryCodecV5,
    RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
    RuntimePollV1, RuntimeWorkerBackendV5, RuntimeWorkerCommandV1,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(120);
const IO_CHUNK_BYTES: usize = 16 * 1024 * 1024;

fn fill_offset_pattern_v1(bytes: &mut [u8], byte_offset: u64) {
    assert_eq!(byte_offset % 8, 0);
    assert_eq!(bytes.len() % 8, 0);
    for (index, word) in bytes.chunks_exact_mut(8).enumerate() {
        let mut value = (byte_offset / 8)
            .checked_add(u64::try_from(index).unwrap())
            .unwrap()
            .wrapping_add(0x9e37_79b9_7f4a_7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        word.copy_from_slice(&(value ^ (value >> 31)).to_le_bytes());
    }
}

struct ThreadWakeV1(thread::Thread);

impl Wake for ThreadWakeV1 {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on_with_timeout_v1<F: Future + Unpin>(future: &mut F, timeout: Duration) -> F::Output {
    let deadline = Instant::now()
        .checked_add(timeout)
        .expect("fixed qualification timeout is representable");
    let waker = Waker::from(Arc::new(ThreadWakeV1(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(result) = Pin::new(&mut *future).poll(&mut context) {
            return result;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "Worker V5 D2D completion timed out");
        thread::park_timeout(remaining.min(Duration::from_millis(10)));
    }
}

fn region(
    allocation: fe2o3_runtime::RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
    }
}

#[test]
#[ignore = "requires an exclusively owned MI300X/gfx942 KFD device"]
fn worker_v5_kfd_composite_progresses_sixty_three_plus_two_d2d_without_manual_flush() {
    let packet_bytes = u64::from(fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let max_window_packets =
        fe2o3_kfd::GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1 as u64;
    let copy_bytes = KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1;
    let packet_count = copy_bytes.div_ceil(packet_bytes);
    assert_eq!(max_window_packets, 63);
    assert_eq!(packet_count, 65);
    assert_eq!(packet_count - max_window_packets, 2);
    assert_eq!(packet_count.div_ceil(max_window_packets), 2);
    assert_eq!(copy_bytes % packet_bytes, 2_048);
    assert_eq!(
        copy_bytes - max_window_packets * packet_bytes,
        packet_bytes + 2_048
    );

    let unique_id = fe2o3_kfd::topology::discover_default_topology()
        .expect("discover KFD topology")
        .topology()
        .gpu_nodes()
        .iter()
        .filter(|node| node.target().name() == "gfx942")
        .filter(|node| node.capacity().wavefront_size() == 64)
        .map(|node| node.unique_id())
        .filter(|unique_id| *unique_id != 0)
        .min()
        .expect("qualification requires one nonzero gfx942 Wave64 device ID");
    let command = RuntimeWorkerCommandV1::new(env!(
        "CARGO_BIN_EXE_fe2o3-runtime-kfd-worker-v5-qualification"
    ))
    .argument(unique_id.to_string());
    let backend = RuntimeWorkerBackendV5::spawn(
        &command,
        RuntimeBinaryCodecV5,
        Duration::from_secs(30),
        REQUEST_TIMEOUT,
    )
    .expect("spawn exact KFD Worker V5 qualification child");
    let mut context = RuntimeContextV1::open(backend).expect("open Worker V5 context");
    assert_eq!(context.devices().len(), 1);
    assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
    let device = context.devices()[0].id();
    let capabilities = context
        .execution_capabilities(device)
        .expect("query Worker V5 execution capabilities");
    assert!(capabilities.native_async_copy);
    assert!(capabilities.memory_pool);

    let stream = context.create_stream(device).expect("create D2D stream");
    let source = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, copy_bytes, 4096)
        .expect("allocate D2D source");
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, copy_bytes, 4096)
        .expect("allocate D2D destination");
    let mut source_chunk = vec![0_u8; IO_CHUNK_BYTES];
    for offset in (0..copy_bytes).step_by(IO_CHUNK_BYTES) {
        let byte_len = usize::try_from((copy_bytes - offset).min(IO_CHUNK_BYTES as u64)).unwrap();
        fill_offset_pattern_v1(&mut source_chunk[..byte_len], offset);
        context
            .write_allocation(source, offset, &source_chunk[..byte_len])
            .expect("initialize D2D source chunk");
    }

    let mut submission = context
        .copy_async(
            stream,
            region(source, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[],
        )
        .expect("submit 63+2 D2D copy");
    let event = context
        .record_event(&submission)
        .expect("record D2D completion event");
    let (engine, progress) = RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .expect("spawn Worker V5 progress engine");
    let mut completion = progress
        .event_future_with_progress(stream, event)
        .expect("atomically register D2D event and stream progress");
    assert_eq!(
        block_on_with_timeout_v1(&mut completion, COMPLETION_TIMEOUT)
            .expect("observe D2D completion"),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(completion.progress_failure_count(), 0);
    assert!(completion.take_progress_failure().is_none());
    drop(completion);
    drop(progress);

    let mut context = engine.into_context().expect("recover Worker V5 context");
    assert_eq!(
        context.poll(&mut submission).expect("reobserve D2D result"),
        RuntimePollV1::Succeeded
    );
    let mut source_observed = vec![0_u8; IO_CHUNK_BYTES];
    let mut destination_observed = vec![0_u8; IO_CHUNK_BYTES];
    for offset in (0..copy_bytes).step_by(IO_CHUNK_BYTES) {
        let byte_len = usize::try_from((copy_bytes - offset).min(IO_CHUNK_BYTES as u64)).unwrap();
        fill_offset_pattern_v1(&mut source_chunk[..byte_len], offset);
        context
            .read_allocation(source, offset, &mut source_observed[..byte_len])
            .expect("read D2D source chunk after copy");
        if let Some(index) = source_observed[..byte_len]
            .iter()
            .zip(&source_chunk[..byte_len])
            .position(|(actual, expected)| actual != expected)
        {
            panic!(
                "D2D source differs from the offset pattern at byte {}",
                offset + u64::try_from(index).unwrap()
            );
        }
        context
            .read_allocation(destination, offset, &mut destination_observed[..byte_len])
            .expect("read D2D destination chunk");
        if let Some(index) = destination_observed[..byte_len]
            .iter()
            .zip(&source_chunk[..byte_len])
            .position(|(actual, expected)| actual != expected)
        {
            panic!(
                "D2D result differs from the offset pattern at byte {}",
                offset + u64::try_from(index).unwrap()
            );
        }
    }

    context.release_event(event).expect("release D2D event");
    context
        .release_submission(submission)
        .expect("release D2D submission");
    context
        .release_allocation(destination)
        .expect("release D2D destination");
    context
        .release_allocation(source)
        .expect("release D2D source");
    context.destroy_stream(stream).expect("destroy D2D stream");
    let worker = context
        .shutdown()
        .expect("complete host-side logical cleanup");
    worker
        .shutdown(Duration::from_secs(30))
        .expect("send the clean shutdown frame and observe explicit child native shutdown");
}
