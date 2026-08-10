use crate::{
    AdmittedFinalizedWorkerV2BundleV1, ArtifactKernelIdentityV1, AuthenticatedWorkerV2ExecutableV1,
    CompilerGeneratedAlphaZetaCov6ArgumentsV1, CompilerGeneratedKernelExpectationV1,
    DeviceIdentity, FinalizedWorkerV2BundleAdmissionError, GeneratedAlphaZetaCov6PrepareError,
    GeneratedAlphaZetaCov6PreparedInvocationV1, HsaExecutableLoadError, HsaLoadAuthorizationError,
    LoadedHsaExecutableV1, MissingFinalizedWorkerV2LoadPrerequisiteV1, ObservedContext,
    PhysicalMetadataValueV1, PublishedKernelPhysicalLayoutV1, ReviewedHsaImplicitKernargAdapterV1,
    UnloadedHsaExecutableV1, WorkerV2ExecutableAuthenticationError,
    WorkerV2PrerequisiteAuthenticatorV1, WorkerV2TypedKernelSelectionError,
};
use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationLeaseV1, DurablePublishedClaimReacquisitionErrorV1,
    PublishedLinkArtifactV1, reacquire_current_hsaco_publication_lease_v1,
};
use fe2o3_core::{GpuContext, Stream, StreamIdentity};
use fe2o3_hsaco::{CodeObjectVersion, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::{FinalizationError, finalize_unfinalized, verify_finalized};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion as DescriptorCodeObjectVersion, KernelDescriptorV1, KernelId,
};
use fe2o3_worker_v2_bundle::{EnvelopeDecodeError, WorkerV2LoadEnvelopeV1};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// Read-only host descriptor recovered from one canonical Worker V2 envelope.
///
/// The value owns the fresh process-local exact-file lease and the complete decoded envelope,
/// but exposes neither. Public accessors return only inert identity and descriptor metadata. It is
/// intentionally neither `Clone` nor `Copy` and has no module-load, launch, or prerequisite-
/// authentication transition.
pub struct RecoveredWorkerV2PinnedDescriptorV1 {
    admission: AdmittedFinalizedWorkerV2BundleV1,
    descriptor: KernelDescriptorV1,
    observed: ObservedContext,
}

impl fmt::Debug for RecoveredWorkerV2PinnedDescriptorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV2PinnedDescriptorV1")
            .field("published", &self.admission.published())
            .field("descriptor", &self.descriptor)
            .field("artifact_identity", self.admission.artifact_identity())
            .field("target", &self.admission.target())
            .field("code_object_version", &self.admission.code_object_version())
            .finish_non_exhaustive()
    }
}

impl RecoveredWorkerV2PinnedDescriptorV1 {
    /// Decodes and admits one exact envelope against its durable output directory.
    pub fn recover(
        output_dir: &Path,
        envelope_bytes: &[u8],
        kernel_id: KernelId,
        observed: &ObservedContext,
    ) -> Result<Self, RecoveredWorkerV2AdmissionError> {
        let envelope = WorkerV2LoadEnvelopeV1::from_bytes(envelope_bytes)
            .map_err(RecoveredWorkerV2AdmissionError::Decode)?;
        let descriptor_code_object_version =
            envelope.descriptor_lineage().table().code_object_version();
        let descriptor = select_descriptor(&envelope, kernel_id)?;
        let current_lease =
            reacquire_current_hsaco_publication_lease_v1(output_dir, envelope.published_claim())
                .map_err(RecoveredWorkerV2AdmissionError::Publication)?;
        validate_raw_final_lineage(&envelope, &current_lease)?;
        let admission = AdmittedFinalizedWorkerV2BundleV1::admit_recovered(
            envelope,
            current_lease,
            kernel_id,
            observed,
        )
        .map_err(RecoveredWorkerV2AdmissionError::Admission)?;
        validate_descriptor_against_physical(
            &descriptor,
            descriptor_code_object_version,
            &admission,
        )?;

        Ok(Self {
            admission,
            descriptor,
            observed: observed.clone(),
        })
    }

    pub const fn published(&self) -> PublishedLinkArtifactV1 {
        self.admission.published()
    }

    /// Returns the inert canonical descriptor metadata selected from the envelope.
    pub const fn descriptor(&self) -> &KernelDescriptorV1 {
        &self.descriptor
    }

    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        self.admission.artifact_identity()
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.admission.device()
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.admission.target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.admission.code_object_version()
    }

    pub fn physical_kernel(&self) -> &PublishedKernelPhysicalLayoutV1 {
        self.admission.selected_kernel()
    }

    pub fn descriptor_binding(&self) -> KernelDescriptorBinding {
        self.admission.selected_descriptor_binding()
    }

    pub const fn missing_prerequisites(
        &self,
    ) -> &'static [MissingFinalizedWorkerV2LoadPrerequisiteV1] {
        self.admission.missing_prerequisites()
    }

    /// Revalidates the current durable generation and exact pinned file occurrence.
    pub fn revalidate_currentness(&self) -> Result<(), FinalizedWorkerV2BundleAdmissionError> {
        let current = self.admission.acquire_currentness()?;
        drop(current);
        Ok(())
    }

    pub const fn authenticates_prerequisites(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Revalidates and consumes this descriptor into one exact application handoff.
    ///
    /// The descriptor remains inert until this transition is called with the existing unsafe
    /// compiler/Verus prerequisite authenticator. The returned authority owns the exact recovered
    /// envelope and publication lease through the loaded executable, retains the observed HIP
    /// context and the exact borrowed application stream, and exposes only generated typed
    /// preparation. Native bytes and handles remain private.
    pub fn load_generated_application_handoff_v1<'stream, K, Authenticator, Adapter>(
        self,
        stream: &'stream Stream,
        authenticator: &mut Authenticator,
        adapter: Adapter,
    ) -> Result<
        RecoveredWorkerV2ApplicationHandoffV1<'stream, K, Adapter>,
        RecoveredWorkerV2ApplicationHandoffError<Authenticator::Error, Adapter::Error>,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        Authenticator: WorkerV2PrerequisiteAuthenticatorV1<K>,
        Adapter: ReviewedHsaImplicitKernargAdapterV1,
    {
        let retained = RetainedApplicationStreamV1::bind(&self.observed, stream)
            .map_err(RecoveredWorkerV2ApplicationHandoffError::Binding)?;
        self.finish_generated_application_handoff_v1::<K, Authenticator, Adapter>(
            retained,
            authenticator,
            adapter,
        )
    }

    fn finish_generated_application_handoff_v1<'stream, K, Authenticator, Adapter>(
        self,
        retained: RetainedApplicationStreamV1<'stream>,
        authenticator: &mut Authenticator,
        adapter: Adapter,
    ) -> Result<
        RecoveredWorkerV2ApplicationHandoffV1<'stream, K, Adapter>,
        RecoveredWorkerV2ApplicationHandoffError<Authenticator::Error, Adapter::Error>,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        Authenticator: WorkerV2PrerequisiteAuthenticatorV1<K>,
        Adapter: ReviewedHsaImplicitKernargAdapterV1,
    {
        self.admission
            .acquire_currentness()
            .map_err(RecoveredWorkerV2ApplicationHandoffError::CurrentPublication)?;
        self.admission
            .select_typed_kernel::<K>()
            .map_err(RecoveredWorkerV2ApplicationHandoffError::Selection)?;
        let authenticated =
            AuthenticatedWorkerV2ExecutableV1::<K>::authenticate(self.admission, authenticator)
                .map_err(RecoveredWorkerV2ApplicationHandoffError::Authentication)?;
        let authorized = authenticated
            .authorize_hsa_load(adapter)
            .map_err(RecoveredWorkerV2ApplicationHandoffError::Authorization)?;
        let loaded = authorized
            .load()
            .map_err(RecoveredWorkerV2ApplicationHandoffError::Load)?;
        Ok(RecoveredWorkerV2ApplicationHandoffV1 {
            loaded,
            observed: self.observed,
            retained,
        })
    }
}

