use crate::{
    BlockSizeV1, DeviceIdentity, DimensionsV1, KernelBrand, KernelId, LaunchConstraintsV1,
    LoadedKernel, ObservedContext, PrepareLaunchError, PreparedLaunch, UntrustedLaunchRequest,
};
use fe2o3_amd_target::{AmdTargetId, ParseAmdTargetIdError};
use fe2o3_artifacts::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, ArtifactContainerV1,
    BlockSize, Capability, CodeObjectIdentity, ContainerDecodeError, DeclaredRustLayoutIdentity,
    DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Endianness, HostLaunchAbi,
    HostLaunchAbiError, KernelSelectionError, LaunchContract, Mutability, Name, PayloadDigest,
    PointerWidth, RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
    RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
    RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1, ScalarType, SelectedNativeKernel,
    TargetIdentity, TypeIdentity, derive_generated_host_contract_identity_v1,
    derive_generated_kernel_identity_v2,
};
use fe2o3_device::{DisjointSlice, Index1D, KernelMarkerV1};
use fe2o3_kernel_descriptor::ValidationError as DescriptorValidationError;
use reserved_fe2o3_symbols::TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2;
use std::fmt;
use std::sync::Arc;

const AMDGPU_TRIPLE: &str = "amdgcn-amd-amdhsa";

/// Version of the exact artifact identity carried by the G3 host bridge.
pub const ARTIFACT_KERNEL_IDENTITY_VERSION: u16 = 1;

const TYPE_ID_DOMAIN: &[u8] = b"fe2o3.rust-type.v1\0";
const LAYOUT_ID_DOMAIN: &[u8] = b"fe2o3.rust-layout.v1\0";
const MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1: &str = "fe2o3.manifest-derived-scalar-slice.v1";

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

    /// Binds a compiler-generated marker to this validated selection.
    ///
    /// This is an explicit unsafe SPI for compiler-generated adapters. It
    /// checks only that the marker's logical and exported names exactly match
    /// the validated artifact identity. It does not inspect
    /// [`KernelMarkerV1::FUNCTION`] and does not infer a packed ABI from the
    /// function-pointer type.
    ///
    /// # Safety
    ///
    /// The caller must independently authenticate the executable payload and
    /// establish that `K` denotes this exact kernel. The caller must also
    /// establish the complete host ABI association, including every argument's
    /// Rust type, size, alignment, field order, mutability, address space,
    /// ownership, aliasing, and packed layout identity. Structural artifact
    /// validation and the name checks performed here discharge none of those
    /// obligations.
    #[doc(hidden)]
    pub unsafe fn bind_generated_marker<K: KernelMarkerV1>(
        &self,
    ) -> Result<GeneratedKernelBindingV1<K>, GeneratedMarkerBindingError> {
        let artifact_logical = self.identity.name().as_str();
        if K::LOGICAL_NAME != artifact_logical {
            return Err(GeneratedMarkerBindingError::LogicalNameMismatch {
                marker: K::LOGICAL_NAME,
                artifact: artifact_logical.to_owned(),
            });
        }

        let artifact_export = self.identity.symbol().as_str();
        if K::EXPORT_NAME != artifact_export {
            return Err(GeneratedMarkerBindingError::ExportNameMismatch {
                marker: K::EXPORT_NAME,
                artifact: artifact_export.to_owned(),
            });
        }

        Ok(GeneratedKernelBindingV1 {
            inner: self.bind_marker(),
        })
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

/// Trusted backend contract for one compiler-generated kernel artifact.
///
/// This trait is an implementation boundary for generated code, not an
/// application extension point. Authentication always decodes the bytes
/// returned by [`CompilerGeneratedKernelContractV1::artifact_container_bytes`]
/// and never accepts a caller-selected container.
///
/// # Safety
///
/// The implementation must be emitted by the trusted compiler backend and
/// return the exact, immutable canonical [`ArtifactContainerV1`] bytes that the
/// backend produced and embedded for `Self`. Those bytes must contain exactly
/// one entry denoted by the marker's logical and export names, and that entry's
/// identity, complete physical host ABI, declared effects, launch contract,
/// and executable behavior must all describe `Self::FUNCTION` exactly.
///
/// Every executable memory effect, including effects selected or sized by
/// scalar arguments, must be represented by the generated adapter's admission
/// contract. Loading, unloading, and all module initialization or finalization
/// behavior must also be safe under the generated contract. This host layer
/// does not inspect init/fini entries; the implementation must establish that
/// they are absent or that their complete semantics satisfy these obligations.
/// A false implementation can make safe code load arbitrary native code.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedKernelContractV1: KernelMarkerV1 {
    /// Versioned host ABI and memory-effect profile expected by generated code.
    const PROFILE: CompilerGeneratedKernelProfileV1;

    /// Full backend-validated identity used by private host linker symbols.
    const KERNEL_BINDING_ID_V1: [u8; 32];

    /// Returns the exact canonical artifact container embedded by the backend.
    fn artifact_container_bytes() -> &'static [u8];
}

