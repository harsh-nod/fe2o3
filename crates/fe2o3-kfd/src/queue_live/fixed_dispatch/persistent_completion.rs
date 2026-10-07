//! Persistent compute cancellation and completion transitions.

use super::*;

impl ComputeAqlQueueSessionV1 {
    #[allow(clippy::result_large_err)]
    pub(in super::super) fn terminalize_persistent_compute_poll_result_v1<T>(
        &mut self,
        result: Result<T, Gfx942PersistentComputePollFailureV1>,
    ) -> Result<T, Gfx942PersistentComputePollFailureV1> {
        if result
            .as_ref()
            .is_err_and(|failure| failure.recovered.is_none())
        {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn terminalize_persistent_compute_recycle_result_v1<T>(
        &mut self,
        result: Result<T, Gfx942PersistentComputeRecycleFailureV1>,
    ) -> Result<T, Gfx942PersistentComputeRecycleFailureV1> {
        if result
            .as_ref()
            .is_err_and(|failure| failure.recovered.is_none())
        {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    /// Cancels an exact prepared attachment before publication and restores
    /// the original initialized or uninitialized persistent input.
    #[allow(clippy::result_large_err)]
    pub fn cancel_prepared_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeInputV1, Gfx942PersistentComputeCancelFailureV1> {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            let recovered = (!self.absorb_terminal_prepared_persistent_compute_v1(binding))
                .then_some(prepared_receipt);
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                recovered,
                retained: None,
            });
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(prepared_receipt),
                retained: None,
            });
        }
        let mut inputs = persistent_cancel::settle_persistent_cancel_v1(
            self,
            persistent_cancel::CancelShapeV1::Single,
        )
        .map_err(|error| Gfx942PersistentComputeCancelFailureV1 {
            error,
            recovered: None,
            retained: None,
        })?;
        Ok(inputs.pop().expect("settled one-input cancellation"))
    }

    /// Cancels the exact prepared three-binding attachment before publication
    /// and restores all three original persistent inputs atomically.
    #[allow(clippy::result_large_err)]
    pub fn cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeInputsV1,
        Gfx942ThreeBindingPersistentComputeCancelFailureV1,
    > {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
            });
        }
        if self.terminal_poisoned {
            let attachment = self
                .persistent_compute
                .as_mut()
                .expect("validated attachment");
            if attachment.terminal_custody.is_none() {
                for entry in &mut attachment.entries {
                    quarantine_persistent_compute_entries_v1(
                        [entry],
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    );
                }
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            }
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }
        let inputs = persistent_cancel::settle_persistent_cancel_v1(
            self,
            persistent_cancel::CancelShapeV1::Three,
        )
        .map_err(
            |error| Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error,
                recovered: None,
            },
        )?;
        Ok(Gfx942ThreeBindingPersistentComputeInputsV1::new(
            inputs
                .into_inner()
                .unwrap_or_else(|_| unreachable!("settled three-input cancellation")),
        ))
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn poll_directional_persistent_fixed_dispatch_inner_v1<Completed>(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
        observe: impl FnOnce(
            &mut Self,
            Gfx942CompletionBatchV1<1>,
        ) -> Result<
            PersistentComputeCompletionObservationV1<Completed>,
            (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<1>),
        >,
        into_completed: impl FnOnce(Completed) -> Gfx942CompletedBatchV1<1>,
    ) -> Result<
        PersistentComputePollTransitionV1<
            Gfx942PersistentComputeDispatchV1,
            PersistentComputeCompletedTransitionV1<Completed>,
        >,
        Gfx942PersistentComputePollFailureV1,
    > {
        let binding = dispatch_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self
                .absorb_terminal_published_persistent_compute_v1(binding, dispatch_receipt.batch)
            {
                Ok(()) => Err(Gfx942PersistentComputePollFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(batch) => Err(Gfx942PersistentComputePollFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch,
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
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(dispatch_receipt),
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
        let PersistentComputeUseStateV1::Published(published) = state else {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: Some(PersistentComputeTerminalNativeCustodyV1::Published(
                    dispatch_receipt.batch,
                )),
            });
        };
        let (completion, identity) = unwrap_published(dispatch_receipt.batch);
        let completion_occurrence = completion.occurrence_v1();
        let generation_is_current = self
            .dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.validate_published(identity, &completion).is_ok());
        if !generation_is_current {
            let batch = wrap_published(completion, identity);
            attachment.state = PersistentComputeUseStateV1::Published(published);
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        let completion_occurrence = completion_occurrence.expect("validated completion identity");
        let observed =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(self, completion)));
        let observed = match observed {
            Ok(observed) => observed,
            Err(payload) => {
                attachment.state = PersistentComputeUseStateV1::Published(published);
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
        let completed = match observed {
            Ok(PersistentComputeCompletionObservationV1::Pending(batch)) => {
                attachment.state = PersistentComputeUseStateV1::Published(published);
                self.set_single_persistent_compute_attachment_v1(attachment);
                return Ok(PersistentComputePollTransitionV1::Pending(
                    Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch: wrap_published(batch, identity),
                        thread_affinity: PhantomData,
                    },
                ));
            }
            Ok(PersistentComputeCompletionObservationV1::Ready(completed)) => completed,
            Err((error, completion)) => {
                let batch = wrap_published(completion, identity);
                attachment.state = PersistentComputeUseStateV1::Published(published);
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputePollFailureV1 {
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
            .mark_completed_occurrence(identity, completion_occurrence)
            .is_err()
        {
            let completed = wrap_completed(into_completed(completed), identity);
            attachment.state = PersistentComputeUseStateV1::Published(published);
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        attachment.state = PersistentComputeUseStateV1::Published(published);
        if !complete_persistent_compute_entries_v1([&mut attachment]) {
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
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute completion ledger transition",
                ),
                recovered: None,
                retained: None,
            });
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            unreachable!("shared completion transition produced completed state")
        };
        Ok(PersistentComputePollTransitionV1::Ready(
            PersistentComputeCompletedTransitionV1 {
                binding,
                attachment,
                completed_use,
                identity,
                completion_occurrence,
                completed,
            },
        ))
    }

    /// Polls and immediately recycles the exact three-binding dispatch.
    #[allow(clippy::result_large_err)]
    pub fn poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        self.poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
            dispatch_receipt,
            |session, identity, completion| {
                session.dispatch.as_ref().is_some_and(|dispatch| {
                    dispatch.validate_published(identity, completion).is_ok()
                })
            },
            |session, completion| {
                session.poll_completion_batch_with_current_handoff_retaining(completion)
            },
        )
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
        validate_dispatch: impl FnOnce(
            &mut Self,
            DispatchEpochIdentityV1,
            &Gfx942CompletionBatchV1<1>,
        ) -> bool,
        observe_completion: impl FnOnce(
            &mut Self,
            Gfx942CompletionBatchV1<1>,
        ) -> Result<
            CompletionPollWithCurrentnessHandoffV1<1>,
            (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<1>),
        >,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        let binding = dispatch_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
            });
        }
        let mut attachment = self
            .take_three_binding_persistent_compute_attachment_v1()
            .expect("validated three-binding published attachment");
        if self.terminal_poisoned {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Published(dispatch_receipt.batch),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                recovered: None,
            });
        }
        if attachment
            .entries
            .iter()
            .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Published(dispatch_receipt.batch),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }
        let (completion, identity) = unwrap_published(dispatch_receipt.batch);
        let completion_occurrence = completion.occurrence_v1();
        if !validate_dispatch(self, identity, &completion) {
            let batch = wrap_published(completion, identity);
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
            });
        }
        let completion_occurrence = completion_occurrence.expect("validated completion occurrence");
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observe_completion(self, completion)
        }));
        let observed = match observed {
            Ok(observed) => observed,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let handoff = match observed {
            Ok(CompletionPollWithCurrentnessHandoffV1::Pending { batch, .. }) => {
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                return Ok(
                    Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(
                        Gfx942ThreeBindingPersistentComputeDispatchV1 {
                            binding,
                            batch: wrap_published(batch, identity),
                            thread_affinity: PhantomData,
                        },
                    ),
                );
            }
            Ok(CompletionPollWithCurrentnessHandoffV1::Ready { handoff, .. }) => handoff,
            Err((error, completion)) => {
                let batch = wrap_published(completion, identity);
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                    error,
                    recovered: None,
                });
            }
        };
        let completion_observed_at = Instant::now();
        if self
            .dispatch
            .as_mut()
            .expect("three-binding persistent dispatch retained")
            .mark_completed_occurrence(identity, completion_occurrence)
            .is_err()
        {
            let completed = wrap_completed(handoff.into_completed(), identity);
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
            });
        }
        if !complete_persistent_compute_entries_v1(attachment.entries.each_mut()) {
            let completed = wrap_completed(handoff.into_completed(), identity);
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent completion ledger transition",
                ),
                recovered: None,
            });
        }
        let recycled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::recycle_completion_current_handoff_retaining(self, handoff)
        }));
        let recycle = match recycled {
            Ok(Ok(recycle)) => recycle,
            Ok(Err((error, handoff))) => {
                let completed = wrap_completed(handoff.into_completed(), identity);
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = Some(
                    PersistentComputeTerminalNativeCustodyV1::Completed(completed),
                );
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                    error,
                    recovered: None,
                });
            }
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("three-binding persistent dispatch retained")
            .mark_recycled_occurrence(identity, completion_occurrence)
            .is_err()
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
            });
        }
        if !recycle_persistent_compute_entries_v1(attachment.entries.each_mut()) {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }
        self.set_three_binding_persistent_compute_attachment_v1(attachment);
        Ok(
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                recycled: Gfx942RecycledThreeBindingPersistentComputeDispatchV1 {
                    binding,
                    recycle,
                    thread_affinity: PhantomData,
                },
                completion_observed_at,
            },
        )
    }
}
