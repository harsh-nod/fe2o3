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
use std::collections::{HashMap, HashSet};

include!("release_pin_budget_body.rs");
include!("batch_event_release_body.rs");

#[cfg(test)]
#[path = "source_rollback_test_support.rs"]
mod source_rollback_test_support;

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
    "profile=fe2o3-mi300x-gfx942-compute-event-custody-r48-v1\n",
    "scope=completion-event-occurrence-and-native-dependency-reader-pin-ledger-only\n",
    "identity=session-occurrence,source-acceptance-epoch,queue-key,signal-mapping,slot-generation,dispatch-generation,packet-id;no-public-signal-address\n",
    "event=complete-batch-prepublication-reservation-is-private-unbound-and-pinned,public-occurrence-only-after-postpublication-exact-packet-bind,explicit-unused-or-target-event-release\n",
    "capacity=8192-live-event-occurrences-and-8192-live-native-readers-per-completion-arena,checked-before-growth-or-pin-mutation\n",
    "reader=one-move-only-lease-per-event-and-caller-supplied-dependent-acceptance-epoch,strictly-newer-epoch,same-session,duplicate-rejected\n",
    "contracted-premise=production-session-orchestrator-authenticates-session-wide-unique-dependent-acceptance-epochs-and-binds-each-to-one-target-publication\n",
    "recycle=completed-source-required,event-and-reader-pin-counts-both-zero-before-any-reset-or-generation-advance\n",
    "batch=record-bind-retain-reader-release-reader-and-release-reader-plus-event-complete-preflight-and-allocation-before-ledger-mutation\n",
    "batch-validation=preallocated-hash-ledgers-with-one-pass-expected-linear-duplicate-and-slot-pin-preflight\n",
    "failure=allocation-overflow-stale-duplicate-cross-session-cycle-self-and-prepublication-rejected-before-ledger-mutation;complete-consumed-roster-returned\n",
    "drop=inert;abandoned-token-retains-ledger-pin-and-blocks-cancel-recycle-and-arena-release\n",
    "proof=host-state-machine-and-hostile-unit-tests-only\n",
    "excluded=dependency-packet-construction-or-publication,native-completion-truth,native-hardware,panic-interception,machine-checked-refinement\n",
);

