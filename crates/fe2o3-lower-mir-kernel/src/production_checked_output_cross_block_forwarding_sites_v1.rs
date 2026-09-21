use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingOriginV1 as Origin,
    CheckedCanonicalKirCrossBlockForwardingV1 as Pair,
};
use fe2o3_kernel_ir::BinaryOp;

/// Inert lineage for one actual original operation at its unchanged coordinate.
/// A selected replacement keeps the Load's original source statement; its Store
/// evidence is a canonical operation coordinate and never substitutes a span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCrossBlockForwardingOriginV1 {
    canonical: Origin,
    original_source: Site,
}

#[cfg(test)]
pub(in super::super) use tests::exercise_cross_block_source_refusals;
impl ProductionCrossBlockForwardingOriginV1 {
    /// Exact original/output coordinates and optional initializing Store.
    pub const fn canonical_origin(self) -> Origin {
        self.canonical
    }
    /// The unchanged original semantic statement, not a newly issued Rust span.
    pub const fn original_source_statement(self) -> Site {
        self.original_source
    }
}
type SourceOrigin = ProductionCrossBlockForwardingOriginV1;

// Retained row backing was paid outside UnitLocal erasure. The borrowed final
// metadata and all scratch remain inside this callback, followed by fresh census.
pub(in super::super) fn check_cross_block_forwarding_sites(
    sites: CheckedPromotedSites<'_, '_>,
    pair: &Pair<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    origins: &mut Vec<SourceOrigin>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    with_checked_cross_block_forwarding_sites(
        sites,
        pair,
        output,
        origins,
        budget,
        binding,
        |sites, budget, binding| {
            census_sites_named(sites, "cross-block private forwarding", budget, binding)
        },
    )
}

pub(in super::super) fn with_checked_cross_block_forwarding_sites<'g, 'w, R>(
    sites: CheckedPromotedSites<'_, '_>,
    pair: &Pair<'_>,
    output: &CanonicalKirInventoryV1<'g>,
    origins: &mut Vec<SourceOrigin>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        CheckedPromotedSites<'s, 'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> PResult<R> {
    budget.charge_work(10)?;
    let input = sites.output();
    let count = input.operations().len();
    if !std::ptr::eq(pair.input(), input.owner())
        || !std::ptr::eq(pair.output(), output.owner())
        || pair.origins().len() != count
        || output.operations().len() != count
        || sites.statements().len() != count
        || sites.traps().len() != count
        || !origins.is_empty()
        || origins.capacity() < count
    {
        return Err(refused(
            "cross-block private forwarding",
            "actual endpoints and complete paid source slots",
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
        return Err(refused(
            "cross-block private forwarding",
            "complete actual kernel roster",
        )
        .into());
    }
    for (ordinal, ((row, original), final_row)) in pair
        .origins()
        .iter()
        .zip(input.operations())
        .zip(output.operations())
        .enumerate()
    {
        budget.charge_work(24)?;
        if row.input != original.coordinate
            || row.output != final_row.coordinate
            || row.input != row.output
        {
            return Err(refused(
                "cross-block private forwarding",
                "complete unchanged operation coordinates",
            )
            .into());
        }
        if let Some(store) = row.store {
            let OperationKind::Load { .. } = original.operation.kind else {
                return Err(refused(
                    "cross-block private forwarding",
                    "selected original Load occurrence",
                )
                .into());
            };
            let store_ordinal = operation_ordinal(input, store)?;
            let OperationKind::Store { value, .. } =
                input.operations()[store_ordinal].operation.kind
            else {
                return Err(refused(
                    "cross-block private forwarding",
                    "exact original initializing Store",
                )
                .into());
            };
            if sites.statements()[ordinal].is_none()
                || sites.traps()[ordinal]
                || store.block.function != row.input.block.function
                || store.block == row.input.block
                || final_row.operation.results != original.operation.results
                || final_row.operation.kind
                    != (OperationKind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: value,
                        rhs: value,
                    })
            {
                return Err(refused(
                    "cross-block private forwarding",
                    "nontrapping copy with original Load source statement",
                )
                .into());
            }
        } else if final_row.operation != original.operation {
            return Err(refused(
                "cross-block private forwarding",
                "exact unchanged retained operation",
            )
            .into());
        }
        budget.charge_work(1)?;
        if origins.len() == origins.capacity() {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        origins.push(SourceOrigin {
            canonical: *row,
            original_source: sites.statements()[ordinal],
        });
    }
    binding.check(budget)?;
    // No operation or ordinal was inserted or moved. Reuse the exact checked
    // statement/trap slices, with only the independent pair's actual final owner.
    use_sites(
        CheckedPromotedSites {
            source: sites.source,
            output,
            output_sites: sites.output_sites,
            traps: sites.traps,
        },
        budget,
        binding,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    impl SourceOrigin {
        pub(crate) fn changed_source_for_forwarding_test(self) -> Self {
            Self {
                original_source: None,
                ..self
            }
        }
        pub(crate) fn other_source_for_forwarding_test(self, other: Self) -> Self {
            Self {
                original_source: other.original_source,
                ..self
            }
        }
    }
    pub(in super::super::super) fn exercise_cross_block_source_refusals(
        sites: CheckedPromotedSites<'_, '_>,
        pair: &Pair<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
        binding: &PromotionBinding,
    ) -> PResult<()> {
        let selected = pair
            .origins()
            .iter()
            .position(|row| row.store.is_some())
            .expect("real selected Load");
        for missing_source in [false, true] {
            let mut statements = scratch::<Site>(sites.statements().len(), budget)?;
            statements.extend_from_slice(sites.statements());
            let mut traps = scratch::<bool>(sites.traps().len(), budget)?;
            traps.extend_from_slice(sites.traps());
            let mut rows = scratch::<SourceOrigin>(pair.origins().len(), budget)?;
            if missing_source {
                statements[selected] = None;
            } else {
                traps[selected] = true;
            }
            let result = check_cross_block_forwarding_sites(
                CheckedPromotedSites {
                    source: sites.source,
                    output: sites.output,
                    output_sites: &statements,
                    traps: &traps,
                },
                pair,
                output,
                &mut rows,
                budget,
                binding,
            );
            assert!(matches!(
                result,
                Err(PError::Admission(E::Unsupported {
                    phase: "cross-block private forwarding",
                    detail: "nontrapping copy with original Load source statement",
                }))
            ));
        }
        binding.check(budget)
    }
}
