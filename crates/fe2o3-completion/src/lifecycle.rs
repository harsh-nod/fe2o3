use crate::Retained;

/// Observable state of one submitted asynchronous operation.
///
/// `Completed` and `Failed` both assert that the backend has established GPU
/// quiescence. `Failed` records a device-operation failure after quiescence;
/// callback and cancellation-request failures are reported separately and
/// never make the operation terminal.
#[derive(Debug, Eq, PartialEq)]
pub enum OperationState<E> {
    /// Work may be executing and all resources must remain retained.
    Submitted,
    /// Cancellation was requested, but work may still be executing.
    CancelRequested,
    /// The operation completed and the GPU can no longer access its resources.
    Completed,
    /// The operation failed after the GPU became quiescent.
    Failed(E),
}

impl<E> OperationState<E> {
    /// Returns whether the backend has established that GPU work is quiescent.
    pub fn is_quiescent(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_))
    }

    /// Returns the terminal state, if GPU quiescence has been established.
    pub fn terminal_state(&self) -> Option<TerminalState> {
        match self {
            Self::Submitted | Self::CancelRequested => None,
            Self::Completed => Some(TerminalState::Completed),
            Self::Failed(_) => Some(TerminalState::Failed),
        }
    }
}

/// A terminal operation state that does not borrow its failure payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalState {
    Completed,
    Failed,
}

/// An attempted transition from an already-terminal operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionError {
    pub terminal: TerminalState,
}

/// Failure to record a cancellation request.
#[derive(Debug, Eq, PartialEq)]
pub enum CancelRequestError<E> {
    /// The backend rejected or failed to submit the cancellation request.
    Request(E),
    /// GPU completion was already established before cancellation was requested.
    Terminal(TerminalState),
}

/// Failure while applying a terminal transition and notifying its observer.
#[derive(Debug, Eq, PartialEq)]
pub enum NotificationError<E> {
    /// The operation was already terminal, so the callback was not invoked.
    Transition(TransitionError),
    /// The terminal state was recorded, but the notification callback failed.
    Callback(E),
}

/// Why retained resources cannot be reclaimed by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReclaimError {
    /// Work may still be executing.
    NotQuiescent,
    /// The resources were already reclaimed exactly once.
    AlreadyReclaimed,
}

/// Runtime-independent lifecycle and retention policy for an owned operation.
///
/// A cancellation request is deliberately non-terminal: it never authorizes
/// resource reclamation. `complete` and `fail` may be called only after the
/// backend has independently established GPU quiescence. Once terminal,
/// resources can be reclaimed explicitly, or are released on drop. Dropping a
/// non-terminal lifecycle leaks its resources because the GPU may still refer
/// to them.
#[derive(Debug)]
#[must_use = "an operation lifecycle retains resources until GPU quiescence"]
pub struct OperationLifecycle<R, E> {
    resources: Retained<R>,
    state: OperationState<E>,
}

impl<R, E> OperationLifecycle<R, E> {
    /// Starts in `Submitted` with all participating resources retained.
    pub fn submitted(resources: R) -> Self {
        Self {
            resources: Retained::new(resources),
            state: OperationState::Submitted,
        }
    }

    pub fn state(&self) -> &OperationState<E> {
        &self.state
    }

    /// Runs the backend cancellation request at most once.
    ///
    /// Success moves `Submitted` to `CancelRequested` and returns `true`.
    /// Repeated requests are idempotent and return `false` without invoking
    /// `request` again. A request error or panic leaves the operation submitted.
    /// Neither successful nor failed cancellation establishes GPU completion.
    pub fn request_cancel_with<C>(
        &mut self,
        request: impl FnOnce() -> Result<(), C>,
    ) -> Result<bool, CancelRequestError<C>> {
        match self.state.terminal_state() {
            Some(terminal) => return Err(CancelRequestError::Terminal(terminal)),
            None if matches!(self.state, OperationState::CancelRequested) => return Ok(false),
            None => {}
        }

        request().map_err(CancelRequestError::Request)?;
        self.state = OperationState::CancelRequested;
        Ok(true)
    }

    /// Records successful completion after the backend establishes quiescence.
    pub fn complete(&mut self) -> Result<(), TransitionError> {
        self.transition_to(OperationState::Completed)
    }

    /// Records a device-operation failure after the backend establishes quiescence.
    pub fn fail(&mut self, error: E) -> Result<(), TransitionError> {
        self.transition_to(OperationState::Failed(error))
    }

    /// Records successful completion and invokes one terminal notification.
    ///
    /// The state becomes terminal before the callback runs. Therefore a
    /// callback error or panic cannot make resources in-flight again, and drop
    /// will still reclaim them exactly once.
    pub fn complete_with_notification<C>(
        &mut self,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), NotificationError<C>> {
        self.complete().map_err(NotificationError::Transition)?;
        notify(&self.state).map_err(NotificationError::Callback)
    }

    /// Records a quiescent operation failure and invokes one notification.
    pub fn fail_with_notification<C>(
        &mut self,
        error: E,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), NotificationError<C>> {
        self.fail(error).map_err(NotificationError::Transition)?;
        notify(&self.state).map_err(NotificationError::Callback)
    }

    /// Returns the retained resources exactly once after GPU quiescence.
    pub fn reclaim(&mut self) -> Result<R, ReclaimError> {
        if !self.state.is_quiescent() {
            return Err(ReclaimError::NotQuiescent);
        }
        self.resources
            .try_take()
            .ok_or(ReclaimError::AlreadyReclaimed)
    }

    fn transition_to(&mut self, next: OperationState<E>) -> Result<(), TransitionError> {
        if let Some(terminal) = self.state.terminal_state() {
            return Err(TransitionError { terminal });
        }
        self.state = next;
        Ok(())
    }
}

impl<R, E> Drop for OperationLifecycle<R, E> {
    fn drop(&mut self) {
        if self.state.is_quiescent()
            && let Some(resources) = self.resources.try_take()
        {
            drop(resources);
        }
    }
}
