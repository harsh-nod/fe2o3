// Original activation sites form a finite may-set. They are not dynamic lifetime
// identities: raw aliases must separately expire before every storage restart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceActivationOriginV29 {
    Entry,
    StorageLive(SourceReferenceSiteV29),
}

// Allocation demand only; no value, initialization, loan or live-pointer fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceStorageActivationV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    origin: SourceReferenceActivationOriginV29,
}

fn source_reference_activation_headers_v29() -> Result<usize, ProductionSemanticKirErrorV1> {
    argument_sum_v1(&[
        argument_product_v1(2, std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>())?,
        std::mem::size_of::<SourceReferenceStorageActivationV29>(),
    ]).map_err(Into::into)
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_storage_activation(
        &mut self,
        row: SourceReferenceStorageActivationV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.plan.has_storage_demands {
            return Ok(());
        }
        self.plan.check_owner(self.plan.instances, budget)?;
        let requests = self.storage_requests.ok_or(ArgumentResourceV1::Accounting)?;
        if !source_backing_original_request_v29(requests, row.instance, row.local, budget)? {
            return Ok(());
        }
        budget.reserve_storage(source_reference_activation_headers_v29()?)?;
        budget.charge_work(12)?;
        let instance = self.plan.instances.instance(row.instance)
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        if self.plan.instances.instance_reachable(row.instance) != Some(true)
            || instance.declaration().locals().get(row.local.index() as usize).is_none()
        {
            return Err(source_reference_cfg_obligation_v29());
        }
        match row.origin {
            SourceReferenceActivationOriginV29::Entry if row.generation == 0 => {}
            SourceReferenceActivationOriginV29::StorageLive(site) if site.instance == row.instance => {
                let statement = site.statement.ok_or_else(source_reference_cfg_obligation_v29)?;
                let original = instance.declaration().blocks().get(site.block.index() as usize)
                    .and_then(|block| block.statements().get(statement))
                    .ok_or_else(source_reference_cfg_obligation_v29)?;
                let first = self.epochs.get(row.instance.index())
                    .and_then(|blocks| blocks.get(site.block.index() as usize))
                    .copied().ok_or_else(source_reference_cfg_obligation_v29)?;
                if !matches!(original.kind(), SemanticStatementKindV1::StorageLive(local) if *local == row.local)
                    || u32::try_from(argument_sum_v1(&[first, statement])?)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)? != row.generation
                {
                    return Err(source_reference_cfg_obligation_v29());
                }
            }
            _ => return Err(source_reference_cfg_obligation_v29()),
        }
        self.intern_storage_activation(row, budget)
    }

    fn intern_storage_activation(
        &mut self,
        row: SourceReferenceStorageActivationV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let key = (row.instance.index(), row.local.index(), row.generation);
        charge_execution_cfg_lookup_v29(self.plan.storage_activation_sites.len(), budget)?;
        if let Some(&index) = self.plan.storage_activation_sites.get(&key) {
            budget.charge_work(2)?;
            if self.plan.storage_activations.get(index) != Some(&row) {
                return Err(source_reference_cfg_obligation_v29());
            }
            return Ok(());
        }
        // Pay both publications before either becomes visible. Vector growth
        // can fail; there is no fallible step after its successful append.
        reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), usize>(
            self.plan.storage_activation_sites.len(), budget)?;
        let index = self.plan.storage_activations.len();
        emission_push_v1(&mut self.plan.storage_activations, row, budget)?;
        self.plan.storage_activation_sites.insert(key, index);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceEpochSetV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    // Retain the canonical label for immutable, indexed recipe replay.
    generation: u32,
    first: usize,
    count: usize,
}

#[derive(Clone, Copy)]
enum SourceReferenceEpochAtomsV29 {
    Singleton(u32),
    Joined { first: usize, count: usize },
}

struct SourceReferenceEpochUnionV29 {
    left: SourceReferenceEpochAtomsV29,
    right: SourceReferenceEpochAtomsV29,
    left_index: usize,
    right_index: usize,
}

