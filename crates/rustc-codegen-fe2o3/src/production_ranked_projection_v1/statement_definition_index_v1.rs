// Compatibility facade; the one sparse-index implementation lives in mir-model.
#[cfg(test)]
struct StatementDefinitionIndexV1<'a> {
    inner: neutral_assertion::SemanticStatementDefinitionIndexV1<'a>,
}
#[cfg(test)]
impl<'a> StatementDefinitionIndexV1<'a> {
    fn new(
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let inner = neutral_assertion::SemanticStatementDefinitionIndexV1::new_metered(
            function,
            assertion_compatibility_limits_v1(),
            &mut AssertionCompatibilityMeterV1::new(&mut 0),
        )
        .map_err(assertion_projection_error_v1)?;
        Ok(Self { inner })
    }
    fn rows(&self) -> &[(usize, usize, usize)] {
        self.inner.rows()
    }
    fn capacity(&self) -> usize {
        self.inner.row_capacity()
    }
    fn before(&self, local: usize, block: usize, statement: usize) -> Option<usize> {
        self.inner
            .before_metered(
                local,
                block,
                statement,
                &mut AssertionCompatibilityMeterV1::new(&mut 0),
            )
            .expect("compatibility index lookup has bounded scalar work")
    }
}

// Diagnostic equivalence only. The neutral total ledger has already charged
// its full debit once; truncation here never refunds or discounts that debit.
fn charge_statement_scan_equivalent_v1(
    work: &mut usize,
    visits: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if visits == 0 {
        return Ok(());
    }
    let first_failure = MAX_PROJECTED_LOOP_GRAPH_WORK_V1
        .saturating_sub(*work)
        .saturating_add(1);
    project_loop_graph_charge_v1(work, visits.min(first_failure))
}
