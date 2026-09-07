//! Whole-submission custody for the experimental two-native logical SDMA mux.

use std::time::Duration;

use super::{
    ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1, SdmaPublicationModeV1,
    admit_sdma_publication_while_compute_detached,
    permanently_poison_process_global_kfd_runtime_gate_v1,
};
use crate::sdma::{
    Gfx942SdmaCopyRequestV1, Gfx942SdmaErrorV1, Gfx942SdmaLogicalMuxCompletedV2,
    Gfx942SdmaLogicalMuxPlanV2, Gfx942SdmaLogicalMuxPollV2, Gfx942SdmaLogicalMuxSubmissionV2,
    Gfx942SdmaMultiQueueShardTicketsV1, Gfx942SdmaStripedDiagnosticSpinBudgetV1,
    Gfx942SdmaStripedTailWaitOutcomeV1, Gfx942SdmaStripedWaitDiagnosticsV1,
    Gfx942SdmaUnpublishedCopyRequestV1, LogicalMuxSdmaSubmitFailureV2,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaLogicalMuxFailureDispositionV2 {
    RetryablePreflight,
    TerminalPrePublication,
    TerminalPartialPublication,
    TerminalPostPublication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2<'a> {
    native_queue: usize,
    queue_id: u32,
    request_indices: &'a [u16],
    retained_ticket_count: usize,
}

impl<'a> Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2<'a> {
    pub const fn native_queue(self) -> usize {
        self.native_queue
    }

    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }

    pub const fn request_indices(self) -> &'a [u16] {
        self.request_indices
    }

    pub const fn retained_ticket_count(self) -> usize {
        self.retained_ticket_count
    }
}

enum Gfx942SdmaLogicalMuxTerminalCustodyStateV2 {
    BeforePublication(Vec<Gfx942SdmaCopyRequestV1>),
    Publication {
        plan: Gfx942SdmaLogicalMuxPlanV2,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
        untouched: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
    },
    CompletePublication(Gfx942SdmaLogicalMuxSubmissionV2),
    CompletedOpaque(Gfx942SdmaLogicalMuxCompletedV2),
}

/// Audit-only custody after a terminal V2 mux failure.
///
/// It intentionally exposes no tickets, buffers, drain operation, or resubmit
/// authority. Native mappings remain held by the poisoned session until
/// process teardown.
#[must_use = "terminal logical-mux custody must remain retained until process teardown"]
pub struct Gfx942SdmaLogicalMuxTerminalCustodyV2 {
    state: Gfx942SdmaLogicalMuxTerminalCustodyStateV2,
}

impl Gfx942SdmaLogicalMuxTerminalCustodyV2 {
    fn before_publication(requests: Vec<Gfx942SdmaCopyRequestV1>) -> Self {
        Self {
            state: Gfx942SdmaLogicalMuxTerminalCustodyStateV2::BeforePublication(requests),
        }
    }

    fn publication(
        plan: Gfx942SdmaLogicalMuxPlanV2,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
        untouched: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
    ) -> Self {
        Self {
            state: Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication {
                plan,
                confirmed,
                indeterminate,
                untouched,
            },
        }
    }

    fn complete_publication(submission: Gfx942SdmaLogicalMuxSubmissionV2) -> Self {
        Self {
            state: Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletePublication(submission),
        }
    }

    fn completed_opaque(completed: Gfx942SdmaLogicalMuxCompletedV2) -> Self {
        Self {
            state: Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletedOpaque(completed),
        }
    }

