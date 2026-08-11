#[cfg(target_os = "linux")]
use crate::application_descriptor_handoff::{
    RetainedWorkerV2ApplicationDescriptorsV1, WorkerV2ApplicationDescriptorHandoffErrorV1,
};
use crate::{
    AdmittedFinalizedWorkerV2BundleV1, ArtifactKernelIdentityV1, AuthenticatedWorkerV2ExecutableV1,
    CompilerGeneratedAlphaZetaCov6ArgumentsV1, CompilerGeneratedKernelExpectationV1,
    CurrentFinalizedWorkerV2BundleAdmissionV1, DeviceIdentity,
    FinalizedWorkerV2BundleAdmissionError, GeneratedAlphaZetaCov6PrepareError,
    GeneratedAlphaZetaCov6PreparedInvocationV1, HsaExecutableLoadError, HsaGeneratedDispatchError,
    HsaLaunchGeometryV1, HsaLoadAuthorizationError, LoadedHsaExecutableV1,
    MissingFinalizedWorkerV2LoadPrerequisiteV1, ObservedContext, PhysicalMetadataValueV1,
    PublishedKernelPhysicalLayoutV1, ReviewedHsaImplicitKernargAdapterV1, UnloadedHsaExecutableV1,
    WorkerV2ExecutableAuthenticationError, WorkerV2PrerequisiteAuthenticatorV1,
    WorkerV2TypedKernelSelectionError,
};
use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationLeaseV1, DurableCurrentLinkPublicationTokenV1,
    DurableLinkPublicationError, DurablePublishedClaimReacquisitionErrorV1,
    PublishedLinkArtifactV1, reacquire_current_hsaco_publication_lease_v1,
};
use fe2o3_hsaco::{CodeObjectVersion, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::{FinalizationError, finalize_unfinalized, verify_finalized};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion as DescriptorCodeObjectVersion, KernelDescriptorV1, KernelId,
};
use fe2o3_worker_v2_bundle::{
    CompilerTransactionEvidenceCapsuleV2, EnvelopeDecodeError, WorkerV2LoadEnvelopeV1,
};
use std::error::Error;
use std::fmt;
use std::path::Path;

/// Read-only host descriptor recovered from one canonical Worker V2 envelope.
///
/// The value owns the fresh process-local exact-file lease and the complete decoded envelope,
/// but exposes neither. Public accessors return only inert identity and descriptor metadata. It is
/// intentionally neither `Clone` nor `Copy` and has no module-load, launch, or prerequisite-
/// authentication transition. This is cooperative-process, non-production evidence; it does not
/// defend authority from malicious code in the same process.
pub struct RecoveredWorkerV2PinnedDescriptorV1 {
    admission: AdmittedFinalizedWorkerV2BundleV1,
    descriptor: KernelDescriptorV1,
    observed: ObservedContext,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<RetainedWorkerV2ApplicationDescriptorsV1>,
}

impl fmt::Debug for RecoveredWorkerV2PinnedDescriptorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(target_os = "linux")]
        let retains_application_descriptors = self.application_descriptors.is_some();
        #[cfg(not(target_os = "linux"))]
        let retains_application_descriptors = false;
        formatter
            .debug_struct("RecoveredWorkerV2PinnedDescriptorV1")
            .field("published", &self.admission.published())
            .field("descriptor", &self.descriptor)
            .field("artifact_identity", self.admission.artifact_identity())
            .field("target", &self.admission.target())
            .field("code_object_version", &self.admission.code_object_version())
            .field(
                "retains_application_descriptors",
                &retains_application_descriptors,
            )
            .finish_non_exhaustive()
    }
}

impl RecoveredWorkerV2PinnedDescriptorV1 {
    /// Decodes and admits one exact envelope against its durable output directory.
    pub(crate) fn recover(
        output_dir: &Path,
        envelope_bytes: &[u8],
        compiler_transaction: CompilerTransactionEvidenceCapsuleV2,
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
            compiler_transaction,
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
            #[cfg(target_os = "linux")]
            application_descriptors: None,
        })
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn retain_application_descriptors(
        mut self,
        descriptors: RetainedWorkerV2ApplicationDescriptorsV1,
    ) -> Self {
        debug_assert!(self.application_descriptors.is_none());
        self.application_descriptors = Some(descriptors);
        self
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

    pub(crate) fn acquire_launch_kernel_v2_currentness(
        &self,
    ) -> Result<CurrentFinalizedWorkerV2BundleAdmissionV1<'_>, FinalizedWorkerV2BundleAdmissionError>
    {
        self.admission.acquire_currentness()
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

    /// Revalidates and consumes this descriptor into one synchronous HSA handoff.
    ///
    /// The descriptor remains inert until this transition is called with the existing unsafe
    /// compiler/Verus prerequisite authenticator. The returned authority owns the exact recovered
    /// envelope and publication lease through the loaded executable, retains the observed device
    /// and context facts, and exposes only generated typed preparation. Dispatch uses the reviewed
    /// adapter's private HSA queue and waits for quiescence; this API makes no HIP-stream ordering
    /// or execution claim. Native bytes and handles remain private.
    pub fn load_generated_synchronous_hsa_handoff_v1<K, Authenticator, Adapter>(
        self,
        authenticator: &mut Authenticator,
        adapter: Adapter,
    ) -> Result<
        RecoveredWorkerV2SynchronousHsaHandoffV1<K, Adapter>,
        RecoveredWorkerV2SynchronousHsaHandoffError<Authenticator::Error, Adapter::Error>,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        Authenticator: WorkerV2PrerequisiteAuthenticatorV1<K>,
        Adapter: ReviewedHsaImplicitKernargAdapterV1,
    {
        #[cfg(target_os = "linux")]
        if let Some(descriptors) = &self.application_descriptors {
            descriptors
                .revalidate()
                .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::ApplicationDescriptors)?;
        }
        self.admission
            .acquire_currentness()
            .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::CurrentPublication)?;
        self.admission
            .select_typed_kernel::<K>()
            .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::Selection)?;
        let authenticated =
            AuthenticatedWorkerV2ExecutableV1::<K>::authenticate(self.admission, authenticator)
                .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::Authentication)?;
        let currentness = authenticated
            .acquire_retained_currentness_token()
            .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::CurrentPublication)?;
        let authorized = authenticated
            .authorize_hsa_load(adapter)
            .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::Authorization)?;
        let loaded = authorized
            .load_with_retained_currentness(&currentness)
            .map_err(RecoveredWorkerV2SynchronousHsaHandoffError::Load)?;
        Ok(RecoveredWorkerV2SynchronousHsaHandoffV1 {
            loaded,
            currentness,
            observed: self.observed,
            #[cfg(target_os = "linux")]
            application_descriptors: self.application_descriptors,
        })
    }
}

/// Linear synchronous HSA authority recovered from one canonical Worker V2 envelope.
///
/// This value is intentionally neither `Clone` nor `Copy`. It retains the exact publication
/// lease, envelope bytes, loaded executable, and original context observation. Its reviewed HSA
/// adapter owns the private queue and every dispatch waits for quiescence. This value does not
/// represent, retain, or order work on a HIP stream. A non-clone currentness token keeps the
/// cooperative publication lock held from before HSA load through prepare, synchronous dispatch
/// completion, and executable unload. Cooperative generation turnover blocks until unload drops
/// that token; retained file and path identities are revalidated before every transition.
pub struct RecoveredWorkerV2SynchronousHsaHandoffV1<K, A: ReviewedHsaImplicitKernargAdapterV1> {
    loaded: LoadedHsaExecutableV1<K, A>,
    currentness: DurableCurrentLinkPublicationTokenV1,
    observed: ObservedContext,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<RetainedWorkerV2ApplicationDescriptorsV1>,
}

/// Result of preparing one invocation through recovered synchronous HSA authority.
#[doc(hidden)]
pub type RecoveredWorkerV2SynchronousHsaPrepareResultV1<
    'loaded,
    'allocation,
    Root,
    Selected,
    Adapter,
    Arguments,
    PrerequisiteError,