/// Exact generated host contract understood by this runtime version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum CompilerGeneratedKernelProfileV1 {
    /// `(&[f32], &[f32], DisjointSlice<f32>)` with read/read/write effects.
    TypedVecAddF32V1,
    /// The same fixed signature with canonical rustc-derived type/layout
    /// identities independently reconstructed by the host.
    TypedVecAddF32RustcLayoutV2,
    /// A bounded scalar/slice signature committed to independently by the
    /// generated adapter.
    ///
    /// The identity is emitted by the compiler from its canonical Rust ABI,
    /// layout, effects, launch, and binding expectation. Authentication
    /// derives the same pre-executable identity from the selected artifact and
    /// rejects a mismatch; the artifact therefore cannot serve as its own
    /// expectation. Final kernel identity is checked separately after source
    /// and executable digests exist.
    ManifestDerivedScalarSliceV1 {
        generated_host_contract_identity: [u8; 32],
    },
}

/// Authenticated compiler-generated artifact for exactly one kernel marker.
///
/// Fields are private so callers cannot replace the validated selection or its
/// marker binding. Construct this token with [`Self::authenticate`].
#[doc(hidden)]
pub struct AuthenticatedKernelArtifactV1<K: CompilerGeneratedKernelContractV1> {
    validated: ValidatedArtifactSelectionV1,
    binding: GeneratedKernelBindingV1<K>,
}

