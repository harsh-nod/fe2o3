//! Private adoption, issue and original-owner completion lifecycle.

use super::*;
use crate::RuntimeValidationErrorV1;
use crate::context::ContextUnpublishedHoldV1;

type AdoptionPreflightV1<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &P,
    &crate::generated_source::GeneratedHostRosterV1,
    RuntimeStreamIdV1,
) -> Result<(), RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;
type AdoptV1<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut P,
    &crate::generated_source::GeneratedHostRosterV1,
    &ContextUnpublishedHoldV1,
) -> Result<(), RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;
type AdoptionRetireV1<B> = fn(
    &mut RuntimeContextV1<B>,
    &ContextUnpublishedHoldV1,
) -> Result<(), RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;
type AdoptionReadyV1<B> = fn(
    &RuntimeContextV1<B>,
    &ContextUnpublishedHoldV1,
) -> Result<bool, RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;

type IssuedRetireV1<B> = fn(
    &mut RuntimeContextV1<B>,
    &ContextUnpublishedHoldV1,
) -> Result<bool, RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;
type IssueProgressV1<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut P,
    &crate::generated_source::GeneratedHostRosterV1,
    &ContextUnpublishedHoldV1,
) -> Result<bool, RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;

pub(in crate::async_engine) struct IssueHooksV1<B: RuntimeBackendV1, P> {
    pub progress: IssueProgressV1<B, P>,
    pub retire_stopped: IssuedRetireV1<B>,
    pub completion: Option<CompletionHooksV1<B, P>>,
}

pub(in crate::async_engine) struct CompletionHooksV1<B: RuntimeBackendV1, P> {
    // Success includes native, Context record, credit and exact hold settlement.
    // Failure must leave P rooted; it cannot authorize retry or decoding.
    pub settle: AdoptV1<B, P>,
    pub decode: fn(P) -> GeneratedCompletionOutcomeV1,
}

impl<B: RuntimeBackendV1, P> Copy for CompletionHooksV1<B, P> {}
impl<B: RuntimeBackendV1, P> Clone for CompletionHooksV1<B, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: RuntimeBackendV1, P> Copy for IssueHooksV1<B, P> {}
impl<B: RuntimeBackendV1, P> Clone for IssueHooksV1<B, P> {
    fn clone(&self) -> Self {
        *self
    }
}

pub(in crate::async_engine) struct AdoptionHooksV1<B: RuntimeBackendV1, P> {
    // Metadata/currentness only: no allocation checkout or native custody transfer.
    pub preflight: AdoptionPreflightV1<B, P>,
    // Readiness cannot access the carrier or transfer native ownership. False
    // means lane contention or exclusion; errors mean invalid identity or health.
    pub ready: AdoptionReadyV1<B>,
    pub adopt: AdoptV1<B, P>,
    // Must accept an empty prefix when Stop precedes the first adoption advance.
    // Success establishes conclusive disposal, including closing currentness.
    pub retire: AdoptionRetireV1<B>,
    pub issue: Option<IssueHooksV1<B, P>>,
}

impl<B: RuntimeBackendV1, P> Copy for AdoptionHooksV1<B, P> {}
impl<B: RuntimeBackendV1, P> Clone for AdoptionHooksV1<B, P> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PhaseV1 {
    Adopting,
    Adopted,
    Issued,
    PhysicallySettled,
    Retiring,
    Retired,
    Quarantined,
}

pub(super) struct UnpublishedAdoptionV1 {
    hold: ContextUnpublishedHoldV1,
    // Keep the original completion consumer as well as the driver's producer.
    _ticket: RuntimeAsyncReservedTicketV1,
    phase: PhaseV1,
}

impl UnpublishedAdoptionV1 {
    pub(super) fn new(
        hold: ContextUnpublishedHoldV1,
        ticket: RuntimeAsyncReservedTicketV1,
    ) -> Self {
        Self {
            hold,
            _ticket: ticket,
            phase: PhaseV1::Adopting,
        }
    }
}

