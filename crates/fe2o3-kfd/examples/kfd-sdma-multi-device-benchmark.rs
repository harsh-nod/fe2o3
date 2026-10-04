//! Concurrent two-device gfx942 SDMA copy benchmark.

use std::time::{Duration, Instant};

use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, ComputeAqlQueueSessionV1, DeviceSelector, Gfx942SdmaBufferV1,
    Gfx942SdmaCopyRequestV1, OpenedKfd,
};

struct Buffers {
    upload: Gfx942SdmaBufferV1,
    device: Gfx942SdmaBufferV1,
    download: Gfx942SdmaBufferV1,
}

struct RoundResult {
    left: Vec<Buffers>,
    right: Vec<Buffers>,
    h2d_ns: u128,
    d2h_ns: u128,
}

fn parse_unique_id(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        value.parse()?
    })
}

fn admit_device(
    unique_id: u64,
) -> Result<CheckedGfx942XnackMinusDevice, Box<dyn std::error::Error>> {
    Ok(OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?)
}

fn create_queue(
    device: CheckedGfx942XnackMinusDevice,
) -> Result<ComputeAqlQueueSessionV1, Box<dyn std::error::Error>> {
    let mut queue = device.create_compute_aql_queue(4096)?;
    queue.enable_gfx942_directional_sdma_copy_engines()?;
    Ok(queue)
}

fn percentile(samples: &[u128], numerator: usize, denominator: usize) -> u128 {
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let rank = ordered
        .len()
        .checked_mul(numerator)
        .and_then(|value| value.checked_add(denominator - 1))
        .expect("bounded percentile rank")
        / denominator;
    ordered[rank.saturating_sub(1)]
}

fn gbps(bytes: usize, nanoseconds: u128) -> f64 {
    bytes as f64 / nanoseconds as f64
}

fn prepare_h2d(
    buffers: Vec<Buffers>,
    copy_bytes: u32,
) -> (Vec<Gfx942SdmaCopyRequestV1>, Vec<Gfx942SdmaBufferV1>) {
    let mut requests = Vec::with_capacity(buffers.len());
    let mut downloads = Vec::with_capacity(buffers.len());
    for buffer in buffers {
        requests.push(Gfx942SdmaCopyRequestV1::new(
            buffer.upload,
            0,
            buffer.device,
            0,
            copy_bytes,
        ));
        downloads.push(buffer.download);
    }
    (requests, downloads)
}

fn finish_h2d(
    completed: Vec<fe2o3_kfd::Gfx942SdmaCompletedCopyV1>,
    downloads: Vec<Gfx942SdmaBufferV1>,
) -> Vec<Buffers> {
    completed
        .into_iter()
        .zip(downloads)
        .map(|(completed, download)| {
            let (upload, device) = completed.into_buffers();
            Buffers {
                upload,
                device,
                download,
            }
        })
        .collect()
}

fn prepare_d2h(
    buffers: Vec<Buffers>,
    copy_bytes: u32,
) -> (Vec<Gfx942SdmaCopyRequestV1>, Vec<Gfx942SdmaBufferV1>) {
    let mut requests = Vec::with_capacity(buffers.len());
    let mut uploads = Vec::with_capacity(buffers.len());
    for buffer in buffers {
        requests.push(Gfx942SdmaCopyRequestV1::new(
            buffer.device,
            0,
            buffer.download,
            0,
            copy_bytes,
        ));
        uploads.push(buffer.upload);
    }
    (requests, uploads)
}

fn finish_d2h(
    completed: Vec<fe2o3_kfd::Gfx942SdmaCompletedCopyV1>,
    uploads: Vec<Gfx942SdmaBufferV1>,
) -> Vec<Buffers> {
    completed
        .into_iter()
        .zip(uploads)
        .map(|(completed, upload)| {
            let (device, download) = completed.into_buffers();
            Buffers {
                upload,
                device,
                download,
            }
        })
        .collect()
}

