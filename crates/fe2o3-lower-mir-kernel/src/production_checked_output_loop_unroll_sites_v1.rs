use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirLoopUnrollCopyV1 as CopyRole, CanonicalKirLoopUnrollOriginV1 as Origin,
    CheckedCanonicalKirCrossBlockForwardingV1 as ForwardingPair,
    CheckedCanonicalKirInductionRefinementV1 as RefinementPair,
    CheckedCanonicalKirLoopUnrollPairV1 as Pair,
};

/// Inert complete operation occurrence, including an explicit zero-trip omission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLoopUnrollOriginV1 {
    canonical: Origin<CanonicalKirOperationCoordinateV1>,
    original_source: Site,
}
impl ProductionLoopUnrollOriginV1 {
    /// Checked original/destination coordinate and exact copy or omission role.
    pub const fn canonical_origin(self) -> Origin<CanonicalKirOperationCoordinateV1> {
        self.canonical
    }
    /// Original F source occurrence, never a donor span or a new source proof.
    /// An omitted row retains this inert provenance but grants no output site.
    pub const fn original_source_statement(self) -> Site {
        self.original_source
    }
    #[cfg(test)]
    pub(crate) fn without_source_for_unroll_test(self) -> Self {
        Self {
            original_source: None,
            ..self
        }
    }
    #[cfg(test)]
    pub(crate) fn donor_source_for_unroll_test(self, other: Self) -> Self {
        Self {
            original_source: other.original_source,
            ..self
        }
    }
    #[cfg(test)]
    pub(crate) fn wrong_copy_for_unroll_test(mut self) -> Self {
        self.canonical.copy = CopyRole::Body(255);
        self
    }
    #[cfg(test)]
    pub(crate) fn wrong_root_for_unroll_test(mut self) -> Self {
        self.canonical.input.block.function.0 = u32::MAX;
        self
    }
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn with_checked_loop_unroll_sites<'w, T>(
    sites: CheckedPromotedSites<'_, '_>,
    intermediate: &CanonicalKirInventoryV1<'_>,
    refinement: &RefinementPair<'_>,
    forwarding: &ForwardingPair<'_>,
    pair: &Pair<'_, '_, '_>,
    output: &CanonicalKirInventoryV1<'_>,
    origins: &mut Vec<ProductionLoopUnrollOriginV1>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    next: impl FnOnce(
        CheckedPromotedSites<'_, '_>,
        &CanonicalKirInventoryV1<'_>,
        &CanonicalKirInventoryV1<'_>,
        &RefinementPair<'_>,
        &ForwardingPair<'_>,
        &Pair<'_, '_, '_>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<T>,
) -> PResult<T> {
    budget.charge_work(12)?;
    let input = sites.output();
    let count = output.operations().len();
    let rows = pair.origins().operations;
    if !std::ptr::eq(pair.input(), input.owner())
        || !std::ptr::eq(pair.output(), output.owner())
        || !std::ptr::eq(forwarding.output(), input.owner())
        || sites.statements().len() != input.operations().len()
        || sites.traps().len() != input.operations().len()
        || !origins.is_empty()
        || origins.capacity() < rows.len()
    {
        return Err(refused(
            "bounded source unroll",
            "actual F/U owners and complete paid rows",
        )
        .into());
    }
    budget.charge_work(
        input
            .owner()
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.owner().canonical().canonical_bytes().len())
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    if input.owner().module().kernels != output.owner().module().kernels {
        return Err(refused("bounded source unroll", "unchanged complete kernel roster").into());
    }
    let mut statements = scratch::<Site>(count, budget)?;
    let mut traps = scratch::<bool>(count, budget)?;
    let mut seen = scratch::<bool>(count, budget)?;
    budget.charge_work(
        count
            .checked_mul(3)
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    statements.resize(count, None);
    traps.resize(count, false);
    seen.resize(count, false);
    for row in rows {
        budget.charge_work(15)?;
        let old = operation_ordinal(input, row.input)?;
        if input.operations()[old].coordinate != row.input {
            return Err(refused("bounded source unroll", "actual original occurrence").into());
        }
        match (row.output, row.copy) {
            (None, CopyRole::OmittedBody) => {}
            (Some(destination), CopyRole::Retained | CopyRole::Header(_) | CopyRole::Body(_)) => {
                let new = operation_ordinal(output, destination)?;
                if seen[new]
                    || output.operations()[new].coordinate != destination
                    || (row.copy != CopyRole::Retained && sites.traps()[old])
                {
                    return Err(refused(
                        "bounded source unroll",
                        "unique clone site and no cloned trap grant",
                    )
                    .into());
                }
                seen[new] = true;
                statements[new] = sites.statements()[old];
                traps[new] = sites.traps()[old];
            }
            _ => {
                return Err(
                    refused("bounded source unroll", "exact emitted or omitted role").into(),
                );
            }
        }
        if origins.len() == origins.capacity() {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        origins.push(ProductionLoopUnrollOriginV1 {
            canonical: *row,
            original_source: sites.statements()[old],
        });
    }
    for present in &seen {
        budget.charge_work(1)?;
        if !present {
            return Err(refused("bounded source unroll", "complete final operation census").into());
        }
    }
    binding.check(budget)?;
    next(
        CheckedPromotedSites {
            source: sites.source,
            output,
            output_sites: &statements,
            traps: &traps,
        },
        intermediate,
        input,
        refinement,
        forwarding,
        pair,
        budget,
        binding,
    )
}
