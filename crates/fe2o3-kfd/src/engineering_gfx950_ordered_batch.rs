//! Opt-in ordered packets on one retained engineering queue. No peer dispatch.

use super::*;
use fe2o3_aql::{
    AMD_SIGNAL_BYTES_V1, AqlDispatchOrderingV1, AqlPacketBatchPublicationTargetV1,
    AqlPreparedKernelDispatchBatchV2, AqlPreparedKernelDispatchV1, AqlRingBatchReservationV1,
};

const ORDERED_KERNARG: usize = CWSR + 1;
const ORDERED_KERNARG_BYTES: usize =
    MAX_ORDERED_BATCH_DISPATCHES_V1 * MAX_KERNARG_BYTES_V1 as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrderedMode {
    V1,
    Batch64,
}

impl OrderedMode {
    fn maximum(self) -> usize {
        match self {
            Self::V1 => MAX_ORDERED_BATCH_DISPATCHES_V1,
            Self::Batch64 => MAX_ORDERED_BATCH64_DISPATCHES_V1,
        }
    }

    fn arena_bytes(self) -> usize {
        match self {
            Self::V1 => ORDERED_KERNARG_BYTES,
            Self::Batch64 => self.maximum() * MAX_KERNARG_BYTES_V1 as usize,
        }
    }

    fn require_arena(self, bytes: usize) -> Result<()> {
        if bytes != self.arena_bytes() {
            return Err("ordered arena mode changed before queue rollover".into());
        }
        Ok(())
    }

    fn slot_offsets(self, index: usize, payload_bytes: usize) -> Result<(usize, usize)> {
        if index >= self.maximum() || payload_bytes > MAX_KERNARG_BYTES_V1 as usize {
            return Err("ordered slot count or payload extent".into());
        }
        let kernarg = index
            .checked_mul(MAX_KERNARG_BYTES_V1 as usize)
            .ok_or("ordered kernarg slot overflow")?;
        let signal = index
            .checked_mul(AMD_SIGNAL_BYTES_V1)
            .ok_or("ordered signal slot overflow")?;
        if kernarg
            .checked_add(MAX_KERNARG_BYTES_V1 as usize)
            .is_none_or(|end| end > self.arena_bytes())
            || signal
                .checked_add(AMD_SIGNAL_BYTES_V1)
                .is_none_or(|end| end > PAGE_BYTES)
        {
            return Err("ordered slot exceeds retained storage".into());
        }
        Ok((kernarg, signal))
    }

    fn require_capacity(self, write: u64, read: u64, count: usize) -> Result<()> {
        match self {
            Self::V1 => require_sequence_capacity(write, read, count),
            Self::Batch64 => require_ordered64_capacity(write, read, count),
        }
    }

    fn payload_bytes(
        self,
        dispatches: &[OrderedBatchDispatchV1],
        timeout_ms: u32,
    ) -> Result<usize> {
        match self {
            Self::V1 => ordered_batch_payload_bytes(dispatches, timeout_ms),
            Self::Batch64 => ordered_batch64_payload_bytes(dispatches, timeout_ms),
        }
        .map_err(explain)
    }
}

fn require_ordered64_capacity(write: u64, read: u64, dispatches: usize) -> Result<()> {
    if !(1..=MAX_ORDERED_BATCH64_DISPATCHES_V1).contains(&dispatches)
        || read > write
        || write
            .checked_add(dispatches as u64)
            .and_then(|next| next.checked_sub(read))
            .is_none_or(|outstanding| outstanding > MAX_UNRETIRED_RING_PACKETS_V1)
    {
        return Err("ordered64 exceeds retained ring capacity; rollover required".into());
    }
    Ok(())
}

trait OrderedBackend {
    type Prepared;
    type Staged;
    type Pending;
    fn dispatch_fence(&mut self) -> Result<()>;
    fn prepare_all(&mut self, count: usize) -> Result<Vec<Self::Prepared>>;
    fn preparation_fence(&mut self) -> Result<()>;
    fn stage(&mut self, prepared: Vec<Self::Prepared>) -> Result<Self::Staged>;
    fn publish(&mut self, staged: Self::Staged, deadline: Instant) -> Result<Self::Pending>;
    fn poll_final(&mut self, pending: &mut Self::Pending) -> Result<bool>;
    fn validate_all(&mut self, pending: &Self::Pending) -> Result<()>;
    fn complete(&mut self, pending: Self::Pending) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn poison(&mut self);
}

