// Shared representation selection for emission and correspondence replay.

fn check_argument_function_abi_v1(
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    role: SemanticKirFunctionRoleV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let abi = function.abi();
    let detail = match role {
        SemanticKirFunctionRoleV1::KernelEntry
            if abi.extern_abi()
                == fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::RustCall =>
        {
            Some("kernel entry requires an ordinary source ABI")
        }
        SemanticKirFunctionRoleV1::InternalHelper
            if function.role() != SemanticFunctionRoleV1::InternalHelper
                || function.export().is_some() =>
        {
            Some("reachable helper has an exported or non-helper semantic role")
        }
        SemanticKirFunctionRoleV1::InternalHelper
            if abi.can_unwind() || abi.c_variadic() || !abi.hidden_arguments().is_empty() =>
        {
            Some("helper does not have an exact non-unwinding direct scalar ABI")
        }
        _ => None,
    };
    match detail {
        Some(detail) => Err(unsupported(function_id.index(), None, None, detail)),
        None => Ok(()),
    }
}

fn lower_parameter_scalar_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    if !matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
    ) {
        return Err(unsupported(
            0,
            None,
            None,
            "parameter leaf has no scalar representation",
        ));
    }
    lower_scalar_type(types, ty)
}

fn lower_parameter_memory_element_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    memory_element_type_v1(types, ty).ok_or_else(|| {
        unsupported(
            0,
            None,
            None,
            "memory parameter element has no scalar or transparent representation",
        )
    })
}

enum KernelParameterShapeV1 {
    Direct(Type),
    Components(Vec<ByValueKernelParameterComponentV1>),
}

fn kernel_parameter_shape_v1(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<KernelParameterShapeV1, ProductionSemanticKirErrorV1> {
    let error = match lower_kernel_parameter_type(
        semantic.types(),
        semantic.callables(),
        function,
        argument,
        ty,
    ) {
        Ok(ty) => return Ok(KernelParameterShapeV1::Direct(ty)),
        Err(error) => error,
    };
    let declaration = semantic
        .types()
        .get(ty.index() as usize)
        .ok_or_else(|| unsupported(0, None, None, "kernel argument type is missing"))?;
    if !matches!(
        declaration.shape(),
        SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Array { .. }
            | SemanticTypeShapeV1::Tuple(_)
            | SemanticTypeShapeV1::Aggregate(_)
    ) || function
        .abi()
        .source_argument_ownership()
        .get(argument as usize)
        != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
    {
        return Err(error);
    }
    lower_by_value_kernel_parameter_components_v1(semantic.types(), function, argument, ty)
        .map(KernelParameterShapeV1::Components)
}

type HelperParameterComponentsV1 = Vec<(
    Vec<SemanticKirParameterProjectionV1>,
    SemanticTypeIdV1,
    Type,
)>;

fn helper_parameter_shape_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSemanticKirErrorV1> {
    let argument = mapped.abi();
    if argument.value().adjusted().is_some() || argument.value().pointee_override().is_some() {
        return Err(unsupported(
            function_id.index(),
            None,
            None,
            "helper parameter has an adjusted ABI type",
        ));
    }
    let shared_slice = mapped.tuple_field().is_none()
        && shared_slice_helper_parameter_v1(
            types,
            function,
            mapped.source_argument(),
            argument.ty(),
        );
    let components = if shared_slice {
        vec![(
            Vec::new(),
            argument.ty(),
            lower_parameter_type(types, &[], argument.ty())?,
        )]
    } else if mapped.tuple_field().is_some()
        && mapped.source_ownership() == SemanticSourceArgumentOwnershipV1::ByValue
        && shared_slice_leaf_v1(types, argument.ty())
    {
        let components = lower_by_value_abi_components_v1(
            types,
            function,
            argument.value(),
            ParameterLeafPolicyV1::SharedSliceLeaves,
        )?;
        return Ok((
            true,
            components
                .into_iter()
                .map(|(path, ty, kir, _, _)| (path, ty, kir))
                .collect(),
        ));
    } else if matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
        && matches!(
            types[argument.ty().index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        )
        && let Ok(scalar) = lower_scalar_type(types, argument.ty())
    {
        vec![(Vec::new(), argument.ty(), scalar)]
    } else {
        if mapped.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue
            || matches!(
                types[argument.ty().index() as usize].shape(),
                SemanticTypeShapeV1::Pointer(_)
            )
        {
            return Err(unsupported(
                function_id.index(),
                None,
                None,
                "helper parameter is not an exact by-value scalar aggregate or shared slice",
            ));
        }
        lower_by_value_abi_components_v1(
            types,
            function,
            argument.value(),
            ParameterLeafPolicyV1::SharedSliceLeaves,
        )
        .map_err(|error| match error {
            ProductionSemanticKirErrorV1::Unsupported {
                block,
                statement,
                detail,
                ..
            } => unsupported(function_id.index(), block, statement, detail),
            other => other,
        })?
        .into_iter()
        .map(|(path, ty, kir, _, _)| (path, ty, kir))
        .collect()
    };
    Ok((shared_slice, components))
}
