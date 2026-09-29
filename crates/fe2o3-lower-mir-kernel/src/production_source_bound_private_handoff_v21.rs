/// An external formal obligation not discharged by private-source completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionBoundPrivateObligationV21 {
    /// An external allocation needs an authenticated runtime binding.
    Allocation,
    /// An external access needs complete address/domain evidence.
    Access,
    /// A bounds requirement remains unresolved.
    Bounds,
    /// A runtime alias requirement remains unresolved.
    Alias,
    /// An inter-invocation conflict remains unresolved.
    Conflict,
}

/// Exact residual occurrence retained by the private-class report consumer.
#[derive(Debug)]
pub enum ProductionBoundPrivateResidualV21 {
    /// The continuing source/resource account refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// An external obligation cannot be inferred from a private proof.
    External(ProductionBoundPrivateObligationV21),
    /// The report/path or complete reason roster is inconsistent.
    Roster,
    /// This original reason is not handled by private/native completion.
    Reason(usize),
    /// A specific original reason failed its actual live coverage join.
    Coverage {
        /// Ordinal in the unchanged original report's reason vector.
        ordinal: usize,
        /// Exact source/native/custody failure.
        error: ProductionSourceNativeLifecycleErrorV18,
    },
}
impl From<ArgumentResourceV1> for ProductionBoundPrivateResidualV21 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl fmt::Display for ProductionBoundPrivateResidualV21 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::External(kind) => write!(out, "private continuation retains external {kind:?}"),
            Self::Roster => out.write_str("private continuation report/path roster differs"),
            Self::Reason(ordinal) => {
                write!(out, "private continuation retains formal reason {ordinal}")
            }
            Self::Coverage { ordinal, error } => {
                write!(out, "private formal reason {ordinal}: {error}")
            }
        }
    }
}
impl std::error::Error for ProductionBoundPrivateResidualV21 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Coverage { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Refusal in the same-adoption private/formal composition.
#[derive(Debug)]
pub enum ProductionBoundPrivateCheckErrorV21 {
    /// The original source or its continuing account refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// A genuine paired report/path retained its typed refusal.
    Reports(ProductionOptimizedSourcePathsErrorV20<ProductionBoundPrivateResidualV21>),
    /// Original private currentness, entry RHS, or native checking refused.
    Private(ProductionPrivateSourceCheckErrorV20),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionBoundPrivateCheckErrorV21 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionBoundPrivateCheckErrorV21 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl fmt::Display for ProductionBoundPrivateCheckErrorV21 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Reports(error) => error.fmt(out),
            Self::Private(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionBoundPrivateCheckErrorV21 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Reports(error) => Some(error),
            Self::Private(error) => Some(error),
        }
    }
}

/// Preparation versus genuine optimizer/adoption refusal.
#[derive(Debug)]
pub enum ProductionBoundPrivateHandoffErrorV21 {
    /// Original ABI/source preparation refused.
    Check(ProductionBoundPrivateCheckErrorV21),
    /// Fixed integer optimization or its private/formal adoption refused.
    Optimization(ProductionSourceOptimizationErrorV18<ProductionBoundPrivateCheckErrorV21>),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionBoundPrivateHandoffErrorV21 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Check(error.into())
    }
}
impl From<ArgumentResourceV1> for ProductionBoundPrivateHandoffErrorV21 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Check(error.into())
    }
}
impl fmt::Display for ProductionBoundPrivateHandoffErrorV21 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check(error) => error.fmt(out),
            Self::Optimization(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionBoundPrivateHandoffErrorV21 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Check(error) => Some(error),
            Self::Optimization(error) => Some(error),
        }
    }
}

/// The same immutable optimized owner after exact private/formal composition.
/// Actual private memory is permitted only through original MemorySSA,
/// currentness, entry-RHS and complete native-function histories. Formal report
/// reasons are preserved and joined individually, never deleted or rewritten.
///
/// This is not a ranked-final proof, source/output functional equivalence,
/// authenticated launch, target, publication, or default-pipeline authority.
/// The launch/width inputs remain borrowed mathematical interpretations.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionBoundPrivateOutputHandoffV21;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mutate(handoff: &ProductionBoundPrivateOutputHandoffV21<'_, '_>,
///           budget: &CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     handoff.output(budget).unwrap().owner().module().functions.clear();
/// }
/// ```
#[must_use = "retain this actual owner or discard it on its original account"]
pub struct ProductionBoundPrivateOutputHandoffV21<'view, 'source> {
    owned: SourceOutputHandoffV18<'view, 'source, IntegerSourceOptimizerV18>,
    launches: &'view [fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
    width: fe2o3_kernel_ir::FormalIndexWidth,
}
source_output_handoff_queries_v18!(
    ProductionBoundPrivateOutputHandoffV21,
    CheckedNeutralKernelIrOwnerIntegerContinuationV18
);
impl ProductionBoundPrivateOutputHandoffV21<'_, '_> {
    /// Checks one of the two immutable owners actually joined during adoption.
    /// Equal bytes, edited copies and replacement owners do not satisfy custody.
    pub fn check_reported_owner_v21(
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
                    "bound private formal owner differs",
                ));
            }
            Ok(())
        })())
    }
    /// Returns the original descriptive formal context under current custody.
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
    /// The remaining general ranked semantics are not discharged by this type.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No execution or publication authority is conferred by this handoff.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

