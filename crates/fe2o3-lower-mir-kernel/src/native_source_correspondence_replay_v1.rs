//! Independent normal-constructor replay of a native source/N subject.

use crate::{
    ProductionHelperSourcePolicyV1, ProductionPreRankedKirErrorV1, ProductionPreRankedKirOwnerV1,
    ProductionSemanticKirLimitsV1, ProductionSourceLaunchErrorV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, KernelIrContractCatalogBindingErrorV1,
    check_kernel_ir_contract_catalog_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog, KernelIrContractCatalogErrorV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticMirDecodeErrorV1, SemanticMirLimitsV1,
};
use fe2o3_pliron::{
    ProductionSemanticMirErrorV1, ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
    ProductionSemanticSsaErrorV1, ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
};

/// Failure of normal source replay or an exact retained ranked-subject join.
#[derive(Debug)]
pub enum NativeSourceReplayErrorV1 {
    /// The shared work or storage ledger refused the operation.
    Resource(Resource),
    /// Canonical semantic MIR did not decode.
    Semantic(SemanticMirDecodeErrorV1),
    /// Semantic ownership was not admitted.
    Source(ProductionSemanticMirErrorV1),
    /// Source SSA planning failed.
    Ssa(ProductionSemanticSsaErrorV1),
    /// The complete launch roster did not match semantic roots.
    Launch(ProductionSourceLaunchErrorV1),
    /// Normal native materialization failed.
    Materialize(ProductionPreRankedKirErrorV1),
    /// The native contract catalog did not decode.
    Catalog(KernelIrContractCatalogErrorV1),
    /// The reconstructed native inventory could not be derived.
    Inventory(CanonicalKirInventoryErrorV1),
    /// The catalog did not bind the exact reconstructed graph.
    CatalogBinding(KernelIrContractCatalogBindingErrorV1),
    /// An explicitly named subject or supported-policy equality failed.
    Mismatch(&'static str),
}

impl std::fmt::Display for NativeSourceReplayErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native source correspondence replay: {self:?}")
    }
}
impl std::error::Error for NativeSourceReplayErrorV1 {}
impl From<Resource> for NativeSourceReplayErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}

/// Complete owned reconstruction. Its native graph is the normal materializer's
/// output, not a caller Module. Launch origin and compiler origin remain separate.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedNativeSourceV1;
/// fn copy(value: ReplayedNativeSourceV1) { let _ = value.clone(); }
/// ```
pub struct ReplayedNativeSourceV1 {
    source: ProductionPreRankedKirOwnerV1,
    catalog: Catalog,
}
impl ReplayedNativeSourceV1 {
    /// Borrows the complete normal-constructor source/N owner.
    pub fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        &self.source
    }
    /// Borrows the exact independently graph-checked catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    /// Detached retained launch equality does not authenticate its origin.
    pub const fn authenticates_launch_origin(&self) -> bool {
        false
    }
    /// Reconstruction grants no artifact or runtime authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Native graph/origin/helper and catalog receipts, wrapper header and retained
/// semantic encoded payload. Existing semantic decoding/planning/launch engines
/// keep their separate bounded allocation domains; this is not heap/RSS coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSourceReplayStorageV1(usize);
impl NativeSourceReplayStorageV1 {
    /// Additional retained logical storage to reserve before further allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Inert copies of the ordered receipt commitments retained by one exact ranked
/// root. These digests are not a receipt, signature, or proof by themselves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRankedStagingCommitmentV1 {
    digests: [[u8; 32]; 9],
}

impl NativeRankedStagingCommitmentV1 {
    /// Retained effect receipt identity.
    pub const fn receipt(&self) -> [u8; 32] {
        self.digests[0]
    }
    /// Retained normalized effect obligation identity.
    pub const fn effect(&self) -> [u8; 32] {
        self.digests[1]
    }
    /// Retained receipt signer identity.
    pub const fn signer(&self) -> [u8; 32] {
        self.digests[2]
    }
    /// Retained proof execution identity.
    pub const fn execution(&self) -> [u8; 32] {
        self.digests[3]
    }
    /// Verus executable/configuration, solver executable/configuration, runtime
    /// closure, in the exact aggregate-obligation hashing order.
    pub const fn toolchain(&self) -> [[u8; 32]; 5] {
        [
            self.digests[4],
            self.digests[5],
            self.digests[6],
            self.digests[7],
            self.digests[8],
        ]
    }
}

