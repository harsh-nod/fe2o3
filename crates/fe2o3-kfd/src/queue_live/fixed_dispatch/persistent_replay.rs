//! Retained-control replay with original DATA custody.

use super::*;

impl ComputeAqlQueueSessionV1 {
    #[allow(clippy::result_large_err)]
    pub(in super::super) fn terminal_persistent_retained_control_replay_after_detach_v1(
        &mut self,
        mut replay: PersistentRetainedControlReplayDetachedV1,
        custody: PersistentComputeTerminalNativeCustodyV1,
        error: ComputeAqlQueueSessionErrorV1,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Gfx942PersistentComputeBindFailureV1 {
        let disposition = match &custody {
            PersistentComputeTerminalNativeCustodyV1::Storage(_) => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Storage,
                    false,
                    false,
                    false,
                )
            }
            PersistentComputeTerminalNativeCustodyV1::Data(_) => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Data,
                    false,
                    false,
                    false,
                )
            }
            PersistentComputeTerminalNativeCustodyV1::Attached => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Attached,
                    false,
                    false,
                    false,
                )
            }
            _ => unreachable!("replay bind admits only pre-publication custody"),
        };
        debug_assert!(matches!(
            (&custody, disposition),
            (
                PersistentComputeTerminalNativeCustodyV1::Storage(_),
                PersistentRetainedControlReplayDispositionV1::TerminalStorage,
            ) | (
                PersistentComputeTerminalNativeCustodyV1::Data(_),
                PersistentRetainedControlReplayDispositionV1::TerminalData,
            ) | (
                PersistentComputeTerminalNativeCustodyV1::Attached,
                PersistentRetainedControlReplayDispositionV1::TerminalAttached,
            )
        ));
        let state = quarantine_persistent_retained_control_replay_prepared_v1(
            &mut replay.allocation.owner,
            replay.prepared,
        );
        self.dispatch = Some(replay.dispatch);
        self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
            allocation: replay.allocation,
            initialization: replay.initialization,
            state,
            binding: PersistentComputeBindingKeyV1 {
                queue: self.key,
                attachment_generation: commit.attachment_generation,
            },
            storage_identity: commit.storage_identity,
            effect: commit.effect,
            predecessor_dispatch_generation: Some(commit.predecessor_generation),
            terminal_custody: Some(custody),
        });
        self.next_persistent_compute_generation = commit.next_attachment_generation;
        self.poison_terminal();
        Gfx942PersistentComputeBindFailureV1 {
            error,
            custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn finish_persistent_retained_control_replay_before_detach_v1(
        &mut self,
        request: PersistentRetainedControlReplayRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
        loan_succeeded: bool,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Gfx942PersistentComputeBindFailureV1 {
        let PersistentRetainedControlReplayRequestV1 {
            mut input,
            prepared,
            dispatch,
            initialized_content: _,
            control_identity: _,
            predecessor_generation: _,
        } = request;
        self.dispatch = Some(dispatch);
        let cancellation = persistent_compute_input_allocation_mut_v1(&mut input)
            .owner
            .cancel_prepared(prepared);
        let cancellation_succeeded = cancellation.is_ok();
        let disposition = classify_persistent_retained_control_replay_failure_v1(
            PersistentRetainedControlReplayCustodyStageV1::Input,
            loan_succeeded,
            cancellation_succeeded,
            !self.terminal_poisoned,
        );
        match (disposition, cancellation) {
            (PersistentRetainedControlReplayDispositionV1::RetryableInput, Ok(())) => {
                persistent_retained_control_replay_input_failure_v1(error, input, true)
            }
            (PersistentRetainedControlReplayDispositionV1::TerminalInput, Ok(())) => {
                self.poison_terminal();
                persistent_retained_control_replay_input_failure_v1(error, input, false)
            }
            (PersistentRetainedControlReplayDispositionV1::TerminalAttached, Err(failure)) => {
                let (_, prepared) = failure.into_parts();
                let (mut allocation, initialization) = input.into_parts();
                let state = quarantine_persistent_retained_control_replay_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                );
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation,
                    initialization,
                    state,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation: commit.attachment_generation,
                    },
                    storage_identity: commit.storage_identity,
                    effect: commit.effect,
                    predecessor_dispatch_generation: Some(commit.predecessor_generation),
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                });
                self.next_persistent_compute_generation = commit.next_attachment_generation;
                self.poison_terminal();
                Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                    ),
                }
            }
            _ => unreachable!("replay failure disposition matches exact cancellation custody"),
        }
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn bind_retained_persistent_fixed_dispatch_control_replay_v1(
        &mut self,
        request: PersistentRetainedControlReplayRequestV1,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Result<Gfx942PreparedPersistentComputeDispatchV1, Gfx942PersistentComputeBindFailureV1>
    {
        let mut request = Some(request);
        let mut phases = PersistentRetainedControlReplayCustodyV1::Empty;
        let mut pipeline_result = None;
        let fused_loan = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model(|memory| {
                phases = PersistentRetainedControlReplayCustodyV1::Input(
                    request.take().expect("one replay input before opening"),
                );
                pipeline_result = Some(execute_persistent_retained_control_replay_pipeline_v1(
                    memory,
                    &mut phases,
                    |memory, phases| phases.mapped_facts(memory),
                    |_, phases| phases.detach(),
                    |_, phases| phases.construct(),
                    |memory, phases| phases.retain(memory),
                    |memory, phases| phases.audit(memory),
                ));
                Ok(())
            })
        }));
        let fused_loan = match fused_loan {
            Ok(result) => result,
            Err(payload) => {
                if let Some(request) = request.take() {
                    phases = PersistentRetainedControlReplayCustodyV1::Input(request);
                }
                let error = ComputeAqlQueueSessionErrorV1::Contract("persistent replay panicked");
                match phases.into_bind_outcome(Err(error)) {
                    PersistentRetainedControlReplayOutcomeV1::BeforeDetach { request, .. } => {
                        let (mut allocation, initialization) = request.input.into_parts();
                        let state = quarantine_persistent_retained_control_replay_prepared_v1(
                            &mut allocation.owner,
                            request.prepared,
                        );
                        self.dispatch = Some(request.dispatch);
                        self.set_single_persistent_compute_attachment_v1(
                            PersistentComputeAttachmentV1 {
                                allocation,
                                initialization,
                                state,
                                binding: PersistentComputeBindingKeyV1 {
                                    queue: self.key,
                                    attachment_generation: commit.attachment_generation,
                                },
                                storage_identity: commit.storage_identity,
                                effect: commit.effect,
                                predecessor_dispatch_generation: Some(
                                    commit.predecessor_generation,
                                ),
                                terminal_custody: Some(
                                    PersistentComputeTerminalNativeCustodyV1::Attached,
                                ),
                            },
                        );
                        self.next_persistent_compute_generation = commit.next_attachment_generation;
                    }
                    PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                        replay,
                        custody,
                        error,
                    } => {
                        let _ = self.terminal_persistent_retained_control_replay_after_detach_v1(
                            replay, custody, error, commit,
                        );
                    }
                    PersistentRetainedControlReplayOutcomeV1::Ready(_) => {
                        unreachable!("panic is not completion")
                    }
                }
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let outcome = pipeline_result.map(|result| phases.into_bind_outcome(result));

        let (outcome, loan_error) = match resolve_persistent_retained_control_replay_loan_v1(
            request,
            outcome,
            fused_loan,
            || {
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent replay foundation loan did not execute",
                )
            },
        ) {
            PersistentRetainedControlReplayLoanResolutionV1::Unopened { request, error } => {
                return Err(
                    self.finish_persistent_retained_control_replay_before_detach_v1(
                        request, error, false, commit,
                    ),
                );
            }
            PersistentRetainedControlReplayLoanResolutionV1::Executed {
                outcome,
                retake_error,
            } => (outcome, retake_error),
        };
        let loan_succeeded = loan_error.is_none();
        match outcome {
            PersistentRetainedControlReplayOutcomeV1::BeforeDetach { request, error } => Err(self
                .finish_persistent_retained_control_replay_before_detach_v1(
                    request,
                    loan_error.unwrap_or(error),
                    loan_succeeded,
                    commit,
                )),
            PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                replay,
                custody,
                error,
            } => Err(
                self.terminal_persistent_retained_control_replay_after_detach_v1(
                    replay,
                    custody,
                    loan_error.unwrap_or(error),
                    commit,
                ),
            ),
            PersistentRetainedControlReplayOutcomeV1::Ready(replay) => {
                if let Some(error) = loan_error {
                    return Err(
                        self.terminal_persistent_retained_control_replay_after_detach_v1(
                            replay,
                            PersistentComputeTerminalNativeCustodyV1::Attached,
                            error,
                            commit,
                        ),
                    );
                }
                let binding = PersistentComputeBindingKeyV1 {
                    queue: self.key,
                    attachment_generation: commit.attachment_generation,
                };
                self.dispatch = Some(replay.dispatch);
                self.detached_data_count = 0;
                self.detached_dispatch_generation = None;
                self.detached_data_identities.clear();
                self.detached_next_insertion_index = None;
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation: replay.allocation,
                    initialization: replay.initialization,
                    state: PersistentComputeUseStateV1::Prepared(replay.prepared),
                    binding,
                    storage_identity: commit.storage_identity,
                    effect: commit.effect,
                    predecessor_dispatch_generation: Some(commit.predecessor_generation),
                    terminal_custody: None,
                });
                self.next_persistent_compute_generation = commit.next_attachment_generation;
                Ok(Gfx942PreparedPersistentComputeDispatchV1 {
                    binding,
                    thread_affinity: PhantomData,
                })
            }
        }
    }
}
