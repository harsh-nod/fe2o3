//! Backend-neutral runtime context, typed launches, streams, events, and memory.

use core::fmt;
use core::marker::PhantomData;
use fe2o3_runtime_model::{IdentityDigestV1, PeerTransferMechanismV1, TypedAsyncKernelV1};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::panic::{AssertUnwindSafe, UnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

macro_rules! completion_settlement_rust_expr {
    ($body:expr) => {
        $body
    };
}
include!("context/completion_settlement_body.rs");
include!("context/cached_poll_body.rs");

mod graph;
pub(crate) use graph::*;
pub use graph::{RuntimeGraphDeviceCoverageV1, RuntimeGraphGroupV1};
mod allocation_admission;
mod allocation_witness;
pub use allocation_witness::*;
mod worker_admission;
pub use worker_admission::RuntimeWorkerRequestOwnerV1;
mod copy_custody;
mod drain;
mod drain_capture;
use copy_custody::SameDeviceCopyRootV1;
mod generated_issue;
mod generated_preparation;
mod generated_scope;
pub use generated_scope::*;
mod generated_shells;
mod peer_batch;
mod scope_epoch;
pub use peer_batch::*;
mod native_retained_pair;
pub use native_retained_pair::*;
mod peer_custody;
use peer_custody::{PreparedPeerSubmissionV1, ScalarPeerCopyRootV1};
mod peer_directed;
pub use peer_directed::*;
mod peer_directed_context;
mod peer_reconciliation;
mod producer_launch;
pub use producer_launch::{
    BackendLaunchProducerV1, BackendProducerAwareLaunchV1, RuntimeProducerAwareLaunchBackendV1,
};
use producer_launch::{PreparedSubmissionCustodyV1, ProducerLaunchRootV1};
mod peer_placement;
mod peer_segments;
pub use peer_placement::*;
#[cfg(feature = "hardware-qualification")]
mod qualification_allocation_version;
#[cfg(feature = "hardware-qualification")]
mod qualification_generated_copy;
#[cfg(feature = "hardware-qualification")]
pub use qualification_allocation_version::RuntimeAllocationVersionObservationV1;
#[cfg(feature = "hardware-qualification")]
mod qualification_xgmi;
use peer_segments::SegmentedPeerCopyRootV1;
pub use peer_segments::*;
mod replicas;
mod unpublished;
mod versions;
use allocation_admission::ContextAllocationAdmissionV1;
pub use drain_capture::*;
pub use generated_preparation::*;
pub use replicas::*;
pub(crate) use unpublished::ContextUnpublishedHoldV1;
use versions::{
    ContextReadSourceV1, ContextVersionsV1, SubmissionReaderMarkerV1, SubmissionWriterDomainV1,
    SubmissionWriterOutcomeV1,
};
pub use versions::{RuntimeContextJournalUsageV1, RuntimeContextOpenFailureV1};

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
/// Maximum number of pending completion callbacks retained by one context.
pub const MAX_RUNTIME_COMPLETION_CALLBACKS_V1: usize = 1_048_576;
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

#[cfg(test)]
pub(crate) const fn resource_credit_test_device_v1() -> RuntimeDeviceIdV1 {
    RuntimeDeviceIdV1::new(1, 2)
}

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

/// Optional execution-detail inventory outside the stable Worker V3 bitset.
///
/// Backends must opt in field by field. The default is deliberately all false,
/// so an older backend cannot accidentally advertise a native mechanism merely
/// because it implements the corresponding logical operation cooperatively.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeExecutionCapabilitiesV1 {
    pub native_async_copy: bool,
    pub native_peer_copy: bool,
    pub concurrent_compute: bool,
    pub compute_copy_overlap: bool,
    pub memory_pool: bool,
    pub profiling: bool,
    pub cancellation: bool,
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
    /// Global work-item extent published in the AQL grid-size fields.
    pub grid: [u32; 3],
    /// Work-items in one workgroup along each active dimension.
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

    fn has_complete_workgroups(self) -> bool {
        self.grid
            .into_iter()
            .zip(self.workgroup)
            .all(|(grid, workgroup)| grid >= workgroup && grid.is_multiple_of(workgroup))
    }
}

/// Memory visibility scope declared by an atomic or collective kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMemoryScopeV1 {
    Workgroup,
    Device,
    System,
}

/// Ordering declared for the memory effects of an atomic or collective kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMemoryOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

/// Read-modify-write operation implemented by an admitted typed kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAtomicOperationV1 {
    Add,
    Minimum,
    Maximum,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Exchange,
    CompareExchange,
}

/// Explicit semantic and launch-shape contract for an atomic kernel.
///
/// The runtime validates this value against [`RuntimeAtomicArgumentsV1`] and
/// preserves it through [`RuntimeAtomicBackendV1`]. This contract does not
/// imply a backend-native intrinsic or prove that the kernel implementation
/// satisfies the declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAtomicLaunchContractV1 {
    pub operation: RuntimeAtomicOperationV1,
    pub scope: RuntimeMemoryScopeV1,
    /// Success ordering, or the sole ordering for non-compare-exchange operations.
    pub order: RuntimeMemoryOrderV1,
    /// Compare-exchange failure ordering; `None` for every other operation.
    pub failure_order: Option<RuntimeMemoryOrderV1>,
    /// Selects weak compare-exchange. Must be false for every other operation.
    pub weak: bool,
    pub geometry: RuntimeLaunchGeometryV1,
}

/// Typed arguments that declare the atomic semantics of their admitted kernel.
pub trait RuntimeAtomicArgumentsV1: RuntimeArgumentsV1 {
    const OPERATION_V1: RuntimeAtomicOperationV1;
    const SCOPE_V1: RuntimeMemoryScopeV1;
    const ORDER_V1: RuntimeMemoryOrderV1;
    const FAILURE_ORDER_V1: Option<RuntimeMemoryOrderV1> = None;
    const WEAK_V1: bool = false;
}

/// Collective operation implemented by an admitted typed kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCollectiveOperationV1 {
    Barrier,
    Broadcast,
    ReduceSum,
    ReduceMinimum,
    ReduceMaximum,
    AllReduceSum,
    InclusiveScanSum,
}

/// Explicit semantic, participation, and launch-shape contract for a collective.
///
/// `participants` must equal the workgroup size for workgroup scope and the
/// complete grid size for device scope. System scope is rejected because a
/// single-stream launch cannot identify a cross-device participant set.
/// Execution requires [`RuntimeCollectiveBackendV1`]; the runtime does not
/// synthesize a collective.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCollectiveLaunchContractV1 {
    pub operation: RuntimeCollectiveOperationV1,
    pub scope: RuntimeMemoryScopeV1,
    pub order: RuntimeMemoryOrderV1,
    pub participants: u64,
    pub geometry: RuntimeLaunchGeometryV1,
}

/// Typed arguments that declare the collective semantics of their admitted kernel.
pub trait RuntimeCollectiveArgumentsV1: RuntimeArgumentsV1 {
    const OPERATION_V1: RuntimeCollectiveOperationV1;
    const SCOPE_V1: RuntimeMemoryScopeV1;
    const ORDER_V1: RuntimeMemoryOrderV1;
}

/// Marker identifying an atomic submission while preserving its argument type.
pub struct RuntimeAtomicLaunchV1<A>(PhantomData<fn(A) -> A>);

/// Marker identifying a collective submission while preserving its argument type.
pub struct RuntimeCollectiveLaunchV1<A>(PhantomData<fn(A) -> A>);

/// Semantic class retained across the backend launch boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendSemanticLaunchV1 {
    Ordinary,
    Atomic(RuntimeAtomicLaunchContractV1),
    Collective(RuntimeCollectiveLaunchContractV1),
}

/// Backend launch description after all context-local validation.
#[derive(Clone, Copy, Debug)]
pub struct BackendLaunchV1<'a> {
    pub stream: u64,
    pub kernel: u64,
    pub explicit_kernarg: &'a [u8],
    pub bindings: &'a [BackendBindingV1],
    pub dependencies: &'a [u64],
    pub geometry: RuntimeLaunchGeometryV1,
    pub semantic_launch: BackendSemanticLaunchV1,
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

/// Allocation-specific custody outcome, stronger than generic quiescence.
#[derive(Debug)]
pub enum RuntimeBackendAllocationOutcomeV1<E> {
    /// Transfers custody of a nonzero, unique live allocation handle.
    Allocated(u64),
    /// The attempt failed, but no requested allocation owner or pending allocation
    /// root remains. Native disposal and model/currentness settlement are complete,
    /// no preexisting logical allocation owner is consumed, and the backend
    /// remains live and retryable. Internal queue, pool, and model bookkeeping
    /// may advance.
    ///
    /// Backend-owned queue infrastructure created by this attempt may remain;
    /// this outcome authorizes refund of only the requested allocation bytes and
    /// record. Generic quiescence, a missing public handle, or an empty allocation
    /// index alone does not establish this outcome. Context preserves the original
    /// error as `BackendQuiescent` after refunding the attempt's charge.
    SettledNoOwner(E),
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
/// Implementations must never silently ignore [`BackendLaunchV1::semantic_launch`].
/// A backend without the corresponding additive atomic or collective SPI must
/// reject a non-[`BackendSemanticLaunchV1::Ordinary`] launch before accepting
/// custody. A supporting backend must preserve the exact semantic contract;
/// its additive SPI must reject the wrong semantic variant.
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

    /// Reports execution details that are not representable in
    /// `RuntimeCapabilitiesV1` or the frozen Runtime Worker V1 capability
    /// bitset. A backend may report capabilities only for a
    /// currently admitted device handle; unknown or superseded handles must
    /// return the all-false record. Worker-backed implementations bind admitted
    /// handles to their most recent successful enumeration, while an in-process
    /// backend may know its fixed admitted roster at construction.
    /// Implementations inherit the fail-closed default.
    fn execution_capabilities_v1(&self, _device: u64) -> RuntimeExecutionCapabilitiesV1 {
        RuntimeExecutionCapabilitiesV1::default()
    }

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>>;

    /// Returns the complete immutable request-account roster for the enumerated
    /// devices. Required backends retain their policy after native startup and
    /// reject witness-free logical allocation, including after Context shutdown.
    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<RuntimeAllocationAdmissionProfileV1, RuntimeBackendFailureV1<Self::Error>> {
        Ok(RuntimeAllocationAdmissionProfileV1::Legacy)
    }

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

    /// Allocates with an optional explicit no-owner settlement guarantee.
    ///
    /// The default preserves every legacy failure unchanged. Implementations
    /// must not upgrade generic `Quiescent` failures without establishing the
    /// stronger [`RuntimeBackendAllocationOutcomeV1::SettledNoOwner`] contract.
    fn allocate_with_outcome_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<RuntimeBackendAllocationOutcomeV1<Self::Error>, RuntimeBackendFailureV1<Self::Error>>
    {
        self.allocate_v1(device, kind, byte_len, alignment)
            .map(RuntimeBackendAllocationOutcomeV1::Allocated)
    }

    /// Authenticates the exact retained request before any allocation effects.
    /// The default fails closed without invoking either legacy allocation entry.
    fn allocate_with_request_v1(
        &mut self,
        _device: u64,
        _kind: RuntimeMemoryKindV1,
        _byte_len: u64,
        _alignment: u64,
        _witness: RuntimeAllocationRequestWitnessV1<'_>,
    ) -> RuntimeRequestAllocationResultV1<Self::Error> {
        RuntimeRequestAllocationResultV1::Unsupported
    }

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

