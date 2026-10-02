// A planned scalar cell becomes usable only through the existing retained slot
// or an exact source-reference payload carried from that slot through SSA/calls.
#[derive(Clone, Copy)]
struct SourceReferenceCellUseV29 {
    cell: usize,
    pointer: ValueId,
    block: BlockId,
    operation: Option<usize>,
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn claim_cell_access(
        &self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        used: SourceReferenceCellUseV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        source_reference_access_at_v29(self.plan, site, source, access, budget)?;
        let index = self.plan.access_sites[&source_reference_access_key_v29(site, source, access)];
        budget.source_reference_charge_v29(self.plan, 2)?;
        if self.cell_failure_reads[index].get().is_some()
            || self.cell_accesses[index].replace(Some(used)).is_some()
        {
            return Err(source_reference_error_v29(
                "source reference cell access was emitted twice",
            ));
        }
        Ok(())
    }

    fn claim_cell_failure_read(
        &self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        receipt: ScopedMemoryAnchorV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let result = (|| {
            let instance = self
                .plan
                .instances
                .instance(site.instance)
                .ok_or_else(execution_call_error_v29)?;
            let occurrences = self
                .plan
                .instances
                .occurrences(site.instance)
                .ok_or_else(execution_call_error_v29)?;
            let original = checked_scoped_failure_read_v29(
                instance.declaration(),
                &occurrences,
                &receipt,
                budget,
            )?;
            if !std::ptr::eq(original, source)
                || site.statement.is_some()
                || receipt
                    .source
                    .map(|frame| scoped_memory_site_key_v29(frame.site))
                    != Some((site.block.index(), None))
            {
                return Err(execution_call_error_v29());
            }
            budget.source_reference_charge_v29(
                self.plan,
                scoped_initialization_search_work_v29(self.plan.access_sites.len()),
            )?;
            let key = source_reference_access_key_v29(site, source, SourceReferenceAccessV29::Read);
            let Some(&index) = self.plan.access_sites.get(&key) else {
                // Non-reference operands still have their independently checked
                // scoped receipt; they do not acquire a cell claim here.
                return Ok(());
            };
            let row = source_reference_access_at_v29(
                self.plan,
                site,
                source,
                SourceReferenceAccessV29::Read,
                budget,
            )?;
            let Some(loan) = row.loan else {
                // Demanded direct/object accesses also have original locators.
                // Their scoped failure receipt is not a scalar-reference claim.
                return Ok(());
            };
            if budget
                .source_reference_scalar_cell_v29(self.plan, loan)?
                .is_none()
            {
                return Ok(());
            }
            budget.source_reference_charge_v29(self.plan, 2)?;
            if self.cell_accesses[index].get().is_some()
                || self.cell_failure_reads[index]
                    .replace(Some(receipt))
                    .is_some()
            {
                return Err(source_reference_error_v29(
                    "source reference failure read was claimed twice",
                ));
            }
            Ok(())
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self.plan, error))
    }
}

