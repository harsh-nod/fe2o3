//! Callback results leave only after the same original scope has closed.
use super::*;

/// A local outcome observed only after complete original scope settlement.
/// These are inert errors, not native release or device-health authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeGfx942SettledFailureV1 {
    RejectedBeforePublication,
    DeviceUnavailableBeforeActivation { device_uid: u64 },
    Readback(RuntimeGfx942ReadbackErrorV1),
    CopyFailed { code: i64 },
}

impl RuntimeGfx942SettledFailureV1 {
    fn classify(error: RuntimeGfx942ScopeErrorV1) -> Result<Self, RuntimeGfx942ScopeErrorV1> {
        use RuntimeGfx942ScopeErrorV1 as E;
        match error {
            E::RejectedBeforePublication => Ok(Self::RejectedBeforePublication),
            E::DeviceUnavailableBeforeActivation { device_uid } => {
                Ok(Self::DeviceUnavailableBeforeActivation { device_uid })
            }
            E::Readback(error) => Ok(Self::Readback(error)),
            E::CopyFailed { code } => Ok(Self::CopyFailed { code }),
            error => Err(error),
        }
    }
}

/// Callback value and the first settled local failure in original roster order.
///
/// Construction requires original carrier, hold, copy and graph settlement and
/// closing the Context epoch. It does not certify successful graph nodes, native
/// health, reset independence, physical overlap, or closure of external owners.
/// Per-ticket observations and graph reports remain separate: callbacks may
/// retain those inert summaries in `R`. Cancellation keeps its existing per-ticket
/// result and is not newly classified as a final scope error.
#[derive(Debug)]
#[must_use = "inspect the settled local outcome before treating the callback value as successful"]
pub struct RuntimeGfx942SettledScopeV1<R> {
    value: R,
    completion: Result<(), RuntimeGfx942SettledFailureV1>,
}

impl<R> RuntimeGfx942SettledScopeV1<R> {
    pub fn completion_v1(&self) -> Result<(), &RuntimeGfx942SettledFailureV1> {
        self.completion.as_ref().copied()
    }

    pub fn into_parts_v1(self) -> (R, Result<(), RuntimeGfx942SettledFailureV1>) {
        (self.value, self.completion)
    }
}

// Reuse the identical finite scan/wake driver. Only its final local-outcome
// observation is deferred to the consuming closure below; progress errors are
// never classified or downgraded by this adapter.
struct ClosingDriver<'a, D>(&'a mut D);
impl<D: futures::Driver> futures::Driver for ClosingDriver<'_, D> {
    fn pending(&self) -> usize {
        self.0.pending()
    }
    fn progress(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        self.0.progress()
    }
    fn deadline(&self) -> Instant {
        self.0.deadline()
    }
    fn settled(&self) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        Ok(())
    }
}

// Fields drop in declaration order, including cancellation while the closing
// async driver is suspended. Unknown scope custody must stop before R drops.
struct ClosingFrame<'scope, 'env, B: RuntimeBackendV1, P, R> {
    scope: RuntimeGfx942GeneratedScopeV1<'scope, 'env, B, P>,
    value: Option<R>,
}

impl<B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>, P, R> ClosingFrame<'_, '_, B, P, R> {
    fn finish(
        self,
        driven: Result<(), RuntimeGfx942ScopeErrorV1>,
    ) -> Result<RuntimeGfx942SettledScopeV1<R>, RuntimeGfx942ScopeErrorV1> {
        let Self { scope, value } = self;
        let Some(value) = value else {
            std::process::abort()
        };
        scope.finish_settled_exit_v1(value, driven)
    }
}

impl<B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>, P>
    RuntimeGfx942GeneratedScopeV1<'_, '_, B, P>
{
    fn validate_settled_exit_v1(&self) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if Instant::now() >= self.deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        if self.slots.iter().any(|slot| {
            slot.lifecycle.unsettled()
                || slot.lifecycle.value.is_some()
                || (slot.lifecycle.phase == Phase::Settled) != slot.lifecycle.outcome.is_some()
        }) || self.copies.iter().any(|slot| {
            slot.unknown
                || !matches!(
                    slot.settled,
                    Some(RuntimePollV1::Succeeded | RuntimePollV1::Failed { .. })
                )
        }) || self.graph.as_ref().is_some_and(graph::Graph::unsettled)
        {
            return Err(RuntimeGfx942ScopeErrorV1::Unknown);
        }
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        // Also refuses terminal Contexts and any unreleased graph reservation.
        self.context
            .require_live()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        if self.context.has_unpublished_holds_v1() {
            return Err(RuntimeGfx942ScopeErrorV1::Context(
                RuntimeValidationErrorV1::ContextReserved.into(),
            ));
        }
        Ok(())
    }

    fn finish_settled_exit_v1<R>(
        self,
        value: R,
        driven: Result<(), RuntimeGfx942ScopeErrorV1>,
    ) -> Result<RuntimeGfx942SettledScopeV1<R>, RuntimeGfx942ScopeErrorV1> {
        let completion = driven.and_then(|()| {
            self.validate_settled_exit_v1()?;
            match self.settled_result_v1() {
                Ok(()) => Ok(Ok(())),
                Err(error) => RuntimeGfx942SettledFailureV1::classify(error).map(Err),
            }
        });
        // A pending/unknown owner's Drop aborts before either returning R or
        // dropping it on error. Successful Drop closes the original epoch and
        // disposes the remaining inert scope/graph fields before construction.
        let deadline = self.deadline;
        drop(self);
        match completion {
            Ok(_) if Instant::now() >= deadline => {
                drop(value);
                Err(RuntimeGfx942ScopeErrorV1::Deadline)
            }
            Ok(completion) => Ok(RuntimeGfx942SettledScopeV1 { value, completion }),
            Err(error) => {
                drop(value);
                Err(error)
            }
        }
    }
}

