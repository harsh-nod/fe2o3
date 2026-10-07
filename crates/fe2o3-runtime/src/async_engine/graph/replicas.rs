//! Group framing and explicit whole-copy effects, never caller replica authority.

use super::*;

impl<B: RuntimeBackendV1> RuntimeGraphRequestV1<B> {
    /// Uses a closed roster from the original Context. Legacy `new` still
    /// accepts only the original single-device identity at actual admission.
    /// The group grants no GPU access; each operation keeps normal admission.
    /// Group writes to one allocation must be ordered even for disjoint ranges:
    /// the retained journal versions whole allocations, not independent slices.
    pub fn new_group_v1(
        graph: CompletionGraphV1,
        group: crate::RuntimeGraphGroupV1,
    ) -> Result<Self, RuntimeGraphValidationErrorV1> {
        if graph.context() != group.context_identity() {
            return Err(RuntimeGraphValidationErrorV1::StreamBindings);
        }
        let mut request = Self::new(graph, group.bindings())?;
        request.group = Some(group);
        Ok(request)
    }

    /// Tracks the exact successful whole copy after original native release.
    /// Its input version is resolved by the existing graph ledger; ProducedBy
    /// names an actual predecessor, not a historical report or equal-byte hash.
    /// A later write to either allocation invalidates the retained locator.
    pub fn bind_tracked_replica_copy_v1(
        &mut self,
        node: CompletionNodeIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        self.check_node(node)?;
        if self.group.is_none()
            || source.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
            || source.allocation == destination.allocation
            || source.byte_offset != 0
            || destination.byte_offset != 0
            || source.byte_len == 0
            || source.byte_len != destination.byte_len
        {
            return Err(RuntimeGraphValidationErrorV1::InvalidReplicaCopy);
        }
        if self.effects > MAX_RUNTIME_GRAPH_EFFECTS_V1 - 2 {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        self.actions
            .insert(node, Action::ReplicaCopy(source, destination));
        self.effects += 2;
        Ok(())
    }

    pub(super) fn validate_group_v1(
        &self,
        context: &RuntimeContextV1<B>,
    ) -> Result<(), RuntimeGraphErrorV1<B::Error>> {
        if let Some(group) = &self.group {
            if self
                .actions
                .values()
                .any(|action| matches!(action, Action::PeerCopy(..)))
            {
                return Err(RuntimeGraphErrorV1::Invalid(
                    RuntimeGraphValidationErrorV1::InvalidPeerGather,
                ));
            }
            group
                .revalidate(context)
                .map_err(|e| RuntimeGraphErrorV1::Context(e.into()))?;
            if self.graph.context() != group.context_identity()
                || self
                    .streams
                    .iter()
                    .any(|(&identity, &stream)| group.stream_identity(stream) != Ok(identity))
            {
                return Err(RuntimeGraphErrorV1::Invalid(
                    RuntimeGraphValidationErrorV1::StreamBindings,
                ));
            }
        }
        let tracked = self
            .actions
            .values()
            .filter(|action| matches!(action, Action::ReplicaCopy(..)))
            .count();
        if tracked != 0 {
            let group = self.group.as_ref().ok_or(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::InvalidReplicaCopy,
            ))?;
            context
                .preflight_graph_replica_capacity_v1(tracked)
                .map_err(|e| RuntimeGraphErrorV1::Context(e.into()))?;
            for action in self.actions.values() {
                if let Action::ReplicaCopy(source, destination) = action {
                    context
                        .preflight_graph_replica_regions_v1(group, *source, *destination)
                        .map_err(|e| RuntimeGraphErrorV1::Context(e.into()))?;
                }
            }
        }
        Ok(())
    }
}
