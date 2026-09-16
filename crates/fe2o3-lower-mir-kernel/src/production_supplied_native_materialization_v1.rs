/// Exact emitter replay consistency for supplied native N and its source catalog.
///
/// This is not independently proved semantic preservation: replay uses the same
/// lowering algorithm and cannot detect semantic bugs shared with that emitter.
/// It grants no source functional/reference/aggregate, producer-execution,
/// protected-compilation, artifact or launch authority. It authenticates no
/// serialized SSA, occurrence or correspondence rows. Source-compatible launch
/// inputs are retained, not proved to be the original producer's configuration;
/// not every launch input is necessarily represented injectively in N.
///
/// All four input owners must outlive this move-only borrowed result. The caller
/// retains their existing receipts and immediately reserves the returned header
/// receipt while retaining this result. No replayed executable owner escapes.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ReplayedSuppliedNativeMaterializationV1<'static, 'static, 'static, 'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1;
/// fn escape<'s, 'l, 'n, 'c>(r: ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c>)
///     -> ReplayedSuppliedNativeMaterializationV1<'static, 'l, 'n, 'c> { r }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1;
/// fn escape<'s, 'l, 'n, 'c>(r: ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c>)
///     -> ReplayedSuppliedNativeMaterializationV1<'s, 'static, 'n, 'c> { r }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1;
/// fn escape<'s, 'l, 'n, 'c>(r: ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c>)
///     -> ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'static, 'c> { r }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1;
/// fn escape<'s, 'l, 'n, 'c>(r: ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c>)
///     -> ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'static> { r }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1;
/// fn forge<'s, 'l, 'n, 'c>(r: ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c>) {
///     let ReplayedSuppliedNativeMaterializationV1 { source, launch, native, catalog } = r;
/// }
/// ```
#[must_use = "this borrowed consistency result is not compilation authority"]
#[derive(Debug)]
pub struct ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c> {
    source: &'s ProductionSemanticSsaOwnerV1,
    launch: &'l crate::ProductionSourceLaunchRosterV1,
    native: &'n fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    catalog: &'c fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
}

impl<'s, 'l, 'n, 'c> ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c> {
    /// The exact supplied source/SSA owner, not reconstructed producer custody.
    pub const fn source(&self) -> &'s ProductionSemanticSsaOwnerV1 {
        self.source
    }
    /// The exact source-compatible supplied launch configuration.
    pub const fn launch(&self) -> &'l crate::ProductionSourceLaunchRosterV1 {
        self.launch
    }
    /// The exact supplied admitted native executable, not the temporary replay.
    pub const fn native(&self) -> &'n fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.native
    }
    /// The complete supplied catalog whose bytes matched source derivation.
    pub const fn catalog(&self) -> &'c fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 {
        self.catalog
    }
    /// This consistency relation does not grant any compilation authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Transfer receipt for the inline borrowed result, excluding all input owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuppliedNativeMaterializationStorageV1(usize);
