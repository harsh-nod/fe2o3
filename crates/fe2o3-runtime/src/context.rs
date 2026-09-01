//! Backend-neutral runtime context, typed launches, streams, events, and memory.

use core::fmt;
use core::marker::PhantomData;
use fe2o3_runtime_model::{IdentityDigestV1, PeerTransferMechanismV1, TypedAsyncKernelV1};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Maximum number of devices retained by one runtime context.
pub const MAX_RUNTIME_DEVICES_V1: usize = 256;
/// Maximum number of live streams retained by one runtime context.
pub const MAX_RUNTIME_STREAMS_V1: usize = 65_536;
/// Maximum number of live allocations retained by one runtime context.
pub const MAX_RUNTIME_ALLOCATIONS_V1: usize = 1_048_576;
/// Maximum number of live modules retained by one runtime context.
pub const MAX_RUNTIME_MODULES_V1: usize = 65_536;
/// Maximum number of live resolved kernels retained by one runtime context.
pub const MAX_RUNTIME_KERNELS_V1: usize = 1_048_576;
/// Maximum number of live events retained by one runtime context.
pub const MAX_RUNTIME_EVENTS_V1: usize = 1_048_576;
/// Maximum number of live submissions retained by one runtime context.
pub const MAX_RUNTIME_SUBMISSIONS_V1: usize = 1_048_576;
/// Maximum number of explicit dependencies accepted by one launch.
pub const MAX_RUNTIME_DEPENDENCIES_V1: usize = 256;
/// Width of one address patch in an explicit AMDGPU kernarg image.
pub const RUNTIME_DEVICE_POINTER_BYTES_V1: u32 = 8;
/// Maximum explicit kernarg image accepted from a safe argument encoder.
pub const MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1: usize = 1024 * 1024;
/// Maximum module image accepted by the facade, matching the HSACO parser.
pub const MAX_RUNTIME_MODULE_IMAGE_BYTES_V1: usize = fe2o3_hsaco::MAX_HSACO_BYTES;
/// Maximum backend-reported device name length in UTF-8 bytes.
pub const MAX_RUNTIME_DEVICE_NAME_BYTES_V1: usize = 256;
/// Maximum backend-reported target name length in UTF-8 bytes.
pub const MAX_RUNTIME_DEVICE_TARGET_BYTES_V1: usize = 256;
/// Maximum kernel symbol length in UTF-8 bytes.
pub const MAX_RUNTIME_KERNEL_NAME_BYTES_V1: usize = 1024;

static NEXT_CONTEXT_GENERATION_V1: AtomicU64 = AtomicU64::new(1);

macro_rules! runtime_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            context_generation: u64,
            local: u64,
        }

        impl $name {
            /// Returns the nonzero context-local identity value.
            pub const fn get(self) -> u64 {
                self.local
            }

            const fn new(context_generation: u64, local: u64) -> Self {
                Self {
                    context_generation,
                    local,
                }
            }
        }
    };
}

runtime_id!(RuntimeDeviceIdV1);
runtime_id!(RuntimeStreamIdV1);
runtime_id!(RuntimeAllocationIdV1);
runtime_id!(RuntimeModuleIdV1);
runtime_id!(RuntimeEventIdV1);
runtime_id!(RuntimeSubmissionIdV1);

/// Stable capability inventory reported for one concrete backend device.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeCapabilitiesV1 {
    pub typed_async_launch: bool,
    pub streams: bool,
    pub events: bool,
    pub device_memory: bool,
    pub host_visible_memory: bool,
    pub peer_copy: bool,
    pub multi_device: bool,
    pub atomics: bool,
    pub collectives: bool,
}

/// Backend-reported immutable device description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDeviceDescriptionV1 {
    pub backend_device: u64,
    pub name: String,
    pub target: String,
    pub global_memory_bytes: u64,
    pub capabilities: RuntimeCapabilitiesV1,
}

/// Runtime-visible immutable device description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDeviceV1 {
    id: RuntimeDeviceIdV1,
    backend_device: u64,
    name: String,
    target: String,
    global_memory_bytes: u64,
    capabilities: RuntimeCapabilitiesV1,
}

impl RuntimeDeviceV1 {
    pub const fn id(&self) -> RuntimeDeviceIdV1 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn global_memory_bytes(&self) -> u64 {
        self.global_memory_bytes
    }

    pub const fn capabilities(&self) -> RuntimeCapabilitiesV1 {
        self.capabilities
    }
}

/// Memory placement selected for a runtime allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMemoryKindV1 {
    DeviceLocal,
    HostVisible,
}

/// Access declared for one launch binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAccessV1 {
    Read,
    Write,
    ReadWrite,
}

/// Address-free, allocation-relative memory region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeMemoryRegionV1 {
    pub allocation: RuntimeAllocationIdV1,
    pub access: RuntimeAccessV1,
    pub byte_offset: u64,
    pub byte_len: u64,
}

/// Allocation region and device-pointer patch carried by a launch request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBindingV1 {
    pub region: RuntimeMemoryRegionV1,
    /// Byte offset of the eight-byte device pointer slot in explicit kernarg.
    pub kernarg_byte_offset: u32,
}

/// Encoded arguments for a typed kernel launch.
pub trait RuntimeArgumentsV1: Send + Sync + 'static {
    /// Stable application-defined signature commitment.
    const SIGNATURE_V1: [u8; 32];

    /// Produces the address-free explicit kernarg image.
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8>;

    /// Produces allocation-relative memory effects in argument order.
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1>;
}

/// A module-resolved kernel bound to one Rust argument type.
pub struct TypedRuntimeKernelV1<A> {
    module: RuntimeModuleIdV1,
    backend_kernel: u64,
    name: String,
    signature: [u8; 32],
    model_kernel: TypedAsyncKernelV1<A>,
    marker: PhantomData<fn(A) -> A>,
}

impl<A> fmt::Debug for TypedRuntimeKernelV1<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypedRuntimeKernelV1")
            .field("module", &self.module)
            .field("name", &self.name)
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

impl<A> TypedRuntimeKernelV1<A> {
    /// Returns the pure-model kernel identity paired with the sealed backend handle.
    pub const fn model_identity(&self) -> IdentityDigestV1 {
        self.model_kernel.identity()
    }
}

/// Three-dimensional grid and workgroup geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLaunchGeometryV1 {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub dynamic_shared_bytes: u32,
}

impl RuntimeLaunchGeometryV1 {
    pub fn validate(self) -> Result<Self, RuntimeValidationErrorV1> {
        if self.grid.contains(&0) || self.workgroup.contains(&0) {
            return Err(RuntimeValidationErrorV1::ZeroGeometry);
        }
        self.workgroup
            .into_iter()
            .try_fold(1_u32, u32::checked_mul)
            .ok_or(RuntimeValidationErrorV1::GeometryOverflow)?;
        Ok(self)
    }
}

/// Backend launch description after all context-local validation.
pub struct BackendLaunchV1<'a> {
    pub stream: u64,
    pub kernel: u64,
    pub explicit_kernarg: &'a [u8],
    pub bindings: &'a [BackendBindingV1],
    pub dependencies: &'a [u64],
    pub geometry: RuntimeLaunchGeometryV1,
}

/// Backend allocation-relative region translated from a stable runtime handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendMemoryRegionV1 {
    pub allocation: u64,
    pub access: RuntimeAccessV1,
    pub byte_offset: u64,
    pub byte_len: u64,
}

/// Backend launch region and address-free device-pointer patch location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendBindingV1 {
    pub region: BackendMemoryRegionV1,
    /// Address-free location the backend must patch with this allocation view.
    pub kernarg_byte_offset: u32,
}

/// Result of a nonblocking backend completion observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendPollV1 {
    Pending,
    Succeeded,
    Failed { code: i64 },
}

/// Failure class reported across the backend boundary.
#[derive(Debug)]
pub enum RuntimeBackendFailureV1<E> {
    /// Rejected before any device-visible mutation.
    Rejected(E),
    /// Conclusive failure after all referenced resources became quiescent.
    Quiescent(E),
    /// The backend may still reference submitted resources.
    Terminal(E),
}

/// Sealed-resource backend SPI implemented by KFD, HSA, or a worker client.
///
/// Implementations must return nonzero handles that are unique among live
/// resources of the same kind. A successful call transfers the described
/// resource custody to the caller; a release consumes that custody only on
/// `Ok`. Modules and allocations must remain retained while a live submission
/// can reference them. Events must retain the source completion state until
/// `release_event_v1` succeeds, including when later submissions depend on the
/// event.
///
/// `submit_v1` is nonblocking: success means the backend has accepted custody
/// and returns a submission handle, not that execution completed. `poll_v1`
/// must not block. `wait_v1` must not wait past its monotonic deadline. A
/// `Succeeded` or `Failed` completion observation is conclusive quiescence for
/// all resources referenced by that submission; `Pending` retains custody.
/// `release_submission_v1` is valid only after such quiescence (or after a
/// successful stream destroy established it) and must not invalidate events
/// that still retain the completion state.
///
/// Failure classes are part of the safety contract:
///
/// - `Rejected` means no device-visible mutation occurred and all prior
///   custody remains unchanged.
/// - `Quiescent` means mutation may have occurred, but every native reference
///   involved in the operation is conclusively quiescent. The facade retains
///   its logical handle so the caller can inspect or retry cleanup.
/// - `Terminal` means native mutation or quiescence is ambiguous. The backend
///   must reject all subsequent operations and retain possibly referenced
///   resources rather than freeing them.
pub trait RuntimeBackendV1 {
    type Error: Error + Send + Sync + 'static;

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>>;

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    /// Destroys a stream only after all of its submitted work is quiescent.
    ///
    /// `Ok` and `Quiescent` both assert that no submission on this stream can
    /// retain a module, allocation, or event. `Rejected` makes no such
    /// assertion, and `Terminal` means resource reachability is ambiguous.
    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn unload_module_v1(&mut self, module: u64)
    -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>>;

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>>;

    /// Releases backend-owned completion state for a quiescent submission.
    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

/// Validation failure before entering a backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeValidationErrorV1 {
    Capacity,
    Unsupported,
    ContextTerminal,
    InvalidBackendDescription,
    InvalidKernelSignature,
    UnknownDevice,
    UnknownStream,
    UnknownAllocation,
    UnknownModule,
    UnknownKernel,
    UnknownEvent,
    UnknownSubmission,
    SubmissionPending,
    SubmissionRetainedByEvent,
    WrongDevice,
    InvalidAlignment,
    InvalidRange,
    InvalidAccess,
    InvalidKernargPatch,
    KernargTooLarge,
    TooManyBindings,
    InvalidDeadline,
    EmptyModule,
    ModuleTooLarge,
    EmptyKernelName,
    KernelNameTooLong,
    InvalidKernelName,
    TooManyDependencies,
    DuplicateDependency,
    ZeroGeometry,
    GeometryOverflow,
}