    pub const fn plan(&self) -> Option<&Gfx942SdmaLogicalMuxPlanV2> {
        match &self.state {
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::BeforePublication(_) => None,
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication { plan, .. } => Some(plan),
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletePublication(submission) => {
                Some(submission.plan())
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletedOpaque(completed) => {
                Some(completed.plan())
            }
        }
    }

    pub fn confirmed_native_shard_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::BeforePublication(_) => 0,
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication { confirmed, .. } => {
                confirmed.len()
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletePublication(submission) => {
                submission.native_shard_count()
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletedOpaque(_) => 0,
        }
    }

    pub fn confirmed_native_shard(
        &self,
        index: usize,
    ) -> Option<Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2<'_>> {
        match &self.state {
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication { confirmed, .. } => {
                shard_observation_v2(confirmed.get(index)?)
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletePublication(submission) => {
                let shard = submission.native_shard(index)?;
                Some(Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2 {
                    native_queue: shard.native_queue(),
                    queue_id: shard.queue_id(),
                    request_indices: shard.request_indices(),
                    retained_ticket_count: shard.retained_ticket_count(),
                })
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::BeforePublication(_)
            | Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletedOpaque(_) => None,
        }
    }

    pub fn indeterminate_native_shard(
        &self,
    ) -> Option<Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2<'_>> {
        let Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication { indeterminate, .. } =
            &self.state
        else {
            return None;
        };
        shard_observation_v2(indeterminate.as_ref()?)
    }

    pub fn untouched_request_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::BeforePublication(requests) => {
                requests.len()
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication { untouched, .. } => {
                untouched.len()
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletePublication(_)
            | Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletedOpaque(_) => 0,
        }
    }

    pub fn untouched_request_index(&self, index: usize) -> Option<usize> {
        match &self.state {
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::BeforePublication(requests) => {
                (index < requests.len()).then_some(index)
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::Publication { untouched, .. } => {
                untouched.get(index).map(|request| request.request_index())
            }
            Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletePublication(_)
            | Gfx942SdmaLogicalMuxTerminalCustodyStateV2::CompletedOpaque(_) => None,
        }
    }
}

fn shard_observation_v2(
    shard: &Gfx942SdmaMultiQueueShardTicketsV1,
) -> Option<Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2<'_>> {
    (shard.queue_ordinal() < 2).then_some(Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2 {
        native_queue: shard.queue_ordinal(),
        queue_id: shard.queue_id(),
        request_indices: shard.request_indices(),
        retained_ticket_count: shard.ticket_count(),
    })
}

#[must_use = "inspect retryable requests or retain terminal custody through process teardown"]
pub enum Gfx942SdmaLogicalMuxFailureCustodyV2 {
    RetryableRequests(Vec<Gfx942SdmaCopyRequestV1>),
    ProcessTeardown(Gfx942SdmaLogicalMuxTerminalCustodyV2),
}

#[must_use = "failure preserves exact logical-mux custody and publication progress"]
pub struct Gfx942SdmaLogicalMuxSubmissionFailureV2 {
    error: ComputeAqlQueueSessionErrorV1,
    disposition: Gfx942SdmaLogicalMuxFailureDispositionV2,
    custody: Gfx942SdmaLogicalMuxFailureCustodyV2,
}

impl Gfx942SdmaLogicalMuxSubmissionFailureV2 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub const fn disposition(&self) -> Gfx942SdmaLogicalMuxFailureDispositionV2 {
        self.disposition
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.disposition,
            Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
        )
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SdmaLogicalMuxFailureDispositionV2,
        Gfx942SdmaLogicalMuxFailureCustodyV2,
    ) {
        (self.error, self.disposition, self.custody)
    }
}

#[must_use = "a timeout returns the whole V2 submission; terminal custody requires teardown"]
pub enum Gfx942SdmaLogicalMuxExecutionCustodyV2 {
    Pending(Gfx942SdmaLogicalMuxSubmissionV2),
    ProcessTeardown(Gfx942SdmaLogicalMuxTerminalCustodyV2),
}

#[must_use = "inspect the V2 aggregate execution error and retain its custody"]
pub struct Gfx942SdmaLogicalMuxExecutionFailureV2 {
    error: ComputeAqlQueueSessionErrorV1,
    custody: Gfx942SdmaLogicalMuxExecutionCustodyV2,
}

impl Gfx942SdmaLogicalMuxExecutionFailureV2 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SdmaLogicalMuxExecutionCustodyV2,
    ) {
        (self.error, self.custody)
    }
}

const fn preparation_disposition_v2(
    owner_poisoned: bool,
    closing_currentness_failed: bool,
) -> Gfx942SdmaLogicalMuxFailureDispositionV2 {
    if owner_poisoned || closing_currentness_failed {
        Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
    } else {
        Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
    }
}

const fn availability_disposition_v2(
    session_terminal: bool,
) -> Gfx942SdmaLogicalMuxFailureDispositionV2 {
    if session_terminal {
        Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
    } else {
        Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
    }
}

const fn publication_disposition_v2(
    published_shards: usize,
    has_indeterminate_shard: bool,
    closing_currentness_failed: bool,
) -> Gfx942SdmaLogicalMuxFailureDispositionV2 {
    if published_shards != 0 || has_indeterminate_shard {
        Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPartialPublication
    } else if closing_currentness_failed {
        Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
    } else {
        Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
    }
}

