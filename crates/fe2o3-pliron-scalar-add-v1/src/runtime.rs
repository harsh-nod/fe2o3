//! One sealed, one-shot HSA consumer for the repository scalar-add profile.

use core::fmt;
use std::{env, ffi::OsStr, sync::Arc};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_core::GpuContext;
use fe2o3_host::{
    HsaCodeObjectLoadObservationV1, HsaEnvironmentObservationV1, HsaLaunchGeometryV1,
    HsaUnloadObservationV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
use fe2o3_hsa_runtime::{
    HsaRuntimeAdapterError, ReviewedHsaHardwareTestBufferV1, ReviewedHsaRuntimeAdapterV1,
};
use fe2o3_hsaco_finalize::{
    PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES, PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES,
    PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT, PLIRON_SCALAR_ADD_V1_KERNARG_BYTES,
    PLIRON_SCALAR_ADD_V1_KERNEL,
};
use sha2::{Digest as _, Sha256};

use crate::authority::{
    FinalizedRepositoryScalarAddV1, FinalizedRepositoryScalarAddV1Identity,
    RepositoryApprovalIdentityV1, RepositoryRuntimeAuthorityV1, ScalarAddLineageIdentityV1,
    ScalarAddObservationIdentityV1,
};

const REQUIRED_VISIBLE_DEVICE: &str = "0";
const REQUIRED_OBSERVED_TARGET: &str = "gfx942:sramecc+:xnack-";
const REQUIRED_RUNTIME_IMPLEMENTATION: &str = "ROCr HSA";
const REQUIRED_RUNTIME_VERSION: &str = "1.18";
const REQUIRED_RUNTIME_IMAGE_SHA256: [u8; 32] = [
    0x70, 0x10, 0xeb, 0xa8, 0x94, 0x56, 0x9c, 0x04, 0x47, 0x49, 0xb7, 0x1b, 0x63, 0xff, 0x78, 0x20,
    0x80, 0xc4, 0xa9, 0x1e, 0x19, 0xff, 0x24, 0xd6, 0xdc, 0x93, 0xe8, 0x57, 0x04, 0x5a, 0xb3, 0x7e,
];
/// HSA UUID observed when qualifying the pinned derived physical identity.
///
/// Runtime enforcement uses [`REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1`],
/// which the reviewed adapter derives from this HSA UUID together with the HIP
/// UUID and PCI BDF. This text is retained only as human-readable lane evidence.
pub const QUALIFIED_MI300X_HSA_UUID_OBSERVATION_V1: &str = "GPU-6ced1647a296545c";
// ReviewedHsaRuntimeAdapterV1 derives this process-safe identity from the exact
// HIP UUID bytes, HSA UUID above, and PCI BDF 0000:05:00.0.
/// Derived physical-device identity actually enforced by the runtime lane.
pub const REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1: [u8; 16] = [
    0x56, 0x0d, 0xc9, 0x31, 0x39, 0xd4, 0xa1, 0x25, 0xd1, 0x82, 0x31, 0x6c, 0x74, 0x55, 0x64, 0x5b,
];
const GRID: [u32; 3] = [1, 1, 1];
const WORKGROUP: [u32; 3] = [1, 1, 1];
const DYNAMIC_LDS_BYTES: u32 = 0;
const REQUIRED_RUNTIME_KERNARG_ALIGNMENT: u64 = 16;
const CANARY_ELEMENTS: usize = 32;
const INPUT_VALUE: f32 = 1.25;
const ADDEND: f32 = 2.5;
const EXPECTED: f32 = 3.75;
const INPUT_PREFIX: f32 = f32::from_bits(0x7fc0_a001);
const INPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_a002);
const OUTPUT_PREFIX: f32 = f32::from_bits(0x7fc0_c001);
const OUTPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_c002);
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_c0ff);
const SUCCESS_MARKER_V1: &str = "FE2O3_REPOSITORY_SCALAR_ADD_V1_MI300X_OK";
const DEVICE_VALIDATION_SCOPE_V1: &str = "after_minimal_context_before_code_load";
const RUNTIME_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PLIRON-SCALAR-ADD-V1/RUNTIME-EVIDENCE/V1\0";

/// One rejected field in the fixed MI300X lane policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeLaneFieldV1 {
    /// `HSA_XNACK` was absent or not exactly `0`.
    HsaXnack,
    /// `HIP_VISIBLE_DEVICES` was absent or not exactly `0`.
    HipVisibleDevices,
    /// `ROCR_VISIBLE_DEVICES` was absent or not exactly `0`.
    RocrVisibleDevices,
    /// The runtime-selected device was not the repository-pinned MI300X.
    DeviceUuid,
    /// The runtime-selected target was not exactly `gfx942:sramecc+:xnack-`.
    Target,
    /// The loaded ROCr/HIP implementation, version, or image digest changed.
    RuntimeStack,
    /// The selected HSA agent did not bind the selected runtime and device.
    Agent,
}

/// One substituted runtime lifecycle observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeObservationFieldV1 {
    /// The retained finalization no longer binds its policy and output bytes.
    FinalizedArtifact,
    /// The HSA load digest or byte length changed.
    LoadArtifact,
    /// The HSA load runtime or agent changed.
    LoadEnvironment,
    /// The resolved export or executable/kernel identity changed.
    KernelIdentity,
    /// Runtime-reported kernarg size was not exactly 280 bytes.
    KernargSize,
    /// Runtime-reported kernarg alignment was not exactly 16 bytes.
    KernargAlignment,
    /// Runtime-reported static group segment was not exactly zero.
    StaticGroupSegment,
    /// Runtime-reported private segment was not exactly zero.
    PrivateSegment,
    /// Grid or workgroup geometry was not exactly `[1, 1, 1]`.
    Geometry,
    /// Dynamic LDS was not exactly zero.
    DynamicLds,
    /// The reviewed adapter did not initialize the exact 24+256 byte layout.
    ImplicitKernarg,
    /// Input/output body pointers were null, misaligned, overflowing, or overlapping.
    KernelPointers,
    /// The synchronous completion observation changed.
    Dispatch,
    /// The unload runtime, agent, executable, or released state changed.
    Unload,
}

/// One failed post-dispatch scalar or allocation check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeResultFieldV1 {
    /// A guarded allocation had the wrong extent.
    AllocationExtent,
    /// The input allocation changed bitwise.
    InputMutation,
    /// An input prefix or suffix canary changed.
    InputCanary,
    /// An output prefix or suffix canary changed.
    OutputCanary,
    /// The scalar output was not bit-exact `3.75f32`.
    OutputValue,
}

/// Failure from the one-shot repository scalar-add runtime transition.
#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeExecutionErrorV1 {
    /// The fixed process/device lane policy failed before or after context creation.
    Lane(RuntimeLaneFieldV1),
    /// A policy, ABI, resource, dispatch, load, or unload observation changed.
    Observation(RuntimeObservationFieldV1),
    /// A guarded allocation or scalar result check failed.
    Result(RuntimeResultFieldV1),
    /// HIP context construction failed.
    Context(fe2o3_core::Error),
    /// The reviewed HSA adapter rejected a native transition.
    Adapter(HsaRuntimeAdapterError),
    /// A bounded integer or target conversion failed.
    Internal(&'static str),
}

impl fmt::Display for RuntimeExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lane(field) => write!(formatter, "scalar-add MI300X lane changed at {field:?}"),
            Self::Observation(field) => {
                write!(
                    formatter,
                    "scalar-add runtime observation changed at {field:?}"
                )
            }
            Self::Result(field) => write!(formatter, "scalar-add result changed at {field:?}"),
            Self::Context(error) => write!(formatter, "HIP context setup failed: {error}"),
            Self::Adapter(error) => write!(formatter, "reviewed HSA adapter failed: {error}"),
            Self::Internal(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RuntimeExecutionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Context(error) => Some(error),
            Self::Adapter(error) => Some(error),
            Self::Lane(_) | Self::Observation(_) | Self::Result(_) | Self::Internal(_) => None,
        }
    }
}

impl From<HsaRuntimeAdapterError> for RuntimeExecutionErrorV1 {
    fn from(error: HsaRuntimeAdapterError) -> Self {
        Self::Adapter(error)
    }
}

/// Aggregate identity of the complete validated load-to-unload transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeEvidenceIdentityV1([u8; 32]);

impl RuntimeEvidenceIdentityV1 {
    /// Returns the canonical SHA-256 transcript digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Failure to parse or integrity-check a scalar-add success marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeEvidenceMarkerErrorV1 {
    /// Marker field count, order, or prefix changed.
    Structure,
    /// One named marker value was malformed or outside the fixed profile.
    Field(&'static str),
    /// The supplied aggregate identity did not match all parsed fields.
    Identity,
}

impl fmt::Display for RuntimeEvidenceMarkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structure => formatter.write_str("runtime evidence marker structure changed"),
            Self::Field(field) => {
                write!(formatter, "runtime evidence marker field changed: {field}")
            }
            Self::Identity => formatter.write_str("runtime evidence marker identity mismatch"),
        }
    }
}

impl std::error::Error for RuntimeEvidenceMarkerErrorV1 {}

