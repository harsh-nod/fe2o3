//! Nominal readmission on the existing receipt-bearing Worker V3 custody path.
use super::*;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3, CompilerDescriptorSourceErrorV3,
    CompilerDescriptorSourceV3, compiler_descriptor_source_validation_storage_v3,
};
use fe2o3_hsaco::InspectedKernelBindings;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3, NominalFinalizationErrorV3,
    derive_unfinalized_nominal_hsaco_v3, inspect_finalized_nominal_hsaco_v3,
};
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3, CanonicalCodeObjectDigest, DescriptorWireErrorV3,
    DeviceDescriptorTableV3,
};
use std::convert::Infallible;

pub(super) const LINEAGE_DOMAIN: &[u8] = b"fe2o3.host.worker-v3-nominal-descriptor-lineage.v3\0";
type CommonError = RecoveredWorkerV3AdmissionErrorV1;
type Result<T> = std::result::Result<T, RecoveredNominalWorkerV3AdmissionError>;

#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredNominalWorkerV3AdmissionError {
    Custody(CommonError),
    Artifact(NominalFinalizationErrorV3<Infallible>),
    Source(CompilerDescriptorSourceErrorV3<Infallible>),
    Descriptor(DescriptorWireErrorV3<Infallible>),
    DescriptorSchema,
    Storage,
}
impl fmt::Display for RecoveredNominalWorkerV3AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custody(e) => e.fmt(f),
            Self::Artifact(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Descriptor(e) => e.fmt(f),
            Self::DescriptorSchema => {
                f.write_str("nominal host admission requires descriptor schema V3")
            }
            Self::Storage => {
                f.write_str("nominal host admission storage allocation or extent failed")
            }
        }
    }
}
impl Error for RecoveredNominalWorkerV3AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Custody(e) => Some(e),
            Self::Artifact(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            _ => None,
        }
    }
}
impl From<CommonError> for RecoveredNominalWorkerV3AdmissionError {
    fn from(e: CommonError) -> Self {
        Self::Custody(e)
    }
}

fn bounded_codec_work(_: usize) -> std::result::Result<(), Infallible> {
    Ok(())
}

struct NominalDescriptor {
    source: CompilerDescriptorSourceV3,
    bindings: InspectedKernelBindings,
    digest: CanonicalCodeObjectDigest,
}
impl NominalDescriptor {
    fn table(&self) -> Result<DeviceDescriptorTableV3<'_>> {
        let storage = self
            .source
            .storage()
            .retained_storage()
            .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
            .ok_or(RecoveredNominalWorkerV3AdmissionError::Storage)?;
        self.source
            .table(storage, &mut bounded_codec_work)
            .map_err(RecoveredNominalWorkerV3AdmissionError::Source)
    }
}