impl<K: CompilerGeneratedKernelContractV1> fmt::Debug for AuthenticatedKernelArtifactV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedKernelArtifactV1")
            .field("identity", self.validated.identity())
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelContractV1> AuthenticatedKernelArtifactV1<K> {
    /// Authenticates the exact artifact bytes embedded for `K` against one
    /// observed context.
    ///
    /// The byte source is fixed by `K`; this API intentionally has no
    /// caller-supplied bytes parameter. It decodes the complete container,
    /// requires exactly one kernel with both marker names, selects that entry,
    /// and applies the existing target, host-ABI, launch, and payload checks.
    pub fn authenticate(
        observed: &ObservedContext,
    ) -> Result<Self, GeneratedArtifactAuthenticationError> {
        let container = ArtifactContainerV1::from_bytes(K::artifact_container_bytes())
            .map_err(GeneratedArtifactAuthenticationError::Decode)?;

        let mut matching = container.manifest().kernels().iter().filter(|kernel| {
            kernel.name().as_str() == K::LOGICAL_NAME && kernel.symbol().as_str() == K::EXPORT_NAME
        });
        let kernel = matching
            .next()
            .ok_or(GeneratedArtifactAuthenticationError::MatchingKernelNotFound)?;
        if matching.next().is_some() {
            return Err(GeneratedArtifactAuthenticationError::MultipleMatchingKernels);
        }

        let selected = container
            .select_native_kernel(kernel.kernel_id())
            .map_err(GeneratedArtifactAuthenticationError::Selection)?;
        let validated = ValidatedArtifactSelectionV1::validate(selected, observed)
            .map_err(GeneratedArtifactAuthenticationError::Binding)?;
        validate_generated_profile(K::PROFILE, K::KERNEL_BINDING_ID_V1, validated.identity())
            .map_err(GeneratedArtifactAuthenticationError::Profile)?;

        // SAFETY: `CompilerGeneratedKernelContractV1` requires the trusted
        // backend to establish the exact marker, identity, complete ABI,
        // effects, init/fini behavior, and executable provenance association.
        // The exact-name cardinality and structural artifact checks above
        // independently reject accidental selection or corruption.
        let binding = unsafe { validated.bind_generated_marker::<K>() }
            .map_err(GeneratedArtifactAuthenticationError::Marker)?;

        Ok(Self { validated, binding })
    }

    pub fn identity(&self) -> &ArtifactKernelIdentityV1 {
        self.validated.identity()
    }

    /// Consumes this authenticated token and safely loads its exact embedded
    /// payload into `context`.
    pub fn load(
        self,
        context: &Arc<fe2o3_core::GpuContext>,
    ) -> Result<LoadedKernel<K>, crate::loaded_kernel::LoadedKernelLoadError> {
        let Self { validated, binding } = self;
        let binding = binding.into_inner();

        // SAFETY: the unsafe generated contract authenticates the exact
        // embedded executable and establishes its marker, complete ABI,
        // effects, and load/unload behavior. `authenticate` decoded only those
        // bytes, selected exactly one matching entry, and applied the existing
        // target/ABI/payload checks. The private token fields preserve that
        // association until this consuming call; the internal loader rechecks
        // selection and exact context identity before invoking HIP.
        unsafe { binding.load(&validated, &validated.context, context) }
    }
}

/// Failure while authenticating a trusted backend's embedded artifact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedArtifactAuthenticationError {
    Decode(ContainerDecodeError),
    MatchingKernelNotFound,
    MultipleMatchingKernels,
    Selection(KernelSelectionError),
    Binding(ArtifactBindingError),
    Profile(GeneratedKernelProfileError),
    Marker(GeneratedMarkerBindingError),
}

impl fmt::Display for GeneratedArtifactAuthenticationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(formatter, "invalid embedded artifact: {error}"),
            Self::MatchingKernelNotFound => formatter
                .write_str("embedded artifact has no kernel matching the generated marker names"),
            Self::MultipleMatchingKernels => formatter.write_str(
                "embedded artifact has multiple kernels matching the generated marker names",
            ),
            Self::Selection(error) => error.fmt(formatter),
            Self::Binding(error) => error.fmt(formatter),
            Self::Profile(error) => error.fmt(formatter),
            Self::Marker(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedArtifactAuthenticationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::Marker(error) => Some(error),
            Self::MatchingKernelNotFound | Self::MultipleMatchingKernels => None,
        }
    }
}

/// Mismatch between an embedded manifest and generated typed host code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum GeneratedKernelProfileError {
    AbiMismatch,
    LaunchMismatch,
    GeneratedContractIdentityMismatch,
    KernelIdentityMismatch,
}

impl fmt::Display for GeneratedKernelProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiMismatch => formatter.write_str(
                "embedded artifact ABI/effects do not match the generated kernel profile",
            ),
            Self::LaunchMismatch => formatter.write_str(
                "embedded artifact launch contract does not match the generated kernel profile",
            ),
            Self::GeneratedContractIdentityMismatch => formatter.write_str(
                "embedded artifact does not match the independently generated contract identity",
            ),
            Self::KernelIdentityMismatch => formatter.write_str(
                "embedded artifact kernel identity does not match the generated binding and contract",
            ),
        }
    }
}

impl std::error::Error for GeneratedKernelProfileError {}

