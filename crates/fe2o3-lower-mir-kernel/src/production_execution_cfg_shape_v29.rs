// A projection of the retained type tree, not another executable representation.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ExecutionCfgLeafV29 {
    Owned(SemanticExecutionBindingV29),
    Borrow(SemanticExecutionBorrowBindingV29),
    Moved,
}

fn execution_cfg_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "execution CFG transport differs from its captured SSA state",
    )
}

fn execution_cfg_charge_node_v29(
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    *nodes = nodes.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(execution_cfg_error_v29());
    }
    Ok(())
}

fn execution_cfg_nominal_kind_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Option<bool>, ProductionSemanticKirErrorV1> {
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(execution_cfg_error_v29)?;
    if matches!(
        declaration.rust_type_kind(),
        fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(_)
    ) {
        return Ok(Some(false));
    }
    if let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape()
        && matches!(
            types
                .get(pointer.pointee().index() as usize)
                .map(|ty| ty.rust_type_kind()),
            Some(fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(_))
        )
    {
        if pointer.kind() != SemanticPointerKindV1::Reference {
            return Err(execution_cfg_error_v29());
        }
        return Ok(Some(true));
    }
    Ok(None)
}

fn execution_cfg_nominal_count_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    fn count(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        nodes: &mut usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        execution_cfg_charge_node_v29(nodes, budget)?;
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            return Ok(1);
        }
        match types[ty.index() as usize].shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let mut total = 0;
                for field in fields.fields() {
                    total = argument_sum_v1(&[total, count(types, *field, nodes, budget)?])?;
                }
                Ok(total)
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let leaf_count = count(types, *element, nodes, budget)?;
                if leaf_count == 0 {
                    return Ok(0);
                }
                let length = usize::try_from(*length).map_err(|_| execution_cfg_error_v29())?;
                let total = argument_product_v1(leaf_count, length)?;
                if total > MAX_SSA_VALUE_COMPONENTS_V1 {
                    return Err(execution_cfg_error_v29());
                }
                Ok(total)
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                for variant in variants {
                    for field in variant.fields().fields() {
                        if count(types, *field, nodes, budget)? != 0 {
                            return Err(execution_cfg_error_v29());
                        }
                    }
                }
                Ok(0)
            }
            _ => Ok(0),
        }
    }
    count(types, ty, &mut 0, budget)
}

fn execution_cfg_leaf_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
) -> Result<ExecutionCfgLeafV29, ProductionSemanticKirErrorV1> {
    match (execution_cfg_nominal_kind_v29(types, ty)?, binding) {
        (Some(false), SemanticValueBindingV1::Execution(value)) => {
            value
                .check_type(types, ty)
                .map_err(|_| execution_cfg_error_v29())?;
            Ok(ExecutionCfgLeafV29::Owned(value.clone()))
        }
        (Some(true), SemanticValueBindingV1::ExecutionBorrow(value)) => {
            value
                .check_type(types, ty)
                .map_err(|_| execution_cfg_error_v29())?;
            Ok(ExecutionCfgLeafV29::Borrow(value.clone()))
        }
        (Some(_), SemanticValueBindingV1::MovedExecution) => Ok(ExecutionCfgLeafV29::Moved),
        _ => Err(execution_cfg_error_v29()),
    }
}

// Ordinary leaves retain the existing representation; nominal leaves have no
// physical component. Arrays and tuples share this bounded structural walk.
type ExecutionCfgFieldsV29<'a> = (usize, Option<&'a [SemanticTypeIdV1]>, SemanticTypeIdV1);

fn execution_cfg_fields_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Option<ExecutionCfgFieldsV29<'_>>, ProductionSemanticKirErrorV1> {
    match types
        .get(ty.index() as usize)
        .ok_or_else(execution_cfg_error_v29)?
        .shape()
    {
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            Ok(Some((fields.fields().len(), Some(fields.fields()), ty)))
        }
        SemanticTypeShapeV1::Array { element, length } => {
            let length = usize::try_from(*length).map_err(|_| execution_cfg_error_v29())?;
            if length > MAX_SSA_VALUE_COMPONENTS_V1 {
                return Err(execution_cfg_error_v29());
            }
            Ok(Some((length, None, *element)))
        }
        _ => Ok(None),
    }
}

