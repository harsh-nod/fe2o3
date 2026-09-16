//! One retained owner spans synchronous preparation, publication and model retake.

#![forbid(unsafe_code)]

use super::*;
use crate::persistent_directional_sdma::Gfx942DirectionalPersistentSdmaTerminalStageV1;
use crate::sdma::SingleSdmaCopyCustodyV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) enum UseV1 {
    Reserved(Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>),
    Prepared(Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>),
    Published(Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>),
    Completed(Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>),
    Settled(Gfx942PersistentDependencyFrontierV1),
}

pub(crate) struct SdmaSynchronousCustodyV1 {
    pub(super) allocation: Option<Gfx942DirectionalQueuePersistentAllocationV1>,
    pub(super) host: Option<Gfx942SdmaBufferV1>,
    pub(super) data: Option<SingleSdmaCopyCustodyV1>,
    pub(super) usage: Option<UseV1>,
    host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    sequence: Option<u64>,
    pub(crate) stage: Gfx942DirectionalPersistentSdmaTerminalStageV1,
}

impl SdmaSynchronousCustodyV1 {
    pub(super) fn new(admitted: DirectionalPersistentSdmaAdmittedRequestV1) -> Self {
        let host_binding = Gfx942PersistentDirectionalSdmaHostBindingV1::capture(
            &admitted.host,
            admitted.allocation.attachment.queue,
        );
        Self {
            allocation: Some(admitted.allocation),
            host: Some(admitted.host),
            data: None,
            usage: None,
            host_binding,
            direction: admitted.direction,
            host_offset: admitted.host_offset,
            device_offset: admitted.device_offset,
            copy_bytes: admitted.copy_bytes,
            sequence: None,
            stage: Gfx942DirectionalPersistentSdmaTerminalStageV1::SynchronousUnsettled,
        }
    }

    fn allocation(&mut self) -> &mut Gfx942DirectionalQueuePersistentAllocationV1 {
        self.allocation
            .as_mut()
            .expect("synchronous allocation stays rooted")
    }

