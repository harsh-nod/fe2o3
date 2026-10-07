//! Whole-submission custody for balanced multi-queue SDMA.

use std::time::Duration;

use super::{
    ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1, SdmaPublicationModeV1,
    admit_sdma_publication_while_compute_detached,
    permanently_poison_process_global_kfd_runtime_gate_v1,
};
use crate::sdma::{
    Gfx942SdmaCopyRequestV1, Gfx942SdmaErrorV1, Gfx942SdmaMultiQueueCompletedV1,
    Gfx942SdmaMultiQueuePlanV1, Gfx942SdmaMultiQueuePollV1, Gfx942SdmaMultiQueueShardTicketsV1,
    Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaStripedDiagnosticSpinBudgetV1,
    Gfx942SdmaStripedTailWaitOutcomeV1, Gfx942SdmaStripedWaitDiagnosticsV1,
    Gfx942SdmaUnpublishedCopyRequestV1, MultiQueueSdmaSubmitFailureV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaMultiQueueFailureDispositionV1 {
    RetryablePreflight,
    TerminalPrePublication,
    TerminalPartialPublication,
    TerminalPostPublication,
}

/// Addressless observation of queue-retained terminal custody.
///
/// Ticket values are intentionally not exposed: after any multi-queue terminal failure the
/// session is poisoned, so these records cannot be polled, drained, or resubmitted safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaTerminalShardObservationV1<'a> {
    queue_ordinal: usize,
    queue_id: u32,
    request_indices: &'a [u16],
    retained_ticket_count: usize,
}

impl<'a> Gfx942SdmaTerminalShardObservationV1<'a> {
    pub const fn queue_ordinal(self) -> usize {
        self.queue_ordinal
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

enum Gfx942SdmaTerminalCustodyStateV1 {
    BeforePublication(Vec<Gfx942SdmaCopyRequestV1>),
    Publication {
        plan: Gfx942SdmaMultiQueuePlanV1,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
        untouched: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
    },
    CompletePublication(Gfx942SdmaMultiQueueSubmissionV1),
    CompletedOpaque(Gfx942SdmaMultiQueueCompletedV1),
}

/// Audit-only ownership retained after a terminal multi-queue failure.
///
/// The contained buffers remain either queue-retained or tied to the poisoned queue occurrence.
/// This type deliberately provides observations only and has no ticket/request extraction or
/// drain API. Dropping the wrapper discards audit observations only: it performs no native
/// cleanup, and every mapping and allocation remains owned by the poisoned session until process
/// teardown.
#[must_use = "terminal SDMA custody must remain retained until process teardown"]
pub struct Gfx942SdmaMultiQueueTerminalCustodyV1 {
    state: Gfx942SdmaTerminalCustodyStateV1,
}
impl Gfx942SdmaMultiQueueTerminalCustodyV1 {
    fn before_publication(requests: Vec<Gfx942SdmaCopyRequestV1>) -> Self {
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::BeforePublication(requests),
        }
    }

    fn publication(
        plan: Gfx942SdmaMultiQueuePlanV1,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
        untouched: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
    ) -> Self {
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::Publication {
                plan,
                confirmed,
                indeterminate,
                untouched,
            },
        }
    }

