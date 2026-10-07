//! Default-off whole-program submission with independently retained arenas.

use super::*;
use fe2o3_aql::{
    AqlClosedProgramHeaderV1, AqlClosedProgramPublicationTargetV1,
    AqlPreparedClosedKernelDispatchProgramV1, AqlPreparedKernelDispatchProgramV1,
};

#[cfg(feature = "engineering-native-wait-diagnostics")]
#[path = "engineering_gfx950_native_wait_diagnostic.rs"]
mod wait_diagnostic;
#[cfg(feature = "engineering-native-wait-diagnostics")]
#[path = "engineering_native_wait_observation.rs"]
mod wait_observation;

const PROGRAM_KERNARG: usize = 0;
const PROGRAM_SIGNAL: usize = 1;
const MAX_PROGRAM_KERNARG_BYTES: usize =
    MAX_TOKEN_PROGRAM_DISPATCHES_V1 * MAX_KERNARG_BYTES_V1 as usize;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProgramLayout {
    slots: Vec<(usize, usize, u64)>,
    kernarg_bytes: usize,
    signal_bytes: usize,
    initialized_bytes: u64,
}

impl ProgramLayout {
    fn new(extents: &[(usize, u64)]) -> Result<Self> {
        if !(1..=MAX_TOKEN_PROGRAM_DISPATCHES_V1).contains(&extents.len()) {
            return Err("native token program count".into());
        }
        let mut slots = Vec::with_capacity(extents.len());
        let mut end = 0usize;
        let mut initialized_bytes = 0u64;
        for &(bytes, alignment) in extents {
            if bytes > MAX_KERNARG_BYTES_V1 as usize
                || alignment == 0
                || alignment > PAGE_BYTES as u64
                || !alignment.is_power_of_two()
            {
                return Err("native token program kernarg extent or alignment".into());
            }
            let alignment = alignment as usize;
            let offset = end
                .checked_add(alignment - 1)
                .map(|value| value & !(alignment - 1))
                .ok_or("native token program kernarg alignment overflow")?;
            // Even an empty segment receives a distinct, non-null slot.
            end = offset
                .checked_add(bytes.max(1))
                .filter(|end| *end <= MAX_PROGRAM_KERNARG_BYTES)
                .ok_or("native token program kernarg arena bound")?;
            initialized_bytes = initialized_bytes
                .checked_add(bytes as u64)
                .ok_or("native token program staging counter overflow")?;
            slots.push((offset, bytes, alignment as u64));
        }
        let kernarg_bytes = end
            .checked_add(PAGE_BYTES - 1)
            .map(|bytes| bytes & !(PAGE_BYTES - 1))
            .filter(|bytes| *bytes <= MAX_PROGRAM_KERNARG_BYTES)
            .ok_or("native token program kernarg rounding")?;
        let signal_bytes = extents
            .len()
            .checked_mul(AMD_SIGNAL_BYTES_V1)
            .and_then(|bytes| bytes.checked_add(PAGE_BYTES - 1))
            .map(|bytes| bytes & !(PAGE_BYTES - 1))
            .ok_or("native token program signal rounding")?;
        Ok(Self {
            slots,
            kernarg_bytes,
            signal_bytes,
            initialized_bytes,
        })
    }
}

#[derive(Default)]
pub(crate) struct ProgramStorage {
    allocations: Vec<Allocation>,
    layout: Option<ProgramLayout>,
}

fn require_storage_layout(
    retained: Option<&ProgramLayout>,
    allocations: usize,
    requested: &ProgramLayout,
) -> Result<bool> {
    match (retained, allocations) {
        (None, 0) => Ok(false),
        (Some(retained), 2) if retained == requested => Ok(true),
        _ => Err("native token program retained storage or layout changed".into()),
    }
}

struct NativeProgram<'a> {
    context: &'a mut Context,
    prepared: Option<Vec<PreparedDispatch>>,
    #[cfg(feature = "engineering-native-wait-diagnostics")]
    wait_diagnostic: wait_diagnostic::Diagnostic,
}

