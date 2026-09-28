// Representation facts only. Original generations and lifetime checks stay distinct.
#[derive(Clone, Copy)]
struct SourceBackingAllocationV29 {
    cell: usize,
    array: Option<fe2o3_kernel_ir::StorageLayoutIdV1>,
}

fn intersect_source_array_generations_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    backing: &mut BTreeMap<(usize, u32, u32), SourceBackingAllocationV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let result = (|| {
        use std::ops::Bound::{Excluded, Unbounded};
        source_reference_owned_prepay_v29::<Option<(usize, u32, u32)>>(plan, budget)?;
        let mut previous = None;
        loop {
            charge_execution_cfg_lookup_v29(backing.len(), budget)?;
            let next = match previous {
                Some(key) => backing.range((Excluded(key), Unbounded)).next(),
                None => backing.iter().next(),
            };
            let Some((&(instance, local, _), first)) = next else {
                break;
            };
            let schema = first.array;
            let mut compatible = schema.is_some();
            let range = (instance, local, 0)..=(instance, local, u32::MAX);
            charge_execution_cfg_lookup_v29(backing.len(), budget)?;
            for row in backing.range(range.clone()).map(|(_, row)| row) {
                budget.charge_work(2)?;
                compatible &= row.array == schema;
            }
            if !compatible {
                charge_execution_cfg_lookup_v29(backing.len(), budget)?;
                for row in backing.range_mut(range).map(|(_, row)| row) {
                    budget.charge_work(1)?;
                    row.array = None;
                }
            }
            previous = Some((instance, local, u32::MAX));
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_array_scalar_row_v29(
    layouts: &source_storage_v29::SourceStorageLayoutsV29<'_>,
    owner: &ProductionSemanticSsaOwnerV1,
    mut ty: SemanticTypeIdV1,
    mut schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    scalar: ScalarType,
    rows: &[fe2o3_kernel_ir::StorageLayoutV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::StorageLayoutKindV1 as Kind;
    for _ in 0..MAX_SSA_VALUE_COMPONENTS_V1 {
        budget.charge_work(8)?;
        layouts.check_selected_schema(owner, ty, schema, budget)?;
        let declaration = owner
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        let row = rows
            .get(schema.0 as usize)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        if declaration.layout().size_bytes() != Some(row.size)
            || declaration.layout().alignment_bytes() != u64::from(row.alignment)
        {
            return Err(scoped_object_allocation_error_v29());
        }
        match (declaration.shape(), &row.kind) {
            (
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_),
                Kind::Scalar(actual),
            ) => return Ok(*actual == scalar),
            (SemanticTypeShapeV1::Aggregate(original), Kind::Record(fields)) => {
                let SemanticTypeLayoutDetailsV1::Aggregate(original_layout) =
                    declaration.layout().details()
                else {
                    return Err(scoped_object_allocation_error_v29());
                };
                if original.fields().len() != 1
                    || fields.len() != 1
                    || fields[0].offset != 0
                    || original_layout.field_offsets() != [0]
                    || !original_layout.padding().is_empty()
                {
                    return Err(scoped_object_allocation_error_v29());
                }
                ty = original.fields()[0];
                schema = fields[0].layout;
            }
            _ => return Err(scoped_object_allocation_error_v29()),
        }
    }
    Err(scoped_object_allocation_error_v29())
}

fn source_array_cell_facts_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    cell: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    Option<(
        fe2o3_kernel_ir::StorageLayoutIdV1,
        PrivateRetainedArrayFactsV1,
    )>,
    ProductionSemanticKirErrorV1,
> {
    use fe2o3_kernel_ir::StorageLayoutKindV1 as Kind;
    plan.check_owner(plan.instances, budget)?;
    source_reference_owned_prepay_v29::<
        Option<(
            fe2o3_kernel_ir::StorageLayoutIdV1,
            PrivateRetainedArrayFactsV1,
        )>,
    >(plan, budget)?;
    plan.charge(12, budget)?;
    let row = plan
        .cells
        .rows
        .get(cell)
        .ok_or_else(scoped_object_allocation_error_v29)?;
    let original = plan
        .instances
        .instance(row.instance)
        .and_then(|instance| {
            instance
                .declaration()
                .locals()
                .get(row.local.index() as usize)
        })
        .ok_or_else(scoped_object_allocation_error_v29)?;
    if plan.instances.instance_reachable(row.instance) != Some(true) || original.ty() != row.ty {
        return Err(scoped_object_allocation_error_v29());
    }
    let SourceBackingKindV29::Object(schema) = row.kind else {
        return Ok(None);
    };
    if original.role().is_entry_argument() {
        return Ok(None);
    }
    let owner = plan.instances.owner();
    let types = owner.source_semantic().types();
    let Some(facts) = private_retained_array_facts_with_representation_v29(
        types,
        row.ty,
        usize::MAX,
        ExecutionCfgRepresentationV29::OriginalSource,
        budget,
    )?
    else {
        return Ok(None);
    };
    if !plan.has_storage_demands || plan.storage_demands.is_none() {
        return Err(scoped_object_allocation_error_v29());
    }
    let layouts = plan
        .storage_root
        .as_ref()
        .ok_or_else(scoped_object_allocation_error_v29)?
        .source_layouts(plan.instances, budget)?;
    layouts.check_selected_schema(owner, row.ty, schema, budget)?;
    if layouts.original_schema(owner, row.ty, budget)? != Some(schema) {
        return Ok(None);
    }
    let rows = layouts.rows(owner, budget)?;
    plan.charge(14, budget)?;
    let selected = rows
        .get(schema.0 as usize)
        .ok_or_else(scoped_object_allocation_error_v29)?;
    let Kind::Array {
        element,
        length,
        stride,
    } = selected.kind
    else {
        return Err(scoped_object_allocation_error_v29());
    };
    if length != facts.length
        || stride != facts.element.size
        || selected.size
            != stride
                .checked_mul(length)
                .ok_or(ArgumentResourceV1::Arithmetic)?
        || selected.alignment != facts.element.alignment
        || layouts.original_schema(owner, facts.element_type, budget)? != Some(element)
    {
        return Err(scoped_object_allocation_error_v29());
    }
    layouts.check_selected_schema(owner, facts.element_type, element, budget)?;
    let child = rows
        .get(element.0 as usize)
        .ok_or_else(scoped_object_allocation_error_v29)?;
    if child.size != facts.element.size || child.alignment != facts.element.alignment {
        return Err(scoped_object_allocation_error_v29());
    }
    let exact = match (facts.element.element, &child.kind) {
        (PrivateRetainedElementFactsV1::Scalar(expected), Kind::Scalar(actual)) => {
            expected == *actual
        }
        (
            PrivateRetainedElementFactsV1::ThinPointer {
                element: expected,
                space,
                access,
            },
            Kind::Pointer(pointer),
        ) => {
            plan.charge(8, budget)?;
            let SemanticTypeShapeV1::Pointer(original_pointer) =
                types[facts.element_type.index() as usize].shape()
            else {
                return Err(scoped_object_allocation_error_v29());
            };
            pointer.value_space == space
                && pointer.encoded_space == space
                && pointer.access == access
                && u64::from(pointer.stored_bits) == facts.element.size * 8
                && source_array_scalar_row_v29(
                    layouts,
                    owner,
                    original_pointer.pointee(),
                    pointer.pointee,
                    expected,
                    &rows,
                    budget,
                )?
        }
        _ => false,
    };
    if !exact {
        return Err(scoped_object_allocation_error_v29());
    }
    Ok(Some((schema, facts)))
}

fn source_array_eligibility_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        source_reference_owned_prepay_v29::<Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>>(
            plan, budget,
        )?;
        let mut eligible = emission_vec_v1(plan.cells.rows.len(), budget)?;
        for cell in 0..plan.cells.rows.len() {
            eligible.push(source_array_cell_facts_v29(plan, cell, budget)?.map(|row| row.0));
        }
        plan.charge(2, budget)?;
        if plan.cells.strategies.len() != plan.loans.len()
            || plan.cells.raw_origins.len() != plan.raw_origins.len()
        {
            return Err(scoped_object_allocation_error_v29());
        }
        for (loan, strategy) in plan.cells.strategies.iter().enumerate() {
            plan.charge(7, budget)?;
            if let SourceReferenceCellStrategyV29::Object(cell) = *strategy {
                let origin = plan
                    .loans
                    .get(loan)
                    .and_then(|loan| plan.origins.get(loan.origin))
                    .ok_or_else(scoped_object_allocation_error_v29)?;
                let row = plan
                    .cells
                    .rows
                    .get(cell)
                    .ok_or_else(scoped_object_allocation_error_v29)?;
                if (row.instance, row.local, row.generation)
                    != (origin.instance, origin.local, origin.generation)
                    || !matches!(row.kind, SourceBackingKindV29::Object(_))
                {
                    return Err(scoped_object_allocation_error_v29());
                }
                *eligible
                    .get_mut(cell)
                    .ok_or_else(scoped_object_allocation_error_v29)? = None;
            }
        }
        for (origin, &cell) in plan.raw_origins.iter().zip(&plan.cells.raw_origins) {
            plan.charge(5, budget)?;
            let row = plan
                .cells
                .rows
                .get(cell)
                .ok_or_else(scoped_object_allocation_error_v29)?;
            if (row.instance, row.local, row.generation)
                != (origin.instance, origin.local, origin.generation)
            {
                return Err(scoped_object_allocation_error_v29());
            }
            *eligible
                .get_mut(cell)
                .ok_or_else(scoped_object_allocation_error_v29)? = None;
        }
        Ok(eligible)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SourceFunctionBackingViewV29<'_> {
    fn array_schema(
        self,
        local: u32,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<fe2o3_kernel_ir::StorageLayoutIdV1>, ProductionSemanticKirErrorV1> {
        self.layouts.check(budget)?;
        source_reference_owned_prepay_v29::<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>(
            self.layouts.references,
            budget,
        )?;
        charge_execution_cfg_lookup_v29(self.layouts.backing.len(), budget)?;
        let range = (self.instance.index(), local, 0)..=(self.instance.index(), local, u32::MAX);
        let mut schema = None;
        let mut first = true;
        for (&(_, _, generation), &row) in self.layouts.backing.range(range) {
            budget.source_reference_charge_v29(self.layouts.references, 4)?;
            let (cell, original) = self
                .cell(SemanticLocalIdV1::from_index(local), generation, budget)?
                .ok_or_else(scoped_object_allocation_error_v29)?;
            if cell != row.cell
                || row
                    .array
                    .is_some_and(|id| original.kind != SourceBackingKindV29::Object(id))
                || (!first && schema != row.array)
            {
                return Err(scoped_object_allocation_error_v29());
            }
            first = false;
            schema = row.array;
        }
        Ok(schema)
    }
}

fn source_array_coverage_counts_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    length: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<usize>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        source_reference_owned_prepay_v29::<Vec<usize>>(plan, budget)?;
        let mut counts = emission_vec_v1(length, budget)?;
        budget.charge_work(length)?;
        counts.resize(length, 0_usize);
        Ok(counts)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn publish_source_array_cell_coverage_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &[ScopedSourceSlotV29],
    counts: &[usize],
    seen: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        plan.charge(4, budget)?;
        if slots.len() != counts.len() || plan.cells.rows.len() != seen.len() {
            return Err(scoped_object_allocation_error_v29());
        }
        for (slot, &count) in slots.iter().zip(counts) {
            budget.charge_work(2)?;
            if matches!(
                slot.origin.source,
                ScopedAllocationSourceV29::OriginalArray { .. }
            ) != (count != 0)
            {
                return Err(scoped_object_allocation_error_v29());
            }
        }
        // This is publication of the preceding complete cell/slot join only.
        // It cannot add initialization, generation equality or currentness facts.
        budget.charge_work(plan.cells.rows.len())?;
        for (cell, row) in plan.cells.rows.iter().enumerate() {
            if matches!(row.kind, SourceBackingKindV29::Object(_)) {
                seen[cell] = true;
            }
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn check_source_array_cell_slot_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    cell: usize,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    slot: ScopedSourceSlotV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let row = plan
        .cells
        .rows
        .get(cell)
        .ok_or_else(scoped_object_allocation_error_v29)?;
    plan.charge(6, budget)?;
    if row.kind != SourceBackingKindV29::Object(schema)
        || slot.instance != row.instance
        || slot.origin.identity != ScopedAllocationIdentityV29::LegacyLocal(row.local.index())
        || slot.origin.semantic_type != row.ty
        || slot.origin.source != (ScopedAllocationSourceV29::OriginalArray { schema })
    {
        return Err(scoped_object_allocation_error_v29());
    }
    let (checked, facts) = source_array_cell_facts_v29(plan, cell, budget)?
        .ok_or_else(scoped_object_allocation_error_v29)?;
    let physical = slot.scalar_array()?;
    plan.charge(8, budget)?;
    if checked != schema
        || physical.element_type != facts.element_type
        || physical.element != facts.element
        || physical.length != facts.length
        || physical.bytes
            != facts
                .element
                .size
                .checked_mul(facts.length)
                .ok_or(ArgumentResourceV1::Arithmetic)?
        || physical.count.is_none()
    {
        return Err(scoped_object_allocation_error_v29());
    }
    Ok(())
}

// A representation locator, not a cell-pointer proof. The caller must still
// check the actual allocation and complete the private-array memory census.
fn source_array_cell_slot_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    cell: usize,
    eligible: Option<fe2o3_kernel_ir::StorageLayoutIdV1>,
    slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let row = plan
        .cells
        .rows
        .get(cell)
        .ok_or_else(scoped_object_allocation_error_v29)?;
    charge_execution_cfg_lookup_v29(slots.len(), budget)?;
    let key = (
        row.instance.index(),
        ScopedAllocationIdentityV29::LegacyLocal(row.local.index()),
    );
    let Ok(index) =
        slots.binary_search_by_key(&key, |slot| (slot.instance.index(), slot.origin.identity))
    else {
        return Ok(None);
    };
    let slot = slots[index];
    let ScopedAllocationSourceV29::OriginalArray { schema } = slot.origin.source else {
        return Ok(None);
    };
    if eligible != Some(schema) {
        return Err(scoped_object_allocation_error_v29());
    }
    check_source_array_cell_slot_v29(plan, cell, schema, slot, budget)?;
    charge_execution_cfg_lookup_v29(slots.len(), budget)?;
    let object = (
        row.instance.index(),
        ScopedAllocationIdentityV29::OriginalObject {
            local: row.local.index(),
            generation: row.generation,
        },
    );
    if slots
        .binary_search_by_key(&object, |slot| {
            (slot.instance.index(), slot.origin.identity)
        })
        .is_ok()
    {
        return Err(scoped_object_allocation_error_v29());
    }
    Ok(Some(index))
}

fn check_source_array_cell_coverage_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &[ScopedSourceSlotV29],
    seen: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let scratch = budget.storage();
    let result = (|| {
        let eligible = source_array_eligibility_v29(plan, budget)?;
        let mut counts = source_array_coverage_counts_v29(plan, slots.len(), budget)?;
        let mut previous = None;
        for slot in slots {
            budget.charge_work(3)?;
            let key = (slot.instance.index(), slot.origin.identity);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(scoped_object_allocation_error_v29());
            }
            previous = Some(key);
        }
        if seen.len() != plan.cells.rows.len() {
            return Err(scoped_object_allocation_error_v29());
        }
        for (cell, row) in plan.cells.rows.iter().enumerate() {
            plan.charge(4, budget)?;
            let SourceBackingKindV29::Object(schema) = row.kind else {
                if seen[cell] {
                    return Err(scoped_object_allocation_error_v29());
                }
                continue;
            };
            charge_execution_cfg_lookup_v29(slots.len(), budget)?;
            let key = (
                row.instance.index(),
                ScopedAllocationIdentityV29::LegacyLocal(row.local.index()),
            );
            let found = slots
                .binary_search_by_key(&key, |slot| (slot.instance.index(), slot.origin.identity))
                .ok();
            if seen[cell] {
                if found.is_some() {
                    return Err(scoped_object_allocation_error_v29());
                }
                continue;
            }
            let index = found.ok_or_else(scoped_object_allocation_error_v29)?;
            let slot = &slots[index];
            if eligible[cell] != Some(schema)
                || slot.origin.semantic_type != row.ty
                || slot.origin.source != (ScopedAllocationSourceV29::OriginalArray { schema })
            {
                return Err(scoped_object_allocation_error_v29());
            }
            check_source_array_cell_slot_v29(plan, cell, schema, *slot, budget)?;
            counts[index] = counts[index]
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        publish_source_array_cell_coverage_v29(plan, slots, &counts, seen, budget)
    })();
    let cleanup = budget
        .storage()
        .checked_sub(scratch)
        .ok_or(ArgumentResourceV1::Accounting)
        .and_then(|bytes| budget.release_storage(bytes))
        .map_err(ProductionSemanticKirErrorV1::from);
    result
        .and(cleanup)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
