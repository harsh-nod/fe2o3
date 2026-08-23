use crate::artifact_binding::validate_generated_profile;
use crate::published_direct_link::payload_kernel_set;
use crate::published_hsaco_inspection::inspect_payload_against_artifact_identity;
use crate::{
    ArtifactBindingError, ArtifactKernelIdentityV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKernelProfileV1, DeviceIdentity, GeneratedKernelProfileError, ObservedContext,
    PublishedKernelPhysicalLayoutV1, PublishedPhysicalLayoutInspectionError,
    ValidatedArtifactSelectionV1,
};
use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationResultV1, BackendPublicationReceiptV1, BuildAttempt,
    DurableCurrentLinkPublicationLeaseV1, DurableCurrentLinkPublicationTokenV1,
    DurableLinkPublicationError, PackageIdentityV1, PublishedLinkArtifactV1,
};
use fe2o3_artifacts::{
    ArtifactContainerV1, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm, DigestBytes,
    DirectLinkBindingSourceV1, DirectLinkBundleIndexIdentityV1, DirectLinkContainerIdentityV1,
    DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkLinkedOutputIdentityV1, PayloadDigest, ProofRecordV1, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
};
use fe2o3_hsaco::{CodeObjectVersion, InspectedKernelBindings, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::PreparedWorkerV2HsacoPublicationV1;
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_worker_v2_bundle::{
    CompilerTransactionEvidenceCapsuleV2, CompilerTransactionEvidenceIdentityV2,
    WorkerV2LoadEnvelopeIdentityV1, WorkerV2LoadEnvelopeV1,
};
use std::fmt;
use std::marker::PhantomData;

/// Version of the complete recovered-lineage prerequisite challenge.
pub const WORKER_V2_FULL_LINEAGE_PREREQUISITE_CHALLENGE_VERSION_V2: u16 = 2;

/// Exact, aggregate identity presented to a reviewed compiler/Verus authenticator.
///
/// This value deliberately embeds existing canonical identities and canonical records rather
/// than defining another digest namespace. Equality therefore compares every recovered lineage
/// component directly, including the selected descriptor and proof record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2FullLineagePrerequisiteChallengeIdentityV2 {
    producer: [u8; 32],
    package: PackageIdentityV1,
    attempt: BuildAttempt,
    receipt: BackendPublicationReceiptV1,
    publication: PublishedLinkArtifactV1,
    envelope: WorkerV2LoadEnvelopeIdentityV1,
    bundle: DirectLinkBundleIndexIdentityV1,
    container: DirectLinkContainerIdentityV1,
    direct_link_evidence: PayloadDigest,
    descriptor_lineage: Box<[u8]>,
    selected_proof_record: ProofRecordV1,
    raw_hsaco: PayloadDigest,
    finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1,
    kernel: ArtifactKernelIdentityV1,
    compiler_transaction: CompilerTransactionEvidenceIdentityV2,
}

impl WorkerV2FullLineagePrerequisiteChallengeIdentityV2 {
    pub const fn version(&self) -> u16 {
        WORKER_V2_FULL_LINEAGE_PREREQUISITE_CHALLENGE_VERSION_V2
    }

    pub const fn producer(&self) -> [u8; 32] {
        self.producer
    }

    pub const fn package(&self) -> PackageIdentityV1 {
        self.package
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn receipt(&self) -> BackendPublicationReceiptV1 {
        self.receipt
    }

    pub const fn publication(&self) -> PublishedLinkArtifactV1 {
        self.publication
    }

    pub const fn envelope(&self) -> WorkerV2LoadEnvelopeIdentityV1 {
        self.envelope
    }

    pub const fn bundle(&self) -> DirectLinkBundleIndexIdentityV1 {
        self.bundle
    }

    pub const fn container(&self) -> DirectLinkContainerIdentityV1 {
        self.container
    }

    pub const fn direct_link_evidence(&self) -> PayloadDigest {
        self.direct_link_evidence
    }

    pub fn descriptor_lineage(&self) -> &[u8] {
        &self.descriptor_lineage
    }

    pub const fn selected_proof_record(&self) -> &ProofRecordV1 {
        &self.selected_proof_record
    }

    pub const fn raw_hsaco(&self) -> PayloadDigest {
        self.raw_hsaco
    }

    pub const fn finalized_hsaco(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.finalized_hsaco
    }

    pub const fn kernel(&self) -> &ArtifactKernelIdentityV1 {
        &self.kernel
    }

    pub const fn compiler_transaction(&self) -> CompilerTransactionEvidenceIdentityV2 {
        self.compiler_transaction
    }
}

#[derive(Debug)]
struct RecoveredWorkerV2FullLineageV2 {
    envelope: WorkerV2LoadEnvelopeIdentityV1,
    descriptor_lineage: Box<[u8]>,
    proof_records: Box<[ProofRecordV1]>,
    raw_hsaco: PayloadDigest,
    compiler_transaction: CompilerTransactionEvidenceIdentityV2,
}

/// Strict, inert host admission for one finalized Worker V2 bundle occurrence.
///
/// Construction consumes both the sealed Worker V2 preparation and its attempt-scoped durable
/// publication. It revalidates the current exact-file lease, measures the concrete container,
/// requires one unique matching bundle/finalization occurrence, selects the kernel from that
/// concrete container, and independently inspects target, code-object version, complete payload
/// kernel set, physical ABI, and represented launch constraints from the retained HSACO bytes.
///
/// This is load-ready evidence rather than load authority. Worker V2 evidence does not yet
/// authenticate the compiler/Verus chain, bind Rust marker types and executable effects, or prove
/// safe HSA module initialization/finalization behavior. Consequently this type has no load or
/// launch method and is intentionally neither `Clone` nor `Copy`.
pub struct AdmittedFinalizedWorkerV2BundleV1 {
    prepared: RetainedWorkerV2PreparationV1,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
    receipt: BackendPublicationReceiptV1,
    published: PublishedLinkArtifactV1,
    bundle_index_identity: DirectLinkBundleIndexIdentityV1,
    bundle_evidence_identity: PayloadDigest,
    binding_index: usize,
    container_identity: DirectLinkContainerIdentityV1,
    linked_output_identity: DirectLinkLinkedOutputIdentityV1,
    finalization_identity: DirectLinkFinalizationIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
    artifact_identity: ArtifactKernelIdentityV1,
    kernel_identities: Box<[ArtifactKernelIdentityV1]>,
    device: DeviceIdentity,
    inspected: InspectedKernelBindings,
    kernels: Box<[PublishedKernelPhysicalLayoutV1]>,
    selected_kernel_index: usize,
    full_lineage: Option<RecoveredWorkerV2FullLineageV2>,
}

enum RetainedWorkerV2PreparationV1 {
    Production(Box<PreparedWorkerV2HsacoPublicationV1>),
    Recovered(Box<WorkerV2LoadEnvelopeV1>),
    #[cfg(any(test, feature = "hardware-test-hooks"))]
    Test {
        attempt: BuildAttempt,
        exact_bytes: Box<[u8]>,
    },
}

impl RetainedWorkerV2PreparationV1 {
    fn attempt(&self) -> BuildAttempt {
        match self {
            Self::Production(prepared) => prepared.attempt(),
            Self::Recovered(envelope) => envelope.published_claim().plan().attempt(),
            #[cfg(any(test, feature = "hardware-test-hooks"))]
            Self::Test { attempt, .. } => *attempt,
        }
    }

    fn exact_bytes(&self) -> &[u8] {
        match self {
            Self::Production(prepared) => prepared.exact_bytes(),
            Self::Recovered(envelope) => envelope.raw_hsaco().bytes(),
            #[cfg(any(test, feature = "hardware-test-hooks"))]
            Self::Test { exact_bytes, .. } => exact_bytes,
        }
    }
}

impl fmt::Debug for AdmittedFinalizedWorkerV2BundleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedFinalizedWorkerV2BundleV1")
            .field("attempt", &self.prepared.attempt())
            .field("published", &self.published)
            .field("bundle_index_identity", &self.bundle_index_identity)
            .field("bundle_evidence_identity", &self.bundle_evidence_identity)
            .field("binding_index", &self.binding_index)
            .field("container_identity", &self.container_identity)
            .field("linked_output_identity", &self.linked_output_identity)
            .field("finalization_identity", &self.finalization_identity)
            .field(
                "finalized_payload_identity",
                &self.finalized_payload_identity,
            )
            .field("artifact_identity", &self.artifact_identity)
            .field("target", &self.inspected.inspection().target())
            .field(
                "code_object_version",
                &self.inspected.inspection().code_object_version(),
            )
            .finish_non_exhaustive()
    }
}

impl AdmittedFinalizedWorkerV2BundleV1 {
    /// Admits one exact Worker V2 publication and finalized bundle occurrence.
    pub fn admit(
        prepared: PreparedWorkerV2HsacoPublicationV1,
        publication: AttemptScopedHsacoPublicationResultV1,
        validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
        container: &ArtifactContainerV1,
        selected: SelectedNativeKernel<'_>,
        observed: &ObservedContext,
    ) -> Result<Self, FinalizedWorkerV2BundleAdmissionError> {
        let parts = admit_parts(
            prepared.attempt(),
            prepared.exact_bytes(),
            publication,
            validated_bundle,
            container,
            selected,
            observed,
        )?;

        Ok(Self::from_parts(
            RetainedWorkerV2PreparationV1::Production(Box::new(prepared)),
            parts,
            None,
        ))
    }

    pub(crate) fn admit_recovered(
        envelope: WorkerV2LoadEnvelopeV1,
        compiler_transaction: CompilerTransactionEvidenceCapsuleV2,
        current_lease: DurableCurrentLinkPublicationLeaseV1,
        kernel_id: KernelId,
        observed: &ObservedContext,
    ) -> Result<Self, FinalizedWorkerV2BundleAdmissionError> {
        let expectation = envelope
            .direct_link_evidence()
            .bindings()
            .first()
            .ok_or(FinalizedWorkerV2BundleAdmissionError::EnvelopeRevalidation)?
            .expectation()
            .clone();
        let source = DirectLinkBindingSourceV1::new(envelope.container(), expectation);
        let validated_bundle = envelope
            .direct_link_evidence()
            .validate_against(envelope.bundle_index(), &[envelope.container()], &[source])
            .map_err(|_| FinalizedWorkerV2BundleAdmissionError::EnvelopeRevalidation)?;
        let selected = envelope
            .container()
            .select_native_kernel(DigestBytes::from_bytes(*kernel_id.as_bytes()))
            .map_err(|_| FinalizedWorkerV2BundleAdmissionError::SelectedKernelSubstitution)?;
        let claim = envelope.published_claim();
        validate_compiler_transaction_lineage(&envelope, &compiler_transaction)?;
        let full_lineage = RecoveredWorkerV2FullLineageV2 {
            envelope: envelope.identity(),
            descriptor_lineage: envelope
                .descriptor_lineage()
                .canonical_bytes()
                .into_boxed_slice(),
            proof_records: envelope.proof_records().to_vec().into_boxed_slice(),
            raw_hsaco: envelope.raw_hsaco().identity(),
            compiler_transaction: compiler_transaction.identity(),
        };
        let parts = admit_parts_with_lease(
            claim.plan().attempt(),
            envelope.raw_hsaco().bytes(),
            claim.receipt(),
            current_lease,
            &validated_bundle,
            envelope.container(),
            selected,
            observed,
        )?;

        Ok(Self::from_parts(
            RetainedWorkerV2PreparationV1::Recovered(Box::new(envelope)),
            parts,
            Some(full_lineage),
        ))
    }

