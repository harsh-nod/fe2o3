//! Private retained initial graph; stops before invocation worklists/seeding.
//! Raw inputs in component tests are DATA, never canonical source authority.
use super::*;
use std::mem::size_of;
#[path = "retained_initial_capability_graph_frame_v1.rs"]
mod frame;
#[path = "whole_root_initial_capability_graph_bridge_v1.rs"]
mod whole;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GraphPhase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct GraphSourceKey {
    source: SourceIdentity,
    counts: (usize, usize),
    options: usize,
    enumeration: usize,
    launch_upper_bound: Option<u64>,
    initial: usize,
    arguments: usize,
    prefix: usize,
}
impl GraphSourceKey {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source: &Source<'_>,
        counts: &[u8],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch_upper_bound: Option<u64>,
        initial: &InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
    ) -> Self {
        Self {
            source: source.identity(),
            counts: (counts.as_ptr() as usize, counts.len()),
            options: options as *const _ as usize,
            enumeration: enumeration as *const _ as usize,
            launch_upper_bound,
            initial: initial as *const _ as usize,
            arguments: arguments as *const _ as usize,
            prefix: prefix as *const _ as usize,
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct WriterFloor {
    next_value: u32,
    operations: usize,
    next_argument: usize,
    index_rows: usize,
    slice_rows: usize,
}
impl WriterFloor {
    fn take(arguments: &RetainedBeforeArgumentWritersV1, prefix: &RootEntryPrefixV1) -> Self {
        Self {
            next_value: prefix.next_value,
            operations: prefix.entry_operations.len(),
            next_argument: arguments.next_runtime_argument,
            index_rows: arguments.runtime_index_arguments.len(),
            slice_rows: arguments.runtime_slice_extent_arguments.len(),
        }
    }
}
pub(super) struct RetainedInitialCapabilityGraphV1 {
    phase: GraphPhase,
    source: Option<GraphSourceKey>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    failure: Option<Backend>,
    launch_extent: Option<u64>,
    local_definitions: Vec<u8>,
    writers: Option<WriterFloor>,
    final_rows: Option<(usize, usize, usize, usize)>,
}
impl RetainedInitialCapabilityGraphV1 {
    pub(super) fn new() -> Self {
        Self {
            phase: GraphPhase::Fresh,
            source: None,
            entry: None,
            held: None,
            failure: None,
            launch_extent: None,
            local_definitions: Vec::new(),
            writers: None,
            final_rows: None,
        }
    }
    fn saved(&self) -> Backend {
        match &self.failure {
            Some(Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))) => {
                resource(*error)
            }
            Some(Backend::Unsupported(detail)) => Backend::Unsupported(*detail),
            Some(Backend::Incomplete(detail)) => Backend::Incomplete(*detail),
            _ => accounting(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_into(
        &mut self,
        source: &Source<'_>,
        counts: &[u8],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch_upper_bound: Option<u64>,
        initial: &mut InitialDestinations,
        arguments: &mut RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        let fresh = self.phase == GraphPhase::Fresh;
        self.phase = GraphPhase::Terminal;
        if !fresh {
            return Err(self.saved());
        }
        self.source = Some(GraphSourceKey::new(
            source,
            counts,
            options,
            enumeration,
            launch_upper_bound,
            initial,
            arguments,
            prefix,
        ));
        self.entry = resources.retained_custody_snapshot_v1();
        let result = self.prepare_original(
            source,
            counts,
            options,
            enumeration,
            launch_upper_bound,
            initial,
            arguments,
            prefix,
            resources,
        );
        match result {
            Ok(()) => {
                self.held = resources.retained_custody_snapshot_v1();
                self.phase = GraphPhase::Complete;
                Ok(())
            }
            Err(error) => {
                self.failure = Some(error);
                Err(self.saved())
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_original(
        &mut self,
        source: &Source<'_>,
        counts: &[u8],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch_upper_bound: Option<u64>,
        initial: &mut InitialDestinations,
        arguments: &mut RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        self.check_custody(resources)?;
        if resources.original_ledger_v1() != Some(source.ledger) {
            return Err(accounting());
        }
        resources.work(32)?;
        resources.reserve_storage(frame::bytes()?)?;
        let count = source.function.locals().len();
        if source.function.blocks().len() > 32
            || count > 4096
            || source.types.len() > 4096
            || source.callables.len() > 4096
        {
            return Err(Backend::Incomplete(
                "retained initial graph exceeds closed source profile",
            ));
        }
        if counts.len() != count
            || !self.local_definitions.is_empty()
            || self.local_definitions.capacity() != 0
            || self.launch_extent.is_some()
            || !arguments.entered
            || !arguments.initialized
            || arguments.runtime_index_arguments.len() != count
            || arguments.runtime_slice_extent_arguments.len() != count
            || arguments.edge_count != 0
            || !arguments.borrowed_locals.is_empty()
            || arguments.borrowed_locals.capacity() != 0
            || !initial.stores.is_empty()
            || initial.stores.capacity() != 0
            || !initial.loads.is_empty()
            || initial.loads.capacity() != 0
            || initial.edges.len() != count
        {
            return Err(accounting());
        }
        check_unseeded(initial, count, resources)?;
        resources.work(count)?;
        if initial
            .edges
            .iter()
            .any(|row| !row.is_empty() || row.capacity() != 0)
        {
            return Err(accounting());
        }
        self.writers = Some(WriterFloor::take(arguments, prefix));
        // The runtime grid domain remains dynamic. This is NOT the finite
        // source-launch bound passed independently to the graph helper below.
        self.launch_extent = Some(0);
        resources.work(count)?;
        resources.reserve(&mut self.local_definitions, count)?;
        self.local_definitions.extend_from_slice(counts);
        // All mutable destinations were already attached to the outer owner.
        // No replacement InitialGraphStorage, temporary graph, or refund scope.
        root_initial_capability_graph_v1::populate_initial_graph_v1(
            source.callables,
            source.function,
            launch_upper_bound,
            &self.local_definitions,
            options,
            enumeration,
            &mut initial.edges,
            &mut arguments.edge_count,
            &mut initial.stores,
            &mut initial.loads,
            &mut arguments.borrowed_locals,
            resources,
        )?;
        self.check_custody(resources)?;
        if self.writers != Some(WriterFloor::take(arguments, prefix)) {
            return Err(accounting());
        }
        self.final_rows = Some((
            arguments.edge_count,
            initial.stores.len(),
            initial.loads.len(),
            arguments.borrowed_locals.len(),
        ));
        // Deliberately no VecDeque, invocation seed, graph propagation,
        // borrowed-payload recovery, ranked operation or argument writer.
        Ok(())
    }
    fn check_custody(&self, resources: &Prep<'_, '_>) -> BResult<()> {
        let entry = self.entry.ok_or_else(accounting)?;
        let now = resources
            .retained_custody_snapshot_v1()
            .ok_or_else(accounting)?;
        let growth = now.owned.checked_sub(entry.owned).ok_or_else(accounting)?;
        if now.budget_slot != entry.budget_slot
            || now.work_ledger != entry.work_ledger
            || now.owned_slot != entry.owned_slot
            || now.storage != entry.storage.checked_add(growth).ok_or_else(arithmetic)?
            || now.work < entry.work
            || now.peak < entry.peak
            || entry.denied_work
            || entry.denied_storage
            || entry.owned > entry.storage
            || now.denied_work
            || now.denied_storage
        {
            return Err(accounting());
        }
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(accounting());
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn completed_for(
        &self,
        source: &Source<'_>,
        counts: &[u8],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch_upper_bound: Option<u64>,
        initial: &InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
        resources: &Prep<'_, '_>,
    ) -> BResult<()> {
        if self.phase != GraphPhase::Complete
            || self.failure.is_some()
            || self.source
                != Some(GraphSourceKey::new(
                    source,
                    counts,
                    options,
                    enumeration,
                    launch_upper_bound,
                    initial,
                    arguments,
                    prefix,
                ))
            || resources.original_ledger_v1() != Some(source.ledger)
            || self.launch_extent != Some(0)
            || self.local_definitions.len() != source.function.locals().len()
            || self.writers != Some(WriterFloor::take(arguments, prefix))
            || self.final_rows
                != Some((
                    arguments.edge_count,
                    initial.stores.len(),
                    initial.loads.len(),
                    arguments.borrowed_locals.len(),
                ))
        {
            return Err(self.saved());
        }
        self.check_custody(resources)
    }
}
fn check_unseeded(
    initial: &InitialDestinations,
    count: usize,
    resources: &mut Prep<'_, '_>,
) -> BResult<()> {
    if initial.indices.len() != count
        || initial.leaders.len() != count
        || initial.predicates.len() != count
    {
        return Err(accounting());
    }
    resources.work(count.checked_mul(3).ok_or_else(arithmetic)?)?;
    if initial.indices.iter().any(Option::is_some)
        || initial.leaders.iter().any(Option::is_some)
        || initial.predicates.iter().any(Option::is_some)
    {
        return Err(accounting());
    }
    Ok(())
}
#[cfg(test)]
#[path = "retained_initial_capability_graph_v1_tests.rs"]
mod tests;
