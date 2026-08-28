use crate::{
    BlockSizeV1, DeviceIdentity, DimensionsV1, KernelId, LaunchConstraintsV1, ObservedContext,
};
use fe2o3_amd_target::{AmdTargetId, ParseAmdTargetIdError};
use fe2o3_artifacts::{
    AbiLayout, BlockSize, Capability, CodeObjectIdentity, DigestBytes, Endianness, HostLaunchAbi,
    HostLaunchAbiError, LaunchContract, Name, PayloadDigest, PointerWidth, SelectedNativeKernel,
    TargetIdentity,
};
use fe2o3_device::KernelMarkerV1;
use fe2o3_kernel_descriptor::ValidationError as DescriptorValidationError;
use reserved_fe2o3_symbols::{
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
};
use std::fmt;
use std::sync::Arc;

const AMDGPU_TRIPLE: &str = "amdgcn-amd-amdhsa";

/// Version of the exact artifact identity carried by the G3 host bridge.
pub const ARTIFACT_KERNEL_IDENTITY_VERSION: u16 = 1;

/// Exact, owned identity of one validated native-kernel selection.
///
/// Values can only be obtained from [`ValidatedArtifactSelectionV1`]. Equality
/// covers the canonical manifest digest, kernel and code-object digests, names,
/// target, ABI, source launch contract, and the conservative launch constraints
/// derived for the observed device. It does not establish artifact authenticity
/// or prove that declarations match the executable payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactKernelIdentityV1 {
    manifest_digest: PayloadDigest,
    kernel_id: KernelId,
    name: Name,
    symbol: Name,
    source_digest: DigestBytes,
    executable_digest: DigestBytes,
    code_object: CodeObjectIdentity,
    payload_digest: PayloadDigest,
    target: TargetIdentity,
    required_capabilities: Vec<Capability>,
    abi: AbiLayout,
    launch: LaunchContract,
    effective_launch: LaunchConstraintsV1,
}

impl ArtifactKernelIdentityV1 {
    pub const fn version(&self) -> u16 {
        ARTIFACT_KERNEL_IDENTITY_VERSION
    }

    pub const fn manifest_digest(&self) -> PayloadDigest {
        self.manifest_digest
    }

    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn name(&self) -> &Name {
        &self.name
    }

    pub const fn symbol(&self) -> &Name {
        &self.symbol
    }

    pub const fn source_digest(&self) -> DigestBytes {
        self.source_digest
    }

    pub const fn executable_digest(&self) -> DigestBytes {
        self.executable_digest
    }

    pub const fn code_object(&self) -> &CodeObjectIdentity {
        &self.code_object
    }

    pub const fn payload_digest(&self) -> PayloadDigest {
        self.payload_digest
    }

    pub const fn target(&self) -> &TargetIdentity {
        &self.target
    }

    pub fn required_capabilities(&self) -> &[Capability] {
        &self.required_capabilities
    }

    pub const fn abi(&self) -> &AbiLayout {
        &self.abi
    }

    pub const fn launch(&self) -> &LaunchContract {
        &self.launch
    }

    pub const fn effective_launch(&self) -> &LaunchConstraintsV1 {
        &self.effective_launch
    }
}

/// A validated native artifact selection bound to one exact observed context.
///
/// This public token is intentionally non-generic: structural artifact and ABI
/// validation cannot establish a relationship to an arbitrary Rust marker
/// type. It retains exact identity and payload bytes but contains no runtime
/// handle and exposes no launch operation.
///
/// [`HostLaunchAbi`] validation performed during construction checks only the
/// manifest's structural ABI subset. It does not match compiler-generated host
/// argument types, layouts, or lifetimes.
pub struct ValidatedArtifactSelectionV1 {
    pub(crate) identity: Arc<ArtifactKernelIdentityV1>,
    pub(crate) payload: Arc<[u8]>,
    context: ObservedContext,
}

impl fmt::Debug for ValidatedArtifactSelectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedArtifactSelectionV1")
            .field("identity", &self.identity)
            .field("payload_len", &self.payload.len())
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl ValidatedArtifactSelectionV1 {
    /// Validates one native artifact selection for an observed context.
    ///
    /// The bridge supports canonical, compatible AMDGPU/64-bit/little-endian
    /// targets. Omitted artifact target-feature states accept observed explicit
    /// states; explicit artifact states must match the observation. Of the
    /// current coarse manifest capabilities, only workgroup memory has enough
    /// HIP observation and launch-contract detail to be admitted. It validates
    /// the ABI structurally against the conservative host-launch subset and
    /// derives launch constraints bounded by observed device limits.
    pub fn validate(
        selected: SelectedNativeKernel<'_>,
        context: &ObservedContext,
    ) -> Result<Self, ArtifactBindingError> {
        let identity = identity_from_selection(selected, context)?;
        Ok(Self {
            identity: Arc::new(identity),
            payload: Arc::from(selected.payload()),
            context: context.clone(),
        })
    }

    pub fn identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.identity
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.context.device()
    }

    /// Revalidates that a later selection and observation are exactly the ones
    /// represented by this token.
    pub fn revalidate(
        &self,
        selected: SelectedNativeKernel<'_>,
        context: &ObservedContext,
    ) -> Result<(), ArtifactRevalidationError> {
        if context.device() != self.context.device() {
            return Err(ArtifactRevalidationError::WrongDevice);
        }
        if !context.same_context(&self.context) {
            return Err(ArtifactRevalidationError::WrongContext);
        }
        if !context.same_launch_limits(&self.context) {
            return Err(ArtifactRevalidationError::DeviceLimitsChanged);
        }
        if !context.same_hip_capabilities(&self.context) {
            return Err(ArtifactRevalidationError::DeviceCapabilitiesChanged);
        }

        let actual = identity_from_selection(selected, context)
            .map_err(ArtifactRevalidationError::Binding)?;
        if actual != *self.identity {
            return Err(ArtifactRevalidationError::WrongArtifactIdentity);
        }
        if selected.payload() != self.payload.as_ref() {
            return Err(ArtifactRevalidationError::WrongArtifactIdentity);
        }
        Ok(())
    }
}

/// Validated semantic authority for one compiler-generated kernel expectation.
///
/// This value is intentionally opaque. Implementations receive one only after
/// parsing the exact backend-issued witness bound to their kernel and generated
/// host-contract identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct ValidatedCompilerGeneratedSemanticWitnessV1 {
    profile: CompilerGeneratedKernelProfileV1,
    kernel_binding: [u8; 32],
    generated_host_contract: [u8; 32],
}

impl ValidatedCompilerGeneratedSemanticWitnessV1 {
    const fn general_v3(kernel_binding: [u8; 32], generated_host_contract: [u8; 32]) -> Self {
        Self {
            profile: CompilerGeneratedKernelProfileV1::new(generated_host_contract),
            kernel_binding,
            generated_host_contract,
        }
    }
}

