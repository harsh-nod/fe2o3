//! Typed projection of the original completion; no second producer or wait protocol.

use super::*;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use fe2o3_runtime::{
    RuntimeAsyncEngineCallErrorV1, RuntimeAsyncGeneratedCompletionResultV1,
    RuntimeAsyncGeneratedCompletionV1, RuntimeGeneratedCompletionReceiptV1,
    RuntimeGfx942ReadbackErrorV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedRuntimeTypedOutputErrorV1 {
    BindingMismatch,
    OutputUnavailable,
    Custody,
}

impl From<GeneratedRuntimeTypedOutputErrorV1> for Error {
    fn from(error: GeneratedRuntimeTypedOutputErrorV1) -> Self {
        match error {
            GeneratedRuntimeTypedOutputErrorV1::BindingMismatch => Self::BindingMismatch,
            GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable => Self::OutputUnavailable,
            GeneratedRuntimeTypedOutputErrorV1::Custody => Self::Custody,
        }
    }
}

impl fmt::Display for GeneratedRuntimeTypedOutputErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Error::from(*self).fmt(f)
    }
}
impl std::error::Error for GeneratedRuntimeTypedOutputErrorV1 {}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedRuntimeTypedBindErrorV1 {
    Busy,
    Output(GeneratedRuntimeTypedOutputErrorV1),
}

