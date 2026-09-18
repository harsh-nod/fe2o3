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
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionSemanticMirErrorV1,
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaErrorV1,
    ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
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
    /// Fresh ranked checks did not correspond to the reconstructed source owner.
    RankedSource(super::ProductionSemanticKirErrorV1),
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
    retained_storage: usize,
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
    /// The complete replay receipt which must remain reserved while this owner lives.
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
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

/// An inert typed recipe, with no ranked verification or source-origin authority.
/// The borrowed form leaves all nested recipe allocations with their existing owner.
#[derive(Clone, Copy, Debug)]
pub struct NativeRankedSourceCandidateV1<'a> {
    semantic_root: u32,
    launch_rank: u8,
    kernel: &'a ProductionRankedKernelV1,
    access_sources: &'a [super::ProductionRankedAccessSourceV1],
    executable_effect_sources: &'a [super::ProductionRankedExecutableEffectSourceV1],
    ranked_ir: &'a str,
}

impl<'a> NativeRankedSourceCandidateV1<'a> {
    /// Packages untrusted parts without validating their relation or provenance.
    pub const fn from_untrusted_parts(
        semantic_root: u32,
        launch_rank: u8,
        kernel: &'a ProductionRankedKernelV1,
        access_sources: &'a [super::ProductionRankedAccessSourceV1],
        executable_effect_sources: &'a [super::ProductionRankedExecutableEffectSourceV1],
        ranked_ir: &'a str,
    ) -> Self {
        Self {
            semantic_root,
            launch_rank,
            kernel,
            access_sources,
            executable_effect_sources,
            ranked_ir,
        }
    }
    /// Canonical semantic root selected by this candidate.
    pub const fn semantic_root(&self) -> u32 {
        self.semantic_root
    }
    /// Source launch rank claimed by this candidate.
    pub const fn launch_rank(&self) -> u8 {
        self.launch_rank
    }
    /// Complete typed ranked construction recipe, not a verified graph.
    pub const fn kernel(&self) -> &'a ProductionRankedKernelV1 {
        self.kernel
    }
    /// Complete ordered source/access correspondence recipe.
    pub const fn access_sources(&self) -> &'a [super::ProductionRankedAccessSourceV1] {
        self.access_sources
    }
    /// Complete ordered compiler-generated executable-effect recipe.
    pub const fn executable_effect_sources(
        &self,
    ) -> &'a [super::ProductionRankedExecutableEffectSourceV1] {
        self.executable_effect_sources
    }
    /// Retained diagnostic text; never interpreted as a semantic proof.
    pub const fn ranked_ir(&self) -> &'a str {
        self.ranked_ir
    }
}

/// Actual returned candidate vector capacity plus its header. Borrowed nested
/// fields remain covered by the source owner's separate allocation domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRankedCandidateStorageV1(usize);
impl NativeRankedCandidateStorageV1 {
    /// Reserve before another allocation, and drop the vector before release.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Exports every genuine retained source root in canonical order. No subset or
/// caller-provided text is accepted. Work is `8 + 5 * roots + 2 * name bytes`;
/// each name is compared with the actual typed kernel. Actual vector capacity
/// is reserved before writing any row. The incoming storage floor is restored.
pub fn native_source_ranked_candidates_v1<'a>(
    source: &'a super::ProductionSemanticKirOwnerV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        Vec<NativeRankedSourceCandidateV1<'a>>,
        NativeRankedCandidateStorageV1,
    ),
    NativeSourceReplayErrorV1,
> {
    budget.charge_work(8)?;
    let minimum = source.pre_ranked_retained_analysis_storage_v1().ok_or(
        NativeSourceReplayErrorV1::Mismatch("missing native source owner"),
    )?;
    if budget.storage() < minimum {
        return Err(Resource::Accounting.into());
    }
    native_source_transfer_v1(budget, |budget| {
        let roots = source.semantic().semantic().roots();
        if roots.is_empty() || roots.len() != source.generic_checks.len() {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "complete typed ranked roster",
            ));
        }
        let header = std::mem::size_of::<Vec<NativeRankedSourceCandidateV1<'_>>>();
        budget.reserve_storage(header)?;
        let mut candidates = native_source_vector_v1(roots.len(), budget)?;
        for (root, retained) in roots.iter().zip(source.generic_checks.iter()) {
            budget.charge_work(
                5usize
                    .checked_add(retained.function_name.len())
                    .and_then(|n| n.checked_add(retained.lowering.kernel().function_name().len()))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if *root != retained.selected_root
                || retained.function_name != retained.lowering.kernel().function_name()
            {
                return Err(NativeSourceReplayErrorV1::Mismatch(
                    "typed ranked root identity",
                ));
            }
            candidates.push(NativeRankedSourceCandidateV1::from_untrusted_parts(
                root.index(),
                retained.launch_rank,
                retained.lowering.kernel(),
                &retained.access_sources,
                &retained.executable_effect_sources,
                &retained.ranked_ir,
            ));
        }
        let storage = header
            .checked_add(native_source_vector_capacity_v1(&candidates)?)
            .ok_or(Resource::Arithmetic)?;
        Ok((candidates, NativeRankedCandidateStorageV1(storage)))
    })
}

