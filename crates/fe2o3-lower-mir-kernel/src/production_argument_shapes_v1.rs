// Representation selection is shared with source-owned correspondence replay.
use source_arguments_v1::{HelperParameterComponentsV1, KernelParameterShapeV1};

fn check_argument_function_abi_v1(
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    role: SemanticKirFunctionRoleV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_arguments_v1::check_argument_function_abi_v1(function, function_id, role)
        .map_err(Into::into)
}

fn lower_parameter_scalar_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    source_arguments_v1::lower_parameter_scalar_v1(types, ty).map_err(Into::into)
}

fn lower_parameter_memory_element_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    source_arguments_v1::lower_parameter_memory_element_v1(types, ty).map_err(Into::into)
}

fn kernel_parameter_shape_v1(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<KernelParameterShapeV1, ProductionSemanticKirErrorV1> {
    source_arguments_v1::kernel_parameter_shape_v1(semantic, function, argument, ty)
        .map_err(Into::into)
}

fn helper_parameter_shape_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSemanticKirErrorV1> {
    source_arguments_v1::helper_parameter_shape_v1(types, function, function_id, mapped)
        .map_err(Into::into)
}

fn helper_parameter_shape_with_policy_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    policy: ParameterLeafPolicyV1,
) -> Result<(bool, HelperParameterComponentsV1), ProductionSemanticKirErrorV1> {
    source_arguments_v1::helper_parameter_shape_with_policy_v1(
        types,
        function,
        function_id,
        mapped,
        policy,
    )
    .map_err(Into::into)
}