impl SourceReferenceCellPointerProofV29<'_, '_, '_> {
    fn check_source_accesses(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        references.check(budget)?;
        let plan = self.plan;
        if !std::ptr::eq(plan, references.plan) {
            return Err(execution_call_error_v29());
        }
        for (index, row) in plan.accesses.iter().enumerate() {
            budget.charge_work(10)?;
            let loan = if matches!(row.key.access, SourceReferenceAccessV29::Borrow(_)) {
                references.loan_at(row.key.site, budget)?
            } else {
                row.loan
            };
            let cell = loan
                .map(|loan| plan.backing_cell(loan, budget))
                .transpose()?
                .flatten()
                .filter(|(_, cell)| cell.kind == SourceBackingKindV29::Scalar);
            let used = references
                .cell_accesses
                .get(index)
                .ok_or_else(execution_call_error_v29)?
                .get();
            let failure = references
                .cell_failure_reads
                .get(index)
                .ok_or_else(execution_call_error_v29)?
                .get();
            let Some((cell_index, cell)) = cell else {
                if used.is_some() || failure.is_some() {
                    return Err(execution_call_error_v29());
                }
                continue;
            };
            if let Some(receipt) = failure {
                if used.is_some()
                    || row.key.access != SourceReferenceAccessV29::Read
                    || row.instance != cell.instance
                    || row.local != cell.local
                    || row.generation != cell.generation
                    || row.ty != cell.ty
                    || !row.projections.is_empty()
                {
                    return Err(execution_call_error_v29());
                }
                self.check_failure_read(row, receipt, emitted, budget)?;
                continue;
            }
            let used = used.ok_or_else(|| {
                source_reference_error_v29(
                    "source reference cell access has no exact emitted occurrence",
                )
            })?;
            if used.cell != cell_index
                || row.instance != cell.instance
                || row.local != cell.local
                || row.generation != cell.generation
                || row.ty != cell.ty
                || !row.projections.is_empty()
            {
                return Err(execution_call_error_v29());
            }
            let instance = row.key.site.instance.index();
            self.require_cell(cell_index, (instance, used.pointer), budget)?;
            if matches!(row.key.access, SourceReferenceAccessV29::Borrow(_)) {
                if used.operation.is_some() {
                    return Err(execution_call_error_v29());
                }
                continue;
            }
            let ordinal = used.operation.ok_or_else(execution_call_error_v29)?;
            let lowered = emitted
                .get(instance)
                .and_then(Option::as_ref)
                .ok_or_else(execution_call_error_v29)?;
            let spans = match row.key.site.statement {
                Some(_) => lowered.statement_operation_spans.len(),
                None => lowered.terminator_operation_spans.len(),
            };
            budget.charge_work(argument_product_v1(spans, 6)?)?;
            let in_span = match row.key.site.statement {
                Some(statement) => lowered
                    .statement_operation_spans
                    .iter()
                    .filter(|span| {
                        span.semantic_block == row.key.site.block
                            && span.statement_ordinal as usize == statement
                            && span.kernel_ir_block == used.block
                            && ordinal >= span.first_operation_ordinal as usize
                            && ordinal
                                < span.first_operation_ordinal as usize
                                    + span.operation_count as usize
                    })
                    .count(),
                None => lowered
                    .terminator_operation_spans
                    .iter()
                    .filter(|span| {
                        span.semantic_block == row.key.site.block
                            && span.kernel_ir_block == used.block
                            && ordinal >= span.first_operation_ordinal as usize
                            && ordinal
                                < span.first_operation_ordinal as usize
                                    + span.operation_count as usize
                    })
                    .count(),
            };
            if in_span != 1 {
                return Err(execution_call_error_v29());
            }
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            budget.charge_work(body.blocks.len())?;
            let operation = body
                .blocks
                .iter()
                .find(|block| block.id == used.block)
                .and_then(|block| block.operations.get(ordinal))
                .ok_or_else(execution_call_error_v29)?;
            let (pointer, writing) = match &operation.kind {
                OperationKind::Load { pointer, .. }
                    if row.key.access == SourceReferenceAccessV29::Read =>
                {
                    (*pointer, false)
                }
                OperationKind::Store { pointer, .. }
                    if row.key.access == SourceReferenceAccessV29::Write =>
                {
                    (*pointer, true)
                }
                _ => return Err(execution_call_error_v29()),
            };
            if pointer != used.pointer || (writing && row.shared_path) {
                return Err(execution_call_error_v29());
            }
            self.check_memory(
                cell_index,
                (instance, pointer),
                operation,
                writing,
                None,
                budget,
            )?;
            self.bind_operand(
                cell_index,
                (instance, used.block, Some(ordinal), 0),
                pointer,
                budget,
            )?;
        }
        Ok(())
    }

    fn check_failure_read(
        &self,
        row: &SourceReferenceAccessRecordV29,
        receipt: ScopedMemoryAnchorV29,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let instance = self
            .plan
            .instances
            .instance(row.key.site.instance)
            .ok_or_else(execution_call_error_v29)?;
        let occurrences = self
            .plan
            .instances
            .occurrences(row.key.site.instance)
            .ok_or_else(execution_call_error_v29)?;
        let original = checked_scoped_failure_read_v29(
            instance.declaration(),
            &occurrences,
            &receipt,
            budget,
        )?;
        if original as *const SemanticPlaceV1 as usize != row.key.source
            || row.key.site.statement.is_some()
            || receipt
                .source
                .map(|frame| scoped_memory_site_key_v29(frame.site))
                != Some((row.key.site.block.index(), None))
        {
            return Err(execution_call_error_v29());
        }
        let lowered = emitted
            .get(row.key.site.instance.index())
            .and_then(Option::as_ref)
            .ok_or_else(execution_call_error_v29)?;
        let anchors = lowered
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_memory_error_v29)?;
        budget.charge_work(argument_sum_v1(&[6, anchors.rows.len()])?)?;
        if anchors.subject.ledger != budget.work_ledger_identity_v1()
            || anchors.subject.instance != row.key.site.instance
            || anchors.subject.function != instance.function()
            || anchors.subject.source
                != ExecutionCallSourceV29::from_instances(self.plan.instances, budget)?
            || anchors
                .rows
                .iter()
                .filter(|&&candidate| candidate == receipt)
                .count()
                != 1
        {
            return Err(scoped_memory_error_v29());
        }
        budget.charge_work(argument_product_v1(
            lowered.terminator_operation_spans.len(),
            6,
        )?)?;
        if lowered
            .terminator_operation_spans
            .iter()
            .filter(|span| {
                span.semantic_block == row.key.site.block
                    && span.kernel_ir_block == receipt.block
                    && receipt.position >= span.first_operation_ordinal as usize
                    && receipt.position
                        <= span.first_operation_ordinal as usize + span.operation_count as usize
            })
            .count()
            != 1
        {
            return Err(scoped_memory_error_v29());
        }
        Ok(())
    }

    fn check_memory(
        &self,
        cell: usize,
        key: SourceReferencePointerKeyV29,
        operation: &Operation,
        writing: bool,
        original_read: Option<SourceReferenceSiteV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.require_cell(cell, key, budget)?;
        let row = self.plan.cells.rows[cell];
        let types = self.plan.instances.owner().source_semantic().types();
        let mut expected = memory_access_for_type(types, row.ty, AddressSpace::Private)?;
        if let Some(site) = original_read {
            if writing {
                return Err(execution_call_error_v29());
            }
            expected.volatile =
                source_reference_cell_read_volatility_v29(self.plan.instances, row, site, budget)?;
        }
        let (pointer, access) = match &operation.kind {
            OperationKind::Load { pointer, access } if !writing => (*pointer, access),
            OperationKind::Store {
                pointer, access, ..
            } if writing => (*pointer, access),
            _ => return Err(execution_call_error_v29()),
        };
        let (Type::Pointer(ty), _) =
            source_reference_pointer_definition_lookup_v29(&self.index.definitions, key, budget)?
                .ok_or_else(execution_call_error_v29)?
        else {
            return Err(execution_call_error_v29());
        };
        budget.charge_work(5)?;
        if pointer != key.1
            || *access != expected
            || (writing && ty.access != AccessMode::ReadWrite)
            || (!writing && ty.access == AccessMode::WriteOnly)
        {
            return Err(execution_call_error_v29());
        }
        Ok(())
    }

    fn ordinary_local_memory<'kir>(
        &'kir self,
        prologue: &mut Option<SourceEntryPrologueV29<'kir>>,
        slots: &OwnedScopedSourceSlotsV29,
        instance: usize,
        lowered: &'kir LoweredFunctionResultV1,
        block: BlockId,
        ordinal: usize,
        operation: &Operation,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let physical = slots.slots.get(slot).ok_or_else(execution_call_error_v29)?;
        if physical.instance.index() != instance {
            return Err(execution_call_error_v29());
        }
        let (pointer, writing) = match operation.kind {
            OperationKind::Load { pointer, .. } => (pointer, false),
            OperationKind::Store { pointer, .. } => (pointer, true),
            _ => return Err(execution_call_error_v29()),
        };
        let id = self
            .plan
            .instances
            .id_at(instance)
            .ok_or_else(execution_call_error_v29)?;
        let location = PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block,
            operation: ordinal,
        };
        let is_initializer = writing
            && physical.allocation.block_ordinal == 0
            && physical.allocation.block == block
            && self.entry_initializations.get(slot).copied().flatten() == Some(location);
        let frame = if is_initializer {
            if prologue.is_none() {
                *prologue = Some(SourceEntryPrologueV29::new(
                    self.plan, id, lowered, location, budget,
                )?);
            }
            prologue
                .as_ref()
                .ok_or_else(execution_call_error_v29)?
                .anchor(self.plan, id, lowered, location, Some(pointer), budget)?
                .source
        } else {
            let anchors = &lowered
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?
                .rows;
            budget.charge_work(argument_product_v1(anchors.len(), 4)?)?;
            let mut matching = anchors.iter().filter(|anchor| anchor.block == block && anchor.position == ordinal
                && matches!(anchor.kind, ScopedMemoryAnchorKindV29::Access { pointer: value, .. } if value == pointer));
            let frame = matching.next().ok_or_else(scoped_memory_error_v29)?.source;
            if matching.next().is_some() {
                return Err(scoped_memory_error_v29());
            }
            frame
        };
        let function = self
            .plan
            .instances
            .instance(id)
            .ok_or_else(execution_call_error_v29)?
            .declaration();
        let Some(frame) = frame else {
            budget.charge_work(3)?;
            let initialization = self
                .entry_initializations
                .get(slot)
                .copied()
                .flatten()
                .ok_or_else(scoped_memory_error_v29)?;
            if !writing
                || block != physical.allocation.block
                || physical.allocation.block_ordinal != 0
                || initialization
                    != (PrivateArrayPhysicalLocationV1 {
                        block_ordinal: 0,
                        block,
                        operation: ordinal,
                    })
            {
                return Err(scoped_memory_error_v29());
            }
            return source_reference_cell_entry_store_with_prologue_v29(
                SourceEntryQueryV29::Prologue(
                    prologue.as_ref().ok_or_else(scoped_memory_error_v29)?,
                ),
                self.plan,
                id,
                physical.origin,
                lowered,
                operation,
                budget,
            );
        };
        let place = match frame.role {
            Some(ScopedMemoryRoleV29::Operand(role)) => {
                scoped_source_place_v29(function, frame.site, role).or_else(|| {
                    match scoped_source_operand_v29(function, frame.site, role)? {
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                            Some(place)
                        }
                        SemanticOperandV1::Constant(_) => None,
                    }
                })
            }
            Some(ScopedMemoryRoleV29::CallResult) if writing => {
                scoped_source_call_destination_v29(function, frame.site)
            }
            _ => None,
        }
        .ok_or_else(scoped_memory_error_v29)?;
        if place.local().index() != physical.legacy_local()?
            || place.ty() != physical.origin.semantic_type
            || !place.projections().is_empty()
        {
            return Err(scoped_memory_error_v29());
        }
        budget.charge_work(self.cell_slots.len())?;
        let cell = self
            .cell_slots
            .iter()
            .position(|&value| value == Some(slot))
            .ok_or_else(execution_call_error_v29)?;
        let original_read = (!writing
            && frame.role
                == Some(ScopedMemoryRoleV29::Operand(
                    ExecutionOperandV29::RvaluePlace,
                )))
        .then_some(SourceReferenceSiteV29 {
            instance: id,
            block: match frame.site {
                ExecutionSiteV29::Statement { block, .. }
                | ExecutionSiteV29::Terminator { block } => {
                    SemanticBlockIdV1::from_index(block.get())
                }
            },
            statement: match frame.site {
                ExecutionSiteV29::Statement { statement, .. } => Some(statement as usize),
                ExecutionSiteV29::Terminator { .. } => None,
            },
        });
        self.check_memory(
            cell,
            (instance, pointer),
            operation,
            writing,
            original_read,
            budget,
        )
    }
}

