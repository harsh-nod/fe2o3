//! Exact never-adopted or definitely unpublished disposal, never packet cancellation.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGfx942ScopedCancelResultV1 {
    CancelledBeforeSubmission,
    CancelledBeforePublication,
    AlreadyCancelled,
    NotCancellable,
}

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    /// Disposes an independent carrier only before its native adoption starts.
    /// A readiness-only Pending observation leaves this boundary open; entry to
    /// adoption closes it even before queue publication. Issued, unknown and
    /// settled owners cannot be cancelled here. Graph-owned nodes are refused.
    ///
    /// Success follows actual original carrier disposal and exact unpublished
    /// hold release, without a decoder or readback result. Its future observes
    /// `CancelledBeforeSubmission`, never successful completion. The original
    /// slot identity and total-scope quota remain retained and cannot be reused.
    /// Disposal unwind or hold-release contradiction leaves fail-stop custody.
    pub fn cancel_before_adoption_v1(
        &mut self,
        ticket: &RuntimeGfx942ScopedTicketV1<'scope>,
    ) -> Result<RuntimeGfx942ScopedCancelResultV1, RuntimeGfx942ScopeErrorV1> {
        use RuntimeGfx942ScopedCancelResultV1 as Cancel;
        if !Rc::ptr_eq(&self.identity, &ticket.scope) || ticket.index >= self.slots.len() {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        if self.graph.is_some() || self.slots[ticket.index].hold.is_graph_scoped_v1() {
            return Err(RuntimeGfx942ScopeErrorV1::Graph(
                crate::RuntimeGraphErrorV1::Busy,
            ));
        }
        match self.slots[ticket.index].lifecycle.phase {
            Phase::Cancelled | Phase::CancelledUnpublished => return Ok(Cancel::AlreadyCancelled),
            Phase::Adopting => {}
            _ => return Ok(Cancel::NotCancellable),
        }
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        let slot = &mut self.slots[ticket.index];
        self.context
            .validate_unpublished_hold_v1(&slot.hold)
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        let cancelled = slot
            .lifecycle
            .cancel_before_adoption(|| self.context.release_unpublished_hold_v1(&slot.hold))
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        assert!(
            cancelled,
            "exclusive owner checked the original adoption boundary"
        );
        slot.reply.complete(Err(
            crate::RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission,
        ));
        Ok(Cancel::CancelledBeforeSubmission)
    }

    /// Retires one adopted independent carrier only before packet publication.
    ///
    /// This is a separate boundary from `cancel_before_adoption_v1`: native DATA
    /// may already exist and a Context submission may already be registered. The
    /// original backend must still retain pristine DATA or its exact Ready or
    /// RetryReady receipt. Published, completed, recycled, unknown and graph
    /// work are refused without polling or inferring nonpublication from quietness.
    /// Existing journal-corruption checks still quarantine the entire Context;
    /// admission never disposes native owners or releases credits.
    ///
    /// Success requires original native DATA disposal, no-result Context
    /// retirement, carrier destruction and exact hold release, in that order.
    /// Its observer receives `CancelledBeforePublication`, never decoded output
    /// or a no-effect writer version. Other independent slots may continue.
    /// Failed currentness/disposal retains the existing global fail-stop rule;
    /// this API does not isolate a hardware reset or post-publication failure.
    pub fn cancel_before_publication_v1(
        &mut self,
        ticket: &RuntimeGfx942ScopedTicketV1<'scope>,
    ) -> Result<RuntimeGfx942ScopedCancelResultV1, RuntimeGfx942ScopeErrorV1> {
        use RuntimeGfx942ScopedCancelResultV1 as Cancel;
        if !Rc::ptr_eq(&self.identity, &ticket.scope) || ticket.index >= self.slots.len() {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        if self.graph.is_some() || self.slots[ticket.index].hold.is_graph_scoped_v1() {
            return Err(RuntimeGfx942ScopeErrorV1::Graph(
                crate::RuntimeGraphErrorV1::Busy,
            ));
        }
        match self.slots[ticket.index].lifecycle.phase {
            Phase::Cancelled | Phase::CancelledUnpublished => return Ok(Cancel::AlreadyCancelled),
            Phase::Issuing => {}
            _ => return Ok(Cancel::NotCancellable),
        }
        if self.context.is_terminal()
            || self
                .slots
                .iter()
                .any(|slot| slot.lifecycle.phase == Phase::Unknown)
            || self.copies.iter().any(|slot| slot.unknown)
        {
            return Err(RuntimeGfx942ScopeErrorV1::Unknown);
        }
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        let slot = &mut self.slots[ticket.index];
        if !(self.hooks.unpublished)(self.context, &slot.hold)
            .map_err(RuntimeGfx942ScopeErrorV1::Context)?
        {
            return Ok(Cancel::NotCancellable);
        }
        let cancelled = slot.lifecycle.cancel_unpublished(
            &mut *self.context,
            |context| (self.hooks.retire_unpublished)(context, &slot.hold),
            |context| {
                context
                    .release_unpublished_hold_v1(&slot.hold)
                    .map_err(Into::into)
            },
        );
        let cancelled = match cancelled {
            Ok(cancelled) => cancelled,
            Err(error) => {
                slot.reply
                    .complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
                return Err(RuntimeGfx942ScopeErrorV1::Context(error));
            }
        };
        assert!(cancelled, "exclusive owner checked original issuing phase");
        slot.reply.complete(Err(
            crate::RuntimeAsyncEngineCallErrorV1::CancelledBeforePublication,
        ));
        Ok(Cancel::CancelledBeforePublication)
    }
}