enum StagedProgram {
    System(AqlPreparedKernelDispatchProgramV1),
    Boundary(AqlPreparedClosedKernelDispatchProgramV1),
}

impl StagedProgram {
    fn packet_count(&self) -> u32 {
        match self {
            Self::System(program) => program.packet_count(),
            Self::Boundary(program) => program.packet_count(),
        }
    }
}

impl NativeProgram<'_> {
    fn retain_storage(&mut self, layout: &ProgramLayout) -> Result<()> {
        self.context.check_idle()?;
        if require_storage_layout(
            self.context.token_program_storage.layout.as_ref(),
            self.context.token_program_storage.allocations.len(),
            layout,
        )? {
            return Ok(());
        }
        let kernarg = self.context.allocate_resource(
            layout.kernarg_bytes,
            KfdAllocMemoryFlags::KERNARG,
            |_| Ok(()),
        )?;
        self.context.token_program_storage.allocations.push(kernarg);
        let signal = self.context.allocate_resource(
            layout.signal_bytes,
            KfdAllocMemoryFlags::KERNARG,
            |_| Ok(()),
        )?;
        // Retain both owners before signal initialization or any packet staging.
        self.context.token_program_storage.allocations.push(signal);
        Backend::initialize_engineering_program_signals(
            &mut self.context.token_program_storage.allocations[PROGRAM_SIGNAL].mapping,
            layout.signal_bytes,
            layout.slots.len(),
        )
        .map_err(explain)?;
        self.context.token_program_storage.layout = Some(layout.clone());
        Ok(())
    }
}

