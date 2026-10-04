/// Refusal while joining original source and all actual mixed native reports.
#[derive(Debug)]
pub enum ProductionMixedSourceCheckErrorV26 {
    /// Original source, occurrence, currentness, or resource agreement failed.
    Source(ProductionSourceOwnedViewErrorV18),
    /// The actual ranked candidate could not be checked against its owner.
    Ranked(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1),
    /// Native stage replay or its source/control conjunction was refused.
    Native(ProductionSourceNativeLifecycleErrorV18),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionMixedSourceCheckErrorV26 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionMixedSourceCheckErrorV26 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl fmt::Display for ProductionMixedSourceCheckErrorV26 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Ranked(error) => error.fmt(out),
            Self::Native(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionMixedSourceCheckErrorV26 {}

/// Preparation and actual consuming fixed-policy adoption remain distinguishable.
#[derive(Debug)]
pub enum ProductionMixedSourceHandoffErrorV26 {
    /// Preparation or source/native conjunction failed before owning adoption.
    Check(ProductionMixedSourceCheckErrorV26),
    /// The real optimizer or consuming checked-output adoption was refused.
    Optimization(ProductionSourceOptimizationErrorV18<ProductionMixedSourceCheckErrorV26>),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionMixedSourceHandoffErrorV26 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Check(error.into())
    }
}
impl From<ArgumentResourceV1> for ProductionMixedSourceHandoffErrorV26 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Check(error.into())
    }
}
impl fmt::Display for ProductionMixedSourceHandoffErrorV26 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check(error) => error.fmt(out),
            Self::Optimization(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionMixedSourceHandoffErrorV26 {}

// Both fixed policies preserve the CFG and share the exact source/native
// completion contract. Instantiate nominal owners, never erase policy identity.
macro_rules! mixed_source_handoff_v26 {
    ($handoff:ident, $policy:ident, $output:ident) => {
        mixed_source_handoff_v26!(
            $handoff,
            $policy,
            $output,
            ProductionMixedRuntimeOccurrenceV26
        );
    };
    ($handoff:ident, $policy:ident, $output:ident, $occurrence:ident) => {
        /// Move-only actual fixed-policy output, inseparable from its source-bound runtime
        /// premise roster. Concrete allocation, initialization, cross-argument
        /// non-overlap and launch/width binding are mandatory downstream conditions.
        /// This intermediate owner confers no launch, artifact or ranked-final authority.
        #[must_use = "retain the actual output and its mandatory runtime contract together"]
        pub struct $handoff<'view, 'source> {
            owned: SourceOutputHandoffV18<'view, 'source, $policy>,
            premises: Vec<ProductionMixedSliceRuntimePremiseV26>,
            occurrences: Vec<$occurrence>,
            launches: &'view [fe2o3_kernel_ir::ExplicitLaunchExtent],
            width: fe2o3_kernel_ir::FormalIndexWidth,
        }
        impl $handoff<'_, '_> {
            /// Borrows the actual nominal output while its source and storage remain bound.
            pub fn output(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<&fe2o3_pliron::$output> {
                self.owned.output(budget)
            }
            /// Replays association with this exact retained original semantic SSA owner.
            pub fn check_original_source(
                &self,
                source: &ProductionSemanticSsaOwnerV1,
                budget: &mut ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.owned.check_original_source(source, budget)
            }
            /// Rejoin a downstream emitter's complete descriptor/component roster to
            /// the captured original ABI. Semantic type and descriptor layout identities
            /// remain distinct; equality of their digest bytes is never substituted.
            pub fn check_original_argument_abi_v26(
                &self,
                abi: ProductionKernelArgumentAbiInputV18<'_>,
                budget: &mut ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.owned.check(budget)?;
                self.owned
                    .source
                    .require_kernel_argument_abi_v18(abi, budget)
            }
            /// Returns every original slice argument, including explicit zero-use rows;
            /// concrete runtime facts remain required for the recorded accesses.
            pub fn runtime_premises(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<&[ProductionMixedSliceRuntimePremiseV26]> {
                self.owned.check(budget)?;
                Ok(&self.premises)
            }
            /// Returns the per-root explicit launch conditions and formal index width.
            pub fn launch_context(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<(
                &[fe2o3_kernel_ir::ExplicitLaunchExtent],
                fe2o3_kernel_ir::FormalIndexWidth,
            )> {
                self.owned.check(budget)?;
                Ok((self.launches, self.width))
            }
            /// Returns every exact global access, including its separate address formation.
            pub fn runtime_occurrences(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<&[$occurrence]> {
                self.owned.check(budget)?;
                Ok(&self.occurrences)
            }
            /// Returns the checked live credit for output ownership and retained premises.
            pub fn retained_storage(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<usize> {
                self.owned.retained_storage(budget)
            }
            /// Refuses lost custody without refunding a caller's retained storage floor.
            pub fn observe_retained_storage_v18(
                &self,
                required: usize,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.owned.observe_retained_storage_v18(required, budget)
            }
            /// Destroys the premise vectors and output before refunding their exact credit.
            pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
                let Self {
                    owned,
                    premises,
                    occurrences,
                    ..
                } = self;
                drop(premises);
                drop(occurrences);
                owned.discard(budget)
            }
            /// Always false: live allocation, alias, initialization, and launch checks remain.
            pub const fn runtime_requirements_are_discharged(&self) -> bool {
                false
            }
            /// Always false: this conditional handoff is not a final ranked certificate.
            pub const fn ranked_verification_is_complete(&self) -> bool {
                false
            }
            /// Always false: downstream authenticated composition must admit execution.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
        }
    };
}

macro_rules! mixed_source_completion_family_v89 {
    ($headers:ident, $complete:ident, $occurrence:ident, $checked:ident, $native_method:ident, $subject:ident) => {
fn $headers() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        ProductionMixedSourceCheckErrorV26,
        ProductionMixedSourceHandoffErrorV26,
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<$occurrence>,
        $occurrence,
        ProductionMixedSliceRuntimePremiseV26,
        &'a $checked<'a, 'a>,
        Option<&'a fe2o3_pliron::CanonicalMixedPipelineReportV26>,
        Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>,
        &'a [fe2o3_kernel_ir::ExplicitLaunchExtent],
        fe2o3_kernel_ir::FormalIndexWidth,
        std::slice::Iter<'a, ProductionMixedSliceRuntimePremiseV26>,
        [usize; 12],
        [&'a (); 24],
    );
    argument_sum_v1(&[
        private_source_completion_headers_v20()?,
        size_of::<Frame<'_>>(),
        argument_product_v1(
            2,
            size_of::<Result<Frame<'_>, ProductionMixedSourceCheckErrorV26>>(),
        )?,
    ])
}

fn $complete<'work, F>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: &mut F,
) -> Result<(), ProductionMixedSourceCheckErrorV26>
where
    F: for<'scope, 'owner> FnMut(
        &$checked<'scope, 'owner>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSourceNativeLifecycleErrorV18>,
{
    use ProductionMixedSourceCheckErrorV26 as Error;
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateMemoryLimitsV1, CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let floor = budget.storage();
    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let scratch_floor = budget.storage();
        optimized_source_endpoints_v18(original, optimized, budget)?;
        source_output_correspondence_checks_v18(original, optimized, budget)?;
        let index = OriginalEntryIndexV20::build(original, budget)?;
        let index_storage = budget
            .storage()
            .checked_sub(scratch_floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let output = optimized.output_inventory(budget)?;
        let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
        let metadata_storage = metadata.storage_extent(budget).map_err(Error::Ranked)?;
        original.retain_query(budget.reserve_storage(metadata_storage).map_err(Into::into))?;
        let (candidate, receipt) = build_canonical_ranked_candidate_v18(output, &metadata, budget)
            .map_err(Error::Ranked)?;
        original.retain_query(
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(Into::into),
        )?;
        let layouts = original.source.limits(budget)?.storage_layout_limits();
        let limits = CanonicalKirPrivateMemoryLimitsV1 {
            max_cells: output.definitions().len(),
        };
        let result = with_checked_canonical_ranked_view_v18(output, &metadata, &candidate, budget, |checked, budget| {
            Ok::<_, CanonicalRankedViewErrorV1>(optimized.$native_method(
                checked, layouts, limits, launches, width, budget,
                |request, budget| complete_private_source_root_v20(original, optimized, &index, request, budget),
                |native, budget| {
                    native.$subject(original, optimized, budget)?;
                    let count = native.function_count(budget)?;
                    if count != output.functions().len() { return Err(original.source.missing::<()>("mixed source native function census differs").unwrap_err().into()); }
                    for (ordinal, function) in output.functions().iter().enumerate() {
                        original.retain_query(budget.charge_work(4).map_err(Into::into))?;
                        match (function.function.body.is_some(), native.report(ordinal, budget)?, native.history(ordinal, budget)?) {
                            (true, Some(report), Some(_)) if report.reports().is_clean() && report.paired_stage_count() == 9 => (),
                            (false, None, None) => (),
                            _ => return Err(original.source.missing::<()>("mixed source completion lacks full clean native history").unwrap_err().into()),
                        }
                    }
                    consume(native, budget)
                },
            ))
        }).map_err(Error::Ranked)?;
        drop(candidate);
        drop(metadata);
        result.map_err(Error::Native)??;
        index.check(budget)?;
        drop(index);
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let paid = argument_sum_v1(&[index_storage, metadata_storage, receipt.retained_storage()])?;
        if scratch_floor.checked_add(paid) != Some(budget.storage()) {
            original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        original.retain_query(budget.release_storage(paid).map_err(Into::into))?;
        Ok(())
    })
}

    };
}
mixed_source_completion_family_v89!(
    mixed_source_completion_headers_v26,
    with_mixed_source_completion_v26,
    ProductionMixedRuntimeOccurrenceV26,
    ProductionMixedMemoryCheckedNativePoliciesV26,
    with_mixed_memory_native_policies_v26,
    check_source_subject_v26
);
mixed_source_completion_family_v89!(
    predicated_source_completion_headers_v89,
    with_predicated_source_completion_v89,
    ProductionMixedRuntimeOccurrenceV89,
    ProductionPredicatedMemoryCheckedNativePoliciesV89,
    with_predicated_memory_native_policies_v89,
    check_source_subject_v89
);

macro_rules! mixed_source_completion_v26 {
    ($method:ident, $handoff:ident, $policy:ident) => {
        mixed_source_completion_v26!($method, $handoff, $policy, ProductionMixedRuntimeOccurrenceV26, mixed_source_completion_headers_v26, with_mixed_source_completion_v26);
    };
    ($method:ident, $handoff:ident, $policy:ident, $occurrence:ident, $headers:ident, $complete:ident) => {
impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Runs the nominal fixed policy and performs complete source/native mixed
    /// conjunction inside the consuming output-adoption callback. The returned
    /// output retains the exact mandatory runtime premises, without claiming
    /// that host pointers or a physical launch exist during compilation.
    pub fn $method<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        launches: &'view [fe2o3_kernel_ir::ExplicitLaunchExtent],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        $handoff<'view, 'source>,
        ProductionMixedSourceHandoffErrorV26,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (premises, occurrences), receipt) = scoped_source_attempt_v29(
            self.cleanup,
            budget,
            floor,
            |budget| {
                self.require_kernel_argument_abi_v18(abi, budget)?;
                let headers = $headers()?;
                self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
                let result = self.with_retained_checked_optimization_policy_v18::<$policy, (Vec<ProductionMixedSliceRuntimePremiseV26>, Vec<$occurrence>), ProductionMixedSourceCheckErrorV26, _>(budget, |original, optimized, budget| {
                let callback_floor = budget.storage();
                scoped_source_attempt_v29(original.source.cleanup, budget, callback_floor, |budget| {
                let payload_floor = budget.storage();
                let output = optimized.output_inventory(budget)?;
                let mut capacity = 0usize;
                for function in output.functions() {
                    budget.charge_work(2)?;
                    capacity = capacity.checked_add(function.function.signature.parameters.len()).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                let mut premises = source_reference_emission_vec_v29(capacity, budget).map_err(source_argument_error_v18)?;
                let mut occurrences = source_reference_emission_vec_v29(output.operations().len(), budget).map_err(source_argument_error_v18)?;
                let backing = budget.storage().checked_sub(payload_floor).ok_or(ArgumentResourceV1::Accounting)?;
                $complete(original, optimized, launches, width, budget, &mut |native, budget| {
                    if !premises.is_empty() { return Err(original.source.missing::<()>("mixed runtime premise callback repeated").unwrap_err().into()); }
                    for premise in native.runtime_premises(budget)? {
                        original.retain_query(budget.charge_work(1).map_err(Into::into))?;
                        if premises.len() == premises.capacity() { return Err(original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting).into()); }
                        premises.push(*premise);
                    }
                    for occurrence in native.runtime_occurrences(budget)? {
                        original.retain_query(budget.charge_work(1).map_err(Into::into))?;
                        if occurrences.len() == occurrences.capacity() { return Err(original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting).into()); }
                        occurrences.push(*occurrence);
                    }
                    Ok(())
                })?;
                let extra = size_of::<$handoff<'_, '_>>()
                    .checked_sub(size_of::<SourceOutputHandoffV18<'_, '_, $policy>>())
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let credit = argument_sum_v1(&[source_output_handoff_credit_v18::<$policy>()?, extra, std::mem::align_of::<$handoff<'_, '_>>(), backing])?;
                // The checked adopter immediately reserves this returned credit
                // with the owned graph and payload; no detached proof escapes.
                original.retain_query(budget.release_storage(backing).map_err(Into::into))?;
                if budget.storage() != payload_floor { original.source.cleanup.deny_refund(); return Err(ArgumentResourceV1::Accounting.into()); }
                original.retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                original.retain_query(budget.release_storage(credit).map_err(Into::into))?;
                Ok(((premises, occurrences), credit))
                })
            }).map_err(ProductionMixedSourceHandoffErrorV26::Optimization)?;
                self.guard.check(self.owner, self.cleanup, budget)?;
                self.retain_query(budget.release_storage(headers).map_err(Into::into))?;
                Ok::<_, ProductionMixedSourceHandoffErrorV26>(result)
            },
        )?;
        Ok($handoff {
            owned: SourceOutputHandoffV18 {
                source: self,
                output,
                receipt,
                required: budget.storage(),
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
            },
            premises,
            occurrences,
            launches,
            width,
        })
    }
}
    };
}

