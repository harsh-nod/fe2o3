//! Single and three-binding persistent compute publication.

use super::*;

impl ComputeAqlQueueSessionV1 {
    /// Publishes the exact prepared three-binding persistent-compute attachment.
    #[allow(clippy::result_large_err)]
    pub fn submit_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeDispatchV1,
        Gfx942ThreeBindingPersistentComputeExecutionFailureV1,
    > {
        self.submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
            prepared_receipt,
            |session| {
                session.submit_fixed_dispatch_inner_classified::<1>(
                    FixedDispatchBindingModeV1::ExactPersistentAttachment,
                )
            },
        )
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        prepared_receipt: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
        submit: impl FnOnce(
            &mut Self,
        )
            -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1>,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeDispatchV1,
        Gfx942ThreeBindingPersistentComputeExecutionFailureV1,
    > {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        let mut attachment = self
            .take_three_binding_persistent_compute_attachment_v1()
            .expect("validated three-binding attachment");
        if self.terminal_poisoned {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable: None,
            });
        }
        if attachment
            .entries
            .iter()
            .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Prepared(_)))
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: None,
            });
        }
        let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| submit(self)));
        let submission = match submission {
            Ok(submission) => submission,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match submission {
            Ok(batch) => {
                if !publish_persistent_compute_entries_v1(attachment.entries.each_mut()) {
                    quarantine_persistent_compute_entries_v1(
                        attachment.entries.each_mut(),
                        Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                    );
                    attachment.terminal_custody =
                        Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                    self.set_three_binding_persistent_compute_attachment_v1(attachment);
                    self.poison_terminal();
                    poison_process_global_after_dispatch_terminal_v1();
                    return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "three-binding persistent publication ledger transition",
                        ),
                        retryable: None,
                    });
                }
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                Ok(Gfx942ThreeBindingPersistentComputeDispatchV1 {
                    binding,
                    batch,
                    thread_affinity: PhantomData,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                    error,
                    retryable: Some(Gfx942PreparedThreeBindingPersistentComputeDispatchV1 {
                        binding,
                        thread_affinity: PhantomData,
                    }),
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
        }
    }

    /// Publishes the exact prepared persistent-compute attachment.
    #[allow(clippy::result_large_err)]
    pub fn submit_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeExecutionFailureV1> {
        self.submit_directional_persistent_fixed_dispatch_v1_using(prepared_receipt, |session| {
            session.submit_fixed_dispatch_inner_classified::<1>(
                FixedDispatchBindingModeV1::ExactPersistentAttachment,
            )
        })
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn submit_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
        submit: impl FnOnce(
            &mut Self,
        )
            -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1>,
    ) -> Result<Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeExecutionFailureV1> {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        if self.terminal_poisoned {
            let retryable = (!self.absorb_terminal_prepared_persistent_compute_v1(binding))
                .then_some(prepared_receipt);
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable,
            });
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: Some(prepared_receipt),
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        if !matches!(attachment.state, PersistentComputeUseStateV1::Prepared(_)) {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: None,
            });
        };
        let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| submit(self)));
        let submission = match submission {
            Ok(submission) => submission,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match submission {
            Ok(batch) => {
                if !publish_persistent_compute_entries_v1([&mut attachment]) {
                    quarantine_persistent_compute_entries_v1(
                        [&mut attachment],
                        Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                    );
                    attachment.terminal_custody =
                        Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                    self.set_single_persistent_compute_attachment_v1(attachment);
                    self.poison_terminal();
                    return Err(Gfx942PersistentComputeExecutionFailureV1 {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent compute publication ledger transition",
                        ),
                        retryable: None,
                    });
                }
                self.set_single_persistent_compute_attachment_v1(attachment);
                Ok(Gfx942PersistentComputeDispatchV1 {
                    binding,
                    batch,
                    thread_affinity: PhantomData,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                self.set_single_persistent_compute_attachment_v1(attachment);
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: Some(Gfx942PreparedPersistentComputeDispatchV1 {
                        binding,
                        thread_affinity: PhantomData,
                    }),
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
        }
    }
}