fn run_round(
    left: &mut ComputeAqlQueueSessionV1,
    right: &mut ComputeAqlQueueSessionV1,
    left_buffers: Vec<Buffers>,
    right_buffers: Vec<Buffers>,
    copy_bytes: u32,
) -> Result<RoundResult, Box<dyn std::error::Error>> {
    let (left_requests, left_downloads) = prepare_h2d(left_buffers, copy_bytes);
    let (right_requests, right_downloads) = prepare_h2d(right_buffers, copy_bytes);
    let start = Instant::now();
    let left_tickets = left
        .submit_sdma_copy_batch(left_requests)
        .map_err(|failure| failure.into_parts().0)?;
    let right_tickets = right
        .submit_sdma_copy_batch(right_requests)
        .map_err(|failure| failure.into_parts().0)?;
    let right_completed =
        right.wait_sdma_copy_batch_for(&right_tickets, Duration::from_secs(30))?;
    let left_completed = left.wait_sdma_copy_batch_for(&left_tickets, Duration::from_secs(30))?;
    let h2d_ns = start.elapsed().as_nanos();
    let left_buffers = finish_h2d(left_completed, left_downloads);
    let right_buffers = finish_h2d(right_completed, right_downloads);

    let (left_requests, left_uploads) = prepare_d2h(left_buffers, copy_bytes);
    let (right_requests, right_uploads) = prepare_d2h(right_buffers, copy_bytes);
    let start = Instant::now();
    let left_tickets = left
        .submit_sdma_copy_batch(left_requests)
        .map_err(|failure| failure.into_parts().0)?;
    let right_tickets = right
        .submit_sdma_copy_batch(right_requests)
        .map_err(|failure| failure.into_parts().0)?;
    let right_completed =
        right.wait_sdma_copy_batch_for(&right_tickets, Duration::from_secs(30))?;
    let left_completed = left.wait_sdma_copy_batch_for(&left_tickets, Duration::from_secs(30))?;
    let d2h_ns = start.elapsed().as_nanos();
    Ok(RoundResult {
        left: finish_d2h(left_completed, left_uploads),
        right: finish_d2h(right_completed, right_uploads),
        h2d_ns,
        d2h_ns,
    })
}

fn allocate_buffers(
    queue: &mut ComputeAqlQueueSessionV1,
    copy_bytes: usize,
    depth: usize,
) -> Result<Vec<Buffers>, Box<dyn std::error::Error>> {
    let mut buffers = Vec::with_capacity(depth);
    for _ in 0..depth {
        buffers.push(Buffers {
            upload: queue.allocate_sdma_pooled_host_buffer(copy_bytes)?,
            device: queue.allocate_sdma_pooled_device_buffer(copy_bytes as u64, 4096)?,
            download: queue.allocate_sdma_pooled_host_buffer(copy_bytes)?,
        });
    }
    Ok(buffers)
}

fn round_pattern(round: usize, slot: usize, device_tag: u8) -> u8 {
    (round
        .wrapping_mul(67)
        .wrapping_add(slot.wrapping_mul(29))
        .wrapping_add(usize::from(device_tag))
        % 251
        + 1) as u8
}

fn prepare_and_poison(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: &mut [Buffers],
    copy_bytes: usize,
    round: usize,
    device_tag: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    for (slot, buffer) in buffers.iter_mut().enumerate() {
        let value = round_pattern(round, slot, device_tag);
        queue.write_sdma_host_buffer(&mut buffer.upload, 0, &vec![value; copy_bytes])?;
        queue.write_sdma_host_buffer(&mut buffer.download, 0, &vec![value ^ 0xff; copy_bytes])?;
    }
    Ok(())
}

fn validate_round(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: &[Buffers],
    copy_bytes: usize,
    round: usize,
    device_tag: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    for (slot, buffer) in buffers.iter().enumerate() {
        let expected = round_pattern(round, slot, device_tag);
        let observed = queue.read_sdma_host_buffer(&buffer.download, 0, copy_bytes as u64)?;
        if observed.iter().any(|byte| *byte != expected) {
            return Err(format!(
                "multi-device SDMA copy mismatch at device tag {device_tag}, round {round}, slot {slot}"
            )
            .into());
        }
    }
    Ok(())
}

