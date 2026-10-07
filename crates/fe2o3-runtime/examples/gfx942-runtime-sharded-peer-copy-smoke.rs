//! Fixed-total, disjoint peer-copy shards driven by one current-thread owner.

use std::time::{Duration, Instant};

use fe2o3_kfd::Gfx942ComputeXgmiPacketPlanV1;
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeAuthorityRequestV1, KfdRuntimeBackendErrorV1,
    KfdRuntimeLaunchAuthorityV1, RuntimeAccessV1, RuntimeAllocationIdV1,
    RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDrainOutcomeV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOperationFutureV1, RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1,
    RuntimeAsyncProgressHandleV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimeStreamIdV1, RuntimeStreamObservationV1, RuntimeSubmissionV1,
};
use sha2::{Digest, Sha256};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const TOTAL_BYTES: u64 = 64 * 1024 * 1024 + 37;
const ROUNDS: usize = 2;
const WAIT: Duration = Duration::from_secs(30);
const DRAIN_TICKS: usize = 256;
const USAGE: &str =
    "usage: gfx942-runtime-sharded-peer-copy-smoke <0xunique-id> <0xunique-id> [up to eight total]";

#[derive(Debug)]
struct NoCompute;

// SAFETY: No invocation is authorized, regardless of its artifact or arguments.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Shard {
    offset: u64,
    bytes: u64,
}

#[derive(Clone, Copy)]
struct Buffers {
    stream: RuntimeStreamIdV1,
    outgoing_host: RuntimeAllocationIdV1,
    outgoing: RuntimeAllocationIdV1,
    incoming_host: RuntimeAllocationIdV1,
    incoming: RuntimeAllocationIdV1,
}

fn unique_ids(arguments: &[String]) -> ResultV1<Vec<u64>> {
    if !(2..=8).contains(&arguments.len()) {
        return Err(USAGE.into());
    }
    let mut ids = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let hex = argument
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        let id = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
        if id == 0 || ids.contains(&id) {
            return Err(USAGE.into());
        }
        ids.push(id);
    }
    Ok(ids)
}

fn partition(total: u64, devices: usize) -> ResultV1<Vec<Shard>> {
    if !(2..=8).contains(&devices) || total < devices as u64 || total > TOTAL_BYTES {
        return Err("invalid bounded shard extent or device count".into());
    }
    let quotient = total / devices as u64;
    let remainder = total % devices as u64;
    let mut shards = Vec::with_capacity(devices);
    let mut offset = 0_u64;
    for index in 0..devices {
        let bytes = quotient + u64::from((index as u64) < remainder);
        let end = offset.checked_add(bytes).ok_or("shard extent overflow")?;
        if bytes == 0 || end > total || Gfx942ComputeXgmiPacketPlanV1::new(bytes).is_none() {
            return Err("invalid native shard extent".into());
        }
        shards.push(Shard { offset, bytes });
        offset = end;
    }
    if offset != total {
        return Err("shards do not cover the fixed total".into());
    }
    Ok(shards)
}

fn pattern(index: u64, round: usize) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8 ^ (round as u8).wrapping_mul(73)
}

fn expected_digest(shards: &[Shard], round: usize, sentinel: bool) -> [u8; 32] {
    let mut digest = Sha256::new();
    let mut block = [0_u8; 8192];
    for shard in shards {
        let mut offset = 0;
        while offset < shard.bytes {
            let count = (shard.bytes - offset).min(block.len() as u64) as usize;
            for (index, byte) in block[..count].iter_mut().enumerate() {
                let value = pattern(shard.offset + offset + index as u64, round);
                *byte = if sentinel { !value } else { value };
            }
            digest.update(&block[..count]);
            offset += count as u64;
        }
    }
    digest.finalize().into()
}

fn hex_digest(digest: [u8; 32]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in digest {
        write!(result, "{byte:02x}").expect("String formatting");
    }
    result
}

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let detail = format!("stage={stage} {error:?}");
    eprintln!("sharded peer-copy diagnostic: {detail}");
    detail
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    bytes: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: bytes,
    }
}