    fn prepare_request(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.host.is_some() && self.data.is_none());
        let operation = match self.direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let request = Gfx942PersistentUseRequestV1::new(
            operation,
            self.device_offset,
            u64::from(self.copy_bytes),
        )
        .map_err(map_directional_persistent_sdma_use_error_v1)?;
        let reserved = self
            .allocation()
            .owner
            .reserve(request, None)
            .map_err(|failure| map_directional_persistent_sdma_use_error_v1(failure.error()))?;
        self.sequence = Some(reserved.sequence());
        self.usage = Some(UseV1::Reserved(reserved));
        self.prepare_use()?;
        let native = self
            .allocation()
            .owner
            .detach_local_native_for_sdma()
            .map_err(map_directional_persistent_sdma_use_error_v1)?;
        let attachment = self.allocation().attachment;
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(native),
            attachment.queue,
            attachment.pool_generation,
            attachment.logical_bytes,
        );
        self.data = Some(SingleSdmaCopyCustodyV1::Request(
            directional_persistent_sdma_request_v1(
                self.direction,
                self.host.take().expect("admitted host"),
                self.host_offset,
                device,
                self.device_offset,
                self.copy_bytes,
            ),
        ));
        Ok(())
    }

    fn prepare_use(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.allocation.is_some());
        assert!(matches!(self.usage, Some(UseV1::Reserved(_))));
        let Some(UseV1::Reserved(lease)) = self.usage.take() else {
            unreachable!("reserved use")
        };
        match self.allocation().owner.prepare(lease) {
            Ok(lease) => {
                self.usage = Some(UseV1::Prepared(lease));
                Ok(())
            }
            Err(failure) => {
                let (error, lease) = failure.into_parts();
                self.usage = Some(UseV1::Reserved(lease));
                Err(map_directional_persistent_sdma_use_error_v1(error))
            }
        }
    }

    fn publish_use(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.allocation.is_some());
        assert!(matches!(self.usage, Some(UseV1::Prepared(_))));
        let Some(UseV1::Prepared(lease)) = self.usage.take() else {
            unreachable!("prepared use")
        };
        match self.allocation().owner.publish(lease) {
            Ok(lease) => {
                self.usage = Some(UseV1::Published(lease));
                Ok(())
            }
            Err(failure) => {
                let (error, lease) = failure.into_parts();
                self.usage = Some(UseV1::Prepared(lease));
                Err(map_directional_persistent_sdma_use_error_v1(error))
            }
        }
    }

    fn cancel_use(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.allocation.is_some());
        let error = match self.usage.take() {
            None => return Ok(()),
            Some(UseV1::Reserved(lease)) => match self.allocation().owner.cancel_reserved(lease) {
                Ok(()) => return Ok(()),
                Err(failure) => {
                    let (error, lease) = failure.into_parts();
                    self.usage = Some(UseV1::Reserved(lease));
                    error
                }
            },
            Some(UseV1::Prepared(lease)) => match self.allocation().owner.cancel_prepared(lease) {
                Ok(()) => return Ok(()),
                Err(failure) => {
                    let (error, lease) = failure.into_parts();
                    self.usage = Some(UseV1::Prepared(lease));
                    error
                }
            },
            other => {
                self.usage = other;
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "synchronous use is not cancelable",
                ));
            }
        };
        Err(map_directional_persistent_sdma_use_error_v1(error))
    }

    fn complete_use(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.allocation.is_some());
        assert!(matches!(self.usage, Some(UseV1::Published(_))));
        let Some(UseV1::Published(lease)) = self.usage.take() else {
            unreachable!("published use")
        };
        match self.allocation().owner.complete(lease) {
            Ok(lease) => self.usage = Some(UseV1::Completed(lease)),
            Err(failure) => {
                let (error, lease) = failure.into_parts();
                self.usage = Some(UseV1::Published(lease));
                return Err(map_directional_persistent_sdma_use_error_v1(error));
            }
        }
        let Some(UseV1::Completed(lease)) = self.usage.take() else {
            unreachable!("completed use")
        };
        match self.allocation().owner.settle(lease) {
            Ok(frontier) => {
                self.usage = Some(UseV1::Settled(frontier));
                Ok(())
            }
            Err(failure) => {
                let (error, lease) = failure.into_parts();
                self.usage = Some(UseV1::Completed(lease));
                Err(map_directional_persistent_sdma_use_error_v1(error))
            }
        }
    }

    fn observe_timeout(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.allocation.is_some());
        assert!(matches!(self.usage, Some(UseV1::Published(_))));
        let Some(UseV1::Published(lease)) = self.usage.take() else {
            unreachable!("published timeout use")
        };
        match self.allocation().owner.observe_timeout(lease) {
            Ok(timeout) => {
                self.usage = Some(UseV1::Published(timeout.into_published()));
                Ok(())
            }
            Err(failure) => {
                let (error, lease) = failure.into_parts();
                self.usage = Some(UseV1::Published(lease));
                Err(map_directional_persistent_sdma_use_error_v1(error))
            }
        }
    }

    fn restore_data(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert!(self.allocation.is_some());
        if self.host.is_some() && self.data.is_none() {
            return Ok(());
        }
        let completed = matches!(self.data, Some(SingleSdmaCopyCustodyV1::Completed(_)));
        // These are callback-free owner moves. Every failed validation returns both inputs.
        let request = match self.data.take() {
            Some(SingleSdmaCopyCustodyV1::Request(request)) => request,
            Some(SingleSdmaCopyCustodyV1::Prepared(prepared)) => prepared.into_request(),
            Some(SingleSdmaCopyCustodyV1::Completed(completed)) => Gfx942SdmaCopyRequestV1 {
                source: completed.source,
                destination: completed.destination,
                source_offset: completed.source_offset,
                destination_offset: completed.destination_offset,
                copy_bytes: completed.copy_bytes,
            },
            other => {
                self.data = other;
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "synchronous data remains queue-retained",
                ));
            }
        };
        let allocation = self.allocation.take().expect("retained allocation");
        match restore_directional_persistent_sdma_request_v1(
            allocation,
            self.direction,
            self.host_offset,
            self.device_offset,
            self.copy_bytes,
            self.host_binding,
            request,
        ) {
            Ok((allocation, host)) => {
                self.allocation = Some(allocation);
                self.host = Some(host);
                Ok(())
            }
            Err((allocation, request)) => {
                self.allocation = Some(allocation);
                self.data = Some(if completed {
                    SingleSdmaCopyCustodyV1::Completed(Gfx942SdmaCompletedCopyV1 {
                        source: request.source,
                        destination: request.destination,
                        source_offset: request.source_offset,
                        destination_offset: request.destination_offset,
                        copy_bytes: request.copy_bytes,
                    })
                } else {
                    SingleSdmaCopyCustodyV1::Request(request)
                });
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "synchronous directional SDMA restoration",
                ))
            }
        }
    }

    fn quarantine(&mut self, reason: Gfx942PersistentQuarantineReasonV1) {
        assert!(self.allocation.is_some());
        let phase = match self.usage.as_ref() {
            None => Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored,
            Some(UseV1::Prepared(_)) if self.host.is_some() => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedRestored
            }
            Some(UseV1::Prepared(_))
                if matches!(self.data, Some(SingleSdmaCopyCustodyV1::QueueRetained(_))) =>
            {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained
            }
            Some(UseV1::Prepared(_)) => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
            }
            Some(UseV1::Published(_))
                if matches!(self.data, Some(SingleSdmaCopyCustodyV1::QueueRetained(_))) =>
            {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
            }
            Some(UseV1::Published(_))
                if matches!(self.data, Some(SingleSdmaCopyCustodyV1::Completed(_))) =>
            {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
            }
            _ => Gfx942DirectionalPersistentSdmaTerminalStageV1::SynchronousUnsettled,
        };
        match self.usage.take() {
            None => self
                .allocation()
                .owner
                .quarantine_for_caller_reported_currentness_loss(),
            Some(UseV1::Prepared(lease)) => {
                if let Err(failure) = self.allocation().owner.quarantine_prepared(lease, reason) {
                    self.usage = Some(UseV1::Prepared(failure.into_parts().1));
                    return;
                }
            }
            Some(UseV1::Published(lease)) => {
                if let Err(failure) = self.allocation().owner.quarantine_published(lease, reason) {
                    self.usage = Some(UseV1::Published(failure.into_parts().1));
                    return;
                }
            }
            Some(UseV1::Completed(lease)) => {
                if let Err(failure) = self.allocation().owner.quarantine_completed(lease, reason) {
                    self.usage = Some(UseV1::Completed(failure.into_parts().1));
                    return;
                }
            }
            other => {
                self.usage = other;
                self.allocation()
                    .owner
                    .quarantine_for_caller_reported_currentness_loss();
            }
        }
        self.stage = phase;
    }

    fn into_terminal(self) -> Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
        Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
            direction: self.direction,
            sequence: self.sequence,
            state: Gfx942DirectionalPersistentSdmaTerminalStateV1::Synchronous(self),
        }
    }
}

