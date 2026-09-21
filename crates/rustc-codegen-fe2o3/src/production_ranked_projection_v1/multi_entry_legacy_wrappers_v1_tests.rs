// Same-scope compatibility for historical isolated projection tests. These
// wrappers cannot attach a new multi-entry proof without genuine live facts.
#[allow(clippy::too_many_arguments)]
fn project_intrinsic_contracts(
    callables: &[SemanticCallableDeclV1],
    callable_effects: &DefinedCallableEmptyEffectSummariesV1,
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    linear_launch_upper_bound: Option<u64>,
    constants: &[Option<u64>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<IntrinsicProjectionV1, ProductionRankedProjectionErrorV1> {
    project_intrinsic_contracts_with_multi_entry_v1(
        callables,
        callable_effects,
        types,
        function,
        linear_launch_upper_bound,
        constants,
        operations,
        next_value,
        ranked_ir,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_uniform_inductions_v1(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<Vec<ProjectedUniformInductionV1>, ProductionRankedProjectionErrorV1> {
    project_uniform_inductions_with_multi_entry_v1(
        callables,
        types,
        function,
        constants,
        stable_argument_origins,
        local_definitions,
        arguments,
        next_argument,
        operations,
        next_value,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_progress_and_emit_unsigned_casts_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    arguments: &[Option<u32>],
    inductions: &mut [ProjectedUniformInductionV1],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    reconcile_source_progress_with_multi_entry_v1(
        types,
        function,
        constants,
        stable_argument_origins,
        local_definitions,
        arguments,
        inductions,
        operations,
        next_value,
        None,
    )
}
