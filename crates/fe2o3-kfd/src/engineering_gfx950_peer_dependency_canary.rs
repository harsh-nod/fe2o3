//! Fixed two-generation P -> barrier -> C experiment, never a generic scheduler.

use super::*;
use crate::topology::EngineeringGttSignalRoutesV1;
use fe2o3_aql::{
    AqlBarrierAndPacketV1, AqlBarrierAndPublicationTargetV1, AqlDependencyBarrierAndPacketV1,
    AqlDependencyBarrierAndPublicationTargetV1, AqlDispatchOrderingV1,
};
use std::path::Path;

const ELEMENTS: usize = 4096;
const HOLD: Duration = Duration::from_millis(25);
const DEADLINE: Duration = Duration::from_secs(2);
const SYMBOL: &str = "ferric_qwen3_tp_batch_residual_bf16_v3";
const OBJECT_SHA: &str = "6b0889e2834bda81313b3cbda5a60207dd9fa41c40ea80251bbec89d7befef2c";
const SOURCE_ROOT: &str = "/usr/src/amdgpu-6.16.13-2303411.24.04";
const SOURCE_PINS: [(&str, &str); 5] = [
    (
        "include/uapi/linux/kfd_ioctl.h",
        "b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d",
    ),
    (
        "amd/amdgpu/amdgpu_amdkfd_gpuvm.c",
        "c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d",
    ),
    (
        "amd/amdkfd/kfd_chardev.c",
        "f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba",
    ),
    (
        "amd/amdkfd/kfd_topology.c",
        "6a1453f8f70a9fba549694b71db132eb80679d7fbb8d0eb7af9dd8e7b669f802",
    ),
    (
        "include/uapi/linux/kfd_sysfs.h",
        "a7dd01da251ef1cb028169a6f20a029f98c56910595088d51c77036e021a6246",
    ),
];

fn sha(bytes: &[u8]) -> String {
    sha_string(&Sha256::digest(bytes).into())
}

pub(super) fn source_contract() -> Result<serde_json::Value> {
    let mut report = Vec::new();
    for (relative, expected) in SOURCE_PINS {
        let path = Path::new(SOURCE_ROOT).join(relative);
        let metadata = std::fs::metadata(&path).map_err(explain)?;
        if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
            return Err("installed source contract file bounds".into());
        }
        let actual = sha(&std::fs::read(&path).map_err(explain)?);
        if actual != expected {
            return Err(format!(
                "installed source contract changed: {}",
                path.display()
            ));
        }
        report.push(serde_json::json!({"path": path, "sha256": actual}));
    }
    if KfdAllocMemoryFlags::KERNARG.bits() != 0x8600_0002 {
        return Err("signal GTT allocation flags changed".into());
    }
    Ok(
        serde_json::json!({"files": report, "flags": 0x8600_0002_u32,
        "authority": "installed source identity plus retained device currentness, not proof of kernel correctness"}),
    )
}

fn check_metadata(metadata: &KernelMetadataV1) -> Result<()> {
    if metadata.symbol != SYMBOL
        || sha_string(&metadata.object_sha256) != OBJECT_SHA
        || metadata.kernarg_bytes != 312
        || metadata.kernarg_alignment != 8
        || metadata.group_segment_bytes != 0
        || metadata.private_segment_bytes != 0
        || metadata.wavefront_size != 64
        || metadata.implicit_argument_offset != Some(56)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 7
    {
        return Err("residual canary metadata identity".into());
    }
    for (argument, (offset, bytes, pointer)) in metadata.explicit_arguments.iter().zip([
        (0, 8, true),
        (8, 8, false),
        (16, 8, true),
        (24, 8, false),
        (32, 8, true),
        (40, 8, false),
        (48, 4, false),
    ]) {
        if (argument.offset, argument.bytes, argument.global_buffer) != (offset, bytes, pointer)
            || argument.pointee_alignment.is_some()
            || argument.access.is_some()
        {
            return Err("residual canary explicit argument identity".into());
        }
    }
    Ok(())
}

fn sha_string(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut rendered = String::with_capacity(64);
    for byte in bytes {
        write!(&mut rendered, "{byte:02x}").expect("writing hex into String cannot fail");
    }
    rendered
}

fn address(value: u64) -> Result<ObservedGpuAddressV1> {
    ObservedGpuAddressV1::new(value).map_err(explain)
}

// Capturing typed packets into ordinary host memory does not expose a queue.
// The resulting private value is consumed only by the fixed retained schedule.
#[derive(Default)]
struct PacketCapture {
    bytes: Option<[u8; 64]>,
    header: Option<u16>,
}

