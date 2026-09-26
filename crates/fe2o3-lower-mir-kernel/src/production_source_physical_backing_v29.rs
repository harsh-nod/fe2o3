// One physical allocation can implement a single original Rust local across
// joined activation paths. Logical generations remain separate source records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferencePhysicalBackingV29 {
    key: (usize, u32, u32),
    cell: usize,
    representative: usize,
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_physical_backing_group(
        &mut self,
        cells: &BTreeMap<(usize, u32, u32), usize>,
        key: (usize, u32),
        representative: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let expected = *self.plan.cells.rows.get(representative).ok_or_else(source_backing_error_v29)?;
        if !matches!(expected.kind, SourceBackingKindV29::Object(_)) { return Err(source_backing_error_v29()); }
        budget.charge_work(call_splice_search_work_v1(cells.len()))?;
        for (&key, &cell) in cells.range((key.0, key.1, 0)..=(key.0, key.1, u32::MAX)) {
            budget.charge_work(6)?;
            let row = self.plan.cells.rows.get(cell).ok_or_else(source_backing_error_v29)?;
            if row.instance != expected.instance || row.local != expected.local
                || row.ty != expected.ty || row.kind != expected.kind
                || self.plan.cells.physical_backings.last().is_some_and(|previous| previous.key >= key)
            { return Err(source_backing_error_v29()); }
            emission_push_v1(&mut self.plan.cells.physical_backings,
                SourceReferencePhysicalBackingV29 { key, cell, representative }, budget)?;
        }
        Ok(())
    }

    fn plan_physical_backings(
        &mut self,
        cells: &BTreeMap<(usize, u32, u32), usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.plan.check_owner(self.plan.instances, budget)?;
        budget.charge_work(3)?;
        if cells.len() != self.plan.cells.rows.len() || !self.plan.cells.physical_backings.is_empty() {
            return Err(source_backing_error_v29());
        }
        if cells.is_empty() { return Ok(()); }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Option<((usize, u32), usize, SourceReferenceScalarCellV29, u32, bool)>>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut group: Option<((usize, u32), usize, SourceReferenceScalarCellV29, u32, bool)> = None;
        for (&key, &cell) in cells {
            budget.charge_work(6)?;
            let row = *self.plan.cells.rows.get(cell).ok_or_else(source_backing_error_v29)?;
            if key != (row.instance.index(), row.local.index(), row.generation) {
                return Err(source_backing_error_v29());
            }
            let local_key = (key.0, key.1);
            if group.is_some_and(|current| current.0 != local_key) {
                let (key, representative, _, _, joined) = group.take().ok_or_else(source_backing_error_v29)?;
                if joined { self.retain_physical_backing_group(cells, key, representative, budget)?; }
            }
            if group.is_none() {
                let original = self.plan.instances.instance(row.instance)
                    .ok_or_else(source_backing_error_v29)?.declaration();
                if original.locals().get(row.local.index() as usize).map(|local| local.ty()) != Some(row.ty)
                    || self.plan.instances.instance_reachable(row.instance) != Some(true)
                { return Err(source_backing_error_v29()); }
                let limit = self.epoch_limit(row.instance, budget)?;
                group = Some((local_key, cell, row, limit, false));
            }
            let (_, _, first, limit, joined) = group.as_mut().ok_or_else(source_backing_error_v29)?;
            if row.ty != first.ty {
                return Err(source_backing_error_v29());
            }
            if !matches!(row.kind, SourceBackingKindV29::Object(_)) { continue; }
            let SourceReferenceEpochAtomsV29::Joined { first, count } =
                self.epoch_atoms(row.instance, row.local, row.generation, *limit, budget)?
            else { continue; };
            let mut previous = None;
            for index in 0..count {
                budget.charge_work(4)?;
                let atom = *self.plan.epoch_members.get(argument_sum_v1(&[first, index])?)
                    .ok_or_else(source_backing_error_v29)?;
                if previous.is_some_and(|value| value >= atom) || atom >= *limit {
                    return Err(source_backing_error_v29());
                }
                if !matches!(self.epoch_atoms(row.instance, row.local, atom, *limit, budget)?,
                    SourceReferenceEpochAtomsV29::Singleton(value) if value == atom)
                { return Err(source_backing_error_v29()); }
                charge_execution_cfg_lookup_v29(cells.len(), budget)?;
                let atomic_cell = *cells.get(&(key.0, key.1, atom)).ok_or_else(source_backing_error_v29)?;
                let actual = self.plan.cells.rows.get(atomic_cell).ok_or_else(source_backing_error_v29)?;
                if (actual.instance, actual.local, actual.generation, actual.ty, actual.kind)
                    != (row.instance, row.local, atom, row.ty, row.kind)
                { return Err(source_backing_error_v29()); }
                previous = Some(atom);
            }
            *joined = true;
        }
        if let Some((key, representative, _, _, true)) = group {
            self.retain_physical_backing_group(cells, key, representative, budget)?;
        }
        Ok(())
    }
}

impl SourceReferencePlanV29<'_, '_> {
    fn physical_object_cell(
        &self,
        cell: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(usize, SourceReferenceScalarCellV29, bool), ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        source_physical_backing_cell_v29(&self.cells.rows, &self.cells.physical_backings, cell, budget)
            .inspect_err(|error| source_reference_record_failure_v29(self, error))
    }

