//! Typed final-output/worker binding. No publisher or inert-packet constructor.
use super::{
    AuthenticatedProductionBindings,
    checked_output_policy4_v1::NativeSourceCheckedOutputProductionCompilationV1 as Direct,
    erased_checked_output_policy4_v1::NativeSourceErasedCheckedOutputProductionCompilationV1 as Erased,
};
use crate::{
    compiler_descriptor::TypedDescriptorRootV1,
    production_worker_handoff::PreparedProductionWorkerHandoff,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, NativeNeutralModuleRefV1 as PacketView,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Graph, WorkgroupSize,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedOutputOwnerPolicy4V1, ProductionSourceLaunchRosterV1,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1,
};
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 as Semantic;
use sha2::{Digest, Sha256};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub(crate) enum NativeOutputHandoffErrorV1 {
    Resource(Resource),
    Admission(Box<fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1>),
    Admission5(Box<fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy5V1>),
    Admission6(Box<fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy6V1>),
    Admission7(Box<fe2o3_lower_mir_kernel::ProductionRedundantStoreAdmissionErrorV1>),
    Admission8(Box<fe2o3_lower_mir_kernel::ProductionCommutativeContinuationErrorV1>),
    Source(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
    Descriptor(Box<crate::compiler_descriptor::CompilerDescriptorError>),
    Native(dialect_amdgcn::NativeV12TextDescriptorReplayErrorV1),
    NativePacket(fe2o3_compiler_lineage::NativeNeutralModuleErrorV1),
    NativeSubject(fe2o3_compiler_lineage::NativeNeutralSubjectErrorV1),
    Context(Box<crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1>),
    Mismatch(&'static str),
    Panicked,
}
impl From<Resource> for NativeOutputHandoffErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for NativeOutputHandoffErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native checked-output worker binding: ")?;
        match self {
            Self::Resource(error) => write!(f, "resource: {error}"),
            Self::Admission(error) => write!(f, "admission: {error}"),
            Self::Admission5(error) => write!(f, "Policy5 admission: {error}"),
            Self::Admission6(error) => write!(f, "Policy6 admission: {error}"),
            Self::Admission7(error) => write!(f, "Policy7 admission: {error}"),
            Self::Admission8(error) => write!(f, "Policy8 admission: {error}"),
            Self::Source(error) => write!(f, "source: {error}"),
            Self::Descriptor(error) => write!(f, "descriptor: {error}"),
            Self::Native(error) => write!(f, "native: {error}"),
            Self::NativePacket(error) => write!(f, "original native packet: {error}"),
            Self::NativeSubject(error) => write!(f, "original native subject: {error}"),
            Self::Context(error) => write!(f, "context: {error}"),
            Self::Mismatch(detail) => f.write_str(detail),
            Self::Panicked => f.write_str("validation panicked"),
        }
    }
}
impl std::error::Error for NativeOutputHandoffErrorV1 {}
type E = NativeOutputHandoffErrorV1;
type R<T> = Result<T, E>;

#[path = "production_native_checked_output_policy5_handoff_v1.rs"]
pub(crate) mod policy5;
#[path = "production_native_checked_output_policy6_handoff_v1.rs"]
pub(crate) mod policy6;

