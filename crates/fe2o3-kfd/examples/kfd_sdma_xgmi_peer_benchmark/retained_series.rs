//! A separately labeled native API timing surface, never the runtime facade.

use super::*;
use fe2o3_kfd::{
    GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1, GFX942_XGMI_RETAINED_PAIR_PROFILE_V1,
    Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
};

struct Samples {
    copies: Vec<u128>,
    scope_entry_ns: u128,
    scope_finish_ns: u128,
}

fn collect_series<E>(
    warmups: usize,
    samples: usize,
    mut copy: impl FnMut() -> Result<u128, E>,
) -> Result<Vec<u128>, E> {
    let mut durations = Vec::with_capacity(samples);
    copy()?;
    for _ in 0..warmups {
        copy()?;
    }
    for _ in 0..samples {
        durations.push(copy()?);
    }
    Ok(durations)
}

fn series(
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    pairs: &mut Vec<Pair>,
    copy_bytes: u32,
    warmups: usize,
    samples: usize,
) -> Result<Samples, Box<dyn std::error::Error>> {
    let depth = pairs.len();
    let entry = Instant::now();
    let mut scope = queue.begin_ordinary_retained_pair_v1(
        source,
        destination,
        Gfx942XgmiRetainedPairEnvironmentAssumptionV1::ReviewedMi300xAmdgpu61613OrdinaryLifetime,
    )?;
    let scope_entry_ns = entry.elapsed().as_nanos();
    let durations = collect_series(
        warmups,
        samples,
        || -> Result<u128, Box<dyn std::error::Error>> {
            let requests = pairs
                .drain(..)
                .map(|pair| {
                    Gfx942XgmiSdmaCopyRequestV1::new(
                        pair.source,
                        CANARY_BYTES as u64,
                        pair.destination,
                        CANARY_BYTES as u64,
                        copy_bytes,
                    )
                })
                .collect();
            let start = Instant::now();
            let tickets = scope
                .submit_batch(requests)
                .map_err(|failure| failure.error().to_string())?;
            let completed = scope
                .wait_batch_for(tickets, Duration::from_secs(30))
                .map_err(|failure| failure.error().to_string())?;
            let elapsed = start.elapsed().as_nanos();
            if completed.len() != depth {
                return Err("retained series completed roster mismatch".into());
            }
            for copy in completed.into_copies() {
                if copy.copy_bytes() != copy_bytes {
                    return Err("retained series completed byte count mismatch".into());
                }
                let (source, destination) = copy.into_mappings();
                pairs.push(Pair {
                    source,
                    destination,
                });
            }
            Ok(elapsed)
        },
    )?;
    let finish = Instant::now();
    scope.finish()?;
    let scope_finish_ns = finish.elapsed().as_nanos();
    if durations.len() != samples {
        return Err("retained series sample count mismatch".into());
    }
    Ok(Samples {
        copies: durations,
        scope_entry_ns,
        scope_finish_ns,
    })
}

fn inspect_and_release(
    owner: &mut SharedGttMemorySessionV1,
    peer: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    mapping: Gfx942XgmiMappedDeviceMemoryV1,
    copy_bytes: usize,
    outer: u8,
    inner: u8,
) -> Result<bool, Box<dyn std::error::Error>> {
    let lease = owner
        .unmap_gfx942_device_memory_from_xgmi_peer(peer, route, mapping)
        .map_err(|failure| failure.error().to_string())?;
    let observed = owner.read_gfx942_xgmi_device_memory(&lease)?;
    let valid = validate_bytes(&observed, copy_bytes, outer, inner).is_ok();
    owner.release_gfx942_device_memory(lease)?;
    Ok(valid)
}