fn execution_cfg_memory_type_v29(
    types: &[SemanticTypeDeclV1],
    mut ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let mut nodes = 0;
    loop {
        execution_cfg_charge_node_v29(&mut nodes, budget)?;
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(execution_cfg_error_v29)?;
        require_ordinary_execution_representation_v29(declaration)?;
        if matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        ) {
            return lower_scalar_type(types, ty);
        }
        let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
            return Err(execution_cfg_error_v29());
        };
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
            return Err(execution_cfg_error_v29());
        };
        if fields.fields().len() != 1
            || layout.field_offsets() != [0]
            || !layout.padding().is_empty()
            || declaration.layout().is_uninhabited()
        {
            return Err(execution_cfg_error_v29());
        }
        ty = fields.fields()[0];
        let field = types
            .get(ty.index() as usize)
            .ok_or_else(execution_cfg_error_v29)?;
        if field.layout().is_uninhabited()
            || field.layout().size_bytes() != declaration.layout().size_bytes()
            || field.layout().alignment_bytes() != declaration.layout().alignment_bytes()
        {
            return Err(execution_cfg_error_v29());
        }
    }
}

fn execution_cfg_ordinary_type_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<Type>, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    match types
        .get(ty.index() as usize)
        .ok_or_else(execution_cfg_error_v29)?
        .shape()
    {
        SemanticTypeShapeV1::Unit => Ok(None),
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            lower_scalar_type(types, ty).map(Some)
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            let access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            let address_space = lower_address_space(pointer.address_space())?;
            let element = match pointer.metadata() {
                SemanticPointerMetadataV1::None => {
                    execution_cfg_memory_type_v29(types, pointer.pointee(), budget)?
                }
                SemanticPointerMetadataV1::SliceLength => {
                    budget.charge_work(2)?;
                    let pointee = types
                        .get(pointer.pointee().index() as usize)
                        .ok_or_else(execution_cfg_error_v29)?;
                    require_ordinary_execution_representation_v29(pointee)?;
                    let SemanticTypeShapeV1::Slice { element } = pointee.shape() else {
                        return Err(execution_cfg_error_v29());
                    };
                    lower_scalar_type(types, *element)?
                }
                SemanticPointerMetadataV1::VTable => return Err(execution_cfg_error_v29()),
            };
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            Ok(Some(
                if pointer.metadata() == SemanticPointerMetadataV1::SliceLength {
                    Type::slice(element, address_space, access)
                } else {
                    Type::pointer(element, address_space, access)
                },
            ))
        }
        _ => Err(execution_cfg_error_v29()),
    }
}

fn execution_cfg_types_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    fn append(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        output: &mut Vec<Type>,
        nodes: &mut usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        execution_cfg_charge_node_v29(nodes, budget)?;
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            return Ok(());
        }
        if let Some((count, fields, element)) = execution_cfg_fields_v29(types, ty)? {
            for index in 0..count {
                append(
                    types,
                    fields.map_or(element, |fields| fields[index]),
                    output,
                    nodes,
                    budget,
                )?;
            }
        } else if let Some(ty) = execution_cfg_ordinary_type_v29(types, ty, budget)? {
            emission_push_v1(output, ty, budget)?;
        }
        Ok(())
    }
    let mut output = Vec::new();
    append(types, ty, &mut output, &mut 0, budget)?;
    Ok(output)
}

fn merge_execution_cfg_binding_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    slots: &mut std::slice::IterMut<'_, Option<ExecutionCfgLeafV29>>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
        let leaf = execution_cfg_leaf_v29(types, ty, held)?;
        let original = execution_cfg_leaf_v29(types, ty, archived)?;
        if leaf != ExecutionCfgLeafV29::Moved && leaf != original {
            return Err(execution_cfg_error_v29());
        }
        let slot = slots.next().ok_or_else(execution_cfg_error_v29)?;
        match slot {
            Some(previous) if *previous != leaf => return Err(execution_cfg_error_v29()),
            Some(_) => {}
            None => *slot = Some(leaf),
        }
        return Ok(());
    }
    if let Some((count, fields, element)) = execution_cfg_fields_v29(types, ty)? {
        let (SemanticValueBindingV1::Aggregate(held), SemanticValueBindingV1::Aggregate(archived)) =
            (held, archived)
        else {
            return Err(execution_cfg_error_v29());
        };
        if held.len() != count || archived.len() != count {
            return Err(execution_cfg_error_v29());
        }
        for index in 0..count {
            merge_execution_cfg_binding_v29(
                types,
                fields.map_or(element, |fields| fields[index]),
                &held[index],
                &archived[index],
                slots,
                nodes,
                budget,
            )?;
        }
    } else if execution_binding_contains_paid_v29(held, budget)?
        || execution_binding_contains_paid_v29(archived, budget)?
    {
        return Err(execution_cfg_error_v29());
    }
    Ok(())
}