/// A borrowed closed view, never a new source or publication owner.
#[derive(Clone, Copy)]
pub(crate) enum OutputOwnerV1<'a> {
    Direct(&'a ProductionCheckedOutputOwnerPolicy4V1),
    Erased(&'a ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1),
    Direct5(&'a fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy5V1),
    Erased5(&'a fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1),
    Direct6(&'a fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1),
    Erased6(&'a fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1),
    Direct7(&'a fe2o3_lower_mir_kernel::ProductionOwnedRedundantStoreContinuationV1),
    Erased7(&'a fe2o3_lower_mir_kernel::ProductionOwnedUnitLocalRedundantStoreContinuationV1),
    Direct8(&'a fe2o3_lower_mir_kernel::ProductionOwnedCommutativeContinuationV1),
    Erased8(&'a fe2o3_lower_mir_kernel::ProductionOwnedUnitLocalCommutativeContinuationV1),
}
impl<'a> OutputOwnerV1<'a> {
    pub(crate) fn output(self) -> &'a Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
            Self::Direct5(v) => v.output(),
            Self::Erased5(v) => v.output(),
            Self::Direct6(v) => v.output(),
            Self::Erased6(v) => v.output(),
            Self::Direct7(v) => v.output(),
            Self::Erased7(v) => v.output(),
            Self::Direct8(v) => v.output(),
            Self::Erased8(v) => v.output(),
        }
    }
    pub(crate) fn source(self, catalog: &'a Catalog) -> R<SourceInputsV1<'a>> {
        match self {
            Self::Direct(v) => SourceInputsV1::direct(v.source_semantic_kir(), catalog),
            Self::Erased(v) => Ok(SourceInputsV1::erased(v.erased_source(), catalog)),
            Self::Direct5(v) => SourceInputsV1::direct(v.source_semantic_kir(), catalog),
            Self::Erased5(v) => Ok(SourceInputsV1::erased(v.erased_source(), catalog)),
            Self::Direct6(v) => SourceInputsV1::direct(v.source_semantic_kir(), catalog),
            Self::Erased6(v) => Ok(SourceInputsV1::erased(v.erased_source(), catalog)),
            Self::Direct7(v) => SourceInputsV1::direct(v.prefix().source_semantic_kir(), catalog),
            Self::Erased7(v) => Ok(SourceInputsV1::erased(v.prefix().erased_source(), catalog)),
            Self::Direct8(v) => {
                SourceInputsV1::direct(v.prefix().prefix().source_semantic_kir(), catalog)
            }
            Self::Erased8(v) => Ok(SourceInputsV1::erased(
                v.prefix().prefix().erased_source(),
                catalog,
            )),
        }
    }
    fn verify(self, budget: &mut Budget<'_>) -> R<()> {
        match self {
            Self::Direct8(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission8(Box::new(e)));
            }
            Self::Erased8(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission8(Box::new(e)));
            }
            Self::Direct7(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission7(Box::new(e)));
            }
            Self::Erased7(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission7(Box::new(e)));
            }
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
            Self::Direct5(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission5(Box::new(e)));
            }
            Self::Erased5(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission5(Box::new(e)));
            }
            Self::Direct6(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission6(Box::new(e)));
            }
            Self::Erased6(v) => {
                return v
                    .verify_equivalence(budget)
                    .map_err(|e| E::Admission6(Box::new(e)));
            }
        }
        .map_err(|e| E::Admission(Box::new(e)))
    }
}

/// Inert borrowed comparison inputs. Only the owning constructor below seals a
/// complete stage, and it derives these fields from its actual retained owners.
#[derive(Clone, Copy)]
pub(crate) struct SourceInputsV1<'a> {
    pub(crate) semantic: &'a Semantic,
    pub(crate) original: &'a Graph,
    pub(crate) erased: Option<&'a Graph>,
    pub(crate) launch: &'a ProductionSourceLaunchRosterV1,
    pub(crate) catalog: &'a Catalog,
}
impl<'a> SourceInputsV1<'a> {
    fn direct(
        source: &'a fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
        catalog: &'a Catalog,
    ) -> R<Self> {
        Ok(Self {
            semantic: source.semantic().semantic(),
            original: source
                .pre_ranked_executable()
                .ok_or(E::Mismatch("original native N"))?,
            erased: None,
            launch: source
                .source_launch_roster()
                .ok_or(E::Mismatch("original source launch"))?,
            catalog,
        })
    }
    fn erased(
        source: &'a fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1,
        catalog: &'a Catalog,
    ) -> Self {
        Self {
            semantic: source.original_source().semantic_ssa().source_semantic(),
            original: source.original_source().executable(),
            erased: Some(source.erased()),
            launch: source.original_source().source_launch(),
            catalog,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum SourceProofV1<'a> {
    Direct(&'a fe2o3_verifier::ValidatedNativeCompilerRankedSourceProofV1),
    Erased(&'a fe2o3_verifier::ValidatedNativeCompilerUnitLocalErasedSourceProofV1),
}
impl<'a> SourceProofV1<'a> {
    fn source(self, budget: &mut Budget<'_>) -> R<SourceInputsV1<'a>> {
        match self {
            Self::Direct(proof) => {
                let replay = proof.source();
                replay
                    .source()
                    .verify_equivalence_with_budget_v1(budget)
                    .map_err(E::Source)?;
                SourceInputsV1::direct(replay.source(), replay.catalog())
            }
            Self::Erased(proof) => {
                let replay = proof.source();
                replay
                    .source()
                    .verify_equivalence(budget)
                    .map_err(E::Source)?;
                Ok(SourceInputsV1::erased(replay.source(), replay.catalog()))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OutputInputsV1<'a> {
    pub(crate) owner: OutputOwnerV1<'a>,
    pub(crate) catalog: &'a Catalog,
    pub(crate) prepared: &'a PreparedProductionWorkerHandoff,
    pub(crate) workgroups: &'a [(String, WorkgroupSize)],
}
pub(super) struct StageInputsV1<'a> {
    pub(super) output: OutputInputsV1<'a>,
    pub(super) proof: SourceProofV1<'a>,
    pub(super) bindings: &'a AuthenticatedProductionBindings,
    pub(super) retained_floor: usize,
}

#[allow(
    clippy::large_enum_variant,
    reason = "retain the existing stage in place without a new allocation; wrapper delta is accounted"
)]
enum StageV1 {
    Direct(Direct),
    Erased(Erased),
}
impl StageV1 {
    fn inputs(&self) -> StageInputsV1<'_> {
        match self {
            Self::Direct(v) => v.native_worker_inputs_v1(),
            Self::Erased(v) => v.native_worker_inputs_v1(),
        }
    }
    fn stored_header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<Direct>(),
            Self::Erased(_) => size_of::<Erased>(),
        }
    }
}

/// Additional wrapper delta only. Source/proof/N/(E)/B/C/O/catalog/native
/// handoff allocations are retained in-place and stay in the incoming floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeOutputHandoffStorageV1(usize);
impl NativeOutputHandoffStorageV1 {
    #[allow(
        dead_code,
        reason = "pending protected-native consumer reserves the returned receipt"
    )]
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Keeps actual signed-source, output, worker and collector custody together.
/// No raw-parts, serialized-receipt, extraction or protected-publisher conversion.
#[allow(
    dead_code,
    reason = "pending versioned protected-native publication consumer"
)]
pub(crate) struct PreparedNativeCheckedOutputWorkerHandoffV1 {
    stage: StageV1,
    retained_floor: usize,
}
#[allow(
    dead_code,
    reason = "pending versioned protected-native publication consumer"
)]
impl PreparedNativeCheckedOutputWorkerHandoffV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.stage.inputs().output.owner.output()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.stage
            .inputs()
            .output
            .prepared
            .native_output_parts_v1()
            .0
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| check_stage(self.stage.inputs(), budget))
    }
}

