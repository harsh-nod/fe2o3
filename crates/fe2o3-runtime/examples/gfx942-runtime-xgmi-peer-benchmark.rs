//! Public-runtime-facade gfx942 XGMI peer-copy benchmark.

use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

use fe2o3_runtime::{
    KfdNativeXgmiRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeContextV1,
    RuntimeDeviceIdV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimePeerCopyV1,
    RuntimePollV1, RuntimeStreamIdV1, RuntimeSubmissionV1,
};

const CANARY_BYTES: usize = 32;
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DEPTH: usize = 32;
const USAGE: &str = "usage: gfx942-runtime-xgmi-peer-benchmark <unique-id-0> <unique-id-1> <bytes> <depth> <warmups> <samples>";

type BenchmarkResult<T> = Result<T, Box<dyn Error>>;
type XgmiContextV1 = RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>;

struct DirectionResourcesV1 {
    sources: Vec<RuntimeAllocationIdV1>,
    destinations: Vec<RuntimeAllocationIdV1>,
    streams: Vec<RuntimeStreamIdV1>,
}

fn facade_error(error: impl Debug) -> Box<dyn Error> {
    format!("{error:?}").into()
}

fn parse_unique_id(value: &str) -> BenchmarkResult<u64> {
    let unique_id = if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        value.parse()?
    };
    if unique_id == 0 {
        return Err("unique IDs must be nonzero".into());
    }
    Ok(unique_id)
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

#[allow(clippy::too_many_arguments)]
fn report_measurement(
    unique_ids: [u64; 2],
    copy_bytes: usize,
    depth: usize,
    warmups: usize,
    samples: usize,
    measurement: &str,
    mapping_lifetime: &str,
    prime_batches: usize,
    mut forward_ns: Vec<u128>,
    mut reverse_ns: Vec<u128>,
) -> BenchmarkResult<()> {
    forward_ns.sort_unstable();
    reverse_ns.sort_unstable();
    let forward_p50 = percentile(&forward_ns, 1, 2).ok_or("missing forward p50")?;
    let forward_p95 = percentile(&forward_ns, 19, 20).ok_or("missing forward p95")?;
    let reverse_p50 = percentile(&reverse_ns, 1, 2).ok_or("missing reverse p50")?;
    let reverse_p95 = percentile(&reverse_ns, 19, 20).ok_or("missing reverse p95")?;
    if forward_p50 == 0 || reverse_p50 == 0 {
        return Err("zero XGMI benchmark duration".into());
    }
    let bytes_per_round = copy_bytes
        .checked_mul(depth)
        .ok_or("XGMI bytes per round overflow")?;
    println!(
        "backend=kfd schema=fe2o3.xgmi-peer-benchmark.v1 surface=runtime-facade unique_ids={:016x},{:016x} target=gfx942:xnack- bytes={} depth={} queue_depth={} batch_size={} direction=forward-then-reverse outstanding_depth={} engine_parallelism=ordered-single-sdma warmups={} samples={} measurement={} peer_access=topology-xgmi mapping_lifetime={} prime_batches={} doorbells_per_batch=1 progress=explicit-flush-then-wait background_progress=false forward_engine=topology-selected reverse_engine=topology-selected forward_p50_ns={} forward_p95_ns={} forward_p50_GBps={:.3} reverse_p50_ns={} reverse_p95_ns={} reverse_p50_GBps={:.3} canaries=pass teardown=explicit timing=facade-enqueue-flush-through-observed-completion",
        unique_ids[0],
        unique_ids[1],
        copy_bytes,
        depth,
        depth,
        depth,
        depth,
        warmups,
        samples,
        measurement,
        mapping_lifetime,
        prime_batches,
        forward_p50,
        forward_p95,
        bytes_per_round as f64 / forward_p50 as f64,
        reverse_p50,
        reverse_p95,
        bytes_per_round as f64 / reverse_p50 as f64,
    );
    Ok(())
}

fn guarded_bytes(copy_bytes: usize, outer: u8, inner: u8) -> BenchmarkResult<Vec<u8>> {
    let total = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .ok_or("XGMI allocation size overflow")?;
    let mut bytes = vec![outer; total];
    bytes[CANARY_BYTES..CANARY_BYTES + copy_bytes].fill(inner);
    Ok(bytes)
}

