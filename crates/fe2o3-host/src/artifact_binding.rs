use crate::{
    BlockSizeV1, DeviceIdentity, DimensionsV1, KernelBrand, KernelId, LaunchConstraintsV1,
    LoadedKernel, ObservedContext, PrepareLaunchError, PreparedLaunch, UntrustedLaunchRequest,
};
use fe2o3_amd_target::{AmdTargetId, ParseAmdTargetIdError};
use fe2o3_artifacts::{
    AbiLayout, BlockSize, Capability, CodeObjectIdentity, DigestBytes, Endianness, HostLaunchAbi,
    HostLaunchAbiError, LaunchContract, Name, PayloadDigest, PointerWidth, SelectedNativeKernel,
    TargetIdentity,
};
use fe2o3_kernel_descriptor::ValidationError as DescriptorValidationError;
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
/// type. It retains exact identity and payload bytes but contains no HIP handle,
/// cannot construct [`KernelBrand`], and exposes no launch operation.
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

    /// Deliberately crate-private until generated code can provide unforgeable
    /// evidence that `K` denotes `self.identity().kernel_id()` and ABI.
    #[allow(dead_code)]
    pub(crate) fn bind_marker<K>(&self) -> ArtifactKernelBrandV1<K> {
        let brand = KernelBrand::from_internal_binding(
            self.identity.kernel_id,
            self.identity.effective_launch.clone(),
            self.context.clone(),
        );
        ArtifactKernelBrandV1 {
            identity: self.identity.clone(),
            payload: self.payload.clone(),
            context: self.context.clone(),
            brand,
        }
    }
}

/// Internal typed bridge. It is not exported because artifact structure does
/// not validate the marker association.
#[allow(dead_code)]
pub(crate) struct ArtifactKernelBrandV1<K> {
    pub(crate) identity: Arc<ArtifactKernelIdentityV1>,
    pub(crate) payload: Arc<[u8]>,
    pub(crate) context: ObservedContext,
    pub(crate) brand: KernelBrand<K>,
}

#[allow(dead_code)]
impl<K> ArtifactKernelBrandV1<K> {
    pub(crate) fn identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.identity
    }

    pub(crate) fn prepare(
        &self,
        validated: &ValidatedArtifactSelectionV1,
        context: &ObservedContext,
        request: UntrustedLaunchRequest<K>,
    ) -> Result<ArtifactPreparedLaunchV1<K>, ArtifactMarkerPrepareError> {
        if !Arc::ptr_eq(&self.identity, &validated.identity)
            || !Arc::ptr_eq(&self.payload, &validated.payload)
            || !context.same_context(&self.context)
        {
            return Err(ArtifactMarkerPrepareError::WrongValidatedSelection);
        }
        let prepared = self
            .brand
            .prepare(context, request)
            .map_err(ArtifactMarkerPrepareError::Launch)?;
        Ok(ArtifactPreparedLaunchV1 {
            identity: self.identity.clone(),
            payload: self.payload.clone(),
            prepared,
        })
    }

    /// Loads the exact validated payload and symbol represented by this marker
    /// binding.
    ///
    /// # Safety
    ///
    /// The issuer must independently establish that the executable payload is
    /// trusted and that `K` denotes this exact kernel identity and complete
    /// host ABI. Structural artifact validation establishes neither fact.
    pub(crate) unsafe fn load(
        self,
        validated: &ValidatedArtifactSelectionV1,
        observed: &ObservedContext,
        context: &Arc<fe2o3_core::GpuContext>,
    ) -> Result<LoadedKernel<K>, crate::loaded_kernel::LoadedKernelLoadError> {
        // SAFETY: The caller owns the executable-trust and marker/ABI proof
        // obligations documented above. The callee rechecks every structural
        // identity and context relationship before invoking HIP.
        unsafe { LoadedKernel::load(self, validated, observed, context) }
    }
}

