//! Finite vecadd shards, joined native compute, peer transfer, and group capture.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_runtime::qualification_gfx942_sharded_vecadd_v1::{
    Gfx942ShardedVecaddQualificationArgumentsV1 as Arguments,
    Gfx942ShardedVecaddQualificationRecipeV1 as Recipe,
    admit_gfx942_sharded_vecadd_qualification_v1,
};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1,
    RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDrainOutcomeV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1, RuntimeCompletionStatusV1,
    RuntimeContextV1, RuntimeLaunchGeometryV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
    RuntimePollV1, RuntimeStreamIdV1, RuntimeStreamObservationV1, RuntimeSubmissionIdV1,
    RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use sha2::{Digest, Sha256};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const TOTAL_ELEMENTS: usize = 65_537;
const ELEMENT_BYTES: usize = 4;
const PAGE_BYTES: usize = 4096;
const WAIT: Duration = Duration::from_secs(30);
const DRAIN_TICKS: usize = 30_000;
const USAGE: &str = "usage: gfx942-runtime-sharded-vecadd-capture-smoke --round <0|1> <0xunique-id> <0xunique-id> [up to eight total]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Shard {
    offset: usize,
    elements: usize,
    padded_bytes: usize,
}

impl Shard {
    fn logical_bytes(self) -> usize {
        self.elements * ELEMENT_BYTES
    }

    fn geometry(self) -> RuntimeLaunchGeometryV1 {
        RuntimeLaunchGeometryV1 {
            grid: [(self.elements.div_ceil(256) * 256) as u32, 1, 1],
            workgroup: [256, 1, 1],
            dynamic_shared_bytes: 0,
        }
    }
}

