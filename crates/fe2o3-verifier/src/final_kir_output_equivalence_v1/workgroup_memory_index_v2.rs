use super::*;

#[cfg(test)]
mod tests;

pub(super) fn issue(
    capability: &fe2o3_kernel_ir::ExecutionCapabilityOpV1,
    state: &ExecutionStateV1,
    intrinsics: &mut BTreeSet<IntrinsicKind>,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
) -> Result<SymbolicValueV1, FinalKirOutputEquivalenceErrorV1> {
    let [receiver] = capability.operands.as_slice() else {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    };
    if !matches!(state.values.get(receiver), Some(SymbolicValueV1::Opaque)) {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let size = workgroup
        .filter(|s| {
            s.x != 0
                && s.y != 0
                && s.z != 0
                && u64::from(s.x)
                    .checked_mul(u64::from(s.y))
                    .and_then(|xy| xy.checked_mul(u64::from(s.z)))
                    .is_some()
        })
        .ok_or(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch)?;
    let mut local = |axis| {
        let kind = IntrinsicKind::InvocationIndex {
            kind: fe2o3_kernel_ir::IndexKind::Local,
            axis,
        };
        intrinsics.insert(kind);
        ExpressionV1::leaf(ScalarV1::Unsigned(64), ExpressionKindV1::Intrinsic(kind))
    };
    let x = local(fe2o3_kernel_ir::Axis::X);
    if size.y == 1 && size.z == 1 {
        return Ok(SymbolicValueV1::WorkgroupIndex(x));
    }
    let y = local(fe2o3_kernel_ir::Axis::Y);
    let z = local(fe2o3_kernel_ir::Axis::Z);
    let size_x = constant_expression(&Constant::Index(u64::from(size.x)))?;
    let size_y = constant_expression(&Constant::Index(u64::from(size.y)))?;
    let scalar = ScalarV1::Unsigned(64);
    let zy = ExpressionV1::binary(scalar, BinaryOp::Multiply, z, size_y)?;
    let zy = ExpressionV1::binary(scalar, BinaryOp::Add, zy, y)?;
    let row = ExpressionV1::binary(scalar, BinaryOp::Multiply, zy, size_x)?;
    Ok(SymbolicValueV1::WorkgroupIndex(ExpressionV1::binary(
        scalar,
        BinaryOp::Add,
        row,
        x,
    )?))
}

pub(super) fn into_disjoint(
    capability: &fe2o3_kernel_ir::ExecutionCapabilityOpV1,
    state: &ExecutionStateV1,
) -> Result<SymbolicValueV1, FinalKirOutputEquivalenceErrorV1> {
    let [input] = capability.operands.as_slice() else {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    };
    match state.values.get(input) {
        Some(value @ SymbolicValueV1::WorkgroupIndex(_)) => Ok(value.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}