impl Direct {
    /// Consumes only the already checked signed-source stage. It does not sign
    /// missing proofs, finish protected execution, write files or publish.
    #[allow(dead_code)]
    pub(crate) fn prepare_native_worker_handoff_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedNativeCheckedOutputWorkerHandoffV1,
        NativeOutputHandoffStorageV1,
    )> {
        prepare(StageV1::Direct(self), budget)
    }
}
impl Erased {
    #[allow(dead_code)]
    pub(crate) fn prepare_native_worker_handoff_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedNativeCheckedOutputWorkerHandoffV1,
        NativeOutputHandoffStorageV1,
    )> {
        prepare(StageV1::Erased(self), budget)
    }
}

fn prepare(
    stage: StageV1,
    budget: &mut Budget<'_>,
) -> R<(
    PreparedNativeCheckedOutputWorkerHandoffV1,
    NativeOutputHandoffStorageV1,
)> {
    budget.charge_work(4)?;
    let inherited = stage.inputs().retained_floor;
    if budget.storage() < inherited {
        return Err(Resource::Accounting.into());
    }
    scoped(budget, move |budget| {
        let additional = size_of::<PreparedNativeCheckedOutputWorkerHandoffV1>()
            .checked_sub(stage.stored_header())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(additional)?;
        check_stage(stage.inputs(), budget)?;
        Ok((
            PreparedNativeCheckedOutputWorkerHandoffV1 {
                retained_floor: inherited
                    .checked_add(additional)
                    .ok_or(Resource::Arithmetic)?,
                stage,
            },
            NativeOutputHandoffStorageV1(additional),
        ))
    })
}

pub(crate) fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    next: impl FnOnce(&mut Budget<'work>) -> R<T>,
) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let (result, payload) = match catch_unwind(AssertUnwindSafe(|| next(budget))) {
        Ok(result) => (result, None),
        Err(payload) => (Err(E::Panicked), Some(payload)),
    };
    if slot != budget as *const Budget<'_> as usize || ledger != budget.work_ledger_identity_v1() {
        drop(result);
        drop(payload);
        return Err(Resource::Accounting.into());
    }
    let released = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    let result = match released {
        Ok(()) => result,
        Err(error) => {
            drop(result);
            Err(error.into())
        }
    };
    // A panic payload can itself panic on drop; finish valid-ledger cleanup first.
    drop(payload);
    result
}

