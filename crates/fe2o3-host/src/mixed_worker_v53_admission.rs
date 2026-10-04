//! Receipt-bearing V53 readmission. Nominal packing never erases V26 contracts.
use super::*;
use fe2o3_compiler_lineage::{
    InertLineageContentIdentityV3, InertProofBindingAssociationInputsV4,
    InertProofBindingAssociationV4,
};
use fe2o3_compiler_lineage::{
    MIXED_MIDDLE_END_WORKING_STORAGE_V50, MixedMiddleEndRefV50, read_mixed_middle_end_v50,
};
use fe2o3_hsaco::InspectedKernelBindings;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53, NominalDescriptorInspectionV53,
    derive_unfinalized_nominal_hsaco_on_budget_v53, inspect_finalized_nominal_hsaco_v53,
};
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53, CanonicalCodeObjectDigest,
    MIXED_DESCRIPTOR_READER_STORAGE_V53, MixedDescriptorTableV53, decode_mixed_descriptor_v53,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, CanonicalKernelIrWorkLimitV1};
use fe2o3_verifier::{InertTypedSourceReceiptV53, check_inert_typed_source_receipt_v53};
use std::mem::size_of;

type AdmissionError = RecoveredWorkerV3AdmissionErrorV1;
type Result<T> = std::result::Result<T, AdmissionError>;
const LINEAGE_DOMAIN: &[u8] = b"fe2o3.host.worker-v3-mixed-descriptor-lineage.v53\0";

#[path = "mixed_worker_v53_preparation.rs"]
mod preparation;
pub use preparation::{MixedWorkerV53PreparationError, PreparedMixedWorkerV53Invocation};

#[path = "mixed_worker_v53_target_readmission.rs"]
mod target_readmission;

fn binding(detail: &'static str) -> AdmissionError {
    AdmissionError::MixedV53(detail)
}
fn codec_error(error: impl std::error::Error + Send + Sync + 'static) -> AdmissionError {
    AdmissionError::MixedCodecV53(Box::new(error))
}
impl From<Resource> for AdmissionError {
    fn from(error: Resource) -> Self {
        codec_error(error)
    }
}
// Currentness and handoff traits have no caller verification ledger. Their
// bounded artifact codec domain must still refuse excessive cumulative work.
const HOST_CODEC_WORK_LIMIT_V53: usize = 512 * 1024 * 1024;
const HOST_CODEC_STORAGE_LIMIT_V53: usize = 4 * NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53
    + MIXED_DESCRIPTOR_READER_STORAGE_V53
    + MIXED_MIDDLE_END_WORKING_STORAGE_V50
    + 64 * 1024;
fn codec_work() -> impl FnMut(usize) -> std::result::Result<(), CanonicalKernelIrWorkLimitV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(HOST_CODEC_WORK_LIMIT_V53);
    move |amount| work.charge_work(amount)
}

