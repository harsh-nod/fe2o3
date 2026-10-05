// Source-to-actual structural transport only. Each leaf still requires its
// conditional guard, bounds, initialization, alias and effect obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionActualNodeV30 {
    original: SourceReferenceSelectionNodeV29,
    pointer: ValueId,
    element: ScalarType,
    space: AddressSpace,
    access: AccessMode,
    parameter: Option<(BlockId, usize)>,
    invocation: Option<SourceReferenceSelectionActualInvocationV30>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionActualInvocationV30 {
    input: usize,
    source: BlockId,
    original: ValueId,
    argument: Option<ValueId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionActualEdgeV30 {
    original: SourceReferenceSelectionEdgeV29,
    source: BlockId,
    ordinal: usize,
    target: BlockId,
    parameter: usize,
    argument: Option<ValueId>,
}

struct SourceReferenceSelectionActualV30 {
    nodes: Vec<SourceReferenceSelectionActualNodeV30>,
    edges: Vec<SourceReferenceSelectionActualEdgeV30>,
}

#[derive(Clone, Copy)]
struct SourceReferenceSelectionControlV30 {
    instance: ProductionCallInstanceIdV1,
    source: SemanticBlockIdV1,
    entry: BlockId,
    terminal: BlockId,
}

#[derive(Clone, Copy)]
struct SourceReferenceSelectionIncomingV30 {
    target: BlockId,
    parameter: usize,
    source: BlockId,
    ordinal: usize,
    edge: usize,
}

#[derive(Clone, Copy)]
struct SourceReferenceSelectionInvocationScopeV30 {
    instance: ProductionCallInstanceIdV1,
    preheader: BlockId,
    source: BlockId,
    position: u32,
}

impl SourceReferenceSelectionIncomingV30 {
    fn key(self) -> (BlockId, usize, BlockId, usize) {
        (self.target, self.parameter, self.source, self.ordinal)
    }
}

fn source_reference_selection_actual_error_v30() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("reference selection differs from its exact emitted SSA edges")
}

fn source_reference_selection_actual_headers_v30() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_emission_headers_v29::<SourceReferenceSelectionActualV30>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionActualNodeV30>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionActualEdgeV30>()?,
        source_reference_emission_headers_v29::<Option<SourceReferenceSelectionActualInvocationV30>>(
        )?,
        source_reference_emission_headers_v29::<InvocationArgumentRowV1>()?,
        source_reference_emission_headers_v29::<InvocationComponentRowV1>()?,
        source_reference_emission_headers_v29::<ScopedEmittedPointsV29<'_, '_, '_>>()?,
        source_reference_emission_headers_v29::<Option<(BlockId, u32)>>()?,
        std::mem::size_of::<Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29>>(),
        source_reference_emission_headers_v29::<Vec<SourceReferenceSelectionControlV30>>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionControlV30>()?,
        source_reference_emission_headers_v29::<Vec<SourceReferenceSelectionIncomingV30>>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionIncomingV30>()?,
        source_reference_emission_headers_v29::<(BlockId, usize, BlockId, usize)>()?,
        source_reference_emission_headers_v29::<Vec<(usize, u32, usize)>>()?,
        source_reference_emission_headers_v29::<(usize, u32, usize)>()?,
        source_reference_emission_headers_v29::<Vec<SourceReferenceSelectionInvocationScopeV30>>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionInvocationScopeV30>()?,
        source_reference_emission_headers_v29::<Vec<(BlockId, usize)>>()?,
        source_reference_emission_headers_v29::<Vec<(ValueId, BlockId, usize)>>()?,
        source_reference_emission_headers_v29::<Vec<usize>>()?,
        source_reference_emission_headers_v29::<&SemanticValueBindingV1>()?,
        source_reference_emission_headers_v29::<Option<(ScalarType, AddressSpace, AccessMode)>>()?,
        source_reference_emission_headers_v29::<Option<(BlockId, usize)>>()?,
        source_reference_emission_headers_v29::<&SourceIssuedActualV29<'_>>()?,
        source_reference_emission_headers_v29::<&Function>()?,
        source_reference_emission_headers_v29::<&Terminator>()?,
        source_reference_emission_headers_v29::<Type>()?,
        source_reference_emission_headers_v29::<usize>()?,
        source_reference_emission_headers_v29::<bool>()?,
        source_reference_emission_headers_v29::<()>()?,
        std::mem::size_of::<fe2o3_kernel_ir::FunctionControlFlowParameterV1>(),
        std::mem::size_of::<fe2o3_kernel_ir::FunctionControlFlowParameterInputV1>(),
        std::mem::size_of::<
            Result<
                fe2o3_kernel_ir::FunctionControlFlowParameterV1,
                fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1,
            >,
        >(),
        std::mem::size_of::<
            Result<
                fe2o3_kernel_ir::FunctionControlFlowParameterInputV1,
                fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1,
            >,
        >(),
        std::mem::size_of::<Result<(), fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>>(),
    ])
}