> = Result<
    RecoveredWorkerV2SynchronousHsaPreparedInvocationV1<
        'loaded,
        'allocation,
        Root,
        Selected,
        Adapter,
        Arguments,
    >,
    RecoveredWorkerV2SynchronousHsaPrepareError<
        PrerequisiteError,
        <Adapter as crate::ReviewedHsaExecutableLifecycleAdapterV1>::Error,
    >,
>;

/// Prepared invocation borrowing the recovered publication's locked currentness token.
///
/// Dispatch revalidates the pinned publication under that still-held cooperative lock immediately
/// before calling the reviewed synchronous adapter.
#[must_use = "a prepared recovered invocation does no work until dispatched"]
#[doc(hidden)]
pub struct RecoveredWorkerV2SynchronousHsaPreparedInvocationV1<
    'loaded,
    'allocation,
    Root,
    Selected,
    Adapter: ReviewedHsaImplicitKernargAdapterV1,
    Arguments,
> {
    prepared: GeneratedAlphaZetaCov6PreparedInvocationV1<
        'loaded,
        'allocation,
        Root,
        Selected,
        Adapter,
        Arguments,
    >,
    currentness: &'loaded DurableCurrentLinkPublicationTokenV1,
    #[cfg(target_os = "linux")]
    application_descriptors: Option<&'loaded RetainedWorkerV2ApplicationDescriptorsV1>,
}

impl<Root, Selected, Adapter, Arguments>
    RecoveredWorkerV2SynchronousHsaPreparedInvocationV1<'_, '_, Root, Selected, Adapter, Arguments>
where
    Adapter: ReviewedHsaImplicitKernargAdapterV1,
{
    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.prepared.geometry()
    }

    pub const fn explicit_byte_len(&self) -> usize {
        self.prepared.explicit_byte_len()
    }

    pub fn physical_kernarg_byte_len(&self) -> usize {
        self.prepared.physical_kernarg_byte_len()
    }

    pub fn physical_kernarg_alignment(&self) -> usize {
        self.prepared.physical_kernarg_alignment()
    }

    pub fn dispatch(
        self,
    ) -> Result<
        crate::GeneratedAlphaZetaCov6CompletionV1<Selected>,
        RecoveredWorkerV2SynchronousHsaDispatchError<Adapter::Error>,
    > {
        self.currentness
            .revalidate_locked_currentness()
            .map_err(RecoveredWorkerV2SynchronousHsaDispatchError::CurrentPublication)?;
        #[cfg(target_os = "linux")]
        if let Some(descriptors) = self.application_descriptors {
            descriptors
                .revalidate()
                .map_err(RecoveredWorkerV2SynchronousHsaDispatchError::ApplicationDescriptors)?;
        }
        self.prepared
            .dispatch()
            .map_err(RecoveredWorkerV2SynchronousHsaDispatchError::Dispatch)
    }
}

impl<K, A: ReviewedHsaImplicitKernargAdapterV1> fmt::Debug
    for RecoveredWorkerV2SynchronousHsaHandoffV1<K, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(target_os = "linux")]
        let retains_application_descriptors = self.application_descriptors.is_some();
        #[cfg(not(target_os = "linux"))]
        let retains_application_descriptors = false;
        formatter
            .debug_struct("RecoveredWorkerV2SynchronousHsaHandoffV1")
            .field("load", self.loaded.load_observation())
            .field("kernel", self.loaded.kernel_observation())
            .field(
                "retains_application_descriptors",
                &retains_application_descriptors,
            )
            .finish_non_exhaustive()
    }
}

impl<K, A: ReviewedHsaImplicitKernargAdapterV1> RecoveredWorkerV2SynchronousHsaHandoffV1<K, A> {
    pub const fn load_observation(&self) -> &crate::HsaCodeObjectLoadObservationV1 {
        self.loaded.load_observation()
    }

    pub const fn kernel_observation(&self) -> &crate::HsaKernelResolutionObservationV1 {
        self.loaded.kernel_observation()
    }

    /// Prepares one generated alpha/zeta COV6 invocation for synchronous HSA dispatch.
    ///
    /// The reviewed adapter owns its HSA queue and waits for dispatch quiescence. Generated
    /// argument capabilities enforce exact context and allocation provenance. No HIP stream is
    /// accepted, retained, or associated with the dispatch.
    #[doc(hidden)]
    pub fn prepare_generated_alpha_zeta_cov6_v1<
        'loaded,
        'allocation,
        Selected,
        Authenticator,
        Arguments,
    >(
        &'loaded mut self,
        authenticator: &mut Authenticator,
        arguments: Arguments,
    ) -> RecoveredWorkerV2SynchronousHsaPrepareResultV1<
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
        #[cfg(target_os = "linux")]
        if let Some(descriptors) = &self.application_descriptors {
            descriptors
                .revalidate()
                .map_err(RecoveredWorkerV2SynchronousHsaPrepareError::ApplicationDescriptors)?;
        }
        self.currentness
            .revalidate_locked_currentness()
            .map_err(RecoveredWorkerV2SynchronousHsaPrepareError::CurrentPublication)?;
        let prepared = self
            .loaded
            .prepare_generated_alpha_zeta_cov6_selected_kernel_v1::<
                Selected,
                Authenticator,
                Arguments,
            >(&self.observed, authenticator, arguments)
            .map_err(RecoveredWorkerV2SynchronousHsaPrepareError::Prepare)?;
        Ok(RecoveredWorkerV2SynchronousHsaPreparedInvocationV1 {
            prepared,
            currentness: &self.currentness,
            #[cfg(target_os = "linux")]
            application_descriptors: self.application_descriptors.as_ref(),
        })
    }

    pub fn unload(
        self,
    ) -> Result<UnloadedHsaExecutableV1, RecoveredWorkerV2SynchronousHsaUnloadError<A::Error>> {
        let Self {
            loaded,
            currentness,
            observed,
            #[cfg(target_os = "linux")]
            application_descriptors,
        } = self;
        let current = currentness.revalidate_locked_currentness();
        let unloaded = loaded.unload();
        #[cfg(target_os = "linux")]
        let _lifetime_guard = (currentness, observed, application_descriptors);
        #[cfg(not(target_os = "linux"))]
        let _lifetime_guard = (currentness, observed);
        match current {
            Ok(()) => unloaded.map_err(RecoveredWorkerV2SynchronousHsaUnloadError::Unload),
            Err(source) => Err(
                RecoveredWorkerV2SynchronousHsaUnloadError::CurrentPublication {
                    source,
                    unload: unloaded.err(),
                },
            ),
        }
    }
}

/// Failure while converting a recovered envelope into synchronous HSA authority.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2SynchronousHsaHandoffError<PrerequisiteError, AdapterError> {
    #[cfg(target_os = "linux")]
    ApplicationDescriptors(WorkerV2ApplicationDescriptorHandoffErrorV1),
    CurrentPublication(FinalizedWorkerV2BundleAdmissionError),
    Selection(WorkerV2TypedKernelSelectionError),
    Authentication(WorkerV2ExecutableAuthenticationError<PrerequisiteError>),
    Authorization(HsaLoadAuthorizationError<AdapterError>),
    Load(HsaExecutableLoadError<AdapterError>),
}

/// Failure while preparing through recovered synchronous HSA authority.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2SynchronousHsaPrepareError<PrerequisiteError, AdapterError> {
    #[cfg(target_os = "linux")]
    ApplicationDescriptors(WorkerV2ApplicationDescriptorHandoffErrorV1),
    CurrentPublication(DurableLinkPublicationError),
    Prepare(GeneratedAlphaZetaCov6PrepareError<PrerequisiteError, AdapterError>),
}

/// Failure while dispatching a recovered prepared invocation.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2SynchronousHsaDispatchError<AdapterError> {
    CurrentPublication(DurableLinkPublicationError),
    #[cfg(target_os = "linux")]
    ApplicationDescriptors(WorkerV2ApplicationDescriptorHandoffErrorV1),
    Dispatch(HsaGeneratedDispatchError<AdapterError>),
}