impl fmt::Display for GeneratedRuntimeTypedBindErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("generated output binding is temporarily busy"),
            Self::Output(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for GeneratedRuntimeTypedBindErrorV1 {}

/// Binding rejection returns both unchanged observers; it does not cancel work.
pub struct GeneratedRuntimeTypedBindFailureV1<T: GeneratedDeviceScalarV1> {
    pub output: GeneratedRuntimeChargedResultV1<T>,
    pub completion: RuntimeAsyncGeneratedCompletionV1,
    pub error: GeneratedRuntimeTypedBindErrorV1,
}

/// The original charged output and its invocation's receipt for other outputs.
#[must_use = "typed storage retains its result credit until disposal"]
pub struct GeneratedRuntimeCompletedOutputV1<T: GeneratedDeviceScalarV1> {
    pub result: ChargedTypedResultV1<T>,
    pub receipt: RuntimeGeneratedCompletionReceiptV1,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedRuntimeTypedCompletionErrorV1 {
    Engine(RuntimeAsyncEngineCallErrorV1),
    Readback(RuntimeGfx942ReadbackErrorV1),
    Output(GeneratedRuntimeTypedOutputErrorV1),
}

impl fmt::Display for GeneratedRuntimeTypedCompletionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => error.fmt(f),
            Self::Readback(error) => error.fmt(f),
            Self::Output(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for GeneratedRuntimeTypedCompletionErrorV1 {}

/// Failure preserves the output observer and any successful completion receipt.
/// It does not establish that data is available or that native work may be retried.
pub struct GeneratedRuntimeTypedCompletionFailureV1<T: GeneratedDeviceScalarV1> {
    pub output: GeneratedRuntimeChargedResultV1<T>,
    pub receipt: Option<RuntimeGeneratedCompletionReceiptV1>,
    pub error: GeneratedRuntimeTypedCompletionErrorV1,
}

pub type GeneratedRuntimeTypedOutcomeV1<T> =
    Result<GeneratedRuntimeCompletedOutputV1<T>, GeneratedRuntimeTypedCompletionFailureV1<T>>;

/// A move-only, executor-neutral future for one original charged typed output.
/// Only the original runtime completion future may return `Pending`. Drop loses
/// observation, not runtime custody, and never cancels the invocation.
///
/// The completion receipt remains available for other heterogeneous outputs.
/// Stop suppresses typed delivery; ordinary drain preserves it. This API does
/// not add a per-invocation cancellation operation after activation.
///
/// ```no_run
/// use fe2o3_host::{
///     GeneratedRuntimeChargedResultV1, GeneratedRuntimeCompletedOutputV1,
///     GeneratedRuntimeTypedCompletionV1,
/// };
/// use fe2o3_runtime::{
///     KfdRuntimeBackendV1, RuntimeAsyncProgressHandleV1,
///     RuntimeAsyncReservedTicketV1, RuntimeStreamIdV1,
/// };
/// async fn launch(
///     handle: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>,
///     ticket: RuntimeAsyncReservedTicketV1,
///     stream: RuntimeStreamIdV1,
///     output: GeneratedRuntimeChargedResultV1<u32>,
/// ) -> Result<GeneratedRuntimeCompletedOutputV1<u32>, Box<dyn std::error::Error>> {
///     let completion = handle.try_activate_generated_v1(ticket, stream)?.await??;
///     Ok(output.bind_completion_v1(completion)?.await?)
/// }
/// fn blocking(
///     completion: GeneratedRuntimeTypedCompletionV1<u32>,
/// ) -> Result<GeneratedRuntimeCompletedOutputV1<u32>, Box<dyn std::error::Error>> {
///     Ok(completion.try_join()??)
/// }
/// fn observers_can_move_between_threads() {
///     fn require_send<T: Send>() {}
///     require_send::<GeneratedRuntimeTypedCompletionV1<u32>>();
///     require_send::<GeneratedRuntimeCompletedOutputV1<u32>>();
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fe2o3_host::GeneratedRuntimeTypedCompletionV1;
/// fn duplicate(value: GeneratedRuntimeTypedCompletionV1<u32>) { value.clone(); }
/// ```
#[must_use = "dropping this observer does not cancel the invocation"]
pub struct GeneratedRuntimeTypedCompletionV1<T: GeneratedDeviceScalarV1> {
    completion: Option<RuntimeAsyncGeneratedCompletionV1>,
    output: Option<GeneratedRuntimeChargedResultV1<T>>,
}

/// Owner-thread blocking rejection preserves the complete typed future.
pub struct GeneratedRuntimeTypedJoinFailureV1<T: GeneratedDeviceScalarV1> {
    pub completion: GeneratedRuntimeTypedCompletionV1<T>,
    pub error: RuntimeAsyncEngineCallErrorV1,
}

macro_rules! failure_display {
    ($name:ident) => {
        impl<T: GeneratedDeviceScalarV1> fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("error", &self.error)
                    .finish_non_exhaustive()
            }
        }
        impl<T: GeneratedDeviceScalarV1> fmt::Display for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.error.fmt(f)
            }
        }
        impl<T: GeneratedDeviceScalarV1> std::error::Error for $name<T> {}
    };
}
failure_display!(GeneratedRuntimeTypedBindFailureV1);
failure_display!(GeneratedRuntimeTypedCompletionFailureV1);
failure_display!(GeneratedRuntimeTypedJoinFailureV1);

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Binds the move-only result to its exact invocation before awaiting it.
    /// This allocates no storage or reply. Mismatch, contention or stale output
    /// return both unchanged owners, so the caller can correct or retry binding.
    pub fn bind_completion_v1(
        self,
        completion: RuntimeAsyncGeneratedCompletionV1,
    ) -> Result<GeneratedRuntimeTypedCompletionV1<T>, GeneratedRuntimeTypedBindFailureV1<T>> {
        match self.check_completion_binding_v1(|gate| completion.matches_owner(gate)) {
            Ok(()) => Ok(GeneratedRuntimeTypedCompletionV1 {
                completion: Some(completion),
                output: Some(self),
            }),
            Err(error) => Err(GeneratedRuntimeTypedBindFailureV1 {
                output: self,
                completion,
                error,
            }),
        }
    }

    pub(super) fn check_completion_binding_v1(
        &self,
        matches: impl FnOnce(&Arc<ResultReadyGateV1>) -> bool,
    ) -> Result<(), GeneratedRuntimeTypedBindErrorV1> {
        let state = match self.slot.state.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(GeneratedRuntimeTypedBindErrorV1::Busy);
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(GeneratedRuntimeTypedBindErrorV1::Output(
                    GeneratedRuntimeTypedOutputErrorV1::Custody,
                ));
            }
        };
        match &*state {
            OutputState::Prepared { gate, .. } if matches(gate) => Ok(()),
            OutputState::Taken | OutputState::Unavailable => {
                Err(GeneratedRuntimeTypedBindErrorV1::Output(
                    GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable,
                ))
            }
            _ => Err(GeneratedRuntimeTypedBindErrorV1::Output(
                GeneratedRuntimeTypedOutputErrorV1::BindingMismatch,
            )),
        }
    }
}

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeTypedCompletionV1<T> {
    /// Joins the original completion future and uses the same extraction as poll.
    /// No Context command, second reply or decoder is created. Owner-thread calls
    /// return this unchanged future; other calls have no deadline and may remain
    /// pending indefinitely if uncertain runtime custody is retained.
    pub fn try_join(
        mut self,
    ) -> Result<GeneratedRuntimeTypedOutcomeV1<T>, GeneratedRuntimeTypedJoinFailureV1<T>> {
        let completion = self.completion.take().expect("unconsumed typed completion");
        let output = self.output.take().expect("owned output observer");
        match join_with(
            completion,
            output,
            |completion| {
                completion
                    .try_join()
                    .map_err(|failure| (failure.completion, failure.error))
            },
            finish,
        ) {
            Ok(outcome) => Ok(outcome),
            Err((completion, output, error)) => {
                self.completion = Some(completion);
                self.output = Some(output);
                Err(GeneratedRuntimeTypedJoinFailureV1 {
                    completion: self,
                    error,
                })
            }
        }
    }
}