include!("production_source_entry_prologue_v29.rs");

fn source_reference_cell_entry_store_with_prologue_v29(
    query: SourceEntryQueryV29<'_, '_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    origin: ScopedSlotOriginV29,
    lowered: &LoweredFunctionResultV1,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let local = origin.legacy_local()?;
    budget.charge_work(argument_product_v1(plan.cells.rows.len(), 4)?)?;
    if !plan.cells.rows.iter().any(|cell| {
        cell.instance == instance
            && cell.local.index() == local
            && cell.ty == origin.semantic_type
            && cell.generation == 0
            && cell.kind == SourceBackingKindV29::Scalar
    }) {
        return Err(scoped_slot_error_v29());
    }
    source_reference_retained_scalar_entry_store_with_prologue_v29(
        query, plan, instance, origin, lowered, operation, budget,
    )
}

// Ordinary retained scalar arguments need the same ABI-bound initializer as
// cells, but their storage does not imply a C1 scalar-cell strategy.
fn source_reference_retained_scalar_entry_store_with_prologue_v29(
    query: SourceEntryQueryV29<'_, '_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    origin: ScopedSlotOriginV29,
    lowered: &LoweredFunctionResultV1,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    budget.charge_work(5)?;
    let local = origin.legacy_local()?;
    let declaration = plan
        .instances
        .instance(instance)
        .and_then(|row| row.declaration().locals().get(local as usize))
        .ok_or_else(scoped_slot_error_v29)?;
    if lowered.source_call_instance != Some(instance)
        || declaration.ty() != origin.semantic_type
        || !declaration.role().is_entry_argument()
    {
        return Err(scoped_slot_error_v29());
    }
    let parameter = query.parameter(
        plan,
        instance,
        SemanticLocalIdV1::from_index(local),
        lowered,
        budget,
    )?;
    let expected = memory_access_for_type(
        plan.instances.owner().source_semantic().types(),
        origin.semantic_type,
        AddressSpace::Private,
    )?;
    budget.charge_work(4)?;
    if !operation.results.is_empty()
        || !matches!(&operation.kind,
        OperationKind::Store { pointer, value, access }
        if *pointer == origin.pointer && *value == parameter && *access == expected)
    {
        return Err(scoped_slot_error_v29());
    }
    Ok(())
}