/// Bounded evidence returned only after exact dispatch checks and terminal unload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeEvidenceV1 {
    identity: RuntimeEvidenceIdentityV1,
    finalization: FinalizedRepositoryScalarAddV1Identity,
    lineage: ScalarAddLineageIdentityV1,
    approval: RepositoryApprovalIdentityV1,
    observation: ScalarAddObservationIdentityV1,
    load_sha256: [u8; 32],
    load_byte_len: u64,
    runtime_instance: [u8; 16],
    runtime_image_sha256: [u8; 32],
    device_uuid: [u8; 16],
    agent_handle: u64,
    executable: [u8; 32],
    kernel: [u8; 32],
    dispatch: [u8; 16],
    kernarg_sha256: [u8; 32],
    kernarg_size: u64,
    kernarg_alignment: u64,
    explicit_kernarg_bytes: u64,
    implicit_kernarg_bytes: u64,
    static_group_segment: u32,
    private_segment: u32,
    grid: [u32; 3],
    workgroup: [u32; 3],
    dynamic_lds: u32,
    input_sha256: [u8; 32],
    output_sha256: [u8; 32],
    output_bits: u32,
    unload_runtime_instance: [u8; 16],
    unload_agent_handle: u64,
    unload_executable: [u8; 32],
    unload_released: bool,
}

impl RuntimeEvidenceV1 {
    /// Returns the aggregate identity binding the complete runtime transcript.
    pub const fn identity(&self) -> RuntimeEvidenceIdentityV1 {
        self.identity
    }

    /// Returns the consumed finalization identity.
    pub const fn finalization_identity(&self) -> FinalizedRepositoryScalarAddV1Identity {
        self.finalization
    }

    /// Returns the consumed opaque source lineage identity.
    pub const fn lineage_identity(&self) -> ScalarAddLineageIdentityV1 {
        self.lineage
    }

    /// Returns the checked-in approval manifest identity.
    pub const fn approval_identity(&self) -> RepositoryApprovalIdentityV1 {
        self.approval
    }

    /// Returns the measured worker/output observation identity.
    pub const fn observation_identity(&self) -> ScalarAddObservationIdentityV1 {
        self.observation
    }

    /// Returns the exact SHA-256 of bytes supplied to HSA load.
    pub const fn load_sha256(&self) -> &[u8; 32] {
        &self.load_sha256
    }

    /// Returns the exact number of bytes supplied to HSA load.
    pub const fn load_byte_len(&self) -> u64 {
        self.load_byte_len
    }

    /// Returns the process-local reviewed HSA runtime instance.
    pub const fn runtime_instance(&self) -> [u8; 16] {
        self.runtime_instance
    }

    /// Returns the exact loaded ROCr/HIP image digest admitted by policy.
    pub const fn runtime_image_sha256(&self) -> &[u8; 32] {
        &self.runtime_image_sha256
    }

    /// Returns the reviewed runtime's exact pinned physical-device identity.
    pub const fn device_uuid(&self) -> [u8; 16] {
        self.device_uuid
    }

    /// Returns the reviewed process-local HSA agent handle.
    pub const fn agent_handle(&self) -> u64 {
        self.agent_handle
    }

    /// Returns the opaque loaded executable identity that was later unloaded.
    pub const fn executable_identity(&self) -> [u8; 32] {
        self.executable
    }

    /// Returns the opaque resolved `scalar_add` kernel identity.
    pub const fn kernel_identity(&self) -> [u8; 32] {
        self.kernel
    }

    /// Returns the completed synchronous dispatch identity.
    pub const fn dispatch_identity(&self) -> [u8; 16] {
        self.dispatch
    }

    /// Returns the complete initialized 280-byte kernarg digest.
    pub const fn kernarg_sha256(&self) -> &[u8; 32] {
        &self.kernarg_sha256
    }

    /// Returns the checked post-dispatch input-allocation digest.
    pub const fn input_sha256(&self) -> &[u8; 32] {
        &self.input_sha256
    }

    /// Returns the checked post-dispatch output-allocation digest.
    pub const fn output_sha256(&self) -> &[u8; 32] {
        &self.output_sha256
    }

    /// Returns the bit-exact checked scalar result (`3.75f32`).
    pub const fn output_bits(&self) -> u32 {
        self.output_bits
    }

    /// Returns the runtime instance recorded by terminal unload.
    pub const fn unload_runtime_instance(&self) -> [u8; 16] {
        self.unload_runtime_instance
    }

    /// Returns the HSA agent recorded by terminal unload.
    pub const fn unload_agent_handle(&self) -> u64 {
        self.unload_agent_handle
    }

    /// Returns the executable identity recorded by terminal unload.
    pub const fn unload_executable_identity(&self) -> [u8; 32] {
        self.unload_executable
    }

    /// Returns whether terminal unload reported the executable released.
    pub const fn unload_released(&self) -> bool {
        self.unload_released
    }

    /// Serializes every aggregate-identity field in one exact fixed-order marker.
    ///
    /// The marker is an integrity-checked observation, not a signature or CI
    /// attestation. External acceptance must authenticate the producing job.
    pub fn success_marker_v1(&self) -> String {
        format!(
            "{SUCCESS_MARKER_V1} evidence={} finalization={} lineage={} approval={} observation={} load_sha256={} load_bytes={} runtime={} runtime_version={REQUIRED_RUNTIME_VERSION} runtime_image_sha256={} device={} target={REQUIRED_OBSERVED_TARGET} agent={:016x} executable={} kernel={} dispatch={} kernarg_sha256={} kernarg={},{},{},{},{} segments={},{},{} grid={},{},{} workgroup={},{},{} input_sha256={} output_sha256={} output_bits={:08x} unload_runtime={} unload_agent={:016x} unload_executable={} unload_released={} device_validation={DEVICE_VALIDATION_SCOPE_V1}",
            hex_bytes(self.identity.as_bytes()),
            hex_bytes(self.finalization.as_bytes()),
            hex_bytes(self.lineage.as_bytes()),
            hex_bytes(self.approval.as_bytes()),
            hex_bytes(self.observation.as_bytes()),
            hex_bytes(&self.load_sha256),
            self.load_byte_len,
            hex_bytes(&self.runtime_instance),
            hex_bytes(&self.runtime_image_sha256),
            hex_bytes(&self.device_uuid),
            self.agent_handle,
            hex_bytes(&self.executable),
            hex_bytes(&self.kernel),
            hex_bytes(&self.dispatch),
            hex_bytes(&self.kernarg_sha256),
            self.explicit_kernarg_bytes,
            self.implicit_kernarg_bytes,
            self.kernarg_size,
            PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT,
            self.kernarg_alignment,
            self.static_group_segment,
            self.private_segment,
            self.dynamic_lds,
            self.grid[0],
            self.grid[1],
            self.grid[2],
            self.workgroup[0],
            self.workgroup[1],
            self.workgroup[2],
            hex_bytes(&self.input_sha256),
            hex_bytes(&self.output_sha256),
            self.output_bits,
            hex_bytes(&self.unload_runtime_instance),
            self.unload_agent_handle,
            hex_bytes(&self.unload_executable),
            self.unload_released,
        )
    }

    /// Parses every marker field and verifies policy, cross-field, and aggregate integrity.
    pub fn parse_success_marker_v1(
        marker: &str,
    ) -> Result<RuntimeEvidenceIdentityV1, RuntimeEvidenceMarkerErrorV1> {
        parse_success_marker_v1(marker)
    }

    /// This one bounded execution is not a proof of general memory safety.
    pub const fn claims_general_memory_safety(&self) -> bool {
        false
    }

    /// This one bounded execution is not a proof of general race freedom.
    pub const fn claims_general_race_freedom(&self) -> bool {
        false
    }

    /// This vertical slice does not claim CUDA-Oxide parity.
    pub const fn claims_cuda_oxide_parity(&self) -> bool {
        false
    }
}

#[repr(align(16))]
struct AlignedKernarg([u8; PLIRON_SCALAR_ADD_V1_KERNARG_BYTES as usize]);

#[derive(Clone, Copy)]
struct LaneFactsV1 {
    runtime_implementation_is_exact: bool,
    runtime_version_is_exact: bool,
    runtime_image_sha256: [u8; 32],
    runtime_image_is_policy_exact: bool,
    device_uuid: [u8; 16],
    target: AmdTargetId,
    hip_ordinal: i32,
    runtime: [u8; 16],
    agent_runtime: [u8; 16],
    agent_uuid: [u8; 16],
    agent_target: AmdTargetId,
    agent_handle: u64,
}

#[derive(Clone, Copy)]
struct KernelFactsV1 {
    executable: [u8; 32],
    kernel: [u8; 32],
    symbol_is_exact: bool,
    kernarg_size: u64,
    kernarg_alignment: u64,
    static_group_segment: u32,
    private_segment: u32,
    resource_observation_is_exact: bool,
}

#[derive(Clone, Copy)]
struct LifecycleFactsV1 {
    digest: PayloadDigest,
    byte_len: u64,
    runtime: [u8; 16],
    agent: u64,
    executable: [u8; 32],
    released: bool,
}

#[derive(Clone, Copy)]
struct ExpectedLifecycleV1 {
    digest: PayloadDigest,
    byte_len: u64,
    runtime: [u8; 16],
    agent: u64,
    executable: [u8; 32],
}

#[derive(Clone, Copy)]
struct RuntimeEvidenceFactsV1 {
    finalization: [u8; 32],
    lineage: [u8; 32],
    approval: [u8; 32],
    observation: [u8; 32],
    load_sha256: [u8; 32],
    load_byte_len: u64,
    runtime_instance: [u8; 16],
    runtime_image_sha256: [u8; 32],
    device_uuid: [u8; 16],
    agent_handle: u64,
    executable: [u8; 32],
    kernel: [u8; 32],
    dispatch: [u8; 16],
    kernarg_sha256: [u8; 32],
    kernarg_size: u64,
    kernarg_alignment: u64,
    explicit_kernarg_bytes: u64,
    implicit_kernarg_bytes: u64,
    static_group_segment: u32,
    private_segment: u32,
    grid: [u32; 3],
    workgroup: [u32; 3],
    dynamic_lds: u32,
    input_sha256: [u8; 32],
    output_sha256: [u8; 32],
    output_bits: u32,
    unload_runtime_instance: [u8; 16],
    unload_agent_handle: u64,
    unload_executable: [u8; 32],
    unload_released: bool,
}

