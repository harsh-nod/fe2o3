// Physical signatures for expanded instances. Nominal bindings arrive only via
// the scoped call seed; this plan is not producer or borrow authority.
fn execution_reference_abi_scalar_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<SemanticBackendScalarV1, ProductionSemanticKirErrorV1> {
    if execution_cfg_nominal_kind_v29(types, ty)? != Some(true) {
        return Err(execution_call_error_v29());
    }
    let declaration = &types[ty.index() as usize];
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Err(execution_call_error_v29());
    };
    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr() else {
        return Err(execution_call_error_v29());
    };
    if pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || declaration.layout().size_bytes() != Some(8)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
        || scalar.primitive() != SemanticBackendPrimitiveV1::pointer(0, 8, 8)
    {
        return Err(execution_call_error_v29());
    }
    Ok(*scalar)
}

fn execution_reference_parameter_v29(
    types: &[SemanticTypeDeclV1],
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(16)?;
    let argument = mapped.abi();
    if execution_cfg_nominal_kind_v29(types, argument.ty())? != Some(true) {
        return Ok(false);
    }
    let declaration = &types[argument.ty().index() as usize];
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Err(execution_call_error_v29());
    };
    let ownership = if mapped.tuple_field().is_some() {
        SemanticSourceArgumentOwnershipV1::ByValue
    } else {
        match pointer.mutability() {
            SemanticMutabilityV1::Immutable => SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticMutabilityV1::Mutable => SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        }
    };
    execution_reference_abi_scalar_v29(types, argument.ty())?;
    if mapped.source_ownership() != ownership
        || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
    {
        return Err(execution_call_error_v29());
    }
    Ok(true)
}

fn execution_parameter_is_nominal_v29(
    types: &[SemanticTypeDeclV1],
    mut ty: SemanticTypeIdV1,
    path: &[SemanticKirParameterProjectionV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    for projection in path {
        budget.charge_work(2)?;
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            return Ok(true);
        }
        ty = match (types[ty.index() as usize].shape(), projection) {
            (
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                SemanticKirParameterProjectionV1::Field(field),
            ) => *fields
                .fields()
                .get(*field as usize)
                .ok_or_else(execution_call_error_v29)?,
            (
                SemanticTypeShapeV1::Array { element, length },
                SemanticKirParameterProjectionV1::ArrayIndex(index),
            ) if u64::from(*index) < *length => *element,
            _ => return Err(execution_call_error_v29()),
        };
    }
    budget.charge_work(1)?;
    Ok(execution_cfg_nominal_kind_v29(types, ty)?.is_some())
}

fn check_execution_instance_abi_v29(
    semantic: &AdmittedInertSemanticMirV1,
    function_id: SemanticFunctionIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let function = &semantic.functions()[function_id.index() as usize];
    check_argument_function_abi_v1(
        function,
        function_id,
        SemanticKirFunctionRoleV1::InternalHelper,
    )?;
    let abi = function.abi();
    let sources = abi.source_input_types().len();
    let adjusted = abi.adjusted_arguments().len();
    let floor = budget.storage();
    let result = (|| {
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(function.locals().len(), 2)?,
            sources,
            adjusted,
        ])?)?;
        budget.reserve_storage(argument_sum_v1(&[
            argument_product_v1(sources, std::mem::size_of::<Option<SemanticLocalIdV1>>())?,
            argument_product_v1(adjusted, std::mem::size_of::<SemanticLocalIdV1>())?,
        ])?)?;
        let arguments =
            semantic
                .logical_arguments_v1(function_id)
                .map_err(|error| match error {
                    fe2o3_mir_model::SemanticLogicalArgumentErrorV1::AllocationFailure => {
                        ProductionSemanticKirErrorV1::from(ArgumentResourceV1::Allocation)
                    }
                    fe2o3_mir_model::SemanticLogicalArgumentErrorV1::UnknownFunction => {
                        execution_call_error_v29()
                    }
                })?;
        for argument in arguments.source_arguments() {
            budget.charge_work(1)?;
            if matches!(
                argument.binding(),
                fe2o3_mir_model::SemanticSourceArgumentBindingV1::ExpandedTuple([])
            ) && argument.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue
            {
                return Err(execution_call_error_v29());
            }
        }
        for mapped in arguments.adjusted_arguments() {
            if execution_reference_parameter_v29(semantic.types(), mapped, budget)? {
                continue;
            }
            let shape_floor = budget.storage();
            prepay_argument_shape_v1(semantic, mapped.abi().ty(), budget)?;
            let (_, shape) = helper_parameter_shape_with_policy_v1(
                semantic.types(),
                function,
                function_id,
                mapped,
                ParameterLeafPolicyV1::ExecutionAbiWords,
            )?;
            let physical = execution_cfg_types_v29(semantic.types(), mapped.abi().ty(), budget)?;
            let mut ordinary = physical.iter();
            for (path, _, expected) in &shape {
                if !execution_parameter_is_nominal_v29(
                    semantic.types(),
                    mapped.abi().ty(),
                    path,
                    budget,
                )? && ordinary.next() != Some(expected)
                {
                    return Err(execution_call_error_v29());
                }
            }
            if ordinary.next().is_some() {
                return Err(execution_call_error_v29());
            }
            drop((shape, physical));
            budget.release_storage(budget.storage() - shape_floor)?;
        }
        Ok(())
    })();
    // This checker returns no allocations; preserve the caller's retained floor.
    budget.release_storage(budget.storage() - floor)?;
    result
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Scope materializer integration remains gated")
)]
fn execution_instance_plan_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    kernel_ir_function: FunctionId,
    placement: SemanticEmissionPlacementV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = build_execution_instance_plan_v29(
        instances,
        instance,
        kernel_ir_function,
        placement,
        budget,
    );
    if result.is_err() {
        budget.release_storage(budget.storage() - floor)?;
    }
    result
}

