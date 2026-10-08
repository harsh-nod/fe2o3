//! Inert graph snapshots for explicit staging, never result or release authority.
use super::*;

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    /// Reads the exact node in this scope's original graph without progressing it.
    /// This is only a scheduler snapshot, not native completion, currentness,
    /// result ownership, or graph retirement. In particular, `Ready` can describe
    /// an issued operation; existing staging/result methods retain their guards.
    /// Failed dependencies can be skipped without treating an invalid ticket as
    /// cancellation. Original result tickets still require complete retirement.
    pub fn graph_node_state_v1(
        &self,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
        node: CompletionNodeIdV1,
    ) -> Result<CompletionNodeStateV1, RuntimeGfx942ScopeErrorV1> {
        let graph = self.check_graph_ticket(ticket)?;
        let index = graph
            .ids
            .binary_search(&node)
            .map_err(|_| RuntimeGfx942ScopeErrorV1::InvalidTicket)?;
        if let Some(core) = &graph.core {
            return Ok(core.state(index));
        }
        graph
            .report
            .as_ref()
            .and_then(|report| report.completion.entries().get(index))
            .filter(|entry| entry.node() == node)
            .map(|entry| entry.state())
            .ok_or(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    }
}