#[derive(Clone, Copy)]
struct DispatchContractFactsV1 {
    kernel: [u8; 32],
    dispatch: [u8; 16],
    kernarg_sha256: [u8; 32],
    kernarg_size: u64,
    kernarg_alignment: u64,
    explicit_kernarg_bytes: u64,
    implicit_kernarg_bytes: u64,
    static_group_segment: u32,
    private_segment: u32,
    grid: [u32; 3],
    workgroup: [u32; 3],
    dynamic_lds: u32,
    input_sha256: [u8; 32],
    output_sha256: [u8; 32],
}

/// Consumes one repository-finalized receipt and executes its retained HSACO once.
///
/// This is the only public runtime entry point. It accepts no second execution,
/// byte slice, callback, payload, lane override, geometry, or LDS value. Exact
/// UUID and target validation occurs after the minimum context/environment
/// discovery required by current APIs and before any code-object load.
pub fn execute_repository_scalar_add_v1_on_mi300x(
    finalized: FinalizedRepositoryScalarAddV1,
) -> Result<RuntimeEvidenceV1, RuntimeExecutionErrorV1> {
    validate_process_lane()?;
    let authority = finalized.into_runtime_authority();
    validate_authority(&authority)?;
    let context = GpuContext::new(0).map_err(RuntimeExecutionErrorV1::Context)?;
    execute_with_adapter(authority, context)
}

fn validate_authority(
    authority: &RepositoryRuntimeAuthorityV1,
) -> Result<(), RuntimeExecutionErrorV1> {
    let output =
        authority
            .execution
            .response()
            .output()
            .ok_or(RuntimeExecutionErrorV1::Observation(
                RuntimeObservationFieldV1::FinalizedArtifact,
            ))?;
    let hsaco = output.bytes();
    let policy = &authority.policy;
    let observation = &authority.observation;
    if output.identity() != policy.output_identity()
        || observation.output_identity() != policy.output_identity()
        || !policy.output_identity().matches(hsaco)
        || authority.execution.worker_executable() != policy.worker_executable()
        || observation.worker_measurement().executable() != policy.worker_executable()
        || observation.worker_measurement().worker_build_identity()
            != policy.worker_build_identity()
        || observation.worker_measurement().llvm_build_identity() != policy.llvm_build_identity()
    {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::FinalizedArtifact,
        ));
    }
    Ok(())
}

fn execute_with_adapter(
    authority: RepositoryRuntimeAuthorityV1,
    context: Arc<GpuContext>,
) -> Result<RuntimeEvidenceV1, RuntimeExecutionErrorV1> {
    let output =
        authority
            .execution
            .response()
            .output()
            .ok_or(RuntimeExecutionErrorV1::Observation(
                RuntimeObservationFieldV1::FinalizedArtifact,
            ))?;
    let hsaco = output.bytes();
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    let environment = adapter.environment().clone();
    validate_environment(&environment, &authority)?;

    let digest = DigestAlgorithm::Sha256.calculate(hsaco);
    let hsaco_byte_len = u64::try_from(hsaco.len())
        .map_err(|_| RuntimeExecutionErrorV1::Internal("HSACO length does not fit u64"))?;
    let (executable, load) = {
        // SAFETY: the consumed finalization binds these exact bytes to the
        // checked-in repository policy. The adapter owns every native handle.
        unsafe { adapter.load_executable(hsaco, digest) }
    }?;
    let executable_identity = *load.executable_object().as_bytes();
    let expected_lifecycle = ExpectedLifecycleV1 {
        digest,
        byte_len: hsaco_byte_len,
        runtime: environment.runtime().instance(),
        agent: environment.agent().agent_handle(),
        executable: executable_identity,
    };

    let execution = (|| {
        validate_load(&load, expected_lifecycle)?;
        // SAFETY: the finalizer admits exactly `scalar_add` and its descriptor.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [PLIRON_SCALAR_ADD_V1_KERNEL]) }?;
        let kernel = kernels.get(0).ok_or(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::KernelIdentity,
        ))?;
        let resolution = &resolutions[0];
        let static_group_segment =
            u32::try_from(resolution.group_segment_size()).map_err(|_| {
                RuntimeExecutionErrorV1::Internal("static group segment does not fit u32")
            })?;
        let private_segment = u32::try_from(resolution.private_segment_size())
            .map_err(|_| RuntimeExecutionErrorV1::Internal("private segment does not fit u32"))?;

        validate_kernel(
            KernelFactsV1 {
                executable: *resolution.executable_object().as_bytes(),
                kernel: *resolution.kernel_object().as_bytes(),
                symbol_is_exact: resolution.export_symbol() == PLIRON_SCALAR_ADD_V1_KERNEL,
                kernarg_size: resolution.kernarg_segment_size(),
                kernarg_alignment: resolution.kernarg_segment_alignment(),
                static_group_segment,
                private_segment,
                resource_observation_is_exact: true,
            },
            executable_identity,
        )?;

        let input_before = guarded_scalar_bytes(INPUT_VALUE, INPUT_PREFIX, INPUT_SUFFIX);
        let output_before = guarded_scalar_bytes(OUTPUT_POISON, OUTPUT_PREFIX, OUTPUT_SUFFIX);
        let input = adapter.allocate_hardware_test_buffer(&input_before)?;
        let output = adapter.allocate_hardware_test_buffer(&output_before)?;
        let input_body = body_address(&input)?;
        let output_body = body_address(&output)?;
        validate_body_ranges(input_body, output_body)?;
        let explicit = explicit_kernarg(input_body, output_body, ADDEND);
        let geometry = HsaLaunchGeometryV1::new(GRID, WORKGROUP, DYNAMIC_LDS_BYTES);
        validate_geometry(geometry)?;

        let mut storage = AlignedKernarg([0; PLIRON_SCALAR_ADD_V1_KERNARG_BYTES as usize]);
        storage.0[..explicit.len()].copy_from_slice(&explicit);
        // SAFETY: the exact explicit ABI is initialized; both guarded buffers,
        // the complete aligned kernarg, and private handles outlive completion.
        let implicit = unsafe {
            adapter.initialize_implicit_kernarg(
                &executable,
                kernel,
                geometry,
                PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES as usize,
                PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES as usize,
                PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES as usize,
                &mut storage.0,
            )
        }?;
        if !implicit.initialized() || storage.0[..explicit.len()] != explicit {
            return Err(RuntimeExecutionErrorV1::Observation(
                RuntimeObservationFieldV1::ImplicitKernarg,
            ));
        }
        let kernarg_sha256 = <[u8; 32]>::from(Sha256::digest(storage.0));
        // SAFETY: the reviewed initializer bound this exact complete kernarg,
        // geometry, executable, and kernel. This call waits for quiescence.
        let completion =
            unsafe { adapter.launch_and_wait(&executable, kernel, geometry, &mut storage.0) }?;
        if !completion.completed()
            || completion.executable_object() != resolution.executable_object()
            || completion.kernel_object() != resolution.kernel_object()
            || completion.geometry() != geometry
        {
            return Err(RuntimeExecutionErrorV1::Observation(
                RuntimeObservationFieldV1::Dispatch,
            ));
        }
        if <[u8; 32]>::from(Sha256::digest(storage.0)) != kernarg_sha256 {
            return Err(RuntimeExecutionErrorV1::Observation(
                RuntimeObservationFieldV1::ImplicitKernarg,
            ));
        }

        let input_after = input.read_after_synchronous_dispatch();
        let output_after = output.read_after_synchronous_dispatch();
        validate_result(&input_before, &input_after, &output_after)?;
        Ok(DispatchContractFactsV1 {
            kernel: *resolution.kernel_object().as_bytes(),
            dispatch: completion.dispatch_identity(),
            kernarg_sha256,
            kernarg_size: resolution.kernarg_segment_size(),
            kernarg_alignment: resolution.kernarg_segment_alignment(),
            explicit_kernarg_bytes: PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES,
            implicit_kernarg_bytes: PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES,
            static_group_segment,
            private_segment,
            grid: geometry.grid(),
            workgroup: geometry.workgroup(),
            dynamic_lds: geometry.dynamic_shared_memory_bytes(),
            input_sha256: Sha256::digest(&input_after).into(),
            output_sha256: Sha256::digest(&output_after).into(),
        })
    })();

    // All kernel, buffer, and kernarg borrows/tokens ended inside the closure.
    // SAFETY: launch errors return only before publication or after quiescence;
    // this is the one consuming terminal unload for the exact executable.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    validate_unload(&unload, expected_lifecycle)?;
    let dispatch_contract = execution?;
    let facts = RuntimeEvidenceFactsV1 {
        finalization: *authority.identity.as_bytes(),
        lineage: *authority.lineage_identity.as_bytes(),
        approval: *authority.policy.identity().as_bytes(),
        observation: *authority.observation.identity().as_bytes(),
        load_sha256: *digest.bytes().as_bytes(),
        load_byte_len: hsaco_byte_len,
        runtime_instance: expected_lifecycle.runtime,
        runtime_image_sha256: *authority.policy.runtime_image_sha256(),
        device_uuid: environment.physical_device().uuid(),
        agent_handle: expected_lifecycle.agent,
        executable: expected_lifecycle.executable,
        kernel: dispatch_contract.kernel,
        dispatch: dispatch_contract.dispatch,
        kernarg_sha256: dispatch_contract.kernarg_sha256,
        kernarg_size: dispatch_contract.kernarg_size,
        kernarg_alignment: dispatch_contract.kernarg_alignment,
        explicit_kernarg_bytes: dispatch_contract.explicit_kernarg_bytes,
        implicit_kernarg_bytes: dispatch_contract.implicit_kernarg_bytes,
        static_group_segment: dispatch_contract.static_group_segment,
        private_segment: dispatch_contract.private_segment,
        grid: dispatch_contract.grid,
        workgroup: dispatch_contract.workgroup,
        dynamic_lds: dispatch_contract.dynamic_lds,
        input_sha256: dispatch_contract.input_sha256,
        output_sha256: dispatch_contract.output_sha256,
        output_bits: EXPECTED.to_bits(),
        unload_runtime_instance: unload.runtime_instance(),
        unload_agent_handle: unload.agent_handle(),
        unload_executable: *unload.executable_object().as_bytes(),
        unload_released: unload.released(),
    };
    let identity = calculate_runtime_evidence_identity(&facts);
    Ok(RuntimeEvidenceV1 {
        identity,
        finalization: authority.identity,
        lineage: authority.lineage_identity,
        approval: authority.policy.identity(),
        observation: authority.observation.identity(),
        load_sha256: facts.load_sha256,
        load_byte_len: facts.load_byte_len,
        runtime_instance: expected_lifecycle.runtime,
        runtime_image_sha256: facts.runtime_image_sha256,
        device_uuid: facts.device_uuid,
        agent_handle: expected_lifecycle.agent,
        executable: expected_lifecycle.executable,
        kernel: facts.kernel,
        dispatch: facts.dispatch,
        kernarg_sha256: facts.kernarg_sha256,
        kernarg_size: facts.kernarg_size,
        kernarg_alignment: facts.kernarg_alignment,
        explicit_kernarg_bytes: facts.explicit_kernarg_bytes,
        implicit_kernarg_bytes: facts.implicit_kernarg_bytes,
        static_group_segment: facts.static_group_segment,
        private_segment: facts.private_segment,
        grid: facts.grid,
        workgroup: facts.workgroup,
        dynamic_lds: facts.dynamic_lds,
        input_sha256: facts.input_sha256,
        output_sha256: facts.output_sha256,
        output_bits: EXPECTED.to_bits(),
        unload_runtime_instance: facts.unload_runtime_instance,
        unload_agent_handle: facts.unload_agent_handle,
        unload_executable: facts.unload_executable,
        unload_released: facts.unload_released,
    })
}

