fn global_bf16_enum_payload_contract_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    plan: &SemanticControlFlowSsaPlanV1,
    (local, variant, field): (u32, u32, u32),
) -> Option<SemanticGlobalBf16MatrixLoadV1> {
    let ty = function.locals().get(local as usize)?.ty();
    let SemanticTypeShapeV1::Enum { variants, .. } = types.get(ty.index() as usize)?.shape() else {
        return None;
    };
    let field = variants
        .get(variant as usize)?
        .fields()
        .fields()
        .get(field as usize)?;
    match plan.compiler_issued_bindings.get(field)? {
        SemanticPromotedBindingV1::GlobalBf16MatrixView { contract } => Some(*contract),
        _ => None,
    }
}

fn global_bf16_view_transport_types_v1(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
    context: Option<&KernelContextTypeV1>,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    let context = context.ok_or_else(|| {
        unsupported(
            0,
            None,
            None,
            "global BF16 view transport lacks authenticated root context",
        )
    })?;
    if semantic_type != contract.types().matrix
        || !semantic_global_bf16_matrix_layout_matches_v1(types, contract.types())
        || !global_bf16_transport_context_matches_v1(contract, context)
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(vec![
        Type::GlobalCapability(lower_global_capability_type_v1(
            types,
            contract.types().element,
            contract.memory(),
            context,
        )?),
        Type::Scalar(ScalarType::U64),
        Type::Scalar(ScalarType::U64),
        Type::Scalar(ScalarType::U64),
        Type::Scalar(ScalarType::U64),
    ])
}

fn global_bf16_transport_context_matches_v1(
    contract: SemanticGlobalBf16MatrixLoadV1,
    context: &KernelContextTypeV1,
) -> bool {
    context.kernel_marker() == contract.provenance().kernel_marker().as_bytes()
        && context.target() == contract.provenance().target_brand().as_bytes()
        && context.launch() == contract.provenance().launch_brand().as_bytes()
}

fn global_bf16_view_transport_values_v1(
    contract: SemanticGlobalBf16MatrixLoadV1,
    binding: &SemanticValueBindingV1,
) -> Result<Vec<(ValueId, Type)>, &'static str> {
    let SemanticValueBindingV1::Aggregate(fields) = binding else {
        return Err("global BF16 view lost its retained aggregate");
    };
    if fields.len() != 8
        || fields[5..]
            .iter()
            .any(|field| !field.values().is_ok_and(|values| values.is_empty()))
    {
        return Err("global BF16 view fields or inhabited markers changed");
    }
    let SemanticValueBindingV1::GlobalCapability {
        value,
        semantic_view,
        element,
        contract: memory,
        provenance,
        capability,
    } = &fields[0]
    else {
        return Err("global BF16 view lacks a typed Global producer");
    };
    if *semantic_view != contract.types().global
        || *element != contract.types().element
        || *memory != contract.memory()
        || *provenance != contract.provenance()
        || capability.role() != GlobalCapabilityRoleV1::ReadOnly
        || capability.element() != &Type::Scalar(ScalarType::U16)
        || !global_bf16_transport_context_matches_v1(contract, capability.context())
        || !capability.is_complete()
    {
        return Err("global BF16 view memory, type, or root provenance changed");
    }
    let mut result = vec![(*value, Type::GlobalCapability(capability.clone()))];
    for field in &fields[1..5] {
        let component = field.value()?;
        if component.1 != Type::Scalar(ScalarType::U64) {
            return Err("global BF16 view coordinate is not its retained usize");
        }
        result.push(component);
    }
    Ok(result)
}

fn global_bf16_view_from_transport_v1(
    contract: SemanticGlobalBf16MatrixLoadV1,
    values: &[ValueDef],
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    let [storage, offset, rows, columns, stride] = values else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let Type::GlobalCapability(capability) = &storage.ty else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    // This reconstructs the descriptor of an existing typed SSA edge. It does
    // not emit GlobalCapabilityBind or authenticate a new allocation/borrow.
    let mut fields = vec![SemanticValueBindingV1::GlobalCapability {
        value: storage.id,
        semantic_view: contract.types().global,
        element: contract.types().element,
        contract: contract.memory(),
        provenance: contract.provenance(),
        capability: capability.clone(),
    }];
    fields.extend([offset, rows, columns, stride].into_iter().map(|value| {
        SemanticValueBindingV1::Value {
            id: value.id,
            ty: value.ty.clone(),
        }
    }));
    fields.extend([
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::Unit,
    ]);
    let binding = SemanticValueBindingV1::Aggregate(fields);
    global_bf16_view_transport_values_v1(contract, &binding)
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    Ok(binding)
}
