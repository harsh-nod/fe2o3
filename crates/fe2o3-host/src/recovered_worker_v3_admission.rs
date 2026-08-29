use std::{error::Error, fmt, marker::PhantomData};

use fe2o3_amd_target::{AmdTargetId, ProductionAmdTargetProfileV1};
use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationTokenV1, DurableLinkPublicationError,
    InertCompilerExecutionSubjectV1, PublishedLinkArtifactV1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1, CompilerModuleSymbolRoleV1,
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco::{CodeObjectVersion, InspectedKernel, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::{
    FinalizationError, FinalizedDescriptorInspection,
    RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3HsacoPublicationErrorV1,
    derive_unfinalized_hsaco_from_finalized_v1,
    revalidate_protected_worker_v3_finalizer_derivation_v1, verify_finalized,
};
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET, DeviceDescriptorTableV1, KernelDescriptorV1, KernelId,
    encode_device_descriptor_table_v1,
};
use fe2o3_runtime_protocol::{
    CompilerExecutionReceiptCarriageV1, RecoveredWorkerV3LoadEnvelopeV2,
    WorkerV3LoadEnvelopeErrorV2,
};
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
use crate::application_descriptor_handoff::RetainedWorkerV3ApplicationDescriptorsV1;
use crate::{
    CompilerGeneratedKernelExpectationRosterEntryV1, CompilerGeneratedKernelExpectationRosterV1,
};

const WORKER_V3_HOST_LINEAGE_DOMAIN_V1: &[u8] = b"fe2o3.host.worker-v3-lineage.v1\0";
const WORKER_V3_HOST_ROSTER_LINEAGE_DOMAIN_V1: &[u8] = b"fe2o3.host.worker-v3-roster-lineage.v1\0";

/// Canonical identity of every V3 compiler, publication, descriptor, and selected-kernel axis
/// independently retained by host admission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3HostLineageIdentityV1([u8; 32]);

impl WorkerV3HostLineageIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerV3HostLineageEvidenceV1 {
    identity: WorkerV3HostLineageIdentityV1,
    finalizer_derivation_sha256: [u8; 32],
    capsule_sha256: [u8; 32],
    formal_memory_sha256: [u8; 32],
    proof_binding_sha256: [u8; 32],
    finalized_sha256: [u8; 32],
    finalized_length: u64,
}

impl WorkerV3HostLineageEvidenceV1 {
    pub(crate) const fn identity(self) -> WorkerV3HostLineageIdentityV1 {
        self.identity
    }

    pub(crate) const fn finalizer_derivation_sha256(self) -> [u8; 32] {
        self.finalizer_derivation_sha256
    }

    pub(crate) const fn capsule_sha256(self) -> [u8; 32] {
        self.capsule_sha256
    }

    pub(crate) const fn formal_memory_sha256(self) -> [u8; 32] {
        self.formal_memory_sha256
    }

    pub(crate) const fn proof_binding_sha256(self) -> [u8; 32] {
        self.proof_binding_sha256
    }

    pub(crate) const fn finalized_sha256(self) -> [u8; 32] {
        self.finalized_sha256
    }

    pub(crate) const fn finalized_length(self) -> u64 {
        self.finalized_length
    }
}

struct RecoveredWorkerV3ArtifactStateV1 {
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution_subject: InertCompilerExecutionSubjectV1,
    outer_handoff: InertSemanticCompilerModuleHandoffV3,
    inspection: FinalizedDescriptorInspection,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<RetainedWorkerV3ApplicationDescriptorsV1>,
}

impl RecoveredWorkerV3ArtifactStateV1 {
    fn acquire_retained_currentness_token(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        let current = self
            .envelope
            .current_publication_lease()
            .acquire_current_token()
            .map_err(RecoveredWorkerV3AdmissionErrorV1::CurrentPublication)?;
        self.revalidate_retained_currentness_token(&current)?;
        Ok(current)
    }

    fn revalidate_retained_currentness_token(
        &self,
        current: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.envelope
            .current_publication_lease()
            .validate_current_token(current)
            .and_then(|()| current.revalidate_locked_currentness())
            .map_err(RecoveredWorkerV3AdmissionErrorV1::CurrentPublication)?;
        self.envelope
            .wire()
            .validate_reacquired_publication_lease_v2(self.envelope.current_publication_lease())
            .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
        let inspected = validate_finalized_identity(
            self.envelope.wire().replay().publication_intent_record(),
            current.exact_artifact_bytes(),
        )?;
        if inspected != self.inspection {
            return Err(RecoveredWorkerV3AdmissionErrorV1::InspectionChanged);
        }
        validate_finalizer_derivation_association(
            &self.envelope,
            current.exact_artifact_bytes(),
            &self.finalizer_derivation,
        )?;
        let outer_handoff = InertSemanticCompilerModuleHandoffV3::decode(
            self.envelope.wire().replay().outer_handoff(),
        )
        .map_err(RecoveredWorkerV3AdmissionErrorV1::OuterHandoff)?;
        if outer_handoff != self.outer_handoff {
            return Err(RecoveredWorkerV3AdmissionErrorV1::CompilerHandoffChanged);
        }
        let compiler_execution_subject = self
            .envelope
            .wire()
            .reconstructed_compiler_execution_subject_v1()
            .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
        if compiler_execution_subject != self.compiler_execution_subject
            || self
                .envelope
                .wire()
                .compiler_execution_receipt()
                .request()
                .subject()
                != &compiler_execution_subject
        {
            return Err(RecoveredWorkerV3AdmissionErrorV1::CompilerExecutionSubjectChanged);
        }
        #[cfg(target_os = "linux")]
        if let Some(descriptors) = &self.application_descriptors {
            descriptors
                .revalidate()
                .map_err(|_| RecoveredWorkerV3AdmissionErrorV1::ApplicationDescriptorsChanged)?;
        }
        Ok(())
    }

    fn published(&self) -> PublishedLinkArtifactV1 {
        self.envelope.current_publication_lease().published()
    }

    fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.inspection.descriptor_table()
    }

    const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        &self.outer_handoff
    }

    const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.compiler_execution_subject
    }

    const fn compiler_execution_receipt(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.envelope.wire().compiler_execution_receipt()
    }

    const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.finalizer_derivation
    }

    const fn finalizer_replay(&self) -> &fe2o3_runtime_protocol::WorkerV3LoadEnvelopeWireV1 {
        self.envelope.wire().replay()
    }

    fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.inspection.hsaco().target()
    }

    fn code_object_version(&self) -> CodeObjectVersion {
        self.inspection.hsaco().code_object_version()
    }
}

/// One inert, exactly selected entrypoint within a recovered Worker V3 artifact.
///
/// Entrypoints are move-only metadata. The containing admission retains the sole
/// recovered envelope and current-publication lease.
pub struct RecoveredWorkerV3EntrypointV1 {
    ordinal: usize,
    descriptor_index: usize,
    physical_kernel_index: usize,
    lineage: WorkerV3HostLineageEvidenceV1,
}

impl RecoveredWorkerV3EntrypointV1 {
    /// Position in the canonical descriptor table, independent of physical ELF order.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity
    }
}

impl fmt::Debug for RecoveredWorkerV3EntrypointV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV3EntrypointV1")
            .field("ordinal", &self.ordinal)
            .field("lineage", &self.lineage.identity)
            .finish_non_exhaustive()
    }
}

/// Read-only admission of one exact compiler-generated kernel roster in
/// canonical descriptor-table order.
///
/// The value owns one recovered envelope for the complete descriptor table. Its
/// entrypoints are inert and grant no verification, load, or launch authority.
pub struct RecoveredWorkerV3PinnedRosterV1<R> {
    artifact: RecoveredWorkerV3ArtifactStateV1,
    entrypoints: Vec<RecoveredWorkerV3EntrypointV1>,
    lineage: WorkerV3HostLineageEvidenceV1,
    _roster: PhantomData<fn() -> R>,
}

impl<R> fmt::Debug for RecoveredWorkerV3PinnedRosterV1<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV3PinnedRosterV1")
            .field("published", &self.published())
            .field("entrypoint_count", &self.entrypoints.len())
            .field("lineage", &self.lineage.identity)
            .field("target", &self.target())
            .field("code_object_version", &self.code_object_version())
            .finish_non_exhaustive()
    }
}

impl<R> RecoveredWorkerV3PinnedRosterV1<R> {
    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        let current = self.artifact.acquire_retained_currentness_token()?;
        drop(current);
        Ok(())
    }

    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.artifact.published()
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity
    }

    pub fn entrypoints(&self) -> &[RecoveredWorkerV3EntrypointV1] {
        &self.entrypoints
    }

    pub fn descriptor(&self, ordinal: usize) -> Option<&KernelDescriptorV1> {
        let entrypoint = self.entrypoints.get(ordinal)?;
        self.artifact
            .inspection
            .descriptor_table()
            .kernels()
            .get(entrypoint.descriptor_index)
    }

    pub fn physical_kernel(&self, ordinal: usize) -> Option<&InspectedKernel> {
        let entrypoint = self.entrypoints.get(ordinal)?;
        self.artifact
            .inspection
            .hsaco()
            .kernels()
            .get(entrypoint.physical_kernel_index)
    }

    pub fn descriptor_binding(&self, ordinal: usize) -> Option<KernelDescriptorBinding> {
        let entrypoint = self.entrypoints.get(ordinal)?;
        self.artifact
            .inspection
            .kernel_bindings()
            .bindings()
            .get(entrypoint.physical_kernel_index)
            .copied()
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.artifact.target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.artifact.code_object_version()
    }

    pub(crate) fn acquire_retained_currentness_token(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        self.artifact.acquire_retained_currentness_token()
    }

    pub(crate) fn revalidate_retained_currentness_token(
        &self,
        current: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.artifact.revalidate_retained_currentness_token(current)
    }

    pub(crate) fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.artifact.descriptor_table()
    }

    pub(crate) const fn lineage_evidence(&self) -> WorkerV3HostLineageEvidenceV1 {
        self.lineage
    }

    pub(crate) const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.artifact.outer_handoff()
    }

    pub(crate) const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        self.artifact.compiler_execution_subject()
    }

    pub(crate) const fn compiler_execution_receipt(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.artifact.compiler_execution_receipt()
    }

    /// Exact descriptor-source association was independently checked during construction.
    /// This does not authenticate compiler process origin or formal verification authority.
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

/// Read-only host admission for one selected entrypoint in a recovered Worker V3 publication.
///
/// This source-compatible single-kernel view owns the same common artifact state
/// used by roster admission and still grants no load or launch authority.
pub struct RecoveredWorkerV3PinnedDescriptorV1 {
    artifact: RecoveredWorkerV3ArtifactStateV1,
    entrypoint: RecoveredWorkerV3EntrypointV1,
}

impl fmt::Debug for RecoveredWorkerV3PinnedDescriptorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV3PinnedDescriptorV1")
            .field("published", &self.published())
            .field("descriptor", self.descriptor())
            .field("target", &self.target())
            .field("code_object_version", &self.code_object_version())
            .field("lineage", &self.entrypoint.lineage.identity)
            .finish_non_exhaustive()
    }
}

