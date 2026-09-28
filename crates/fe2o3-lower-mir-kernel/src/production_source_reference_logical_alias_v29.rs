// Equation inputs only. A source/output producer must authenticate the original
// alias recipe, exact checked rewrite/use lineage and boundary ordering before
// invoking these equations. None of these rows is a source-admission token.
#[derive(Clone, Copy)]
enum SourceAddressAliasInputV29 {
    Physical(ValueId),
    Logical(usize),
}

#[derive(Clone, Copy)]
enum SourceAddressAliasRecipeV29 {
    Birth,
    Inherit(SourceAddressAliasInputV29),
    Select(SourceAddressAliasInputV29, SourceAddressAliasInputV29),
}

#[derive(Clone, Copy)]
struct SourceAddressLogicalAliasV29 {
    pointer: ValueId,
    slot: usize,
    recipe: SourceAddressAliasRecipeV29,
}

#[derive(Clone, Copy)]
struct SourceAddressLogicalUseV29 {
    block: BlockId,
    operation: Option<usize>,
    successor: Option<usize>,
    operand: usize,
    value: ValueId,
    alias: usize,
}

impl SourceAddressLogicalUseV29 {
    fn key(&self) -> [usize; 4] {
        [
            self.block.0 as usize,
            self.operation.unwrap_or(usize::MAX),
            self.successor.unwrap_or(usize::MAX),
            self.operand,
        ]
    }
}

#[derive(Clone, Copy)]
enum SourceAddressBoundaryKindV29 {
    Lifetime(usize),
    Kill(usize),
    Alias(usize),
}

#[derive(Clone, Copy)]
struct SourceAddressBoundaryEventV29 {
    block: BlockId,
    gap: usize,
    kind: SourceAddressBoundaryKindV29,
}

#[derive(Clone, Copy, Default)]
struct SourceAddressAliasTransportV29<'a> {
    aliases: &'a [SourceAddressLogicalAliasV29],
    uses: &'a [SourceAddressLogicalUseV29],
    // None preserves the original graph's lifetime-then-kill boundary order.
    // Some is a complete, interleaved order, not a supplemental event list.
    boundaries: Option<&'a [SourceAddressBoundaryEventV29]>,
}

#[derive(Default)]
struct SourceAddressBoundaryCursorV29 {
    event: usize,
    lifetime: usize,
    kill: usize,
}

impl SourceAddressBoundaryCursorV29 {
    fn next_at(
        &mut self,
        block: BlockId,
        gap: usize,
        lifetimes: &[SourceAddressLifetimeV29],
        kills: &[SourceAddressKillV29],
        transport: SourceAddressAliasTransportV29<'_>,
    ) -> Option<SourceAddressBoundaryKindV29> {
        if let Some(events) = transport.boundaries {
            let event = events
                .get(self.event)
                .filter(|row| (row.block, row.gap) == (block, gap))?;
            self.event += 1;
            return Some(event.kind);
        }
        if lifetimes
            .get(self.lifetime)
            .is_some_and(|row| (row.block, row.gap) == (block, gap))
        {
            let index = self.lifetime;
            self.lifetime += 1;
            return Some(SourceAddressBoundaryKindV29::Lifetime(index));
        }
        if kills
            .get(self.kill)
            .is_some_and(|row| (row.block, row.gap) == (block, gap))
        {
            let index = self.kill;
            self.kill += 1;
            return Some(SourceAddressBoundaryKindV29::Kill(index));
        }
        None
    }

    fn complete(
        &self,
        lifetimes: usize,
        kills: usize,
        transport: SourceAddressAliasTransportV29<'_>,
    ) -> bool {
        match transport.boundaries {
            Some(events) => self.event == events.len(),
            None => self.lifetime == lifetimes && self.kill == kills,
        }
    }
}

