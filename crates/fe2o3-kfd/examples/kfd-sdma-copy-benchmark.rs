//! Steady-state gfx942 SDMA and mapped-allocation pool benchmark.

use std::time::{Duration, Instant};

use fe2o3_kfd::{
    ComputeAqlQueueSessionV1, DeviceSelector, Gfx942SdmaBufferV1, Gfx942SdmaCopyRequestV1,
    OpenedKfd,
};

struct Buffers {
    upload: Gfx942SdmaBufferV1,
    device: Gfx942SdmaBufferV1,
    download: Gfx942SdmaBufferV1,
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

fn run_round(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: Vec<Buffers>,
    copy_bytes: usize,
) -> Result<(Vec<Buffers>, u128, u128), Box<dyn std::error::Error>> {
    let mut requests = Vec::with_capacity(buffers.len());
    let mut download_buffers = Vec::with_capacity(buffers.len());
    for buffer in buffers {
        requests.push(Gfx942SdmaCopyRequestV1::new(
            buffer.upload,
            0,
            buffer.device,
            0,
            copy_bytes as u32,
        ));
        download_buffers.push(buffer.download);
    }
    let start = Instant::now();
    let tickets = queue
        .submit_sdma_copy_batch(requests)
        .map_err(|failure| failure.into_parts().0)?;
    let completed = queue.wait_sdma_copy_batch_for(&tickets, Duration::from_secs(30))?;
    let h2d_ns = start.elapsed().as_nanos();
    let mut uploaded = Vec::with_capacity(completed.len());
    for (completed, download) in completed.into_iter().zip(download_buffers) {
        let (upload, device) = completed.into_buffers();
        uploaded.push((upload, device, download));
    }
    let mut requests = Vec::with_capacity(uploaded.len());
    let mut upload_buffers = Vec::with_capacity(uploaded.len());
    for (upload, device, download) in uploaded {
        requests.push(Gfx942SdmaCopyRequestV1::new(
            device,
            0,
            download,
            0,
            copy_bytes as u32,
        ));
        upload_buffers.push(upload);
    }
    let start = Instant::now();
    let tickets = queue
        .submit_sdma_copy_batch(requests)
        .map_err(|failure| failure.into_parts().0)?;
    let downloads = queue.wait_sdma_copy_batch_for(&tickets, Duration::from_secs(30))?;
    let d2h_ns = start.elapsed().as_nanos();
    let mut completed_buffers = Vec::with_capacity(downloads.len());
    for (completed, upload) in downloads.into_iter().zip(upload_buffers) {
        let (device, download) = completed.into_buffers();
        completed_buffers.push(Buffers {
            upload,
            device,
            download,
        });
    }
    Ok((completed_buffers, h2d_ns, d2h_ns))
}

fn round_pattern(round: usize, slot: usize) -> u8 {
    (round
        .wrapping_mul(67)
        .wrapping_add(slot.wrapping_mul(29))
        .wrapping_add(1)
        % 251
        + 1) as u8
}

fn prepare_and_poison(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: &mut [Buffers],
    copy_bytes: usize,
    round: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    for (slot, buffer) in buffers.iter_mut().enumerate() {
        let value = round_pattern(round, slot);
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
) -> Result<(), Box<dyn std::error::Error>> {
    for (slot, buffer) in buffers.iter().enumerate() {
        let expected = round_pattern(round, slot);
        let observed = queue.read_sdma_host_buffer(&buffer.download, 0, copy_bytes as u64)?;
        if observed.iter().any(|byte| *byte != expected) {
            return Err(format!("SDMA copy mismatch at round {round}, slot {slot}").into());
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "usage: kfd-sdma-copy-benchmark <unique-id> <bytes> <depth> <warmups> <samples>".into(),
        );
    }
    let unique_id = if let Some(hex) = args[0].strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        args[0].parse()?
    };
    let copy_bytes: usize = args[1].parse()?;
    let depth: usize = args[2].parse()?;
    let warmups: usize = args[3].parse()?;
    let samples: usize = args[4].parse()?;
    if copy_bytes == 0 || copy_bytes > fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize {
        return Err("copy size is outside one gfx942 linear-copy packet".into());
    }
    if depth == 0 || depth > fe2o3_kfd::GFX942_SDMA_MAX_IN_FLIGHT_V1 || samples == 0 {
        return Err("depth or sample count is out of range".into());
    }

    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = device.create_compute_aql_queue(4096)?;
    queue.enable_sdma_copy_engine()?;
    let mut buffers = Vec::with_capacity(depth);
    for _ in 0..depth {
        buffers.push(Buffers {
            upload: queue.allocate_sdma_pooled_host_buffer(copy_bytes)?,
            device: queue.allocate_sdma_pooled_device_buffer(copy_bytes as u64, 4096)?,
            download: queue.allocate_sdma_pooled_host_buffer(copy_bytes)?,
        });
    }
    let rounds = warmups
        .checked_add(samples)
        .ok_or("warmup and sample count overflow")?;
    let mut h2d = Vec::with_capacity(samples);
    let mut d2h = Vec::with_capacity(samples);
    for round in 0..rounds {
        prepare_and_poison(&mut queue, &mut buffers, copy_bytes, round)?;
        let (next, h2d_ns, d2h_ns) = run_round(&mut queue, buffers, copy_bytes)?;
        buffers = next;
        validate_round(&mut queue, &buffers, copy_bytes, round)?;
        if round >= warmups {
            h2d.push(h2d_ns);
            d2h.push(d2h_ns);
        }
    }
    for buffer in buffers {
        queue.recycle_sdma_buffer(buffer.upload)?;
        queue.recycle_sdma_buffer(buffer.device)?;
        queue.recycle_sdma_buffer(buffer.download)?;
    }

    let pool_iterations = 10_000_usize;
    let pool_start = Instant::now();
    for _ in 0..pool_iterations {
        let host = queue.allocate_sdma_pooled_host_buffer(copy_bytes)?;
        let device = queue.allocate_sdma_pooled_device_buffer(copy_bytes as u64, 4096)?;
        queue.recycle_sdma_buffer(host)?;
        queue.recycle_sdma_buffer(device)?;
    }
    let pool_ns_per_pair = pool_start.elapsed().as_nanos() / pool_iterations as u128;
    let transferred = copy_bytes * depth;
    let h2d_p50 = percentile(&h2d, 1, 2);
    let h2d_p95 = percentile(&h2d, 19, 20);
    let d2h_p50 = percentile(&d2h, 1, 2);
    let d2h_p95 = percentile(&d2h, 19, 20);
    let pool = queue.sdma_memory_pool_observation()?;
    let trimmed = queue.trim_sdma_memory_pool()?;
    queue.destroy()?;
    println!(
        "backend=kfd schema=fe2o3.async-copy-benchmark.v1 unique_id={unique_id:016x} bytes={copy_bytes} depth={depth} warmups={warmups} samples={samples} h2d_p50_ns={h2d_p50} h2d_p95_ns={h2d_p95} h2d_p50_GBps={:.3} d2h_p50_ns={d2h_p50} d2h_p95_ns={d2h_p95} d2h_p50_GBps={:.3} pool_checkout_recycle_pair_ns={pool_ns_per_pair} pool_reuse_count={} pool_trimmed={trimmed}",
        gbps(transferred, h2d_p50),
        gbps(transferred, d2h_p50),
        pool.reuse_count,
    );
    Ok(())
}