const fn logical_mux_cursor_commit_is_authorized_v2(
    request_count: usize,
    published_native_shards: usize,
    closing_currentness_succeeded: bool,
    owner_retake_succeeded: bool,
) -> bool {
    request_count >= 2
        && published_native_shards == 2
        && closing_currentness_succeeded
        && owner_retake_succeeded
}

fn recover_no_effect_logical_mux_requests_v2<R, U>(
    plan: &Gfx942SdmaLogicalMuxPlanV2,
    untouched: Vec<U>,
    request_index: impl Fn(&U) -> usize,
    into_request: impl Fn(U) -> R,
) -> Result<Vec<R>, Vec<U>> {
    if untouched.len() != plan.request_count()
        || untouched
            .iter()
            .enumerate()
            .any(|(index, request)| request_index(request) != index)
    {
        return Err(untouched);
    }
    let mut requests = Vec::new();
    if requests.try_reserve_exact(untouched.len()).is_err() {
        return Err(untouched);
    }
    requests.extend(untouched.into_iter().map(into_request));
    Ok(requests)
}

fn catch_logical_mux_operation_unwind_v2<R>(
    operation: impl FnOnce() -> R,
) -> std::thread::Result<R> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
}

impl ComputeAqlQueueSessionV1 {
    fn poison_logical_mux_terminal_v2(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }

    #[cold]
    fn fail_stop_logical_mux_unwind_v2(&mut self, payload: Box<dyn core::any::Any + Send>) -> ! {
        // No checkpoint proves that an unwind reaching this facade catch is pre-effect.
        self.poison_logical_mux_terminal_v2();
        core::mem::forget(payload);
        std::process::abort()
    }

