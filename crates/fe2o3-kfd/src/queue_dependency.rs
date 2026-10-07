//! Crate-private acceptance and native-publication custody for cross-queue dependencies.
//!
//! It authenticates one exact target use, preserves source events and reader
//! leases across native outcomes, binds successful publication to the exact
//! final target packet, and releases source pins only after exact dependent
//! completion. The public lane facade owns the sole production session owner.

#![allow(dead_code)]

use core::fmt;
use std::collections::{HashMap, HashSet};

use super::completion::{
    CompletionBatchRetentionV1, CompletionSignalArenaOwnerV1,
    ComputeDependencyOccurrenceIdentityV1, ComputeDependencyTargetPlanErrorV1,
    Gfx942CompletedBatchV1, Gfx942CompletionBatchV1, Gfx942CompletionErrorV1,
    Gfx942CompletionPollWithProgressV1, Gfx942ComputeDependencyReaderLeaseV1,
    Gfx942ComputeEventOccurrenceV1, NativeCompletionSignalBackendV1,
    PreparedComputeDependencyTargetV1,
};
use super::submit::{
    NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1, NativeAqlSubmissionOwnerV1,
    NativeDependencyDispatchSubmissionFailureV1,
};
use fe2o3_aql::{
    AQL_MAX_DEPENDENCY_SIGNALS_V1, AqlDependencyDispatchPlanErrorV1,
    AqlDependencyDispatchPublicationBoundaryV1, AqlDependencyDispatchPublicationV1,
    AqlDependencySignalObservationV1, AqlPreparedDependencyDispatchV1,
    AqlTerminalDependencyDispatchCustodyV1,
};

/// Frozen storage bound for exact active dependency targets in one live session.
pub const MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1: usize = 128;

/// Canonical claim boundary for the crate-private R43 publisher foundation.
pub const GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-dependency-publisher-foundation-r48-v1\n",
    "scope=session-owned-acceptance-source-occurrence-native-b37-publication-exact-dependent-completion-and-linear-release\n",
    "capacity=1-through-256-distinct-source-occurrences,1-through-52-barriers,one-final-dispatch,128-preallocated-active-target-records-per-session\n",
    "acceptance=nonzero-session-occurrence,per-owner-unique-monotonic-epochs,burned-without-rewind\n",
    "concurrency=one-production-owner-per-live-queue-session,multiple-exact-active-targets-keyed-by-unique-epoch,each-target-and-its-one-source-arena-use-distinct-session-lanes\n",
    "target=target-owner-sealed-bound-batch,event,completion-signal,session-occurrence,acceptance-epoch,queue,mapping,batch,slot-generation,dispatch-generation,final-packet-id\n",
    "source=exactly-one-authenticated-source-arena-per-target,same-session,strictly-earlier-epoch,different-queue,published-occurrence,distinct-exact-signal\n",
    "source-event=one-private-prepublication-reservation-per-bound-source-packet,public-addressless-occurrence-exposed-only-after-exact-source-publication-and-packet-id-bind\n",
    "validation=preallocated-hash-ledger-with-one-pass-expected-linear-exact-signal-duplicate-preflight\n",
    "owner=sole-private-owner-is-structurally-retained-in-live-session,session-occurrence-derived-from-the-nonzero-monotonic-queue-session-identity,all-source-and-target-epochs-minted-there\n",
    "prepublication-failure=exact-acceptance,sealed-target-event-bound-resources,and-source-reader-custody-returned-without-native-publication\n",
    "native=one-complete-ring-reservation,one-write-index-claim,all-bodies-before-all-release-headers,one-final-doorbell\n",
    "rollback=ring-capacity-only-retryable,complete-immutable-exact-source-arena-route-and-target-custody-preflight,source-reader-leases-released-in-reverse,events-returned-in-original-order,before-target-resource-cancel\n",
    "completion=exact-target-owner-session-epoch-queue-mapping-batch-slot-dispatch-and-packet-occurrence-observed-before-one-atomic-source-reader-event-release\n",
    "recycle=source-and-target-signal-reset-blocked-while-event-or-reader-pinned,source-enabled-after-exact-dependent-completion-release,target-enabled-after-explicit-target-event-release\n",
    "terminal=preclaim-native-invariant-or-first-claim-attempt-and-later-poisons,opaque-custody,native-callback-panic-is-typed-callback-panic,no-retry-conversion\n",
    "observation=signal-address-identity-projection-only,no-completion-load-or-host-prepoll\n",
    "facade=public-move-only-addressless-lane-event-source-batch-target-poll-and-release-custody,successful-target-publication-returns-one-stable-boxed-dispatch-and-independent-published-target-event,no-signal-address-packet-id-slot-generation-or-reader-ticket-authority\n",
    "proof=executable-host-state-native-callback-fault-hostile-identity-and-source-shape-tests-only\n",
    "excluded=native-ordering-or-completion-truth-refinement,hardware,performance,parity,machine-checked-refinement\n",
);