fn build_execution_instance_plan_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    kernel_ir_function: FunctionId,
    placement: SemanticEmissionPlacementV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let row = instances
        .instance(instance)
        .ok_or_else(execution_call_error_v29)?;
    let incoming = instances
        .incoming(instance)
        .ok_or_else(execution_call_error_v29)?;
    if incoming.child() != Some(instance) {
        return Err(execution_call_error_v29());
    }
    let semantic = instances.owner().source_semantic();
    let function = row.declaration();
    check_execution_instance_abi_v29(semantic, row.function(), budget)?;
    let mut parameter_declarations = Vec::new();
    let mut parameter_types = Vec::new();
    let mut parameter_values = Vec::new();
    let mut call_arguments = Vec::new();
    let mut next_value = placement.first_value;
    // Entry-local order is also the scoped constructor's reconstruction order.
    // A packed RustCall tuple stays whole; expanded fields retain their selector.
    for (local, declaration) in function.locals().iter().enumerate() {
        budget.charge_work(1)?;
        if !declaration.role().is_entry_argument() {
            continue;
        }
        let selector = instances
            .parameter_source(
                instance,
                SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
                budget,
            )
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        emission_push_v1(
            &mut parameter_declarations,
            (selector.source_argument, local, selector.ty),
            budget,
        )?;
        let physical = execution_cfg_types_v29(semantic.types(), selector.ty, budget)?;
        let backing = argument_product_v1(physical.capacity(), std::mem::size_of::<Type>())?;
        for (component, ty) in physical.into_iter().enumerate() {
            let id = ValueId(next_value);
            next_value = next_value
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            emission_push_v1(&mut parameter_types, ty, budget)?;
            emission_push_v1(&mut parameter_values, id, budget)?;
            emission_push_v1(
                &mut call_arguments,
                HelperCallArgumentV1 {
                    source_argument: selector.source_argument,
                    tuple_field: selector.tuple_field,
                    component: Some(component),
                },
                budget,
            )?;
        }
        budget.release_storage(backing)?;
    }
    if execution_cfg_nominal_count_v29(
        semantic.types(),
        function.abi().source_output_type(),
        budget,
    )? != 0
    {
        return Err(execution_call_error_v29());
    }
    budget.charge_work(function.locals().len())?;
    let floor = budget.storage();
    prepay_typed_shape_v1(
        semantic.types(),
        function.abi().source_output_type(),
        0,
        budget,
    )?;
    let scratch = budget.storage() - floor;
    let result_types = (|| {
        let shape = helper_result_components_v1(semantic.types(), function, row.function())?;
        let mut result = emission_vec_v1(shape.components.len(), budget)?;
        for (_, _, ty, _, _) in &shape.components {
            result.push(execution_cfg_clone_type_v29(ty, budget)?);
        }
        Ok::<_, ProductionSemanticKirErrorV1>(result)
    })();
    budget.release_storage(scratch)?;
    Ok(LoweredFunctionPlanV1 {
        correspondence_owner: instances
            .instance(instances.root())
            .ok_or_else(execution_call_error_v29)?
            .function(),
        semantic_function: row.function(),
        kernel_ir_function,
        role: SemanticKirFunctionRoleV1::InternalHelper,
        parameter_declarations,
        parameter_types,
        parameter_values,
        call_arguments,
        parameter_local_bindings: Vec::new(),
        parameter_component_bindings: Vec::new(),
        ignored_parameter_bindings: Vec::new(),
        result_types: result_types?,
    })
}