impl<B: RuntimeBackendV1, P, E> PreparationDriver<B, P, E> {
    pub(super) fn advance_unpublished_v1(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        let owner = self.unpublished.as_mut().expect("active unpublished owner");
        if owner.phase == PhaseV1::Adopting {
            // No error or unwind may make this effect callable a second time.
            owner.phase = PhaseV1::Quarantined;
            match (self.adoption.as_ref().expect("admitted hooks").ready)(context, &owner.hold) {
                Ok(false) if !context.is_terminal() => {
                    owner.phase = PhaseV1::Adopting;
                    return false;
                }
                Ok(true) if !context.is_terminal() => {}
                _ => {
                    context.quarantine_after_async_command_panic_v1();
                    return false;
                }
            }
            let result = (self.adoption.as_ref().expect("admitted hooks").adopt)(
                context,
                self.prepared.as_mut().expect("retained payload"),
                self.roster.as_ref().expect("retained roster"),
                &owner.hold,
            );
            match result {
                Ok(()) if !context.is_terminal() => owner.phase = PhaseV1::Adopted,
                _ => context.quarantine_after_async_command_panic_v1(),
            }
        } else if matches!(owner.phase, PhaseV1::Adopted | PhaseV1::Issued)
            && let Some(issue) = self.adoption.as_ref().expect("admitted hooks").issue
        {
            owner.phase = PhaseV1::Quarantined;
            let result = (issue.progress)(
                context,
                self.prepared.as_mut().expect("retained payload"),
                self.roster.as_ref().expect("retained roster"),
                &owner.hold,
            );
            match result {
                Ok(complete) if !context.is_terminal() => {
                    owner.phase = if complete {
                        PhaseV1::PhysicallySettled
                    } else {
                        PhaseV1::Issued
                    }
                }
                _ => context.quarantine_after_async_command_panic_v1(),
            }
        } else if owner.phase == PhaseV1::PhysicallySettled
            && !self.observations_stopped
            && let Some(completion) = self
                .adoption
                .as_ref()
                .expect("admitted hooks")
                .issue
                .and_then(|issue| issue.completion)
        {
            owner.phase = PhaseV1::Quarantined;
            let result = (completion.settle)(
                context,
                self.prepared.as_mut().expect("retained payload"),
                self.roster.as_ref().expect("retained roster"),
                &owner.hold,
            );
            if result.is_err() || context.is_terminal() {
                context.quarantine_after_async_command_panic_v1();
                return false;
            }
            owner.phase = PhaseV1::Retired;
            // Native/Context custody is now conclusively gone. Decoder failure
            // consumes only host storage and resolves this original cell once.
            let result = catch_unwind(AssertUnwindSafe(|| {
                (completion.decode)(self.prepared.take().expect("settled payload"))
            }));
            let observation = match result {
                Ok(decoded) => Ok(decoded),
                Err(payload) => {
                    core::mem::forget(payload);
                    Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
                }
            };
            if let Some(mut reply) = self.completion.take() {
                reply.complete(observation);
            }
            return true;
        }
        false
    }

    pub(super) fn retire_unpublished_v1(
        &mut self,
        context: &mut RuntimeContextV1<B>,
    ) -> Result<bool, RuntimeErrorV1<B::Error>> {
        let Some(owner) = self.unpublished.as_mut() else {
            return Ok(false);
        };
        if matches!(owner.phase, PhaseV1::Issued | PhaseV1::PhysicallySettled) {
            // Drain observes the accepted prefix without cancelling output.
            // Only actual Stop, after stopping the reserved completion observer,
            // permits destructive disposal without output delivery.
            if !self.observations_stopped {
                return Ok(false);
            }
            let phase = owner.phase;
            owner.phase = PhaseV1::Quarantined;
            let issue = self
                .adoption
                .as_ref()
                .expect("admitted hooks")
                .issue
                .expect("issued hooks");
            if !(issue.retire_stopped)(context, &owner.hold)? {
                owner.phase = phase;
                return Ok(false);
            }
            context.release_unpublished_hold_v1(&owner.hold)?;
            owner.phase = PhaseV1::Retired;
            return Ok(true);
        }
        if !matches!(owner.phase, PhaseV1::Adopting | PhaseV1::Adopted) {
            return Err(RuntimeValidationErrorV1::ContextTerminal.into());
        }
        owner.phase = PhaseV1::Retiring;
        (self.adoption.as_ref().expect("admitted hooks").retire)(context, &owner.hold)?;
        context.release_unpublished_hold_v1(&owner.hold)?;
        owner.phase = PhaseV1::Retired;
        Ok(true)
    }
}

