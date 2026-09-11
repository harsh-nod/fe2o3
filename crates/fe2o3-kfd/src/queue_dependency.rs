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

impl ComputeDependencySessionOwnerV1 {
    pub(super) fn new(session_occurrence: u64) -> Result<Self, ComputeDependencyTargetUseErrorV1> {
        if session_occurrence == 0 {
            return Err(ComputeDependencyTargetUseErrorV1::InvalidSessionOccurrence);
        }
        let mut active = HashMap::new();
        active
            .try_reserve(MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1)
            .map_err(|_| ComputeDependencyTargetUseErrorV1::Allocation)?;
        Ok(Self {
            session_occurrence,
            next_acceptance_epoch: Some(1),
            active,
            poisoned: false,
        })
    }

    pub(super) const fn session_occurrence(&self) -> u64 {
        self.session_occurrence
    }

    #[cfg(test)]
    pub(super) fn custody_snapshot_for_test(&self) -> (u64, Option<u64>, Vec<(u64, usize)>) {
        let mut active: Vec<_> = self
            .active
            .iter()
            .map(|(epoch, owner)| (*epoch, owner as *const _ as usize))
            .collect();
        active.sort_unstable();
        (self.session_occurrence, self.next_acceptance_epoch, active)
    }

    pub(super) fn ensure_idle(&self) -> Result<(), ComputeDependencyTargetUseErrorV1> {
        if self.poisoned {
            return Err(ComputeDependencyTargetUseErrorV1::Poisoned);
        }
        if !self.active.is_empty() {
            return Err(ComputeDependencyTargetUseErrorV1::ActiveTargetUse);
        }
        Ok(())
    }

    pub(super) fn poison(&mut self) {
        self.poisoned = true;
    }

    pub(super) fn ensure_target_capacity(&self) -> Result<(), ComputeDependencyTargetUseErrorV1> {
        if self.poisoned {
            return Err(ComputeDependencyTargetUseErrorV1::Poisoned);
        }
        if self.active.len() >= MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1 {
            return Err(ComputeDependencyTargetUseErrorV1::ActiveTargetCapacity);
        }
        Ok(())
    }

    /// Issues and burns one epoch. Cancellation never rewinds this counter.
    pub(super) fn reserve_acceptance_epoch(
        &mut self,
    ) -> Result<ComputeDependencyAcceptanceV1, ComputeDependencyTargetUseErrorV1> {
        if self.poisoned {
            return Err(ComputeDependencyTargetUseErrorV1::Poisoned);
        }
        let Some(epoch) = self.next_acceptance_epoch else {
            self.poisoned = true;
            return Err(ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted);
        };
        self.next_acceptance_epoch = epoch.checked_add(1);
        Ok(ComputeDependencyAcceptanceV1 {
            session_occurrence: self.session_occurrence,
            epoch,
        })
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn begin_target_use(
        &mut self,
        target_owner: &CompletionSignalArenaOwnerV1,
        acceptance: ComputeDependencyAcceptanceV1,
        target: PreparedComputeDependencyTargetV1,
        readers: Vec<RetainedComputeDependencyReaderV1>,
    ) -> Result<PreparedComputeDependencyTargetUseV1, ComputeDependencyBeginFailureV1> {
        let fail = |error, acceptance, target, readers| ComputeDependencyBeginFailureV1 {
            error,
            acceptance,
            target,
            readers,
        };
        if self.poisoned {
            return Err(fail(
                ComputeDependencyTargetUseErrorV1::Poisoned,
                acceptance,
                target,
                readers,
            ));
        }
        if self.active.contains_key(&acceptance.epoch) {
            return Err(fail(
                ComputeDependencyTargetUseErrorV1::ActiveTargetUse,
                acceptance,
                target,
                readers,
            ));
        }
        if let Err(error) = self.ensure_target_capacity() {
            return Err(fail(error, acceptance, target, readers));
        }
        if let Err(error) = target_owner.validate_dependency_target_v1(&target) {
            return Err(fail(
                ComputeDependencyTargetUseErrorV1::Completion(error),
                acceptance,
                target,
                readers,
            ));
        }
        let target_identity = target.identity();
        let mut sources = Vec::new();
        if sources.try_reserve_exact(readers.len()).is_err() {
            return Err(fail(
                ComputeDependencyTargetUseErrorV1::Allocation,
                acceptance,
                target,
                readers,
            ));
        }
        sources.extend(readers.iter().map(|reader| reader.source));
        if let Err(error) = validate_target_use_v1(
            self.session_occurrence,
            &acceptance,
            target_identity,
            &sources,
        ) {
            return Err(fail(error, acceptance, target, readers));
        }
        let mut signals = Vec::new();
        if signals.try_reserve_exact(readers.len()).is_err() {
            return Err(fail(
                ComputeDependencyTargetUseErrorV1::Allocation,
                acceptance,
                target,
                readers,
            ));
        }
        signals.extend(readers.iter().map(|reader| reader.signal));
        let planned = match target_owner.plan_dependency_target_v1(target, &signals) {
            Ok(planned) => planned,
            Err((error, target)) => {
                let error = match error {
                    ComputeDependencyTargetPlanErrorV1::Completion(error) => {
                        ComputeDependencyTargetUseErrorV1::Completion(error)
                    }
                    ComputeDependencyTargetPlanErrorV1::Plan(error) => {
                        ComputeDependencyTargetUseErrorV1::Plan(error)
                    }
                };
                return Err(fail(error, acceptance, target, readers));
            }
        };
        let (target, target_completion, target_event, plan) = planned.into_parts();
        let key = ActiveComputeDependencyTargetUseV1 {
            target,
            dependency_count: readers.len() as u16,
            phase: ComputeDependencyTargetUsePhaseV1::Prepared,
        };
        let replaced = self.active.insert(key.target.acceptance_epoch, key);
        debug_assert!(replaced.is_none());
        Ok(PreparedComputeDependencyTargetUseV1 {
            key,
            target_completion,
            target_event,
            readers,
            plan,
        })
    }

    pub(super) fn publish_native<B: NativeAqlSubmissionBackendV1>(
        &mut self,
        prepared: PreparedComputeDependencyTargetUseV1,
        submission: &mut NativeAqlSubmissionOwnerV1,
        backend: &mut B,
    ) -> Result<NativePublishedComputeDependencyTargetUseV1, ComputeDependencyPublicationFailureV1>
    {
        if self.poisoned
            || self.active.get(&prepared.key.target.acceptance_epoch) != Some(&prepared.key)
        {
            self.poisoned = true;
            return Err(ComputeDependencyPublicationFailureV1::Terminal(Box::new(
                TerminalComputeDependencyTargetUseV1 {
                    error: NativeAqlSubmissionErrorV1::Poisoned,
                    boundary: None,
                    key: prepared.key,
                    target_completion: TerminalComputeDependencyCompletionV1::Bound {
                        retention: prepared.target_completion,
                        event: Some(prepared.target_event),
                    },
                    readers: prepared.readers,
                    plan: TerminalComputeDependencyPlanV1::BeforeClaim(prepared.plan),
                },
            )));
        }
        let PreparedComputeDependencyTargetUseV1 {
            key,
            target_completion,
            target_event,
            readers,
            plan,
        } = prepared;
        match submission.submit_dependency_dispatch_classified(plan, backend) {
            Ok(publication) => {
                let Some(active) = self.active.get_mut(&key.target.acceptance_epoch) else {
                    unreachable!("active target was validated before native publication")
                };
                active.phase = ComputeDependencyTargetUsePhaseV1::NativePublished;
                Ok(NativePublishedComputeDependencyTargetUseV1 {
                    key: *active,
                    target_completion,
                    target_event,
                    readers,
                    publication,
                })
            }
            Err(NativeDependencyDispatchSubmissionFailureV1::RetryableBeforeSideEffect {
                error,
                prepared,
            }) => Err(ComputeDependencyPublicationFailureV1::Retryable(Box::new(
                RetryableComputeDependencyTargetUseV1 {
                    error,
                    prepared: PreparedComputeDependencyTargetUseV1 {
                        key,
                        target_completion,
                        target_event,
                        readers,
                        plan: prepared,
                    },
                },
            ))),
            Err(NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                error,
                prepared,
            }) => {
                self.poisoned = true;
                Err(ComputeDependencyPublicationFailureV1::Terminal(Box::new(
                    TerminalComputeDependencyTargetUseV1 {
                        error,
                        boundary: None,
                        key,
                        target_completion: TerminalComputeDependencyCompletionV1::Bound {
                            retention: target_completion,
                            event: Some(target_event),
                        },
                        readers,
                        plan: TerminalComputeDependencyPlanV1::BeforeClaim(prepared),
                    },
                )))
            }
            Err(NativeDependencyDispatchSubmissionFailureV1::TerminalAmbiguous {
                error,
                boundary,
                custody,
            }) => {
                self.poisoned = true;
                Err(ComputeDependencyPublicationFailureV1::Terminal(Box::new(
                    TerminalComputeDependencyTargetUseV1 {
                        error,
                        boundary: Some(boundary),
                        key,
                        target_completion: TerminalComputeDependencyCompletionV1::Bound {
                            retention: target_completion,
                            event: Some(target_event),
                        },
                        readers,
                        plan: TerminalComputeDependencyPlanV1::Ambiguous(custody),
                    },
                )))
            }
        }
    }