fn exact(left: &[u8], right: &[u8], detail: &'static str, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(
        left.len()
            .checked_add(right.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if left != right {
        return Err(E::Mismatch(detail));
    }
    Ok(())
}

pub(crate) fn check_source_inputs_v1(
    original: SourceInputsV1<'_>,
    replay: SourceInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(5)?;
    if original.erased.is_some() != replay.erased.is_some() {
        return Err(E::Mismatch("direct/erased source route"));
    }
    exact(
        original.semantic.canonical_encoding(),
        replay.semantic.canonical_encoding(),
        "exact original semantic source",
        budget,
    )?;
    exact(
        original.original.canonical().canonical_bytes(),
        replay.original.canonical().canonical_bytes(),
        "exact original native N",
        budget,
    )?;
    if let (Some(a), Some(b)) = (original.erased, replay.erased) {
        exact(
            a.canonical().canonical_bytes(),
            b.canonical().canonical_bytes(),
            "exact independently reconstructed E",
            budget,
        )?;
    }
    exact(
        original.catalog.canonical_bytes(),
        replay.catalog.canonical_bytes(),
        "exact original/output contract catalog",
        budget,
    )?;
    let rows = original
        .launch
        .roots()
        .len()
        .checked_add(replay.launch.roots().len())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(
        rows.checked_mul(size_of::<
            fe2o3_lower_mir_kernel::ProductionSourceLaunchRootV1,
        >())
        .and_then(|n| n.checked_add(65))
        .ok_or(Resource::Arithmetic)?,
    )?;
    if original.launch != replay.launch {
        return Err(E::Mismatch("exact original source launch roster"));
    }
    Ok(())
}

pub(super) fn check_stage(inputs: StageInputsV1<'_>, budget: &mut Budget<'_>) -> R<()> {
    check_stage_entry(inputs.retained_floor, budget)?;
    let original = inputs.output.owner.source(inputs.output.catalog)?;
    check_source_binding_prepaid(inputs.bindings, original, inputs.proof, budget)?;
    check_output_inputs_v1(
        inputs.output,
        inputs.bindings.rustc_target.profile(),
        &inputs.bindings.typed_descriptor_roots,
        budget,
    )
}

fn check_stage_entry(retained_floor: usize, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(4 + 64)?;
    if budget.storage() < retained_floor {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

/// Shared collector/source custody; independent of descriptor wire version.
pub(super) fn check_source_binding_v1(
    bindings: &AuthenticatedProductionBindings,
    original: SourceInputsV1<'_>,
    proof: SourceProofV1<'_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(64)?;
    check_source_binding_prepaid(bindings, original, proof, budget)
}

fn check_source_binding_prepaid(
    bindings: &AuthenticatedProductionBindings,
    original: SourceInputsV1<'_>,
    proof: SourceProofV1<'_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    if bindings
        .rustc_preflight_plan
        .rustc_identity_inventory_sha256()
        != bindings.rustc_identity_inventory.sha256()
    {
        return Err(E::Mismatch("collector identity/preflight custody"));
    }
    bindings
        .context_entries
        .validate_source(original.semantic)
        .map_err(|e| E::Context(Box::new(e)))?;
    let replay = proof.source(budget)?;
    check_source_inputs_v1(original, replay, budget)
}

#[cfg(test)]
#[path = "production_native_source_binding_entry_v1_tests.rs"]
mod source_binding_entry_tests;

pub(super) fn check_original_native_packet_v1(
    original: SourceInputsV1<'_>,
    proof: SourceProofV1<'_>,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> R<()> {
    let (erased, subjects) = match proof {
        SourceProofV1::Direct(proof) => (
            false,
            [
                proof.middle_end_roster().native_neutral_subject(),
                proof.correspondence_roster().native_neutral_subject(),
                proof.verus_roster().native_neutral_subject(),
            ],
        ),
        SourceProofV1::Erased(proof) => (
            true,
            [
                proof.middle_end_roster().native_neutral_subject(),
                proof.correspondence_roster().native_neutral_subject(),
                proof.verus_roster().native_neutral_subject(),
            ],
        ),
    };
    check_original_native_packet_components_v1(original, erased, subjects, bytes, budget)
}

/// Inert component check only. Production derives every input from its retained
/// owners above; component success cannot construct a signed-source stage.
pub(crate) fn check_original_native_packet_components_v1(
    original: SourceInputsV1<'_>,
    erased: bool,
    subjects: [&Subject; 3],
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        let graph = original.original.canonical().canonical_bytes();
        let catalog = original.catalog.canonical_bytes();
        // Complete comparison extents plus two fixed subject hashes. Existing
        // codecs keep their bounded internals, not an exact stack/RSS claim.
        let work = bytes
            .len()
            .checked_add(graph.len())
            .and_then(|n| n.checked_add(catalog.len()))
            .and_then(|n| n.checked_add(4 * (2 * size_of::<Subject>() + 1) + 512))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(work)?;
        budget.reserve_storage(
            size_of::<PacketView<'_>>() + size_of::<Subject>() + size_of::<Sha256>(),
        )?;
        if erased != original.erased.is_some() {
            return Err(E::Mismatch("original packet direct/erased route"));
        }
        let expected = Subject::new(
            *original.original.canonical().identity().digest(),
            u64::try_from(graph.len()).map_err(|_| Resource::Arithmetic)?,
            *original.catalog.digest(),
            u64::try_from(catalog.len()).map_err(|_| Resource::Arithmetic)?,
        )
        .map_err(E::NativeSubject)?;
        let packet = PacketView::decode(bytes).map_err(E::NativePacket)?;
        if packet.subject() != &expected {
            return Err(E::Mismatch("exact original packet subject"));
        }
        if packet.graph_bytes() != graph {
            return Err(E::Mismatch("exact original packet N bytes"));
        }
        if packet.catalog_bytes() != catalog {
            return Err(E::Mismatch("exact original packet catalog bytes"));
        }
        if subjects.iter().any(|subject| **subject != expected) {
            return Err(E::Mismatch("each signed-source roster original subject"));
        }
        Ok(())
    })
}

/// The production owning constructor calls this exact component. Component
/// success is not signed-source, collector or protected execution custody.
pub(crate) fn check_output_inputs_v1(
    inputs: OutputInputsV1<'_>,
    profile: Profile,
    typed_roots: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        budget.charge_work(4)?;
        inputs.owner.verify(budget)?;
        crate::compiler_descriptor::checked_output_policy3_v1::native_worker_binding_v1::check_native_worker_descriptor_source_v1(
            inputs.owner, typed_roots, profile, budget,
        )?;
        check_prepared_output_pair_v1(
            inputs.owner.output(),
            inputs.catalog,
            inputs.prepared,
            inputs.workgroups,
            profile,
            budget,
        )
    })
}