pub(in crate::async_engine) enum ActivationErrorV1<E> {
    Engine(RuntimeAsyncEngineCallErrorV1),
    Context(RuntimeErrorV1<E>),
}

pub(in crate::async_engine) struct ActivationFailureV1<E> {
    pub ticket: RuntimeAsyncReservedTicketV1,
    pub error: ActivationErrorV1<E>,
}

impl<E: fmt::Debug> fmt::Debug for ActivationErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => f.debug_tuple("Engine").field(error).finish(),
            Self::Context(error) => f.debug_tuple("Context").field(error).finish(),
        }
    }
}

impl<E: fmt::Debug> fmt::Debug for ActivationFailureV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActivationFailureV1")
            .field("ticket", &self.ticket)
            .field("error", &self.error)
            .finish()
    }
}

type ActivationReplyV1<E> = owned::Reply<Result<(), ActivationFailureV1<E>>>;
type ActivationFutureV1<E> = RuntimeAsyncCommandFutureV1<Result<(), ActivationFailureV1<E>>>;

pub(in crate::async_engine) struct ActivateCommandV1<B: RuntimeBackendV1> {
    ticket: Option<RuntimeAsyncReservedTicketV1>,
    stream: RuntimeStreamIdV1,
    reply: Option<ActivationReplyV1<B::Error>>,
}

impl<B: RuntimeBackendV1> ActivateCommandV1<B> {
    pub(in crate::async_engine) fn reject_reserved_context(&mut self) {
        self.reject(ActivationErrorV1::Context(
            RuntimeValidationErrorV1::ContextReserved.into(),
        ));
    }
    fn reject(&mut self, error: ActivationErrorV1<B::Error>) {
        if let Some(ticket) = self.ticket.take()
            && let Some(mut reply) = self.reply.take()
        {
            reply.complete(Ok(Err(ActivationFailureV1 { ticket, error })));
        }
    }

    pub(in crate::async_engine) fn run(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            operations.activate_reserved(context, &mut self.ticket, self.stream)
        }));
        match result {
            Ok(Ok(())) => {
                if let Some(mut reply) = self.reply.take() {
                    reply.complete(Ok(Ok(())));
                }
            }
            Ok(Err(error)) if !context.is_terminal() => self.reject(error),
            failed => {
                let error = if let Err(payload) = failed {
                    core::mem::forget(payload);
                    RuntimeAsyncEngineCallErrorV1::CommandPanicked
                } else {
                    RuntimeAsyncEngineCallErrorV1::EngineStopped
                };
                context.quarantine_after_async_command_panic_v1();
                drop(self.ticket.take());
                stop_reply(&mut self.reply, None, error);
            }
        }
    }
}

impl<B: RuntimeBackendV1> Drop for ActivateCommandV1<B> {
    fn drop(&mut self) {
        self.reject(ActivationErrorV1::Engine(
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        ));
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    // Kept private until DATA, ISSUE and COMPLETE supply their real adapters.
    #[allow(
        dead_code,
        reason = "private integration boundary for native DATA adoption"
    )]
    pub(in crate::async_engine) fn try_activate_reserved_v1(
        &self,
        ticket: RuntimeAsyncReservedTicketV1,
        stream: RuntimeStreamIdV1,
    ) -> Result<ActivationFutureV1<B::Error>, ActivationFailureV1<B::Error>> {
        let failure = |ticket, error| ActivationFailureV1 {
            ticket,
            error: ActivationErrorV1::Engine(error),
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
        let command = ActivateCommandV1 {
            ticket: Some(ticket),
            stream,
            reply: Some(reply),
        };
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::ActivateReserved(command))
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
                let RuntimeAsyncEngineCommandV1::ActivateReserved(mut command) = command else {
                    unreachable!()
                };
                Err(failure(
                    command.ticket.take().expect("unqueued ticket"),
                    error,
                ))
            }
        }
    }
}
