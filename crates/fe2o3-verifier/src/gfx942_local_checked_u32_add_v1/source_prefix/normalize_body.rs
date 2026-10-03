macro_rules! checked_u32_prefix_type_body_v1 {
    ($exec:ident, $types:ident, $ty:ident) => {
        $exec!({
            let index = $ty.index() as usize;
            if index >= $types.len() {
                return false;
            }
            matches!(
                $types[index].shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            )
        })
    };
}

macro_rules! checked_u32_prefix_local_body_v1 {
    ($exec:ident, $types:ident, $locals:ident, $place:ident) => {
        $exec!({
            let index = $place.local().index() as usize;
            if !$place.projections().is_empty()
                || !is_u32($types, $place.ty())
                || index >= $locals.len()
                || $locals[index].ty().index() != $place.ty().index()
            {
                return Err(CheckedU32PrefixErrorV1::Source);
            }
            Ok(index)
        })
    };
}

macro_rules! checked_u32_prefix_constant_body_v1 {
    ($exec:ident, $types:ident, $operand:ident) => {
        $exec!({
            let SemanticOperandV1::Constant(constant) = $operand else {
                return None;
            };
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return None;
            };
            if !is_u32($types, constant.ty())
                || value.size_bytes() != 4
                || value.bits() > u32::MAX as u128
            {
                return None;
            }
            Some(value.bits() as u32)
        })
    };
}

macro_rules! checked_u32_prefix_source_step_body_v1 {
    ($exec:ident, $types:ident, $locals:ident, $statement:ident, $operations:ident) => {
        $exec!({
            match $statement.kind() {
                SemanticStatementKindV1::Nop if $operations == 0 => Ok(None),
                SemanticStatementKindV1::Assign(assign) => {
                    let destination = scalar_local($types, $locals, assign.destination())?;
                    if assign.value().result_type().index() != assign.destination().ty().index() {
                        return Err(CheckedU32PrefixErrorV1::Source);
                    }
                    let SemanticRvalueKindV1::Use(operand) = assign.value().kind() else {
                        return Err(CheckedU32PrefixErrorV1::Source);
                    };
                    let input = match operand {
                        SemanticOperandV1::Copy(place) if $operations == 0 => {
                            PrefixInput::Cell(scalar_local($types, $locals, place)?)
                        }
                        _ if $operations == 1 => {
                            let Some(value) = scalar_constant($types, operand) else {
                                return Err(CheckedU32PrefixErrorV1::Source);
                            };
                            PrefixInput::Constant(value)
                        }
                        _ => return Err(CheckedU32PrefixErrorV1::Source),
                    };
                    Ok(Some(PrefixStep { destination, input }))
                }
                _ => Err(CheckedU32PrefixErrorV1::Source),
            }
        })
    };
}