/// Actual returned vector capacity plus its header, transferred to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRankedStagingStorageV1(usize);
impl NativeRankedStagingStorageV1 {
    /// Reserve before another allocation; drop the returned vector before release.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Copies the complete ordered staging roster from one exact retained source
/// root, never from caller-supplied ranked text or claims. Both root ordinal and
/// source ID must match the owner. The caller of a complete-module replay must
/// query every root; a successful individual query is not a module proof.
///
/// Work is exactly `13 + 289 * rows`: entry6, root checks4, reservation3 and one
/// row visit plus nine 32-byte copies. Actual vector capacity is reserved before
/// copying. Result/unwind restore the incoming full storage floor on the same
/// Work ledger; the source's retained analysis receipt must already be reserved.
pub fn native_source_ranked_staging_commitments_v1(
    source: &super::ProductionSemanticKirOwnerV1,
    root_ordinal: usize,
    semantic_root: u32,
    budget: &mut Budget<'_>,
) -> Result<
    (
        Vec<NativeRankedStagingCommitmentV1>,
        NativeRankedStagingStorageV1,
    ),
    NativeSourceReplayErrorV1,
> {
    budget.charge_work(6)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let minimum = source.pre_ranked_retained_analysis_storage_v1().ok_or(
        NativeSourceReplayErrorV1::Mismatch("missing native source owner"),
    )?;
    if floor < minimum {
        return Err(Resource::Accounting.into());
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.charge_work(4)?;
        let roots = source.semantic().semantic().roots();
        let retained = source
            .generic_checks
            .get(root_ordinal)
            .ok_or(NativeSourceReplayErrorV1::Mismatch("retained staging root"))?;
        if source.generic_checks.len() != roots.len()
            || roots.get(root_ordinal).map(|root| root.index()) != Some(semantic_root)
            || retained.selected_root.index() != semantic_root
        {
            return Err(NativeSourceReplayErrorV1::Mismatch("retained staging root"));
        }
        let receipts = retained
            .lowering
            .retained_policy_checked_refinement_staging();
        budget.charge_work(3)?;
        let header = std::mem::size_of::<Vec<NativeRankedStagingCommitmentV1>>();
        let requested = receipts
            .len()
            .checked_mul(std::mem::size_of::<NativeRankedStagingCommitmentV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(header.checked_add(requested).ok_or(Resource::Arithmetic)?)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(receipts.len())
            .map_err(|_| Resource::Allocation)?;
        let capacity = rows
            .capacity()
            .checked_mul(std::mem::size_of::<NativeRankedStagingCommitmentV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(
            capacity
                .checked_sub(requested)
                .ok_or(Resource::Accounting)?,
        )?;
        for receipt in receipts {
            budget.charge_work(289)?;
            let toolchain = receipt.toolchain();
            rows.push(NativeRankedStagingCommitmentV1 {
                digests: [
                    *receipt.receipt_identity().digest().as_bytes(),
                    *receipt
                        .binding()
                        .normalized_obligation_effect_ir_hash()
                        .as_bytes(),
                    *receipt.signer_identity().as_bytes(),
                    *receipt.execution_identity().as_bytes(),
                    *toolchain.verus_executable().as_bytes(),
                    *toolchain.verus_configuration().as_bytes(),
                    *toolchain.solver_executable().as_bytes(),
                    *toolchain.solver_configuration().as_bytes(),
                    *toolchain.runtime_closure().as_bytes(),
                ],
            });
        }
        Ok((
            rows,
            NativeRankedStagingStorageV1(header.checked_add(capacity).ok_or(Resource::Arithmetic)?),
        ))
    }));
    if token != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let Some(release) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(Resource::Accounting.into());
    };
    if let Err(error) = budget.release_storage(release) {
        drop(result);
        return Err(error.into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Compares a complete ordered `(semantic root, function name, ranked text)`
/// roster with the exact checks retained inside the attached source owner.
/// This is an owner join, not a detached ranked proof or a source revalidation.
/// Every retained root is required; no caller-selected subset can pass. The
/// source analysis receipt must already be reserved. No storage is allocated.
pub fn check_native_source_ranked_roster_v1(
    source: &super::ProductionSemanticKirOwnerV1,
    roots: &[(u32, &str, &str)],
    budget: &mut Budget<'_>,
) -> Result<(), NativeSourceReplayErrorV1> {
    budget.charge_work(4)?;
    let floor = source.pre_ranked_retained_analysis_storage_v1().ok_or(
        NativeSourceReplayErrorV1::Mismatch("missing native source owner"),
    )?;
    if budget.storage() < floor {
        return Err(Resource::Accounting.into());
    }
    let semantic_roots = source.semantic().semantic().roots();
    if roots.len() != source.generic_checks.len()
        || roots.len() != semantic_roots.len()
        || roots.is_empty()
    {
        return Err(NativeSourceReplayErrorV1::Mismatch(
            "complete retained ranked roster",
        ));
    }
    for ((&(root, name, text), retained), semantic_root) in roots
        .iter()
        .zip(source.generic_checks.iter())
        .zip(semantic_roots)
    {
        let work = 4usize
            .checked_add(name.len())
            .and_then(|n| n.checked_add(retained.function_name.len()))
            .and_then(|n| n.checked_add(text.len()))
            .and_then(|n| n.checked_add(retained.ranked_ir.len()))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(work)?;
        if root != semantic_root.index()
            || root != retained.selected_root.index()
            || name != retained.function_name
            || text != retained.ranked_ir.as_ref()
        {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "exact retained ranked root/text",
            ));
        }
    }
    Ok(())
}

/// Reconstructs normal source/SSA/materialized N and compares the complete N
/// bytes before exposing the owned result. The first catalog policy is empty:
/// actual Execution/import markers still fail the independent graph checker.
/// Caller retains borrowed bytes and launch inputs. Their origin is not inferred
/// from equality. A protected caller must authenticate the original launch fields.
///
/// Fixed entry costs four work units; canonical byte comparisons charge their
/// complete lengths before reads. Called engines retain their own schedules.
/// Reserve the returned receipt before further allocation and drop the owner
/// before releasing it. Result and unwind restore the incoming storage floor.
pub fn replay_native_source_correspondence_v1(
    semantic_bytes: &[u8],
    native_n_bytes: &[u8],
    catalog_bytes: &[u8],
    launch_inputs: &[ProductionSourceLaunchRootInputV1<'_>],
    budget: &mut Budget<'_>,
) -> Result<(ReplayedNativeSourceV1, NativeSourceReplayStorageV1), NativeSourceReplayErrorV1> {
    budget.charge_work(4)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let wrapper = std::mem::size_of::<ReplayedNativeSourceV1>()
            .checked_add(semantic_bytes.len())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        budget.charge_work(semantic_bytes.len())?;
        let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            semantic_bytes,
            SemanticMirLimitsV1::default(),
        )
        .map_err(NativeSourceReplayErrorV1::Semantic)?;
        let semantic_identity = *semantic.semantic_sha256().as_bytes();
        let launch = ProductionSourceLaunchRosterV1::try_new(&semantic, launch_inputs)
            .map_err(NativeSourceReplayErrorV1::Launch)?;
        let source = ProductionSemanticMirOwnerV1::try_new(
            semantic,
            ProductionSemanticMirLimitsV1::default(),
        )
        .map_err(NativeSourceReplayErrorV1::Source)?;
        let ssa =
            ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
                .map_err(NativeSourceReplayErrorV1::Ssa)?;
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
        .map_err(NativeSourceReplayErrorV1::Materialize)?;
        let source_storage = source.retained_analysis_storage_v1();
        budget.reserve_storage(source_storage)?;
        budget.charge_work(1)?;
        if source.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::RawEmpty {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "local helper source policy",
            ));
        }
        let actual = source.executable().canonical().canonical_bytes();
        budget.charge_work(
            actual
                .len()
                .checked_add(native_n_bytes.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if actual != native_n_bytes {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "complete normal-materialized N bytes",
            ));
        }
        let (catalog, catalog_storage) = Catalog::decode_with_budget(catalog_bytes, budget)
            .map_err(NativeSourceReplayErrorV1::Catalog)?;
        budget.reserve_storage(catalog_storage.retained_storage())?;
        budget.charge_work(34)?;
        if catalog.semantic_source() != &semantic_identity {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "catalog semantic source",
            ));
        }
        if !catalog.definitions().is_empty() || !catalog.bindings().is_empty() {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "nonempty source catalog is unsupported",
            ));
        }
        {
            let (inventory, storage) = CanonicalKirInventoryV1::derive(source.executable(), budget)
                .map_err(NativeSourceReplayErrorV1::Inventory)?;
            budget.reserve_storage(storage.retained_storage())?;
            let _ = check_kernel_ir_contract_catalog_v1(&inventory, &catalog, budget)
                .map_err(NativeSourceReplayErrorV1::CatalogBinding)?;
            drop(inventory);
            budget.release_storage(storage.retained_storage())?;
        }
        let retained = wrapper
            .checked_add(source_storage)
            .and_then(|n| n.checked_add(catalog_storage.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        Ok((
            ReplayedNativeSourceV1 { source, catalog },
            NativeSourceReplayStorageV1(retained),
        ))
    }));
    if token != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let Some(release) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(Resource::Accounting.into());
    };
    if let Err(error) = budget.release_storage(release) {
        drop(result);
        return Err(error.into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
