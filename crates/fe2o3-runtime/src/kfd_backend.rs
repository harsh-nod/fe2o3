//! Pure-Rust KFD implementation of the backend-neutral runtime SPI.
//!
//! The admitted gfx942 KFD surface owns explicit process VMs and native queues.
//! The single-device adapter owns a bounded set of independent compute queues
//! and directional SDMA queues. The separate two-device adapter retains exact
//! directional XGMI routes for copy-only peer execution. Atomic and collective
//! execution is fail-closed unless a separate unsafe authority enumerates and
//! authorizes the exact semantic contract carried by each launch.

use core::fmt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fe2o3_amdhsa_loader::{
    AdmittedProfile, KernelGlobalBufferAbiV1, OwnedValidatedEnvelope, OwnedValidatedKernelEnvelope,
    ValidatedKernelEnvelope, validate_owned,
};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_hsaco::{ArgumentAccess, ExplicitValueKind};
use fe2o3_kfd::topology::Gfx942XgmiRouteV1;
use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, ComputeAqlQueueLaneDispatchV1, ComputeAqlQueueLaneV1,
    ComputeAqlQueueSessionV1, DeviceSelector, GFX942_MAX_FIXED_DISPATCH_DATA_V1,
    GFX942_SDMA_MAX_IN_FLIGHT_V1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
    Gfx942CompletedDispatchReadRequestV1, Gfx942DeviceContentDescriptorV1,
    Gfx942DeviceContentRoleV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryUnmappedV1,
    Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaTerminalCustodyV1, Gfx942DispatchBatchV1,
    Gfx942DispatchBufferBindingV1, Gfx942DispatchPollV1, Gfx942FixedDispatchDataV1,
    Gfx942FixedDispatchPacketV1, Gfx942NativeXgmiSdmaQueueV1, Gfx942PersistentSdmaDirectionV1,
    Gfx942RecycledDispatchWriteRequestV1, Gfx942SdmaBufferV1, Gfx942SdmaCopyTicketV1,
    Gfx942SdmaMemoryPoolObservationV1, Gfx942XgmiBatchSubmissionFailureV1, Gfx942XgmiCopyFailureV1,
    Gfx942XgmiCopyPollV1, Gfx942XgmiMapRecoveryV1, Gfx942XgmiMappedDeviceMemoryV1,
    Gfx942XgmiSdmaCopyRequestV1, Gfx942XgmiUnmapRecoveryV1, HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
    OpenedKfd, SharedGttMemorySessionV1,
};
use fe2o3_profiler_protocol::{
    KfdProfileAccessV1, KfdProfileAtomicContractV1, KfdProfileAtomicOperationV1,
    KfdProfileBindingV1, KfdProfileCollectiveContractV1, KfdProfileCollectiveOperationV1,
    KfdProfileHostContentV1, KfdProfileHostTimingV1, KfdProfileLaunchV1, KfdProfileMemoryKindV1,
    KfdProfileMemoryOrderV1, KfdProfileMemoryScopeV1, KfdProfileResourceKindV1,
    KfdProfileSemanticContractV1, KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1,
    ProfileContentIdentityV1, ProfileIdentityV1,
};
use sha2::{Digest, Sha256};

use crate::{
    AuthenticatedKfdRuntimeDispatchTimestampsV1, AuthenticatedKfdRuntimeDispatchTimestampsV2,
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, BackendSemanticLaunchV1, KfdRuntimeProfileRecorderV1,
    KfdRuntimeProfileWithSemanticSidecarV1, KfdRuntimeProfilerConfigV1,
    MAX_RUNTIME_DEPENDENCIES_V1, MAX_RUNTIME_EVENTS_V1, MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1,
    MAX_RUNTIME_STREAMS_V1, MAX_RUNTIME_SUBMISSIONS_V1, RuntimeAccessV1, RuntimeAsyncCopyBackendV1,
    RuntimeAtomicBackendV1, RuntimeAtomicLaunchContractV1, RuntimeAtomicOperationV1,
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCancellationBackendV1, RuntimeCapabilitiesV1,
    RuntimeCollectiveBackendV1, RuntimeCollectiveLaunchContractV1, RuntimeExecutionCapabilitiesV1,
    RuntimeFlushBackendV1, RuntimeMemoryKindV1, RuntimeMemoryOrderV1, RuntimeMemoryScopeV1,
};

mod kfd_backend_sdma_seam;
#[cfg(test)]
use kfd_backend_sdma_seam::ScriptedSdmaDriverV1;
use kfd_backend_sdma_seam::{
    DirectionalSdmaCompletedOwnerV1, DirectionalSdmaDeviceOwnerV1,
    DirectionalSdmaExecutionFailureV1, DirectionalSdmaPairOwnerV1, DirectionalSdmaPollV1,
    DirectionalSdmaSubmissionOwnerV1, SdmaBufferOwnerV1, SdmaRecycleFailureV1,
    SdmaTransitionFailureV1,
};

const KFD_RUNTIME_RING_BYTES_V1: u32 = 64 * 1024;
/// Reviewed V1 bound for independently in-flight native compute queues.
pub const KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1: usize = 2;
/// Reviewed V1 bound for logical streams multiplexed over the native queues.
pub const KFD_RUNTIME_MAX_LOGICAL_STREAMS_V1: usize = MAX_RUNTIME_STREAMS_V1;
/// Maximum exact atomic or collective profiles inspected per launch.
pub const KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1: usize = 64;
const COV6_IMPLICIT_KERNARG_BYTES_V1: usize = 256;
const WAIT_SPINS_V1: u32 = 32;
const WAIT_YIELDS_V1: u32 = 8;
const WAIT_INITIAL_SLEEP_V1: Duration = Duration::from_micros(50);
const WAIT_MAX_SLEEP_V1: Duration = Duration::from_millis(1);
const COOPERATIVE_COPY_CHUNK_BYTES_V1: usize = 64 * 1024;
const COOPERATIVE_COPY_FAILURE_CODE_V1: i64 = -1;
const MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1: usize = 256;
const MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1: usize = MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1;
const MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1: usize = MAX_RUNTIME_DEPENDENCIES_V1;
const KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1: u64 = 1;

/// Maximum host-staged size of one logical direct-KFD allocation.
pub const KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1: u64 = 256 * 1024 * 1024;

/// Maximum aggregate host-staged logical allocation bytes in one backend.
pub const KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1: u64 = 1024 * 1024 * 1024;

/// Maximum aggregate host staging retained by pending cooperative copies.
pub const KFD_RUNTIME_MAX_COOPERATIVE_COPY_STAGING_BYTES_V1: u64 = 1024 * 1024 * 1024;

/// Stable classification for failures returned by [`KfdRuntimeBackendV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum KfdRuntimeBackendErrorKindV1 {
    Unsupported,
    UnknownHandle,
    WrongDevice,
    Busy,
    InvalidLaunch,
    Capacity,
    Native,
    Terminal,
}

/// Owned, thread-safe error crossing the backend SPI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KfdRuntimeBackendErrorV1 {
    kind: KfdRuntimeBackendErrorKindV1,
    detail: String,
}

impl KfdRuntimeBackendErrorV1 {
    fn new(kind: KfdRuntimeBackendErrorKindV1, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Returns the stable failure class.
    pub const fn kind(&self) -> KfdRuntimeBackendErrorKindV1 {
        self.kind
    }

    /// Returns the operation-specific detail without exposing native handles.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for KfdRuntimeBackendErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for KfdRuntimeBackendErrorV1 {}

/// Host-side phase durations for the most recently completed direct-KFD launch.
///
/// `publish_to_completion` begins after the doorbell publication call returns
/// and ends when completion is first observed. It is the nearest available KFD
/// counterpart to a synchronized launch/wait interval; it is not a device clock.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KfdRuntimeLaunchPerformanceV1 {
    preparation: Duration,
    bound_snapshot: Duration,
    authority: Duration,
    native_binding: Duration,
    publication: Duration,
    publish_to_completion: Duration,
    completed_readback: Duration,
    recycle: Duration,
}

impl KfdRuntimeLaunchPerformanceV1 {
    pub const fn preparation(self) -> Duration {
        self.preparation
    }

    pub const fn bound_snapshot(self) -> Duration {
        self.bound_snapshot
    }

    pub const fn authority(self) -> Duration {
        self.authority
    }

    pub const fn native_binding(self) -> Duration {
        self.native_binding
    }

    pub const fn publication(self) -> Duration {
        self.publication
    }

    pub const fn publish_to_completion(self) -> Duration {
        self.publish_to_completion
    }

    pub const fn completed_readback(self) -> Duration {
        self.completed_readback
    }

    pub const fn recycle(self) -> Duration {
        self.recycle
    }
}

/// One exact staged allocation window presented to direct-launch authority.
#[derive(Clone, Copy, Debug)]
pub struct KfdRuntimeAuthorityAllocationV1<'a> {
    pub allocation: u64,
    pub kind: RuntimeMemoryKindV1,
    pub alignment: u64,
    /// Offset in the logical allocation represented by `bytes`.
    pub byte_offset: u64,
    pub bytes: &'a [u8],
    /// Whole-allocation digest retained from the last complete host write.
    /// Partial host writes and device writeback clear this evidence.
    pub content_sha256: Option<[u8; 32]>,
}

/// Reconciled source/physical global-buffer row used by fixed dispatch.
#[derive(Clone, Copy, Debug)]
pub struct KfdRuntimeAuthorityGlobalBufferV1<'a> {
    pub explicit_argument_index: usize,
    pub name: &'a str,
    pub kernarg_byte_offset: u64,
    pub pointee_alignment: u64,
    pub access: ArgumentAccess,
}

/// Exact, geometry-independent atomic profile admitted by semantic authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeAtomicExecutionProfileV1 {
    pub operation: RuntimeAtomicOperationV1,
    pub scope: RuntimeMemoryScopeV1,
    pub order: RuntimeMemoryOrderV1,
    pub failure_order: Option<RuntimeMemoryOrderV1>,
    pub weak: bool,
}

impl KfdRuntimeAtomicExecutionProfileV1 {
    fn matches_v1(self, contract: RuntimeAtomicLaunchContractV1) -> bool {
        self.operation == contract.operation
            && self.scope == contract.scope
            && self.order == contract.order
            && self.failure_order == contract.failure_order
            && self.weak == contract.weak
    }
}

/// Exact, geometry-independent collective profile admitted by semantic authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeCollectiveExecutionProfileV1 {
    pub operation: crate::RuntimeCollectiveOperationV1,
    pub scope: RuntimeMemoryScopeV1,
    pub order: RuntimeMemoryOrderV1,
}

impl KfdRuntimeCollectiveExecutionProfileV1 {
    fn matches_v1(self, contract: RuntimeCollectiveLaunchContractV1) -> bool {
        self.operation == contract.operation
            && self.scope == contract.scope
            && self.order == contract.order
    }
}

/// KFD name for the semantic class carried into final native authorization.
pub type KfdRuntimeSemanticLaunchV1 = BackendSemanticLaunchV1;

/// Exact address-free invocation presented before any direct KFD mutation.
#[derive(Clone, Copy, Debug)]
pub struct KfdRuntimeAuthorityRequestV1<'a> {
    pub module_image: &'a [u8],
    pub module_sha256: [u8; 32],
    pub kernel_name: &'a str,
    pub signature: [u8; 32],
    pub explicit_kernarg: &'a [u8],
    pub complete_kernarg_template: &'a [u8],
    pub bindings: &'a [crate::BackendBindingV1],
    pub dispatch_abi: &'a [KfdRuntimeAuthorityGlobalBufferV1<'a>],
    pub allocations: &'a [KfdRuntimeAuthorityAllocationV1<'a>],
    pub geometry: crate::RuntimeLaunchGeometryV1,
    pub semantic_launch: KfdRuntimeSemanticLaunchV1,
}

/// Invocation-specific authority for the in-process direct-KFD backend.
///
/// Community applications should use the worker backend. Direct KFD execution
/// shares the application's GPU VM and therefore requires the same artifact,
/// ABI, effect, bounds, alias, initialization, and quiescence evidence as the
/// Worker V3 transition.
///
/// Safe code cannot implement this boundary:
///
/// ```compile_fail
/// use fe2o3_runtime::{KfdRuntimeAuthorityRequestV1, KfdRuntimeLaunchAuthorityV1};
///
/// struct Forged;
/// impl KfdRuntimeLaunchAuthorityV1 for Forged {
///     fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool { true }
/// }
/// ```
///
/// # Safety
///
/// Returning `true` must mean the exact request is covered by authenticated
/// compiler lineage and an invocation-specific proof of all device memory
/// effects. It must also establish that completion observation is sufficient
/// for host reuse of every referenced allocation. Descriptive hashes or
/// structural AMDHSA validation alone do not satisfy this contract. A panic is
/// contained and treated as a fail-closed denial before native publication.
pub unsafe trait KfdRuntimeLaunchAuthorityV1: fmt::Debug {
    fn authorize_launch_v1(&self, request: KfdRuntimeAuthorityRequestV1<'_>) -> bool;
}

/// Additive authority for exact atomic and collective native launches.
///
/// Profiles are an admission filter, not evidence by themselves. The final
/// invocation request still carries the exact contract and must be authorized
/// after its complete kernarg, allocation windows, and geometry are known.
/// Empty or over-bound profile slices advertise no semantic capability.
///
/// # Safety
///
/// Every returned profile must be backed by authenticated compiler-to-machine
/// lineage and native evidence for its operation, address space, width, return
/// value, ordering, scope, fences, and instruction sequence. Collective
/// profiles additionally require authenticated convergence, participant mask,
/// LDS, barrier, and result-layout evidence. Implementations must reject any
/// final request outside that evidence in [`Self::authorize_launch_v1`].
/// Both slices and their contents must remain immutable for the lifetime of
/// the backend so stable capability enumeration cannot become stale.
pub unsafe trait KfdRuntimeSemanticLaunchAuthorityV1: KfdRuntimeLaunchAuthorityV1 {
    fn atomic_profiles_v1(&self) -> &[KfdRuntimeAtomicExecutionProfileV1];

    fn collective_profiles_v1(&self) -> &[KfdRuntimeCollectiveExecutionProfileV1];
}

enum KfdRuntimeLaunchGateV1 {
    Production(Box<dyn KfdRuntimeLaunchAuthorityV1>),
    Semantic(Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942Vecadd(crate::qualification_gfx942_vecadd_v1::AdmittedGfx942VecaddQualificationV1),
}

impl fmt::Debug for KfdRuntimeLaunchGateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Production(authority) => formatter
                .debug_tuple("Production")
                .field(authority)
                .finish(),
            Self::Semantic(authority) => {
                formatter.debug_tuple("Semantic").field(authority).finish()
            }
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942Vecadd(_) => formatter.write_str("ExactGfx942Vecadd"),
        }
    }
}

impl KfdRuntimeLaunchGateV1 {
    fn authorize_launch_v1(&self, request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        catch_authority_callback_v1(|| match self {
            Self::Production(authority) => authority.authorize_launch_v1(request),
            Self::Semantic(authority) => authority.authorize_launch_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942Vecadd(admitted) => admitted.authorizes_kfd_request_v1(request),
        })
        .unwrap_or(false)
    }

    fn supports_atomic_v1(&self, contract: RuntimeAtomicLaunchContractV1) -> bool {
        let Self::Semantic(authority) = self else {
            return false;
        };
        let Some(profiles) = catch_authority_callback_v1(|| authority.atomic_profiles_v1()) else {
            return false;
        };
        profiles.len() <= KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1
            && profiles.iter().any(|profile| {
                atomic_profile_is_admissible_v1(*profile) && profile.matches_v1(contract)
            })
    }

    fn supports_collective_v1(&self, contract: RuntimeCollectiveLaunchContractV1) -> bool {
        let Self::Semantic(authority) = self else {
            return false;
        };
        let Some(profiles) = catch_authority_callback_v1(|| authority.collective_profiles_v1())
        else {
            return false;
        };
        profiles.len() <= KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1
            && profiles.iter().any(|profile| {
                collective_profile_is_admissible_v1(*profile) && profile.matches_v1(contract)
            })
    }

    fn advertises_atomics_v1(&self) -> bool {
        let Self::Semantic(authority) = self else {
            return false;
        };
        let Some(profiles) = catch_authority_callback_v1(|| authority.atomic_profiles_v1()) else {
            return false;
        };
        profiles.len() <= KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1
            && profiles
                .iter()
                .copied()
                .any(atomic_profile_is_admissible_v1)
    }

    fn advertises_collectives_v1(&self) -> bool {
        let Self::Semantic(authority) = self else {
            return false;
        };
        let Some(profiles) = catch_authority_callback_v1(|| authority.collective_profiles_v1())
        else {
            return false;
        };
        profiles.len() <= KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1
            && profiles
                .iter()
                .copied()
                .any(collective_profile_is_admissible_v1)
    }
}

fn catch_authority_callback_v1<T>(operation: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(value) => Some(value),
        Err(payload) => {
            // An unsafe authority may supply a payload whose destructor also panics.
            core::mem::forget(payload);
            None
        }
    }
}

#[derive(Debug)]
struct AllocationRecordV1 {
    device: u64,
    kind: RuntimeMemoryKindV1,
    alignment: u64,
    bytes: Arc<[u8]>,
    content_sha256: Option<[u8; 32]>,
    last_full_host_write: Option<(Arc<[u8]>, [u8; 32])>,
    native_dirty: Vec<NativeDirtyExtentV1>,
    sdma_storage: KfdRuntimeSdmaStorageV1,
    sdma_backed: bool,
    sdma_initialized: bool,
    sdma_shadow_dirty: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KfdRuntimeSdmaInFlightV1 {
    Async(u64),
    Synchronous,
}

#[derive(Debug)]
enum KfdRuntimeSdmaStorageV1 {
    Synthetic,
    Host(SdmaBufferOwnerV1),
    Device(Box<DirectionalSdmaDeviceOwnerV1>),
    DemotedDevice(SdmaBufferOwnerV1),
    InFlight(KfdRuntimeSdmaInFlightV1),
}

impl KfdRuntimeSdmaStorageV1 {
    const fn is_available_for_kind_v1(&self, kind: RuntimeMemoryKindV1) -> bool {
        matches!(
            (self, kind),
            (Self::Host(_), RuntimeMemoryKindV1::HostVisible)
                | (Self::Device(_), RuntimeMemoryKindV1::DeviceLocal)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeDirtyExtentV1 {
    compute_lane: usize,
    data_index: usize,
    allocation_offset: usize,
    data_offset: u64,
    byte_len: u64,
}

struct ModuleRecordV1 {
    device: u64,
    validated: OwnedValidatedEnvelope,
    image_sha256: [u8; 32],
}

struct KernelRecordV1 {
    module: u64,
    validated: OwnedValidatedKernelEnvelope,
    signature: [u8; 32],
}

impl fmt::Debug for ModuleRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleRecordV1")
            .field("device", &self.device)
            .field("image_bytes", &self.validated.bytes().len())
            .field("image_sha256", &self.image_sha256)
            .finish()
    }
}

impl fmt::Debug for KernelRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KernelRecordV1")
            .field("module", &self.module)
            .field("name", &self.validated.selected_kernel().name())
            .field("signature", &self.signature)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
struct SubmissionRecordV1 {
    stream: u64,
    status: BackendPollV1,
}

#[derive(Clone, Copy, Debug)]
struct EventRecordV1 {
    submission: u64,
}

#[derive(Clone, Copy, Debug)]
struct WritebackV1 {
    allocation: u64,
    allocation_offset: usize,
    data_index: usize,
    data_offset: u64,
    byte_len: u64,
}

struct ActiveSubmissionV1 {
    id: u64,
    stream: u64,
    kernel: u64,
    dependency_depth: usize,
    allocations: HashSet<u64>,
    writebacks: Vec<WritebackV1>,
    resident_descriptors: Vec<ResidentDataDescriptorV1>,
    dispatch_shape_sha256: [u8; 32],
    published_at: Instant,
    performance: KfdRuntimeLaunchPerformanceV1,
    batch: Option<Gfx942DispatchBatchV1<1>>,
}

#[derive(Debug)]
struct OwnedComputeLaunchV1 {
    stream: u64,
    kernel: u64,
    explicit_kernarg: Box<[u8]>,
    bindings: Box<[BackendBindingV1]>,
    geometry: crate::RuntimeLaunchGeometryV1,
    semantic_launch: KfdRuntimeSemanticLaunchV1,
}

impl OwnedComputeLaunchV1 {
    fn borrowed(&self) -> BackendLaunchV1<'_> {
        BackendLaunchV1 {
            stream: self.stream,
            kernel: self.kernel,
            explicit_kernarg: &self.explicit_kernarg,
            bindings: &self.bindings,
            dependencies: &[],
            geometry: self.geometry,
            semantic_launch: self.semantic_launch,
        }
    }
}

#[derive(Debug)]
struct PendingComputeSubmissionV1 {
    id: u64,
    module: u64,
    launch: OwnedComputeLaunchV1,
    retained_allocations: Box<[u64]>,
    prior_stream_submission: Option<u64>,
    dependencies: Vec<u64>,
    dependency_cursor: usize,
    dependency_depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeAllocationCustodyKindV1 {
    Compute,
    Sdma,
}

impl RuntimeAllocationCustodyKindV1 {
    const fn index(self) -> usize {
        match self {
            Self::Compute => 0,
            Self::Sdma => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAllocationCustodyOwnerV1 {
    submission: u64,
    stream: u64,
    kind: RuntimeAllocationCustodyKindV1,
}

#[derive(Debug)]
struct RuntimeAllocationCustodyV1 {
    owners: VecDeque<RuntimeAllocationCustodyOwnerV1>,
    sole_stream: Option<u64>,
    owner_counts: [usize; 2],
}

#[derive(Debug)]
struct ActiveSdmaCopyV1 {
    id: u64,
    stream: u64,
    prior_stream_submission: Option<u64>,
    source: u64,
    destination: u64,
    source_offset: u64,
    destination_offset: u64,
    byte_len: u64,
    completed_bytes: u64,
    packet_bytes: u32,
    dependencies: Vec<u64>,
    dependency_cursor: usize,
    dependency_depth: usize,
    phase: ActiveDirectionalSdmaPhaseV1,
}

#[derive(Debug)]
enum ActiveDirectionalSdmaPhaseV1 {
    Ready,
    Published(Box<DirectionalSdmaSubmissionOwnerV1>),
}

#[allow(dead_code)]
enum KfdRuntimeTerminalSdmaCustodyV1 {
    Buffer(SdmaBufferOwnerV1),
    Promotion(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1),
    Demotion(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1),
    Submission(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
    Pending(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
    Retirement {
        failure: Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
        host: Gfx942SdmaBufferV1,
    },
    Pair {
        device: DirectionalSdmaDeviceOwnerV1,
        host: SdmaBufferOwnerV1,
    },
    #[cfg(test)]
    Scripted(kfd_backend_sdma_seam::ScriptedTerminalCustodyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectSdmaDependencyDepthErrorV1 {
    Overflow,
    LimitExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KfdCopyComputeAdmissionV1 {
    Concurrent,
    DeferredByDependency,
    Busy,
}

fn launch_overlaps_active_compute_v1<'a>(
    bindings: &[BackendBindingV1],
    mut active: impl Iterator<Item = &'a ActiveSubmissionV1>,
) -> bool {
    active.any(|submission| {
        bindings
            .iter()
            .any(|binding| submission.allocations.contains(&binding.region.allocation))
    })
}

fn indexed_published_sdma_conflict_v1(
    bindings: &[BackendBindingV1],
    custody: &HashMap<u64, RuntimeAllocationCustodyV1>,
    submission: u64,
    stream: u64,
    mut is_published: impl FnMut(u64) -> bool,
) -> Option<u64> {
    bindings.iter().find_map(|binding| {
        custody.get(&binding.region.allocation).and_then(|custody| {
            custody.owners.iter().find_map(|owner| {
                (owner.kind == RuntimeAllocationCustodyKindV1::Sdma
                    && (owner.stream != stream || owner.submission < submission)
                    && is_published(owner.submission))
                .then_some(owner.submission)
            })
        })
    })
}

fn native_sdma_region_is_admitted_v1(
    allocation: Option<&AllocationRecordV1>,
    device: u64,
    region: BackendMemoryRegionV1,
) -> bool {
    region
        .byte_offset
        .checked_add(region.byte_len)
        .zip(allocation)
        .is_some_and(|(end, allocation)| {
            allocation.device == device
                && allocation.sdma_backed
                && allocation.sdma_initialized
                && !matches!(
                    allocation.sdma_storage,
                    KfdRuntimeSdmaStorageV1::DemotedDevice(_)
                )
                && end <= allocation.bytes.len() as u64
        })
}

fn direct_sdma_direction_v1(
    source: RuntimeMemoryKindV1,
    destination: RuntimeMemoryKindV1,
) -> Option<Gfx942PersistentSdmaDirectionV1> {
    match (source, destination) {
        (RuntimeMemoryKindV1::HostVisible, RuntimeMemoryKindV1::DeviceLocal) => {
            Some(Gfx942PersistentSdmaDirectionV1::HostToDevice)
        }
        (RuntimeMemoryKindV1::DeviceLocal, RuntimeMemoryKindV1::HostVisible) => {
            Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost)
        }
        (RuntimeMemoryKindV1::HostVisible, RuntimeMemoryKindV1::HostVisible)
        | (RuntimeMemoryKindV1::DeviceLocal, RuntimeMemoryKindV1::DeviceLocal) => None,
    }
}

fn directional_sdma_allocation_ids_v1(
    active: &ActiveSdmaCopyV1,
    direction: Gfx942PersistentSdmaDirectionV1,
) -> (u64, u64) {
    match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => (active.source, active.destination),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => (active.destination, active.source),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectSdmaPacketPlanV1 {
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
}

fn direct_sdma_packet_plan_v1(
    active: &ActiveSdmaCopyV1,
    direction: Gfx942PersistentSdmaDirectionV1,
) -> Option<DirectSdmaPacketPlanV1> {
    let remaining = active.byte_len.checked_sub(active.completed_bytes)?;
    let copy_bytes =
        u32::try_from(remaining.min(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))).ok()?;
    if copy_bytes == 0 {
        return None;
    }
    let source_offset = active.source_offset.checked_add(active.completed_bytes)?;
    let destination_offset = active
        .destination_offset
        .checked_add(active.completed_bytes)?;
    Some(match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => DirectSdmaPacketPlanV1 {
            host_offset: source_offset,
            device_offset: destination_offset,
            copy_bytes,
        },
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => DirectSdmaPacketPlanV1 {
            host_offset: destination_offset,
            device_offset: source_offset,
            copy_bytes,
        },
    })
}

impl fmt::Debug for ActiveSubmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveSubmissionV1")
            .field("id", &self.id)
            .field("stream", &self.stream)
            .field("kernel", &self.kernel)
            .field("allocations", &self.allocations)
            .field("writebacks", &self.writebacks)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct DataSpecV1 {
    allocation: u64,
    kind: RuntimeMemoryKindV1,
    alignment: u64,
    allocation_offset: u64,
    bytes: Arc<[u8]>,
    byte_range: Range<usize>,
    content_sha256: Option<[u8; 32]>,
}

impl DataSpecV1 {
    fn bytes(&self) -> &[u8] {
        &self.bytes[self.byte_range.clone()]
    }

    fn try_owned_bytes(&self) -> Result<Box<[u8]>, String> {
        let source = self.bytes();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(source.len())
            .map_err(|_| "KFD native-data content allocation failed".to_owned())?;
        bytes.extend_from_slice(source);
        Ok(bytes.into_boxed_slice())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StagedPlacementV1 {
    data_index: usize,
    allocation_offset: u64,
}

#[derive(Debug)]
struct StagedDataRosterV1 {
    data: Vec<DataSpecV1>,
    placements: HashMap<u64, StagedPlacementV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentDataDescriptorV1 {
    allocation: u64,
    kind: RuntimeMemoryKindV1,
    alignment: u64,
    allocation_offset: u64,
    byte_len: u64,
    host_content_sha256: Option<[u8; 32]>,
    device_may_have_modified: bool,
}

struct ResidentDataRosterV1 {
    descriptors: Vec<ResidentDataDescriptorV1>,
    data: Vec<Gfx942FixedDispatchDataV1>,
}

struct RecycledDispatchV1 {
    kernel: u64,
    dispatch_shape_sha256: [u8; 32],
    descriptors: Vec<ResidentDataDescriptorV1>,
}

struct NativeComputeLaneRuntimeV1 {
    owner_stream: Option<u64>,
    active: Option<ActiveSubmissionV1>,
    resident_data: Option<ResidentDataRosterV1>,
    recycled_dispatch: Option<RecycledDispatchV1>,
}

impl NativeComputeLaneRuntimeV1 {
    const fn vacant() -> Self {
        Self {
            owner_stream: None,
            active: None,
            resident_data: None,
            recycled_dispatch: None,
        }
    }
}

struct PreparedLaunchV1 {
    stream: u64,
    kernel: u64,
    program: OwnedValidatedKernelEnvelope,
    signature: [u8; 32],
    kernarg: Box<[u8]>,
    geometry: AqlDispatchGeometryV1,
    dynamic_shared_bytes: u32,
    buffer_bindings: Box<[Gfx942DispatchBufferBindingV1]>,
    abi_rows: Vec<OwnedAbiRowV1>,
    data: Vec<DataSpecV1>,
    allocations: HashSet<u64>,
    writebacks: Vec<WritebackV1>,
    dispatch_shape_sha256: [u8; 32],
    profile_launch: KfdProfileLaunchV1,
    profile_semantic_contract: Option<KfdProfileSemanticContractV1>,
    profile_bindings: Option<Result<Vec<KfdProfileBindingV1>, ()>>,
    performance: KfdRuntimeLaunchPerformanceV1,
}

fn recycled_dispatch_reuse_is_admitted_v1(
    recycled: &RecycledDispatchV1,
    dispatch_shape_sha256: [u8; 32],
    resident_descriptors: &[ResidentDataDescriptorV1],
    data: &[DataSpecV1],
) -> bool {
    recycled.dispatch_shape_sha256 == dispatch_shape_sha256
        && same_resident_storage_shape_v1(&recycled.descriptors, resident_descriptors)
        && data
            .iter()
            .all(|spec| spec.kind == RuntimeMemoryKindV1::HostVisible)
}

#[derive(Clone, Copy, Debug)]
struct StagingBudgetsV1 {
    max_allocation_bytes: u64,
    max_context_bytes: u64,
}

#[derive(Debug)]
struct OwnedAbiRowV1 {
    explicit_argument_index: usize,
    offset: u64,
    pointee_alignment: u64,
    access: ArgumentAccess,
}

/// Concrete address-free adapter for the admitted MI300X/gfx942 KFD profile.
///
/// Construction retains one checked device but performs no VM, allocation,
/// queue, or dispatch operation. Native resources are materialized lazily on
/// the first launch. [`Self::shutdown_native_v1`] provides reportable native
/// teardown. Clean implicit drop performs the same teardown and aborts if it
/// cannot prove success; dropping live or terminal native custody also aborts.
///
/// The adapter schedules bounded logical streams over two persistent,
/// independently publishable compute queues. Live allocations retain native SDMA
/// storage, and same-device asynchronous copies can wait on explicit event
/// dependencies. One compute dispatch and SDMA copies may overlap only when
/// their allocation sets are disjoint. Accepted compute work remains in an owned
/// per-stream FIFO until its predecessor and explicit dependencies complete and
/// one native lane can be leased without reordering overlapping cross-stream work.
/// Persistent buffers are
/// leased from a queue-owned pool, scrubbed as required before recycle, and the
/// pool is trimmed during explicit shutdown. Compute still materializes separate
/// fixed-dispatch storage from the bounded logical host image, so persistent
/// copy storage is not yet a shared compute allocation. The adapter exposes one
/// gfx942 device and no peer copy or multi-device operations. Atomic and
/// collective profiles remain unavailable unless an unsafe semantic authority
/// explicitly enumerates and authorizes their exact contracts.
#[must_use = "direct KFD backends must remain owned through quiescence"]
pub struct KfdRuntimeBackendV1 {
    description: BackendDeviceDescriptionV1,
    admitted_device: Option<CheckedGfx942XnackMinusDevice>,
    queue: Option<ComputeAqlQueueSessionV1>,
    terminal_memory: Option<SharedGttMemorySessionV1>,
    terminal_sdma_custody: Option<KfdRuntimeTerminalSdmaCustodyV1>,
    queue_retired: bool,
    terminal: bool,
    next_handle: u64,
    streams: HashMap<u64, u64>,
    allocations: HashMap<u64, AllocationRecordV1>,
    modules: HashMap<u64, ModuleRecordV1>,
    kernels: HashMap<u64, KernelRecordV1>,
    submissions: HashMap<u64, SubmissionRecordV1>,
    compute_completion_reservations: usize,
    sdma_completion_reservations: usize,
    pending_compute: HashMap<u64, PendingComputeSubmissionV1>,
    pending_compute_streams: HashMap<u64, VecDeque<u64>>,
    allocation_custody: HashMap<u64, RuntimeAllocationCustodyV1>,
    compute_module_retain_counts: HashMap<u64, usize>,
    compute_dependency_retain_counts: HashMap<u64, usize>,
    stream_submission_tails: HashMap<u64, u64>,
    events: HashMap<u64, EventRecordV1>,
    event_submission_retain_counts: HashMap<u64, usize>,
    active: Option<ActiveSubmissionV1>,
    resident_data: Option<ResidentDataRosterV1>,
    recycled_dispatch: Option<RecycledDispatchV1>,
    auxiliary_compute_lanes: Vec<NativeComputeLaneRuntimeV1>,
    native_compute_lanes: Vec<Option<ComputeAqlQueueLaneV1>>,
    stream_compute_lanes: HashMap<u64, usize>,
    selected_compute_lane: usize,
    native_dirty_extents: usize,
    active_sdma: HashMap<u64, ActiveSdmaCopyV1>,
    active_sdma_streams: HashMap<u64, VecDeque<u64>>,
    sdma_dependency_retain_counts: HashMap<u64, usize>,
    quiescent_sdma_submissions: HashSet<u64>,
    last_launch_performance: Option<KfdRuntimeLaunchPerformanceV1>,
    staging_budgets: StagingBudgetsV1,
    staged_context_bytes: u64,
    sdma_enabled: bool,
    native_available: bool,
    launch_gate: KfdRuntimeLaunchGateV1,
    profiler: Option<KfdRuntimeProfileRecorderV1>,
    #[cfg(test)]
    scripted_sdma: Option<ScriptedSdmaDriverV1>,
    #[cfg(test)]
    scripted_drop_disarmed: bool,
}

impl fmt::Debug for KfdRuntimeBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdRuntimeBackendV1")
            .field("description", &self.description)
            .field("has_admitted_device", &self.admitted_device.is_some())
            .field("has_queue", &self.queue.is_some())
            .field("has_terminal_memory", &self.terminal_memory.is_some())
            .field(
                "has_terminal_sdma_custody",
                &self.terminal_sdma_custody.is_some(),
            )
            .field("queue_retired", &self.queue_retired)
            .field("terminal", &self.terminal)
            .field("streams", &self.streams.len())
            .field("allocations", &self.allocations.len())
            .field("modules", &self.modules.len())
            .field("kernels", &self.kernels.len())
            .field("submissions", &self.submissions.len())
            .field(
                "compute_completion_reservations",
                &self.compute_completion_reservations,
            )
            .field(
                "sdma_completion_reservations",
                &self.sdma_completion_reservations,
            )
            .field("pending_compute", &self.pending_compute.len())
            .field("allocation_custody", &self.allocation_custody.len())
            .field(
                "compute_module_retain_counts",
                &self.compute_module_retain_counts.len(),
            )
            .field("events", &self.events.len())
            .field(
                "event_submission_retain_counts",
                &self.event_submission_retain_counts.len(),
            )
            .field(
                "active_compute_lanes",
                &(self
                    .auxiliary_compute_lanes
                    .iter()
                    .filter(|lane| lane.active.is_some())
                    .count()
                    + usize::from(self.active.is_some())),
            )
            .field("active_sdma", &self.active_sdma.len())
            .field("active_sdma_streams", &self.active_sdma_streams.len())
            .field("native_dirty_extents", &self.native_dirty_extents)
            .field(
                "sdma_dependency_retain_counts",
                &self.sdma_dependency_retain_counts.len(),
            )
            .field(
                "quiescent_sdma_submissions",
                &self.quiescent_sdma_submissions.len(),
            )
            .field("compute_lanes", &(1 + self.auxiliary_compute_lanes.len()))
            .field("last_launch_performance", &self.last_launch_performance)
            .field("staged_context_bytes", &self.staged_context_bytes)
            .field("sdma_enabled", &self.sdma_enabled)
            .field("staging_budgets", &self.staging_budgets)
            .field("launch_gate", &self.launch_gate)
            .field("profiler", &self.profiler)
            .finish()
    }
}

impl KfdRuntimeBackendV1 {
    /// Opens `/dev/kfd`, admits the reviewed UAPI, and binds one exact GPU.
    pub fn open_default<A>(
        device_unique_id: u64,
        authority: A,
    ) -> Result<Self, KfdRuntimeBackendErrorV1>
    where
        A: KfdRuntimeLaunchAuthorityV1 + 'static,
    {
        Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::Production(Box::new(authority)),
        )
    }

    /// Opens a direct backend whose exact semantic profiles are supplied by a
    /// separate unsafe authority.
    pub fn open_default_with_semantic_authority_v1<A>(
        device_unique_id: u64,
        authority: A,
    ) -> Result<Self, KfdRuntimeBackendErrorV1>
    where
        A: KfdRuntimeSemanticLaunchAuthorityV1 + 'static,
    {
        Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::Semantic(Box::new(authority)),
        )
    }

    #[cfg(feature = "hardware-qualification")]
    /// Opens the exact repository-owned gfx942 vecadd qualification backend.
    ///
    /// This constructor re-admits and retains the embedded fixture, then
    /// accepts only its fixed ABI, metadata-declared effects, contents, and
    /// launch geometry. It grants no production authority and cannot launch
    /// another module or invocation.
    pub fn open_gfx942_vecadd_qualification_v1(
        device_unique_id: u64,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let admitted = crate::qualification_gfx942_vecadd_v1::admit_gfx942_vecadd_qualification_v1(
        )
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                error.to_string(),
            )
        })?;
        Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::ExactGfx942Vecadd(admitted),
        )
    }

    fn open_default_with_gate(
        device_unique_id: u64,
        launch_gate: KfdRuntimeLaunchGateV1,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if device_unique_id == 0 {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "device unique id must be nonzero",
            ));
        }
        let kfd = OpenedKfd::open_default().map_err(|error| {
            KfdRuntimeBackendErrorV1::new(KfdRuntimeBackendErrorKindV1::Native, error.to_string())
        })?;
        let admitted = kfd.admit_uapi().map_err(|error| {
            KfdRuntimeBackendErrorV1::new(KfdRuntimeBackendErrorKindV1::Native, error.to_string())
        })?;
        let device = admitted
            .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(device_unique_id))
            .map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    error.to_string(),
                )
            })?;
        Ok(Self::from_checked_device_with_gate(device, launch_gate))
    }

    /// Wraps an already checked gfx942/XNACK-disabled device.
    pub fn from_checked_device<A>(device: CheckedGfx942XnackMinusDevice, authority: A) -> Self
    where
        A: KfdRuntimeLaunchAuthorityV1 + 'static,
    {
        Self::from_checked_device_with_gate(
            device,
            KfdRuntimeLaunchGateV1::Production(Box::new(authority)),
        )
    }

    /// Wraps a checked device with exact semantic launch authority.
    pub fn from_checked_device_with_semantic_authority_v1<A>(
        device: CheckedGfx942XnackMinusDevice,
        authority: A,
    ) -> Self
    where
        A: KfdRuntimeSemanticLaunchAuthorityV1 + 'static,
    {
        Self::from_checked_device_with_gate(
            device,
            KfdRuntimeLaunchGateV1::Semantic(Box::new(authority)),
        )
    }

    fn from_checked_device_with_gate(
        device: CheckedGfx942XnackMinusDevice,
        launch_gate: KfdRuntimeLaunchGateV1,
    ) -> Self {
        let observation = device.observation();
        let unique_id = observation.unique_id();
        let name = device
            .topology_snapshot()
            .topology()
            .gpu_nodes()
            .iter()
            .find(|node| node.unique_id() == unique_id)
            .map_or_else(|| "AMD MI300X".to_owned(), |node| node.name().to_owned());
        Self::new(
            BackendDeviceDescriptionV1 {
                backend_device: unique_id,
                name,
                target: "gfx942:xnack-".to_owned(),
                // The admitted topology schema does not currently expose a
                // trustworthy aggregate VRAM capacity.
                global_memory_bytes: 0,
                capabilities: kfd_capabilities_v1(),
            },
            Some(device),
            launch_gate,
        )
    }

    fn new(
        description: BackendDeviceDescriptionV1,
        admitted_device: Option<CheckedGfx942XnackMinusDevice>,
        launch_gate: KfdRuntimeLaunchGateV1,
    ) -> Self {
        Self::new_with_staging_budgets(
            description,
            admitted_device,
            launch_gate,
            StagingBudgetsV1 {
                max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
                max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
            },
        )
    }

    fn new_with_staging_budgets(
        mut description: BackendDeviceDescriptionV1,
        admitted_device: Option<CheckedGfx942XnackMinusDevice>,
        launch_gate: KfdRuntimeLaunchGateV1,
        staging_budgets: StagingBudgetsV1,
    ) -> Self {
        let native_available = admitted_device.is_some();
        description.capabilities.atomics = launch_gate.advertises_atomics_v1();
        description.capabilities.collectives = launch_gate.advertises_collectives_v1();
        Self {
            description,
            admitted_device,
            queue: None,
            terminal_memory: None,
            terminal_sdma_custody: None,
            queue_retired: false,
            terminal: false,
            next_handle: 1,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            modules: HashMap::new(),
            kernels: HashMap::new(),
            submissions: HashMap::new(),
            compute_completion_reservations: 0,
            sdma_completion_reservations: 0,
            pending_compute: HashMap::new(),
            pending_compute_streams: HashMap::new(),
            allocation_custody: HashMap::new(),
            compute_module_retain_counts: HashMap::new(),
            compute_dependency_retain_counts: HashMap::new(),
            stream_submission_tails: HashMap::new(),
            events: HashMap::new(),
            event_submission_retain_counts: HashMap::new(),
            active: None,
            resident_data: None,
            recycled_dispatch: None,
            auxiliary_compute_lanes: vec![NativeComputeLaneRuntimeV1::vacant()],
            native_compute_lanes: vec![None; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1],
            stream_compute_lanes: HashMap::new(),
            selected_compute_lane: 0,
            native_dirty_extents: 0,
            active_sdma: HashMap::new(),
            active_sdma_streams: HashMap::new(),
            sdma_dependency_retain_counts: HashMap::new(),
            quiescent_sdma_submissions: HashSet::new(),
            last_launch_performance: None,
            staging_budgets,
            staged_context_bytes: 0,
            sdma_enabled: false,
            native_available,
            launch_gate,
            profiler: None,
            #[cfg(test)]
            scripted_sdma: None,
            #[cfg(test)]
            scripted_drop_disarmed: false,
        }
    }

    /// Enables bounded, authority-free profiling before any logical runtime
    /// resource is created. Collection is opt-in and does not alter launch
    /// authority or expose native handles.
    pub fn enable_profiler_v1(
        &mut self,
        config: KfdRuntimeProfilerConfigV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.profiler.is_some()
            || self.next_handle != 1
            || self.queue_retired
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || self.any_compute_active_v1()
            || self.queue.is_some()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "direct-KFD profiling must begin before runtime resource creation",
            ));
        }
        let recorder = KfdRuntimeProfileRecorderV1::new(
            config,
            self.description.backend_device,
            &self.description.target,
            64,
        )
        .map_err(Self::capacity)?;
        self.profiler = Some(recorder);
        Ok(())
    }

    /// Enables the frozen V1 profiler together with the separately versioned
    /// typed semantic sidecar. The extra sidecar storage is opt-in so the V1
    /// producer's allocation and failure surface remains unchanged.
    pub fn enable_profiler_with_semantic_profile_v1(
        &mut self,
        config: KfdRuntimeProfilerConfigV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.profiler.is_some()
            || self.next_handle != 1
            || self.queue_retired
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || self.any_compute_active_v1()
            || self.queue.is_some()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "direct-KFD profiling must begin before runtime resource creation",
            ));
        }
        let recorder = KfdRuntimeProfileRecorderV1::new_with_semantic_profile(
            config,
            self.description.backend_device,
            &self.description.target,
            64,
        )
        .map_err(Self::capacity)?;
        self.profiler = Some(recorder);
        Ok(())
    }

    /// Finishes profiling after all runtime and native KFD custody is closed.
    pub fn finish_profiler_v1(
        &mut self,
    ) -> Result<KfdRuntimeProfileV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.take_finished_profiler_recorder_v1()?
            .finish()
            .map_err(|detail| Self::rejected(KfdRuntimeBackendErrorKindV1::InvalidLaunch, detail))
    }

    /// Finishes the frozen Runtime Profile V1 together with the separately
    /// versioned, exact semantic-contract sidecar.
    pub fn finish_profiler_with_semantic_profile_v1(
        &mut self,
    ) -> Result<
        KfdRuntimeProfileWithSemanticSidecarV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.take_finished_semantic_profiler_recorder_v1()?
            .finish_with_semantic_profile()
            .map_err(|detail| Self::rejected(KfdRuntimeBackendErrorKindV1::InvalidLaunch, detail))
    }

    /// Finishes profiling with runtime-authenticated host dispatch timestamps
    /// after all logical and native KFD custody is closed.
    pub fn finish_profiler_with_dispatch_timestamps_v1(
        &mut self,
    ) -> Result<
        AuthenticatedKfdRuntimeDispatchTimestampsV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.take_finished_profiler_recorder_v1()?
            .finish_with_dispatch_timestamps()
            .map_err(|detail| Self::rejected(KfdRuntimeBackendErrorKindV1::InvalidLaunch, detail))
    }

    /// Finishes the explicit semantic profiler with V2 runtime custody over
    /// host timestamps and the exact semantic sidecar.
    pub fn finish_profiler_with_dispatch_timestamps_v2(
        &mut self,
    ) -> Result<
        AuthenticatedKfdRuntimeDispatchTimestampsV2,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.take_finished_semantic_profiler_recorder_v1()?
            .finish_with_dispatch_timestamps_v2()
            .map_err(|detail| Self::rejected(KfdRuntimeBackendErrorKindV1::InvalidLaunch, detail))
    }

    fn finished_profiler_recorder_v1(
        &self,
    ) -> Result<&KfdRuntimeProfileRecorderV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        self.require_live()?;
        if !self.queue_retired
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || self.any_compute_active_v1()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "direct-KFD profiling can finish only after logical cleanup and native shutdown",
            ));
        }
        self.profiler.as_ref().ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "direct-KFD profiling was not enabled",
            )
        })
    }

    fn take_finished_profiler_recorder_v1(
        &mut self,
    ) -> Result<KfdRuntimeProfileRecorderV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        self.finished_profiler_recorder_v1()?;
        Ok(self
            .profiler
            .take()
            .expect("borrowed finished profiler remains installed"))
    }

    fn take_finished_semantic_profiler_recorder_v1(
        &mut self,
    ) -> Result<KfdRuntimeProfileRecorderV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        if !self
            .finished_profiler_recorder_v1()?
            .captures_semantic_profile()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "semantic profiling was not enabled for this capture",
            ));
        }
        Ok(self
            .profiler
            .take()
            .expect("borrowed finished semantic profiler remains installed"))
    }

    fn profile_resource_v1(
        &self,
        kind: KfdProfileResourceKindV1,
        handle: u64,
    ) -> Option<ProfileIdentityV1> {
        self.profiler.as_ref()?.resource(kind, handle)
    }

    fn observe_profile_v1(&mut self, event: Option<KfdRuntimeProfileEventKindV1>) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.observe(event);
        }
    }

    fn observe_profile_dispatch_v1(
        &mut self,
        event: Option<KfdRuntimeProfileEventKindV1>,
        semantic_contract: Option<KfdProfileSemanticContractV1>,
    ) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.observe_dispatch(event, semantic_contract);
        }
    }

    fn profile_content_v1(&self, bytes: &[u8]) -> Option<ProfileContentIdentityV1> {
        self.profiler.as_ref()?;
        ProfileContentIdentityV1::observed(bytes).ok()
    }

    fn profile_host_content_v1(
        &self,
        bytes: &[u8],
        known_sha256: Option<[u8; 32]>,
    ) -> Option<KfdProfileHostContentV1> {
        self.profiler.as_ref()?.host_content(bytes, known_sha256)
    }

    fn prepare_profile_bindings_v1(
        &self,
        bindings: &[BackendBindingV1],
    ) -> Option<Result<Vec<KfdProfileBindingV1>, ()>> {
        let profiler = self.profiler.as_ref()?;
        if bindings.len() > fe2o3_profiler_protocol::MAX_KFD_RUNTIME_PROFILE_BINDINGS_V1 {
            return Some(Err(()));
        }
        let mut output = Vec::new();
        if output.try_reserve_exact(bindings.len()).is_err() {
            return Some(Err(()));
        }
        for binding in bindings {
            let Some(allocation) = profiler.resource(
                KfdProfileResourceKindV1::Allocation,
                binding.region.allocation,
            ) else {
                return Some(Err(()));
            };
            output.push(KfdProfileBindingV1 {
                allocation,
                access: match binding.region.access {
                    RuntimeAccessV1::Read => KfdProfileAccessV1::Read,
                    RuntimeAccessV1::Write => KfdProfileAccessV1::Write,
                    RuntimeAccessV1::ReadWrite => KfdProfileAccessV1::ReadWrite,
                },
                byte_offset: binding.region.byte_offset,
                byte_len: binding.region.byte_len,
                kernarg_byte_offset: binding.kernarg_byte_offset,
            });
        }
        Some(Ok(output))
    }

    fn rejected(
        kind: KfdRuntimeBackendErrorKindV1,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(kind, detail))
    }

    fn capacity(detail: impl Into<String>) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, detail)
    }

    fn quiescent_error(
        kind: KfdRuntimeBackendErrorKindV1,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        RuntimeBackendFailureV1::Quiescent(KfdRuntimeBackendErrorV1::new(kind, detail))
    }

    fn after_possible_host_mutation(
        failure: RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        match failure {
            RuntimeBackendFailureV1::Rejected(error) => RuntimeBackendFailureV1::Quiescent(error),
            failure => failure,
        }
    }

    fn terminal_error(
        &mut self,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        self.terminal = true;
        RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Terminal,
            detail,
        ))
    }

    fn retain_terminal_sdma_custody_v1(&mut self, custody: KfdRuntimeTerminalSdmaCustodyV1) {
        if self.terminal_sdma_custody.is_some() {
            // Replacing either opaque owner would drop safety-significant
            // custody. There is no recoverable transition after this point.
            std::process::abort();
        }
        self.terminal_sdma_custody = Some(custody);
    }

    fn require_live(&self) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.terminal {
            Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "KFD backend is terminal",
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn next_id(&mut self) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let id = self.next_handle;
        self.next_handle = self.next_handle.checked_add(1).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "backend handle space exhausted",
            )
        })?;
        Ok(id)
    }

    fn require_device(
        &self,
        device: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if device == self.description.backend_device {
            Ok(())
        } else {
            Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "backend device does not belong to this admitted KFD adapter",
            ))
        }
    }

    fn allocation_is_active(&self, allocation: u64) -> bool {
        self.allocation_custody.contains_key(&allocation)
    }

    fn reserve_event_submission_retain_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self
            .event_submission_retain_counts
            .contains_key(&submission)
        {
            self.event_submission_retain_counts
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD event-retain index growth failed"))?;
        }
        if self
            .event_submission_retain_counts
            .get(&submission)
            .is_some_and(|count| *count == usize::MAX)
        {
            return Err(Self::capacity("KFD event retain count overflow"));
        }
        Ok(())
    }

    fn retain_event_submission_v1(&mut self, submission: u64) {
        *self
            .event_submission_retain_counts
            .entry(submission)
            .or_insert(0) += 1;
    }

    fn release_event_submission_v1(&mut self, submission: u64) {
        let remove = {
            let count = self
                .event_submission_retain_counts
                .get_mut(&submission)
                .expect("live KFD event retains its submission index");
            *count = count
                .checked_sub(1)
                .expect("live KFD event retain count is positive");
            *count == 0
        };
        if remove {
            self.event_submission_retain_counts.remove(&submission);
        }
    }

    fn reserve_active_sdma_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<Option<VecDeque<u64>>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if let Some(submissions) = self.active_sdma_streams.get_mut(&stream) {
            submissions
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD active SDMA stream queue growth failed"))?;
            return Ok(None);
        }
        self.active_sdma_streams
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD active SDMA stream index growth failed"))?;
        let mut submissions = VecDeque::new();
        submissions
            .try_reserve_exact(1)
            .map_err(|_| Self::capacity("KFD active SDMA stream queue allocation failed"))?;
        Ok(Some(submissions))
    }

    fn retain_active_sdma_stream_v1(
        &mut self,
        stream: u64,
        submission: u64,
        new_stream_queue: Option<VecDeque<u64>>,
    ) {
        if let Some(submissions) = self.active_sdma_streams.get_mut(&stream) {
            debug_assert!(new_stream_queue.is_none());
            debug_assert!(submissions.back().is_none_or(|prior| *prior < submission));
            submissions.push_back(submission);
        } else {
            let mut submissions = new_stream_queue
                .expect("new active SDMA stream queue was reserved before retention");
            submissions.push_back(submission);
            debug_assert!(
                self.active_sdma_streams
                    .insert(stream, submissions)
                    .is_none()
            );
        }
    }

    fn release_active_sdma_stream_v1(&mut self, stream: u64, submission: u64) {
        let remove = {
            let submissions = self
                .active_sdma_streams
                .get_mut(&stream)
                .expect("active SDMA submission remains stream-indexed");
            if submissions.front() == Some(&submission) {
                submissions.pop_front();
            } else if submissions.back() == Some(&submission) {
                submissions.pop_back();
            } else {
                let position = submissions
                    .iter()
                    .position(|candidate| *candidate == submission)
                    .expect("active SDMA stream index retains the submission");
                submissions.remove(position);
            }
            submissions.is_empty()
        };
        if remove {
            self.active_sdma_streams.remove(&stream);
        }
    }

    fn require_submission_capacity_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let live = self
            .submissions
            .len()
            .checked_add(self.compute_completion_reservations)
            .and_then(|live| live.checked_add(self.sdma_completion_reservations))
            .ok_or_else(|| Self::capacity("KFD submission count overflow"))?;
        if live >= MAX_RUNTIME_SUBMISSIONS_V1 {
            Err(Self::capacity("KFD submission capacity exceeded"))
        } else {
            Ok(())
        }
    }

    fn reserve_allocation_custody_v1(
        &mut self,
        allocations: &[u64],
    ) -> Result<
        Vec<(u64, RuntimeAllocationCustodyV1)>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let mut new_entries = Vec::new();
        new_entries
            .try_reserve_exact(allocations.len())
            .map_err(|_| Self::capacity("KFD allocation-custody preflight failed"))?;
        for (index, allocation) in allocations.iter().copied().enumerate() {
            if allocations[..index].contains(&allocation) {
                continue;
            }
            if let Some(custody) = self.allocation_custody.get_mut(&allocation) {
                if custody.owners.len() == MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 {
                    return Err(Self::capacity(
                        "KFD per-allocation custody owner capacity exceeded",
                    ));
                }
                custody
                    .owners
                    .try_reserve(1)
                    .map_err(|_| Self::capacity("KFD allocation-custody owner growth failed"))?;
            } else {
                let mut owners = VecDeque::new();
                owners
                    .try_reserve(1)
                    .map_err(|_| Self::capacity("KFD allocation-custody owner growth failed"))?;
                new_entries.push((
                    allocation,
                    RuntimeAllocationCustodyV1 {
                        owners,
                        sole_stream: None,
                        owner_counts: [0; 2],
                    },
                ));
            }
        }
        self.allocation_custody
            .try_reserve(new_entries.len())
            .map_err(|_| Self::capacity("KFD allocation-custody index growth failed"))?;
        Ok(new_entries)
    }

    fn retain_allocation_custody_v1(
        &mut self,
        allocations: &[u64],
        owner: RuntimeAllocationCustodyOwnerV1,
        new_entries: Vec<(u64, RuntimeAllocationCustodyV1)>,
    ) {
        for (allocation, custody) in new_entries {
            debug_assert!(
                self.allocation_custody
                    .insert(allocation, custody)
                    .is_none()
            );
        }
        for (index, allocation) in allocations.iter().copied().enumerate() {
            if allocations[..index].contains(&allocation) {
                continue;
            }
            let custody = self
                .allocation_custody
                .get_mut(&allocation)
                .expect("preflighted allocation custody remains indexed");
            debug_assert!(
                !custody
                    .owners
                    .iter()
                    .any(|existing| existing.submission == owner.submission)
            );
            custody.sole_stream = match custody.owners.front() {
                None => Some(owner.stream),
                Some(_) if custody.sole_stream == Some(owner.stream) => Some(owner.stream),
                Some(_) => None,
            };
            custody.owner_counts[owner.kind.index()] += 1;
            custody.owners.push_back(owner);
        }
    }

    fn release_allocation_custody_v1(&mut self, allocation: u64, submission: u64) {
        let custody = self
            .allocation_custody
            .get_mut(&allocation)
            .expect("accepted submission retains indexed allocation custody");
        let removed = if custody
            .owners
            .front()
            .is_some_and(|owner| owner.submission == submission)
        {
            custody.owners.pop_front().expect("nonempty custody")
        } else if custody
            .owners
            .back()
            .is_some_and(|owner| owner.submission == submission)
        {
            custody.owners.pop_back().expect("nonempty custody")
        } else {
            let position = custody
                .owners
                .iter()
                .position(|owner| owner.submission == submission)
                .expect("accepted submission remains an allocation owner");
            custody
                .owners
                .remove(position)
                .expect("indexed custody position remains valid")
        };
        custody.owner_counts[removed.kind.index()] -= 1;
        if custody.owners.is_empty() {
            self.allocation_custody.remove(&allocation);
        } else if custody.sole_stream.is_none() {
            let stream = custody.owners.front().expect("nonempty custody").stream;
            custody.sole_stream = custody
                .owners
                .iter()
                .all(|owner| owner.stream == stream)
                .then_some(stream);
        }
    }

    fn release_compute_custody_v1(
        &mut self,
        submission: u64,
        module: u64,
        allocations: impl IntoIterator<Item = u64>,
    ) {
        for allocation in allocations {
            self.release_allocation_custody_v1(allocation, submission);
        }
        let count = self
            .compute_module_retain_counts
            .get_mut(&module)
            .expect("accepted compute retains its module");
        *count = count.checked_sub(1).expect("positive module retain count");
        if *count == 0 {
            self.compute_module_retain_counts.remove(&module);
        }
    }

    fn allocation_has_unordered_custody_v1(
        &self,
        allocation: u64,
        stream: u64,
        dependencies: &[u64],
        kind: Option<RuntimeAllocationCustodyKindV1>,
    ) -> bool {
        self.allocation_custody
            .get(&allocation)
            .is_some_and(|custody| {
                if custody.sole_stream == Some(stream) {
                    return false;
                }
                custody.owners.iter().any(|owner| {
                    kind.is_none_or(|kind| owner.kind == kind)
                        && owner.stream != stream
                        && !dependencies.contains(&owner.submission)
                })
            })
    }

    fn published_sdma_conflict_v1(
        &self,
        submission: u64,
        stream: u64,
        bindings: &[BackendBindingV1],
    ) -> Option<u64> {
        indexed_published_sdma_conflict_v1(
            bindings,
            &self.allocation_custody,
            submission,
            stream,
            |candidate| {
                self.active_sdma.get(&candidate).is_some_and(|copy| {
                    matches!(copy.phase, ActiveDirectionalSdmaPhaseV1::Published(_))
                })
            },
        )
    }

    fn any_compute_active_v1(&self) -> bool {
        self.active.is_some()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.active.is_some())
    }

    fn active_compute_lane_v1(&self, submission: u64) -> Option<usize> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.id == submission)
        {
            return Some(0);
        }
        self.auxiliary_compute_lanes
            .iter()
            .position(|lane| {
                lane.active
                    .as_ref()
                    .is_some_and(|active| active.id == submission)
            })
            .map(|index| index + 1)
    }

    fn active_compute_submission_v1(&self, submission: u64) -> Option<&ActiveSubmissionV1> {
        self.active
            .as_ref()
            .filter(|active| active.id == submission)
            .or_else(|| {
                self.auxiliary_compute_lanes
                    .iter()
                    .filter_map(|lane| lane.active.as_ref())
                    .find(|active| active.id == submission)
            })
    }

    fn pending_compute_submission_v1(
        &self,
        submission: u64,
    ) -> Option<&PendingComputeSubmissionV1> {
        self.pending_compute.get(&submission)
    }

    fn next_dependency_depth_v1(
        &self,
        dependencies: &[u64],
    ) -> Result<usize, DirectSdmaDependencyDepthErrorV1> {
        let mut depth = 1_usize;
        for dependency in dependencies {
            let dependency_depth = self
                .pending_compute
                .get(dependency)
                .map(|pending| pending.dependency_depth)
                .or_else(|| {
                    self.active_compute_submission_v1(*dependency)
                        .map(|active| active.dependency_depth)
                })
                .or_else(|| {
                    self.active_sdma
                        .get(dependency)
                        .map(|copy| copy.dependency_depth)
                });
            if let Some(dependency_depth) = dependency_depth {
                depth = depth.max(
                    dependency_depth
                        .checked_add(1)
                        .ok_or(DirectSdmaDependencyDepthErrorV1::Overflow)?,
                );
            }
        }
        if depth > MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 {
            Err(DirectSdmaDependencyDepthErrorV1::LimitExceeded)
        } else {
            Ok(depth)
        }
    }

    fn free_compute_lane_v1(&self) -> Option<usize> {
        (0..self.native_compute_lanes.len()).find(|lane| {
            let active = if *lane == 0 {
                self.active.is_some()
            } else {
                self.auxiliary_compute_lanes[*lane - 1].active.is_some()
            };
            !active
                && !self
                    .stream_compute_lanes
                    .values()
                    .any(|assigned| assigned == lane)
        })
    }

    fn active_compute_progress_roster_v1(&self) -> [bool; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1] {
        debug_assert_eq!(
            self.native_compute_lanes.len(),
            KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1
        );
        [
            self.active.is_some(),
            self.auxiliary_compute_lanes[0].active.is_some(),
        ]
    }

    fn lease_compute_lane_v1(&mut self, stream: u64, lane: usize) {
        let replaced = self.stream_compute_lanes.insert(stream, lane);
        debug_assert!(replaced.is_none());
        if lane != 0 {
            let replaced = self.auxiliary_compute_lanes[lane - 1]
                .owner_stream
                .replace(stream);
            debug_assert!(replaced.is_none());
        }
    }

    fn release_compute_lane_lease_v1(&mut self, stream: u64, lane: usize) {
        debug_assert_eq!(self.stream_compute_lanes.remove(&stream), Some(lane));
        if lane != 0 {
            debug_assert_eq!(
                self.auxiliary_compute_lanes[lane - 1].owner_stream.take(),
                Some(stream)
            );
        }
    }

    fn release_compute_dependency_retains_v1(&mut self, dependencies: &[u64]) {
        for dependency in dependencies {
            let remove = {
                let count = self
                    .compute_dependency_retain_counts
                    .get_mut(dependency)
                    .expect("pending compute dependency remains retained");
                *count = count
                    .checked_sub(1)
                    .expect("positive compute dependency retain count");
                *count == 0
            };
            if remove {
                self.compute_dependency_retain_counts.remove(dependency);
            }
        }
    }

    fn remove_pending_compute_from_stream_v1(&mut self, stream: u64, submission: u64) {
        let queue = self
            .pending_compute_streams
            .get_mut(&stream)
            .expect("pending compute retains its stream FIFO");
        if queue.front() == Some(&submission) {
            queue.pop_front();
        } else if queue.back() == Some(&submission) {
            queue.pop_back();
        } else {
            let position = queue
                .iter()
                .position(|candidate| *candidate == submission)
                .expect("pending compute is indexed by its stream FIFO");
            queue.remove(position);
        }
        if queue.is_empty() {
            self.pending_compute_streams.remove(&stream);
        }
    }

    fn restore_stream_tail_before_v1(&mut self, stream: u64, removed: u64, prior: Option<u64>) {
        if self.stream_submission_tails.get(&stream) != Some(&removed) {
            return;
        }
        match prior {
            Some(prior) => {
                self.stream_submission_tails.insert(stream, prior);
            }
            None => {
                self.stream_submission_tails.remove(&stream);
            }
        }
    }

    fn settle_unpublished_compute_v1(
        &mut self,
        pending: PendingComputeSubmissionV1,
        status: BackendPollV1,
    ) -> BackendPollV1 {
        self.remove_pending_compute_from_stream_v1(pending.launch.stream, pending.id);
        self.release_compute_dependency_retains_v1(&pending.dependencies);
        self.release_compute_custody_v1(
            pending.id,
            pending.module,
            pending.retained_allocations.iter().copied(),
        );
        self.submissions.insert(
            pending.id,
            SubmissionRecordV1 {
                stream: pending.launch.stream,
                status,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("accepted compute reserves one completion slot");
        status
    }

    fn compute_lane_caches_allocation_v1(&self, lane: usize, allocation: u64) -> bool {
        let (recycled, resident) = if lane == 0 {
            (self.recycled_dispatch.as_ref(), self.resident_data.as_ref())
        } else {
            let state = &self.auxiliary_compute_lanes[lane - 1];
            (
                state.recycled_dispatch.as_ref(),
                state.resident_data.as_ref(),
            )
        };
        recycled.is_some_and(|recycled| {
            recycled
                .descriptors
                .iter()
                .any(|descriptor| descriptor.allocation == allocation)
        }) || resident.is_some_and(|resident| {
            resident
                .descriptors
                .iter()
                .any(|descriptor| descriptor.allocation == allocation)
        })
    }

    fn release_compute_lane_cache_v1(
        &mut self,
        lane: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_compute_lane_state_v1(lane, |backend| {
            backend.detach_recycled_dispatch()?;
            backend.release_resident_data()
        })
    }

    fn release_all_compute_caches_for_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        for lane in 0..self.native_compute_lanes.len() {
            if self.compute_lane_caches_allocation_v1(lane, allocation) {
                self.release_compute_lane_cache_v1(lane)?;
            }
        }
        Ok(())
    }

    fn with_compute_lane_state_v1<R>(
        &mut self,
        lane: usize,
        operation: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if lane == 0 {
            let prior = core::mem::replace(&mut self.selected_compute_lane, 0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
            self.selected_compute_lane = prior;
            return match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            };
        }
        let index = lane - 1;
        let auxiliary = &mut self.auxiliary_compute_lanes[index];
        core::mem::swap(&mut self.active, &mut auxiliary.active);
        core::mem::swap(&mut self.resident_data, &mut auxiliary.resident_data);
        core::mem::swap(
            &mut self.recycled_dispatch,
            &mut auxiliary.recycled_dispatch,
        );
        let prior = core::mem::replace(&mut self.selected_compute_lane, lane);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
        self.selected_compute_lane = prior;
        let auxiliary = &mut self.auxiliary_compute_lanes[index];
        core::mem::swap(&mut self.active, &mut auxiliary.active);
        core::mem::swap(&mut self.resident_data, &mut auxiliary.resident_data);
        core::mem::swap(
            &mut self.recycled_dispatch,
            &mut auxiliary.recycled_dispatch,
        );
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn selected_native_compute_lane_v1(
        &self,
    ) -> Result<ComputeAqlQueueLaneV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.native_compute_lanes
            .get(self.selected_compute_lane)
            .copied()
            .flatten()
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "selected KFD compute queue has not been materialized",
                )
            })
    }

    fn ensure_sdma_queue_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native KFD SDMA is unavailable on a synthetic backend",
            ));
        }
        if self.any_compute_active_v1() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cannot change native SDMA ownership while compute is pending",
            ));
        }
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            self.sdma_enabled = true;
            return Ok(());
        }
        if self.queue.is_none() {
            let device = self.admitted_device.take().ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "the admitted KFD queue lifecycle has already retired",
                )
            })?;
            let queue = device
                .create_compute_aql_queue(KFD_RUNTIME_RING_BYTES_V1)
                .map_err(|error| self.terminal_error(format!("KFD queue creation: {error}")))?;
            self.queue = Some(queue);
        }
        if !self.sdma_enabled {
            self.queue
                .as_mut()
                .expect("native queue was established")
                .enable_gfx942_directional_sdma_copy_engines()
                .map_err(|error| {
                    self.terminal_error(format!("KFD directional SDMA creation: {error}"))
                })?;
            self.sdma_enabled = true;
        }
        Ok(())
    }

    fn directional_sdma_ops_v1(&mut self) -> kfd_backend_sdma_seam::DirectionalSdmaOpsV1<'_> {
        #[cfg(test)]
        if let Some(driver) = self.scripted_sdma.as_mut() {
            return kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Scripted(driver);
        }
        kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Native(
            self.queue
                .as_mut()
                .expect("native directional SDMA ownership retains its queue"),
        )
    }

    fn retain_sdma_seam_terminal_v1(
        &mut self,
        custody: kfd_backend_sdma_seam::SdmaTerminalCustodyV1,
    ) {
        let custody = match custody {
            kfd_backend_sdma_seam::SdmaTerminalCustodyV1::Native(custody) => match custody {
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::Promotion(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::Promotion(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::Demotion(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::Demotion(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::Submission(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::Submission(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::Retirement {
                    failure,
                    host,
                } => KfdRuntimeTerminalSdmaCustodyV1::Retirement { failure, host },
            },
            #[cfg(test)]
            kfd_backend_sdma_seam::SdmaTerminalCustodyV1::Scripted(custody) => {
                KfdRuntimeTerminalSdmaCustodyV1::Scripted(custody)
            }
        };
        self.retain_terminal_sdma_custody_v1(custody);
    }

    fn direct_sdma_direction_for_active_v1(
        &self,
        active: &ActiveSdmaCopyV1,
    ) -> Result<Gfx942PersistentSdmaDirectionV1, &'static str> {
        let source = self
            .allocations
            .get(&active.source)
            .ok_or("SDMA source allocation disappeared")?;
        let destination = self
            .allocations
            .get(&active.destination)
            .ok_or("SDMA destination allocation disappeared")?;
        direct_sdma_direction_v1(source.kind, destination.kind)
            .ok_or("unsupported SDMA direction reached publication")
    }

    fn take_directional_sdma_storage_v1(
        &mut self,
        active: &ActiveSdmaCopyV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        owner: KfdRuntimeSdmaInFlightV1,
    ) -> Result<DirectionalSdmaPairOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let (host_id, device_id) = directional_sdma_allocation_ids_v1(active, direction);
        let host_ready = self.allocations.get(&host_id).is_some_and(|record| {
            record
                .sdma_storage
                .is_available_for_kind_v1(RuntimeMemoryKindV1::HostVisible)
        });
        let device_ready = self.allocations.get(&device_id).is_some_and(|record| {
            record
                .sdma_storage
                .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
        });
        if !host_ready || !device_ready {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "directional persistent SDMA storage is retained by pending work",
            ));
        }
        let host = match std::mem::replace(
            &mut self
                .allocations
                .get_mut(&host_id)
                .expect("preflighted host allocation remains indexed")
                .sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(owner),
        ) {
            KfdRuntimeSdmaStorageV1::Host(host) => host,
            _ => unreachable!("preflighted host storage remains available"),
        };
        let device = match std::mem::replace(
            &mut self
                .allocations
                .get_mut(&device_id)
                .expect("preflighted device allocation remains indexed")
                .sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(owner),
        ) {
            KfdRuntimeSdmaStorageV1::Device(device) => *device,
            _ => unreachable!("preflighted device storage remains available"),
        };
        Ok(DirectionalSdmaPairOwnerV1 { device, host })
    }

    fn restore_directional_sdma_storage_v1(
        &mut self,
        active: &ActiveSdmaCopyV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        owner: KfdRuntimeSdmaInFlightV1,
        pair: DirectionalSdmaPairOwnerV1,
        destination_dirty: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let (host_id, device_id) = directional_sdma_allocation_ids_v1(active, direction);
        let host_slot_matches = self.allocations.get(&host_id).is_some_and(|record| {
            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(actual) if actual == owner)
        });
        let device_slot_matches = self.allocations.get(&device_id).is_some_and(|record| {
            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(actual) if actual == owner)
        });
        if !host_slot_matches || !device_slot_matches {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Pair {
                device: pair.device,
                host: pair.host,
            });
            return Err(self.terminal_error(
                "directional persistent SDMA restoration slot changed unexpectedly",
            ));
        }
        self.allocations
            .get_mut(&host_id)
            .expect("preflighted host allocation remains indexed")
            .sdma_storage = KfdRuntimeSdmaStorageV1::Host(pair.host);
        self.allocations
            .get_mut(&device_id)
            .expect("preflighted device allocation remains indexed")
            .sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(pair.device));
        if destination_dirty {
            let destination = self
                .allocations
                .get_mut(&active.destination)
                .expect("active destination allocation remains indexed");
            destination.sdma_shadow_dirty = true;
            destination.content_sha256 = None;
            destination.last_full_host_write = None;
        }
        Ok(())
    }

    fn finish_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
        completed: DirectionalSdmaCompletedOwnerV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let direction = match self.direct_sdma_direction_for_active_v1(&active) {
            Ok(direction) => direction,
            Err(detail) => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                    completed,
                ));
                return Err(self.terminal_error(detail));
            }
        };
        if completed.direction() != direction || completed.copy_bytes() != active.packet_bytes {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                completed,
            ));
            return Err(self.terminal_error(
                "directional persistent SDMA completion metadata changed unexpectedly",
            ));
        }
        let pair =
            match self.directional_sdma_ops_v1().retire(completed) {
                Ok(pair) => pair,
                Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => {
                    self.retain_terminal_sdma_custody_v1(
                        KfdRuntimeTerminalSdmaCustodyV1::Completed(custody),
                    );
                    return Err(self
                        .terminal_error("directional persistent SDMA frontier retirement failed"));
                }
                Err(SdmaTransitionFailureV1::ProcessTeardown { custody, .. }) => {
                    self.retain_sdma_seam_terminal_v1(custody);
                    return Err(self
                        .terminal_error("directional persistent SDMA frontier retirement failed"));
                }
            };
        self.restore_directional_sdma_storage_v1(
            &active,
            direction,
            KfdRuntimeSdmaInFlightV1::Async(active.id),
            pair,
            true,
        )?;
        active.completed_bytes = active
            .completed_bytes
            .checked_add(u64::from(active.packet_bytes))
            .ok_or_else(|| self.terminal_error("SDMA copy progress overflow"))?;
        if active.completed_bytes < active.byte_len {
            // Poll only observes and returns exact custody. Explicit flush owns
            // every continuation publication.
            active.phase = ActiveDirectionalSdmaPhaseV1::Ready;
            active.packet_bytes = 0;
            self.active_sdma.insert(active.id, active);
            return Ok(BackendPollV1::Pending);
        }
        self.release_sdma_dependency_retains_v1(&active.dependencies);
        self.release_allocation_custody_v1(active.source, active.id);
        self.release_allocation_custody_v1(active.destination, active.id);
        self.release_active_sdma_stream_v1(active.stream, active.id);
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
            },
        );
        self.sdma_completion_reservations = self
            .sdma_completion_reservations
            .checked_sub(1)
            .expect("accepted SDMA copy reserves one completion slot");
        Ok(status)
    }

    fn release_sdma_dependency_retains_v1(&mut self, dependencies: &[u64]) {
        for dependency in dependencies {
            let remove = {
                let count = self
                    .sdma_dependency_retain_counts
                    .get_mut(dependency)
                    .expect("active SDMA dependency remains retained");
                *count = count.checked_sub(1).expect("positive SDMA retain count");
                *count == 0
            };
            if remove {
                self.sdma_dependency_retain_counts.remove(dependency);
            }
        }
    }

    fn fail_unpublished_sdma_copy_v1(&mut self, active: ActiveSdmaCopyV1) -> BackendPollV1 {
        self.release_sdma_dependency_retains_v1(&active.dependencies);
        self.release_allocation_custody_v1(active.source, active.id);
        self.release_allocation_custody_v1(active.destination, active.id);
        self.release_active_sdma_stream_v1(active.stream, active.id);
        let status = BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
        };
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
            },
        );
        self.sdma_completion_reservations = self
            .sdma_completion_reservations
            .checked_sub(1)
            .expect("accepted SDMA copy reserves one completion slot");
        status
    }

    fn quiescent_sdma_marker_capacity_is_reserved_v1(&self) -> bool {
        self.quiescent_sdma_submissions.capacity()
            >= self
                .quiescent_sdma_submissions
                .len()
                .saturating_add(self.sdma_completion_reservations)
    }

    fn fail_quiescent_sdma_copy_v1(&mut self, active: ActiveSdmaCopyV1) {
        let id = active.id;
        let _ = self.fail_unpublished_sdma_copy_v1(active);
        let inserted = self.quiescent_sdma_submissions.insert(id);
        debug_assert!(inserted, "quiescent SDMA result is marked exactly once");
        debug_assert!(self.quiescent_sdma_marker_capacity_is_reserved_v1());
    }

    fn publish_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        for allocation in [active.source, active.destination] {
            if let Err(failure) = self.synchronize_native_allocation_v1(allocation) {
                if active.completed_bytes != 0 {
                    self.fail_quiescent_sdma_copy_v1(active);
                    return Err(match failure {
                        RuntimeBackendFailureV1::Rejected(error) => {
                            RuntimeBackendFailureV1::Quiescent(error)
                        }
                        failure => failure,
                    });
                }
                return match failure {
                    RuntimeBackendFailureV1::Rejected(_)
                    | RuntimeBackendFailureV1::Quiescent(_) => {
                        Ok(self.fail_unpublished_sdma_copy_v1(active))
                    }
                    failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
                };
            }
        }
        let direction = match self.direct_sdma_direction_for_active_v1(&active) {
            Ok(direction) => direction,
            Err(_) => return Ok(self.fail_unpublished_sdma_copy_v1(active)),
        };
        let Some(packet) = direct_sdma_packet_plan_v1(&active, direction) else {
            return Ok(self.fail_unpublished_sdma_copy_v1(active));
        };
        let pair = match self.take_directional_sdma_storage_v1(
            &active,
            direction,
            KfdRuntimeSdmaInFlightV1::Async(active.id),
        ) {
            Ok(custody) => custody,
            Err(failure) if active.completed_bytes != 0 => {
                self.fail_quiescent_sdma_copy_v1(active);
                return Err(match failure {
                    RuntimeBackendFailureV1::Rejected(error) => {
                        RuntimeBackendFailureV1::Quiescent(error)
                    }
                    failure => failure,
                });
            }
            Err(_) => return Ok(self.fail_unpublished_sdma_copy_v1(active)),
        };
        match self.directional_sdma_ops_v1().submit(
            pair,
            direction,
            packet.host_offset,
            packet.device_offset,
            packet.copy_bytes,
        ) {
            Ok(submission) => {
                active.packet_bytes = packet.copy_bytes;
                active.phase = ActiveDirectionalSdmaPhaseV1::Published(Box::new(submission));
                self.active_sdma.insert(active.id, active);
                Ok(BackendPollV1::Pending)
            }
            Err(failure) => match failure {
                SdmaTransitionFailureV1::Retryable { detail, custody } => {
                    let detail = format!("KFD directional SDMA publication: {detail}");
                    self.restore_directional_sdma_storage_v1(
                        &active,
                        direction,
                        KfdRuntimeSdmaInFlightV1::Async(active.id),
                        custody,
                        false,
                    )?;
                    if active.completed_bytes != 0 {
                        self.fail_quiescent_sdma_copy_v1(active);
                        Err(Self::quiescent_error(
                            KfdRuntimeBackendErrorKindV1::Native,
                            detail,
                        ))
                    } else {
                        Ok(self.fail_unpublished_sdma_copy_v1(active))
                    }
                }
                SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                    let detail = format!("KFD directional SDMA publication: {detail}");
                    self.retain_sdma_seam_terminal_v1(custody);
                    Err(self.terminal_error(detail))
                }
            },
        }
    }

    fn progress_unpublished_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        while let Some(dependency) = active.dependencies.get(active.dependency_cursor).copied() {
            let status = match self.poll_v1(dependency) {
                Ok(status) => status,
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.active_sdma.insert(active.id, active);
                    return Err(failure);
                }
                Err(failure @ RuntimeBackendFailureV1::Quiescent(_)) => {
                    self.fail_quiescent_sdma_copy_v1(active);
                    return Err(failure);
                }
            };
            match status {
                BackendPollV1::Succeeded => active.dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.active_sdma.insert(active.id, active);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => {
                    return Ok(self.fail_unpublished_sdma_copy_v1(active));
                }
            }
        }
        self.publish_sdma_copy_v1(active)
    }

    fn observe_unpublished_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        while let Some(dependency) = active.dependencies.get(active.dependency_cursor).copied() {
            let dependency_status = match self.poll_v1(dependency) {
                Ok(status) => status,
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.active_sdma.insert(active.id, active);
                    return Err(failure);
                }
                Err(failure @ RuntimeBackendFailureV1::Quiescent(_)) => {
                    self.fail_quiescent_sdma_copy_v1(active);
                    return Err(failure);
                }
            };
            match dependency_status {
                BackendPollV1::Succeeded => active.dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.active_sdma.insert(active.id, active);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => {
                    return Ok(self.fail_unpublished_sdma_copy_v1(active));
                }
            }
        }
        self.active_sdma.insert(active.id, active);
        Ok(BackendPollV1::Pending)
    }

    fn recycle_transient_sdma_buffer_v1(
        &mut self,
        buffer: SdmaBufferOwnerV1,
        operation: &'static str,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match self.directional_sdma_ops_v1().recycle(buffer) {
            Ok(()) => Ok(()),
            Err(SdmaRecycleFailureV1::Recovered { detail, buffer }) => {
                // No logical handle can own a transient after this point.
                // Retain its explicit custody until fail-closed teardown.
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(
                    buffer,
                ));
                Err(self.terminal_error(format!(
                    "KFD {operation} transient release became ambiguous: {detail}"
                )))
            }
            Err(SdmaRecycleFailureV1::Ambiguous { detail }) => Err(self.terminal_error(format!(
                "KFD {operation} transient release became ambiguous: {detail}"
            ))),
            #[cfg(test)]
            Err(SdmaRecycleFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                Err(self.terminal_error(format!(
                    "KFD {operation} transient release became ambiguous: {detail}"
                )))
            }
        }
    }

    fn release_sdma_storage_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let storage = {
            let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD allocation",
                )
            })?;
            if matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic) {
                return Ok(());
            }
            if matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(_)) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent SDMA allocation is retained by pending work",
                ));
            }
            std::mem::replace(
                &mut record.sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
            )
        };
        let buffer = match storage {
            KfdRuntimeSdmaStorageV1::Host(buffer)
            | KfdRuntimeSdmaStorageV1::DemotedDevice(buffer) => buffer,
            KfdRuntimeSdmaStorageV1::Device(device) => {
                match self.directional_sdma_ops_v1().demote(*device) {
                    Ok(buffer) => buffer,
                    Err(failure) => {
                        return match failure {
                            SdmaTransitionFailureV1::Retryable {
                                detail,
                                custody: device,
                            } => {
                                self.allocations
                                    .get_mut(&allocation)
                                    .expect("retryable demotion allocation remains indexed")
                                    .sdma_storage =
                                    KfdRuntimeSdmaStorageV1::Device(Box::new(device));
                                Err(Self::quiescent_error(
                                    KfdRuntimeBackendErrorKindV1::Native,
                                    format!("KFD persistent device demotion: {detail}"),
                                ))
                            }
                            SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                                self.retain_sdma_seam_terminal_v1(custody);
                                Err(self.terminal_error(format!(
                                    "KFD persistent device demotion: {detail}"
                                )))
                            }
                        };
                    }
                }
            }
            KfdRuntimeSdmaStorageV1::Synthetic | KfdRuntimeSdmaStorageV1::InFlight(_) => {
                unreachable!("preflighted releasable SDMA storage")
            }
        };
        let kind = self
            .allocations
            .get(&allocation)
            .expect("released native allocation remains indexed")
            .kind;
        match self.directional_sdma_ops_v1().recycle(buffer) {
            Ok(()) => Ok(()),
            Err(SdmaRecycleFailureV1::Recovered { detail, buffer }) => {
                self.allocations
                    .get_mut(&allocation)
                    .expect("recoverable recycle allocation remains indexed")
                    .sdma_storage = match kind {
                    RuntimeMemoryKindV1::HostVisible => KfdRuntimeSdmaStorageV1::Host(buffer),
                    RuntimeMemoryKindV1::DeviceLocal => {
                        KfdRuntimeSdmaStorageV1::DemotedDevice(buffer)
                    }
                };
                Err(Self::quiescent_error(
                    KfdRuntimeBackendErrorKindV1::Native,
                    format!("KFD persistent allocation recycle rejected: {detail}"),
                ))
            }
            Err(SdmaRecycleFailureV1::Ambiguous { detail }) => Err(self.terminal_error(format!(
                "KFD persistent allocation recycle became ambiguous: {detail}"
            ))),
            #[cfg(test)]
            Err(SdmaRecycleFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                Err(self.terminal_error(format!(
                    "KFD persistent allocation recycle became ambiguous: {detail}"
                )))
            }
        }
    }

    fn discard_hidden_sdma_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.release_sdma_storage_v1(allocation)?;
        let removed = self
            .allocations
            .remove(&allocation)
            .expect("hidden allocation remains indexed after native cleanup");
        self.staged_context_bytes = self
            .staged_context_bytes
            .checked_sub(removed.bytes.len() as u64)
            .expect("hidden allocation remains in staged-byte accounting");
        Ok(())
    }

    fn restore_synchronous_directional_storage_v1(
        &mut self,
        allocation: u64,
        pair: DirectionalSdmaPairOwnerV1,
    ) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let slot_matches = self.allocations.get(&allocation).is_some_and(|record| {
            matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
            )
        });
        if !slot_matches {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Pair {
                device: pair.device,
                host: pair.host,
            });
            return Err(self.terminal_error(
                "synchronous directional SDMA restoration slot changed unexpectedly",
            ));
        }
        self.allocations
            .get_mut(&allocation)
            .expect("synchronous device allocation remains indexed")
            .sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(pair.device));
        Ok(pair.host)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_synchronous_directional_sdma_v1(
        &mut self,
        allocation: u64,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: SdmaBufferOwnerV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        operation: &'static str,
    ) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let device_ready = self.allocations.get(&allocation).is_some_and(|record| {
            record
                .sdma_storage
                .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
        });
        if !device_ready {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "persistent device allocation is retained by pending work",
            ));
        }
        let device = match std::mem::replace(
            &mut self
                .allocations
                .get_mut(&allocation)
                .expect("preflighted synchronous device remains indexed")
                .sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
        ) {
            KfdRuntimeSdmaStorageV1::Device(device) => *device,
            _ => unreachable!("preflighted synchronous device remains available"),
        };
        let submission = match self.directional_sdma_ops_v1().submit(
            DirectionalSdmaPairOwnerV1 { device, host },
            direction,
            host_offset,
            device_offset,
            copy_bytes,
        ) {
            Ok(submission) => submission,
            Err(failure) => {
                return match failure {
                    SdmaTransitionFailureV1::Retryable { detail, custody } => {
                        let host =
                            self.restore_synchronous_directional_storage_v1(allocation, custody)?;
                        self.recycle_transient_sdma_buffer_v1(host, operation)?;
                        Err(Self::rejected(
                            KfdRuntimeBackendErrorKindV1::Native,
                            format!("KFD {operation} publication: {detail}"),
                        ))
                    }
                    SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                        self.retain_sdma_seam_terminal_v1(custody);
                        Err(self.terminal_error(format!("KFD {operation} publication: {detail}")))
                    }
                };
            }
        };
        let completed = match self
            .directional_sdma_ops_v1()
            .wait(submission, Duration::from_secs(30))
        {
            Ok(completed) => completed,
            Err(DirectionalSdmaExecutionFailureV1::Retryable { detail, submission }) => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Pending(
                    submission,
                ));
                return Err(self.terminal_error(format!(
                    "KFD {operation} completion became ambiguous: {detail}"
                )));
            }
            Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                return Err(self.terminal_error(format!(
                    "KFD {operation} completion became ambiguous: {detail}"
                )));
            }
        };
        if completed.direction() != direction || completed.copy_bytes() != copy_bytes {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                completed,
            ));
            return Err(self.terminal_error(format!(
                "KFD {operation} completion metadata changed unexpectedly"
            )));
        }
        let pair = match self.directional_sdma_ops_v1().retire(completed) {
            Ok(pair) => pair,
            Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                    custody,
                ));
                return Err(
                    self.terminal_error(format!("KFD {operation} frontier retirement failed"))
                );
            }
            Err(SdmaTransitionFailureV1::ProcessTeardown { custody, .. }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                return Err(
                    self.terminal_error(format!("KFD {operation} frontier retirement failed"))
                );
            }
        };
        self.restore_synchronous_directional_storage_v1(allocation, pair)
    }

    fn upload_sdma_range_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if bytes.is_empty()
            || !self
                .allocations
                .get(&allocation)
                .is_some_and(|record| record.sdma_backed)
        {
            return Ok(());
        }
        if bytes.len() > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize {
            let mut completed_chunks = 0_usize;
            for (index, chunk) in bytes
                .chunks(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
                .enumerate()
            {
                let delta = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or_else(|| Self::capacity("SDMA upload chunk offset overflow"))?;
                let result = self.upload_sdma_range_v1(
                    allocation,
                    byte_offset
                        .checked_add(delta)
                        .ok_or_else(|| Self::capacity("SDMA upload offset overflow"))?,
                    chunk,
                );
                match result {
                    Ok(()) => completed_chunks += 1,
                    Err(failure) => {
                        if completed_chunks != 0
                            && let Some(record) = self.allocations.get_mut(&allocation)
                            && record.kind == RuntimeMemoryKindV1::DeviceLocal
                        {
                            record.sdma_shadow_dirty = true;
                            record.content_sha256 = None;
                            record.last_full_host_write = None;
                        }
                        return Err(classify_sdma_chunk_failure_v1(completed_chunks, failure));
                    }
                }
            }
            return Ok(());
        }
        let is_host = self
            .allocations
            .get(&allocation)
            .is_some_and(|record| matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Host(_)));
        if is_host {
            let buffer = match &mut self
                .allocations
                .get_mut(&allocation)
                .expect("admitted host allocation remains indexed")
                .sdma_storage
            {
                KfdRuntimeSdmaStorageV1::Host(buffer) => buffer,
                _ => unreachable!("checked host storage"),
            };
            let result = {
                #[cfg(test)]
                let scripted = self.scripted_sdma.as_mut();
                #[cfg(test)]
                let mut ops = if let Some(driver) = scripted {
                    kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Scripted(driver)
                } else {
                    kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Native(
                        self.queue
                            .as_mut()
                            .expect("persistent SDMA allocation retains queue"),
                    )
                };
                #[cfg(not(test))]
                let mut ops = kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Native(
                    self.queue
                        .as_mut()
                        .expect("persistent SDMA allocation retains queue"),
                );
                ops.write_host(buffer, byte_offset, bytes)
            };
            return result.map_err(|error| {
                self.terminal_error(format!("KFD persistent host write: {error}"))
            });
        }

        let copy_bytes = u32::try_from(bytes.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "SDMA upload exceeds one admitted linear packet",
            )
        })?;
        let mut staging = self
            .directional_sdma_ops_v1()
            .allocate_host(bytes.len())
            .map_err(|error| self.terminal_error(format!("KFD upload staging: {error}")))?;
        if let Err(error) = self
            .directional_sdma_ops_v1()
            .write_host(&mut staging, 0, bytes)
        {
            self.recycle_transient_sdma_buffer_v1(staging, "upload")?;
            return Err(self.terminal_error(format!("KFD upload staging write: {error}")));
        }
        let staging = self.execute_synchronous_directional_sdma_v1(
            allocation,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            staging,
            0,
            byte_offset,
            copy_bytes,
            "upload",
        )?;
        self.recycle_transient_sdma_buffer_v1(staging, "upload")
    }

    fn zero_sdma_range_v1(
        &mut self,
        allocation: u64,
        byte_len: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let chunk_len =
            usize::try_from(byte_len.min(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)))
                .expect("bounded zero staging length fits usize");
        let zeros = try_zeroed_staging_v1(chunk_len)?;
        let mut offset = 0_u64;
        while offset < byte_len {
            let remaining = byte_len - offset;
            let this_len = usize::try_from(remaining.min(chunk_len as u64))
                .expect("bounded zero chunk fits usize");
            if let Err(failure) = self.upload_sdma_range_v1(allocation, offset, &zeros[..this_len])
            {
                if offset != 0 {
                    let record = self
                        .allocations
                        .get_mut(&allocation)
                        .expect("partially zeroed allocation remains indexed");
                    record.sdma_shadow_dirty = true;
                    record.content_sha256 = None;
                    record.last_full_host_write = None;
                    return Err(Self::after_possible_host_mutation(failure));
                }
                return Err(failure);
            }
            offset = offset
                .checked_add(this_len as u64)
                .expect("zeroed range progress fits allocation extent");
        }
        Ok(())
    }

    fn download_sdma_range_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if destination.is_empty()
            || !self
                .allocations
                .get(&allocation)
                .is_some_and(|record| record.sdma_backed)
        {
            return Ok(false);
        }
        if destination.len() > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize {
            let mut completed_chunks = 0_usize;
            for (index, chunk) in destination
                .chunks_mut(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
                .enumerate()
            {
                let delta = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or_else(|| Self::capacity("SDMA download chunk offset overflow"))?;
                let result = self.download_sdma_range_v1(
                    allocation,
                    byte_offset
                        .checked_add(delta)
                        .ok_or_else(|| Self::capacity("SDMA download offset overflow"))?,
                    chunk,
                );
                match result {
                    Ok(_) => completed_chunks += 1,
                    Err(failure) => {
                        return Err(classify_sdma_chunk_failure_v1(completed_chunks, failure));
                    }
                }
            }
            return Ok(true);
        }
        let is_host = self
            .allocations
            .get(&allocation)
            .is_some_and(|record| matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Host(_)));
        if is_host {
            let buffer = match &self
                .allocations
                .get(&allocation)
                .expect("admitted host allocation remains indexed")
                .sdma_storage
            {
                KfdRuntimeSdmaStorageV1::Host(buffer) => buffer,
                _ => unreachable!("checked host storage"),
            };
            let result = {
                #[cfg(test)]
                let scripted = self.scripted_sdma.as_mut();
                #[cfg(test)]
                let mut ops = if let Some(driver) = scripted {
                    kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Scripted(driver)
                } else {
                    kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Native(
                        self.queue
                            .as_mut()
                            .expect("persistent SDMA allocation retains queue"),
                    )
                };
                #[cfg(not(test))]
                let mut ops = kfd_backend_sdma_seam::DirectionalSdmaOpsV1::Native(
                    self.queue
                        .as_mut()
                        .expect("persistent SDMA allocation retains queue"),
                );
                ops.read_host(buffer, byte_offset, destination.len() as u64)
            };
            let bytes = result.map_err(|error| {
                self.terminal_error(format!("KFD persistent host read: {error}"))
            })?;
            destination.copy_from_slice(&bytes);
            return Ok(true);
        }

        let copy_bytes = u32::try_from(destination.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "SDMA download exceeds one admitted linear packet",
            )
        })?;
        let staging = self
            .directional_sdma_ops_v1()
            .allocate_host(destination.len())
            .map_err(|error| self.terminal_error(format!("KFD download staging: {error}")))?;
        let staging = self.execute_synchronous_directional_sdma_v1(
            allocation,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            staging,
            0,
            byte_offset,
            copy_bytes,
            "download",
        )?;
        let readback =
            self.directional_sdma_ops_v1()
                .read_host(&staging, 0, destination.len() as u64);
        let bytes = match readback {
            Ok(bytes) => bytes,
            Err(error) => {
                self.recycle_transient_sdma_buffer_v1(staging, "download")?;
                return Err(self.terminal_error(format!("KFD download readback: {error}")));
            }
        };
        destination.copy_from_slice(&bytes);
        self.recycle_transient_sdma_buffer_v1(staging, "download")?;
        Ok(true)
    }

    fn synchronize_sdma_shadow_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(byte_len) = self.allocations.get(&allocation).and_then(|record| {
            (record.sdma_backed && record.sdma_shadow_dirty).then_some(record.bytes.len())
        }) else {
            return Ok(());
        };
        let chunk_len = byte_len.min(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize);
        let mut staging = try_zeroed_staging_v1(chunk_len)?;
        {
            let record = self
                .allocations
                .get_mut(&allocation)
                .expect("persistent allocation remains indexed");
            record.content_sha256 = None;
            record.last_full_host_write = None;
        }
        let mut offset = 0_usize;
        while offset < byte_len {
            let this_len = (byte_len - offset).min(chunk_len);
            if let Err(failure) =
                self.download_sdma_range_v1(allocation, offset as u64, &mut staging[..this_len])
            {
                return Err(if offset == 0 {
                    failure
                } else {
                    Self::after_possible_host_mutation(failure)
                });
            }
            let record = self
                .allocations
                .get_mut(&allocation)
                .expect("persistent allocation remains indexed");
            Arc::make_mut(&mut record.bytes)[offset..offset + this_len]
                .copy_from_slice(&staging[..this_len]);
            offset += this_len;
        }
        self.allocations
            .get_mut(&allocation)
            .expect("persistent allocation remains indexed")
            .sdma_shadow_dirty = false;
        Ok(())
    }

    fn collect_compute_dependencies_v1(
        &self,
        stream: u64,
        dependencies: &[u64],
    ) -> Result<Vec<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(Self::capacity("KFD compute dependency capacity exceeded"));
        }
        let extra_tail = usize::from(self.stream_submission_tails.contains_key(&stream));
        let mut submissions = Vec::new();
        submissions
            .try_reserve_exact(dependencies.len().saturating_add(extra_tail))
            .map_err(|_| Self::capacity("KFD compute dependency allocation failed"))?;
        for event_handle in dependencies {
            let event = self.events.get(event_handle).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD event dependency",
                )
            })?;
            if submissions.contains(&event.submission) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute dependencies must name distinct submissions",
                ));
            }
            let status = self
                .submissions
                .get(&event.submission)
                .map(|record| record.status)
                .or_else(|| {
                    (self.active_compute_lane_v1(event.submission).is_some()
                        || self.pending_compute.contains_key(&event.submission)
                        || self.active_sdma.contains_key(&event.submission))
                    .then_some(BackendPollV1::Pending)
                });
            match status {
                Some(BackendPollV1::Succeeded | BackendPollV1::Pending) => {}
                Some(BackendPollV1::Failed { .. }) => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "event dependency completed with failure",
                    ));
                }
                None => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::UnknownHandle,
                        "event refers to an unknown submission",
                    ));
                }
            }
            submissions.push(event.submission);
        }
        if let Some(tail) = self.stream_submission_tails.get(&stream).copied()
            && !submissions.contains(&tail)
        {
            if submissions.len() == MAX_RUNTIME_DEPENDENCIES_V1 {
                return Err(Self::capacity(
                    "KFD compute dependency capacity exceeded by stream ordering",
                ));
            }
            if self
                .submissions
                .get(&tail)
                .is_some_and(|record| matches!(record.status, BackendPollV1::Failed { .. }))
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "prior work in the KFD stream completed with failure",
                ));
            }
            submissions.push(tail);
        }
        Ok(submissions)
    }

    fn validate_compute_launch_v1(
        &self,
        launch: &BackendLaunchV1<'_>,
        dependencies: &[u64],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if launch.explicit_kernarg.len() > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(Self::capacity(
                "KFD explicit kernarg exceeds the runtime admission bound",
            ));
        }
        if launch.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
            return Err(Self::capacity(
                "KFD binding roster exceeds the host dispatch admission bound",
            ));
        }
        let stream_device = *self.streams.get(&launch.stream).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD stream",
            )
        })?;
        let kernel = self.kernels.get(&launch.kernel).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD kernel",
            )
        })?;
        let module = self.modules.get(&kernel.module).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "kernel module is no longer loaded",
            )
        })?;
        if module.device != stream_device {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "stream and kernel belong to different devices",
            ));
        }
        for binding in launch.bindings {
            if !native_sdma_region_is_admitted_v1(
                self.allocations.get(&binding.region.allocation),
                stream_device,
                binding.region,
            ) || binding.region.byte_len == 0
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute binding exceeds its retained allocation",
                ));
            }
        }

        if launch.bindings.iter().any(|binding| {
            self.allocation_has_unordered_custody_v1(
                binding.region.allocation,
                launch.stream,
                dependencies,
                None,
            )
        }) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping cross-stream compute/copy requires an explicit event dependency",
            ));
        }
        Ok(())
    }

    fn poll_compute_lane_v1(
        &mut self,
        lane: usize,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_compute_lane_state_v1(lane, |backend| {
            let mut active = backend.active.take().expect("selected active lane");
            let batch = active
                .batch
                .take()
                .expect("active submission retains batch");
            let native_lane = backend.selected_native_compute_lane_v1()?;
            let poll = backend
                .queue
                .as_mut()
                .expect("active submission retains queue")
                .with_compute_lane_v1(native_lane, |queue| queue.poll_fixed_dispatch(batch))
                .map_err(|error| {
                    backend.terminal_error(format!("KFD completion observation: {error}"))
                })?
                .map_err(|error| {
                    backend.terminal_error(format!("KFD completion observation: {error}"))
                })?;
            match poll {
                Gfx942DispatchPollV1::Pending(batch) => {
                    active.batch = Some(batch);
                    backend.active = Some(active);
                    Ok(BackendPollV1::Pending)
                }
                Gfx942DispatchPollV1::Ready(completed) => {
                    backend.finish_completed(active, completed)
                }
            }
        })
    }

    fn progress_pending_compute_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_head = self
            .pending_compute_streams
            .get(&pending.launch.stream)
            .and_then(|queue| queue.front())
            .is_some_and(|head| *head == pending.id);
        if !is_head {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        }
        while let Some(dependency) = pending.dependencies.get(pending.dependency_cursor).copied() {
            let status = match self.poll_v1(dependency) {
                Ok(status) => status,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    return Ok(self.settle_unpublished_compute_v1(
                        pending,
                        BackendPollV1::Failed { code: -1 },
                    ));
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            };
            match status {
                BackendPollV1::Succeeded => pending.dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.pending_compute.insert(pending.id, pending);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => {
                    return Ok(self.settle_unpublished_compute_v1(
                        pending,
                        BackendPollV1::Failed { code: -1 },
                    ));
                }
            }
        }
        let Some(lane) = self.free_compute_lane_v1() else {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        };
        let conflicting_compute_lane = (0..self.native_compute_lanes.len()).find(|lane| {
            let active = if *lane == 0 {
                self.active.as_ref()
            } else {
                self.auxiliary_compute_lanes[*lane - 1].active.as_ref()
            };
            launch_overlaps_active_compute_v1(&pending.launch.bindings, active.into_iter())
        });
        if let Some(conflicting_lane) = conflicting_compute_lane {
            self.pending_compute.insert(pending.id, pending);
            let _ = self.poll_compute_lane_v1(conflicting_lane)?;
            return Ok(BackendPollV1::Pending);
        }
        let conflicting_copy = self.published_sdma_conflict_v1(
            pending.id,
            pending.launch.stream,
            &pending.launch.bindings,
        );
        if let Some(copy) = conflicting_copy {
            return match self.poll_v1(copy) {
                Ok(_) => {
                    self.pending_compute.insert(pending.id, pending);
                    Ok(BackendPollV1::Pending)
                }
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    Ok(self
                        .settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    Err(failure)
                }
            };
        }
        let staging = (|| {
            for binding in &pending.launch.bindings {
                self.synchronize_native_allocation_v1(binding.region.allocation)?;
                for cached_lane in 0..self.native_compute_lanes.len() {
                    if cached_lane != lane
                        && self.compute_lane_caches_allocation_v1(
                            cached_lane,
                            binding.region.allocation,
                        )
                    {
                        self.release_compute_lane_cache_v1(cached_lane)?;
                    }
                }
            }
            Ok(())
        })();
        if let Err(failure) = staging {
            return match failure {
                RuntimeBackendFailureV1::Rejected(_) | RuntimeBackendFailureV1::Quiescent(_) => {
                    Ok(self
                        .settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
                }
                failure @ RuntimeBackendFailureV1::Terminal(_) => {
                    self.pending_compute.insert(pending.id, pending);
                    Err(failure)
                }
            };
        }
        self.lease_compute_lane_v1(pending.launch.stream, lane);
        let publication = self.with_compute_lane_state_v1(lane, |backend| {
            let prepared = backend.prepare_launch(pending.launch.borrowed())?;
            backend.publish(pending.id, pending.dependency_depth, prepared)
        });
        match publication {
            Ok(()) => {
                self.remove_pending_compute_from_stream_v1(pending.launch.stream, pending.id);
                self.release_compute_dependency_retains_v1(&pending.dependencies);
                Ok(BackendPollV1::Pending)
            }
            Err(RuntimeBackendFailureV1::Rejected(_) | RuntimeBackendFailureV1::Quiescent(_)) => {
                self.release_compute_lane_lease_v1(pending.launch.stream, lane);
                Ok(self.settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
            }
            Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                self.pending_compute.insert(pending.id, pending);
                Err(failure)
            }
        }
    }

    fn observe_pending_compute_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_head = self
            .pending_compute_streams
            .get(&pending.launch.stream)
            .and_then(|queue| queue.front())
            .is_some_and(|head| *head == pending.id);
        if !is_head {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        }
        while let Some(dependency) = pending.dependencies.get(pending.dependency_cursor).copied() {
            let status = match self.poll_v1(dependency) {
                Ok(status) => status,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    return Ok(self.settle_unpublished_compute_v1(
                        pending,
                        BackendPollV1::Failed { code: -1 },
                    ));
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            };
            match status {
                BackendPollV1::Succeeded => pending.dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.pending_compute.insert(pending.id, pending);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => {
                    return Ok(self.settle_unpublished_compute_v1(
                        pending,
                        BackendPollV1::Failed { code: -1 },
                    ));
                }
            }
        }
        self.pending_compute.insert(pending.id, pending);
        Ok(BackendPollV1::Pending)
    }

    fn pending_compute_can_publish_under_deadline_v1(&self, submission: u64) -> bool {
        let Some(pending) = self.pending_compute.get(&submission) else {
            return false;
        };
        if pending.dependency_cursor != pending.dependencies.len()
            || self.native_dirty_extents != 0
            || !pending.launch.bindings.iter().all(|binding| {
                self.allocations
                    .get(&binding.region.allocation)
                    .is_some_and(|allocation| {
                        !allocation.sdma_shadow_dirty && allocation.native_dirty.is_empty()
                    })
            })
        {
            return false;
        }
        true
    }

    fn prepare_launch(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<PreparedLaunchV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let preparation_started = Instant::now();
        let dispatch_shape_sha256 = dispatch_shape_sha256_v1(&launch, launch.semantic_launch);
        let profile_launch = KfdProfileLaunchV1 {
            grid: launch.geometry.grid,
            workgroup: launch.geometry.workgroup,
            dynamic_shared_bytes: launch.geometry.dynamic_shared_bytes,
        };
        let profile_semantic_contract = self
            .profiler
            .as_ref()
            .is_some_and(KfdRuntimeProfileRecorderV1::captures_semantic_profile)
            .then(|| profile_semantic_contract_v1(launch.semantic_launch, profile_launch))
            .flatten();
        let profile_bindings = self.prepare_profile_bindings_v1(launch.bindings);
        let stream_device = *self.streams.get(&launch.stream).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD stream",
            )
        })?;
        let mut synchronized = HashSet::new();
        for binding in launch.bindings {
            if synchronized.insert(binding.region.allocation) {
                self.synchronize_native_allocation_v1(binding.region.allocation)?;
                self.synchronize_sdma_shadow_v1(binding.region.allocation)?;
            }
        }
        let kernel = self.kernels.get(&launch.kernel).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD kernel",
            )
        })?;
        let module = self.modules.get(&kernel.module).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "kernel module is no longer loaded",
            )
        })?;
        if module.device != stream_device {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "stream and kernel belong to different devices",
            ));
        }
        let geometry = AqlDispatchGeometryV1::new(launch.geometry.grid, launch.geometry.workgroup)
            .map_err(|error| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    format!("invalid AQL geometry: {error:?}"),
                )
            })?;
        let closure = kernel.validated.validated();
        let inspected = closure.selected_kernel();
        let arguments = inspected.explicit_arguments();
        let global_argument_count = arguments
            .iter()
            .filter(|argument| argument.value_kind() == ExplicitValueKind::GlobalBuffer)
            .count();
        if global_argument_count != launch.bindings.len() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "typed binding roster does not cover every AMDHSA global buffer",
            ));
        }

        let snapshot_started = Instant::now();
        let staged = snapshot_bound_data_v1(&self.allocations, launch.bindings, stream_device)?;
        let bound_snapshot = snapshot_started.elapsed();
        let mut buffer_bindings = Vec::new();
        let mut abi_rows = Vec::new();
        let mut allocations = HashSet::new();
        let mut writebacks = Vec::new();
        let mut seen_argument_indices = HashSet::new();
        buffer_bindings
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD buffer-binding preparation allocation failed"))?;
        abi_rows
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD dispatch-ABI preparation allocation failed"))?;
        allocations
            .try_reserve(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD allocation-retention roster allocation failed"))?;
        writebacks
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD writeback roster allocation failed"))?;
        seen_argument_indices
            .try_reserve(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD argument-roster allocation failed"))?;

        for binding in launch.bindings {
            let region = binding.region;
            let (argument_index, argument) = arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| argument.offset() == u64::from(binding.kernarg_byte_offset))
                .ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "kernarg pointer patch does not match an AMDHSA global buffer",
                    )
                })?;
            if !seen_argument_indices.insert(argument_index) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "more than one binding targets the same AMDHSA argument",
                ));
            }
            if argument.value_kind() != ExplicitValueKind::GlobalBuffer {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "kernarg pointer patch targets a non-global AMDHSA argument",
                ));
            }
            let placement = staged.placements[&region.allocation];
            let staged_offset = region
                .byte_offset
                .checked_sub(placement.allocation_offset)
                .expect("staged allocation window starts before every bound range");
            buffer_bindings.push(Gfx942DispatchBufferBindingV1::new(
                argument_index,
                placement.data_index,
                staged_offset,
                region.byte_len,
            ));
            argument.name().ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "AMDHSA global buffer has no source argument name",
                )
            })?;
            abi_rows.push(OwnedAbiRowV1 {
                explicit_argument_index: argument_index,
                offset: argument.offset(),
                pointee_alignment: argument.pointee_alignment().unwrap_or(1),
                access: map_access_v1(region.access),
            });
            allocations.insert(region.allocation);
            if region.access != RuntimeAccessV1::Read {
                writebacks.push(WritebackV1 {
                    allocation: region.allocation,
                    allocation_offset: usize::try_from(region.byte_offset).map_err(|_| {
                        Self::rejected(
                            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                            "binding offset does not fit host address space",
                        )
                    })?,
                    data_index: placement.data_index,
                    data_offset: staged_offset,
                    byte_len: region.byte_len,
                });
            }
        }

        let total_kernarg =
            usize::try_from(closure.resources().kernarg_segment_size()).map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "kernarg size does not fit host address space",
                )
            })?;
        let explicit_len = launch.explicit_kernarg.len();
        match inspected.implicit_argument_offset() {
            Some(offset)
                if usize::try_from(offset).ok() == Some(explicit_len)
                    && usize::try_from(inspected.implicit_argument_size()).ok()
                        == Some(COV6_IMPLICIT_KERNARG_BYTES_V1)
                    && explicit_len.checked_add(COV6_IMPLICIT_KERNARG_BYTES_V1)
                        == Some(total_kernarg) => {}
            None if explicit_len == total_kernarg => {}
            _ => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "explicit kernarg does not match the inspected COV6 layout",
                ));
            }
        }
        let mut kernarg = Vec::new();
        kernarg
            .try_reserve_exact(total_kernarg)
            .map_err(|_| Self::capacity("KFD kernarg staging allocation failed"))?;
        kernarg.extend_from_slice(launch.explicit_kernarg);
        kernarg.resize(total_kernarg, 0);

        let mut authority_allocations = Vec::new();
        authority_allocations
            .try_reserve_exact(staged.data.len())
            .map_err(|_| Self::capacity("KFD authority allocation roster allocation failed"))?;
        for spec in &staged.data {
            authority_allocations.push(KfdRuntimeAuthorityAllocationV1 {
                allocation: spec.allocation,
                kind: spec.kind,
                alignment: spec.alignment,
                byte_offset: spec.allocation_offset,
                bytes: spec.bytes(),
                content_sha256: spec.content_sha256,
            });
        }
        let mut authority_abi = Vec::new();
        authority_abi
            .try_reserve_exact(abi_rows.len())
            .map_err(|_| Self::capacity("KFD authority ABI roster allocation failed"))?;
        for row in &abi_rows {
            let argument = &arguments[row.explicit_argument_index];
            authority_abi.push(KfdRuntimeAuthorityGlobalBufferV1 {
                explicit_argument_index: row.explicit_argument_index,
                name: argument
                    .name()
                    .expect("prepared global-buffer ABI row retains a source name"),
                kernarg_byte_offset: row.offset,
                pointee_alignment: row.pointee_alignment,
                access: row.access,
            });
        }
        let authority_started = Instant::now();
        let authorized = self
            .launch_gate
            .authorize_launch_v1(KfdRuntimeAuthorityRequestV1 {
                module_image: module.validated.bytes(),
                module_sha256: module.image_sha256,
                kernel_name: kernel.validated.selected_kernel().name(),
                signature: kernel.signature,
                explicit_kernarg: launch.explicit_kernarg,
                complete_kernarg_template: &kernarg,
                bindings: launch.bindings,
                dispatch_abi: &authority_abi,
                allocations: &authority_allocations,
                geometry: launch.geometry,
                semantic_launch: launch.semantic_launch,
            });
        let authority = authority_started.elapsed();
        if !authorized {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "direct KFD launch authority denied the exact invocation",
            ));
        }

        let preparation = preparation_started.elapsed();
        Ok(PreparedLaunchV1 {
            stream: launch.stream,
            kernel: launch.kernel,
            program: kernel.validated.clone(),
            signature: kernel.signature,
            kernarg: kernarg.into_boxed_slice(),
            geometry,
            dynamic_shared_bytes: launch.geometry.dynamic_shared_bytes,
            buffer_bindings: buffer_bindings.into_boxed_slice(),
            abi_rows,
            data: staged.data,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            performance: KfdRuntimeLaunchPerformanceV1 {
                preparation,
                bound_snapshot,
                authority,
                ..KfdRuntimeLaunchPerformanceV1::default()
            },
        })
    }

    fn publish(
        &mut self,
        id: u64,
        dependency_depth: usize,
        prepared: PreparedLaunchV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let PreparedLaunchV1 {
            stream,
            kernel,
            program,
            signature,
            kernarg,
            geometry,
            dynamic_shared_bytes,
            buffer_bindings,
            abi_rows,
            data,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            mut performance,
        } = prepared;
        for (index, writeback) in writebacks.iter().enumerate() {
            if writebacks[..index]
                .iter()
                .any(|prior| prior.allocation == writeback.allocation)
            {
                continue;
            }
            let required = writebacks[index..]
                .iter()
                .filter(|candidate| candidate.allocation == writeback.allocation)
                .count();
            self.allocations
                .get_mut(&writeback.allocation)
                .expect("prepared writeback allocation remains retained")
                .native_dirty
                .try_reserve(required)
                .map_err(|_| Self::capacity("KFD native-dirty extent reservation failed"))?;
        }
        let resident_descriptors = resident_descriptors_v1(&data)?;

        let native_binding_started = Instant::now();
        let creates_native_queue = self.native_compute_lanes[self.selected_compute_lane].is_none();
        let mut reused_attached = false;
        let reuse_attached = self.recycled_dispatch.as_ref().is_some_and(|recycled| {
            recycled_dispatch_reuse_is_admitted_v1(
                recycled,
                dispatch_shape_sha256,
                &resident_descriptors,
                &data,
            )
        });
        if self.recycled_dispatch.is_some() && !reuse_attached {
            self.detach_recycled_dispatch()?;
        }
        if reuse_attached {
            let recycled = self
                .recycled_dispatch
                .take()
                .expect("admitted attached dispatch remains retained");
            let overwrite = {
                let native_lane = self.selected_native_compute_lane_v1()?;
                let queue = self
                    .queue
                    .as_mut()
                    .expect("recycled dispatch retains queue");
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        queue
                            .recycled_fixed_dispatch_generation()
                            .map_err(|error| format!("KFD recycled generation: {error}"))
                            .and_then(|generation| {
                                recycled
                                    .descriptors
                                    .iter()
                                    .zip(&data)
                                    .enumerate()
                                    .try_for_each(|(index, (prior, spec))| {
                                        if !prior.device_may_have_modified
                                            && prior.host_content_sha256.is_some()
                                            && prior.host_content_sha256 == spec.content_sha256
                                        {
                                            return Ok(());
                                        }
                                        queue
                                            .overwrite_recycled_fixed_dispatch_host_data(
                                                Gfx942RecycledDispatchWriteRequestV1::new(
                                                    generation, index, 0,
                                                ),
                                                spec.bytes(),
                                            )
                                            .map_err(|error| {
                                                format!("KFD recycled-data overwrite: {error}")
                                            })
                                    })
                            })
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))
                    .and_then(core::convert::identity)
            };
            if let Err(detail) = overwrite {
                return Err(self.terminal_error(detail));
            }
            reused_attached = true;
        }

        if !reused_attached {
            let validated_program = build_program_v1(&program, signature, &abi_rows)?;
            let mut programs = Vec::new();
            programs
                .try_reserve_exact(1)
                .map_err(|_| Self::capacity("KFD program roster allocation failed"))?;
            programs.push(validated_program);
            let packet = Gfx942FixedDispatchPacketV1::new(
                0,
                geometry,
                dynamic_shared_bytes,
                kernarg,
                buffer_bindings,
            );
            if creates_native_queue && self.queue.is_none() {
                let device = self.admitted_device.take().ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Unsupported,
                        "the admitted KFD queue lifecycle has already retired",
                    )
                })?;
                let mut memory = device
                    .acquire_shared_gtt_memory_session()
                    .map_err(|error| self.terminal_error(format!("KFD VM acquisition: {error}")))?;
                let native_data = match materialize_initial_data_v1(&mut memory, data, signature) {
                    Ok(data) => data,
                    Err(detail) => {
                        self.terminal_memory = Some(memory);
                        return Err(self.terminal_error(detail));
                    }
                };
                let queue = memory
                    .create_compute_aql_queue_with_fixed_dispatch(
                        KFD_RUNTIME_RING_BYTES_V1,
                        programs,
                        [packet],
                        native_data,
                    )
                    .map_err(|error| self.terminal_error(format!("KFD queue creation: {error}")))?;
                let primary_lane = queue.primary_compute_lane_v1();
                self.queue = Some(queue);
                self.native_compute_lanes[self.selected_compute_lane] = Some(primary_lane);
            } else if creates_native_queue {
                let mut materialization_error = None;
                let lane = self
                    .queue
                    .as_mut()
                    .expect("shared KFD queue owner exists")
                    .create_auxiliary_compute_lane_with_fixed_dispatch(
                        KFD_RUNTIME_RING_BYTES_V1,
                        programs,
                        [packet],
                        |memory| {
                            materialize_initial_data_v1(memory, data, signature).map_err(|detail| {
                                materialization_error = Some(detail);
                                fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                                    "KFD auxiliary data materialization",
                                )
                            })
                        },
                    )
                    .map_err(|error| {
                        self.terminal_error(
                            materialization_error.unwrap_or_else(|| {
                                format!("KFD auxiliary queue creation: {error}")
                            }),
                        )
                    })?;
                self.native_compute_lanes[self.selected_compute_lane] = Some(lane);
            } else {
                let rebound = {
                    let native_lane = self.selected_native_compute_lane_v1()?;
                    let queue = self.queue.as_mut().expect("checked queue");
                    queue
                        .with_compute_lane_v1(native_lane, |queue| {
                            let native_data = match self.resident_data.take() {
                                Some(mut resident)
                                    if same_resident_storage_shape_v1(
                                        &resident.descriptors,
                                        &resident_descriptors,
                                    ) && data.iter().all(|spec| {
                                        spec.kind == RuntimeMemoryKindV1::HostVisible
                                    }) =>
                                {
                                    let overwrite = resident
                                        .data
                                        .iter_mut()
                                        .zip(resident.descriptors.iter().zip(&data))
                                        .enumerate()
                                        .try_for_each(|(index, (native, (prior, spec)))| {
                                            if !prior.device_may_have_modified
                                                && prior.host_content_sha256.is_some()
                                                && prior.host_content_sha256
                                                    == spec.content_sha256
                                            {
                                                return Ok(());
                                            }
                                            queue
                                                .overwrite_detached_initialized_host_visible_fixed_dispatch_data(
                                                    index,
                                                    native,
                                                    0,
                                                    spec.bytes(),
                                                )
                                                .map_err(|error| {
                                                    format!("KFD resident-data overwrite: {error}")
                                                })
                                        });
                                    overwrite.map(|()| resident.data)
                                }
                                Some(resident) => release_resident_data_v1(queue, resident)
                                    .and_then(|()| {
                                        materialize_rebound_data_v1(queue, data, signature)
                                    }),
                                None => materialize_rebound_data_v1(queue, data, signature),
                            };
                            native_data.and_then(|native_data| {
                                queue
                                    .bind_fixed_dispatch(programs, [packet], native_data)
                                    .map_err(|error| format!("KFD dispatch rebind: {error}"))
                            })
                        })
                        .map_err(|error| format!("KFD compute-lane selection: {error}"))
                        .and_then(core::convert::identity)
                };
                if let Err(detail) = rebound {
                    return Err(self.terminal_error(detail));
                }
            }
        }
        performance.native_binding = native_binding_started.elapsed();
        if creates_native_queue {
            let queue = self.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
            );
            self.observe_profile_v1(
                queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
            );
        }

        let publication_started = Instant::now();
        let native_lane = self.selected_native_compute_lane_v1()?;
        let batch = self
            .queue
            .as_mut()
            .expect("queue was created or rebound")
            .with_compute_lane_v1(native_lane, |queue| queue.submit_fixed_dispatch::<1>())
            .map_err(|error| self.terminal_error(format!("KFD compute-lane selection: {error}")))?
            .map_err(|error| self.terminal_error(format!("KFD dispatch publication: {error}")))?;
        performance.publication = publication_started.elapsed();
        let published_at = Instant::now();
        self.active = Some(ActiveSubmissionV1 {
            id,
            stream,
            kernel,
            dependency_depth,
            allocations,
            writebacks,
            resident_descriptors,
            dispatch_shape_sha256,
            published_at,
            performance,
            batch: Some(batch),
        });
        let profile_dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
        let profile_queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
        );
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
        let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
        let profile_event = match profile_bindings {
            Some(Ok(bindings)) => profile_dispatch
                .zip(profile_queue)
                .zip(profile_stream)
                .zip(profile_kernel)
                .zip(profile_shape)
                .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                    KfdRuntimeProfileEventKindV1::DispatchPublished {
                        dispatch,
                        queue,
                        stream,
                        kernel,
                        dispatch_shape,
                        launch: profile_launch,
                        bindings,
                    }
                }),
            Some(Err(())) => None,
            None => None,
        };
        self.observe_profile_dispatch_v1(profile_event, profile_semantic_contract);
        Ok(())
    }

    fn finish_completed(
        &mut self,
        mut active: ActiveSubmissionV1,
        completed: fe2o3_kfd::Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        active.performance.publish_to_completion = active.published_at.elapsed();
        let compute_lane = self.selected_compute_lane;
        let native_lane = self.selected_native_compute_lane_v1()?;
        let native_result = (|| -> Result<_, String> {
            let queue = self
                .queue
                .as_mut()
                .expect("active submission retains queue");
            let recycle_started = Instant::now();
            queue
                .with_compute_lane_v1(native_lane, |queue| queue.recycle_fixed_dispatch(completed))
                .map_err(|error| format!("KFD compute-lane selection: {error}"))?
                .map_err(|error| format!("KFD completion recycle: {error}"))?;
            let initial_recycle = recycle_started.elapsed();
            Ok(initial_recycle)
        })();
        let recycle = match native_result {
            Ok(result) => result,
            Err(detail) => return Err(self.terminal_error(detail)),
        };
        active.performance.completed_readback = Duration::ZERO;
        active.performance.recycle = recycle;
        for writeback in &active.writebacks {
            self.native_dirty_extents = self
                .native_dirty_extents
                .checked_add(1)
                .expect("native-dirty extent count is memory-bounded");
            let record = self
                .allocations
                .get_mut(&writeback.allocation)
                .expect("active allocation remains retained");
            record.content_sha256 = None;
            record.native_dirty.push(NativeDirtyExtentV1 {
                compute_lane,
                data_index: writeback.data_index,
                allocation_offset: writeback.allocation_offset,
                data_offset: writeback.data_offset,
                byte_len: writeback.byte_len,
            });
            if let Some(descriptor) = active.resident_descriptors.get_mut(writeback.data_index) {
                descriptor.device_may_have_modified = true;
                descriptor.host_content_sha256 = None;
            }
        }
        self.recycled_dispatch = Some(RecycledDispatchV1 {
            kernel: active.kernel,
            dispatch_shape_sha256: active.dispatch_shape_sha256,
            descriptors: core::mem::take(&mut active.resident_descriptors),
        });
        let module = self
            .kernels
            .get(&active.kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(active.id, module, active.allocations.iter().copied());
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.release_compute_lane_lease_v1(active.stream, compute_lane);
        self.last_launch_performance = Some(active.performance);
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id);
        self.observe_profile_v1(profile_dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(active.performance),
            }
        }));
        active.batch = None;
        Ok(status)
    }

    /// Returns phase timings for the latest successfully completed launch.
    pub const fn last_launch_performance_v1(&self) -> Option<KfdRuntimeLaunchPerformanceV1> {
        self.last_launch_performance
    }

    /// Observes the queue-owned SDMA memory pool without changing custody.
    pub fn sdma_memory_pool_observation_v1(
        &self,
    ) -> Result<Gfx942SdmaMemoryPoolObservationV1, KfdRuntimeBackendErrorV1> {
        if self.terminal {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "KFD backend is terminal",
            ));
        }
        if !self.native_available || !self.sdma_enabled {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native KFD SDMA memory pool is unavailable",
            ));
        }
        self.queue
            .as_ref()
            .ok_or_else(|| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "enabled KFD SDMA pool lost its queue",
                )
            })?
            .sdma_memory_pool_observation()
            .map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    format!("KFD SDMA memory-pool observation: {error}"),
                )
            })
    }

    /// Explicitly tears down the retained native queue after logical cleanup.
    ///
    /// Every logical stream must already be destroyed and no submission may
    /// be active. A teardown failure is terminal because the consuming KFD
    /// transition cannot return queue custody for a retry.
    pub fn shutdown_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.streams.is_empty()
            || !self.events.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || !self.submissions.is_empty()
            || !self.modules.is_empty()
            || !self.allocations.is_empty()
            || !self.pending_compute.is_empty()
            || !self.pending_compute_streams.is_empty()
            || !self.allocation_custody.is_empty()
            || !self.compute_module_retain_counts.is_empty()
            || !self.compute_dependency_retain_counts.is_empty()
            || !self.stream_submission_tails.is_empty()
            || self.any_compute_active_v1()
            || !self.active_sdma.is_empty()
            || !self.active_sdma_streams.is_empty()
            || !self.sdma_dependency_retain_counts.is_empty()
            || !self.quiescent_sdma_submissions.is_empty()
            || self.compute_completion_reservations != 0
            || self.sdma_completion_reservations != 0
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "logical runtime resources remain live",
            ));
        }
        #[cfg(test)]
        if let Some(driver) = self.scripted_sdma.as_ref() {
            if !driver.is_exhausted()
                || driver.live_owner_count() != 0
                || driver.unexpected_drops() != 0
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "scripted directional SDMA custody or operations remain live",
                ));
            }
            self.native_available = false;
            self.sdma_enabled = false;
            self.queue_retired = true;
            return Ok(());
        }
        self.detach_recycled_dispatch()?;
        self.release_resident_data()?;
        for lane in 1..self.native_compute_lanes.len() {
            self.with_compute_lane_state_v1(lane, |backend| {
                backend.detach_recycled_dispatch()?;
                backend.release_resident_data()
            })?;
        }
        if self.sdma_enabled {
            let trimmed = self
                .queue
                .as_mut()
                .expect("enabled SDMA pool retains queue")
                .trim_sdma_memory_pool();
            trimmed.map_err(|error| {
                self.terminal_error(format!("KFD SDMA memory-pool trim: {error}"))
            })?;
        }
        for index in 0..self.native_compute_lanes.len() {
            let Some(native_lane) = self.native_compute_lanes[index] else {
                continue;
            };
            if native_lane.ordinal() == 0 {
                continue;
            }
            let result = self
                .queue
                .as_mut()
                .expect("auxiliary queue retains its shared owner")
                .destroy_auxiliary_compute_lane_v1(native_lane);
            if let Err(error) = result {
                return Err(
                    self.terminal_error(format!("explicit auxiliary KFD queue teardown: {error}"))
                );
            }
            let profile_queue = self.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + index as u64,
            );
            self.observe_profile_v1(
                profile_queue
                    .map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue }),
            );
        }
        let primary_logical_lane = self
            .native_compute_lanes
            .iter()
            .position(|lane| lane.is_some_and(|lane| lane.ordinal() == 0));
        let profile_queue = primary_logical_lane.and_then(|lane| {
            self.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + lane as u64,
            )
        });
        if let Some(queue) = self.queue.take() {
            queue.destroy().map_err(|error| {
                self.terminal_error(format!("explicit KFD queue teardown: {error}"))
            })?;
            self.observe_profile_v1(
                profile_queue
                    .map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue }),
            );
        }
        self.admitted_device.take();
        self.native_compute_lanes.fill(None);
        self.queue_retired = true;
        Ok(())
    }

    fn detach_recycled_dispatch(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.recycled_dispatch.is_some() {
            let lane = self.selected_compute_lane;
            let mut dirty = Vec::new();
            dirty
                .try_reserve_exact(self.allocations.len())
                .map_err(|_| Self::capacity("KFD native-dirty synchronization roster failed"))?;
            dirty.extend(self.allocations.iter().filter_map(|(allocation, record)| {
                record
                    .native_dirty
                    .iter()
                    .any(|extent| extent.compute_lane == lane)
                    .then_some(*allocation)
            }));
            for allocation in dirty {
                self.synchronize_native_allocation_lane_v1(allocation, lane)?;
            }
        }
        let Some(recycled) = self.recycled_dispatch.take() else {
            return Ok(());
        };
        let native_lane = self.selected_native_compute_lane_v1()?;
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD recycled dispatch exists without a native queue".to_owned())
            .and_then(|queue| {
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        queue.detach_recycled_fixed_dispatch()
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
                    .map_err(|error| format!("KFD recycled dispatch detach: {error}"))
            });
        match result {
            Ok(detached) => {
                self.resident_data = Some(ResidentDataRosterV1 {
                    descriptors: recycled.descriptors,
                    data: detached.into_data(),
                });
                Ok(())
            }
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }

    fn release_resident_data(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(resident) = self.resident_data.take() else {
            return Ok(());
        };
        let native_lane = self.selected_native_compute_lane_v1()?;
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD resident data exists without a native queue".to_owned())
            .and_then(|queue| {
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        release_resident_data_v1(queue, resident)
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
            });
        match result {
            Ok(()) => Ok(()),
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }

    fn synchronize_native_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let dirty_lanes = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD allocation",
                )
            })?
            .native_dirty
            .iter()
            .fold(
                [false; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1],
                |mut lanes, extent| {
                    if let Some(lane) = lanes.get_mut(extent.compute_lane) {
                        *lane = true;
                    }
                    lanes
                },
            );
        for (lane, dirty) in dirty_lanes.into_iter().enumerate() {
            if dirty {
                self.with_compute_lane_state_v1(lane, |backend| {
                    backend.synchronize_native_allocation_lane_v1(allocation, lane)
                })?;
            }
        }
        Ok(())
    }

    fn synchronize_native_allocation_lane_v1(
        &mut self,
        allocation: u64,
        compute_lane: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let dirty: Vec<_> = self
            .allocations
            .get(&allocation)
            .expect("validated allocation remains indexed")
            .native_dirty
            .iter()
            .filter(|extent| extent.compute_lane == compute_lane)
            .copied()
            .collect();
        if dirty.is_empty() {
            return Ok(());
        }
        if self.recycled_dispatch.is_none() {
            return Err(self.terminal_error("native-dirty allocation has no recycled dispatch"));
        }
        let descriptors = &self
            .recycled_dispatch
            .as_ref()
            .expect("checked native-dirty dispatch custody")
            .descriptors;
        if dirty.iter().any(|extent| {
            descriptors
                .get(extent.data_index)
                .is_none_or(|descriptor| descriptor.allocation != allocation)
        }) {
            return Err(
                self.terminal_error("KFD native-dirty allocation descriptor mismatch".to_owned())
            );
        }
        let native_result = {
            let native_lane = self.selected_native_compute_lane_v1()?;
            let queue = self
                .queue
                .as_mut()
                .expect("native-dirty allocation retains its queue");
            queue
                .with_compute_lane_v1(native_lane, |queue| {
                    queue
                        .recycled_fixed_dispatch_generation()
                        .and_then(|generation| {
                            dirty
                                .iter()
                                .map(|extent| {
                                    queue
                                        .read_recycled_fixed_dispatch_data(
                                            Gfx942CompletedDispatchReadRequestV1::new(
                                                generation,
                                                extent.data_index,
                                                extent.data_offset,
                                                extent.byte_len,
                                            ),
                                        )
                                        .map(|readback| {
                                            (extent.allocation_offset, readback.into_bytes())
                                        })
                                })
                                .collect::<Result<Vec<_>, _>>()
                        })
                })
                .map_err(|error| format!("KFD compute-lane selection: {error}"))
                .and_then(|result| {
                    result.map_err(|error| {
                        format!("KFD recycled generation before readback: {error}")
                    })
                })
        };
        let updates = match native_result {
            Ok(updates) => updates,
            Err(detail) => return Err(self.terminal_error(detail)),
        };
        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("native-dirty allocation remains retained");
        for (offset, bytes) in updates {
            let end = offset
                .checked_add(bytes.len())
                .expect("validated native readback range fits host address space");
            if offset == 0 && end == record.bytes.len() {
                record.bytes = Arc::from(bytes);
            } else {
                Arc::make_mut(&mut record.bytes)[offset..end].copy_from_slice(&bytes);
            }
        }
        record.content_sha256 = None;
        let bytes = Arc::clone(&record.bytes);
        let _ = record;
        self.upload_sdma_range_v1(allocation, 0, &bytes)
            .map_err(Self::after_possible_host_mutation)?;
        if let Some(record) = self.allocations.get_mut(&allocation) {
            record
                .native_dirty
                .retain(|extent| extent.compute_lane != compute_lane);
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = false;
        }
        self.native_dirty_extents = self
            .native_dirty_extents
            .checked_sub(dirty.len())
            .expect("native-dirty index covers every retained extent");
        Ok(())
    }

    fn read_native_allocation_into_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if destination.is_empty() {
            return Ok(false);
        }
        let requested_start = usize::try_from(byte_offset).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation offset does not fit host address space",
            )
        })?;
        let requested_end = requested_start
            .checked_add(destination.len())
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "allocation read range overflow",
                )
            })?;
        let extent = self
            .allocations
            .get(&allocation)
            .and_then(|record| {
                record.native_dirty.iter().find(|extent| {
                    let extent_len = usize::try_from(extent.byte_len).ok();
                    let extent_end =
                        extent_len.and_then(|len| extent.allocation_offset.checked_add(len));
                    requested_start >= extent.allocation_offset
                        && extent_end.is_some_and(|end| requested_end <= end)
                })
            })
            .copied();
        let Some(extent) = extent else {
            return Ok(false);
        };
        let delta = requested_start - extent.allocation_offset;
        let data_offset = extent
            .data_offset
            .checked_add(delta as u64)
            .expect("contained native-dirty read offset does not overflow");
        let compute_lane = extent.compute_lane;
        self.with_compute_lane_state_v1(compute_lane, |backend| {
            backend.read_native_allocation_extent_into_v1(extent, data_offset, destination)
        })
    }

    fn read_native_allocation_extent_into_v1(
        &mut self,
        extent: NativeDirtyExtentV1,
        data_offset: u64,
        destination: &mut [u8],
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let native_result = {
            let native_lane = self.selected_native_compute_lane_v1()?;
            let queue = self
                .queue
                .as_mut()
                .expect("native-dirty allocation retains its queue");
            queue
                .with_compute_lane_v1(native_lane, |queue| {
                    queue
                        .recycled_fixed_dispatch_generation()
                        .and_then(|generation| {
                            queue.read_recycled_fixed_dispatch_data_into(
                                Gfx942CompletedDispatchReadRequestV1::new(
                                    generation,
                                    extent.data_index,
                                    data_offset,
                                    destination.len() as u64,
                                ),
                                destination,
                            )
                        })
                })
                .map_err(|error| format!("KFD compute-lane selection: {error}"))
                .and_then(|result| {
                    result.map_err(|error| format!("KFD direct coherent readback: {error}"))
                })
        };
        match native_result {
            Ok(()) => Ok(true),
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }

    fn validate_semantic_launch_v1(
        &self,
        semantic_launch: KfdRuntimeSemanticLaunchV1,
        geometry: crate::RuntimeLaunchGeometryV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let supported = match semantic_launch {
            KfdRuntimeSemanticLaunchV1::Ordinary => return Ok(()),
            KfdRuntimeSemanticLaunchV1::Atomic(contract) => {
                contract.geometry == geometry
                    && contract.scope != RuntimeMemoryScopeV1::System
                    && atomic_contract_is_legal_v1(contract)
                    && self.launch_gate.supports_atomic_v1(contract)
            }
            KfdRuntimeSemanticLaunchV1::Collective(contract) => {
                contract.geometry == geometry
                    && contract.scope == RuntimeMemoryScopeV1::Workgroup
                    && complete_workgroup_geometry_v1(geometry)
                    && workgroup_participants_v1(geometry) == Some(contract.participants)
                    && self.launch_gate.supports_collective_v1(contract)
            }
        };
        if supported {
            Ok(())
        } else {
            Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "semantic launch contract is not covered by direct KFD authority",
            ))
        }
    }

    #[cfg(test)]
    fn mock() -> Self {
        Self::mock_with_staging_budgets(StagingBudgetsV1 {
            max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
            max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
        })
    }

    #[cfg(test)]
    fn mock_with_staging_budgets(staging_budgets: StagingBudgetsV1) -> Self {
        Self::new_with_staging_budgets(
            BackendDeviceDescriptionV1 {
                backend_device: 7,
                name: "mock gfx942".to_owned(),
                target: "gfx942:xnack-".to_owned(),
                global_memory_bytes: 0,
                capabilities: kfd_capabilities_v1(),
            },
            None,
            KfdRuntimeLaunchGateV1::Production(Box::new(TestAuthorityV1)),
            staging_budgets,
        )
    }

    #[cfg(test)]
    fn mock_with_semantic_authority_v1() -> Self {
        Self::new_with_staging_budgets(
            BackendDeviceDescriptionV1 {
                backend_device: 7,
                name: "mock semantic gfx942".to_owned(),
                target: "gfx942:xnack-".to_owned(),
                global_memory_bytes: 0,
                capabilities: kfd_capabilities_v1(),
            },
            None,
            KfdRuntimeLaunchGateV1::Semantic(Box::new(TestSemanticAuthorityV1)),
            StagingBudgetsV1 {
                max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
                max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
            },
        )
    }

    #[cfg(test)]
    fn mock_with_panicking_authority_v1() -> Self {
        Self::new_with_staging_budgets(
            BackendDeviceDescriptionV1 {
                backend_device: 7,
                name: "mock panicking-authority gfx942".to_owned(),
                target: "gfx942:xnack-".to_owned(),
                global_memory_bytes: 0,
                capabilities: kfd_capabilities_v1(),
            },
            None,
            KfdRuntimeLaunchGateV1::Production(Box::new(TestPanickingAuthorityV1)),
            StagingBudgetsV1 {
                max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
                max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
            },
        )
    }
}

#[cfg(test)]
#[derive(Debug)]
struct TestAuthorityV1;

#[cfg(test)]
unsafe impl KfdRuntimeLaunchAuthorityV1 for TestAuthorityV1 {
    fn authorize_launch_v1(&self, _request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        true
    }
}

#[cfg(test)]
#[derive(Debug)]
struct TestPanickingAuthorityV1;

#[cfg(test)]
struct TestPanickingAuthorityPayloadV1;

#[cfg(test)]
impl Drop for TestPanickingAuthorityPayloadV1 {
    fn drop(&mut self) {
        panic!("requested KFD authority payload-drop panic");
    }
}

#[cfg(test)]
unsafe impl KfdRuntimeLaunchAuthorityV1 for TestPanickingAuthorityV1 {
    fn authorize_launch_v1(&self, _request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        std::panic::panic_any(TestPanickingAuthorityPayloadV1);
    }
}

#[cfg(test)]
const TEST_ATOMIC_PROFILE_V1: KfdRuntimeAtomicExecutionProfileV1 =
    KfdRuntimeAtomicExecutionProfileV1 {
        operation: RuntimeAtomicOperationV1::Add,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::Relaxed,
        failure_order: None,
        weak: false,
    };

#[cfg(test)]
const TEST_COLLECTIVE_PROFILE_V1: KfdRuntimeCollectiveExecutionProfileV1 =
    KfdRuntimeCollectiveExecutionProfileV1 {
        operation: crate::RuntimeCollectiveOperationV1::ReduceSum,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::AcquireRelease,
    };

#[cfg(test)]
#[derive(Debug)]
struct TestSemanticAuthorityV1;

#[cfg(test)]
unsafe impl KfdRuntimeLaunchAuthorityV1 for TestSemanticAuthorityV1 {
    fn authorize_launch_v1(&self, request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        match request.semantic_launch {
            KfdRuntimeSemanticLaunchV1::Ordinary => true,
            KfdRuntimeSemanticLaunchV1::Atomic(contract) => {
                TEST_ATOMIC_PROFILE_V1.matches_v1(contract)
            }
            KfdRuntimeSemanticLaunchV1::Collective(contract) => {
                TEST_COLLECTIVE_PROFILE_V1.matches_v1(contract)
            }
        }
    }
}

#[cfg(test)]
unsafe impl KfdRuntimeSemanticLaunchAuthorityV1 for TestSemanticAuthorityV1 {
    fn atomic_profiles_v1(&self) -> &[KfdRuntimeAtomicExecutionProfileV1] {
        core::slice::from_ref(&TEST_ATOMIC_PROFILE_V1)
    }

    fn collective_profiles_v1(&self) -> &[KfdRuntimeCollectiveExecutionProfileV1] {
        core::slice::from_ref(&TEST_COLLECTIVE_PROFILE_V1)
    }
}

#[cfg(test)]
#[derive(Debug)]
struct TestPanickingSemanticProfileAuthorityV1;

#[cfg(test)]
unsafe impl KfdRuntimeLaunchAuthorityV1 for TestPanickingSemanticProfileAuthorityV1 {
    fn authorize_launch_v1(&self, _request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        true
    }
}

#[cfg(test)]
unsafe impl KfdRuntimeSemanticLaunchAuthorityV1 for TestPanickingSemanticProfileAuthorityV1 {
    fn atomic_profiles_v1(&self) -> &[KfdRuntimeAtomicExecutionProfileV1] {
        std::panic::panic_any(TestPanickingAuthorityPayloadV1);
    }

    fn collective_profiles_v1(&self) -> &[KfdRuntimeCollectiveExecutionProfileV1] {
        std::panic::panic_any(TestPanickingAuthorityPayloadV1);
    }
}

#[cfg(test)]
static TEST_OVERBOUND_ATOMIC_PROFILES_V1: [KfdRuntimeAtomicExecutionProfileV1;
    KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1 + 1] =
    [TEST_ATOMIC_PROFILE_V1; KFD_RUNTIME_MAX_SEMANTIC_PROFILES_V1 + 1];

#[cfg(test)]
#[derive(Debug)]
struct TestOverboundSemanticAuthorityV1;

#[cfg(test)]
unsafe impl KfdRuntimeLaunchAuthorityV1 for TestOverboundSemanticAuthorityV1 {
    fn authorize_launch_v1(&self, _request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        true
    }
}

#[cfg(test)]
unsafe impl KfdRuntimeSemanticLaunchAuthorityV1 for TestOverboundSemanticAuthorityV1 {
    fn atomic_profiles_v1(&self) -> &[KfdRuntimeAtomicExecutionProfileV1] {
        &TEST_OVERBOUND_ATOMIC_PROFILES_V1
    }

    fn collective_profiles_v1(&self) -> &[KfdRuntimeCollectiveExecutionProfileV1] {
        &[]
    }
}

fn kfd_capabilities_v1() -> RuntimeCapabilitiesV1 {
    RuntimeCapabilitiesV1 {
        typed_async_launch: true,
        streams: true,
        events: true,
        device_memory: true,
        host_visible_memory: true,
        peer_copy: false,
        multi_device: false,
        atomics: false,
        collectives: false,
    }
}

fn map_access_v1(access: RuntimeAccessV1) -> ArgumentAccess {
    match access {
        RuntimeAccessV1::Read => ArgumentAccess::ReadOnly,
        RuntimeAccessV1::Write => ArgumentAccess::WriteOnly,
        RuntimeAccessV1::ReadWrite => ArgumentAccess::ReadWrite,
    }
}

fn profile_semantic_contract_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    geometry: KfdProfileLaunchV1,
) -> Option<KfdProfileSemanticContractV1> {
    match semantic_launch {
        KfdRuntimeSemanticLaunchV1::Ordinary => None,
        KfdRuntimeSemanticLaunchV1::Atomic(contract) => Some(KfdProfileSemanticContractV1::Atomic(
            KfdProfileAtomicContractV1 {
                operation: match contract.operation {
                    RuntimeAtomicOperationV1::Add => KfdProfileAtomicOperationV1::Add,
                    RuntimeAtomicOperationV1::Minimum => KfdProfileAtomicOperationV1::Minimum,
                    RuntimeAtomicOperationV1::Maximum => KfdProfileAtomicOperationV1::Maximum,
                    RuntimeAtomicOperationV1::BitwiseAnd => KfdProfileAtomicOperationV1::BitwiseAnd,
                    RuntimeAtomicOperationV1::BitwiseOr => KfdProfileAtomicOperationV1::BitwiseOr,
                    RuntimeAtomicOperationV1::BitwiseXor => KfdProfileAtomicOperationV1::BitwiseXor,
                    RuntimeAtomicOperationV1::Exchange => KfdProfileAtomicOperationV1::Exchange,
                    RuntimeAtomicOperationV1::CompareExchange => {
                        KfdProfileAtomicOperationV1::CompareExchange
                    }
                },
                scope: profile_memory_scope_v1(contract.scope),
                order: profile_memory_order_v1(contract.order),
                failure_order: contract.failure_order.map(profile_memory_order_v1),
                weak: contract.weak,
                geometry,
            },
        )),
        KfdRuntimeSemanticLaunchV1::Collective(contract) => Some(
            KfdProfileSemanticContractV1::Collective(KfdProfileCollectiveContractV1 {
                operation: match contract.operation {
                    crate::RuntimeCollectiveOperationV1::Barrier => {
                        KfdProfileCollectiveOperationV1::Barrier
                    }
                    crate::RuntimeCollectiveOperationV1::Broadcast => {
                        KfdProfileCollectiveOperationV1::Broadcast
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceSum => {
                        KfdProfileCollectiveOperationV1::ReduceSum
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceMinimum => {
                        KfdProfileCollectiveOperationV1::ReduceMinimum
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceMaximum => {
                        KfdProfileCollectiveOperationV1::ReduceMaximum
                    }
                    crate::RuntimeCollectiveOperationV1::AllReduceSum => {
                        KfdProfileCollectiveOperationV1::AllReduceSum
                    }
                    crate::RuntimeCollectiveOperationV1::InclusiveScanSum => {
                        KfdProfileCollectiveOperationV1::InclusiveScanSum
                    }
                },
                scope: profile_memory_scope_v1(contract.scope),
                order: profile_memory_order_v1(contract.order),
                participants: contract.participants,
                geometry,
            }),
        ),
    }
}

const fn profile_memory_scope_v1(scope: RuntimeMemoryScopeV1) -> KfdProfileMemoryScopeV1 {
    match scope {
        RuntimeMemoryScopeV1::Workgroup => KfdProfileMemoryScopeV1::Workgroup,
        RuntimeMemoryScopeV1::Device => KfdProfileMemoryScopeV1::Device,
        RuntimeMemoryScopeV1::System => KfdProfileMemoryScopeV1::System,
    }
}

const fn profile_memory_order_v1(order: RuntimeMemoryOrderV1) -> KfdProfileMemoryOrderV1 {
    match order {
        RuntimeMemoryOrderV1::Relaxed => KfdProfileMemoryOrderV1::Relaxed,
        RuntimeMemoryOrderV1::Acquire => KfdProfileMemoryOrderV1::Acquire,
        RuntimeMemoryOrderV1::Release => KfdProfileMemoryOrderV1::Release,
        RuntimeMemoryOrderV1::AcquireRelease => KfdProfileMemoryOrderV1::AcquireRelease,
        RuntimeMemoryOrderV1::SequentiallyConsistent => {
            KfdProfileMemoryOrderV1::SequentiallyConsistent
        }
    }
}

fn dispatch_shape_sha256_v1(
    launch: &BackendLaunchV1<'_>,
    semantic_launch: KfdRuntimeSemanticLaunchV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.runtime.kfd.recycled-dispatch-shape.v1\0");
    digest.update(launch.kernel.to_le_bytes());
    for value in launch.geometry.grid {
        digest.update(value.to_le_bytes());
    }
    for value in launch.geometry.workgroup {
        digest.update(value.to_le_bytes());
    }
    digest.update(launch.geometry.dynamic_shared_bytes.to_le_bytes());
    digest.update((launch.explicit_kernarg.len() as u64).to_le_bytes());
    digest.update(launch.explicit_kernarg);
    digest.update((launch.bindings.len() as u64).to_le_bytes());
    for binding in launch.bindings {
        digest.update(binding.region.allocation.to_le_bytes());
        digest.update([match binding.region.access {
            RuntimeAccessV1::Read => 1,
            RuntimeAccessV1::Write => 2,
            RuntimeAccessV1::ReadWrite => 3,
        }]);
        digest.update(binding.region.byte_offset.to_le_bytes());
        digest.update(binding.region.byte_len.to_le_bytes());
        digest.update(binding.kernarg_byte_offset.to_le_bytes());
    }
    match semantic_launch {
        KfdRuntimeSemanticLaunchV1::Ordinary => digest.update([0]),
        KfdRuntimeSemanticLaunchV1::Atomic(contract) => {
            digest.update([1, atomic_operation_tag_v1(contract.operation)]);
            digest.update([memory_scope_tag_v1(contract.scope)]);
            digest.update([memory_order_tag_v1(contract.order)]);
            digest.update([contract
                .failure_order
                .map_or(0, |order| memory_order_tag_v1(order).saturating_add(1))]);
            digest.update([u8::from(contract.weak)]);
        }
        KfdRuntimeSemanticLaunchV1::Collective(contract) => {
            digest.update([2, collective_operation_tag_v1(contract.operation)]);
            digest.update([memory_scope_tag_v1(contract.scope)]);
            digest.update([memory_order_tag_v1(contract.order)]);
            digest.update(contract.participants.to_le_bytes());
        }
    }
    digest.finalize().into()
}

const fn atomic_operation_tag_v1(operation: RuntimeAtomicOperationV1) -> u8 {
    match operation {
        RuntimeAtomicOperationV1::Add => 0,
        RuntimeAtomicOperationV1::Minimum => 1,
        RuntimeAtomicOperationV1::Maximum => 2,
        RuntimeAtomicOperationV1::BitwiseAnd => 3,
        RuntimeAtomicOperationV1::BitwiseOr => 4,
        RuntimeAtomicOperationV1::BitwiseXor => 5,
        RuntimeAtomicOperationV1::Exchange => 6,
        RuntimeAtomicOperationV1::CompareExchange => 7,
    }
}

const fn collective_operation_tag_v1(operation: crate::RuntimeCollectiveOperationV1) -> u8 {
    match operation {
        crate::RuntimeCollectiveOperationV1::Barrier => 0,
        crate::RuntimeCollectiveOperationV1::Broadcast => 1,
        crate::RuntimeCollectiveOperationV1::ReduceSum => 2,
        crate::RuntimeCollectiveOperationV1::ReduceMinimum => 3,
        crate::RuntimeCollectiveOperationV1::ReduceMaximum => 4,
        crate::RuntimeCollectiveOperationV1::AllReduceSum => 5,
        crate::RuntimeCollectiveOperationV1::InclusiveScanSum => 6,
    }
}

const fn memory_scope_tag_v1(scope: RuntimeMemoryScopeV1) -> u8 {
    match scope {
        RuntimeMemoryScopeV1::Workgroup => 0,
        RuntimeMemoryScopeV1::Device => 1,
        RuntimeMemoryScopeV1::System => 2,
    }
}

const fn memory_order_tag_v1(order: RuntimeMemoryOrderV1) -> u8 {
    match order {
        RuntimeMemoryOrderV1::Relaxed => 0,
        RuntimeMemoryOrderV1::Acquire => 1,
        RuntimeMemoryOrderV1::Release => 2,
        RuntimeMemoryOrderV1::AcquireRelease => 3,
        RuntimeMemoryOrderV1::SequentiallyConsistent => 4,
    }
}

const fn atomic_contract_is_legal_v1(contract: RuntimeAtomicLaunchContractV1) -> bool {
    match (contract.operation, contract.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(contract.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !contract.weak,
        (_, Some(_)) => false,
    }
}

const fn compare_exchange_orders_are_legal_v1(
    success: RuntimeMemoryOrderV1,
    failure: RuntimeMemoryOrderV1,
) -> bool {
    match success {
        RuntimeMemoryOrderV1::Relaxed => matches!(failure, RuntimeMemoryOrderV1::Relaxed),
        RuntimeMemoryOrderV1::Acquire => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed | RuntimeMemoryOrderV1::Acquire
        ),
        RuntimeMemoryOrderV1::Release => matches!(failure, RuntimeMemoryOrderV1::Relaxed),
        RuntimeMemoryOrderV1::AcquireRelease => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed | RuntimeMemoryOrderV1::Acquire
        ),
        RuntimeMemoryOrderV1::SequentiallyConsistent => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed
                | RuntimeMemoryOrderV1::Acquire
                | RuntimeMemoryOrderV1::SequentiallyConsistent
        ),
    }
}

fn complete_workgroup_geometry_v1(geometry: crate::RuntimeLaunchGeometryV1) -> bool {
    geometry
        .grid
        .into_iter()
        .zip(geometry.workgroup)
        .all(|(grid, workgroup)| {
            workgroup != 0 && grid >= workgroup && grid.is_multiple_of(workgroup)
        })
}

fn workgroup_participants_v1(geometry: crate::RuntimeLaunchGeometryV1) -> Option<u64> {
    geometry
        .workgroup
        .into_iter()
        .try_fold(1_u64, |product, value| {
            product.checked_mul(u64::from(value))
        })
}

const fn atomic_profile_is_admissible_v1(profile: KfdRuntimeAtomicExecutionProfileV1) -> bool {
    if matches!(profile.scope, RuntimeMemoryScopeV1::System) {
        return false;
    }
    match (profile.operation, profile.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(profile.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !profile.weak,
        (_, Some(_)) => false,
    }
}

const fn collective_profile_is_admissible_v1(
    profile: KfdRuntimeCollectiveExecutionProfileV1,
) -> bool {
    matches!(profile.scope, RuntimeMemoryScopeV1::Workgroup)
}

fn try_copy_vec_v1(
    source: &[u8],
    detail: &'static str,
) -> Result<Vec<u8>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(source.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity(detail))?;
    bytes.extend_from_slice(source);
    Ok(bytes)
}

fn try_zeroed_staging_v1(
    len: usize,
) -> Result<Vec<u8>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged allocation failed"))?;
    bytes.resize(len, 0);
    Ok(bytes)
}

fn classify_sdma_chunk_failure_v1<E>(
    completed_chunks: usize,
    failure: RuntimeBackendFailureV1<E>,
) -> RuntimeBackendFailureV1<E> {
    match failure {
        RuntimeBackendFailureV1::Rejected(error) if completed_chunks != 0 => {
            RuntimeBackendFailureV1::Quiescent(error)
        }
        failure => failure,
    }
}

fn snapshot_bound_data_v1(
    allocations: &HashMap<u64, AllocationRecordV1>,
    bindings: &[BackendBindingV1],
    stream_device: u64,
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut ranges = HashMap::<u64, (u64, u64)>::new();
    let mut order = Vec::<u64>::new();
    ranges
        .try_reserve(bindings.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-range map allocation failed"))?;
    order
        .try_reserve_exact(bindings.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-range order allocation failed"))?;

    for binding in bindings {
        let region = binding.region;
        let allocation = allocations.get(&region.allocation).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        if allocation.device != stream_device {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "allocation and stream belong to different devices",
            ));
        }
        if allocation.kind == RuntimeMemoryKindV1::DeviceLocal
            && region.access != RuntimeAccessV1::Read
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "device-local writeback is unavailable without an admitted copy path",
            ));
        }
        let range_end = region
            .byte_offset
            .checked_add(region.byte_len)
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "binding range overflow",
                )
            })?;
        if region.byte_len == 0 || range_end > allocation.bytes.len() as u64 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "binding lies outside its allocation",
            ));
        }
        let aligned_start = region.byte_offset & !(allocation.alignment - 1);
        if let Some((start, end)) = ranges.get_mut(&region.allocation) {
            *start = (*start).min(aligned_start);
            *end = (*end).max(range_end);
        } else {
            if order.len() == GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "fixed KFD dispatch data roster is full",
                ));
            }
            ranges.insert(region.allocation, (aligned_start, range_end));
            order.push(region.allocation);
        }
    }

    let mut data = Vec::new();
    let mut placements = HashMap::new();
    data.try_reserve_exact(order.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-data roster allocation failed"))?;
    placements
        .try_reserve(order.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-placement map allocation failed"))?;
    for allocation_id in order {
        let allocation = &allocations[&allocation_id];
        let (start, end) = ranges[&allocation_id];
        let start_index = usize::try_from(start).map_err(|_| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "staged allocation offset does not fit host address space",
            )
        })?;
        let end_index = usize::try_from(end).map_err(|_| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "staged allocation end does not fit host address space",
            )
        })?;
        let data_index = data.len();
        data.push(DataSpecV1 {
            allocation: allocation_id,
            kind: allocation.kind,
            alignment: allocation.alignment,
            allocation_offset: start,
            bytes: Arc::clone(&allocation.bytes),
            byte_range: start_index..end_index,
            content_sha256: (start_index == 0 && end_index == allocation.bytes.len())
                .then_some(allocation.content_sha256)
                .flatten(),
        });
        placements.insert(
            allocation_id,
            StagedPlacementV1 {
                data_index,
                allocation_offset: start,
            },
        );
    }
    Ok(StagedDataRosterV1 { data, placements })
}

fn build_program_v1<'a>(
    program: &'a OwnedValidatedKernelEnvelope,
    signature: [u8; 32],
    owned_rows: &[OwnedAbiRowV1],
) -> Result<ValidatedKernelEnvelope<'a>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let arguments = program.selected_kernel().explicit_arguments();
    let mut rows = Vec::new();
    rows.try_reserve_exact(owned_rows.len()).map_err(|_| {
        KfdRuntimeBackendV1::capacity("KFD reconciled ABI roster allocation failed")
    })?;
    for row in owned_rows {
        let name = arguments[row.explicit_argument_index]
            .name()
            .expect("prepared global-buffer ABI row retains a source name");
        rows.push(KernelGlobalBufferAbiV1::new(
            row.explicit_argument_index,
            name,
            row.offset,
            row.pointee_alignment,
            row.access,
        ));
    }
    program
        .validated()
        .reconcile_dispatch_abi(signature, &rows)
        .map_err(|error| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                format!("typed AMDHSA dispatch ABI: {error:?}"),
            )
        })
}

fn materialize_initial_data_v1(
    memory: &mut SharedGttMemorySessionV1,
    specs: Vec<DataSpecV1>,
    role_identity: [u8; 32],
) -> Result<Vec<Gfx942FixedDispatchDataV1>, String> {
    let mut data = Vec::new();
    data.try_reserve_exact(specs.len())
        .map_err(|_| "KFD native-data roster allocation failed".to_owned())?;
    for (index, spec) in specs.into_iter().enumerate() {
        let owned_bytes = spec.try_owned_bytes()?;
        let item = match spec.kind {
            RuntimeMemoryKindV1::HostVisible => memory
                .initialize_host_visible_coherent(owned_bytes)
                .map(Gfx942FixedDispatchDataV1::host_visible_initialized)
                .map_err(|error| format!("KFD host-visible initialization: {error}"))?,
            RuntimeMemoryKindV1::DeviceLocal => {
                let ordinal = u32::try_from(index)
                    .map_err(|_| "KFD device-content ordinal does not fit u32".to_owned())?;
                let role = Gfx942DeviceContentRoleV1::new(role_identity, ordinal)
                    .map_err(|error| format!("KFD device-content role: {error}"))?;
                let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &owned_bytes)
                    .map_err(|error| format!("KFD device-content descriptor: {error}"))?;
                memory
                    .initialize_gfx942_device_memory(owned_bytes, spec.alignment, content)
                    .map(Gfx942FixedDispatchDataV1::initialized)
                    .map_err(|error| format!("KFD device-local initialization: {error}"))?
            }
        };
        data.push(item);
    }
    Ok(data)
}

fn resident_descriptors_v1(
    specs: &[DataSpecV1],
) -> Result<Vec<ResidentDataDescriptorV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut descriptors = Vec::new();
    descriptors
        .try_reserve_exact(specs.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD resident-data roster allocation failed"))?;
    for spec in specs {
        descriptors.push(ResidentDataDescriptorV1 {
            allocation: spec.allocation,
            kind: spec.kind,
            alignment: spec.alignment,
            allocation_offset: spec.allocation_offset,
            byte_len: u64::try_from(spec.bytes().len()).map_err(|_| {
                KfdRuntimeBackendV1::capacity("KFD resident-data extent does not fit u64")
            })?,
            host_content_sha256: spec.content_sha256,
            device_may_have_modified: false,
        });
    }
    Ok(descriptors)
}

fn same_resident_storage_shape_v1(
    left: &[ResidentDataDescriptorV1],
    right: &[ResidentDataDescriptorV1],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.allocation == right.allocation
                && left.kind == right.kind
                && left.alignment == right.alignment
                && left.allocation_offset == right.allocation_offset
                && left.byte_len == right.byte_len
        })
}

fn release_resident_data_v1(
    queue: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    resident: ResidentDataRosterV1,
) -> Result<(), String> {
    for data in resident.data {
        queue
            .release_detached_fixed_dispatch_data(data)
            .map_err(|error| format!("KFD resident-data release: {error}"))?;
    }
    Ok(())
}

fn materialize_rebound_data_v1(
    queue: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    specs: Vec<DataSpecV1>,
    role_identity: [u8; 32],
) -> Result<Vec<Gfx942FixedDispatchDataV1>, String> {
    let mut data = Vec::new();
    data.try_reserve_exact(specs.len())
        .map_err(|_| "KFD rebound-data roster allocation failed".to_owned())?;
    for (index, spec) in specs.into_iter().enumerate() {
        queue
            .preflight_fixed_dispatch_data_insertion(index)
            .map_err(|error| format!("KFD dispatch-data insertion preflight: {error}"))?;
        let owned_bytes = spec.try_owned_bytes()?;
        let item = match spec.kind {
            RuntimeMemoryKindV1::HostVisible => queue
                .insert_initialized_host_visible_fixed_dispatch_data(index, owned_bytes)
                .map_err(|error| format!("KFD host-visible insertion: {error}"))?,
            RuntimeMemoryKindV1::DeviceLocal => {
                let ordinal = u32::try_from(index)
                    .map_err(|_| "KFD device-content ordinal does not fit u32".to_owned())?;
                let role = Gfx942DeviceContentRoleV1::new(role_identity, ordinal)
                    .map_err(|error| format!("KFD device-content role: {error}"))?;
                let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &owned_bytes)
                    .map_err(|error| format!("KFD device-content descriptor: {error}"))?;
                queue
                    .insert_initialized_fixed_dispatch_data(
                        index,
                        owned_bytes,
                        spec.alignment,
                        content,
                    )
                    .map_err(|error| format!("KFD device-local insertion: {error}"))?
            }
        };
        data.push(item);
    }
    Ok(data)
}

fn wait_with_deadline_v1<E>(
    deadline: Instant,
    mut poll: impl FnMut() -> Result<BackendPollV1, E>,
) -> Result<BackendPollV1, E> {
    wait_with_deadline_tracking_progress_v1(deadline, || poll().map(|status| (status, false)))
}

fn wait_with_deadline_tracking_progress_v1<E>(
    deadline: Instant,
    poll: impl FnMut() -> Result<(BackendPollV1, bool), E>,
) -> Result<BackendPollV1, E> {
    wait_with_deadline_tracking_progress_by_v1(deadline, poll, apply_wait_backoff_v1)
}

fn wait_with_deadline_tracking_progress_by_v1<E>(
    deadline: Instant,
    mut poll: impl FnMut() -> Result<(BackendPollV1, bool), E>,
    mut backoff: impl FnMut(u32, &mut Duration, Instant) -> bool,
) -> Result<BackendPollV1, E> {
    let mut attempts = 0_u32;
    let mut sleep = WAIT_INITIAL_SLEEP_V1;
    loop {
        let (status, made_progress) = poll()?;
        if status != BackendPollV1::Pending || Instant::now() >= deadline {
            return Ok(status);
        }
        if made_progress {
            attempts = 0;
            sleep = WAIT_INITIAL_SLEEP_V1;
            continue;
        }
        if !backoff(attempts, &mut sleep, deadline) {
            return Ok(BackendPollV1::Pending);
        }
        attempts = attempts.saturating_add(1);
    }
}

fn apply_wait_backoff_v1(attempts: u32, sleep: &mut Duration, deadline: Instant) -> bool {
    if Instant::now() >= deadline {
        return false;
    }
    if attempts < WAIT_SPINS_V1 {
        core::hint::spin_loop();
    } else if attempts < WAIT_SPINS_V1 + WAIT_YIELDS_V1 {
        std::thread::yield_now();
    } else {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        std::thread::sleep((*sleep).min(remaining));
        *sleep = sleep.saturating_mul(2).min(WAIT_MAX_SLEEP_V1);
    }
    true
}

fn apply_unbounded_wait_backoff_v1(attempts: u32, sleep: &mut Duration) {
    if attempts < WAIT_SPINS_V1 {
        core::hint::spin_loop();
    } else if attempts < WAIT_SPINS_V1 + WAIT_YIELDS_V1 {
        std::thread::yield_now();
    } else {
        std::thread::sleep(*sleep);
        *sleep = sleep.saturating_mul(2).min(WAIT_MAX_SLEEP_V1);
    }
}

fn profile_host_timing_v1(performance: KfdRuntimeLaunchPerformanceV1) -> KfdProfileHostTimingV1 {
    KfdProfileHostTimingV1 {
        preparation_ns: duration_nanoseconds_v1(performance.preparation),
        bound_snapshot_ns: duration_nanoseconds_v1(performance.bound_snapshot),
        authority_ns: duration_nanoseconds_v1(performance.authority),
        native_binding_ns: duration_nanoseconds_v1(performance.native_binding),
        publication_ns: duration_nanoseconds_v1(performance.publication),
        publish_to_completion_ns: duration_nanoseconds_v1(performance.publish_to_completion),
        completed_readback_ns: duration_nanoseconds_v1(performance.completed_readback),
        recycle_ns: duration_nanoseconds_v1(performance.recycle),
    }
}

fn duration_nanoseconds_v1(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

impl RuntimeBackendV1 for KfdRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
        if device != self.description.backend_device || !self.native_available {
            return RuntimeExecutionCapabilitiesV1::default();
        }
        RuntimeExecutionCapabilitiesV1 {
            concurrent_compute: true,
            native_async_copy: true,
            compute_copy_overlap: true,
            memory_pool: true,
            cancellation: true,
            atomics: self.launch_gate.advertises_atomics_v1(),
            collectives: self.launch_gate.advertises_collectives_v1(),
            ..RuntimeExecutionCapabilitiesV1::default()
        }
    }

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        Ok(vec![self.description.clone()])
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_device(device)?;
        if self.queue_retired {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "KFD VM/queue ownership was retired after its last stream",
            ));
        }
        if self.streams.len() >= KFD_RUNTIME_MAX_LOGICAL_STREAMS_V1 {
            return Err(Self::capacity("KFD logical stream capacity exceeded"));
        }
        self.streams
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD stream-table growth failed"))?;
        let id = self.next_id()?;
        self.streams.insert(id, device);
        let stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, id);
        self.observe_profile_v1(
            stream.map(|stream| KfdRuntimeProfileEventKindV1::StreamCreated { stream }),
        );
        Ok(id)
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD stream",
            ));
        }
        if self
            .pending_compute_streams
            .get(&stream)
            .is_some_and(|queue| !queue.is_empty())
            || self
                .active
                .as_ref()
                .is_some_and(|active| active.stream == stream)
            || self.auxiliary_compute_lanes.iter().any(|lane| {
                lane.active
                    .as_ref()
                    .is_some_and(|active| active.stream == stream)
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream still owns a pending KFD dispatch",
            ));
        }
        if self.active_sdma_streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream still owns a pending KFD SDMA copy",
            ));
        }
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        debug_assert!(!self.stream_compute_lanes.contains_key(&stream));
        self.stream_submission_tails.remove(&stream);
        self.streams.remove(&stream);
        self.observe_profile_v1(
            profile_stream.map(|stream| KfdRuntimeProfileEventKindV1::StreamDestroyed { stream }),
        );
        Ok(())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_device(device)?;
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation length and power-of-two alignment must be nonzero",
            ));
        }
        if kind == RuntimeMemoryKindV1::DeviceLocal && alignment > HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "device-local KFD allocation alignment exceeds 4096 bytes",
            ));
        }
        if kind == RuntimeMemoryKindV1::HostVisible && alignment > HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "host-visible KFD allocation alignment exceeds the admitted page alignment",
            ));
        }
        if byte_len > self.staging_budgets.max_allocation_bytes {
            return Err(Self::capacity(
                "allocation exceeds the direct-KFD per-allocation staging budget",
            ));
        }
        let next_staged_context_bytes = self
            .staged_context_bytes
            .checked_add(byte_len)
            .filter(|total| *total <= self.staging_budgets.max_context_bytes)
            .ok_or_else(|| {
                Self::capacity("allocation exceeds the direct-KFD context staging budget")
            })?;
        let len = usize::try_from(byte_len)
            .map_err(|_| Self::capacity("allocation does not fit host staging address space"))?;
        self.allocations
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD allocation-table growth failed"))?;
        let bytes = try_zeroed_staging_v1(len)?;
        let id = self.next_id()?;
        let sdma_storage = if self.native_available {
            self.ensure_sdma_queue_v1()?;
            let result = match kind {
                RuntimeMemoryKindV1::DeviceLocal => self
                    .directional_sdma_ops_v1()
                    .allocate_device_buffer(byte_len, alignment),
                RuntimeMemoryKindV1::HostVisible => {
                    self.directional_sdma_ops_v1().allocate_host(len)
                }
            };
            let mut buffer = result.map_err(|error| {
                self.terminal_error(format!("KFD persistent SDMA allocation: {error}"))
            })?;
            match kind {
                RuntimeMemoryKindV1::HostVisible => {
                    let initialized =
                        self.directional_sdma_ops_v1()
                            .write_host(&mut buffer, 0, &bytes);
                    if let Err(error) = initialized {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer),
                        );
                        return Err(self.terminal_error(format!(
                            "KFD persistent host allocation initialization: {error}"
                        )));
                    }
                    KfdRuntimeSdmaStorageV1::Host(buffer)
                }
                RuntimeMemoryKindV1::DeviceLocal => {
                    match self.directional_sdma_ops_v1().promote(buffer) {
                        Ok(allocation) => KfdRuntimeSdmaStorageV1::Device(Box::new(allocation)),
                        Err(failure) => {
                            return match failure {
                                SdmaTransitionFailureV1::Retryable {
                                    detail,
                                    custody: buffer,
                                } => {
                                    self.recycle_transient_sdma_buffer_v1(buffer, "promotion")?;
                                    Err(Self::rejected(
                                        KfdRuntimeBackendErrorKindV1::Native,
                                        format!("KFD persistent device promotion: {detail}"),
                                    ))
                                }
                                SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                                    self.retain_sdma_seam_terminal_v1(custody);
                                    Err(self.terminal_error(format!(
                                        "KFD persistent device promotion: {detail}"
                                    )))
                                }
                            };
                        }
                    }
                }
            }
        } else {
            KfdRuntimeSdmaStorageV1::Synthetic
        };
        let sdma_initialized = !self.native_available || kind == RuntimeMemoryKindV1::HostVisible;
        self.allocations.insert(
            id,
            AllocationRecordV1 {
                device,
                kind,
                alignment,
                bytes: bytes.into(),
                content_sha256: None,
                last_full_host_write: None,
                native_dirty: Vec::new(),
                sdma_storage,
                sdma_backed: self.native_available,
                sdma_initialized,
                sdma_shadow_dirty: false,
            },
        );
        self.staged_context_bytes = next_staged_context_bytes;
        if self.native_available && kind == RuntimeMemoryKindV1::DeviceLocal {
            if let Err(failure) = self.zero_sdma_range_v1(id, byte_len) {
                if matches!(failure, RuntimeBackendFailureV1::Terminal(_)) {
                    return Err(failure);
                }
                if let Err(cleanup) = self.discard_hidden_sdma_allocation_v1(id) {
                    return match cleanup {
                        failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
                        RuntimeBackendFailureV1::Rejected(_)
                        | RuntimeBackendFailureV1::Quiescent(_) => Err(self.terminal_error(
                            "hidden KFD allocation cleanup retained unreachable native custody",
                        )),
                    };
                }
                return Err(failure);
            }
            self.allocations
                .get_mut(&id)
                .expect("initialized device allocation remains indexed")
                .sdma_initialized = true;
        }
        let allocation = self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, id);
        self.observe_profile_v1(allocation.map(|allocation| {
            KfdRuntimeProfileEventKindV1::AllocationCreated {
                allocation,
                memory_kind: match kind {
                    RuntimeMemoryKindV1::HostVisible => KfdProfileMemoryKindV1::HostVisible,
                    RuntimeMemoryKindV1::DeviceLocal => {
                        KfdProfileMemoryKindV1::DeviceLocalHostStaged
                    }
                },
                byte_len,
                alignment,
            }
        }));
        Ok(id)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.allocations.contains_key(&allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            ));
        }
        if self.allocation_is_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending KFD dispatch",
            ));
        }
        self.release_all_compute_caches_for_allocation_v1(allocation)
            .map_err(Self::after_possible_host_mutation)?;
        let scrub_device_bytes = self.allocations.get(&allocation).and_then(|record| {
            (record.sdma_backed
                && record.kind == RuntimeMemoryKindV1::DeviceLocal
                && matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Device(_)))
            .then_some(record.bytes.len() as u64)
        });
        let scrub = if let Some(byte_len) = scrub_device_bytes {
            self.zero_sdma_range_v1(allocation, byte_len)
                .map_err(Self::after_possible_host_mutation)
        } else {
            Ok(())
        };
        scrub.map_err(Self::after_possible_host_mutation)?;
        if scrub_device_bytes.is_some() {
            let record = self
                .allocations
                .get_mut(&allocation)
                .expect("scrubbed device allocation remains indexed");
            record.sdma_shadow_dirty = true;
            record.content_sha256 = None;
            record.last_full_host_write = None;
        }
        self.release_sdma_storage_v1(allocation)
            .map_err(Self::after_possible_host_mutation)?;
        let profile_allocation =
            self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
        let removed = self.allocations.remove(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        self.staged_context_bytes = self
            .staged_context_bytes
            .checked_sub(removed.bytes.len() as u64)
            .expect("retained staged-byte accounting covers every allocation");
        self.observe_profile_v1(
            profile_allocation
                .map(|allocation| KfdRuntimeProfileEventKindV1::AllocationReleased { allocation }),
        );
        Ok(())
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.allocation_is_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending KFD dispatch",
            ));
        }
        let record = self.allocations.get(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        let offset = usize::try_from(byte_offset).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation offset does not fit host address space",
            )
        })?;
        let end = offset.checked_add(bytes.len()).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation write range overflow",
            )
        })?;
        if record.bytes.get(offset..end).is_none() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation write is out of bounds",
            ));
        }
        let full_write = offset == 0 && end == record.bytes.len();
        let full_image = if full_write {
            if let Some((image, digest)) = record
                .last_full_host_write
                .as_ref()
                .filter(|(image, _)| image.as_ref() == bytes)
            {
                Some((Arc::clone(image), *digest))
            } else {
                let image: Arc<[u8]> =
                    try_copy_vec_v1(bytes, "KFD complete host-write image allocation failed")?
                        .into();
                let digest = Sha256::digest(bytes).into();
                Some((image, digest))
            }
        } else {
            None
        };

        self.release_all_compute_caches_for_allocation_v1(allocation)
            .map_err(Self::after_possible_host_mutation)?;
        self.synchronize_sdma_shadow_v1(allocation)
            .map_err(Self::after_possible_host_mutation)?;
        if !self.allocations[&allocation].native_dirty.is_empty() {
            self.synchronize_native_allocation_v1(allocation)
                .map_err(Self::after_possible_host_mutation)?;
        }
        // Publish the persistent-SDMA image before changing retained host
        // authority. Earlier reconciliation may already have changed dirty
        // coordinates, so any later recovered rejection is Quiescent.
        self.upload_sdma_range_v1(allocation, byte_offset, bytes)
            .map_err(Self::after_possible_host_mutation)?;
        self.allocations
            .get_mut(&allocation)
            .expect("written allocation remains indexed")
            .sdma_initialized = true;

        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("validated allocation remains retained");
        if let Some((image, digest)) = full_image {
            record.bytes = Arc::clone(&image);
            record.content_sha256 = Some(digest);
            record.last_full_host_write = Some((image, digest));
        } else {
            let destination = Arc::make_mut(&mut record.bytes)
                .get_mut(offset..end)
                .ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "allocation write is out of bounds",
                    )
                })?;
            destination.copy_from_slice(bytes);
            record.content_sha256 = None;
        }
        let known_sha256 = full_write.then_some(record.content_sha256).flatten();
        let profile_allocation =
            self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
        let content = self.profile_host_content_v1(bytes, known_sha256);
        self.observe_profile_v1(
            profile_allocation
                .zip(content)
                .map(
                    |(allocation, content)| KfdRuntimeProfileEventKindV1::HostWrite {
                        allocation,
                        byte_offset,
                        content,
                    },
                ),
        );
        Ok(())
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.allocation_is_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending KFD dispatch",
            ));
        }
        let allocation_len = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD allocation",
                )
            })?
            .bytes
            .len();
        let offset = usize::try_from(byte_offset).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation offset does not fit host address space",
            )
        })?;
        let end = offset.checked_add(destination.len()).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation read range overflow",
            )
        })?;
        if end > allocation_len {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation read is out of bounds",
            ));
        }
        if self
            .read_native_allocation_into_v1(allocation, byte_offset, destination)
            .map_err(Self::after_possible_host_mutation)?
        {
            let profile_allocation =
                self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
            let content = self.profile_host_content_v1(destination, None);
            self.observe_profile_v1(profile_allocation.zip(content).map(
                |(allocation, content)| KfdRuntimeProfileEventKindV1::HostRead {
                    allocation,
                    byte_offset,
                    content,
                },
            ));
            return Ok(());
        }
        self.synchronize_native_allocation_v1(allocation)
            .map_err(Self::after_possible_host_mutation)?;
        if self
            .download_sdma_range_v1(allocation, byte_offset, destination)
            .map_err(Self::after_possible_host_mutation)?
        {
            return Ok(());
        }
        let record = self.allocations.get(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        let source = &record.bytes[offset..end];
        destination.copy_from_slice(source);
        let known_sha256 = (offset == 0 && end == record.bytes.len())
            .then_some(record.content_sha256)
            .flatten();
        let profile_allocation =
            self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
        let content = self.profile_host_content_v1(destination, known_sha256);
        self.observe_profile_v1(
            profile_allocation
                .zip(content)
                .map(
                    |(allocation, content)| KfdRuntimeProfileEventKindV1::HostRead {
                        allocation,
                        byte_offset,
                        content,
                    },
                ),
        );
        Ok(())
    }

    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_device(device)?;
        let owned_image = try_copy_vec_v1(image, "KFD module image allocation failed")?;
        let profile_artifact = self.profile_content_v1(&owned_image);
        let image_sha256 = Sha256::digest(&owned_image).into();
        let validated =
            validate_owned(owned_image, AdmittedProfile::Gfx942XnackOffCov6).map_err(|error| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    format!("invalid AMDHSA module: {error:?}"),
                )
            })?;
        self.modules
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD module-table growth failed"))?;
        let id = self.next_id()?;
        self.modules.insert(
            id,
            ModuleRecordV1 {
                device,
                validated,
                image_sha256,
            },
        );
        let profile_module = self.profile_resource_v1(KfdProfileResourceKindV1::Module, id);
        self.observe_profile_v1(
            profile_module
                .zip(profile_artifact)
                .map(
                    |(module, artifact)| KfdRuntimeProfileEventKindV1::ModuleLoaded {
                        module,
                        artifact,
                    },
                ),
        );
        Ok(id)
    }

    fn unload_module_v1(
        &mut self,
        module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.modules.contains_key(&module) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD module",
            ));
        }
        if self.compute_module_retain_counts.contains_key(&module) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "module is retained by a pending KFD dispatch",
            ));
        }
        if self.recycled_dispatch.as_ref().is_some_and(|recycled| {
            self.kernels
                .get(&recycled.kernel)
                .is_some_and(|kernel| kernel.module == module)
        }) {
            self.detach_recycled_dispatch()?;
        }
        for lane in 1..self.native_compute_lanes.len() {
            let detach = self.auxiliary_compute_lanes[lane - 1]
                .recycled_dispatch
                .as_ref()
                .is_some_and(|recycled| {
                    self.kernels
                        .get(&recycled.kernel)
                        .is_some_and(|kernel| kernel.module == module)
                });
            if detach {
                self.with_compute_lane_state_v1(lane, Self::detach_recycled_dispatch)?;
            }
        }
        let profile_module = self.profile_resource_v1(KfdProfileResourceKindV1::Module, module);
        self.modules.remove(&module);
        self.kernels.retain(|_, kernel| kernel.module != module);
        self.observe_profile_v1(
            profile_module.map(|module| KfdRuntimeProfileEventKindV1::ModuleUnloaded { module }),
        );
        Ok(())
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let record = self.modules.get(&module).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD module",
            )
        })?;
        let validated = record.validated.bind_kernel(name).map_err(|error| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                format!("AMDHSA kernel resolution: {error:?}"),
            )
        })?;
        let profile_module = self.profile_resource_v1(KfdProfileResourceKindV1::Module, module);
        let profile_name = self.profile_content_v1(name.as_bytes());
        let profile_signature = self.profile_content_v1(&signature);
        self.kernels
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD kernel-table growth failed"))?;
        let id = self.next_id()?;
        self.kernels.insert(
            id,
            KernelRecordV1 {
                module,
                validated,
                signature,
            },
        );
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, id);
        self.observe_profile_v1(
            profile_kernel
                .zip(profile_module)
                .zip(profile_name)
                .zip(profile_signature)
                .map(|(((kernel, module), name), signature)| {
                    KfdRuntimeProfileEventKindV1::KernelResolved {
                        kernel,
                        module,
                        name,
                        signature,
                    }
                }),
        );
        Ok(id)
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.queue_retired || !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "the admitted KFD queue lifecycle has already retired",
            ));
        }
        self.validate_semantic_launch_v1(launch.semantic_launch, launch.geometry)?;
        self.require_submission_capacity_v1()?;
        let prior_stream_submission = self.stream_submission_tails.get(&launch.stream).copied();
        let dependencies =
            self.collect_compute_dependencies_v1(launch.stream, launch.dependencies)?;
        let dependency_depth = self
            .next_dependency_depth_v1(&dependencies)
            .map_err(|error| {
                let detail = match error {
                    DirectSdmaDependencyDepthErrorV1::Overflow => {
                        "KFD compute dependency depth overflow"
                    }
                    DirectSdmaDependencyDepthErrorV1::LimitExceeded => {
                        "KFD compute dependency depth capacity exceeded"
                    }
                };
                Self::capacity(detail)
            })?;
        self.validate_compute_launch_v1(&launch, &dependencies)?;

        let explicit_kernarg = try_copy_vec_v1(
            launch.explicit_kernarg,
            "KFD pending kernarg custody allocation failed",
        )?
        .into_boxed_slice();
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD pending binding custody allocation failed"))?;
        bindings.extend_from_slice(launch.bindings);
        let mut retained_allocations = Vec::new();
        retained_allocations
            .try_reserve_exact(bindings.len())
            .map_err(|_| Self::capacity("KFD retained-allocation roster allocation failed"))?;
        for binding in &bindings {
            if !retained_allocations.contains(&binding.region.allocation) {
                retained_allocations.push(binding.region.allocation);
            }
        }
        let new_allocation_custody = self.reserve_allocation_custody_v1(&retained_allocations)?;
        let module = self
            .kernels
            .get(&launch.kernel)
            .expect("validated compute kernel remains indexed")
            .module;
        if !self.compute_module_retain_counts.contains_key(&module) {
            self.compute_module_retain_counts
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD module-retain index growth failed"))?;
        }
        if self
            .compute_module_retain_counts
            .get(&module)
            .is_some_and(|count| *count == usize::MAX)
        {
            return Err(Self::capacity("KFD module retain count overflow"));
        }
        let next_completion_reservations = self
            .compute_completion_reservations
            .checked_add(1)
            .ok_or_else(|| Self::capacity("KFD compute completion reservation overflow"))?;
        let total_completion_reservations = next_completion_reservations
            .checked_add(self.sdma_completion_reservations)
            .ok_or_else(|| Self::capacity("KFD completion reservation overflow"))?;
        self.submissions
            .try_reserve(total_completion_reservations)
            .map_err(|_| Self::capacity("KFD submission-table growth failed"))?;
        self.pending_compute
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD pending-compute ledger growth failed"))?;
        if !self.pending_compute_streams.contains_key(&launch.stream) {
            self.pending_compute_streams
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD compute stream-FIFO index growth failed"))?;
        }
        if !self.stream_submission_tails.contains_key(&launch.stream) {
            self.stream_submission_tails
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD stream-tail index growth failed"))?;
        }
        if !self.stream_compute_lanes.contains_key(&launch.stream) {
            self.stream_compute_lanes
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD compute-lane lease index growth failed"))?;
        }
        let new_dependency_entries = dependencies
            .iter()
            .filter(|submission| {
                !self
                    .compute_dependency_retain_counts
                    .contains_key(submission)
            })
            .count();
        self.compute_dependency_retain_counts
            .try_reserve(new_dependency_entries)
            .map_err(|_| Self::capacity("KFD compute dependency-retain growth failed"))?;
        if dependencies.iter().any(|submission| {
            self.compute_dependency_retain_counts
                .get(submission)
                .is_some_and(|count| *count == usize::MAX)
        }) {
            return Err(Self::capacity(
                "KFD compute dependency retain count overflow",
            ));
        }
        if self.next_handle == u64::MAX {
            return Err(Self::capacity("backend handle space exhausted"));
        }
        let mut new_stream_queue = None;
        if let Some(stream_queue) = self.pending_compute_streams.get_mut(&launch.stream) {
            stream_queue
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD compute stream FIFO growth failed"))?;
        } else {
            let mut stream_queue = VecDeque::new();
            stream_queue
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD compute stream FIFO growth failed"))?;
            new_stream_queue = Some(stream_queue);
        }
        let id = self.next_id()?;
        self.retain_allocation_custody_v1(
            &retained_allocations,
            RuntimeAllocationCustodyOwnerV1 {
                submission: id,
                stream: launch.stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            },
            new_allocation_custody,
        );
        *self.compute_module_retain_counts.entry(module).or_insert(0) += 1;
        for dependency in &dependencies {
            *self
                .compute_dependency_retain_counts
                .entry(*dependency)
                .or_insert(0) += 1;
        }
        self.compute_completion_reservations = next_completion_reservations;
        self.stream_submission_tails.insert(launch.stream, id);
        if let Some(mut stream_queue) = new_stream_queue {
            stream_queue.push_back(id);
            self.pending_compute_streams
                .insert(launch.stream, stream_queue);
        } else {
            self.pending_compute_streams
                .get_mut(&launch.stream)
                .expect("reserved compute stream FIFO remains indexed")
                .push_back(id);
        }
        self.pending_compute.insert(
            id,
            PendingComputeSubmissionV1 {
                id,
                module,
                launch: OwnedComputeLaunchV1 {
                    stream: launch.stream,
                    kernel: launch.kernel,
                    explicit_kernarg,
                    bindings: bindings.into_boxed_slice(),
                    geometry: launch.geometry,
                    semantic_launch: launch.semantic_launch,
                },
                retained_allocations: retained_allocations.into_boxed_slice(),
                prior_stream_submission,
                dependencies,
                dependency_cursor: 0,
                dependency_depth,
            },
        );
        if self.pending_compute_can_publish_under_deadline_v1(id) {
            let pending = self
                .pending_compute
                .remove(&id)
                .expect("accepted clean compute remains pending before first progress");
            let _ = self.progress_pending_compute_v1(pending)?;
        }
        Ok(id)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.quiescent_sdma_submissions.contains(&submission) {
            debug_assert!(self.submissions.contains_key(&submission));
            return Err(Self::quiescent_error(
                KfdRuntimeBackendErrorKindV1::Native,
                "KFD SDMA submission is quiescent without a complete result",
            ));
        }
        if let Some(record) = self.submissions.get(&submission) {
            return Ok(record.status);
        }
        if self.pending_compute.contains_key(&submission) {
            if self.free_compute_lane_v1().is_none() {
                for (lane, active) in self
                    .active_compute_progress_roster_v1()
                    .into_iter()
                    .enumerate()
                {
                    if active {
                        let _ = self.poll_compute_lane_v1(lane)?;
                    }
                }
            }
            let pending = self
                .pending_compute
                .remove(&submission)
                .expect("known pending compute remains indexed");
            return self.observe_pending_compute_v1(pending);
        }
        if let Some(mut active) = self.active_sdma.remove(&submission) {
            let phase = std::mem::replace(&mut active.phase, ActiveDirectionalSdmaPhaseV1::Ready);
            let ActiveDirectionalSdmaPhaseV1::Published(native_submission) = phase else {
                return self.observe_unpublished_sdma_copy_v1(active);
            };
            let poll = self.directional_sdma_ops_v1().poll(*native_submission);
            return match poll {
                Ok(DirectionalSdmaPollV1::Pending(native_submission)) => {
                    active.phase =
                        ActiveDirectionalSdmaPhaseV1::Published(Box::new(native_submission));
                    self.active_sdma.insert(submission, active);
                    Ok(BackendPollV1::Pending)
                }
                Ok(DirectionalSdmaPollV1::Completed(completed)) => {
                    self.finish_sdma_copy_v1(active, completed)
                }
                Err(failure) => match failure {
                    DirectionalSdmaExecutionFailureV1::Retryable {
                        detail,
                        submission: native_submission,
                    } => {
                        active.phase =
                            ActiveDirectionalSdmaPhaseV1::Published(Box::new(native_submission));
                        self.active_sdma.insert(submission, active);
                        Err(Self::rejected(
                            KfdRuntimeBackendErrorKindV1::Native,
                            format!("KFD directional SDMA completion observation: {detail}"),
                        ))
                    }
                    DirectionalSdmaExecutionFailureV1::ProcessTeardown { detail, custody } => {
                        self.retain_sdma_seam_terminal_v1(custody);
                        Err(self.terminal_error(format!(
                            "KFD directional SDMA completion observation: {detail}"
                        )))
                    }
                },
            };
        }
        let lane = self.active_compute_lane_v1(submission).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD submission",
            )
        })?;
        self.poll_compute_lane_v1(lane)
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        loop {
            let status = self.poll_v1(submission)?;
            if status != BackendPollV1::Pending {
                return Ok(status);
            }
            attempts = attempts.saturating_add(1);
            if !apply_wait_backoff_v1(attempts, &mut sleep, deadline) {
                return Ok(BackendPollV1::Pending);
            }
        }
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.active_compute_lane_v1(submission).is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission still owns a pending KFD dispatch",
            ));
        }
        if self.pending_compute.contains_key(&submission) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission still owns an unpublished KFD dispatch",
            ));
        }
        if self.active_sdma.contains_key(&submission) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission still owns a pending KFD SDMA copy",
            ));
        }
        if self
            .event_submission_retain_counts
            .contains_key(&submission)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission is retained by a live event",
            ));
        }
        if self.sdma_dependency_retain_counts.contains_key(&submission) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission is retained by a pending KFD SDMA dependency",
            ));
        }
        if self
            .compute_dependency_retain_counts
            .contains_key(&submission)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission is retained by a pending KFD compute dependency",
            ));
        }
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, submission);
        let removed = self.submissions.remove(&submission).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD submission",
            )
        })?;
        if self.stream_submission_tails.get(&removed.stream) == Some(&submission) {
            self.stream_submission_tails.remove(&removed.stream);
        }
        self.quiescent_sdma_submissions.remove(&submission);
        self.observe_profile_v1(
            profile_dispatch
                .map(|dispatch| KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch }),
        );
        Ok(())
    }

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD stream",
            ));
        }
        let submission_stream = self
            .submissions
            .get(&submission)
            .map(|record| record.stream)
            .or_else(|| {
                self.active_compute_submission_v1(submission)
                    .map(|active| active.stream)
            })
            .or_else(|| {
                self.pending_compute_submission_v1(submission)
                    .map(|pending| pending.launch.stream)
            })
            .or_else(|| {
                self.active_sdma
                    .get(&submission)
                    .map(|active| active.stream)
            })
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD submission",
                )
            })?;
        if submission_stream != stream {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "submission belongs to a different stream",
            ));
        }
        if self.events.len() >= MAX_RUNTIME_EVENTS_V1 {
            return Err(Self::capacity("KFD event capacity exceeded"));
        }
        self.events
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD event-table growth failed"))?;
        self.reserve_event_submission_retain_v1(submission)?;
        let id = self.next_id()?;
        self.events.insert(id, EventRecordV1 { submission });
        self.retain_event_submission_v1(submission);
        Ok(id)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let record = self.events.remove(&event).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD event",
            )
        })?;
        self.release_event_submission_v1(record.submission);
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        _stream: u64,
        _source: BackendMemoryRegionV1,
        _destination: BackendMemoryRegionV1,
        _dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "peer copy requires an admitted multi-device copy path",
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RoutedHandleV1 {
    child: usize,
    local: u64,
}

#[derive(Debug)]
enum RoutedSubmissionV1 {
    Native { route: RoutedHandleV1, stream: u64 },
    CooperativeCopy(CooperativeCopySubmissionV1),
}

#[derive(Clone, Copy, Debug)]
enum RoutedEventV1 {
    Native {
        route: RoutedHandleV1,
        submission: u64,
    },
    CooperativeCopy {
        submission: u64,
        child: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CooperativeCopyPhaseV1 {
    Dependencies,
    Read,
    Write,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug)]
struct CooperativeCopySubmissionV1 {
    stream: u64,
    prior_stream_submission: Option<u64>,
    source: RoutedHandleV1,
    source_region: BackendMemoryRegionV1,
    destination: RoutedHandleV1,
    destination_region: BackendMemoryRegionV1,
    dependencies: Vec<u64>,
    dependency_cursor: usize,
    dependency_depth: usize,
    staging: Vec<u8>,
    phase: CooperativeCopyPhaseV1,
    byte_cursor: usize,
}

impl CooperativeCopySubmissionV1 {
    const fn status(&self) -> BackendPollV1 {
        match self.phase {
            CooperativeCopyPhaseV1::Succeeded => BackendPollV1::Succeeded,
            CooperativeCopyPhaseV1::Failed => BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            },
            CooperativeCopyPhaseV1::Cancelled => BackendPollV1::Failed { code: -2 },
            CooperativeCopyPhaseV1::Dependencies
            | CooperativeCopyPhaseV1::Read
            | CooperativeCopyPhaseV1::Write => BackendPollV1::Pending,
        }
    }

    const fn is_quiescent(&self) -> bool {
        matches!(
            self.phase,
            CooperativeCopyPhaseV1::Succeeded
                | CooperativeCopyPhaseV1::Failed
                | CooperativeCopyPhaseV1::Cancelled
        )
    }
}

/// Process-local multi-device KFD router.
///
/// Every selected device is admitted before any child lazily creates a VM or
/// queue, satisfying KFD's process-wide no-queue XNACK barrier. Dispatches on
/// different children can execute independently. Live same-device copies use
/// the selected child's native SDMA path. Peer copies use a bounded,
/// explicitly flush-driven host staging state machine; poll and deadline wait
/// only observe stored state. Native XGMI is exposed only by
/// [`KfdNativeXgmiRuntimeBackendV1`]. Mixed native/cooperative work is rejected
/// while either domain remains live on one logical stream.
#[must_use = "multi-device KFD backends must remain owned through quiescence"]
pub struct KfdMultiDeviceRuntimeBackendV1 {
    children: Vec<KfdRuntimeBackendV1>,
    device_children: HashMap<u64, usize>,
    terminal: bool,
    next_handle: u64,
    streams: HashMap<u64, RoutedHandleV1>,
    allocations: HashMap<u64, RoutedHandleV1>,
    modules: HashMap<u64, RoutedHandleV1>,
    kernels: HashMap<u64, RoutedHandleV1>,
    kernel_modules: HashMap<u64, u64>,
    submissions: HashMap<u64, RoutedSubmissionV1>,
    events: HashMap<u64, RoutedEventV1>,
    cooperative_allocation_owners: HashMap<RoutedHandleV1, Vec<u64>>,
    cooperative_dependency_retain_counts: HashMap<u64, usize>,
    cooperative_stream_pending_counts: HashMap<u64, usize>,
    cooperative_stream_tails: HashMap<u64, u64>,
    native_stream_submission_counts: HashMap<u64, usize>,
    event_submission_retain_counts: HashMap<u64, usize>,
    cooperative_progress_generation: u64,
    cooperative_staging_bytes: u64,
    cooperative_staging_limit_bytes: u64,
}

enum XgmiAllocationAuthorityV1 {
    Unmapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
    /// Fully mapped into the exact two-device roster and available for reuse.
    Mapped(Gfx942XgmiMappedDeviceMemoryV1),
    QuarantinedMapped(Gfx942XgmiMappedDeviceMemoryV1),
}

struct XgmiRuntimeAllocationV1 {
    device: usize,
    byte_len: u64,
    alignment: u64,
    authority: Option<XgmiAllocationAuthorityV1>,
}

struct XgmiRuntimeSubmissionV1 {
    id: u64,
    stream: u64,
    direction: usize,
    source: u64,
    destination: u64,
    source_offset: u64,
    destination_offset: u64,
    byte_len: u32,
    dependencies: Vec<u64>,
    dependency_cursor: usize,
    ticket: Option<Gfx942SdmaCopyTicketV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiPairAdmissionErrorV1 {
    ZeroUniqueId,
    DuplicateUniqueId,
}

const fn admit_xgmi_unique_id_pair_v1(
    first_unique_id: u64,
    second_unique_id: u64,
) -> Result<(), XgmiPairAdmissionErrorV1> {
    if first_unique_id == 0 || second_unique_id == 0 {
        return Err(XgmiPairAdmissionErrorV1::ZeroUniqueId);
    }
    if first_unique_id == second_unique_id {
        return Err(XgmiPairAdmissionErrorV1::DuplicateUniqueId);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiPeerCopyAdmissionErrorV1 {
    UnknownDevice,
    SameDevice,
    WrongDestinationStream,
    ZeroLength,
    LengthMismatch,
    PacketTooLarge,
    SourceRange,
    DestinationRange,
    SourceAccess,
    DestinationAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XgmiPeerCopyAdmissionV1 {
    stream_device: usize,
    source_device: usize,
    destination_device: usize,
    source_offset: u64,
    source_len: u64,
    source_allocation_len: u64,
    source_access: RuntimeAccessV1,
    destination_offset: u64,
    destination_len: u64,
    destination_allocation_len: u64,
    destination_access: RuntimeAccessV1,
}

fn admit_xgmi_peer_copy_v1(
    request: XgmiPeerCopyAdmissionV1,
) -> Result<usize, XgmiPeerCopyAdmissionErrorV1> {
    if request.stream_device > 1 || request.source_device > 1 || request.destination_device > 1 {
        return Err(XgmiPeerCopyAdmissionErrorV1::UnknownDevice);
    }
    if request.source_device == request.destination_device {
        return Err(XgmiPeerCopyAdmissionErrorV1::SameDevice);
    }
    if request.stream_device != request.destination_device {
        return Err(XgmiPeerCopyAdmissionErrorV1::WrongDestinationStream);
    }
    if request.source_len == 0 {
        return Err(XgmiPeerCopyAdmissionErrorV1::ZeroLength);
    }
    if request.source_len != request.destination_len {
        return Err(XgmiPeerCopyAdmissionErrorV1::LengthMismatch);
    }
    if request.source_len > u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) {
        return Err(XgmiPeerCopyAdmissionErrorV1::PacketTooLarge);
    }
    if request
        .source_offset
        .checked_add(request.source_len)
        .is_none_or(|end| end > request.source_allocation_len)
    {
        return Err(XgmiPeerCopyAdmissionErrorV1::SourceRange);
    }
    if request
        .destination_offset
        .checked_add(request.destination_len)
        .is_none_or(|end| end > request.destination_allocation_len)
    {
        return Err(XgmiPeerCopyAdmissionErrorV1::DestinationRange);
    }
    if !matches!(
        request.source_access,
        RuntimeAccessV1::Read | RuntimeAccessV1::ReadWrite
    ) {
        return Err(XgmiPeerCopyAdmissionErrorV1::SourceAccess);
    }
    if !matches!(
        request.destination_access,
        RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
    ) {
        return Err(XgmiPeerCopyAdmissionErrorV1::DestinationAccess);
    }

    // Direction indexes the source device's retained directional route and
    // queue. The public peer-copy stream belongs to the destination device.
    Ok(request.source_device)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiDependencyAdmissionErrorV1 {
    TooMany,
    Capacity,
    Unknown,
    Duplicate,
}

fn collect_xgmi_dependencies_v1(
    events: &HashMap<u64, EventRecordV1>,
    dependencies: &[u64],
) -> Result<Vec<u64>, XgmiDependencyAdmissionErrorV1> {
    if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
        return Err(XgmiDependencyAdmissionErrorV1::TooMany);
    }
    let mut submissions = Vec::new();
    submissions
        .try_reserve_exact(dependencies.len())
        .map_err(|_| XgmiDependencyAdmissionErrorV1::Capacity)?;
    for event in dependencies {
        let submission = events
            .get(event)
            .map(|event| event.submission)
            .ok_or(XgmiDependencyAdmissionErrorV1::Unknown)?;
        if submissions.contains(&submission) {
            return Err(XgmiDependencyAdmissionErrorV1::Duplicate);
        }
        submissions.push(submission);
    }
    Ok(submissions)
}

#[cfg(test)]
fn has_unordered_xgmi_overlap_v1<'a>(
    active: impl Iterator<Item = &'a XgmiRuntimeSubmissionV1>,
    source: u64,
    destination: u64,
    dependencies: &[u64],
) -> bool {
    active.into_iter().any(|submission| {
        (submission.source == source
            || submission.destination == source
            || submission.source == destination
            || submission.destination == destination)
            && !dependencies.contains(&submission.id)
    })
}

#[cfg(test)]
fn xgmi_allocation_is_active_v1<'a>(
    active: impl Iterator<Item = &'a XgmiRuntimeSubmissionV1>,
    allocation: u64,
) -> bool {
    active
        .into_iter()
        .any(|submission| submission.source == allocation || submission.destination == allocation)
}

#[cfg(test)]
fn has_active_xgmi_stream_v1<'a>(
    active: impl Iterator<Item = &'a XgmiRuntimeSubmissionV1>,
    stream: u64,
) -> bool {
    active
        .into_iter()
        .any(|submission| submission.stream == stream)
}

fn next_xgmi_dependency_depth_v1(
    depths: &HashMap<u64, usize>,
    dependencies: &[u64],
) -> Result<usize, XgmiDependencyAdmissionErrorV1> {
    let mut maximum = 0;
    for dependency in dependencies {
        maximum = maximum.max(
            *depths
                .get(dependency)
                .ok_or(XgmiDependencyAdmissionErrorV1::Unknown)?,
        );
    }
    let next = maximum
        .checked_add(1)
        .ok_or(XgmiDependencyAdmissionErrorV1::TooMany)?;
    if next > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        return Err(XgmiDependencyAdmissionErrorV1::TooMany);
    }
    Ok(next)
}

fn xgmi_submission_is_ready_v1(
    submission: &XgmiRuntimeSubmissionV1,
    completed: &HashMap<u64, SubmissionRecordV1>,
    direction: usize,
) -> bool {
    submission.direction == direction
        && submission.ticket.is_none()
        && submission.dependencies.iter().all(|dependency| {
            completed
                .get(dependency)
                .is_some_and(|record| record.status == BackendPollV1::Succeeded)
        })
}

fn xgmi_submission_has_failed_dependency_v1(
    submission: &XgmiRuntimeSubmissionV1,
    completed: &HashMap<u64, SubmissionRecordV1>,
) -> bool {
    submission.dependencies.iter().any(|dependency| {
        completed
            .get(dependency)
            .is_some_and(|record| matches!(record.status, BackendPollV1::Failed { .. }))
    })
}

#[cfg(test)]
fn ready_xgmi_batch_ids_v1(
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: &HashMap<u64, SubmissionRecordV1>,
    direction: usize,
    limit: usize,
) -> Result<Vec<u64>, XgmiBatchSelectionErrorV1> {
    let limit = limit.min(GFX942_SDMA_MAX_IN_FLIGHT_V1);
    let mut ids = Vec::new();
    ids.try_reserve_exact(limit)
        .map_err(|_| XgmiBatchSelectionErrorV1::Capacity)?;
    if limit == 0 {
        return Ok(ids);
    }
    for submission in active
        .values()
        .filter(|submission| xgmi_submission_is_ready_v1(submission, completed, direction))
    {
        let insertion = ids.partition_point(|id| *id < submission.id);
        if ids.len() < limit {
            ids.insert(insertion, submission.id);
        } else if insertion < limit {
            ids.pop();
            ids.insert(insertion, submission.id);
        }
    }
    Ok(ids)
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiBatchSelectionErrorV1 {
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiFlushAdmissionV1 {
    NoReadyWork,
    Publish { ready: usize },
    InFlight,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiBatchPublicationOutcomeV1 {
    NoReadyWork,
    Published,
    AlreadyInFlight,
    RecoveredPrepublicationFailure,
}

const fn classify_xgmi_flush_v1(
    ready: usize,
    in_flight: bool,
    limit: usize,
) -> XgmiFlushAdmissionV1 {
    if ready == 0 {
        XgmiFlushAdmissionV1::NoReadyWork
    } else if in_flight {
        XgmiFlushAdmissionV1::InFlight
    } else if limit == 0 {
        XgmiFlushAdmissionV1::Capacity
    } else {
        XgmiFlushAdmissionV1::Publish {
            ready: if ready < limit { ready } else { limit },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XgmiFlushPrefixProgressV1 {
    remaining_at_entry: usize,
    completed_prefixes: usize,
}

impl XgmiFlushPrefixProgressV1 {
    const fn new(ready_at_entry: usize) -> Self {
        Self {
            remaining_at_entry: ready_at_entry,
            completed_prefixes: 0,
        }
    }

    const fn next_batch_len(self) -> usize {
        if self.remaining_at_entry < GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            self.remaining_at_entry
        } else {
            GFX942_SDMA_MAX_IN_FLIGHT_V1
        }
    }

    fn note_published(&mut self, published: usize) {
        assert!(published != 0 && published <= self.remaining_at_entry);
        self.remaining_at_entry -= published;
    }

    fn note_completed_prefix(&mut self) {
        self.completed_prefixes = self
            .completed_prefixes
            .checked_add(1)
            .expect("bounded XGMI flush prefix count");
    }

    fn classify_publication_failure(
        self,
        failure: RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        if self.completed_prefixes == 0 {
            failure
        } else {
            KfdRuntimeBackendV1::after_possible_host_mutation(failure)
        }
    }
}

const fn xgmi_direction_for_destination_v1(destination: usize) -> Option<usize> {
    match destination {
        0 => Some(1),
        1 => Some(0),
        _ => None,
    }
}

fn indexed_xgmi_progress_id_v1(in_flight: &[u64], focus: u64) -> Option<u64> {
    if in_flight.binary_search(&focus).is_ok() {
        Some(focus)
    } else {
        in_flight.first().copied()
    }
}

fn insert_ordered_xgmi_id_v1(ids: &mut Vec<u64>, id: u64) {
    match ids.binary_search(&id) {
        Ok(_) => std::process::abort(),
        Err(index) => {
            if ids.len() == ids.capacity() {
                std::process::abort();
            }
            ids.insert(index, id);
        }
    }
}

fn remove_ordered_xgmi_id_v1(ids: &mut Vec<u64>, id: u64) -> bool {
    let Ok(index) = ids.binary_search(&id) else {
        return false;
    };
    ids.remove(index);
    true
}

fn enqueue_xgmi_ready_id_v1(ids: &mut VecDeque<u64>, id: u64) {
    if ids.len() == ids.capacity() {
        std::process::abort();
    }
    ids.push_back(id);
}

fn prepend_xgmi_ready_id_v1(ids: &mut VecDeque<u64>, id: u64) {
    if ids.len() == ids.capacity() {
        std::process::abort();
    }
    ids.push_front(id);
}

fn remove_xgmi_ready_id_v1(ids: &mut VecDeque<u64>, id: u64) -> bool {
    let Some(index) = ids.iter().position(|candidate| *candidate == id) else {
        return false;
    };
    ids.remove(index);
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiProgressIndexPhaseV1 {
    InFlight,
    Ready,
    Waiting,
}

fn remove_xgmi_progress_index_v1(
    ready: &mut VecDeque<u64>,
    in_flight: &mut Vec<u64>,
    id: u64,
) -> XgmiProgressIndexPhaseV1 {
    if remove_ordered_xgmi_id_v1(in_flight, id) {
        return XgmiProgressIndexPhaseV1::InFlight;
    }
    if remove_xgmi_ready_id_v1(ready, id) {
        XgmiProgressIndexPhaseV1::Ready
    } else {
        XgmiProgressIndexPhaseV1::Waiting
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiCompletionReservationErrorV1 {
    Capacity,
}

fn reserve_xgmi_completion_slot_v1(
    submissions: &mut HashMap<u64, SubmissionRecordV1>,
    reservations: &mut usize,
) -> Result<(), XgmiCompletionReservationErrorV1> {
    let next = reservations
        .checked_add(1)
        .ok_or(XgmiCompletionReservationErrorV1::Capacity)?;
    submissions
        .try_reserve(next)
        .map_err(|_| XgmiCompletionReservationErrorV1::Capacity)?;
    *reservations = next;
    Ok(())
}

fn release_xgmi_dependencies_v1(
    dependency_retain_counts: &mut HashMap<u64, usize>,
    dependencies: &[u64],
) {
    for dependency in dependencies {
        let remove = {
            let count = dependency_retain_counts
                .get_mut(dependency)
                .expect("active XGMI dependency remains retained");
            *count -= 1;
            *count == 0
        };
        if remove {
            dependency_retain_counts.remove(dependency);
        }
    }
}

#[cfg(test)]
fn finish_failed_xgmi_batch_records_v1(
    dependency_retain_counts: &mut HashMap<u64, usize>,
    submissions: &mut HashMap<u64, SubmissionRecordV1>,
    completion_reservations: &mut usize,
    active_batch: impl IntoIterator<Item = XgmiRuntimeSubmissionV1>,
) {
    for active in active_batch {
        settle_xgmi_submission_record_v1(
            dependency_retain_counts,
            submissions,
            completion_reservations,
            active,
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            },
        );
    }
}

fn settle_xgmi_submission_record_v1(
    dependency_retain_counts: &mut HashMap<u64, usize>,
    submissions: &mut HashMap<u64, SubmissionRecordV1>,
    completion_reservations: &mut usize,
    active: XgmiRuntimeSubmissionV1,
    status: BackendPollV1,
) {
    if *completion_reservations == 0
        || submissions.capacity().saturating_sub(submissions.len()) < *completion_reservations
        || submissions.contains_key(&active.id)
    {
        std::process::abort();
    }
    release_xgmi_dependencies_v1(dependency_retain_counts, &active.dependencies);
    submissions.insert(
        active.id,
        SubmissionRecordV1 {
            stream: active.stream,
            status,
        },
    );
    *completion_reservations -= 1;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XgmiCancellationDispositionV1 {
    CancelPrepublication,
    TooLate,
    Unknown,
}

const fn xgmi_cancellation_disposition_v1(
    active_has_ticket: Option<bool>,
    has_quiescent_record: bool,
) -> XgmiCancellationDispositionV1 {
    match (active_has_ticket, has_quiescent_record) {
        (Some(false), false) => XgmiCancellationDispositionV1::CancelPrepublication,
        (Some(true), _) | (_, true) => XgmiCancellationDispositionV1::TooLate,
        (None, false) => XgmiCancellationDispositionV1::Unknown,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct XgmiLogicalResourceCountsV1 {
    streams: usize,
    allocations: usize,
    submissions: usize,
    active: usize,
    events: usize,
    event_retains: usize,
    dependency_retains: usize,
    dependency_depths: usize,
    dependency_waiters: usize,
    completion_reservations: usize,
    ready_index_entries: usize,
    in_flight_index_entries: usize,
    directional_active: usize,
    stream_owners: usize,
    allocation_owners: usize,
}

impl XgmiLogicalResourceCountsV1 {
    const fn permits_shutdown(self) -> bool {
        self.streams == 0
            && self.allocations == 0
            && self.submissions == 0
            && self.active == 0
            && self.events == 0
            && self.event_retains == 0
            && self.dependency_retains == 0
            && self.dependency_depths == 0
            && self.dependency_waiters == 0
            && self.completion_reservations == 0
            && self.ready_index_entries == 0
            && self.in_flight_index_entries == 0
            && self.directional_active == 0
            && self.stream_owners == 0
            && self.allocation_owners == 0
    }
}

fn native_xgmi_execution_capabilities_v1() -> RuntimeExecutionCapabilitiesV1 {
    RuntimeExecutionCapabilitiesV1 {
        native_peer_copy: true,
        cancellation: true,
        ..RuntimeExecutionCapabilitiesV1::default()
    }
}

/// Exact two-device, copy-only gfx942 native-XGMI runtime backend.
///
/// This owner acquires both process VMs before allocating memory, retains the
/// two directional topology routes, and retains successful PUBLIC VRAM peer
/// mappings across copies until host access or allocation release requires an
/// explicit unmap. It intentionally does not expose compute launch
/// or same-device copy: the current low-level XGMI queue requires raw access to
/// both VM sessions, while the compute adapter consumes a session into its queue.
#[must_use = "native XGMI backends must remain owned through quiescence"]
pub struct KfdNativeXgmiRuntimeBackendV1 {
    descriptions: [BackendDeviceDescriptionV1; 2],
    sessions: [SharedGttMemorySessionV1; 2],
    routes: [Gfx942XgmiRouteV1; 2],
    queues: [Option<Gfx942NativeXgmiSdmaQueueV1>; 2],
    terminal: bool,
    shutdown: bool,
    next_handle: u64,
    streams: HashMap<u64, usize>,
    allocations: HashMap<u64, XgmiRuntimeAllocationV1>,
    submissions: HashMap<u64, SubmissionRecordV1>,
    active: HashMap<u64, XgmiRuntimeSubmissionV1>,
    active_stream_owners: HashMap<u64, u64>,
    active_allocation_owners: HashMap<u64, Vec<u64>>,
    ready_by_direction: [VecDeque<u64>; 2],
    in_flight_by_direction: [Vec<u64>; 2],
    active_by_direction: [usize; 2],
    completion_reservations: usize,
    events: HashMap<u64, EventRecordV1>,
    event_submission_retain_counts: HashMap<u64, usize>,
    dependency_retain_counts: HashMap<u64, usize>,
    dependency_depths: HashMap<u64, usize>,
    dependency_waiters: HashMap<u64, Vec<u64>>,
}

impl fmt::Debug for KfdNativeXgmiRuntimeBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mapped_allocations = self
            .allocations
            .values()
            .filter(|allocation| {
                matches!(
                    allocation.authority.as_ref(),
                    Some(XgmiAllocationAuthorityV1::Mapped(mapping))
                        if mapping.is_fully_mapped()
                )
            })
            .count();
        let quarantined_mappings = self
            .allocations
            .values()
            .filter(|allocation| {
                matches!(
                    allocation.authority.as_ref(),
                    Some(XgmiAllocationAuthorityV1::QuarantinedMapped(mapping))
                        if !mapping.gpu_ids().is_empty()
                )
            })
            .count();
        let max_alignment = self
            .allocations
            .values()
            .map(|allocation| allocation.alignment)
            .max();
        formatter
            .debug_struct("KfdNativeXgmiRuntimeBackendV1")
            .field("devices", &self.descriptions)
            .field(
                "queues",
                &self.queues.iter().filter(|queue| queue.is_some()).count(),
            )
            .field("streams", &self.streams.len())
            .field("allocations", &self.allocations.len())
            .field("mapped_allocations", &mapped_allocations)
            .field("quarantined_mappings", &quarantined_mappings)
            .field("max_alignment", &max_alignment)
            .field("submissions", &self.submissions.len())
            .field("active", &self.active.len())
            .field("active_stream_owners", &self.active_stream_owners.len())
            .field(
                "active_allocation_owners",
                &self.active_allocation_owners.len(),
            )
            .field("ready_by_direction", &self.ready_by_direction)
            .field("in_flight_by_direction", &self.in_flight_by_direction)
            .field("completion_reservations", &self.completion_reservations)
            .field("events", &self.events.len())
            .field(
                "event_submission_retain_counts",
                &self.event_submission_retain_counts.len(),
            )
            .field("dependency_depths", &self.dependency_depths.len())
            .field("terminal", &self.terminal)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for KfdMultiDeviceRuntimeBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdMultiDeviceRuntimeBackendV1")
            .field("devices", &self.device_children.len())
            .field("streams", &self.streams.len())
            .field("allocations", &self.allocations.len())
            .field("modules", &self.modules.len())
            .field("kernels", &self.kernels.len())
            .field("submissions", &self.submissions.len())
            .field("events", &self.events.len())
            .field(
                "cooperative_allocation_owners",
                &self.cooperative_allocation_owners.len(),
            )
            .field(
                "cooperative_dependency_retain_counts",
                &self.cooperative_dependency_retain_counts.len(),
            )
            .field(
                "cooperative_stream_pending_counts",
                &self.cooperative_stream_pending_counts.len(),
            )
            .field(
                "cooperative_stream_tails",
                &self.cooperative_stream_tails.len(),
            )
            .field(
                "event_submission_retain_counts",
                &self.event_submission_retain_counts.len(),
            )
            .field("cooperative_staging_bytes", &self.cooperative_staging_bytes)
            .field(
                "cooperative_staging_limit_bytes",
                &self.cooperative_staging_limit_bytes,
            )
            .finish_non_exhaustive()
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    /// Admits all selected devices before any queue can be materialized.
    pub fn open_default(
        devices: Vec<(u64, Box<dyn KfdRuntimeLaunchAuthorityV1>)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let mut gated = Vec::new();
        gated.try_reserve_exact(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device authority roster allocation failed",
            )
        })?;
        gated.extend(
            devices
                .into_iter()
                .map(|(device, authority)| (device, KfdRuntimeLaunchGateV1::Production(authority))),
        );
        Self::open_default_with_gates_v1(gated)
    }

    /// Admits multiple devices with exact semantic launch authorities.
    pub fn open_default_with_semantic_authorities_v1(
        devices: Vec<(u64, Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let mut gated = Vec::new();
        gated.try_reserve_exact(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device semantic-authority roster allocation failed",
            )
        })?;
        gated.extend(
            devices
                .into_iter()
                .map(|(device, authority)| (device, KfdRuntimeLaunchGateV1::Semantic(authority))),
        );
        Self::open_default_with_gates_v1(gated)
    }

    fn open_default_with_gates_v1(
        devices: Vec<(u64, KfdRuntimeLaunchGateV1)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if devices.len() < 2 {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device KFD requires at least two devices",
            ));
        }
        let mut checked = Vec::new();
        checked.try_reserve_exact(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device checked-device roster allocation failed",
            )
        })?;
        let mut seen = HashSet::new();
        seen.try_reserve(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device identity-set allocation failed",
            )
        })?;
        for (unique_id, gate) in devices {
            if unique_id == 0 || !seen.insert(unique_id) {
                return Err(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "multi-device unique IDs must be nonzero and distinct",
                ));
            }
            let opened = OpenedKfd::open_default().map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    error.to_string(),
                )
            })?;
            let admitted = opened.admit_uapi().map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    error.to_string(),
                )
            })?;
            let device = admitted
                .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))
                .map_err(|error| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Native,
                        error.to_string(),
                    )
                })?;
            checked.push((device, gate));
        }
        let mut children = Vec::new();
        children.try_reserve_exact(checked.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device child roster allocation failed",
            )
        })?;
        for (device, gate) in checked {
            children.push(KfdRuntimeBackendV1::from_checked_device_with_gate(
                device, gate,
            ));
        }
        Self::from_backends(children)
    }

    // Composition stays private so a caller cannot hide already-live child
    // handles behind newly empty routing tables.
    fn from_backends(children: Vec<KfdRuntimeBackendV1>) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if children.len() < 2 {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device KFD requires at least two child backends",
            ));
        }
        let mut device_children = HashMap::new();
        device_children.try_reserve(children.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device routing-table allocation failed",
            )
        })?;
        for (index, child) in children.iter().enumerate() {
            if device_children
                .insert(child.description.backend_device, index)
                .is_some()
            {
                return Err(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "multi-device child IDs must be distinct",
                ));
            }
        }
        Ok(Self {
            children,
            device_children,
            terminal: false,
            next_handle: 1,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            modules: HashMap::new(),
            kernels: HashMap::new(),
            kernel_modules: HashMap::new(),
            submissions: HashMap::new(),
            events: HashMap::new(),
            cooperative_allocation_owners: HashMap::new(),
            cooperative_dependency_retain_counts: HashMap::new(),
            cooperative_stream_pending_counts: HashMap::new(),
            cooperative_stream_tails: HashMap::new(),
            native_stream_submission_counts: HashMap::new(),
            event_submission_retain_counts: HashMap::new(),
            cooperative_progress_generation: 0,
            cooperative_staging_bytes: 0,
            cooperative_staging_limit_bytes: KFD_RUNTIME_MAX_COOPERATIVE_COPY_STAGING_BYTES_V1,
        })
    }

    /// Explicitly tears down every quiescent child in reverse admission order.
    pub fn shutdown_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.kernel_modules.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || !self.cooperative_allocation_owners.is_empty()
            || !self.cooperative_dependency_retain_counts.is_empty()
            || !self.cooperative_stream_pending_counts.is_empty()
            || !self.cooperative_stream_tails.is_empty()
            || !self.native_stream_submission_counts.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || self.cooperative_staging_bytes != 0
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "multi-device logical runtime resources remain live",
            ));
        }
        for child in self.children.iter_mut().rev() {
            let result = child.shutdown_native_v1();
            if matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                self.terminal = true;
            }
            result?;
        }
        Ok(())
    }

    fn require_live(&self) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.terminal {
            Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "multi-device KFD backend is terminal",
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn latch<T>(
        &mut self,
        result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.terminal = true;
        }
        result
    }

    fn next_id(&mut self) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let id = self.next_handle;
        self.next_handle = self.next_handle.checked_add(1).ok_or_else(|| {
            KfdRuntimeBackendV1::capacity("multi-device routing handle space exhausted")
        })?;
        Ok(id)
    }

    fn require_submission_capacity_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            Err(KfdRuntimeBackendV1::capacity(
                "multi-device submission capacity exceeded",
            ))
        } else {
            Ok(())
        }
    }

    fn reserve_route<T>(
        table: &mut HashMap<u64, T>,
        detail: &'static str,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        table
            .try_reserve(1)
            .map_err(|_| KfdRuntimeBackendV1::capacity(detail))
    }

    fn child_for_device(
        &self,
        device: u64,
    ) -> Result<usize, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.device_children.get(&device).copied().ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "unknown multi-device KFD device",
            )
        })
    }

    fn route(
        table: &HashMap<u64, RoutedHandleV1>,
        handle: u64,
        detail: &'static str,
    ) -> Result<RoutedHandleV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        table.get(&handle).copied().ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(KfdRuntimeBackendErrorKindV1::UnknownHandle, detail)
        })
    }

    fn routed_region_fits(&self, route: RoutedHandleV1, region: BackendMemoryRegionV1) -> bool {
        let Some(end) = region.byte_offset.checked_add(region.byte_len) else {
            return false;
        };
        self.children
            .get(route.child)
            .and_then(|child| child.allocations.get(&route.local))
            .is_some_and(|allocation| end <= allocation.bytes.len() as u64)
    }

    fn stream_has_native_submission_v1(&self, stream: u64) -> bool {
        self.native_stream_submission_counts.contains_key(&stream)
    }

    fn stream_has_pending_cooperative_copy_v1(&self, stream: u64) -> bool {
        self.cooperative_stream_pending_counts.contains_key(&stream)
    }

    fn reserve_native_stream_submission_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self.native_stream_submission_counts.contains_key(&stream) {
            self.native_stream_submission_counts
                .try_reserve(1)
                .map_err(|_| {
                    KfdRuntimeBackendV1::capacity(
                        "multi-device native stream-retain index growth failed",
                    )
                })?;
        }
        if self
            .native_stream_submission_counts
            .get(&stream)
            .is_some_and(|count| *count == usize::MAX)
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "multi-device native stream retain count overflow",
            ));
        }
        Ok(())
    }

    fn retain_native_stream_submission_v1(&mut self, stream: u64) {
        *self
            .native_stream_submission_counts
            .entry(stream)
            .or_insert(0) += 1;
    }

    fn release_native_stream_submission_v1(&mut self, stream: u64) {
        Self::decrement_indexed_count(
            &mut self.native_stream_submission_counts,
            stream,
            "native routed submission remains stream-indexed",
        );
    }

    fn dependency_for_child(
        &mut self,
        event: u64,
        child: usize,
    ) -> Result<Option<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match self.events.get(&event).copied().ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD event",
            )
        })? {
            RoutedEventV1::Native { route, .. } if route.child == child => Ok(Some(route.local)),
            RoutedEventV1::Native { .. } => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "kernel dependency belongs to another KFD device",
            )),
            RoutedEventV1::CooperativeCopy {
                submission,
                child: event_child,
            } if event_child == child => {
                let status = match self.submissions.get(&submission) {
                    Some(RoutedSubmissionV1::CooperativeCopy(copy)) => copy.status(),
                    Some(RoutedSubmissionV1::Native { .. }) | None => {
                        return Err(KfdRuntimeBackendV1::rejected(
                            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                            "copy event does not retain its cooperative submission",
                        ));
                    }
                };
                match status {
                    BackendPollV1::Succeeded => Ok(None),
                    BackendPollV1::Pending => Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "host-staged peer dependency is pending",
                    )),
                    BackendPollV1::Failed { .. } => Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "host-staged peer dependency failed",
                    )),
                }
            }
            RoutedEventV1::CooperativeCopy { .. } => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "copy dependency belongs to another KFD device",
            )),
        }
    }

    fn peer_dependency_submission(
        &self,
        event: u64,
        source_child: usize,
        destination_child: usize,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match self.events.get(&event).copied().ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD event",
            )
        })? {
            RoutedEventV1::Native { route, submission }
                if route.child == source_child || route.child == destination_child =>
            {
                Ok(submission)
            }
            RoutedEventV1::CooperativeCopy { submission, child }
                if child == source_child || child == destination_child =>
            {
                Ok(submission)
            }
            _ => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "peer-copy dependency belongs to an unrelated KFD device",
            )),
        }
    }

    fn allocation_retained_by_cooperative_copy(&self, route: RoutedHandleV1) -> bool {
        self.cooperative_allocation_owners.contains_key(&route)
    }

    fn submission_retained_as_dependency(&self, submission: u64) -> bool {
        self.cooperative_dependency_retain_counts
            .contains_key(&submission)
    }

    fn remove_cooperative_allocation_owner(&mut self, route: RoutedHandleV1, submission: u64) {
        let remove_entry = {
            let owners = self
                .cooperative_allocation_owners
                .get_mut(&route)
                .expect("pending cooperative copy retains indexed allocation custody");
            let index = owners
                .iter()
                .position(|owner| *owner == submission)
                .expect("indexed allocation custody retains the pending submission");
            owners.swap_remove(index);
            owners.is_empty()
        };
        if remove_entry {
            self.cooperative_allocation_owners.remove(&route);
        }
    }

    fn decrement_indexed_count(table: &mut HashMap<u64, usize>, key: u64, detail: &'static str) {
        let remove_entry = {
            let count = table.get_mut(&key).expect(detail);
            *count = count.checked_sub(1).expect(detail);
            *count == 0
        };
        if remove_entry {
            table.remove(&key);
        }
    }

    fn finish_cooperative_copy(
        &mut self,
        submission: u64,
        phase: CooperativeCopyPhaseV1,
    ) -> BackendPollV1 {
        debug_assert!(matches!(
            phase,
            CooperativeCopyPhaseV1::Succeeded
                | CooperativeCopyPhaseV1::Failed
                | CooperativeCopyPhaseV1::Cancelled
        ));
        let (stream, source, destination, dependencies, released_staging_bytes, status) = {
            let RoutedSubmissionV1::CooperativeCopy(copy) = self
                .submissions
                .get_mut(&submission)
                .expect("validated cooperative copy remains retained")
            else {
                unreachable!("validated cooperative copy changed kind")
            };
            debug_assert!(!copy.is_quiescent());
            copy.phase = phase;
            let staging = core::mem::take(&mut copy.staging);
            let released_staging_bytes = u64::try_from(staging.len())
                .expect("cooperative staging length was admitted as u64");
            debug_assert_eq!(released_staging_bytes, copy.source_region.byte_len);
            (
                copy.stream,
                copy.source,
                copy.destination,
                core::mem::take(&mut copy.dependencies),
                released_staging_bytes,
                copy.status(),
            )
        };

        self.cooperative_staging_bytes = self
            .cooperative_staging_bytes
            .checked_sub(released_staging_bytes)
            .expect("pending cooperative staging is accounted exactly");

        self.remove_cooperative_allocation_owner(source, submission);
        if destination != source {
            self.remove_cooperative_allocation_owner(destination, submission);
        }
        for dependency in dependencies {
            Self::decrement_indexed_count(
                &mut self.cooperative_dependency_retain_counts,
                dependency,
                "pending cooperative dependency retain count is indexed",
            );
        }
        Self::decrement_indexed_count(
            &mut self.cooperative_stream_pending_counts,
            stream,
            "pending cooperative stream retain count is indexed",
        );
        self.note_cooperative_progress();
        status
    }

    fn note_cooperative_progress(&mut self) {
        self.cooperative_progress_generation = self.cooperative_progress_generation.wrapping_add(1);
    }

    #[cfg(test)]
    fn assert_cooperative_indexes_consistent(&self) {
        let mut expected_allocation_owners = HashMap::<RoutedHandleV1, Vec<u64>>::new();
        let mut expected_dependency_counts = HashMap::<u64, usize>::new();
        let mut expected_stream_counts = HashMap::<u64, usize>::new();
        let mut expected_native_stream_counts = HashMap::<u64, usize>::new();
        let mut expected_staging_bytes = 0_u64;
        for (submission, record) in &self.submissions {
            let copy = match record {
                RoutedSubmissionV1::Native { stream, .. } => {
                    *expected_native_stream_counts.entry(*stream).or_insert(0) += 1;
                    continue;
                }
                RoutedSubmissionV1::CooperativeCopy(copy) => copy,
            };
            assert!(copy.dependency_depth <= MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1);
            if copy.is_quiescent() {
                assert!(copy.dependencies.is_empty());
                assert!(copy.staging.is_empty());
                continue;
            }
            assert!(copy.dependency_cursor <= copy.dependencies.len());
            assert_eq!(
                u64::try_from(copy.staging.len()).unwrap(),
                copy.source_region.byte_len
            );
            expected_staging_bytes = expected_staging_bytes
                .checked_add(copy.source_region.byte_len)
                .unwrap();
            expected_allocation_owners
                .entry(copy.source)
                .or_default()
                .push(*submission);
            if copy.destination != copy.source {
                expected_allocation_owners
                    .entry(copy.destination)
                    .or_default()
                    .push(*submission);
            }
            for dependency in &copy.dependencies {
                *expected_dependency_counts.entry(*dependency).or_insert(0) += 1;
            }
            *expected_stream_counts.entry(copy.stream).or_insert(0) += 1;
        }
        for owners in expected_allocation_owners.values_mut() {
            owners.sort_unstable();
        }
        let mut actual_allocation_owners = self.cooperative_allocation_owners.clone();
        for owners in actual_allocation_owners.values_mut() {
            owners.sort_unstable();
            assert!(!owners.is_empty());
            assert!(owners.windows(2).all(|pair| pair[0] != pair[1]));
        }
        assert_eq!(actual_allocation_owners, expected_allocation_owners);
        assert_eq!(
            self.cooperative_dependency_retain_counts,
            expected_dependency_counts
        );
        assert_eq!(
            self.cooperative_stream_pending_counts,
            expected_stream_counts
        );
        assert_eq!(
            self.native_stream_submission_counts,
            expected_native_stream_counts
        );

        let mut expected_event_counts = HashMap::<u64, usize>::new();
        for event in self.events.values() {
            let submission = match event {
                RoutedEventV1::Native { submission, .. }
                | RoutedEventV1::CooperativeCopy { submission, .. } => *submission,
            };
            *expected_event_counts.entry(submission).or_insert(0) += 1;
        }
        assert_eq!(self.event_submission_retain_counts, expected_event_counts);
        for (stream, submission) in &self.cooperative_stream_tails {
            assert!(matches!(
                self.submissions.get(submission),
                Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.stream == *stream
            ));
        }
        assert_eq!(self.cooperative_staging_bytes, expected_staging_bytes);
        assert!(self.cooperative_staging_bytes <= self.cooperative_staging_limit_bytes);
    }

    fn oldest_pending_cooperative_dependency(
        &mut self,
        submission: u64,
    ) -> Result<Option<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut current = submission;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&current)
            else {
                return Ok(None);
            };
            if copy.is_quiescent() {
                return Ok(None);
            }
            let predecessor = (copy.phase == CooperativeCopyPhaseV1::Dependencies)
                .then(|| copy.dependencies.get(copy.dependency_cursor).copied())
                .flatten()
                .filter(|dependency| {
                    matches!(
                        self.submissions.get(dependency),
                        Some(RoutedSubmissionV1::CooperativeCopy(prior))
                            if !prior.is_quiescent()
                    )
                });
            let Some(predecessor) = predecessor else {
                return Ok(Some(current));
            };
            debug_assert!(
                predecessor < current,
                "copy dependencies precede submission"
            );
            current = predecessor;
        }
        self.terminal = true;
        Err(RuntimeBackendFailureV1::Terminal(
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "cooperative copy dependency depth exceeded its admitted bound",
            ),
        ))
    }

    fn observe_dependency(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "cooperative copy retained an unknown dependency submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
        };
        match native_route {
            Some(route) => {
                let result = self.children[route.child].poll_v1(route.local);
                self.latch(result)
            }
            None => Ok(match &self.submissions[&submission] {
                RoutedSubmissionV1::CooperativeCopy(copy) => copy.status(),
                RoutedSubmissionV1::Native { .. } => unreachable!(),
            }),
        }
    }

    fn fail_cooperative_copy(&mut self, submission: u64) -> BackendPollV1 {
        self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Failed)
    }

    /// Advances at most one cooperative host-staging transition.
    ///
    /// This is cooperative host progress, not background DMA. Submission is
    /// nonblocking because no child allocation access occurs before this path.
    /// A read/write transition issues one child range request of at most 64 KiB,
    /// but that child may first reconcile allocation-wide native-dirty or copy-
    /// on-write state; this is not a strict host-work or latency bound.
    fn progress_cooperative_copy(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // Dependencies name older submissions. Select the oldest reachable
        // pending copy first, advance exactly that one operation, and return;
        // this keeps fan-in progress bounded without recursive chain growth.
        if let Some(oldest) = self.oldest_pending_cooperative_dependency(submission)?
            && oldest != submission
        {
            self.progress_cooperative_copy(oldest)?;
            return Ok(BackendPollV1::Pending);
        }
        let phase = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown cooperative copy submission",
            )
        })? {
            RoutedSubmissionV1::CooperativeCopy(copy) => copy.phase,
            RoutedSubmissionV1::Native { .. } => {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "native submission routed through cooperative copy progress",
                ));
            }
        };

        match phase {
            CooperativeCopyPhaseV1::Succeeded
            | CooperativeCopyPhaseV1::Failed
            | CooperativeCopyPhaseV1::Cancelled => {
                let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission]
                else {
                    unreachable!()
                };
                Ok(copy.status())
            }
            CooperativeCopyPhaseV1::Dependencies => {
                let dependency = match &self.submissions[&submission] {
                    RoutedSubmissionV1::CooperativeCopy(copy) => {
                        copy.dependencies.get(copy.dependency_cursor).copied()
                    }
                    RoutedSubmissionV1::Native { .. } => unreachable!(),
                };
                if let Some(dependency) = dependency {
                    match self.observe_dependency(dependency) {
                        Ok(BackendPollV1::Succeeded) => {
                            let RoutedSubmissionV1::CooperativeCopy(copy) =
                                self.submissions.get_mut(&submission).unwrap()
                            else {
                                unreachable!()
                            };
                            copy.dependency_cursor += 1;
                            self.note_cooperative_progress();
                            return Ok(BackendPollV1::Pending);
                        }
                        Ok(BackendPollV1::Pending) => return Ok(BackendPollV1::Pending),
                        Ok(BackendPollV1::Failed { .. })
                        | Err(RuntimeBackendFailureV1::Rejected(_))
                        | Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                            return Ok(self.fail_cooperative_copy(submission));
                        }
                        Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                            self.terminal = true;
                            return Err(failure);
                        }
                    }
                }
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    self.submissions.get_mut(&submission).unwrap()
                else {
                    unreachable!()
                };
                copy.phase = CooperativeCopyPhaseV1::Read;
                self.note_cooperative_progress();
                Ok(BackendPollV1::Pending)
            }
            CooperativeCopyPhaseV1::Read => {
                let (route, byte_offset, start, end) = {
                    let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission]
                    else {
                        unreachable!()
                    };
                    let start = copy.byte_cursor;
                    let end = start
                        .saturating_add(COOPERATIVE_COPY_CHUNK_BYTES_V1)
                        .min(copy.staging.len());
                    (
                        copy.source,
                        copy.source_region.byte_offset + start as u64,
                        start,
                        end,
                    )
                };
                let result = {
                    let children = &mut self.children;
                    let submissions = &mut self.submissions;
                    let RoutedSubmissionV1::CooperativeCopy(copy) =
                        submissions.get_mut(&submission).unwrap()
                    else {
                        unreachable!()
                    };
                    children[route.child].read_allocation_v1(
                        route.local,
                        byte_offset,
                        &mut copy.staging[start..end],
                    )
                };
                match result {
                    Ok(()) => {
                        let RoutedSubmissionV1::CooperativeCopy(copy) =
                            self.submissions.get_mut(&submission).unwrap()
                        else {
                            unreachable!()
                        };
                        copy.byte_cursor = end;
                        if end == copy.staging.len() {
                            copy.phase = CooperativeCopyPhaseV1::Write;
                            copy.byte_cursor = 0;
                        }
                        self.note_cooperative_progress();
                        Ok(BackendPollV1::Pending)
                    }
                    Err(RuntimeBackendFailureV1::Rejected(error))
                        if error.kind() == KfdRuntimeBackendErrorKindV1::Busy =>
                    {
                        Ok(BackendPollV1::Pending)
                    }
                    Err(RuntimeBackendFailureV1::Rejected(_))
                    | Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                        Ok(self.fail_cooperative_copy(submission))
                    }
                    Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                        self.terminal = true;
                        Err(failure)
                    }
                }
            }
            CooperativeCopyPhaseV1::Write => {
                let (route, byte_offset, start, end) = {
                    let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission]
                    else {
                        unreachable!()
                    };
                    let start = copy.byte_cursor;
                    let end = start
                        .saturating_add(COOPERATIVE_COPY_CHUNK_BYTES_V1)
                        .min(copy.staging.len());
                    (
                        copy.destination,
                        copy.destination_region.byte_offset + start as u64,
                        start,
                        end,
                    )
                };
                let result = {
                    let children = &mut self.children;
                    let submissions = &self.submissions;
                    let RoutedSubmissionV1::CooperativeCopy(copy) = &submissions[&submission]
                    else {
                        unreachable!()
                    };
                    children[route.child].write_allocation_v1(
                        route.local,
                        byte_offset,
                        &copy.staging[start..end],
                    )
                };
                match result {
                    Ok(()) => {
                        let RoutedSubmissionV1::CooperativeCopy(copy) =
                            self.submissions.get_mut(&submission).unwrap()
                        else {
                            unreachable!()
                        };
                        copy.byte_cursor = end;
                        if end == copy.staging.len() {
                            return Ok(self.finish_cooperative_copy(
                                submission,
                                CooperativeCopyPhaseV1::Succeeded,
                            ));
                        }
                        let status = copy.status();
                        self.note_cooperative_progress();
                        Ok(status)
                    }
                    Err(RuntimeBackendFailureV1::Rejected(error))
                        if error.kind() == KfdRuntimeBackendErrorKindV1::Busy =>
                    {
                        Ok(BackendPollV1::Pending)
                    }
                    Err(RuntimeBackendFailureV1::Rejected(_))
                    | Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                        Ok(self.fail_cooperative_copy(submission))
                    }
                    Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                        self.terminal = true;
                        Err(failure)
                    }
                }
            }
        }
    }

    fn submit_cooperative_copy(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
        require_distinct_devices: bool,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_submission_capacity_v1()?;
        let stream_route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        let source_route = Self::route(
            &self.allocations,
            source.allocation,
            "unknown source KFD allocation",
        )?;
        let destination_route = Self::route(
            &self.allocations,
            destination.allocation,
            "unknown destination KFD allocation",
        )?;
        let distinct_devices = source_route.child != destination_route.child;
        if distinct_devices != require_distinct_devices
            || destination_route.child != stream_route.child
            || source.byte_len != destination.byte_len
            || source.byte_len == 0
            || source.byte_offset.checked_add(source.byte_len).is_none()
            || destination
                .byte_offset
                .checked_add(destination.byte_len)
                .is_none()
            || !matches!(
                source.access,
                RuntimeAccessV1::Read | RuntimeAccessV1::ReadWrite
            )
            || !matches!(
                destination.access,
                RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
            )
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "cooperative copy requires equal nonzero ranges, valid access, and a destination stream",
            ));
        }
        if self.stream_has_native_submission_v1(stream) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "mixed native/cooperative stream ordering requires releasing prior native work",
            ));
        }
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "cooperative copy dependency capacity exceeded",
            ));
        }
        if !self.routed_region_fits(source_route, source)
            || !self.routed_region_fits(destination_route, destination)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "cooperative copy range exceeds its routed allocation",
            ));
        }
        if self.children[source_route.child].allocation_is_active(source_route.local)
            || self.children[destination_route.child].allocation_is_active(destination_route.local)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cooperative copy allocation is retained by an active native dispatch",
            ));
        }
        let len = usize::try_from(source.byte_len)
            .map_err(|_| KfdRuntimeBackendV1::capacity("copy staging size overflow"))?;
        let stream_tail = self.cooperative_stream_tails.get(&stream).copied();
        let mut dependency_submissions = Vec::new();
        dependency_submissions
            .try_reserve_exact(
                dependencies
                    .len()
                    .saturating_add(usize::from(stream_tail.is_some())),
            )
            .map_err(|_| KfdRuntimeBackendV1::capacity("copy dependency allocation failed"))?;
        let mut dependency_set = HashSet::new();
        dependency_set
            .try_reserve(
                dependencies
                    .len()
                    .saturating_add(usize::from(stream_tail.is_some())),
            )
            .map_err(|_| KfdRuntimeBackendV1::capacity("copy dependency set allocation failed"))?;
        for event in dependencies {
            let dependency = self.peer_dependency_submission(
                *event,
                source_route.child,
                destination_route.child,
            )?;
            if !dependency_set.insert(dependency) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "cooperative copy dependencies must name distinct submissions",
                ));
            }
            dependency_submissions.push(dependency);
        }
        if let Some(tail) = stream_tail
            && dependency_set.insert(tail)
        {
            if dependency_submissions.len() == MAX_RUNTIME_DEPENDENCIES_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "cooperative copy dependency capacity exceeded by stream ordering",
                ));
            }
            if matches!(
                self.submissions.get(&tail),
                Some(RoutedSubmissionV1::CooperativeCopy(copy))
                    if matches!(copy.status(), BackendPollV1::Failed { .. })
            ) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "prior cooperative work in the stream completed with failure",
                ));
            }
            dependency_submissions.push(tail);
        }
        let mut dependency_depth = 1_usize;
        for dependency in &dependency_submissions {
            if let Some(RoutedSubmissionV1::CooperativeCopy(copy)) =
                self.submissions.get(dependency)
                && !copy.is_quiescent()
            {
                dependency_depth = dependency_depth.max(
                    copy.dependency_depth.checked_add(1).ok_or_else(|| {
                        KfdRuntimeBackendV1::capacity("cooperative copy dependency depth overflow")
                    })?,
                );
            }
        }
        if dependency_depth > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "cooperative copy dependency depth exceeds its admitted bound",
            ));
        }
        let source_dependencies_complete = self
            .cooperative_allocation_owners
            .get(&source_route)
            .is_none_or(|owners| {
                owners.iter().all(|owner| {
                    dependency_set.contains(owner)
                        || matches!(
                            self.submissions.get(owner),
                            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                                if copy.stream == stream
                        )
                })
            });
        let destination_dependencies_complete = self
            .cooperative_allocation_owners
            .get(&destination_route)
            .is_none_or(|owners| {
                owners.iter().all(|owner| {
                    dependency_set.contains(owner)
                        || matches!(
                            self.submissions.get(owner),
                            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                                if copy.stream == stream
                        )
                })
            });
        if !source_dependencies_complete || !destination_dependencies_complete {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping cooperative copies require an explicit dependency",
            ));
        }

        let next_cooperative_staging_bytes = self
            .cooperative_staging_bytes
            .checked_add(source.byte_len)
            .filter(|total| *total <= self.cooperative_staging_limit_bytes)
            .ok_or_else(|| {
                KfdRuntimeBackendV1::capacity(
                    "cooperative copy aggregate staging capacity exceeded",
                )
            })?;

        let distinct_allocation_routes = source_route != destination_route;
        let missing_allocation_owner_entries = usize::from(
            !self
                .cooperative_allocation_owners
                .contains_key(&source_route),
        ) + usize::from(
            distinct_allocation_routes
                && !self
                    .cooperative_allocation_owners
                    .contains_key(&destination_route),
        );
        self.cooperative_allocation_owners
            .try_reserve(missing_allocation_owner_entries)
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity(
                    "cooperative copy allocation-custody index growth failed",
                )
            })?;
        let mut new_source_owners = None;
        if let Some(owners) = self.cooperative_allocation_owners.get_mut(&source_route) {
            owners.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("cooperative source allocation owner growth failed")
            })?;
        } else {
            let mut owners = Vec::new();
            owners.try_reserve_exact(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity(
                    "cooperative source allocation owner allocation failed",
                )
            })?;
            new_source_owners = Some(owners);
        }
        let mut new_destination_owners = None;
        if distinct_allocation_routes {
            if let Some(owners) = self
                .cooperative_allocation_owners
                .get_mut(&destination_route)
            {
                owners.try_reserve(1).map_err(|_| {
                    KfdRuntimeBackendV1::capacity(
                        "cooperative destination allocation owner growth failed",
                    )
                })?;
            } else {
                let mut owners = Vec::new();
                owners.try_reserve_exact(1).map_err(|_| {
                    KfdRuntimeBackendV1::capacity(
                        "cooperative destination allocation owner allocation failed",
                    )
                })?;
                new_destination_owners = Some(owners);
            }
        }
        let new_dependency_count_entries = dependency_submissions
            .iter()
            .filter(|dependency| {
                !self
                    .cooperative_dependency_retain_counts
                    .contains_key(dependency)
            })
            .count();
        self.cooperative_dependency_retain_counts
            .try_reserve(new_dependency_count_entries)
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("cooperative dependency-retain index growth failed")
            })?;
        if dependency_submissions.iter().any(|dependency| {
            self.cooperative_dependency_retain_counts
                .get(dependency)
                .is_some_and(|count| *count == usize::MAX)
        }) {
            return Err(KfdRuntimeBackendV1::capacity(
                "cooperative dependency retain count overflow",
            ));
        }
        if !self.cooperative_stream_pending_counts.contains_key(&stream) {
            self.cooperative_stream_pending_counts
                .try_reserve(1)
                .map_err(|_| {
                    KfdRuntimeBackendV1::capacity("cooperative stream-retain index growth failed")
                })?;
        }
        if !self.cooperative_stream_tails.contains_key(&stream) {
            self.cooperative_stream_tails.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("cooperative stream-tail index growth failed")
            })?;
        }
        if self
            .cooperative_stream_pending_counts
            .get(&stream)
            .is_some_and(|count| *count == usize::MAX)
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "cooperative stream retain count overflow",
            ));
        }
        Self::reserve_route(
            &mut self.submissions,
            "multi-device copy submission route allocation failed",
        )?;
        let staging = try_zeroed_staging_v1(len)?;
        let id = self.next_id()?;

        if let Some(owners) = self.cooperative_allocation_owners.get_mut(&source_route) {
            owners.push(id);
        } else {
            let mut owners = new_source_owners
                .take()
                .expect("new cooperative source owner storage was reserved");
            owners.push(id);
            self.cooperative_allocation_owners
                .insert(source_route, owners);
        }
        if distinct_allocation_routes {
            if let Some(owners) = self
                .cooperative_allocation_owners
                .get_mut(&destination_route)
            {
                owners.push(id);
            } else {
                let mut owners = new_destination_owners
                    .take()
                    .expect("new cooperative destination owner storage was reserved");
                owners.push(id);
                self.cooperative_allocation_owners
                    .insert(destination_route, owners);
            }
        }
        for dependency in &dependency_submissions {
            let count = self
                .cooperative_dependency_retain_counts
                .entry(*dependency)
                .or_insert(0);
            *count += 1;
        }
        let stream_count = self
            .cooperative_stream_pending_counts
            .entry(stream)
            .or_insert(0);
        *stream_count += 1;
        self.cooperative_stream_tails.insert(stream, id);
        self.cooperative_staging_bytes = next_cooperative_staging_bytes;
        self.submissions.insert(
            id,
            RoutedSubmissionV1::CooperativeCopy(CooperativeCopySubmissionV1 {
                stream,
                prior_stream_submission: stream_tail,
                source: source_route,
                source_region: source,
                destination: destination_route,
                destination_region: destination,
                dependencies: dependency_submissions,
                dependency_cursor: 0,
                dependency_depth,
                staging,
                phase: CooperativeCopyPhaseV1::Dependencies,
                byte_cursor: 0,
            }),
        );
        Ok(id)
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    /// Opens and admits two exact gfx942 devices before acquiring either VM.
    pub fn open_default(
        first_unique_id: u64,
        second_unique_id: u64,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if admit_xgmi_unique_id_pair_v1(first_unique_id, second_unique_id).is_err() {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI requires two distinct nonzero unique IDs",
            ));
        }
        let bind = |unique_id| {
            OpenedKfd::open_default()
                .map_err(|error| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Native,
                        error.to_string(),
                    )
                })?
                .admit_uapi()
                .map_err(|error| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Native,
                        error.to_string(),
                    )
                })?
                .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))
                .map_err(|error| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Native,
                        error.to_string(),
                    )
                })
        };
        let first = bind(first_unique_id)?;
        let second = bind(second_unique_id)?;
        Self::from_checked_pair(first, second)
    }

    /// Builds the copy-only owner from two already-admitted devices.
    ///
    /// Once the first process VM is acquired, failure to acquire the second is
    /// fail-stop because the low-level session has no inverse transition that
    /// can return the first consumed device authority.
    pub fn from_checked_pair(
        first: CheckedGfx942XnackMinusDevice,
        second: CheckedGfx942XnackMinusDevice,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let first_observation = first.observation();
        let second_observation = second.observation();
        let first_unique_id = first_observation.unique_id();
        let second_unique_id = second_observation.unique_id();
        if admit_xgmi_unique_id_pair_v1(first_unique_id, second_unique_id).is_err() {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI checked devices must have distinct nonzero unique IDs",
            ));
        }
        let first_gpu_id = first_observation.kfd_gpu_id();
        let second_gpu_id = second_observation.kfd_gpu_id();
        let forward = first
            .topology_snapshot()
            .topology()
            .admit_gfx942_xgmi_route(first_gpu_id, second_gpu_id)
            .map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    format!("forward XGMI route admission: {error}"),
                )
            })?;
        let reverse = second
            .topology_snapshot()
            .topology()
            .admit_gfx942_xgmi_route(second_gpu_id, first_gpu_id)
            .map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    format!("reverse XGMI route admission: {error}"),
                )
            })?;
        let name = |device: &CheckedGfx942XnackMinusDevice, unique_id| {
            device
                .topology_snapshot()
                .topology()
                .gpu_nodes()
                .iter()
                .find(|node| node.unique_id() == unique_id)
                .map_or_else(|| "AMD MI300X".to_owned(), |node| node.name().to_owned())
        };
        let capabilities = RuntimeCapabilitiesV1 {
            streams: true,
            events: true,
            device_memory: true,
            peer_copy: true,
            multi_device: true,
            ..RuntimeCapabilitiesV1::default()
        };
        let descriptions = [
            BackendDeviceDescriptionV1 {
                backend_device: first_unique_id,
                name: name(&first, first_unique_id),
                target: "gfx942:xnack-".to_owned(),
                global_memory_bytes: 0,
                capabilities,
            },
            BackendDeviceDescriptionV1 {
                backend_device: second_unique_id,
                name: name(&second, second_unique_id),
                target: "gfx942:xnack-".to_owned(),
                global_memory_bytes: 0,
                capabilities,
            },
        ];
        let first = first.acquire_shared_gtt_memory_session().map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Native,
                format!("first XGMI VM acquisition: {error}"),
            )
        })?;
        let second = match second.acquire_shared_gtt_memory_session() {
            Ok(session) => session,
            Err(_) => {
                // Acquiring the first process VM consumed its checked device,
                // and this profile has no inverse transition that can return
                // that authority. Returning would abandon native custody
                // through an inert Drop, so this post-mutation failure stops.
                std::process::abort();
            }
        };
        Ok(Self {
            descriptions,
            sessions: [first, second],
            routes: [forward, reverse],
            queues: [None, None],
            terminal: false,
            shutdown: false,
            next_handle: 1,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            submissions: HashMap::new(),
            active: HashMap::new(),
            active_stream_owners: HashMap::new(),
            active_allocation_owners: HashMap::new(),
            ready_by_direction: [VecDeque::new(), VecDeque::new()],
            in_flight_by_direction: [Vec::new(), Vec::new()],
            active_by_direction: [0, 0],
            completion_reservations: 0,
            events: HashMap::new(),
            event_submission_retain_counts: HashMap::new(),
            dependency_retain_counts: HashMap::new(),
            dependency_depths: HashMap::new(),
            dependency_waiters: HashMap::new(),
        })
    }

    fn rejected(
        kind: KfdRuntimeBackendErrorKindV1,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        KfdRuntimeBackendV1::rejected(kind, detail)
    }

    fn terminal_error(
        &mut self,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        self.terminal = true;
        RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Terminal,
            detail,
        ))
    }

    fn quiescent_error(
        kind: KfdRuntimeBackendErrorKindV1,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        RuntimeBackendFailureV1::Quiescent(KfdRuntimeBackendErrorV1::new(kind, detail))
    }

    fn require_live(&self) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.terminal {
            return Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "native XGMI backend is terminal",
                ),
            ));
        }
        if self.shutdown {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native XGMI backend is shut down",
            ));
        }
        Ok(())
    }

    fn next_id(&mut self) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let id = self.next_handle;
        self.next_handle = id.checked_add(1).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI handle space exhausted",
            )
        })?;
        Ok(id)
    }

    fn reserve_event_submission_retain(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self
            .event_submission_retain_counts
            .contains_key(&submission)
        {
            self.event_submission_retain_counts
                .try_reserve(1)
                .map_err(|_| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "XGMI event-retain index",
                    )
                })?;
        }
        if self
            .event_submission_retain_counts
            .get(&submission)
            .is_some_and(|count| *count == usize::MAX)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI event retain count overflow",
            ));
        }
        Ok(())
    }

    fn retain_event_submission(&mut self, submission: u64) {
        *self
            .event_submission_retain_counts
            .entry(submission)
            .or_insert(0) += 1;
    }

    fn release_event_submission(&mut self, submission: u64) {
        let remove = {
            let count = self
                .event_submission_retain_counts
                .get_mut(&submission)
                .expect("live XGMI event retains its submission index");
            *count = count
                .checked_sub(1)
                .expect("live XGMI event retain count is positive");
            *count == 0
        };
        if remove {
            self.event_submission_retain_counts.remove(&submission);
        }
    }

    fn device_index(&self, device: u64) -> Option<usize> {
        self.descriptions
            .iter()
            .position(|description| description.backend_device == device)
    }

    fn session_pair(
        sessions: &mut [SharedGttMemorySessionV1; 2],
        direction: usize,
    ) -> (&mut SharedGttMemorySessionV1, &mut SharedGttMemorySessionV1) {
        let (first, second) = sessions.split_at_mut(1);
        if direction == 0 {
            (&mut first[0], &mut second[0])
        } else {
            (&mut second[0], &mut first[0])
        }
    }

    fn ensure_queue(
        &mut self,
        direction: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.queues[direction].is_some() {
            return Ok(());
        }
        let route = self.routes[direction];
        let result = {
            let (source, destination) = Self::session_pair(&mut self.sessions, direction);
            Gfx942NativeXgmiSdmaQueueV1::create(source, destination, route)
        };
        self.queues[direction] = Some(
            result.map_err(|error| self.terminal_error(format!("XGMI queue creation: {error}")))?,
        );
        Ok(())
    }

    fn restore_unmapped(
        &mut self,
        allocation: u64,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(record) = self.allocations.get_mut(&allocation) else {
            return Err(self.terminal_error("XGMI allocation disappeared"));
        };
        if record.authority.is_some() {
            // Both the existing authority and `lease` are move-only native
            // custody. There is no second logical slot in which to return the
            // latter, so an impossible double restoration must fail-stop
            // before either value is dropped.
            std::process::abort();
        }
        record.authority = Some(XgmiAllocationAuthorityV1::Unmapped(lease));
        Ok(())
    }

    fn restore_mapped(
        &mut self,
        allocation: u64,
        mapping: Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !mapping.is_fully_mapped() {
            self.quarantine_mapping(allocation, mapping);
            return Err(self.terminal_error("incomplete XGMI mapping cannot become reusable"));
        }
        let Some(record) = self.allocations.get_mut(&allocation) else {
            return Err(self.terminal_error("XGMI allocation disappeared"));
        };
        if record.authority.is_some() {
            // There is no safe place to return a second linear native owner.
            std::process::abort();
        }
        record.authority = Some(XgmiAllocationAuthorityV1::Mapped(mapping));
        Ok(())
    }

    fn map_allocation(
        &mut self,
        allocation: u64,
        direction: usize,
    ) -> Result<Gfx942XgmiMappedDeviceMemoryV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let (owner, authority) = {
            let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown native XGMI allocation",
                )
            })?;
            let authority = record.authority.take().ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "native XGMI allocation is retained by pending work",
                )
            })?;
            (record.device, authority)
        };
        let lease = match authority {
            XgmiAllocationAuthorityV1::Mapped(mapping) => return Ok(mapping),
            XgmiAllocationAuthorityV1::Unmapped(lease) => lease,
            authority @ XgmiAllocationAuthorityV1::QuarantinedMapped(_) => {
                self.allocations
                    .get_mut(&allocation)
                    .expect("indexed XGMI allocation")
                    .authority = Some(authority);
                return Err(self.terminal_error("quarantined XGMI mapping was reused"));
            }
        };
        let route = self.routes[direction];
        let result = {
            let (first, second) = self.sessions.split_at_mut(1);
            if owner == 0 {
                first[0].map_gfx942_device_memory_for_xgmi_peer(&mut second[0], route, lease)
            } else {
                second[0].map_gfx942_device_memory_for_xgmi_peer(&mut first[0], route, lease)
            }
        };
        match result {
            Ok(mapping) => Ok(mapping),
            Err(failure) => {
                let (error, recovery) = failure.into_parts();
                match recovery {
                    Gfx942XgmiMapRecoveryV1::Unmapped(lease) => {
                        self.restore_unmapped(allocation, lease)?;
                        Err(Self::rejected(
                            KfdRuntimeBackendErrorKindV1::Native,
                            format!("XGMI map rejected: {error}"),
                        ))
                    }
                    Gfx942XgmiMapRecoveryV1::PartiallyMapped(mapping) => {
                        self.allocations
                            .get_mut(&allocation)
                            .expect("mapped allocation remains indexed")
                            .authority =
                            Some(XgmiAllocationAuthorityV1::QuarantinedMapped(mapping));
                        Err(self.terminal_error(format!("XGMI map became ambiguous: {error}")))
                    }
                }
            }
        }
    }

    fn unmap_allocation(
        &mut self,
        allocation: u64,
        direction: usize,
        mapping: Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let owner = self.allocations[&allocation].device;
        let route = self.routes[direction];
        let result = {
            let (first, second) = self.sessions.split_at_mut(1);
            if owner == 0 {
                first[0].unmap_gfx942_device_memory_from_xgmi_peer(&mut second[0], route, mapping)
            } else {
                second[0].unmap_gfx942_device_memory_from_xgmi_peer(&mut first[0], route, mapping)
            }
        };
        match result {
            Ok(lease) => self.restore_unmapped(allocation, lease),
            Err(failure) => {
                let (error, recovery) = failure.into_parts();
                match recovery {
                    Gfx942XgmiUnmapRecoveryV1::Unmapped(lease) => {
                        self.restore_unmapped(allocation, lease)?;
                    }
                    Gfx942XgmiUnmapRecoveryV1::PartiallyUnmapped(mapping) => {
                        self.allocations
                            .get_mut(&allocation)
                            .expect("mapped allocation remains indexed")
                            .authority =
                            Some(XgmiAllocationAuthorityV1::QuarantinedMapped(mapping));
                    }
                }
                Err(self.terminal_error(format!("XGMI unmap became ambiguous: {error}")))
            }
        }
    }

    fn ensure_allocation_unmapped(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let authority = self
            .allocations
            .get_mut(&allocation)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown native XGMI allocation",
                )
            })?
            .authority
            .take()
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "native XGMI allocation is retained by pending work",
                )
            })?;
        match authority {
            XgmiAllocationAuthorityV1::Unmapped(lease) => self.restore_unmapped(allocation, lease),
            XgmiAllocationAuthorityV1::Mapped(mapping) => {
                self.unmap_allocation(allocation, 0, mapping)
            }
            authority @ XgmiAllocationAuthorityV1::QuarantinedMapped(_) => {
                self.allocations
                    .get_mut(&allocation)
                    .expect("indexed XGMI allocation")
                    .authority = Some(authority);
                Err(self.terminal_error("quarantined XGMI mapping cannot be unmapped normally"))
            }
        }
    }

    fn restore_mapped_copy_pair(
        &mut self,
        source_allocation: u64,
        destination_allocation: u64,
        direction: usize,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let _ = direction;
        if let Err(failure) = self.restore_mapped(source_allocation, source) {
            self.quarantine_mapping(destination_allocation, destination);
            return Err(failure);
        }
        self.restore_mapped(destination_allocation, destination)
    }

    fn reserve_directional_index_slot(
        &mut self,
        direction: usize,
    ) -> Result<usize, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let next = self.active_by_direction[direction]
            .checked_add(1)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI directional active count",
                )
            })?;
        let ready = &mut self.ready_by_direction[direction];
        if ready.capacity() < next {
            ready.try_reserve_exact(next - ready.len()).map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI directional ready queue",
                )
            })?;
        }
        let in_flight = &mut self.in_flight_by_direction[direction];
        let in_flight_needed = next.min(GFX942_SDMA_MAX_IN_FLIGHT_V1);
        if in_flight.capacity() < in_flight_needed {
            in_flight
                .try_reserve_exact(in_flight_needed - in_flight.len())
                .map_err(|_| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "native XGMI directional in-flight index",
                    )
                })?;
        }
        Ok(next)
    }

    fn remove_directional_indexes(&mut self, active: &XgmiRuntimeSubmissionV1) {
        let _ = remove_xgmi_progress_index_v1(
            &mut self.ready_by_direction[active.direction],
            &mut self.in_flight_by_direction[active.direction],
            active.id,
        );
        if self.active_by_direction[active.direction] == 0 {
            std::process::abort();
        }
        self.active_by_direction[active.direction] -= 1;
    }

    fn wake_dependency_waiters(&mut self, dependency: u64) {
        let Some(waiters) = self.dependency_waiters.remove(&dependency) else {
            return;
        };
        for waiter in waiters {
            let Some(active) = self.active.get(&waiter) else {
                continue;
            };
            if xgmi_submission_is_ready_v1(active, &self.submissions, active.direction) {
                enqueue_xgmi_ready_id_v1(&mut self.ready_by_direction[active.direction], active.id);
            }
        }
    }

    fn unregister_dependency_waiter(&mut self, active: &XgmiRuntimeSubmissionV1) {
        for dependency in &active.dependencies {
            let remove_entry = self
                .dependency_waiters
                .get_mut(dependency)
                .is_some_and(|waiters| {
                    let _ = remove_ordered_xgmi_id_v1(waiters, active.id);
                    waiters.is_empty()
                });
            if remove_entry {
                self.dependency_waiters.remove(dependency);
            }
        }
    }

    fn settle_submission(
        &mut self,
        active: XgmiRuntimeSubmissionV1,
        status: BackendPollV1,
    ) -> BackendPollV1 {
        let id = active.id;
        self.unregister_dependency_waiter(&active);
        self.remove_directional_indexes(&active);
        self.unregister_active_xgmi_ownership_v1(&active);
        settle_xgmi_submission_record_v1(
            &mut self.dependency_retain_counts,
            &mut self.submissions,
            &mut self.completion_reservations,
            active,
            status,
        );
        self.wake_dependency_waiters(id);
        status
    }

    fn finish_failed(&mut self, active: XgmiRuntimeSubmissionV1) -> BackendPollV1 {
        let status = BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
        };
        self.settle_submission(active, status)
    }

    fn allocation_active(&self, allocation: u64) -> bool {
        self.active_allocation_owners.contains_key(&allocation)
    }

    fn unregister_active_xgmi_ownership_v1(&mut self, active: &XgmiRuntimeSubmissionV1) {
        if self.active_stream_owners.remove(&active.stream) != Some(active.id) {
            std::process::abort();
        }
        for allocation in core::iter::once(active.source)
            .chain((active.destination != active.source).then_some(active.destination))
        {
            let remove_entry = {
                let owners = self
                    .active_allocation_owners
                    .get_mut(&allocation)
                    .expect("active XGMI allocation ownership remains indexed");
                let index = owners
                    .iter()
                    .position(|owner| *owner == active.id)
                    .expect("active XGMI allocation owner remains indexed");
                owners.swap_remove(index);
                owners.is_empty()
            };
            if remove_entry {
                self.active_allocation_owners.remove(&allocation);
            }
        }
    }

    fn quarantine_mapping(&mut self, allocation: u64, mapping: Gfx942XgmiMappedDeviceMemoryV1) {
        self.allocations
            .get_mut(&allocation)
            .expect("XGMI allocation remains indexed")
            .authority = Some(XgmiAllocationAuthorityV1::QuarantinedMapped(mapping));
    }

    fn restore_prepared_xgmi_batch(
        &mut self,
        active_batch: Vec<XgmiRuntimeSubmissionV1>,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if active_batch.len() != requests.len() {
            std::process::abort();
        }
        if active_batch.len() > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            std::process::abort();
        }
        let direction = active_batch.first().map(|active| active.direction);
        if direction.is_some_and(|direction| {
            active_batch
                .iter()
                .any(|active| active.direction != direction)
        }) {
            std::process::abort();
        }
        for (active, request) in active_batch.into_iter().zip(requests).rev() {
            let (source, destination) = request.into_mappings();
            self.restore_mapped_copy_pair(
                active.source,
                active.destination,
                active.direction,
                source,
                destination,
            )?;
            prepend_xgmi_ready_id_v1(&mut self.ready_by_direction[active.direction], active.id);
            self.active.insert(active.id, active);
        }
        Ok(())
    }

    /// Publishes every currently ready submission for one direction in a
    /// single native SDMA reservation and doorbell store. The maintained
    /// FIFO ready queue makes selection O(batch) and independent of total
    /// active work; the ordered in-flight index remains bounded to 63 tickets.
    fn publish_ready_peer_batch(
        &mut self,
        direction: usize,
    ) -> Result<XgmiBatchPublicationOutcomeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        if !self.in_flight_by_direction[direction].is_empty() {
            return Ok(XgmiBatchPublicationOutcomeV1::AlreadyInFlight);
        }
        let batch_len = self.ready_by_direction[direction]
            .len()
            .min(GFX942_SDMA_MAX_IN_FLIGHT_V1);
        if batch_len == 0 {
            return Ok(XgmiBatchPublicationOutcomeV1::NoReadyWork);
        }
        let mut active_batch = Vec::new();
        let mut requests = Vec::new();
        active_batch.try_reserve_exact(batch_len).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI active batch",
            )
        })?;
        requests.try_reserve_exact(batch_len).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI request batch",
            )
        })?;
        self.ensure_queue(direction)?;
        for _ in 0..batch_len {
            let id = self.ready_by_direction[direction]
                .pop_front()
                .expect("non-empty XGMI ready queue");
            let active = self
                .active
                .remove(&id)
                .expect("selected XGMI submission remains active");
            let source = match self.map_allocation(active.source, direction) {
                Ok(mapping) => mapping,
                Err(failure) => {
                    // Every earlier pair is restored before the current item is
                    // settled, so a recoverable map rejection cannot strand or
                    // drop any allocation authority selected for this batch.
                    self.restore_prepared_xgmi_batch(active_batch, requests)?;
                    return match failure {
                        RuntimeBackendFailureV1::Rejected(_)
                        | RuntimeBackendFailureV1::Quiescent(_) => {
                            self.finish_failed(active);
                            Ok(XgmiBatchPublicationOutcomeV1::RecoveredPrepublicationFailure)
                        }
                        failure @ RuntimeBackendFailureV1::Terminal(_) => {
                            enqueue_xgmi_ready_id_v1(
                                &mut self.ready_by_direction[active.direction],
                                active.id,
                            );
                            self.active.insert(active.id, active);
                            Err(failure)
                        }
                    };
                }
            };
            let destination = match self.map_allocation(active.destination, direction) {
                Ok(mapping) => mapping,
                Err(failure) => {
                    // Restore the current source and all earlier pairs before
                    // resolving the current logical submission.
                    self.restore_mapped(active.source, source)?;
                    self.restore_prepared_xgmi_batch(active_batch, requests)?;
                    return match failure {
                        RuntimeBackendFailureV1::Rejected(_)
                        | RuntimeBackendFailureV1::Quiescent(_) => {
                            self.finish_failed(active);
                            Ok(XgmiBatchPublicationOutcomeV1::RecoveredPrepublicationFailure)
                        }
                        failure @ RuntimeBackendFailureV1::Terminal(_) => {
                            enqueue_xgmi_ready_id_v1(
                                &mut self.ready_by_direction[active.direction],
                                active.id,
                            );
                            self.active.insert(active.id, active);
                            Err(failure)
                        }
                    };
                }
            };
            let request = Gfx942XgmiSdmaCopyRequestV1::new(
                source,
                active.source_offset,
                destination,
                active.destination_offset,
                active.byte_len,
            );
            active_batch.push(active);
            requests.push(request);
        }
        let result = {
            let (source_session, destination_session) =
                Self::session_pair(&mut self.sessions, direction);
            self.queues[direction]
                .as_mut()
                .expect("directional XGMI queue was established")
                .submit_batch(source_session, destination_session, requests)
        };
        match result {
            Ok(tickets) => {
                if tickets.len() != active_batch.len() {
                    // Native publication retained mappings, but correspondence
                    // to logical submissions is no longer recoverable.
                    std::process::abort();
                }
                for (mut active, ticket) in active_batch.into_iter().zip(tickets) {
                    active.ticket = Some(ticket);
                    insert_ordered_xgmi_id_v1(
                        &mut self.in_flight_by_direction[active.direction],
                        active.id,
                    );
                    self.active.insert(active.id, active);
                }
                Ok(XgmiBatchPublicationOutcomeV1::Published)
            }
            Err(Gfx942XgmiBatchSubmissionFailureV1::Recoverable { error: _, requests }) => {
                if requests.len() != active_batch.len() {
                    std::process::abort();
                }
                for (active, request) in active_batch.into_iter().zip(requests) {
                    let (source, destination) = request.into_mappings();
                    self.restore_mapped_copy_pair(
                        active.source,
                        active.destination,
                        active.direction,
                        source,
                        destination,
                    )?;
                    self.finish_failed(active);
                }
                Ok(XgmiBatchPublicationOutcomeV1::RecoveredPrepublicationFailure)
            }
            Err(Gfx942XgmiBatchSubmissionFailureV1::Retained { error, tickets }) => {
                if tickets.len() != active_batch.len() {
                    std::process::abort();
                }
                for (mut active, ticket) in active_batch.into_iter().zip(tickets) {
                    active.ticket = Some(ticket);
                    insert_ordered_xgmi_id_v1(
                        &mut self.in_flight_by_direction[active.direction],
                        active.id,
                    );
                    self.active.insert(active.id, active);
                }
                Err(self.terminal_error(format!(
                    "native XGMI batch publication retained tickets: {error}"
                )))
            }
        }
    }

    fn progress_peer_copy(
        &mut self,
        mut active: XgmiRuntimeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if let Some(ticket) = active.ticket.take() {
            let result = {
                let (source_session, destination_session) =
                    Self::session_pair(&mut self.sessions, active.direction);
                self.queues[active.direction]
                    .as_mut()
                    .expect("published XGMI copy retains queue")
                    .poll(source_session, destination_session, ticket)
            };
            return match result {
                Ok(Gfx942XgmiCopyPollV1::Pending(ticket)) => {
                    active.ticket = Some(ticket);
                    self.active.insert(active.id, active);
                    Ok(BackendPollV1::Pending)
                }
                Ok(Gfx942XgmiCopyPollV1::Completed(completed)) => {
                    let (source, destination) = completed.into_mappings();
                    self.restore_mapped_copy_pair(
                        active.source,
                        active.destination,
                        active.direction,
                        source,
                        destination,
                    )?;
                    let status = BackendPollV1::Succeeded;
                    Ok(self.settle_submission(active, status))
                }
                Err(Gfx942XgmiCopyFailureV1::Retained { error, ticket }) => {
                    active.ticket = Some(ticket);
                    self.active.insert(active.id, active);
                    Err(self
                        .terminal_error(format!("native XGMI completion retained ticket: {error}")))
                }
                Err(Gfx942XgmiCopyFailureV1::CompletedCurrentnessIndeterminate {
                    error,
                    completed,
                }) => {
                    let (source, destination) = completed.into_mappings();
                    self.quarantine_mapping(active.source, source);
                    self.quarantine_mapping(active.destination, destination);
                    Err(self.terminal_error(format!(
                        "native XGMI completion currentness became ambiguous: {error}"
                    )))
                }
                Err(Gfx942XgmiCopyFailureV1::Recoverable {
                    error,
                    source,
                    destination,
                }) => {
                    self.quarantine_mapping(active.source, source);
                    self.quarantine_mapping(active.destination, destination);
                    Err(self.terminal_error(format!(
                        "native XGMI poll returned unexpected recovered mappings: {error}"
                    )))
                }
            };
        }
        while let Some(dependency) = active.dependencies.get(active.dependency_cursor).copied() {
            match self.poll_v1(dependency)? {
                BackendPollV1::Succeeded => active.dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.active.insert(active.id, active);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => return Ok(self.finish_failed(active)),
            }
        }
        let id = active.id;
        let direction = active.direction;
        enqueue_xgmi_ready_id_v1(&mut self.ready_by_direction[direction], id);
        self.active.insert(id, active);
        let _ = self.publish_ready_peer_batch(direction)?;
        if let Some(record) = self.submissions.get(&id) {
            return Ok(record.status);
        }
        let progress_id = indexed_xgmi_progress_id_v1(&self.in_flight_by_direction[direction], id);
        if let Some(progress_id) = progress_id {
            let published = self
                .active
                .remove(&progress_id)
                .expect("selected published XGMI submission remains active");
            let _ = self.progress_peer_copy(published)?;
        }
        Ok(self
            .submissions
            .get(&id)
            .map_or(BackendPollV1::Pending, |record| record.status))
    }

    fn drain_published_direction_for_flush(
        &mut self,
        direction: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        while let Some(submission) = self.in_flight_by_direction[direction].first().copied() {
            let active = self
                .active
                .remove(&submission)
                .expect("indexed in-flight XGMI submission remains active");
            match self.progress_peer_copy(active)? {
                BackendPollV1::Succeeded => {
                    attempts = 0;
                    sleep = WAIT_INITIAL_SLEEP_V1;
                }
                BackendPollV1::Pending => {
                    attempts = attempts.saturating_add(1);
                    apply_unbounded_wait_backoff_v1(attempts, &mut sleep);
                }
                BackendPollV1::Failed { .. } => {
                    return Err(Self::quiescent_error(
                        KfdRuntimeBackendErrorKindV1::Native,
                        "published XGMI prefix completed with failure",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Destroys both directional queues after every logical handle is released.
    pub fn shutdown_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let resources = XgmiLogicalResourceCountsV1 {
            streams: self.streams.len(),
            allocations: self.allocations.len(),
            submissions: self.submissions.len(),
            active: self.active.len(),
            events: self.events.len(),
            event_retains: self.event_submission_retain_counts.len(),
            dependency_retains: self.dependency_retain_counts.len(),
            dependency_depths: self.dependency_depths.len(),
            dependency_waiters: self.dependency_waiters.len(),
            completion_reservations: self.completion_reservations,
            ready_index_entries: self.ready_by_direction.iter().map(|ids| ids.len()).sum(),
            in_flight_index_entries: self.in_flight_by_direction.iter().map(Vec::len).sum(),
            directional_active: self.active_by_direction.iter().sum(),
            stream_owners: self.active_stream_owners.len(),
            allocation_owners: self.active_allocation_owners.len(),
        };
        if !resources.permits_shutdown() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI logical resources remain live",
            ));
        }
        for direction in (0..2).rev() {
            if let Some(mut queue) = self.queues[direction].take() {
                let (source, destination) = Self::session_pair(&mut self.sessions, direction);
                queue
                    .destroy_and_release(source, destination)
                    .map_err(|error| {
                        self.terminal_error(format!("XGMI queue teardown: {error}"))
                    })?;
            }
        }
        self.shutdown = true;
        Ok(())
    }
}

impl RuntimeBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
        if self.device_index(device).is_none() {
            return RuntimeExecutionCapabilitiesV1::default();
        }
        native_xgmi_execution_capabilities_v1()
    }

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        Ok(self.descriptions.to_vec())
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let index = self.device_index(device).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "unknown native XGMI device",
            )
        })?;
        if self.streams.len() >= MAX_RUNTIME_STREAMS_V1 {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI stream capacity exceeded",
            ));
        }
        self.streams.try_reserve(1).map_err(|_| {
            Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, "XGMI stream table")
        })?;
        let id = self.next_id()?;
        self.streams.insert(id, index);
        Ok(id)
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown native XGMI stream",
            ));
        }
        if self.active_stream_owners.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI stream retains pending work",
            ));
        }
        self.streams.remove(&stream);
        Ok(())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let index = self.device_index(device).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "unknown native XGMI device",
            )
        })?;
        if kind != RuntimeMemoryKindV1::DeviceLocal {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native XGMI exposes PUBLIC device-local allocations only",
            ));
        }
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI allocation geometry",
            ));
        }
        self.allocations.try_reserve(1).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI allocation table",
            )
        })?;
        let id = self.next_id()?;
        let lease = self.sessions[index]
            .allocate_gfx942_xgmi_device_memory(byte_len, alignment)
            .map_err(|error| self.terminal_error(format!("native XGMI allocation: {error}")))?;
        self.allocations.insert(
            id,
            XgmiRuntimeAllocationV1 {
                device: index,
                byte_len,
                alignment,
                authority: Some(XgmiAllocationAuthorityV1::Unmapped(lease)),
            },
        );
        Ok(id)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.allocations.contains_key(&allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI allocation",
            ));
        }
        if self.allocation_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI allocation is retained by pending work",
            ));
        }
        self.ensure_allocation_unmapped(allocation)?;
        let (device, authority) = {
            let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown XGMI allocation",
                )
            })?;
            (record.device, record.authority.take())
        };
        let Some(XgmiAllocationAuthorityV1::Unmapped(lease)) = authority else {
            if let Some(authority) = authority {
                self.allocations.get_mut(&allocation).unwrap().authority = Some(authority);
            }
            return Err(self.terminal_error("native XGMI allocation lacks releasable authority"));
        };
        if let Err(error) = self.sessions[device].release_gfx942_device_memory(lease) {
            return Err(self.terminal_error(format!("native XGMI allocation release: {error}")));
        }
        self.allocations.remove(&allocation);
        Ok(())
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.allocation_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI allocation pending",
            ));
        }
        let (device, byte_len) = self
            .allocations
            .get(&allocation)
            .map(|record| (record.device, record.byte_len))
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown XGMI allocation",
                )
            })?;
        let end = byte_offset.checked_add(bytes.len() as u64).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "XGMI write overflow",
            )
        })?;
        if end > byte_len {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "XGMI write range",
            ));
        }
        let full_write = byte_offset == 0 && end == byte_len;
        let staged_full = if full_write {
            Some(
                try_copy_vec_v1(bytes, "native XGMI full-write staging allocation failed")?
                    .into_boxed_slice(),
            )
        } else {
            None
        };
        self.ensure_allocation_unmapped(allocation)?;
        let mut full = if let Some(full) = staged_full {
            full
        } else {
            match self.allocations[&allocation].authority.as_ref() {
                Some(XgmiAllocationAuthorityV1::Unmapped(lease)) => self.sessions[device]
                    .read_gfx942_xgmi_device_memory(lease)
                    .map_err(|error| {
                        self.terminal_error(format!("XGMI write read-modify: {error}"))
                    })?,
                _ => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "XGMI allocation authority unavailable",
                    ));
                }
            }
        };
        full[byte_offset as usize..end as usize].copy_from_slice(bytes);
        let lease = match self.allocations[&allocation].authority.as_ref() {
            Some(XgmiAllocationAuthorityV1::Unmapped(lease)) => lease,
            _ => unreachable!("validated unmapped authority"),
        };
        self.sessions[device]
            .write_gfx942_xgmi_device_memory(lease, &full)
            .map_err(|error| self.terminal_error(format!("native XGMI write: {error}")))
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.allocation_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI allocation pending",
            ));
        }
        let byte_len = self
            .allocations
            .get(&allocation)
            .map(|record| record.byte_len)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown XGMI allocation",
                )
            })?;
        let end = byte_offset
            .checked_add(destination.len() as u64)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "XGMI read overflow",
                )
            })?;
        if end > byte_len {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "XGMI read range",
            ));
        }
        self.ensure_allocation_unmapped(allocation)?;
        let record = self
            .allocations
            .get(&allocation)
            .expect("validated XGMI allocation remains indexed");
        let device = record.device;
        let Some(XgmiAllocationAuthorityV1::Unmapped(lease)) = record.authority.as_ref() else {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI allocation authority unavailable",
            ));
        };
        let bytes = self.sessions[device]
            .read_gfx942_xgmi_device_memory(lease)
            .map_err(|error| self.terminal_error(format!("native XGMI read: {error}")))?;
        destination.copy_from_slice(&bytes[byte_offset as usize..end as usize]);
        Ok(())
    }

    fn load_module_v1(
        &mut self,
        _device: u64,
        _image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "copy-only XGMI backend has no module loader",
        ))
    }

    fn unload_module_v1(
        &mut self,
        _module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "copy-only XGMI backend has no modules",
        ))
    }

    fn resolve_kernel_v1(
        &mut self,
        _module: u64,
        _name: &str,
        _signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "copy-only XGMI backend has no kernels",
        ))
    }

    fn submit_v1(
        &mut self,
        _launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "copy-only XGMI backend has no compute queue",
        ))
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if let Some(record) = self.submissions.get(&submission) {
            return Ok(record.status);
        }
        let active = self.active.get(&submission).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI submission",
            )
        })?;
        if xgmi_submission_has_failed_dependency_v1(active, &self.submissions) {
            let active = self
                .active
                .remove(&submission)
                .expect("failed-dependent XGMI submission remains active");
            return Ok(self.finish_failed(active));
        }
        if active.ticket.is_none() {
            return Ok(BackendPollV1::Pending);
        }
        let active = self
            .active
            .remove(&submission)
            .expect("validated XGMI submission remains active");
        self.progress_peer_copy(active)
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        wait_with_deadline_v1(deadline, || self.poll_v1(submission))
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.active.contains_key(&submission)
            || self
                .event_submission_retain_counts
                .contains_key(&submission)
            || self.dependency_retain_counts.contains_key(&submission)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI submission remains retained",
            ));
        }
        if !self.submissions.contains_key(&submission) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI submission",
            ));
        }
        if !self.dependency_depths.contains_key(&submission) {
            return Err(self.terminal_error("XGMI submission lost dependency-depth custody"));
        }
        self.submissions.remove(&submission);
        self.dependency_depths.remove(&submission);
        Ok(())
    }

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let submission_stream = self
            .submissions
            .get(&submission)
            .map(|record| record.stream)
            .or_else(|| self.active.get(&submission).map(|active| active.stream))
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown XGMI submission",
                )
            })?;
        if submission_stream != stream || !self.streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "XGMI event stream mismatch",
            ));
        }
        if self.events.len() >= MAX_RUNTIME_EVENTS_V1 {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI event capacity exceeded",
            ));
        }
        self.events.try_reserve(1).map_err(|_| {
            Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, "XGMI event table")
        })?;
        self.reserve_event_submission_retain(submission)?;
        let id = self.next_id()?;
        self.events.insert(id, EventRecordV1 { submission });
        self.retain_event_submission(submission);
        Ok(id)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let record = self.events.remove(&event).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI event",
            )
        })?;
        self.release_event_submission(record.submission);
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let outstanding = self
            .submissions
            .len()
            .checked_add(self.completion_reservations)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI submission count overflow",
                )
            })?;
        if outstanding >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI submission capacity exceeded",
            ));
        }
        let stream_device = *self.streams.get(&stream).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI stream",
            )
        })?;
        let source_record = self.allocations.get(&source.allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI source",
            )
        })?;
        let destination_record =
            self.allocations
                .get(&destination.allocation)
                .ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::UnknownHandle,
                        "unknown XGMI destination",
                    )
                })?;
        let source_device = source_record.device;
        let destination_device = destination_record.device;
        let admission = XgmiPeerCopyAdmissionV1 {
            stream_device,
            source_device,
            destination_device,
            source_offset: source.byte_offset,
            source_len: source.byte_len,
            source_allocation_len: source_record.byte_len,
            source_access: source.access,
            destination_offset: destination.byte_offset,
            destination_len: destination.byte_len,
            destination_allocation_len: destination_record.byte_len,
            destination_access: destination.access,
        };
        let Ok(direction) = admit_xgmi_peer_copy_v1(admission) else {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI peer-copy contract",
            ));
        };
        let dependency_submissions = collect_xgmi_dependencies_v1(&self.events, dependencies)
            .map_err(|error| match error {
                XgmiDependencyAdmissionErrorV1::TooMany
                | XgmiDependencyAdmissionErrorV1::Capacity => Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI dependency roster",
                ),
                XgmiDependencyAdmissionErrorV1::Unknown => Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown XGMI dependency",
                ),
                XgmiDependencyAdmissionErrorV1::Duplicate => Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "duplicate XGMI dependency",
                ),
            })?;
        let dependency_depth =
            match next_xgmi_dependency_depth_v1(&self.dependency_depths, &dependency_submissions) {
                Ok(depth) => depth,
                Err(XgmiDependencyAdmissionErrorV1::TooMany) => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "XGMI dependency depth exceeds the bounded profile",
                    ));
                }
                Err(XgmiDependencyAdmissionErrorV1::Unknown) => {
                    return Err(
                        self.terminal_error("XGMI dependency event lost submission-depth custody")
                    );
                }
                Err(
                    XgmiDependencyAdmissionErrorV1::Capacity
                    | XgmiDependencyAdmissionErrorV1::Duplicate,
                ) => {
                    unreachable!("depth admission does not allocate or deduplicate")
                }
            };
        if self.active_stream_owners.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI preserves stream order by admitting one pending copy per stream",
            ));
        }
        if [source.allocation, destination.allocation]
            .into_iter()
            .any(|allocation| {
                self.active_allocation_owners
                    .get(&allocation)
                    .is_some_and(|owners| {
                        owners
                            .iter()
                            .any(|owner| !dependency_submissions.contains(owner))
                    })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping XGMI copies require dependency",
            ));
        }
        if [source.allocation, destination.allocation]
            .into_iter()
            .any(|allocation| {
                self.active_allocation_owners
                    .get(&allocation)
                    .is_some_and(|owners| owners.len() >= MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1)
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI allocation custody capacity exceeded",
            ));
        }
        self.active_stream_owners.try_reserve(1).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI stream-owner index",
            )
        })?;
        let distinct_allocations = source.allocation != destination.allocation;
        let missing_allocation_entries = usize::from(
            !self
                .active_allocation_owners
                .contains_key(&source.allocation),
        ) + usize::from(
            distinct_allocations
                && !self
                    .active_allocation_owners
                    .contains_key(&destination.allocation),
        );
        self.active_allocation_owners
            .try_reserve(missing_allocation_entries)
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI allocation-owner index",
                )
            })?;
        let mut new_source_owners = None;
        if let Some(owners) = self.active_allocation_owners.get_mut(&source.allocation) {
            owners.try_reserve(1).map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI source-owner roster",
                )
            })?;
        } else {
            let mut owners = Vec::new();
            owners.try_reserve_exact(1).map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI source-owner roster",
                )
            })?;
            new_source_owners = Some(owners);
        }
        let mut new_destination_owners = None;
        if distinct_allocations {
            if let Some(owners) = self
                .active_allocation_owners
                .get_mut(&destination.allocation)
            {
                owners.try_reserve(1).map_err(|_| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "native XGMI destination-owner roster",
                    )
                })?;
            } else {
                let mut owners = Vec::new();
                owners.try_reserve_exact(1).map_err(|_| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "native XGMI destination-owner roster",
                    )
                })?;
                new_destination_owners = Some(owners);
            }
        }
        self.active.try_reserve(1).map_err(|_| {
            Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, "XGMI active table")
        })?;
        self.dependency_retain_counts
            .try_reserve(dependency_submissions.len())
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI dependency index",
                )
            })?;
        self.dependency_depths.try_reserve(1).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI dependency-depth index",
            )
        })?;
        if dependency_submissions.iter().any(|dependency| {
            self.dependency_retain_counts
                .get(dependency)
                .is_some_and(|count| *count == usize::MAX)
        }) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI dependency retain count overflow",
            ));
        }
        let next_direction_active = self.reserve_directional_index_slot(direction)?;

        // Preallocate every dependency-wakeup insertion before acquiring any
        // logical submission custody. Existing waiter lists remain sorted
        // because submission handles are monotonically increasing.
        let mut active_dependencies = Vec::new();
        active_dependencies
            .try_reserve_exact(dependency_submissions.len())
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI active-dependency preparation",
                )
            })?;
        active_dependencies.extend(
            dependency_submissions
                .iter()
                .copied()
                .filter(|dependency| self.active.contains_key(dependency)),
        );
        self.dependency_waiters
            .try_reserve(active_dependencies.len())
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI dependency-waiter index",
                )
            })?;
        let mut new_waiter_lists = Vec::new();
        new_waiter_lists
            .try_reserve_exact(active_dependencies.len())
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI dependency-waiter preparation",
                )
            })?;
        for dependency in &active_dependencies {
            if let Some(waiters) = self.dependency_waiters.get_mut(dependency) {
                waiters.try_reserve(1).map_err(|_| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "XGMI dependency-waiter list",
                    )
                })?;
            } else {
                let mut waiters = Vec::new();
                waiters.try_reserve_exact(1).map_err(|_| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Capacity,
                        "XGMI dependency-waiter list",
                    )
                })?;
                new_waiter_lists.push((*dependency, waiters));
            }
        }
        reserve_xgmi_completion_slot_v1(&mut self.submissions, &mut self.completion_reservations)
            .map_err(|XgmiCompletionReservationErrorV1::Capacity| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI completion table",
            )
        })?;
        let id = self.next_handle;
        let Some(next_handle) = id.checked_add(1) else {
            self.completion_reservations -= 1;
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI handle space exhausted",
            ));
        };
        self.next_handle = next_handle;
        if self.active_stream_owners.insert(stream, id).is_some() {
            std::process::abort();
        }
        if let Some(owners) = self.active_allocation_owners.get_mut(&source.allocation) {
            owners.push(id);
        } else {
            let mut owners = new_source_owners
                .take()
                .expect("new native XGMI source-owner roster was reserved");
            owners.push(id);
            self.active_allocation_owners
                .insert(source.allocation, owners);
        }
        if distinct_allocations {
            if let Some(owners) = self
                .active_allocation_owners
                .get_mut(&destination.allocation)
            {
                owners.push(id);
            } else {
                let mut owners = new_destination_owners
                    .take()
                    .expect("new native XGMI destination-owner roster was reserved");
                owners.push(id);
                self.active_allocation_owners
                    .insert(destination.allocation, owners);
            }
        }
        for (dependency, waiters) in new_waiter_lists {
            if self
                .dependency_waiters
                .insert(dependency, waiters)
                .is_some()
            {
                std::process::abort();
            }
        }
        for dependency in &active_dependencies {
            let waiters = self
                .dependency_waiters
                .get_mut(dependency)
                .expect("prepared XGMI dependency-waiter list");
            if waiters.last().is_some_and(|waiter| *waiter >= id) {
                std::process::abort();
            }
            waiters.push(id);
        }
        for dependency in &dependency_submissions {
            let count = self
                .dependency_retain_counts
                .entry(*dependency)
                .or_insert(0);
            *count += 1;
        }
        self.dependency_depths.insert(id, dependency_depth);
        let active = XgmiRuntimeSubmissionV1 {
            id,
            stream,
            direction,
            source: source.allocation,
            destination: destination.allocation,
            source_offset: source.byte_offset,
            destination_offset: destination.byte_offset,
            byte_len: source.byte_len as u32,
            dependencies: dependency_submissions,
            dependency_cursor: 0,
            ticket: None,
        };
        if xgmi_submission_is_ready_v1(&active, &self.submissions, direction) {
            enqueue_xgmi_ready_id_v1(&mut self.ready_by_direction[direction], id);
        }
        self.active_by_direction[direction] = next_direction_active;
        // Publication is intentionally deferred to the first progress call.
        // This gives adjacent facade submissions a bounded coalescing window;
        // all ready copies in the same direction are then published with one
        // native write-pointer update and one doorbell store.
        self.active.insert(id, active);
        Ok(id)
    }
}

impl RuntimeAsyncCopyBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn copy_async_v1(
        &mut self,
        _stream: u64,
        _source: BackendMemoryRegionV1,
        _destination: BackendMemoryRegionV1,
        _dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "copy-only XGMI backend has no same-device SDMA owner",
        ))
    }
}

fn reject_native_xgmi_semantic_submission_v1(
    semantic_launch: BackendSemanticLaunchV1,
    expected_atomic: bool,
) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
    let correct_variant = matches!(
        (expected_atomic, semantic_launch),
        (true, BackendSemanticLaunchV1::Atomic(_))
            | (false, BackendSemanticLaunchV1::Collective(_))
    );
    let (kind, detail) = if correct_variant {
        (
            KfdRuntimeBackendErrorKindV1::Unsupported,
            "copy-only native XGMI backend has no compute semantic owner",
        )
    } else {
        (
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            "native XGMI semantic SPI variant mismatch",
        )
    };
    RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(kind, detail))
}

impl RuntimeAtomicBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn submit_atomic_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        Err(reject_native_xgmi_semantic_submission_v1(
            launch.semantic_launch,
            true,
        ))
    }
}

impl RuntimeCollectiveBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn submit_collective_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        Err(reject_native_xgmi_semantic_submission_v1(
            launch.semantic_launch,
            false,
        ))
    }
}

impl RuntimeFlushBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let destination = *self.streams.get(&stream).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown native XGMI stream",
            )
        })?;
        let direction = xgmi_direction_for_destination_v1(destination)
            .ok_or_else(|| self.terminal_error("native XGMI stream lost destination binding"))?;
        let failed = self
            .active_stream_owners
            .get(&stream)
            .copied()
            .filter(|submission| {
                self.active.get(submission).is_some_and(|active| {
                    active.ticket.is_none()
                        && xgmi_submission_has_failed_dependency_v1(active, &self.submissions)
                })
            });
        if let Some(submission) = failed {
            let active = self
                .active
                .remove(&submission)
                .expect("failed-dependent XGMI submission remains active");
            self.finish_failed(active);
            return Err(Self::quiescent_error(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI dependency failed before publication",
            ));
        }

        let ready_at_entry = self.ready_by_direction[direction].len();
        match classify_xgmi_flush_v1(
            ready_at_entry,
            !self.in_flight_by_direction[direction].is_empty(),
            GFX942_SDMA_MAX_IN_FLIGHT_V1,
        ) {
            XgmiFlushAdmissionV1::NoReadyWork => return Ok(()),
            XgmiFlushAdmissionV1::InFlight => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "native XGMI direction already has a published batch",
                ));
            }
            XgmiFlushAdmissionV1::Capacity => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "native XGMI ready flush exceeds ring admission",
                ));
            }
            XgmiFlushAdmissionV1::Publish { .. } => {}
        }
        let mut progress = XgmiFlushPrefixProgressV1::new(ready_at_entry);
        loop {
            let published = progress.next_batch_len();
            let outcome = self
                .publish_ready_peer_batch(direction)
                .map_err(|failure| progress.classify_publication_failure(failure))?;
            match outcome {
                XgmiBatchPublicationOutcomeV1::Published => {}
                XgmiBatchPublicationOutcomeV1::RecoveredPrepublicationFailure => {
                    return Err(Self::quiescent_error(
                        KfdRuntimeBackendErrorKindV1::Native,
                        "native XGMI flush recovered a prepublication failure",
                    ));
                }
                XgmiBatchPublicationOutcomeV1::NoReadyWork
                | XgmiBatchPublicationOutcomeV1::AlreadyInFlight => {
                    return Err(self.terminal_error(
                        "native XGMI flush admission changed without concurrent access",
                    ));
                }
            }
            progress.note_published(published);
            if progress.remaining_at_entry == 0 {
                return Ok(());
            }
            self.drain_published_direction_for_flush(direction)?;
            progress.note_completed_prefix();
        }
    }
}

impl RuntimeCancellationBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let disposition = xgmi_cancellation_disposition_v1(
            self.active
                .get(&submission)
                .map(|active| active.ticket.is_some()),
            self.submissions.contains_key(&submission),
        );
        match disposition {
            XgmiCancellationDispositionV1::TooLate => {
                return Ok(crate::BackendCancellationV1::TooLate);
            }
            XgmiCancellationDispositionV1::Unknown => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown XGMI submission",
                ));
            }
            XgmiCancellationDispositionV1::CancelPrepublication => {}
        }
        let active = self
            .active
            .remove(&submission)
            .expect("prepublication XGMI submission remains active");
        self.settle_submission(active, BackendPollV1::Failed { code: -2 });
        Ok(crate::BackendCancellationV1::Cancelled)
    }

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.wait_v1(submission, deadline)
    }
}

impl Drop for KfdNativeXgmiRuntimeBackendV1 {
    fn drop(&mut self) {
        if self.terminal
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.submissions.is_empty()
            || !self.active.is_empty()
            || !self.active_stream_owners.is_empty()
            || !self.active_allocation_owners.is_empty()
            || !self.events.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || !self.dependency_retain_counts.is_empty()
            || !self.dependency_depths.is_empty()
            || !self.dependency_waiters.is_empty()
            || self.completion_reservations != 0
            || self.ready_by_direction.iter().any(|ids| !ids.is_empty())
            || self
                .in_flight_by_direction
                .iter()
                .any(|ids| !ids.is_empty())
            || self.active_by_direction != [0, 0]
        {
            std::process::abort();
        }
        for direction in (0..2).rev() {
            if let Some(mut queue) = self.queues[direction].take() {
                let (source, destination) = Self::session_pair(&mut self.sessions, direction);
                if queue.destroy_and_release(source, destination).is_err() {
                    std::process::abort();
                }
            }
        }
    }
}

impl RuntimeBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
        let Some(child) = self
            .device_children
            .get(&device)
            .and_then(|index| self.children.get(*index))
        else {
            return RuntimeExecutionCapabilitiesV1::default();
        };
        let mut capabilities = child.execution_capabilities_v1(device);
        capabilities.cancellation = true;
        capabilities
    }

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let mut descriptions = Vec::new();
        descriptions
            .try_reserve_exact(self.children.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("multi-device description allocation failed")
            })?;
        for index in 0..self.children.len() {
            let current = self.children[index].require_live();
            self.latch(current)?;
            let child = &self.children[index];
            let mut description = child.description.clone();
            description.capabilities.multi_device = true;
            description.capabilities.peer_copy = true;
            descriptions.push(description);
        }
        Ok(descriptions)
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let child = self.child_for_device(device)?;
        Self::reserve_route(
            &mut self.streams,
            "multi-device stream route allocation failed",
        )?;
        let id = self.next_id()?;
        let result = self.children[child].create_stream_v1(device);
        let local = self.latch(result)?;
        self.streams.insert(id, RoutedHandleV1 { child, local });
        Ok(id)
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.cooperative_stream_pending_counts.contains_key(&stream)
            || self.cooperative_stream_tails.contains_key(&stream)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream retains a pending cooperative copy",
            ));
        }
        let route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        let result = self.children[route.child].destroy_stream_v1(route.local);
        self.latch(result)?;
        self.streams.remove(&stream);
        Ok(())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let child = self.child_for_device(device)?;
        Self::reserve_route(
            &mut self.allocations,
            "multi-device allocation route allocation failed",
        )?;
        let id = self.next_id()?;
        let result = self.children[child].allocate_v1(device, kind, byte_len, alignment);
        let local = self.latch(result)?;
        self.allocations.insert(id, RoutedHandleV1 { child, local });
        Ok(id)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = Self::route(
            &self.allocations,
            allocation,
            "unknown multi-device KFD allocation",
        )?;
        if self.allocation_retained_by_cooperative_copy(route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending cooperative copy",
            ));
        }
        let result = self.children[route.child].release_allocation_v1(route.local);
        self.latch(result)?;
        self.allocations.remove(&allocation);
        Ok(())
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = Self::route(
            &self.allocations,
            allocation,
            "unknown multi-device KFD allocation",
        )?;
        if self.allocation_retained_by_cooperative_copy(route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending cooperative copy",
            ));
        }
        let result =
            self.children[route.child].write_allocation_v1(route.local, byte_offset, bytes);
        self.latch(result)
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = Self::route(
            &self.allocations,
            allocation,
            "unknown multi-device KFD allocation",
        )?;
        if self.allocation_retained_by_cooperative_copy(route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending cooperative copy",
            ));
        }
        let result =
            self.children[route.child].read_allocation_v1(route.local, byte_offset, destination);
        self.latch(result)
    }

    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let child = self.child_for_device(device)?;
        Self::reserve_route(
            &mut self.modules,
            "multi-device module route allocation failed",
        )?;
        let id = self.next_id()?;
        let result = self.children[child].load_module_v1(device, image);
        let local = self.latch(result)?;
        self.modules.insert(id, RoutedHandleV1 { child, local });
        Ok(id)
    }

    fn unload_module_v1(
        &mut self,
        module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = Self::route(&self.modules, module, "unknown multi-device KFD module")?;
        let result = self.children[route.child].unload_module_v1(route.local);
        self.latch(result)?;
        self.modules.remove(&module);
        self.kernels
            .retain(|kernel, _| self.kernel_modules.get(kernel) != Some(&module));
        self.kernel_modules
            .retain(|_, retained_module| *retained_module != module);
        Ok(())
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = Self::route(&self.modules, module, "unknown multi-device KFD module")?;
        Self::reserve_route(
            &mut self.kernels,
            "multi-device kernel route allocation failed",
        )?;
        Self::reserve_route(
            &mut self.kernel_modules,
            "multi-device kernel-module route allocation failed",
        )?;
        let id = self.next_id()?;
        let result = self.children[route.child].resolve_kernel_v1(route.local, name, signature);
        let local = self.latch(result)?;
        self.kernels.insert(
            id,
            RoutedHandleV1 {
                child: route.child,
                local,
            },
        );
        self.kernel_modules.insert(id, module);
        Ok(id)
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_submission_capacity_v1()?;
        let stream = Self::route(
            &self.streams,
            launch.stream,
            "unknown multi-device KFD stream",
        )?;
        let kernel = Self::route(
            &self.kernels,
            launch.kernel,
            "unknown multi-device KFD kernel",
        )?;
        if stream.child != kernel.child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "kernel and stream belong to different KFD devices",
            ));
        }
        if self.stream_has_pending_cooperative_copy_v1(launch.stream) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "mixed cooperative/native stream ordering requires quiescing prior cooperative work",
            ));
        }
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("multi-device binding translation failed")
            })?;
        for binding in launch.bindings {
            let allocation = Self::route(
                &self.allocations,
                binding.region.allocation,
                "unknown multi-device KFD allocation",
            )?;
            if allocation.child != stream.child {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::WrongDevice,
                    "kernel binding belongs to another KFD device",
                ));
            }
            if self.allocation_retained_by_cooperative_copy(allocation) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "kernel binding is retained by a pending cooperative copy",
                ));
            }
            bindings.push(BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocation.local,
                    access: binding.region.access,
                    byte_offset: binding.region.byte_offset,
                    byte_len: binding.region.byte_len,
                },
                kernarg_byte_offset: binding.kernarg_byte_offset,
            });
        }
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(launch.dependencies.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("multi-device dependency translation failed")
            })?;
        for event in launch.dependencies {
            if let Some(local) = self.dependency_for_child(*event, stream.child)? {
                dependencies.push(local);
            }
        }
        self.reserve_native_stream_submission_v1(launch.stream)?;
        Self::reserve_route(
            &mut self.submissions,
            "multi-device submission route allocation failed",
        )?;
        let id = self.next_id()?;
        let result = self.children[stream.child].submit_v1(BackendLaunchV1 {
            stream: stream.local,
            kernel: kernel.local,
            explicit_kernarg: launch.explicit_kernarg,
            bindings: &bindings,
            dependencies: &dependencies,
            geometry: launch.geometry,
            semantic_launch: launch.semantic_launch,
        });
        let local = self.latch(result)?;
        self.submissions.insert(
            id,
            RoutedSubmissionV1::Native {
                route: RoutedHandleV1 {
                    child: stream.child,
                    local,
                },
                stream: launch.stream,
            },
        );
        self.retain_native_stream_submission_v1(launch.stream);
        Ok(id)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
        };
        match native_route {
            Some(route) => {
                let result = self.children[route.child].poll_v1(route.local);
                self.latch(result)
            }
            None => {
                let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission]
                else {
                    unreachable!()
                };
                Ok(copy.status())
            }
        }
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
        };
        match native_route {
            Some(route) => {
                let result = self.children[route.child].wait_v1(route.local, deadline);
                self.latch(result)
            }
            None => {
                let mut attempts = 0_u32;
                let mut sleep = WAIT_INITIAL_SLEEP_V1;
                loop {
                    let status = self.poll_v1(submission)?;
                    if status != BackendPollV1::Pending {
                        return Ok(status);
                    }
                    attempts = attempts.saturating_add(1);
                    if !apply_wait_backoff_v1(attempts, &mut sleep, deadline) {
                        return Ok(BackendPollV1::Pending);
                    }
                }
            }
        }
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let (native_route, native_stream, cooperative_stream, cooperative_quiescent) =
            match self.submissions.get(&submission).ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown multi-device KFD submission",
                )
            })? {
                RoutedSubmissionV1::Native { route, stream } => {
                    (Some(*route), Some(*stream), None, true)
                }
                RoutedSubmissionV1::CooperativeCopy(copy) => {
                    (None, None, Some(copy.stream), copy.is_quiescent())
                }
            };
        if self
            .event_submission_retain_counts
            .contains_key(&submission)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission is retained by a multi-device event",
            ));
        }
        if self.submission_retained_as_dependency(submission) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission is retained by a pending cooperative copy",
            ));
        }
        if !cooperative_quiescent {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cooperative copy submission is pending",
            ));
        }
        if let Some(route) = native_route {
            let result = self.children[route.child].release_submission_v1(route.local);
            self.latch(result)?;
        }
        if let Some(stream) = native_stream {
            self.release_native_stream_submission_v1(stream);
        }
        if cooperative_stream
            .is_some_and(|stream| self.cooperative_stream_tails.get(&stream) == Some(&submission))
        {
            self.cooperative_stream_tails
                .remove(&cooperative_stream.expect("matched cooperative stream"));
        }
        self.submissions.remove(&submission);
        Ok(())
    }

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let stream_route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        let submission_route = self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })?;
        let submission_route = match submission_route {
            RoutedSubmissionV1::Native { route, .. } => (Some(*route), None),
            RoutedSubmissionV1::CooperativeCopy(copy) => (None, Some(copy.stream)),
        };
        let stream_matches = match submission_route {
            (Some(route), None) => route.child == stream_route.child,
            (None, Some(copy_stream)) => copy_stream == stream,
            _ => false,
        };
        if !stream_matches {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "submission belongs to another multi-device stream",
            ));
        }
        Self::reserve_route(
            &mut self.events,
            "multi-device event route allocation failed",
        )?;
        if !self
            .event_submission_retain_counts
            .contains_key(&submission)
        {
            self.event_submission_retain_counts
                .try_reserve(1)
                .map_err(|_| {
                    KfdRuntimeBackendV1::capacity("multi-device event-retain index growth failed")
                })?;
        }
        if self
            .event_submission_retain_counts
            .get(&submission)
            .is_some_and(|count| *count == usize::MAX)
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "multi-device event retain count overflow",
            ));
        }
        if self.next_handle == u64::MAX {
            return Err(KfdRuntimeBackendV1::capacity(
                "multi-device routing handle space exhausted",
            ));
        }
        let routed = match submission_route {
            (Some(route), None) => {
                let result =
                    self.children[route.child].record_event_v1(stream_route.local, route.local);
                let local = self.latch(result)?;
                RoutedEventV1::Native {
                    route: RoutedHandleV1 {
                        child: route.child,
                        local,
                    },
                    submission,
                }
            }
            (None, Some(_)) => RoutedEventV1::CooperativeCopy {
                submission,
                child: stream_route.child,
            },
            _ => unreachable!("validated routed submission has one kind"),
        };
        let id = self.next_id()?;
        self.events.insert(id, routed);
        let count = self
            .event_submission_retain_counts
            .entry(submission)
            .or_insert(0);
        *count += 1;
        Ok(id)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = self.events.get(&event).copied().ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD event",
            )
        })?;
        if let RoutedEventV1::Native { route, .. } = route {
            let result = self.children[route.child].release_event_v1(route.local);
            self.latch(result)?;
        }
        let submission = match route {
            RoutedEventV1::Native { submission, .. }
            | RoutedEventV1::CooperativeCopy { submission, .. } => submission,
        };
        self.events.remove(&event);
        Self::decrement_indexed_count(
            &mut self.event_submission_retain_counts,
            submission,
            "live multi-device event retain count is indexed",
        );
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.submit_cooperative_copy(stream, source, destination, dependencies, true)
    }
}

impl RuntimeAtomicBackendV1 for KfdRuntimeBackendV1 {
    fn submit_atomic_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        if !matches!(
            launch.semantic_launch,
            KfdRuntimeSemanticLaunchV1::Atomic(_)
        ) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "atomic SPI requires an atomic semantic launch contract",
            ));
        }
        self.submit_v1(launch)
    }
}

impl RuntimeCollectiveBackendV1 for KfdRuntimeBackendV1 {
    fn submit_collective_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        if !matches!(
            launch.semantic_launch,
            KfdRuntimeSemanticLaunchV1::Collective(_)
        ) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "collective SPI requires a collective semantic launch contract",
            ));
        }
        self.submit_v1(launch)
    }
}

impl RuntimeAsyncCopyBackendV1 for KfdRuntimeBackendV1 {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native KFD async copy is unavailable on a synthetic backend",
            ));
        }
        if !self.streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD copy stream",
            ));
        }
        if source.allocation == destination.allocation
            || source.byte_len == 0
            || source.byte_len != destination.byte_len
            || !matches!(
                source.access,
                RuntimeAccessV1::Read | RuntimeAccessV1::ReadWrite
            )
            || !matches!(
                destination.access,
                RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
            )
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native KFD copy requires distinct allocations, equal nonzero ranges, and valid access",
            ));
        }
        let fits = |region: BackendMemoryRegionV1| {
            native_sdma_region_is_admitted_v1(
                self.allocations.get(&region.allocation),
                self.description.backend_device,
                region,
            )
        };
        if !fits(source) || !fits(destination) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native KFD copy range exceeds its persistent allocation",
            ));
        }
        let source_kind = self
            .allocations
            .get(&source.allocation)
            .expect("admitted source remains indexed")
            .kind;
        let destination_kind = self
            .allocations
            .get(&destination.allocation)
            .expect("admitted destination remains indexed")
            .kind;
        if direct_sdma_direction_v1(source_kind, destination_kind).is_none() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "direct KFD copy supports only host-to-device or device-to-host direction",
            ));
        }
        self.require_submission_capacity_v1()?;
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(Self::capacity("KFD copy dependency capacity exceeded"));
        }
        let stream_tail = self.stream_submission_tails.get(&stream).copied();
        let mut dependency_submissions = Vec::new();
        dependency_submissions
            .try_reserve_exact(
                dependencies
                    .len()
                    .saturating_add(usize::from(stream_tail.is_some())),
            )
            .map_err(|_| Self::capacity("KFD copy dependency allocation failed"))?;
        for event in dependencies {
            let submission = self
                .events
                .get(event)
                .map(|event| event.submission)
                .ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::UnknownHandle,
                        "unknown KFD event dependency",
                    )
                })?;
            if dependency_submissions.contains(&submission) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD copy dependencies must name distinct submissions",
                ));
            }
            if self
                .submissions
                .get(&submission)
                .is_some_and(|record| matches!(record.status, BackendPollV1::Failed { .. }))
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD copy dependency completed with failure",
                ));
            }
            dependency_submissions.push(submission);
        }
        if let Some(tail) = stream_tail
            && !dependency_submissions.contains(&tail)
        {
            if dependency_submissions.len() == MAX_RUNTIME_DEPENDENCIES_V1 {
                return Err(Self::capacity(
                    "KFD copy dependency capacity exceeded by stream ordering",
                ));
            }
            if self
                .submissions
                .get(&tail)
                .is_some_and(|record| matches!(record.status, BackendPollV1::Failed { .. }))
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "prior work in the KFD stream completed with failure",
                ));
            }
            dependency_submissions.push(tail);
        }
        let dependency_depth = self
            .next_dependency_depth_v1(&dependency_submissions)
            .map_err(|error| {
                let detail = match error {
                    DirectSdmaDependencyDepthErrorV1::Overflow => {
                        "KFD SDMA dependency depth overflow"
                    }
                    DirectSdmaDependencyDepthErrorV1::LimitExceeded => {
                        "KFD SDMA dependency depth capacity exceeded"
                    }
                };
                Self::capacity(detail)
            })?;
        let mut compute_admission = KfdCopyComputeAdmissionV1::Concurrent;
        for allocation in [source.allocation, destination.allocation] {
            let Some(custody) = self.allocation_custody.get(&allocation) else {
                continue;
            };
            if custody.sole_stream == Some(stream) {
                if custody.owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] != 0 {
                    compute_admission = KfdCopyComputeAdmissionV1::DeferredByDependency;
                }
                continue;
            }
            for owner in custody
                .owners
                .iter()
                .filter(|owner| owner.kind == RuntimeAllocationCustodyKindV1::Compute)
            {
                let next = if owner.stream == stream
                    || dependency_submissions.contains(&owner.submission)
                {
                    KfdCopyComputeAdmissionV1::DeferredByDependency
                } else {
                    KfdCopyComputeAdmissionV1::Busy
                };
                compute_admission = match (compute_admission, next) {
                    (KfdCopyComputeAdmissionV1::Busy, _) | (_, KfdCopyComputeAdmissionV1::Busy) => {
                        KfdCopyComputeAdmissionV1::Busy
                    }
                    _ => KfdCopyComputeAdmissionV1::DeferredByDependency,
                };
            }
        }
        if compute_admission == KfdCopyComputeAdmissionV1::Busy {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "KFD copy overlaps active compute without its explicit event dependency",
            ));
        }
        if [source.allocation, destination.allocation]
            .into_iter()
            .any(|allocation| {
                self.allocation_has_unordered_custody_v1(
                    allocation,
                    stream,
                    &dependency_submissions,
                    Some(RuntimeAllocationCustodyKindV1::Sdma),
                )
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping KFD copies require an explicit event dependency",
            ));
        }
        self.active_sdma
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD SDMA submission ledger growth failed"))?;
        let next_sdma_completion_reservations = self
            .sdma_completion_reservations
            .checked_add(1)
            .ok_or_else(|| Self::capacity("KFD SDMA completion reservation overflow"))?;
        self.quiescent_sdma_submissions
            .try_reserve(next_sdma_completion_reservations)
            .map_err(|_| Self::capacity("KFD quiescent SDMA ledger growth failed"))?;
        let total_completion_reservations = self
            .compute_completion_reservations
            .checked_add(next_sdma_completion_reservations)
            .ok_or_else(|| Self::capacity("KFD completion reservation overflow"))?;
        self.submissions
            .try_reserve(total_completion_reservations)
            .map_err(|_| Self::capacity("KFD submission-table growth failed"))?;
        let retained_allocations = [source.allocation, destination.allocation];
        let new_allocation_custody = self.reserve_allocation_custody_v1(&retained_allocations)?;
        let new_active_sdma_stream = self.reserve_active_sdma_stream_v1(stream)?;
        if !self.stream_submission_tails.contains_key(&stream) {
            self.stream_submission_tails
                .try_reserve(1)
                .map_err(|_| Self::capacity("KFD stream-tail index growth failed"))?;
        }
        let new_dependency_entries = dependency_submissions
            .iter()
            .filter(|submission| !self.sdma_dependency_retain_counts.contains_key(submission))
            .count();
        self.sdma_dependency_retain_counts
            .try_reserve(new_dependency_entries)
            .map_err(|_| Self::capacity("KFD SDMA dependency-retain growth failed"))?;
        if dependency_submissions.iter().any(|submission| {
            self.sdma_dependency_retain_counts
                .get(submission)
                .is_some_and(|count| *count == usize::MAX)
        }) {
            return Err(Self::capacity("KFD SDMA dependency retain count overflow"));
        }
        if self.next_handle == u64::MAX {
            return Err(Self::capacity("backend handle space exhausted"));
        }
        if compute_admission == KfdCopyComputeAdmissionV1::Concurrent
            && self.any_compute_active_v1()
            && [source.allocation, destination.allocation]
                .into_iter()
                .any(|allocation| !self.allocations[&allocation].native_dirty.is_empty())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "disjoint KFD copy requires deferred native-data reconciliation",
            ));
        }
        let id = self.next_id()?;
        self.sdma_completion_reservations = next_sdma_completion_reservations;
        debug_assert!(self.quiescent_sdma_marker_capacity_is_reserved_v1());
        self.retain_allocation_custody_v1(
            &retained_allocations,
            RuntimeAllocationCustodyOwnerV1 {
                submission: id,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            },
            new_allocation_custody,
        );
        for submission in &dependency_submissions {
            *self
                .sdma_dependency_retain_counts
                .entry(*submission)
                .or_insert(0) += 1;
        }
        let active = ActiveSdmaCopyV1 {
            id,
            stream,
            prior_stream_submission: stream_tail,
            source: source.allocation,
            destination: destination.allocation,
            source_offset: source.byte_offset,
            destination_offset: destination.byte_offset,
            byte_len: source.byte_len,
            completed_bytes: 0,
            packet_bytes: 0,
            dependencies: dependency_submissions,
            dependency_cursor: 0,
            dependency_depth,
            phase: ActiveDirectionalSdmaPhaseV1::Ready,
        };
        self.retain_active_sdma_stream_v1(stream, id, new_active_sdma_stream);
        self.stream_submission_tails.insert(stream, id);
        let all_ready = active.dependencies.iter().all(|submission| {
            self.submissions
                .get(submission)
                .is_some_and(|record| record.status == BackendPollV1::Succeeded)
        });
        let storage_is_clean =
            [source.allocation, destination.allocation]
                .into_iter()
                .all(|allocation| {
                    self.allocations.get(&allocation).is_some_and(|record| {
                        !record.sdma_shadow_dirty && record.native_dirty.is_empty()
                    })
                });
        if all_ready && storage_is_clean {
            self.publish_sdma_copy_v1(active)?;
        } else {
            self.active_sdma.insert(id, active);
        }
        Ok(id)
    }
}

impl RuntimeFlushBackendV1 for KfdRuntimeBackendV1 {
    /// Publishes the dependency-ready stream head. This explicit progress call
    /// may block while reconciling dirty native storage; `poll_v1` never enters
    /// that preparation path.
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if !self.streams.contains_key(&stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD compute stream",
            ));
        }
        let compute = self
            .pending_compute_streams
            .get(&stream)
            .and_then(|queue| queue.front())
            .copied();
        let sdma = self
            .active_sdma_streams
            .get(&stream)
            .and_then(|submissions| submissions.front())
            .copied()
            .filter(|submission| {
                self.active_sdma
                    .get(submission)
                    .is_some_and(|copy| matches!(copy.phase, ActiveDirectionalSdmaPhaseV1::Ready))
            });
        let Some(submission) = compute.into_iter().chain(sdma).min() else {
            return Ok(());
        };
        if sdma == Some(submission) {
            let active = self
                .active_sdma
                .remove(&submission)
                .expect("selected unpublished SDMA submission remains indexed");
            let status = self.progress_unpublished_sdma_copy_v1(active)?;
            if matches!(status, BackendPollV1::Failed { .. }) {
                return Err(Self::quiescent_error(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "KFD SDMA stream-head publication failed before publication",
                ));
            }
            if self.active_sdma.get(&submission).is_some_and(|copy| {
                matches!(copy.phase, ActiveDirectionalSdmaPhaseV1::Ready)
                    && copy.dependency_cursor == copy.dependencies.len()
            }) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "dependency-ready KFD SDMA work was not published",
                ));
            }
            return Ok(());
        }
        if self.free_compute_lane_v1().is_none() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "KFD compute stream head has no mutation-free publication slot",
            ));
        }
        let pending = self
            .pending_compute
            .get(&submission)
            .expect("compute stream FIFO head remains pending");
        let overlaps_published_compute = (0..self.native_compute_lanes.len()).any(|lane| {
            let active = if lane == 0 {
                self.active.as_ref()
            } else {
                self.auxiliary_compute_lanes[lane - 1].active.as_ref()
            };
            launch_overlaps_active_compute_v1(&pending.launch.bindings, active.into_iter())
        });
        let overlaps_published_sdma = self
            .published_sdma_conflict_v1(pending.id, pending.launch.stream, &pending.launch.bindings)
            .is_some();
        if overlaps_published_compute || overlaps_published_sdma {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "KFD compute stream head conflicts with published native work",
            ));
        }
        let pending = self
            .pending_compute
            .remove(&submission)
            .expect("compute stream FIFO head remains pending");
        let status = self.progress_pending_compute_v1(pending)?;
        if matches!(status, BackendPollV1::Failed { .. }) {
            return Err(Self::quiescent_error(
                KfdRuntimeBackendErrorKindV1::Native,
                "KFD compute stream-head publication failed before publication",
            ));
        }
        if self
            .pending_compute
            .get(&submission)
            .is_some_and(|pending| pending.dependency_cursor == pending.dependencies.len())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "dependency-ready KFD compute work has no publishable native lane",
            ));
        }
        Ok(())
    }
}

impl RuntimeCancellationBackendV1 for KfdRuntimeBackendV1 {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.submissions.contains_key(&submission)
            || self.active_compute_lane_v1(submission).is_some()
        {
            // A published AQL/SDMA packet has no reviewed withdrawal primitive;
            // completed records are likewise conclusive.
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        if let Some(pending) = self.pending_compute.remove(&submission) {
            let stream = pending.launch.stream;
            let prior = pending.prior_stream_submission;
            self.settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -2 });
            self.restore_stream_tail_before_v1(stream, submission, prior);
            return Ok(crate::BackendCancellationV1::Cancelled);
        }
        if self.active_sdma.get(&submission).is_some_and(|active| {
            matches!(active.phase, ActiveDirectionalSdmaPhaseV1::Published(_))
                || active.completed_bytes != 0
        }) {
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        if let Some(active) = self.active_sdma.remove(&submission) {
            let stream = active.stream;
            let prior = active.prior_stream_submission;
            self.release_sdma_dependency_retains_v1(&active.dependencies);
            self.release_allocation_custody_v1(active.source, active.id);
            self.release_allocation_custody_v1(active.destination, active.id);
            self.release_active_sdma_stream_v1(stream, submission);
            self.submissions.insert(
                submission,
                SubmissionRecordV1 {
                    stream: active.stream,
                    status: BackendPollV1::Failed { code: -2 },
                },
            );
            self.sdma_completion_reservations = self
                .sdma_completion_reservations
                .checked_sub(1)
                .expect("cancelled SDMA copy reserved one completion slot");
            self.restore_stream_tail_before_v1(stream, submission, prior);
            return Ok(crate::BackendCancellationV1::Cancelled);
        }
        Err(Self::rejected(
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
            "unknown KFD submission",
        ))
    }

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.wait_v1(submission, deadline)
    }
}

impl RuntimeAtomicBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn submit_atomic_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        if !matches!(
            launch.semantic_launch,
            KfdRuntimeSemanticLaunchV1::Atomic(_)
        ) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "atomic SPI requires an atomic semantic launch contract",
            ));
        }
        self.submit_v1(launch)
    }
}

impl RuntimeCollectiveBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn submit_collective_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        if !matches!(
            launch.semantic_launch,
            KfdRuntimeSemanticLaunchV1::Collective(_)
        ) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "collective SPI requires a collective semantic launch contract",
            ));
        }
        self.submit_v1(launch)
    }
}

impl RuntimeAsyncCopyBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_submission_capacity_v1()?;
        let stream_route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        let source_route = Self::route(
            &self.allocations,
            source.allocation,
            "unknown source KFD allocation",
        )?;
        let destination_route = Self::route(
            &self.allocations,
            destination.allocation,
            "unknown destination KFD allocation",
        )?;
        if source_route.child == destination_route.child
            && destination_route.child == stream_route.child
            && self.children[stream_route.child].native_available
        {
            if self.stream_has_pending_cooperative_copy_v1(stream) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "mixed cooperative/native stream ordering requires quiescing prior cooperative work",
                ));
            }
            if self.allocation_retained_by_cooperative_copy(source_route)
                || self.allocation_retained_by_cooperative_copy(destination_route)
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "native copy allocation is retained by a pending cooperative copy",
                ));
            }
            let mut translated_dependencies = Vec::new();
            translated_dependencies
                .try_reserve_exact(dependencies.len())
                .map_err(|_| KfdRuntimeBackendV1::capacity("copy dependency translation failed"))?;
            for event in dependencies {
                if let Some(local) = self.dependency_for_child(*event, stream_route.child)? {
                    translated_dependencies.push(local);
                }
            }
            self.reserve_native_stream_submission_v1(stream)?;
            Self::reserve_route(
                &mut self.submissions,
                "multi-device native-copy submission route allocation failed",
            )?;
            let id = self.next_id()?;
            let result = self.children[stream_route.child].copy_async_v1(
                stream_route.local,
                BackendMemoryRegionV1 {
                    allocation: source_route.local,
                    ..source
                },
                BackendMemoryRegionV1 {
                    allocation: destination_route.local,
                    ..destination
                },
                &translated_dependencies,
            );
            let local = self.latch(result)?;
            self.submissions.insert(
                id,
                RoutedSubmissionV1::Native {
                    route: RoutedHandleV1 {
                        child: stream_route.child,
                        local,
                    },
                    stream,
                },
            );
            self.retain_native_stream_submission_v1(stream);
            return Ok(id);
        }
        self.submit_cooperative_copy(stream, source, destination, dependencies, false)
    }
}

impl RuntimeFlushBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        if let Some(submission) =
            self.cooperative_stream_tails
                .get(&stream)
                .copied()
                .filter(|submission| {
                    matches!(
                        self.submissions.get(submission),
                        Some(RoutedSubmissionV1::CooperativeCopy(copy)) if !copy.is_quiescent()
                    )
                })
        {
            loop {
                let progress_before = self.cooperative_progress_generation;
                let status = self.progress_cooperative_copy(submission)?;
                match status {
                    BackendPollV1::Succeeded => return Ok(()),
                    BackendPollV1::Failed { .. } => {
                        return Err(KfdRuntimeBackendV1::quiescent_error(
                            KfdRuntimeBackendErrorKindV1::Native,
                            "multi-device cooperative flush ended in quiescent failure",
                        ));
                    }
                    BackendPollV1::Pending
                        if self.cooperative_progress_generation == progress_before =>
                    {
                        return Ok(());
                    }
                    BackendPollV1::Pending => {}
                }
            }
        }
        let result = self.children[route.child].flush_stream_v1(route.local);
        self.latch(result)
    }
}

impl RuntimeCancellationBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(copy) => {
                let cancellable = match copy.phase {
                    CooperativeCopyPhaseV1::Dependencies | CooperativeCopyPhaseV1::Read => true,
                    CooperativeCopyPhaseV1::Write => copy.byte_cursor == 0,
                    CooperativeCopyPhaseV1::Succeeded
                    | CooperativeCopyPhaseV1::Failed
                    | CooperativeCopyPhaseV1::Cancelled => false,
                };
                if !cancellable {
                    return Ok(crate::BackendCancellationV1::TooLate);
                }
                None
            }
        };
        if let Some(route) = native_route {
            let result = self.children[route.child].cancel_v1(route.local);
            return self.latch(result);
        }

        let (stream, prior) = match &self.submissions[&submission] {
            RoutedSubmissionV1::CooperativeCopy(copy) => {
                (copy.stream, copy.prior_stream_submission)
            }
            RoutedSubmissionV1::Native { .. } => unreachable!(),
        };
        self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Cancelled);
        if self.cooperative_stream_tails.get(&stream) == Some(&submission) {
            match prior {
                Some(prior) => {
                    self.cooperative_stream_tails.insert(stream, prior);
                }
                None => {
                    self.cooperative_stream_tails.remove(&stream);
                }
            }
        }
        Ok(crate::BackendCancellationV1::Cancelled)
    }

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(copy) => {
                if deadline <= Instant::now() || copy.is_quiescent() {
                    return Ok(copy.status());
                }
                None
            }
        };
        if let Some(route) = native_route {
            let result = self.children[route.child].drain_v1(route.local, deadline);
            return self.latch(result);
        }

        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        loop {
            let status = self.progress_cooperative_copy(submission)?;
            if status != BackendPollV1::Pending {
                return Ok(status);
            }
            attempts = attempts.saturating_add(1);
            if !apply_wait_backoff_v1(attempts, &mut sleep, deadline) {
                return Ok(BackendPollV1::Pending);
            }
        }
    }
}

impl Drop for KfdRuntimeBackendV1 {
    fn drop(&mut self) {
        #[cfg(test)]
        if self.scripted_sdma.is_some() && self.scripted_drop_disarmed {
            return;
        }
        #[cfg(test)]
        let scripted_owner_live = self
            .scripted_sdma
            .as_ref()
            .is_some_and(|driver| driver.live_owner_count() != 0);
        #[cfg(not(test))]
        let scripted_owner_live = false;
        if self.terminal
            || scripted_owner_live
            || !self.pending_compute.is_empty()
            || !self.pending_compute_streams.is_empty()
            || !self.allocation_custody.is_empty()
            || !self.compute_module_retain_counts.is_empty()
            || !self.compute_dependency_retain_counts.is_empty()
            || self.any_compute_active_v1()
            || !self.active_sdma.is_empty()
            || !self.active_sdma_streams.is_empty()
            || self.compute_completion_reservations != 0
            || self.sdma_completion_reservations != 0
            || self.terminal_memory.is_some()
            || self.terminal_sdma_custody.is_some()
            || !self.quiescent_sdma_submissions.is_empty()
        {
            // Native custody may still exist, and Drop cannot return it to the
            // caller. Process termination is the fail-closed transition.
            std::process::abort();
        }
        if let Some(mut queue) = self.queue.take() {
            for native_lane in self
                .native_compute_lanes
                .iter()
                .copied()
                .flatten()
                .filter(|lane| lane.ordinal() != 0)
            {
                if queue
                    .destroy_auxiliary_compute_lane_v1(native_lane)
                    .is_err()
                {
                    std::process::abort();
                }
            }
            if queue.destroy().is_err() {
                std::process::abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::kfd_backend_sdma_seam::{
        DirectionalSdmaOpsV1, DirectionalSdmaPairOwnerV1, ScriptedBufferKindV1,
        ScriptedExecutionOutcomeV1, ScriptedFailureModeV1, ScriptedRecycleOutcomeV1,
        ScriptedSdmaStepV1, SdmaTerminalCustodyV1, SdmaTransitionFailureV1,
    };
    use super::*;

    mod synthetic_cov6;

    fn scripted_submit_step_v1(
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        outcome: ScriptedFailureModeV1,
    ) -> ScriptedSdmaStepV1 {
        ScriptedSdmaStepV1::Submit {
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            outcome,
        }
    }

    fn scripted_direct_backend_v1(
        byte_len: usize,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
    ) -> (KfdRuntimeBackendV1, u64, u64, u64) {
        let driver = ScriptedSdmaDriverV1::new(steps);
        let host_owner = driver.test_host_owner(byte_len);
        let device_owner = driver.test_device_owner(byte_len);
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let host = backend.next_id().unwrap();
        let device = backend.next_id().unwrap();
        let record = |kind, storage| AllocationRecordV1 {
            device: 7,
            kind,
            alignment: 8,
            bytes: vec![0; byte_len].into(),
            content_sha256: None,
            last_full_host_write: None,
            native_dirty: Vec::new(),
            sdma_storage: storage,
            sdma_backed: true,
            sdma_initialized: true,
            sdma_shadow_dirty: false,
        };
        backend.allocations.insert(
            host,
            record(
                RuntimeMemoryKindV1::HostVisible,
                KfdRuntimeSdmaStorageV1::Host(host_owner),
            ),
        );
        backend.allocations.insert(
            device,
            record(
                RuntimeMemoryKindV1::DeviceLocal,
                KfdRuntimeSdmaStorageV1::Device(Box::new(device_owner)),
            ),
        );
        backend.staged_context_bytes = (byte_len as u64) * 2;
        backend.native_available = true;
        backend.sdma_enabled = true;
        backend.scripted_sdma = Some(driver);
        (backend, stream, host, device)
    }

    fn scripted_copy_regions_v1(
        host: u64,
        device: u64,
        byte_len: u64,
    ) -> (BackendMemoryRegionV1, BackendMemoryRegionV1) {
        (
            BackendMemoryRegionV1 {
                allocation: host,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len,
            },
            BackendMemoryRegionV1 {
                allocation: device,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len,
            },
        )
    }

    fn scripted_release_steps_v1() -> [ScriptedSdmaStepV1; 3] {
        [
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    }

    fn scripted_sync_copy_steps_v1(
        direction: Gfx942PersistentSdmaDirectionV1,
        offset: u64,
        copy_bytes: u32,
        outcome: ScriptedFailureModeV1,
    ) -> Vec<ScriptedSdmaStepV1> {
        let len = usize::try_from(copy_bytes).unwrap();
        let mut steps = vec![ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: len,
        }];
        if direction == Gfx942PersistentSdmaDirectionV1::HostToDevice {
            steps.push(ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: len,
            });
        }
        steps.push(scripted_submit_step_v1(
            direction, 0, offset, copy_bytes, outcome,
        ));
        if outcome == ScriptedFailureModeV1::Success {
            steps.push(ScriptedSdmaStepV1::Wait(
                ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                },
            ));
            steps.push(ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success));
            if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
                steps.push(ScriptedSdmaStepV1::Read {
                    offset: 0,
                    byte_len: u64::from(copy_bytes),
                });
            }
        }
        steps.push(ScriptedSdmaStepV1::Recycle(
            ScriptedRecycleOutcomeV1::Success,
        ));
        steps
    }

    fn clean_scripted_direct_backend_v1(
        backend: &mut KfdRuntimeBackendV1,
        stream: u64,
        host: u64,
        device: u64,
        submission: Option<u64>,
    ) {
        if let Some(submission) = submission {
            backend.release_submission_v1(submission).unwrap();
        }
        backend.release_allocation_v1(host).unwrap();
        backend
            .allocations
            .get_mut(&device)
            .expect("scripted cleanup device remains indexed")
            .sdma_backed = false;
        backend.release_allocation_v1(device).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
    }

    fn disarm_scripted_drop_after_inspection_v1(backend: &mut KfdRuntimeBackendV1) {
        backend.scripted_drop_disarmed = true;
    }

    #[test]
    fn scripted_sdma_cross_driver_and_mixed_pair_mismatches_retain_without_consuming_fifo() {
        let mut left = ScriptedSdmaDriverV1::new([ScriptedSdmaStepV1::Promote(
            ScriptedFailureModeV1::Success,
        )]);
        let right = ScriptedSdmaDriverV1::new([]);
        let foreign_buffer = right.test_host_owner(8);
        let failure = DirectionalSdmaOpsV1::Scripted(&mut left)
            .promote(foreign_buffer)
            .unwrap_err();
        assert!(matches!(
            &failure,
            SdmaTransitionFailureV1::ProcessTeardown {
                custody: SdmaTerminalCustodyV1::Scripted(_),
                ..
            }
        ));
        assert_eq!(left.remaining_steps(), 1);
        assert_eq!(left.live_owner_count(), 0);
        assert_eq!(right.live_owner_count(), 1);
        assert_eq!(right.unexpected_drops(), 0);

        let mut pair_driver = ScriptedSdmaDriverV1::new([scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        )]);
        let foreign_driver = ScriptedSdmaDriverV1::new([]);
        let host = pair_driver.test_host_owner(8);
        let device = foreign_driver.test_device_owner(8);
        let (device, host) = match (device, host) {
            (DirectionalSdmaDeviceOwnerV1::Scripted(device), SdmaBufferOwnerV1::Scripted(host)) => {
                (device, host)
            }
            _ => unreachable!("scripted factories return scripted owners"),
        };
        let failure = DirectionalSdmaOpsV1::Scripted(&mut pair_driver)
            .submit(
                DirectionalSdmaPairOwnerV1 {
                    device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                    host: SdmaBufferOwnerV1::Scripted(host),
                },
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
            )
            .unwrap_err();
        assert!(matches!(
            &failure,
            SdmaTransitionFailureV1::ProcessTeardown {
                custody: SdmaTerminalCustodyV1::Scripted(_),
                ..
            }
        ));
        assert_eq!(pair_driver.remaining_steps(), 1);
        assert_eq!(pair_driver.live_owner_count(), 1);
        assert_eq!(foreign_driver.live_owner_count(), 1);
        assert_eq!(pair_driver.unexpected_drops(), 0);
        assert_eq!(foreign_driver.unexpected_drops(), 0);
    }

    #[test]
    fn scripted_sdma_drop_still_aborts_with_live_or_terminal_custody() {
        use std::os::unix::process::ExitStatusExt;

        const CHILD: &str = "FE2O3_TEST_SCRIPTED_SDMA_ABORT_CHILD";
        if let Some(case) = std::env::var_os(CHILD) {
            if case == "live" {
                let (backend, _, _, _) = scripted_direct_backend_v1(8, []);
                drop(backend);
                std::process::exit(97);
            }
            let mut backend = KfdRuntimeBackendV1::mock();
            backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([]));
            backend.terminal = true;
            drop(backend);
            std::process::exit(97);
        }
        for case in ["terminal", "live"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "kfd_backend::tests::scripted_sdma_drop_still_aborts_with_live_or_terminal_custody",
                    "--nocapture",
                ])
                .env(CHILD, case)
                .status()
                .unwrap();
            assert_eq!(
                status.signal(),
                Some(6),
                "scripted Drop case {case} did not terminate through SIGABRT"
            );
        }
    }

    #[test]
    fn scripted_sdma_promotion_retry_and_teardown_preserve_exact_custody() {
        let retry_steps = [
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Device,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Retryable),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ];
        let mut retry = KfdRuntimeBackendV1::mock();
        retry.native_available = true;
        retry.scripted_sdma = Some(ScriptedSdmaDriverV1::new(retry_steps));
        assert!(matches!(
            retry.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(retry.allocations.is_empty());
        let driver = retry.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        retry.shutdown_native_v1().unwrap();

        let teardown_steps = [
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Device,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::ProcessTeardown),
        ];
        let mut teardown = KfdRuntimeBackendV1::mock();
        teardown.native_available = true;
        teardown.scripted_sdma = Some(ScriptedSdmaDriverV1::new(teardown_steps));
        assert!(matches!(
            teardown.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(teardown.terminal);
        assert!(teardown.terminal_sdma_custody.is_some());
        let driver = teardown.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 1);
        assert_eq!(driver.unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(&mut teardown);
    }

    #[test]
    fn scripted_sdma_demotion_and_recycle_recovery_are_retryable_without_loss() {
        let steps = [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Retryable),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ];
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        backend.allocations.get_mut(&device).unwrap().sdma_backed = false;
        assert!(matches!(
            backend.release_allocation_v1(device),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::Device(_)
        ));
        assert!(matches!(
            backend.release_allocation_v1(device),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::DemotedDevice(_)
        ));
        backend.release_allocation_v1(device).unwrap();
        backend.release_allocation_v1(host).unwrap_err();
        // The final host recycle was deliberately not scripted: mismatch is
        // terminal and exact host custody is retained instead of disappearing.
        assert!(backend.terminal);
        assert!(backend.terminal_sdma_custody.is_some());
        assert_eq!(
            backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            1
        );
        let _ = stream;
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }

    #[test]
    fn scripted_sdma_initial_submit_retry_is_conclusive_and_releasable() {
        let mut steps = vec![scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Retryable,
        )];
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1
            }
        );
        assert!(!backend.quiescent_sdma_submissions.contains(&submission));
        assert!(backend.active_sdma.is_empty());
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    }

    #[test]
    fn scripted_sdma_poll_pending_retry_and_completion_preserve_facade_owner() {
        let mut steps = vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Retryable),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ];
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert_eq!(backend.active_sdma.len(), 1);
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(backend.active_sdma.is_empty());
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    }

    #[test]
    fn scripted_sdma_partial_continuation_retry_becomes_exact_quiescent_marker() {
        let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        let byte_len = usize::try_from(first).unwrap() + 1;
        let mut steps = vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                first,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                u64::from(first),
                u64::from(first),
                1,
                ScriptedFailureModeV1::Retryable,
            ),
        ];
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        // Observation returned exact custody and did not publish packet two.
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 4);
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(backend.quiescent_sdma_submissions.contains(&submission));
        assert!(matches!(
            backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(backend.active_sdma.is_empty());
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    }

    #[test]
    fn scripted_sdma_dependency_pending_is_observed_without_publication() {
        let mut steps = vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ];
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let dependency = backend.next_id().unwrap();
        let event = backend.next_id().unwrap();
        backend.submissions.insert(
            dependency,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Pending,
            },
        );
        backend.events.insert(
            event,
            EventRecordV1 {
                submission: dependency,
            },
        );
        backend.event_submission_retain_counts.insert(dependency, 1);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[event])
            .unwrap();
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 6);
        backend.submissions.get_mut(&dependency).unwrap().status = BackendPollV1::Succeeded;
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(dependency).unwrap();
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    }

    #[test]
    fn scripted_sdma_metadata_and_retirement_failures_retain_terminal_custody() {
        for steps in [
            vec![
                scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    8,
                    ScriptedFailureModeV1::Success,
                ),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: Some(7),
                }),
            ],
            vec![
                scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    8,
                    ScriptedFailureModeV1::Success,
                ),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                }),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::ProcessTeardown),
            ],
            vec![
                scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    8,
                    ScriptedFailureModeV1::Success,
                ),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
                    copy_bytes: None,
                }),
            ],
        ] {
            let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
            let (source, destination) = scripted_copy_regions_v1(host, device, 8);
            let submission = backend
                .copy_async_v1(stream, source, destination, &[])
                .unwrap();
            assert!(matches!(
                backend.poll_v1(submission),
                Err(RuntimeBackendFailureV1::Terminal(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
            ));
            assert!(backend.terminal);
            assert!(backend.terminal_sdma_custody.is_some());
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert!(driver.is_exhausted());
            assert_eq!(driver.live_owner_count(), 2);
            assert_eq!(driver.unexpected_drops(), 0);
            disarm_scripted_drop_after_inspection_v1(&mut backend);
        }
    }

    #[test]
    fn scripted_sdma_poll_teardown_and_sync_timeout_fail_closed_with_custody() {
        let poll_steps = [
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::ProcessTeardown),
        ];
        let (mut poll_backend, stream, host, device) = scripted_direct_backend_v1(8, poll_steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let submission = poll_backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert!(matches!(
            poll_backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(
            poll_backend
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count(),
            2
        );
        disarm_scripted_drop_after_inspection_v1(&mut poll_backend);

        let sync_steps = [
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Host,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Pending),
        ];
        let (mut sync_backend, _, _, sync_device) = scripted_direct_backend_v1(8, sync_steps);
        assert!(matches!(
            sync_backend.write_allocation_v1(sync_device, 0, &[9; 8]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(sync_backend.terminal_sdma_custody.is_some());
        assert_eq!(
            sync_backend
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count(),
            3
        );
        disarm_scripted_drop_after_inspection_v1(&mut sync_backend);
    }

    #[test]
    fn scripted_sdma_hidden_zero_failure_cleans_unreachable_allocation() {
        let steps = [
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Device,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Promote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Host,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Retryable,
            ),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ];
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.native_available = true;
        backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new(steps));
        assert!(matches!(
            backend.allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(backend.allocations.is_empty());
        assert_eq!(backend.staged_context_bytes, 0);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn scripted_sdma_chunk_n_upload_and_zero_failures_mark_device_shadow_dirty() {
        let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        let byte_len = usize::try_from(first).unwrap() + 1;
        for zero in [false, true] {
            let mut steps = scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                first,
                ScriptedFailureModeV1::Success,
            );
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                u64::from(first),
                1,
                ScriptedFailureModeV1::Retryable,
            ));
            steps.extend(scripted_release_steps_v1());
            let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
            let device_owner = match &mut backend.allocations.get_mut(&device).unwrap().sdma_storage
            {
                KfdRuntimeSdmaStorageV1::Device(device) => device,
                _ => unreachable!("scripted device allocation remains directional"),
            };
            device_owner
                .scripted_bytes_mut()
                .unwrap()
                .fill(if zero { 0xa5 } else { 0 });
            let result = if zero {
                backend.zero_sdma_range_v1(device, byte_len as u64)
            } else {
                backend.upload_sdma_range_v1(device, 0, &vec![0x5a; byte_len])
            };
            assert!(matches!(
                result,
                Err(RuntimeBackendFailureV1::Quiescent(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Native
            ));
            let record = &backend.allocations[&device];
            assert!(record.sdma_shadow_dirty);
            assert!(record.content_sha256.is_none());
            assert!(record.last_full_host_write.is_none());
            let device_bytes = match &record.sdma_storage {
                KfdRuntimeSdmaStorageV1::Device(device) => device.scripted_bytes().unwrap(),
                _ => unreachable!("recovered chunk failure restores device custody"),
            };
            let first = usize::try_from(first).unwrap();
            assert!(
                device_bytes[..first]
                    .iter()
                    .all(|byte| *byte == if zero { 0 } else { 0x5a })
            );
            assert_eq!(device_bytes[first], if zero { 0xa5 } else { 0 });
            clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
        }
    }

    #[test]
    fn scripted_sdma_chunk_n_download_and_shadow_failure_are_quiescent() {
        let first = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        let byte_len = usize::try_from(first).unwrap() + 1;
        for reconcile_shadow in [false, true] {
            let mut steps = scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                0,
                first,
                ScriptedFailureModeV1::Success,
            );
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                u64::from(first),
                1,
                ScriptedFailureModeV1::Retryable,
            ));
            steps.extend(scripted_release_steps_v1());
            let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
            let result = if reconcile_shadow {
                let record = backend.allocations.get_mut(&device).unwrap();
                record.sdma_shadow_dirty = true;
                Arc::make_mut(&mut record.bytes).fill(0xff);
                let result = backend.synchronize_sdma_shadow_v1(device);
                let bytes = &backend.allocations[&device].bytes;
                let first = usize::try_from(first).unwrap();
                assert!(bytes[..first].iter().all(|byte| *byte == 0));
                assert_eq!(bytes[first], 0xff);
                result
            } else {
                let mut destination = vec![0xff; byte_len];
                let result = backend
                    .download_sdma_range_v1(device, 0, &mut destination)
                    .map(|_| ());
                let first = usize::try_from(first).unwrap();
                assert!(destination[..first].iter().all(|byte| *byte == 0));
                assert_eq!(destination[first], 0xff);
                result
            };
            assert!(matches!(
                result,
                Err(RuntimeBackendFailureV1::Quiescent(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Native
            ));
            if reconcile_shadow {
                assert!(backend.allocations[&device].sdma_shadow_dirty);
            }
            clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
        }
    }

    #[test]
    fn scripted_sdma_device_release_executes_scrub_before_demotion_and_recycle() {
        let mut steps = scripted_sync_copy_steps_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            8,
            ScriptedFailureModeV1::Success,
        );
        steps.extend([
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        backend.release_allocation_v1(device).unwrap();
        backend.release_allocation_v1(host).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn scripted_sdma_demotion_and_submit_teardown_retain_exact_runtime_custody() {
        let (mut demotion, _, _, device) = scripted_direct_backend_v1(
            8,
            [ScriptedSdmaStepV1::Demote(
                ScriptedFailureModeV1::ProcessTeardown,
            )],
        );
        demotion.allocations.get_mut(&device).unwrap().sdma_backed = false;
        assert!(matches!(
            demotion.release_allocation_v1(device),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(demotion.terminal_sdma_custody.is_some());
        assert_eq!(
            demotion.scripted_sdma.as_ref().unwrap().live_owner_count(),
            2
        );
        disarm_scripted_drop_after_inspection_v1(&mut demotion);

        let steps = [scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::ProcessTeardown,
        )];
        let (mut submit, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        assert!(matches!(
            submit.copy_async_v1(stream, source, destination, &[]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(submit.terminal_sdma_custody.is_some());
        assert_eq!(submit.scripted_sdma.as_ref().unwrap().live_owner_count(), 2);
        disarm_scripted_drop_after_inspection_v1(&mut submit);
    }

    #[test]
    fn scripted_sdma_host_read_and_ambiguous_recycle_follow_runtime_policy() {
        let mut read_steps = vec![ScriptedSdmaStepV1::Read {
            offset: 0,
            byte_len: 8,
        }];
        read_steps.extend(scripted_release_steps_v1());
        let (mut read_backend, stream, host, device) = scripted_direct_backend_v1(8, read_steps);
        let mut bytes = [0xff; 8];
        read_backend
            .read_allocation_v1(host, 0, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0; 8]);
        clean_scripted_direct_backend_v1(&mut read_backend, stream, host, device, None);

        let steps = [ScriptedSdmaStepV1::Recycle(
            ScriptedRecycleOutcomeV1::Ambiguous,
        )];
        let (mut ambiguous, _, host, _) = scripted_direct_backend_v1(8, steps);
        assert!(matches!(
            ambiguous.release_allocation_v1(host),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(ambiguous.terminal);
        assert_eq!(
            ambiguous.scripted_sdma.as_ref().unwrap().live_owner_count(),
            1
        );
        disarm_scripted_drop_after_inspection_v1(&mut ambiguous);
    }

    fn semantic_geometry_v1() -> crate::RuntimeLaunchGeometryV1 {
        crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        }
    }

    fn atomic_contract_v1() -> RuntimeAtomicLaunchContractV1 {
        RuntimeAtomicLaunchContractV1 {
            operation: RuntimeAtomicOperationV1::Add,
            scope: RuntimeMemoryScopeV1::Workgroup,
            order: RuntimeMemoryOrderV1::Relaxed,
            failure_order: None,
            weak: false,
            geometry: semantic_geometry_v1(),
        }
    }

    fn collective_contract_v1() -> RuntimeCollectiveLaunchContractV1 {
        RuntimeCollectiveLaunchContractV1 {
            operation: crate::RuntimeCollectiveOperationV1::ReduceSum,
            scope: RuntimeMemoryScopeV1::Workgroup,
            order: RuntimeMemoryOrderV1::AcquireRelease,
            participants: 64,
            geometry: semantic_geometry_v1(),
        }
    }

    #[test]
    fn profiler_projection_retains_exact_atomic_and_collective_contracts() {
        let geometry = KfdProfileLaunchV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        };
        let atomic = RuntimeAtomicLaunchContractV1 {
            operation: RuntimeAtomicOperationV1::CompareExchange,
            scope: RuntimeMemoryScopeV1::Device,
            order: RuntimeMemoryOrderV1::SequentiallyConsistent,
            failure_order: Some(RuntimeMemoryOrderV1::Acquire),
            weak: true,
            geometry: semantic_geometry_v1(),
        };
        assert_eq!(
            profile_semantic_contract_v1(KfdRuntimeSemanticLaunchV1::Atomic(atomic), geometry),
            Some(KfdProfileSemanticContractV1::Atomic(
                KfdProfileAtomicContractV1 {
                    operation: KfdProfileAtomicOperationV1::CompareExchange,
                    scope: KfdProfileMemoryScopeV1::Device,
                    order: KfdProfileMemoryOrderV1::SequentiallyConsistent,
                    failure_order: Some(KfdProfileMemoryOrderV1::Acquire),
                    weak: true,
                    geometry,
                }
            ))
        );

        assert_eq!(
            profile_semantic_contract_v1(
                KfdRuntimeSemanticLaunchV1::Collective(collective_contract_v1()),
                geometry,
            ),
            Some(KfdProfileSemanticContractV1::Collective(
                KfdProfileCollectiveContractV1 {
                    operation: KfdProfileCollectiveOperationV1::ReduceSum,
                    scope: KfdProfileMemoryScopeV1::Workgroup,
                    order: KfdProfileMemoryOrderV1::AcquireRelease,
                    participants: 64,
                    geometry,
                }
            ))
        );
        assert_eq!(
            profile_semantic_contract_v1(KfdRuntimeSemanticLaunchV1::Ordinary, geometry),
            None
        );
    }

    #[test]
    fn semantic_profiles_control_both_capability_layers_fail_closed() {
        let overbound =
            KfdRuntimeLaunchGateV1::Semantic(Box::new(TestOverboundSemanticAuthorityV1));
        assert!(!overbound.advertises_atomics_v1());
        assert!(!overbound.supports_atomic_v1(atomic_contract_v1()));

        let panicking =
            KfdRuntimeLaunchGateV1::Semantic(Box::new(TestPanickingSemanticProfileAuthorityV1));
        assert!(!panicking.advertises_atomics_v1());
        assert!(!panicking.advertises_collectives_v1());
        assert!(!panicking.supports_atomic_v1(atomic_contract_v1()));
        assert!(!panicking.supports_collective_v1(collective_contract_v1()));

        let mut ordinary = KfdRuntimeBackendV1::mock();
        assert!(!ordinary.description.capabilities.atomics);
        assert!(!ordinary.description.capabilities.collectives);
        ordinary.native_available = true;
        assert!(!ordinary.execution_capabilities_v1(7).atomics);
        assert!(!ordinary.execution_capabilities_v1(7).collectives);

        let mut semantic = KfdRuntimeBackendV1::mock_with_semantic_authority_v1();
        assert!(semantic.description.capabilities.atomics);
        assert!(semantic.description.capabilities.collectives);
        assert_eq!(
            semantic.execution_capabilities_v1(7),
            RuntimeExecutionCapabilitiesV1::default()
        );
        semantic.native_available = true;
        assert!(semantic.execution_capabilities_v1(7).atomics);
        assert!(semantic.execution_capabilities_v1(7).collectives);
        assert_eq!(
            semantic.execution_capabilities_v1(8),
            RuntimeExecutionCapabilitiesV1::default()
        );
        semantic.native_available = false;
        semantic.shutdown_native_v1().unwrap();
        ordinary.native_available = false;
        ordinary.shutdown_native_v1().unwrap();
    }

    #[test]
    fn semantic_rejections_precede_scheduler_custody_and_handle_allocation() {
        let mut backend = KfdRuntimeBackendV1::mock_with_semantic_authority_v1();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.native_available = true;
        let before_handle = backend.next_handle;
        let unsupported_atomic = RuntimeAtomicLaunchContractV1 {
            operation: RuntimeAtomicOperationV1::Exchange,
            ..atomic_contract_v1()
        };
        let launch = |semantic_launch| BackendLaunchV1 {
            stream,
            kernel: 999,
            explicit_kernarg: &[],
            bindings: &[],
            dependencies: &[],
            geometry: semantic_geometry_v1(),
            semantic_launch,
        };
        assert!(matches!(
            backend.submit_atomic_v1(launch(KfdRuntimeSemanticLaunchV1::Atomic(
                unsupported_atomic,
            ))),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
        let system_atomic = RuntimeAtomicLaunchContractV1 {
            scope: RuntimeMemoryScopeV1::System,
            ..atomic_contract_v1()
        };
        assert!(matches!(
            backend.submit_atomic_v1(launch(KfdRuntimeSemanticLaunchV1::Atomic(system_atomic))),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
        let bad_collective = RuntimeCollectiveLaunchContractV1 {
            participants: 63,
            ..collective_contract_v1()
        };
        assert!(matches!(
            backend.submit_collective_v1(launch(KfdRuntimeSemanticLaunchV1::Collective(
                bad_collective,
            ))),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
        assert!(matches!(
            backend.submit_atomic_v1(launch(KfdRuntimeSemanticLaunchV1::Collective(
                collective_contract_v1(),
            ))),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        assert_eq!(backend.next_handle, before_handle);
        assert!(backend.pending_compute.is_empty());
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.compute_module_retain_counts.is_empty());
        assert_eq!(backend.compute_completion_reservations, 0);
        backend.native_available = false;
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn semantic_contract_is_part_of_recycled_dispatch_identity() {
        let geometry = semantic_geometry_v1();
        let ordinary = BackendLaunchV1 {
            stream: 1,
            kernel: 2,
            explicit_kernarg: &[3],
            bindings: &[],
            dependencies: &[],
            geometry,
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        let atomic = BackendLaunchV1 {
            semantic_launch: KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1()),
            ..ordinary
        };
        let collective = BackendLaunchV1 {
            semantic_launch: KfdRuntimeSemanticLaunchV1::Collective(collective_contract_v1()),
            ..ordinary
        };
        assert_ne!(
            dispatch_shape_sha256_v1(&ordinary, ordinary.semantic_launch),
            dispatch_shape_sha256_v1(&atomic, atomic.semantic_launch)
        );
        assert_ne!(
            dispatch_shape_sha256_v1(&atomic, atomic.semantic_launch),
            dispatch_shape_sha256_v1(&collective, collective.semantic_launch)
        );
    }

    #[test]
    fn later_chunk_rejection_is_quiescent_after_prior_device_publication() {
        let rejected = || {
            RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Native,
                "injected recovered rejection",
            ))
        };
        assert!(matches!(
            classify_sdma_chunk_failure_v1(0, rejected()),
            RuntimeBackendFailureV1::Rejected(_)
        ));
        assert!(matches!(
            classify_sdma_chunk_failure_v1(1, rejected()),
            RuntimeBackendFailureV1::Quiescent(_)
        ));
    }

    fn pending_compute_for_test_v1(
        id: u64,
        stream: u64,
        allocation: u64,
        dependencies: Vec<u64>,
    ) -> PendingComputeSubmissionV1 {
        let dependency_depth = dependencies.len().saturating_add(1);
        PendingComputeSubmissionV1 {
            id,
            module: 9,
            launch: OwnedComputeLaunchV1 {
                stream,
                kernel: 9,
                explicit_kernarg: Box::new([]),
                bindings: vec![BackendBindingV1 {
                    region: BackendMemoryRegionV1 {
                        allocation,
                        access: RuntimeAccessV1::ReadWrite,
                        byte_offset: 0,
                        byte_len: 8,
                    },
                    kernarg_byte_offset: 0,
                }]
                .into_boxed_slice(),
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [1, 1, 1],
                    workgroup: [1, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            },
            retained_allocations: vec![allocation].into_boxed_slice(),
            prior_stream_submission: dependencies.last().copied(),
            dependencies,
            dependency_cursor: 0,
            dependency_depth,
        }
    }

    fn index_pending_compute_custody_for_test_v1(
        backend: &mut KfdRuntimeBackendV1,
        submission: u64,
    ) {
        let pending = &backend.pending_compute[&submission];
        let stream = pending.launch.stream;
        let module = pending.module;
        let allocations = pending.retained_allocations.to_vec();
        let new_entries = backend.reserve_allocation_custody_v1(&allocations).unwrap();
        backend.retain_allocation_custody_v1(
            &allocations,
            RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            },
            new_entries,
        );
        *backend
            .compute_module_retain_counts
            .entry(module)
            .or_insert(0) += 1;
    }

    fn index_sdma_custody_for_test_v1(backend: &mut KfdRuntimeBackendV1, submission: u64) {
        let active = &backend.active_sdma[&submission];
        let stream = active.stream;
        let allocations = [active.source, active.destination];
        let new_entries = backend.reserve_allocation_custody_v1(&allocations).unwrap();
        backend.retain_allocation_custody_v1(
            &allocations,
            RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            },
            new_entries,
        );
        backend.sdma_completion_reservations += 1;
        let new_stream_queue = backend.reserve_active_sdma_stream_v1(stream).unwrap();
        backend.retain_active_sdma_stream_v1(stream, submission, new_stream_queue);
        backend
            .submissions
            .try_reserve(
                backend.compute_completion_reservations + backend.sdma_completion_reservations,
            )
            .unwrap();
    }

    #[test]
    fn capability_inventory_is_fail_closed() {
        let capabilities = kfd_capabilities_v1();
        assert!(capabilities.typed_async_launch);
        assert!(capabilities.streams);
        assert!(capabilities.events);
        assert!(capabilities.device_memory);
        assert!(capabilities.host_visible_memory);
        assert!(!capabilities.peer_copy);
        assert!(!capabilities.multi_device);
        assert!(!capabilities.atomics);
        assert!(!capabilities.collectives);
    }

    #[test]
    fn direct_kfd_compute_sdma_overlap_is_allocation_scoped() {
        let mut custody = HashMap::new();
        for allocation in 1_000..2_000 {
            custody.insert(
                allocation,
                RuntimeAllocationCustodyV1 {
                    owners: VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                        submission: allocation,
                        stream: allocation,
                        kind: RuntimeAllocationCustodyKindV1::Sdma,
                    }]),
                    sole_stream: Some(allocation),
                    owner_counts: [0, 1],
                },
            );
        }
        custody.insert(
            21,
            RuntimeAllocationCustodyV1 {
                owners: VecDeque::from([RuntimeAllocationCustodyOwnerV1 {
                    submission: 50,
                    stream: 5,
                    kind: RuntimeAllocationCustodyKindV1::Sdma,
                }]),
                sole_stream: Some(5),
                owner_counts: [0, 1],
            },
        );
        let disjoint = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 10,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            kernarg_byte_offset: 0,
        }];
        let overlapping = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 21,
                ..disjoint[0].region
            },
            kernarg_byte_offset: 0,
        }];
        let mut publication_lookups = 0;
        assert_eq!(
            indexed_published_sdma_conflict_v1(&disjoint, &custody, 60, 6, |_| {
                publication_lookups += 1;
                true
            },),
            None
        );
        assert_eq!(publication_lookups, 0);
        assert_eq!(
            indexed_published_sdma_conflict_v1(&overlapping, &custody, 60, 6, |submission| {
                publication_lookups += 1;
                submission == 50
            }),
            Some(50)
        );
        assert_eq!(publication_lookups, 1);
    }

    #[test]
    fn direct_kfd_sdma_dependency_depth_is_bounded_before_mutation() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 1, 1)
            .unwrap();
        for allocation in [source, destination] {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        backend.native_available = true;
        backend.active_sdma.insert(
            100,
            ActiveSdmaCopyV1 {
                id: 100,
                stream,
                prior_stream_submission: None,
                source: 1_000,
                destination: 1_001,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 1,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: Vec::new(),
                dependency_cursor: 0,
                dependency_depth: MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        backend
            .events
            .insert(200, EventRecordV1 { submission: 100 });
        let next_handle_before = backend.next_handle;
        let active_before = backend.active_sdma.len();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 1,
        };

        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[200],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert_eq!(backend.next_handle, next_handle_before);
        assert_eq!(backend.active_sdma.len(), active_before);
        assert!(backend.submissions.is_empty());
        assert!(backend.sdma_dependency_retain_counts.is_empty());

        backend.active_sdma.get_mut(&100).unwrap().dependency_depth =
            MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 - 1;
        assert_eq!(
            backend.next_dependency_depth_v1(&[100]),
            Ok(MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
        );
        backend.active_sdma.get_mut(&100).unwrap().dependency_depth = usize::MAX;
        assert_eq!(
            backend.next_dependency_depth_v1(&[100]),
            Err(DirectSdmaDependencyDepthErrorV1::Overflow)
        );

        backend.events.remove(&200);
        backend.active_sdma.remove(&100);
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_sdma_capacity_rejection_precedes_native_reconciliation() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap();
        for allocation in [source, destination] {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        backend
            .allocations
            .get_mut(&source)
            .unwrap()
            .native_dirty
            .push(NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 0,
                allocation_offset: 0,
                data_offset: 0,
                byte_len: 8,
            });
        backend.native_dirty_extents = 1;
        backend.native_available = true;
        backend.next_handle = u64::MAX;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };

        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(!backend.terminal);
        assert_eq!(backend.allocations[&source].native_dirty.len(), 1);
        assert!(backend.active_sdma.is_empty());
        assert!(backend.sdma_dependency_retain_counts.is_empty());

        backend.native_available = false;
        backend.native_dirty_extents = 0;
        backend
            .allocations
            .get_mut(&source)
            .unwrap()
            .native_dirty
            .clear();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_sdma_submit_defers_dirty_native_reconciliation() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap();
        for allocation in [source, destination] {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        backend
            .allocations
            .get_mut(&source)
            .unwrap()
            .native_dirty
            .push(NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 0,
                allocation_offset: 0,
                data_offset: 0,
                byte_len: 8,
            });
        backend.native_dirty_extents = 1;
        backend.native_available = true;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };

        let submission = backend
            .copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        assert!(!backend.terminal);
        assert_eq!(backend.allocations[&source].native_dirty.len(), 1);
        assert!(matches!(
            backend.active_sdma[&submission].phase,
            ActiveDirectionalSdmaPhaseV1::Ready
        ));

        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        backend.release_submission_v1(submission).unwrap();
        backend.native_available = false;
        backend.native_dirty_extents = 0;
        backend
            .allocations
            .get_mut(&source)
            .unwrap()
            .native_dirty
            .clear();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_rebind_requires_synchronizing_detach_for_disjoint_or_new_shape() {
        let prior = ResidentDataDescriptorV1 {
            allocation: 10,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment: 8,
            allocation_offset: 0,
            byte_len: 8,
            host_content_sha256: None,
            device_may_have_modified: true,
        };
        let recycled = RecycledDispatchV1 {
            kernel: 1,
            dispatch_shape_sha256: [7; 32],
            descriptors: vec![prior],
        };
        let data_for = |allocation, kind| DataSpecV1 {
            allocation,
            kind,
            alignment: 8,
            allocation_offset: 0,
            bytes: Arc::from([0_u8; 8]),
            byte_range: 0..8,
            content_sha256: None,
        };

        assert!(recycled_dispatch_reuse_is_admitted_v1(
            &recycled,
            [7; 32],
            &[prior],
            &[data_for(10, RuntimeMemoryKindV1::HostVisible)],
        ));
        let disjoint = ResidentDataDescriptorV1 {
            allocation: 20,
            ..prior
        };
        assert!(!recycled_dispatch_reuse_is_admitted_v1(
            &recycled,
            [7; 32],
            &[disjoint],
            &[data_for(20, RuntimeMemoryKindV1::HostVisible)],
        ));
        assert!(!recycled_dispatch_reuse_is_admitted_v1(
            &recycled,
            [8; 32],
            &[prior],
            &[data_for(10, RuntimeMemoryKindV1::HostVisible)],
        ));
        assert!(!recycled_dispatch_reuse_is_admitted_v1(
            &recycled,
            [7; 32],
            &[prior],
            &[data_for(10, RuntimeMemoryKindV1::DeviceLocal)],
        ));
    }

    #[test]
    fn direct_kfd_sdma_direction_preflight_is_explicit() {
        assert_eq!(
            direct_sdma_direction_v1(
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::DeviceLocal
            ),
            Some(Gfx942PersistentSdmaDirectionV1::HostToDevice)
        );
        assert_eq!(
            direct_sdma_direction_v1(
                RuntimeMemoryKindV1::DeviceLocal,
                RuntimeMemoryKindV1::HostVisible
            ),
            Some(Gfx942PersistentSdmaDirectionV1::DeviceToHost)
        );
        assert_eq!(
            direct_sdma_direction_v1(
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::HostVisible
            ),
            None
        );
        assert_eq!(
            direct_sdma_direction_v1(
                RuntimeMemoryKindV1::DeviceLocal,
                RuntimeMemoryKindV1::DeviceLocal
            ),
            None
        );
    }

    #[test]
    fn direct_kfd_sdma_packet_plan_covers_full_extent_with_exact_offsets() {
        let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        let mut active = ActiveSdmaCopyV1 {
            id: 1,
            stream: 2,
            prior_stream_submission: None,
            source: 3,
            destination: 4,
            source_offset: 11,
            destination_offset: 29,
            byte_len: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
            completed_bytes: 0,
            packet_bytes: 0,
            dependencies: Vec::new(),
            dependency_cursor: 0,
            dependency_depth: 1,
            phase: ActiveDirectionalSdmaPhaseV1::Ready,
        };
        let mut packet_count = 0;
        let mut last_bytes = 0;
        while active.completed_bytes < active.byte_len {
            let h2d =
                direct_sdma_packet_plan_v1(&active, Gfx942PersistentSdmaDirectionV1::HostToDevice)
                    .unwrap();
            let d2h =
                direct_sdma_packet_plan_v1(&active, Gfx942PersistentSdmaDirectionV1::DeviceToHost)
                    .unwrap();
            assert_eq!(h2d.host_offset, 11 + active.completed_bytes);
            assert_eq!(h2d.device_offset, 29 + active.completed_bytes);
            assert_eq!(d2h.host_offset, 29 + active.completed_bytes);
            assert_eq!(d2h.device_offset, 11 + active.completed_bytes);
            assert_eq!(h2d.copy_bytes, d2h.copy_bytes);
            packet_count += 1;
            last_bytes = h2d.copy_bytes;
            active.completed_bytes += u64::from(h2d.copy_bytes);
        }
        assert_eq!(packet_count, 65);
        assert_eq!(last_bytes, 2_048);
        assert_eq!(active.completed_bytes, 256 * 1024 * 1024);
        assert_eq!(cap, 0x003f_ffe0);
    }

    #[test]
    fn direct_kfd_unsupported_copy_direction_is_mutation_free() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        for allocation in [source, destination] {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        backend.native_available = true;
        let next_handle = backend.next_handle;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };
        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
        assert_eq!(backend.next_handle, next_handle);
        assert!(backend.active_sdma.is_empty());
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.sdma_dependency_retain_counts.is_empty());
        assert!(backend.stream_submission_tails.is_empty());
        backend.native_available = false;
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_quiescent_copy_marker_has_no_live_custody() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let dependency = 30;
        let submission = 40;
        backend.active_sdma.insert(
            submission,
            ActiveSdmaCopyV1 {
                id: submission,
                stream,
                prior_stream_submission: None,
                source: 10,
                destination: 20,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 4,
                packet_bytes: 0,
                dependencies: vec![dependency],
                dependency_cursor: 1,
                dependency_depth: 1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, submission);
        backend.sdma_dependency_retain_counts.insert(dependency, 1);
        backend.stream_submission_tails.insert(stream, submission);
        let active = backend.active_sdma.remove(&submission).unwrap();
        backend.fail_quiescent_sdma_copy_v1(active);

        assert!(backend.quiescent_sdma_submissions.contains(&submission));
        assert!(!backend.active_sdma.contains_key(&submission));
        assert!(backend.active_sdma_streams.is_empty());
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.sdma_dependency_retain_counts.is_empty());
        assert_eq!(backend.sdma_completion_reservations, 0);
        assert!(matches!(
            backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(matches!(
            backend.wait_v1(submission, Instant::now() + Duration::from_secs(1)),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            backend.drain_v1(submission, Instant::now() + Duration::from_secs(1)),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        let event = backend.record_event_v1(stream, submission).unwrap();
        assert!(matches!(
            backend.release_submission_v1(submission),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        let dependent = 41;
        backend.active_sdma.insert(
            dependent,
            ActiveSdmaCopyV1 {
                id: dependent,
                stream,
                prior_stream_submission: Some(submission),
                source: 10,
                destination: 20,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: vec![submission],
                dependency_cursor: 0,
                dependency_depth: 2,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, dependent);
        backend.sdma_dependency_retain_counts.insert(submission, 1);
        backend.stream_submission_tails.insert(stream, dependent);
        assert!(matches!(
            backend.poll_v1(dependent),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(backend.quiescent_sdma_submissions.contains(&dependent));
        assert!(!backend.active_sdma.contains_key(&dependent));
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.sdma_dependency_retain_counts.is_empty());
        assert_eq!(backend.sdma_completion_reservations, 0);
        assert!(backend.quiescent_sdma_marker_capacity_is_reserved_v1());
        backend.release_event_v1(event).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        assert!(backend.quiescent_sdma_submissions.contains(&submission));
        assert!(backend.quiescent_sdma_submissions.contains(&dependent));
        assert!(matches!(
            backend.shutdown_native_v1(),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        backend.release_submission_v1(dependent).unwrap();
        backend.release_submission_v1(submission).unwrap();
        assert!(backend.quiescent_sdma_submissions.is_empty());
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_zero_progress_failure_is_conclusive_without_marker() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let submission = 40;
        backend.active_sdma.insert(
            submission,
            ActiveSdmaCopyV1 {
                id: submission,
                stream,
                prior_stream_submission: None,
                source: 10,
                destination: 20,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: Vec::new(),
                dependency_cursor: 0,
                dependency_depth: 1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, submission);
        backend.stream_submission_tails.insert(stream, submission);
        let active = backend.active_sdma.remove(&submission).unwrap();
        assert_eq!(
            backend.fail_unpublished_sdma_copy_v1(active),
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1
            }
        );
        assert!(!backend.quiescent_sdma_submissions.contains(&submission));
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1
            }
        );
        assert!(!backend.active_sdma.contains_key(&submission));
        assert!(backend.active_sdma_streams.is_empty());
        assert!(backend.allocation_custody.is_empty());
        assert_eq!(backend.sdma_completion_reservations, 0);
        backend.release_submission_v1(submission).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_quiescent_marker_capacity_covers_every_reserved_result() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.quiescent_sdma_submissions.insert(10);
        backend.sdma_completion_reservations = 4;
        backend
            .quiescent_sdma_submissions
            .try_reserve(backend.sdma_completion_reservations)
            .unwrap();
        assert!(backend.quiescent_sdma_marker_capacity_is_reserved_v1());

        for submission in 11..15 {
            backend.sdma_completion_reservations -= 1;
            assert!(backend.quiescent_sdma_submissions.insert(submission));
            assert!(backend.quiescent_sdma_marker_capacity_is_reserved_v1());
        }
        assert_eq!(backend.quiescent_sdma_submissions.len(), 5);
        backend.quiescent_sdma_submissions.clear();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_native_copy_requires_initialization_and_scrub_retains_custody() {
        let mut allocation = AllocationRecordV1 {
            device: 7,
            kind: RuntimeMemoryKindV1::DeviceLocal,
            alignment: 8,
            bytes: Arc::from([0_u8; 16]),
            content_sha256: None,
            last_full_host_write: None,
            native_dirty: Vec::new(),
            sdma_storage: KfdRuntimeSdmaStorageV1::Synthetic,
            sdma_backed: true,
            sdma_initialized: false,
            sdma_shadow_dirty: false,
        };
        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 16,
        };
        assert!(!native_sdma_region_is_admitted_v1(
            Some(&allocation),
            7,
            region
        ));
        allocation.sdma_initialized = true;
        assert!(native_sdma_region_is_admitted_v1(
            Some(&allocation),
            7,
            region
        ));
        assert!(!native_sdma_region_is_admitted_v1(
            Some(&allocation),
            8,
            region
        ));
        assert!(!native_sdma_region_is_admitted_v1(
            Some(&allocation),
            7,
            BackendMemoryRegionV1 {
                byte_offset: 1,
                ..region
            }
        ));

        assert!(
            !allocation
                .sdma_storage
                .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
        );
        allocation.sdma_storage =
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(17));
        assert!(
            !allocation
                .sdma_storage
                .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
        );
    }

    fn synthetic_xgmi_submission_v1(
        id: u64,
        stream: u64,
        source: u64,
        destination: u64,
        dependencies: Vec<u64>,
    ) -> XgmiRuntimeSubmissionV1 {
        XgmiRuntimeSubmissionV1 {
            id,
            stream,
            direction: 0,
            source,
            destination,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            dependencies,
            dependency_cursor: 0,
            ticket: None,
        }
    }

    #[test]
    fn unticketed_xgmi_dependency_failure_is_observable_without_publication() {
        let active = synthetic_xgmi_submission_v1(2, 3, 4, 5, vec![1]);
        let mut completed = HashMap::new();
        completed.insert(
            1,
            SubmissionRecordV1 {
                stream: 3,
                status: BackendPollV1::Failed { code: -7 },
            },
        );
        assert!(xgmi_submission_has_failed_dependency_v1(
            &active, &completed
        ));
        assert!(!xgmi_submission_is_ready_v1(&active, &completed, 0));
    }

    #[test]
    fn native_xgmi_pair_and_capability_admission_fail_closed() {
        assert_eq!(admit_xgmi_unique_id_pair_v1(11, 22), Ok(()));
        assert_eq!(
            admit_xgmi_unique_id_pair_v1(0, 22),
            Err(XgmiPairAdmissionErrorV1::ZeroUniqueId)
        );
        assert_eq!(
            admit_xgmi_unique_id_pair_v1(11, 0),
            Err(XgmiPairAdmissionErrorV1::ZeroUniqueId)
        );
        assert_eq!(
            admit_xgmi_unique_id_pair_v1(11, 11),
            Err(XgmiPairAdmissionErrorV1::DuplicateUniqueId)
        );

        fn assert_runtime_extensions<T>()
        where
            T: RuntimeBackendV1
                + RuntimeAsyncCopyBackendV1
                + RuntimeAtomicBackendV1
                + RuntimeCancellationBackendV1
                + RuntimeCollectiveBackendV1
                + RuntimeFlushBackendV1,
        {
        }
        assert_runtime_extensions::<KfdNativeXgmiRuntimeBackendV1>();
        let capabilities = native_xgmi_execution_capabilities_v1();
        assert!(capabilities.native_peer_copy);
        assert!(capabilities.cancellation);
        assert!(!capabilities.native_async_copy);
        assert!(!capabilities.concurrent_compute);
        assert!(!capabilities.compute_copy_overlap);
        assert!(!capabilities.memory_pool);
        assert!(!capabilities.profiling);
        assert!(!capabilities.atomics);
        assert!(!capabilities.collectives);

        for failure in [
            reject_native_xgmi_semantic_submission_v1(
                BackendSemanticLaunchV1::Atomic(atomic_contract_v1()),
                true,
            ),
            reject_native_xgmi_semantic_submission_v1(
                BackendSemanticLaunchV1::Collective(collective_contract_v1()),
                false,
            ),
        ] {
            let RuntimeBackendFailureV1::Rejected(error) = failure else {
                panic!("unsupported native XGMI semantics must reject before custody");
            };
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Unsupported);
        }
        let RuntimeBackendFailureV1::Rejected(error) = reject_native_xgmi_semantic_submission_v1(
            BackendSemanticLaunchV1::Collective(collective_contract_v1()),
            true,
        ) else {
            panic!("mismatched native XGMI semantic variant must reject");
        };
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    }

    #[test]
    fn native_xgmi_batch_selection_is_ready_directional_bounded_and_ordered() {
        let mut active = HashMap::new();
        active.insert(9, synthetic_xgmi_submission_v1(9, 1, 10, 11, vec![]));
        active.insert(3, synthetic_xgmi_submission_v1(3, 2, 12, 13, vec![70]));
        active.insert(5, synthetic_xgmi_submission_v1(5, 3, 14, 15, vec![71]));
        let mut reverse = synthetic_xgmi_submission_v1(4, 4, 16, 17, vec![]);
        reverse.direction = 1;
        active.insert(4, reverse);

        let mut completed = HashMap::new();
        completed.insert(
            70,
            SubmissionRecordV1 {
                stream: 8,
                status: BackendPollV1::Succeeded,
            },
        );
        completed.insert(
            71,
            SubmissionRecordV1 {
                stream: 8,
                status: BackendPollV1::Pending,
            },
        );

        assert_eq!(
            ready_xgmi_batch_ids_v1(&active, &completed, 0, 8).unwrap(),
            vec![3, 9]
        );
        assert_eq!(
            ready_xgmi_batch_ids_v1(&active, &completed, 0, 1).unwrap(),
            vec![3]
        );
        assert_eq!(
            ready_xgmi_batch_ids_v1(&active, &completed, 1, 8).unwrap(),
            vec![4]
        );
        assert!(
            ready_xgmi_batch_ids_v1(&active, &completed, 2, 8)
                .unwrap()
                .is_empty()
        );

        let mut oversized = HashMap::new();
        for id in 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 2 {
            oversized.insert(
                id,
                synthetic_xgmi_submission_v1(id, id, id * 2, id * 2 + 1, vec![]),
            );
        }
        let admitted =
            ready_xgmi_batch_ids_v1(&oversized, &HashMap::new(), 0, GFX942_SDMA_MAX_IN_FLIGHT_V1)
                .unwrap();
        assert_eq!(admitted.len(), GFX942_SDMA_MAX_IN_FLIGHT_V1);
        assert_eq!(admitted[0], 1);
        assert_eq!(
            admitted[GFX942_SDMA_MAX_IN_FLIGHT_V1 - 1],
            GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64
        );

        // A caller focused beyond the first admitted ring batch advances one
        // published ticket per poll instead of waiting on an unrelated handle
        // forever. Once that batch drains, the focus can enter the next batch.
        let focus = GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 2;
        for completed in 0..GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 {
            let in_flight =
                (completed + 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64).collect::<Vec<_>>();
            assert_eq!(
                indexed_xgmi_progress_id_v1(&in_flight, focus),
                Some(completed + 1)
            );
        }
        assert_eq!(indexed_xgmi_progress_id_v1(&[], focus), None);
    }

    #[test]
    fn native_xgmi_recoverable_batch_failure_settles_every_logical_owner() {
        let first = synthetic_xgmi_submission_v1(40, 4, 10, 11, vec![70]);
        let second = synthetic_xgmi_submission_v1(41, 5, 12, 13, vec![70, 71]);
        let mut dependency_retains = HashMap::from([(70, 2), (71, 1)]);
        let mut submissions = HashMap::new();
        let mut completion_reservations = 0;
        reserve_xgmi_completion_slot_v1(&mut submissions, &mut completion_reservations).unwrap();
        reserve_xgmi_completion_slot_v1(&mut submissions, &mut completion_reservations).unwrap();
        let completion_capacity = submissions.capacity();

        finish_failed_xgmi_batch_records_v1(
            &mut dependency_retains,
            &mut submissions,
            &mut completion_reservations,
            [first, second],
        );

        assert!(dependency_retains.is_empty());
        assert_eq!(completion_reservations, 0);
        assert_eq!(submissions.capacity(), completion_capacity);
        let first = submissions.get(&40).expect("first failure record");
        assert_eq!(first.stream, 4);
        assert_eq!(
            first.status,
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            }
        );
        let second = submissions.get(&41).expect("second failure record");
        assert_eq!(second.stream, 5);
        assert_eq!(
            second.status,
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            }
        );
    }

    #[test]
    fn native_xgmi_completion_slots_cover_every_outstanding_submission() {
        const OUTSTANDING: u64 = 1_024;

        let mut submissions = HashMap::new();
        let mut completion_reservations = 0;
        let active = (1..=OUTSTANDING)
            .map(|id| synthetic_xgmi_submission_v1(id, id, id * 2, id * 2 + 1, vec![]))
            .collect::<Vec<_>>();
        for expected in 1..=OUTSTANDING as usize {
            reserve_xgmi_completion_slot_v1(&mut submissions, &mut completion_reservations)
                .unwrap();
            assert_eq!(completion_reservations, expected);
            assert!(
                submissions.capacity().saturating_sub(submissions.len()) >= completion_reservations
            );
        }

        let reserved_capacity = submissions.capacity();
        let mut dependency_retains = HashMap::new();
        for (settled, active) in active.into_iter().enumerate() {
            settle_xgmi_submission_record_v1(
                &mut dependency_retains,
                &mut submissions,
                &mut completion_reservations,
                active,
                BackendPollV1::Succeeded,
            );
            assert_eq!(completion_reservations, OUTSTANDING as usize - settled - 1);
            assert_eq!(submissions.capacity(), reserved_capacity);
            assert!(
                submissions.capacity().saturating_sub(submissions.len()) >= completion_reservations
            );
        }
        assert_eq!(submissions.len(), OUTSTANDING as usize);
        assert_eq!(completion_reservations, 0);
    }

    #[test]
    fn native_xgmi_ready_and_in_flight_indexes_remain_bounded_under_stress() {
        const READY: u64 = 16_384;

        let mut ready = VecDeque::new();
        ready.try_reserve_exact(READY as usize).unwrap();
        for id in 1..=READY {
            enqueue_xgmi_ready_id_v1(&mut ready, id);
        }
        assert!(remove_xgmi_ready_id_v1(&mut ready, READY / 2));
        enqueue_xgmi_ready_id_v1(&mut ready, READY / 2);

        let mut observed = 0;
        while !ready.is_empty() {
            let batch_len = ready.len().min(GFX942_SDMA_MAX_IN_FLIGHT_V1);
            let mut in_flight = Vec::new();
            in_flight.try_reserve_exact(batch_len).unwrap();
            for _ in 0..batch_len {
                let id = ready.pop_front().unwrap();
                insert_ordered_xgmi_id_v1(&mut in_flight, id);
            }
            assert!(in_flight.len() <= GFX942_SDMA_MAX_IN_FLIGHT_V1);
            let focus = READY + 1;
            while let Some(id) = indexed_xgmi_progress_id_v1(&in_flight, focus) {
                assert!(remove_ordered_xgmi_id_v1(&mut in_flight, id));
                observed += 1;
            }
        }
        assert_eq!(observed, READY);
    }

    #[test]
    fn native_xgmi_completed_ticket_bypasses_a_large_ready_backlog() {
        const READY: u64 = 131_072;
        const COMPLETED: u64 = READY / 2;

        let mut ready = VecDeque::new();
        ready.try_reserve_exact(READY as usize).unwrap();
        ready.extend(1..=READY);
        let expected_ready = ready.clone();
        let mut in_flight = Vec::with_capacity(GFX942_SDMA_MAX_IN_FLIGHT_V1);
        in_flight.push(COMPLETED);

        // Mirroring the marker in the hostile test backlog makes index-order
        // observable: an implementation that searches ready first removes it.
        assert_eq!(
            remove_xgmi_progress_index_v1(&mut ready, &mut in_flight, COMPLETED),
            XgmiProgressIndexPhaseV1::InFlight
        );
        assert!(in_flight.is_empty());
        assert_eq!(ready, expected_ready);
    }

    #[test]
    fn native_xgmi_recoverable_prefix_restoration_preserves_fifo_order() {
        let mut ready = VecDeque::new();
        ready.try_reserve_exact(8).unwrap();
        ready.extend([40, 50, 60]);

        for id in [10, 20, 30].into_iter().rev() {
            prepend_xgmi_ready_id_v1(&mut ready, id);
        }

        assert_eq!(
            ready.into_iter().collect::<Vec<_>>(),
            [10, 20, 30, 40, 50, 60]
        );
    }

    #[test]
    fn native_xgmi_partial_reverse_restoration_keeps_each_restored_owner_indexed() {
        let mut ready = VecDeque::new();
        ready.try_reserve_exact(8).unwrap();
        ready.extend([40, 50, 60]);

        // Reverse restoration has completed owners 30 and 20 when restoring
        // owner 10 fails. Both completed owners remain a FIFO prefix.
        prepend_xgmi_ready_id_v1(&mut ready, 30);
        prepend_xgmi_ready_id_v1(&mut ready, 20);

        assert_eq!(ready.into_iter().collect::<Vec<_>>(), [20, 30, 40, 50, 60]);
    }

    #[test]
    fn native_xgmi_flush_admission_is_complete_bounded_and_nonmutating() {
        assert_eq!(xgmi_direction_for_destination_v1(0), Some(1));
        assert_eq!(xgmi_direction_for_destination_v1(1), Some(0));
        assert_eq!(xgmi_direction_for_destination_v1(2), None);
        assert_eq!(
            classify_xgmi_flush_v1(0, false, GFX942_SDMA_MAX_IN_FLIGHT_V1),
            XgmiFlushAdmissionV1::NoReadyWork
        );
        assert_eq!(
            classify_xgmi_flush_v1(1, true, GFX942_SDMA_MAX_IN_FLIGHT_V1),
            XgmiFlushAdmissionV1::InFlight
        );

        let mut active = HashMap::new();
        for id in 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 1 {
            active.insert(
                id,
                synthetic_xgmi_submission_v1(id, id, id * 2, id * 2 + 1, vec![]),
            );
        }
        let before: Vec<_> = {
            let mut ids: Vec<_> = active.keys().copied().collect();
            ids.sort_unstable();
            ids
        };
        assert_eq!(
            classify_xgmi_flush_v1(active.len(), false, GFX942_SDMA_MAX_IN_FLIGHT_V1),
            XgmiFlushAdmissionV1::Publish {
                ready: GFX942_SDMA_MAX_IN_FLIGHT_V1,
            }
        );
        let mut prefix_progress = XgmiFlushPrefixProgressV1::new(active.len());
        let mut bounded_prefixes = Vec::new();
        while prefix_progress.remaining_at_entry != 0 {
            let published = prefix_progress.next_batch_len();
            bounded_prefixes.push(published);
            prefix_progress.note_published(published);
            if prefix_progress.remaining_at_entry != 0 {
                prefix_progress.note_completed_prefix();
            }
        }
        assert_eq!(bounded_prefixes, [GFX942_SDMA_MAX_IN_FLIGHT_V1, 1]);

        let mut second_prefix_failure =
            XgmiFlushPrefixProgressV1::new(GFX942_SDMA_MAX_IN_FLIGHT_V1 + 1);
        second_prefix_failure.note_published(GFX942_SDMA_MAX_IN_FLIGHT_V1);
        second_prefix_failure.note_completed_prefix();
        let retained_state = second_prefix_failure;
        assert!(matches!(
            second_prefix_failure.classify_publication_failure(
                RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "injected second-prefix allocation failure",
                ))
            ),
            RuntimeBackendFailureV1::Quiescent(error)
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
                    && error.detail() == "injected second-prefix allocation failure"
        ));
        assert_eq!(
            retained_state,
            XgmiFlushPrefixProgressV1 {
                remaining_at_entry: 1,
                completed_prefixes: 1,
            }
        );
        assert!(matches!(
            XgmiFlushPrefixProgressV1::new(1).classify_publication_failure(
                RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "injected first-prefix allocation failure",
                ))
            ),
            RuntimeBackendFailureV1::Rejected(_)
        ));
        let mut after: Vec<_> = active.keys().copied().collect();
        after.sort_unstable();
        assert_eq!(after, before);
        assert!(
            active
                .values()
                .all(|submission| submission.ticket.is_none())
        );
        assert_eq!(
            classify_xgmi_flush_v1(1, false, 0),
            XgmiFlushAdmissionV1::Capacity
        );

        active.remove(&(GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 1));
        assert_eq!(
            classify_xgmi_flush_v1(active.len(), false, GFX942_SDMA_MAX_IN_FLIGHT_V1),
            XgmiFlushAdmissionV1::Publish {
                ready: GFX942_SDMA_MAX_IN_FLIGHT_V1,
            }
        );
        assert_eq!(
            ready_xgmi_batch_ids_v1(&active, &HashMap::new(), 0, GFX942_SDMA_MAX_IN_FLIGHT_V1,)
                .unwrap(),
            (1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64).collect::<Vec<_>>()
        );
    }

    #[test]
    fn native_xgmi_peer_admission_binds_direction_and_rejects_hostile_ranges() {
        let forward = XgmiPeerCopyAdmissionV1 {
            stream_device: 1,
            source_device: 0,
            destination_device: 1,
            source_offset: 8,
            source_len: 16,
            source_allocation_len: 32,
            source_access: RuntimeAccessV1::Read,
            destination_offset: 4,
            destination_len: 16,
            destination_allocation_len: 32,
            destination_access: RuntimeAccessV1::Write,
        };
        assert_eq!(admit_xgmi_peer_copy_v1(forward), Ok(0));
        assert_eq!(
            admit_xgmi_peer_copy_v1(XgmiPeerCopyAdmissionV1 {
                stream_device: 0,
                source_device: 1,
                destination_device: 0,
                ..forward
            }),
            Ok(1)
        );

        let mutations = [
            (
                XgmiPeerCopyAdmissionV1 {
                    source_device: 2,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::UnknownDevice,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    destination_device: 0,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::SameDevice,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    stream_device: 0,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::WrongDestinationStream,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    source_len: 0,
                    destination_len: 0,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::ZeroLength,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    destination_len: 15,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::LengthMismatch,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    source_len: u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) + 1,
                    destination_len: u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) + 1,
                    source_allocation_len: u64::MAX,
                    destination_allocation_len: u64::MAX,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::PacketTooLarge,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    source_offset: u64::MAX,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::SourceRange,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    destination_offset: 17,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::DestinationRange,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    source_access: RuntimeAccessV1::Write,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::SourceAccess,
            ),
            (
                XgmiPeerCopyAdmissionV1 {
                    destination_access: RuntimeAccessV1::Read,
                    ..forward
                },
                XgmiPeerCopyAdmissionErrorV1::DestinationAccess,
            ),
        ];
        for (request, expected) in mutations {
            assert_eq!(admit_xgmi_peer_copy_v1(request), Err(expected));
        }
    }

    #[test]
    fn native_xgmi_dependency_and_pending_ownership_rules_are_bounded() {
        let events = HashMap::from([
            (10, EventRecordV1 { submission: 100 }),
            (11, EventRecordV1 { submission: 101 }),
            (12, EventRecordV1 { submission: 100 }),
        ]);
        assert_eq!(
            collect_xgmi_dependencies_v1(&events, &[10, 11]),
            Ok(vec![100, 101])
        );
        assert_eq!(
            collect_xgmi_dependencies_v1(&events, &[99]),
            Err(XgmiDependencyAdmissionErrorV1::Unknown)
        );
        assert_eq!(
            collect_xgmi_dependencies_v1(&events, &[10, 12]),
            Err(XgmiDependencyAdmissionErrorV1::Duplicate)
        );
        assert_eq!(
            collect_xgmi_dependencies_v1(&events, &vec![10; MAX_RUNTIME_DEPENDENCIES_V1 + 1]),
            Err(XgmiDependencyAdmissionErrorV1::TooMany)
        );

        let active = synthetic_xgmi_submission_v1(100, 7, 20, 21, Vec::new());
        assert!(xgmi_allocation_is_active_v1([&active].into_iter(), 20));
        assert!(xgmi_allocation_is_active_v1([&active].into_iter(), 21));
        assert!(!xgmi_allocation_is_active_v1([&active].into_iter(), 22));
        assert!(has_active_xgmi_stream_v1([&active].into_iter(), 7));
        assert!(!has_active_xgmi_stream_v1([&active].into_iter(), 8));
        assert!(has_unordered_xgmi_overlap_v1(
            [&active].into_iter(),
            22,
            20,
            &[]
        ));
        assert!(!has_unordered_xgmi_overlap_v1(
            [&active].into_iter(),
            22,
            20,
            &[100]
        ));

        let mut depths = HashMap::from([(100, 1), (101, 255)]);
        assert_eq!(next_xgmi_dependency_depth_v1(&depths, &[100]), Ok(2));
        assert_eq!(next_xgmi_dependency_depth_v1(&depths, &[101]), Ok(256));
        depths.insert(102, 256);
        assert_eq!(
            next_xgmi_dependency_depth_v1(&depths, &[102]),
            Err(XgmiDependencyAdmissionErrorV1::TooMany)
        );
        assert_eq!(
            next_xgmi_dependency_depth_v1(&depths, &[999]),
            Err(XgmiDependencyAdmissionErrorV1::Unknown)
        );
    }

    #[test]
    fn native_xgmi_cancellation_and_shutdown_preserve_phase_custody() {
        assert_eq!(
            xgmi_cancellation_disposition_v1(Some(false), false),
            XgmiCancellationDispositionV1::CancelPrepublication
        );
        assert_eq!(
            xgmi_cancellation_disposition_v1(Some(true), false),
            XgmiCancellationDispositionV1::TooLate
        );
        assert_eq!(
            xgmi_cancellation_disposition_v1(None, true),
            XgmiCancellationDispositionV1::TooLate
        );
        assert_eq!(
            xgmi_cancellation_disposition_v1(None, false),
            XgmiCancellationDispositionV1::Unknown
        );

        assert!(XgmiLogicalResourceCountsV1::default().permits_shutdown());
        for occupied in 0..15 {
            let mut resources = XgmiLogicalResourceCountsV1::default();
            match occupied {
                0 => resources.streams = 1,
                1 => resources.allocations = 1,
                2 => resources.submissions = 1,
                3 => resources.active = 1,
                4 => resources.events = 1,
                5 => resources.event_retains = 1,
                6 => resources.dependency_retains = 1,
                7 => resources.dependency_depths = 1,
                8 => resources.dependency_waiters = 1,
                9 => resources.completion_reservations = 1,
                10 => resources.ready_index_entries = 1,
                11 => resources.in_flight_index_entries = 1,
                12 => resources.directional_active = 1,
                13 => resources.stream_owners = 1,
                14 => resources.allocation_owners = 1,
                _ => unreachable!(),
            }
            assert!(!resources.permits_shutdown());
        }
    }

    #[test]
    fn staged_allocations_are_bounded_and_round_trip() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        backend
            .write_allocation_v1(allocation, 4, &[1, 2, 3])
            .unwrap();
        let mut bytes = [0_u8; 5];
        backend
            .read_allocation_v1(allocation, 2, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0, 0, 1, 2, 3]);
        assert!(matches!(
            backend.write_allocation_v1(allocation, 15, &[1, 2]),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
    }

    #[test]
    fn profiler_records_complete_address_free_runtime_lifecycle() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([11; 32], 32).unwrap())
            .unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        backend
            .write_allocation_v1(allocation, 4, &[1, 2, 3])
            .unwrap();
        let mut readback = [0_u8; 3];
        backend
            .read_allocation_v1(allocation, 4, &mut readback)
            .unwrap();
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        backend.unload_module_v1(module).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
        let capture = backend.finish_profiler_v1().unwrap();
        capture.validate().unwrap();
        assert_eq!(
            capture.host_content_mode,
            fe2o3_profiler_protocol::KfdProfileHostContentModeV1::RangeOnly
        );
        assert!(capture.coverage.complete_runtime_operation_history);
        assert_eq!(capture.coverage.dropped_events, 0);
        assert!(
            capture
                .events
                .iter()
                .any(|event| matches!(event.event, KfdRuntimeProfileEventKindV1::HostWrite { .. }))
        );
        assert!(
            capture
                .events
                .iter()
                .any(|event| matches!(event.event, KfdRuntimeProfileEventKindV1::HostRead { .. }))
        );
        let encoded = fe2o3_profiler_protocol::encode_kfd_runtime_profile_v1(&capture).unwrap();
        let encoded = String::from_utf8(encoded).unwrap();
        assert!(!encoded.contains("backend_handle"));
        assert!(!encoded.contains("device_address"));
        assert!(!encoded.contains("queue_id"));
    }

    #[test]
    fn profiler_content_identity_mode_is_explicit_in_every_host_record() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(
                KfdRuntimeProfilerConfigV1::new([15; 32], 16)
                    .unwrap()
                    .with_host_content_identities(),
            )
            .unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.write_allocation_v1(allocation, 0, &[1; 8]).unwrap();
        let mut readback = [0; 8];
        backend
            .read_allocation_v1(allocation, 0, &mut readback)
            .unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.shutdown_native_v1().unwrap();
        let capture = backend.finish_profiler_v1().unwrap();
        assert_eq!(
            capture.host_content_mode,
            fe2o3_profiler_protocol::KfdProfileHostContentModeV1::ContentIdentity
        );
        let host_records: Vec<_> = capture
            .events
            .iter()
            .filter_map(|event| match &event.event {
                KfdRuntimeProfileEventKindV1::HostWrite { content, .. }
                | KfdRuntimeProfileEventKindV1::HostRead { content, .. } => Some(*content),
                _ => None,
            })
            .collect();
        assert_eq!(host_records.len(), 2);
        assert!(host_records.iter().all(|content| matches!(
            content,
            KfdProfileHostContentV1::ContentIdentity { content }
                if content.byte_len == 8
        )));
    }

    #[test]
    fn profiler_timestamp_retrieval_requires_cleanup_and_preserves_runtime_custody() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([24; 32], 16).unwrap())
            .unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        assert!(matches!(
            backend.finish_profiler_with_dispatch_timestamps_v1(),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
        let evidence = backend
            .finish_profiler_with_dispatch_timestamps_v1()
            .unwrap();
        assert!(
            evidence
                .runtime_profile()
                .coverage
                .complete_runtime_operation_history
        );
        assert!(
            evidence
                .dispatch_timestamps()
                .coverage()
                .complete_runtime_operation_history
        );
        assert!(evidence.dispatch_timestamps().records().is_empty());
    }

    #[test]
    fn semantic_timestamp_v2_requires_explicit_sidecar_enablement() {
        let mut ordinary = KfdRuntimeBackendV1::mock();
        ordinary
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([25; 32], 16).unwrap())
            .unwrap();
        ordinary.shutdown_native_v1().unwrap();
        assert!(matches!(
            ordinary.finish_profiler_with_dispatch_timestamps_v2(),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        let ordinary_evidence = ordinary
            .finish_profiler_with_dispatch_timestamps_v1()
            .unwrap();
        assert!(ordinary_evidence.runtime_profile().events.is_empty());
        assert!(ordinary_evidence.dispatch_timestamps().records().is_empty());

        let mut semantic = KfdRuntimeBackendV1::mock();
        semantic
            .enable_profiler_with_semantic_profile_v1(
                KfdRuntimeProfilerConfigV1::new([26; 32], 16).unwrap(),
            )
            .unwrap();
        semantic.shutdown_native_v1().unwrap();
        let evidence = semantic
            .finish_profiler_with_dispatch_timestamps_v2()
            .unwrap();
        assert!(evidence.runtime_profile().events.is_empty());
        assert!(evidence.dispatch_timestamps().records().is_empty());
        assert!(evidence.semantic_profile().records().is_empty());
        assert!(
            evidence
                .semantic_profile()
                .coverage()
                .complete_retained_dispatch_classification
        );
    }

    #[test]
    fn semantic_sidecar_finish_rejection_preserves_ordinary_v1_profiler() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([27; 32], 16).unwrap())
            .unwrap();
        backend.shutdown_native_v1().unwrap();
        assert!(matches!(
            backend.finish_profiler_with_semantic_profile_v1(),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        let capture = backend.finish_profiler_v1().unwrap();
        assert!(capture.events.is_empty());
        assert!(capture.coverage.complete_runtime_operation_history);
    }

    #[test]
    fn profiler_loss_is_bounded_and_freezes_a_valid_prefix() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([12; 32], 2).unwrap())
            .unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.write_allocation_v1(allocation, 0, &[1; 8]).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
        let capture = backend.finish_profiler_v1().unwrap();
        capture.validate().unwrap();
        assert_eq!(capture.events.len(), 2);
        assert_eq!(capture.coverage.dropped_events, 3);
        assert!(!capture.coverage.complete_runtime_operation_history);
    }

    #[test]
    fn profiler_enable_rejects_a_logically_clean_but_used_backend() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        assert!(matches!(
            backend.enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([13; 32], 8).unwrap()),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
    }

    #[test]
    fn profiler_enable_rejects_a_shutdown_backend() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.shutdown_native_v1().unwrap();
        assert!(matches!(
            backend.enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([14; 32], 8).unwrap()),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
    }

    #[test]
    fn complete_writes_cache_content_evidence_and_partial_writes_invalidate_it() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let complete = [1_u8, 2, 3, 4, 5, 6, 7, 8];
        backend
            .write_allocation_v1(allocation, 0, &complete)
            .unwrap();
        assert_eq!(
            backend.allocations[&allocation].content_sha256,
            Some(Sha256::digest(complete).into())
        );
        let first_image = Arc::clone(&backend.allocations[&allocation].bytes);
        backend
            .write_allocation_v1(allocation, 0, &complete)
            .unwrap();
        assert!(Arc::ptr_eq(
            &first_image,
            &backend.allocations[&allocation].bytes
        ));

        let full = snapshot_bound_data_v1(
            &backend.allocations,
            &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 8,
                },
                kernarg_byte_offset: 0,
            }],
            7,
        )
        .unwrap();
        assert_eq!(
            full.data[0].content_sha256,
            backend.allocations[&allocation].content_sha256
        );

        backend.write_allocation_v1(allocation, 3, &[9]).unwrap();
        assert_eq!(backend.allocations[&allocation].content_sha256, None);
    }

    #[test]
    fn staging_budgets_reject_before_allocation_and_release_exact_accounting() {
        let mut backend = KfdRuntimeBackendV1::mock_with_staging_budgets(StagingBudgetsV1 {
            max_allocation_bytes: 8,
            max_context_bytes: 12,
        });
        let first = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let second = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 4)
            .unwrap();
        assert_eq!(backend.staged_context_bytes, 12);
        assert!(matches!(
            backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(matches!(
            backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 9, 1),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        backend.release_allocation_v1(first).unwrap();
        assert_eq!(backend.staged_context_bytes, 4);
        let replacement = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        assert_eq!(backend.staged_context_bytes, 12);
        backend.release_allocation_v1(second).unwrap();
        backend.release_allocation_v1(replacement).unwrap();
    }

    #[test]
    fn staged_allocation_capacity_failure_is_fallible() {
        assert!(matches!(
            try_zeroed_staging_v1(usize::MAX),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
    }

    #[test]
    fn launch_snapshot_copies_only_the_alignment_preserving_bound_window() {
        let bytes = (0_u8..64).collect::<Vec<_>>();
        let mut allocations = HashMap::new();
        allocations.insert(
            9,
            AllocationRecordV1 {
                device: 7,
                kind: RuntimeMemoryKindV1::HostVisible,
                alignment: 8,
                bytes: bytes.into(),
                content_sha256: None,
                last_full_host_write: None,
                native_dirty: Vec::new(),
                sdma_storage: KfdRuntimeSdmaStorageV1::Synthetic,
                sdma_backed: false,
                sdma_initialized: false,
                sdma_shadow_dirty: false,
            },
        );
        let bindings = [
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: 9,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 19,
                    byte_len: 4,
                },
                kernarg_byte_offset: 0,
            },
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: 9,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 40,
                    byte_len: 4,
                },
                kernarg_byte_offset: 8,
            },
        ];

        let staged = snapshot_bound_data_v1(&allocations, &bindings, 7).unwrap();
        assert_eq!(staged.data.len(), 1);
        assert_eq!(staged.data[0].allocation_offset, 16);
        assert_eq!(staged.data[0].content_sha256, None);
        assert_eq!(staged.data[0].bytes(), &allocations[&9].bytes[16..44]);
        assert_eq!(
            staged.placements[&9],
            StagedPlacementV1 {
                data_index: 0,
                allocation_offset: 16,
            }
        );
        assert!(staged.data[0].bytes().len() < allocations[&9].bytes.len());
    }

    #[test]
    fn valid_cov6_module_reaches_cached_launch_and_native_acquisition_boundary() {
        let image = synthetic_cov6::module();
        let mut backend = KfdRuntimeBackendV1::mock_with_semantic_authority_v1();
        let stream = backend.create_stream_v1(7).unwrap();
        let module = backend.load_module_v1(7, &image).unwrap();
        assert_eq!(backend.modules[&module].validated.validation_passes(), 1);
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        assert_eq!(
            backend.kernels[&kernel].validated.semantic_binding_passes(),
            1
        );
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
            .unwrap();
        let initial = (0_u8..64).collect::<Vec<_>>();
        backend
            .write_allocation_v1(allocation, 0, &initial)
            .unwrap();

        let mut explicit_kernarg = [0_u8; 16];
        explicit_kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
        let bindings = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Read,
                byte_offset: 11,
                byte_len: 13,
            },
            kernarg_byte_offset: 0,
        }];
        let geometry = crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        };
        let prepared = backend
            .prepare_launch(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &bindings,
                dependencies: &[],
                geometry,
                semantic_launch: KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1()),
            })
            .unwrap();
        assert_eq!(prepared.data.len(), 1);
        assert_eq!(prepared.data[0].allocation_offset, 8);
        assert_eq!(prepared.data[0].bytes(), &initial[8..24]);
        let reconciled =
            build_program_v1(&prepared.program, prepared.signature, &prepared.abi_rows).unwrap();
        assert!(reconciled.dispatch_abi_identity().is_some());
        drop(reconciled);
        drop(prepared);

        assert!(matches!(
            backend.submit_v1(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &bindings,
                dependencies: &[],
                geometry,
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            }),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
                    && error.detail() == "the admitted KFD queue lifecycle has already retired"
        ));

        backend.release_allocation_v1(allocation).unwrap();
        backend.unload_module_v1(module).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn launch_authority_panic_fails_before_publication_and_releases_custody() {
        let image = synthetic_cov6::module();
        let mut backend = KfdRuntimeBackendV1::mock_with_panicking_authority_v1();
        let stream = backend.create_stream_v1(7).unwrap();
        let module = backend.load_module_v1(7, &image).unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
            .unwrap();
        {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        backend.native_available = true;
        let mut explicit_kernarg = [0_u8; 16];
        explicit_kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
        let bindings = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Read,
                byte_offset: 11,
                byte_len: 13,
            },
            kernarg_byte_offset: 0,
        }];
        let submission = backend
            .submit_v1(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &bindings,
                dependencies: &[],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            })
            .unwrap();

        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Failed { code: -1 }
        );
        assert!(backend.pending_compute.is_empty());
        assert!(backend.pending_compute_streams.is_empty());
        assert!(backend.stream_compute_lanes.is_empty());
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.compute_module_retain_counts.is_empty());
        assert!(backend.compute_dependency_retain_counts.is_empty());
        assert_eq!(backend.compute_completion_reservations, 0);
        assert!(backend.active.is_none());
        assert!(
            backend
                .auxiliary_compute_lanes
                .iter()
                .all(|lane| lane.active.is_none() && lane.owner_stream.is_none())
        );

        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.unload_module_v1(module).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.native_available = false;
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_spi_enforces_kernarg_and_binding_bounds_before_custody() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.native_available = true;
        let geometry = crate::RuntimeLaunchGeometryV1 {
            grid: [1, 1, 1],
            workgroup: [1, 1, 1],
            dynamic_shared_bytes: 0,
        };
        let oversized_kernarg = vec![0_u8; MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 + 1];
        assert!(matches!(
            backend.submit_v1(BackendLaunchV1 {
                stream,
                kernel: 99,
                explicit_kernarg: &oversized_kernarg,
                bindings: &[],
                dependencies: &[],
                geometry,
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            }),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        let binding = BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 99,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 1,
            },
            kernarg_byte_offset: 0,
        };
        let oversized_bindings = vec![binding; fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 + 1];
        assert!(matches!(
            backend.submit_v1(BackendLaunchV1 {
                stream,
                kernel: 99,
                explicit_kernarg: &[],
                bindings: &oversized_bindings,
                dependencies: &[],
                geometry,
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            }),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(backend.pending_compute.is_empty());
        assert!(backend.allocation_custody.is_empty());
        backend.native_available = false;
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn logical_streams_and_events_enforce_submission_ownership() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let left = backend.create_stream_v1(7).unwrap();
        let right = backend.create_stream_v1(7).unwrap();
        backend.submissions.insert(
            99,
            SubmissionRecordV1 {
                stream: left,
                status: BackendPollV1::Succeeded,
            },
        );
        let event = backend.record_event_v1(left, 99).unwrap();
        assert_eq!(
            backend
                .collect_compute_dependencies_v1(left, &[event])
                .unwrap(),
            vec![99]
        );
        assert!(matches!(
            backend.record_event_v1(right, 99),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(99).unwrap();
        assert!(matches!(
            backend.release_submission_v1(99),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
    }

    #[test]
    fn logical_stream_destroy_and_recreate_preserves_backend_lifecycle() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        let replacement = backend.create_stream_v1(7).unwrap();
        backend.destroy_stream_v1(replacement).unwrap();
        backend.shutdown_native_v1().unwrap();
        assert!(matches!(
            backend.create_stream_v1(7),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
    }

    #[test]
    fn terminal_state_stays_terminal_across_the_spi() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.terminal = true;
        assert!(matches!(
            backend.enumerate_devices_v1(),
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        // Production drop aborts to enact the terminal process-teardown
        // contract. This synthetic backend owns no native resource.
        std::mem::forget(backend);
    }

    #[test]
    fn live_event_retains_completed_submission_state() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.submissions.insert(
            42,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Succeeded,
            },
        );
        let event = backend.record_event_v1(stream, 42).unwrap();
        assert!(matches!(
            backend.release_submission_v1(42),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(42).unwrap();
    }

    #[test]
    fn deadline_wait_returns_pending_without_a_poll_budget_loop() {
        let start = Instant::now();
        let deadline = start + Duration::from_millis(2);
        let mut polls = 0_u32;
        let status = wait_with_deadline_v1(deadline, || {
            polls += 1;
            Ok::<_, ()>(BackendPollV1::Pending)
        })
        .unwrap();
        assert_eq!(status, BackendPollV1::Pending);
        assert!(Instant::now() >= deadline);
        assert!(polls < 10_000);
    }

    #[test]
    fn deadline_wait_stops_on_success() {
        let mut polls = 0;
        let status = wait_with_deadline_v1(Instant::now() + Duration::from_secs(1), || {
            polls += 1;
            Ok::<_, ()>(if polls == 3 {
                BackendPollV1::Succeeded
            } else {
                BackendPollV1::Pending
            })
        })
        .unwrap();
        assert_eq!(status, BackendPollV1::Succeeded);
        assert_eq!(polls, 3);
    }

    #[test]
    fn productive_pending_polls_do_not_enter_wait_backoff() {
        let mut polls = 0_u32;
        let mut backoffs = 0_u32;
        let status = wait_with_deadline_tracking_progress_by_v1(
            Instant::now() + Duration::from_secs(1),
            || {
                polls += 1;
                Ok::<_, ()>((
                    if polls == 128 {
                        BackendPollV1::Succeeded
                    } else {
                        BackendPollV1::Pending
                    },
                    true,
                ))
            },
            |_, _, _| {
                backoffs += 1;
                true
            },
        )
        .unwrap();
        assert_eq!(status, BackendPollV1::Succeeded);
        assert_eq!(polls, 128);
        assert_eq!(backoffs, 0);
    }

    #[test]
    fn stalled_pending_polls_still_enter_wait_backoff() {
        let mut polls = 0_u32;
        let mut backoffs = 0_u32;
        let status = wait_with_deadline_tracking_progress_by_v1(
            Instant::now() + Duration::from_secs(1),
            || {
                polls += 1;
                Ok::<_, ()>((
                    if polls == 4 {
                        BackendPollV1::Succeeded
                    } else {
                        BackendPollV1::Pending
                    },
                    false,
                ))
            },
            |_, _, _| {
                backoffs += 1;
                true
            },
        )
        .unwrap();
        assert_eq!(status, BackendPollV1::Succeeded);
        assert_eq!(backoffs, 3);
    }

    #[test]
    fn peer_copy_is_explicitly_rejected() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let binding = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        };
        assert!(matches!(
            backend.peer_copy_v1(1, binding, binding, &[]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
    }

    #[test]
    fn multi_device_router_host_stages_peer_copy_and_preserves_event_custody() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        right.description.name = "mock gfx942 right".to_owned();
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let descriptions = backend.enumerate_devices_v1().unwrap();
        assert_eq!(descriptions.len(), 2);
        assert!(
            descriptions.iter().all(|device| {
                device.capabilities.multi_device && device.capabilities.peer_copy
            })
        );

        let left_stream = backend.create_stream_v1(7).unwrap();
        let right_stream = backend.create_stream_v1(8).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 32, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 32, 8)
            .unwrap();
        let expected = (1_u8..=32).collect::<Vec<_>>();
        backend.write_allocation_v1(source, 0, &expected).unwrap();
        let destination_route = backend.allocations[&destination];
        let submission = backend
            .peer_copy_v1(
                right_stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 32,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 32,
                },
                &[],
            )
            .unwrap();
        assert!(
            backend.children[destination_route.child].allocations[&destination_route.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        let event = backend.record_event_v1(right_stream, submission).unwrap();
        let left_child = backend.child_for_device(7).unwrap();
        assert!(matches!(
            backend.dependency_for_child(event, left_child),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::WrongDevice
        ));
        assert!(matches!(
            backend.release_submission_v1(submission),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert!(matches!(
            backend.read_allocation_v1(destination, 0, &mut [0_u8; 1]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert_eq!(
            backend.wait_v1(submission, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert!(
            backend.children[destination_route.child].allocations[&destination_route.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );
        backend.flush_stream_v1(right_stream).unwrap();
        assert_eq!(
            backend
                .wait_v1(submission, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        let mut observed = [0_u8; 32];
        backend
            .read_allocation_v1(destination, 0, &mut observed)
            .unwrap();
        assert_eq!(observed.as_slice(), expected);
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(left_stream).unwrap();
        backend.destroy_stream_v1(right_stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_router_cooperatively_copies_on_one_device() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        backend
            .write_allocation_v1(source, 4, &[9, 8, 7, 6])
            .unwrap();
        let submission = backend
            .copy_async_v1(
                stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 4,
                    byte_len: 4,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 8,
                    byte_len: 4,
                },
                &[],
            )
            .unwrap();
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        let mut observed = [0_u8; 4];
        backend
            .read_allocation_v1(destination, 8, &mut observed)
            .unwrap();
        assert_eq!(observed, [9, 8, 7, 6]);
        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_dependency_translation_is_observational() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend
            .write_allocation_v1(source, 0, &[1, 2, 3, 4])
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let submission = backend
            .copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let event = backend.record_event_v1(stream, submission).unwrap();
        let child = backend.child_for_device(7).unwrap();

        assert!(matches!(
            backend.dependency_for_child(event, child),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert!(matches!(
            &backend.submissions[&submission],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.phase == CooperativeCopyPhaseV1::Dependencies
                    && copy.dependency_cursor == 0
                    && copy.byte_cursor == 0
        ));
        let destination_route = backend.allocations[&destination];
        assert!(
            backend.children[destination_route.child].allocations[&destination_route.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );

        backend.release_event_v1(event).unwrap();
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend
                .wait_v1(submission, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_rejects_native_allocation_custody_before_mutation() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let source_route = backend.allocations[&source];
        let mut active_allocations = HashSet::new();
        active_allocations.insert(source_route.local);
        backend.children[source_route.child].active = Some(ActiveSubmissionV1 {
            id: 99,
            stream: 1,
            kernel: 1,
            dependency_depth: 1,
            allocations: active_allocations,
            writebacks: Vec::new(),
            resident_descriptors: Vec::new(),
            dispatch_shape_sha256: [0; 32],
            published_at: Instant::now(),
            performance: KfdRuntimeLaunchPerformanceV1::default(),
            batch: None,
        });
        let child = &mut backend.children[source_route.child];
        let reserved = child
            .reserve_allocation_custody_v1(&[source_route.local])
            .unwrap();
        child.retain_allocation_custody_v1(
            &[source_route.local],
            RuntimeAllocationCustodyOwnerV1 {
                submission: 99,
                stream: 1,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            },
            reserved,
        );
        let submissions_before = backend.submissions.len();
        let next_handle_before = backend.next_handle;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };

        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert_eq!(backend.submissions.len(), submissions_before);
        assert_eq!(backend.next_handle, next_handle_before);

        backend.children[source_route.child].active = None;
        backend.children[source_route.child].release_allocation_custody_v1(source_route.local, 99);
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_backend_enforces_dependency_capacity() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let excessive = vec![0_u64; MAX_RUNTIME_DEPENDENCIES_V1 + 1];
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };

        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &excessive,
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(backend.submissions.is_empty());

        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_rejects_both_out_of_bounds_ranges_before_publication() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.write_allocation_v1(source, 0, &[7; 8]).unwrap();
        let region = |allocation, byte_offset, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset,
            byte_len: 8,
        };

        for (source_offset, destination_offset) in [(1, 0), (0, 1)] {
            assert!(matches!(
                backend.copy_async_v1(
                    stream,
                    region(source, source_offset, RuntimeAccessV1::Read),
                    region(destination, destination_offset, RuntimeAccessV1::Write),
                    &[],
                ),
                Err(RuntimeBackendFailureV1::Rejected(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
            ));
            assert!(backend.submissions.is_empty());
            let destination_route = backend.allocations[&destination];
            assert!(
                backend.children[destination_route.child].allocations[&destination_route.local]
                    .bytes
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }

        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn synthetic_kfd_async_copy_is_explicitly_unsupported() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        };
        assert!(matches!(
            backend.copy_async_v1(1, region, region, &[]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
        ));
    }

    #[test]
    fn direct_kfd_cancels_only_an_unpublished_dependency_waiter() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.submissions.insert(
            40,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Pending,
            },
        );
        backend.sdma_dependency_retain_counts.insert(40, 1);
        backend.active_sdma.insert(
            41,
            ActiveSdmaCopyV1 {
                id: 41,
                stream,
                prior_stream_submission: Some(40),
                source: 1,
                destination: 2,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: vec![40],
                dependency_cursor: 0,
                dependency_depth: 1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, 41);

        assert_eq!(
            backend.cancel_v1(41).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert!(!backend.active_sdma.contains_key(&41));
        assert!(backend.sdma_dependency_retain_counts.is_empty());
        assert_eq!(
            backend.submissions[&41].status,
            BackendPollV1::Failed { code: -2 }
        );
        backend.release_submission_v1(40).unwrap();
        backend.release_submission_v1(41).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_execution_capabilities_claim_native_queue_concurrency() {
        let mut backend = KfdRuntimeBackendV1::mock();
        assert_eq!(
            backend
                .sdma_memory_pool_observation_v1()
                .unwrap_err()
                .kind(),
            KfdRuntimeBackendErrorKindV1::Unsupported
        );
        assert_eq!(
            backend.execution_capabilities_v1(7),
            RuntimeExecutionCapabilitiesV1::default()
        );
        backend.native_available = true;
        let capabilities = backend.execution_capabilities_v1(7);
        assert!(capabilities.native_async_copy);
        assert!(capabilities.memory_pool);
        assert!(capabilities.cancellation);
        assert!(!capabilities.native_peer_copy);
        assert!(capabilities.concurrent_compute);
        assert!(capabilities.compute_copy_overlap);
        backend.native_available = false;

        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut multi = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        multi.children[0].native_available = true;
        let capabilities = multi.execution_capabilities_v1(7);
        assert!(capabilities.native_async_copy);
        assert!(capabilities.concurrent_compute);
        assert!(capabilities.compute_copy_overlap);
        assert!(capabilities.cancellation);
        multi.children[0].native_available = false;
        multi.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_logical_streams_lease_two_native_lanes_deterministically() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let first = backend.create_stream_v1(7).unwrap();
        let second = backend.create_stream_v1(7).unwrap();
        let third = backend.create_stream_v1(7).unwrap();
        assert!(backend.stream_compute_lanes.is_empty());

        let first_lane = backend.free_compute_lane_v1().unwrap();
        assert_eq!(first_lane, 0);
        backend.lease_compute_lane_v1(third, first_lane);
        let second_lane = backend.free_compute_lane_v1().unwrap();
        assert_eq!(second_lane, 1);
        backend.lease_compute_lane_v1(first, second_lane);
        assert_eq!(backend.free_compute_lane_v1(), None);
        assert_eq!(backend.auxiliary_compute_lanes[0].owner_stream, Some(first));

        backend.release_compute_lane_lease_v1(third, 0);
        assert_eq!(backend.free_compute_lane_v1(), Some(0));
        backend.release_compute_lane_lease_v1(first, 1);
        backend.destroy_stream_v1(first).unwrap();
        backend.destroy_stream_v1(second).unwrap();
        backend.destroy_stream_v1(third).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_allocation_custody_is_bounded_and_fifo_indexed() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let allocation = 77;
        for submission in 1..=MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64 {
            let new_entries = backend
                .reserve_allocation_custody_v1(&[allocation])
                .unwrap();
            backend.retain_allocation_custody_v1(
                &[allocation],
                RuntimeAllocationCustodyOwnerV1 {
                    submission,
                    stream: 9,
                    kind: RuntimeAllocationCustodyKindV1::Compute,
                },
                new_entries,
            );
        }
        let custody = &backend.allocation_custody[&allocation];
        assert_eq!(
            custody.owners.len(),
            MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1
        );
        assert_eq!(custody.sole_stream, Some(9));
        assert_eq!(
            custody.owner_counts,
            [MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1, 0]
        );
        assert!(matches!(
            backend.reserve_allocation_custody_v1(&[allocation]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        backend.release_allocation_custody_v1(allocation, 1);
        backend.release_allocation_custody_v1(
            allocation,
            MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64,
        );
        for submission in 2..MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64 {
            backend.release_allocation_custody_v1(allocation, submission);
        }
        assert!(!backend.allocation_is_active(allocation));
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_submission_capacity_counts_compute_sdma_and_completed() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.submissions.insert(
            1,
            SubmissionRecordV1 {
                stream: 1,
                status: BackendPollV1::Succeeded,
            },
        );
        backend.compute_completion_reservations = MAX_RUNTIME_SUBMISSIONS_V1 - 2;
        backend.sdma_completion_reservations = 1;
        assert!(matches!(
            backend.require_submission_capacity_v1(),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        backend.submissions.clear();
        backend.compute_completion_reservations = 0;
        backend.sdma_completion_reservations = 0;
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_cancelled_tail_restores_earlier_stream_head() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.compute_completion_reservations = 2;
        backend
            .pending_compute_streams
            .insert(stream, VecDeque::from([40, 41]));
        backend
            .pending_compute
            .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
        backend
            .pending_compute
            .insert(41, pending_compute_for_test_v1(41, stream, 101, vec![40]));
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        index_pending_compute_custody_for_test_v1(&mut backend, 41);
        backend.compute_dependency_retain_counts.insert(40, 1);
        backend.stream_submission_tails.insert(stream, 41);

        assert_eq!(
            backend.cancel_v1(41).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert_eq!(backend.stream_submission_tails.get(&stream), Some(&40));
        assert_eq!(
            backend.pending_compute_streams[&stream],
            VecDeque::from([40])
        );
        assert_eq!(backend.compute_completion_reservations, 1);
        assert!(!backend.compute_dependency_retain_counts.contains_key(&40));

        assert_eq!(
            backend.cancel_v1(40).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert!(!backend.stream_submission_tails.contains_key(&stream));
        assert!(!backend.pending_compute_streams.contains_key(&stream));
        assert_eq!(backend.compute_completion_reservations, 0);
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.compute_module_retain_counts.is_empty());
        backend.release_submission_v1(40).unwrap();
        backend.release_submission_v1(41).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_blocked_target_progress_roster_includes_both_lanes() {
        fn active(id: u64, stream: u64) -> ActiveSubmissionV1 {
            ActiveSubmissionV1 {
                id,
                stream,
                kernel: 9,
                dependency_depth: 1,
                allocations: HashSet::new(),
                writebacks: Vec::new(),
                resident_descriptors: Vec::new(),
                dispatch_shape_sha256: [0; 32],
                published_at: Instant::now(),
                performance: KfdRuntimeLaunchPerformanceV1::default(),
                batch: None,
            }
        }

        let mut backend = KfdRuntimeBackendV1::mock();
        backend.active = Some(active(10, 1));
        backend.auxiliary_compute_lanes[0].active = Some(active(11, 2));
        assert_eq!(backend.active_compute_progress_roster_v1(), [true, true]);
        backend.active = None;
        backend.auxiliary_compute_lanes[0].active = None;
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_poll_never_prepares_dependency_ready_queued_compute() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.compute_completion_reservations = 1;
        backend
            .pending_compute_streams
            .insert(stream, VecDeque::from([40]));
        backend
            .pending_compute
            .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        assert_eq!(backend.poll_v1(40).unwrap(), BackendPollV1::Pending);
        assert!(backend.pending_compute.contains_key(&40));
        assert!(backend.stream_compute_lanes.is_empty());
        assert_eq!(backend.compute_completion_reservations, 1);

        assert_eq!(
            backend.cancel_v1(40).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        backend.release_submission_v1(40).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_wait_observes_but_does_not_prepare_queued_predecessors() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let producer_stream = backend.create_stream_v1(7).unwrap();
        let consumer_stream = backend.create_stream_v1(7).unwrap();
        backend.compute_completion_reservations = 2;
        backend
            .pending_compute_streams
            .insert(producer_stream, VecDeque::from([40]));
        backend
            .pending_compute_streams
            .insert(consumer_stream, VecDeque::from([41]));
        backend.pending_compute.insert(
            40,
            pending_compute_for_test_v1(40, producer_stream, 100, vec![]),
        );
        let mut consumer = pending_compute_for_test_v1(41, consumer_stream, 101, vec![40]);
        consumer.prior_stream_submission = None;
        backend.pending_compute.insert(41, consumer);
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        index_pending_compute_custody_for_test_v1(&mut backend, 41);
        backend.compute_dependency_retain_counts.insert(40, 1);
        backend.stream_submission_tails.insert(producer_stream, 40);
        backend.stream_submission_tails.insert(consumer_stream, 41);

        assert_eq!(
            backend
                .wait_v1(41, Instant::now() + Duration::from_millis(1))
                .unwrap(),
            BackendPollV1::Pending
        );
        assert!(backend.pending_compute.contains_key(&40));
        assert!(backend.pending_compute.contains_key(&41));
        assert!(backend.stream_compute_lanes.is_empty());

        backend.cancel_v1(41).unwrap();
        backend.cancel_v1(40).unwrap();
        backend.release_submission_v1(40).unwrap();
        backend.release_submission_v1(41).unwrap();
        backend.destroy_stream_v1(producer_stream).unwrap();
        backend.destroy_stream_v1(consumer_stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_wait_does_not_enter_fixed_wait_for_native_dirty_publication() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .native_dirty
            .push(NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 0,
                allocation_offset: 0,
                data_offset: 0,
                byte_len: 8,
            });
        backend.native_dirty_extents = 1;
        backend.compute_completion_reservations = 1;
        backend
            .pending_compute_streams
            .insert(stream, VecDeque::from([40]));
        backend.pending_compute.insert(
            40,
            pending_compute_for_test_v1(40, stream, allocation, vec![]),
        );
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        assert_eq!(
            backend
                .wait_v1(40, Instant::now() + Duration::from_millis(10))
                .unwrap(),
            BackendPollV1::Pending
        );
        assert!(backend.pending_compute.contains_key(&40));
        assert!(backend.stream_compute_lanes.is_empty());

        backend.cancel_v1(40).unwrap();
        backend.release_submission_v1(40).unwrap();
        backend.native_dirty_extents = 0;
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .native_dirty
            .clear();
        backend.release_allocation_v1(allocation).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_invalid_host_ranges_do_not_reconcile_dirty_authority() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .native_dirty
            .push(NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 0,
                allocation_offset: 0,
                data_offset: 0,
                byte_len: 8,
            });
        backend.native_dirty_extents = 1;
        let dirty_before = backend.allocations[&allocation].native_dirty.clone();

        assert!(matches!(
            backend.write_allocation_v1(allocation, 8, &[1]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        let mut destination = [0_u8; 1];
        assert!(matches!(
            backend.read_allocation_v1(allocation, 8, &mut destination),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        assert_eq!(backend.allocations[&allocation].native_dirty, dirty_before);
        assert_eq!(backend.native_dirty_extents, 1);

        backend.native_dirty_extents = 0;
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .native_dirty
            .clear();
        backend.release_allocation_v1(allocation).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_alternating_compute_copy_dependency_chain_is_bounded() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let mut deepest = pending_compute_for_test_v1(40, 1, 100, vec![]);
        deepest.dependency_depth = MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 - 1;
        backend.pending_compute.insert(40, deepest);
        assert_eq!(
            backend.next_dependency_depth_v1(&[40]),
            Ok(MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
        );
        backend.active_sdma.insert(
            41,
            ActiveSdmaCopyV1 {
                id: 41,
                stream: 1,
                prior_stream_submission: Some(40),
                source: 100,
                destination: 101,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: vec![40],
                dependency_cursor: 0,
                dependency_depth: MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        assert_eq!(
            backend.next_dependency_depth_v1(&[41]),
            Err(DirectSdmaDependencyDepthErrorV1::LimitExceeded)
        );
        backend.pending_compute.clear();
        backend.active_sdma.clear();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_flush_rejects_unknown_stream_without_mutation() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let next_handle = backend.next_handle;
        assert!(matches!(
            backend.flush_stream_v1(99),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle
        ));
        assert_eq!(backend.next_handle, next_handle);
        assert!(backend.pending_compute.is_empty());
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_flush_covers_dependency_ready_unpublished_sdma() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.active_sdma.insert(
            40,
            ActiveSdmaCopyV1 {
                id: 40,
                stream,
                prior_stream_submission: None,
                source,
                destination,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: Vec::new(),
                dependency_cursor: 0,
                dependency_depth: 1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(!backend.active_sdma.contains_key(&40));
        assert_eq!(
            backend.submissions[&40].status,
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1
            }
        );

        backend.release_submission_v1(40).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_flush_reports_quiescent_compute_prepublication_failure() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        backend.compute_completion_reservations = 1;
        backend
            .pending_compute_streams
            .insert(stream, VecDeque::from([40]));
        backend
            .pending_compute
            .insert(40, pending_compute_for_test_v1(40, stream, 100, vec![]));
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Quiescent(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Native
        ));
        assert!(!backend.pending_compute.contains_key(&40));
        assert_eq!(backend.compute_completion_reservations, 0);
        assert_eq!(
            backend.submissions[&40].status,
            BackendPollV1::Failed { code: -1 }
        );

        backend.release_submission_v1(40).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_flush_does_not_treat_later_same_stream_owner_as_conflict() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.compute_completion_reservations = 2;
        backend
            .pending_compute_streams
            .insert(stream, VecDeque::from([40, 41]));
        backend.pending_compute.insert(
            40,
            pending_compute_for_test_v1(40, stream, allocation, vec![]),
        );
        let mut second = pending_compute_for_test_v1(41, stream, allocation, vec![40]);
        second.prior_stream_submission = Some(40);
        backend.pending_compute.insert(41, second);
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        index_pending_compute_custody_for_test_v1(&mut backend, 41);
        backend.compute_dependency_retain_counts.insert(40, 1);
        backend.stream_submission_tails.insert(stream, 41);

        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(!backend.pending_compute.contains_key(&40));
        assert!(backend.pending_compute.contains_key(&41));

        backend.cancel_v1(41).unwrap();
        backend.release_submission_v1(40).unwrap();
        backend.release_submission_v1(41).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_flush_rejects_published_conflict_before_observation() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.active = Some(ActiveSubmissionV1 {
            id: 50,
            stream: 2,
            kernel: 9,
            dependency_depth: 1,
            allocations: HashSet::from([allocation]),
            writebacks: Vec::new(),
            resident_descriptors: Vec::new(),
            dispatch_shape_sha256: [0; 32],
            published_at: Instant::now(),
            performance: KfdRuntimeLaunchPerformanceV1::default(),
            batch: None,
        });
        backend.compute_completion_reservations = 1;
        backend
            .pending_compute_streams
            .insert(stream, VecDeque::from([40]));
        backend.pending_compute.insert(
            40,
            pending_compute_for_test_v1(40, stream, allocation, vec![]),
        );
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        let custody_before = backend.allocation_custody[&allocation].owners.len();
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert_eq!(backend.active.as_ref().map(|active| active.id), Some(50));
        assert!(backend.pending_compute.contains_key(&40));
        assert_eq!(
            backend.allocation_custody[&allocation].owners.len(),
            custody_before
        );
        assert_eq!(backend.compute_completion_reservations, 1);

        backend.active = None;
        backend.cancel_v1(40).unwrap();
        backend.release_submission_v1(40).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_poll_never_publishes_dependency_ready_sdma() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.active_sdma.insert(
            40,
            ActiveSdmaCopyV1 {
                id: 40,
                stream,
                prior_stream_submission: None,
                source,
                destination,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 0,
                packet_bytes: 0,
                dependencies: Vec::new(),
                dependency_cursor: 0,
                dependency_depth: 1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        assert_eq!(backend.poll_v1(40).unwrap(), BackendPollV1::Pending);
        assert!(backend.active_sdma.contains_key(&40));
        assert!(backend.submissions.is_empty());

        backend.cancel_v1(40).unwrap();
        backend.release_submission_v1(40).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_partial_sdma_continuation_is_observed_and_not_cancellable() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.active_sdma.insert(
            40,
            ActiveSdmaCopyV1 {
                id: 40,
                stream,
                prior_stream_submission: None,
                source,
                destination,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 8,
                completed_bytes: 4,
                packet_bytes: 0,
                dependencies: Vec::new(),
                dependency_cursor: 0,
                dependency_depth: 1,
                phase: ActiveDirectionalSdmaPhaseV1::Ready,
            },
        );
        index_sdma_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(stream, 40);

        assert_eq!(backend.poll_v1(40).unwrap(), BackendPollV1::Pending);
        assert_eq!(backend.active_sdma[&40].completed_bytes, 4);
        assert!(matches!(
            backend.active_sdma[&40].phase,
            ActiveDirectionalSdmaPhaseV1::Ready
        ));
        assert_eq!(
            backend.cancel_v1(40).unwrap(),
            crate::BackendCancellationV1::TooLate
        );

        // Repair the synthetic fixture to exercise ordinary prepublication
        // cleanup; production cannot roll back already-published bytes.
        backend.active_sdma.get_mut(&40).unwrap().completed_bytes = 0;
        assert_eq!(
            backend.cancel_v1(40).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        backend.release_submission_v1(40).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_poll_and_expired_wait_leave_cooperative_copy_for_explicit_flush() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend
            .write_allocation_v1(source, 0, &[1, 2, 3, 4])
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let submission = backend
            .copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let destination_route = backend.allocations[&destination];
        let generation = backend.cooperative_progress_generation;
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            backend.wait_v1(submission, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(backend.cooperative_progress_generation, generation);
        assert!(
            backend.children[destination_route.child].allocations[&destination_route.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );

        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_stream_tail_cannot_exceed_dependency_bound() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let mut events = Vec::new();
        for index in 0..MAX_RUNTIME_DEPENDENCIES_V1 {
            let submission = 1_000 + index as u64;
            let event = 2_000 + index as u64;
            backend.submissions.insert(
                submission,
                SubmissionRecordV1 {
                    stream,
                    status: BackendPollV1::Succeeded,
                },
            );
            backend.events.insert(event, EventRecordV1 { submission });
            events.push(event);
        }
        backend.stream_submission_tails.insert(stream, 9_999);
        backend.submissions.insert(
            9_999,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Succeeded,
            },
        );

        assert!(matches!(
            backend.collect_compute_dependencies_v1(stream, &events),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));

        backend.events.clear();
        backend.submissions.clear();
        backend.stream_submission_tails.clear();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_copy_stream_tail_cannot_exceed_dependency_bound() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap();
        for allocation in [source, destination] {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        let mut events = Vec::new();
        for index in 0..MAX_RUNTIME_DEPENDENCIES_V1 {
            let submission = 1_000 + index as u64;
            let event = 2_000 + index as u64;
            backend.submissions.insert(
                submission,
                SubmissionRecordV1 {
                    stream,
                    status: BackendPollV1::Succeeded,
                },
            );
            backend.events.insert(event, EventRecordV1 { submission });
            events.push(event);
        }
        backend.stream_submission_tails.insert(stream, 9_999);
        backend.submissions.insert(
            9_999,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Succeeded,
            },
        );
        backend.native_available = true;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };

        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &events,
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(backend.active_sdma.is_empty());

        backend.native_available = false;
        for allocation in [source, destination] {
            backend
                .allocations
                .get_mut(&allocation)
                .unwrap()
                .sdma_backed = false;
        }
        backend.events.clear();
        backend.submissions.clear();
        backend.stream_submission_tails.clear();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_copy_cannot_pass_unpublished_cross_stream_compute() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let compute_stream = backend.create_stream_v1(7).unwrap();
        let copy_stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap();
        for allocation in [source, destination] {
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        backend.compute_completion_reservations = 1;
        backend
            .pending_compute_streams
            .insert(compute_stream, VecDeque::from([40]));
        backend.pending_compute.insert(
            40,
            pending_compute_for_test_v1(40, compute_stream, source, vec![]),
        );
        index_pending_compute_custody_for_test_v1(&mut backend, 40);
        backend.stream_submission_tails.insert(compute_stream, 40);
        backend.native_available = true;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };

        assert!(matches!(
            backend.copy_async_v1(
                copy_stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert!(backend.active_sdma.is_empty());

        backend.native_available = false;
        backend.cancel_v1(40).unwrap();
        backend.release_submission_v1(40).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(compute_stream).unwrap();
        backend.destroy_stream_v1(copy_stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn direct_kfd_active_compute_custody_is_exact_per_lane() {
        fn active(id: u64, stream: u64, allocation: u64) -> ActiveSubmissionV1 {
            ActiveSubmissionV1 {
                id,
                stream,
                kernel: 9,
                dependency_depth: 1,
                allocations: HashSet::from([allocation]),
                writebacks: Vec::new(),
                resident_descriptors: Vec::new(),
                dispatch_shape_sha256: [0; 32],
                published_at: Instant::now(),
                performance: KfdRuntimeLaunchPerformanceV1::default(),
                batch: None,
            }
        }

        let mut backend = KfdRuntimeBackendV1::mock();
        backend.active = Some(active(11, 1, 101));
        backend.auxiliary_compute_lanes[0].active = Some(active(12, 2, 202));
        for (submission, stream, allocation) in [(11, 1, 101), (12, 2, 202)] {
            let new_entries = backend
                .reserve_allocation_custody_v1(&[allocation])
                .unwrap();
            backend.retain_allocation_custody_v1(
                &[allocation],
                RuntimeAllocationCustodyOwnerV1 {
                    submission,
                    stream,
                    kind: RuntimeAllocationCustodyKindV1::Compute,
                },
                new_entries,
            );
        }
        assert_eq!(backend.active_compute_lane_v1(11), Some(0));
        assert_eq!(backend.active_compute_lane_v1(12), Some(1));
        assert!(backend.allocation_is_active(101));
        assert!(backend.allocation_is_active(202));
        assert!(!backend.allocation_is_active(303));
        let disjoint = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 303,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 8,
            },
            kernarg_byte_offset: 0,
        }];
        let conflicting = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 202,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 8,
            },
            kernarg_byte_offset: 0,
        }];
        let active = backend.active.iter().chain(
            backend
                .auxiliary_compute_lanes
                .iter()
                .filter_map(|lane| lane.active.as_ref()),
        );
        assert!(!launch_overlaps_active_compute_v1(&disjoint, active));
        let active = backend.active.iter().chain(
            backend
                .auxiliary_compute_lanes
                .iter()
                .filter_map(|lane| lane.active.as_ref()),
        );
        assert!(launch_overlaps_active_compute_v1(&conflicting, active));

        backend.with_compute_lane_state_v1(1, |selected| {
            assert_eq!(selected.active.as_ref().map(|active| active.id), Some(12));
        });
        let failed: Result<(), &'static str> = backend.with_compute_lane_state_v1(1, |selected| {
            assert_eq!(selected.active.as_ref().map(|active| active.id), Some(12));
            Err("injected lane-local rejection")
        });
        assert_eq!(failed, Err("injected lane-local rejection"));
        assert_eq!(backend.active.as_ref().map(|active| active.id), Some(11));
        assert_eq!(
            backend.auxiliary_compute_lanes[0]
                .active
                .as_ref()
                .map(|active| active.id),
            Some(12)
        );
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.with_compute_lane_state_v1(1, |_| panic!("injected lane-local panic"));
        }));
        assert!(panicked.is_err());
        assert_eq!(backend.active.as_ref().map(|active| active.id), Some(11));
        assert_eq!(
            backend.auxiliary_compute_lanes[0]
                .active
                .as_ref()
                .map(|active| active.id),
            Some(12)
        );
        backend.active = None;
        backend.auxiliary_compute_lanes[0].active = None;
        backend.release_allocation_custody_v1(101, 11);
        backend.release_allocation_custody_v1(202, 12);
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_dependency_retains_prior_submission_until_completion() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let first_source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let shared = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let final_destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend
            .write_allocation_v1(first_source, 0, &[1, 3, 3, 7])
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let first = backend
            .copy_async_v1(
                stream,
                region(first_source, RuntimeAccessV1::Read),
                region(shared, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let event = backend.record_event_v1(stream, first).unwrap();
        let second = backend
            .copy_async_v1(
                stream,
                region(shared, RuntimeAccessV1::Read),
                region(final_destination, RuntimeAccessV1::Write),
                &[event],
            )
            .unwrap();
        assert_eq!(backend.poll_v1(second).unwrap(), BackendPollV1::Pending);
        backend.release_event_v1(event).unwrap();
        assert!(matches!(
            backend.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend
                .wait_v1(second, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
        backend.release_submission_v1(first).unwrap();
        backend.release_submission_v1(second).unwrap();
        let mut observed = [0_u8; 4];
        backend
            .read_allocation_v1(final_destination, 0, &mut observed)
            .unwrap();
        assert_eq!(observed, [1, 3, 3, 7]);
        for allocation in [first_source, shared, final_destination] {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_same_stream_overlap_uses_transitive_fifo_tail() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocations = (0..4)
            .map(|_| {
                backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        backend
            .write_allocation_v1(allocations[0], 0, &[4, 3, 2, 1])
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let first = backend
            .copy_async_v1(
                stream,
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[1], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let second = backend
            .copy_async_v1(
                stream,
                region(allocations[1], RuntimeAccessV1::Read),
                region(allocations[2], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let third = backend
            .copy_async_v1(
                stream,
                region(allocations[2], RuntimeAccessV1::Read),
                region(allocations[3], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        assert!(matches!(
            &backend.submissions[&second],
            RoutedSubmissionV1::CooperativeCopy(copy) if copy.dependencies == [first]
        ));
        assert!(matches!(
            &backend.submissions[&third],
            RoutedSubmissionV1::CooperativeCopy(copy) if copy.dependencies == [second]
        ));
        assert_eq!(backend.poll_v1(third).unwrap(), BackendPollV1::Pending);
        backend.flush_stream_v1(stream).unwrap();
        for submission in [first, second, third] {
            assert_eq!(
                backend.poll_v1(submission).unwrap(),
                BackendPollV1::Succeeded
            );
            backend.release_submission_v1(submission).unwrap();
        }
        let mut observed = [0_u8; 4];
        backend
            .read_allocation_v1(allocations[3], 0, &mut observed)
            .unwrap();
        assert_eq!(observed, [4, 3, 2, 1]);
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn native_copy_rejects_parent_cooperative_allocation_custody_before_child_call() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let cooperative_stream = backend.create_stream_v1(7).unwrap();
        let native_stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let cooperative = backend
            .copy_async_v1(
                cooperative_stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 4,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 4,
                },
                &[],
            )
            .unwrap();
        let child = backend.allocations[&source].child;
        let child_next_handle = backend.children[child].next_handle;
        backend.children[child].native_available = true;
        assert!(matches!(
            backend.copy_async_v1(
                native_stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 4,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 4,
                },
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert_eq!(backend.children[child].next_handle, child_next_handle);
        backend.children[child].native_available = false;
        backend.flush_stream_v1(cooperative_stream).unwrap();
        backend.release_submission_v1(cooperative).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(cooperative_stream).unwrap();
        backend.destroy_stream_v1(native_stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_indexes_track_fan_out_and_quiescence_exactly() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocations = (0..6)
            .map(|_| {
                backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let first = backend
            .copy_async_v1(
                stream,
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[1], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.cooperative_stream_pending_counts[&stream], 1);

        let first_event = backend.record_event_v1(stream, first).unwrap();
        let second_event = backend.record_event_v1(stream, first).unwrap();
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.event_submission_retain_counts[&first], 2);

        let second = backend
            .copy_async_v1(
                stream,
                region(allocations[2], RuntimeAccessV1::Read),
                region(allocations[3], RuntimeAccessV1::Write),
                &[first_event],
            )
            .unwrap();
        let third = backend
            .copy_async_v1(
                stream,
                region(allocations[4], RuntimeAccessV1::Read),
                region(allocations[5], RuntimeAccessV1::Write),
                &[second_event],
            )
            .unwrap();
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.cooperative_dependency_retain_counts[&first], 2);
        assert_eq!(backend.cooperative_stream_pending_counts[&stream], 3);

        backend.release_event_v1(first_event).unwrap();
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.event_submission_retain_counts[&first], 1);
        backend.release_event_v1(second_event).unwrap();
        backend.assert_cooperative_indexes_consistent();
        assert!(!backend.event_submission_retain_counts.contains_key(&first));
        assert!(matches!(
            backend.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));

        while backend.progress_cooperative_copy(second).unwrap() == BackendPollV1::Pending {}
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.cooperative_dependency_retain_counts[&first], 1);
        assert_eq!(backend.cooperative_stream_pending_counts[&stream], 1);
        assert!(matches!(
            backend.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));

        while backend.progress_cooperative_copy(third).unwrap() == BackendPollV1::Pending {}
        backend.assert_cooperative_indexes_consistent();
        assert!(backend.cooperative_allocation_owners.is_empty());
        assert!(backend.cooperative_dependency_retain_counts.is_empty());
        assert!(backend.cooperative_stream_pending_counts.is_empty());
        for submission in [first, second, third] {
            backend.release_submission_v1(submission).unwrap();
        }
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_cancellation_before_destination_write_releases_exact_custody() {
        fn requires_worker_v4_backend<B>()
        where
            B: RuntimeBackendV1
                + RuntimeAsyncCopyBackendV1
                + RuntimeFlushBackendV1
                + RuntimeCancellationBackendV1,
        {
        }
        requires_worker_v4_backend::<KfdMultiDeviceRuntimeBackendV1>();

        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        assert!(backend.execution_capabilities_v1(7).cancellation);
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };
        let submission = backend
            .copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let progress_before = backend.cooperative_progress_generation;
        assert_eq!(
            backend.drain_v1(submission, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(backend.cooperative_progress_generation, progress_before);

        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Failed { code: -2 }
        );
        assert_eq!(backend.cooperative_staging_bytes, 0);
        assert!(!backend.cooperative_stream_tails.contains_key(&stream));
        backend.assert_cooperative_indexes_consistent();

        let replacement = backend
            .copy_async_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        assert_eq!(
            backend.cancel_v1(replacement).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        backend.assert_cooperative_indexes_consistent();
        backend.release_submission_v1(submission).unwrap();
        backend.release_submission_v1(replacement).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_cancellation_is_too_late_after_first_destination_write() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(8).unwrap();
        let byte_len = u64::try_from(COOPERATIVE_COPY_CHUNK_BYTES_V1 + 1).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
            .unwrap();
        let submission = backend
            .peer_copy_v1(
                stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len,
                },
                &[],
            )
            .unwrap();

        for _ in 0..8 {
            let first_write_completed = matches!(
                &backend.submissions[&submission],
                RoutedSubmissionV1::CooperativeCopy(copy)
                    if copy.phase == CooperativeCopyPhaseV1::Write && copy.byte_cursor != 0
            );
            if first_write_completed {
                break;
            }
            assert_eq!(
                backend.progress_cooperative_copy(submission).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert!(matches!(
            &backend.submissions[&submission],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.phase == CooperativeCopyPhaseV1::Write
                    && copy.byte_cursor == COOPERATIVE_COPY_CHUNK_BYTES_V1
        ));
        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::TooLate
        );
        assert_eq!(backend.cooperative_staging_bytes, byte_len);
        backend.assert_cooperative_indexes_consistent();

        assert_eq!(
            backend
                .drain_v1(submission, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        backend.assert_cooperative_indexes_consistent();
        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_staging_budget_rejects_before_publication_and_releases_at_quiescence() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        backend.cooperative_staging_limit_bytes = 8;
        let stream = backend.create_stream_v1(7).unwrap();
        let allocations = (0..6)
            .map(|_| {
                backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let region = |allocation, access, byte_len| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len,
        };
        let first = backend
            .copy_async_v1(
                stream,
                region(allocations[0], RuntimeAccessV1::Read, 4),
                region(allocations[1], RuntimeAccessV1::Write, 4),
                &[],
            )
            .unwrap();
        let second = backend
            .copy_async_v1(
                stream,
                region(allocations[2], RuntimeAccessV1::Read, 4),
                region(allocations[3], RuntimeAccessV1::Write, 4),
                &[],
            )
            .unwrap();
        assert_eq!(backend.cooperative_staging_bytes, 8);
        backend.assert_cooperative_indexes_consistent();

        let submissions_before = backend.submissions.len();
        let next_handle_before = backend.next_handle;
        let allocation_owners_before = backend.cooperative_allocation_owners.clone();
        let dependency_counts_before = backend.cooperative_dependency_retain_counts.clone();
        let stream_counts_before = backend.cooperative_stream_pending_counts.clone();
        let event_counts_before = backend.event_submission_retain_counts.clone();
        let events_before = backend.events.len();
        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(allocations[4], RuntimeAccessV1::Read, 1),
                region(allocations[5], RuntimeAccessV1::Write, 1),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert_eq!(backend.submissions.len(), submissions_before);
        assert_eq!(backend.next_handle, next_handle_before);
        assert_eq!(backend.cooperative_staging_bytes, 8);
        assert_eq!(
            backend.cooperative_allocation_owners,
            allocation_owners_before
        );
        assert_eq!(
            backend.cooperative_dependency_retain_counts,
            dependency_counts_before
        );
        assert_eq!(
            backend.cooperative_stream_pending_counts,
            stream_counts_before
        );
        assert_eq!(backend.event_submission_retain_counts, event_counts_before);
        assert_eq!(backend.events.len(), events_before);
        backend.assert_cooperative_indexes_consistent();

        while backend.progress_cooperative_copy(first).unwrap() == BackendPollV1::Pending {}
        assert_eq!(backend.cooperative_staging_bytes, 4);
        assert!(matches!(
            &backend.submissions[&first],
            RoutedSubmissionV1::CooperativeCopy(copy) if copy.staging.is_empty()
        ));
        backend.assert_cooperative_indexes_consistent();

        let third = backend
            .copy_async_v1(
                stream,
                region(allocations[4], RuntimeAccessV1::Read, 1),
                region(allocations[5], RuntimeAccessV1::Write, 1),
                &[],
            )
            .unwrap();
        assert_eq!(backend.cooperative_staging_bytes, 5);
        backend.flush_stream_v1(stream).unwrap();
        for submission in [second, third] {
            assert_eq!(
                backend.poll_v1(submission).unwrap(),
                BackendPollV1::Succeeded
            );
        }
        assert_eq!(backend.cooperative_staging_bytes, 0);
        backend.assert_cooperative_indexes_consistent();

        for submission in [first, second, third] {
            backend.release_submission_v1(submission).unwrap();
        }
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_index_overflow_rejects_before_publication() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocations = (0..4)
            .map(|_| {
                backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let first = backend
            .copy_async_v1(
                stream,
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[1], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let submissions_before = backend.submissions.len();
        let next_handle_before = backend.next_handle;
        let owners_before = backend.cooperative_allocation_owners.clone();

        backend
            .cooperative_stream_pending_counts
            .insert(stream, usize::MAX);
        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(allocations[2], RuntimeAccessV1::Read),
                region(allocations[3], RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert_eq!(backend.submissions.len(), submissions_before);
        assert_eq!(backend.next_handle, next_handle_before);
        assert_eq!(backend.cooperative_allocation_owners, owners_before);
        backend.cooperative_stream_pending_counts.insert(stream, 1);
        backend.assert_cooperative_indexes_consistent();

        backend
            .event_submission_retain_counts
            .insert(first, usize::MAX);
        assert!(matches!(
            backend.record_event_v1(stream, first),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(backend.events.is_empty());
        assert_eq!(backend.next_handle, next_handle_before);
        backend.event_submission_retain_counts.remove(&first);
        backend.assert_cooperative_indexes_consistent();

        let event = backend.record_event_v1(stream, first).unwrap();
        let next_handle_before = backend.next_handle;
        backend
            .cooperative_dependency_retain_counts
            .insert(first, usize::MAX);
        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(allocations[2], RuntimeAccessV1::Read),
                region(allocations[3], RuntimeAccessV1::Write),
                &[event],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert_eq!(backend.submissions.len(), submissions_before);
        assert_eq!(backend.next_handle, next_handle_before);
        assert_eq!(backend.cooperative_allocation_owners, owners_before);
        backend.cooperative_dependency_retain_counts.remove(&first);
        backend.assert_cooperative_indexes_consistent();

        backend.release_event_v1(event).unwrap();
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend
                .wait_v1(first, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        backend.assert_cooperative_indexes_consistent();
        backend.release_submission_v1(first).unwrap();
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_dependency_depth_is_bounded_before_publication() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let mut allocations = Vec::new();
        let mut submissions = Vec::new();
        let mut dependency_event = None;
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 1,
        };

        for expected_depth in 1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let source = backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
                .unwrap();
            let destination = backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
                .unwrap();
            let dependencies = dependency_event.as_slice();
            let submission = backend
                .copy_async_v1(
                    stream,
                    region(source, RuntimeAccessV1::Read),
                    region(destination, RuntimeAccessV1::Write),
                    dependencies,
                )
                .unwrap();
            assert!(matches!(
                &backend.submissions[&submission],
                RoutedSubmissionV1::CooperativeCopy(copy)
                    if copy.dependency_depth == expected_depth
            ));
            if let Some(event) =
                dependency_event.replace(backend.record_event_v1(stream, submission).unwrap())
            {
                backend.release_event_v1(event).unwrap();
            }
            allocations.extend([source, destination]);
            submissions.push(submission);
        }
        backend.assert_cooperative_indexes_consistent();

        let rejected_source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
            .unwrap();
        let rejected_destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
            .unwrap();
        let submissions_before = backend.submissions.len();
        let next_handle_before = backend.next_handle;
        let allocation_owners_before = backend.cooperative_allocation_owners.clone();
        let dependency_counts_before = backend.cooperative_dependency_retain_counts.clone();
        let stream_counts_before = backend.cooperative_stream_pending_counts.clone();
        let event_counts_before = backend.event_submission_retain_counts.clone();
        assert!(matches!(
            backend.copy_async_v1(
                stream,
                region(rejected_source, RuntimeAccessV1::Read),
                region(rejected_destination, RuntimeAccessV1::Write),
                dependency_event.as_slice(),
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert_eq!(backend.submissions.len(), submissions_before);
        assert_eq!(backend.next_handle, next_handle_before);
        assert_eq!(
            backend.cooperative_allocation_owners,
            allocation_owners_before
        );
        assert_eq!(
            backend.cooperative_dependency_retain_counts,
            dependency_counts_before
        );
        assert_eq!(
            backend.cooperative_stream_pending_counts,
            stream_counts_before
        );
        assert_eq!(backend.event_submission_retain_counts, event_counts_before);
        backend.assert_cooperative_indexes_consistent();
        backend.release_allocation_v1(rejected_source).unwrap();
        backend.release_allocation_v1(rejected_destination).unwrap();

        let last = *submissions.last().unwrap();
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend
                .wait_v1(last, Instant::now() + Duration::from_secs(2))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        backend.release_event_v1(dependency_event.unwrap()).unwrap();
        backend.assert_cooperative_indexes_consistent();
        assert!(backend.cooperative_allocation_owners.is_empty());
        assert!(backend.cooperative_dependency_retain_counts.is_empty());
        assert!(backend.cooperative_stream_pending_counts.is_empty());
        assert!(backend.event_submission_retain_counts.is_empty());
        for submission in submissions {
            backend.release_submission_v1(submission).unwrap();
        }
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_poll_is_observational_and_flush_drives_fifo_fan_in() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocations = (0..6)
            .map(|_| {
                backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        backend
            .write_allocation_v1(allocations[0], 0, &[1, 2, 3, 4])
            .unwrap();
        backend
            .write_allocation_v1(allocations[2], 0, &[5, 6, 7, 8])
            .unwrap();
        backend
            .write_allocation_v1(allocations[4], 0, &[9, 10, 11, 12])
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 4,
        };
        let first = backend
            .copy_async_v1(
                stream,
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[1], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let second = backend
            .copy_async_v1(
                stream,
                region(allocations[2], RuntimeAccessV1::Read),
                region(allocations[3], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        for submission in [first, second] {
            assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
            assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        }
        let first_event = backend.record_event_v1(stream, first).unwrap();
        let second_event = backend.record_event_v1(stream, second).unwrap();
        let dependent = backend
            .copy_async_v1(
                stream,
                region(allocations[4], RuntimeAccessV1::Read),
                region(allocations[5], RuntimeAccessV1::Write),
                &[first_event, second_event],
            )
            .unwrap();

        let generation = backend.cooperative_progress_generation;
        assert_eq!(backend.poll_v1(dependent).unwrap(), BackendPollV1::Pending);
        assert_eq!(backend.cooperative_progress_generation, generation);
        for submission in [first, second, dependent] {
            assert!(matches!(
                &backend.submissions[&submission],
                RoutedSubmissionV1::CooperativeCopy(copy)
                    if copy.status() == BackendPollV1::Pending
            ));
        }
        let second_destination = backend.allocations[&allocations[3]];
        assert!(
            backend.children[second_destination.child].allocations[&second_destination.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );

        assert_eq!(
            backend.wait_v1(dependent, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(backend.cooperative_progress_generation, generation);
        backend.flush_stream_v1(stream).unwrap();
        for submission in [first, second, dependent] {
            assert_eq!(
                backend.poll_v1(submission).unwrap(),
                BackendPollV1::Succeeded
            );
        }
        backend.release_event_v1(first_event).unwrap();
        backend.release_event_v1(second_event).unwrap();
        for submission in [first, second, dependent] {
            backend.release_submission_v1(submission).unwrap();
        }
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn cooperative_copy_terminal_failure_latches_and_retains_custody() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(8).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let submission = backend
            .peer_copy_v1(
                stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 8,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 8,
                },
                &[],
            )
            .unwrap();
        assert_eq!(backend.cooperative_staging_bytes, 8);
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        let source_child = backend.allocations[&source].child;
        backend.children[source_child].terminal = true;
        assert!(matches!(
            backend.flush_stream_v1(stream),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.terminal);
        assert_eq!(backend.cooperative_staging_bytes, 8);
        assert!(backend.submissions.contains_key(&submission));
        assert!(matches!(
            backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));

        // Private test-only repair prevents the mock child's fail-closed Drop
        // path from aborting the test process; production has no reset API.
        backend.children[source_child].terminal = false;
        backend.terminal = false;
        backend.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Failed);
        assert_eq!(backend.cooperative_staging_bytes, 0);
        backend.assert_cooperative_indexes_consistent();
        backend.release_submission_v1(submission).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_router_latches_a_child_terminal_failure_globally() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        backend.children[0].terminal = true;
        assert!(matches!(
            backend.enumerate_devices_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        backend.children[0].terminal = false;
        assert!(matches!(
            backend.create_stream_v1(8),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        backend.terminal = false;
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_router_rejects_invalid_peer_access_before_copy() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(8).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };
        assert!(matches!(
            backend.peer_copy_v1(
                stream,
                region(source, RuntimeAccessV1::Write),
                region(destination, RuntimeAccessV1::Read),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn runtime_context_composes_multi_device_peer_copy_and_cleanup() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let mut context = crate::RuntimeContextV1::open(backend).unwrap();
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        let stream = context.create_stream(destination_device).unwrap();
        let source = context
            .allocate(source_device, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = context
            .allocate(destination_device, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        context
            .write_allocation(source, 0, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        let mut submission = context
            .peer_copy(
                stream,
                crate::RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 8,
                },
                crate::RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 8,
                },
                &[],
            )
            .unwrap();
        context.flush_stream(stream).unwrap();
        assert_eq!(
            context
                .wait(&mut submission, Duration::from_secs(1))
                .unwrap(),
            crate::RuntimePollV1::Succeeded
        );
        let mut observed = [0_u8; 8];
        context
            .read_allocation(destination, 0, &mut observed)
            .unwrap();
        assert_eq!(observed, [1, 2, 3, 4, 5, 6, 7, 8]);
        context.release_submission(submission).unwrap();
        context.release_allocation(source).unwrap();
        context.release_allocation(destination).unwrap();
        context.destroy_stream(stream).unwrap();
        let mut backend = context.shutdown().unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_router_rejects_peer_copy_on_the_source_stream() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let left_stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };
        assert!(matches!(
            backend.peer_copy_v1(
                left_stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(left_stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }

    #[test]
    fn multi_device_route_exhaustion_precedes_child_mutation() {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        backend.next_handle = u64::MAX;
        assert!(matches!(
            backend.create_stream_v1(7),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert!(backend.streams.is_empty());
        assert!(backend.children[0].streams.is_empty());
    }
}
