/// A genuine scalar CFG obligation check refused the exact adopted output.
#[derive(Debug)]
pub enum ProductionScalarCfgCheckErrorV18 {
    /// The genuine source query or its original custody refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// The actual ranked candidate or checked read scope refused.
    Ranked(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1),
    /// The source-bound lifecycle or native policy engine refused.
    Native(ProductionSourceNativeLifecycleErrorV18),
    /// The bounded actual-owner formal engine refused its input.
    Formal(fe2o3_kernel_ir::CanonicalScalarCfgFormalErrorV18),
    /// The formal engine retained these genuine incomplete obligations.
    FormalIncomplete(Vec<fe2o3_kernel_ir::FormalMemoryIncompleteReason>),
    /// A required identity, report or discharged obligation was absent.
    Incomplete(&'static str),
}

impl fmt::Display for ProductionScalarCfgCheckErrorV18 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Ranked(error) => error.fmt(out),
            Self::Native(error) => error.fmt(out),
            Self::Formal(error) => error.fmt(out),
            Self::FormalIncomplete(reasons) => {
                write!(out, "scalar CFG formal extraction incomplete: {reasons:?}")
            }
            Self::Incomplete(detail) => write!(out, "scalar CFG obligations incomplete: {detail}"),
        }
    }
}

impl std::error::Error for ProductionScalarCfgCheckErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Ranked(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Formal(error) => Some(error),
            Self::Incomplete(_) | Self::FormalIncomplete(_) => None,
        }
    }
}

impl From<ProductionSourceOwnedViewErrorV18> for ProductionScalarCfgCheckErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionScalarCfgCheckErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

/// Failure to retain the actual optimized owner with complete scalar CFG checks.
#[derive(Debug)]
pub enum ProductionScalarCfgHandoffErrorV18 {
    /// A source, ranked, formal or native obligation check refused.
    Check(ProductionScalarCfgCheckErrorV18),
    /// The fixed integer optimizer or its genuine adoption callback refused.
    Optimization(ProductionSourceOptimizationErrorV18<ProductionScalarCfgCheckErrorV18>),
}
impl fmt::Display for ProductionScalarCfgHandoffErrorV18 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check(error) => error.fmt(out),
            Self::Optimization(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionScalarCfgHandoffErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Check(error) => Some(error),
            Self::Optimization(error) => Some(error),
        }
    }
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionScalarCfgHandoffErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Check(error.into())
    }
}
impl From<ArgumentResourceV1> for ProductionScalarCfgHandoffErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Check(error.into())
    }
}

/// The same owned integer-continuation output after complete source/currentness,
/// memory-free scalar CFG formal extraction, and actual lifecycle/native checks.
///
/// This does not grant final ranked, target, publication, or launch authority.
/// Full formal extraction is not a proof of termination or runtime inputs.
/// Construction accepts no caller-supplied completion flags or replacement graph.
/// The source and all output storage remain under their original ledger custody.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionScalarCfgOutputHandoffV18, ProductionClosedScalarOutputHandoffV18};
/// fn relabel<'a, 's>(v: ProductionScalarCfgOutputHandoffV18<'a, 's>) -> ProductionClosedScalarOutputHandoffV18<'a, 's> { v }
/// ```
#[must_use = "retain the paid output or discard it on its original ledger"]
pub struct ProductionScalarCfgOutputHandoffV18<'view, 'source> {
    owned: SourceOutputHandoffV18<'view, 'source, IntegerSourceOptimizerV18>,
}

source_output_handoff_queries_v18!(
    ProductionScalarCfgOutputHandoffV18,
    CheckedNeutralKernelIrOwnerIntegerContinuationV18
);

