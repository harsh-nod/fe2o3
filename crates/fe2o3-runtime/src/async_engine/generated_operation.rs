//! Finite owner-local preparation. Parked custody is never a native launch roster.

use super::*;
use crate::{KfdRuntimeBackendV1, RuntimeDeviceIdV1, RuntimeGfx942PreparationErrorV1};
use crate::{RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedReservationErrorV1};
use operation::{EngineOperationFactoryV1, EngineOperationV1, stop_reply};

pub(super) mod adoption;
mod reservation;
pub(super) use reservation::ReserveCommandV1;
pub use reservation::*;

type Reserve<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut P,
) -> Result<
    crate::generated_source::GeneratedHostRosterV1,
    RuntimeGfx942GeneratedReservationErrorV1,
>;

pub(super) struct PreparedKeyV1 {
    pub(super) context_generation: u64,
}

/// Linear reference to retained, nonexecuting owner-local preparation.
///
/// This carries no payload, native address or execution authority. Dropping it
/// does not discard storage; unreachable prepared owners remain bounded until
/// owned shutdown. Explicit discard requires the original live engine.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::RuntimeAsyncPreparedTicketV1;
/// fn duplicate(ticket: RuntimeAsyncPreparedTicketV1) { let _ = ticket.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_runtime::RuntimeAsyncPreparedTicketV1;
/// fn extract(ticket: RuntimeAsyncPreparedTicketV1) { let _ = ticket.key; }
/// ```
#[must_use]
pub struct RuntimeAsyncPreparedTicketV1 {
    pub(super) key: Arc<PreparedKeyV1>,
}

impl fmt::Debug for RuntimeAsyncPreparedTicketV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeAsyncPreparedTicketV1")
            .field("context_generation", &self.key.context_generation)
            .finish_non_exhaustive()
    }
}

/// Finite host-preparation future, not a GPU submission or completion future.
/// Cancellation races only with entry to the preparation callback. A ready
/// ticket remains in the owner's custody independently of the result observer.
#[must_use]
pub struct RuntimeAsyncPreparationV1<E> {
    future: RuntimeAsyncCommandFutureV1<Result<RuntimeAsyncPreparedTicketV1, E>>,
    control: RuntimeAsyncOperationControlV1,
}

impl<E> RuntimeAsyncPreparationV1<E> {
    pub fn control(&self) -> RuntimeAsyncOperationControlV1 {
        self.control.clone()
    }
}

impl<E> Future for RuntimeAsyncPreparationV1<E> {
    type Output = Result<Result<RuntimeAsyncPreparedTicketV1, E>, RuntimeAsyncEngineCallErrorV1>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.future).poll(cx)
    }
}

/// Failed discard admission returns the unchanged ticket for a later attempt.
#[derive(Debug)]
pub struct RuntimeAsyncPreparedDiscardFailureV1 {
    pub ticket: RuntimeAsyncPreparedTicketV1,
    pub error: RuntimeAsyncEngineCallErrorV1,
}

impl fmt::Display for RuntimeAsyncPreparedDiscardFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "prepared discard admission failed: {}", self.error)
    }
}

impl Error for RuntimeAsyncPreparedDiscardFailureV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

type Prepare<B, P, E> = Box<dyn FnOnce(&mut RuntimeContextV1<B>) -> Result<P, E> + Send>;
type PreparationReply<E> = owned::Reply<Result<RuntimeAsyncPreparedTicketV1, E>>;

struct PreparationFactory<B: RuntimeBackendV1, P, E> {
    prepare: Option<Prepare<B, P, E>>,
    reply: Option<PreparationReply<E>>,
    key: Arc<PreparedKeyV1>,
    control: Option<RuntimeAsyncOperationControlV1>,
    reserve: Option<Reserve<B, P>>,
    adoption: Option<adoption::AdoptionHooksV1<B, P>>,
}

struct PreparationDriver<B: RuntimeBackendV1, P, E> {
    // Prepared storage owns its decoder and credits; it never leaves this driver.
    prepared: Option<P>,
    prepare: Option<Prepare<B, P, E>>,
    reply: Option<PreparationReply<E>>,
    key: Arc<PreparedKeyV1>,
    control: RuntimeAsyncOperationControlV1,
    reserve: Option<Reserve<B, P>>,
    roster: Option<crate::generated_source::GeneratedHostRosterV1>,
    completion: Option<owned::Reply<()>>,
    adoption: Option<adoption::AdoptionHooksV1<B, P>>,
    unpublished: Option<adoption::UnpublishedAdoptionV1>,
}

