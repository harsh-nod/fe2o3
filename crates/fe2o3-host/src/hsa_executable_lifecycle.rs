use crate::{
    AdmittedFinalizedWorkerV2BundleV1, AdmittedWorkerV2TypedKernelV1, ArtifactKernelIdentityV1,
    CompilerGeneratedKernelContractV1, DeviceIdentity, FinalizedWorkerV2BundleAdmissionError,
    PhysicalMetadataValueV1, PublishedKernelPhysicalLayoutV1, PublishedPhysicalLaunchLayoutV1,
    WorkerV2TypedKernelSelectionError,
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{
    AbiLayout, BlockSize, DigestAlgorithm, DigestBytes, LaunchContract, PayloadDigest,
};
use fe2o3_hsaco::CodeObjectVersion;
use fe2o3_kernel_descriptor::KernelId;
use std::fmt;
use std::marker::PhantomData;

const REQUIRED_TARGET: &str = "gfx942";
const REQUIRED_RUNTIME_TARGET: &str = "gfx942:xnack-";
const HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const MAX_HSA_IDENTITY_TEXT_BYTES: usize = 256;

/// Safety properties an authenticated compiler/Verus chain must establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV2SafetyPropertyV1 {
    Bounds,
    AddressOverflowFreedom,
    MemorySafety,
    Initialization,
    RaceFreedom,
    LaunchValidity,
}

impl WorkerV2SafetyPropertyV1 {
    const fn bit(self) -> u8 {
        match self {
            Self::Bounds => 1 << 0,
            Self::AddressOverflowFreedom => 1 << 1,
            Self::MemorySafety => 1 << 2,
            Self::Initialization => 1 << 3,
            Self::RaceFreedom => 1 << 4,
            Self::LaunchValidity => 1 << 5,
        }
    }
}

/// Canonical set of safety properties reported by a reviewed authenticator.
///
/// Constructing this descriptive value grants no authority. The lifecycle
/// transition accepts it only as the result of an unsafe authenticator and
/// requires every property in [`Self::required`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2SafetyPropertiesV1(u8);

impl WorkerV2SafetyPropertiesV1 {
    const KNOWN_BITS: u8 = (1 << 6) - 1;

    pub const fn new(bits: u8) -> Option<Self> {
        if bits & !Self::KNOWN_BITS == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    pub const fn required() -> Self {
        Self(Self::KNOWN_BITS)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, property: WorkerV2SafetyPropertyV1) -> bool {
        self.0 & property.bit() != 0
    }
}

/// Descriptive decision returned by a reviewed compiler/Verus authenticator.
///
/// This type is intentionally constructible because its contents are evidence,
/// not authority. Only the private lifecycle transition can promote a decision,
/// and that transition obtains it through an unsafe authenticator and compares
/// every executable field against the admitted Worker V2 bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2PrerequisiteDecisionV1 {
    finalized_digest: PayloadDigest,
    kernel_id: KernelId,
    executable_digest: DigestBytes,
    target: AmdTargetId,
    code_object_version: CodeObjectVersion,
    logical_name: Box<str>,
    export_symbol: Box<str>,
    abi: AbiLayout,
    launch: LaunchContract,
    marker_binding_identity: [u8; 32],
    compiler_measurement: PayloadDigest,
    verus_transcript: PayloadDigest,
    proof_executable_binding: PayloadDigest,
    rust_type_layout_contract: PayloadDigest,
    rust_effect_contract: PayloadDigest,
    safety_properties: WorkerV2SafetyPropertiesV1,
}

impl WorkerV2PrerequisiteDecisionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        finalized_digest: PayloadDigest,
        kernel_id: KernelId,
        executable_digest: DigestBytes,
        target: AmdTargetId,
        code_object_version: CodeObjectVersion,
        logical_name: impl Into<Box<str>>,
        export_symbol: impl Into<Box<str>>,
        abi: AbiLayout,
        launch: LaunchContract,
        marker_binding_identity: [u8; 32],
        compiler_measurement: PayloadDigest,
        verus_transcript: PayloadDigest,
        proof_executable_binding: PayloadDigest,
        rust_type_layout_contract: PayloadDigest,
        rust_effect_contract: PayloadDigest,
        safety_properties: WorkerV2SafetyPropertiesV1,
    ) -> Self {
        Self {
            finalized_digest,
            kernel_id,
            executable_digest,
            target,
            code_object_version,
            logical_name: logical_name.into(),
            export_symbol: export_symbol.into(),
            abi,
            launch,
            marker_binding_identity,
            compiler_measurement,
            verus_transcript,
            proof_executable_binding,
            rust_type_layout_contract,
            rust_effect_contract,
            safety_properties,
        }
    }

    pub const fn finalized_digest(&self) -> PayloadDigest {
        self.finalized_digest
    }

    pub const fn compiler_measurement(&self) -> PayloadDigest {
        self.compiler_measurement
    }

    pub const fn verus_transcript(&self) -> PayloadDigest {
        self.verus_transcript
    }

    pub const fn proof_executable_binding(&self) -> PayloadDigest {
        self.proof_executable_binding
    }

    pub const fn rust_type_layout_contract(&self) -> PayloadDigest {
        self.rust_type_layout_contract
    }

    pub const fn rust_effect_contract(&self) -> PayloadDigest {
        self.rust_effect_contract
    }

    pub const fn safety_properties(&self) -> WorkerV2SafetyPropertiesV1 {
        self.safety_properties
    }
}

/// Exact challenge presented to a reviewed compiler/Verus authenticator.
pub struct WorkerV2PrerequisiteRequestV1<'admission, K> {
    artifact_identity: &'admission ArtifactKernelIdentityV1,
    finalized_digest: PayloadDigest,
    target: AmdTargetId,
    code_object_version: CodeObjectVersion,
    device: &'admission DeviceIdentity,
    _marker: PhantomData<fn() -> K>,
}

impl<K> WorkerV2PrerequisiteRequestV1<'_, K> {
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        self.artifact_identity
    }

    pub const fn finalized_digest(&self) -> PayloadDigest {
        self.finalized_digest
    }

    pub fn target(&self) -> AmdTargetId {
        self.target
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.device
    }
}

impl<K: CompilerGeneratedKernelContractV1> WorkerV2PrerequisiteRequestV1<'_, K> {
    pub const fn marker_logical_name(&self) -> &'static str {
        K::LOGICAL_NAME
    }

    pub const fn marker_export_name(&self) -> &'static str {
        K::EXPORT_NAME
    }

    pub const fn marker_binding_identity(&self) -> [u8; 32] {
        K::KERNEL_BINDING_ID_V1
    }
}

/// Reviewed boundary that authenticates compiler, Verus, Rust ABI, and effects.
///
/// # Safety
///
/// An implementation must authenticate immutable compiler and verifier
/// executions under an approved policy, bind their result to every field in
/// the request, and establish that `K` is the exact Rust marker whose complete
/// ABI and executable memory effects are represented. A false implementation
/// can authorize native code loading and dispatch from safe generated code.
pub unsafe trait WorkerV2PrerequisiteAuthenticatorV1<K: CompilerGeneratedKernelContractV1> {
    type Error;

    /// Authenticates all non-runtime prerequisites for one exact admission.
    ///
    /// # Safety
    ///
    /// The implementation obligations are those of the unsafe trait. Returned
    /// fields must derive from authenticated inputs rather than caller claims.
    unsafe fn authenticate(
        &mut self,
        request: &WorkerV2PrerequisiteRequestV1<'_, K>,
    ) -> Result<WorkerV2PrerequisiteDecisionV1, Self::Error>;
}

/// Opaque non-runtime authentication for one exact Worker V2 executable.
///
/// This state is neither `Clone` nor `Copy` and still grants no load authority;
/// it must first be paired with a reviewed HSA environment adapter.
pub struct AuthenticatedWorkerV2ExecutableV1<K> {
    admission: AdmittedFinalizedWorkerV2BundleV1,
    prerequisites: WorkerV2PrerequisiteDecisionV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for AuthenticatedWorkerV2ExecutableV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV2ExecutableV1")
            .field("artifact_identity", self.admission.artifact_identity())
            .field("finalized_digest", &self.prerequisites.finalized_digest)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelContractV1> AuthenticatedWorkerV2ExecutableV1<K> {
    pub fn authenticate<A: WorkerV2PrerequisiteAuthenticatorV1<K>>(
        admission: AdmittedFinalizedWorkerV2BundleV1,
        authenticator: &mut A,
    ) -> Result<Self, WorkerV2ExecutableAuthenticationError<A::Error>> {
        validate_required_profile(&admission)
            .map_err(WorkerV2ExecutableAuthenticationError::Profile)?;
        admission
            .acquire_currentness()
            .map_err(WorkerV2ExecutableAuthenticationError::CurrentPublication)?;
        let request = WorkerV2PrerequisiteRequestV1 {
            artifact_identity: admission.artifact_identity(),
            finalized_digest: admission.finalized_payload_identity().digest(),
            target: admission.target(),
            code_object_version: admission.code_object_version(),
            device: admission.device(),
            _marker: PhantomData,
        };
        // SAFETY: callers cannot reach this transition through a safe trait
        // implementation. Every returned field is independently checked below.
        let prerequisites = unsafe { authenticator.authenticate(&request) }
            .map_err(WorkerV2ExecutableAuthenticationError::Authenticator)?;
        validate_prerequisites::<K>(&request, &prerequisites)
            .map_err(WorkerV2ExecutableAuthenticationError::Prerequisite)?;
        Ok(Self {
            admission,
            prerequisites,
            _marker: PhantomData,
        })
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn prerequisites(&self) -> &WorkerV2PrerequisiteDecisionV1 {
        &self.prerequisites
    }

    pub fn authorize_hsa_load<A: ReviewedHsaExecutableLifecycleAdapterV1>(
        self,
        mut adapter: A,
    ) -> Result<AuthorizedHsaLoadV1<K, A>, HsaLoadAuthorizationError<A::Error>> {
        // SAFETY: the adapter is an unsafe implementation whose observation is
        // validated against the exact admission before authority is issued.
        let environment = reviewed_adapter_call(|| unsafe { adapter.observe_environment() })
            .map_err(HsaLoadAuthorizationError::Adapter)?;
        validate_environment(&self.admission, &environment)
            .map_err(HsaLoadAuthorizationError::Environment)?;
        Ok(AuthorizedHsaLoadV1 {
            authenticated: self,
            adapter,
            environment,
        })
    }
}

/// Failure while authenticating compiler/Verus and Rust marker prerequisites.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2ExecutableAuthenticationError<E> {
    Profile(WorkerV2RequiredProfileError),
    CurrentPublication(FinalizedWorkerV2BundleAdmissionError),
    Authenticator(E),
    Prerequisite(WorkerV2PrerequisiteError),
}

/// Required gfx942/COV6 profile mismatch.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV2RequiredProfileError {
    Target { actual: String },
    CodeObjectVersion { actual: u8 },
}

/// Mismatch in a reviewed prerequisite decision.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV2PrerequisiteError {
    IdentityMismatch(&'static str),
    EmptyAuthenticatedIdentity(&'static str),
    MissingSafetyProperty(WorkerV2SafetyPropertyV1),
}

fn validate_required_profile(
    admission: &AdmittedFinalizedWorkerV2BundleV1,
) -> Result<(), WorkerV2RequiredProfileError> {
    if admission.target().to_string() != REQUIRED_TARGET {
        return Err(WorkerV2RequiredProfileError::Target {
            actual: admission.target().to_string(),
        });
    }
    if admission.code_object_version() != CodeObjectVersion::V6 {
        return Err(WorkerV2RequiredProfileError::CodeObjectVersion {
            actual: admission.code_object_version().number(),
        });
    }
    Ok(())
}

fn validate_prerequisites<K: CompilerGeneratedKernelContractV1>(
    request: &WorkerV2PrerequisiteRequestV1<'_, K>,
    actual: &WorkerV2PrerequisiteDecisionV1,
) -> Result<(), WorkerV2PrerequisiteError> {
    let artifact = request.artifact_identity();
    for (matches, field) in [
        (
            actual.finalized_digest == request.finalized_digest(),
            "finalized code object",
        ),
        (actual.kernel_id == artifact.kernel_id(), "kernel"),
        (
            actual.executable_digest == artifact.executable_digest(),
            "executable semantics",
        ),
        (actual.target == request.target(), "target"),
        (
            actual.code_object_version == request.code_object_version(),
            "code-object version",
        ),
        (
            actual.logical_name.as_ref() == artifact.name().as_str()
                && actual.logical_name.as_ref() == K::LOGICAL_NAME,
            "Rust marker logical name",
        ),
        (
            actual.export_symbol.as_ref() == artifact.symbol().as_str()
                && actual.export_symbol.as_ref() == K::EXPORT_NAME,
            "Rust marker export symbol",
        ),
        (actual.abi == *artifact.abi(), "Rust ABI"),
        (actual.launch == *artifact.launch(), "launch contract"),
        (
            actual.marker_binding_identity == K::KERNEL_BINDING_ID_V1,
            "Rust marker binding",
        ),
    ] {
        if !matches {
            return Err(WorkerV2PrerequisiteError::IdentityMismatch(field));
        }
    }
    for (digest, field) in [
        (actual.compiler_measurement, "compiler measurement"),
        (actual.verus_transcript, "Verus transcript"),
        (actual.proof_executable_binding, "proof/executable binding"),
        (
            actual.rust_type_layout_contract,
            "Rust type/layout contract",
        ),
        (actual.rust_effect_contract, "Rust effect contract"),
    ] {
        require_authenticated_digest(digest, field)?;
    }
    for property in [
        WorkerV2SafetyPropertyV1::Bounds,
        WorkerV2SafetyPropertyV1::AddressOverflowFreedom,
        WorkerV2SafetyPropertyV1::MemorySafety,
        WorkerV2SafetyPropertyV1::Initialization,
        WorkerV2SafetyPropertyV1::RaceFreedom,
        WorkerV2SafetyPropertyV1::LaunchValidity,
    ] {
        if !actual.safety_properties.contains(property) {
            return Err(WorkerV2PrerequisiteError::MissingSafetyProperty(property));
        }
    }
    Ok(())
}

fn require_authenticated_digest(
    digest: PayloadDigest,
    field: &'static str,
) -> Result<(), WorkerV2PrerequisiteError> {
    if digest.algorithm() != DigestAlgorithm::Sha256
        || digest.bytes().as_bytes().iter().all(|byte| *byte == 0)
    {
        Err(WorkerV2PrerequisiteError::EmptyAuthenticatedIdentity(field))
    } else {
        Ok(())
    }
}

/// Process-local identity of one measured HSA runtime instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaRuntimeIdentityV1 {
    implementation: Box<str>,
    version: Box<str>,
    image_digest: PayloadDigest,
    instance: [u8; 16],
}

