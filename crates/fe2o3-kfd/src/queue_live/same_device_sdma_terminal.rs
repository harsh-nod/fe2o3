//! Same-device SDMA terminal custody transitions.

use super::*;

impl ComputeAqlQueueSessionV1 {
    #[allow(clippy::result_large_err)]
    pub(super) fn finish_same_device_persistent_sdma_window_publication_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        transition: SameDevicePersistentSdmaWindowPublicationTransitionV1,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1,
    > {
        match transition {
            SameDevicePersistentSdmaWindowPublicationTransitionV1::Retryable {
                source,
                destination,
            } => Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    source,
                    destination,
                },
            }),
            SameDevicePersistentSdmaWindowPublicationTransitionV1::Published(submission) => {
                Ok(submission)
            }
            SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) => {
                self.poison_terminal();
                Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                    error,
                    custody:
                        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                            custody,
                        ),
                })
            }
        }
    }

    pub(super) fn terminal_same_device_persistent_sdma_window_execution_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        custody: Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1,
    ) -> Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
        self.poison_terminal();
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                custody,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn terminal_prepared_same_device_persistent_sdma_window_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        source_prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        destination_prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        descriptor: crate::persistent_same_device_sdma::Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
        let transition = transition_same_device_persistent_sdma_window_publication_v1(
            SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                source,
                source_prepared,
                destination,
                destination_prepared,
                planned_tickets: Vec::new(),
                descriptor,
            },
            SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request),
            false,
            false,
        );
        let SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
            transition
        else {
            unreachable!("failed enclosing operation must retain terminal same-device custody")
        };
        self.poison_terminal();
        Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                custody,
            ),
        }
    }

    pub(super) fn terminal_queued_same_device_persistent_sdma_window_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
        let Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
            mut source,
            source_published,
            mut destination,
            destination_published,
            tickets,
            descriptor,
        } = submission;
        let source_sequence = source_published.sequence();
        let destination_sequence = destination_published.sequence();
        quarantine_published_local_sdma_pair_v1(
            &mut source.owner,
            source_published,
            &mut destination.owner,
            destination_published,
            reason,
        )
        .unwrap_or_else(crate::persistent_same_device_sdma::abort_paired_lease_contradiction_v1);
        self.poison_terminal();
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                    source_sequence: Some(source_sequence),
                    destination_sequence: Some(destination_sequence),
                    descriptor,
                    state:
                        Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                            source,
                            destination,
                            tickets,
                        },
                },
            ),
        }
    }
}
