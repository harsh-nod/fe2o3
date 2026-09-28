//! Indexed native source pins are independent of public logical-event retains.
//! Requests persist for an indexed Prepared owner. A no-effect ordered withdrawal
//! discards that owner and resamples live logical retains on its next attempt.

use super::materialized_submission_attempt::MaterializedSubmissionAttemptV1 as Attempt;
use super::ordinary_queue_io::{OrdinaryLaneIoV1, OrdinaryQueueIoV1};
use fe2o3_kfd::{
    ComputeAqlQueueSessionErrorV1, Gfx942ComputeDependencyEventV1,
    Gfx942FixedDispatchSubmissionFailureV1 as Failure,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum SourceEventPhaseV1 {
    #[default]
    Absent,
    Requested,
    NativeOwned,
    Owned,
    ConsumingRelease,
    Released,
}

#[derive(Debug, Default)]
pub(super) struct MaterializedSourceEventV1 {
    phase: SourceEventPhaseV1,
    events: Vec<Gfx942ComputeDependencyEventV1>,
}

impl MaterializedSourceEventV1 {
    pub(super) fn may_publish(&self) -> bool {
        (self.phase == SourceEventPhaseV1::Absent && self.events.is_empty())
            || (self.phase == SourceEventPhaseV1::Owned && self.events.len() == 1)
    }

    pub(super) fn may_complete(&self) -> bool {
        self.may_publish() || self.may_retire()
    }

    pub(super) fn may_retire(&self) -> bool {
        matches!(
            self.phase,
            SourceEventPhaseV1::Absent | SourceEventPhaseV1::Released
        ) && self.events.is_empty()
    }

    pub(super) fn may_retry(&self) -> bool {
        matches!(
            self.phase,
            SourceEventPhaseV1::Absent | SourceEventPhaseV1::Requested
        ) && self.events.is_empty()
    }

    pub(super) fn submit(
        &mut self,
        requested: bool,
        attempt: &mut Attempt,
        queue: &mut OrdinaryLaneIoV1<'_, '_>,
    ) -> Result<(), Failure> {
        if !self.may_retry() {
            return Err(Failure::Terminal(ComputeAqlQueueSessionErrorV1::Contract(
                "source publication lost unconsumed request",
            )));
        }
        if requested {
            self.phase = SourceEventPhaseV1::Requested;
        }
        if self.phase == SourceEventPhaseV1::Absent {
            return attempt.submit_classified(|| queue.submit_classified());
        }
        self.phase = SourceEventPhaseV1::NativeOwned;
        attempt.submit(|| match queue.submit_source_classified() {
            Ok(source) => {
                let (batch, events) = source.into_parts();
                // Deposit the whole returned roster before the outer lane closes.
                self.events = events;
                self.phase = SourceEventPhaseV1::Owned;
                Ok(Attempt::Published(batch))
            }
            Err(Failure::RetryableBeforeSideEffect(_)) => {
                self.phase = SourceEventPhaseV1::Requested;
                Ok(Attempt::Retryable)
            }
            Err(error) => Err(error),
        })
    }

    pub(super) fn release_unused(
        &mut self,
        queue: OrdinaryQueueIoV1<'_>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.may_retire() {
            return Ok(());
        }
        if self.phase != SourceEventPhaseV1::Owned || self.events.len() != 1 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "source release lost exact indexed event",
            ));
        }
        self.phase = SourceEventPhaseV1::ConsumingRelease;
        let event = self.events.pop().expect("preflighted single source event");
        match queue.release_source_event(event) {
            Ok(()) => {
                self.phase = SourceEventPhaseV1::Released;
                Ok(())
            }
            Err(failure) => {
                let (error, retained) = failure.into_parts();
                if let Some(event) = retained {
                    // Reuse the original vector's capacity; diagnostics confer no custody.
                    self.events.push(*event);
                    self.phase = SourceEventPhaseV1::Owned;
                }
                // Owning-lane preselection must already be exact. Returned custody
                // proves no consumption, not transient backpressure.
                Err(error)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn snapshot(&self) -> (SourceEventPhaseV1, usize, usize) {
        (self.phase, self.events.len(), self.events.capacity())
    }
}