fn execution_cfg_values_v29(
    binding: &SemanticValueBindingV1,
    values: &mut Vec<ValueDef>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    match binding {
        SemanticValueBindingV1::Execution(_)
        | SemanticValueBindingV1::ExecutionBorrow(_)
        | SemanticValueBindingV1::MovedExecution
        | SemanticValueBindingV1::Unit => {}
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                execution_cfg_values_v29(field, values, nodes, budget)?;
            }
        }
        SemanticValueBindingV1::Value { id, ty } => {
            if matches!(ty, Type::Execution(_)) {
                return Err(execution_cfg_error_v29());
            }
            let ty = execution_cfg_clone_type_v29(ty, budget)?;
            emission_push_v1(values, ValueDef::new(*id, ty), budget)?;
        }
        _ => return Err(execution_cfg_error_v29()),
    }
    Ok(())
}

fn execution_cfg_clone_type_v29(
    ty: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    execution_cfg_clone_type_inner_v29(ty, &mut 0, budget)
}

fn execution_cfg_clone_type_inner_v29(
    ty: &Type,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    match ty {
        Type::Unit | Type::Scalar(_) | Type::Vector(_) => Ok(ty.clone()),
        Type::Pointer(pointer) => {
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            Ok(Type::pointer(
                execution_cfg_clone_type_inner_v29(&pointer.pointee, nodes, budget)?,
                pointer.address_space,
                pointer.access,
            ))
        }
        Type::Slice(slice) => {
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            Ok(Type::slice(
                execution_cfg_clone_type_inner_v29(&slice.element, nodes, budget)?,
                slice.address_space,
                slice.access,
            ))
        }
        Type::Execution(_) => Err(execution_cfg_error_v29()),
    }
}

fn clone_execution_cfg_binding_v29(
    binding: &SemanticValueBindingV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    Ok(match binding {
        SemanticValueBindingV1::Unit
        | SemanticValueBindingV1::Execution(_)
        | SemanticValueBindingV1::ExecutionBorrow(_)
        | SemanticValueBindingV1::MovedExecution => binding.clone(),
        SemanticValueBindingV1::Aggregate(fields) => {
            let mut output = emission_vec_v1(fields.len(), budget)?;
            for field in fields {
                output.push(clone_execution_cfg_binding_v29(field, nodes, budget)?);
            }
            SemanticValueBindingV1::Aggregate(output)
        }
        SemanticValueBindingV1::Value { id, ty } => SemanticValueBindingV1::Value {
            id: *id,
            ty: execution_cfg_clone_type_inner_v29(ty, nodes, budget)?,
        },
        _ => return Err(execution_cfg_error_v29()),
    })
}

fn rebuild_execution_cfg_binding_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    canonical: bool,
    leaves: &mut std::slice::Iter<'_, Option<ExecutionCfgLeafV29>>,
    values: &mut std::slice::Iter<'_, ValueDef>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
        let binding = match leaves
            .next()
            .and_then(Option::as_ref)
            .ok_or_else(execution_cfg_error_v29)?
        {
            ExecutionCfgLeafV29::Owned(value) => SemanticValueBindingV1::Execution(value.clone()),
            ExecutionCfgLeafV29::Borrow(value) => {
                SemanticValueBindingV1::ExecutionBorrow(value.clone())
            }
            ExecutionCfgLeafV29::Moved => SemanticValueBindingV1::MovedExecution,
        };
        execution_cfg_leaf_v29(types, ty, &binding)?;
        return Ok(binding);
    }
    if let Some((count, fields, element)) = execution_cfg_fields_v29(types, ty)? {
        let mut output = emission_vec_v1(count, budget)?;
        for index in 0..count {
            output.push(rebuild_execution_cfg_binding_v29(
                types,
                fields.map_or(element, |fields| fields[index]),
                canonical,
                leaves,
                values,
                nodes,
                budget,
            )?);
        }
        return Ok(SemanticValueBindingV1::Aggregate(output));
    }
    if matches!(
        types[ty.index() as usize].shape(),
        SemanticTypeShapeV1::Unit
    ) {
        return Ok(SemanticValueBindingV1::Unit);
    }
    if matches!(
        types[ty.index() as usize].shape(),
        SemanticTypeShapeV1::Enum { .. }
    ) {
        return Err(execution_cfg_error_v29());
    }
    let value = values.next().ok_or_else(execution_cfg_error_v29)?;
    let output_type = if canonical {
        let expected = execution_cfg_ordinary_type_v29(types, ty, budget)?
            .ok_or_else(execution_cfg_error_v29)?;
        if value.ty != expected {
            return Err(execution_cfg_error_v29());
        }
        expected
    } else {
        execution_cfg_clone_type_v29(&value.ty, budget)?
    };
    Ok(SemanticValueBindingV1::Value {
        id: value.id,
        ty: output_type,
    })
}