pub(super) fn require_deadline(now: Instant, deadline: Instant) -> Result<()> {
    if now >= deadline {
        return Err("ordered batch aggregate deadline expired; process teardown required".into());
    }
    Ok(())
}

fn run_ordered_batch(
    backend: &mut impl OrderedBackend,
    count: usize,
    timeout_ms: u32,
) -> Result<u64> {
    run_ordered_batch_mode(backend, count, timeout_ms, OrderedMode::V1)
}

fn run_ordered_batch_mode(
    backend: &mut impl OrderedBackend,
    count: usize,
    timeout_ms: u32,
    mode: OrderedMode,
) -> Result<u64> {
    run_ordered_batch_deadline(backend, count, timeout_ms, mode, None)
}

fn run_ordered_batch_deadline(
    backend: &mut impl OrderedBackend,
    count: usize,
    timeout_ms: u32,
    mode: OrderedMode,
    aggregate_deadline: Option<Instant>,
) -> Result<u64> {
    let result = (|| {
        if !(1..=mode.maximum()).contains(&count) || !(1..=600_000).contains(&timeout_ms) {
            return Err("ordered batch count or aggregate timeout".into());
        }
        backend.dispatch_fence()?;
        if let Some(deadline) = aggregate_deadline {
            require_deadline(Instant::now(), deadline)?;
        }
        let prepared = backend.prepare_all(count)?;
        backend.preparation_fence()?;
        if let Some(deadline) = aggregate_deadline {
            require_deadline(Instant::now(), deadline)?;
        }
        let staged = backend.stage(prepared)?;
        let started = Instant::now();
        let deadline = match aggregate_deadline {
            Some(deadline) => {
                require_deadline(started, deadline)?;
                deadline
            }
            None => started
                .checked_add(Duration::from_millis(u64::from(timeout_ms)))
                .ok_or("ordered batch deadline overflow")?,
        };
        let mut pending = backend.publish(staged, deadline)?;
        loop {
            require_deadline(Instant::now(), deadline)?;
            if backend.poll_final(&mut pending)? {
                break;
            }
            backend.pause()?;
        }
        // The final ordered completion is necessary but never substitutes for
        // observing every retained signal before storage/frontier reuse.
        backend.validate_all(&pending)?;
        backend.complete(pending)?;
        backend.dispatch_fence()?;
        require_deadline(Instant::now(), deadline)?;
        u64::try_from(started.elapsed().as_nanos()).map_err(explain)
    })();
    if result.is_err() {
        backend.poison();
    }
    result
}

struct NativeOrdered<'a> {
    context: &'a mut Context,
    mode: OrderedMode,
    commands: Vec<Option<OrderedBatchDispatchV1>>,
    payload: Vec<u8>,
    offset: usize,
    count: u32,
    active_wait: Option<wait_policy::ActivePollWait>,
    prepared_for_execution: Option<Vec<PreparedDispatch>>,
}

struct OrderedPending {
    unique_id: u64,
    queue_epoch: u64,
    next: u64,
    count: u32,
    deadline: Instant,
    next_currentness: Instant,
    wait_started: Option<Instant>,
}

fn require_signals_complete(
    signals: impl IntoIterator<Item = AqlCompletionObservationV1>,
) -> Result<()> {
    for signal in signals {
        if signal != AqlCompletionObservationV1::Completed {
            return Err("ordered batch retained signal did not complete".into());
        }
    }
    Ok(())
}