/// Failure while obtaining or validating compiler-generated semantic authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum CompilerGeneratedSemanticWitnessErrorV1 {
    MissingBackendWitness,
    InvalidPointer,
    InvalidLength,
    MagicMismatch,
    VersionMismatch,
    DomainMismatch,
    KernelBindingMismatch,
    GeneratedHostContractMismatch,
    ProfileTagMismatch,
    TrailingBytes,
    WitnessSubstitution,
}

impl fmt::Display for CompilerGeneratedSemanticWitnessErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingBackendWitness => {
                "the general typed kernel has no backend-issued semantic witness"
            }
            Self::InvalidPointer => "the backend semantic-witness pointer is invalid",
            Self::InvalidLength => "the backend semantic-witness length is invalid",
            Self::MagicMismatch => "the backend semantic-witness magic does not match",
            Self::VersionMismatch => "the backend semantic-witness version does not match",
            Self::DomainMismatch => "the backend semantic-witness domain does not match",
            Self::KernelBindingMismatch => {
                "the backend semantic witness names a different kernel binding"
            }
            Self::GeneratedHostContractMismatch => {
                "the backend semantic witness names a different generated host contract"
            }
            Self::ProfileTagMismatch => "the backend semantic-witness profile tag does not match",
            Self::TrailingBytes => "the backend semantic witness contains trailing bytes",
            Self::WitnessSubstitution => {
                "the backend semantic witness was substituted across expectations"
            }
        })
    }
}

impl std::error::Error for CompilerGeneratedSemanticWitnessErrorV1 {}

/// Trusted generated expectation for one compiler-generated kernel.
///
/// The associated constants are a frontend declaration of the expected host
/// ABI, effects, launch, and kernel binding. They are not by themselves proof
/// that rustc accepted those semantics. Production Worker V3 admission matches
/// the binding and generated argument layout to the independently admitted
/// compiler descriptor. The trait deliberately carries no artifact bytes.
///
/// # Safety
///
/// The profile and binding identity must describe `Self::FUNCTION` exactly,
/// including the complete physical host ABI, memory effects, launch contract,
/// and all behavior relevant to safe loading and dispatch. Implementations are
/// an explicit unsafe trust boundary. A false implementation can authorize
/// dispatch of native code under the wrong Rust signature or safety contract.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedKernelExpectationV1: KernelMarkerV1 {
    /// Versioned host ABI and memory-effect profile expected by generated code.
    const PROFILE: CompilerGeneratedKernelProfileV1;

    /// Full backend-validated identity used by private host linker symbols.
    const KERNEL_BINDING_ID_V1: [u8; 32];

    /// Obtains the backend-issued witness for this exact expectation.
    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        Err(CompilerGeneratedSemanticWitnessErrorV1::MissingBackendWitness)
    }
}

/// Metadata for one marker in an exact compiler-generated kernel roster.
///
/// This value carries no artifact bytes and grants no verification, load, or
/// launch authority. Host admission compares the complete ordered roster with
/// the independently recovered compiler descriptor table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompilerGeneratedKernelExpectationRosterEntryV1 {
    logical_name: &'static str,
    export_name: &'static str,
    kernel_binding_id: [u8; 32],
    generated_host_contract_identity: [u8; 32],
}

impl CompilerGeneratedKernelExpectationRosterEntryV1 {
    pub(crate) const fn from_parts(
        logical_name: &'static str,
        export_name: &'static str,
        kernel_binding_id: [u8; 32],
        generated_host_contract_identity: [u8; 32],
    ) -> Self {
        Self {
            logical_name,
            export_name,
            kernel_binding_id,
            generated_host_contract_identity,
        }
    }

    #[doc(hidden)]
    pub const fn for_marker<K: CompilerGeneratedKernelExpectationV1>() -> Self {
        Self::from_parts(
            K::LOGICAL_NAME,
            K::EXPORT_NAME,
            K::KERNEL_BINDING_ID_V1,
            K::PROFILE.generated_host_contract_identity(),
        )
    }

    pub const fn logical_name(&self) -> &'static str {
        self.logical_name
    }

    pub const fn export_name(&self) -> &'static str {
        self.export_name
    }

    pub const fn kernel_binding_id(&self) -> [u8; 32] {
        self.kernel_binding_id
    }

    pub const fn generated_host_contract_identity(&self) -> [u8; 32] {
        self.generated_host_contract_identity
    }
}

/// Exact ordered set of compiler-generated kernel expectations for one artifact.
///
/// Implementations are metadata only. They grant no authority and are checked
/// against the complete receipt-bound compiler descriptor table during host
/// admission. The generated host-contract identity is retained for the later
/// sealed verification transition; descriptor admission itself matches only
/// the ordered logical name, export name, and kernel binding carried on both
/// boundaries. Prefer [`compiler_generated_kernel_expectation_roster_v1!`] so
/// every entry is derived directly from its generated marker.
#[doc(hidden)]
pub trait CompilerGeneratedKernelExpectationRosterV1: Send + Sync + 'static {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1];
}

/// Declares an exact ordered roster from compiler-generated kernel markers.
#[macro_export]
#[doc(hidden)]
macro_rules! compiler_generated_kernel_expectation_roster_v1 {
    (
        $(#[$metadata:meta])*
        $visibility:vis struct $roster:ident = [$($marker:ty),+ $(,)?];
    ) => {
        $(#[$metadata])*
        $visibility struct $roster;

        impl $crate::CompilerGeneratedKernelExpectationRosterV1 for $roster {
            const ENTRIES: &'static [
                $crate::CompilerGeneratedKernelExpectationRosterEntryV1
            ] = &[
                $(
                    $crate::CompilerGeneratedKernelExpectationRosterEntryV1::for_marker::<
                        $marker
                    >()
                ),+
            ];
        }
    };
}

/// Obtains an opaque semantic-authority token for one exact generated
/// expectation and rejects cross-kernel token substitution.
#[doc(hidden)]
pub fn validate_compiler_generated_semantic_witness_v1<K: CompilerGeneratedKernelExpectationV1>()
-> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    let witness = K::semantic_witness_v1()?;
    if witness.profile != K::PROFILE
        || witness.kernel_binding != K::KERNEL_BINDING_ID_V1
        || witness.generated_host_contract != K::PROFILE.generated_host_contract_identity()
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::WitnessSubstitution);
    }
    Ok(witness)
}

