//! Closed point-output slice custody. Source identity and physical index-space
//! authentication remain in the collector/ABI producer, not in this IR check.

use super::*;

pub(super) fn is_present(ir: &ReferenceEffectIrV1) -> bool {
    ir.relations.iter().any(|relation| {
        matches!(
            relation,
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D { .. }
            | ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { .. }
        )
    })
}

pub(super) fn validate(
    ir: &ReferenceEffectIrV1,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<(), ReferenceBindingErrorV1> {
    if !is_present(ir) {
        return Ok(());
    }
    let fail = || {
        ReferenceBindingErrorV1::new(
            "invocation output-slice reference requires immutable arguments, one direct point write, and no output reads or escapes",
        )
    };
    work.charge_v2(ir.relations.len())?;
    charge_reference_cfg_v1(ir, work)?;
    if ir.relations.len() != ir.argument_count as usize
        || !matches!(
            ir.relations.first(),
            Some(ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            })
        )
    {
        return Err(fail());
    }
    let mut output = None;
    for (index, relation) in ir.relations.iter().enumerate().skip(1) {
        let argument = match relation {
            ReferenceArgumentRelationV1::ScalarInput { argument, .. }
            | ReferenceArgumentRelationV1::SharedSliceInput { argument, .. } => *argument,
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D { argument, .. }
            | ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { argument, .. }
                if output.is_none() =>
            {
                output = Some(index as u32 + 1);
                *argument
            }
            _ => return Err(fail()),
        };
        if argument as usize != index - 1 {
            return Err(fail());
        }
    }
    let output = output.ok_or_else(fail)?;
    let operand = |value: &ReferenceOperandV1| match value {
        ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place) => place.local != output,
        ReferenceOperandV1::Constant(_) => true,
    };
    let mut writes = 0_usize;
    for block in &ir.blocks {
        for assignment in &block.assignments {
            work.charge_v2(1)?;
            let destination = &assignment.destination;
            if destination.local == output {
                if !matches!(
                    destination.projection.as_ref(),
                    [
                        ReferencePlaceProjectionV1::Dereference,
                        ReferencePlaceProjectionV1::Index(_),
                    ]
                ) {
                    return Err(fail());
                }
                writes += 1;
            } else if (destination.local != 0 && destination.local <= ir.argument_count)
                || !destination.projection.is_empty()
            {
                return Err(fail());
            }
            let valid =
                match &assignment.value {
                    ReferenceValueV1::Use(value)
                    | ReferenceValueV1::Unary { operand: value, .. }
                    | ReferenceValueV1::Cast { operand: value, .. } => operand(value),
                    ReferenceValueV1::Binary { lhs, rhs, .. } => operand(lhs) && operand(rhs),
                    ReferenceValueV1::InputLength { reference_argument } => {
                        matches!(ir.relations.get(*reference_argument as usize), Some(
                        ReferenceArgumentRelationV1::SharedSliceInput { .. }
                        | ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D { .. }
                        | ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { .. }
                    ))
                    }
                    ReferenceValueV1::SafeHelperCall { arguments, .. } => {
                        work.charge_v2(arguments.len())?;
                        arguments.iter().all(operand)
                    }
                };
            if !valid {
                return Err(fail());
            }
        }
        work.charge_v2(1)?;
        let valid = match &block.terminator {
            ReferenceTerminatorV1::Return | ReferenceTerminatorV1::Goto { .. } => true,
            ReferenceTerminatorV1::Switch { discriminant, .. } => operand(discriminant),
            ReferenceTerminatorV1::Assert {
                condition,
                bounds_check,
                ..
            } => {
                operand(condition)
                    && bounds_check
                        .as_ref()
                        .is_none_or(|check| operand(&check.index) && operand(&check.length))
            }
        };
        if !valid {
            return Err(fail());
        }
    }
    if writes != 1 {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn coordinate(
    ir: &ReferenceEffectIrV1,
    argument: u32,
    coordinate: ReferenceOutputCoordinateV1,
    guard: &ReferencePathPredicateV1,
    resolver: &ReferenceExpressionResolverV1<'_>,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<ReferenceOutputCoordinateV1, ReferenceBindingErrorV1> {
    if !ir.relations.iter().any(|relation| {
        matches!(relation,
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D { argument: actual, .. }
            | ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { argument: actual, .. }
                if *actual == argument
        )
    }) {
        return Ok(coordinate);
    }
    match coordinate {
        ReferenceOutputCoordinateV1::Dynamic(
            point @ ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
        ) => Ok(ReferenceOutputCoordinateV1::LogicalPoint(
            vec![point].into_boxed_slice(),
        )),
        ReferenceOutputCoordinateV1::Dynamic(expression) => {
            work.charge_v2(ir.relations.len())?;
            if !ir.relations.iter().any(|relation| matches!(relation,
                ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { argument: actual, .. }
                    if *actual == argument
            )) {
                return Err(ReferenceBindingErrorV1::new(
                    "invocation output-slice write is not its exact source point coordinate",
                ));
            }
            mapped_coordinate_v1::from_source(ir, &expression, guard, resolver, work)
        }
        _ => Err(ReferenceBindingErrorV1::new(
            "invocation output-slice write is not its exact source point coordinate",
        )),
    }
}

mod mapped_coordinate_v1;

#[cfg(test)]
#[path = "output_slice_v1/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "output_slice_v1/exclusive_tests.rs"]
mod exclusive_tests;
