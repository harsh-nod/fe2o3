//! CPU receipt composition only. Addresses are inert; no Linux or GPU authority is created.

#![allow(clippy::result_large_err)]

use super::*;
use crate::queue::completion::{
    COMPLETION_SIGNAL_CAPACITY_V1, CompletionBatchOccurrenceV1, CompletionCustodySnapshotV1,
    CompletionDispatchGenerationBindingV1, Gfx942ComputeEventOccurrenceV1,
};
use crate::queue::dispatch_binding::CpuDispatchOwnerSnapshotV1;
use crate::queue::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1 as DispatchOwner;
use fe2o3_aql::{AMD_SIGNAL_VALUE_PENDING_V1, AqlCompletionObservationV1};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
    MemoryAllocationKeyV1, MemoryMappingKeyV1, PhysicalDeviceIdV1, VmIdV1, VmKeyV1,
};
use std::sync::atomic::AtomicI64;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

/// Equality-only identity of one fixture-issued receipt. It confers no native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuDispatchIdentityV1 {
    dispatch: DispatchEpochIdentityV1,
    completion: CompletionBatchOccurrenceV1,
}

impl CpuDispatchIdentityV1 {
    /// Passive receipt comparison, including after terminal quarantine.
    pub fn matches_published(self, batch: &Gfx942DispatchBatchV1<1>) -> bool {
        let (completion, dispatch) = batch.cpu_fixture_parts();
        self.dispatch == dispatch
            && completion
                .occurrence_v1()
                .is_ok_and(|occurrence| occurrence == self.completion)
    }

    pub fn matches_completed(self, batch: &Gfx942CompletedDispatchBatchV1<1>) -> bool {
        let (completion, dispatch) = batch.cpu_fixture_parts();
        self.dispatch == dispatch
            && completion
                .occurrence_v1()
                .is_ok_and(|occurrence| occurrence == self.completion)
    }
}

/// Addressless, equality-only owner bookkeeping and CPU signal snapshot.
#[derive(Eq, PartialEq)]
pub struct CpuLaneSnapshotV1 {
    dispatch: CpuDispatchOwnerSnapshotV1,
    completion: CompletionCustodySnapshotV1,
    event_ledger: (u64, usize, usize),
    signals: Vec<i64>,
    observations: usize,
    resets: usize,
    next_packet: u64,
    saturated: bool,
    pins: Vec<CpuDispatchIdentityV1>,
}

impl fmt::Debug for CpuLaneSnapshotV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CpuLaneSnapshotV1")
            .finish_non_exhaustive()
    }
}

impl CpuLaneSnapshotV1 {
    pub fn event_ledger_counts(&self) -> (u64, usize, usize) {
        self.event_ledger
    }

    pub fn reset_signals(&self) -> usize {
        self.resets
    }

    /// Compares custody and I/O, allowing only the dispatch poison bit to differ.
    /// Completion snapshots preserve slot records and ledger storage, not the owner phase.
    pub fn same_custody(&self, other: &Self) -> bool {
        self.dispatch.same_custody(&other.dispatch)
            && self.completion == other.completion
            && self.event_ledger == other.event_ledger
            && self.signals == other.signals
            && self.observations == other.observations
            && self.resets == other.resets
            && self.next_packet == other.next_packet
            && self.saturated == other.saturated
            && self.pins == other.pins
    }
}

struct CpuSignals {
    values: Vec<AtomicI64>,
    observations: usize,
    resets: usize,
}

impl CpuSignals {
    fn new() -> Self {
        Self {
            values: (0..COMPLETION_SIGNAL_CAPACITY_V1)
                .map(|_| AtomicI64::new(AMD_SIGNAL_VALUE_PENDING_V1))
                .collect(),
            observations: 0,
            resets: 0,
        }
    }

    fn complete(&self, slot: u32) {
        self.values[slot as usize].store(0, Ordering::Release);
    }
}