    fn complete_publication(submission: Gfx942SdmaMultiQueueSubmissionV1) -> Self {
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::CompletePublication(submission),
        }
    }

    fn completed_opaque(completed: Gfx942SdmaMultiQueueCompletedV1) -> Self {
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(completed),
        }
    }

    pub const fn plan(&self) -> Option<&Gfx942SdmaMultiQueuePlanV1> {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => None,
            Gfx942SdmaTerminalCustodyStateV1::Publication { plan, .. } => Some(plan),
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication(submission) => {
                Some(submission.plan())
            }
            Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(completed) => Some(completed.plan()),
        }
    }

    pub fn confirmed_shard_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => 0,
            Gfx942SdmaTerminalCustodyStateV1::Publication { confirmed, .. } => confirmed.len(),
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication(submission) => {
                submission.shards().len()
            }
            Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(_) => 0,
        }
    }

    pub fn confirmed_shard(
        &self,
        index: usize,
    ) -> Option<Gfx942SdmaTerminalShardObservationV1<'_>> {
        let shard = match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => None,
            Gfx942SdmaTerminalCustodyStateV1::Publication { confirmed, .. } => confirmed.get(index),
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication(submission) => {
                submission.shards().get(index)
            }
            Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(_) => None,
        }?;
        Some(Gfx942SdmaTerminalShardObservationV1 {
            queue_ordinal: shard.queue_ordinal(),
            queue_id: shard.queue_id(),
            request_indices: shard.request_indices(),
            retained_ticket_count: shard.tickets().len(),
        })
    }

    pub fn indeterminate_shard(&self) -> Option<Gfx942SdmaTerminalShardObservationV1<'_>> {
        let Gfx942SdmaTerminalCustodyStateV1::Publication { indeterminate, .. } = &self.state
        else {
            return None;
        };
        let shard = indeterminate.as_ref()?;
        Some(Gfx942SdmaTerminalShardObservationV1 {
            queue_ordinal: shard.queue_ordinal(),
            queue_id: shard.queue_id(),
            request_indices: shard.request_indices(),
            retained_ticket_count: shard.tickets().len(),
        })
    }

    pub fn untouched_request_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(requests) => requests.len(),
            Gfx942SdmaTerminalCustodyStateV1::Publication { untouched, .. } => untouched.len(),
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication(_)
            | Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(_) => 0,
        }
    }

    pub fn untouched_request_index(&self, index: usize) -> Option<usize> {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(requests) => {
                (index < requests.len()).then_some(index)
            }
            Gfx942SdmaTerminalCustodyStateV1::Publication { untouched, .. } => {
                untouched.get(index).map(|request| request.request_index())
            }
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication(_)
            | Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(_) => None,
        }
    }

    #[cfg(test)]
    fn exact_complete_publication_identity_for_test(
        &self,
    ) -> Option<crate::sdma::Gfx942SdmaMultiQueueIdentityForTestV1> {
        let Gfx942SdmaTerminalCustodyStateV1::CompletePublication(submission) = &self.state else {
            return None;
        };
        Some(submission.exact_identity_for_unwind_test())
    }
}

#[must_use = "inspect retryable requests or retain terminal custody through process teardown"]
pub enum Gfx942SdmaMultiQueueFailureCustodyV1 {
    /// No native side effect occurred and the requests may be submitted again.
    RetryableRequests(Vec<Gfx942SdmaCopyRequestV1>),
    /// The queue occurrence is terminal. This is audit-only/process-teardown custody.
    ProcessTeardown(Gfx942SdmaMultiQueueTerminalCustodyV1),
}

#[must_use = "failure preserves exact multi-queue custody and publication progress"]
pub struct Gfx942SdmaMultiQueueSubmissionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    disposition: Gfx942SdmaMultiQueueFailureDispositionV1,
    custody: Gfx942SdmaMultiQueueFailureCustodyV1,
}

impl Gfx942SdmaMultiQueueSubmissionFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub const fn disposition(&self) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
        self.disposition
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.disposition,
            Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
        )
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SdmaMultiQueueFailureDispositionV1,
        Gfx942SdmaMultiQueueFailureCustodyV1,
    ) {
        (self.error, self.disposition, self.custody)
    }
}

#[must_use = "a timeout returns the whole submission; terminal custody requires teardown"]
pub enum Gfx942SdmaMultiQueueExecutionCustodyV1 {
    Pending(Gfx942SdmaMultiQueueSubmissionV1),
    ProcessTeardown(Gfx942SdmaMultiQueueTerminalCustodyV1),
}

#[must_use = "inspect the aggregate execution error and retain its custody"]
pub struct Gfx942SdmaMultiQueueExecutionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    custody: Gfx942SdmaMultiQueueExecutionCustodyV1,
}

fn catch_striped_wait_epoch_unwind_v1<R>(operation: impl FnOnce() -> R) -> std::thread::Result<R> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
}

fn poison_multi_queue_terminal_boundary_v1(
    poison_local: impl FnOnce(),
    poison_process: impl FnOnce(),
) {
    poison_local();
    poison_process();
}

fn enforce_multi_queue_submission_disposition_v1(
    disposition: Gfx942SdmaMultiQueueFailureDispositionV1,
    poison_local: impl FnOnce(),
    poison_process: impl FnOnce(),
) {
    if disposition != Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight {
        poison_multi_queue_terminal_boundary_v1(poison_local, poison_process);
    }
}

fn take_striped_wait_panic_custody_v1<T>(
    retained: &mut Option<T>,
    poison_terminal: impl FnOnce(),
) -> T {
    let custody = retained.take().unwrap_or_else(|| std::process::abort());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(poison_terminal)) {
        Ok(()) => custody,
        Err(payload) => {
            core::mem::forget(custody);
            core::mem::forget(payload);
            std::process::abort();
        }
    }
}

impl Gfx942SdmaMultiQueueExecutionFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SdmaMultiQueueExecutionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

const fn classify_multi_queue_preparation_failure(
    owner_poisoned: bool,
    closing_currentness_failed: bool,
) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
    if owner_poisoned || closing_currentness_failed {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    } else {
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
    }
}

const fn classify_multi_queue_availability_failure(
    session_terminal: bool,
) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
    if session_terminal {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    } else {
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
    }
}

const fn classify_multi_queue_publication_failure(
    published_shards: usize,
    has_indeterminate_shard: bool,
) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
    if published_shards == 0 && !has_indeterminate_shard {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    } else {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
    }
}

impl ComputeAqlQueueSessionV1 {
    fn poison_gfx942_multi_queue_terminal_v1(&mut self) {
        poison_multi_queue_terminal_boundary_v1(
            || self.poison_terminal(),
            permanently_poison_process_global_kfd_runtime_gate_v1,
        );
    }

    fn enforce_gfx942_multi_queue_submission_disposition_v1(
        &mut self,
        disposition: Gfx942SdmaMultiQueueFailureDispositionV1,
    ) {
        enforce_multi_queue_submission_disposition_v1(
            disposition,
            || self.poison_terminal(),
            permanently_poison_process_global_kfd_runtime_gate_v1,
        );
    }