impl fmt::Display for RuntimeValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for RuntimeValidationErrorV1 {}

/// Resource namespace associated with a backend protocol violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBackendResourceKindV1 {
    Stream,
    Allocation,
    Module,
    Kernel,
    Submission,
    Event,
}

/// A successful backend call returned a handle that cannot represent new custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBackendProtocolErrorV1 {
    ZeroHandle(RuntimeBackendResourceKindV1),
    DuplicateHandle(RuntimeBackendResourceKindV1),
}

impl fmt::Display for RuntimeBackendProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for RuntimeBackendProtocolErrorV1 {}

/// Public runtime failure preserving the terminal-ambiguity distinction.
#[derive(Debug)]
pub enum RuntimeErrorV1<E> {
    Validation(RuntimeValidationErrorV1),
    /// The backend reported successful mutation but returned an invalid handle.
    BackendProtocol(RuntimeBackendProtocolErrorV1),
    BackendRejected(E),
    BackendQuiescent(E),
    /// The backend must be considered lost; callers must not release retained resources in-process.
    BackendTerminal(E),
}

impl<E: fmt::Display> fmt::Display for RuntimeErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "runtime validation failed: {error}"),
            Self::BackendProtocol(error) => {
                write!(
                    formatter,
                    "backend protocol violation after mutation: {error}"
                )
            }
            Self::BackendRejected(error) => {
                write!(formatter, "backend rejected operation: {error}")
            }
            Self::BackendQuiescent(error) => {
                write!(formatter, "backend failed after quiescence: {error}")
            }
            Self::BackendTerminal(error) => write!(
                formatter,
                "backend entered terminal ambiguous state: {error}"
            ),
        }
    }
}

impl<E: Error + 'static> Error for RuntimeErrorV1<E> {}

impl<E> From<RuntimeValidationErrorV1> for RuntimeErrorV1<E> {
    fn from(value: RuntimeValidationErrorV1) -> Self {
        Self::Validation(value)
    }
}

/// Context-owned resource selected for deterministic cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCleanupResourceV1 {
    Stream(RuntimeStreamIdV1),
    Event(RuntimeEventIdV1),
    Submission(RuntimeSubmissionIdV1),
    Module(RuntimeModuleIdV1),
    Allocation(RuntimeAllocationIdV1),
}

/// One cleanup operation that did not conclusively release its resource.
#[derive(Debug)]
pub struct RuntimeCleanupFailureV1<E> {
    resource: RuntimeCleanupResourceV1,
    failure: RuntimeBackendFailureV1<E>,
}

impl<E> RuntimeCleanupFailureV1<E> {
    pub const fn resource(&self) -> RuntimeCleanupResourceV1 {
        self.resource
    }

    pub const fn failure(&self) -> &RuntimeBackendFailureV1<E> {
        &self.failure
    }
}

/// Counts of context handles retained after a cleanup pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeRetainedResourcesV1 {
    pub streams: usize,
    pub events: usize,
    pub submissions: usize,
    pub modules: usize,
    pub allocations: usize,
}

impl RuntimeRetainedResourcesV1 {
    pub const fn is_empty(self) -> bool {
        self.streams == 0
            && self.events == 0
            && self.submissions == 0
            && self.modules == 0
            && self.allocations == 0
    }
}

/// Result of one deterministic context cleanup pass.
#[derive(Debug)]
pub struct RuntimeCleanupReportV1<E> {
    failures: Vec<RuntimeCleanupFailureV1<E>>,
    retained: RuntimeRetainedResourcesV1,
    terminal: bool,
}

impl<E> RuntimeCleanupReportV1<E> {
    pub fn failures(&self) -> &[RuntimeCleanupFailureV1<E>] {
        &self.failures
    }

    pub const fn retained(&self) -> RuntimeRetainedResourcesV1 {
        self.retained
    }

    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub const fn is_complete(&self) -> bool {
        !self.terminal && self.retained.is_empty()
    }
}

fn map_backend_error<E>(error: RuntimeBackendFailureV1<E>) -> RuntimeErrorV1<E> {
    match error {
        RuntimeBackendFailureV1::Rejected(error) => RuntimeErrorV1::BackendRejected(error),
        RuntimeBackendFailureV1::Quiescent(error) => RuntimeErrorV1::BackendQuiescent(error),
        RuntimeBackendFailureV1::Terminal(error) => RuntimeErrorV1::BackendTerminal(error),
    }
}

#[derive(Clone, Copy, Debug)]
struct StreamRecordV1 {
    backend_stream: u64,
    device: RuntimeDeviceIdV1,
}

#[derive(Clone, Copy, Debug)]
struct AllocationRecordV1 {
    backend_allocation: u64,
    device: RuntimeDeviceIdV1,
    byte_len: u64,
}

#[derive(Clone, Copy, Debug)]
struct ModuleRecordV1 {
    backend_module: u64,
    device: RuntimeDeviceIdV1,
    image_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
struct KernelRecordV1 {
    module: RuntimeModuleIdV1,
}

#[derive(Clone, Copy, Debug)]
struct EventRecordV1 {
    backend_event: u64,
    device: RuntimeDeviceIdV1,
    submission: RuntimeSubmissionIdV1,
}

#[derive(Clone, Copy, Debug)]
struct SubmissionRecordV1 {
    backend_submission: u64,
    stream: RuntimeStreamIdV1,
    device: RuntimeDeviceIdV1,
    quiescent: bool,
}

/// One independently owned runtime backend and all of its context-local handles.
///
/// Before normal shutdown, observe every submission to a terminal result,
/// release events that retain it, consume [`Self::release_submission`], destroy
/// streams, and then call [`Self::shutdown`]. `cleanup` performs a deterministic
/// best-effort version of that ordering while retaining every failed handle.
/// Direct native backends may abort from their own `Drop` implementation when
/// live or ambiguous GPU custody remains, so applications should prefer the
/// supervised worker transport and always perform explicit shutdown.
#[must_use = "runtime contexts retain backend resources until shutdown succeeds"]
pub struct RuntimeContextV1<B: RuntimeBackendV1> {
    backend: B,
    context_generation: u64,
    devices: Vec<RuntimeDeviceV1>,
    streams: HashMap<RuntimeStreamIdV1, StreamRecordV1>,
    backend_streams: HashSet<u64>,
    allocations: HashMap<RuntimeAllocationIdV1, AllocationRecordV1>,
    backend_allocations: HashSet<u64>,
    modules: HashMap<RuntimeModuleIdV1, ModuleRecordV1>,
    backend_modules: HashSet<u64>,
    kernels: HashMap<u64, KernelRecordV1>,
    events: HashMap<RuntimeEventIdV1, EventRecordV1>,
    backend_events: HashSet<u64>,
    submissions: HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    backend_submissions: HashSet<u64>,
    next_identity: u64,
    terminal: bool,
}

/// A failed consuming shutdown retaining the context and every unreleased handle.
pub struct RuntimeContextShutdownFailureV1<B: RuntimeBackendV1> {
    context: Box<RuntimeContextV1<B>>,
    report: RuntimeCleanupReportV1<B::Error>,
}

/// Failed consuming submission release, retaining the move-only token.
pub struct RuntimeSubmissionReleaseFailureV1<A, E> {
    submission: RuntimeSubmissionV1<A>,
    error: RuntimeErrorV1<E>,
}

impl<A, E: fmt::Debug> fmt::Debug for RuntimeSubmissionReleaseFailureV1<A, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeSubmissionReleaseFailureV1")
            .field("submission", &self.submission.id)
            .field("error", &self.error)
            .finish()
    }
}

impl<A, E> RuntimeSubmissionReleaseFailureV1<A, E> {
    pub const fn submission(&self) -> &RuntimeSubmissionV1<A> {
        &self.submission
    }

    pub const fn error(&self) -> &RuntimeErrorV1<E> {
        &self.error
    }

    pub fn into_parts(self) -> (RuntimeSubmissionV1<A>, RuntimeErrorV1<E>) {
        (self.submission, self.error)
    }
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeContextShutdownFailureV1<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeContextShutdownFailureV1")
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

impl<B: RuntimeBackendV1> RuntimeContextShutdownFailureV1<B> {
    pub fn context(&self) -> &RuntimeContextV1<B> {
        self.context.as_ref()
    }

    pub const fn report(&self) -> &RuntimeCleanupReportV1<B::Error> {
        &self.report
    }

    pub fn into_context(self) -> RuntimeContextV1<B> {
        *self.context
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub fn open(mut backend: B) -> Result<Self, RuntimeErrorV1<B::Error>> {
        let descriptions = backend.enumerate_devices_v1().map_err(map_backend_error)?;
        if descriptions.len() > MAX_RUNTIME_DEVICES_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        for (index, device) in descriptions.iter().enumerate() {
            if device.backend_device == 0
                || device.name.is_empty()
                || device.name.len() > MAX_RUNTIME_DEVICE_NAME_BYTES_V1
                || device.target.is_empty()
                || device.target.len() > MAX_RUNTIME_DEVICE_TARGET_BYTES_V1
                || descriptions[..index]
                    .iter()
                    .any(|prior| prior.backend_device == device.backend_device)
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            }
        }
        let context_generation = NEXT_CONTEXT_GENERATION_V1
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |generation| {
                generation.checked_add(1)
            })
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let devices = descriptions
            .into_iter()
            .enumerate()
            .map(|(index, device)| RuntimeDeviceV1 {
                id: RuntimeDeviceIdV1::new(context_generation, index as u64 + 1),
                backend_device: device.backend_device,
                name: device.name,
                target: device.target,
                global_memory_bytes: device.global_memory_bytes,
                capabilities: device.capabilities,
            })
            .collect();
        Ok(Self {
            backend,
            context_generation,
            devices,
            streams: HashMap::new(),
            backend_streams: HashSet::new(),
            allocations: HashMap::new(),
            backend_allocations: HashSet::new(),
            modules: HashMap::new(),
            backend_modules: HashSet::new(),
            kernels: HashMap::new(),
            events: HashMap::new(),
            backend_events: HashSet::new(),
            submissions: HashMap::new(),
            backend_submissions: HashSet::new(),
            next_identity: 1,
            terminal: false,
        })
    }