pub(crate) fn validate_generated_profile(
    profile: CompilerGeneratedKernelProfileV1,
    kernel_binding: [u8; 32],
    identity: &ArtifactKernelIdentityV1,
) -> Result<(), GeneratedKernelProfileError> {
    match profile {
        CompilerGeneratedKernelProfileV1::TypedVecAddF32V1 => {
            validate_typed_vecadd_abi(identity.abi())?;
            let launch = identity.launch();
            let exact_block = match launch.block_size() {
                BlockSize::Exact(block) => block,
                BlockSize::Any | BlockSize::AtMost(_) => {
                    return Err(GeneratedKernelProfileError::LaunchMismatch);
                }
            };
            if launch.rank() != 1
                || [exact_block.x(), exact_block.y(), exact_block.z()] != [256, 1, 1]
                || launch.max_grid().y() != 1
                || launch.max_grid().z() != 1
                || launch.static_shared_memory_bytes() != 0
                || launch.max_dynamic_shared_memory_bytes() != 0
            {
                return Err(GeneratedKernelProfileError::LaunchMismatch);
            }
            Ok(())
        }
        CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2 => {
            validate_typed_vecadd_rustc_layout_abi(identity.abi())?;
            let launch = identity.launch();
            let exact_block = match launch.block_size() {
                BlockSize::Exact(block) => block,
                BlockSize::Any | BlockSize::AtMost(_) => {
                    return Err(GeneratedKernelProfileError::LaunchMismatch);
                }
            };
            if launch.rank() != 1
                || [exact_block.x(), exact_block.y(), exact_block.z()] != [256, 1, 1]
                || launch.max_grid().y() != 1
                || launch.max_grid().z() != 1
                || launch.static_shared_memory_bytes() != 0
                || launch.max_dynamic_shared_memory_bytes() != 0
            {
                return Err(GeneratedKernelProfileError::LaunchMismatch);
            }
            let expected_kernel_id = derive_generated_kernel_identity_v2(
                TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
                kernel_binding,
                identity.name().as_str(),
                identity.symbol().as_str(),
                identity.source_digest(),
                identity.executable_digest(),
                identity.abi(),
                identity.launch(),
            );
            if identity.kernel_id().as_bytes() != expected_kernel_id.as_bytes() {
                return Err(GeneratedKernelProfileError::KernelIdentityMismatch);
            }
            Ok(())
        }
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity,
        } => {
            validate_manifest_derived_scalar_slice_abi(identity.abi())?;
            let derived_host_contract = derive_generated_host_contract_identity_v1(
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                kernel_binding,
                identity.name().as_str(),
                identity.symbol().as_str(),
                identity.abi(),
                identity.launch(),
            );
            if derived_host_contract.as_bytes() != &generated_host_contract_identity {
                return Err(GeneratedKernelProfileError::GeneratedContractIdentityMismatch);
            }
            let expected_kernel_id = derive_generated_kernel_identity_v2(
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                kernel_binding,
                identity.name().as_str(),
                identity.symbol().as_str(),
                identity.source_digest(),
                identity.executable_digest(),
                identity.abi(),
                identity.launch(),
            );
            if identity.kernel_id().as_bytes() != expected_kernel_id.as_bytes() {
                return Err(GeneratedKernelProfileError::KernelIdentityMismatch);
            }
            Ok(())
        }
    }
}