// Read-only actual graph/text/descriptor pair validation. This accepts no proof
// and creates no OutputOwnerV1 or protected publication owner.
pub(crate) fn check_prepared_output_pair_v1(
    output: &Graph,
    catalog: &Catalog,
    prepared: &PreparedProductionWorkerHandoff,
    workgroups: &[(String, WorkgroupSize)],
    profile: Profile,
    budget: &mut Budget<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        let (handoff, descriptors, expected_llvm) = prepared.native_output_parts_v1();
        budget.charge_work(
            handoff
                .module_bytes()
                .len()
                .checked_add(32 + 6)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let target = fe2o3_compiler_ffi::DeviceTargetV1::parse(profile.device_target())
            .map_err(|_| E::Mismatch("closed target profile"))?;
        if handoff.target() != target
            || Sha256::digest(handoff.module_bytes()).as_slice() != expected_llvm
            || handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
        {
            return Err(E::Mismatch("exact prepared target/LLVM handoff"));
        }
        if workgroups.len() != output.module().kernels.len() {
            return Err(E::Mismatch("complete actual-O workgroup roster"));
        }
        for ((name, workgroup), kernel) in workgroups.iter().zip(&output.module().kernels) {
            budget.charge_work(
                name.len()
                    .checked_add(kernel.id.as_str().len())
                    .and_then(|n| n.checked_add(5))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if name != kernel.id.as_str() || Some(*workgroup) != kernel.workgroup_size {
                return Err(E::Mismatch("exact actual-O workgroup order"));
            }
        }
        let llvm = std::str::from_utf8(handoff.module_bytes())
            .map_err(|_| E::Mismatch("prepared LLVM UTF-8"))?;
        let _relation = dialect_amdgcn::check_native_v12_text_descriptor_relation_v1(
            output,
            catalog,
            output.canonical().canonical_bytes(),
            profile,
            descriptors.table(),
            llvm,
            budget,
        )
        .map_err(E::Native)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "production_native_handoff_scope_v1_tests.rs"]
mod scoped_tests;