fn scalar_cfg_check_envelopes_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        closed_scalar_native_envelopes_v18()?,
        size_of::<ProductionScalarCfgCheckErrorV18>(),
        size_of::<Result<(), ProductionScalarCfgCheckErrorV18>>(),
        size_of::<fe2o3_kernel_ir::CanonicalScalarCfgFormalScopeV18<'_>>(),
        size_of::<
            Result<
                fe2o3_kernel_ir::CanonicalScalarCfgFormalScopeV18<'_>,
                fe2o3_kernel_ir::CanonicalScalarCfgFormalErrorV18,
            >,
        >(),
        size_of::<
            Result<
                fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
                fe2o3_kernel_ir::CanonicalScalarCfgFormalErrorV18,
            >,
        >(),
        size_of::<fe2o3_kernel_ir::FormalMemoryObligations>(),
        size_of::<[u64; 3]>(),
        size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            &Inventory<'_>,
            usize,
            &mut ArgumentBudgetV1<'_>,
        )>(),
    ])
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Explicit fixed integer optimization plus actual scalar CFG obligation
    /// checks. Memory and unsupported formal/native roles remain refused.
    pub fn checked_scalar_cfg_output_v18<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionScalarCfgOutputHandoffV18<'view, 'source>,
        ProductionScalarCfgHandoffErrorV18,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (), receipt) = scoped_source_attempt_v29(self.cleanup, budget, floor, |budget| {
            self.require_kernel_argument_abi_v18(abi, budget)?;
            let credit = source_output_handoff_credit_v18::<IntegerSourceOptimizerV18>()?;
            let envelopes = scalar_cfg_check_envelopes_v18()?;
            self.retain_query(budget.reserve_storage(envelopes).map_err(Into::into))?;
            let output = self.with_retained_checked_optimization_policy_v18::<IntegerSourceOptimizerV18, (), ProductionScalarCfgCheckErrorV18, _>(
                budget,
                |original, optimized, budget| {
                    scalar_cfg_source_checks_v18(original, optimized, budget)?;
                    original.retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                    original.retain_query(budget.release_storage(credit).map_err(Into::into))?;
                    Ok(((), credit))
                },
            ).map_err(ProductionScalarCfgHandoffErrorV18::Optimization)?;
            self.guard.check(self.owner, self.cleanup, budget)?;
            self.retain_query(budget.release_storage(envelopes).map_err(Into::into))?;
            Ok(output)
        }).map_err(|error| {
            if let ProductionScalarCfgHandoffErrorV18::Check(ProductionScalarCfgCheckErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &error {
                let _ = self.retain_query_resource_error_v18(*resource);
            }
            error
        })?;
        Ok(ProductionScalarCfgOutputHandoffV18 {
            owned: SourceOutputHandoffV18 {
                source: self,
                output,
                receipt,
                required: budget.storage(),
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
            },
        })
    }
}

fn scalar_cfg_formal_owner_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionScalarCfgCheckErrorV18> {
    use ProductionScalarCfgCheckErrorV18 as Error;
    use fe2o3_kernel_ir::{
        CanonicalScalarCfgFormalScopeV18, ExplicitLaunchExtent, FormalIndexWidth, LaunchExtent,
    };
    let mut scope = CanonicalScalarCfgFormalScopeV18::new(owner).map_err(Error::Formal)?;
    if !std::ptr::eq(scope.owner(), owner) {
        return Err(Error::Incomplete("formal owner differs"));
    }
    for kernel in &owner.module().kernels {
        original.retain_query(
            budget
                .charge_work(argument_sum_v1(&[
                    kernel.id.as_str().len(),
                    kernel.entry.as_str().len(),
                    9,
                ])?)
                .map_err(Into::into),
        )?;
        let mut extents = [1; 3];
        let mut known = true;
        for (axis, extent) in kernel.domain.extents().enumerate() {
            match extent {
                LaunchExtent::Static(value) => extents[axis] = u64::from(value),
                LaunchExtent::Dynamic => known = false,
            }
        }
        let extent = if known {
            ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            }
        } else {
            ExplicitLaunchExtent::Unknown
        };
        let result = scope
            .derive(&kernel.id, extent, FormalIndexWidth::Bits64)
            .map_err(Error::Formal)?;
        let obligations = match result {
            fe2o3_kernel_ir::FormalMemoryObligationAnalysis::Complete(obligations) => obligations,
            fe2o3_kernel_ir::FormalMemoryObligationAnalysis::Incomplete { reasons, .. } => {
                return Err(Error::FormalIncomplete(reasons));
            }
        };
        if obligations.kernel() != &kernel.id
            || obligations.entry() != &kernel.entry
            || !obligations.allocations().is_empty()
            || !obligations.accesses().is_empty()
            || !obligations.bounds_requirements().is_empty()
            || !obligations.runtime_alias_requirements().is_empty()
            || !obligations.inter_invocation_conflicts().is_empty()
        {
            return Err(Error::Incomplete(
                "formal extraction or residual memory obligations",
            ));
        }
    }
    Ok(())
}

