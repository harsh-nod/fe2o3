//! Same-device persistent SDMA window submission and completion.

use super::*;

impl ComputeAqlQueueSessionV1 {
    /// Publishes one same-device D2D copy from two distinct persistent owners.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn submit_same_device_persistent_sdma_window_v1(
        &mut self,
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        source_offset: u64,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1,
    > {
        let retryable =
            |error, source, destination| Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    source,
                    destination,
                },
            };
        let (mut source, mut destination, descriptor) =
            admit_same_device_persistent_sdma_window_input_v1(
                self.key,
                self.terminal_poisoned,
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            )?;
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.has_any_persistent_compute_attachment_v1(),
            SdmaPublicationModeV1::SameDeviceWindow,
        ) {
            return Err(retryable(error.into(), source, destination));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(retryable(error, source, destination));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&source.attachment)
            || !self.directional_persistent_sdma_attachment_is_current(&destination.attachment)
        {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "same-device persistent SDMA queue-pair attachment changed",
                ),
                source,
                destination,
            ));
        }
        if let Err(error) = self.check_directional_persistent_sdma_operational_currentness() {
            source
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            destination
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            self.poison_terminal();
            return Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                        source_sequence: None,
                        destination_sequence: None,
                        descriptor,
                        state:
                            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::AdmissionRestored {
                                source,
                                destination,
                            },
                    },
                ),
            });
        }

        let source_request = match same_device_source_use_request_v1(source_offset, copy_bytes) {
            Ok(request) => request,
            Err(error) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let destination_request =
            match same_device_destination_use_request_v1(destination_offset, copy_bytes) {
                Ok(request) => request,
                Err(error) => {
                    return Err(retryable(
                        map_directional_persistent_sdma_use_error_v1(error),
                        source,
                        destination,
                    ));
                }
            };
        let source_reserved = match source.owner.reserve(source_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    source,
                    destination,
                ));
            }
        };
        let destination_reserved = match destination.owner.reserve(destination_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                source
                    .owner
                    .cancel_reserved(source_reserved)
                    .expect("private source reservation must cancel");
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    source,
                    destination,
                ));
            }
        };
        let source_prepared = match source.owner.prepare(source_reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, source_reserved) = failure.into_parts();
                source
                    .owner
                    .cancel_reserved(source_reserved)
                    .expect("private source reservation must cancel");
                destination
                    .owner
                    .cancel_reserved(destination_reserved)
                    .expect("private destination reservation must cancel");
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let destination_prepared = match destination.owner.prepare(destination_reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, destination_reserved) = failure.into_parts();
                source
                    .owner
                    .cancel_prepared(source_prepared)
                    .expect("private source preparation must cancel");
                destination
                    .owner
                    .cancel_reserved(destination_reserved)
                    .expect("private destination reservation must cancel");
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let (source_buffer, destination_buffer) = match detach_sdma_buffer_pair_v1(
            &mut source.owner,
            (
                source.attachment.queue,
                source.attachment.pool_generation,
                source.attachment.logical_bytes,
            ),
            &mut destination.owner,
            (
                destination.attachment.queue,
                destination.attachment.pool_generation,
                destination.attachment.logical_bytes,
            ),
        ) {
            Ok(leases) => leases,
            Err(error) => {
                cancel_prepared_local_sdma_pair_v1(
                    &mut source.owner,
                    source_prepared,
                    &mut destination.owner,
                    destination_prepared,
                )
                .unwrap_or_else(
                    crate::persistent_same_device_sdma::abort_paired_lease_contradiction_v1,
                );
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let request = same_device_persistent_sdma_request_v1(
            source_buffer,
            source_offset,
            destination_buffer,
            destination_offset,
            copy_bytes,
        );

        let mut request = Some(request);
        let mut preparation = None;
        let prepare_operation = self.with_sdma_owner_memory(|owner, memory| {
            preparation = Some(owner.prepare_same_device_persistent_window_recoverable(
                memory,
                request.take().expect("same-device request consumed once"),
            ));
            Ok(())
        });
        let preparation = preparation.unwrap_or_else(|| {
            Err((
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA preparation did not execute",
                ),
                request.expect("unexecuted same-device preparation retains request"),
            ))
        });
        let closing_prepare = self.check_directional_persistent_sdma_operational_currentness();
        let owner_poisoned = self
            .sdma
            .as_ref()
            .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
        let preparation_terminal = prepare_operation.is_err()
            || closing_prepare.is_err()
            || (preparation.is_err() && owner_poisoned);
        let prepared_lower = match preparation {
            Ok(prepared) if !preparation_terminal => prepared,
            Ok(prepared) => {
                return Err(
                    self.terminal_prepared_same_device_persistent_sdma_window_failure(
                        prepare_operation
                            .err()
                            .or_else(|| closing_prepare.err())
                            .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                                "same-device persistent SDMA preparation poisoned its queue",
                            )),
                        source,
                        source_prepared,
                        destination,
                        destination_prepared,
                        descriptor,
                        prepared.into_request(),
                    ),
                );
            }
            Err((error, request)) if !preparation_terminal => {
                let (mut source, mut destination) = restore_same_device_persistent_sdma_request_v1(
                    source,
                    destination,
                    descriptor,
                    request,
                )
                .unwrap_or_else(|_| unreachable!("exact same-device request must restore"));
                cancel_prepared_local_sdma_pair_v1(
                    &mut source.owner,
                    source_prepared,
                    &mut destination.owner,
                    destination_prepared,
                )
                .unwrap_or_else(
                    crate::persistent_same_device_sdma::abort_paired_lease_contradiction_v1,
                );
                return Err(retryable(error.into(), source, destination));
            }
            Err((error, request)) => {
                return Err(
                    self.terminal_prepared_same_device_persistent_sdma_window_failure(
                        prepare_operation
                            .err()
                            .or_else(|| closing_prepare.err())
                            .unwrap_or_else(|| error.into()),
                        source,
                        source_prepared,
                        destination,
                        destination_prepared,
                        descriptor,
                        request,
                    ),
                );
            }
        };

        let mut planned_tickets = Vec::new();
        if planned_tickets
            .try_reserve_exact(prepared_lower.tickets().len())
            .is_err()
        {
            let request = prepared_lower.into_request();
            let (mut source, mut destination) = restore_same_device_persistent_sdma_request_v1(
                source,
                destination,
                descriptor,
                request,
            )
            .unwrap_or_else(|_| unreachable!("exact same-device request must restore"));
            cancel_prepared_local_sdma_pair_v1(
                &mut source.owner,
                source_prepared,
                &mut destination.owner,
                destination_prepared,
            )
            .unwrap_or_else(
                crate::persistent_same_device_sdma::abort_paired_lease_contradiction_v1,
            );
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "same-device persistent SDMA planned ticket allocation",
                ),
                source,
                destination,
            ));
        }
        planned_tickets.extend_from_slice(prepared_lower.tickets());
        let mut prepared_lower = Some(prepared_lower);
        let mut publication = None;
        let publication_operation = self.with_sdma_owner_memory(|owner, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            publication = Some(
                owner.submit_prepared_persistent_window_with_custody(
                    memory,
                    prepared_lower
                        .take()
                        .expect("same-device preparation consumed once"),
                ),
            );
            Ok(())
        });
        if publication.is_none() {
            return Err(
                self.terminal_prepared_same_device_persistent_sdma_window_failure(
                    publication_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA publication did not execute",
                        )),
                    source,
                    source_prepared,
                    destination,
                    destination_prepared,
                    descriptor,
                    prepared_lower
                        .expect("unexecuted publication retains preparation")
                        .into_request(),
                ),
            );
        }
        let (observation, lower_error) = match publication.expect("publication outcome stored") {
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable {
                error,
                prepared,
            }) => (
                SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(
                    prepared.into_request(),
                ),
                error,
            ),
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Retained { error, tickets }) => (
                SameDevicePersistentSdmaWindowPublicationObservationV1::Retained(tickets),
                error,
            ),
            Ok(tickets) => (
                SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA post-publication currentness",
                ),
            ),
        };
        let closing = self.check_directional_persistent_sdma_operational_currentness();
        let transition = transition_same_device_persistent_sdma_window_publication_v1(
            SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                source,
                source_prepared,
                destination,
                destination_prepared,
                planned_tickets,
                descriptor,
            },
            observation,
            publication_operation.is_ok(),
            closing.is_ok(),
        );
        self.finish_same_device_persistent_sdma_window_publication_transition(
            publication_operation
                .err()
                .or_else(|| closing.err())
                .unwrap_or_else(|| lower_error.into()),
            transition,
        )
    }

    #[allow(clippy::result_large_err)]
    pub fn poll_same_device_persistent_sdma_window_v1(
        &mut self,
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowCopyPollV1,
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.source.attachment.queue != self.key
            || submission.destination.attachment.queue != self.key
        {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign same-device persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&submission.source.attachment)
            || !self.directional_persistent_sdma_attachment_is_current(
                &submission.destination.attachment,
            )
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission.source.attachment.pair.host_to_device_queue_id;
        if submission.source.attachment.pair != submission.destination.attachment.pair
            || submission.tickets.len() != submission.packet_count()
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.source.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut poll_result = None;
        let poll_operation = self.with_sdma_owner_memory(|owner, memory| {
            poll_result = Some(owner.poll_persistent_window(memory, &submission.tickets));
            Ok(())
        });
        let Some(poll_result) = poll_result else {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    poll_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA poll did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match poll_result {
            Ok(PersistentSdmaWindowPollV1::Pending) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Pending,
                None,
            ),
            Err(error) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(PersistentSdmaWindowPollV1::Completed(completed)) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            observation,
            poll_operation.is_ok(),
        ) {
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(submission) => Ok(
                Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Pending(submission),
            ),
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) => Ok(
                Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Completed(completed),
            ),
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(_) => {
                unreachable!("same-device poll cannot produce timeout custody")
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_same_device_persistent_sdma_window_execution_transition(
                    poll_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn wait_same_device_persistent_sdma_window_for_v1(
        &mut self,
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        timeout: Duration,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowCompletedV1,
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.source.attachment.queue != self.key
            || submission.destination.attachment.queue != self.key
        {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign same-device persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&submission.source.attachment)
            || !self.directional_persistent_sdma_attachment_is_current(
                &submission.destination.attachment,
            )
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission.source.attachment.pair.host_to_device_queue_id;
        if submission.source.attachment.pair != submission.destination.attachment.pair
            || submission.tickets.len() != submission.packet_count()
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.source.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut wait_result = None;
        let wait_operation = self.with_sdma_owner_memory(|owner, memory| {
            wait_result = Some(owner.wait_persistent_window_for(
                memory,
                &submission.tickets,
                timeout,
                SdmaWaitProfileV1::PersistentElapsedSpinFloor(PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1),
            ));
            Ok(())
        });
        let Some(wait_result) = wait_result else {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    wait_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA wait did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match wait_result {
            Err(Gfx942SdmaErrorV1::Timeout) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Timeout,
                None,
            ),
            Err(error) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(completed) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            observation,
            wait_operation.is_ok(),
        ) {
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(submission) => {
                Err(pending(
                    ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    submission,
                ))
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) => {
                Ok(completed)
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(_) => {
                unreachable!("same-device wait cannot produce pending custody")
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_same_device_persistent_sdma_window_execution_transition(
                    wait_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }
}