impl<B: RuntimeBackendV1 + 'static, P: 'static, E: Send + 'static> EngineOperationFactoryV1<B>
    for PreparationFactory<B, P, E>
{
    fn materialize(&mut self) -> Box<dyn EngineOperationV1<B>> {
        Box::new(PreparationDriver {
            prepared: None,
            prepare: Some(
                self.prepare
                    .take()
                    .expect("one preparation materialization"),
            ),
            key: Arc::clone(&self.key),
            control: self
                .control
                .take()
                .expect("one preparation control transfer"),
            reply: self.reply.take(),
            reserve: self.reserve,
            roster: None,
            completion: None,
            adoption: self.adoption,
            unpublished: None,
        })
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        stop_reply(&mut self.reply, self.control.as_ref(), error);
    }
}

impl<B: RuntimeBackendV1, P, E> Drop for PreparationFactory<B, P, E> {
    fn drop(&mut self) {
        stop_reply(
            &mut self.reply,
            self.control.as_ref(),
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        );
    }
}

impl<B: RuntimeBackendV1, P, E> EngineOperationV1<B> for PreparationDriver<B, P, E> {
    fn advance(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        if self.unpublished.is_some() {
            return self.advance_unpublished_v1(context);
        }
        if self.key.context_generation != context.capture_context_generation_v1() {
            self.reject(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket);
            return true;
        }
        if !self.control.start_submission() {
            self.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
            return true;
        }
        let prepare = self.prepare.take().expect("preparation is called once");
        match prepare(context) {
            Ok(prepared) => {
                self.prepared = Some(prepared);
                false
            }
            Err(error) => {
                self.control.finish_observation();
                if let Some(mut reply) = self.reply.take() {
                    reply.complete(Ok(Err(error)));
                }
                true
            }
        }
    }

    fn stream(&self) -> Option<RuntimeStreamIdV1> {
        None
    }

    fn prepared_key(&self) -> Option<&Arc<PreparedKeyV1>> {
        self.prepared
            .as_ref()
            .filter(|_| self.roster.is_none() && self.unpublished.is_none())
            .map(|_| &self.key)
    }

    fn reserved_key(&self) -> Option<&Arc<PreparedKeyV1>> {
        self.roster
            .as_ref()
            .filter(|_| self.unpublished.is_none())
            .map(|_| &self.key)
    }

    fn preflight_adoption(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        let hooks = self
            .adoption
            .as_ref()
            .ok_or(crate::RuntimeValidationErrorV1::Unsupported)?;
        (hooks.preflight)(
            context,
            self.prepared.as_ref().expect("reserved payload"),
            self.roster.as_ref().expect("reserved roster"),
            stream,
        )
    }

    fn activate_adoption(
        &mut self,
        hold: crate::context::ContextUnpublishedHoldV1,
        ticket: RuntimeAsyncReservedTicketV1,
    ) {
        self.unpublished = Some(adoption::UnpublishedAdoptionV1::new(hold, ticket));
    }

    fn retire_unpublished(
        &mut self,
        context: &mut RuntimeContextV1<B>,
    ) -> Result<bool, RuntimeErrorV1<B::Error>> {
        self.retire_unpublished_v1(context)
    }

    fn reserve_generated(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        completion: &mut Option<owned::Reply<()>>,
    ) -> Result<(), RuntimeGfx942GeneratedReservationErrorV1> {
        let reserve = self
            .reserve
            .ok_or(RuntimeGfx942GeneratedReservationErrorV1::UnsupportedPreparation)?;
        let prepared = self.prepared.as_mut().expect("parked preparation");
        let roster = reserve(context, prepared)?;
        self.roster = Some(roster);
        self.completion = completion.take();
        Ok(())
    }

    fn complete_preparation(&mut self) {
        self.control.finish_observation();
        if let Some(mut reply) = self.reply.take() {
            reply.complete(Ok(Ok(RuntimeAsyncPreparedTicketV1 {
                key: Arc::clone(&self.key),
            })));
        }
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        stop_reply(&mut self.reply, Some(&self.control), error);
        stop_reply(&mut self.completion, None, error);
        drop(self.prepare.take());
    }
}