impl HsaRuntimeIdentityV1 {
    pub fn new(
        implementation: impl Into<Box<str>>,
        version: impl Into<Box<str>>,
        image_digest: PayloadDigest,
        instance: [u8; 16],
    ) -> Result<Self, HsaObservationError> {
        let implementation = implementation.into();
        let version = version.into();
        validate_identity_text(&implementation, "HSA implementation")?;
        validate_identity_text(&version, "HSA runtime version")?;
        validate_nonzero_digest(image_digest, "HSA runtime image")?;
        validate_nonzero_bytes(&instance, "HSA runtime instance")?;
        Ok(Self {
            implementation,
            version,
            image_digest,
            instance,
        })
    }

    pub fn implementation(&self) -> &str {
        &self.implementation
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn image_digest(&self) -> PayloadDigest {
        self.image_digest
    }

    pub const fn instance(&self) -> [u8; 16] {
        self.instance
    }
}

/// Physical device identity observed through a reviewed HSA adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaPhysicalDeviceIdentityV1 {
    uuid: [u8; 16],
    node_id: u32,
    hip_ordinal: i32,
    target: AmdTargetId,
}

impl HsaPhysicalDeviceIdentityV1 {
    pub fn new(
        uuid: [u8; 16],
        node_id: u32,
        hip_ordinal: i32,
        target: AmdTargetId,
    ) -> Result<Self, HsaObservationError> {
        validate_nonzero_bytes(&uuid, "physical device UUID")?;
        if hip_ordinal < 0 {
            return Err(HsaObservationError::InvalidOrdinal(hip_ordinal));
        }
        Ok(Self {
            uuid,
            node_id,
            hip_ordinal,
            target,
        })
    }

    pub const fn uuid(&self) -> [u8; 16] {
        self.uuid
    }

    pub const fn node_id(&self) -> u32 {
        self.node_id
    }

    pub const fn hip_ordinal(&self) -> i32 {
        self.hip_ordinal
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }
}

/// Exact process-local HSA agent observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaAgentIdentityV1 {
    runtime_instance: [u8; 16],
    agent_handle: u64,
    physical_device_uuid: [u8; 16],
    target: AmdTargetId,
}

impl HsaAgentIdentityV1 {
    pub fn new(
        runtime_instance: [u8; 16],
        agent_handle: u64,
        physical_device_uuid: [u8; 16],
        target: AmdTargetId,
    ) -> Result<Self, HsaObservationError> {
        validate_nonzero_bytes(&runtime_instance, "HSA runtime instance")?;
        if agent_handle == 0 {
            return Err(HsaObservationError::ZeroIdentity("HSA agent handle"));
        }
        validate_nonzero_bytes(&physical_device_uuid, "physical device UUID")?;
        Ok(Self {
            runtime_instance,
            agent_handle,
            physical_device_uuid,
            target,
        })
    }

    pub const fn runtime_instance(&self) -> [u8; 16] {
        self.runtime_instance
    }

    pub const fn agent_handle(&self) -> u64 {
        self.agent_handle
    }

    pub const fn physical_device_uuid(&self) -> [u8; 16] {
        self.physical_device_uuid
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }
}

/// Internally consistent runtime, physical-device, and HSA-agent observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaEnvironmentObservationV1 {
    runtime: HsaRuntimeIdentityV1,
    physical_device: HsaPhysicalDeviceIdentityV1,
    agent: HsaAgentIdentityV1,
}

impl HsaEnvironmentObservationV1 {
    pub fn new(
        runtime: HsaRuntimeIdentityV1,
        physical_device: HsaPhysicalDeviceIdentityV1,
        agent: HsaAgentIdentityV1,
    ) -> Result<Self, HsaObservationError> {
        if agent.runtime_instance != runtime.instance {
            return Err(HsaObservationError::IdentityMismatch("runtime instance"));
        }
        if agent.physical_device_uuid != physical_device.uuid {
            return Err(HsaObservationError::IdentityMismatch(
                "physical device UUID",
            ));
        }
        if agent.target != physical_device.target {
            return Err(HsaObservationError::IdentityMismatch("agent target"));
        }
        Ok(Self {
            runtime,
            physical_device,
            agent,
        })
    }

    pub const fn runtime(&self) -> &HsaRuntimeIdentityV1 {
        &self.runtime
    }

    pub const fn physical_device(&self) -> &HsaPhysicalDeviceIdentityV1 {
        &self.physical_device
    }

    pub const fn agent(&self) -> &HsaAgentIdentityV1 {
        &self.agent
    }
}