impl NativeOrdered<'_> {
    fn retain_storage(&mut self) -> Result<()> {
        self.context.check_idle()?;
        if self.mode.maximum() * AMD_SIGNAL_BYTES_V1 > PAGE_BYTES {
            return Err("ordered signal page capacity".into());
        }
        match self.context.internal.len() {
            ORDERED_KERNARG => {
                let allocation = self.context.allocate_resource(
                    self.mode.arena_bytes(),
                    KfdAllocMemoryFlags::KERNARG,
                    |_| Ok(()),
                )?;
                // Retain the allocation before the next fallible operation.
                self.context.internal.push(allocation);
                match self.mode {
                    OrderedMode::V1 => Backend::initialize_engineering_signal_slots(
                        &mut self.context.internal[SIGNAL].mapping,
                        MAX_ORDERED_BATCH_DISPATCHES_V1,
                    ),
                    OrderedMode::Batch64 => Backend::initialize_engineering_signal_slots64(
                        &mut self.context.internal[SIGNAL].mapping,
                        MAX_ORDERED_BATCH64_DISPATCHES_V1,
                    ),
                }
                .map_err(explain)?;
            }
            count if count == ORDERED_KERNARG + 1 => {}
            _ => return Err("ordered batch private storage identity".into()),
        }
        self.mode
            .require_arena(self.context.internal[ORDERED_KERNARG].requested)
    }

    fn publish_fixed<const N: usize>(
        &mut self,
        packets: Vec<AqlPreparedKernelDispatchV1>,
        deadline: Instant,
    ) -> Result<OrderedPending> {
        let count = match self.mode {
            OrderedMode::V1 => batch_count::<N>()?,
            OrderedMode::Batch64 => batch64_count::<N>()?,
        };
        let packets: [AqlPreparedKernelDispatchV1; N] = packets
            .try_into()
            .map_err(|_| "ordered batch packet cardinality")?;
        let batch = AqlPreparedKernelDispatchBatchV2::try_from_packets(packets).map_err(explain)?;
        self.context.check_idle()?;
        self.mode.require_capacity(
            self.context.ring.write(),
            self.context.last_observed_read,
            N,
        )?;
        self.context.check_currentness(false)?;
        require_deadline(Instant::now(), deadline)?;
        let reservation = self
            .context
            .ring
            .reserve_fixed_batch_v2(self.context.last_observed_read, batch.packet_count())
            .map_err(explain)?;
        let next = reservation.next_write();
        let publish_started = self.context.profile_started();
        // No rollback or signal/arena reuse is allowed after this reservation.
        expose_batch(
            batch,
            &mut OrderedPublication {
                context: self.context,
                reservation: &reservation,
                deadline,
            },
        )?;
        record_elapsed(
            &mut self.context.counters.dispatch_publish_ns,
            publish_started,
        )?;
        self.active_wait = self
            .context
            .ordered64_wait_policy
            .start(self.mode == OrderedMode::Batch64, deadline)?;
        Ok(OrderedPending {
            unique_id: self.context.unique_id,
            queue_epoch: self.context.queue_epoch,
            next,
            count,
            deadline,
            next_currentness: Instant::now(),
            wait_started: self.context.profile_started(),
        })
    }
}

fn batch_count<const N: usize>() -> Result<u32> {
    if !(1..=MAX_ORDERED_BATCH_DISPATCHES_V1).contains(&N) {
        return Err("ordered batch fixed cardinality".into());
    }
    u32::try_from(N).map_err(explain)
}

fn batch64_count<const N: usize>() -> Result<u32> {
    if !(1..=MAX_ORDERED_BATCH64_DISPATCHES_V1).contains(&N) {
        return Err("ordered64 fixed cardinality".into());
    }
    u32::try_from(N).map_err(explain)
}

