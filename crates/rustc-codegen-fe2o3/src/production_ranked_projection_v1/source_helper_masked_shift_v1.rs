fn source_helper_masked_shift_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block: usize,
    statement: usize,
    value: &SemanticRvalueV1,
    result: Scalar,
) -> bool {
    let Scalar::Integer {
        bits: width @ (8 | 16 | 32 | 64),
        ..
    } = result
    else {
        return false;
    };
    let Some(block) = function.blocks().get(block) else {
        return false;
    };
    let Some(current) = block.statements().get(statement) else {
        return false;
    };
    let SemanticStatementKindV1::Assign(current) = current.kind() else {
        return false;
    };
    if !std::ptr::eq(current.value(), value) || current.destination().ty() != value.result_type() {
        return false;
    }
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight,
        left,
        right: SemanticOperandV1::Copy(count) | SemanticOperandV1::Move(count),
    } = value.kind()
    else {
        return false;
    };
    if left.ty() != value.result_type()
        || !count.projections().is_empty()
        || function
            .locals()
            .get(count.local().index() as usize)
            .map(|local| local.ty())
            != Some(count.ty())
    {
        return false;
    }
    let Ok(Scalar::Integer { signed, bits }) = scalar(types, count.ty()) else {
        return false;
    };
    let Some(previous) = statement
        .checked_sub(1)
        .and_then(|index| block.statements().get(index))
    else {
        return false;
    };
    let SemanticStatementKindV1::Assign(mask) = previous.kind() else {
        return false;
    };
    if mask.destination() != count || mask.value().result_type() != count.ty() {
        return false;
    }
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::BitAnd,
        left: input,
        right: SemanticOperandV1::Constant(limit),
    } = mask.value().kind()
    else {
        return false;
    };
    if input.ty() != count.ty() || limit.ty() != count.ty() {
        return false;
    }
    let SemanticConstantValueV1::Scalar(limit) = limit.value() else {
        return false;
    };
    u16::from(limit.size_bytes()) * 8 == bits
        && (!signed || limit.bits() & (1_u128 << (bits - 1)) == 0)
        && limit.bits() == u128::from(width - 1)
}
