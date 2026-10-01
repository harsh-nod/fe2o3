//! Closed cadence experiment, separate from all accepted ordinary/diagnostic schemas.

use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, DeviceSelector, GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1,
    GFX942_XGMI_RETAINED_PAIR_PROFILE_V1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryUnmappedV1,
    Gfx942NativeXgmiSdmaQueueCreationRootV1, Gfx942NativeXgmiSdmaQueueV1,
    Gfx942XgmiCopyCallDiagnosticsV1, Gfx942XgmiMapRecoveryV1, Gfx942XgmiMappedDeviceMemoryV1,
    Gfx942XgmiRetainedPairEnvironmentAssumptionV1, Gfx942XgmiRetainedWaitCadenceV1,
    Gfx942XgmiRetainedWaitCpuV1, Gfx942XgmiRetainedWaitDiagnosticsV1, Gfx942XgmiSdmaCopyRequestV1,
    OpenedKfd, SharedGttMemorySessionV1, topology::Gfx942XgmiRouteV1,
};
use std::time::{Duration, Instant};

type Error = Box<dyn std::error::Error>;
const BYTES: usize = 1_048_576;
const CANARY: usize = 32;
const WARMUPS: usize = 2;
const SAMPLES: usize = 10;

struct Pair {
    source: Gfx942XgmiMappedDeviceMemoryV1,
    destination: Gfx942XgmiMappedDeviceMemoryV1,
}

struct Sample {
    elapsed_ns: u128,
    diagnostic: Option<(
        Gfx942XgmiCopyCallDiagnosticsV1,
        Gfx942XgmiRetainedWaitDiagnosticsV1,
    )>,
}

struct Series {
    entry_ns: u128,
    finish_ns: u128,
    samples: Vec<Sample>,
}

fn pattern(slot: usize, direction: usize) -> u8 {
    (((WARMUPS + SAMPLES + 1) * 67 + slot * 29 + direction * 101 + 1) % 251 + 1) as u8
}

fn admit(unique_id: u64) -> Result<CheckedGfx942XnackMinusDevice, Error> {
    Ok(OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?)
}

fn gpu_id(device: &CheckedGfx942XnackMinusDevice, unique_id: u64) -> Result<u32, Error> {
    let node = device
        .topology_snapshot()
        .topology()
        .gpu_nodes()
        .iter()
        .find(|node| node.unique_id() == unique_id)
        .ok_or("missing diagnostic endpoint")?;
    Ok(u32::try_from(node.gpu_id())?)
}

fn map(
    owner: &mut SharedGttMemorySessionV1,
    peer: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
) -> Result<Gfx942XgmiMappedDeviceMemoryV1, Error> {
    match owner.map_gfx942_device_memory_for_xgmi_peer(peer, route, lease) {
        Ok(mapping) => Ok(mapping),
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            match recovery {
                Gfx942XgmiMapRecoveryV1::Unmapped(lease) => {
                    owner.release_gfx942_device_memory(lease)?
                }
                Gfx942XgmiMapRecoveryV1::PartiallyMapped(mapping) => {
                    let lease = owner
                        .unmap_gfx942_device_memory_from_xgmi_peer(peer, route, mapping)
                        .map_err(|cleanup| {
                            format!(
                                "diagnostic map failed ({error}); indeterminate cleanup ({})",
                                cleanup.error()
                            )
                        })?;
                    owner.release_gfx942_device_memory(lease)?;
                }
            }
            Err(error.into())
        }
    }
}

fn contents(canary: u8, value: u8) -> Vec<u8> {
    let mut bytes = vec![canary; BYTES + 2 * CANARY];
    bytes[CANARY..CANARY + BYTES].fill(value);
    bytes
}

fn allocate_pair(
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    source_canary: u8,
    destination_canary: u8,
    value: u8,
) -> Result<Pair, Error> {
    let source_lease =
        source.allocate_gfx942_xgmi_device_memory((BYTES + 2 * CANARY) as u64, 4096)?;
    source.write_gfx942_xgmi_device_memory(&source_lease, &contents(source_canary, value))?;
    let destination_lease =
        destination.allocate_gfx942_xgmi_device_memory((BYTES + 2 * CANARY) as u64, 4096)?;
    destination.write_gfx942_xgmi_device_memory(
        &destination_lease,
        &contents(destination_canary, destination_canary),
    )?;
    Ok(Pair {
        source: map(source, destination, route, source_lease)?,
        destination: map(destination, source, route, destination_lease)?,
    })
}