impl RecoveredWorkerV3PinnedDescriptorV1 {
    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        let current = self.acquire_retained_currentness_token()?;
        drop(current);
        Ok(())
    }

    pub(crate) fn acquire_retained_currentness_token(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        self.artifact.acquire_retained_currentness_token()
    }

    pub(crate) fn revalidate_retained_currentness_token(
        &self,
        current: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.artifact.revalidate_retained_currentness_token(current)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn retain_application_descriptors(
        mut self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        debug_assert!(self.artifact.application_descriptors.is_none());
        self.artifact.application_descriptors = Some(descriptors);
        self
    }

    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.artifact.published()
    }

    pub fn descriptor(&self) -> &KernelDescriptorV1 {
        &self.artifact.inspection.descriptor_table().kernels()[self.entrypoint.descriptor_index]
    }

    pub(crate) fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.artifact.descriptor_table()
    }

    pub fn physical_kernel(&self) -> &InspectedKernel {
        &self.artifact.inspection.hsaco().kernels()[self.entrypoint.physical_kernel_index]
    }

    pub fn descriptor_binding(&self) -> KernelDescriptorBinding {
        self.artifact.inspection.kernel_bindings().bindings()[self.entrypoint.physical_kernel_index]
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.entrypoint.lineage.identity
    }

    pub(crate) const fn lineage_evidence(&self) -> WorkerV3HostLineageEvidenceV1 {
        self.entrypoint.lineage
    }

    pub(crate) const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.artifact.outer_handoff()
    }

    pub(crate) const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        self.artifact.compiler_execution_subject()
    }

    pub(crate) const fn compiler_execution_receipt(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.artifact.compiler_execution_receipt()
    }

    pub(crate) const fn finalizer_derivation(
        &self,
    ) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        self.artifact.finalizer_derivation()
    }

    pub(crate) const fn finalizer_replay(
        &self,
    ) -> &fe2o3_runtime_protocol::WorkerV3LoadEnvelopeWireV1 {
        self.artifact.finalizer_replay()
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.artifact.target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.artifact.code_object_version()
    }

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

/// Consumes recovered Worker V3 custody into one independently checked inert host descriptor.
pub fn admit_recovered_worker_v3_descriptor_v1(
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
    kernel_id: KernelId,
) -> Result<RecoveredWorkerV3PinnedDescriptorV1, RecoveredWorkerV3AdmissionErrorV1> {
    let (artifact, current) = admit_recovered_worker_v3_artifact_v1(envelope)?;
    let entrypoint = select_entrypoint(&artifact, kernel_id)?;
    drop(current);

    Ok(RecoveredWorkerV3PinnedDescriptorV1 {
        artifact,
        entrypoint,
    })
}

/// Consumes one recovered envelope into an exact, inert marker roster in
/// canonical descriptor-table order.
///
/// This transition matches every marker's logical name, export name, and
/// binding identity to the complete receipt-bound compiler descriptor table.
/// V1 descriptor tables are currently ordered by `KernelId`; source registration
/// and physical ELF kernel order are separate axes.
/// Generated host-contract identities remain inert until a later sealed
/// verification transition.
pub fn admit_recovered_worker_v3_roster_v1<R>(
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<RecoveredWorkerV3PinnedRosterV1<R>, RecoveredWorkerV3AdmissionErrorV1>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    let (artifact, current) = admit_recovered_worker_v3_artifact_v1(envelope)?;
    validate_exact_roster(R::ENTRIES, artifact.descriptor_table().kernels())?;
    let mut entrypoints = Vec::with_capacity(R::ENTRIES.len());
    for (ordinal, expected) in R::ENTRIES.iter().enumerate() {
        let entrypoint = select_entrypoint(
            &artifact,
            KernelId::from_bytes(expected.kernel_binding_id()),
        )?;
        if entrypoint.descriptor_index != ordinal {
            return Err(RecoveredWorkerV3AdmissionErrorV1::RosterEntryReordered {
                expected_ordinal: ordinal,
                actual_ordinal: entrypoint.descriptor_index,
            });
        }
        entrypoints.push(entrypoint);
    }
    let lineage = derive_roster_host_lineage_identity(entrypoints.as_slice());
    drop(current);
    Ok(RecoveredWorkerV3PinnedRosterV1 {
        artifact,
        entrypoints,
        lineage,
        _roster: PhantomData,
    })
}

fn derive_roster_host_lineage_identity(
    entrypoints: &[RecoveredWorkerV3EntrypointV1],
) -> WorkerV3HostLineageEvidenceV1 {
    let first = entrypoints
        .first()
        .expect("exact roster admission rejects an empty roster")
        .lineage;
    debug_assert!(entrypoints.iter().all(|entrypoint| {
        entrypoint.lineage.capsule_sha256 == first.capsule_sha256
            && entrypoint.lineage.formal_memory_sha256 == first.formal_memory_sha256
            && entrypoint.lineage.proof_binding_sha256 == first.proof_binding_sha256
            && entrypoint.lineage.finalized_sha256 == first.finalized_sha256
            && entrypoint.lineage.finalized_length == first.finalized_length
    }));

    let mut digest = Sha256::new();
    digest.update(WORKER_V3_HOST_ROSTER_LINEAGE_DOMAIN_V1);
    digest.update(
        u64::try_from(entrypoints.len())
            .expect("admitted roster length fits u64")
            .to_le_bytes(),
    );
    for entrypoint in entrypoints {
        digest.update(entrypoint.lineage.identity.as_bytes());
    }
    WorkerV3HostLineageEvidenceV1 {
        identity: WorkerV3HostLineageIdentityV1(digest.finalize().into()),
        ..first
    }
}

