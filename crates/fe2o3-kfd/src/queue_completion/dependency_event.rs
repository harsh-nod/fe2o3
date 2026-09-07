//! Addressless event occurrences and native dependency-reader custody.
//!
//! This module owns only completion-signal pin accounting. It does not publish
//! dependency packets and it does not authorize a reader release: the future
//! queue orchestration must call the crate-private release operation only after
//! the dependent dispatch completed, or after publication was proven to have
//! had no effect. Dropping an event or reader lease is deliberately inert; its
//! ledger pin remains and prevents signal reset, generation advance, and arena
//! release. There is no unwind interception here because these host-ledger
//! transitions contain no callback or native boundary.

use core::fmt;
use std::collections::HashMap;

use fe2o3_aql::{AMD_SIGNAL_BYTES_V1, AqlDependencySignalObservationV1};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};

use super::{
    COMPLETION_SIGNAL_CAPACITY_V1, CompletionBatchRetentionV1, CompletionSignalArenaOwnerV1,
    CompletionSlotLeaseV1, CompletionSlotPhaseV1, ComputeDependencyOccurrenceIdentityV1,
    Gfx942CompletionBatchV1, Gfx942CompletionErrorV1,
};

/// Maximum simultaneously retained event occurrences in one signal arena.
pub const GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1: usize = COMPLETION_SIGNAL_CAPACITY_V1;

/// Maximum simultaneously retained native dependency-reader uses per arena.
pub const GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1: usize = COMPLETION_SIGNAL_CAPACITY_V1;

/// Canonical claim boundary for addressless compute-event signal custody.
pub const GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-event-custody-r42-v1\n",
    "scope=completion-event-occurrence-and-native-dependency-reader-pin-ledger-only\n",
    "identity=session-occurrence,source-acceptance-epoch,queue-key,signal-mapping,slot-generation,dispatch-generation,packet-id;no-public-signal-address\n",
    "event=prepublication-record-is-unbound-and-pinned,postpublication-bind-exactly-once,explicit-release-independent-of-reader-release\n",
    "capacity=8192-live-event-occurrences-and-8192-live-native-readers-per-completion-arena,checked-before-growth-or-pin-mutation\n",
    "reader=one-move-only-lease-per-event-and-caller-supplied-dependent-acceptance-epoch,strictly-newer-epoch,same-session,duplicate-rejected\n",
    "contracted-premise=future-session-orchestrator-authenticates-session-wide-unique-dependent-acceptance-epochs-and-binds-each-to-one-target-publication;not-checked-by-this-ledger\n",
    "recycle=completed-source-required,event-and-reader-pin-counts-both-zero-before-any-reset-or-generation-advance\n",
    "failure=allocation-overflow-stale-duplicate-cross-session-cycle-self-and-prepublication-rejected-before-ledger-mutation;consumed-owner-returned\n",
    "drop=inert;abandoned-token-retains-ledger-pin-and-blocks-cancel-recycle-and-arena-release\n",
    "proof=host-state-machine-and-hostile-unit-tests-only\n",
    "excluded=dependency-packet-construction-or-publication,target-publication-identity-binding,runtime-facade,dependent-completion-authority,native-hardware,panic-interception,machine-checked-refinement\n",
);

/// SHA-256 of [`GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_V1`].
pub const GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_SHA256_V1: &str =
    "f8ecd29fabba9c6dd924cf8a0c41272f313d87115f41aeb7a7bd38f2b0acf0cd";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ExactCompletionOccurrenceV1 {
    session_occurrence: u64,
    source_acceptance_epoch: u64,
    batch_id: u64,
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    slot: CompletionSlotLeaseV1,
    dispatch_generation: u64,
    packet_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct DependencyReaderUseKeyV1 {
    event_id: u64,
    dependent_acceptance_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveDependencyReaderV1 {
    lease_id: u64,
    source: ExactCompletionOccurrenceV1,
}

pub(super) struct CompletionDependencyLedgerV1 {
    next_event_id: u64,
    next_reader_lease_id: u64,
    events: HashMap<u64, ExactCompletionOccurrenceV1>,
    readers: HashMap<DependencyReaderUseKeyV1, ActiveDependencyReaderV1>,
}

impl CompletionDependencyLedgerV1 {
    pub(super) fn new() -> Self {
        Self {
            next_event_id: 1,
            next_reader_lease_id: 1,
            events: HashMap::new(),
            readers: HashMap::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.events.is_empty() && self.readers.is_empty()
    }
}

/// Publication binding state of an addressless compute-event occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ComputeEventBindingStateV1 {
    /// The source dispatch owns a signal slot but has not been published.
    Unbound,
    /// The occurrence is bound to the exact published source packet.
    Bound,
}

/// Move-only, addressless custody for one recorded compute-event occurrence.
///
/// The token retains the session occurrence, acceptance epoch, queue and
/// signal-mapping identities, slot and generation, dispatch generation, and
/// (after binding) packet ID. None of those implementation identities or the
/// native signal address are publicly observable. `Drop` has no native or
/// ledger effect, so an abandoned token safely pins its completion slot.
#[must_use = "an event occurrence must be explicitly released or retained for teardown"]
pub struct Gfx942ComputeEventOccurrenceV1 {
    event_id: u64,
    exact: ExactCompletionOccurrenceV1,
}

impl Gfx942ComputeEventOccurrenceV1 {
    /// Reports whether the source packet identity has been bound.
    pub const fn binding_state(&self) -> Gfx942ComputeEventBindingStateV1 {
        if self.exact.packet_id.is_some() {
            Gfx942ComputeEventBindingStateV1::Bound
        } else {
            Gfx942ComputeEventBindingStateV1::Unbound
        }
    }
}

impl fmt::Debug for Gfx942ComputeEventOccurrenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeEventOccurrenceV1")
            .field("binding_state", &self.binding_state())
            .finish_non_exhaustive()
    }
}

