//! Pure-Rust KFD implementation of the backend-neutral runtime SPI.
//!
//! The admitted gfx942 KFD surface owns explicit process VMs and native queues.
//! The single-device adapter multiplexes logical streams onto one compute queue
//! and directional SDMA queues. The separate two-device adapter retains exact
//! directional XGMI routes for copy-only peer execution. Neither adapter
//! advertises atomic or collective execution.

use core::fmt;
use std::collections::{HashMap, HashSet};
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
    CheckedGfx942XnackMinusDevice, ComputeAqlQueueSessionV1, DeviceSelector,
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
    Gfx942CompletedDispatchReadRequestV1, Gfx942DeviceContentDescriptorV1,
    Gfx942DeviceContentRoleV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryUnmappedV1,
    Gfx942DispatchBatchV1, Gfx942DispatchBufferBindingV1, Gfx942DispatchPollV1,
    Gfx942FixedDispatchDataV1, Gfx942FixedDispatchPacketV1, Gfx942NativeXgmiSdmaQueueV1,
    Gfx942RecycledDispatchWriteRequestV1, Gfx942SdmaBufferKindV1, Gfx942SdmaBufferV1,
    Gfx942SdmaCopyPollV1, Gfx942SdmaCopyTicketV1, Gfx942SdmaMemoryPoolObservationV1,
    Gfx942XgmiCopyFailureV1, Gfx942XgmiCopyPollV1, Gfx942XgmiMapRecoveryV1,
    Gfx942XgmiMappedDeviceMemoryV1, Gfx942XgmiUnmapRecoveryV1, HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
    OpenedKfd, SharedGttMemorySessionV1,
};
use sha2::{Digest, Sha256};

use crate::{
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, MAX_RUNTIME_DEPENDENCIES_V1, RuntimeAccessV1, RuntimeAsyncCopyBackendV1,
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCancellationBackendV1, RuntimeCapabilitiesV1,
    RuntimeExecutionCapabilitiesV1, RuntimeMemoryKindV1,
};

const KFD_RUNTIME_RING_BYTES_V1: u32 = 64 * 1024;
const COV6_IMPLICIT_KERNARG_BYTES_V1: usize = 256;
const WAIT_SPINS_V1: u32 = 32;
const WAIT_YIELDS_V1: u32 = 8;
const WAIT_INITIAL_SLEEP_V1: Duration = Duration::from_micros(50);
const WAIT_MAX_SLEEP_V1: Duration = Duration::from_millis(1);
const COOPERATIVE_COPY_CHUNK_BYTES_V1: usize = 64 * 1024;
const COOPERATIVE_COPY_FAILURE_CODE_V1: i64 = -1;
const MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1: usize = 256;
const MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1: usize = MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1;

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
/// structural AMDHSA validation alone do not satisfy this contract.
pub unsafe trait KfdRuntimeLaunchAuthorityV1: fmt::Debug {
    fn authorize_launch_v1(&self, request: KfdRuntimeAuthorityRequestV1<'_>) -> bool;
}

enum KfdRuntimeLaunchGateV1 {
    Production(Box<dyn KfdRuntimeLaunchAuthorityV1>),
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
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942Vecadd(_) => formatter.write_str("ExactGfx942Vecadd"),
        }
    }
}

