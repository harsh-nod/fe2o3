source_optimizer_policy_v18!(
    IntegerWorklistSourceOptimizerV18,
    KirNeutralOptimizationOutputIntegerWorklistV18,
    CheckedNeutralKernelIrOwnerIntegerWorklistV18,
    optimize_neutral_kernel_ir_integer_worklist_v18
);

impl ProductionSourceOwnedViewV18<'_> {
    /// Executes the fixed Policy9 integer-neutral def-use worklist and DCE on
    /// this genuine V18 source, then checks the complete observed transition.
    /// Historical policies are unchanged. The callback sees both original and
    /// optimized correspondences on the original ledger; no final admission is
    /// granted. Reserve both returned receipts before controlled allocation.
    pub fn with_checked_integer_worklist_optimization_v18<T: 'static, E: 'static>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerWorklistV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_checked_optimization_policy_v18::<IntegerWorklistSourceOptimizerV18, T, E>(
            budget, consume,
        )
    }

    /// Runs the same fixed continuation and retains its actual checked owner
    /// and callback result before the source scope retires. Their existing
    /// storage receipts must stay live until destruction or exact transfer.
    /// This does not authorize ranked, formal-memory, native or target output.
    pub fn with_retained_checked_integer_worklist_optimization_v18<T: 'static, E: 'static, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerWorklistV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
        F: for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    {
        self.with_retained_checked_optimization_policy_v18::<IntegerWorklistSourceOptimizerV18, T, E, F>(
            budget, consume,
        )
    }
}