/// Move-only, addressless pin for one native dependency read.
///
/// This token deliberately has no public constructor, identity accessor, or
/// signal-address accessor. Dropping it is inert and leaves the signal pinned.
#[must_use = "a native dependency reader must be explicitly released after its use completes"]
pub struct Gfx942ComputeDependencyReaderLeaseV1 {
    lease_id: u64,
    event_id: u64,
    dependent_acceptance_epoch: u64,
    source: ExactCompletionOccurrenceV1,
}

impl fmt::Debug for Gfx942ComputeDependencyReaderLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencyReaderLeaseV1")
            .finish_non_exhaustive()
    }
}

/// Evidence that one event pin was removed without resetting its signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ComputeEventReleaseObservationV1;

/// Evidence that one native-reader pin was removed without resetting its signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ComputeDependencyReaderReleaseObservationV1;

#[allow(dead_code)]
impl CompletionSignalArenaOwnerV1 {
    /// Records an event before its source batch is published.
    ///
    /// Recording pins the exact slot. The returned occurrence is unbound until
    /// `bind_compute_event_after_publication` authenticates the published
    /// packet ID. All allocation and overflow checks precede ledger mutation.
    pub(super) fn record_unbound_compute_event<const N: usize>(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<N>,
        batch_index: usize,
    ) -> Result<Gfx942ComputeEventOccurrenceV1, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_bound(retention)?;
        validate_logical_identity(session_occurrence, source_acceptance_epoch)?;
        if self.dependency_ledger.events.len() >= GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1 {
            return Err(Gfx942CompletionErrorV1::EventCapacityExhausted);
        }
        let exact = exact_occurrence(
            session_occurrence,
            source_acceptance_epoch,
            retention,
            batch_index,
            None,
        )?;
        let next_event_id = self
            .dependency_ledger
            .next_event_id
            .checked_add(1)
            .ok_or(Gfx942CompletionErrorV1::EventIdentityExhausted)?;
        let record = &self.slots[exact.slot.index as usize];
        let next_event_pins = record
            .event_pins
            .checked_add(1)
            .ok_or(Gfx942CompletionErrorV1::SignalPinCountExhausted)?;
        self.dependency_ledger
            .events
            .try_reserve(1)
            .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;

