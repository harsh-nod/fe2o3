use super::*;

pub(super) fn from_source(
    ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    guard: &ReferencePathPredicateV1,
    resolver: &ReferenceExpressionResolverV1<'_>,
    work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<ReferenceOutputCoordinateV1, ReferenceBindingErrorV1> {
    if !std::ptr::eq(ir, resolver.effect_ir) {
        return Err(ReferenceBindingErrorV1::new(
            "compact row assertion resolver belongs to another source owner",
        ));
    }
    // Normalization erases Assert origins. Audit the complete original IR,
    // independently of reachability and without using path clauses as facts.
    for block in &ir.blocks {
        work.charge_v2(1)?;
        let ReferenceTerminatorV1::Assert {
            condition,
            expected,
            bounds_check: None,
            ..
        } = &block.terminator
        else {
            continue;
        };
        let condition = resolve_predicate_operand_v1(resolver, condition, work)?;
        work.charge_expression_v2(&condition)?;
        if !matches!(reference_constant_bits_v2(&condition),
            Some((ReferenceScalarTypeV1::Bool, bits @ (0 | 1)))
                if (bits == 1) == *expected)
        {
            return Err(ReferenceBindingErrorV1::new(
                "mapped output requires every raw nonbounds assertion to succeed independently as a constant Bool",
            ));
        }
    }
    CompactRowsUsize1D::from_expression(expression, guard, work)?
        .map(ReferenceOutputCoordinateV1::CompactRowsUsize1D)
        .ok_or_else(|| ReferenceBindingErrorV1::new(
            "exclusive output-slice write is not its exact source point coordinate or guarded compact row",
        ))
}
