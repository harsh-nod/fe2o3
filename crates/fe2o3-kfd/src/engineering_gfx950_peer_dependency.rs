//! Closed row-one TP2 partial-producer/ordered-residual transaction.

use super::*;
use crate::engineering_wire::ExplicitArgumentV1;
use crate::topology::EngineeringGttSignalRoutesV1;
use fe2o3_aql::{
    AqlDependencyBarrierAndPacketV1, AqlDependencyBarrierAndPublicationTargetV1,
    AqlDispatchOrderingV1,
};

const PRODUCER: &str = "ferric_qwen3_tp_mfma_gemm_partial_f32_v3";
const CONSUMER: &str = "ferric_qwen3_tp_peer_tp2_ordered_residual_bf16_v18";
const ELEMENTS: u64 = 4096;
const CAPACITY: u64 = 16;
const TIMEOUT_MS: u32 = 2000;
const DEADLINE: Duration = Duration::from_secs(2);
const SIGNALS_PER_RANK: usize = 3;
const SIGNAL_STORAGE: usize = 0;
const ARGUMENT_STORAGE: usize = 1;
const ARGUMENT_SLOT_BYTES: usize = PAGE_BYTES / 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gfx950EngineeringTp2DependencyOperationV1 {
    AttentionOutputSum,
    FeedForwardDownSum,
}

/// Host transaction labels, not memory authority or a Ferric collective proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gfx950EngineeringTp2DependencyIdentityV1 {
    pub request_id: u64,
    pub generation: u64,
    pub group_id: u64,
    pub epoch: u64,
    pub layer: u32,
    pub operation: Gfx950EngineeringTp2DependencyOperationV1,
}

/// Exact rank-zero/rank-one command arrays. Completion signals stay private.
pub struct Gfx950EngineeringTp2DependencyRequestV1<'a> {
    pub identity: Gfx950EngineeringTp2DependencyIdentityV1,
    pub producers: [Gfx950EngineeringPeerDispatchV1<'a>; 2],
    pub consumers: [Gfx950EngineeringPeerDispatchV1<'a>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gfx950EngineeringTp2DependencyQueueV1 {
    pub unique_id: u64,
    pub queue_epoch: u64,
    pub first_packet: u64,
    pub next_packet: u64,
}

/// Completed observations only. No field is a GPU timestamp or reusable proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gfx950EngineeringTp2DependencyReceiptV1 {
    pub identity: Gfx950EngineeringTp2DependencyIdentityV1,
    pub queues: [Gfx950EngineeringTp2DependencyQueueV1; 2],
    pub kernel_counts: [u32; 2],
    pub barrier_counts: [u32; 2],
    pub packet_counts: [u32; 2],
    /// Rank-major slots: [[P0, B0, C0], [P1, B1, C1]].
    pub completion_values: [[i64; 3]; 2],
    /// Rank-major [actual write, actual read]. Both equal the reserved next ID.
    pub final_frontiers: [[u64; 2]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Fresh,
    Preparing,
    Completed,
    Released,
}

pub(super) struct DependencyOwner {
    routes: EngineeringGttSignalRoutesV1,
    sources: serde_json::Value,
    mappings: [PeerMapping; 2],
    next_generation: u64,
    last_request: u64,
    phase: Phase,
}

fn next_identity(
    identity: Gfx950EngineeringTp2DependencyIdentityV1,
    expected_generation: u64,
    last_request: u64,
) -> Result<u64> {
    if identity.generation != expected_generation
        || identity.generation == 0
        || identity.request_id == 0
        || identity.request_id <= last_request
        || identity.layer >= 36
    {
        return Err("TP2 dependency transaction identity".into());
    }
    identity
        .generation
        .checked_add(1)
        .ok_or_else(|| "TP2 dependency generation exhausted".into())
}

fn shape(operation: Gfx950EngineeringTp2DependencyOperationV1) -> (u64, u32) {
    match operation {
        Gfx950EngineeringTp2DependencyOperationV1::AttentionOutputSum => (2048, 1),
        Gfx950EngineeringTp2DependencyOperationV1::FeedForwardDownSum => (6144, 2),
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset.checked_add(8).ok_or("argument offset overflow")?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or("argument word bounds")?
            .try_into()
            .map_err(explain)?,
    ))
}

fn scalar(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or("argument offset overflow")?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or("argument scalar bounds")?
            .try_into()
            .map_err(explain)?,
    ))
}