fn validate_manifest_derived_scalar_slice_abi(
    abi: &AbiLayout,
) -> Result<(), GeneratedKernelProfileError> {
    if abi.pointer_width() != PointerWidth::Bits64 || abi.fields().is_empty() {
        return Err(GeneratedKernelProfileError::AbiMismatch);
    }

    for field in abi.fields() {
        match field.kind() {
            AbiKind::Scalar(scalar) => {
                let (size, alignment) = scalar_size_alignment(scalar);
                if field.size() != size
                    || field.alignment() != alignment
                    || field.mutability() != Mutability::Immutable
                    || field.access() != Access::ByValue
                    || field.address_space() != AddressSpace::Value
                    || field.ownership() != ArgumentOwnership::ByValue
                    || field.alias_class() != AliasClass::Value
                {
                    return Err(GeneratedKernelProfileError::AbiMismatch);
                }
            }
            AbiKind::Slice {
                element_size,
                element_alignment,
            } => {
                if field.size() != 16
                    || field.alignment() != 8
                    || element_size == 0
                    || element_alignment == 0
                    || !element_alignment.is_power_of_two()
                    || !element_size.is_multiple_of(u64::from(element_alignment))
                    || field.address_space() != AddressSpace::Global
                {
                    return Err(GeneratedKernelProfileError::AbiMismatch);
                }
                let shared = field.mutability() == Mutability::Immutable
                    && field.access() == Access::ReadOnly
                    && field.ownership() == ArgumentOwnership::SharedBorrow
                    && field.alias_class() == AliasClass::SharedReadOnly;
                let exclusive = field.mutability() == Mutability::Mutable
                    && matches!(
                        field.access(),
                        Access::ReadOnly | Access::WriteOnly | Access::ReadWrite
                    )
                    && field.ownership() == ArgumentOwnership::UniqueBorrow
                    && field.alias_class() == AliasClass::Exclusive;
                if !shared && !exclusive {
                    return Err(GeneratedKernelProfileError::AbiMismatch);
                }
            }
            AbiKind::Pointer { .. } => {
                return Err(GeneratedKernelProfileError::AbiMismatch);
            }
        }
    }
    Ok(())
}

const fn scalar_size_alignment(scalar: ScalarType) -> (u64, u32) {
    match scalar {
        ScalarType::I8 | ScalarType::U8 => (1, 1),
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 => (2, 2),
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => (4, 4),
        ScalarType::I64 | ScalarType::U64 | ScalarType::F64 => (8, 8),
    }
}

fn validate_typed_vecadd_rustc_layout_abi(
    abi: &AbiLayout,
) -> Result<(), GeneratedKernelProfileError> {
    let type_identities = host_typed_vecadd_type_identities()?;
    validate_typed_vecadd_abi_with_identities(abi, type_identities)
}

fn validate_typed_vecadd_abi(abi: &AbiLayout) -> Result<(), GeneratedKernelProfileError> {
    if abi.size() != 48
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != 3
    {
        return Err(GeneratedKernelProfileError::AbiMismatch);
    }

    let shared_identity = generated_type_identity("&[f32]", "slice-f32-ptr64-size16-align8");
    let output_identity = generated_type_identity(
        "fe2o3_device::DisjointSlice<f32>",
        "disjoint-slice-f32-ptr64-size16-align8",
    );
    validate_typed_vecadd_abi_with_identities(
        abi,
        [shared_identity, shared_identity, output_identity],
    )
}

fn validate_typed_vecadd_abi_with_identities(
    abi: &AbiLayout,
    type_identities: [TypeIdentity; 3],
) -> Result<(), GeneratedKernelProfileError> {
    if abi.size() != 48
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != 3
    {
        return Err(GeneratedKernelProfileError::AbiMismatch);
    }

    let expected = [
        (
            0,
            Mutability::Immutable,
            Access::ReadOnly,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
            type_identities[0],
        ),
        (
            16,
            Mutability::Immutable,
            Access::ReadOnly,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
            type_identities[1],
        ),
        (
            32,
            Mutability::Mutable,
            Access::WriteOnly,
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
            type_identities[2],
        ),
    ];

    for (field, (offset, mutability, access, ownership, alias, type_identity)) in
        abi.fields().iter().zip(expected)
    {
        if field.offset() != offset
            || field.size() != 16
            || field.alignment() != 8
            || field.kind()
                != (AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                })
            || field.mutability() != mutability
            || field.access() != access
            || field.address_space() != AddressSpace::Global
            || field.ownership() != ownership
            || field.alias_class() != alias
            || field.type_identity() != type_identity
        {
            return Err(GeneratedKernelProfileError::AbiMismatch);
        }
    }
    Ok(())
}

