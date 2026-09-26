// Private nominal entry planning. These canonical scalar components are NOT the
// physical Rust FnABI (whose actual Direct/Cast/Indirect modes stay in the source).
struct Bf16CallEmissionStateV1<'a> {
    source: &'a CheckedBf16CallInstanceV1<'a>,
    capture: Bf16CallEmissionCaptureV1,
}

fn bf16_emission_refusal_v1(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

fn bf16_call_error_v1(error: Bf16CallInstanceErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        Bf16CallInstanceErrorV1::Resource(error) => error.into(),
        Bf16CallInstanceErrorV1::Unavailable(detail) => bf16_emission_refusal_v1(detail),
        Bf16CallInstanceErrorV1::CallbackPanicked => {
            bf16_emission_refusal_v1("BF16 materialization callback unwound")
        }
    }
}

fn bf16_parameter_plan_v1(
    source: &CheckedBf16CallInstanceV1<'_>,
    semantic: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
    helper: SemanticFunctionIdV1,
    id: FunctionId,
    closure: &mut ReachableClosureBudgetV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1> {
    if !std::ptr::eq(source.owner().source_semantic(), semantic)
        || root != source.root()
        || helper != source.helper()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let function = source.helper_declaration();
    let mut parameters = Vec::with_capacity(4);
    for (local, decl) in function.locals().iter().enumerate() {
        if let SemanticLocalRoleV1::Argument(argument) = decl.role() {
            parameters.push((argument, local, decl.ty()));
        } else if matches!(decl.role(), SemanticLocalRoleV1::RustCallTupleField { .. }) {
            return Err(bf16_emission_refusal_v1(
                "BF16 tuple-expanded helper parameter",
            ));
        }
    }
    parameters.sort_unstable_by_key(|row| row.0);
    if parameters.len() != 4
        || parameters
            .iter()
            .enumerate()
            .any(|(i, p)| p.0 as usize != i)
    {
        return Err(bf16_emission_refusal_v1(
            "BF16 exact four source parameters",
        ));
    }
    let mut parameter_types = Vec::with_capacity(12);
    let mut parameter_values = Vec::with_capacity(12);
    let mut call_arguments = Vec::with_capacity(12);
    let mut bindings = Vec::with_capacity(4);
    let mut next =
        u32::try_from(function.locals().len()).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    for (argument, local, ty) in &parameters {
        let descriptor = bf16_parameter_descriptor_v1(semantic, function, *argument, budget)?;
        let types = descriptor.transport_types(semantic.types(), *ty)?;
        let expected = if *argument == 0 { 0 } else { 4 };
        if types.len() != expected {
            return Err(bf16_emission_refusal_v1("BF16 nominal parameter width"));
        }
        closure.charge_parameter_expansion(4, parameter_types.len(), expected, 1024)?;
        let mut values = Vec::with_capacity(expected);
        for (component, ty) in types.into_iter().enumerate() {
            let value = if component == 0 {
                ValueId(u32::try_from(*local).map_err(|_| ArgumentResourceV1::Arithmetic)?)
            } else {
                let result = ValueId(next);
                next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                result
            };
            parameter_types.push(ty.clone());
            parameter_values.push(value);
            values.push(ValueDef::new(value, ty));
            call_arguments.push(HelperCallArgumentV1 {
                source_argument: *argument,
                tuple_field: None,
                component: Some(component),
            });
        }
        bindings.push(PlannedParameterLocalBindingV1::Bf16Nominal {
            local: *local,
            semantic_type: *ty,
            descriptor,
            values,
        });
    }
    Ok(LoweredFunctionPlanV1 {
        correspondence_owner: root,
        semantic_function: helper,
        kernel_ir_function: id,
        role: SemanticKirFunctionRoleV1::InternalHelper,
        parameter_declarations: parameters,
        parameter_types,
        parameter_values,
        call_arguments,
        parameter_local_bindings: bindings,
        // No fictitious structural tuple path and no ignored pointer argument.
        parameter_component_bindings: Vec::new(),
        ignored_parameter_bindings: Vec::new(),
        result_types: vec![Type::Scalar(ScalarType::F32); 4],
    })
}

fn bf16_parameter_descriptor_v1(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    argument: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticPromotedBindingV1, ProductionSemanticKirErrorV1> {
    let ty = *function
        .abi()
        .source_input_types()
        .get(argument as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if argument == 0 {
        // The checked relation, not this type alone, establishes the authentic
        // Current -> shared borrow -> actual helper MFMA receiver relation.
        return Ok(SemanticPromotedBindingV1::MatrixContext);
    }
    let bindings = compiler_issued_ssa_bindings_v1(
        semantic.types(),
        semantic.callables(),
        function,
        semantic
            .functions()
            .iter()
            .position(|f| std::ptr::eq(f, function))
            .and_then(|i| u32::try_from(i).ok())
            .map(SemanticFunctionIdV1::from_index)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
        None,
        Some(budget),
    )?;
    let binding = bindings
        .get(&ty)
        .copied()
        .ok_or_else(|| bf16_emission_refusal_v1("BF16 authenticated nominal type absent"))?;
    let good = match (argument, binding) {
        (1 | 2, SemanticPromotedBindingV1::MatrixFragment { contract, storage_layout }) => {
            storage_layout == SemanticMfmaStorageLayoutV1::RowMajor
                && contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
                && contract.wave_width == 64
                && contract.register_distribution == SemanticMfmaRegisterDistributionV1::Tile16x16
                && contract.role == if argument == 1 { SemanticMfmaOperandRoleV1::A } else { SemanticMfmaOperandRoleV1::B }
        }
        (3, SemanticPromotedBindingV1::AccumulatorFragment { contract }) => {
            contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
                && contract.wave_width == 64
                && contract.distribution == fe2o3_mir_model::semantic_mir_v1::SemanticMfmaAccumulatorDistributionV1::RowMajor
        }
        _ => false,
    };
    if !good {
        return Err(bf16_emission_refusal_v1("BF16 nominal role/layout differs"));
    }
    Ok(binding)
}

// A pending effect admission is scoped to the exact source-selected helper.
// It is not exposed through CanonicalKirCallEffects or the RawEmpty route.
fn bf16_pending_helper_v1(
    state: &Bf16CallEmissionStateV1<'_>,
    plan: &LoweredFunctionPlanV1,
    module: &Module,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(module.functions.len() + 4)?;
    if plan.semantic_function != state.source.helper()
        || plan.correspondence_owner != state.source.root()
        || plan.role != SemanticKirFunctionRoleV1::InternalHelper
        || state.capture.seen != [true, true]
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let function = module
        .function(&plan.kernel_ir_function)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let body = function
        .body
        .as_ref()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if body.blocks.is_empty() || body.blocks.len() > 32 {
        return Err(bf16_emission_refusal_v1(
            "BF16 pending helper block ceiling",
        ));
    }
    let mut matrices = 0usize;
    let mut returns = 0usize;
    for block in &body.blocks {
        for operation in &block.operations {
            budget.charge_work(1)?;
            let OperationKind::Matrix(matrix) = &operation.kind else {
                return Err(bf16_emission_refusal_v1(
                    "BF16 pending helper has extra operation",
                ));
            };
            let MatrixOperationKind::MultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } = &matrix.kind
            else {
                return Err(bf16_emission_refusal_v1("BF16 pending helper matrix kind"));
            };
            if *matrix
                != MatrixOperation::multiply_accumulate(*lhs, *rhs, *accumulator)
                    .with_declared_tensor_layout(
                        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                            .with_zero_filled_predicate_inputs(),
                    )
                || operation.results.len() != 4
            {
                return Err(bf16_emission_refusal_v1(
                    "BF16 pending helper matrix contract",
                ));
            }
            matrices += 1;
        }
        budget.charge_work(1)?;
        match &block.terminator {
            Some(Terminator::Branch { .. }) => {}
            Some(Terminator::Return { values }) if values.len() == 4 => {
                returns += 1;
            }
            _ => return Err(bf16_emission_refusal_v1("BF16 pending helper control")),
        }
    }
    if matrices != 1 || returns != 1 {
        return Err(bf16_emission_refusal_v1(
            "BF16 pending helper complete body roster",
        ));
    }
    Ok(())
}

fn bf16_pending_capabilities_v1(
    state: &Bf16CallEmissionStateV1<'_>,
    plans: &[LoweredFunctionPlanV1],
    module: &mut Module,
    symbol: &str,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if plans.len() != 2 || module.required_capabilities.len() > 32 {
        return Err(bf16_emission_refusal_v1("BF16 pending capability roster"));
    }
    bf16_pending_helper_v1(state, &plans[1], module, budget)?;
    budget.charge_work(module.required_capabilities.len() + module.functions.len())?;
    let root = module
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == symbol)
        .filter(|f| f.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    // Only actual operation capabilities already emitted under this closed
    // source profile. No capability text grants nominal/effect admission.
    root.required_capabilities
        .extend(module.required_capabilities.iter().cloned());
    Ok(())
}
