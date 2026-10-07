//! Explicit ordinary host staging under the original graph reservation.

use super::*;

#[derive(Debug)]
pub enum RuntimeGfx942ScopedGraphStagingErrorV1<E> {
    Scope(RuntimeGfx942ScopeErrorV1),
    Encoding(E),
}

impl<E: fmt::Debug> fmt::Display for RuntimeGfx942ScopedGraphStagingErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scoped graph staging: {self:?}")
    }
}

impl<E: fmt::Debug + 'static> Error for RuntimeGfx942ScopedGraphStagingErrorV1<E> {}

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    /// Attempts one separately admitted ordinary HostVisible graph write.
    /// `None` means its dependency or the encoder is not ready; neither native
    /// write nor graph-version transition occurred. An encoder refusal likewise
    /// preserves readiness and the caller's original result owner for correction.
    ///
    /// The callback receives only the exact successful predecessor's original
    /// completion domain, never Context access. It must authenticate its own
    /// result owner. This runtime method certifies only an ordinary host write;
    /// it does not certify the origin or meaning of caller-provided bytes.
    /// Scratch must already exist and match the complete declared allocation.
    /// No callback, byte buffer or result owner is boxed or retained here.
    ///
    /// A successful write commits the distinct staging node's graph version.
    /// A definitely rejected write fails that node and its dependent successors.
    /// Unknown effects retain/quarantine the whole graph and are never retried.
    // SAFETY: the only successful operation transition follows the actual
    // synchronous host write; failed transitions have no ambiguous native owner.
    #[allow(unsafe_code)]
    pub fn try_stage_graph_host_write_v1<E>(
        &mut self,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
        node: CompletionNodeIdV1,
        producer: CompletionNodeIdV1,
        allocation: RuntimeAllocationIdV1,
        scratch: &mut [u8],
        encode: impl FnOnce(&crate::RuntimeGeneratedResultDomainV1, &mut [u8]) -> Result<Option<()>, E>,
    ) -> Result<Option<()>, RuntimeGfx942ScopedGraphStagingErrorV1<E>> {
        use RuntimeGfx942ScopedGraphStagingErrorV1::{Encoding, Scope};
        self.check_graph_ticket(ticket).map_err(Scope)?;
        if Instant::now() >= self.deadline {
            return Err(Scope(RuntimeGfx942ScopeErrorV1::Deadline));
        }
        let _permit = self
            .epoch
            .enter()
            .map_err(|e| Scope(RuntimeGfx942ScopeErrorV1::Context(e.into())))?;
        if self.context.is_terminal()
            || self
                .slots
                .iter()
                .any(|slot| slot.lifecycle.phase == Phase::Unknown)
            || self.copies.iter().any(|slot| slot.unknown)
        {
            return Err(Scope(RuntimeGfx942ScopeErrorV1::Unknown));
        }
        let invalid = || Scope(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        let graph = self.graph.as_mut().ok_or_else(invalid)?;
        let core = graph.core.as_mut().ok_or_else(invalid)?;
        let index = core.index(node).ok_or_else(invalid)?;
        let staging = core.host_staging(index).ok_or_else(invalid)?;
        if staging.producer != producer
            || staging.destination.allocation != allocation
            || u64::try_from(scratch.len()).ok() != Some(staging.destination.byte_len)
            || core.issued(index)
            || graph.reconciled[index]
        {
            return Err(invalid());
        }
        self.context
            .require_graph_access(Some(core.token()))
            .map_err(|e| Scope(RuntimeGfx942ScopeErrorV1::Context(e.into())))?;
        if !fe2o3_runtime_model::r63_graph_can_issue_v1(
            self.context.terminal,
            self.context.graph_reservation == Some(core.token()),
            self.context.graph_issue_closed,
        ) {
            return Err(Scope(RuntimeGfx942ScopeErrorV1::Context(
                RuntimeValidationErrorV1::ContextReserved.into(),
            )));
        }
        if core.state(index) == CompletionNodeStateV1::Blocked {
            return Ok(None);
        }
        if core.state(index) != CompletionNodeStateV1::Ready {
            return Err(invalid());
        }
        let predecessor = core.index(producer).ok_or_else(invalid)?;
        let slot = graph.generated[predecessor].ok_or_else(invalid)?;
        if core.state(predecessor) != CompletionNodeStateV1::Succeeded
            || !graph.reconciled[predecessor]
            || !matches!(self.slots[slot].lifecycle.outcome, Some(Ok(())))
        {
            return Err(invalid());
        }
        if encode(&self.slots[slot].domain, scratch)
            .map_err(Encoding)?
            .is_none()
        {
            return Ok(None);
        }
        if !core.begin(index) {
            self.context.quarantine_after_async_command_panic_v1();
            return Err(Scope(RuntimeGfx942ScopeErrorV1::Unknown));
        }
        match self.context.write_host_visible_with_graph_access_v1(
            allocation,
            scratch,
            Some(core.token()),
        ) {
            Ok(()) => {
                // SAFETY: exact ordinary host write and its journal settlement
                // both succeeded before the separate graph version is committed.
                if !unsafe { core.succeed_host_staging(index) } {
                    self.context.quarantine_after_async_command_panic_v1();
                    return Err(Scope(RuntimeGfx942ScopeErrorV1::Unknown));
                }
                graph.reconciled[index] = true;
                graph
                    .observations
                    .push((node, RuntimeCompletionStatusV1::Succeeded));
                Ok(Some(()))
            }
            Err(error) => {
                if self.context.is_terminal()
                    || !matches!(
                        error,
                        RuntimeErrorV1::Validation(_) | RuntimeErrorV1::BackendRejected(_)
                    )
                {
                    self.context.quarantine_after_async_command_panic_v1();
                } else {
                    // SAFETY: validation/no-effect refusal left no native write
                    // owner; failing the staged version blocks all consumers.
                    unsafe {
                        core.fail(index, 5);
                    }
                    graph.reconciled[index] = true;
                    graph
                        .observations
                        .push((node, RuntimeCompletionStatusV1::QuiescentWithoutResult));
                }
                Err(Scope(RuntimeGfx942ScopeErrorV1::Context(error)))
            }
        }
    }
}