/// Linear application authority recovered from one canonical Worker V2 envelope.
///
/// This value is intentionally neither `Clone` nor `Copy`. It retains the exact publication
/// lease, envelope bytes, loaded executable, original context observation, and borrowed stream.
/// Preparation accepts the bound stream again so a separately created wrapper cannot be
/// substituted at the application boundary.
pub struct RecoveredWorkerV2ApplicationHandoffV1<'stream, K, A: ReviewedHsaImplicitKernargAdapterV1>
{
    loaded: LoadedHsaExecutableV1<K, A>,
    observed: ObservedContext,
    retained: RetainedApplicationStreamV1<'stream>,
}

/// Result of preparing one invocation through recovered application authority.
#[doc(hidden)]
pub type RecoveredWorkerV2ApplicationPrepareResultV1<
    'loaded,
    'allocation,
    Root,
    Selected,
    Adapter,
    Arguments,
    PrerequisiteError,
> = Result<
    GeneratedAlphaZetaCov6PreparedInvocationV1<
        'loaded,
        'allocation,
        Root,
        Selected,
        Adapter,
        Arguments,
    >,
    RecoveredWorkerV2ApplicationPrepareError<
        PrerequisiteError,
        <Adapter as crate::ReviewedHsaExecutableLifecycleAdapterV1>::Error,
    >,
>;

impl<K, A: ReviewedHsaImplicitKernargAdapterV1> fmt::Debug
    for RecoveredWorkerV2ApplicationHandoffV1<'_, K, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredWorkerV2ApplicationHandoffV1")
            .field("load", self.loaded.load_observation())
            .field("kernel", self.loaded.kernel_observation())
            .field("stream", &self.retained.identity())
            .finish_non_exhaustive()
    }
}

impl<K, A: ReviewedHsaImplicitKernargAdapterV1> RecoveredWorkerV2ApplicationHandoffV1<'_, K, A> {
    pub const fn stream_identity(&self) -> StreamIdentity {
        self.retained.identity()
    }

    pub const fn load_observation(&self) -> &crate::HsaCodeObjectLoadObservationV1 {
        self.loaded.load_observation()
    }

    pub const fn kernel_observation(&self) -> &crate::HsaKernelResolutionObservationV1 {
        self.loaded.kernel_observation()
    }

    /// Prepares one generated alpha/zeta COV6 invocation on the exact bound stream.
    ///
    /// The HSA dispatch is synchronous; retaining the stream prevents its surrounding HIP
    /// lifetime from ending, while the generated argument capabilities enforce exact context and
    /// allocation provenance. Supplying another stream wrapper fails before authentication or
    /// argument binding.
    #[doc(hidden)]
    pub fn prepare_generated_alpha_zeta_cov6_v1<
        'loaded,
        'allocation,
        Selected,
        Authenticator,
        Arguments,
    >(
        &'loaded mut self,
        stream: &Stream,
        authenticator: &mut Authenticator,
        arguments: Arguments,
    ) -> RecoveredWorkerV2ApplicationPrepareResultV1<
        'loaded,
        'allocation,
        K,
        Selected,
        A,
        Arguments,
        Authenticator::Error,
    >
    where
        Selected: CompilerGeneratedKernelExpectationV1,
        Authenticator: WorkerV2PrerequisiteAuthenticatorV1<Selected>,
        Arguments: CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, Selected>,
    {
        if self.retained.identity() != stream.identity() {
            return Err(
                RecoveredWorkerV2ApplicationPrepareError::StreamSubstitution {
                    expected: self.retained.identity(),
                    actual: stream.identity(),
                },
            );
        }
        self.loaded
            .prepare_generated_alpha_zeta_cov6_selected_kernel_v1::<
                Selected,
                Authenticator,
                Arguments,
            >(&self.observed, authenticator, arguments)
            .map_err(RecoveredWorkerV2ApplicationPrepareError::Prepare)
    }

    pub fn unload(
        self,
    ) -> Result<UnloadedHsaExecutableV1, crate::HsaExecutableUnloadError<A::Error>> {
        let Self {
            loaded,
            observed,
            retained,
        } = self;
        let _lifetime_guard = (observed, retained);
        loaded.unload()
    }
}

enum RetainedApplicationStreamV1<'stream> {
    Production {
        stream: &'stream Stream,
        context: Arc<GpuContext>,
    },
    #[cfg(test)]
    Test,
}

impl<'stream> RetainedApplicationStreamV1<'stream> {
    fn bind(
        observed: &ObservedContext,
        stream: &'stream Stream,
    ) -> Result<Self, RecoveredWorkerV2ApplicationBindingError> {
        if !observed.is_for_context(stream.context()) {
            return Err(RecoveredWorkerV2ApplicationBindingError::ContextSubstitution);
        }
        Ok(Self::Production {
            stream,
            context: stream.context().clone(),
        })
    }

    const fn identity(&self) -> StreamIdentity {
        match self {
            Self::Production { stream, .. } => stream.identity(),
            #[cfg(test)]
            Self::Test => panic!("test stream retention has no production identity"),
        }
    }
}

impl Drop for RetainedApplicationStreamV1<'_> {
    fn drop(&mut self) {
        match self {
            Self::Production { stream, context } => {
                debug_assert!(Arc::ptr_eq(context, stream.context()));
            }
            #[cfg(test)]
            Self::Test => {}
        }
    }
}

