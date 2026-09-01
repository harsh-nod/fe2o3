use crate::generated_argument_plan::validate_worker_v3_argument_packing;
use crate::{
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedArgumentLayoutV1,
    CompilerGeneratedKernelExpectationV1, DeviceIdentity, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, ObservedContext, RecoveredWorkerV3AdmissionErrorV1,
};
use fe2o3_amd_target::{AmdTargetId, ProductionAmdTargetProfileV1};
use fe2o3_artifacts::{DigestAlgorithm, DigestBytes, PayloadDigest};
use fe2o3_hsaco::InspectedKernel;
use fe2o3_kernel_descriptor::{BlockSizeV1, KernelDescriptorV1, KernelId};
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;

const HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;
const MAX_HSA_IDENTITY_TEXT_BYTES: usize = 256;

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

    pub const fn runtime_instance(&self) -> [u8; 16] {
        self.runtime_instance
    }

    pub const fn agent_handle(&self) -> u64 {
        self.agent_handle
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
    group_segment_size: u64,
    private_segment_size: u64,
}

impl HsaKernelResolutionObservationV1 {
    pub fn new(
        executable_object: HsaExecutableObjectIdentityV1,
        kernel_object: HsaKernelObjectIdentityV1,
        export_symbol: impl Into<Box<str>>,
        kernarg_segment_size: u64,
        kernarg_segment_alignment: u64,
        group_segment_size: u64,
        private_segment_size: u64,
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
            group_segment_size,
            private_segment_size,
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

    pub const fn group_segment_size(&self) -> u64 {
        self.group_segment_size
    }

    pub const fn private_segment_size(&self) -> u64 {
        self.private_segment_size
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

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.kernel_object
    }

    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.geometry
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

    pub const fn runtime_instance(&self) -> [u8; 16] {
        self.runtime_instance
    }

    pub const fn agent_handle(&self) -> u64 {
        self.agent_handle
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

/// Reviewed extension that prepares compiler-declared implicit kernargs and
/// binds the exact launch queue for explicit-only kernels.
///
/// The base lifecycle adapter deliberately accepts only complete raw kernarg
/// storage. Generated typed launch code uses this extension so application code
/// never constructs or initializes AMDHSA hidden arguments.
///
/// # Safety
///
/// Implementations must preserve every byte in the explicit span, initialize
/// the complete implicit span when present for the exact executable, kernel,
/// and geometry, bind exactly one reviewed queue even when the implicit span is
/// empty, and report an observation derived from that same operation. Success
/// must not be reported while any hidden byte required by the code-object ABI
/// remains uninitialized. Implementing this trait does not itself grant launch
/// authority; the lifecycle validates the observation before dispatch.
pub unsafe trait ReviewedHsaImplicitKernargAdapterV1:
    ReviewedHsaExecutableLifecycleAdapterV1
{
    /// Initializes only the compiler-declared implicit span in `kernarg` and
    /// creates the exact queue binding used by the following launch.
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

    /// Returns whether the reviewed adapter reported complete initialization.
    pub const fn initialized(&self) -> bool {
        self.initialized
    }

    /// Returns the executable identity used for initialization.
    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    /// Returns the kernel identity used for initialization.
    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.kernel_object
    }

    /// Returns the exact launch geometry used to initialize hidden arguments.
    pub const fn geometry(&self) -> HsaLaunchGeometryV1 {
        self.geometry
    }

    /// Returns the preserved explicit prefix length.
    pub const fn explicit_byte_len(&self) -> u64 {
        self.explicit_byte_len
    }

    /// Returns the first byte of the initialized implicit suffix.
    pub const fn implicit_byte_offset(&self) -> u64 {
        self.implicit_byte_offset
    }

    /// Returns the complete initialized implicit suffix length.
    pub const fn implicit_byte_len(&self) -> u64 {
        self.implicit_byte_len
    }
}

/// Environment-authenticated permission to load one exact verified Worker V3 executable.
///
/// The value is linear. The verifier-entry durable publication lock remains retained through native
/// executable unload; no stale generation can be loaded or turned over between verification and
/// native state retirement.
pub struct AuthorizedWorkerV3HsaLoadV1<K, A: ReviewedHsaExecutableLifecycleAdapterV1> {
    authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
    observed: ObservedContext,
    adapter: A,
    environment: HsaEnvironmentObservationV1,
}

pub(crate) fn authorize_worker_v3_hsa_load_v1<
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
>(
    authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
    observed: ObservedContext,
    mut adapter: A,
) -> Result<AuthorizedWorkerV3HsaLoadV1<K, A>, WorkerV3HsaLoadAuthorizationErrorV1<A::Error>> {
    authenticated
        .revalidate_currentness()
        .map_err(WorkerV3HsaLoadAuthorizationErrorV1::CurrentPublication)?;
    // SAFETY: only an unsafe reviewed adapter can enter this migration boundary. Its complete
    // observation is checked against the artifact target and separately supplied HIP context
    // before load authority is returned.
    let environment = reviewed_adapter_call(|| unsafe { adapter.observe_environment() })
        .map_err(WorkerV3HsaLoadAuthorizationErrorV1::Adapter)?;
    validate_environment_facts(authenticated.target(), observed.device(), &environment)
        .map_err(WorkerV3HsaLoadAuthorizationErrorV1::Environment)?;
    Ok(AuthorizedWorkerV3HsaLoadV1 {
        authenticated,
        observed,
        adapter,
        environment,
    })
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> AuthorizedWorkerV3HsaLoadV1<K, A> {
    pub const fn grants_load_authority(&self) -> bool {
        true
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn environment(&self) -> &HsaEnvironmentObservationV1 {
        &self.environment
    }
}

impl<K: CompilerGeneratedKernelExpectationV1, A: ReviewedHsaExecutableLifecycleAdapterV1>
    AuthorizedWorkerV3HsaLoadV1<K, A>
{
    pub fn load(
        mut self,
    ) -> Result<LoadedWorkerV3HsaExecutableV1<K, A>, WorkerV3HsaExecutableLoadErrorV1<A::Error>>
    {
        self.authenticated
            .revalidate_currentness()
            .map_err(WorkerV3HsaExecutableLoadErrorV1::CurrentPublication)?;
        let current = self.authenticated.current_publication_token();
        let bytes = current.exact_artifact_bytes();
        let verification = self.authenticated.verification();
        if u64::try_from(bytes.len()).ok() != Some(verification.finalized_hsaco_length()) {
            return Err(WorkerV3HsaExecutableLoadErrorV1::ExactBytesChanged);
        }
        let digest = PayloadDigest::new(
            DigestAlgorithm::Sha256,
            DigestBytes::from_bytes(verification.finalized_hsaco_sha256()),
        );
        digest
            .verify(bytes)
            .map_err(|_| WorkerV3HsaExecutableLoadErrorV1::ExactBytesChanged)?;

        // SAFETY: the verified authority, reviewed environment, retained currentness token, and
        // exact digest all remain live. Adapter observations are checked before they advance.
        let (executable, load) =
            reviewed_adapter_call(|| unsafe { self.adapter.load_executable(bytes, digest) })
                .map_err(WorkerV3HsaExecutableLoadErrorV1::AdapterLoad)?;
        if let Err(field) = validate_load_observation(
            &self.environment,
            digest,
            verification.finalized_hsaco_length(),
            &load,
        ) {
            terminal_unload(&mut self.adapter, executable, &self.environment, &load);
            return Err(WorkerV3HsaExecutableLoadErrorV1::LoadObservationMismatch { field });
        }

        let descriptor = self.authenticated.descriptor();
        let physical = self.authenticated.admission().physical_kernel();
        let symbol = descriptor.entry_name().as_str();
        // SAFETY: the exact executable and admitted symbol are retained by this linear state.
        let (kernel, resolution) = match reviewed_adapter_call(|| unsafe {
            self.adapter.resolve_kernel(&executable, symbol)
        }) {
            Ok(resolved) => resolved,
            Err(source) => {
                terminal_unload(&mut self.adapter, executable, &self.environment, &load);
                return Err(WorkerV3HsaExecutableLoadErrorV1::KernelResolution(source));
            }
        };
        if let Err(field) = validate_kernel_resolution_fields(
            symbol,
            physical.kernarg_segment_size(),
            physical.kernarg_segment_alignment(),
            physical.group_segment_fixed_size(),
            physical.private_segment_fixed_size(),
            load.executable_object(),
            &resolution,
        ) {
            drop(kernel);
            terminal_unload(&mut self.adapter, executable, &self.environment, &load);
            return Err(WorkerV3HsaExecutableLoadErrorV1::KernelObservationMismatch { field });
        }

        Ok(LoadedWorkerV3HsaExecutableV1 {
            authenticated: self.authenticated,
            observed: self.observed,
            adapter: self.adapter,
            environment: self.environment,
            executable: Some(executable),
            kernel: Some(kernel),
            load,
            resolution,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3HsaLoadAuthorizationErrorV1<E> {
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    Adapter(E),
    Environment(HsaEnvironmentMismatch),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3HsaExecutableLoadErrorV1<E> {
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    ExactBytesChanged,
    AdapterLoad(E),
    LoadObservationMismatch { field: &'static str },
    KernelResolution(E),
    KernelObservationMismatch { field: &'static str },
}

impl<E: fmt::Display> fmt::Display for WorkerV3HsaLoadAuthorizationErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "Worker V3 publication revalidation failed: {error}"
                )
            }
            Self::Adapter(error) => write!(formatter, "reviewed HSA adapter failed: {error}"),
            Self::Environment(error) => write!(formatter, "HSA environment mismatch: {error}"),
        }
    }
}

impl<E> Error for WorkerV3HsaLoadAuthorizationErrorV1<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::Adapter(error) => Some(error),
            Self::Environment(error) => Some(error),
        }
    }
}

impl<E: fmt::Display> fmt::Display for WorkerV3HsaExecutableLoadErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "Worker V3 publication revalidation failed: {error}"
                )
            }
            Self::ExactBytesChanged => formatter.write_str("verified HSACO bytes changed"),
            Self::AdapterLoad(error) => write!(formatter, "reviewed HSA load failed: {error}"),
            Self::LoadObservationMismatch { field } => {
                write!(formatter, "HSA load observation {field} mismatch")
            }
            Self::KernelResolution(error) => {
                write!(formatter, "reviewed HSA kernel resolution failed: {error}")
            }
            Self::KernelObservationMismatch { field } => {
                write!(formatter, "HSA kernel observation {field} mismatch")
            }
        }
    }
}