impl OrderedBackend for NativeOrdered<'_> {
    type Prepared = PreparedDispatch;
    type Staged = Vec<AqlPreparedKernelDispatchV1>;
    type Pending = OrderedPending;

    fn dispatch_fence(&mut self) -> Result<()> {
        // Reuse the explicit dispatch policy; allocation and lifecycle fences
        // remain full checks, including first-use ordered arena allocation.
        self.context.check_currentness(false)?;
        self.context.check_idle()
    }

    fn prepare_all(&mut self, count: usize) -> Result<Vec<PreparedDispatch>> {
        if let Some(prepared) = self.prepared_for_execution.take() {
            if prepared.len() != count || !self.commands.is_empty() || !self.payload.is_empty() {
                return Err("token program prepared group identity".into());
            }
            return Ok(prepared);
        }
        // The scope borrows ownership maps immutably for the whole CPU phase.
        // It has no queue/backend access and cannot stage or publish anything.
        let mut scope = self.context.preparation_scope();
        let mut prepared = Vec::with_capacity(count);
        for index in 0..count {
            let command = self
                .commands
                .get_mut(index)
                .and_then(Option::take)
                .ok_or("ordered batch command unavailable")?;
            let end = self
                .offset
                .checked_add(command.payload_bytes as usize)
                .ok_or("ordered batch payload overflow")?;
            let bytes = self
                .payload
                .get(self.offset..end)
                .ok_or("ordered batch payload bounds")?
                .to_vec();
            let prepare_started = scope.profile_started();
            prepared.push(scope.prepare(
                command.kernel,
                bytes,
                command.workgroup,
                command.grid,
                &command.pointers,
                None,
            )?);
            record_elapsed(&mut scope.counters.dispatch_prepare_ns, prepare_started)?;
            self.offset = end;
        }
        Ok(prepared)
    }

    fn preparation_fence(&mut self) -> Result<()> {
        // CPU preparation cannot authorize later GPU-visible writes. Reobserve
        // reset/process identity, poison, counters and exceptions before staging.
        self.context.check_idle()
    }

    fn stage(&mut self, prepared: Vec<PreparedDispatch>) -> Result<Self::Staged> {
        if self.offset != self.payload.len() || prepared.len() != self.count as usize {
            return Err("ordered batch staged payload or count".into());
        }
        self.retain_storage()?;
        let mut packets = Vec::with_capacity(prepared.len());
        for (index, prepared) in prepared.into_iter().enumerate() {
            let (offset, signal_offset) = self.mode.slot_offsets(index, prepared.bytes.len())?;
            let kernarg_address = self.context.internal[ORDERED_KERNARG]
                .va
                .checked_add(offset as u64)
                .ok_or("ordered kernarg address")?;
            let signal_address = self.context.internal[SIGNAL]
                .va
                .checked_add(signal_offset as u64)
                .ok_or("ordered signal address")?;
            packets.push(
                AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
                    prepared.geometry,
                    0,
                    prepared.group_bytes,
                    ObservedGpuAddressV1::new(prepared.descriptor).map_err(explain)?,
                    ObservedGpuAddressV1::new(kernarg_address).map_err(explain)?,
                    prepared.alignment,
                    ObservedGpuAddressV1::new(signal_address).map_err(explain)?,
                    AqlDispatchOrderingV1::WaitForPrior,
                )
                .map_err(explain)?,
            );
            Backend::with_bytes_mut(
                &mut self.context.internal[ORDERED_KERNARG].mapping,
                self.mode.arena_bytes(),
                |mapped| {
                    let slot = &mut mapped[offset..offset + MAX_KERNARG_BYTES_V1 as usize];
                    slot.fill(0);
                    slot[..prepared.bytes.len()].copy_from_slice(&prepared.bytes);
                },
            );
            Backend::reset_completion_signal_release(
                &mut self.context.internal[SIGNAL].mapping,
                PAGE_BYTES,
                u32::try_from(index).map_err(explain)?,
            )
            .map_err(explain)?;
        }
        Ok(packets)
    }

    fn publish(&mut self, packets: Self::Staged, deadline: Instant) -> Result<OrderedPending> {
        macro_rules! fixed {
            ($($count:literal),+ $(,)?) => {
                match packets.len() {
                    $($count => self.publish_fixed::<$count>(packets, deadline),)+
                    _ => Err("ordered batch publication count".into()),
                }
            };
        }
        match self.mode {
            OrderedMode::V1 => fixed!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16),
            OrderedMode::Batch64 => fixed!(
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
                45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64
            ),
        }
    }

    fn poll_final(&mut self, pending: &mut OrderedPending) -> Result<bool> {
        require_pending_dispatch_identity(
            [
                self.context.unique_id,
                self.context.queue_epoch,
                self.context.ring.write(),
            ],
            [pending.unique_id, pending.queue_epoch, pending.next],
            false,
        )?;
        require_deadline(Instant::now(), pending.deadline)?;
        if pending.wait_started.is_some() {
            add_counter(&mut self.context.counters.completion_polls, 1)?;
        }
        let completion = Backend::observe_completion_signal_acquire(
            &mut self.context.internal[SIGNAL].mapping,
            PAGE_BYTES,
            pending.count - 1,
        )
        .map_err(explain)?;
        let counters =
            Backend::observe_aql_counters(&mut self.context.internal[CONTROL].mapping, PAGE_BYTES)
                .map_err(explain)?;
        let exception = Backend::observe_i64_acquire(
            &mut self.context.internal[CONTROL].mapping,
            PAGE_BYTES,
            256,
        )
        .map_err(explain)?;
        let completed = dispatch_completed(
            pending.next,
            self.context.last_observed_read,
            counters,
            completion,
            exception,
        )?;
        self.context.last_observed_read = counters.1;
        let now = Instant::now();
        if now >= pending.next_currentness {
            self.context.check_currentness(false)?;
            pending.next_currentness = now
                .checked_add(Duration::from_millis(100))
                .ok_or("ordered batch currentness deadline")?;
        }
        Ok(completed)
    }

    fn validate_all(&mut self, pending: &OrderedPending) -> Result<()> {
        let signals = (0..pending.count)
            .map(|slot| {
                Backend::observe_completion_signal_acquire(
                    &mut self.context.internal[SIGNAL].mapping,
                    PAGE_BYTES,
                    slot,
                )
                .map_err(explain)
            })
            .collect::<Result<Vec<_>>>()?;
        require_signals_complete(signals)
    }

    fn complete(&mut self, pending: OrderedPending) -> Result<()> {
        require_pending_dispatch_identity(
            [
                self.context.unique_id,
                self.context.queue_epoch,
                self.context.ring.write(),
            ],
            [pending.unique_id, pending.queue_epoch, pending.next],
            false,
        )?;
        require_deadline(Instant::now(), pending.deadline)?;
        self.context.completed_write = pending.next;
        record_elapsed(
            &mut self.context.counters.dispatch_wait_ns,
            pending.wait_started,
        )?;
        if pending.wait_started.is_some() {
            add_counter(
                &mut self.context.counters.dispatches,
                u64::from(pending.count),
            )?;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        if let Some(wait) = &mut self.active_wait {
            wait.pause()
        } else {
            std::thread::sleep(Duration::from_micros(50));
            Ok(())
        }
    }

    fn poison(&mut self) {
        self.context.ordered_batch_poisoned = true;
    }
}