fn validate_process_lane() -> Result<(), RuntimeExecutionErrorV1> {
    let hsa = env::var_os("HSA_XNACK");
    let hip = env::var_os("HIP_VISIBLE_DEVICES");
    let rocr = env::var_os("ROCR_VISIBLE_DEVICES");
    validate_visibility(hsa.as_deref(), hip.as_deref(), rocr.as_deref())
}

fn validate_visibility(
    hsa_xnack: Option<&OsStr>,
    hip_visible_devices: Option<&OsStr>,
    rocr_visible_devices: Option<&OsStr>,
) -> Result<(), RuntimeExecutionErrorV1> {
    for (actual, expected, field) in [
        (
            hsa_xnack,
            REQUIRED_VISIBLE_DEVICE,
            RuntimeLaneFieldV1::HsaXnack,
        ),
        (
            hip_visible_devices,
            REQUIRED_VISIBLE_DEVICE,
            RuntimeLaneFieldV1::HipVisibleDevices,
        ),
        (
            rocr_visible_devices,
            REQUIRED_VISIBLE_DEVICE,
            RuntimeLaneFieldV1::RocrVisibleDevices,
        ),
    ] {
        if actual != Some(OsStr::new(expected)) {
            return Err(RuntimeExecutionErrorV1::Lane(field));
        }
    }
    Ok(())
}

fn validate_environment(
    environment: &HsaEnvironmentObservationV1,
    authority: &RepositoryRuntimeAuthorityV1,
) -> Result<(), RuntimeExecutionErrorV1> {
    let runtime_image_sha256 = *environment.runtime().image_digest().bytes().as_bytes();
    validate_lane_facts(LaneFactsV1 {
        runtime_implementation_is_exact: environment.runtime().implementation()
            == authority.policy.runtime_implementation()
            && environment.runtime().implementation() == REQUIRED_RUNTIME_IMPLEMENTATION,
        runtime_version_is_exact: environment.runtime().version()
            == authority.policy.runtime_version()
            && environment.runtime().version() == REQUIRED_RUNTIME_VERSION,
        runtime_image_sha256,
        runtime_image_is_policy_exact: runtime_image_sha256
            == *authority.policy.runtime_image_sha256(),
        device_uuid: environment.physical_device().uuid(),
        target: environment.physical_device().target(),
        hip_ordinal: environment.physical_device().hip_ordinal(),
        runtime: environment.runtime().instance(),
        agent_runtime: environment.agent().runtime_instance(),
        agent_uuid: environment.agent().physical_device_uuid(),
        agent_target: environment.agent().target(),
        agent_handle: environment.agent().agent_handle(),
    })
}

fn validate_lane_facts(facts: LaneFactsV1) -> Result<(), RuntimeExecutionErrorV1> {
    let target = AmdTargetId::parse(REQUIRED_OBSERVED_TARGET)
        .map_err(|_| RuntimeExecutionErrorV1::Internal("pinned MI300X target is invalid"))?;
    if !facts.runtime_implementation_is_exact
        || !facts.runtime_version_is_exact
        || !facts.runtime_image_is_policy_exact
        || facts.runtime_image_sha256 != REQUIRED_RUNTIME_IMAGE_SHA256
    {
        return Err(RuntimeExecutionErrorV1::Lane(
            RuntimeLaneFieldV1::RuntimeStack,
        ));
    }
    if facts.device_uuid != REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1 {
        return Err(RuntimeExecutionErrorV1::Lane(
            RuntimeLaneFieldV1::DeviceUuid,
        ));
    }
    if facts.target != target {
        return Err(RuntimeExecutionErrorV1::Lane(RuntimeLaneFieldV1::Target));
    }
    if facts.hip_ordinal != 0
        || facts.agent_runtime != facts.runtime
        || facts.agent_uuid != REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1
        || facts.agent_target != target
        || facts.agent_handle == 0
    {
        return Err(RuntimeExecutionErrorV1::Lane(RuntimeLaneFieldV1::Agent));
    }
    Ok(())
}

fn validate_load(
    load: &HsaCodeObjectLoadObservationV1,
    expected: ExpectedLifecycleV1,
) -> Result<(), RuntimeExecutionErrorV1> {
    validate_lifecycle(
        LifecycleFactsV1 {
            digest: load.finalized_digest(),
            byte_len: load.byte_len(),
            runtime: load.runtime_instance(),
            agent: load.agent_handle(),
            executable: *load.executable_object().as_bytes(),
            released: false,
        },
        expected,
        false,
    )
}

fn validate_unload(
    unload: &HsaUnloadObservationV1,
    expected: ExpectedLifecycleV1,
) -> Result<(), RuntimeExecutionErrorV1> {
    validate_lifecycle(
        LifecycleFactsV1 {
            digest: expected.digest,
            byte_len: expected.byte_len,
            runtime: unload.runtime_instance(),
            agent: unload.agent_handle(),
            executable: *unload.executable_object().as_bytes(),
            released: unload.released(),
        },
        expected,
        true,
    )
}

fn validate_lifecycle(
    actual: LifecycleFactsV1,
    expected: ExpectedLifecycleV1,
    is_unload: bool,
) -> Result<(), RuntimeExecutionErrorV1> {
    if actual.digest != expected.digest || actual.byte_len != expected.byte_len {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::LoadArtifact,
        ));
    }
    if actual.runtime != expected.runtime || actual.agent != expected.agent {
        return Err(RuntimeExecutionErrorV1::Observation(if is_unload {
            RuntimeObservationFieldV1::Unload
        } else {
            RuntimeObservationFieldV1::LoadEnvironment
        }));
    }
    if actual.executable != expected.executable || (is_unload && !actual.released) {
        return Err(RuntimeExecutionErrorV1::Observation(if is_unload {
            RuntimeObservationFieldV1::Unload
        } else {
            RuntimeObservationFieldV1::LoadArtifact
        }));
    }
    Ok(())
}

fn validate_kernel(
    facts: KernelFactsV1,
    expected_executable: [u8; 32],
) -> Result<(), RuntimeExecutionErrorV1> {
    if facts.executable != expected_executable
        || facts.kernel == [0; 32]
        || !facts.symbol_is_exact
        || !facts.resource_observation_is_exact
    {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::KernelIdentity,
        ));
    }
    if facts.kernarg_size != PLIRON_SCALAR_ADD_V1_KERNARG_BYTES {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::KernargSize,
        ));
    }
    if facts.kernarg_alignment != REQUIRED_RUNTIME_KERNARG_ALIGNMENT {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::KernargAlignment,
        ));
    }
    if facts.static_group_segment != 0 {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::StaticGroupSegment,
        ));
    }
    if facts.private_segment != 0 {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::PrivateSegment,
        ));
    }
    Ok(())
}

fn validate_geometry(geometry: HsaLaunchGeometryV1) -> Result<(), RuntimeExecutionErrorV1> {
    if geometry.grid() != GRID || geometry.workgroup() != WORKGROUP {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::Geometry,
        ));
    }
    if geometry.dynamic_shared_memory_bytes() != DYNAMIC_LDS_BYTES {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::DynamicLds,
        ));
    }
    Ok(())
}

