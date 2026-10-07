//! Pending native-peer inputs, exact D2H continuations, and one group cutoff.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeAuthorityRequestV1, KfdRuntimeLaunchAuthorityV1,
    RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeAsyncCurrentThreadOwnedEngineV1,
    RuntimeAsyncDrainOutcomeV1, RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1,
    RuntimeAsyncProgressConfigV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeStreamObservationV1,
    RuntimeSubmissionIdV1, RuntimeSubmissionV1,
};
use sha2::{Digest, Sha256};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const TOTAL_BYTES: u64 = 64 * 1024 * 1024 + 37;
const WAIT: Duration = Duration::from_secs(30);
const DRAIN_TICKS: usize = 30_000;
const USAGE: &str = "usage: gfx942-runtime-pending-peer-capture-smoke --round <0|1> <0xunique-id> <0xunique-id> [up to eight total]";

#[derive(Debug)]
struct NoCompute;

// SAFETY: No invocation receives compute authority.
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

struct Buffers {
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    outgoing_host: RuntimeAllocationIdV1,
    outgoing: RuntimeAllocationIdV1,
    incoming_host: RuntimeAllocationIdV1,
    incoming: RuntimeAllocationIdV1,
}

#[derive(Default)]
struct Receipts {
    observations: Vec<Option<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>,
    deliveries: Vec<usize>,
}

impl Receipts {
    fn new(count: usize) -> Self {
        Self {
            observations: vec![None; count],
            deliveries: vec![0; count],
        }
    }

    fn record(
        &mut self,
        index: usize,
        id: RuntimeSubmissionIdV1,
        status: RuntimeCompletionStatusV1,
    ) {
        self.deliveries[index] = self.deliveries[index].saturating_add(1);
        self.observations[index] = Some((id, status));
    }

    fn succeeded(&self, expected: &[RuntimeSubmissionIdV1]) -> bool {
        self.observations.len() == expected.len()
            && self.deliveries.len() == expected.len()
            && expected.iter().enumerate().all(|(index, &id)| {
                self.deliveries[index] == 1
                    && self.observations[index] == Some((id, RuntimeCompletionStatusV1::Succeeded))
            })
    }
}

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let detail = format!("stage={stage} {error:?}");
    eprintln!("pending peer capture diagnostic: {detail}");
    detail
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

fn options(arguments: &[String]) -> ResultV1<(usize, Vec<u64>)> {
    if arguments.first().map(String::as_str) != Some("--round") {
        return Err(USAGE.into());
    }
    let round = match arguments.get(1).map(String::as_str) {
        Some("0") => 0,
        Some("1") => 1,
        _ => return Err(USAGE.into()),
    };
    Ok((round, unique_ids(&arguments[2..])?))
}

fn partition(total: u64, count: usize) -> ResultV1<Vec<Shard>> {
    if !(2..=8).contains(&count) || total < count as u64 || total > TOTAL_BYTES {
        return Err("invalid fixed-total partition".into());
    }
    let mut offset = 0;
    let mut shards = Vec::with_capacity(count);
    for index in 0..count {
        let bytes = total / count as u64 + u64::from((index as u64) < total % count as u64);
        shards.push(Shard { offset, bytes });
        offset += bytes;
    }
    if offset != total {
        return Err("inexact shard coverage".into());
    }
    Ok(shards)
}

fn pattern(index: u64, round: usize) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8 ^ (round as u8).wrapping_mul(73)
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

fn verify(bytes: &[u8], offset: u64, round: usize, sentinel: bool) -> ResultV1<()> {
    for (index, &byte) in bytes.iter().enumerate() {
        let global = offset + index as u64;
        let expected = pattern(global, round);
        let expected = if sentinel { !expected } else { expected };
        if byte != expected {
            return Err(failure("byte-mismatch", (global, byte, expected)));
        }
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut text, "{byte:02x}").expect("String formatting");
    }
    text
}