impl PacketCapture {
    fn body(&mut self, bytes: [u8; 64]) -> Result<()> {
        if self.bytes.replace(bytes).is_some() || self.header.is_some() {
            return Err("duplicate staged packet body".into());
        }
        Ok(())
    }
    fn header(&mut self, header: u16) -> Result<()> {
        if self.bytes.is_none() || self.header.replace(header).is_some() {
            return Err("staged packet header order".into());
        }
        Ok(())
    }
    fn finish(self) -> Result<Packet> {
        Ok(Packet {
            bytes: self.bytes.ok_or("missing staged body")?,
            header: self.header.ok_or("missing staged header")?,
        })
    }
}
impl AqlPacketPublicationTargetV1 for PacketCapture {
    type Error = String;
    fn write_unpublished(&mut self, packet: &AqlKernelDispatchPacketV1) -> Result<()> {
        self.body(packet.encode_unpublished_le())
    }
    fn publish_release_header(&mut self, header: u16) -> Result<()> {
        self.header(header)
    }
}
impl AqlBarrierAndPublicationTargetV1 for PacketCapture {
    type Error = String;
    fn write_unpublished_barrier(&mut self, packet: &AqlBarrierAndPacketV1) -> Result<()> {
        self.body(packet.encode_unpublished_le())
    }
    fn publish_barrier_release_header(&mut self, header: u16) -> Result<()> {
        self.header(header)
    }
}
impl AqlDependencyBarrierAndPublicationTargetV1 for PacketCapture {
    type Error = String;
    fn write_unpublished_dependency_barrier(
        &mut self,
        packet: &AqlDependencyBarrierAndPacketV1,
    ) -> Result<()> {
        self.body(packet.encode_unpublished_le())
    }
    fn publish_dependency_barrier_release_header(&mut self, header: u16) -> Result<()> {
        self.header(header)
    }
}

struct Packet {
    bytes: [u8; 64],
    header: u16,
}
struct Ready {
    generation: u32,
    identities: [[u64; 3]; 2], // unique ID, queue epoch, reserved next write
    first: [u64; 2],
    slots: [u32; 4], // P, W, Bc, C
    packets: [Packet; 4],
}

struct SignalTransaction<'a> {
    contexts: &'a mut [Context],
    routes: &'a EngineeringGttSignalRoutesV1,
}
impl PeerTransactionBackend for SignalTransaction<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(self.contexts, false)?;
        self.routes
            .refresh(self.contexts[0].backend.engineering_peer_topology())
    }
    fn map(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        let context = &mut self.contexts[0];
        context
            .backend
            .map_gpu_ids(context.dependency_internal[0].handle, peers, 0)
    }
    fn unmap(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        let context = &mut self.contexts[0];
        context
            .backend
            .unmap_gpu_ids(context.dependency_internal[0].handle, peers, 0)
    }
    fn release_owner(&mut self) -> Result<()> {
        // Peer unmap is complete; Context still owns the BO until group close.
        Ok(())
    }
}

fn current(contexts: &mut [Context], routes: &EngineeringGttSignalRoutesV1) -> Result<()> {
    for context in contexts.iter_mut() {
        context.check_currentness(true)?;
    }
    routes.refresh(contexts[0].backend.engineering_peer_topology())
}

fn initialize_signals(contexts: &mut [Context]) -> Result<()> {
    for (rank, count) in [(0, 1), (1, 3)] {
        if !contexts[rank].dependency_internal.is_empty() {
            return Err("dependency signal storage already exists".into());
        }
        let allocation =
            contexts[rank]
                .allocate_resource(PAGE_BYTES, KfdAllocMemoryFlags::KERNARG, |_| Ok(()))?;
        contexts[rank].dependency_internal.push(allocation);
        Backend::initialize_engineering_signal_slots(
            &mut contexts[rank].dependency_internal[0].mapping,
            count,
        )
        .map_err(explain)?;
    }
    let allocation = &contexts[0].dependency_internal[0];
    let aperture = contexts[1].backend.gpuvm_aperture();
    if allocation.va < aperture.base()
        || allocation
            .va
            .checked_add(allocation.backing as u64 - 1)
            .ok_or("signal VA overflow")?
            > aperture.limit()
    {
        return Err("shared signal outside peer aperture".into());
    }
    Ok(())
}

