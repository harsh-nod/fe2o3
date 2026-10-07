//! Dependency-session acceptance, publication, and reader retirement.

use super::*;

impl ComputeDependencySessionOwnerV1 {
    pub(in super::super) fn new(
        session_occurrence: u64,
    ) -> Result<Self, ComputeDependencyTargetUseErrorV1> {
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

    pub(in super::super) const fn session_occurrence(&self) -> u64 {
        self.session_occurrence
    }

    #[cfg(test)]
    pub(in super::super) fn custody_snapshot_for_test(
        &self,
    ) -> (u64, Option<u64>, Vec<(u64, usize)>) {
        let mut active: Vec<_> = self
            .active
            .iter()
            .map(|(epoch, owner)| (*epoch, owner as *const _ as usize))
            .collect();
        active.sort_unstable();
        (self.session_occurrence, self.next_acceptance_epoch, active)
    }

    #[cfg(test)]
    pub(in super::super) fn advance_to_last_acceptance_epoch_for_test(&mut self) {
        self.next_acceptance_epoch = Some(u64::MAX);
    }

    pub(in super::super) fn ensure_idle(&self) -> Result<(), ComputeDependencyTargetUseErrorV1> {
        if self.poisoned {
            return Err(ComputeDependencyTargetUseErrorV1::Poisoned);
        }
        if !self.active.is_empty() {
            return Err(ComputeDependencyTargetUseErrorV1::ActiveTargetUse);
        }
        Ok(())
    }

    pub(in super::super) fn poison(&mut self) {
        self.poisoned = true;
    }

    pub(in super::super) fn ensure_target_capacity(
        &self,
    ) -> Result<(), ComputeDependencyTargetUseErrorV1> {
        if self.poisoned {
            return Err(ComputeDependencyTargetUseErrorV1::Poisoned);
        }
        if self.active.len() >= MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1 {
            return Err(ComputeDependencyTargetUseErrorV1::ActiveTargetCapacity);
        }
        Ok(())
    }

    /// Issues and burns one epoch. Cancellation never rewinds this counter.
    pub(in super::super) fn reserve_acceptance_epoch(
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
    pub(in super::super) fn begin_target_use(
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

    pub(in super::super) fn publish_native<B: NativeAqlSubmissionBackendV1>(
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

    pub(in super::super) fn terminal_before_native_publication(
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
    pub(in super::super) fn bind_published_target(
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
    pub(in super::super) fn observe_published_target_once<B: NativeCompletionSignalBackendV1>(
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
    pub(in super::super) fn release_after_dependent_completion(
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

    pub(in super::super) fn terminal_after_dependent_completion(
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
    pub(in super::super) fn rollback_retryable_before_side_effect(
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
    pub(super) fn next_epoch(&self) -> Option<u64> {
        self.next_acceptance_epoch
    }
}
