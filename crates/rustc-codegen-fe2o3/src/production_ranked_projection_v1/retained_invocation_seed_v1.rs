//! Retained ThreadIndex1d seed continuation, before recovery or propagation.
//! Raw component inputs are DATA; only the whole-owner bridge binds actual facts.
use super::*;
use crate::production_ranked_projection_v1::root_invocation_index_preparation_v1 as donor;
#[path = "retained_invocation_seed_frame_v1.rs"]
mod frame;
#[path = "whole_root_invocation_seed_bridge_v1.rs"]
mod whole;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeedPhase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct SeedKey {
    graph: GraphSourceKey,
    graph_owner: usize,
    definitions: (usize, usize),
    address_escaped: (usize, usize),
}
impl SeedKey {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source: &Source<'_>,
        counts: &[u8],
        escaped: &[bool],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch: Option<u64>,
        graph: &RetainedInitialCapabilityGraphV1,
        initial: &InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
    ) -> Self {
        Self {
            graph: GraphSourceKey::new(
                source,
                counts,
                options,
                enumeration,
                launch,
                initial,
                arguments,
                prefix,
            ),
            graph_owner: graph as *const _ as usize,
            definitions: (
                graph.local_definitions.as_ptr() as usize,
                graph.local_definitions.len(),
            ),
            address_escaped: (escaped.as_ptr() as usize, escaped.len()),
        }
    }
}
pub(in super::super) struct RetainedInvocationSeedV1 {
    phase: SeedPhase,
    key: Option<SeedKey>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    failure: Option<Backend>,
    before: Option<WriterFloor>,
    after: Option<WriterFloor>,
    graph_rows: Option<(usize, usize, usize, usize)>,
    index_fifo: Vec<usize>,
    index_cursor: usize,
    grid_fifo: Vec<usize>,
    grid_cursor: usize,
}
impl RetainedInvocationSeedV1 {
    pub(in super::super) fn new() -> Self {
        Self {
            phase: SeedPhase::Fresh,
            key: None,
            entry: None,
            held: None,
            failure: None,
            before: None,
            after: None,
            graph_rows: None,
            index_fifo: Vec::new(),
            index_cursor: 0,
            grid_fifo: Vec::new(),
            grid_cursor: 0,
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
        escaped: &[bool],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch: Option<u64>,
        graph: &RetainedInitialCapabilityGraphV1,
        initial: &mut InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &mut RootEntryPrefixV1,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        let fresh = self.phase == SeedPhase::Fresh;
        self.phase = SeedPhase::Terminal;
        if !fresh {
            return Err(self.saved());
        }
        self.key = Some(SeedKey::new(
            source,
            counts,
            escaped,
            options,
            enumeration,
            launch,
            graph,
            initial,
            arguments,
            prefix,
        ));
        self.entry = resources.retained_custody_snapshot_v1();
        let result = self.prepare_original(
            source,
            counts,
            escaped,
            options,
            enumeration,
            launch,
            graph,
            initial,
            arguments,
            prefix,
            resources,
        );
        match result {
            Ok(()) => {
                self.held = resources.retained_custody_snapshot_v1();
                self.phase = SeedPhase::Complete;
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
        escaped: &[bool],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch: Option<u64>,
        graph: &RetainedInitialCapabilityGraphV1,
        initial: &mut InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &mut RootEntryPrefixV1,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        self.check_custody(resources)?;
        if resources.original_ledger_v1() != Some(source.ledger) {
            return Err(accounting());
        }
        // The predecessor binds the original owner counter before any new
        // debit. This completion is checked only BEFORE seed mutation and must
        // never be reused as the seed completion's writer/row floor afterwards.
        graph.completed_for(
            source,
            counts,
            options,
            enumeration,
            launch,
            initial,
            arguments,
            prefix,
            resources,
        )?;
        resources.work(32)?;
        resources.reserve_storage(frame::bytes()?)?;
        let count = source.function.locals().len();
        if count > 4096
            || source.function.blocks().len() > 32
            || source.types.len() > 4096
            || source.callables.len() > 4096
            || counts.len() != count
            || escaped.len() != count
            || !self.index_fifo.is_empty()
            || self.index_fifo.capacity() != 0
            || !self.grid_fifo.is_empty()
            || self.grid_fifo.capacity() != 0
            || self.index_cursor != 0
            || self.grid_cursor != 0
        {
            return Err(accounting());
        }
        check_unseeded(initial, count, resources)?;
        resources.work(count)?;
        if graph.local_definitions.as_slice() != counts {
            return Err(accounting());
        }
        self.before = Some(WriterFloor::take(arguments, prefix));
        self.graph_rows = Some((
            arguments.edge_count,
            initial.stores.len(),
            initial.loads.len(),
            arguments.borrowed_locals.len(),
        ));
        // Every destination and both empty FIFO owners are already attached.
        // The exact donor emits the operation/value before scalar-custody
        // refusal, and writes the index before its fallible queue append.
        donor::seed_retained_root_invocation_values_v1(
            source.callables,
            source.function,
            &graph.local_definitions,
            escaped,
            options,
            &mut initial.indices,
            &mut initial.leaders,
            &mut initial.predicates,
            &mut self.index_fifo,
            &mut self.index_cursor,
            &mut self.grid_fifo,
            &mut self.grid_cursor,
            &mut prefix.entry_operations,
            &mut prefix.next_value,
            resources,
        )?;
        self.after = Some(WriterFloor::take(arguments, prefix));
        self.check_seed_rows(
            source, counts, escaped, graph, initial, arguments, prefix, resources,
        )?;
        self.check_custody(resources)
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
    fn check_seed_rows(
        &self,
        source: &Source<'_>,
        counts: &[u8],
        escaped: &[bool],
        graph: &RetainedInitialCapabilityGraphV1,
        initial: &InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        let count = source.function.locals().len();
        let before = self.before.ok_or_else(accounting)?;
        let after = self.after.ok_or_else(accounting)?;
        let seeds = self.index_fifo.len();
        if counts.len() != count
            || escaped.len() != count
            || graph.local_definitions.len() != count
            || initial.indices.len() != count
            || initial.leaders.len() != count
            || initial.predicates.len() != count
            || seeds > count
            || self.index_cursor != 0
            || self.grid_cursor != 0
            || !self.grid_fifo.is_empty()
            || self.grid_fifo.capacity() != 0
            || after != WriterFloor::take(arguments, prefix)
            || after.next_argument != before.next_argument
            || after.index_rows != before.index_rows
            || after.slice_rows != before.slice_rows
            || after.operations
                != before
                    .operations
                    .checked_add(seeds)
                    .ok_or_else(arithmetic)?
            || after.next_value
                != before
                    .next_value
                    .checked_add(u32::try_from(seeds).map_err(|_| arithmetic())?)
                    .ok_or_else(arithmetic)?
            || self.graph_rows
                != Some((
                    arguments.edge_count,
                    initial.stores.len(),
                    initial.loads.len(),
                    arguments.borrowed_locals.len(),
                ))
        {
            return Err(accounting());
        }
        resources.work(
            count
                .checked_mul(4)
                .and_then(|n| seeds.checked_mul(16).and_then(|m| n.checked_add(m)))
                .and_then(|n| n.checked_add(32))
                .ok_or_else(arithmetic)?,
        )?;
        let mut seeded = 0usize;
        for row in &initial.indices {
            seeded += usize::from(row.is_some());
        }
        if graph.local_definitions.as_slice() != counts
            || seeded != seeds
            || initial.leaders.iter().any(Option::is_some)
            || initial.predicates.iter().any(Option::is_some)
        {
            return Err(accounting());
        }
        for (ordinal, destination) in self.index_fifo.iter().copied().enumerate() {
            let result = ProductionRankedValueIdV1::new(
                before
                    .next_value
                    .checked_add(u32::try_from(ordinal).map_err(|_| arithmetic())?)
                    .ok_or_else(arithmetic)?,
            );
            if counts.get(destination).copied() != Some(1)
                || escaped.get(destination).copied() != Some(false)
                || initial.indices.get(destination).copied().flatten()
                    != Some(ProjectedDisjointIndexV1 {
                        value: ProductionRankedValueV1::Local(result),
                        mapping: SemanticDisjointIndexSpaceV1::Index1d,
                        precondition: None,
                        availability: None,
                    })
                || prefix.entry_operations.get(
                    before
                        .operations
                        .checked_add(ordinal)
                        .ok_or_else(arithmetic)?,
                ) != Some(&ProductionRankedOperationV1::InvocationIndex {
                    result,
                    dimension: 0,
                    launch_extent: 0,
                })
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
        escaped: &[bool],
        options: &SemanticOptionDominanceV1,
        enumeration: &SemanticEnumPayloadDominanceV1,
        launch: Option<u64>,
        graph: &RetainedInitialCapabilityGraphV1,
        initial: &InitialDestinations,
        arguments: &RetainedBeforeArgumentWritersV1,
        prefix: &RootEntryPrefixV1,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        if self.phase != SeedPhase::Complete
            || self.failure.is_some()
            || self.key
                != Some(SeedKey::new(
                    source,
                    counts,
                    escaped,
                    options,
                    enumeration,
                    launch,
                    graph,
                    initial,
                    arguments,
                    prefix,
                ))
            || resources.original_ledger_v1() != Some(source.ledger)
            || graph.phase != GraphPhase::Complete
            || graph.failure.is_some()
            || graph.source != self.key.map(|key| key.graph)
            || graph.launch_extent != Some(0)
        {
            return Err(self.saved());
        }
        self.check_custody(resources)?;
        graph.check_custody(resources)?;
        self.check_seed_rows(
            source, counts, escaped, graph, initial, arguments, prefix, resources,
        )
    }
}
#[cfg(test)]
#[path = "retained_invocation_seed_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "retained_invocation_seed_genuine_v1_tests.rs"]
pub(in super::super) mod genuine;
