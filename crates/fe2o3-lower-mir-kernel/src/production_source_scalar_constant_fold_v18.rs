// Source endpoints may straddle a constant fold. Canonicalize only pure integer
// constants, using the KIR evaluator; ranked operator congruence stays unchanged.
fn source_scalar_constant_fold_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_ir::scalar_ops_v2 as scalar;
    let frame = argument_sum_v1(&[
        size_of::<&mut NormalizedScalarExpressionV1>(),
        size_of::<NormalizedScalarExpressionV1>(),
        2 * size_of::<Option<&mut NormalizedScalarExpressionV1>>(),
        size_of::<
            Option<(
                &mut NormalizedScalarExpressionV1,
                &mut NormalizedScalarExpressionV1,
            )>,
        >(),
        size_of::<&mut dyn CorrelationChargeV18>(),
        size_of::<&mut SourceCorrelationChargeV18<'_, '_, '_, '_>>()
            .max(size_of::<&mut SourceTranslationChargeV18<'_, '_, '_, '_>>()),
        2 * size_of::<usize>(),
        size_of::<Option<usize>>(),
        3 * size_of::<&mut NormalizedScalarNodeV18>(),
        size_of::<&mut ProductionSemanticBinaryOpV2>(),
        size_of::<&mut ProductionSemanticScalarTypeV2>(),
        size_of::<&mut ProductionOverflowContractV2>(),
        2 * size_of::<&NormalizedScalarExpressionV1>(),
        2 * size_of::<&ProductionSemanticScalarTypeV2>(),
        2 * size_of::<&u64>(),
        3 * size_of::<ProductionSemanticScalarTypeV2>(),
        size_of::<ProductionSemanticBinaryOpV2>(),
        size_of::<ProductionOverflowContractV2>(),
        size_of::<scalar::ScalarType>(),
        size_of::<scalar::IntWidth>(),
        size_of::<scalar::IntBinary>(),
        size_of::<scalar::IntMode>(),
        size_of::<scalar::ShiftDirection>(),
        size_of::<scalar::ShiftPolicy>(),
        size_of::<scalar::Operation>(),
        size_of::<scalar::FloatCapabilities>(),
        size_of::<Vec<scalar::Diagnostic>>(),
        size_of::<Result<(), Vec<scalar::Diagnostic>>>(),
        size_of::<Option<scalar::IntOutcome>>(),
        size_of::<Option<u64>>(),
        size_of::<Option<()>>(),
        size_of::<Result<u64, std::num::TryFromIntError>>(),
        3 * size_of::<u64>(),
        3 * size_of::<u128>(),
        size_of::<u16>(),
        size_of::<bool>(),
    ])?;
    argument_sum_v1(&[
        argument_product_v1(MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1, frame)?,
        source_scalar_integer_evaluator_headers_v18()?,
    ])
}

fn source_scalar_integer_evaluator_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_ir::scalar_ops_v2 as scalar;
    // One evaluator is live after the child traversals return. Include both
    // fixed binary/shift frames and their width/sign helper return carriers.
    argument_sum_v1(&[
        size_of::<(
            scalar::ScalarType,
            scalar::IntBinary,
            scalar::IntMode,
            u128,
            u128,
        )>(),
        size_of::<(
            scalar::ScalarType,
            scalar::ScalarType,
            scalar::ShiftDirection,
            scalar::ShiftPolicy,
            u128,
            u128,
        )>(),
        4 * size_of::<Option<(scalar::IntWidth, bool)>>(),
        4 * size_of::<(scalar::IntWidth, bool)>(),
        2 * size_of::<(i128, i128)>(),
        2 * size_of::<(i128, bool)>(),
        2 * size_of::<(u128, bool)>(),
        2 * size_of::<(u128, bool, u128)>(),
        2 * size_of::<Option<scalar::IntOutcome>>(),
        2 * size_of::<scalar::IntOutcome>(),
        4 * size_of::<scalar::IntWidth>(),
        2 * size_of::<scalar::ScalarType>(),
        20 * size_of::<u128>(),
        12 * size_of::<i128>(),
        8 * size_of::<bool>(),
        4 * size_of::<u16>(),
        2 * size_of::<u32>(),
    ])
}