struct OrderedPublication<'a> {
    context: &'a mut Context,
    reservation: &'a AqlRingBatchReservationV1,
    deadline: Instant,
}

trait OrderedExposure: AqlPacketBatchPublicationTargetV1<Error = String> {
    fn advance_write(&mut self, count: u32) -> Result<()>;
    fn publication_checkpoint(&mut self) -> Result<()>;
    fn ring_final_doorbell(&mut self) -> Result<()>;
}

fn expose_batch<const N: usize>(
    batch: AqlPreparedKernelDispatchBatchV2<N>,
    target: &mut impl OrderedExposure,
) -> Result<()> {
    target.advance_write(batch.packet_count())?;
    batch.publish_with(target)?;
    target.publication_checkpoint()?;
    target.ring_final_doorbell()
}

impl OrderedExposure for OrderedPublication<'_> {
    fn advance_write(&mut self, count: u32) -> Result<()> {
        if count != self.reservation.packet_count() {
            return Err("ordered batch reservation count".into());
        }
        let prior = Backend::fetch_add_aql_write(
            &mut self.context.internal[CONTROL].mapping,
            PAGE_BYTES,
            u64::from(count),
        )
        .map_err(explain)?;
        if prior != self.reservation.first_packet_id() {
            return Err("ordered batch queue write reservation substitution".into());
        }
        Ok(())
    }

    fn publication_checkpoint(&mut self) -> Result<()> {
        self.context.check_currentness(false)?;
        let counters =
            Backend::observe_aql_counters(&mut self.context.internal[CONTROL].mapping, PAGE_BYTES)
                .map_err(explain)?;
        validate_counters(
            self.reservation.next_write(),
            self.context.last_observed_read,
            counters,
        )?;
        self.context.last_observed_read = counters.1;
        if Backend::observe_i64_acquire(
            &mut self.context.internal[CONTROL].mapping,
            PAGE_BYTES,
            256,
        )
        .map_err(explain)?
            != 0
        {
            return Err("ordered batch publication exception".into());
        }
        require_deadline(Instant::now(), self.deadline)
    }

    fn ring_final_doorbell(&mut self) -> Result<()> {
        self.context
            .doorbell
            .as_mut()
            .ok_or("missing doorbell")?
            .store_packet_id_release(self.reservation.last_packet_id())
            .map_err(explain)
    }
}