fn command<T: Send + 'static>(
    engine: &mut Engine,
    handle: &Handle,
    stage: &str,
    operation: impl FnOnce(&mut Context) -> ResultV1<T> + Send + 'static,
) -> ResultV1<T> {
    let mut future = Box::pin(
        handle
            .observer()
            .enqueue_with_context(operation)
            .map_err(|error| failure(stage, error))?,
    );
    engine
        .drive_until_ready(future.as_mut(), Instant::now() + WAIT)
        .map_err(|error| failure(stage, error))?
        .map_err(|error| failure(stage, error))?
}

fn finish_operations<A>(
    engine: &mut Engine,
    operations: Vec<RuntimeAsyncOperationFutureV1<A, KfdRuntimeBackendErrorV1>>,
    stage: &str,
) -> ResultV1<Vec<RuntimeSubmissionV1<A>>> {
    // Every future is already enqueued. One absolute deadline covers the whole group.
    let deadline = Instant::now() + WAIT;
    let mut submissions = Vec::with_capacity(operations.len());
    for operation in operations {
        let mut operation = Box::pin(operation);
        let result = engine
            .drive_until_ready(operation.as_mut(), deadline)
            .map_err(|error| failure(stage, error))?
            .map_err(|error| failure(stage, error))?;
        match result.observation {
            Ok(RuntimeCompletionStatusV1::Succeeded) if result.rejected_observations == 0 => {}
            status => return Err(failure(stage, (status, result.rejected_observations))),
        }
        submissions.push(
            result
                .submission
                .ok_or_else(|| failure(stage, "successful operation lost its submission"))?,
        );
    }
    Ok(submissions)
}

fn write_round_hosts(
    context: &mut Context,
    buffers: &[Buffers],
    shards: &[Shard],
    round: usize,
) -> ResultV1<()> {
    for (index, shard) in shards.iter().enumerate() {
        let mut contents: Vec<_> = (0..shard.bytes)
            .map(|offset| pattern(shard.offset + offset, round))
            .collect();
        context
            .write_allocation(buffers[index].outgoing_host, 0, &contents)
            .map_err(|error| failure("source-host-write", error))?;
        for byte in &mut contents {
            *byte = !*byte;
        }
        context
            .write_allocation(
                buffers[(index + 1) % buffers.len()].incoming_host,
                0,
                &contents,
            )
            .map_err(|error| failure("sentinel-host-write", error))?;
    }
    Ok(())
}

fn verify_shard(
    context: &mut Context,
    allocation: RuntimeAllocationIdV1,
    shard: Shard,
    round: usize,
    sentinel: bool,
    digest: &mut Sha256,
) -> ResultV1<()> {
    let mut observed = vec![0; shard.bytes as usize];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure("full-shard-readback", error))?;
    for (index, &byte) in observed.iter().enumerate() {
        let global = shard.offset + index as u64;
        let expected = pattern(global, round);
        let expected = if sentinel { !expected } else { expected };
        if byte != expected {
            return Err(failure(
                "full-shard-mismatch",
                format!("global_offset={global} observed={byte} expected={expected}"),
            ));
        }
    }
    digest.update(&observed);
    Ok(())
}

fn verify_round(
    context: &mut Context,
    buffers: &[Buffers],
    shards: &[Shard],
    round: usize,
    sentinel: bool,
) -> ResultV1<[u8; 32]> {
    let mut source = Sha256::new();
    let mut destination = Sha256::new();
    for (index, &shard) in shards.iter().enumerate() {
        verify_shard(
            context,
            buffers[index].outgoing,
            shard,
            round,
            false,
            &mut source,
        )?;
        verify_shard(
            context,
            buffers[(index + 1) % buffers.len()].incoming,
            shard,
            round,
            sentinel,
            &mut destination,
        )?;
    }
    let source: [u8; 32] = source.finalize().into();
    let destination: [u8; 32] = destination.finalize().into();
    if source != expected_digest(shards, round, false)
        || destination != expected_digest(shards, round, sentinel)
    {
        return Err(failure(
            "global-digest",
            "full ordered shard digest mismatch",
        ));
    }
    Ok(source)
}

