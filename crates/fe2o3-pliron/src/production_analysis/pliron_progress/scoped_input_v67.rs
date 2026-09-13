pub(crate) struct ScopedProgressReportV1<'scope> {
    pub(crate) context: &'scope Context,
    pub(crate) function: &'scope FuncOp,
    pub(crate) report: PlironProgressReportV1,
}

pub(crate) fn run_pliron_progress_with_scoped_input_v1<'scope, const BARRIER: bool>(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedProgressInputV1<
        'scope,
        BARRIER,
    >,
) -> Result<
    ScopedProgressReportV1<'scope>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    let (context, function) = input.into_endpoints()?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let inventory = match bounded_structural_inventory(context, function) {
            Ok(inventory) => inventory,
            Err(finding) => return report(finding),
        };
        let mut work = ProgressWorkBudgetV1::default();
        if let Err(finding) = work.charge(inventory.structural_work().unwrap_or(usize::MAX)) {
            return report(finding);
        }
        run_pliron_progress_after_verification_v1(context, inventory, work)
    }));
    let report = match result {
        Ok(report) => report,
        Err(payload) => report(structural_rejection(format!(
            "bounded structural preflight panicked: {}",
            panic_detail(payload)
        ))),
    };
    Ok(ScopedProgressReportV1 {
        context,
        function,
        report,
    })
}
