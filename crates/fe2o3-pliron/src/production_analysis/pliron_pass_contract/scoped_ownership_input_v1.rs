/// Borrowed only within the owner-held ownership pass. The snapshot is the
/// already verified input, not a digest or a second independently captured graph.
pub(crate) struct ScopedVerifiedOwnershipInputV1<'scope> {
    context: &'scope Context,
    function: &'scope FuncOp,
    mutation_epoch: u64,
    identity: &'scope crate::PlironIrStructuralIdentityV1,
}

impl<'scope> ScopedVerifiedOwnershipInputV1<'scope> {
    pub(crate) fn endpoints(
        &self,
    ) -> Result<(&'scope Context, &'scope FuncOp), PlironPassPreservationErrorV1> {
        require_scoped_progress_epoch_for_pass_v1(
            KernelCheckPassKindV1::HierarchicalOwnership,
            self.mutation_epoch,
            self.context
                .ir_mutation_attempt_epoch()
                .map(|epoch| epoch.value()),
        )?;
        Ok((self.context, self.function))
    }

    pub(crate) fn snapshot(
        &self,
    ) -> Result<(&'scope crate::PlironIrStructuralIdentityV1, u64), PlironPassPreservationErrorV1>
    {
        self.endpoints()?;
        Ok((self.identity, self.mutation_epoch))
    }
}

fn scoped_ownership_input_resource_upper_bound_v1()
-> Result<ProductionAnalysisResourceUpperBoundV1, PlironPassPreservationErrorV1> {
    let base = scoped_progress_input_resource_upper_bound_v1()?;
    // One additional borrowed field, with its extraction and publication work.
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::PassPreservation,
        base.work_upper_bound() + 4,
        0,
        base.peak_storage_upper_bound() + 1,
    )
    .map_err(preservation_resource_error_v1)
}

impl PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>> {
    pub(crate) fn run_scoped_ownership_with_resource_limits_v1<T, E>(
        &mut self,
        limits: impl Into<ProductionAnalysisReplacementLimitsV1>,
        execute: impl for<'scope> FnOnce(
            ScopedVerifiedOwnershipInputV1<'scope>,
        ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        self.run_scoped_analysis_with_resource_limits_v1(
            KernelCheckPassKindV1::HierarchicalOwnership,
            scoped_ownership_input_resource_upper_bound_v1()?,
            limits,
            |context, function, mutation_epoch, snapshot| {
                execute(ScopedVerifiedOwnershipInputV1 {
                    context,
                    function,
                    mutation_epoch,
                    identity: snapshot.structural_identity_v1(),
                })
            },
        )
    }
}