/// Internal data-only prepared bridge; no module handle or launch method.
#[allow(dead_code)]
pub(crate) struct ArtifactPreparedLaunchV1<K> {
    identity: Arc<ArtifactKernelIdentityV1>,
    payload: Arc<[u8]>,
    prepared: PreparedLaunch<K>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ArtifactMarkerPrepareError {
    WrongValidatedSelection,
    Launch(PrepareLaunchError),
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

    struct VecAdd;

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

    fn request(kernel: KernelId) -> UntrustedLaunchRequest<VecAdd> {
        UntrustedLaunchRequest::new(kernel, 1, [17, 1, 1], [64, 1, 1], 128)
    }

    fn test_loaded(validated: &ValidatedArtifactSelectionV1) -> crate::LoadedKernel<VecAdd> {
        crate::LoadedKernel::from_test_binding(validated.bind_marker())
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
    fn internal_typed_bridge_preserves_brand_and_prepared_identity() {
        let observed = context(7, 0, "gfx942");
        let container = decoded_container(FixtureSpec::default());
        let selected = container.select_native_kernel(digest(0x11)).unwrap();
        let validated = ValidatedArtifactSelectionV1::validate(selected, &observed).unwrap();
        let brand = validated.bind_marker::<VecAdd>();
        let prepared = brand
            .prepare(&validated, &observed, request(kernel_id(0x11)))
            .unwrap();

        assert_eq!(brand.identity(), validated.identity());
        assert!(Arc::ptr_eq(&prepared.identity, &validated.identity));
        assert!(Arc::ptr_eq(&prepared.payload, &validated.payload));
        assert_eq!(prepared.payload.as_ref(), selected.payload());
        assert!(prepared.prepared.belongs_to(&brand.brand));
        assert_eq!(prepared.prepared.device(), observed.device());
    }

    #[test]
    fn loaded_authority_consumes_only_its_own_prepared_launch() {
        let observed = context(7, 0, "gfx942");
        let validated = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let loaded = test_loaded(&validated);
        let prepared = loaded.prepare(&observed, request(kernel_id(0x11))).unwrap();
        let bound = loaded.bind(prepared).unwrap();

        assert_eq!(loaded.identity(), validated.identity());
        assert_eq!(loaded.device(), observed.device());
        assert_eq!(bound.identity(), validated.identity());
        assert_eq!(bound.geometry().grid().dimensions(), [17, 1, 1]);
        assert_eq!(bound.geometry().block().dimensions(), [64, 1, 1]);
        assert_eq!(bound.resources().dynamic_shared_memory_bytes(), 128);
        let config = bound.launch_config();
        assert_eq!(config.grid_dim, (17, 1, 1));
        assert_eq!(config.block_dim, (64, 1, 1));
        assert_eq!(config.shared_mem_bytes, 128);
    }

    #[test]
    fn separate_marker_issuance_cannot_reuse_a_prepared_launch() {
        let observed = context(7, 0, "gfx942");
        let validated = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let first = test_loaded(&validated);
        let second = test_loaded(&validated);
        let prepared = first.prepare(&observed, request(kernel_id(0x11))).unwrap();

        assert_eq!(
            second.bind(prepared).unwrap_err(),
            crate::LoadedKernelMatchError::WrongArtifactAuthority
        );
    }

    #[test]
    fn payload_manifest_and_abi_identity_changes_cannot_cross_authorities() {
        let observed = context(7, 0, "gfx942");
        let original = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let original_loaded = test_loaded(&original);

        for changed in [
            FixtureSpec {
                payload: b"different-native-code-object".to_vec(),
                ..FixtureSpec::default()
            },
            FixtureSpec {
                compiler_version: "1.94.1",
                ..FixtureSpec::default()
            },
            FixtureSpec {
                abi: scalar_abi(),
                ..FixtureSpec::default()
            },
        ] {
            let changed = validate_fixture(changed, &observed).unwrap();
            assert_ne!(original.identity(), changed.identity());
            let changed_loaded = test_loaded(&changed);
            let prepared = changed_loaded
                .prepare(&observed, request(kernel_id(0x11)))
                .unwrap();
            assert_eq!(
                original_loaded.bind(prepared).unwrap_err(),
                crate::LoadedKernelMatchError::WrongArtifactAuthority
            );
        }
    }

    #[test]
    fn kernel_identity_change_cannot_cross_loaded_authorities() {
        let observed = context(7, 0, "gfx942");
        let original = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let changed_container = decoded_container(FixtureSpec {
            kernel_id: 0x12,
            ..FixtureSpec::default()
        });
        let changed = ValidatedArtifactSelectionV1::validate(
            changed_container
                .select_native_kernel(digest(0x12))
                .unwrap(),
            &observed,
        )
        .unwrap();
        let original_loaded = test_loaded(&original);
        let changed_loaded = test_loaded(&changed);
        let prepared = changed_loaded
            .prepare(&observed, request(kernel_id(0x12)))
            .unwrap();

        assert_eq!(
            original_loaded.bind(prepared).unwrap_err(),
            crate::LoadedKernelMatchError::WrongKernel
        );
    }

    #[test]
    fn context_device_limits_and_capabilities_cannot_cross_loaded_authorities() {
        let original_observed = context(7, 0, "gfx942");
        let original = validate_fixture(FixtureSpec::default(), &original_observed).unwrap();
        let original_loaded = test_loaded(&original);

        let cases = [
            (
                context(7, 1, "gfx942"),
                crate::LoadedKernelMatchError::WrongDevice,
            ),
            (
                context(8, 0, "gfx942"),
                crate::LoadedKernelMatchError::WrongContext,
            ),
            (
                ObservedContext::for_test(7, 0, "gfx942", 512, 65_536),
                crate::LoadedKernelMatchError::DeviceLimitsChanged,
            ),
            (
                original_observed
                    .clone()
                    .with_changed_test_hip_capabilities(),
                crate::LoadedKernelMatchError::DeviceCapabilitiesChanged,
            ),
        ];

        for (changed_observed, expected) in cases {
            let changed = validate_fixture(FixtureSpec::default(), &changed_observed).unwrap();
            let changed_loaded = test_loaded(&changed);
            let prepared = changed_loaded
                .prepare(&changed_observed, request(kernel_id(0x11)))
                .unwrap();
            assert_eq!(original_loaded.bind(prepared).unwrap_err(), expected);
        }
    }

    #[test]
    fn loaded_issuance_rechecks_selection_context_device_limits_and_capabilities() {
        use crate::loaded_kernel::{LoadedKernelLoadError, validate_issuance};

        let observed = context(7, 0, "gfx942");
        let first = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let second = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let binding = first.bind_marker::<VecAdd>();

        assert!(validate_issuance(&binding, &first, &observed).is_ok());
        assert!(matches!(
            validate_issuance(&binding, &second, &observed),
            Err(LoadedKernelLoadError::WrongValidatedSelection)
        ));
        assert!(matches!(
            validate_issuance(&binding, &first, &context(7, 1, "gfx942")),
            Err(LoadedKernelLoadError::WrongDevice)
        ));
        assert!(matches!(
            validate_issuance(&binding, &first, &context(8, 0, "gfx942")),
            Err(LoadedKernelLoadError::WrongContext)
        ));
        assert!(matches!(
            validate_issuance(
                &binding,
                &first,
                &ObservedContext::for_test(7, 0, "gfx942", 512, 65_536)
            ),
            Err(LoadedKernelLoadError::DeviceLimitsChanged)
        ));
        assert!(matches!(
            validate_issuance(
                &binding,
                &first,
                &observed.clone().with_changed_test_hip_capabilities()
            ),
            Err(LoadedKernelLoadError::DeviceCapabilitiesChanged)
        ));
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn loaded_issuance_rejects_another_wrapper_for_the_same_hip_device() {
        use crate::loaded_kernel::LoadedKernelLoadError;

        let context = fe2o3_core::GpuContext::new(0).unwrap();
        let another_context = fe2o3_core::GpuContext::new(0).unwrap();
        let observed = ObservedContext::observe(&context).unwrap();
        let architecture: &'static str =
            Box::leak(observed.device().target().to_owned().into_boxed_str());
        let validated = validate_fixture(
            FixtureSpec {
                architecture,
                ..FixtureSpec::default()
            },
            &observed,
        )
        .unwrap();
        let binding = validated.bind_marker::<VecAdd>();

        // SAFETY: This test expects rejection before HIP sees the deliberately
        // fake payload, so none of the unsafe loading obligations are relied on.
        let error = unsafe { binding.load(&validated, &observed, &another_context) }.unwrap_err();
        assert!(matches!(error, LoadedKernelLoadError::WrongContextWrapper));
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
        let brand = validated.bind_marker::<VecAdd>();
        assert!(matches!(
            brand.prepare(&second_validated, &observed, request(kernel_id(0x11))),
            Err(ArtifactMarkerPrepareError::WrongValidatedSelection)
        ));
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
    fn internal_typed_preparation_rejects_wrong_kernel_and_cross_issuance() {
        let observed = context(1, 0, "gfx942");
        let container = decoded_container(FixtureSpec::default());
        let selected = container.select_native_kernel(digest(0x11)).unwrap();
        let validated = ValidatedArtifactSelectionV1::validate(selected, &observed).unwrap();
        let first = validated.bind_marker::<VecAdd>();
        let second = validated.bind_marker::<VecAdd>();

        assert!(matches!(
            first.prepare(&validated, &observed, request(kernel_id(0x12))),
            Err(ArtifactMarkerPrepareError::Launch(
                PrepareLaunchError::WrongKernel { .. }
            ))
        ));
        let prepared = first
            .prepare(&validated, &observed, request(kernel_id(0x11)))
            .unwrap();
        assert!(prepared.prepared.belongs_to(&first.brand));
        assert!(!prepared.prepared.belongs_to(&second.brand));
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