pub(super) enum OutcomeV1 {
    BeforePublication {
        error: ComputeAqlQueueSessionErrorV1,
        healthy: bool,
        closing: bool,
    },
    Published {
        error: Option<ComputeAqlQueueSessionErrorV1>,
        closing: bool,
        timeout: bool,
    },
}

pub(super) fn run_in_place(
    root: &mut SdmaSynchronousCustodyV1,
    owner: &mut Gfx942SdmaQueueSetV1,
    memory: &mut impl crate::sdma::SdmaSingleMemoryV1,
    timeout: Duration,
) -> OutcomeV1 {
    if let Err(error) = owner.prepare_directional_single_in_place(memory, &mut root.data) {
        let closing = memory.check_queue_operational_currentness();
        return OutcomeV1::BeforePublication {
            healthy: !owner.is_poisoned(),
            closing: closing.is_ok(),
            error: closing
                .err()
                .map(Into::into)
                .unwrap_or_else(|| error.into()),
        };
    }
    let Some(SingleSdmaCopyCustodyV1::Prepared(prepared)) = root.data.as_ref() else {
        unreachable!("prepared copy")
    };
    let planned = prepared.ticket();
    if let Err(error) = memory.check_queue_operational_currentness() {
        return OutcomeV1::BeforePublication {
            error: error.into(),
            healthy: false,
            closing: false,
        };
    }
    let ticket = match owner.publish_single_in_place(memory, &mut root.data) {
        Ok(ticket) => ticket,
        Err(error) => {
            let closing = memory.check_queue_operational_currentness();
            return OutcomeV1::BeforePublication {
                healthy: !owner.is_poisoned()
                    && matches!(root.data, Some(SingleSdmaCopyCustodyV1::Prepared(_))),
                closing: closing.is_ok(),
                error: closing
                    .err()
                    .map(Into::into)
                    .unwrap_or_else(|| error.into()),
            };
        }
    };
    let published = root.publish_use();
    let attachment = root.allocation().attachment;
    if published.is_err()
        || ticket != planned
        || !planned_ticket_matches_queue_occurrence(
            planned,
            attachment.queue,
            attachment.pair.queue_id(root.direction),
        )
    {
        let closing = memory.check_queue_operational_currentness();
        return OutcomeV1::Published {
            closing: closing.is_ok(),
            timeout: false,
            error: Some(
                closing
                    .err()
                    .map(Into::into)
                    .or_else(|| published.err())
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA synchronous publication ticket identity",
                    )),
            ),
        };
    }
    match owner.wait_for_in_current_scope_with_final_currentness(memory, ticket, timeout) {
        SingleSdmaWaitInCurrentScopeV1::Completed(completed) => {
            root.data = Some(SingleSdmaCopyCustodyV1::Completed(completed));
            OutcomeV1::Published {
                error: None,
                closing: true,
                timeout: false,
            }
        }
        SingleSdmaWaitInCurrentScopeV1::Timeout => OutcomeV1::Published {
            error: Some(Gfx942SdmaErrorV1::Timeout.into()),
            closing: true,
            timeout: true,
        },
        SingleSdmaWaitInCurrentScopeV1::QueueRetained(error) => OutcomeV1::Published {
            error: Some(error.into()),
            closing: true,
            timeout: false,
        },
        SingleSdmaWaitInCurrentScopeV1::FinalCurrentnessLost(error) => OutcomeV1::Published {
            error: Some(error.into()),
            closing: false,
            timeout: false,
        },
    }
}

