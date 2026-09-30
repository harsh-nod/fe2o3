// A real Policy12 output retains every checked stage beside its original source.
// The storage-only policy shares custody without fabricating scalar transitions.
include!("production_source_aggregate_errors_v30.rs");
include!("production_source_aggregate_stages_v30.rs");
include!("production_source_aggregate_memory_chain_v31.rs");
include!("production_source_aggregate_memory_visit_v31.rs");
include!("production_source_aggregate_initial_callback_v30.rs");
include!("production_source_pending_native_v30.rs");
include!("production_source_aggregate_completion_v30.rs");

impl From<fe2o3_kernel_analysis::CanonicalRankedViewErrorV1> for ProductionAggregateSourceErrorV30 {
    fn from(error: fe2o3_kernel_analysis::CanonicalRankedViewErrorV1) -> Self {
        Self::Ranked(error)
    }
}
impl From<fe2o3_pliron::CanonicalRankedPolicyFailureV1> for ProductionAggregateSourceErrorV30 {
    fn from(error: fe2o3_pliron::CanonicalRankedPolicyFailureV1) -> Self {
        Self::Native(ProductionSourceNativeLifecycleErrorV18::Pending(error))
    }
}

struct AggregateSourceStorageV30;
impl SourceOutputStoragePolicyV30 for AggregateSourceStorageV30 {
    type Owned = fe2o3_kernel_opt::OwnedAggregateFixedpointV18;
    type Credit = usize;
    fn output_storage(output: &Self::Owned) -> usize {
        output.retained_storage()
    }
    fn receipt_storage(receipt: &Self::Credit) -> usize {
        *receipt
    }
}

/// Actual source, checked Policy12 execution, or independent relation refusal.
#[derive(Debug)]
pub enum ProductionAggregateSourceErrorV30 {
    /// Original source identity, currentness, custody, or resource validation failed.
    Source(ProductionSourceOwnedViewErrorV18),
    /// Actual Policy12 execution or replay of a retained stage failed.
    Optimization(fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18),
    /// Independent construction of an actual canonical endpoint inventory failed.
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    /// The actual first scalar transition did not satisfy its checked relation.
    Transition(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1),
    /// The real first-stage source/native conjunction failed before transport.
    InitialCompletion(ProductionMixedSourceCheckErrorV26),
    /// Fresh final native execution or final source-role replay failed.
    Native(ProductionSourceNativeLifecycleErrorV18),
    /// Final ranked metadata did not describe the exact final canonical owner.
    Ranked(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1),
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionAggregateSourceErrorV30 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionAggregateSourceErrorV30 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl fmt::Display for ProductionAggregateSourceErrorV30 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => e.fmt(out),
            Self::Optimization(e) => e.fmt(out),
            Self::Inventory(e) => e.fmt(out),
            Self::Transition(e) => e.fmt(out),
            Self::InitialCompletion(e) => e.fmt(out),
            Self::Native(e) => e.fmt(out),
            Self::Ranked(e) => e.fmt(out),
        }
    }
}
impl std::error::Error for ProductionAggregateSourceErrorV30 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(e) => Some(e),
            Self::Optimization(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Transition(e) => Some(e),
            Self::InitialCompletion(e) => Some(e),
            Self::Native(e) => Some(e),
            Self::Ranked(e) => Some(e),
        }
    }
}

/// Original-source custody of the actual nominal Policy12 chain.
///
/// Every scalar and aggregate stage, full execution witness, and original
/// canonical input remain available together. The first scalar correspondence
/// replays original source currentness; it is not a replacement for all later
/// memory/CFG refinement and final native/runtime completion. This owner has no
/// scalar transition projection, formal execution receipt, or launch authority.
/// It cannot escape the source visit. Ordinary Drop releases no ledger credit.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionAggregateSourceOutputHandoffV30,
///     ProductionConditionalMixedFixedpointOutputHandoffV29};
/// fn relabel<'v, 's>(x: ProductionAggregateSourceOutputHandoffV30<'v, 's>)
///     -> ProductionConditionalMixedFixedpointOutputHandoffV29<'v, 's> { x }
/// ```
#[must_use = "keep the real aggregate chain live or discard it on its original ledger"]
pub struct ProductionAggregateSourceOutputHandoffV30<'view, 'source> {
    owned: SourceOutputHandoffV18<'view, 'source, AggregateSourceStorageV30>,
}