/// Parses the immutable witness bytes returned by one reserved backend accessor
/// pair and binds them to an exact general typed V3 expectation.
///
/// # Safety
///
/// `pointer` must be non-null and point to one live, immutable allocation of
/// exactly `length` initialized bytes. The allocation must remain live and
/// immutable for the entire call. The range must not wrap the address space.
/// Only compiler-generated unsafe trait implementations may call this function
/// with values returned by their exact backend-owned accessor pair.
#[doc(hidden)]
pub unsafe fn semantic_witness_from_backend_v1(
    pointer: *const u8,
    length: usize,
    expected_kernel_binding: [u8; 32],
    expected_generated_host_contract: [u8; 32],
) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    if pointer.is_null() {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidPointer);
    }
    if !(GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        ..=MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1)
        .contains(&length)
        || length > isize::MAX as usize
        || pointer.addr().checked_add(length).is_none()
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }

    // SAFETY: the caller establishes the allocation, initialization,
    // immutability, range, and lifetime requirements above.
    let bytes = unsafe { core::slice::from_raw_parts(pointer, length) };
    parse_general_typed_v3_semantic_witness_v1(
        bytes,
        expected_kernel_binding,
        expected_generated_host_contract,
    )
}

fn parse_general_typed_v3_semantic_witness_v1(
    bytes: &[u8],
    expected_kernel_binding: [u8; 32],
    expected_generated_host_contract: [u8; 32],
) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    if !(GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        ..=MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1)
        .contains(&bytes.len())
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }

    let magic = u64::from_le_bytes(bytes[0..8].try_into().expect("fixed witness magic range"));
    if magic != GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1 {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::MagicMismatch);
    }
    let version = u16::from_le_bytes(
        bytes[8..10]
            .try_into()
            .expect("fixed witness version range"),
    );
    if version != GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1 {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::VersionMismatch);
    }
    let domain = u16::from_le_bytes(
        bytes[10..12]
            .try_into()
            .expect("fixed witness domain range"),
    );
    if domain != GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1 {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::DomainMismatch);
    }

    let declared_length = usize::try_from(u32::from_le_bytes(
        bytes[12..16]
            .try_into()
            .expect("fixed witness length range"),
    ))
    .map_err(|_| CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)?;
    if !(GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        ..=MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1)
        .contains(&declared_length)
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }
    if bytes.len() > declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes);
    }
    if bytes.len() != declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }

    if bytes[16..48] != expected_kernel_binding {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::KernelBindingMismatch);
    }
    if bytes[48..80] != expected_generated_host_contract {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::GeneratedHostContractMismatch);
    }

    let profile_length = usize::from(u16::from_le_bytes(
        bytes[80..82]
            .try_into()
            .expect("fixed witness profile-length range"),
    ));
    let profile_end = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        .checked_add(profile_length)
        .ok_or(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)?;
    if profile_end < declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes);
    }
    if profile_end != declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }
    if bytes[GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1..profile_end]
        != *TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes()
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::ProfileTagMismatch);
    }

    Ok(ValidatedCompilerGeneratedSemanticWitnessV1::general_v3(
        expected_kernel_binding,
        expected_generated_host_contract,
    ))
}

/// Exact generated host contract understood by this runtime version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompilerGeneratedKernelProfileV1 {
    generated_host_contract_identity: [u8; 32],
}

impl CompilerGeneratedKernelProfileV1 {
    pub const fn new(generated_host_contract_identity: [u8; 32]) -> Self {
        Self {
            generated_host_contract_identity,
        }
    }

    pub const fn generated_host_contract_identity(self) -> [u8; 32] {
        self.generated_host_contract_identity
    }
}

/// Failure while binding a validated artifact selection to an observed device.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArtifactBindingError {
    UnsupportedTargetTriple(String),
    InvalidArtifactTargetId {
        target: String,
        error: ParseAmdTargetIdError,
    },
    IncompatibleAmdTarget {
        artifact: AmdTargetId,
        observed: AmdTargetId,
    },
    UnsupportedPointerWidth(PointerWidth),
    UnsupportedEndianness(Endianness),
    RequiredCapabilityUnavailable(Capability),
    InsufficientCapabilityObservation(Capability),
    UnsupportedRequiredCapability(Capability),
    PayloadDigestMismatch,
    UnsupportedHostAbi(HostLaunchAbiError),
    LaunchContract(ArtifactLaunchContractError),
}

impl fmt::Display for ArtifactBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTargetTriple(triple) => {
                write!(formatter, "unsupported artifact target triple {triple}")
            }
            Self::InvalidArtifactTargetId { target, error } => {
                write!(
                    formatter,
                    "invalid artifact AMD target ID {target:?}: {error}"
                )
            }
            Self::IncompatibleAmdTarget { artifact, observed } => write!(
                formatter,
                "artifact AMD target {artifact} is incompatible with observed target {observed}"
            ),
            Self::UnsupportedPointerWidth(width) => {
                write!(formatter, "unsupported artifact pointer width {width:?}")
            }
            Self::UnsupportedEndianness(endianness) => {
                write!(formatter, "unsupported artifact endianness {endianness:?}")
            }
            Self::RequiredCapabilityUnavailable(capability) => write!(
                formatter,
                "the observed context does not provide required capability {capability:?}"
            ),
            Self::InsufficientCapabilityObservation(capability) => write!(
                formatter,
                "HIP observations are too coarse to establish required capability {capability:?}"
            ),
            Self::UnsupportedRequiredCapability(capability) => write!(
                formatter,
                "the host bridge cannot observe required capability {capability:?}"
            ),
            Self::PayloadDigestMismatch => {
                formatter.write_str("selected payload no longer matches its validated digest")
            }
            Self::UnsupportedHostAbi(error) => error.fmt(formatter),
            Self::LaunchContract(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArtifactBindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidArtifactTargetId { error, .. } => Some(error),
            Self::UnsupportedHostAbi(error) => Some(error),
            Self::LaunchContract(error) => Some(error),
            _ => None,
        }
    }
}

/// Failure while converting an artifact launch declaration into the stricter
/// host-side launch model.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArtifactLaunchContractError {
    FlatWorkgroupSizeOverflow,
    BlockExceedsObservedDevice { declared: u64, observed_max: u32 },
    StaticSharedMemoryExceedsObservedDevice { declared: u32, observed_max: u64 },
    Descriptor(DescriptorValidationError),
}

impl fmt::Display for ArtifactLaunchContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FlatWorkgroupSizeOverflow => {
                formatter.write_str("artifact block dimensions exceed the host contract range")
            }
            Self::BlockExceedsObservedDevice {
                declared,
                observed_max,
            } => write!(
                formatter,
                "artifact block permits {declared} threads, exceeding observed maximum {observed_max}"
            ),
            Self::StaticSharedMemoryExceedsObservedDevice {
                declared,
                observed_max,
            } => write!(
                formatter,
                "artifact static shared memory {declared} exceeds observed maximum {observed_max}"
            ),
            Self::Descriptor(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArtifactLaunchContractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Descriptor(error) => Some(error),
            _ => None,
        }
    }
}

