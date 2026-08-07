use crate::published_direct_link::payload_kernel_set;
use crate::published_hsaco_inspection::inspect_payload_against_artifact_identity;
use crate::{
    ArtifactBindingError, ArtifactKernelIdentityV1, DeviceIdentity, ObservedContext,
    PublishedKernelPhysicalLayoutV1, PublishedPhysicalLayoutInspectionError,
    ValidatedArtifactSelectionV1,
};
use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationResultV1, BackendPublicationReceiptV1, BuildAttempt,
    DurableCurrentLinkPublicationLeaseV1, DurableCurrentLinkPublicationTokenV1,
    DurableLinkPublicationError, PublishedLinkArtifactV1,
};
use fe2o3_artifacts::{
    ArtifactContainerV1, DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DigestAlgorithm,
    DirectLinkBundleIndexIdentityV1, DirectLinkContainerIdentityV1,
    DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkLinkedOutputIdentityV1, PayloadDigest, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
};
use fe2o3_hsaco::{CodeObjectVersion, InspectedKernelBindings, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::PreparedWorkerV2HsacoPublicationV1;
use std::fmt;

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
    device: DeviceIdentity,
    inspected: InspectedKernelBindings,
    kernels: Box<[PublishedKernelPhysicalLayoutV1]>,
    selected_kernel_index: usize,
}

enum RetainedWorkerV2PreparationV1 {
    Production(Box<PreparedWorkerV2HsacoPublicationV1>),
    #[cfg(test)]
    Test {
        attempt: BuildAttempt,
        exact_bytes: Box<[u8]>,
    },
}

impl RetainedWorkerV2PreparationV1 {
    fn attempt(&self) -> BuildAttempt {
        match self {
            Self::Production(prepared) => prepared.attempt(),
            #[cfg(test)]
            Self::Test { attempt, .. } => *attempt,
        }
    }

    fn exact_bytes(&self) -> &[u8] {
        match self {
            Self::Production(prepared) => prepared.exact_bytes(),
            #[cfg(test)]
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

        Ok(Self {
            prepared: RetainedWorkerV2PreparationV1::Production(Box::new(prepared)),
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
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
        })
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
        let current = self
            .current_lease
            .acquire_current_token()
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
        Ok(CurrentFinalizedWorkerV2BundleAdmissionV1 {
            admission: self,
            _current: current,
        })
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
    device: DeviceIdentity,
    inspected: InspectedKernelBindings,
    kernels: Box<[PublishedKernelPhysicalLayoutV1]>,
    selected_kernel_index: usize,
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
        device: observed.device().clone(),
        inspected,
        kernels,
        selected_kernel_index,
    })
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::published_direct_link::tests::{
        Fixture, make_observed_for, make_single_hsaco_fixture, physical_arguments_hsaco_for_target,
        physical_test_abi,
    };
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
        KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
        PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
        UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
        publish_exact_hsaco_evidence_for_attempt_v1,
    };
    use fe2o3_artifacts::{
        AbiLayout, BlockSize, Dimensions, DirectLinkBindingExpectationV1,
        DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1, DirectLinkFinalizationIdentityV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkTransformationIdentityV1, LaunchContract,
        PointerWidth,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    pub(crate) struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-host-worker-v2-admission-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
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

    fn admission_fixture(seed: u8, plan_finalization_delta: u8) -> AdmissionFixture {
        admission_fixture_with_linked_bytes(seed, plan_finalization_delta, None)
    }

    fn admission_fixture_with_linked_bytes(
        seed: u8,
        plan_finalization_delta: u8,
        linked_bytes: Option<Vec<u8>>,
    ) -> AdmissionFixture {
        let hsaco = physical_arguments_hsaco_for_target(
            "gfx942",
            288,
            8,
            Some([256, 1, 1]),
            [None; 3],
            false,
        );
        let mut fixture = make_single_hsaco_fixture(
            seed,
            hsaco.bytes.clone(),
            "gfx942",
            physical_test_abi(false),
            exact_launch(256),
        );
        let linked_bytes = linked_bytes.unwrap_or_else(|| hsaco.bytes.clone());
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
        let output_digest = DigestAlgorithm::Sha256.calculate(&hsaco.bytes).bytes();
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
            &hsaco.bytes,
        )
        .unwrap();
        AdmissionFixture {
            _directory: directory,
            fixture,
            attempt,
            publication,
            exact_bytes: linked_bytes,
            finalized_bytes: hsaco.bytes,
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
        let observed = make_observed_for(seed.into(), "gfx942");
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
            device: parts.device,
            inspected: parts.inspected,
            kernels: parts.kernels,
            selected_kernel_index: parts.selected_kernel_index,
        };
        (admission, input._directory)
    }

    #[test]
    fn admits_exact_current_worker_publication_and_binds_all_host_identities() {
        let input = admission_fixture(0x31, 0);
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x31, "gfx942");
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
        assert_eq!(parts.inspected.inspection().target().to_string(), "gfx942");
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
        let mut linked_bytes = physical_arguments_hsaco_for_target(
            "gfx942",
            288,
            8,
            Some([256, 1, 1]),
            [None; 3],
            false,
        )
        .bytes;
        linked_bytes.push(0);
        let input = admission_fixture_with_linked_bytes(0x35, 0, Some(linked_bytes.clone()));
        let validated = input.fixture.validated();
        let selected_kernel = selected(&input.fixture);
        let observed = make_observed_for(0x35, "gfx942");
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
        let observed = make_observed_for(0x41, "gfx942");
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
        let observed = make_observed_for(0x42, "gfx942");
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
        let observed = make_observed_for(0x51, "gfx942");
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
        let observed = make_observed_for(0x52, "gfx942");
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
}
