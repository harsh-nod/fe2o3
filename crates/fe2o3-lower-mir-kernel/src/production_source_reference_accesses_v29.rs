// Exact source occurrences and normalized targets retained by the common owner.
// Source addresses are private locators inside that immutable owner, not permits.
include!("production_source_reference_raw_scalar_epochs_v29.rs");
include!("production_source_enum_checked_read_v58.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceAccessKeyV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    access: SourceReferenceAccessV29,
}

struct SourceReferenceAccessRecordV29 {
    key: SourceReferenceAccessKeyV29,
    source_local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    projections: std::ops::Range<usize>,
    loan: Option<usize>,
    traversed: std::ops::Range<usize>,
    shared_path: bool,
    checked_enum_read: Option<source_enum_checked_read_v58::CheckedRead>,
}

type SourceReferenceAccessIndexKeyV29 = (usize, u32, Option<usize>, usize, u8);

fn reserve_source_reference_access_index_v29(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(count, budget)?;
    // Pay the unchanged split-path allowance and one exact owned key, without
    // reserving a full source occurrence in every unused BTree node slot.
    budget.reserve_storage(argument_sum_v1(&[
        execution_cfg_map_entry_storage_v29::<Box<SourceReferenceAccessIndexKeyV29>, usize>(count)?,
        std::mem::size_of::<SourceReferenceAccessIndexKeyV29>(),
    ])?)
}

fn source_reference_access_key_v29(
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
) -> SourceReferenceAccessIndexKeyV29 {
    let kind = match access {
        SourceReferenceAccessV29::Read => 0,
        SourceReferenceAccessV29::Write => 1,
        SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared) => 2,
        SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable) => 3,
        SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Fake) => 4,
        SourceReferenceAccessV29::Address => 5,
        SourceReferenceAccessV29::ReadDiscriminant => 6,
    };
    (
        site.instance.index(),
        site.block.index(),
        site.statement,
        place as *const SemanticPlaceV1 as usize,
        kind,
    )
}

