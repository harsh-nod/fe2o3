/// Failure to construct or replay a pending scoped source graph.
#[derive(Debug)]
pub enum ProductionPendingScopedSourceErrorV29 {
    /// Source, lowering, correspondence or resource validation failed.
    Source(ProductionSemanticKirErrorV1),
    /// The complete canonical V18 graph and table failed admission or replay.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18),
    /// Capture of genuine source SSA occurrences failed.
    Occurrences(fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1),
}

impl std::fmt::Display for ProductionPendingScopedSourceErrorV29 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "pending scoped source: {error}"),
            Self::Canonical(error) => write!(formatter, "pending scoped graph: {error}"),
            Self::Occurrences(error) => write!(formatter, "pending scoped occurrences: {error}"),
        }
    }
}

impl std::error::Error for ProductionPendingScopedSourceErrorV29 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Canonical(error) => Some(error),
            Self::Occurrences(error) => Some(error),
        }
    }
}

impl From<ScopedModuleErrorV29> for ProductionPendingScopedSourceErrorV29 {
    fn from(error: ScopedModuleErrorV29) -> Self {
        match error {
            ScopedModuleErrorV29::Source(error) => Self::Source(error),
            ScopedModuleErrorV29::Canonical(error) => Self::Canonical(error),
            ScopedModuleErrorV29::Occurrences(error) => Self::Occurrences(error),
        }
    }
}

/// Owns source, its reconstructed scoped graph, and instance-qualified attachments.
///
/// This is pending evidence, not source-value equivalence, assertion-elision,
/// memory-lifetime or execution-discharge proof. It cannot be converted into an
/// executable, pre-ranked owner or launch receipt. Projected input agreement does
/// not authenticate its Rust producer; that custody remains with the caller.
///
/// The owner is neither constructible from detached data nor cloneable:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29;
/// fn duplicate(owner: ProductionPendingScopedSourceOwnerV29) {
///     let _ = owner.clone();
/// }
/// ```
///
/// Detached inspection data cannot construct or mutate this owner:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29;
/// let _ = ProductionPendingScopedSourceOwnerV29 { inner: panic!() };
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29;
/// fn mutate(owner: &ProductionPendingScopedSourceOwnerV29) {
///     owner.pending_module().functions.clear();
/// }
/// ```
///
/// Pending evidence has no executable conversion:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29;
/// fn execute(owner: ProductionPendingScopedSourceOwnerV29) {
///     let _ = owner.into_executable();
/// }
/// ```
pub struct ProductionPendingScopedSourceOwnerV29 {
    inner: SourceOwnedScopedModuleV29,
}

impl ProductionPendingScopedSourceOwnerV29 {
    /// Consumes the actual SSA owner and launch roster, even on failure.
    ///
    /// The projected input is borrowed only during validation and capture. Its
    /// lifetime need not outlive the returned owner. Successful construction
    /// leaves the new reservation live on this exact ledger; do not reserve it
    /// again. Existing source/SSA/launch allocations retain their upstream
    /// accounting domain. Any preexisting occurrence capture must already be
    /// reserved and remains separately caller-accounted, including on failure.
    ///
    /// On error or panic newly adopted data drops before permitted rollback.
    /// Lost source custody prevents every containing refund. Work is never refunded.
    pub fn try_materialize_with_budget(
        owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPendingScopedSourceErrorV29> {
        Self::materialize_with_optional_kernel_abi_v18(owner, launch, input, None, limits, budget)
    }

    /// Materializes the same pending source owner with its complete original
    /// kernel descriptor ABI. No execution or memory-access authority is issued.
    ///
    /// The original descriptor/source census is authenticated before emission
    /// and retained for independent replay. Preexisting occurrence storage keeps
    /// the historical Pending contract: it remains separately caller-accounted.
    /// Lost cleanup custody prevents containing refunds just as in the unprofiled
    /// constructor. The profile cannot recover a concrete space from Generic.
    pub fn try_materialize_with_kernel_abi_budget_v18(
        owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
        kernel_abi: ProductionKernelArgumentAbiInputV18<'_>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPendingScopedSourceErrorV29> {
        Self::materialize_with_optional_kernel_abi_v18(
            owner, launch, input, Some(kernel_abi), limits, budget,
        )
    }

    fn materialize_with_optional_kernel_abi_v18(
        owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
        kernel_abi: Option<ProductionKernelArgumentAbiInputV18<'_>>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPendingScopedSourceErrorV29> {
        let floor = budget.storage();
        if owner
            .occurrence_storage()
            .is_some_and(|receipt| floor < receipt.retained_storage())
        {
            return Err(ScopedModuleErrorV29::from(ArgumentResourceV1::Accounting).into());
        }
        with_scoped_source_cleanup_v29(budget, floor, move |cleanup, budget| {
            let floor = budget.storage();
            scoped_source_attempt_v29(cleanup, budget, floor, move |budget| {
                let mut source = capture_pending_source_inputs_v18(owner, launch, input, budget)?;
                if let Some(kernel_abi) = kernel_abi {
                    source.input.capture_kernel_argument_abi_v18(
                        &source.owner,
                        kernel_abi,
                        budget,
                    )?;
                }
                let mut donor = Some(source);
                SourceOwnedScopedModuleV29::try_new_with_cleanup(
                    &mut donor, limits, cleanup, budget,
                )
                .map(|inner| Self { inner })
            })
        })
        .map_err(Into::into)
    }

    /// Reconstructs against the retained source on the same live resource ledger.
    ///
    /// This establishes graph/attachment replay only, not the pending semantic or
    /// discharge obligations. Temporary storage is released, work remains charged.
    pub fn replay_with_budget(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionPendingScopedSourceErrorV29> {
        self.inner.replay(budget).map_err(Into::into)
    }

    /// Returns non-authoritative graph data for inspection, not an executable.
    pub fn pending_module(&self) -> &Module {
        self.inner.pending.graph.module()
    }

    /// Returns the identity of the complete pending canonical V18 graph and table.
    pub fn pending_identity(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV18 {
        self.inner.pending.graph.identity()
    }

    /// Returns the retained source digest, not a producer-authentication receipt.
    pub fn source_semantic_sha256(&self) -> &[u8; 32] {
        self.inner.source.owner.source_semantic_sha256()
    }

    /// Counts replayed instance-qualified assertion attachments, not proven assertions.
    pub fn assertion_attachment_count(&self) -> usize {
        self.inner.assertions.len()
    }

    /// Returns the newly adopted logical storage reservation, already live.
    ///
    /// Keep it reserved while this owner lives; release it only after dropping the
    /// owner. This excludes a preexisting occurrence capture, whose original
    /// reservation remains separately live. This is neither allocator capacity
    /// telemetry nor a total including upstream source/SSA/launch allocations.
    pub fn adopted_storage(&self) -> usize {
        self.inner.retained_storage
    }
}

// Capture owns the projection before any module table or root-emission scope
// exists. The producer and the source-owning continuation share this exact join.
fn capture_pending_source_inputs_v18(
    owner: ProductionSemanticSsaOwnerV1,
    launch: crate::ProductionSourceLaunchRosterV1,
    input: crate::ProductionExecutionSourceInputV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedSourceInputsV29, ScopedModuleErrorV29> {
    let input = {
        let source = ExecutionLifecycleSourceV29::new(&owner, &launch, input, budget)?;
        OwnedExecutionInputV29::capture(&source, budget)?
    };
    Ok(ScopedSourceInputsV29 { owner, launch, input })
}