impl SourceAddressAliasTransportV29<'_> {
    fn validate(
        self,
        graph: &SourceAddressMemoryV29<'_>,
        slots: &[ScopedSourceSlotV29],
        lifetimes: &[SourceAddressLifetimeV29],
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(events) = self.boundaries else {
            return if self.aliases.is_empty() && self.uses.is_empty() {
                Ok(())
            } else {
                Err(source_raw_physical_error_v29())
            };
        };
        let count = argument_sum_v1(&[lifetimes.len(), kills.len(), self.aliases.len()])?;
        if events.len() != count {
            return Err(source_raw_physical_error_v29());
        }
        budget.reserve_storage(std::mem::size_of::<Vec<bool>>())?;
        let mut seen = emission_vec_v1(count, budget)?;
        budget.charge_work(count)?;
        seen.resize(count, false);
        let mut previous = None;
        let (mut next_lifetime, mut next_kill) = (0, 0);
        for event in events {
            budget.charge_work(3)?;
            let point = (event.block, event.gap);
            if previous.is_some_and(|previous| previous > point)
                || event.gap
                    > graph.blocks[graph.block(event.block, budget)?]
                        .1
                        .operations
                        .len()
            {
                return Err(source_raw_physical_error_v29());
            }
            let index = match event.kind {
                SourceAddressBoundaryKindV29::Lifetime(index) => {
                    let row = lifetimes
                        .get(index)
                        .ok_or_else(source_raw_physical_error_v29)?;
                    if (row.block, row.gap) != point || index != next_lifetime {
                        return Err(source_raw_physical_error_v29());
                    }
                    next_lifetime += 1;
                    index
                }
                SourceAddressBoundaryKindV29::Kill(index) => {
                    let row = kills.get(index).ok_or_else(source_raw_physical_error_v29)?;
                    if (row.block, row.gap) != point || index != next_kill {
                        return Err(source_raw_physical_error_v29());
                    }
                    next_kill += 1;
                    argument_sum_v1(&[lifetimes.len(), index])?
                }
                SourceAddressBoundaryKindV29::Alias(index) => {
                    if index >= self.aliases.len() {
                        return Err(source_raw_physical_error_v29());
                    }
                    argument_sum_v1(&[lifetimes.len(), kills.len(), index])?
                }
            };
            if seen[index] {
                return Err(source_raw_physical_error_v29());
            }
            seen[index] = true;
            previous = Some(point);
        }
        for alias in self.aliases {
            budget.charge_work(2)?;
            if alias.slot >= slots.len()
                || graph.origins[graph.value(alias.pointer, budget)?]
                    != SourceAddressOriginV29::Exact(Some(alias.slot))
            {
                return Err(source_raw_physical_error_v29());
            }
            if matches!(alias.recipe, SourceAddressAliasRecipeV29::Birth)
                && alias.pointer != slots[alias.slot].origin.pointer
            {
                return Err(source_raw_physical_error_v29());
            }
            let inputs = match alias.recipe {
                SourceAddressAliasRecipeV29::Birth => [None, None],
                SourceAddressAliasRecipeV29::Inherit(value) => [Some(value), None],
                SourceAddressAliasRecipeV29::Select(left, right) => [Some(left), Some(right)],
            };
            for input in inputs.into_iter().flatten() {
                budget.charge_work(1)?;
                let slot = match input {
                    SourceAddressAliasInputV29::Physical(value) => {
                        match graph.origins[graph.value(value, budget)?] {
                            SourceAddressOriginV29::Exact(Some(slot)) => slot,
                            _ => return Err(source_raw_physical_error_v29()),
                        }
                    }
                    SourceAddressAliasInputV29::Logical(index) => {
                        self.aliases
                            .get(index)
                            .ok_or_else(source_raw_physical_error_v29)?
                            .slot
                    }
                };
                if slot != alias.slot {
                    return Err(source_raw_physical_error_v29());
                }
            }
        }
        let mut previous = None;
        for usage in self.uses {
            budget.charge_work(4)?;
            let alias = self
                .aliases
                .get(usage.alias)
                .ok_or_else(source_raw_physical_error_v29)?;
            let key = usage.key();
            if previous.is_some_and(|previous| previous >= key)
                || usage.operation.is_some() && usage.successor.is_some()
                || usage.value != alias.pointer
                || graph.origins[graph.value(usage.value, budget)?]
                    != SourceAddressOriginV29::Exact(Some(alias.slot))
            {
                return Err(source_raw_physical_error_v29());
            }
            graph.block(usage.block, budget)?;
            previous = Some(key);
        }
        let bytes = argument_sum_v1(&[std::mem::size_of::<Vec<bool>>(), seen.capacity()])?;
        drop(seen);
        budget.release_storage(bytes)?;
        Ok(())
    }
}