fn scalar_cfg_source_checks_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionScalarCfgCheckErrorV18> {
    use ProductionScalarCfgCheckErrorV18 as Error;
    use fe2o3_kernel_analysis::CanonicalRankedViewErrorV1;
    let floor = budget.storage();
    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let scratch_floor = budget.storage();
        optimized_source_endpoints_v18(original, optimized, budget)?;
        source_output_correspondence_checks_v18(original, optimized, budget)?;
        // Each formal scope borrows this exact immutable owner under the formal
        // engine's separate bounded policy. No V12 copy or reconstructed graph.
        scalar_cfg_formal_owner_v18(original, original.inventory(budget)?.owner(), budget)?;
        let output = optimized.output_inventory(budget)?;
        scalar_cfg_formal_owner_v18(original, output.owner(), budget)?;
        source_output_ranked_native_checks_v19(original, optimized, output, scratch_floor, budget)
    })
    .map_err(|error| {
        if let Error::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))
        | Error::Ranked(CanonicalRankedViewErrorV1::Resource(resource)) = &error
        {
            let _ = original.source.retain_query_resource_error_v18(*resource);
        }
        error
    })
}

// Shared actual-owner candidate/native check. Callers retain their distinct
// formal policy and scoped cleanup; no caller supplies completion booleans.
fn source_output_ranked_native_checks_v19(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    output: &Inventory<'_>,
    scratch_floor: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionScalarCfgCheckErrorV18> {
    use ProductionScalarCfgCheckErrorV18 as Error;
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
    let metadata_storage = metadata.storage_extent(budget).map_err(Error::Ranked)?;
    original.retain_query(budget.reserve_storage(metadata_storage).map_err(Into::into))?;
    let (candidate, receipt) =
        build_canonical_ranked_candidate_v18(output, &metadata, budget).map_err(Error::Ranked)?;
    original.retain_query(
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(Into::into),
    )?;
    let layouts = original.source.limits(budget)?.storage_layout_limits();
    let result = with_checked_canonical_ranked_view_v18(
        output,
        &metadata,
        &candidate,
        budget,
        |checked, budget| {
            Ok::<_, CanonicalRankedViewErrorV1>(optimized.with_lifecycle_native_policies_v18(
                checked,
                layouts,
                budget,
                |policies, budget| {
                    policies.check_source_subject_v18(original, optimized, budget)?;
                    for ordinal in 0..policies.function_count(budget)? {
                        if policies
                            .report(ordinal, budget)?
                            .is_none_or(|report| !report.is_clean())
                            || policies.history(ordinal, budget)?.is_none()
                        {
                            return Err(original
                                .source
                                .missing::<()>("scalar CFG incomplete native report")
                                .unwrap_err()
                                .into());
                        }
                    }
                    Ok(())
                },
            ))
        },
    )
    .map_err(Error::Ranked)?;
    drop(candidate);
    drop(metadata);
    result.map_err(Error::Native)?;
    optimized_source_endpoints_v18(original, optimized, budget)?;
    let paid = argument_sum_v1(&[metadata_storage, receipt.retained_storage()])?;
    if scratch_floor.checked_add(paid) != Some(budget.storage()) {
        original.source.cleanup.deny_refund();
        return Err(ArgumentResourceV1::Accounting.into());
    }
    original.retain_query(budget.release_storage(paid).map_err(Into::into))?;
    Ok(())
}
