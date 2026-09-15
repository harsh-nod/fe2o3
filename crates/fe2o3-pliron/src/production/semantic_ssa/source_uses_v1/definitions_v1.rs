//! Indexes of existing planner outputs. No SSA or control-flow reconstruction.
use super::*;
use fe2o3_mir_model::{SsaArgumentV1, SsaEdgeIdV1, SsaEdgeRoleV1};

mod query;
#[cfg(test)]
mod tests;
pub use query::{
    ProductionSemanticSsaIncomingEdgeV1, ProductionSemanticSsaIncomingValuesV1,
    ProductionSemanticSsaValueOriginV1, ProductionSemanticSsaValueV1,
};

mod compact_v2;
use compact_v2::{DefinitionOrigin, DefinitionRow, IncomingRow};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct ValueOriginsV1 {
    entry: SsaBlockIdV1,
    definitions: Vec<DefinitionRow>,
    offsets: Vec<u32>,
    incoming: Vec<IncomingRow>,
}

impl Default for ValueOriginsV1 {
    fn default() -> Self {
        Self {
            entry: SsaBlockIdV1::new(0),
            definitions: Vec::new(),
            offsets: Vec::new(),
            incoming: Vec::new(),
        }
    }
}

impl ValueOriginsV1 {
    fn fixed<T: Clone>(count: usize, value: T) -> Result<Vec<T>, ProductionSemanticSsaErrorV1> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        if result.capacity() != count {
            return Err(ProductionSemanticSsaErrorV1::ResourceOverflow);
        }
        result.resize(count, value);
        Ok(result)
    }

    pub(in super::super) fn resources_for(
        plan: &SsaConstructionPlanV1,
    ) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
        let r = plan.resources();
        let blocks = r.input_blocks();
        let edges = r.input_edges();
        let defs = plan.definition_count();
        let word = std::mem::size_of::<usize>();
        let checked = || -> Option<SemanticSsaAuxiliaryResourcesV1> {
            // Exact-capacity retained arrays, replay's replacement, and the
            // construction cursor coexist. No growing vectors or query caches.
            let words = |count: usize, bytes: usize| {
                count.checked_mul(bytes).map(|bytes| bytes.div_ceil(word))
            };
            let payload = words(defs, std::mem::size_of::<DefinitionRow>())?
                .checked_add(words(edges, std::mem::size_of::<IncomingRow>())?)?
                .checked_add(words(blocks.checked_add(1)?, std::mem::size_of::<u32>())?)?;
            let storage_words = payload
                .checked_mul(2)?
                .checked_add(words(blocks, std::mem::size_of::<u32>())?)?
                .checked_add(2 * std::mem::size_of::<Self>().div_ceil(word))?
                .checked_add(std::mem::size_of::<Vec<u32>>().div_ceil(word))?;
            // Construction passes, initialization, full hashing and replay
            // comparison are bounded by this precharged linear reservation.
            let work_units = blocks
                .checked_add(edges)?
                .checked_add(defs)?
                .checked_add(r.input_events())?
                .checked_add(1)?
                .checked_mul(12)?;
            Some(SemanticSsaAuxiliaryResourcesV1 {
                storage_words,
                work_units,
            })
        };
        checked().ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
    }

    pub(in super::super) fn build(
        input: &SsaConstructionInputV1,
        plan: &SsaConstructionPlanV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let invalid = || ProductionSemanticSsaErrorV1::ReplayMismatch;
        let blocks = input.blocks().len();
        if blocks != plan.resources().input_blocks() {
            return Err(invalid());
        }
        let mut offsets = Self::fixed(
            blocks
                .checked_add(1)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            0_u32,
        )?;
        for (source, block) in input.blocks().iter().enumerate() {
            if !plan.is_reachable(SsaBlockIdV1::new(source as u32)) {
                continue;
            }
            for edge in block.edges() {
                if !plan.is_reachable(edge.target()) {
                    return Err(invalid());
                }
                let count = offsets
                    .get_mut(edge.target().get() as usize + 1)
                    .ok_or_else(invalid)?;
                *count = count
                    .checked_add(1)
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            }
        }
        for block in 0..blocks {
            offsets[block + 1] = offsets[block + 1]
                .checked_add(offsets[block])
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        }
        let count = offsets[blocks] as usize;
        if count > plan.resources().input_edges() {
            return Err(invalid());
        }
        let mut cursors = Self::fixed(blocks, 0_u32)?;
        cursors.copy_from_slice(&offsets[..blocks]);
        let mut result = Self {
            entry: input.entry(),
            definitions: Self::fixed(plan.definition_count(), DefinitionRow::MISSING)?,
            offsets,
            incoming: Self::fixed(
                count,
                IncomingRow::new(
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0),
                    SsaEdgeRoleV1::new(0),
                    SsaBlockIdV1::new(0),
                )?,
            )?,
        };
        for (argument, value) in plan.entry_definitions().iter().copied().enumerate() {
            result.define(value.value(), DefinitionOrigin::Entry { argument })?;
        }
        for (block_index, block) in input.blocks().iter().enumerate() {
            let source = SsaBlockIdV1::new(block_index as u32);
            if !plan.is_reachable(source) {
                continue;
            }
            for (event, resolved) in plan.resolved_events(source).ok_or_else(invalid)? {
                if let SsaResolvedEventV1::Define { value, .. } = resolved {
                    result.define(
                        *value,
                        DefinitionOrigin::Event {
                            block: source,
                            event: *event,
                        },
                    )?;
                }
            }
            for (ordinal, edge) in block.edges().iter().enumerate() {
                let target = edge.target().get() as usize;
                let position = *cursors.get(target).ok_or_else(invalid)?;
                if position >= result.offsets[target + 1] {
                    return Err(invalid());
                }
                let id = SsaEdgeIdV1::new(
                    source,
                    u32::try_from(ordinal)
                        .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?,
                );
                result.incoming[position as usize] =
                    IncomingRow::new(id, edge.role(), edge.target())?;
                cursors[target] = position
                    .checked_add(1)
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
                for (definition, value) in plan
                    .edge_definitions(id)
                    .ok_or_else(invalid)?
                    .iter()
                    .copied()
                    .enumerate()
                {
                    result.define(
                        value.value(),
                        DefinitionOrigin::Edge {
                            incoming: position as usize,
                            definition,
                        },
                    )?;
                }
            }
        }
        if result
            .definitions
            .iter()
            .any(|row| row.origin() == DefinitionOrigin::Missing)
            || cursors
                .iter()
                .enumerate()
                .any(|(block, cursor)| *cursor != result.offsets[block + 1])
        {
            return Err(invalid());
        }
        Ok(result)
    }

    fn define(
        &mut self,
        value: SsaValueV1,
        row: DefinitionOrigin,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let SsaValueV1::Definition(id) = value else {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        };
        let slot = self
            .definitions
            .get_mut(id.get() as usize)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        if *slot != DefinitionRow::MISSING {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        *slot = DefinitionRow::new(row)?;
        Ok(())
    }

    pub(in super::super) fn hash_into(&self, hash: &mut Sha256) {
        hash.update(b"fe2o3.semantic-ssa.value-origin-index.v2\0");
        hash.update(self.entry.get().to_le_bytes());
        hash.update((self.definitions.len() as u64).to_le_bytes());
        for row in &self.definitions {
            row.hash_into(hash);
        }
        hash.update((self.offsets.len() as u64).to_le_bytes());
        for offset in &self.offsets {
            hash.update(offset.to_le_bytes());
        }
        hash.update((self.incoming.len() as u64).to_le_bytes());
        for row in &self.incoming {
            row.hash_into(hash);
        }
    }
}