impl ProductionAggregateSourceOutputHandoffV30<'_, '_> {
    /// Borrows the full nominal chain, not a synthetic original-to-final scalar map.
    pub fn output(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_opt::OwnedAggregateFixedpointV18> {
        self.owned.output(budget)
    }
    /// Rejoins this output to its exact retained original semantic SSA owner.
    pub fn check_original_source(
        &self,
        original: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.owned.check_original_source(original, budget)
    }
    /// Replays the complete original argument ABI on this handoff's source account.
    pub fn check_original_argument_abi_v30(
        &self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.owned.check(budget)?;
        self.owned
            .source
            .require_kernel_argument_abi_v18(abi, budget)
    }
    /// Rechecks the complete actual chain on the same original input and account.
    pub fn replay(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        self.owned.check(budget)?;
        let source = self.owned.source;
        let floor = budget.storage();
        scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
            let result = (|| {
                let header = aggregate_source_headers_v30()?;
                source.retain_query(budget.reserve_storage(header).map_err(Into::into))?;
                source.retain_query(budget.charge_work(32).map_err(Into::into))?;
                let result = self
                    .owned
                    .output
                    .replay_against(source.canonical(budget)?, budget)
                    .map_err(ProductionAggregateSourceErrorV30::Optimization);
                source.retain_aggregate_result_v30(result)?;
                source.retain_query(budget.release_storage(header).map_err(Into::into))?;
                Ok::<_, ProductionAggregateSourceErrorV30>(())
            })();
            source.retain_aggregate_result_v30(result)
        })?;
        self.owned.check(budget)?;
        Ok(())
    }
    /// Returns the actual chain and custody credit after checking the original ledger.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.owned.retained_storage(budget)
    }
    /// Refuses a lost retained floor without authorizing credit on another account.
    pub fn observe_retained_storage_v30(
        &self,
        required: usize,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.owned.observe_retained_storage_v18(required, budget)
    }
    /// Drops the actual chain before settling only its intact original-account credit.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.owned.discard(budget)
    }
    /// Always false: first-stage currentness is not final native/runtime completion.
    pub const fn final_native_completion_is_complete(&self) -> bool {
        false
    }
    /// Always false: retaining the actual chain does not execute its composed proof.
    pub const fn executed_source_refinement_is_complete(&self) -> bool {
        false
    }
    /// Always false: downstream authenticated publication and runtime gates remain.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn aggregate_source_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        ProductionKernelArgumentAbiInputV18<'a>,
        fe2o3_kernel_ir::StorageLayoutLimitsV1,
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'a, 'a, 'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirTransitionStorageV1,
        // Longest closed typed classification path, with its selected payload.
        [(
            &'a ProductionAggregateSourceErrorV30,
            Option<ArgumentResourceV1>,
        ); 9],
        SourceOwnedQueryFailureV18,
        &'a mut ArgumentBudgetV1<'a>,
        [usize; 8],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        size_of::<Result<Frame<'_>, ProductionAggregateSourceErrorV30>>(),
        size_of::<
            Result<
                fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
                fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18,
            >,
        >(),
        size_of::<Result<(), ProductionAggregateSourceErrorV30>>(),
        size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
                    fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >() * 2,
        size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>,
                    fe2o3_kernel_analysis::CanonicalKirTransitionStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1,
            >,
        >(),
    ])
}