        let event_id = self.dependency_ledger.next_event_id;
        let replaced = self.dependency_ledger.events.insert(event_id, exact);
        debug_assert!(replaced.is_none());
        self.dependency_ledger.next_event_id = next_event_id;
        self.slots[exact.slot.index as usize].event_pins = next_event_pins;
        Ok(Gfx942ComputeEventOccurrenceV1 { event_id, exact })
    }

    /// Binds one unbound event to its exact source packet exactly once.
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_compute_event_after_publication<const N: usize>(
        &mut self,
        mut event: Gfx942ComputeEventOccurrenceV1,
        batch: &Gfx942CompletionBatchV1<N>,
        batch_index: usize,
    ) -> Result<
        Gfx942ComputeEventOccurrenceV1,
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        let result = (|| {
            self.require_ready()?;
            self.validate_published(&batch.retention)?;
            if event.exact.packet_id.is_some() {
                return Err(Gfx942CompletionErrorV1::EventAlreadyBound);
            }
            self.validate_active_event(&event)?;
            let packet_id = packet_id_at(&batch.retention, batch_index)?;
            let expected = exact_occurrence(
                event.exact.session_occurrence,
                event.exact.source_acceptance_epoch,
                &batch.retention,
                batch_index,
                Some(packet_id),
            )?;
            let mut unbound_expected = expected;
            unbound_expected.packet_id = None;
            if event.exact != unbound_expected {
                return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
            }
            Ok(expected)
        })();
        let exact = match result {
            Ok(exact) => exact,
            Err(error) => return Err((error, event)),
        };

        *self
            .dependency_ledger
            .events
            .get_mut(&event.event_id)
            .expect("active event validated before binding") = exact;
        event.exact = exact;
        Ok(event)
    }

    /// Reserves one native dependency read while returning event custody.
    ///
    /// The dependent acceptance epoch must be strictly newer than the source
    /// epoch. A repeated `(event, dependent epoch)` is rejected, which excludes
    /// duplicate and self edges at this low-level admission boundary. This
    /// owner does not mint or authenticate target epochs; the future session
    /// orchestrator must bind each session-wide unique epoch to one dependent
    /// publication before using this lease.
    #[allow(clippy::result_large_err)]
    pub(super) fn retain_compute_dependency_reader(
        &mut self,
        event: Gfx942ComputeEventOccurrenceV1,
        session_occurrence: u64,
        dependent_acceptance_epoch: u64,
    ) -> Result<
        (
            Gfx942ComputeEventOccurrenceV1,
            Gfx942ComputeDependencyReaderLeaseV1,
        ),
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        let result = (|| {
            self.require_ready()?;
            if event.exact.packet_id.is_none() {
                return Err(Gfx942CompletionErrorV1::EventNotPublished);
            }
            if session_occurrence == 0 || session_occurrence != event.exact.session_occurrence {
                return Err(Gfx942CompletionErrorV1::CrossSessionEvent);
            }
            if dependent_acceptance_epoch == 0 {
                return Err(Gfx942CompletionErrorV1::InvalidAcceptanceEpoch);
            }
            if dependent_acceptance_epoch == event.exact.source_acceptance_epoch {
                return Err(Gfx942CompletionErrorV1::SelfDependency);
            }
            if dependent_acceptance_epoch < event.exact.source_acceptance_epoch {
                return Err(Gfx942CompletionErrorV1::DependencyCycle);
            }
            self.validate_active_event(&event)?;
            self.validate_published_or_completed_occurrence(event.exact)?;
            let use_key = DependencyReaderUseKeyV1 {
                event_id: event.event_id,
                dependent_acceptance_epoch,
            };
            if self.dependency_ledger.readers.contains_key(&use_key) {
                return Err(Gfx942CompletionErrorV1::DuplicateDependency);
            }
            if self.dependency_ledger.readers.len() >= GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1 {
                return Err(Gfx942CompletionErrorV1::DependencyReaderCapacityExhausted);
            }
            let next_lease_id = self
                .dependency_ledger
                .next_reader_lease_id
                .checked_add(1)
                .ok_or(Gfx942CompletionErrorV1::DependencyReaderIdentityExhausted)?;
            let next_reader_pins = self.slots[event.exact.slot.index as usize]
                .native_reader_pins
                .checked_add(1)
                .ok_or(Gfx942CompletionErrorV1::SignalPinCountExhausted)?;
            self.dependency_ledger
                .readers
                .try_reserve(1)
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            Ok((use_key, next_lease_id, next_reader_pins))
        })();
        let (use_key, next_lease_id, next_reader_pins) = match result {
            Ok(prepared) => prepared,
            Err(error) => return Err((error, event)),
        };

        let lease = Gfx942ComputeDependencyReaderLeaseV1 {
            lease_id: self.dependency_ledger.next_reader_lease_id,
            event_id: event.event_id,
            dependent_acceptance_epoch,
            source: event.exact,
        };
        let replaced = self.dependency_ledger.readers.insert(
            use_key,
            ActiveDependencyReaderV1 {
                lease_id: lease.lease_id,
                source: lease.source,
            },
        );
        debug_assert!(replaced.is_none());
        self.dependency_ledger.next_reader_lease_id = next_lease_id;
        self.slots[event.exact.slot.index as usize].native_reader_pins = next_reader_pins;
        Ok((event, lease))
    }

    /// Explicitly releases an event pin without affecting reader pins.
    #[allow(clippy::result_large_err)]
    pub(super) fn release_compute_event(
        &mut self,
        event: Gfx942ComputeEventOccurrenceV1,
    ) -> Result<
        Gfx942ComputeEventReleaseObservationV1,
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        let result = (|| {
            self.require_ready()?;
            self.validate_active_event(&event)?;
            self.validate_live_occurrence(event.exact)?;
            let record = &self.slots[event.exact.slot.index as usize];
            let next = record
                .event_pins
                .checked_sub(1)
                .ok_or(Gfx942CompletionErrorV1::StaleEventOccurrence)?;
            Ok(next)
        })();
        let next = match result {
            Ok(next) => next,
            Err(error) => return Err((error, event)),
        };
        let removed = self.dependency_ledger.events.remove(&event.event_id);
        debug_assert_eq!(removed, Some(event.exact));
        self.slots[event.exact.slot.index as usize].event_pins = next;
        Ok(Gfx942ComputeEventReleaseObservationV1)
    }

    /// Explicitly releases one reader pin.
    ///
    /// The future dependency-publication owner is responsible for invoking
    /// this only after dependent completion, or after proven no-effect cancel.
    #[allow(clippy::result_large_err)]
    pub(super) fn release_compute_dependency_reader(
        &mut self,
        lease: Gfx942ComputeDependencyReaderLeaseV1,
    ) -> Result<
        Gfx942ComputeDependencyReaderReleaseObservationV1,
        (
            Gfx942CompletionErrorV1,
            Gfx942ComputeDependencyReaderLeaseV1,
        ),
    > {
        let result = (|| {
            self.require_ready()?;
            self.validate_published_or_completed_occurrence(lease.source)?;
            let use_key = DependencyReaderUseKeyV1 {
                event_id: lease.event_id,
                dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
            };
            let expected = ActiveDependencyReaderV1 {
                lease_id: lease.lease_id,
                source: lease.source,
            };
            if self.dependency_ledger.readers.get(&use_key) != Some(&expected) {
                return Err(Gfx942CompletionErrorV1::StaleDependencyReader);
            }
            let next = self.slots[lease.source.slot.index as usize]
                .native_reader_pins
                .checked_sub(1)
                .ok_or(Gfx942CompletionErrorV1::StaleDependencyReader)?;
            Ok((use_key, next))
        })();
        let (use_key, next) = match result {
            Ok(prepared) => prepared,
            Err(error) => return Err((error, lease)),
        };
        let removed = self.dependency_ledger.readers.remove(&use_key);
        debug_assert!(removed.is_some());
        self.slots[lease.source.slot.index as usize].native_reader_pins = next;
        Ok(Gfx942ComputeDependencyReaderReleaseObservationV1)
    }

    pub(super) fn project_compute_dependency_source_identity(
        &self,
        event: &Gfx942ComputeEventOccurrenceV1,
    ) -> Result<ComputeDependencyOccurrenceIdentityV1, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_active_event(event)?;
        self.validate_published_or_completed_occurrence(event.exact)?;
        Ok(ComputeDependencyOccurrenceIdentityV1 {
            session_occurrence: event.exact.session_occurrence,
            acceptance_epoch: event.exact.source_acceptance_epoch,
            batch_id: event.exact.batch_id,
            queue: event.exact.queue,
            signal_mapping: event.exact.signal_mapping,
            slot_index: event.exact.slot.index,
            slot_generation: event.exact.slot.generation,
            dispatch_generation: event.exact.dispatch_generation,
            packet_id: event.exact.packet_id,
        })
    }

    pub(super) fn validate_unbound_dependency_target_event_v1(
        &self,
        event: &Gfx942ComputeEventOccurrenceV1,
        session_occurrence: u64,
        acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<1>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_bound(retention)?;
        self.validate_active_event(event)?;
        let expected = exact_occurrence(session_occurrence, acceptance_epoch, retention, 0, None)?;
        if event.exact != expected {
            return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
        }
        Ok(())
    }

    /// Revalidates an active native-reader lease and derives its GPU address.
    /// This is an identity projection only; it never loads the signal value.
    pub(super) fn project_native_dependency_signal_observation(
        &self,
        lease: &Gfx942ComputeDependencyReaderLeaseV1,
    ) -> Result<AqlDependencySignalObservationV1, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_published_or_completed_occurrence(lease.source)?;
        let use_key = DependencyReaderUseKeyV1 {
            event_id: lease.event_id,
            dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
        };
        let expected = ActiveDependencyReaderV1 {
            lease_id: lease.lease_id,
            source: lease.source,
        };
        if self.dependency_ledger.readers.get(&use_key) != Some(&expected) {
            return Err(Gfx942CompletionErrorV1::StaleDependencyReader);
        }
        let offset = u64::from(lease.source.slot.index)
            .checked_mul(AMD_SIGNAL_BYTES_V1 as u64)
            .ok_or(Gfx942CompletionErrorV1::InvalidArena(
                "dependency signal slot offset",
            ))?;
        let raw =
            self.gpu_base
                .checked_add(offset)
                .ok_or(Gfx942CompletionErrorV1::InvalidArena(
                    "dependency signal address",
                ))?;
        AqlDependencySignalObservationV1::new(raw)
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("dependency signal observation"))
    }

    fn validate_active_event(
        &self,
        event: &Gfx942ComputeEventOccurrenceV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if self.dependency_ledger.events.get(&event.event_id) != Some(&event.exact) {
            return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
        }
        Ok(())
    }

    fn validate_live_occurrence(
        &self,
        exact: ExactCompletionOccurrenceV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if exact.queue != self.queue || exact.signal_mapping != self.signal_mapping {
            return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
        }
        let Some(record) = self.slots.get(exact.slot.index as usize) else {
            return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
        };
        if record.generation != exact.slot.generation
            || !matches!(
                record.phase,
                CompletionSlotPhaseV1::Bound { batch_id }
                    | CompletionSlotPhaseV1::Published { batch_id }
                    | CompletionSlotPhaseV1::Completed { batch_id }
                    if batch_id == exact.batch_id
            )
        {
            return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
        }
        Ok(())
    }

    fn validate_published_or_completed_occurrence(
        &self,
        exact: ExactCompletionOccurrenceV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.validate_live_occurrence(exact)?;
        let phase = self.slots[exact.slot.index as usize].phase;
        if exact.packet_id.is_none()
            || !matches!(
                phase,
                CompletionSlotPhaseV1::Published { batch_id }
                    | CompletionSlotPhaseV1::Completed { batch_id }
                    if batch_id == exact.batch_id
            )
        {
            return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
        }
        Ok(())
    }
}