impl KfdRuntimeLaunchGateV1 {
    fn authorize_launch_v1(&self, request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        match self {
            Self::Production(authority) => authority.authorize_launch_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942Vecadd(admitted) => admitted.authorizes_kfd_request_v1(request),
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
    sdma_buffer: Option<Gfx942SdmaBufferV1>,
    sdma_backed: bool,
    sdma_initialized: bool,
    sdma_shadow_dirty: bool,
}

#[derive(Clone, Copy, Debug)]
struct NativeDirtyExtentV1 {
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
    allocations: HashSet<u64>,
    writebacks: Vec<WritebackV1>,
    resident_descriptors: Vec<ResidentDataDescriptorV1>,
    dispatch_shape_sha256: [u8; 32],
    published_at: Instant,
    performance: KfdRuntimeLaunchPerformanceV1,
    batch: Option<Gfx942DispatchBatchV1<1>>,
}

#[derive(Debug)]
struct ActiveSdmaCopyV1 {
    id: u64,
    stream: u64,
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
    ticket: Option<Gfx942SdmaCopyTicketV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectSdmaDependencyDepthErrorV1 {
    Overflow,
    LimitExceeded,
}

fn next_direct_sdma_dependency_depth_v1(
    active: &HashMap<u64, ActiveSdmaCopyV1>,
    dependencies: &[u64],
) -> Result<usize, DirectSdmaDependencyDepthErrorV1> {
    let mut depth = 1_usize;
    for dependency in dependencies {
        let Some(copy) = active.get(dependency) else {
            continue;
        };
        let candidate = copy
            .dependency_depth
            .checked_add(1)
            .ok_or(DirectSdmaDependencyDepthErrorV1::Overflow)?;
        depth = depth.max(candidate);
    }
    if depth > MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 {
        Err(DirectSdmaDependencyDepthErrorV1::LimitExceeded)
    } else {
        Ok(depth)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KfdCopyComputeAdmissionV1 {
    Concurrent,
    DeferredByDependency,
    Busy,
}

fn admit_copy_against_active_compute_v1(
    active: Option<&ActiveSubmissionV1>,
    source: u64,
    destination: u64,
    dependencies: &[u64],
) -> KfdCopyComputeAdmissionV1 {
    let Some(active) = active else {
        return KfdCopyComputeAdmissionV1::Concurrent;
    };
    let overlaps =
        active.allocations.contains(&source) || active.allocations.contains(&destination);
    if !overlaps {
        return KfdCopyComputeAdmissionV1::Concurrent;
    }
    if dependencies.contains(&active.id) {
        KfdCopyComputeAdmissionV1::DeferredByDependency
    } else {
        KfdCopyComputeAdmissionV1::Busy
    }
}

fn launch_overlaps_active_sdma_v1<'a>(
    bindings: &[BackendBindingV1],
    active: impl Iterator<Item = &'a ActiveSdmaCopyV1>,
) -> bool {
    active.into_iter().any(|copy| {
        bindings.iter().any(|binding| {
            binding.region.allocation == copy.source
                || binding.region.allocation == copy.destination
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
                && end <= allocation.bytes.len() as u64
        })
}

fn take_sdma_buffer_after_scrub_v1<T, E>(
    slot: &mut Option<T>,
    scrub: Result<(), E>,
) -> Result<Option<T>, E> {
    scrub?;
    Ok(slot.take())
}

fn validate_sdma_copy_buffer_restore_slots_v1(
    source: u64,
    destination: u64,
    source_occupied: Option<bool>,
    destination_occupied: Option<bool>,
) -> Result<(), &'static str> {
    if source == destination {
        return Err("SDMA completion aliases one allocation twice");
    }
    if source_occupied.ok_or("SDMA source allocation disappeared")? {
        return Err("SDMA source custody was already restored");
    }
    if destination_occupied.ok_or("SDMA destination allocation disappeared")? {
        return Err("SDMA destination custody was already restored");
    }
    Ok(())
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
/// The adapter exposes multiple logical streams over one reusable compute queue
/// and serializes compute dispatches. Live allocations retain native SDMA
/// storage, and same-device asynchronous copies can wait on explicit event
/// dependencies. One compute dispatch and SDMA copies may overlap only when
/// their allocation sets are disjoint. An overlapping copy can wait unpublished
/// on an explicit event for the active compute dispatch; an overlapping compute
/// launch is rejected because compute queuing is absent. Persistent buffers are
/// leased from a queue-owned pool, scrubbed as required before recycle, and the
/// pool is trimmed during explicit shutdown. Compute still materializes separate
/// fixed-dispatch storage from the bounded logical host image, so persistent
/// copy storage is not yet a shared compute allocation. The adapter exposes one
/// gfx942 device and no peer copy, multi-device, atomic, or collective operations.
#[must_use = "direct KFD backends must remain owned through quiescence"]
pub struct KfdRuntimeBackendV1 {
    description: BackendDeviceDescriptionV1,
    admitted_device: Option<CheckedGfx942XnackMinusDevice>,
    queue: Option<ComputeAqlQueueSessionV1>,
    terminal_memory: Option<SharedGttMemorySessionV1>,
    terminal_sdma_buffer: Option<Gfx942SdmaBufferV1>,
    queue_retired: bool,
    terminal: bool,
    next_handle: u64,
    streams: HashMap<u64, u64>,
    allocations: HashMap<u64, AllocationRecordV1>,
    modules: HashMap<u64, ModuleRecordV1>,
    kernels: HashMap<u64, KernelRecordV1>,
    submissions: HashMap<u64, SubmissionRecordV1>,
    events: HashMap<u64, EventRecordV1>,
    active: Option<ActiveSubmissionV1>,
    active_sdma: HashMap<u64, ActiveSdmaCopyV1>,
    sdma_dependency_retain_counts: HashMap<u64, usize>,
    resident_data: Option<ResidentDataRosterV1>,
    recycled_dispatch: Option<RecycledDispatchV1>,
    last_launch_performance: Option<KfdRuntimeLaunchPerformanceV1>,
    staging_budgets: StagingBudgetsV1,
    staged_context_bytes: u64,
    sdma_enabled: bool,
    native_available: bool,
    launch_gate: KfdRuntimeLaunchGateV1,
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
                "has_terminal_sdma_buffer",
                &self.terminal_sdma_buffer.is_some(),
            )
            .field("queue_retired", &self.queue_retired)
            .field("terminal", &self.terminal)
            .field("streams", &self.streams.len())
            .field("allocations", &self.allocations.len())
            .field("modules", &self.modules.len())
            .field("kernels", &self.kernels.len())
            .field("submissions", &self.submissions.len())
            .field("events", &self.events.len())
            .field("active", &self.active)
            .field("active_sdma", &self.active_sdma.len())
            .field(
                "sdma_dependency_retain_counts",
                &self.sdma_dependency_retain_counts.len(),
            )
            .field(
                "resident_data",
                &self
                    .resident_data
                    .as_ref()
                    .map(|resident| resident.data.len()),
            )
            .field("last_launch_performance", &self.last_launch_performance)
            .field(
                "recycled_dispatch",
                &self
                    .recycled_dispatch
                    .as_ref()
                    .map(|recycled| recycled.kernel),
            )
            .field("staged_context_bytes", &self.staged_context_bytes)
            .field("sdma_enabled", &self.sdma_enabled)
            .field("staging_budgets", &self.staging_budgets)
            .field("launch_gate", &self.launch_gate)
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
        description: BackendDeviceDescriptionV1,
        admitted_device: Option<CheckedGfx942XnackMinusDevice>,
        launch_gate: KfdRuntimeLaunchGateV1,
        staging_budgets: StagingBudgetsV1,
    ) -> Self {
        let native_available = admitted_device.is_some();
        Self {
            description,
            admitted_device,
            queue: None,
            terminal_memory: None,
            terminal_sdma_buffer: None,
            queue_retired: false,
            terminal: false,
            next_handle: 1,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            modules: HashMap::new(),
            kernels: HashMap::new(),
            submissions: HashMap::new(),
            events: HashMap::new(),
            active: None,
            active_sdma: HashMap::new(),
            sdma_dependency_retain_counts: HashMap::new(),
            resident_data: None,
            recycled_dispatch: None,
            last_launch_performance: None,
            staging_budgets,
            staged_context_bytes: 0,
            sdma_enabled: false,
            native_available,
            launch_gate,
        }
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
        self.active
            .as_ref()
            .is_some_and(|active| active.allocations.contains(&allocation))
            || self
                .active_sdma
                .values()
                .any(|copy| copy.source == allocation || copy.destination == allocation)
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
        if self.active.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cannot change native SDMA ownership while compute is pending",
            ));
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

    fn restore_sdma_copy_buffers_v1(
        &mut self,
        source: u64,
        destination: u64,
        source_buffer: Gfx942SdmaBufferV1,
        destination_buffer: Gfx942SdmaBufferV1,
        destination_dirty: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let source_occupied = self
            .allocations
            .get(&source)
            .map(|record| record.sdma_buffer.is_some());
        let destination_occupied = self
            .allocations
            .get(&destination)
            .map(|record| record.sdma_buffer.is_some());
        if let Err(detail) = validate_sdma_copy_buffer_restore_slots_v1(
            source,
            destination,
            source_occupied,
            destination_occupied,
        ) {
            return Err(self.terminal_error(detail));
        }
        self.allocations
            .get_mut(&source)
            .expect("preflighted SDMA source remains indexed")
            .sdma_buffer = Some(source_buffer);
        let destination_record = self
            .allocations
            .get_mut(&destination)
            .expect("preflighted SDMA destination remains indexed");
        destination_record.sdma_buffer = Some(destination_buffer);
        destination_record.sdma_shadow_dirty |= destination_dirty;
        if destination_dirty {
            destination_record.content_sha256 = None;
            destination_record.last_full_host_write = None;
        }
        Ok(())
    }

    fn finish_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
        completed: fe2o3_kfd::Gfx942SdmaCompletedCopyV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let (source, destination) = completed.into_buffers();
        active.completed_bytes = active
            .completed_bytes
            .checked_add(u64::from(active.packet_bytes))
            .ok_or_else(|| self.terminal_error("SDMA copy progress overflow"))?;
        if active.completed_bytes < active.byte_len {
            let packet_bytes = u32::try_from(
                (active.byte_len - active.completed_bytes)
                    .min(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)),
            )
            .expect("bounded SDMA packet size");
            let source_offset = active
                .source_offset
                .checked_add(active.completed_bytes)
                .ok_or_else(|| self.terminal_error("SDMA source progress overflow"))?;
            let destination_offset = active
                .destination_offset
                .checked_add(active.completed_bytes)
                .ok_or_else(|| self.terminal_error("SDMA destination progress overflow"))?;
            match self
                .queue
                .as_mut()
                .expect("active SDMA submission retains queue")
                .submit_sdma_copy(
                    source,
                    source_offset,
                    destination,
                    destination_offset,
                    packet_bytes,
                ) {
                Ok(ticket) => {
                    active.ticket = Some(ticket);
                    active.packet_bytes = packet_bytes;
                    self.active_sdma.insert(active.id, active);
                    return Ok(BackendPollV1::Pending);
                }
                Err(failure) => {
                    let (error, recovered) = failure.into_parts();
                    if let Some((source, destination)) = recovered {
                        self.restore_sdma_copy_buffers_v1(
                            active.source,
                            active.destination,
                            source,
                            destination,
                            true,
                        )?;
                        self.release_sdma_dependency_retains_v1(&active.dependencies);
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
                        return Ok(status);
                    }
                    return Err(self.terminal_error(format!(
                        "KFD continued SDMA copy publication became ambiguous: {error}"
                    )));
                }
            }
        }
        self.restore_sdma_copy_buffers_v1(
            active.source,
            active.destination,
            source,
            destination,
            true,
        )?;
        self.release_sdma_dependency_retains_v1(&active.dependencies);
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
            },
        );
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
        status
    }

    fn publish_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        for allocation in [active.source, active.destination] {
            if let Err(failure) = self.synchronize_native_allocation_v1(allocation) {
                return match failure {
                    RuntimeBackendFailureV1::Rejected(_)
                    | RuntimeBackendFailureV1::Quiescent(_) => {
                        Ok(self.fail_unpublished_sdma_copy_v1(active))
                    }
                    failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
                };
            }
        }
        let Some(source_buffer) = self
            .allocations
            .get_mut(&active.source)
            .and_then(|record| record.sdma_buffer.take())
        else {
            return Ok(self.fail_unpublished_sdma_copy_v1(active));
        };
        let destination_buffer = match self
            .allocations
            .get_mut(&active.destination)
            .and_then(|record| record.sdma_buffer.take())
        {
            Some(buffer) => buffer,
            None => {
                self.allocations
                    .get_mut(&active.source)
                    .expect("source allocation remains indexed")
                    .sdma_buffer = Some(source_buffer);
                return Ok(self.fail_unpublished_sdma_copy_v1(active));
            }
        };
        let copy_bytes = u32::try_from(
            active
                .byte_len
                .min(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)),
        )
        .expect("bounded SDMA packet size");
        match self
            .queue
            .as_mut()
            .expect("persistent allocations retain their SDMA queue")
            .submit_sdma_copy(
                source_buffer,
                active.source_offset,
                destination_buffer,
                active.destination_offset,
                copy_bytes,
            ) {
            Ok(ticket) => {
                active.packet_bytes = copy_bytes;
                active.ticket = Some(ticket);
                self.active_sdma.insert(active.id, active);
                Ok(BackendPollV1::Pending)
            }
            Err(failure) => {
                let (error, recovered) = failure.into_parts();
                if let Some((source_buffer, destination_buffer)) = recovered {
                    self.restore_sdma_copy_buffers_v1(
                        active.source,
                        active.destination,
                        source_buffer,
                        destination_buffer,
                        false,
                    )?;
                    let _ = error;
                    Ok(self.fail_unpublished_sdma_copy_v1(active))
                } else {
                    Err(self.terminal_error(format!(
                        "KFD SDMA copy publication became ambiguous: {error}"
                    )))
                }
            }
        }
    }

    fn progress_unpublished_sdma_copy_v1(
        &mut self,
        mut active: ActiveSdmaCopyV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        while let Some(dependency) = active.dependencies.get(active.dependency_cursor).copied() {
            match self.poll_v1(dependency)? {
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

    fn recycle_transient_sdma_buffer_v1(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
        operation: &'static str,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match self
            .queue
            .as_mut()
            .expect("transient SDMA buffer retains queue")
            .recycle_sdma_buffer(buffer)
        {
            Ok(()) => Ok(()),
            Err(failure) => {
                let (error, recovered) = failure.into_parts();
                if let Some(buffer) = recovered {
                    // No logical handle can own a transient after this point.
                    // Retain its explicit custody until fail-closed teardown.
                    debug_assert!(self.terminal_sdma_buffer.is_none());
                    self.terminal_sdma_buffer = Some(buffer);
                }
                Err(self.terminal_error(format!(
                    "KFD {operation} transient release became ambiguous: {error}"
                )))
            }
        }
    }

    fn discard_hidden_sdma_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let buffer = self
            .allocations
            .get_mut(&allocation)
            .and_then(|record| record.sdma_buffer.take())
            .ok_or_else(|| {
                self.terminal_error("hidden KFD allocation lost native initialization custody")
            })?;
        let release = self
            .queue
            .as_mut()
            .expect("hidden SDMA allocation retains queue")
            .release_sdma_buffer(buffer);
        if let Err(failure) = release {
            let (error, recovered) = failure.into_parts();
            if let Some(buffer) = recovered {
                self.allocations
                    .get_mut(&allocation)
                    .expect("hidden allocation remains indexed")
                    .sdma_buffer = Some(buffer);
            }
            return Err(self.terminal_error(format!(
                "hidden KFD allocation cleanup became ambiguous: {error}"
            )));
        }
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
            for (index, chunk) in bytes
                .chunks(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
                .enumerate()
            {
                let delta = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or_else(|| Self::capacity("SDMA upload chunk offset overflow"))?;
                self.upload_sdma_range_v1(
                    allocation,
                    byte_offset
                        .checked_add(delta)
                        .ok_or_else(|| Self::capacity("SDMA upload offset overflow"))?,
                    chunk,
                )?;
            }
            return Ok(());
        }
        let mut buffer = self
            .allocations
            .get_mut(&allocation)
            .and_then(|record| record.sdma_buffer.take())
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent SDMA allocation is retained by pending work",
                )
            })?;
        if buffer.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent {
            let result = self
                .queue
                .as_mut()
                .expect("persistent SDMA allocation retains queue")
                .write_sdma_host_buffer(&mut buffer, byte_offset, bytes);
            self.allocations
                .get_mut(&allocation)
                .expect("persistent allocation remains indexed")
                .sdma_buffer = Some(buffer);
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
            .queue
            .as_mut()
            .expect("persistent SDMA allocation retains queue")
            .allocate_sdma_pooled_host_buffer(bytes.len())
            .map_err(|error| self.terminal_error(format!("KFD upload staging: {error}")))?;
        if let Err(error) = self
            .queue
            .as_mut()
            .expect("persistent SDMA allocation retains queue")
            .write_sdma_host_buffer(&mut staging, 0, bytes)
        {
            self.allocations
                .get_mut(&allocation)
                .expect("persistent allocation remains indexed")
                .sdma_buffer = Some(buffer);
            let _ = self.recycle_transient_sdma_buffer_v1(staging, "upload");
            return Err(self.terminal_error(format!("KFD upload staging write: {error}")));
        }
        let ticket = match self
            .queue
            .as_mut()
            .expect("persistent SDMA allocation retains queue")
            .submit_sdma_copy(staging, 0, buffer, byte_offset, copy_bytes)
        {
            Ok(ticket) => ticket,
            Err(failure) => {
                let (error, recovered) = failure.into_parts();
                if let Some((staging, recovered_buffer)) = recovered {
                    self.allocations
                        .get_mut(&allocation)
                        .expect("persistent allocation remains indexed")
                        .sdma_buffer = Some(recovered_buffer);
                    self.recycle_transient_sdma_buffer_v1(staging, "upload")?;
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("KFD upload rejected before publication: {error}"),
                    ));
                }
                return Err(self
                    .terminal_error(format!("KFD upload publication became ambiguous: {error}")));
            }
        };
        let completed = self
            .queue
            .as_mut()
            .expect("published SDMA upload retains queue")
            .wait_sdma_copy_for(ticket, Duration::from_secs(30))
            .map_err(|error| {
                self.terminal_error(format!("KFD upload completion became ambiguous: {error}"))
            })?;
        let (staging, buffer) = completed.into_buffers();
        self.allocations
            .get_mut(&allocation)
            .expect("persistent allocation remains indexed")
            .sdma_buffer = Some(buffer);
        self.recycle_transient_sdma_buffer_v1(staging, "upload")
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
            for (index, chunk) in destination
                .chunks_mut(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
                .enumerate()
            {
                let delta = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or_else(|| Self::capacity("SDMA download chunk offset overflow"))?;
                self.download_sdma_range_v1(
                    allocation,
                    byte_offset
                        .checked_add(delta)
                        .ok_or_else(|| Self::capacity("SDMA download offset overflow"))?,
                    chunk,
                )?;
            }
            return Ok(true);
        }
        let buffer = self
            .allocations
            .get_mut(&allocation)
            .and_then(|record| record.sdma_buffer.take())
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent SDMA allocation is retained by pending work",
                )
            })?;
        if buffer.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent {
            let result = self
                .queue
                .as_mut()
                .expect("persistent SDMA allocation retains queue")
                .read_sdma_host_buffer(&buffer, byte_offset, destination.len() as u64);
            self.allocations
                .get_mut(&allocation)
                .expect("persistent allocation remains indexed")
                .sdma_buffer = Some(buffer);
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
            .queue
            .as_mut()
            .expect("persistent SDMA allocation retains queue")
            .allocate_sdma_pooled_host_buffer(destination.len())
            .map_err(|error| self.terminal_error(format!("KFD download staging: {error}")))?;
        let ticket = match self
            .queue
            .as_mut()
            .expect("persistent SDMA allocation retains queue")
            .submit_sdma_copy(buffer, byte_offset, staging, 0, copy_bytes)
        {
            Ok(ticket) => ticket,
            Err(failure) => {
                let (error, recovered) = failure.into_parts();
                if let Some((recovered_buffer, staging)) = recovered {
                    self.allocations
                        .get_mut(&allocation)
                        .expect("persistent allocation remains indexed")
                        .sdma_buffer = Some(recovered_buffer);
                    self.recycle_transient_sdma_buffer_v1(staging, "download")?;
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("KFD download rejected before publication: {error}"),
                    ));
                }
                return Err(self.terminal_error(format!(
                    "KFD download publication became ambiguous: {error}"
                )));
            }
        };
        let completed = self
            .queue
            .as_mut()
            .expect("published SDMA download retains queue")
            .wait_sdma_copy_for(ticket, Duration::from_secs(30))
            .map_err(|error| {
                self.terminal_error(format!("KFD download completion became ambiguous: {error}"))
            })?;
        let (buffer, staging) = completed.into_buffers();
        let bytes = self
            .queue
            .as_mut()
            .expect("completed SDMA download retains queue")
            .read_sdma_host_buffer(&staging, 0, destination.len() as u64)
            .map_err(|error| self.terminal_error(format!("KFD download readback: {error}")))?;
        destination.copy_from_slice(&bytes);
        self.allocations
            .get_mut(&allocation)
            .expect("persistent allocation remains indexed")
            .sdma_buffer = Some(buffer);
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
        let mut bytes = try_zeroed_staging_v1(byte_len)?;
        self.download_sdma_range_v1(allocation, 0, &mut bytes)?;
        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("persistent allocation remains indexed");
        record.bytes = bytes.into();
        record.sdma_shadow_dirty = false;
        record.content_sha256 = None;
        record.last_full_host_write = None;
        Ok(())
    }

    fn check_dependencies(
        &self,
        dependencies: &[u64],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        for dependency in dependencies {
            let event = self.events.get(dependency).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD event dependency",
                )
            })?;
            let submission = self
                .submissions
                .get(&event.submission)
                .or_else(|| {
                    self.active
                        .as_ref()
                        .filter(|active| active.id == event.submission)
                        .map(|active| {
                            // Only status is inspected below; a pending synthetic
                            // record does not escape this call.
                            let _ = active;
                            &PENDING_SUBMISSION_RECORD_V1
                        })
                })
                .or_else(|| {
                    self.active_sdma
                        .contains_key(&event.submission)
                        .then_some(&PENDING_SUBMISSION_RECORD_V1)
                });
            match submission.map(|record| record.status) {
                Some(BackendPollV1::Succeeded) => {}
                Some(BackendPollV1::Pending) => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "event dependency is still pending",
                    ));
                }
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
        }
        Ok(())
    }

    fn prepare_launch(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<PreparedLaunchV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let preparation_started = Instant::now();
        let dispatch_shape_sha256 = dispatch_shape_sha256_v1(&launch);
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
        prepared: PreparedLaunchV1,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
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
            mut performance,
        } = prepared;
        self.submissions
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD submission-table growth failed"))?;
        let id = self.next_id()?;
        let resident_descriptors = resident_descriptors_v1(&data)?;

        let native_binding_started = Instant::now();
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
                let queue = self
                    .queue
                    .as_mut()
                    .expect("recycled dispatch retains queue");
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
            if self.queue.is_none() {
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
                self.queue = Some(queue);
            } else {
                let rebound = {
                    let queue = self.queue.as_mut().expect("checked queue");
                    let native_data = match self.resident_data.take() {
                        Some(mut resident)
                            if same_resident_storage_shape_v1(
                                &resident.descriptors,
                                &resident_descriptors,
                            ) && data
                                .iter()
                                .all(|spec| spec.kind == RuntimeMemoryKindV1::HostVisible) =>
                        {
                            let overwrite = resident
                                .data
                                .iter_mut()
                                .zip(resident.descriptors.iter().zip(&data))
                                .enumerate()
                                .try_for_each(|(index, (native, (prior, spec)))| {
                                    if !prior.device_may_have_modified
                                        && prior.host_content_sha256.is_some()
                                        && prior.host_content_sha256 == spec.content_sha256
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
                            .and_then(|()| materialize_rebound_data_v1(queue, data, signature)),
                        None => materialize_rebound_data_v1(queue, data, signature),
                    };
                    native_data.and_then(|native_data| {
                        queue
                            .bind_fixed_dispatch(programs, [packet], native_data)
                            .map_err(|error| format!("KFD dispatch rebind: {error}"))
                    })
                };
                if let Err(detail) = rebound {
                    return Err(self.terminal_error(detail));
                }
            }
        }
        performance.native_binding = native_binding_started.elapsed();

        let publication_started = Instant::now();
        let batch = self
            .queue
            .as_mut()
            .expect("queue was created or rebound")
            .submit_fixed_dispatch::<1>()
            .map_err(|error| self.terminal_error(format!("KFD dispatch publication: {error}")))?;
        performance.publication = publication_started.elapsed();
        let published_at = Instant::now();
        self.active = Some(ActiveSubmissionV1 {
            id,
            stream,
            kernel,
            allocations,
            writebacks,
            resident_descriptors,
            dispatch_shape_sha256,
            published_at,
            performance,
            batch: Some(batch),
        });
        Ok(id)
    }

    fn finish_completed(
        &mut self,
        mut active: ActiveSubmissionV1,
        completed: fe2o3_kfd::Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        active.performance.publish_to_completion = active.published_at.elapsed();
        let native_result = (|| -> Result<_, String> {
            let queue = self
                .queue
                .as_mut()
                .expect("active submission retains queue");
            let recycle_started = Instant::now();
            queue
                .recycle_fixed_dispatch(completed)
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
            let record = self
                .allocations
                .get_mut(&writeback.allocation)
                .expect("active allocation remains retained");
            record.content_sha256 = None;
            record.native_dirty.push(NativeDirtyExtentV1 {
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
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
            },
        );
        self.last_launch_performance = Some(active.performance);
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
            || !self.submissions.is_empty()
            || !self.modules.is_empty()
            || !self.allocations.is_empty()
            || self.active.is_some()
            || !self.active_sdma.is_empty()
            || !self.sdma_dependency_retain_counts.is_empty()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "logical runtime resources remain live",
            ));
        }
        self.detach_recycled_dispatch()?;
        self.release_resident_data()?;
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
        if let Some(queue) = self.queue.take() {
            queue.destroy().map_err(|error| {
                self.terminal_error(format!("explicit KFD queue teardown: {error}"))
            })?;
        }
        self.admitted_device.take();
        self.queue_retired = true;
        Ok(())
    }

    fn detach_recycled_dispatch(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.recycled_dispatch.is_some() {
            self.synchronize_all_native_allocations_v1()?;
        }
        let Some(recycled) = self.recycled_dispatch.take() else {
            return Ok(());
        };
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD recycled dispatch exists without a native queue".to_owned())
            .and_then(|queue| {
                queue
                    .detach_recycled_fixed_dispatch()
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

    fn synchronize_all_native_allocations_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut dirty = Vec::new();
        dirty
            .try_reserve_exact(self.allocations.len())
            .map_err(|_| Self::capacity("KFD native-dirty synchronization roster failed"))?;
        dirty.extend(self.allocations.iter().filter_map(|(allocation, record)| {
            (!record.native_dirty.is_empty()).then_some(*allocation)
        }));
        for allocation in dirty {
            self.synchronize_native_allocation_v1(allocation)?;
        }
        Ok(())
    }

    fn release_resident_data(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(resident) = self.resident_data.take() else {
            return Ok(());
        };
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD resident data exists without a native queue".to_owned())
            .and_then(|queue| release_resident_data_v1(queue, resident));
        match result {
            Ok(()) => Ok(()),
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }

    fn synchronize_native_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let dirty = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD allocation",
                )
            })?
            .native_dirty
            .clone();
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
            let queue = self
                .queue
                .as_mut()
                .expect("native-dirty allocation retains its queue");
            queue
                .recycled_fixed_dispatch_generation()
                .map_err(|error| format!("KFD recycled generation before readback: {error}"))
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
                                .map(|readback| (extent.allocation_offset, readback.into_bytes()))
                                .map_err(|error| format!("KFD coherent readback: {error}"))
                        })
                        .collect::<Result<Vec<_>, _>>()
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
        record.native_dirty.clear();
        record.content_sha256 = None;
        let bytes = Arc::clone(&record.bytes);
        let _ = record;
        self.upload_sdma_range_v1(allocation, 0, &bytes)?;
        if let Some(record) = self.allocations.get_mut(&allocation) {
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = false;
        }
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
        let native_result = {
            let queue = self
                .queue
                .as_mut()
                .expect("native-dirty allocation retains its queue");
            queue
                .recycled_fixed_dispatch_generation()
                .map_err(|error| format!("KFD recycled generation before direct read: {error}"))
                .and_then(|generation| {
                    queue
                        .read_recycled_fixed_dispatch_data_into(
                            Gfx942CompletedDispatchReadRequestV1::new(
                                generation,
                                extent.data_index,
                                data_offset,
                                destination.len() as u64,
                            ),
                            destination,
                        )
                        .map_err(|error| format!("KFD direct coherent readback: {error}"))
                })
        };
        match native_result {
            Ok(()) => Ok(true),
            Err(detail) => Err(self.terminal_error(detail)),
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

const PENDING_SUBMISSION_RECORD_V1: SubmissionRecordV1 = SubmissionRecordV1 {
    stream: 0,
    status: BackendPollV1::Pending,
};

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

fn dispatch_shape_sha256_v1(launch: &BackendLaunchV1<'_>) -> [u8; 32] {
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
    digest.finalize().into()
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
    queue: &mut ComputeAqlQueueSessionV1,
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
    queue: &mut ComputeAqlQueueSessionV1,
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

impl RuntimeBackendV1 for KfdRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
        if device != self.description.backend_device || !self.native_available {
            return RuntimeExecutionCapabilitiesV1::default();
        }
        RuntimeExecutionCapabilitiesV1 {
            native_async_copy: true,
            compute_copy_overlap: true,
            memory_pool: true,
            cancellation: true,
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
        let id = self.next_id()?;
        self.streams.insert(id, device);
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
            .active
            .as_ref()
            .is_some_and(|active| active.stream == stream)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream still owns a pending KFD dispatch",
            ));
        }
        if self.active_sdma.values().any(|copy| copy.stream == stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream still owns a pending KFD SDMA copy",
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
        let sdma_buffer = if self.native_available {
            self.ensure_sdma_queue_v1()?;
            let result = match kind {
                RuntimeMemoryKindV1::DeviceLocal => self
                    .queue
                    .as_mut()
                    .expect("native SDMA queue")
                    .allocate_sdma_pooled_device_buffer(byte_len, alignment),
                RuntimeMemoryKindV1::HostVisible => self
                    .queue
                    .as_mut()
                    .expect("native SDMA queue")
                    .allocate_sdma_pooled_host_buffer(len),
            };
            let mut buffer = result.map_err(|error| {
                self.terminal_error(format!("KFD persistent SDMA allocation: {error}"))
            })?;
            if kind == RuntimeMemoryKindV1::HostVisible {
                let initialized = self
                    .queue
                    .as_mut()
                    .expect("native SDMA queue")
                    .write_sdma_host_buffer(&mut buffer, 0, &bytes);
                if let Err(error) = initialized {
                    debug_assert!(self.terminal_sdma_buffer.is_none());
                    self.terminal_sdma_buffer = Some(buffer);
                    return Err(self.terminal_error(format!(
                        "KFD persistent host allocation initialization: {error}"
                    )));
                }
            }
            Some(buffer)
        } else {
            None
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
                sdma_buffer,
                sdma_backed: self.native_available,
                sdma_initialized,
                sdma_shadow_dirty: false,
            },
        );
        self.staged_context_bytes = next_staged_context_bytes;
        if self.native_available && kind == RuntimeMemoryKindV1::DeviceLocal {
            let zero_image = Arc::clone(&self.allocations[&id].bytes);
            if let Err(failure) = self.upload_sdma_range_v1(id, 0, &zero_image) {
                if matches!(failure, RuntimeBackendFailureV1::Terminal(_)) {
                    return Err(failure);
                }
                self.discard_hidden_sdma_allocation_v1(id)?;
                return Err(failure);
            }
            self.allocations
                .get_mut(&id)
                .expect("initialized device allocation remains indexed")
                .sdma_initialized = true;
        }
        Ok(id)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.allocation_is_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending KFD dispatch",
            ));
        }
        if self.recycled_dispatch.as_ref().is_some_and(|recycled| {
            recycled
                .descriptors
                .iter()
                .any(|descriptor| descriptor.allocation == allocation)
        }) {
            self.detach_recycled_dispatch()?;
        }
        if self.resident_data.as_ref().is_some_and(|resident| {
            resident
                .descriptors
                .iter()
                .any(|descriptor| descriptor.allocation == allocation)
        }) {
            self.release_resident_data()?;
        }
        let scrub_device_bytes = self.allocations.get(&allocation).and_then(|record| {
            (record.sdma_backed && record.kind == RuntimeMemoryKindV1::DeviceLocal)
                .then_some(record.bytes.len())
        });
        let scrub = if let Some(byte_len) = scrub_device_bytes {
            let zeros = try_zeroed_staging_v1(byte_len)?;
            self.upload_sdma_range_v1(allocation, 0, &zeros)
        } else {
            Ok(())
        };
        let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        let native = take_sdma_buffer_after_scrub_v1(&mut record.sdma_buffer, scrub)?;
        if let Some(buffer) = native {
            let release = self
                .queue
                .as_mut()
                .expect("persistent SDMA allocation retains its queue")
                .recycle_sdma_buffer(buffer);
            if let Err(failure) = release {
                let (error, recovered) = failure.into_parts();
                if let Some(buffer) = recovered {
                    self.allocations
                        .get_mut(&allocation)
                        .expect("allocation remains retained after recoverable release")
                        .sdma_buffer = Some(buffer);
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("KFD persistent allocation recycle rejected: {error}"),
                    ));
                }
                return Err(self.terminal_error(format!(
                    "KFD persistent allocation recycle became ambiguous: {error}"
                )));
            }
        }
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
        self.synchronize_sdma_shadow_v1(allocation)?;
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

        let attached_index = self.recycled_dispatch.as_ref().and_then(|recycled| {
            recycled
                .descriptors
                .iter()
                .enumerate()
                .find(|(_, descriptor)| {
                    descriptor.allocation == allocation
                        && descriptor.kind == RuntimeMemoryKindV1::HostVisible
                        && descriptor.allocation_offset == byte_offset
                        && descriptor.byte_len == bytes.len() as u64
                })
                .map(|(index, _)| index)
        });
        if attached_index.is_none() && !record.native_dirty.is_empty() {
            self.synchronize_native_allocation_v1(allocation)?;
        }
        if let Some(data_index) = attached_index {
            let native_write = {
                let queue = self
                    .queue
                    .as_mut()
                    .expect("recycled dispatch retains its queue");
                queue
                    .recycled_fixed_dispatch_generation()
                    .map_err(|error| format!("KFD recycled generation before host write: {error}"))
                    .and_then(|generation| {
                        queue
                            .overwrite_recycled_fixed_dispatch_host_data(
                                Gfx942RecycledDispatchWriteRequestV1::new(
                                    generation, data_index, 0,
                                ),
                                bytes,
                            )
                            .map_err(|error| {
                                format!("KFD attached host-visible allocation write: {error}")
                            })
                    })
            };
            if let Err(detail) = native_write {
                return Err(self.terminal_error(detail));
            }
        }

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
        if let Some(data_index) = attached_index {
            record.native_dirty.clear();
            let descriptor = &mut self
                .recycled_dispatch
                .as_mut()
                .expect("attached write retained recycled dispatch")
                .descriptors[data_index];
            descriptor.device_may_have_modified = false;
            descriptor.host_content_sha256 = record.content_sha256;
        }
        self.upload_sdma_range_v1(allocation, byte_offset, bytes)?;
        self.allocations
            .get_mut(&allocation)
            .expect("written allocation remains indexed")
            .sdma_initialized = true;
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
        if self.read_native_allocation_into_v1(allocation, byte_offset, destination)? {
            return Ok(());
        }
        self.synchronize_native_allocation_v1(allocation)?;
        if self.download_sdma_range_v1(allocation, byte_offset, destination)? {
            return Ok(());
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
        let end = offset.checked_add(destination.len()).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation read range overflow",
            )
        })?;
        let source = record.bytes.get(offset..end).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation read is out of bounds",
            )
        })?;
        destination.copy_from_slice(source);
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
        if self.active.as_ref().is_some_and(|active| {
            self.kernels
                .get(&active.kernel)
                .is_some_and(|kernel| kernel.module == module)
        }) {
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
        self.modules.remove(&module);
        self.kernels.retain(|_, kernel| kernel.module != module);
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
        Ok(id)
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.active.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "the admitted KFD queue already has a live dispatch",
            ));
        }
        if launch_overlaps_active_sdma_v1(launch.bindings, self.active_sdma.values()) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "KFD compute bindings overlap a pending SDMA copy",
            ));
        }
        self.check_dependencies(launch.dependencies)?;
        let prepared = self.prepare_launch(launch)?;
        self.publish(prepared)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if let Some(record) = self.submissions.get(&submission) {
            return Ok(record.status);
        }
        if let Some(mut active) = self.active_sdma.remove(&submission) {
            let Some(ticket) = active.ticket.take() else {
                return self.progress_unpublished_sdma_copy_v1(active);
            };
            let poll = self
                .queue
                .as_mut()
                .expect("active SDMA submission retains queue")
                .poll_sdma_copy(ticket)
                .map_err(|error| {
                    self.terminal_error(format!("KFD SDMA completion observation: {error}"))
                })?;
            return match poll {
                Gfx942SdmaCopyPollV1::Pending => {
                    active.ticket = Some(ticket);
                    self.active_sdma.insert(submission, active);
                    Ok(BackendPollV1::Pending)
                }
                Gfx942SdmaCopyPollV1::Completed(completed) => {
                    self.finish_sdma_copy_v1(active, completed)
                }
            };
        }
        let mut active = self.active.take().ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD submission",
            )
        })?;
        if active.id != submission {
            self.active = Some(active);
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD submission",
            ));
        }
        let batch = active
            .batch
            .take()
            .expect("active submission retains batch");
        let poll = self
            .queue
            .as_mut()
            .expect("active submission retains queue")
            .poll_fixed_dispatch(batch)
            .map_err(|error| self.terminal_error(format!("KFD completion observation: {error}")))?;
        match poll {
            Gfx942DispatchPollV1::Pending(batch) => {
                active.batch = Some(batch);
                self.active = Some(active);
                Ok(BackendPollV1::Pending)
            }
            Gfx942DispatchPollV1::Ready(completed) => self.finish_completed(active, completed),
        }
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
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.id == submission)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission still owns a pending KFD dispatch",
            ));
        }
        if self.active_sdma.contains_key(&submission) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission still owns a pending KFD SDMA copy",
            ));
        }
        if self
            .events
            .values()
            .any(|event| event.submission == submission)
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
        self.submissions
            .remove(&submission)
            .map(|_| ())
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD submission",
                )
            })
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
                self.active
                    .as_ref()
                    .filter(|active| active.id == submission)
                    .map(|active| active.stream)
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
        let id = self.next_id()?;
        self.events.insert(id, EventRecordV1 { submission });
        Ok(id)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.events.remove(&event).map(|_| ()).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD event",
            )
        })
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
    Native(RoutedHandleV1),
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
}