    fn from_parts(
        prepared: RetainedWorkerV2PreparationV1,
        parts: AdmissionParts,
        full_lineage: Option<RecoveredWorkerV2FullLineageV2>,
    ) -> Self {
        Self {
            prepared,
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage,
        }
    }

    pub const fn published(&self) -> PublishedLinkArtifactV1 {
        self.published
    }

    pub const fn receipt(&self) -> BackendPublicationReceiptV1 {
        self.receipt
    }

    pub const fn bundle_index_identity(&self) -> DirectLinkBundleIndexIdentityV1 {
        self.bundle_index_identity
    }

    pub const fn bundle_evidence_identity(&self) -> PayloadDigest {
        self.bundle_evidence_identity
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub const fn container_identity(&self) -> DirectLinkContainerIdentityV1 {
        self.container_identity
    }

    pub const fn linked_output_identity(&self) -> DirectLinkLinkedOutputIdentityV1 {
        self.linked_output_identity
    }

    pub const fn finalization_identity(&self) -> DirectLinkFinalizationIdentityV1 {
        self.finalization_identity
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.finalized_payload_identity
    }

    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.artifact_identity
    }

    pub(crate) fn full_lineage_challenge_for(
        &self,
        kernel: &ArtifactKernelIdentityV1,
    ) -> Result<
        WorkerV2FullLineagePrerequisiteChallengeIdentityV2,
        FinalizedWorkerV2BundleAdmissionError,
    > {
        let lineage = self
            .full_lineage
            .as_ref()
            .ok_or(FinalizedWorkerV2BundleAdmissionError::MissingFullLineage)?;
        let mut proof_matches = lineage.proof_records.iter().filter(|record| {
            record.target().artifact().kernel_id().bytes().as_bytes()
                == kernel.kernel_id().as_bytes()
        });
        let selected_proof_record = proof_matches
            .next()
            .ok_or(FinalizedWorkerV2BundleAdmissionError::MissingKernelLineage)?;
        if proof_matches.next().is_some() {
            return Err(FinalizedWorkerV2BundleAdmissionError::MissingKernelLineage);
        }
        Ok(WorkerV2FullLineagePrerequisiteChallengeIdentityV2 {
            producer: self.receipt.producer_identity(),
            package: self.published.scope().package(),
            attempt: self.prepared.attempt(),
            receipt: self.receipt,
            publication: self.published,
            envelope: lineage.envelope,
            bundle: self.bundle_index_identity,
            container: self.container_identity,
            direct_link_evidence: self.bundle_evidence_identity,
            descriptor_lineage: lineage.descriptor_lineage.clone(),
            selected_proof_record: selected_proof_record.clone(),
            raw_hsaco: lineage.raw_hsaco,
            finalized_hsaco: self.finalized_payload_identity,
            kernel: kernel.clone(),
            compiler_transaction: lineage.compiler_transaction,
        })
    }

    /// Number of manifest kernels physically admitted from the exact finalized payload.
    pub fn kernel_count(&self) -> usize {
        self.kernel_identities.len()
    }