fn admit_recovered_worker_v3_artifact_v1(
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<
    (
        RecoveredWorkerV3ArtifactStateV1,
        DurableCurrentLinkPublicationTokenV1,
    ),
    RecoveredWorkerV3AdmissionErrorV1,
> {
    envelope
        .wire()
        .validate_reacquired_publication_lease_v2(envelope.current_publication_lease())
        .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
    let current = envelope
        .current_publication_lease()
        .acquire_current_token()
        .map_err(RecoveredWorkerV3AdmissionErrorV1::CurrentPublication)?;
    current
        .revalidate_locked_currentness()
        .map_err(RecoveredWorkerV3AdmissionErrorV1::CurrentPublication)?;

    let finalizer_derivation = revalidate_protected_worker_v3_finalizer_derivation_v1(
        envelope
            .wire()
            .replay()
            .publication_intent_record()
            .attempt(),
        envelope.wire().replay().outer_handoff(),
        envelope.wire().replay().external_provider_payloads(),
        envelope.wire().replay().transcript(),
        current.exact_artifact_bytes(),
    )
    .map_err(RecoveredWorkerV3AdmissionErrorV1::FinalizerDerivation)?;
    validate_finalizer_derivation_association(
        &envelope,
        current.exact_artifact_bytes(),
        &finalizer_derivation,
    )?;

    let inspection = validate_finalized_identity(
        envelope.wire().replay().publication_intent_record(),
        current.exact_artifact_bytes(),
    )?;
    let outer =
        InertSemanticCompilerModuleHandoffV3::decode(envelope.wire().replay().outer_handoff())
            .map_err(RecoveredWorkerV3AdmissionErrorV1::OuterHandoff)?;
    let compiler_execution_subject = envelope
        .wire()
        .reconstructed_compiler_execution_subject_v1()
        .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
    validate_compiler_source_and_exports(&outer, &inspection)?;
    validate_target_and_code_object(&outer, &inspection)?;

    Ok((
        RecoveredWorkerV3ArtifactStateV1 {
            envelope,
            finalizer_derivation,
            compiler_execution_subject,
            outer_handoff: outer,
            inspection,
            #[cfg(target_os = "linux")]
            application_descriptors: None,
        },
        current,
    ))
}

fn select_entrypoint(
    artifact: &RecoveredWorkerV3ArtifactStateV1,
    kernel_id: KernelId,
) -> Result<RecoveredWorkerV3EntrypointV1, RecoveredWorkerV3AdmissionErrorV1> {
    let (descriptor_index, physical_kernel_index) =
        select_exact_kernel(&artifact.outer_handoff, &artifact.inspection, kernel_id)?;
    let lineage = derive_host_lineage_identity(
        &artifact.outer_handoff,
        artifact
            .envelope
            .wire()
            .replay()
            .publication_intent_record(),
        &artifact.inspection,
        kernel_id,
        &artifact.compiler_execution_subject,
        artifact.envelope.wire().compiler_execution_receipt(),
        &artifact.finalizer_derivation,
    );
    Ok(RecoveredWorkerV3EntrypointV1 {
        ordinal: descriptor_index,
        descriptor_index,
        physical_kernel_index,
        lineage,
    })
}

fn derive_host_lineage_identity(
    outer: &InertSemanticCompilerModuleHandoffV3,
    record: fe2o3_artifact_transaction::WorkerV3PublicationIntentRecordV1,
    inspection: &FinalizedDescriptorInspection,
    kernel_id: KernelId,
    compiler_execution_subject: &InertCompilerExecutionSubjectV1,
    compiler_execution_receipt: &CompilerExecutionReceiptCarriageV1,
    finalizer_derivation: &RevalidatedProtectedWorkerV3FinalizerDerivationV1,
) -> WorkerV3HostLineageEvidenceV1 {
    let capsule = outer.capsule();
    let receipts = capsule.receipts();
    let capsule_identity = capsule.identity();
    let outer_identity = outer.identity();
    let module_identity = outer.module_handoff().identity();
    let formal_memory = receipts.formal_memory().identity();
    let proof_binding = receipts.proof_binding().identity();
    let finalized_length = u64::try_from(record.output_length())
        .expect("durable publication output length is bounded below u64::MAX");
    let mut digest = Sha256::new();
    digest.update(WORKER_V3_HOST_LINEAGE_DOMAIN_V1);
    digest.update(compiler_execution_subject.identity().sha256());
    digest.update(compiler_execution_subject.canonical_bytes());
    digest.update(compiler_execution_receipt.identity().as_bytes());
    digest.update(compiler_execution_receipt.canonical_bytes());
    digest.update(finalizer_derivation.identity().as_bytes());
    digest.update(record.identity().as_bytes());
    update_identity(
        &mut digest,
        outer_identity.sha256(),
        outer_identity.byte_len(),
    );
    update_identity(
        &mut digest,
        capsule_identity.sha256(),
        capsule_identity.byte_len(),
    );
    update_identity(
        &mut digest,
        module_identity.sha256(),
        module_identity.byte_len(),
    );
    digest.update(15_u16.to_le_bytes());
    update_identity(
        &mut digest,
        receipts.rustc_identity_inventory().identity().sha256(),
        receipts.rustc_identity_inventory().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.rustc_preflight_plan().identity().sha256(),
        receipts.rustc_preflight_plan().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.semantic_mir().identity().sha256(),
        receipts.semantic_mir().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.middle_end().identity().sha256(),
        receipts.middle_end().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.kernel_ir().identity().sha256(),
        receipts.kernel_ir().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.mir_to_kir_correspondence().identity().sha256(),
        receipts.mir_to_kir_correspondence().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        formal_memory.sha256(),
        formal_memory.byte_len(),
    );
    update_identity(
        &mut digest,
        proof_binding.sha256(),
        proof_binding.byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.target_binding().identity().sha256(),
        receipts.target_binding().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.data_layout().identity().sha256(),
        receipts.data_layout().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.abi().identity().sha256(),
        receipts.abi().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.export_manifest().identity().sha256(),
        receipts.export_manifest().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.amdgpu_lowering().identity().sha256(),
        receipts.amdgpu_lowering().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts.semantic_to_llvm().identity().sha256(),
        receipts.semantic_to_llvm().identity().byte_len(),
    );
    update_identity(
        &mut digest,
        receipts
            .final_compiler_module_commitment()
            .identity()
            .sha256(),
        receipts
            .final_compiler_module_commitment()
            .identity()
            .byte_len(),
    );
    digest.update(record.plan().linked_output().as_bytes());
    digest.update(record.output_sha256());
    digest.update(finalized_length.to_le_bytes());
    digest.update(inspection.digest().as_bytes());
    digest.update(kernel_id.as_bytes());

    WorkerV3HostLineageEvidenceV1 {
        identity: WorkerV3HostLineageIdentityV1(digest.finalize().into()),
        finalizer_derivation_sha256: *finalizer_derivation.identity().as_bytes(),
        capsule_sha256: *capsule_identity.sha256(),
        formal_memory_sha256: *formal_memory.sha256(),
        proof_binding_sha256: *proof_binding.sha256(),
        finalized_sha256: record.output_sha256(),
        finalized_length,
    }
}

fn update_identity(digest: &mut Sha256, sha256: &[u8; 32], byte_len: u64) {
    digest.update(sha256);
    digest.update(byte_len.to_le_bytes());
}

fn validate_finalizer_derivation_association(
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    finalized: &[u8],
    derivation: &RevalidatedProtectedWorkerV3FinalizerDerivationV1,
) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
    let record = envelope.wire().replay().publication_intent_record();
    if !derivation.finalized_hsaco_identity().matches(finalized)
        || derivation.finalized_hsaco_identity().sha256() != &record.output_sha256()
        || derivation.finalized_hsaco_identity().byte_len()
            != u64::try_from(record.output_length()).map_err(|_| {
                RecoveredWorkerV3AdmissionErrorV1::FinalizerDerivationAssociationMismatch
            })?
        || derivation.raw_hsaco_identity().sha256() != record.plan().linked_output().as_bytes()
    {
        return Err(RecoveredWorkerV3AdmissionErrorV1::FinalizerDerivationAssociationMismatch);
    }
    Ok(())
}