/// Failure while revalidating and unloading recovered synchronous HSA authority.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV2SynchronousHsaUnloadError<AdapterError> {
    CurrentPublication {
        source: DurableLinkPublicationError,
        unload: Option<crate::HsaExecutableUnloadError<AdapterError>>,
    },
    Unload(crate::HsaExecutableUnloadError<AdapterError>),
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
pub(crate) fn recover_worker_v2_load_envelope_v1(
    output_dir: &Path,
    envelope_bytes: &[u8],
    compiler_transaction: CompilerTransactionEvidenceCapsuleV2,
    kernel_id: KernelId,
    observed: &ObservedContext,
) -> Result<RecoveredWorkerV2PinnedDescriptorV1, RecoveredWorkerV2AdmissionError> {
    RecoveredWorkerV2PinnedDescriptorV1::recover(
        output_dir,
        envelope_bytes,
        compiler_transaction,
        kernel_id,
        observed,
    )
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
    #[cfg(target_os = "linux")]
    use crate::application_descriptor_handoff::consume_worker_v2_application_handoff_descriptors_v1;
    use crate::hsa_executable_lifecycle::tests::{
        AlphaCov6TestKernel, alpha_cov6_arguments_for_lifecycle_test,
    };
    use crate::published_direct_link::tests::{
        Fixture, make_observed_for, make_single_hsaco_fixture_with_names_and_kernel_id,
    };
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, DurableLinkPublicationPlanV1, PackageIdentityV1,
        ProducerIdentity, UpstreamCodeObjectEvidenceIdentityV1, begin_build_attempt,
        fail_build_attempt, install_begin_build_attempt_lock_probe_v1,
        publish_exact_hsaco_evidence_for_attempt_v1,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        BlockSize, CallerClaimedPackageIdentityV1, DeclaredRustLayoutIdentity,
        DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions,
        DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1,
        DirectLinkContainerIdentityV1, DirectLinkLinkedOutputIdentityV1,
        DirectLinkTransformationIdentityV1, ManifestClaimDerivedLinkPublicationScopeV1,
        ManifestClaimDirectLinkPublicationBridgeV1, MeasuredToolIdentity, Mutability, Name,
        PayloadDigest, PointerWidth, ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome,
        ProofProperty, ProofRecordV1, ProofTargetIdentity, SourceContractIdentity, TypeIdentity,
        VerificationModelIdentity, derive_generated_host_contract_identity_v1,
        derive_generated_kernel_identity_v2,
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
    use fe2o3_rustc_invocation::InvocationDigestV2;
    use fe2o3_worker_v2_bundle::{
        CallerMeasuredBackendInvocationIdentityV2, CallerMeasuredKernelIrIdentityV2,
        CallerMeasuredSemanticWitnessIdentityV2, CallerMeasuredSourceRootIdentityV2,
        CompilerSourceClosureV2, CompilerTransactionEvidenceCapsuleV2,
        CompilerTransactionEvidencePartsV2, DescriptorLineageV1, ExactRawHsacoV1,
        WorkerV2ApplicationHandoffAckV1, WorkerV2ApplicationHandoffChallengeV1,
        WorkerV2ApplicationHandoffExpectationV1,
    };
    use reserved_fe2o3_symbols::{
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
    };
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::io::{Read, Write};
    #[cfg(target_os = "linux")]
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    #[allow(dead_code)]
    mod canonical_hsaco_fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fe2o3-hsaco-finalize/tests/fixtures/worker_v2_hsaco_test_support.rs"
        ));

        pub(super) fn with_descriptor_table(
            target: &str,
            table: &[u8],
            include_explicit_argument_alignments: bool,
        ) -> Vec<u8> {
            let mut options = FixtureOptions::valid();
            options.target = target;
            options.include_explicit_argument_alignments = include_explicit_argument_alignments;
            fixture_with_descriptor_table(options, Some(table)).bytes
        }
    }

    const ARTIFACT_PREFIX: &str = ".fe2o3-link-artifact-v1-";
    const ARTIFACT_SUFFIX: &str = ".bin";
    const REQUIRED_GFX942_TEST_TARGET: &str = "gfx942:xnack-";
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
        compiler_transaction: CompilerTransactionEvidenceCapsuleV2,
        kernel_id: KernelId,
        observed: ObservedContext,
    }

    fn digest(seed: u8) -> DigestBytes {
        DigestBytes::from_bytes([seed; 32])
    }

    fn tagged(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, digest(seed))
    }

    fn measured_tool(name: &str, seed: u8) -> MeasuredToolIdentity {
        MeasuredToolIdentity::new(
            identity_text(name),
            identity_text("test"),
            tagged(seed.max(1)),
            tagged(seed.wrapping_add(1).max(1)),
        )
    }

    fn compiler_transaction(
        fixture: &Fixture,
        target: fe2o3_artifact_transaction::TargetIdentityV1,
        seed: u8,
    ) -> CompilerTransactionEvidenceCapsuleV2 {
        let expectation = &fixture.expectations[0];
        CompilerTransactionEvidenceCapsuleV2::new(CompilerTransactionEvidencePartsV2 {
            source_closure: CompilerSourceClosureV2::new(
                CallerMeasuredSourceRootIdentityV2::try_from_sha256([seed.max(1); 32]).unwrap(),
                vec![],
                vec![],
            )
            .unwrap(),
            rustc_tool: measured_tool("rustc", seed.wrapping_add(1)),
            rustc_invocation: InvocationDigestV2::from_bytes([seed.wrapping_add(3).max(1); 32])
                .unwrap(),
            backend_tool: measured_tool("rustc-codegen-fe2o3", seed.wrapping_add(4)),
            backend_invocation: CallerMeasuredBackendInvocationIdentityV2::try_from_sha256(
                [seed.wrapping_add(6).max(1); 32],
            )
            .unwrap(),
            semantic_witness: CallerMeasuredSemanticWitnessIdentityV2::try_from_sha256(
                [seed.wrapping_add(7).max(1); 32],
            )
            .unwrap(),
            kernel_ir: CallerMeasuredKernelIrIdentityV2::try_from_sha256(
                [seed.wrapping_add(8).max(1); 32],
            )
            .unwrap(),
            worker_request: expectation.request_identity(),
            worker_response: expectation.response_identity(),
            target,
            raw_hsaco: expectation.linked_output_identity(),
            finalized_hsaco: expectation.finalized_payload_identity(),
            artifact: DirectLinkContainerIdentityV1::new(
                DigestAlgorithm::Sha256.calculate(&fixture.container.to_bytes()),
            ),
        })
        .unwrap()
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
        recovery_fixture_with_explicit_argument_alignments(seed, raw_target, manifest_symbol, false)
    }

    fn recovery_fixture_with_explicit_argument_alignments(
        seed: u8,
        raw_target: &str,
        manifest_symbol: &str,
        include_explicit_argument_alignments: bool,
    ) -> RecoveryFixture {
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
            REQUIRED_GFX942_TEST_TARGET,
            [0; 32],
        );
        let final_raw = canonical_hsaco_fixture::with_descriptor_table(
            REQUIRED_GFX942_TEST_TARGET,
            &encode_device_descriptor_table_v1(&final_raw_table).unwrap(),
            include_explicit_argument_alignments,
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
                include_explicit_argument_alignments,
            )
        };
        let mut fixture = make_single_hsaco_fixture_with_names_and_kernel_id(
            seed,
            finalized.clone(),
            REQUIRED_GFX942_TEST_TARGET,
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
                REQUIRED_GFX942_TEST_TARGET,
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
        let target_identity = descriptive_scope.target();
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
        let compiler_transaction = compiler_transaction(&fixture, target_identity, seed);
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
            compiler_transaction,
            kernel_id: KernelId::from_bytes(*kernel_id.as_bytes()),
            observed: make_observed_for(usize::from(seed), REQUIRED_GFX942_TEST_TARGET),
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
                request.challenge_identity().clone(),
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
        turnover_completed: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    }

    impl ExactHsaAdapter {
        fn new() -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
            let unloads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            (
                Self {
                    unloads: unloads.clone(),
                    turnover_completed: None,
                },
                unloads,
            )
        }

        fn with_turnover_probe(
            turnover_completed: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
            let (mut adapter, unloads) = Self::new();
            adapter.turnover_completed = Some(turnover_completed);
            (adapter, unloads)
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

        fn assert_turnover_pending(&self, stage: &str) {
            if let Some(turnover_completed) = &self.turnover_completed {
                assert!(
                    !turnover_completed.load(std::sync::atomic::Ordering::SeqCst),
                    "publication turned over before {stage}"
                );
            }
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
            let kernarg_segment_size = if export_symbol == "alpha" { 296 } else { 272 };
            Ok((
                TestKernel,
                crate::HsaKernelResolutionObservationV1::new(
                    Self::executable_object(),
                    Self::kernel_object(),
                    export_symbol,
                    kernarg_segment_size,
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
            self.assert_turnover_pending("synchronous dispatch completed");
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
            self.assert_turnover_pending("recovered executable unload completed");
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
            self.assert_turnover_pending("generated invocation preparation completed");
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

    #[cfg(target_os = "linux")]
    struct ApplicationDescriptorFixture {
        envelope_path: std::path::PathBuf,
        envelope: OwnedFd,
        artifact_directory: OwnedFd,
        acknowledgment_read: OwnedFd,
        acknowledgment_write: OwnedFd,
        expectation: WorkerV2ApplicationHandoffExpectationV1,
        challenge: WorkerV2ApplicationHandoffChallengeV1,
    }

    #[cfg(target_os = "linux")]
    fn application_descriptor_fixture(
        fixture: &RecoveryFixture,
        seed: u8,
    ) -> ApplicationDescriptorFixture {
        fs::set_permissions(&fixture.output, fs::Permissions::from_mode(0o700)).unwrap();
        let envelope_value = WorkerV2LoadEnvelopeV1::from_bytes(&fixture.envelope).unwrap();
        let envelope_name = fe2o3_worker_v2_bundle::worker_v2_load_envelope_name_v1(
            envelope_value
                .published_claim()
                .receipt()
                .publication_identity(),
        );
        let envelope_path = fixture.output.join(envelope_name);
        let mut writer = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&envelope_path)
            .unwrap();
        writer.write_all(&fixture.envelope).unwrap();
        writer.sync_all().unwrap();
        drop(writer);

        let envelope = fs::File::open(&envelope_path).unwrap().into();
        let artifact_directory = fs::File::open(&fixture.output).unwrap().into();
        let mut pipe = [-1; 2];
        // SAFETY: `pipe` has room for both returned descriptors and successful descriptors are
        // immediately transferred into separate `OwnedFd` values.
        assert_eq!(
            unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        // SAFETY: successful `pipe2` returned two distinct descriptors owned by this process.
        let acknowledgment_read = unsafe { OwnedFd::from_raw_fd(pipe[0]) };
        // SAFETY: successful `pipe2` returned two distinct descriptors owned by this process.
        let acknowledgment_write = unsafe { OwnedFd::from_raw_fd(pipe[1]) };
        let application =
            crate::application_descriptor_handoff::current_application_identity().unwrap();
        let expectation =
            WorkerV2ApplicationHandoffExpectationV1::new(&envelope_value, application);
        let challenge =
            WorkerV2ApplicationHandoffChallengeV1::from_bytes([seed.max(1); 32]).unwrap();
        ApplicationDescriptorFixture {
            envelope_path,
            envelope,
            artifact_directory,
            acknowledgment_read,
            acknowledgment_write,
            expectation,
            challenge,
        }
    }

    #[cfg(target_os = "linux")]
    fn read_acknowledgment(descriptor: OwnedFd) -> Vec<u8> {
        let mut bytes = Vec::new();
        fs::File::from(descriptor).read_to_end(&mut bytes).unwrap();
        bytes
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn spoofed_canonical_ack_is_only_reproducible_liveness_data() {
        let fixture = recovery_fixture(28, "gfx942", "vecadd");
        let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&fixture.envelope).unwrap();
        let application =
            crate::application_descriptor_handoff::current_application_identity().unwrap();
        let expectation = WorkerV2ApplicationHandoffExpectationV1::new(&envelope, application);
        let challenge = WorkerV2ApplicationHandoffChallengeV1::from_bytes([29; 32]).unwrap();

        // A child that sees the protocol values can reproduce this exact canonical ACK without
        // performing host recovery. No host API accepts the resulting value as authority.
        let spoofed = expectation.acknowledgment(challenge).encode_canonical();
        WorkerV2ApplicationHandoffAckV1::decode_canonical(&spoofed)
            .unwrap()
            .validate(expectation, challenge)
            .unwrap();
    }

    #[cfg(target_os = "linux")]
    fn clear_close_on_exec(descriptor: RawFd) {
        // SAFETY: the caller retains ownership of this live descriptor while only its descriptor
        // flags are changed for an inherited-handoff test.
        assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_SETFD, 0) }, 0);
    }

    #[cfg(target_os = "linux")]
    fn duplicate_inheritable(descriptor: RawFd) -> OwnedFd {
        // SAFETY: `dup` creates one independently owned descriptor for this test process.
        let duplicate = unsafe { libc::dup(descriptor) };
        assert!(
            duplicate >= 0,
            "failed to duplicate descriptor {descriptor}"
        );
        // SAFETY: successful `dup` transferred ownership of this new descriptor number.
        let duplicate = unsafe { OwnedFd::from_raw_fd(duplicate) };
        clear_close_on_exec(duplicate.as_raw_fd());
        duplicate
    }

    #[cfg(target_os = "linux")]
    fn assert_close_on_exec(descriptor: RawFd) {
        // SAFETY: `F_GETFD` only queries the supplied live descriptor number.
        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
    }

    #[cfg(target_os = "linux")]
    fn assert_descriptor_closed(descriptor: RawFd) {
        // SAFETY: `F_GETFD` only queries the supplied descriptor number.
        assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }

    #[cfg(target_os = "linux")]
    fn handoff_environment_names() -> [&'static str; 5] {
        [
            fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
            fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
            fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
            fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
            fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
        ]
    }

    #[cfg(target_os = "linux")]
    fn descriptor_identity(descriptor: RawFd) -> (u64, u64) {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(format!("/proc/self/fd/{descriptor}")).unwrap();
        (metadata.dev(), metadata.ino())
    }

    #[cfg(target_os = "linux")]
    fn encoded_descriptor_identities(identities: &[(u64, u64)]) -> String {
        identities
            .iter()
            .map(|(device, inode)| format!("{device}:{inode}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    #[cfg(target_os = "linux")]
    fn assert_handoff_environment_scrubbed() {
        for name in handoff_environment_names() {
            assert_eq!(std::env::var_os(name), None, "{name} was not scrubbed");
        }
    }

    #[cfg(target_os = "linux")]
    fn run_descriptor_leak_probe() {
        use std::os::unix::fs::MetadataExt;
        let expected = std::env::var("FE2O3_TEST_HANDOFF_DESCRIPTOR_IDENTITIES")
            .unwrap()
            .split(',')
            .map(|identity| {
                let (device, inode) = identity.split_once(':').unwrap();
                (
                    device.parse::<u64>().unwrap(),
                    inode.parse::<u64>().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_handoff_environment_scrubbed();
        for entry in fs::read_dir("/proc/self/fd").unwrap() {
            let entry = entry.unwrap();
            let Ok(metadata) = fs::metadata(entry.path()) else {
                continue;
            };
            assert!(
                !expected.contains(&(metadata.dev(), metadata.ino())),
                "inherited handoff evidence {:?} leaked through descriptor {} -> {:?}",
                (metadata.dev(), metadata.ino()),
                entry.file_name().to_string_lossy(),
                fs::read_link(entry.path()).ok(),
            );
        }
    }

    #[cfg(target_os = "linux")]
    fn spawn_descriptor_leak_probe(identities: &[(u64, u64)]) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("recovered_worker_v2_admission::tests::inherited_handoff_scrubs_environment_and_descriptors_in_subprocesses")
            .arg("--nocapture")
            .env("FE2O3_TEST_HANDOFF_SUBPROCESS_MODE", "probe")
            .env(
                "FE2O3_TEST_HANDOFF_DESCRIPTOR_IDENTITIES",
                encoded_descriptor_identities(identities),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "descriptor leak probe failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(target_os = "linux")]
    fn run_inherited_handoff_subprocess(mode: &str) {
        let fixture = recovery_fixture(26, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 27);
        let expectation = descriptors.expectation;
        let challenge = descriptors.challenge;
        let envelope = descriptors.envelope.into_raw_fd();
        let artifact_directory = descriptors.artifact_directory.into_raw_fd();
        let acknowledgment_write = descriptors.acknowledgment_write.into_raw_fd();
        let identities = [
            descriptor_identity(envelope),
            descriptor_identity(artifact_directory),
            descriptor_identity(acknowledgment_write),
        ];
        for descriptor in [envelope, artifact_directory, acknowledgment_write] {
            clear_close_on_exec(descriptor);
        }
        let envelope_duplicate = duplicate_inheritable(envelope);
        let directory_duplicate = duplicate_inheritable(artifact_directory);
        let acknowledgment_duplicate = duplicate_inheritable(acknowledgment_write);
        // SAFETY: no other test or application code reads these protocol-specific variables; they
        // are removed immediately after the one-shot inherited consumer returns.
        unsafe {
            std::env::set_var(
                fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
                envelope.to_string(),
            );
            std::env::set_var(
                fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
                artifact_directory.to_string(),
            );
            std::env::set_var(
                fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
                acknowledgment_write.to_string(),
            );
            std::env::set_var(
                fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
                if mode != "failure" {
                    expectation.commitment().to_hex()
                } else {
                    "00".repeat(32)
                },
            );
            if mode != "missing" {
                std::env::set_var(
                    fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
                    challenge.to_hex(),
                );
            }
        }
        // SAFETY: this dedicated child has not created application threads or descendants.
        let recovered = unsafe {
            crate::consume_inherited_worker_v2_application_handoff_v1(
                fixture.compiler_transaction.clone(),
                fixture.kernel_id,
                &fixture.observed,
            )
        };
        assert_descriptor_closed(acknowledgment_write);
        assert_handoff_environment_scrubbed();
        for descriptor in [
            envelope_duplicate.as_raw_fd(),
            directory_duplicate.as_raw_fd(),
            acknowledgment_duplicate.as_raw_fd(),
        ] {
            assert_close_on_exec(descriptor);
        }
        // SAFETY: the same dedicated child still has no competing environment access. A repeated
        // call must scrub every value before rejecting the already-consumed handoff.
        unsafe {
            for name in handoff_environment_names() {
                std::env::set_var(name, "must-be-scrubbed-without-parsing");
            }
        }
        // SAFETY: as above, this is still single-threaded startup code.
        let repeated = unsafe {
            crate::consume_inherited_worker_v2_application_handoff_v1(
                fixture.compiler_transaction.clone(),
                fixture.kernel_id,
                &fixture.observed,
            )
        };
        assert!(matches!(
            repeated,
            Err(crate::WorkerV2ApplicationDescriptorHandoffErrorV1::AlreadyConsumed)
        ));
        assert_handoff_environment_scrubbed();
        spawn_descriptor_leak_probe(&identities);
        drop(acknowledgment_duplicate);
        if mode == "success" {
            let recovered = recovered.unwrap();
            let acknowledgment = read_acknowledgment(descriptors.acknowledgment_read);
            WorkerV2ApplicationHandoffAckV1::decode_canonical(&acknowledgment)
                .unwrap()
                .validate(expectation, challenge)
                .unwrap();
            recovered.revalidate_currentness().unwrap();
            drop(recovered);
        } else if mode == "failure" {
            assert!(matches!(
                recovered,
                Err(crate::WorkerV2ApplicationDescriptorHandoffErrorV1::CommitmentMismatch)
            ));
            assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
        } else {
            assert!(matches!(
                recovered,
                Err(
                    crate::WorkerV2ApplicationDescriptorHandoffErrorV1::MissingEnvironment(
                        fe2o3_worker_v2_bundle::WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1
                    )
                )
            ));
            assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn inherited_handoff_scrubs_environment_and_descriptors_in_subprocesses() {
        match std::env::var("FE2O3_TEST_HANDOFF_SUBPROCESS_MODE").as_deref() {
            Ok("probe") => run_descriptor_leak_probe(),
            Ok(mode @ ("success" | "failure" | "missing")) => {
                run_inherited_handoff_subprocess(mode)
            }
            Ok(mode) => panic!("unknown handoff subprocess mode {mode}"),
            Err(_) => {
                for mode in ["success", "failure", "missing"] {
                    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                    command
                        .arg("--exact")
                        .arg("recovered_worker_v2_admission::tests::inherited_handoff_scrubs_environment_and_descriptors_in_subprocesses")
                        .arg("--nocapture")
                        .env("FE2O3_TEST_HANDOFF_SUBPROCESS_MODE", mode);
                    for name in handoff_environment_names() {
                        command.env_remove(name);
                    }
                    let output = command.output().unwrap();
                    assert!(
                        output.status.success(),
                        "{mode} handoff subprocess failed:\nstdout:\n{}\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn application_descriptor_handoff_acks_and_survives_directory_rename_through_unload() {
        let fixture = recovery_fixture(30, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 31);
        let expectation = descriptors.expectation;
        let challenge = descriptors.challenge;
        let recovered = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            expectation.commitment(),
            challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let acknowledgment = read_acknowledgment(descriptors.acknowledgment_read);
        WorkerV2ApplicationHandoffAckV1::decode_canonical(&acknowledgment)
            .unwrap()
            .validate(expectation, challenge)
            .unwrap();

        let renamed = fixture._directory.0.join("renamed-output");
        fs::rename(&fixture.output, &renamed).unwrap();
        recovered.revalidate_currentness().unwrap();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let authority = recovered
            .load_generated_synchronous_hsa_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&mut authenticator, adapter)
            .unwrap();
        assert_eq!(authentication_calls.load(Ordering::SeqCst), 1);
        authority.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn package_and_receipt_commitment_substitution_is_rejected_without_ack() {
        let fixture = recovery_fixture(32, "gfx942", "vecadd");
        let substitute = recovery_fixture(33, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 34);
        let substitute_descriptors = application_descriptor_fixture(&substitute, 35);
        let error = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            substitute_descriptors.expectation.commitment(),
            descriptors.challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::WorkerV2ApplicationDescriptorHandoffErrorV1::CommitmentMismatch
        ));
        assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn artifact_directory_substitution_is_rejected_without_ack() {
        let fixture = recovery_fixture(36, "gfx942", "vecadd");
        let substitute = recovery_fixture(37, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 38);
        let substitute_descriptors = application_descriptor_fixture(&substitute, 39);
        let error = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            substitute_descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            descriptors.expectation.commitment(),
            descriptors.challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked
        ));
        assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn regular_file_ack_endpoint_is_rejected() {
        let fixture = recovery_fixture(44, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 45);
        drop(descriptors.acknowledgment_read);
        drop(descriptors.acknowledgment_write);
        let acknowledgment: OwnedFd = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(fixture.output.join("unsafe-ack.bin"))
            .unwrap()
            .into();
        let error = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            acknowledgment,
            descriptors.expectation.commitment(),
            descriptors.challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeAcknowledgment
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn symlink_alias_does_not_satisfy_exact_envelope_link() {
        let fixture = recovery_fixture(40, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 41);
        let displaced = fixture._directory.0.join("displaced-envelope.bin");
        fs::rename(&descriptors.envelope_path, &displaced).unwrap();
        std::os::unix::fs::symlink(&displaced, &descriptors.envelope_path).unwrap();
        let error = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            descriptors.expectation.commitment(),
            descriptors.challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked
        ));
        assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn same_directory_rename_and_noncanonical_name_are_rejected() {
        for (fixture_seed, challenge_seed, replacement) in
            [(46, 47, "renamed.envelope"), (48, 49, "uppercase")]
        {
            let fixture = recovery_fixture(fixture_seed, "gfx942", "vecadd");
            let descriptors = application_descriptor_fixture(&fixture, challenge_seed);
            let replacement = if replacement == "uppercase" {
                descriptors
                    .envelope_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_uppercase()
            } else {
                replacement.to_owned()
            };
            fs::rename(&descriptors.envelope_path, fixture.output.join(replacement)).unwrap();

            let error = consume_worker_v2_application_handoff_descriptors_v1(
                descriptors.envelope,
                descriptors.artifact_directory,
                descriptors.acknowledgment_write,
                descriptors.expectation.commitment(),
                descriptors.challenge,
                fixture.compiler_transaction.clone(),
                fixture.kernel_id,
                &fixture.observed,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                crate::WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked
            ));
            assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn duplicate_envelope_hard_link_is_rejected() {
        let fixture = recovery_fixture(50, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 51);
        fs::hard_link(
            &descriptors.envelope_path,
            fixture.output.join("duplicate-envelope-link"),
        )
        .unwrap();

        let error = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            descriptors.expectation.commitment(),
            descriptors.challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeEnvelope
        ));
        assert!(read_acknowledgment(descriptors.acknowledgment_read).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn retained_envelope_mutation_blocks_load_before_authentication() {
        let fixture = recovery_fixture(42, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&fixture, 43);
        let envelope_path = descriptors.envelope_path.clone();
        let recovered = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            descriptors.expectation.commitment(),
            descriptors.challenge,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let acknowledgment = read_acknowledgment(descriptors.acknowledgment_read);
        assert!(WorkerV2ApplicationHandoffAckV1::decode_canonical(&acknowledgment).is_ok());
        let mut envelope_bytes = fs::read(&envelope_path).unwrap();
        envelope_bytes[0] ^= 1;
        fs::write(&envelope_path, envelope_bytes).unwrap();

        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let error = recovered
            .load_generated_synchronous_hsa_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&mut authenticator, adapter)
            .unwrap_err();
        assert!(matches!(
            error,
            RecoveredWorkerV2SynchronousHsaHandoffError::ApplicationDescriptors(_)
        ));
        assert_eq!(authentication_calls.load(Ordering::SeqCst), 0);
        assert_eq!(unloads.load(Ordering::SeqCst), 0);
    }

    #[cfg(target_os = "linux")]
    #[derive(Clone, Copy)]
    enum PreparedDescriptorMutation {
        Rename,
        HardLink,
        SymlinkReplacement,
        DirentReplacement,
    }

    #[cfg(target_os = "linux")]
    fn assert_prepared_dispatch_rejects_descriptor_mutation(
        seed: u8,
        mutation: PreparedDescriptorMutation,
    ) {
        let descriptor_fixture = recovery_fixture(seed, "gfx942", "vecadd");
        let descriptors = application_descriptor_fixture(&descriptor_fixture, seed.wrapping_add(1));
        let envelope_path = descriptors.envelope_path.clone();
        let expectation = descriptors.expectation;
        let challenge = descriptors.challenge;
        let mut recovered = consume_worker_v2_application_handoff_descriptors_v1(
            descriptors.envelope,
            descriptors.artifact_directory,
            descriptors.acknowledgment_write,
            expectation.commitment(),
            challenge,
            descriptor_fixture.compiler_transaction.clone(),
            descriptor_fixture.kernel_id,
            &descriptor_fixture.observed,
        )
        .unwrap();
        let acknowledgment = read_acknowledgment(descriptors.acknowledgment_read);
        WorkerV2ApplicationHandoffAckV1::decode_canonical(&acknowledgment)
            .unwrap()
            .validate(expectation, challenge)
            .unwrap();
        let application_descriptors = recovered.application_descriptors.take().unwrap();
        drop(recovered);

        let (admission, _alpha_directory) =
            crate::worker_v2_bundle_admission::tests::admitted_alpha_cov6_for_lifecycle_test(
                seed.wrapping_add(2),
            );
        let observed =
            make_observed_for(usize::from(seed.wrapping_add(2)), "gfx942:sramecc+:xnack-");
        let (mut authenticator, _) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();
        let authenticated = AuthenticatedWorkerV2ExecutableV1::<AlphaCov6TestKernel>::authenticate(
            admission,
            &mut authenticator,
        )
        .unwrap();
        let currentness = authenticated.acquire_retained_currentness_token().unwrap();
        let authorized = authenticated.authorize_hsa_load(adapter).unwrap();
        let loaded = authorized
            .load_with_retained_currentness(&currentness)
            .unwrap();
        let mut authority = RecoveredWorkerV2SynchronousHsaHandoffV1 {
            loaded,
            currentness,
            observed: observed.clone(),
            application_descriptors: Some(application_descriptors),
        };
        let (arguments, drops) = alpha_cov6_arguments_for_lifecycle_test(&observed);
        let prepared = authority
            .prepare_generated_alpha_zeta_cov6_v1::<
                AlphaCov6TestKernel,
                ExactPrerequisiteAuthenticator,
                _,
            >(&mut authenticator, arguments)
            .unwrap();

        let displaced = descriptor_fixture
            .output
            .join(format!("displaced-after-prepare-{seed}.envelope"));
        match mutation {
            PreparedDescriptorMutation::Rename => {
                fs::rename(&envelope_path, &displaced).unwrap();
            }
            PreparedDescriptorMutation::HardLink => {
                fs::hard_link(&envelope_path, &displaced).unwrap();
            }
            PreparedDescriptorMutation::SymlinkReplacement => {
                fs::rename(&envelope_path, &displaced).unwrap();
                std::os::unix::fs::symlink(&displaced, &envelope_path).unwrap();
            }
            PreparedDescriptorMutation::DirentReplacement => {
                fs::rename(&envelope_path, &displaced).unwrap();
                let mut replacement = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&envelope_path)
                    .unwrap();
                replacement.write_all(&descriptor_fixture.envelope).unwrap();
                replacement.sync_all().unwrap();
            }
        }

        let error = match prepared.dispatch() {
            Ok(_) => panic!("descriptor mutation must prevent dispatch"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RecoveredWorkerV2SynchronousHsaDispatchError::ApplicationDescriptors(_)
        ));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        authority.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_dispatch_rejects_canonical_envelope_rename() {
        assert_prepared_dispatch_rejects_descriptor_mutation(
            80,
            PreparedDescriptorMutation::Rename,
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_dispatch_rejects_new_envelope_hard_link() {
        assert_prepared_dispatch_rejects_descriptor_mutation(
            84,
            PreparedDescriptorMutation::HardLink,
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_dispatch_rejects_canonical_symlink_replacement() {
        assert_prepared_dispatch_rejects_descriptor_mutation(
            88,
            PreparedDescriptorMutation::SymlinkReplacement,
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepared_dispatch_rejects_canonical_dirent_replacement() {
        assert_prepared_dispatch_rejects_descriptor_mutation(
            92,
            PreparedDescriptorMutation::DirentReplacement,
        );
    }

    #[test]
    fn canonical_envelope_recovers_one_inert_pinned_descriptor() {
        let fixture = recovery_fixture(1, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.compiler_transaction.clone(),
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
                    fixture.compiler_transaction.clone(),
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
                fixture.compiler_transaction.clone(),
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
                fixture.compiler_transaction.clone(),
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
                fixture.compiler_transaction.clone(),
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
                stale.compiler_transaction.clone(),
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
                first.compiler_transaction.clone(),
                first.kernel_id,
                &first.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Publication(_))
        ));
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &first.output,
                &first.envelope,
                first.compiler_transaction.clone(),
                second.kernel_id,
                &first.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::KernelNotFound)
        ));
    }

    #[test]
    fn compiler_transaction_substitution_is_rejected() {
        let first = recovery_fixture(9, "gfx942", "vecadd");
        let second = recovery_fixture(10, "gfx942", "vecadd");

        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &first.output,
                &first.envelope,
                second.compiler_transaction.clone(),
                first.kernel_id,
                &first.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Admission(
                FinalizedWorkerV2BundleAdmissionError::CompilerTransactionLineageMismatch(_)
            ))
        ));
    }

    #[test]
    fn raw_final_and_physical_kernel_substitution_are_rejected() {
        let raw_substitution = recovery_fixture(6, "gfx950", "vecadd");
        assert!(matches!(
            recover_worker_v2_load_envelope_v1(
                &raw_substitution.output,
                &raw_substitution.envelope,
                raw_substitution.compiler_transaction.clone(),
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
                physical_substitution.compiler_transaction.clone(),
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
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let expected_digest = recovered.artifact_identity().payload_digest();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();

        let authority = recovered
            .load_generated_synchronous_hsa_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&mut authenticator, adapter)
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
    fn retained_currentness_blocks_generation_turnover_through_unload() {
        const SEED: u8 = 0xa5;
        let (admission, directory) =
            crate::worker_v2_bundle_admission::tests::admitted_alpha_cov6_for_lifecycle_test(SEED);
        let observed = make_observed_for(SEED.into(), "gfx942:sramecc+:xnack-");
        let (mut authenticator, _) = ExactPrerequisiteAuthenticator::new();
        let turnover_completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (adapter, unloads) = ExactHsaAdapter::with_turnover_probe(turnover_completed.clone());
        let authenticated = AuthenticatedWorkerV2ExecutableV1::<AlphaCov6TestKernel>::authenticate(
            admission,
            &mut authenticator,
        )
        .unwrap();
        let currentness = authenticated.acquire_retained_currentness_token().unwrap();
        let authorized = authenticated.authorize_hsa_load(adapter).unwrap();
        let loaded = authorized
            .load_with_retained_currentness(&currentness)
            .unwrap();
        let mut authority = RecoveredWorkerV2SynchronousHsaHandoffV1 {
            loaded,
            currentness,
            observed: observed.clone(),
            #[cfg(target_os = "linux")]
            application_descriptors: None,
        };

        let output = directory.path().to_path_buf();
        let owner = ProducerIdentity::from_codegen(
            "fe2o3_host_worker_v2_admission",
            Some(Path::new("tests/worker_v2_bundle_admission.rs")),
        )
        .unwrap();
        let completed = turnover_completed.clone();
        let turnover_owner = owner.clone();
        let lock_probe = install_begin_build_attempt_lock_probe_v1(&output, &owner);
        let (completed_tx, completed_rx) = std::sync::mpsc::channel();
        let turnover = std::thread::spawn(move || {
            let next = begin_build_attempt(
                &output,
                &turnover_owner,
                BuildInvocation::from_bytes([0xd1; 32]),
                BuildSession::from_bytes([0xd2; 16]),
            );
            completed.store(true, std::sync::atomic::Ordering::SeqCst);
            completed_tx.send(()).unwrap();
            next
        });
        lock_probe.wait_until_contended();

        let (arguments, drops) = alpha_cov6_arguments_for_lifecycle_test(&observed);
        let prepared = authority
            .prepare_generated_alpha_zeta_cov6_v1::<
                AlphaCov6TestKernel,
                ExactPrerequisiteAuthenticator,
                _,
            >(&mut authenticator, arguments)
            .unwrap();
        assert_eq!(prepared.geometry().grid(), [2, 1, 1]);
        let completion = prepared.dispatch().unwrap();
        assert!(completion.completed_dispatch().dispatch().completed());
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);

        authority.unload().unwrap();
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 1);
        completed_rx.recv().unwrap();
        let next = turnover.join().unwrap().unwrap();
        assert_eq!(next.generation(), 2);
        fail_build_attempt(directory.path(), &owner, next).unwrap();
    }

    #[test]
    fn wrong_marker_is_rejected_before_the_unsafe_authenticator() {
        let fixture = recovery_fixture(21, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        let (mut authenticator, authentication_calls) = ExactPrerequisiteAuthenticator::new();
        let (adapter, unloads) = ExactHsaAdapter::new();

        let error = recovered
            .load_generated_synchronous_hsa_handoff_v1::<
                WrongMarker,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&mut authenticator, adapter)
            .unwrap_err();

        assert!(matches!(
            error,
            RecoveredWorkerV2SynchronousHsaHandoffError::Selection(_)
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
            fixture.compiler_transaction.clone(),
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
            .load_generated_synchronous_hsa_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&mut authenticator, adapter)
            .unwrap_err();

        assert!(matches!(
            error,
            RecoveredWorkerV2SynchronousHsaHandoffError::CurrentPublication(_)
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
            fixture.compiler_transaction.clone(),
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
            .load_generated_synchronous_hsa_handoff_v1::<
                HandoffKernel,
                ExactPrerequisiteAuthenticator,
                ExactHsaAdapter,
            >(&mut authenticator, adapter)
            .unwrap_err();

        assert!(matches!(
            error,
            RecoveredWorkerV2SynchronousHsaHandoffError::CurrentPublication(_)
        ));
        assert_eq!(
            authentication_calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(unloads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[cfg(unix)]
    #[test]
    fn same_byte_artifact_replacement_invalidates_an_existing_descriptor() {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let fixture = recovery_fixture(8, "gfx942", "vecadd");
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.compiler_transaction.clone(),
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
                fixture.compiler_transaction.clone(),
                fixture.kernel_id,
                &fixture.observed,
            ),
            Err(RecoveredWorkerV2AdmissionError::Publication(_))
        ));
    }

    fn recovered_launch_bridge_fixture(
        seed: u8,
    ) -> (RecoveryFixture, RecoveredWorkerV2PinnedDescriptorV1) {
        recovered_launch_bridge_fixture_with_explicit_argument_alignments(seed, true)
    }

    fn recovered_launch_bridge_fixture_with_explicit_argument_alignments(
        seed: u8,
        include_explicit_argument_alignments: bool,
    ) -> (RecoveryFixture, RecoveredWorkerV2PinnedDescriptorV1) {
        let fixture = recovery_fixture_with_explicit_argument_alignments(
            seed,
            "gfx942",
            "vecadd",
            include_explicit_argument_alignments,
        );
        let recovered = recover_worker_v2_load_envelope_v1(
            &fixture.output,
            &fixture.envelope,
            fixture.compiler_transaction.clone(),
            fixture.kernel_id,
            &fixture.observed,
        )
        .unwrap();
        (fixture, recovered)
    }

    #[test]
    fn launch_bridge_rejects_unknown_physical_argument_alignment_without_authority() {
        let (_fixture, recovered) =
            recovered_launch_bridge_fixture_with_explicit_argument_alignments(88, false);
        let (_model_fixture, model_recovered) = recovered_launch_bridge_fixture(88);
        let family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &model_recovered,
            );

        let current = recovered.acquire_launch_kernel_v2_currentness().unwrap();
        assert!(
            current
                .admission()
                .selected_kernel()
                .arguments()
                .iter()
                .all(|argument| matches!(
                    argument.alignment(),
                    crate::PhysicalMetadataValueV1::Unknown
                ))
        );
        drop(current);

        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &family,
                "recovered-exact-wave64"
            ),
            Err(
                crate::LaunchKernelMetadataBridgeErrorV2::MissingPhysicalMetadata(
                    "physical argument alignment"
                )
            )
        ));
    }

    #[test]
    fn launch_bridge_binds_exact_recovered_physical_metadata_without_authority() {
        let (_fixture, recovered) = recovered_launch_bridge_fixture(80);
        let family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        let binding = crate::bind_current_recovered_launch_kernel_metadata_v2(
            &recovered,
            &family,
            "recovered-exact-wave64",
        )
        .unwrap();

        assert_eq!(binding.target(), family.target);
        assert_eq!(
            binding.artifact_identity(),
            family.variants[0].artifact_identity
        );
        assert_eq!(
            binding.kernel_identity(),
            family.variants[0].kernel_identity
        );
        assert_eq!(binding.signature(), &family.signature);
        assert_eq!(binding.signature().explicit_argument_bytes, 16);
        assert_eq!(binding.signature().kernarg_segment_bytes, 272);
        assert_eq!(binding.signature().kernarg_segment_alignment, 8);
        assert_eq!(binding.signature().parameters.len(), 2);
        assert_eq!(
            binding.signature().parameters[0].kind,
            fe2o3_kernel_ir::AbiParameterKindV2::SharedGlobalPointer
        );
        assert_eq!(
            binding.signature().parameters[1].kind,
            fe2o3_kernel_ir::AbiParameterKindV2::ByValue
        );
        assert_eq!(binding.resources(), family.variants[0].resources);
        assert_eq!(
            binding.occupancy_subject(),
            family.variants[0]
                .occupancy_witness
                .unwrap()
                .subject_identity
        );
        assert_eq!(binding.variant_name(), "recovered-exact-wave64");
        assert!(!binding.authenticates_compiler_or_verus_provenance());
        assert!(!binding.authenticates_rust_type_or_effect_semantics());
        assert!(!binding.authenticates_policy_or_proof_claims());
        assert!(!binding.grants_load_authority());
        assert!(!binding.grants_dispatch_authority());
        assert_eq!(
            binding.require_occupancy_dependent_admission(),
            Err(
                crate::OccupancyDependentLaunchAdmissionErrorV2::PhysicalOccupancyUnavailable(
                    crate::Gfx942OccupancyMetadataStatusV2::NoReviewedPhysicalDerivation
                )
            )
        );
    }

    #[test]
    fn launch_bridge_rejects_every_caller_identity_substitution() {
        let (_fixture, recovered) = recovered_launch_bridge_fixture(81);
        let canonical =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );

        let mut candidate = canonical.clone();
        candidate.target.identity = fe2o3_kernel_ir::TargetIdentityV2::from_bytes([0x81; 32]);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::TargetSubstitution)
        ));

        candidate = canonical.clone();
        candidate.logical_name = "substituted-logical-name".to_owned();
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::LogicalNameSubstitution)
        ));

        candidate = canonical.clone();
        candidate.variants[0].entry_name = "substituted_entry".to_owned();
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::EntryNameSubstitution)
        ));

        candidate = canonical.clone();
        candidate.variants[0].artifact_identity =
            fe2o3_kernel_ir::ArtifactIdentityV2::from_bytes([0x82; 32]);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::ArtifactSubstitution)
        ));

        candidate = canonical.clone();
        candidate.variants[0].kernel_identity =
            fe2o3_kernel_ir::KernelIdentityV2::from_bytes([0x83; 32]);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::KernelSubstitution)
        ));

        candidate = canonical;
        candidate.signature.identity =
            fe2o3_kernel_ir::KernelSignatureIdentityV2::from_bytes([0x84; 32]);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::SignatureSubstitution)
        ));

        candidate =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        candidate.signature.parameters[0].semantic_type =
            fe2o3_kernel_ir::SemanticTypeIdentityV2::from_bytes([0x85; 32]);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::SignatureSubstitution)
        ));
    }

    #[test]
    fn launch_bridge_rejects_resource_geometry_and_limit_substitution() {
        let (_fixture, recovered) = recovered_launch_bridge_fixture(82);
        let canonical =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );

        let mut candidate = canonical.clone();
        candidate.variants[0].resources.private_segment_bytes += 1;
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::ResourceSubstitution)
        ));

        candidate = canonical.clone();
        candidate.variants[0].launch.max_grid_blocks.x -= 1;
        candidate.variants[0].launch.max_total_workitems =
            u64::from(candidate.variants[0].launch.max_grid_blocks.x)
                * u64::from(candidate.variants[0].launch.maximum_flat_workgroup_size);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::LaunchGeometrySubstitution)
        ));

        candidate = canonical.clone();
        candidate.variants[0].resources.static_lds_bytes = 65_537;
        candidate.variants[0]
            .capabilities
            .push(fe2o3_kernel_ir::LaunchCapabilityV2::StaticLds);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(
                crate::LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel(
                    fe2o3_kernel_ir::LaunchKernelValidationErrorV2::LdsLimitExceeded
                )
            )
        ));

        candidate = canonical;
        candidate.variants[0].resources.private_segment_bytes = 1_048_577;
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut candidate);
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &candidate,
                "recovered-exact-wave64"
            ),
            Err(
                crate::LaunchKernelMetadataBridgeErrorV2::InvalidLaunchModel(
                    fe2o3_kernel_ir::LaunchKernelValidationErrorV2::PrivateSegmentLimitExceeded
                )
            )
        ));
    }

    #[test]
    fn launch_bridge_preflights_non_exact_policy_before_model_enumeration() {
        let (_fixture, recovered) = recovered_launch_bridge_fixture(86);
        let mut family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        family.variants[0].launch.block = fe2o3_kernel_ir::BlockShapePolicyV2::Bounded {
            minimum: fe2o3_kernel_ir::DimensionsV2::new(1, 1, 1),
            maximum: fe2o3_kernel_ir::DimensionsV2::new(1_024, 1_024, 1_024),
        };

        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &family,
                "recovered-exact-wave64"
            ),
            Err(
                crate::LaunchKernelMetadataBridgeErrorV2::UnsupportedPhysicalLaunchContract(
                    "non-exact block policy"
                )
            )
        ));
    }

    #[test]
    fn caller_occupancy_authority_identities_are_discarded() {
        let (_fixture, recovered) = recovered_launch_bridge_fixture(83);
        let mut family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        let first = crate::bind_current_recovered_launch_kernel_metadata_v2(
            &recovered,
            &family,
            "recovered-exact-wave64",
        )
        .unwrap();
        let subject = first.occupancy_subject();
        drop(first);

        let witness = family.variants[0].occupancy_witness.as_mut().unwrap();
        witness.verifier_identity =
            fe2o3_kernel_ir::OccupancyVerifierIdentityV2::from_bytes([0xe1; 32]);
        witness.metadata_identity =
            fe2o3_kernel_ir::OccupancyMetadataIdentityV2::from_bytes([0xe2; 32]);
        crate::launch_kernel_v2_bridge::rebind_launch_family_for_bridge_test(&mut family);
        let rebound = crate::bind_current_recovered_launch_kernel_metadata_v2(
            &recovered,
            &family,
            "recovered-exact-wave64",
        )
        .unwrap();
        assert_eq!(rebound.occupancy_subject(), subject);
        assert_eq!(
            rebound.occupancy_status(),
            crate::Gfx942OccupancyMetadataStatusV2::NoReviewedPhysicalDerivation
        );
        assert!(rebound.require_occupancy_dependent_admission().is_err());
    }

    #[test]
    fn launch_bridge_revalidates_publication_before_matching_metadata() {
        let (fixture, recovered) = recovered_launch_bridge_fixture(84);
        let family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        let artifact = managed_artifact(&fixture.output);
        let mut bytes = fs::read(&artifact).unwrap();
        bytes[0] ^= 0xff;
        fs::write(artifact, bytes).unwrap();

        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(
                &recovered,
                &family,
                "recovered-exact-wave64"
            ),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::CurrentPublication(_))
        ));
    }

    #[test]
    fn launch_bridge_rejects_unknown_variant_before_acquiring_authority() {
        let (_fixture, recovered) = recovered_launch_bridge_fixture(85);
        let family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        assert!(matches!(
            crate::bind_current_recovered_launch_kernel_metadata_v2(&recovered, &family, "absent"),
            Err(crate::LaunchKernelMetadataBridgeErrorV2::UnknownVariant)
        ));
    }

    #[test]
    fn launch_bridge_holds_current_publication_until_the_binding_drops() {
        let (fixture, recovered) = recovered_launch_bridge_fixture(87);
        let family =
            crate::launch_kernel_v2_bridge::canonical_family_for_recovered_launch_bridge_test(
                &recovered,
            );
        let binding = crate::bind_current_recovered_launch_kernel_metadata_v2(
            &recovered,
            &family,
            "recovered-exact-wave64",
        )
        .unwrap();

        let output = fixture.output.clone();
        let owner = fixture.owner.clone();
        let turnover_owner = owner.clone();
        let lock_probe = install_begin_build_attempt_lock_probe_v1(&output, &owner);
        let (completed_tx, completed_rx) = std::sync::mpsc::channel();
        let turnover = std::thread::spawn(move || {
            let next = begin_build_attempt(
                &output,
                &turnover_owner,
                BuildInvocation::from_bytes([0xd3; 32]),
                BuildSession::from_bytes([0xd4; 16]),
            );
            completed_tx.send(()).unwrap();
            next
        });
        lock_probe.wait_until_contended();
        assert!(matches!(
            completed_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        drop(binding);
        completed_rx.recv().unwrap();
        let next = turnover.join().unwrap().unwrap();
        assert_eq!(next.generation(), 2);
        fail_build_attempt(&fixture.output, &owner, next).unwrap();
    }
}