pub(super) fn run(
    unique_ids: [u64; 2],
    copy_bytes: usize,
    depth: usize,
    warmups: usize,
    samples: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let hot_pattern_round = warmups
        .checked_add(samples)
        .and_then(|n| n.checked_add(1))
        .ok_or("retained series pattern overflow")?;
    let bytes_per_batch = copy_bytes
        .checked_mul(depth)
        .ok_or("retained series bytes overflow")?;
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
    let mut forward_root = Gfx942NativeXgmiSdmaQueueCreationRootV1::new();
    let mut reverse_root = Gfx942NativeXgmiSdmaQueueCreationRootV1::new();
    let mut forward_queue =
        Gfx942NativeXgmiSdmaQueueV1::create(&mut left, &mut right, forward, &mut forward_root)?;
    let mut reverse_queue =
        Gfx942NativeXgmiSdmaQueueV1::create(&mut right, &mut left, reverse, &mut reverse_root)?;
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
    prepare_round(
        &mut left,
        &mut right,
        forward,
        &mut forward_pairs,
        copy_bytes,
        hot_pattern_round,
        None,
        0,
        0x17,
        0xa5,
    )?;
    prepare_round(
        &mut right,
        &mut left,
        reverse,
        &mut reverse_pairs,
        copy_bytes,
        hot_pattern_round,
        None,
        1,
        0x71,
        0x5a,
    )?;
    let mut forward_samples = series(
        &mut forward_queue,
        &mut left,
        &mut right,
        &mut forward_pairs,
        u32::try_from(copy_bytes)?,
        warmups,
        samples,
    )?;
    let mut reverse_samples = series(
        &mut reverse_queue,
        &mut right,
        &mut left,
        &mut reverse_pairs,
        u32::try_from(copy_bytes)?,
        warmups,
        samples,
    )?;
    if forward_pairs.len() != depth || reverse_pairs.len() != depth {
        return Err("retained series final mapping roster mismatch".into());
    }
    let mut invalid = 0;
    for (slot, pair) in forward_pairs.into_iter().enumerate() {
        let expected = pattern(hot_pattern_round, slot, 0);
        invalid += usize::from(!inspect_and_release(
            &mut left,
            &mut right,
            forward,
            pair.source,
            copy_bytes,
            0x17,
            expected,
        )?);
        invalid += usize::from(!inspect_and_release(
            &mut right,
            &mut left,
            forward,
            pair.destination,
            copy_bytes,
            0xa5,
            expected,
        )?);
    }
    for (slot, pair) in reverse_pairs.into_iter().enumerate() {
        let expected = pattern(hot_pattern_round, slot, 1);
        invalid += usize::from(!inspect_and_release(
            &mut right,
            &mut left,
            reverse,
            pair.source,
            copy_bytes,
            0x71,
            expected,
        )?);
        invalid += usize::from(!inspect_and_release(
            &mut left,
            &mut right,
            reverse,
            pair.destination,
            copy_bytes,
            0x5a,
            expected,
        )?);
    }
    reverse_queue.destroy_and_release(&mut right, &mut left)?;
    forward_queue.destroy_and_release(&mut left, &mut right)?;
    if invalid != 0 {
        return Err(
            format!("retained series payload/canary mismatch in {invalid} allocations").into(),
        );
    }
    forward_samples.copies.sort_unstable();
    reverse_samples.copies.sort_unstable();
    let f50 = percentile(&forward_samples.copies, 1, 2).ok_or("missing forward p50")?;
    let f95 = percentile(&forward_samples.copies, 19, 20).ok_or("missing forward p95")?;
    let r50 = percentile(&reverse_samples.copies, 1, 2).ok_or("missing reverse p50")?;
    let r95 = percentile(&reverse_samples.copies, 19, 20).ok_or("missing reverse p95")?;
    if [
        f50,
        r50,
        forward_samples.scope_entry_ns,
        forward_samples.scope_finish_ns,
        reverse_samples.scope_entry_ns,
        reverse_samples.scope_finish_ns,
    ]
    .contains(&0)
    {
        return Err("zero retained series duration".into());
    }
    print!(
        "validation=final-readback forward_samples={} reverse_samples={} ",
        forward_samples.copies.len(),
        reverse_samples.copies.len()
    );
    println!(
        "backend=kfd schema=fe2o3.xgmi-peer-retained-pair-series-benchmark.v1 surface=native-api measurement=persistent-series qualification_profile={} qualification_policy_sha256={} environment_assumption=reviewed-mi300x-amdgpu61613-ordinary-lifetime unique_ids={:016x},{:016x} gpu_ids={},{} target=gfx942:xnack- bytes={} depth={} queue_depth={} batch_size={} outstanding_depth={} warmups={} samples={} mapping_lifetime=directional-retained-pair prime_batches=1 direction=forward-series-then-reverse-series engine_parallelism=ordered-single-sdma peer_access=topology-xgmi doorbells_per_batch=1 progress=explicit-exact-roster-native-wait timing=native-enqueue-through-paired-operational-completion operational_fences=inside-samples lifetime_setup=outside-samples lifetime_finish=outside-samples scopes=2 forward_engine={} reverse_engine={} forward_scope_entry_ns={} forward_scope_finish_ns={} reverse_scope_entry_ns={} reverse_scope_finish_ns={} forward_p50_ns={} forward_p95_ns={} forward_p50_GBps={:.3} reverse_p50_ns={} reverse_p95_ns={} reverse_p50_GBps={:.3} canaries=pass teardown=explicit",
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1,
        GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1,
        unique_ids[0],
        unique_ids[1],
        gpu_ids[0],
        gpu_ids[1],
        copy_bytes,
        depth,
        depth,
        depth,
        depth,
        warmups,
        samples,
        forward.recommended_engine_id(),
        reverse.recommended_engine_id(),
        forward_samples.scope_entry_ns,
        forward_samples.scope_finish_ns,
        reverse_samples.scope_entry_ns,
        reverse_samples.scope_finish_ns,
        f50,
        f95,
        bytes_per_batch as f64 / f50 as f64,
        r50,
        r95,
        bytes_per_batch as f64 / r50 as f64,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_samples_exclude_exact_prime_and_warmups() {
        for warmups in [0, 2] {
            for samples in [1, 3] {
                let mut calls = 0;
                let result = collect_series(warmups, samples, || {
                    calls += 1;
                    Ok::<_, ()>(calls as u128)
                })
                .unwrap();
                assert_eq!(calls, 1 + warmups + samples);
                assert_eq!(
                    result,
                    ((warmups + 2)..=(warmups + samples + 1))
                        .map(|n| n as u128)
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn series_failure_keeps_first_error_and_stops_later_copies() {
        for failure_at in 1..=6 {
            let mut calls = 0;
            let result = collect_series(2, 3, || {
                calls += 1;
                if calls == failure_at {
                    Err(failure_at)
                } else {
                    Ok(calls as u128)
                }
            });
            assert_eq!(result, Err(failure_at));
            assert_eq!(calls, failure_at);
        }
    }

    #[test]
    fn series_payload_and_canaries_reject_every_changed_region() {
        let bytes = 7;
        let mut contents = vec![0x17; bytes + 2 * CANARY_BYTES];
        contents[CANARY_BYTES..CANARY_BYTES + bytes].fill(pattern(6, 2, 1));
        assert!(validate_bytes(&contents, bytes, 0x17, pattern(6, 2, 1)).is_ok());
        for index in [
            0,
            CANARY_BYTES,
            CANARY_BYTES + bytes - 1,
            contents.len() - 1,
        ] {
            contents[index] ^= 1;
            assert!(validate_bytes(&contents, bytes, 0x17, pattern(6, 2, 1)).is_err());
            contents[index] ^= 1;
        }
    }
}
