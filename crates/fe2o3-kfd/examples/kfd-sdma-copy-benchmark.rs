//! Steady-state gfx942 SDMA and mapped-allocation pool benchmark.

use std::fmt::Write;
use std::time::{Duration, Instant};

use fe2o3_kfd::{
    ComputeAqlQueueSessionV1, DeviceSelector, Gfx942CombinedSdmaCapacityV1, Gfx942SdmaBufferV1,
    Gfx942SdmaCopyRequestV1, Gfx942SdmaMultiQueueCompletedV1, Gfx942SdmaMultiQueuePollV1,
    Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaQueueObservationV1, OpenedKfd,
};
use sha2::{Digest, Sha256};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AggregateProfile {
    Combined(u32),
    Standalone16,
}

impl AggregateProfile {
    const fn queue_count(self) -> u32 {
        match self {
            Self::Combined(queue_count) => queue_count,
            Self::Standalone16 => 16,
        }
    }

    const fn directional_queue_count(self) -> u32 {
        match self {
            Self::Combined(_) => 2,
            Self::Standalone16 => 0,
        }
    }

    const fn workload_kind(self) -> &'static str {
        match self {
            Self::Combined(_) => "combined",
            Self::Standalone16 => "standalone",
        }
    }

    const fn directional_smoke(self) -> &'static str {
        match self {
            Self::Combined(_) => "pass",
            Self::Standalone16 => "not-applicable",
        }
    }
}

struct AggregateSamples {
    submit: Vec<u128>,
    wait: Vec<u128>,
    e2e: Vec<u128>,
}

impl AggregateSamples {
    fn with_capacity(samples: usize) -> Self {
        Self {
            submit: Vec::with_capacity(samples),
            wait: Vec::with_capacity(samples),
            e2e: Vec::with_capacity(samples),
        }
    }