fn validate_finalized_identity(
    record: fe2o3_artifact_transaction::WorkerV3PublicationIntentRecordV1,
    finalized: &[u8],
) -> Result<FinalizedDescriptorInspection, RecoveredWorkerV3AdmissionErrorV1> {
    if finalized.len() != record.output_length() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::FinalizedLengthMismatch);
    }
    let digest: [u8; 32] = Sha256::digest(finalized).into();
    if digest != record.output_sha256() || digest != *record.plan().finalized_output().as_bytes() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::FinalizedIdentityMismatch);
    }
    let inspection = verify_finalized(finalized)
        .map_err(RecoveredWorkerV3AdmissionErrorV1::FinalizedVerification)?;
    let unfinalized = derive_unfinalized_hsaco_from_finalized_v1(finalized)
        .map_err(RecoveredWorkerV3AdmissionErrorV1::UnfinalizedReconstruction)?;
    let linked_digest: [u8; 32] = Sha256::digest(&unfinalized).into();
    if linked_digest != *record.plan().linked_output().as_bytes() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::LinkedIdentityMismatch);
    }
    Ok(inspection)
}

fn validate_compiler_source_and_exports(
    outer: &InertSemanticCompilerModuleHandoffV3,
    inspection: &FinalizedDescriptorInspection,
) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
    let descriptor_source =
        CompilerDescriptorSourceV1::decode(outer.capsule().receipts().abi().canonical_preimage())
            .map_err(RecoveredWorkerV3AdmissionErrorV1::DescriptorSource)?;
    if outer
        .capsule()
        .receipts()
        .export_manifest()
        .canonical_preimage()
        != outer.module_handoff().symbol_manifest().canonical_bytes()
    {
        return Err(RecoveredWorkerV3AdmissionErrorV1::ExportManifestMismatch);
    }

    let mut normalized = encode_device_descriptor_table_v1(inspection.descriptor_table())
        .map_err(RecoveredWorkerV3AdmissionErrorV1::FinalizedDescriptorEncoding)?;
    let digest_end = CANONICAL_CODE_OBJECT_DIGEST_OFFSET
        .checked_add(32)
        .ok_or(RecoveredWorkerV3AdmissionErrorV1::DescriptorSourceMismatch)?;
    normalized
        .get_mut(CANONICAL_CODE_OBJECT_DIGEST_OFFSET..digest_end)
        .ok_or(RecoveredWorkerV3AdmissionErrorV1::DescriptorSourceMismatch)?
        .fill(0);
    if normalized != descriptor_source.canonical_bytes() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::DescriptorSourceMismatch);
    }
    Ok(())
}

fn validate_target_and_code_object(
    outer: &InertSemanticCompilerModuleHandoffV3,
    inspection: &FinalizedDescriptorInspection,
) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
    let table = inspection.descriptor_table();
    let target = inspection.hsaco().target();
    if production_profile_for_artifact_target(target).is_none() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::UnsupportedTarget);
    }
    if outer.capsule().target().as_amd_target_id() != target
        || outer.module_handoff().target().as_amd_target_id() != target
        || table.device_target().as_amd_target_id() != target
    {
        return Err(RecoveredWorkerV3AdmissionErrorV1::TargetMismatch);
    }
    if inspection.hsaco().code_object_version() != CodeObjectVersion::V6
        || outer.module_handoff().code_object_version().number() != 6
        || table.code_object_version().number() != 6
    {
        return Err(RecoveredWorkerV3AdmissionErrorV1::CodeObjectVersionMismatch);
    }
    Ok(())
}

fn production_profile_for_artifact_target(
    target: AmdTargetId,
) -> Option<ProductionAmdTargetProfileV1> {
    ProductionAmdTargetProfileV1::from_device_target(&target.to_string())
}

#[derive(Clone, Copy)]
struct DescriptorRosterIdentityV1<'identity> {
    logical_name: &'identity str,
    export_name: &'identity str,
    kernel_binding_id: [u8; 32],
}

fn validate_exact_roster(
    expected: &[CompilerGeneratedKernelExpectationRosterEntryV1],
    descriptors: &[KernelDescriptorV1],
) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
    let actual = descriptors
        .iter()
        .map(|descriptor| DescriptorRosterIdentityV1 {
            logical_name: descriptor.logical_name().as_str(),
            export_name: descriptor.entry_name().as_str(),
            kernel_binding_id: *descriptor.kernel_id().as_bytes(),
        })
        .collect::<Vec<_>>();
    validate_exact_roster_identities(expected, &actual)
}