fn signals(contexts: &mut [Context]) -> Result<[i64; 4]> {
    let mut values = [0; 4];
    for (index, (rank, slot)) in [(0, 0), (1, 0), (1, 1), (1, 2)].into_iter().enumerate() {
        let (kind, value) = Backend::observe_completion_signal_state_acquire(
            &mut contexts[rank].dependency_internal[0].mapping,
            PAGE_BYTES,
            slot,
        )
        .map_err(explain)?;
        if kind != 1 || !matches!(value, 0 | 1) {
            return Err("dependency signal kind/value changed".into());
        }
        values[index] = value;
    }
    Ok(values)
}

fn kernel_packet(
    prepared: PreparedDispatch,
    context: &mut Context,
    signal: u64,
    ordering: AqlDispatchOrderingV1,
) -> Result<Packet> {
    Backend::with_bytes_mut(
        &mut context.internal[KERNARG].mapping,
        MAX_KERNARG_BYTES_V1 as usize,
        |bytes| {
            bytes.fill(0);
            bytes[..prepared.bytes.len()].copy_from_slice(&prepared.bytes);
        },
    );
    let mut capture = PacketCapture::default();
    AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
        prepared.geometry,
        0,
        prepared.group_bytes,
        address(prepared.descriptor)?,
        address(context.internal[KERNARG].va)?,
        prepared.alignment,
        address(signal)?,
        ordering,
    )
    .map_err(explain)?
    .publish_with(&mut capture)?;
    capture.finish()
}

fn stage(
    contexts: &mut [Context],
    prepared: [PreparedDispatch; 2],
    generation: u32,
) -> Result<Ready> {
    if !(1..=2).contains(&generation) {
        return Err("dependency generation outside fixed pair".into());
    }
    for context in contexts.iter_mut() {
        context.check_idle()?;
        let counters =
            Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                .map_err(explain)?;
        if counters != (context.completed_write, context.completed_write) {
            return Err("dependency stage requires actual drained frontier".into());
        }
        context.last_observed_read = counters.1;
    }
    let prior = signals(contexts)?;
    if (generation == 1 && prior != [1; 4]) || (generation == 2 && prior != [0; 4]) {
        return Err("dependency generation signal reuse gate".into());
    }
    if generation == 2 {
        for (rank, slot) in [(0, 0), (1, 0), (1, 1), (1, 2)] {
            Backend::reset_completion_signal_release(
                &mut contexts[rank].dependency_internal[0].mapping,
                PAGE_BYTES,
                slot,
            )
            .map_err(explain)?;
        }
    }
    let p = contexts[0].dependency_internal[0].va;
    let w = contexts[1].dependency_internal[0].va;
    let [producer, consumer] = prepared;
    let producer = kernel_packet(
        producer,
        &mut contexts[0],
        p,
        AqlDispatchOrderingV1::Independent,
    )?;
    let consumer = kernel_packet(
        consumer,
        &mut contexts[1],
        w + 128,
        AqlDispatchOrderingV1::WaitForPrior,
    )?;
    let mut witness = PacketCapture::default();
    AqlBarrierAndPacketV1::new_unpublished(address(w)?)
        .map_err(explain)?
        .publish_with(&mut witness)?;
    let mut barrier = PacketCapture::default();
    AqlDependencyBarrierAndPacketV1::new_unpublished(&[address(p)?], address(w + 64)?)
        .map_err(explain)?
        .publish_with(&mut barrier)?;
    let packets = [producer, witness.finish()?, barrier.finish()?, consumer];
    if packets
        .iter()
        .map(|packet| packet.header)
        .collect::<Vec<_>>()
        != [0x1402, 0x1403, 0x1503, 0x1502]
    {
        return Err("dependency fixed packet header identity".into());
    }
    let mut first = [0; 2];
    let mut slots = [0; 4];
    let mut identities = [[0; 3]; 2];
    for rank in 0..2 {
        let count = if rank == 0 { 1 } else { 3 };
        let context = &mut contexts[rank];
        let reservation = context
            .ring
            .reserve_batch(context.last_observed_read, count)
            .map_err(explain)?;
        first[rank] = reservation.first_packet_id();
        identities[rank] = [
            context.unique_id,
            context.queue_epoch,
            reservation.next_write(),
        ];
        for offset in 0..count {
            let index = if rank == 0 { 0 } else { offset as usize + 1 };
            let slot = ((first[rank] + u64::from(offset)) % (RING_BYTES as u64 / 64)) as u32;
            slots[index] = slot;
            Backend::write_aql_slot(
                &mut context.internal[RING].mapping,
                RING_BYTES,
                slot,
                &packets[index].bytes,
            )
            .map_err(explain)?;
        }
        let previous = Backend::fetch_add_aql_write(
            &mut context.internal[CONTROL].mapping,
            PAGE_BYTES,
            u64::from(count),
        )
        .map_err(explain)?;
        if previous != first[rank] {
            return Err("dependency queue reservation substitution".into());
        }
    }
    if signals(contexts)? != [1; 4] {
        return Err("staged dependency signals not pending".into());
    }
    Ok(Ready {
        generation,
        identities,
        first,
        slots,
        packets,
    })
}