pub(super) trait SdmaSynchronousContextV1 {
    fn root(&mut self) -> &mut Option<SdmaSynchronousCustodyV1>;
    fn opening(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn run(&mut self, timeout: Duration) -> OutcomeV1;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn seal(&mut self);
    fn poison(&mut self);
}

pub(super) fn poison<C: SdmaSynchronousContextV1>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

type ExecutionResult = Result<
    Gfx942DirectionalPersistentSdmaCompletedV1,
    Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
>;

#[allow(clippy::result_large_err)]
fn terminal<C: SdmaSynchronousContextV1>(
    context: &mut C,
    error: ComputeAqlQueueSessionErrorV1,
    execution: bool,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> ExecutionResult {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.seal())));
    let root = context.root().as_mut().expect("retained synchronous root");
    if matches!(
        root.data,
        Some(SingleSdmaCopyCustodyV1::Request(_) | SingleSdmaCopyCustodyV1::Prepared(_))
    ) {
        let _ = root.restore_data();
    }
    root.quarantine(reason);
    let custody = context
        .root()
        .take()
        .expect("settled terminal root")
        .into_terminal();
    Err(if execution {
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(
            Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                    custody,
                ),
            },
        )
    } else {
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
            Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                    custody,
                ),
            },
        )
    })
}

#[allow(clippy::result_large_err)]
fn retry<C: SdmaSynchronousContextV1>(
    context: &mut C,
    error: ComputeAqlQueueSessionErrorV1,
) -> ExecutionResult {
    let root = context.root().as_mut().expect("retry root");
    let prepared = matches!(root.data, Some(SingleSdmaCopyCustodyV1::Prepared(_)));
    if root.restore_data().is_err() {
        return terminal(
            context,
            if prepared {
                error
            } else {
                ComputeAqlQueueSessionErrorV1::Contract(
                    "synchronous directional persistent SDMA preparation restoration",
                )
            },
            false,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
    }
    if let Err(failure) = root.cancel_use() {
        return terminal(
            context,
            failure,
            false,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
    }
    assert!(root.allocation.is_some() && root.host.is_some());
    assert!(root.data.is_none() && root.usage.is_none());
    let mut root = context.root().take().expect("restored retry root");
    Err(
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
            Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                    allocation: root.allocation.take().expect("restored allocation"),
                    host: root.host.take().expect("restored host"),
                },
            },
        ),
    )
}