fn prepare(
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    pairs: &mut Vec<Pair>,
    direction: usize,
    source_canary: u8,
    destination_canary: u8,
) -> Result<(), Error> {
    let mut prepared = Vec::with_capacity(pairs.len());
    for (slot, pair) in std::mem::take(pairs).into_iter().enumerate() {
        let source_lease = source
            .unmap_gfx942_device_memory_from_xgmi_peer(destination, route, pair.source)
            .map_err(|failure| failure.error().to_string())?;
        let destination_lease = destination
            .unmap_gfx942_device_memory_from_xgmi_peer(source, route, pair.destination)
            .map_err(|failure| failure.error().to_string())?;
        source.write_gfx942_xgmi_device_memory(
            &source_lease,
            &contents(source_canary, pattern(slot, direction)),
        )?;
        destination.write_gfx942_xgmi_device_memory(
            &destination_lease,
            &contents(destination_canary, pattern(slot, direction) ^ 0xff),
        )?;
        prepared.push(Pair {
            source: map(source, destination, route, source_lease)?,
            destination: map(destination, source, route, destination_lease)?,
        });
    }
    *pairs = prepared;
    Ok(())
}

fn series<const PROFILED: bool>(
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    pairs: &mut Vec<Pair>,
    cadence: Gfx942XgmiRetainedWaitCadenceV1,
) -> Result<Series, Error> {
    let depth = pairs.len();
    let entry = Instant::now();
    let mut scope = queue.begin_ordinary_retained_pair_v1(
        source,
        destination,
        Gfx942XgmiRetainedPairEnvironmentAssumptionV1::ReviewedMi300xAmdgpu61613OrdinaryLifetime,
    )?;
    let entry_ns = entry.elapsed().as_nanos();
    let mut samples = Vec::with_capacity(SAMPLES);
    for round in 0..1 + WARMUPS + SAMPLES {
        let requests = pairs
            .drain(..)
            .map(|pair| {
                Gfx942XgmiSdmaCopyRequestV1::new(
                    pair.source,
                    CANARY as u64,
                    pair.destination,
                    CANARY as u64,
                    BYTES as u32,
                )
            })
            .collect();
        let started = Instant::now();
        let (completed, diagnostic) = if PROFILED {
            let (tickets, submit) = scope
                .submit_batch_diagnostic_v1(requests)
                .map_err(|failure| failure.error().to_string())?;
            let (completed, wait) = scope
                .wait_batch_for_cadence_diagnostic_v1(tickets, Duration::from_secs(30), cadence)
                .map_err(|failure| failure.error().to_string())?;
            (completed, Some((submit, wait)))
        } else {
            let tickets = scope
                .submit_batch(requests)
                .map_err(|failure| failure.error().to_string())?;
            let completed = scope
                .wait_batch_for_cadence_experiment_v1(tickets, Duration::from_secs(30), cadence)
                .map_err(|failure| failure.error().to_string())?;
            (completed, None)
        };
        let elapsed_ns = started.elapsed().as_nanos();
        if completed.len() != depth {
            return Err("diagnostic completed roster mismatch".into());
        }
        for copy in completed.into_copies() {
            if copy.copy_bytes() != BYTES as u32 {
                return Err("diagnostic completed size mismatch".into());
            }
            let (source, destination) = copy.into_mappings();
            pairs.push(Pair {
                source,
                destination,
            });
        }
        if round > WARMUPS {
            samples.push(Sample {
                elapsed_ns,
                diagnostic,
            });
        }
    }
    let finish = Instant::now();
    scope.finish()?;
    Ok(Series {
        entry_ns,
        finish_ns: finish.elapsed().as_nanos(),
        samples,
    })
}