/// Failure while revalidating a selected artifact and its context binding.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArtifactRevalidationError {
    Binding(ArtifactBindingError),
    WrongArtifactIdentity,
    WrongDevice,
    WrongContext,
    DeviceLimitsChanged,
    DeviceCapabilitiesChanged,
}

impl fmt::Display for ArtifactRevalidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(error) => error.fmt(formatter),
            Self::WrongArtifactIdentity => {
                formatter.write_str("selected artifact identity does not match the validated token")
            }
            Self::WrongDevice => formatter.write_str("observed device identity changed"),
            Self::WrongContext => formatter.write_str("observed context identity changed"),
            Self::DeviceLimitsChanged => {
                formatter.write_str("observed device launch limits changed")
            }
            Self::DeviceCapabilitiesChanged => {
                formatter.write_str("observed HIP device capability facts changed")
            }
        }
    }
}

impl std::error::Error for ArtifactRevalidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Binding(error) => Some(error),
            Self::WrongArtifactIdentity
            | Self::WrongDevice
            | Self::WrongContext
            | Self::DeviceLimitsChanged
            | Self::DeviceCapabilitiesChanged => None,
        }
    }
}

fn identity_from_selection(
    selected: SelectedNativeKernel<'_>,
    context: &ObservedContext,
) -> Result<ArtifactKernelIdentityV1, ArtifactBindingError> {
    validate_target(selected, context)?;
    validate_required_capabilities(selected.kernel().required_capabilities(), context)?;
    HostLaunchAbi::validate(selected.kernel().abi())
        .map_err(ArtifactBindingError::UnsupportedHostAbi)?;

    let effective_launch = effective_launch(selected.kernel().launch(), context)?;
    let payload_digest =
        PayloadDigest::new(selected.digest_algorithm(), selected.code_object().digest());
    payload_digest
        .verify(selected.payload())
        .map_err(|_| ArtifactBindingError::PayloadDigestMismatch)?;
    let manifest_digest = selected
        .digest_algorithm()
        .calculate(&selected.manifest().to_bytes());
    let kernel = selected.kernel();

    Ok(ArtifactKernelIdentityV1 {
        manifest_digest,
        kernel_id: KernelId::from_bytes(*kernel.kernel_id().as_bytes()),
        name: kernel.name().clone(),
        symbol: kernel.symbol().clone(),
        source_digest: kernel.source_digest(),
        executable_digest: kernel.executable_digest(),
        code_object: selected.code_object().clone(),
        payload_digest,
        target: selected.target().clone(),
        required_capabilities: kernel.required_capabilities().to_vec(),
        abi: kernel.abi().clone(),
        launch: kernel.launch().clone(),
        effective_launch,
    })
}

fn validate_target(
    selected: SelectedNativeKernel<'_>,
    context: &ObservedContext,
) -> Result<(), ArtifactBindingError> {
    let target = selected.target();
    if target.triple().as_str() != AMDGPU_TRIPLE {
        return Err(ArtifactBindingError::UnsupportedTargetTriple(
            target.triple().as_str().into(),
        ));
    }
    if target.pointer_width() != PointerWidth::Bits64 {
        return Err(ArtifactBindingError::UnsupportedPointerWidth(
            target.pointer_width(),
        ));
    }
    if target.endianness() != Endianness::Little {
        return Err(ArtifactBindingError::UnsupportedEndianness(
            target.endianness(),
        ));
    }
    let architecture = target.architecture().as_str();
    let artifact_target = AmdTargetId::parse(architecture).map_err(|error| {
        ArtifactBindingError::InvalidArtifactTargetId {
            target: architecture.into(),
            error,
        }
    })?;
    let observed_target = context.device().target_id();
    if !artifact_target.is_compatible_with_observed(&observed_target) {
        return Err(ArtifactBindingError::IncompatibleAmdTarget {
            artifact: artifact_target,
            observed: observed_target,
        });
    }
    Ok(())
}

fn validate_required_capabilities(
    capabilities: &[Capability],
    context: &ObservedContext,
) -> Result<(), ArtifactBindingError> {
    for &capability in capabilities {
        match capability {
            Capability::WorkgroupMemory if context.max_shared_memory_per_block() != 0 => {}
            Capability::WorkgroupMemory => {
                return Err(ArtifactBindingError::RequiredCapabilityUnavailable(
                    capability,
                ));
            }
            // The default warp size and HIP architecture bits are device-level
            // facts. The manifest does not record per-kernel wave size, ballot
            // width, shuffle semantics, or atomic width/scope/ordering/address
            // space, so these observations cannot satisfy these contracts.
            Capability::Subgroup
            | Capability::Ballot
            | Capability::Shuffle
            | Capability::Atomics
            | Capability::AmdWave => {
                return Err(ArtifactBindingError::InsufficientCapabilityObservation(
                    capability,
                ));
            }
            Capability::MatrixMultiply
            | Capability::AsyncCopy
            | Capability::AmdMfma
            | Capability::AmdWmma
            | Capability::AmdDsPermute => {
                return Err(ArtifactBindingError::UnsupportedRequiredCapability(
                    capability,
                ));
            }
        }
    }
    Ok(())
}

fn effective_launch(
    launch: &LaunchContract,
    context: &ObservedContext,
) -> Result<LaunchConstraintsV1, ArtifactBindingError> {
    let max_grid = descriptor_dimensions(launch.max_grid());
    let (block_size, max_flat_workgroup_size) = match launch.block_size() {
        BlockSize::Any => (BlockSizeV1::Any, context.max_threads_per_block()),
        BlockSize::Exact(dimensions) => {
            let flat = checked_flat_workgroup_size(dimensions, context)?;
            (BlockSizeV1::Exact(descriptor_dimensions(dimensions)), flat)
        }
        BlockSize::AtMost(dimensions) => {
            let flat = checked_flat_workgroup_size(dimensions, context)?;
            (BlockSizeV1::AtMost(descriptor_dimensions(dimensions)), flat)
        }
    };
    if u64::from(launch.static_shared_memory_bytes()) > context.max_shared_memory_per_block() {
        return Err(ArtifactBindingError::LaunchContract(
            ArtifactLaunchContractError::StaticSharedMemoryExceedsObservedDevice {
                declared: launch.static_shared_memory_bytes(),
                observed_max: context.max_shared_memory_per_block(),
            },
        ));
    }

    LaunchConstraintsV1::new(
        launch.rank(),
        block_size,
        max_grid,
        max_flat_workgroup_size,
        launch.static_shared_memory_bytes(),
        launch.max_dynamic_shared_memory_bytes(),
    )
    .map_err(|error| {
        ArtifactBindingError::LaunchContract(ArtifactLaunchContractError::Descriptor(error))
    })
}

