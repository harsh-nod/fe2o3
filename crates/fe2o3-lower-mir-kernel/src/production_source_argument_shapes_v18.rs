// Original-source representation only; legacy V1 ABI selection remains unchanged.
fn source_address_space_v18(space: u32) -> Result<AddressSpace, ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_address_space_v18(space).map_err(Into::into)
}

fn source_parameter_type_v18(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_parameter_type_v18(types, callables, ty)
        .map_err(Into::into)
}

fn source_kernel_parameter_shape_v18(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<KernelParameterShapeV1, ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_kernel_parameter_shape_v18(
        semantic, function, argument, ty,
    )
    .map_err(Into::into)
}

fn source_helper_parameter_shape_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_helper_parameter_shape_v18(
        types,
        function,
        function_id,
        mapped,
    )
    .map_err(Into::into)
}

fn source_kernel_parameter_components_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_kernel_parameter_components_v18(
        types, function, argument, ty,
    )
    .map_err(Into::into)
}

fn source_helper_parameter_shape_with_policy_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    policy: ParameterLeafPolicyV1,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_helper_parameter_shape_with_policy_v18(
        types,
        function,
        function_id,
        mapped,
        policy,
    )
    .map_err(Into::into)
}

fn source_abi_components_v18(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueV1,
    policy: ParameterLeafPolicyV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSemanticKirErrorV1> {
    source_arguments_v1::scoped_v18::source_abi_components_v18(types, function, abi, policy)
        .map_err(Into::into)
}
