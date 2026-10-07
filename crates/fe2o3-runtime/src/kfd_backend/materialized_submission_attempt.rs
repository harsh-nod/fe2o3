//! Receipt storage shared with lower KFD qualification; no publication authority.

use super::{Gfx942DispatchBatchV1, Gfx942FixedDispatchSubmissionFailureV1};

pub(super) enum MaterializedSubmissionAttemptV1 {
    Unattempted,
    NativeOwned,
    Retryable,
    Published(Gfx942DispatchBatchV1<1>),
    #[cfg(test)]
    #[allow(dead_code)] // Used by runtime fault scripts, not lower receipt tests.
    ScriptedPublished,
}

impl MaterializedSubmissionAttemptV1 {
    pub(super) fn submit<E>(
        &mut self,
        operation: impl FnOnce() -> Result<Self, E>,
    ) -> Result<(), E> {
        *self = Self::NativeOwned;
        // Root the returned outcome before the outer native lane loan closes.
        *self = operation()?;
        Ok(())
    }

    pub(super) fn submit_classified(
        &mut self,
        operation: impl FnOnce() -> Result<
            Gfx942DispatchBatchV1<1>,
            Gfx942FixedDispatchSubmissionFailureV1,
        >,
    ) -> Result<(), Gfx942FixedDispatchSubmissionFailureV1> {
        self.submit(|| match operation() {
            Ok(batch) => Ok(Self::Published(batch)),
            Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                Ok(Self::Retryable)
            }
            Err(error) => Err(error),
        })
    }
}