pub(crate) fn host_typed_vecadd_type_identities()
-> Result<[TypeIdentity; 3], GeneratedKernelProfileError> {
    let pointer_size = u64::try_from(core::mem::size_of::<*const f32>())
        .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?;
    let pointer_alignment = u32::try_from(core::mem::align_of::<*const f32>())
        .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?;
    let usize_size = u64::try_from(core::mem::size_of::<usize>())
        .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?;
    let usize_alignment = u32::try_from(core::mem::align_of::<usize>())
        .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?;
    if pointer_size != 8 || pointer_alignment != 8 || usize_size != 8 || usize_alignment != 8 {
        return Err(GeneratedKernelProfileError::AbiMismatch);
    }

    let shared = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(
            RustScalarElementTypeV1::F32,
        )),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        u64::try_from(core::mem::size_of::<&[f32]>())
            .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?,
        u32::try_from(core::mem::align_of::<&[f32]>())
            .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?,
        vec![
            rust_layout_component(
                0,
                pointer_size,
                pointer_alignment,
                RustPhysicalComponentKindV1::Pointer {
                    mutability: RustPointerMutabilityV1::Const,
                    pointee: RustScalarElementTypeV1::F32,
                },
            )?,
            rust_layout_component(
                pointer_size,
                usize_size,
                usize_alignment,
                RustPhysicalComponentKindV1::Usize,
            )?,
        ],
    )
    .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?
    .type_identity();

    let (output_size, output_alignment, pointer_offset, length_offset) =
        DisjointSlice::<f32, Index1D>::__fe2o3_rust_layout_v1();
    let output = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::disjoint_slice(
            RustScalarElementTypeV1::F32,
            RustDisjointIndexSpaceV1::Index1D,
        )),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        u64::try_from(output_size).map_err(|_| GeneratedKernelProfileError::AbiMismatch)?,
        u32::try_from(output_alignment).map_err(|_| GeneratedKernelProfileError::AbiMismatch)?,
        vec![
            rust_layout_component(
                u64::try_from(pointer_offset)
                    .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?,
                pointer_size,
                pointer_alignment,
                RustPhysicalComponentKindV1::Pointer {
                    mutability: RustPointerMutabilityV1::Mut,
                    pointee: RustScalarElementTypeV1::F32,
                },
            )?,
            rust_layout_component(
                u64::try_from(length_offset)
                    .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?,
                usize_size,
                usize_alignment,
                RustPhysicalComponentKindV1::Usize,
            )?,
        ],
    )
    .map_err(|_| GeneratedKernelProfileError::AbiMismatch)?
    .type_identity();

    Ok([shared, shared, output])
}

fn rust_layout_component(
    offset: u64,
    size: u64,
    alignment: u32,
    kind: RustPhysicalComponentKindV1,
) -> Result<RustPhysicalComponentV1, GeneratedKernelProfileError> {
    RustPhysicalComponentV1::new(offset, size, alignment, kind)
        .map_err(|_| GeneratedKernelProfileError::AbiMismatch)
}

fn generated_type_identity(rust_type: &str, layout: &str) -> TypeIdentity {
    TypeIdentity::new(
        DeclaredRustTypeIdentity::from_untrusted_bytes(generated_profile_digest(
            TYPE_ID_DOMAIN,
            rust_type.as_bytes(),
        )),
        DeclaredRustLayoutIdentity::from_untrusted_bytes(generated_profile_digest(
            LAYOUT_ID_DOMAIN,
            layout.as_bytes(),
        )),
    )
}

fn generated_profile_digest(domain: &[u8], field: &[u8]) -> DigestBytes {
    let mut canonical = Vec::with_capacity(domain.len() + 8 + field.len());
    canonical.extend_from_slice(domain);
    canonical.extend_from_slice(&(field.len() as u64).to_le_bytes());
    canonical.extend_from_slice(field);
    DigestAlgorithm::Sha256.calculate(&canonical).bytes()
}

/// Unsafe generated-code association between marker `K` and one validated
/// artifact selection.
///
/// Fields are private so downstream code cannot manufacture or retarget a
/// binding. Possessing this value does not authenticate the payload or prove a
/// complete host ABI.
#[doc(hidden)]
pub struct GeneratedKernelBindingV1<K: KernelMarkerV1> {
    inner: ArtifactKernelBrandV1<K>,
}