    pub fn devices(&self) -> &[RuntimeDeviceV1] {
        &self.devices
    }

    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Performs one deterministic cleanup pass without discarding retained handles.
    ///
    /// Streams are destroyed first because a successful destroy, or a
    /// `Quiescent` failure, is the backend's assertion that submitted work no
    /// longer references context resources. Events, submission state, modules,
    /// and allocations are then released in that order. IDs are processed in
    /// ascending order.
    /// A rejected stream destroy blocks all dependent cleanup, and a rejected
    /// event release blocks module and allocation cleanup because that event
    /// may retain submission resources. A terminal failure stops the pass
    /// immediately and permanently seals the context.
    pub fn cleanup(&mut self) -> RuntimeCleanupReportV1<B::Error> {
        let mut failures = Vec::new();
        if self.terminal {
            return self.cleanup_report(failures);
        }

        let mut stream_ids: Vec<_> = self.streams.keys().copied().collect();
        stream_ids.sort_unstable();
        let mut streams_quiescent = true;
        for id in stream_ids {
            let record = self.streams[&id];
            match self.backend.destroy_stream_v1(record.backend_stream) {
                Ok(()) => {
                    self.mark_stream_quiescent(id);
                    self.streams.remove(&id);
                    self.backend_streams.remove(&record.backend_stream);
                }
                Err(failure) => {
                    if matches!(failure, RuntimeBackendFailureV1::Rejected(_)) {
                        streams_quiescent = false;
                    }
                    if matches!(failure, RuntimeBackendFailureV1::Quiescent(_)) {
                        self.mark_stream_quiescent(id);
                    }
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Stream(id),
                        failure,
                    });
                    if terminal {
                        self.terminal = true;
                        return self.cleanup_report(failures);
                    }
                }
            }
        }
        if !streams_quiescent {
            return self.cleanup_report(failures);
        }

        let mut event_ids: Vec<_> = self.events.keys().copied().collect();
        event_ids.sort_unstable();
        let mut events_released_or_quiescent = true;
        for id in event_ids {
            let record = self.events[&id];
            match self.backend.release_event_v1(record.backend_event) {
                Ok(()) => {
                    self.events.remove(&id);
                    self.backend_events.remove(&record.backend_event);
                }
                Err(failure) => {
                    if matches!(failure, RuntimeBackendFailureV1::Rejected(_)) {
                        events_released_or_quiescent = false;
                    }
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Event(id),
                        failure,
                    });
                    if terminal {
                        self.terminal = true;
                        return self.cleanup_report(failures);
                    }
                }
            }
        }
        if !events_released_or_quiescent {
            return self.cleanup_report(failures);
        }

        let mut submission_ids: Vec<_> = self.submissions.keys().copied().collect();
        submission_ids.sort_unstable();
        let mut submissions_released = true;
        for id in submission_ids {
            let record = self.submissions[&id];
            if !record.quiescent {
                debug_assert!(false, "destroyed stream left a live submission");
                submissions_released = false;
                continue;
            }
            match self
                .backend
                .release_submission_v1(record.backend_submission)
            {
                Ok(()) => {
                    self.submissions.remove(&id);
                    self.backend_submissions.remove(&record.backend_submission);
                }
                Err(failure) => {
                    submissions_released = false;
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Submission(id),
                        failure,
                    });
                    if terminal {
                        self.terminal = true;
                        return self.cleanup_report(failures);
                    }
                }
            }
        }
        if !submissions_released {
            return self.cleanup_report(failures);
        }

        let mut module_ids: Vec<_> = self.modules.keys().copied().collect();
        module_ids.sort_unstable();
        for id in module_ids {
            let record = self.modules[&id];
            match self.backend.unload_module_v1(record.backend_module) {
                Ok(()) => {
                    self.modules.remove(&id);
                    self.backend_modules.remove(&record.backend_module);
                    self.kernels.retain(|_, kernel| kernel.module != id);
                }
                Err(failure) => {
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Module(id),
                        failure,
                    });
                    if terminal {
                        self.terminal = true;
                        return self.cleanup_report(failures);
                    }
                }
            }
        }

        let mut allocation_ids: Vec<_> = self.allocations.keys().copied().collect();
        allocation_ids.sort_unstable();
        for id in allocation_ids {
            let record = self.allocations[&id];
            match self
                .backend
                .release_allocation_v1(record.backend_allocation)
            {
                Ok(()) => {
                    self.allocations.remove(&id);
                    self.backend_allocations.remove(&record.backend_allocation);
                }
                Err(failure) => {
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Allocation(id),
                        failure,
                    });
                    if terminal {
                        self.terminal = true;
                        return self.cleanup_report(failures);
                    }
                }
            }
        }

        self.cleanup_report(failures)
    }

    /// Cleans every context-owned handle and returns the backend only on success.
    ///
    /// Failure returns the still-owning context so quiescent or rejected
    /// operations may be inspected and retried. Terminal contexts retain their
    /// symbolic handle custody but will never call the lost backend again.
    pub fn shutdown(mut self) -> Result<B, RuntimeContextShutdownFailureV1<B>> {
        let report = self.cleanup();
        if report.is_complete() {
            Ok(self.backend)
        } else {
            Err(RuntimeContextShutdownFailureV1 {
                context: Box::new(self),
                report,
            })
        }
    }

    fn cleanup_report(
        &self,
        failures: Vec<RuntimeCleanupFailureV1<B::Error>>,
    ) -> RuntimeCleanupReportV1<B::Error> {
        RuntimeCleanupReportV1 {
            failures,
            retained: RuntimeRetainedResourcesV1 {
                streams: self.streams.len(),
                events: self.events.len(),
                submissions: self.submissions.len(),
                modules: self.modules.len(),
                allocations: self.allocations.len(),
            },
            terminal: self.terminal,
        }
    }

    fn next_id(&mut self) -> Result<u64, RuntimeValidationErrorV1> {
        let identity = self.next_identity;
        self.next_identity = identity
            .checked_add(1)
            .ok_or(RuntimeValidationErrorV1::Capacity)?;
        Ok(identity)
    }

    fn mark_stream_quiescent(&mut self, stream: RuntimeStreamIdV1) {
        for submission in self.submissions.values_mut() {
            if submission.stream == stream {
                submission.quiescent = true;
            }
        }
    }

    fn require_live(&self) -> Result<(), RuntimeValidationErrorV1> {
        if self.terminal {
            Err(RuntimeValidationErrorV1::ContextTerminal)
        } else {
            Ok(())
        }
    }

    fn submission_record<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<SubmissionRecordV1, RuntimeValidationErrorV1> {
        if submission.id.context_generation != self.context_generation {
            return Err(RuntimeValidationErrorV1::UnknownSubmission);
        }
        let record = *self
            .submissions
            .get(&submission.id)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
        if record.backend_submission != submission.backend_submission
            || record.stream != submission.stream
            || record.device != submission.device
        {
            return Err(RuntimeValidationErrorV1::UnknownSubmission);
        }
        Ok(record)
    }

    fn live_submission_record<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<SubmissionRecordV1, RuntimeValidationErrorV1> {
        let record = self.submission_record(submission)?;
        let stream = self
            .streams
            .get(&record.stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        if stream.device != record.device {
            return Err(RuntimeValidationErrorV1::WrongDevice);
        }
        Ok(record)
    }

    fn backend_result<T>(
        &mut self,
        result: Result<T, RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<T, RuntimeErrorV1<B::Error>> {
        match result {
            Ok(value) => Ok(value),
            Err(RuntimeBackendFailureV1::Rejected(error)) => {
                Err(RuntimeErrorV1::BackendRejected(error))
            }
            Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                Err(RuntimeErrorV1::BackendQuiescent(error))
            }
            Err(RuntimeBackendFailureV1::Terminal(error)) => {
                self.terminal = true;
                Err(RuntimeErrorV1::BackendTerminal(error))
            }
        }
    }

    fn backend_handle_protocol_error(
        &self,
        resource: RuntimeBackendResourceKindV1,
        handle: u64,
    ) -> Option<RuntimeBackendProtocolErrorV1> {
        if handle == 0 {
            return Some(RuntimeBackendProtocolErrorV1::ZeroHandle(resource));
        }
        let duplicate = match resource {
            RuntimeBackendResourceKindV1::Stream => self.backend_streams.contains(&handle),
            RuntimeBackendResourceKindV1::Allocation => self.backend_allocations.contains(&handle),
            RuntimeBackendResourceKindV1::Module => self.backend_modules.contains(&handle),
            RuntimeBackendResourceKindV1::Kernel => self.kernels.contains_key(&handle),
            RuntimeBackendResourceKindV1::Submission => self.backend_submissions.contains(&handle),
            RuntimeBackendResourceKindV1::Event => self.backend_events.contains(&handle),
        };
        duplicate.then_some(RuntimeBackendProtocolErrorV1::DuplicateHandle(resource))
    }

    fn seal_backend_protocol<T>(
        &mut self,
        error: Option<RuntimeBackendProtocolErrorV1>,
        value: T,
    ) -> Result<T, RuntimeErrorV1<B::Error>> {
        if let Some(error) = error {
            self.terminal = true;
            Err(RuntimeErrorV1::BackendProtocol(error))
        } else {
            Ok(value)
        }
    }

    fn device(&self, id: RuntimeDeviceIdV1) -> Result<&RuntimeDeviceV1, RuntimeValidationErrorV1> {
        if id.context_generation != self.context_generation {
            return Err(RuntimeValidationErrorV1::UnknownDevice);
        }
        self.devices
            .get(
                id.local
                    .checked_sub(1)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(usize::MAX),
            )
            .filter(|device| device.id == id)
            .ok_or(RuntimeValidationErrorV1::UnknownDevice)
    }

    pub fn create_stream(
        &mut self,
        device: RuntimeDeviceIdV1,
    ) -> Result<RuntimeStreamIdV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if self.streams.len() >= MAX_RUNTIME_STREAMS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        let device_record = self.device(device)?;
        if !device_record.capabilities.streams {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let backend_device = device_record.backend_device;
        let id = RuntimeStreamIdV1::new(self.context_generation, self.next_id()?);
        let result = self.backend.create_stream_v1(backend_device);
        let backend_stream = self.backend_result(result)?;
        let protocol_error = self
            .backend_handle_protocol_error(RuntimeBackendResourceKindV1::Stream, backend_stream);
        self.streams.insert(
            id,
            StreamRecordV1 {
                backend_stream,
                device,
            },
        );
        if protocol_error.is_none() {
            self.backend_streams.insert(backend_stream);
        }
        self.seal_backend_protocol(protocol_error, id)
    }

    pub fn destroy_stream(
        &mut self,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        let result = self.backend.destroy_stream_v1(record.backend_stream);
        if matches!(&result, Err(RuntimeBackendFailureV1::Quiescent(_))) {
            self.mark_stream_quiescent(stream);
        }
        self.backend_result(result)?;
        self.mark_stream_quiescent(stream);
        self.streams.remove(&stream);
        self.backend_streams.remove(&record.backend_stream);
        Ok(())
    }

    pub fn allocate(
        &mut self,
        device: RuntimeDeviceIdV1,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<RuntimeAllocationIdV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if self.allocations.len() >= MAX_RUNTIME_ALLOCATIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(RuntimeValidationErrorV1::InvalidAlignment.into());
        }
        let device_record = self.device(device)?;
        let supported = match kind {
            RuntimeMemoryKindV1::DeviceLocal => device_record.capabilities.device_memory,
            RuntimeMemoryKindV1::HostVisible => device_record.capabilities.host_visible_memory,
        };
        if !supported {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let backend_device = device_record.backend_device;
        let id = RuntimeAllocationIdV1::new(self.context_generation, self.next_id()?);
        let result = self
            .backend
            .allocate_v1(backend_device, kind, byte_len, alignment);
        let backend_allocation = self.backend_result(result)?;
        let protocol_error = self.backend_handle_protocol_error(
            RuntimeBackendResourceKindV1::Allocation,
            backend_allocation,
        );
        self.allocations.insert(
            id,
            AllocationRecordV1 {
                backend_allocation,
                device,
                byte_len,
            },
        );
        if protocol_error.is_none() {
            self.backend_allocations.insert(backend_allocation);
        }
        self.seal_backend_protocol(protocol_error, id)
    }

    pub fn release_allocation(
        &mut self,
        allocation: RuntimeAllocationIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        let result = self
            .backend
            .release_allocation_v1(record.backend_allocation);
        self.backend_result(result)?;
        self.allocations.remove(&allocation);
        self.backend_allocations.remove(&record.backend_allocation);
        Ok(())
    }

    pub fn write_allocation(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        validate_byte_range(record.byte_len, byte_offset, bytes.len())?;
        let result =
            self.backend
                .write_allocation_v1(record.backend_allocation, byte_offset, bytes);
        self.backend_result(result)
    }

    pub fn read_allocation(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        validate_byte_range(record.byte_len, byte_offset, destination.len())?;
        let result =
            self.backend
                .read_allocation_v1(record.backend_allocation, byte_offset, destination);
        self.backend_result(result)
    }

    pub fn load_module(
        &mut self,
        device: RuntimeDeviceIdV1,
        image: &[u8],
    ) -> Result<RuntimeModuleIdV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if image.is_empty() {
            return Err(RuntimeValidationErrorV1::EmptyModule.into());
        }
        if image.len() > MAX_RUNTIME_MODULE_IMAGE_BYTES_V1 {
            return Err(RuntimeValidationErrorV1::ModuleTooLarge.into());
        }
        if self.modules.len() >= MAX_RUNTIME_MODULES_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        let backend_device = self.device(device)?.backend_device;
        let id = RuntimeModuleIdV1::new(self.context_generation, self.next_id()?);
        let image_sha256 = Sha256::digest(image).into();
        let result = self.backend.load_module_v1(backend_device, image);
        let backend_module = self.backend_result(result)?;
        let protocol_error = self
            .backend_handle_protocol_error(RuntimeBackendResourceKindV1::Module, backend_module);
        self.modules.insert(
            id,
            ModuleRecordV1 {
                backend_module,
                device,
                image_sha256,
            },
        );
        if protocol_error.is_none() {
            self.backend_modules.insert(backend_module);
        }
        self.seal_backend_protocol(protocol_error, id)
    }

    pub fn unload_module(
        &mut self,
        module: RuntimeModuleIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .modules
            .get(&module)
            .ok_or(RuntimeValidationErrorV1::UnknownModule)?;
        let result = self.backend.unload_module_v1(record.backend_module);
        self.backend_result(result)?;
        self.modules.remove(&module);
        self.backend_modules.remove(&record.backend_module);
        self.kernels.retain(|_, kernel| kernel.module != module);
        Ok(())
    }

    pub fn resolve_kernel<A: RuntimeArgumentsV1>(
        &mut self,
        module: RuntimeModuleIdV1,
        name: &str,
    ) -> Result<TypedRuntimeKernelV1<A>, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if name.is_empty() {
            return Err(RuntimeValidationErrorV1::EmptyKernelName.into());
        }
        if name.len() > MAX_RUNTIME_KERNEL_NAME_BYTES_V1 {
            return Err(RuntimeValidationErrorV1::KernelNameTooLong.into());
        }
        if self.kernels.len() >= MAX_RUNTIME_KERNELS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if name.as_bytes().contains(&0) {
            return Err(RuntimeValidationErrorV1::InvalidKernelName.into());
        }
        let record = *self
            .modules
            .get(&module)
            .ok_or(RuntimeValidationErrorV1::UnknownModule)?;
        if A::SIGNATURE_V1.iter().all(|byte| *byte == 0) {
            return Err(RuntimeValidationErrorV1::InvalidKernelSignature.into());
        }
        let target = self.device(record.device)?.target();
        let model_kernel = TypedAsyncKernelV1::new_model_only(runtime_kernel_identity(
            record.image_sha256,
            target,
            name,
            A::SIGNATURE_V1,
        ))
        .map_err(|_| RuntimeValidationErrorV1::InvalidKernelSignature)?;
        let result = self
            .backend
            .resolve_kernel_v1(record.backend_module, name, A::SIGNATURE_V1);
        let backend_kernel = self.backend_result(result)?;
        let protocol_error = self
            .backend_handle_protocol_error(RuntimeBackendResourceKindV1::Kernel, backend_kernel);
        if protocol_error.is_none() {
            self.kernels
                .insert(backend_kernel, KernelRecordV1 { module });
        }
        let kernel = TypedRuntimeKernelV1 {
            module,
            backend_kernel,
            name: name.to_owned(),
            signature: A::SIGNATURE_V1,
            model_kernel,
            marker: PhantomData,
        };
        self.seal_backend_protocol(protocol_error, kernel)
    }

    pub fn launch<A: RuntimeArgumentsV1>(
        &mut self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        arguments: &A,
        geometry: RuntimeLaunchGeometryV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        let geometry = geometry.validate()?;
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies.into());
        }
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependencies[..index].contains(dependency) {
                return Err(RuntimeValidationErrorV1::DuplicateDependency.into());
            }
        }
        let stream_record = *self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        if !self
            .device(stream_record.device)?
            .capabilities
            .typed_async_launch
        {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let module = *self
            .modules
            .get(&kernel.module)
            .ok_or(RuntimeValidationErrorV1::UnknownModule)?;
        if module.device != stream_record.device {
            return Err(RuntimeValidationErrorV1::WrongDevice.into());
        }
        if self
            .kernels
            .get(&kernel.backend_kernel)
            .is_none_or(|record| record.module != kernel.module)
        {
            return Err(RuntimeValidationErrorV1::UnknownKernel.into());
        }
        let explicit_kernarg = arguments.encode_explicit_kernarg_v1();
        if explicit_kernarg.len() > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(RuntimeValidationErrorV1::KernargTooLarge.into());
        }
        let bindings = arguments.bindings_v1();
        if bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
            return Err(RuntimeValidationErrorV1::TooManyBindings.into());
        }
        let mut backend_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let region = binding.region;
            let allocation = *self
                .allocations
                .get(&region.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
            if allocation.device != stream_record.device {
                return Err(RuntimeValidationErrorV1::WrongDevice.into());
            }
            let end = region
                .byte_offset
                .checked_add(region.byte_len)
                .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
            if region.byte_len == 0 || end > allocation.byte_len {
                return Err(RuntimeValidationErrorV1::InvalidRange.into());
            }
            let patch_end = binding
                .kernarg_byte_offset
                .checked_add(RUNTIME_DEVICE_POINTER_BYTES_V1)
                .and_then(|end| usize::try_from(end).ok())
                .ok_or(RuntimeValidationErrorV1::InvalidKernargPatch)?;
            let patch_start = usize::try_from(binding.kernarg_byte_offset)
                .map_err(|_| RuntimeValidationErrorV1::InvalidKernargPatch)?;
            if !binding
                .kernarg_byte_offset
                .is_multiple_of(RUNTIME_DEVICE_POINTER_BYTES_V1)
                || patch_end > explicit_kernarg.len()
                || explicit_kernarg[patch_start..patch_end]
                    .iter()
                    .any(|byte| *byte != 0)
                || backend_bindings.iter().any(|prior: &BackendBindingV1| {
                    let prior_start = prior.kernarg_byte_offset;
                    let prior_end = prior_start + RUNTIME_DEVICE_POINTER_BYTES_V1;
                    binding.kernarg_byte_offset < prior_end
                        && prior_start
                            < binding.kernarg_byte_offset + RUNTIME_DEVICE_POINTER_BYTES_V1
                })
            {
                return Err(RuntimeValidationErrorV1::InvalidKernargPatch.into());
            }
            backend_bindings.push(BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocation.backend_allocation,
                    access: region.access,
                    byte_offset: region.byte_offset,
                    byte_len: region.byte_len,
                },
                kernarg_byte_offset: binding.kernarg_byte_offset,
            });
        }
        let mut backend_dependencies = Vec::with_capacity(dependencies.len());
        for dependency in dependencies {
            let event = *self
                .events
                .get(dependency)
                .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
            if event.device != stream_record.device {
                return Err(RuntimeValidationErrorV1::WrongDevice.into());
            }
            backend_dependencies.push(event.backend_event);
        }
        let id = RuntimeSubmissionIdV1::new(self.context_generation, self.next_id()?);
        let result = self.backend.submit_v1(BackendLaunchV1 {
            stream: stream_record.backend_stream,
            kernel: kernel.backend_kernel,
            explicit_kernarg: &explicit_kernarg,
            bindings: &backend_bindings,
            dependencies: &backend_dependencies,
            geometry,
        });
        let backend_submission = self.backend_result(result)?;
        let protocol_error = self.backend_handle_protocol_error(
            RuntimeBackendResourceKindV1::Submission,
            backend_submission,
        );
        self.submissions.insert(
            id,
            SubmissionRecordV1 {
                backend_submission,
                stream,
                device: stream_record.device,
                quiescent: false,
            },
        );
        if protocol_error.is_none() {
            self.backend_submissions.insert(backend_submission);
        }
        let submission = RuntimeSubmissionV1 {
            id,
            backend_submission,
            stream,
            device: stream_record.device,
            completion: None,
            peer_transfer: None,
            marker: PhantomData,
        };
        self.seal_backend_protocol(protocol_error, submission)
    }

    pub fn poll<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = self.live_submission_record(submission)?;
        if let Some(completion) = submission.completion {
            return Ok(completion);
        }
        let result = self.backend.poll_v1(record.backend_submission);
        let observation = self.backend_result(result)?;
        let observation = submission.observe(observation);
        if observation != RuntimePollV1::Pending {
            self.submissions.get_mut(&submission.id).unwrap().quiescent = true;
        }
        Ok(observation)
    }

    pub fn wait<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
        timeout: Duration,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = self.live_submission_record(submission)?;
        if let Some(completion) = submission.completion {
            return Ok(completion);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;
        let result = self.backend.wait_v1(record.backend_submission, deadline);
        let observation = self.backend_result(result)?;
        let observation = submission.observe(observation);
        if observation != RuntimePollV1::Pending {
            self.submissions.get_mut(&submission.id).unwrap().quiescent = true;
        }
        Ok(observation)
    }

    /// Consumes and releases a submission after terminal completion or stream quiescence.
    pub fn release_submission<A>(
        &mut self,
        submission: RuntimeSubmissionV1<A>,
    ) -> Result<(), RuntimeSubmissionReleaseFailureV1<A, B::Error>> {
        match self.release_submission_ref(&submission) {
            Ok(()) => Ok(()),
            Err(error) => Err(RuntimeSubmissionReleaseFailureV1 { submission, error }),
        }
    }

    fn release_submission_ref<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = self.submission_record(submission)?;
        if !record.quiescent {
            return Err(RuntimeValidationErrorV1::SubmissionPending.into());
        }
        if self
            .events
            .values()
            .any(|event| event.submission == submission.id)
        {
            return Err(RuntimeValidationErrorV1::SubmissionRetainedByEvent.into());
        }
        let result = self
            .backend
            .release_submission_v1(record.backend_submission);
        self.backend_result(result)?;
        self.submissions.remove(&submission.id);
        self.backend_submissions.remove(&record.backend_submission);
        Ok(())
    }

    pub fn record_event<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<RuntimeEventIdV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let submission_record = self.live_submission_record(submission)?;
        if self.events.len() >= MAX_RUNTIME_EVENTS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if !self.device(submission_record.device)?.capabilities.events {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let stream = self.streams[&submission_record.stream];
        let id = RuntimeEventIdV1::new(self.context_generation, self.next_id()?);
        let result = self
            .backend
            .record_event_v1(stream.backend_stream, submission_record.backend_submission);
        let backend_event = self.backend_result(result)?;
        let protocol_error =
            self.backend_handle_protocol_error(RuntimeBackendResourceKindV1::Event, backend_event);
        self.events.insert(
            id,
            EventRecordV1 {
                backend_event,
                device: submission_record.device,
                submission: submission.id,
            },
        );
        if protocol_error.is_none() {
            self.backend_events.insert(backend_event);
        }
        self.seal_backend_protocol(protocol_error, id)
    }

    pub fn release_event(
        &mut self,
        event: RuntimeEventIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .events
            .get(&event)
            .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
        let result = self.backend.release_event_v1(record.backend_event);
        self.backend_result(result)?;
        self.events.remove(&event);
        self.backend_events.remove(&record.backend_event);
        Ok(())
    }

    pub fn peer_copy(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimePeerCopyV1>, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies.into());
        }
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependencies[..index].contains(dependency) {
                return Err(RuntimeValidationErrorV1::DuplicateDependency.into());
            }
        }
        let stream_record = *self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        let peer_contract_identity = peer_copy_contract_identity(stream, source, destination);
        let translate = |region: RuntimeMemoryRegionV1| -> Result<
            (BackendMemoryRegionV1, RuntimeDeviceIdV1),
            RuntimeValidationErrorV1,
        > {
            let allocation = *self
                .allocations
                .get(&region.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
            let end = region
                .byte_offset
                .checked_add(region.byte_len)
                .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
            if region.byte_len == 0 || end > allocation.byte_len {
                return Err(RuntimeValidationErrorV1::InvalidRange);
            }
            Ok((
                BackendMemoryRegionV1 {
                    allocation: allocation.backend_allocation,
                    access: region.access,
                    byte_offset: region.byte_offset,
                    byte_len: region.byte_len,
                },
                allocation.device,
            ))
        };
        let (source, source_device) = translate(source)?;
        let (destination, destination_device) = translate(destination)?;
        if !matches!(
            source.access,
            RuntimeAccessV1::Read | RuntimeAccessV1::ReadWrite
        ) || !matches!(
            destination.access,
            RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
        ) {
            return Err(RuntimeValidationErrorV1::InvalidAccess.into());
        }
        if stream_record.device != destination_device || source.byte_len != destination.byte_len {
            return Err(RuntimeValidationErrorV1::WrongDevice.into());
        }
        let source_capabilities = self.device(source_device)?.capabilities;
        let destination_capabilities = self.device(destination_device)?.capabilities;
        if source_device == destination_device {
            return Err(RuntimeValidationErrorV1::WrongDevice.into());
        }
        if !source_capabilities.peer_copy
            || !source_capabilities.multi_device
            || !destination_capabilities.peer_copy
            || !destination_capabilities.multi_device
        {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let mut backend_dependencies = Vec::with_capacity(dependencies.len());
        for dependency in dependencies {
            let event = self
                .events
                .get(dependency)
                .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
            if event.device != source_device && event.device != destination_device {
                return Err(RuntimeValidationErrorV1::WrongDevice.into());
            }
            backend_dependencies.push(event.backend_event);
        }
        let id = RuntimeSubmissionIdV1::new(self.context_generation, self.next_id()?);
        let result = self.backend.peer_copy_v1(
            stream_record.backend_stream,
            source,
            destination,
            &backend_dependencies,
        );
        let backend_submission = self.backend_result(result)?;
        let protocol_error = self.backend_handle_protocol_error(
            RuntimeBackendResourceKindV1::Submission,
            backend_submission,
        );
        self.submissions.insert(
            id,
            SubmissionRecordV1 {
                backend_submission,
                stream,
                device: destination_device,
                quiescent: false,
            },
        );
        if protocol_error.is_none() {
            self.backend_submissions.insert(backend_submission);
        }
        let submission = RuntimeSubmissionV1 {
            id,
            backend_submission,
            stream,
            device: destination_device,
            completion: None,
            peer_transfer: Some(PeerTransferMechanismV1::DeclaredPeerCopy {
                contract_identity: peer_contract_identity,
            }),
            marker: PhantomData,
        };
        self.seal_backend_protocol(protocol_error, submission)
    }
}