// Source ordinals are not physical ordinals for a false-valued Boolean case
// or assertion. The caller separately authenticates both terminator owners.
fn source_reference_selection_edge_ordinal_v30(
    types: &[SemanticTypeDeclV1],
    original: &SemanticTerminatorKindV1,
    role: SemanticEdgeRoleV1,
    ordinal: usize,
    actual: &Terminator,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    match (original, actual) {
        (SemanticTerminatorKindV1::Goto(_), Terminator::Branch { .. })
            if role == SemanticEdgeRoleV1::Goto && ordinal == 0 =>
        {
            Ok(0)
        }
        (SemanticTerminatorKindV1::Call(call), Terminator::Branch { .. })
            if role == SemanticEdgeRoleV1::CallReturn
                && ordinal == 0
                && call.destination().is_some() =>
        {
            Ok(0)
        }
        (
            SemanticTerminatorKindV1::Assert { expected, .. },
            Terminator::ConditionalBranch { .. },
        ) if role == SemanticEdgeRoleV1::AssertSuccess && ordinal == 0 => {
            Ok(usize::from(!*expected))
        }
        (SemanticTerminatorKindV1::Assert { .. }, Terminator::Branch { .. })
            if role == SemanticEdgeRoleV1::AssertSuccess && ordinal == 0 =>
        {
            Ok(0)
        }
        (
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            },
            actual,
        ) => {
            let boolean = lower_scalar_type(types, discriminant.ty())? == Type::BOOL;
            if (ordinal < targets.values().len() && role != SemanticEdgeRoleV1::SwitchValue)
                || (ordinal == targets.values().len()
                    && role != SemanticEdgeRoleV1::SwitchOtherwise)
                || ordinal > targets.values().len()
            {
                return Err(source_reference_selection_actual_error_v30());
            }
            if boolean {
                let [case] = targets.values() else {
                    return Err(source_reference_selection_actual_error_v30());
                };
                if case.value() > 1 || !matches!(actual, Terminator::ConditionalBranch { .. }) {
                    return Err(source_reference_selection_actual_error_v30());
                }
                Ok(if case.value() == 1 {
                    ordinal
                } else {
                    1 - ordinal
                })
            } else if matches!(actual, Terminator::Switch { cases, .. } if cases.len() == targets.values().len())
            {
                Ok(ordinal)
            } else {
                Err(source_reference_selection_actual_error_v30())
            }
        }
        _ => Err(source_reference_selection_actual_error_v30()),
    }
}

