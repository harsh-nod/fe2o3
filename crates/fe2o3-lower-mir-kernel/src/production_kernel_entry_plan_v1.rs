// The physical ABI planner is shared by ordinary and scoped roots. Nominal
// context admission is checked separately; this planner grants no capability.
fn kernel_entry_plan_v1(
    semantic: &AdmittedInertSemanticMirV1,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    kernel_ir_function: FunctionId,
    max_operations: usize,
    closure_budget: &mut ReachableClosureBudgetV1,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    kernel_entry_plan_with_shape_v1(
        semantic,
        correspondence_owner,
        semantic_function,
        kernel_ir_function,
        max_operations,
        closure_budget,
        |body, argument, _, ty| kernel_parameter_shape_v1(semantic, body, argument, ty),
    )
}

fn kernel_entry_plan_with_descriptors_v29(
    semantic: &AdmittedInertSemanticMirV1,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    kernel_ir_function: FunctionId,
    max_operations: usize,
    closure_budget: &mut ReachableClosureBudgetV1,
    references: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    if references.descriptor_root.is_none() {
        return kernel_entry_plan_with_shape_v1(
            semantic,
            correspondence_owner,
            semantic_function,
            kernel_ir_function,
            max_operations,
            closure_budget,
            |body, argument, _, ty| source_kernel_parameter_shape_v18(semantic, body, argument, ty),
        );
    }
    references.check_owner(references.instances, budget)?;
    if !std::ptr::eq(semantic, references.instances.owner().source_semantic())
        || references
            .instances
            .instance(references.root)
            .map(|row| row.function())
            != Some(semantic_function)
    {
        return Err(execution_call_error_v29());
    }
    kernel_entry_plan_with_shape_v1(
        semantic,
        correspondence_owner,
        semantic_function,
        kernel_ir_function,
        max_operations,
        closure_budget,
        |body, argument, local, ty| {
            kernel_parameter_shape_with_descriptors_v29(
                semantic, body, argument, local, ty, references, budget,
            )
        },
    )
}

