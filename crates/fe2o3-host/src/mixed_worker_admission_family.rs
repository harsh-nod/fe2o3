// Closed versioned host admission: exact artifacts, roster, receipts and native replay.
use super::*;
use fe2o3_compiler_lineage::{
    InertLineageContentIdentityV3, InertProofBindingAssociationInputsV4,
    InertProofBindingAssociationV4,
};
use fe2o3_hsaco::InspectedKernelBindings;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, CanonicalKernelIrWorkLimitV1};
use std::mem::size_of;

type AdmissionError = RecoveredWorkerV3AdmissionErrorV1;
type Result<T> = std::result::Result<T, AdmissionError>;

// Currentness and handoff traits have no caller verification ledger. Their
// bounded artifact codec domain must still refuse excessive cumulative work.
const HOST_CODEC_WORK_LIMIT: usize = 512 * 1024 * 1024;
const HOST_CODEC_STORAGE_LIMIT: usize = 4 * NOMINAL_DESCRIPTOR_SCRATCH_STORAGE
    + MIXED_DESCRIPTOR_READER_STORAGE
    + MIDDLE_END_WORKING_STORAGE
    + 64 * 1024;
fn codec_work() -> impl FnMut(usize) -> std::result::Result<(), CanonicalKernelIrWorkLimitV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(HOST_CODEC_WORK_LIMIT);
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
    let mut work = CanonicalKernelIrWorkBudgetV1::new(HOST_CODEC_WORK_LIMIT);
    let mut budget = Budget::new(&mut work, HOST_CODEC_STORAGE_LIMIT);
    codec_on_budget(&mut budget, scratch, operation)
}

struct MixedDescriptor {
    source: Vec<u8>,
    bindings: InspectedKernelBindings,
    digest: CanonicalCodeObjectDigest,
}
impl MixedDescriptor {
    fn table(&self) -> Result<MixedDescriptorTable<'_>> {
        codec_scope(MIXED_DESCRIPTOR_READER_STORAGE, |budget| {
            decode_mixed_descriptor(&self.source, &mut |n| budget.charge_work(n))
                .map_err(codec_error)
        })
    }
}

