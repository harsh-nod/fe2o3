/// Actual Policy9 output after the fixed private/native and paired formal/path
/// checks. This is nominally distinct from the historical Policy6 handoff.
/// Full report reasons are preserved and individually joined by the shared
/// private-class checker; every external residual still refuses.
///
/// The supplied launch/width remains descriptive at this lowerer boundary.
/// No ranked-final, target, publication or launch authority is conferred.
///
/// Historical Policy6 custody cannot be relabeled as a Policy9 execution.
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionBoundPrivateOutputHandoffV21,
///     ProductionBoundPrivateWorklistOutputHandoffV21,
/// };
/// fn relabel<'view, 'source>(
///     old: ProductionBoundPrivateOutputHandoffV21<'view, 'source>,
/// ) -> ProductionBoundPrivateWorklistOutputHandoffV21<'view, 'source> {
///     old
/// }
/// ```
#[must_use = "retain this actual owner or discard it on its original account"]
pub struct ProductionBoundPrivateWorklistOutputHandoffV21<'view, 'source> {
    owned: SourceOutputHandoffV18<'view, 'source, IntegerWorklistSourceOptimizerV18>,
    launches: &'view [fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
    width: fe2o3_kernel_ir::FormalIndexWidth,
}
source_output_handoff_queries_v18!(
    ProductionBoundPrivateWorklistOutputHandoffV21,
    CheckedNeutralKernelIrOwnerIntegerWorklistV18
);
impl ProductionBoundPrivateWorklistOutputHandoffV21<'_, '_> {
    /// The exact descriptive context used by the two original reports.
    pub fn formal_context_v21(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        fe2o3_kernel_ir::FormalIndexWidth,
    )> {
        self.owned.check(budget)?;
        Ok((self.launches, self.width))
    }
    /// Private/report composition is not general ranked-final verification.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// This retained output is not executable or publication authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Runs only Policy9, then reuses the complete same-adoption private class
    /// consumer. Historical Policy6, memory rules and final gates are unchanged.
    pub fn checked_bound_private_worklist_output_v21<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        launches: &'view [fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionBoundPrivateWorklistOutputHandoffV21<'view, 'source>,
        ProductionBoundPrivateHandoffErrorV21,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (), receipt) = scoped_source_attempt_v29(self.cleanup, budget, floor, |budget| {
            self.require_kernel_argument_abi_v18(abi, budget)?;
            let extra = size_of::<ProductionBoundPrivateWorklistOutputHandoffV21<'_, '_>>()
                .checked_sub(size_of::<SourceOutputHandoffV18<'_, '_, IntegerWorklistSourceOptimizerV18>>())
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let credit = argument_sum_v1(&[
                source_output_handoff_credit_v18::<IntegerWorklistSourceOptimizerV18>()?,
                extra,
                std::mem::align_of::<ProductionBoundPrivateWorklistOutputHandoffV21<'_, '_>>(),
            ])?;
            let headers = argument_sum_v1(&[
                private_source_completion_headers_v20()?,
                size_of::<ProductionBoundPrivateCheckErrorV21>(),
                size_of::<ProductionBoundPrivateHandoffErrorV21>(),
                size_of::<Result<(), ProductionBoundPrivateCheckErrorV21>>(),
                size_of::<&[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19]>(),
                size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
            ])?;
            self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
            let result = self.with_retained_checked_optimization_policy_v18::<IntegerWorklistSourceOptimizerV18, (), ProductionBoundPrivateCheckErrorV21, _>(budget, |original, optimized, budget| {
                bound_private_source_checks_v21(original, optimized, launches, width, budget)?;
                original.retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                original.retain_query(budget.release_storage(credit).map_err(Into::into))?;
                Ok(((), credit))
            }).map_err(ProductionBoundPrivateHandoffErrorV21::Optimization)?;
            self.guard.check(self.owner, self.cleanup, budget)?;
            self.retain_query(budget.release_storage(headers).map_err(Into::into))?;
            Ok(result)
        }).map_err(|error| {
            if let ProductionBoundPrivateHandoffErrorV21::Check(ProductionBoundPrivateCheckErrorV21::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &error {
                let _ = self.retain_query_resource_error_v18(*resource);
            }
            error
        })?;
        Ok(ProductionBoundPrivateWorklistOutputHandoffV21 {
            owned: SourceOutputHandoffV18 {
                source: self,
                output,
                receipt,
                required: budget.storage(),
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
            },
            launches,
            width,
        })
    }
}