// A direct scalar occurrence can retain its finite may-activation set. This
// never merges a pointer, loan, projection, or initialized-value certificate.
fn source_reference_direct_scalar_epoch_access_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    resolved: &SourceReferencePlaceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    if !matches!(
        access,
        SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
    ) || resolved.instance != site.instance
        || resolved.local != source.local()
        || !source.projections().is_empty()
        || !resolved.projections.is_empty()
        || resolved.loan.is_some()
        || resolved.shared_path
        || !resolved.traversed.is_empty()
    {
        return Ok(false);
    }
    let original = plan
        .instances
        .instance(site.instance)
        .and_then(|row| {
            row.declaration()
                .locals()
                .get(source.local().index() as usize)
        })
        .ok_or_else(source_reference_cfg_obligation_v29)?;
    if original.ty() != source.ty() {
        return Ok(false);
    }
    let declaration = plan
        .instances
        .owner()
        .source_semantic()
        .types()
        .get(original.ty().index() as usize)
        .ok_or_else(source_reference_cfg_obligation_v29)?;
    Ok(matches!(
        declaration.shape(),
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
    ))
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_reference_access(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        resolved: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.storage == SourceReferenceStorageV29::PromotedOnly {
            return Ok(());
        }
        if resolved.loan.is_none() && !matches!(access, SourceReferenceAccessV29::Borrow(_)) {
            if !self.plan.has_storage_demands {
                return Ok(());
            }
            self.plan.check_owner(self.plan.instances, budget)?;
            let requests = self
                .storage_requests
                .ok_or(ArgumentResourceV1::Accounting)?;
            let (original_requests, _) = self
                .plan
                .storage_demands
                .ok_or(ArgumentResourceV1::Accounting)?
                .requests(self.plan.instances, budget)?;
            budget.charge_work(1)?;
            if !std::ptr::eq(requests, original_requests) {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if !source_backing_original_request_v29(
                requests,
                resolved.instance,
                resolved.local,
                budget,
            )? {
                return Ok(());
            }
            budget.charge_work(3)?;
            let original = self
                .plan
                .instances
                .instance(resolved.instance)
                .and_then(|row| {
                    row.declaration()
                        .locals()
                        .get(resolved.local.index() as usize)
                })
                .ok_or_else(execution_call_error_v29)?;
            let declaration = self
                .plan
                .instances
                .owner()
                .source_semantic()
                .types()
                .get(original.ty().index() as usize)
                .ok_or_else(execution_call_error_v29)?;
            // An original object access locator, not a selected representation
            // or an initialized/current-memory certificate.
            let thin_raw = matches!(declaration.shape(), SemanticTypeShapeV1::Pointer(pointer)
                if pointer.kind() == SemanticPointerKindV1::Raw
                    && pointer.metadata() == SemanticPointerMetadataV1::None);
            if !thin_raw
                && !matches!(
                    declaration.shape(),
                    SemanticTypeShapeV1::Tuple(_)
                        | SemanticTypeShapeV1::Aggregate(_)
                        | SemanticTypeShapeV1::Union(_)
                        | SemanticTypeShapeV1::Array { .. }
                        | SemanticTypeShapeV1::Enum { .. }
                        | SemanticTypeShapeV1::Scalar(_)
                        | SemanticTypeShapeV1::ValidityScalar(_)
                )
            {
                return Ok(());
            }
        }
        let checked_enum_read =
            source_enum_checked_read_v58::check(self, site, source, access, resolved, budget)?;
        let key = source_reference_access_key_v29(site, source, access);
        charge_execution_cfg_lookup_v29(self.plan.access_sites.len(), budget)?;
        if let Some(&index) = self.plan.access_sites.get(&key) {
            let row = &self.plan.accesses[index];
            budget.charge_work(argument_sum_v1(&[
                12,
                resolved.projections.len(),
                resolved.traversed.len(),
            ])?)?;
            if row.source_local != source.local()
                || row.ty != source.ty()
                || row.instance != resolved.instance
                || row.local != resolved.local
                || row.loan != resolved.loan
                || row.shared_path != resolved.shared_path
                || self.plan.projections[row.projections.clone()] != resolved.projections
                || self.plan.access_loans[row.traversed.clone()] != resolved.traversed
            {
                return Err(source_reference_cfg_obligation_v29());
            }
            if row.generation != resolved.generation {
                let generation = row.generation;
                if self.storage_root.is_none()
                    || !(source_reference_direct_scalar_epoch_access_v29(
                        &self.plan, site, source, access, resolved, budget,
                    )? || source_reference_raw_scalar_epoch_access_v29(
                        &self.plan, site, source, access, resolved, budget,
                    )?)
                {
                    return Err(source_reference_cfg_obligation_v29());
                }
                // Revisited CFG observations are not final authority. Join only
                // original activation labels through the same canonical set
                // solver as C2; final source/effect/currentness replay remains
                // responsible for each actual access and storage restart.
                let generation = self.join_storage_epochs(
                    resolved.instance,
                    resolved.local,
                    generation,
                    resolved.generation,
                    budget,
                )?;
                self.plan.accesses[index].generation = generation;
            }
            self.plan.accesses[index].checked_enum_read = self.plan.checked_enum_reads.intersect(
                self.plan.accesses[index].checked_enum_read,
                checked_enum_read,
                budget,
            )?;
            return Ok(());
        }
        reserve_source_reference_access_index_v29(self.plan.access_sites.len(), budget)?;
        scoped_object_reserve_additional_v29(
            &mut self.plan.projections,
            resolved.projections.len(),
            budget,
        )?;
        scoped_object_reserve_additional_v29(
            &mut self.plan.access_loans,
            resolved.traversed.len(),
            budget,
        )?;
        scoped_object_reserve_append_v29(&mut self.plan.accesses, budget)?;
        let checked_enum_read = self
            .plan
            .checked_enum_reads
            .append(checked_enum_read, budget)?;
        budget.charge_work(argument_sum_v1(&[
            1,
            resolved.projections.len(),
            resolved.traversed.len(),
        ])?)?;
        let std::collections::btree_map::Entry::Vacant(entry) =
            self.plan.access_sites.entry(Box::new(key))
        else {
            return Err(ArgumentResourceV1::Accounting.into());
        };
        let first = self.plan.projections.len();
        let first_loan = self.plan.access_loans.len();
        self.plan
            .projections
            .extend_from_slice(&resolved.projections);
        self.plan
            .access_loans
            .extend_from_slice(&resolved.traversed);
        let index = self.plan.accesses.len();
        self.plan.accesses.push(SourceReferenceAccessRecordV29 {
            key: SourceReferenceAccessKeyV29 {
                site,
                source: source as *const SemanticPlaceV1 as usize,
                access,
            },
            source_local: source.local(),
            ty: source.ty(),
            instance: resolved.instance,
            local: resolved.local,
            generation: resolved.generation,
            projections: first..self.plan.projections.len(),
            loan: resolved.loan,
            traversed: first_loan..self.plan.access_loans.len(),
            shared_path: resolved.shared_path,
            checked_enum_read,
        });
        entry.insert(index);
        Ok(())
    }

    fn retain_reference_return(
        &mut self,
        site: SourceReferenceSiteV29,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.storage == SourceReferenceStorageV29::PromotedOnly {
            if self.contains_loan(node, budget)? {
                return Err(source_reference_error_v29(
                    "source reference return escape requires lifetime transport",
                ));
            }
            return Ok(());
        }
        for loan in self.node_loans(node, budget)? {
            self.check_loan_use(loan, budget)?;
            let origin = &self.plan.origins[self.plan.loans[loan].origin];
            if origin.instance == site.instance {
                return Err(source_reference_error_v29(
                    "source reference return escapes its referent call frame",
                ));
            }
            let local = self.local(origin.instance, origin.local)?;
            if local.generation != origin.generation || local.node.is_none() {
                return Err(source_reference_error_v29(
                    "source reference return has no live ancestor referent",
                ));
            }
            let mut current = site.instance;
            let mut ancestor = false;
            for _ in 0..self.plan.instances.instances().len() {
                budget.charge_work(3)?;
                let Some(incoming) = self.plan.instances.incoming(current) else {
                    break;
                };
                let parent = incoming.occurrence().caller;
                if parent.index() >= current.index() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                if parent == origin.instance {
                    ancestor = true;
                    break;
                }
                current = parent;
            }
            if !ancestor {
                return Err(source_reference_error_v29(
                    "source reference return origin is not an ancestor frame",
                ));
            }
        }
        let previous = *self
            .plan
            .returns
            .get(site.instance.index())
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        let joined = match previous {
            Some(previous) => self.merge_node(previous, node, budget)?,
            None => node,
        };
        self.plan.returns[site.instance.index()] = Some(joined);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SourceReferencePointerDefinitionV29<'a> {
    Parameter(usize),
    BlockParameter(&'a BasicBlock, usize),
    Operation(&'a Operation, usize),
}

type SourceReferencePointerEntryV29<'a> = (&'a Type, SourceReferencePointerDefinitionV29<'a>);
type SourceReferencePointerRowV29<'a> = (
    SourceReferencePointerKeyV29,
    SourceReferencePointerEntryV29<'a>,
);

struct SourceReferencePointerIndexV29<'a> {
    definitions: Vec<SourceReferencePointerRowV29<'a>>,
    functions: BTreeMap<&'a FunctionId, usize>,
    name_width: usize,
    calls: Vec<Option<(usize, BlockId, usize, &'a Operation)>>,
    seeds: BTreeMap<(usize, ValueId), (u32, SemanticTypeIdV1)>,
}

fn source_reference_insert_pointer_definition_v29<'a>(
    definitions: &mut Vec<SourceReferencePointerRowV29<'a>>,
    key: SourceReferencePointerKeyV29,
    entry: SourceReferencePointerEntryV29<'a>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if definitions.len() == definitions.capacity() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    definitions.push((key, entry));
    Ok(())
}

fn source_reference_finish_pointer_definitions_v29(
    definitions: &mut [SourceReferencePointerRowV29<'_>],
    expected: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if definitions.len() != expected {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    call_splice_sort_work_v1(argument_product_v1(definitions.len(), 2)?, budget)
        .map_err(source_address_call_error_v29)?;
    budget.charge_work(argument_product_v1(definitions.len(), 2)?)?;
    // The complete census is prepaid before insertion. Sort in place, then
    // reject duplicate coordinates before this immutable index can escape.
    definitions.sort_unstable_by_key(|row| row.0);
    if definitions.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err(execution_call_error_v29());
    }
    Ok(())
}

fn source_reference_pointer_definition_lookup_v29<'index, 'kir>(
    definitions: &'index [SourceReferencePointerRowV29<'kir>],
    key: SourceReferencePointerKeyV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<&'index SourceReferencePointerEntryV29<'kir>>, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(definitions.len(), budget)?;
    Ok(definitions
        .binary_search_by_key(&key, |row| row.0)
        .ok()
        .map(|ordinal| &definitions[ordinal].1))
}

fn source_reference_pointer_definition_v29<'a>(
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &mut SourceReferencePointerIndexV29<'a>,
    instance: usize,
    value: ValueId,
    ty: &'a Type,
    definition: SourceReferencePointerDefinitionV29<'a>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_charge_v29(plan, 1)?;
    if !matches!(ty, Type::Pointer(_)) {
        return Ok(());
    }
    source_reference_insert_pointer_definition_v29(
        &mut index.definitions,
        (instance, value),
        (ty, definition),
        budget,
    )
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_active_emitted_v29<'kir>(
    plan: &SourceReferencePlanV29<'_, '_>,
    original: usize,
    lowered: &'kir Option<LoweredFunctionResultV1>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<&'kir LoweredFunctionResultV1>, ProductionSemanticKirErrorV1> {
    budget.source_reference_charge_v29(plan, 4)?;
    let instance = plan
        .instances
        .id_at(original)
        .ok_or_else(execution_call_error_v29)?;
    match (
        plan.instances.instance_reachable(instance),
        lowered.as_ref(),
    ) {
        (Some(false), None) => Ok(None),
        (Some(true), Some(lowered)) if lowered.source_call_instance == Some(instance) => {
            Ok(Some(lowered))
        }
        _ => Err(execution_call_error_v29()),
    }
}

fn source_reference_pointer_definition_count_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut count = 0;
    for (instance, lowered) in emitted.iter().enumerate() {
        let Some(lowered) = source_reference_active_emitted_v29(plan, instance, lowered, budget)?
        else {
            continue;
        };
        let body = lowered
            .function
            .body
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        budget.source_reference_charge_v29(plan, 1)?;
        if body.parameters.len() != lowered.function.signature.parameters.len() {
            return Err(execution_call_error_v29());
        }
        for ty in &lowered.function.signature.parameters {
            budget.source_reference_charge_v29(plan, 2)?;
            if matches!(ty, Type::Pointer(_)) {
                count = argument_sum_v1(&[count, 1])?;
            }
        }
        for block in &body.blocks {
            budget.source_reference_charge_v29(plan, 1)?;
            for parameter in &block.parameters {
                budget.source_reference_charge_v29(plan, 2)?;
                if matches!(parameter.ty, Type::Pointer(_)) {
                    count = argument_sum_v1(&[count, 1])?;
                }
            }
            for operation in &block.operations {
                budget.source_reference_charge_v29(plan, 1)?;
                for result in &operation.results {
                    budget.source_reference_charge_v29(plan, 2)?;
                    if matches!(result.ty, Type::Pointer(_)) {
                        count = argument_sum_v1(&[count, 1])?;
                    }
                }
            }
        }
    }
    Ok(count)
}