fn on_completion<A>(
    context: &mut Context,
    submission: &RuntimeSubmissionV1<A>,
    index: usize,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<()> {
    let id = submission.id();
    let receipts = Arc::clone(receipts);
    context
        .on_completion(submission, move |status| {
            receipts
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .record(index, id, status);
        })
        .map_err(|error| failure("callback-admission", error))
}

fn run_round(ids: &[u64], shards: &[Shard], round: usize) -> ResultV1<String> {
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
    // Failed setup retains native custody until this bounded witness process exits.
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 64, 64, 64)
            .map_err(|error| failure("journal-context-open", error))?,
    );
    if context.devices().len() != ids.len()
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "device-roster",
            "exact gfx942:xnack- roster required",
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
        let peer_stream = context
            .create_stream(device)
            .map_err(|error| failure("peer-stream", error))?;
        let readback_stream = context
            .create_stream(device)
            .map_err(|error| failure("readback-stream", error))?;
        buffers.push(Buffers {
            peer_stream,
            readback_stream,
            outgoing_host,
            outgoing,
            incoming_host,
            incoming,
        });
    }
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
            .write_allocation(buffers[(index + 1) % ids.len()].incoming_host, 0, &contents)
            .map_err(|error| failure("sentinel-host-write", error))?;
    }
    let mut uploads = Vec::with_capacity(2 * ids.len());
    for (index, buffer) in buffers.iter().enumerate() {
        let incoming = shards[(index + ids.len() - 1) % ids.len()];
        for (host, allocation, bytes) in [
            (buffer.outgoing_host, buffer.outgoing, shards[index].bytes),
            (buffer.incoming_host, buffer.incoming, incoming.bytes),
        ] {
            let copy = context
                .copy_async(
                    buffer.peer_stream,
                    region(host, RuntimeAccessV1::Read, bytes),
                    region(allocation, RuntimeAccessV1::Write, bytes),
                    &[],
                )
                .map_err(|error| failure("upload-admission", error))?;
            uploads.push((buffer.peer_stream, copy));
        }
    }
    let upload_deadline = Instant::now() + WAIT;
    for (stream, copy) in &mut uploads {
        loop {
            if Instant::now() >= upload_deadline {
                return Err(failure("upload-deadline", "pending setup"));
            }
            context
                .flush_stream(*stream)
                .map_err(|error| failure("upload-flush", error))?;
            match context
                .poll(copy)
                .map_err(|error| failure("upload-poll", error))?
            {
                RuntimePollV1::Succeeded => break,
                RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
                status => return Err(failure("upload-status", status)),
            }
        }
    }
    // Later uploads retain their stream predecessors until they settle.
    for (_, copy) in uploads.into_iter().rev() {
        context
            .release_submission(copy)
            .map_err(|error| failure("upload-release", error))?;
    }
    let mut capture_sources = Vec::with_capacity(ids.len());
    for (index, shard) in shards.iter().enumerate() {
        let destination = &buffers[(index + 1) % ids.len()];
        let mut observed = vec![0; shard.bytes as usize];
        context
            .read_allocation(buffers[index].outgoing, 0, &mut observed)
            .map_err(|error| failure("initial-source-read", error))?;
        verify(&observed, shard.offset, round, false)?;
        context
            .read_allocation(destination.incoming, 0, &mut observed)
            .map_err(|error| failure("initial-sentinel-read", error))?;
        verify(&observed, shard.offset, round, true)?;
        capture_sources.push(
            context
                .prepare_host_drain_capture_v1(destination.incoming_host, 0, shard.bytes as usize)
                .map_err(|error| failure("capture-token", error))?,
        );
    }
    let receipts = Arc::new(Mutex::new(Receipts::new(2 * ids.len())));
    let mut peers = Vec::with_capacity(ids.len());
    let mut events = Vec::with_capacity(ids.len());
    let mut expected = Vec::with_capacity(2 * ids.len());
    // All ring edges are admitted while every original owner slot is available.
    for (index, shard) in shards.iter().enumerate() {
        let destination = &buffers[(index + 1) % ids.len()];
        let peer = context
            .peer_copy(
                destination.peer_stream,
                region(buffers[index].outgoing, RuntimeAccessV1::Read, shard.bytes),
                region(destination.incoming, RuntimeAccessV1::Write, shard.bytes),
                &[],
            )
            .map_err(|error| failure("peer-admission", error))?;
        on_completion(&mut context, &peer, index, &receipts)?;
        expected.push(peer.id());
        events.push(
            context
                .record_event(&peer)
                .map_err(|error| failure("peer-event", error))?,
        );
        peers.push(peer);
    }
    let mut readbacks = Vec::with_capacity(ids.len());
    for (index, shard) in shards.iter().enumerate() {
        let destination = &buffers[(index + 1) % ids.len()];
        let readback = context
            .copy_async(
                destination.readback_stream,
                region(destination.incoming, RuntimeAccessV1::Read, shard.bytes),
                region(
                    destination.incoming_host,
                    RuntimeAccessV1::Write,
                    shard.bytes,
                ),
                &[events[index]],
            )
            .map_err(|error| failure("exact-readback-admission", error))?;
        on_completion(&mut context, &readback, ids.len() + index, &receipts)?;
        expected.push(readback.id());
        readbacks.push(readback);
    }
    for event in events {
        context
            .release_event(event)
            .map_err(|error| failure("event-release", error))?;
    }
    for peer in &mut peers {
        if context
            .poll(peer)
            .map_err(|error| failure("peer-before-cutoff", error))?
            != RuntimePollV1::Pending
        {
            return Err(failure("peer-before-cutoff", "not pending"));
        }
    }
    for readback in &mut readbacks {
        if context
            .poll(readback)
            .map_err(|error| failure("readback-before-cutoff", error))?
            != RuntimePollV1::Pending
        {
            return Err(failure("readback-before-cutoff", "not pending"));
        }
    }
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure(
            "native-before-cutoff",
            "unexpected prior completion",
        ));
    }
    let capacity = 2 * ids.len() + 2;
    let config = RuntimeAsyncEngineConfigV1::new(
        capacity,
        capacity,
        capacity,
        capacity,
        Duration::from_micros(50),
    )
    .and_then(|config| config.with_reply_capacity(capacity))
    .and_then(|config| config.with_drain_capture_group_byte_capacity(TOTAL_BYTES as usize))
    .map_err(|error| failure("owner-config", error))?;
    let progress = RuntimeAsyncProgressConfigV1::new(ids.len(), 1)
        .map_err(|error| failure("owner-progress", error))?;
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || Ok::<_, String>(ManuallyDrop::into_inner(context)),
        config,
        progress,
    )
    .map_err(|error| failure("owner-open", error))?;
    let mut registrations = Vec::with_capacity(ids.len());
    for buffer in &buffers {
        registrations.push(Box::pin(
            handle
                .enqueue_stream_registration(buffer.readback_stream)
                .map_err(|error| failure("progress-registration", error))?,
        ));
    }
    let mut capture = Box::pin(
        handle
            .begin_drain_with_capture_group(
                DRAIN_TICKS,
                capture_sources.into_boxed_slice(),
                vec![0; TOTAL_BYTES as usize].into_boxed_slice(),
            )
            .map_err(|error| failure("capture-cutoff", error))?,
    );
    let deadline = Instant::now() + WAIT;
    let captured = engine.drive_until_ready(capture.as_mut(), deadline);
    let registrations: Vec<_> = registrations
        .iter_mut()
        .map(|registration| engine.drive_until_ready(registration.as_mut(), deadline))
        .collect();
    let shutdown = engine.shutdown();
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
    for registration in registrations {
        let registration = registration
            .map_err(|error| failure("registration-drive", error))?
            .map_err(|error| failure("registration-command", error))?
            .map_err(|error| failure("registration-result", error))?;
        if registration.failure_count() != 0 {
            return Err(failure("progress-failure", registration.take_failure()));
        }
    }
    let report = captured
        .map_err(|error| failure("capture-drive", error))?
        .map_err(|error| failure("capture-result", error))?;
    let expected_counts = RuntimeStreamObservationV1 {
        total_submissions: 2 * ids.len(),
        succeeded: 2 * ids.len(),
        ..RuntimeStreamObservationV1::default()
    };
    if report.drain.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || !report.drain.queued_commands_exhausted
        || report.drain.operations_remaining != 0
        || report.drain.graph_active
        || report.drain.retained_submissions != expected_counts
        || report.drain.ticks == 0
        || report.drain.ticks > DRAIN_TICKS
    {
        return Err(failure("pending-group-drain", report.drain));
    }
    if !receipts
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .succeeded(&expected)
    {
        return Err(failure(
            "exact-completion-receipts",
            "missing, duplicate, foreign or failed result",
        ));
    }
    let captured = report
        .capture
        .map_err(|error| failure("coherent-group-capture", error))?;
    if captured.as_bytes().len() != TOTAL_BYTES as usize
        || handle.observer().drain_capture_bytes_in_use() != TOTAL_BYTES as usize
    {
        return Err(failure(
            "capture-extent",
            "aggregate byte accounting differs",
        ));
    }
    verify(captured.as_bytes(), 0, round, false)?;
    let digest = hex_digest(captured.as_bytes());
    drop(captured);
    if handle.observer().drain_capture_bytes_in_use() != 0 {
        return Err(failure("capture-release", "aggregate byte credit retained"));
    }
    Ok(digest)
}

