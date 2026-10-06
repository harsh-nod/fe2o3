//! Fresh paired signal/kernarg custody. No signal reset or capacity fabrication.
use super::*;
use fe2o3_aql::{
    AMD_SIGNAL_BYTES_V1, AMD_SIGNAL_KIND_USER_V1, AqlDispatchOrderingV1, AqlPeerBarrierAndPacketV1,
    AqlPeerPacketBatchPublicationTargetV1, AqlPreparedPeerPacketBatchV1, AqlPreparedPeerPacketV1,
    AqlRingBatchReservationV1,
};

pub(super) const PACKETS: usize = 5;
pub(super) const KERNEL_SLOTS: [usize; 4] = [0, 1, 2, 4];
pub(super) const ARENA_BYTES: usize = PAGE_BYTES + 4 * MAX_KERNARG_BYTES_V1 as usize;
type Batch = AqlPreparedPeerPacketBatchV1<PACKETS>;
type Values = [[i64; PACKETS]; 2];

#[path = "engineering_gfx950_peer_combined_mlp_paired_retired_v1.rs"]
mod retired;
pub(super) use retired::Retired;

pub(super) fn signal_value(kind: i64, value: i64) -> Result<i64> {
    if kind != AMD_SIGNAL_KIND_USER_V1 || !matches!(value, 0 | 1) {
        return Err("paired guarded MLP signal kind/value".into());
    }
    Ok(value)
}

pub(super) fn before_publication(values: Values, published: [bool; 2], rank: usize) -> Result<()> {
    if values
        .iter()
        .flatten()
        .any(|&value| !matches!(value, 0 | 1))
    {
        return Err("paired guarded MLP unexpected pending signal".into());
    }
    match rank {
        0 if published == [false; 2] && values == [[1; PACKETS]; 2] => Ok(()),
        // Rank0 may finish R1/MLP/validator before rank1 is published. Its
        // barrier and R2 cannot finish while rank1's validator is unpublished.
        1 if published == [true, false]
            && values[1] == [1; PACKETS]
            && values[0][3..] == [1, 1] =>
        {
            Ok(())
        }
        _ => Err("paired guarded MLP publication order/pending contract".into()),
    }
}

pub(super) fn completion_gate(
    values: Values,
    published: [bool; 2],
    counters: [(u64, u64); 2],
    next: [u64; 2],
    previous_reads: [u64; 2],
) -> Result<bool> {
    if published != [true; 2] || values.iter().flatten().any(|&v| !matches!(v, 0 | 1)) {
        return Err(
            "paired guarded MLP completion before paired publication or invalid signal".into(),
        );
    }
    let complete = values.iter().flatten().all(|&v| v == 0);
    for rank in 0..2 {
        dispatch_completed(
            next[rank],
            previous_reads[rank],
            counters[rank],
            if complete {
                AqlCompletionObservationV1::Completed
            } else {
                AqlCompletionObservationV1::Pending
            },
            0,
        )?;
    }
    Ok(complete)
}

struct Arena {
    token: Gfx950EngineeringPeerBufferV1,
    local: u64,
    allocation: [u64; 4],
    participant: [u64; 4],
    peer_gpu: u32,
}

impl Arena {
    fn check(&self, group: &Gfx950EngineeringPeerGroupV1) -> Result<()> {
        let record = group.validate_token(self.token)?;
        let rank = self.token.owner;
        let context = group
            .contexts
            .get(rank)
            .ok_or("paired arena rank missing")?;
        let allocation = context
            .buffers
            .get(&self.local)
            .ok_or("paired arena backing missing")?;
        if rank >= 2
            || record.local_id != self.local
            || record.kind != BufferKind::PeerDependencyArena
            || record.mapping.phase != Phase::PeersMapped
            || record.mapping.peers != [self.peer_gpu]
            || record.mapping.mapped != 1
            || record.mapping.unmapped != 0
            || self.peer_gpu != group.contexts[1 - rank].backend.gpu_id()
            || self.token.bytes != ARENA_BYTES as u64
            || self.allocation
                != [
                    allocation.handle,
                    allocation.va,
                    allocation.requested as u64,
                    allocation.backing as u64,
                ]
            || self.participant
                != [
                    context.unique_id,
                    u64::from(context.backend.gpu_id()),
                    context.queue_epoch,
                    u64::from(context.queue_id.ok_or("paired arena queue missing")?),
                ]
        {
            return Err("paired arena allocation/mapping/queue identity changed".into());
        }
        Ok(())
    }

