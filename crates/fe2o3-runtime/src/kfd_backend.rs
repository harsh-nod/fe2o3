//! Pure-Rust KFD implementation of the backend-neutral runtime SPI.
//!
//! The admitted gfx942 KFD surface currently owns one process VM and one
//! reusable native queue. This adapter therefore multiplexes logical streams
//! onto that queue and rejects a second launch while the first is live. It
//! never advertises peer, multi-device, atomic, or collective support.

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
use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, ComputeAqlQueueSessionV1, DeviceSelector,
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, Gfx942CompletedDispatchReadRequestV1,
    Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1, Gfx942DispatchBatchV1,
    Gfx942DispatchBufferBindingV1, Gfx942DispatchPollV1, Gfx942FixedDispatchDataV1,
    Gfx942FixedDispatchPacketV1, Gfx942RecycledDispatchWriteRequestV1,
    HOST_VISIBLE_MEMORY_PAGE_BYTES_V1, OpenedKfd, SharedGttMemorySessionV1,
};
use sha2::{Digest, Sha256};

use crate::{
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, RuntimeAccessV1, RuntimeBackendFailureV1, RuntimeBackendV1,
    RuntimeCapabilitiesV1, RuntimeMemoryKindV1,
};

const KFD_RUNTIME_RING_BYTES_V1: u32 = 64 * 1024;
const COV6_IMPLICIT_KERNARG_BYTES_V1: usize = 256;
const WAIT_SPINS_V1: u32 = 32;
const WAIT_YIELDS_V1: u32 = 8;
const WAIT_INITIAL_SLEEP_V1: Duration = Duration::from_micros(50);
const WAIT_MAX_SLEEP_V1: Duration = Duration::from_millis(1);

/// Maximum host-staged size of one logical direct-KFD allocation.
pub const KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1: u64 = 256 * 1024 * 1024;

/// Maximum aggregate host-staged logical allocation bytes in one backend.
pub const KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1: u64 = 1024 * 1024 * 1024;

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
/// The adapter exposes multiple logical streams over one reusable native queue
/// and serializes them: a second launch is rejected while one dispatch is live,
/// and a dependency on a still-pending event is rejected. `DeviceLocal`
/// allocations are host-staged and materialized into device memory for each
/// launch; only read-only device-local bindings are currently admitted because
/// there is no reviewed device-to-host writeback path. This is not persistent,
/// general-purpose device allocation support. The adapter exposes one gfx942
/// device and no peer copy, multi-device, atomic, or collective operations.
#[must_use = "direct KFD backends must remain owned through quiescence"]
pub struct KfdRuntimeBackendV1 {
    description: BackendDeviceDescriptionV1,
    admitted_device: Option<CheckedGfx942XnackMinusDevice>,
    queue: Option<ComputeAqlQueueSessionV1>,
    terminal_memory: Option<SharedGttMemorySessionV1>,
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
    resident_data: Option<ResidentDataRosterV1>,
    recycled_dispatch: Option<RecycledDispatchV1>,
    last_launch_performance: Option<KfdRuntimeLaunchPerformanceV1>,
    staging_budgets: StagingBudgetsV1,
    staged_context_bytes: u64,
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
            .field("queue_retired", &self.queue_retired)
            .field("terminal", &self.terminal)
            .field("streams", &self.streams.len())
            .field("allocations", &self.allocations.len())
            .field("modules", &self.modules.len())
            .field("kernels", &self.kernels.len())
            .field("submissions", &self.submissions.len())
            .field("events", &self.events.len())
            .field("active", &self.active)
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
        Self {
            description,
            admitted_device,
            queue: None,
            terminal_memory: None,
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
            resident_data: None,
            recycled_dispatch: None,
            last_launch_performance: None,
            staging_budgets,
            staged_context_bytes: 0,
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
            let submission = self.submissions.get(&event.submission).or_else(|| {
                self.active
                    .as_ref()
                    .filter(|active| active.id == event.submission)
                    .map(|active| {
                        // Only status is inspected below; a pending synthetic
                        // record does not escape this call.
                        let _ = active;
                        &PENDING_SUBMISSION_RECORD_V1
                    })
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
        if let Some(recycled) = self.recycled_dispatch.take() {
            if recycled.dispatch_shape_sha256 == dispatch_shape_sha256
                && same_resident_storage_shape_v1(&recycled.descriptors, &resident_descriptors)
                && data
                    .iter()
                    .all(|spec| spec.kind == RuntimeMemoryKindV1::HostVisible)
            {
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
            } else {
                let detached = self
                    .queue
                    .as_mut()
                    .expect("recycled dispatch retains queue")
                    .detach_recycled_fixed_dispatch()
                    .map_err(|error| {
                        self.terminal_error(format!("KFD dispatch detach for rebind: {error}"))
                    })?;
                self.resident_data = Some(ResidentDataRosterV1 {
                    descriptors: recycled.descriptors,
                    data: detached.into_data(),
                });
            }
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
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "logical runtime resources remain live",
            ));
        }
        self.detach_recycled_dispatch()?;
        self.release_resident_data()?;
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
        let descriptors = &self
            .recycled_dispatch
            .as_ref()
            .ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "native-dirty allocation has no recycled dispatch",
                )
            })?
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
    let mut attempts = 0_u32;
    let mut sleep = WAIT_INITIAL_SLEEP_V1;
    loop {
        let status = poll()?;
        if status != BackendPollV1::Pending || Instant::now() >= deadline {
            return Ok(status);
        }
        if attempts < WAIT_SPINS_V1 {
            core::hint::spin_loop();
        } else if attempts < WAIT_SPINS_V1 + WAIT_YIELDS_V1 {
            std::thread::yield_now();
        } else {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(BackendPollV1::Pending);
            }
            std::thread::sleep(sleep.min(remaining));
            sleep = sleep.saturating_mul(2).min(WAIT_MAX_SLEEP_V1);
        }
        attempts = attempts.saturating_add(1);
    }
}

impl RuntimeBackendV1 for KfdRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

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
            },
        );
        self.staged_context_bytes = next_staged_context_bytes;
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

impl Drop for KfdRuntimeBackendV1 {
    fn drop(&mut self) {
        if self.terminal || self.active.is_some() || self.terminal_memory.is_some() {
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
}