/// Move-only current publication, complete marker roster and typed source association.
///
/// The receipt's strict signature is checked, but its key and claimed runtime
/// are not thereby trusted. A protected measured compiler and native-refinement
/// consumer must authenticate these same owners before any load or dispatch.
/// This value cannot be converted into the unconditional V1 admission.
/// Artifact/codec allocations use the existing bounded readmission domains;
/// `budget` controls the typed-receipt and lineage validation. Other artifact
/// codec traversals have a finite per-operation work limit, not a host RSS claim.
///
pub struct RecoveredMixedWorkerPinnedRoster<R> {
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    custody: ReconstructedReplayCustody,
    descriptor: MixedDescriptor,
    entrypoints: Vec<RecoveredWorkerV3EntrypointV1>,
    lineage: WorkerV3HostLineageEvidenceV1,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<RetainedWorkerV3ApplicationDescriptorsV1>,
    _roster: PhantomData<fn() -> R>,
}
impl<R> fmt::Debug for RecoveredMixedWorkerPinnedRoster<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(ROSTER_DEBUG_NAME)
            .field("published", &self.published())
            .field("lineage", &self.lineage.identity)
            .field("entrypoint_count", &self.entrypoints.len())
            .finish_non_exhaustive()
    }
}
impl<R> RecoveredMixedWorkerPinnedRoster<R> {
    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.envelope.current_publication_lease().published()
    }
    pub fn descriptor_table(&self) -> Result<MixedDescriptorTable<'_>> {
        self.descriptor.table()
    }
    pub fn entrypoints(&self) -> &[RecoveredWorkerV3EntrypointV1] {
        &self.entrypoints
    }
    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity
    }
    pub fn typed_receipt(&self, budget: &mut Budget<'_>) -> Result<TypedSourceReceipt<'_>> {
        codec_on_budget(budget, MIDDLE_END_WORKING_STORAGE, |budget| {
            let receipts = self.custody.outer_handoff.capsule().receipts();
            let middle = read_middle_end(
                receipts.middle_end().canonical_preimage(),
                MIDDLE_END_WORKING_STORAGE,
                |n| budget.charge_work(n),
            )
            .map_err(codec_error)?;
            check_typed_source_receipt(middle.input(), budget).map_err(typed_receipt_error)
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
        receive: impl for<'a> FnOnce(MixedWorkerVerificationRequest<'a, R>) -> std::result::Result<T, E>,
    ) -> Result<std::result::Result<T, E>> {
        let current = self
            .envelope
            .current_publication_lease()
            .acquire_current_token()
            .map_err(AdmissionError::CurrentPublication)?;
        self.check_current(&current)?;
        let result = receive(MixedWorkerVerificationRequest {
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
pub struct MixedWorkerVerificationRequest<'a, R> {
    owner: &'a RecoveredMixedWorkerPinnedRoster<R>,
    current: &'a DurableCurrentLinkPublicationTokenV1,
}
impl<'a, R> MixedWorkerVerificationRequest<'a, R> {
    pub fn admission(&self) -> &'a RecoveredMixedWorkerPinnedRoster<R> {
        self.owner
    }
    pub fn finalized_hsaco_bytes(&self) -> &'a [u8] {
        self.current.exact_artifact_bytes()
    }
    pub fn outer_handoff(&self) -> &'a InertSemanticCompilerModuleHandoffV3 {
        &self.owner.custody.outer_handoff
    }
    pub fn middle_end(&self) -> Result<MiddleEndRef<'a>> {
        codec_scope(MIDDLE_END_WORKING_STORAGE, |budget| {
            read_middle_end(
                self.outer_handoff()
                    .capsule()
                    .receipts()
                    .middle_end()
                    .canonical_preimage(),
                MIDDLE_END_WORKING_STORAGE,
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
pub fn admit_recovered_mixed_worker_roster<R: CompilerGeneratedKernelExpectationRosterV1>(
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    budget: &mut Budget<'_>,
) -> Result<RecoveredMixedWorkerPinnedRoster<R>> {
    let (custody, current) = reconstruct_replay_custody(&envelope)?;
    let descriptor = validate_descriptor(&envelope, &current, &custody)?;
    let table = descriptor.table()?;
    // validate_descriptor already checked the complete physical target/features.
    let profile = ProductionAmdTargetProfileV1::from_cpu(
        descriptor.bindings.inspection().target().processor(),
    )
    .ok_or_else(|| binding(TARGET_REFUSAL))?;
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
    Ok(RecoveredMixedWorkerPinnedRoster {
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
        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE + size_of::<NominalDescriptorInspection<'_>>(),
        |budget| validate_descriptor_on_budget(envelope, current, custody, budget),
    )
}

fn validate_descriptor_on_budget(
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    current: &DurableCurrentLinkPublicationTokenV1,
    custody: &ReconstructedReplayCustody,
    budget: &mut Budget<'_>,
) -> Result<MixedDescriptor> {
    if custody.finalizer_derivation.descriptor_schema_version() != DESCRIPTOR_SCHEMA {
        return Err(binding(SCHEMA_REFUSAL));
    }
    let record = envelope.wire().replay().publication_intent_record();
    let bytes = current.exact_artifact_bytes();
    validate_finalized_publication_identity(record, bytes)?;
    let inspection =
        inspect_finalized_nominal_hsaco(bytes, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE, &mut |n| {
            budget.charge_work(n)
        })
        .map_err(codec_error)?;
    let raw = derive_unfinalized_nominal_hsaco_on_budget(bytes, budget).map_err(codec_error)?;
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
        .get_mut(CANONICAL_CODE_OBJECT_DIGEST_OFFSET..CANONICAL_CODE_OBJECT_DIGEST_OFFSET + 32)
        .ok_or(AdmissionError::DescriptorSourceMismatch)?
        .fill(0);
    Ok(source)
}

fn validate_lineage(
    outer: &InertSemanticCompilerModuleHandoffV3,
    table: &MixedDescriptorTable<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    codec_on_budget(
        budget,
        MIDDLE_END_WORKING_STORAGE + CONTRACT_CODEC_STORAGE,
        |budget| validate_lineage_on_budget(outer, table, profile, budget),
    )
}

fn validate_lineage_on_budget(
    outer: &InertSemanticCompilerModuleHandoffV3,
    table: &MixedDescriptorTable<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    with_validated_lineage_on_budget(outer, table, profile, budget, |_, _| Ok(()))
}

fn with_validated_lineage_on_budget<T>(
    outer: &InertSemanticCompilerModuleHandoffV3,
    table: &MixedDescriptorTable<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
    receive: impl FnOnce(
        &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> Result<T> {
    let receipts = outer.capsule().receipts();
    let middle = read_middle_end(
        receipts.middle_end().canonical_preimage(),
        MIDDLE_END_WORKING_STORAGE,
        |n| budget.charge_work(n),
    )
    .map_err(codec_error)?;
    middle
        .check_capsule(receipts, |n| budget.charge_work(n))
        .map_err(codec_error)?;
    if receipts.formal_memory().canonical_preimage() != table.canonical_bytes()
        || receipts.abi().canonical_preimage() != table.canonical_bytes()
    {
        return Err(binding(ABI_REFUSAL));
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
    let receipt =
        check_typed_source_receipt(middle.input(), budget).map_err(typed_receipt_error)?;
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
    target_readmission::readmit_target_selection(
        middle.input().forwarded,
        receipts.semantic_mir(),
        invocation,
        table.canonical_bytes(),
        profile,
        receipts.target_binding().canonical_preimage(),
        budget,
        |owner, budget| {
            check_native_correspondence(owner, profile, table.canonical_bytes(), outer, budget)
                .map_err(codec_error)?;
            receive(owner, budget)
        },
    )
}