impl<T: GeneratedDeviceScalarV1> Future for GeneratedRuntimeTypedCompletionV1<T> {
    type Output = GeneratedRuntimeTypedOutcomeV1<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        poll_with(&mut this.completion, &mut this.output, cx, finish)
    }
}

pub(super) fn poll_with<F: Future + Unpin, O, R>(
    completion: &mut Option<F>,
    output: &mut Option<O>,
    cx: &mut Context<'_>,
    finish: impl FnOnce(O, F::Output) -> R,
) -> Poll<R> {
    let future = completion.as_mut().expect("unconsumed typed completion");
    let Poll::Ready(outcome) = Pin::new(future).poll(cx) else {
        return Poll::Pending;
    };
    drop(completion.take());
    Poll::Ready(finish(
        output.take().expect("owned output observer"),
        outcome,
    ))
}

pub(super) fn join_with<C, O, V, R, E>(
    completion: C,
    output: O,
    join: impl FnOnce(C) -> Result<V, (C, E)>,
    finish: impl FnOnce(O, V) -> R,
) -> Result<R, (C, O, E)> {
    match join(completion) {
        Ok(outcome) => Ok(finish(output, outcome)),
        Err((completion, error)) => Err((completion, output, error)),
    }
}

pub(super) fn finish<T: GeneratedDeviceScalarV1>(
    output: GeneratedRuntimeChargedResultV1<T>,
    outcome: RuntimeAsyncGeneratedCompletionResultV1,
) -> GeneratedRuntimeTypedOutcomeV1<T> {
    let receipt = match outcome {
        Ok(Ok(receipt)) => receipt,
        Ok(Err(error)) => {
            return Err(GeneratedRuntimeTypedCompletionFailureV1 {
                output,
                receipt: None,
                error: GeneratedRuntimeTypedCompletionErrorV1::Readback(error),
            });
        }
        Err(error) => {
            return Err(GeneratedRuntimeTypedCompletionFailureV1 {
                output,
                receipt: None,
                error: GeneratedRuntimeTypedCompletionErrorV1::Engine(error),
            });
        }
    };
    // Prebinding excludes foreign in-flight decoders. The exact decoder has
    // returned before this receipt, releasing every slot lock before publication.
    let result = output
        .slot
        .state
        .lock()
        .map_err(|_| GeneratedRuntimeTypedOutputErrorV1::Custody)
        .and_then(|mut state| {
            GeneratedRuntimeChargedResultV1::take_completed_state_v1(&mut state, |gate| {
                receipt.matches_owner(gate)
            })
        });
    match result {
        Ok(result) => Ok(GeneratedRuntimeCompletedOutputV1 { result, receipt }),
        Err(error) => Err(GeneratedRuntimeTypedCompletionFailureV1 {
            output,
            receipt: Some(receipt),
            error: GeneratedRuntimeTypedCompletionErrorV1::Output(error),
        }),
    }
}
