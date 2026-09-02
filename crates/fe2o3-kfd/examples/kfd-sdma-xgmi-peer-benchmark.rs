//! Native bidirectional gfx942 XGMI SDMA validation and depth benchmark.

use std::time::{Duration, Instant};

use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, DeviceSelector, Gfx942DeviceMemoryLeaseV1,
    Gfx942DeviceMemoryUnmappedV1, Gfx942NativeXgmiSdmaQueueV1, Gfx942XgmiMapRecoveryV1,
    Gfx942XgmiMappedDeviceMemoryV1, Gfx942XgmiSdmaCopyRequestV1, Gfx942XgmiUnmapRecoveryV1,
    OpenedKfd, SharedGttMemorySessionV1, topology::Gfx942XgmiRouteV1,
};

const CANARY_BYTES: usize = 32;

struct Pair {
    source: Gfx942XgmiMappedDeviceMemoryV1,
    destination: Gfx942XgmiMappedDeviceMemoryV1,
}

fn pattern(round: usize, slot: usize, direction: usize) -> u8 {
    let value = ((round as u128 * 67 + slot as u128 * 29 + direction as u128 * 101 + 1) % 251) + 1;
    u8::try_from(value).expect("pattern is reduced to u8 range")
}

fn percentile(values: &[u128], numerator: usize, denominator: usize) -> Option<u128> {
    if values.is_empty() || numerator == 0 || denominator == 0 || numerator > denominator {
        return None;
    }
    let scaled = values.len() as u128 * numerator as u128 + denominator as u128 - 1;
    let rank = usize::try_from(scaled / denominator as u128).ok()?;
    values.get(rank.checked_sub(1)?).copied()
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

fn gpu_id(
    device: &CheckedGfx942XnackMinusDevice,
    unique_id: u64,
) -> Result<u32, Box<dyn std::error::Error>> {
    let raw = device
        .topology_snapshot()
        .topology()
        .gpu_nodes()
        .iter()
        .find(|node| node.unique_id() == unique_id)
        .ok_or("selected unique ID disappeared from retained topology")?
        .gpu_id();
    Ok(u32::try_from(raw)?)
}

fn map_with_cleanup(
    owner: &mut SharedGttMemorySessionV1,
    peer: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
) -> Result<Gfx942XgmiMappedDeviceMemoryV1, Box<dyn std::error::Error>> {
    match owner.map_gfx942_device_memory_for_xgmi_peer(peer, route, lease) {
        Ok(mapping) => Ok(mapping),
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            match recovery {
                Gfx942XgmiMapRecoveryV1::Unmapped(lease) => {
                    owner.release_gfx942_device_memory(lease)?;
                }
                Gfx942XgmiMapRecoveryV1::PartiallyMapped(mapping) => {
                    match owner.unmap_gfx942_device_memory_from_xgmi_peer(peer, route, mapping) {
                        Ok(lease) => owner.release_gfx942_device_memory(lease)?,
                        Err(cleanup) => {
                            return Err(format!(
                                "XGMI map failed ({error}); cleanup is indeterminate ({})",
                                cleanup.error()
                            )
                            .into());
                        }
                    }
                }
            }
            Err(error.into())
        }
    }
}

fn allocate_pair(
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    copy_bytes: usize,
    source_value: u8,
    source_canary: u8,
    destination_canary: u8,
) -> Result<Pair, Box<dyn std::error::Error>> {
    let total = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .ok_or("XGMI allocation size overflow")?;
    let total_u64 = u64::try_from(total)?;
    let mut source_bytes = vec![source_canary; total];
    source_bytes[CANARY_BYTES..CANARY_BYTES + copy_bytes].fill(source_value);
    let destination_bytes = vec![destination_canary; total];
    let source_lease = source.allocate_gfx942_xgmi_device_memory(total_u64, 4096)?;
    source.write_gfx942_xgmi_device_memory(&source_lease, &source_bytes)?;
    let destination_lease = destination.allocate_gfx942_xgmi_device_memory(total_u64, 4096)?;
    destination.write_gfx942_xgmi_device_memory(&destination_lease, &destination_bytes)?;
    let source_mapping = map_with_cleanup(source, destination, route, source_lease)?;
    let destination_mapping = map_with_cleanup(destination, source, route, destination_lease)?;
    Ok(Pair {
        source: source_mapping,
        destination: destination_mapping,
    })
}