fn codec_on_budget<T, F>(budget: &mut Budget<'_>, scratch: usize, operation: F) -> Result<T>
where
    F: FnOnce(&mut Budget<'_>) -> Result<T>,
{
    budget.check_prior_denials_v1()?;
    let frame = scratch
        .checked_add(size_of::<F>())
        .and_then(|n| n.checked_add(size_of::<Budget<'_>>()))
        .and_then(|n| n.checked_add(3 * size_of::<usize>()))
        .and_then(|n| n.checked_add(size_of::<T>()))
        .and_then(|n| n.checked_add(2 * size_of::<Result<T>>()))
        .and_then(|n| {
            n.checked_add(size_of::<
                std::result::Result<Result<T>, Box<dyn std::any::Any + Send>>,
            >())
        })
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, 1, frame, operation)
}

fn codec_scope<T>(
    scratch: usize,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(HOST_CODEC_WORK_LIMIT_V53);
    let mut budget = Budget::new(&mut work, HOST_CODEC_STORAGE_LIMIT_V53);
    codec_on_budget(&mut budget, scratch, operation)
}

struct MixedDescriptor {
    source: Vec<u8>,
    bindings: InspectedKernelBindings,
    digest: CanonicalCodeObjectDigest,
}
impl MixedDescriptor {
    fn table(&self) -> Result<MixedDescriptorTableV53<'_>> {
        codec_scope(MIXED_DESCRIPTOR_READER_STORAGE_V53, |budget| {
            decode_mixed_descriptor_v53(&self.source, &mut |n| budget.charge_work(n))
                .map_err(codec_error)
        })
    }
}

/// Move-only current publication, complete marker roster and V50/V26 association.
///
/// The receipt's strict signature is checked, but its key and claimed runtime
/// are not thereby trusted. A protected measured compiler and native-refinement
/// consumer must authenticate these same owners before any load or dispatch.
/// This value cannot be converted into the unconditional V1 admission.
/// Artifact/codec allocations use the existing bounded readmission domains;
/// `budget` controls the typed-receipt and lineage validation. Other artifact
/// codec traversals have a finite per-operation work limit, not a host RSS claim.
///
/// ```compile_fail
/// use fe2o3_host::RecoveredMixedWorkerV53PinnedRoster;
/// fn clone<R>(v: RecoveredMixedWorkerV53PinnedRoster<R>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::RecoveredMixedWorkerV53PinnedRoster;
/// fn load<R>(v: RecoveredMixedWorkerV53PinnedRoster<R>) { v.load(); }
/// ```
pub struct RecoveredMixedWorkerV53PinnedRoster<R> {
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    custody: ReconstructedReplayCustody,
    descriptor: MixedDescriptor,
    entrypoints: Vec<RecoveredWorkerV3EntrypointV1>,
    lineage: WorkerV3HostLineageEvidenceV1,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<RetainedWorkerV3ApplicationDescriptorsV1>,
    _roster: PhantomData<fn() -> R>,
}
impl<R> fmt::Debug for RecoveredMixedWorkerV53PinnedRoster<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveredMixedWorkerV53PinnedRoster")
            .field("published", &self.published())
            .field("lineage", &self.lineage.identity)
            .field("entrypoint_count", &self.entrypoints.len())
            .finish_non_exhaustive()
    }
}
impl<R> RecoveredMixedWorkerV53PinnedRoster<R> {
    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.envelope.current_publication_lease().published()
    }
    pub fn descriptor_table(&self) -> Result<MixedDescriptorTableV53<'_>> {
        self.descriptor.table()
    }
    pub fn entrypoints(&self) -> &[RecoveredWorkerV3EntrypointV1] {
        &self.entrypoints
    }
    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity
    }
    pub fn typed_receipt(&self, budget: &mut Budget<'_>) -> Result<InertTypedSourceReceiptV53<'_>> {
        codec_on_budget(budget, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |budget| {
            let receipts = self.custody.outer_handoff.capsule().receipts();
            let middle = read_mixed_middle_end_v50(
                receipts.middle_end().canonical_preimage(),
                MIXED_MIDDLE_END_WORKING_STORAGE_V50,
                |n| budget.charge_work(n),
            )
            .map_err(codec_error)?;
            check_inert_typed_source_receipt_v53(middle.input(), budget)
                .map_err(AdmissionError::MixedReceiptV53)
        })
    }
    pub const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.custody.compiler_execution_subject
    }
    pub const fn compiler_execution_receipt(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.envelope.wire().compiler_execution_receipt()
    }
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.custody.finalizer_derivation
    }
    pub fn physical_kernel(&self, ordinal: usize) -> Option<&InspectedKernel> {
        let entry = self.entrypoints.get(ordinal)?;
        self.descriptor
            .bindings
            .inspection()
            .kernels()
            .get(entry.physical_kernel_index)
    }
    pub fn descriptor_binding(&self, ordinal: usize) -> Option<KernelDescriptorBinding> {
        let entry = self.entrypoints.get(ordinal)?;
        self.descriptor
            .bindings
            .bindings()
            .get(entry.physical_kernel_index)
            .copied()
    }
    fn check_current(&self, current: &DurableCurrentLinkPublicationTokenV1) -> Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(descriptors) = &self.application_descriptors {
            descriptors
                .revalidate()
                .map_err(|_| AdmissionError::ApplicationDescriptorsChanged)?;
        }
        validate_retained_replay_custody(
            &self.envelope,
            current,
            &self.custody.finalizer_derivation,
            &self.custody.outer_handoff,
            &self.custody.compiler_execution_subject,
        )?;
        let actual = validate_descriptor(&self.envelope, current, &self.custody)?;
        if actual.source != self.descriptor.source
            || actual.bindings != self.descriptor.bindings
            || actual.digest != self.descriptor.digest
        {
            return Err(AdmissionError::InspectionChanged);
        }
        Ok(())
    }
    pub fn revalidate_currentness(&self) -> Result<()> {
        let current = self
            .envelope
            .current_publication_lease()
            .acquire_current_token()
            .map_err(AdmissionError::CurrentPublication)?;
        self.check_current(&current)
    }
    /// Provides the complete request while holding the current publication lock.
    /// The post-check also runs when the receiver returns an error. Callback
    /// output is not converted to proof, load or dispatch authority.
    pub fn with_verification_request<T, E>(
        &self,
        receive: impl for<'a> FnOnce(
            MixedWorkerV53VerificationRequest<'a, R>,
        ) -> std::result::Result<T, E>,
    ) -> Result<std::result::Result<T, E>> {
        let current = self
            .envelope
            .current_publication_lease()
            .acquire_current_token()
            .map_err(AdmissionError::CurrentPublication)?;
        self.check_current(&current)?;
        let result = receive(MixedWorkerV53VerificationRequest {
            owner: self,
            current: &current,
        });
        self.check_current(&current)?;
        Ok(result)
    }
    #[cfg(target_os = "linux")]
    pub(crate) fn retain_application_descriptors(
        mut self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        self.application_descriptors = Some(descriptors);
        self
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn authenticates_verification_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Borrow of the exact current artifact and all mandatory mixed input owners.
/// No caller can construct this request from claimed hashes or signed wire alone.
pub struct MixedWorkerV53VerificationRequest<'a, R> {
    owner: &'a RecoveredMixedWorkerV53PinnedRoster<R>,
    current: &'a DurableCurrentLinkPublicationTokenV1,
}
impl<'a, R> MixedWorkerV53VerificationRequest<'a, R> {
    pub fn admission(&self) -> &'a RecoveredMixedWorkerV53PinnedRoster<R> {
        self.owner
    }
    pub fn finalized_hsaco_bytes(&self) -> &'a [u8] {
        self.current.exact_artifact_bytes()
    }
    pub fn outer_handoff(&self) -> &'a InertSemanticCompilerModuleHandoffV3 {
        &self.owner.custody.outer_handoff
    }
    pub fn middle_end(&self) -> Result<MixedMiddleEndRefV50<'a>> {
        codec_scope(MIXED_MIDDLE_END_WORKING_STORAGE_V50, |budget| {
            read_mixed_middle_end_v50(
                self.outer_handoff()
                    .capsule()
                    .receipts()
                    .middle_end()
                    .canonical_preimage(),
                MIXED_MIDDLE_END_WORKING_STORAGE_V50,
                |n| budget.charge_work(n),
            )
            .map_err(codec_error)
        })
    }
    pub fn revalidate_currentness(&self) -> Result<()> {
        self.owner.check_current(self.current)
    }
}

