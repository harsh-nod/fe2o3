/// An exact full-report residual prevents the memory-free output continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionBoundScalarResidualV19 {
    /// A complete report still has memory rows, or extraction was incomplete.
    Obligations,
    /// The exact continuing ledger refused the residual census.
    Resource(ArgumentResourceV1),
}
impl fmt::Display for ProductionBoundScalarResidualV19 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Obligations => {
                out.write_str("bound scalar output retains incomplete or memory obligations")
            }
            Self::Resource(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionBoundScalarResidualV19 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Obligations => None,
            Self::Resource(error) => Some(error),
        }
    }
}

/// Refusal before or inside actual optimized-owner adoption.
#[derive(Debug)]
pub enum ProductionBoundScalarCheckErrorV19 {
    /// The original source or continuing ledger refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// Complete paired extraction or its fixed residual check refused.
    Reports(ProductionOptimizedSourceReportsErrorV19<ProductionBoundScalarResidualV19>),
    /// The shared actual ranked-candidate/native lifecycle checker refused.
    Native(ProductionScalarCfgCheckErrorV18),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionBoundScalarCheckErrorV19 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionBoundScalarCheckErrorV19 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl fmt::Display for ProductionBoundScalarCheckErrorV19 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Reports(error) => error.fmt(out),
            Self::Native(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionBoundScalarCheckErrorV19 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Reports(error) => Some(error),
            Self::Native(error) => Some(error),
        }
    }
}

/// Distinguishes preparation refusal from fixed optimizer/adoption refusal.
#[derive(Debug)]
pub enum ProductionBoundScalarHandoffErrorV19 {
    /// Source, full ABI, or continuing resource custody refused preparation.
    Check(ProductionBoundScalarCheckErrorV19),
    /// The actual integer optimizer or its exact adoption checks refused.
    Optimization(ProductionSourceOptimizationErrorV18<ProductionBoundScalarCheckErrorV19>),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionBoundScalarHandoffErrorV19 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Check(error.into())
    }
}
impl From<ArgumentResourceV1> for ProductionBoundScalarHandoffErrorV19 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Check(error.into())
    }
}
impl fmt::Display for ProductionBoundScalarHandoffErrorV19 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check(error) => error.fmt(out),
            Self::Optimization(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionBoundScalarHandoffErrorV19 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Check(error) => Some(error),
            Self::Optimization(error) => Some(error),
        }
    }
}

/// Actual optimized owner after complete memory-free paired reports and the
/// shared ranked/native checker. The original source and exact launch inputs
/// remain borrowed; no graph, report clone, or digest can construct this value.
///
/// Launch inputs and Index width are mathematical interpretations, not target
/// authority. The backend must derive them from its genuine compiler bindings.
/// This nominal continuation grants neither protected execution/publication
/// rights nor source/output memory equivalence. Any allocation, access, bound,
/// alias obligation, conflict or incomplete formal reason refuses construction.
#[must_use = "retain the actual output or discard it on its original ledger"]
pub struct ProductionBoundScalarOutputHandoffV19<'view, 'source> {
    owned: SourceOutputHandoffV18<'view, 'source, IntegerSourceOptimizerV18>,
    launches: &'view [fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
    width: fe2o3_kernel_ir::FormalIndexWidth,
}
source_output_handoff_queries_v18!(
    ProductionBoundScalarOutputHandoffV19,
    CheckedNeutralKernelIrOwnerIntegerContinuationV18
);

impl ProductionBoundScalarOutputHandoffV19<'_, '_> {
    /// Returns the unchanged borrowed interpretation under actual source custody.
    pub fn formal_context_v19(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        fe2o3_kernel_ir::FormalIndexWidth,
    )> {
        self.owned.check(budget)?;
        Ok((self.launches, self.width))
    }

    /// Requires one of the two actual immutable owners whose full reports were
    /// checked during adoption. Equal bytes or a replacement owner do not join.
    pub fn check_reported_owner_v19(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.owned.check(budget)?;
        self.owned.source.retain_query((|| {
            budget.charge_work(2)?;
            if !std::ptr::eq(owner, self.owned.source.canonical(budget)?)
                && !std::ptr::eq(owner, self.owned.output(budget)?.owner())
            {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "bound scalar formal owner differs",
                ));
            }
            Ok(())
        })())
    }
}