fn validate_byte_range(
    allocation_bytes: u64,
    byte_offset: u64,
    byte_len: usize,
) -> Result<(), RuntimeValidationErrorV1> {
    let byte_len = u64::try_from(byte_len).map_err(|_| RuntimeValidationErrorV1::InvalidRange)?;
    if byte_len == 0
        || byte_offset
            .checked_add(byte_len)
            .is_none_or(|end| end > allocation_bytes)
    {
        return Err(RuntimeValidationErrorV1::InvalidRange);
    }
    Ok(())
}

fn runtime_kernel_identity(
    module_image_sha256: [u8; 32],
    target: &str,
    symbol: &str,
    signature: [u8; 32],
) -> IdentityDigestV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.runtime.typed-kernel.v1\0");
    digest.update(module_image_sha256);
    digest.update((target.len() as u64).to_le_bytes());
    digest.update(target.as_bytes());
    digest.update((symbol.len() as u64).to_le_bytes());
    digest.update(symbol.as_bytes());
    digest.update(signature);
    IdentityDigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn peer_copy_contract_identity(
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
) -> IdentityDigestV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.runtime.peer-copy.v1\0");
    digest.update(stream.context_generation.to_le_bytes());
    digest.update(stream.local.to_le_bytes());
    for region in [source, destination] {
        digest.update(region.allocation.context_generation.to_le_bytes());
        digest.update(region.allocation.local.to_le_bytes());
        digest.update([match region.access {
            RuntimeAccessV1::Read => 1,
            RuntimeAccessV1::Write => 2,
            RuntimeAccessV1::ReadWrite => 3,
        }]);
        digest.update(region.byte_offset.to_le_bytes());
        digest.update(region.byte_len.to_le_bytes());
    }
    IdentityDigestV1::from_untrusted_bytes(digest.finalize().into())
}