impl OrderedBackend for NativeProgram<'_> {
    type Prepared = PreparedDispatch;
    type Staged = StagedProgram;
    type Pending = OrderedPending;

    fn dispatch_fence(&mut self) -> Result<()> {
        self.context.check_currentness(false)?;
        self.context.check_idle()
    }

    fn prepare_all(&mut self, count: usize) -> Result<Vec<PreparedDispatch>> {
        let prepared = self
            .prepared
            .take()
            .ok_or("native token program reused preparation")?;
        if prepared.len() != count {
            return Err("native token program preparation count".into());
        }
        Ok(prepared)
    }

    fn preparation_fence(&mut self) -> Result<()> {
        self.context.check_idle()
    }

    fn stage(&mut self, prepared: Vec<PreparedDispatch>) -> Result<Self::Staged> {
        let layout = ProgramLayout::new(
            &prepared
                .iter()
                .map(|item| (item.bytes.len(), item.alignment))
                .collect::<Vec<_>>(),
        )?;
        self.retain_storage(&layout)?;
        let started = self.context.profile_started();
        let storage = &mut self.context.token_program_storage;
        let mut packets = Vec::with_capacity(prepared.len());
        for (index, prepared) in prepared.into_iter().enumerate() {
            let (offset, bytes, _) = layout.slots[index];
            let kernarg_address = storage.allocations[PROGRAM_KERNARG]
                .va
                .checked_add(offset as u64)
                .ok_or("native token program kernarg address")?;
            let signal_address = storage.allocations[PROGRAM_SIGNAL]
                .va
                .checked_add((index * AMD_SIGNAL_BYTES_V1) as u64)
                .ok_or("native token program signal address")?;
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
                &mut storage.allocations[PROGRAM_KERNARG].mapping,
                layout.kernarg_bytes,
                |mapped| mapped[offset..offset + bytes].copy_from_slice(&prepared.bytes),
            );
            Backend::reset_completion_signal_release(
                &mut storage.allocations[PROGRAM_SIGNAL].mapping,
                layout.signal_bytes,
                index as u32,
            )
            .map_err(explain)?;
        }
        if started.is_some() {
            record_elapsed(&mut self.context.token_program_counters.staging_ns, started)?;
            add_counter(
                &mut self
                    .context
                    .token_program_counters
                    .kernarg_initialized_bytes,
                layout.initialized_bytes,
            )?;
        }
        if self.context.token_program_boundary_fences {
            AqlPreparedClosedKernelDispatchProgramV1::try_from_packets(packets.into_boxed_slice())
                .map(StagedProgram::Boundary)
                .map_err(explain)
        } else {
            AqlPreparedKernelDispatchProgramV1::try_from_packets(packets.into_boxed_slice())
                .map(StagedProgram::System)
                .map_err(explain)
        }
    }

    fn publish(&mut self, program: Self::Staged, deadline: Instant) -> Result<Self::Pending> {
        let count = program.packet_count();
        self.context.check_idle()?;
        super::super::token_program::require_frontier(
            self.context.ring.write(),
            self.context.completed_write,
            self.context.completed_write,
            self.context.last_observed_read,
            count as usize,
        )?;
        self.context.check_currentness(false)?;
        require_deadline(Instant::now(), deadline)?;
        let reservation = self
            .context
            .ring
            .reserve_fixed_batch_v2(self.context.last_observed_read, count)
            .map_err(explain)?;
        let next = reservation.next_write();
        let started = self.context.profile_started();
        // Reservation is irreversible. The enclosing lifecycle poisons on any
        // subsequent error and the disposable worker retains all arena owners.
        let mut target = OrderedPublication {
            context: self.context,
            reservation: &reservation,
            deadline,
        };
        match program {
            StagedProgram::System(program) => expose_program(program, &mut target)?,
            StagedProgram::Boundary(program) => expose_closed_program(program, &mut target)?,
        }
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        self.wait_diagnostic.published();
        record_elapsed(&mut self.context.counters.dispatch_publish_ns, started)?;
        if started.is_some() {
            add_counter(&mut self.context.token_program_counters.publications, 1)?;
            add_counter(&mut self.context.token_program_counters.final_waits, 1)?;
        }
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

    fn poll_final(&mut self, pending: &mut Self::Pending) -> Result<bool> {
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
        let signal = &mut self.context.token_program_storage.allocations[PROGRAM_SIGNAL];
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        let read_before = self.wait_diagnostic.stamp();
        let completion = Backend::observe_completion_signal_acquire(
            &mut signal.mapping,
            signal.requested,
            pending.count - 1,
        )
        .map_err(explain)?;
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        let read_after = {
            let after = self.wait_diagnostic.stamp();
            let state = match &completion {
                AqlCompletionObservationV1::Pending => wait_diagnostic::SignalState::Pending,
                AqlCompletionObservationV1::Completed => wait_diagnostic::SignalState::Completed,
                AqlCompletionObservationV1::Unexpected(_) => {
                    wait_diagnostic::SignalState::Unexpected
                }
            };
            self.wait_diagnostic
                .observation
                .read(read_before, after, state);
            after
        };
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
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        let mut currentness_ns = 0;
        if now >= pending.next_currentness {
            #[cfg(feature = "engineering-native-wait-diagnostics")]
            let before = self.wait_diagnostic.stamp();
            self.context.check_currentness(false)?;
            #[cfg(feature = "engineering-native-wait-diagnostics")]
            {
                currentness_ns = self.wait_diagnostic.stamp().saturating_sub(before);
            }
            pending.next_currentness = now
                .checked_add(Duration::from_millis(100))
                .ok_or("native token program currentness deadline")?;
        }
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        {
            let after = self.wait_diagnostic.stamp();
            self.wait_diagnostic.observation.post_read(
                read_after,
                after,
                currentness_ns,
                completed,
            );
        }
        Ok(completed)
    }

    fn validate_all(&mut self, pending: &Self::Pending) -> Result<()> {
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        let before = self.wait_diagnostic.stamp();
        let signal = &mut self.context.token_program_storage.allocations[PROGRAM_SIGNAL];
        for slot in 0..pending.count {
            require_signals_complete([Backend::observe_completion_signal_acquire(
                &mut signal.mapping,
                signal.requested,
                slot,
            )
            .map_err(explain)?])?;
        }
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        {
            let after = self.wait_diagnostic.stamp();
            self.wait_diagnostic.observation.retirement(before, after);
        }
        if pending.wait_started.is_some() {
            add_counter(
                &mut self.context.token_program_counters.retirement_signals,
                u64::from(pending.count),
            )?;
        }
        Ok(())
    }

    fn complete(&mut self, pending: Self::Pending) -> Result<()> {
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
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        let before = self.wait_diagnostic.stamp();
        std::thread::sleep(Duration::from_micros(50));
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        {
            let after = self.wait_diagnostic.stamp();
            self.wait_diagnostic.observation.pause(before, after);
        }
        Ok(())
    }

    fn poison(&mut self) {
        self.context.ordered_batch_poisoned = true;
    }
}