/// SHA-256 of [`GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_V1`].
pub const GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_SHA256_V1: &str =
    "f988416cc136b8c09f3716af33a50207a929b3e53459e0a6de04abdb930008f7";

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ComputeDependencyTargetUseErrorV1 {
    InvalidSessionOccurrence,
    AcceptanceEpochExhausted,
    ActiveTargetUse,
    ActiveTargetCapacity,
    Poisoned,
    EmptyDependencyRoster,
    TooManyDependencies,
    InvalidTargetIdentity,
    CrossSessionDependency,
    SameQueueDependency,
    SelfDependency,
    DependencyCycle,
    DuplicateDependency,
    SourceOwnerRosterMismatch,
    Allocation,
    Plan(AqlDependencyDispatchPlanErrorV1),
    PublishedTargetMismatch,
    Completion(Gfx942CompletionErrorV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComputeDependencyTargetUsePhaseV1 {
    Prepared,
    NativePublished,
    Published,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveComputeDependencyTargetUseV1 {
    target: ComputeDependencyOccurrenceIdentityV1,
    dependency_count: u16,
    phase: ComputeDependencyTargetUsePhaseV1,
}

/// Per-owner issuer and exact active-target ledger.
///
/// Production construction occurs exactly once inside the live queue session.
/// Unit tests may construct isolated owners to exercise the foundation.
pub(super) struct ComputeDependencySessionOwnerV1 {
    session_occurrence: u64,
    next_acceptance_epoch: Option<u64>,
    active: HashMap<u64, ActiveComputeDependencyTargetUseV1>,
    poisoned: bool,
}

impl ComputeDependencySessionOwnerV1 {}

fn rollback_preflight_failure_v1(
    error: ComputeDependencyTargetUseErrorV1,
    prepared: PreparedComputeDependencyTargetUseV1,
) -> ComputeDependencyRollbackFailureV1 {
    ComputeDependencyRollbackFailureV1 {
        error,
        target: prepared.key.target,
        target_completion: prepared.target_completion,
        target_event: Some(prepared.target_event),
        released_events: Vec::new(),
        retained_readers: prepared.readers,
        plan: prepared.plan,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ComputeDependencyAcceptanceV1 {
    session_occurrence: u64,
    epoch: u64,
}

impl ComputeDependencyAcceptanceV1 {
    pub(super) const fn session_occurrence(&self) -> u64 {
        self.session_occurrence
    }

    pub(super) const fn epoch(&self) -> u64 {
        self.epoch
    }
}

pub(super) struct RetainedComputeDependencyReaderV1 {
    event: Gfx942ComputeEventOccurrenceV1,
    lease: Gfx942ComputeDependencyReaderLeaseV1,
    source: ComputeDependencyOccurrenceIdentityV1,
    signal: AqlDependencySignalObservationV1,
}

impl fmt::Debug for RetainedComputeDependencyReaderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedComputeDependencyReaderV1")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(super) enum ComputeDependencyReaderPreparationFailureV1 {
    Rejected {
        error: Gfx942CompletionErrorV1,
        event: Gfx942ComputeEventOccurrenceV1,
    },
    Terminal {
        error: Gfx942CompletionErrorV1,
        event: Gfx942ComputeEventOccurrenceV1,
        lease: Gfx942ComputeDependencyReaderLeaseV1,
    },
}

#[allow(clippy::result_large_err)]
pub(super) fn retain_dependency_reader_for_target_v1(
    source_owner: &mut CompletionSignalArenaOwnerV1,
    event: Gfx942ComputeEventOccurrenceV1,
    acceptance: &ComputeDependencyAcceptanceV1,
) -> Result<RetainedComputeDependencyReaderV1, ComputeDependencyReaderPreparationFailureV1> {
    let source = match source_owner.dependency_source_identity_v1(&event) {
        Ok(source) => source,
        Err(error) => {
            return Err(ComputeDependencyReaderPreparationFailureV1::Rejected { error, event });
        }
    };
    let (event, lease) = match source_owner.retain_dependency_reader_v1(
        event,
        acceptance.session_occurrence,
        acceptance.epoch,
    ) {
        Ok(retained) => retained,
        Err((error, event)) => {
            return Err(ComputeDependencyReaderPreparationFailureV1::Rejected { error, event });
        }
    };
    let signal = match source_owner.native_dependency_signal_observation_v1(&lease) {
        Ok(signal) => signal,
        Err(error) => {
            return Err(ComputeDependencyReaderPreparationFailureV1::Terminal {
                error,
                event,
                lease,
            });
        }
    };
    Ok(RetainedComputeDependencyReaderV1 {
        event,
        lease,
        source,
        signal,
    })
}

#[allow(clippy::result_large_err)]
pub(super) fn retain_dependency_readers_for_target_v1(
    source_owner: &mut CompletionSignalArenaOwnerV1,
    events: Vec<Gfx942ComputeEventOccurrenceV1>,
    acceptance: &ComputeDependencyAcceptanceV1,
) -> Result<Vec<RetainedComputeDependencyReaderV1>, ComputeDependencyReaderBatchFailureV1> {
    let mut readers = Vec::new();
    if readers.try_reserve_exact(events.len()).is_err() {
        return Err(ComputeDependencyReaderBatchFailureV1::Rejected {
            error: Gfx942CompletionErrorV1::DependencyLedgerAllocation,
            events,
        });
    }
    let retained = match source_owner.retain_dependency_reader_batch_v1(
        events,
        acceptance.session_occurrence,
        acceptance.epoch,
    ) {
        Ok(retained) => retained,
        Err((error, events)) => {
            return Err(ComputeDependencyReaderBatchFailureV1::Rejected { error, events });
        }
    };
    for (event, lease) in retained {
        let source = match source_owner.dependency_source_identity_v1(&event) {
            Ok(source) => source,
            Err(error) => {
                return Err(ComputeDependencyReaderBatchFailureV1::Terminal(Box::new(
                    TerminalComputeDependencyReaderBatchV1 {
                        error,
                        retained: readers,
                        event,
                        lease,
                    },
                )));
            }
        };
        let signal = match source_owner.native_dependency_signal_observation_v1(&lease) {
            Ok(signal) => signal,
            Err(error) => {
                return Err(ComputeDependencyReaderBatchFailureV1::Terminal(Box::new(
                    TerminalComputeDependencyReaderBatchV1 {
                        error,
                        retained: readers,
                        event,
                        lease,
                    },
                )));
            }
        };
        readers.push(RetainedComputeDependencyReaderV1 {
            event,
            lease,
            source,
            signal,
        });
    }
    Ok(readers)
}

pub(super) enum ComputeDependencyReaderBatchFailureV1 {
    Rejected {
        error: Gfx942CompletionErrorV1,
        events: Vec<Gfx942ComputeEventOccurrenceV1>,
    },
    Terminal(Box<TerminalComputeDependencyReaderBatchV1>),
}

pub(super) struct TerminalComputeDependencyReaderBatchV1 {
    error: Gfx942CompletionErrorV1,
    retained: Vec<RetainedComputeDependencyReaderV1>,
    event: Gfx942ComputeEventOccurrenceV1,
    lease: Gfx942ComputeDependencyReaderLeaseV1,
}

#[allow(clippy::result_large_err)]
pub(super) fn rollback_dependency_readers_before_publication_v1(
    source_owner: &mut CompletionSignalArenaOwnerV1,
    readers: Vec<RetainedComputeDependencyReaderV1>,
) -> Result<
    Vec<Gfx942ComputeEventOccurrenceV1>,
    (
        Gfx942CompletionErrorV1,
        Vec<RetainedComputeDependencyReaderV1>,
    ),
> {
    let mut retained = Vec::new();
    let mut metadata = Vec::new();
    if retained.try_reserve_exact(readers.len()).is_err()
        || metadata.try_reserve_exact(readers.len()).is_err()
    {
        return Err((Gfx942CompletionErrorV1::DependencyLedgerAllocation, readers));
    }
    for reader in &readers {
        if source_owner.dependency_source_identity_v1(&reader.event) != Ok(reader.source)
            || source_owner.native_dependency_signal_observation_v1(&reader.lease)
                != Ok(reader.signal)
        {
            return Err((Gfx942CompletionErrorV1::StaleDependencyReader, readers));
        }
        metadata.push((reader.source, reader.signal));
    }
    retained.extend(
        readers
            .into_iter()
            .map(|reader| (reader.event, reader.lease)),
    );
    match source_owner.release_dependency_reader_batch_v1(retained) {
        Ok(events) => Ok(events),
        Err((error, retained)) => {
            let readers = retained
                .into_iter()
                .zip(metadata)
                .map(
                    |((event, lease), (source, signal))| RetainedComputeDependencyReaderV1 {
                        event,
                        lease,
                        source,
                        signal,
                    },
                )
                .collect();
            Err((error, readers))
        }
    }
}

#[derive(Debug)]
pub(super) struct ComputeDependencyBeginFailureV1 {
    pub(super) error: ComputeDependencyTargetUseErrorV1,
    acceptance: ComputeDependencyAcceptanceV1,
    target: PreparedComputeDependencyTargetV1,
    readers: Vec<RetainedComputeDependencyReaderV1>,
}

impl ComputeDependencyBeginFailureV1 {
    pub(super) fn into_parts(
        self,
    ) -> (
        ComputeDependencyTargetUseErrorV1,
        ComputeDependencyAcceptanceV1,
        PreparedComputeDependencyTargetV1,
        Vec<RetainedComputeDependencyReaderV1>,
    ) {
        (self.error, self.acceptance, self.target, self.readers)
    }
}

pub(super) struct PreparedComputeDependencyTargetUseV1 {
    key: ActiveComputeDependencyTargetUseV1,
    target_completion: CompletionBatchRetentionV1<1>,
    target_event: Gfx942ComputeEventOccurrenceV1,
    readers: Vec<RetainedComputeDependencyReaderV1>,
    plan: AqlPreparedDependencyDispatchV1,
}

pub(super) struct RetryableComputeDependencyTargetUseV1 {
    pub(super) error: NativeAqlSubmissionErrorV1,
    prepared: PreparedComputeDependencyTargetUseV1,
}

#[derive(Debug)]
pub(super) struct NativePublishedComputeDependencyTargetUseV1 {
    key: ActiveComputeDependencyTargetUseV1,
    target_completion: CompletionBatchRetentionV1<1>,
    target_event: Gfx942ComputeEventOccurrenceV1,
    readers: Vec<RetainedComputeDependencyReaderV1>,
    publication: AqlDependencyDispatchPublicationV1,
}

pub(super) struct PublishedComputeDependencyTargetUseV1 {
    key: ActiveComputeDependencyTargetUseV1,
    target_batch: Gfx942CompletionBatchV1<1>,
    readers: Vec<RetainedComputeDependencyReaderV1>,
    publication: AqlDependencyDispatchPublicationV1,
}

#[derive(Debug)]
pub(super) struct PublishedComputeDependencyTargetBundleV1 {
    published: PublishedComputeDependencyTargetUseV1,
    target_event: Gfx942ComputeEventOccurrenceV1,
}

impl PublishedComputeDependencyTargetBundleV1 {
    pub(super) fn into_parts(
        self,
    ) -> (
        PublishedComputeDependencyTargetUseV1,
        Gfx942ComputeEventOccurrenceV1,
    ) {
        (self.published, self.target_event)
    }
}

pub(super) enum ComputeDependencyTargetPollV1 {
    Pending(PublishedComputeDependencyTargetUseV1),
    Ready(CompletedComputeDependencyTargetUseV1),
}

pub(super) struct CompletedComputeDependencyTargetUseV1 {
    key: ActiveComputeDependencyTargetUseV1,
    target_completion: Gfx942CompletedBatchV1<1>,
    readers: Vec<RetainedComputeDependencyReaderV1>,
    publication: AqlDependencyDispatchPublicationV1,
}

impl CompletedComputeDependencyTargetUseV1 {
    pub(super) fn completion_occurrence_v1(
        &self,
    ) -> Result<super::completion::CompletionBatchOccurrenceV1, Gfx942CompletionErrorV1> {
        self.target_completion.occurrence_v1()
    }

    pub(super) fn matches_source_owner(&self, owner: &CompletionSignalArenaOwnerV1) -> bool {
        !self.readers.is_empty()
            && self.readers.iter().all(|reader| {
                owner.matches_dependency_source_arena_v1(
                    reader.source.queue,
                    reader.source.signal_mapping,
                )
            })
    }
}

impl PublishedComputeDependencyTargetUseV1 {
    pub(super) fn completion_occurrence_v1(
        &self,
    ) -> Result<super::completion::CompletionBatchOccurrenceV1, Gfx942CompletionErrorV1> {
        self.target_batch.occurrence_v1()
    }
}

#[derive(Debug)]
pub(super) struct ReleasedComputeDependencyTargetUseV1 {
    pub(super) target_completion: Gfx942CompletedBatchV1<1>,
    pub(super) dependency_count: u16,
    publication: AqlDependencyDispatchPublicationV1,
}

impl fmt::Debug for PublishedComputeDependencyTargetUseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishedComputeDependencyTargetUseV1")
            .field("dependency_count", &self.readers.len())
            .field("publication", &self.publication)
            .finish_non_exhaustive()
    }
}

enum TerminalComputeDependencyPlanV1 {
    BeforeClaim(AqlPreparedDependencyDispatchV1),
    Ambiguous(AqlTerminalDependencyDispatchCustodyV1),
    Published(AqlDependencyDispatchPublicationV1),
    PostPublicationCompletion(Gfx942CompletionErrorV1),
}

enum TerminalComputeDependencyCompletionV1 {
    Bound {
        retention: CompletionBatchRetentionV1<1>,
        event: Option<Gfx942ComputeEventOccurrenceV1>,
    },
    Published {
        batch: Gfx942CompletionBatchV1<1>,
        event: Option<Gfx942ComputeEventOccurrenceV1>,
    },
    Completed {
        batch: Gfx942CompletedBatchV1<1>,
        event: Option<Gfx942ComputeEventOccurrenceV1>,
    },
}

pub(super) struct TerminalComputeDependencyTargetUseV1 {
    pub(super) error: NativeAqlSubmissionErrorV1,
    pub(super) boundary: Option<AqlDependencyDispatchPublicationBoundaryV1>,
    key: ActiveComputeDependencyTargetUseV1,
    target_completion: TerminalComputeDependencyCompletionV1,
    readers: Vec<RetainedComputeDependencyReaderV1>,
    plan: TerminalComputeDependencyPlanV1,
}

impl fmt::Debug for TerminalComputeDependencyTargetUseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalComputeDependencyTargetUseV1")
            .field("error", &self.error)
            .field("boundary", &self.boundary)
            .field("dependency_count", &self.readers.len())
            .finish_non_exhaustive()
    }
}

