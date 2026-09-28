// Inert source representations: preserve the original address space, not legacy ABI coercion.
// These helpers share existing Pliron trace/leaf identities and grant no descriptor authority.
pub fn source_address_space_v18(
    space: u32,
) -> Result<AddressSpace, ProductionSourceArgumentErrorV1> {
    match space {
        0 => Ok(AddressSpace::Generic),
        1 => Ok(AddressSpace::Global),
        3 => Ok(AddressSpace::Workgroup),
        4 => Ok(AddressSpace::Constant),
        5 => Ok(AddressSpace::Private),
        _ => Err(unsupported(
            0,
            None,
            None,
            "semantic pointer address space is unsupported",
        )),
    }
}

fn restore_source_pointer_space_v18(
    types: &[SemanticTypeDeclV1],
    source: SemanticTypeIdV1,
    physical: &mut Type,
) -> Result<(), ProductionSourceArgumentErrorV1> {
    let declaration = types
        .get(source.index() as usize)
        .ok_or_else(|| unsupported(0, None, None, "kernel argument type is missing"))?;
    if let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() {
        match physical {
            Type::Pointer(value) => {
                value.address_space = source_address_space_v18(pointer.address_space())?
            }
            Type::Slice(value) => {
                value.address_space = source_address_space_v18(pointer.address_space())?
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn source_parameter_type_v18(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSourceArgumentErrorV1> {
    let mut physical = lower_parameter_type(types, callables, ty)?;
    restore_source_pointer_space_v18(types, ty, &mut physical)?;
    Ok(physical)
}

fn source_kernel_parameter_type_v18(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSourceArgumentErrorV1> {
    let mut physical = lower_kernel_parameter_type(types, callables, function, argument, ty)?;
    restore_source_pointer_space_v18(types, ty, &mut physical)?;
    Ok(physical)
}

pub fn source_abi_components_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueV1,
    policy: ParameterLeafPolicyV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSourceArgumentErrorV1> {
    let mut rows = lower_by_value_abi_components_v1(types, function, abi, policy)?;
    for (_, semantic_type, physical, _, _) in &mut rows {
        restore_source_pointer_space_v18(types, *semantic_type, physical)?;
    }
    Ok(rows)
}

pub fn source_helper_parameter_shape_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSourceArgumentErrorV1> {
    source_helper_parameter_shape_with_policy_v18(
        types,
        function,
        function_id,
        mapped,
        ParameterLeafPolicyV1::SharedSliceLeaves,
    )
}

pub fn source_helper_parameter_shape_with_policy_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    policy: ParameterLeafPolicyV1,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSourceArgumentErrorV1> {
    let (shared, mut rows) =
        helper_parameter_shape_with_policy_v1(types, function, function_id, mapped, policy)?;
    for (_, semantic_type, physical) in &mut rows {
        restore_source_pointer_space_v18(types, *semantic_type, physical)?;
    }
    Ok((shared, rows))
}

pub fn source_kernel_parameter_components_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSourceArgumentErrorV1> {
    let argument = usize::try_from(argument).map_err(|_| {
        unsupported(
            0,
            None,
            None,
            "kernel argument index does not fit this host",
        )
    })?;
    if function.abi().source_argument_ownership().get(argument)
        != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
        || function.abi().source_input_types().get(argument) != Some(&ty)
    {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument lacks exact by-value ABI ownership",
        ));
    }
    let abi = function
        .abi()
        .adjusted_arguments()
        .get(argument)
        .ok_or_else(|| unsupported(0, None, None, "aggregate kernel argument ABI is absent"))?;
    if abi.ty() != ty {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument ABI type changed",
        ));
    }
    source_parameter_components_v18(types, function, abi)
}

fn source_parameter_components_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSourceArgumentErrorV1> {
    source_abi_components_v18(
        types,
        function,
        abi.value(),
        // The original ByValue argument/ABI was authenticated by the caller.
        // Shared slice fields remain Generic carriers, not descriptor regions.
        ParameterLeafPolicyV1::SharedSliceLeaves,
    )
}

pub fn source_kernel_parameter_shape_v18(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<KernelParameterShapeV1, ProductionSourceArgumentErrorV1> {
    let error = match source_kernel_parameter_type_v18(
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
    source_kernel_parameter_components_v18(semantic.types(), function, argument, ty)
        .map(KernelParameterShapeV1::Components)
}