/// Replays the real Worker publication before examining any host-selected root.
pub fn admit_recovered_mixed_worker_v53_roster<R: CompilerGeneratedKernelExpectationRosterV1>(
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    budget: &mut Budget<'_>,
) -> Result<RecoveredMixedWorkerV53PinnedRoster<R>> {
    let (custody, current) = reconstruct_replay_custody(&envelope)?;
    let descriptor = validate_descriptor(&envelope, &current, &custody)?;
    let table = descriptor.table()?;
    // validate_descriptor already checked the complete physical target/features.
    let profile = ProductionAmdTargetProfileV1::from_cpu(
        descriptor.bindings.inspection().target().processor(),
    )
    .ok_or_else(|| binding("unsupported inspected V53 target"))?;
    validate_lineage(&custody.outer_handoff, &table, profile, budget)?;
    let mut charge = codec_work();
    let mut identities = Vec::new();
    identities
        .try_reserve_exact(table.kernel_count())
        .map_err(|_| binding("roster storage"))?;
    for ordinal in 0..table.kernel_count() {
        let kernel = table.kernel(ordinal, &mut charge).map_err(codec_error)?;
        identities.push(DescriptorRosterIdentityV1 {
            logical_name: kernel.logical_name(),
            export_name: kernel.entry_name(),
            kernel_binding_id: *kernel.kernel_id().as_bytes(),
        });
    }
    validate_exact_roster_identities(R::ENTRIES, &identities)?;
    let mut entrypoints = Vec::new();
    entrypoints
        .try_reserve_exact(table.kernel_count())
        .map_err(|_| binding("entrypoint storage"))?;
    let manifest = custody.outer_handoff.module_handoff().symbol_manifest();
    for ordinal in 0..table.kernel_count() {
        let kernel = table.kernel(ordinal, &mut charge).map_err(codec_error)?;
        let physical_kernel_index = select_unique_index(
            descriptor
                .bindings
                .inspection()
                .kernels()
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.name() == kernel.entry_name() && p.symbol() == kernel.descriptor_symbol()
                })
                .map(|(i, _)| i),
            AdmissionError::PhysicalKernelNotFound,
            AdmissionError::AmbiguousPhysicalKernel,
        )?;
        if descriptor
            .bindings
            .bindings()
            .get(physical_kernel_index)
            .is_none_or(|b| b.kernel_index() != physical_kernel_index)
        {
            return Err(AdmissionError::DescriptorBindingMismatch);
        }
        if manifest
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .filter(|s| *s == kernel.entry_name())
            .count()
            != 1
            || manifest
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .filter(|s| *s == kernel.descriptor_symbol())
                .count()
                != 1
        {
            return Err(AdmissionError::SelectedExportMismatch);
        }
        let lineage = derive_descriptor_host_lineage_identity(
            &custody.outer_handoff,
            envelope.wire().replay().publication_intent_record(),
            HostDescriptorIdentity {
                digest: descriptor.digest,
                kernel_id: kernel.kernel_id(),
                domain: LINEAGE_DOMAIN,
            },
            &custody.compiler_execution_subject,
            envelope.wire().compiler_execution_receipt(),
            &custody.finalizer_derivation,
        );
        entrypoints.push(RecoveredWorkerV3EntrypointV1 {
            ordinal,
            descriptor_index: ordinal,
            physical_kernel_index,
            lineage,
        });
    }
    let lineage = derive_roster_host_lineage_identity(&entrypoints);
    drop(identities);
    drop(table);
    drop(current);
    Ok(RecoveredMixedWorkerV53PinnedRoster {
        envelope,
        custody,
        descriptor,
        entrypoints,
        lineage,
        #[cfg(target_os = "linux")]
        application_descriptors: None,
        _roster: PhantomData,
    })
}