    fn commit_restored_logical_mux_cursor_v2(
        &mut self,
        plan: &Gfx942SdmaLogicalMuxPlanV2,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.sdma
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing restored logical-mux SDMA owner",
            ))?
            .commit_logical_mux_success_v2(plan)
            .map_err(Into::into)
    }

    fn enforce_logical_mux_disposition_v2(
        &mut self,
        disposition: Gfx942SdmaLogicalMuxFailureDispositionV2,
    ) {
        if disposition != Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight {
            self.poison_logical_mux_terminal_v2();
        }
    }

    /// Publishes the stable-filtered logical-lane roster once on each native queue.
    ///
    /// This API deliberately adds ordering between logical lanes mapped to the
    /// same native queue. It is not HIP stream independence or scheduling parity.
    #[allow(clippy::result_large_err)]
    pub fn submit_gfx942_sdma_logical_mux_batch_v2(
        &mut self,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaLogicalMuxSubmissionV2, Gfx942SdmaLogicalMuxSubmissionFailureV2> {
        if let Err(error) = self.require_logical_mux_sdma_enabled_v2() {
            let disposition = availability_disposition_v2(self.terminal_poisoned);
            let terminal =
                disposition != Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight;
            self.enforce_logical_mux_disposition_v2(disposition);
            let custody = if terminal {
                Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                    Gfx942SdmaLogicalMuxTerminalCustodyV2::before_publication(requests),
                )
            } else {
                Gfx942SdmaLogicalMuxFailureCustodyV2::RetryableRequests(requests)
            };
            return Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                error,
                disposition,
                custody,
            });
        }
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.persistent_compute.is_some(),
            SdmaPublicationModeV1::StripedBatch,
        ) {
            return Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                error: error.into(),
                disposition: Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight,
                custody: Gfx942SdmaLogicalMuxFailureCustodyV2::RetryableRequests(requests),
            });
        }
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                disposition: Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight,
                custody: Gfx942SdmaLogicalMuxFailureCustodyV2::RetryableRequests(requests),
            });
        }

        let mut pending = Some(requests);
        let mut submitted = None;
        let operation = catch_logical_mux_operation_unwind_v2(|| {
            let operation = self.with_logical_mux_sdma_owner_memory_v2(|owner, memory| {
                memory.check_queue_operational_currentness()?;
                let requests = pending.take().expect("logical-mux requests consumed once");
                let result = owner.submit_logical_mux_batch_v2(memory, requests);
                let closing = memory
                    .check_queue_operational_currentness()
                    .map_err(ComputeAqlQueueSessionErrorV1::from);
                submitted = Some((result, closing));
                Ok(())
            });
            let commit_plan = match submitted.as_ref() {
                Some((Ok(submission), closing))
                    if logical_mux_cursor_commit_is_authorized_v2(
                        submission.plan().request_count(),
                        submission.native_shard_count(),
                        closing.is_ok(),
                        operation.is_ok(),
                    ) =>
                {
                    Some(*submission.plan())
                }
                _ => None,
            };
            if let Some(plan) = commit_plan {
                let commit = self.commit_restored_logical_mux_cursor_v2(&plan);
                let Some((Ok(_), closing)) = submitted.as_mut() else {
                    std::process::abort();
                };
                *closing = commit;
            } else if operation.is_ok()
                && let Some((Ok(_), closing)) = submitted.as_mut()
                && closing.is_ok()
            {
                *closing = Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "logical-mux cursor commit requires both native shards",
                ));
            }
            operation
        });
        let operation = match operation {
            Ok(operation) => operation,
            Err(payload) => self.fail_stop_logical_mux_unwind_v2(payload),
        };
        if submitted.is_none() {
            self.poison_logical_mux_terminal_v2();
            return Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                error: operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "logical-mux operation did not execute",
                    )),
                disposition: Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication,
                custody: Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                    Gfx942SdmaLogicalMuxTerminalCustodyV2::before_publication(
                        pending.expect("unexecuted operation retains requests"),
                    ),
                ),
            });
        }
        let (submitted, closing) = submitted.expect("executed operation stores result");
        let closing_error = operation.err().or_else(|| closing.err());
        match submitted {
            Ok(submission) => match closing_error {
                None => Ok(submission),
                Some(error) => {
                    self.poison_logical_mux_terminal_v2();
                    Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                        error,
                        disposition:
                            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPostPublication,
                        custody: Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                            Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(submission),
                        ),
                    })
                }
            },
            Err(LogicalMuxSdmaSubmitFailureV2::Preparation(failure)) => {
                let disposition = preparation_disposition_v2(
                    self.logical_mux_sdma_is_poisoned_v2(),
                    closing_error.is_some(),
                );
                let terminal =
                    disposition != Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight;
                let error = closing_error.unwrap_or_else(|| failure.error.into());
                self.enforce_logical_mux_disposition_v2(disposition);
                let custody = if terminal {
                    Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::before_publication(failure.requests),
                    )
                } else {
                    Gfx942SdmaLogicalMuxFailureCustodyV2::RetryableRequests(failure.requests)
                };
                Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                    error,
                    disposition,
                    custody,
                })
            }
            Err(LogicalMuxSdmaSubmitFailureV2::Publication(failure)) => {
                let disposition = publication_disposition_v2(
                    failure.published.len(),
                    failure.indeterminate.is_some(),
                    closing_error.is_some(),
                );
                if disposition == Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight {
                    match recover_no_effect_logical_mux_requests_v2(
                        &failure.plan,
                        failure.unpublished,
                        |request| request.request_index(),
                        Gfx942SdmaUnpublishedCopyRequestV1::into_request,
                    ) {
                        Ok(requests) => {
                            return Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                                error: failure.error.into(),
                                disposition,
                                custody: Gfx942SdmaLogicalMuxFailureCustodyV2::RetryableRequests(
                                    requests,
                                ),
                            });
                        }
                        Err(untouched) => {
                            self.poison_logical_mux_terminal_v2();
                            return Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                                error: ComputeAqlQueueSessionErrorV1::Contract(
                                    "logical-mux no-effect request reconstruction",
                                ),
                                disposition:
                                    Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication,
                                custody: Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                                    Gfx942SdmaLogicalMuxTerminalCustodyV2::publication(
                                        failure.plan,
                                        failure.published,
                                        failure.indeterminate,
                                        untouched,
                                    ),
                                ),
                            });
                        }
                    }
                }
                self.enforce_logical_mux_disposition_v2(disposition);
                Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                    error: closing_error.unwrap_or_else(|| failure.error.into()),
                    disposition,
                    custody: Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::publication(
                            failure.plan,
                            failure.published,
                            failure.indeterminate,
                            failure.unpublished,
                        ),
                    ),
                })
            }
            Err(LogicalMuxSdmaSubmitFailureV2::PublishedValidation { error, submission }) => {
                self.poison_logical_mux_terminal_v2();
                Err(Gfx942SdmaLogicalMuxSubmissionFailureV2 {
                    error: closing_error.unwrap_or_else(|| error.into()),
                    disposition: Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPostPublication,
                    custody: Gfx942SdmaLogicalMuxFailureCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(submission),
                    ),
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn poll_gfx942_sdma_logical_mux_batch_v2(
        &mut self,
        submission: Gfx942SdmaLogicalMuxSubmissionV2,
    ) -> Result<Gfx942SdmaLogicalMuxPollV2, Gfx942SdmaLogicalMuxExecutionFailureV2> {
        let mut retained = Some(submission);
        let mut lower = None;
        let operation = catch_logical_mux_operation_unwind_v2(|| {
            self.with_logical_mux_sdma_owner_memory_v2(|owner, memory| {
                memory.check_queue_operational_currentness()?;
                let ready = owner.observe_logical_mux_completion_v2(
                    memory,
                    retained.as_ref().expect("logical-mux custody retained"),
                )?;
                memory.check_queue_operational_currentness()?;
                let submission = retained.take().expect("logical-mux custody consumed once");
                lower = Some(if ready {
                    owner
                        .retire_logical_mux_completion_v2(submission)
                        .map(Gfx942SdmaLogicalMuxPollV2::Completed)
                } else {
                    Ok(Gfx942SdmaLogicalMuxPollV2::Pending(submission))
                });
                Ok(())
            })
        });
        let operation = match operation {
            Ok(operation) => operation,
            Err(payload) => self.fail_stop_logical_mux_unwind_v2(payload),
        };
        if let Err(error) = operation {
            let terminal = match lower {
                Some(Ok(Gfx942SdmaLogicalMuxPollV2::Pending(submission)))
                | Some(Err((_, submission))) => {
                    Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(submission)
                }
                Some(Ok(Gfx942SdmaLogicalMuxPollV2::Completed(completed))) => {
                    Gfx942SdmaLogicalMuxTerminalCustodyV2::completed_opaque(completed)
                }
                None => Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(
                    retained.take().unwrap_or_else(|| std::process::abort()),
                ),
            };
            self.poison_logical_mux_terminal_v2();
            return Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                error,
                custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::ProcessTeardown(terminal),
            });
        }
        match lower {
            Some(Ok(outcome)) => Ok(outcome),
            Some(Err((error, submission))) => {
                self.poison_logical_mux_terminal_v2();
                Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                    error: error.into(),
                    custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(submission),
                    ),
                })
            }
            None => {
                self.poison_logical_mux_terminal_v2();
                Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "logical-mux completion observation did not execute",
                    ),
                    custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(
                            retained.take().unwrap_or_else(|| std::process::abort()),
                        ),
                    ),
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn wait_gfx942_sdma_logical_mux_batch_for_v2(
        &mut self,
        submission: Gfx942SdmaLogicalMuxSubmissionV2,
        timeout: Duration,
    ) -> Result<Gfx942SdmaLogicalMuxCompletedV2, Gfx942SdmaLogicalMuxExecutionFailureV2> {
        self.wait_gfx942_sdma_logical_mux_batch_impl_v2::<false>(submission, timeout)
            .map(|(completed, _)| completed)
    }

    /// Profiles the same two-tail wait with the existing host-only diagnostics.
    #[allow(clippy::result_large_err)]
    pub fn wait_gfx942_sdma_logical_mux_batch_profiled_for_v2(
        &mut self,
        submission: Gfx942SdmaLogicalMuxSubmissionV2,
        timeout: Duration,
    ) -> Result<
        (
            Gfx942SdmaLogicalMuxCompletedV2,
            Gfx942SdmaStripedWaitDiagnosticsV1,
        ),
        Gfx942SdmaLogicalMuxExecutionFailureV2,
    > {
        self.wait_gfx942_sdma_logical_mux_batch_impl_v2::<true>(submission, timeout)
    }

    #[allow(clippy::result_large_err)]
    fn wait_gfx942_sdma_logical_mux_batch_impl_v2<const PROFILE: bool>(
        &mut self,
        submission: Gfx942SdmaLogicalMuxSubmissionV2,
        timeout: Duration,
    ) -> Result<
        (
            Gfx942SdmaLogicalMuxCompletedV2,
            Gfx942SdmaStripedWaitDiagnosticsV1,
        ),
        Gfx942SdmaLogicalMuxExecutionFailureV2,
    > {
        let (plan, lower_submission) = submission.into_parts();
        let mut retained = Some(lower_submission);
        let mut lower = None;
        let mut diagnostics = Gfx942SdmaStripedWaitDiagnosticsV1::default();
        let operation = catch_logical_mux_operation_unwind_v2(|| {
            self.with_logical_mux_sdma_owner_memory_v2(|owner, memory| {
                lower = Some(
                    owner.wait_prepared_striped_multi_queue_tails_retaining_for::<PROFILE>(
                        memory,
                        &mut retained,
                        timeout,
                        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current,
                        &mut diagnostics,
                    ),
                );
                Ok(())
            })
        });
        let operation = match operation {
            Ok(operation) => operation,
            Err(payload) => self.fail_stop_logical_mux_unwind_v2(payload),
        };
        if let Err(error) = operation {
            let terminal = match lower {
                Some(Gfx942SdmaStripedTailWaitOutcomeV1::Completed(completed)) => {
                    Gfx942SdmaLogicalMuxTerminalCustodyV2::completed_opaque(
                        crate::sdma::Gfx942SdmaQueueSetV1::wrap_logical_mux_completed_v2(
                            plan, completed,
                        ),
                    )
                }
                Some(Gfx942SdmaStripedTailWaitOutcomeV1::Pending)
                | Some(Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(_))
                | None => {
                    let lower = retained.take().unwrap_or_else(|| std::process::abort());
                    Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(
                        Gfx942SdmaLogicalMuxSubmissionV2::from_parts(plan, lower),
                    )
                }
            };
            self.poison_logical_mux_terminal_v2();
            return Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                error,
                custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::ProcessTeardown(terminal),
            });
        }
        match lower {
            Some(Gfx942SdmaStripedTailWaitOutcomeV1::Completed(completed)) => Ok((
                crate::sdma::Gfx942SdmaQueueSetV1::wrap_logical_mux_completed_v2(plan, completed),
                diagnostics,
            )),
            Some(Gfx942SdmaStripedTailWaitOutcomeV1::Pending) => {
                let lower = retained.take().unwrap_or_else(|| std::process::abort());
                Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                    error: ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::Pending(
                        Gfx942SdmaLogicalMuxSubmissionV2::from_parts(plan, lower),
                    ),
                })
            }
            Some(Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(error)) => {
                let lower = retained.take().unwrap_or_else(|| std::process::abort());
                let submission = Gfx942SdmaLogicalMuxSubmissionV2::from_parts(plan, lower);
                self.poison_logical_mux_terminal_v2();
                Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                    error: error.into(),
                    custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(submission),
                    ),
                })
            }
            None => {
                let lower = retained.take().unwrap_or_else(|| std::process::abort());
                let submission = Gfx942SdmaLogicalMuxSubmissionV2::from_parts(plan, lower);
                self.poison_logical_mux_terminal_v2();
                Err(Gfx942SdmaLogicalMuxExecutionFailureV2 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "logical-mux completion wait did not execute",
                    ),
                    custody: Gfx942SdmaLogicalMuxExecutionCustodyV2::ProcessTeardown(
                        Gfx942SdmaLogicalMuxTerminalCustodyV2::complete_publication(submission),
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_classification_is_fail_closed() {
        assert_eq!(
            availability_disposition_v2(false),
            Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
        );
        assert_eq!(
            availability_disposition_v2(true),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
        );
        assert_eq!(
            preparation_disposition_v2(false, false),
            Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
        );
        assert_eq!(
            preparation_disposition_v2(true, false),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
        );
        assert_eq!(
            preparation_disposition_v2(false, true),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
        );
        assert_eq!(
            publication_disposition_v2(0, false, false),
            Gfx942SdmaLogicalMuxFailureDispositionV2::RetryablePreflight
        );
        assert_eq!(
            publication_disposition_v2(0, false, true),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPrePublication
        );
        assert_eq!(
            publication_disposition_v2(0, true, false),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPartialPublication
        );
        assert_eq!(
            publication_disposition_v2(1, false, true),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPartialPublication
        );
    }

    #[test]
    fn no_effect_publication_recovers_canonical_request_custody() {
        let plan = Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 16, 4, 15).unwrap();
        let untouched = vec![(0_usize, 'a'), (1, 'b'), (2, 'c'), (3, 'd')];
        let recovered = recover_no_effect_logical_mux_requests_v2(
            &plan,
            untouched,
            |request| request.0,
            |request| request.1,
        )
        .unwrap();
        assert_eq!(recovered, ['a', 'b', 'c', 'd']);

        for malformed in [
            vec![(0_usize, 'a'), (2, 'c'), (1, 'b'), (3, 'd')],
            vec![(0_usize, 'a'), (1, 'b'), (1, 'c'), (3, 'd')],
            vec![(0_usize, 'a'), (1, 'b'), (2, 'c')],
        ] {
            assert!(
                recover_no_effect_logical_mux_requests_v2(
                    &plan,
                    malformed,
                    |request| request.0,
                    |request| request.1,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn cursor_commit_gate_requires_a_complete_two_native_success() {
        assert!(logical_mux_cursor_commit_is_authorized_v2(2, 2, true, true));
        assert!(logical_mux_cursor_commit_is_authorized_v2(
            126, 2, true, true
        ));
        assert!(!logical_mux_cursor_commit_is_authorized_v2(
            1, 2, true, true
        ));
        assert!(!logical_mux_cursor_commit_is_authorized_v2(
            2, 1, true, true
        ));
        assert!(!logical_mux_cursor_commit_is_authorized_v2(
            2, 2, false, true
        ));
        assert!(!logical_mux_cursor_commit_is_authorized_v2(
            2, 2, true, false
        ));
    }

    #[test]
    fn first_native_indeterminate_is_terminal_and_cannot_advance_cursor() {
        let plan = Gfx942SdmaLogicalMuxPlanV2::new([101, 103], 16, 112, 15).unwrap();
        assert_eq!(
            publication_disposition_v2(0, true, false),
            Gfx942SdmaLogicalMuxFailureDispositionV2::TerminalPartialPublication
        );
        assert!(!logical_mux_cursor_commit_is_authorized_v2(
            plan.request_count(),
            0,
            false,
            true
        ));
        assert_eq!(plan.first_logical_lane(), 15);
    }

    #[test]
    fn publication_path_has_no_post_publication_allocation_site() {
        let lower = include_str!("../sdma/multi_queue.rs");
        let publish = lower
            .split("fn publish_multi_queue_batch<")
            .nth(1)
            .unwrap()
            .split("fn append_prepared_requests")
            .next()
            .unwrap();
        for forbidden in ["try_reserve", "Vec::new", ".collect", "to_string"] {
            assert!(!publish.contains(forbidden), "found {forbidden}");
        }

        let source = include_str!("sdma_logical_mux.rs");
        let submit = source
            .split("pub fn submit_gfx942_sdma_logical_mux_batch_v2")
            .nth(1)
            .unwrap()
            .split("pub fn poll_gfx942_sdma_logical_mux_batch_v2")
            .next()
            .unwrap();
        assert!(submit.contains("submit_logical_mux_batch_v2"));
        assert!(submit.contains("commit_restored_logical_mux_cursor_v2"));
        assert!(submit.contains("Some(*submission.plan())"));
        assert!(!submit.contains("submission.plan().clone()"));
        assert!(submit.contains("logical_mux_cursor_commit_is_authorized_v2"));
        assert!(submit.contains("closing.is_ok()"));
        assert!(submit.contains("operation.is_ok()"));
        assert!(submit.contains("closing_error.is_some(),"));
        assert!(submit.contains("recover_no_effect_logical_mux_requests_v2"));
        let owner_operation = submit
            .find("let operation = self.with_logical_mux_sdma_owner_memory_v2")
            .unwrap();
        let commit_gate = submit.find("let commit_plan =").unwrap();
        let commit = submit
            .find("self.commit_restored_logical_mux_cursor_v2")
            .unwrap();
        assert!(owner_operation < commit_gate && commit_gate < commit);
        assert!(!submit[owner_operation..commit_gate].contains("commit_logical_mux_success_v2"));
        assert!(submit.contains("let closing_error = operation.err().or_else(|| closing.err());"));
        let post_helper_failure = submit
            .split("let closing_error = operation.err().or_else(|| closing.err());")
            .nth(1)
            .unwrap();
        assert!(post_helper_failure.contains("Ok(submission) => match closing_error"));
        assert!(post_helper_failure.contains("TerminalPostPublication"));
        assert!(post_helper_failure.contains("complete_publication(submission)"));

        let restored_commit = source
            .split("fn commit_restored_logical_mux_cursor_v2")
            .nth(1)
            .unwrap()
            .split("fn enforce_logical_mux_disposition_v2")
            .next()
            .unwrap();
        assert!(restored_commit.contains("self.sdma"));
        assert!(restored_commit.contains(".commit_logical_mux_success_v2(plan)"));

        let lower = include_str!("../sdma/multi_queue/logical_mux.rs");
        let commit = lower
            .split("pub(crate) fn commit_logical_mux_success_v2")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn observe_logical_mux_completion_v2")
            .next()
            .unwrap();
        assert!(commit.contains("plan.is_current_for"));
        assert!(commit.contains("plan.next_logical_lane_after_success()"));
    }

    #[test]
    fn operation_unwind_catcher_contains_an_injected_panic() {
        let result = catch_logical_mux_operation_unwind_v2(|| {
            panic!("injected logical-mux operation panic")
        });
        assert!(result.is_err());
    }

    #[test]
    fn facade_caught_mux_unwinds_restore_poison_and_fail_stop() {
        let source = include_str!("sdma_logical_mux.rs");
        let submit = source
            .split("pub fn submit_gfx942_sdma_logical_mux_batch_v2")
            .nth(1)
            .unwrap()
            .split("pub fn poll_gfx942_sdma_logical_mux_batch_v2")
            .next()
            .unwrap();
        let poll = source
            .split("pub fn poll_gfx942_sdma_logical_mux_batch_v2")
            .nth(1)
            .unwrap()
            .split("pub fn wait_gfx942_sdma_logical_mux_batch_for_v2")
            .next()
            .unwrap();
        let wait = source
            .split("fn wait_gfx942_sdma_logical_mux_batch_impl_v2")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for facade in [submit, poll, wait] {
            let catch = facade
                .find("catch_logical_mux_operation_unwind_v2")
                .unwrap();
            let owner = facade
                .find("with_logical_mux_sdma_owner_memory_v2")
                .unwrap();
            assert!(catch < owner, "unwind catch must surround the owner helper");
            assert!(facade.contains("self.fail_stop_logical_mux_unwind_v2(payload)"));
            assert!(!facade.contains("resume_unwind"));
        }

        let fail_stop = source
            .split("fn fail_stop_logical_mux_unwind_v2")
            .nth(1)
            .unwrap()
            .split("fn commit_restored_logical_mux_cursor_v2")
            .next()
            .unwrap();
        let poison = fail_stop
            .find("self.poison_logical_mux_terminal_v2()")
            .unwrap();
        let abort = fail_stop.find("std::process::abort()").unwrap();
        assert!(poison < abort);
        assert!(fail_stop.contains("core::mem::forget(payload)"));
        assert!(!fail_stop.contains("resume_unwind"));
        assert!(!fail_stop.contains("ProcessTeardown"));

        let terminalizer = source
            .split("fn poison_logical_mux_terminal_v2")
            .nth(1)
            .unwrap()
            .split("fn fail_stop_logical_mux_unwind_v2")
            .next()
            .unwrap();
        assert!(
            terminalizer.find("self.poison_terminal()").unwrap()
                < terminalizer
                    .find("permanently_poison_process_global_kfd_runtime_gate_v1()")
                    .unwrap()
        );

        let owner_helper = include_str!("../queue_live.rs")
            .split("fn with_logical_mux_sdma_owner_memory_v2")
            .nth(1)
            .unwrap()
            .split("fn with_live_queue_memory_model<R>")
            .next()
            .unwrap();
        assert!(
            owner_helper.find("self.sdma = Some(owner)").unwrap()
                < owner_helper
                    .find("std::panic::resume_unwind(payload)")
                    .unwrap()
        );
    }

    #[test]
    fn nested_retirement_suffix_unwind_is_lower_immediate_abort() {
        let source = include_str!("../sdma/multi_queue/tail_wait.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let wait = source
            .split("pub(crate) fn wait_prepared_striped_multi_queue_tails_retaining_for")
            .nth(1)
            .unwrap()
            .split("fn audit_prepared_striped_multi_queue_tails_for")
            .next()
            .unwrap();
        assert!(
            wait.find("retained.take()").unwrap()
                < wait
                    .find("retire_after_striped_full_audit_no_unwind_v1")
                    .unwrap()
        );

        let retirement = source
            .split("fn retire_after_striped_full_audit_no_unwind_v1")
            .nth(1)
            .unwrap()
            .split("fn retire_after_striped_full_audit_infallible_v1")
            .next()
            .unwrap();
        assert!(retirement.contains("abort_if_striped_retirement_unwinds_v1"));

        let guard = source
            .split("fn abort_if_striped_retirement_unwinds_v1")
            .nth(1)
            .unwrap()
            .split("fn with_borrowed_striped_submission_v1")
            .next()
            .unwrap();
        assert!(guard.contains("std::panic::catch_unwind"));
        assert!(guard.contains("core::mem::forget(payload)"));
        assert!(guard.contains("std::process::abort()"));
        assert!(!guard.contains("resume_unwind"));
        assert!(!guard.contains("poison"));
    }
}
