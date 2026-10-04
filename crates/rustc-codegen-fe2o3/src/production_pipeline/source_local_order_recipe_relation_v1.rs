//! Bounded observation of the actual checked current I/L occurrence relation.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirOperationOriginV1 as Origin, InertCanonicalKirTransitionReceiptV1, Operation,
};
use fe2o3_kernel_opt::U32LocalOrderRegionV1 as Region;

fn selected(owner: &Owner, region: Region) -> Result<&[Operation], String> {
    if region.operation_count != 3 {
        return Err("local-order recipe requires three actual operations".into());
    }
    let first =
        usize::try_from(region.first_operation).map_err(|_| "local-order operation coordinate")?;
    let end = first
        .checked_add(3)
        .ok_or("local-order operation range overflow")?;
    owner
        .module()
        .functions
        .get(region.block.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(region.block.block as usize))
        .and_then(|block| block.operations.get(first..end))
        .ok_or_else(|| "local-order recipe actual selected region unavailable".into())
}
pub(super) fn observe(
    input: &Owner,
    output: &Owner,
    region: Region,
    receipt: &InertCanonicalKirTransitionReceiptV1,
    budget: &mut Budget<'_>,
) -> Result<(codec::Relation, [u32; 3]), String> {
    if region.expected_input != *input.canonical().identity()
        || !receipt
            .input_identity()
            .matches_verified(input.canonical().identity())
        || !receipt
            .output_identity()
            .matches_verified(output.canonical().identity())
    {
        return Err("local-order recipe actual I/L identity mismatch".into());
    }
    let rows = receipt.candidate().operations;
    let work = input
        .canonical()
        .canonical_bytes()
        .len()
        .checked_add(output.canonical().canonical_bytes().len())
        .and_then(|n| rows.len().checked_mul(3).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(64))
        .ok_or("local-order relation work overflow")?;
    budget
        .charge_work(work)
        .map_err(|error| error.to_string())?;
    let before = selected(input, region)?;
    let after = selected(output, region)?;
    let mut positions = [0_usize; 3];
    for (role, opcode) in [BinaryOp::BitXor, BinaryOp::BitOr, BinaryOp::BitAnd]
        .into_iter()
        .enumerate()
    {
        if !matches!(before[role].kind, OperationKind::Binary { op, .. } if op == opcode) {
            return Err("local-order recipe current I source role changed".into());
        }
        let operation = region
            .first_operation
            .checked_add(role as u32)
            .ok_or("local-order relation coordinate overflow")?;
        let input_coordinate = CanonicalKirOperationCoordinateV1 {
            block: region.block,
            operation,
        };
        let mut matched = None;
        for row in rows {
            if row.origin == Origin::Retained(input_coordinate)
                && matched.replace(row.output).is_some()
            {
                return Err("local-order recipe duplicate current occurrence".into());
            }
        }
        let coordinate = matched.ok_or("local-order recipe current occurrence absent")?;
        if coordinate.block != region.block {
            return Err("local-order recipe occurrence left region".into());
        }
        let local = coordinate
            .operation
            .checked_sub(region.first_operation)
            .ok_or("local-order recipe occurrence before region")? as usize;
        let actual = after
            .get(local)
            .ok_or("local-order recipe occurrence after region")?;
        if actual != &before[role] {
            return Err("local-order recipe retained operation payload changed".into());
        }
        positions[role] = local;
    }
    if positions[2] != 2 || !matches!((positions[0], positions[1]), (0, 1) | (1, 0)) {
        return Err("local-order recipe actual order violates supported diamond".into());
    }
    let mut results = [0_u32; 3];
    for (index, operation) in after.iter().enumerate() {
        let [result] = operation.results.as_slice() else {
            return Err("local-order recipe output result arity".into());
        };
        results[index] = result.id.0;
    }
    Ok((
        if positions[0] < positions[1] {
            codec::Relation::XorBeforeOr
        } else {
            codec::Relation::OrBeforeXor
        },
        results,
    ))
}