fn validate_exact_roster_identities(
    expected: &[CompilerGeneratedKernelExpectationRosterEntryV1],
    actual: &[DescriptorRosterIdentityV1<'_>],
) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
    if expected.is_empty() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::EmptyRoster);
    }
    for duplicate_ordinal in 0..expected.len() {
        if let Some(first_ordinal) = expected[..duplicate_ordinal]
            .iter()
            .position(|first| roster_expectations_conflict(first, &expected[duplicate_ordinal]))
        {
            return Err(RecoveredWorkerV3AdmissionErrorV1::DuplicateRosterEntry {
                first_ordinal,
                duplicate_ordinal,
            });
        }
    }
    for duplicate_ordinal in 0..actual.len() {
        if let Some(first_ordinal) = actual[..duplicate_ordinal].iter().position(|first| {
            descriptor_roster_identities_conflict(first, &actual[duplicate_ordinal])
        }) {
            return Err(
                RecoveredWorkerV3AdmissionErrorV1::DuplicateDescriptorRosterEntry {
                    first_ordinal,
                    duplicate_ordinal,
                },
            );
        }
    }
    if expected.len() != actual.len() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::RosterLengthMismatch {
            expected: expected.len(),
            actual: actual.len(),
        });
    }
    for (ordinal, (expected_entry, actual_entry)) in expected.iter().zip(actual.iter()).enumerate()
    {
        if roster_entry_matches_descriptor(expected_entry, actual_entry) {
            continue;
        }
        if let Some(actual_ordinal) = actual
            .iter()
            .position(|entry| roster_entry_matches_descriptor(expected_entry, entry))
        {
            return Err(RecoveredWorkerV3AdmissionErrorV1::RosterEntryReordered {
                expected_ordinal: ordinal,
                actual_ordinal,
            });
        }
        return Err(RecoveredWorkerV3AdmissionErrorV1::RosterEntrySubstituted { ordinal });
    }
    Ok(())
}

fn roster_expectations_conflict(
    left: &CompilerGeneratedKernelExpectationRosterEntryV1,
    right: &CompilerGeneratedKernelExpectationRosterEntryV1,
) -> bool {
    left.logical_name() == right.logical_name()
        || left.export_name() == right.export_name()
        || left.kernel_binding_id() == right.kernel_binding_id()
}

fn descriptor_roster_identities_conflict(
    left: &DescriptorRosterIdentityV1<'_>,
    right: &DescriptorRosterIdentityV1<'_>,
) -> bool {
    left.logical_name == right.logical_name
        || left.export_name == right.export_name
        || left.kernel_binding_id == right.kernel_binding_id
}

fn roster_entry_matches_descriptor(
    expected: &CompilerGeneratedKernelExpectationRosterEntryV1,
    actual: &DescriptorRosterIdentityV1<'_>,
) -> bool {
    expected.logical_name() == actual.logical_name
        && expected.export_name() == actual.export_name
        && expected.kernel_binding_id() == actual.kernel_binding_id
}

fn select_unique_index(
    mut matching_indices: impl Iterator<Item = usize>,
    missing: RecoveredWorkerV3AdmissionErrorV1,
    ambiguous: RecoveredWorkerV3AdmissionErrorV1,
) -> Result<usize, RecoveredWorkerV3AdmissionErrorV1> {
    let selected = matching_indices.next().ok_or(missing)?;
    if matching_indices.next().is_some() {
        return Err(ambiguous);
    }
    Ok(selected)
}

