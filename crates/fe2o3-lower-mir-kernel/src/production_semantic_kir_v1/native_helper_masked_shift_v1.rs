fn native_helper_masked_shift_v1(
    rows: &[Binding<'_>],
    rhs: ValueId,
    ty: &Type,
    meter: &mut dyn Meter,
) -> Result<bool, Error> {
    meter.work(24)?;
    let Some(width) = kir_semantic_scalar_v1(ty).and_then(fixed_shift_width_v1) else {
        return Ok(false);
    };
    let mut row = &rows[find(rows, rhs, meter)?];
    if row.ty != ty {
        return Ok(false);
    }
    let mut count_ty = ty;
    if let Some(Operation {
        kind: OperationKind::Cast { kind, value, to },
        ..
    }) = row.producer
    {
        if to != ty {
            return Ok(false);
        }
        row = &rows[find(rows, *value, meter)?];
        count_ty = row.ty;
        let (Some(from), Some(to)) = (count_ty.as_scalar(), ty.as_scalar()) else {
            return Ok(false);
        };
        if kir_semantic_scalar_v1(count_ty)
            .and_then(fixed_shift_width_v1)
            .is_none()
            || fe2o3_kernel_ir::plan_integer_cast_v1(from, to) != Some([Some((*kind, to)), None])
        {
            return Ok(false);
        }
    }
    let Some(Operation {
        kind:
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs,
                rhs,
            },
        ..
    }) = row.producer
    else {
        return Ok(false);
    };
    let input = &rows[find(rows, *lhs, meter)?];
    let limit = &rows[find(rows, *rhs, meter)?];
    if input.ty != count_ty || limit.ty != count_ty {
        return Ok(false);
    }
    let Some(Operation {
        kind: OperationKind::Constant(constant),
        ..
    }) = limit.producer
    else {
        return Ok(false);
    };
    let Some((scalar, bits)) = normalize_kir_constant_v1(constant) else {
        return Ok(false);
    };
    Ok(
        fixed_shift_literal_v1(&NormalizedScalarExpressionV1::Constant { scalar, bits })
            == Some(u64::from(width) - 1),
    )
}