impl NativeCompletionSignalBackendV1 for CpuSignals {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        // CPU storage is owned for this entire callback; no Linux currentness claim.
        Ok(())
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        slot: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        let value = self
            .values
            .get(slot as usize)
            .ok_or(Gfx942CompletionErrorV1::Observation)?;
        self.observations += 1;
        Ok(classify_acquired_completion_value_v1(
            value.load(Ordering::Acquire),
        ))
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slots: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        slots
            .iter()
            .map(|&slot| self.observe_one_acquire_in_current_scope(slot))
            .collect()
    }

    fn reset_pending_release(&mut self, slot: u32) -> Result<(), Gfx942CompletionErrorV1> {
        let value = self
            .values
            .get(slot as usize)
            .ok_or(Gfx942CompletionErrorV1::Recycle)?;
        value.store(AMD_SIGNAL_VALUE_PENDING_V1, Ordering::Release);
        self.resets += 1;
        Ok(())
    }
}

struct CpuLane {
    dispatch: DispatchOwner,
    signals: CpuSignals,
    saturation: Option<Gfx942CompletionBatchV1<COMPLETION_SIGNAL_CAPACITY_V1>>,
    next_packet: u64,
    pins: Vec<(CpuDispatchIdentityV1, Gfx942ComputeEventOccurrenceV1)>,
}

impl CpuLane {
    fn new() -> Self {
        Self {
            dispatch: DispatchOwner::new(),
            signals: CpuSignals::new(),
            saturation: None,
            next_packet: 1,
            pins: Vec::new(),
        }
    }
}

/// Opaque CPU queue with primary and auxiliary lanes and genuine lower receipt owners.
///
/// The fixture cannot materialize native code or memory and exposes no raw session.
/// Terminal operations retain the normal process-global poison behavior. Run tests
/// that intentionally terminalize or unwind in a child process.
pub struct CpuFixedDispatchFixtureV1 {
    session: ComputeAqlQueueSessionV1,
    lanes: [CpuLane; 2],
}

impl CpuFixedDispatchFixtureV1 {
    pub fn new() -> Result<Self, ComputeAqlQueueSessionErrorV1> {
        let id = NEXT_FIXTURE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("CPU fixture identity exhausted")
            })?;
        let queue = queue_key(id, 1);
        let mut session = cpu_session(queue);
        session
            .auxiliary_compute_lanes
            .push(AuxiliaryComputeLaneSlotV1 {
                generation: 1,
                state: Some(cpu_auxiliary(queue_key(id, 2))),
            });
        Ok(Self {
            session,
            lanes: [CpuLane::new(), CpuLane::new()],
        })
    }

    pub fn primary_lane(&self) -> ComputeAqlQueueLaneV1 {
        self.session.primary_compute_lane_v1()
    }

    pub fn auxiliary_lane(&self) -> ComputeAqlQueueLaneV1 {
        ComputeAqlQueueLaneV1 {
            session: self.session.compute_lane_session,
            ordinal: 1,
            generation: 1,
        }
    }

    pub fn with_lane<R>(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
        operation: impl FnOnce(&mut CpuFixedDispatchLaneV1<'_>) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        // Admit before selecting an external logical owner. The native callback still
        // performs its own admission and restores AUX state before unwinding.
        admit_compute_lane_v1(
            self.session.compute_lane_session,
            &self.session.auxiliary_compute_lanes,
            lane,
        )?;
        let state = &mut self.lanes[lane.ordinal];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.session.with_compute_lane_v1(lane, |selected| {
                operation(&mut CpuFixedDispatchLaneV1 {
                    session: selected.session,
                    lane: selected.lane,
                    state,
                })
            })
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                for lane in &mut self.lanes {
                    lane.dispatch.poison();
                }
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub fn poison_terminal(&mut self) {
        self.session.poison_terminal();
        for lane in &mut self.lanes {
            lane.dispatch.poison();
        }
        poison_process_global_after_dispatch_terminal_v1();
    }

    pub fn is_terminal(&self) -> bool {
        self.session.terminal_poisoned
    }

    /// Hostile CPU-owner state for testing accepted release failure, not a receipt.
    pub fn poison_source_event_owner(
        &mut self,
        event: &Gfx942ComputeDependencyEventV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session.with_compute_lane_v1(event.lane, |lane| {
            lane.session.completion_owner.poison_owner();
        })
    }

    /// Uses the public release path, including owning-lane selection and restoration.
    pub fn release_dependency_event(
        &mut self,
        event: Gfx942ComputeDependencyEventV1,
    ) -> Result<
        super::super::completion::Gfx942ComputeEventReleaseObservationV1,
        Gfx942ComputeDependencyEventReleaseFailureV1,
    > {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.session.release_compute_dependency_event_v1(event)
        }));
        if self.session.terminal_poisoned {
            for lane in &mut self.lanes {
                lane.dispatch.poison();
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Valid even after poisoning; does not select a lane or grant execution access.
    pub fn snapshots(&self) -> [CpuLaneSnapshotV1; 2] {
        std::array::from_fn(|index| {
            let completion = if index == 0 {
                &self.session.completion_owner
            } else {
                &self.session.auxiliary_compute_lanes[0]
                    .state
                    .as_ref()
                    .expect("restored fixture AUX owner")
                    .completion_owner
            };
            snapshot(&self.lanes[index], completion)
        })
    }

    /// Checks every owner and signal; this neither repairs state nor disposes receipts.
    pub fn ensure_clean(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.session.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        self.session.completion_owner.ensure_releasable()?;
        self.session
            .dependency_owner
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        self.session.auxiliary_compute_lanes[0]
            .state
            .as_ref()
            .expect("fixture AUX owner")
            .completion_owner
            .ensure_releasable()?;
        for lane in &self.lanes {
            lane.dispatch.ensure_releasable()?;
            if lane.saturation.is_some()
                || !lane.pins.is_empty()
                || lane
                    .signals
                    .values
                    .iter()
                    .any(|signal| signal.load(Ordering::Acquire) != AMD_SIGNAL_VALUE_PENDING_V1)
            {
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "CPU fixture signals still retained",
                ));
            }
        }
        Ok(())
    }
}