pub(super) fn expose_program(
    program: AqlPreparedKernelDispatchProgramV1,
    target: &mut impl OrderedExposure,
) -> Result<()> {
    target.advance_write(program.packet_count())?;
    program.publish_with(target)?;
    target.publication_checkpoint()?;
    target.ring_final_doorbell()
}

pub(super) fn expose_closed_program(
    program: AqlPreparedClosedKernelDispatchProgramV1,
    target: &mut (impl OrderedExposure + AqlClosedProgramPublicationTargetV1),
) -> Result<()> {
    target.advance_write(program.packet_count())?;
    program.publish_with(target)?;
    target.publication_checkpoint()?;
    target.ring_final_doorbell()
}

impl AqlClosedProgramPublicationTargetV1 for OrderedPublication<'_> {
    fn publish_closed_release_header(
        &mut self,
        index: u32,
        header: AqlClosedProgramHeaderV1,
    ) -> Result<()> {
        if !self.context.token_program_enabled
            || !self.context.token_program_native
            || !self.context.token_program_boundary_fences
            || self.context.ordered64_wait_policy != Ordered64WaitPolicy::Sleep50usV1
            || !header.matches_position(index, self.reservation.packet_count())
        {
            return Err("closed-program publication mode or position".into());
        }
        let entry = self
            .reservation
            .entry(index)
            .ok_or("closed-program publication slot")?;
        Backend::publish_closed_program_aql_header(
            &mut self.context.internal[RING].mapping,
            RING_BYTES,
            entry.slot_index(),
            index,
            self.reservation.packet_count(),
            header,
        )
        .map_err(explain)
    }
}

impl Context {
    pub(in crate::engineering_gfx950) fn run_prepared_token_program(
        &mut self,
        prepared: Vec<PreparedDispatch>,
        deadline: Instant,
    ) -> Result<()> {
        if !self.token_program_enabled
            || !self.token_program_native
            || self.ordered64_wait_policy != Ordered64WaitPolicy::Sleep50usV1
        {
            return Err("native token program requires its separate diagnostic entry".into());
        }
        let count = prepared.len();
        let mut native = NativeProgram {
            context: self,
            prepared: Some(prepared),
            #[cfg(feature = "engineering-native-wait-diagnostics")]
            wait_diagnostic: wait_diagnostic::Diagnostic::new(),
        };
        let result = run_ordered_batch_deadline_bounded(
            &mut native,
            count,
            600_000,
            MAX_TOKEN_PROGRAM_DISPATCHES_V1,
            Some(deadline),
        );
        #[cfg(feature = "engineering-native-wait-diagnostics")]
        native.wait_diagnostic.emit(
            native.context.queue_epoch,
            native.context.ring.write(),
            count,
            result.is_ok(),
        );
        result?;
        Ok(())
    }

    pub(in crate::engineering_gfx950) fn release_token_program_storage(&mut self) -> Result<()> {
        while let Some(allocation) = self.token_program_storage.allocations.pop() {
            self.release_resource(allocation)?;
        }
        self.token_program_storage.layout = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_token_program_native_tests.rs"]
mod tests;