    fn push(&mut self, timing: PhaseTiming) -> Result<(), Box<dyn std::error::Error>> {
        if timing.submit_ns == 0
            || timing.wait_ns == 0
            || timing.total_ns == 0
            || timing.submit_ns.checked_add(timing.wait_ns) != Some(timing.total_ns)
        {
            return Err("aggregate phase timing is non-positive or internally inconsistent".into());
        }
        self.submit.push(timing.submit_ns);
        self.wait.push(timing.wait_ns);
        self.e2e.push(timing.total_ns);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum AggregateDirection {
    HostToDevice,
    DeviceToHost,
}

struct AggregateQueueEvidence {
    queue_ids: String,
    queue_ids_sha256: String,
    engine_placement: String,
    engine_placement_sha256: String,
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
    concurrent_batches: usize,
) -> Result<(Vec<Buffers>, PhaseTiming, PhaseTiming), Box<dyn std::error::Error>> {
    let batches = partition_buffers(buffers, concurrent_batches);
    let mut pending = Vec::with_capacity(batches.len());
    let start = Instant::now();
    for batch in batches {
        let mut requests = Vec::with_capacity(batch.len());
        let mut download_buffers = Vec::with_capacity(batch.len());
        for buffer in batch {
            requests.push(Gfx942SdmaCopyRequestV1::new(
                buffer.upload,
                0,
                buffer.device,
                0,
                copy_bytes as u32,
            ));
            download_buffers.push(buffer.download);
        }
        let tickets = queue
            .submit_sdma_copy_batch(requests)
            .map_err(|failure| failure.into_parts().0)?;
        pending.push((tickets, download_buffers));
    }
    let submitted = Instant::now();
    let uploaded_count = pending.iter().map(|(tickets, _)| tickets.len()).sum();
    let mut uploaded = Vec::with_capacity(uploaded_count);
    for (tickets, download_buffers) in pending {
        let completed = queue.wait_sdma_copy_batch_for(&tickets, Duration::from_secs(30))?;
        for (completed, download) in completed.into_iter().zip(download_buffers) {
            let (upload, device) = completed.into_buffers();
            uploaded.push((upload, device, download));
        }
    }
    let finished = Instant::now();
    let h2d_timing = PhaseTiming {
        total_ns: finished.duration_since(start).as_nanos(),
        submit_ns: submitted.duration_since(start).as_nanos(),
        wait_ns: finished.duration_since(submitted).as_nanos(),
    };
    let uploaded = uploaded
        .into_iter()
        .map(|(upload, device, download)| Buffers {
            upload,
            device,
            download,
        })
        .collect();
    let batches = partition_buffers(uploaded, concurrent_batches);
    let mut pending = Vec::with_capacity(batches.len());
    let start = Instant::now();
    for batch in batches {
        let mut requests = Vec::with_capacity(batch.len());
        let mut upload_buffers = Vec::with_capacity(batch.len());
        for buffer in batch {
            requests.push(Gfx942SdmaCopyRequestV1::new(
                buffer.device,
                0,
                buffer.download,
                0,
                copy_bytes as u32,
            ));
            upload_buffers.push(buffer.upload);
        }
        let tickets = queue
            .submit_sdma_copy_batch(requests)
            .map_err(|failure| failure.into_parts().0)?;
        pending.push((tickets, upload_buffers));
    }
    let submitted = Instant::now();
    let completed_count = pending.iter().map(|(tickets, _)| tickets.len()).sum();
    let mut completed_buffers = Vec::with_capacity(completed_count);
    for (tickets, upload_buffers) in pending {
        let downloads = queue.wait_sdma_copy_batch_for(&tickets, Duration::from_secs(30))?;
        for (completed, upload) in downloads.into_iter().zip(upload_buffers) {
            let (device, download) = completed.into_buffers();
            completed_buffers.push(Buffers {
                upload,
                device,
                download,
            });
        }
    }
    let finished = Instant::now();
    let d2h_timing = PhaseTiming {
        total_ns: finished.duration_since(start).as_nanos(),
        submit_ns: submitted.duration_since(start).as_nanos(),
        wait_ns: finished.duration_since(submitted).as_nanos(),
    };
    Ok((completed_buffers, h2d_timing, d2h_timing))
}

fn partition_buffers(buffers: Vec<Buffers>, requested_batches: usize) -> Vec<Vec<Buffers>> {
    let mut input = buffers.into_iter();
    balanced_batch_lengths(input.len(), requested_batches)
        .into_iter()
        .map(|batch_len| input.by_ref().take(batch_len).collect())
        .collect()
}

fn balanced_batch_lengths(item_count: usize, requested_batches: usize) -> Vec<usize> {
    if item_count == 0 {
        return Vec::new();
    }
    let batch_count = requested_batches.max(1).min(item_count);
    let base = item_count / batch_count;
    let remainder = item_count % batch_count;
    (0..batch_count)
        .map(|index| base + usize::from(index < remainder))
        .collect()
}

fn admitted_striped_queue_count(profile: &str) -> Option<u32> {
    match profile {
        "striped2" => Some(2),
        "striped4" => Some(4),
        "striped6" => Some(6),
        "striped8" => Some(8),
        "striped10" => Some(10),
        "striped12" => Some(12),
        "striped14" => Some(14),
        "striped16" => Some(16),
        _ => None,
    }
}

fn admitted_aggregate_profile(profile: &str) -> Option<AggregateProfile> {
    match profile {
        "combined-striped2" => Some(AggregateProfile::Combined(2)),
        "combined-striped4" => Some(AggregateProfile::Combined(4)),
        "combined-striped8" => Some(AggregateProfile::Combined(8)),
        "combined-striped14" => Some(AggregateProfile::Combined(14)),
        "striped16" => Some(AggregateProfile::Standalone16),
        _ => None,
    }
}

fn allocate_buffers(
    queue: &mut ComputeAqlQueueSessionV1,
    count: usize,
    copy_bytes: usize,
) -> Result<Vec<Buffers>, Box<dyn std::error::Error>> {
    let mut buffers = Vec::with_capacity(count);
    for _ in 0..count {
        buffers.push(Buffers {
            upload: queue.allocate_sdma_pooled_host_buffer(copy_bytes)?,
            device: queue.allocate_sdma_pooled_device_buffer(copy_bytes as u64, 4096)?,
            download: queue.allocate_sdma_pooled_host_buffer(copy_bytes)?,
        });
    }
    Ok(buffers)
}

fn recycle_buffers(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: Vec<Buffers>,
) -> Result<(), Box<dyn std::error::Error>> {
    for buffer in buffers {
        queue.recycle_sdma_buffer(buffer.upload)?;
        queue.recycle_sdma_buffer(buffer.device)?;
        queue.recycle_sdma_buffer(buffer.download)?;
    }
    Ok(())
}

fn aggregate_phase_inputs(
    buffers: Vec<Buffers>,
    copy_bytes: usize,
    direction: AggregateDirection,
) -> (Vec<Gfx942SdmaCopyRequestV1>, Vec<Gfx942SdmaBufferV1>) {
    let mut requests = Vec::with_capacity(buffers.len());
    let mut retained = Vec::with_capacity(buffers.len());
    for buffer in buffers {
        match direction {
            AggregateDirection::HostToDevice => {
                requests.push(Gfx942SdmaCopyRequestV1::new(
                    buffer.upload,
                    0,
                    buffer.device,
                    0,
                    copy_bytes as u32,
                ));
                retained.push(buffer.download);
            }
            AggregateDirection::DeviceToHost => {
                requests.push(Gfx942SdmaCopyRequestV1::new(
                    buffer.device,
                    0,
                    buffer.download,
                    0,
                    copy_bytes as u32,
                ));
                retained.push(buffer.upload);
            }
        }
    }
    (requests, retained)
}

fn restore_aggregate_buffers(
    completed: Gfx942SdmaMultiQueueCompletedV1,
    retained: Vec<Gfx942SdmaBufferV1>,
    direction: AggregateDirection,
) -> Result<Vec<Buffers>, Box<dyn std::error::Error>> {
    if completed.plan().request_count() != retained.len()
        || completed.completed().len() != retained.len()
    {
        return Err("aggregate completion cardinality mismatch".into());
    }
    let mut restored = Vec::with_capacity(retained.len());
    for (completed, retained) in completed.into_completed().into_iter().zip(retained) {
        let (source, destination) = completed.into_buffers();
        restored.push(match direction {
            AggregateDirection::HostToDevice => Buffers {
                upload: source,
                device: destination,
                download: retained,
            },
            AggregateDirection::DeviceToHost => Buffers {
                upload: retained,
                device: source,
                download: destination,
            },
        });
    }
    Ok(restored)
}

fn submit_aggregate(
    queue: &mut ComputeAqlQueueSessionV1,
    requests: Vec<Gfx942SdmaCopyRequestV1>,
) -> Result<Gfx942SdmaMultiQueueSubmissionV1, Box<dyn std::error::Error>> {
    queue
        .submit_gfx942_striped_sdma_copy_batch_v1(requests)
        .map_err(|failure| failure.into_parts().0.into())
}

fn wait_aggregate(
    queue: &mut ComputeAqlQueueSessionV1,
    submission: Gfx942SdmaMultiQueueSubmissionV1,
) -> Result<Gfx942SdmaMultiQueueCompletedV1, Box<dyn std::error::Error>> {
    queue
        .wait_gfx942_striped_sdma_copy_batch_for_v1(submission, Duration::from_secs(30))
        .map_err(|failure| failure.into_parts().0.into())
}

fn run_aggregate_phase(
    queue: &mut ComputeAqlQueueSessionV1,
    buffers: Vec<Buffers>,
    copy_bytes: usize,
    direction: AggregateDirection,
) -> Result<(Vec<Buffers>, PhaseTiming), Box<dyn std::error::Error>> {
    let (requests, retained) = aggregate_phase_inputs(buffers, copy_bytes, direction);
    let t0 = Instant::now();
    let submission = submit_aggregate(queue, requests)?;
    let t1 = Instant::now();
    let completed = wait_aggregate(queue, submission)?;
    let t2 = Instant::now();
    let timing = PhaseTiming {
        total_ns: t2.duration_since(t0).as_nanos(),
        submit_ns: t1.duration_since(t0).as_nanos(),
        wait_ns: t2.duration_since(t1).as_nanos(),
    };
    let restored = restore_aggregate_buffers(completed, retained, direction)?;
    Ok((restored, timing))
}

fn finish_aggregate_poll_smoke(
    queue: &mut ComputeAqlQueueSessionV1,
    submission: Gfx942SdmaMultiQueueSubmissionV1,
) -> Result<Gfx942SdmaMultiQueueCompletedV1, Box<dyn std::error::Error>> {
    match queue
        .poll_gfx942_striped_sdma_copy_batch_v1(submission)
        .map_err(|failure| failure.into_parts().0)?
    {
        Gfx942SdmaMultiQueuePollV1::Pending(submission) => wait_aggregate(queue, submission),
        Gfx942SdmaMultiQueuePollV1::Completed(completed) => Ok(completed),
    }
}

fn run_aggregate_poll_smoke(
    queue: &mut ComputeAqlQueueSessionV1,
    queue_count: usize,
    copy_bytes: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffers = allocate_buffers(queue, queue_count, copy_bytes)?;
    prepare_and_poison(queue, &mut buffers, copy_bytes, 0)?;
    let (requests, retained) =
        aggregate_phase_inputs(buffers, copy_bytes, AggregateDirection::HostToDevice);
    let submission = submit_aggregate(queue, requests)?;
    if submission.plan().active_shard_count() != queue_count {
        return Err("aggregate poll smoke did not cover every striped queue".into());
    }
    let completed = finish_aggregate_poll_smoke(queue, submission)?;
    let buffers = restore_aggregate_buffers(completed, retained, AggregateDirection::HostToDevice)?;
    let (buffers, _) =
        run_aggregate_phase(queue, buffers, copy_bytes, AggregateDirection::DeviceToHost)?;
    validate_round(queue, &buffers, copy_bytes, 0)?;
    recycle_buffers(queue, buffers)
}

fn run_directional_smoke(
    queue: &mut ComputeAqlQueueSessionV1,
    copy_bytes: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffers = allocate_buffers(queue, 1, copy_bytes)?;
    prepare_and_poison(queue, &mut buffers, copy_bytes, 0)?;
    let (buffers, _, _) = run_round_combined(queue, buffers, copy_bytes)?;
    validate_round(queue, &buffers, copy_bytes, 0)?;
    recycle_buffers(queue, buffers)
}

fn sha256_ascii(preimage: &str) -> String {
    let digest = Sha256::digest(preimage.as_bytes());
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        write!(rendered, "{byte:02x}").expect("writing to a String cannot fail");
    }
    rendered
}

fn queue_evidence(
    profile: AggregateProfile,
    directional: Option<Gfx942CombinedSdmaCapacityV1>,
    standalone: Vec<Gfx942SdmaQueueObservationV1>,
) -> Result<AggregateQueueEvidence, Box<dyn std::error::Error>> {
    let queue_count = profile.queue_count() as usize;
    let mut roster = Vec::with_capacity(queue_count + profile.directional_queue_count() as usize);
    match profile {
        AggregateProfile::Combined(_) => {
            let capacity = directional.ok_or("combined SDMA capacity observation is missing")?;
            if capacity.striped_queue_count() != queue_count
                || capacity.admitted_engine_count() != 2
                || capacity.maximum_striped_queue_count() < profile.queue_count()
                || !standalone.is_empty()
            {
                return Err("combined SDMA capacity observation mismatch".into());
            }
            let directional = capacity.directional();
            roster.push(("h2d".to_owned(), directional.host_to_device));
            roster.push(("d2h".to_owned(), directional.device_to_host));
            roster.extend(
                capacity
                    .striped()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, observation)| (format!("striped{index}"), observation)),
            );
        }
        AggregateProfile::Standalone16 => {
            if directional.is_some() || standalone.len() != queue_count {
                return Err("standalone striped SDMA capacity observation mismatch".into());
            }
            roster.extend(
                standalone
                    .into_iter()
                    .enumerate()
                    .map(|(index, observation)| (format!("striped{index}"), observation)),
            );
        }
    }
    let mut observed_queue_ids = Vec::with_capacity(roster.len());
    for (index, (_, observation)) in roster.iter().enumerate() {
        let expected_engine = if matches!(profile, AggregateProfile::Combined(_)) && index < 2 {
            1 - index as u32
        } else {
            (index - profile.directional_queue_count() as usize) as u32 % 2
        };
        if observation.engine_index != Some(expected_engine)
            || observed_queue_ids.contains(&observation.queue_id)
        {
            return Err("SDMA queue placement or identity observation mismatch".into());
        }
        observed_queue_ids.push(observation.queue_id);
    }
    let queue_ids = roster
        .iter()
        .map(|(role, observation)| format!("{role}:{}", observation.queue_id))
        .collect::<Vec<_>>()
        .join(",");
    let engine_placement = roster
        .iter()
        .map(|(role, observation)| {
            format!(
                "{role}:{}:{}",
                observation.queue_id,
                observation
                    .engine_index
                    .expect("validated targeted queue observation")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(AggregateQueueEvidence {
        queue_ids_sha256: sha256_ascii(&queue_ids),
        engine_placement_sha256: sha256_ascii(&engine_placement),
        queue_ids,
        engine_placement,
    })
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

fn sample_csv(samples: &[u128]) -> String {
    samples
        .iter()
        .map(u128::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn append_aggregate_metrics(
    row: &mut String,
    direction: &str,
    samples: &AggregateSamples,
    transfer_bytes: usize,
) {
    for (component, values) in [
        ("submit", samples.submit.as_slice()),
        ("wait", samples.wait.as_slice()),
        ("e2e", samples.e2e.as_slice()),
    ] {
        let p50 = percentile(values, 1, 2);
        let p95 = percentile(values, 19, 20);
        write!(
            row,
            " {direction}_{component}_samples_ns={} {direction}_{component}_p50_ns={p50} {direction}_{component}_p95_ns={p95}",
            sample_csv(values),
        )
        .expect("writing to a String cannot fail");
    }
    let e2e_p50 = percentile(&samples.e2e, 1, 2);
    write!(
        row,
        " {direction}_e2e_p50_GBps={:.9}",
        gbps(transfer_bytes, e2e_p50),
    )
    .expect("writing to a String cannot fail");
}

fn run_aggregate_benchmark(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let unique_id = if let Some(hex) = args[0].strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        args[0].parse()?
    };
    let copy_bytes: usize = args[1].parse()?;
    let depth: usize = args[2].parse()?;
    let warmups: usize = args[3].parse()?;
    let sample_count: usize = args[4].parse()?;
    let resource_profile = args[5].as_str();
    let profile = admitted_aggregate_profile(resource_profile)
        .ok_or("unknown aggregate SDMA queue profile")?;
    let queue_count = profile.queue_count() as usize;
    if copy_bytes == 0 || copy_bytes > fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize {
        return Err("copy size is outside one gfx942 linear-copy packet".into());
    }
    if depth == 0
        || !depth.is_multiple_of(queue_count)
        || depth
            > queue_count
                .checked_mul(fe2o3_kfd::GFX942_SDMA_MAX_IN_FLIGHT_V1)
                .ok_or("aggregate depth bound overflow")?
        || sample_count == 0
    {
        return Err("aggregate depth or sample count is out of range".into());
    }
    let rounds = warmups
        .checked_add(sample_count)
        .ok_or("warmup and sample count overflow")?;
    let transfer_bytes = copy_bytes
        .checked_mul(depth)
        .ok_or("aggregate transfer byte count overflow")?;

    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut queue = device.create_compute_aql_queue(4096)?;
    let (combined, standalone) = match profile {
        AggregateProfile::Combined(queue_count) => (
            Some(queue.enable_gfx942_directional_and_striped_sdma_copy_engines_v1(queue_count)?),
            Vec::new(),
        ),
        AggregateProfile::Standalone16 => (
            None,
            queue.enable_gfx942_striped_sdma_copy_engines(profile.queue_count())?,
        ),
    };
    let queue_evidence = queue_evidence(profile, combined, standalone)?;
    if matches!(profile, AggregateProfile::Combined(_)) {
        run_directional_smoke(&mut queue, copy_bytes)?;
    }
    run_aggregate_poll_smoke(&mut queue, queue_count, copy_bytes)?;

    let mut buffers = allocate_buffers(&mut queue, depth, copy_bytes)?;
    let mut h2d = AggregateSamples::with_capacity(sample_count);
    let mut d2h = AggregateSamples::with_capacity(sample_count);
    for round in 0..rounds {
        prepare_and_poison(&mut queue, &mut buffers, copy_bytes, round)?;
        let (next, h2d_timing) = run_aggregate_phase(
            &mut queue,
            buffers,
            copy_bytes,
            AggregateDirection::HostToDevice,
        )?;
        let (next, d2h_timing) = run_aggregate_phase(
            &mut queue,
            next,
            copy_bytes,
            AggregateDirection::DeviceToHost,
        )?;
        buffers = next;
        validate_round(&mut queue, &buffers, copy_bytes, round)?;
        if round >= warmups {
            h2d.push(h2d_timing)?;
            d2h.push(d2h_timing)?;
        }
    }
    recycle_buffers(&mut queue, buffers)?;
    queue.trim_sdma_memory_pool()?;
    queue.destroy()?;

    let workload_id = format!(
        "bytes{copy_bytes}-q{queue_count}-{}",
        profile.workload_kind()
    );
    let mut row = format!(
        "backend=kfd schema=fe2o3.async-copy-striped-benchmark.v3 workload_id={workload_id} unique_id={unique_id:016x} bytes={copy_bytes} depth={depth} logical_queue_count={queue_count} per_queue_depth={} assignment=continuing-round-robin-v1 submit_order=cursor-queue-major-v1 direction=h2d-then-d2h warmups={warmups} samples={sample_count} validation=full-buffer-every-round queue_creation_timed=no allocation_timed=no api=native-kfd-sdma resource_profile={resource_profile} physical_engine_count=2",
        depth / queue_count,
    );
    append_aggregate_metrics(&mut row, "h2d", &h2d, transfer_bytes);
    append_aggregate_metrics(&mut row, "d2h", &d2h, transfer_bytes);
    write!(
        row,
        " directional_queue_count={} striped_queue_count={queue_count} queue_ids={} queue_ids_sha256={} engine_placement={} engine_placement_sha256={} directional_smoke={} aggregate_poll_smoke=pass destroy=pass",
        profile.directional_queue_count(),
        queue_evidence.queue_ids,
        queue_evidence.queue_ids_sha256,
        queue_evidence.engine_placement,
        queue_evidence.engine_placement_sha256,
        profile.directional_smoke(),
    )
    .expect("writing to a String cannot fail");
    println!("{row}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() == 7 && args[6] == "aggregate" {
        return run_aggregate_benchmark(&args);
    }
    if !(5..=6).contains(&args.len()) {
        return Err(
            "usage: kfd-sdma-copy-benchmark <unique-id> <bytes> <depth> <warmups> <samples> [generic|directional|engine0|engine1|striped{even 2..16}]".into(),
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
    let (h2d_engine_index, d2h_engine_index, configured_queues) = match profile {
        "generic" => {
            let queue = queue.enable_sdma_copy_engine()?;
            (queue.engine_index, queue.engine_index, 1)
        }
        "directional" => {
            let queues = queue.enable_gfx942_directional_sdma_copy_engines()?;
            (
                queues.host_to_device.engine_index,
                queues.device_to_host.engine_index,
                1,
            )
        }
        "engine0" => {
            let queue = queue.enable_gfx942_sdma_copy_engine_on_engine_index(0)?;
            (queue.engine_index, queue.engine_index, 1)
        }
        "engine1" => {
            let queue = queue.enable_gfx942_sdma_copy_engine_on_engine_index(1)?;
            (queue.engine_index, queue.engine_index, 1)
        }
        profile if admitted_striped_queue_count(profile).is_some() => {
            let queue_count = admitted_striped_queue_count(profile)
                .expect("guard admits only explicit striped profiles");
            let queues = queue.enable_gfx942_striped_sdma_copy_engines(queue_count)?;
            if queues.len() != queue_count as usize {
                return Err("striped queue observation count mismatch".into());
            }
            (None, None, queues.len())
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
        let (next, h2d_timing, d2h_timing) =
            run_round(&mut queue, buffers, copy_bytes, configured_queues)?;
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
    let mut combined = None;
    if configured_queues == 1 {
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
        combined = Some((
            percentile(&combined_h2d, 1, 2),
            percentile(&combined_d2h, 1, 2),
        ));
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
    let concurrent_batches = configured_queues.min(depth);
    let per_queue_depth = depth.div_ceil(concurrent_batches);
    let pool = queue.sdma_memory_pool_observation()?;
    let trimmed = queue.trim_sdma_memory_pool()?;
    queue.destroy()?;
    let mut row = format!(
        "backend=kfd schema=fe2o3.async-copy-benchmark.v1 unique_id={unique_id:016x} profile={profile} bytes={copy_bytes} depth={depth} queue_depth={per_queue_depth} batch_size={depth} direction=h2d-then-d2h concurrency={concurrent_batches} configured_queues={configured_queues} doorbells_per_batch={concurrent_batches} warmups={warmups} samples={samples} h2d_engine_index={} d2h_engine_index={} h2d_p50_ns={h2d_p50} h2d_p95_ns={h2d_p95} h2d_submit_p50_ns={h2d_submit_p50} h2d_wait_p50_ns={h2d_wait_p50} h2d_p50_GBps={:.3} d2h_p50_ns={d2h_p50} d2h_p95_ns={d2h_p95} d2h_submit_p50_ns={d2h_submit_p50} d2h_wait_p50_ns={d2h_wait_p50} d2h_p50_GBps={:.3} pool_checkout_recycle_pair_ns={pool_ns_per_pair} pool_reuse_count={} pool_trimmed={trimmed}",
        h2d_engine_index.unwrap_or(u32::MAX),
        d2h_engine_index.unwrap_or(u32::MAX),
        gbps(transferred, h2d_p50),
        gbps(transferred, d2h_p50),
        pool.reuse_count,
    );
    if let Some((combined_h2d_p50, combined_d2h_p50)) = combined {
        row.push_str(&format!(
            " combined_h2d_p50_ns={combined_h2d_p50} combined_h2d_p50_GBps={:.3} combined_d2h_p50_ns={combined_d2h_p50} combined_d2h_p50_GBps={:.3}",
            gbps(transferred, combined_h2d_p50),
            gbps(transferred, combined_d2h_p50),
        ));
    }
    println!("{row}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AggregateProfile, AggregateSamples, PhaseTiming, admitted_aggregate_profile,
        admitted_striped_queue_count, balanced_batch_lengths, sha256_ascii,
    };

    #[test]
    fn balanced_shards_cover_every_item_once() {
        assert_eq!(balanced_batch_lengths(0, 16), []);
        assert_eq!(balanced_batch_lengths(1, 16), [1]);
        assert_eq!(balanced_batch_lengths(16, 16), [1; 16]);
        assert_eq!(balanced_batch_lengths(17, 4), [5, 4, 4, 4]);
        assert_eq!(balanced_batch_lengths(8, 0), [8]);
    }

    #[test]
    fn striped_profile_roster_is_exact() {
        assert_eq!(admitted_striped_queue_count("striped2"), Some(2));
        assert_eq!(admitted_striped_queue_count("striped16"), Some(16));
        for rejected in ["striped0", "striped1", "striped3", "striped18", "stripedx"] {
            assert_eq!(admitted_striped_queue_count(rejected), None);
        }
    }

    #[test]
    fn aggregate_profile_roster_and_digest_are_exact() {
        assert_eq!(
            admitted_aggregate_profile("combined-striped2"),
            Some(AggregateProfile::Combined(2))
        );
        assert_eq!(
            admitted_aggregate_profile("combined-striped14"),
            Some(AggregateProfile::Combined(14))
        );
        assert_eq!(
            admitted_aggregate_profile("striped16"),
            Some(AggregateProfile::Standalone16)
        );
        for rejected in [
            "combined-striped0",
            "combined-striped6",
            "combined-striped16",
            "striped14",
        ] {
            assert_eq!(admitted_aggregate_profile(rejected), None);
        }
        assert_eq!(
            sha256_ascii("striped0:7,striped1:9"),
            "4198c4dae0e6f70aa220a2aeb33789e80f8955f94e8bdb421825be6b8f204a8a"
        );
    }

    #[test]
    fn aggregate_samples_require_exact_positive_phase_accounting() {
        let mut samples = AggregateSamples::with_capacity(1);
        samples
            .push(PhaseTiming {
                total_ns: 7,
                submit_ns: 2,
                wait_ns: 5,
            })
            .unwrap();
        assert_eq!(samples.submit, [2]);
        assert_eq!(samples.wait, [5]);
        assert_eq!(samples.e2e, [7]);
        assert!(
            samples
                .push(PhaseTiming {
                    total_ns: 8,
                    submit_ns: 2,
                    wait_ns: 5,
                })
                .is_err()
        );
    }

    #[test]
    fn aggregate_source_keeps_setup_and_teardown_outside_timing() {
        let source = include_str!("kfd-sdma-copy-benchmark.rs");
        let phase = source
            .split("fn run_aggregate_phase(")
            .nth(1)
            .unwrap()
            .split("fn finish_aggregate_poll_smoke(")
            .next()
            .unwrap();
        for marker in [
            "aggregate_phase_inputs(buffers, copy_bytes, direction)",
            "let t0 = Instant::now()",
            "submit_aggregate(queue, requests)",
            "let t1 = Instant::now()",
            "wait_aggregate(queue, submission)",
            "let t2 = Instant::now()",
            "restore_aggregate_buffers(completed, retained, direction)",
        ] {
            assert!(phase.contains(marker));
        }
        assert!(
            phase.find("aggregate_phase_inputs").unwrap()
                < phase.find("let t0 = Instant::now()").unwrap()
        );
        assert!(
            phase.find("let t2 = Instant::now()").unwrap()
                < phase.find("restore_aggregate_buffers").unwrap()
        );
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains(".tickets()"));
        assert!(!production.contains("into_tickets"));

        let benchmark = source
            .split("fn run_aggregate_benchmark(")
            .nth(1)
            .unwrap()
            .split("fn main()")
            .next()
            .unwrap();
        assert!(
            benchmark.find("queue.destroy()?").unwrap() < benchmark.find("destroy=pass").unwrap()
        );
    }
}