#[derive(Debug)]
struct CooperativeCopySubmissionV1 {
    stream: u64,
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
            CooperativeCopyPhaseV1::Dependencies
            | CooperativeCopyPhaseV1::Read
            | CooperativeCopyPhaseV1::Write => BackendPollV1::Pending,
        }
    }

    const fn is_quiescent(&self) -> bool {
        matches!(
            self.phase,
            CooperativeCopyPhaseV1::Succeeded | CooperativeCopyPhaseV1::Failed
        )
    }
}

/// Process-local multi-device KFD router.
///
/// Every selected device is admitted before any child lazily creates a VM or
/// queue, satisfying KFD's process-wide no-queue XNACK barrier. Dispatches on
/// different children can execute independently. Live same-device copies use
/// the selected child's native SDMA path. Peer copies use a bounded,
/// poll-driven host staging state machine; native XGMI is exposed only by
/// [`KfdNativeXgmiRuntimeBackendV1`].
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
    event_submission_retain_counts: HashMap<u64, usize>,
    cooperative_progress_generation: u64,
    cooperative_staging_bytes: u64,
    cooperative_staging_limit_bytes: u64,
}

enum XgmiAllocationAuthorityV1 {
    Unmapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
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

fn xgmi_allocation_is_active_v1<'a>(
    active: impl Iterator<Item = &'a XgmiRuntimeSubmissionV1>,
    allocation: u64,
) -> bool {
    active
        .into_iter()
        .any(|submission| submission.source == allocation || submission.destination == allocation)
}

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
    dependency_retains: usize,
    dependency_depths: usize,
}

