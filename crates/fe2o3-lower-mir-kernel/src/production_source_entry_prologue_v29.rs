// Immutable, transaction-local reconstruction. Never retain this across an
// observer or make it part of an owned inventory.
struct SourceEntryPrologueV29<'kir> {
    lowered: &'kir LoweredFunctionResultV1,
    instance: ProductionCallInstanceIdV1,
    // Borrowing the custody slot also keeps the exact plan alive and immovable.
    plan: &'kir usize,
    ledger: ArgumentLedgerV1,
    floor: usize,
    block: BlockId,
    first: usize,
    parameters: Vec<Option<usize>>,
    anchors: Vec<SourceEntryAnchorRowV29>,
}

#[derive(Clone, Copy, Default)]
struct SourceEntryAnchorRowV29 {
    any: Option<usize>,
    any_duplicate: bool,
    access: Option<usize>,
    access_duplicate: bool,
}

fn source_entry_anchor_index_v29(
    first: PrivateArrayPhysicalLocationV1,
    end: usize,
    operations: &[Operation],
    source: &[ScopedMemoryAnchorV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceEntryAnchorRowV29>, ProductionSemanticKirErrorV1> {
    if first.block_ordinal != 0 || first.operation >= end || end > operations.len() {
        return Err(scoped_slot_error_v29());
    }
    let count = end
        .checked_sub(first.operation)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    source_reference_emission_prepay_v29::<Vec<SourceEntryAnchorRowV29>>(budget)?;
    let mut anchors = emission_vec_v1(count, budget)?;
    budget.charge_work(count)?;
    anchors.resize(count, SourceEntryAnchorRowV29::default());
    for (index, anchor) in source.iter().enumerate() {
        budget.charge_work(8)?;
        if anchor.block != first.block {
            continue;
        }
        let Some(offset) = anchor.position.checked_sub(first.operation) else {
            continue;
        };
        let Some(target) = anchors.get_mut(offset) else {
            continue;
        };
        target.any_duplicate |= target.any.replace(index).is_some();
        if let (
            ScopedMemoryAnchorKindV29::Access { pointer, .. },
            OperationKind::Store {
                pointer: actual, ..
            },
        ) = (&anchor.kind, &operations[anchor.position].kind)
        {
            if pointer == actual {
                target.access_duplicate |= target.access.replace(index).is_some();
            }
        }
    }
    Ok(anchors)
}

fn source_entry_parameter_roster_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<usize>>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let row = plan
        .instances
        .instance(instance)
        .ok_or_else(execution_call_error_v29)?;
    let function = row.declaration();
    let body = lowered
        .function
        .body
        .as_ref()
        .ok_or_else(execution_call_error_v29)?;
    if lowered.source_call_instance != Some(instance) {
        return Err(execution_call_error_v29());
    }
    source_reference_owned_prepay_v29::<Vec<Option<usize>>>(plan, budget)?;
    let mut parameters = emission_vec_v1(function.locals().len(), budget)?;
    budget.charge_work(function.locals().len())?;
    parameters.resize(function.locals().len(), None);
    with_canonical_call_scratch_v1(budget, |budget| {
        if instance != plan.instances.root() {
            let layout =
                execution_function_layout_v29(plan.instances, instance, Some(plan), budget)?;
            budget.charge_work(layout.parameter_types.len())?;
            if layout.parameter_types != lowered.function.signature.parameters
                || layout.parameter_types.len() != body.parameters.len()
                || layout.call_arguments.len() != body.parameters.len()
            {
                return Err(execution_call_error_v29());
            }
            let mut ordinal = 0usize;
            for &(argument, local, ty) in &layout.parameter_declarations {
                budget.charge_work(5)?;
                let selector = plan
                    .instances
                    .parameter_source(
                        instance,
                        SemanticLocalIdV1::from_index(
                            u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        budget,
                    )
                    .map_err(|error| match error {
                        production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(
                            error,
                        ) => error.into(),
                        _ => execution_call_error_v29(),
                    })?;
                if selector.source_argument != argument || selector.ty != ty {
                    return Err(execution_call_error_v29());
                }
                let first = ordinal;
                let mut scalar_component = false;
                while let Some(component) = layout.call_arguments.get(ordinal) {
                    budget.charge_work(4)?;
                    if component.source_argument != selector.source_argument
                        || component.tuple_field != selector.tuple_field
                    {
                        break;
                    }
                    scalar_component = matches!(component.component, None | Some(0));
                    ordinal = argument_sum_v1(&[ordinal, 1])?;
                }
                // Zero-width and aggregate entries remain valid ABI arguments;
                // they simply have no scalar initializer parameter.
                if ordinal == argument_sum_v1(&[first, 1])? && scalar_component {
                    *parameters
                        .get_mut(local)
                        .ok_or_else(execution_call_error_v29)? = Some(first);
                }
            }
            if ordinal != body.parameters.len() {
                return Err(execution_call_error_v29());
            }
        } else {
            check_argument_function_abi_v1(
                function,
                row.function(),
                SemanticKirFunctionRoleV1::KernelEntry,
            )?;
            let semantic = plan.instances.owner().source_semantic();
            let inputs = function.abi().source_input_types();
            source_reference_owned_prepay_v29::<Vec<Option<usize>>>(plan, budget)?;
            let mut locals = emission_vec_v1(inputs.len(), budget)?;
            budget.charge_work(inputs.len())?;
            locals.resize(inputs.len(), None);
            for (index, declaration) in function.locals().iter().enumerate() {
                budget.charge_work(3)?;
                let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
                    continue;
                };
                let target = locals
                    .get_mut(argument as usize)
                    .ok_or_else(execution_call_error_v29)?;
                if target.replace(index).is_some()
                    || inputs.get(argument as usize) != Some(&declaration.ty())
                {
                    return Err(execution_call_error_v29());
                }
            }
            let mut ordinal = 0usize;
            for (argument, &ty) in inputs.iter().enumerate() {
                budget.charge_work(2)?;
                let local = locals[argument].ok_or_else(execution_call_error_v29)?;
                let argument =
                    u32::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                with_canonical_call_scratch_v1(budget, |budget| {
                    source_reference_owned_prepay_v29::<KernelParameterShapeV1>(plan, budget)?;
                    prepay_argument_shape_v1(semantic, ty, budget)?;
                    let shape = kernel_parameter_shape_v1(semantic, function, argument, ty)?;
                    match &shape {
                        KernelParameterShapeV1::Direct(expected) => {
                            budget.charge_work(3)?;
                            if lowered.function.signature.parameters.get(ordinal) != Some(expected)
                            {
                                return Err(execution_call_error_v29());
                            }
                            parameters[local] = Some(ordinal);
                            ordinal = argument_sum_v1(&[ordinal, 1])?;
                        }
                        KernelParameterShapeV1::Components(components) => {
                            for (_, _, expected, _, _) in components {
                                budget.charge_work(2)?;
                                if lowered.function.signature.parameters.get(ordinal)
                                    != Some(expected)
                                {
                                    return Err(execution_call_error_v29());
                                }
                                ordinal = argument_sum_v1(&[ordinal, 1])?;
                            }
                        }
                    }
                    Ok(())
                })?;
            }
            if ordinal != body.parameters.len()
                || ordinal != lowered.function.signature.parameters.len()
            {
                return Err(execution_call_error_v29());
            }
        }
        Ok(())
    })?;
    Ok(parameters)
}

fn source_entry_parameter_from_roster_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    lowered: &LoweredFunctionResultV1,
    parameters: &[Option<usize>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ValueId, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    plan.charge(5, budget)?;
    let declaration = plan
        .instances
        .instance(instance)
        .and_then(|row| row.declaration().locals().get(local.index() as usize))
        .ok_or_else(execution_call_error_v29)?;
    let semantic = plan.instances.owner().source_semantic();
    let shape = semantic.types()[declaration.ty().index() as usize].shape();
    let represented = matches!(
        shape,
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
    ) || matches!(shape, SemanticTypeShapeV1::Pointer(_))
        && private_retained_slot_facts_v1(semantic.types(), declaration.ty(), budget)
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
            .is_some();
    if lowered.source_call_instance != Some(instance)
        || !declaration.role().is_entry_argument()
        || !represented
    {
        return Err(execution_call_error_v29());
    }
    let ordinal = parameters
        .get(local.index() as usize)
        .copied()
        .flatten()
        .ok_or_else(execution_call_error_v29)?;
    lowered
        .function
        .body
        .as_ref()
        .and_then(|body| body.parameters.get(ordinal))
        .copied()
        .ok_or_else(execution_call_error_v29)
}

impl<'kir> SourceEntryPrologueV29<'kir> {
    // Caller owns a unit-return scratch scope. Every owned field dies inside it.
    fn new(
        plan: &'kir SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        lowered: &'kir LoweredFunctionResultV1,
        first: PrivateArrayPhysicalLocationV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let result = (|| {
            plan.check_owner(plan.instances, budget)?;
            source_reference_owned_prepay_v29::<Self>(plan, budget)?;
            // The caller's owned optional slot and reusable query envelopes stay
            // prepaid until this lexical context is dropped. Lookups reserve none.
            source_reference_owned_prepay_v29::<Option<Self>>(plan, budget)?;
            source_reference_owned_prepay_v29::<SourceEntryQueryV29<'_, '_>>(plan, budget)?;
            source_reference_owned_prepay_v29::<ValueId>(plan, budget)?;
            source_reference_owned_prepay_v29::<&ScopedMemoryAnchorV29>(plan, budget)?;
            source_reference_owned_prepay_v29::<()>(plan, budget)?;
            let row = plan
                .instances
                .instance(instance)
                .ok_or_else(execution_call_error_v29)?;
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            let entry = body.blocks.first().ok_or_else(execution_call_error_v29)?;
            let span = scoped_slot_prologue_v29(
                lowered,
                plan.instances
                    .instance(plan.instances.root())
                    .ok_or_else(execution_call_error_v29)?
                    .function(),
                row.function(),
                entry.id,
                budget,
            )?
            .ok_or_else(scoped_slot_error_v29)?;
            let end = span.operation_count as usize;
            if first.block_ordinal != 0
                || first.block != entry.id
                || first.operation >= end
                || end > entry.operations.len()
            {
                return Err(scoped_slot_error_v29());
            }
            let parameters = source_entry_parameter_roster_v29(plan, instance, lowered, budget)?;
            let source = lowered
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            let anchors =
                source_entry_anchor_index_v29(first, end, &entry.operations, &source.rows, budget)?;
            Ok(Self {
                lowered,
                instance,
                plan: &plan.slot,
                ledger: budget.work_ledger_identity_v1(),
                floor: budget.storage(),
                block: first.block,
                first: first.operation,
                parameters,
                anchors,
            })
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
    }

    fn check(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        lowered: &LoweredFunctionResultV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        plan.check_owner(plan.instances, budget)?;
        if self.ledger != budget.work_ledger_identity_v1() || budget.storage() < self.floor {
            plan.failure.record_resource(ArgumentResourceV1::Accounting);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        plan.charge(6, budget)?;
        if self.instance != instance
            || !std::ptr::eq(self.lowered, lowered)
            || !std::ptr::eq(self.plan, &plan.slot)
        {
            return Err(execution_call_error_v29());
        }
        Ok(())
    }

    fn anchor<'a>(
        &self,
        plan: &SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        lowered: &'a LoweredFunctionResultV1,
        location: PrivateArrayPhysicalLocationV1,
        legacy_pointer: Option<ValueId>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a ScopedMemoryAnchorV29, ProductionSemanticKirErrorV1> {
        self.check(plan, instance, lowered, budget)?;
        let error: fn() -> ProductionSemanticKirErrorV1 = if legacy_pointer.is_some() {
            scoped_memory_error_v29
        } else {
            scoped_object_error_v29
        };
        plan.charge(7, budget)?;
        if location.block_ordinal != 0 || location.block != self.block {
            return Err(error());
        }
        let offset = location
            .operation
            .checked_sub(self.first)
            .ok_or_else(error)?;
        let row = self.anchors.get(offset).ok_or_else(error)?;
        let (index, duplicate) = if legacy_pointer.is_some() {
            (row.access, row.access_duplicate)
        } else {
            (row.any, row.any_duplicate)
        };
        if duplicate {
            return Err(error());
        }
        let anchor = lowered
            .scoped_memory_anchors
            .as_ref()
            .and_then(|source| source.rows.get(index?))
            .ok_or_else(error)?;
        if legacy_pointer.is_some_and(|expected|
            !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Access { pointer, .. } if pointer == expected))
        {
            return Err(error());
        }
        Ok(anchor)
    }
}

#[derive(Clone, Copy)]
enum SourceEntryQueryV29<'context, 'kir> {
    Prologue(&'context SourceEntryPrologueV29<'kir>),
    #[cfg(test)]
    OneShot,
}

impl SourceEntryQueryV29<'_, '_> {
    fn parameter(
        self,
        plan: &SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        lowered: &LoweredFunctionResultV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        match self {
            Self::Prologue(context) => {
                context.check(plan, instance, lowered, budget)?;
                source_entry_parameter_from_roster_v29(
                    plan,
                    instance,
                    local,
                    lowered,
                    &context.parameters,
                    budget,
                )
            }
            #[cfg(test)]
            Self::OneShot => {
                source_reference_cell_initial_parameter_v29(plan, instance, local, lowered, budget)
            }
        }
    }

    fn object_anchor<'a>(
        self,
        plan: &SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        lowered: &'a LoweredFunctionResultV1,
        location: PrivateArrayPhysicalLocationV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a ScopedMemoryAnchorV29, ProductionSemanticKirErrorV1> {
        match self {
            Self::Prologue(context) => {
                context.anchor(plan, instance, lowered, location, None, budget)
            }
            #[cfg(test)]
            Self::OneShot => {
                let anchors = lowered
                    .scoped_memory_anchors
                    .as_ref()
                    .ok_or_else(scoped_object_error_v29)?;
                budget.charge_work(argument_product_v1(anchors.rows.len(), 3)?)?;
                let mut matching = anchors.rows.iter().filter(|row| {
                    row.block == location.block && row.position == location.operation
                });
                let anchor = matching.next().ok_or_else(scoped_object_error_v29)?;
                if matching.next().is_some() {
                    return Err(scoped_object_error_v29());
                }
                Ok(anchor)
            }
        }
    }
}

#[cfg(test)]
fn source_reference_cell_initial_parameter_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    lowered: &LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ValueId, ProductionSemanticKirErrorV1> {
    let mut found = None;
    with_canonical_call_scratch_v1(budget, |budget| {
        source_reference_owned_prepay_v29::<ValueId>(plan, budget)?;
        let parameters = source_entry_parameter_roster_v29(plan, instance, lowered, budget)?;
        found = Some(source_entry_parameter_from_roster_v29(
            plan,
            instance,
            local,
            lowered,
            &parameters,
            budget,
        )?);
        Ok(())
    })
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    found.ok_or_else(execution_call_error_v29)
}