fn run(ids: Vec<u64>, round: usize) -> ResultV1<()> {
    let shards = partition(TOTAL_BYTES, ids.len())?;
    // ACQUIRE_VM retains process-lifetime admission; each round needs a new process.
    let digest = run_round(&ids, &shards, round)?;
    let unique_ids = ids
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
        "PASS schema=fe2o3.pending-peer-capture-smoke.v2 authority=production-deny-all devices={} unique_ids={} round={} rounds=1 total_bytes_per_round={} total_copied_bytes={} shard_ranges={} peer_copies={} dependent_readbacks={} completion_receipts={} capture_ranges={} captured_bytes={} launches=0 journal=enabled admission=all-pending-before-cutoff public_events=released-before-cutoff native_transport=authenticated-pending-peer native_counter_before=0 native_counter_observed=false native_counter_after=unobserved source_initial=full-byte-pass destination_sentinel=full-byte-pass source_preservation=unobserved output=full-byte-pass coverage=exact-contiguous-disjoint round_sha256={} allocation_reuse=false contexts=one-per-process drain=pending-group capture=coherent-host-group physical_overlap=unmeasured cleanup=owned-shutdown-explicit performance_acceptance=false formal_refinement=false",
        ids.len(),
        unique_ids,
        round,
        TOTAL_BYTES,
        TOTAL_BYTES,
        ranges,
        ids.len(),
        ids.len(),
        2 * ids.len(),
        ids.len(),
        TOTAL_BYTES,
        digest
    );
    Ok(())
}