fn observe(contexts: &mut [Context], ready: &Ready) -> Result<([i64; 4], [[u64; 2]; 2])> {
    let values = signals(contexts)?;
    let mut frontiers = [[0; 2]; 2];
    for rank in 0..2 {
        let context = &mut contexts[rank];
        if [context.unique_id, context.queue_epoch, context.ring.write()] != ready.identities[rank]
        {
            return Err("dependency queue generation substitution".into());
        }
        let counters =
            Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                .map_err(explain)?;
        validate_counters(
            ready.identities[rank][2],
            context.last_observed_read,
            counters,
        )?;
        if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
            .map_err(explain)?
            != 0
        {
            return Err("dependency queue exception".into());
        }
        context.last_observed_read = counters.1;
        frontiers[rank] = [counters.0, counters.1];
    }
    Ok((values, frontiers))
}

fn publish(
    contexts: &mut [Context],
    ready: &Ready,
    rank: usize,
    diagnostic: &mut Diagnostic,
) -> Result<()> {
    for index in if rank == 0 { 0..1 } else { 1..4 } {
        let context = &mut contexts[rank];
        diagnostic.operation = "publish_header";
        diagnostic.header_attempted[index] = true;
        if ready.packets[index].header == 0x1503 {
            Backend::publish_engineering_dependency_header(
                &mut context.internal[RING].mapping,
                RING_BYTES,
                ready.slots[index],
            )
            .map_err(explain)?;
        } else {
            Backend::publish_aql_header(
                &mut context.internal[RING].mapping,
                RING_BYTES,
                ready.slots[index],
                ready.packets[index].header,
            )
            .map_err(explain)?;
        }
        diagnostic.header_completed[index] = true;
    }
    diagnostic.operation = "publish_doorbell";
    diagnostic.doorbell_attempted[rank] = true;
    contexts[rank]
        .doorbell
        .as_mut()
        .ok_or("missing dependency doorbell")?
        .store_packet_id_release(ready.identities[rank][2] - 1)
        .map_err(explain)?;
    diagnostic.doorbell_completed[rank] = true;
    Ok(())
}

fn require_held(values: [i64; 4]) -> Result<()> {
    if values != [1, 0, 1, 1] {
        return Err("dependency hold escaped before producer publication".into());
    }
    Ok(())
}

fn drained(values: [i64; 4], frontiers: [[u64; 2]; 2], next: [u64; 2]) -> bool {
    values == [0; 4] && frontiers == [[next[0]; 2], [next[1]; 2]]
}

#[derive(Clone, Copy, Debug)]
struct Observation {
    values: [i64; 4],
    frontiers: [[u64; 2]; 2],
    producer_header: (u32, u16, u16),
}

#[derive(Default)]
struct Diagnostic {
    generation: Option<u32>,
    phase: &'static str,
    operation: &'static str,
    identities: Option<[[u64; 3]; 2]>,
    first: Option<[u64; 2]>,
    slots: Option<[u32; 4]>,
    last: Option<(Observation, Duration)>,
    observation_count: u64,
    protocol_finished_after: Option<Duration>,
    header_attempted: [bool; 4],
    header_completed: [bool; 4],
    doorbell_attempted: [bool; 2],
    doorbell_completed: [bool; 2],
    outputs_verified: [bool; 2],
}

impl Diagnostic {
    fn begin_generation(&mut self, generation: u32) {
        *self = Self {
            generation: Some(generation),
            phase: "prepare_generation",
            outputs_verified: self.outputs_verified,
            ..Self::default()
        };
    }