fn run_round(
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    source_session: &mut SharedGttMemorySessionV1,
    destination_session: &mut SharedGttMemorySessionV1,
    pairs: &mut Vec<Pair>,
    copy_bytes: u32,
) -> Result<u128, Box<dyn std::error::Error>> {
    let mut requests = Vec::with_capacity(pairs.len());
    let mut batch = queue.begin_batch(source_session, destination_session)?;
    for pair in std::mem::take(pairs) {
        requests.push(Gfx942XgmiSdmaCopyRequestV1::new(
            pair.source,
            CANARY_BYTES as u64,
            pair.destination,
            CANARY_BYTES as u64,
            copy_bytes,
        ));
    }
    let start = Instant::now();
    let tickets = batch
        .submit_batch(requests)
        .map_err(|failure| failure.error().to_string())?;
    let completed = batch
        .wait_batch_for(tickets, Duration::from_secs(30))
        .map_err(|failure| failure.error().to_string())?;
    for completed in completed {
        let (source, destination) = completed.into_mappings();
        pairs.push(Pair {
            source,
            destination,
        });
    }
    let elapsed = start.elapsed().as_nanos();
    batch.finish()?;
    Ok(elapsed)
}

fn validate_bytes(
    observed: &[u8],
    copy_bytes: usize,
    outer: u8,
    inner: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let end = CANARY_BYTES
        .checked_add(copy_bytes)
        .ok_or("XGMI validation range overflow")?;
    let expected_total = end
        .checked_add(CANARY_BYTES)
        .ok_or("XGMI validation size overflow")?;
    if observed.len() != expected_total
        || !observed[..CANARY_BYTES].iter().all(|byte| *byte == outer)
        || !observed[CANARY_BYTES..end]
            .iter()
            .all(|byte| *byte == inner)
        || !observed[end..].iter().all(|byte| *byte == outer)
    {
        return Err("XGMI payload or canary mismatch".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_round(
    source_session: &mut SharedGttMemorySessionV1,
    destination_session: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    pairs: &mut Vec<Pair>,
    copy_bytes: usize,
    round: usize,
    direction: usize,
    source_canary: u8,
    destination_canary: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let total = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .ok_or("XGMI preparation size overflow")?;
    let mut prepared = Vec::with_capacity(pairs.len());
    for (slot, pair) in std::mem::take(pairs).into_iter().enumerate() {
        let source_lease = source_session
            .unmap_gfx942_device_memory_from_xgmi_peer(destination_session, route, pair.source)
            .map_err(|failure| failure.error().to_string())?;
        let destination_lease = destination_session
            .unmap_gfx942_device_memory_from_xgmi_peer(source_session, route, pair.destination)
            .map_err(|failure| failure.error().to_string())?;
        if round != 0 {
            let prior = pattern(round - 1, slot, direction);
            let observed_source = source_session.read_gfx942_xgmi_device_memory(&source_lease)?;
            let observed_destination =
                destination_session.read_gfx942_xgmi_device_memory(&destination_lease)?;
            validate_bytes(&observed_source, copy_bytes, source_canary, prior)?;
            validate_bytes(&observed_destination, copy_bytes, destination_canary, prior)?;
        }
        let next = pattern(round, slot, direction);
        let mut source_bytes = vec![source_canary; total];
        source_bytes[CANARY_BYTES..CANARY_BYTES + copy_bytes].fill(next);
        let mut destination_bytes = vec![destination_canary; total];
        destination_bytes[CANARY_BYTES..CANARY_BYTES + copy_bytes].fill(next ^ 0xff);
        source_session.write_gfx942_xgmi_device_memory(&source_lease, &source_bytes)?;
        destination_session
            .write_gfx942_xgmi_device_memory(&destination_lease, &destination_bytes)?;
        prepared.push(Pair {
            source: map_with_cleanup(source_session, destination_session, route, source_lease)?,
            destination: map_with_cleanup(
                destination_session,
                source_session,
                route,
                destination_lease,
            )?,
        });
    }
    *pairs = prepared;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_and_release_pair(
    owner: &mut SharedGttMemorySessionV1,
    peer: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    mapping: Gfx942XgmiMappedDeviceMemoryV1,
    copy_bytes: usize,
    outer: u8,
    inner: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let lease = match owner.unmap_gfx942_device_memory_from_xgmi_peer(peer, route, mapping) {
        Ok(lease) => lease,
        Err(failure) => {
            let error = failure.error().to_string();
            match failure.into_parts().1 {
                Gfx942XgmiUnmapRecoveryV1::Unmapped(lease) => {
                    owner.release_gfx942_device_memory(lease)?;
                }
                Gfx942XgmiUnmapRecoveryV1::PartiallyUnmapped(_) => {}
            }
            return Err(error.into());
        }
    };
    let observed = owner.read_gfx942_xgmi_device_memory(&lease)?;
    validate_bytes(&observed, copy_bytes, outer, inner)?;
    owner.release_gfx942_device_memory(lease)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("usage: kfd-sdma-xgmi-peer-benchmark <unique-id-0> <unique-id-1> <bytes> <depth> <warmups> <samples>".into());
    }
    let unique_ids = [parse_unique_id(&args[0])?, parse_unique_id(&args[1])?];
    let copy_bytes: usize = args[2].parse()?;
    let depth: usize = args[3].parse()?;
    let warmups: usize = args[4].parse()?;
    let samples: usize = args[5].parse()?;
    if unique_ids[0] == unique_ids[1]
        || copy_bytes == 0
        || copy_bytes > fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize
        || depth == 0
        || depth > 32
        || samples == 0
    {
        return Err("XGMI benchmark controls are out of range".into());
    }
    let rounds = warmups.checked_add(samples).ok_or("round count overflow")?;
    let bytes_per_round = copy_bytes
        .checked_mul(depth)
        .ok_or("XGMI bytes per round overflow")?;
    let copy_bytes_u32 = u32::try_from(copy_bytes)?;

    // Process-wide XNACK admission for both devices precedes either VM.
    let left_device = admit_device(unique_ids[0])?;
    let right_device = admit_device(unique_ids[1])?;
    let gpu_ids = [
        gpu_id(&left_device, unique_ids[0])?,
        gpu_id(&left_device, unique_ids[1])?,
    ];
    let forward = left_device
        .topology_snapshot()
        .topology()
        .admit_gfx942_xgmi_route(gpu_ids[0], gpu_ids[1])?;
    let reverse = left_device
        .topology_snapshot()
        .topology()
        .admit_gfx942_xgmi_route(gpu_ids[1], gpu_ids[0])?;
    let mut left = left_device.acquire_shared_gtt_memory_session()?;
    let mut right = right_device.acquire_shared_gtt_memory_session()?;
    let mut forward_queue = Gfx942NativeXgmiSdmaQueueV1::create(&mut left, &mut right, forward)?;
    let mut reverse_queue = Gfx942NativeXgmiSdmaQueueV1::create(&mut right, &mut left, reverse)?;

    let mut forward_pairs = Vec::with_capacity(depth);
    let mut reverse_pairs = Vec::with_capacity(depth);
    for _ in 0..depth {
        forward_pairs.push(allocate_pair(
            &mut left, &mut right, forward, copy_bytes, 0x35, 0x17, 0xa5,
        )?);
        reverse_pairs.push(allocate_pair(
            &mut right, &mut left, reverse, copy_bytes, 0xca, 0x71, 0x5a,
        )?);
    }
    let mut forward_ns = Vec::with_capacity(samples);
    let mut reverse_ns = Vec::with_capacity(samples);
    for round in 0..rounds {
        prepare_round(
            &mut left,
            &mut right,
            forward,
            &mut forward_pairs,
            copy_bytes,
            round,
            0,
            0x17,
            0xa5,
        )?;
        let forward_elapsed = run_round(
            &mut forward_queue,
            &mut left,
            &mut right,
            &mut forward_pairs,
            copy_bytes_u32,
        )?;
        prepare_round(
            &mut right,
            &mut left,
            reverse,
            &mut reverse_pairs,
            copy_bytes,
            round,
            1,
            0x71,
            0x5a,
        )?;
        let reverse_elapsed = run_round(
            &mut reverse_queue,
            &mut right,
            &mut left,
            &mut reverse_pairs,
            copy_bytes_u32,
        )?;
        if round >= warmups {
            forward_ns.push(forward_elapsed);
            reverse_ns.push(reverse_elapsed);
        }
    }

    let final_round = rounds
        .checked_sub(1)
        .ok_or("missing XGMI benchmark round")?;
    for (slot, pair) in forward_pairs.into_iter().enumerate() {
        let expected = pattern(final_round, slot, 0);
        validate_and_release_pair(
            &mut left,
            &mut right,
            forward,
            pair.source,
            copy_bytes,
            0x17,
            expected,
        )?;
        validate_and_release_pair(
            &mut right,
            &mut left,
            forward,
            pair.destination,
            copy_bytes,
            0xa5,
            expected,
        )?;
    }
    for (slot, pair) in reverse_pairs.into_iter().enumerate() {
        let expected = pattern(final_round, slot, 1);
        validate_and_release_pair(
            &mut right,
            &mut left,
            reverse,
            pair.source,
            copy_bytes,
            0x71,
            expected,
        )?;
        validate_and_release_pair(
            &mut left,
            &mut right,
            reverse,
            pair.destination,
            copy_bytes,
            0x5a,
            expected,
        )?;
    }
    reverse_queue.destroy_and_release(&mut right, &mut left)?;
    forward_queue.destroy_and_release(&mut left, &mut right)?;
    forward_ns.sort_unstable();
    reverse_ns.sort_unstable();
    let forward_p50 = percentile(&forward_ns, 1, 2).ok_or("missing forward p50")?;
    let forward_p95 = percentile(&forward_ns, 19, 20).ok_or("missing forward p95")?;
    let reverse_p50 = percentile(&reverse_ns, 1, 2).ok_or("missing reverse p50")?;
    let reverse_p95 = percentile(&reverse_ns, 19, 20).ok_or("missing reverse p95")?;
    if forward_p50 == 0 || reverse_p50 == 0 {
        return Err("zero XGMI benchmark duration".into());
    }
    println!(
        "backend=kfd schema=fe2o3.xgmi-peer-benchmark.v1 gpu_ids={},{} unique_ids={:016x},{:016x} target=gfx942:xnack- bytes={} depth={} queue_depth={} batch_size={} direction=forward-then-reverse concurrency=1 warmups={} samples={} peer_access=topology-xgmi doorbells_per_batch=1 forward_engine={} reverse_engine={} forward_p50_ns={} forward_p95_ns={} forward_p50_GBps={:.3} reverse_p50_ns={} reverse_p95_ns={} reverse_p50_GBps={:.3} canaries=pass teardown=explicit",
        gpu_ids[0],
        gpu_ids[1],
        unique_ids[0],
        unique_ids[1],
        copy_bytes,
        depth,
        depth,
        depth,
        warmups,
        samples,
        forward.recommended_engine_id(),
        reverse.recommended_engine_id(),
        forward_p50,
        forward_p95,
        bytes_per_round as f64 / forward_p50 as f64,
        reverse_p50,
        reverse_p95,
        bytes_per_round as f64 / reverse_p50 as f64,
    );
    Ok(())
}