/// Failure while validating descriptive HSA identity values.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HsaObservationError {
    EmptyText(&'static str),
    InvalidText(&'static str),
    TextTooLong(&'static str),
    UnsupportedDigest(&'static str),
    ZeroIdentity(&'static str),
    InvalidOrdinal(i32),
    IdentityMismatch(&'static str),
}

/// Opaque identity assigned to one loaded HSA executable object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HsaExecutableObjectIdentityV1([u8; 32]);

impl HsaExecutableObjectIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, HsaObservationError> {
        validate_nonzero_bytes(&bytes, "HSA executable object")?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Opaque identity assigned to one resolved HSA kernel object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HsaKernelObjectIdentityV1([u8; 32]);

impl HsaKernelObjectIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, HsaObservationError> {
        validate_nonzero_bytes(&bytes, "HSA kernel object")?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Adapter-reported result of loading one exact finalized code object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaCodeObjectLoadObservationV1 {
    finalized_digest: PayloadDigest,
    byte_len: u64,
    runtime_instance: [u8; 16],
    agent_handle: u64,
    executable_object: HsaExecutableObjectIdentityV1,
}

impl HsaCodeObjectLoadObservationV1 {
    pub const fn new(
        finalized_digest: PayloadDigest,
        byte_len: u64,
        runtime_instance: [u8; 16],
        agent_handle: u64,
        executable_object: HsaExecutableObjectIdentityV1,
    ) -> Self {
        Self {
            finalized_digest,
            byte_len,
            runtime_instance,
            agent_handle,
            executable_object,
        }
    }

    pub const fn finalized_digest(&self) -> PayloadDigest {
        self.finalized_digest
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }
}

/// Adapter-reported binding from export symbol to HSA kernel object and ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaKernelResolutionObservationV1 {
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    export_symbol: Box<str>,
    kernarg_segment_size: u64,
    kernarg_segment_alignment: u64,
}

impl HsaKernelResolutionObservationV1 {
    pub fn new(
        executable_object: HsaExecutableObjectIdentityV1,
        kernel_object: HsaKernelObjectIdentityV1,
        export_symbol: impl Into<Box<str>>,
        kernarg_segment_size: u64,
        kernarg_segment_alignment: u64,
    ) -> Result<Self, HsaObservationError> {
        let export_symbol = export_symbol.into();
        validate_identity_text(&export_symbol, "HSA kernel symbol")?;
        if kernarg_segment_size == 0 {
            return Err(HsaObservationError::ZeroIdentity("kernarg segment size"));
        }
        if kernarg_segment_alignment == 0 || !kernarg_segment_alignment.is_power_of_two() {
            return Err(HsaObservationError::IdentityMismatch(
                "kernarg segment alignment",
            ));
        }
        Ok(Self {
            executable_object,
            kernel_object,
            export_symbol,
            kernarg_segment_size,
            kernarg_segment_alignment,
        })
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.kernel_object
    }

    pub fn export_symbol(&self) -> &str {
        &self.export_symbol
    }

    pub const fn kernarg_segment_size(&self) -> u64 {
        self.kernarg_segment_size
    }

    pub const fn kernarg_segment_alignment(&self) -> u64 {
        self.kernarg_segment_alignment
    }
}

/// One exact dispatch geometry admitted against source and physical contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HsaLaunchGeometryV1 {
    grid: [u32; 3],
    workgroup: [u32; 3],
    dynamic_shared_memory_bytes: u32,
}

impl HsaLaunchGeometryV1 {
    pub const fn new(
        grid: [u32; 3],
        workgroup: [u32; 3],
        dynamic_shared_memory_bytes: u32,
    ) -> Self {
        Self {
            grid,
            workgroup,
            dynamic_shared_memory_bytes,
        }
    }

    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    pub const fn dynamic_shared_memory_bytes(self) -> u32 {
        self.dynamic_shared_memory_bytes
    }
}

/// Adapter-reported completion of one synchronous HSA dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaDispatchObservationV1 {
    dispatch_identity: [u8; 16],
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    geometry: HsaLaunchGeometryV1,
    completed: bool,
}

impl HsaDispatchObservationV1 {
    pub fn new(
        dispatch_identity: [u8; 16],
        executable_object: HsaExecutableObjectIdentityV1,
        kernel_object: HsaKernelObjectIdentityV1,
        geometry: HsaLaunchGeometryV1,
        completed: bool,
    ) -> Result<Self, HsaObservationError> {
        validate_nonzero_bytes(&dispatch_identity, "HSA dispatch")?;
        Ok(Self {
            dispatch_identity,
            executable_object,
            kernel_object,
            geometry,
            completed,
        })
    }

    pub const fn dispatch_identity(&self) -> [u8; 16] {
        self.dispatch_identity
    }

    pub const fn completed(&self) -> bool {
        self.completed
    }
}

/// Adapter-reported terminal state for an HSA executable object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaUnloadObservationV1 {
    executable_object: HsaExecutableObjectIdentityV1,
    runtime_instance: [u8; 16],
    agent_handle: u64,
    released: bool,
}

impl HsaUnloadObservationV1 {
    pub const fn new(
        executable_object: HsaExecutableObjectIdentityV1,
        runtime_instance: [u8; 16],
        agent_handle: u64,
        released: bool,
    ) -> Self {
        Self {
            executable_object,
            runtime_instance,
            agent_handle,
            released,
        }
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn released(&self) -> bool {
        self.released
    }
}

/// Reviewed unsafe adapter for the HSA executable lifecycle.
///
/// # Safety
///
/// Implementations must obtain identities from the same initialized HSA
/// runtime instance, map the HSA agent to the reported physical HIP device,
/// load exactly the supplied bytes, resolve exactly the supplied symbol,
/// report ABI properties queried from that kernel object, synchronously wait
/// for dispatch completion, and release the exact executable on unload. A
/// launch error may return only before packet publication or after quiescence.
/// Once publication may have occurred, an implementation must retain all
/// device-reachable authority in a non-returning state or terminate the process
/// if bounded quiescence cannot be established. No adapter method may unwind:
/// unwinding across an unsafe lifecycle transition makes native authority
/// ambiguous. An `Err` from load, symbol resolution, or implicit-kernarg
/// initialization must mean that no native authority from that operation
/// remains live. Handles must remain valid while owned by the adapter state and
/// must never be reused under an existing identity.
pub unsafe trait ReviewedHsaExecutableLifecycleAdapterV1 {
    type Executable;
    type Kernel;
    type Error;

    /// Observes the runtime, physical device, and agent used by later calls.
    ///
    /// # Safety
    ///
    /// The observation must satisfy the unsafe trait contract.
    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error>;

    /// Loads the exact supplied finalized bytes into the observed agent.
    ///
    /// # Safety
    ///
    /// The returned handle and observation must denote only `bytes`. `Err` may
    /// be returned only after every partially created reader, executable, and
    /// backing allocation has been conclusively released. This method must not
    /// unwind.
    unsafe fn load_executable(
        &mut self,
        bytes: &[u8],
        finalized_digest: PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error>;

    /// Resolves the exact export symbol and queries its physical kernarg ABI.
    ///
    /// # Safety
    ///
    /// The kernel handle must remain tied to `executable` until it is dropped.
    /// `Err` may be returned only after every partially created kernel handle
    /// and query resource has been conclusively released. This method must not
    /// unwind.
    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error>;

    /// Dispatches and waits until all effects have quiesced.
    ///
    /// # Safety
    ///
    /// The adapter must use the exact handles, geometry, and kernarg storage.
    /// It may return only after proving no packet was submitted or after all
    /// submitted effects are quiescent. Success additionally requires an exact
    /// completion observation. This method must not unwind before or after
    /// packet publication.
    unsafe fn launch_and_wait(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error>;

    /// Releases the exact executable after all kernel handles are dropped.
    ///
    /// # Safety
    ///
    /// Success must mean the executable and all runtime-owned lifecycle state
    /// are fully released. Failure is treated as an ambiguous terminal state,
    /// and this method must not unwind.
    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error>;
}

/// Reviewed extension that prepares compiler-declared implicit kernargs.
///
/// The base lifecycle adapter deliberately accepts only complete raw kernarg
/// storage. Generated typed launch code uses this extension so application code
/// never constructs or initializes AMDHSA hidden arguments.
///
/// # Safety
///
/// Implementations must preserve every byte in the explicit span, initialize
/// the complete implicit span for the exact executable, kernel, and geometry,
/// and report an observation derived from that same operation. Success must not
/// be reported while any hidden byte required by the code-object ABI remains
/// uninitialized. Implementing this trait does not itself grant launch
/// authority; the lifecycle validates the observation before dispatch.
pub unsafe trait ReviewedHsaImplicitKernargAdapterV1:
    ReviewedHsaExecutableLifecycleAdapterV1
{
    /// Initializes only the compiler-declared implicit span in `kernarg`.
    ///
    /// # Safety
    ///
    /// The implementation obligations are those of the unsafe trait. The
    /// executable and kernel are the exact private handles retained by the
    /// lifecycle, and all spans have already been bounds checked. `Err` may be
    /// returned only when no queue, callback, or other native authority created
    /// by this operation remains live. This method must not unwind.
    #[allow(clippy::too_many_arguments)]
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error>;
}

/// Adapter-reported completion of one implicit-kernarg initialization.
///
/// This is descriptive data. Only the private generated lifecycle transition
/// can reconcile it with exact loaded handles and issue dispatch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HsaImplicitKernargInitializationObservationV1 {
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: u64,
    implicit_byte_offset: u64,
    implicit_byte_len: u64,
    initialized: bool,
}

impl HsaImplicitKernargInitializationObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        executable_object: HsaExecutableObjectIdentityV1,
        kernel_object: HsaKernelObjectIdentityV1,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: u64,
        implicit_byte_offset: u64,
        implicit_byte_len: u64,
        initialized: bool,
    ) -> Self {
        Self {
            executable_object,
            kernel_object,
            geometry,
            explicit_byte_len,
            implicit_byte_offset,
            implicit_byte_len,
            initialized,
        }
    }

    pub const fn initialized(&self) -> bool {
        self.initialized
    }
}

/// Environment-authenticated permission to load one exact finalized code object.
///
/// This state owns the reviewed adapter and is intentionally linear.
pub struct AuthorizedHsaLoadV1<K, A: ReviewedHsaExecutableLifecycleAdapterV1> {
    authenticated: AuthenticatedWorkerV2ExecutableV1<K>,
    adapter: A,
    environment: HsaEnvironmentObservationV1,
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> AuthorizedHsaLoadV1<K, A> {
    pub const fn grants_load_authority(&self) -> bool {
        true
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn environment(&self) -> &HsaEnvironmentObservationV1 {
        &self.environment
    }

    pub fn load(mut self) -> Result<LoadedHsaExecutableV1<K, A>, HsaExecutableLoadError<A::Error>> {
        let current = self
            .authenticated
            .admission
            .acquire_currentness()
            .map_err(HsaExecutableLoadError::CurrentPublication)?;
        let bytes = current.exact_artifact_bytes();
        let digest = self.authenticated.prerequisites.finalized_digest;
        digest
            .verify(bytes)
            .map_err(|_| HsaExecutableLoadError::ExactBytesChanged)?;
        let byte_len =
            u64::try_from(bytes.len()).map_err(|_| HsaExecutableLoadError::ExactBytesChanged)?;
        // SAFETY: authority is bound to this reviewed adapter and exact locked
        // bytes. The returned observation is checked before it can advance.
        let (executable, load) =
            reviewed_adapter_call(|| unsafe { self.adapter.load_executable(bytes, digest) })
                .map_err(HsaExecutableLoadError::AdapterLoad)?;
        drop(current);

        if let Err(field) = validate_load_observation(&self.environment, digest, byte_len, &load) {
            terminal_unload(&mut self.adapter, executable, &self.environment, &load);
            return Err(HsaExecutableLoadError::LoadObservationMismatch {
                field,
                cleanup: None,
            });
        }

        let symbol = self
            .authenticated
            .admission
            .selected_kernel()
            .export_symbol();
        // SAFETY: the executable survived exact load observation validation;
        // the adapter must resolve only this exact symbol against that handle.
        let (kernel, resolution) = match reviewed_adapter_call(|| unsafe {
            self.adapter.resolve_kernel(&executable, symbol)
        }) {
            Ok(resolved) => resolved,
            Err(source) => {
                terminal_unload(&mut self.adapter, executable, &self.environment, &load);
                return Err(HsaExecutableLoadError::KernelResolution {
                    source,
                    cleanup: None,
                });
            }
        };
        if let Err(field) = validate_kernel_resolution(
            &self.authenticated.admission,
            load.executable_object,
            &resolution,
        ) {
            drop(kernel);
            terminal_unload(&mut self.adapter, executable, &self.environment, &load);
            return Err(HsaExecutableLoadError::KernelObservationMismatch {
                field,
                cleanup: None,
            });
        }

        Ok(LoadedHsaExecutableV1 {
            authenticated: self.authenticated,
            adapter: self.adapter,
            environment: self.environment,
            executable: Some(executable),
            kernel: Some(kernel),
            load,
            resolution,
        })
    }
}

/// Failure while binding an authenticated executable to an HSA environment.
#[derive(Debug)]
#[non_exhaustive]
pub enum HsaLoadAuthorizationError<E> {
    Adapter(E),
    Environment(HsaEnvironmentMismatch),
}

/// Exact HSA environment mismatch.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HsaEnvironmentMismatch {
    Target { actual: String },
    DeviceOrdinal { expected: i32, actual: i32 },
    RuntimeInstance,
    PhysicalDevice,
}

fn validate_environment(
    admission: &AdmittedFinalizedWorkerV2BundleV1,
    environment: &HsaEnvironmentObservationV1,
) -> Result<(), HsaEnvironmentMismatch> {
    let expected_target = admission.target();
    let actual_target = environment.physical_device.target;
    let required_runtime_target = AmdTargetId::parse(REQUIRED_RUNTIME_TARGET)
        .expect("reviewed runtime target ID is a valid static constant");
    if expected_target.to_string() != REQUIRED_TARGET
        || !expected_target.is_compatible_with_observed(&actual_target)
        || !required_runtime_target.is_compatible_with_observed(&actual_target)
        || environment.agent.target != actual_target
    {
        return Err(HsaEnvironmentMismatch::Target {
            actual: actual_target.to_string(),
        });
    }
    if environment.physical_device.hip_ordinal != admission.device().ordinal() {
        return Err(HsaEnvironmentMismatch::DeviceOrdinal {
            expected: admission.device().ordinal(),
            actual: environment.physical_device.hip_ordinal,
        });
    }
    if environment.agent.runtime_instance != environment.runtime.instance {
        return Err(HsaEnvironmentMismatch::RuntimeInstance);
    }
    if environment.agent.physical_device_uuid != environment.physical_device.uuid {
        return Err(HsaEnvironmentMismatch::PhysicalDevice);
    }
    Ok(())
}

/// Failure while loading or resolving an HSA executable.
#[derive(Debug)]
#[non_exhaustive]
pub enum HsaExecutableLoadError<E> {
    CurrentPublication(FinalizedWorkerV2BundleAdmissionError),
    ExactBytesChanged,
    AdapterLoad(E),
    LoadObservationMismatch {
        field: &'static str,
        cleanup: Option<E>,
    },
    KernelResolution {
        source: E,
        cleanup: Option<E>,
    },
    KernelObservationMismatch {
        field: &'static str,
        cleanup: Option<E>,
    },
}

fn validate_load_observation(
    environment: &HsaEnvironmentObservationV1,
    digest: PayloadDigest,
    byte_len: u64,
    load: &HsaCodeObjectLoadObservationV1,
) -> Result<(), &'static str> {
    for (matches, field) in [
        (load.finalized_digest == digest, "finalized digest"),
        (load.byte_len == byte_len, "finalized byte length"),
        (
            load.runtime_instance == environment.runtime.instance,
            "HSA runtime instance",
        ),
        (
            load.agent_handle == environment.agent.agent_handle,
            "HSA agent handle",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

fn validate_kernel_resolution(
    admission: &AdmittedFinalizedWorkerV2BundleV1,
    executable: HsaExecutableObjectIdentityV1,
    resolution: &HsaKernelResolutionObservationV1,
) -> Result<(), &'static str> {
    let selected = admission.selected_kernel();
    let physical = selected.launch();
    let expected_hsa_alignment = physical
        .kernarg_segment_alignment()
        .max(HSA_MINIMUM_KERNARG_ALIGNMENT);
    for (matches, field) in [
        (
            resolution.executable_object == executable,
            "HSA executable object",
        ),
        (
            resolution.export_symbol.as_ref() == selected.export_symbol(),
            "HSA kernel symbol",
        ),
        (
            resolution.kernarg_segment_size == physical.kernarg_segment_size(),
            "kernarg segment size",
        ),
        (
            resolution.kernarg_segment_alignment == expected_hsa_alignment,
            "kernarg segment alignment",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

/// Loaded and resolved HSA authority for one exact Worker V2 kernel.
///
/// The raw executable and kernel handles are private. Kernel launch authority
/// borrows this value, preventing unload while a launch authorization exists.
/// Dropping without explicit unload invokes the reviewed adapter; an ambiguous
/// drop-time unload aborts the process rather than continuing after releasing
/// the evidence while native state may remain live.
pub struct LoadedHsaExecutableV1<K, A: ReviewedHsaExecutableLifecycleAdapterV1> {
    authenticated: AuthenticatedWorkerV2ExecutableV1<K>,
    adapter: A,
    environment: HsaEnvironmentObservationV1,
    executable: Option<A::Executable>,
    kernel: Option<A::Kernel>,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> fmt::Debug for LoadedHsaExecutableV1<K, A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedHsaExecutableV1")
            .field(
                "artifact_identity",
                self.authenticated.admission.artifact_identity(),
            )
            .field("environment", &self.environment)
            .field("load", &self.load)
            .field("resolution", &self.resolution)
            .finish_non_exhaustive()
    }
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> LoadedHsaExecutableV1<K, A> {
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn load_observation(&self) -> &HsaCodeObjectLoadObservationV1 {
        &self.load
    }

    pub const fn kernel_observation(&self) -> &HsaKernelResolutionObservationV1 {
        &self.resolution
    }

    pub(crate) const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        self.authenticated.admission.artifact_identity()
    }

    pub(crate) fn physical_kernel(&self) -> &crate::PublishedKernelPhysicalLayoutV1 {
        self.authenticated.admission.selected_kernel()
    }

    pub(crate) const fn environment(&self) -> &HsaEnvironmentObservationV1 {
        &self.environment
    }

    /// Selects another typed kernel from this exact loaded executable.
    ///
    /// The result is deliberately inert. It binds the exact HSA executable
    /// object to admission-scoped marker/ABI/effect evidence, but it does not
    /// resolve the second HSA kernel handle or authenticate that marker's
    /// compiler/Verus prerequisite decision. Consequently it has no dispatch
    /// operation and cannot be converted into the existing vecadd executor.
    #[doc(hidden)]
    pub fn select_typed_kernel<S: CompilerGeneratedKernelContractV1>(
        &self,
    ) -> Result<InertLoadedWorkerV2KernelSelectionV1<'_, S>, WorkerV2TypedKernelSelectionError>
    {
        let selected = self.authenticated.admission.select_typed_kernel::<S>()?;
        if selected.finalized_payload_identity().digest() != self.load.finalized_digest {
            return Err(WorkerV2TypedKernelSelectionError::ExecutableSubstitution);
        }
        Ok(InertLoadedWorkerV2KernelSelectionV1 {
            selected,
            executable_object: self.load.executable_object,
            environment: self.environment.clone(),
            target: self.authenticated.admission.target(),
            code_object_version: self.authenticated.admission.code_object_version(),
            device: self.authenticated.admission.device(),
        })
    }

    pub fn authorize_launch(
        &mut self,
        geometry: HsaLaunchGeometryV1,
    ) -> Result<HsaKernelLaunchAuthorizationV1<'_, K, A>, HsaLaunchAuthorizationError> {
        validate_launch_geometry(&self.authenticated.admission, geometry)?;
        Ok(HsaKernelLaunchAuthorizationV1 {
            loaded: self,
            geometry,
        })
    }

    pub fn unload(mut self) -> Result<UnloadedHsaExecutableV1, HsaExecutableUnloadError<A::Error>> {
        self.kernel.take();
        let executable = self
            .executable
            .take()
            .expect("loaded executable state must own an executable");
        // SAFETY: launch authority borrows `self`, so this consuming method can
        // run only after all launch witnesses and synchronous dispatches end.
        let unload = terminal_unload(&mut self.adapter, executable, &self.environment, &self.load);
        Ok(UnloadedHsaExecutableV1 {
            finalized_digest: self.load.finalized_digest,
            executable_object: self.load.executable_object,
            runtime: self.environment.runtime.clone(),
            physical_device: self.environment.physical_device.clone(),
            agent: self.environment.agent.clone(),
            unload,
        })
    }
}

/// Typed selection bound to one exact live HSA executable object.
///
/// This is the integration boundary for general multi-kernel execution. A
/// future transition must consume this evidence, authenticate the selected
/// marker's compiler/Verus prerequisites, resolve its symbol against the
/// retained executable handle, and validate that resolution before issuing
/// launch authority. Fields are private, the loaded executable remains
/// borrowed, and this value is intentionally neither `Clone` nor `Copy`.
#[doc(hidden)]
pub struct InertLoadedWorkerV2KernelSelectionV1<'loaded, K> {
    selected: AdmittedWorkerV2TypedKernelV1<'loaded, K>,
    executable_object: HsaExecutableObjectIdentityV1,
    environment: HsaEnvironmentObservationV1,
    target: AmdTargetId,
    code_object_version: CodeObjectVersion,
    device: &'loaded DeviceIdentity,
}

impl<K> fmt::Debug for InertLoadedWorkerV2KernelSelectionV1<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertLoadedWorkerV2KernelSelectionV1")
            .field("artifact_identity", self.artifact_identity())
            .field("executable_object", &self.executable_object)
            .finish_non_exhaustive()
    }
}

impl<K> InertLoadedWorkerV2KernelSelectionV1<'_, K> {
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        self.selected.artifact_identity()
    }

    pub const fn physical_kernel(&self) -> &PublishedKernelPhysicalLayoutV1 {
        self.selected.physical_kernel()
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn requires_prerequisite_authentication(&self) -> bool {
        true
    }

    pub const fn requires_hsa_kernel_resolution(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<K: CompilerGeneratedKernelContractV1> InertLoadedWorkerV2KernelSelectionV1<'_, K> {
    /// Consumes this loaded-executable selection and authenticates the exact
    /// selected marker's compiler, Verus, ABI, and effect prerequisites.
    ///
    /// The returned state owns only validated evidence. It retains no native
    /// handle and grants no load or launch authority, so the original borrow
    /// can end before the reviewed adapter is mutably entered for resolution.
    #[doc(hidden)]
    pub fn authenticate<A: WorkerV2PrerequisiteAuthenticatorV1<K>>(
        self,
        authenticator: &mut A,
    ) -> Result<
        AuthenticatedLoadedWorkerV2KernelSelectionV1<K>,
        WorkerV2ExecutableAuthenticationError<A::Error>,
    > {
        let request = WorkerV2PrerequisiteRequestV1 {
            artifact_identity: self.selected.artifact_identity(),
            finalized_digest: self.selected.finalized_payload_identity().digest(),
            target: self.target,
            code_object_version: self.code_object_version,
            device: self.device,
            _marker: PhantomData,
        };
        // SAFETY: callers cannot reach this transition through a safe trait
        // implementation. The exact selected-kernel fields are checked below.
        let prerequisites = unsafe { authenticator.authenticate(&request) }
            .map_err(WorkerV2ExecutableAuthenticationError::Authenticator)?;
        validate_prerequisites::<K>(&request, &prerequisites)
            .map_err(WorkerV2ExecutableAuthenticationError::Prerequisite)?;

        Ok(AuthenticatedLoadedWorkerV2KernelSelectionV1 {
            artifact_identity: self.selected.artifact_identity().clone(),
            physical_kernel: self.selected.physical_kernel().clone(),
            executable_object: self.executable_object,
            environment: self.environment,
            finalized_digest: request.finalized_digest,
            target: request.target,
            code_object_version: request.code_object_version,
            prerequisites,
            _marker: PhantomData,
        })
    }
}

/// Authenticated evidence for one selected kernel in an already loaded object.
///
/// This intermediate state is intentionally non-`Clone` and carries no native
/// authority. Resolving it revalidates the loaded object before entering the
/// reviewed adapter, then returns a token that borrows that object for as long
/// as the resolved kernel handle remains live.
#[doc(hidden)]
pub struct AuthenticatedLoadedWorkerV2KernelSelectionV1<K> {
    artifact_identity: ArtifactKernelIdentityV1,
    physical_kernel: PublishedKernelPhysicalLayoutV1,
    executable_object: HsaExecutableObjectIdentityV1,
    environment: HsaEnvironmentObservationV1,
    finalized_digest: PayloadDigest,
    target: AmdTargetId,
    code_object_version: CodeObjectVersion,
    prerequisites: WorkerV2PrerequisiteDecisionV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for AuthenticatedLoadedWorkerV2KernelSelectionV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedLoadedWorkerV2KernelSelectionV1")
            .field("artifact_identity", &self.artifact_identity)
            .field("executable_object", &self.executable_object)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelContractV1> AuthenticatedLoadedWorkerV2KernelSelectionV1<K> {
    pub const fn requires_prerequisite_authentication(&self) -> bool {
        false
    }

    pub const fn requires_hsa_kernel_resolution(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Resolves the authenticated symbol through the exact retained adapter.
    ///
    /// The returned opaque state owns the selected kernel handle and borrows
    /// `loaded`, preventing executable unload until that handle is dropped. It
    /// deliberately exposes no dispatch transition.
    #[doc(hidden)]
    pub fn resolve<'loaded, P, A>(
        self,
        loaded: &'loaded mut LoadedHsaExecutableV1<P, A>,
    ) -> Result<
        ResolvedLoadedWorkerV2KernelSelectionV1<'loaded, P, K, A>,
        HsaExecutableLoadError<A::Error>,
    >
    where
        A: ReviewedHsaExecutableLifecycleAdapterV1,
    {
        validate_authenticated_selection_against_loaded(&self, loaded).map_err(|field| {
            HsaExecutableLoadError::KernelObservationMismatch {
                field,
                cleanup: None,
            }
        })?;

        let executable = loaded
            .executable
            .as_ref()
            .expect("loaded executable state must own an executable");
        // SAFETY: the selected evidence has been authenticated and rebound to
        // this exact live object. The reviewed adapter's complete observation
        // is independently checked before the handle can escape this method.
        let (kernel, resolution) = reviewed_adapter_call(|| unsafe {
            loaded
                .adapter
                .resolve_kernel(executable, self.artifact_identity.symbol().as_str())
        })
        .map_err(|source| HsaExecutableLoadError::KernelResolution {
            source,
            cleanup: None,
        })?;

        if let Err(field) = validate_selected_kernel_resolution(
            &self.artifact_identity,
            &self.physical_kernel,
            self.executable_object,
            &loaded.resolution,
            &resolution,
        ) {
            drop(kernel);
            return Err(HsaExecutableLoadError::KernelObservationMismatch {
                field,
                cleanup: None,
            });
        }

        Ok(ResolvedLoadedWorkerV2KernelSelectionV1 {
            artifact_identity: self.artifact_identity,
            physical_kernel: self.physical_kernel,
            prerequisites: self.prerequisites,
            environment: self.environment,
            resolution,
            kernel,
            loaded,
            _marker: PhantomData,
        })
    }
}

fn validate_authenticated_selection_against_loaded<K, P, A>(
    selected: &AuthenticatedLoadedWorkerV2KernelSelectionV1<K>,
    loaded: &LoadedHsaExecutableV1<P, A>,
) -> Result<(), &'static str>
where
    A: ReviewedHsaExecutableLifecycleAdapterV1,
{
    for (matches, field) in [
        (
            selected.executable_object == loaded.load.executable_object,
            "HSA executable object",
        ),
        (
            selected.environment == loaded.environment,
            "HSA environment",
        ),
        (
            selected.finalized_digest == loaded.load.finalized_digest,
            "finalized digest",
        ),
        (
            selected.artifact_identity.payload_digest() == loaded.load.finalized_digest,
            "selected payload digest",
        ),
        (
            selected.target == loaded.authenticated.admission.target(),
            "target",
        ),
        (
            selected.code_object_version == loaded.authenticated.admission.code_object_version(),
            "code-object version",
        ),
        (
            selected.physical_kernel.export_symbol()
                == selected.artifact_identity.symbol().as_str(),
            "selected physical kernel",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

fn validate_selected_kernel_resolution(
    identity: &ArtifactKernelIdentityV1,
    physical: &PublishedKernelPhysicalLayoutV1,
    executable_object: HsaExecutableObjectIdentityV1,
    primary: &HsaKernelResolutionObservationV1,
    selected: &HsaKernelResolutionObservationV1,
) -> Result<(), &'static str> {
    let launch = physical.launch();
    let expected_hsa_alignment = launch
        .kernarg_segment_alignment()
        .max(HSA_MINIMUM_KERNARG_ALIGNMENT);
    for (matches, field) in [
        (
            selected.executable_object == executable_object,
            "HSA executable object",
        ),
        (
            selected.export_symbol.as_ref() == identity.symbol().as_str()
                && selected.export_symbol.as_ref() == physical.export_symbol(),
            "HSA kernel symbol",
        ),
        (
            selected.kernarg_segment_size == launch.kernarg_segment_size(),
            "kernarg segment size",
        ),
        (
            selected.kernarg_segment_alignment == expected_hsa_alignment,
            "kernarg segment alignment",
        ),
        (
            selected.export_symbol == primary.export_symbol
                || selected.kernel_object != primary.kernel_object,
            "HSA kernel object alias",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

/// Resolved selected kernel bound to one exact loaded object.
///
/// The private kernel handle and actual mutable executable borrow are retained
/// together. Only the unsafe compiler-generated SPI can consume this token;
/// no method exposes either native handle or reusable launch authority.
#[doc(hidden)]
pub struct ResolvedLoadedWorkerV2KernelSelectionV1<
    'loaded,
    P,
    K,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
> {
    artifact_identity: ArtifactKernelIdentityV1,
    physical_kernel: PublishedKernelPhysicalLayoutV1,
    prerequisites: WorkerV2PrerequisiteDecisionV1,
    environment: HsaEnvironmentObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    kernel: A::Kernel,
    loaded: &'loaded mut LoadedHsaExecutableV1<P, A>,
    _marker: PhantomData<fn() -> K>,
}

impl<P, K, A: ReviewedHsaExecutableLifecycleAdapterV1> fmt::Debug
    for ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedLoadedWorkerV2KernelSelectionV1")
            .field("artifact_identity", &self.artifact_identity)
            .field("resolution", &self.resolution)
            .finish_non_exhaustive()
    }
}

impl<P, K, A: ReviewedHsaExecutableLifecycleAdapterV1>
    ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>
{
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.artifact_identity
    }

    pub const fn physical_kernel(&self) -> &PublishedKernelPhysicalLayoutV1 {
        &self.physical_kernel
    }

    pub const fn prerequisites(&self) -> &WorkerV2PrerequisiteDecisionV1 {
        &self.prerequisites
    }

    pub const fn kernel_observation(&self) -> &HsaKernelResolutionObservationV1 {
        &self.resolution
    }

    pub const fn requires_prerequisite_authentication(&self) -> bool {
        false
    }

    pub const fn requires_hsa_kernel_resolution(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<P, K, A: ReviewedHsaImplicitKernargAdapterV1>
    ResolvedLoadedWorkerV2KernelSelectionV1<'_, P, K, A>
{
    /// Completes and synchronously dispatches one compiler-generated kernarg.
    ///
    /// This is an SPI for generated safe adapters, not an application launch
    /// API. It consumes the selected-kernel resolution and therefore cannot
    /// yield reusable dispatch or native-handle authority.
    ///
    /// # Safety
    ///
    /// Generated code must have initialized the complete explicit ABI in
    /// `kernarg` and must retain every referenced allocation until this method
    /// returns. Those allocations must satisfy the authenticated bounds,
    /// provenance, initialization, ownership, alias, and effect contracts for
    /// `K`. The supplied spans must describe the compiler-selected code-object
    /// ABI; callers cannot infer them from untrusted runtime metadata.
    #[doc(hidden)]
    pub unsafe fn dispatch_generated_and_wait(
        self,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
    ) -> Result<HsaCompletedSelectedWorkerV2DispatchV1<K>, HsaGeneratedDispatchError<A::Error>>
    {
        let Self {
            artifact_identity,
            physical_kernel,
            prerequisites,
            environment,
            resolution,
            kernel,
            loaded,
            _marker: _,
        } = self;

        // Keep the selected handle alive through the synchronous operation,
        // then release it before the mutable executable borrow can end on every
        // ordinary success or error path.
        let result = (|| {
            validate_selected_dispatch_state(
                &artifact_identity,
                &physical_kernel,
                &prerequisites,
                &environment,
                &resolution,
                loaded,
            )
            .map_err(HsaGeneratedDispatchError::SelectionMismatch)?;
            validate_launch_geometry_contract(
                artifact_identity.launch(),
                physical_kernel.launch(),
                geometry,
            )
            .map_err(HsaGeneratedDispatchError::LaunchAuthorization)?;

            let expected_size = usize::try_from(resolution.kernarg_segment_size)
                .map_err(|_| HsaGeneratedDispatchError::KernargSize)?;
            let expected_explicit = usize::try_from(artifact_identity.abi().size())
                .map_err(|_| HsaGeneratedDispatchError::KernargSize)?;
            if kernarg.len() != expected_size
                || explicit_byte_len != expected_explicit
                || explicit_byte_len != implicit_byte_offset
                || physical_kernel.launch().implicit_argument_offset()
                    != PhysicalMetadataValueV1::Known(
                        u64::try_from(implicit_byte_offset)
                            .map_err(|_| HsaGeneratedDispatchError::KernargSize)?,
                    )
                || physical_kernel.launch().implicit_argument_size()
                    != u64::try_from(implicit_byte_len)
                        .map_err(|_| HsaGeneratedDispatchError::KernargSize)?
                || implicit_byte_offset
                    .checked_add(implicit_byte_len)
                    .is_none_or(|end| end != expected_size)
            {
                return Err(HsaGeneratedDispatchError::KernargSize);
            }
            let alignment = usize::try_from(resolution.kernarg_segment_alignment)
                .map_err(|_| HsaGeneratedDispatchError::KernargAlignment)?;
            if !kernarg.as_ptr().addr().is_multiple_of(alignment) {
                return Err(HsaGeneratedDispatchError::KernargAlignment);
            }

            let explicit = kernarg[..explicit_byte_len].to_vec();
            let executable = loaded
                .executable
                .as_ref()
                .expect("resolved selection retains the loaded executable");
            // SAFETY: the consumed token binds these private handles to the
            // authenticated selected ABI. The caller owns the generated-memory
            // obligations stated above; the reviewed adapter owns initialization.
            let implicit = reviewed_adapter_call(|| unsafe {
                loaded.adapter.initialize_implicit_kernarg(
                    executable,
                    &kernel,
                    geometry,
                    explicit_byte_len,
                    implicit_byte_offset,
                    implicit_byte_len,
                    kernarg,
                )
            })
            .map_err(HsaGeneratedDispatchError::ImplicitAdapter)?;
            if kernarg[..explicit_byte_len] != *explicit {
                return Err(HsaGeneratedDispatchError::ExplicitKernargMutation);
            }
            validate_implicit_kernarg_observation(
                &loaded.load,
                &resolution,
                geometry,
                explicit_byte_len,
                implicit_byte_offset,
                implicit_byte_len,
                &implicit,
            )
            .map_err(HsaGeneratedDispatchError::ImplicitObservationMismatch)?;

            // SAFETY: all explicit and implicit bytes are now validated, and
            // the unsafe adapter returns only before submission or after every
            // effect is quiescent.
            let dispatch = reviewed_adapter_call(|| unsafe {
                loaded
                    .adapter
                    .launch_and_wait(executable, &kernel, geometry, kernarg)
            })
            .map_err(HsaGeneratedDispatchError::DispatchAdapter)?;
            validate_dispatch_observation(&loaded.load, &resolution, geometry, &dispatch)
                .map_err(HsaGeneratedDispatchError::DispatchObservationMismatch)?;

            Ok(HsaCompletedSelectedWorkerV2DispatchV1 {
                artifact_identity,
                completed: HsaCompletedDispatchV1 {
                    finalized_digest: loaded.load.finalized_digest,
                    executable_object: loaded.load.executable_object,
                    kernel_object: resolution.kernel_object,
                    geometry,
                    dispatch,
                },
                _marker: PhantomData,
            })
        })();
        drop(kernel);
        result
    }
}

fn validate_selected_dispatch_state<P, A: ReviewedHsaExecutableLifecycleAdapterV1>(
    identity: &ArtifactKernelIdentityV1,
    physical: &PublishedKernelPhysicalLayoutV1,
    prerequisites: &WorkerV2PrerequisiteDecisionV1,
    environment: &HsaEnvironmentObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    loaded: &LoadedHsaExecutableV1<P, A>,
) -> Result<(), &'static str> {
    for (matches, field) in [
        (
            environment == &loaded.environment,
            "selected HSA environment",
        ),
        (
            identity.payload_digest() == loaded.load.finalized_digest,
            "selected finalized digest",
        ),
        (
            prerequisites.finalized_digest == loaded.load.finalized_digest,
            "selected prerequisite digest",
        ),
        (
            prerequisites.kernel_id == identity.kernel_id(),
            "selected prerequisite kernel",
        ),
        (
            physical.export_symbol() == identity.symbol().as_str(),
            "selected physical kernel",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    validate_selected_kernel_resolution(
        identity,
        physical,
        loaded.load.executable_object,
        &loaded.resolution,
        resolution,
    )
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> Drop for LoadedHsaExecutableV1<K, A> {
    fn drop(&mut self) {
        self.kernel.take();
        let Some(executable) = self.executable.take() else {
            return;
        };
        // SAFETY: Drop runs only after all Rust borrows of this value end. The
        // unsafe adapter owns the remaining native lifetime obligations.
        terminal_unload(&mut self.adapter, executable, &self.environment, &self.load);
    }
}

/// Geometry-specific authority for one loaded HSA kernel.
///
/// The mutable borrow prevents concurrent authorization and unload. Raw
/// dispatch remains unsafe because generated code must bind concrete kernarg
/// pointers, allocation lifetimes, and alias admission to the authenticated
/// Rust effect contract.
pub struct HsaKernelLaunchAuthorizationV1<'loaded, K, A: ReviewedHsaExecutableLifecycleAdapterV1> {
    loaded: &'loaded mut LoadedHsaExecutableV1<K, A>,
    geometry: HsaLaunchGeometryV1,
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> HsaKernelLaunchAuthorizationV1<'_, K, A> {
    pub const fn grants_launch_authority(&self) -> bool {
        true
    }

    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.geometry
    }

    pub const fn kernarg_segment_size(&self) -> u64 {
        self.loaded.resolution.kernarg_segment_size
    }

    pub const fn kernarg_segment_alignment(&self) -> u64 {
        self.loaded.resolution.kernarg_segment_alignment
    }

    /// Dispatches exact kernarg storage and synchronously waits for quiescence.
    ///
    /// # Safety
    ///
    /// The initialized bytes must implement the complete authenticated Rust ABI
    /// for `K`. Every pointer and reachable allocation must satisfy the proved
    /// bounds, provenance, ownership, alias, and effect contract for the whole
    /// call. Only generated typed launch code should invoke this boundary.
    pub unsafe fn launch_and_wait(
        self,
        kernarg: &mut [u8],
    ) -> Result<HsaCompletedDispatchV1, HsaDispatchError<A::Error>> {
        let expected_size = usize::try_from(self.loaded.resolution.kernarg_segment_size)
            .map_err(|_| HsaDispatchError::KernargSize)?;
        if kernarg.len() != expected_size {
            return Err(HsaDispatchError::KernargSize);
        }
        let alignment = usize::try_from(self.loaded.resolution.kernarg_segment_alignment)
            .map_err(|_| HsaDispatchError::KernargAlignment)?;
        if !kernarg.as_ptr().addr().is_multiple_of(alignment) {
            return Err(HsaDispatchError::KernargAlignment);
        }
        let executable = self
            .loaded
            .executable
            .as_ref()
            .expect("launch authority retains the loaded executable");
        let kernel = self
            .loaded
            .kernel
            .as_ref()
            .expect("launch authority retains the resolved kernel");
        // SAFETY: the caller owns concrete ABI and resource obligations. The
        // reviewed adapter owns exact-handle dispatch and wait semantics.
        let dispatch = reviewed_adapter_call(|| unsafe {
            self.loaded
                .adapter
                .launch_and_wait(executable, kernel, self.geometry, kernarg)
        })
        .map_err(HsaDispatchError::Adapter)?;
        validate_dispatch_observation(
            &self.loaded.load,
            &self.loaded.resolution,
            self.geometry,
            &dispatch,
        )
        .map_err(HsaDispatchError::ObservationMismatch)?;
        Ok(HsaCompletedDispatchV1 {
            finalized_digest: self.loaded.load.finalized_digest,
            executable_object: self.loaded.load.executable_object,
            kernel_object: self.loaded.resolution.kernel_object,
            geometry: self.geometry,
            dispatch,
        })
    }
}

impl<K, A: ReviewedHsaImplicitKernargAdapterV1> HsaKernelLaunchAuthorizationV1<'_, K, A> {
    pub(crate) fn launch_generated_with_implicit_kernarg(
        self,
        explicit: &[u8],
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaCompletedDispatchV1, HsaGeneratedDispatchError<A::Error>> {
        let expected_size = usize::try_from(self.loaded.resolution.kernarg_segment_size)
            .map_err(|_| HsaGeneratedDispatchError::KernargSize)?;
        if kernarg.len() != expected_size
            || explicit.len() != implicit_byte_offset
            || implicit_byte_offset
                .checked_add(implicit_byte_len)
                .is_none_or(|end| end != expected_size)
        {
            return Err(HsaGeneratedDispatchError::KernargSize);
        }
        let alignment = usize::try_from(self.loaded.resolution.kernarg_segment_alignment)
            .map_err(|_| HsaGeneratedDispatchError::KernargAlignment)?;
        if !kernarg.as_ptr().addr().is_multiple_of(alignment) {
            return Err(HsaGeneratedDispatchError::KernargAlignment);
        }

        kernarg[..explicit.len()].copy_from_slice(explicit);
        let executable = self
            .loaded
            .executable
            .as_ref()
            .expect("launch authority retains the loaded executable");
        let kernel = self
            .loaded
            .kernel
            .as_ref()
            .expect("launch authority retains the resolved kernel");
        // SAFETY: this crate-private transition is reachable only after typed
        // generated code sealed its exact explicit ABI and resource witnesses.
        // The unsafe extension owns the hidden-argument initialization contract.
        let observation = reviewed_adapter_call(|| unsafe {
            self.loaded.adapter.initialize_implicit_kernarg(
                executable,
                kernel,
                self.geometry,
                explicit.len(),
                implicit_byte_offset,
                implicit_byte_len,
                kernarg,
            )
        })
        .map_err(HsaGeneratedDispatchError::ImplicitAdapter)?;
        if kernarg[..explicit.len()] != *explicit {
            return Err(HsaGeneratedDispatchError::ExplicitKernargMutation);
        }
        validate_implicit_kernarg_observation(
            &self.loaded.load,
            &self.loaded.resolution,
            self.geometry,
            explicit.len(),
            implicit_byte_offset,
            implicit_byte_len,
            &observation,
        )
        .map_err(HsaGeneratedDispatchError::ImplicitObservationMismatch)?;

        // SAFETY: the generated path supplied the complete explicit ABI and
        // the reviewed extension initialized the exact implicit span. By the
        // unsafe adapter contract, either result returns only before submission
        // or after quiescence, so caller-owned allocations may be released.
        match unsafe { self.launch_and_wait(kernarg) } {
            Ok(completed) => Ok(completed),
            Err(HsaDispatchError::KernargSize) => Err(HsaGeneratedDispatchError::KernargSize),
            Err(HsaDispatchError::KernargAlignment) => {
                Err(HsaGeneratedDispatchError::KernargAlignment)
            }
            Err(HsaDispatchError::Adapter(error)) => {
                Err(HsaGeneratedDispatchError::DispatchAdapter(error))
            }
            Err(HsaDispatchError::ObservationMismatch(field)) => Err(
                HsaGeneratedDispatchError::DispatchObservationMismatch(field),
            ),
        }
    }
}

/// Failure while completing a generated typed kernarg and dispatching it.
#[derive(Debug)]
#[non_exhaustive]
pub enum HsaGeneratedDispatchError<E> {
    KernargSize,
    KernargAlignment,
    LaunchAuthorization(HsaLaunchAuthorizationError),
    SelectionMismatch(&'static str),
    ImplicitAdapter(E),
    DispatchAdapter(E),
    ExplicitKernargMutation,
    ImplicitObservationMismatch(&'static str),
    DispatchObservationMismatch(&'static str),
}

impl<E: fmt::Display> fmt::Display for HsaGeneratedDispatchError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KernargSize => formatter.write_str("generated kernarg size is not exact"),
            Self::KernargAlignment => {
                formatter.write_str("generated kernarg storage is not sufficiently aligned")
            }
            Self::LaunchAuthorization(error) => {
                write!(
                    formatter,
                    "generated launch geometry was rejected: {error:?}"
                )
            }
            Self::SelectionMismatch(field) => {
                write!(formatter, "selected HSA kernel mismatched {field}")
            }
            Self::ImplicitAdapter(error) => {
                write!(formatter, "implicit-kernarg adapter failed: {error}")
            }
            Self::DispatchAdapter(error) => write!(formatter, "HSA dispatch failed: {error}"),
            Self::ExplicitKernargMutation => {
                formatter.write_str("implicit-kernarg adapter mutated explicit bytes")
            }
            Self::ImplicitObservationMismatch(field) => {
                write!(formatter, "implicit-kernarg observation mismatched {field}")
            }
            Self::DispatchObservationMismatch(field) => {
                write!(formatter, "HSA dispatch observation mismatched {field}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for HsaGeneratedDispatchError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ImplicitAdapter(error) | Self::DispatchAdapter(error) => Some(error),
            _ => None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_implicit_kernarg_observation(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: usize,
    implicit_byte_offset: usize,
    implicit_byte_len: usize,
    observation: &HsaImplicitKernargInitializationObservationV1,
) -> Result<(), &'static str> {
    let explicit_byte_len =
        u64::try_from(explicit_byte_len).map_err(|_| "explicit kernarg length")?;
    let implicit_byte_offset =
        u64::try_from(implicit_byte_offset).map_err(|_| "implicit kernarg offset")?;
    let implicit_byte_len =
        u64::try_from(implicit_byte_len).map_err(|_| "implicit kernarg length")?;
    for (matches, field) in [
        (
            observation.executable_object == load.executable_object,
            "implicit kernarg executable object",
        ),
        (
            observation.kernel_object == resolution.kernel_object,
            "implicit kernarg kernel object",
        ),
        (
            observation.geometry == geometry,
            "implicit kernarg launch geometry",
        ),
        (
            observation.explicit_byte_len == explicit_byte_len,
            "explicit kernarg length",
        ),
        (
            observation.implicit_byte_offset == implicit_byte_offset,
            "implicit kernarg offset",
        ),
        (
            observation.implicit_byte_len == implicit_byte_len,
            "implicit kernarg length",
        ),
        (
            observation.initialized,
            "implicit kernarg initialization completion",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

/// Failure while validating launch geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HsaLaunchAuthorizationError {
    ZeroDimension,
    DimensionOverflow,
    RankMismatch,
    GridExceedsContract,
    WorkgroupMismatch,
    WorkgroupExceedsContract,
    WorkgroupExceedsPhysicalLimit,
    PhysicalWorkgroupRequirementUnknown,
    DynamicSharedMemoryExceedsContract,
    DynamicSharedMemoryNotRepresented,
}

fn validate_launch_geometry(
    admission: &AdmittedFinalizedWorkerV2BundleV1,
    geometry: HsaLaunchGeometryV1,
) -> Result<(), HsaLaunchAuthorizationError> {
    validate_launch_geometry_contract(
        admission.artifact_identity().launch(),
        admission.selected_kernel().launch(),
        geometry,
    )
}

fn validate_launch_geometry_contract(
    source: &LaunchContract,
    physical: &PublishedPhysicalLaunchLayoutV1,
    geometry: HsaLaunchGeometryV1,
) -> Result<(), HsaLaunchAuthorizationError> {
    if geometry.grid.contains(&0) || geometry.workgroup.contains(&0) {
        return Err(HsaLaunchAuthorizationError::ZeroDimension);
    }
    let workgroup_product = product(geometry.workgroup)?;
    product(geometry.grid)?;
    if (source.rank() < 2 && (geometry.grid[1] != 1 || geometry.workgroup[1] != 1))
        || (source.rank() < 3 && (geometry.grid[2] != 1 || geometry.workgroup[2] != 1))
    {
        return Err(HsaLaunchAuthorizationError::RankMismatch);
    }
    let max_grid = source.max_grid();
    if geometry.grid[0] > max_grid.x()
        || geometry.grid[1] > max_grid.y()
        || geometry.grid[2] > max_grid.z()
    {
        return Err(HsaLaunchAuthorizationError::GridExceedsContract);
    }
    match source.block_size() {
        BlockSize::Any => {}
        BlockSize::Exact(block) if geometry.workgroup == [block.x(), block.y(), block.z()] => {}
        BlockSize::Exact(_) => return Err(HsaLaunchAuthorizationError::WorkgroupMismatch),
        BlockSize::AtMost(block)
            if geometry.workgroup[0] <= block.x()
                && geometry.workgroup[1] <= block.y()
                && geometry.workgroup[2] <= block.z() => {}
        BlockSize::AtMost(_) => {
            return Err(HsaLaunchAuthorizationError::WorkgroupExceedsContract);
        }
    }
    if workgroup_product > u64::from(physical.max_flat_workgroup_size()) {
        return Err(HsaLaunchAuthorizationError::WorkgroupExceedsPhysicalLimit);
    }
    match physical.required_workgroup_size() {
        PhysicalMetadataValueV1::Known(required) if required == geometry.workgroup => {}
        PhysicalMetadataValueV1::Known(_) => {
            return Err(HsaLaunchAuthorizationError::WorkgroupMismatch);
        }
        PhysicalMetadataValueV1::Unknown => {
            return Err(HsaLaunchAuthorizationError::PhysicalWorkgroupRequirementUnknown);
        }
    }
    if geometry.dynamic_shared_memory_bytes > source.max_dynamic_shared_memory_bytes() {
        return Err(HsaLaunchAuthorizationError::DynamicSharedMemoryExceedsContract);
    }
    if geometry.dynamic_shared_memory_bytes != 0
        && physical.dynamic_shared_memory_indicator() != PhysicalMetadataValueV1::Known(true)
    {
        return Err(HsaLaunchAuthorizationError::DynamicSharedMemoryNotRepresented);
    }
    Ok(())
}

fn product(dimensions: [u32; 3]) -> Result<u64, HsaLaunchAuthorizationError> {
    u64::from(dimensions[0])
        .checked_mul(u64::from(dimensions[1]))
        .and_then(|xy| xy.checked_mul(u64::from(dimensions[2])))
        .ok_or(HsaLaunchAuthorizationError::DimensionOverflow)
}

/// Failure while dispatching through a reviewed HSA adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum HsaDispatchError<E> {
    KernargSize,
    KernargAlignment,
    Adapter(E),
    ObservationMismatch(&'static str),
}

fn validate_dispatch_observation(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    dispatch: &HsaDispatchObservationV1,
) -> Result<(), &'static str> {
    for (matches, field) in [
        (
            dispatch.executable_object == load.executable_object,
            "dispatch executable object",
        ),
        (
            dispatch.kernel_object == resolution.kernel_object,
            "dispatch kernel object",
        ),
        (dispatch.geometry == geometry, "dispatch geometry"),
        (dispatch.completed, "dispatch completion"),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

/// Completed synchronous dispatch bound to exact executable and kernel objects.
#[derive(Debug)]
pub struct HsaCompletedDispatchV1 {
    finalized_digest: PayloadDigest,
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    geometry: HsaLaunchGeometryV1,
    dispatch: HsaDispatchObservationV1,
}

/// Quiescent completion evidence for one exact typed selected kernel.
///
/// This value is descriptive. It contains no executable, kernel, queue, or
/// allocation authority and cannot be used to dispatch again.
#[derive(Debug)]
pub struct HsaCompletedSelectedWorkerV2DispatchV1<K> {
    artifact_identity: ArtifactKernelIdentityV1,
    completed: HsaCompletedDispatchV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> HsaCompletedSelectedWorkerV2DispatchV1<K> {
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        &self.artifact_identity
    }

    pub const fn completed_dispatch(&self) -> &HsaCompletedDispatchV1 {
        &self.completed
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl HsaCompletedDispatchV1 {
    pub const fn finalized_digest(&self) -> PayloadDigest {
        self.finalized_digest
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.kernel_object
    }

    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.geometry
    }

    pub const fn dispatch(&self) -> &HsaDispatchObservationV1 {
        &self.dispatch
    }
}

/// Failure while explicitly unloading an HSA executable.
#[derive(Debug)]
#[non_exhaustive]
pub enum HsaExecutableUnloadError<E> {
    Adapter(E),
    ObservationMismatch(&'static str),
}

fn reviewed_adapter_call<T>(call: impl FnOnce() -> T) -> T {
    // `AssertUnwindSafe` is sound here because no unwind is ever resumed. The
    // process terminates while the outer lifecycle and caller allocations are
    // still live, so their destructors cannot release GPU-reachable authority.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
        Ok(value) => value,
        Err(payload) => {
            std::mem::forget(payload);
            std::process::abort()
        }
    }
}

fn terminal_unload<A: ReviewedHsaExecutableLifecycleAdapterV1>(
    adapter: &mut A,
    executable: A::Executable,
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
) -> HsaUnloadObservationV1 {
    let unload = match reviewed_adapter_call(|| unsafe { adapter.unload_executable(executable) }) {
        Ok(unload) => unload,
        Err(error) => {
            std::mem::forget(error);
            std::process::abort()
        }
    };
    if validate_unload_observation(environment, load, &unload).is_err() {
        std::process::abort();
    }
    unload
}

fn validate_unload_observation(
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
    unload: &HsaUnloadObservationV1,
) -> Result<(), &'static str> {
    for (matches, field) in [
        (
            unload.executable_object == load.executable_object,
            "unloaded executable object",
        ),
        (
            unload.runtime_instance == environment.runtime.instance,
            "unload runtime instance",
        ),
        (
            unload.agent_handle == environment.agent.agent_handle,
            "unload HSA agent",
        ),
        (unload.released, "unload completion"),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

/// Terminal evidence that one exact HSA executable was released.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnloadedHsaExecutableV1 {
    finalized_digest: PayloadDigest,
    executable_object: HsaExecutableObjectIdentityV1,
    runtime: HsaRuntimeIdentityV1,
    physical_device: HsaPhysicalDeviceIdentityV1,
    agent: HsaAgentIdentityV1,
    unload: HsaUnloadObservationV1,
}

impl UnloadedHsaExecutableV1 {
    pub const fn finalized_digest(&self) -> PayloadDigest {
        self.finalized_digest
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn runtime(&self) -> &HsaRuntimeIdentityV1 {
        &self.runtime
    }

    pub const fn physical_device(&self) -> &HsaPhysicalDeviceIdentityV1 {
        &self.physical_device
    }

    pub const fn agent(&self) -> &HsaAgentIdentityV1 {
        &self.agent
    }

    pub const fn unload_observation(&self) -> &HsaUnloadObservationV1 {
        &self.unload
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_identity_text(value: &str, field: &'static str) -> Result<(), HsaObservationError> {
    if value.is_empty() {
        return Err(HsaObservationError::EmptyText(field));
    }
    if value.len() > MAX_HSA_IDENTITY_TEXT_BYTES {
        return Err(HsaObservationError::TextTooLong(field));
    }
    if value.trim() != value || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(HsaObservationError::InvalidText(field));
    }
    Ok(())
}

fn validate_nonzero_digest(
    digest: PayloadDigest,
    field: &'static str,
) -> Result<(), HsaObservationError> {
    if digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(HsaObservationError::UnsupportedDigest(field));
    }
    validate_nonzero_bytes(digest.bytes().as_bytes(), field)
}

fn validate_nonzero_bytes(bytes: &[u8], field: &'static str) -> Result<(), HsaObservationError> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(HsaObservationError::ZeroIdentity(field))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_v2_bundle_admission::tests::{
        TestDirectory, admitted_for_lifecycle_test, admitted_two_kernel_for_lifecycle_test,
    };
    use crate::{CompilerGeneratedKernelProfileV1, ObservedContext};
    use fe2o3_device::KernelMarkerV1;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_kernel() {}

    static TEST_REGISTRATION: (u16, &str, &str, fn()) =
        (1, "logical_primary", "primary_kernel", test_kernel);

    struct TestKernel;

    unsafe impl KernelMarkerV1 for TestKernel {
        type Function = fn();
        type Registration = (u16, &'static str, &'static str, fn());

        const LOGICAL_NAME: &'static str = "logical_primary";
        const EXPORT_NAME: &'static str = "primary_kernel";
        const FUNCTION: Self::Function = test_kernel;
        const REGISTRATION: &'static Self::Registration = &TEST_REGISTRATION;
    }

    unsafe impl CompilerGeneratedKernelContractV1 for TestKernel {
        const PROFILE: CompilerGeneratedKernelProfileV1 =
            CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2;
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x4b; 32];

        fn artifact_container_bytes() -> &'static [u8] {
            &[]
        }
    }

    struct SecondTestKernel;

    unsafe impl KernelMarkerV1 for SecondTestKernel {
        type Function = fn();
        type Registration = (u16, &'static str, &'static str, fn());

        const LOGICAL_NAME: &'static str = "logical_second";
        const EXPORT_NAME: &'static str = "second_kernel";
        const FUNCTION: Self::Function = test_kernel;
        const REGISTRATION: &'static Self::Registration =
            &(1, "logical_second", "second_kernel", test_kernel);
    }

    unsafe impl CompilerGeneratedKernelContractV1 for SecondTestKernel {
        const PROFILE: CompilerGeneratedKernelProfileV1 =
            CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2;
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x5b; 32];

        fn artifact_container_bytes() -> &'static [u8] {
            &[]
        }
    }

    fn digest(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([seed; 32]))
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PrerequisiteFault {
        None,
        FinalizedDigest,
        Kernel,
        Marker,
        Compiler,
        TypeLayout,
        Effects,
        MissingRaceFreedom,
    }

    struct FakeAuthenticator {
        fault: PrerequisiteFault,
    }

    impl FakeAuthenticator {
        fn exact() -> Self {
            Self {
                fault: PrerequisiteFault::None,
            }
        }
    }

    unsafe impl<K: CompilerGeneratedKernelContractV1> WorkerV2PrerequisiteAuthenticatorV1<K>
        for FakeAuthenticator
    {
        type Error = &'static str;

        unsafe fn authenticate(
            &mut self,
            request: &WorkerV2PrerequisiteRequestV1<'_, K>,
        ) -> Result<WorkerV2PrerequisiteDecisionV1, Self::Error> {
            let artifact = request.artifact_identity();
            let finalized_digest = if self.fault == PrerequisiteFault::FinalizedDigest {
                digest(0xf1)
            } else {
                request.finalized_digest()
            };
            let kernel_id = if self.fault == PrerequisiteFault::Kernel {
                KernelId::from_bytes([0xf2; 32])
            } else {
                artifact.kernel_id()
            };
            let marker_binding_identity = if self.fault == PrerequisiteFault::Marker {
                [0xf3; 32]
            } else {
                K::KERNEL_BINDING_ID_V1
            };
            let compiler = if self.fault == PrerequisiteFault::Compiler {
                digest(0)
            } else {
                digest(1)
            };
            let type_layout = if self.fault == PrerequisiteFault::TypeLayout {
                digest(0)
            } else {
                digest(4)
            };
            let effects = if self.fault == PrerequisiteFault::Effects {
                digest(0)
            } else {
                digest(5)
            };
            let properties = if self.fault == PrerequisiteFault::MissingRaceFreedom {
                WorkerV2SafetyPropertiesV1::new(
                    WorkerV2SafetyPropertiesV1::required().bits()
                        & !WorkerV2SafetyPropertyV1::RaceFreedom.bit(),
                )
                .unwrap()
            } else {
                WorkerV2SafetyPropertiesV1::required()
            };
            Ok(WorkerV2PrerequisiteDecisionV1::new(
                finalized_digest,
                kernel_id,
                artifact.executable_digest(),
                request.target(),
                request.code_object_version(),
                artifact.name().as_str(),
                artifact.symbol().as_str(),
                artifact.abi().clone(),
                artifact.launch().clone(),
                marker_binding_identity,
                compiler,
                digest(2),
                digest(3),
                type_layout,
                effects,
                properties,
            ))
        }
    }

    fn authenticate(seed: u8) -> (AuthenticatedWorkerV2ExecutableV1<TestKernel>, TestDirectory) {
        let (admission, directory) = admitted_for_lifecycle_test(seed);
        let authenticated = AuthenticatedWorkerV2ExecutableV1::authenticate(
            admission,
            &mut FakeAuthenticator::exact(),
        )
        .unwrap();
        (authenticated, directory)
    }

    fn authenticate_two_kernels(
        seed: u8,
    ) -> (AuthenticatedWorkerV2ExecutableV1<TestKernel>, TestDirectory) {
        let (admission, directory) = admitted_two_kernel_for_lifecycle_test(seed);
        let authenticated = AuthenticatedWorkerV2ExecutableV1::authenticate(
            admission,
            &mut FakeAuthenticator::exact(),
        )
        .unwrap();
        (authenticated, directory)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum AdapterFault {
        None,
        DeviceOrdinal,
        TargetSramEccDisabled,
        TargetXnackEnabled,
        TargetXnackOmitted,
        TargetProcessor,
        RuntimeInstance,
        LoadDigest,
        LoadLength,
        ResolutionExecutable,
        Symbol,
        KernelObjectAlias,
        KernargSize,
        KernargAlignment,
        DispatchObject,
        DispatchKernel,
        DispatchGeometry,
        DispatchIncomplete,
        DispatchAdapterError,
        DispatchPanic,
        ImplicitExecutable,
        ImplicitKernel,
        ImplicitGeometry,
        ImplicitOffset,
        ImplicitIncomplete,
        ExplicitMutation,
        UnloadObject,
        UnloadIncomplete,
        UnloadAdapterError,
        UnloadPanic,
    }

    #[derive(Debug)]
    struct FakeExecutable;

    #[derive(Debug)]
    struct FakeKernel {
        object: HsaKernelObjectIdentityV1,
    }

    type TestLoadedResult = Result<
        LoadedHsaExecutableV1<TestKernel, FakeHsaAdapter>,
        HsaExecutableLoadError<&'static str>,
    >;

    struct FakeHsaAdapter {
        fault: AdapterFault,
        unloads: Arc<AtomicUsize>,
        implicit_initialized: bool,
    }

    impl FakeHsaAdapter {
        fn new(fault: AdapterFault) -> (Self, Arc<AtomicUsize>) {
            let unloads = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    fault,
                    unloads: unloads.clone(),
                    implicit_initialized: false,
                },
                unloads,
            )
        }

        fn environment(&self) -> HsaEnvironmentObservationV1 {
            let target = AmdTargetId::parse(match self.fault {
                AdapterFault::TargetSramEccDisabled => "gfx942:sramecc-:xnack-",
                AdapterFault::TargetXnackEnabled => "gfx942:sramecc+:xnack+",
                AdapterFault::TargetXnackOmitted => "gfx942:sramecc+",
                AdapterFault::TargetProcessor => "gfx950:sramecc+:xnack-",
                _ => "gfx942:sramecc+:xnack-",
            })
            .unwrap();
            let runtime_instance = if self.fault == AdapterFault::RuntimeInstance {
                [0x7a; 16]
            } else {
                [0x72; 16]
            };
            let runtime =
                HsaRuntimeIdentityV1::new("ROCr", "test-v1", digest(0x71), runtime_instance)
                    .unwrap();
            let ordinal = if self.fault == AdapterFault::DeviceOrdinal {
                1
            } else {
                0
            };
            let device = HsaPhysicalDeviceIdentityV1::new([0x73; 16], 7, ordinal, target).unwrap();
            let agent =
                HsaAgentIdentityV1::new(runtime.instance(), 0x7474, device.uuid(), target).unwrap();
            HsaEnvironmentObservationV1::new(runtime, device, agent).unwrap()
        }

        fn executable_object() -> HsaExecutableObjectIdentityV1 {
            HsaExecutableObjectIdentityV1::new([0x75; 32]).unwrap()
        }

        fn kernel_object() -> HsaKernelObjectIdentityV1 {
            HsaKernelObjectIdentityV1::new([0x76; 32]).unwrap()
        }

        fn second_kernel_object() -> HsaKernelObjectIdentityV1 {
            HsaKernelObjectIdentityV1::new([0x78; 32]).unwrap()
        }
    }

    unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for FakeHsaAdapter {
        type Executable = FakeExecutable;
        type Kernel = FakeKernel;
        type Error = &'static str;

        unsafe fn observe_environment(
            &mut self,
        ) -> Result<HsaEnvironmentObservationV1, Self::Error> {
            Ok(self.environment())
        }

        unsafe fn load_executable(
            &mut self,
            bytes: &[u8],
            finalized_digest: PayloadDigest,
        ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
            let environment = self.environment();
            let digest = if self.fault == AdapterFault::LoadDigest {
                digest(0x77)
            } else {
                finalized_digest
            };
            let mut byte_len = u64::try_from(bytes.len()).unwrap();
            if self.fault == AdapterFault::LoadLength {
                byte_len += 1;
            }
            Ok((
                FakeExecutable,
                HsaCodeObjectLoadObservationV1::new(
                    digest,
                    byte_len,
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
        ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
            let symbol = if self.fault == AdapterFault::Symbol {
                "substituted_kernel"
            } else {
                export_symbol
            };
            let size = if self.fault == AdapterFault::KernargSize {
                312
            } else {
                304
            };
            let alignment = if self.fault == AdapterFault::KernargAlignment {
                8
            } else {
                16
            };
            let executable_object = if self.fault == AdapterFault::ResolutionExecutable {
                HsaExecutableObjectIdentityV1::new([0x79; 32]).unwrap()
            } else {
                Self::executable_object()
            };
            let kernel_object = if export_symbol == "second_kernel"
                && self.fault != AdapterFault::KernelObjectAlias
            {
                Self::second_kernel_object()
            } else {
                Self::kernel_object()
            };
            Ok((
                FakeKernel {
                    object: kernel_object,
                },
                HsaKernelResolutionObservationV1::new(
                    executable_object,
                    kernel_object,
                    symbol,
                    size,
                    alignment,
                )
                .unwrap(),
            ))
        }

        unsafe fn launch_and_wait(
            &mut self,
            _executable: &Self::Executable,
            kernel: &Self::Kernel,
            geometry: HsaLaunchGeometryV1,
            kernarg: &mut [u8],
        ) -> Result<HsaDispatchObservationV1, Self::Error> {
            if self.fault == AdapterFault::DispatchPanic {
                panic!("malicious adapter panic after simulated packet publication");
            }
            if self.fault == AdapterFault::DispatchAdapterError {
                return Err("definite pre-submit dispatch failure");
            }
            if self.implicit_initialized
                && (kernarg[..48] != [0x5a; 48] || kernarg[48..] != [0xa5; 256])
            {
                return Err("generated kernarg was not preserved");
            }
            let executable = if self.fault == AdapterFault::DispatchObject {
                HsaExecutableObjectIdentityV1::new([0x81; 32]).unwrap()
            } else {
                Self::executable_object()
            };
            let geometry = if self.fault == AdapterFault::DispatchGeometry {
                HsaLaunchGeometryV1::new([2, 1, 1], [256, 1, 1], 0)
            } else {
                geometry
            };
            let kernel_object = if self.fault == AdapterFault::DispatchKernel {
                Self::kernel_object()
            } else {
                kernel.object
            };
            HsaDispatchObservationV1::new(
                [0x82; 16],
                executable,
                kernel_object,
                geometry,
                self.fault != AdapterFault::DispatchIncomplete,
            )
            .map_err(|_| "invalid dispatch observation")
        }

        unsafe fn unload_executable(
            &mut self,
            _executable: Self::Executable,
        ) -> Result<HsaUnloadObservationV1, Self::Error> {
            self.unloads.fetch_add(1, Ordering::SeqCst);
            if self.fault == AdapterFault::UnloadPanic {
                panic!("malicious adapter panic during executable unload");
            }
            if self.fault == AdapterFault::UnloadAdapterError {
                return Err("ambiguous executable unload");
            }
            let executable = if self.fault == AdapterFault::UnloadObject {
                HsaExecutableObjectIdentityV1::new([0x83; 32]).unwrap()
            } else {
                Self::executable_object()
            };
            let environment = self.environment();
            Ok(HsaUnloadObservationV1::new(
                executable,
                environment.runtime().instance(),
                environment.agent().agent_handle(),
                self.fault != AdapterFault::UnloadIncomplete,
            ))
        }
    }

    unsafe impl ReviewedHsaImplicitKernargAdapterV1 for FakeHsaAdapter {
        unsafe fn initialize_implicit_kernarg(
            &mut self,
            _executable: &Self::Executable,
            kernel: &Self::Kernel,
            geometry: HsaLaunchGeometryV1,
            explicit_byte_len: usize,
            implicit_byte_offset: usize,
            implicit_byte_len: usize,
            kernarg: &mut [u8],
        ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
            if self.fault == AdapterFault::ExplicitMutation {
                kernarg[0] ^= 0xff;
            }
            kernarg[implicit_byte_offset..implicit_byte_offset + implicit_byte_len].fill(0xa5);
            self.implicit_initialized = true;
            let executable = if self.fault == AdapterFault::ImplicitExecutable {
                HsaExecutableObjectIdentityV1::new([0x91; 32]).unwrap()
            } else {
                Self::executable_object()
            };
            let reported_offset = if self.fault == AdapterFault::ImplicitOffset {
                implicit_byte_offset + 8
            } else {
                implicit_byte_offset
            };
            let reported_geometry = if self.fault == AdapterFault::ImplicitGeometry {
                HsaLaunchGeometryV1::new([2, 1, 1], [256, 1, 1], 0)
            } else {
                geometry
            };
            let kernel_object = if self.fault == AdapterFault::ImplicitKernel {
                Self::kernel_object()
            } else {
                kernel.object
            };
            Ok(HsaImplicitKernargInitializationObservationV1::new(
                executable,
                kernel_object,
                reported_geometry,
                u64::try_from(explicit_byte_len).unwrap(),
                u64::try_from(reported_offset).unwrap(),
                u64::try_from(implicit_byte_len).unwrap(),
                self.fault != AdapterFault::ImplicitIncomplete,
            ))
        }
    }

    fn load(seed: u8, fault: AdapterFault) -> (TestLoadedResult, Arc<AtomicUsize>, TestDirectory) {
        let (authenticated, directory) = authenticate(seed);
        let (adapter, unloads) = FakeHsaAdapter::new(fault);
        let authorized = authenticated.authorize_hsa_load(adapter).unwrap();
        assert!(authorized.grants_load_authority());
        assert!(!authorized.grants_launch_authority());
        (authorized.load(), unloads, directory)
    }

    fn load_two_kernels(seed: u8) -> (TestLoadedResult, Arc<AtomicUsize>, TestDirectory) {
        load_two_kernels_with_fault(seed, AdapterFault::None)
    }

    fn load_two_kernels_with_fault(
        seed: u8,
        fault: AdapterFault,
    ) -> (TestLoadedResult, Arc<AtomicUsize>, TestDirectory) {
        let (authenticated, directory) = authenticate_two_kernels(seed);
        let (adapter, unloads) = FakeHsaAdapter::new(fault);
        let authorized = authenticated.authorize_hsa_load(adapter).unwrap();
        (authorized.load(), unloads, directory)
    }

    #[repr(align(16))]
    struct AlignedKernarg([u8; 304]);

    #[repr(align(16))]
    struct OffsetKernarg([u8; 305]);

    #[test]
    fn complete_lifecycle_binds_all_identities_and_unloads() {
        let (loaded, unloads, _directory) = load(0x91, AdapterFault::None);
        let mut loaded = loaded.unwrap();
        assert_eq!(loaded.load_observation().byte_len(), 3256);
        assert_eq!(
            loaded.kernel_observation().export_symbol(),
            "primary_kernel"
        );
        assert_eq!(loaded.kernel_observation().kernarg_segment_size(), 304);
        assert_eq!(loaded.kernel_observation().kernarg_segment_alignment(), 16);

        let geometry = HsaLaunchGeometryV1::new([32, 1, 1], [256, 1, 1], 0);
        let launch = loaded.authorize_launch(geometry).unwrap();
        assert!(launch.grants_launch_authority());
        let mut kernarg = AlignedKernarg([0; 304]);
        let completed = unsafe { launch.launch_and_wait(&mut kernarg.0) }.unwrap();
        assert_eq!(completed.geometry(), geometry);
        assert!(completed.dispatch().completed());

        let unloaded = loaded.unload().unwrap();
        assert!(unloaded.unload_observation().released());
        assert!(!unloaded.grants_load_authority());
        assert!(!unloaded.grants_launch_authority());
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn loaded_executable_issues_only_inert_exact_typed_selection() {
        let (loaded, unloads, _directory) = load(0x8f, AdapterFault::None);
        let loaded = loaded.unwrap();
        {
            let selection = loaded.select_typed_kernel::<TestKernel>().unwrap();

            assert_eq!(
                selection.artifact_identity().kernel_id(),
                loaded.artifact_identity().kernel_id()
            );
            assert_eq!(
                selection.executable_object(),
                loaded.load_observation().executable_object()
            );
            assert!(selection.requires_prerequisite_authentication());
            assert!(selection.requires_hsa_kernel_resolution());
            assert!(!selection.grants_load_authority());
            assert!(!selection.grants_launch_authority());
        }
        loaded.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn one_exact_loaded_executable_selects_two_distinct_typed_markers() {
        let (loaded, unloads, _directory) = load_two_kernels(0x8e);
        let loaded = loaded.unwrap();
        {
            let first = loaded.select_typed_kernel::<TestKernel>().unwrap();
            let second = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();

            assert_ne!(
                first.artifact_identity().kernel_id(),
                second.artifact_identity().kernel_id()
            );
            assert_eq!(
                first.artifact_identity().symbol().as_str(),
                "primary_kernel"
            );
            assert_eq!(
                second.artifact_identity().symbol().as_str(),
                "second_kernel"
            );
            assert_eq!(first.executable_object(), second.executable_object());
            assert_eq!(
                first.executable_object(),
                loaded.load_observation().executable_object()
            );
            assert_eq!(first.physical_kernel().export_symbol(), "primary_kernel");
            assert_eq!(second.physical_kernel().export_symbol(), "second_kernel");
            assert!(!second.grants_launch_authority());
        }
        loaded.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn selected_marker_authenticates_and_resolves_without_launch_authority() {
        let (loaded, unloads, _directory) = load_two_kernels(0x8d);
        let mut loaded = loaded.unwrap();
        let selection = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();
        let authenticated = selection
            .authenticate(&mut FakeAuthenticator::exact())
            .unwrap();
        assert!(!authenticated.requires_prerequisite_authentication());
        assert!(authenticated.requires_hsa_kernel_resolution());
        assert!(!authenticated.grants_load_authority());
        assert!(!authenticated.grants_launch_authority());

        let resolved = authenticated.resolve(&mut loaded).unwrap();
        assert_eq!(
            resolved.artifact_identity().symbol().as_str(),
            "second_kernel"
        );
        assert_eq!(resolved.physical_kernel().export_symbol(), "second_kernel");
        assert_eq!(
            resolved.kernel_observation().kernel_object(),
            FakeHsaAdapter::second_kernel_object()
        );
        assert!(!resolved.requires_prerequisite_authentication());
        assert!(!resolved.requires_hsa_kernel_resolution());
        assert!(!resolved.grants_load_authority());
        assert!(!resolved.grants_launch_authority());
        drop(resolved);

        loaded.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    fn dispatch_second_kernel(
        seed: u8,
        fault: AdapterFault,
        geometry: HsaLaunchGeometryV1,
        kernarg_len: usize,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
    ) -> Result<
        HsaCompletedSelectedWorkerV2DispatchV1<SecondTestKernel>,
        HsaGeneratedDispatchError<&'static str>,
    > {
        let (loaded, unloads, _directory) = load_two_kernels(seed);
        let mut loaded = loaded.unwrap();
        let selection = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();
        let authenticated = selection
            .authenticate(&mut FakeAuthenticator::exact())
            .unwrap();
        let resolved = authenticated.resolve(&mut loaded).unwrap();
        resolved.loaded.adapter.fault = fault;
        let mut kernarg = AlignedKernarg([0; 304]);
        kernarg.0[..48].fill(0x5a);
        // SAFETY: this test supplies the fixture's exact explicit bytes and
        // retains its empty synthetic resource set through synchronous return.
        let result = unsafe {
            resolved.dispatch_generated_and_wait(
                geometry,
                &mut kernarg.0[..kernarg_len],
                explicit_byte_len,
                implicit_byte_offset,
                implicit_byte_len,
            )
        };
        loaded.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
        result
    }

    #[test]
    fn selected_kernel_generated_dispatch_is_exact_and_quiescent() {
        let geometry = HsaLaunchGeometryV1::new([32, 1, 1], [256, 1, 1], 0);
        let completed =
            dispatch_second_kernel(0x89, AdapterFault::None, geometry, 304, 48, 48, 256).unwrap();
        assert_eq!(
            completed.artifact_identity().symbol().as_str(),
            "second_kernel"
        );
        assert_eq!(
            completed.completed_dispatch().kernel_object(),
            FakeHsaAdapter::second_kernel_object()
        );
        assert_eq!(completed.completed_dispatch().geometry(), geometry);
        assert!(completed.completed_dispatch().dispatch().completed());
        assert!(!completed.grants_launch_authority());
    }

    #[test]
    fn selected_kernel_dispatch_rejects_byte_and_span_substitutions() {
        let geometry = HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0);
        for (kernarg_len, explicit, offset, implicit) in [
            (303, 48, 48, 255),
            (304, 47, 48, 256),
            (304, 48, 47, 257),
            (304, 48, 48, 255),
        ] {
            assert!(matches!(
                dispatch_second_kernel(
                    0x88,
                    AdapterFault::None,
                    geometry,
                    kernarg_len,
                    explicit,
                    offset,
                    implicit,
                ),
                Err(HsaGeneratedDispatchError::KernargSize)
            ));
        }
    }

    #[test]
    fn selected_kernel_dispatch_rejects_misaligned_complete_storage() {
        let (loaded, unloads, _directory) = load_two_kernels(0x84);
        let mut loaded = loaded.unwrap();
        let selection = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();
        let authenticated = selection
            .authenticate(&mut FakeAuthenticator::exact())
            .unwrap();
        let resolved = authenticated.resolve(&mut loaded).unwrap();
        let mut kernarg = OffsetKernarg([0; 305]);
        assert!(matches!(
            // SAFETY: rejection occurs before the adapter is entered.
            unsafe {
                resolved.dispatch_generated_and_wait(
                    HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0),
                    &mut kernarg.0[1..],
                    48,
                    48,
                    256,
                )
            },
            Err(HsaGeneratedDispatchError::KernargAlignment)
        ));
        loaded.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn selected_kernel_dispatch_rejects_geometry_substitution() {
        assert!(matches!(
            dispatch_second_kernel(
                0x87,
                AdapterFault::None,
                HsaLaunchGeometryV1::new([1, 1, 1], [64, 1, 1], 0),
                304,
                48,
                48,
                256,
            ),
            Err(HsaGeneratedDispatchError::LaunchAuthorization(
                HsaLaunchAuthorizationError::WorkgroupMismatch
            ))
        ));
    }

    #[test]
    fn selected_kernel_dispatch_rejects_adapter_observation_substitutions() {
        let geometry = HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0);
        for fault in [
            AdapterFault::ImplicitExecutable,
            AdapterFault::ImplicitKernel,
            AdapterFault::ImplicitGeometry,
            AdapterFault::ImplicitOffset,
            AdapterFault::ImplicitIncomplete,
            AdapterFault::ExplicitMutation,
            AdapterFault::DispatchObject,
            AdapterFault::DispatchKernel,
            AdapterFault::DispatchGeometry,
            AdapterFault::DispatchIncomplete,
        ] {
            assert!(
                dispatch_second_kernel(0x86, fault, geometry, 304, 48, 48, 256).is_err(),
                "adapter fault {fault:?} was accepted"
            );
        }
    }

    #[derive(Clone, Copy)]
    enum SelectedDispatchStateFault {
        Environment,
        Executable,
        Symbol,
        Kernel,
    }

    #[test]
    fn selected_kernel_dispatch_revalidates_retained_identity_state() {
        for fault in [
            SelectedDispatchStateFault::Environment,
            SelectedDispatchStateFault::Executable,
            SelectedDispatchStateFault::Symbol,
            SelectedDispatchStateFault::Kernel,
        ] {
            let (loaded, unloads, _directory) = load_two_kernels(0x85);
            let mut loaded = loaded.unwrap();
            let selection = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();
            let authenticated = selection
                .authenticate(&mut FakeAuthenticator::exact())
                .unwrap();
            let mut resolved = authenticated.resolve(&mut loaded).unwrap();
            match fault {
                SelectedDispatchStateFault::Environment => {
                    let (crossed, _) = FakeHsaAdapter::new(AdapterFault::RuntimeInstance);
                    resolved.environment = crossed.environment();
                }
                SelectedDispatchStateFault::Executable => {
                    resolved.resolution.executable_object =
                        HsaExecutableObjectIdentityV1::new([0xa1; 32]).unwrap();
                }
                SelectedDispatchStateFault::Symbol => {
                    resolved.resolution.export_symbol = "primary_kernel".into();
                }
                SelectedDispatchStateFault::Kernel => {
                    resolved.resolution.kernel_object = FakeHsaAdapter::kernel_object();
                }
            }
            let mut kernarg = AlignedKernarg([0; 304]);
            kernarg.0[..48].fill(0x5a);
            // SAFETY: the test intentionally corrupts retained descriptive
            // state and expects rejection before any adapter operation.
            assert!(matches!(
                unsafe {
                    resolved.dispatch_generated_and_wait(
                        HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0),
                        &mut kernarg.0,
                        48,
                        48,
                        256,
                    )
                },
                Err(HsaGeneratedDispatchError::SelectionMismatch(_))
            ));
            loaded.unload().unwrap();
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn selected_marker_prerequisite_substitutions_are_rejected() {
        for fault in [
            PrerequisiteFault::FinalizedDigest,
            PrerequisiteFault::Kernel,
            PrerequisiteFault::Marker,
            PrerequisiteFault::Compiler,
            PrerequisiteFault::TypeLayout,
            PrerequisiteFault::Effects,
            PrerequisiteFault::MissingRaceFreedom,
        ] {
            let (loaded, unloads, _directory) = load_two_kernels(0x8c);
            let loaded = loaded.unwrap();
            let selection = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();
            assert!(matches!(
                selection.authenticate(&mut FakeAuthenticator { fault }),
                Err(WorkerV2ExecutableAuthenticationError::Prerequisite(_))
            ));
            loaded.unload().unwrap();
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn selected_hsa_resolution_substitutions_are_rejected() {
        for (fault, expected_field) in [
            (AdapterFault::ResolutionExecutable, "HSA executable object"),
            (AdapterFault::Symbol, "HSA kernel symbol"),
            (AdapterFault::KernelObjectAlias, "HSA kernel object alias"),
            (AdapterFault::KernargSize, "kernarg segment size"),
            (AdapterFault::KernargAlignment, "kernarg segment alignment"),
        ] {
            let (loaded, unloads, _directory) = load_two_kernels(0x8b);
            let mut loaded = loaded.unwrap();
            let selection = loaded.select_typed_kernel::<SecondTestKernel>().unwrap();
            let authenticated = selection
                .authenticate(&mut FakeAuthenticator::exact())
                .unwrap();
            loaded.adapter.fault = fault;
            match authenticated.resolve(&mut loaded) {
                Err(HsaExecutableLoadError::KernelObservationMismatch { field, .. }) => {
                    assert_eq!(field, expected_field);
                }
                other => panic!("fault {fault:?} returned {other:?}"),
            }
            loaded.unload().unwrap();
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn authenticated_selection_cannot_cross_hsa_environments() {
        let (first, first_unloads, _first_directory) = load_two_kernels(0x8a);
        let first = first.unwrap();
        let selection = first.select_typed_kernel::<SecondTestKernel>().unwrap();
        let authenticated = selection
            .authenticate(&mut FakeAuthenticator::exact())
            .unwrap();

        let (second, second_unloads, _second_directory) =
            load_two_kernels_with_fault(0x8a, AdapterFault::RuntimeInstance);
        let mut second = second.unwrap();
        assert!(matches!(
            authenticated.resolve(&mut second),
            Err(HsaExecutableLoadError::KernelObservationMismatch {
                field: "HSA environment",
                ..
            })
        ));

        first.unload().unwrap();
        second.unload().unwrap();
        assert_eq!(first_unloads.load(Ordering::SeqCst), 1);
        assert_eq!(second_unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn exact_typed_vecadd_profile_enters_generated_worker_v2_executor() {
        let (loaded, unloads, _directory) = load(0x92, AdapterFault::None);
        let executor = crate::GeneratedWorkerV2VecAddExecutorV1::bind_observed_for_test(
            loaded.unwrap(),
            ObservedContext::for_test(0x92, 0, "gfx942:sramecc+:xnack-", 1_024, 65_536),
        )
        .unwrap();
        executor.unload().unwrap();
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn generated_worker_v2_executor_rejects_context_substitution() {
        let (loaded, unloads, _directory) = load(0x93, AdapterFault::None);
        assert!(matches!(
            crate::GeneratedWorkerV2VecAddExecutorV1::bind_observed_for_test(
                loaded.unwrap(),
                ObservedContext::for_test(0x93, 1, "gfx942", 1_024, 65_536),
            ),
            Err(crate::GeneratedWorkerV2VecAddBindError::ContextDeviceMismatch)
        ));
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drop_performs_exact_unload() {
        let (loaded, unloads, _directory) = load(0x92, AdapterFault::None);
        drop(loaded.unwrap());
        assert_eq!(unloads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn prerequisite_substitutions_fail_before_hsa_authority() {
        for fault in [
            PrerequisiteFault::FinalizedDigest,
            PrerequisiteFault::Kernel,
            PrerequisiteFault::Marker,
            PrerequisiteFault::Compiler,
            PrerequisiteFault::TypeLayout,
            PrerequisiteFault::Effects,
            PrerequisiteFault::MissingRaceFreedom,
        ] {
            let (admission, _directory) = admitted_for_lifecycle_test(0x93);
            let error = AuthenticatedWorkerV2ExecutableV1::<TestKernel>::authenticate(
                admission,
                &mut FakeAuthenticator { fault },
            )
            .unwrap_err();
            assert!(matches!(
                error,
                WorkerV2ExecutableAuthenticationError::Prerequisite(_)
            ));
        }
    }

    #[test]
    fn physical_device_substitution_denies_load_authority() {
        let (authenticated, _directory) = authenticate(0x94);
        let (adapter, _) = FakeHsaAdapter::new(AdapterFault::DeviceOrdinal);
        assert!(matches!(
            authenticated.authorize_hsa_load(adapter),
            Err(HsaLoadAuthorizationError::Environment(
                HsaEnvironmentMismatch::DeviceOrdinal { .. }
            ))
        ));
    }

    #[test]
    fn runtime_target_requires_exact_xnack_off_and_allows_observed_sramecc_state() {
        for fault in [AdapterFault::None, AdapterFault::TargetSramEccDisabled] {
            let (authenticated, _directory) = authenticate(0xa4);
            let (adapter, _) = FakeHsaAdapter::new(fault);
            assert!(authenticated.authorize_hsa_load(adapter).is_ok());
        }
        for fault in [
            AdapterFault::TargetXnackEnabled,
            AdapterFault::TargetXnackOmitted,
            AdapterFault::TargetProcessor,
        ] {
            let (authenticated, _directory) = authenticate(0xa5);
            let (adapter, _) = FakeHsaAdapter::new(fault);
            assert!(matches!(
                authenticated.authorize_hsa_load(adapter),
                Err(HsaLoadAuthorizationError::Environment(
                    HsaEnvironmentMismatch::Target { .. }
                ))
            ));
        }
    }

    #[test]
    fn load_and_kernel_substitutions_cleanup_without_issuing_authority() {
        for fault in [
            AdapterFault::LoadDigest,
            AdapterFault::LoadLength,
            AdapterFault::Symbol,
            AdapterFault::KernargSize,
            AdapterFault::KernargAlignment,
        ] {
            let (result, unloads, _directory) = load(0x95, fault);
            assert!(result.is_err(), "fault {fault:?} was accepted");
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn geometry_is_checked_against_source_and_physical_contracts() {
        let (loaded, _unloads, _directory) = load(0x96, AdapterFault::None);
        let mut loaded = loaded.unwrap();
        for (geometry, expected) in [
            (
                HsaLaunchGeometryV1::new([0, 1, 1], [256, 1, 1], 0),
                HsaLaunchAuthorizationError::ZeroDimension,
            ),
            (
                HsaLaunchGeometryV1::new([1, 2, 1], [256, 1, 1], 0),
                HsaLaunchAuthorizationError::RankMismatch,
            ),
            (
                HsaLaunchGeometryV1::new([65_536, 1, 1], [256, 1, 1], 0),
                HsaLaunchAuthorizationError::GridExceedsContract,
            ),
            (
                HsaLaunchGeometryV1::new([1, 1, 1], [64, 1, 1], 0),
                HsaLaunchAuthorizationError::WorkgroupMismatch,
            ),
            (
                HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 1),
                HsaLaunchAuthorizationError::DynamicSharedMemoryExceedsContract,
            ),
        ] {
            assert!(matches!(
                loaded.authorize_launch(geometry),
                Err(actual) if actual == expected
            ));
        }
    }

    #[test]
    fn kernarg_and_dispatch_observation_substitutions_fail_closed() {
        let (loaded, _unloads, _directory) = load(0x97, AdapterFault::None);
        let mut loaded = loaded.unwrap();
        let launch = loaded
            .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
            .unwrap();
        let mut short = [0_u8; 8];
        assert!(matches!(
            unsafe { launch.launch_and_wait(&mut short) },
            Err(HsaDispatchError::KernargSize)
        ));

        for fault in [
            AdapterFault::DispatchObject,
            AdapterFault::DispatchGeometry,
            AdapterFault::DispatchIncomplete,
        ] {
            let (loaded, _unloads, _directory) = load(0x98, fault);
            let mut loaded = loaded.unwrap();
            let launch = loaded
                .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
                .unwrap();
            let mut kernarg = AlignedKernarg([0; 304]);
            assert!(matches!(
                unsafe { launch.launch_and_wait(&mut kernarg.0) },
                Err(HsaDispatchError::ObservationMismatch(_))
            ));
        }
    }

    #[test]
    fn generated_dispatch_delegates_only_the_implicit_span() {
        let (loaded, _unloads, _directory) = load(0x9a, AdapterFault::None);
        let mut loaded = loaded.unwrap();
        let launch = loaded
            .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
            .unwrap();
        let explicit = [0x5a; 48];
        let mut kernarg = AlignedKernarg([0; 304]);
        let completed = launch
            .launch_generated_with_implicit_kernarg(&explicit, 48, 256, &mut kernarg.0)
            .unwrap();
        assert!(completed.dispatch().completed());
        assert_eq!(kernarg.0[..48], explicit);
        assert_eq!(kernarg.0[48..], [0xa5; 256]);
    }

    #[test]
    fn generated_dispatch_rejects_span_and_adapter_substitution() {
        let (loaded, _unloads, _directory) = load(0x9b, AdapterFault::None);
        let mut loaded = loaded.unwrap();
        let launch = loaded
            .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
            .unwrap();
        let explicit = [0x5a; 48];
        let mut kernarg = AlignedKernarg([0; 304]);
        assert!(matches!(
            launch.launch_generated_with_implicit_kernarg(&explicit, 56, 248, &mut kernarg.0),
            Err(HsaGeneratedDispatchError::KernargSize)
        ));

        for (fault, expected) in [
            (
                AdapterFault::ImplicitExecutable,
                "implicit kernarg executable object",
            ),
            (AdapterFault::ImplicitOffset, "implicit kernarg offset"),
            (
                AdapterFault::ImplicitIncomplete,
                "implicit kernarg initialization completion",
            ),
        ] {
            let (loaded, _unloads, _directory) = load(0x9c, fault);
            let mut loaded = loaded.unwrap();
            let launch = loaded
                .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
                .unwrap();
            let mut kernarg = AlignedKernarg([0; 304]);
            assert!(matches!(
                launch.launch_generated_with_implicit_kernarg(
                    &explicit,
                    48,
                    256,
                    &mut kernarg.0,
                ),
                Err(HsaGeneratedDispatchError::ImplicitObservationMismatch(field))
                    if field == expected
            ));
        }

        let (loaded, _unloads, _directory) = load(0x9d, AdapterFault::ExplicitMutation);
        let mut loaded = loaded.unwrap();
        let launch = loaded
            .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
            .unwrap();
        let mut kernarg = AlignedKernarg([0; 304]);
        assert!(matches!(
            launch.launch_generated_with_implicit_kernarg(&explicit, 48, 256, &mut kernarg.0),
            Err(HsaGeneratedDispatchError::ExplicitKernargMutation)
        ));

        let (loaded, _unloads, _directory) = load(0x9e, AdapterFault::DispatchAdapterError);
        let mut loaded = loaded.unwrap();
        let launch = loaded
            .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
            .unwrap();
        let mut kernarg = AlignedKernarg([0; 304]);
        assert!(matches!(
            launch.launch_generated_with_implicit_kernarg(&explicit, 48, 256, &mut kernarg.0),
            Err(HsaGeneratedDispatchError::DispatchAdapter(
                "definite pre-submit dispatch failure"
            ))
        ));

        let (loaded, _unloads, _directory) = load(0x9f, AdapterFault::DispatchObject);
        let mut loaded = loaded.unwrap();
        let launch = loaded
            .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
            .unwrap();
        let mut kernarg = AlignedKernarg([0; 304]);
        assert!(matches!(
            launch.launch_generated_with_implicit_kernarg(&explicit, 48, 256, &mut kernarg.0),
            Err(HsaGeneratedDispatchError::DispatchObservationMismatch(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn adapter_unwind_and_ambiguous_unload_are_terminal() {
        const CASE: &str = "FE2O3_HSA_TERMINAL_ADAPTER_CASE";
        if let Ok(case) = std::env::var(CASE) {
            match case.as_str() {
                "dispatch-panic" => {
                    let (loaded, _unloads, _directory) = load(0xa0, AdapterFault::DispatchPanic);
                    let mut loaded = loaded.unwrap();
                    let launch = loaded
                        .authorize_launch(HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0))
                        .unwrap();
                    let explicit = [0x5a; 48];
                    let mut kernarg = AlignedKernarg([0; 304]);
                    let _caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = launch.launch_generated_with_implicit_kernarg(
                            &explicit,
                            48,
                            256,
                            &mut kernarg.0,
                        );
                    }));
                }
                "unload-error" => {
                    let (loaded, _unloads, _directory) =
                        load(0xa1, AdapterFault::UnloadAdapterError);
                    let _caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = loaded.unwrap().unload();
                    }));
                }
                "unload-panic" => {
                    let (loaded, _unloads, _directory) = load(0xa2, AdapterFault::UnloadPanic);
                    let _caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = loaded.unwrap().unload();
                    }));
                }
                "unload-observation" => {
                    let (loaded, _unloads, _directory) = load(0xa3, AdapterFault::UnloadIncomplete);
                    let _caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = loaded.unwrap().unload();
                    }));
                }
                _ => panic!("unknown terminal adapter test case"),
            }
            std::process::exit(91);
        }

        use std::os::unix::process::ExitStatusExt;
        for case in [
            "dispatch-panic",
            "unload-error",
            "unload-panic",
            "unload-observation",
        ] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg(
                    "hsa_executable_lifecycle::tests::adapter_unwind_and_ambiguous_unload_are_terminal",
                )
                .arg("--nocapture")
                .env(CASE, case)
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(6), "terminal case {case}: {status}");
        }
    }

    #[test]
    fn descriptive_observations_reject_zero_and_crossed_identities() {
        assert!(matches!(
            HsaRuntimeIdentityV1::new("ROCr", "v1", digest(1), [0; 16]),
            Err(HsaObservationError::ZeroIdentity("HSA runtime instance"))
        ));
        let target = AmdTargetId::parse(REQUIRED_TARGET).unwrap();
        let runtime = HsaRuntimeIdentityV1::new("ROCr", "v1", digest(1), [1; 16]).unwrap();
        let device = HsaPhysicalDeviceIdentityV1::new([2; 16], 0, 0, target).unwrap();
        let crossed_agent = HsaAgentIdentityV1::new([3; 16], 1, device.uuid(), target).unwrap();
        assert!(matches!(
            HsaEnvironmentObservationV1::new(runtime, device, crossed_agent),
            Err(HsaObservationError::IdentityMismatch("runtime instance"))
        ));
    }
}