fn validate_guarded_bytes(
    observed: &[u8],
    copy_bytes: usize,
    outer: u8,
    inner: u8,
) -> BenchmarkResult<()> {
    let end = CANARY_BYTES
        .checked_add(copy_bytes)
        .ok_or("XGMI validation range overflow")?;
    let total = end
        .checked_add(CANARY_BYTES)
        .ok_or("XGMI validation size overflow")?;
    if observed.len() != total
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

fn allocate_direction(
    context: &mut XgmiContextV1,
    source_device: RuntimeDeviceIdV1,
    destination_device: RuntimeDeviceIdV1,
    total_bytes: u64,
    depth: usize,
) -> BenchmarkResult<DirectionResourcesV1> {
    let mut sources = Vec::with_capacity(depth);
    let mut destinations = Vec::with_capacity(depth);
    let mut streams = Vec::with_capacity(depth);
    for _ in 0..depth {
        sources.push(
            context
                .allocate(
                    source_device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    total_bytes,
                    4096,
                )
                .map_err(facade_error)?,
        );
        destinations.push(
            context
                .allocate(
                    destination_device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    total_bytes,
                    4096,
                )
                .map_err(facade_error)?,
        );
        streams.push(
            context
                .create_stream(destination_device)
                .map_err(facade_error)?,
        );
    }
    Ok(DirectionResourcesV1 {
        sources,
        destinations,
        streams,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_direction(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: usize,
    round: usize,
    direction: usize,
    source_canary: u8,
    destination_canary: u8,
) -> BenchmarkResult<()> {
    for slot in 0..resources.sources.len() {
        let value = pattern(round, slot, direction);
        let source = guarded_bytes(copy_bytes, source_canary, value)?;
        let destination = guarded_bytes(copy_bytes, destination_canary, value ^ 0xff)?;
        context
            .write_allocation(resources.sources[slot], 0, &source)
            .map_err(facade_error)?;
        context
            .write_allocation(resources.destinations[slot], 0, &destination)
            .map_err(facade_error)?;
    }
    Ok(())
}

fn run_direction(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: u64,
) -> BenchmarkResult<u128> {
    let mut submissions: Vec<RuntimeSubmissionV1<RuntimePeerCopyV1>> =
        Vec::with_capacity(resources.sources.len());
    let start = Instant::now();
    for slot in 0..resources.sources.len() {
        submissions.push(
            context
                .peer_copy(
                    resources.streams[slot],
                    RuntimeMemoryRegionV1 {
                        allocation: resources.sources[slot],
                        access: RuntimeAccessV1::Read,
                        byte_offset: CANARY_BYTES as u64,
                        byte_len: copy_bytes,
                    },
                    RuntimeMemoryRegionV1 {
                        allocation: resources.destinations[slot],
                        access: RuntimeAccessV1::Write,
                        byte_offset: CANARY_BYTES as u64,
                        byte_len: copy_bytes,
                    },
                    &[],
                )
                .map_err(facade_error)?,
        );
    }
    context
        .flush_stream(resources.streams[0])
        .map_err(facade_error)?;
    for submission in &mut submissions {
        let status = context
            .wait(submission, COMPLETION_TIMEOUT)
            .map_err(facade_error)?;
        if status != RuntimePollV1::Succeeded {
            return Err(format!("XGMI peer copy did not succeed: {status:?}").into());
        }
    }
    let elapsed = start.elapsed().as_nanos();
    for submission in submissions {
        context
            .release_submission(submission)
            .map_err(facade_error)?;
    }
    Ok(elapsed)
}

#[allow(clippy::too_many_arguments)]
fn validate_direction(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: usize,
    round: usize,
    direction: usize,
    source_canary: u8,
    destination_canary: u8,
) -> BenchmarkResult<()> {
    let total = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .ok_or("XGMI validation size overflow")?;
    let mut observed = vec![0_u8; total];
    for slot in 0..resources.sources.len() {
        let expected = pattern(round, slot, direction);
        context
            .read_allocation(resources.sources[slot], 0, &mut observed)
            .map_err(facade_error)?;
        validate_guarded_bytes(&observed, copy_bytes, source_canary, expected)?;
        observed.fill(0);
        context
            .read_allocation(resources.destinations[slot], 0, &mut observed)
            .map_err(facade_error)?;
        validate_guarded_bytes(&observed, copy_bytes, destination_canary, expected)?;
    }
    Ok(())
}

fn release_direction(
    context: &mut XgmiContextV1,
    resources: DirectionResourcesV1,
) -> BenchmarkResult<()> {
    for destination in resources.destinations.into_iter().rev() {
        context
            .release_allocation(destination)
            .map_err(facade_error)?;
    }
    for source in resources.sources.into_iter().rev() {
        context.release_allocation(source).map_err(facade_error)?;
    }
    for stream in resources.streams.into_iter().rev() {
        context.destroy_stream(stream).map_err(facade_error)?;
    }
    Ok(())
}

fn main() -> BenchmarkResult<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 {
        return Err(USAGE.into());
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
        || depth > MAX_DEPTH
        || samples == 0
    {
        return Err("XGMI benchmark controls are out of range".into());
    }
    let rounds = warmups.checked_add(samples).ok_or("round count overflow")?;
    let total_bytes = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or("XGMI allocation size overflow")?;

    let backend = KfdNativeXgmiRuntimeBackendV1::open_default(unique_ids[0], unique_ids[1])?;
    let mut context = RuntimeContextV1::open(backend).map_err(facade_error)?;
    if context.devices().len() != 2
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err("native XGMI facade did not enumerate two gfx942:xnack- devices".into());
    }
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for device in devices {
        let capabilities = context.execution_capabilities(device)?;
        if !capabilities.native_peer_copy {
            return Err("native XGMI facade did not report native peer copy".into());
        }
    }

    let forward = allocate_direction(&mut context, devices[0], devices[1], total_bytes, depth)?;
    let reverse = allocate_direction(&mut context, devices[1], devices[0], total_bytes, depth)?;
    let mut remap_forward_ns = Vec::with_capacity(samples);
    let mut remap_reverse_ns = Vec::with_capacity(samples);
    for round in 0..rounds {
        prepare_direction(&mut context, &forward, copy_bytes, round, 0, 0x17, 0xa5)?;
        let elapsed = run_direction(&mut context, &forward, copy_bytes as u64)?;
        validate_direction(&mut context, &forward, copy_bytes, round, 0, 0x17, 0xa5)?;
        if round >= warmups {
            remap_forward_ns.push(elapsed);
        }

        prepare_direction(&mut context, &reverse, copy_bytes, round, 1, 0x71, 0x5a)?;
        let elapsed = run_direction(&mut context, &reverse, copy_bytes as u64)?;
        validate_direction(&mut context, &reverse, copy_bytes, round, 1, 0x71, 0x5a)?;
        if round >= warmups {
            remap_reverse_ns.push(elapsed);
        }
    }

    // Establish one mapped, completed batch in each direction, then time only
    // repetitions with no intervening host access. Final readback validates the
    // exact payload and canaries after the entire persistent-hot sequence.
    let hot_pattern_round = rounds.checked_add(1).ok_or("hot pattern round overflow")?;
    prepare_direction(
        &mut context,
        &forward,
        copy_bytes,
        hot_pattern_round,
        0,
        0x17,
        0xa5,
    )?;
    prepare_direction(
        &mut context,
        &reverse,
        copy_bytes,
        hot_pattern_round,
        1,
        0x71,
        0x5a,
    )?;
    let _ = run_direction(&mut context, &forward, copy_bytes as u64)?;
    let _ = run_direction(&mut context, &reverse, copy_bytes as u64)?;
    let mut hot_forward_ns = Vec::with_capacity(samples);
    let mut hot_reverse_ns = Vec::with_capacity(samples);
    for round in 0..rounds {
        let forward_elapsed = run_direction(&mut context, &forward, copy_bytes as u64)?;
        let reverse_elapsed = run_direction(&mut context, &reverse, copy_bytes as u64)?;
        if round >= warmups {
            hot_forward_ns.push(forward_elapsed);
            hot_reverse_ns.push(reverse_elapsed);
        }
    }
    validate_direction(
        &mut context,
        &forward,
        copy_bytes,
        hot_pattern_round,
        0,
        0x17,
        0xa5,
    )?;
    validate_direction(
        &mut context,
        &reverse,
        copy_bytes,
        hot_pattern_round,
        1,
        0x71,
        0x5a,
    )?;

    release_direction(&mut context, reverse)?;
    release_direction(&mut context, forward)?;
    let mut backend = context.shutdown().map_err(facade_error)?;
    backend.shutdown_native_v1().map_err(facade_error)?;

    report_measurement(
        unique_ids,
        copy_bytes,
        depth,
        warmups,
        samples,
        "remap-per-round",
        "host-access-between-rounds",
        0,
        remap_forward_ns,
        remap_reverse_ns,
    )?;
    report_measurement(
        unique_ids,
        copy_bytes,
        depth,
        warmups,
        samples,
        "persistent-hot",
        "persistent-no-host-access-between-timed-rounds",
        1,
        hot_forward_ns,
        hot_reverse_ns,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canaries_bind_the_exact_inner_copy_region() {
        let bytes = guarded_bytes(17, 0xa5, 0x3c).unwrap();
        validate_guarded_bytes(&bytes, 17, 0xa5, 0x3c).unwrap();
        let mut prefix = bytes.clone();
        prefix[0] ^= 1;
        assert!(validate_guarded_bytes(&prefix, 17, 0xa5, 0x3c).is_err());
        let mut payload = bytes.clone();
        payload[CANARY_BYTES + 8] ^= 1;
        assert!(validate_guarded_bytes(&payload, 17, 0xa5, 0x3c).is_err());
        let mut suffix = bytes;
        *suffix.last_mut().unwrap() ^= 1;
        assert!(validate_guarded_bytes(&suffix, 17, 0xa5, 0x3c).is_err());
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let values = [10, 20, 30, 40, 50];
        assert_eq!(percentile(&values, 1, 2), Some(30));
        assert_eq!(percentile(&values, 19, 20), Some(50));
        assert_eq!(percentile(&[], 1, 2), None);
        assert_eq!(percentile(&values, 0, 2), None);
    }

    #[test]
    fn patterns_distinguish_round_slot_and_direction() {
        assert_ne!(pattern(0, 0, 0), pattern(1, 0, 0));
        assert_ne!(pattern(0, 0, 0), pattern(0, 1, 0));
        assert_ne!(pattern(0, 0, 0), pattern(0, 0, 1));
    }
}
