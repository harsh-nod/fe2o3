//! Whole-submission custody for balanced multi-queue SDMA.

use std::time::Duration;

use super::{
    ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1, SdmaPublicationModeV1,
    admit_sdma_publication_while_compute_detached,
};
use crate::sdma::{
    Gfx942SdmaCopyRequestV1, Gfx942SdmaErrorV1, Gfx942SdmaMultiQueueCompletedV1,
    Gfx942SdmaMultiQueuePlanV1, Gfx942SdmaMultiQueuePollV1, Gfx942SdmaMultiQueueShardTicketsV1,
    Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaUnpublishedCopyRequestV1,
    MultiQueueSdmaSubmitFailureV1,
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
    CompletePublication {
        plan: Gfx942SdmaMultiQueuePlanV1,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
    },
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
        let (plan, confirmed, _completion) = submission.into_parts();
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::CompletePublication { plan, confirmed },
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
            Gfx942SdmaTerminalCustodyStateV1::Publication { plan, .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletePublication { plan, .. } => Some(plan),
            Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(completed) => Some(completed.plan()),
        }
    }

    pub fn confirmed_shard_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => 0,
            Gfx942SdmaTerminalCustodyStateV1::Publication { confirmed, .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletePublication { confirmed, .. } => {
                confirmed.len()
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
            Gfx942SdmaTerminalCustodyStateV1::Publication { confirmed, .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletePublication { confirmed, .. } => {
                confirmed.get(index)
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
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication { .. }
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
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication { .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletedOpaque(_) => None,
        }
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
            let custody = if self.terminal_poisoned {
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
            self.poison_terminal();
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
                    self.poison_terminal();
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
                    disposition == Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication;
                let error = closing_error.unwrap_or_else(|| failure.error.into());
                if terminal {
                    self.poison_terminal();
                }
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
                self.poison_terminal();
                let disposition = classify_multi_queue_publication_failure(
                    failure.published.len(),
                    failure.indeterminate.is_some(),
                );
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
                self.poison_terminal();
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
            self.poison_terminal();
            return Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                error,
                custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(terminal),
            });
        }
        match lower {
            Some(Ok(outcome)) => Ok(outcome),
            Some(Err((error, submission))) => {
                self.poison_terminal();
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: error.into(),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                    ),
                })
            }
            None => {
                self.poison_terminal();
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

    /// Waits on every striped shard with one shared deadline and whole-batch custody.
    // Whole-submission custody is preallocated before publication and stays inline on failure.
    #[allow(clippy::result_large_err)]
    pub fn wait_gfx942_striped_sdma_copy_batch_for_v1(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
    ) -> Result<Gfx942SdmaMultiQueueCompletedV1, Gfx942SdmaMultiQueueExecutionFailureV1> {
        let mut retained = Some(submission);
        let mut lower = None;
        let operation = self.with_striped_sdma_owner_memory(|owner, memory| {
            memory.check_queue_operational_currentness()?;
            let ready = owner.wait_prepared_striped_multi_queue_completion_for(
                memory,
                retained.as_ref().expect("aggregate custody retained"),
                timeout,
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
            self.poison_terminal();
            return Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                error,
                custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(terminal),
            });
        }
        match lower {
            Some(Ok(Gfx942SdmaMultiQueuePollV1::Completed(completed))) => Ok(completed),
            Some(Ok(Gfx942SdmaMultiQueuePollV1::Pending(submission))) => {
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::Pending(submission),
                })
            }
            Some(Err((error, submission))) => {
                self.poison_terminal();
                Err(Gfx942SdmaMultiQueueExecutionFailureV1 {
                    error: error.into(),
                    custody: Gfx942SdmaMultiQueueExecutionCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                    ),
                })
            }
            None => {
                self.poison_terminal();
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
mod tests {
    use super::*;

    #[test]
    fn multi_queue_failures_distinguish_retryable_and_terminal_truth() {
        assert_eq!(
            classify_multi_queue_preparation_failure(false, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
        );
        assert_eq!(
            classify_multi_queue_preparation_failure(true, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
        assert_eq!(
            classify_multi_queue_preparation_failure(false, true),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
        assert_eq!(
            classify_multi_queue_publication_failure(0, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
        assert_eq!(
            classify_multi_queue_publication_failure(0, true),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
        );
        assert_eq!(
            classify_multi_queue_publication_failure(1, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
        );
    }

    #[test]
    fn already_terminal_multi_queue_session_never_advertises_retryable_custody() {
        assert_eq!(
            classify_multi_queue_availability_failure(false),
            Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
        );
        assert_eq!(
            classify_multi_queue_availability_failure(true),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
    }

    #[test]
    fn terminal_shard_observation_returns_the_source_slice_lifetime() {
        fn request_indices<'a>(observation: Gfx942SdmaTerminalShardObservationV1<'a>) -> &'a [u16] {
            observation.request_indices()
        }

        let indices = [1_u16, 5, 9];
        let observation = Gfx942SdmaTerminalShardObservationV1 {
            queue_ordinal: 3,
            queue_id: 17,
            request_indices: &indices,
            retained_ticket_count: indices.len(),
        };
        let retained = request_indices(observation);
        assert_eq!(retained, indices);
    }

    #[test]
    fn aggregate_completion_has_one_envelope_and_never_partially_retires() {
        let live = include_str!("sdma_multi_queue.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let submit = live
            .split("pub fn submit_gfx942_striped_sdma_copy_batch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_gfx942_striped_sdma_copy_batch_v1")
            .next()
            .unwrap();
        assert_eq!(submit.matches("with_striped_sdma_owner_memory").count(), 1);
        assert_eq!(
            submit
                .matches("check_queue_operational_currentness")
                .count(),
            2
        );
        let opening = submit.find("check_queue_operational_currentness").unwrap();
        let publication = submit.find("submit_striped_multi_queue_batch").unwrap();
        let closing = submit.rfind("check_queue_operational_currentness").unwrap();
        let cursor = submit.find("commit_striped_multi_queue_success").unwrap();
        assert!(opening < publication && publication < closing && closing < cursor);

        let poll = live
            .split("pub fn poll_gfx942_striped_sdma_copy_batch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
            .next()
            .unwrap();
        assert_eq!(poll.matches("with_striped_sdma_owner_memory").count(), 1);
        assert_eq!(
            poll.matches("check_queue_operational_currentness").count(),
            2
        );
        assert!(!poll.contains("Vec::"));
        assert!(!poll.contains("try_reserve"));
        assert!(!poll.contains("collect"));
        assert!(!poll.contains("prepare_striped_multi_queue_completion"));
        let observe = poll
            .find("observe_prepared_striped_multi_queue_completion")
            .unwrap();
        let closing = poll.rfind("check_queue_operational_currentness").unwrap();
        let retire = poll
            .find("retire_prepared_striped_multi_queue_completion")
            .unwrap();
        assert!(observe < closing && closing < retire);
        assert!(!poll.contains(".striped_sdma.as_mut()"));
        assert!(poll.contains("completed_opaque(completed)"));

        let wait = live
            .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
            .nth(1)
            .unwrap()
            .split("pub fn execute_sdma_copy_batch_for")
            .next()
            .unwrap();
        assert_eq!(wait.matches("with_striped_sdma_owner_memory").count(), 1);
        assert_eq!(
            wait.matches("check_queue_operational_currentness").count(),
            2
        );
        assert!(!wait.contains("Vec::"));
        assert!(!wait.contains("try_reserve"));
        assert!(!wait.contains("collect"));
        assert!(!wait.contains("prepare_striped_multi_queue_completion"));
        assert!(wait.contains("Gfx942SdmaErrorV1::Timeout"));
        assert!(wait.contains("Gfx942SdmaMultiQueueExecutionCustodyV1::Pending(submission)"));
        assert!(!wait.contains(".striped_sdma.as_mut()"));
        assert!(wait.contains("completed_opaque(completed)"));
        assert!(
            wait.find("wait_prepared_striped_multi_queue_completion_for")
                .unwrap()
                < wait
                    .find("retire_prepared_striped_multi_queue_completion")
                    .unwrap()
        );
    }

    #[test]
    fn terminal_custody_is_observation_only_and_drop_is_physically_inert() {
        let source = include_str!("sdma_multi_queue.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let terminal_impl = source
            .split("impl Gfx942SdmaMultiQueueTerminalCustodyV1")
            .nth(1)
            .unwrap()
            .split("pub enum Gfx942SdmaMultiQueueFailureCustodyV1")
            .next()
            .unwrap();

        for forbidden in [
            "pub fn into_",
            "pub fn release",
            "pub fn recycle",
            "pub fn drain",
            "pub fn ticket",
            "pub fn request(",
            "pub fn buffer",
        ] {
            assert!(
                !terminal_impl.contains(forbidden),
                "forbidden terminal API: {forbidden}"
            );
        }
        assert!(terminal_impl.contains("pub const fn plan"));
        assert!(terminal_impl.contains("pub fn confirmed_shard"));
        assert!(terminal_impl.contains("pub fn untouched_request_index"));
        assert!(!source.contains("impl Drop for Gfx942SdmaMultiQueueTerminalCustodyV1"));
        assert!(source.contains("Dropping the wrapper discards audit observations only"));
        assert!(source.contains("performs no native"));
    }
}