fn select_exact_kernel(
    outer: &InertSemanticCompilerModuleHandoffV3,
    inspection: &FinalizedDescriptorInspection,
    kernel_id: KernelId,
) -> Result<(usize, usize), RecoveredWorkerV3AdmissionErrorV1> {
    let descriptor_index = select_unique_index(
        inspection
            .descriptor_table()
            .kernels()
            .iter()
            .enumerate()
            .filter(|(_, descriptor)| descriptor.kernel_id() == kernel_id)
            .map(|(index, _)| index),
        RecoveredWorkerV3AdmissionErrorV1::KernelNotFound,
        RecoveredWorkerV3AdmissionErrorV1::AmbiguousKernel,
    )?;
    let descriptor = &inspection.descriptor_table().kernels()[descriptor_index];

    let physical_index = select_unique_index(
        inspection
            .hsaco()
            .kernels()
            .iter()
            .enumerate()
            .filter(|(_, kernel)| {
                kernel.name() == descriptor.entry_name().as_str()
                    && kernel.symbol() == descriptor.descriptor_symbol().as_str()
            })
            .map(|(index, _)| index),
        RecoveredWorkerV3AdmissionErrorV1::PhysicalKernelNotFound,
        RecoveredWorkerV3AdmissionErrorV1::AmbiguousPhysicalKernel,
    )?;
    let binding = inspection
        .kernel_bindings()
        .bindings()
        .get(physical_index)
        .ok_or(RecoveredWorkerV3AdmissionErrorV1::DescriptorBindingMismatch)?;
    if binding.kernel_index() != physical_index {
        return Err(RecoveredWorkerV3AdmissionErrorV1::DescriptorBindingMismatch);
    }

    let manifest = outer.module_handoff().symbol_manifest();
    let entry_matches = manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
        .filter(|symbol| *symbol == descriptor.entry_name().as_str())
        .count();
    let descriptor_matches = manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
        .filter(|symbol| *symbol == descriptor.descriptor_symbol().as_str())
        .count();
    if entry_matches != 1 || descriptor_matches != 1 {
        return Err(RecoveredWorkerV3AdmissionErrorV1::SelectedExportMismatch);
    }
    Ok((descriptor_index, physical_index))
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV3AdmissionErrorV1 {
    Envelope(WorkerV3LoadEnvelopeErrorV2),
    CurrentPublication(DurableLinkPublicationError),
    FinalizerDerivation(WorkerV3HsacoPublicationErrorV1),
    FinalizerDerivationAssociationMismatch,
    FinalizedLengthMismatch,
    FinalizedIdentityMismatch,
    FinalizedVerification(FinalizationError),
    UnfinalizedReconstruction(FinalizationError),
    LinkedIdentityMismatch,
    OuterHandoff(InertSemanticCompilerModuleHandoffErrorV3),
    DescriptorSource(CompilerDescriptorSourceErrorV1),
    FinalizedDescriptorEncoding(fe2o3_kernel_descriptor::ValidationError),
    DescriptorSourceMismatch,
    ExportManifestMismatch,
    UnsupportedTarget,
    TargetMismatch,
    CodeObjectVersionMismatch,
    EmptyRoster,
    DuplicateRosterEntry {
        first_ordinal: usize,
        duplicate_ordinal: usize,
    },
    DuplicateDescriptorRosterEntry {
        first_ordinal: usize,
        duplicate_ordinal: usize,
    },
    RosterLengthMismatch {
        expected: usize,
        actual: usize,
    },
    RosterEntryReordered {
        expected_ordinal: usize,
        actual_ordinal: usize,
    },
    RosterEntrySubstituted {
        ordinal: usize,
    },
    KernelNotFound,
    AmbiguousKernel,
    PhysicalKernelNotFound,
    AmbiguousPhysicalKernel,
    DescriptorBindingMismatch,
    SelectedExportMismatch,
    InspectionChanged,
    CompilerHandoffChanged,
    CompilerExecutionSubjectChanged,
    ApplicationDescriptorsChanged,
}

impl fmt::Display for RecoveredWorkerV3AdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => write!(formatter, "invalid Worker V3 envelope: {error}"),
            Self::CurrentPublication(error) => {
                write!(formatter, "Worker V3 publication is not current: {error}")
            }
            Self::FinalizerDerivation(error) => {
                write!(formatter, "invalid Worker V3 finalizer derivation: {error}")
            }
            Self::FinalizerDerivationAssociationMismatch => formatter.write_str(
                "Worker V3 finalizer derivation differs from the current publication",
            ),
            Self::FinalizedLengthMismatch => {
                formatter.write_str("finalized HSACO length differs from the Worker V3 publication")
            }
            Self::FinalizedIdentityMismatch => {
                formatter.write_str("finalized HSACO digest differs from the Worker V3 publication")
            }
            Self::FinalizedVerification(error) => {
                write!(
                    formatter,
                    "cannot verify finalized Worker V3 HSACO: {error}"
                )
            }
            Self::UnfinalizedReconstruction(error) => {
                write!(
                    formatter,
                    "cannot reconstruct linked Worker V3 HSACO: {error}"
                )
            }
            Self::LinkedIdentityMismatch => formatter.write_str(
                "reconstructed linked HSACO differs from the Worker V3 publication plan",
            ),
            Self::OuterHandoff(error) => {
                write!(formatter, "invalid Worker V3 semantic handoff: {error}")
            }
            Self::DescriptorSource(error) => {
                write!(
                    formatter,
                    "invalid Worker V3 compiler descriptor source: {error}"
                )
            }
            Self::FinalizedDescriptorEncoding(error) => {
                write!(
                    formatter,
                    "cannot encode finalized Worker V3 descriptor: {error}"
                )
            }
            Self::DescriptorSourceMismatch => formatter
                .write_str("finalized Worker V3 descriptor differs from its exact compiler source"),
            Self::ExportManifestMismatch => formatter
                .write_str("Worker V3 export receipt differs from its compiler module manifest"),
            Self::UnsupportedTarget => {
                formatter.write_str(
                    "Worker V3 host admission requires an exact production gfx942:xnack- or gfx950:xnack- target",
                )
            }
            Self::TargetMismatch => {
                formatter.write_str("Worker V3 compiler, descriptor, and HSACO targets differ")
            }
            Self::CodeObjectVersionMismatch => formatter
                .write_str("Worker V3 host admission requires code-object V6 on every boundary"),
            Self::EmptyRoster => {
                formatter.write_str("Worker V3 marker roster must contain at least one entry")
            }
            Self::DuplicateRosterEntry {
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "Worker V3 marker roster entry {duplicate_ordinal} conflicts with entry {first_ordinal}"
            ),
            Self::DuplicateDescriptorRosterEntry {
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "Worker V3 descriptor roster entry {duplicate_ordinal} conflicts with entry {first_ordinal}"
            ),
            Self::RosterLengthMismatch { expected, actual } => write!(
                formatter,
                "Worker V3 marker roster has {expected} entries but its exact compiler descriptor has {actual}"
            ),
            Self::RosterEntryReordered {
                expected_ordinal,
                actual_ordinal,
            } => write!(
                formatter,
                "Worker V3 marker roster entry {expected_ordinal} occurs at compiler descriptor ordinal {actual_ordinal}"
            ),
            Self::RosterEntrySubstituted { ordinal } => write!(
                formatter,
                "Worker V3 marker roster entry {ordinal} differs from its compiler descriptor entry"
            ),
            Self::KernelNotFound => {
                formatter.write_str("requested kernel is absent from the Worker V3 descriptor")
            }
            Self::AmbiguousKernel => formatter
                .write_str("requested kernel occurs more than once in the Worker V3 descriptor"),
            Self::PhysicalKernelNotFound => {
                formatter.write_str("Worker V3 descriptor has no matching physical kernel")
            }
            Self::AmbiguousPhysicalKernel => {
                formatter.write_str("Worker V3 descriptor matches more than one physical kernel")
            }
            Self::DescriptorBindingMismatch => {
                formatter.write_str("Worker V3 physical ELF kernel and binding indices differ")
            }
            Self::SelectedExportMismatch => formatter
                .write_str("selected Worker V3 kernel is absent from the compiler export roles"),
            Self::InspectionChanged => {
                formatter.write_str("revalidated Worker V3 HSACO inspection changed")
            }
            Self::CompilerHandoffChanged => {
                formatter.write_str("revalidated Worker V3 compiler handoff changed")
            }
            Self::CompilerExecutionSubjectChanged => formatter.write_str(
                "revalidated Worker V3 compiler-execution subject or receipt binding changed",
            ),
            Self::ApplicationDescriptorsChanged => {
                formatter.write_str("retained Worker V3 application descriptors changed")
            }
        }
    }
}

