use crate::{
    AdmittedFinalizedWorkerV2BundleV1, ArtifactKernelIdentityV1, CompilerGeneratedKernelContractV1,
    DeviceIdentity, FinalizedWorkerV2BundleAdmissionError, PhysicalMetadataValueV1,
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
    admission: &'admission AdmittedFinalizedWorkerV2BundleV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> WorkerV2PrerequisiteRequestV1<'_, K> {
    pub const fn artifact_identity(&self) -> &ArtifactKernelIdentityV1 {
        self.admission.artifact_identity()
    }

    pub const fn finalized_digest(&self) -> PayloadDigest {
        self.admission.finalized_payload_identity().digest()
    }

    pub fn target(&self) -> AmdTargetId {
        self.admission.target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.admission.code_object_version()
    }

    pub const fn device(&self) -> &DeviceIdentity {
        self.admission.device()
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
            admission: &admission,
            _marker: PhantomData,
        };
        // SAFETY: callers cannot reach this transition through a safe trait
        // implementation. Every returned field is independently checked below.
        let prerequisites = unsafe { authenticator.authenticate(&request) }
            .map_err(WorkerV2ExecutableAuthenticationError::Authenticator)?;
        validate_prerequisites::<K>(&admission, &prerequisites)
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
        let environment =
            unsafe { adapter.observe_environment() }.map_err(HsaLoadAuthorizationError::Adapter)?;
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
    admission: &AdmittedFinalizedWorkerV2BundleV1,
    actual: &WorkerV2PrerequisiteDecisionV1,
) -> Result<(), WorkerV2PrerequisiteError> {
    let artifact = admission.artifact_identity();
    for (matches, field) in [
        (
            actual.finalized_digest == admission.finalized_payload_identity().digest(),
            "finalized code object",
        ),
        (actual.kernel_id == artifact.kernel_id(), "kernel"),
        (
            actual.executable_digest == artifact.executable_digest(),
            "executable semantics",
        ),
        (actual.target == admission.target(), "target"),
        (
            actual.code_object_version == admission.code_object_version(),
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
/// for dispatch completion, and release the exact executable on unload. Handles
/// must remain valid while owned by the adapter state and must never be reused
/// under an existing identity.
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
    /// The returned handle and observation must denote only `bytes`.
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
    unsafe fn resolve_kernel(
        &mut self,
        executable: &Self::Executable,
        export_symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error>;

    /// Dispatches and waits until all effects have quiesced.
    ///
    /// # Safety
    ///
    /// The adapter must use the exact handles, geometry, and kernarg storage,
    /// and return success only after completion is unambiguous.
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
    /// are fully released. Failure is treated as an ambiguous terminal state.
    unsafe fn unload_executable(
        &mut self,
        executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error>;
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
        let (executable, load) = unsafe { self.adapter.load_executable(bytes, digest) }
            .map_err(HsaExecutableLoadError::AdapterLoad)?;
        drop(current);

        if let Err(field) = validate_load_observation(&self.environment, digest, byte_len, &load) {
            let cleanup = unsafe { self.adapter.unload_executable(executable) }.err();
            return Err(HsaExecutableLoadError::LoadObservationMismatch { field, cleanup });
        }

        let symbol = self
            .authenticated
            .admission
            .selected_kernel()
            .export_symbol();
        // SAFETY: the executable survived exact load observation validation;
        // the adapter must resolve only this exact symbol against that handle.
        let (kernel, resolution) = match unsafe { self.adapter.resolve_kernel(&executable, symbol) }
        {
            Ok(resolved) => resolved,
            Err(source) => {
                let cleanup = unsafe { self.adapter.unload_executable(executable) }.err();
                return Err(HsaExecutableLoadError::KernelResolution { source, cleanup });
            }
        };
        if let Err(field) = validate_kernel_resolution(
            &self.authenticated.admission,
            load.executable_object,
            &resolution,
        ) {
            drop(kernel);
            let cleanup = unsafe { self.adapter.unload_executable(executable) }.err();
            return Err(HsaExecutableLoadError::KernelObservationMismatch { field, cleanup });
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
    if expected_target.to_string() != REQUIRED_TARGET
        || actual_target != expected_target
        || environment.agent.target != expected_target
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
            resolution.kernarg_segment_alignment == physical.kernarg_segment_alignment(),
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
        let unload = unsafe { self.adapter.unload_executable(executable) }
            .map_err(HsaExecutableUnloadError::Adapter)?;
        validate_unload_observation(&self.environment, &self.load, &unload)
            .map_err(HsaExecutableUnloadError::ObservationMismatch)?;
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

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> Drop for LoadedHsaExecutableV1<K, A> {
    fn drop(&mut self) {
        self.kernel.take();
        let Some(executable) = self.executable.take() else {
            return;
        };
        // SAFETY: Drop runs only after all Rust borrows of this value end. The
        // unsafe adapter owns the remaining native lifetime obligations.
        let unload = unsafe { self.adapter.unload_executable(executable) };
        let valid = unload.as_ref().is_ok_and(|observation| {
            validate_unload_observation(&self.environment, &self.load, observation).is_ok()
        });
        if !valid {
            std::process::abort();
        }
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
        let dispatch = unsafe {
            self.loaded
                .adapter
                .launch_and_wait(executable, kernel, self.geometry, kernarg)
        }
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
    if geometry.grid.contains(&0) || geometry.workgroup.contains(&0) {
        return Err(HsaLaunchAuthorizationError::ZeroDimension);
    }
    let workgroup_product = product(geometry.workgroup)?;
    product(geometry.grid)?;
    let source = admission.artifact_identity().launch();
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
    let physical = admission.selected_kernel().launch();
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
    use crate::CompilerGeneratedKernelProfileV1;
    use crate::worker_v2_bundle_admission::tests::{TestDirectory, admitted_for_lifecycle_test};
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
            CompilerGeneratedKernelProfileV1::TypedVecAddF32V1;
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x4b; 32];

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

    unsafe impl WorkerV2PrerequisiteAuthenticatorV1<TestKernel> for FakeAuthenticator {
        type Error = &'static str;

        unsafe fn authenticate(
            &mut self,
            request: &WorkerV2PrerequisiteRequestV1<'_, TestKernel>,
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
                TestKernel::KERNEL_BINDING_ID_V1
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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum AdapterFault {
        None,
        DeviceOrdinal,
        LoadDigest,
        LoadLength,
        Symbol,
        KernargSize,
        KernargAlignment,
        DispatchObject,
        DispatchGeometry,
        DispatchIncomplete,
        UnloadObject,
        UnloadIncomplete,
    }

    #[derive(Debug)]
    struct FakeExecutable;

    #[derive(Debug)]
    struct FakeKernel;

    type TestLoadedResult = Result<
        LoadedHsaExecutableV1<TestKernel, FakeHsaAdapter>,
        HsaExecutableLoadError<&'static str>,
    >;

    struct FakeHsaAdapter {
        fault: AdapterFault,
        unloads: Arc<AtomicUsize>,
    }

    impl FakeHsaAdapter {
        fn new(fault: AdapterFault) -> (Self, Arc<AtomicUsize>) {
            let unloads = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    fault,
                    unloads: unloads.clone(),
                },
                unloads,
            )
        }

        fn environment(&self) -> HsaEnvironmentObservationV1 {
            let target = AmdTargetId::parse(REQUIRED_TARGET).unwrap();
            let runtime =
                HsaRuntimeIdentityV1::new("ROCr", "test-v1", digest(0x71), [0x72; 16]).unwrap();
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
                296
            } else {
                288
            };
            let alignment = if self.fault == AdapterFault::KernargAlignment {
                16
            } else {
                8
            };
            Ok((
                FakeKernel,
                HsaKernelResolutionObservationV1::new(
                    Self::executable_object(),
                    Self::kernel_object(),
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
            _kernel: &Self::Kernel,
            geometry: HsaLaunchGeometryV1,
            _kernarg: &mut [u8],
        ) -> Result<HsaDispatchObservationV1, Self::Error> {
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
            HsaDispatchObservationV1::new(
                [0x82; 16],
                executable,
                Self::kernel_object(),
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

    fn load(seed: u8, fault: AdapterFault) -> (TestLoadedResult, Arc<AtomicUsize>, TestDirectory) {
        let (authenticated, directory) = authenticate(seed);
        let (adapter, unloads) = FakeHsaAdapter::new(fault);
        let authorized = authenticated.authorize_hsa_load(adapter).unwrap();
        assert!(authorized.grants_load_authority());
        assert!(!authorized.grants_launch_authority());
        (authorized.load(), unloads, directory)
    }

    #[repr(align(8))]
    struct AlignedKernarg([u8; 288]);

    #[test]
    fn complete_lifecycle_binds_all_identities_and_unloads() {
        let (loaded, unloads, _directory) = load(0x91, AdapterFault::None);
        let mut loaded = loaded.unwrap();
        assert_eq!(loaded.load_observation().byte_len(), 3256);
        assert_eq!(
            loaded.kernel_observation().export_symbol(),
            "primary_kernel"
        );
        assert_eq!(loaded.kernel_observation().kernarg_segment_size(), 288);
        assert_eq!(loaded.kernel_observation().kernarg_segment_alignment(), 8);

        let geometry = HsaLaunchGeometryV1::new([32, 1, 1], [256, 1, 1], 0);
        let launch = loaded.authorize_launch(geometry).unwrap();
        assert!(launch.grants_launch_authority());
        let mut kernarg = AlignedKernarg([0; 288]);
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
            let mut kernarg = AlignedKernarg([0; 288]);
            assert!(matches!(
                unsafe { launch.launch_and_wait(&mut kernarg.0) },
                Err(HsaDispatchError::ObservationMismatch(_))
            ));
        }
    }

    #[test]
    fn explicit_unload_rejects_object_and_completion_substitution() {
        for fault in [AdapterFault::UnloadObject, AdapterFault::UnloadIncomplete] {
            let (loaded, unloads, _directory) = load(0x99, fault);
            assert!(matches!(
                loaded.unwrap().unload(),
                Err(HsaExecutableUnloadError::ObservationMismatch(_))
            ));
            assert_eq!(unloads.load(Ordering::SeqCst), 1);
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
