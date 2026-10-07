//! Local cancellation and recoverable timeout observation. No device authority.

use super::*;
use fe2o3_runtime_model::{R62OperationActionV1 as Action, r62_operation_transition_v1};
use std::sync::atomic::AtomicU8;

use RuntimeAsyncOperationPhaseV1 as Phase;
/// Host-side phase only. `SubmissionStarted` is deliberately earlier than native
/// publication; `ObservationFinished` includes errors and is not GPU success.
/// `StoppedAfterSubmission` cannot establish whether publication occurred.
pub use fe2o3_runtime_model::R62OperationPhaseV1 as RuntimeAsyncOperationPhaseV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncCancelResultV1 {
    CancelledBeforeSubmission,
    AlreadyCancelled,
    /// The cancellation boundary has closed. This includes stopped operations.
    NotCancellable(Phase),
}

/// Cloneable, process-local identity and cancellation control for one operation.
///
/// It neither holds a backend nor grants release, completion, or retry authority.
/// Identity is meaningful only while a clone is retained; it is not a canonical
/// distributed operation ID and has no serializable address or native handle.
#[derive(Clone)]
pub struct RuntimeAsyncOperationControlV1 {
    phase: Arc<AtomicU8>,
}

impl RuntimeAsyncOperationControlV1 {
    pub(super) fn new() -> Self {
        Self {
            phase: Arc::new(AtomicU8::new(Phase::Queued as u8)),
        }
    }

    pub fn same_operation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.phase, &other.phase)
    }

    pub fn phase(&self) -> Phase {
        Self::decode(self.phase.load(Ordering::Acquire))
    }

    /// Races atomically with the owner immediately before context submission.
    /// Winning prevents this operation's submit closure from ever being called.
    /// It does not cancel dependencies or unrelated work, or free context-owned
    /// resources. The result future resolves when the owner processes/drops the
    /// record, which can be delayed by an externally blocking adapter.
    pub fn cancel_before_submission(&self) -> RuntimeAsyncCancelResultV1 {
        match self.transition(Action::Cancel) {
            Phase::Queued => RuntimeAsyncCancelResultV1::CancelledBeforeSubmission,
            Phase::CancelledBeforeSubmission => RuntimeAsyncCancelResultV1::AlreadyCancelled,
            phase => RuntimeAsyncCancelResultV1::NotCancellable(phase),
        }
    }

    fn decode(value: u8) -> Phase {
        match value {
            0 => Phase::Queued,
            1 => Phase::CancelledBeforeSubmission,
            2 => Phase::SubmissionStarted,
            3 => Phase::Observing,
            4 => Phase::ObservationFinished,
            5 => Phase::StoppedBeforeSubmission,
            6 => Phase::StoppedAfterSubmission,
            _ => unreachable!("private operation phase is valid"),
        }
    }

    // Return the phase at the linearization point, not a later observation.
    fn transition(&self, action: Action) -> Phase {
        let mut before = self.phase.load(Ordering::Acquire);
        loop {
            let phase = Self::decode(before);
            let after = r62_operation_transition_v1(phase, action) as u8;
            if before == after {
                return phase;
            }
            match self
                .phase
                .compare_exchange(before, after, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return phase,
                Err(current) => before = current,
            }
        }
    }

    pub(super) fn start_submission(&self) -> bool {
        self.transition(Action::Start) == Phase::Queued
    }

    pub(super) fn observing(&self) {
        self.transition(Action::AcceptSubmission);
    }

    pub(super) fn finish_observation(&self) {
        self.transition(Action::FinishObservation);
    }

    pub(super) fn stopped(&self) {
        self.transition(Action::Stop);
    }
}

/// Standard operation future with independent, cloneable cancellation control.
/// Dropping either handle does not cancel work or withdraw owner progress.
#[must_use = "dropping the observer does not cancel the operation"]
pub struct RuntimeAsyncTrackedOperationV1<A, E> {
    pub(super) future: RuntimeAsyncOperationFutureV1<A, E>,
    pub(super) control: RuntimeAsyncOperationControlV1,
}

impl<A, E> RuntimeAsyncTrackedOperationV1<A, E> {
    pub fn control(&self) -> RuntimeAsyncOperationControlV1 {
        self.control.clone()
    }

    /// Observe with an executor-supplied timer future. The runtime creates no
    /// timer thread. A ready operation is polled before the timer; a concurrently
    /// arriving result may instead be recovered from `TimedOut.operation`.
    /// Timeout never cancels, retries, releases custody, or changes GPU status.
    /// The supplied timer owns deadline/wakeup semantics; a future that never
    /// wakes cannot provide a deadline guarantee. Dropping this wrapper abandons
    /// only observation, exactly like dropping the underlying operation future.
    pub fn observe_with_timeout<T: Future<Output = ()>>(
        self,
        timeout: T,
    ) -> RuntimeAsyncTimedObservationV1<A, E, T> {
        RuntimeAsyncTimedObservationV1 {
            operation: Some(self),
            timeout: Box::pin(timeout),
        }
    }
}

impl<A, E> Future for RuntimeAsyncTrackedOperationV1<A, E> {
    type Output = Result<RuntimeAsyncOperationResultV1<A, E>, RuntimeAsyncEngineCallErrorV1>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.future).poll(context)
    }
}

/// Timeout preserves the sole result consumer for later awaiting or observation.
pub enum RuntimeAsyncTimeoutResultV1<A, E> {
    Completed(Result<RuntimeAsyncOperationResultV1<A, E>, RuntimeAsyncEngineCallErrorV1>),
    TimedOut {
        operation: RuntimeAsyncTrackedOperationV1<A, E>,
    },
}

#[must_use = "futures must be polled to observe completion or timeout"]
pub struct RuntimeAsyncTimedObservationV1<A, E, T> {
    operation: Option<RuntimeAsyncTrackedOperationV1<A, E>>,
    timeout: Pin<Box<T>>,
}

impl<A, E, T: Future<Output = ()>> Future for RuntimeAsyncTimedObservationV1<A, E, T> {
    type Output = RuntimeAsyncTimeoutResultV1<A, E>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let operation = self
            .operation
            .as_mut()
            .expect("timeout observer polled after completion");
        if let Poll::Ready(result) = Pin::new(operation).poll(context) {
            self.operation.take();
            return Poll::Ready(RuntimeAsyncTimeoutResultV1::Completed(result));
        }
        if self.timeout.as_mut().poll(context).is_ready() {
            let operation = self
                .operation
                .take()
                .expect("pending operation is retained");
            operation.future.clear_waker();
            return Poll::Ready(RuntimeAsyncTimeoutResultV1::TimedOut { operation });
        }
        Poll::Pending
    }
}