impl XgmiLogicalResourceCountsV1 {
    const fn permits_shutdown(self) -> bool {
        self.streams == 0
            && self.allocations == 0
            && self.submissions == 0
            && self.active == 0
            && self.events == 0
            && self.dependency_retains == 0
            && self.dependency_depths == 0
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
/// two directional topology routes, and keeps PUBLIC VRAM unmapped except while
/// a native peer copy owns it. It intentionally does not expose compute launch
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
    events: HashMap<u64, EventRecordV1>,
    dependency_retain_counts: HashMap<u64, usize>,
    dependency_depths: HashMap<u64, usize>,
}

impl fmt::Debug for KfdNativeXgmiRuntimeBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            .field("quarantined_mappings", &quarantined_mappings)
            .field("max_alignment", &max_alignment)
            .field("submissions", &self.submissions.len())
            .field("active", &self.active.len())
            .field("events", &self.events.len())
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
        for (unique_id, authority) in devices {
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
            checked.push((device, authority));
        }
        let mut children = Vec::new();
        children.try_reserve_exact(checked.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device child roster allocation failed",
            )
        })?;
        for (device, authority) in checked {
            children.push(KfdRuntimeBackendV1::from_checked_device_with_gate(
                device,
                KfdRuntimeLaunchGateV1::Production(authority),
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
                    Some(RoutedSubmissionV1::Native(_)) | None => {
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
            CooperativeCopyPhaseV1::Succeeded | CooperativeCopyPhaseV1::Failed
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
        let mut expected_staging_bytes = 0_u64;
        for (submission, record) in &self.submissions {
            let RoutedSubmissionV1::CooperativeCopy(copy) = record else {
                continue;
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

        let mut expected_event_counts = HashMap::<u64, usize>::new();
        for event in self.events.values() {
            let submission = match event {
                RoutedEventV1::Native { submission, .. }
                | RoutedEventV1::CooperativeCopy { submission, .. } => *submission,
            };
            *expected_event_counts.entry(submission).or_insert(0) += 1;
        }
        assert_eq!(self.event_submission_retain_counts, expected_event_counts);
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
            RoutedSubmissionV1::Native(route) => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
        };
        match native_route {
            Some(route) => {
                let result = self.children[route.child].poll_v1(route.local);
                self.latch(result)
            }
            None => Ok(match &self.submissions[&submission] {
                RoutedSubmissionV1::CooperativeCopy(copy) => copy.status(),
                RoutedSubmissionV1::Native(_) => unreachable!(),
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
            RoutedSubmissionV1::Native(_) => {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "native submission routed through cooperative copy progress",
                ));
            }
        };

        match phase {
            CooperativeCopyPhaseV1::Succeeded | CooperativeCopyPhaseV1::Failed => {
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
                    RoutedSubmissionV1::Native(_) => unreachable!(),
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
        let mut dependency_submissions = Vec::new();
        dependency_submissions
            .try_reserve_exact(dependencies.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("copy dependency allocation failed"))?;
        let mut dependency_set = HashSet::new();
        dependency_set
            .try_reserve(dependencies.len())
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
            .is_none_or(|owners| owners.iter().all(|owner| dependency_set.contains(owner)));
        let destination_dependencies_complete = self
            .cooperative_allocation_owners
            .get(&destination_route)
            .is_none_or(|owners| owners.iter().all(|owner| dependency_set.contains(owner)));
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
        self.cooperative_staging_bytes = next_cooperative_staging_bytes;
        self.submissions.insert(
            id,
            RoutedSubmissionV1::CooperativeCopy(CooperativeCopySubmissionV1 {
                stream,
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
            events: HashMap::new(),
            dependency_retain_counts: HashMap::new(),
            dependency_depths: HashMap::new(),
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

    fn map_allocation(
        &mut self,
        allocation: u64,
        direction: usize,
    ) -> Result<Gfx942XgmiMappedDeviceMemoryV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let (owner, lease) = {
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
            let XgmiAllocationAuthorityV1::Unmapped(lease) = authority else {
                record.authority = Some(authority);
                return Err(self.terminal_error("quarantined XGMI mapping was reused"));
            };
            (record.device, lease)
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

    fn unmap_copy_pair(
        &mut self,
        source_allocation: u64,
        destination_allocation: u64,
        direction: usize,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if let Err(failure) = self.unmap_allocation(source_allocation, direction, source) {
            // The first transition made the session state untrustworthy. Do
            // not issue another ioctl; retain the still-mapped peer token.
            self.quarantine_mapping(destination_allocation, destination);
            return Err(failure);
        }
        self.unmap_allocation(destination_allocation, direction, destination)
    }

    fn release_dependencies(&mut self, dependencies: &[u64]) {
        for dependency in dependencies {
            let remove = {
                let count = self
                    .dependency_retain_counts
                    .get_mut(dependency)
                    .expect("active XGMI dependency remains retained");
                *count -= 1;
                *count == 0
            };
            if remove {
                self.dependency_retain_counts.remove(dependency);
            }
        }
    }

    fn finish_failed(&mut self, active: XgmiRuntimeSubmissionV1) -> BackendPollV1 {
        self.release_dependencies(&active.dependencies);
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
        status
    }

    fn allocation_active(&self, allocation: u64) -> bool {
        xgmi_allocation_is_active_v1(self.active.values(), allocation)
    }

    fn quarantine_mapping(&mut self, allocation: u64, mapping: Gfx942XgmiMappedDeviceMemoryV1) {
        self.allocations
            .get_mut(&allocation)
            .expect("XGMI allocation remains indexed")
            .authority = Some(XgmiAllocationAuthorityV1::QuarantinedMapped(mapping));
    }

    fn publish_peer_copy(
        &mut self,
        mut active: XgmiRuntimeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.ensure_queue(active.direction)?;
        let source = match self.map_allocation(active.source, active.direction) {
            Ok(mapping) => mapping,
            Err(RuntimeBackendFailureV1::Rejected(_) | RuntimeBackendFailureV1::Quiescent(_)) => {
                return Ok(self.finish_failed(active));
            }
            Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => return Err(failure),
        };
        let destination = match self.map_allocation(active.destination, active.direction) {
            Ok(mapping) => mapping,
            Err(failure) => {
                self.unmap_allocation(active.source, active.direction, source)?;
                return match failure {
                    RuntimeBackendFailureV1::Rejected(_)
                    | RuntimeBackendFailureV1::Quiescent(_) => Ok(self.finish_failed(active)),
                    failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
                };
            }
        };
        let result = {
            let (source_session, destination_session) =
                Self::session_pair(&mut self.sessions, active.direction);
            self.queues[active.direction]
                .as_mut()
                .expect("directional XGMI queue was established")
                .submit(
                    source_session,
                    destination_session,
                    source,
                    active.source_offset,
                    destination,
                    active.destination_offset,
                    active.byte_len,
                )
        };
        match result {
            Ok(ticket) => {
                active.ticket = Some(ticket);
                self.active.insert(active.id, active);
                Ok(BackendPollV1::Pending)
            }
            Err(Gfx942XgmiCopyFailureV1::Recoverable {
                error,
                source,
                destination,
            }) => {
                self.unmap_copy_pair(
                    active.source,
                    active.destination,
                    active.direction,
                    source,
                    destination,
                )?;
                let _ = error;
                Ok(self.finish_failed(active))
            }
            Err(Gfx942XgmiCopyFailureV1::Retained { error, ticket }) => {
                active.ticket = Some(ticket);
                self.active.insert(active.id, active);
                Err(self.terminal_error(format!(
                    "native XGMI publication retained a ticket: {error}"
                )))
            }
            Err(Gfx942XgmiCopyFailureV1::CompletedCurrentnessIndeterminate {
                error,
                completed,
            }) => {
                let (source, destination) = completed.into_mappings();
                self.quarantine_mapping(active.source, source);
                self.quarantine_mapping(active.destination, destination);
                Err(self.terminal_error(format!(
                    "native XGMI publication completion currentness became ambiguous: {error}"
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
                    self.unmap_copy_pair(
                        active.source,
                        active.destination,
                        active.direction,
                        source,
                        destination,
                    )?;
                    self.release_dependencies(&active.dependencies);
                    let status = BackendPollV1::Succeeded;
                    self.submissions.insert(
                        active.id,
                        SubmissionRecordV1 {
                            stream: active.stream,
                            status,
                        },
                    );
                    Ok(status)
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
        self.publish_peer_copy(active)
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
            dependency_retains: self.dependency_retain_counts.len(),
            dependency_depths: self.dependency_depths.len(),
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
        if self
            .active
            .values()
            .any(|submission| submission.stream == stream)
        {
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
        if self.allocation_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI allocation is retained by pending work",
            ));
        }
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
        let mut full = if byte_offset == 0 && end == byte_len {
            try_copy_vec_v1(bytes, "native XGMI full-write staging allocation failed")?
                .into_boxed_slice()
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
        let record = self.allocations.get(&allocation).ok_or_else(|| {
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
        if end > record.byte_len {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "XGMI read range",
            ));
        }
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
        let active = self.active.remove(&submission).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI submission",
            )
        })?;
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
                .events
                .values()
                .any(|event| event.submission == submission)
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
        self.events.try_reserve(1).map_err(|_| {
            Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, "XGMI event table")
        })?;
        let id = self.next_id()?;
        self.events.insert(id, EventRecordV1 { submission });
        Ok(id)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.events.remove(&event).map(|_| ()).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown XGMI event",
            )
        })
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
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
        if has_active_xgmi_stream_v1(self.active.values(), stream) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI preserves stream order by admitting one pending copy per stream",
            ));
        }
        if has_unordered_xgmi_overlap_v1(
            self.active.values(),
            source.allocation,
            destination.allocation,
            &dependency_submissions,
        ) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping XGMI copies require dependency",
            ));
        }
        self.active.try_reserve(1).map_err(|_| {
            Self::rejected(KfdRuntimeBackendErrorKindV1::Capacity, "XGMI active table")
        })?;
        self.submissions.try_reserve(1).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI completion table",
            )
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
        let id = self.next_id()?;
        for dependency in &dependency_submissions {
            let count = self
                .dependency_retain_counts
                .entry(*dependency)
                .or_insert(0);
            *count = count.checked_add(1).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI dependency count",
                )
            })?;
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
        let all_ready = active.dependencies.iter().all(|dependency| {
            self.submissions
                .get(dependency)
                .is_some_and(|record| record.status == BackendPollV1::Succeeded)
        });
        if all_ready {
            self.publish_peer_copy(active)?;
        } else {
            self.active.insert(id, active);
        }
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
        self.release_dependencies(&active.dependencies);
        self.submissions.insert(
            submission,
            SubmissionRecordV1 {
                stream: active.stream,
                status: BackendPollV1::Failed { code: -2 },
            },
        );
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
            || !self.events.is_empty()
            || !self.dependency_retain_counts.is_empty()
            || !self.dependency_depths.is_empty()
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
        child.execution_capabilities_v1(device)
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
        if self.cooperative_stream_pending_counts.contains_key(&stream) {
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
        });
        let local = self.latch(result)?;
        self.submissions.insert(
            id,
            RoutedSubmissionV1::Native(RoutedHandleV1 {
                child: stream.child,
                local,
            }),
        );
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
            RoutedSubmissionV1::Native(route) => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
        };
        match native_route {
            Some(route) => {
                let result = self.children[route.child].poll_v1(route.local);
                self.latch(result)
            }
            None => self.progress_cooperative_copy(submission),
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
            RoutedSubmissionV1::Native(route) => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
        };
        match native_route {
            Some(route) => {
                let result = self.children[route.child].wait_v1(route.local, deadline);
                self.latch(result)
            }
            None => wait_with_deadline_tracking_progress_v1(deadline, || {
                let progress_before = self.cooperative_progress_generation;
                let status = self.progress_cooperative_copy(submission)?;
                Ok((
                    status,
                    self.cooperative_progress_generation != progress_before,
                ))
            }),
        }
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let route = self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })?;
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
        if let RoutedSubmissionV1::CooperativeCopy(copy) = route
            && !copy.is_quiescent()
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cooperative copy submission is pending",
            ));
        }
        if let RoutedSubmissionV1::Native(route) = route {
            let route = *route;
            let result = self.children[route.child].release_submission_v1(route.local);
            self.latch(result)?;
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
            RoutedSubmissionV1::Native(route) => (Some(*route), None),
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
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(Self::capacity("KFD copy dependency capacity exceeded"));
        }
        let mut dependency_submissions = Vec::new();
        dependency_submissions
            .try_reserve_exact(dependencies.len())
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
        let dependency_depth =
            next_direct_sdma_dependency_depth_v1(&self.active_sdma, &dependency_submissions)
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
        let compute_admission = admit_copy_against_active_compute_v1(
            self.active.as_ref(),
            source.allocation,
            destination.allocation,
            &dependency_submissions,
        );
        if compute_admission == KfdCopyComputeAdmissionV1::Busy {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "KFD copy overlaps active compute without its explicit event dependency",
            ));
        }
        if compute_admission == KfdCopyComputeAdmissionV1::Concurrent {
            if self.active.is_some()
                && [source.allocation, destination.allocation]
                    .into_iter()
                    .any(|allocation| !self.allocations[&allocation].native_dirty.is_empty())
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "disjoint KFD copy requires deferred native-data reconciliation",
                ));
            }
            if self.active.is_none() {
                self.synchronize_native_allocation_v1(source.allocation)?;
                self.synchronize_native_allocation_v1(destination.allocation)?;
            }
        }
        if self.active_sdma.values().any(|active| {
            (active.source == source.allocation
                || active.destination == source.allocation
                || active.source == destination.allocation
                || active.destination == destination.allocation)
                && !dependency_submissions.contains(&active.id)
        }) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping KFD copies require an explicit event dependency",
            ));
        }
        self.active_sdma
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD SDMA submission ledger growth failed"))?;
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
        let id = self.next_id()?;
        for submission in &dependency_submissions {
            *self
                .sdma_dependency_retain_counts
                .entry(*submission)
                .or_insert(0) += 1;
        }
        let active = ActiveSdmaCopyV1 {
            id,
            stream,
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
            ticket: None,
        };
        let all_ready = active.dependencies.iter().all(|submission| {
            self.submissions
                .get(submission)
                .is_some_and(|record| record.status == BackendPollV1::Succeeded)
        });
        if all_ready {
            self.publish_sdma_copy_v1(active)?;
        } else {
            self.active_sdma.insert(id, active);
        }
        Ok(id)
    }
}