/// Callback-scoped CPU I/O, preserving the exact move-only native receipt types.
pub struct CpuFixedDispatchLaneV1<'a> {
    session: &'a mut ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
    state: &'a mut CpuLane,
}

struct CpuSourceRecipe<'a>(&'a mut DispatchOwner);

impl fixed_dispatch::DependencySourceRecipeV1<1> for CpuSourceRecipe<'_> {
    fn bind(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; 1]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    > {
        let packet = template(session.key, self.0.next_generation());
        let templates = Box::new([packet]);
        self.0
            .reserve_one(session.key, packet)
            .map(|identity| (templates, identity))
    }

    fn mark_published(
        &mut self,
        _session: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<1>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.0.mark_published(identity, completion)
    }

    fn cancel(
        &mut self,
        _session: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.0.cancel(identity)
    }
}

impl CpuFixedDispatchLaneV1<'_> {
    pub fn source_identity(
        &self,
        source: &Gfx942ComputeDependencySourceBatchV1<1>,
    ) -> Result<CpuDispatchIdentityV1, ComputeAqlQueueSessionErrorV1> {
        self.identity(&source.batch)
    }

    /// Burned event identity frontier, live event count, and live reader count.
    pub fn event_ledger_counts(&self) -> (u64, usize, usize) {
        self.session
            .completion_owner
            .dependency_ledger_counts_for_test()
    }

    /// Genuine source-event custody; only native packet publication is replaced.
    pub fn submit_dependency_source(
        &mut self,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<1>, Gfx942FixedDispatchSubmissionFailureV1>
    {
        self.submit_source_using_cpu_publication(false)
    }

    /// Injects no-effect native ring refusal through the actual rollback body.
    pub fn submit_dependency_source_backpressured(
        &mut self,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<1>, Gfx942FixedDispatchSubmissionFailureV1>
    {
        self.submit_source_using_cpu_publication(true)
    }

    fn submit_source_using_cpu_publication(
        &mut self,
        ring_full: bool,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<1>, Gfx942FixedDispatchSubmissionFailureV1>
    {
        let packet = self.state.next_packet;
        let next = packet.checked_add(1).ok_or(
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Contract("CPU packet identity exhausted"),
            ),
        )?;
        let result = self.session.submit_dependency_source_using_v1(
            self.lane,
            &mut CpuSourceRecipe(&mut self.state.dispatch),
            |_, packets| {
                assert_eq!(packets.packet_count(), 1);
                if ring_full {
                    return Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                        NativeAqlSubmissionErrorV1::Ring(
                            fe2o3_aql::AqlRingReservationError::InsufficientSpace {
                                requested: 1,
                                available: 0,
                            },
                        ),
                    ));
                }
                Ok(packet)
            },
        );
        if result.is_ok() {
            self.state.next_packet = next;
        } else if self.session.terminal_poisoned {
            self.state.dispatch.poison();
        }
        result
    }

    pub fn snapshot(&self) -> CpuLaneSnapshotV1 {
        snapshot(self.state, &self.session.completion_owner)
    }

    fn require_live(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.session.terminal_poisoned {
            Err(Gfx942DispatchBindingErrorV1::Poisoned.into())
        } else {
            Ok(())
        }
    }

    /// Uses real bound-event pins. Only publication I/O is replaced by a CPU packet id.
    pub fn submit_pinned(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<1>, ComputeAqlQueueSessionErrorV1> {
        self.require_live()?;
        self.state
            .pins
            .try_reserve(1)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("CPU pin capacity"))?;
        let packet = self.state.next_packet;
        let next = packet
            .checked_add(1)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "CPU packet identity exhausted",
            ))?;
        let result = (|| {
            let acceptance = self
                .session
                .dependency_owner
                .reserve_acceptance_epoch()
                .map_err(map_dependency_target_use_error_v1)?;
            let template = template(self.session.key, self.state.dispatch.next_generation());
            let identity = self
                .state
                .dispatch
                .reserve_one(self.session.key, template)?;
            let bound = self.session.completion_owner.bind_batch([template])?;
            let mut events = self
                .session
                .completion_owner
                .record_dependency_event_batch_for_bound_v1(
                    acceptance.session_occurrence(),
                    acceptance.epoch(),
                    &bound,
                )?;
            let event = events.pop().expect("one bound packet");
            let (_, retention) = bound.into_parts();
            let completion = self
                .session
                .completion_owner
                .mark_published(retention, packet)?;
            let event = self
                .session
                .completion_owner
                .bind_dependency_event_v1(event, &completion)
                .map_err(|(error, _)| error)?;
            self.state.dispatch.mark_published(identity, &completion)?;
            let batch = wrap_published(completion, identity);
            self.state.pins.push((self.identity(&batch)?, event));
            self.state.next_packet = next;
            Ok(batch)
        })();
        if result.is_err() {
            self.state.dispatch.poison();
        }
        self.session
            .terminalize_fixed_dispatch_observation_result_v1(result)
    }

    pub fn release_pin(
        &mut self,
        identity: CpuDispatchIdentityV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_live()?;
        let index = self
            .state
            .pins
            .iter()
            .position(|(id, _)| *id == identity)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "CPU pin identity absent",
            ))?;
        let (_, event) = self.state.pins.remove(index);
        match self
            .session
            .completion_owner
            .release_dependency_event_v1(event)
        {
            Ok(_) => Ok(()),
            Err((error, event)) => {
                self.state.pins.insert(index, (identity, event));
                Err(error.into())
            }
        }
    }

    pub fn submit(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<1>, Gfx942FixedDispatchSubmissionFailureV1> {
        let queue = self.session.key;
        let packet = self.state.next_packet;
        let next_packet = packet.checked_add(1).ok_or(
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Contract("CPU packet identity exhausted"),
            ),
        )?;
        let result = self
            .session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut self.state.dispatch,
                |generation| template(queue, generation),
                |_, packets| {
                    assert_eq!(packets.packet_count(), 1);
                    Ok(packet)
                },
            )
            .map_err(FixedDispatchSubmissionFailureV1::into_public);
        if result.is_ok() {
            self.state.next_packet = next_packet;
        }
        result
    }

    pub fn identity(
        &self,
        batch: &Gfx942DispatchBatchV1<1>,
    ) -> Result<CpuDispatchIdentityV1, ComputeAqlQueueSessionErrorV1> {
        self.require_live()?;
        let (completion, dispatch) = batch.cpu_fixture_parts();
        let occurrence = self
            .state
            .dispatch
            .validate_published(dispatch, completion)?;
        Ok(CpuDispatchIdentityV1 {
            dispatch,
            completion: occurrence,
        })
    }

    /// Changes only the CPU signal, not completion or dispatch-owner metadata.
    pub fn complete_signal(
        &mut self,
        batch: &Gfx942DispatchBatchV1<1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.identity(batch)?;
        let slot = batch
            .cpu_fixture_parts()
            .0
            .first_packet_and_signal_slot()?
            .1;
        self.state.signals.complete(slot);
        Ok(())
    }

    pub fn poll(
        &mut self,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<Gfx942DispatchPollV1<1>, ComputeAqlQueueSessionErrorV1> {
        self.require_live()?;
        let result = (|| {
            let (completion, identity) = unwrap_published(batch);
            let occurrence = self
                .state
                .dispatch
                .validate_published(identity, &completion)?;
            match self
                .session
                .completion_owner
                .observe_once_with_progress(completion, &mut self.state.signals)?
            {
                Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => Ok(
                    Gfx942DispatchPollV1::Pending(wrap_published(batch, identity)),
                ),
                Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                    self.state.dispatch.mark_completed(identity, occurrence)?;
                    Ok(Gfx942DispatchPollV1::Ready(wrap_completed(
                        completed, identity,
                    )))
                }
            }
        })();
        if result.is_err() {
            self.state.dispatch.poison();
        }
        self.session
            .terminalize_fixed_dispatch_observation_result_v1(result)
    }

    pub fn recycle(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<1>> {
        self.require_live()
            .map_err(|error| Gfx942FixedDispatchRecycleFailureV1 {
                error,
                retryable_completed: None,
            })?;
        let (completion, identity) = unwrap_completed(completed);
        let result = match self
            .state
            .dispatch
            .validate_completed(identity, &completion)
        {
            Err(error) => Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: error.into(),
                retryable_completed: None,
            }),
            Ok(occurrence) => match self
                .session
                .completion_owner
                .recycle_retaining(completion, &mut self.state.signals)
            {
                Ok(observation) => self
                    .state
                    .dispatch
                    .mark_recycled(identity, occurrence)
                    .map(|()| observation)
                    .map_err(|error| Gfx942FixedDispatchRecycleFailureV1 {
                        error: error.into(),
                        retryable_completed: None,
                    }),
                Err((error, completion)) => Err(
                    Gfx942FixedDispatchRecycleFailureV1::from_completion_failure(
                        error.into(),
                        completion,
                        identity,
                    ),
                ),
            },
        };
        if result
            .as_ref()
            .is_err_and(|failure| failure.retryable_completed.is_none())
        {
            self.state.dispatch.poison();
        }
        self.session
            .terminalize_fixed_dispatch_recycle_result_v1(result)
    }

    /// Occupies the real completion arena without consuming dispatch-epoch capacity.
    pub fn saturate_signals(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_live()?;
        self.session.completion_owner.ensure_releasable()?;
        let count = COMPLETION_SIGNAL_CAPACITY_V1 as u64;
        let next = self.state.next_packet.checked_add(count).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("CPU packet identity exhausted"),
        )?;
        self.state.saturation = Some(
            self.session
                .completion_owner
                .saturate_cpu_fixture(template(self.session.key, 1), next - 1)?,
        );
        self.state.next_packet = next;
        Ok(())
    }

    /// Drains saturation through ordinary acquire observation and release recycling.
    pub fn drain_saturation(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_live()?;
        let batch = self
            .state
            .saturation
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "CPU saturation receipt absent",
            ))?;
        for slot in 0..COMPLETION_SIGNAL_CAPACITY_V1 {
            self.state.signals.complete(slot as u32);
        }
        let result = (|| match self
            .session
            .completion_owner
            .observe_once_with_progress(batch, &mut self.state.signals)?
        {
            Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                self.session
                    .completion_owner
                    .recycle_retaining(completed, &mut self.state.signals)
                    .map_err(|(error, _)| error)?;
                Ok(())
            }
            Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                self.state.saturation = Some(batch);
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "CPU saturation unexpectedly pending",
                ))
            }
        })();
        self.session
            .terminalize_fixed_dispatch_observation_result_v1(result)
    }

    pub fn live_epochs(&self) -> usize {
        self.state.dispatch.live_epoch_count()
    }
    pub fn next_generation(&self) -> u64 {
        self.state.dispatch.next_generation()
    }
    pub fn available_signals(&self) -> usize {
        self.session.completion_owner.state_snapshot_for_test().1
    }
    pub fn observed_signals(&self) -> usize {
        self.state.signals.observations
    }
    pub fn reset_signals(&self) -> usize {
        self.state.signals.resets
    }
}