impl ProductionSourceOwnedViewV18<'_> {
    fn retain_aggregate_source_result_v30<T>(
        &self,
        result: SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        if let Err(error) = &result {
            if aggregate_source_owned_resource_v30(error) == Some(ArgumentResourceV1::Accounting)
                || matches!(error, ProductionSourceOwnedViewErrorV18::Source(_))
            {
                self.cleanup.deny_refund();
            }
        }
        self.retain_query(result)
    }

    fn deny_aggregate_accounting_v30(&self, error: &ProductionAggregateSourceErrorV30) {
        // Original construction errors predate this source-owned interface.
        // Preserve them exactly, but never infer refund permission from their
        // opaque diagnostic children. Normal query errors are typed separately.
        if aggregate_source_resource_v30(error) == Some(ArgumentResourceV1::Accounting)
            || aggregate_opaque_source_v30(error)
        {
            self.cleanup.deny_refund();
        }
    }

    // Called inside each cleanup scope, including before source callback return.
    // Diagnostic Error::source implementations never select ledger behavior.
    fn retain_aggregate_result_v30<T>(
        &self,
        result: Result<T, ProductionAggregateSourceErrorV30>,
    ) -> Result<T, ProductionAggregateSourceErrorV30> {
        if let Err(error) = &result {
            let resource = aggregate_source_resource_v30(error);
            let refusal = match (resource, error) {
                (Some(resource), _) => SourceOwnedQueryFailureV18::Resource(resource),
                (
                    None,
                    ProductionAggregateSourceErrorV30::Source(
                        ProductionSourceOwnedViewErrorV18::Binding(message),
                    ),
                ) => SourceOwnedQueryFailureV18::Binding(message),
                _ => SourceOwnedQueryFailureV18::Binding("actual source Policy12 chain refused"),
            };
            self.deny_aggregate_accounting_v30(error);
            let _ = self.guard.reject::<()>(refusal);
        }
        result
    }
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Executes Policy12 on the exact original canonical graph and retains the
    /// real checked chain. No caller graph, map, receipt, or completion callback
    /// can substitute its input or manufacture a scalar relation for memory SSA.
    pub fn aggregate_output_v30<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionAggregateSourceOutputHandoffV30<'view, 'source>,
        ProductionAggregateSourceErrorV30,
    > {
        use ProductionAggregateSourceErrorV30 as Error;
        self.query(budget)?;
        let floor = budget.storage();
        let result = scoped_source_attempt_v29(self.cleanup, budget, floor, |budget| {
            let result = (|| {
                self.require_kernel_argument_abi_v18(abi, budget)?;
                let header = aggregate_source_headers_v30()?;
                self.retain_query(budget.reserve_storage(header).map_err(Into::into))?;
                self.retain_query(budget.charge_work(64).map_err(Into::into))?;
                let layout_limits = self.limits(budget)?.storage_layout_limits();
                let input = self.canonical(budget)?;
                let output = self.retain_aggregate_result_v30(
                    fe2o3_kernel_opt::optimize_owned_aggregate_fixedpoint_v18(
                        input,
                        layout_limits,
                        budget,
                    )
                    .map_err(Error::Optimization),
                )?;
                let credit = argument_sum_v1(&[
                    source_output_handoff_credit_v18::<AggregateSourceStorageV30>()?,
                    size_of::<usize>(),
                ])?;
                let retained = argument_sum_v1(&[output.retained_storage(), credit])?;
                self.retain_query(budget.reserve_storage(retained).map_err(Into::into))?;
                self.retain_aggregate_result_v30(
                    output
                        .replay_against(input, budget)
                        .map_err(Error::Optimization),
                )?;
                with_aggregate_initial_source_v30(
                    self,
                    &output,
                    budget,
                    |original, optimized, budget| {
                        optimized_source_endpoints_v18(original, optimized, budget)?;
                        source_output_correspondence_checks_v18(original, optimized, budget)?;
                        Ok(())
                    },
                )?;
                self.guard.check(self.owner, self.cleanup, budget)?;
                self.retain_query(budget.release_storage(header).map_err(Into::into))?;
                Ok((output, credit))
            })();
            self.retain_aggregate_result_v30(result)
        });
        let (output, receipt) = self.retain_aggregate_result_v30(result)?;
        Ok(ProductionAggregateSourceOutputHandoffV30 {
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

// The first real scalar stage is the only one directly borrowing the original
// source inventory. Later stages require their actual checked chain, never this
// view with substituted endpoints. This helper also serves mixed completion.
fn with_aggregate_initial_source_v30<T>(
    source: &ProductionSourceOwnedViewV18<'_>,
    chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, ProductionAggregateSourceErrorV30>,
) -> Result<T, ProductionAggregateSourceErrorV30> {
    with_aggregate_initial_callback_v30(source, chain, budget, consume)
}