    pub(super) fn terminal_before_native_publication(
        &mut self,
        prepared: PreparedComputeDependencyTargetUseV1,
        error: NativeAqlSubmissionErrorV1,
    ) -> Box<TerminalComputeDependencyTargetUseV1> {
        self.poisoned = true;
        Box::new(TerminalComputeDependencyTargetUseV1 {
            error,
            boundary: None,
            key: prepared.key,
            target_completion: TerminalComputeDependencyCompletionV1::Bound {
                retention: prepared.target_completion,
                event: Some(prepared.target_event),
            },
            readers: prepared.readers,
            plan: TerminalComputeDependencyPlanV1::BeforeClaim(prepared.plan),
        })
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn bind_published_target(
        &mut self,
        native: NativePublishedComputeDependencyTargetUseV1,
        target_owner: &mut CompletionSignalArenaOwnerV1,
    ) -> Result<PublishedComputeDependencyTargetBundleV1, TerminalComputeDependencyTargetUseV1>
    {
        let target_batch = match target_owner.mark_published_retaining(
            native.target_completion,
            native.publication.last_packet_id(),
        ) {
            Ok(batch) => batch,
            Err((error, retention)) => {
                self.poisoned = true;
                return Err(TerminalComputeDependencyTargetUseV1 {
                    error: NativeAqlSubmissionErrorV1::InvalidQueue("published dependency target"),
                    boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                    key: native.key,
                    target_completion: TerminalComputeDependencyCompletionV1::Bound {
                        retention,
                        event: Some(native.target_event),
                    },
                    readers: native.readers,
                    plan: TerminalComputeDependencyPlanV1::PostPublicationCompletion(error),
                });
            }
        };
        let published_target = match target_owner.published_dependency_target_identity_v1(
            native.key.target.session_occurrence,
            native.key.target.acceptance_epoch,
            &target_batch,
        ) {
            Ok(identity) => identity,
            Err(error) => {
                self.poisoned = true;
                return Err(TerminalComputeDependencyTargetUseV1 {
                    error: NativeAqlSubmissionErrorV1::InvalidQueue("published dependency target"),
                    boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                    key: native.key,
                    target_completion: TerminalComputeDependencyCompletionV1::Published {
                        batch: target_batch,
                        event: Some(native.target_event),
                    },
                    readers: native.readers,
                    plan: TerminalComputeDependencyPlanV1::PostPublicationCompletion(error),
                });
            }
        };
        let target_event = match target_owner
            .bind_dependency_event_v1(native.target_event, &target_batch)
        {
            Ok(event) => event,
            Err((error, event)) => {
                self.poisoned = true;
                return Err(TerminalComputeDependencyTargetUseV1 {
                    error: NativeAqlSubmissionErrorV1::InvalidQueue("published dependency event"),
                    boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                    key: native.key,
                    target_completion: TerminalComputeDependencyCompletionV1::Published {
                        batch: target_batch,
                        event: Some(event),
                    },
                    readers: native.readers,
                    plan: TerminalComputeDependencyPlanV1::PostPublicationCompletion(error),
                });
            }
        };
        let expected = ComputeDependencyOccurrenceIdentityV1 {
            packet_id: Some(native.publication.last_packet_id()),
            ..native.key.target
        };
        if self.poisoned
            || self.active.get(&native.key.target.acceptance_epoch) != Some(&native.key)
            || published_target != expected
            || native.publication.dependency_count() != native.key.dependency_count
        {
            self.poisoned = true;
            return Err(TerminalComputeDependencyTargetUseV1 {
                error: NativeAqlSubmissionErrorV1::InvalidQueue("published dependency target"),
                boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                key: native.key,
                target_completion: TerminalComputeDependencyCompletionV1::Published {
                    batch: target_batch,
                    event: Some(target_event),
                },
                readers: native.readers,
                plan: TerminalComputeDependencyPlanV1::Published(native.publication),
            });
        }
        let key = ActiveComputeDependencyTargetUseV1 {
            target: published_target,
            phase: ComputeDependencyTargetUsePhaseV1::Published,
            ..native.key
        };
        let replaced = self.active.insert(key.target.acceptance_epoch, key);
        debug_assert!(replaced.is_some());
        Ok(PublishedComputeDependencyTargetBundleV1 {
            published: PublishedComputeDependencyTargetUseV1 {
                key,
                target_batch,
                readers: native.readers,
                publication: native.publication,
            },
            target_event,
        })
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn observe_published_target_once<B: NativeCompletionSignalBackendV1>(
        &mut self,
        published: PublishedComputeDependencyTargetUseV1,
        target_owner: &mut CompletionSignalArenaOwnerV1,
        backend: &mut B,
    ) -> Result<ComputeDependencyTargetPollV1, TerminalComputeDependencyTargetUseV1> {
        if self.poisoned
            || self.active.get(&published.key.target.acceptance_epoch) != Some(&published.key)
        {
            self.poisoned = true;
            return Err(TerminalComputeDependencyTargetUseV1 {
                error: NativeAqlSubmissionErrorV1::Poisoned,
                boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                key: published.key,
                target_completion: TerminalComputeDependencyCompletionV1::Published {
                    batch: published.target_batch,
                    event: None,
                },
                readers: published.readers,
                plan: TerminalComputeDependencyPlanV1::Published(published.publication),
            });
        }
        let PublishedComputeDependencyTargetUseV1 {
            key,
            target_batch,
            readers,
            publication,
        } = published;
        match target_owner.observe_once_with_progress_retaining(target_batch, backend) {
            Ok(Gfx942CompletionPollWithProgressV1::Pending { batch, .. }) => Ok(
                ComputeDependencyTargetPollV1::Pending(PublishedComputeDependencyTargetUseV1 {
                    key,
                    target_batch: batch,
                    readers,
                    publication,
                }),
            ),
            Ok(Gfx942CompletionPollWithProgressV1::Ready { completed, .. }) => {
                let completed_key = ActiveComputeDependencyTargetUseV1 {
                    phase: ComputeDependencyTargetUsePhaseV1::Completed,
                    ..key
                };
                let replaced = self
                    .active
                    .insert(completed_key.target.acceptance_epoch, completed_key);
                debug_assert!(replaced.is_some());
                Ok(ComputeDependencyTargetPollV1::Ready(
                    CompletedComputeDependencyTargetUseV1 {
                        key: completed_key,
                        target_completion: completed,
                        readers,
                        publication,
                    },
                ))
            }
            Err((error, batch)) => {
                self.poisoned = true;
                Err(TerminalComputeDependencyTargetUseV1 {
                    error: NativeAqlSubmissionErrorV1::InvalidQueue(
                        "dependent completion observation",
                    ),
                    boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                    key,
                    target_completion: TerminalComputeDependencyCompletionV1::Published {
                        batch,
                        event: None,
                    },
                    readers,
                    plan: TerminalComputeDependencyPlanV1::PostPublicationCompletion(error),
                })
            }
        }
    }

    /// Consumes source reader and event pins only after exact dependent
    /// completion. The production profile has two compute lanes, so all valid
    /// sources for one target belong to the one other lane owner.
    #[allow(clippy::result_large_err)]
    pub(super) fn release_after_dependent_completion(
        &mut self,
        completed: CompletedComputeDependencyTargetUseV1,
        target_owner: &CompletionSignalArenaOwnerV1,
        source_owner: &mut CompletionSignalArenaOwnerV1,
    ) -> Result<ReleasedComputeDependencyTargetUseV1, TerminalComputeDependencyTargetUseV1> {
        let fail = |owner: &mut Self,
                    error: Gfx942CompletionErrorV1,
                    completed: CompletedComputeDependencyTargetUseV1| {
            owner.poisoned = true;
            TerminalComputeDependencyTargetUseV1 {
                error: NativeAqlSubmissionErrorV1::InvalidQueue(
                    "dependent completion reader release",
                ),
                boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                key: completed.key,
                target_completion: TerminalComputeDependencyCompletionV1::Completed {
                    batch: completed.target_completion,
                    event: None,
                },
                readers: completed.readers,
                plan: TerminalComputeDependencyPlanV1::PostPublicationCompletion(error),
            }
        };
        if self.poisoned
            || self.active.get(&completed.key.target.acceptance_epoch) != Some(&completed.key)
        {
            return Err(fail(
                self,
                Gfx942CompletionErrorV1::StaleBatchGeneration,
                completed,
            ));
        }
        let _target = match target_owner.completed_dependency_target_identity_v1(
            completed.key.target.session_occurrence,
            completed.key.target.acceptance_epoch,
            &completed.target_completion,
        ) {
            Ok(target) if target == completed.key.target => target,
            Ok(_) => {
                return Err(fail(
                    self,
                    Gfx942CompletionErrorV1::StaleBatchGeneration,
                    completed,
                ));
            }
            Err(error) => return Err(fail(self, error, completed)),
        };
        let mut retained = Vec::new();
        let mut metadata = Vec::new();
        let mut restored_readers = Vec::new();
        if retained.try_reserve_exact(completed.readers.len()).is_err()
            || metadata.try_reserve_exact(completed.readers.len()).is_err()
            || restored_readers
                .try_reserve_exact(completed.readers.len())
                .is_err()
        {
            return Err(fail(
                self,
                Gfx942CompletionErrorV1::DependencyLedgerAllocation,
                completed,
            ));
        }
        for reader in &completed.readers {
            if !source_owner.matches_dependency_source_arena_v1(
                reader.source.queue,
                reader.source.signal_mapping,
            ) || source_owner.dependency_source_identity_v1(&reader.event) != Ok(reader.source)
                || source_owner.native_dependency_signal_observation_v1(&reader.lease)
                    != Ok(reader.signal)
            {
                return Err(fail(
                    self,
                    Gfx942CompletionErrorV1::StaleDependencyReader,
                    completed,
                ));
            }
            metadata.push((reader.source, reader.signal));
        }
        let CompletedComputeDependencyTargetUseV1 {
            key,
            target_completion,
            readers,
            publication,
        } = completed;
        retained.extend(
            readers
                .into_iter()
                .map(|reader| (reader.event, reader.lease)),
        );
        if let Err((error, retained)) =
            source_owner.release_dependency_reader_event_batch_v1(retained)
        {
            for ((event, lease), (source, signal)) in retained.into_iter().zip(metadata) {
                restored_readers.push(RetainedComputeDependencyReaderV1 {
                    event,
                    lease,
                    source,
                    signal,
                });
            }
            self.poisoned = true;
            return Err(TerminalComputeDependencyTargetUseV1 {
                error: NativeAqlSubmissionErrorV1::InvalidQueue(
                    "dependent completion reader release",
                ),
                boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
                key,
                target_completion: TerminalComputeDependencyCompletionV1::Completed {
                    batch: target_completion,
                    event: None,
                },
                readers: restored_readers,
                plan: TerminalComputeDependencyPlanV1::PostPublicationCompletion(error),
            });
        }
        let removed = self.active.remove(&key.target.acceptance_epoch);
        debug_assert_eq!(removed, Some(key));
        Ok(ReleasedComputeDependencyTargetUseV1 {
            target_completion,
            dependency_count: key.dependency_count,
            publication,
        })
    }

    pub(super) fn terminal_after_dependent_completion(
        &mut self,
        completed: CompletedComputeDependencyTargetUseV1,
        error: NativeAqlSubmissionErrorV1,
    ) -> TerminalComputeDependencyTargetUseV1 {
        self.poisoned = true;
        TerminalComputeDependencyTargetUseV1 {
            error,
            boundary: Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell),
            key: completed.key,
            target_completion: TerminalComputeDependencyCompletionV1::Completed {
                batch: completed.target_completion,
                event: None,
            },
            readers: completed.readers,
            plan: TerminalComputeDependencyPlanV1::Published(completed.publication),
        }
    }

    /// Releases every reader after a proven no-effect native rejection.
    /// Returned events remain in their original dependency order.
    #[allow(clippy::result_large_err)]
    pub(super) fn rollback_retryable_before_side_effect(
        &mut self,
        retryable: Box<RetryableComputeDependencyTargetUseV1>,
        source_owners: &mut [&mut CompletionSignalArenaOwnerV1],
        target_owner: &mut CompletionSignalArenaOwnerV1,
    ) -> Result<CancelledComputeDependencyTargetUseV1, ComputeDependencyRollbackFailureV1> {
        let RetryableComputeDependencyTargetUseV1 { prepared, .. } = *retryable;
        if self.poisoned
            || self.active.get(&prepared.key.target.acceptance_epoch) != Some(&prepared.key)
        {
            self.poisoned = true;
            return Err(rollback_preflight_failure_v1(
                ComputeDependencyTargetUseErrorV1::Poisoned,
                prepared,
            ));
        }

        let target_identity = match target_owner.validate_bound_dependency_target_custody_v1(
            prepared.key.target.session_occurrence,
            prepared.key.target.acceptance_epoch,
            &prepared.target_completion,
            &prepared.target_event,
        ) {
            Ok(identity) => identity,
            Err(error) => {
                self.poisoned = true;
                return Err(rollback_preflight_failure_v1(
                    ComputeDependencyTargetUseErrorV1::Completion(error),
                    prepared,
                ));
            }
        };
        if target_identity != prepared.key.target {
            self.poisoned = true;
            return Err(rollback_preflight_failure_v1(
                ComputeDependencyTargetUseErrorV1::Completion(
                    Gfx942CompletionErrorV1::StaleBatchGeneration,
                ),
                prepared,
            ));
        }
        let mut routes = Vec::new();
        if routes.try_reserve_exact(prepared.readers.len()).is_err() {
            self.poisoned = true;
            return Err(rollback_preflight_failure_v1(
                ComputeDependencyTargetUseErrorV1::Allocation,
                prepared,
            ));
        }
        for reader in &prepared.readers {
            let mut route = None;
            for (owner_index, owner) in source_owners.iter().enumerate() {
                if owner.matches_dependency_source_arena_v1(
                    reader.source.queue,
                    reader.source.signal_mapping,
                ) && route.replace(owner_index).is_some()
                {
                    self.poisoned = true;
                    return Err(rollback_preflight_failure_v1(
                        ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch,
                        prepared,
                    ));
                }
            }
            let Some(owner_index) = route else {
                self.poisoned = true;
                return Err(rollback_preflight_failure_v1(
                    ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch,
                    prepared,
                ));
            };
            match source_owners[owner_index].native_dependency_signal_observation_v1(&reader.lease)
            {
                Ok(signal) if signal == reader.signal => routes.push(owner_index),
                Ok(_) => {
                    self.poisoned = true;
                    return Err(rollback_preflight_failure_v1(
                        ComputeDependencyTargetUseErrorV1::Completion(
                            Gfx942CompletionErrorV1::StaleDependencyReader,
                        ),
                        prepared,
                    ));
                }
                Err(error) => {
                    self.poisoned = true;
                    return Err(rollback_preflight_failure_v1(
                        ComputeDependencyTargetUseErrorV1::Completion(error),
                        prepared,
                    ));
                }
            }
        }

        let mut released_events = Vec::new();
        if released_events
            .try_reserve_exact(prepared.readers.len())
            .is_err()
        {
            self.poisoned = true;
            return Err(rollback_preflight_failure_v1(
                ComputeDependencyTargetUseErrorV1::Allocation,
                prepared,
            ));
        }
        let PreparedComputeDependencyTargetUseV1 {
            key,
            target_completion,
            target_event,
            readers,
            plan,
        } = prepared;
        let mut remaining = readers;
        while let Some(reader) = remaining.pop() {
            let owner_index = routes
                .pop()
                .expect("one source-owner route was preflighted per reader");
            let RetainedComputeDependencyReaderV1 {
                event,
                lease,
                source,
                signal,
            } = reader;
            match source_owners[owner_index].release_dependency_reader_v1(lease) {
                Ok(_) => released_events.push(event),
                Err((error, lease)) => {
                    remaining.push(RetainedComputeDependencyReaderV1 {
                        event,
                        lease,
                        source,
                        signal,
                    });
                    self.poisoned = true;
                    return Err(ComputeDependencyRollbackFailureV1 {
                        error: ComputeDependencyTargetUseErrorV1::Completion(error),
                        target: key.target,
                        target_completion,
                        target_event: Some(target_event),
                        released_events,
                        retained_readers: remaining,
                        plan,
                    });
                }
            }
        }
        released_events.reverse();
        if let Err((error, target_event)) = target_owner.release_dependency_event_v1(target_event) {
            self.poisoned = true;
            return Err(ComputeDependencyRollbackFailureV1 {
                error: ComputeDependencyTargetUseErrorV1::Completion(error),
                target: key.target,
                target_completion,
                target_event: Some(target_event),
                released_events,
                retained_readers: remaining,
                plan,
            });
        }
        let removed = self.active.remove(&key.target.acceptance_epoch);
        debug_assert_eq!(removed, Some(key));
        Ok(CancelledComputeDependencyTargetUseV1 {
            target: key.target,
            target_completion,
            events: released_events,
        })
    }

    #[cfg(test)]
    fn next_epoch(&self) -> Option<u64> {
        self.next_acceptance_epoch
    }
}

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
mod tests {
    use super::*;
    use crate::queue::completion::{
        CompletionDispatchGenerationBindingV1, CompletionPacketTemplateV1,
        ComputeDependencyTargetSubstitutionV1, substitute_dependency_target_component_for_test,
    };
    use crate::queue::submit::{
        initialize_invalid_ring, publish_slot_header_release, write_unpublished_slot,
    };
    use core::sync::atomic::{AtomicU64, Ordering};
    use fe2o3_aql::{AqlDispatchGeometryV1, AqlDispatchOrderingV1, ObservedGpuAddressV1};
    use fe2o3_runtime_model::{
        AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
        MemoryAllocationKeyV1, MemoryMappingKeyV1, PhysicalDeviceIdV1, QueueGenerationV1,
        QueueInstanceIdV1, QueueKeyV1, VmIdV1, VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    #[repr(align(64))]
    struct DependencyRing([u8; 4_096]);

    struct DependencyBackend {
        ring: DependencyRing,
        write: AtomicU64,
        read: AtomicU64,
        checks: usize,
        fail_check: Option<usize>,
        panic_check: Option<usize>,
    }

    struct CompletionBackend {
        observation: fe2o3_aql::AqlCompletionObservationV1,
        reset_calls: usize,
    }

    impl NativeCompletionSignalBackendV1 for CompletionBackend {
        fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
            Ok(())
        }

        fn observe_one_acquire_in_current_scope(
            &mut self,
            _slot_index: u32,
        ) -> Result<fe2o3_aql::AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
            Ok(self.observation)
        }