fn validate_descriptor(
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    current: &DurableCurrentLinkPublicationTokenV1,
    custody: &ReconstructedReplayCustody,
) -> Result<MixedDescriptor> {
    codec_scope(
        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53 + size_of::<NominalDescriptorInspectionV53<'_>>(),
        |budget| validate_descriptor_on_budget(envelope, current, custody, budget),
    )
}

fn validate_descriptor_on_budget(
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    current: &DurableCurrentLinkPublicationTokenV1,
    custody: &ReconstructedReplayCustody,
    budget: &mut Budget<'_>,
) -> Result<MixedDescriptor> {
    if custody.finalizer_derivation.descriptor_schema_version() != 53 {
        return Err(binding("descriptor schema must be V53; no legacy retry"));
    }
    let record = envelope.wire().replay().publication_intent_record();
    let bytes = current.exact_artifact_bytes();
    validate_finalized_publication_identity(record, bytes)?;
    let inspection = inspect_finalized_nominal_hsaco_v53(
        bytes,
        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53,
        &mut |n| budget.charge_work(n),
    )
    .map_err(codec_error)?;
    let raw = derive_unfinalized_nominal_hsaco_on_budget_v53(bytes, budget).map_err(codec_error)?;
    if Sha256::digest(&raw).as_slice() != record.plan().linked_output().as_bytes() {
        return Err(AdmissionError::LinkedIdentityMismatch);
    }
    drop(raw);
    let table = inspection.descriptor_table();
    let physical = inspection.kernel_bindings().inspection();
    let outer = &custody.outer_handoff;
    if production_profile_for_artifact_target(physical.target()).is_none() {
        return Err(AdmissionError::UnsupportedTarget);
    }
    if outer.capsule().target().as_amd_target_id() != physical.target()
        || outer.module_handoff().target().as_amd_target_id() != physical.target()
        || table.device_target().as_amd_target_id() != physical.target()
    {
        return Err(AdmissionError::TargetMismatch);
    }
    if physical.code_object_version() != CodeObjectVersion::V6
        || outer.module_handoff().code_object_version().number() != 6
        || table.code_object_version().number() != 6
    {
        return Err(AdmissionError::CodeObjectVersionMismatch);
    }
    if outer
        .capsule()
        .receipts()
        .export_manifest()
        .canonical_preimage()
        != outer.module_handoff().symbol_manifest().canonical_bytes()
    {
        return Err(AdmissionError::ExportManifestMismatch);
    }
    let source = normalize_descriptor(table.canonical_bytes())?;
    if source != outer.capsule().receipts().abi().canonical_preimage() {
        return Err(AdmissionError::DescriptorSourceMismatch);
    }
    let digest = inspection.digest();
    Ok(MixedDescriptor {
        source,
        bindings: inspection.into_kernel_bindings(),
        digest,
    })
}

