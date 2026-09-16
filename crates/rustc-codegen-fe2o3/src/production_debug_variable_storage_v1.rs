type DebugSemanticFunctionV1 = fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
type DebugFunctionLayoutsV1<'a> =
    BTreeMap<(DebugSemanticFunctionV1, DebugSemanticFunctionV1), ExactDebugMapFunctionV1<'a>>;

fn exact_debug_function_ordinals_v1(
    layouts: &DebugFunctionLayoutsV1<'_>,
) -> Result<BTreeMap<DebugSemanticFunctionV1, u64>, ProductionPipelineError> {
    let mut ordinals = BTreeMap::new();
    for ((_, semantic), layout) in layouts {
        if let Some(previous) = ordinals.insert(*semantic, layout.function_ordinal)
            && previous != layout.function_ordinal
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "one semantic function maps to different physical KIR functions",
            ));
        }
    }
    Ok(ordinals)
}

fn compiler_debug_variable_storage_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    captured: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2],
) -> Result<Vec<fe2o3_kernel_ir::SemanticVariableStorageV1>, ProductionPipelineError> {
    use crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableClassV2 as Class;
    use fe2o3_kernel_ir::{
        SemanticStorageBindingV1 as Storage, SemanticStorageUnavailableReasonV1 as Reason,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticLocalRoleV1, SemanticSourceArgumentOwnershipV1,
    };
    let layouts = exact_debug_map_functions_v1(lowered)?;
    let ordinals = exact_debug_function_ordinals_v1(&layouts)?;
    let semantic = lowered.semantic().semantic();
    let mut parameters = BTreeMap::<_, Vec<_>>::new();
    for binding in lowered.correspondence().parameter_bindings() {
        parameters
            .entry(binding.semantic_function())
            .or_default()
            .push(*binding);
    }
    let ignored = lowered
        .correspondence()
        .ignored_parameter_bindings()
        .iter()
        .map(|binding| (binding.semantic_function(), binding.semantic_local()))
        .collect::<BTreeSet<_>>();
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(
            captured
                .iter()
                .filter(|variable| ordinals.contains_key(&variable.function))
                .count(),
        )
        .map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed storage variable allocation failed",
            )
        })?;
    for variable in captured
        .iter()
        .filter(|variable| ordinals.contains_key(&variable.function))
    {
        let error = || {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed source variable has no exact function/local storage",
            )
        };
        let function = semantic
            .functions()
            .get(variable.function.index() as usize)
            .ok_or_else(error)?;
        let ordinal = usize::try_from(ordinals[&variable.function]).map_err(|_| error())?;
        let physical = lowered.module().functions.get(ordinal).ok_or_else(error)?;
        let body = physical.body.as_ref().ok_or_else(error)?;
        let (local, ty, storage) = match variable.class {
            Class::Local(local) => {
                let declaration = function
                    .locals()
                    .get(local.index() as usize)
                    .ok_or_else(error)?;
                let ownership = match declaration.role() {
                    SemanticLocalRoleV1::Argument(argument)
                    | SemanticLocalRoleV1::RustCallTupleField { argument, .. } => *function
                        .abi()
                        .source_argument_ownership()
                        .get(argument as usize)
                        .ok_or_else(error)?,
                    _ => SemanticSourceArgumentOwnershipV1::ByValue,
                };
                let storage = if variable.entry_value_preserved {
                    compiler_parameter_storage_v1(
                        local.index(),
                        declaration.ty().index(),
                        ownership,
                        ignored.contains(&(variable.function, local)),
                        parameters
                            .get(&variable.function)
                            .map_or(&[], Vec::as_slice),
                        body,
                        &physical.signature.parameters,
                        semantic.types(),
                    )?
                } else {
                    Storage::Unavailable {
                        reason: Reason::OptimizedOut,
                    }
                };
                (Some(local.index()), Some(declaration.ty().index()), storage)
            }
            Class::Unrepresented => (
                None,
                None,
                Storage::Unavailable {
                    reason: Reason::UnrepresentedSourceVariable,
                },
            ),
        };
        variables.push(fe2o3_kernel_ir::SemanticVariableStorageV1::new(
            variable.identity,
            variable.function.index(),
            local,
            ty,
            storage,
        ));
    }
    Ok(variables)
}