fn run(ids: Vec<u64>) -> ResultV1<()> {
    let shards = partition(TOTAL_BYTES, ids.len())?;
    let native_packets: usize = shards
        .iter()
        .map(|shard| {
            Gfx942ComputeXgmiPacketPlanV1::new(shard.bytes)
                .unwrap()
                .count()
        })
        .sum();
    let authorities = ids
        .iter()
        .map(|&id| {
            (
                id,
                Box::new(NoCompute) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
            )
        })
        .collect();
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1(authorities)
            .map_err(|error| failure("device-admission", error))?;
    let mut context = Context::open(backend).map_err(|error| failure("context-open", error))?;
    if context.devices().len() != ids.len()
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "device-roster",
            "exact admitted gfx942:xnack- roster required",
        ));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut buffers = Vec::with_capacity(ids.len());
    for (index, device) in devices.into_iter().enumerate() {
        let incoming = shards[(index + ids.len() - 1) % ids.len()];
        let mut allocate = |kind, bytes| {
            context
                .allocate(device, kind, bytes, 4096)
                .map_err(|error| failure("allocation", error))
        };
        let outgoing_host = allocate(RuntimeMemoryKindV1::HostVisible, shards[index].bytes)?;
        let outgoing = allocate(RuntimeMemoryKindV1::DeviceLocal, shards[index].bytes)?;
        let incoming_host = allocate(RuntimeMemoryKindV1::HostVisible, incoming.bytes)?;
        let incoming = allocate(RuntimeMemoryKindV1::DeviceLocal, incoming.bytes)?;
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("stream-create", error))?;
        buffers.push(Buffers {
            stream,
            outgoing_host,
            outgoing,
            incoming_host,
            incoming,
        });
    }
    // Commands first create every driver; the poll budget admits all before any flush.
    let capacity = 2 * ids.len();
    let config = RuntimeAsyncEngineConfigV1::new(
        capacity,
        capacity,
        capacity,
        capacity,
        Duration::from_micros(50),
    )
    .and_then(|config| config.with_reply_capacity(capacity + 2))
    .map_err(|error| failure("owner-config", error))?;
    let progress = RuntimeAsyncProgressConfigV1::new(ids.len(), 1)
        .map_err(|error| failure("owner-progress-config", error))?;
    let (mut engine, handle) =
        Engine::new_with_progress(|| Ok::<_, String>(context), config, progress)
            .map_err(|error| failure("owner-open", error))?;
    let mut digests = Vec::with_capacity(ROUNDS);
    for round in 0..ROUNDS {
        let (owned_buffers, owned_shards) = (buffers.clone(), shards.clone());
        command(&mut engine, &handle, "host-setup", move |context| {
            write_round_hosts(context, &owned_buffers, &owned_shards, round)
        })?;
        let mut uploads = Vec::with_capacity(capacity);
        for (index, buffer) in buffers.iter().enumerate() {
            let incoming = shards[(index + ids.len() - 1) % ids.len()];
            for (host, data, bytes) in [
                (buffer.outgoing_host, buffer.outgoing, shards[index].bytes),
                (buffer.incoming_host, buffer.incoming, incoming.bytes),
            ] {
                uploads.push(
                    handle
                        .copy_async(
                            buffer.stream,
                            region(host, RuntimeAccessV1::Read, bytes),
                            region(data, RuntimeAccessV1::Write, bytes),
                            Vec::new(),
                        )
                        .map_err(|error| failure("upload-enqueue", error))?,
                );
            }
        }
        // Owner progress repeatedly flushes and polls all setup windows, not one wait.
        let uploads = finish_operations(&mut engine, uploads, "upload-progress")?;
        let (owned_buffers, owned_shards) = (buffers.clone(), shards.clone());
        command(&mut engine, &handle, "initial-readback", move |context| {
            for upload in uploads {
                context
                    .release_submission(upload)
                    .map_err(|error| failure("upload-release", error))?;
            }
            verify_round(context, &owned_buffers, &owned_shards, round, true)?;
            let observed = context.backend().completed_compute_xgmi_copies_v1();
            if observed != (round * owned_buffers.len()) as u64 {
                return Err(failure("native-count-before", observed));
            }
            Ok(())
        })?;

        let mut copies = Vec::with_capacity(ids.len());
        for (index, shard) in shards.iter().enumerate() {
            let destination = &buffers[(index + 1) % buffers.len()];
            copies.push(
                handle
                    .peer_copy(
                        destination.stream,
                        region(buffers[index].outgoing, RuntimeAccessV1::Read, shard.bytes),
                        region(destination.incoming, RuntimeAccessV1::Write, shard.bytes),
                        Vec::new(),
                    )
                    .map_err(|error| failure("peer-enqueue", error))?,
            );
        }
        let copies = finish_operations(&mut engine, copies, "peer-progress")?;
        let (owned_buffers, owned_shards) = (buffers.clone(), shards.clone());
        let digest = command(&mut engine, &handle, "final-readback", move |context| {
            let observed = context.backend().completed_compute_xgmi_copies_v1();
            if observed != ((round + 1) * owned_buffers.len()) as u64 {
                return Err(failure("native-count-after", observed));
            }
            for copy in copies {
                context
                    .release_submission(copy)
                    .map_err(|error| failure("peer-release", error))?;
            }
            verify_round(context, &owned_buffers, &owned_shards, round, false)
        })?;
        digests.push(hex_digest(digest));
    }
    let owned_buffers = buffers.clone();
    let native_copies = command(&mut engine, &handle, "logical-cleanup", move |context| {
        let count = context.backend().completed_compute_xgmi_copies_v1();
        for buffer in owned_buffers.into_iter().rev() {
            for allocation in [
                buffer.incoming,
                buffer.incoming_host,
                buffer.outgoing,
                buffer.outgoing_host,
            ] {
                context
                    .release_allocation(allocation)
                    .map_err(|error| failure("allocation-release", error))?;
            }
            context
                .destroy_stream(buffer.stream)
                .map_err(|error| failure("stream-destroy", error))?;
        }
        Ok(count)
    })?;
    let mut draining = Box::pin(
        handle
            .begin_drain(DRAIN_TICKS)
            .map_err(|error| failure("drain-admission", error))?,
    );
    let drained = engine.drive_until_ready(draining.as_mut(), Instant::now() + WAIT);
    let shutdown = engine.shutdown();
    let drained = drained
        .map_err(|error| failure("drain-drive", error))?
        .map_err(|error| failure("drain-result", error))?;
    if drained.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || !drained.queued_commands_exhausted
        || drained.operations_remaining != 0
        || drained.graph_active
        || drained.retained_submissions != RuntimeStreamObservationV1::default()
        || drained.ticks == 0
        || drained.ticks > DRAIN_TICKS
    {
        return Err(failure("completed-group-drain", drained));
    }
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(failure("native-shutdown", shutdown));
    }
    let ids = ids
        .iter()
        .map(|id| format!("0x{id:016x}"))
        .collect::<Vec<_>>()
        .join(",");
    let ranges = shards
        .iter()
        .map(|shard| format!("{}:{}", shard.offset, shard.bytes))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS schema=fe2o3.sharded-peer-copy-smoke.v1 authority=production-deny-all transport=NATIVE-XGMI devices={} unique_ids={} rounds={} total_bytes_per_round={} total_copied_bytes={} shard_ranges={} peer_copies={} observed_native_copies={} native_packets={} allocations={} streams={} launches=0 readbacks={} submission=all-enqueued-before-drive source_unchanged=full-byte-pass destination_sentinel=full-byte-pass output=full-byte-pass coverage=exact-contiguous-disjoint round_sha256={} rounds_changed=true allocation_reuse=true drain=completed-group-only pending_group_drain=false physical_overlap=unmeasured cleanup=logical-and-native-explicit performance_acceptance=false formal_refinement=false",
        buffers.len(),
        ids,
        ROUNDS,
        TOTAL_BYTES,
        TOTAL_BYTES * ROUNDS as u64,
        ranges,
        buffers.len() * ROUNDS,
        native_copies,
        native_packets * ROUNDS,
        4 * buffers.len(),
        buffers.len(),
        4 * buffers.len() * ROUNDS,
        digests.join(","),
    );
    Ok(())
}

