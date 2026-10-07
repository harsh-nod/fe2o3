//! Persistent compute recycling and bounded waits.

use super::*;

impl ComputeAqlQueueSessionV1 {
    /// Polls one published persistent-compute dispatch, retaining all custody
    /// in either returned typestate.
    #[allow(clippy::result_large_err)]
    pub fn poll_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputePollV1, Gfx942PersistentComputePollFailureV1> {
        let transition = self.poll_directional_persistent_fixed_dispatch_inner_v1(
            dispatch_receipt,
            |session, completion| {
                session
                    .poll_completion_batch_with_progress_retaining(completion)
                    .map(|poll| match poll {
                        Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                            PersistentComputeCompletionObservationV1::Pending(batch)
                        }
                        Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                            PersistentComputeCompletionObservationV1::Ready(completed)
                        }
                    })
            },
            |completed| completed,
        );
        let transition = self.terminalize_persistent_compute_poll_result_v1(transition)?;
        match transition {
            PersistentComputePollTransitionV1::Pending(dispatch) => {
                Ok(Gfx942PersistentComputePollV1::Pending(dispatch))
            }
            PersistentComputePollTransitionV1::Ready(PersistentComputeCompletedTransitionV1 {
                binding,
                mut attachment,
                completed_use,
                identity,
                completion_occurrence: _,
                completed,
            }) => {
                attachment.state = PersistentComputeUseStateV1::Completed(completed_use);
                self.set_single_persistent_compute_attachment_v1(attachment);
                Ok(Gfx942PersistentComputePollV1::Ready(
                    Gfx942CompletedPersistentComputeDispatchV1 {
                        binding,
                        completed: wrap_completed(completed, identity),
                        thread_affinity: PhantomData,
                    },
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(in super::super) fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1<
        Completed,
    >(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        mut attachment: PersistentComputeAttachmentV1,
        completed_use: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        identity: DispatchEpochIdentityV1,
        completion_occurrence: super::super::completion::CompletionBatchOccurrenceV1,
        completed: Completed,
        recycle: impl FnOnce(
            &mut Self,
            Completed,
        ) -> Result<
            Gfx942CompletionRecycleObservationV1,
            (ComputeAqlQueueSessionErrorV1, Completed),
        >,
        into_completed: impl FnOnce(Completed) -> Gfx942CompletedBatchV1<1>,
    ) -> Result<Gfx942RecycledPersistentComputeDispatchV1, Gfx942PersistentComputeRecycleFailureV1>
    {
        attachment.state = PersistentComputeUseStateV1::Completed(completed_use);
        let recycled =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| recycle(self, completed)));
        let recycled = match recycled {
            Ok(recycled) => recycled,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let recycle = match recycled {
            Ok(recycle) => recycle,
            Err((error, completed)) => {
                let completed = wrap_completed(into_completed(completed), identity);
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = Some(
                    PersistentComputeTerminalNativeCustodyV1::Completed(completed),
                );
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("persistent dispatch retained")
            .mark_recycled_occurrence(identity, completion_occurrence)
            .is_err()
        {
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        if !recycle_persistent_compute_entries_v1([&mut attachment]) {
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: None,
            });
        }
        self.set_single_persistent_compute_attachment_v1(attachment);
        Ok(Gfx942RecycledPersistentComputeDispatchV1 {
            binding,
            recycle,
            thread_affinity: PhantomData,
        })
    }

    /// Polls one published persistent-compute dispatch and, on Ready, recycles
    /// its exact completion signal without reopening the just-closed
    /// currentness envelope. Pending preserves the ordinary two-check poll.
    #[allow(clippy::result_large_err)]
    pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
    ) -> Result<
        Gfx942PersistentComputePollAndRecycleV1,
        Gfx942PersistentComputePollAndRecycleFailureV1,
    > {
        let transition = execute_persistent_compute_poll_and_recycle_v1(
            self,
            |session| {
                session.poll_directional_persistent_fixed_dispatch_inner_v1(
                    dispatch_receipt,
                    |session, completion| {
                        session
                            .poll_completion_batch_with_current_handoff_retaining(completion)
                            .map(|poll| match poll {
                                CompletionPollWithCurrentnessHandoffV1::Pending {
                                    batch, ..
                                } => PersistentComputeCompletionObservationV1::Pending(batch),
                                CompletionPollWithCurrentnessHandoffV1::Ready {
                                    handoff, ..
                                } => PersistentComputeCompletionObservationV1::Ready(handoff),
                            })
                    },
                    CompletionCurrentnessHandoffV1::into_completed,
                )
            },
            |_| Instant::now(),
            |session, completed| {
                let PersistentComputeCompletedTransitionV1 {
                    binding,
                    attachment,
                    completed_use,
                    identity,
                    completion_occurrence,
                    completed: handoff,
                } = completed;
                session.finish_directional_persistent_fixed_dispatch_recycle_inner_v1(
                    binding,
                    attachment,
                    completed_use,
                    identity,
                    completion_occurrence,
                    handoff,
                    Self::recycle_completion_current_handoff_retaining,
                    CompletionCurrentnessHandoffV1::into_completed,
                )
            },
        );
        match transition {
            Ok(PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)) => {
                Ok(Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch))
            }
            Ok(PersistentComputePollAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
            }) => Ok(Gfx942PersistentComputePollAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
            }),
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => {
                let failure = self
                    .terminalize_persistent_compute_poll_result_v1::<()>(Err(failure))
                    .expect_err("poll transition already failed");
                Err(Gfx942PersistentComputePollAndRecycleFailureV1::Poll(
                    failure,
                ))
            }
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                let failure = self
                    .terminalize_persistent_compute_recycle_result_v1::<()>(Err(failure))
                    .expect_err("recycle transition already failed");
                Err(Gfx942PersistentComputePollAndRecycleFailureV1::Recycle(
                    failure,
                ))
            }
        }
    }

    /// Waits until one published persistent-compute dispatch completes or the
    /// monotonic deadline expires, recycling its signal immediately on Ready.
    ///
    /// The first completion observation is unconditional, including when
    /// `deadline` has already elapsed. A clean timeout returns the exact
    /// Published dispatch without resetting its signal or advancing either
    /// retirement ledger.
    #[allow(clippy::result_large_err)]
    pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
        deadline: Instant,
    ) -> Result<
        Gfx942PersistentComputeWaitAndRecycleV1,
        Gfx942PersistentComputePollAndRecycleFailureV1,
    > {
        let mut wait = MonotonicWaitV1::until(deadline);
        let transition = execute_persistent_compute_wait_and_recycle_v1(
            self,
            dispatch_receipt,
            |session, dispatch| {
                session
                    .poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch)
                    .map(|transition| match transition {
                        Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch) => {
                            PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)
                        }
                        Gfx942PersistentComputePollAndRecycleV1::Recycled {
                            recycled,
                            completion_observed_at,
                        } => PersistentComputePollAndRecycleTransitionV1::Recycled {
                            recycled,
                            completion_observed_at,
                        },
                    })
            },
            |_| {
                if wait.expired() {
                    true
                } else {
                    wait.pause();
                    wait.expired()
                }
            },
        )?;
        Ok(match transition {
            PersistentComputeWaitAndRecycleTransitionV1::Timeout {
                pending: dispatch,
                observations,
            } => Gfx942PersistentComputeWaitAndRecycleV1::Timeout {
                dispatch,
                observations,
            },
            PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            } => Gfx942PersistentComputeWaitAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            },
        })
    }

    /// Waits for the exact three-binding dispatch until a monotonic deadline,
    /// preserving Published custody on a clean timeout.
    #[allow(clippy::result_large_err)]
    pub fn wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
        deadline: Instant,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        self.wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1_using(
            dispatch_receipt,
            deadline,
            |session, dispatch| {
                session.poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(
                    dispatch,
                )
            },
        )
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1_using(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
        deadline: Instant,
        mut poll: impl FnMut(
            &mut Self,
            Gfx942ThreeBindingPersistentComputeDispatchV1,
        ) -> Result<
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
            Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
        >,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        let mut wait = MonotonicWaitV1::until(deadline);
        let transition = execute_persistent_compute_wait_and_recycle_v1(
            self,
            dispatch_receipt,
            |session, dispatch| {
                poll(session, dispatch).map(|transition| match transition {
                    Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(dispatch) => {
                        PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)
                    }
                    Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                        recycled,
                        completion_observed_at,
                    } => PersistentComputePollAndRecycleTransitionV1::Recycled {
                        recycled,
                        completion_observed_at,
                    },
                })
            },
            |_| {
                if wait.expired() {
                    true
                } else {
                    wait.pause();
                    wait.expired()
                }
            },
        )?;
        Ok(match transition {
            PersistentComputeWaitAndRecycleTransitionV1::Timeout {
                pending: dispatch,
                observations,
            } => Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Timeout {
                dispatch,
                observations,
            },
            PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            } => Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            },
        })
    }

    /// Recycles the exact completion signal after device completion.
    #[allow(clippy::result_large_err)]
    pub fn recycle_directional_persistent_fixed_dispatch_v1(
        &mut self,
        completed_receipt: Gfx942CompletedPersistentComputeDispatchV1,
    ) -> Result<Gfx942RecycledPersistentComputeDispatchV1, Gfx942PersistentComputeRecycleFailureV1>
    {
        let binding = completed_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(completed_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self.absorb_terminal_completed_persistent_compute_v1(
                binding,
                completed_receipt.completed,
            ) {
                Ok(()) => Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(completed) => Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942CompletedPersistentComputeDispatchV1 {
                        binding,
                        completed,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(completed_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return self.terminalize_persistent_compute_recycle_result_v1(Err(
                Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                    recovered: None,
                    retained: Some(PersistentComputeTerminalNativeCustodyV1::Completed(
                        completed_receipt.completed,
                    )),
                },
            ));
        };
        let (completion, identity) = unwrap_completed(completed_receipt.completed);
        let completion_occurrence = completion.occurrence_v1();
        let generation_is_current = self
            .dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.validate_completed(identity, &completion).is_ok());
        if !generation_is_current {
            let completed = wrap_completed(completion, identity);
            attachment.state = quarantine_persistent_compute_completed_v1(
                &mut attachment.allocation.owner,
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return self.terminalize_persistent_compute_recycle_result_v1(Err(
                Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                    recovered: None,
                    retained: None,
                },
            ));
        }
        let result = self.finish_directional_persistent_fixed_dispatch_recycle_inner_v1(
            binding,
            attachment,
            completed_use,
            identity,
            completion_occurrence.expect("validated completion occurrence"),
            completion,
            Self::recycle_completion_batch_retaining,
            |completed| completed,
        );
        self.terminalize_persistent_compute_recycle_result_v1(result)
    }
}
