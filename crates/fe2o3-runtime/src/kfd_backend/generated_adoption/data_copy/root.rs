//! In-place custody for one original generated DATA to device copy.
//!
//! This root carries owners, not an allocation/version witness. Its caller must
//! retain the admitted producer/source bracket, destination reservation and
//! original account until the returned destination is restored and committed.

pub(super) trait Owners {
    type Source;
    type Destination;
    type Buffer;
    type Submission;
    type Completed;
    type Frontier;
    type TransferFailure;
    type SubmitFailure;
    type PollFailure;
    type RetireFailure;
    type ReleaseFailure;
}

pub(super) trait Operations<O: Owners> {
    fn transfer(&mut self, source: O::Source) -> Result<O::Buffer, O::TransferFailure>;
    fn submit(
        &mut self,
        buffer: O::Buffer,
        destination: O::Destination,
    ) -> Result<O::Submission, O::SubmitFailure>;
    fn poll(
        &mut self,
        submission: O::Submission,
    ) -> Result<Poll<O::Submission, O::Completed>, O::PollFailure>;
    // Closed implementations perform only the infallible move out of the
    // original completed owner here, before either retirement operation.
    fn split_completed(completed: O::Completed) -> (O::Buffer, O::Destination, O::Frontier);
    fn retire(
        &mut self,
        destination: O::Destination,
        frontier: O::Frontier,
    ) -> Result<O::Destination, O::RetireFailure>;
    fn release(&mut self, buffer: O::Buffer) -> Result<(), O::ReleaseFailure>;
}

pub(super) enum Poll<S, C> {
    Pending(S),
    Completed(C),
}

enum State<O: Owners> {
    Empty,
    Input(O::Source, O::Destination),
    Transferring(O::Destination),
    Transferred(O::Buffer, O::Destination),
    Entered,
    Submitted(O::Submission),
    Completed(O::Completed),
    Retiring(O::Buffer),
    Retired(O::Buffer, O::Destination),
    Releasing(O::Destination),
    Released(O::Destination),
    TransferFailed(O::TransferFailure, O::Destination),
    SubmitFailed(O::SubmitFailure),
    PollFailed(O::PollFailure),
    RetireFailed(O::RetireFailure, O::Buffer),
    ReleaseFailed(O::ReleaseFailure, O::Destination),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kfd_backend::generated_adoption) enum Progress {
    Transferred,
    Submitted,
    Pending,
    Completed,
    Retired,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kfd_backend::generated_adoption) enum Refusal {
    Phase,
    Transfer,
    Submit,
    Poll,
    Retire,
    Release,
}

/// Must remain in a containing root across every consuming lower operation.
/// A panic/ambiguous failure never permits cancellation, retry or ordinary Drop.
pub(super) struct PendingCopy<O: Owners> {
    state: State<O>,
}

impl<O: Owners> PendingCopy<O> {
    pub(super) fn empty() -> Self {
        Self {
            state: State::Empty,
        }
    }

    /// No native operation, callback or allocation occurs during installation.
    pub(super) fn install(
        &mut self,
        source: O::Source,
        destination: O::Destination,
    ) -> Result<(), (O::Source, O::Destination)> {
        if !matches!(self.state, State::Empty) {
            return Err((source, destination));
        }
        self.state = State::Input(source, destination);
        Ok(())
    }

    pub(super) fn is_empty(&self) -> bool {
        matches!(self.state, State::Empty)
    }

    pub(super) fn is_released(&self) -> bool {
        matches!(self.state, State::Released(_))
    }

    /// Cancellation is supported only before transfer has been entered.
    pub(super) fn cancel_unentered(&mut self) -> Option<(O::Source, O::Destination)> {
        if !matches!(self.state, State::Input(..)) {
            return None;
        }
        let State::Input(source, destination) = core::mem::replace(&mut self.state, State::Empty)
        else {
            std::process::abort()
        };
        Some((source, destination))
    }

