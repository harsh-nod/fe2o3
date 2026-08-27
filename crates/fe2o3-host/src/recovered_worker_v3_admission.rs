use std::{error::Error, fmt};

use fe2o3_amd_target::{AmdTargetId, ProductionAmdTargetProfileV1};
use fe2o3_artifact_transaction::DurableCurrentLinkPublicationTokenV1;
use fe2o3_artifact_transaction::{DurableLinkPublicationError, PublishedLinkArtifactV1};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1, CompilerModuleSymbolRoleV1,
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_hsaco::{CodeObjectVersion, InspectedKernel, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::{
    FinalizationError, FinalizedDescriptorInspection, derive_unfinalized_hsaco_from_finalized_v1,
    verify_finalized,
};
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET, DeviceDescriptorTableV1, KernelDescriptorV1, KernelId,
    encode_device_descriptor_table_v1,
};
use fe2o3_runtime_protocol::{RecoveredWorkerV3LoadEnvelopeV1, WorkerV3LoadEnvelopeErrorV1};
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
use crate::application_descriptor_handoff::RetainedWorkerV3ApplicationDescriptorsV1;
use crate::{DeviceIdentity, ObservedContext};

const WORKER_V3_HOST_LINEAGE_DOMAIN_V1: &[u8] = b"fe2o3.host.worker-v3-lineage.v1\0";

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

/// Read-only host admission for one restart-recovered Worker V3 publication.
///
/// Construction independently binds the exact durable artifact to the linked precursor,
/// compiler descriptor source, export manifest, physical metadata and ELF symbols, requested
/// logical kernel, and observed exact production device. The value owns the move-only recovered
/// envelope and its current publication lease, but exposes no HSACO bytes or load/launch
/// transition.
pub struct RecoveredWorkerV3PinnedDescriptorV1 {
    envelope: RecoveredWorkerV3LoadEnvelopeV1,
    outer_handoff: InertSemanticCompilerModuleHandoffV3,
    inspection: FinalizedDescriptorInspection,
    descriptor_index: usize,
    physical_kernel_index: usize,
    lineage: WorkerV3HostLineageEvidenceV1,
    observed: ObservedContext,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<RetainedWorkerV3ApplicationDescriptorsV1>,
}

impl fmt::Debug for RecoveredWorkerV3PinnedDescriptorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV3PinnedDescriptorV1")
            .field("published", &self.published())
            .field("descriptor", self.descriptor())
            .field("target", &self.target())
            .field("code_object_version", &self.code_object_version())
            .field("lineage", &self.lineage.identity)
            .field("device", self.device())
            .finish_non_exhaustive()
    }
}