fn source_reference_pointer_index_v29<'a>(
    references: &SourceReferenceEmissionV29<'_, '_>,
    emitted: &'a [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferencePointerIndexV29<'a>, ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    references.check(budget)?;
    budget.source_reference_charge_v29(plan, 1)?;
    if emitted.len() != plan.instances.instances().len() {
        return Err(execution_call_error_v29());
    }
    let definition_count = source_reference_pointer_definition_count_v29(plan, emitted, budget)?;
    source_reference_owned_prepay_v29::<SourceReferencePointerIndexV29<'_>>(plan, budget)?;
    let definitions = source_reference_owned_vec_v29(plan, definition_count, budget)?;
    let mut calls = source_reference_owned_vec_v29(plan, emitted.len(), budget)?;
    budget.source_reference_charge_v29(plan, emitted.len())?;
    calls.resize(emitted.len(), None);
    let mut index = SourceReferencePointerIndexV29 {
        definitions,
        functions: BTreeMap::new(),
        name_width: 0,
        calls,
        seeds: BTreeMap::new(),
    };
    for (instance, lowered) in emitted.iter().enumerate() {
        budget.source_reference_charge_v29(plan, 3)?;
        let Some(lowered) = source_reference_active_emitted_v29(plan, instance, lowered, budget)?
        else {
            continue;
        };
        if lowered.source_call_instance.map(|id| id.index()) != Some(instance) {
            return Err(execution_call_error_v29());
        }
        index.name_width = index.name_width.max(lowered.function.id.as_str().len());
        budget.source_reference_charge_v29(
            plan,
            argument_product_v1(
                argument_sum_v1(&[index.name_width, lowered.function.id.as_str().len()])?,
                call_splice_search_work_v1(index.functions.len()),
            )?,
        )?;
        reserve_execution_cfg_map_entry_v29::<&FunctionId, usize>(index.functions.len(), budget)
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
        if index
            .functions
            .insert(&lowered.function.id, instance)
            .is_some()
        {
            return Err(execution_call_error_v29());
        }
    }
    for slot in &slots.slots {
        budget.source_reference_charge_v29(plan, 5)?;
        let local = match (
            slot.origin.identity,
            slot.origin.source,
            slot.representation,
        ) {
            (
                ScopedAllocationIdentityV29::LegacyLocal(local),
                ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. },
                ScopedSlotRepresentationV29::ScalarArray(_),
            ) => {
                slot.scalar_array()?;
                local
            }
            (
                ScopedAllocationIdentityV29::OriginalObject { .. },
                ScopedAllocationSourceV29::OriginalObject { schema, .. },
                ScopedSlotRepresentationV29::Object { schema: actual, .. },
            ) if schema == actual => continue,
            _ => return Err(scoped_object_allocation_error_v29()),
        };
        reserve_execution_cfg_map_entry_v29::<(usize, ValueId), (u32, SemanticTypeIdV1)>(
            index.seeds.len(),
            budget,
        )
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
        if index
            .seeds
            .insert(
                (slot.instance.index(), slot.origin.pointer),
                (local, slot.origin.semantic_type),
            )
            .is_some()
        {
            return Err(execution_call_error_v29());
        }
    }
    for (instance, lowered) in emitted.iter().enumerate() {
        let Some(lowered) = source_reference_active_emitted_v29(plan, instance, lowered, budget)?
        else {
            continue;
        };
        let body = lowered
            .function
            .body
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        if body.parameters.len() != lowered.function.signature.parameters.len() {
            return Err(execution_call_error_v29());
        }
        for (ordinal, (&value, ty)) in body
            .parameters
            .iter()
            .zip(&lowered.function.signature.parameters)
            .enumerate()
        {
            source_reference_pointer_definition_v29(
                plan,
                &mut index,
                instance,
                value,
                ty,
                SourceReferencePointerDefinitionV29::Parameter(ordinal),
                budget,
            )?;
        }
        for block in &body.blocks {
            budget.source_reference_charge_v29(plan, 1)?;
            for (ordinal, value) in block.parameters.iter().enumerate() {
                source_reference_pointer_definition_v29(
                    plan,
                    &mut index,
                    instance,
                    value.id,
                    &value.ty,
                    SourceReferencePointerDefinitionV29::BlockParameter(block, ordinal),
                    budget,
                )?;
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                budget.source_reference_charge_v29(plan, 1)?;
                for (result, value) in operation.results.iter().enumerate() {
                    source_reference_pointer_definition_v29(
                        plan,
                        &mut index,
                        instance,
                        value.id,
                        &value.ty,
                        SourceReferencePointerDefinitionV29::Operation(operation, result),
                        budget,
                    )?;
                }
                if let OperationKind::Call { callee, .. } = &operation.kind {
                    budget.source_reference_charge_v29(
                        plan,
                        argument_product_v1(
                            argument_sum_v1(&[index.name_width, callee.as_str().len()])?,
                            call_splice_search_work_v1(index.functions.len()),
                        )?,
                    )?;
                    charge_execution_cfg_lookup_v29(index.functions.len(), budget)
                        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
                    let Some(&child) = index.functions.get(callee) else {
                        continue;
                    };
                    let child_id = plan
                        .instances
                        .id_at(child)
                        .ok_or_else(execution_call_error_v29)?;
                    let incoming = plan
                        .instances
                        .incoming(child_id)
                        .ok_or_else(execution_call_error_v29)?;
                    let source = incoming.occurrence();
                    budget.source_reference_charge_v29(
                        plan,
                        argument_product_v1(lowered.terminator_operation_spans.len(), 5)?,
                    )?;
                    if source.caller.index() != instance
                        || !lowered.terminator_operation_spans.iter().any(|span| {
                            span.semantic_block == source.block
                                && span.kernel_ir_block == block.id
                                && ordinal >= span.first_operation_ordinal as usize
                                && ordinal
                                    < span.first_operation_ordinal as usize
                                        + span.operation_count as usize
                        })
                        || index.calls[child]
                            .replace((instance, block.id, ordinal, operation))
                            .is_some()
                    {
                        return Err(execution_call_error_v29());
                    }
                }
            }
        }
    }
    source_reference_finish_pointer_definitions_v29(
        &mut index.definitions,
        definition_count,
        budget,
    )
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    Ok(index)
}