    /// Selects one compiler-generated marker from this exact admitted payload.
    ///
    /// This token is inert: it proves structural marker, ABI/effect, target,
    /// executable, and physical-layout agreement but grants neither HSA load nor
    /// launch authority. A later loaded-executable selection must additionally
    /// bind the exact HSA executable object and resolve this symbol through the
    /// reviewed lifecycle adapter.
    #[doc(hidden)]
    pub fn select_typed_kernel<K: CompilerGeneratedKernelExpectationV1>(
        &self,
    ) -> Result<AdmittedWorkerV2TypedKernelV1<'_, K>, WorkerV2TypedKernelSelectionError> {
        let identity_index = select_typed_kernel_identity::<K>(
            &self.kernel_identities,
            self.artifact_identity.target(),
            self.finalized_payload_identity.digest(),
        )?;
        let identity = &self.kernel_identities[identity_index];
        let mut physical_matches = self
            .kernels
            .iter()
            .enumerate()
            .filter(|(_, physical)| physical.export_symbol() == identity.symbol().as_str());
        let (physical_index, physical) = physical_matches
            .next()
            .ok_or(WorkerV2TypedKernelSelectionError::PhysicalKernelSubstitution)?;
        if physical_matches.next().is_some()
            || validate_selected_identity(identity, physical).is_err()
        {
            return Err(WorkerV2TypedKernelSelectionError::PhysicalKernelSubstitution);
        }
        Ok(AdmittedWorkerV2TypedKernelV1 {
            admission: self,
            identity_index,
            physical_index,
            _marker: PhantomData,
        })
    }

    /// Physical HIP device observation used when this bundle was admitted.
    ///
    /// This remains descriptive identity and grants no HSA authority.
    pub const fn device(&self) -> &DeviceIdentity {
        &self.device
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.inspected.inspection().target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.inspected.inspection().code_object_version()
    }

    pub fn selected_kernel(&self) -> &PublishedKernelPhysicalLayoutV1 {
        &self.kernels[self.selected_kernel_index]
    }

    pub fn selected_descriptor_binding(&self) -> KernelDescriptorBinding {
        self.inspected.bindings()[self.selected_kernel_index]
    }

    pub const fn missing_prerequisites(
        &self,
    ) -> &'static [MissingFinalizedWorkerV2LoadPrerequisiteV1] {
        &MISSING_FINALIZED_WORKER_V2_LOAD_PREREQUISITES_V1
    }

    /// Revalidates currentness and keeps the publication lock in the returned guard.
    pub fn acquire_currentness(
        &self,
    ) -> Result<CurrentFinalizedWorkerV2BundleAdmissionV1<'_>, FinalizedWorkerV2BundleAdmissionError>
    {
        let current = self.acquire_retained_currentness_token()?;
        Ok(CurrentFinalizedWorkerV2BundleAdmissionV1 {
            admission: self,
            _current: current,
        })
    }

    pub(crate) fn acquire_retained_currentness_token(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, FinalizedWorkerV2BundleAdmissionError> {
        let current = self
            .current_lease
            .acquire_current_token()
            .map_err(FinalizedWorkerV2BundleAdmissionError::current_publication)?;
        self.revalidate_retained_currentness_token(&current)?;
        Ok(current)
    }

    pub(crate) fn revalidate_retained_currentness_token(
        &self,
        current: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
        self.current_lease
            .validate_current_token(current)
            .and_then(|()| current.revalidate_locked_currentness())
            .map_err(FinalizedWorkerV2BundleAdmissionError::current_publication)?;
        validate_worker_publication(
            self.prepared.attempt(),
            self.receipt,
            self.published,
            current.exact_artifact_bytes(),
        )?;
        validate_worker_source_identity(
            self.prepared.exact_bytes(),
            self.linked_output_identity,
            self.published,
        )?;
        Ok(())
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

struct AdmissionParts {
    current_lease: DurableCurrentLinkPublicationLeaseV1,
    receipt: BackendPublicationReceiptV1,
    published: PublishedLinkArtifactV1,
    bundle_index_identity: DirectLinkBundleIndexIdentityV1,
    bundle_evidence_identity: PayloadDigest,
    binding_index: usize,
    container_identity: DirectLinkContainerIdentityV1,
    linked_output_identity: DirectLinkLinkedOutputIdentityV1,
    finalization_identity: DirectLinkFinalizationIdentityV1,
    finalized_payload_identity: DirectLinkFinalizedPayloadIdentityV1,
    artifact_identity: ArtifactKernelIdentityV1,
    kernel_identities: Box<[ArtifactKernelIdentityV1]>,
    device: DeviceIdentity,
    inspected: InspectedKernelBindings,
    kernels: Box<[PublishedKernelPhysicalLayoutV1]>,
    selected_kernel_index: usize,
}

fn validate_compiler_transaction_lineage(
    envelope: &WorkerV2LoadEnvelopeV1,
    compiler_transaction: &CompilerTransactionEvidenceCapsuleV2,
) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
    let expectation = envelope
        .direct_link_evidence()
        .bindings()
        .first()
        .ok_or(FinalizedWorkerV2BundleAdmissionError::EnvelopeRevalidation)?
        .expectation();
    let container = DirectLinkContainerIdentityV1::new(
        DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&envelope.container().to_bytes()),
    );
    let checks = [
        (
            compiler_transaction.worker_request() == expectation.request_identity(),
            "direct-link request",
        ),
        (
            compiler_transaction.worker_response() == expectation.response_identity(),
            "direct-link response",
        ),
        (
            compiler_transaction.target() == envelope.published_claim().plan().scope().target(),
            "target",
        ),
        (
            compiler_transaction.raw_hsaco() == expectation.linked_output_identity(),
            "raw HSACO",
        ),
        (
            compiler_transaction.finalized_hsaco() == expectation.finalized_payload_identity(),
            "finalized HSACO",
        ),
        (compiler_transaction.artifact() == container, "container"),
    ];
    for (matches, field) in checks {
        if !matches {
            return Err(
                FinalizedWorkerV2BundleAdmissionError::CompilerTransactionLineageMismatch(field),
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn admit_parts(
    prepared_attempt: BuildAttempt,
    prepared_bytes: &[u8],
    publication: AttemptScopedHsacoPublicationResultV1,
    validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
    observed: &ObservedContext,
) -> Result<AdmissionParts, FinalizedWorkerV2BundleAdmissionError> {
    let receipt = publication.receipt();
    let current_lease = publication.into_current_lease();
    admit_parts_with_lease(
        prepared_attempt,
        prepared_bytes,
        receipt,
        current_lease,
        validated_bundle,
        container,
        selected,
        observed,
    )
}

#[allow(clippy::too_many_arguments)]
fn admit_parts_with_lease(
    prepared_attempt: BuildAttempt,
    prepared_bytes: &[u8],
    receipt: BackendPublicationReceiptV1,
    current_lease: DurableCurrentLinkPublicationLeaseV1,
    validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
    observed: &ObservedContext,
) -> Result<AdmissionParts, FinalizedWorkerV2BundleAdmissionError> {
    let current = current_lease
        .acquire_current_token()
        .map_err(FinalizedWorkerV2BundleAdmissionError::current_publication)?;
    let published = current_lease.published();

    validate_worker_publication(
        prepared_attempt,
        receipt,
        published,
        current.exact_artifact_bytes(),
    )?;

    let container_identity = DirectLinkContainerIdentityV1::new(
        DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM.calculate(&container.to_bytes()),
    );
    let concrete_selection = container
        .select_native_kernel(selected.kernel().kernel_id())
        .map_err(|_| FinalizedWorkerV2BundleAdmissionError::SelectedKernelSubstitution)?;
    if concrete_selection != selected {
        return Err(FinalizedWorkerV2BundleAdmissionError::SelectedKernelSubstitution);
    }

    let (binding_index, linked_output_identity, finalization_identity, finalized_payload_identity) =
        unique_finalized_occurrence(validated_bundle, container_identity, selected)?;
    validate_worker_source_identity(prepared_bytes, linked_output_identity, published)?;
    validate_finalization_identity(
        finalization_identity,
        finalized_payload_identity,
        published,
        current.exact_artifact_bytes(),
        selected,
    )?;

    let validated = ValidatedArtifactSelectionV1::validate(selected, observed)
        .map_err(FinalizedWorkerV2BundleAdmissionError::ArtifactBinding)?;
    let artifact_identity = validated.identity().clone();
    let expected_kernels = payload_kernel_set(selected);
    let kernel_identities = validate_payload_kernel_identities(container, selected, observed)?;
    let (linked_inspected, linked_kernels, linked_selected_kernel_index) =
        inspect_payload_against_artifact_identity(
            prepared_bytes,
            &artifact_identity,
            &expected_kernels,
        )
        .map_err(FinalizedWorkerV2BundleAdmissionError::PhysicalInspection)?;
    let (inspected, kernels, selected_kernel_index) = inspect_payload_against_artifact_identity(
        current.exact_artifact_bytes(),
        &artifact_identity,
        &expected_kernels,
    )
    .map_err(FinalizedWorkerV2BundleAdmissionError::PhysicalInspection)?;
    validate_selected_identity(&artifact_identity, &kernels[selected_kernel_index])?;
    validate_complete_identity_set(&kernel_identities, &kernels)?;
    if linked_inspected.inspection().target() != inspected.inspection().target()
        || linked_inspected.inspection().code_object_version()
            != inspected.inspection().code_object_version()
        || linked_kernels != kernels
        || linked_kernels[linked_selected_kernel_index] != kernels[selected_kernel_index]
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::FinalizationSemanticMismatch);
    }
    drop(current);

    Ok(AdmissionParts {
        current_lease,
        receipt,
        published,
        bundle_index_identity: validated_bundle.evidence().bundle_index_identity(),
        bundle_evidence_identity: validated_bundle
            .evidence()
            .digest(DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM),
        binding_index,
        container_identity,
        linked_output_identity,
        finalization_identity,
        finalized_payload_identity,
        artifact_identity,
        kernel_identities,
        device: observed.device().clone(),
        inspected,
        kernels,
        selected_kernel_index,
    })
}

fn validate_payload_kernel_identities(
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
    observed: &ObservedContext,
) -> Result<Box<[ArtifactKernelIdentityV1]>, FinalizedWorkerV2BundleAdmissionError> {
    let mut identities = Vec::new();
    for kernel in selected
        .manifest()
        .kernels()
        .iter()
        .filter(|kernel| kernel.code_object_digest() == selected.code_object().digest())
    {
        let candidate = container
            .select_native_kernel(kernel.kernel_id())
            .map_err(|_| FinalizedWorkerV2BundleAdmissionError::SelectedKernelSubstitution)?;
        if candidate.payload() != selected.payload() {
            return Err(FinalizedWorkerV2BundleAdmissionError::SelectedKernelSubstitution);
        }
        let validated = ValidatedArtifactSelectionV1::validate(candidate, observed)
            .map_err(FinalizedWorkerV2BundleAdmissionError::ArtifactBinding)?;
        identities.push(validated.identity().clone());
    }
    if identities.is_empty() {
        return Err(FinalizedWorkerV2BundleAdmissionError::SelectedKernelSubstitution);
    }
    Ok(identities.into_boxed_slice())
}

fn validate_complete_identity_set(
    identities: &[ArtifactKernelIdentityV1],
    physical: &[PublishedKernelPhysicalLayoutV1],
) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
    if identities.len() != physical.len() {
        return Err(FinalizedWorkerV2BundleAdmissionError::FinalizationSemanticMismatch);
    }
    for identity in identities {
        let mut matches = physical
            .iter()
            .filter(|kernel| kernel.export_symbol() == identity.symbol().as_str());
        let kernel = matches
            .next()
            .ok_or(FinalizedWorkerV2BundleAdmissionError::FinalizationSemanticMismatch)?;
        if matches.next().is_some() || validate_selected_identity(identity, kernel).is_err() {
            return Err(FinalizedWorkerV2BundleAdmissionError::FinalizationSemanticMismatch);
        }
    }
    Ok(())
}

fn select_typed_kernel_identity<K: CompilerGeneratedKernelExpectationV1>(
    identities: &[ArtifactKernelIdentityV1],
    target: &fe2o3_artifacts::TargetIdentity,
    finalized_payload: PayloadDigest,
) -> Result<usize, WorkerV2TypedKernelSelectionError> {
    if !matches!(
        K::PROFILE,
        CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2
            | CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 { .. }
    ) {
        return Err(WorkerV2TypedKernelSelectionError::UnsupportedGeneratedProfile);
    }

    let mut matches = identities.iter().enumerate().filter(|(_, identity)| {
        identity.name().as_str() == K::LOGICAL_NAME && identity.symbol().as_str() == K::EXPORT_NAME
    });
    let (index, identity) = match matches.next() {
        Some(candidate) => candidate,
        None => {
            let partial_name_match = identities.iter().any(|identity| {
                identity.name().as_str() == K::LOGICAL_NAME
                    || identity.symbol().as_str() == K::EXPORT_NAME
            });
            return Err(if partial_name_match {
                WorkerV2TypedKernelSelectionError::NameSubstitution
            } else {
                WorkerV2TypedKernelSelectionError::KernelNotFound
            });
        }
    };
    if matches.next().is_some() {
        return Err(WorkerV2TypedKernelSelectionError::AmbiguousKernel);
    }
    if identity.target() != target {
        return Err(WorkerV2TypedKernelSelectionError::TargetSubstitution);
    }
    if identity.payload_digest() != finalized_payload {
        return Err(WorkerV2TypedKernelSelectionError::ExecutableSubstitution);
    }
    validate_generated_profile(K::PROFILE, K::KERNEL_BINDING_ID_V1, identity)
        .map_err(WorkerV2TypedKernelSelectionError::GeneratedProfile)?;
    Ok(index)
}

/// Inert typed selection from one exact admitted Worker V2 payload.
///
/// Private fields and the retained admission borrow prevent callers from
/// manufacturing or substituting this evidence. It is intentionally neither
/// `Clone` nor `Copy` and has no HSA operation.
#[doc(hidden)]
pub struct AdmittedWorkerV2TypedKernelV1<'admission, K> {
    admission: &'admission AdmittedFinalizedWorkerV2BundleV1,
    identity_index: usize,
    physical_index: usize,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for AdmittedWorkerV2TypedKernelV1<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedWorkerV2TypedKernelV1")
            .field("artifact_identity", self.artifact_identity())
            .finish_non_exhaustive()
    }
}

impl<K> AdmittedWorkerV2TypedKernelV1<'_, K> {
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.admission.kernel_identities[self.identity_index]
    }

    pub const fn physical_kernel(&self) -> &PublishedKernelPhysicalLayoutV1 {
        &self.admission.kernels[self.physical_index]
    }

    pub fn descriptor_binding(&self) -> KernelDescriptorBinding {
        self.admission.inspected.bindings()[self.physical_index]
    }

    pub const fn finalized_payload_identity(&self) -> DirectLinkFinalizedPayloadIdentityV1 {
        self.admission.finalized_payload_identity
    }

    pub(crate) fn full_lineage_challenge(
        &self,
    ) -> Result<
        WorkerV2FullLineagePrerequisiteChallengeIdentityV2,
        FinalizedWorkerV2BundleAdmissionError,
    > {
        self.admission
            .full_lineage_challenge_for(self.artifact_identity())
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Failure while selecting a typed marker from an exact Worker V2 payload.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum WorkerV2TypedKernelSelectionError {
    UnsupportedGeneratedProfile,
    KernelNotFound,
    NameSubstitution,
    AmbiguousKernel,
    TargetSubstitution,
    ExecutableSubstitution,
    GeneratedProfile(GeneratedKernelProfileError),
    PhysicalKernelSubstitution,
}

impl fmt::Display for WorkerV2TypedKernelSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedGeneratedProfile => {
                formatter.write_str("generated marker profile lacks an admitted typed binding")
            }
            Self::KernelNotFound => {
                formatter.write_str("generated marker is absent from the admitted executable")
            }
            Self::NameSubstitution => {
                formatter.write_str("generated logical and export names select different kernels")
            }
            Self::AmbiguousKernel => {
                formatter.write_str("generated marker names select multiple admitted kernels")
            }
            Self::TargetSubstitution => {
                formatter.write_str("selected kernel target differs from the admitted executable")
            }
            Self::ExecutableSubstitution => {
                formatter.write_str("selected kernel payload differs from the finalized executable")
            }
            Self::GeneratedProfile(error) => error.fmt(formatter),
            Self::PhysicalKernelSubstitution => {
                formatter.write_str("selected kernel differs from the inspected physical layout")
            }
        }
    }
}

impl std::error::Error for WorkerV2TypedKernelSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GeneratedProfile(error) => Some(error),
            _ => None,
        }
    }
}

/// Currentness guard for one exact admitted Worker V2 bundle.
pub struct CurrentFinalizedWorkerV2BundleAdmissionV1<'admission> {
    admission: &'admission AdmittedFinalizedWorkerV2BundleV1,
    _current: DurableCurrentLinkPublicationTokenV1,
}