impl<E> Error for WorkerV3HsaExecutableLoadErrorV1<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::AdapterLoad(error) | Self::KernelResolution(error) => Some(error),
            Self::ExactBytesChanged
            | Self::LoadObservationMismatch { .. }
            | Self::KernelObservationMismatch { .. } => None,
        }
    }
}

/// Failure while synchronously dispatching one compiler-generated Worker V3 invocation.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3GeneratedDispatchErrorV1<E> {
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    LaunchAuthorization(HsaLaunchAuthorizationError),
    KernargSize,
    KernargAlignment,
    ImplicitAdapter(E),
    ExplicitKernargMutation,
    ImplicitObservationMismatch(&'static str),
    DispatchAdapter(E),
    DispatchObservationMismatch(&'static str),
    PostDispatchCurrentPublication {
        source: Box<RecoveredWorkerV3AdmissionErrorV1>,
        lineage: crate::WorkerV3HostLineageIdentityV1,
        kernel_id: KernelId,
        completed: Box<HsaCompletedDispatchV1>,
    },
}

/// Loaded and resolved HSA authority for one exact verified Worker V3 kernel.
///
/// Native handles and the exact durable currentness token remain private. Dispatch is available
/// only through a compiler-generated typed argument implementation and a linear prepared value.
pub struct LoadedWorkerV3HsaExecutableV1<K, A: ReviewedHsaExecutableLifecycleAdapterV1> {
    authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
    observed: ObservedContext,
    adapter: A,
    environment: HsaEnvironmentObservationV1,
    executable: Option<A::Executable>,
    kernel: Option<A::Kernel>,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
}

impl<K: CompilerGeneratedKernelExpectationV1, A: ReviewedHsaExecutableLifecycleAdapterV1> fmt::Debug
    for LoadedWorkerV3HsaExecutableV1<K, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedWorkerV3HsaExecutableV1")
            .field(
                "lineage",
                &self.authenticated.verification().lineage_identity(),
            )
            .field("environment", &self.environment)
            .field("load", &self.load)
            .field("resolution", &self.resolution)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1, A: ReviewedHsaExecutableLifecycleAdapterV1>
    LoadedWorkerV3HsaExecutableV1<K, A>
{
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

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.authenticated.revalidate_currentness()
    }

    pub(crate) fn descriptor(&self) -> &KernelDescriptorV1 {
        self.authenticated.descriptor()
    }

    pub(crate) fn physical_kernel(&self) -> &InspectedKernel {
        self.authenticated.admission().physical_kernel()
    }

    pub(crate) const fn authenticated_verification_v1(
        &self,
    ) -> &crate::WorkerV3VerificationDecisionV1 {
        self.authenticated.verification()
    }

    pub(crate) fn matches_observed_context(&self, observed: &crate::ObservedContext) -> bool {
        let admitted = &self.observed;
        observed.device() == admitted.device()
            && observed.same_context(admitted)
            && observed.same_launch_limits(admitted)
            && observed.same_hip_capabilities(admitted)
            && observed.device().ordinal() == self.environment.physical_device().hip_ordinal()
            && observed
                .device()
                .target_id()
                .is_compatible_with_observed(&self.environment.physical_device().target())
    }

    pub(crate) fn validate_worker_v3_launch_geometry(
        &self,
        geometry: HsaLaunchGeometryV1,
    ) -> Result<(), HsaLaunchAuthorizationError> {
        validate_worker_v3_launch_geometry(self.descriptor(), self.physical_kernel(), geometry)
    }

    pub(crate) unsafe fn validate_worker_v3_argument_packing(
        &self,
        generated: &CompilerGeneratedArgumentLayoutV1,
    ) -> Result<GeneratedArgumentPackingPlanV1, GeneratedArgumentPackingError> {
        validate_worker_v3_argument_packing(
            self.authenticated.admission().descriptor_table(),
            self.descriptor(),
            generated,
        )
    }

    pub fn unload(mut self) -> Result<UnloadedHsaExecutableV1, HsaExecutableUnloadError<A::Error>> {
        self.revalidate_currentness()
            .map_err(|_| HsaExecutableUnloadError::ObservationMismatch("current publication"))?;
        self.kernel.take();
        let executable = self
            .executable
            .take()
            .expect("loaded Worker V3 state must own an executable");
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

impl<K: CompilerGeneratedKernelExpectationV1, A: ReviewedHsaImplicitKernargAdapterV1>
    LoadedWorkerV3HsaExecutableV1<K, A>
{
    /// Completes the physical COV6 kernarg and synchronously dispatches one generated invocation.
    ///
    /// # Safety
    ///
    /// `kernarg` must contain the exact compiler-generated explicit ABI, and every referenced
    /// allocation must remain live under its admitted ownership and alias contract until return.
    /// The safe generated prepared-invocation API is the only caller of this crate-private SPI.
    pub(crate) unsafe fn dispatch_generated_and_wait(
        &mut self,
        geometry: HsaLaunchGeometryV1,
        kernarg: &mut [u8],
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
    ) -> Result<HsaCompletedWorkerV3DispatchV1<K>, WorkerV3GeneratedDispatchErrorV1<A::Error>> {
        self.revalidate_currentness()
            .map_err(WorkerV3GeneratedDispatchErrorV1::CurrentPublication)?;
        self.validate_worker_v3_launch_geometry(geometry)
            .map_err(WorkerV3GeneratedDispatchErrorV1::LaunchAuthorization)?;

        let expected_size = usize::try_from(self.resolution.kernarg_segment_size)
            .map_err(|_| WorkerV3GeneratedDispatchErrorV1::KernargSize)?;
        let expected_explicit =
            usize::try_from(self.descriptor().abi_layout().explicit_argument_size())
                .map_err(|_| WorkerV3GeneratedDispatchErrorV1::KernargSize)?;
        let physical = self.physical_kernel();
        let physical_implicit_matches = physical_implicit_kernarg_metadata_matches(
            implicit_byte_offset,
            implicit_byte_len,
            physical.implicit_argument_offset(),
            physical.implicit_argument_size(),
        );
        if kernarg.len() != expected_size
            || expected_size
                != usize::try_from(physical.kernarg_segment_size()).unwrap_or(usize::MAX)
            || explicit_byte_len != expected_explicit
            || explicit_byte_len != implicit_byte_offset
            || !physical_implicit_matches
            || implicit_byte_offset
                .checked_add(implicit_byte_len)
                .is_none_or(|end| end != expected_size)
        {
            return Err(WorkerV3GeneratedDispatchErrorV1::KernargSize);
        }
        let alignment = usize::try_from(self.resolution.kernarg_segment_alignment)
            .map_err(|_| WorkerV3GeneratedDispatchErrorV1::KernargAlignment)?;
        if !kernarg.as_ptr().addr().is_multiple_of(alignment) {
            return Err(WorkerV3GeneratedDispatchErrorV1::KernargAlignment);
        }

        let explicit = kernarg[..explicit_byte_len].to_vec();
        let executable = self
            .executable
            .as_ref()
            .expect("loaded Worker V3 state retains its executable");
        let kernel = self
            .kernel
            .as_ref()
            .expect("loaded Worker V3 state retains its resolved kernel");
        // SAFETY: the caller retains all generated argument resources, and the adapter is the
        // reviewed exact lifecycle instance bound to these private handles.
        let implicit = reviewed_adapter_call(|| unsafe {
            self.adapter.initialize_implicit_kernarg(
                executable,
                kernel,
                geometry,
                explicit_byte_len,
                implicit_byte_offset,
                implicit_byte_len,
                kernarg,
            )
        })
        .map_err(WorkerV3GeneratedDispatchErrorV1::ImplicitAdapter)?;
        if kernarg[..explicit_byte_len] != *explicit {
            return Err(WorkerV3GeneratedDispatchErrorV1::ExplicitKernargMutation);
        }
        validate_implicit_kernarg_observation(
            &self.load,
            &self.resolution,
            geometry,
            explicit_byte_len,
            implicit_byte_offset,
            implicit_byte_len,
            &implicit,
        )
        .map_err(WorkerV3GeneratedDispatchErrorV1::ImplicitObservationMismatch)?;

        // SAFETY: complete explicit and implicit storage is validated, and the reviewed adapter
        // can return only before publication or after all submitted effects are quiescent.
        let dispatch = reviewed_adapter_call(|| unsafe {
            self.adapter
                .launch_and_wait(executable, kernel, geometry, kernarg)
        })
        .map_err(WorkerV3GeneratedDispatchErrorV1::DispatchAdapter)?;
        validate_dispatch_observation(&self.load, &self.resolution, geometry, &dispatch)
            .map_err(WorkerV3GeneratedDispatchErrorV1::DispatchObservationMismatch)?;
        let lineage = self.authenticated.verification().lineage_identity();
        let kernel_id = self.descriptor().kernel_id();
        let completed = HsaCompletedDispatchV1 {
            finalized_digest: self.load.finalized_digest,
            executable_object: self.load.executable_object,
            kernel_object: self.resolution.kernel_object,
            geometry,
            dispatch,
        };
        if let Err(source) = self.revalidate_currentness() {
            return Err(
                WorkerV3GeneratedDispatchErrorV1::PostDispatchCurrentPublication {
                    source: Box::new(source),
                    lineage,
                    kernel_id,
                    completed: Box::new(completed),
                },
            );
        }

        Ok(HsaCompletedWorkerV3DispatchV1 {
            lineage,
            kernel_id,
            completed,
            _marker: PhantomData,
        })
    }
}

fn physical_implicit_kernarg_metadata_matches(
    implicit_byte_offset: usize,
    implicit_byte_len: usize,
    physical_offset: Option<u64>,
    physical_size: u64,
) -> bool {
    match implicit_byte_len {
        0 => physical_offset.is_none() && physical_size == 0,
        COV6_IMPLICIT_KERNARG_BYTES => {
            u64::try_from(implicit_byte_offset).is_ok_and(|offset| physical_offset == Some(offset))
                && physical_size == COV6_IMPLICIT_KERNARG_BYTES as u64
        }
        _ => false,
    }
}

/// Quiescent completion evidence for one exact Worker V3 kernel occurrence.
#[derive(Debug)]
pub struct HsaCompletedWorkerV3DispatchV1<K> {
    lineage: crate::WorkerV3HostLineageIdentityV1,
    kernel_id: KernelId,
    completed: HsaCompletedDispatchV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> HsaCompletedWorkerV3DispatchV1<K> {
    pub const fn lineage_identity(&self) -> crate::WorkerV3HostLineageIdentityV1 {
        self.lineage
    }

    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn completed_dispatch(&self) -> &HsaCompletedDispatchV1 {
        &self.completed
    }
}

impl<K, A: ReviewedHsaExecutableLifecycleAdapterV1> Drop for LoadedWorkerV3HsaExecutableV1<K, A> {
    fn drop(&mut self) {
        self.kernel.take();
        let Some(executable) = self.executable.take() else {
            return;
        };
        terminal_unload(&mut self.adapter, executable, &self.environment, &self.load);
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum HsaEnvironmentMismatch {
    Target { actual: String },
    DeviceOrdinal { expected: i32, actual: i32 },
    RuntimeInstance,
    PhysicalDevice,
}

impl fmt::Display for HsaEnvironmentMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target { actual } => write!(formatter, "target {actual} is not authorized"),
            Self::DeviceOrdinal { expected, actual } => write!(
                formatter,
                "HIP device ordinal {actual} does not match admitted ordinal {expected}"
            ),
            Self::RuntimeInstance => formatter.write_str("HSA runtime instance changed"),
            Self::PhysicalDevice => formatter.write_str("HSA physical device changed"),
        }
    }
}

impl Error for HsaEnvironmentMismatch {}

fn production_profile_for_artifact_target(
    target: AmdTargetId,
) -> Option<ProductionAmdTargetProfileV1> {
    ProductionAmdTargetProfileV1::from_device_target(&target.to_string())
}

fn validate_environment_facts(
    expected_target: AmdTargetId,
    device: &DeviceIdentity,
    environment: &HsaEnvironmentObservationV1,
) -> Result<(), HsaEnvironmentMismatch> {
    let actual_target = environment.physical_device.target;
    let Some(profile) = production_profile_for_artifact_target(expected_target) else {
        return Err(HsaEnvironmentMismatch::Target {
            actual: actual_target.to_string(),
        });
    };
    let required_runtime_target = AmdTargetId::parse(profile.device_target())
        .expect("typed production target profile must contain a canonical target ID");
    let observed_target = device.target_id();
    if required_runtime_target != expected_target
        || !expected_target.is_compatible_with_observed(&observed_target)
        || !required_runtime_target.is_compatible_with_observed(&observed_target)
    {
        return Err(HsaEnvironmentMismatch::Target {
            actual: observed_target.to_string(),
        });
    }
    if !expected_target.is_compatible_with_observed(&actual_target)
        || !required_runtime_target.is_compatible_with_observed(&actual_target)
        || environment.agent.target != actual_target
    {
        return Err(HsaEnvironmentMismatch::Target {
            actual: actual_target.to_string(),
        });
    }
    if environment.physical_device.hip_ordinal != device.ordinal() {
        return Err(HsaEnvironmentMismatch::DeviceOrdinal {
            expected: device.ordinal(),
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

fn validate_kernel_resolution_fields(
    export_symbol: &str,
    kernarg_segment_size: u64,
    kernarg_segment_alignment: u64,
    group_segment_size: u64,
    private_segment_size: u64,
    executable: HsaExecutableObjectIdentityV1,
    resolution: &HsaKernelResolutionObservationV1,
) -> Result<(), &'static str> {
    let expected_hsa_alignment = kernarg_segment_alignment.max(HSA_MINIMUM_KERNARG_ALIGNMENT);
    for (matches, field) in [
        (
            resolution.executable_object == executable,
            "HSA executable object",
        ),
        (
            resolution.export_symbol.as_ref() == export_symbol,
            "HSA kernel symbol",
        ),
        (
            resolution.kernarg_segment_size == kernarg_segment_size,
            "kernarg segment size",
        ),
        (
            resolution.kernarg_segment_alignment == expected_hsa_alignment,
            "kernarg segment alignment",
        ),
        (
            resolution.group_segment_size == group_segment_size,
            "static group segment size",
        ),
        (
            resolution.private_segment_size == private_segment_size,
            "private segment size",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
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

fn validate_worker_v3_launch_geometry(
    descriptor: &KernelDescriptorV1,
    physical: &InspectedKernel,
    geometry: HsaLaunchGeometryV1,
) -> Result<(), HsaLaunchAuthorizationError> {
    let grid = geometry.grid();
    let workgroup = geometry.workgroup();
    if grid.contains(&0) || workgroup.contains(&0) {
        return Err(HsaLaunchAuthorizationError::ZeroDimension);
    }
    let workgroup_product = product(workgroup)?;
    product(grid)?;
    if workgroup.into_iter().any(|axis| axis > u32::from(u16::MAX)) {
        return Err(HsaLaunchAuthorizationError::WorkgroupExceedsPhysicalLimit);
    }
    for (blocks, threads) in grid.into_iter().zip(workgroup) {
        blocks
            .checked_mul(threads)
            .ok_or(HsaLaunchAuthorizationError::DimensionOverflow)?;
    }

    let source = descriptor.launch();
    if (source.rank() < 2 && (grid[1] != 1 || workgroup[1] != 1))
        || (source.rank() < 3 && (grid[2] != 1 || workgroup[2] != 1))
    {
        return Err(HsaLaunchAuthorizationError::RankMismatch);
    }
    let max_grid = source.max_grid();
    if grid[0] > max_grid.x() || grid[1] > max_grid.y() || grid[2] > max_grid.z() {
        return Err(HsaLaunchAuthorizationError::GridExceedsContract);
    }
    match source.block_size() {
        BlockSizeV1::Any => {}
        BlockSizeV1::Exact(block) if workgroup == [block.x(), block.y(), block.z()] => {}
        BlockSizeV1::Exact(_) => return Err(HsaLaunchAuthorizationError::WorkgroupMismatch),
        BlockSizeV1::AtMost(block)
            if workgroup[0] <= block.x()
                && workgroup[1] <= block.y()
                && workgroup[2] <= block.z() => {}
        BlockSizeV1::AtMost(_) => {
            return Err(HsaLaunchAuthorizationError::WorkgroupExceedsContract);
        }
    }
    if workgroup_product > u64::from(source.max_flat_workgroup_size())
        || workgroup_product > u64::from(physical.max_flat_workgroup_size())
    {
        return Err(HsaLaunchAuthorizationError::WorkgroupExceedsPhysicalLimit);
    }
    match (source.block_size(), physical.required_workgroup_size()) {
        (BlockSizeV1::Exact(_), Some(required)) if required == workgroup => {}
        (BlockSizeV1::Exact(_), Some(_)) => {
            return Err(HsaLaunchAuthorizationError::WorkgroupMismatch);
        }
        (BlockSizeV1::Exact(_), None) => {
            return Err(HsaLaunchAuthorizationError::PhysicalWorkgroupRequirementUnknown);
        }
        (BlockSizeV1::Any | BlockSizeV1::AtMost(_), None) => {}
        (BlockSizeV1::Any | BlockSizeV1::AtMost(_), Some(_)) => {
            return Err(HsaLaunchAuthorizationError::WorkgroupMismatch);
        }
    }
    for (actual, maximum) in grid.into_iter().zip(physical.max_workgroups()) {
        if maximum.is_some_and(|maximum| actual > maximum) {
            return Err(HsaLaunchAuthorizationError::GridExceedsContract);
        }
    }
    if physical.group_segment_fixed_size() != u64::from(source.static_shared_memory_bytes()) {
        return Err(HsaLaunchAuthorizationError::DynamicSharedMemoryNotRepresented);
    }
    if geometry.dynamic_shared_memory_bytes() > source.max_dynamic_shared_memory_bytes() {
        return Err(HsaLaunchAuthorizationError::DynamicSharedMemoryExceedsContract);
    }
    // Generic dynamic-LDS argument association is not yet represented in the V3 descriptor-to-host
    // bridge. Fail closed until that physical binding is authenticated explicitly.
    if geometry.dynamic_shared_memory_bytes() != 0 {
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

    fn environment(target: &str, ordinal: i32) -> HsaEnvironmentObservationV1 {
        let target = AmdTargetId::parse(target).unwrap();
        let runtime = HsaRuntimeIdentityV1::new("ROCr", "v1", digest(1), [1; 16]).unwrap();
        let device = HsaPhysicalDeviceIdentityV1::new([2; 16], 0, ordinal, target).unwrap();
        let agent = HsaAgentIdentityV1::new(runtime.instance(), 3, device.uuid(), target).unwrap();
        HsaEnvironmentObservationV1::new(runtime, device, agent).unwrap()
    }

    fn device(target: &str, ordinal: i32) -> DeviceIdentity {
        crate::ObservedContext::for_test(4, ordinal, target, 1_024, 65_536)
            .device()
            .clone()
    }

    fn digest(seed: u8) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([seed; 32]))
    }

    #[test]
    fn descriptive_observations_reject_zero_and_crossed_identities() {
        assert!(matches!(
            HsaRuntimeIdentityV1::new("ROCr", "v1", digest(1), [0; 16]),
            Err(HsaObservationError::ZeroIdentity("HSA runtime instance"))
        ));
        let target = AmdTargetId::parse("gfx942:xnack-").unwrap();
        let runtime = HsaRuntimeIdentityV1::new("ROCr", "v1", digest(1), [1; 16]).unwrap();
        let device = HsaPhysicalDeviceIdentityV1::new([2; 16], 0, 0, target).unwrap();
        let crossed_agent = HsaAgentIdentityV1::new([3; 16], 1, device.uuid(), target).unwrap();
        assert!(matches!(
            HsaEnvironmentObservationV1::new(runtime, device, crossed_agent),
            Err(HsaObservationError::IdentityMismatch("runtime instance"))
        ));
    }

    #[test]
    fn lifecycle_observation_getters_return_typed_evidence() {
        let executable = HsaExecutableObjectIdentityV1::new([0x31; 32]).unwrap();
        let kernel = HsaKernelObjectIdentityV1::new([0x32; 32]).unwrap();
        let runtime_instance = [0x33; 16];
        let agent_handle = 0x3434;
        let geometry = HsaLaunchGeometryV1::new([7, 1, 1], [64, 1, 1], 1_024);

        let load = HsaCodeObjectLoadObservationV1::new(
            digest(0x35),
            4_096,
            runtime_instance,
            agent_handle,
            executable,
        );
        assert_eq!(load.runtime_instance(), runtime_instance);
        assert_eq!(load.agent_handle(), agent_handle);

        let dispatch =
            HsaDispatchObservationV1::new([0x36; 16], executable, kernel, geometry, true).unwrap();
        assert_eq!(dispatch.executable_object(), executable);
        assert_eq!(dispatch.kernel_object(), kernel);
        assert_eq!(dispatch.geometry(), geometry);

        let unload = HsaUnloadObservationV1::new(executable, runtime_instance, agent_handle, true);
        assert_eq!(unload.runtime_instance(), runtime_instance);
        assert_eq!(unload.agent_handle(), agent_handle);
    }

    #[test]
    fn generated_dispatch_accepts_only_the_two_reviewed_physical_implicit_layouts() {
        assert!(physical_implicit_kernarg_metadata_matches(48, 0, None, 0));
        assert!(physical_implicit_kernarg_metadata_matches(
            48,
            COV6_IMPLICIT_KERNARG_BYTES,
            Some(48),
            COV6_IMPLICIT_KERNARG_BYTES as u64,
        ));

        for (implicit, offset, size) in [
            (0, Some(48), 0),
            (0, None, COV6_IMPLICIT_KERNARG_BYTES as u64),
            (1, None, 0),
            (255, Some(48), 255),
            (257, Some(48), 257),
            (COV6_IMPLICIT_KERNARG_BYTES, None, 256),
            (COV6_IMPLICIT_KERNARG_BYTES, Some(47), 256),
            (COV6_IMPLICIT_KERNARG_BYTES, Some(48), 0),
        ] {
            assert!(!physical_implicit_kernarg_metadata_matches(
                48, implicit, offset, size,
            ));
        }
    }

    #[test]
    fn production_environment_accepts_exact_gfx942_and_gfx950_profiles() {
        for (artifact, observed) in [
            ("gfx942:xnack-", "gfx942:sramecc+:xnack-"),
            ("gfx950:xnack-", "gfx950:sramecc+:xnack-"),
        ] {
            validate_environment_facts(
                AmdTargetId::parse(artifact).unwrap(),
                &device(observed, 0),
                &environment(observed, 0),
            )
            .unwrap();
        }
    }

    #[test]
    fn production_environment_rejects_unspecified_or_enabled_xnack_artifacts() {
        for artifact in ["gfx942", "gfx942:xnack+", "gfx950", "gfx950:xnack+"] {
            let processor = AmdTargetId::parse(artifact).unwrap().processor();
            let observed = format!("{processor}:sramecc+:xnack-");
            assert!(matches!(
                validate_environment_facts(
                    AmdTargetId::parse(artifact).unwrap(),
                    &device(&observed, 0),
                    &environment(&observed, 0),
                ),
                Err(HsaEnvironmentMismatch::Target { .. })
            ));
        }
    }

    #[test]
    fn production_environment_rejects_cross_processor_substitution() {
        for (artifact, observed) in [
            ("gfx942:xnack-", "gfx950:sramecc+:xnack-"),
            ("gfx950:xnack-", "gfx942:sramecc+:xnack-"),
        ] {
            assert!(matches!(
                validate_environment_facts(
                    AmdTargetId::parse(artifact).unwrap(),
                    &device(observed, 0),
                    &environment(observed, 0),
                ),
                Err(HsaEnvironmentMismatch::Target { .. })
            ));
        }
    }
}