impl AqlPacketBatchPublicationTargetV1 for OrderedPublication<'_> {
    type Error = String;

    fn write_unpublished(&mut self, index: u32, packet: &AqlKernelDispatchPacketV1) -> Result<()> {
        let entry = self
            .reservation
            .entry(index)
            .ok_or("ordered publication slot")?;
        Publication {
            mapping: &mut self.context.internal[RING].mapping,
            slot: entry.slot_index(),
        }
        .write_unpublished(packet)
    }

    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<()> {
        if header != AqlDispatchOrderingV1::WaitForPrior.header() {
            return Err("ordered publication requires wait-for-prior".into());
        }
        let entry = self
            .reservation
            .entry(index)
            .ok_or("ordered publication header slot")?;
        Publication {
            mapping: &mut self.context.internal[RING].mapping,
            slot: entry.slot_index(),
        }
        .publish_release_header(header)
    }
}

impl Context {
    /// Dedicated-process trusted-code obligations are identical to `dispatch`.
    /// Batch failures are terminal; callers must retain this owner until exit.
    pub(super) unsafe fn dispatch_ordered_batch(
        &mut self,
        dispatches: Vec<OrderedBatchDispatchV1>,
        payload: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<ResponseV1> {
        // SAFETY: preserve the dedicated-process trusted-code caller contract.
        unsafe {
            self.dispatch_ordered_batch_mode(dispatches, payload, timeout_ms, OrderedMode::V1)
        }
    }

    /// Separate engineering route; no default, sequence or peer limit changes.
    pub(super) unsafe fn dispatch_ordered_batch64(
        &mut self,
        dispatches: Vec<OrderedBatchDispatchV1>,
        payload: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<ResponseV1> {
        // SAFETY: identical trusted-code/lifetime obligations to the V1 route.
        unsafe {
            self.dispatch_ordered_batch_mode(dispatches, payload, timeout_ms, OrderedMode::Batch64)
        }
    }

    unsafe fn dispatch_ordered_batch_mode(
        &mut self,
        dispatches: Vec<OrderedBatchDispatchV1>,
        payload: Vec<u8>,
        timeout_ms: u32,
        mode: OrderedMode,
    ) -> Result<ResponseV1> {
        let result = (|| {
            self.ordered64_wait_policy.require_profile(
                mode == OrderedMode::Batch64,
                self.performance.is_some_and(|options| options.profile),
            )?;
            let expected = mode.payload_bytes(&dispatches, timeout_ms)?;
            if expected != payload.len() {
                return Err("ordered batch payload length".into());
            }
            let count = dispatches.len();
            mode.require_capacity(self.ring.write(), self.last_observed_read, count)?;
            let mut native = NativeOrdered {
                context: self,
                mode,
                commands: dispatches.into_iter().map(Some).collect(),
                payload,
                offset: 0,
                count: u32::try_from(count).map_err(explain)?,
                active_wait: None,
                prepared_for_execution: None,
            };
            let elapsed_ns = match mode {
                OrderedMode::V1 => run_ordered_batch(&mut native, count, timeout_ms)?,
                OrderedMode::Batch64 => {
                    run_ordered_batch_mode(&mut native, count, timeout_ms, mode)?
                }
            };
            if let Some(wait) = &native.active_wait {
                native.context.active_poll_counters.record_completed(wait)?;
            }
            Ok(match mode {
                OrderedMode::V1 => ResponseV1::DispatchOrderedBatchCompleted {
                    completed_dispatches: native.count,
                    elapsed_ns,
                },
                OrderedMode::Batch64 => ResponseV1::DispatchOrderedBatch64Completed {
                    completed_dispatches: native.count,
                    elapsed_ns,
                },
            })
        })();
        if result.is_err() {
            self.ordered_batch_poisoned = true;
        }
        result
    }
}

impl Context {
    /// Receives only freshly prepared CPU data from this execution, never a
    /// retained packet or completion object. Every group uses the same deadline.
    pub(super) fn run_prepared_token_group(
        &mut self,
        prepared: Vec<PreparedDispatch>,
        deadline: Instant,
    ) -> Result<()> {
        let count = prepared.len();
        let mut native = NativeOrdered {
            context: self,
            mode: OrderedMode::Batch64,
            commands: Vec::new(),
            payload: Vec::new(),
            offset: 0,
            count: u32::try_from(count).map_err(explain)?,
            active_wait: None,
            prepared_for_execution: Some(prepared),
        };
        run_ordered_batch_deadline(
            &mut native,
            count,
            600_000,
            OrderedMode::Batch64,
            Some(deadline),
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_ordered_batch_tests.rs"]
mod tests;