/// Context/stream mismatch before recovered authority can be authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RecoveredWorkerV2ApplicationBindingError {
    ContextSubstitution,
}

/// Failure while converting a recovered envelope into application authority.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2ApplicationHandoffError<PrerequisiteError, AdapterError> {
    Binding(RecoveredWorkerV2ApplicationBindingError),
    CurrentPublication(FinalizedWorkerV2BundleAdmissionError),
    Selection(WorkerV2TypedKernelSelectionError),
    Authentication(WorkerV2ExecutableAuthenticationError<PrerequisiteError>),
    Authorization(HsaLoadAuthorizationError<AdapterError>),
    Load(HsaExecutableLoadError<AdapterError>),
}

/// Failure while preparing through recovered application authority.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2ApplicationPrepareError<PrerequisiteError, AdapterError> {
    StreamSubstitution {
        expected: StreamIdentity,
        actual: StreamIdentity,
    },
    Prepare(GeneratedAlphaZetaCov6PrepareError<PrerequisiteError, AdapterError>),
}

fn validate_raw_final_lineage(
    envelope: &WorkerV2LoadEnvelopeV1,
    current_lease: &DurableCurrentLinkPublicationLeaseV1,
) -> Result<(), RecoveredWorkerV2AdmissionError> {
    let replayed = finalize_unfinalized(envelope.raw_hsaco().bytes())
        .map_err(RecoveredWorkerV2AdmissionError::RawFinalization)?;
    let pinned = current_lease.exact_artifact_bytes();
    if replayed.as_bytes() != pinned || envelope.finalized_payload() != pinned {
        return Err(RecoveredWorkerV2AdmissionError::RawFinalizedPayloadMismatch);
    }
    let verified =
        verify_finalized(pinned).map_err(RecoveredWorkerV2AdmissionError::FinalizedVerification)?;
    if replayed.inspection() != &verified {
        return Err(RecoveredWorkerV2AdmissionError::RawFinalizedInspectionMismatch);
    }
    if verified.descriptor_table() != envelope.descriptor_lineage().table() {
        return Err(RecoveredWorkerV2AdmissionError::DescriptorLineageMismatch);
    }
    Ok(())
}

/// Recovers one read-only descriptor without exposing the envelope's HSACO bytes.
pub fn recover_worker_v2_load_envelope_v1(
    output_dir: &Path,
    envelope_bytes: &[u8],
    kernel_id: KernelId,
    observed: &ObservedContext,
) -> Result<RecoveredWorkerV2PinnedDescriptorV1, RecoveredWorkerV2AdmissionError> {
    RecoveredWorkerV2PinnedDescriptorV1::recover(output_dir, envelope_bytes, kernel_id, observed)
}

fn select_descriptor(
    envelope: &WorkerV2LoadEnvelopeV1,
    kernel_id: KernelId,
) -> Result<KernelDescriptorV1, RecoveredWorkerV2AdmissionError> {
    let mut matches = envelope
        .descriptor_lineage()
        .table()
        .kernels()
        .iter()
        .filter(|descriptor| descriptor.kernel_id() == kernel_id);
    let descriptor = matches
        .next()
        .ok_or(RecoveredWorkerV2AdmissionError::KernelNotFound)?;
    if matches.next().is_some() {
        return Err(RecoveredWorkerV2AdmissionError::AmbiguousKernel);
    }
    Ok(descriptor.clone())
}

fn validate_descriptor_against_physical(
    descriptor: &KernelDescriptorV1,
    descriptor_version: DescriptorCodeObjectVersion,
    admission: &AdmittedFinalizedWorkerV2BundleV1,
) -> Result<(), RecoveredWorkerV2AdmissionError> {
    let physical = admission.selected_kernel();
    let declared_abi = descriptor.abi_layout();
    let physical_launch = physical.launch();
    let checks = [
        (
            descriptor.entry_name().as_str() == physical.export_symbol(),
            "entry symbol",
        ),
        (
            descriptor.descriptor_symbol().as_str() == physical.descriptor_symbol(),
            "descriptor symbol",
        ),
        (
            descriptor_version == descriptor_code_object_version(admission.code_object_version()),
            "code-object version",
        ),
        (
            u64::from(declared_abi.kernarg_segment_size())
                == physical_launch.kernarg_segment_size(),
            "kernarg segment size",
        ),
        (
            u64::from(declared_abi.kernarg_segment_alignment())
                == physical_launch.kernarg_segment_alignment(),
            "kernarg segment alignment",
        ),
        (
            descriptor.launch().max_flat_workgroup_size()
                == physical_launch.max_flat_workgroup_size(),
            "maximum flat workgroup size",
        ),
        (
            u64::from(descriptor.launch().static_shared_memory_bytes())
                == physical_launch.group_segment_fixed_size(),
            "fixed group segment size",
        ),
    ];
    for (matches, field) in checks {
        if !matches {
            return Err(RecoveredWorkerV2AdmissionError::DescriptorPhysicalMismatch { field });
        }
    }
    if let PhysicalMetadataValueV1::Known(required) = physical_launch.required_workgroup_size() {
        let declared = match descriptor.launch().block_size() {
            fe2o3_kernel_descriptor::BlockSizeV1::Exact(dimensions) => {
                [dimensions.x(), dimensions.y(), dimensions.z()]
            }
            fe2o3_kernel_descriptor::BlockSizeV1::Any
            | fe2o3_kernel_descriptor::BlockSizeV1::AtMost(_) => {
                return Err(
                    RecoveredWorkerV2AdmissionError::DescriptorPhysicalMismatch {
                        field: "required workgroup size",
                    },
                );
            }
        };
        if declared != required {
            return Err(
                RecoveredWorkerV2AdmissionError::DescriptorPhysicalMismatch {
                    field: "required workgroup size",
                },
            );
        }
    }
    Ok(())
}

const fn descriptor_code_object_version(version: CodeObjectVersion) -> DescriptorCodeObjectVersion {
    match version {
        CodeObjectVersion::V4 => DescriptorCodeObjectVersion::V4,
        CodeObjectVersion::V5 => DescriptorCodeObjectVersion::V5,
        CodeObjectVersion::V6 => DescriptorCodeObjectVersion::V6,
    }
}

/// Failure while recovering a canonical Worker V2 host descriptor.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2AdmissionError {
    Decode(EnvelopeDecodeError),
    Publication(DurablePublishedClaimReacquisitionErrorV1),
    RawFinalization(FinalizationError),
    FinalizedVerification(FinalizationError),
    RawFinalizedPayloadMismatch,
    RawFinalizedInspectionMismatch,
    DescriptorLineageMismatch,
    KernelNotFound,
    AmbiguousKernel,
    Admission(FinalizedWorkerV2BundleAdmissionError),
    DescriptorPhysicalMismatch { field: &'static str },
}

