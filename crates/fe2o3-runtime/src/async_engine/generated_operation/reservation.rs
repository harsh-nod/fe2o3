//! Finite host-only reservation. Native adoption is a later, non-discardable state.

use super::*;

/// Exact host-only reserved custody. Dropping the ticket does not dispose it.
/// No native work or externally awaitable completion is created by reservation.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::RuntimeAsyncReservedTicketV1;
/// fn duplicate(ticket: RuntimeAsyncReservedTicketV1) { let _ = ticket.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_runtime::RuntimeAsyncReservedTicketV1;
/// fn extract(ticket: RuntimeAsyncReservedTicketV1) { let _ = ticket.completion; }
/// ```
#[must_use]
pub struct RuntimeAsyncReservedTicketV1 {
    pub(in crate::async_engine) key: Arc<PreparedKeyV1>,
    #[allow(
        dead_code,
        reason = "private completion consumer retained until native issue exists"
    )]
    completion: RuntimeAsyncCommandFutureV1<()>,
}

impl fmt::Debug for RuntimeAsyncReservedTicketV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeAsyncReservedTicketV1")
            .field("context_generation", &self.key.context_generation)
            .finish_non_exhaustive()
    }
}

/// An ordinary pre-effect failure preserves the original preparation ticket.
#[derive(Debug)]
pub struct RuntimeAsyncPreparedReservationFailureV1 {
    pub ticket: RuntimeAsyncPreparedTicketV1,
    pub error: RuntimeGfx942GeneratedReservationErrorV1,
}

impl fmt::Display for RuntimeAsyncPreparedReservationFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for RuntimeAsyncPreparedReservationFailureV1 {}

pub type RuntimeAsyncGeneratedReservationV1 = RuntimeAsyncCommandFutureV1<
    Result<RuntimeAsyncReservedTicketV1, RuntimeAsyncPreparedReservationFailureV1>,
>;

#[derive(Debug)]
pub struct RuntimeAsyncReservedDiscardFailureV1 {
    pub ticket: RuntimeAsyncReservedTicketV1,
    pub error: RuntimeAsyncEngineCallErrorV1,
}

impl fmt::Display for RuntimeAsyncReservedDiscardFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for RuntimeAsyncReservedDiscardFailureV1 {}

pub(in crate::async_engine) struct ReserveCommandV1 {
    ticket: Option<RuntimeAsyncPreparedTicketV1>,
    reply: Option<
        owned::Reply<
            Result<RuntimeAsyncReservedTicketV1, RuntimeAsyncPreparedReservationFailureV1>,
        >,
    >,
    completion: Option<owned::Reply<()>>,
    consumer: Option<RuntimeAsyncCommandFutureV1<()>>,
}

impl ReserveCommandV1 {
    fn reject(&mut self, error: RuntimeGfx942GeneratedReservationErrorV1) {
        if let Some(ticket) = self.ticket.take()
            && let Some(mut reply) = self.reply.take()
        {
            reply.complete(Ok(Err(RuntimeAsyncPreparedReservationFailureV1 {
                ticket,
                error,
            })));
        }
    }

    pub(in crate::async_engine) fn run<B: RuntimeBackendV1>(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) {
        let ticket = self.ticket.as_ref().expect("queued preparation ticket");
        let result = if context.is_terminal() {
            Ok(Err(RuntimeGfx942GeneratedReservationErrorV1::Engine(
                RuntimeAsyncEngineCallErrorV1::EngineStopped,
            )))
        } else if ticket.key.context_generation != context.capture_context_generation_v1() {
            Ok(Err(RuntimeGfx942GeneratedReservationErrorV1::Engine(
                RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket,
            )))
        } else {
            catch_unwind(AssertUnwindSafe(|| {
                operations.reserve_generated(context, &ticket.key, &mut self.completion)
            }))
        };
        match result {
            Ok(Ok(())) => {
                let ticket = self.ticket.take().expect("reserved exact ticket");
                let reserved = RuntimeAsyncReservedTicketV1 {
                    key: ticket.key,
                    completion: self.consumer.take().expect("reserved completion consumer"),
                };
                if let Some(mut reply) = self.reply.take() {
                    reply.complete(Ok(Ok(reserved)));
                }
            }
            Ok(Err(error)) => self.reject(error),
            Err(payload) => {
                core::mem::forget(payload);
                context.quarantine_after_async_command_panic_v1();
                // Installation may have started. Keep the owner rooted and do
                // not return a ticket suggesting the preparation is retryable.
                drop(self.ticket.take());
                if let Some(mut reply) = self.reply.take() {
                    reply.complete(Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked));
                }
            }
        }
    }
}