fn checked_flat_workgroup_size(
    dimensions: fe2o3_artifacts::Dimensions,
    context: &ObservedContext,
) -> Result<u32, ArtifactBindingError> {
    let product = u64::from(dimensions.x())
        .checked_mul(u64::from(dimensions.y()))
        .and_then(|value| value.checked_mul(u64::from(dimensions.z())))
        .ok_or(ArtifactBindingError::LaunchContract(
            ArtifactLaunchContractError::FlatWorkgroupSizeOverflow,
        ))?;
    let flat = u32::try_from(product).map_err(|_| {
        ArtifactBindingError::LaunchContract(ArtifactLaunchContractError::FlatWorkgroupSizeOverflow)
    })?;
    if flat > context.max_threads_per_block() {
        return Err(ArtifactBindingError::LaunchContract(
            ArtifactLaunchContractError::BlockExceedsObservedDevice {
                declared: product,
                observed_max: context.max_threads_per_block(),
            },
        ));
    }
    Ok(flat)
}

fn descriptor_dimensions(dimensions: fe2o3_artifacts::Dimensions) -> DimensionsV1 {
    DimensionsV1::new(dimensions.x(), dimensions.y(), dimensions.z())
        .expect("validated artifact dimensions must satisfy descriptor dimensions")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::{
        AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership,
        ArtifactContainerV1, CodeObjectFormat, CodeObjectPayload, CompilerIdentity,
        DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestAlgorithm, Dimensions,
        IdentityText, KernelEntry, ManifestV1, Mutability, ScalarType, ToolIdentity, TypeIdentity,
    };

    struct ExpectationWithoutBackend;

    fn marker_function() {}

    unsafe impl KernelMarkerV1 for ExpectationWithoutBackend {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "general";
        const EXPORT_NAME: &'static str = "general";
        const FUNCTION: Self::Function = marker_function;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl CompilerGeneratedKernelExpectationV1 for ExpectationWithoutBackend {
        const PROFILE: CompilerGeneratedKernelProfileV1 =
            CompilerGeneratedKernelProfileV1::new([0x42; 32]);
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x41; 32];
    }

    struct SecondExpectation;

    fn second_marker_function() {}

    unsafe impl KernelMarkerV1 for SecondExpectation {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "second";
        const EXPORT_NAME: &'static str = "second_export";
        const FUNCTION: Self::Function = second_marker_function;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl CompilerGeneratedKernelExpectationV1 for SecondExpectation {
        const PROFILE: CompilerGeneratedKernelProfileV1 =
            CompilerGeneratedKernelProfileV1::new([0x52; 32]);
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x51; 32];
    }

    crate::compiler_generated_kernel_expectation_roster_v1! {
        struct OrderedTestRoster = [ExpectationWithoutBackend, SecondExpectation];
    }

    #[test]
    fn generated_expectation_roster_preserves_marker_order_and_identity() {
        let entries = OrderedTestRoster::ENTRIES;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].logical_name(), "general");
        assert_eq!(entries[0].export_name(), "general");
        assert_eq!(entries[0].kernel_binding_id(), [0x41; 32]);
        assert_eq!(entries[0].generated_host_contract_identity(), [0x42; 32]);
        assert_eq!(entries[1].logical_name(), "second");
        assert_eq!(entries[1].export_name(), "second_export");
        assert_eq!(entries[1].kernel_binding_id(), [0x51; 32]);
        assert_eq!(entries[1].generated_host_contract_identity(), [0x52; 32]);
    }

    fn general_v3_semantic_witness_bytes(
        kernel_binding: [u8; 32],
        generated_host_contract: [u8; 32],
    ) -> Vec<u8> {
        let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
        let length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(length)
                .expect("test witness length fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&kernel_binding);
        bytes.extend_from_slice(&generated_host_contract);
        bytes.extend_from_slice(
            &u16::try_from(profile.len())
                .expect("test profile length fits u16")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(profile);
        assert_eq!(bytes.len(), length);
        bytes
    }

    fn parse_test_semantic_witness(
        bytes: &[u8],
        kernel_binding: [u8; 32],
        generated_host_contract: [u8; 32],
    ) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        // SAFETY: `bytes` is one initialized immutable allocation that remains
        // live for the complete parser call.
        unsafe {
            semantic_witness_from_backend_v1(
                bytes.as_ptr(),
                bytes.len(),
                kernel_binding,
                generated_host_contract,
            )
        }
    }

    #[test]
    fn general_v3_semantic_witness_is_exact_and_identity_bound() {
        let binding = [0x51; 32];
        let contract = [0x52; 32];
        let bytes = general_v3_semantic_witness_bytes(binding, contract);
        let witness = parse_test_semantic_witness(&bytes, binding, contract).unwrap();

        assert_eq!(
            witness.profile,
            CompilerGeneratedKernelProfileV1::new(contract)
        );
        assert_eq!(witness.kernel_binding, binding);
        assert_eq!(witness.generated_host_contract, contract);
    }

    #[test]
    fn general_v3_semantic_witness_rejects_malformed_and_substituted_payloads() {
        let binding = [0x61; 32];
        let contract = [0x62; 32];
        let canonical = general_v3_semantic_witness_bytes(binding, contract);

        let mut changed = canonical.clone();
        changed[0] ^= 1;
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::MagicMismatch)
        );

        let mut changed = canonical.clone();
        changed[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::VersionMismatch)
        );

        let mut changed = canonical.clone();
        changed[10..12].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::DomainMismatch)
        );

        let mut changed = canonical.clone();
        let too_long = u32::try_from(changed.len() + 1).unwrap();
        changed[12..16].copy_from_slice(&too_long.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)
        );

        assert_eq!(
            parse_test_semantic_witness(&canonical, [0x63; 32], contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::KernelBindingMismatch)
        );
        assert_eq!(
            parse_test_semantic_witness(&canonical, binding, [0x64; 32]),
            Err(CompilerGeneratedSemanticWitnessErrorV1::GeneratedHostContractMismatch)
        );

        let mut changed = canonical.clone();
        *changed.last_mut().expect("profile tag is nonempty") ^= 1;
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::ProfileTagMismatch)
        );

        let mut changed = canonical.clone();
        changed.push(0);
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes)
        );

        let mut changed = canonical.clone();
        changed[80..82].copy_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes)
        );

        assert_eq!(
            parse_general_typed_v3_semantic_witness_v1(&[], binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)
        );
    }

    #[test]
    fn semantic_authority_requires_a_backend_witness() {
        assert_eq!(
            validate_compiler_generated_semantic_witness_v1::<ExpectationWithoutBackend>(),
            Err(CompilerGeneratedSemanticWitnessErrorV1::MissingBackendWitness)
        );
    }

    #[derive(Clone)]
    struct FixtureSpec {
        payload: Vec<u8>,
        kernel_id: u8,
        architecture: &'static str,
        triple: &'static str,
        pointer_width: PointerWidth,
        endianness: Endianness,
        compiler_version: &'static str,
        abi: AbiLayout,
        launch: LaunchContract,
        required_capabilities: Vec<Capability>,
    }

    impl Default for FixtureSpec {
        fn default() -> Self {
            Self {
                payload: b"native-gfx942-code-object".to_vec(),
                kernel_id: 0x11,
                architecture: "gfx942",
                triple: AMDGPU_TRIPLE,
                pointer_width: PointerWidth::Bits64,
                endianness: Endianness::Little,
                compiler_version: "1.94.0",
                abi: empty_abi(),
                launch: exact_launch(64, 32, 4_096),
                required_capabilities: vec![],
            }
        }
    }

    fn text(value: &str) -> IdentityText {
        IdentityText::new(value).unwrap()
    }

    fn name(value: &str) -> Name {
        Name::new(value).unwrap()
    }

    fn digest(byte: u8) -> DigestBytes {
        DigestBytes::from_bytes([byte; 32])
    }

    fn type_identity(byte: u8) -> TypeIdentity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(digest(byte)),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(byte.wrapping_add(1))),
        )
    }

    fn empty_abi() -> AbiLayout {
        empty_abi_with_width(PointerWidth::Bits64)
    }

    fn empty_abi_with_width(pointer_width: PointerWidth) -> AbiLayout {
        AbiLayout::new(0, 1, pointer_width, vec![]).unwrap()
    }

    fn scalar_abi() -> AbiLayout {
        AbiLayout::new(
            4,
            4,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    name("value"),
                    0,
                    4,
                    4,
                    AbiKind::Scalar(ScalarType::U32),
                    Mutability::Immutable,
                    Access::ByValue,
                    AddressSpace::Value,
                    type_identity(0x61),
                    ArgumentOwnership::ByValue,
                    AliasClass::Value,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn unsupported_constant_pointer_abi() -> AbiLayout {
        AbiLayout::new(
            8,
            8,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    name("input"),
                    0,
                    8,
                    8,
                    AbiKind::Pointer {
                        pointee_size: 4,
                        pointee_alignment: 4,
                    },
                    Mutability::Immutable,
                    Access::ReadOnly,
                    AddressSpace::Constant,
                    type_identity(0x71),
                    ArgumentOwnership::SharedBorrow,
                    AliasClass::SharedReadOnly,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn exact_launch(block_x: u32, static_bytes: u32, dynamic_bytes: u32) -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(block_x, 1, 1).unwrap()),
            Dimensions::new(65_535, 1, 1).unwrap(),
            static_bytes,
            dynamic_bytes,
        )
        .unwrap()
    }

    fn decoded_container(spec: FixtureSpec) -> ArtifactContainerV1 {
        let payload = CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, spec.payload).unwrap();
        let object_digest = payload.digest().bytes();
        let code_object = CodeObjectIdentity::new(
            object_digest,
            CodeObjectFormat::NativeExecutable,
            payload.bytes().len() as u64,
        )
        .unwrap();
        let target = TargetIdentity::new(
            text(spec.triple),
            text(spec.architecture),
            spec.pointer_width,
            spec.endianness,
            spec.required_capabilities.clone(),
        )
        .unwrap();
        let kernel = KernelEntry::new(
            digest(spec.kernel_id),
            name("vector_add"),
            name("vector_add.kd"),
            digest(0x22),
            digest(0x33),
            object_digest,
            spec.required_capabilities,
            spec.launch,
            spec.abi,
        )
        .unwrap();
        let manifest = ManifestV1::new(
            CompilerIdentity::new(text("rustc"), text(spec.compiler_version)),
            ToolIdentity::new(text("fe2o3"), text("0.1.0")),
            target,
            vec![code_object],
            vec![kernel],
        )
        .unwrap();
        let container =
            ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
        ArtifactContainerV1::from_bytes(&container.to_bytes()).unwrap()
    }

    fn context(identity: usize, ordinal: i32, target: &str) -> ObservedContext {
        ObservedContext::for_test(identity, ordinal, target, 1_024, 65_536)
    }

    fn validate_fixture(
        spec: FixtureSpec,
        observed: &ObservedContext,
    ) -> Result<ValidatedArtifactSelectionV1, ArtifactBindingError> {
        let container = decoded_container(spec);
        let selected = container.select_native_kernel(digest(0x11)).unwrap();
        ValidatedArtifactSelectionV1::validate(selected, observed)
    }

    fn kernel_id(byte: u8) -> KernelId {
        KernelId::from_bytes([byte; 32])
    }

    #[test]
    fn validates_exact_identity_and_retains_payload_without_marker_authority() {
        let observed = context(7, 3, "gfx942");
        let container = decoded_container(FixtureSpec::default());
        let selected = container.select_native_kernel(digest(0x11)).unwrap();
        let validated = ValidatedArtifactSelectionV1::validate(selected, &observed).unwrap();

        assert_eq!(validated.identity().version(), 1);
        assert_eq!(validated.identity().kernel_id(), kernel_id(0x11));
        assert_eq!(validated.identity().name().as_str(), "vector_add");
        assert_eq!(validated.identity().symbol().as_str(), "vector_add.kd");
        assert_eq!(validated.identity().source_digest(), digest(0x22));
        assert_eq!(validated.identity().executable_digest(), digest(0x33));
        assert_eq!(
            validated.identity().payload_digest().bytes(),
            selected.code_object().digest()
        );
        assert_eq!(validated.identity().target(), selected.target());
        assert_eq!(validated.identity().abi(), selected.kernel().abi());
        assert_eq!(validated.identity().launch(), selected.kernel().launch());
        assert_eq!(
            validated
                .identity()
                .effective_launch()
                .max_flat_workgroup_size(),
            64
        );
        assert_eq!(validated.payload(), selected.payload());
        assert_eq!(validated.device(), observed.device());
        validated.revalidate(selected, &observed).unwrap();
    }

    #[test]
    fn same_names_and_kernel_id_cannot_confuse_different_payloads() {
        let observed = context(1, 0, "gfx942");
        let first = decoded_container(FixtureSpec::default());
        let second_spec = FixtureSpec {
            payload: b"different-native-code-object".to_vec(),
            ..FixtureSpec::default()
        };
        let second = decoded_container(second_spec);
        let first_selected = first.select_native_kernel(digest(0x11)).unwrap();
        let second_selected = second.select_native_kernel(digest(0x11)).unwrap();
        assert_eq!(
            first_selected.kernel().name(),
            second_selected.kernel().name()
        );
        assert_eq!(
            first_selected.kernel().kernel_id(),
            second_selected.kernel().kernel_id()
        );

        let validated = ValidatedArtifactSelectionV1::validate(first_selected, &observed).unwrap();
        assert_eq!(
            validated.revalidate(second_selected, &observed),
            Err(ArtifactRevalidationError::WrongArtifactIdentity)
        );
        let second_validated =
            ValidatedArtifactSelectionV1::validate(second_selected, &observed).unwrap();
        assert_ne!(validated.identity(), second_validated.identity());
    }

    #[test]
    fn whole_manifest_identity_prevents_cross_artifact_confusion() {
        let observed = context(1, 0, "gfx942");
        let first = decoded_container(FixtureSpec::default());
        let second_spec = FixtureSpec {
            compiler_version: "1.94.1",
            ..FixtureSpec::default()
        };
        let second = decoded_container(second_spec);
        let first_selected = first.select_native_kernel(digest(0x11)).unwrap();
        let second_selected = second.select_native_kernel(digest(0x11)).unwrap();
        assert_eq!(first_selected.payload(), second_selected.payload());
        assert_eq!(first_selected.kernel(), second_selected.kernel());

        let validated = ValidatedArtifactSelectionV1::validate(first_selected, &observed).unwrap();
        assert_eq!(
            validated.revalidate(second_selected, &observed),
            Err(ArtifactRevalidationError::WrongArtifactIdentity)
        );
    }

    #[test]
    fn valid_kernel_abi_and_launch_mutations_fail_exact_revalidation() {
        let observed = context(1, 0, "gfx942");
        let original = decoded_container(FixtureSpec::default());
        let original_selected = original.select_native_kernel(digest(0x11)).unwrap();
        let validated =
            ValidatedArtifactSelectionV1::validate(original_selected, &observed).unwrap();

        let abi_spec = FixtureSpec {
            abi: scalar_abi(),
            ..FixtureSpec::default()
        };
        let changed_abi = decoded_container(abi_spec);
        assert_eq!(
            validated.revalidate(
                changed_abi.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactRevalidationError::WrongArtifactIdentity)
        );

        let launch_spec = FixtureSpec {
            launch: exact_launch(32, 32, 4_096),
            ..FixtureSpec::default()
        };
        let changed_launch = decoded_container(launch_spec);
        assert_eq!(
            validated.revalidate(
                changed_launch.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactRevalidationError::WrongArtifactIdentity)
        );

        let kernel_spec = FixtureSpec {
            kernel_id: 0x12,
            ..FixtureSpec::default()
        };
        let changed_kernel = decoded_container(kernel_spec);
        assert_eq!(
            validated.revalidate(
                changed_kernel.select_native_kernel(digest(0x12)).unwrap(),
                &observed
            ),
            Err(ArtifactRevalidationError::WrongArtifactIdentity)
        );
    }

    #[test]
    fn target_feature_compatibility_is_asymmetric_and_processor_exact() {
        let explicit = context(1, 0, "gfx942:sramecc+:xnack-");

        for architecture in ["gfx942", "gfx942:sramecc+:xnack-"] {
            validate_fixture(
                FixtureSpec {
                    architecture,
                    ..FixtureSpec::default()
                },
                &explicit,
            )
            .unwrap();
        }

        for architecture in [
            "gfx942:sramecc-:xnack-",
            "gfx942:sramecc+:xnack+",
            "gfx950:sramecc+:xnack-",
        ] {
            assert!(matches!(
                validate_fixture(
                    FixtureSpec {
                        architecture,
                        ..FixtureSpec::default()
                    },
                    &explicit,
                ),
                Err(ArtifactBindingError::IncompatibleAmdTarget { .. })
            ));
        }

        let omitted_observation = context(1, 0, "gfx942");
        validate_fixture(FixtureSpec::default(), &omitted_observation).unwrap();
        assert!(matches!(
            validate_fixture(
                FixtureSpec {
                    architecture: "gfx942:xnack-",
                    ..FixtureSpec::default()
                },
                &omitted_observation,
            ),
            Err(ArtifactBindingError::IncompatibleAmdTarget { .. })
        ));
    }

    #[test]
    fn malformed_generic_and_unknown_artifact_target_ids_fail_closed() {
        use fe2o3_amd_target::{AmdTargetFeature, ParseAmdTargetIdError};

        let observed = context(1, 0, "gfx942:sramecc+:xnack-");
        for (target, expected) in [
            ("gfx9-generic", ParseAmdTargetIdError::GenericProcessor),
            ("gfx9999", ParseAmdTargetIdError::UnknownProcessor),
            ("gfx942:future+", ParseAmdTargetIdError::UnknownFeature),
            (
                "gfx942:xnack",
                ParseAmdTargetIdError::MissingFeatureState(AmdTargetFeature::Xnack),
            ),
            (
                "gfx942:xnack=on",
                ParseAmdTargetIdError::InvalidFeature(AmdTargetFeature::Xnack),
            ),
        ] {
            assert_eq!(
                validate_fixture(
                    FixtureSpec {
                        architecture: target,
                        ..FixtureSpec::default()
                    },
                    &observed,
                )
                .unwrap_err(),
                ArtifactBindingError::InvalidArtifactTargetId {
                    target: target.into(),
                    error: expected,
                }
            );
        }
    }

    #[test]
    fn workgroup_memory_is_the_only_currently_established_artifact_capability() {
        let observed = context(1, 0, "gfx942");
        let validated = validate_fixture(
            FixtureSpec {
                required_capabilities: vec![Capability::WorkgroupMemory],
                ..FixtureSpec::default()
            },
            &observed,
        )
        .unwrap();
        assert_eq!(
            validated.identity().required_capabilities(),
            &[Capability::WorkgroupMemory]
        );

        let without_workgroup_memory = ObservedContext::for_test(1, 0, "gfx942", 1_024, 0);
        assert_eq!(
            validate_fixture(
                FixtureSpec {
                    required_capabilities: vec![Capability::WorkgroupMemory],
                    ..FixtureSpec::default()
                },
                &without_workgroup_memory,
            )
            .unwrap_err(),
            ArtifactBindingError::RequiredCapabilityUnavailable(Capability::WorkgroupMemory)
        );
    }

    #[test]
    fn coarse_hip_facts_do_not_overclaim_artifact_capabilities() {
        let observed = context(1, 0, "gfx942");
        assert!(observed.has_global_int32_atomics());
        assert!(observed.has_shared_int32_atomics());
        assert!(observed.has_global_int64_atomics());
        assert!(observed.has_shared_int64_atomics());
        assert!(observed.has_warp_ballot());
        assert!(observed.has_warp_shuffle());

        for capability in [
            Capability::Subgroup,
            Capability::Ballot,
            Capability::Shuffle,
            Capability::Atomics,
            Capability::AmdWave,
        ] {
            assert_eq!(
                validate_fixture(
                    FixtureSpec {
                        required_capabilities: vec![capability],
                        ..FixtureSpec::default()
                    },
                    &observed,
                )
                .unwrap_err(),
                ArtifactBindingError::InsufficientCapabilityObservation(capability)
            );
        }
    }

    #[test]
    fn amd_wave_requires_finer_capability_observation() {
        let error = validate_fixture(
            FixtureSpec {
                required_capabilities: vec![Capability::AmdWave],
                ..FixtureSpec::default()
            },
            &context(1, 0, "gfx942"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ArtifactBindingError::InsufficientCapabilityObservation(Capability::AmdWave)
        );
        assert_eq!(
            error.to_string(),
            "HIP observations are too coarse to establish required capability AmdWave"
        );
    }

    #[test]
    fn unobserved_specialized_capabilities_remain_unsupported() {
        let observed = context(1, 0, "gfx942");
        for capability in [
            Capability::MatrixMultiply,
            Capability::AsyncCopy,
            Capability::AmdMfma,
            Capability::AmdWmma,
            Capability::AmdDsPermute,
        ] {
            assert_eq!(
                validate_fixture(
                    FixtureSpec {
                        required_capabilities: vec![capability],
                        ..FixtureSpec::default()
                    },
                    &observed,
                )
                .unwrap_err(),
                ArtifactBindingError::UnsupportedRequiredCapability(capability)
            );
        }
    }

    #[test]
    fn every_unsupported_target_component_fails_binding() {
        let observed = context(1, 0, "gfx942");

        let triple = FixtureSpec {
            triple: "spirv64-unknown-unknown",
            ..FixtureSpec::default()
        };
        let container = decoded_container(triple);
        assert!(matches!(
            ValidatedArtifactSelectionV1::validate(
                container.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactBindingError::UnsupportedTargetTriple(_))
        ));

        let width = FixtureSpec {
            pointer_width: PointerWidth::Bits32,
            abi: empty_abi_with_width(PointerWidth::Bits32),
            ..FixtureSpec::default()
        };
        let container = decoded_container(width);
        assert!(matches!(
            ValidatedArtifactSelectionV1::validate(
                container.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactBindingError::UnsupportedPointerWidth(
                PointerWidth::Bits32
            ))
        ));

        let endianness = FixtureSpec {
            endianness: Endianness::Big,
            ..FixtureSpec::default()
        };
        let container = decoded_container(endianness);
        assert!(matches!(
            ValidatedArtifactSelectionV1::validate(
                container.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactBindingError::UnsupportedEndianness(Endianness::Big))
        ));
    }

    #[test]
    fn unsupported_structural_abi_fails_without_claiming_host_type_matching() {
        let observed = context(1, 0, "gfx942");
        let spec = FixtureSpec {
            abi: unsupported_constant_pointer_abi(),
            ..FixtureSpec::default()
        };
        let container = decoded_container(spec);
        assert!(matches!(
            ValidatedArtifactSelectionV1::validate(
                container.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactBindingError::UnsupportedHostAbi(
                HostLaunchAbiError::UnsupportedAddressSpace {
                    address_space: AddressSpace::Constant,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn impossible_launch_contracts_fail_before_validation_token_creation() {
        let observed = context(1, 0, "gfx942");
        let block = FixtureSpec {
            launch: exact_launch(2_048, 0, 0),
            ..FixtureSpec::default()
        };
        let container = decoded_container(block);
        assert!(matches!(
            ValidatedArtifactSelectionV1::validate(
                container.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactBindingError::LaunchContract(
                ArtifactLaunchContractError::BlockExceedsObservedDevice { .. }
            ))
        ));

        let shared = FixtureSpec {
            launch: exact_launch(64, 65_537, 0),
            ..FixtureSpec::default()
        };
        let container = decoded_container(shared);
        assert!(matches!(
            ValidatedArtifactSelectionV1::validate(
                container.select_native_kernel(digest(0x11)).unwrap(),
                &observed
            ),
            Err(ArtifactBindingError::LaunchContract(
                ArtifactLaunchContractError::StaticSharedMemoryExceedsObservedDevice { .. }
            ))
        ));
    }

    #[test]
    fn exact_context_device_and_limit_changes_fail_revalidation() {
        let observed = context(1, 0, "gfx942");
        let container = decoded_container(FixtureSpec::default());
        let selected = container.select_native_kernel(digest(0x11)).unwrap();
        let validated = ValidatedArtifactSelectionV1::validate(selected, &observed).unwrap();

        assert_eq!(
            validated.revalidate(selected, &context(1, 1, "gfx942")),
            Err(ArtifactRevalidationError::WrongDevice)
        );
        assert_eq!(
            validated.revalidate(selected, &context(2, 0, "gfx942")),
            Err(ArtifactRevalidationError::WrongContext)
        );
        let changed_limits = ObservedContext::for_test(1, 0, "gfx942", 512, 65_536);
        assert_eq!(
            validated.revalidate(selected, &changed_limits),
            Err(ArtifactRevalidationError::DeviceLimitsChanged)
        );
        let changed_capabilities = observed.clone().with_changed_test_hip_capabilities();
        assert_eq!(
            validated.revalidate(selected, &changed_capabilities),
            Err(ArtifactRevalidationError::DeviceCapabilitiesChanged)
        );
    }

    #[test]
    fn payload_bytes_survive_container_lifetime() {
        let observed = context(1, 0, "gfx942");
        let validated = {
            let container = decoded_container(FixtureSpec::default());
            let selected = container.select_native_kernel(digest(0x11)).unwrap();
            ValidatedArtifactSelectionV1::validate(selected, &observed).unwrap()
        };
        assert_eq!(validated.payload(), b"native-gfx942-code-object");
    }

    #[test]
    fn adversarial_wire_mutations_never_reach_selection_validation() {
        let container = decoded_container(FixtureSpec::default());
        let encoded = container.to_bytes();

        for length in [0, 1, 8, encoded.len() - 1] {
            assert!(ArtifactContainerV1::from_bytes(&encoded[..length]).is_err());
        }
        for payload_offset in encoded.len() - container.payloads()[0].bytes().len()..encoded.len() {
            let mut mutated = encoded.clone();
            mutated[payload_offset] ^= 0x80;
            assert!(ArtifactContainerV1::from_bytes(&mutated).is_err());
        }
        let mut unknown_version = encoded;
        unknown_version[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(ArtifactContainerV1::from_bytes(&unknown_version).is_err());
    }
}