#[cfg(test)]
thread_local! {
    static SOURCE_RAW_RESTART_TEST_V29: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn test_raw_restart_probe(
        &self,
        site: SourceReferenceSiteV29,
        local: SemanticLocalIdV1,
    ) -> Option<(u32, usize)> {
        if !SOURCE_RAW_RESTART_TEST_V29.with(|observer| observer.get().is_some()) {
            return None;
        }
        let instance = site.instance;
        let generation = u32::try_from(
            self.epochs[instance.index()][site.block.index() as usize] + site.statement.unwrap(),
        )
        .unwrap();
        let mut current = 0;
        for frame in self.frames.iter().flatten() {
            for holder in &self.plan.states[*frame] {
                let Some(node) = holder.node else {
                    continue;
                };
                let SourceReferenceNodeKindV29::Address(set) = self.plan.nodes[node].kind else {
                    continue;
                };
                let row = self.plan.raw_sets[set];
                if row.instance == instance && row.local == local {
                    current += self.plan.raw_choices[row.first..row.first + row.count]
                        .iter()
                        .filter(|choice| {
                            !choice.expired
                                && self.plan.raw_origins[choice.origin].generation == generation
                        })
                        .count();
                }
            }
        }
        Some((generation, current))
    }

    fn test_raw_restart_observe(
        &self,
        site: SourceReferenceSiteV29,
        local: SemanticLocalIdV1,
        generation: u32,
        before: Option<(u32, usize)>,
    ) {
        if before.is_some_and(|(previous, count)| previous == generation && count > 0) {
            let after = self.test_raw_restart_probe(site, local).unwrap().1;
            SOURCE_RAW_RESTART_TEST_V29.with(|observer| {
                let (same_site, expired) = observer.get().unwrap();
                observer.set(Some((same_site + 1, expired + usize::from(after == 0))));
            });
        }
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn epochs_overlap(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        left: u32,
        right: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if left == u32::MAX || right == u32::MAX {
            return Ok(false);
        }
        if left == right {
            return Ok(true);
        }
        let limit = self.epoch_limit(instance, budget)?;
        let left = self.epoch_atoms(instance, local, left, limit, budget)?;
        let right = self.epoch_atoms(instance, local, right, limit, budget)?;
        let (mut a, mut b) = (0usize, 0usize);
        while let (Some(x), Some(y)) = (
            self.epoch_atom(left, a, budget)?,
            self.epoch_atom(right, b, budget)?,
        ) {
            budget.charge_work(1)?;
            if x == y {
                return Ok(true);
            }
            if x < y {
                a = argument_sum_v1(&[a, 1])?;
            } else {
                b = argument_sum_v1(&[b, 1])?;
            }
        }
        Ok(false)
    }

    fn epoch_includes(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        current: u32,
        captured: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if current == u32::MAX || captured == u32::MAX {
            return Ok(false);
        }
        if current == captured {
            return Ok(true);
        }
        let limit = self.epoch_limit(instance, budget)?;
        let current = self.epoch_atoms(instance, local, current, limit, budget)?;
        let captured = self.epoch_atoms(instance, local, captured, limit, budget)?;
        let (mut a, mut b) = (0usize, 0usize);
        while let Some(wanted) = self.epoch_atom(captured, b, budget)? {
            loop {
                let Some(found) = self.epoch_atom(current, a, budget)? else {
                    return Ok(false);
                };
                budget.charge_work(1)?;
                if found > wanted {
                    return Ok(false);
                }
                a = argument_sum_v1(&[a, 1])?;
                if found == wanted {
                    break;
                }
            }
            b = argument_sum_v1(&[b, 1])?;
        }
        Ok(true)
    }

    fn epoch_limit(
        &self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<u32, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let function = self
            .plan
            .instances
            .instance(instance)
            .ok_or_else(source_reference_cfg_obligation_v29)?
            .declaration();
        let last = function
            .blocks()
            .last()
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        let first = *self
            .epochs
            .get(instance.index())
            .and_then(|epochs| epochs.last())
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        u32::try_from(argument_sum_v1(&[first, last.statements().len()])?)
            .map_err(|_| ArgumentResourceV1::Arithmetic.into())
    }

    fn epoch_atoms(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        generation: u32,
        limit: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceEpochAtomsV29, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if generation == u32::MAX {
            return Err(source_reference_error_v29(
                "source address has no live storage activation",
            ));
        }
        if generation < limit {
            let function = self
                .plan
                .instances
                .instance(instance)
                .ok_or_else(source_reference_cfg_obligation_v29)?
                .declaration();
            if function.locals().get(local.index() as usize).is_none() {
                return Err(source_reference_cfg_obligation_v29());
            }
            if generation != 0 {
                let offsets = self
                    .epochs
                    .get(instance.index())
                    .ok_or_else(source_reference_cfg_obligation_v29)?;
                charge_execution_cfg_lookup_v29(offsets.len(), budget)?;
                let block = offsets
                    .partition_point(|first| *first <= generation as usize)
                    .checked_sub(1)
                    .ok_or_else(source_reference_cfg_obligation_v29)?;
                let statement = generation as usize - offsets[block];
                budget.charge_work(3)?;
                if !matches!(function.blocks().get(block)
                    .and_then(|row| row.statements().get(statement)).map(|row| row.kind()),
                    Some(SemanticStatementKindV1::StorageLive(found)) if *found == local)
                {
                    return Err(source_reference_error_v29(
                        "source activation singleton is not its original StorageLive",
                    ));
                }
            }
            return Ok(SourceReferenceEpochAtomsV29::Singleton(generation));
        }
        let row = self
            .plan
            .epoch_sets
            .get((generation - limit) as usize)
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        if row.instance != instance || row.local != local || row.generation != generation || row.count < 2 {
            return Err(source_reference_error_v29(
                "source activation set changed its original object",
            ));
        }
        let end = argument_sum_v1(&[row.first, row.count])?;
        if end > self.plan.epoch_members.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(SourceReferenceEpochAtomsV29::Joined {
            first: row.first,
            count: row.count,
        })
    }

    fn epoch_atom(
        &self,
        atoms: SourceReferenceEpochAtomsV29,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        match atoms {
            SourceReferenceEpochAtomsV29::Singleton(value) => Ok((index == 0).then_some(value)),
            SourceReferenceEpochAtomsV29::Joined { first, count } => {
                if index >= count {
                    return Ok(None);
                }
                self.plan
                    .epoch_members
                    .get(argument_sum_v1(&[first, index])?)
                    .copied()
                    .map(Some)
                    .ok_or_else(|| ArgumentResourceV1::Accounting.into())
            }
        }
    }

    fn next_epoch_union(
        &self,
        union: &mut SourceReferenceEpochUnionV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        let left = self.epoch_atom(union.left, union.left_index, budget)?;
        let right = self.epoch_atom(union.right, union.right_index, budget)?;
        budget.charge_work(1)?;
        Ok(match (left, right) {
            (None, None) => None,
            (Some(a), Some(b)) if a == b => {
                union.left_index = argument_sum_v1(&[union.left_index, 1])?;
                union.right_index = argument_sum_v1(&[union.right_index, 1])?;
                Some(a)
            }
            (Some(a), Some(b)) if a < b => {
                union.left_index = argument_sum_v1(&[union.left_index, 1])?;
                Some(a)
            }
            (Some(a), None) => {
                union.left_index = argument_sum_v1(&[union.left_index, 1])?;
                Some(a)
            }
            (_, Some(b)) => {
                union.right_index = argument_sum_v1(&[union.right_index, 1])?;
                Some(b)
            }
        })
    }

    fn join_storage_epochs(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        left: u32,
        right: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<u32, ProductionSemanticKirErrorV1> {
        if left == right {
            return Ok(left);
        }
        // Equality uses a virtual sorted merge. No candidate backing is allocated
        // before proving that this original object's canonical set is absent.
        budget.reserve_storage(argument_sum_v1(&[
            2 * std::mem::size_of::<SourceReferenceEpochAtomsV29>(),
            std::mem::size_of::<SourceReferenceEpochUnionV29>(),
            2 * std::mem::size_of::<
                Result<SourceReferenceEpochAtomsV29, ProductionSemanticKirErrorV1>,
            >(),
            4 * std::mem::size_of::<Result<Option<u32>, ProductionSemanticKirErrorV1>>(),
            2 * std::mem::size_of::<Option<u32>>(),
            2 * std::mem::size_of::<Result<u32, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let limit = self.epoch_limit(instance, budget)?;
        let left = self.epoch_atoms(instance, local, left, limit, budget)?;
        let right = self.epoch_atoms(instance, local, right, limit, budget)?;
        let key = (instance.index(), local.index());
        charge_execution_cfg_lookup_v29(self.plan.epoch_objects.len(), budget)?;
        if let Some(indices) = self.plan.epoch_objects.get(&key) {
            for &index in indices {
                budget.charge_work(1)?;
                let row = self
                    .plan
                    .epoch_sets
                    .get(index)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let mut union = SourceReferenceEpochUnionV29 {
                    left,
                    right,
                    left_index: 0,
                    right_index: 0,
                };
                let mut position = 0usize;
                let mut same = true;
                while let Some(atom) = self.next_epoch_union(&mut union, budget)? {
                    budget.charge_work(1)?;
                    if position >= row.count
                        || self
                            .plan
                            .epoch_members
                            .get(argument_sum_v1(&[row.first, position])?)
                            != Some(&atom)
                    {
                        same = false;
                        break;
                    }
                    position = argument_sum_v1(&[position, 1])?;
                }
                if same && position == row.count {
                    return u32::try_from(argument_sum_v1(&[limit as usize, index])?)
                        .map_err(|_| ArgumentResourceV1::Arithmetic.into());
                }
            }
        }
        let index = self.plan.epoch_sets.len();
        let generation = u32::try_from(argument_sum_v1(&[limit as usize, index])?)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
        if generation == u32::MAX {
            return Err(ArgumentResourceV1::Arithmetic.into());
        }
        let first = self.plan.epoch_members.len();
        let mut union = SourceReferenceEpochUnionV29 {
            left,
            right,
            left_index: 0,
            right_index: 0,
        };
        while let Some(atom) = self.next_epoch_union(&mut union, budget)? {
            emission_push_v1(&mut self.plan.epoch_members, atom, budget)?;
        }
        let count = self
            .plan
            .epoch_members
            .len()
            .checked_sub(first)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if count < 2 {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        emission_push_v1(
            &mut self.plan.epoch_sets,
            SourceReferenceEpochSetV29 {
                instance,
                local,
                generation,
                first,
                count,
            },
            budget,
        )?;
        charge_execution_cfg_lookup_v29(self.plan.epoch_objects.len(), budget)?;
        if !self.plan.epoch_objects.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<(usize, u32), Vec<usize>>(
                self.plan.epoch_objects.len(),
                budget,
            )?;
            self.plan.epoch_objects.insert(key, Vec::new());
        }
        emission_push_v1(
            self.plan
                .epoch_objects
                .get_mut(&key)
                .ok_or(ArgumentResourceV1::Accounting)?,
            index,
            budget,
        )?;
        Ok(generation)
    }
}
