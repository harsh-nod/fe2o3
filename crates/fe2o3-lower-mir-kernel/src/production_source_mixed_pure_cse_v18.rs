source_optimizer_policy_v18!(
    MixedPureCseSourceOptimizerV18,
    KirNeutralOptimizationOutputMixedPureCseV18,
    CheckedNeutralKernelIrOwnerMixedPureCseV18,
    optimize_neutral_kernel_ir_mixed_pure_cse_v18
);

impl ProductionSourceOwnedViewV18<'_> {
    /// Executes fixed Policy10: integer-neutral worklist, local pure CSE,
    /// dominance pure CSE, and DCE. The complete observed transition is checked
    /// against this original source. No caller-supplied pass list is accepted.
    /// Historical policies and their nominal output owners remain unchanged.
    ///
    /// The mixed handoff also preserves this nominal distinction:
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionConditionalMixedOutputHandoffV26,
    ///     ProductionConditionalMixedPureCseOutputHandoffV26};
    /// fn relabel<'v, 's>(old: ProductionConditionalMixedOutputHandoffV26<'v, 's>)
    ///     -> ProductionConditionalMixedPureCseOutputHandoffV26<'v, 's> { old }
    /// ```
    pub fn with_checked_mixed_pure_cse_optimization_v18<T: 'static, E: 'static>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedPureCseV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_checked_optimization_policy_v18::<MixedPureCseSourceOptimizerV18, T, E>(
            budget, consume,
        )
    }

    /// Retains the actual checked Policy10 owner and callback payload on the
    /// original ledger. Their exact receipts stay live until destruction or
    /// checked transfer; this is not ranked, target, or launch admission.
    pub fn with_retained_checked_mixed_pure_cse_optimization_v18<T: 'static, E: 'static, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedPureCseV18,
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
        self.with_retained_checked_optimization_policy_v18::<MixedPureCseSourceOptimizerV18, T, E, F>(
            budget, consume,
        )
    }
}