    fn signal(&self, slot: usize) -> Result<ObservedGpuAddressV1> {
        if slot >= PACKETS {
            return Err("paired arena signal slot".into());
        }
        ObservedGpuAddressV1::new(
            self.allocation[1]
                .checked_add((slot * AMD_SIGNAL_BYTES_V1) as u64)
                .ok_or("paired arena signal overflow")?,
        )
        .map_err(explain)
    }

    fn kernarg(&self, index: usize) -> Result<ObservedGpuAddressV1> {
        if index >= 4 {
            return Err("paired arena kernarg slot".into());
        }
        ObservedGpuAddressV1::new(
            self.allocation[1]
                .checked_add((PAGE_BYTES + index * MAX_KERNARG_BYTES_V1 as usize) as u64)
                .ok_or("paired arena kernarg overflow")?,
        )
        .map_err(explain)
    }
}

fn allocate(
    group: &mut Gfx950EngineeringPeerGroupV1,
    rank: usize,
    prepared: &[PreparedDispatch; 4],
) -> Result<Arena> {
    if rank >= 2
        || group.contexts.len() != 2
        || prepared
            .iter()
            .any(|p| p.bytes.len() > MAX_KERNARG_BYTES_V1 as usize)
    {
        return Err("paired arena rank/kernarg shape".into());
    }
    check_contexts(&mut group.contexts, group.shared_full_currentness)?;
    let id = group.next_buffer;
    let local = group.contexts[rank].next_buffer;
    let peer_gpu = group.contexts[1 - rank].backend.gpu_id();
    let mapping = PeerMapping::new(vec![peer_gpu])?;
    if group.buffers.len() >= group_allocation_limit(2)?
        || group.contexts[rank].buffers.len() >= MAX_ALLOCATIONS
    {
        return Err("paired arena allocation capacity".into());
    }
    group.next_buffer = id
        .checked_add(1)
        .ok_or("paired arena group identity exhausted")?;
    group.contexts[rank].next_buffer = local
        .checked_add(1)
        .ok_or("paired arena local identity exhausted")?;
    let allocation = group.contexts[rank].allocate_resource(
        ARENA_BYTES,
        KfdAllocMemoryFlags::KERNARG,
        |bytes| {
            // The only ordinary-byte initialization precedes all signal lifetimes.
            for (i, command) in prepared.iter().enumerate() {
                let offset = PAGE_BYTES + i * MAX_KERNARG_BYTES_V1 as usize;
                bytes[offset..offset + command.bytes.len()].copy_from_slice(&command.bytes);
            }
            Ok(())
        },
    )?;
    let identity = [
        allocation.handle,
        allocation.va,
        allocation.requested as u64,
        allocation.backing as u64,
    ];
    group.contexts[rank].buffers.insert(local, allocation);
    let token = Gfx950EngineeringPeerBufferV1 {
        group: group.incarnation,
        id,
        owner: rank,
        bytes: ARENA_BYTES as u64,
    };
    group.buffers.insert(
        id,
        BufferRecord {
            token,
            local_id: local,
            mapping,
            kind: BufferKind::PeerDependencyArena,
        },
    );
    let end = identity[1]
        .checked_add(
            identity[3]
                .checked_sub(1)
                .ok_or("paired arena empty backing")?,
        )
        .ok_or("paired arena peer aperture overflow")?;
    let aperture = group.contexts[1 - rank].backend.gpuvm_aperture();
    if identity[1] < aperture.base() || end > aperture.limit() {
        return Err("paired arena peer aperture".into());
    }
    Backend::initialize_engineering_signal_slots(
        &mut group.contexts[rank]
            .buffers
            .get_mut(&local)
            .ok_or("paired arena new backing missing")?
            .mapping,
        PACKETS,
    )
    .map_err(explain)?;
    group
        .buffers
        .get_mut(&id)
        .ok_or("paired arena new record missing")?
        .mapping
        .map(&mut NativeTransaction {
            contexts: &mut group.contexts,
            shared_full_currentness: group.shared_full_currentness,
            owner: rank,
            local_id: local,
        })?;
    let context = &group.contexts[rank];
    let arena = Arena {
        token,
        local,
        allocation: identity,
        participant: [
            context.unique_id,
            u64::from(context.backend.gpu_id()),
            context.queue_epoch,
            u64::from(context.queue_id.ok_or("paired arena queue absent")?),
        ],
        peer_gpu,
    };
    arena.check(group)?;
    Ok(arena)
}