impl SuppliedNativeMaterializationStorageV1 {
    /// Reserve before further allocation while retaining the borrowed result.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Failure of emitter replay consistency for supplied source/native inputs.
#[derive(Debug)]
pub enum SuppliedNativeMaterializationErrorV1 {
    /// The shared canonical ledger refused work, storage or accounting.
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    /// Existing source/SSA replay, launch correspondence or lowering failed.
    Source(ProductionSemanticKirErrorV1),
    /// Existing bytes-only native encoder/inverse admission failed.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12),
    /// Existing source-derived catalog construction failed.
    Catalog(ProductionSourceOutputCatalogErrorV1),
    /// Exact supplied ownership or complete byte content did not match.
    Invalid(&'static str),
}
impl fmt::Display for SuppliedNativeMaterializationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
            Self::Catalog(error) => error.fmt(formatter),
            Self::Invalid(reason) => write!(
                formatter,
                "supplied native materialization rejected: {reason}"
            ),
        }
    }
}
impl std::error::Error for SuppliedNativeMaterializationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Canonical(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

/// Replays the existing emitter and compares complete supplied native/catalog bytes.
///
/// The checked catalog must belong to this exact supplied N owner. The query
/// verifies the supplied source SSA and complete source-launch roster, invokes
/// the original lowering algorithm exactly once, then uses existing bytes-only
/// V12 admission and source-catalog derivation. It runs no binder, optimizer or
/// native lowering and constructs no protected pre-ranked owner.
///
/// Canonical wrapper work is 8 + both native byte lengths + both catalog byte
/// lengths, plus unchanged encoder/inverse-verifier and catalog-builder work.
/// Source verification, SSA replay, launch rows and emitter/correspondence heaps
/// retain their separate existing source/lowering limits: this is not a complete
/// canonical-budget bound or a recoverable-host-allocation guarantee. Optional
/// captured occurrences remain caller-owned and are not captured or decoded here.
///
/// Input receipts stay caller-reserved. Actual temporary receipts are accepted
/// immediately, and their owners drop before release. Every returned path restores
/// incoming storage and preserves work/peak/denial history. Unwind drops temporaries
/// before cleanup; accounting failure precedes ordinary failure or panic resumption.
/// Only the inline result receipt transfers to the caller on success.
pub fn check_supplied_native_materialization_consistency_v1<'s, 'l, 'n, 'c>(
    source: &'s ProductionSemanticSsaOwnerV1,
    launch: &'l crate::ProductionSourceLaunchRosterV1,
    native: &'n fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    catalog: &fe2o3_kernel_analysis::CheckedKernelIrContractCatalogV1<'c, 'n>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<
    (
        ReplayedSuppliedNativeMaterializationV1<'s, 'l, 'n, 'c>,
        SuppliedNativeMaterializationStorageV1,
    ),
    SuppliedNativeMaterializationErrorV1,
> {
    use SuppliedNativeMaterializationErrorV1 as ReplayError;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let floor = budget.storage();
    let header = std::mem::size_of::<ReplayedSuppliedNativeMaterializationV1<'_, '_, '_, '_>>();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.charge_work(4).map_err(ReplayError::Resource)?;
        budget
            .reserve_storage(header)
            .map_err(ReplayError::Resource)?;
        if !catalog.inventory().belongs_to(native) {
            return Err(ReplayError::Invalid("catalog graph owner"));
        }
        let roots = materialization_launch_roots_v1(source, launch).map_err(ReplayError::Source)?;
        let (module, correspondence) =
            lower_module(source, limits, Some(&roots)).map_err(ReplayError::Source)?;
        let (replayed, replay_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::
            from_module_ref_with_verification_budget_v12(&module, budget).map_err(ReplayError::Canonical)?;
        budget
            .reserve_storage(replay_storage.retained_storage())
            .map_err(ReplayError::Resource)?;
        let comparison = replayed
            .canonical_bytes()
            .len()
            .checked_add(native.canonical().canonical_bytes().len())
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(ReplayError::Resource(Resource::Arithmetic))?;
        budget
            .charge_work(comparison)
            .map_err(ReplayError::Resource)?;
        if replayed.canonical_bytes() != native.canonical().canonical_bytes() {
            return Err(ReplayError::Invalid("complete native bytes"));
        }
        drop(replayed);
        budget
            .release_storage(replay_storage.retained_storage())
            .map_err(ReplayError::Resource)?;
        let (derived, derived_storage) = source_catalog_from_live_v1(
            source.source_semantic(),
            SourceCatalogCorrespondenceV1(&correspondence),
            catalog.inventory(),
            budget,
        )
        .map_err(ReplayError::Catalog)?;
        budget
            .reserve_storage(derived_storage.retained_storage())
            .map_err(ReplayError::Resource)?;
        let comparison = derived
            .canonical_bytes()
            .len()
            .checked_add(catalog.catalog().canonical_bytes().len())
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(ReplayError::Resource(Resource::Arithmetic))?;
        budget
            .charge_work(comparison)
            .map_err(ReplayError::Resource)?;
        if derived.canonical_bytes() != catalog.catalog().canonical_bytes() {
            return Err(ReplayError::Invalid("complete source catalog bytes"));
        }
        drop(derived);
        budget
            .release_storage(derived_storage.retained_storage())
            .map_err(ReplayError::Resource)?;
        Ok(ReplayedSuppliedNativeMaterializationV1 {
            source,
            launch,
            native,
            catalog: catalog.catalog(),
        })
    }));
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ReplayError::Resource(Resource::Accounting))?;
    budget
        .release_storage(release)
        .map_err(ReplayError::Resource)?;
    match outcome {
        Ok(result) => {
            result.map(|relation| (relation, SuppliedNativeMaterializationStorageV1(header)))
        }
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
