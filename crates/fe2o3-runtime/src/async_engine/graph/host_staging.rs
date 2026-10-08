//! Ordinary host writes ordered after a generated decoder, not generated effects.

use super::*;

#[derive(Clone, Copy)]
pub(crate) struct HostStagingV1 {
    pub(crate) producer: CompletionNodeIdV1,
    pub(crate) destination: RuntimeMemoryRegionV1,
}

impl<B: RuntimeBackendV1> RuntimeGraphRequestV1<B> {
    /// Declares a separate whole-allocation HostVisible write after a generated
    /// predecessor. The scoped adapter supplies bytes from an original decoded
    /// result later; binding this inert action neither writes nor certifies DATA.
    ///
    /// The existing static adapter refuses this action before reservation. A
    /// scoped caller must stage it explicitly before draining the whole graph.
    /// A copy can require `ProducedBy(node)` for this ordinary staging version;
    /// the generated kernel's sealed effects and argument bindings do not change.
    pub fn bind_host_staging_v1(
        &mut self,
        node: CompletionNodeIdV1,
        producer: CompletionNodeIdV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        self.check_node(node)?;
        if node == producer
            || destination.access != RuntimeAccessV1::Write
            || destination.byte_offset != 0
            || destination.byte_len == 0
            || destination.byte_len > MAX_RUNTIME_GRAPH_HOST_STAGING_BYTES_V1
        {
            return Err(RuntimeGraphValidationErrorV1::InvalidHostStaging);
        }
        let producer_index = self
            .graph
            .nodes()
            .binary_search_by_key(&producer, |n| n.id())
            .map_err(|_| RuntimeGraphValidationErrorV1::UnknownNode)?;
        if !matches!(
            self.graph.nodes()[producer_index].kind(),
            CompletionNodeKindV1::Future(_)
        ) {
            return Err(RuntimeGraphValidationErrorV1::NotOperation);
        }
        if self.effects == MAX_RUNTIME_GRAPH_EFFECTS_V1 {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        self.effects += 1;
        self.actions.insert(
            node,
            Action::HostStaging(HostStagingV1 {
                producer,
                destination,
            }),
        );
        Ok(())
    }

    pub(super) fn validate_host_staging_dependencies_v1(
        &self,
        ancestors: &[[u64; MAX_RUNTIME_GRAPH_NODES_V1 / 64]],
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        let nodes = self.graph.nodes();
        for (&node, action) in &self.actions {
            let Action::HostStaging(staging) = action else {
                continue;
            };
            let producer = nodes
                .binary_search_by_key(&staging.producer, |n| n.id())
                .map_err(|_| RuntimeGraphValidationErrorV1::UnknownNode)?;
            let index = nodes
                .binary_search_by_key(&node, |n| n.id())
                .map_err(|_| RuntimeGraphValidationErrorV1::UnknownNode)?;
            if self.actions.contains_key(&staging.producer)
                || ancestors[index][producer / 64] & (1 << (producer % 64)) == 0
            {
                return Err(RuntimeGraphValidationErrorV1::InvalidHostStaging);
            }
        }
        Ok(())
    }
}