    fn physical_object_generation(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        generation: u32,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<(usize, usize, SourceReferenceScalarCellV29)>, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        let result = (|| {
            budget.source_reference_charge_v29(self,
                argument_sum_v1(&[call_splice_search_work_v1(self.cells.physical_backings.len()), 4])?)?;
            let key = (instance.index(), local.index(), generation);
            let Ok(index) = self.cells.physical_backings.binary_search_by_key(&key, |row| row.key) else {
                return Ok(None);
            };
            let mapping = self.cells.physical_backings[index];
            let logical = self.cells.rows.get(mapping.cell).ok_or_else(source_backing_error_v29)?;
            if (logical.instance, logical.local, logical.generation) != (instance, local, generation) {
                return Err(source_backing_error_v29());
            }
            let (representative, physical, joined) = self.physical_object_cell(mapping.cell, budget)?;
            if !joined { return Err(source_backing_error_v29()); }
            Ok(Some((mapping.cell, representative, physical)))
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }
}

// Structural lookup only. The caller authenticates the owning source plan;
// construction and the complete slot census separately bind these rows.
fn source_physical_backing_cell_v29(
    cells: &[SourceReferenceScalarCellV29],
    rows: &[SourceReferencePhysicalBackingV29],
    cell: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(usize, SourceReferenceScalarCellV29, bool), ProductionSemanticKirErrorV1> {
    budget.charge_work(10)?;
    let logical = *cells.get(cell).ok_or_else(source_backing_error_v29)?;
    if !matches!(logical.kind, SourceBackingKindV29::Object(_)) { return Err(source_backing_error_v29()); }
    budget.charge_work(call_splice_search_work_v1(rows.len()))?;
    let key = (logical.instance.index(), logical.local.index(), logical.generation);
    let Ok(index) = rows.binary_search_by_key(&key, |row| row.key) else {
        return Ok((cell, logical, false));
    };
    let mapping = rows[index];
    let physical = *cells.get(mapping.representative).ok_or_else(source_backing_error_v29)?;
    if mapping.cell != cell || physical.instance != logical.instance || physical.local != logical.local
        || physical.ty != logical.ty || physical.kind != logical.kind || physical.generation > logical.generation
    { return Err(source_backing_error_v29()); }
    budget.charge_work(call_splice_search_work_v1(rows.len()))?;
    let representative_key = (physical.instance.index(), physical.local.index(), physical.generation);
    let representative = rows.binary_search_by_key(&representative_key, |row| row.key)
        .ok().and_then(|index| rows.get(index)).ok_or_else(source_backing_error_v29)?;
    if representative.cell != mapping.representative || representative.representative != mapping.representative {
        return Err(source_backing_error_v29());
    }
    Ok((mapping.representative, physical, true))
}

// The physical allocation census marks canonical representatives first. Every
// logical member is then joined to that exact checked slot, never just to an
// equal schema or a currently live epoch label. Legacy arrays retain their
// separate complete coverage check when no typed representative was emitted.
fn check_source_physical_backing_coverage_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &[ScopedSourceSlotV29],
    seen: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let result = (|| {
        plan.charge(2, budget)?;
        if seen.len() != plan.cells.rows.len() { return Err(scoped_object_allocation_error_v29()); }
        let mut previous = None;
        for mapping in &plan.cells.physical_backings {
            plan.charge(8, budget)?;
            if previous.is_some_and(|key| key >= mapping.key) {
                return Err(scoped_object_allocation_error_v29());
            }
            previous = Some(mapping.key);
            let logical = plan.cells.rows.get(mapping.cell).ok_or_else(scoped_object_allocation_error_v29)?;
            if mapping.key != (logical.instance.index(), logical.local.index(), logical.generation) {
                return Err(scoped_object_allocation_error_v29());
            }
            let (representative, physical, joined) = plan.physical_object_cell(mapping.cell, budget)?;
            if !joined || representative != mapping.representative { return Err(scoped_object_allocation_error_v29()); }
            let representative_seen = *seen.get(representative).ok_or_else(scoped_object_allocation_error_v29)?;
            if !representative_seen {
                if seen[mapping.cell] { return Err(scoped_object_allocation_error_v29()); }
                continue;
            }
            if mapping.cell != representative && seen[mapping.cell] {
                return Err(scoped_object_allocation_error_v29());
            }
            plan.charge(call_splice_search_work_v1(slots.len()), budget)?;
            let key = (physical.instance.index(), ScopedAllocationIdentityV29::OriginalObject {
                local: physical.local.index(), generation: physical.generation,
            });
            let slot = slots.binary_search_by_key(&key, |slot| (slot.instance.index(), slot.origin.identity))
                .ok().and_then(|index| slots.get(index)).ok_or_else(scoped_object_allocation_error_v29)?;
            let SourceBackingKindV29::Object(schema) = physical.kind else { return Err(scoped_object_allocation_error_v29()); };
            if slot.origin.semantic_type != logical.ty
                || slot.origin.source != (ScopedAllocationSourceV29::OriginalObject { cell: representative, schema })
                || !matches!(slot.representation, ScopedSlotRepresentationV29::Object { schema: actual, .. } if actual == schema)
            { return Err(scoped_object_allocation_error_v29()); }
            seen[mapping.cell] = true;
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