fn normalize_descriptor(bytes: &[u8]) -> Result<Vec<u8>> {
    // Only this slot is finalized. All contracts and nominal ABI bytes remain exact.
    let mut source = Vec::new();
    source
        .try_reserve_exact(bytes.len())
        .map_err(|_| binding("descriptor storage"))?;
    source.extend_from_slice(bytes);
    source
        .get_mut(
            CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53..CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53 + 32,
        )
        .ok_or(AdmissionError::DescriptorSourceMismatch)?
        .fill(0);
    Ok(source)
}

fn validate_lineage(
    outer: &InertSemanticCompilerModuleHandoffV3,
    table: &MixedDescriptorTableV53<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    codec_on_budget(
        budget,
        MIXED_MIDDLE_END_WORKING_STORAGE_V50
            + fe2o3_kernel_descriptor::mixed_conditional_v26::MIXED_CONTRACT_CODEC_STORAGE_V26,
        |budget| validate_lineage_on_budget(outer, table, profile, budget),
    )
}

fn validate_lineage_on_budget(
    outer: &InertSemanticCompilerModuleHandoffV3,
    table: &MixedDescriptorTableV53<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let receipts = outer.capsule().receipts();
    let middle = read_mixed_middle_end_v50(
        receipts.middle_end().canonical_preimage(),
        MIXED_MIDDLE_END_WORKING_STORAGE_V50,
        |n| budget.charge_work(n),
    )
    .map_err(codec_error)?;
    middle
        .check_capsule(receipts, |n| budget.charge_work(n))
        .map_err(codec_error)?;
    if receipts.formal_memory().canonical_preimage() != table.canonical_bytes()
        || receipts.abi().canonical_preimage() != table.canonical_bytes()
    {
        return Err(binding("mandatory V53 ABI/formal-memory equality"));
    }
    let proof =
        InertProofBindingAssociationV4::decode(receipts.proof_binding().canonical_preimage())
            .map_err(|_| binding("typed proof association"))?;
    let identity = |sha: &[u8; 32], len| {
        InertLineageContentIdentityV3::new(*sha, len).map_err(|_| binding("proof input identity"))
    };
    let expected = InertProofBindingAssociationInputsV4::new(
        identity(
            receipts.semantic_mir().identity().sha256(),
            receipts.semantic_mir().identity().byte_len(),
        )?,
        identity(
            receipts.middle_end().identity().sha256(),
            receipts.middle_end().identity().byte_len(),
        )?,
        identity(
            receipts.kernel_ir().identity().sha256(),
            receipts.kernel_ir().identity().byte_len(),
        )?,
        identity(
            receipts.mir_to_kir_correspondence().identity().sha256(),
            receipts.mir_to_kir_correspondence().identity().byte_len(),
        )?,
        identity(
            receipts.formal_memory().identity().sha256(),
            receipts.formal_memory().identity().byte_len(),
        )?,
    );
    if proof.inputs() != expected
        || proof.verus_execution_evidence() != middle.input().execution_receipt
    {
        return Err(binding(
            "proof association differs from exact typed execution",
        ));
    }
    let receipt = check_inert_typed_source_receipt_v53(middle.input(), budget)
        .map_err(AdmissionError::MixedReceiptV53)?;
    let graphs = receipt.graph_identities();
    for ordinal in 0..table.kernel_count() {
        let contract = table
            .contract(ordinal, &mut |n| budget.charge_work(n))
            .map_err(codec_error)?;
        let subjects = contract.subjects();
        if subjects.source_semantic_identity != receipt.source_semantic_identity()
            || subjects.original_graph_identity != graphs[0].0
            || subjects.output_graph_identity != graphs[3].0
        {
            return Err(binding("contract differs from exact typed source/graphs"));
        }
    }
    let capsule = outer.capsule();
    let invocation = fe2o3_compiler_lineage::TargetLineageIdentityV3::new(
        *capsule.invocation_digest().as_bytes(),
        u64::try_from(capsule.invocation_canonical_length()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(codec_error)?;
    target_readmission::readmit_target_selection_v53(
        middle.input().forwarded,
        receipts.semantic_mir(),
        invocation,
        table.canonical_bytes(),
        profile,
        receipts.target_binding().canonical_preimage(),
        budget,
        |owner, budget| {
            fe2o3_verifier::check_mixed_native_correspondence_v60(
                owner,
                profile,
                table.canonical_bytes(),
                outer,
                budget,
            )
            .map_err(codec_error)
        },
    )
}

#[cfg(test)]
#[path = "mixed_worker_v53_native_lineage_v60_tests.rs"]
mod native_lineage_tests_v60;

#[cfg(test)]
mod codec_tests_v53 {
    use super::*;

    fn resource(error: &AdmissionError) -> Resource {
        *std::error::Error::source(error)
            .unwrap()
            .downcast_ref::<Resource>()
            .unwrap()
    }

    #[test]
    fn mixed_v53_host_codec_scope_prepays_exact_coexisting_headers_and_preserves_denials() {
        fn inspect(budget: &mut Budget<'_>) -> Result<u32> {
            budget.charge_work(7)?;
            Ok(23)
        }
        type Callback = fn(&mut Budget<'_>) -> Result<u32>;
        let frame = 37
            + size_of::<Callback>()
            + size_of::<Budget<'_>>()
            + 3 * size_of::<usize>()
            + size_of::<u32>()
            + 2 * size_of::<Result<u32>>()
            + size_of::<std::result::Result<Result<u32>, Box<dyn std::any::Any + Send>>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, 19 + frame);
        budget.reserve_storage(19).unwrap();
        assert_eq!(
            codec_on_budget(&mut budget, 37, inspect as Callback).unwrap(),
            23
        );
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (8, 19, 19 + frame)
        );

        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = Budget::new(&mut work, 18 + frame);
        budget.reserve_storage(19).unwrap();
        let first = codec_on_budget(&mut budget, 37, inspect as Callback).unwrap_err();
        assert!(matches!(resource(&first), Resource::Storage(e)
            if e.actual() == 19 + frame && e.limit() == 18 + frame));
        let second = codec_on_budget(&mut budget, 37, inspect as Callback).unwrap_err();
        assert_eq!(resource(&first), resource(&second));
        assert_eq!((budget.work(), budget.storage()), (1, 19));

        let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
        let mut budget = Budget::new(&mut work, 19 + frame);
        budget.reserve_storage(19).unwrap();
        let error = codec_on_budget(&mut budget, 37, inspect as Callback).unwrap_err();
        assert!(matches!(resource(&error), Resource::Work(e)
            if e.actual() == 8 && e.limit() == 7));
        assert_eq!((budget.work(), budget.storage()), (1, 19));
    }

    #[test]
    fn mixed_v53_host_codec_scope_retires_scratch_on_error_and_unwind() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, HOST_CODEC_STORAGE_LIMIT_V53);
        budget.reserve_storage(19).unwrap();
        let error = codec_on_budget::<(), _>(&mut budget, 37, |budget| {
            budget.charge_work(5)?;
            Err(binding("test codec refusal"))
        });
        assert!(matches!(
            error,
            Err(AdmissionError::MixedV53("test codec refusal"))
        ));
        assert_eq!((budget.work(), budget.storage()), (6, 19));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            codec_on_budget::<(), _>(&mut budget, 37, |budget| {
                budget.charge_work(5)?;
                panic!("test codec unwind")
            })
        }));
        assert!(panic.is_err());
        assert_eq!((budget.work(), budget.storage()), (12, 19));
    }

    #[test]
    fn mixed_v53_host_codec_work_is_cumulative_finite_and_overflow_checked() {
        let mut charge = codec_work();
        charge(HOST_CODEC_WORK_LIMIT_V53 - 1).unwrap();
        charge(1).unwrap();
        let error = charge(1).expect_err("no unbounded artifact codec work");
        assert_eq!(error.limit(), HOST_CODEC_WORK_LIMIT_V53);
        assert_eq!(error.actual(), HOST_CODEC_WORK_LIMIT_V53 + 1);
        let error = charge(usize::MAX).expect_err("checked cumulative arithmetic");
        assert_eq!(error.actual(), usize::MAX);
    }

    #[test]
    fn mixed_v53_host_codec_refusal_retains_the_exact_resource_error() {
        fn require_send_sync<T: Send + Sync>() {}
        require_send_sync::<AdmissionError>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
        work.charge_work(2).unwrap();
        let expected = work.charge_work(2).unwrap_err();
        let error = codec_error(expected);
        let actual = std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<CanonicalKernelIrWorkLimitV1>()
            .unwrap();
        assert_eq!(*actual, expected);
        assert_eq!((actual.actual(), actual.limit()), (4, 3));
    }
}