pub(super) fn make_batch(
    rank: usize,
    prepared: [PreparedDispatch; 4],
    signals: [[ObservedGpuAddressV1; PACKETS]; 2],
    kernargs: [ObservedGpuAddressV1; 4],
) -> Result<Batch> {
    if rank >= 2
        || signals
            .iter()
            .flatten()
            .map(|s| s.raw())
            .collect::<BTreeSet<_>>()
            .len()
            != 10
    {
        return Err("paired guarded MLP distinct completion slots".into());
    }
    let mut kernels = Vec::with_capacity(4);
    for (i, command) in prepared.into_iter().enumerate() {
        kernels.push(AqlPreparedPeerPacketV1::Kernel(
            AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
                command.geometry,
                0,
                command.group_bytes,
                ObservedGpuAddressV1::new(command.descriptor).map_err(explain)?,
                kernargs[i],
                command.alignment,
                signals[rank][KERNEL_SLOTS[i]],
                AqlDispatchOrderingV1::WaitForPrior,
            )
            .map_err(explain)?,
        ));
    }
    let barrier = AqlPreparedPeerPacketV1::Barrier(
        AqlPeerBarrierAndPacketV1::new_unpublished(
            [signals[0][2], signals[1][2]],
            signals[rank][3],
        )
        .map_err(explain)?,
    );
    let r2 = kernels.pop().ok_or("paired R2 packet missing")?;
    kernels.push(barrier);
    kernels.push(r2);
    Batch::try_from_packets(kernels.try_into().map_err(|_| "paired five-packet shape")?)
        .map_err(explain)
}

pub(super) struct Staged {
    arenas: [Arena; 2],
    reservations: [Option<AqlRingBatchReservationV1>; 2],
    batches: [Option<Batch>; 2],
    published: [bool; 2],
    retired: [bool; 2],
    pub(super) poisoned: bool,
    pub(super) last: Option<(Values, [(u64, u64); 2])>,
}

impl Staged {
    pub(super) fn prepare(
        group: &mut Gfx950EngineeringPeerGroupV1,
        prepared: [[PreparedDispatch; 4]; 2],
    ) -> Result<Self> {
        let arenas = [
            allocate(group, 0, &prepared[0])?,
            allocate(group, 1, &prepared[1])?,
        ];
        let signals = arenas.each_ref().map(|a| {
            (0..PACKETS)
                .map(|i| a.signal(i))
                .collect::<Result<Vec<_>>>()
        });
        let [left, right] = signals;
        let signals = [
            left?.try_into().map_err(|_| "paired signal count")?,
            right?.try_into().map_err(|_| "paired signal count")?,
        ];
        let mut batches = Vec::with_capacity(2);
        for (rank, row) in prepared.into_iter().enumerate() {
            let kernargs = (0..4)
                .map(|i| arenas[rank].kernarg(i))
                .collect::<Result<Vec<_>>>()?
                .try_into()
                .map_err(|_| "paired kernarg count")?;
            batches.push(Some(make_batch(rank, row, signals, kernargs)?));
        }
        let mut result = Self {
            arenas,
            reservations: [None, None],
            batches: batches.try_into().map_err(|_| "paired batch count")?,
            published: [false; 2],
            retired: [false; 2],
            poisoned: false,
            last: None,
        };
        if result.read_signals(group)? != [[1; PACKETS]; 2] {
            return Err("paired initial signal readback".into());
        }
        Ok(result)
    }