/// Marker identifying a typed peer-copy submission.
pub enum RuntimePeerCopyV1 {}

/// Moveable asynchronous submission bound to its argument type.
pub struct RuntimeSubmissionV1<A> {
    id: RuntimeSubmissionIdV1,
    backend_submission: u64,
    stream: RuntimeStreamIdV1,
    device: RuntimeDeviceIdV1,
    completion: Option<RuntimePollV1>,
    peer_transfer: Option<PeerTransferMechanismV1>,
    marker: PhantomData<fn(A) -> A>,
}

impl<A> RuntimeSubmissionV1<A> {
    pub const fn id(&self) -> RuntimeSubmissionIdV1 {
        self.id
    }

    pub const fn stream(&self) -> RuntimeStreamIdV1 {
        self.stream
    }

    /// Returns the pure-model peer-copy contract paired with this submission, if any.
    pub const fn peer_transfer_mechanism(&self) -> Option<PeerTransferMechanismV1> {
        self.peer_transfer
    }

    fn observe(&mut self, observation: BackendPollV1) -> RuntimePollV1 {
        match observation {
            BackendPollV1::Pending => RuntimePollV1::Pending,
            BackendPollV1::Succeeded => {
                self.completion = Some(RuntimePollV1::Succeeded);
                RuntimePollV1::Succeeded
            }
            BackendPollV1::Failed { code } => {
                let completion = RuntimePollV1::Failed { code };
                self.completion = Some(completion);
                completion
            }
        }
    }
}