fn guarded_scalar_bytes(value: f32, prefix: f32, suffix: f32) -> Vec<u8> {
    let mut values = Vec::with_capacity(CANARY_ELEMENTS * 2 + 1);
    values.resize(CANARY_ELEMENTS, prefix);
    values.push(value);
    values.resize(CANARY_ELEMENTS * 2 + 1, suffix);
    values.into_iter().flat_map(f32::to_ne_bytes).collect()
}

fn body_address(buffer: &ReviewedHsaHardwareTestBufferV1) -> Result<u64, RuntimeExecutionErrorV1> {
    let expected = (CANARY_ELEMENTS * 2 + 1) * core::mem::size_of::<f32>();
    if buffer.byte_len() != expected {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::AllocationExtent,
        ));
    }
    buffer
        .device_address(CANARY_ELEMENTS * core::mem::size_of::<f32>())
        .map_err(Into::into)
}

fn validate_body_ranges(input: u64, output: u64) -> Result<(), RuntimeExecutionErrorV1> {
    const BODY_BYTES: u64 = core::mem::size_of::<f32>() as u64;
    const BODY_ALIGN: u64 = core::mem::align_of::<f32>() as u64;
    let input_end = input.checked_add(BODY_BYTES);
    let output_end = output.checked_add(BODY_BYTES);
    if input == 0
        || output == 0
        || !input.is_multiple_of(BODY_ALIGN)
        || !output.is_multiple_of(BODY_ALIGN)
        || input_end.is_none()
        || output_end.is_none()
        || input < output_end.expect("checked above") && output < input_end.expect("checked above")
    {
        return Err(RuntimeExecutionErrorV1::Observation(
            RuntimeObservationFieldV1::KernelPointers,
        ));
    }
    Ok(())
}

fn explicit_kernarg(input: u64, output: u64, addend: f32) -> [u8; 24] {
    let mut bytes = [0; 24];
    bytes[0..8].copy_from_slice(&input.to_le_bytes());
    bytes[8..16].copy_from_slice(&output.to_le_bytes());
    bytes[16..20].copy_from_slice(&addend.to_bits().to_le_bytes());
    bytes
}

fn validate_result(
    input_before: &[u8],
    input_after: &[u8],
    output_after: &[u8],
) -> Result<(), RuntimeExecutionErrorV1> {
    let expected_bytes = (CANARY_ELEMENTS * 2 + 1) * core::mem::size_of::<f32>();
    if input_before.len() != expected_bytes
        || input_after.len() != expected_bytes
        || output_after.len() != expected_bytes
    {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::AllocationExtent,
        ));
    }
    if input_before != input_after {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::InputMutation,
        ));
    }
    let input = decode_f32(input_after)?;
    let output = decode_f32(output_after)?;
    if !bits_equal(&input[..CANARY_ELEMENTS], INPUT_PREFIX)
        || !bits_equal(&input[CANARY_ELEMENTS + 1..], INPUT_SUFFIX)
    {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::InputCanary,
        ));
    }
    if !bits_equal(&output[..CANARY_ELEMENTS], OUTPUT_PREFIX)
        || !bits_equal(&output[CANARY_ELEMENTS + 1..], OUTPUT_SUFFIX)
    {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::OutputCanary,
        ));
    }
    if output[CANARY_ELEMENTS].to_bits() != EXPECTED.to_bits()
        || (INPUT_VALUE + ADDEND).to_bits() != EXPECTED.to_bits()
    {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::OutputValue,
        ));
    }
    Ok(())
}

fn decode_f32(bytes: &[u8]) -> Result<Vec<f32>, RuntimeExecutionErrorV1> {
    if !bytes.len().is_multiple_of(core::mem::size_of::<f32>()) {
        return Err(RuntimeExecutionErrorV1::Result(
            RuntimeResultFieldV1::AllocationExtent,
        ));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect())
}

fn bits_equal(values: &[f32], expected: f32) -> bool {
    values
        .iter()
        .all(|value| value.to_bits() == expected.to_bits())
}

fn calculate_runtime_evidence_identity(
    facts: &RuntimeEvidenceFactsV1,
) -> RuntimeEvidenceIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(RUNTIME_EVIDENCE_IDENTITY_DOMAIN_V1);
    digest.update(facts.finalization);
    digest.update(facts.lineage);
    digest.update(facts.approval);
    digest.update(facts.observation);
    digest.update(facts.load_sha256);
    digest.update(facts.load_byte_len.to_le_bytes());
    digest.update(facts.runtime_instance);
    digest.update((REQUIRED_RUNTIME_IMPLEMENTATION.len() as u64).to_le_bytes());
    digest.update(REQUIRED_RUNTIME_IMPLEMENTATION.as_bytes());
    digest.update((REQUIRED_RUNTIME_VERSION.len() as u64).to_le_bytes());
    digest.update(REQUIRED_RUNTIME_VERSION.as_bytes());
    digest.update(facts.runtime_image_sha256);
    digest.update(facts.device_uuid);
    digest.update((REQUIRED_OBSERVED_TARGET.len() as u64).to_le_bytes());
    digest.update(REQUIRED_OBSERVED_TARGET.as_bytes());
    digest.update(facts.agent_handle.to_le_bytes());
    digest.update(facts.executable);
    digest.update(facts.kernel);
    digest.update(facts.dispatch);
    digest.update(facts.kernarg_sha256);
    digest.update(facts.kernarg_size.to_le_bytes());
    digest.update(PLIRON_SCALAR_ADD_V1_KERNARG_ALIGNMENT.to_le_bytes());
    digest.update(facts.kernarg_alignment.to_le_bytes());
    digest.update(facts.explicit_kernarg_bytes.to_le_bytes());
    digest.update(facts.implicit_kernarg_bytes.to_le_bytes());
    digest.update(facts.static_group_segment.to_le_bytes());
    digest.update(facts.private_segment.to_le_bytes());
    for dimension in facts.grid {
        digest.update(dimension.to_le_bytes());
    }
    for dimension in facts.workgroup {
        digest.update(dimension.to_le_bytes());
    }
    digest.update(facts.dynamic_lds.to_le_bytes());
    digest.update(facts.input_sha256);
    digest.update(facts.output_sha256);
    digest.update(facts.output_bits.to_le_bytes());
    digest.update((CANARY_ELEMENTS as u64).to_le_bytes());
    for value in [
        INPUT_VALUE,
        ADDEND,
        EXPECTED,
        INPUT_PREFIX,
        INPUT_SUFFIX,
        OUTPUT_PREFIX,
        OUTPUT_SUFFIX,
        OUTPUT_POISON,
    ] {
        digest.update(value.to_bits().to_le_bytes());
    }
    digest.update(facts.unload_runtime_instance);
    digest.update(facts.unload_agent_handle.to_le_bytes());
    digest.update(facts.unload_executable);
    digest.update([u8::from(facts.unload_released)]);
    digest.update((DEVICE_VALIDATION_SCOPE_V1.len() as u64).to_le_bytes());
    digest.update(DEVICE_VALIDATION_SCOPE_V1.as_bytes());
    RuntimeEvidenceIdentityV1(digest.finalize().into())
}

