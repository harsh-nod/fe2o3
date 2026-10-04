// This is only the closed representation relation. Original occurrence,
// current value and destination ownership are checked at each calling boundary.
fn source_descriptor_widening_v29(actual: &Type, expected: &Type) -> bool {
    matches!((actual, expected), (Type::Slice(actual), Type::Slice(expected))
        if actual.address_space != AddressSpace::Generic
            && expected.address_space == AddressSpace::Generic
            && actual.element == expected.element
            && actual.access == expected.access
            && (actual.address_space != AddressSpace::Constant || actual.access == AccessMode::ReadOnly))
}

fn source_descriptor_node_present_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    fn children(
        plan: &SourceReferencePlanV29<'_, '_>,
        node: usize,
        first: usize,
        count: usize,
        nodes: &mut usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        for field in 0..count {
            budget.source_reference_charge_v29(plan, 1)?;
            let child = *plan
                .children
                .get(argument_sum_v1(&[first, field])?)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if child >= node {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if source_descriptor_node_present_v29(plan, child, nodes, budget)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    if plan.descriptor_root.is_none() {
        return Ok(false);
    }
    execution_cfg_charge_node_v29(nodes, budget)?;
    let row = plan.nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
    if row.descriptor.is_some() {
        return Ok(true);
    }
    match row.kind {
        SourceReferenceNodeKindV29::Aggregate { first, count } => {
            return children(plan, node, first, count, nodes, budget);
        }
        SourceReferenceNodeKindV29::Enum { first, count } => {
            for offset in 0..count {
                budget.source_reference_charge_v29(plan, 3)?;
                let member = *plan
                    .enum_members
                    .get(argument_sum_v1(&[first, offset])?)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let alternative = plan
                    .enum_alternatives
                    .get(member)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if alternative.ty != row.ty {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                if let Some(source) = alternative.opaque {
                    budget.source_reference_charge_v29(plan, 2)?;
                    let original = plan
                        .nodes
                        .get(source)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if source >= node
                        || alternative.first != 0
                        || alternative.count != 0
                        || original.ty != row.ty
                        || original.inactive.is_some()
                        || !matches!(original.kind, SourceReferenceNodeKindV29::Plain(_))
                    {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    if source_descriptor_node_present_v29(plan, source, nodes, budget)? {
                        return Ok(true);
                    }
                    continue;
                }
                if children(
                    plan,
                    node,
                    alternative.first,
                    alternative.count,
                    nodes,
                    budget,
                )? {
                    return Ok(true);
                }
            }
        }
        SourceReferenceNodeKindV29::EnumView(view) => {
            budget.source_reference_charge_v29(plan, 2)?;
            let view = plan
                .enum_views
                .get(view)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if view.source >= node {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            return children(plan, node, view.children, view.child_count, nodes, budget);
        }
        SourceReferenceNodeKindV29::Absent
        | SourceReferenceNodeKindV29::Plain(_)
        | SourceReferenceNodeKindV29::Loan(_)
        | SourceReferenceNodeKindV29::Address(_)
        | SourceReferenceNodeKindV29::Discriminant(_) => {}
    }
    Ok(false)
}

fn source_descriptor_cfg_types_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    carriers: &ExecutionCfgCarriersV29,
    block: SemanticBlockIdV1,
    local: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    cursor.check_ledger(budget)?;
    let references = cursor.references.ok_or_else(execution_cfg_error_v29)?;
    let node = references
        .block_node(
            cursor.instance,
            block,
            SemanticLocalIdV1::from_index(local),
            budget,
        )?
        .ok_or_else(execution_cfg_error_v29)?;
    if let Some(carrier) = carriers.at(cursor, local, node, budget)? {
        return carrier.types(budget);
    }
    source_reference_cfg_node_types_v29(references.plan, node, budget)
}

#[derive(Clone, Copy)]
enum SourceDescriptorDestinationV29 {
    Edge {
        ordinal: u32,
        target: SemanticBlockIdV1,
        local: u32,
        definition: SsaValueV1,
        component: usize,
    },
    Invocation {
        local: u32,
        definition: SsaValueV1,
        component: usize,
    },
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn check_descriptor_call_argument_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        origin: &PreparedExecutionCallOriginV29<'_>,
        parameter: usize,
        projection: &HelperCallArgumentV1,
        binding: &SemanticValueBindingV1,
        value: ValueId,
        actual: &Type,
        expected: &Type,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this.execution.as_ref().ok_or_else(execution_call_error_v29)?;
            cursor.check_ledger(budget)?;
            let references = cursor.references.ok_or_else(execution_call_error_v29)?;
            references.check(budget)?;
            budget.charge_work(12)?;
            let original = cursor.function.blocks().get(block.index() as usize)
                .ok_or_else(execution_call_error_v29)?;
            if !matches!(original.terminator().kind(), SemanticTerminatorKindV1::Call(source) if std::ptr::eq(source, call))
                || origin.ledger != budget.work_ledger_identity_v1()
                || origin.scope.ledger != origin.ledger
                || origin.source != cursor.source
                || origin.function != cursor.function_id
                || origin.occurrence.caller != cursor.instance
                || origin.occurrence.block != block
                || origin.parameter_types.get(parameter) != Some(expected)
                || origin.projections.get(parameter).is_none_or(|original|
                    original.source_argument != projection.source_argument
                        || original.tuple_field != projection.tuple_field
                        || original.component != projection.component)
                || !source_descriptor_widening_v29(actual, expected)
            {
                return Err(execution_call_error_v29());
            }
            with_execution_cfg_values_and_references_v29(binding, Some(references), budget, |values, budget| {
                budget.charge_work(1)?;
                let component = projection.component.unwrap_or(0);
                if (projection.component.is_none() && values.len() != 1)
                    || values.get(component).is_none_or(|row| row.id != value || &row.ty != actual)
                {
                    return Err(execution_call_error_v29());
                }
                Ok(())
            })
        })
    }

    fn check_descriptor_return_v29(
        &mut self,
        block: SemanticBlockIdV1,
        local: usize,
        component: usize,
        input: ValueId,
        actual: &Type,
        expected: &Type,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            cursor.check_ledger(budget)?;
            let references = cursor.references.ok_or_else(execution_call_error_v29)?;
            references.check(budget)?;
            budget.charge_work(6)?;
            if !std::ptr::eq(cursor.function, this.function)
                || !matches!(
                    this.function
                        .blocks()
                        .get(block.index() as usize)
                        .map(|row| row.terminator().kind()),
                    Some(SemanticTerminatorKindV1::Return)
                )
                || this.function.locals().get(local).map(|row| row.role())
                    != Some(SemanticLocalRoleV1::Return)
                || !source_descriptor_widening_v29(actual, expected)
            {
                return Err(execution_call_error_v29());
            }
            let node = references
                .plan
                .returns
                .get(cursor.instance.index())
                .copied()
                .flatten()
                .ok_or_else(execution_call_error_v29)?;
            let types = source_reference_node_types_v29(references.plan, node, budget)?;
            if types.get(component) != Some(expected)
                || this.result_types.get(component) != Some(expected)
            {
                return Err(execution_call_error_v29());
            }
            let binding = this
                .locals
                .get(local)
                .and_then(Option::as_ref)
                .ok_or_else(execution_call_error_v29)?;
            source_reference_call_shape_v29(references, node, binding, budget)?;
            with_execution_cfg_values_and_references_v29(
                binding,
                Some(references),
                budget,
                |values, budget| {
                    budget.charge_work(1)?;
                    if values
                        .get(component)
                        .is_none_or(|value| value.id != input || &value.ty != actual)
                    {
                        return Err(execution_call_error_v29());
                    }
                    Ok(())
                },
            )
        })
    }

    fn check_descriptor_destination_v29(
        &mut self,
        block: SemanticBlockIdV1,
        destination: SourceDescriptorDestinationV29,
        value: ValueId,
        actual: &Type,
        expected: &Type,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(execution_cfg_error_v29)?;
            cursor.check_ledger(budget)?;
            let references = cursor.references.ok_or_else(execution_cfg_error_v29)?;
            references.check(budget)?;
            if !std::ptr::eq(cursor.function, this.function)
                || cursor.function_id != this.semantic_function
            {
                return Err(execution_cfg_error_v29());
            }
            let (arguments, target, local, definition, component) = match destination {
                SourceDescriptorDestinationV29::Edge {
                    ordinal,
                    target,
                    local,
                    definition,
                    component,
                } => {
                    let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(block.index()), ordinal);
                    let arguments = cursor
                        .ssa
                        .plan()
                        .edge_arguments(edge)
                        .ok_or_else(execution_cfg_error_v29)?;
                    // The existing cursor consumed/authenticated this exact edge
                    // before emission. Rejoin its original target, not a type.
                    budget.charge_work(cursor.cfg.edges.len())?;
                    if !cursor.cfg.edges.iter().any(|row| {
                        row.id == edge && row.target == target.index() as usize && row.claimed
                    }) {
                        return Err(execution_cfg_error_v29());
                    }
                    (arguments, target, local, definition, component)
                }
                SourceDescriptorDestinationV29::Invocation {
                    local,
                    definition,
                    component,
                } => {
                    if block != this.function.entry() || this.invocation_preheader.is_none() {
                        return Err(invocation_entry_error_v1());
                    }
                    (
                        cursor.ssa.plan().entry_arguments(),
                        block,
                        local,
                        definition,
                        component,
                    )
                }
            };
            budget.charge_work(arguments.len())?;
            if !arguments.iter().any(|argument| {
                argument.variable().get() == local && argument.value() == definition
            }) {
                return Err(execution_cfg_error_v29());
            }
            let types = source_descriptor_cfg_types_v29(
                cursor,
                &this.control_flow_ssa.cfg_carriers,
                target,
                local,
                budget,
            )?;
            if types.get(component) != Some(expected) {
                return Err(execution_cfg_error_v29());
            }
            charge_execution_cfg_lookup_v29(this.block_parameters.len(), budget)?;
            let parameters = this
                .block_parameters
                .get(&target.index())
                .ok_or_else(execution_cfg_error_v29)?;
            charge_execution_cfg_lookup_v29(parameters.len(), budget)?;
            if parameters
                .get(&local)
                .and_then(|values| values.get(component))
                .map(|value| &value.ty)
                != Some(expected)
            {
                return Err(execution_cfg_error_v29());
            }
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            let binding = this
                .semantic_ssa_bindings
                .get(&definition)
                .ok_or_else(execution_archive_error_v29)?;
            with_execution_cfg_local_values_v29(
                cursor,
                &this.control_flow_ssa.cfg_carriers,
                target,
                local,
                binding,
                budget,
                |values, budget| {
                    budget.charge_work(1)?;
                    // Thin references use the same source-typed representation
                    // relation as calls; this does not prove a memory origin.
                    let widening = source_descriptor_widening_v29(actual, expected)
                        || (component == 0
                            && values.len() == 1
                            && source_reference_call_widening_v26(
                                this.types,
                                cursor
                                    .function
                                    .locals()
                                    .get(local as usize)
                                    .ok_or_else(execution_cfg_error_v29)?
                                    .ty(),
                                actual,
                                expected,
                                budget,
                            )?);
                    if values
                        .get(component)
                        .is_none_or(|row| row.id != value || &row.ty != actual)
                        || !widening
                    {
                        return Err(execution_cfg_error_v29());
                    }
                    Ok(())
                },
            )
        })
    }
}