pub(super) enum ComputeDependencyPublicationFailureV1 {
    Retryable(Box<RetryableComputeDependencyTargetUseV1>),
    Terminal(Box<TerminalComputeDependencyTargetUseV1>),
}

impl fmt::Debug for ComputeDependencyPublicationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retryable(custody) => formatter
                .debug_tuple("Retryable")
                .field(&custody.error)
                .finish(),
            Self::Terminal(custody) => formatter.debug_tuple("Terminal").field(custody).finish(),
        }
    }
}

pub(super) struct CancelledComputeDependencyTargetUseV1 {
    target: ComputeDependencyOccurrenceIdentityV1,
    target_completion: CompletionBatchRetentionV1<1>,
    events: Vec<Gfx942ComputeEventOccurrenceV1>,
}

impl CancelledComputeDependencyTargetUseV1 {
    pub(super) fn into_parts(
        self,
    ) -> (
        CompletionBatchRetentionV1<1>,
        Vec<Gfx942ComputeEventOccurrenceV1>,
    ) {
        (self.target_completion, self.events)
    }
}

#[derive(Debug)]
pub(super) struct ComputeDependencyRollbackFailureV1 {
    error: ComputeDependencyTargetUseErrorV1,
    target: ComputeDependencyOccurrenceIdentityV1,
    target_completion: CompletionBatchRetentionV1<1>,
    target_event: Option<Gfx942ComputeEventOccurrenceV1>,
    released_events: Vec<Gfx942ComputeEventOccurrenceV1>,
    retained_readers: Vec<RetainedComputeDependencyReaderV1>,
    plan: AqlPreparedDependencyDispatchV1,
}