    fn report(&self, error: &str) -> serde_json::Value {
        let last = self.last.map(|(observation, at)| {
            serde_json::json!({"completion_values": observation.values,
                "actual_frontiers": observation.frontiers,
                "producer_header": observation.producer_header,
                "observed_after_ns": at.as_nanos(),
                "age_at_protocol_return_ns": self.protocol_finished_after.map(|end| end.saturating_sub(at).as_nanos())})
        });
        serde_json::json!({"schema": "gfx950-tp2-dependency-terminal-diagnostic-v1",
            "error": error.chars().take(512).collect::<String>(),
            "generation": self.generation, "phase": self.phase, "last_protocol_operation": self.operation,
            "queue_identities": self.identities, "first_packet_ids": self.first, "slots": self.slots,
            "last_observation": last, "observation_count": self.observation_count,
            "protocol_finished_after_ns": self.protocol_finished_after.map(|time| time.as_nanos()),
            "header_order": ["P", "W", "Bc", "C"],
            "header_attempted": self.header_attempted, "header_completed": self.header_completed,
            "doorbell_rank_order": ["producer", "consumer"],
            "doorbell_attempted": self.doorbell_attempted, "doorbell_completed": self.doorbell_completed,
            "doorbell_values": self.identities.map(|ids| [ids[0][2] - 1, ids[1][2] - 1]),
            "publication_completion_is_device_completion": false,
            "generation_outputs_verified": self.outputs_verified,
            "last_observation_is_terminal_snapshot": false,
            "additional_device_operations_after_failure": false,
            "actual_output_bytes_retained": false, "raw_output_replay": false,
            "host_time_is_gpu_time": false, "acceptance": false})
    }
}

trait ProtocolBackend {
    fn diagnostic(&mut self) -> &mut Diagnostic;
    fn now(&self) -> Duration;
    fn refresh(&mut self) -> Result<()>;
    fn observe(&mut self) -> Result<Observation>;
    fn publish_consumer(&mut self) -> Result<()>;
    fn publish_producer(&mut self) -> Result<()>;
    fn pause(&mut self);
}

struct ProtocolReceipt {
    held: Observation,
    final_observation: Observation,
    witness_after: Duration,
    hold: Duration,
    elapsed: Duration,
}

fn checked_observation(
    backend: &mut impl ProtocolBackend,
    previous: &mut [i64; 4],
) -> Result<Observation> {
    backend.diagnostic().operation = "observe";
    let observation = backend.observe()?;
    let at = backend.now();
    let diagnostic = backend.diagnostic();
    diagnostic.last = Some((observation, at));
    diagnostic.observation_count = diagnostic.observation_count.saturating_add(1);
    diagnostic.operation = "validate_observation";
    for (before, after) in previous.iter().zip(observation.values) {
        if !matches!(after, 0 | 1) || (*before == 0 && after != 0) {
            return Err("dependency completion regressed or changed value".into());
        }
    }
    *previous = observation.values;
    Ok(observation)
}

fn checked_refresh(backend: &mut impl ProtocolBackend) -> Result<()> {
    backend.diagnostic().operation = "refresh_currentness";
    backend.refresh()
}

fn require_unpublished(observation: Observation, first: u64, slot: u32) -> Result<()> {
    if observation.producer_header != (slot, 1, 1) || observation.frontiers[0][1] != first {
        return Err("producer was consumed or published before hold completed".into());
    }
    Ok(())
}

// This state machine has no allocation, reset, unmap, free, or rollback hook.
// Every failing operation returns immediately to the retained group owner.
fn protocol(
    backend: &mut impl ProtocolBackend,
    next: [u64; 2],
    producer_slot: u32,
) -> Result<ProtocolReceipt> {
    backend.diagnostic().phase = "initial_fence";
    checked_refresh(backend)?;
    let mut previous = [1; 4];
    let initial = checked_observation(backend, &mut previous)?;
    if initial.values != [1; 4] {
        return Err("dependency initial signals not pending".into());
    }
    require_unpublished(initial, next[0] - 1, producer_slot)?;
    let started = backend.now();
    let deadline = started
        .checked_add(DEADLINE)
        .ok_or("dependency deadline overflow")?;
    backend.diagnostic().phase = "consumer_publication";
    backend.publish_consumer()?;
    backend.diagnostic().phase = "witness_and_hold";
    let mut next_refresh = started;
    let mut hold_started = None;
    loop {
        let observation = checked_observation(backend, &mut previous)?;
        require_unpublished(observation, next[0] - 1, producer_slot)?;
        let now = backend.now();
        if now >= deadline {
            return Err("dependency witness/hold deadline; terminal teardown required".into());
        }
        if observation.values[0] != 1 || observation.values[2] != 1 || observation.values[3] != 1 {
            return Err("dependency consumer escaped unpublished producer".into());
        }
        if observation.values[1] == 0 {
            require_held(observation.values)?;
            let beginning = *hold_started.get_or_insert(now);
            if now.saturating_sub(beginning) >= HOLD {
                break;
            }
        }
        if now >= next_refresh {
            checked_refresh(backend)?;
            next_refresh = now + Duration::from_millis(100);
        }
        backend.pause();
    }
    backend.diagnostic().phase = "held_fence";
    checked_refresh(backend)?;
    let held = checked_observation(backend, &mut previous)?;
    require_held(held.values)?;
    require_unpublished(held, next[0] - 1, producer_slot)?;
    let hold_end = backend.now();
    if hold_end >= deadline {
        return Err("dependency deadline before producer exposure".into());
    }
    let beginning = hold_started.ok_or("missing dependency hold start")?;
    let hold = hold_end.saturating_sub(beginning);
    if hold < HOLD {
        return Err("dependency hold interval regressed".into());
    }
    // Producer publication must consume already staged bytes, never reset P.
    backend.diagnostic().phase = "producer_publication";
    backend.publish_producer()?;
    backend.diagnostic().phase = "drain";
    loop {
        let observation = checked_observation(backend, &mut previous)?;
        let now = backend.now();
        if now >= deadline {
            return Err("dependency drain deadline; terminal teardown required".into());
        }
        if drained(observation.values, observation.frontiers, next) {
            break;
        }
        if now >= next_refresh {
            checked_refresh(backend)?;
            next_refresh = now + Duration::from_millis(100);
        }
        backend.pause();
    }
    backend.diagnostic().phase = "final_fence";
    checked_refresh(backend)?;
    let final_observation = checked_observation(backend, &mut previous)?;
    let finished = backend.now();
    if finished >= deadline || !drained(final_observation.values, final_observation.frontiers, next)
    {
        return Err("dependency final completion fence changed or expired".into());
    }
    Ok(ProtocolReceipt {
        held,
        final_observation,
        witness_after: beginning.saturating_sub(started),
        hold,
        elapsed: finished.saturating_sub(started),
    })
}