#[cfg(test)]
fn source_reference_cell_entry_store_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    origin: ScopedSlotOriginV29,
    lowered: &LoweredFunctionResultV1,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_cell_entry_store_with_prologue_v29(
        SourceEntryQueryV29::OneShot,
        plan,
        instance,
        origin,
        lowered,
        operation,
        budget,
    )
}

#[cfg(test)]
fn source_reference_retained_scalar_entry_store_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    origin: ScopedSlotOriginV29,
    lowered: &LoweredFunctionResultV1,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_retained_scalar_entry_store_with_prologue_v29(
        SourceEntryQueryV29::OneShot,
        plan,
        instance,
        origin,
        lowered,
        operation,
        budget,
    )
}

fn source_reference_cell_pointer_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    loan: usize,
    cell: SourceReferenceScalarCellV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<Type>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 5)?;
    let record = plan.loans.get(loan).ok_or_else(execution_call_error_v29)?;
    let origin = &plan.origins[record.origin];
    if origin.instance != cell.instance
        || origin.local != cell.local
        || origin.generation != cell.generation
        || origin.ty != cell.ty
        || !origin.projections.is_empty()
    {
        return Err(execution_call_error_v29());
    }
    let access = match record.kind {
        SemanticBorrowKindV1::Shared => AccessMode::ReadOnly,
        SemanticBorrowKindV1::Mutable => AccessMode::ReadWrite,
        SemanticBorrowKindV1::Fake => return Err(execution_call_error_v29()),
    };
    let element = lower_scalar_type(plan.instances.owner().source_semantic().types(), cell.ty)?;
    budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
    Ok(Type::pointer(element, AddressSpace::Private, access))
}

