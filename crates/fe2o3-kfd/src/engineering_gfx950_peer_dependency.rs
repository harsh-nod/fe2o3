//! One-shot two-rank dependency sentinel, not a general graph scheduler.

use super::*;
use fe2o3_aql::{
    AMD_SIGNAL_BYTES_V1, AMD_SIGNAL_KIND_USER_V1, AqlDispatchOrderingV1, AqlPeerBarrierAndPacketV1,
    AqlPeerPacketBatchPublicationTargetV1, AqlPreparedPeerPacketBatchV1, AqlPreparedPeerPacketV1,
    AqlRingBatchReservationV1,
};

const PACKETS: usize = 7;
const KERNEL_SLOTS: [usize; 4] = [0, 2, 4, 6];
const BARRIER_PRODUCERS: [(usize, usize); 3] = [(1, 0), (3, 2), (5, 4)];
const ARENA_BYTES: usize = PAGE_BYTES + 4 * MAX_KERNARG_BYTES_V1 as usize;
type Values<const N: usize = PACKETS> = [[i64; N]; 2];
type Batch<const N: usize = PACKETS> = AqlPreparedPeerPacketBatchV1<N>;
const TRACE_CHANGE_LIMIT: usize = 16;

fn require_packet_count<const N: usize>() -> Result<()> {
    if !matches!(N, 7 | 8) {
        return Err("dependency graph requires seven or eight packets".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TraceState<const N: usize> {
    values: Values<N>,
    frontiers: [(u64, u64); 2],
    headers: Option<[[(u32, u16, u16); N]; 2]>,
    published: [bool; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimedSnapshot<const N: usize> {
    elapsed_ns: u64,
    state: TraceState<N>,
}

#[derive(Debug)]
struct DependencyTrace<const N: usize> {
    first_all_zero_elapsed_ns: Option<u64>,
    changes: Vec<TimedSnapshot<N>>,
    last: Option<TimedSnapshot<N>>,
    dropped_changes: u64,
}

impl<const N: usize> Default for DependencyTrace<N> {
    fn default() -> Self {
        Self {
            first_all_zero_elapsed_ns: None,
            changes: Vec::new(),
            last: None,
            dropped_changes: 0,
        }
    }
}

impl<const N: usize> DependencyTrace<N> {
    fn record(&mut self, elapsed_ns: u64, state: TraceState<N>) {
        if self.first_all_zero_elapsed_ns.is_none()
            && state.values.iter().flatten().all(|&v| v == 0)
        {
            self.first_all_zero_elapsed_ns = Some(elapsed_ns);
        }
        let snapshot = TimedSnapshot { elapsed_ns, state };
        if self
            .last
            .as_ref()
            .is_none_or(|previous| previous.state != state)
        {
            if self.changes.len() < TRACE_CHANGE_LIMIT {
                self.changes.push(snapshot);
            } else {
                self.dropped_changes = self.dropped_changes.saturating_add(1);
            }
        }
        self.last = Some(snapshot);
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerDependencyObservationV1 {
    /// Whole invocation including preparation, publication, waits and fences.
    pub elapsed_ns: u64,
    pub witness_observed: bool,
    pub completion_count: u32,
}

fn deadline_check(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err("peer dependency aggregate deadline expired; process teardown required".into());
    }
    Ok(())
}

fn witness_ready<const N: usize>(values: Values<N>, published: [bool; 2]) -> Result<bool> {
    require_packet_count::<N>()?;
    if published != [true, false] {
        return Err("dependency witness requires rank1 to remain unpublished".into());
    }
    for (rank, row) in values.iter().enumerate() {
        for (slot, &value) in row.iter().enumerate() {
            if (rank, slot) == (0, 0) {
                if !matches!(value, 0 | 1) {
                    return Err("invalid witness producer signal".into());
                }
            } else if value != 1 {
                return Err("dependent or unpublished signal changed before witness".into());
            }
        }
    }
    Ok(values[0][0] == 0)
}

fn signals_complete<const N: usize>(values: Values<N>) -> Result<bool> {
    require_packet_count::<N>()?;
    if values.iter().flatten().any(|&v| !matches!(v, 0 | 1)) {
        return Err("unexpected peer dependency completion value".into());
    }
    Ok(values.iter().flatten().all(|&v| v == 0))
}

fn complete_at_frontiers<const N: usize>(
    values: Values<N>,
    frontiers: [(u64, u64); 2],
    expected_next: [u64; 2],
    published: [bool; 2],
) -> Result<bool> {
    Ok(signals_complete(values)?
        && published == [true; 2]
        && frontiers
            .iter()
            .zip(expected_next)
            .all(|(&actual, next)| actual == (next, next)))
}

trait DependencyBackend {
    fn fence(&mut self) -> Result<()>;
    fn publish(&mut self, rank: usize) -> Result<()>;
    fn witness(&mut self) -> Result<bool>;
    fn complete(&mut self) -> Result<bool>;
    fn retire(&mut self) -> Result<()>;
    fn pause(&mut self);
    fn poison(&mut self);
}

fn run_dependencies(
    backend: &mut impl DependencyBackend,
    witness: bool,
    deadline: Instant,
) -> Result<()> {
    let result = (|| {
        deadline_check(deadline)?;
        backend.fence()?;
        deadline_check(deadline)?;
        backend.publish(0)?;
        if witness {
            loop {
                deadline_check(deadline)?;
                backend.fence()?;
                deadline_check(deadline)?;
                if backend.witness()? {
                    break;
                }
                backend.pause();
            }
        }
        backend.fence()?;
        deadline_check(deadline)?;
        // Reobserve after the potentially slow fresh fence, before rank1 is exposed.
        if witness && !backend.witness()? {
            return Err("dependency witness changed before rank1 publication".into());
        }
        backend.publish(1)?;
        loop {
            deadline_check(deadline)?;
            backend.fence()?;
            deadline_check(deadline)?;
            if backend.complete()? {
                break;
            }
            backend.pause();
        }
        backend.fence()?;
        deadline_check(deadline)?;
        backend.retire()?;
        deadline_check(deadline)
    })();
    if result.is_err() {
        backend.poison();
    }
    result
}

/// The public buffer token is kept private; ordinary host access and kernel
/// binding never obtain an alias to initialized atomic signal storage.
struct Arena<const N: usize = PACKETS> {
    token: Gfx950EngineeringPeerBufferV1,
    local: u64,
    allocation: [u64; 4],  // handle, VA, requested, backing
    participant: [u64; 4], // unique ID, GPU ID, queue epoch, queue ID
    peer_gpu: u32,
}

impl<const N: usize> Arena<N> {
    fn check(&self, group: &Gfx950EngineeringPeerGroupV1) -> Result<()> {
        let record = group.validate_token(self.token)?;
        let context = &group.contexts[self.token.owner];
        let allocation = context
            .buffers
            .get(&self.local)
            .ok_or("lost dependency arena")?;
        if record.local_id != self.local
            || record.kind != BufferKind::PeerDependencyArena
            || record.mapping.peers != [self.peer_gpu]
            || record.mapping.mapped != 1
            || record.mapping.unmapped != 0
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
                    u64::from(context.queue_id.ok_or("lost dependency queue")?),
                ]
        {
            return Err("dependency arena mapping or queue identity changed".into());
        }
        Ok(())
    }

    fn signal(&self, slot: usize) -> Result<ObservedGpuAddressV1> {
        require_packet_count::<N>()?;
        if slot >= N {
            return Err("dependency signal slot".into());
        }
        ObservedGpuAddressV1::new(
            self.allocation[1]
                .checked_add((slot * AMD_SIGNAL_BYTES_V1) as u64)
                .ok_or("dependency signal address overflow")?,
        )
        .map_err(explain)
    }
}

fn allocate_arena<const N: usize>(
    group: &mut Gfx950EngineeringPeerGroupV1,
    rank: usize,
    prepared: &[PreparedDispatch],
) -> Result<Arena<N>> {
    require_packet_count::<N>()?;
    if prepared.len() != 4
        || prepared
            .iter()
            .any(|p| p.bytes.len() > MAX_KERNARG_BYTES_V1 as usize)
    {
        return Err("dependency kernarg shape".into());
    }
    check_contexts(&mut group.contexts, group.shared_full_currentness)?;
    let id = group.next_buffer;
    let local = group.contexts[rank].next_buffer;
    let peer_gpu = group.contexts[1 - rank].backend.gpu_id();
    if group.buffers.len() >= group_allocation_limit(2)?
        || group.contexts[rank].buffers.len() >= MAX_ALLOCATIONS
    {
        return Err("dependency arena allocation capacity".into());
    }
    let next_id = id
        .checked_add(1)
        .ok_or("dependency group identity exhausted")?;
    let next_local = local
        .checked_add(1)
        .ok_or("dependency local identity exhausted")?;
    group.next_buffer = next_id;
    group.contexts[rank].next_buffer = next_local;
    let allocation = group.contexts[rank].allocate_resource(
        ARENA_BYTES,
        KfdAllocMemoryFlags::KERNARG,
        |bytes| {
            // Raw kernargs are copied before any atomic signal lifetime begins.
            for (index, command) in prepared.iter().enumerate() {
                let offset = PAGE_BYTES + index * MAX_KERNARG_BYTES_V1 as usize;
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
    // Retain ownership before any subsequent fallible map/initialization/check.
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
            mapping: PeerMapping::new(vec![peer_gpu])?,
            kind: BufferKind::PeerDependencyArena,
        },
    );
    let end = identity[1]
        .checked_add(identity[3] - 1)
        .ok_or("dependency aperture end")?;
    let aperture = group.contexts[1 - rank].backend.gpuvm_aperture();
    if identity[1] < aperture.base() || end > aperture.limit() {
        return Err("dependency arena outside peer aperture".into());
    }
    Backend::initialize_engineering_signal_slots(
        &mut group.contexts[rank]
            .buffers
            .get_mut(&local)
            .ok_or("lost new arena")?
            .mapping,
        N,
    )
    .map_err(explain)?;
    group
        .buffers
        .get_mut(&id)
        .ok_or("lost new arena record")?
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
            u64::from(context.queue_id.ok_or("dependency queue absent")?),
        ],
        peer_gpu,
    };
    arena.check(group)?;
    Ok(arena)
}

fn make_batch<const N: usize>(
    rank: usize,
    prepared: Vec<PreparedDispatch>,
    arenas: &[Arena<N>],
) -> Result<Batch<N>> {
    require_packet_count::<N>()?;
    if prepared.len() != 4 || arenas.len() != 2 || rank >= 2 {
        return Err("dependency packet cardinality".into());
    }
    let mut packets = BTreeMap::new();
    for (index, command) in prepared.into_iter().enumerate() {
        let kernarg = arenas[rank].allocation[1]
            .checked_add((PAGE_BYTES + index * MAX_KERNARG_BYTES_V1 as usize) as u64)
            .ok_or("dependency kernarg address")?;
        packets.insert(
            KERNEL_SLOTS[index],
            AqlPreparedPeerPacketV1::Kernel(
                AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
                    command.geometry,
                    0,
                    command.group_bytes,
                    ObservedGpuAddressV1::new(command.descriptor).map_err(explain)?,
                    ObservedGpuAddressV1::new(kernarg).map_err(explain)?,
                    command.alignment,
                    arenas[rank].signal(KERNEL_SLOTS[index])?,
                    AqlDispatchOrderingV1::WaitForPrior,
                )
                .map_err(explain)?,
            ),
        );
    }
    let terminal = (N == 8).then_some((7, 6));
    for (slot, producer) in BARRIER_PRODUCERS.into_iter().chain(terminal) {
        packets.insert(
            slot,
            AqlPreparedPeerPacketV1::Barrier(
                AqlPeerBarrierAndPacketV1::new_unpublished(
                    [arenas[0].signal(producer)?, arenas[1].signal(producer)?],
                    arenas[rank].signal(slot)?,
                )
                .map_err(explain)?,
            ),
        );
    }
    let packets: [_; N] = packets
        .into_values()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "dependency fixed graph shape")?;
    Batch::try_from_packets(packets).map_err(explain)
}

struct Native<'a, const N: usize> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    arenas: Vec<Arena<N>>,
    reservations: Vec<AqlRingBatchReservationV1>,
    batches: Vec<Option<Batch<N>>>,
    published: [bool; 2],
    last_observation: Option<(Values<N>, [(u64, u64); 2])>,
    trace: DependencyTrace<N>,
    diagnostic_error: Option<String>,
    started: Instant,
    deadline: Instant,
}

impl<const N: usize> Native<'_, N> {
    fn observe(&mut self) -> Result<(Values<N>, [(u64, u64); 2])> {
        deadline_check(self.deadline)?;
        let mut values = [[1; N]; 2];
        let mut frontiers = [(0, 0); 2];
        for rank in 0..2 {
            self.arenas[rank].check(self.group)?;
            let context = &mut self.group.contexts[rank];
            let reservation = &self.reservations[rank];
            if context.ring.write() != reservation.next_write() {
                return Err("dependency reserved frontier changed".into());
            }
            let expected_write = if self.published[rank] {
                reservation.next_write()
            } else {
                reservation.first_packet_id()
            };
            let counters =
                Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(explain)?;
            validate_counters(expected_write, context.last_observed_read, counters)?;
            frontiers[rank] = counters;
            context.last_observed_read = counters.1;
            if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
                .map_err(explain)?
                != 0
            {
                return Err("dependency queue exception".into());
            }
            let allocation = context
                .buffers
                .get_mut(&self.arenas[rank].local)
                .ok_or("missing observed arena")?;
            for slot in 0..N {
                let (kind, value) = Backend::observe_completion_signal_state_acquire(
                    &mut allocation.mapping,
                    PAGE_BYTES,
                    slot as u32,
                )
                .map_err(explain)?;
                if kind != AMD_SIGNAL_KIND_USER_V1 || !matches!(value, 0 | 1) {
                    return Err(format!(
                        "dependency signal rank={rank} slot={slot} kind={kind} value={value}"
                    ));
                }
                values[rank][slot] = value;
            }
        }
        self.last_observation = Some((values, frontiers));
        if N == 8 {
            // Sample headers only for diagnostics. Failure must not replace a
            // deadline or protocol error, and headers cannot authorize reuse.
            let headers = self.diagnostic_headers();
            let headers = match headers {
                Ok(headers) => Some(headers),
                Err(error) => {
                    if self.diagnostic_error.is_none() {
                        self.diagnostic_error = Some(error);
                    }
                    None
                }
            };
            self.trace.record(
                self.started.elapsed().as_nanos().min(u64::MAX as u128) as u64,
                TraceState {
                    values,
                    frontiers,
                    headers,
                    published: self.published,
                },
            );
        }
        deadline_check(self.deadline)?;
        Ok((values, frontiers))
    }

    fn diagnostic_headers(&mut self) -> Result<[[(u32, u16, u16); N]; 2]> {
        deadline_check(self.deadline)?;
        let mut headers = [[(0, 0, 0); N]; 2];
        for (rank, row) in headers.iter_mut().enumerate() {
            let ring = &mut self.group.contexts[rank].internal[RING];
            for (index, header) in row.iter_mut().enumerate() {
                let packet_id = self.reservations[rank]
                    .entry(index as u32)
                    .ok_or("dependency diagnostic packet slot")?
                    .packet_id();
                *header = Backend::observe_aql_packet_header_acquire(
                    &mut ring.mapping,
                    ring.requested,
                    packet_id,
                )
                .map_err(explain)?;
            }
        }
        Ok(headers)
    }
}

