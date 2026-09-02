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

#[derive(Clone, Copy)]
struct PhaseTiming {
    total_ns: u128,
    submit_ns: u128,
    wait_ns: u128,
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
) -> Result<(Vec<Buffers>, PhaseTiming, PhaseTiming), Box<dyn std::error::Error>> {
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
    let submitted = Instant::now();
    let completed = queue.wait_sdma_copy_batch_for(&tickets, Duration::from_secs(30))?;
    let finished = Instant::now();
    let h2d_timing = PhaseTiming {
        total_ns: finished.duration_since(start).as_nanos(),
        submit_ns: submitted.duration_since(start).as_nanos(),
        wait_ns: finished.duration_since(submitted).as_nanos(),
    };
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
    let submitted = Instant::now();
    let downloads = queue.wait_sdma_copy_batch_for(&tickets, Duration::from_secs(30))?;
    let finished = Instant::now();
    let d2h_timing = PhaseTiming {
        total_ns: finished.duration_since(start).as_nanos(),
        submit_ns: submitted.duration_since(start).as_nanos(),
        wait_ns: finished.duration_since(submitted).as_nanos(),
    };
    let mut completed_buffers = Vec::with_capacity(downloads.len());
    for (completed, upload) in downloads.into_iter().zip(upload_buffers) {
        let (device, download) = completed.into_buffers();
        completed_buffers.push(Buffers {
            upload,
            device,
            download,
        });
    }
    Ok((completed_buffers, h2d_timing, d2h_timing))
}