impl<K: KernelMarkerV1> GeneratedKernelBindingV1<K> {
    pub(crate) fn into_inner(self) -> ArtifactKernelBrandV1<K> {
        self.inner
    }
}

/// Failure while matching generated marker names to a validated artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedMarkerBindingError {
    LogicalNameMismatch {
        marker: &'static str,
        artifact: String,
    },
    ExportNameMismatch {
        marker: &'static str,
        artifact: String,
    },
}

impl fmt::Display for GeneratedMarkerBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LogicalNameMismatch { marker, artifact } => write!(
                formatter,
                "generated marker logical name {marker:?} does not match artifact name {artifact:?}"
            ),
            Self::ExportNameMismatch { marker, artifact } => write!(
                formatter,
                "generated marker export name {marker:?} does not match artifact symbol {artifact:?}"
            ),
        }
    }
}

impl std::error::Error for GeneratedMarkerBindingError {}

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
#[path = "artifact_binding_authentication_tests.rs"]
mod authentication_tests;

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
    struct WrongLogicalName;
    struct WrongExportName;

    fn marker_function() {}

    unsafe impl KernelMarkerV1 for VecAdd {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "vector_add";
        const EXPORT_NAME: &'static str = "vector_add.kd";
        const FUNCTION: Self::Function = marker_function;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl KernelMarkerV1 for WrongLogicalName {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "not_vector_add";
        const EXPORT_NAME: &'static str = "vector_add.kd";
        const FUNCTION: Self::Function = marker_function;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl KernelMarkerV1 for WrongExportName {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "vector_add";
        const EXPORT_NAME: &'static str = "wrong_export.kd";
        const FUNCTION: Self::Function = marker_function;
        const REGISTRATION: &'static Self::Registration = &();
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
    fn generated_marker_binding_rejects_logical_and_export_name_mismatches() {
        let observed = context(7, 3, "gfx942");
        let validated = validate_fixture(FixtureSpec::default(), &observed).unwrap();

        let logical = unsafe { validated.bind_generated_marker::<WrongLogicalName>() }
            .err()
            .expect("logical-name mismatch must fail");
        assert_eq!(
            logical,
            GeneratedMarkerBindingError::LogicalNameMismatch {
                marker: "not_vector_add",
                artifact: "vector_add".to_owned(),
            }
        );

        let export = unsafe { validated.bind_generated_marker::<WrongExportName>() }
            .err()
            .expect("export-name mismatch must fail");
        assert_eq!(
            export,
            GeneratedMarkerBindingError::ExportNameMismatch {
                marker: "wrong_export.kd",
                artifact: "vector_add.kd".to_owned(),
            }
        );

        // SAFETY: the test fixture deliberately models the exact marker names;
        // it does not load or execute the unauthenticated fixture payload.
        assert!(unsafe { validated.bind_generated_marker::<VecAdd>() }.is_ok());
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
    fn loaded_admission_preserves_existing_artifact_authority() {
        let observed = context(17, 0, "gfx942");
        let validated = validate_fixture(FixtureSpec::default(), &observed).unwrap();
        let first = test_loaded(&validated);
        let second = test_loaded(&validated);
        let admitted = first
            .prepare(&observed, request(kernel_id(0x11)))
            .unwrap()
            .admit_arguments(std::iter::empty::<crate::ArgumentAccess<'static>>())
            .unwrap();

        assert_eq!(
            second.bind_admitted(admitted).unwrap_err(),
            crate::LoadedKernelMatchError::WrongArtifactAuthority
        );

        let admitted = first
            .prepare(&observed, request(kernel_id(0x11)))
            .unwrap()
            .admit_arguments(std::iter::empty::<crate::ArgumentAccess<'static>>())
            .unwrap();
        let bound = first.bind_admitted(admitted).unwrap();
        assert_eq!(bound.identity(), validated.identity());
        assert_eq!(bound.argument_count(), 0);
        assert_eq!(bound.geometry().grid().dimensions(), [17, 1, 1]);
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