fn main() {
    if let Err(error) = options(&std::env::args().skip(1).collect::<Vec<_>>())
        .and_then(|(round, ids)| run(ids, round))
    {
        eprintln!("pending peer capture smoke failed before full acceptance: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_preserves_two_three_five_and_eight_explicit_ids() {
        for count in [2, 3, 5, 8] {
            let expected: Vec<_> = (1..=count).rev().collect();
            for round in 0..2 {
                let mut args = vec!["--round".into(), round.to_string()];
                args.extend(expected.iter().map(|id| format!("0x{id:x}")));
                assert_eq!(options(&args).unwrap(), (round, expected.clone()));
            }
        }
    }

    #[test]
    fn cli_rejects_missing_duplicate_foreign_syntax_zero_and_overflow_ids() {
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
            assert!(unique_ids(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        for args in [
            vec![],
            vec!["--round"],
            vec!["0x1", "0x2"],
            vec!["--round", "0"],
            vec!["--round", "2", "0x1", "0x2"],
            vec!["--round", "00", "0x1", "0x2"],
            vec!["--round", "-1", "0x1", "0x2"],
            vec!["--round", "0", "0x1", "0x01"],
            vec!["--round", "0", "--round", "1", "0x1", "0x2"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn odd_shards_cover_the_fixed_total_once_without_empty_ranges() {
        for count in [2, 3, 5, 8] {
            for total in [count as u64, 257, TOTAL_BYTES] {
                let shards = partition(total, count).unwrap();
                let mut offset = 0;
                for (index, shard) in shards.iter().enumerate() {
                    assert_eq!(shard.offset, offset);
                    assert_eq!(
                        shard.bytes,
                        total / count as u64 + u64::from((index as u64) < total % count as u64)
                    );
                    assert!(shard.bytes > 0);
                    offset += shard.bytes;
                }
                assert_eq!(offset, total);
            }
        }
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
    fn exact_global_patterns_detect_stale_rounds_and_packet_origin_reuse() {
        for round in 0..2 {
            let expected: Vec<_> = (0..257).map(|offset| pattern(offset, round)).collect();
            verify(&expected, 0, round, false).unwrap();
            assert!(verify(&expected, 0, round, true).is_err());
            assert!(verify(&expected, 0, 1 - round, false).is_err());
            for count in [2, 3, 5, 8] {
                let shards = partition(257, count).unwrap();
                let actual: Vec<_> = shards
                    .iter()
                    .flat_map(|shard| {
                        (0..shard.bytes).map(|offset| pattern(shard.offset + offset, round))
                    })
                    .collect();
                assert_eq!(actual, expected);
                assert_eq!(hex_digest(&actual), hex_digest(&expected));
            }
        }
        for offset in 0..257 {
            assert_ne!(pattern(offset, 0), pattern(offset, 1));
        }
    }
}