fn recycle_all(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: Vec<Buffers>,
) -> Result<(), Box<dyn std::error::Error>> {
    for buffer in buffers {
        queue.recycle_sdma_buffer(buffer.upload)?;
        queue.recycle_sdma_buffer(buffer.device)?;
        queue.recycle_sdma_buffer(buffer.download)?;
    }
    queue.trim_sdma_memory_pool()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("usage: kfd-sdma-multi-device-benchmark <unique-id-0> <unique-id-1> <bytes> <depth-per-device> <warmups> <samples>".into());
    }
    let ids = [parse_unique_id(&args[0])?, parse_unique_id(&args[1])?];
    let copy_bytes: usize = args[2].parse()?;
    let depth: usize = args[3].parse()?;
    let warmups: usize = args[4].parse()?;
    let samples: usize = args[5].parse()?;
    if ids[0] == ids[1]
        || copy_bytes == 0
        || copy_bytes > fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize
        || depth == 0
        || depth > fe2o3_kfd::GFX942_SDMA_MAX_IN_FLIGHT_V1
        || samples == 0
    {
        return Err("multi-device benchmark controls are out of range".into());
    }

    // SET_XNACK_MODE is process-wide and must run before either queue exists.
    let left_device = admit_device(ids[0])?;
    let right_device = admit_device(ids[1])?;
    let mut left = create_queue(left_device)?;
    let mut right = create_queue(right_device)?;
    let mut left_buffers = allocate_buffers(&mut left, copy_bytes, depth)?;
    let mut right_buffers = allocate_buffers(&mut right, copy_bytes, depth)?;
    let rounds = warmups
        .checked_add(samples)
        .ok_or("warmup and sample count overflow")?;
    let mut h2d = Vec::with_capacity(samples);
    let mut d2h = Vec::with_capacity(samples);
    for round_index in 0..rounds {
        prepare_and_poison(&mut left, &mut left_buffers, copy_bytes, round_index, 0x35)?;
        prepare_and_poison(
            &mut right,
            &mut right_buffers,
            copy_bytes,
            round_index,
            0xca,
        )?;
        let result = run_round(
            &mut left,
            &mut right,
            left_buffers,
            right_buffers,
            copy_bytes as u32,
        )?;
        left_buffers = result.left;
        right_buffers = result.right;
        validate_round(&mut left, &left_buffers, copy_bytes, round_index, 0x35)?;
        validate_round(&mut right, &right_buffers, copy_bytes, round_index, 0xca)?;
        if round_index >= warmups {
            h2d.push(result.h2d_ns);
            d2h.push(result.d2h_ns);
        }
    }
    recycle_all(&mut right, right_buffers)?;
    recycle_all(&mut left, left_buffers)?;
    let right_destroyed = right.destroy()?;
    let left_destroyed = left.destroy()?;
    assert_eq!(right_destroyed.released_resources(), 11);
    assert_eq!(left_destroyed.released_resources(), 11);

    let transferred = copy_bytes * depth * 2;
    let h2d_p50 = percentile(&h2d, 1, 2);
    let h2d_p95 = percentile(&h2d, 19, 20);
    let d2h_p50 = percentile(&d2h, 1, 2);
    let d2h_p95 = percentile(&d2h, 19, 20);
    println!(
        "backend=kfd schema=fe2o3.async-copy-multi-device-benchmark.v1 devices=2 unique_ids={:016x},{:016x} bytes={} depth_per_device={} queue_depth_per_device={} batch_size_per_device={} direction=h2d-then-d2h concurrency=2 doorbells_per_device_batch=1 warmups={} samples={} h2d_p50_ns={} h2d_p95_ns={} h2d_aggregate_p50_GBps={:.3} d2h_p50_ns={} d2h_p95_ns={} d2h_aggregate_p50_GBps={:.3}",
        ids[0],
        ids[1],
        copy_bytes,
        depth,
        depth,
        depth,
        warmups,
        samples,
        h2d_p50,
        h2d_p95,
        gbps(transferred, h2d_p50),
        d2h_p50,
        d2h_p95,
        gbps(transferred, d2h_p50),
    );
    Ok(())
}