fn metadata_contract(metadata: &KernelMetadataV1, producer: bool) -> Result<()> {
    let slices: u32 = if producer { 3 } else { 10 };
    let scalars: u32 = if producer { 5 } else { 2 };
    let explicit_bytes = slices * 16 + scalars * 4;
    let implicit = (explicit_bytes + 7) & !7;
    let expected = (0..slices * 2)
        .map(|index| ExplicitArgumentV1 {
            offset: index * 8,
            bytes: 8,
            global_buffer: index.is_multiple_of(2),
            pointee_alignment: None,
            access: None,
        })
        .chain((0..scalars).map(|index| ExplicitArgumentV1 {
            offset: slices * 16 + index * 4,
            bytes: 4,
            global_buffer: false,
            pointee_alignment: None,
            access: None,
        }))
        .collect::<Vec<_>>();
    if metadata.symbol != if producer { PRODUCER } else { CONSUMER }
        || metadata.kernarg_bytes != implicit + 256
        || metadata.kernarg_alignment != 8
        || metadata.implicit_argument_offset != Some(implicit)
        || metadata.implicit_argument_bytes != 256
        || metadata.wavefront_size != 64
        || metadata.group_segment_bytes != 0
        || metadata.private_segment_bytes != 0
        || metadata.explicit_arguments != expected
    {
        return Err("TP2 dependency exact kernel ABI/profile".into());
    }
    Ok(())
}

fn pointer_contract(
    pointer: &Gfx950EngineeringPeerPointerV1,
    offset: u32,
    elements: u64,
    width: u64,
    access: BufferAccessV1,
) -> Result<()> {
    let bytes = elements
        .checked_mul(width)
        .ok_or("TP2 pointer extent overflow")?;
    if pointer.kernarg_offset != offset
        || pointer.buffer_offset != 0
        || pointer.extent_bytes != bytes
        || bytes > pointer.buffer.bytes
        || pointer.access != access
    {
        return Err("TP2 dependency exact pointer range/access".into());
    }
    Ok(())
}

fn command_contract(
    command: &Gfx950EngineeringPeerDispatchV1<'_>,
    rank: usize,
    producer: bool,
    operation: Gfx950EngineeringTp2DependencyOperationV1,
) -> Result<()> {
    metadata_contract(command.kernel.metadata(), producer)?;
    let (k, tag) = shape(operation);
    let slices = if producer { 3 } else { 10 };
    let explicit = if producer { 68 } else { 168 };
    if command.kernel.rank != rank
        || command.timeout_ms != TIMEOUT_MS
        || command.workgroup != [64, 1, 1]
        || command.grid != [if producer { 16384 } else { 4096 }, 1, 1]
        || command.bytes.len() != command.kernel.metadata.kernarg_bytes as usize
        || command.pointers.len() != slices
        || command.bytes[explicit..].iter().any(|&byte| byte != 0)
    {
        return Err("TP2 dependency command shape/timeout/payload".into());
    }
    for index in 0..slices {
        if word(&command.bytes, index * 16)? != 0 {
            return Err("TP2 dependency requires unresolved pointer placeholders".into());
        }
    }
    if producer {
        for (index, (elements, width, access)) in [
            (CAPACITY * k, 2, BufferAccessV1::Read),
            (ELEMENTS * k, 2, BufferAccessV1::Read),
            (CAPACITY * ELEMENTS, 4, BufferAccessV1::Write),
        ]
        .into_iter()
        .enumerate()
        {
            if word(&command.bytes, index * 16 + 8)? != elements
                || command.pointers[index].buffer.owner != rank
            {
                return Err("TP2 producer exact slice/owner".into());
            }
            pointer_contract(
                &command.pointers[index],
                (index * 16) as u32,
                elements,
                width,
                access,
            )?;
        }
        for (index, expected) in [1, 4096, k as u32, 2, tag].into_iter().enumerate() {
            if scalar(&command.bytes, 48 + index * 4)? != expected {
                return Err("TP2 producer scalar shape".into());
            }
        }
    } else {
        for index in 0..10 {
            let elements = if (2..8).contains(&index) { 0 } else { ELEMENTS };
            if word(&command.bytes, index * 16 + 8)? != elements {
                return Err("TP2 consumer exact slice".into());
            }
            pointer_contract(
                &command.pointers[index],
                (index * 16) as u32,
                elements,
                if index < 8 { 4 } else { 2 },
                if index == 9 {
                    BufferAccessV1::Write
                } else {
                    BufferAccessV1::Read
                },
            )?;
        }
        if scalar(&command.bytes, 160)? != 1 || scalar(&command.bytes, 164)? != 2 {
            return Err("TP2 consumer scalar shape".into());
        }
    }
    Ok(())
}