        fn observe_batch_acquire_in_current_scope(
            &mut self,
            slot_indices: &[u32],
        ) -> Result<Vec<fe2o3_aql::AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
            Ok(vec![self.observation; slot_indices.len()])
        }

        fn reset_pending_release(
            &mut self,
            _slot_index: u32,
        ) -> Result<(), Gfx942CompletionErrorV1> {
            self.reset_calls += 1;
            Ok(())
        }
    }

    impl DependencyBackend {
        fn new(write: u64, read: u64) -> Self {
            let mut ring = DependencyRing([0; 4_096]);
            initialize_invalid_ring(&mut ring.0).unwrap();
            Self {
                ring,
                write: AtomicU64::new(write),
                read: AtomicU64::new(read),
                checks: 0,
                fail_check: None,
                panic_check: None,
            }
        }
    }

    impl NativeAqlSubmissionBackendV1 for DependencyBackend {
        fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.checks += 1;
            assert_ne!(self.panic_check, Some(self.checks), "injected claim panic");
            if self.fail_check == Some(self.checks) {
                Err(NativeAqlSubmissionErrorV1::Currentness)
            } else {
                Ok(())
            }
        }

        fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
            Ok((
                self.write.load(Ordering::Acquire),
                self.read.load(Ordering::Acquire),
            ))
        }

        fn fetch_add_write_acq_rel(
            &mut self,
            increment: u64,
        ) -> Result<u64, NativeAqlSubmissionErrorV1> {
            Ok(self.write.fetch_add(increment, Ordering::AcqRel))
        }

        fn write_unpublished(
            &mut self,
            slot: u32,
            packet: &[u8; 64],
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            write_unpublished_slot(&mut self.ring.0, slot, packet)
        }

        fn publish_release_header(
            &mut self,
            slot: u32,
            header: u16,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            publish_slot_header_release(&mut self.ring.0, slot, header)
        }

        fn ring_doorbell_release(
            &mut self,
            _packet_id: u64,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            Ok(())
        }
    }

    fn queue(id: u64) -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(1),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(id),
            generation: QueueGenerationV1(1),
        }
    }

    fn mapping(id: u64) -> MemoryMappingKeyV1 {
        MemoryMappingKeyV1 {
            allocation: MemoryAllocationKeyV1 {
                vm: queue(1).vm,
                id: AllocationIdV1(id),
                generation: AllocationGenerationV1(1),
            },
            id: MappingIdV1(id),
        }
    }

    fn template(queue: QueueKeyV1, generation: u64) -> CompletionPacketTemplateV1 {
        CompletionPacketTemplateV1::new(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            0,
            ObservedGpuAddressV1::new(0x40_0000 + generation * 0x100).unwrap(),
            ObservedGpuAddressV1::new(0x50_0000 + generation * 0x100).unwrap(),
            16,
            CompletionDispatchGenerationBindingV1::new(
                queue,
                mapping(100 + generation),
                mapping(200 + generation),
                generation,
            ),
        )
    }

    fn real_prepared_target() -> (
        CompletionSignalArenaOwnerV1,
        Gfx942CompletionBatchV1<1>,
        CompletionSignalArenaOwnerV1,
        ComputeDependencySessionOwnerV1,
        PreparedComputeDependencyTargetUseV1,
        NativeAqlSubmissionOwnerV1,
    ) {
        const SESSION: u64 = 7;
        let source_queue = queue(1);
        let target_queue = queue(2);
        let mut source_owner =
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(source_queue);
        let source_bound = source_owner
            .bind_batch([template(source_queue, 11)])
            .unwrap();
        let (_source_packets, source_retention) = source_bound.into_parts();
        let source_event = source_owner
            .record_dependency_event_v1(SESSION, 1, &source_retention)
            .unwrap();
        let source_batch = source_owner.mark_published(source_retention, 100).unwrap();
        let source_event = source_owner
            .bind_dependency_event_v1(source_event, &source_batch)
            .unwrap();

        let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(SESSION).unwrap();
        let source_epoch = acceptance_owner.reserve_acceptance_epoch().unwrap();
        assert_eq!(source_epoch.epoch(), 1);
        let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        let reader =
            retain_dependency_reader_for_target_v1(&mut source_owner, source_event, &acceptance)
                .unwrap();

        let mut target_owner =
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(target_queue);
        let target_bound = target_owner
            .bind_batch([template(target_queue, 21)])
            .unwrap();
        let target = target_owner
            .prepare_dependency_target_v1(
                acceptance.session_occurrence(),
                acceptance.epoch(),
                target_bound,
            )
            .unwrap();
        let prepared = acceptance_owner
            .begin_target_use(&target_owner, acceptance, target, vec![reader])
            .unwrap();
        (
            source_owner,
            source_batch,
            target_owner,
            acceptance_owner,
            prepared,
            NativeAqlSubmissionOwnerV1::new(4_096).unwrap(),
        )
    }

    fn published_source_event(
        owner: &mut CompletionSignalArenaOwnerV1,
        source_queue: QueueKeyV1,
        generation: u64,
        packet_id: u64,
    ) -> Gfx942ComputeEventOccurrenceV1 {
        let bound = owner
            .bind_batch([template(source_queue, generation)])
            .unwrap();
        let (_packets, retention) = bound.into_parts();
        let event = owner.record_dependency_event_v1(7, 1, &retention).unwrap();
        let batch = owner.mark_published(retention, packet_id).unwrap();
        owner.bind_dependency_event_v1(event, &batch).unwrap()
    }

    fn occurrence(
        session: u64,
        epoch: u64,
        queue_id: u64,
        slot: u32,
        packet_id: Option<u64>,
    ) -> ComputeDependencyOccurrenceIdentityV1 {
        ComputeDependencyOccurrenceIdentityV1 {
            session_occurrence: session,
            acceptance_epoch: epoch,
            batch_id: epoch,
            queue: queue(queue_id),
            signal_mapping: mapping(queue_id),
            slot_index: slot,
            slot_generation: epoch,
            dispatch_generation: epoch,
            packet_id,
        }
    }

    #[test]
    fn acceptance_epochs_are_unique_monotonic_and_burned() {
        let mut owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
        {
            let first = owner.reserve_acceptance_epoch().unwrap();
            assert_eq!(first.epoch, 1);
        }
        let second = owner.reserve_acceptance_epoch().unwrap();
        assert_eq!(second.epoch, 2);
        assert_eq!(owner.next_epoch(), Some(3));
    }

    #[test]
    fn final_nonzero_epoch_is_issued_once_then_exhaustion_poison_is_permanent() {
        let mut owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
        owner.next_acceptance_epoch = Some(u64::MAX);
        assert_eq!(owner.reserve_acceptance_epoch().unwrap().epoch(), u64::MAX);
        assert_eq!(owner.next_epoch(), None);
        assert_eq!(
            owner.reserve_acceptance_epoch(),
            Err(ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted)
        );
        assert_eq!(
            owner.reserve_acceptance_epoch(),
            Err(ComputeDependencyTargetUseErrorV1::Poisoned)
        );
    }

    #[test]
    fn isolated_foundation_owners_show_why_production_construction_stays_private() {
        let mut first = ComputeDependencySessionOwnerV1::new(7).unwrap();
        let mut duplicate = ComputeDependencySessionOwnerV1::new(7).unwrap();
        assert_eq!(first.reserve_acceptance_epoch().unwrap().epoch(), 1);
        assert_eq!(duplicate.reserve_acceptance_epoch().unwrap().epoch(), 1);
    }

    #[test]
    fn cross_batch_target_components_fail_before_active_or_native_mutation() {
        for substitution in [
            ComputeDependencyTargetSubstitutionV1::FinalDispatch,
            ComputeDependencyTargetSubstitutionV1::Retention,
            ComputeDependencyTargetSubstitutionV1::Event,
        ] {
            let target_queue = queue(2);
            let mut target_owner =
                CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(
                    target_queue,
                );
            let first_bound = target_owner
                .bind_batch([template(target_queue, 21)])
                .unwrap();
            let second_bound = target_owner
                .bind_batch([template(target_queue, 22)])
                .unwrap();
            let first = target_owner
                .prepare_dependency_target_v1(7, 1, first_bound)
                .unwrap();
            let second = target_owner
                .prepare_dependency_target_v1(7, 2, second_bound)
                .unwrap();
            let (hostile, _other) =
                substitute_dependency_target_component_for_test(first, second, substitution);

            let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
            let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
            let backend = DependencyBackend::new(0, 0);
            let before = backend.ring.0;
            let failure = match acceptance_owner.begin_target_use(
                &target_owner,
                acceptance,
                hostile,
                Vec::new(),
            ) {
                Err(failure) => failure,
                Ok(_) => panic!("cross-batch target substitution was admitted"),
            };
            let (error, returned_acceptance, _returned_target, returned_readers) =
                failure.into_parts();
            assert!(matches!(
                error,
                ComputeDependencyTargetUseErrorV1::Completion(_)
            ));
            assert_eq!(returned_acceptance.epoch(), 1);
            assert!(returned_readers.is_empty());
            assert!(acceptance_owner.active.is_empty());
            assert_eq!(backend.write.load(Ordering::Relaxed), 0);
            assert_eq!(backend.ring.0, before);
        }
    }

    #[test]
    fn target_use_requires_cross_queue_earlier_distinct_sources() {
        let acceptance = ComputeDependencyAcceptanceV1 {
            session_occurrence: 7,
            epoch: 4,
        };
        let target = occurrence(7, 4, 2, 0, None);
        let first = occurrence(7, 1, 1, 3, Some(10));
        let second = occurrence(7, 3, 1, 4, Some(11));
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &[first, second]),
            Ok(())
        );

        let cases = [
            (
                occurrence(8, 1, 1, 3, Some(10)),
                ComputeDependencyTargetUseErrorV1::CrossSessionDependency,
            ),
            (
                occurrence(7, 1, 2, 3, Some(10)),
                ComputeDependencyTargetUseErrorV1::SameQueueDependency,
            ),
            (
                occurrence(7, 4, 1, 3, Some(10)),
                ComputeDependencyTargetUseErrorV1::SelfDependency,
            ),
            (
                occurrence(7, 5, 1, 3, Some(10)),
                ComputeDependencyTargetUseErrorV1::DependencyCycle,
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(
                validate_target_use_v1(7, &acceptance, target, &[source]),
                Err(expected)
            );
        }
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &[first, first]),
            Err(ComputeDependencyTargetUseErrorV1::DuplicateDependency)
        );
        assert_eq!(
            validate_target_use_v1(
                7,
                &acceptance,
                target,
                &[first, occurrence(7, 2, 3, 4, Some(11))],
            ),
            Err(ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch)
        );
    }

    #[test]
    fn target_use_rejects_empty_oversized_and_prepublished_sources() {
        let acceptance = ComputeDependencyAcceptanceV1 {
            session_occurrence: 7,
            epoch: 4,
        };
        let target = occurrence(7, 4, 2, 0, None);
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &[]),
            Err(ComputeDependencyTargetUseErrorV1::EmptyDependencyRoster)
        );
        let oversized = vec![occurrence(7, 1, 1, 3, Some(10)); 257];
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &oversized),
            Err(ComputeDependencyTargetUseErrorV1::TooManyDependencies)
        );
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &[occurrence(7, 1, 1, 3, None)]),
            Err(ComputeDependencyTargetUseErrorV1::InvalidTargetIdentity)
        );

        let maximum = (0..AQL_MAX_DEPENDENCY_SIGNALS_V1)
            .map(|slot| occurrence(7, 1, 1, slot as u32, Some(u64::from(slot as u32) + 10)))
            .collect::<Vec<_>>();
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &maximum),
            Ok(())
        );
    }

    #[test]
    fn occupancy_rollback_releases_real_readers_in_reverse_and_returns_events_in_order() {
        const SESSION: u64 = 7;
        let source_queue = queue(1);
        let target_queue = queue(2);
        let mut source_owner =
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(source_queue);
        let source_bound = source_owner
            .bind_batch([
                template(source_queue, 11),
                template(source_queue, 12),
                template(source_queue, 13),
            ])
            .unwrap();
        let (_source_packets, source_retention) = source_bound.into_parts();
        let mut events = Vec::new();
        for index in 0..3 {
            events.push(
                source_owner
                    .record_dependency_event_at_v1(SESSION, 1, &source_retention, index)
                    .unwrap(),
            );
        }
        let source_batch = source_owner.mark_published(source_retention, 102).unwrap();
        let events = events
            .into_iter()
            .enumerate()
            .map(|(index, event)| {
                source_owner
                    .bind_dependency_event_at_v1(event, &source_batch, index)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(SESSION).unwrap();
        let source_epoch = acceptance_owner.reserve_acceptance_epoch().unwrap();
        assert_eq!(source_epoch.epoch(), 1);
        let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        assert_eq!(acceptance.epoch(), 2);
        let mut readers = Vec::new();
        for event in events {
            readers.push(
                retain_dependency_reader_for_target_v1(&mut source_owner, event, &acceptance)
                    .unwrap(),
            );
        }

        let mut target_owner =
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(target_queue);
        let target_bound = target_owner
            .bind_batch([template(target_queue, 21)])
            .unwrap();
        let target = target_owner
            .prepare_dependency_target_v1(
                acceptance.session_occurrence(),
                acceptance.epoch(),
                target_bound,
            )
            .unwrap();
        let prepared = acceptance_owner
            .begin_target_use(&target_owner, acceptance, target, readers)
            .unwrap();

        let mut submission = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
        let mut backend = DependencyBackend::new(64, 0);
        let retryable = match acceptance_owner
            .publish_native(prepared, &mut submission, &mut backend)
            .unwrap_err()
        {
            ComputeDependencyPublicationFailureV1::Retryable(custody) => custody,
            ComputeDependencyPublicationFailureV1::Terminal(_) => {
                panic!("ring occupancy unexpectedly became terminal")
            }
        };
        assert!(matches!(
            retryable.error,
            NativeAqlSubmissionErrorV1::Ring(fe2o3_aql::AqlRingReservationError::Full)
        ));
        let mut source_owners = [&mut source_owner];
        let cancelled = acceptance_owner
            .rollback_retryable_before_side_effect(retryable, &mut source_owners, &mut target_owner)
            .unwrap();
        assert_eq!(cancelled.events.len(), 3);
        let returned_slots = cancelled
            .events
            .iter()
            .map(|event| {
                source_owner
                    .dependency_source_identity_v1(event)
                    .unwrap()
                    .slot_index
            })
            .collect::<Vec<_>>();
        assert_eq!(returned_slots, [0, 1, 2]);
        for event in cancelled.events {
            let (event, lease) = source_owner
                .retain_dependency_reader_v1(event, SESSION, 2)
                .unwrap();
            source_owner.release_dependency_reader_v1(lease).unwrap();
            source_owner.release_dependency_event_v1(event).unwrap();
        }
        target_owner
            .cancel_bound(cancelled.target_completion)
            .unwrap();
        assert_eq!(
            acceptance_owner.reserve_acceptance_epoch().unwrap().epoch(),
            3
        );
    }

    #[test]
    fn claim_currentness_and_panic_keep_real_target_and_reader_authority_terminal() {
        for panic in [false, true] {
            let (
                source_owner,
                _source_batch,
                target_owner,
                mut acceptance_owner,
                prepared,
                mut submission,
            ) = real_prepared_target();
            let mut backend = DependencyBackend::new(0, 0);
            if panic {
                backend.panic_check = Some(3);
            } else {
                backend.fail_check = Some(3);
            }
            let terminal = match acceptance_owner
                .publish_native(prepared, &mut submission, &mut backend)
                .unwrap_err()
            {
                ComputeDependencyPublicationFailureV1::Terminal(custody) => custody,
                ComputeDependencyPublicationFailureV1::Retryable(_) => {
                    panic!("claim attempt incorrectly yielded retryable target authority")
                }
            };
            assert_eq!(
                terminal.boundary,
                Some(AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex)
            );
            assert!(matches!(
                terminal.error,
                NativeAqlSubmissionErrorV1::Currentness | NativeAqlSubmissionErrorV1::CallbackPanic
            ));
            assert_eq!(terminal.readers.len(), 1);
            assert!(matches!(
                terminal.target_completion,
                TerminalComputeDependencyCompletionV1::Bound { .. }
            ));
            assert_eq!(
                acceptance_owner.reserve_acceptance_epoch(),
                Err(ComputeDependencyTargetUseErrorV1::Poisoned)
            );
            assert_eq!(
                target_owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::BatchStillRetained)
            );
            assert_eq!(
                source_owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::BatchStillRetained)
            );
        }
    }

    #[test]
    fn successful_native_publication_binds_exact_target_without_source_polling() {
        let (
            source_owner,
            _source_batch,
            mut target_owner,
            mut acceptance_owner,
            prepared,
            mut submission,
        ) = real_prepared_target();
        let mut backend = DependencyBackend::new(0, 0);
        let native = acceptance_owner
            .publish_native(prepared, &mut submission, &mut backend)
            .unwrap();
        assert_eq!(backend.checks, 4);
        assert_eq!(backend.write.load(Ordering::Relaxed), 2);
        let (published, target_event) = acceptance_owner
            .bind_published_target(native, &mut target_owner)
            .unwrap()
            .into_parts();
        assert_eq!(
            published.key.phase,
            ComputeDependencyTargetUsePhaseV1::Published
        );
        assert_eq!(published.key.target.packet_id, Some(1));
        assert_eq!(published.readers.len(), 1);
        assert_eq!(
            target_event.binding_state(),
            super::super::completion::Gfx942ComputeEventBindingStateV1::Bound
        );
        assert_eq!(published.publication.dependency_count(), 1);
        assert_eq!(
            source_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
        assert_eq!(
            target_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
    }

    #[test]
    fn published_target_owner_substitution_is_terminal() {
        let (
            mut source_owner,
            _source_batch,
            target_owner,
            mut acceptance_owner,
            prepared,
            mut submission,
        ) = real_prepared_target();
        let mut backend = DependencyBackend::new(0, 0);
        let native = acceptance_owner
            .publish_native(prepared, &mut submission, &mut backend)
            .unwrap();
        let terminal = acceptance_owner
            .bind_published_target(native, &mut source_owner)
            .unwrap_err();
        assert_eq!(
            terminal.boundary,
            Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell)
        );
        assert!(matches!(
            terminal.target_completion,
            TerminalComputeDependencyCompletionV1::Bound { .. }
        ));
        assert_eq!(
            acceptance_owner.reserve_acceptance_epoch(),
            Err(ComputeDependencyTargetUseErrorV1::Poisoned)
        );
        assert_eq!(
            target_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
    }

    #[test]
    fn exact_target_completion_releases_source_pins_once_then_enables_recycle() {
        let (
            mut source_owner,
            source_batch,
            mut target_owner,
            mut acceptance_owner,
            prepared,
            mut submission,
        ) = real_prepared_target();

        let mut native_backend = DependencyBackend::new(0, 0);
        let native = acceptance_owner
            .publish_native(prepared, &mut submission, &mut native_backend)
            .unwrap();
        let (published, target_event) = acceptance_owner
            .bind_published_target(native, &mut target_owner)
            .unwrap()
            .into_parts();

        let mut pending_backend = CompletionBackend {
            observation: fe2o3_aql::AqlCompletionObservationV1::Pending,
            reset_calls: 0,
        };
        let published = match acceptance_owner
            .observe_published_target_once(published, &mut target_owner, &mut pending_backend)
            .unwrap()
        {
            ComputeDependencyTargetPollV1::Pending(published) => published,
            ComputeDependencyTargetPollV1::Ready(_) => panic!("pending target completed"),
        };

        let mut complete_backend = CompletionBackend {
            observation: fe2o3_aql::AqlCompletionObservationV1::Completed,
            reset_calls: 0,
        };
        let source_completed = match source_owner
            .observe_once(source_batch, &mut complete_backend)
            .unwrap()
        {
            super::super::completion::Gfx942CompletionPollV1::Ready(completed) => completed,
            super::super::completion::Gfx942CompletionPollV1::Pending(_) => {
                panic!("completed source remained pending")
            }
        };
        let source_completed =
            match source_owner.recycle_retaining(source_completed, &mut complete_backend) {
                Err((Gfx942CompletionErrorV1::SignalPinned { .. }, completed)) => completed,
                other => panic!("source recycle was not pinned: {other:?}"),
            };
        assert_eq!(complete_backend.reset_calls, 0);

        let completed = match acceptance_owner
            .observe_published_target_once(published, &mut target_owner, &mut complete_backend)
            .unwrap()
        {
            ComputeDependencyTargetPollV1::Ready(completed) => completed,
            ComputeDependencyTargetPollV1::Pending(_) => {
                panic!("completed target remained pending")
            }
        };
        let released = acceptance_owner
            .release_after_dependent_completion(completed, &target_owner, &mut source_owner)
            .unwrap();
        assert_eq!(released.dependency_count, 1);
        assert!(acceptance_owner.active.is_empty());

        source_owner
            .recycle_retaining(source_completed, &mut complete_backend)
            .unwrap();
        assert_eq!(complete_backend.reset_calls, 1);
        let target_completed = released.target_completion;
        let target_completed =
            match target_owner.recycle_retaining(target_completed, &mut complete_backend) {
                Err((Gfx942CompletionErrorV1::SignalPinned { .. }, completed)) => completed,
                other => panic!("target recycle was not event-pinned: {other:?}"),
            };
        target_owner
            .release_dependency_event_v1(target_event)
            .unwrap();
        target_owner
            .recycle_retaining(target_completed, &mut complete_backend)
            .unwrap();
        assert_eq!(complete_backend.reset_calls, 2);
        assert_eq!(source_owner.ensure_releasable(), Ok(()));
        assert_eq!(target_owner.ensure_releasable(), Ok(()));
    }

    #[test]
    fn published_target_event_can_feed_a_second_target_before_first_completion() {
        let (
            mut source_owner,
            _source_batch,
            mut middle_owner,
            mut acceptance_owner,
            prepared_middle,
            mut submission,
        ) = real_prepared_target();
        let mut native_backend = DependencyBackend::new(0, 0);
        let native_middle = acceptance_owner
            .publish_native(prepared_middle, &mut submission, &mut native_backend)
            .unwrap();
        let (published_middle, middle_event) = acceptance_owner
            .bind_published_target(native_middle, &mut middle_owner)
            .unwrap()
            .into_parts();

        let final_acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        assert_eq!(final_acceptance.epoch(), 3);
        let middle_reader = retain_dependency_reader_for_target_v1(
            &mut middle_owner,
            middle_event,
            &final_acceptance,
        )
        .unwrap();
        let final_queue = queue(3);
        let mut final_owner =
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(final_queue);
        let final_bound = final_owner.bind_batch([template(final_queue, 31)]).unwrap();
        let final_target = final_owner
            .prepare_dependency_target_v1(
                final_acceptance.session_occurrence(),
                final_acceptance.epoch(),
                final_bound,
            )
            .unwrap();
        let _prepared_final = acceptance_owner
            .begin_target_use(
                &final_owner,
                final_acceptance,
                final_target,
                vec![middle_reader],
            )
            .unwrap();
        assert_eq!(acceptance_owner.active.len(), 2);

        let mut completed_backend = CompletionBackend {
            observation: fe2o3_aql::AqlCompletionObservationV1::Completed,
            reset_calls: 0,
        };
        let completed_middle = match acceptance_owner
            .observe_published_target_once(
                published_middle,
                &mut middle_owner,
                &mut completed_backend,
            )
            .unwrap()
        {
            ComputeDependencyTargetPollV1::Ready(completed) => completed,
            ComputeDependencyTargetPollV1::Pending(_) => panic!("middle target remained pending"),
        };
        acceptance_owner
            .release_after_dependent_completion(completed_middle, &middle_owner, &mut source_owner)
            .unwrap();
        assert_eq!(acceptance_owner.active.len(), 1);
        assert_eq!(
            middle_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
    }

    #[test]
    fn active_target_capacity_accepts_128_rejects_129_without_effect_and_reuses_release() {
        let (
            mut source_owner,
            _source_batch,
            mut target_owner,
            mut acceptance_owner,
            first_prepared,
            mut submission,
        ) = real_prepared_target();
        assert!(acceptance_owner.active.capacity() >= MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1);

        for epoch in 10_000..10_126 {
            let target = occurrence(7, epoch, 100 + epoch, epoch as u32, None);
            assert!(
                acceptance_owner
                    .active
                    .insert(
                        epoch,
                        ActiveComputeDependencyTargetUseV1 {
                            target,
                            dependency_count: 1,
                            phase: ComputeDependencyTargetUsePhaseV1::Published,
                        },
                    )
                    .is_none()
            );
        }
        assert_eq!(acceptance_owner.active.len(), 127);

        let second_acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        let second_event = published_source_event(&mut source_owner, queue(1), 12, 101);
        let second_reader = retain_dependency_reader_for_target_v1(
            &mut source_owner,
            second_event,
            &second_acceptance,
        )
        .unwrap();
        let second_bound = target_owner.bind_batch([template(queue(2), 22)]).unwrap();
        let second_target = target_owner
            .prepare_dependency_target_v1(7, second_acceptance.epoch(), second_bound)
            .unwrap();
        let _second_prepared = acceptance_owner
            .begin_target_use(
                &target_owner,
                second_acceptance,
                second_target,
                vec![second_reader],
            )
            .unwrap();
        assert_eq!(
            acceptance_owner.active.len(),
            MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1
        );

        let rejected_acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        let rejected_epoch = rejected_acceptance.epoch();
        let rejected_event = published_source_event(&mut source_owner, queue(1), 13, 102);
        let rejected_reader = retain_dependency_reader_for_target_v1(
            &mut source_owner,
            rejected_event,
            &rejected_acceptance,
        )
        .unwrap();
        let rejected_bound = target_owner.bind_batch([template(queue(2), 23)]).unwrap();
        let rejected_target = target_owner
            .prepare_dependency_target_v1(7, rejected_epoch, rejected_bound)
            .unwrap();
        let rejected_identity = rejected_target.identity();
        let native_backend = DependencyBackend::new(0, 0);
        let ring_before = native_backend.ring.0;
        let failure = match acceptance_owner.begin_target_use(
            &target_owner,
            rejected_acceptance,
            rejected_target,
            vec![rejected_reader],
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("the 129th active target was admitted"),
        };
        let (error, returned_acceptance, returned_target, returned_readers) = failure.into_parts();
        assert_eq!(
            error,
            ComputeDependencyTargetUseErrorV1::ActiveTargetCapacity
        );
        assert_eq!(returned_acceptance.epoch(), rejected_epoch);
        assert_eq!(returned_target.identity(), rejected_identity);
        assert_eq!(returned_readers.len(), 1);
        assert_eq!(native_backend.write.load(Ordering::Relaxed), 0);
        assert_eq!(native_backend.ring.0, ring_before);
        assert_eq!(
            acceptance_owner.active.len(),
            MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1
        );

        let mut native_backend = DependencyBackend::new(0, 0);
        let native_first = acceptance_owner
            .publish_native(first_prepared, &mut submission, &mut native_backend)
            .unwrap();
        let (published_first, _target_event) = acceptance_owner
            .bind_published_target(native_first, &mut target_owner)
            .unwrap()
            .into_parts();
        let mut completion_backend = CompletionBackend {
            observation: fe2o3_aql::AqlCompletionObservationV1::Completed,
            reset_calls: 0,
        };
        let completed_first = match acceptance_owner
            .observe_published_target_once(
                published_first,
                &mut target_owner,
                &mut completion_backend,
            )
            .unwrap()
        {
            ComputeDependencyTargetPollV1::Ready(completed) => completed,
            ComputeDependencyTargetPollV1::Pending(_) => panic!("target remained pending"),
        };
        acceptance_owner
            .release_after_dependent_completion(completed_first, &target_owner, &mut source_owner)
            .unwrap();
        assert_eq!(acceptance_owner.active.len(), 127);

        acceptance_owner
            .begin_target_use(
                &target_owner,
                returned_acceptance,
                returned_target,
                returned_readers,
            )
            .expect("a completed target must release one active-record slot");
        assert_eq!(
            acceptance_owner.active.len(),
            MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1
        );
    }

    #[test]
    fn mixed_source_owners_are_rejected_before_native_publication() {
        let first_queue = queue(1);
        let second_queue = queue(2);
        let target_queue = queue(3);
        let mut first_owner =
            CompletionSignalArenaOwnerV1::for_dependency_test(first_queue, 101, 0x10_000);
        let mut second_owner =
            CompletionSignalArenaOwnerV1::for_dependency_test(second_queue, 102, 0x20_000);
        let first_event = published_source_event(&mut first_owner, first_queue, 11, 100);
        let second_event = published_source_event(&mut second_owner, second_queue, 12, 200);
        let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
        acceptance_owner.reserve_acceptance_epoch().unwrap();
        let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        let first_reader =
            retain_dependency_reader_for_target_v1(&mut first_owner, first_event, &acceptance)
                .unwrap();
        let second_reader =
            retain_dependency_reader_for_target_v1(&mut second_owner, second_event, &acceptance)
                .unwrap();
        let mut target_owner =
            CompletionSignalArenaOwnerV1::for_dependency_test(target_queue, 103, 0x30_000);
        let target_bound = target_owner
            .bind_batch([template(target_queue, 21)])
            .unwrap();
        let target = target_owner
            .prepare_dependency_target_v1(7, acceptance.epoch(), target_bound)
            .unwrap();
        let failure = acceptance_owner.begin_target_use(
            &target_owner,
            acceptance,
            target,
            vec![first_reader, second_reader],
        );
        let failure = match failure {
            Err(failure) => failure,
            Ok(_) => panic!("mixed-owner foundation preparation was admitted"),
        };
        let (error, _acceptance, _target, readers) = failure.into_parts();
        assert_eq!(
            error,
            ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch
        );
        assert_eq!(readers.len(), 2);
        assert!(acceptance_owner.active.is_empty());
        assert_eq!(first_owner.dependency_reader_count_for_test(), 1);
        assert_eq!(second_owner.dependency_reader_count_for_test(), 1);
    }

    #[test]
    fn dependency_publisher_foundation_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_SHA256_V1
        );
    }
}
