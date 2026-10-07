//! Directional persistent SDMA window submission and completion.

use super::*;

impl ComputeAqlQueueSessionV1 {
    /// Publishes one aggregate directional copy as a bounded packet window.
    ///
    /// The window owns one host/device pair and one persistent use lease. All
    /// packet slots are prepared before one write-pointer publication and one
    /// doorbell store make the complete window visible to the device.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn submit_directional_persistent_sdma_window_v1(
        &mut self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1,
    > {
        let retryable =
            |error, allocation, host| Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                },
            };
        let (allocation, host, packet_count) = admit_directional_persistent_sdma_window_input_v1(
            self.key,
            self.terminal_poisoned,
            allocation,
            direction,
            host,
            host_offset,
            device_offset,
            copy_bytes,
        )?;
        if !self.directional_sdma_coexists_with_persistent_compute_v1(&allocation, &host) {
            return Err(retryable(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                allocation,
                host,
            ));
        }
        let mut allocation = allocation;
        if let Err(error) = self.require_sdma_enabled() {
            return Err(retryable(error, allocation, host));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA window queue-pair attachment changed",
                ),
                allocation,
                host,
            ));
        }
        let mut planned_tickets = Vec::new();
        if planned_tickets.try_reserve_exact(packet_count).is_err() {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA planned ticket roster allocation",
                ),
                allocation,
                host,
            ));
        }
        if let Err(error) = self.check_directional_persistent_sdma_operational_currentness() {
            allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            self.poison_terminal();
            return Err(Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody:
                    Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                        Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                            direction,
                            sequence: None,
                            packet_count,
                            state: Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::AdmissionRestored {
                                allocation,
                                host,
                            },
                        },
                    ),
            });
        }

        let host_binding = Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, self.key);
        let operation = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let use_request = match Gfx942PersistentUseRequestV1::new(
            operation,
            device_offset,
            u64::from(copy_bytes),
        ) {
            Ok(request) => request,
            Err(error) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let reserved = match allocation.owner.reserve(use_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    allocation,
                    host,
                ));
            }
        };
        let prepared_use = match allocation.owner.prepare(reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, reserved) = failure.into_parts();
                let _ = allocation.owner.cancel_reserved(reserved);
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let device = match allocation.owner.detach_sdma_buffer(
            allocation.attachment.queue,
            allocation.attachment.pool_generation,
            allocation.attachment.logical_bytes,
        ) {
            Ok(device) => device,
            Err(error) => {
                let _ = allocation.owner.cancel_prepared(prepared_use);
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let request = directional_persistent_sdma_request_v1(
            direction,
            host,
            host_offset,
            device,
            device_offset,
            copy_bytes,
        );

        let handoff_queue = allocation.attachment.queue;
        let handoff_native_queue_id = allocation.attachment.pair.queue_id(direction);
        let mut request = Some(request);
        let mut preparation_failure = None;
        let mut preparation_contract_failed = false;
        let mut prepared_without_handoff = None;
        let mut handoff_attempted = false;
        let mut publication = None;
        let prepare_and_publish_operation = self.with_sdma_owner_memory(|owner, memory| {
            match owner.prepare_persistent_window_recoverable(
                memory,
                request
                    .take()
                    .expect("persistent window request consumed once"),
            ) {
                Ok(prepared) => {
                    if prepared.tickets().len() != packet_count {
                        preparation_contract_failed = true;
                        preparation_failure = Some((
                            Gfx942SdmaErrorV1::Contract(
                                "directional persistent SDMA window prepared ticket count",
                            ),
                            prepared.into_request(),
                        ));
                    } else {
                        planned_tickets.extend_from_slice(prepared.tickets());
                        handoff_attempted = true;
                        if let Err(error) = memory.check_queue_operational_currentness() {
                            prepared_without_handoff = Some(prepared);
                            return Err(error.into());
                        }
                        let handoff = DirectionalPersistentSdmaWindowPreparedHandoffV1 {
                            queue: handoff_queue,
                            native_queue_id: handoff_native_queue_id,
                            direction,
                            packet_count,
                            planned_tickets: core::mem::take(&mut planned_tickets),
                            prepared,
                        };
                        publication = Some(handoff.publish(owner, memory));
                    }
                }
                Err(failure) => preparation_failure = Some(failure),
            }
            Ok(())
        });
        if !handoff_attempted {
            let (lower_error, request) = preparation_failure.unwrap_or_else(|| {
                (
                    Gfx942SdmaErrorV1::Contract(
                        "directional persistent SDMA window preparation did not execute",
                    ),
                    request.expect("unexecuted window preparation retains request"),
                )
            });
            let closing_prepare = self.check_directional_persistent_sdma_operational_currentness();
            let owner_poisoned = self
                .sdma
                .as_ref()
                .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
            if prepare_and_publish_operation.is_err()
                || closing_prepare.is_err()
                || owner_poisoned
                || preparation_contract_failed
            {
                return Err(
                    self.terminal_prepared_directional_persistent_sdma_window_failure(
                        prepare_and_publish_operation
                            .err()
                            .or_else(|| closing_prepare.err())
                            .unwrap_or_else(|| lower_error.into()),
                        allocation,
                        prepared_use,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        packet_count,
                        host_binding,
                        request,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    ),
                );
            }
            let (mut allocation, host) = restore_directional_persistent_sdma_request_v1(
                allocation,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                request,
            )
            .unwrap_or_else(|_| unreachable!("exact prepared window request must restore"));
            allocation
                .owner
                .cancel_prepared(prepared_use)
                .expect("private prepared window use must cancel");
            return Err(retryable(lower_error.into(), allocation, host));
        }
        let Some((handoff_direction, handoff_packet_count, planned_tickets, publication)) =
            publication
        else {
            return Err(
                self.terminal_prepared_directional_persistent_sdma_window_failure(
                    prepare_and_publish_operation.err().unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window handoff did not publish",
                        ),
                    ),
                    allocation,
                    prepared_use,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    packet_count,
                    host_binding,
                    prepared_without_handoff
                        .expect("failed handoff retains window preparation")
                        .into_request(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let direction = handoff_direction;
        let packet_count = handoff_packet_count;
        let (observation, lower_error) = match publication {
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable {
                error,
                prepared,
            }) => (
                DirectionalPersistentSdmaWindowPublicationObservationV1::Recoverable(
                    prepared.into_request(),
                ),
                error,
            ),
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Retained { error, tickets }) => (
                DirectionalPersistentSdmaWindowPublicationObservationV1::Retained(tickets),
                error,
            ),
            Ok(tickets) => (
                DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                Gfx942SdmaErrorV1::Contract(
                    "directional persistent SDMA window post-publication currentness",
                ),
            ),
        };
        let closing = self.check_directional_persistent_sdma_operational_currentness();
        let transition = transition_directional_persistent_sdma_window_publication_v1(
            DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                allocation,
                prepared: prepared_use,
                planned_tickets,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                packet_count,
            },
            observation,
            prepare_and_publish_operation.is_ok(),
            closing.is_ok(),
        );
        self.finish_directional_persistent_sdma_window_publication_transition(
            prepare_and_publish_operation
                .err()
                .or_else(|| closing.err())
                .unwrap_or_else(|| lower_error.into()),
            transition,
        )
    }

    /// Observes a complete persistent packet window without retiring a prefix.
    #[allow(clippy::result_large_err)]
    pub fn poll_directional_persistent_sdma_window_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowCopyPollV1,
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
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
        if !self
            .directional_persistent_sdma_attachment_is_current(&submission.allocation.attachment)
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission
            .allocation
            .attachment
            .pair
            .queue_id(submission.direction);
        if submission.tickets.len() != submission.packet_count
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.allocation.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window ticket identity",
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
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    poll_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window poll did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match poll_result {
            Ok(PersistentSdmaWindowPollV1::Pending) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Pending,
                None,
            ),
            Err(error) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(PersistentSdmaWindowPollV1::Completed(completed)) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_directional_persistent_sdma_window_completion_v1(
            submission,
            observation,
            poll_operation.is_ok(),
        ) {
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(submission) => Ok(
                Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Pending(submission),
            ),
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) => Ok(
                Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Completed(completed),
            ),
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(_) => {
                unreachable!("window poll cannot produce timeout custody")
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_directional_persistent_sdma_window_execution_transition(
                    poll_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }

    /// Waits for every packet in a persistent window. Timeout returns the
    /// unchanged aggregate submission for a later wait or poll.
    #[allow(clippy::result_large_err)]
    pub fn wait_directional_persistent_sdma_window_for_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        timeout: Duration,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowCompletedV1,
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    > {
        self.wait_directional_persistent_sdma_window_with_v1(
            submission,
            timeout,
            |owner, memory, tickets, timeout| {
                owner.wait_persistent_window_for(
                    memory,
                    tickets,
                    timeout,
                    SdmaWaitProfileV1::PersistentElapsedSpinFloor(
                        PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1,
                    ),
                )
            },
        )
    }

    /// Diagnostic-only wait with the same custody/currentness transition as the
    /// ordinary route. Timeout and errors return no diagnostic observations.
    #[cfg(feature = "hardware-diagnostic")]
    #[allow(clippy::result_large_err)]
    pub fn wait_directional_persistent_sdma_window_profiled_for_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        timeout: Duration,
        policy: crate::Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    ) -> Result<
        (
            Gfx942DirectionalPersistentSdmaWindowCompletedV1,
            crate::Gfx942SdmaPersistentWaitDiagnosticsV1,
        ),
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    > {
        let mut diagnostics = None;
        let completed = self.wait_directional_persistent_sdma_window_with_v1(
            submission,
            timeout,
            |owner, memory, tickets, timeout| {
                owner
                    .wait_persistent_window_profiled_for_v1(memory, tickets, timeout, policy)
                    .map(|(completed, observed)| {
                        diagnostics = Some(observed);
                        completed
                    })
            },
        )?;
        Ok((
            completed,
            diagnostics.expect("profiled lower completion retains its observations"),
        ))
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn wait_directional_persistent_sdma_window_with_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        timeout: Duration,
        lower_wait: impl FnOnce(
            &mut crate::sdma::Gfx942SdmaQueueSetV1,
            &mut SharedGttMemorySessionV1,
            &[crate::sdma::Gfx942SdmaCopyTicketV1],
            Duration,
        ) -> Result<
            crate::sdma::CompletedPersistentSdmaWindowV1,
            Gfx942SdmaErrorV1,
        >,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowCompletedV1,
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
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
        if !self
            .directional_persistent_sdma_attachment_is_current(&submission.allocation.attachment)
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission
            .allocation
            .attachment
            .pair
            .queue_id(submission.direction);
        if submission.tickets.len() != submission.packet_count
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.allocation.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut wait_result = None;
        let wait_operation = self.with_sdma_owner_memory(|owner, memory| {
            wait_result = Some(lower_wait(owner, memory, &submission.tickets, timeout));
            Ok(())
        });
        let Some(wait_result) = wait_result else {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    wait_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window wait did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match wait_result {
            Err(Gfx942SdmaErrorV1::Timeout) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Timeout,
                None,
            ),
            Err(error) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(completed) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_directional_persistent_sdma_window_completion_v1(
            submission,
            observation,
            wait_operation.is_ok(),
        ) {
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(submission) => {
                Err(pending(
                    ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    submission,
                ))
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) => {
                Ok(completed)
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(_) => {
                unreachable!("window wait cannot produce pending custody")
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_directional_persistent_sdma_window_execution_transition(
                    wait_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }
}
