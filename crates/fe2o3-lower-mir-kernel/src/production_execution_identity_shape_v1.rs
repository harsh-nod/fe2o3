// An owned leaf has one identity channel. A borrowed leaf additionally keeps
// its referent channel: copying a borrow and dereferencing it are not aliases
// of the same nominal identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExecutionIdentitySelectionV1 {
    ty: SemanticTypeIdV1,
    first: usize,
    count: usize,
}

fn execution_identity_channels_v1(
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
        if let Some(borrowed) = execution_cfg_nominal_kind_v29(types, ty)? {
            return Ok(if borrowed { 2 } else { 1 });
        }
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(execution_identity_error_v1)?;
        let result = match declaration.shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let mut total = 0;
                for &field in fields.fields() {
                    total = argument_sum_v1(&[total, count(types, field, nodes, budget)?])?;
                }
                total
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let channels = count(types, *element, nodes, budget)?;
                argument_product_v1(
                    channels,
                    usize::try_from(*length).map_err(|_| execution_identity_error_v1())?,
                )?
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                // Preserve the current nominal-enum refusal. A discriminator
                // cannot turn mutually exclusive nominal owners into a tuple.
                for variant in variants {
                    for &field in variant.fields().fields() {
                        if count(types, field, nodes, budget)? != 0 {
                            return Err(execution_identity_error_v1());
                        }
                    }
                }
                0
            }
            _ => 0,
        };
        if result > argument_product_v1(MAX_SSA_VALUE_COMPONENTS_V1, 2)? {
            return Err(execution_identity_error_v1());
        }
        Ok(result)
    }
    count(types, ty, &mut 0, budget)
}

fn execution_identity_field_v1(
    types: &[SemanticTypeDeclV1],
    selection: ExecutionIdentitySelectionV1,
    field: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let declaration = types
        .get(selection.ty.index() as usize)
        .ok_or_else(execution_identity_error_v1)?;
    let (ty, offset) = match declaration.shape() {
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            let ty = *fields
                .fields()
                .get(field)
                .ok_or_else(execution_identity_error_v1)?;
            let mut offset = 0;
            for &preceding in &fields.fields()[..field] {
                offset = argument_sum_v1(&[
                    offset,
                    execution_identity_channels_v1(types, preceding, budget)?,
                ])?;
            }
            (ty, offset)
        }
        SemanticTypeShapeV1::Array { element, length }
            if u64::try_from(field).is_ok_and(|field| field < *length) =>
        {
            let width = execution_identity_channels_v1(types, *element, budget)?;
            (*element, argument_product_v1(field, width)?)
        }
        _ => return Err(execution_identity_error_v1()),
    };
    let count = execution_identity_channels_v1(types, ty, budget)?;
    if argument_sum_v1(&[offset, count])? > selection.count {
        return Err(execution_identity_error_v1());
    }
    Ok(ExecutionIdentitySelectionV1 {
        ty,
        first: argument_sum_v1(&[selection.first, offset])?,
        count,
    })
}

fn execution_identity_project_v1(
    types: &[SemanticTypeDeclV1],
    mut selection: ExecutionIdentitySelectionV1,
    projections: &[SemanticProjectionV1],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
    for projection in projections {
        budget.charge_work(3)?;
        selection = match projection.kind() {
            SemanticProjectionKindV1::Field(field) => {
                execution_identity_field_v1(types, selection, field as usize, budget)?
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                let SemanticTypeShapeV1::Array { length, .. } =
                    types[selection.ty.index() as usize].shape()
                else {
                    return Err(execution_identity_error_v1());
                };
                if *length < minimum_length {
                    return Err(execution_identity_error_v1());
                }
                let index = if from_end {
                    length.checked_sub(offset)
                } else {
                    Some(offset)
                }
                .filter(|index| *index < *length)
                .and_then(|index| usize::try_from(index).ok())
                .ok_or_else(execution_identity_error_v1)?;
                execution_identity_field_v1(types, selection, index, budget)?
            }
            SemanticProjectionKindV1::Dereference
                if execution_cfg_nominal_kind_v29(types, selection.ty)? == Some(true)
                    && selection.count == 2 =>
            {
                let SemanticTypeShapeV1::Pointer(pointer) =
                    types[selection.ty.index() as usize].shape()
                else {
                    return Err(execution_identity_error_v1());
                };
                ExecutionIdentitySelectionV1 {
                    ty: pointer.pointee(),
                    first: argument_sum_v1(&[selection.first, 1])?,
                    count: 1,
                }
            }
            // Dynamic selectors and representation-changing projections need
            // their own original source proof; a nominal shape is not one.
            _ => return Err(execution_identity_error_v1()),
        };
        if selection.ty != projection.result_type() {
            return Err(execution_identity_error_v1());
        }
    }
    Ok(selection)
}