struct NativeProtocol<'a> {
    contexts: &'a mut [Context],
    routes: &'a EngineeringGttSignalRoutesV1,
    ready: &'a Ready,
    clock: Instant,
    diagnostic: &'a mut Diagnostic,
}
impl ProtocolBackend for NativeProtocol<'_> {
    fn diagnostic(&mut self) -> &mut Diagnostic {
        self.diagnostic
    }
    fn now(&self) -> Duration {
        self.clock.elapsed()
    }
    fn refresh(&mut self) -> Result<()> {
        current(self.contexts, self.routes)
    }
    fn observe(&mut self) -> Result<Observation> {
        let (values, frontiers) = observe(self.contexts, self.ready)?;
        let producer_header = Backend::observe_aql_packet_header_acquire(
            &mut self.contexts[0].internal[RING].mapping,
            RING_BYTES,
            self.ready.first[0],
        )
        .map_err(explain)?;
        Ok(Observation {
            values,
            frontiers,
            producer_header,
        })
    }
    fn publish_consumer(&mut self) -> Result<()> {
        publish(self.contexts, self.ready, 1, self.diagnostic)
    }
    fn publish_producer(&mut self) -> Result<()> {
        publish(self.contexts, self.ready, 0, self.diagnostic)
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_micros(50));
    }
}

fn execute(
    contexts: &mut [Context],
    routes: &EngineeringGttSignalRoutesV1,
    ready: Ready,
    diagnostic: &mut Diagnostic,
) -> Result<serde_json::Value> {
    diagnostic.identities = Some(ready.identities);
    diagnostic.first = Some(ready.first);
    diagnostic.slots = Some(ready.slots);
    let mut backend = NativeProtocol {
        contexts,
        routes,
        ready: &ready,
        clock: Instant::now(),
        diagnostic,
    };
    let result = protocol(
        &mut backend,
        [ready.identities[0][2], ready.identities[1][2]],
        ready.slots[0],
    );
    backend.diagnostic.protocol_finished_after = Some(backend.now());
    let receipt = result?;
    diagnostic.phase = "context_completion_fence";
    for (context, identity) in contexts.iter_mut().zip(ready.identities) {
        context.completed_write = identity[2];
    }
    check_contexts(contexts, false)?;
    Ok(
        serde_json::json!({"generation": ready.generation, "first_packet_ids": ready.first,
        "queue_identities": ready.identities, "completion_values": receipt.final_observation.values,
        "frontiers": receipt.final_observation.frontiers,
        "held_values": receipt.held.values, "held_frontiers": receipt.held.frontiers,
        "held_producer_header": receipt.held.producer_header,
        "witness_after_ns": receipt.witness_after.as_nanos(), "hold_ns": receipt.hold.as_nanos(),
        "host_elapsed_ns": receipt.elapsed.as_nanos(), "host_time_is_gpu_time": false}),
    )
}

fn arguments() -> Vec<u8> {
    let mut bytes = vec![0; 312];
    for offset in [8, 24, 40] {
        bytes[offset..offset + 8].copy_from_slice(&(ELEMENTS as u64).to_le_bytes());
    }
    bytes[48..52].copy_from_slice(&1_u32.to_le_bytes());
    bytes
}