mixed_source_handoff_v26!(
    ProductionConditionalMixedOutputHandoffV26,
    IntegerWorklistSourceOptimizerV18,
    CheckedNeutralKernelIrOwnerIntegerWorklistV18
);
mixed_source_handoff_v26!(
    ProductionConditionalMixedPureCseOutputHandoffV26,
    MixedPureCseSourceOptimizerV18,
    CheckedNeutralKernelIrOwnerMixedPureCseV18
);
mixed_source_completion_v26!(
    conditional_mixed_worklist_output_v26,
    ProductionConditionalMixedOutputHandoffV26,
    IntegerWorklistSourceOptimizerV18
);
mixed_source_completion_v26!(
    conditional_mixed_pure_cse_output_v26,
    ProductionConditionalMixedPureCseOutputHandoffV26,
    MixedPureCseSourceOptimizerV18
);

mixed_source_handoff_v26!(
    ProductionConditionalMixedFixedpointOutputHandoffV29,
    MixedFixedpointSourceOptimizerV18,
    CheckedNeutralKernelIrOwnerMixedFixedpointV18
);
mixed_source_completion_v26!(
    conditional_mixed_fixedpoint_output_v29,
    ProductionConditionalMixedFixedpointOutputHandoffV29,
    MixedFixedpointSourceOptimizerV18
);