impl RuntimeCancellationBackendV1 for KfdRuntimeBackendV1 {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.submissions.contains_key(&submission)
            || self
                .active
                .as_ref()
                .is_some_and(|active| active.id == submission)
        {
            // Direct KFD returns handles only after the AQL/SDMA doorbell has
            // been published. The reviewed queue has no withdrawal primitive.
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        if self
            .active_sdma
            .get(&submission)
            .is_some_and(|active| active.ticket.is_some())
        {
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        if let Some(active) = self.active_sdma.remove(&submission) {
            self.release_sdma_dependency_retains_v1(&active.dependencies);
            self.submissions.insert(
                submission,
                SubmissionRecordV1 {
                    stream: active.stream,
                    status: BackendPollV1::Failed { code: -2 },
                },
            );
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

impl RuntimeAsyncCopyBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
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
            let mut translated_dependencies = Vec::new();
            translated_dependencies
                .try_reserve_exact(dependencies.len())
                .map_err(|_| KfdRuntimeBackendV1::capacity("copy dependency translation failed"))?;
            for event in dependencies {
                if let Some(local) = self.dependency_for_child(*event, stream_route.child)? {
                    translated_dependencies.push(local);
                }
            }
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
                RoutedSubmissionV1::Native(RoutedHandleV1 {
                    child: stream_route.child,
                    local,
                }),
            );
            return Ok(id);
        }
        self.submit_cooperative_copy(stream, source, destination, dependencies, false)
    }
}