/// Public nonblocking completion state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePollV1 {
    Pending,
    Succeeded,
    Failed { code: i64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockError(&'static str);

    impl fmt::Display for MockError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl Error for MockError {}

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockCleanupKind {
        Stream,
        Event,
        Submission,
        Module,
        Allocation,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockHandleKind {
        Stream,
        Allocation,
        Module,
        Kernel,
        Submission,
        Event,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum MockCleanupFailure {
        #[default]
        None,
        RejectStreamOnce,
        QuiescentStreamOnce,
        RejectEventOnce,
        TerminalEvent,
    }

    #[derive(Debug, Default)]
    struct MockBackend {
        next: u64,
        memory: HashMap<u64, Vec<u8>>,
        polls: HashMap<u64, u8>,
        terminal_on_submit: bool,
        last_dependency_count: usize,
        cleanup_failure: MockCleanupFailure,
        cleanup_log: Vec<(MockCleanupKind, u64)>,
        device_name_len: usize,
        device_target_len: usize,
        handle_override: Option<(MockHandleKind, u64)>,
    }

    impl MockBackend {
        fn identity(&mut self) -> u64 {
            self.next += 1;
            self.next
        }

        fn handle(&mut self, kind: MockHandleKind) -> u64 {
            if let Some((override_kind, value)) = self.handle_override
                && override_kind == kind
            {
                self.handle_override = None;
                return value;
            }
            self.identity()
        }
    }

    impl RuntimeBackendV1 for MockBackend {
        type Error = MockError;

        fn enumerate_devices_v1(
            &mut self,
        ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
            let capabilities = RuntimeCapabilitiesV1 {
                typed_async_launch: true,
                streams: true,
                events: true,
                device_memory: true,
                host_visible_memory: true,
                peer_copy: true,
                multi_device: true,
                atomics: true,
                collectives: true,
            };
            let device_name = if self.device_name_len == 0 {
                "device-0".into()
            } else {
                "n".repeat(self.device_name_len)
            };
            let device_target = if self.device_target_len == 0 {
                "gfx942".into()
            } else {
                "t".repeat(self.device_target_len)
            };
            Ok(vec![
                BackendDeviceDescriptionV1 {
                    backend_device: 10,
                    name: device_name,
                    target: device_target,
                    global_memory_bytes: 1 << 30,
                    capabilities,
                },
                BackendDeviceDescriptionV1 {
                    backend_device: 20,
                    name: "device-1".into(),
                    target: "gfx942".into(),
                    global_memory_bytes: 1 << 30,
                    capabilities,
                },
            ])
        }

        fn create_stream_v1(
            &mut self,
            _device: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle(MockHandleKind::Stream))
        }

        fn destroy_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.cleanup_log.push((MockCleanupKind::Stream, stream));
            if self.cleanup_failure == MockCleanupFailure::RejectStreamOnce {
                self.cleanup_failure = MockCleanupFailure::None;
                return Err(RuntimeBackendFailureV1::Rejected(MockError("busy")));
            }
            if self.cleanup_failure == MockCleanupFailure::QuiescentStreamOnce {
                self.cleanup_failure = MockCleanupFailure::None;
                return Err(RuntimeBackendFailureV1::Quiescent(MockError(
                    "destroy failed after quiescence",
                )));
            }
            Ok(())
        }

        fn allocate_v1(
            &mut self,
            _device: u64,
            _kind: RuntimeMemoryKindV1,
            byte_len: u64,
            _alignment: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let identity = self.handle(MockHandleKind::Allocation);
            self.memory.insert(identity, vec![0; byte_len as usize]);
            Ok(identity)
        }

        fn release_allocation_v1(
            &mut self,
            allocation: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.cleanup_log
                .push((MockCleanupKind::Allocation, allocation));
            self.memory.remove(&allocation);
            Ok(())
        }

        fn write_allocation_v1(
            &mut self,
            allocation: u64,
            byte_offset: u64,
            bytes: &[u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            let memory = self.memory.get_mut(&allocation).unwrap();
            let start = byte_offset as usize;
            memory[start..start + bytes.len()].copy_from_slice(bytes);
            Ok(())
        }

        fn read_allocation_v1(
            &mut self,
            allocation: u64,
            byte_offset: u64,
            destination: &mut [u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            let memory = self.memory.get(&allocation).unwrap();
            let start = byte_offset as usize;
            destination.copy_from_slice(&memory[start..start + destination.len()]);
            Ok(())
        }

        fn load_module_v1(
            &mut self,
            _device: u64,
            _image: &[u8],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle(MockHandleKind::Module))
        }

        fn unload_module_v1(
            &mut self,
            module: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.cleanup_log.push((MockCleanupKind::Module, module));
            Ok(())
        }

        fn resolve_kernel_v1(
            &mut self,
            _module: u64,
            _name: &str,
            _signature: [u8; 32],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle(MockHandleKind::Kernel))
        }

        fn submit_v1(
            &mut self,
            launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            if self.terminal_on_submit {
                return Err(RuntimeBackendFailureV1::Terminal(MockError("lost")));
            }
            self.last_dependency_count = launch.dependencies.len();
            let identity = self.handle(MockHandleKind::Submission);
            self.polls.insert(identity, 0);
            Ok(identity)
        }

        fn poll_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            let polls = self.polls.get_mut(&submission).unwrap();
            *polls += 1;
            Ok(if *polls == 1 {
                BackendPollV1::Pending
            } else {
                BackendPollV1::Succeeded
            })
        }

        fn wait_v1(
            &mut self,
            _submission: u64,
            _deadline: Instant,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            Ok(BackendPollV1::Succeeded)
        }

        fn release_submission_v1(
            &mut self,
            submission: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.cleanup_log
                .push((MockCleanupKind::Submission, submission));
            self.polls.remove(&submission);
            Ok(())
        }

        fn record_event_v1(
            &mut self,
            _stream: u64,
            _submission: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle(MockHandleKind::Event))
        }

        fn release_event_v1(
            &mut self,
            event: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.cleanup_log.push((MockCleanupKind::Event, event));
            if self.cleanup_failure == MockCleanupFailure::RejectEventOnce {
                self.cleanup_failure = MockCleanupFailure::None;
                return Err(RuntimeBackendFailureV1::Rejected(MockError("event busy")));
            }
            if self.cleanup_failure == MockCleanupFailure::TerminalEvent {
                return Err(RuntimeBackendFailureV1::Terminal(MockError("lost")));
            }
            Ok(())
        }

        fn peer_copy_v1(
            &mut self,
            _stream: u64,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
            dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.last_dependency_count = dependencies.len();
            let source_start = source.byte_offset as usize;
            let source_end = source_start + source.byte_len as usize;
            let bytes = self.memory[&source.allocation][source_start..source_end].to_vec();
            let destination_start = destination.byte_offset as usize;
            self.memory.get_mut(&destination.allocation).unwrap()
                [destination_start..destination_start + bytes.len()]
                .copy_from_slice(&bytes);
            let identity = self.handle(MockHandleKind::Submission);
            self.polls.insert(identity, 0);
            Ok(identity)
        }
    }

    struct AddArguments {
        allocation: RuntimeAllocationIdV1,
        scalar: u32,
    }

    struct PatchArguments {
        allocation: RuntimeAllocationIdV1,
        offsets: [u32; 2],
        kernarg_len: usize,
        patch_fill: u8,
    }

    struct HostileArguments {
        allocation: RuntimeAllocationIdV1,
        kernarg_len: usize,
        binding_count: usize,
    }

    impl RuntimeArgumentsV1 for HostileArguments {
        const SIGNATURE_V1: [u8; 32] = [9; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            vec![0; self.kernarg_len]
        }

        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            vec![
                RuntimeBindingV1 {
                    region: RuntimeMemoryRegionV1 {
                        allocation: self.allocation,
                        access: RuntimeAccessV1::Read,
                        byte_offset: 0,
                        byte_len: 1,
                    },
                    kernarg_byte_offset: 0,
                };
                self.binding_count
            ]
        }
    }

    impl RuntimeArgumentsV1 for PatchArguments {
        const SIGNATURE_V1: [u8; 32] = [8; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            vec![self.patch_fill; self.kernarg_len]
        }

        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            self.offsets
                .into_iter()
                .enumerate()
                .map(|(index, kernarg_byte_offset)| RuntimeBindingV1 {
                    region: RuntimeMemoryRegionV1 {
                        allocation: self.allocation,
                        access: RuntimeAccessV1::ReadWrite,
                        byte_offset: index as u64 * 8,
                        byte_len: 8,
                    },
                    kernarg_byte_offset,
                })
                .collect()
        }
    }

    impl RuntimeArgumentsV1 for AddArguments {
        const SIGNATURE_V1: [u8; 32] = [7; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            let mut bytes = vec![0; RUNTIME_DEVICE_POINTER_BYTES_V1 as usize];
            bytes.extend_from_slice(&self.scalar.to_le_bytes());
            bytes
        }

        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            vec![RuntimeBindingV1 {
                region: RuntimeMemoryRegionV1 {
                    allocation: self.allocation,
                    access: RuntimeAccessV1::ReadWrite,
                    byte_offset: 0,
                    byte_len: 16,
                },
                kernarg_byte_offset: 0,
            }]
        }
    }

    fn geometry() -> RuntimeLaunchGeometryV1 {
        RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        }
    }

    fn context_with_cleanup_resources(backend: MockBackend) -> RuntimeContextV1<MockBackend> {
        let mut context = RuntimeContextV1::open(backend).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let submission = context
            .launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        context.record_event(&submission).unwrap();
        context
    }

    fn context_with_launch_prerequisites() -> (
        RuntimeContextV1<MockBackend>,
        RuntimeStreamIdV1,
        RuntimeAllocationIdV1,
        TypedRuntimeKernelV1<AddArguments>,
    ) {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        (context, stream, allocation, kernel)
    }

    fn context_with_peer_prerequisites() -> (
        RuntimeContextV1<MockBackend>,
        RuntimeStreamIdV1,
        RuntimeMemoryRegionV1,
        RuntimeMemoryRegionV1,
    ) {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let devices = context.devices().to_vec();
        let stream = context.create_stream(devices[1].id()).unwrap();
        let source = context
            .allocate(devices[0].id(), RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        let destination = context
            .allocate(devices[1].id(), RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        (
            context,
            stream,
            RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 16,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 16,
            },
        )
    }

    fn assert_protocol_failure<T>(
        result: Result<T, RuntimeErrorV1<MockError>>,
        expected: RuntimeBackendProtocolErrorV1,
    ) {
        match result {
            Err(RuntimeErrorV1::BackendProtocol(actual)) => assert_eq!(actual, expected),
            _ => panic!("expected a terminal backend protocol failure"),
        }
    }

    #[test]
    fn one_context_multiplexes_streams_across_devices() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let devices = context.devices().to_vec();
        let first = context.create_stream(devices[0].id()).unwrap();
        let second = context.create_stream(devices[0].id()).unwrap();
        let third = context.create_stream(devices[1].id()).unwrap();
        assert_ne!(first, second);
        assert_ne!(second, third);
        context.destroy_stream(first).unwrap();
        context.destroy_stream(second).unwrap();
        context.destroy_stream(third).unwrap();
    }

    #[test]
    fn exhausted_facade_identity_prevents_backend_resource_creation() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        context.next_identity = u64::MAX;
        assert!(matches!(
            context.create_stream(device),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Capacity
            ))
        ));
        assert_eq!(context.backend().next, 0);
        assert!(context.streams.is_empty());
    }

    #[test]
    fn zero_backend_handles_terminally_seal_every_handle_producing_operation() {
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            context.backend.handle_override = Some((MockHandleKind::Stream, 0));
            let device = context.devices()[0].id();
            assert_protocol_failure(
                context.create_stream(device),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Stream),
            );
            assert!(context.is_terminal());
            assert_eq!(context.streams.len(), 1);
        }
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            context.backend.handle_override = Some((MockHandleKind::Allocation, 0));
            let device = context.devices()[0].id();
            assert_protocol_failure(
                context.allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Allocation),
            );
            assert!(context.is_terminal());
            assert_eq!(context.allocations.len(), 1);
        }
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            context.backend.handle_override = Some((MockHandleKind::Module, 0));
            let device = context.devices()[0].id();
            assert_protocol_failure(
                context.load_module(device, b"object"),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Module),
            );
            assert!(context.is_terminal());
            assert_eq!(context.modules.len(), 1);
        }
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            let device = context.devices()[0].id();
            let module = context.load_module(device, b"object").unwrap();
            context.backend.handle_override = Some((MockHandleKind::Kernel, 0));
            assert_protocol_failure(
                context.resolve_kernel::<AddArguments>(module, "add"),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Kernel),
            );
            assert!(context.is_terminal());
        }
        {
            let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
            context.backend.handle_override = Some((MockHandleKind::Submission, 0));
            assert_protocol_failure(
                context.launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 1,
                    },
                    geometry(),
                    &[],
                ),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Submission),
            );
            assert!(context.is_terminal());
            assert_eq!(context.submissions.len(), 1);
        }
        {
            let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
            let submission = context
                .launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 1,
                    },
                    geometry(),
                    &[],
                )
                .unwrap();
            context.backend.handle_override = Some((MockHandleKind::Event, 0));
            assert_protocol_failure(
                context.record_event(&submission),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Event),
            );
            assert!(context.is_terminal());
            assert_eq!(context.events.len(), 1);
        }
        {
            let (mut context, stream, source, destination) = context_with_peer_prerequisites();
            context.backend.handle_override = Some((MockHandleKind::Submission, 0));
            assert_protocol_failure(
                context.peer_copy(stream, source, destination, &[]),
                RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Submission),
            );
            assert!(context.is_terminal());
            assert_eq!(context.submissions.len(), 1);
        }
    }

    #[test]
    fn duplicate_backend_handles_terminally_seal_every_handle_producing_operation() {
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            let device = context.devices()[0].id();
            let first = context.create_stream(device).unwrap();
            let duplicate = context.streams[&first].backend_stream;
            context.backend.handle_override = Some((MockHandleKind::Stream, duplicate));
            assert_protocol_failure(
                context.create_stream(device),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(
                    RuntimeBackendResourceKindV1::Stream,
                ),
            );
            assert!(context.is_terminal());
            assert_eq!(context.streams.len(), 2);
        }
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            let device = context.devices()[0].id();
            let first = context
                .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
                .unwrap();
            let duplicate = context.allocations[&first].backend_allocation;
            context.backend.handle_override = Some((MockHandleKind::Allocation, duplicate));
            assert_protocol_failure(
                context.allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(
                    RuntimeBackendResourceKindV1::Allocation,
                ),
            );
            assert!(context.is_terminal());
            assert_eq!(context.allocations.len(), 2);
        }
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            let device = context.devices()[0].id();
            let first = context.load_module(device, b"object-a").unwrap();
            let duplicate = context.modules[&first].backend_module;
            context.backend.handle_override = Some((MockHandleKind::Module, duplicate));
            assert_protocol_failure(
                context.load_module(device, b"object-b"),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(
                    RuntimeBackendResourceKindV1::Module,
                ),
            );
            assert!(context.is_terminal());
            assert_eq!(context.modules.len(), 2);
        }
        {
            let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
            let device = context.devices()[0].id();
            let module = context.load_module(device, b"object").unwrap();
            let first = context
                .resolve_kernel::<AddArguments>(module, "add-a")
                .unwrap();
            context.backend.handle_override = Some((MockHandleKind::Kernel, first.backend_kernel));
            assert_protocol_failure(
                context.resolve_kernel::<AddArguments>(module, "add-b"),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(
                    RuntimeBackendResourceKindV1::Kernel,
                ),
            );
            assert!(context.is_terminal());
        }
        {
            let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
            let first = context
                .launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 1,
                    },
                    geometry(),
                    &[],
                )
                .unwrap();
            context.backend.handle_override =
                Some((MockHandleKind::Submission, first.backend_submission));
            assert_protocol_failure(
                context.launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 2,
                    },
                    geometry(),
                    &[],
                ),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(
                    RuntimeBackendResourceKindV1::Submission,
                ),
            );
            assert!(context.is_terminal());
            assert_eq!(context.submissions.len(), 2);
        }
        {
            let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
            let submission = context
                .launch(
                    stream,
                    &kernel,
                    &AddArguments {
                        allocation,
                        scalar: 1,
                    },
                    geometry(),
                    &[],
                )
                .unwrap();
            let first = context.record_event(&submission).unwrap();
            let duplicate = context.events[&first].backend_event;
            context.backend.handle_override = Some((MockHandleKind::Event, duplicate));
            assert_protocol_failure(
                context.record_event(&submission),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(RuntimeBackendResourceKindV1::Event),
            );
            assert!(context.is_terminal());
            assert_eq!(context.events.len(), 2);
        }
        {
            let (mut context, stream, source, destination) = context_with_peer_prerequisites();
            let first = context.peer_copy(stream, source, destination, &[]).unwrap();
            context.backend.handle_override =
                Some((MockHandleKind::Submission, first.backend_submission));
            assert_protocol_failure(
                context.peer_copy(stream, source, destination, &[]),
                RuntimeBackendProtocolErrorV1::DuplicateHandle(
                    RuntimeBackendResourceKindV1::Submission,
                ),
            );
            assert!(context.is_terminal());
            assert_eq!(context.submissions.len(), 2);
        }
    }

    #[test]
    fn opaque_handles_with_equal_local_ids_never_cross_contexts() {
        let mut first = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let mut second = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let first_device = first.devices()[0].id();
        let second_device = second.devices()[0].id();
        let first_stream = first.create_stream(first_device).unwrap();
        let second_stream = second.create_stream(second_device).unwrap();
        let first_allocation = first
            .allocate(first_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let second_allocation = second
            .allocate(second_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let first_module = first.load_module(first_device, b"object").unwrap();
        let second_module = second.load_module(second_device, b"object").unwrap();
        let first_kernel = first
            .resolve_kernel::<AddArguments>(first_module, "add")
            .unwrap();
        let second_kernel = second
            .resolve_kernel::<AddArguments>(second_module, "add")
            .unwrap();
        let mut first_submission = first
            .launch(
                first_stream,
                &first_kernel,
                &AddArguments {
                    allocation: first_allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        let second_submission = second
            .launch(
                second_stream,
                &second_kernel,
                &AddArguments {
                    allocation: second_allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        let first_event = first.record_event(&first_submission).unwrap();
        let second_event = second.record_event(&second_submission).unwrap();

        assert_eq!(first_device.get(), second_device.get());
        assert_eq!(first_stream.get(), second_stream.get());
        assert_eq!(first_allocation.get(), second_allocation.get());
        assert_eq!(first_module.get(), second_module.get());
        assert_eq!(first_event.get(), second_event.get());
        assert_eq!(first_submission.id().get(), second_submission.id().get());
        assert_ne!(first_device, second_device);
        assert_ne!(first_stream, second_stream);

        assert!(matches!(
            second.create_stream(first_device),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownDevice
            ))
        ));
        assert!(matches!(
            second.destroy_stream(first_stream),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        assert!(matches!(
            second.write_allocation(first_allocation, 0, &[1]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownAllocation
            ))
        ));
        assert!(matches!(
            second.unload_module(first_module),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownModule
            ))
        ));
        assert!(matches!(
            second.release_event(first_event),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownEvent
            ))
        ));
        assert!(matches!(
            second.poll(&mut first_submission),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownSubmission
            ))
        ));
        assert!(matches!(
            second.record_event(&first_submission),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownSubmission
            ))
        ));
        let failure = second.release_submission(first_submission).unwrap_err();
        assert!(matches!(
            failure.error(),
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::UnknownSubmission)
        ));
    }

    #[test]
    fn typed_launch_memory_event_and_dependency_flow_is_address_free() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        context
            .write_allocation(allocation, 4, &[1, 2, 3, 4])
            .unwrap();
        let mut readback = [0; 4];
        context
            .read_allocation(allocation, 4, &mut readback)
            .unwrap();
        assert_eq!(readback, [1, 2, 3, 4]);
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        assert_ne!(kernel.model_identity().as_bytes(), &[7; 32]);
        let arguments = AddArguments {
            allocation,
            scalar: 9,
        };
        let mut first = context
            .launch(stream, &kernel, &arguments, geometry(), &[])
            .unwrap();
        assert_eq!(context.poll(&mut first).unwrap(), RuntimePollV1::Pending);
        assert_eq!(context.poll(&mut first).unwrap(), RuntimePollV1::Succeeded);
        let event = context.record_event(&first).unwrap();
        let mut second = context
            .launch(stream, &kernel, &arguments, geometry(), &[event])
            .unwrap();
        assert_eq!(context.backend().last_dependency_count, 1);
        assert_eq!(
            context.wait(&mut second, Duration::from_secs(1)).unwrap(),
            RuntimePollV1::Succeeded
        );
        context.release_event(event).unwrap();
    }

    #[test]
    fn typed_kernel_identity_commits_to_module_target_symbol_and_signature() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let first_module = context.load_module(device, b"object-a").unwrap();
        let second_module = context.load_module(device, b"object-b").unwrap();
        let first = context
            .resolve_kernel::<AddArguments>(first_module, "same")
            .unwrap();
        let repeated = context
            .resolve_kernel::<AddArguments>(first_module, "same")
            .unwrap();
        let other_symbol = context
            .resolve_kernel::<AddArguments>(first_module, "other")
            .unwrap();
        let other_module = context
            .resolve_kernel::<AddArguments>(second_module, "same")
            .unwrap();

        assert_eq!(first.model_identity(), repeated.model_identity());
        assert_ne!(first.model_identity(), other_symbol.model_identity());
        assert_ne!(first.model_identity(), other_module.model_identity());
    }

    #[test]
    fn retained_submission_cannot_reach_backend_after_its_stream_is_destroyed() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let mut submission = context
            .launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        context.destroy_stream(stream).unwrap();

        assert!(matches!(
            context.poll(&mut submission),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        assert!(matches!(
            context.wait(&mut submission, Duration::from_secs(1)),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        assert!(matches!(
            context.record_event(&submission),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
    }

    #[test]
    fn wait_rejects_unrepresentable_deadline() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let mut submission = context
            .launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        assert!(matches!(
            context.wait(&mut submission, Duration::MAX),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidDeadline
            ))
        ));
    }

    #[test]
    fn submission_release_is_consuming_retryable_and_requires_quiescence() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let submission = context
            .launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();

        let failure = context.release_submission(submission).unwrap_err();
        assert!(matches!(
            failure.error(),
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
        ));
        let (mut submission, _) = failure.into_parts();
        assert_eq!(
            context
                .wait(&mut submission, Duration::from_secs(1))
                .unwrap(),
            RuntimePollV1::Succeeded
        );
        context.release_submission(submission).unwrap();
        assert_eq!(
            context.backend().cleanup_log.last().map(|(kind, _)| *kind),
            Some(MockCleanupKind::Submission)
        );

        let submission = context
            .launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 2,
                },
                geometry(),
                &[],
            )
            .unwrap();
        context.destroy_stream(stream).unwrap();
        context.release_submission(submission).unwrap();
    }

    #[test]
    fn peer_copy_moves_between_distinct_devices() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        let stream = context.create_stream(destination_device).unwrap();
        let source = context
            .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let destination = context
            .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        context.write_allocation(source, 0, &[3, 1, 4, 1]).unwrap();
        let submission = context
            .peer_copy(
                stream,
                RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 4,
                },
                RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 4,
                    byte_len: 4,
                },
                &[],
            )
            .unwrap();
        assert!(matches!(
            submission.peer_transfer_mechanism(),
            Some(PeerTransferMechanismV1::DeclaredPeerCopy { .. })
        ));
        let mut bytes = [0; 4];
        context.read_allocation(destination, 4, &mut bytes).unwrap();
        assert_eq!(bytes, [3, 1, 4, 1]);

        for (source_access, destination_access) in [
            (RuntimeAccessV1::Write, RuntimeAccessV1::Write),
            (RuntimeAccessV1::Read, RuntimeAccessV1::Read),
        ] {
            assert!(matches!(
                context.peer_copy(
                    stream,
                    RuntimeMemoryRegionV1 {
                        allocation: source,
                        access: source_access,
                        byte_offset: 0,
                        byte_len: 4,
                    },
                    RuntimeMemoryRegionV1 {
                        allocation: destination,
                        access: destination_access,
                        byte_offset: 4,
                        byte_len: 4,
                    },
                    &[],
                ),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidAccess
                ))
            ));
        }
    }

    #[test]
    fn peer_copy_identity_includes_the_private_context_brand() {
        fn transfer(context: &mut RuntimeContextV1<MockBackend>) -> PeerTransferMechanismV1 {
            let source_device = context.devices()[0].id();
            let destination_device = context.devices()[1].id();
            let stream = context.create_stream(destination_device).unwrap();
            let source = context
                .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
                .unwrap();
            let destination = context
                .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
                .unwrap();
            context
                .peer_copy(
                    stream,
                    RuntimeMemoryRegionV1 {
                        allocation: source,
                        access: RuntimeAccessV1::Read,
                        byte_offset: 0,
                        byte_len: 4,
                    },
                    RuntimeMemoryRegionV1 {
                        allocation: destination,
                        access: RuntimeAccessV1::Write,
                        byte_offset: 0,
                        byte_len: 4,
                    },
                    &[],
                )
                .unwrap()
                .peer_transfer_mechanism()
                .unwrap()
        }

        let mut first = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let mut second = RuntimeContextV1::open(MockBackend::default()).unwrap();
        assert_ne!(transfer(&mut first), transfer(&mut second));
    }

    #[test]
    fn peer_copy_bounds_dependencies_and_admits_source_device_events() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        let source_stream = context.create_stream(source_device).unwrap();
        let destination_stream = context.create_stream(destination_device).unwrap();
        let source = context
            .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let destination = context
            .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(source_device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let submission = context
            .launch(
                source_stream,
                &kernel,
                &AddArguments {
                    allocation: source,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        let source_event = context.record_event(&submission).unwrap();
        let source_region = RuntimeMemoryRegionV1 {
            allocation: source,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 4,
        };
        let destination_region = RuntimeMemoryRegionV1 {
            allocation: destination,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: 4,
        };

        context
            .peer_copy(
                destination_stream,
                source_region,
                destination_region,
                &[source_event],
            )
            .unwrap();
        assert_eq!(context.backend().last_dependency_count, 1);
        assert!(matches!(
            context.peer_copy(
                destination_stream,
                source_region,
                destination_region,
                &[source_event, source_event],
            ),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::DuplicateDependency
            ))
        ));
        let excessive = vec![source_event; MAX_RUNTIME_DEPENDENCIES_V1 + 1];
        assert!(matches!(
            context.peer_copy(
                destination_stream,
                source_region,
                destination_region,
                &excessive,
            ),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::TooManyDependencies
            ))
        ));
    }

    #[test]
    fn terminal_backend_failure_marks_the_context_lost() {
        let mut context = RuntimeContextV1::open(MockBackend {
            terminal_on_submit: true,
            ..MockBackend::default()
        })
        .unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let result = context.launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        );
        assert!(matches!(result, Err(RuntimeErrorV1::BackendTerminal(_))));
        assert!(context.is_terminal());
        assert!(matches!(
            context.release_allocation(allocation),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextTerminal
            ))
        ));
    }

    #[test]
    fn facade_rejects_oversized_backend_descriptions_and_kernel_symbols() {
        for backend in [
            MockBackend {
                device_name_len: MAX_RUNTIME_DEVICE_NAME_BYTES_V1 + 1,
                ..MockBackend::default()
            },
            MockBackend {
                device_target_len: MAX_RUNTIME_DEVICE_TARGET_BYTES_V1 + 1,
                ..MockBackend::default()
            },
        ] {
            assert!(matches!(
                RuntimeContextV1::open(backend),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidBackendDescription
                ))
            ));
        }

        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let module = context.load_module(device, b"object").unwrap();
        assert!(matches!(
            context.resolve_kernel::<AddArguments>(
                module,
                &"k".repeat(MAX_RUNTIME_KERNEL_NAME_BYTES_V1 + 1),
            ),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::KernelNameTooLong
            ))
        ));
        assert!(matches!(
            context.resolve_kernel::<AddArguments>(module, "bad\0symbol"),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidKernelName
            ))
        ));
    }

    #[test]
    fn module_image_limit_matches_hsaco_and_accepts_its_exact_boundary() {
        assert_eq!(
            MAX_RUNTIME_MODULE_IMAGE_BYTES_V1,
            fe2o3_hsaco::MAX_HSACO_BYTES
        );
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let exact = vec![1; MAX_RUNTIME_MODULE_IMAGE_BYTES_V1];
        assert!(context.load_module(device, &exact).is_ok());
        drop(exact);
        let oversized = vec![1; MAX_RUNTIME_MODULE_IMAGE_BYTES_V1 + 1];
        assert!(matches!(
            context.load_module(device, &oversized),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ModuleTooLarge
            ))
        ));
    }

    #[test]
    fn argument_encoder_outputs_are_bounded_before_binding_validation() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<HostileArguments>(module, "hostile")
            .unwrap();

        assert!(matches!(
            context.launch(
                stream,
                &kernel,
                &HostileArguments {
                    allocation,
                    kernarg_len: MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 + 1,
                    binding_count: 0,
                },
                geometry(),
                &[],
            ),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::KernargTooLarge
            ))
        ));
        assert!(matches!(
            context.launch(
                stream,
                &kernel,
                &HostileArguments {
                    allocation,
                    kernarg_len: 8,
                    binding_count: fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 + 1,
                },
                geometry(),
                &[],
            ),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::TooManyBindings
            ))
        ));
    }

    #[test]
    fn shutdown_quiesces_streams_before_releasing_dependent_resources() {
        let context = context_with_cleanup_resources(MockBackend::default());
        let backend = context.shutdown().unwrap();
        assert_eq!(
            backend
                .cleanup_log
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [
                MockCleanupKind::Stream,
                MockCleanupKind::Event,
                MockCleanupKind::Submission,
                MockCleanupKind::Module,
                MockCleanupKind::Allocation,
            ]
        );
        assert!(backend.memory.is_empty());
    }

    #[test]
    fn rejected_stream_cleanup_retains_dependencies_and_can_retry() {
        let context = context_with_cleanup_resources(MockBackend {
            cleanup_failure: MockCleanupFailure::RejectStreamOnce,
            ..MockBackend::default()
        });
        let failure = context.shutdown().unwrap_err();
        assert_eq!(
            failure.report().retained(),
            RuntimeRetainedResourcesV1 {
                streams: 1,
                events: 1,
                submissions: 1,
                modules: 1,
                allocations: 1,
            }
        );
        assert_eq!(failure.report().failures().len(), 1);
        assert!(matches!(
            failure.report().failures()[0].failure(),
            RuntimeBackendFailureV1::Rejected(_)
        ));
        let mut context = failure.into_context();
        let retry = context.cleanup();
        assert!(retry.is_complete());
        assert_eq!(
            context
                .backend()
                .cleanup_log
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [
                MockCleanupKind::Stream,
                MockCleanupKind::Stream,
                MockCleanupKind::Event,
                MockCleanupKind::Submission,
                MockCleanupKind::Module,
                MockCleanupKind::Allocation,
            ]
        );
    }

    #[test]
    fn quiescent_stream_failure_allows_dependent_cleanup_and_retains_stream() {
        let context = context_with_cleanup_resources(MockBackend {
            cleanup_failure: MockCleanupFailure::QuiescentStreamOnce,
            ..MockBackend::default()
        });
        let failure = context.shutdown().unwrap_err();
        assert_eq!(
            failure.report().retained(),
            RuntimeRetainedResourcesV1 {
                streams: 1,
                events: 0,
                submissions: 0,
                modules: 0,
                allocations: 0,
            }
        );
        assert!(matches!(
            failure.report().failures()[0].failure(),
            RuntimeBackendFailureV1::Quiescent(_)
        ));
        let mut context = failure.into_context();
        assert!(context.cleanup().is_complete());
    }

    #[test]
    fn rejected_event_cleanup_blocks_module_and_allocation_release() {
        let context = context_with_cleanup_resources(MockBackend {
            cleanup_failure: MockCleanupFailure::RejectEventOnce,
            ..MockBackend::default()
        });
        let failure = context.shutdown().unwrap_err();
        assert_eq!(
            failure.report().retained(),
            RuntimeRetainedResourcesV1 {
                streams: 0,
                events: 1,
                submissions: 1,
                modules: 1,
                allocations: 1,
            }
        );
        assert_eq!(
            failure
                .context()
                .backend()
                .cleanup_log
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [MockCleanupKind::Stream, MockCleanupKind::Event]
        );
        let mut context = failure.into_context();
        assert!(context.cleanup().is_complete());
    }

    #[test]
    fn terminal_cleanup_stops_calls_and_retains_unprocessed_resources() {
        let context = context_with_cleanup_resources(MockBackend {
            cleanup_failure: MockCleanupFailure::TerminalEvent,
            ..MockBackend::default()
        });
        let failure = context.shutdown().unwrap_err();
        assert!(failure.report().is_terminal());
        assert_eq!(
            failure.report().retained(),
            RuntimeRetainedResourcesV1 {
                streams: 0,
                events: 1,
                submissions: 1,
                modules: 1,
                allocations: 1,
            }
        );
        let mut context = failure.into_context();
        assert_eq!(
            context
                .backend()
                .cleanup_log
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [MockCleanupKind::Stream, MockCleanupKind::Event]
        );
        let second = context.cleanup();
        assert!(second.is_terminal());
        assert_eq!(context.backend().cleanup_log.len(), 2);
    }

    #[test]
    fn launch_rejects_invalid_or_nonzero_pointer_patches() {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<PatchArguments>(module, "patches")
            .unwrap();

        for arguments in [
            PatchArguments {
                allocation,
                offsets: [0, 16],
                kernarg_len: 16,
                patch_fill: 0,
            },
            PatchArguments {
                allocation,
                offsets: [0, 4],
                kernarg_len: 16,
                patch_fill: 0,
            },
            PatchArguments {
                allocation,
                offsets: [0, 0],
                kernarg_len: 16,
                patch_fill: 0,
            },
            PatchArguments {
                allocation,
                offsets: [0, 8],
                kernarg_len: 16,
                patch_fill: 1,
            },
        ] {
            assert!(matches!(
                context.launch(stream, &kernel, &arguments, geometry(), &[]),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidKernargPatch
                ))
            ));
        }
    }
}