fn validate_logical_identity(
    session_occurrence: u64,
    acceptance_epoch: u64,
) -> Result<(), Gfx942CompletionErrorV1> {
    if session_occurrence == 0 {
        return Err(Gfx942CompletionErrorV1::InvalidSessionOccurrence);
    }
    if acceptance_epoch == 0 {
        return Err(Gfx942CompletionErrorV1::InvalidAcceptanceEpoch);
    }
    Ok(())
}

fn exact_occurrence<const N: usize>(
    session_occurrence: u64,
    source_acceptance_epoch: u64,
    retention: &CompletionBatchRetentionV1<N>,
    batch_index: usize,
    packet_id: Option<u64>,
) -> Result<ExactCompletionOccurrenceV1, Gfx942CompletionErrorV1> {
    let slot = *retention
        .slots
        .get(batch_index)
        .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
    let dispatch = retention
        .dispatches
        .get(batch_index)
        .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
    Ok(ExactCompletionOccurrenceV1 {
        session_occurrence,
        source_acceptance_epoch,
        batch_id: retention.batch_id,
        queue: retention.queue,
        signal_mapping: retention.signal_mapping,
        slot,
        dispatch_generation: dispatch.dispatch_generation,
        packet_id,
    })
}

fn packet_id_at<const N: usize>(
    retention: &CompletionBatchRetentionV1<N>,
    batch_index: usize,
) -> Result<u64, Gfx942CompletionErrorV1> {
    if batch_index >= N {
        return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
    }
    let packet_count =
        u64::try_from(N).map_err(|_| Gfx942CompletionErrorV1::StaleBatchGeneration)?;
    let batch_index =
        u64::try_from(batch_index).map_err(|_| Gfx942CompletionErrorV1::StaleBatchGeneration)?;
    retention
        .last_packet_id
        .and_then(|last| last.checked_add(1))
        .and_then(|next| next.checked_sub(packet_count))
        .and_then(|first| first.checked_add(batch_index))
        .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_aql::{
        AMD_SIGNAL_ALIGNMENT_V1, AqlCompletionObservationV1, AqlDispatchGeometryV1,
        AqlDispatchOrderingV1, ObservedGpuAddressV1,
    };
    use fe2o3_runtime_model::{
        AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
        MemoryAllocationKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1, VmIdV1,
        VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    use crate::queue::completion::{
        CompletionDispatchGenerationBindingV1, CompletionPacketTemplateV1, Gfx942CompletedBatchV1,
        Gfx942CompletionPollV1, NativeCompletionSignalBackendV1,
    };

    const SESSION: u64 = 17;
    const SOURCE_EPOCH: u64 = 23;
    const DEPENDENT_EPOCH: u64 = 29;

    struct CompletedBackend {
        reset_calls: usize,
    }

    impl NativeCompletionSignalBackendV1 for CompletedBackend {
        fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
            Ok(())
        }

        fn observe_one_acquire_in_current_scope(
            &mut self,
            _slot_index: u32,
        ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
            Ok(AqlCompletionObservationV1::Completed)
        }

        fn observe_batch_acquire_in_current_scope(
            &mut self,
            slot_indices: &[u32],
        ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
            Ok(vec![
                AqlCompletionObservationV1::Completed;
                slot_indices.len()
            ])
        }

        fn reset_pending_release(
            &mut self,
            _slot_index: u32,
        ) -> Result<(), Gfx942CompletionErrorV1> {
            self.reset_calls += 1;
            Ok(())
        }
    }

    fn queue() -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(7),
                    generation: DeviceGenerationV1(3),
                },
                id: VmIdV1(11),
            },
            id: QueueInstanceIdV1(19),
            generation: QueueGenerationV1(5),
        }
    }

    fn mapping(id: u64) -> MemoryMappingKeyV1 {
        MemoryMappingKeyV1 {
            allocation: MemoryAllocationKeyV1 {
                vm: queue().vm,
                id: AllocationIdV1(id),
                generation: AllocationGenerationV1(1),
            },
            id: MappingIdV1(id),
        }
    }

    fn template() -> CompletionPacketTemplateV1 {
        CompletionPacketTemplateV1::new(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            0,
            ObservedGpuAddressV1::new(0x40_0000).unwrap(),
            ObservedGpuAddressV1::new(0x50_0000).unwrap(),
            16,
            CompletionDispatchGenerationBindingV1::new(queue(), mapping(30), mapping(31), 41),
        )
    }

    fn owner() -> CompletionSignalArenaOwnerV1 {
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue())
    }

    fn unbound(
        owner: &mut CompletionSignalArenaOwnerV1,
    ) -> (
        CompletionBatchRetentionV1<1>,
        Gfx942ComputeEventOccurrenceV1,
    ) {
        let bound = owner.bind_batch([template()]).unwrap();
        let (_, retention) = bound.into_parts();
        let event = owner
            .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0)
            .unwrap();
        (retention, event)
    }

    fn published(
        owner: &mut CompletionSignalArenaOwnerV1,
    ) -> (Gfx942CompletionBatchV1<1>, Gfx942ComputeEventOccurrenceV1) {
        let (retention, event) = unbound(owner);
        let batch = owner.mark_published(retention, 101).unwrap();
        let event = owner
            .bind_compute_event_after_publication(event, &batch, 0)
            .unwrap();
        (batch, event)
    }

    fn complete(
        owner: &mut CompletionSignalArenaOwnerV1,
        batch: Gfx942CompletionBatchV1<1>,
        backend: &mut CompletedBackend,
    ) -> Gfx942CompletedBatchV1<1> {
        match owner.observe_once(batch, backend).unwrap() {
            Gfx942CompletionPollV1::Ready(completed) => completed,
            Gfx942CompletionPollV1::Pending(_) => panic!("completed backend reported pending"),
        }
    }

    fn duplicate_event(event: &Gfx942ComputeEventOccurrenceV1) -> Gfx942ComputeEventOccurrenceV1 {
        Gfx942ComputeEventOccurrenceV1 {
            event_id: event.event_id,
            exact: event.exact,
        }
    }

    fn duplicate_reader(
        lease: &Gfx942ComputeDependencyReaderLeaseV1,
    ) -> Gfx942ComputeDependencyReaderLeaseV1 {
        Gfx942ComputeDependencyReaderLeaseV1 {
            lease_id: lease.lease_id,
            event_id: lease.event_id,
            dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
            source: lease.source,
        }
    }

    #[test]
    fn unbound_event_pins_cancel_and_binds_exactly_once() {
        let mut owner = owner();
        let (retention, event) = unbound(&mut owner);
        assert_eq!(
            event.binding_state(),
            Gfx942ComputeEventBindingStateV1::Unbound
        );
        let retained = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
        {
            Err((Gfx942CompletionErrorV1::EventNotPublished, event)) => event,
            other => panic!("unexpected prepublication result: {other:?}"),
        };
        let retention = match owner.cancel_bound_retaining(retention) {
            Err((
                Gfx942CompletionErrorV1::SignalPinned {
                    event_pins: 1,
                    native_reader_pins: 0,
                    ..
                },
                retention,
            )) => retention,
            other => panic!("unexpected pinned cancellation result: {other:?}"),
        };
        owner.release_compute_event(retained).unwrap();
        owner.cancel_bound_retaining(retention).unwrap();

        let (retention, event) = unbound(&mut owner);
        let batch = owner.mark_published(retention, 101).unwrap();
        let event = owner
            .bind_compute_event_after_publication(event, &batch, 0)
            .unwrap();
        assert_eq!(
            event.binding_state(),
            Gfx942ComputeEventBindingStateV1::Bound
        );
        let event = match owner.bind_compute_event_after_publication(event, &batch, 0) {
            Err((Gfx942CompletionErrorV1::EventAlreadyBound, event)) => event,
            other => panic!("unexpected second bind result: {other:?}"),
        };
        owner.release_compute_event(event).unwrap();
    }

    #[test]
    fn event_release_does_not_release_reader_and_both_block_recycle() {
        let mut owner = owner();
        let (batch, event) = published(&mut owner);
        let (event, lease) = owner
            .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        let mut backend = CompletedBackend { reset_calls: 0 };
        let handoff = match owner
            .observe_once_with_progress_current_handoff_retaining(batch, &mut backend)
            .unwrap()
        {
            super::super::CompletionPollWithCurrentnessHandoffV1::Ready { handoff, .. } => handoff,
            super::super::CompletionPollWithCurrentnessHandoffV1::Pending { .. } => {
                panic!("completed backend reported pending")
            }
        };
        let handoff = match owner.recycle_current_handoff_retaining(handoff, &mut backend) {
            Err((
                Gfx942CompletionErrorV1::SignalPinned {
                    event_pins: 1,
                    native_reader_pins: 1,
                    ..
                },
                handoff,
            )) => handoff,
            other => panic!("unexpected pinned recycle result: {other:?}"),
        };
        assert_eq!(backend.reset_calls, 0);

        owner.release_compute_event(event).unwrap();
        let handoff = match owner.recycle_current_handoff_retaining(handoff, &mut backend) {
            Err((
                Gfx942CompletionErrorV1::SignalPinned {
                    event_pins: 0,
                    native_reader_pins: 1,
                    ..
                },
                handoff,
            )) => handoff,
            other => panic!("unexpected reader-pinned recycle result: {other:?}"),
        };
        assert_eq!(backend.reset_calls, 0);
        owner.release_compute_dependency_reader(lease).unwrap();
        owner
            .recycle_current_handoff_retaining(handoff, &mut backend)
            .unwrap();
        assert_eq!(backend.reset_calls, 1);
    }

    #[test]
    fn dependency_admission_rejects_cross_session_cycles_and_duplicates_without_mutation() {
        let mut owner = owner();
        let (_batch, event) = published(&mut owner);
        let event =
            match owner.retain_compute_dependency_reader(event, SESSION + 1, DEPENDENT_EPOCH) {
                Err((Gfx942CompletionErrorV1::CrossSessionEvent, event)) => event,
                other => panic!("unexpected cross-session result: {other:?}"),
            };
        let event = match owner.retain_compute_dependency_reader(event, SESSION, SOURCE_EPOCH) {
            Err((Gfx942CompletionErrorV1::SelfDependency, event)) => event,
            other => panic!("unexpected self-dependency result: {other:?}"),
        };
        let event = match owner.retain_compute_dependency_reader(event, SESSION, SOURCE_EPOCH - 1) {
            Err((Gfx942CompletionErrorV1::DependencyCycle, event)) => event,
            other => panic!("unexpected cycle result: {other:?}"),
        };
        let (event, lease) = owner
            .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        let event = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
            Err((Gfx942CompletionErrorV1::DuplicateDependency, event)) => event,
            other => panic!("unexpected duplicate result: {other:?}"),
        };
        assert_eq!(
            owner.slots[event.exact.slot.index as usize].native_reader_pins,
            1
        );
        owner.release_compute_event(event).unwrap();
        owner.release_compute_dependency_reader(lease).unwrap();
    }

    #[test]
    fn stale_generation_and_checked_overflow_return_linear_owners() {
        let mut owner = owner();
        let (_batch, event) = published(&mut owner);
        let mut substituted = duplicate_event(&event);
        substituted.exact.slot.generation += 1;
        let substituted =
            match owner.retain_compute_dependency_reader(substituted, SESSION, DEPENDENT_EPOCH) {
                Err((Gfx942CompletionErrorV1::StaleEventOccurrence, event)) => event,
                other => panic!("unexpected generation-substitution result: {other:?}"),
            };
        assert_eq!(
            substituted.exact.slot.generation,
            event.exact.slot.generation + 1
        );
        assert_eq!(
            owner.slots[event.exact.slot.index as usize].native_reader_pins,
            0
        );

        owner.slots[event.exact.slot.index as usize].native_reader_pins = u32::MAX;
        let event = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
            Err((Gfx942CompletionErrorV1::SignalPinCountExhausted, event)) => event,
            other => panic!("unexpected reader-pin overflow result: {other:?}"),
        };
        assert!(owner.dependency_ledger.readers.is_empty());
        owner.slots[event.exact.slot.index as usize].native_reader_pins = 0;
        owner.release_compute_event(event).unwrap();

        let bound = owner.bind_batch([template()]).unwrap();
        let (_, retention) = bound.into_parts();
        let slot = retention.slots[0].index as usize;
        owner.slots[slot].event_pins = u32::MAX;
        assert!(matches!(
            owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0),
            Err(Gfx942CompletionErrorV1::SignalPinCountExhausted)
        ));
        assert!(owner.dependency_ledger.events.is_empty());
    }

    #[test]
    fn checked_identity_exhaustion_precedes_pin_or_ledger_mutation() {
        let mut owner = owner();
        let bound = owner.bind_batch([template()]).unwrap();
        let (_, retention) = bound.into_parts();
        owner.dependency_ledger.next_event_id = u64::MAX;
        assert!(matches!(
            owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0),
            Err(Gfx942CompletionErrorV1::EventIdentityExhausted)
        ));
        assert_eq!(owner.slots[retention.slots[0].index as usize].event_pins, 0);
        assert!(owner.dependency_ledger.events.is_empty());
        owner.cancel_bound_retaining(retention).unwrap();

        owner.dependency_ledger.next_event_id = 1;
        let (_batch, event) = published(&mut owner);
        owner.dependency_ledger.next_reader_lease_id = u64::MAX;
        let event = match owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
            Err((Gfx942CompletionErrorV1::DependencyReaderIdentityExhausted, event)) => event,
            other => panic!("unexpected reader-identity exhaustion result: {other:?}"),
        };
        assert_eq!(
            owner.slots[event.exact.slot.index as usize].native_reader_pins,
            0
        );
        assert!(owner.dependency_ledger.readers.is_empty());
        owner.release_compute_event(event).unwrap();
    }

    #[test]
    fn bounded_ledgers_reject_capacity_before_pin_mutation() {
        let mut event_owner = owner();
        let bound = event_owner.bind_batch([template()]).unwrap();
        let (_, retention) = bound.into_parts();
        let exact = exact_occurrence(SESSION, SOURCE_EPOCH, &retention, 0, None).unwrap();
        for event_id in 1..=GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1 as u64 {
            assert!(
                event_owner
                    .dependency_ledger
                    .events
                    .insert(event_id, exact)
                    .is_none()
            );
        }
        assert!(matches!(
            event_owner.record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0),
            Err(Gfx942CompletionErrorV1::EventCapacityExhausted)
        ));
        assert_eq!(event_owner.slots[exact.slot.index as usize].event_pins, 0);

        let mut reader_owner = owner();
        let (_batch, event) = published(&mut reader_owner);
        for dependent_acceptance_epoch in 1..=GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1 as u64 {
            let key = DependencyReaderUseKeyV1 {
                event_id: event.event_id + dependent_acceptance_epoch,
                dependent_acceptance_epoch,
            };
            assert!(
                reader_owner
                    .dependency_ledger
                    .readers
                    .insert(
                        key,
                        ActiveDependencyReaderV1 {
                            lease_id: dependent_acceptance_epoch,
                            source: event.exact,
                        },
                    )
                    .is_none()
            );
        }
        let event =
            match reader_owner.retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH) {
                Err((Gfx942CompletionErrorV1::DependencyReaderCapacityExhausted, event)) => event,
                other => panic!("unexpected reader-capacity result: {other:?}"),
            };
        assert_eq!(
            reader_owner.slots[event.exact.slot.index as usize].native_reader_pins,
            0
        );
        reader_owner.release_compute_event(event).unwrap();
    }

    #[test]
    fn dependency_reader_release_is_exactly_once_and_addressless() {
        let mut owner = owner();
        let (batch, event) = published(&mut owner);
        let (event, lease) = owner
            .retain_compute_dependency_reader(event, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        let duplicate = duplicate_reader(&lease);
        let mut backend = CompletedBackend { reset_calls: 0 };
        let completed = complete(&mut owner, batch, &mut backend);
        owner.release_compute_event(event).unwrap();
        owner.release_compute_dependency_reader(lease).unwrap();
        let duplicate = match owner.release_compute_dependency_reader(duplicate) {
            Err((Gfx942CompletionErrorV1::StaleDependencyReader, lease)) => lease,
            other => panic!("unexpected duplicate release result: {other:?}"),
        };
        assert_eq!(
            owner.slots[duplicate.source.slot.index as usize].native_reader_pins,
            0
        );
        owner.recycle_retaining(completed, &mut backend).unwrap();
        assert_eq!(backend.reset_calls, 1);
        assert!(!format!("{duplicate:?}").contains("0x"));
    }

    #[test]
    fn parent_bridge_projects_exact_identity_and_signal_without_observation() {
        let mut owner = owner();
        let (batch, event) = published(&mut owner);
        let source = owner.dependency_source_identity_v1(&event).unwrap();
        assert_eq!(source.session_occurrence, SESSION);
        assert_eq!(source.acceptance_epoch, SOURCE_EPOCH);
        assert_eq!(source.queue, queue());
        assert_eq!(source.packet_id, Some(101));

        let (event, lease) = owner
            .retain_dependency_reader_v1(event, SESSION, DEPENDENT_EPOCH)
            .unwrap();
        let signal = owner
            .native_dependency_signal_observation_v1(&lease)
            .unwrap();
        assert_eq!(signal.raw(), AMD_SIGNAL_ALIGNMENT_V1 as u64);

        let mut backend = CompletedBackend { reset_calls: 0 };
        let completed = complete(&mut owner, batch, &mut backend);
        assert_eq!(backend.reset_calls, 0);
        owner.release_dependency_event_v1(event).unwrap();
        owner.release_dependency_reader_v1(lease).unwrap();
        owner.recycle_retaining(completed, &mut backend).unwrap();
        assert_eq!(backend.reset_calls, 1);
    }

    #[test]
    fn target_identity_bridge_binds_only_after_publication() {
        let mut owner = owner();
        let bound = owner.bind_batch([template()]).unwrap();
        let (_, retention) = bound.into_parts();
        let prepared = owner
            .bound_dependency_target_identity_v1(SESSION, DEPENDENT_EPOCH, &retention)
            .unwrap();
        assert_eq!(prepared.session_occurrence, SESSION);
        assert_eq!(prepared.acceptance_epoch, DEPENDENT_EPOCH);
        assert_eq!(prepared.packet_id, None);
        let batch = owner.mark_published(retention, 401).unwrap();
        let published = owner
            .published_dependency_target_identity_v1(SESSION, DEPENDENT_EPOCH, &batch)
            .unwrap();
        assert_eq!(published.packet_id, Some(401));
        assert_eq!(published.queue, prepared.queue);
        assert_eq!(published.signal_mapping, prepared.signal_mapping);
        assert_eq!(published.slot_index, prepared.slot_index);
        assert_eq!(published.slot_generation, prepared.slot_generation);
        assert_eq!(published.dispatch_generation, prepared.dispatch_generation);
    }

    #[test]
    fn dropping_tokens_is_inert_and_never_hides_a_reset() {
        let mut owner = owner();
        let (batch, event) = published(&mut owner);
        let slot = event.exact.slot.index as usize;
        drop(event);
        assert_eq!(owner.slots[slot].event_pins, 1);
        let mut backend = CompletedBackend { reset_calls: 0 };
        let completed = complete(&mut owner, batch, &mut backend);
        drop(completed);
        assert_eq!(owner.slots[slot].event_pins, 1);
        assert_eq!(backend.reset_calls, 0);
        assert!(matches!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        ));
    }

    #[test]
    fn event_custody_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_SHA256_V1);
    }
}