// A fixed report borrow and monotone ordinal cursor prevent one covered
// occurrence from satisfying another, even if their operation kinds match.
struct PrivateFormalReasonCursorV21<'view, 'report, 'owner> {
    report: &'view fe2o3_kernel_ir::CanonicalFormalReportViewV19<'report, 'owner>,
    next: usize,
    previous: Option<usize>,
    refused: bool,
}
impl<'view, 'report, 'owner> PrivateFormalReasonCursorV21<'view, 'report, 'owner> {
    fn new(
        native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
        report: &'view fe2o3_kernel_ir::CanonicalFormalReportViewV19<'report, 'owner>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionBoundPrivateResidualV21> {
        native
            .check_formal_report_subject_v21(report, budget)
            .map_err(|error| ProductionBoundPrivateResidualV21::Coverage { ordinal: 0, error })?;
        Ok(Self {
            report,
            next: 0,
            previous: None,
            refused: false,
        })
    }

    fn check_next(
        &mut self,
        native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionBoundPrivateResidualV21> {
        use ProductionBoundPrivateResidualV21 as Error;
        if self.refused {
            return Err(Error::Roster);
        }
        // A failed occurrence cannot later be replaced by a successful retry.
        self.refused = true;
        budget.charge_work(10)?;
        if ordinal != self.next {
            return Err(Error::Roster);
        }
        let reasons = self.report.analysis().incomplete_reasons();
        let Some(reason) = reasons.get(ordinal) else {
            return Err(Error::Reason(ordinal));
        };
        if !matches!(
            reason,
            fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
                | fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedPointerDerivation { .. }
        ) {
            return Err(Error::Reason(ordinal));
        }
        native
            .check_formal_reason_v21(self.report, ordinal, budget)
            .map_err(|error| Error::Coverage { ordinal, error })?;
        // The unchanged report orders variants before original locations.
        // One operation may need both its effect and pointer-use proof.
        if self.previous.is_some_and(|old| reasons[old] >= *reason) {
            return Err(Error::Roster);
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        self.previous = Some(ordinal);
        self.refused = false;
        Ok(())
    }

    fn finish(
        self,
        native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionBoundPrivateResidualV21> {
        budget.charge_work(1)?;
        if self.refused || self.next != self.report.analysis().incomplete_reasons().len() {
            return Err(ProductionBoundPrivateResidualV21::Roster);
        }
        native
            .check_formal_report_subject_v21(self.report, budget)
            .map_err(|error| ProductionBoundPrivateResidualV21::Coverage {
                ordinal: self.next,
                error,
            })
    }
}

// Fixed-size borrowed classification, paid by the report consumer's 16 units.
// Conflict observations never remove any of these original obligations.
fn bound_private_external_rows_v21(
    analysis: &fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
) -> [(bool, ProductionBoundPrivateObligationV21); 5] {
    use ProductionBoundPrivateObligationV21 as Kind;
    let rows = analysis.obligations();
    [
        (
            !rows.inter_invocation_conflicts().is_empty(),
            Kind::Conflict,
        ),
        (!rows.runtime_alias_requirements().is_empty(), Kind::Alias),
        (!rows.bounds_requirements().is_empty(), Kind::Bounds),
        (!rows.accesses().is_empty(), Kind::Access),
        (!rows.allocations().is_empty(), Kind::Allocation),
    ]
}

fn bound_private_report_v21(
    native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
    report: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
    paths: &fe2o3_kernel_analysis::FormalPaidPathViewV20<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionBoundPrivateResidualV21> {
    use ProductionBoundPrivateResidualV21 as Error;
    budget.charge_work(16)?;
    if !std::ptr::eq(report.analysis(), paths.analysis())
        || !std::ptr::eq(report.original_owner(), paths.original_owner())
        || !std::ptr::eq(report.original_function(), paths.original_function())
        || report.root_index() != paths.root_index()
        || report.launch_input() != paths.launch_input()
        || report.index_width() != paths.index_width()
    {
        return Err(Error::Roster);
    }
    for (nonempty, kind) in bound_private_external_rows_v21(report.analysis()) {
        if nonempty {
            return Err(Error::External(kind));
        }
    }
    if !paths.observations().is_empty()
        || (!report.analysis().is_complete() && report.analysis().incomplete_reasons().is_empty())
    {
        return Err(Error::Roster);
    }
    let mut cursor = PrivateFormalReasonCursorV21::new(native, report, budget)?;
    for ordinal in 0..report.analysis().incomplete_reasons().len() {
        cursor.check_next(native, ordinal, budget)?;
    }
    cursor.finish(native, budget)
}

fn bound_private_source_checks_v21<'work>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut ArgumentBudgetV1<'work>,
) -> Result<(), ProductionBoundPrivateCheckErrorV21> {
    use ProductionBoundPrivateCheckErrorV21 as Error;
    let floor = budget.storage();
    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let mut reports = None;
        let mut consume = |native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
                           budget: &mut ArgumentBudgetV1<'work>| {
            let result = original.with_optimized_formal_paths_v20(
                optimized,
                launches,
                width,
                fe2o3_kernel_ir::ControlFlowLimits::DEFAULT,
                budget,
                |side, before, after, paths, budget| match side {
                    ProductionFormalPathSideV20::Original => {
                        bound_private_report_v21(native, before, paths, budget)
                    }
                    ProductionFormalPathSideV20::Optimized => {
                        bound_private_report_v21(native, after, paths, budget)
                    }
                },
            );
            let failed = result.is_err();
            reports = Some(result);
            if failed {
                Err(ProductionSourceNativeLifecycleErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "private formal report continuation refused",
                    ),
                ))
            } else {
                Ok(())
            }
        };
        let headers = argument_sum_v1(&[
            std::mem::size_of_val(&consume),
            size_of::<
                Option<
                    Result<
                        (),
                        ProductionOptimizedSourcePathsErrorV20<ProductionBoundPrivateResidualV21>,
                    >,
                >,
            >(),
            size_of::<Result<(), ProductionPrivateSourceCheckErrorV20>>(),
            size_of::<Result<(), Error>>(),
            size_of::<[usize; 12]>(),
            size_of::<[(bool, ProductionBoundPrivateObligationV21); 5]>(),
            size_of::<Option<[usize; 3]>>(),
            size_of::<PrivateFormalReasonCursorV21<'_, '_, '_>>(),
            size_of::<
                Result<PrivateFormalReasonCursorV21<'_, '_, '_>, ProductionBoundPrivateResidualV21>,
            >(),
            size_of::<
                std::iter::Enumerate<
                    std::slice::Iter<'_, fe2o3_kernel_ir::FormalMemoryIncompleteReason>,
                >,
            >(),
        ])?;
        original.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
        let native = with_private_source_completion_v21(original, optimized, budget, &mut consume);
        drop(consume);
        match reports {
            Some(Err(error)) => {
                if let ProductionOptimizedSourcePathsErrorV20::Consumer(
                    ProductionBoundPrivateResidualV21::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(resource),
                    ),
                ) = &error
                {
                    let _ = original.retain_query_resource_error_v18(*resource);
                }
                return Err(Error::Reports(error));
            }
            Some(Ok(())) => native.map_err(Error::Private)?,
            None => {
                native.map_err(Error::Private)?;
                return Err(Error::Source(ProductionSourceOwnedViewErrorV18::Binding(
                    "private report callback was not reached",
                )));
            }
        }
        optimized_source_endpoints_v18(original, optimized, budget)?;
        original.retain_query(budget.release_storage(headers).map_err(Into::into))?;
        Ok(())
    })
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Retains the actual integer successor only after complete original
    /// private-source checking and individually covered full-report reasons.
    /// External allocation/bounds/alias/conflict requirements remain refused.
    pub fn checked_bound_private_output_v21<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        launches: &'view [fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionBoundPrivateOutputHandoffV21<'view, 'source>,
        ProductionBoundPrivateHandoffErrorV21,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (), receipt) = scoped_source_attempt_v29(
            self.cleanup,
            budget,
            floor,
            |budget| {
                self.require_kernel_argument_abi_v18(abi, budget)?;
                let extra = size_of::<ProductionBoundPrivateOutputHandoffV21<'_, '_>>()
                    .checked_sub(size_of::<
                        SourceOutputHandoffV18<'_, '_, IntegerSourceOptimizerV18>,
                    >())
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let credit = argument_sum_v1(&[
                    source_output_handoff_credit_v18::<IntegerSourceOptimizerV18>()?,
                    extra,
                    std::mem::align_of::<ProductionBoundPrivateOutputHandoffV21<'_, '_>>(),
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
                let result = self.with_retained_checked_optimization_policy_v18::<IntegerSourceOptimizerV18, (), ProductionBoundPrivateCheckErrorV21, _>(budget, |original, optimized, budget| {
                bound_private_source_checks_v21(original, optimized, launches, width, budget)?;
                original.retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                original.retain_query(budget.release_storage(credit).map_err(Into::into))?;
                Ok(((), credit))
            }).map_err(ProductionBoundPrivateHandoffErrorV21::Optimization)?;
                self.guard.check(self.owner, self.cleanup, budget)?;
                self.retain_query(budget.release_storage(headers).map_err(Into::into))?;
                Ok(result)
            },
        ).map_err(|error| {
            if let ProductionBoundPrivateHandoffErrorV21::Check(ProductionBoundPrivateCheckErrorV21::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &error {
                let _ = self.retain_query_resource_error_v18(*resource);
            }
            error
        })?;
        Ok(ProductionBoundPrivateOutputHandoffV21 {
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