fn source_reference_cell_edge_values_v29(
    terminator: &Terminator,
    target: BlockId,
    mut accept: impl FnMut(&[ValueId]) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    match terminator {
        Terminator::Branch {
            target: next,
            arguments,
        } => {
            if *next == target {
                accept(arguments)?;
            }
        }
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            if *then_target == target {
                accept(then_arguments)?;
            }
            if *else_target == target {
                accept(else_arguments)?;
            }
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for case in cases {
                if case.target == target {
                    accept(&case.arguments)?;
                }
            }
            if *default_target == target {
                accept(default_arguments)?;
            }
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for case in cases {
                if case.target == target {
                    accept(&case.arguments)?;
                }
            }
            if *default_target == target {
                accept(default_arguments)?;
            }
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}

type SourceReferencePointerKeyV29 = (usize, ValueId);
type SourceReferencePhysicalOriginV29 = origin_worklist_v1::OriginStateV1<Option<usize>>;
type SourceReferenceCellOperandKeyV29 = (usize, BlockId, Option<usize>, usize);

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceReferencePointerClaimsV29 {
    ScalarOnly,
    SelectedBacking,
}

include!("production_source_reference_pending_reads_v29.rs");

#[cfg(test)]
#[path = "production_source_reference_pointer_ordinals_v29_tests.rs"]
mod pointer_ordinals_tests_v29;

struct SourceReferenceCellPointerProofV29<'plan, 'source, 'kir> {
    plan: &'plan SourceReferencePlanV29<'plan, 'source>,
    index: SourceReferencePointerIndexV29<'kir>,
    ordinals: Vec<SourceReferencePointerKeyV29>,
    origins: Vec<SourceReferencePhysicalOriginV29>,
    cell_slots: Vec<Option<usize>>,
    entry_initializations: Vec<Option<PrivateArrayPhysicalLocationV1>>,
    operands: BTreeMap<SourceReferenceCellOperandKeyV29, usize>,
    selectors: Vec<SourceReferenceHistorySelectorV29>,
    floor: usize,
}