impl Drop for KfdRuntimeBackendV1 {
    fn drop(&mut self) {
        if self.terminal
            || self.active.is_some()
            || !self.active_sdma.is_empty()
            || self.terminal_memory.is_some()
            || self.terminal_sdma_buffer.is_some()
        {
            // Native custody may still exist, and Drop cannot return it to the
            // caller. Process termination is the fail-closed transition.
            std::process::abort();
        }
        if let Some(queue) = self.queue.take()
            && queue.destroy().is_err()
        {
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod synthetic_cov6;

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
        let compute = ActiveSubmissionV1 {
            id: 40,
            stream: 4,
            kernel: 9,
            allocations: HashSet::from([10, 11]),
            writebacks: Vec::new(),
            resident_descriptors: Vec::new(),
            dispatch_shape_sha256: [0; 32],
            published_at: Instant::now(),
            performance: KfdRuntimeLaunchPerformanceV1::default(),
            batch: None,
        };
        assert_eq!(
            admit_copy_against_active_compute_v1(Some(&compute), 20, 21, &[]),
            KfdCopyComputeAdmissionV1::Concurrent
        );
        assert_eq!(
            admit_copy_against_active_compute_v1(Some(&compute), 10, 21, &[40]),
            KfdCopyComputeAdmissionV1::DeferredByDependency
        );
        assert_eq!(
            admit_copy_against_active_compute_v1(Some(&compute), 10, 21, &[]),
            KfdCopyComputeAdmissionV1::Busy
        );

        let copy = ActiveSdmaCopyV1 {
            id: 50,
            stream: 5,
            source: 20,
            destination: 21,
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
            completed_bytes: 0,
            packet_bytes: 8,
            dependencies: Vec::new(),
            dependency_cursor: 0,
            dependency_depth: 1,
            ticket: None,
        };
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
        assert!(!launch_overlaps_active_sdma_v1(
            &disjoint,
            [&copy].into_iter()
        ));
        assert!(launch_overlaps_active_sdma_v1(
            &overlapping,
            [&copy].into_iter()
        ));
    }

    #[test]
    fn direct_kfd_sdma_dependency_depth_is_bounded_before_mutation() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
            .unwrap();
        let destination = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1)
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
                ticket: None,
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
            next_direct_sdma_dependency_depth_v1(&backend.active_sdma, &[100]),
            Ok(MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
        );
        backend.active_sdma.get_mut(&100).unwrap().dependency_depth = usize::MAX;
        assert_eq!(
            next_direct_sdma_dependency_depth_v1(&backend.active_sdma, &[100]),
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
    fn direct_kfd_sdma_restore_preflight_is_atomic() {
        assert_eq!(
            validate_sdma_copy_buffer_restore_slots_v1(1, 2, Some(false), Some(false)),
            Ok(())
        );
        for rejected in [
            validate_sdma_copy_buffer_restore_slots_v1(1, 1, Some(false), Some(false)),
            validate_sdma_copy_buffer_restore_slots_v1(1, 2, None, Some(false)),
            validate_sdma_copy_buffer_restore_slots_v1(1, 2, Some(false), None),
            validate_sdma_copy_buffer_restore_slots_v1(1, 2, Some(true), Some(false)),
            validate_sdma_copy_buffer_restore_slots_v1(1, 2, Some(false), Some(true)),
        ] {
            assert!(rejected.is_err());
        }
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
            sdma_buffer: None,
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

        let mut buffer = Some(17_u64);
        assert_eq!(
            take_sdma_buffer_after_scrub_v1(&mut buffer, Err(23_u64)),
            Err(23)
        );
        assert_eq!(buffer, Some(17));
        assert_eq!(
            take_sdma_buffer_after_scrub_v1(&mut buffer, Ok::<(), u64>(())),
            Ok(Some(17))
        );
        assert_eq!(buffer, None);
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
            T: RuntimeBackendV1 + RuntimeAsyncCopyBackendV1 + RuntimeCancellationBackendV1,
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
        for occupied in 0..7 {
            let mut resources = XgmiLogicalResourceCountsV1::default();
            match occupied {
                0 => resources.streams = 1,
                1 => resources.allocations = 1,
                2 => resources.submissions = 1,
                3 => resources.active = 1,
                4 => resources.events = 1,
                5 => resources.dependency_retains = 1,
                6 => resources.dependency_depths = 1,
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
                sdma_buffer: None,
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
        let mut backend = KfdRuntimeBackendV1::mock();
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
        assert!(backend.check_dependencies(&[event]).is_ok());
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
            allocations: active_allocations,
            writebacks: Vec::new(),
            resident_descriptors: Vec::new(),
            dispatch_shape_sha256: [0; 32],
            published_at: Instant::now(),
            performance: KfdRuntimeLaunchPerformanceV1::default(),
            batch: None,
        });
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
                ticket: None,
            },
        );

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
    fn direct_kfd_execution_capabilities_claim_only_implemented_overlap() {
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
        assert!(!capabilities.concurrent_compute);
        assert!(capabilities.compute_copy_overlap);
        backend.native_available = false;

        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut multi = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        multi.children[0].native_available = true;
        let capabilities = multi.execution_capabilities_v1(7);
        assert!(capabilities.native_async_copy);
        assert!(!capabilities.concurrent_compute);
        assert!(capabilities.compute_copy_overlap);
        multi.children[0].native_available = false;
        multi.shutdown_native_v1().unwrap();
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

        assert_eq!(
            backend
                .wait_v1(second, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        backend.assert_cooperative_indexes_consistent();
        assert_eq!(backend.cooperative_dependency_retain_counts[&first], 1);
        assert_eq!(backend.cooperative_stream_pending_counts[&stream], 1);
        assert!(matches!(
            backend.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));

        assert_eq!(
            backend
                .wait_v1(third, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
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

        assert_eq!(
            backend
                .wait_v1(first, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
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
        for submission in [second, third] {
            assert_eq!(
                backend
                    .wait_v1(submission, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
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
    fn cooperative_copy_fan_in_advances_only_one_predecessor_per_poll() {
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

        assert_eq!(backend.poll_v1(dependent).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            &backend.submissions[&first],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.status() == BackendPollV1::Succeeded
        ));
        assert!(matches!(
            &backend.submissions[&second],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.status() == BackendPollV1::Pending
        ));
        let second_destination = backend.allocations[&allocations[3]];
        assert!(
            backend.children[second_destination.child].allocations[&second_destination.local]
                .bytes
                .iter()
                .all(|byte| *byte == 0)
        );

        assert_eq!(backend.poll_v1(dependent).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            &backend.submissions[&dependent],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.dependency_cursor == 1
                    && copy.status() == BackendPollV1::Pending
        ));
        assert!(matches!(
            &backend.submissions[&second],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.status() == BackendPollV1::Pending
        ));

        assert_eq!(backend.poll_v1(dependent).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            &backend.submissions[&second],
            RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.status() == BackendPollV1::Succeeded
        ));
        assert_eq!(
            backend
                .wait_v1(dependent, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
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
            backend.poll_v1(submission),
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
        backend.submissions.remove(&submission);
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