fn inspect(
    owner: &mut SharedGttMemorySessionV1,
    peer: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    mapping: Gfx942XgmiMappedDeviceMemoryV1,
    canary: u8,
    value: u8,
) -> Result<bool, Error> {
    let lease = owner
        .unmap_gfx942_device_memory_from_xgmi_peer(peer, route, mapping)
        .map_err(|failure| failure.error().to_string())?;
    let observed = owner.read_gfx942_xgmi_device_memory(&lease)?;
    let valid = observed.as_ref() == contents(canary, value).as_slice();
    owner.release_gfx942_device_memory(lease)?;
    Ok(valid)
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

fn print_sample(direction: &str, index: usize, sample: &Sample) {
    let Some((submit, wait)) = &sample.diagnostic else {
        println!(
            "row=sample direction={} index={} elapsed_ns={}",
            direction, index, sample.elapsed_ns
        );
        return;
    };
    let (cpu_status, cpu_ns, voluntary, involuntary) = match wait.cpu {
        Gfx942XgmiRetainedWaitCpuV1::Available {
            thread_cpu_ns,
            voluntary_context_switches,
            involuntary_context_switches,
        } => (
            "available",
            Some(thread_cpu_ns),
            Some(voluntary_context_switches),
            Some(involuntary_context_switches),
        ),
        Gfx942XgmiRetainedWaitCpuV1::Unavailable => ("unavailable", None, None, None),
        Gfx942XgmiRetainedWaitCpuV1::Invalid => ("invalid", None, None, None),
    };
    let counters = wait.counters;
    println!(
        concat!(
            "row=sample direction={} index={} elapsed_ns={} ",
            "submit_open_ns={} prepare_ns={} publish_ns={} submit_close_ns={} submit_total_ns={} ",
            "wait_open_ns={} validation_ns={} scan_ns={} retirement_ns={} wait_close_ns={} wait_total_ns={} ",
            "first_observed_ns={} all_observed_ns={} counters_status={} rounds={} observations={} ",
            "spins={} yields={} sleeps={} requested_sleep_ns={} max_requested_sleep_ns={} ",
            "cpu_status={} thread_cpu_ns={} voluntary_switches={} involuntary_switches={}"
        ),
        direction,
        index,
        sample.elapsed_ns,
        optional(submit.opening_currentness_ns),
        optional(submit.preparation_ns),
        optional(submit.native_call_ns),
        optional(submit.closing_currentness_ns),
        optional(submit.total_ns),
        optional(wait.opening_currentness_ns),
        optional(wait.validation_ns),
        optional(wait.scan_ns),
        optional(wait.retirement_ns),
        optional(wait.closing_currentness_ns),
        optional(wait.total_ns),
        optional(wait.first_observed_completion_ns),
        optional(wait.all_observed_completion_ns),
        if counters.is_some() {
            "available"
        } else {
            "invalid"
        },
        optional(counters.map(|v| v.scan_rounds)),
        optional(counters.map(|v| v.completion_observations)),
        optional(counters.map(|v| v.spin_pauses)),
        optional(counters.map(|v| v.yield_pauses)),
        optional(counters.map(|v| v.sleep_pauses)),
        optional(counters.map(|v| v.requested_sleep_ns)),
        optional(counters.map(|v| v.max_requested_sleep_ns)),
        cpu_status,
        optional(cpu_ns),
        optional(voluntary),
        optional(involuntary)
    );
}

fn main() -> Result<(), Error> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 || args[5] != "--reviewed-mi300x-retained-cadence-experiment" {
        return Err("usage: kfd-xgmi-retained-cadence-experiment <0xunique-id-0> <0xunique-id-1> <1|16|32> <ordinary-1ms|ceiling-25us> <ordinary|profiled> --reviewed-mi300x-retained-cadence-experiment".into());
    }
    let cadence = match args[3].as_str() {
        "ordinary-1ms" => Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms,
        "ceiling-25us" => Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us,
        _ => return Err("unknown closed cadence".into()),
    };
    let profiled = match args[4].as_str() {
        "ordinary" => false,
        "profiled" => true,
        _ => return Err("unknown instrumentation mode".into()),
    };
    let parse_id = |value: &str| -> Result<u64, Error> {
        Ok(u64::from_str_radix(
            value.strip_prefix("0x").ok_or("expected hex unique ID")?,
            16,
        )?)
    };
    let ids = [parse_id(&args[0])?, parse_id(&args[1])?];
    let depth: usize = args[2].parse()?;
    if ids[0] == ids[1] || ![1, 16, 32].contains(&depth) {
        return Err("diagnostic controls out of range".into());
    }
    let left_device = admit(ids[0])?;
    let right_device = admit(ids[1])?;
    let gpu_ids = [gpu_id(&left_device, ids[0])?, gpu_id(&left_device, ids[1])?];
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
            &mut left, &mut right, forward, 0x17, 0xa5, 0x35,
        )?);
        reverse_pairs.push(allocate_pair(
            &mut right, &mut left, reverse, 0x71, 0x5a, 0xca,
        )?);
    }
    prepare(
        &mut left,
        &mut right,
        forward,
        &mut forward_pairs,
        0,
        0x17,
        0xa5,
    )?;
    prepare(
        &mut right,
        &mut left,
        reverse,
        &mut reverse_pairs,
        1,
        0x71,
        0x5a,
    )?;
    let (forward_samples, reverse_samples) = if profiled {
        (
            series::<true>(
                &mut forward_queue,
                &mut left,
                &mut right,
                &mut forward_pairs,
                cadence,
            )?,
            series::<true>(
                &mut reverse_queue,
                &mut right,
                &mut left,
                &mut reverse_pairs,
                cadence,
            )?,
        )
    } else {
        (
            series::<false>(
                &mut forward_queue,
                &mut left,
                &mut right,
                &mut forward_pairs,
                cadence,
            )?,
            series::<false>(
                &mut reverse_queue,
                &mut right,
                &mut left,
                &mut reverse_pairs,
                cadence,
            )?,
        )
    };
    if forward_pairs.len() != depth || reverse_pairs.len() != depth {
        return Err("diagnostic final roster mismatch".into());
    }
    let mut invalid = 0;
    for (slot, pair) in forward_pairs.into_iter().enumerate() {
        invalid += usize::from(!inspect(
            &mut left,
            &mut right,
            forward,
            pair.source,
            0x17,
            pattern(slot, 0),
        )?);
        invalid += usize::from(!inspect(
            &mut right,
            &mut left,
            forward,
            pair.destination,
            0xa5,
            pattern(slot, 0),
        )?);
    }
    for (slot, pair) in reverse_pairs.into_iter().enumerate() {
        invalid += usize::from(!inspect(
            &mut right,
            &mut left,
            reverse,
            pair.source,
            0x71,
            pattern(slot, 1),
        )?);
        invalid += usize::from(!inspect(
            &mut left,
            &mut right,
            reverse,
            pair.destination,
            0x5a,
            pattern(slot, 1),
        )?);
    }
    reverse_queue.destroy_and_release(&mut right, &mut left)?;
    forward_queue.destroy_and_release(&mut left, &mut right)?;
    if invalid != 0 {
        return Err("diagnostic payload/canary mismatch".into());
    }
    println!(
        concat!(
            "row=summary schema=fe2o3.xgmi-retained-wait-cadence.v1 backend=kfd authority=none ",
            "profile={} policy_sha256={} environment=reviewed-mi300x-amdgpu61613-ordinary-lifetime ",
            "unique_ids={:016x},{:016x} gpu_ids={},{} bytes={} depth={} warmups={} samples={} prime_batches=1 ",
            "direction=forward-series-then-reverse-series wait_policy=explicit-cadence-experiment engine_parallelism=ordered-single-sdma ",
            "forward_engine={} reverse_engine={} forward_entry_ns={} forward_finish_ns={} reverse_entry_ns={} reverse_finish_ns={} ",
            "validation=final-readback canaries=pass teardown=explicit timing={} mode={} cadence={} sleep_ceiling_ns={}"
        ),
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1,
        GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1,
        ids[0],
        ids[1],
        gpu_ids[0],
        gpu_ids[1],
        BYTES,
        depth,
        WARMUPS,
        SAMPLES,
        forward.recommended_engine_id(),
        reverse.recommended_engine_id(),
        forward_samples.entry_ns,
        forward_samples.finish_ns,
        reverse_samples.entry_ns,
        reverse_samples.finish_ns,
        if profiled {
            "instrumented-host-only"
        } else {
            "native-enqueue-through-paired-operational-completion"
        },
        if profiled { "profiled" } else { "ordinary" },
        cadence.name(),
        cadence.sleep_ceiling_ns()
    );
    for (direction, series) in [("forward", forward_samples), ("reverse", reverse_samples)] {
        for (index, sample) in series.samples.iter().enumerate() {
            print_sample(direction, index, sample);
        }
    }
    Ok(())
}
