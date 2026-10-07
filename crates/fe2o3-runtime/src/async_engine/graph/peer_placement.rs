//! Bounded explicit placement; allocation names are checked by the original Context.

use super::*;

/// The first gather profile retains at most eight source regions/devices.
pub const MAX_RUNTIME_GRAPH_PEER_SHARDS_V1: usize = 8;

/// An inert requested peer transfer, not a route or data-version capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGraphPeerShardV1 {
    pub node: CompletionNodeIdV1,
    pub source: RuntimeMemoryRegionV1,
    pub destination: RuntimeMemoryRegionV1,
}

impl<B: RuntimeBackendV1> RuntimeGraphRequestV1<B> {
    /// Binds a Context-selected inert placement. Shared admission rechecks all
    /// selected observations before any graph reservation or backend operation.
    pub fn bind_selected_peer_gather_v1(
        &mut self,
        placement: crate::RuntimePeerGatherPlacementV1,
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        for shard in placement.shards() {
            let node = self
                .graph
                .nodes()
                .iter()
                .find(|node| node.id() == shard.node)
                .ok_or(RuntimeGraphValidationErrorV1::UnknownNode)?;
            if self.streams.get(&node.stream()) != Some(&placement.stream()) {
                return Err(RuntimeGraphValidationErrorV1::StreamBindings);
            }
        }
        self.bind_peer_gather_v1(placement.shards())?;
        self.peer_placement = Some(placement);
        Ok(())
    }

    pub(super) fn revalidate_peer_placement_v1(
        &self,
        context: &RuntimeContextV1<B>,
    ) -> Result<(), RuntimeGraphErrorV1<B::Error>> {
        if let Some(placement) = &self.peer_placement {
            for shard in placement.shards() {
                if !matches!(self.actions.get(&shard.node), Some(Action::PeerCopy(source, destination))
                    if *source == shard.source && *destination == shard.destination)
                {
                    return Err(RuntimeGraphErrorV1::Invalid(
                        RuntimeGraphValidationErrorV1::InvalidPeerGather,
                    ));
                }
            }
            placement
                .revalidate(context)
                .map_err(RuntimeGraphErrorV1::Context)?;
        }
        Ok(())
    }