impl Error for RecoveredWorkerV3AdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Envelope(error) => Some(error),
            Self::CurrentPublication(error) => Some(error),
            Self::FinalizerDerivation(error) => Some(error),
            Self::FinalizedVerification(error) | Self::UnfinalizedReconstruction(error) => {
                Some(error)
            }
            Self::OuterHandoff(error) => Some(error),
            Self::DescriptorSource(error) => Some(error),
            Self::FinalizedDescriptorEncoding(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster_entry(
        logical_name: &'static str,
        export_name: &'static str,
        binding: u8,
    ) -> CompilerGeneratedKernelExpectationRosterEntryV1 {
        CompilerGeneratedKernelExpectationRosterEntryV1::from_parts(
            logical_name,
            export_name,
            [binding; 32],
            [binding.wrapping_add(1); 32],
        )
    }

    fn descriptor_identity(
        logical_name: &'static str,
        export_name: &'static str,
        binding: u8,
    ) -> DescriptorRosterIdentityV1<'static> {
        DescriptorRosterIdentityV1 {
            logical_name,
            export_name,
            kernel_binding_id: [binding; 32],
        }
    }

    #[test]
    fn exact_roster_order_is_complete_and_identity_bound() {
        let expected = [
            roster_entry("alpha", "alpha_export", 1),
            roster_entry("beta", "beta_export", 2),
        ];
        let exact = [
            descriptor_identity("alpha", "alpha_export", 1),
            descriptor_identity("beta", "beta_export", 2),
        ];
        assert!(validate_exact_roster_identities(&expected, &exact).is_ok());

        let reordered = [exact[1], exact[0]];
        assert!(matches!(
            validate_exact_roster_identities(&expected, &reordered),
            Err(RecoveredWorkerV3AdmissionErrorV1::RosterEntryReordered {
                expected_ordinal: 0,
                actual_ordinal: 1,
            })
        ));

        let substituted = [
            descriptor_identity("alpha", "alpha_export", 1),
            descriptor_identity("gamma", "gamma_export", 3),
        ];
        assert!(matches!(
            validate_exact_roster_identities(&expected, &substituted),
            Err(RecoveredWorkerV3AdmissionErrorV1::RosterEntrySubstituted { ordinal: 1 })
        ));
    }

    #[test]
    fn exact_roster_rejects_duplicate_missing_and_extra_entries() {
        let expected = [
            roster_entry("alpha", "alpha_export", 1),
            roster_entry("beta", "beta_export", 2),
        ];
        let actual = [
            descriptor_identity("alpha", "alpha_export", 1),
            descriptor_identity("beta", "beta_export", 2),
        ];

        assert!(matches!(
            validate_exact_roster_identities(&[], &[]),
            Err(RecoveredWorkerV3AdmissionErrorV1::EmptyRoster)
        ));

        let duplicate_expected = [expected[0], expected[0]];
        assert!(matches!(
            validate_exact_roster_identities(&duplicate_expected, &actual),
            Err(RecoveredWorkerV3AdmissionErrorV1::DuplicateRosterEntry { .. })
        ));

        let duplicate_actual = [actual[0], actual[0]];
        assert!(matches!(
            validate_exact_roster_identities(&expected, &duplicate_actual),
            Err(RecoveredWorkerV3AdmissionErrorV1::DuplicateDescriptorRosterEntry { .. })
        ));

        assert!(matches!(
            validate_exact_roster_identities(&expected, &actual[..1]),
            Err(RecoveredWorkerV3AdmissionErrorV1::RosterLengthMismatch {
                expected: 2,
                actual: 1,
            })
        ));
        let extra = [
            actual[0],
            actual[1],
            descriptor_identity("gamma", "gamma_export", 3),
        ];
        assert!(matches!(
            validate_exact_roster_identities(&expected, &extra),
            Err(RecoveredWorkerV3AdmissionErrorV1::RosterLengthMismatch {
                expected: 2,
                actual: 3,
            })
        ));
    }

    #[test]
    fn physical_selection_rejects_missing_and_ambiguous_matches() {
        assert!(matches!(
            select_unique_index(
                std::iter::empty(),
                RecoveredWorkerV3AdmissionErrorV1::PhysicalKernelNotFound,
                RecoveredWorkerV3AdmissionErrorV1::AmbiguousPhysicalKernel,
            ),
            Err(RecoveredWorkerV3AdmissionErrorV1::PhysicalKernelNotFound)
        ));
        assert!(matches!(
            select_unique_index(
                [2, 3].into_iter(),
                RecoveredWorkerV3AdmissionErrorV1::PhysicalKernelNotFound,
                RecoveredWorkerV3AdmissionErrorV1::AmbiguousPhysicalKernel,
            ),
            Err(RecoveredWorkerV3AdmissionErrorV1::AmbiguousPhysicalKernel)
        ));
    }

    #[test]
    fn production_artifact_target_profile_is_exact_and_never_relabels() {
        for (target, expected) in [
            ("gfx942:xnack-", ProductionAmdTargetProfileV1::Gfx942),
            ("gfx950:xnack-", ProductionAmdTargetProfileV1::Gfx950),
        ] {
            assert_eq!(
                production_profile_for_artifact_target(AmdTargetId::parse(target).unwrap()),
                Some(expected)
            );
            assert_eq!(expected.device_target(), target);
        }

        for target in ["gfx942", "gfx942:xnack+", "gfx950", "gfx950:xnack+"] {
            assert_eq!(
                production_profile_for_artifact_target(AmdTargetId::parse(target).unwrap()),
                None
            );
        }
    }
}