fn source_reference_origin_error_v29(
    error: origin_worklist_v1::OriginWorkErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        origin_worklist_v1::OriginWorkErrorV1::Resource(error) => error.into(),
        origin_worklist_v1::OriginWorkErrorV1::Shape => execution_call_error_v29(),
    }
}

fn source_reference_pointer_ordinals_v29(
    index: &SourceReferencePointerIndexV29<'_>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<SourceReferencePointerKeyV29>, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Vec<SourceReferencePointerKeyV29>>(budget)?;
    let mut ordinals = emission_vec_v1(index.definitions.len(), budget)?;
    budget.charge_work(index.definitions.len())?;
    // The immutable definition census supplies unique, sorted keys. Their positions
    // are the same ordinals used by the origin equations, without a second tree.
    ordinals.extend(index.definitions.iter().map(|row| row.0));
    Ok(ordinals)
}

fn source_reference_pointer_ordinal_v29(
    ordinals: &[SourceReferencePointerKeyV29],
    key: SourceReferencePointerKeyV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(ordinals.len(), budget)?;
    ordinals
        .binary_search(&key)
        .map_err(|_| execution_call_error_v29())
}

fn source_reference_pointer_dependencies_v29(
    index: &SourceReferencePointerIndexV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    key: SourceReferencePointerKeyV29,
    definition: SourceReferencePointerDefinitionV29<'_>,
    claims: SourceReferencePointerClaimsV29,
    budget: &mut ArgumentBudgetV1<'_>,
    mut visit: impl FnMut(
        SourceReferencePointerKeyV29,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let (instance, _) = key;
    budget.charge_work(2)?;
    match definition {
        SourceReferencePointerDefinitionV29::Parameter(ordinal) => {
            let Some((caller, _, _, call)) = index.calls.get(instance).copied().flatten() else {
                return Ok(false);
            };
            let OperationKind::Call { arguments, .. } = &call.kind else {
                return Err(execution_call_error_v29());
            };
            visit(
                (
                    caller,
                    *arguments
                        .get(ordinal)
                        .ok_or_else(execution_call_error_v29)?,
                ),
                budget,
            )?;
        }
        SourceReferencePointerDefinitionV29::Operation(operation, ordinal) => match &operation.kind
        {
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                ..
            } if operation.results.len() == 1 && ordinal == 0 => visit((instance, *value), budget)?,
            OperationKind::Select {
                true_value,
                false_value,
                ..
            } if claims == SourceReferencePointerClaimsV29::SelectedBacking
                && operation.results.len() == 1
                && ordinal == 0 =>
            {
                visit((instance, *true_value), budget)?;
                if false_value != true_value {
                    visit((instance, *false_value), budget)?;
                }
            }
            OperationKind::Call { callee, .. } => {
                budget.charge_work(argument_product_v1(
                    argument_sum_v1(&[index.name_width, callee.as_str().len()])?,
                    call_splice_search_work_v1(index.functions.len()),
                )?)?;
                charge_execution_cfg_lookup_v29(index.functions.len(), budget)?;
                let Some(&child) = index.functions.get(callee) else {
                    return Ok(false);
                };
                let body = emitted[child]
                    .as_ref()
                    .and_then(|row| row.function.body.as_ref())
                    .ok_or_else(execution_call_error_v29)?;
                let mut count = 0_usize;
                for block in &body.blocks {
                    budget.charge_work(1)?;
                    if let Some(Terminator::Return { values }) = &block.terminator {
                        visit(
                            (
                                child,
                                *values.get(ordinal).ok_or_else(execution_call_error_v29)?,
                            ),
                            budget,
                        )?;
                        count = argument_sum_v1(&[count, 1])?;
                    }
                }
                if count == 0 {
                    return Err(execution_call_error_v29());
                }
            }
            _ => return Ok(false),
        },
        SourceReferencePointerDefinitionV29::BlockParameter(target, ordinal) => {
            let body = emitted[instance]
                .as_ref()
                .and_then(|row| row.function.body.as_ref())
                .ok_or_else(execution_call_error_v29)?;
            let mut count = 0_usize;
            for block in &body.blocks {
                let terminator = block
                    .terminator
                    .as_ref()
                    .ok_or_else(execution_call_error_v29)?;
                let edges = match terminator {
                    Terminator::Switch { cases, .. } => argument_sum_v1(&[cases.len(), 1])?,
                    Terminator::IntegerSwitch { cases, .. } => argument_sum_v1(&[cases.len(), 1])?,
                    _ => 2,
                };
                budget.charge_work(argument_sum_v1(&[edges, 1])?)?;
                source_reference_cell_edge_values_v29(terminator, target.id, |arguments| {
                    if arguments.len() != target.parameters.len() {
                        return Err(execution_call_error_v29());
                    }
                    visit(
                        (
                            instance,
                            *arguments
                                .get(ordinal)
                                .ok_or_else(execution_call_error_v29)?,
                        ),
                        budget,
                    )?;
                    count = argument_sum_v1(&[count, 1])?;
                    Ok(())
                })?;
            }
            // An entry parameter or isolated cycle is not a cell origin.
            if count == 0 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn source_reference_cell_pointer_proof_v29<'plan, 'source, 'kir>(
    references: &SourceReferenceEmissionV29<'plan, 'source>,
    emitted: &'kir [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceCellPointerProofV29<'plan, 'source, 'kir>, ProductionSemanticKirErrorV1>
{
    source_reference_backing_pointer_claims_v29(
        references,
        emitted,
        slots,
        SourceReferencePointerClaimsV29::ScalarOnly,
        budget,
    )
}

fn source_reference_backing_pointer_claims_v29<'plan, 'source, 'kir>(
    references: &SourceReferenceEmissionV29<'plan, 'source>,
    emitted: &'kir [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    claims: SourceReferencePointerClaimsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceCellPointerProofV29<'plan, 'source, 'kir>, ProductionSemanticKirErrorV1>
{
    references.check(budget)?;
    let plan = references.plan;
    let result = (|| {
        source_reference_owned_prepay_v29::<SourceReferenceCellPointerProofV29<'_, '_, '_>>(
            plan, budget,
        )?;
        if slots.ledger != budget.work_ledger_identity_v1()
            || slots.source != ExecutionCallSourceV29::from_instances(plan.instances, budget)?
            || emitted.len() != plan.instances.instances().len()
        {
            return Err(execution_call_error_v29());
        }
        let index = source_reference_pointer_index_v29(references, emitted, slots, budget)?;
        let mut entry_initializations =
            source_reference_owned_vec_v29(plan, slots.slots.len(), budget)?;
        budget.charge_work(slots.slots.len())?;
        entry_initializations.resize(slots.slots.len(), None);
        let mut start = 0_usize;
        for (instance, lowered) in emitted.iter().enumerate() {
            budget.charge_work(4)?;
            let id = plan
                .instances
                .id_at(instance)
                .ok_or_else(execution_call_error_v29)?;
            let Some(lowered) =
                source_reference_active_emitted_v29(plan, instance, lowered, budget)?
            else {
                continue;
            };
            let entry = lowered
                .function
                .body
                .as_ref()
                .and_then(|body| body.blocks.first())
                .ok_or_else(execution_call_error_v29)?
                .id;
            let mut end = start;
            while let Some(slot) = slots.slots.get(end) {
                budget.charge_work(2)?;
                if slot.instance != id {
                    break;
                }
                end = argument_sum_v1(&[end, 1])?;
            }
            visit_scoped_slot_initializers_v29(
                plan.instances,
                id,
                entry,
                &slots.slots[start..end],
                budget,
                |ordinal, _, location, budget| {
                    budget.charge_work(2)?;
                    let ordinal = argument_sum_v1(&[start, ordinal])?;
                    if entry_initializations[ordinal].replace(location).is_some() {
                        return Err(execution_call_error_v29());
                    }
                    Ok(())
                },
            )?;
            start = end;
        }
        if start != slots.slots.len() {
            return Err(execution_call_error_v29());
        }
        let ordinals = source_reference_pointer_ordinals_v29(&index, budget)?;
        let mut cell_slots = source_reference_owned_vec_v29(plan, plan.cells.rows.len(), budget)?;
        let mut seeds = source_reference_owned_vec_v29(plan, ordinals.len(), budget)?;
        budget.charge_work(ordinals.len())?;
        seeds.resize(ordinals.len(), origin_worklist_v1::OriginStateV1::Unknown);
        let array_floor = budget.storage();
        let array_eligibility = if claims == SourceReferencePointerClaimsV29::SelectedBacking {
            Some(source_array_eligibility_v29(plan, budget)?)
        } else {
            None
        };
        let array_scratch = budget
            .storage()
            .checked_sub(array_floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        for (cell_index, cell) in plan.cells.rows.iter().enumerate() {
            budget.charge_work(1)?;
            if let SourceBackingKindV29::Object(schema) = cell.kind {
                if claims == SourceReferencePointerClaimsV29::ScalarOnly {
                    // Checked scalar consumers never acquire object authority.
                    cell_slots.push(None);
                    continue;
                }
                if let Some(slot) = source_array_cell_slot_v29(
                    plan,
                    cell_index,
                    array_eligibility
                        .as_ref()
                        .and_then(|rows| rows.get(cell_index))
                        .copied()
                        .flatten(),
                    &slots.slots,
                    budget,
                )? {
                    let physical = slots.slots[slot];
                    let block = emitted
                        .get(cell.instance.index())
                        .and_then(Option::as_ref)
                        .and_then(|lowered| lowered.function.body.as_ref())
                        .and_then(|body| body.blocks.get(physical.allocation.block_ordinal))
                        .filter(|block| block.id == physical.allocation.block)
                        .ok_or_else(execution_call_error_v29)?;
                    let operation = block
                        .operations
                        .get(physical.allocation.operation)
                        .ok_or_else(execution_call_error_v29)?;
                    check_scoped_slot_alloca_v29(&physical, operation, budget)?;
                    // Every generation must join the same original array schema.
                    // Arrays keep their separate complete memory obligations;
                    // they do not become scalar or typed-object pointer seeds.
                    cell_slots.push(None);
                    continue;
                }
                let slot = source_address_object_slot_v29(
                    plan.instances,
                    plan,
                    slots,
                    cell.instance,
                    cell.local,
                    cell.generation,
                    cell.ty,
                    budget,
                )?;
                let physical = &slots.slots[slot];
                let (representative, represented, _) =
                    plan.physical_object_cell(cell_index, budget)?;
                budget.charge_work(8)?;
                if physical.origin.source
                    != (ScopedAllocationSourceV29::OriginalObject {
                        cell: representative,
                        schema,
                    })
                    || represented.instance != cell.instance
                    || represented.local != cell.local
                    || represented.ty != cell.ty
                    || represented.kind != cell.kind
                {
                    return Err(execution_call_error_v29());
                }
                let block = emitted
                    .get(cell.instance.index())
                    .and_then(Option::as_ref)
                    .and_then(|lowered| lowered.function.body.as_ref())
                    .and_then(|body| body.blocks.get(physical.allocation.block_ordinal))
                    .filter(|block| block.id == physical.allocation.block)
                    .ok_or_else(execution_call_error_v29)?;
                let operation = block
                    .operations
                    .get(physical.allocation.operation)
                    .ok_or_else(execution_call_error_v29)?;
                check_scoped_slot_alloca_v29(physical, operation, budget)?;
                cell_slots.push(Some(slot));
                // Every logical source row above is checked independently.
                // Its unique representative alone publishes the allocation seed.
                if representative != cell_index {
                    continue;
                }
                let definition = source_reference_pointer_ordinal_v29(
                    &ordinals,
                    (cell.instance.index(), physical.origin.pointer),
                    budget,
                )?;
                if seeds[definition] != origin_worklist_v1::OriginStateV1::Unknown {
                    return Err(execution_call_error_v29());
                }
                seeds[definition] = origin_worklist_v1::OriginStateV1::Exact(Some(slot));
                continue;
            }
            charge_execution_cfg_lookup_v29(slots.slots.len(), budget)?;
            let slot = slots
                .slots
                .binary_search_by_key(
                    &(
                        cell.instance.index(),
                        ScopedAllocationIdentityV29::LegacyLocal(cell.local.index()),
                    ),
                    |slot| (slot.instance.index(), slot.origin.identity),
                )
                .map_err(|_| execution_call_error_v29())?;
            let physical = &slots.slots[slot];
            budget.charge_work(9)?;
            let scalar = physical.scalar_array()?;
            if physical.origin.semantic_type != cell.ty
                || scalar.element_type != cell.ty
                || scalar.length != 1
                || scalar.bytes != scalar.element.size
            {
                return Err(execution_call_error_v29());
            }
            cell_slots.push(Some(slot));
            let definition = source_reference_pointer_ordinal_v29(
                &ordinals,
                (cell.instance.index(), physical.origin.pointer),
                budget,
            )?;
            seeds[definition] = origin_worklist_v1::OriginStateV1::Exact(Some(slot));
        }
        drop(array_eligibility);
        budget.release_storage(array_scratch)?;
        if claims == SourceReferencePointerClaimsV29::SelectedBacking {
            source_reference_pending_read_seeds_v29(
                plan,
                emitted,
                &index,
                &ordinals,
                &cell_slots,
                &mut seeds,
                budget,
            )?;
        }
        let mut edges = source_reference_owned_vec_v29(plan, 0, budget)?;
        for &(key, (_, definition)) in &index.definitions {
            let target = source_reference_pointer_ordinal_v29(&ordinals, key, budget)?;
            let before = edges.len();
            let transported = source_reference_pointer_dependencies_v29(
                &index,
                emitted,
                key,
                definition,
                claims,
                budget,
                |source, budget| {
                    let source = source_reference_pointer_ordinal_v29(&ordinals, source, budget)?;
                    source_reference_owned_push_v29(plan, &mut edges, (source, target), budget)
                },
            )?;
            if matches!(
                seeds[target],
                origin_worklist_v1::OriginStateV1::Exact(Some(_))
            ) {
                if edges.len() != before {
                    return Err(execution_call_error_v29());
                }
            } else if transported {
                seeds[target] = origin_worklist_v1::OriginStateV1::Pending;
            } else if matches!(
                definition,
                SourceReferencePointerDefinitionV29::Parameter(_)
            ) || matches!(definition, SourceReferencePointerDefinitionV29::Operation(operation, _)
                    if matches!(operation.kind, OperationKind::Alloca { .. }))
            {
                seeds[target] = origin_worklist_v1::OriginStateV1::Exact(None);
            }
        }
        // The existing solver converts every ungrounded cycle to Unknown and
        // propagates it, including a cycle hidden behind a seeded alternative.
        type Work = origin_worklist_v1::OriginWorkV1<Option<usize>>;
        type Error = origin_worklist_v1::OriginWorkErrorV1;
        use std::mem::size_of;
        budget.reserve_storage(argument_sum_v1(&[
            argument_product_v1(2, size_of::<Work>())?,
            argument_product_v1(2, size_of::<Result<Work, Error>>())?,
            argument_product_v1(
                5,
                argument_sum_v1(&[
                    size_of::<Vec<usize>>(),
                    argument_product_v1(2, size_of::<Result<Vec<usize>, Error>>())?,
                ])?,
            )?,
            argument_product_v1(
                2,
                size_of::<Result<Vec<SourceReferencePhysicalOriginV29>, Error>>(),
            )?,
        ])?)?;
        let mut work = Work::new(seeds.len(), edges.len(), budget)
            .map_err(source_reference_origin_error_v29)?;
        for seed in seeds {
            work.seed_next(seed, budget)
                .map_err(source_reference_origin_error_v29)?;
        }
        for (source, target) in edges {
            work.add_link(source, target, budget)
                .map_err(source_reference_origin_error_v29)?;
        }
        let origins = work
            .solve(budget)
            .map_err(source_reference_origin_error_v29)?;
        Ok(SourceReferenceCellPointerProofV29 {
            plan,
            index,
            ordinals,
            origins,
            cell_slots,
            entry_initializations,
            operands: BTreeMap::new(),
            selectors: Vec::new(),
            floor: budget.storage(),
        })
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SourceReferenceCellPointerProofV29<'_, '_, '_> {
    fn cell_slot(
        &self,
        key: SourceReferencePointerKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        self.plan.check_owner(self.plan.instances, budget)?;
        let result = (|| {
            if budget.storage() < self.floor {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            charge_execution_cfg_lookup_v29(self.ordinals.len(), budget)?;
            let Ok(ordinal) = self.ordinals.binary_search(&key) else {
                return Ok(None);
            };
            Ok(match self.origins[ordinal] {
                origin_worklist_v1::OriginStateV1::Exact(Some(slot)) => Some(slot),
                _ => None,
            })
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self.plan, error))
    }

    fn require_not_cell(
        &self,
        key: SourceReferencePointerKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.cell_slot(key, budget)?.is_some() {
            return Err(source_reference_error_v29(
                "source reference occupies an unbound ABI component",
            ));
        }
        Ok(())
    }

    fn operand(
        &self,
        key: SourceReferenceCellOperandKeyV29,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.plan.check_owner(self.plan.instances, budget)?;
        let result = (|| {
            if budget.storage() < self.floor {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            charge_execution_cfg_lookup_v29(self.operands.len(), budget)?;
            if let Some(&cell) = self.operands.get(&key) {
                self.require_cell(cell, (key.0, value), budget)?;
                Ok(true)
            } else {
                Ok(false)
            }
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self.plan, error))
    }

    fn require_cell(
        &self,
        cell: usize,
        key: SourceReferencePointerKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.plan.check_owner(self.plan.instances, budget)?;
        self.plan.charge(1, budget)?;
        if self.plan.cells.rows.get(cell).map(|row| row.kind) != Some(SourceBackingKindV29::Scalar)
        {
            let error = execution_call_error_v29();
            source_reference_record_failure_v29(self.plan, &error);
            return Err(error);
        }
        self.require_payload_backing(cell, key, budget)
    }

    // This authenticates a pending ABI origin, not a memory-use permit.
    // Selected-object claims are dropped before final all-use/currentness replay.
    fn require_payload_backing(
        &self,
        cell: usize,
        key: SourceReferencePointerKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let plan = self.plan;
        plan.check_owner(plan.instances, budget)?;
        if budget.storage() < self.floor {
            plan.failure.record_resource(ArgumentResourceV1::Accounting);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let result = (|| {
            let row = plan
                .cells
                .rows
                .get(cell)
                .ok_or_else(execution_call_error_v29)?;
            let slot = self
                .cell_slots
                .get(cell)
                .copied()
                .flatten()
                .ok_or_else(execution_call_error_v29)?;
            let ordinal = source_reference_pointer_ordinal_v29(&self.ordinals, key, budget)?;
            if self.origins[ordinal] != origin_worklist_v1::OriginStateV1::Exact(Some(slot)) {
                return Err(source_reference_error_v29(
                    "source reference pointer differs from its exact retained cell",
                ));
            }
            let (ty, _) = source_reference_pointer_definition_lookup_v29(
                &self.index.definitions,
                key,
                budget,
            )?
            .ok_or_else(execution_call_error_v29)?;
            let Type::Pointer(pointer) = ty else {
                return Err(execution_call_error_v29());
            };
            budget.charge_work(3)?;
            let expected = match row.kind {
                SourceBackingKindV29::Scalar => {
                    lower_scalar_type(plan.instances.owner().source_semantic().types(), row.ty)?
                }
                SourceBackingKindV29::Object(schema) => Type::StorageObject(schema),
            };
            if pointer.address_space != AddressSpace::Private
                || pointer.pointee.as_ref() != &expected
            {
                return Err(execution_call_error_v29());
            }
            Ok(())
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
    }
}
