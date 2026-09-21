use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as Origin,
    CheckedCanonicalKirInductionRefinementV1 as Pair,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirDefinitionCoordinateV1 as Definition, CheckedBinaryOperator, Constant,
};

/// Inert association for one original operation and all of its final outputs.
/// The original source coordinate is not a newly issued Rust span. A split's
/// false output is synthetic and has only the old overflow definition as origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionInductionRefinementOriginV1 {
    canonical: Origin,
    original_source: Site,
    synthetic_overflow: Option<Definition>,
}
impl ProductionInductionRefinementOriginV1 {
    /// Complete original-order one-to-one or one-to-two canonical coordinates.
    pub const fn canonical_origin(self) -> Origin {
        self.canonical
    }
    /// The original operation's optional semantic statement, never a new span.
    pub const fn original_source_statement(self) -> Site {
        self.original_source
    }
    /// Exact old result-1 definition for a synthetic false, absent otherwise.
    pub const fn synthetic_overflow_definition(self) -> Option<Definition> {
        self.synthetic_overflow
    }
    /// A synthetic false operation never acquires a source statement or span.
    pub const fn synthetic_false_source_statement(self) -> Site {
        None
    }
}
type SourceOrigin = ProductionInductionRefinementOriginV1;

fn source_origin(row: Origin, source: Site) -> SourceOrigin {
    SourceOrigin {
        canonical: row,
        original_source: source,
        synthetic_overflow: match row {
            Origin::Unchanged { .. } => None,
            Origin::CheckedAddSplit { input, .. } => Some(Definition::Result {
                operation: input,
                result: 1,
            }),
        },
    }
}
// Retained rows are allocated before the UnitLocal callback. This function may
// fill those paid slots, but never allocate their backing or export scratch sites.
pub(in super::super) fn check_induction_refinement_sites(
    sites: CheckedPromotedSites<'_, '_>,
    pair: &Pair<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    origins: &mut Vec<SourceOrigin>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    with_checked_induction_refinement_sites(
        sites,
        pair,
        output,
        origins,
        budget,
        binding,
        |sites, budget, binding| census_induction_refinement_sites(sites, pair, budget, binding),
    )
}

pub(in super::super) fn with_checked_induction_refinement_sites<'g, 'w, R>(
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
    if !std::ptr::eq(pair.input(), input.owner())
        || !std::ptr::eq(pair.output(), output.owner())
        || pair.origins().len() != input.operations().len()
        || sites.statements().len() != input.operations().len()
        || sites.traps().len() != input.operations().len()
        || input.owner().module().kernels.len() != output.owner().module().kernels.len()
        || !origins.is_empty()
        || origins.capacity() < input.operations().len()
    {
        return Err(refused(
            "checked induction refinement",
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
    let count = output.operations().len();
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
    for (ordinal, (row, original)) in pair.origins().iter().zip(input.operations()).enumerate() {
        budget.charge_work(24)?;
        if row.input() != original.coordinate {
            return Err(refused(
                "checked induction refinement",
                "ordered actual original occurrence",
            )
            .into());
        }
        let (destination, extra) = match *row {
            Origin::Unchanged {
                input: _,
                output: site,
            } => {
                let destination = operation_ordinal(output, site)?;
                if output.operations()[destination].coordinate != site
                    || output.operations()[destination].operation != original.operation
                {
                    return Err(refused(
                        "checked induction refinement",
                        "unchanged operation source association",
                    )
                    .into());
                }
                traps[destination] = sites.traps()[ordinal];
                (destination, None)
            }
            Origin::CheckedAddSplit {
                input: site,
                sum_output,
                false_output,
                ..
            } => {
                let destination = operation_ordinal(output, sum_output)?;
                let extra = operation_ordinal(output, false_output)?;
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = original.operation.kind
                else {
                    return Err(refused(
                        "checked induction refinement",
                        "original CheckedAdd source association",
                    )
                    .into());
                };
                let sum = output.operations()[destination].operation;
                let flag = output.operations()[extra].operation;
                if sites.traps()[ordinal]
                    || site.block != sum_output.block
                    || site.block != false_output.block
                    || output.operations()[destination].coordinate != sum_output
                    || output.operations()[extra].coordinate != false_output
                    || destination.checked_add(1) != Some(extra)
                    || original.operation.results.len() != 2
                    || sum.kind
                        != (OperationKind::Binary {
                            op: BinaryOp::Add,
                            lhs,
                            rhs,
                        })
                    || sum.results.as_slice() != &original.operation.results[..1]
                    || flag.kind != OperationKind::Constant(Constant::Bool(false))
                    || flag.results.as_slice() != &original.operation.results[1..]
                {
                    return Err(refused(
                        "checked induction refinement",
                        "exact nontrapping sum and synthetic false",
                    )
                    .into());
                }
                (destination, Some(extra))
            }
        };
        if seen[destination] {
            return Err(refused(
                "checked induction refinement",
                "unique final source occurrence",
            )
            .into());
        }
        seen[destination] = true;
        statements[destination] = sites.statements()[ordinal];
        if let Some(extra) = extra {
            if seen[extra] {
                return Err(refused(
                    "checked induction refinement",
                    "unique synthetic false occurrence",
                )
                .into());
            }
            seen[extra] = true;
            // No source statement or trap is assigned to this synthetic value.
            statements[extra] = None;
            traps[extra] = false;
        }
        budget.charge_work(1)?;
        if origins.len() == origins.capacity() {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        origins.push(source_origin(*row, sites.statements()[ordinal]));
    }
    for present in seen {
        budget.charge_work(1)?;
        if !present {
            return Err(refused(
                "checked induction refinement",
                "complete final source coverage",
            )
            .into());
        }
    }
    binding.check(budget)?;
    use_sites(
        CheckedPromotedSites {
            source: sites.source,
            output,
            output_sites: &statements,
            traps: &traps,
        },
        budget,
        binding,
    )
}

#[cfg(test)]
#[path = "production_checked_output_induction_refinement_sites_v1_tests.rs"]
mod tests;