    /// Binds one explicit gather of 1..=8 peer shards to existing Future nodes.
    ///
    /// Shards use one destination allocation and stream, with disjoint output
    /// ranges. The original Context resolves all allocation and device names;
    /// each source must be on a different device from the destination. The
    /// backend retains its existing native/staged route policy and custody.
    ///
    /// Source allocations must remain read-only throughout this graph. This
    /// profile accepts only their initial versions, not cross-device producer
    /// tokens, replica equivalence or versions from a previous graph. Other
    /// actions may consume the gathered output but may not write it. One-stream
    /// ordering preserves the existing whole-allocation writer journal; this
    /// is not concurrent-copy or physical-overlap evidence.
    ///
    /// All validation refusals leave the request unchanged. Device/range checks
    /// and graph hazards are revalidated during actual Context admission.
    pub fn bind_peer_gather_v1(
        &mut self,
        shards: &[RuntimeGraphPeerShardV1],
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        if self
            .actions
            .values()
            .any(|a| matches!(a, Action::PeerCopy(..)))
        {
            return Err(RuntimeGraphValidationErrorV1::InvalidPeerGather);
        }
        self.check_peer_shape_v1(shards)?;
        for shard in shards {
            self.check_node(shard.node)?;
        }
        let effects = shards.len() * 2;
        if effects > MAX_RUNTIME_GRAPH_EFFECTS_V1 - self.effects {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        for shard in shards {
            self.actions.insert(
                shard.node,
                Action::PeerCopy(shard.source, shard.destination),
            );
        }
        self.effects += effects;
        Ok(())
    }

    fn check_peer_shape_v1(
        &self,
        shards: &[RuntimeGraphPeerShardV1],
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        let invalid = RuntimeGraphValidationErrorV1::InvalidPeerGather;
        if shards.is_empty() || shards.len() > MAX_RUNTIME_GRAPH_PEER_SHARDS_V1 {
            return Err(invalid);
        }
        let mut stream = None;
        for (index, shard) in shards.iter().enumerate() {
            let node_index = self
                .graph
                .nodes()
                .binary_search_by_key(&shard.node, |n| n.id())
                .map_err(|_| RuntimeGraphValidationErrorV1::UnknownNode)?;
            let node = &self.graph.nodes()[node_index];
            if !matches!(node.kind(), CompletionNodeKindV1::Future(_)) {
                return Err(RuntimeGraphValidationErrorV1::NotOperation);
            }
            if stream.is_some_and(|prior| prior != node.stream()) {
                return Err(invalid);
            }
            stream = Some(node.stream());
            if shard.source.access != RuntimeAccessV1::Read
                || shard.destination.access != RuntimeAccessV1::Write
                || shard.source.byte_len == 0
                || shard.source.byte_len != shard.destination.byte_len
                || shard.source.allocation == shard.destination.allocation
                || shard.destination.allocation != shards[0].destination.allocation
                || shard
                    .source
                    .byte_offset
                    .checked_add(shard.source.byte_len)
                    .is_none()
            {
                return Err(invalid);
            }
            let end = shard
                .destination
                .byte_offset
                .checked_add(shard.destination.byte_len)
                .ok_or(invalid)?;
            for prior in &shards[..index] {
                let prior_end = prior
                    .destination
                    .byte_offset
                    .checked_add(prior.destination.byte_len)
                    .ok_or(invalid)?;
                if prior.node == shard.node
                    || (prior.destination.byte_offset < end
                        && shard.destination.byte_offset < prior_end)
                {
                    return Err(invalid);
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_peer_gather_v1(&self) -> Result<(), RuntimeGraphValidationErrorV1> {
        let mut shards = Vec::new();
        for (&node, action) in &self.actions {
            if let Action::PeerCopy(source, destination) = action {
                if shards.len() == MAX_RUNTIME_GRAPH_PEER_SHARDS_V1 {
                    return Err(RuntimeGraphValidationErrorV1::InvalidPeerGather);
                }
                shards.push(RuntimeGraphPeerShardV1 {
                    node,
                    source: *source,
                    destination: *destination,
                });
            }
        }
        if shards.is_empty() {
            return Ok(());
        }
        self.check_peer_shape_v1(&shards)?;
        let invalid = RuntimeGraphValidationErrorV1::InvalidPeerGather;
        for shard in &shards {
            if self
                .version_inputs
                .iter()
                .any(|(&(node, allocation, _, _), source)| {
                    node == shard.node
                        && allocation == shard.source.allocation
                        && *source != RuntimeGraphVersionSourceV1::InitialAtAdmission
                })
            {
                return Err(invalid);
            }
        }
        let forbidden_write = |region: RuntimeMemoryRegionV1, peer: bool| {
            region.access != RuntimeAccessV1::Read
                && (shards
                    .iter()
                    .any(|s| s.source.allocation == region.allocation)
                    || (!peer && region.allocation == shards[0].destination.allocation))
        };
        for action in self.actions.values() {
            let bad = match action {
                Action::Launch(launch) => launch
                    .bindings()
                    .iter()
                    .any(|b| forbidden_write(b.region, false)),
                Action::Copy(source, destination) | Action::ReplicaCopy(source, destination) => {
                    forbidden_write(*source, false) || forbidden_write(*destination, false)
                }
                Action::PeerCopy(source, destination) => {
                    forbidden_write(*source, true) || forbidden_write(*destination, true)
                }
                Action::HostStaging(staging) => forbidden_write(staging.destination, false),
            };
            if bad {
                return Err(invalid);
            }
        }
        Ok(())
    }
}