macro_rules! impl_settled_exit {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            /// Retains the callback value on a fully settled local failure.
            ///
            /// This additive alternative to the strict scope API returns a
            /// separate local outcome after draining original owners and closing
            /// the epoch. Structural errors still return `Err`; unsettled Drop
            /// still aborts. All Context unpublished holds must be gone. It does
            /// not relax process-global poison or certify any healthy device.
            ///
            /// ```
            /// use fe2o3_runtime::*;
            /// fn settled<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
            /// ) -> Result<RuntimeGfx942SettledScopeV1<u64>, RuntimeGfx942ScopeErrorV1> {
            ///     context.with_generated_gfx942_scope_settled_v1::<P, _>(1,
            ///         std::time::Instant::now() + std::time::Duration::from_secs(1), |_| 7)
            /// }
            /// ```
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
            ///     s: RuntimeStreamIdV1, p: P,
            /// ) {
            ///     let _ticket = c.with_generated_gfx942_scope_settled_v1(1,
            ///         std::time::Instant::now(), |scope|
            ///         scope.try_submit_v1(d, s, |_| Ok::<P, ()>(p)).unwrap());
            /// }
            /// ```
            pub fn with_generated_gfx942_scope_settled_v1<'env, P, R>(
                &'env mut self,
                capacity: usize,
                deadline: Instant,
                use_scope: impl for<'scope> FnOnce(
                    &mut RuntimeGfx942GeneratedScopeV1<'scope, 'env, $backend, P>,
                ) -> R,
            ) -> Result<RuntimeGfx942SettledScopeV1<R>, RuntimeGfx942ScopeErrorV1>
            where
                P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
            {
                let mut frame = ClosingFrame {
                    scope: self.new_generated_scope_v1(capacity, deadline)?,
                    value: None,
                };
                frame.value = Some(use_scope(&mut frame.scope));
                let driven = frame.scope.drain_owners_v1();
                frame.finish(driven)
            }

            /// Async settled-exit alternative using the same caller-driven scan
            /// and wake semantics as the strict async scope. No extra executor,
            /// timer or task is created. The invariant brand still cannot escape.
            ///
            /// ```
            /// use fe2o3_runtime::*;
            /// async fn settled<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
            /// ) -> Result<RuntimeGfx942SettledScopeV1<u64>, RuntimeGfx942ScopeErrorV1> {
            ///     context.with_generated_gfx942_scope_settled_async_v1::<P, _, _, _>(1,
            ///         std::time::Instant::now() + std::time::Duration::from_secs(1),
            ///         |_| std::future::ready(()), async |_| 7).await
            /// }
            /// ```
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// async fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
            ///     s: RuntimeStreamIdV1, p: P,
            /// ) {
            ///     let _future = c.with_generated_gfx942_scope_settled_async_v1(1,
            ///         std::time::Instant::now(), |_| std::future::ready(()), async |scope| {
            ///             let ticket = scope.try_submit_v1(d, s, |_| Ok::<P, ()>(p)).unwrap();
            ///             scope.completion_future_v1(&ticket).unwrap()
            ///         }).await;
            /// }
            /// ```
            pub async fn with_generated_gfx942_scope_settled_async_v1<'env, P, R, W, F>(
                &'env mut self,
                capacity: usize,
                deadline: Instant,
                wait: W,
                use_scope: impl for<'scope> AsyncFnOnce(
                    &mut RuntimeGfx942GeneratedScopeV1<'scope, 'env, $backend, P>,
                ) -> R,
            ) -> Result<RuntimeGfx942SettledScopeV1<R>, RuntimeGfx942ScopeErrorV1>
            where
                P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
                W: FnMut(Instant) -> F,
                F: std::future::Future<Output = ()>,
            {
                let mut frame = ClosingFrame {
                    scope: self.new_generated_scope_v1(capacity, deadline)?,
                    value: None,
                };
                frame.value = Some(use_scope(&mut frame.scope).await);
                let driven = futures::drive(&mut ClosingDriver(&mut frame.scope), wait).await;
                frame.finish(driven)
            }
        }
    };
}
impl_settled_exit!(KfdRuntimeBackendV1);
impl_settled_exit!(KfdMultiDeviceRuntimeBackendV1);

#[cfg(test)]
mod classification_tests {
    use super::*;

    #[test]
    fn only_exact_four_final_local_classes_are_accepted() {
        use RuntimeGfx942ScopeErrorV1 as E;
        use RuntimeGfx942SettledFailureV1 as S;
        let pairs = [
            (E::RejectedBeforePublication, S::RejectedBeforePublication),
            (
                E::DeviceUnavailableBeforeActivation { device_uid: 77 },
                S::DeviceUnavailableBeforeActivation { device_uid: 77 },
            ),
            (
                E::Readback(RuntimeGfx942ReadbackErrorV1::InvalidStorage),
                S::Readback(RuntimeGfx942ReadbackErrorV1::InvalidStorage),
            ),
            (E::CopyFailed { code: -19 }, S::CopyFailed { code: -19 }),
        ];
        for (error, expected) in pairs {
            assert_eq!(
                RuntimeGfx942SettledFailureV1::classify(error).unwrap(),
                expected
            );
        }
        for error in [
            E::Deadline,
            E::Unknown,
            E::CancelledBeforeSubmission,
            E::CancelledBeforePublication,
            E::Capacity,
            E::InvalidTicket,
            E::CompletionObserverTaken,
            E::Context(RuntimeValidationErrorV1::ContextTerminal.into()),
        ] {
            assert!(RuntimeGfx942SettledFailureV1::classify(error).is_err());
        }
    }
}