fn parse_success_marker_v1(
    marker: &str,
) -> Result<RuntimeEvidenceIdentityV1, RuntimeEvidenceMarkerErrorV1> {
    let tokens = marker.split(' ').collect::<Vec<_>>();
    if tokens.len() != 30 || tokens[0] != SUCCESS_MARKER_V1 {
        return Err(RuntimeEvidenceMarkerErrorV1::Structure);
    }
    let supplied = RuntimeEvidenceIdentityV1(parse_hex_array::<32>(
        marker_value(&tokens, 1, "evidence")?,
        "evidence",
    )?);
    let finalization =
        parse_hex_array::<32>(marker_value(&tokens, 2, "finalization")?, "finalization")?;
    let lineage = parse_hex_array::<32>(marker_value(&tokens, 3, "lineage")?, "lineage")?;
    let approval = parse_hex_array::<32>(marker_value(&tokens, 4, "approval")?, "approval")?;
    let observation =
        parse_hex_array::<32>(marker_value(&tokens, 5, "observation")?, "observation")?;
    let load_sha256 =
        parse_hex_array::<32>(marker_value(&tokens, 6, "load_sha256")?, "load_sha256")?;
    let load_byte_len = parse_canonical_u64(marker_value(&tokens, 7, "load_bytes")?, "load_bytes")?;
    let runtime_instance = parse_hex_array::<16>(marker_value(&tokens, 8, "runtime")?, "runtime")?;
    if marker_value(&tokens, 9, "runtime_version")? != REQUIRED_RUNTIME_VERSION {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("runtime_version"));
    }
    let runtime_image_sha256 = parse_hex_array::<32>(
        marker_value(&tokens, 10, "runtime_image_sha256")?,
        "runtime_image_sha256",
    )?;
    if runtime_image_sha256 != REQUIRED_RUNTIME_IMAGE_SHA256 {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("runtime_image_sha256"));
    }
    let device_uuid = parse_hex_array::<16>(marker_value(&tokens, 11, "device")?, "device")?;
    if device_uuid != REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1 {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("device"));
    }
    if marker_value(&tokens, 12, "target")? != REQUIRED_OBSERVED_TARGET {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("target"));
    }
    let agent_handle = parse_fixed_hex_u64(marker_value(&tokens, 13, "agent")?, 16, "agent")?;
    let executable = parse_hex_array::<32>(marker_value(&tokens, 14, "executable")?, "executable")?;
    let kernel = parse_hex_array::<32>(marker_value(&tokens, 15, "kernel")?, "kernel")?;
    let dispatch = parse_hex_array::<16>(marker_value(&tokens, 16, "dispatch")?, "dispatch")?;
    let kernarg_sha256 = parse_hex_array::<32>(
        marker_value(&tokens, 17, "kernarg_sha256")?,
        "kernarg_sha256",
    )?;
    if marker_value(&tokens, 18, "kernarg")? != "24,256,280,8,16" {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("kernarg"));
    }
    if marker_value(&tokens, 19, "segments")? != "0,0,0" {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("segments"));
    }
    if marker_value(&tokens, 20, "grid")? != "1,1,1" {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("grid"));
    }
    if marker_value(&tokens, 21, "workgroup")? != "1,1,1" {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("workgroup"));
    }
    let input_sha256 =
        parse_hex_array::<32>(marker_value(&tokens, 22, "input_sha256")?, "input_sha256")?;
    let output_sha256 =
        parse_hex_array::<32>(marker_value(&tokens, 23, "output_sha256")?, "output_sha256")?;
    let output_bits = u32::try_from(parse_fixed_hex_u64(
        marker_value(&tokens, 24, "output_bits")?,
        8,
        "output_bits",
    )?)
    .map_err(|_| RuntimeEvidenceMarkerErrorV1::Field("output_bits"))?;
    if output_bits != EXPECTED.to_bits() {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("output_bits"));
    }
    let unload_runtime_instance = parse_hex_array::<16>(
        marker_value(&tokens, 25, "unload_runtime")?,
        "unload_runtime",
    )?;
    let unload_agent_handle = parse_fixed_hex_u64(
        marker_value(&tokens, 26, "unload_agent")?,
        16,
        "unload_agent",
    )?;
    let unload_executable = parse_hex_array::<32>(
        marker_value(&tokens, 27, "unload_executable")?,
        "unload_executable",
    )?;
    if marker_value(&tokens, 28, "unload_released")? != "true" {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("unload_released"));
    }
    if marker_value(&tokens, 29, "device_validation")? != DEVICE_VALIDATION_SCOPE_V1 {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("device_validation"));
    }
    if unload_runtime_instance != runtime_instance {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("unload_runtime"));
    }
    if unload_agent_handle != agent_handle {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("unload_agent"));
    }
    if unload_executable != executable {
        return Err(RuntimeEvidenceMarkerErrorV1::Field("unload_executable"));
    }
    let calculated = calculate_runtime_evidence_identity(&RuntimeEvidenceFactsV1 {
        finalization,
        lineage,
        approval,
        observation,
        load_sha256,
        load_byte_len,
        runtime_instance,
        runtime_image_sha256,
        device_uuid,
        agent_handle,
        executable,
        kernel,
        dispatch,
        kernarg_sha256,
        kernarg_size: PLIRON_SCALAR_ADD_V1_KERNARG_BYTES,
        kernarg_alignment: REQUIRED_RUNTIME_KERNARG_ALIGNMENT,
        explicit_kernarg_bytes: PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES,
        implicit_kernarg_bytes: PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES,
        static_group_segment: 0,
        private_segment: 0,
        grid: GRID,
        workgroup: WORKGROUP,
        dynamic_lds: DYNAMIC_LDS_BYTES,
        input_sha256,
        output_sha256,
        output_bits,
        unload_runtime_instance,
        unload_agent_handle,
        unload_executable,
        unload_released: true,
    });
    if supplied != calculated {
        return Err(RuntimeEvidenceMarkerErrorV1::Identity);
    }
    Ok(supplied)
}

fn marker_value<'a>(
    tokens: &[&'a str],
    index: usize,
    key: &'static str,
) -> Result<&'a str, RuntimeEvidenceMarkerErrorV1> {
    tokens
        .get(index)
        .and_then(|token| token.strip_prefix(key))
        .and_then(|value| value.strip_prefix('='))
        .filter(|value| !value.is_empty() && !value.contains('='))
        .ok_or(RuntimeEvidenceMarkerErrorV1::Structure)
}

fn parse_hex_array<const N: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; N], RuntimeEvidenceMarkerErrorV1> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeEvidenceMarkerErrorV1::Field(field));
    }
    let mut output = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] =
            (lower_hex_nibble(pair[0], field)? << 4) | lower_hex_nibble(pair[1], field)?;
    }
    Ok(output)
}

fn lower_hex_nibble(byte: u8, field: &'static str) -> Result<u8, RuntimeEvidenceMarkerErrorV1> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(RuntimeEvidenceMarkerErrorV1::Field(field)),
    }
}

fn parse_fixed_hex_u64(
    value: &str,
    digits: usize,
    field: &'static str,
) -> Result<u64, RuntimeEvidenceMarkerErrorV1> {
    if value.len() != digits {
        return Err(RuntimeEvidenceMarkerErrorV1::Field(field));
    }
    value.bytes().try_fold(0_u64, |parsed, byte| {
        Ok((parsed << 4) | u64::from(lower_hex_nibble(byte, field)?))
    })
}

fn parse_canonical_u64(
    value: &str,
    field: &'static str,
) -> Result<u64, RuntimeEvidenceMarkerErrorV1> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| RuntimeEvidenceMarkerErrorV1::Field(field))?;
    if parsed == 0 || parsed.to_string() != value {
        return Err(RuntimeEvidenceMarkerErrorV1::Field(field));
    }
    Ok(parsed)
}