fn canary_bindings(
    buffers: [Gfx950EngineeringPeerBufferV1; 5],
) -> Result<[[Gfx950EngineeringPeerPointerV1; 3]; 2]> {
    let [partial, residual, bridge, c_partial, output] = buffers;
    if buffers
        .iter()
        .map(|buffer| buffer.id)
        .collect::<BTreeSet<_>>()
        .len()
        != 5
        || buffers.iter().any(|buffer| buffer.group != partial.group)
        || buffers
            .iter()
            .map(|buffer| buffer.owner)
            .collect::<Vec<_>>()
            != [0, 0, 0, 1, 1]
        || buffers
            .iter()
            .map(|buffer| buffer.bytes)
            .collect::<Vec<_>>()
            != [16384, 8192, 8192, 16384, 8192]
    {
        return Err("dependency fixed dataflow/extent identity".into());
    }
    Ok([
        [
            partial.pointer(0, 0, 16384, BufferAccessV1::Read),
            residual.pointer(16, 0, 8192, BufferAccessV1::Read),
            bridge.pointer(32, 0, 8192, BufferAccessV1::Write),
        ],
        [
            c_partial.pointer(0, 0, 16384, BufferAccessV1::Read),
            bridge.pointer(16, 0, 8192, BufferAccessV1::Read),
            output.pointer(32, 0, 8192, BufferAccessV1::Write),
        ],
    ])
}

struct GenerationData {
    partial: Vec<u8>,
    residual: Vec<u8>,
    consumer_partial: Vec<u8>,
    producer_expected: Vec<u8>,
    consumer_expected: Vec<u8>,
}

fn data(generation: u32) -> GenerationData {
    let mut partial = Vec::with_capacity(ELEMENTS * 4);
    let mut residual = Vec::with_capacity(ELEMENTS * 2);
    let mut consumer = Vec::with_capacity(ELEMENTS * 4);
    let mut p_expected = Vec::with_capacity(ELEMENTS * 2);
    let mut c_expected = Vec::with_capacity(ELEMENTS * 2);
    for index in 0..ELEMENTS {
        // Every index is distinct. BF16 x, 2x, and 8x are exact; FP32 6x
        // differs from the peer input. Generations use disjoint exponent ranges.
        let encoded = (generation as u16) * 0x2000 + index as u16;
        let p = f32::from_bits(u32::from(encoded) << 16);
        let c = p * 6.0;
        partial.extend(p.to_le_bytes());
        residual.extend(encoded.to_le_bytes());
        consumer.extend(c.to_le_bytes());
        p_expected.extend((encoded + 0x80).to_le_bytes());
        c_expected.extend((encoded + 0x180).to_le_bytes());
    }
    GenerationData {
        partial,
        residual,
        consumer_partial: consumer,
        producer_expected: p_expected,
        consumer_expected: c_expected,
    }
}

