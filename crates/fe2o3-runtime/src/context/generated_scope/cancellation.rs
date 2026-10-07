//! Exact never-adopted carrier disposal, not native cancellation or quiescence.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGfx942ScopedCancelResultV1 {
    CancelledBeforeSubmission,
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
            Phase::Cancelled => return Ok(Cancel::AlreadyCancelled),
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
}