mixed_source_handoff_v26!(
    ProductionConditionalPredicatedFixedpointOutputHandoffV89,
    MixedFixedpointSourceOptimizerV18,
    CheckedNeutralKernelIrOwnerMixedFixedpointV18,
    ProductionMixedRuntimeOccurrenceV89
);
mixed_source_completion_v26!(
    conditional_predicated_fixedpoint_output_v89,
    ProductionConditionalPredicatedFixedpointOutputHandoffV89,
    MixedFixedpointSourceOptimizerV18,
    ProductionMixedRuntimeOccurrenceV89,
    predicated_source_completion_headers_v89,
    with_predicated_source_completion_v89
);

include!("production_source_mixed_prefix_v29.rs");
include!("production_source_aggregate_owner_v30.rs");
include!("production_source_mixed_contract_v26.rs");

#[path = "production_source_mixed_licm_v28.rs"]
mod mixed_licm_v28;
pub use mixed_licm_v28::{
    ProductionConditionalMixedFixedpointLicmOutputHandoffV29,
    ProductionConditionalMixedLicmOutputHandoffV28,
    ProductionConditionalPredicatedLicmOutputHandoffV90,
    ProductionMixedFixedpointLicmRelocationV29, ProductionMixedFixedpointStoreConsensusV46,
    ProductionMixedLicmCompletionErrorV28, ProductionMixedLicmDefinitionProjectionV28,
    ProductionMixedLicmRelocationErrorV28, ProductionMixedLicmRelocationV28,
    ProductionMixedLicmRuntimeOccurrenceV28, ProductionMixedStoreConsensusV46,
    ProductionPredicatedFixedpointLicmRelocationV90, ProductionPredicatedLicmRuntimeOccurrenceV90,
    ProductionPredicatedStoreConsensusV90,
};
