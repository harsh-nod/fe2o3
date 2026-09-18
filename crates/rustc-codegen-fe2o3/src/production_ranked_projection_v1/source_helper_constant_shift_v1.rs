fn source_helper_constant_shift_v1(
    types: &[SemanticTypeDeclV1],
    right: &SemanticOperandV1,
    result: Scalar,
) -> bool {
    let Scalar::Integer {
        bits: width @ (8 | 16 | 32 | 64),
        ..
    } = result
    else {
        return false;
    };
    let SemanticOperandV1::Constant(constant) = right else {
        return false;
    };
    let Ok(Scalar::Integer {
        signed,
        bits: rhs_width,
    }) = scalar(types, constant.ty())
    else {
        return false;
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return false;
    };
    u16::from(value.size_bytes()) * 8 == rhs_width
        && (!signed || value.bits() & (1_u128 << (rhs_width - 1)) == 0)
        && value.bits() < u128::from(width)
}