fn snapshot(state: &CpuLane, completion: &CompletionSignalArenaOwnerV1) -> CpuLaneSnapshotV1 {
    CpuLaneSnapshotV1 {
        dispatch: state.dispatch.cpu_snapshot(),
        completion: completion.custody_snapshot_for_test(),
        event_ledger: completion.dependency_ledger_counts_for_test(),
        signals: state
            .signals
            .values
            .iter()
            .map(|signal| signal.load(Ordering::Acquire))
            .collect(),
        observations: state.signals.observations,
        resets: state.signals.resets,
        next_packet: state.next_packet,
        saturated: state.saturation.is_some(),
        pins: state.pins.iter().map(|(identity, _)| *identity).collect(),
    }
}

fn queue_key(fixture: u64, lane: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(0),
                generation: DeviceGenerationV1(1),
            },
            id: VmIdV1(fixture),
        },
        id: QueueInstanceIdV1(lane),
        generation: QueueGenerationV1(1),
    }
}

fn template(queue: QueueKeyV1, generation: u64) -> CompletionPacketTemplateV1 {
    let mapping = |id| MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 {
            vm: queue.vm,
            id: AllocationIdV1(id),
            generation: AllocationGenerationV1(1),
        },
        id: MappingIdV1(id),
    };
    CompletionPacketTemplateV1::new(
        fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior,
        0,
        0,
        fe2o3_aql::ObservedGpuAddressV1::new(0x40_0000).unwrap(),
        fe2o3_aql::ObservedGpuAddressV1::new(0x50_0000).unwrap(),
        16,
        CompletionDispatchGenerationBindingV1::new(queue, mapping(30), mapping(31), generation),
    )
}

