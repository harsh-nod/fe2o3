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
    Published(P),
    Completed(C),
    Recycled,
    // The original queue retains native resources when a consuming call does
    // not return custody. This marker is not a replacement completion token.
    HandedToLower(HandoffV1),
}

pub(super) enum PollV1<P, C> {
    Pending(P),
    Completed(C),
}

impl<P, C> ReceiptV1<P, C> {
    pub(super) fn issue(
        &mut self,
        issue: impl FnOnce() -> Result<P, Gfx942FixedDispatchSubmissionFailureV1>,
    ) -> Result<(), fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        assert!(matches!(self, Self::Ready));
        *self = Self::HandedToLower(HandoffV1::Issue);
        match issue() {
            Ok(batch) => *self = Self::Published(batch),
            Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                *self = Self::Ready;
            }
            Err(failure) => return Err(failure.into_error()),
        }
        Ok(())
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