fn graph_contract(request: &Gfx950EngineeringTp2DependencyRequestV1<'_>) -> Result<()> {
    for rank in 0..2 {
        command_contract(
            &request.producers[rank],
            rank,
            true,
            request.identity.operation,
        )?;
        command_contract(
            &request.consumers[rank],
            rank,
            false,
            request.identity.operation,
        )?;
    }
    if request.producers[0].kernel.metadata.object_sha256
        != request.producers[1].kernel.metadata.object_sha256
        || request.consumers[0].kernel.metadata.object_sha256
            != request.consumers[1].kernel.metadata.object_sha256
    {
        return Err("TP2 dependency image differs between ranks".into());
    }
    let partials = [
        request.producers[0].pointers[2].buffer,
        request.producers[1].pointers[2].buffer,
    ];
    let hidden = [
        request.consumers[0].pointers[8].buffer,
        request.consumers[1].pointers[8].buffer,
    ];
    let outputs = [
        request.consumers[0].pointers[9].buffer,
        request.consumers[1].pointers[9].buffer,
    ];
    let distinct = partials
        .into_iter()
        .chain(hidden)
        .chain(outputs)
        .chain(
            request
                .producers
                .iter()
                .flat_map(|command| command.pointers[..2].iter().map(|pointer| pointer.buffer)),
        )
        .map(|b| (b.group, b.id))
        .collect::<BTreeSet<_>>();
    if distinct.len() != 10 {
        return Err("TP2 dependency workspace roles alias".into());
    }
    for rank in 0..2 {
        if hidden[rank].owner != rank || outputs[rank].owner != rank {
            return Err("TP2 consumer hidden/output owner".into());
        }
        for index in 0..8 {
            if request.consumers[rank].pointers[index].buffer
                != partials[if index < 2 { index } else { 0 }]
            {
                return Err("TP2 consumer ordered partial/filler binding".into());
            }
        }
    }
    let commands = [
        &request.producers[0],
        &request.producers[1],
        &request.consumers[0],
        &request.consumers[1],
    ];
    for (index, command) in commands.iter().enumerate() {
        for (pointer_index, pointer) in command.pointers.iter().enumerate() {
            for other in &command.pointers[..pointer_index] {
                if pointer.extent_bytes != 0
                    && other.extent_bytes != 0
                    && pointer.buffer == other.buffer
                    && (pointer.access != BufferAccessV1::Read
                        || other.access != BufferAccessV1::Read)
                {
                    return Err("TP2 dependency intra-command mutable alias".into());
                }
            }
        }
        for pointer in &command.pointers {
            if pointer.extent_bytes == 0 {
                continue;
            }
            for (other_index, other) in commands[..index].iter().enumerate() {
                for other_pointer in &other.pointers {
                    if other_pointer.extent_bytes == 0
                        || pointer.buffer != other_pointer.buffer
                        || pointer.access == BufferAccessV1::Read
                            && other_pointer.access == BufferAccessV1::Read
                    {
                        continue;
                    }
                    let declared = other_index < 2
                        && index >= 2
                        && other_pointer.kernarg_offset == 32
                        && other_pointer.access == BufferAccessV1::Write
                        && pointer.kernarg_offset == (other_index * 16) as u32
                        && pointer.access == BufferAccessV1::Read;
                    if !declared {
                        return Err("TP2 dependency undeclared cross-command alias".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn source_contract() -> Result<serde_json::Value> {
    super::dependency_canary::source_contract()
}

fn current(contexts: &mut [Context], owner: &DependencyOwner) -> Result<()> {
    for context in contexts.iter_mut() {
        context.check_currentness(true)?;
    }
    owner
        .routes
        .refresh(contexts[0].backend.engineering_peer_topology())?;
    if source_contract()? != owner.sources {
        return Err("TP2 dependency installed source changed".into());
    }
    Ok(())
}

struct SignalTransaction<'a> {
    contexts: &'a mut [Context],
    routes: &'a EngineeringGttSignalRoutesV1,
    owner: usize,
}

impl PeerTransactionBackend for SignalTransaction<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(self.contexts, false)?;
        self.routes
            .refresh(self.contexts[0].backend.engineering_peer_topology())
    }
    fn map(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        let context = &mut self.contexts[self.owner];
        context
            .backend
            .map_gpu_ids(context.dependency_internal[SIGNAL_STORAGE].handle, peers, 0)
    }
    fn unmap(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        let context = &mut self.contexts[self.owner];
        context
            .backend
            .unmap_gpu_ids(context.dependency_internal[SIGNAL_STORAGE].handle, peers, 0)
    }
    fn release_owner(&mut self) -> Result<()> {
        // Both contexts retain every private BO until all peer unmaps succeed.
        Ok(())
    }
}

fn initialize_storage(contexts: &mut [Context]) -> Result<()> {
    for context in contexts.iter_mut() {
        if !context.dependency_internal.is_empty() {
            return Err("TP2 dependency storage already exists".into());
        }
        let signals =
            context.allocate_resource(PAGE_BYTES, KfdAllocMemoryFlags::KERNARG, |_| Ok(()))?;
        context.dependency_internal.push(signals);
        Backend::initialize_engineering_signal_slots(
            &mut context.dependency_internal[SIGNAL_STORAGE].mapping,
            SIGNALS_PER_RANK,
        )
        .map_err(explain)?;
        let arguments =
            context.allocate_resource(PAGE_BYTES, KfdAllocMemoryFlags::KERNARG, |_| Ok(()))?;
        context.dependency_internal.push(arguments);
    }
    for rank in 0..2 {
        let allocation = &contexts[rank].dependency_internal[SIGNAL_STORAGE];
        let aperture = contexts[1 - rank].backend.gpuvm_aperture();
        if allocation.va < aperture.base()
            || allocation
                .va
                .checked_add(allocation.backing as u64 - 1)
                .ok_or("TP2 signal VA overflow")?
                > aperture.limit()
        {
            return Err("TP2 signal outside peer aperture".into());
        }
    }
    Ok(())
}

fn signals(contexts: &mut [Context]) -> Result<[[i64; 3]; 2]> {
    let mut values = [[0; 3]; 2];
    for rank in 0..2 {
        for (slot, output) in values[rank].iter_mut().enumerate() {
            let (kind, value) = Backend::observe_completion_signal_state_acquire(
                &mut contexts[rank].dependency_internal[SIGNAL_STORAGE].mapping,
                PAGE_BYTES,
                slot as u32,
            )
            .map_err(explain)?;
            if kind != 1 || !matches!(value, 0 | 1) {
                return Err("TP2 dependency signal kind/value changed".into());
            }
            *output = value;
        }
    }
    Ok(values)
}

fn require_quiescent(contexts: &mut [Context], phase: Phase) -> Result<()> {
    let expected = match phase {
        Phase::Fresh => [[1; 3]; 2],
        Phase::Completed => [[0; 3]; 2],
        _ => return Err("TP2 dependency lifecycle is not quiescent".into()),
    };
    check_contexts(contexts, false)?;
    if signals(contexts)? != expected {
        return Err("TP2 dependency signal reuse/close gate".into());
    }
    for context in contexts {
        let counters =
            Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                .map_err(explain)?;
        if context.queue_epoch != 0
            || counters != (context.completed_write, context.completed_write)
        {
            return Err("TP2 dependency requires actual drained same-epoch frontier".into());
        }
        context.last_observed_read = counters.1;
    }
    Ok(())
}

pub(super) fn release_private(group: &mut Gfx950EngineeringPeerGroupV1) -> Result<()> {
    let Some(owner) = group.dependency_collective.as_mut() else {
        return Ok(());
    };
    current(&mut group.contexts, owner)?;
    require_quiescent(&mut group.contexts, owner.phase)?;
    for rank in 0..2 {
        owner.mappings[rank].release(&mut SignalTransaction {
            contexts: &mut group.contexts,
            routes: &owner.routes,
            owner: rank,
        })?;
    }
    owner.phase = Phase::Released;
    Ok(())
}

fn address(value: u64) -> Result<ObservedGpuAddressV1> {
    ObservedGpuAddressV1::new(value).map_err(explain)
}

struct Packet {
    bytes: [u8; 64],
    header: u16,
}

#[derive(Default)]
struct PacketCapture {
    bytes: Option<[u8; 64]>,
    header: Option<u16>,
}

impl PacketCapture {
    fn body(&mut self, bytes: [u8; 64]) -> Result<()> {
        if self.bytes.replace(bytes).is_some() || self.header.is_some() {
            return Err("TP2 duplicate staged packet body".into());
        }
        Ok(())
    }
    fn header(&mut self, header: u16) -> Result<()> {
        if self.bytes.is_none() || self.header.replace(header).is_some() {
            return Err("TP2 staged packet header order".into());
        }
        Ok(())
    }
    fn finish(self) -> Result<Packet> {
        Ok(Packet {
            bytes: self.bytes.ok_or("TP2 missing body")?,
            header: self.header.ok_or("TP2 missing header")?,
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

fn kernel_packet(
    prepared: PreparedDispatch,
    context: &mut Context,
    argument_slot: usize,
    signal_slot: usize,
) -> Result<Packet> {
    if argument_slot > 1
        || prepared.bytes.len() > ARGUMENT_SLOT_BYTES
        || prepared.alignment > ARGUMENT_SLOT_BYTES as u64
    {
        return Err("TP2 private argument slot bounds".into());
    }
    let offset = argument_slot * ARGUMENT_SLOT_BYTES;
    Backend::with_bytes_mut(
        &mut context.dependency_internal[ARGUMENT_STORAGE].mapping,
        PAGE_BYTES,
        |bytes| {
            bytes[offset..offset + ARGUMENT_SLOT_BYTES].fill(0);
            bytes[offset..offset + prepared.bytes.len()].copy_from_slice(&prepared.bytes);
        },
    );
    let mut capture = PacketCapture::default();
    AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
        prepared.geometry,
        0,
        prepared.group_bytes,
        address(prepared.descriptor)?,
        address(context.dependency_internal[ARGUMENT_STORAGE].va + offset as u64)?,
        prepared.alignment,
        address(context.dependency_internal[SIGNAL_STORAGE].va + signal_slot as u64 * 64)?,
        if argument_slot == 0 {
            AqlDispatchOrderingV1::Independent
        } else {
            AqlDispatchOrderingV1::WaitForPrior
        },
    )
    .map_err(explain)?
    .publish_with(&mut capture)?;
    capture.finish()
}

struct Ready {
    queues: [Gfx950EngineeringTp2DependencyQueueV1; 2],
    slots: [[u32; 3]; 2],
    packets: [[Packet; 3]; 2],
}

fn stage(
    contexts: &mut [Context],
    prepared: [[PreparedDispatch; 2]; 2],
    phase: Phase,
) -> Result<Ready> {
    require_quiescent(contexts, phase)?;
    if phase == Phase::Completed {
        for context in contexts.iter_mut() {
            for slot in 0..SIGNALS_PER_RANK {
                Backend::reset_completion_signal_release(
                    &mut context.dependency_internal[SIGNAL_STORAGE].mapping,
                    PAGE_BYTES,
                    slot as u32,
                )
                .map_err(explain)?;
            }
        }
    }
    let dependencies = [
        address(contexts[0].dependency_internal[SIGNAL_STORAGE].va)?,
        address(contexts[1].dependency_internal[SIGNAL_STORAGE].va)?,
    ];
    let mut packet_rows = Vec::with_capacity(2);
    for (rank, [producer, consumer]) in prepared.into_iter().enumerate() {
        let producer = kernel_packet(producer, &mut contexts[rank], 0, 0)?;
        let consumer = kernel_packet(consumer, &mut contexts[rank], 1, 2)?;
        let mut barrier = PacketCapture::default();
        AqlDependencyBarrierAndPacketV1::new_unpublished(
            &dependencies,
            address(contexts[rank].dependency_internal[SIGNAL_STORAGE].va + 64)?,
        )
        .map_err(explain)?
        .publish_with(&mut barrier)?;
        let packets = [producer, barrier.finish()?, consumer];
        if packets.each_ref().map(|packet| packet.header) != [0x1402, 0x1503, 0x1502] {
            return Err("TP2 fixed packet header identity".into());
        }
        packet_rows.push(packets);
    }
    let packets: [[Packet; 3]; 2] = packet_rows.try_into().map_err(|_| "TP2 packet roster")?;
    let mut queues = [Gfx950EngineeringTp2DependencyQueueV1 {
        unique_id: 0,
        queue_epoch: 0,
        first_packet: 0,
        next_packet: 0,
    }; 2];
    let mut slots = [[0; 3]; 2];
    for rank in 0..2 {
        let context = &mut contexts[rank];
        let reservation = context
            .ring
            .reserve_batch(context.last_observed_read, 3)
            .map_err(explain)?;
        queues[rank] = Gfx950EngineeringTp2DependencyQueueV1 {
            unique_id: context.unique_id,
            queue_epoch: context.queue_epoch,
            first_packet: reservation.first_packet_id(),
            next_packet: reservation.next_write(),
        };
        for slot in 0..3 {
            slots[rank][slot] =
                ((queues[rank].first_packet + slot as u64) % (RING_BYTES as u64 / 64)) as u32;
            Backend::write_aql_slot(
                &mut context.internal[RING].mapping,
                RING_BYTES,
                slots[rank][slot],
                &packets[rank][slot].bytes,
            )
            .map_err(explain)?;
        }
        let previous =
            Backend::fetch_add_aql_write(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 3)
                .map_err(explain)?;
        if previous != queues[rank].first_packet {
            return Err("TP2 queue reservation substitution".into());
        }
    }
    if signals(contexts)? != [[1; 3]; 2] {
        return Err("TP2 staged signals not pending".into());
    }
    Ok(Ready {
        queues,
        slots,
        packets,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Observation {
    values: [[i64; 3]; 2],
    frontiers: [[u64; 2]; 2],
}

#[derive(Default)]
struct Diagnostic {
    phase: &'static str,
    last: Option<Observation>,
    observed_at: Option<Duration>,
    header_attempted: [[bool; 3]; 2],
    header_completed: [[bool; 3]; 2],
    doorbell_attempted: [bool; 2],
    doorbell_completed: [bool; 2],
}

trait ProtocolBackend {
    fn diagnostic(&mut self) -> &mut Diagnostic;
    fn now(&self) -> Duration;
    fn refresh(&mut self) -> Result<()>;
    fn observe(&mut self) -> Result<Observation>;
    fn publish(&mut self, rank: usize) -> Result<()>;
    fn retire(&mut self) -> Result<()>;
    fn pause(&mut self);
}

fn checked_observation(
    backend: &mut impl ProtocolBackend,
    previous: &mut Observation,
    next: [u64; 2],
) -> Result<Observation> {
    let observation = backend.observe()?;
    let at = backend.now();
    backend.diagnostic().last = Some(observation);
    backend.diagnostic().observed_at = Some(at);
    for (rank, expected_write) in next.into_iter().enumerate() {
        validate_counters(
            expected_write,
            previous.frontiers[rank][1],
            (
                observation.frontiers[rank][0],
                observation.frontiers[rank][1],
            ),
        )?;
        for slot in 0..3 {
            let value = observation.values[rank][slot];
            if !matches!(value, 0 | 1) || previous.values[rank][slot] == 0 && value != 0 {
                return Err("TP2 completion regressed or changed value".into());
            }
        }
    }
    *previous = observation;
    Ok(observation)
}

// No reset, allocation, body mutation, free or retry operation exists here.
fn protocol(
    backend: &mut impl ProtocolBackend,
    first: [u64; 2],
    next: [u64; 2],
) -> Result<Observation> {
    let deadline = backend
        .now()
        .checked_add(DEADLINE)
        .ok_or("TP2 deadline overflow")?;
    if first.map(|value| value.checked_add(3)) != next.map(Some) {
        return Err("TP2 exact packet reservation".into());
    }
    backend.diagnostic().phase = "initial_fence";
    backend.refresh()?;
    if backend.now() >= deadline {
        return Err("TP2 initial currentness deadline".into());
    }
    let mut previous = Observation {
        values: [[1; 3]; 2],
        frontiers: [[next[0], first[0]], [next[1], first[1]]],
    };
    checked_observation(backend, &mut previous, next)?;
    if backend.now() >= deadline {
        return Err("TP2 initial observation deadline".into());
    }
    if previous.values != [[1; 3]; 2]
        || previous.frontiers != [[next[0], first[0]], [next[1], first[1]]]
    {
        return Err("TP2 initial staging observation".into());
    }
    for rank in 0..2 {
        backend.diagnostic().phase = if rank == 0 {
            "publish_rank_zero"
        } else {
            "publish_rank_one"
        };
        backend.refresh()?;
        if backend.now() >= deadline {
            return Err("TP2 publication currentness deadline".into());
        }
        backend.publish(rank)?;
    }
    backend.diagnostic().phase = "drain";
    let mut next_refresh = backend.now();
    loop {
        if backend.now() >= deadline {
            return Err("TP2 dependency drain deadline; terminal teardown required".into());
        }
        let observation = checked_observation(backend, &mut previous, next)?;
        if backend.now() >= deadline {
            return Err("TP2 dependency drain deadline; terminal teardown required".into());
        }
        if observation.values == [[0; 3]; 2]
            && observation.frontiers == [[next[0]; 2], [next[1]; 2]]
        {
            backend.diagnostic().phase = "final_fence";
            backend.refresh()?;
            if backend.now() >= deadline {
                return Err("TP2 final currentness/drain fence".into());
            }
            let final_observation = checked_observation(backend, &mut previous, next)?;
            if backend.now() >= deadline || final_observation != observation {
                return Err("TP2 final currentness/drain fence".into());
            }
            backend.diagnostic().phase = "retire";
            backend.retire()?;
            if backend.now() >= deadline {
                return Err("TP2 final retirement currentness deadline".into());
            }
            return Ok(final_observation);
        }
        if backend.now() >= next_refresh {
            backend.refresh()?;
            next_refresh = backend
                .now()
                .checked_add(Duration::from_millis(100))
                .ok_or("TP2 refresh deadline overflow")?;
        }
        backend.pause();
    }
}

struct NativeProtocol<'a> {
    contexts: &'a mut [Context],
    owner: &'a DependencyOwner,
    ready: &'a Ready,
    started: Instant,
    diagnostic: Diagnostic,
}

impl ProtocolBackend for NativeProtocol<'_> {
    fn diagnostic(&mut self) -> &mut Diagnostic {
        &mut self.diagnostic
    }
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
    fn refresh(&mut self) -> Result<()> {
        current(self.contexts, self.owner)
    }
    fn observe(&mut self) -> Result<Observation> {
        let values = signals(self.contexts)?;
        let mut frontiers = [[0; 2]; 2];
        for (rank, frontier) in frontiers.iter_mut().enumerate() {
            let context = &mut self.contexts[rank];
            let queue = self.ready.queues[rank];
            if (context.unique_id, context.queue_epoch, context.ring.write())
                != (queue.unique_id, queue.queue_epoch, queue.next_packet)
            {
                return Err("TP2 queue identity substitution".into());
            }
            let counters =
                Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(explain)?;
            validate_counters(queue.next_packet, context.last_observed_read, counters)?;
            if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
                .map_err(explain)?
                != 0
            {
                return Err("TP2 queue exception".into());
            }
            context.last_observed_read = counters.1;
            *frontier = [counters.0, counters.1];
        }
        Ok(Observation { values, frontiers })
    }
    fn publish(&mut self, rank: usize) -> Result<()> {
        let context = &mut self.contexts[rank];
        for slot in 0..3 {
            self.diagnostic.header_attempted[rank][slot] = true;
            if slot == 1 {
                Backend::publish_engineering_dependency_header(
                    &mut context.internal[RING].mapping,
                    RING_BYTES,
                    self.ready.slots[rank][slot],
                )
                .map_err(explain)?;
            } else {
                Backend::publish_aql_header(
                    &mut context.internal[RING].mapping,
                    RING_BYTES,
                    self.ready.slots[rank][slot],
                    self.ready.packets[rank][slot].header,
                )
                .map_err(explain)?;
            }
            self.diagnostic.header_completed[rank][slot] = true;
        }
        self.diagnostic.doorbell_attempted[rank] = true;
        context
            .doorbell
            .as_mut()
            .ok_or("missing TP2 doorbell")?
            .store_packet_id_release(self.ready.queues[rank].next_packet - 1)
            .map_err(explain)?;
        self.diagnostic.doorbell_completed[rank] = true;
        Ok(())
    }
    fn retire(&mut self) -> Result<()> {
        for (rank, context) in self.contexts.iter_mut().enumerate() {
            context.completed_write = self.ready.queues[rank].next_packet;
            context.check_idle()?;
        }
        Ok(())
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_micros(50));
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Opens the closed TP2 collective mode with immutable GTT/UNCACHED CONTROL.
    /// This is an allocation-policy experiment, not a cache-bit causal claim.
    ///
    /// # Safety
    /// Requires the same dedicated disposable process and terminal-error policy
    /// as `open_unchecked`; no other KFD client or GPU access may coexist.
    pub unsafe fn open_tp2_dependency_unchecked(unique_ids: &[u64; 2]) -> Result<Self> {
        // SAFETY: caller accepts the dedicated-process engineering contract.
        let mut group = unsafe {
            Self::open_with_queue_control_unchecked(
                unique_ids,
                QueueControlPolicy::DependencyCanaryGttUncached,
            )
        }?;
        let result = (|| {
            let sources = source_contract()?;
            let gpu_ids = [
                group.contexts[0].backend.gpu_id(),
                group.contexts[1].backend.gpu_id(),
            ];
            let routes = EngineeringGttSignalRoutesV1::observe(
                group.contexts[0].backend.engineering_peer_topology(),
                gpu_ids,
            )?;
            // The ledger and all native owners are retained before fallible mapping.
            group.dependency_collective = Some(DependencyOwner {
                routes,
                sources,
                mappings: [
                    PeerMapping::new(vec![gpu_ids[1]])?,
                    PeerMapping::new(vec![gpu_ids[0]])?,
                ],
                next_generation: 1,
                last_request: 0,
                phase: Phase::Fresh,
            });
            initialize_storage(&mut group.contexts)?;
            let owner = group
                .dependency_collective
                .as_mut()
                .ok_or("missing TP2 owner")?;
            for rank in 0..2 {
                owner.mappings[rank].map(&mut SignalTransaction {
                    contexts: &mut group.contexts,
                    routes: &owner.routes,
                    owner: rank,
                })?;
            }
            current(&mut group.contexts, owner)?;
            require_quiescent(&mut group.contexts, Phase::Fresh)
        })();
        group.finish(result)?;
        Ok(group)
    }

    /// Runs exactly P/B/C on each rank under one exclusive group borrow.
    /// Returns only after six acquire-zero completions and both real read drains.
    ///
    /// # Safety
    /// The two admitted MFMA producers and ordered-residual consumers are expert
    /// unauthenticated code. They must honor every declared access, shape, and
    /// termination contract. An error is terminal; no reuse or rollback is safe.
    pub unsafe fn dispatch_tp2_dependency_collective_unchecked(
        &mut self,
        request: Gfx950EngineeringTp2DependencyRequestV1<'_>,
    ) -> Result<Gfx950EngineeringTp2DependencyReceiptV1> {
        self.require_active()?;
        let result = (|| {
            let owner = self
                .dependency_collective
                .as_ref()
                .ok_or("TP2 dependency mode not enabled")?;
            if self.contexts.len() != 2
                || self.shared_full_currentness
                || self.contexts.iter().any(|context| {
                    context.queue_control_policy != QueueControlPolicy::DependencyCanaryGttUncached
                        || context.performance.is_some_and(|options| {
                            options.operational_currentness || options.profile
                        })
                })
            {
                return Err("TP2 dependency mode/profile changed".into());
            }
            let next_generation =
                next_identity(request.identity, owner.next_generation, owner.last_request)?;
            graph_contract(&request)?;
            current(&mut self.contexts, owner)?;
            require_quiescent(&mut self.contexts, owner.phase)?;
            let prior_phase = owner.phase;
            let identity = request.identity;
            let [p0, p1] = request.producers;
            let [c0, c1] = request.consumers;
            let mut prepared = Vec::with_capacity(2);
            for [producer, consumer] in [[p0, c0], [p1, c1]] {
                let p = self.prepare_peer_dispatch(
                    producer.kernel,
                    producer.bytes,
                    producer.workgroup,
                    producer.grid,
                    &producer.pointers,
                    producer.timeout_ms,
                )?;
                let c = self.prepare_peer_dispatch(
                    consumer.kernel,
                    consumer.bytes,
                    consumer.workgroup,
                    consumer.grid,
                    &consumer.pointers,
                    consumer.timeout_ms,
                )?;
                prepared.push([p, c]);
            }
            let owner = self
                .dependency_collective
                .as_mut()
                .ok_or("missing TP2 owner")?;
            owner.phase = Phase::Preparing;
            let ready = stage(
                &mut self.contexts,
                prepared.try_into().map_err(|_| "TP2 prepared roster")?,
                prior_phase,
            )?;
            let mut native = NativeProtocol {
                contexts: &mut self.contexts,
                owner,
                ready: &ready,
                started: Instant::now(),
                diagnostic: Diagnostic::default(),
            };
            let outcome = protocol(
                &mut native,
                ready.queues.map(|queue| queue.first_packet),
                ready.queues.map(|queue| queue.next_packet),
            );
            let observation = match outcome {
                Ok(value) => value,
                Err(error) => {
                    // Host-only diagnostic: no extra device observation after failure.
                    let diagnostic = &native.diagnostic;
                    return Err(format!(
                        "{error}; generation={} phase={} last={:?} observed_at={:?} headers_attempted={:?} headers_completed={:?} doorbells_attempted={:?} doorbells_completed={:?}",
                        identity.generation,
                        diagnostic.phase,
                        diagnostic.last,
                        diagnostic.observed_at,
                        diagnostic.header_attempted,
                        diagnostic.header_completed,
                        diagnostic.doorbell_attempted,
                        diagnostic.doorbell_completed
                    ));
                }
            };
            owner.phase = Phase::Completed;
            owner.next_generation = next_generation;
            owner.last_request = identity.request_id;
            Ok(Gfx950EngineeringTp2DependencyReceiptV1 {
                identity,
                queues: ready.queues,
                kernel_counts: [2; 2],
                barrier_counts: [1; 2],
                packet_counts: [3; 2],
                completion_values: observation.values,
                final_frontiers: observation.frontiers,
            })
        })();
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_dependency_tests.rs"]
mod tests;