#[allow(clippy::result_large_err)]
pub(super) fn execute_in_place<C: SdmaSynchronousContextV1>(
    context: &mut C,
    admitted: DirectionalPersistentSdmaAdmittedRequestV1,
    timeout: Duration,
) -> ExecutionResult {
    if context.root().is_some() {
        std::process::abort();
    }
    *context.root() = Some(SdmaSynchronousCustodyV1::new(admitted));
    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Err(error) = context.opening() {
            return terminal(
                context,
                error,
                false,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
        }
        if let Err(error) = context
            .root()
            .as_mut()
            .expect("admitted root")
            .prepare_request()
        {
            return retry(context, error);
        }
        let (outcome, retake) = match execute_live_model_custody_v1(
            context,
            C::loan,
            |context| context.run(timeout),
            C::retake,
            poison,
        ) {
            Ok(result) => result,
            Err(error) => {
                return terminal(
                    context,
                    error,
                    false,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
            }
        };
        match outcome {
            OutcomeV1::BeforePublication {
                error,
                healthy,
                closing,
            } => {
                let reason = if matches!(
                    context.root().as_ref().expect("publication root").data,
                    Some(SingleSdmaCopyCustodyV1::QueueRetained(_))
                ) {
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate
                } else {
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
                };
                if let Err(error) = retake {
                    return terminal(context, error, false, reason);
                }
                if healthy && closing {
                    retry(context, error)
                } else {
                    terminal(context, error, false, reason)
                }
            }
            OutcomeV1::Published {
                error,
                closing,
                timeout,
            } => {
                if let Err(error) = retake {
                    return terminal(
                        context,
                        error,
                        true,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    );
                }
                if !closing || (error.is_some() && !timeout) {
                    return terminal(
                        context,
                        error.unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "synchronous final currentness",
                        )),
                        true,
                        if closing {
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
                        } else {
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
                        },
                    );
                }
                let root = context.root().as_mut().expect("published root");
                if timeout {
                    if let Err(error) = root.observe_timeout() {
                        return terminal(context, error, true,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate);
                    }
                    assert!(root.allocation.is_some() && root.host.is_none());
                    assert!(matches!(root.usage, Some(UseV1::Published(_))));
                    assert!(matches!(
                        root.data,
                        Some(SingleSdmaCopyCustodyV1::QueueRetained(_))
                    ));
                    let mut root = context.root().take().expect("timeout root");
                    let Some(UseV1::Published(published)) = root.usage.take() else {
                        unreachable!("timeout published use")
                    };
                    let Some(SingleSdmaCopyCustodyV1::QueueRetained(ticket)) = root.data.take()
                    else {
                        unreachable!("timeout queue custody")
                    };
                    return Err(
                        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(
                            Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
                                error: error.unwrap_or(Gfx942SdmaErrorV1::Timeout.into()),
                                custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(
                                    Gfx942DirectionalPersistentSdmaSubmissionV1 {
                                        allocation: root
                                            .allocation
                                            .take()
                                            .expect("timeout allocation"),
                                        published,
                                        ticket,
                                        host_binding: root.host_binding,
                                        direction: root.direction,
                                        host_offset: root.host_offset,
                                        device_offset: root.device_offset,
                                        copy_bytes: root.copy_bytes,
                                    },
                                ),
                            },
                        ),
                    );
                }
                if root.restore_data().is_err() {
                    return terminal(
                        context,
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "synchronous directional persistent SDMA completion identity",
                        ),
                        true,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                    );
                }
                if let Err(error) = root.complete_use() {
                    return terminal(
                        context,
                        error,
                        true,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                    );
                }
                assert!(root.allocation.is_some() && root.host.is_some() && root.data.is_none());
                assert!(matches!(root.usage, Some(UseV1::Settled(_))));
                let mut root = context.root().take().expect("completed root");
                let Some(UseV1::Settled(frontier)) = root.usage.take() else {
                    unreachable!("settled use")
                };
                Ok(Gfx942DirectionalPersistentSdmaCompletedV1::from_settled_v1(
                    root.allocation.take().expect("completed allocation"),
                    root.host.take().expect("completed host"),
                    frontier,
                    root.direction,
                    root.host_offset,
                    root.device_offset,
                    root.copy_bytes,
                ))
            }
        }
    }));
    match result {
        Ok(result) => result,
        Err(payload) => {
            poison(context);
            resume_unwind(payload)
        }
    }
}

impl SdmaSynchronousContextV1 for ComputeAqlQueueSessionV1 {
    fn root(&mut self) -> &mut Option<SdmaSynchronousCustodyV1> {
        &mut self.sdma_synchronous
    }
    fn opening(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let (result, retake) = execute_live_model_custody_v1(
            self,
            Self::loan,
            |session| {
                session
                    .engine
                    .as_mut()
                    .expect("opening engine")
                    .backend
                    .session
                    .check_queue_operational_currentness()
                    .map_err(Into::into)
            },
            Self::retake,
            poison,
        )?;
        retake?;
        result
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }
    fn run(&mut self, timeout: Duration) -> OutcomeV1 {
        run_in_place(
            self.sdma_synchronous
                .as_mut()
                .expect("installed synchronous root"),
            self.sdma.as_mut().expect("admitted SDMA owner"),
            &mut self.engine.as_mut().expect("loaned engine").backend.session,
            timeout,
        )
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.retake_model_ownership_after_live_mutation(loan)
    }
    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
    fn seal(&mut self) {
        self.poison_terminal();
    }
}