impl fmt::Display for RecoveredWorkerV2AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(formatter, "invalid Worker V2 envelope: {error}"),
            Self::Publication(error) => {
                write!(formatter, "cannot reacquire Worker V2 publication: {error}")
            }
            Self::RawFinalization(error) => {
                write!(
                    formatter,
                    "cannot replay Worker V2 HSACO finalization: {error}"
                )
            }
            Self::FinalizedVerification(error) => {
                write!(
                    formatter,
                    "cannot verify finalized Worker V2 HSACO: {error}"
                )
            }
            Self::RawFinalizedPayloadMismatch => formatter.write_str(
                "replayed Worker V2 finalization differs from the pinned finalized payload",
            ),
            Self::RawFinalizedInspectionMismatch => formatter
                .write_str("replayed and independently verified Worker V2 finalization differ"),
            Self::DescriptorLineageMismatch => formatter.write_str(
                "Worker V2 envelope descriptor differs from the pinned embedded descriptor",
            ),
            Self::KernelNotFound => {
                formatter.write_str("requested kernel is absent from the Worker V2 descriptor")
            }
            Self::AmbiguousKernel => formatter
                .write_str("requested kernel occurs more than once in the Worker V2 descriptor"),
            Self::Admission(error) => write!(formatter, "Worker V2 host admission failed: {error}"),
            Self::DescriptorPhysicalMismatch { field } => {
                write!(
                    formatter,
                    "Worker V2 descriptor {field} differs from the physical HSACO"
                )
            }
        }
    }
}