struct PublicationTarget<'a> {
    context: &'a mut Context,
    reservation: &'a AqlRingBatchReservationV1,
    deadline: Instant,
}

impl PublicationTarget<'_> {
    fn write(&mut self, index: u32, bytes: [u8; 64]) -> Result<()> {
        deadline_check(self.deadline)?;
        let slot = self
            .reservation
            .entry(index)
            .ok_or("dependency packet slot")?
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
        deadline_check(self.deadline)?;
        let slot = self
            .reservation
            .entry(index)
            .ok_or("dependency header slot")?
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

impl<const N: usize> DependencyBackend for Native<'_, N> {
    fn fence(&mut self) -> Result<()> {
        deadline_check(self.deadline)?;
        for arena in &self.arenas {
            arena.check(self.group)?;
        }
        round::fresh_publication_fence(self.group)?;
        deadline_check(self.deadline)
    }
    fn publish(&mut self, rank: usize) -> Result<()> {
        deadline_check(self.deadline)?;
        if rank >= 2 || self.published[rank] || (rank == 1 && !self.published[0]) {
            return Err("dependency publication order".into());
        }
        let (values, _) = self.observe()?;
        if rank == 0 {
            if values != [[1; N]; 2] {
                return Err("dependency signals are not initially pending".into());
            }
        } else {
            // Even without the diagnostic wait, no consumer or rank1 producer
            // can have completed while rank1's packets are unpublished.
            witness_ready(values, self.published)?;
        }
        let batch = self.batches[rank]
            .take()
            .ok_or("dependency batch already consumed")?;
        let context = &mut self.group.contexts[rank];
        let reservation = &self.reservations[rank];
        let prior = Backend::fetch_add_aql_write(
            &mut context.internal[CONTROL].mapping,
            PAGE_BYTES,
            N as u64,
        )
        .map_err(explain)?;
        if prior != reservation.first_packet_id() {
            return Err("dependency hardware reservation mismatch".into());
        }
        batch.publish_with(&mut PublicationTarget {
            context,
            reservation,
            deadline: self.deadline,
        })?;
        // Headers may already be visible to the CP. Any failure from here is
        // terminal even when the final doorbell has not yet been written.
        self.published[rank] = true;
        self.fence()?;
        self.observe()?;
        deadline_check(self.deadline)?;
        self.group.contexts[rank]
            .doorbell
            .as_mut()
            .ok_or("dependency doorbell missing")?
            .store_packet_id_release(self.reservations[rank].last_packet_id())
            .map_err(explain)
    }
    fn witness(&mut self) -> Result<bool> {
        let (values, _) = self.observe()?;
        witness_ready(values, self.published)
    }
    fn complete(&mut self) -> Result<bool> {
        let (values, frontiers) = self.observe()?;
        complete_at_frontiers(
            values,
            frontiers,
            std::array::from_fn(|rank| self.reservations[rank].next_write()),
            self.published,
        )
    }
    fn retire(&mut self) -> Result<()> {
        if !self.complete()? {
            return Err("dependency retirement without all signals and frontiers".into());
        }
        deadline_check(self.deadline)?;
        for rank in 0..2 {
            self.group.contexts[rank].completed_write = self.reservations[rank].next_write();
        }
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)?;
        // Arenas stay retained until healthy group Close. No reset, allocator
        // reuse or release occurs while a remote CP might still read a signal.
        deadline_check(self.deadline)
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_micros(50));
    }
    fn poison(&mut self) {
        self.group.poisoned = true;
        for context in &mut self.group.contexts {
            context.ordered_batch_poisoned = true;
        }
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Executes P0, C0, P1, C1 on each rank, with system-scoped waits for both
    /// producers before consumers and both C0 consumers before P1 buffer reuse.
    /// Fresh private signal/kernarg arenas remain owned until healthy Close.
    /// `witness` deliberately leaves rank1 unpublished until rank0 P0 is complete
    /// and all other signals are pending. It is diagnostic, not a timing metric.
    ///
    /// # Safety
    /// The disposable-process/trusted-machine-code contract of `dispatch_unchecked`
    /// applies to every command. Declared access/bounds must be truthful. Commands
    /// within one phase must be independent across ranks; the API checks declared
    /// range conflicts. Every failure is terminal; do not retry or reset the group.
    pub unsafe fn dispatch_peer_dependency_sentinel_unchecked_v1(
        &mut self,
        commands: [[Gfx950EngineeringPeerDispatchV1<'_>; 4]; 2],
        timeout_ms: u32,
        witness: bool,
    ) -> Result<Gfx950EngineeringPeerDependencyObservationV1> {
        // SAFETY: the caller provides the same trusted commands and lifetime contract.
        unsafe { self.dispatch_peer_dependency_graph::<7>(commands, timeout_ms, witness) }
    }

    /// Adds a terminal wait for both C1 completions to the V1 graph. This is an
    /// opt-in diagnostic, not a guaranteed queue-frontier flush. All sixteen
    /// signals and both actual queue frontiers must retire before success.
    ///
    /// # Safety
    /// The V1 trusted-machine-code, truthful bounds, disposable process and
    /// terminal-error contract applies unchanged to every command.
    pub unsafe fn dispatch_peer_dependency_terminal_join_unchecked_v2(
        &mut self,
        commands: [[Gfx950EngineeringPeerDispatchV1<'_>; 4]; 2],
        timeout_ms: u32,
        witness: bool,
    ) -> Result<Gfx950EngineeringPeerDependencyObservationV1> {
        // SAFETY: only a distinct terminal barrier is added to the same checked commands.
        unsafe { self.dispatch_peer_dependency_graph::<8>(commands, timeout_ms, witness) }
    }

    unsafe fn dispatch_peer_dependency_graph<const N: usize>(
        &mut self,
        commands: [[Gfx950EngineeringPeerDispatchV1<'_>; 4]; 2],
        timeout_ms: u32,
        witness: bool,
    ) -> Result<Gfx950EngineeringPeerDependencyObservationV1> {
        require_packet_count::<N>()?;
        self.require_active()?;
        let started = Instant::now();
        let result = (|| {
            if self.contexts.len() != 2 || !(1..=600_000).contains(&timeout_ms) {
                return Err("dependency sentinel requires two ranks and bounded timeout".into());
            }
            let deadline = started
                .checked_add(Duration::from_millis(u64::from(timeout_ms)))
                .ok_or("dependency deadline overflow")?;
            for (rank, row) in commands.iter().enumerate() {
                let context = &self.contexts[rank];
                if context.raw_timestamps_enabled
                    || context
                        .performance
                        .is_some_and(|p| p.operational_currentness || p.profile)
                {
                    return Err(
                        "dependency sentinel requires full-currentness unprofiled queues".into(),
                    );
                }
                for command in row {
                    if command.kernel.rank != rank
                        || command.kernel.group != self.incarnation
                        || command.timeout_ms != timeout_ms
                    {
                        return Err("dependency command rank, group or timeout mismatch".into());
                    }
                }
            }
            for phase in 0..4 {
                round::require_round_independence(&[
                    &commands[0][phase].pointers,
                    &commands[1][phase].pointers,
                ])?;
            }
            check_contexts(&mut self.contexts, self.shared_full_currentness)?;
            let mut prepared = Vec::with_capacity(2);
            for row in commands {
                let mut rank = Vec::with_capacity(4);
                for command in row {
                    deadline_check(deadline)?;
                    rank.push(self.prepare_peer_dispatch(
                        command.kernel,
                        command.bytes,
                        command.workgroup,
                        command.grid,
                        &command.pointers,
                        command.timeout_ms,
                    )?);
                }
                prepared.push(rank);
            }
            let mut arenas = Vec::with_capacity(2);
            for rank in 0..2 {
                deadline_check(deadline)?;
                arenas.push(allocate_arena::<N>(self, rank, &prepared[rank])?);
            }
            let signals = arenas
                .iter()
                .map(|arena| {
                    (0..N)
                        .map(|slot| arena.signal(slot).map(|a| a.raw()))
                        .collect::<Result<Vec<_>>>()
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<BTreeSet<_>>();
            if signals.len() != 2 * N {
                return Err("dependency completion slots alias".into());
            }
            let mut batches = Vec::with_capacity(2);
            for (rank, row) in prepared.into_iter().enumerate() {
                batches.push(Some(make_batch(rank, row, &arenas)?));
            }
            check_contexts(&mut self.contexts, self.shared_full_currentness)?;
            deadline_check(deadline)?;
            for context in &self.contexts {
                require_sequence_capacity(context.ring.write(), context.last_observed_read, N)?;
            }
            let mut reservations = Vec::with_capacity(2);
            for context in &mut self.contexts {
                reservations.push(
                    context
                        .ring
                        .reserve_fixed_batch_v2(context.last_observed_read, N as u32)
                        .map_err(explain)?,
                );
            }
            let mut native = Native {
                group: self,
                arenas,
                reservations,
                batches,
                published: [false; 2],
                last_observation: None,
                trace: DependencyTrace::default(),
                diagnostic_error: None,
                started,
                deadline,
            };
            run_dependencies(&mut native, witness, deadline).map_err(|error| {
                let primary = format!(
                    "{error}; published={:?}; last_acquired_signals_and_frontiers={:?}",
                    native.published, native.last_observation,
                );
                if N == 8 {
                    format!(
                        "{primary}; sampled_trace={:?}; diagnostic_error={:?}",
                        native.trace, native.diagnostic_error
                    )
                } else {
                    primary
                }
            })?;
            Ok(Gfx950EngineeringPeerDependencyObservationV1 {
                elapsed_ns: u64::try_from(started.elapsed().as_nanos()).map_err(explain)?,
                witness_observed: witness,
                completion_count: (2 * N) as u32,
            })
        })();
        if result.is_err() {
            for context in &mut self.contexts {
                context.ordered_batch_poisoned = true;
            }
        }
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_dependency_tests.rs"]
mod tests;