impl fmt::Debug for CurrentFinalizedWorkerV2BundleAdmissionV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CurrentFinalizedWorkerV2BundleAdmissionV1")
            .field("published", &self.admission.published)
            .field("artifact_identity", &self.admission.artifact_identity)
            .finish_non_exhaustive()
    }
}

impl CurrentFinalizedWorkerV2BundleAdmissionV1<'_> {
    pub const fn admission(&self) -> &AdmittedFinalizedWorkerV2BundleV1 {
        self.admission
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(crate) fn exact_artifact_bytes(&self) -> &[u8] {
        self._current.exact_artifact_bytes()
    }
}

/// Evidence still required before the admitted bytes may enter HSA loading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MissingFinalizedWorkerV2LoadPrerequisiteV1 {
    AuthenticatedCompilerAndVerusChain,
    AuthenticatedRustMarkerAbiAndEffectsBinding,
    AuthenticatedHsaModuleLifecycle,
}

const MISSING_FINALIZED_WORKER_V2_LOAD_PREREQUISITES_V1:
    [MissingFinalizedWorkerV2LoadPrerequisiteV1; 3] = [
    MissingFinalizedWorkerV2LoadPrerequisiteV1::AuthenticatedCompilerAndVerusChain,
    MissingFinalizedWorkerV2LoadPrerequisiteV1::AuthenticatedRustMarkerAbiAndEffectsBinding,
    MissingFinalizedWorkerV2LoadPrerequisiteV1::AuthenticatedHsaModuleLifecycle,
];

fn unique_finalized_occurrence(
    validated_bundle: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    container_identity: DirectLinkContainerIdentityV1,
    selected: SelectedNativeKernel<'_>,
) -> Result<
    (
        usize,
        DirectLinkLinkedOutputIdentityV1,
        DirectLinkFinalizationIdentityV1,
        DirectLinkFinalizedPayloadIdentityV1,
    ),
    FinalizedWorkerV2BundleAdmissionError,
> {
    if !validated_bundle
        .container_identities()
        .contains(&container_identity)
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::ContainerSubstitution);
    }
    let selected_payload =
        PayloadDigest::new(selected.digest_algorithm(), selected.code_object().digest());
    let mut matches = validated_bundle
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, binding)| {
            binding.container_identity() == container_identity
                && binding.expectation().finalized_payload_identity().digest() == selected_payload
        });
    let (index, binding) = matches
        .next()
        .ok_or(FinalizedWorkerV2BundleAdmissionError::MissingFinalizedOccurrence)?;
    if matches.next().is_some() {
        return Err(FinalizedWorkerV2BundleAdmissionError::AmbiguousFinalizedOccurrence);
    }
    Ok((
        index,
        binding.expectation().linked_output_identity(),
        binding.expectation().finalization_identity(),
        binding.expectation().finalized_payload_identity(),
    ))
}

fn validate_worker_publication(
    prepared_attempt: BuildAttempt,
    receipt: BackendPublicationReceiptV1,
    published: PublishedLinkArtifactV1,
    current_bytes: &[u8],
) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
    if prepared_attempt != published.attempt() {
        return Err(FinalizedWorkerV2BundleAdmissionError::WorkerAttemptSubstitution);
    }
    let digest = DigestAlgorithm::Sha256.calculate(current_bytes).bytes();
    if digest.as_bytes() != published.finalized_output().as_bytes()
        || receipt.finalized_output_identity() != *published.finalized_output().as_bytes()
        || receipt.publication_identity() != *published.publication().as_bytes()
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::PublicationReceiptMismatch);
    }
    Ok(())
}

fn validate_worker_source_identity(
    prepared_bytes: &[u8],
    linked_output: DirectLinkLinkedOutputIdentityV1,
    published: PublishedLinkArtifactV1,
) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
    let linked_digest = linked_output.digest();
    if linked_digest.algorithm() != DigestAlgorithm::Sha256
        || linked_digest.bytes().as_bytes() != published.linked_output().as_bytes()
        || linked_digest.verify(prepared_bytes).is_err()
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::WorkerPayloadSubstitution);
    }
    Ok(())
}

fn validate_finalization_identity(
    finalization: DirectLinkFinalizationIdentityV1,
    finalized_payload: DirectLinkFinalizedPayloadIdentityV1,
    published: PublishedLinkArtifactV1,
    current_bytes: &[u8],
    selected: SelectedNativeKernel<'_>,
) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
    let finalization_digest = finalization.digest();
    if finalization_digest.algorithm() != DigestAlgorithm::Sha256
        || finalization_digest.bytes().as_bytes() != published.finalization().as_bytes()
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::FinalizationIdentityMismatch);
    }
    let finalized_digest = finalized_payload.digest();
    if finalized_digest.algorithm() != DigestAlgorithm::Sha256
        || finalized_digest.bytes().as_bytes() != published.finalized_output().as_bytes()
        || finalized_digest.verify(current_bytes).is_err()
        || selected.payload() != current_bytes
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::FinalizedPayloadMismatch);
    }
    Ok(())
}

fn validate_selected_identity(
    identity: &ArtifactKernelIdentityV1,
    physical: &PublishedKernelPhysicalLayoutV1,
) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
    if physical.export_symbol() != identity.symbol().as_str()
        || physical.launch().kernarg_segment_size() < identity.abi().size()
    {
        return Err(FinalizedWorkerV2BundleAdmissionError::SelectedIdentityMismatch);
    }
    Ok(())
}

/// Failure while constructing or revalidating strict Worker V2 host admission.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FinalizedWorkerV2BundleAdmissionError {
    Busy,
    CurrentPublication { reason: String },
    EnvelopeRevalidation,
    WorkerAttemptSubstitution,
    WorkerPayloadSubstitution,
    PublicationReceiptMismatch,
    ContainerSubstitution,
    SelectedKernelSubstitution,
    MissingFinalizedOccurrence,
    AmbiguousFinalizedOccurrence,
    FinalizationIdentityMismatch,
    FinalizedPayloadMismatch,
    FinalizationSemanticMismatch,
    ArtifactBinding(ArtifactBindingError),
    PhysicalInspection(PublishedPhysicalLayoutInspectionError),
    SelectedIdentityMismatch,
    MissingFullLineage,
    MissingKernelLineage,
    CompilerTransactionLineageMismatch(&'static str),
}

impl FinalizedWorkerV2BundleAdmissionError {
    fn current_publication(error: DurableLinkPublicationError) -> Self {
        match error {
            DurableLinkPublicationError::Busy => Self::Busy,
            error => Self::CurrentPublication {
                reason: error.to_string(),
            },
        }
    }
}

impl fmt::Display for FinalizedWorkerV2BundleAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => formatter.write_str("durable Worker V2 publication lock is busy"),
            Self::CurrentPublication { reason } => {
                write!(formatter, "Worker V2 publication is not current: {reason}")
            }
            Self::EnvelopeRevalidation => {
                formatter.write_str("decoded Worker V2 envelope failed structural revalidation")
            }
            Self::WorkerAttemptSubstitution => {
                formatter.write_str("published attempt differs from sealed Worker V2 preparation")
            }
            Self::WorkerPayloadSubstitution => {
                formatter.write_str("published bytes differ from sealed Worker V2 preparation")
            }
            Self::PublicationReceiptMismatch => {
                formatter.write_str("backend receipt differs from the exact durable publication")
            }
            Self::ContainerSubstitution => {
                formatter.write_str("container is absent from validated bundle evidence")
            }
            Self::SelectedKernelSubstitution => {
                formatter.write_str("selected kernel does not belong to the concrete container")
            }
            Self::MissingFinalizedOccurrence => {
                formatter.write_str("validated bundle has no matching finalized payload occurrence")
            }
            Self::AmbiguousFinalizedOccurrence => formatter
                .write_str("validated bundle has multiple matching finalized payload occurrences"),
            Self::FinalizationIdentityMismatch => {
                formatter.write_str("bundle finalization identity differs from durable publication")
            }
            Self::FinalizedPayloadMismatch => {
                formatter.write_str("bundle finalized payload differs from exact published bytes")
            }
            Self::FinalizationSemanticMismatch => formatter.write_str(
                "finalization changed target, code-object version, kernel, ABI, or launch identity",
            ),
            Self::ArtifactBinding(error) => error.fmt(formatter),
            Self::PhysicalInspection(error) => error.fmt(formatter),
            Self::SelectedIdentityMismatch => formatter
                .write_str("selected kernel identity differs from inspected physical layout"),
            Self::MissingFullLineage => formatter.write_str(
                "Worker V2 admission lacks a recovered full-lineage prerequisite challenge",
            ),
            Self::MissingKernelLineage => formatter.write_str(
                "selected kernel lacks one exact descriptor and proof record in recovered lineage",
            ),
            Self::CompilerTransactionLineageMismatch(field) => write!(
                formatter,
                "compiler transaction {field} differs from the recovered envelope"
            ),
        }
    }
}