fn observation() -> ComputeAqlQueueObservationV1 {
    ComputeAqlQueueObservationV1 {
        queue_id: 0,
        ring_bytes: 0,
        doorbell_slice_bytes: 0,
        doorbell_byte_offset: 0,
        event_id: 0,
        cwsr_shadow_pages: 0,
    }
}

fn cpu_auxiliary(queue: QueueKeyV1) -> ComputeAqlQueueLaneStateV1 {
    ComputeAqlQueueLaneStateV1 {
        key: queue,
        doorbell: None,
        submission: None,
        completion_signals: None,
        completion_owner: QueueOwnerSlotV1(Some(
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
        )),
        dispatch: None,
        unpublished_dispatch: UnpublishedDispatchStateV1::default(),
        detached_data_count: 0,
        detached_dispatch_generation: None,
        detached_data_identities: Vec::new(),
        detached_next_insertion_index: None,
        exception: None,
        observation: observation(),
    }
}

fn cpu_session(queue: QueueKeyV1) -> ComputeAqlQueueSessionV1 {
    ComputeAqlQueueSessionV1 {
        dispatch_capacity: Gfx942FixedDispatchCapacityV1::default(),
        engine: None,
        key: queue,
        compute_lane_session: queue,
        doorbell: None,
        submission: None,
        completion_signals: None,
        completion_owner: QueueOwnerSlotV1(Some(
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
        )),
        dependency_owner: QueueOwnerSlotV1(Some(
            ComputeDependencySessionOwnerV1::new(queue.vm.id.0).unwrap(),
        )),
        terminal_dependency: None,
        dispatch: None,
        unpublished_dispatch: UnpublishedDispatchStateV1::default(),
        detached_data_count: 0,
        detached_dispatch_generation: None,
        detached_data_identities: Vec::new(),
        detached_next_insertion_index: None,
        persistent_compute: None,
        #[cfg(test)]
        persistent_compute_test_release: None,
        next_persistent_compute_generation: 1,
        exception: None,
        sdma: None,
        striped_sdma: None,
        xgmi_attachment: None,
        sdma_outstanding_buffers: 0,
        sdma_pool_free: Vec::new(),
        sdma_pool_trim: None,
        sdma_allocation: None,
        sdma_promotion: None,
        sdma_demotion: None,
        detached_sdma: None,
        sdma_dispatch_promotion: None,
        initialized_storage_promotion: None,
        sdma_synchronous: None,
        sdma_recycle: None,
        auxiliary_release: None,
        sdma_pool_reuse_count: 0,
        sdma_device_pool: SdmaDevicePoolConfigurationV1::default(),
        sdma_host_pool_limits: None,
        terminal_poisoned: false,
        observation: observation(),
        auxiliary_compute_lanes: Vec::new(),
    }
}