// Shared original ABI shape selection. Callers prepay the structural shape
// walk; the descriptor service independently authenticates the source node.
fn kernel_parameter_shape_with_descriptors_v29(
    semantic: &AdmittedInertSemanticMirV1,
    body: &SemanticFunctionDeclV1,
    argument: u32,
    local: usize,
    ty: SemanticTypeIdV1,
    references: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<KernelParameterShapeV1, ProductionSemanticKirErrorV1> {
    let shape = source_kernel_parameter_shape_v18(semantic, body, argument, ty)?;
    if references.descriptor_root.is_some()
        && matches!(&shape, KernelParameterShapeV1::Direct(Type::Slice(_)))
        && source_descriptor_original_space_v29(semantic.types(), ty)?.is_some()
    {
        let local = SemanticLocalIdV1::from_index(
            u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let node =
            source_reference_entry_node_v29(references, references.root, local, None, budget)?;
        budget.source_reference_reserve_v29(references, std::mem::size_of::<Type>())?;
        Ok(KernelParameterShapeV1::Direct(
            source_descriptor_node_type_v29(references, node, ty, budget)?,
        ))
    } else {
        Ok(shape)
    }
}

fn kernel_entry_plan_with_shape_v1(
    semantic: &AdmittedInertSemanticMirV1,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    kernel_ir_function: FunctionId,
    max_operations: usize,
    closure_budget: &mut ReachableClosureBudgetV1,
    mut shape: impl FnMut(
        &SemanticFunctionDeclV1,
        u32,
        usize,
        SemanticTypeIdV1,
    ) -> Result<KernelParameterShapeV1, ProductionSemanticKirErrorV1>,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    let body = semantic
        .functions()
        .get(semantic_function.index() as usize)
        .ok_or_else(|| {
            unsupported(
                semantic_function.index(),
                None,
                None,
                "the selected kernel body is missing",
            )
        })?;
    let entry_parameters = semantic_function_parameters_v1(semantic_function, body)?;
    let mut entry_parameter_types = Vec::new();
    let mut entry_parameter_values = Vec::new();
    let mut entry_parameter_local_bindings = Vec::new();
    let mut entry_parameter_component_bindings = Vec::new();
    let mut entry_ignored_parameter_bindings = Vec::new();
    entry_parameter_types
        .try_reserve_exact(entry_parameters.len())
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::DebugBindings,
        })?;
    entry_parameter_local_bindings
        .try_reserve_exact(entry_parameters.len())
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::DebugBindings,
        })?;
    entry_parameter_values
        .try_reserve_exact(entry_parameters.len())
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::DebugBindings,
        })?;
    entry_ignored_parameter_bindings
        .try_reserve_exact(entry_parameters.len())
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::DebugBindings,
        })?;
    let mut next_component_value = u32::try_from(body.locals().len()).map_err(|_| {
        unsupported(
            semantic_function.index(),
            None,
            None,
            "local count does not fit Kernel IR",
        )
    })?;
    for (argument, local, ty) in &entry_parameters {
        let components = match shape(body, *argument, *local, *ty)? {
            KernelParameterShapeV1::Direct(parameter_ty) => {
                closure_budget.charge_parameter_expansion(
                    logical_argument_rows_v1(body),
                    entry_parameter_types.len(),
                    1,
                    max_operations,
                )?;
                let value = u32::try_from(*local).map(ValueId).map_err(|_| {
                    unsupported(
                        semantic_function.index(),
                        None,
                        None,
                        "local identity does not fit Kernel IR",
                    )
                })?;
                entry_parameter_types.push(parameter_ty.clone());
                entry_parameter_values.push(value);
                entry_parameter_local_bindings.push(PlannedParameterLocalBindingV1::Direct {
                    local: *local,
                    value,
                    ty: parameter_ty,
                });
                continue;
            }
            KernelParameterShapeV1::Components(components) => components,
        };
        closure_budget.charge_parameter_expansion(
            logical_argument_rows_v1(body),
            entry_parameter_types.len(),
            components.len(),
            max_operations,
        )?;
        if components.is_empty() {
            entry_ignored_parameter_bindings.push(SemanticKirIgnoredParameterBindingV1 {
                correspondence_owner,
                semantic_function,
                semantic_local: SemanticLocalIdV1::from_index(*local as u32),
                semantic_type: *ty,
            });
        }
        let mut values = Vec::new();
        values.try_reserve_exact(components.len()).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::DebugBindings,
            }
        })?;
        entry_parameter_types
            .try_reserve_exact(components.len())
            .and_then(|_| entry_parameter_values.try_reserve_exact(components.len()))
            .and_then(|_| entry_parameter_component_bindings.try_reserve_exact(components.len()))
            .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::DebugBindings,
            })?;
        for (component_index, (projection, component_type, parameter_ty, _, _)) in
            components.into_iter().enumerate()
        {
            let value = if component_index == 0 {
                ValueId(u32::try_from(*local).map_err(|_| {
                    unsupported(
                        semantic_function.index(),
                        None,
                        None,
                        "aggregate local identity does not fit Kernel IR",
                    )
                })?)
            } else {
                let value = ValueId(next_component_value);
                next_component_value = next_component_value.checked_add(1).ok_or_else(|| {
                    unsupported(
                        semantic_function.index(),
                        None,
                        None,
                        "aggregate parameter value identity overflow",
                    )
                })?;
                value
            };
            entry_parameter_types.push(parameter_ty.clone());
            entry_parameter_values.push(value);
            values.push(ValueDef::new(value, parameter_ty));
            entry_parameter_component_bindings.push(SemanticKirParameterComponentBindingV1 {
                correspondence_owner,
                semantic_function,
                semantic_local: SemanticLocalIdV1::from_index(*local as u32),
                semantic_component_type: component_type,
                projection: projection.into_boxed_slice(),
                kernel_ir_value: value,
            });
        }
        entry_parameter_local_bindings.push(PlannedParameterLocalBindingV1::Flattened {
            local: *local,
            semantic_type: *ty,
            values,
        });
    }
    Ok(LoweredFunctionPlanV1 {
        correspondence_owner,
        semantic_function,
        kernel_ir_function,
        role: SemanticKirFunctionRoleV1::KernelEntry,
        parameter_declarations: entry_parameters,
        parameter_types: entry_parameter_types,
        parameter_values: entry_parameter_values,
        call_arguments: Vec::new(),
        parameter_local_bindings: entry_parameter_local_bindings,
        parameter_component_bindings: entry_parameter_component_bindings,
        ignored_parameter_bindings: entry_ignored_parameter_bindings,
        result_types: Vec::new(),
    })
}
