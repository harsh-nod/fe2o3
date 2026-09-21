use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirLicmV1 as LicmPair, CheckedCanonicalKirLoopPreheadersV1 as PreheaderPair,
};

#[cfg(test)]
pub(in super::super) fn check_licm_after_preheaders_sites(
    sites: CheckedPromotedSites<'_, '_>,
    preheaders: &PreheaderPair<'_>,
    licm: &LicmPair<'_>,
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    with_licm_after_preheaders_sites(
        sites,
        preheaders,
        licm,
        input,
        output,
        budget,
        binding,
        check_licm_sites,
    )
}

// Only the actual connected promotion -> preheaders -> LICM path may construct
// these final sites. The old neutral pair never describes the populated final
// preheader; the distinct LICM pair proves every moved/retained operation.
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn with_licm_after_preheaders_sites<'g, 'w, R>(
    sites: CheckedPromotedSites<'_, '_>,
    preheaders: &PreheaderPair<'_>,
    licm: &LicmPair<'_>,
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'g>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        CheckedPromotedSites<'s, 'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> PResult<R> {
    with_final_sites(
        sites,
        preheaders,
        licm,
        (input, output),
        budget,
        binding,
        use_sites,
    )
}

fn with_final_sites<'g, 'w, R>(
    sites: CheckedPromotedSites<'_, '_>,
    preheaders: &PreheaderPair<'_>,
    licm: &LicmPair<'_>,
    inventories: (&CanonicalKirInventoryV1<'_>, &CanonicalKirInventoryV1<'g>),
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        CheckedPromotedSites<'s, 'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> PResult<R> {
    let (input, output) = inventories;
    with_checked_loop_preheader_sites(
        sites,
        preheaders,
        input,
        budget,
        binding,
        |sites, budget, binding| {
            budget.charge_work(8)?;
            if !std::ptr::eq(licm.input(), sites.output.owner())
                || !std::ptr::eq(licm.output(), output.owner())
                || licm.origins().len() != sites.output.operations().len()
                || licm.origins().len() != output.operations().len()
                || sites.statements().len() != licm.origins().len()
                || sites.traps().len() != licm.origins().len()
                || sites.output.owner().module().kernels.len()
                    != output.owner().module().kernels.len()
            {
                return Err(refused(
                    "total-integer LICM",
                    "connected actual owners and complete sites",
                )
                .into());
            }
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
            for (ordinal, (origin, original)) in licm
                .origins()
                .iter()
                .zip(sites.output.operations())
                .enumerate()
            {
                budget.charge_work(12)?;
                if origin.input != original.coordinate {
                    return Err(
                        refused("total-integer LICM", "exact ordered input operation").into(),
                    );
                }
                let destination = operation_ordinal(output, origin.output)?;
                if seen[destination] || output.operations()[destination].coordinate != origin.output
                {
                    return Err(
                        refused("total-integer LICM", "unique exact final operation").into(),
                    );
                }
                if sites.traps()[ordinal] && origin.hoist.is_some() {
                    return Err(refused("total-integer LICM", "retained trap occurrence").into());
                }
                seen[destination] = true;
                statements[destination] = sites.statements()[ordinal];
                traps[destination] = sites.traps()[ordinal];
            }
            for present in seen {
                budget.charge_work(1)?;
                if !present {
                    return Err(
                        refused("total-integer LICM", "complete final operation coverage").into(),
                    );
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
        },
    )
}

#[cfg(test)]
pub(in super::super) use tests::exercise_licm_source_sites;

#[cfg(test)]
mod tests {
    use super::*;

    pub(in super::super::super) fn exercise_licm_source_sites(
        sites: CheckedPromotedSites<'_, '_>,
        preheaders: &PreheaderPair<'_>,
        licm: &LicmPair<'_>,
        input: &CanonicalKirInventoryV1<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
        binding: &PromotionBinding,
    ) -> PResult<()> {
        with_final_sites(
            sites,
            preheaders,
            licm,
            (input, output),
            budget,
            binding,
            |sites, budget, binding| {
                let mut bitwise = 0;
                let mut comparison = 0;
                let mut shifted_stores = 0;
                for row in licm.origins() {
                    let index = operation_ordinal(output, row.output)?;
                    let operation = &output.operations()[index].operation.kind;
                    let source_statement = || {
                        let (function, block, statement) =
                            sites.statements()[index].expect("actual source occurrence");
                        &sites.source().semantic().functions()[function.index() as usize].blocks()
                            [block.index() as usize]
                            .statements()[statement as usize]
                    };
                    if row.hoist.is_some()
                        && matches!(
                            operation,
                            OperationKind::Binary {
                                op: fe2o3_kernel_ir::BinaryOp::BitAnd,
                                ..
                            }
                        )
                    {
                        let SemanticStatementKindV1::Assign(value) = source_statement().kind()
                        else {
                            panic!("source bitwise assignment")
                        };
                        assert!(matches!(
                            value.value().kind(),
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitAnd,
                                ..
                            }
                        ));
                        bitwise += 1;
                    }
                    if row.hoist.is_some() && matches!(operation, OperationKind::Compare { .. }) {
                        let SemanticStatementKindV1::Assign(value) = source_statement().kind()
                        else {
                            panic!("source comparison assignment")
                        };
                        assert!(matches!(
                            value.value().kind(),
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::Equal,
                                ..
                            }
                        ));
                        comparison += 1;
                    }
                    if row.hoist.is_none()
                        && row.input.operation != row.output.operation
                        && matches!(operation, OperationKind::Store { .. })
                    {
                        assert!(matches!(
                            source_statement().kind(),
                            SemanticStatementKindV1::Store(_)
                        ));
                        shifted_stores += 1;
                    }
                }
                assert_eq!(bitwise, output.owner().module().kernels.len());
                assert_eq!(comparison, bitwise);
                assert_eq!(shifted_stores, bitwise);
                let _reports = census_sites_named(sites, "total-integer LICM", budget, binding)?;
                Ok(())
            },
        )
    }
}
