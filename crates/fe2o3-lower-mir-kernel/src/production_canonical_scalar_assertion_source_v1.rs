fn csa_with_source_v1<'w, T>(
    view: &mut ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_>,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(
        &ProductionCanonicalRankedMetadataV1<'_>,
        &Coverage<'_>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CsResultV1<T>,
) -> CsResultV1<T> {
    let source = view.source;
    fe2o3_pliron::with_canonical_trap_shape_v1(view.checked, budget, |shape, budget| {
        Ok(csa_scope_v1(budget, |budget| {
            let coverage = derive(source, shape, budget)?;
            cr_policy_source_profile_with_assertions_v1(source, Some(&coverage), budget)?;
            run(source, &coverage, budget)
        }))
    })?
}

impl ProductionCanonicalScalarFixedPointOwnerV1 {
    /// Consume actual source/N after fresh assertion proofs and complete original
    /// terminal-pair joins, then run the unchanged real neutral fixed-point factory.
    /// This scalar/control entry does not admit private memory or ordinary calls.
    /// Original source storage is prepaid; reserve the returned additional receipt
    /// immediately. No original-N nine-stage result or donor history is accepted.
    pub fn try_prepare_with_assertions_v1(
        original: ProductionPreRankedKirOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<(Self, ProductionCanonicalScalarStorageV1)> {
        Self::prepare_with_source_v1(original, budget, |original, budget| {
            original.with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(csa_with_source_v1(view, budget, |_, _, _| Ok(())))
            })?
        })
    }
}