    pub(super) fn check(&self, group: &Gfx950EngineeringPeerGroupV1) -> Result<()> {
        if self.poisoned || group.contexts.len() != 2 {
            return Err("paired arena poisoned or wrong group size".into());
        }
        for arena in &self.arenas {
            arena.check(group)?;
        }
        Ok(())
    }

    fn read_signals(&mut self, group: &mut Gfx950EngineeringPeerGroupV1) -> Result<Values> {
        self.check(group)?;
        let mut values = [[1; PACKETS]; 2];
        for (rank, row) in values.iter_mut().enumerate() {
            let allocation = group.contexts[rank]
                .buffers
                .get_mut(&self.arenas[rank].local)
                .ok_or("paired observed arena missing")?;
            for (slot, value) in row.iter_mut().enumerate() {
                let (kind, observed) = Backend::observe_completion_signal_state_acquire(
                    &mut allocation.mapping,
                    PAGE_BYTES,
                    slot as u32,
                )
                .map_err(explain)?;
                *value = signal_value(kind, observed)?;
            }
        }
        Ok(values)
    }

    pub(super) fn reserve(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        rank: usize,
    ) -> Result<()> {
        self.check(group)?;
        if rank >= 2
            || self.reservations[rank].is_some()
            || self.published != [false; 2]
            || (rank == 1 && self.reservations[0].is_none())
        {
            return Err("paired reservation order".into());
        }
        let context = &mut group.contexts[rank];
        require_sequence_capacity(context.ring.write(), context.last_observed_read, PACKETS)?;
        self.reservations[rank] = Some(
            context
                .ring
                .reserve_fixed_batch_v2(context.last_observed_read, PACKETS as u32)
                .map_err(explain)?,
        );
        Ok(())
    }

    fn observe(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        deadline: Instant,
    ) -> Result<(Values, [(u64, u64); 2])> {
        deadline_check(Instant::now(), deadline)?;
        self.check(group)?;
        let mut frontiers = [(0, 0); 2];
        for rank in 0..2 {
            let context = &mut group.contexts[rank];
            let reservation = self.reservations[rank]
                .as_ref()
                .ok_or("paired observation without reservation")?;
            if context.ring.write() != reservation.next_write() {
                return Err("paired reserved frontier drift".into());
            }
            let expected = if self.published[rank] {
                reservation.next_write()
            } else {
                reservation.first_packet_id()
            };
            let counters =
                Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(explain)?;
            validate_counters(expected, context.last_observed_read, counters)?;
            context.last_observed_read = counters.1;
            if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
                .map_err(explain)?
                != 0
            {
                return Err("paired guarded MLP queue exception".into());
            }
            frontiers[rank] = counters;
        }
        let values = self.read_signals(group)?;
        self.last = Some((values, frontiers));
        deadline_check(Instant::now(), deadline)?;
        Ok((values, frontiers))
    }

