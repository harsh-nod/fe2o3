//! Isolated gfx942 SDMA copy, concurrency, and memory-pool validation.

use std::time::Duration;

use fe2o3_kfd::{
    DeviceSelector, GFX942_SDMA_MAX_IN_FLIGHT_V1, Gfx942SdmaCompletedCopyV1, OpenedKfd,
};

const COPY_BYTES: usize = 1024 * 1024;
const ASYNC_DEPTH: usize = 16;

fn parse_unique_id() -> Result<u64, Box<dyn std::error::Error>> {
    let value = std::env::args()
        .nth(1)
        .ok_or("usage: kfd-sdma-copy <selected-unique-id>")?;
    Ok(if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        value.parse()?
    })
}

fn verify_and_recycle(
    queue: &mut fe2o3_kfd::ComputeAqlQueueSessionV1,
    completed: Gfx942SdmaCompletedCopyV1,
    expected: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let (source, destination) = completed.into_buffers();
    let observed = queue.read_sdma_host_buffer(&destination, 0, COPY_BYTES as u64)?;
    if observed.iter().any(|byte| *byte != expected) {
        return Err("SDMA host-visible copy mismatch".into());
    }
    queue.recycle_sdma_buffer(source)?;
    queue.recycle_sdma_buffer(destination)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let unique_id = parse_unique_id()?;
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = device.create_compute_aql_queue(4096)?;
    let sdma = queue.enable_sdma_copy_engine()?;
    assert_eq!(
        usize::from(sdma.maximum_in_flight),
        GFX942_SDMA_MAX_IN_FLIGHT_V1
    );

    let mut tickets = Vec::with_capacity(ASYNC_DEPTH);
    for index in 0..ASYNC_DEPTH {
        let expected = (index as u8).wrapping_add(1);
        let mut source = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES)?;
        let destination = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES)?;
        queue.write_sdma_host_buffer(&mut source, 0, &vec![expected; COPY_BYTES])?;
        let ticket = queue
            .submit_sdma_copy(source, 0, destination, 0, COPY_BYTES as u32)
            .map_err(|failure| failure.into_parts().0)?;
        tickets.push((ticket, expected));
    }
    for (ticket, expected) in tickets {
        let completed = queue.wait_sdma_copy_for(ticket, Duration::from_secs(10))?;
        verify_and_recycle(&mut queue, completed, expected)?;
    }

    let mut upload = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES)?;
    queue.write_sdma_host_buffer(&mut upload, 0, &vec![0xa5; COPY_BYTES])?;
    let device_buffer = queue.allocate_sdma_pooled_device_buffer(COPY_BYTES as u64, 4096)?;
    let upload = queue
        .submit_sdma_copy(upload, 0, device_buffer, 0, COPY_BYTES as u32)
        .map_err(|failure| failure.into_parts().0)?;
    let uploaded = queue.wait_sdma_copy_for(upload, Duration::from_secs(10))?;
    let (upload, device_buffer) = uploaded.into_buffers();
    let download = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES)?;
    let download = queue
        .submit_sdma_copy(device_buffer, 0, download, 0, COPY_BYTES as u32)
        .map_err(|failure| failure.into_parts().0)?;
    let downloaded = queue.wait_sdma_copy_for(download, Duration::from_secs(10))?;
    let (device_buffer, download) = downloaded.into_buffers();
    let observed = queue.read_sdma_host_buffer(&download, 0, COPY_BYTES as u64)?;
    assert!(observed.iter().all(|byte| *byte == 0xa5));
    queue.recycle_sdma_buffer(upload)?;
    queue.recycle_sdma_buffer(device_buffer)?;
    queue.recycle_sdma_buffer(download)?;

    let pool_before = queue.sdma_memory_pool_observation()?;
    assert_eq!(pool_before.checked_out_buffers, 0);
    assert!(pool_before.retained_free_buffers >= ASYNC_DEPTH * 2);
    let reused = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES / 2)?;
    assert_eq!(reused.requested_bytes(), (COPY_BYTES / 2) as u64);
    queue.recycle_sdma_buffer(reused)?;
    let pool_after = queue.sdma_memory_pool_observation()?;
    assert!(pool_after.reuse_count > pool_before.reuse_count);
    let trimmed = queue.trim_sdma_memory_pool()?;
    assert_eq!(trimmed, pool_after.retained_free_buffers);

    let destroyed = queue.destroy()?;
    assert_eq!(destroyed.released_resources(), 8);
    println!(
        "schema=fe2o3.kfd-sdma-hardware.v1 unique_id={unique_id:016x} queue_id={} async_depth={ASYNC_DEPTH} bytes={COPY_BYTES} host_copies={ASYNC_DEPTH} h2d=1 d2h=1 pool_reuse={} pool_trimmed={trimmed} status=pass",
        sdma.queue_id, pool_after.reuse_count,
    );
    Ok(())
}