/// Runs only the fixed, pinned two-generation dependency canary.
///
/// # Safety
/// The caller must be a dedicated disposable single-threaded process, with no
/// other KFD client or GPU access. The pinned unauthenticated machine code must
/// satisfy its stated bounds/access contract. Every error requires immediate
/// process teardown; no retry, rollback, or further GPU operation is permitted.
pub unsafe fn run_gfx950_tp2_dependency_canary_unchecked_v1(
    unique_ids: [u64; 2],
    object_path: &Path,
) -> Result<serde_json::Value> {
    let sources = source_contract()?;
    let metadata = std::fs::metadata(object_path).map_err(explain)?;
    if !metadata.is_file() || metadata.len() > u64::from(MAX_OBJECT_BYTES_V1) {
        return Err("canary object file bounds".into());
    }
    let object = std::fs::read(object_path).map_err(explain)?;
    if sha(&object) != OBJECT_SHA {
        return Err("canary object digest differs from qualified residual".into());
    }
    let hash: [u8; 32] = Sha256::digest(&object).into();
    // SAFETY: inherited dedicated-process contract, retained through all errors.
    let mut group = unsafe {
        Gfx950EngineeringPeerGroupV1::open_with_queue_control_unchecked(
            &unique_ids,
            QueueControlPolicy::DependencyCanaryGttUncached,
        )
    }?;
    let mut diagnostic = Diagnostic {
        phase: "initialize_group",
        ..Diagnostic::default()
    };
    let result: Result<serde_json::Value> = (|| {
        let gpu_ids = [
            group.contexts[0].backend.gpu_id(),
            group.contexts[1].backend.gpu_id(),
        ];
        let routes = EngineeringGttSignalRoutesV1::observe(
            group.contexts[0].backend.engineering_peer_topology(),
            gpu_ids,
        )?;
        let producer = group.load_kernel(0, object.clone(), hash, SYMBOL.into())?;
        let consumer = group.load_kernel(1, object, hash, SYMBOL.into())?;
        check_metadata(producer.metadata())?;
        check_metadata(consumer.metadata())?;
        let partial = group.allocate(0, &[], (ELEMENTS * 4) as u64)?;
        let residual = group.allocate(0, &[], (ELEMENTS * 2) as u64)?;
        let bridge = group.allocate(0, &[1], (ELEMENTS * 2) as u64)?;
        let c_partial = group.allocate(1, &[], (ELEMENTS * 4) as u64)?;
        let output = group.allocate(1, &[], (ELEMENTS * 2) as u64)?;
        let [p_bindings, c_bindings] =
            canary_bindings([partial, residual, bridge, c_partial, output])?;
        initialize_signals(&mut group.contexts)?;
        let mut mapping = PeerMapping::new(vec![gpu_ids[1]])?;
        mapping.map(&mut SignalTransaction {
            contexts: &mut group.contexts,
            routes: &routes,
        })?;
        let mut generations = Vec::with_capacity(2);
        for generation in 1..=2 {
            diagnostic.begin_generation(generation);
            let GenerationData {
                partial: p,
                residual: r,
                consumer_partial: c,
                producer_expected: expected_p,
                consumer_expected: expected_c,
            } = data(generation);
            group.write(partial, 0, &p)?;
            group.write(residual, 0, &r)?;
            group.write(c_partial, 0, &c)?;
            let sentinel = vec![0xff; ELEMENTS * 2];
            group.write(bridge, 0, &sentinel)?;
            group.write(output, 0, &sentinel)?;
            let prepared_p = group.prepare_peer_dispatch(
                &producer,
                arguments(),
                [64, 1, 1],
                [4096, 1, 1],
                &p_bindings,
                2000,
            )?;
            let prepared_c = group.prepare_peer_dispatch(
                &consumer,
                arguments(),
                [64, 1, 1],
                [4096, 1, 1],
                &c_bindings,
                2000,
            )?;
            current(&mut group.contexts, &routes)?;
            diagnostic.phase = "stage";
            let ready = stage(&mut group.contexts, [prepared_p, prepared_c], generation)?;
            let mut receipt = execute(&mut group.contexts, &routes, ready, &mut diagnostic)?;
            diagnostic.phase = "verify_full_outputs";
            let actual_p = group.read(bridge, 0, (ELEMENTS * 2) as u32)?;
            let actual_c = group.read(output, 0, (ELEMENTS * 2) as u32)?;
            if actual_p != expected_p || actual_c != expected_c {
                return Err("dependency full-vector output mismatch".into());
            }
            diagnostic.outputs_verified[generation as usize - 1] = true;
            receipt["producer_output_sha256"] = sha(&actual_p).into();
            receipt["consumer_output_sha256"] = sha(&actual_c).into();
            receipt["verified_bf16_words_per_output"] = ELEMENTS.into();
            generations.push(receipt);
        }
        diagnostic.phase = "final_currentness_and_signals";
        current(&mut group.contexts, &routes)?;
        if signals(&mut group.contexts)? != [0; 4] {
            return Err("dependency final signal reuse gate".into());
        }
        diagnostic.phase = "release_peer_signal_mapping";
        mapping.release(&mut SignalTransaction {
            contexts: &mut group.contexts,
            routes: &routes,
        })?;
        diagnostic.phase = "final_installed_source_check";
        if source_contract()? != sources {
            return Err("installed source contract changed during canary".into());
        }
        let report = serde_json::json!({"schema": "gfx950-tp2-dependency-canary-v1",
            "object_sha256": OBJECT_SHA, "kernel_metadata": producer.metadata(),
            "sources": sources, "signal_routes": routes.report(), "generations": generations,
            "packet_headers": [0x1402, 0x1403, 0x1503, 0x1502],
            "all_outputs_verified": true, "all_signals_complete": true,
            "actual_frontiers_drained": true, "native_peer_signal_unmap_complete": true});
        diagnostic.phase = "close_group";
        group.close()?;
        Ok(report)
    })();
    if let Err(error) = &result {
        // Only retained host facts are serialized; no device reads or cleanup.
        use std::io::Write;
        let _ = writeln!(std::io::stderr().lock(), "{}", diagnostic.report(error));
    }
    group.finish(result)
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_dependency_canary_tests.rs"]
mod tests;
