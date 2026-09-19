/// Failure to construct or replay a pending scoped source graph.
#[derive(Debug)]
pub enum ProductionPendingScopedSourceErrorV29 {
    /// Source, lowering, correspondence or resource validation failed.
    Source(ProductionSemanticKirErrorV1),
    /// The complete canonical V15 graph failed admission or replay.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15),
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
    /// On error or panic all newly adopted data drops before restoring the entry
    /// storage floor. Work is never refunded.
    pub fn try_materialize_with_budget(
        owner: ProductionSemanticSsaOwnerV1,
        launch: crate::ProductionSourceLaunchRosterV1,
        input: crate::ProductionExecutionSourceInputV29<'_>,
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
        scoped_module_attempt_v29(budget, floor, move |budget| {
            let input = {
                let source = ExecutionLifecycleSourceV29::new(&owner, &launch, input, budget)?;
                OwnedExecutionInputV29::capture(&source, budget)?
            };
            let mut donor = Some(ScopedSourceInputsV29 {
                owner,
                launch,
                input,
            });
            SourceOwnedScopedModuleV29::try_new(&mut donor, limits, budget)
                .map(|inner| Self { inner })
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

    /// Returns the identity of the complete pending canonical V15 graph.
    pub fn pending_identity(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV15 {
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