impl Drop for ReserveCommandV1 {
    fn drop(&mut self) {
        self.reject(RuntimeGfx942GeneratedReservationErrorV1::Engine(
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        ));
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    /// Reserves complete host readback and a separate private completion cell.
    /// The outer future is finite; success still submits no GPU work. Ordinary
    /// failures and queued Stop return the unchanged preparation ticket. Panic
    /// returns an outer error and retains uncertain owner custody for shutdown.
    pub fn try_reserve_prepared_v1(
        &self,
        ticket: RuntimeAsyncPreparedTicketV1,
    ) -> Result<RuntimeAsyncGeneratedReservationV1, RuntimeAsyncPreparedReservationFailureV1> {
        let failure = |ticket, error| RuntimeAsyncPreparedReservationFailureV1 {
            ticket,
            error: RuntimeGfx942GeneratedReservationErrorV1::Engine(error),
        };
        if self.observer.is_worker_thread() {
            return Err(failure(
                ticket,
                RuntimeAsyncEngineCallErrorV1::ReentrantCall,
            ));
        }
        if ticket.key.context_generation != self.observer.context_generation {
            return Err(failure(
                ticket,
                RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket,
            ));
        }
        let (reply, future) = match owned::Reply::budgeted_pair(&self.observer.reply_budget) {
            Ok(pair) => pair,
            Err(error) => return Err(failure(ticket, error)),
        };
        let (completion, consumer) = match owned::Reply::budgeted_pair(&self.observer.reply_budget)
        {
            Ok(pair) => pair,
            Err(error) => return Err(failure(ticket, error)),
        };
        let command = ReserveCommandV1 {
            ticket: Some(ticket),
            reply: Some(reply),
            completion: Some(completion),
            consumer: Some(consumer),
        };
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::ReservePrepared(command))
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
                let RuntimeAsyncEngineCommandV1::ReservePrepared(mut command) = command else {
                    unreachable!()
                };
                Err(failure(
                    command.ticket.take().expect("unqueued ticket"),
                    error,
                ))
            }
        }
    }

    /// Disposes exact host-only reserved custody on its owner thread.
    pub fn try_discard_reserved_v1(
        &self,
        ticket: RuntimeAsyncReservedTicketV1,
    ) -> Result<RuntimeAsyncCommandFutureV1<()>, RuntimeAsyncReservedDiscardFailureV1> {
        let error = if self.observer.is_worker_thread() {
            Some(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
        } else if ticket.key.context_generation != self.observer.context_generation {
            Some(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(RuntimeAsyncReservedDiscardFailureV1 { ticket, error });
        }
        let (reply, future) = match owned::Reply::budgeted_pair(&self.observer.reply_budget) {
            Ok(pair) => pair,
            Err(error) => return Err(RuntimeAsyncReservedDiscardFailureV1 { ticket, error }),
        };
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::DiscardReserved { ticket, reply })
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
                let RuntimeAsyncEngineCommandV1::DiscardReserved { ticket, .. } = command else {
                    unreachable!()
                };
                Err(RuntimeAsyncReservedDiscardFailureV1 { ticket, error })
            }
        }
    }
}
