// Exact logical-to-physical argument planning for admitted ordinary helpers.

fn direct_scalar_helper_plan_v1(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    correspondence_owner: SemanticFunctionIdV1,
    function_id: SemanticFunctionIdV1,
    kernel_ir_function: FunctionId,
    max_argument_rows: usize,
    closure_budget: &mut ReachableClosureBudgetV1,
    call_budget: &mut ArgumentBudgetV1<'_>,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    let arguments = semantic
        .logical_arguments_v1(function_id)
        .map_err(|error| match error {
            fe2o3_mir_model::SemanticLogicalArgumentErrorV1::AllocationFailure => {
                ProductionSemanticKirErrorV1::AllocationFailure {
                    resource: ProductionSemanticKirResourceV1::DebugBindings,
                }
            }
            fe2o3_mir_model::SemanticLogicalArgumentErrorV1::UnknownFunction => unsupported(
                function_id.index(),
                None,
                None,
                "helper function is missing",
            ),
        })?;
    let function = &semantic.functions()[function_id.index() as usize];
    let types = semantic.types();
    check_argument_function_abi_v1(
        function,
        function_id,
        SemanticKirFunctionRoleV1::InternalHelper,
    )?;
    let abi = function.abi();
    for argument in arguments.source_arguments() {
        if matches!(
            argument.binding(),
            fe2o3_mir_model::SemanticSourceArgumentBindingV1::ExpandedTuple([])
        ) && argument.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue
        {
            return Err(unsupported(
                function_id.index(),
                None,
                None,
                "empty expanded helper argument lacks an exact by-value source ABI",
            ));
        }
    }
    let parameters = function
        .locals()
        .iter()
        .enumerate()
        .filter_map(|(local, declaration)| {
            let source = match declaration.role() {
                SemanticLocalRoleV1::Argument(source)
                | SemanticLocalRoleV1::RustCallTupleField {
                    argument: source, ..
                } => source,
                SemanticLocalRoleV1::Return | SemanticLocalRoleV1::Temporary => return None,
            };
            Some((source, local, declaration.ty()))
        })
        .collect::<Vec<_>>();
    let mut parameter_types = Vec::new();
    let mut parameter_values = Vec::new();
    let mut call_arguments = Vec::new();
    let mut parameter_component_bindings = Vec::new();
    let mut local_values = BTreeMap::<usize, Vec<ValueDef>>::new();
    let mut flattened = BTreeSet::new();
    let mut borrowed_shapes = BTreeMap::new();
    let mut borrowed_parameter_bindings = Vec::new();
    let mut next_value = u32::try_from(function.locals().len()).map_err(|_| {
        unsupported(
            function_id.index(),
            None,
            None,
            "helper local identity exceeds Kernel IR",
        )
    })?;
    for mapped in arguments.adjusted_arguments() {
        let local = mapped.local().index() as usize;
        if let Some(shape) = borrowed_aggregate_helper_parameter_v1(
            types,
            function,
            function_id,
            mapped,
            call_budget,
        )? {
            closure_budget.charge_parameter_expansion(
                logical_argument_rows_v1(function),
                parameter_types.len(),
                shape.leaves.len(),
                max_argument_rows,
            )?;
            let mut values = borrowed_aggregate_vec_v1(shape.leaves.len(), call_budget)?;
            for (component, leaf) in shape.leaves.iter().enumerate() {
                let ty = leaf.parameter_type(shape.access, call_budget)?;
                let value = if component == 0 {
                    ValueId(local as u32)
                } else {
                    let value = ValueId(next_value);
                    next_value = next_value
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    value
                };
                call_budget.reserve_storage(std::mem::size_of::<Type>())?;
                values.push(ValueDef::new(value, ty.clone()));
                borrowed_aggregate_push_shared_v1(&mut parameter_types, ty, call_budget)?;
                borrowed_aggregate_push_shared_v1(&mut parameter_values, value, call_budget)?;
                let path = leaf.path.clone_in(call_budget)?;
                let formal_path = path.clone_in(call_budget)?;
                borrowed_aggregate_push_v1(
                    &mut borrowed_parameter_bindings,
                    SemanticKirBorrowedParameterBindingV1 {
                        correspondence_owner,
                        semantic_function: function_id,
                        semantic_local: mapped.local(),
                        reference_type: shape.reference_type,
                        semantic_component_type: leaf.semantic_type,
                        projection: formal_path.fields,
                        transport: borrowed_parameter_transport_v1(leaf),
                        kernel_ir_value: value,
                    },
                    call_budget,
                )?;
                borrowed_aggregate_push_shared_v1(
                    &mut call_arguments,
                    HelperCallArgumentV1 {
                        source_argument: mapped.source_argument(),
                        tuple_field: None,
                        component: Some(component),
                        borrowed: Some(BorrowedAggregateFormalV1 {
                            local: mapped.local(),
                            path,
                            value,
                        }),
                    },
                    call_budget,
                )?;
            }
            call_budget.reserve_storage(
                std::mem::size_of::<BorrowedAggregateShapeV1>()
                    + std::mem::size_of::<Vec<ValueDef>>()
                    + 8 * std::mem::size_of::<usize>(),
            )?;
            local_values.insert(local, values);
            borrowed_shapes.insert(local, shape);
            continue;
        }
        let (shared_slice, components) =
            helper_parameter_shape_v1(types, function, function_id, mapped)?;
        closure_budget.charge_parameter_expansion(
            logical_argument_rows_v1(function),
            parameter_types.len(),
            components.len(),
            max_argument_rows,
        )?;
        parameter_types
            .try_reserve(components.len())
            .and_then(|_| parameter_values.try_reserve(components.len()))
            .and_then(|_| call_arguments.try_reserve(components.len()))
            .and_then(|_| parameter_component_bindings.try_reserve(components.len()))
            .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::DebugBindings,
            })?;
        let values = local_values.entry(local).or_default();
        values.try_reserve(components.len()).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::DebugBindings,
            }
        })?;
        if components.is_empty() {
            flattened.insert(local);
        }
        for (component, (path, semantic_component_type, parameter_ty)) in
            components.into_iter().enumerate()
        {
            let mut projection = Vec::new();
            projection
                .try_reserve_exact(path.len() + usize::from(mapped.local_field().is_some()))
                .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
                    resource: ProductionSemanticKirResourceV1::DebugBindings,
                })?;
            projection.extend(
                mapped
                    .local_field()
                    .map(SemanticKirParameterProjectionV1::Field),
            );
            projection.extend(path);
            let value = if values.is_empty() {
                ValueId(local as u32)
            } else {
                let value = ValueId(next_value);
                next_value = next_value.checked_add(1).ok_or_else(|| {
                    unsupported(
                        function_id.index(),
                        None,
                        None,
                        "helper parameter value identity overflow",
                    )
                })?;
                value
            };
            values.push(ValueDef::new(value, parameter_ty.clone()));
            if !projection.is_empty() {
                flattened.insert(local);
                parameter_component_bindings.push(SemanticKirParameterComponentBindingV1 {
                    correspondence_owner,
                    semantic_function: function_id,
                    semantic_local: mapped.local(),
                    semantic_component_type,
                    projection: projection.into_boxed_slice(),
                    kernel_ir_value: value,
                });
            }
            parameter_types.push(parameter_ty);
            parameter_values.push(value);
            call_arguments.push(HelperCallArgumentV1 {
                source_argument: mapped.source_argument(),
                tuple_field: mapped.tuple_field(),
                component: (!shared_slice).then_some(component),
                borrowed: None,
            });
        }
    }
    let mut parameter_local_bindings = Vec::new();
    let mut ignored_parameter_bindings = Vec::new();
    parameter_local_bindings
        .try_reserve_exact(parameters.len())
        .and_then(|_| ignored_parameter_bindings.try_reserve_exact(parameters.len()))
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::DebugBindings,
        })?;
    for (source_argument, local, semantic_type) in &parameters {
        let values = local_values.remove(local).unwrap_or_default();
        if values.is_empty() {
            if types[semantic_type.index() as usize].layout().size_bytes() != Some(0)
                || abi
                    .source_argument_ownership()
                    .get(*source_argument as usize)
                    != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
            {
                return Err(unsupported(
                    function_id.index(),
                    None,
                    None,
                    "helper entry local has no physical argument",
                ));
            }
            binding_from_value_defs(types, *semantic_type, &[])?;
            ignored_parameter_bindings.push(SemanticKirIgnoredParameterBindingV1 {
                correspondence_owner,
                semantic_function: function_id,
                semantic_local: SemanticLocalIdV1::from_index(*local as u32),
                semantic_type: *semantic_type,
            });
        }
        let binding = if let Some(shape) = borrowed_shapes.remove(local) {
            PlannedParameterLocalBindingV1::BorrowedAggregate {
                local: *local,
                shape,
                values,
            }
        } else if !flattened.contains(local) && values.len() == 1 {
            PlannedParameterLocalBindingV1::Direct {
                local: *local,
                value: values[0].id,
                ty: values[0].ty.clone(),
            }
        } else {
            PlannedParameterLocalBindingV1::Flattened {
                local: *local,
                semantic_type: *semantic_type,
                values,
            }
        };
        parameter_local_bindings.push(binding);
    }

    let result_types = helper_result_components_v1(types, function, function_id)?
        .components
        .into_iter()
        .map(|(_, _, ty, _, _)| ty)
        .collect();
    Ok(LoweredFunctionPlanV1 {
        borrowed_parameter_bindings,
        correspondence_owner,
        semantic_function: function_id,
        kernel_ir_function,
        role: SemanticKirFunctionRoleV1::InternalHelper,
        parameter_declarations: parameters,
        parameter_types,
        parameter_values,
        call_arguments,
        parameter_local_bindings,
        parameter_component_bindings,
        ignored_parameter_bindings,
        result_types,
    })
}
