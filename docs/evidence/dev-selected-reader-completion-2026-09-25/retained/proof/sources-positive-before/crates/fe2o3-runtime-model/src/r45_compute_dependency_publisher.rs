//! Executable finite R45 model for the crate-private R43 dependency publisher.
//!
//! The model keeps publication and rollback custody addressless. Session-wide
//! epoch uniqueness is an explicit caller premise: the constructor cannot
//! prevent another owner from claiming the same session occurrence. Signal,
//! packet, event, lease, arena, and currentness identities are mathematical
//! inputs rather than native observations.
//!
//! This model performs no I/O and establishes no public-facade, dependent
//! completion, production-Rust refinement, KFD/HSA/HIP, hardware, progress,
//! parity, or performance claim.

#![allow(clippy::result_large_err)]

use alloc::vec::Vec;

pub const R45_MAX_DEPENDENCIES_V1: usize = 256;
pub const R45_BARRIER_FAN_IN_V1: usize = 5;
pub const R45_MAX_BARRIERS_V1: usize = 52;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45EpochMintingPremiseV1 {
    pub exactly_one_owner_for_session: bool,
    pub all_source_epochs_minted_by_owner: bool,
}

impl R45EpochMintingPremiseV1 {
    pub const fn is_contracted_model_only(self) -> bool {
        self.exactly_one_owner_for_session && self.all_source_epochs_minted_by_owner
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45AcceptanceV1 {
    mint_id: u64,
    owner_occurrence: u64,
    session_occurrence: u64,
    epoch: u64,
}

impl R45AcceptanceV1 {
    pub const fn mint_id_model_only(&self) -> u64 {
        self.mint_id
    }

    pub const fn owner_occurrence_model_only(&self) -> u64 {
        self.owner_occurrence
    }

    pub const fn session_occurrence_model_only(&self) -> u64 {
        self.session_occurrence
    }

    pub const fn epoch_model_only(&self) -> u64 {
        self.epoch
    }

    pub fn substitute_epoch_model_only(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }

    pub fn substitute_owner_model_only(mut self, owner_occurrence: u64) -> Self {
        self.owner_occurrence = owner_occurrence;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45OccurrenceIdentityV1 {
    pub session_occurrence: u64,
    pub acceptance_epoch: u64,
    pub queue_occurrence: u64,
    pub signal_mapping: u64,
    pub batch_id: u64,
    pub slot: u16,
    pub slot_generation: u64,
    pub dispatch_generation: u64,
    pub packet_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45SignalIdentityV1 {
    pub signal_mapping: u64,
    pub slot: u16,
    pub slot_generation: u64,
}

impl R45SignalIdentityV1 {
    pub const fn from_occurrence_model_only(occurrence: R45OccurrenceIdentityV1) -> Self {
        Self {
            signal_mapping: occurrence.signal_mapping,
            slot: occurrence.slot,
            slot_generation: occurrence.slot_generation,
        }
    }
}

impl R45OccurrenceIdentityV1 {
    pub const fn valid_source_model_only(self) -> bool {
        self.session_occurrence != 0
            && self.acceptance_epoch != 0
            && self.queue_occurrence != 0
            && self.signal_mapping != 0
            && self.batch_id != 0
            && self.slot_generation != 0
            && self.dispatch_generation != 0
            && self.packet_id != 0
    }

    pub const fn valid_unpublished_target_model_only(self) -> bool {
        self.session_occurrence != 0
            && self.acceptance_epoch != 0
            && self.queue_occurrence != 0
            && self.signal_mapping != 0
            && self.batch_id != 0
            && self.slot_generation != 0
            && self.dispatch_generation != 0
            && self.packet_id == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45TargetBatchIdentityV1 {
    pub target: R45OccurrenceIdentityV1,
    pub retention_id: u64,
    pub completion_signal: R45SignalIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45TargetEventV1 {
    pub event_id: u64,
    pub target: R45OccurrenceIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45FinalDispatchV1 {
    pub dispatch_id: u64,
    pub completion_signal: R45SignalIdentityV1,
    pub target: R45OccurrenceIdentityV1,
}

/// Move-only sealed target custody. All components are checked before sealing.
#[derive(Debug, Eq, PartialEq)]
pub struct R45SealedTargetBundleV1 {
    target: R45OccurrenceIdentityV1,
    batch: R45TargetBatchIdentityV1,
    event: R45TargetEventV1,
    final_dispatch: R45FinalDispatchV1,
}

impl R45SealedTargetBundleV1 {
    pub fn seal_model_only(
        target: R45OccurrenceIdentityV1,
        batch: R45TargetBatchIdentityV1,
        event: R45TargetEventV1,
        final_dispatch: R45FinalDispatchV1,
    ) -> Result<Self, R45DependencyPublisherErrorV1> {
        let candidate = Self {
            target,
            batch,
            event,
            final_dispatch,
        };
        if candidate.is_exact_model_only() {
            Ok(candidate)
        } else {
            Err(R45DependencyPublisherErrorV1::TargetComponentSplit)
        }
    }

    pub const fn target_model_only(&self) -> R45OccurrenceIdentityV1 {
        self.target
    }

    pub fn is_exact_model_only(&self) -> bool {
        self.target.valid_unpublished_target_model_only()
            && self.batch.target == self.target
            && self.event.target == self.target
            && self.batch.retention_id != 0
            && self.event.event_id != 0
            && self.final_dispatch.dispatch_id != 0
            && self.batch.completion_signal
                == R45SignalIdentityV1::from_occurrence_model_only(self.target)
            && self.final_dispatch.completion_signal == self.batch.completion_signal
            && self.final_dispatch.target == self.target
    }

    pub fn substitute_event_model_only(mut self, event: R45TargetEventV1) -> Self {
        self.event = event;
        self
    }

    pub fn substitute_final_dispatch_model_only(
        mut self,
        final_dispatch: R45FinalDispatchV1,
    ) -> Self {
        self.final_dispatch = final_dispatch;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45SourceRecordV1 {
    pub source: R45OccurrenceIdentityV1,
    pub event_id: u64,
    pub signal_identity: R45SignalIdentityV1,
    pub dependent_epoch: u64,
}

impl R45SourceRecordV1 {
    pub fn is_exact_model_only(self) -> bool {
        self.source.valid_source_model_only()
            && self.event_id != 0
            && self.signal_identity == R45SignalIdentityV1::from_occurrence_model_only(self.source)
            && self.dependent_epoch != 0
    }
}

/// Move-only reader custody for one immutable source occurrence record.
#[derive(Debug, Eq, PartialEq)]
pub struct R45SourceReaderCustodyV1 {
    record: R45SourceRecordV1,
    lease_id: u64,
}

impl R45SourceReaderCustodyV1 {
    pub fn new_model_only(
        record: R45SourceRecordV1,
        lease_id: u64,
    ) -> Result<Self, R45DependencyPublisherErrorV1> {
        if !record.is_exact_model_only() || lease_id == 0 {
            return Err(R45DependencyPublisherErrorV1::InvalidSource);
        }
        Ok(Self { record, lease_id })
    }

    pub const fn record_model_only(&self) -> R45SourceRecordV1 {
        self.record
    }

    pub const fn lease_id_model_only(&self) -> u64 {
        self.lease_id
    }

    pub fn substitute_lease_model_only(mut self, lease_id: u64) -> Self {
        self.lease_id = lease_id;
        self
    }

    pub fn substitute_event_model_only(mut self, event_id: u64) -> Self {
        self.record.event_id = event_id;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R45BarrierPacketV1 {
    pub signals: Vec<R45SignalIdentityV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45PreparedDependencyPlanV1 {
    pub barriers: Vec<R45BarrierPacketV1>,
    pub final_dispatch: R45FinalDispatchV1,
}

impl R45PreparedDependencyPlanV1 {
    pub fn packet_count_model_only(&self) -> usize {
        self.barriers.len() + 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R45PublisherPhaseV1 {
    Idle,
    Prepared { epoch: u64 },
    Published { epoch: u64 },
    Poisoned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R45PublisherSnapshotV1 {
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub next_acceptance_epoch: Option<u64>,
    pub next_mint_id: Option<u64>,
    pub outstanding_acceptances: Vec<(u64, u64)>,
    pub phase: R45PublisherPhaseV1,
    pub completion_loads: u64,
    pub native_effects: Vec<R45NativeEffectV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R45DependencyPublisherErrorV1 {
    InvalidOwnerOccurrence,
    InvalidSessionOccurrence,
    MissingEpochMintingPremise,
    EpochExhausted,
    ActiveTargetUse,
    Poisoned,
    EmptyDependencies,
    TooManyDependencies,
    InvalidTarget,
    TargetComponentSplit,
    AcceptanceSubstitution,
    InvalidSource,
    CrossSession,
    SameQueue,
    SelfDependency,
    DependencyCycle,
    DuplicateSource,
    DuplicateSignal,
    DuplicateEvent,
    DuplicateLease,
    InvalidSourceArena,
    InvalidRetryCustody,
    TargetCurrentnessMismatch,
    MissingSourceOwner,
    DuplicateSourceOwner,
    StaleSourceLease,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45PreparationFailureV1 {
    pub error: R45DependencyPublisherErrorV1,
    pub acceptance: R45AcceptanceV1,
    pub target: R45SealedTargetBundleV1,
    pub readers: Vec<R45SourceReaderCustodyV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45PreparedTargetUseV1 {
    acceptance: R45AcceptanceV1,
    target: R45SealedTargetBundleV1,
    readers: Vec<R45SourceReaderCustodyV1>,
    plan: R45PreparedDependencyPlanV1,
}

impl R45PreparedTargetUseV1 {
    pub fn source_count_model_only(&self) -> usize {
        self.readers.len()
    }

    pub fn barrier_count_model_only(&self) -> usize {
        self.plan.barriers.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R45NativeFaultV1 {
    None,
    RingOccupied,
    PreClaimInvariant,
    ClaimAttempt,
    BarrierBody(usize),
    FinalDispatchBody,
    BarrierHeader(usize),
    FinalDispatchHeader,
    Doorbell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R45NativeEffectV1 {
    Reservation { packet_count: usize },
    ClaimAttempt { packet_count: usize },
    BarrierBody { barrier_index: usize },
    FinalDispatchBody,
    BarrierHeader { barrier_index: usize },
    FinalDispatchHeader,
    Doorbell,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45RetryableTargetUseV1 {
    prepared: R45PreparedTargetUseV1,
}

impl R45RetryableTargetUseV1 {
    pub fn source_count_model_only(&self) -> usize {
        self.prepared.readers.len()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45OpaqueTerminalCustodyV1 {
    pub fault: R45NativeFaultV1,
    pub claim_attempted: bool,
    pub acceptance: R45AcceptanceV1,
    pub target: R45SealedTargetBundleV1,
    pub readers: Vec<R45SourceReaderCustodyV1>,
    pub plan: R45PreparedDependencyPlanV1,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45PublishedTargetUseV1 {
    pub acceptance: R45AcceptanceV1,
    pub target: R45SealedTargetBundleV1,
    pub readers: Vec<R45SourceReaderCustodyV1>,
    pub plan: R45PreparedDependencyPlanV1,
}

#[derive(Debug, Eq, PartialEq)]
pub enum R45PublicationFailureV1 {
    Retryable(R45RetryableTargetUseV1),
    Terminal(R45OpaqueTerminalCustodyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45ArenaIdentityV1 {
    pub queue_occurrence: u64,
    pub signal_mapping: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R45SourceArenaSnapshotV1 {
    pub identity: R45ArenaIdentityV1,
    pub live_readers: Vec<R45LiveReaderRecordV1>,
    pub released_lease_ids: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R45LiveReaderRecordV1 {
    pub source: R45SourceRecordV1,
    pub lease_id: u64,
}

impl R45LiveReaderRecordV1 {
    pub const fn from_parts_model_only(source: R45SourceRecordV1, lease_id: u64) -> Self {
        Self { source, lease_id }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45SourceArenaV1 {
    identity: R45ArenaIdentityV1,
    live_readers: Vec<R45LiveReaderRecordV1>,
    released_lease_ids: Vec<u64>,
}

impl R45SourceArenaV1 {
    pub fn new_model_only(
        identity: R45ArenaIdentityV1,
        live_readers: Vec<R45LiveReaderRecordV1>,
    ) -> Result<Self, R45DependencyPublisherErrorV1> {
        for (index, reader) in live_readers.iter().enumerate() {
            if reader.lease_id == 0
                || !reader.source.is_exact_model_only()
                || reader.source.source.queue_occurrence != identity.queue_occurrence
                || reader.source.source.signal_mapping != identity.signal_mapping
                || live_readers[..index].iter().any(|prior| {
                    prior == reader
                        || prior.lease_id == reader.lease_id
                        || prior.source.event_id == reader.source.event_id
                })
            {
                return Err(R45DependencyPublisherErrorV1::InvalidSourceArena);
            }
        }
        Ok(Self {
            identity,
            live_readers,
            released_lease_ids: Vec::new(),
        })
    }

    pub fn snapshot_model_only(&self) -> R45SourceArenaSnapshotV1 {
        R45SourceArenaSnapshotV1 {
            identity: self.identity,
            live_readers: self.live_readers.clone(),
            released_lease_ids: self.released_lease_ids.clone(),
        }
    }

    fn matches_source_model_only(&self, source: R45OccurrenceIdentityV1) -> bool {
        self.identity.queue_occurrence == source.queue_occurrence
            && self.identity.signal_mapping == source.signal_mapping
    }

    fn validates_lease_model_only(&self, custody: &R45SourceReaderCustodyV1) -> bool {
        let expected = R45LiveReaderRecordV1 {
            source: custody.record,
            lease_id: custody.lease_id,
        };
        self.live_readers
            .iter()
            .filter(|record| **record == expected)
            .count()
            == 1
    }

    fn release_prevalidated_model_only(&mut self, custody: &R45SourceReaderCustodyV1) {
        let expected = R45LiveReaderRecordV1 {
            source: custody.record,
            lease_id: custody.lease_id,
        };
        let index = self
            .live_readers
            .iter()
            .position(|entry| *entry == expected)
            .expect("complete rollback preflight guarantees a live exact lease");
        self.live_readers.remove(index);
        self.released_lease_ids.push(custody.lease_id);
    }

    pub fn duplicate_live_reader_model_only(&mut self, index: usize) {
        if let Some(record) = self.live_readers.get(index).copied() {
            self.live_readers.push(record);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R45TargetArenaSnapshotV1 {
    pub current_target: R45OccurrenceIdentityV1,
    pub retention_id: u64,
    pub event_id: u64,
    pub completion_signal: R45SignalIdentityV1,
    pub target_live: bool,
    pub cancellations: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45TargetArenaV1 {
    current_target: R45OccurrenceIdentityV1,
    retention_id: u64,
    event_id: u64,
    completion_signal: R45SignalIdentityV1,
    target_live: bool,
    cancellations: u64,
}

impl R45TargetArenaV1 {
    pub fn from_bundle_model_only(bundle: &R45SealedTargetBundleV1) -> Self {
        Self {
            current_target: bundle.target,
            retention_id: bundle.batch.retention_id,
            event_id: bundle.event.event_id,
            completion_signal: bundle.batch.completion_signal,
            target_live: true,
            cancellations: 0,
        }
    }

    pub fn snapshot_model_only(&self) -> R45TargetArenaSnapshotV1 {
        R45TargetArenaSnapshotV1 {
            current_target: self.current_target,
            retention_id: self.retention_id,
            event_id: self.event_id,
            completion_signal: self.completion_signal,
            target_live: self.target_live,
            cancellations: self.cancellations,
        }
    }

    pub fn validates_bundle_model_only(&self, bundle: &R45SealedTargetBundleV1) -> bool {
        self.target_live
            && bundle.is_exact_model_only()
            && self.current_target == bundle.target
            && self.retention_id == bundle.batch.retention_id
            && self.event_id == bundle.event.event_id
            && self.completion_signal == bundle.batch.completion_signal
    }

    pub fn substitute_event_model_only(&mut self, event_id: u64) {
        self.event_id = event_id;
    }

    fn cancel_prevalidated_model_only(&mut self) {
        self.target_live = false;
        self.cancellations += 1;
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45CancelledTargetUseV1 {
    pub acceptance: R45AcceptanceV1,
    pub target: R45SealedTargetBundleV1,
    pub returned_source_events: Vec<u64>,
    pub released_sources: Vec<R45SourceRecordV1>,
    pub plan: R45PreparedDependencyPlanV1,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R45RollbackFailureV1 {
    pub error: R45DependencyPublisherErrorV1,
    pub retryable: R45RetryableTargetUseV1,
    pub released_source_events: Vec<u64>,
}

#[derive(Debug)]
pub struct R45ComputeDependencyPublisherV1 {
    owner_occurrence: u64,
    session_occurrence: u64,
    premise: R45EpochMintingPremiseV1,
    next_acceptance_epoch: Option<u64>,
    next_mint_id: Option<u64>,
    outstanding_acceptances: Vec<(u64, u64)>,
    phase: R45PublisherPhaseV1,
    completion_loads: u64,
    native_effects: Vec<R45NativeEffectV1>,
}

impl R45ComputeDependencyPublisherV1 {
    pub fn new_model_only(
        owner_occurrence: u64,
        session_occurrence: u64,
        premise: R45EpochMintingPremiseV1,
    ) -> Result<Self, R45DependencyPublisherErrorV1> {
        if owner_occurrence == 0 {
            return Err(R45DependencyPublisherErrorV1::InvalidOwnerOccurrence);
        }
        if session_occurrence == 0 {
            return Err(R45DependencyPublisherErrorV1::InvalidSessionOccurrence);
        }
        if !premise.is_contracted_model_only() {
            return Err(R45DependencyPublisherErrorV1::MissingEpochMintingPremise);
        }
        Ok(Self {
            owner_occurrence,
            session_occurrence,
            premise,
            next_acceptance_epoch: Some(1),
            next_mint_id: Some(1),
            outstanding_acceptances: Vec::new(),
            phase: R45PublisherPhaseV1::Idle,
            completion_loads: 0,
            native_effects: Vec::new(),
        })
    }

    pub fn snapshot_model_only(&self) -> R45PublisherSnapshotV1 {
        R45PublisherSnapshotV1 {
            owner_occurrence: self.owner_occurrence,
            session_occurrence: self.session_occurrence,
            next_acceptance_epoch: self.next_acceptance_epoch,
            next_mint_id: self.next_mint_id,
            outstanding_acceptances: self.outstanding_acceptances.clone(),
            phase: self.phase,
            completion_loads: self.completion_loads,
            native_effects: self.native_effects.clone(),
        }
    }

    pub fn reserve_acceptance_epoch_model_only(
        &mut self,
    ) -> Result<R45AcceptanceV1, R45DependencyPublisherErrorV1> {
        match self.phase {
            R45PublisherPhaseV1::Idle => {}
            R45PublisherPhaseV1::Poisoned => {
                return Err(R45DependencyPublisherErrorV1::Poisoned);
            }
            _ => return Err(R45DependencyPublisherErrorV1::ActiveTargetUse),
        }
        let Some(epoch) = self.next_acceptance_epoch else {
            self.phase = R45PublisherPhaseV1::Poisoned;
            return Err(R45DependencyPublisherErrorV1::EpochExhausted);
        };
        let Some(mint_id) = self.next_mint_id else {
            self.phase = R45PublisherPhaseV1::Poisoned;
            return Err(R45DependencyPublisherErrorV1::EpochExhausted);
        };
        self.next_acceptance_epoch = epoch.checked_add(1);
        self.next_mint_id = mint_id.checked_add(1);
        self.outstanding_acceptances.push((mint_id, epoch));
        Ok(R45AcceptanceV1 {
            mint_id,
            owner_occurrence: self.owner_occurrence,
            session_occurrence: self.session_occurrence,
            epoch,
        })
    }

    pub fn begin_target_use_model_only(
        &mut self,
        acceptance: R45AcceptanceV1,
        target: R45SealedTargetBundleV1,
        readers: Vec<R45SourceReaderCustodyV1>,
    ) -> Result<R45PreparedTargetUseV1, R45PreparationFailureV1> {
        let before = self.phase;
        let error = self.validate_preparation_model_only(&acceptance, &target, &readers);
        if let Err(error) = error {
            debug_assert_eq!(self.phase, before);
            return Err(R45PreparationFailureV1 {
                error,
                acceptance,
                target,
                readers,
            });
        }
        let mut barriers = Vec::with_capacity(readers.len().div_ceil(R45_BARRIER_FAN_IN_V1));
        for chunk in readers.chunks(R45_BARRIER_FAN_IN_V1) {
            barriers.push(R45BarrierPacketV1 {
                signals: chunk
                    .iter()
                    .map(|reader| reader.record.signal_identity)
                    .collect(),
            });
        }
        let final_dispatch = target.final_dispatch;
        let mint_index = self
            .outstanding_acceptances
            .iter()
            .position(|mint| *mint == (acceptance.mint_id, acceptance.epoch))
            .expect("preparation validation authenticated the exact mint");
        self.outstanding_acceptances.remove(mint_index);
        self.phase = R45PublisherPhaseV1::Prepared {
            epoch: acceptance.epoch,
        };
        Ok(R45PreparedTargetUseV1 {
            acceptance,
            target,
            readers,
            plan: R45PreparedDependencyPlanV1 {
                barriers,
                final_dispatch,
            },
        })
    }

    fn validate_preparation_model_only(
        &self,
        acceptance: &R45AcceptanceV1,
        target: &R45SealedTargetBundleV1,
        readers: &[R45SourceReaderCustodyV1],
    ) -> Result<(), R45DependencyPublisherErrorV1> {
        match self.phase {
            R45PublisherPhaseV1::Idle => {}
            R45PublisherPhaseV1::Poisoned => {
                return Err(R45DependencyPublisherErrorV1::Poisoned);
            }
            _ => return Err(R45DependencyPublisherErrorV1::ActiveTargetUse),
        }
        if !self.premise.is_contracted_model_only() {
            return Err(R45DependencyPublisherErrorV1::MissingEpochMintingPremise);
        }
        if acceptance.owner_occurrence != self.owner_occurrence
            || acceptance.session_occurrence != self.session_occurrence
            || acceptance.epoch == 0
            || !self
                .outstanding_acceptances
                .contains(&(acceptance.mint_id, acceptance.epoch))
            || target.target.acceptance_epoch != acceptance.epoch
            || target.target.session_occurrence != acceptance.session_occurrence
        {
            return Err(R45DependencyPublisherErrorV1::AcceptanceSubstitution);
        }
        if !target.is_exact_model_only() {
            return Err(R45DependencyPublisherErrorV1::TargetComponentSplit);
        }
        if readers.is_empty() {
            return Err(R45DependencyPublisherErrorV1::EmptyDependencies);
        }
        if readers.len() > R45_MAX_DEPENDENCIES_V1 {
            return Err(R45DependencyPublisherErrorV1::TooManyDependencies);
        }
        for (index, reader) in readers.iter().enumerate() {
            let source = reader.record;
            if reader.lease_id == 0
                || !source.is_exact_model_only()
                || source.dependent_epoch != acceptance.epoch
            {
                return Err(R45DependencyPublisherErrorV1::InvalidSource);
            }
            if source.source.session_occurrence != self.session_occurrence {
                return Err(R45DependencyPublisherErrorV1::CrossSession);
            }
            if source.source.queue_occurrence == target.target.queue_occurrence {
                return Err(R45DependencyPublisherErrorV1::SameQueue);
            }
            if source.source.acceptance_epoch == acceptance.epoch {
                return Err(R45DependencyPublisherErrorV1::SelfDependency);
            }
            if source.source.acceptance_epoch > acceptance.epoch {
                return Err(R45DependencyPublisherErrorV1::DependencyCycle);
            }
            for prior in &readers[..index] {
                if prior.record.source == source.source {
                    return Err(R45DependencyPublisherErrorV1::DuplicateSource);
                }
                if prior.record.signal_identity == source.signal_identity {
                    return Err(R45DependencyPublisherErrorV1::DuplicateSignal);
                }
                if prior.record.event_id == source.event_id {
                    return Err(R45DependencyPublisherErrorV1::DuplicateEvent);
                }
                if prior.lease_id == reader.lease_id {
                    return Err(R45DependencyPublisherErrorV1::DuplicateLease);
                }
            }
        }
        Ok(())
    }

    pub fn publish_native_model_only(
        &mut self,
        prepared: R45PreparedTargetUseV1,
        fault: R45NativeFaultV1,
    ) -> Result<R45PublishedTargetUseV1, R45PublicationFailureV1> {
        if self.phase
            != (R45PublisherPhaseV1::Prepared {
                epoch: prepared.acceptance.epoch,
            })
        {
            self.phase = R45PublisherPhaseV1::Poisoned;
            return Err(R45PublicationFailureV1::Terminal(
                R45OpaqueTerminalCustodyV1 {
                    fault: R45NativeFaultV1::PreClaimInvariant,
                    claim_attempted: false,
                    acceptance: prepared.acceptance,
                    target: prepared.target,
                    readers: prepared.readers,
                    plan: prepared.plan,
                },
            ));
        }
        if fault == R45NativeFaultV1::RingOccupied {
            return Err(R45PublicationFailureV1::Retryable(
                R45RetryableTargetUseV1 { prepared },
            ));
        }
        if fault == R45NativeFaultV1::PreClaimInvariant {
            self.phase = R45PublisherPhaseV1::Poisoned;
            return Err(R45PublicationFailureV1::Terminal(
                R45OpaqueTerminalCustodyV1 {
                    fault,
                    claim_attempted: false,
                    acceptance: prepared.acceptance,
                    target: prepared.target,
                    readers: prepared.readers,
                    plan: prepared.plan,
                },
            ));
        }

        let malformed_fault = match fault {
            R45NativeFaultV1::BarrierBody(index) | R45NativeFaultV1::BarrierHeader(index) => {
                index >= prepared.plan.barriers.len()
            }
            _ => false,
        };
        if malformed_fault {
            self.phase = R45PublisherPhaseV1::Poisoned;
            return Err(R45PublicationFailureV1::Terminal(
                R45OpaqueTerminalCustodyV1 {
                    fault,
                    claim_attempted: false,
                    acceptance: prepared.acceptance,
                    target: prepared.target,
                    readers: prepared.readers,
                    plan: prepared.plan,
                },
            ));
        }

        let packet_count = prepared.plan.packet_count_model_only();
        self.native_effects
            .push(R45NativeEffectV1::Reservation { packet_count });
        self.native_effects
            .push(R45NativeEffectV1::ClaimAttempt { packet_count });
        if fault == R45NativeFaultV1::ClaimAttempt {
            return Err(self.terminal_after_claim_model_only(prepared, fault));
        }
        for barrier_index in 0..prepared.plan.barriers.len() {
            self.native_effects
                .push(R45NativeEffectV1::BarrierBody { barrier_index });
            if fault == R45NativeFaultV1::BarrierBody(barrier_index) {
                return Err(self.terminal_after_claim_model_only(prepared, fault));
            }
        }
        self.native_effects
            .push(R45NativeEffectV1::FinalDispatchBody);
        if fault == R45NativeFaultV1::FinalDispatchBody {
            return Err(self.terminal_after_claim_model_only(prepared, fault));
        }
        for barrier_index in 0..prepared.plan.barriers.len() {
            self.native_effects
                .push(R45NativeEffectV1::BarrierHeader { barrier_index });
            if fault == R45NativeFaultV1::BarrierHeader(barrier_index) {
                return Err(self.terminal_after_claim_model_only(prepared, fault));
            }
        }
        self.native_effects
            .push(R45NativeEffectV1::FinalDispatchHeader);
        if fault == R45NativeFaultV1::FinalDispatchHeader {
            return Err(self.terminal_after_claim_model_only(prepared, fault));
        }
        self.native_effects.push(R45NativeEffectV1::Doorbell);
        if fault == R45NativeFaultV1::Doorbell {
            return Err(self.terminal_after_claim_model_only(prepared, fault));
        }
        self.phase = R45PublisherPhaseV1::Published {
            epoch: prepared.acceptance.epoch,
        };
        Ok(R45PublishedTargetUseV1 {
            acceptance: prepared.acceptance,
            target: prepared.target,
            readers: prepared.readers,
            plan: prepared.plan,
        })
    }

    fn terminal_after_claim_model_only(
        &mut self,
        prepared: R45PreparedTargetUseV1,
        fault: R45NativeFaultV1,
    ) -> R45PublicationFailureV1 {
        self.phase = R45PublisherPhaseV1::Poisoned;
        R45PublicationFailureV1::Terminal(R45OpaqueTerminalCustodyV1 {
            fault,
            claim_attempted: true,
            acceptance: prepared.acceptance,
            target: prepared.target,
            readers: prepared.readers,
            plan: prepared.plan,
        })
    }

    pub fn rollback_retryable_model_only(
        &mut self,
        retryable: R45RetryableTargetUseV1,
        source_owners: &mut [&mut R45SourceArenaV1],
        target_owner: &mut R45TargetArenaV1,
    ) -> Result<R45CancelledTargetUseV1, R45RollbackFailureV1> {
        let epoch = retryable.prepared.acceptance.epoch;
        if self.phase != (R45PublisherPhaseV1::Prepared { epoch }) {
            return Err(self.rollback_failure_model_only(
                R45DependencyPublisherErrorV1::InvalidRetryCustody,
                retryable,
            ));
        }
        if !target_owner.validates_bundle_model_only(&retryable.prepared.target) {
            return Err(self.rollback_failure_model_only(
                R45DependencyPublisherErrorV1::TargetCurrentnessMismatch,
                retryable,
            ));
        }

        let mut route = Vec::with_capacity(retryable.prepared.readers.len());
        for reader in &retryable.prepared.readers {
            let source = reader.record;
            let mut found = None;
            for (owner_index, owner) in source_owners.iter().enumerate() {
                if owner.matches_source_model_only(source.source) {
                    if found.is_some() {
                        return Err(self.rollback_failure_model_only(
                            R45DependencyPublisherErrorV1::DuplicateSourceOwner,
                            retryable,
                        ));
                    }
                    found = Some(owner_index);
                }
            }
            let Some(owner_index) = found else {
                return Err(self.rollback_failure_model_only(
                    R45DependencyPublisherErrorV1::MissingSourceOwner,
                    retryable,
                ));
            };
            if !source_owners[owner_index].validates_lease_model_only(reader) {
                return Err(self.rollback_failure_model_only(
                    R45DependencyPublisherErrorV1::StaleSourceLease,
                    retryable,
                ));
            }
            route.push(owner_index);
        }

        let mut returned_source_events = Vec::with_capacity(retryable.prepared.readers.len());
        for index in (0..retryable.prepared.readers.len()).rev() {
            source_owners[route[index]]
                .release_prevalidated_model_only(&retryable.prepared.readers[index]);
            returned_source_events.push(retryable.prepared.readers[index].record.event_id);
        }
        returned_source_events.reverse();
        let released_sources = retryable
            .prepared
            .readers
            .iter()
            .map(R45SourceReaderCustodyV1::record_model_only)
            .collect();
        target_owner.cancel_prevalidated_model_only();
        self.phase = R45PublisherPhaseV1::Idle;
        Ok(R45CancelledTargetUseV1 {
            acceptance: retryable.prepared.acceptance,
            target: retryable.prepared.target,
            returned_source_events,
            released_sources,
            plan: retryable.prepared.plan,
        })
    }

    fn rollback_failure_model_only(
        &mut self,
        error: R45DependencyPublisherErrorV1,
        retryable: R45RetryableTargetUseV1,
    ) -> R45RollbackFailureV1 {
        self.phase = R45PublisherPhaseV1::Poisoned;
        R45RollbackFailureV1 {
            error,
            retryable,
            released_source_events: Vec::new(),
        }
    }
}
