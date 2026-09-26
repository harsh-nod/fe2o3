fn cpc_with_source_v1<'w, T>(
    view: &mut ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_>,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(
        &ProductionCanonicalRankedMetadataV1<'_>,
        &Coverage<'_>,
        &[ProductionCanonicalAssertionCallableV1],
        &mut ArgumentBudgetV1<'w>,
    ) -> CsResultV1<T>,
) -> CsResultV1<T> {
    let source = view.source;
    fe2o3_pliron::with_canonical_trap_shape_v1(view.checked, budget, |shape, budget| {
        Ok(cpc_scope_v1(budget, |budget| {
            let coverage = derive(source, shape, budget)?;
            cr_private_source_profile_with_assertions_v1(source, Some(&coverage), budget)?;
            let calls = callable_rows(source, budget)?;
            cr_private_calls_v1(source, budget)?;
            checked_output_admission_policy3_v1::with_canonical_private_source_reader_v1(
                source,
                budget,
                |_, budget| Ok(run(source, &coverage, &calls, budget)),
            )
            .map_err(ProductionCanonicalScalarSourceErrorV1::SourcePolicy)?
        }))
    })
    .map_err(ProductionCanonicalScalarSourceErrorV1::Query)?
}

impl ProductionCanonicalScalarFixedPointOwnerV1 {
    /// Consume genuine original source/N after its complete private, call, frame
    /// and assertion checks, then derive the existing real scalar fixed point.
    /// This does not prove inlining, promotion, SROA or function/root deletion.
    /// Prepay the complete original source floor; reserve the returned additional
    /// receipt immediately. No caller history or detached proof is accepted.
    pub fn try_prepare_with_private_calls_v1(
        original: ProductionPreRankedKirOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<(Self, ProductionCanonicalScalarStorageV1)> {
        budget.charge_work(1)?;
        let floor = original
            .unit_local_source_storage_floor_v1()
            .map_err(ProductionCanonicalRankedSourceErrorV1::from)?;
        if budget.storage() < floor {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Self::prepare_with_source_v1(original, budget, |original, budget| {
            original.with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(cpc_with_source_v1(view, budget, |_, _, _, _| Ok(())))
            })?
        })
    }
}