/// Reconstructed native source retaining freshly compiled ranked checks and
/// their independently replayed source correspondence. This is not publication
/// authority or a claim of complete indexed-address/operational equivalence.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ReplayedRankedNativeSourceV1;
/// fn copy(owner: ReplayedRankedNativeSourceV1) { let _ = owner.clone(); }
/// ```
pub struct ReplayedRankedNativeSourceV1 {
    source: super::ProductionSemanticKirOwnerV1,
    catalog: Catalog,
}
impl ReplayedRankedNativeSourceV1 {
    /// The actual retained source/ranked owner, not a detached success report.
    pub const fn source(&self) -> &super::ProductionSemanticKirOwnerV1 {
        &self.source
    }
    /// The catalog independently checked against the reconstructed native graph.
    pub const fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    /// Independent source/ranked equality does not authenticate compiler origin.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    /// No object, runtime, launch, or publication authority is created.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Additional copied transport and wrapper storage; the consumed replay source
/// and freshly compiled lowerings keep their separate incoming reservations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRankedAttachmentStorageV1(usize);
impl NativeRankedAttachmentStorageV1 {
    /// Reserve alongside the input receipts, and release only after owner drop.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Attaches a complete fresh ranked roster to the already reconstructed N owner,
/// then replays the resulting source correspondence. Exact candidate recipes
/// must equal the fresh lowering recipes before attachment. All inputs are
/// consumed on failure; their incoming reservations remain caller-owned.
///
/// New copied maps/text, root vectors and wrapper are charged using their actual
/// capacities. Existing ranked compilation and source reconstruction retain
/// their separate bounded allocation/work domains, as in production attachment.
/// This receipt is not a claim of complete allocator/RSS accounting.
pub fn attach_replayed_native_source_ranked_v1(
    replayed: ReplayedNativeSourceV1,
    candidates: &[NativeRankedSourceCandidateV1<'_>],
    lowerings: Vec<ProductionRankedKernelLoweringInputV1>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ReplayedRankedNativeSourceV1,
        NativeRankedAttachmentStorageV1,
    ),
    NativeSourceReplayErrorV1,
> {
    budget.charge_work(8)?;
    if budget.storage() < replayed.retained_storage {
        return Err(Resource::Accounting.into());
    }
    native_source_transfer_v1(budget, move |budget| {
        let semantic_roots = replayed.source.semantic_ssa().source_semantic().roots();
        if candidates.is_empty()
            || candidates.len() != semantic_roots.len()
            || candidates.len() != lowerings.len()
        {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "complete fresh ranked roster",
            ));
        }
        let wrapper = std::mem::size_of::<ReplayedRankedNativeSourceV1>();
        budget.reserve_storage(wrapper)?;
        let mut roots = native_source_vector_v1(candidates.len(), budget)?;
        let retained_roots = candidates
            .len()
            .checked_mul(std::mem::size_of::<super::RetainedGenericKernelChecksV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(retained_roots)?;
        let mut retained = wrapper
            .checked_add(retained_roots)
            .ok_or(Resource::Arithmetic)?;
        for ((candidate, lowering), semantic_root) in
            candidates.iter().zip(lowerings).zip(semantic_roots)
        {
            budget.charge_work(5)?;
            // Typed recipe equality belongs to the existing ranked construction
            // domain; no diagnostic string is parsed to establish this relation.
            if candidate.semantic_root != semantic_root.index()
                || candidate.kernel != lowering.kernel()
            {
                return Err(NativeSourceReplayErrorV1::Mismatch(
                    "exact fresh ranked root/recipe",
                ));
            }
            let function_name_bytes = candidate.kernel.function_name().len();
            budget.charge_work(function_name_bytes)?;
            budget.reserve_storage(function_name_bytes)?;
            retained = retained
                .checked_add(function_name_bytes)
                .ok_or(Resource::Arithmetic)?;
            let mut access_sources =
                native_source_vector_v1(candidate.access_sources.len(), budget)?;
            budget.charge_work(candidate.access_sources.len())?;
            access_sources.extend_from_slice(candidate.access_sources);
            let mut effect_sources =
                native_source_vector_v1(candidate.executable_effect_sources.len(), budget)?;
            budget.charge_work(candidate.executable_effect_sources.len())?;
            effect_sources.extend_from_slice(candidate.executable_effect_sources);
            budget.charge_work(candidate.ranked_ir.len())?;
            budget.reserve_storage(candidate.ranked_ir.len())?;
            let mut ranked_ir = String::new();
            ranked_ir
                .try_reserve_exact(candidate.ranked_ir.len())
                .map_err(|_| Resource::Allocation)?;
            budget.reserve_storage(
                ranked_ir
                    .capacity()
                    .checked_sub(candidate.ranked_ir.len())
                    .ok_or(Resource::Accounting)?,
            )?;
            ranked_ir.push_str(candidate.ranked_ir);
            retained = retained
                .checked_add(
                    candidate
                        .access_sources
                        .len()
                        .checked_mul(std::mem::size_of::<super::ProductionRankedAccessSourceV1>())
                        .ok_or(Resource::Arithmetic)?,
                )
                .and_then(|n| {
                    n.checked_add(candidate.executable_effect_sources.len().checked_mul(
                        std::mem::size_of::<super::ProductionRankedExecutableEffectSourceV1>(),
                    )?)
                })
                .and_then(|n| n.checked_add(candidate.ranked_ir.len()))
                .ok_or(Resource::Arithmetic)?;
            roots.push(super::ProductionRankedSemanticProjectionRootV1::new(
                *semantic_root,
                candidate.launch_rank,
                lowering,
                ranked_ir,
                access_sources,
                effect_sources,
            ));
        }
        let ReplayedNativeSourceV1 {
            source, catalog, ..
        } = replayed;
        let receipt = super::ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(source, roots)
            .map_err(NativeSourceReplayErrorV1::RankedSource)?;
        let source =
            super::ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
                .map_err(NativeSourceReplayErrorV1::RankedSource)?;
        // The existing attachment creates these strings. Their exact retained
        // capacities, not just the requested payload, transfer with this owner.
        for root in &source.generic_checks {
            let surplus = root
                .function_name
                .capacity()
                .checked_sub(root.function_name.len())
                .ok_or(Resource::Accounting)?;
            budget.reserve_storage(surplus)?;
            retained = retained.checked_add(surplus).ok_or(Resource::Arithmetic)?;
        }
        source
            .verify_equivalence()
            .map_err(NativeSourceReplayErrorV1::RankedSource)?;
        Ok((
            ReplayedRankedNativeSourceV1 { source, catalog },
            NativeRankedAttachmentStorageV1(retained),
        ))
    })
}

fn native_source_vector_capacity_v1<T>(values: &Vec<T>) -> Result<usize, Resource> {
    values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)
}

fn native_source_vector_v1<T>(length: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Resource> {
    let requested = length
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        native_source_vector_capacity_v1(&values)?
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(values)
}

fn native_source_transfer_v1<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, NativeSourceReplayErrorV1>,
) -> Result<T, NativeSourceReplayErrorV1> {
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget)));
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
    let (parts, storage) = replay_native_source_parts_v1(
        semantic_bytes,
        native_n_bytes,
        catalog_bytes,
        launch_inputs,
        NativeSourceReplayRouteV1::RawEmpty,
        std::mem::size_of::<ReplayedNativeSourceV1>(),
        budget,
    )?;
    Ok((
        ReplayedNativeSourceV1 {
            source: parts.source,
            catalog: parts.catalog,
            retained_storage: storage.retained_storage(),
        },
        storage,
    ))
}

