fn native_helper_constant_shift_v1(
    rows: &[Binding<'_>],
    rhs: ValueId,
    ty: &Type,
    meter: &mut dyn Meter,
) -> Result<bool, Error> {
    fn literal(
        rows: &[Binding<'_>],
        id: ValueId,
        ty: &Type,
        meter: &mut dyn Meter,
    ) -> Result<Option<u64>, Error> {
        meter.work(4)?;
        let row = &rows[find(rows, id, meter)?];
        if row.ty != ty {
            return Ok(None);
        }
        let Some(Operation {
            kind: OperationKind::Constant(constant),
            ..
        }) = row.producer
        else {
            return Ok(None);
        };
        let Some((scalar, bits)) = normalize_kir_constant_v1(constant) else {
            return Ok(None);
        };
        Ok(fixed_shift_literal_v1(
            &NormalizedScalarExpressionV1::Constant { scalar, bits },
        ))
    }
    meter.work(4)?;
    let Some(width) = kir_semantic_scalar_v1(ty).and_then(fixed_shift_width_v1) else {
        return Ok(false);
    };
    if let Some(count) = literal(rows, rhs, ty, meter)? {
        return Ok(count < u64::from(width));
    }
    let row = &rows[find(rows, rhs, meter)?];
    if row.ty != ty {
        return Ok(false);
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
    let count = literal(rows, *lhs, ty, meter)?;
    let mask = literal(rows, *rhs, ty, meter)?;
    Ok(count.is_some_and(|count| count < u64::from(width)) && mask == Some(u64::from(width) - 1))
}