fn run_round_combined(
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
    let completed = queue
        .execute_sdma_copy_batch_for(requests, Duration::from_secs(30))
        .map_err(|failure| failure.into_parts().0)?;
    let h2d_ns = start.elapsed().as_nanos();
    let mut uploaded = Vec::with_capacity(completed.len());
    for (completed, download) in completed.into_iter().zip(download_buffers) {
        let (upload, device) = completed.into_buffers();
        uploaded.push(Buffers {
            upload,
            device,
            download,
        });
    }

    let mut requests = Vec::with_capacity(uploaded.len());
    let mut upload_buffers = Vec::with_capacity(uploaded.len());
    for buffer in uploaded {
        requests.push(Gfx942SdmaCopyRequestV1::new(
            buffer.device,
            0,
            buffer.download,
            0,
            copy_bytes as u32,
        ));
        upload_buffers.push(buffer.upload);
    }
    let start = Instant::now();
    let downloads = queue
        .execute_sdma_copy_batch_for(requests, Duration::from_secs(30))
        .map_err(|failure| failure.into_parts().0)?;
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
    if !(5..=6).contains(&args.len()) {
        return Err(
            "usage: kfd-sdma-copy-benchmark <unique-id> <bytes> <depth> <warmups> <samples> [generic|directional|engine0|engine1|striped2|striped4|striped8|striped16]".into(),
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

    let profile = args.get(5).map_or("directional", String::as_str);
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = device.create_compute_aql_queue(4096)?;
    let (h2d_engine_index, d2h_engine_index) = match profile {
        "generic" => {
            let queue = queue.enable_sdma_copy_engine()?;
            (queue.engine_index, queue.engine_index)
        }
        "directional" => {
            let queues = queue.enable_gfx942_directional_sdma_copy_engines()?;
            (
                queues.host_to_device.engine_index,
                queues.device_to_host.engine_index,
            )
        }
        "engine0" => {
            let queue = queue.enable_gfx942_sdma_copy_engine_on_engine_index(0)?;
            (queue.engine_index, queue.engine_index)
        }
        "engine1" => {
            let queue = queue.enable_gfx942_sdma_copy_engine_on_engine_index(1)?;
            (queue.engine_index, queue.engine_index)
        }
        profile if profile.starts_with("striped") => {
            let queue_count: u32 = profile["striped".len()..].parse()?;
            let queues = queue.enable_gfx942_striped_sdma_copy_engines(queue_count)?;
            if queues.len() != queue_count as usize {
                return Err("striped queue observation count mismatch".into());
            }
            (None, None)
        }
        _ => return Err("unknown SDMA queue profile".into()),
    };
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
    let mut h2d_submit = Vec::with_capacity(samples);
    let mut h2d_wait = Vec::with_capacity(samples);
    let mut d2h_submit = Vec::with_capacity(samples);
    let mut d2h_wait = Vec::with_capacity(samples);
    for round in 0..rounds {
        prepare_and_poison(&mut queue, &mut buffers, copy_bytes, round)?;
        let (next, h2d_timing, d2h_timing) = run_round(&mut queue, buffers, copy_bytes)?;
        buffers = next;
        validate_round(&mut queue, &buffers, copy_bytes, round)?;
        if round >= warmups {
            h2d.push(h2d_timing.total_ns);
            h2d_submit.push(h2d_timing.submit_ns);
            h2d_wait.push(h2d_timing.wait_ns);
            d2h.push(d2h_timing.total_ns);
            d2h_submit.push(d2h_timing.submit_ns);
            d2h_wait.push(d2h_timing.wait_ns);
        }
    }
    let mut combined_h2d = Vec::with_capacity(samples);
    let mut combined_d2h = Vec::with_capacity(samples);
    for round in 0..rounds {
        let pattern_round = rounds
            .checked_add(round)
            .ok_or("combined round index overflow")?;
        prepare_and_poison(&mut queue, &mut buffers, copy_bytes, pattern_round)?;
        let (next, h2d_ns, d2h_ns) = run_round_combined(&mut queue, buffers, copy_bytes)?;
        buffers = next;
        validate_round(&mut queue, &buffers, copy_bytes, pattern_round)?;
        if round >= warmups {
            combined_h2d.push(h2d_ns);
            combined_d2h.push(d2h_ns);
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
    let h2d_submit_p50 = percentile(&h2d_submit, 1, 2);
    let h2d_wait_p50 = percentile(&h2d_wait, 1, 2);
    let d2h_submit_p50 = percentile(&d2h_submit, 1, 2);
    let d2h_wait_p50 = percentile(&d2h_wait, 1, 2);
    let combined_h2d_p50 = percentile(&combined_h2d, 1, 2);
    let combined_d2h_p50 = percentile(&combined_d2h, 1, 2);
    let pool = queue.sdma_memory_pool_observation()?;
    let trimmed = queue.trim_sdma_memory_pool()?;
    queue.destroy()?;
    println!(
        "backend=kfd schema=fe2o3.async-copy-benchmark.v1 unique_id={unique_id:016x} profile={profile} bytes={copy_bytes} depth={depth} queue_depth={depth} batch_size={depth} direction=h2d-then-d2h concurrency=1 doorbells_per_batch=1 warmups={warmups} samples={samples} h2d_engine_index={} d2h_engine_index={} h2d_p50_ns={h2d_p50} h2d_p95_ns={h2d_p95} h2d_submit_p50_ns={h2d_submit_p50} h2d_wait_p50_ns={h2d_wait_p50} h2d_p50_GBps={:.3} d2h_p50_ns={d2h_p50} d2h_p95_ns={d2h_p95} d2h_submit_p50_ns={d2h_submit_p50} d2h_wait_p50_ns={d2h_wait_p50} d2h_p50_GBps={:.3} combined_h2d_p50_ns={combined_h2d_p50} combined_h2d_p50_GBps={:.3} combined_d2h_p50_ns={combined_d2h_p50} combined_d2h_p50_GBps={:.3} pool_checkout_recycle_pair_ns={pool_ns_per_pair} pool_reuse_count={} pool_trimmed={trimmed}",
        h2d_engine_index.unwrap_or(u32::MAX),
        d2h_engine_index.unwrap_or(u32::MAX),
        gbps(transferred, h2d_p50),
        gbps(transferred, d2h_p50),
        gbps(transferred, combined_h2d_p50),
        gbps(transferred, combined_d2h_p50),
        pool.reuse_count,
    );
    Ok(())
}