fn source_reference_selection_control_index_v30(
    controls: &[InstanceControlV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceReferenceSelectionControlV30>, ProductionSemanticKirErrorV1> {
    let mut rows = Vec::new();
    for control in controls {
        budget.charge_work(4)?;
        let Some(source) = control.semantic_block else {
            continue;
        };
        if !matches!(
            control.origin,
            InstanceControlOriginV1::Retained | InstanceControlOriginV1::ExpandedReturn { .. }
        ) {
            continue;
        }
        emission_push_v1(
            &mut rows,
            SourceReferenceSelectionControlV30 {
                instance: control.instance,
                source,
                entry: control.original_block,
                terminal: control.physical_block,
            },
            budget,
        )?;
    }
    call_splice_sort_work_v1(argument_product_v1(rows.len(), 2)?, budget)
        .map_err(source_address_call_error_v29)?;
    rows.sort_unstable_by_key(|row| (row.instance.index(), row.source.index()));
    for pair in rows.windows(2) {
        budget.charge_work(2)?;
        if (pair[0].instance, pair[0].source) == (pair[1].instance, pair[1].source) {
            return Err(source_reference_selection_actual_error_v30());
        }
    }
    Ok(rows)
}

fn source_reference_selection_controls_v30(
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &SourceAddressSourceIndexV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceReferenceSelectionControlV30>, ProductionSemanticKirErrorV1> {
    let rows = source_reference_selection_control_index_v30(
        &index.pending.coordinates.controls.rows,
        budget,
    )?;
    if rows.len() != index.terminators.len() {
        return Err(source_reference_selection_actual_error_v30());
    }
    for (row, original) in rows.iter().zip(&index.terminators) {
        budget.charge_work(5)?;
        if plan.instances.id_at(original.instance) != Some(row.instance)
            || row.source.index() != original.block
            || row.entry != original.span.kernel_ir_block
        {
            return Err(source_reference_selection_actual_error_v30());
        }
    }
    Ok(rows)
}

fn source_reference_selection_incoming_index_v30(
    edges: &[SourceReferenceSelectionActualEdgeV30],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceReferenceSelectionIncomingV30>, ProductionSemanticKirErrorV1> {
    let mut rows = emission_vec_v1(edges.len(), budget)?;
    for (edge, original) in edges.iter().enumerate() {
        budget.charge_work(5)?;
        rows.push(SourceReferenceSelectionIncomingV30 {
            target: original.target,
            parameter: original.parameter,
            source: original.source,
            ordinal: original.ordinal,
            edge,
        });
    }
    call_splice_sort_work_v1(argument_product_v1(rows.len(), 4)?, budget)
        .map_err(source_address_call_error_v29)?;
    rows.sort_unstable_by_key(|row| row.key());
    for pair in rows.windows(2) {
        budget.charge_work(4)?;
        if pair[0].key() == pair[1].key() {
            return Err(source_reference_selection_actual_error_v30());
        }
    }
    Ok(rows)
}

fn source_reference_selection_control_v30(
    rows: &[SourceReferenceSelectionControlV30],
    instance: ProductionCallInstanceIdV1,
    source: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceSelectionControlV30, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(rows.len(), budget)?;
    let index = rows
        .binary_search_by_key(&(instance.index(), source.index()), |row| {
            (row.instance.index(), row.source.index())
        })
        .map_err(|_| source_reference_selection_actual_error_v30())?;
    Ok(rows[index])
}

fn source_reference_selection_bind_v30(
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    graph: &SourceReferenceSelectionGraphV29,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceSelectionActualV30, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    if !std::ptr::eq(plan.instances.owner(), source_index.owner)
        || source_index.slot != std::ptr::from_ref(&*budget) as usize
        || source_index.ledger != budget.work_ledger_identity_v1()
        || budget.storage() < source_index.required
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.reserve_storage(source_reference_selection_actual_headers_v30()?)?;
    let function = &source_index.pending.function;
    let body = function
        .body
        .as_ref()
        .ok_or_else(source_reference_selection_actual_error_v30)?;
    let controls = source_reference_selection_controls_v30(plan, source_index, budget)?;
    let mut invocation_arguments = Vec::new();
    let mut invocation_scopes = Vec::new();
    for sidecar in &source_index.pending.sidecars.rows {
        budget.charge_work(2)?;
        let Some(entry) = &sidecar.invocation_entry else {
            continue;
        };
        let instance = sidecar
            .source_call_instance
            .ok_or_else(source_reference_selection_actual_error_v30)?;
        let preheader =
            source_address_invocation_entry_v29(plan.instances, instance, sidecar, budget)?;
        let gap = entry
            .span
            .first_operation_ordinal
            .checked_add(entry.span.operation_count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let mut mapping = ScopedEmittedPointsV29 {
            coordinates: &source_index.pending.coordinates,
            relocation: &source_index.pending.slot_relocation,
            budget,
        };
        let (source, position) = mapping
            .emitted_point(instance, preheader, gap, true)
            .map_err(source_address_point_error_v29)?
            .ok_or_else(source_reference_selection_actual_error_v30)?;
        emission_push_v1(
            &mut invocation_scopes,
            SourceReferenceSelectionInvocationScopeV30 {
                instance,
                preheader,
                source,
                position,
            },
            budget,
        )?;
        for (ordinal, row) in entry.arguments.iter().enumerate() {
            emission_push_v1(
                &mut invocation_arguments,
                (instance.index(), row.original.variable().get(), ordinal),
                budget,
            )?;
        }
    }
    call_splice_sort_work_v1(
        argument_sum_v1(&[
            argument_product_v1(invocation_arguments.len(), 2)?,
            invocation_scopes.len(),
        ])?,
        budget,
    )
    .map_err(source_address_call_error_v29)?;
    invocation_arguments.sort_unstable_by_key(|row| (row.0, row.1));
    invocation_scopes.sort_unstable_by_key(|row| row.instance.index());
    for pair in invocation_arguments.windows(2) {
        budget.charge_work(2)?;
        if (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1) {
            return Err(source_reference_selection_actual_error_v30());
        }
    }
    for pair in invocation_scopes.windows(2) {
        budget.charge_work(1)?;
        if pair[0].instance == pair[1].instance {
            return Err(source_reference_selection_actual_error_v30());
        }
    }
    let mut parameters = Vec::new();
    let mut blocks = emission_vec_v1(body.blocks.len(), budget)?;
    for (index, block) in body.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        blocks.push((block.id, index));
        for (ordinal, parameter) in block.parameters.iter().enumerate() {
            emission_push_v1(&mut parameters, (parameter.id, block.id, ordinal), budget)?;
        }
    }
    call_splice_sort_work_v1(argument_sum_v1(&[parameters.len(), blocks.len()])?, budget)
        .map_err(source_address_call_error_v29)?;
    parameters.sort_unstable_by_key(|row| row.0);
    blocks.sort_unstable_by_key(|row| row.0);
    let mut nodes = emission_vec_v1(graph.nodes.len(), budget)?;
    let mut edges = emission_vec_v1(graph.edges.len(), budget)?;
    for original in &graph.nodes {
        budget.charge_work(8)?;
        let archive = source_index
            .sidecar(original.value.instance, budget)?
            .execution_observation
            .as_ref()
            .ok_or_else(source_reference_selection_actual_error_v30)?;
        let binding = archive.lookup_original_v29(
            plan.instances,
            original.value.instance,
            original.value.value,
            budget,
        )?;
        let SemanticValueBindingV1::Value { id: pointer, ty } = binding else {
            return Err(source_reference_selection_actual_error_v30());
        };
        let Some((element, space, access)) = source_issued_pointer_shape_v26(ty) else {
            return Err(source_reference_selection_actual_error_v30());
        };
        let types = plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(source_type)) = types
            .get(original.value.pointer_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(source_reference_selection_actual_error_v30());
        };
        if source_type.kind() != SemanticPointerKindV1::Reference
            || source_type.metadata() != SemanticPointerMetadataV1::None
            || lower_scalar_type(types, source_type.pointee())? != Type::Scalar(element)
            || !matches!(space, AddressSpace::Global | AddressSpace::Generic)
            || !matches!(
                (source_type.mutability(), access),
                (SemanticMutabilityV1::Immutable, AccessMode::ReadOnly)
                    | (SemanticMutabilityV1::Mutable, AccessMode::ReadWrite)
            )
            || actual.value(*pointer, budget)?.ty != ty
        {
            return Err(source_reference_selection_actual_error_v30());
        }
        let parameter =
            if let SourceReferenceSelectionStepV29::Parameter { block, .. } = original.step {
                charge_execution_cfg_lookup_v29(parameters.len(), budget)?;
                let index = parameters
                    .binary_search_by_key(pointer, |row| row.0)
                    .map_err(|_| source_reference_selection_actual_error_v30())?;
                let (_, target, ordinal) = parameters[index];
                let original_target = source_reference_selection_control_v30(
                    &controls,
                    original.value.instance,
                    SemanticBlockIdV1::from_index(block.get()),
                    budget,
                )?;
                if target != original_target.entry {
                    return Err(source_reference_selection_actual_error_v30());
                }
                Some((target, ordinal))
            } else {
                None
            };
        let invocation = match original.step {
            SourceReferenceSelectionStepV29::Parameter {
                block,
                variable,
                invocation: Some(input),
                ..
            } => {
                let source_instance = plan
                    .instances
                    .instance(original.value.instance)
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                let input_node = graph
                    .nodes
                    .get(input)
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                let sidecar = source_index.sidecar(original.value.instance, budget)?;
                charge_execution_cfg_lookup_v29(invocation_scopes.len(), budget)?;
                let scope = invocation_scopes
                    .binary_search_by_key(&original.value.instance.index(), |row| {
                        row.instance.index()
                    })
                    .map_err(|_| source_reference_selection_actual_error_v30())?;
                let scope = invocation_scopes[scope];
                let entry = sidecar
                    .invocation_entry
                    .as_ref()
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                budget.charge_work(12)?;
                charge_execution_cfg_lookup_v29(invocation_arguments.len(), budget)?;
                let argument = invocation_arguments
                    .binary_search_by_key(
                        &(original.value.instance.index(), variable.get()),
                        |row| (row.0, row.1),
                    )
                    .map_err(|_| source_reference_selection_actual_error_v30())?;
                let row = entry
                    .arguments
                    .get(invocation_arguments[argument].2)
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                if row.original.variable() != variable
                    || block.get() != source_instance.declaration().entry().index()
                    || input_node.value.instance != original.value.instance
                    || input_node.value.pointer_type != original.value.pointer_type
                    || row.original.value() != input_node.value.value
                    || row.component_count != 1
                    || entry.layout.preheader != Some(scope.preheader)
                    || !entry.inputs_retained
                {
                    return Err(source_reference_selection_actual_error_v30());
                }
                let component = entry
                    .components
                    .get(row.first_component)
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                let initial = archive.lookup_original_v29(
                    plan.instances,
                    original.value.instance,
                    input_node.value.value,
                    budget,
                )?;
                if !matches!(initial, SemanticValueBindingV1::Value { id, ty: initial_type }
                    if *id == component.original && initial_type == ty)
                    || component.parameter != *pointer
                    || component.transported != component.original
                    || component.conversion.is_some()
                {
                    return Err(source_reference_selection_actual_error_v30());
                }
                let SourceReferenceSelectionInvocationScopeV30 {
                    source, position, ..
                } = scope;
                charge_execution_cfg_lookup_v29(blocks.len(), budget)?;
                let index = blocks
                    .binary_search_by_key(&source, |row| row.0)
                    .map_err(|_| source_reference_selection_actual_error_v30())?;
                let actual_block = &body.blocks[blocks[index].1];
                let (target, parameter) =
                    parameter.ok_or_else(source_reference_selection_actual_error_v30)?;
                if position as usize != actual_block.operations.len()
                    || !matches!(&actual_block.terminator,
                        Some(Terminator::Branch { target: actual_target, arguments })
                            if *actual_target == target
                                && arguments.get(parameter) == Some(&component.transported))
                {
                    return Err(source_reference_selection_actual_error_v30());
                }
                Some(SourceReferenceSelectionActualInvocationV30 {
                    input,
                    source,
                    original: component.original,
                    argument: None,
                })
            }
            _ => None,
        };
        nodes.push(SourceReferenceSelectionActualNodeV30 {
            original: *original,
            pointer: *pointer,
            element,
            space,
            access,
            parameter,
            invocation,
        });
        if let SourceReferenceSelectionStepV29::CallArgument {
            local,
            source_argument,
            ..
        } = original.step
        {
            check_source_reference_parameter_transport_v29(
                source_index,
                original.value.instance,
                local,
                source_argument,
                original.value.pointer_type,
                budget,
            )?;
        }
        let SourceReferenceSelectionStepV29::Parameter { first, count, .. } = original.step else {
            continue;
        };
        if first != edges.len() {
            return Err(source_reference_selection_actual_error_v30());
        }
        let end = argument_sum_v1(&[first, count])?;
        let (target, parameter) =
            parameter.ok_or_else(source_reference_selection_actual_error_v30)?;
        for edge in graph
            .edges
            .get(first..end)
            .ok_or_else(source_reference_selection_actual_error_v30)?
        {
            let source_block = SemanticBlockIdV1::from_index(edge.edge.source().get());
            let control = source_reference_selection_control_v30(
                &controls,
                original.value.instance,
                source_block,
                budget,
            )?;
            let original_terminator = plan
                .instances
                .instance(original.value.instance)
                .and_then(|instance| {
                    instance
                        .declaration()
                        .blocks()
                        .get(source_block.index() as usize)
                })
                .ok_or_else(source_reference_selection_actual_error_v30)?
                .terminator()
                .kind();
            charge_execution_cfg_lookup_v29(blocks.len(), budget)?;
            let block = blocks
                .binary_search_by_key(&control.terminal, |row| row.0)
                .map_err(|_| source_reference_selection_actual_error_v30())?;
            let terminator = body.blocks[blocks[block].1]
                .terminator
                .as_ref()
                .ok_or_else(source_reference_selection_actual_error_v30)?;
            let ordinal = source_reference_selection_edge_ordinal_v30(
                types,
                original_terminator,
                edge.role,
                edge.edge.ordinal() as usize,
                terminator,
                budget,
            )?;
            edges.push(SourceReferenceSelectionActualEdgeV30 {
                original: *edge,
                source: control.terminal,
                ordinal,
                target,
                parameter,
                argument: None,
            });
        }
    }
    if edges.len() != graph.edges.len() {
        return Err(source_reference_selection_actual_error_v30());
    }
    Ok(SourceReferenceSelectionActualV30 { nodes, edges })
}

fn source_reference_selection_check_transport_v30(
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    bound: &mut SourceReferenceSelectionActualV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let incoming_index = source_reference_selection_incoming_index_v30(&bound.edges, budget)?;
    let queries = argument_sum_v1(&[
        argument_product_v1(bound.nodes.len(), 2)?,
        bound.edges.len(),
    ])?;
    source_issued_pointer_walk_quote_v26(actual.values.len(), queries, budget)?;
    let mut comparisons = argument_product_v1(bound.nodes.len(), 12)?;
    for node in &bound.nodes {
        budget.charge_work(6)?;
        if source_issued_pointer_shape_v26(actual.value(node.pointer, budget)?.ty)
            != Some((node.element, node.space, node.access))
            || !matches!(node.space, AddressSpace::Global | AddressSpace::Generic)
            || node.access == AccessMode::WriteOnly
        {
            return Err(source_reference_selection_actual_error_v30());
        }
        match node.original.step {
            SourceReferenceSelectionStepV29::Pending => {
                return Err(source_reference_selection_actual_error_v30());
            }
            SourceReferenceSelectionStepV29::Leaf(_) => {
                if node.parameter.is_some() || node.invocation.is_some() {
                    return Err(source_reference_selection_actual_error_v30());
                }
            }
            SourceReferenceSelectionStepV29::Alias { input, .. }
            | SourceReferenceSelectionStepV29::CallArgument { input, .. } => {
                if node.parameter.is_some()
                    || node.invocation.is_some()
                    || input >= bound.nodes.len()
                {
                    return Err(source_reference_selection_actual_error_v30());
                }
            }
            SourceReferenceSelectionStepV29::Parameter {
                first,
                count,
                invocation,
                ..
            } => {
                let end = argument_sum_v1(&[first, count])?;
                let (target, parameter) = node
                    .parameter
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                let edges = bound
                    .edges
                    .get(first..end)
                    .ok_or_else(source_reference_selection_actual_error_v30)?;
                budget.charge_work(argument_product_v1(edges.len(), 6)?)?;
                let incoming = argument_sum_v1(&[count, usize::from(invocation.is_some())])?;
                if incoming == 0
                    || node.invocation.map(|row| row.input) != invocation
                    || node.invocation.is_some_and(|row| {
                        row.input >= bound.nodes.len()
                            || row.argument.is_some()
                            || row.original != bound.nodes[row.input].pointer
                    })
                    || edges.iter().any(|edge| {
                        edge.target != target
                            || edge.parameter != parameter
                            || edge.original.input >= bound.nodes.len()
                            || edge.argument.is_some()
                            || node
                                .invocation
                                .is_some_and(|row| row.source == edge.source && edge.ordinal == 0)
                    })
                {
                    return Err(source_reference_selection_actual_error_v30());
                }
                comparisons = argument_sum_v1(&[
                    comparisons,
                    argument_product_v1(
                        incoming,
                        argument_sum_v1(&[
                            24,
                            argument_product_v1(
                                call_splice_search_work_v1(incoming_index.len()),
                                4,
                            )?,
                        ])?,
                    )?,
                ])?;
            }
        }
    }
    // The CFG view owns the only live ledger. Prepay the fixed record matching
    // done alongside its separately metered edge and pointer-origin queries.
    budget.charge_work(comparisons)?;
    let mut valid = true;
    fe2o3_kernel_ir::with_function_control_flow_v1(function, Default::default(), budget, |view| {
        for node_index in 0..bound.nodes.len() {
            let node = bound.nodes[node_index];
            match node.original.step {
                SourceReferenceSelectionStepV29::Pending => valid = false,
                SourceReferenceSelectionStepV29::Leaf(_) => {}
                SourceReferenceSelectionStepV29::Alias { input, .. }
                | SourceReferenceSelectionStepV29::CallArgument { input, .. } => {
                    let expected = bound.nodes[input].pointer;
                    valid &= node.pointer == expected
                        || source_reference_pointer_transport_until_v29(
                            actual,
                            node.pointer,
                            expected,
                            view,
                        )? == Some(expected);
                }
                SourceReferenceSelectionStepV29::Parameter { first, count, .. } => {
                    let (target, parameter) = node.parameter.expect("checked parameter locator");
                    let actual_parameter = view.block_parameter(target, parameter)?;
                    if actual_parameter.value != node.pointer
                        || actual_parameter.incoming_count
                            != count + usize::from(node.invocation.is_some())
                    {
                        valid = false;
                        continue;
                    }
                    for incoming in 0..actual_parameter.incoming_count {
                        let row = view.block_parameter_input(target, parameter, incoming)?;
                        if let Some(invocation) = node.invocation
                            && row.source == invocation.source
                            && row.source_ordinal == 0
                        {
                            let already = bound.nodes[node_index]
                                .invocation
                                .expect("checked invocation locator")
                                .argument;
                            if already.is_some()
                                || !row.source_reachable
                                || row.target != target
                                || row.parameter_ordinal != parameter
                                || row.parameter != node.pointer
                            {
                                valid = false;
                                continue;
                            }
                            let expected = bound.nodes[invocation.input].pointer;
                            valid &= row.argument == invocation.original
                                && (row.argument == expected
                                    || source_reference_pointer_transport_until_v29(
                                        actual,
                                        row.argument,
                                        expected,
                                        view,
                                    )? == Some(expected));
                            bound.nodes[node_index]
                                .invocation
                                .as_mut()
                                .expect("checked invocation locator")
                                .argument = Some(row.argument);
                            continue;
                        }
                        let Ok(index) = incoming_index.binary_search_by_key(
                            &(target, parameter, row.source, row.source_ordinal),
                            |row| row.key(),
                        ) else {
                            valid = false;
                            continue;
                        };
                        let index = incoming_index[index].edge;
                        if index < first || index >= first + count {
                            valid = false;
                            continue;
                        }
                        let expected = &mut bound.edges[index];
                        if expected.argument.is_some()
                            || !row.source_reachable
                            || row.target != target
                            || row.parameter_ordinal != parameter
                            || row.parameter != node.pointer
                        {
                            valid = false;
                            continue;
                        }
                        let input = bound.nodes[expected.original.input].pointer;
                        valid &= row.argument == input
                            || source_reference_pointer_transport_until_v29(
                                actual,
                                row.argument,
                                input,
                                view,
                            )? == Some(input);
                        expected.argument = Some(row.argument);
                    }
                }
            }
        }
        Ok(())
    })
    .map_err(|error| match error {
        fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
        _ => source_reference_selection_actual_error_v30(),
    })?;
    budget.charge_work(argument_sum_v1(&[bound.nodes.len(), bound.edges.len()])?)?;
    if !valid
        || bound.edges.iter().any(|edge| edge.argument.is_none())
        || bound
            .nodes
            .iter()
            .any(|node| node.invocation.is_some_and(|row| row.argument.is_none()))
    {
        return Err(source_reference_selection_actual_error_v30());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn with_source_reference_selection_actual_v30<R>(
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    source: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &SourceReferenceSelectionGraphV29,
        &SourceReferenceSelectionActualV30,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_source_reference_selection_v29(
        plan,
        instance,
        site,
        role,
        source,
        budget,
        |graph, budget| {
            with_canonical_call_scratch_v1(budget, |budget| {
                budget.reserve_storage(argument_sum_v1(&[
                    source_reference_selection_call_headers_v29::<R>(&consume)?,
                    source_reference_emission_headers_v29::<&SourceReferenceSelectionGraphV29>()?,
                    source_reference_emission_headers_v29::<&SourceReferenceSelectionActualV30>()?,
                    source_reference_emission_headers_v29::<&SourceAddressSourceIndexV29<'_>>()?,
                ])?)?;
                let actual =
                    SourceIssuedActualV29::from_function(&source_index.pending.function, budget)?;
                let mut bound = source_reference_selection_bind_v30(
                    plan,
                    source_index,
                    graph,
                    &actual,
                    budget,
                )?;
                source_reference_selection_check_transport_v30(
                    &source_index.pending.function,
                    &actual,
                    &mut bound,
                    budget,
                )?;
                let retained = budget.storage();
                let result = consume(graph, &bound, budget);
                if !plan.retains_custody(plan.instances, budget) || budget.storage() != retained {
                    plan.failure.record_resource(ArgumentResourceV1::Accounting);
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                result
            })
        },
    )
}