fn main() {
    let result = unique_ids(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run);
    if let Err(error) = result {
        eprintln!("sharded peer-copy smoke failed before complete cleanup: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_accepts_two_to_eight_distinct_explicit_ids_in_order() {
        for count in [2, 3, 5, 8] {
            let expected: Vec<_> = (1..=count).rev().collect();
            let arguments: Vec<_> = expected.iter().map(|id| format!("0x{id:x}")).collect();
            assert_eq!(unique_ids(&arguments).unwrap(), expected);
        }
    }

    #[test]
    fn cli_rejects_invalid_count_duplicates_zero_and_noncanonical_input() {
        for args in [
            vec![],
            vec!["0x1"],
            vec!["0x1"; 9],
            vec!["0x1", "0x01"],
            vec!["0x0", "0x2"],
            vec!["1", "0x2"],
            vec!["0X1", "0x2"],
            vec!["0x", "0x2"],
            vec!["0x+1", "0x2"],
            vec!["0xg", "0x2"],
            vec!["0x10000000000000000", "0x2"],
            vec!["--all", "0x1", "0x2"],
        ] {
            let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
            assert!(unique_ids(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn partition_covers_fixed_total_once_for_each_admitted_device_count() {
        for count in 2..=8 {
            for total in [count as u64, 257, TOTAL_BYTES] {
                let shards = partition(total, count).unwrap();
                assert_eq!(shards.len(), count);
                let mut cursor = 0;
                for (index, shard) in shards.iter().enumerate() {
                    assert_eq!(shard.offset, cursor);
                    assert_eq!(
                        shard.bytes,
                        total / count as u64 + u64::from((index as u64) < total % count as u64)
                    );
                    assert!(shard.bytes > 0);
                    cursor = shard.offset.checked_add(shard.bytes).unwrap();
                }
                assert_eq!(cursor, total);
                assert!(shards[0].bytes - shards[count - 1].bytes <= 1);
            }
        }
    }

    #[test]
    fn partition_rejects_zero_empty_shards_over_cap_and_invalid_rosters() {
        for (total, count) in [
            (0, 2),
            (1, 2),
            (7, 8),
            (TOTAL_BYTES + 1, 2),
            (u64::MAX, 2),
            (257, 0),
            (257, 1),
            (257, 9),
        ] {
            assert!(partition(total, count).is_err());
        }
    }

    #[test]
    fn global_patterns_and_digests_do_not_depend_on_partition_count() {
        for round in 0..ROUNDS {
            let expected: Vec<_> = (0..257).map(|index| pattern(index, round)).collect();
            let digest: [u8; 32] = Sha256::digest(&expected).into();
            for count in [2, 3, 5, 8] {
                let shards = partition(257, count).unwrap();
                assert_eq!(expected_digest(&shards, round, false), digest);
                let reconstructed: Vec<_> = shards
                    .iter()
                    .flat_map(|shard| {
                        (0..shard.bytes).map(|index| pattern(shard.offset + index, round))
                    })
                    .collect();
                assert_eq!(reconstructed, expected);
                assert_ne!(expected_digest(&shards, round, true), digest);
                for shard in shards.iter().skip(1) {
                    assert!(
                        (0..shard.bytes.min(16))
                            .any(|index| pattern(index, round)
                                != pattern(shard.offset + index, round))
                    );
                }
            }
        }
        for index in 0..257 {
            assert_ne!(pattern(index, 0), pattern(index, 1));
            assert_ne!(!pattern(index, 0), pattern(index, 1));
        }
    }

    #[test]
    fn ring_edges_cover_each_shard_and_destination_exactly_once() {
        for count in [2, 3, 5, 8] {
            let shards = partition(257, count).unwrap();
            let mut incoming = vec![None; count];
            for (source, shard) in shards.iter().enumerate() {
                let destination = (source + 1) % count;
                assert_ne!(source, destination);
                assert!(incoming[destination].replace(*shard).is_none());
            }
            for (device, incoming) in incoming.into_iter().enumerate() {
                assert_eq!(incoming, Some(shards[(device + count - 1) % count]));
            }
        }
    }

    #[test]
    fn authority_never_admits_a_compute_invocation() {
        let request = KfdRuntimeAuthorityRequestV1 {
            module_image: &[],
            module_sha256: [0; 32],
            kernel_name: "not-authorized",
            signature: [0; 32],
            explicit_kernarg: &[],
            complete_kernarg_template: &[],
            bindings: &[],
            dispatch_abi: &[],
            allocations: &[],
            geometry: fe2o3_runtime::RuntimeLaunchGeometryV1 {
                grid: [1; 3],
                workgroup: [1; 3],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: fe2o3_runtime::KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        assert!(!NoCompute.authorize_launch_v1(request));
    }
}