impl std::error::Error for FinalizedWorkerV2BundleAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ArtifactBinding(error) => Some(error),
            Self::PhysicalInspection(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
pub(crate) mod tests {
    #![cfg_attr(not(test), allow(dead_code, unused_imports))]

    use super::*;
    use crate::published_direct_link::tests::{
        Fixture, alpha_cov6_hsaco_for_target, alpha_zeta_cov6_hsaco_for_target, make_observed_for,
        make_single_hsaco_fixture, make_single_hsaco_fixture_with_kernel_id,
        make_single_hsaco_fixture_with_names_and_kernel_id, make_two_hsaco_fixture_with_kernel_ids,
        make_two_hsaco_fixture_with_kernel_ids_and_abis, physical_test_abi,
        scalar_gemm_v1_hsaco_for_target, typed_vecadd_hsaco_for_target,
        typed_vecadd_two_kernel_hsaco_for_target,
    };
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
        KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
        PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
        UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
        publish_exact_hsaco_evidence_for_attempt_v1,
    };
    #[cfg(feature = "hardware-test-hooks")]
    use fe2o3_artifact_transaction::{
        fail_build_attempt, install_begin_build_attempt_lock_probe_v1,
    };
    use fe2o3_artifacts::{
        AbiField, AbiLayout, Access, BlockSize, CodeObjectFormat, CodeObjectIdentity,
        CodeObjectPayload, CompilerIdentity, Dimensions, DirectLinkBindingExpectationV1,
        DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1, DirectLinkFinalizationIdentityV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkTransformationIdentityV1, Endianness,
        IdentityText, KernelEntry, LaunchContract, ManifestV1, MeasuredToolIdentity, Name,
        PointerWidth, ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome, ProofProperty,
        ProofTargetIdentity, SourceContractIdentity, TargetIdentity, ToolIdentity,
        VerificationModelIdentity, derive_generated_kernel_identity_v2,
    };
    use fe2o3_device::KernelMarkerV1;
    use reserved_fe2o3_symbols::{
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    #[cfg(feature = "hardware-test-hooks")]
    use std::sync::Arc;
    #[cfg(feature = "hardware-test-hooks")]
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::{AtomicU64, Ordering};

    const REQUIRED_GFX942_TEST_TARGET: &str = "gfx942:xnack-";
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    pub struct TestDirectory(PathBuf);

    #[cfg(feature = "hardware-test-hooks")]
    pub struct TestPublicationTurnover {
        completed: Arc<AtomicBool>,
        thread: std::thread::JoinHandle<()>,
    }

    #[cfg(feature = "hardware-test-hooks")]
    impl TestPublicationTurnover {
        pub fn completed(&self) -> bool {
            self.completed.load(Ordering::SeqCst)
        }

        pub fn finish(self) {
            let completed = self.completed;
            self.thread.join().expect("turnover thread must not panic");
            assert!(completed.load(Ordering::SeqCst));
        }
    }

    impl TestDirectory {
        fn new() -> Self {
            fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
            let path = std::env::temp_dir().join(format!(
                "fe2o3-host-worker-v2-admission-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    #[cfg(feature = "hardware-test-hooks")]
    pub fn begin_test_publication_turnover(directory: &TestDirectory) -> TestPublicationTurnover {
        let output = directory.path().to_path_buf();
        let owner = ProducerIdentity::from_codegen(
            "fe2o3_host_worker_v2_admission",
            Some(Path::new("tests/worker_v2_bundle_admission.rs")),
        )
        .unwrap();
        let completed = Arc::new(AtomicBool::new(false));
        let thread_completed = completed.clone();
        let lock_probe = install_begin_build_attempt_lock_probe_v1(&output, &owner);
        let thread = std::thread::spawn(move || {
            let next = begin_build_attempt(
                &output,
                &owner,
                BuildInvocation::from_bytes([0xd3; 32]),
                BuildSession::from_bytes([0xd4; 16]),
            )
            .unwrap();
            assert_eq!(next.generation(), 2);
            fail_build_attempt(&output, &owner, next).unwrap();
            thread_completed.store(true, Ordering::SeqCst);
        });
        lock_probe.wait_until_contended();
        TestPublicationTurnover { completed, thread }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct AdmissionFixture {
        _directory: TestDirectory,
        fixture: Fixture,
        attempt: BuildAttempt,
        publication: AttemptScopedHsacoPublicationResultV1,
        exact_bytes: Vec<u8>,
        finalized_bytes: Vec<u8>,
    }

    fn exact_launch(block_x: u32) -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(block_x, 1, 1).unwrap()),
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    fn repeated_digest(seed: u8) -> fe2o3_artifacts::DigestBytes {
        fe2o3_artifacts::DigestBytes::from_bytes([seed; 32])
    }

    fn test_payload_digest(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, repeated_digest(seed.max(1)))
    }

    fn test_proof_record(kernel: &fe2o3_artifacts::KernelEntry, seed: u8) -> ProofRecordV1 {
        let tool = |name: &str, offset: u8| {
            MeasuredToolIdentity::new(
                IdentityText::new(name).unwrap(),
                IdentityText::new("test").unwrap(),
                test_payload_digest(seed.wrapping_add(offset)),
                test_payload_digest(seed.wrapping_add(offset).wrapping_add(1)),
            )
        };
        ProofRecordV1::new(
            ProofTargetIdentity::new(
                ProofArtifactIdentity::new(
                    PayloadDigest::new(DigestAlgorithm::Sha256, kernel.kernel_id()),
                    test_payload_digest(seed.wrapping_add(1)),
                    PayloadDigest::new(DigestAlgorithm::Sha256, kernel.source_digest()),
                    test_payload_digest(seed.wrapping_add(2)),
                    PayloadDigest::new(DigestAlgorithm::Sha256, kernel.executable_digest()),
                    test_payload_digest(seed.wrapping_add(3)),
                    test_payload_digest(seed.wrapping_add(4)),
                    test_payload_digest(seed.wrapping_add(5)),
                ),
                SourceContractIdentity::new(
                    test_payload_digest(seed.wrapping_add(6)),
                    test_payload_digest(seed.wrapping_add(7)),
                    test_payload_digest(seed.wrapping_add(8)),
                    test_payload_digest(seed.wrapping_add(9)),
                    test_payload_digest(seed.wrapping_add(10)),
                ),
            ),
            vec![],
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(
                    IdentityText::new("test-model").unwrap(),
                    test_payload_digest(seed.wrapping_add(11)),
                ),
                tool("verus", 12),
                tool("solver", 14),
                tool("recorder", 16),
                test_payload_digest(seed.wrapping_add(18)),
            ),
            ProofOutcome::Proved,
            vec![ProofProperty::Bounds],
            vec![],
        )
        .unwrap()
    }

    fn test_full_lineage(fixture: &Fixture, seed: u8) -> RecoveredWorkerV2FullLineageV2 {
        let selected = selected(fixture);
        let proof_records = selected
            .manifest()
            .kernels()
            .iter()
            .filter(|kernel| kernel.code_object_digest() == selected.code_object().digest())
            .map(|kernel| test_proof_record(kernel, seed))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        RecoveredWorkerV2FullLineageV2 {
            envelope: WorkerV2LoadEnvelopeIdentityV1::from_bytes([seed.max(1); 32]),
            descriptor_lineage: vec![seed.max(1)].into_boxed_slice(),
            proof_records,
            raw_hsaco: fixture.expectations[0].linked_output_identity().digest(),
            compiler_transaction: CompilerTransactionEvidenceIdentityV2::from_bytes(
                [seed.wrapping_add(1).max(1); 32],
            )
            .unwrap(),
        }
    }

    fn admission_fixture(seed: u8, plan_finalization_delta: u8) -> AdmissionFixture {
        admission_fixture_with_linked_bytes(seed, plan_finalization_delta, None)
    }

    fn admission_fixture_with_linked_bytes(
        seed: u8,
        plan_finalization_delta: u8,
        linked_bytes: Option<Vec<u8>>,
    ) -> AdmissionFixture {
        let hsaco = typed_vecadd_hsaco_for_target(REQUIRED_GFX942_TEST_TARGET);
        let abi = crate::generated_vecadd::generated_vecadd_abi_v2().unwrap();
        let launch = exact_launch(256);
        let kernel_id = derive_generated_kernel_identity_v2(
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            [0x4b; 32],
            "logical_primary",
            "primary_kernel",
            repeated_digest(seed.wrapping_add(0x40)),
            repeated_digest(seed.wrapping_add(0x50)),
            &abi,
            &launch,
        );
        let fixture = make_single_hsaco_fixture_with_kernel_id(
            seed,
            hsaco.bytes.clone(),
            REQUIRED_GFX942_TEST_TARGET,
            abi,
            launch,
            kernel_id,
        );
        let linked_bytes = linked_bytes.unwrap_or_else(|| hsaco.bytes.clone());
        finish_admission_fixture(
            seed,
            plan_finalization_delta,
            fixture,
            linked_bytes,
            hsaco.bytes,
        )
    }

    fn two_kernel_admission_fixture(seed: u8) -> AdmissionFixture {
        let hsaco = typed_vecadd_two_kernel_hsaco_for_target(REQUIRED_GFX942_TEST_TARGET);
        let abi = crate::generated_vecadd::generated_vecadd_abi_v2().unwrap();
        let launch = exact_launch(256);
        let first_kernel = derive_generated_kernel_identity_v2(
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            [0x4b; 32],
            "logical_primary",
            "primary_kernel",
            repeated_digest(seed.wrapping_add(0x40)),
            repeated_digest(seed.wrapping_add(0x50)),
            &abi,
            &launch,
        );
        let second_kernel = derive_generated_kernel_identity_v2(
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            [0x5b; 32],
            "logical_second",
            "second_kernel",
            repeated_digest(seed.wrapping_add(0x41)),
            repeated_digest(seed.wrapping_add(0x51)),
            &abi,
            &launch,
        );
        let fixture = make_two_hsaco_fixture_with_kernel_ids(
            seed,
            hsaco.bytes.clone(),
            REQUIRED_GFX942_TEST_TARGET,
            "logical_primary",
            "primary_kernel",
            first_kernel,
            "logical_second",
            "second_kernel",
            second_kernel,
            abi,
            launch,
        );
        finish_admission_fixture(seed, 0, fixture, hsaco.bytes.clone(), hsaco.bytes)
    }

    fn alpha_cov6_admission_fixture(seed: u8) -> AdmissionFixture {
        let hsaco = alpha_cov6_hsaco_for_target(REQUIRED_GFX942_TEST_TARGET);
        let abi = crate::generated_alpha_zeta_cov6::alpha_cov6_test_abi();
        let launch = crate::generated_alpha_zeta_cov6::alpha_cov6_test_launch();
        let kernel_binding = [0x61; 32];
        let kernel_id = derive_generated_kernel_identity_v2(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            kernel_binding,
            "alpha",
            "alpha",
            repeated_digest(seed.wrapping_add(0x40)),
            repeated_digest(seed.wrapping_add(0x50)),
            &abi,
            &launch,
        );
        let fixture = make_single_hsaco_fixture_with_names_and_kernel_id(
            seed,
            hsaco.bytes.clone(),
            REQUIRED_GFX942_TEST_TARGET,
            "alpha",
            "alpha",
            abi,
            launch,
            kernel_id,
        );
        finish_admission_fixture(seed, 0, fixture, hsaco.bytes.clone(), hsaco.bytes)
    }

    fn scalar_gemm_v1_admission_fixture(seed: u8) -> AdmissionFixture {
        let hsaco = scalar_gemm_v1_hsaco_for_target(REQUIRED_GFX942_TEST_TARGET);
        let abi = crate::generated_scalar_gemm_v1::scalar_gemm_v1_test_abi();
        let launch = crate::generated_scalar_gemm_v1::scalar_gemm_v1_test_launch();
        let kernel_binding = [0x71; 32];
        let kernel_id = derive_generated_kernel_identity_v2(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            kernel_binding,
            "scalar_gemm_v1",
            "scalar_gemm_v1",
            repeated_digest(seed.wrapping_add(0x40)),
            repeated_digest(seed.wrapping_add(0x50)),
            &abi,
            &launch,
        );
        let fixture = make_single_hsaco_fixture_with_names_and_kernel_id(
            seed,
            hsaco.bytes.clone(),
            REQUIRED_GFX942_TEST_TARGET,
            "scalar_gemm_v1",
            "scalar_gemm_v1",
            abi,
            launch,
            kernel_id,
        );
        finish_admission_fixture(seed, 0, fixture, hsaco.bytes.clone(), hsaco.bytes)
    }

    fn renamed_abi_field(template: &AbiField, name: &str, offset: u64) -> AbiField {
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            template.size(),
            template.alignment(),
            template.kind(),
            template.mutability(),
            template.access(),
            template.address_space(),
            template.type_identity(),
            template.ownership(),
            template.alias_class(),
        )
        .unwrap()
    }

    fn zeta_cov6_test_abi() -> AbiLayout {
        let alpha = crate::generated_alpha_zeta_cov6::alpha_cov6_test_abi();
        let scalar = &alpha.fields()[0];
        let shared_slice = &alpha.fields()[1];
        let disjoint_slice = &alpha.fields()[2];
        AbiLayout::new(
            56,
            8,
            PointerWidth::Bits64,
            vec![
                renamed_abi_field(shared_slice, "a", 0),
                renamed_abi_field(shared_slice, "b", 16),
                renamed_abi_field(scalar, "bias", 32),
                renamed_abi_field(disjoint_slice, "output", 40),
            ],
        )
        .unwrap()
    }

    fn finish_admission_fixture(
        seed: u8,
        plan_finalization_delta: u8,
        mut fixture: Fixture,
        linked_bytes: Vec<u8>,
        finalized_bytes: Vec<u8>,
    ) -> AdmissionFixture {
        bind_worker_linked_output(&mut fixture, &linked_bytes);
        let directory = TestDirectory::new();
        let producer = ProducerIdentity::from_codegen(
            "fe2o3_host_worker_v2_admission",
            Some(Path::new("tests/worker_v2_bundle_admission.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory.0,
            &producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap();
        let output_digest = DigestAlgorithm::Sha256.calculate(&finalized_bytes).bytes();
        let linked_digest = DigestAlgorithm::Sha256.calculate(&linked_bytes).bytes();
        let expected_finalization = fixture.expectations[0]
            .finalization_identity()
            .digest()
            .bytes();
        let mut finalization = *expected_finalization.as_bytes();
        finalization[0] = finalization[0].wrapping_add(plan_finalization_delta);
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
                KernelSetIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
                TargetIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
            PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
            ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
            LinkedOutputIdentityV1::from_bytes(*linked_digest.as_bytes()),
            FinalizationIdentityV1::from_bytes(finalization),
            FinalizedOutputIdentityV1::from_bytes(*output_digest.as_bytes()),
            AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
        );
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(9); 32]),
            &finalized_bytes,
        )
        .unwrap();
        AdmissionFixture {
            _directory: directory,
            fixture,
            attempt,
            publication,
            exact_bytes: linked_bytes,
            finalized_bytes,
        }
    }

    fn selected(fixture: &Fixture) -> SelectedNativeKernel<'_> {
        fixture
            .container
            .select_native_kernel(fixture.primary_kernel)
            .unwrap()
    }

    fn bind_worker_linked_output(fixture: &mut Fixture, linked_bytes: &[u8]) {
        let previous = fixture.expectations[0].clone();
        fixture.expectations[0] = DirectLinkBindingExpectationV1::new(
            previous.request_identity(),
            previous.worker().clone(),
            previous.toolchain().clone(),
            previous.response_identity(),
            DirectLinkTransformationIdentityV1::new(
                DirectLinkLinkedOutputIdentityV1::new(
                    DigestAlgorithm::Sha256.calculate(linked_bytes),
                ),
                previous.finalization_identity(),
                previous.finalized_payload_identity(),
            ),
            previous.ffi_contract_identity(),
        );
        let sources = fixture
            .expectations
            .iter()
            .cloned()
            .map(|expectation| DirectLinkBindingSourceV1::new(&fixture.container, expectation))
            .collect::<Vec<_>>();
        fixture.evidence =
            DirectLinkBundleEvidenceV1::bind(&fixture.bundle, &[&fixture.container], &sources)
                .unwrap();
    }

    pub(crate) fn admitted_for_lifecycle_test(
        seed: u8,
    ) -> (AdmittedFinalizedWorkerV2BundleV1, TestDirectory) {
        let input = admission_fixture(seed, 0);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(seed.into(), REQUIRED_GFX942_TEST_TARGET);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            &observed,
        )
        .unwrap();
        let admission = AdmittedFinalizedWorkerV2BundleV1 {
            prepared: RetainedWorkerV2PreparationV1::Test {
                attempt: input.attempt,
                exact_bytes: input.exact_bytes.into_boxed_slice(),
            },
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage: Some(test_full_lineage(&input.fixture, seed)),
        };
        (admission, input._directory)
    }

    pub(crate) fn admitted_two_kernel_for_lifecycle_test(
        seed: u8,
    ) -> (AdmittedFinalizedWorkerV2BundleV1, TestDirectory) {
        let input = two_kernel_admission_fixture(seed);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(seed.into(), REQUIRED_GFX942_TEST_TARGET);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            &observed,
        )
        .unwrap();
        let admission = AdmittedFinalizedWorkerV2BundleV1 {
            prepared: RetainedWorkerV2PreparationV1::Test {
                attempt: input.attempt,
                exact_bytes: input.exact_bytes.into_boxed_slice(),
            },
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage: Some(test_full_lineage(&input.fixture, seed)),
        };
        (admission, input._directory)
    }

    pub(crate) fn admitted_alpha_cov6_for_lifecycle_test(
        seed: u8,
    ) -> (AdmittedFinalizedWorkerV2BundleV1, TestDirectory) {
        let input = alpha_cov6_admission_fixture(seed);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(seed.into(), REQUIRED_GFX942_TEST_TARGET);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            &observed,
        )
        .unwrap();
        let admission = AdmittedFinalizedWorkerV2BundleV1 {
            prepared: RetainedWorkerV2PreparationV1::Test {
                attempt: input.attempt,
                exact_bytes: input.exact_bytes.into_boxed_slice(),
            },
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage: Some(test_full_lineage(&input.fixture, seed)),
        };
        (admission, input._directory)
    }

    pub(crate) fn admitted_scalar_gemm_v1_for_lifecycle_test(
        seed: u8,
    ) -> (AdmittedFinalizedWorkerV2BundleV1, TestDirectory) {
        let input = scalar_gemm_v1_admission_fixture(seed);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(seed.into(), REQUIRED_GFX942_TEST_TARGET);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            &observed,
        )
        .unwrap();
        let admission = AdmittedFinalizedWorkerV2BundleV1 {
            prepared: RetainedWorkerV2PreparationV1::Test {
                attempt: input.attempt,
                exact_bytes: input.exact_bytes.into_boxed_slice(),
            },
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage: Some(test_full_lineage(&input.fixture, seed)),
        };
        (admission, input._directory)
    }

    #[allow(dead_code)]
    pub fn admitted_hardware_for_lifecycle_test(
        seed: u8,
        finalized_bytes: Vec<u8>,
        logical_name: &str,
        export_symbol: &str,
        marker_binding_identity: [u8; 32],
        observed: &ObservedContext,
    ) -> (AdmittedFinalizedWorkerV2BundleV1, TestDirectory) {
        let abi = crate::generated_vecadd::generated_vecadd_abi_v2().unwrap();
        let launch = exact_launch(256);
        let kernel_id = derive_generated_kernel_identity_v2(
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            marker_binding_identity,
            logical_name,
            export_symbol,
            repeated_digest(seed.wrapping_add(0x40)),
            repeated_digest(seed.wrapping_add(0x50)),
            &abi,
            &launch,
        );
        let mut fixture = make_single_hsaco_fixture_with_names_and_kernel_id(
            seed,
            finalized_bytes.clone(),
            "gfx942",
            logical_name,
            export_symbol,
            abi,
            launch,
            kernel_id,
        );
        bind_worker_linked_output(&mut fixture, &finalized_bytes);

        let directory = TestDirectory::new();
        let producer = ProducerIdentity::from_codegen(
            "fe2o3_host_worker_v2_hardware_admission",
            Some(Path::new("tests/gfx942_hardware.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory.0,
            &producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap();
        let output_digest = DigestAlgorithm::Sha256.calculate(&finalized_bytes).bytes();
        let expected_finalization = fixture.expectations[0]
            .finalization_identity()
            .digest()
            .bytes();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
                KernelSetIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
                TargetIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
            PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
            ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
            LinkedOutputIdentityV1::from_bytes(*output_digest.as_bytes()),
            FinalizationIdentityV1::from_bytes(*expected_finalization.as_bytes()),
            FinalizedOutputIdentityV1::from_bytes(*output_digest.as_bytes()),
            AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
        );
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(9); 32]),
            &finalized_bytes,
        )
        .unwrap();
        let validated = fixture.validated();
        let selected_kernel = selected(&fixture);
        let parts = admit_parts(
            attempt,
            &finalized_bytes,
            publication,
            &validated,
            &fixture.container,
            selected_kernel,
            observed,
        )
        .unwrap();
        let admission = AdmittedFinalizedWorkerV2BundleV1 {
            prepared: RetainedWorkerV2PreparationV1::Test {
                attempt,
                exact_bytes: finalized_bytes.into_boxed_slice(),
            },
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage: Some(test_full_lineage(&fixture, seed)),
        };
        (admission, directory)
    }

    /// Admits one exact finalized gfx942 COV6 payload containing both generated roles.
    #[allow(dead_code)]
    pub fn admitted_alpha_zeta_cov6_hardware_for_lifecycle_test(
        seed: u8,
        finalized_bytes: Vec<u8>,
        alpha_marker_binding_identity: [u8; 32],
        zeta_marker_binding_identity: [u8; 32],
        observed: &ObservedContext,
    ) -> (AdmittedFinalizedWorkerV2BundleV1, TestDirectory) {
        let alpha_abi = crate::generated_alpha_zeta_cov6::alpha_cov6_test_abi();
        let zeta_abi = zeta_cov6_test_abi();
        let launch = crate::generated_alpha_zeta_cov6::alpha_cov6_test_launch();
        let payload_target = fe2o3_hsaco::inspect(&finalized_bytes)
            .expect("hardware test payload must be inspectable")
            .target()
            .to_string();
        let alpha_kernel = derive_generated_kernel_identity_v2(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            alpha_marker_binding_identity,
            "alpha",
            "alpha",
            repeated_digest(seed.wrapping_add(0x40)),
            repeated_digest(seed.wrapping_add(0x50)),
            &alpha_abi,
            &launch,
        );
        let zeta_kernel = derive_generated_kernel_identity_v2(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            zeta_marker_binding_identity,
            "zeta",
            "zeta",
            repeated_digest(seed.wrapping_add(0x41)),
            repeated_digest(seed.wrapping_add(0x51)),
            &zeta_abi,
            &launch,
        );
        let fixture = make_two_hsaco_fixture_with_kernel_ids_and_abis(
            seed,
            finalized_bytes.clone(),
            &payload_target,
            "alpha",
            "alpha",
            alpha_kernel,
            alpha_abi,
            "zeta",
            "zeta",
            zeta_kernel,
            zeta_abi,
            launch,
        );
        let input =
            finish_admission_fixture(seed, 0, fixture, finalized_bytes.clone(), finalized_bytes);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            observed,
        )
        .unwrap();
        let admission = AdmittedFinalizedWorkerV2BundleV1 {
            prepared: RetainedWorkerV2PreparationV1::Test {
                attempt: input.attempt,
                exact_bytes: input.exact_bytes.into_boxed_slice(),
            },
            current_lease: parts.current_lease,
            receipt: parts.receipt,
            published: parts.published,
            bundle_index_identity: parts.bundle_index_identity,
            bundle_evidence_identity: parts.bundle_evidence_identity,
            binding_index: parts.binding_index,
            container_identity: parts.container_identity,
            linked_output_identity: parts.linked_output_identity,
            finalization_identity: parts.finalization_identity,
            finalized_payload_identity: parts.finalized_payload_identity,
            artifact_identity: parts.artifact_identity,
            kernel_identities: parts.kernel_identities,
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
            full_lineage: Some(test_full_lineage(&input.fixture, seed)),
        };
        (admission, input._directory)
    }

    #[test]
    fn alpha_zeta_hardware_helper_admits_exact_two_kernel_cov6_payload() {
        let seed = 0xa7;
        let alpha_binding = [0x61; 32];
        let zeta_binding = [0x71; 32];
        let hsaco = alpha_zeta_cov6_hsaco_for_target("gfx942:xnack-");
        let exact_bytes = hsaco.bytes.clone();
        let observed = make_observed_for(seed.into(), "gfx942:sramecc+:xnack-");
        let (admission, _directory) = admitted_alpha_zeta_cov6_hardware_for_lifecycle_test(
            seed,
            hsaco.bytes,
            alpha_binding,
            zeta_binding,
            &observed,
        );

        assert_eq!(admission.kernel_count(), 2);
        assert_eq!(admission.target().to_string(), "gfx942:xnack-");
        assert_eq!(admission.artifact_identity.name().as_str(), "alpha");
        assert_eq!(admission.artifact_identity.symbol().as_str(), "alpha");
        assert_eq!(admission.artifact_identity.abi().size(), 40);
        assert_eq!(admission.selected_kernel().export_symbol(), "alpha");
        assert_eq!(
            admission.selected_kernel().launch().kernarg_segment_size(),
            296
        );

        let alpha = admission
            .kernel_identities
            .iter()
            .find(|identity| identity.name().as_str() == "alpha")
            .unwrap();
        let zeta = admission
            .kernel_identities
            .iter()
            .find(|identity| identity.name().as_str() == "zeta")
            .unwrap();
        assert_eq!((alpha.name().as_str(), alpha.abi().size()), ("alpha", 40));
        assert_eq!((zeta.name().as_str(), zeta.abi().size()), ("zeta", 56));
        assert_eq!(
            alpha.kernel_id().as_bytes(),
            derive_generated_kernel_identity_v2(
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                alpha_binding,
                "alpha",
                "alpha",
                repeated_digest(seed.wrapping_add(0x40)),
                repeated_digest(seed.wrapping_add(0x50)),
                alpha.abi(),
                alpha.launch(),
            )
            .as_bytes()
        );
        assert_eq!(
            zeta.kernel_id().as_bytes(),
            derive_generated_kernel_identity_v2(
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                zeta_binding,
                "zeta",
                "zeta",
                repeated_digest(seed.wrapping_add(0x41)),
                repeated_digest(seed.wrapping_add(0x51)),
                zeta.abi(),
                zeta.launch(),
            )
            .as_bytes()
        );
        let physical_zeta = admission
            .kernels
            .iter()
            .find(|kernel| kernel.export_symbol() == "zeta")
            .unwrap();
        assert_eq!(physical_zeta.launch().kernarg_segment_size(), 312);
        let current = admission.acquire_currentness().unwrap();
        assert_eq!(current.exact_artifact_bytes(), exact_bytes);
    }

    #[test]
    fn admits_exact_current_worker_publication_and_binds_all_host_identities() {
        let input = admission_fixture(0x31, 0);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x31, REQUIRED_GFX942_TEST_TARGET);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            &observed,
        )
        .unwrap();

        assert_eq!(parts.published.attempt(), input.attempt);
        assert_eq!(parts.binding_index, 0);
        assert_eq!(
            parts.artifact_identity.kernel_id().as_bytes(),
            selected_kernel.kernel().kernel_id().as_bytes()
        );
        assert_eq!(
            parts.inspected.inspection().target().to_string(),
            REQUIRED_GFX942_TEST_TARGET
        );
        assert_eq!(
            parts.inspected.inspection().code_object_version().number(),
            6
        );
        assert_eq!(
            parts.kernels[parts.selected_kernel_index].export_symbol(),
            "primary_kernel"
        );
        assert_eq!(
            parts.kernels[parts.selected_kernel_index]
                .launch()
                .required_workgroup_size(),
            crate::PhysicalMetadataValueV1::Known([256, 1, 1])
        );
        let current = parts.current_lease.acquire_current_token().unwrap();
        assert_eq!(current.exact_artifact_bytes(), input.exact_bytes);
    }

    #[test]
    fn binds_transformed_finalized_bytes_to_their_distinct_worker_linked_output() {
        let mut linked_bytes = typed_vecadd_hsaco_for_target(REQUIRED_GFX942_TEST_TARGET).bytes;
        linked_bytes.push(0);
        let input = admission_fixture_with_linked_bytes(0x35, 0, Some(linked_bytes.clone()));
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x35, REQUIRED_GFX942_TEST_TARGET);
        let parts = admit_parts(
            input.attempt,
            &input.exact_bytes,
            input.publication,
            &validated,
            &input.fixture.container,
            selected_kernel,
            &observed,
        )
        .unwrap();

        assert_eq!(input.exact_bytes, linked_bytes);
        assert_ne!(input.exact_bytes, input.finalized_bytes);
        assert_eq!(
            parts.linked_output_identity.digest(),
            DigestAlgorithm::Sha256.calculate(&input.exact_bytes)
        );
        let current = parts.current_lease.acquire_current_token().unwrap();
        assert_eq!(current.exact_artifact_bytes(), input.finalized_bytes);
    }

    #[test]
    fn rejects_worker_attempt_and_payload_substitution() {
        let input = admission_fixture(0x41, 0);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x41, REQUIRED_GFX942_TEST_TARGET);
        let other_attempt = BuildAttempt::from_env_value(
            "999:42424242424242424242424242424242:4343434343434343434343434343434343434343434343434343434343434343",
        )
        .unwrap();
        assert!(matches!(
            admit_parts(
                other_attempt,
                &input.exact_bytes,
                input.publication,
                &validated,
                &input.fixture.container,
                selected_kernel,
                &observed,
            ),
            Err(FinalizedWorkerV2BundleAdmissionError::WorkerAttemptSubstitution)
        ));

        let input = admission_fixture(0x42, 0);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x42, REQUIRED_GFX942_TEST_TARGET);
        let mut substituted = input.exact_bytes.clone();
        substituted.push(0);
        assert!(matches!(
            admit_parts(
                input.attempt,
                &substituted,
                input.publication,
                &validated,
                &input.fixture.container,
                selected_kernel,
                &observed,
            ),
            Err(FinalizedWorkerV2BundleAdmissionError::WorkerPayloadSubstitution)
        ));
    }

    #[test]
    fn rejects_finalization_and_container_substitution() {
        let input = admission_fixture(0x51, 1);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x51, REQUIRED_GFX942_TEST_TARGET);
        assert!(matches!(
            admit_parts(
                input.attempt,
                &input.exact_bytes,
                input.publication,
                &validated,
                &input.fixture.container,
                selected_kernel,
                &observed,
            ),
            Err(FinalizedWorkerV2BundleAdmissionError::FinalizationIdentityMismatch)
        ));

        let input = admission_fixture(0x52, 0);
        let other = admission_fixture(0x53, 0);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&other.fixture);
        let observed = make_observed_for(0x52, REQUIRED_GFX942_TEST_TARGET);
        assert!(matches!(
            admit_parts(
                input.attempt,
                &input.exact_bytes,
                input.publication,
                &validated,
                &other.fixture.container,
                selected_kernel,
                &observed,
            ),
            Err(FinalizedWorkerV2BundleAdmissionError::ContainerSubstitution)
                | Err(FinalizedWorkerV2BundleAdmissionError::MissingFinalizedOccurrence)
        ));
    }

    #[test]
    fn rejects_target_kernel_abi_and_launch_substitution() {
        assert_physical_substitution_rejected(
            "gfx950",
            "primary_kernel",
            physical_test_abi(false),
            exact_launch(256),
        );
        assert_physical_substitution_rejected(
            "gfx942",
            "other_kernel",
            physical_test_abi(false),
            exact_launch(256),
        );
        assert_physical_substitution_rejected(
            "gfx942",
            "primary_kernel",
            AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap(),
            exact_launch(256),
        );
        assert_physical_substitution_rejected(
            "gfx942",
            "primary_kernel",
            physical_test_abi(false),
            exact_launch(64),
        );
    }

    fn assert_physical_substitution_rejected(
        architecture: &str,
        symbol: &str,
        abi: AbiLayout,
        launch: LaunchContract,
    ) {
        let input = admission_fixture(0x61, 0);
        let replacement = replacement_fixture(&input, architecture, symbol, abi, launch);
        let validated = replacement.validated();
        let selected = selected(&replacement);
        let observed = make_observed_for(0x61, architecture);
        assert!(matches!(
            admit_parts(
                input.attempt,
                &input.exact_bytes,
                input.publication,
                &validated,
                &replacement.container,
                selected,
                &observed,
            ),
            Err(FinalizedWorkerV2BundleAdmissionError::PhysicalInspection(_))
        ));
    }

    fn replacement_fixture(
        input: &AdmissionFixture,
        architecture: &str,
        symbol: &str,
        abi: AbiLayout,
        launch: LaunchContract,
    ) -> Fixture {
        let mut fixture =
            make_single_hsaco_fixture(0x61, input.exact_bytes.clone(), architecture, abi, launch);
        bind_worker_linked_output(&mut fixture, &input.exact_bytes);
        if symbol != "primary_kernel" {
            let payload = fe2o3_artifacts::CodeObjectPayload::from_bytes(
                fixture.container.digest_algorithm(),
                fixture.container.payloads()[0].bytes().to_vec(),
            )
            .unwrap();
            let payload_identity = payload.digest();
            let kernel = fe2o3_artifacts::KernelEntry::new(
                fixture.primary_kernel,
                fe2o3_artifacts::Name::new("logical_primary").unwrap(),
                fe2o3_artifacts::Name::new(symbol).unwrap(),
                fe2o3_artifacts::DigestBytes::from_bytes([0xa1; 32]),
                fe2o3_artifacts::DigestBytes::from_bytes([0xa2; 32]),
                payload_identity.bytes(),
                vec![],
                exact_launch(256),
                physical_test_abi(false),
            )
            .unwrap();
            let manifest = fe2o3_artifacts::ManifestV1::new(
                fixture.container.manifest().compiler().clone(),
                fixture.container.manifest().producer().clone(),
                fixture.container.manifest().target().clone(),
                fixture.container.manifest().code_objects().to_vec(),
                vec![kernel],
            )
            .unwrap();
            fixture.container =
                ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
            fixture.bundle = fe2o3_artifacts::BundleIndexV1::from_containers(std::slice::from_ref(
                &fixture.container,
            ))
            .unwrap();
            let sources = fixture
                .expectations
                .iter()
                .cloned()
                .map(|expectation| DirectLinkBindingSourceV1::new(&fixture.container, expectation))
                .collect::<Vec<_>>();
            fixture.evidence =
                DirectLinkBundleEvidenceV1::bind(&fixture.bundle, &[&fixture.container], &sources)
                    .unwrap();
        }
        fixture
    }

    #[test]
    fn finalization_identity_adapter_remains_structural() {
        let bytes = [0x91; 32];
        let identity = DirectLinkFinalizationIdentityV1::new(PayloadDigest::new(
            DigestAlgorithm::Sha256,
            fe2o3_artifacts::DigestBytes::from_bytes(bytes),
        ));
        assert_eq!(identity.digest().bytes().as_bytes(), &bytes);
        assert_eq!(
            MISSING_FINALIZED_WORKER_V2_LOAD_PREREQUISITES_V1,
            [
                MissingFinalizedWorkerV2LoadPrerequisiteV1::AuthenticatedCompilerAndVerusChain,
                MissingFinalizedWorkerV2LoadPrerequisiteV1::AuthenticatedRustMarkerAbiAndEffectsBinding,
                MissingFinalizedWorkerV2LoadPrerequisiteV1::AuthenticatedHsaModuleLifecycle,
            ]
        );
    }

    fn marker_function() {}

    macro_rules! typed_selection_marker {
        ($marker:ident, $logical:literal, $export:literal, $binding:expr) => {
            struct $marker;

            unsafe impl KernelMarkerV1 for $marker {
                type Function = fn();
                type Registration = ();

                const LOGICAL_NAME: &'static str = $logical;
                const EXPORT_NAME: &'static str = $export;
                const FUNCTION: Self::Function = marker_function;
                const REGISTRATION: &'static Self::Registration = &();
            }

            // SAFETY: The private fixture independently constructs the exact
            // profile and binding identity for this marker. The admitted
            // shared bundle supplies artifact transport separately.
            unsafe impl CompilerGeneratedKernelExpectationV1 for $marker {
                const PROFILE: CompilerGeneratedKernelProfileV1 =
                    CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2;
                const KERNEL_BINDING_ID_V1: [u8; 32] = $binding;
            }
        };
    }

    typed_selection_marker!(
        FirstTypedKernel,
        "logical_first",
        "first_kernel",
        [0x11; 32]
    );
    typed_selection_marker!(
        SecondTypedKernel,
        "logical_second",
        "second_kernel",
        [0x22; 32]
    );
    typed_selection_marker!(
        MissingTypedKernel,
        "logical_missing",
        "missing_kernel",
        [0x33; 32]
    );
    typed_selection_marker!(
        PartialNameTypedKernel,
        "logical_first",
        "second_kernel",
        [0x44; 32]
    );
    typed_selection_marker!(
        WrongBindingTypedKernel,
        "logical_second",
        "second_kernel",
        [0x55; 32]
    );

    #[derive(Clone, Copy)]
    struct SelectionSpec {
        logical: &'static str,
        export: &'static str,
        binding: [u8; 32],
    }

    impl SelectionSpec {
        const fn new(logical: &'static str, export: &'static str, binding: [u8; 32]) -> Self {
            Self {
                logical,
                export,
                binding,
            }
        }
    }

    fn selection_text(value: &str) -> IdentityText {
        IdentityText::new(value).unwrap()
    }

    fn selection_name(value: &str) -> Name {
        Name::new(value).unwrap()
    }

    fn selection_container(
        target: &str,
        payload_seed: u8,
        specs: &[SelectionSpec],
        wrong_effect_index: Option<usize>,
    ) -> ArtifactContainerV1 {
        let payload =
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, vec![payload_seed; 96]).unwrap();
        let payload_digest = payload.digest();
        let code_object = CodeObjectIdentity::new(
            payload_digest.bytes(),
            CodeObjectFormat::NativeExecutable,
            payload.bytes().len() as u64,
        )
        .unwrap();
        let launch = exact_launch(256);
        let kernels = specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let mut abi = crate::generated_vecadd::generated_vecadd_abi_v2().unwrap();
                if wrong_effect_index == Some(index) {
                    let mut fields = abi.fields().to_vec();
                    let output = &fields[2];
                    fields[2] = AbiField::new(
                        output.name().clone(),
                        output.offset(),
                        output.size(),
                        output.alignment(),
                        output.kind(),
                        output.mutability(),
                        Access::ReadWrite,
                        output.address_space(),
                        output.type_identity(),
                        output.ownership(),
                        output.alias_class(),
                    )
                    .unwrap();
                    abi = AbiLayout::new(abi.size(), abi.alignment(), abi.pointer_width(), fields)
                        .unwrap();
                }
                let source = repeated_digest(payload_seed.wrapping_add(0x20 + index as u8));
                let executable = repeated_digest(payload_seed.wrapping_add(0x40 + index as u8));
                let kernel_id = derive_generated_kernel_identity_v2(
                    TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
                    spec.binding,
                    spec.logical,
                    spec.export,
                    source,
                    executable,
                    &abi,
                    &launch,
                );
                KernelEntry::new(
                    kernel_id,
                    selection_name(spec.logical),
                    selection_name(spec.export),
                    source,
                    executable,
                    payload_digest.bytes(),
                    vec![],
                    launch.clone(),
                    abi,
                )
                .unwrap()
            })
            .collect();
        let manifest = ManifestV1::new(
            CompilerIdentity::new(selection_text("rustc"), selection_text("test")),
            ToolIdentity::new(selection_text("fe2o3"), selection_text("test")),
            TargetIdentity::new(
                selection_text("amdgcn-amd-amdhsa"),
                selection_text(target),
                PointerWidth::Bits64,
                Endianness::Little,
                vec![],
            )
            .unwrap(),
            vec![code_object],
            kernels,
        )
        .unwrap();
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap()
    }

    fn selection_identities(
        target: &str,
        payload_seed: u8,
        wrong_effect_index: Option<usize>,
    ) -> Box<[ArtifactKernelIdentityV1]> {
        let specs = [
            SelectionSpec::new("logical_first", "first_kernel", [0x11; 32]),
            SelectionSpec::new("logical_second", "second_kernel", [0x22; 32]),
        ];
        let container = selection_container(target, payload_seed, &specs, wrong_effect_index);
        let observed = make_observed_for(payload_seed.into(), target);
        container
            .manifest()
            .kernels()
            .iter()
            .map(|kernel| {
                let selected = container.select_native_kernel(kernel.kernel_id()).unwrap();
                ValidatedArtifactSelectionV1::validate(selected, &observed)
                    .unwrap()
                    .identity()
                    .clone()
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    #[test]
    fn selects_two_distinct_typed_markers_from_one_exact_payload() {
        let identities = selection_identities("gfx942", 0xb1, None);
        let target = identities[0].target();
        let finalized = identities[0].payload_digest();
        let first =
            select_typed_kernel_identity::<FirstTypedKernel>(&identities, target, finalized)
                .unwrap();
        let second =
            select_typed_kernel_identity::<SecondTypedKernel>(&identities, target, finalized)
                .unwrap();

        assert_ne!(first, second);
        assert_eq!(identities[first].symbol().as_str(), "first_kernel");
        assert_eq!(identities[second].symbol().as_str(), "second_kernel");
        assert_eq!(
            identities[first].payload_digest(),
            identities[second].payload_digest()
        );
    }

    #[test]
    fn typed_selection_rejects_name_binding_abi_effect_target_and_executable_substitution() {
        let identities = selection_identities("gfx942", 0xb2, None);
        let target = identities[0].target();
        let finalized = identities[0].payload_digest();
        assert_eq!(
            select_typed_kernel_identity::<MissingTypedKernel>(&identities, target, finalized),
            Err(WorkerV2TypedKernelSelectionError::KernelNotFound)
        );
        assert_eq!(
            select_typed_kernel_identity::<PartialNameTypedKernel>(&identities, target, finalized,),
            Err(WorkerV2TypedKernelSelectionError::NameSubstitution)
        );
        assert_eq!(
            select_typed_kernel_identity::<WrongBindingTypedKernel>(&identities, target, finalized,),
            Err(WorkerV2TypedKernelSelectionError::GeneratedProfile(
                GeneratedKernelProfileError::KernelIdentityMismatch,
            ))
        );

        let wrong_effects = selection_identities("gfx942", 0xb3, Some(1));
        assert_eq!(
            select_typed_kernel_identity::<SecondTypedKernel>(
                &wrong_effects,
                wrong_effects[0].target(),
                wrong_effects[0].payload_digest(),
            ),
            Err(WorkerV2TypedKernelSelectionError::GeneratedProfile(
                GeneratedKernelProfileError::AbiMismatch,
            ))
        );

        let wrong_target = selection_identities("gfx950", 0xb2, None);
        assert_eq!(
            select_typed_kernel_identity::<SecondTypedKernel>(
                &wrong_target,
                target,
                wrong_target[0].payload_digest(),
            ),
            Err(WorkerV2TypedKernelSelectionError::TargetSubstitution)
        );
        assert_eq!(
            select_typed_kernel_identity::<SecondTypedKernel>(
                &identities,
                target,
                wrong_effects[0].payload_digest(),
            ),
            Err(WorkerV2TypedKernelSelectionError::ExecutableSubstitution)
        );
    }
}