fn source_scalar_constant_fold_v18(
    expression: &mut NormalizedScalarExpressionV1,
    depth: usize,
    charge: &mut dyn CorrelationChargeV18,
) -> Option<()> {
    charge.charge()?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return None;
    }
    let next = depth.checked_add(1)?;
    match expression {
        NormalizedScalarExpressionV1::Unary { operand, .. }
        | NormalizedScalarExpressionV1::Cast { operand, .. } => {
            source_scalar_constant_fold_v18(&mut operand.0[0], next, charge)?;
        }
        NormalizedScalarExpressionV1::Compare { lhs, rhs, .. }
        | NormalizedScalarExpressionV1::Binary { lhs, rhs, .. } => {
            source_scalar_constant_fold_v18(&mut lhs.0[0], next, charge)?;
            source_scalar_constant_fold_v18(&mut rhs.0[0], next, charge)?;
        }
        NormalizedScalarExpressionV1::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            source_scalar_constant_fold_v18(&mut condition.0[0], next, charge)?;
            source_scalar_constant_fold_v18(&mut when_true.0[0], next, charge)?;
            source_scalar_constant_fold_v18(&mut when_false.0[0], next, charge)?;
        }
        NormalizedScalarExpressionV1::Symbol { .. }
        | NormalizedScalarExpressionV1::Constant { .. }
        | NormalizedScalarExpressionV1::Load { .. } => (),
    }
    if let NormalizedScalarExpressionV1::Binary {
        operation,
        scalar,
        overflow,
        lhs,
        rhs,
    } = expression
    {
        if let Some(bits) =
            source_scalar_binary_constant_v18(*operation, *scalar, *overflow, lhs, rhs)
        {
            // Descendant allocations drop now. Their credits remain owned by
            // the enclosing normalizer until both complete trees have dropped.
            *expression = NormalizedScalarExpressionV1::Constant {
                scalar: *scalar,
                bits,
            };
        }
    }
    Some(())
}

fn source_scalar_binary_constant_v18(
    operation: ProductionSemanticBinaryOpV2,
    scalar: ProductionSemanticScalarTypeV2,
    overflow: ProductionOverflowContractV2,
    lhs: &NormalizedScalarExpressionV1,
    rhs: &NormalizedScalarExpressionV1,
) -> Option<u64> {
    use ProductionSemanticBinaryOpV2 as Binary;
    use fe2o3_kernel_ir::scalar_ops_v2 as eval;
    let ProductionSemanticScalarTypeV2::Integer { signed, bits } = scalar else {
        return None;
    };
    let width = match bits {
        8 => eval::IntWidth::W8,
        16 => eval::IntWidth::W16,
        32 => eval::IntWidth::W32,
        64 => eval::IntWidth::W64,
        _ => return None,
    };
    let (
        NormalizedScalarExpressionV1::Constant {
            scalar: left_scalar,
            bits: left,
        },
        NormalizedScalarExpressionV1::Constant {
            scalar: right_scalar,
            bits: right,
        },
    ) = (lhs, rhs)
    else {
        return None;
    };
    if *left_scalar != scalar
        || *right_scalar != scalar
        || (bits < 64 && (*left >> bits != 0 || *right >> bits != 0))
    {
        return None;
    }
    if overflow == ProductionOverflowContractV2::Checked
        && !matches!(operation, Binary::Add | Binary::Subtract | Binary::Multiply)
    {
        return None;
    }
    let ty = eval::ScalarType::Int { width, signed };
    let result = match operation {
        Binary::ShiftLeft | Binary::ShiftRight => eval::evaluate_shift(
            ty,
            ty,
            if operation == Binary::ShiftLeft {
                eval::ShiftDirection::Left
            } else {
                eval::ShiftDirection::Right
            },
            // KIR shifts are partial at an invalid width. Do not equate them
            // to a wrapping shift solely because the scalar transcript wraps.
            eval::ShiftPolicy::Checked,
            u128::from(*left),
            u128::from(*right),
        ),
        _ => {
            let op = match operation {
                Binary::Add => eval::IntBinary::Add,
                Binary::Subtract => eval::IntBinary::Sub,
                Binary::Multiply => eval::IntBinary::Mul,
                Binary::Divide => eval::IntBinary::Div,
                Binary::Remainder => eval::IntBinary::Rem,
                Binary::BitAnd => eval::IntBinary::And,
                Binary::BitOr => eval::IntBinary::Or,
                Binary::BitXor => eval::IntBinary::Xor,
                Binary::ShiftLeft | Binary::ShiftRight => unreachable!(),
            };
            let mode = if overflow == ProductionOverflowContractV2::Checked
                || matches!(operation, Binary::Divide | Binary::Remainder)
            {
                eval::IntMode::Checked
            } else {
                eval::IntMode::Wrapping
            };
            eval::evaluate_integer_binary(ty, op, mode, u128::from(*left), u128::from(*right))
        }
    };
    match result {
        Some(eval::IntOutcome::Value(value)) => u64::try_from(value).ok(),
        // Checked overflow, partial division, and invalid shifts retain their
        // operator. Neither a trap nor an absent value is a scalar constant.
        Some(
            eval::IntOutcome::CheckedNone
            | eval::IntOutcome::Trap
            | eval::IntOutcome::Overflowing { .. },
        )
        | None => None,
    }
}