struct Buffers {
    compute_stream: RuntimeStreamIdV1,
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    host: [RuntimeAllocationIdV1; 3],
    device: [RuntimeAllocationIdV1; 3],
    incoming_host: RuntimeAllocationIdV1,
    incoming: RuntimeAllocationIdV1,
    recipe: Recipe,
    kernel: TypedRuntimeKernelV1<Arguments>,
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
    eprintln!("sharded vecadd capture diagnostic: {detail}");
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

fn partition(count: usize) -> ResultV1<Vec<Shard>> {
    if !(2..=8).contains(&count) {
        return Err("two to eight shards required".into());
    }
    let mut offset = 0;
    let mut shards = Vec::with_capacity(count);
    for index in 0..count {
        let elements = TOTAL_ELEMENTS / count + usize::from(index < TOTAL_ELEMENTS % count);
        let logical = elements
            .checked_mul(ELEMENT_BYTES)
            .ok_or("logical extent overflow")?;
        let padded_bytes = logical
            .checked_add(PAGE_BYTES - 1)
            .ok_or("padding overflow")?
            / PAGE_BYTES
            * PAGE_BYTES;
        shards.push(Shard {
            offset,
            elements,
            padded_bytes,
        });
        offset = offset
            .checked_add(elements)
            .ok_or("shard offset overflow")?;
    }
    if offset != TOTAL_ELEMENTS {
        return Err("inexact fixed-total partition".into());
    }
    Ok(shards)
}

fn checked_recipe(count: usize, index: usize, round: usize, shard: Shard) -> ResultV1<Recipe> {
    let recipe =
        Recipe::new(count, index, round).map_err(|error| failure("shard-recipe", error))?;
    if recipe.count() != count
        || recipe.index() != index
        || recipe.round() != round
        || recipe.global_offset() != shard.offset
        || recipe.elements() != shard.elements
        || recipe.padded_bytes() != shard.padded_bytes
        || recipe.geometry() != shard.geometry()
    {
        return Err(failure("shard-recipe", "independent partition differs"));
    }
    Ok(recipe)
}

fn filled(value: f32, bytes: usize) -> Vec<u8> {
    value.to_bits().to_le_bytes().repeat(bytes / ELEMENT_BYTES)
}

fn initial_buffers(shard: Shard, round: usize) -> [Vec<u8>; 3] {
    let mut a = filled(-7.0, shard.padded_bytes);
    let mut b = filled(-11.0, shard.padded_bytes);
    for index in 0..shard.elements {
        let offset = index * ELEMENT_BYTES;
        let left = (shard.offset + index + 131_072 * round) as f32;
        let right = (3 + round) as f32;
        a[offset..offset + ELEMENT_BYTES].copy_from_slice(&left.to_bits().to_le_bytes());
        b[offset..offset + ELEMENT_BYTES].copy_from_slice(&right.to_bits().to_le_bytes());
    }
    [a, b, filled(-1.0, shard.padded_bytes)]
}

fn incoming_sentinel(shard: Shard) -> Vec<u8> {
    let mut bytes = filled(-19.0, shard.padded_bytes);
    bytes[..shard.logical_bytes()].copy_from_slice(&filled(-17.0, shard.logical_bytes()));
    bytes
}

// This oracle is independent of the authority's generated expected buffers.
fn expected_element(global: usize, round: usize) -> [u8; ELEMENT_BYTES] {
    ((global + 3 + 131_073 * round) as f32)
        .to_bits()
        .to_le_bytes()
}

fn reference_output(round: usize) -> Vec<u8> {
    (0..TOTAL_ELEMENTS)
        .flat_map(|global| expected_element(global, round))
        .collect()
}

fn verify_output(bytes: &[u8], shard: Shard, round: usize) -> ResultV1<()> {
    if bytes.len() != shard.padded_bytes {
        return Err(failure("output-extent", (bytes.len(), shard.padded_bytes)));
    }
    for (index, observed) in bytes.chunks_exact(ELEMENT_BYTES).enumerate() {
        let expected = if index < shard.elements {
            expected_element(shard.offset + index, round)
        } else {
            (-1.0_f32).to_bits().to_le_bytes()
        };
        if observed != expected {
            return Err(failure(
                "output-bits",
                (shard.offset, index, observed, expected),
            ));
        }
    }
    Ok(())
}

fn captured_digest(bytes: &[u8], shards: &[Shard], round: usize) -> ResultV1<String> {
    let extent: usize = shards.iter().map(|shard| shard.padded_bytes).sum();
    if bytes.len() != extent {
        return Err(failure("capture-extent", (bytes.len(), extent)));
    }
    let mut gathered = Vec::with_capacity(TOTAL_ELEMENTS * ELEMENT_BYTES);
    let mut cursor = 0;
    for &shard in shards {
        let end = cursor + shard.padded_bytes;
        verify_output(&bytes[cursor..end], shard, round)?;
        gathered.extend_from_slice(&bytes[cursor..cursor + shard.logical_bytes()]);
        cursor = end;
    }
    let expected = reference_output(round);
    if gathered.len() != TOTAL_ELEMENTS * ELEMENT_BYTES || gathered != expected {
        return Err(failure(
            "global-output",
            "gathered logical coverage differs",
        ));
    }
    let digest = hex_digest(&gathered);
    if digest != hex_digest(&expected) {
        return Err(failure("global-digest", "independent reference differs"));
    }
    Ok(digest)
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

fn verify_initial(
    context: &mut Context,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<()> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure("initial-readback", error))?;
    if observed != expected {
        return Err(failure("initial-bytes", "full allocation differs"));
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

fn join_all<A>(
    context: &mut Context,
    submissions: &mut [(RuntimeStreamIdV1, RuntimeSubmissionV1<A>)],
    stage: &str,
) -> ResultV1<()> {
    let deadline = Instant::now() + WAIT;
    for (stream, submission) in submissions {
        loop {
            if Instant::now() >= deadline {
                return Err(failure(stage, "shared deadline expired"));
            }
            context
                .flush_stream(*stream)
                .map_err(|error| failure(stage, error))?;
            match context
                .poll(submission)
                .map_err(|error| failure(stage, error))?
            {
                RuntimePollV1::Succeeded => break,
                RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
                status => return Err(failure(stage, status)),
            }
        }
    }
    Ok(())
}

fn run_round(ids: &[u64], shards: &[Shard], round: usize) -> ResultV1<String> {
    let recipes = shards
        .iter()
        .enumerate()
        .map(|(index, &shard)| checked_recipe(ids.len(), index, round, shard))
        .collect::<ResultV1<Vec<_>>>()?;
    let admitted = (0..ids.len())
        .map(|index| {
            admit_gfx942_sharded_vecadd_qualification_v1(ids.len(), index, round)
                .map_err(|error| failure("artifact-admission", error))
        })
        .collect::<ResultV1<Vec<_>>>()?;
    let backend = KfdMultiDeviceRuntimeBackendV1::open_gfx942_sharded_vecadd_peer_qualification_v1(
        ids, round,
    )
    .map_err(|error| failure("device-admission", error))?;
    // Failed setup retains native custody until this bounded witness process exits.
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 128, 128, 128)
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
            "exact ordered gfx942:xnack- roster required",
        ));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut buffers = Vec::with_capacity(ids.len());
    for (index, device) in devices.into_iter().enumerate() {
        let incoming_shard = shards[(index + ids.len() - 1) % ids.len()];
        let mut allocate = |kind, bytes: usize| {
            context
                .allocate(device, kind, bytes as u64, PAGE_BYTES as u64)
                .map_err(|error| failure("allocation", error))
        };
        let mut host = Vec::with_capacity(3);
        let mut local = Vec::with_capacity(3);
        for _ in 0..3 {
            host.push(allocate(
                RuntimeMemoryKindV1::HostVisible,
                shards[index].padded_bytes,
            )?);
            local.push(allocate(
                RuntimeMemoryKindV1::DeviceLocal,
                shards[index].padded_bytes,
            )?);
        }
        let incoming_host = allocate(
            RuntimeMemoryKindV1::HostVisible,
            incoming_shard.padded_bytes,
        )?;
        let incoming = allocate(
            RuntimeMemoryKindV1::DeviceLocal,
            incoming_shard.padded_bytes,
        )?;
        let host = host
            .try_into()
            .map_err(|_| failure("host-roster", "three host buffers required"))?;
        let device_allocations = local
            .try_into()
            .map_err(|_| failure("device-roster", "three device buffers required"))?;
        let compute_stream = context
            .create_stream(device)
            .map_err(|error| failure("compute-stream", error))?;
        let peer_stream = context
            .create_stream(device)
            .map_err(|error| failure("peer-stream", error))?;
        let readback_stream = context
            .create_stream(device)
            .map_err(|error| failure("readback-stream", error))?;
        let module = context
            .load_module(device, admitted[index].hsaco())
            .map_err(|error| failure("module-load", error))?;
        let kernel = context
            .resolve_kernel::<Arguments>(module, admitted[index].kernel_name())
            .map_err(|error| failure("kernel-resolve", error))?;
        buffers.push(Buffers {
            compute_stream,
            peer_stream,
            readback_stream,
            host,
            device: device_allocations,
            incoming_host,
            incoming,
            recipe: recipes[index],
            kernel,
        });
    }
    for (index, &shard) in shards.iter().enumerate() {
        for (allocation, bytes) in buffers[index]
            .host
            .iter()
            .zip(initial_buffers(shard, round))
        {
            context
                .write_allocation(*allocation, 0, &bytes)
                .map_err(|error| failure("input-host-write", error))?;
            verify_initial(&mut context, *allocation, &bytes)?;
        }
        context
            .write_allocation(
                buffers[(index + 1) % ids.len()].incoming_host,
                0,
                &incoming_sentinel(shard),
            )
            .map_err(|error| failure("sentinel-host-write", error))?;
    }
    let mut uploads = Vec::with_capacity(4 * ids.len());
    for (index, buffer) in buffers.iter().enumerate() {
        let incoming = shards[(index + ids.len() - 1) % ids.len()];
        let pairs = [
            (buffer.host[0], buffer.device[0], shards[index].padded_bytes),
            (buffer.host[1], buffer.device[1], shards[index].padded_bytes),
            (buffer.host[2], buffer.device[2], shards[index].padded_bytes),
            (buffer.incoming_host, buffer.incoming, incoming.padded_bytes),
        ];
        for (host, device, bytes) in pairs {
            let copy = context
                .copy_async(
                    buffer.compute_stream,
                    region(host, RuntimeAccessV1::Read, bytes as u64),
                    region(device, RuntimeAccessV1::Write, bytes as u64),
                    &[],
                )
                .map_err(|error| failure("upload-admission", error))?;
            uploads.push((buffer.compute_stream, copy));
        }
    }
    join_all(&mut context, &mut uploads, "upload-join")?;
    // Completed successors can still retain their predecessors' submission records.
    for (_, upload) in uploads.into_iter().rev() {
        context
            .release_submission(upload)
            .map_err(|error| failure("upload-release", error))?;
    }
    let mut capture_sources = Vec::with_capacity(ids.len());
    for (index, &shard) in shards.iter().enumerate() {
        // Preserve H2dReady input digests through exact compute authorization.
        // Native input readback would normalize those owners and clear the digests.
        let destination = &buffers[(index + 1) % ids.len()];
        verify_initial(
            &mut context,
            destination.incoming,
            &incoming_sentinel(shard),
        )?;
        capture_sources.push(
            context
                .prepare_host_drain_capture_v1(destination.incoming_host, 0, shard.padded_bytes)
                .map_err(|error| failure("capture-token", error))?,
        );
    }
    let compute_receipts = Arc::new(Mutex::new(Receipts::new(ids.len())));
    let mut compute = Vec::with_capacity(ids.len());
    let mut compute_ids = Vec::with_capacity(ids.len());
    // Admission may eagerly publish; no explicit progress begins before all N admissions.
    for (index, buffer) in buffers.iter().enumerate() {
        let [a, b, c] = buffer.device;
        let arguments = Arguments::new(buffer.recipe, a, b, c)
            .map_err(|error| failure("compute-arguments", error))?;
        let submission = context
            .launch(
                buffer.compute_stream,
                &buffer.kernel,
                &arguments,
                buffer.recipe.geometry(),
                &[],
            )
            .map_err(|error| failure("compute-admission", error))?;
        on_completion(&mut context, &submission, index, &compute_receipts)?;
        compute_ids.push(submission.id());
        compute.push((buffer.compute_stream, submission));
    }
    join_all(&mut context, &mut compute, "compute-join")?;
    if !compute_receipts
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .succeeded(&compute_ids)
    {
        return Err(failure(
            "compute-receipts",
            "missing, foreign, duplicate or failed completion",
        ));
    }
    for (_, submission) in compute.into_iter().rev() {
        context
            .release_submission(submission)
            .map_err(|error| failure("compute-release", error))?;
    }
    // Peer selection happens only after exact Context success and original owner restoration.
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
                region(
                    buffers[index].device[2],
                    RuntimeAccessV1::Read,
                    shard.padded_bytes as u64,
                ),
                region(
                    destination.incoming,
                    RuntimeAccessV1::Write,
                    shard.padded_bytes as u64,
                ),
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
                region(
                    destination.incoming,
                    RuntimeAccessV1::Read,
                    shard.padded_bytes as u64,
                ),
                region(
                    destination.incoming_host,
                    RuntimeAccessV1::Write,
                    shard.padded_bytes as u64,
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
    let capture_bytes: usize = shards.iter().map(|shard| shard.padded_bytes).sum();
    let capacity = 2 * ids.len() + 2;
    let config = RuntimeAsyncEngineConfigV1::new(
        capacity,
        capacity,
        capacity,
        capacity,
        Duration::from_micros(50),
    )
    .and_then(|config| config.with_reply_capacity(capacity))
    .and_then(|config| config.with_drain_capture_group_byte_capacity(capture_bytes))
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
                vec![0; capture_bytes].into_boxed_slice(),
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
    if captured.as_bytes().len() != capture_bytes
        || handle.observer().drain_capture_bytes_in_use() != capture_bytes
    {
        return Err(failure(
            "capture-extent",
            "aggregate byte accounting differs",
        ));
    }
    let digest = captured_digest(captured.as_bytes(), shards, round)?;
    drop(captured);
    if handle.observer().drain_capture_bytes_in_use() != 0 {
        return Err(failure("capture-release", "aggregate byte credit retained"));
    }
    Ok(digest)
}

fn run(ids: Vec<u64>, round: usize) -> ResultV1<()> {
    let shards = partition(ids.len())?;
    let digest = run_round(&ids, &shards, round)?;
    let unique_ids = ids
        .iter()
        .map(|id| format!("0x{id:016x}"))
        .collect::<Vec<_>>()
        .join(",");
    let ranges = shards
        .iter()
        .map(|shard| format!("{}:{}:{}", shard.offset, shard.elements, shard.padded_bytes))
        .collect::<Vec<_>>()
        .join(",");
    let padded_bytes: usize = shards.iter().map(|shard| shard.padded_bytes).sum();
    println!(
        "PASS schema=fe2o3.sharded-vecadd-capture-smoke.v1 authority=qualification-sharded-vecadd-v1 devices={} unique_ids={} round={} rounds=1 logical_elements={} logical_bytes={} shard_ranges={} padded_bytes={} launches={} compute_receipts={} transfer_receipts={} completion_receipts={} compute_admission=all-before-explicit-progress compute_join=exact-succeeded-restored peer_copies={} dependent_readbacks={} capture_ranges={} captured_bytes={} abi_bytes=48 kernel=vecadd journal=enabled host_output_installations=0 c_initial=-1 incoming_sentinel=full-byte-pass output=full-byte-pass padding=full-byte-pass coverage=exact-contiguous-disjoint global_sha256={} admission=all-transfers-pending-before-cutoff public_events=released-before-cutoff native_transport=authenticated-pending-peer native_counter_before=0 native_counter_observed=false native_counter_after=unobserved source_preservation=unobserved allocation_reuse=false contexts=one-per-process drain=pending-group capture=coherent-host-group physical_overlap=unmeasured cleanup=owned-shutdown-explicit performance_acceptance=false formal_refinement=false",
        ids.len(),
        unique_ids,
        round,
        TOTAL_ELEMENTS,
        TOTAL_ELEMENTS * ELEMENT_BYTES,
        ranges,
        padded_bytes,
        ids.len(),
        ids.len(),
        2 * ids.len(),
        3 * ids.len(),
        ids.len(),
        ids.len(),
        ids.len(),
        padded_bytes,
        digest
    );
    Ok(())
}

fn main() {
    if let Err(error) = options(&std::env::args().skip(1).collect::<Vec<_>>())
        .and_then(|(round, ids)| run(ids, round))
    {
        eprintln!("FAIL {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_inputs(shard: Shard, round: usize) -> Vec<u8> {
        let [a, b, mut c] = initial_buffers(shard, round);
        for index in 0..shard.elements {
            let offset = index * ELEMENT_BYTES;
            let left = f32::from_le_bytes(a[offset..offset + ELEMENT_BYTES].try_into().unwrap());
            let right = f32::from_le_bytes(b[offset..offset + ELEMENT_BYTES].try_into().unwrap());
            c[offset..offset + ELEMENT_BYTES].copy_from_slice(&(left + right).to_le_bytes());
        }
        c
    }

    #[test]
    fn cli_preserves_each_explicit_round_and_device_order() {
        for count in 2..=8 {
            for round in 0..=1 {
                let mut arguments = vec!["--round".into(), round.to_string()];
                arguments.extend((1..=count).rev().map(|id| format!("0x{id:x}")));
                assert_eq!(
                    options(&arguments).unwrap(),
                    (round, (1..=count).rev().collect())
                );
            }
        }
    }

    #[test]
    fn cli_rejects_implicit_round_duplicate_and_out_of_domain_ids() {
        for arguments in [
            vec![],
            vec!["--round"],
            vec!["0x1", "0x2"],
            vec!["--round", "0"],
            vec!["--round", "0", "0x1"],
            vec!["--round", "2", "0x1", "0x2"],
            vec!["--round", "00", "0x1", "0x2"],
            vec!["--round", "-1", "0x1", "0x2"],
            vec!["--round", "0", "0x1", "0x01"],
            vec!["--round", "0", "0x0", "0x2"],
            vec!["--round", "0", "1", "0x2"],
            vec!["--round", "0", "0x", "0x2"],
            vec!["--round", "0", "0xg", "0x2"],
            vec!["--round", "0", "0x+1", "0x2"],
            vec!["--round", "0", "0x10000000000000000", "0x2"],
            vec!["--round", "0", "--round", "1", "0x1", "0x2"],
        ] {
            assert!(
                options(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
        let mut arguments = vec!["--round".into(), "0".into()];
        arguments.extend((1..=9).map(|id| format!("0x{id:x}")));
        assert!(options(&arguments).is_err());
    }

    #[test]
    fn every_finite_recipe_matches_independent_fixed_total_and_page_partition() {
        let mut recipes = 0;
        for count in 2..=8 {
            let shards = partition(count).unwrap();
            assert_eq!(shards.len(), count);
            assert_eq!(shards[0].offset, 0);
            assert_eq!(
                shards.iter().map(|shard| shard.elements).sum::<usize>(),
                65_537
            );
            assert_eq!(
                shards.last().unwrap().offset + shards.last().unwrap().elements,
                65_537
            );
            assert!(
                shards
                    .windows(2)
                    .all(|pair| pair[0].offset + pair[0].elements == pair[1].offset)
            );
            assert!(shards.iter().map(|shard| shard.padded_bytes).sum::<usize>() <= 286_720);
            for (index, &shard) in shards.iter().enumerate() {
                assert!(shard.elements > 0);
                assert_eq!(shard.padded_bytes % 4096, 0);
                assert!(shard.padded_bytes >= shard.logical_bytes());
                assert!(shard.padded_bytes - shard.logical_bytes() < 4096);
                for round in 0..=1 {
                    checked_recipe(count, index, round, shard).unwrap();
                    recipes += 1;
                }
            }
        }
        assert_eq!(recipes, 70);
        assert_eq!(
            partition(2)
                .unwrap()
                .iter()
                .map(|shard| (shard.elements, shard.padded_bytes))
                .collect::<Vec<_>>(),
            [(32_769, 135_168), (32_768, 131_072)]
        );
        for count in [0, 1, 9, usize::MAX] {
            assert!(partition(count).is_err());
        }
    }

    #[test]
    fn abi_lengths_are_logical_and_grid_tail_does_not_expand_bindings() {
        for count in 2..=8 {
            for (index, shard) in partition(count).unwrap().into_iter().enumerate() {
                for round in 0..=1 {
                    let recipe = checked_recipe(count, index, round, shard).unwrap();
                    let abi = recipe.explicit_kernarg();
                    assert_eq!(abi.len(), 48);
                    for offset in [0, 16, 32] {
                        assert_eq!(&abi[offset..offset + 8], &[0; 8]);
                        assert_eq!(
                            &abi[offset + 8..offset + 16],
                            &(shard.elements as u64).to_le_bytes()
                        );
                    }
                    let geometry = recipe.geometry();
                    assert_eq!(geometry.workgroup, [256, 1, 1]);
                    assert_eq!(geometry.grid[0] % 256, 0);
                    assert!(geometry.grid[0] as usize >= shard.elements);
                    assert!((geometry.grid[0] as usize) < shard.elements + 256);
                    assert_eq!(geometry.dynamic_shared_bytes, 0);
                }
            }
        }
        let tail = partition(2).unwrap()[0];
        assert_ne!(tail.elements, tail.padded_bytes / ELEMENT_BYTES);
        assert_ne!(tail.geometry().grid[0] as usize, tail.elements);
    }

    #[test]
    fn independent_inputs_match_authority_without_using_its_expected_outputs() {
        for count in 2..=8 {
            for (index, shard) in partition(count).unwrap().into_iter().enumerate() {
                for round in 0..=1 {
                    let recipe = checked_recipe(count, index, round, shard).unwrap();
                    let authority = recipe.host_buffers().unwrap();
                    let [a, b, c] = initial_buffers(shard, round);
                    assert_eq!(a, authority.a());
                    assert_eq!(b, authority.b());
                    assert_eq!(c, authority.c_initial());
                    assert!(
                        c.chunks_exact(4)
                            .all(|word| word == (-1.0_f32).to_le_bytes())
                    );
                    assert!(
                        a[shard.logical_bytes()..]
                            .chunks_exact(4)
                            .all(|word| word == (-7.0_f32).to_le_bytes())
                    );
                    assert!(
                        b[shard.logical_bytes()..]
                            .chunks_exact(4)
                            .all(|word| word == (-11.0_f32).to_le_bytes())
                    );
                }
            }
        }
    }

    #[test]
    fn gathered_output_is_fixed_global_work_with_all_padding_verified() {
        let mut digests = Vec::new();
        for round in 0..=1 {
            let reference = reference_output(round);
            assert_eq!(reference.len(), 262_148);
            let digest = hex_digest(&reference);
            for count in 2..=8 {
                let shards = partition(count).unwrap();
                let captured: Vec<_> = shards
                    .iter()
                    .flat_map(|&shard| from_inputs(shard, round))
                    .collect();
                assert_eq!(captured_digest(&captured, &shards, round).unwrap(), digest);
                assert_eq!(
                    shards
                        .iter()
                        .map(|shard| shard.logical_bytes())
                        .sum::<usize>(),
                    reference.len()
                );
            }
            digests.push(digest);
        }
        assert_ne!(digests[0], digests[1]);
    }

    #[test]
    fn output_rejects_unchanged_c_wrong_round_bit_errors_padding_and_order() {
        let shards = partition(2).unwrap();
        let shard = shards[0];
        let valid = from_inputs(shard, 0);
        assert!(verify_output(&valid, shard, 0).is_ok());
        assert!(verify_output(&initial_buffers(shard, 0)[2], shard, 0).is_err());
        assert!(verify_output(&incoming_sentinel(shard), shard, 0).is_err());
        assert!(verify_output(&valid, shard, 1).is_err());
        assert!(verify_output(&valid[..valid.len() - 1], shard, 0).is_err());
        for offset in [
            0,
            shard.logical_bytes() - 1,
            shard.logical_bytes(),
            valid.len() - 1,
        ] {
            let mut corrupt = valid.clone();
            corrupt[offset] ^= 1;
            assert!(verify_output(&corrupt, shard, 0).is_err());
        }
        let reversed: Vec<_> = shards
            .iter()
            .rev()
            .flat_map(|&shard| from_inputs(shard, 0))
            .collect();
        assert!(captured_digest(&reversed, &shards, 0).is_err());
        let mut extra: Vec<_> = shards
            .iter()
            .flat_map(|&shard| from_inputs(shard, 0))
            .collect();
        extra.push(0);
        assert!(captured_digest(&extra, &shards, 0).is_err());
    }

    #[test]
    fn incoming_sentinel_differs_from_both_computed_data_and_preserved_padding() {
        for count in 2..=8 {
            for shard in partition(count).unwrap() {
                let sentinel = incoming_sentinel(shard);
                for round in 0..=1 {
                    let output = from_inputs(shard, round);
                    assert!(
                        sentinel
                            .chunks_exact(4)
                            .zip(output.chunks_exact(4))
                            .all(|(a, b)| a != b)
                    );
                    assert!(verify_output(&sentinel, shard, round).is_err());
                }
            }
        }
    }
}