impl RecoveredWorkerV3PinnedDescriptorV1 {
    /// Revalidates the current durable generation and exact pinned artifact occurrence.
    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        let current = self.acquire_retained_currentness_token()?;
        drop(current);
        Ok(())
    }

    pub(crate) fn acquire_retained_currentness_token(
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

    pub(crate) fn revalidate_retained_currentness_token(
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
            .validate_reacquired_publication_lease_v1(self.envelope.current_publication_lease())
            .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
        let inspected = validate_finalized_identity(
            self.envelope.wire().publication_intent_record(),
            current.exact_artifact_bytes(),
        )?;
        if inspected != self.inspection {
            return Err(RecoveredWorkerV3AdmissionErrorV1::InspectionChanged);
        }
        let outer_handoff =
            InertSemanticCompilerModuleHandoffV3::decode(self.envelope.wire().outer_handoff())
                .map_err(RecoveredWorkerV3AdmissionErrorV1::OuterHandoff)?;
        if outer_handoff != self.outer_handoff {
            return Err(RecoveredWorkerV3AdmissionErrorV1::CompilerHandoffChanged);
        }
        #[cfg(target_os = "linux")]
        if let Some(descriptors) = &self.application_descriptors {
            descriptors
                .revalidate()
                .map_err(|_| RecoveredWorkerV3AdmissionErrorV1::ApplicationDescriptorsChanged)?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn retain_application_descriptors(
        mut self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        debug_assert!(self.application_descriptors.is_none());
        self.application_descriptors = Some(descriptors);
        self
    }

    pub fn published(&self) -> PublishedLinkArtifactV1 {
        self.envelope.current_publication_lease().published()
    }

    pub fn descriptor(&self) -> &KernelDescriptorV1 {
        &self.inspection.descriptor_table().kernels()[self.descriptor_index]
    }

    pub(crate) fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.inspection.descriptor_table()
    }

    pub fn physical_kernel(&self) -> &InspectedKernel {
        &self.inspection.hsaco().kernels()[self.physical_kernel_index]
    }

    pub fn descriptor_binding(&self) -> KernelDescriptorBinding {
        self.inspection.kernel_bindings().bindings()[self.physical_kernel_index]
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity
    }

    pub(crate) const fn lineage_evidence(&self) -> WorkerV3HostLineageEvidenceV1 {
        self.lineage
    }

    pub(crate) const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        &self.outer_handoff
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.observed.device()
    }

    pub(crate) const fn observed_context(&self) -> &ObservedContext {
        &self.observed
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.inspection.hsaco().target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.inspection.hsaco().code_object_version()
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

/// Consumes recovered Worker V3 custody into one independently checked inert host descriptor.
pub fn admit_recovered_worker_v3_descriptor_v1(
    envelope: RecoveredWorkerV3LoadEnvelopeV1,
    kernel_id: KernelId,
    observed: &ObservedContext,
) -> Result<RecoveredWorkerV3PinnedDescriptorV1, RecoveredWorkerV3AdmissionErrorV1> {
    envelope
        .wire()
        .validate_reacquired_publication_lease_v1(envelope.current_publication_lease())
        .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
    let current = envelope
        .current_publication_lease()
        .acquire_current_token()
        .map_err(RecoveredWorkerV3AdmissionErrorV1::CurrentPublication)?;
    current
        .revalidate_locked_currentness()
        .map_err(RecoveredWorkerV3AdmissionErrorV1::CurrentPublication)?;

    let inspection = validate_finalized_identity(
        envelope.wire().publication_intent_record(),
        current.exact_artifact_bytes(),
    )?;
    let outer = InertSemanticCompilerModuleHandoffV3::decode(envelope.wire().outer_handoff())
        .map_err(RecoveredWorkerV3AdmissionErrorV1::OuterHandoff)?;
    validate_compiler_source_and_exports(&outer, &inspection)?;
    validate_target_and_code_object(&outer, &inspection, observed)?;
    let (descriptor_index, physical_kernel_index) =
        select_exact_kernel(&outer, &inspection, kernel_id)?;
    let lineage = derive_host_lineage_identity(
        &outer,
        envelope.wire().publication_intent_record(),
        &inspection,
        kernel_id,
    );
    drop(current);

    Ok(RecoveredWorkerV3PinnedDescriptorV1 {
        envelope,
        outer_handoff: outer,
        inspection,
        descriptor_index,
        physical_kernel_index,
        lineage,
        observed: observed.clone(),
        #[cfg(target_os = "linux")]
        application_descriptors: None,
    })
}

fn derive_host_lineage_identity(
    outer: &InertSemanticCompilerModuleHandoffV3,
    record: fe2o3_artifact_transaction::WorkerV3PublicationIntentRecordV1,
    inspection: &FinalizedDescriptorInspection,
    kernel_id: KernelId,
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
    observed: &ObservedContext,
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
    if !target.is_compatible_with_observed(&observed.device().target_id()) {
        return Err(RecoveredWorkerV3AdmissionErrorV1::ObservedTargetMismatch);
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

fn select_exact_kernel(
    outer: &InertSemanticCompilerModuleHandoffV3,
    inspection: &FinalizedDescriptorInspection,
    kernel_id: KernelId,
) -> Result<(usize, usize), RecoveredWorkerV3AdmissionErrorV1> {
    let mut descriptors = inspection
        .descriptor_table()
        .kernels()
        .iter()
        .enumerate()
        .filter(|(_, descriptor)| descriptor.kernel_id() == kernel_id);
    let (descriptor_index, descriptor) = descriptors
        .next()
        .ok_or(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)?;
    if descriptors.next().is_some() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::AmbiguousKernel);
    }

    let mut physical = inspection
        .hsaco()
        .kernels()
        .iter()
        .enumerate()
        .filter(|(_, kernel)| {
            kernel.name() == descriptor.entry_name().as_str()
                && kernel.symbol() == descriptor.descriptor_symbol().as_str()
        });
    let (physical_index, _) = physical
        .next()
        .ok_or(RecoveredWorkerV3AdmissionErrorV1::PhysicalKernelNotFound)?;
    if physical.next().is_some() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::AmbiguousPhysicalKernel);
    }
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
    Envelope(WorkerV3LoadEnvelopeErrorV1),
    CurrentPublication(DurableLinkPublicationError),
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
    ObservedTargetMismatch,
    CodeObjectVersionMismatch,
    KernelNotFound,
    AmbiguousKernel,
    PhysicalKernelNotFound,
    AmbiguousPhysicalKernel,
    DescriptorBindingMismatch,
    SelectedExportMismatch,
    InspectionChanged,
    CompilerHandoffChanged,
    ApplicationDescriptorsChanged,
}

impl fmt::Display for RecoveredWorkerV3AdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => write!(formatter, "invalid Worker V3 envelope: {error}"),
            Self::CurrentPublication(error) => {
                write!(formatter, "Worker V3 publication is not current: {error}")
            }
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
            Self::ObservedTargetMismatch => formatter
                .write_str("Worker V3 artifact target is incompatible with the observed device"),
            Self::CodeObjectVersionMismatch => formatter
                .write_str("Worker V3 host admission requires code-object V6 on every boundary"),
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
                formatter.write_str("Worker V3 descriptor and physical ELF binding order differ")
            }
            Self::SelectedExportMismatch => formatter
                .write_str("selected Worker V3 kernel is absent from the compiler export roles"),
            Self::InspectionChanged => {
                formatter.write_str("revalidated Worker V3 HSACO inspection changed")
            }
            Self::CompilerHandoffChanged => {
                formatter.write_str("revalidated Worker V3 compiler handoff changed")
            }
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
