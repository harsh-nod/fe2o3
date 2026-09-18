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
    Source(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
    Descriptor(Box<crate::compiler_descriptor::CompilerDescriptorError>),
    Native(dialect_amdgcn::NativeV12TextDescriptorReplayErrorV1),
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
            Self::Source(error) => write!(f, "source: {error}"),
            Self::Descriptor(error) => write!(f, "descriptor: {error}"),
            Self::Native(error) => write!(f, "native: {error}"),
            Self::Context(error) => write!(f, "context: {error}"),
            Self::Mismatch(detail) => f.write_str(detail),
            Self::Panicked => f.write_str("validation panicked"),
        }
    }
}
impl std::error::Error for NativeOutputHandoffErrorV1 {}
type E = NativeOutputHandoffErrorV1;
type R<T> = Result<T, E>;

/// A borrowed closed view, never a new source or publication owner.
#[derive(Clone, Copy)]
pub(crate) enum OutputOwnerV1<'a> {
    Direct(&'a ProductionCheckedOutputOwnerPolicy4V1),
    Erased(&'a ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1),
}
impl<'a> OutputOwnerV1<'a> {
    pub(crate) fn output(self) -> &'a Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    pub(crate) fn source(self, catalog: &'a Catalog) -> R<SourceInputsV1<'a>> {
        match self {
            Self::Direct(v) => SourceInputsV1::direct(v.source_semantic_kir(), catalog),
            Self::Erased(v) => Ok(SourceInputsV1::erased(v.erased_source(), catalog)),
        }
    }
    fn verify(self, budget: &mut Budget<'_>) -> R<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
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
                replay.source().verify_equivalence().map_err(E::Source)?;
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
    let result = match catch_unwind(AssertUnwindSafe(|| next(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(E::Panicked)
        }
    };
    if slot != budget as *const Budget<'_> as usize || ledger != budget.work_ledger_identity_v1() {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let released = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    match released {
        Ok(()) => result,
        Err(error) => {
            drop(result);
            Err(error.into())
        }
    }
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

fn check_stage(inputs: StageInputsV1<'_>, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(4 + 64)?;
    if budget.storage() < inputs.retained_floor {
        return Err(Resource::Accounting.into());
    }
    if inputs
        .bindings
        .rustc_preflight_plan
        .rustc_identity_inventory_sha256()
        != inputs.bindings.rustc_identity_inventory.sha256()
    {
        return Err(E::Mismatch("collector identity/preflight custody"));
    }
    let original = inputs.output.owner.source(inputs.output.catalog)?;
    inputs
        .bindings
        .context_entries
        .validate_source(original.semantic)
        .map_err(|e| E::Context(Box::new(e)))?;
    let replay = inputs.proof.source(budget)?;
    check_source_inputs_v1(original, replay, budget)?;
    check_output_inputs_v1(
        inputs.output,
        inputs.bindings.rustc_target.profile(),
        &inputs.bindings.typed_descriptor_roots,
        budget,
    )
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
        let (handoff, descriptors, expected_llvm) = inputs.prepared.native_output_parts_v1();
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
        let output = inputs.owner.output();
        if inputs.workgroups.len() != output.module().kernels.len() {
            return Err(E::Mismatch("complete actual-O workgroup roster"));
        }
        for ((name, workgroup), kernel) in inputs.workgroups.iter().zip(&output.module().kernels) {
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
            inputs.catalog,
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