    /// Reads into caller-owned storage. A failure or panic after reading begins
    /// may leave partial data in `destination`; those bytes are not a successful read.
    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    /// Copies already-coherent host bytes after private owner-drain quiescence.
    ///
    /// This must not publish work, poll completion, flush, synchronize, allocate
    /// native storage or create a readback buffer. Implementations validate the
    /// exact live native storage and currentness before and after the CPU copy.
    /// Bounded observation-only reset/currentness syscalls are permitted. Errors
    /// are fixed-size; `Terminal` retains native custody and seals the context.
    /// No bytes from an unsuccessful capture are delivered to the observer.
    fn capture_coherent_host_range_v1(
        &mut self,
        _request: BackendHostCaptureV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>> {
        Err(RuntimeBackendFailureV1::Rejected(
            RuntimeHostCaptureErrorV1::UnsupportedBackend,
        ))
    }

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

    /// Opt in to checked contained DeviceLocal peer reads behind an exact pending
    /// producer-aware full-allocation Write. Context additionally requires its
    /// version journal and exact writer lease; this flag grants neither kernel
    /// authority nor initialized data. Source and destination windows may differ
    /// in offset and allocation extent, but must have the same positive length.
    ///
    /// Authenticate the explicit event, producer-aware submission, original source
    /// allocation and complete writable coverage. Retain the producer independently
    /// of public events. No peer effect may begin before every dependency succeeded
    /// and the producer's original owners were restored. Admission and observations
    /// must not progress the producer or acquire physical peer custody. Explicit
    /// peer/dependent progress must service the retained producer within the existing
    /// bounded progress contract. Unknown/failed completion is never success.
    /// Cancellation before peer custody releases only that consumer's retains;
    /// uncertain custody remains rooted. Ordinary, non-producer-aware native compute
    /// must not enter this profile. Context independently reconciles logical parents
    /// before committing the peer's destination version.
    fn supports_pending_compute_peer_copy_v1(&self) -> bool {
        false
    }

    /// Opt in to an immutable ordered peer segment list behind one exact pending
    /// producer-aware full-allocation Write. Context requires its version journal,
    /// the exact current writer/event, and checked source and destination envelopes.
    /// This is distinct from scalar range coverage and grants no kernel authority.
    ///
    /// Validate and retain the complete ordered list and original allocations before
    /// effects. Retain every dependency independently of public events; issue no
    /// segment until every dependency succeeded and original owners were restored.
    /// Keep one logical result and whole-owner custody until the complete list and
    /// closing checks finish. Cancellation is no-effect only before first publication;
    /// a failed or uncertain prefix cannot authorize a dependent consumer.
    ///
    /// Authenticate an initialized whole destination and preserve every byte outside
    /// the listed writes, including envelope gaps. Pending producer-aware Read-only
    /// consumers and opted-in peer readbacks may read that original preserved frame,
    /// but must authenticate the exact list event and wait for final success/restoration.
    /// Admission and observation must not progress parents or acquire conflicting
    /// physical custody. Explicit dependent progress services the retained chain.
    /// Context independently observes logical parents before committing versions.
    /// Pending destination-writer chaining is not part of this profile.
    fn supports_pending_compute_peer_copy_segments_v1(&self) -> bool {
        false
    }

    /// Opt in to a retained destination frame for ordered lists with a settled source.
    ///
    /// Authenticate initialized original source and destination allocations and
    /// retain the immutable complete descriptor list and original endpoint identities.
    /// Every descriptor affects only its checked destination window; gaps retain
    /// their initialized contents. This is a whole-allocation frame guarantee,
    /// not a claim that either the list or its bounding envelope writes every byte.
    /// Context holds an ordinary current-version source read lease and one whole
    /// destination writer. Explicit control dependencies must already be successful
    /// and quiescent. This does not admit pending source or destination writers.
    ///
    /// Pending Read-only compute and readback consumers must authenticate the exact
    /// retained list event/frame, independently retain the list after public event
    /// release, and wait for whole-list success and original-owner restoration.
    /// Cancellation, failed or unknown completion cannot authorize a consumer.
    /// This capability is independent of pending-compute-source list support.
    fn supports_peer_copy_segments_frame_v1(&self) -> bool {
        false
    }

    /// Opt in to a scalar peer read of an exact pending segmented destination frame.
    ///
    /// Context requires its version journal, the latest writer's explicit event,
    /// and a retained immutable frame identity with strictly older dependency ranks.
    /// The source is a checked Read window of the original initialized DeviceLocal
    /// frame, including preserved gaps, not a fabricated compute producer or a
    /// claim that the descriptor envelope wrote every byte. The destination is a
    /// fresh initialized DeviceLocal allocation with an equally sized Write window.
    /// Preserve its initialized complement. Opted-in pending readbacks may consume
    /// that whole destination only behind this scalar copy's exact retained event,
    /// successful completion, and original-owner restoration.
    ///
    /// Retain the frame, original endpoint identities, and every dependency after
    /// public event release. Admission and observation are inert, including after
    /// native publication, when both original endpoint ownership markers must be
    /// authenticated. Explicit dependent progress services the bounded chain; no
    /// source extraction or peer effect may begin until every dependency actually
    /// succeeds and original owners are restored. Quiescence alone is not success.
    /// Failure, cancellation, and Unknown cannot authorize a consumer or commit a
    /// destination version. No staged fallback, pending destination writer, list
    /// consumer, or kernel authority is granted by this capability.
    fn supports_pending_segment_frame_peer_copy_v1(&self) -> bool {
        false
    }

    /// Opt in to an ordered list reading an exact pending segmented frame.
    ///
    /// Requires the version journal, latest source-writer event, immutable frame
    /// receipt and complete descriptor snapshot. Source and destination envelopes
    /// may differ in length. Retain both original owners until every descriptor
    /// and closing check completes; initialized destination gaps are preserved.
    /// No extraction or publication precedes actual parent success and restoration.
    /// This is independent of scalar forwarding and pending-compute-source support.
    /// The destination must be fresh and initialized; pending destination writers
    /// are excluded. Exact-event full-frame readback is supported, but pending
    /// compute consumers require separate qualification. No kernel authority or
    /// staged fallback follows from this capability.
    fn supports_pending_segment_frame_peer_copy_segments_v1(&self) -> bool {
        false
    }

    /// Opt in to success-ordered lists sharing one initialized destination frame.
    ///
    /// Requires `supports_peer_copy_segments_frame_v1` and Context's version
    /// journal. Each source is settled and retains its ordinary read lease. A
    /// pending destination must name its exact latest segmented writer with an
    /// explicit event on the same stream and original allocation. Retain the
    /// immutable list/frame and predecessor identities independently of public
    /// event release, with bounded strictly older dependency ranks.
    ///
    /// Acquire the destination owner and publish a successor only after actual
    /// predecessor success and restoration. Ordered windows, including overlaps,
    /// preserve initialized bytes outside those windows. Read-only compute and
    /// readback consumers of the latest whole frame obey the same success chain;
    /// failure, cancellation, and Unknown never promote it. This does not grant
    /// scalar coverage or admit pending compute sources into ordered list chains.
    fn supports_ordered_peer_copy_segments_v1(&self) -> bool {
        false
    }

    /// Opt in to ordered destination-list chains with pending compute sources.
    ///
    /// Requires both pending-compute segment support and ordered settled-list
    /// frame support; neither capability alone implies this composition. Each
    /// pending source retains its exact current producer-aware full-allocation
    /// Write/event independently of the latest same-stream destination writer.
    /// Settled and compute-backed list origins may mix only with exact immutable
    /// source, destination frame, and predecessor identities and bounded ranks.
    ///
    /// Retain both dependency paths after public event release. Do not acquire
    /// conflicting owners or publish until every dependency actually succeeds
    /// and restores its owners. Whole-frame consumers wait for the entire chain;
    /// cancellation, failure, or Unknown cannot promote a destination version.
    /// Context independently reconciles every logical parent. This grants no
    /// scalar envelope coverage, kernel authority, or pending unrelated controls.
    fn supports_ordered_pending_compute_peer_copy_segments_v1(&self) -> bool {
        false
    }

    /// Opt in to success-ordered partial peer writes into one initialized allocation.
    ///
    /// This additionally requires `supports_pending_compute_peer_copy_v1`. Each
    /// pending destination predecessor must be the exact retained earlier writer
    /// on the same stream and original allocation, with a bounded acyclic chain.
    /// A successor must not acquire the destination owner or publish any transfer
    /// until every predecessor succeeded and restored its original owners. Public
    /// event release must not remove this dependency. Failure, cancellation, or
    /// unknown completion cannot authorize a successor or a dependent readback.
    ///
    /// The backend authenticates that the entire original destination is initialized
    /// and that every write affects only its checked window, preserving all other
    /// bytes. This frame guarantee permits a pending readback of the final whole
    /// allocation, not a claim that the last partial peer produced every byte.
    /// Whole-allocation custody remains serialized, including overlapping windows.
    /// Backends supporting pending peer readback or producer-aware compute consumers
    /// must apply the same exact retained chain and original-owner restoration checks
    /// before a dependent effect. Compute aliases must each be checked Read ranges
    /// of the original initialized frame; writable aliases gain no extra authority.
    fn supports_ordered_compute_peer_copy_v1(&self) -> bool {
        false
    }

    /// Optional read-only placement estimate for these exact original regions.
    /// `None` refuses selection. An estimate reserves nothing and grants no
    /// route, currentness, residency or future submission authority.
    fn observe_peer_copy_placement_v1(
        &self,
        _stream: u64,
        _source: BackendMemoryRegionV1,
        _destination: BackendMemoryRegionV1,
    ) -> Option<BackendPeerCopyPlacementV1> {
        None
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

/// Optional nonblocking same-device copy SPI.
///
/// This extension makes the operation explicit at the backend type boundary
/// without silently changing the Runtime Worker V1 wire contract. Runtime
/// Worker V1 has no per-device same-device-copy capability bit, so
/// implementations may reject the operation as unsupported. Negotiated Runtime
/// Worker V4 encodes this SPI explicitly. Implementations may use a native copy
/// engine or bounded cooperative host progress, but must document which.
/// Successful submission retains source and destination against mutation until
/// conclusive completion; the submission handle remains retained until
/// [`RuntimeBackendV1::release_submission_v1`].
pub trait RuntimeAsyncCopyBackendV1: RuntimeBackendV1 {
    /// Opt in to journal-aware DeviceLocal-to-HostVisible readback behind an
    /// exact pending ordinary peer producer on the readback device.
    ///
    /// The backend must authenticate the explicit producer event, destination
    /// allocation and covered read range, retain each pending dependency or an
    /// authenticated immutable success independently of public events, and
    /// issue no read until all dependencies succeeded and
    /// the peer's original owners were restored. Failure or unknown completion
    /// must not become consumer success. Admission must not acquire child owner
    /// custody that prevents the peer from progressing. Existing cancellation,
    /// uncertain-custody and observer-only completion contracts still apply.
    /// This is not a capability-bit or Worker-protocol authorization.
    /// Context retains its logical producer until its own reconciliation even
    /// when the backend already has conclusive success at admission.
    fn supports_pending_peer_readback_v1(&self) -> bool {
        false
    }

    /// Opt in to the same readback contract for exact pending directed peer
    /// producers, including their retained transitive dependencies and resource
    /// blockers. Explicit consumer progress must advance those ancestors without
    /// depending on public producer events or reserving child SDMA ahead of them.
    /// This is independent of the ordinary-peer opt-in above and defaults off.
    fn supports_pending_directed_peer_readback_v1(&self) -> bool {
        false
    }

    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

/// Optional atomic-kernel submission SPI.
///
/// The launch must carry [`BackendSemanticLaunchV1::Atomic`] with the validated
/// operation, scope, ordering, compare-exchange mode, and geometry.
/// Implementations must reject the wrong variant or a contract that is not
/// covered by their native execution authority before accepting custody.
pub trait RuntimeAtomicBackendV1: RuntimeBackendV1 {
    fn submit_atomic_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

/// Optional collective-kernel submission SPI.
///
/// The launch must carry [`BackendSemanticLaunchV1::Collective`] with the
/// validated operation, scope, ordering, participation, and geometry.
/// Implementations must reject the wrong variant or an unsupported contract
/// before accepting custody.
pub trait RuntimeCollectiveBackendV1: RuntimeBackendV1 {
    fn submit_collective_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

/// Additive full-publication and cooperative-progress SPI.
///
/// Remote full flush is encoded only by negotiated Runtime Worker V4.
pub trait RuntimeFlushBackendV1: RuntimeBackendV1 {
    /// For native work, success establishes that every dependency-ready operation
    /// in the stream's backend scheduling domain at entry was published. A
    /// cooperative backend with no native publication point may instead drive
    /// its retained, explicitly bounded host operation to a conclusive state;
    /// it must document the work bound and blocking behavior. Recoverable
    /// prepublication or cooperative-progress failure must be returned as an
    /// error, not hidden behind success. This does not wait for native completion
    /// or provide background progress. A backend whose bounded publication
    /// window cannot hold the complete ready native set must reject before mutation.
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    /// Makes one cooperative progress attempt in the stream's scheduling domain.
    ///
    /// Unlike `flush_stream_v1`, success may leave dependency-ready work
    /// unpublished. Backends document their own work bound; this method does not
    /// impose a generic time bound or imply completion. Errors retain the same
    /// rejected, quiescent, and terminal meanings as explicit flush.
    ///
    /// The default preserves legacy full-flush behavior, including for negotiated
    /// Worker V4 adapters; it introduces no additional wire operation.
    fn progress_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.flush_stream_v1(stream)
    }
}

/// Explicit native teardown for a backend created on an async owner thread.
///
/// Called only after context cleanup discharged every logical handle. Success
/// must also discharge native queues, pools, and other backend-owned custody,
/// making ordinary backend Drop safe. Any error retains the complete backend;
/// neither rejection nor a timeout is permission to drop it. This is a
/// contracted adapter boundary, not evidence of executable verification.
pub trait RuntimeOwnedShutdownBackendV1: RuntimeBackendV1 {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;
}

/// Backend result of a cancellation attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendCancellationV1 {
    /// The operation was withdrawn before publication and is quiescent.
    Cancelled,
    /// Publication already occurred; custody and completion remain live.
    TooLate,
}

/// Additive cancellation and drain SPI encoded only by negotiated Runtime Worker V4.
pub trait RuntimeCancellationBackendV1: RuntimeBackendV1 {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>>;

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>>;
}

/// Public cancellation observation. `TooLate` retains the submission token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCancellationV1 {
    Cancelled,
    TooLate,
}

const RUNTIME_CANCELLED_CODE_V1: i64 = -2;
const RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1: i64 = -3;

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
    SubmissionRetainedByDependency,
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
    InvalidAtomicContract,
    InvalidCollectiveContract,
    InvalidPeerCopyBatch,
    ContextReserved,
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
    graph_reserved: bool,
    native_pair_reserved: bool,
    scope_reserved: bool,
    replica_pending: usize,
    allocation_credit_records: usize,
    // Journal capacity is bounded by CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1.
    allocation_journal_records: u32,
    writer_journal_records: usize,
    reader_journal_records: usize,
    scalar_peer_copy_records: u32,
    // The submission admission bound fits u32; keep shutdown errors inline.
    producer_launch_records: u32,
    same_device_copy_records: u32,
    segmented_peer_copy_records: u32,
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
        !self.terminal
            && !self.graph_reserved
            && !self.native_pair_reserved
            && !self.scope_reserved
            && self.replica_pending == 0
            && self.retained.is_empty()
            && self.allocation_credit_records == 0
            && self.allocation_journal_records == 0
            && self.writer_journal_records == 0
            && self.reader_journal_records == 0
            && self.scalar_peer_copy_records == 0
            && self.producer_launch_records == 0
            && self.same_device_copy_records == 0
            && self.segmented_peer_copy_records == 0
    }

    pub const fn is_graph_reserved(&self) -> bool {
        self.graph_reserved
    }

    /// An unfinished native retained-pair facade still owns the context gate.
    pub const fn is_native_pair_reserved_v1(&self) -> bool {
        self.native_pair_reserved
    }

    /// A lexical generated scope still retains this Context, even if empty or forgotten.
    pub const fn is_generated_scope_reserved_v1(&self) -> bool {
        self.scope_reserved
    }

    /// Tracked copies still requiring original retirement; not replica availability.
    pub const fn pending_replica_copies_v1(&self) -> usize {
        self.replica_pending
    }

    /// Remaining opt-in allocation credit records, including unidentified
    /// quarantined attempts. This count is not a full native resource inventory.
    pub const fn allocation_credit_records_v1(&self) -> usize {
        self.allocation_credit_records
    }

    /// Opt-in journal records, including attempts with no returned handle.
    pub const fn allocation_journal_records_v1(&self) -> usize {
        self.allocation_journal_records as usize
    }

    /// Retained writer metadata; this is not an available-data count.
    pub const fn writer_journal_records_v1(&self) -> usize {
        self.writer_journal_records
    }

    /// Retained input custody, including provisional roots without a returned handle.
    pub const fn reader_journal_records_v1(&self) -> usize {
        self.reader_journal_records
    }

    /// Scalar provenance roots, including backend-entered attempts without a handle.
    /// Completed roots are metadata only; they are removed with their submission.
    pub const fn scalar_peer_copy_records_v1(&self) -> usize {
        self.scalar_peer_copy_records as usize
    }

    /// Retained producer-aware launch roots, including attempts without a handle.
    pub const fn producer_launch_records_v1(&self) -> usize {
        self.producer_launch_records as usize
    }

    /// Retained same-device readback roots, including attempts without a handle.
    pub const fn same_device_copy_records_v1(&self) -> usize {
        self.same_device_copy_records as usize
    }

    pub const fn segmented_peer_copy_records_v1(&self) -> usize {
        self.segmented_peer_copy_records as usize
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
    unpublished: Option<u64>,
    generated: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationRecordV1 {
    backend_allocation: u64,
    device: RuntimeDeviceIdV1,
    kind: RuntimeMemoryKindV1,
    byte_len: u64,
    journal: Option<fe2o3_runtime_model::ContextAllocationReferenceV1>,
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
    status: RuntimeCompletionStatusV1,
    journal_writer: Option<fe2o3_runtime_model::ContextWriterReferenceV1>,
    journal_read: Option<SubmissionReaderMarkerV1>,
    journal_producer_read: Option<versions::SubmissionProducerReaderMarkerV1>,
    scalar_peer_copy: bool,
    directed_peer_copy: bool,
    producer_launch: bool,
    same_device_copy: bool,
    segmented_peer_copy: bool,
    segmented_destination: Option<RuntimeAllocationIdV1>,
    dependency_retains: usize,
}

type RuntimeCompletionCallbackV1 =
    Box<dyn FnOnce(RuntimeCompletionStatusV1) + Send + UnwindSafe + 'static>;

fn completion_callback_panicked_v1(
    callback: impl FnOnce(RuntimeCompletionStatusV1),
    status: RuntimeCompletionStatusV1,
) -> bool {
    match catch_unwind(AssertUnwindSafe(|| callback(status))) {
        Ok(()) => false,
        Err(payload) => {
            // A callback can panic with a payload whose destructor also panics.
            core::mem::forget(payload);
            true
        }
    }
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
    // Fail stop before backend/resource destruction after a forgotten scope.
    scope_epoch: scope_epoch::Anchor,
    // A forgotten tracked copy must fail stop before backend destruction.
    replicas: Option<RuntimeReplicaStorageV1>,
    backend: B,
    context_generation: u64,
    devices: Vec<RuntimeDeviceV1>,
    streams: HashMap<RuntimeStreamIdV1, StreamRecordV1>,
    backend_streams: HashSet<u64>,
    allocations: HashMap<RuntimeAllocationIdV1, AllocationRecordV1>,
    backend_allocations: HashSet<u64>,
    allocation_admission: ContextAllocationAdmissionV1,
    versions: Option<ContextVersionsV1>,
    modules: HashMap<RuntimeModuleIdV1, ModuleRecordV1>,
    backend_modules: HashSet<u64>,
    kernels: HashMap<u64, KernelRecordV1>,
    events: HashMap<RuntimeEventIdV1, EventRecordV1>,
    backend_events: HashSet<u64>,
    submissions: HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    backend_submissions: HashSet<u64>,
    scalar_peer_copies: HashMap<RuntimeSubmissionIdV1, ScalarPeerCopyRootV1>,
    producer_launches: HashMap<RuntimeSubmissionIdV1, ProducerLaunchRootV1>,
    same_device_copies: HashMap<RuntimeSubmissionIdV1, SameDeviceCopyRootV1>,
    segmented_peer_copies: HashMap<RuntimeSubmissionIdV1, SegmentedPeerCopyRootV1>,
    generated_issues: HashMap<RuntimeStreamIdV1, generated_issue::GeneratedIssueV1>,
    completion_callbacks: HashMap<RuntimeSubmissionIdV1, Vec<RuntimeCompletionCallbackV1>>,
    completion_callback_count: usize,
    completion_callback_panic_count: u64,
    next_identity: u64,
    terminal: bool,
    graph_reservation: Option<ContextGraphReservationV1>,
    native_pair_reservation: Option<u64>,
    graph_issue_closed: bool,
}

struct ContextLaunchRequestV1<'a, A: RuntimeArgumentsV1> {
    stream: RuntimeStreamIdV1,
    kernel: &'a TypedRuntimeKernelV1<A>,
    arguments: ContextLaunchArgumentsV1<'a, A>,
    geometry: RuntimeLaunchGeometryV1,
    dependencies: &'a [RuntimeEventIdV1],
    semantic_launch: BackendSemanticLaunchV1,
}

enum ContextLaunchArgumentsV1<'a, A> {
    Live(&'a A),
    Frozen(&'a [u8], &'a [RuntimeBindingV1]),
}

pub(crate) struct PreparedContextLaunchV1 {
    stream: RuntimeStreamIdV1,
    stream_record: StreamRecordV1,
    kernel: u64,
    explicit_kernarg: Vec<u8>,
    backend_bindings: Vec<BackendBindingV1>,
    journal_destinations: Vec<RuntimeAllocationIdV1>,
    journal_sources: Vec<ContextReadSourceV1>,
    producer_bindings: Vec<ContextReadSourceV1>,
    backend_dependencies: Vec<u64>,
    geometry: RuntimeLaunchGeometryV1,
    semantic_launch: BackendSemanticLaunchV1,
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
    /// Opens without a version journal. Backend initialization unwind retains
    /// the backend until process exit and propagates the original panic.
    /// Ordinary errors retain the legacy behavior of dropping the backend;
    /// use `open_with_version_journal_v1` to recover it on returned failure.
    pub fn open(backend: B) -> Result<Self, RuntimeErrorV1<B::Error>> {
        Self::open_configured_v1(backend, None).map_err(|failure| failure.error)
    }

    fn open_configured_v1(
        backend: B,
        journal: Option<(usize, usize, usize)>,
    ) -> Result<Self, RuntimeContextOpenFailureV1<B>> {
        if journal.is_some_and(|(allocations, writers, members)| {
            let bounds = 1..=fe2o3_runtime_model::CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
            !bounds.contains(&allocations)
                || !bounds.contains(&writers)
                || !bounds.contains(&members)
                || members < allocations
        }) {
            return Err(RuntimeContextOpenFailureV1 {
                backend,
                error: RuntimeValidationErrorV1::Capacity.into(),
            });
        }
        // A backend hook may panic after acquiring native custody. Do not run
        // its destructor before the caller's initializer-panic boundary.
        let mut backend = core::mem::ManuallyDrop::new(backend);
        let initialization = (|| {
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
            let devices: Vec<_> = descriptions
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
            let allocation_admission = ContextAllocationAdmissionV1::from_profile(
                &devices,
                backend
                    .allocation_admission_profile_v1()
                    .map_err(map_backend_error)?,
            )?;
            let versions = journal
                .map(|(allocations, writers, members)| {
                    ContextVersionsV1::new(context_generation, allocations, writers, members)
                        .map_err(|_| RuntimeValidationErrorV1::Capacity)
                })
                .transpose()?;
            Ok((context_generation, devices, versions, allocation_admission))
        })();
        let (context_generation, devices, versions, allocation_admission) = match initialization {
            Ok(initialized) => initialized,
            Err(error) => {
                return Err(RuntimeContextOpenFailureV1 {
                    error,
                    backend: core::mem::ManuallyDrop::into_inner(backend),
                });
            }
        };
        Ok(Self {
            scope_epoch: scope_epoch::Anchor::default(),
            replicas: None,
            context_generation,
            devices,
            streams: HashMap::new(),
            backend_streams: HashSet::new(),
            allocations: HashMap::new(),
            backend_allocations: HashSet::new(),
            allocation_admission,
            versions,
            modules: HashMap::new(),
            backend_modules: HashSet::new(),
            kernels: HashMap::new(),
            events: HashMap::new(),
            backend_events: HashSet::new(),
            submissions: HashMap::new(),
            generated_issues: HashMap::new(),
            backend_submissions: HashSet::new(),
            scalar_peer_copies: HashMap::new(),
            producer_launches: HashMap::new(),
            same_device_copies: HashMap::new(),
            segmented_peer_copies: HashMap::new(),
            completion_callbacks: HashMap::new(),
            completion_callback_count: 0,
            completion_callback_panic_count: 0,
            next_identity: 1,
            terminal: false,
            graph_reservation: None,
            native_pair_reservation: None,
            graph_issue_closed: false,
            // Keep extraction last: earlier field initialization must still
            // retain the backend if it unwinds.
            backend: core::mem::ManuallyDrop::into_inner(backend),
        })
    }

    pub fn devices(&self) -> &[RuntimeDeviceV1] {
        &self.devices
    }

    /// Returns mechanism-level capabilities for one context device.
    pub fn execution_capabilities(
        &self,
        device: RuntimeDeviceIdV1,
    ) -> Result<RuntimeExecutionCapabilitiesV1, RuntimeValidationErrorV1> {
        self.scope_epoch.require_access()?;
        let record = self.device(device)?;
        Ok(self
            .backend
            .execution_capabilities_v1(record.backend_device))
    }

    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    /// Panics if a forgotten generated scope still owns the backend. No backend
    /// reference is exposed until that original scope has actually settled.
    pub fn backend(&self) -> &B {
        assert!(
            !self.scope_epoch.active(),
            "generated scope retains the backend"
        );
        &self.backend
    }

    fn has_unwind_custody_v1(&self) -> bool {
        self.versions.is_some()
            || self.allocation_admission.is_configured()
            || !self.scalar_peer_copies.is_empty()
            || !self.producer_launches.is_empty()
            || !self.same_device_copies.is_empty()
            || !self.segmented_peer_copies.is_empty()
            || self.native_pair_reservation.is_some()
    }

    pub(crate) fn quarantine_after_async_command_panic_v1(&mut self) {
        self.quarantine_submission_writers_v1();
    }

    pub(crate) fn shutdown_owned_backend_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<B::Error>>
    where
        B: RuntimeOwnedShutdownBackendV1,
    {
        assert!(
            !self.scope_epoch.active(),
            "generated scope retains backend shutdown"
        );
        let result = self.invoke_journal_backend_v1(|backend| backend.shutdown_owned_v1());
        if matches!(&result, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.quarantine_after_async_command_panic_v1();
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn backend_mut_for_test_v1(&mut self) -> &mut B {
        &mut self.backend
    }

    #[cfg(test)]
    pub(crate) fn backend_submission_for_test_v1<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<u64, RuntimeValidationErrorV1> {
        Ok(self.submission_record(submission)?.backend_submission)
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
        if self.terminal
            || self.scope_epoch.active()
            || self.graph_reservation.is_some()
            || self.native_pair_reservation.is_some()
            || self.has_unpublished_holds_v1()
            || self.pending_replicas_v1() != 0
        {
            return self.cleanup_report(failures);
        }

        let mut stream_ids: Vec<_> = self.streams.keys().copied().collect();
        stream_ids.sort_unstable();
        let mut streams_quiescent = true;
        for id in stream_ids {
            let record = self.streams[&id];
            match self.invoke_journal_backend_v1(|backend| {
                backend.destroy_stream_v1(record.backend_stream)
            }) {
                Ok(()) => {
                    if self.mark_stream_quiescent(id).is_err() {
                        return self.cleanup_report(failures);
                    }
                    self.streams.remove(&id);
                    self.backend_streams.remove(&record.backend_stream);
                }
                Err(failure) => {
                    if matches!(failure, RuntimeBackendFailureV1::Rejected(_)) {
                        streams_quiescent = false;
                    }
                    if matches!(failure, RuntimeBackendFailureV1::Quiescent(_)) {
                        let _ = self.mark_stream_quiescent(id);
                    }
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Stream(id),
                        failure,
                    });
                    if terminal || self.terminal {
                        self.quarantine_after_async_command_panic_v1();
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
            match self
                .invoke_journal_backend_v1(|backend| backend.release_event_v1(record.backend_event))
            {
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
                        self.quarantine_submission_writers_v1();
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
            if record.dependency_retains != 0 {
                submissions_released = false;
                continue;
            }
            if self.check_operation_custody_v1(id).is_err() {
                return self.cleanup_report(failures);
            }
            match self.invoke_journal_backend_v1(|backend| {
                backend.release_submission_v1(record.backend_submission)
            }) {
                Ok(()) => {
                    self.submissions.remove(&id);
                    self.scalar_peer_copies.remove(&id);
                    self.producer_launches.remove(&id);
                    self.same_device_copies.remove(&id);
                    self.segmented_peer_copies.remove(&id);
                    self.backend_submissions.remove(&record.backend_submission);
                    debug_assert!(!self.completion_callbacks.contains_key(&id));
                }
                Err(failure) => {
                    submissions_released = false;
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Submission(id),
                        failure,
                    });
                    if terminal {
                        self.quarantine_submission_writers_v1();
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
            match self.invoke_journal_backend_v1(|backend| {
                backend.unload_module_v1(record.backend_module)
            }) {
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
                        self.quarantine_submission_writers_v1();
                        return self.cleanup_report(failures);
                    }
                }
            }
        }

        let mut allocation_ids: Vec<_> = self.allocations.keys().copied().collect();
        allocation_ids.sort_unstable();
        for id in allocation_ids {
            let record = self.allocations[&id];
            let journal = match self.prepare_journal_disposal_v1(id, &record) {
                Ok(plan) => plan,
                Err(
                    RuntimeValidationErrorV1::ContextReserved
                    | RuntimeValidationErrorV1::Unsupported,
                ) => continue,
                Err(_) => return self.cleanup_report(failures),
            };
            match self.release_admitted_allocation_backend_v1(id, record.backend_allocation) {
                Ok(()) => {
                    self.finish_allocation_disposal_v1(id, record, journal);
                }
                Err(failure) => {
                    let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                    failures.push(RuntimeCleanupFailureV1 {
                        resource: RuntimeCleanupResourceV1::Allocation(id),
                        failure,
                    });
                    if terminal {
                        self.quarantine_submission_writers_v1();
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
    #[allow(
        clippy::result_large_err,
        reason = "the original cleanup report stays inline in the existing owner-return contract"
    )]
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
            graph_reserved: self.graph_reservation.is_some(),
            native_pair_reserved: self.native_pair_reservation.is_some(),
            scope_reserved: self.scope_epoch.active(),
            replica_pending: self.pending_replicas_v1(),
            allocation_credit_records: self.allocation_admission.retained_records(),
            allocation_journal_records: u32::try_from(
                self.versions
                    .as_ref()
                    .map_or(0, ContextVersionsV1::retained_records),
            )
            .expect("bounded allocation journal"),
            writer_journal_records: self
                .versions
                .as_ref()
                .map_or(0, ContextVersionsV1::retained_writers),
            reader_journal_records: self
                .versions
                .as_ref()
                .map_or(0, ContextVersionsV1::retained_readers),
            scalar_peer_copy_records: u32::try_from(self.scalar_peer_copies.len())
                .expect("bounded scalar-peer-copy registry"),
            producer_launch_records: u32::try_from(self.producer_launches.len())
                .expect("bounded producer-launch registry"),
            same_device_copy_records: u32::try_from(self.same_device_copies.len())
                .expect("bounded same-device-copy registry"),
            segmented_peer_copy_records: u32::try_from(self.segmented_peer_copies.len())
                .expect("bounded segmented-peer-copy registry"),
        }
    }

    fn next_id(&mut self) -> Result<u64, RuntimeValidationErrorV1> {
        let identity = self.next_identity;
        self.next_identity = identity
            .checked_add(1)
            .ok_or(RuntimeValidationErrorV1::Capacity)?;
        Ok(identity)
    }

    fn mark_stream_quiescent(
        &mut self,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let mut submissions: Vec<_> = self
            .submissions
            .iter()
            .filter_map(|(id, record)| (record.stream == stream).then_some(*id))
            .collect();
        submissions.sort_unstable();
        for submission in submissions {
            self.transition_submission_status(
                submission,
                RuntimeCompletionStatusV1::QuiescentWithoutResult,
            )?;
        }
        Ok(())
    }

    fn transition_submission_status(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        status: RuntimeCompletionStatusV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        let outcome = if status == RuntimeCompletionStatusV1::Succeeded {
            SubmissionWriterOutcomeV1::Success
        } else {
            SubmissionWriterOutcomeV1::Unknown
        };
        self.settle_terminal_submission_v1(submission, status, outcome)
    }

    #[allow(clippy::question_mark)] // Explicit matches are shared with Verus.
    fn settle_terminal_submission_v1(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        status: RuntimeCompletionStatusV1,
        outcome: SubmissionWriterOutcomeV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        self.require_ordinary_submission_v1(submission)?;
        let Some(record) = self.submissions.get(&submission) else {
            return Ok(status);
        };
        if record.status.is_terminal() || !status.is_terminal() {
            return Ok(record.status);
        }
        if record.producer_launch {
            let result = self.validate_pending_producer_launch_roots_v1(submission);
            self.journal_result_v1(result)?;
        }
        if self.submissions[&submission].same_device_copy {
            let result = self.validate_pending_same_device_copy_roots_v1(submission);
            self.journal_result_v1(result)?;
        }
        if self.submissions[&submission].segmented_peer_copy {
            let result = self.validate_pending_segmented_peer_roots_v1(submission);
            self.journal_result_v1(result)?;
        }
        self.check_operation_custody_v1(submission)?;
        if status == RuntimeCompletionStatusV1::Succeeded {
            self.require_directed_success_v1(submission)?;
        }
        completion_settlement_execution_body!(
            completion_settlement_rust_expr,
            self,
            submission,
            status,
            outcome
        )
    }

    // Generated callers must settle their receipt-bound writer separately.
    fn publish_submission_status_v1(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        status: RuntimeCompletionStatusV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        let record = self
            .submissions
            .get(&submission)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
        if record.status.is_terminal() || !status.is_terminal() {
            return Ok(record.status);
        }
        let record = self
            .submissions
            .get_mut(&submission)
            .expect("retained observed submission");
        record.status = status;
        record.quiescent = true;
        let callbacks = self
            .completion_callbacks
            .remove(&submission)
            .unwrap_or_default();
        self.completion_callback_count = self
            .completion_callback_count
            .checked_sub(callbacks.len())
            .expect("callback count tracks retained callbacks");
        for callback in callbacks {
            if completion_callback_panicked_v1(callback, status) {
                self.completion_callback_panic_count =
                    self.completion_callback_panic_count.saturating_add(1);
            }
        }
        Ok(status)
    }

    fn observe_submission_backend(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        observation: BackendPollV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        if self.retain_directed_observation_v1(submission, observation)?
            && observation == BackendPollV1::Succeeded
        {
            return self.reconcile_directed_success_v1(submission);
        }
        let status = match observation {
            BackendPollV1::Pending => RuntimeCompletionStatusV1::Pending,
            BackendPollV1::Succeeded => RuntimeCompletionStatusV1::Succeeded,
            BackendPollV1::Failed { code } => {
                RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(code))
            }
        };
        self.transition_submission_status(submission, status)
    }

    fn completion_backend_result(
        &mut self,
        submission: RuntimeSubmissionIdV1,
        result: Result<BackendPollV1, RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>> {
        match result {
            Ok(observation) => self
                .observe_submission_backend(submission, observation)
                .map_err(Into::into),
            Err(RuntimeBackendFailureV1::Rejected(error)) => {
                Err(RuntimeErrorV1::BackendRejected(error))
            }
            Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                if self
                    .transition_submission_status(
                        submission,
                        RuntimeCompletionStatusV1::QuiescentWithoutResult,
                    )
                    .is_err()
                {
                    self.quarantine_after_async_command_panic_v1();
                }
                Err(RuntimeErrorV1::BackendQuiescent(error))
            }
            Err(RuntimeBackendFailureV1::Terminal(error)) => {
                self.quarantine_after_async_command_panic_v1();
                Err(RuntimeErrorV1::BackendTerminal(error))
            }
        }
    }

    fn stream_observation(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<RuntimeStreamObservationV1, RuntimeValidationErrorV1> {
        if !self.streams.contains_key(&stream) {
            return Err(RuntimeValidationErrorV1::UnknownStream);
        }
        let mut observation = RuntimeStreamObservationV1::default();
        let mut first_failure = None;
        for (id, record) in &self.submissions {
            if record.stream != stream {
                continue;
            }
            observation.total_submissions += 1;
            match record.status {
                RuntimeCompletionStatusV1::Pending => observation.pending += 1,
                RuntimeCompletionStatusV1::Succeeded => observation.succeeded += 1,
                RuntimeCompletionStatusV1::Failed(failure) => {
                    observation.failed += 1;
                    if first_failure.is_none_or(|(first_id, _)| *id < first_id) {
                        first_failure = Some((*id, failure));
                    }
                }
                RuntimeCompletionStatusV1::QuiescentWithoutResult => {
                    observation.quiescent_without_result += 1;
                }
            }
        }
        observation.first_failure = first_failure.map(|(_, failure)| failure);
        Ok(observation)
    }

    fn require_live(&self) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(None)
    }

    fn require_graph_access(
        &self,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self.terminal {
            Err(RuntimeValidationErrorV1::ContextTerminal)
        } else if self.scope_epoch.require_access().is_err()
            || self.graph_reservation != access
            || self.native_pair_reservation.is_some()
        {
            Err(RuntimeValidationErrorV1::ContextReserved)
        } else {
            Ok(())
        }
    }

    fn submission_record<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<SubmissionRecordV1, RuntimeValidationErrorV1> {
        cached_submission_record_body_v1!(cached_poll_rust_expr, self, submission)
    }

    fn live_submission_record<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<SubmissionRecordV1, RuntimeValidationErrorV1> {
        cached_live_submission_body_v1!(cached_poll_rust_expr, self, submission)
    }

    fn require_ordinary_submission_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self.submissions.get(&id).is_some_and(|record| {
            self.generated_issues
                .get(&record.stream)
                .is_some_and(|attempt| attempt.owns_submission_v1(id))
        }) {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        Ok(())
    }

    fn require_retained_submission_unheld_v1(
        &self,
        record: &SubmissionRecordV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        // A quiescent submission may legitimately outlive its destroyed stream.
        if self
            .streams
            .get(&record.stream)
            .is_some_and(|stream| stream.unpublished.is_some())
        {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        Ok(())
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
                self.quarantine_after_async_command_panic_v1();
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
            self.quarantine_after_async_command_panic_v1();
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
        let result =
            self.invoke_journal_backend_v1(|backend| backend.create_stream_v1(backend_device));
        let backend_stream = self.backend_result(result)?;
        let protocol_error = self
            .backend_handle_protocol_error(RuntimeBackendResourceKindV1::Stream, backend_stream);
        self.streams.insert(
            id,
            StreamRecordV1 {
                backend_stream,
                device,
                unpublished: None,
                generated: None,
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
        let record = *self.unheld_stream_v1(stream)?;
        let result = self
            .invoke_journal_backend_v1(|backend| backend.destroy_stream_v1(record.backend_stream));
        if matches!(&result, Err(RuntimeBackendFailureV1::Quiescent(_))) {
            let _ = self.mark_stream_quiescent(stream);
        }
        self.backend_result(result)?;
        self.mark_stream_quiescent(stream)?;
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
        self.preflight_journal_capacity_v1(1)?;
        let id = RuntimeAllocationIdV1::new(self.context_generation, self.next_id()?);
        if self
            .allocation_admission
            .prepare_registry(device)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?
            || self.versions.is_some()
        {
            self.allocations
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            self.backend_allocations
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        }
        let enrollment = self.enroll_journal_allocation_v1(id, device, byte_len)?;
        let credit_result = self.guard_journal_unwind_v1(|context| {
            context.allocation_admission.reserve(device, byte_len)
        });
        let credits = match credit_result {
            Ok(credits) => credits,
            Err(error) => {
                self.dispose_journal_provisional_v1(enrollment);
                if error == crate::RuntimeResourceCreditErrorV1::Invariant {
                    self.quarantine_submission_writers_v1();
                    return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
                }
                return Err(RuntimeValidationErrorV1::Capacity.into());
            }
        };
        let allocate = |backend: &mut B, admission: &ContextAllocationAdmissionV1| {
            let witness = admission.witness(device, credits.as_ref(), byte_len)?;
            Ok::<_, RuntimeValidationErrorV1>(match witness {
                Some(witness) => backend.allocate_with_request_v1(
                    backend_device,
                    kind,
                    byte_len,
                    alignment,
                    witness,
                ),
                None => RuntimeRequestAllocationResultV1::Outcome(
                    backend.allocate_with_outcome_v1(backend_device, kind, byte_len, alignment),
                ),
            })
        };
        let result = if self.has_unwind_custody_v1() {
            match catch_unwind(AssertUnwindSafe(|| {
                allocate(&mut self.backend, &self.allocation_admission)
            })) {
                Ok(result) => result,
                Err(payload) => {
                    self.quarantine_submission_writers_v1();
                    drop(credits);
                    std::panic::resume_unwind(payload);
                }
            }
        } else {
            allocate(&mut self.backend, &self.allocation_admission)
        };
        let result = match result {
            Err(error) => {
                self.quarantine_submission_writers_v1();
                drop(credits);
                return Err(error.into());
            }
            Ok(RuntimeRequestAllocationResultV1::Unsupported) => {
                if let Some(credits) = credits
                    && let Err(error) = credits.release_after_rejection()
                {
                    self.quarantine_submission_writers_v1();
                    // fe2o3-hygiene: allow-panic (#182): quarantine precedes unwind; contradictory credit custody must not permit provisional-journal disposal.
                    panic!("allocation credit invariant after unsupported witness: {error:?}");
                }
                self.dispose_journal_provisional_v1(enrollment);
                return Err(RuntimeValidationErrorV1::Unsupported.into());
            }
            Ok(RuntimeRequestAllocationResultV1::Outcome(result)) => result,
        };
        let backend_allocation = match result {
            Ok(RuntimeBackendAllocationOutcomeV1::Allocated(handle)) => handle,
            Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error)) => {
                if let Some(credits) = credits
                    && let Err(invariant) = credits.release_after_disposal()
                {
                    self.quarantine_submission_writers_v1();
                    // fe2o3-hygiene: allow-panic (#182): settled storage does not justify refunding contradictory credit custody; quarantine survives unwind.
                    panic!(
                        "allocation credit owner invariant failed after settlement: {invariant:?}"
                    );
                }
                self.dispose_journal_provisional_v1(enrollment);
                return Err(RuntimeErrorV1::BackendQuiescent(error));
            }
            Err(failure) => {
                if let Some(credits) = credits {
                    if matches!(&failure, RuntimeBackendFailureV1::Rejected(_)) {
                        if let Err(error) = credits.release_after_rejection() {
                            self.quarantine_submission_writers_v1();
                            // fe2o3-hygiene: allow-panic (#182): rejection cannot release a contradictory credit owner; quarantined roots survive unwind.
                            panic!(
                                "allocation credit owner invariant failed after rejection: {error:?}"
                            );
                        }
                    } else {
                        credits.quarantine();
                    }
                }
                if matches!(&failure, RuntimeBackendFailureV1::Rejected(_)) {
                    self.dispose_journal_provisional_v1(enrollment);
                }
                return self.backend_result(Err(failure));
            }
        };
        let journal = enrollment
            .as_ref()
            .map(versions::AllocationEnrollmentV1::reference);
        let protocol_error = self.backend_handle_protocol_error(
            RuntimeBackendResourceKindV1::Allocation,
            backend_allocation,
        );
        self.allocations.insert(
            id,
            AllocationRecordV1 {
                backend_allocation,
                device,
                kind,
                byte_len,
                journal,
            },
        );
        if protocol_error.is_none() {
            self.backend_allocations.insert(backend_allocation);
        }
        // Root the returned handle before any credit or journal invariant can
        // unwind. A partial metadata commit must retain it in a sealed Context.
        let guarded = self.has_unwind_custody_v1();
        let commit = |context: &mut Self| {
            context.allocation_admission.attach(id, credits);
            context.commit_journal_allocation_v1(enrollment);
        };
        if guarded {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| commit(self))) {
                self.quarantine_submission_writers_v1();
                std::panic::resume_unwind(payload);
            }
        } else {
            commit(self);
        }
        self.seal_backend_protocol(protocol_error, id)
    }

    /// Releases only the named allocation owner after backend confirmation.
    ///
    /// In the opt-in journal profile, an Unknown async writer may cover several
    /// allocations. Each successful release makes its handle unusable, but the
    /// complete writer's journal slots and request credits remain retained until
    /// every original destination is disposed. Cleanup can finish the remainder.
    /// A successful release does not imply that backend pool residency is zero.
    pub fn release_allocation(
        &mut self,
        allocation: RuntimeAllocationIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = *self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        let journal = self.prepare_journal_disposal_v1(allocation, &record)?;
        let result =
            self.release_admitted_allocation_backend_v1(allocation, record.backend_allocation);
        self.backend_result(result)?;
        self.finish_allocation_disposal_v1(allocation, record, journal);
        Ok(())
    }

    /// Replaces one complete HostVisible allocation using ordinary host-write
    /// journal and backend settlement. Rejects DeviceLocal placement and partial
    /// images before backend effects; this never performs a DeviceLocal upload.
    /// A backend failure retains the same NoEffect/Unknown distinction as
    /// `write_allocation`. The caller retains the allocation handle on every path.
    pub fn write_host_visible_allocation_v1(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        bytes: &[u8],
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.write_host_visible_with_graph_access_v1(allocation, bytes, None)
    }

    fn write_host_visible_with_graph_access_v1(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        bytes: &[u8],
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_graph_access(access)?;
        let record = self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        if record.kind != RuntimeMemoryKindV1::HostVisible {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        if u64::try_from(bytes.len()).ok() != Some(record.byte_len) {
            return Err(RuntimeValidationErrorV1::InvalidRange.into());
        }
        self.write_allocation_with_graph_access_v1(allocation, 0, bytes, access)
    }

    pub fn write_allocation(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.write_allocation_with_graph_access_v1(allocation, byte_offset, bytes, None)
    }

    fn write_allocation_with_graph_access_v1(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        byte_offset: u64,
        bytes: &[u8],
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_graph_access(access)?;
        let record = *self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        validate_byte_range(record.byte_len, byte_offset, bytes.len())?;
        if let Some(ticket) = self.begin_journal_host_write_v1(allocation, &record)? {
            return self.write_with_journal_v1(ticket, &record, byte_offset, bytes);
        }
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.write_allocation_v1(record.backend_allocation, byte_offset, bytes)
        });
        self.backend_result(result)
    }

    /// Reads into caller-owned storage. A failure or panic after reading begins
    /// may leave partial data in `destination`; those bytes are not a successful read.
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
        self.validate_journal_unqueued_v1(allocation, &record)?;
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.read_allocation_v1(record.backend_allocation, byte_offset, destination)
        });
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
        let result =
            self.invoke_journal_backend_v1(|backend| backend.load_module_v1(backend_device, image));
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
        if self.producer_launch_retains_module_v1(module) {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        let record = *self
            .modules
            .get(&module)
            .ok_or(RuntimeValidationErrorV1::UnknownModule)?;
        let result = self
            .invoke_journal_backend_v1(|backend| backend.unload_module_v1(record.backend_module));
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
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.resolve_kernel_v1(record.backend_module, name, A::SIGNATURE_V1)
        });
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
        self.launch_with_backend_submit(
            ContextLaunchRequestV1 {
                stream,
                kernel,
                arguments: ContextLaunchArgumentsV1::Live(arguments),
                geometry,
                dependencies,
                semantic_launch: BackendSemanticLaunchV1::Ordinary,
            },
            |backend, launch| backend.submit_v1(launch),
        )
    }

    pub(crate) fn launch_snapshot_v1<A: RuntimeArgumentsV1>(
        &mut self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        bytes: &[u8],
        bindings: &[RuntimeBindingV1],
        geometry: RuntimeLaunchGeometryV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<B::Error>> {
        self.launch_with_backend_submit(
            ContextLaunchRequestV1 {
                stream,
                kernel,
                arguments: ContextLaunchArgumentsV1::Frozen(bytes, bindings),
                geometry,
                dependencies,
                semantic_launch: BackendSemanticLaunchV1::Ordinary,
            },
            |backend, launch| backend.submit_v1(launch),
        )
    }

    fn launch_with_backend_submit<A, M, F>(
        &mut self,
        request: ContextLaunchRequestV1<'_, A>,
        submit: F,
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<B::Error>>
    where
        A: RuntimeArgumentsV1,
        F: for<'launch> FnOnce(
            &mut B,
            BackendLaunchV1<'launch>,
        ) -> Result<u64, RuntimeBackendFailureV1<B::Error>>,
    {
        let prepared = self.prepare_context_launch_v1(request, None)?;
        self.submit_prepared_launch_v1(prepared, None, submit)
    }

    fn prepare_context_launch_v1<A: RuntimeArgumentsV1>(
        &self,
        request: ContextLaunchRequestV1<'_, A>,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<PreparedContextLaunchV1, RuntimeErrorV1<B::Error>> {
        self.prepare_context_launch_profile_v1(request, access, false)
    }

    fn prepare_context_launch_profile_v1<A: RuntimeArgumentsV1>(
        &self,
        request: ContextLaunchRequestV1<'_, A>,
        access: Option<ContextGraphReservationV1>,
        producer_aware: bool,
    ) -> Result<PreparedContextLaunchV1, RuntimeErrorV1<B::Error>> {
        let ContextLaunchRequestV1 {
            stream,
            kernel,
            arguments,
            geometry,
            dependencies,
            semantic_launch,
        } = request;
        self.require_graph_access(access)?;
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
        let stream_record = *self.unheld_stream_v1(stream)?;
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
        let explicit_kernarg = match &arguments {
            ContextLaunchArgumentsV1::Live(arguments) => arguments.encode_explicit_kernarg_v1(),
            ContextLaunchArgumentsV1::Frozen(bytes, _) => bytes.to_vec(),
        };
        if explicit_kernarg.len() > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(RuntimeValidationErrorV1::KernargTooLarge.into());
        }
        let bindings = match arguments {
            ContextLaunchArgumentsV1::Live(arguments) => arguments.bindings_v1(),
            ContextLaunchArgumentsV1::Frozen(_, bindings) => bindings.to_vec(),
        };
        if bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
            return Err(RuntimeValidationErrorV1::TooManyBindings.into());
        }
        let mut backend_bindings = Vec::with_capacity(bindings.len());
        let mut producer_bindings = Vec::new();
        if producer_aware {
            producer_bindings
                .try_reserve_exact(bindings.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        }
        let read_bindings = bindings
            .iter()
            .filter(|binding| binding.region.access == RuntimeAccessV1::Read)
            .count();
        let mut journal_sources = Vec::new();
        journal_sources
            .try_reserve_exact(read_bindings)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let mut journal_destinations = Vec::new();
        if self.versions.is_some() {
            journal_destinations
                .try_reserve_exact(bindings.len() - read_bindings)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        }
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
            if producer_aware {
                producer_bindings.push(ContextReadSourceV1 {
                    region,
                    record: allocation,
                });
            }
            if matches!(
                region.access,
                RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
            ) {
                if self.versions.is_some() {
                    journal_destinations.push(region.allocation);
                }
            } else {
                journal_sources.push(ContextReadSourceV1 {
                    region: RuntimeMemoryRegionV1 {
                        allocation: region.allocation,
                        access: RuntimeAccessV1::Read,
                        byte_offset: 0,
                        byte_len: allocation.byte_len,
                    },
                    record: allocation,
                });
            }
        }
        journal_destinations.sort_unstable();
        journal_destinations.dedup();
        journal_sources.sort_unstable_by_key(|source| source.region.allocation);
        journal_sources.dedup_by_key(|source| source.region.allocation);
        // Journaled writable aliases already have exclusive whole-allocation custody.
        // Without a journal, preserve every Read identity for prepared-issue validation.
        journal_sources.retain(|source| {
            journal_destinations
                .binary_search(&source.region.allocation)
                .is_err()
        });
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
        Ok(PreparedContextLaunchV1 {
            stream,
            stream_record,
            kernel: kernel.backend_kernel,
            explicit_kernarg,
            backend_bindings,
            journal_destinations,
            journal_sources,
            producer_bindings,
            backend_dependencies,
            geometry,
            semantic_launch,
        })
    }

    fn submit_prepared_launch_v1<M, F>(
        &mut self,
        prepared: PreparedContextLaunchV1,
        access: Option<ContextGraphReservationV1>,
        submit: F,
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<B::Error>>
    where
        F: for<'launch> FnOnce(
            &mut B,
            BackendLaunchV1<'launch>,
        ) -> Result<u64, RuntimeBackendFailureV1<B::Error>>,
    {
        self.submit_prepared_launch_with_custody_v1(prepared, access, None, submit)
    }

    fn submit_prepared_launch_with_custody_v1<M, F>(
        &mut self,
        prepared: PreparedContextLaunchV1,
        access: Option<ContextGraphReservationV1>,
        custody: Option<PreparedSubmissionCustodyV1>,
        submit: F,
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<B::Error>>
    where
        F: for<'launch> FnOnce(
            &mut B,
            BackendLaunchV1<'launch>,
        ) -> Result<u64, RuntimeBackendFailureV1<B::Error>>,
    {
        self.require_graph_access(access)?;
        self.require_stream_unheld_v1(prepared.stream)?;
        if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        let PreparedContextLaunchV1 {
            stream,
            stream_record,
            kernel,
            explicit_kernarg,
            backend_bindings,
            journal_destinations,
            journal_sources,
            producer_bindings: _,
            backend_dependencies,
            geometry,
            semantic_launch,
        } = prepared;
        self.submit_context_operation_v1(
            stream,
            stream_record,
            &journal_destinations,
            custody,
            &journal_sources,
            |backend| {
                submit(
                    backend,
                    BackendLaunchV1 {
                        stream: stream_record.backend_stream,
                        kernel,
                        explicit_kernarg: &explicit_kernarg,
                        bindings: &backend_bindings,
                        dependencies: &backend_dependencies,
                        geometry,
                        semantic_launch,
                    },
                )
            },
        )
    }

    /// Submits an admitted typed kernel under an explicit atomic contract.
    ///
    /// Both stable and execution-detail atomic capabilities must be advertised,
    /// and the backend must implement the contract-preserving atomic SPI.
    pub fn launch_atomic<A: RuntimeAtomicArgumentsV1>(
        &mut self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        arguments: &A,
        contract: RuntimeAtomicLaunchContractV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimeAtomicLaunchV1<A>>, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAtomicBackendV1,
    {
        self.require_live()?;
        let geometry = contract
            .geometry
            .validate()
            .map_err(|_| RuntimeValidationErrorV1::InvalidAtomicContract)?;
        if contract.operation != A::OPERATION_V1
            || contract.scope != A::SCOPE_V1
            || contract.order != A::ORDER_V1
            || contract.failure_order != A::FAILURE_ORDER_V1
            || contract.weak != A::WEAK_V1
            || !atomic_contract_is_legal(contract)
        {
            return Err(RuntimeValidationErrorV1::InvalidAtomicContract.into());
        }
        let stream_record = *self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        let device = self.device(stream_record.device)?;
        if !device.capabilities.atomics
            || !self
                .backend
                .execution_capabilities_v1(device.backend_device)
                .atomics
        {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        self.launch_with_backend_submit(
            ContextLaunchRequestV1 {
                stream,
                kernel,
                arguments: ContextLaunchArgumentsV1::Live(arguments),
                geometry,
                dependencies,
                semantic_launch: BackendSemanticLaunchV1::Atomic(contract),
            },
            |backend, launch| backend.submit_atomic_v1(launch),
        )
    }

    /// Submits an admitted typed kernel under an explicit collective contract.
    ///
    /// The participant count is checked against geometry before the
    /// contract-preserving collective SPI is entered. System-wide collectives
    /// are rejected because one stream launch cannot establish a cross-device
    /// participant set.
    pub fn launch_collective<A: RuntimeCollectiveArgumentsV1>(
        &mut self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        arguments: &A,
        contract: RuntimeCollectiveLaunchContractV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimeCollectiveLaunchV1<A>>, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeCollectiveBackendV1,
    {
        self.require_live()?;
        let geometry = contract
            .geometry
            .validate()
            .map_err(|_| RuntimeValidationErrorV1::InvalidCollectiveContract)?;
        if !geometry.has_complete_workgroups() {
            return Err(RuntimeValidationErrorV1::InvalidCollectiveContract.into());
        }
        let workgroup_participants = geometry
            .workgroup
            .into_iter()
            .try_fold(1_u64, |product, value| {
                product.checked_mul(u64::from(value))
            })
            .ok_or(RuntimeValidationErrorV1::InvalidCollectiveContract)?;
        let grid_participants = geometry
            .grid
            .into_iter()
            .try_fold(1_u64, |product, value| {
                product.checked_mul(u64::from(value))
            })
            .ok_or(RuntimeValidationErrorV1::InvalidCollectiveContract)?;
        let expected_participants = match contract.scope {
            RuntimeMemoryScopeV1::Workgroup => workgroup_participants,
            RuntimeMemoryScopeV1::Device => grid_participants,
            RuntimeMemoryScopeV1::System => {
                return Err(RuntimeValidationErrorV1::InvalidCollectiveContract.into());
            }
        };
        if contract.operation != A::OPERATION_V1
            || contract.scope != A::SCOPE_V1
            || contract.order != A::ORDER_V1
            || contract.participants == 0
            || contract.participants != expected_participants
        {
            return Err(RuntimeValidationErrorV1::InvalidCollectiveContract.into());
        }
        let stream_record = *self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        let device = self.device(stream_record.device)?;
        if !device.capabilities.collectives
            || !self
                .backend
                .execution_capabilities_v1(device.backend_device)
                .collectives
        {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        self.launch_with_backend_submit(
            ContextLaunchRequestV1 {
                stream,
                kernel,
                arguments: ContextLaunchArgumentsV1::Live(arguments),
                geometry,
                dependencies,
                semantic_launch: BackendSemanticLaunchV1::Collective(contract),
            },
            |backend, launch| backend.submit_collective_v1(launch),
        )
    }

    pub fn poll<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.poll_with_graph_access_v1(submission, None)
    }

    pub(crate) fn poll_with_graph_access_v1<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.require_graph_access(access)?;
        if let Some(status) = self.poll_cached_status_v1(submission)? {
            return Ok(status);
        }
        let status =
            self.observe_completion_step_v1(submission.id, |backend, id| backend.poll_v1(id))?;
        Ok(submission.observe_status(status))
    }

    fn poll_cached_status_v1<A>(
        &self,
        submission: &mut RuntimeSubmissionV1<A>,
    ) -> Result<Option<RuntimePollV1>, RuntimeValidationErrorV1> {
        cached_poll_prefix_body_v1!(cached_poll_rust_expr, self, submission)
    }

    pub fn wait<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
        timeout: Duration,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = self.live_submission_record(submission)?;
        self.require_stream_unheld_v1(record.stream)?;
        if record.status.is_terminal() {
            return Ok(submission.observe_status(record.status));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;
        let status = self.observe_completion_step_v1(submission.id, |backend, id| {
            backend.wait_v1(id, deadline)
        })?;
        Ok(submission.observe_status(status))
    }

    /// Returns the last conclusive submission state without entering the backend.
    pub fn query_submission<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        Ok(self.submission_record(submission)?.status)
    }

    /// Returns an aggregate view of every retained submission on `stream`.
    ///
    /// This is a context-local query and never enters the backend. The first
    /// failure is selected by submission identity, making mixed stream results
    /// deterministic without discarding the aggregate counts.
    pub fn query_stream(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<RuntimeStreamObservationV1, RuntimeValidationErrorV1> {
        self.stream_observation(stream)
    }

    /// Explicitly publishes dependency-ready native work, or drives a bounded
    /// cooperative host operation, in this stream's backend scheduling domain.
    ///
    /// Runtime Worker V1 does not encode this operation; negotiated Runtime
    /// Worker V4 does. It lets batching backends begin device work before a later poll or wait. A
    /// cooperative backend may complete bounded host work in the call; native
    /// completion remains observation-only. The call does not create a progress
    /// thread or otherwise promise background host progress.
    pub fn flush_stream(
        &mut self,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>>
    where
        B: RuntimeFlushBackendV1,
    {
        self.flush_with_graph_access_v1(stream, None)
    }

    /// Makes one backend-defined cooperative progress attempt for a stream.
    ///
    /// Success need not publish all ready work or observe completion. The backend
    /// documents its work bound; the default delegates to full flush and there is
    /// no generic hard time bound. Validation, reservations, journal custody, and
    /// failure handling are identical to `flush_stream`.
    pub fn progress_stream_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>>
    where
        B: RuntimeFlushBackendV1,
    {
        self.drive_stream_with_graph_access_v1(stream, None, B::progress_stream_v1)
    }

    pub(crate) fn flush_with_graph_access_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeErrorV1<B::Error>>
    where
        B: RuntimeFlushBackendV1,
    {
        self.drive_stream_with_graph_access_v1(stream, access, B::flush_stream_v1)
    }

    fn drive_stream_with_graph_access_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        access: Option<ContextGraphReservationV1>,
        drive: impl FnOnce(&mut B, u64) -> Result<(), RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_graph_access(access)?;
        let backend_stream = self.unheld_stream_v1(stream)?.backend_stream;
        let result = self.invoke_journal_backend_v1(|backend| drive(backend, backend_stream));
        self.backend_result(result)
    }

    /// Waits once for every pending submission using one shared deadline.
    ///
    /// A backend may return `Pending` at the deadline, in which case the
    /// returned observation remains non-quiescent. Conclusive observations are
    /// committed through the same centralized transition used by submission
    /// and event waits, including exact-once callback delivery. The first
    /// nonterminal backend error is returned after the remaining pending
    /// submissions are observed; terminal backend ambiguity stops immediately.
    pub fn synchronize_stream(
        &mut self,
        stream: RuntimeStreamIdV1,
        timeout: Duration,
    ) -> Result<RuntimeStreamObservationV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        self.require_stream_unheld_v1(stream)?;
        self.stream_observation(stream)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;
        let pending_count = self
            .submissions
            .values()
            .filter(|record| record.stream == stream && !record.status.is_terminal())
            .count();
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(pending_count)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        pending.extend(self.submissions.iter().filter_map(|(id, record)| {
            (record.stream == stream && !record.status.is_terminal())
                .then_some((*id, record.backend_submission))
        }));
        pending.sort_unstable_by_key(|(id, _)| *id);
        let mut first_error = None;
        for (submission, _) in pending {
            match self
                .observe_completion_step_v1(submission, |backend, id| backend.wait_v1(id, deadline))
            {
                Ok(_) => {}
                Err(error) if self.terminal => return Err(error),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(self.stream_observation(stream)?)
    }

    /// Registers one callback for a submission's first conclusive state.
    ///
    /// Pending callbacks are removed before invocation, so each registered
    /// callback runs exactly once when a conclusive status is first established
    /// across submission and event poll/wait, cancellation, drain, stream
    /// quiescence, cleanup, or release. A callback registered after completion
    /// runs synchronously. Panics are contained to protect resource custody and
    /// counted by [`Self::completion_callback_panic_count`]. Terminal backend
    /// ambiguity is not a completion and therefore does not invoke callbacks.
    pub fn on_completion<A, F>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
        callback: F,
    ) -> Result<(), RuntimeErrorV1<B::Error>>
    where
        F: FnOnce(RuntimeCompletionStatusV1) + Send + UnwindSafe + 'static,
    {
        self.require_live()?;
        let record = self.submission_record(submission)?;
        if record.status.is_terminal() {
            if completion_callback_panicked_v1(callback, record.status) {
                self.completion_callback_panic_count =
                    self.completion_callback_panic_count.saturating_add(1);
            }
            return Ok(());
        }
        if self.completion_callback_count >= MAX_RUNTIME_COMPLETION_CALLBACKS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if let Some(callbacks) = self.completion_callbacks.get_mut(&submission.id) {
            callbacks
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            callbacks.push(Box::new(callback));
        } else {
            let mut callbacks: Vec<RuntimeCompletionCallbackV1> = Vec::new();
            callbacks
                .try_reserve_exact(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            callbacks.push(Box::new(callback));
            self.completion_callbacks
                .try_reserve(1)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            self.completion_callbacks.insert(submission.id, callbacks);
        }
        self.completion_callback_count += 1;
        Ok(())
    }

    /// Returns the number of callback panics contained by this context.
    pub const fn completion_callback_panic_count(&self) -> u64 {
        self.completion_callback_panic_count
    }

    /// Consumes and releases a submission after terminal completion or stream quiescence.
    pub fn release_submission<A>(
        &mut self,
        submission: RuntimeSubmissionV1<A>,
    ) -> Result<(), RuntimeSubmissionReleaseFailureV1<A, B::Error>> {
        match self.release_submission_ref(&submission, None) {
            Ok(()) => Ok(()),
            Err(error) => Err(RuntimeSubmissionReleaseFailureV1 { submission, error }),
        }
    }

    fn release_submission_ref<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_graph_access(access)?;
        let record = self.submission_record(submission)?;
        self.require_retained_submission_unheld_v1(&record)?;
        if !record.quiescent {
            return Err(RuntimeValidationErrorV1::SubmissionPending.into());
        }
        if record.dependency_retains != 0 {
            return Err(RuntimeValidationErrorV1::SubmissionRetainedByDependency.into());
        }
        self.check_operation_custody_v1(submission.id)?;
        if self
            .events
            .values()
            .any(|event| event.submission == submission.id)
        {
            return Err(RuntimeValidationErrorV1::SubmissionRetainedByEvent.into());
        }
        if !record.status.is_terminal() {
            self.transition_submission_status(
                submission.id,
                RuntimeCompletionStatusV1::QuiescentWithoutResult,
            )?;
        }
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.release_submission_v1(record.backend_submission)
        });
        self.backend_result(result)?;
        self.submissions.remove(&submission.id);
        self.scalar_peer_copies.remove(&submission.id);
        self.producer_launches.remove(&submission.id);
        self.same_device_copies.remove(&submission.id);
        self.segmented_peer_copies.remove(&submission.id);
        self.backend_submissions.remove(&record.backend_submission);
        debug_assert!(!self.completion_callbacks.contains_key(&submission.id));
        Ok(())
    }

    pub fn record_event<A>(
        &mut self,
        submission: &RuntimeSubmissionV1<A>,
    ) -> Result<RuntimeEventIdV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let submission_record = self.live_submission_record(submission)?;
        self.require_stream_unheld_v1(submission_record.stream)?;
        if self.events.len() >= MAX_RUNTIME_EVENTS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if !self.device(submission_record.device)?.capabilities.events {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let stream = self.streams[&submission_record.stream];
        let id = RuntimeEventIdV1::new(self.context_generation, self.next_id()?);
        let result = self.invoke_journal_backend_v1(|backend| {
            backend.record_event_v1(stream.backend_stream, submission_record.backend_submission)
        });
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

    /// Queries a recorded event without polling the backend.
    ///
    /// Events alias their source submission's centralized completion state, so
    /// event and submission queries cannot disagree after either observes a
    /// conclusive result.
    pub fn query_event(
        &self,
        event: RuntimeEventIdV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        let event = self
            .events
            .get(&event)
            .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
        self.submissions
            .get(&event.submission)
            .map(|submission| submission.status)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)
    }

    pub(crate) fn event_stream_for_async_progress_v1(
        &self,
        event: RuntimeEventIdV1,
    ) -> Result<RuntimeStreamIdV1, RuntimeValidationErrorV1> {
        let event = self
            .events
            .get(&event)
            .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
        self.submissions
            .get(&event.submission)
            .map(|submission| submission.stream)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)
    }

    /// Performs one nonblocking completion observation for a recorded event.
    pub fn poll_event(
        &mut self,
        event: RuntimeEventIdV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let event = *self
            .events
            .get(&event)
            .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
        let submission = *self
            .submissions
            .get(&event.submission)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
        if submission.status.is_terminal() {
            return Ok(submission.status);
        }
        self.observe_completion_step_v1(event.submission, |backend, id| backend.poll_v1(id))
    }

    /// Waits until an event's source submission completes or `timeout` expires.
    pub fn wait_event(
        &mut self,
        event: RuntimeEventIdV1,
        timeout: Duration,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let event = *self
            .events
            .get(&event)
            .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
        let submission = *self
            .submissions
            .get(&event.submission)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
        if submission.status.is_terminal() {
            return Ok(submission.status);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeValidationErrorV1::InvalidDeadline)?;
        self.observe_completion_step_v1(event.submission, |backend, id| {
            backend.wait_v1(id, deadline)
        })
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
        let result = self
            .invoke_journal_backend_v1(|backend| backend.release_event_v1(record.backend_event));
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
        let prepared =
            self.prepare_context_peer_copy_v1(stream, source, destination, dependencies, true)?;
        let mut custody =
            self.prepare_scalar_peer_custody_v1(stream, source, destination, dependencies)?;
        self.prepare_compute_peer_custody_v1(&mut custody)?;
        self.submit_context_operation_v1(
            stream,
            prepared.stream_record,
            &[destination.allocation],
            Some(PreparedSubmissionCustodyV1::Peer(
                PreparedPeerSubmissionV1 {
                    mechanism: PeerTransferMechanismV1::DeclaredPeerCopy {
                        contract_identity: peer_copy_contract_identity(stream, source, destination),
                    },
                    scalar: Some(custody),
                },
            )),
            &[prepared.journal_source],
            |backend| {
                backend.peer_copy_v1(
                    prepared.stream_record.backend_stream,
                    prepared.source,
                    prepared.destination,
                    &prepared.dependencies,
                )
            },
        )
    }

    fn prepare_context_peer_copy_v1(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
        equal_lengths: bool,
    ) -> Result<peer_segments::PreparedPeerCopyV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies.into());
        }
        let mut ordered = [RuntimeEventIdV1::new(0, 0); MAX_RUNTIME_DEPENDENCIES_V1];
        let ordered = &mut ordered[..dependencies.len()];
        ordered.copy_from_slice(dependencies);
        ordered.sort_unstable();
        if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(RuntimeValidationErrorV1::DuplicateDependency.into());
        }
        let stream_record = *self.unheld_stream_v1(stream)?;
        let journal_source = ContextReadSourceV1 {
            region: source,
            record: *self
                .allocations
                .get(&source.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?,
        };
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
        if stream_record.device != destination_device
            || equal_lengths && source.byte_len != destination.byte_len
        {
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
        Ok(peer_segments::PreparedPeerCopyV1 {
            stream_record,
            journal_source,
            source,
            destination,
            dependencies: backend_dependencies,
        })
    }

    /// Submits a same-device copy without waiting for completion.
    ///
    /// The concrete backend determines whether progress is native or
    /// cooperative. A live direct KFD backend uses persistent native SDMA
    /// storage; its synthetic constructor rejects the extension. The KFD
    /// multi-device router uses native SDMA within one live child and bounded
    /// cooperative host progress otherwise.
    pub fn copy_async(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimeCopyV1>, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        let prepared =
            self.prepare_context_copy_v1(stream, source, destination, dependencies, None)?;
        let ordinary = self.backend.supports_pending_peer_readback_v1();
        let directed = self.backend.supports_pending_directed_peer_readback_v1();
        let custody = if ordinary || directed {
            self.prepare_same_device_copy_custody_v1(
                stream,
                source,
                destination,
                dependencies,
                ordinary,
                directed,
            )?
        } else {
            None
        };
        self.submit_prepared_copy_with_custody_v1(prepared, None, custody)
    }

    fn prepare_context_copy_v1(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
        access: Option<ContextGraphReservationV1>,
    ) -> Result<PreparedContextCopyV1, RuntimeErrorV1<B::Error>> {
        self.require_graph_access(access)?;
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
        let stream_record = *self.unheld_stream_v1(stream)?;
        if source.allocation == destination.allocation {
            return Err(RuntimeValidationErrorV1::InvalidRange.into());
        }
        let journal_destination = destination.allocation;
        let journal_source = ContextReadSourceV1 {
            region: source,
            record: *self
                .allocations
                .get(&source.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?,
        };
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
        if source_device != destination_device || stream_record.device != destination_device {
            return Err(RuntimeValidationErrorV1::WrongDevice.into());
        }
        if source.byte_len != destination.byte_len {
            return Err(RuntimeValidationErrorV1::InvalidRange.into());
        }
        let mut backend_dependencies = Vec::with_capacity(dependencies.len());
        for dependency in dependencies {
            let event = self
                .events
                .get(dependency)
                .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
            if event.device != destination_device {
                return Err(RuntimeValidationErrorV1::WrongDevice.into());
            }
            backend_dependencies.push(event.backend_event);
        }
        Ok(PreparedContextCopyV1 {
            stream,
            stream_record,
            source,
            destination,
            journal_destination,
            journal_source,
            backend_dependencies,
        })
    }

    fn submit_prepared_copy_v1<M>(
        &mut self,
        prepared: PreparedContextCopyV1,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.submit_prepared_copy_with_custody_v1(prepared, access, None)
    }

    fn submit_prepared_copy_with_custody_v1<M>(
        &mut self,
        prepared: PreparedContextCopyV1,
        access: Option<ContextGraphReservationV1>,
        custody: Option<SameDeviceCopyRootV1>,
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.require_graph_access(access)?;
        if access.is_some() && custody.is_some() {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        self.require_stream_unheld_v1(prepared.stream)?;
        if self.submissions.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        let PreparedContextCopyV1 {
            stream,
            stream_record,
            source,
            destination,
            journal_destination,
            journal_source,
            backend_dependencies,
        } = prepared;
        self.submit_context_operation_v1(
            stream,
            stream_record,
            &[journal_destination],
            custody.map(PreparedSubmissionCustodyV1::Copy),
            &[journal_source],
            |backend| {
                backend.copy_async_v1(
                    stream_record.backend_stream,
                    source,
                    destination,
                    &backend_dependencies,
                )
            },
        )
    }

    /// Attempts to withdraw a submission before native publication.
    ///
    /// A `TooLate` result does not release or otherwise weaken submission
    /// custody. The caller must continue polling or drain it to completion.
    pub fn cancel<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
    ) -> Result<RuntimeCancellationV1, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeCancellationBackendV1,
    {
        self.require_live()?;
        let record = self.submission_record(submission)?;
        self.require_retained_submission_unheld_v1(&record)?;
        self.check_operation_custody_v1(submission.id)?;
        if !record.quiescent && record.directed_peer_copy {
            let result = self.validate_pending_peer_copy_roots_v1(submission.id);
            self.journal_result_v1(result)?;
        }
        if !record.quiescent && record.producer_launch {
            let result = self.validate_pending_producer_launch_roots_v1(submission.id);
            self.journal_result_v1(result)?;
        }
        if !record.quiescent && record.same_device_copy {
            let result = self.validate_pending_same_device_copy_roots_v1(submission.id);
            self.journal_result_v1(result)?;
        }
        if !record.quiescent && record.segmented_peer_copy {
            let result = self.validate_pending_segmented_peer_roots_v1(submission.id);
            self.journal_result_v1(result)?;
        }
        if record.quiescent || self.retained_directed_success_v1(submission.id) {
            return Ok(RuntimeCancellationV1::TooLate);
        }
        let result =
            self.invoke_journal_backend_v1(|backend| backend.cancel_v1(record.backend_submission));
        let cancellation = match result {
            Ok(cancellation) => cancellation,
            Err(RuntimeBackendFailureV1::Rejected(error)) => {
                return Err(RuntimeErrorV1::BackendRejected(error));
            }
            Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                if self
                    .transition_submission_status(
                        submission.id,
                        RuntimeCompletionStatusV1::QuiescentWithoutResult,
                    )
                    .is_err()
                {
                    self.quarantine_after_async_command_panic_v1();
                }
                return Err(RuntimeErrorV1::BackendQuiescent(error));
            }
            Err(RuntimeBackendFailureV1::Terminal(error)) => {
                self.quarantine_after_async_command_panic_v1();
                return Err(RuntimeErrorV1::BackendTerminal(error));
            }
        };
        match cancellation {
            BackendCancellationV1::Cancelled => {
                let status = self.settle_terminal_submission_v1(
                    submission.id,
                    RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled),
                    SubmissionWriterOutcomeV1::NoEffect,
                )?;
                submission.observe_status(status);
                Ok(RuntimeCancellationV1::Cancelled)
            }
            BackendCancellationV1::TooLate => Ok(RuntimeCancellationV1::TooLate),
        }
    }

    /// Waits for a submission through a backend's explicit drain path.
    pub fn drain<A>(
        &mut self,
        submission: &mut RuntimeSubmissionV1<A>,
        deadline: Instant,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeCancellationBackendV1,
    {
        self.require_live()?;
        if deadline <= Instant::now() {
            return Err(RuntimeValidationErrorV1::InvalidDeadline.into());
        }
        let record = self.submission_record(submission)?;
        self.require_retained_submission_unheld_v1(&record)?;
        if record.status.is_terminal() {
            return Ok(submission.observe_status(record.status));
        }
        let status = self.observe_completion_step_v1(submission.id, |backend, id| {
            backend.drain_v1(id, deadline)
        })?;
        Ok(submission.observe_status(status))
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

const fn atomic_contract_is_legal(contract: RuntimeAtomicLaunchContractV1) -> bool {
    match (contract.operation, contract.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            valid_compare_exchange_order_pair(contract.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !contract.weak,
        (_, Some(_)) => false,
    }
}

const fn valid_compare_exchange_order_pair(
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

/// Marker identifying a same-device asynchronous copy submission.
pub enum RuntimeCopyV1 {}

/// Typed reason for a conclusively failed submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCompletionFailureV1 {
    /// Backend-defined device or execution failure code.
    BackendCode(i64),
    /// Submission was withdrawn before device-visible publication.
    Cancelled,
}

/// Backend-neutral query state shared by submissions and recorded events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCompletionStatusV1 {
    Pending,
    Succeeded,
    Failed(RuntimeCompletionFailureV1),
    /// Native references are gone, but no execution result was observed.
    QuiescentWithoutResult,
}

/// Aggregate completion state for all submissions retained by one stream.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStreamObservationV1 {
    pub total_submissions: usize,
    pub pending: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub quiescent_without_result: usize,
    pub first_failure: Option<RuntimeCompletionFailureV1>,
}

impl RuntimeStreamObservationV1 {
    pub const fn is_quiescent(self) -> bool {
        self.pending == 0
    }
}

impl RuntimeCompletionStatusV1 {
    pub const fn is_terminal(self) -> bool {
        cached_status_terminal_body_v1!(cached_poll_rust_expr, self)
    }

    fn legacy_poll(self) -> RuntimePollV1 {
        cached_status_legacy_body_v1!(cached_poll_rust_expr, self)
    }
}

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

    fn observe_status(&mut self, status: RuntimeCompletionStatusV1) -> RuntimePollV1 {
        cached_observe_status_body_v1!(cached_poll_rust_expr, self, status)
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
    mod admission_identity;
    mod cleanup_protocol;
    mod completion_events;
    mod launch_copy;
    mod semantic_launch;

    use super::*;
    mod accounted_fail_stop_tests;
    mod allocation_admission_tests;
    mod allocation_outcome_tests;
    mod async_journal_tests;
    mod completion_settlement_tests;
    mod compute_peer_tests;
    mod construction_custody_tests;
    mod copy_custody_tests;
    mod copy_source_lease_tests;
    mod directed_readback_tests;
    mod kernel_read_lease_tests;
    mod native_retained_pair_tests;
    mod peer_batch_tests;
    mod peer_custody_tests;
    mod peer_directed_tests;
    mod peer_gather_tests;
    mod peer_segments_tests;
    mod producer_launch_tests;
    mod progress_stream_tests;
    mod quiescence_order_tests;
    mod replica_tests;
    mod submission_identity_tests;
    mod version_journal_tests;

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

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum MockFlushFailure {
        #[default]
        None,
        RejectOnce,
        Quiescent,
        Terminal,
        Panic,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum MockWaitFailure {
        #[default]
        None,
        RejectFirst,
        QuiescentFirst,
        TerminalFirst,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum MockMemoryFailure {
        #[default]
        None,
        Rejected,
        Quiescent,
        Terminal,
        Panic,
    }

    fn mock_memory_failure_v1(
        failure: MockMemoryFailure,
    ) -> Result<(), RuntimeBackendFailureV1<MockError>> {
        match failure {
            MockMemoryFailure::None => Ok(()),
            MockMemoryFailure::Rejected => Err(RuntimeBackendFailureV1::Rejected(MockError(
                "allocation rejected",
            ))),
            MockMemoryFailure::Quiescent => Err(RuntimeBackendFailureV1::Quiescent(MockError(
                "allocation quiescent failure",
            ))),
            MockMemoryFailure::Terminal => Err(RuntimeBackendFailureV1::Terminal(MockError(
                "allocation terminal failure",
            ))),
            MockMemoryFailure::Panic => panic!("scripted allocation adapter panic"),
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MockPendingKernelReads {
        stream: u64,
        bindings: Vec<BackendBindingV1>,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct MockObservedKernelRead {
        submission: u64,
        ordinal: usize,
        binding: BackendBindingV1,
        bytes: Vec<u8>,
    }

    #[derive(Debug, Default)]
    struct MockBackend {
        accounted_fault: accounted_fail_stop_tests::Fault,
        shutdown_failure: MockMemoryFailure,
        producer_launch: producer_launch_tests::MockProducerLaunchState,
        next: u64,
        enumeration_calls: usize,
        allocation_calls: usize,
        allocation_failure: MockMemoryFailure,
        release_allocation_failure: MockMemoryFailure,
        memory: HashMap<u64, Vec<u8>>,
        peer_placement: HashMap<(u64, u64), Option<BackendPeerCopyPlacementV1>>,
        polls: HashMap<u64, u8>,
        terminal_on_submit: bool,
        last_dependency_count: usize,
        cleanup_failure: MockCleanupFailure,
        cleanup_log: Vec<(MockCleanupKind, u64)>,
        device_name_len: usize,
        device_target_len: usize,
        third_device: bool,
        handle_override: Option<(MockHandleKind, u64)>,
        cancel_before_publication: bool,
        deferred_copies: bool,
        pending_peer_readback: bool,
        pending_directed_peer_readback: bool,
        pending_compute_peer: bool,
        pending_compute_segments: bool,
        peer_segments_frame: bool,
        pending_segment_frame_peer_copy: bool,
        pending_segment_frame_peer_segments: bool,
        ordered_peer_segments: bool,
        ordered_pending_compute_segments: bool,
        ordered_compute_peer: bool,
        pending_copies: HashMap<u64, (u64, BackendMemoryRegionV1, BackendMemoryRegionV1)>,
        pending_peer_segments: HashMap<u64, peer_segments_tests::PendingSegments>,
        deferred_kernel_reads: bool,
        pending_kernel_reads: HashMap<u64, MockPendingKernelReads>,
        observed_kernel_reads: Vec<MockObservedKernelRead>,
        launch_failure: MockMemoryFailure,
        copy_failure: MockMemoryFailure,
        release_submission_failure: MockMemoryFailure,
        copy_call_count: usize,
        write_call_count: usize,
        cancel_failure: MockMemoryFailure,
        execution_capabilities: RuntimeExecutionCapabilitiesV1,
        submit_count: usize,
        poll_call_count: usize,
        wait_call_count: usize,
        cancel_call_count: usize,
        last_waited_submission: Option<u64>,
        last_drained_submission: Option<u64>,
        last_cancelled_submission: Option<u64>,
        last_recorded_event: Option<(u64, u64)>,
        flush_call_count: usize,
        last_flushed_stream: Option<u64>,
        flush_failure: MockFlushFailure,
        last_launch_geometry: Option<RuntimeLaunchGeometryV1>,
        last_atomic_contract: Option<RuntimeAtomicLaunchContractV1>,
        last_collective_contract: Option<RuntimeCollectiveLaunchContractV1>,
        wait_observation: Option<BackendPollV1>,
        first_wait_failure: MockWaitFailure,
        wait_deadlines: Vec<Instant>,
        batch_calls: Vec<(Vec<u64>, Instant)>,
        batch_pending: bool,
        batch_failure: MockMemoryFailure,
        directed_routes: HashMap<
            u64,
            (
                BackendDirectedPeerRouteV1,
                Vec<BackendDirectedPeerDependencyV1>,
            ),
        >,
        directed_calls: Vec<(&'static str, u64)>,
        directed_observations: HashMap<u64, peer_directed_tests::Observation>,
    }

    impl MockBackend {
        fn apply_copy(
            &mut self,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
        ) {
            let start = source.byte_offset as usize;
            let bytes =
                self.memory[&source.allocation][start..start + source.byte_len as usize].to_vec();
            let start = destination.byte_offset as usize;
            self.memory.get_mut(&destination.allocation).unwrap()[start..start + bytes.len()]
                .copy_from_slice(&bytes);
        }

        fn finish_submission(&mut self, submission: u64, success: bool) {
            if self.finish_producer_launch_test_v1(submission, success) {
                return;
            }
            if let Some(pending) = self.pending_peer_segments.remove(&submission) {
                if success {
                    for segment in pending.segments {
                        self.apply_copy(
                            BackendMemoryRegionV1 {
                                byte_offset: pending.source.byte_offset + segment.source_offset,
                                byte_len: segment.byte_len,
                                ..pending.source
                            },
                            BackendMemoryRegionV1 {
                                byte_offset: pending.destination.byte_offset
                                    + segment.destination_offset,
                                byte_len: segment.byte_len,
                                ..pending.destination
                            },
                        );
                    }
                }
                self.record_copy_completion_test_v1(submission, success);
            }
            if let Some((_, source, destination)) = self.pending_copies.remove(&submission) {
                if success {
                    self.apply_copy(source, destination);
                }
                self.record_copy_completion_test_v1(submission, success);
            }
            if let Some(pending) = self.pending_kernel_reads.remove(&submission)
                && success
            {
                for (ordinal, binding) in pending.bindings.into_iter().enumerate() {
                    if binding.region.access == RuntimeAccessV1::Write {
                        continue;
                    }
                    let start = binding.region.byte_offset as usize;
                    let end = start + binding.region.byte_len as usize;
                    let bytes = self.memory[&binding.region.allocation][start..end].to_vec();
                    self.observed_kernel_reads.push(MockObservedKernelRead {
                        submission,
                        ordinal,
                        binding,
                        bytes,
                    });
                }
            }
        }

        fn submit_copy(
            &mut self,
            stream: u64,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
            dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<MockError>> {
            self.copy_call_count += 1;
            self.last_dependency_count = dependencies.len();
            let failure = core::mem::take(&mut self.copy_failure);
            if failure == MockMemoryFailure::Rejected {
                mock_memory_failure_v1(failure)?;
            }
            let identity = self.handle(MockHandleKind::Submission);
            self.polls.insert(identity, 0);
            if self.deferred_copies {
                self.pending_copies
                    .insert(identity, (stream, source, destination));
                if failure == MockMemoryFailure::Quiescent {
                    self.finish_submission(identity, true);
                }
            } else {
                self.apply_copy(source, destination);
                self.record_copy_completion_test_v1(identity, true);
            }
            mock_memory_failure_v1(failure)?;
            Ok(identity)
        }

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

        fn execution_capabilities_v1(&self, _device: u64) -> RuntimeExecutionCapabilitiesV1 {
            self.execution_capabilities
        }

        fn enumerate_devices_v1(
            &mut self,
        ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
            self.enumeration_calls += 1;
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
            let mut devices = vec![
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
            ];
            if self.third_device {
                devices.push(BackendDeviceDescriptionV1 {
                    backend_device: 30,
                    name: "device-2".into(),
                    target: "gfx942".into(),
                    global_memory_bytes: 1 << 30,
                    capabilities,
                });
            }
            Ok(devices)
        }

        fn create_stream_v1(
            &mut self,
            _device: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("create-stream");
            Ok(self.handle(MockHandleKind::Stream))
        }

        fn destroy_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("destroy-stream");
            self.cleanup_log.push((MockCleanupKind::Stream, stream));
            if self.cleanup_failure == MockCleanupFailure::RejectStreamOnce {
                self.cleanup_failure = MockCleanupFailure::None;
                return Err(RuntimeBackendFailureV1::Rejected(MockError("busy")));
            }
            let submissions: Vec<_> = self
                .pending_copies
                .iter()
                .filter_map(|(&id, (owner, _, _))| (*owner == stream).then_some(id))
                .chain(
                    self.pending_kernel_reads
                        .iter()
                        .filter_map(|(&id, pending)| (pending.stream == stream).then_some(id)),
                )
                .collect();
            for id in submissions {
                self.finish_submission(id, true);
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
            self.accounted_fault.enter("allocate");
            self.allocation_calls += 1;
            let failure = core::mem::take(&mut self.allocation_failure);
            if failure == MockMemoryFailure::Rejected {
                mock_memory_failure_v1(failure)?;
            }
            let identity = self.handle(MockHandleKind::Allocation);
            self.memory.insert(identity, vec![0; byte_len as usize]);
            mock_memory_failure_v1(failure)?;
            Ok(identity)
        }

        fn release_allocation_v1(
            &mut self,
            allocation: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("release-allocation");
            self.cleanup_log
                .push((MockCleanupKind::Allocation, allocation));
            let failure = core::mem::take(&mut self.release_allocation_failure);
            mock_memory_failure_v1(failure)?;
            self.memory.remove(&allocation);
            Ok(())
        }

        fn write_allocation_v1(
            &mut self,
            allocation: u64,
            byte_offset: u64,
            bytes: &[u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("write");
            self.write_call_count += 1;
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
            self.accounted_fault.enter("read");
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
            self.accounted_fault.enter("load-module");
            Ok(self.handle(MockHandleKind::Module))
        }

        fn unload_module_v1(
            &mut self,
            module: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("unload-module");
            self.cleanup_log.push((MockCleanupKind::Module, module));
            Ok(())
        }

        fn resolve_kernel_v1(
            &mut self,
            _module: u64,
            _name: &str,
            _signature: [u8; 32],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("resolve-kernel");
            Ok(self.handle(MockHandleKind::Kernel))
        }

        fn submit_v1(
            &mut self,
            launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("submit");
            if self.terminal_on_submit {
                return Err(RuntimeBackendFailureV1::Terminal(MockError("lost")));
            }
            self.submit_count += 1;
            self.last_dependency_count = launch.dependencies.len();
            self.last_launch_geometry = Some(launch.geometry);
            let failure = core::mem::take(&mut self.launch_failure);
            if failure == MockMemoryFailure::Rejected {
                mock_memory_failure_v1(failure)?;
            }
            let identity = self.handle(MockHandleKind::Submission);
            self.polls.insert(identity, 0);
            if self.deferred_kernel_reads {
                self.pending_kernel_reads.insert(
                    identity,
                    MockPendingKernelReads {
                        stream: launch.stream,
                        bindings: launch.bindings.to_vec(),
                    },
                );
                if failure == MockMemoryFailure::Quiescent {
                    self.finish_submission(identity, true);
                }
            }
            mock_memory_failure_v1(failure)?;
            Ok(identity)
        }

        fn poll_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("poll");
            self.poll_call_count += 1;
            if self.is_producer_launch_test_v1(submission) {
                return self.observe_producer_launch_test_v1("poll", submission);
            }
            if self.directed_routes.contains_key(&submission) {
                return self.observe_directed_test_v1("poll", submission);
            }
            if let Some(result) = self.observe_ordinary_copy_fault_test_v1(submission) {
                return result;
            }
            let polls = self.polls.get_mut(&submission).unwrap();
            *polls += 1;
            Ok(if *polls == 1 {
                BackendPollV1::Pending
            } else {
                self.finish_submission(submission, true);
                BackendPollV1::Succeeded
            })
        }

        fn wait_v1(
            &mut self,
            submission: u64,
            deadline: Instant,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("wait");
            self.wait_call_count += 1;
            if self.directed_routes.contains_key(&submission) {
                return self.observe_directed_test_v1("wait", submission);
            }
            self.last_waited_submission = Some(submission);
            self.wait_deadlines.push(deadline);
            if self.is_producer_launch_test_v1(submission) {
                return self.observe_producer_launch_test_v1("wait", submission);
            }
            if self.wait_call_count == 1 {
                match self.first_wait_failure {
                    MockWaitFailure::None => {}
                    MockWaitFailure::RejectFirst => {
                        return Err(RuntimeBackendFailureV1::Rejected(MockError(
                            "wait rejected",
                        )));
                    }
                    MockWaitFailure::QuiescentFirst => {
                        self.finish_submission(submission, false);
                        return Err(RuntimeBackendFailureV1::Quiescent(MockError(
                            "wait quiescent",
                        )));
                    }
                    MockWaitFailure::TerminalFirst => {
                        return Err(RuntimeBackendFailureV1::Terminal(MockError(
                            "wait terminal",
                        )));
                    }
                }
            }
            let observation = self.wait_observation.unwrap_or(BackendPollV1::Succeeded);
            if observation != BackendPollV1::Pending {
                self.finish_submission(submission, observation == BackendPollV1::Succeeded);
            }
            Ok(observation)
        }

        fn release_submission_v1(
            &mut self,
            submission: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("release-submission");
            mock_memory_failure_v1(core::mem::take(&mut self.release_submission_failure))?;
            assert!(!self.pending_copies.contains_key(&submission));
            assert!(!self.pending_peer_segments.contains_key(&submission));
            assert!(!self.pending_kernel_reads.contains_key(&submission));
            self.release_producer_launch_test_v1(submission);
            self.cleanup_log
                .push((MockCleanupKind::Submission, submission));
            self.polls.remove(&submission);
            Ok(())
        }

        fn record_event_v1(
            &mut self,
            stream: u64,
            submission: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("record-event");
            self.last_recorded_event = Some((stream, submission));
            let event = self.handle(MockHandleKind::Event);
            self.record_producer_launch_event_test_v1(event, submission);
            Ok(event)
        }

        fn release_event_v1(
            &mut self,
            event: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("release-event");
            self.cleanup_log.push((MockCleanupKind::Event, event));
            if self.cleanup_failure == MockCleanupFailure::RejectEventOnce {
                self.cleanup_failure = MockCleanupFailure::None;
                return Err(RuntimeBackendFailureV1::Rejected(MockError("event busy")));
            }
            if self.cleanup_failure == MockCleanupFailure::TerminalEvent {
                return Err(RuntimeBackendFailureV1::Terminal(MockError("lost")));
            }
            self.release_producer_launch_event_test_v1(event);
            Ok(())
        }

        fn supports_pending_compute_peer_copy_v1(&self) -> bool {
            self.pending_compute_peer
        }

        fn supports_pending_compute_peer_copy_segments_v1(&self) -> bool {
            self.pending_compute_segments
        }

        fn supports_peer_copy_segments_frame_v1(&self) -> bool {
            self.peer_segments_frame
        }

        fn supports_pending_segment_frame_peer_copy_v1(&self) -> bool {
            self.pending_segment_frame_peer_copy
        }

        fn supports_pending_segment_frame_peer_copy_segments_v1(&self) -> bool {
            self.pending_segment_frame_peer_segments
        }

        fn supports_ordered_peer_copy_segments_v1(&self) -> bool {
            self.ordered_peer_segments
        }

        fn supports_ordered_pending_compute_peer_copy_segments_v1(&self) -> bool {
            self.ordered_pending_compute_segments
        }

        fn supports_ordered_compute_peer_copy_v1(&self) -> bool {
            self.ordered_compute_peer
        }

        fn observe_peer_copy_placement_v1(
            &self,
            _stream: u64,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
        ) -> Option<BackendPeerCopyPlacementV1> {
            self.peer_placement
                .get(&(source.allocation, destination.allocation))
                .copied()
                .unwrap_or(Some(BackendPeerCopyPlacementV1::HostStaged {
                    peak_bytes: source.byte_len,
                }))
        }

        fn peer_copy_v1(
            &mut self,
            stream: u64,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
            dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("peer-copy");
            self.submit_copy(stream, source, destination, dependencies)
        }
    }

    impl RuntimeAsyncCopyBackendV1 for MockBackend {
        fn supports_pending_peer_readback_v1(&self) -> bool {
            self.pending_peer_readback
        }

        fn supports_pending_directed_peer_readback_v1(&self) -> bool {
            self.pending_directed_peer_readback
        }

        fn copy_async_v1(
            &mut self,
            stream: u64,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
            dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("copy");
            self.submit_copy(stream, source, destination, dependencies)
        }
    }

    impl RuntimeAtomicBackendV1 for MockBackend {
        fn submit_atomic_v1(
            &mut self,
            launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let BackendSemanticLaunchV1::Atomic(contract) = launch.semantic_launch else {
                return Err(RuntimeBackendFailureV1::Rejected(MockError("not atomic")));
            };
            self.last_atomic_contract = Some(contract);
            self.submit_v1(launch)
        }
    }

    impl RuntimeCollectiveBackendV1 for MockBackend {
        fn submit_collective_v1(
            &mut self,
            launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let BackendSemanticLaunchV1::Collective(contract) = launch.semantic_launch else {
                return Err(RuntimeBackendFailureV1::Rejected(MockError(
                    "not collective",
                )));
            };
            self.last_collective_contract = Some(contract);
            self.submit_v1(launch)
        }
    }

    impl RuntimeCancellationBackendV1 for MockBackend {
        fn cancel_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("cancel");
            self.cancel_call_count += 1;
            self.last_cancelled_submission = Some(submission);
            let failure = core::mem::take(&mut self.cancel_failure);
            if failure == MockMemoryFailure::Quiescent {
                self.finish_submission(submission, false);
            }
            mock_memory_failure_v1(failure)?;
            if !self.polls.contains_key(&submission) {
                return Err(RuntimeBackendFailureV1::Rejected(MockError(
                    "unknown submission",
                )));
            }
            Ok(if self.cancel_before_publication {
                self.finish_submission(submission, false);
                BackendCancellationV1::Cancelled
            } else {
                BackendCancellationV1::TooLate
            })
        }

        fn drain_v1(
            &mut self,
            submission: u64,
            deadline: Instant,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.last_drained_submission = Some(submission);
            self.wait_v1(submission, deadline)
        }
    }

    impl RuntimeFlushBackendV1 for MockBackend {
        fn flush_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.accounted_fault.enter("flush");
            self.flush_call_count += 1;
            self.last_flushed_stream = Some(stream);
            match self.flush_failure {
                MockFlushFailure::None => Ok(()),
                MockFlushFailure::RejectOnce => {
                    self.flush_failure = MockFlushFailure::None;
                    Err(RuntimeBackendFailureV1::Rejected(MockError(
                        "flush rejected",
                    )))
                }
                MockFlushFailure::Terminal => Err(RuntimeBackendFailureV1::Terminal(MockError(
                    "flush terminal",
                ))),
                MockFlushFailure::Quiescent => Err(RuntimeBackendFailureV1::Quiescent(MockError(
                    "flush quiescent",
                ))),
                MockFlushFailure::Panic => panic!("flush adapter panic"),
            }
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

    impl RuntimeAtomicArgumentsV1 for AddArguments {
        const OPERATION_V1: RuntimeAtomicOperationV1 = RuntimeAtomicOperationV1::Add;
        const SCOPE_V1: RuntimeMemoryScopeV1 = RuntimeMemoryScopeV1::Workgroup;
        const ORDER_V1: RuntimeMemoryOrderV1 = RuntimeMemoryOrderV1::Relaxed;
    }

    impl RuntimeCollectiveArgumentsV1 for AddArguments {
        const OPERATION_V1: RuntimeCollectiveOperationV1 = RuntimeCollectiveOperationV1::ReduceSum;
        const SCOPE_V1: RuntimeMemoryScopeV1 = RuntimeMemoryScopeV1::Workgroup;
        const ORDER_V1: RuntimeMemoryOrderV1 = RuntimeMemoryOrderV1::AcquireRelease;
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
        context_with_launch_prerequisites_using(MockBackend::default())
    }

    fn context_with_launch_prerequisites_using(
        backend: MockBackend,
    ) -> (
        RuntimeContextV1<MockBackend>,
        RuntimeStreamIdV1,
        RuntimeAllocationIdV1,
        TypedRuntimeKernelV1<AddArguments>,
    ) {
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
}