    pub(super) fn publish(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        rank: usize,
        deadline: Instant,
    ) -> Result<()> {
        let (values, _) = self.observe(group, deadline)?;
        before_publication(values, self.published, rank)?;
        let batch = self.batches[rank]
            .take()
            .ok_or("paired batch already consumed")?;
        let context = &mut group.contexts[rank];
        let reservation = self.reservations[rank]
            .as_ref()
            .ok_or("paired publication without reservation")?;
        let prior = Backend::fetch_add_aql_write(
            &mut context.internal[CONTROL].mapping,
            PAGE_BYTES,
            PACKETS as u64,
        )
        .map_err(explain)?;
        if prior != reservation.first_packet_id() {
            return Err("paired hardware reservation mismatch".into());
        }
        batch.publish_with(&mut PublicationTarget {
            context,
            reservation,
            deadline,
        })?;
        self.published[rank] = true;
        round::fresh_publication_fence(group)?;
        self.observe(group, deadline)?;
        group.contexts[rank]
            .doorbell
            .as_mut()
            .ok_or("paired doorbell missing")?
            .store_packet_id_release(
                self.reservations[rank]
                    .as_ref()
                    .ok_or("paired reservation missing")?
                    .last_packet_id(),
            )
            .map_err(explain)
    }

    pub(super) fn complete(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        deadline: Instant,
    ) -> Result<bool> {
        if self.published != [true; 2] {
            return Err("paired poll before both publications".into());
        }
        let previous = std::array::from_fn(|rank| group.contexts[rank].last_observed_read);
        let (values, counters) = self.observe(group, deadline)?;
        let next = [
            self.reservations[0]
                .as_ref()
                .ok_or("paired rank0 reservation missing")?
                .next_write(),
            self.reservations[1]
                .as_ref()
                .ok_or("paired rank1 reservation missing")?
                .next_write(),
        ];
        completion_gate(values, self.published, counters, next, previous)
    }

    pub(super) fn retire(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
        rank: usize,
        deadline: Instant,
    ) -> Result<()> {
        if rank >= 2
            || self.retired[rank]
            || (rank == 1 && !self.retired[0])
            || !self.complete(group, deadline)?
        {
            return Err("paired retirement requires both complete batches".into());
        }
        // Logical completed work only; actual read credit is never advanced here.
        group.contexts[rank].completed_write = self.reservations[rank]
            .as_ref()
            .ok_or("paired retirement reservation missing")?
            .next_write();
        self.retired[rank] = true;
        deadline_check(Instant::now(), deadline)
    }

    pub(super) fn require_retired(&self) -> Result<()> {
        if self.poisoned || self.published != [true; 2] || self.retired != [true; 2] {
            return Err("paired owner completion before both logical retirements".into());
        }
        Ok(())
    }
}

struct PublicationTarget<'a> {
    context: &'a mut Context,
    reservation: &'a AqlRingBatchReservationV1,
    deadline: Instant,
}

impl PublicationTarget<'_> {
    fn write(&mut self, index: u32, bytes: [u8; 64]) -> Result<()> {
        deadline_check(Instant::now(), self.deadline)?;
        let slot = self
            .reservation
            .entry(index)
            .ok_or("paired packet slot")?
            .slot_index();
        let ring = &mut self.context.internal[RING];
        Backend::write_aql_slot(&mut ring.mapping, ring.requested, slot, &bytes).map_err(explain)
    }
}

impl AqlPeerPacketBatchPublicationTargetV1 for PublicationTarget<'_> {
    type Error = String;
    fn write_unpublished_kernel(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<()> {
        self.write(index, packet.encode_unpublished_le())
    }
    fn write_unpublished_barrier(
        &mut self,
        index: u32,
        packet: &AqlPeerBarrierAndPacketV1,
    ) -> Result<()> {
        self.write(index, packet.encode_unpublished_le())
    }
    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<()> {
        deadline_check(Instant::now(), self.deadline)?;
        let slot = self
            .reservation
            .entry(index)
            .ok_or("paired header slot")?
            .slot_index();
        let ring = &mut self.context.internal[RING];
        Backend::publish_engineering_peer_aql_header(
            &mut ring.mapping,
            ring.requested,
            slot,
            header,
        )
        .map_err(explain)
    }
}
