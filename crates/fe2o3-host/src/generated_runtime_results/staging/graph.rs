//! The closed charged-result adapter for a separately admitted host-write node.

use super::*;
use fe2o3_runtime::{
    KfdRuntimeBackendErrorV1, RuntimeGfx942GeneratedCompletionCarrierV1,
    RuntimeGfx942GeneratedScopeV1, RuntimeGfx942ScopedGraphStagingErrorV1,
    RuntimeGfx942ScopedGraphTicketV1, completion::CompletionNodeIdV1,
};

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Stages this exact original decoded result into a distinct ordinary graph
    /// write. The scope validates the producer, graph occurrence, dependency and
    /// complete staging allocation; this adapter matches its private result gate
    /// before encoding. Caller-preallocated scratch is outside result accounting.
    ///
    /// No output is taken, cloned or refunded on success, contention or refusal.
    /// The existing observer retains its charged result until actual disposal or
    /// a later authentic take. Only the ordinary host-write node receives a new
    /// graph version, after the real host write succeeds. This does not widen the
    /// admitted kernel's effect set or allow a new generated kernel consumer.
    pub fn try_stage_graph_completed_v1<'scope, B, P>(
        &mut self,
        scope: &mut RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
        node: CompletionNodeIdV1,
        producer: CompletionNodeIdV1,
        staging: RuntimeAllocationIdV1,
        scratch: &mut [u8],
    ) -> Result<Option<()>, RuntimeGfx942ScopedGraphStagingErrorV1<Error>>
    where
        B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
        P: RuntimeGfx942GeneratedCompletionCarrierV1,
    {
        scope.try_stage_graph_host_write_v1(
            ticket,
            node,
            producer,
            staging,
            scratch,
            |domain, bytes| self.encode_graph_completed_v1(domain, bytes),
        )
    }

    pub(super) fn encode_graph_completed_v1(
        &mut self,
        domain: &fe2o3_runtime::RuntimeGeneratedResultDomainV1,
        bytes: &mut [u8],
    ) -> Result<Option<()>, Error> {
        self.encode_completed_matching_v1(|gate| domain.matches_owner(gate), bytes)
    }
}