    /// Preflights and then publishes one balanced batch across every striped SDMA queue.
    ///
    /// All shards are prepared before the first queue write-pointer publication. Native queues
    /// cannot be rolled back as a group, so a later shard failure reports confirmed earlier
    /// shards, one optional indeterminate retained shard, and every untouched request separately.
    /// Terminal custody is observation-only and must remain retained until process teardown.
    // Inline custody avoids allocating an error after native publication has begun.
    #[allow(clippy::result_large_err)]
    pub fn submit_gfx942_striped_sdma_copy_batch_v1(
        &mut self,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaMultiQueueSubmissionFailureV1> {
        if let Err(error) = self.require_striped_sdma_enabled() {
            let disposition = classify_multi_queue_availability_failure(self.terminal_poisoned);
            let terminal =
                disposition != Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight;
            self.enforce_gfx942_multi_queue_submission_disposition_v1(disposition);
            let custody = if terminal {
                Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                    Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(requests),
                )
            } else {
                Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests)
            };
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
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
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: error.into(),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests),
            });
        }
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests),
            });
        }
        let mut pending = Some(requests);
        let mut submitted = None;
        let operation = self.with_striped_sdma_owner_memory(|owner, memory| {
            memory.check_queue_operational_currentness()?;
            let requests = pending.take().expect("multi-queue requests consumed once");
            let result = owner.submit_striped_multi_queue_batch(memory, requests);
            let mut closing = memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from);
            if closing.is_ok()
                && let Ok(submission) = &result
            {
                closing = owner
                    .commit_striped_multi_queue_success(submission.plan())
                    .map_err(Into::into);
            }
            submitted = Some((result, closing));
            Ok(())
        });
        if submitted.is_none() {
            self.poison_gfx942_multi_queue_terminal_v1();
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "multi-queue operation did not execute",
                    )),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                    Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(
                        pending.expect("unexecuted operation retains requests"),
                    ),
                ),
            });
        }
        let (submitted, closing) = submitted.expect("executed multi-queue operation stores result");
        let closing_error = operation.err().or_else(|| closing.err());
        match submitted {
            Ok(submission) => match closing_error {
                None => Ok(submission),
                Some(error) => {
                    self.poison_gfx942_multi_queue_terminal_v1();
                    Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                        error,
                        disposition:
                            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPostPublication,
                        custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                            Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                        ),
                    })
                }
            },
            Err(MultiQueueSdmaSubmitFailureV1::Preparation(failure)) => {
                let poisoned = self.striped_sdma_is_poisoned();
                let disposition =
                    classify_multi_queue_preparation_failure(poisoned, closing_error.is_some());
                let terminal =
                    disposition != Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight;
                let error = closing_error.unwrap_or_else(|| failure.error.into());
                self.enforce_gfx942_multi_queue_submission_disposition_v1(disposition);
                let custody = if terminal {
                    Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(failure.requests),
                    )
                } else {
                    Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(failure.requests)
                };
                Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                    error,
                    disposition,
                    custody,
                })
            }
            Err(MultiQueueSdmaSubmitFailureV1::Publication(failure)) => {
                let disposition = classify_multi_queue_publication_failure(
                    failure.published.len(),
                    failure.indeterminate.is_some(),
                );
                self.enforce_gfx942_multi_queue_submission_disposition_v1(disposition);
                Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                    error: closing_error.unwrap_or_else(|| failure.error.into()),
                    disposition,
                    custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::publication(
                            failure.plan,
                            failure.published,
                            failure.indeterminate,
                            failure.unpublished,
                        ),
                    ),
                })
            }
            Err(MultiQueueSdmaSubmitFailureV1::PublishedValidation { error, submission }) => {
                self.poison_gfx942_multi_queue_terminal_v1();
                Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                    error: closing_error.unwrap_or_else(|| error.into()),
                    disposition: Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPostPublication,
                    custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                    ),
                })
            }
        }
    }

    /// Observes every shard before retiring any queue record.
    // Whole-submission custody is preallocated before publication and stays inline on failure.
    #[allow(clippy::result_large_err)]
    pub fn poll_gfx942_striped_sdma_copy_batch_v1(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<Gfx942SdmaMultiQueuePollV1, Gfx942SdmaMultiQueueExecutionFailureV1> {
        let mut retained = Some(submission);
        let mut lower = None;
        let operation = self.with_striped_sdma_owner_memory(|owner, memory| {
            memory.check_queue_operational_currentness()?;
            let ready = owner.observe_prepared_striped_multi_queue_completion(
                memory,
                retained.as_ref().expect("aggregate custody retained"),
            )?;
            memory.check_queue_operational_currentness()?;
            let submission = retained.take().expect("aggregate custody consumed once");
            lower = Some(if ready {
                owner
                    .retire_prepared_striped_multi_queue_completion(submission)
                    .map(Gfx942SdmaMultiQueuePollV1::Completed)
            } else {
                Ok(Gfx942SdmaMultiQueuePollV1::Pending(submission))
            });
            Ok(())
        });
        if let Err(error) = operation {
            let terminal = match lower {
                Some(Ok(Gfx942SdmaMultiQueuePollV1::Pending(submission)))
                | Some(Err((_, submission))) => {
                    Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission)
                }
                Some(Ok(Gfx942SdmaMultiQueuePollV1::Completed(completed))) => {
                    Gfx942SdmaMultiQueueTerminalCustodyV1::completed_opaque(completed)
                }
                None => Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(
                    retained.take().unwrap_or_else(|| std::process::abort()),
                ),
            };
            self.poison_gfx942_multi_queue_terminal_v1();
            return Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                error,
                custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(terminal),
            });
        }
        match lower {
            Some(Ok(outcome)) => Ok(outcome),
            Some(Err((error, submission))) => {
                self.poison_gfx942_multi_queue_terminal_v1();
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: error.into(),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                    ),
                })
            }
            None => {
                self.poison_gfx942_multi_queue_terminal_v1();
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "striped completion observation did not execute",
                    ),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(
                            retained.take().unwrap_or_else(|| std::process::abort()),
                        ),
                    ),
                })
            }
        }
    }

    /// Waits on exact striped-shard tail fences with one shared deadline.
    ///
    /// Each call is one native wait epoch. Before the deadline, an epoch observes only one
    /// prebound system+snoop tail fence per active shard. Tail readiness or the deadline triggers
    /// one full ordered completion/currentness/retirement-preflight audit. A timeout returns the
    /// unchanged whole submission; passing it to this method again begins a new epoch with a new
    /// final audit. This retryable native timeout is not a refinement of R46's terminal timeout
    /// receipt, and the native gfx942 fence-ordering premise is not proved by Rust.
    // Whole-submission custody is preallocated before publication and stays inline on failure.
    #[allow(clippy::result_large_err)]
    pub fn wait_gfx942_striped_sdma_copy_batch_for_v1(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
    ) -> Result<Gfx942SdmaMultiQueueCompletedV1, Gfx942SdmaMultiQueueExecutionFailureV1> {
        self.wait_gfx942_striped_sdma_copy_batch_impl_v1::<false>(
            submission,
            timeout,
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current,
        )
        .map(|(completed, _)| completed)
    }

    /// Runs the same retained striped-tail wait while recording host-side diagnostics.
    ///
    /// The diagnostic path adds host timestamp reads, Linux thread CPU/rusage
    /// observations, and counters. Measurement failure or invalidity remains
    /// diagnostic-only. These observations are not device timestamps or execution
    /// authority, and benchmark comparisons must use this method consistently rather
    /// than comparing it to the unprofiled path as though host overhead were identical.
    #[allow(clippy::result_large_err)]
    pub fn wait_gfx942_striped_sdma_copy_batch_profiled_for_v1(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
    ) -> Result<
        (
            Gfx942SdmaMultiQueueCompletedV1,
            Gfx942SdmaStripedWaitDiagnosticsV1,
        ),
        Gfx942SdmaMultiQueueExecutionFailureV1,
    > {
        self.wait_gfx942_striped_sdma_copy_batch_impl_v1::<true>(
            submission,
            timeout,
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current,
        )
    }

    /// Runs the profiled wait with one preregistered diagnostic active-spin budget.
    ///
    /// Non-current choices busy-poll for the selected elapsed floor, clamped to the
    /// same caller deadline, before continuing the existing adaptive pause schedule
    /// with its 25 us sleep-request ceiling. This is a benchmark experiment only:
    /// the selector is closed, the ordinary wait cannot receive it, and neither the
    /// budget nor any timing observation supplies completion authority.
    #[allow(clippy::result_large_err)]
    pub fn wait_gfx942_striped_sdma_copy_batch_profiled_with_diagnostic_spin_budget_for_v1(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
        diagnostic_spin_budget: Gfx942SdmaStripedDiagnosticSpinBudgetV1,
    ) -> Result<
        (
            Gfx942SdmaMultiQueueCompletedV1,
            Gfx942SdmaStripedWaitDiagnosticsV1,
        ),
        Gfx942SdmaMultiQueueExecutionFailureV1,
    > {
        self.wait_gfx942_striped_sdma_copy_batch_impl_v1::<true>(
            submission,
            timeout,
            diagnostic_spin_budget,
        )
    }

    #[allow(clippy::result_large_err)]
    fn wait_gfx942_striped_sdma_copy_batch_impl_v1<const PROFILE: bool>(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
        diagnostic_spin_budget: Gfx942SdmaStripedDiagnosticSpinBudgetV1,
    ) -> Result<
        (
            Gfx942SdmaMultiQueueCompletedV1,
            Gfx942SdmaStripedWaitDiagnosticsV1,
        ),
        Gfx942SdmaMultiQueueExecutionFailureV1,
    > {
        let mut retained = Some(submission);
        let mut lower = None;
        let mut diagnostics = Gfx942SdmaStripedWaitDiagnosticsV1::default();
        let operation = catch_striped_wait_epoch_unwind_v1(|| {
            self.with_striped_sdma_owner_memory(|owner, memory| {
                lower = Some(
                    owner.wait_prepared_striped_multi_queue_tails_retaining_for::<PROFILE>(
                        memory,
                        &mut retained,
                        timeout,
                        diagnostic_spin_budget,
                        &mut diagnostics,
                    ),
                );
                Ok(())
            })
        });
        let operation = match operation {
            Ok(operation) => operation,
            Err(payload) => {
                let submission = take_striped_wait_panic_custody_v1(&mut retained, || {
                    self.poison_gfx942_multi_queue_terminal_v1()
                });
                core::mem::forget(payload);
                return Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "striped tail wait panicked after publication",
                    ),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                    ),
                });
            }
        };
        if let Err(error) = operation {
            let terminal = match lower {
                Some(Gfx942SdmaStripedTailWaitOutcomeV1::Completed(completed)) => {
                    Gfx942SdmaMultiQueueTerminalCustodyV1::completed_opaque(completed)
                }
                Some(Gfx942SdmaStripedTailWaitOutcomeV1::Pending)
                | Some(Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(_))
                | None => Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(
                    retained.take().unwrap_or_else(|| std::process::abort()),
                ),
            };
            self.poison_gfx942_multi_queue_terminal_v1();
            return Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                error,
                custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(terminal),
            });
        }
        match lower {
            Some(Gfx942SdmaStripedTailWaitOutcomeV1::Completed(completed)) => {
                Ok((completed, diagnostics))
            }
            Some(Gfx942SdmaStripedTailWaitOutcomeV1::Pending) => {
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::Pending(
                        retained.take().unwrap_or_else(|| std::process::abort()),
                    ),
                })
            }
            Some(Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(error)) => {
                self.poison_gfx942_multi_queue_terminal_v1();
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: error.into(),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(
                            retained.take().unwrap_or_else(|| std::process::abort()),
                        ),
                    ),
                })
            }
            None => {
                self.poison_gfx942_multi_queue_terminal_v1();
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "striped completion wait did not execute",
                    ),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(
                            retained.take().unwrap_or_else(|| std::process::abort()),
                        ),
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
#[path = "sdma_multi_queue/tests.rs"]
mod tests;