    pub(super) fn advance<A: Operations<O>>(&mut self, ops: &mut A) -> Result<Progress, Refusal> {
        match &self.state {
            State::Input(..)
            | State::Transferred(..)
            | State::Submitted(_)
            | State::Completed(_)
            | State::Retired(..) => {}
            _ => return Err(Refusal::Phase),
        }
        // Mark entry before moving an owner into a consuming lower operation.
        // Returned failure objects are rooted whole before any diagnostics.
        match core::mem::replace(&mut self.state, State::Entered) {
            State::Input(source, destination) => {
                self.state = State::Transferring(destination);
                let result = ops.transfer(source);
                let State::Transferring(destination) =
                    core::mem::replace(&mut self.state, State::Entered)
                else {
                    std::process::abort()
                };
                match result {
                    Ok(buffer) => {
                        self.state = State::Transferred(buffer, destination);
                        Ok(Progress::Transferred)
                    }
                    Err(failure) => {
                        self.state = State::TransferFailed(failure, destination);
                        Err(Refusal::Transfer)
                    }
                }
            }
            State::Transferred(buffer, destination) => match ops.submit(buffer, destination) {
                Ok(submission) => {
                    self.state = State::Submitted(submission);
                    Ok(Progress::Submitted)
                }
                Err(failure) => {
                    self.state = State::SubmitFailed(failure);
                    Err(Refusal::Submit)
                }
            },
            State::Submitted(submission) => match ops.poll(submission) {
                Ok(Poll::Pending(submission)) => {
                    self.state = State::Submitted(submission);
                    Ok(Progress::Pending)
                }
                Ok(Poll::Completed(completed)) => {
                    self.state = State::Completed(completed);
                    Ok(Progress::Completed)
                }
                Err(failure) => {
                    self.state = State::PollFailed(failure);
                    Err(Refusal::Poll)
                }
            },
            State::Completed(completed) => {
                let (buffer, destination, frontier) = A::split_completed(completed);
                self.state = State::Retiring(buffer);
                let result = ops.retire(destination, frontier);
                let State::Retiring(buffer) = core::mem::replace(&mut self.state, State::Entered)
                else {
                    std::process::abort()
                };
                match result {
                    Ok(destination) => {
                        self.state = State::Retired(buffer, destination);
                        Ok(Progress::Retired)
                    }
                    Err(failure) => {
                        self.state = State::RetireFailed(failure, buffer);
                        Err(Refusal::Retire)
                    }
                }
            }
            State::Retired(buffer, destination) => {
                self.state = State::Releasing(destination);
                let result = ops.release(buffer);
                let State::Releasing(destination) =
                    core::mem::replace(&mut self.state, State::Entered)
                else {
                    std::process::abort()
                };
                match result {
                    Ok(()) => {
                        self.state = State::Released(destination);
                        Ok(Progress::Released)
                    }
                    Err(failure) => {
                        self.state = State::ReleaseFailed(failure, destination);
                        Err(Refusal::Release)
                    }
                }
            }
            _ => std::process::abort(),
        }
    }

    /// This consumes only the copy root. Context destination version success
    /// still requires restoration, closing currentness and its original journal.
    pub(super) fn take_released(&mut self) -> Option<O::Destination> {
        if !self.is_released() {
            return None;
        }
        let State::Released(destination) = core::mem::replace(&mut self.state, State::Empty) else {
            std::process::abort()
        };
        Some(destination)
    }

    #[cfg(test)]
    pub(super) fn retained_retirement_buffer(&self) -> Option<&O::Buffer> {
        match &self.state {
            State::Retiring(buffer) | State::RetireFailed(_, buffer) => Some(buffer),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn retained_retirement_failure(&self) -> Option<&O::RetireFailure> {
        match &self.state {
            State::RetireFailed(failure, _) => Some(failure),
            _ => None,
        }
    }
}

impl<O: Owners> Drop for PendingCopy<O> {
    fn drop(&mut self) {
        if !self.is_empty() {
            std::process::abort();
        }
    }
}