fn validate_target_use_v1(
    session_occurrence: u64,
    acceptance: &ComputeDependencyAcceptanceV1,
    target: ComputeDependencyOccurrenceIdentityV1,
    sources: &[ComputeDependencyOccurrenceIdentityV1],
) -> Result<(), ComputeDependencyTargetUseErrorV1> {
    if acceptance.session_occurrence != session_occurrence
        || acceptance.epoch == 0
        || target.session_occurrence != session_occurrence
        || target.acceptance_epoch != acceptance.epoch
        || target.batch_id == 0
        || target.slot_generation == 0
        || target.dispatch_generation == 0
        || target.packet_id.is_some()
    {
        return Err(ComputeDependencyTargetUseErrorV1::InvalidTargetIdentity);
    }
    if sources.is_empty() {
        return Err(ComputeDependencyTargetUseErrorV1::EmptyDependencyRoster);
    }
    if sources.len() > AQL_MAX_DEPENDENCY_SIGNALS_V1 {
        return Err(ComputeDependencyTargetUseErrorV1::TooManyDependencies);
    }
    let mut exact_signals = HashSet::new();
    exact_signals
        .try_reserve(sources.len())
        .map_err(|_| ComputeDependencyTargetUseErrorV1::Allocation)?;
    let mut source_arena = None;
    for source in sources {
        if source.session_occurrence != session_occurrence {
            return Err(ComputeDependencyTargetUseErrorV1::CrossSessionDependency);
        }
        if source.queue == target.queue {
            return Err(ComputeDependencyTargetUseErrorV1::SameQueueDependency);
        }
        if source.acceptance_epoch == acceptance.epoch {
            return Err(ComputeDependencyTargetUseErrorV1::SelfDependency);
        }
        if source.acceptance_epoch > acceptance.epoch {
            return Err(ComputeDependencyTargetUseErrorV1::DependencyCycle);
        }
        if source.acceptance_epoch == 0
            || source.batch_id == 0
            || source.slot_generation == 0
            || source.dispatch_generation == 0
            || source.packet_id.is_none()
        {
            return Err(ComputeDependencyTargetUseErrorV1::InvalidTargetIdentity);
        }
        let exact_arena = (source.queue, source.signal_mapping);
        if source_arena.is_some_and(|expected| expected != exact_arena) {
            return Err(ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch);
        }
        source_arena = Some(exact_arena);
        if !exact_signals.insert((
            source.signal_mapping,
            source.slot_index,
            source.slot_generation,
        )) {
            return Err(ComputeDependencyTargetUseErrorV1::DuplicateDependency);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "queue_dependency/tests.rs"]
mod tests;

#[path = "queue_dependency/session.rs"]
mod session;