/// Exact, move-only nominal roster with receipt-bearing replay and publication lease.
/// Queries borrow the independently checked zero-digest compiler descriptor source.
/// No V1 descriptor projection or native proof/load/launch promotion is provided.
/// Like existing host readmission, this uses bounded artifact/codec allocation
/// domains, not a complete caller-owned work/storage ledger.
///
/// ```compile_fail
/// use fe2o3_host::RecoveredNominalWorkerV3PinnedRoster;
/// fn duplicate<R>(value: RecoveredNominalWorkerV3PinnedRoster<R>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::RecoveredNominalWorkerV3PinnedRoster;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
/// fn escape<R>(value: RecoveredNominalWorkerV3PinnedRoster<R>) -> DeviceDescriptorTableV3<'static> {
///     value.descriptor_table().unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_host::RecoveredNominalWorkerV3PinnedRoster;
/// fn load<R>(value: RecoveredNominalWorkerV3PinnedRoster<R>) { value.load(); }
/// ```
pub struct RecoveredNominalWorkerV3PinnedRoster<R> {
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    custody: ReconstructedReplayCustody,
    descriptor: NominalDescriptor,
    entrypoints: Vec<RecoveredWorkerV3EntrypointV1>,
    lineage: WorkerV3HostLineageEvidenceV1,
    _roster: PhantomData<fn() -> R>,
}
impl<R> fmt::Debug for RecoveredNominalWorkerV3PinnedRoster<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveredNominalWorkerV3PinnedRoster")
            .field("published", &self.published())
            .field("lineage", &self.lineage.identity)
            .field("entrypoint_count", &self.entrypoints.len())
            .finish_non_exhaustive()
    }
}
impl<R> RecoveredNominalWorkerV3PinnedRoster<R> {
    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.envelope.current_publication_lease().published()
    }
    pub fn descriptor_table(&self) -> Result<DeviceDescriptorTableV3<'_>> {
        self.descriptor.table()
    }
    pub fn entrypoints(&self) -> &[RecoveredWorkerV3EntrypointV1] {
        &self.entrypoints
    }
    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity
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
    pub const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.custody.compiler_execution_subject
    }
    pub const fn compiler_execution_receipt(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.envelope.wire().compiler_execution_receipt()
    }
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.custody.finalizer_derivation
    }
    pub fn revalidate_currentness(&self) -> Result<()> {
        let current = self
            .envelope
            .current_publication_lease()
            .acquire_current_token()
            .map_err(CommonError::CurrentPublication)?;
        validate_retained_replay_custody(
            &self.envelope,
            &current,
            &self.custody.finalizer_derivation,
            &self.custody.outer_handoff,
            &self.custody.compiler_execution_subject,
        )?;
        let actual = validate_nominal_descriptor(&self.envelope, &current, &self.custody)?;
        if actual.source.canonical_bytes() != self.descriptor.source.canonical_bytes()
            || actual.bindings != self.descriptor.bindings
            || actual.digest != self.descriptor.digest
        {
            return Err(CommonError::InspectionChanged.into());
        }
        Ok(())
    }
    /// Exact ABI receipt association only, not authenticated rustc source semantics.
    pub const fn authenticates_descriptor_source(&self) -> bool {
        true
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

/// Independently replays one current nominal artifact and matches the complete
/// generated roster, in descriptor order, before returning inert host metadata.
pub fn admit_recovered_nominal_worker_v3_roster<R: CompilerGeneratedKernelExpectationRosterV1>(
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<RecoveredNominalWorkerV3PinnedRoster<R>> {
    let (custody, current) = reconstruct_replay_custody(&envelope)?;
    let descriptor = validate_nominal_descriptor(&envelope, &current, &custody)?;
    let table = descriptor.table()?;
    let mut identities = Vec::new();
    identities
        .try_reserve_exact(table.kernel_count())
        .map_err(|_| RecoveredNominalWorkerV3AdmissionError::Storage)?;
    for ordinal in 0..table.kernel_count() {
        let kernel = table
            .kernel(ordinal, &mut bounded_codec_work)
            .map_err(RecoveredNominalWorkerV3AdmissionError::Descriptor)?;
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
        .map_err(|_| RecoveredNominalWorkerV3AdmissionError::Storage)?;
    for ordinal in 0..table.kernel_count() {
        let kernel = table
            .kernel(ordinal, &mut bounded_codec_work)
            .map_err(RecoveredNominalWorkerV3AdmissionError::Descriptor)?;
        let physical_kernel_index = select_unique_index(
            descriptor
                .bindings
                .inspection()
                .kernels()
                .iter()
                .enumerate()
                .filter(|(_, physical)| {
                    physical.name() == kernel.entry_name()
                        && physical.symbol() == kernel.descriptor_symbol()
                })
                .map(|(index, _)| index),
            CommonError::PhysicalKernelNotFound,
            CommonError::AmbiguousPhysicalKernel,
        )?;
        if descriptor
            .bindings
            .bindings()
            .get(physical_kernel_index)
            .is_none_or(|binding| binding.kernel_index() != physical_kernel_index)
        {
            return Err(CommonError::DescriptorBindingMismatch.into());
        }
        let manifest = custody.outer_handoff.module_handoff().symbol_manifest();
        if manifest
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .filter(|symbol| *symbol == kernel.entry_name())
            .count()
            != 1
            || manifest
                .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
                .filter(|symbol| *symbol == kernel.descriptor_symbol())
                .count()
                != 1
        {
            return Err(CommonError::SelectedExportMismatch.into());
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
    drop(table);
    drop(current);
    Ok(RecoveredNominalWorkerV3PinnedRoster {
        envelope,
        custody,
        descriptor,
        entrypoints,
        lineage,
        _roster: PhantomData,
    })
}

fn validate_nominal_descriptor(
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    current: &DurableCurrentLinkPublicationTokenV1,
    custody: &ReconstructedReplayCustody,
) -> Result<NominalDescriptor> {
    if custody.finalizer_derivation.descriptor_schema_version() != 3 {
        return Err(RecoveredNominalWorkerV3AdmissionError::DescriptorSchema);
    }
    let record = envelope.wire().replay().publication_intent_record();
    let bytes = current.exact_artifact_bytes();
    validate_finalized_publication_identity(record, bytes)?;
    let inspection = inspect_finalized_nominal_hsaco_v3(
        bytes,
        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
        &mut bounded_codec_work,
    )
    .map_err(RecoveredNominalWorkerV3AdmissionError::Artifact)?;
    let raw = derive_unfinalized_nominal_hsaco_v3(
        bytes,
        NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
        &mut bounded_codec_work,
    )
    .map_err(RecoveredNominalWorkerV3AdmissionError::Artifact)?;
    if Sha256::digest(&raw).as_slice() != record.plan().linked_output().as_bytes() {
        return Err(CommonError::LinkedIdentityMismatch.into());
    }
    drop(raw);
    let table = inspection.descriptor_table();
    let physical = inspection.kernel_bindings().inspection();
    let outer = &custody.outer_handoff;
    if production_profile_for_artifact_target(physical.target()).is_none() {
        return Err(CommonError::UnsupportedTarget.into());
    }
    if outer.capsule().target().as_amd_target_id() != physical.target()
        || outer.module_handoff().target().as_amd_target_id() != physical.target()
        || table.device_target().as_amd_target_id() != physical.target()
    {
        return Err(CommonError::TargetMismatch.into());
    }
    if physical.code_object_version() != CodeObjectVersion::V6
        || outer.module_handoff().code_object_version().number() != 6
        || table.code_object_version().number() != 6
    {
        return Err(CommonError::CodeObjectVersionMismatch.into());
    }
    if outer
        .capsule()
        .receipts()
        .export_manifest()
        .canonical_preimage()
        != outer.module_handoff().symbol_manifest().canonical_bytes()
    {
        return Err(CommonError::ExportManifestMismatch.into());
    }
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(table.canonical_bytes().len())
        .map_err(|_| RecoveredNominalWorkerV3AdmissionError::Storage)?;
    normalized.extend_from_slice(table.canonical_bytes());
    normalized
        .get_mut(
            CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3..CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3 + 32,
        )
        .ok_or(CommonError::DescriptorSourceMismatch)?
        .fill(0);
    if normalized != outer.capsule().receipts().abi().canonical_preimage() {
        return Err(CommonError::DescriptorSourceMismatch.into());
    }
    let storage = compiler_descriptor_source_validation_storage_v3(normalized.capacity())
        .ok_or(RecoveredNominalWorkerV3AdmissionError::Storage)?;
    let source = CompilerDescriptorSourceV3::from_owned_canonical_bytes(
        normalized,
        storage,
        &mut bounded_codec_work,
    )
    .map_err(RecoveredNominalWorkerV3AdmissionError::Source)?;
    let digest = inspection.digest();
    Ok(NominalDescriptor {
        source,
        bindings: inspection.into_kernel_bindings(),
        digest,
    })
}