fn bound_scalar_report_residual_v19(
    report: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionBoundScalarResidualV19> {
    budget
        .charge_work(7)
        .map_err(ProductionBoundScalarResidualV19::Resource)?;
    let analysis = report.analysis();
    let obligations = analysis.obligations();
    if !analysis.is_complete()
        || !obligations.allocations().is_empty()
        || !obligations.accesses().is_empty()
        || !obligations.bounds_requirements().is_empty()
        || !obligations.runtime_alias_requirements().is_empty()
        || !obligations.inter_invocation_conflicts().is_empty()
    {
        return Err(ProductionBoundScalarResidualV19::Obligations);
    }
    Ok(())
}

fn bound_scalar_handoff_credit_v19() -> Result<usize, ArgumentResourceV1> {
    let extra = size_of::<ProductionBoundScalarOutputHandoffV19<'_, '_>>()
        .checked_sub(size_of::<
            SourceOutputHandoffV18<'_, '_, IntegerSourceOptimizerV18>,
        >())
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    argument_sum_v1(&[
        source_output_handoff_credit_v18::<IntegerSourceOptimizerV18>()?,
        extra,
        std::mem::align_of::<ProductionBoundScalarOutputHandoffV19<'_, '_>>(),
    ])
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Runs the fixed integer optimizer, complete paired reports and the actual
    /// ranked/native checker before retaining its genuine successor. Only the
    /// exact source-owned memory-free subset passes; descriptive launch inputs
    /// are not an authority to publish target code.
    pub fn checked_bound_scalar_output_v19<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        launches: &'view [fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionBoundScalarOutputHandoffV19<'view, 'source>,
        ProductionBoundScalarHandoffErrorV19,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (), receipt) = scoped_source_attempt_v29(self.cleanup, budget, floor, |budget| {
            self.require_kernel_argument_abi_v18(abi, budget)?;
            let credit = bound_scalar_handoff_credit_v19()?;
            let envelopes = argument_sum_v1(&[
                scalar_cfg_check_envelopes_v18()?,
                size_of::<ProductionBoundScalarCheckErrorV19>(),
                size_of::<Result<(), ProductionBoundScalarCheckErrorV19>>(),
                size_of::<ProductionBoundScalarHandoffErrorV19>(),
                size_of::<&[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19]>(),
                size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
            ])?;
            self.retain_query(budget.reserve_storage(envelopes).map_err(Into::into))?;
            let result = self.with_retained_checked_optimization_policy_v18::<IntegerSourceOptimizerV18, (), ProductionBoundScalarCheckErrorV19, _>(
                budget,
                |original, optimized, budget| {
                    bound_scalar_source_checks_v19(original, optimized, launches, width, budget)?;
                    original.retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                    original.retain_query(budget.release_storage(credit).map_err(Into::into))?;
                    Ok(((), credit))
                },
            ).map_err(ProductionBoundScalarHandoffErrorV19::Optimization)?;
            self.guard.check(self.owner, self.cleanup, budget)?;
            self.retain_query(budget.release_storage(envelopes).map_err(Into::into))?;
            Ok(result)
        }).map_err(|error| {
            if let ProductionBoundScalarHandoffErrorV19::Check(ProductionBoundScalarCheckErrorV19::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &error {
                let _ = self.retain_query_resource_error_v18(*resource);
            }
            error
        })?;
        Ok(ProductionBoundScalarOutputHandoffV19 {
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

fn bound_scalar_source_checks_v19(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionBoundScalarCheckErrorV19> {
    use ProductionBoundScalarCheckErrorV19 as Error;
    let floor = budget.storage();
    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        source_output_correspondence_checks_v18(original, optimized, budget)?;
        original
            .with_optimized_formal_reports_v19(
                optimized,
                launches,
                width,
                fe2o3_kernel_ir::ControlFlowLimits::DEFAULT,
                budget,
                |before, after, budget| {
                    bound_scalar_report_residual_v19(before, budget)?;
                    bound_scalar_report_residual_v19(after, budget)
                },
            )
            .map_err(Error::Reports)?;
        let scratch_floor = budget.storage();
        let output = optimized.output_inventory(budget)?;
        source_output_ranked_native_checks_v19(original, optimized, output, scratch_floor, budget)
            .map_err(Error::Native)
    })
    .map_err(|error| {
        if let Error::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))
        | Error::Native(ProductionScalarCfgCheckErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Resource(resource),
        ))
        | Error::Native(ProductionScalarCfgCheckErrorV18::Ranked(
            fe2o3_kernel_analysis::CanonicalRankedViewErrorV1::Resource(resource),
        )) = &error
        {
            let _ = original.source.retain_query_resource_error_v18(*resource);
        }
        error
    })
}