impl<B: RuntimeBackendV1, P, E> Drop for PreparationDriver<B, P, E> {
    fn drop(&mut self) {
        stop_reply(
            &mut self.completion,
            None,
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        );
        stop_reply(
            &mut self.reply,
            Some(&self.control),
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        );
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    // Only the public immutable native scope below exposes this private driver.
    pub(super) fn enqueue_preparation_v1<P: 'static, E: Send + 'static>(
        &self,
        prepare: Prepare<B, P, E>,
    ) -> Result<RuntimeAsyncPreparationV1<E>, RuntimeAsyncEngineCallErrorV1> {
        self.enqueue_preparation_with_reservation_v1(prepare, None)
    }

    pub(super) fn enqueue_preparation_with_reservation_v1<P: 'static, E: Send + 'static>(
        &self,
        prepare: Prepare<B, P, E>,
        reserve: Option<Reserve<B, P>>,
    ) -> Result<RuntimeAsyncPreparationV1<E>, RuntimeAsyncEngineCallErrorV1> {
        self.enqueue_preparation_with_adoption_v1(prepare, reserve, None)
    }

    pub(super) fn enqueue_preparation_with_adoption_v1<P: 'static, E: Send + 'static>(
        &self,
        prepare: Prepare<B, P, E>,
        reserve: Option<Reserve<B, P>>,
        adoption: Option<adoption::AdoptionHooksV1<B, P>>,
    ) -> Result<RuntimeAsyncPreparationV1<E>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (reply, future) = owned::Reply::budgeted_pair(&self.observer.reply_budget)?;
        let control = RuntimeAsyncOperationControlV1::new();
        let factory = PreparationFactory {
            prepare: Some(prepare),
            reply: Some(reply),
            key: Arc::new(PreparedKeyV1 {
                context_generation: self.observer.context_generation,
            }),
            control: Some(control.clone()),
            reserve,
            adoption,
        };
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::Operation(Box::new(factory)))
        {
            Ok(()) => Ok(RuntimeAsyncPreparationV1 { future, control }),
            Err(TrySendError::Full(_)) => Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull),
            Err(TrySendError::Disconnected(_)) => Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
        }
    }

    /// Disposes one exact, never-adopted preparation on its owner thread.
    /// Success follows actual host storage disposal, not native completion.
    /// Queue/reply rejection preserves the ticket; a terminal owner retains its
    /// custody through the existing shutdown/quarantine path.
    pub fn try_discard_prepared_v1(
        &self,
        ticket: RuntimeAsyncPreparedTicketV1,
    ) -> Result<RuntimeAsyncCommandFutureV1<()>, RuntimeAsyncPreparedDiscardFailureV1> {
        let error = if self.observer.is_worker_thread() {
            Some(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
        } else if ticket.key.context_generation != self.observer.context_generation {
            Some(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(RuntimeAsyncPreparedDiscardFailureV1 { ticket, error });
        }
        let (reply, future) = match owned::Reply::budgeted_pair(&self.observer.reply_budget) {
            Ok(pair) => pair,
            Err(error) => return Err(RuntimeAsyncPreparedDiscardFailureV1 { ticket, error }),
        };
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::DiscardPrepared { ticket, reply })
        {
            Ok(()) => Ok(future),
            Err(error) => {
                let (command, error) = match error {
                    TrySendError::Full(command) => {
                        (command, RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
                    }
                    TrySendError::Disconnected(command) => {
                        (command, RuntimeAsyncEngineCallErrorV1::EngineStopped)
                    }
                };
                let RuntimeAsyncEngineCommandV1::DiscardPrepared { ticket, .. } = command else {
                    unreachable!()
                };
                Err(RuntimeAsyncPreparedDiscardFailureV1 { ticket, error })
            }
        }
    }
}

impl RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1> {
    /// Nonexecuting generated-host bridge. Retains a typed source/readback
    /// adapter without exposing payload extraction or native execution.
    #[doc(hidden)]
    pub fn try_prepare_generated_gfx942_v1<
        P: RuntimeGfx942GeneratedCarrierV1 + 'static,
        E: Send + 'static,
    >(
        &self,
        device: RuntimeDeviceIdV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<P, E> + Send + 'static,
    ) -> Result<
        RuntimeAsyncPreparationV1<RuntimeGfx942PreparationErrorV1<E>>,
        RuntimeAsyncEngineCallErrorV1,
    > {
        self.enqueue_preparation_with_reservation_v1(
            Box::new(move |context| context.with_gfx942_preparation_device_v1(device, prepare)),
            Some(RuntimeContextV1::reserve_gfx942_prepared_v1::<P>),
        )
    }

    /// Runtime-defined bridge for generated-host preparation. The callback gets
    /// only an immutable checked-device borrow, bracketed by full currentness.
    /// The complete Context-bound result remains owner-local; no native storage,
    /// launch registration, output decoding or payload extraction is added.
    ///
    /// ```compile_fail,E0277
    /// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeAsyncProgressHandleV1, RuntimeDeviceIdV1};
    /// fn not_send(handle: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>, device: RuntimeDeviceIdV1) {
    ///     let local = std::rc::Rc::new(());
    ///     let _ = handle.try_prepare_gfx942_v1(device, move |_| Ok::<_, ()>(local));
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeAsyncProgressHandleV1, RuntimeDeviceIdV1};
    /// fn escape(handle: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>, device: RuntimeDeviceIdV1) {
    ///     let _ = handle.try_prepare_gfx942_v1(device, |owner| Ok::<_, ()>(owner));
    /// }
    /// ```
    #[doc(hidden)]
    pub fn try_prepare_gfx942_v1<P: 'static, E: Send + 'static>(
        &self,
        device: RuntimeDeviceIdV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<P, E> + Send + 'static,
    ) -> Result<
        RuntimeAsyncPreparationV1<RuntimeGfx942PreparationErrorV1<E>>,
        RuntimeAsyncEngineCallErrorV1,
    > {
        self.enqueue_preparation_v1(Box::new(move |context| {
            context.with_gfx942_preparation_device_v1(device, prepare)
        }))
    }
}
