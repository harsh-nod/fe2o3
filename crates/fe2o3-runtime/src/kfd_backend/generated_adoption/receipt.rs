//! In-place custody across consuming lower operations. No token is reconstructed.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HandoffV1 {
    Issue,
    Poll,
    Recycle,
}

pub(super) enum ReceiptV1<P, C> {
    Ready,
    RetryReady,
    Published(P),
    Completed(C),
    Recycled,
    // The actual classified lower refusal is retained, not rebranded Ready.
    RejectedUnpublished {
        prior: RetirementV1,
        error: fe2o3_kfd::ComputeAqlQueueSessionErrorV1,
    },
    RejectedDisposed(fe2o3_kfd::ComputeAqlQueueSessionErrorV1),
    // The original queue retains native resources when a consuming call does
    // not return custody. This marker is not a replacement completion token.
    HandedToLower(HandoffV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetirementV1 {
    Pristine,
    CancelledOnly,
    Recycled,
}

pub(super) enum PollV1<P, C> {
    Pending(P),
    Completed(C),
}

pub(crate) fn observe_generated_retirement_v1<T, E>(
    owner: &mut T,
    ready: impl Fn(&T) -> bool,
    mut progress: impl FnMut(&mut T) -> Result<(), E>,
) -> Result<bool, E> {
    // Observation/recycle only. Keep the legacy two-step bound and final
    // readiness check; original Ready/RetryReady custody never enters progress.
    for _ in 0..2 {
        if ready(owner) {
            break;
        }
        progress(owner)?;
    }
    Ok(ready(owner))
}

impl<P, C> ReceiptV1<P, C> {
    pub(super) fn issue_ready(&self) -> bool {
        matches!(self, Self::Ready | Self::RetryReady)
    }

    pub(super) fn retirement(&self) -> Option<RetirementV1> {
        match self {
            Self::Ready => Some(RetirementV1::Pristine),
            Self::RetryReady => Some(RetirementV1::CancelledOnly),
            Self::Recycled => Some(RetirementV1::Recycled),
            Self::Published(_)
            | Self::Completed(_)
            | Self::HandedToLower(_)
            | Self::RejectedUnpublished { .. }
            | Self::RejectedDisposed(_) => None,
        }
    }

    pub(super) fn issue(
        &mut self,
        issue: impl FnOnce() -> Result<P, Gfx942FixedDispatchSubmissionFailureV1>,
    ) -> Result<(), fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        self.issue_with_rejection(false, issue)
    }

    pub(super) fn issue_with_rejection(
        &mut self,
        preserve_rejection: bool,
        issue: impl FnOnce() -> Result<P, Gfx942FixedDispatchSubmissionFailureV1>,
    ) -> Result<(), fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        assert!(self.issue_ready());
        let Some(prior) = self.retirement() else {
            std::process::abort()
        };
        *self = Self::HandedToLower(HandoffV1::Issue);
        match issue() {
            Ok(batch) => *self = Self::Published(batch),
            Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                // Ordinary lower retry is returned only after exact epoch cancellation.
                *self = Self::RetryReady;
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error))
                if preserve_rejection =>
            {
                *self = Self::RejectedUnpublished { prior, error };
            }
            Err(failure) => return Err(failure.into_error()),
        }
        Ok(())
    }

    pub(super) fn rejected(
        &self,
    ) -> Option<(RetirementV1, &fe2o3_kfd::ComputeAqlQueueSessionErrorV1)> {
        match self {
            Self::RejectedUnpublished { prior, error } => Some((*prior, error)),
            _ => None,
        }
    }

    // Only actual lower control/DATA disposal may reach this transition.
    pub(super) fn mark_rejected_disposed(&mut self) {
        let Self::RejectedUnpublished { error, .. } =
            core::mem::replace(self, Self::HandedToLower(HandoffV1::Issue))
        else {
            std::process::abort()
        };
        *self = Self::RejectedDisposed(error);
    }

    pub(super) fn rejected_disposed_error(
        &self,
    ) -> Option<&fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::RejectedDisposed(error) => Some(error),
            _ => None,
        }
    }

    pub(super) fn poll<E>(
        &mut self,
        poll: impl FnOnce(P) -> Result<PollV1<P, C>, E>,
    ) -> Result<(), E> {
        assert!(matches!(self, Self::Published(_)));
        let Self::Published(batch) = core::mem::replace(self, Self::HandedToLower(HandoffV1::Poll))
        else {
            unreachable!()
        };
        *self = match poll(batch)? {
            PollV1::Completed(completed) => Self::Completed(completed),
            PollV1::Pending(pending) => Self::Published(pending),
        };
        Ok(())
    }

    pub(super) fn recycle<E>(
        &mut self,
        recycle: impl FnOnce(C) -> Result<(), (E, Option<C>)>,
    ) -> Result<bool, E> {
        assert!(matches!(self, Self::Completed(_)));
        let Self::Completed(completed) =
            core::mem::replace(self, Self::HandedToLower(HandoffV1::Recycle))
        else {
            unreachable!()
        };
        match recycle(completed) {
            Ok(()) => *self = Self::Recycled,
            Err((error, returned)) => {
                if let Some(completed) = returned {
                    *self = Self::Completed(completed);
                    return Ok(false);
                }
                return Err(error);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