/// SHA-256 of [`GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_V1`].
pub const GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_SHA256_V1: &str =
    "3b235c35d117c198fcb21f65197431a47de78ed5081b95b459bbde61c6410e9a";

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
        completion_dependency_ledger_new_body!(completion_rust_expr)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.events.is_empty() && self.readers.is_empty()
    }
}

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
impl CompletionSignalArenaOwnerV1 {
    pub(crate) fn dependency_ledger_counts_for_test(&self) -> (u64, usize, usize) {
        (
            self.dependency_ledger.next_event_id,
            self.dependency_ledger.events.len(),
            self.dependency_ledger.readers.len(),
        )
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

    pub(super) const fn arena_identity(&self) -> (QueueKeyV1, MemoryMappingKeyV1) {
        (self.exact.queue, self.exact.signal_mapping)
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

pub(super) type Gfx942ComputeDependencyReaderPairV1 = (
    Gfx942ComputeEventOccurrenceV1,
    Gfx942ComputeDependencyReaderLeaseV1,
);
pub(super) type Gfx942ComputeDependencyReaderBatchV1 = Vec<Gfx942ComputeDependencyReaderPairV1>;
pub(super) type Gfx942ComputeEventBatchFailureV1 =
    (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>);
pub(super) type Gfx942ComputeDependencyReaderBatchFailureV1 = (
    Gfx942CompletionErrorV1,
    Gfx942ComputeDependencyReaderBatchV1,
);

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
    #[cfg(test)]
    pub(crate) fn dependency_reader_count_for_test(&self) -> usize {
        self.dependency_ledger.readers.len()
    }

    /// Records one event for every packet in an exact bound batch.
    ///
    /// Capacity, identity, allocation, and pin-count checks complete before
    /// the first ledger mutation, so a rejection returns the bound batch
    /// unchanged to its owner.
    #[allow(
        clippy::question_mark,
        reason = "shared proof returns carry reservation observations"
    )]
    pub(super) fn record_unbound_compute_event_batch<const N: usize>(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1> {
        completion_record_event_batch_body!(
            completion_rust_expr,
            self,
            session_occurrence,
            source_acceptance_epoch,
            retention,
            N
        )
    }

    /// Records an event before its source batch is published.
    ///
    /// Recording pins the exact slot. The returned occurrence is unbound until
    /// `bind_compute_event_after_publication` authenticates the published
    /// packet ID. All allocation and overflow checks precede ledger mutation.
    #[allow(
        clippy::question_mark,
        reason = "shared proof returns carry reservation observations"
    )]
    pub(super) fn record_unbound_compute_event<const N: usize>(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<N>,
        batch_index: usize,
    ) -> Result<Gfx942ComputeEventOccurrenceV1, Gfx942CompletionErrorV1> {
        completion_record_single_event_body!(
            completion_rust_expr,
            self,
            session_occurrence,
            source_acceptance_epoch,
            retention,
            batch_index
        )
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

    /// Binds a complete event roster to one exact published batch atomically,
    /// without allocating after publication.
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_compute_event_batch_after_publication<const N: usize>(
        &mut self,
        mut events: Vec<Gfx942ComputeEventOccurrenceV1>,
        batch: &Gfx942CompletionBatchV1<N>,
    ) -> Result<
        Vec<Gfx942ComputeEventOccurrenceV1>,
        (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>),
    > {
        completion_bind_event_batch_body!(completion_rust_expr, self, events, batch, N)
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

    /// Retains one exact same-arena event roster for a dependent occurrence.
    /// Every check and allocation completes before reader or pin mutation.
    #[allow(clippy::result_large_err)]
    pub(super) fn retain_compute_dependency_reader_batch(
        &mut self,
        events: Vec<Gfx942ComputeEventOccurrenceV1>,
        session_occurrence: u64,
        dependent_acceptance_epoch: u64,
    ) -> Result<Gfx942ComputeDependencyReaderBatchV1, Gfx942ComputeEventBatchFailureV1> {
        let result = (|| {
            self.require_ready()?;
            if events.is_empty() {
                return Err(Gfx942CompletionErrorV1::InvalidAcceptanceEpoch);
            }
            let next_len = self
                .dependency_ledger
                .readers
                .len()
                .checked_add(events.len())
                .ok_or(Gfx942CompletionErrorV1::DependencyReaderCapacityExhausted)?;
            if next_len > GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1 {
                return Err(Gfx942CompletionErrorV1::DependencyReaderCapacityExhausted);
            }
            let count = u64::try_from(events.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyReaderIdentityExhausted)?;
            let next_lease_id = self
                .dependency_ledger
                .next_reader_lease_id
                .checked_add(count)
                .ok_or(Gfx942CompletionErrorV1::DependencyReaderIdentityExhausted)?;
            let mut exact_events = HashSet::new();
            exact_events
                .try_reserve(events.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            let mut slot_increments = HashMap::new();
            slot_increments
                .try_reserve(events.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            for event in &events {
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
                self.validate_active_event(event)?;
                self.validate_published_or_completed_occurrence(event.exact)?;
                if !exact_events.insert(event.exact) {
                    return Err(Gfx942CompletionErrorV1::DuplicateDependency);
                }
                let use_key = DependencyReaderUseKeyV1 {
                    event_id: event.event_id,
                    dependent_acceptance_epoch,
                };
                if self.dependency_ledger.readers.contains_key(&use_key) {
                    return Err(Gfx942CompletionErrorV1::DuplicateDependency);
                }
                let same_slot_count = slot_increments.entry(event.exact.slot).or_insert(0_u32);
                *same_slot_count = same_slot_count
                    .checked_add(1)
                    .ok_or(Gfx942CompletionErrorV1::SignalPinCountExhausted)?;
                self.slots[event.exact.slot.index as usize]
                    .native_reader_pins
                    .checked_add(*same_slot_count)
                    .ok_or(Gfx942CompletionErrorV1::SignalPinCountExhausted)?;
            }
            self.dependency_ledger
                .readers
                .try_reserve(events.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            let mut retained = Vec::new();
            retained
                .try_reserve_exact(events.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            Ok((next_lease_id, retained))
        })();
        let (next_lease_id, mut retained) = match result {
            Ok(prepared) => prepared,
            Err(error) => return Err((error, events)),
        };
        for (offset, event) in events.into_iter().enumerate() {
            let lease = Gfx942ComputeDependencyReaderLeaseV1 {
                lease_id: self.dependency_ledger.next_reader_lease_id + offset as u64,
                event_id: event.event_id,
                dependent_acceptance_epoch,
                source: event.exact,
            };
            let use_key = DependencyReaderUseKeyV1 {
                event_id: lease.event_id,
                dependent_acceptance_epoch,
            };
            let replaced = self.dependency_ledger.readers.insert(
                use_key,
                ActiveDependencyReaderV1 {
                    lease_id: lease.lease_id,
                    source: lease.source,
                },
            );
            debug_assert!(replaced.is_none());
            self.slots[event.exact.slot.index as usize].native_reader_pins += 1;
            retained.push((event, lease));
        }
        self.dependency_ledger.next_reader_lease_id = next_lease_id;
        Ok(retained)
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
        completion_release_event_body!(completion_rust_expr, self, event)
    }

    fn event_release_preflight(
        &self,
        event: &Gfx942ComputeEventOccurrenceV1,
    ) -> Result<u32, Gfx942CompletionErrorV1> {
        completion_event_release_preflight_body!(completion_rust_expr, self, event)
    }

    /// Releases an exact event roster atomically with respect to host-ledger
    /// validation. It is used only before proven-no-effect bound cancellation.
    #[allow(clippy::result_large_err)]
    pub(super) fn release_compute_event_batch(
        &mut self,
        events: Vec<Gfx942ComputeEventOccurrenceV1>,
    ) -> Result<usize, (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>)> {
        completion_release_event_batch_body!(completion_rust_expr, self, events)
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

    /// Releases a prepublication reader roster while preserving every event
    /// occurrence in its original order for a proven-no-effect retry.
    #[allow(clippy::result_large_err)]
    pub(super) fn release_compute_dependency_reader_batch(
        &mut self,
        retained: Gfx942ComputeDependencyReaderBatchV1,
    ) -> Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942ComputeDependencyReaderBatchFailureV1>
    {
        let result = (|| {
            self.require_ready()?;
            let mut event_ids = HashSet::new();
            event_ids
                .try_reserve(retained.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            let mut lease_ids = HashSet::new();
            lease_ids
                .try_reserve(retained.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            for (event, lease) in &retained {
                if !event_ids.insert(event.event_id) || !lease_ids.insert(lease.lease_id) {
                    return Err(Gfx942CompletionErrorV1::DuplicateDependency);
                }
                self.validate_active_event(event)?;
                self.validate_live_occurrence(event.exact)?;
                if event.event_id != lease.event_id || event.exact != lease.source {
                    return Err(Gfx942CompletionErrorV1::StaleDependencyReader);
                }
                let use_key = DependencyReaderUseKeyV1 {
                    event_id: lease.event_id,
                    dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
                };
                let expected = ActiveDependencyReaderV1 {
                    lease_id: lease.lease_id,
                    source: lease.source,
                };
                if self.dependency_ledger.readers.get(&use_key) != Some(&expected)
                    || self.slots[lease.source.slot.index as usize].native_reader_pins == 0
                {
                    return Err(Gfx942CompletionErrorV1::StaleDependencyReader);
                }
            }
            validate_release_pin_budgets(
                retained.iter().map(|(_, lease)| {
                    let index = lease.source.slot.index;
                    (index, self.slots[index as usize].native_reader_pins)
                }),
                Gfx942CompletionErrorV1::StaleDependencyReader,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            return Err((error, retained));
        }
        let mut events = Vec::new();
        if events.try_reserve_exact(retained.len()).is_err() {
            return Err((
                Gfx942CompletionErrorV1::DependencyLedgerAllocation,
                retained,
            ));
        }
        for (event, lease) in retained {
            let use_key = DependencyReaderUseKeyV1 {
                event_id: lease.event_id,
                dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
            };
            let removed = self.dependency_ledger.readers.remove(&use_key);
            debug_assert!(removed.is_some());
            self.slots[lease.source.slot.index as usize].native_reader_pins -= 1;
            events.push(event);
        }
        Ok(events)
    }

    /// Releases exact reader/event pairs only after authenticating the complete
    /// roster. This is the post-dependent-completion transition; it cannot
    /// expose a released prefix on a stale or substituted input.
    #[allow(clippy::result_large_err)]
    pub(super) fn release_compute_dependency_reader_event_batch(
        &mut self,
        retained: Gfx942ComputeDependencyReaderBatchV1,
    ) -> Result<usize, Gfx942ComputeDependencyReaderBatchFailureV1> {
        let result = (|| {
            self.require_ready()?;
            let mut event_ids = HashSet::new();
            event_ids
                .try_reserve(retained.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            let mut lease_ids = HashSet::new();
            lease_ids
                .try_reserve(retained.len())
                .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
            for (event, lease) in &retained {
                if !event_ids.insert(event.event_id) || !lease_ids.insert(lease.lease_id) {
                    return Err(Gfx942CompletionErrorV1::DuplicateDependency);
                }
                self.validate_active_event(event)?;
                self.validate_live_occurrence(event.exact)?;
                if event.event_id != lease.event_id || event.exact != lease.source {
                    return Err(Gfx942CompletionErrorV1::StaleDependencyReader);
                }
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
                let record = &self.slots[lease.source.slot.index as usize];
                if record.native_reader_pins == 0 || record.event_pins == 0 {
                    return Err(Gfx942CompletionErrorV1::StaleDependencyReader);
                }
            }
            validate_release_pin_budgets(
                retained.iter().map(|(_, lease)| {
                    let index = lease.source.slot.index;
                    let record = &self.slots[index as usize];
                    (index, record.event_pins.min(record.native_reader_pins))
                }),
                Gfx942CompletionErrorV1::StaleDependencyReader,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            return Err((error, retained));
        }
        let released = retained.len();
        for (event, lease) in retained {
            let use_key = DependencyReaderUseKeyV1 {
                event_id: lease.event_id,
                dependent_acceptance_epoch: lease.dependent_acceptance_epoch,
            };
            let removed_reader = self.dependency_ledger.readers.remove(&use_key);
            let removed_event = self.dependency_ledger.events.remove(&event.event_id);
            debug_assert!(removed_reader.is_some());
            debug_assert_eq!(removed_event, Some(event.exact));
            let record = &mut self.slots[lease.source.slot.index as usize];
            record.native_reader_pins -= 1;
            record.event_pins -= 1;
        }
        Ok(released)
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
        completion_validate_active_event_body!(completion_rust_expr, self, event)
    }

    fn validate_live_occurrence(
        &self,
        exact: ExactCompletionOccurrenceV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        completion_validate_live_occurrence_body!(completion_rust_expr, self, exact)
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

// Call only after authenticating the whole roster. Repeated indices observe the
// same immutable slot budget; scratch changes never alter owner or token custody.
fn validate_release_pin_budgets(
    budgets: impl ExactSizeIterator<Item = (u32, u32)>,
    insufficient: Gfx942CompletionErrorV1,
) -> Result<(), Gfx942CompletionErrorV1> {
    if budgets.len() <= 1 {
        return validate_single_release_pin_budget(budgets, insufficient);
    }
    let mut remaining = HashMap::new();
    remaining
        .try_reserve(budgets.len())
        .map_err(|_| Gfx942CompletionErrorV1::DependencyLedgerAllocation)?;
    validate_reserved_release_pin_budgets(budgets, remaining, insufficient)
}

fn validate_single_release_pin_budget(
    mut budgets: impl Iterator<Item = (u32, u32)>,
    insufficient: Gfx942CompletionErrorV1,
) -> Result<(), Gfx942CompletionErrorV1> {
    completion_single_release_pin_budget_body!(completion_rust_expr, budgets, insufficient)
}

fn validate_reserved_release_pin_budgets(
    mut budgets: impl Iterator<Item = (u32, u32)>,
    mut remaining: HashMap<u32, u32>,
    insufficient: Gfx942CompletionErrorV1,
) -> Result<(), Gfx942CompletionErrorV1> {
    completion_reserved_release_pin_budgets_body!(
        completion_rust_expr,
        budgets,
        remaining,
        insufficient
    )
}

fn validate_logical_identity(
    session_occurrence: u64,
    acceptance_epoch: u64,
) -> Result<(), Gfx942CompletionErrorV1> {
    completion_logical_identity_body!(completion_rust_expr, session_occurrence, acceptance_epoch)
}

fn exact_occurrence<const N: usize>(
    session_occurrence: u64,
    source_acceptance_epoch: u64,
    retention: &CompletionBatchRetentionV1<N>,
    batch_index: usize,
    packet_id: Option<u64>,
) -> Result<ExactCompletionOccurrenceV1, Gfx942CompletionErrorV1> {
    completion_exact_occurrence_body!(
        completion_rust_expr,
        session_occurrence,
        source_acceptance_epoch,
        retention,
        batch_index,
        packet_id
    )
}

fn packet_id_at<const N: usize>(
    retention: &CompletionBatchRetentionV1<N>,
    batch_index: usize,
) -> Result<u64, Gfx942CompletionErrorV1> {
    completion_packet_id_at_body!(completion_rust_expr, retention, batch_index, N)
}

#[cfg(test)]
#[path = "dependency_event/tests.rs"]
mod tests;