impl Error for RecoveredWorkerV2AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::RawFinalization(error) | Self::FinalizedVerification(error) => Some(error),
            Self::Admission(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::published_direct_link::tests::{
        Fixture, make_observed_for, make_single_hsaco_fixture_with_names_and_kernel_id,
    };
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, DurableLinkPublicationPlanV1, PackageIdentityV1,
        ProducerIdentity, UpstreamCodeObjectEvidenceIdentityV1, begin_build_attempt,
        fail_build_attempt, publish_exact_hsaco_evidence_for_attempt_v1,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        BlockSize, CallerClaimedPackageIdentityV1, DeclaredRustLayoutIdentity,
        DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions,
        DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkTransformationIdentityV1,
        ManifestClaimDerivedLinkPublicationScopeV1, ManifestClaimDirectLinkPublicationBridgeV1,
        MeasuredToolIdentity, Mutability, Name, PayloadDigest, PointerWidth, ProofArtifactIdentity,
        ProofExecutionIdentity, ProofOutcome, ProofProperty, ProofRecordV1, ProofTargetIdentity,
        SourceContractIdentity, TypeIdentity, VerificationModelIdentity,
        derive_generated_host_contract_identity_v1, derive_generated_kernel_identity_v2,
    };
    use fe2o3_device::KernelMarkerV1;
    use fe2o3_kernel_descriptor::{
        BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CompilerIdentityV1,
        DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
        DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
        LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
        encode_device_descriptor_table_v1,
    };
    use fe2o3_worker_v2_bundle::{DescriptorLineageV1, ExactRawHsacoV1};
    use reserved_fe2o3_symbols::{
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
    };
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[allow(dead_code)]
    mod canonical_hsaco_fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fe2o3-hsaco-finalize/tests/fixtures/worker_v2_hsaco_test_support.rs"
        ));

        pub(super) fn with_descriptor_table(target: &str, table: &[u8]) -> Vec<u8> {
            let mut options = FixtureOptions::valid();
            options.target = target;
            fixture_with_descriptor_table(options, Some(table)).bytes
        }
    }

    const ARTIFACT_PREFIX: &str = ".fe2o3-link-artifact-v1-";
    const ARTIFACT_SUFFIX: &str = ".bin";
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-recovered-worker-v2-admission-{}-{}",
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

    struct RecoveryFixture {
        _directory: TestDirectory,
        output: std::path::PathBuf,
        owner: ProducerIdentity,
        attempt: fe2o3_artifact_transaction::BuildAttempt,
        envelope: Vec<u8>,
        kernel_id: KernelId,
        observed: ObservedContext,
    }

    fn digest(seed: u8) -> DigestBytes {
        DigestBytes::from_bytes([seed; 32])
    }

    fn tagged(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, digest(seed))
    }

    fn identity_text(value: &str) -> fe2o3_artifacts::IdentityText {
        fe2o3_artifacts::IdentityText::new(value).unwrap()
    }

    fn descriptor_text(value: &str) -> Text {
        Text::new(value).unwrap()
    }

    fn descriptor_name(value: &str) -> ValidName {
        ValidName::new(value).unwrap()
    }

    fn launch() -> fe2o3_artifacts::LaunchContract {
        fe2o3_artifacts::LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    fn descriptor_launch() -> LaunchConstraintsV1 {
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(65_535, 1, 1).unwrap(),
            256,
            0,
            0,
        )
        .unwrap()
    }

    fn evidence(identity: u8, digest: DigestBytes) -> BuildEvidenceV1 {
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([identity; 32]),
            EvidenceDigest::from_sha256_bytes(*digest.as_bytes()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn descriptor_table(
        kernel_id: DigestBytes,
        logical_name: &str,
        entry_name: &str,
        source_digest: DigestBytes,
        executable_digest: DigestBytes,
        target: &str,
        canonical_digest: [u8; 32],
    ) -> DeviceDescriptorTableV1 {
        let shared_source =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let shared_layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let descriptor = KernelDescriptorV1::new(
            KernelId::from_bytes(*kernel_id.as_bytes()),
            descriptor_name(logical_name),
            descriptor_name(entry_name),
            descriptor_name(&format!("{entry_name}.kd")),
            evidence(0x31, source_digest),
            evidence(0x32, executable_digest),
            vec![],
            KernelAbiLayoutV1::new(16, 272, 8).unwrap(),
            descriptor_launch(),
            vec![
                LogicalArgumentV1::shared_slice(
                    0,
                    descriptor_name("values"),
                    &shared_source,
                    &shared_layout,
                    0,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes(canonical_digest),
            DescriptorCodeObjectVersion::V6,
            CompilerIdentityV1::new(
                descriptor_text("rustc-codegen-fe2o3"),
                descriptor_text("test"),
                [0x41; 20],
            ),
            ProducerIdentityV1::new(
                descriptor_text("rustc-codegen-fe2o3"),
                descriptor_text("test"),
            ),
            DeviceTargetV1::parse(target).unwrap(),
            vec![shared_source],
            vec![shared_layout],
            vec![descriptor],
        )
        .unwrap()
    }

    fn manifest_abi() -> AbiLayout {
        AbiLayout::new(
            16,
            8,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    Name::new("values").unwrap(),
                    0,
                    16,
                    8,
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4,
                    },
                    Mutability::Immutable,
                    Access::ReadOnly,
                    AddressSpace::Global,
                    TypeIdentity::new(
                        DeclaredRustTypeIdentity::from_untrusted_bytes(digest(0xa1)),
                        DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(0xa2)),
                    ),
                    ArgumentOwnership::SharedBorrow,
                    AliasClass::SharedReadOnly,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn proof(
        kernel_id: DigestBytes,
        source: DigestBytes,
        executable: DigestBytes,
    ) -> ProofRecordV1 {
        let artifact = ProofArtifactIdentity::new(
            PayloadDigest::new(DigestAlgorithm::Sha256, kernel_id),
            tagged(0x51),
            PayloadDigest::new(DigestAlgorithm::Sha256, source),
            tagged(0x53),
            PayloadDigest::new(DigestAlgorithm::Sha256, executable),
            tagged(0x55),
            tagged(0x56),
            tagged(0x57),
        );
        let contracts = SourceContractIdentity::new(
            tagged(0x61),
            tagged(0x62),
            tagged(0x63),
            tagged(0x64),
            tagged(0x65),
        );
        let tool = |name: &str, seed: u8| {
            MeasuredToolIdentity::new(
                identity_text(name),
                identity_text("test"),
                tagged(seed),
                tagged(seed.wrapping_add(1)),
            )
        };
        ProofRecordV1::new(
            ProofTargetIdentity::new(artifact, contracts),
            vec![],
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(identity_text("model"), tagged(0x70)),
                tool("verus", 0x71),
                tool("solver", 0x73),
                tool("recorder", 0x75),
                tagged(0x77),
            ),
            ProofOutcome::Proved,
            vec![ProofProperty::Bounds],
            vec![],
        )
        .unwrap()
    }

    fn bind_raw_hsaco(fixture: &mut Fixture, raw: &[u8]) {
        let previous = fixture.expectations[0].clone();
        fixture.expectations[0] = DirectLinkBindingExpectationV1::new(
            previous.request_identity(),
            previous.worker().clone(),
            previous.toolchain().clone(),
            previous.response_identity(),
            DirectLinkTransformationIdentityV1::new(
                DirectLinkLinkedOutputIdentityV1::new(DigestAlgorithm::Sha256.calculate(raw)),
                previous.finalization_identity(),
                previous.finalized_payload_identity(),
            ),
            previous.ffi_contract_identity(),
        );
        let source =
            DirectLinkBindingSourceV1::new(&fixture.container, fixture.expectations[0].clone());
        fixture.evidence =
            DirectLinkBundleEvidenceV1::bind(&fixture.bundle, &[&fixture.container], &[source])
                .unwrap();
    }

    fn recovery_fixture(seed: u8, raw_target: &str, manifest_symbol: &str) -> RecoveryFixture {
        let source_digest = digest(seed.wrapping_add(0x40));
        let executable_digest = digest(seed.wrapping_add(0x50));
        let abi = manifest_abi();
        let launch = launch();
        let kernel_id = derive_generated_kernel_identity_v2(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            HANDOFF_MARKER_BINDING,
            "logical_primary",
            manifest_symbol,
            source_digest,
            executable_digest,
            &abi,
            &launch,
        );
        let final_raw_table = descriptor_table(
            kernel_id,
            "logical_primary",
            "vecadd",
            source_digest,
            executable_digest,
            "gfx942",
            [0; 32],
        );
        let final_raw = canonical_hsaco_fixture::with_descriptor_table(
            "gfx942",
            &encode_device_descriptor_table_v1(&final_raw_table).unwrap(),
        );
        let finalized_hsaco = finalize_unfinalized(&final_raw).unwrap();
        let embedded_descriptor = finalized_hsaco.inspection().descriptor_table().clone();
        let canonical_digest = *finalized_hsaco.inspection().digest().as_bytes();
        let finalized = finalized_hsaco.into_bytes();
        let raw = if raw_target == "gfx942" {
            final_raw
        } else {
            let substituted_raw_table = descriptor_table(
                kernel_id,
                "logical_primary",
                "vecadd",
                source_digest,
                executable_digest,
                raw_target,
                [0; 32],
            );
            canonical_hsaco_fixture::with_descriptor_table(
                raw_target,
                &encode_device_descriptor_table_v1(&substituted_raw_table).unwrap(),
            )
        };
        let mut fixture = make_single_hsaco_fixture_with_names_and_kernel_id(
            seed,
            finalized.clone(),
            "gfx942",
            "logical_primary",
            manifest_symbol,
            abi,
            launch,
            kernel_id,
        );
        bind_raw_hsaco(&mut fixture, &raw);
        let descriptor = if manifest_symbol == "vecadd" {
            embedded_descriptor
        } else {
            descriptor_table(
                kernel_id,
                "logical_primary",
                manifest_symbol,
                source_digest,
                executable_digest,
                "gfx942",
                canonical_digest,
            )
        };
        let kernel = &fixture.container.manifest().kernels()[0];
        let proof = proof(
            kernel.kernel_id(),
            kernel.source_digest(),
            kernel.executable_digest(),
        );

        let directory = TestDirectory::new();
        let output = directory.0.join("output");
        let owner = ProducerIdentity::from_codegen(
            "fe2o3_recovered_worker_v2_admission",
            Some(Path::new("tests/recovered_worker_v2_admission.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &output,
            &owner,
            BuildInvocation::from_bytes([seed.wrapping_add(1); 32]),
            BuildSession::from_bytes([seed.wrapping_add(2); 16]),
        )
        .unwrap();
        let validated = fixture.validated();
        let scope = ManifestClaimDerivedLinkPublicationScopeV1::derive(
            CallerClaimedPackageIdentityV1::new(PackageIdentityV1::from_bytes(
                [seed.wrapping_add(3); 32],
            )),
            &validated,
            0,
            &fixture.container,
        )
        .unwrap();
        let bridge = ManifestClaimDirectLinkPublicationBridgeV1::prepare_with_manifest_claim_scope(
            attempt, scope, &validated, 0,
        )
        .unwrap();
        let descriptive_scope = bridge
            .non_authoritative_diagnostics()
            .descriptive_scope_claim();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            descriptive_scope,
            bridge.request_identity(),
            bridge.worker_identity(),
            bridge.response_identity(),
            bridge.linked_output_identity(),
            bridge.finalization_identity(),
            bridge.finalized_output_identity(),
            bridge.publication_identity(),
        );
        let evidence_identity = fixture
            .evidence
            .digest(fe2o3_artifacts::DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM);
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &output,
            &owner,
            attempt,
            plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes(*evidence_identity.bytes().as_bytes()),
            &finalized,
        )
        .unwrap();
        let claim = publication.published_claim().clone();
        drop(publication);
        let envelope = WorkerV2LoadEnvelopeV1::new(
            fixture.container,
            fixture.bundle,
            fixture.evidence,
            DescriptorLineageV1::new(descriptor),
            vec![proof],
            ExactRawHsacoV1::from_bytes(raw).unwrap(),
            claim,
        )
        .unwrap()
        .to_bytes();
        RecoveryFixture {
            _directory: directory,
            output,
            owner,
            attempt,
            envelope,
            kernel_id: KernelId::from_bytes(*kernel_id.as_bytes()),
            observed: make_observed_for(usize::from(seed), "gfx942"),
        }
    }

    const HANDOFF_MARKER_BINDING: [u8; 32] = [0x4b; 32];
    const HANDOFF_HOST_CONTRACT: [u8; 32] = [
        164, 4, 156, 183, 6, 194, 68, 206, 62, 75, 94, 12, 225, 132, 34, 167, 151, 17, 98, 253,
        137, 47, 10, 13, 246, 241, 18, 73, 51, 167, 69, 16,
    ];

    fn handoff_kernel() {}

    struct HandoffKernel;

    unsafe impl KernelMarkerV1 for HandoffKernel {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "logical_primary";
        const EXPORT_NAME: &'static str = "vecadd";
        const FUNCTION: Self::Function = handoff_kernel;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl CompilerGeneratedKernelExpectationV1 for HandoffKernel {
        const PROFILE: crate::CompilerGeneratedKernelProfileV1 =
            crate::CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
                generated_host_contract_identity: HANDOFF_HOST_CONTRACT,
            };
        const KERNEL_BINDING_ID_V1: [u8; 32] = HANDOFF_MARKER_BINDING;

        fn semantic_witness_v1() -> Result<
            crate::ValidatedCompilerGeneratedSemanticWitnessV1,
            crate::CompilerGeneratedSemanticWitnessErrorV1,
        > {
            let bytes = handoff_semantic_witness_bytes();
            // SAFETY: the immutable vector remains live for the complete parser call and contains
            // the exact fixture identities declared above.
            unsafe {
                crate::semantic_witness_from_backend_v1(
                    bytes.as_ptr(),
                    bytes.len(),
                    HANDOFF_MARKER_BINDING,
                    HANDOFF_HOST_CONTRACT,
                )
            }
        }
    }

    fn handoff_semantic_witness_bytes() -> Vec<u8> {
        let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
        let length = reserved_fe2o3_symbols::GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
            + profile.len();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(
            &reserved_fe2o3_symbols::GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes(),
        );
        bytes.extend_from_slice(
            &reserved_fe2o3_symbols::GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes(),
        );
        bytes.extend_from_slice(
            &reserved_fe2o3_symbols::GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes(),
        );
        bytes.extend_from_slice(&u32::try_from(length).unwrap().to_le_bytes());
        bytes.extend_from_slice(&HANDOFF_MARKER_BINDING);
        bytes.extend_from_slice(&HANDOFF_HOST_CONTRACT);
        bytes.extend_from_slice(&u16::try_from(profile.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(profile);
        bytes
    }

    fn assert_handoff_contract_identity() {
        assert_eq!(
            derive_generated_host_contract_identity_v1(
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                HANDOFF_MARKER_BINDING,
                "logical_primary",
                "vecadd",
                &manifest_abi(),
                &launch(),
            )
            .as_bytes(),
            &HANDOFF_HOST_CONTRACT,
        );
    }

    struct WrongMarker;

    unsafe impl KernelMarkerV1 for WrongMarker {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "wrong";
        const EXPORT_NAME: &'static str = "wrong";
        const FUNCTION: Self::Function = handoff_kernel;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl CompilerGeneratedKernelExpectationV1 for WrongMarker {
        const PROFILE: crate::CompilerGeneratedKernelProfileV1 =
            crate::CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
                generated_host_contract_identity: HANDOFF_HOST_CONTRACT,
            };
        const KERNEL_BINDING_ID_V1: [u8; 32] = HANDOFF_MARKER_BINDING;
    }

    struct ExactPrerequisiteAuthenticator {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl ExactPrerequisiteAuthenticator {
        fn new() -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
            let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            (
                Self {
                    calls: calls.clone(),
                },
                calls,
            )
        }
    }

    unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV2PrerequisiteAuthenticatorV1<K>
        for ExactPrerequisiteAuthenticator
    {
        type Error = &'static str;

        unsafe fn authenticate(
            &mut self,
            request: &crate::WorkerV2PrerequisiteRequestV1<'_, K>,
        ) -> Result<crate::WorkerV2PrerequisiteDecisionV1, Self::Error> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let artifact = request.artifact_identity();
            Ok(crate::WorkerV2PrerequisiteDecisionV1::new(
                request.finalized_digest(),
                artifact.kernel_id(),
                artifact.executable_digest(),
                request.target(),
                request.code_object_version(),
                K::LOGICAL_NAME,
                K::EXPORT_NAME,
                artifact.abi().clone(),
                artifact.launch().clone(),
                K::KERNEL_BINDING_ID_V1,
                tagged(0xb1),
                tagged(0xb2),
                tagged(0xb3),
                tagged(0xb4),
                tagged(0xb5),
                crate::WorkerV2SafetyPropertiesV1::required(),
            ))
        }
    }

    struct TestExecutable;
    struct TestKernel;

    struct ExactHsaAdapter {
        unloads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl ExactHsaAdapter {
        fn new() -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
            let unloads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            (
                Self {
                    unloads: unloads.clone(),
                },
                unloads,
            )
        }

        fn environment() -> crate::HsaEnvironmentObservationV1 {
            let target = fe2o3_amd_target::AmdTargetId::parse("gfx942:sramecc+:xnack-").unwrap();
            let runtime =
                crate::HsaRuntimeIdentityV1::new("ROCr", "test", tagged(0xc1), [0xc2; 16]).unwrap();
            let device = crate::HsaPhysicalDeviceIdentityV1::new([0xc3; 16], 7, 0, target).unwrap();
            let agent =
                crate::HsaAgentIdentityV1::new(runtime.instance(), 0xc4, device.uuid(), target)
                    .unwrap();
            crate::HsaEnvironmentObservationV1::new(runtime, device, agent).unwrap()
        }

        fn executable_object() -> crate::HsaExecutableObjectIdentityV1 {
            crate::HsaExecutableObjectIdentityV1::new([0xc5; 32]).unwrap()
        }

        fn kernel_object() -> crate::HsaKernelObjectIdentityV1 {
            crate::HsaKernelObjectIdentityV1::new([0xc6; 32]).unwrap()
        }
    }

    unsafe impl crate::ReviewedHsaExecutableLifecycleAdapterV1 for ExactHsaAdapter {
        type Executable = TestExecutable;
        type Kernel = TestKernel;
        type Error = &'static str;

        unsafe fn observe_environment(
            &mut self,
        ) -> Result<crate::HsaEnvironmentObservationV1, Self::Error> {
            Ok(Self::environment())
        }

        unsafe fn load_executable(
            &mut self,
            bytes: &[u8],
            finalized_digest: PayloadDigest,
        ) -> Result<(Self::Executable, crate::HsaCodeObjectLoadObservationV1), Self::Error>
        {
            let environment = Self::environment();
            Ok((
                TestExecutable,
                crate::HsaCodeObjectLoadObservationV1::new(
                    finalized_digest,
                    u64::try_from(bytes.len()).map_err(|_| "test byte length overflow")?,
                    environment.runtime().instance(),
                    environment.agent().agent_handle(),
                    Self::executable_object(),
                ),
            ))
        }

        unsafe fn resolve_kernel(
            &mut self,
            _executable: &Self::Executable,
            export_symbol: &str,
        ) -> Result<(Self::Kernel, crate::HsaKernelResolutionObservationV1), Self::Error> {
            Ok((
                TestKernel,
                crate::HsaKernelResolutionObservationV1::new(
                    Self::executable_object(),
                    Self::kernel_object(),
                    export_symbol,
                    272,
                    16,
                )
                .map_err(|_| "invalid test kernel observation")?,
            ))
        }

        unsafe fn launch_and_wait(
            &mut self,
            _executable: &Self::Executable,
            _kernel: &Self::Kernel,
            geometry: crate::HsaLaunchGeometryV1,
            _kernarg: &mut [u8],
        ) -> Result<crate::HsaDispatchObservationV1, Self::Error> {
            crate::HsaDispatchObservationV1::new(
                [0xc7; 16],
                Self::executable_object(),
                Self::kernel_object(),
                geometry,
                true,
            )
            .map_err(|_| "invalid test dispatch observation")
        }

        unsafe fn unload_executable(
            &mut self,
            _executable: Self::Executable,
        ) -> Result<crate::HsaUnloadObservationV1, Self::Error> {
            self.unloads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let environment = Self::environment();
            Ok(crate::HsaUnloadObservationV1::new(
                Self::executable_object(),
                environment.runtime().instance(),
                environment.agent().agent_handle(),
                true,
            ))
        }
    }

    unsafe impl ReviewedHsaImplicitKernargAdapterV1 for ExactHsaAdapter {
        unsafe fn initialize_implicit_kernarg(
            &mut self,
            _executable: &Self::Executable,
            _kernel: &Self::Kernel,
            geometry: crate::HsaLaunchGeometryV1,
            explicit_byte_len: usize,
            implicit_byte_offset: usize,
            implicit_byte_len: usize,
            kernarg: &mut [u8],
        ) -> Result<crate::HsaImplicitKernargInitializationObservationV1, Self::Error> {
            kernarg[implicit_byte_offset..implicit_byte_offset + implicit_byte_len].fill(0);
            Ok(crate::HsaImplicitKernargInitializationObservationV1::new(
                Self::executable_object(),
                Self::kernel_object(),
                geometry,
                u64::try_from(explicit_byte_len).map_err(|_| "explicit length overflow")?,
                u64::try_from(implicit_byte_offset).map_err(|_| "implicit offset overflow")?,
                u64::try_from(implicit_byte_len).map_err(|_| "implicit length overflow")?,
                true,
            ))
        }
    }

    struct UnreachableArguments;

    unsafe impl<'allocation> CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, HandoffKernel>
        for UnreachableArguments
    {
        fn dispatch_identity_v1() -> crate::AlphaZetaCov6DispatchIdentityV1 {
            panic!("stream substitution must fail before generated identity is requested")
        }

        fn generated_argument_layout_v1()
        -> Result<crate::CompilerGeneratedArgumentLayoutV1, crate::GeneratedArgumentLayoutError>
        {
            panic!("stream substitution must fail before generated layout is requested")
        }

        fn bind_arguments_v1(
            &self,
            _plan: &crate::GeneratedArgumentPackingPlanV1,
        ) -> Result<
            crate::GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation>,
            crate::GeneratedArgumentPackError,
        > {
            panic!("stream substitution must fail before argument binding")
        }
    }

    fn managed_artifact(output: &Path) -> std::path::PathBuf {
        let mut matches = fs::read_dir(output)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.starts_with(ARTIFACT_PREFIX) && name.ends_with(ARTIFACT_SUFFIX)
            });
        let path = matches.next().unwrap();
        assert!(matches.next().is_none());
        path
    }

    #[test]
    fn canonical_envelope_recovers_one_inert_pinned_descriptor() {
        let fixture = recovery_fixture(1, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();

        assert_eq!(recovered.descriptor().kernel_id(), fixture.kernel_id);
        assert_eq!(recovered.physical_kernel().export_symbol(), "vecadd");
        assert_eq!(recovered.target().processor(), "gfx942");
        assert_eq!(recovered.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(
            recovered.descriptor_binding().descriptor().kernarg_size(),
            272
        );
        assert!(!recovered.authenticates_prerequisites());
        assert!(!recovered.grants_load_authority());
        assert!(!recovered.grants_launch_authority());
        recovered.revalidate_currentness().unwrap();
    }

    #[test]
    fn malformed_truncated_and_trailing_envelopes_are_rejected_before_recovery() {
        let fixture = recovery_fixture(2, "gfx942", "vecadd");
        for bytes in [
            &fixture.envelope[..1],
            &fixture.envelope[..fixture.envelope.len() - 1],
        ] {
            assert!(matches!(
                recover_worker_v2_load_envelope_v1(
                    &fixture.output,
                    bytes,
                    fixture.kernel_id,
                    &fixture.observed,
                ),
                Err(RecoveredWorkerV2AdmissionError::Decode(_))
            ));
        }
        let mut trailing = fixture.envelope.clone();
        trailing.push(0);
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &fixture.output,
                &trailing,
                fixture.kernel_id,
                &fixture.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Decode(
                EnvelopeDecodeError::TrailingBytes
            ))
        ));
        let mut substituted = fixture.envelope.clone();
        let last = substituted.len() - 1;
        substituted[last] ^= 1;
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &fixture.output,
                &substituted,
                fixture.kernel_id,
                &fixture.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Decode(_))
        ));
        let oversized = vec![0; fe2o3_worker_v2_bundle::MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1];
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &fixture.output,
                &oversized,
                fixture.kernel_id,
                &fixture.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Decode(
                EnvelopeDecodeError::TooLarge { .. }
            ))
        ));
    }

    #[test]
    fn stale_attempt_and_cross_output_substitution_are_rejected() {
        let stale = recovery_fixture(3, "gfx942", "vecadd");
        fail_build_attempt(&stale.output, &stale.owner, stale.attempt).unwrap();
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &stale.output,
                &stale.envelope,
                stale.kernel_id,
                &stale.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Publication(_))
        ));

        let first = recovery_fixture(4, "gfx942", "vecadd");
        let second = recovery_fixture(5, "gfx942", "vecadd");
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &second.output,
                &first.envelope,
                first.kernel_id,
                &first.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Publication(_))
        ));
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &first.output,
                &first.envelope,
                second.kernel_id,
                &first.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::KernelNotFound)
        ));
    }

    #[test]
    fn raw_final_and_physical_kernel_substitution_are_rejected() {
        let raw_substitution = recovery_fixture(6, "gfx950", "vecadd");
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &raw_substitution.output,
                &raw_substitution.envelope,
                raw_substitution.kernel_id,
                &raw_substitution.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::RawFinalizedPayloadMismatch)
        ));

        let physical_substitution = recovery_fixture(7, "gfx942", "substituted_kernel");
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &physical_substitution.output,
                &physical_substitution.envelope,
                physical_substitution.kernel_id,
                &physical_substitution.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::DescriptorLineageMismatch)
        ));
    }

    #[test]
    fn recovered_envelope_handoff_authenticates_loads_and_unloads_exact_bytes() {
        assert_handoff_contract_identity();
        let fixture = recovery_fixture(20, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let expected_digest = recovered.artifact_identity().payload_digest();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();

        let authority = recovered
            .finish_generated_application_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(
                RetainedApplicationStreamV1::Test,
                &mut authenticator,
                adapter,
            )
            .unwrap();

        assert_eq!(
            authority.load_observation().finalized_digest(),
            expected_digest
        );
        assert_eq!(authority.kernel_observation().export_symbol(), "vecadd");
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 0);

        let unloaded = authority.unload().unwrap();
        assert_eq!(unloaded.finalized_digest(), expected_digest);
        assert!(unloaded.unload_observation().released());
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn wrong_marker_is_rejected_before_the_unsafe_authenticator() {
        let fixture = recovery_fixture(21, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();

        let error = recovered
            .finish_generated_application_handoff_v1::<
                WrongMarker,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(
                RetainedApplicationStreamV1::Test,
                &mut authenticator,
                adapter,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RecoveredWorkerV2ApplicationHandoffError::Selection(_)
        ));
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[cfg(unix)]
    #[test]
    fn replaced_publication_is_rejected_before_authentication_or_hsa_load() {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let fixture = recovery_fixture(22, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let artifact = managed_artifact(&fixture.output);
        let bytes = fs::read(&artifact).unwrap();
        fs::rename(&artifact, artifact.with_extension("replaced")).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&artifact)
            .unwrap();
        fs::write(&artifact, bytes).unwrap();
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o600)).unwrap();

        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let error = recovered
            .finish_generated_application_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(
                RetainedApplicationStreamV1::Test,
                &mut authenticator,
                adapter,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RecoveredWorkerV2ApplicationHandoffError::CurrentPublication(_)
        ));
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn changed_publication_bytes_are_rejected_before_authentication_or_hsa_load() {
        let fixture = recovery_fixture(25, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let artifact = managed_artifact(&fixture.output);
        let mut bytes = fs::read(&artifact).unwrap();
        bytes[0] ^= 0xff;
        fs::write(&artifact, bytes).unwrap();

        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let error = recovered
            .finish_generated_application_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(
                RetainedApplicationStreamV1::Test,
                &mut authenticator,
                adapter,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RecoveredWorkerV2ApplicationHandoffError::CurrentPublication(_)
        ));
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    #[ignore = "requires a working gfx942 HIP context"]
    fn application_handoff_rejects_context_and_stream_substitution() {
        let context = GpuContext::new(0).unwrap();
        let observed = ObservedContext::observe(&context).unwrap();
        assert_eq!(observed.device().target_id().processor(), "gfx942");
        let stream = context.default_stream();
        let other_stream = context.default_stream();

        let mut context_fixture = recovery_fixture(23, "gfx942", "vecadd");
        context_fixture.observed = observed.clone();
        let recovered = recover_worker_v2_load_envelope_v1(
            &context_fixture.output,
            &context_fixture.envelope,
            context_fixture.kernel_id,
            &context_fixture.observed,
        )
        .unwrap();
        let other_context = GpuContext::new(0).unwrap();
        let foreign_stream = other_context.default_stream();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let error = recovered
            .load_generated_application_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&foreign_stream, &mut authenticator, adapter)
            .unwrap_err();
        assert!(matches!(
            error,
            RecoveredWorkerV2ApplicationHandoffError::Binding(
                RecoveredWorkerV2ApplicationBindingError::ContextSubstitution
            )
        ));
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 0);

        let mut stream_fixture = recovery_fixture(24, "gfx942", "vecadd");
        stream_fixture.observed = observed;
        let recovered = recover_worker_v2_load_envelope_v1(
            &stream_fixture.output,
            &stream_fixture.envelope,
            stream_fixture.kernel_id,
            &stream_fixture.observed,
        )
        .unwrap();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let mut authority = recovered
            .load_generated_application_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&stream, &mut authenticator, adapter)
            .unwrap();
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        let (mut selected_authenticator, selected_calls) = ExactPrerequisiteAuthenticator::new();
        let error = match authority
            .prepare_generated_alpha_zeta_cov6_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                UnreachableArguments,
            >(
                &other_stream,
                &mut selected_authenticator,
                UnreachableArguments,
            )
        {
            Ok(_) => panic!("substituted stream unexpectedly prepared an invocation"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RecoveredWorkerV2ApplicationPrepareError::StreamSubstitution { .. }
        ));
        assert_eq!(selected_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        authority.unload().unwrap();
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[cfg(unix)]
    #[test]
    fn same_byte_artifact_replacement_invalidates_an_existing_descriptor() {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let fixture = recovery_fixture(8, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let artifact = managed_artifact(&fixture.output);
        let bytes = fs::read(&artifact).unwrap();
        fs::rename(&artifact, artifact.with_extension("displaced")).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&artifact)
            .unwrap();
        fs::write(&artifact, bytes).unwrap();
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(recovered.revalidate_currentness().is_err());
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &fixture.output,
                &fixture.envelope,
                fixture.kernel_id,
                &fixture.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Publication(_))
        ));
    }
}