#[derive(Clone, Copy)]
enum NativeSourceReplayRouteV1 {
    RawEmpty,
    UnitLocal,
}

struct ReplayedNativeSourcePartsV1 {
    source: ProductionPreRankedKirOwnerV1,
    catalog: Catalog,
}

#[allow(clippy::too_many_arguments)]
fn replay_native_source_parts_v1(
    semantic_bytes: &[u8],
    native_n_bytes: &[u8],
    catalog_bytes: &[u8],
    launch_inputs: &[ProductionSourceLaunchRootInputV1<'_>],
    route: NativeSourceReplayRouteV1,
    wrapper_bytes: usize,
    budget: &mut Budget<'_>,
) -> Result<(ReplayedNativeSourcePartsV1, NativeSourceReplayStorageV1), NativeSourceReplayErrorV1> {
    budget.charge_work(4)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let wrapper = wrapper_bytes
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
        let source_storage = match route {
            NativeSourceReplayRouteV1::RawEmpty => source.retained_analysis_storage_v1(),
            NativeSourceReplayRouteV1::UnitLocal => source
                .unit_local_source_storage_floor_v1()
                .map_err(NativeSourceReplayErrorV1::RankedSource)?,
        };
        budget.reserve_storage(source_storage)?;
        budget.charge_work(1)?;
        let expected = match route {
            NativeSourceReplayRouteV1::RawEmpty => ProductionHelperSourcePolicyV1::RawEmpty,
            NativeSourceReplayRouteV1::UnitLocal => ProductionHelperSourcePolicyV1::UnitLocal,
        };
        if source.helper_source_policy_v1() != expected {
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
            ReplayedNativeSourcePartsV1 { source, catalog },
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

#[path = "native_unit_local_source_replay_v1.rs"]
mod unit_local;
pub use unit_local::*;