fn source_reference_access_at_v29<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SourceReferenceAccessRecordV29, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(
        plan,
        argument_sum_v1(&[
            plan.access_sites.len().checked_ilog2().unwrap_or(0) as usize,
            9,
        ])?,
    )?;
    let key = source_reference_access_key_v29(site, source, access);
    let index = plan.access_sites.get(&key).copied().ok_or_else(|| {
        source_reference_error_v29("source reference access occurrence is not retained")
    })?;
    let row = plan
        .accesses
        .get(index)
        .ok_or_else(execution_call_error_v29)?;
    if row.key.site != site
        || row.key.source != source as *const SemanticPlaceV1 as usize
        || row.key.access != access
        || row.source_local != source.local()
        || row.ty != source.ty()
    {
        return Err(execution_call_error_v29());
    }
    Ok(row)
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn source_reference_return_inputs_v29(
        &mut self,
        local: usize,
    ) -> Result<Option<Vec<(ValueId, Type)>>, ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let Some(cursor) = this.execution.as_ref() else {
                return Ok(None);
            };
            let Some(references) = cursor.references else {
                return Ok(None);
            };
            let plan = references.plan;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Option<Vec<(ValueId, Type)>>>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 4)?;
            let Some(node) = plan.returns.get(cursor.instance.index()).copied().flatten() else {
                return Ok(None);
            };
            if !source_call_requires_captured_carrier_v55(plan, node, budget)?
                && execution_cfg_return_transport_count_v29(
                    plan.instances.owner().source_semantic().types(),
                    plan.nodes[node].ty,
                    budget,
                )? == 0
            {
                return Ok(None);
            }
            let declaration = this
                .function
                .locals()
                .get(local)
                .ok_or_else(execution_call_error_v29)?;
            if declaration.role() != SemanticLocalRoleV1::Return
                || declaration.ty() != plan.nodes[node].ty
            {
                return Err(execution_call_error_v29());
            }
            let binding = this
                .locals
                .get(local)
                .and_then(Option::as_ref)
                .ok_or_else(execution_call_error_v29)?;
            source_reference_call_shape_v29(references, node, binding, budget)?;
            let mut values = source_reference_owned_vec_v29(plan, 0, budget)?;
            source_reference_values_v29(references, binding, &mut values, &mut 0, budget)?;
            let mut inputs = source_reference_owned_vec_v29(plan, values.len(), budget)?;
            for value in values {
                budget.source_reference_charge_v29(plan, 1)?;
                inputs.push((value.id, value.ty));
            }
            Ok(Some(inputs))
        })
    }

    fn source_reference_call_result_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        results: &[ValueDef],
        nominal: &[Option<ExecutionCfgLeafV29>],
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let Some(cursor) = this.execution.as_ref() else {
                return Ok(None);
            };
            let Some(references) = cursor.references else {
                return Ok(None);
            };
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<Option<SemanticValueBindingV1>>(plan, budget)?;
            let calls = plan
                .instances
                .calls(cursor.instance)
                .ok_or_else(execution_call_error_v29)?;
            budget.source_reference_charge_v29(plan, argument_product_v1(calls.len(), 4)?)?;
            let mut matching = calls
                .iter()
                .filter(|incoming| incoming.occurrence().block == block);
            let incoming = matching.next().ok_or_else(execution_call_error_v29)?;
            if matching.next().is_some() || !std::ptr::eq(incoming.source(), call) {
                return Err(execution_call_error_v29());
            }
            let child = incoming.child().ok_or_else(execution_call_error_v29)?;
            if plan.instances.instance(child).map(|row| row.function()) != Some(callee) {
                return Err(execution_call_error_v29());
            }
            let Some(node) = plan.returns.get(child.index()).copied().flatten() else {
                return Ok(None);
            };
            if !source_call_requires_captured_carrier_v55(plan, node, budget)?
                && execution_cfg_return_transport_count_v29(
                    plan.instances.owner().source_semantic().types(),
                    plan.nodes[node].ty,
                    budget,
                )? == 0
            {
                if !nominal.is_empty() {
                    return Err(execution_identity_error_v1());
                }
                return Ok(None);
            }
            if call
                .destination()
                .map(|destination| destination.place().ty())
                != Some(plan.nodes[node].ty)
            {
                return Err(execution_call_error_v29());
            }
            let mut values = results.iter();
            let mut leaves = nominal.iter();
            let binding = source_reference_rebuild_node_v29(
                references,
                node,
                true,
                &mut leaves,
                &mut values,
                &mut 0,
                budget,
            )?;
            if values.next().is_some() || leaves.next().is_some() {
                return Err(execution_call_error_v29());
            }
            Ok(Some(binding))
        })
    }

    fn source_reference_scalar_address_v29(
        &mut self,
        site: SourceReferenceSiteV29,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        loan: usize,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some((references, cell, expected, kind)) =
            self.with_emission_budget_v1(|this, budget| {
                let references = this
                    .execution
                    .as_ref()
                    .and_then(|cursor| cursor.references)
                    .ok_or_else(execution_call_error_v29)?;
                references.check(budget)?;
                let Some((_, cell)) =
                    budget.source_reference_scalar_cell_v29(references.plan, loan)?
                else {
                    return Ok(None);
                };
                let kind = references.plan.loans[loan].kind;
                let access = source_reference_access_at_v29(
                    references.plan,
                    site,
                    source,
                    SourceReferenceAccessV29::Borrow(kind),
                    budget,
                )?;
                if access.instance != cell.instance
                    || access.local != cell.local
                    || access.generation != cell.generation
                    || access.ty != cell.ty
                    || !access.projections.is_empty()
                {
                    return Err(execution_call_error_v29());
                }
                let expected =
                    source_reference_cell_pointer_type_v29(references.plan, loan, cell, budget)?;
                Ok(Some((references, cell, expected, kind)))
            })?
        else {
            return Ok(None);
        };
        let address = if source.projections().is_empty() {
            if site.instance != cell.instance || source.local() != cell.local {
                return Err(execution_call_error_v29());
            }
            self.require_retained_local_initialized_v1(site.block, statement, cell.local)?;
            self.with_emission_budget_v1(|this, budget| {
                references.check(budget)?;
                budget.source_reference_charge_v29(references.plan, 4)?;
                let slot = lookup_legacy_retained_slot_v29(
                    &this.retained_local_slots,
                    cell.local.index(),
                    Some(budget),
                )?
                .ok_or_else(execution_call_error_v29)?;
                if slot.semantic_type != cell.ty
                    || *slot.storage.scalar_array()?.0 != lower_scalar_type(this.types, cell.ty)?
                {
                    return Err(execution_call_error_v29());
                }
                // The retained slot is the original SSA candidate's allocation,
                // not a new private copy of the borrowed value.
                Ok(())
            })?;
            let access = match kind {
                SemanticBorrowKindV1::Shared => AccessMode::ReadOnly,
                SemanticBorrowKindV1::Mutable => AccessMode::ReadWrite,
                SemanticBorrowKindV1::Fake => return Err(execution_call_error_v29()),
            };
            self.retained_local_pointer_binding_v1(cell.local, access, operations)?
        } else {
            self.resolve_place_for_source_reference_access_v29(
                site.block,
                statement,
                source,
                SourceReferenceAccessV29::Borrow(kind),
                operations,
            )?
            .0
        };
        let (pointer, actual) = address.value().map_err(source_reference_error_v29)?;
        let address = if actual == expected {
            address
        } else {
            let (Type::Pointer(from), Type::Pointer(to)) = (&actual, &expected) else {
                return Err(execution_call_error_v29());
            };
            if from.address_space != AddressSpace::Private
                || to.address_space != AddressSpace::Private
                || from.pointee != to.pointee
                || from.access != AccessMode::ReadWrite
                || to.access != AccessMode::ReadOnly
            {
                return Err(execution_call_error_v29());
            }
            self.emit(
                operations,
                expected.clone(),
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: pointer,
                    to: expected,
                },
            )?
        };
        self.with_emission_budget_v1(|this, budget| {
            let (cell, _) = budget
                .source_reference_scalar_cell_v29(references.plan, loan)?
                .ok_or_else(execution_call_error_v29)?;
            references.claim_cell_access(
                site,
                source,
                SourceReferenceAccessV29::Borrow(kind),
                SourceReferenceCellUseV29 {
                    cell,
                    pointer: address.value().map_err(source_reference_error_v29)?.0,
                    block: this.kernel_block_id_v1(site.block)?,
                    operation: None,
                },
                budget,
            )
        })?;
        Ok(Some(address))
    }

    #[allow(clippy::too_many_arguments)]
    fn try_dereference_scalar_cell_v29(
        &mut self,
        binding: &SemanticSourceReferenceBindingV29,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        projection: usize,
        access: SourceReferenceAccessV29,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some((pointer, pointer_type, ty)) = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            let references = cursor.references.ok_or_else(execution_call_error_v29)?;
            references.check(budget)?;
            let SourceReferenceBindingOriginV29::SingleLoan(loan) = binding.origin else {
                return Ok(None);
            };
            let Some((cell_index, cell)) =
                budget.source_reference_scalar_cell_v29(references.plan, loan)?
            else {
                return Ok(None);
            };
            source_reference_validate_binding_v29(references.plan, binding, budget)?;
            let site = SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|value| value as usize),
            };
            let row =
                source_reference_access_at_v29(references.plan, site, source, access, budget)?;
            budget.source_reference_charge_v29(references.plan, 10)?;
            if projection.checked_add(1) != Some(source.projections().len())
                || row.instance != cell.instance
                || row.local != cell.local
                || row.generation != cell.generation
                || row.ty != cell.ty
                || row.loan != Some(loan)
                || !row.projections.is_empty()
                || references
                    .plan
                    .access_loans
                    .get(row.traversed.clone())
                    .and_then(|loans| loans.last())
                    != Some(&loan)
                || binding.values.len() != 1
            {
                return Err(execution_call_error_v29());
            }
            let pointer = &binding.values[0];
            let Type::Pointer(pointer_type) = &pointer.ty else {
                return Err(execution_call_error_v29());
            };
            if matches!(
                access,
                SourceReferenceAccessV29::Write
                    | SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable)
            ) && (row.shared_path || pointer_type.access != AccessMode::ReadWrite)
            {
                return Err(execution_call_error_v29());
            }
            let cloned = execution_cfg_clone_type_v29(&pointer.ty, budget)
                .inspect_err(|error| source_reference_record_failure_v29(references.plan, error))?;
            if !matches!(access, SourceReferenceAccessV29::Borrow(_)) {
                references.claim_cell_access(
                    site,
                    source,
                    access,
                    SourceReferenceCellUseV29 {
                        cell: cell_index,
                        pointer: pointer.id,
                        block: this.kernel_block_id_v1(block)?,
                        operation: Some(operations.len()),
                    },
                    budget,
                )?;
            }
            Ok(Some((pointer.id, cloned, cell.ty)))
        })?
        else {
            return Ok(None);
        };
        if access != SourceReferenceAccessV29::Read {
            return Ok(Some(SemanticValueBindingV1::Value {
                id: pointer,
                ty: pointer_type,
            }));
        }
        let element = lower_scalar_type(self.types, ty)?;
        let access = memory_access_for_type(self.types, ty, AddressSpace::Private)?;
        self.with_scoped_read_payload_v29(source, source.projections().len(), |this| {
            this.emit(operations, element, OperationKind::Load { pointer, access })
        })
        .map(Some)
    }
}