fn hex_bytes(bytes: &[u8]) -> String {
    use fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_host::HsaExecutableObjectIdentityV1;

    fn exact_target() -> AmdTargetId {
        AmdTargetId::parse(REQUIRED_OBSERVED_TARGET).unwrap()
    }

    fn lane_facts() -> LaneFactsV1 {
        LaneFactsV1 {
            runtime_implementation_is_exact: true,
            runtime_version_is_exact: true,
            runtime_image_sha256: REQUIRED_RUNTIME_IMAGE_SHA256,
            runtime_image_is_policy_exact: true,
            device_uuid: REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1,
            target: exact_target(),
            hip_ordinal: 0,
            runtime: [5; 16],
            agent_runtime: [5; 16],
            agent_uuid: REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1,
            agent_target: exact_target(),
            agent_handle: 7,
        }
    }

    fn kernel_facts() -> KernelFactsV1 {
        KernelFactsV1 {
            executable: [1; 32],
            kernel: [2; 32],
            symbol_is_exact: true,
            kernarg_size: 280,
            kernarg_alignment: REQUIRED_RUNTIME_KERNARG_ALIGNMENT,
            static_group_segment: 0,
            private_segment: 0,
            resource_observation_is_exact: true,
        }
    }

    fn expected_lifecycle() -> ExpectedLifecycleV1 {
        ExpectedLifecycleV1 {
            digest: DigestAlgorithm::Sha256.calculate(b"exact hsaco"),
            byte_len: 11,
            runtime: [3; 16],
            agent: 7,
            executable: [4; 32],
        }
    }

    fn runtime_facts() -> RuntimeEvidenceFactsV1 {
        RuntimeEvidenceFactsV1 {
            finalization: [1; 32],
            lineage: [2; 32],
            approval: [3; 32],
            observation: [4; 32],
            load_sha256: [5; 32],
            load_byte_len: 4_984,
            runtime_instance: [6; 16],
            runtime_image_sha256: REQUIRED_RUNTIME_IMAGE_SHA256,
            device_uuid: REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1,
            agent_handle: 7,
            executable: [8; 32],
            kernel: [9; 32],
            dispatch: [10; 16],
            kernarg_sha256: [13; 32],
            kernarg_size: PLIRON_SCALAR_ADD_V1_KERNARG_BYTES,
            kernarg_alignment: REQUIRED_RUNTIME_KERNARG_ALIGNMENT,
            explicit_kernarg_bytes: PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES,
            implicit_kernarg_bytes: PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES,
            static_group_segment: 0,
            private_segment: 0,
            grid: GRID,
            workgroup: WORKGROUP,
            dynamic_lds: DYNAMIC_LDS_BYTES,
            input_sha256: [11; 32],
            output_sha256: [12; 32],
            output_bits: EXPECTED.to_bits(),
            unload_runtime_instance: [6; 16],
            unload_agent_handle: 7,
            unload_executable: [8; 32],
            unload_released: true,
        }
    }

    fn runtime_evidence() -> RuntimeEvidenceV1 {
        let facts = runtime_facts();
        RuntimeEvidenceV1 {
            identity: calculate_runtime_evidence_identity(&facts),
            finalization: FinalizedRepositoryScalarAddV1Identity::from_digest(facts.finalization),
            lineage: ScalarAddLineageIdentityV1::from_digest(facts.lineage),
            approval: RepositoryApprovalIdentityV1::from_manifest_digest(facts.approval),
            observation: ScalarAddObservationIdentityV1::from_digest(facts.observation),
            load_sha256: facts.load_sha256,
            load_byte_len: facts.load_byte_len,
            runtime_instance: facts.runtime_instance,
            runtime_image_sha256: facts.runtime_image_sha256,
            device_uuid: facts.device_uuid,
            agent_handle: facts.agent_handle,
            executable: facts.executable,
            kernel: facts.kernel,
            dispatch: facts.dispatch,
            kernarg_sha256: facts.kernarg_sha256,
            kernarg_size: facts.kernarg_size,
            kernarg_alignment: facts.kernarg_alignment,
            explicit_kernarg_bytes: facts.explicit_kernarg_bytes,
            implicit_kernarg_bytes: facts.implicit_kernarg_bytes,
            static_group_segment: facts.static_group_segment,
            private_segment: facts.private_segment,
            grid: facts.grid,
            workgroup: facts.workgroup,
            dynamic_lds: facts.dynamic_lds,
            input_sha256: facts.input_sha256,
            output_sha256: facts.output_sha256,
            output_bits: facts.output_bits,
            unload_runtime_instance: facts.unload_runtime_instance,
            unload_agent_handle: facts.unload_agent_handle,
            unload_executable: facts.unload_executable,
            unload_released: facts.unload_released,
        }
    }

    fn load_observation(expected: ExpectedLifecycleV1) -> HsaCodeObjectLoadObservationV1 {
        HsaCodeObjectLoadObservationV1::new(
            expected.digest,
            expected.byte_len,
            expected.runtime,
            expected.agent,
            HsaExecutableObjectIdentityV1::new(expected.executable).unwrap(),
        )
    }

    fn unload_observation(expected: ExpectedLifecycleV1) -> HsaUnloadObservationV1 {
        HsaUnloadObservationV1::new(
            HsaExecutableObjectIdentityV1::new(expected.executable).unwrap(),
            expected.runtime,
            expected.agent,
            true,
        )
    }

    #[test]
    fn visibility_is_exact_and_complete_before_context_creation() {
        let zero = OsStr::new("0");
        validate_visibility(Some(zero), Some(zero), Some(zero)).unwrap();
        let cases = [
            (None, Some(zero), Some(zero), RuntimeLaneFieldV1::HsaXnack),
            (
                Some(OsStr::new("1")),
                Some(zero),
                Some(zero),
                RuntimeLaneFieldV1::HsaXnack,
            ),
            (
                Some(zero),
                None,
                Some(zero),
                RuntimeLaneFieldV1::HipVisibleDevices,
            ),
            (
                Some(zero),
                Some(zero),
                Some(OsStr::new("0,1")),
                RuntimeLaneFieldV1::RocrVisibleDevices,
            ),
        ];
        for (hsa, hip, rocr, expected) in cases {
            assert!(matches!(
                validate_visibility(hsa, hip, rocr),
                Err(RuntimeExecutionErrorV1::Lane(actual)) if actual == expected
            ));
        }
    }

    #[test]
    fn lane_rejects_uuid_target_ordinal_runtime_and_agent_substitutions() {
        validate_lane_facts(lane_facts()).unwrap();
        for mutate in 0..10 {
            let mut facts = lane_facts();
            let expected = match mutate {
                0 => {
                    facts.runtime_implementation_is_exact = false;
                    RuntimeLaneFieldV1::RuntimeStack
                }
                1 => {
                    facts.runtime_version_is_exact = false;
                    RuntimeLaneFieldV1::RuntimeStack
                }
                2 => {
                    facts.runtime_image_sha256[0] ^= 1;
                    RuntimeLaneFieldV1::RuntimeStack
                }
                3 => {
                    facts.runtime_image_is_policy_exact = false;
                    RuntimeLaneFieldV1::RuntimeStack
                }
                4 => {
                    facts.device_uuid[0] ^= 1;
                    RuntimeLaneFieldV1::DeviceUuid
                }
                5 => {
                    facts.target = AmdTargetId::parse("gfx942:xnack-").unwrap();
                    RuntimeLaneFieldV1::Target
                }
                6 => {
                    facts.hip_ordinal = 1;
                    RuntimeLaneFieldV1::Agent
                }
                7 => {
                    facts.agent_runtime[0] ^= 1;
                    RuntimeLaneFieldV1::Agent
                }
                8 => {
                    facts.agent_uuid[0] ^= 1;
                    RuntimeLaneFieldV1::Agent
                }
                9 => {
                    facts.agent_handle = 0;
                    RuntimeLaneFieldV1::Agent
                }
                _ => unreachable!(),
            };
            assert!(matches!(
                validate_lane_facts(facts),
                Err(RuntimeExecutionErrorV1::Lane(actual)) if actual == expected
            ));
        }
    }

    #[test]
    fn exact_kernel_contract_accepts_only_280_bytes_aligned_to_sixteen() {
        validate_kernel(kernel_facts(), [1; 32]).unwrap();
        for size in [279, 281] {
            let mut facts = kernel_facts();
            facts.kernarg_size = size;
            assert!(matches!(
                validate_kernel(facts, [1; 32]),
                Err(RuntimeExecutionErrorV1::Observation(
                    RuntimeObservationFieldV1::KernargSize
                ))
            ));
        }
        for alignment in [1, 4, 8, 32] {
            let mut facts = kernel_facts();
            facts.kernarg_alignment = alignment;
            assert!(matches!(
                validate_kernel(facts, [1; 32]),
                Err(RuntimeExecutionErrorV1::Observation(
                    RuntimeObservationFieldV1::KernargAlignment
                ))
            ));
        }
    }

    #[test]
    fn exact_kernel_contract_rejects_resource_and_identity_substitutions() {
        let mut cases = Vec::new();
        let mut group = kernel_facts();
        group.static_group_segment = 1;
        cases.push((group, RuntimeObservationFieldV1::StaticGroupSegment));
        let mut private = kernel_facts();
        private.private_segment = 1;
        cases.push((private, RuntimeObservationFieldV1::PrivateSegment));
        let mut symbol = kernel_facts();
        symbol.symbol_is_exact = false;
        cases.push((symbol, RuntimeObservationFieldV1::KernelIdentity));
        let mut resource_binding = kernel_facts();
        resource_binding.resource_observation_is_exact = false;
        cases.push((resource_binding, RuntimeObservationFieldV1::KernelIdentity));
        let mut kernel = kernel_facts();
        kernel.kernel = [0; 32];
        cases.push((kernel, RuntimeObservationFieldV1::KernelIdentity));

        for (facts, expected) in cases {
            assert!(matches!(
                validate_kernel(facts, [1; 32]),
                Err(RuntimeExecutionErrorV1::Observation(field)) if field == expected
            ));
        }
        assert!(matches!(
            validate_kernel(kernel_facts(), [9; 32]),
            Err(RuntimeExecutionErrorV1::Observation(
                RuntimeObservationFieldV1::KernelIdentity
            ))
        ));
    }

    #[test]
    fn exact_geometry_rejects_every_nonunit_or_lds_substitution() {
        validate_geometry(HsaLaunchGeometryV1::new(GRID, WORKGROUP, 0)).unwrap();
        for geometry in [
            HsaLaunchGeometryV1::new([2, 1, 1], WORKGROUP, 0),
            HsaLaunchGeometryV1::new(GRID, [2, 1, 1], 0),
            HsaLaunchGeometryV1::new(GRID, WORKGROUP, 1),
        ] {
            assert!(validate_geometry(geometry).is_err());
        }
    }

    #[test]
    fn body_ranges_require_nonzero_aligned_disjoint_nonoverflowing_f32_storage() {
        validate_body_ranges(0x1000, 0x1004).unwrap();
        for (input, output) in [
            (0, 0x1004),
            (0x1000, 0),
            (0x1001, 0x1008),
            (0x1000, 0x1006),
            (0x1000, 0x1000),
            (u64::MAX - 3, 0x1000),
            (0x1000, u64::MAX - 3),
        ] {
            assert!(matches!(
                validate_body_ranges(input, output),
                Err(RuntimeExecutionErrorV1::Observation(
                    RuntimeObservationFieldV1::KernelPointers
                ))
            ));
        }
    }

    #[test]
    fn load_and_unload_bind_digest_runtime_agent_and_executable() {
        let expected = expected_lifecycle();
        validate_load(&load_observation(expected), expected).unwrap();
        validate_unload(&unload_observation(expected), expected).unwrap();

        let mut facts = LifecycleFactsV1 {
            digest: DigestAlgorithm::Sha256.calculate(b"substitute"),
            byte_len: expected.byte_len,
            runtime: expected.runtime,
            agent: expected.agent,
            executable: expected.executable,
            released: false,
        };
        let mut load_cases = vec![(facts, RuntimeObservationFieldV1::LoadArtifact)];
        facts.digest = expected.digest;
        facts.byte_len += 1;
        load_cases.push((facts, RuntimeObservationFieldV1::LoadArtifact));
        facts.byte_len = expected.byte_len;
        facts.runtime[0] ^= 1;
        load_cases.push((facts, RuntimeObservationFieldV1::LoadEnvironment));
        facts.runtime = expected.runtime;
        facts.agent += 1;
        load_cases.push((facts, RuntimeObservationFieldV1::LoadEnvironment));
        facts.agent = expected.agent;
        facts.executable[0] ^= 1;
        load_cases.push((facts, RuntimeObservationFieldV1::LoadArtifact));
        for (actual, field) in load_cases {
            assert!(matches!(
                validate_lifecycle(actual, expected, false),
                Err(RuntimeExecutionErrorV1::Observation(actual)) if actual == field
            ));
        }

        for mutate in 0..4 {
            let mut unload = LifecycleFactsV1 {
                digest: expected.digest,
                byte_len: expected.byte_len,
                runtime: expected.runtime,
                agent: expected.agent,
                executable: expected.executable,
                released: true,
            };
            match mutate {
                0 => unload.runtime[0] ^= 1,
                1 => unload.agent += 1,
                2 => unload.executable[0] ^= 1,
                3 => unload.released = false,
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_lifecycle(unload, expected, true),
                Err(RuntimeExecutionErrorV1::Observation(
                    RuntimeObservationFieldV1::Unload
                ))
            ));
        }
    }

    #[test]
    fn guarded_scalar_checks_input_both_canary_sides_and_exact_output() {
        let input = guarded_scalar_bytes(INPUT_VALUE, INPUT_PREFIX, INPUT_SUFFIX);
        let output = guarded_scalar_bytes(EXPECTED, OUTPUT_PREFIX, OUTPUT_SUFFIX);
        validate_result(&input, &input, &output).unwrap();

        let mut changed_input = input.clone();
        changed_input[CANARY_ELEMENTS * 4] ^= 1;
        assert!(matches!(
            validate_result(&input, &changed_input, &output),
            Err(RuntimeExecutionErrorV1::Result(
                RuntimeResultFieldV1::InputMutation
            ))
        ));
        for index in [0, (CANARY_ELEMENTS * 2) * 4] {
            let mut changed = output.clone();
            changed[index] ^= 1;
            assert!(matches!(
                validate_result(&input, &input, &changed),
                Err(RuntimeExecutionErrorV1::Result(
                    RuntimeResultFieldV1::OutputCanary
                ))
            ));
        }
        let wrong_output = guarded_scalar_bytes(3.5, OUTPUT_PREFIX, OUTPUT_SUFFIX);
        assert!(matches!(
            validate_result(&input, &input, &wrong_output),
            Err(RuntimeExecutionErrorV1::Result(
                RuntimeResultFieldV1::OutputValue
            ))
        ));
    }

    #[test]
    fn pinned_lane_and_abi_are_exact() {
        assert_eq!(
            QUALIFIED_MI300X_HSA_UUID_OBSERVATION_V1,
            "GPU-6ced1647a296545c"
        );
        assert_eq!(REQUIRED_OBSERVED_TARGET, "gfx942:sramecc+:xnack-");
        assert_eq!(
            REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1,
            [
                0x56, 0x0d, 0xc9, 0x31, 0x39, 0xd4, 0xa1, 0x25, 0xd1, 0x82, 0x31, 0x6c, 0x74, 0x55,
                0x64, 0x5b,
            ]
        );
        assert_eq!(core::mem::align_of::<AlignedKernarg>(), 16);
        assert_eq!(core::mem::size_of::<AlignedKernarg>(), 288);
        assert_eq!(
            AlignedKernarg([0; PLIRON_SCALAR_ADD_V1_KERNARG_BYTES as usize])
                .0
                .len(),
            280
        );
        assert_eq!(PLIRON_SCALAR_ADD_V1_EXPLICIT_KERNARG_BYTES, 24);
        assert_eq!(PLIRON_SCALAR_ADD_V1_IMPLICIT_KERNARG_BYTES, 256);
        assert_eq!(
            DEVICE_VALIDATION_SCOPE_V1,
            "after_minimal_context_before_code_load"
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockPostLoadStageV1 {
        LoadValidation,
        Resolve,
        ResourceObservation,
        InputAllocation,
        OutputAllocation,
        PointerValidation,
        ImplicitKernarg,
        QuiescedDispatch,
        ResultValidation,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockLifecycleStateV1 {
        Loaded,
        Quiesced,
        Unloaded,
    }

    struct MockLifecycleV1 {
        state: MockLifecycleStateV1,
        unloads: usize,
    }

    impl MockLifecycleV1 {
        fn complete_stage(&mut self, stage: MockPostLoadStageV1) {
            assert_ne!(self.state, MockLifecycleStateV1::Unloaded);
            if stage == MockPostLoadStageV1::QuiescedDispatch {
                self.state = MockLifecycleStateV1::Quiesced;
            }
        }

        fn terminal_unload(&mut self) {
            assert_ne!(self.state, MockLifecycleStateV1::Unloaded);
            self.unloads += 1;
            self.state = MockLifecycleStateV1::Unloaded;
        }
    }

    fn simulate_post_load_lifecycle(
        fail_at: Option<MockPostLoadStageV1>,
    ) -> (usize, MockLifecycleStateV1, bool) {
        let stages = [
            MockPostLoadStageV1::LoadValidation,
            MockPostLoadStageV1::Resolve,
            MockPostLoadStageV1::ResourceObservation,
            MockPostLoadStageV1::InputAllocation,
            MockPostLoadStageV1::OutputAllocation,
            MockPostLoadStageV1::PointerValidation,
            MockPostLoadStageV1::ImplicitKernarg,
            MockPostLoadStageV1::QuiescedDispatch,
            MockPostLoadStageV1::ResultValidation,
        ];
        let mut lifecycle = MockLifecycleV1 {
            state: MockLifecycleStateV1::Loaded,
            unloads: 0,
        };
        let mut completed = true;
        for stage in stages {
            if fail_at == Some(stage) {
                completed = false;
                break;
            }
            lifecycle.complete_stage(stage);
        }
        lifecycle.terminal_unload();
        (lifecycle.unloads, lifecycle.state, completed)
    }

    #[test]
    fn every_recoverable_post_load_path_performs_exactly_one_terminal_unload() {
        for stage in [
            MockPostLoadStageV1::LoadValidation,
            MockPostLoadStageV1::Resolve,
            MockPostLoadStageV1::ResourceObservation,
            MockPostLoadStageV1::InputAllocation,
            MockPostLoadStageV1::OutputAllocation,
            MockPostLoadStageV1::PointerValidation,
            MockPostLoadStageV1::ImplicitKernarg,
            MockPostLoadStageV1::QuiescedDispatch,
            MockPostLoadStageV1::ResultValidation,
        ] {
            assert_eq!(
                simulate_post_load_lifecycle(Some(stage)),
                (1, MockLifecycleStateV1::Unloaded, false)
            );
        }
        assert_eq!(
            simulate_post_load_lifecycle(None),
            (1, MockLifecycleStateV1::Unloaded, true)
        );
    }

    #[test]
    fn aggregate_identity_rejects_every_runtime_transcript_substitution() {
        let expected = runtime_facts();
        let identity = calculate_runtime_evidence_identity(&expected);
        for field in 0..30 {
            let mut hostile = expected;
            match field {
                0 => hostile.finalization[0] ^= 1,
                1 => hostile.lineage[0] ^= 1,
                2 => hostile.approval[0] ^= 1,
                3 => hostile.observation[0] ^= 1,
                4 => hostile.load_sha256[0] ^= 1,
                5 => hostile.load_byte_len += 1,
                6 => hostile.runtime_instance[0] ^= 1,
                7 => hostile.runtime_image_sha256[0] ^= 1,
                8 => hostile.device_uuid[0] ^= 1,
                9 => hostile.agent_handle += 1,
                10 => hostile.executable[0] ^= 1,
                11 => hostile.kernel[0] ^= 1,
                12 => hostile.dispatch[0] ^= 1,
                13 => hostile.kernarg_sha256[0] ^= 1,
                14 => hostile.kernarg_size += 1,
                15 => hostile.kernarg_alignment += 1,
                16 => hostile.explicit_kernarg_bytes += 1,
                17 => hostile.implicit_kernarg_bytes += 1,
                18 => hostile.static_group_segment += 1,
                19 => hostile.private_segment += 1,
                20 => hostile.grid[0] += 1,
                21 => hostile.workgroup[0] += 1,
                22 => hostile.dynamic_lds += 1,
                23 => hostile.input_sha256[0] ^= 1,
                24 => hostile.output_sha256[0] ^= 1,
                25 => hostile.output_bits ^= 1,
                26 => hostile.unload_runtime_instance[0] ^= 1,
                27 => hostile.unload_agent_handle += 1,
                28 => hostile.unload_executable[0] ^= 1,
                29 => hostile.unload_released = false,
                _ => unreachable!(),
            }
            assert_ne!(calculate_runtime_evidence_identity(&hostile), identity);
        }
    }

    #[test]
    fn success_marker_round_trips_and_parses_every_fixed_order_field() {
        let evidence = runtime_evidence();
        let marker = evidence.success_marker_v1();
        assert_eq!(
            RuntimeEvidenceV1::parse_success_marker_v1(&marker).unwrap(),
            evidence.identity()
        );
        let tokens = marker.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(tokens.len(), 30);
        for index in 0..tokens.len() {
            let mut hostile = tokens.clone();
            hostile[index] = if index == 0 {
                "WRONG".to_owned()
            } else {
                let key = hostile[index].split_once('=').unwrap().0;
                format!("{key}=x")
            };
            assert!(RuntimeEvidenceV1::parse_success_marker_v1(&hostile.join(" ")).is_err());
        }
    }

    #[test]
    fn success_marker_rejects_aggregate_identity_substitution() {
        let marker = runtime_evidence().success_marker_v1();
        let hostile = marker.replacen("evidence=", "evidence=00", 1);
        assert!(RuntimeEvidenceV1::parse_success_marker_v1(&hostile).is_err());
    }

    #[test]
    fn success_marker_rejects_self_consistent_cross_field_substitutions() {
        for mutate in 0..3 {
            let mut facts = runtime_facts();
            match mutate {
                0 => facts.unload_runtime_instance[0] ^= 1,
                1 => facts.unload_agent_handle += 1,
                2 => facts.unload_executable[0] ^= 1,
                _ => unreachable!(),
            }
            let mut evidence = runtime_evidence();
            evidence.identity = calculate_runtime_evidence_identity(&facts);
            evidence.unload_runtime_instance = facts.unload_runtime_instance;
            evidence.unload_agent_handle = facts.unload_agent_handle;
            evidence.unload_executable = facts.unload_executable;
            assert!(
                RuntimeEvidenceV1::parse_success_marker_v1(&evidence.success_marker_v1()).is_err()
            );
        }
    }

    #[test]
    fn bounded_evidence_claims_no_general_property_or_parity() {
        let evidence = runtime_evidence();
        assert!(!evidence.claims_general_memory_safety());
        assert!(!evidence.claims_general_race_freedom());
        assert!(!evidence.claims_cuda_oxide_parity());
    }
}
