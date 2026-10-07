//! Pure-Rust KFD implementation of the backend-neutral runtime SPI.
//!
//! The admitted gfx942 KFD surface owns explicit process VMs and native queues.
//! The single-device adapter owns a bounded set of independent compute queues
//! and persistent SDMA queues for host/device and same-device copies. The
//! separate two-device adapter retains exact directional XGMI routes for
//! copy-only peer execution. Atomic and collective execution is fail-closed
//! unless a separate unsafe authority enumerates and authorizes the exact
//! semantic contract carried by each launch.

use core::fmt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::MaybeUninit;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fe2o3_amdhsa_loader::{KernelGlobalBufferAbiV1, ValidatedKernelEnvelope};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_hsaco::{ArgumentAccess, ExplicitValueKind};
use fe2o3_kfd::topology::Gfx942XgmiRouteV1;
use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, ComputeAqlQueueLaneDispatchV1, ComputeAqlQueueLaneV1,
    ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1, DeviceSelector,
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1,
    GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1, GFX942_SDMA_MAX_IN_FLIGHT_V1,
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942CompletedDispatchBatchV1,
    Gfx942CompletedDispatchReadRequestV1, Gfx942CompletedPersistentComputeDispatchV1,
    Gfx942CompletionRecycleObservationV1, Gfx942DeviceBackingBudgetV1,
    Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1, Gfx942DeviceMemoryLeaseV1,
    Gfx942DeviceMemoryUnmappedV1, Gfx942DevicePoolLimitsV1,
    Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1, Gfx942DispatchBatchV1,
    Gfx942DispatchBindingErrorV1, Gfx942DispatchBufferBindingV1, Gfx942DispatchPollV1,
    Gfx942FixedDispatchDataV1, Gfx942FixedDispatchPacketV1, Gfx942FixedDispatchRecycleFailureV1,
    Gfx942FixedDispatchSubmissionFailureV1, Gfx942HostVisibleBackingBudgetV1,
    Gfx942NativeXgmiSdmaQueueCreationRootV1, Gfx942NativeXgmiSdmaQueueV1,
    Gfx942PersistentComputeBindFailureCustodyV1, Gfx942PersistentComputeBindTerminalCustodyV1,
    Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeEffectV1,
    Gfx942PersistentComputeInputV1, Gfx942PersistentComputePollAndRecycleFailureV1,
    Gfx942PersistentComputePollAndRecycleV1, Gfx942PersistentComputeReadyTerminalCustodyV1,
    Gfx942PersistentComputeTerminalCustodyV1, Gfx942PersistentComputeTransitionFailureCustodyV1,
    Gfx942PersistentComputeWaitAndRecycleV1, Gfx942PersistentSdmaDirectionV1,
    Gfx942PreparedPersistentComputeDispatchV1,
    Gfx942PreparedThreeBindingPersistentComputeDispatchV1, Gfx942RecycledDispatchWriteRequestV1,
    Gfx942RecycledPersistentComputeDispatchV1,
    Gfx942RecycledThreeBindingPersistentComputeDispatchV1, Gfx942SdmaBufferV1,
    Gfx942SdmaCopyTicketV1, Gfx942SdmaMemoryPoolObservationV1,
    Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1,
    Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1,
    Gfx942ThreeBindingPersistentComputeCompletedV1, Gfx942ThreeBindingPersistentComputeDispatchV1,
    Gfx942ThreeBindingPersistentComputeInputsV1,
    Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
    Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1, Gfx942XgmiBatchSubmissionFailureV1,
    Gfx942XgmiCopyFailureV1, Gfx942XgmiCopyPollV1, Gfx942XgmiMapRecoveryV1,
    Gfx942XgmiMappedDeviceMemoryV1, Gfx942XgmiSdmaCopyRequestV1, Gfx942XgmiUnmapRecoveryV1,
    HOST_VISIBLE_MEMORY_PAGE_BYTES_V1, OpenedKfd, PrimaryQueueReleaseCustodyV1,
    SharedGttMemorySessionV1,
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
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchProducerV1, BackendLaunchV1,
    BackendMemoryRegionV1, BackendPollV1, BackendProducerAwareLaunchV1, BackendSemanticLaunchV1,
    KfdRuntimeProfileRecorderV1, KfdRuntimeProfileWithSemanticSidecarV1,
    KfdRuntimeProfilerConfigV1, MAX_RUNTIME_DEPENDENCIES_V1, MAX_RUNTIME_EVENTS_V1,
    MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1, MAX_RUNTIME_STREAMS_V1, MAX_RUNTIME_SUBMISSIONS_V1,
    RuntimeAccessV1, RuntimeAsyncCopyBackendV1, RuntimeAtomicBackendV1,
    RuntimeAtomicLaunchContractV1, RuntimeAtomicOperationV1, RuntimeBackendAllocationOutcomeV1,
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCancellationBackendV1, RuntimeCapabilitiesV1,
    RuntimeCollectiveBackendV1, RuntimeCollectiveLaunchContractV1, RuntimeExecutionCapabilitiesV1,
    RuntimeFlushBackendV1, RuntimeMemoryKindV1, RuntimeMemoryOrderV1, RuntimeMemoryScopeV1,
    RuntimeProducerAwareLaunchBackendV1,
};

mod allocation_request;
mod allocation_table;
mod multi_admission;
mod multi_allocation;
mod multi_generated;
pub(crate) use multi_generated::GeneratedAdoptionScopeV1;
mod multi_open;
#[cfg(feature = "hardware-qualification")]
mod multi_qualification;
mod peer_placement;
use allocation_table::AllocationTableV1;
mod compute_dispatch;
mod compute_launch_payload;
mod deferred_compute;
use compute_launch_payload::RetainedComputeLaunchV1;
mod compute_peer_gate;
mod compute_quiescence_control;
mod compute_settlement;
mod materialized_cancellation;
mod materialized_completion;
mod materialized_completion_receipt;
use materialized_completion_receipt::MaterializedCompletionReceiptV1;
mod materialized_publication;
mod materialized_source_event;
mod materialized_submission_attempt;
mod ordered_publication;
mod ordinary_queue_io;
use ordinary_queue_io::OrdinaryQueueIoV1;
mod peer_ancestry;
mod peer_compute_access;
mod peer_readback;
mod persistent_completion;
mod prepared_cancellation;
mod prepared_publication;
mod three_binding_completion;
use peer_ancestry::PeerLaunchAncestryV1;
use peer_compute_access::{
    PeerAccessPurposeV1, PeerComputePermitsV1, PeerCopyAccessV1, PeerCopyLegV1, PeerCopyOriginV1,
};
mod compute_state;
use compute_peer_gate::{
    PeerComputeActionV1, PeerComputeGateV1, PeerComputeResultV1, PeerComputeStepV1,
};
mod cooperative_sdma;
use cooperative_sdma::CooperativeSdmaLeafV1;
mod compute_peer;
mod compute_xgmi;
mod cooperative_directed;
mod native_reconcile;
mod peer_frame;
use native_reconcile::NativeReconciliationV1;
mod scale_capacity;
use scale_capacity::{RuntimeDispatchCapacityV1, RuntimeDispatchStateV1};
#[cfg(feature = "hardware-diagnostic")]
mod directional_wait_diagnostic;
mod drain_capture;
mod xgmi_batch;
mod xgmi_batch_diagnostic;
mod xgmi_budget;
pub use xgmi_budget::{KfdNativeXgmiBackingBudgetV1, KfdNativeXgmiBackingUsageV1};
mod xgmi_directed;
mod xgmi_native_custody;
use xgmi_native_custody::NativeXgmiCustodyV1;
mod xgmi_progress;
mod xgmi_request;
mod xgmi_retained;
mod xgmi_segments;
mod xgmi_segments_diagnostic;
#[cfg(feature = "hardware-diagnostic")]
pub use directional_wait_diagnostic::KfdRuntimeDirectionalWaitObservationV1;
#[cfg(feature = "hardware-diagnostic")]
pub use xgmi_segments_diagnostic::{
    KfdRuntimeXgmiSegmentsObservationV1, KfdRuntimeXgmiSegmentsTimingV1,
};
#[cfg(feature = "hardware-diagnostic")]
mod xgmi_diagnostic;
#[cfg(test)]
pub(crate) use drain_capture::tests::counted as counted_allocations_for_test_v1;
#[cfg(feature = "hardware-diagnostic")]
pub use xgmi_batch_diagnostic::{
    KfdRuntimeXgmiAggregateCallDiagnosticsV1, KfdRuntimeXgmiAggregateCallObservationV1,
    KfdRuntimeXgmiAggregateCurrentnessObservationV1,
};
#[cfg(feature = "hardware-diagnostic")]
pub use xgmi_diagnostic::{KfdRuntimeXgmiCallObservationV1, KfdRuntimeXgmiDiagnosticCallV1};
mod generated_adoption;
pub(crate) use generated_adoption::observe_generated_retirement_v1;
#[cfg(feature = "hardware-qualification")]
pub use generated_adoption::qualification::{
    KfdGeneratedCopyCoexistenceFailureV1, KfdGeneratedCopyCoexistenceWitnessV1,
    KfdGeneratedCopyPublicationKindV1,
};
pub(crate) use generated_adoption::registry4::RegistryStorageV1;
mod generated_preparation;
mod generated_shells;
pub(crate) use generated_shells::{
    GeneratedShellBindingV1, GeneratedShellCommitPlanV1, GeneratedShellPlanV1,
};
pub(crate) use multi_generated::MultiGeneratedShellCommitV1;
mod compute_dependencies;
mod native_budget;
mod producer_peers;
mod progress_quantum;
use producer_peers::PeerLaunchRetainsV1;
mod residency;
use residency::{ResidentKernelImageV1, ResidentModuleImageV1};
#[cfg(test)]
mod producer_launch_tests;
#[cfg(feature = "hardware-qualification")]
mod qualification_coexistence;
#[cfg(feature = "hardware-qualification")]
pub use qualification_coexistence::{
    KfdR66RetainedCustodyObservationFailureV1, KfdR66RetainedCustodyObservationV1,
};
#[cfg(feature = "hardware-qualification")]
mod qualification_drain_capture;
#[cfg(feature = "hardware-qualification")]
pub use qualification_drain_capture::{
    KfdDrainCaptureCopyObservationV1, KfdDrainCaptureCopyPhaseV1,
    KfdDrainCaptureCustodyObservationV1, KfdDrainCaptureObservationFailureV1,
};
mod initialized_storage;
mod kfd_backend_sdma_seam;
mod sdma_allocation;
mod sdma_demotion;
mod sdma_host_read;
mod sdma_host_write;
mod sdma_observation;
mod sdma_promotion;
mod sdma_publication;
use initialized_storage::InitializedStorageOwnerV1;
mod sdma_recycle;
mod sdma_settlement;
mod sdma_synchronous;
use compute_dispatch::*;
use compute_state::*;
#[cfg(test)]
use kfd_backend_sdma_seam::ScriptedSdmaDriverV1;
use kfd_backend_sdma_seam::{
    DirectionalSdmaCompletedOwnerV1, DirectionalSdmaCopyRequestV1, DirectionalSdmaDeviceOwnerV1,
    DirectionalSdmaExecutionFailureV1, DirectionalSdmaPairOwnerV1, DirectionalSdmaPollV1,
    DirectionalSdmaRequestPlanV1, DirectionalSdmaSubmissionOwnerV1,
    DirectionalSdmaSynchronousExecutionFailureV1, DirectionalSdmaWaitV1,
    PersistentComputeReadyOwnerV1, PersistentComputeReadyTransitionFailureV1,
    SameDeviceSdmaCompletedOwnerV1, SameDeviceSdmaCopyRequestV1, SameDeviceSdmaExecutionFailureV1,
    SameDeviceSdmaPairOwnerV1, SameDeviceSdmaPollV1, SameDeviceSdmaSubmissionOwnerV1,
    SameDeviceSdmaWaitV1, SdmaBufferOwnerV1, SdmaRecycleFailureV1, SdmaTransitionFailureV1,
};
use persistent_completion::*;
use three_binding_completion::*;

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
const KFD_RUNTIME_PERSISTENT_H2D_PROVENANCE_ROLE_V1: [u8; 32] = [0x25; 32];
const _: () = assert!(
    GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1
        == GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1
);
const KFD_RUNTIME_MAX_SDMA_WINDOW_PACKETS_V1: usize =
    GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1;

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
/// `preparation` encloses `bound_snapshot` and `authority`. `native_binding`,
/// `publication`, `publish_to_completion`, and the inclusive `recycle` are
/// mutually exclusive portions of the successful launch critical path. For a
/// persistent launch, `recycle` is the sum of `completion_signal_recycle` and
/// `completion_detach_restore`; ordinary launches have no detach/restore phase.
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
    completion_signal_recycle: Duration,
    completion_detach_restore: Duration,
    data_path: KfdRuntimeLaunchDataPathV1,
    user_data_materializations: u64,
    persistent_control_reused: bool,
    ready_promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
}

/// Address-free host observation of one successful authenticated H2D-ready
/// promotion.
///
/// `authentication` is the full ready-promotion interval after H2D completion.
/// It includes affiliation and structural preflight, two operational-currentness
/// checks, constant-time certificate lookup and comparison, and frontier
/// retirement. It remains inside caller-visible H2D duration and does not reread
/// payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeReadyPromotionPerformanceV1 {
    ordinal: u64,
    content_ordinal: u32,
    authenticated_bytes: u64,
    authentication: Duration,
}

impl KfdRuntimeReadyPromotionPerformanceV1 {
    /// Returns the monotonic successful-promotion ordinal for this backend.
    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }

    /// Returns the stable content-role ordinal authenticated by the promotion.
    pub const fn content_ordinal(self) -> u32 {
        self.content_ordinal
    }

    /// Returns the exact nonempty byte extent authenticated by the promotion.
    pub const fn authenticated_bytes(self) -> u64 {
        self.authenticated_bytes
    }

    /// Returns host time spent in the full ready-promotion transition.
    pub const fn authentication(self) -> Duration {
        self.authentication
    }
}

/// Address-free user-data storage path observed for a completed launch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum KfdRuntimeLaunchDataPathV1 {
    /// One or more fixed-dispatch user-data allocations were materialized.
    #[default]
    Materialized,
    /// Existing fixed-dispatch user-data storage was reused.
    ResidentReused,
    /// One persistent SDMA device allocation was rebound in place for compute.
    PersistentDeviceReused,
}

impl KfdRuntimeLaunchPerformanceV1 {
    /// Returns inclusive launch preparation, including snapshot and authority.
    pub const fn preparation(self) -> Duration {
        self.preparation
    }

    /// Returns the nested bound-allocation snapshot interval.
    pub const fn bound_snapshot(self) -> Duration {
        self.bound_snapshot
    }

    /// Returns the nested launch-authority interval.
    pub const fn authority(self) -> Duration {
        self.authority
    }

    /// Returns only native dispatch binding, stopping before publication starts.
    pub const fn native_binding(self) -> Duration {
        self.native_binding
    }

    /// Returns native publication time, excluding native binding.
    pub const fn publication(self) -> Duration {
        self.publication
    }

    pub const fn publish_to_completion(self) -> Duration {
        self.publish_to_completion
    }

    pub const fn completed_readback(self) -> Duration {
        self.completed_readback
    }

    /// Returns the inclusive successful completion-cleanup interval.
    ///
    /// This is the sum of [`Self::completion_signal_recycle`] and
    /// [`Self::completion_detach_restore`].
    pub const fn recycle(self) -> Duration {
        self.completion_signal_recycle
            .saturating_add(self.completion_detach_restore)
    }

    /// Returns time spent recycling the completed dispatch signal.
    pub const fn completion_signal_recycle(self) -> Duration {
        self.completion_signal_recycle
    }

    /// Returns time spent after signal recycle, including the handoff into
    /// persistent-data detach, frontier retirement, and allocation-owner
    /// restoration.
    ///
    /// This is zero for nonpersistent launches.
    pub const fn completion_detach_restore(self) -> Duration {
        self.completion_detach_restore
    }

    /// Returns the address-free user-data storage path used by the launch.
    pub const fn data_path(self) -> KfdRuntimeLaunchDataPathV1 {
        self.data_path
    }

    /// Returns the number of user-data allocations materialized while binding.
    ///
    /// Queue rings, kernarg storage, and other control-plane allocations are not
    /// included in this counter.
    pub const fn user_data_materializations(self) -> u64 {
        self.user_data_materializations
    }

    /// Returns whether the launch replayed the exact retained dispatch control.
    pub const fn persistent_control_reused(self) -> bool {
        self.persistent_control_reused
    }

    /// Returns the exact H2D-ready promotion consumed by the persistent launch.
    pub const fn ready_promotion(self) -> Option<KfdRuntimeReadyPromotionPerformanceV1> {
        self.ready_promotion
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
    WorkerV3GeneratedOnly,
    #[cfg(feature = "hardware-qualification")]
    CopyOnlyQualification,
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942Vecadd(crate::qualification_gfx942_vecadd_v1::AdmittedGfx942VecaddQualificationV1),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942ShardedVecadd(
        crate::qualification_gfx942_sharded_vecadd_v1::AdmittedGfx942ShardedVecaddQualificationV1,
    ),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942ShardedVecaddRounds(
        crate::qualification_gfx942_sharded_vecadd_rounds_v1::AdmittedGfx942ShardedVecaddRoundsQualificationV1,
    ),
    #[cfg(feature = "scale-qualification")]
    ExactGfx942VecaddRepeat(
        crate::qualification_gfx942_vecadd_repeat_v1::AdmittedGfx942VecaddRepeatQualificationV1,
    ),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942R57N3(
        crate::qualification_gfx942_r57_n3_v1::AdmittedGfx942R57N3QualificationV1,
    ),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942R57N3V2(
        crate::qualification_gfx942_r57_n3_v1::AdmittedGfx942R57N3QualificationV2,
    ),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942InplaceTransform(
        crate::qualification_gfx942_inplace_transform_v1::AdmittedGfx942InplaceTransformQualificationV1,
    ),
    #[cfg(feature = "hardware-qualification")]
    ExactGfx942MixedDuration(
        crate::qualification_gfx942_mixed_duration_v1::AdmittedGfx942MixedDurationQualificationV1,
    ),
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
            Self::WorkerV3GeneratedOnly => formatter.write_str("WorkerV3GeneratedOnly"),
            #[cfg(feature = "hardware-qualification")]
            Self::CopyOnlyQualification => formatter.write_str("CopyOnlyQualification"),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942Vecadd(_) => formatter.write_str("ExactGfx942Vecadd"),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942ShardedVecadd(_) => formatter.write_str("ExactGfx942ShardedVecadd"),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942ShardedVecaddRounds(_) => {
                formatter.write_str("ExactGfx942ShardedVecaddRounds")
            }
            #[cfg(feature = "scale-qualification")]
            Self::ExactGfx942VecaddRepeat(_) => formatter.write_str("ExactGfx942VecaddRepeat"),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942R57N3(_) => formatter.write_str("ExactGfx942R57N3"),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942R57N3V2(_) => formatter.write_str("ExactGfx942R57N3V2"),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942InplaceTransform(_) => {
                formatter.write_str("ExactGfx942InplaceTransform")
            }
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942MixedDuration(_) => formatter.write_str("ExactGfx942MixedDuration"),
        }
    }
}

impl KfdRuntimeLaunchGateV1 {
    fn authorize_launch_v1(&self, request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        catch_authority_callback_v1(|| match self {
            Self::Production(authority) => authority.authorize_launch_v1(request),
            Self::Semantic(authority) => authority.authorize_launch_v1(request),
            Self::WorkerV3GeneratedOnly => false,
            #[cfg(feature = "hardware-qualification")]
            Self::CopyOnlyQualification => false,
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942Vecadd(admitted) => admitted.authorizes_kfd_request_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942ShardedVecadd(admitted) => admitted.authorizes_kfd_request_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942ShardedVecaddRounds(admitted) => {
                admitted.authorizes_kfd_request_v1(request)
            }
            #[cfg(feature = "scale-qualification")]
            Self::ExactGfx942VecaddRepeat(admitted) => admitted.authorizes_kfd_request_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942R57N3(admitted) => admitted.authorizes_kfd_request_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942R57N3V2(admitted) => admitted.authorizes_kfd_request_v1(request),
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942InplaceTransform(admitted) => {
                admitted.authorizes_kfd_request_v1(request)
            }
            #[cfg(feature = "hardware-qualification")]
            Self::ExactGfx942MixedDuration(admitted) => admitted.authorizes_kfd_request_v1(request),
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

    fn advertises_generic_compute_v1(&self) -> bool {
        !matches!(self, Self::WorkerV3GeneratedOnly)
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
    persistent_storage_restore: Option<ThreeBindingPersistentRestoreShellV1>,
    #[cfg(test)]
    scripted_three_binding_replay: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KfdRuntimeSdmaInFlightV1 {
    Async(u64),
    Synchronous,
    ComputeXgmi(u64),
}

#[derive(Debug)]
enum KfdRuntimeSdmaStorageV1 {
    Synthetic,
    Host(SdmaBufferOwnerV1),
    Device(Box<DirectionalSdmaDeviceOwnerV1>),
    InitializedStorage(Box<InitializedStorageOwnerV1>),
    PersistentReplay(Box<Gfx942PersistentComputeInputV1>),
    H2dReady(Box<PersistentComputeReadyStorageV1>),
    ComputeInFlight(u64),
    DemotedDevice(SdmaBufferOwnerV1),
    InFlight(KfdRuntimeSdmaInFlightV1),
}

#[derive(Debug)]
struct PersistentComputeReadyStorageV1 {
    owner: PersistentComputeReadyOwnerV1,
    promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
}

enum KfdRuntimePersistentComputeInputV1 {
    Native(Gfx942PersistentComputeInputV1),
    #[cfg(test)]
    ScriptedReady(PersistentComputeReadyStorageV1),
    #[cfg(test)]
    ScriptedReplay(DirectionalSdmaDeviceOwnerV1),
    #[cfg(test)]
    ScriptedStorage(DirectionalSdmaDeviceOwnerV1),
}

type ThreeBindingPersistentInputRosterV1 = (
    [KfdRuntimePersistentComputeInputV1; 3],
    [Option<KfdRuntimeReadyPromotionPerformanceV1>; 3],
);

struct PersistentComputeBindRestoreV1 {
    admission: PersistentFullRangeComputeAdmissionV1,
    submission: u64,
    promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
    // None uses initialized storage's broader, record-owned restoration shell.
    restore_shell: Option<ThreeBindingPersistentRestoreShellV1>,
}

#[derive(Debug)]
struct ThreeBindingPersistentRestoreShellV1 {
    ready: Option<Box<MaybeUninit<PersistentComputeReadyStorageV1>>>,
    device: Option<Box<MaybeUninit<DirectionalSdmaDeviceOwnerV1>>>,
    replay: Option<Box<MaybeUninit<Gfx942PersistentComputeInputV1>>>,
    initialized: Option<Box<MaybeUninit<InitializedStorageOwnerV1>>>,
}

fn try_uninit_box_v1<T>() -> Result<Box<MaybeUninit<T>>, ()> {
    let mut storage = Vec::new();
    storage.try_reserve_exact(1).map_err(|_| ())?;
    storage.push(MaybeUninit::uninit());
    let storage: Box<[MaybeUninit<T>]> = storage.into_boxed_slice();
    let storage: Box<[MaybeUninit<T>; 1]> = storage.try_into().map_err(|_| ())?;
    let raw = Box::into_raw(storage).cast::<MaybeUninit<T>>();
    // A one-element boxed array has the same allocation and pointee layout as
    // its element. Ownership of that allocation is transferred exactly once.
    Ok(unsafe { Box::from_raw(raw) })
}

fn fill_restore_shell_v1<T>(shell: Box<MaybeUninit<T>>, value: T) -> Box<T> {
    Box::write(shell, value)
}

fn take_restore_shell_v1<T>(value: Box<T>) -> (T, Box<MaybeUninit<T>>) {
    let raw = Box::into_raw(value);
    // Safe Box APIs cannot move out T while retaining its allocation. The unique
    // pointer is initialized and aligned; MaybeUninit<T> has T's layout (including
    // ZSTs) but no destructor. No fallible operation occurs between these moves.
    unsafe {
        let value = raw.read();
        let shell = Box::from_raw(raw.cast::<MaybeUninit<T>>());
        (value, shell)
    }
}

impl KfdRuntimeSdmaStorageV1 {
    const fn is_available_for_kind_v1(&self, kind: RuntimeMemoryKindV1) -> bool {
        matches!(
            (self, kind),
            (Self::Host(_), RuntimeMemoryKindV1::HostVisible)
                | (
                    Self::Device(_) | Self::PersistentReplay(_) | Self::InitializedStorage(_),
                    RuntimeMemoryKindV1::DeviceLocal
                )
        )
    }

    fn persistent_compute_ready_facts_v1(&self) -> Option<PersistentComputeReadyFactsV1> {
        let Self::H2dReady(ready) = self else {
            return None;
        };
        Some(PersistentComputeReadyFactsV1 {
            logical_bytes: ready.owner.byte_len(),
            physical_bytes: ready.owner.physical_byte_len(),
            authenticated_sha256: ready.owner.authenticated_sha256(),
        })
    }

    #[cfg(test)]
    fn ready_promotion_performance_v1(&self) -> Option<KfdRuntimeReadyPromotionPerformanceV1> {
        let Self::H2dReady(ready) = self else {
            return None;
        };
        ready.promotion
    }
}

#[derive(Clone, Copy, Debug)]
struct SubmissionRecordV1 {
    stream: u64,
    status: BackendPollV1,
    dependency_depth: usize,
    profile_dispatch_published: bool,
}

#[derive(Clone, Copy, Debug)]
struct EventRecordV1 {
    submission: u64,
}

enum ComputeDependencyRosterV1<'a> {
    Events(&'a [u64]),
    Exact(&'a [BackendLaunchProducerV1]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComputeInputAdmissionV1 {
    Ready,
    ExactProducers,
}

#[derive(Debug)]
struct CollectedComputeDependenciesV1 {
    minimum_dependency_depth: usize,
    ordered_predecessor: Option<u64>,
    explicit_success_dependencies: Box<[u64]>,
    input_admission: ComputeInputAdmissionV1,
    peer_gate: Option<PeerComputeGateV1>,
    peer_access: PeerComputePermitsV1,
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

struct RuntimeAllocationCustodyV1 {
    owners: VecDeque<RuntimeAllocationCustodyOwnerV1>,
    sole_stream: Option<u64>,
    owner_counts: [usize; 2],
    metadata_credits: Option<fe2o3_resource_accounting::RetainedResourceCreditsV1>,
}

#[derive(Debug)]
struct ActiveSdmaCopyV1 {
    peer_access: Option<PeerCopyAccessV1>,
    id: u64,
    stream: u64,
    prior_stream_submission: Option<u64>,
    source: u64,
    destination: u64,
    source_offset: u64,
    destination_offset: u64,
    byte_len: u64,
    completed_bytes: u64,
    window_bytes: u64,
    window_requests: Option<DirectSdmaRequestPlanV1>,
    dependencies: Vec<u64>,
    dependency_cursor: usize,
    dependency_depth: usize,
    phase: ActiveSdmaPhaseV1,
}

#[derive(Debug)]
enum ActiveSdmaPhaseV1 {
    Ready,
    // The exact descriptor stays indexed while native custody is lower-owned.
    // Failure leaves this phase sealed behind the terminal backend gate.
    Quarantined,
    DirectionalPublished(Box<DirectionalSdmaSubmissionOwnerV1>),
    SameDevicePublished(Box<SameDeviceSdmaSubmissionOwnerV1>),
}

#[derive(Clone, Copy)]
struct SdmaStorageBindingV1 {
    id: u64,
    source: u64,
    destination: u64,
}

impl From<&ActiveSdmaCopyV1> for SdmaStorageBindingV1 {
    fn from(active: &ActiveSdmaCopyV1) -> Self {
        Self {
            id: active.id,
            source: active.source,
            destination: active.destination,
        }
    }
}

// Terminal transitions must retain native custody without allocating in the
// failure path, so these move-only owners intentionally remain inline.
#[allow(dead_code, clippy::large_enum_variant)]
enum KfdRuntimeTerminalSdmaCustodyV1 {
    Synchronous(sdma_synchronous::SynchronousSdmaCustodyV1),
    Buffer(SdmaBufferOwnerV1),
    Device(DirectionalSdmaDeviceOwnerV1),
    InitializedStorage(InitializedStorageOwnerV1),
    StoragePromotion(fe2o3_kfd::Gfx942PersistentComputeStoragePromotionTerminalCustodyV1),
    Promotion(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1),
    Demotion(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1),
    SingleSubmission(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
    WindowSubmission(Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1),
    Pending(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
    Retirement {
        failure: Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
        host: Gfx942SdmaBufferV1,
    },
    ComputeRetirement(Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1),
    ReadyPromotion(Gfx942PersistentComputeReadyTerminalCustodyV1),
    PersistentComputeBind(Gfx942PersistentComputeBindTerminalCustodyV1),
    ThreeBindingPersistentComputeBind(Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1),
    PersistentCompute(Gfx942PersistentComputeTerminalCustodyV1),
    PersistentComputePublished(Gfx942PersistentComputeDispatchV1),
    PersistentComputeCompleted(Gfx942CompletedPersistentComputeDispatchV1),
    PersistentComputeRecycled(Gfx942RecycledPersistentComputeDispatchV1),
    ThreeBindingPersistentComputePublished(Gfx942ThreeBindingPersistentComputeDispatchV1),
    ThreeBindingPersistentComputeRecycled(Gfx942RecycledThreeBindingPersistentComputeDispatchV1),
    ThreeBindingPersistentComputeCompleted(Gfx942ThreeBindingPersistentComputeCompletedV1),
    PersistentComputeInput(Gfx942PersistentComputeInputV1),
    PersistentRuntimeInput(KfdRuntimePersistentComputeInputV1),
    ThreeBindingPersistentInputs([KfdRuntimePersistentComputeInputV1; 3]),
    Pair {
        device: DirectionalSdmaDeviceOwnerV1,
        host: SdmaBufferOwnerV1,
    },
    Ready(PersistentComputeReadyOwnerV1),
    ReadyPair {
        ready: PersistentComputeReadyOwnerV1,
        host: SdmaBufferOwnerV1,
    },
    SameDevicePair(SameDeviceSdmaPairOwnerV1),
    SameDevicePending(SameDeviceSdmaSubmissionOwnerV1),
    SameDeviceCompleted(SameDeviceSdmaCompletedOwnerV1),
    SameDevice(kfd_backend_sdma_seam::NativeSameDeviceSdmaTerminalCustodyV1),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectSdmaCopyKindV1 {
    Directional(Gfx942PersistentSdmaDirectionV1),
    SameDevice,
}

fn direct_sdma_copy_kind_v1(
    source: RuntimeMemoryKindV1,
    destination: RuntimeMemoryKindV1,
) -> Option<DirectSdmaCopyKindV1> {
    direct_sdma_direction_v1(source, destination)
        .map(DirectSdmaCopyKindV1::Directional)
        .or_else(|| {
            (source == RuntimeMemoryKindV1::DeviceLocal
                && destination == RuntimeMemoryKindV1::DeviceLocal)
                .then_some(DirectSdmaCopyKindV1::SameDevice)
        })
}

fn directional_sdma_allocation_ids_v1(
    active: SdmaStorageBindingV1,
    direction: Gfx942PersistentSdmaDirectionV1,
) -> (u64, u64) {
    match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => (active.source, active.destination),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => (active.destination, active.source),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DirectSdmaWindowPlanV1 {
    requests: DirectSdmaRequestPlanV1,
    copy_bytes: u64,
}

enum EitherSdmaWindowRequestsV1 {
    Directional(
        Gfx942PersistentSdmaDirectionV1,
        DirectionalSdmaRequestPlanV1,
    ),
    SameDevice(Box<[SameDeviceSdmaCopyRequestV1]>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectSdmaCopyRequestV1 {
    source_offset: u64,
    destination_offset: u64,
    copy_bytes: u32,
}

#[derive(Debug, Eq, PartialEq)]
enum DirectSdmaRequestPlanV1 {
    Single(DirectSdmaCopyRequestV1),
    Window(Box<[DirectSdmaCopyRequestV1]>),
}

impl DirectSdmaRequestPlanV1 {
    fn as_slice(&self) -> &[DirectSdmaCopyRequestV1] {
        match self {
            Self::Single(request) => core::slice::from_ref(request),
            Self::Window(requests) => requests,
        }
    }

    fn first(&self) -> &DirectSdmaCopyRequestV1 {
        match self {
            Self::Single(request) => request,
            Self::Window(requests) => requests
                .first()
                .expect("directional SDMA window is nonempty"),
        }
    }

    fn packet_count(&self) -> usize {
        self.as_slice().len()
    }
}

fn direct_sdma_window_plan_v1(active: &ActiveSdmaCopyV1) -> Option<DirectSdmaWindowPlanV1> {
    direct_sdma_window_plan_with_limit_v1(active, u64::MAX)
}

fn direct_sdma_window_plan_with_limit_v1(
    active: &ActiveSdmaCopyV1,
    byte_limit: u64,
) -> Option<DirectSdmaWindowPlanV1> {
    let mut remaining = active
        .byte_len
        .checked_sub(active.completed_bytes)?
        .min(byte_limit);
    if remaining == 0 {
        return None;
    }
    let mut source_offset = active.source_offset.checked_add(active.completed_bytes)?;
    let mut destination_offset = active
        .destination_offset
        .checked_add(active.completed_bytes)?;
    if remaining <= u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) {
        let copy_bytes = u32::try_from(remaining).ok()?;
        return Some(DirectSdmaWindowPlanV1 {
            requests: DirectSdmaRequestPlanV1::Single(DirectSdmaCopyRequestV1 {
                source_offset,
                destination_offset,
                copy_bytes,
            }),
            copy_bytes: remaining,
        });
    }
    let max_window_packets = u64::try_from(KFD_RUNTIME_MAX_SDMA_WINDOW_PACKETS_V1).ok()?;
    let request_capacity = usize::try_from(
        remaining
            .div_ceil(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
            .min(max_window_packets),
    )
    .ok()?;
    let mut requests = Vec::new();
    requests.try_reserve_exact(request_capacity).ok()?;
    let mut window_bytes = 0_u64;
    while remaining != 0 && requests.len() < KFD_RUNTIME_MAX_SDMA_WINDOW_PACKETS_V1 {
        let copy_bytes =
            u32::try_from(remaining.min(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))).ok()?;
        requests.push(DirectSdmaCopyRequestV1 {
            source_offset,
            destination_offset,
            copy_bytes,
        });
        let copy_bytes = u64::from(copy_bytes);
        source_offset = source_offset.checked_add(copy_bytes)?;
        destination_offset = destination_offset.checked_add(copy_bytes)?;
        remaining -= copy_bytes;
        window_bytes = window_bytes.checked_add(copy_bytes)?;
    }
    Some(DirectSdmaWindowPlanV1 {
        requests: DirectSdmaRequestPlanV1::Window(requests.into_boxed_slice()),
        copy_bytes: window_bytes,
    })
}

fn directional_sdma_requests_v1(
    requests: &DirectSdmaRequestPlanV1,
    direction: Gfx942PersistentSdmaDirectionV1,
) -> Option<DirectionalSdmaRequestPlanV1> {
    let translate = |request: &DirectSdmaCopyRequestV1| {
        let (host_offset, device_offset) = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                (request.source_offset, request.destination_offset)
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                (request.destination_offset, request.source_offset)
            }
        };
        DirectionalSdmaCopyRequestV1 {
            host_offset,
            device_offset,
            copy_bytes: request.copy_bytes,
        }
    };
    match requests {
        DirectSdmaRequestPlanV1::Single(request) => {
            Some(DirectionalSdmaRequestPlanV1::Single(translate(request)))
        }
        DirectSdmaRequestPlanV1::Window(requests) => {
            let mut directional = Vec::new();
            directional.try_reserve_exact(requests.len()).ok()?;
            directional.extend(requests.iter().map(translate));
            Some(DirectionalSdmaRequestPlanV1::Window(
                directional.into_boxed_slice(),
            ))
        }
    }
}

fn same_device_sdma_requests_v1(
    requests: &DirectSdmaRequestPlanV1,
) -> Option<Box<[SameDeviceSdmaCopyRequestV1]>> {
    let requests = requests.as_slice();
    let mut same_device = Vec::new();
    same_device.try_reserve_exact(requests.len()).ok()?;
    same_device.extend(requests.iter().map(|request| SameDeviceSdmaCopyRequestV1 {
        source_offset: request.source_offset,
        destination_offset: request.destination_offset,
        copy_bytes: request.copy_bytes,
    }));
    Some(same_device.into_boxed_slice())
}

#[derive(Clone, Copy, Debug)]
struct StagingBudgetsV1 {
    max_allocation_bytes: u64,
    max_context_bytes: u64,
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
/// per-stream FIFO until its explicit success dependencies complete and one
/// native lane can be leased without reordering overlapping cross-stream work.
/// An ordinary immutable recipe may retain up to 64 ordered physical epochs on
/// that lane. Only the immediate same-stream predecessor supplies ordering;
/// explicit event dependencies remain separately success-gated. Physical
/// completion may be observed out of order, while status, effects, and custody
/// commit only at the contiguous logical stream frontier.
/// Persistent buffers are leased from a queue-owned pool, scrubbed as required
/// before recycle, and the pool is trimmed during explicit shutdown. One narrow
/// ordinary-compute path rebinds an authenticated, full-range H2D destination
/// directly on the primary compute lane; every other launch uses the bounded
/// host-image materialization path. The adapter exposes one gfx942 device and no
/// peer copy or multi-device operations. Atomic and collective profiles remain
/// unavailable unless an unsafe semantic authority explicitly enumerates and
/// authorizes their exact contracts.
#[must_use = "direct KFD backends must remain owned through quiescence"]
pub struct KfdRuntimeBackendV1 {
    description: BackendDeviceDescriptionV1,
    dispatch_capacity: RuntimeDispatchCapacityV1,
    admitted_device: Option<CheckedGfx942XnackMinusDevice>,
    queue: Option<ComputeAqlQueueSessionV1>,
    #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
    cpu_queue: Option<Box<ordinary_queue_io::CpuOrdinaryQueueV1>>,
    primary_teardown: Option<Box<PrimaryQueueReleaseCustodyV1>>,
    terminal_memory: Option<SharedGttMemorySessionV1>,
    terminal_sdma_custody: Option<KfdRuntimeTerminalSdmaCustodyV1>,
    queue_retired: bool,
    terminal: bool,
    next_handle: u64,
    streams: HashMap<u64, u64>,
    allocations: AllocationTableV1,
    generated_shells: HashMap<u64, generated_shells::GeneratedShellRecordV1>,
    generated_submissions: HashMap<u64, u64>,
    modules: HashMap<u64, ModuleRecordV1>,
    kernels: HashMap<u64, KernelRecordV1>,
    host_image_account: Option<fe2o3_resource_accounting::ResourceCreditAccountV1>,
    launch_payload_account: Option<fe2o3_resource_accounting::ResourceCreditAccountV1>,
    submissions: HashMap<u64, SubmissionRecordV1>,
    compute_completion_reservations: usize,
    sdma_completion_reservations: usize,
    pending_compute: HashMap<u64, PendingComputeSubmissionV1>,
    terminal_pending_compute: Option<PendingComputeSubmissionV1>,
    // Monotone: an empty router ledger is only a fast path before the first gate.
    has_admitted_peer_gate: bool,
    pending_compute_streams: HashMap<u64, VecDeque<u64>>,
    allocation_custody: HashMap<u64, RuntimeAllocationCustodyV1>,
    compute_module_retain_counts: HashMap<u64, usize>,
    compute_dependency_retain_counts: HashMap<u64, usize>,
    stream_submission_tails: HashMap<u64, u64>,
    events: HashMap<u64, EventRecordV1>,
    event_submission_retain_counts: HashMap<u64, usize>,
    active: Option<ActiveSubmissionV1>,
    compute_pipeline: RuntimeComputePipelineV1,
    resident_data: Option<ResidentDataRosterV1>,
    recycled_dispatch: Option<RecycledDispatchV1>,
    retained_persistent_dispatch: Option<RetainedPersistentDispatchV1>,
    auxiliary_compute_lanes: Vec<NativeComputeLaneRuntimeV1>,
    native_compute_lanes: Vec<Option<ComputeAqlQueueLaneV1>>,
    stream_compute_lanes: HashMap<u64, usize>,
    selected_compute_lane: usize,
    native_dirty_extents: usize,
    native_reconciliations: [Option<NativeReconciliationV1>; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1],
    #[cfg(test)]
    scripted_native_reconcile: Option<native_reconcile::ScriptedNativeReconcileV1>,
    active_sdma: HashMap<u64, ActiveSdmaCopyV1>,
    published_sdma_submissions: Vec<u64>,
    #[cfg(feature = "hardware-qualification")]
    drain_capture_publications: Option<qualification_drain_capture::PublicationHistoryV1>,
    #[cfg(feature = "hardware-qualification")]
    generated_copy_coexistence: Option<generated_adoption::qualification::RecorderV1>,
    active_sdma_streams: HashMap<u64, VecDeque<u64>>,
    sdma_dependency_retain_counts: HashMap<u64, usize>,
    quiescent_sdma_submissions: HashSet<u64>,
    last_launch_performance: Option<KfdRuntimeLaunchPerformanceV1>,
    #[cfg(feature = "hardware-diagnostic")]
    directional_wait_diagnostic: Option<directional_wait_diagnostic::DirectionalWaitRecorderV1>,
    next_ready_promotion_ordinal: Option<u64>,
    last_ready_promotion_performance: Option<KfdRuntimeReadyPromotionPerformanceV1>,
    staging_budgets: StagingBudgetsV1,
    device_backing_budget: Option<Gfx942DeviceBackingBudgetV1>,
    host_visible_backing_budget: Option<Gfx942HostVisibleBackingBudgetV1>,
    rooted_backing: Option<native_budget::RootedBackingV1>,
    composed_request_binding: Option<crate::RuntimeAllocationDeviceAdmissionV1>,
    host_pool_limits: Option<fe2o3_kfd::Gfx942HostPoolLimitsV1>,
    device_pool_limits: Option<Gfx942DevicePoolLimitsV1>,
    staged_context_bytes: u64,
    sdma_enabled: bool,
    peer_visible_device_allocations: bool,
    native_available: bool,
    launch_gate: KfdRuntimeLaunchGateV1,
    profiler: Option<KfdRuntimeProfileRecorderV1>,
    #[cfg(test)]
    scripted_sdma: Option<ScriptedSdmaDriverV1>,
    #[cfg(test)]
    scripted_persistent_publication_retries: usize,
    #[cfg(test)]
    scripted_persistent_bind_rejections: usize,
    #[cfg(test)]
    scripted_persistent_transition_failure: Option<ScriptedPersistentTransitionFailureV1>,
    #[cfg(test)]
    scripted_three_completion_fault: Option<ScriptedThreeCompletionFaultV1>,
    #[cfg(test)]
    scripted_prepared_cancel_fault: Option<prepared_cancellation::ScriptedPreparedCancelFaultV1>,
    #[cfg(test)]
    scripted_materialized_preparation: Option<(MaterializedPreparationOriginV1, usize)>,
    #[cfg(test)]
    scripted_materialized_publication_fault:
        Option<materialized_publication::ScriptedMaterializedPublicationFaultV1>,
    #[cfg(test)]
    scripted_materialized_cancel_fault:
        Option<materialized_cancellation::ScriptedMaterializedCancelFaultV1>,
    #[cfg(test)]
    scripted_materialized_completion: Option<
        std::collections::VecDeque<(u64, materialized_completion::ScriptedCompletionStepV1)>,
    >,
    #[cfg(test)]
    scripted_ordered_publication:
        Option<VecDeque<(u64, ordered_publication::ScriptedOrderedPublicationV1)>>,
    #[cfg(test)]
    scripted_prepared_publication_fault:
        Option<prepared_publication::ScriptedPreparedPublicationFaultV1>,
    #[cfg(test)]
    scripted_persistent_poll_pending_observations: u64,
    #[cfg(test)]
    scripted_persistent_wait_pending_observations: u64,
    #[cfg(test)]
    scripted_persistent_wait_observations: u64,
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
            .field("primary_teardown", &self.primary_teardown.is_some())
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
            .field("terminal_pending_compute", &self.terminal_pending_compute)
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
            .field("pipelined_compute", &self.compute_pipeline.len())
            .field("active_sdma", &self.active_sdma.len())
            .field("published_sdma", &self.published_sdma_submissions.len())
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
            .field(
                "last_ready_promotion_performance",
                &self.last_ready_promotion_performance,
            )
            .field("staged_context_bytes", &self.staged_context_bytes)
            .field("sdma_enabled", &self.sdma_enabled)
            .field(
                "peer_visible_device_allocations",
                &self.peer_visible_device_allocations,
            )
            .field("staging_budgets", &self.staging_budgets)
            .field("device_backing_budget", &self.device_backing_budget)
            .field(
                "host_visible_backing_budget",
                &self.host_visible_backing_budget,
            )
            .field("device_pool_limits", &self.device_pool_limits)
            .field("host_pool_limits", &self.host_pool_limits)
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

    /// Opens a backend for authenticated Worker V3 generated execution only.
    ///
    /// The returned backend exposes allocation, stream, and copy facilities used
    /// by the protected generated route, but advertises and admits no public
    /// generic, atomic, or collective kernel launch. This constructor does not
    /// create verifier or semantic-machine authority.
    ///
    /// ```no_run
    /// use fe2o3_runtime::{KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1};
    ///
    /// fn open_generated_only(
    ///     device_unique_id: u64,
    /// ) -> Result<KfdRuntimeBackendV1, KfdRuntimeBackendErrorV1> {
    ///     KfdRuntimeBackendV1::open_worker_v3_generated_only_v1(device_unique_id)
    /// }
    /// ```
    pub fn open_worker_v3_generated_only_v1(
        device_unique_id: u64,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly,
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

    #[cfg(feature = "hardware-qualification")]
    /// Opens the exact repository-owned gfx942 DeviceLocal R57 N3 qualification backend.
    ///
    /// The returned observer exposes only the bounded authority-call count used
    /// by the historical V1 lane. The retained gate admits exactly `A+B -> C`
    /// followed by `C+B -> D`. Current zero-initialized DeviceLocal allocation
    /// semantics do not reproduce V1's original prepublication negative case;
    /// use the separately identified V2 lane for new qualification runs.
    pub fn open_gfx942_r57_n3_qualification_v1(
        device_unique_id: u64,
    ) -> Result<
        (
            Self,
            crate::qualification_gfx942_r57_n3_v1::Gfx942R57N3QualificationAuthorityObservationV1,
        ),
        KfdRuntimeBackendErrorV1,
    > {
        let admitted = crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v1(
        )
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                error.to_string(),
            )
        })?;
        let observation = admitted.observation_v1();
        let backend = Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::ExactGfx942R57N3(admitted),
        )?;
        Ok((backend, observation))
    }

    #[cfg(feature = "hardware-qualification")]
    /// Opens the V2 R57 N3 gate with a separately pinned mixed-memory negative.
    /// This grants no production or general kernel authority.
    pub fn open_gfx942_r57_n3_qualification_v2(
        device_unique_id: u64,
    ) -> Result<
        (
            Self,
            crate::qualification_gfx942_r57_n3_v1::Gfx942R57N3QualificationAuthorityObservationV1,
        ),
        KfdRuntimeBackendErrorV1,
    > {
        let admitted = crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v2(
        )
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                error.to_string(),
            )
        })?;
        let observation = admitted.observation_v1();
        let backend = Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::ExactGfx942R57N3V2(admitted),
        )?;
        Ok((backend, observation))
    }

    #[cfg(feature = "hardware-qualification")]
    /// Opens the exact repository-owned gfx942 in-place-transform qualification backend.
    ///
    /// This constructor re-admits and retains one source-authenticated fixture,
    /// then accepts only its fixed ABI, two pinned initial images, whole-buffer
    /// read/write effect, and launch geometry. It grants no production authority.
    pub fn open_gfx942_inplace_transform_qualification_v1(
        device_unique_id: u64,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let admitted = crate::qualification_gfx942_inplace_transform_v1::admit_gfx942_inplace_transform_qualification_v1()
            .map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    error.to_string(),
                )
            })?;
        Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::ExactGfx942InplaceTransform(admitted),
        )
    }

    #[cfg(feature = "hardware-qualification")]
    /// Opens only the two exact fixed-work HostVisible scheduling fixtures.
    ///
    /// This grants no production, generated, atomic or collective authority.
    /// Work bounds, guarded input bytes, ABI, effects and geometry are fixed.
    pub fn open_gfx942_mixed_duration_qualification_v1(
        device_unique_id: u64,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let admitted = crate::qualification_gfx942_mixed_duration_v1::admit_gfx942_mixed_duration_qualification_v1()
            .map_err(|error| KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch, error.to_string()))?;
        Self::open_default_with_gate(
            device_unique_id,
            KfdRuntimeLaunchGateV1::ExactGfx942MixedDuration(admitted),
        )
    }

    fn open_default_with_gate(
        device_unique_id: u64,
        launch_gate: KfdRuntimeLaunchGateV1,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let device = Self::open_checked_device_v1(device_unique_id)?;
        Ok(Self::from_checked_device_with_gate(device, launch_gate))
    }

    fn open_checked_device_v1(
        device_unique_id: u64,
    ) -> Result<CheckedGfx942XnackMinusDevice, KfdRuntimeBackendErrorV1> {
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
        Ok(device)
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

    /// Wraps a checked device for authenticated Worker V3 generated execution only.
    ///
    /// As with [`Self::open_worker_v3_generated_only_v1`], this grants no public
    /// generic launch authority and does not stand in for Worker V3 verification
    /// or semantic-machine refinement.
    pub fn from_checked_device_worker_v3_generated_only_v1(
        device: CheckedGfx942XnackMinusDevice,
    ) -> Self {
        Self::from_checked_device_with_gate(device, KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
    }

    fn from_checked_device_with_gate(
        device: CheckedGfx942XnackMinusDevice,
        launch_gate: KfdRuntimeLaunchGateV1,
    ) -> Self {
        Self::new(Self::describe_device_v1(&device), Some(device), launch_gate)
    }

    fn describe_device_v1(device: &CheckedGfx942XnackMinusDevice) -> BackendDeviceDescriptionV1 {
        let observation = device.observation();
        let unique_id = observation.unique_id();
        let name = device
            .topology_snapshot()
            .topology()
            .gpu_nodes()
            .iter()
            .find(|node| node.unique_id() == unique_id)
            .map_or_else(|| "AMD MI300X".to_owned(), |node| node.name().to_owned());
        BackendDeviceDescriptionV1 {
            backend_device: unique_id,
            name,
            target: "gfx942:xnack-".to_owned(),
            // The admitted topology schema does not currently expose a
            // trustworthy aggregate VRAM capacity.
            global_memory_bytes: 0,
            capabilities: kfd_capabilities_v1(),
        }
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
        Self::new_with_dispatch_state_v1(
            description,
            admitted_device,
            launch_gate,
            staging_budgets,
            RuntimeDispatchStateV1::try_new(RuntimeDispatchCapacityV1::default())
                .expect("default runtime dispatch tables"),
        )
    }

    fn new_with_dispatch_state_v1(
        mut description: BackendDeviceDescriptionV1,
        admitted_device: Option<CheckedGfx942XnackMinusDevice>,
        launch_gate: KfdRuntimeLaunchGateV1,
        staging_budgets: StagingBudgetsV1,
        dispatch: RuntimeDispatchStateV1,
    ) -> Self {
        let native_available = admitted_device.is_some();
        description.capabilities.typed_async_launch &= launch_gate.advertises_generic_compute_v1();
        description.capabilities.atomics = launch_gate.advertises_atomics_v1();
        description.capabilities.collectives = launch_gate.advertises_collectives_v1();
        Self {
            description,
            dispatch_capacity: dispatch.capacity,
            admitted_device,
            queue: None,
            #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
            cpu_queue: None,
            primary_teardown: None,
            terminal_memory: None,
            terminal_sdma_custody: None,
            queue_retired: false,
            terminal: false,
            next_handle: 1,
            streams: HashMap::new(),
            allocations: AllocationTableV1::default(),
            generated_shells: HashMap::new(),
            generated_submissions: HashMap::new(),
            modules: HashMap::new(),
            kernels: HashMap::new(),
            host_image_account: None,
            launch_payload_account: None,
            submissions: HashMap::new(),
            compute_completion_reservations: 0,
            sdma_completion_reservations: 0,
            pending_compute: HashMap::new(),
            terminal_pending_compute: None,
            has_admitted_peer_gate: false,
            pending_compute_streams: HashMap::new(),
            allocation_custody: HashMap::new(),
            compute_module_retain_counts: HashMap::new(),
            compute_dependency_retain_counts: HashMap::new(),
            stream_submission_tails: HashMap::new(),
            events: HashMap::new(),
            event_submission_retain_counts: HashMap::new(),
            active: None,
            compute_pipeline: dispatch.primary,
            resident_data: None,
            recycled_dispatch: None,
            retained_persistent_dispatch: None,
            auxiliary_compute_lanes: dispatch.auxiliary,
            native_compute_lanes: vec![None; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1],
            stream_compute_lanes: HashMap::new(),
            selected_compute_lane: 0,
            native_dirty_extents: 0,
            native_reconciliations: core::array::from_fn(|_| None),
            #[cfg(test)]
            scripted_native_reconcile: None,
            active_sdma: HashMap::new(),
            published_sdma_submissions: Vec::new(),
            #[cfg(feature = "hardware-qualification")]
            drain_capture_publications: None,
            #[cfg(feature = "hardware-qualification")]
            generated_copy_coexistence: None,
            active_sdma_streams: HashMap::new(),
            sdma_dependency_retain_counts: HashMap::new(),
            quiescent_sdma_submissions: HashSet::new(),
            last_launch_performance: None,
            #[cfg(feature = "hardware-diagnostic")]
            directional_wait_diagnostic: None,
            next_ready_promotion_ordinal: Some(0),
            last_ready_promotion_performance: None,
            staging_budgets,
            device_backing_budget: None,
            host_visible_backing_budget: None,
            rooted_backing: None,
            composed_request_binding: None,
            device_pool_limits: None,
            host_pool_limits: None,
            staged_context_bytes: 0,
            sdma_enabled: false,
            peer_visible_device_allocations: false,
            native_available,
            launch_gate,
            profiler: None,
            #[cfg(test)]
            scripted_sdma: None,
            #[cfg(test)]
            scripted_persistent_publication_retries: 0,
            #[cfg(test)]
            scripted_persistent_bind_rejections: 0,
            #[cfg(test)]
            scripted_persistent_transition_failure: None,
            #[cfg(test)]
            scripted_three_completion_fault: None,
            #[cfg(test)]
            scripted_prepared_cancel_fault: None,
            #[cfg(test)]
            scripted_materialized_preparation: None,
            #[cfg(test)]
            scripted_materialized_publication_fault: None,
            #[cfg(test)]
            scripted_materialized_cancel_fault: None,
            #[cfg(test)]
            scripted_materialized_completion: None,
            #[cfg(test)]
            scripted_ordered_publication: None,
            #[cfg(test)]
            scripted_prepared_publication_fault: None,
            #[cfg(test)]
            scripted_persistent_poll_pending_observations: 0,
            #[cfg(test)]
            scripted_persistent_wait_pending_observations: 0,
            #[cfg(test)]
            scripted_persistent_wait_observations: 0,
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

    fn poison_terminal_v1(&mut self) {
        self.terminal = true;
        if let Some(queue) = self.queue.as_mut() {
            queue.poison_after_runtime_owner_failure_v1();
        }
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if let Some(queue) = self.cpu_queue.as_mut() {
            queue.fixture.poison_terminal();
        }
        if let Some(custody) = self.primary_teardown.as_mut() {
            custody.poison_after_runtime_owner_failure_v1();
        }
        self.compute_pipeline.quarantine_all();
        for lane in &mut self.auxiliary_compute_lanes {
            lane.pipeline.quarantine_all();
        }
    }

    fn terminal_error(
        &mut self,
        detail: impl Into<String>,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        self.poison_terminal_v1();
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
        if self.terminal || self.primary_teardown.is_some() {
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
            || self.native_reconciliation_holds_v1(allocation)
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
            let replaced = self.active_sdma_streams.insert(stream, submissions);
            debug_assert!(replaced.is_none());
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
            .and_then(|live| live.checked_add(self.generated_submissions.len()))
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
                if custody.owners.len() >= self.dispatch_capacity.custody_limit() {
                    return Err(Self::capacity(
                        "KFD per-allocation custody owner capacity exceeded",
                    ));
                }
                if !self.dispatch_capacity.is_scaled() {
                    custody.owners.try_reserve(1).map_err(|_| {
                        Self::capacity("KFD allocation-custody owner growth failed")
                    })?;
                }
            } else {
                new_entries.push((
                    allocation,
                    RuntimeAllocationCustodyV1::try_new(&self.dispatch_capacity)
                        .map_err(|error| Self::capacity(error.to_string()))?,
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
            let replaced = self.allocation_custody.insert(allocation, custody);
            debug_assert!(replaced.is_none());
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
        self.release_compute_module_retain_v1(module);
    }

    fn release_compute_module_retain_v1(&mut self, module: u64) {
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
                    matches!(
                        copy.phase,
                        ActiveSdmaPhaseV1::DirectionalPublished(_)
                            | ActiveSdmaPhaseV1::SameDevicePublished(_)
                    )
                })
            },
        )
    }

    fn allocation_retains_exact_owner_v1(
        &self,
        allocation: u64,
        owner: RuntimeAllocationCustodyOwnerV1,
    ) -> bool {
        self.allocation_custody
            .get(&allocation)
            .is_some_and(|custody| custody.owners.contains(&owner))
    }

    fn persistent_compute_sdma_blocker_v1(
        &self,
        pending: &PendingComputeSubmissionV1,
    ) -> Option<u64> {
        let compute_owner = RuntimeAllocationCustodyOwnerV1 {
            submission: pending.id,
            stream: pending.launch.stream,
            kind: RuntimeAllocationCustodyKindV1::Compute,
        };
        let retained_roster_matches = !pending.retained_allocations.is_empty()
            && pending.retained_allocations.iter().all(|allocation| {
                self.allocations.contains_key(allocation)
                    && self.allocation_retains_exact_owner_v1(*allocation, compute_owner)
                    && pending
                        .launch
                        .bindings
                        .iter()
                        .any(|binding| binding.region.allocation == *allocation)
            })
            && pending.launch.bindings.iter().all(|binding| {
                pending
                    .retained_allocations
                    .contains(&binding.region.allocation)
            });
        // This is only a scheduling filter. Native publication independently
        // checks the retained storage identities and complete directional ledger.
        self.published_sdma_submissions
            .iter()
            .copied()
            .find(|submission| {
                let Some(copy) = self.active_sdma.get(submission) else {
                    return true;
                };
                let copy_owner = RuntimeAllocationCustodyOwnerV1 {
                    submission: *submission,
                    stream: copy.stream,
                    kind: RuntimeAllocationCustodyKindV1::Sdma,
                };
                !retained_roster_matches
                    || copy.id != *submission
                    || !matches!(copy.phase, ActiveSdmaPhaseV1::DirectionalPublished(_))
                    || self.direct_sdma_direction_for_active_v1(copy).is_err()
                    || [copy.source, copy.destination]
                        .into_iter()
                        .any(|allocation| {
                            pending.retained_allocations.contains(&allocation)
                                || !self.allocation_retains_exact_owner_v1(allocation, copy_owner)
                                || !self.allocations.get(&allocation).is_some_and(|record| {
                                    matches!(record.sdma_storage,
                                    KfdRuntimeSdmaStorageV1::InFlight(
                                        KfdRuntimeSdmaInFlightV1::Async(actual)
                                    ) if actual == *submission)
                                })
                        })
            })
    }

    fn sdma_can_coexist_with_persistent_compute_v1(&self, copy: &ActiveSdmaCopyV1) -> bool {
        let Some(compute) = self.active.as_ref() else {
            return false;
        };
        if !self.persistent_compute_is_active_v1()
            || !self.compute_pipeline.is_empty()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.active.is_some() || !lane.pipeline.is_empty())
            || !matches!(copy.phase, ActiveSdmaPhaseV1::Ready)
            || self.direct_sdma_direction_for_active_v1(copy).is_err()
            || compute.allocations.is_empty()
        {
            return false;
        }
        let retained_roster_matches = match compute.execution.as_ref() {
            Some(
                ActiveComputeExecutionV1::PersistentPrepared { allocation, .. }
                | ActiveComputeExecutionV1::Persistent { allocation, .. },
            ) => compute.allocations.len() == 1 && compute.allocations.contains(allocation),
            Some(
                ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { admissions, .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistent { admissions, .. },
            ) => {
                compute.allocations.len() == admissions.len()
                    && admissions
                        .iter()
                        .all(|admission| compute.allocations.contains(&admission.allocation))
                    && compute.allocations.iter().all(|allocation| {
                        admissions
                            .iter()
                            .any(|admission| admission.allocation == *allocation)
                    })
            }
            #[cfg(test)]
            Some(
                ActiveComputeExecutionV1::ScriptedPersistent { allocation, .. }
                | ActiveComputeExecutionV1::ScriptedPersistentPrepared { allocation, .. },
            ) => compute.allocations.len() == 1 && compute.allocations.contains(allocation),
            #[cfg(test)]
            Some(
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { admissions, .. }
                | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                    admissions, ..
                },
            ) => {
                compute.allocations.len() == admissions.len()
                    && admissions
                        .iter()
                        .all(|admission| compute.allocations.contains(&admission.allocation))
                    && compute.allocations.iter().all(|allocation| {
                        admissions
                            .iter()
                            .any(|admission| admission.allocation == *allocation)
                    })
            }
            _ => false,
        };
        if !retained_roster_matches {
            return false;
        }
        let compute_owner = RuntimeAllocationCustodyOwnerV1 {
            submission: compute.id,
            stream: compute.stream,
            kind: RuntimeAllocationCustodyKindV1::Compute,
        };
        let copy_owner = RuntimeAllocationCustodyOwnerV1 {
            submission: copy.id,
            stream: copy.stream,
            kind: RuntimeAllocationCustodyKindV1::Sdma,
        };
        compute.allocations.iter().all(|allocation| {
            self.allocation_retains_exact_owner_v1(*allocation, compute_owner)
                && self.allocations.get(allocation).is_some_and(|record| {
                    matches!(record.sdma_storage,
                        KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == compute.id)
                })
        }) && [copy.source, copy.destination]
            .into_iter()
            .all(|allocation| {
                !compute.allocations.contains(&allocation)
                    && self.allocation_retains_exact_owner_v1(allocation, copy_owner)
            })
    }

    fn reserve_published_sdma_index_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let additional =
            GFX942_SDMA_MAX_IN_FLIGHT_V1.saturating_sub(self.published_sdma_submissions.capacity());
        self.published_sdma_submissions
            .try_reserve_exact(additional)
            .map_err(|_| Self::capacity("KFD published-SDMA index allocation failed"))
    }

    fn index_published_sdma_v1(&mut self, submission: u64) {
        let position = self
            .published_sdma_submissions
            .binary_search(&submission)
            .expect_err("published SDMA submission is indexed exactly once");
        debug_assert!(
            self.published_sdma_submissions.len() < self.published_sdma_submissions.capacity(),
            "published SDMA capacity is reserved before acceptance"
        );
        self.published_sdma_submissions.insert(position, submission);
        debug_assert!(self.published_sdma_submissions.len() <= GFX942_SDMA_MAX_IN_FLIGHT_V1);
    }

    #[cfg(feature = "hardware-qualification")]
    fn record_drain_capture_publication_v1(&mut self, submission: u64) {
        if let Some(history) = self.drain_capture_publications.as_mut() {
            history.record(submission);
        }
    }

    fn unindex_published_sdma_v1(&mut self, submission: u64) {
        if let Ok(position) = self.published_sdma_submissions.binary_search(&submission) {
            self.published_sdma_submissions.remove(position);
        }
    }

    #[cfg(test)]
    fn published_sdma_index_is_consistent_v1(&self) -> bool {
        self.published_sdma_submissions
            .windows(2)
            .all(|window| window[0] < window[1])
            && self.published_sdma_submissions.iter().all(|submission| {
                self.active_sdma.get(submission).is_some_and(|active| {
                    matches!(
                        active.phase,
                        ActiveSdmaPhaseV1::DirectionalPublished(_)
                            | ActiveSdmaPhaseV1::SameDevicePublished(_)
                    )
                })
            })
            && self
                .active_sdma
                .iter()
                .filter(|(_, active)| {
                    matches!(
                        active.phase,
                        ActiveSdmaPhaseV1::DirectionalPublished(_)
                            | ActiveSdmaPhaseV1::SameDevicePublished(_)
                    )
                })
                .count()
                == self.published_sdma_submissions.len()
    }

    fn persistent_compute_is_active_v1(&self) -> bool {
        self.active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
            .is_some_and(|execution| match execution {
                ActiveComputeExecutionV1::PersistentPrepared { .. }
                | ActiveComputeExecutionV1::Persistent { .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistent { .. } => true,
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedPersistent { .. }
                | ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. }
                | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { .. }
                | ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. } => true,
                ActiveComputeExecutionV1::PersistentCancelling(_)
                | ActiveComputeExecutionV1::PersistentCompleting(_)
                | ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(_) => true,
                ActiveComputeExecutionV1::MaterializedBinding(_)
                | ActiveComputeExecutionV1::MaterializedSuccessorPublication(_)
                | ActiveComputeExecutionV1::MaterializedPrepared(_)
                | ActiveComputeExecutionV1::MaterializedCancelling(_)
                | ActiveComputeExecutionV1::Materialized(_) => false,
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedMaterialized
                | ActiveComputeExecutionV1::ScriptedMaterializedCompleted
                | ActiveComputeExecutionV1::ScriptedMaterializedRetired => false,
            })
    }

    fn any_compute_active_v1(&self) -> bool {
        self.active.is_some()
            || !self.generated_submissions.is_empty()
            || !self.compute_pipeline.is_empty()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.active.is_some() || !lane.pipeline.is_empty())
    }

    fn active_compute_lane_v1(&self, submission: u64) -> Option<usize> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.id == submission)
        {
            return Some(0);
        }
        if self.compute_pipeline.contains(submission) {
            return Some(0);
        }
        self.auxiliary_compute_lanes
            .iter()
            .position(|lane| {
                lane.active
                    .as_ref()
                    .is_some_and(|active| active.id == submission)
                    || lane.pipeline.contains(submission)
            })
            .map(|index| index + 1)
    }

    fn active_compute_submission_v1(&self, submission: u64) -> Option<&ActiveSubmissionV1> {
        self.active
            .as_ref()
            .filter(|active| active.id == submission)
            .or_else(|| self.compute_pipeline.get(submission))
            .or_else(|| {
                self.auxiliary_compute_lanes
                    .iter()
                    .flat_map(|lane| lane.active.iter().chain(lane.pipeline.iter()))
                    .find(|active| active.id == submission)
            })
    }

    fn published_persistent_compute_lane_v1(&self, submission: u64) -> Option<usize> {
        let active = self.active_compute_submission_v1(submission)?;
        let published = active
            .execution
            .as_ref()
            .is_some_and(|execution| match execution {
                ActiveComputeExecutionV1::Persistent { .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistent { .. } => true,
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedPersistent { .. }
                | ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. } => true,
                _ => false,
            });
        if published {
            self.active_compute_lane_v1(submission)
        } else {
            None
        }
    }

    fn pending_compute_submission_v1(
        &self,
        submission: u64,
    ) -> Option<&PendingComputeSubmissionV1> {
        self.pending_compute.get(&submission)
    }

    fn next_dependency_depth_v1(
        &self,
        _ordered_predecessor: Option<u64>,
        explicit_success_dependencies: &[u64],
    ) -> Result<usize, DirectSdmaDependencyDepthErrorV1> {
        let mut depth = 1_usize;
        for dependency in explicit_success_dependencies.iter().copied() {
            let dependency_depth = self
                .pending_compute
                .get(&dependency)
                .map(|pending| pending.dependency_depth)
                .or_else(|| {
                    self.active_compute_submission_v1(dependency)
                        .map(|active| active.dependency_depth)
                })
                .or_else(|| {
                    self.active_sdma
                        .get(&dependency)
                        .map(|copy| copy.dependency_depth)
                })
                .or_else(|| {
                    self.submissions
                        .get(&dependency)
                        .map(|record| record.dependency_depth)
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
                self.active.is_some() || !self.compute_pipeline.is_empty()
            } else {
                let lane = &self.auxiliary_compute_lanes[*lane - 1];
                lane.active.is_some() || !lane.pipeline.is_empty()
            };
            !active
                && !self.native_reconciliation_pins_lane_v1(*lane)
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
            self.active.is_some() || !self.compute_pipeline.is_empty(),
            self.auxiliary_compute_lanes[0].active.is_some()
                || !self.auxiliary_compute_lanes[0].pipeline.is_empty(),
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
        let released = self.stream_compute_lanes.remove(&stream);
        debug_assert_eq!(released, Some(lane));
        if lane != 0 {
            let released = self.auxiliary_compute_lanes[lane - 1].owner_stream.take();
            debug_assert_eq!(released, Some(stream));
        }
    }

    fn release_compute_dependency_retains_v1(&mut self, dependencies: &[u64]) {
        Self::release_compute_dependency_counts_v1(
            &mut self.compute_dependency_retain_counts,
            dependencies,
        );
    }

    fn release_compute_dependency_counts_v1(
        counts: &mut HashMap<u64, usize>,
        dependencies: &[u64],
    ) {
        for dependency in dependencies {
            let remove = {
                let count = counts
                    .get_mut(dependency)
                    .expect("pending compute dependency remains retained");
                *count = count
                    .checked_sub(1)
                    .expect("positive compute dependency retain count");
                *count == 0
            };
            if remove {
                counts.remove(dependency);
            }
        }
    }

    fn release_pending_compute_dependency_retains_v1(
        &mut self,
        pending: &PendingComputeSubmissionV1,
    ) {
        self.release_compute_dependency_retains_v1(&pending.explicit_success_dependencies);
        self.release_compute_dependency_retains_v1(&pending.quiescence_dependencies);
        if let Some(predecessor) = pending.ordered_predecessor
            && !pending.explicit_success_dependencies.contains(&predecessor)
        {
            self.release_compute_dependency_retains_v1(core::slice::from_ref(&predecessor));
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

    fn restore_unfinished_stream_tail_v1(&mut self, stream: u64, removed: u64) {
        if self.stream_submission_tails.get(&stream) != Some(&removed) {
            return;
        }
        // Admission IDs and the per-stream FIFOs are monotone. Terminal records
        // are observations, not unfinished ordering nodes; resurrecting them can
        // turn a resolved failure into a new success prerequisite.
        let queued = [
            self.pending_compute_streams
                .get(&stream)
                .and_then(|queue| queue.back())
                .copied(),
            self.active_sdma_streams
                .get(&stream)
                .and_then(|queue| queue.back())
                .copied(),
        ];
        let primary = self.active.iter().chain(
            self.compute_pipeline
                .iter()
                .take(self.compute_pipeline.len()),
        );
        let auxiliary = self.auxiliary_compute_lanes.iter().flat_map(|lane| {
            lane.active
                .iter()
                .chain(lane.pipeline.iter().take(lane.pipeline.len()))
        });
        let prior = queued
            .into_iter()
            .flatten()
            .chain(
                primary
                    .chain(auxiliary)
                    .filter(|active| active.stream == stream && active.id != removed)
                    .map(|active| active.id),
            )
            .max();
        match prior {
            Some(prior) => {
                *self
                    .stream_submission_tails
                    .get_mut(&stream)
                    .expect("selected stream tail remains indexed") = prior;
            }
            None => {
                self.stream_submission_tails.remove(&stream);
            }
        }
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
        self.require_unpinned_native_lane_v1(lane)?;
        self.with_compute_lane_state_v1(lane, |backend| {
            backend.detach_recycled_dispatch()?;
            backend.release_resident_data()
        })
    }

    fn release_all_compute_caches_for_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self
            .retained_persistent_dispatch
            .is_some_and(|retained| retained.allocation == allocation)
        {
            self.release_retained_persistent_control_v1()?;
        }
        for lane in 0..self.native_compute_lanes.len() {
            if self.compute_lane_caches_allocation_v1(lane, allocation) {
                self.release_compute_lane_cache_v1(lane)?;
            }
        }
        Ok(())
    }

    fn can_retain_host_visible_write_cache_v1(&self, allocation: u64, full_write: bool) -> bool {
        full_write
            && self.allocations.get(&allocation).is_some_and(|record| {
                record.kind == RuntimeMemoryKindV1::HostVisible && !record.bytes.is_empty()
            })
            && !self.any_compute_active_v1()
            && self.retained_persistent_dispatch.is_none()
    }

    fn prepare_compute_caches_for_host_write_v1(
        &mut self,
        allocation: u64,
        full_write: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self.can_retain_host_visible_write_cache_v1(allocation, full_write) {
            return self.release_all_compute_caches_for_allocation_v1(allocation);
        }
        for lane in 0..self.native_compute_lanes.len() {
            if !self.compute_lane_caches_allocation_v1(lane, allocation) {
                continue;
            }
            self.with_compute_lane_state_v1(lane, |backend| {
                if backend.can_retain_host_visible_recycled_control_v1() {
                    let native_lane = backend.selected_native_compute_lane_v1()?;
                    let validation = backend
                        .queue
                        .as_mut()
                        .ok_or_else(|| "KFD recycled dispatch has no native queue".to_owned())
                        .and_then(|queue| {
                            queue
                                .with_compute_lane_v1(native_lane, |queue| {
                                    queue.recycled_fixed_dispatch_generation()
                                })
                                .map_err(|error| format!("KFD compute-lane selection: {error}"))?
                                .map_err(|error| format!("KFD retained control preflight: {error}"))
                        });
                    validation.map_err(|detail| backend.terminal_error(detail))?;
                    backend.synchronize_recycled_dispatch_data_v1()?;
                    // Native bytes and control stay owned together. Fresh launch
                    // admission still precedes any generation-checked overwrite.
                    return Ok(());
                }
                backend.detach_recycled_dispatch()?;
                let retain_data = backend.resident_data.as_ref().is_some_and(|resident| {
                    host_visible_resident_roster_is_reusable_v1(
                        &resident.descriptors,
                        resident.data.len(),
                    )
                });
                if retain_data {
                    // These descriptors still describe native bytes. The next
                    // checked overwrite/rebind, not this host write, refreshes them.
                    Ok(())
                } else {
                    backend.release_resident_data()
                }
            })?;
        }
        Ok(())
    }

    fn can_retain_host_visible_recycled_control_v1(&self) -> bool {
        self.resident_data.is_none()
            && self.recycled_dispatch.as_ref().is_some_and(|recycled| {
                host_visible_resident_descriptors_are_reusable_v1(&recycled.descriptors)
            })
    }

    fn release_retained_persistent_control_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.retained_persistent_dispatch.is_none() {
            return Ok(());
        }
        self.release_primary_detached_persistent_control_v1(
            "backend retained persistent identity without detached queue control",
        )?;
        self.retained_persistent_dispatch = None;
        Ok(())
    }

    fn release_primary_detached_persistent_control_v1(
        &mut self,
        missing_detail: &'static str,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            match self.scripted_three_completion_fault {
                Some(ScriptedThreeCompletionFaultV1::ControlMissing) => {
                    self.scripted_three_completion_fault = None;
                    return Err(self.terminal_error(missing_detail));
                }
                Some(ScriptedThreeCompletionFaultV1::ControlFailure) => {
                    self.scripted_three_completion_fault = None;
                    return Err(self.terminal_error("scripted detached-control release failure"));
                }
                Some(ScriptedThreeCompletionFaultV1::ControlUnwind) => {
                    self.scripted_three_completion_fault = None;
                    panic!("scripted detached-control release unwind");
                }
                _ => {}
            }
            return Ok(());
        }
        let primary = self
            .native_compute_lanes
            .first()
            .copied()
            .flatten()
            .ok_or_else(|| {
                self.terminal_error("retained persistent control lost its primary compute lane")
            })?;
        let released = self
            .queue
            .as_mut()
            .ok_or_else(|| "retained persistent control lost its queue".to_owned())
            .and_then(|queue| {
                queue
                    .with_compute_lane_v1(primary, |lane| {
                        lane.release_retained_persistent_fixed_dispatch_control_v1()
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
                    .map_err(|error| format!("KFD persistent-control release: {error}"))
            });
        match released {
            Ok(true) => Ok(()),
            Ok(false) => Err(self.terminal_error(missing_detail)),
            Err(detail) => Err(self.terminal_error(detail)),
        }
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
        core::mem::swap(&mut self.compute_pipeline, &mut auxiliary.pipeline);
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
        core::mem::swap(&mut self.compute_pipeline, &mut auxiliary.pipeline);
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

    fn retain_primary_compute_lane_v1(&mut self) {
        if self.native_compute_lanes[0].is_some() {
            return;
        }
        let primary_lane = self
            .queue
            .as_ref()
            .expect("persistent-compute attachment retains its queue")
            .primary_compute_lane_v1();
        self.native_compute_lanes[0] = Some(primary_lane);
        let queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1,
        );
        self.observe_profile_v1(
            queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
        );
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
        if self.sdma_allocation_ready_v1() {
            return Ok(());
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
            let admission = self.take_rooted_backing_v1()?;
            let device = self.admitted_device.take().ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "the admitted KFD queue lifecycle has already retired",
                )
            })?;
            let queue = match admission {
                Some(native_budget::BackingAdmissionV1::Host(admission)) => device
                    .create_compute_aql_queue_with_rooted_host_backing_v1(
                        KFD_RUNTIME_RING_BYTES_V1,
                        self.device_backing_budget,
                        admission,
                        self.dispatch_capacity.native().clone(),
                    ),
                Some(native_budget::BackingAdmissionV1::Native(admission)) => device
                    .create_compute_aql_queue_with_rooted_native_backing_v1(
                        KFD_RUNTIME_RING_BYTES_V1,
                        admission,
                        self.dispatch_capacity.native().clone(),
                    ),
                Some(native_budget::BackingAdmissionV1::Composed(admission)) => device
                    .create_compute_aql_queue_with_composed_backing_v1(
                        KFD_RUNTIME_RING_BYTES_V1,
                        admission,
                        self.dispatch_capacity.native().clone(),
                    ),
                None => device.create_compute_aql_queue_with_backing_budgets_and_capacity_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    self.device_backing_budget,
                    self.host_visible_backing_budget,
                    self.dispatch_capacity.native().clone(),
                ),
            }
            .map_err(|error| self.terminal_error(format!("KFD queue creation: {error}")))?;
            self.queue = Some(queue);
            self.configure_native_device_pool_v1()?;
            self.configure_native_host_pool_v1()?;
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
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::SingleSubmission(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::WindowSubmission(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::Published(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::Pending(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::Retirement {
                    failure,
                    host,
                } => KfdRuntimeTerminalSdmaCustodyV1::Retirement { failure, host },
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::ReadyPromotion(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::ReadyPromotion(custody),
                kfd_backend_sdma_seam::NativeDirectionalSdmaTerminalCustodyV1::StoragePromotion(
                    custody,
                ) => KfdRuntimeTerminalSdmaCustodyV1::StoragePromotion(custody),
            },
            kfd_backend_sdma_seam::SdmaTerminalCustodyV1::NativeSameDevice(custody) => {
                KfdRuntimeTerminalSdmaCustodyV1::SameDevice(custody)
            }
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

    fn direct_sdma_copy_kind_for_active_v1(
        &self,
        active: &ActiveSdmaCopyV1,
    ) -> Result<DirectSdmaCopyKindV1, &'static str> {
        let source = self
            .allocations
            .get(&active.source)
            .ok_or("SDMA source allocation disappeared")?;
        let destination = self
            .allocations
            .get(&active.destination)
            .ok_or("SDMA destination allocation disappeared")?;
        direct_sdma_copy_kind_v1(source.kind, destination.kind)
            .ok_or("unsupported SDMA copy kind reached publication")
    }

    fn take_directional_sdma_storage_v1(
        &mut self,
        active: SdmaStorageBindingV1,
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
        active: SdmaStorageBindingV1,
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

    fn take_same_device_sdma_storage_v1(
        &mut self,
        active: SdmaStorageBindingV1,
        owner: KfdRuntimeSdmaInFlightV1,
    ) -> Result<SameDeviceSdmaPairOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if active.source == active.destination {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "same-device persistent SDMA requires distinct allocation identities",
            ));
        }
        let source_ready = self.allocations.get(&active.source).is_some_and(|record| {
            record
                .sdma_storage
                .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
        });
        let destination_ready = self
            .allocations
            .get(&active.destination)
            .is_some_and(|record| {
                record
                    .sdma_storage
                    .is_available_for_kind_v1(RuntimeMemoryKindV1::DeviceLocal)
            });
        if !source_ready || !destination_ready {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "same-device persistent SDMA storage is retained by pending work",
            ));
        }
        let source = match std::mem::replace(
            &mut self
                .allocations
                .get_mut(&active.source)
                .expect("preflighted same-device source remains indexed")
                .sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(owner),
        ) {
            KfdRuntimeSdmaStorageV1::Device(source) => *source,
            _ => unreachable!("preflighted same-device source remains available"),
        };
        let destination = match std::mem::replace(
            &mut self
                .allocations
                .get_mut(&active.destination)
                .expect("preflighted same-device destination remains indexed")
                .sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(owner),
        ) {
            KfdRuntimeSdmaStorageV1::Device(destination) => *destination,
            _ => unreachable!("preflighted same-device destination remains available"),
        };
        Ok(SameDeviceSdmaPairOwnerV1 {
            source,
            destination,
        })
    }

    fn restore_same_device_sdma_storage_v1(
        &mut self,
        active: SdmaStorageBindingV1,
        owner: KfdRuntimeSdmaInFlightV1,
        pair: SameDeviceSdmaPairOwnerV1,
        destination_dirty: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let source_slot_matches = self.allocations.get(&active.source).is_some_and(|record| {
            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(actual) if actual == owner)
        });
        let destination_slot_matches = self
            .allocations
            .get(&active.destination)
            .is_some_and(|record| {
                matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(actual) if actual == owner)
            });
        if !source_slot_matches || !destination_slot_matches {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::SameDevicePair(
                pair,
            ));
            return Err(self.terminal_error(
                "same-device persistent SDMA restoration slot changed unexpectedly",
            ));
        }
        self.allocations
            .get_mut(&active.source)
            .expect("same-device source remains indexed")
            .sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(pair.source));
        self.allocations
            .get_mut(&active.destination)
            .expect("same-device destination remains indexed")
            .sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(pair.destination));
        if destination_dirty {
            let destination = self
                .allocations
                .get_mut(&active.destination)
                .expect("active same-device destination remains indexed");
            destination.sdma_shadow_dirty = true;
            destination.content_sha256 = None;
            destination.last_full_host_write = None;
        }
        Ok(())
    }

    fn full_h2d_ready_provenance_v1(
        &self,
        active: &ActiveSdmaCopyV1,
        direction: Gfx942PersistentSdmaDirectionV1,
    ) -> Option<(Gfx942DeviceContentDescriptorV1, Arc<[u8]>, [u8; 32])> {
        if direction != Gfx942PersistentSdmaDirectionV1::HostToDevice
            || active.completed_bytes != 0
            || active.window_bytes != active.byte_len
            || active.source_offset != 0
            || active.destination_offset != 0
        {
            return None;
        }
        let source = self.allocations.get(&active.source)?;
        let destination = self.allocations.get(&active.destination)?;
        let byte_len = u64::try_from(source.bytes.len()).ok()?;
        let max_window_bytes = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
            .checked_mul(u64::try_from(KFD_RUNTIME_MAX_SDMA_WINDOW_PACKETS_V1).ok()?)?;
        if source.kind != RuntimeMemoryKindV1::HostVisible
            || destination.kind != RuntimeMemoryKindV1::DeviceLocal
            || source.sdma_shadow_dirty
            || !source.native_dirty.is_empty()
            || destination.bytes.len() != source.bytes.len()
            || active.byte_len != byte_len
            || !byte_len.is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
            || byte_len > max_window_bytes
        {
            return None;
        }
        let sha256 = source.content_sha256?;
        let role = Gfx942DeviceContentRoleV1::new(KFD_RUNTIME_PERSISTENT_H2D_PROVENANCE_ROLE_V1, 0)
            .ok()?;
        let content = Gfx942DeviceContentDescriptorV1::new(role, byte_len, sha256).ok()?;
        Some((content, Arc::clone(&source.bytes), sha256))
    }

    #[allow(clippy::too_many_arguments)]
    fn restore_h2d_ready_storage_v1(
        &mut self,
        active: SdmaStorageBindingV1,
        ready: PersistentComputeReadyOwnerV1,
        promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
        host: SdmaBufferOwnerV1,
        bytes: Arc<[u8]>,
        sha256: [u8; 32],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let owner = KfdRuntimeSdmaInFlightV1::Async(active.id);
        let source_slot_matches = self.allocations.get(&active.source).is_some_and(|record| {
            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(actual) if actual == owner)
        });
        let destination_slot_matches =
            self.allocations
                .get(&active.destination)
                .is_some_and(|record| {
                    matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(actual) if actual == owner)
                });
        if !source_slot_matches || !destination_slot_matches {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::ReadyPair {
                ready,
                host,
            });
            return Err(self.terminal_error(
                "persistent-compute H2D-ready restoration slot changed unexpectedly",
            ));
        }
        self.allocations
            .get_mut(&active.source)
            .expect("authenticated H2D source remains indexed")
            .sdma_storage = KfdRuntimeSdmaStorageV1::Host(host);
        let destination = self
            .allocations
            .get_mut(&active.destination)
            .expect("authenticated H2D destination remains indexed");
        destination.sdma_storage =
            KfdRuntimeSdmaStorageV1::H2dReady(Box::new(PersistentComputeReadyStorageV1 {
                owner: ready,
                promotion,
            }));
        destination.bytes = bytes;
        destination.content_sha256 = Some(sha256);
        destination.last_full_host_write = None;
        destination.sdma_shadow_dirty = false;
        Ok(())
    }

    fn observe_ready_promotion_performance_v1(
        &mut self,
        content: Gfx942DeviceContentDescriptorV1,
        authentication: Duration,
    ) -> Option<KfdRuntimeReadyPromotionPerformanceV1> {
        let ordinal = self.next_ready_promotion_ordinal?;
        self.next_ready_promotion_ordinal = ordinal.checked_add(1);
        let observation = KfdRuntimeReadyPromotionPerformanceV1 {
            ordinal,
            content_ordinal: content.role().ordinal(),
            authenticated_bytes: content.byte_len(),
            authentication,
        };
        self.last_ready_promotion_performance = Some(observation);
        Some(observation)
    }

    fn finish_sdma_copy_v1(
        &mut self,
        submission: u64,
        completed: DirectionalSdmaCompletedOwnerV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = &self.active_sdma[&submission];
        let binding = SdmaStorageBindingV1::from(active);
        let direction = match self.direct_sdma_direction_for_active_v1(active) {
            Ok(direction) => direction,
            Err(detail) => {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                    completed,
                ));
                return Err(self.terminal_error(detail));
            }
        };
        let Some(expected_requests) = active.window_requests.as_ref() else {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                completed,
            ));
            return Err(self.terminal_error(
                "directional persistent SDMA completion has no published request custody",
            ));
        };
        let expected = expected_requests.first();
        let expected_offsets = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                (expected.source_offset, expected.destination_offset)
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                (expected.destination_offset, expected.source_offset)
            }
        };
        if completed.direction() != direction
            || u64::from(completed.copy_bytes()) != active.window_bytes
            || completed.packet_count() != expected_requests.packet_count()
            || completed.host_offset() != expected_offsets.0
            || completed.device_offset() != expected_offsets.1
        {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Completed(
                completed,
            ));
            return Err(self.terminal_error(
                "directional persistent SDMA completion metadata changed unexpectedly",
            ));
        }
        if let Some((content, bytes, sha256)) = self.full_h2d_ready_provenance_v1(active, direction)
        {
            let promotion_started = Instant::now();
            match self
                .directional_sdma_ops_v1()
                .promote_full_h2d_to_compute_ready(completed, content)
            {
                Ok((ready, host)) => {
                    let promotion = self.observe_ready_promotion_performance_v1(
                        content,
                        promotion_started.elapsed(),
                    );
                    self.restore_h2d_ready_storage_v1(
                        binding, ready, promotion, host, bytes, sha256,
                    )?;
                    return self.finish_sdma_window_progress_v1(submission);
                }
                Err(PersistentComputeReadyTransitionFailureV1::Recovered { pair }) => {
                    self.restore_directional_sdma_storage_v1(
                        binding,
                        direction,
                        KfdRuntimeSdmaInFlightV1::Async(submission),
                        pair,
                        true,
                    )?;
                    return self.finish_sdma_window_progress_v1(submission);
                }
                Err(PersistentComputeReadyTransitionFailureV1::ForeignQueue {
                    detail,
                    terminal_receiver,
                    completed,
                }) => {
                    self.retain_terminal_sdma_custody_v1(
                        KfdRuntimeTerminalSdmaCustodyV1::Completed(completed),
                    );
                    let receiver = if terminal_receiver {
                        "terminal receiver"
                    } else {
                        "live receiver"
                    };
                    return Err(self.terminal_error(format!(
                        "KFD persistent-compute H2D-ready promotion returned a foreign receipt to a {receiver}: {detail}"
                    )));
                }
                Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                    detail,
                    custody,
                }) => {
                    if let Some(custody) = custody {
                        self.retain_sdma_seam_terminal_v1(custody);
                    }
                    return Err(self.terminal_error(format!(
                        "KFD persistent-compute H2D-ready promotion: {detail}"
                    )));
                }
            }
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
            binding,
            direction,
            KfdRuntimeSdmaInFlightV1::Async(submission),
            pair,
            true,
        )?;
        self.finish_sdma_window_progress_v1(submission)
    }

    fn finish_same_device_sdma_copy_v1(
        &mut self,
        submission: u64,
        completed: SameDeviceSdmaCompletedOwnerV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = &self.active_sdma[&submission];
        let binding = SdmaStorageBindingV1::from(active);
        let Some(expected_requests) = active.window_requests.as_ref() else {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::SameDeviceCompleted(completed),
            );
            return Err(self.terminal_error(
                "same-device persistent SDMA completion has no published request custody",
            ));
        };
        let expected = expected_requests.first();
        if self.direct_sdma_copy_kind_for_active_v1(active) != Ok(DirectSdmaCopyKindV1::SameDevice)
            || u64::from(completed.copy_bytes()) != active.window_bytes
            || completed.packet_count() != expected_requests.packet_count()
            || completed.source_offset() != expected.source_offset
            || completed.destination_offset() != expected.destination_offset
        {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::SameDeviceCompleted(completed),
            );
            return Err(self.terminal_error(
                "same-device persistent SDMA completion metadata changed unexpectedly",
            ));
        }
        let pair =
            match self.directional_sdma_ops_v1().retire_same_device(completed) {
                Ok(pair) => pair,
                Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => {
                    self.retain_terminal_sdma_custody_v1(
                        KfdRuntimeTerminalSdmaCustodyV1::SameDeviceCompleted(custody),
                    );
                    return Err(self
                        .terminal_error("same-device persistent SDMA frontier retirement failed"));
                }
                Err(SdmaTransitionFailureV1::ProcessTeardown { custody, .. }) => {
                    self.retain_sdma_seam_terminal_v1(custody);
                    return Err(self
                        .terminal_error("same-device persistent SDMA frontier retirement failed"));
                }
            };
        self.restore_same_device_sdma_storage_v1(
            binding,
            KfdRuntimeSdmaInFlightV1::Async(submission),
            pair,
            true,
        )?;
        self.finish_sdma_window_progress_v1(submission)
    }

    fn finish_sdma_window_progress_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = &self.active_sdma[&submission];
        let completed_bytes = active
            .completed_bytes
            .checked_add(active.window_bytes)
            .filter(|bytes| *bytes <= active.byte_len);
        let Some(completed_bytes) = completed_bytes else {
            return Err(self.terminal_error("SDMA copy progress overflow"));
        };
        if completed_bytes < active.byte_len {
            // Poll only observes and returns exact custody. Explicit flush owns
            // every continuation publication.
            let active = self
                .active_sdma
                .get_mut(&submission)
                .expect("retained SDMA copy");
            active.completed_bytes = completed_bytes;
            active.phase = ActiveSdmaPhaseV1::Ready;
            active.window_bytes = 0;
            active.window_requests = None;
            return Ok(BackendPollV1::Pending);
        }
        self.settle_sdma_copy_v1(submission, sdma_settlement::SdmaSettlementV1::Succeeded)
    }

    fn release_sdma_dependency_counts_v1(counts: &mut HashMap<u64, usize>, dependencies: &[u64]) {
        for dependency in dependencies {
            let remove = {
                let count = counts
                    .get_mut(dependency)
                    .expect("active SDMA dependency remains retained");
                *count = count.checked_sub(1).expect("positive SDMA retain count");
                *count == 0
            };
            if remove {
                counts.remove(dependency);
            }
        }
    }

    fn quiescent_sdma_marker_capacity_is_reserved_v1(&self) -> bool {
        self.quiescent_sdma_submissions.capacity()
            >= self
                .quiescent_sdma_submissions
                .len()
                .saturating_add(self.sdma_completion_reservations)
    }

    fn progress_unpublished_sdma_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.observe_sdma_dependencies_v1(submission, true)?;
        let Some(active) = self.active_sdma.get(&submission) else {
            return Ok(self.submissions[&submission].status);
        };
        if active.dependency_cursor != active.dependencies.len() {
            return Ok(BackendPollV1::Pending);
        }
        self.publish_sdma_copy_v1(submission)
    }

    fn observe_unpublished_sdma_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.observe_sdma_dependencies_v1(submission, false)
    }

    fn observe_sdma_dependencies_v1(
        &mut self,
        submission: u64,
        progress: bool,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        loop {
            let active = &self.active_sdma[&submission];
            let Some(dependency) = active.dependencies.get(active.dependency_cursor).copied()
            else {
                return Ok(BackendPollV1::Pending);
            };
            // Every ancestor remains indexed throughout recursive observation.
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.poll_v1(dependency)));
            let status = match outcome {
                Err(payload) => sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    self.poison_terminal_v1()
                }),
                Ok(Ok(status)) => status,
                Ok(Err(RuntimeBackendFailureV1::Rejected(error))) if !progress => {
                    return Err(self.terminal_error(format!(
                        "KFD unpublished SDMA copy retained an exact dependency that was rejected during observation: {error}"
                    )));
                }
                Ok(Err(failure @ RuntimeBackendFailureV1::Quiescent(_))) => {
                    self.fail_quiescent_sdma_copy_v1(submission)?;
                    return Err(failure);
                }
                Ok(Err(failure)) => return Err(failure),
            };
            match status {
                BackendPollV1::Succeeded => {
                    self.active_sdma
                        .get_mut(&submission)
                        .expect("retained copy")
                        .dependency_cursor += 1;
                }
                BackendPollV1::Pending => return Ok(BackendPollV1::Pending),
                BackendPollV1::Failed { .. } => {
                    return self.fail_unpublished_sdma_copy_v1(submission);
                }
            }
        }
    }

    fn recycle_transient_sdma_buffer_v1(
        &mut self,
        buffer: SdmaBufferOwnerV1,
        operation: &'static str,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.recycle_sdma_owner_v1(
            buffer,
            sdma_recycle::SdmaRecycleTargetV1::Transient(operation),
        )
    }

    fn normalize_h2d_ready_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.try_normalize_h2d_ready_v1(allocation)
            .map_err(|device| {
                self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(
                    device,
                ));
                self.terminal_error(
                    "persistent-compute ready normalization slot changed unexpectedly",
                )
            })
    }

    #[allow(
        clippy::result_large_err,
        reason = "failed normalization returns its original device owner without allocating"
    )]
    fn try_normalize_h2d_ready_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), DirectionalSdmaDeviceOwnerV1> {
        let is_ready = self.allocations.get(&allocation).is_some_and(|record| {
            matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::H2dReady(_)
                    | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                    | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
            )
        });
        if !is_ready {
            return Ok(());
        }
        let device = match core::mem::replace(
            &mut self
                .allocations
                .get_mut(&allocation)
                .expect("ready allocation remains indexed")
                .sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
        ) {
            KfdRuntimeSdmaStorageV1::H2dReady(ready) => ready.owner.normalize(),
            KfdRuntimeSdmaStorageV1::PersistentReplay(input) => {
                DirectionalSdmaDeviceOwnerV1::Native(input.into_allocation())
            }
            KfdRuntimeSdmaStorageV1::InitializedStorage(ready) => ready.normalize(),
            _ => unreachable!("preflighted persistent-compute storage remains normalizable"),
        };
        let slot_matches = self.allocations.get(&allocation).is_some_and(|record| {
            matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
            )
        });
        if !slot_matches {
            return Err(device);
        }
        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("normalized allocation remains indexed");
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(device));
        #[cfg(test)]
        {
            record.scripted_three_binding_replay = false;
        }
        Ok(())
    }

    #[cfg(test)]
    fn take_h2d_ready_for_compute_v1(
        &mut self,
        allocation: u64,
        submission: u64,
    ) -> Result<PersistentComputeReadyStorageV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "persistent-compute allocation disappeared",
            )
        })?;
        match core::mem::replace(
            &mut record.sdma_storage,
            KfdRuntimeSdmaStorageV1::ComputeInFlight(submission),
        ) {
            KfdRuntimeSdmaStorageV1::H2dReady(ready) => Ok(*ready),
            storage => {
                record.sdma_storage = storage;
                Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent-compute H2D-ready custody changed; materialization is forbidden",
                ))
            }
        }
    }

    fn take_persistent_compute_input_v1(
        &mut self,
        admission: PersistentFullRangeComputeAdmissionV1,
        submission: u64,
    ) -> Result<
        (
            KfdRuntimePersistentComputeInputV1,
            PersistentComputeBindRestoreV1,
        ),
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let PersistentFullRangeComputeAdmissionV1 {
            allocation, source, ..
        } = admission;
        let retained = |input, promotion, restore_shell| {
            (
                input,
                PersistentComputeBindRestoreV1 {
                    admission,
                    submission,
                    promotion,
                    restore_shell,
                },
            )
        };
        let restore = if source == PersistentFullRangeComputeSourceV1::InitializedStorage {
            Some(self.prepare_persistent_restore_shell_v1(
                PersistentFullRangeComputeAdmissionV1 {
                    allocation,
                    access: RuntimeAccessV1::ReadWrite,
                    source,
                },
            )?)
        } else {
            None
        };
        let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "persistent-compute allocation disappeared",
            )
        })?;
        if record.persistent_storage_restore.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "persistent storage restoration is already pending",
            ));
        }
        let storage = core::mem::replace(
            &mut record.sdma_storage,
            KfdRuntimeSdmaStorageV1::ComputeInFlight(submission),
        );
        match (source, storage) {
            (
                PersistentFullRangeComputeSourceV1::InitializedStorage,
                KfdRuntimeSdmaStorageV1::InitializedStorage(ready),
            ) => {
                record.persistent_storage_restore = restore;
                Ok(retained(ready.into_input(), None, None))
            }
            (
                PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
                KfdRuntimeSdmaStorageV1::H2dReady(ready),
            ) => {
                let (ready, shell) = take_restore_shell_v1(ready);
                let promotion = ready.promotion;
                let input = match ready.owner {
                    PersistentComputeReadyOwnerV1::Native(ready) => {
                        KfdRuntimePersistentComputeInputV1::Native(
                            Gfx942PersistentComputeInputV1::Initialized(ready),
                        )
                    }
                    #[cfg(test)]
                    owner @ PersistentComputeReadyOwnerV1::Scripted { .. } => {
                        KfdRuntimePersistentComputeInputV1::ScriptedReady(
                            PersistentComputeReadyStorageV1 { owner, promotion },
                        )
                    }
                };
                Ok(retained(
                    input,
                    promotion,
                    Some(ThreeBindingPersistentRestoreShellV1 {
                        ready: Some(shell),
                        device: None,
                        replay: None,
                        initialized: None,
                    }),
                ))
            }
            (
                PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                KfdRuntimeSdmaStorageV1::PersistentReplay(input),
            ) => {
                let (input, shell) = take_restore_shell_v1(input);
                Ok(retained(
                    KfdRuntimePersistentComputeInputV1::Native(input),
                    None,
                    Some(ThreeBindingPersistentRestoreShellV1 {
                        ready: None,
                        device: None,
                        replay: Some(shell),
                        initialized: None,
                    }),
                ))
            }
            #[cfg(test)]
            (
                PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                KfdRuntimeSdmaStorageV1::Device(device),
            ) => {
                let (device, shell) = take_restore_shell_v1(device);
                Ok(retained(
                    KfdRuntimePersistentComputeInputV1::ScriptedReplay(device),
                    None,
                    Some(ThreeBindingPersistentRestoreShellV1 {
                        ready: None,
                        device: Some(shell),
                        replay: None,
                        initialized: None,
                    }),
                ))
            }
            (_, storage) => {
                record.sdma_storage = storage;
                Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent-compute input custody changed; materialization is forbidden",
                ))
            }
        }
    }

    fn restore_persistent_bind_input_v1(
        &mut self,
        input: KfdRuntimePersistentComputeInputV1,
        restoration: PersistentComputeBindRestoreV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let PersistentComputeBindRestoreV1 {
            admission,
            submission,
            promotion,
            restore_shell,
        } = restoration;
        let storage_input = match &input {
            KfdRuntimePersistentComputeInputV1::Native(
                Gfx942PersistentComputeInputV1::InitializedStorage(_),
            ) => true,
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedStorage(_) => true,
            _ => false,
        };
        if admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage
            && restore_shell.is_none()
            && storage_input
        {
            return self.restore_initialized_storage_input_v1(
                admission.allocation,
                submission,
                input,
            );
        }
        let valid = admission.source != PersistentFullRangeComputeSourceV1::InitializedStorage
            && self.allocations.get(&admission.allocation).is_some_and(|record| {
                matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == submission)
                    && record.persistent_storage_restore.is_none()
            })
            && restore_shell.as_ref().is_some_and(|shell| shell.accepts_v1(admission, &input));
        if !valid {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::PersistentRuntimeInput(input),
            );
            return Err(
                self.terminal_error("persistent bind rejection restore slot/input/shell mismatch")
            );
        }
        // Retryable bind preserves the input variant, so its original empty box suffices.
        let storage = restore_shell.unwrap().restore_v1(input, promotion);
        let record = self.allocations.get_mut(&admission.allocation).unwrap();
        record.sdma_storage = storage;
        Ok(())
    }

    fn take_three_binding_persistent_inputs_v1(
        &mut self,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        submission: u64,
    ) -> Result<
        ThreeBindingPersistentInputRosterV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let all_current = admissions.iter().all(|admission| {
            self.allocations
                .get(&admission.allocation)
                .is_some_and(|record| match (admission.source, &record.sdma_storage) {
                    (
                        PersistentFullRangeComputeSourceV1::InitializedStorage,
                        KfdRuntimeSdmaStorageV1::InitializedStorage(_),
                    ) => true,
                    (
                        PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
                        KfdRuntimeSdmaStorageV1::H2dReady(_),
                    ) => true,
                    (
                        PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                        KfdRuntimeSdmaStorageV1::PersistentReplay(_),
                    ) => true,
                    #[cfg(test)]
                    (
                        PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                        KfdRuntimeSdmaStorageV1::Device(_),
                    ) => record.scripted_three_binding_replay,
                    _ => false,
                })
        });
        if !all_current {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "three-binding persistent input custody changed before extraction",
            ));
        }
        let mut take_preflighted = |admission: PersistentFullRangeComputeAdmissionV1| {
            let record = self
                .allocations
                .get_mut(&admission.allocation)
                .expect("preflighted three-binding allocation remains indexed");
            let storage = core::mem::replace(
                &mut record.sdma_storage,
                KfdRuntimeSdmaStorageV1::ComputeInFlight(submission),
            );
            let (input, promotion) = match (admission.source, storage) {
                (
                    PersistentFullRangeComputeSourceV1::InitializedStorage,
                    KfdRuntimeSdmaStorageV1::InitializedStorage(ready),
                ) => (ready.into_input(), None),
                (
                    PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
                    KfdRuntimeSdmaStorageV1::H2dReady(ready),
                ) => {
                    let ready = *ready;
                    let promotion = ready.promotion;
                    let input = match ready.owner {
                        PersistentComputeReadyOwnerV1::Native(ready) => {
                            KfdRuntimePersistentComputeInputV1::Native(
                                Gfx942PersistentComputeInputV1::Initialized(ready),
                            )
                        }
                        #[cfg(test)]
                        owner @ PersistentComputeReadyOwnerV1::Scripted { .. } => {
                            KfdRuntimePersistentComputeInputV1::ScriptedReady(
                                PersistentComputeReadyStorageV1 { owner, promotion },
                            )
                        }
                    };
                    (input, promotion)
                }
                (
                    PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                    KfdRuntimeSdmaStorageV1::PersistentReplay(input),
                ) => (KfdRuntimePersistentComputeInputV1::Native(*input), None),
                #[cfg(test)]
                (
                    PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                    KfdRuntimeSdmaStorageV1::Device(device),
                ) => (
                    KfdRuntimePersistentComputeInputV1::ScriptedReplay(*device),
                    None,
                ),
                _ => unreachable!("three-binding extraction was preflighted atomically"),
            };
            (input, promotion)
        };
        let [admission_a, admission_b, admission_c] = admissions;
        let (input_a, promotion_a) = take_preflighted(admission_a);
        let (input_b, promotion_b) = take_preflighted(admission_b);
        let (input_c, promotion_c) = take_preflighted(admission_c);
        Ok((
            [input_a, input_b, input_c],
            [promotion_a, promotion_b, promotion_c],
        ))
    }

    fn prepare_three_binding_restore_shells_v1(
        &self,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
    ) -> Result<
        [ThreeBindingPersistentRestoreShellV1; 3],
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        Ok([
            self.prepare_persistent_restore_shell_v1(admissions[0])?,
            self.prepare_persistent_restore_shell_v1(admissions[1])?,
            self.prepare_persistent_restore_shell_v1(admissions[2])?,
        ])
    }

    fn prepare_persistent_restore_shell_v1(
        &self,
        admission: PersistentFullRangeComputeAdmissionV1,
    ) -> Result<
        ThreeBindingPersistentRestoreShellV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let record = self.allocations.get(&admission.allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "three-binding persistent restore allocation disappeared",
            )
        })?;
        let shell = match record.sdma_storage {
            KfdRuntimeSdmaStorageV1::H2dReady(_) => ThreeBindingPersistentRestoreShellV1 {
                initialized: None,
                ready: Some(try_uninit_box_v1().map_err(|_| {
                    Self::capacity("KFD three-binding ready restore-shell allocation failed")
                })?),
                #[cfg(test)]
                device: (admission.access != RuntimeAccessV1::Read)
                    .then(try_uninit_box_v1)
                    .transpose()
                    .map_err(|_| {
                        Self::capacity(
                            "KFD three-binding scripted device restore-shell allocation failed",
                        )
                    })?,
                #[cfg(not(test))]
                device: None,
                replay: (admission.access != RuntimeAccessV1::Read)
                    .then(try_uninit_box_v1)
                    .transpose()
                    .map_err(|_| {
                        Self::capacity(
                            "KFD three-binding output replay restore-shell allocation failed",
                        )
                    })?,
            },
            KfdRuntimeSdmaStorageV1::Device(_) => ThreeBindingPersistentRestoreShellV1 {
                initialized: None,
                ready: None,
                device: Some(try_uninit_box_v1().map_err(|_| {
                    Self::capacity("KFD three-binding device restore-shell allocation failed")
                })?),
                replay: (admission.access == RuntimeAccessV1::Write)
                    .then(try_uninit_box_v1)
                    .transpose()
                    .map_err(|_| {
                        Self::capacity("KFD three-binding replay restore-shell allocation failed")
                    })?,
            },
            KfdRuntimeSdmaStorageV1::PersistentReplay(_) => ThreeBindingPersistentRestoreShellV1 {
                initialized: None,
                ready: None,
                device: None,
                replay: Some(try_uninit_box_v1().map_err(|_| {
                    Self::capacity("KFD three-binding replay restore-shell allocation failed")
                })?),
            },
            KfdRuntimeSdmaStorageV1::InitializedStorage(_) => {
                ThreeBindingPersistentRestoreShellV1 {
                    initialized: Some(try_uninit_box_v1().map_err(|_| {
                        Self::capacity("KFD initialized-storage restore-shell allocation failed")
                    })?),
                    ready: None,
                    replay: Some(try_uninit_box_v1().map_err(|_| {
                        Self::capacity("KFD storage-origin replay restore-shell allocation failed")
                    })?),
                    #[cfg(test)]
                    device: Some(try_uninit_box_v1().map_err(|_| {
                        Self::capacity("KFD scripted storage restore-shell allocation failed")
                    })?),
                    #[cfg(not(test))]
                    device: None,
                }
            }
            _ => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "three-binding persistent restore storage changed before extraction",
                ));
            }
        };
        Ok(shell)
    }

    fn restore_three_binding_persistent_inputs_v1(
        &mut self,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        submission: u64,
        inputs: [KfdRuntimePersistentComputeInputV1; 3],
        promotions: [Option<KfdRuntimeReadyPromotionPerformanceV1>; 3],
        shells: [ThreeBindingPersistentRestoreShellV1; 3],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let slots_current = admissions.iter().all(|admission| {
            self.allocations.get(&admission.allocation).is_some_and(|record| {
                matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == submission)
            })
        });
        if !slots_current {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(inputs),
            );
            return Err(self.terminal_error(
                "three-binding persistent restoration slots changed unexpectedly",
            ));
        }
        let shells_match = admissions
            .iter()
            .zip(&inputs)
            .zip(&shells)
            .all(|((admission, input), shell)| shell.accepts_v1(*admission, input));
        if !shells_match {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(inputs),
            );
            return Err(
                self.terminal_error("three-binding persistent restore-shell/input mismatch")
            );
        }
        for (((admission, input), promotion), shell) in admissions
            .into_iter()
            .zip(inputs)
            .zip(promotions)
            .zip(shells)
        {
            #[cfg(test)]
            let scripted_three_binding_replay = matches!(
                &input,
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(_)
            );
            let storage = shell.restore_v1(input, promotion);
            let record = self
                .allocations
                .get_mut(&admission.allocation)
                .expect("preflighted three-binding restoration allocation");
            record.sdma_storage = storage;
            #[cfg(test)]
            {
                record.scripted_three_binding_replay = scripted_three_binding_replay;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn restore_persistent_compute_completion_v1(
        &mut self,
        allocation: u64,
        submission: u64,
        device: DirectionalSdmaDeviceOwnerV1,
        effect: Gfx942PersistentComputeEffectV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self
            .allocations
            .get(&allocation)
            .is_some_and(|record| record.persistent_storage_restore.is_some())
        {
            self.restore_initialized_storage_input_v1(
                allocation,
                submission,
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(device),
            )?;
            apply_persistent_compute_effect_v1(
                self.allocations
                    .get_mut(&allocation)
                    .expect("restored storage-origin completion"),
                effect,
            );
            return Ok(());
        }
        let slot_matches = self.allocations.get(&allocation).is_some_and(|record| {
            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == submission)
        });
        if !slot_matches {
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(device));
            return Err(self.terminal_error(
                "persistent-compute completion restoration slot changed unexpectedly",
            ));
        }
        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("persistent-compute allocation remains indexed");
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(device));
        apply_persistent_compute_effect_v1(record, effect);
        debug_assert!(record.native_dirty.is_empty());
        Ok(())
    }

    fn release_sdma_storage_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.normalize_h2d_ready_v1(allocation)?;
        let (storage, kind) = {
            let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD allocation",
                )
            })?;
            if matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic) {
                return Ok(());
            }
            if matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(_) | KfdRuntimeSdmaStorageV1::ComputeInFlight(_)
            ) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent SDMA allocation is retained by pending work",
                ));
            }
            (
                std::mem::replace(
                    &mut record.sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
                ),
                record.kind,
            )
        };
        let buffer = match storage {
            KfdRuntimeSdmaStorageV1::Host(buffer)
            | KfdRuntimeSdmaStorageV1::DemotedDevice(buffer) => buffer,
            KfdRuntimeSdmaStorageV1::Device(device) => {
                self.demote_sdma_device_v1(allocation, device)?
            }
            KfdRuntimeSdmaStorageV1::Synthetic
            | KfdRuntimeSdmaStorageV1::H2dReady(_)
            | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
            | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
            | KfdRuntimeSdmaStorageV1::ComputeInFlight(_)
            | KfdRuntimeSdmaStorageV1::InFlight(_) => {
                unreachable!("preflighted releasable SDMA storage")
            }
        };
        self.recycle_sdma_owner_v1(
            buffer,
            sdma_recycle::SdmaRecycleTargetV1::Indexed { allocation, kind },
        )
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
            return self.access_indexed_sdma_host_v1(
                allocation,
                "KFD persistent host write",
                |ops, buffer| ops.write_host(buffer, byte_offset, bytes),
            );
        }

        let copy_bytes = u32::try_from(bytes.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "SDMA upload exceeds one admitted linear packet",
            )
        })?;
        let staging = self.allocate_sdma_owner_v1(
            RuntimeMemoryKindV1::HostVisible,
            bytes.len(),
            1,
            true,
            "KFD upload staging",
        )?;
        let staging = self.initialize_sdma_host_v1(staging, bytes, "KFD upload staging write")?;
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

    fn upload_full_sdma_host_v1(
        &mut self,
        allocation: u64,
        bytes: &[u8],
    ) -> Result<Option<[u8; 32]>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_host = self
            .allocations
            .get(&allocation)
            .is_some_and(|record| matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Host(_)));
        if !is_host {
            self.upload_sdma_range_v1(allocation, 0, bytes)?;
            return Ok(None);
        }
        self.access_indexed_sdma_host_v1(
            allocation,
            "KFD persistent authenticated host write",
            |ops, buffer| ops.write_full_host_authenticated(buffer, bytes),
        )
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
            self.read_indexed_sdma_host_into_v1(allocation, byte_offset, destination)?;
            return Ok(true);
        }

        let copy_bytes = u32::try_from(destination.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "SDMA download exceeds one admitted linear packet",
            )
        })?;
        let staging = self.allocate_sdma_owner_v1(
            RuntimeMemoryKindV1::HostVisible,
            destination.len(),
            1,
            true,
            "KFD download staging",
        )?;
        let staging = self.execute_synchronous_directional_sdma_v1(
            allocation,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            staging,
            0,
            byte_offset,
            copy_bytes,
            "download",
        )?;
        self.readback_and_recycle_transient_sdma_v1(staging, destination)?;
        Ok(true)
    }

    fn synchronize_sdma_shadow_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.normalize_h2d_ready_v1(allocation)?;
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
        self.require_unpinned_native_lane_v1(compute_lane)?;
        if self.native_reconciliation_holds_v1(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native reconciliation retains this allocation",
            ));
        }
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
        for (offset, bytes) in updates {
            // Only the recycled extent is authoritative. Other shadow bytes
            // may predate an already-completed asynchronous reconciliation.
            self.upload_sdma_range_v1(allocation, offset as u64, &bytes)
                .map_err(Self::after_possible_host_mutation)?;
            let record = self
                .allocations
                .get_mut(&allocation)
                .expect("native-dirty allocation remains retained");
            let end = offset
                .checked_add(bytes.len())
                .expect("validated native readback range fits host address space");
            if offset == 0 && end == record.bytes.len() {
                record.bytes = Arc::from(bytes);
            } else {
                Arc::make_mut(&mut record.bytes)[offset..end].copy_from_slice(&bytes);
            }
            record.content_sha256 = None;
            record.last_full_host_write = None;
        }
        if let Some(record) = self.allocations.get_mut(&allocation) {
            record
                .native_dirty
                .retain(|extent| extent.compute_lane != compute_lane);
            record.sdma_initialized = true;
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
    pub(crate) fn mock() -> Self {
        Self::mock_with_staging_budgets(StagingBudgetsV1 {
            max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
            max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
        })
    }

    #[cfg(test)]
    pub(crate) fn mock_worker_v3_generated_only_v1() -> Self {
        Self::new_with_staging_budgets(
            BackendDeviceDescriptionV1 {
                backend_device: 7,
                name: "mock generated-only gfx942".to_owned(),
                target: "gfx942:xnack-".to_owned(),
                global_memory_bytes: 0,
                capabilities: kfd_capabilities_v1(),
            },
            None,
            KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly,
            StagingBudgetsV1 {
                max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
                max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
            },
        )
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

fn continue_pending_compute_wait_v1(
    prior_reservations: usize,
    current_reservations: usize,
    attempts: &mut u32,
    sleep: &mut Duration,
    deadline: Instant,
    backoff: impl FnOnce(u32, &mut Duration, Instant) -> bool,
) -> bool {
    if Instant::now() >= deadline {
        return false;
    }
    // Logical settlement is finite progress under wait's exclusive borrow.
    // Physical retirement or another observation of Pending is not progress.
    if current_reservations < prior_reservations {
        *attempts = 0;
        *sleep = WAIT_INITIAL_SLEEP_V1;
        return true;
    }
    *attempts = attempts.saturating_add(1);
    backoff(*attempts, sleep, deadline)
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

fn profile_host_timing_v1(performance: KfdRuntimeLaunchPerformanceV1) -> KfdProfileHostTimingV1 {
    KfdProfileHostTimingV1 {
        preparation_ns: duration_nanoseconds_v1(performance.preparation),
        bound_snapshot_ns: duration_nanoseconds_v1(performance.bound_snapshot),
        authority_ns: duration_nanoseconds_v1(performance.authority),
        native_binding_ns: duration_nanoseconds_v1(performance.native_binding),
        publication_ns: duration_nanoseconds_v1(performance.publication),
        publish_to_completion_ns: duration_nanoseconds_v1(performance.publish_to_completion),
        completed_readback_ns: duration_nanoseconds_v1(performance.completed_readback),
        recycle_ns: duration_nanoseconds_v1(performance.recycle()),
    }
}

fn record_initial_persistent_timing_v1(
    performance: &mut KfdRuntimeLaunchPerformanceV1,
    native_binding: Duration,
    publication: Duration,
) {
    performance.native_binding = native_binding;
    performance.publication = publication;
}

fn completion_detach_restore_duration_v1(
    recycle_inclusive: Duration,
    completion_signal_recycle: Duration,
) -> Duration {
    recycle_inclusive
        .checked_sub(completion_signal_recycle)
        .expect("continuous recycle timing cannot precede its signal-recycle boundary")
}

fn duration_nanoseconds_v1(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

impl KfdRuntimeBackendV1 {
    fn preflight_compute_v1(
        &self,
        launch: BackendLaunchV1<'_>,
        dependencies: ComputeDependencyRosterV1<'_>,
    ) -> Result<CollectedComputeDependenciesV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        self.require_live()?;
        self.require_no_generated_stream_v1(launch.stream)?;
        for binding in launch.bindings {
            self.allocations
                .reject_generated(binding.region.allocation)?;
        }
        if self.queue_retired || !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "the admitted KFD queue lifecycle has already retired",
            ));
        }
        self.validate_semantic_launch_v1(launch.semantic_launch, launch.geometry)?;
        self.require_submission_capacity_v1()?;
        let ordered_predecessor = self.stream_submission_tails.get(&launch.stream).copied();
        let (explicit_success_dependencies, input_admission) = match dependencies {
            ComputeDependencyRosterV1::Events(events) => (
                self.collect_compute_dependencies_v1(events)?,
                ComputeInputAdmissionV1::Ready,
            ),
            ComputeDependencyRosterV1::Exact(dependencies) => (
                self.collect_exact_compute_dependencies_v1(dependencies)?,
                ComputeInputAdmissionV1::ExactProducers,
            ),
        };
        Ok(CollectedComputeDependenciesV1 {
            minimum_dependency_depth: 1,
            ordered_predecessor,
            explicit_success_dependencies,
            input_admission,
            peer_gate: None,
            peer_access: PeerComputePermitsV1::default(),
        })
    }

    fn submit_collected_compute_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
        collected: CollectedComputeDependenciesV1,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.submit_collected_compute_with_payload_v1(launch, collected, None)
    }

    fn submit_collected_compute_with_payload_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
        collected: CollectedComputeDependenciesV1,
        retained: Option<Arc<RetainedComputeLaunchV1>>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let CollectedComputeDependenciesV1 {
            minimum_dependency_depth,
            ordered_predecessor,
            explicit_success_dependencies,
            input_admission,
            peer_gate,
            peer_access,
        } = collected;
        if !(1..=MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1).contains(&minimum_dependency_depth)
            || !peer_access.valid_for(peer_gate, self.next_handle, launch.bindings)
            || peer_gate.is_some_and(|gate| {
                gate.action(self.next_handle, false, false) == PeerComputeActionV1::Invalid
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "KFD compute peer gate names another consumer",
            ));
        }
        let dependency_depth = self
            .next_dependency_depth_v1(ordered_predecessor, &explicit_success_dependencies)
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
            })?
            .max(minimum_dependency_depth);
        self.validate_compute_launch_base_v1(&launch)?;
        let peer_dma = self.admit_peer_dma_owners_v1(launch.bindings, peer_gate, &peer_access)?;
        let quiescence_dependencies = self.capture_compute_quiescence_v1(
            &launch,
            &explicit_success_dependencies,
            ordered_predecessor,
        )?;
        self.validate_compute_launch_with_peer_v1(
            &launch,
            &explicit_success_dependencies,
            ordered_predecessor,
            &quiescence_dependencies,
            input_admission,
            &peer_dma,
        )?;

        let owned_launch = match retained {
            Some(retained) => retained,
            None => {
                RetainedComputeLaunchV1::copy_from(launch, self.launch_payload_account.as_ref())?
            }
        };
        let bindings = &*owned_launch.bindings;
        let mut retained_allocations = Vec::new();
        retained_allocations
            .try_reserve_exact(bindings.len())
            .map_err(|_| Self::capacity("KFD retained-allocation roster allocation failed"))?;
        for binding in bindings {
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
        let retained_dependencies = explicit_success_dependencies
            .iter()
            .copied()
            .chain(
                ordered_predecessor
                    .filter(|predecessor| !explicit_success_dependencies.contains(predecessor)),
            )
            .chain(quiescence_dependencies.iter().copied());
        let new_dependency_entries = retained_dependencies
            .clone()
            .filter(|submission| {
                !self
                    .compute_dependency_retain_counts
                    .contains_key(submission)
            })
            .count();
        self.compute_dependency_retain_counts
            .try_reserve(new_dependency_entries)
            .map_err(|_| Self::capacity("KFD compute dependency-retain growth failed"))?;
        if retained_dependencies.clone().any(|submission| {
            self.compute_dependency_retain_counts
                .get(&submission)
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
        for dependency in retained_dependencies {
            *self
                .compute_dependency_retain_counts
                .entry(dependency)
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
        self.has_admitted_peer_gate |= peer_gate.is_some();
        self.pending_compute.insert(
            id,
            PendingComputeSubmissionV1 {
                id,
                module,
                launch: owned_launch,
                retained_allocations: retained_allocations.into_boxed_slice(),
                ordered_predecessor,
                explicit_success_dependencies,
                explicit_dependency_cursor: 0,
                quiescence_dependencies,
                quiescence_cursor: 0,
                dependency_depth,
                peer_gate,
                peer_access,
            },
        );
        if self.pending_compute_can_publish_under_deadline_v1(id) {
            let pending = self
                .pending_compute
                .remove(&id)
                .expect("accepted clean compute remains pending before first progress");
            self.progress_pending_compute_v1(pending)
                .map_err(|failure| {
                    if let RuntimeBackendFailureV1::Rejected(mut error) = failure {
                        self.poison_terminal_v1();
                        error.kind = KfdRuntimeBackendErrorKindV1::Terminal;
                        RuntimeBackendFailureV1::Terminal(error)
                    } else {
                        failure
                    }
                })?;
        }
        Ok(id)
    }

    fn wait_published_sdma_v1(
        &mut self,
        submission: u64,
        timeout: Duration,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.active_sdma.contains_key(&submission) {
            self.observe_sdma_copy_v1(submission, Some(timeout))
        } else {
            self.poll_v1(submission)
        }
    }
}

impl RuntimeBackendV1 for KfdRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<crate::RuntimeAllocationAdmissionProfileV1, RuntimeBackendFailureV1<Self::Error>>
    {
        match self.request_binding_v1()? {
            None => Ok(crate::RuntimeAllocationAdmissionProfileV1::Legacy),
            Some(binding) => {
                let mut entries = Vec::new();
                entries
                    .try_reserve_exact(1)
                    .map_err(|_| Self::capacity("request admission roster"))?;
                entries.push(binding.clone());
                Ok(crate::RuntimeAllocationAdmissionProfileV1::Required(
                    entries,
                ))
            }
        }
    }

    fn capture_coherent_host_range_v1(
        &mut self,
        request: crate::BackendHostCaptureV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<crate::RuntimeHostCaptureErrorV1>> {
        self.capture_coherent_host_range_impl_v1(request)
    }

    fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
        if device != self.description.backend_device || !self.native_available {
            return RuntimeExecutionCapabilitiesV1::default();
        }
        RuntimeExecutionCapabilitiesV1 {
            concurrent_compute: self.launch_gate.advertises_generic_compute_v1(),
            native_async_copy: true,
            compute_copy_overlap: self.launch_gate.advertises_generic_compute_v1(),
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
        self.require_no_generated_stream_v1(stream)?;
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
        match self.allocate_with_outcome_v1(device, kind, byte_len, alignment)? {
            RuntimeBackendAllocationOutcomeV1::Allocated(handle) => Ok(handle),
            RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error) => {
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
        }
    }

    fn allocate_with_outcome_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<RuntimeBackendAllocationOutcomeV1<Self::Error>, RuntimeBackendFailureV1<Self::Error>>
    {
        if self.requires_request_witness_v1() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "composed allocation requires a Context request witness",
            ));
        }
        self.allocate_request_backing_v1(device, kind, byte_len, alignment)
    }

    fn allocate_with_request_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
        witness: crate::RuntimeAllocationRequestWitnessV1<'_>,
    ) -> crate::RuntimeRequestAllocationResultV1<Self::Error> {
        let valid = self
            .composed_request_binding
            .as_ref()
            .is_some_and(|binding| {
                binding.backend_device_v1() == device && witness.matches_v1(binding, byte_len)
            });
        crate::RuntimeRequestAllocationResultV1::Outcome(if valid {
            self.allocate_request_backing_v1(device, kind, byte_len, alignment)
        } else {
            Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "missing or mismatched composed request witness",
            ))
        })
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.allocations.require_ordinary(allocation)?;
        if self.allocation_is_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending KFD dispatch",
            ));
        }
        self.release_all_compute_caches_for_allocation_v1(allocation)
            .map_err(Self::after_possible_host_mutation)?;
        self.normalize_h2d_ready_v1(allocation)
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
        self.allocations.reject_generated(allocation)?;
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
                Some((Arc::clone(image), Some(*digest)))
            } else {
                let image: Arc<[u8]> =
                    try_copy_vec_v1(bytes, "KFD complete host-write image allocation failed")?
                        .into();
                Some((image, None))
            }
        } else {
            None
        };

        self.prepare_compute_caches_for_host_write_v1(allocation, full_write)
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
        let authenticated_sha256 = if full_write {
            self.upload_full_sdma_host_v1(allocation, bytes)
                .map_err(Self::after_possible_host_mutation)?
        } else {
            self.upload_sdma_range_v1(allocation, byte_offset, bytes)
                .map_err(Self::after_possible_host_mutation)?;
            None
        };
        self.allocations
            .get_mut(&allocation)
            .expect("written allocation remains indexed")
            .sdma_initialized = true;

        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("validated allocation remains retained");
        if let Some((image, cached_digest)) = full_image {
            let digest = authenticated_sha256
                .or(cached_digest)
                .unwrap_or_else(|| Sha256::digest(bytes).into());
            record.bytes = Arc::clone(&image);
            record.content_sha256 = Some(digest);
            record.last_full_host_write = Some((image, digest));
        } else {
            let destination = Arc::make_mut(&mut record.bytes)
                .get_mut(offset..end)
                .expect("preflighted host-write range remains valid after upload");
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
        self.allocations.reject_generated(allocation)?;
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
        let validated = ResidentModuleImageV1::load(image, self.host_image_account.as_ref())?;
        let profile_artifact = self.profile_content_v1(validated.bytes());
        let image_sha256 = Sha256::digest(validated.bytes()).into();
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
        self.release_retained_persistent_control_v1()?;
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
        let collected = self.preflight_compute_v1(
            launch,
            ComputeDependencyRosterV1::Events(launch.dependencies),
        )?;
        self.submit_collected_compute_v1(launch, collected)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.generated_submissions.contains_key(&submission) {
            return self.poll_generated_submission_v1(submission);
        }
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
            if !self.pending_compute[&submission].peer_gate_allows_native_checks_v1() {
                let pending = self
                    .pending_compute
                    .remove(&submission)
                    .expect("peer-gated pending compute remains indexed");
                return self.observe_pending_compute_v1(pending);
            }
            let persistent_pending = self
                .pending_compute
                .get(&submission)
                .is_some_and(|pending| {
                    let launch = pending.launch.borrowed();
                    self.persistent_full_range_admission_for_launch_v1(launch)
                        .is_some()
                        || self
                            .three_binding_persistent_admission_for_launch_v1(launch)
                            .is_some()
                });
            if persistent_pending
                && let Some(copy) = self
                    .pending_compute
                    .get(&submission)
                    .and_then(|pending| self.persistent_compute_sdma_blocker_v1(pending))
            {
                match self.poll_v1(copy) {
                    Ok(_) => {}
                    // This error describes the blocker, not the retained consumer.
                    Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                        return Ok(BackendPollV1::Pending);
                    }
                    Err(failure) => return Err(failure),
                }
            }
            let no_lane_is_free = self.free_compute_lane_v1().is_none();
            let active_lanes = self.active_compute_progress_roster_v1();
            let exclusive_blocker = if persistent_pending {
                active_lanes.iter().position(|active| *active)
            } else if self.persistent_compute_is_active_v1() {
                Some(0)
            } else {
                None
            };
            if no_lane_is_free || exclusive_blocker.is_some() {
                for (lane, active) in active_lanes.into_iter().enumerate() {
                    if active && (no_lane_is_free || exclusive_blocker == Some(lane)) {
                        match self.poll_compute_lane_v1(lane) {
                            Ok(_) => {}
                            Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                                return Ok(BackendPollV1::Pending);
                            }
                            Err(failure) => return Err(failure),
                        }
                    }
                }
            }
            let pending = self
                .pending_compute
                .remove(&submission)
                .expect("known pending compute remains indexed");
            return self.observe_pending_compute_v1(pending);
        }
        if self.active_sdma.contains_key(&submission) {
            return self.observe_sdma_copy_v1(submission, None);
        }
        let lane = self.active_compute_lane_v1(submission).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD submission",
            )
        })?;
        self.poll_compute_submission_v1(lane, submission)
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.active_sdma.get(&submission).is_some_and(|active| {
            matches!(
                active.phase,
                ActiveSdmaPhaseV1::DirectionalPublished(_)
                    | ActiveSdmaPhaseV1::SameDevicePublished(_)
            )
        }) {
            return self.wait_published_sdma_v1(
                submission,
                deadline.saturating_duration_since(Instant::now()),
            );
        }
        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        loop {
            if let Some(lane) = self.published_persistent_compute_lane_v1(submission)
                && let Some(status) =
                    self.wait_published_persistent_compute_lane_v1(lane, deadline)?
            {
                return Ok(status);
            }
            let prior_reservations = self.compute_completion_reservations;
            let status = self.poll_v1(submission)?;
            if status != BackendPollV1::Pending {
                return Ok(status);
            }
            if !continue_pending_compute_wait_v1(
                prior_reservations,
                self.compute_completion_reservations,
                &mut attempts,
                &mut sleep,
                deadline,
                apply_wait_backoff_v1,
            ) {
                return Ok(BackendPollV1::Pending);
            }
        }
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.generated_submissions.contains_key(&submission) {
            return self.release_generated_submission_v1(submission);
        }
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
        let profile_dispatch = self
            .submissions
            .get(&submission)
            .filter(|record| record.profile_dispatch_published)
            .and_then(|_| self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, submission));
        let removed = self.submissions.remove(&submission).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD submission",
            )
        })?;
        self.restore_unfinished_stream_tail_v1(removed.stream, submission);
        self.quiescent_sdma_submissions.remove(&submission);
        // Copies and unpublished cancellations have no dispatch lifecycle event.
        // A missing identity for an actual published dispatch still records loss.
        if removed.profile_dispatch_published {
            self.observe_profile_v1(
                profile_dispatch
                    .map(|dispatch| KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch }),
            );
        }
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

impl RuntimeProducerAwareLaunchBackendV1 for KfdRuntimeBackendV1 {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let launch = BackendLaunchV1 {
            stream: request.stream,
            kernel: request.kernel,
            explicit_kernarg: request.explicit_kernarg,
            bindings: request.bindings,
            dependencies: &[],
            geometry: request.geometry,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        };
        let collected = self.preflight_compute_v1(
            launch,
            ComputeDependencyRosterV1::Exact(request.dependencies),
        )?;
        self.submit_collected_compute_v1(launch, collected)
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
    CooperativeCopy(Box<CooperativeCopySubmissionV1>),
    DeferredCompute(Box<deferred_compute::DeferredComputeV1>),
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
    DeferredCompute {
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
    directed: Option<cooperative_directed::Root>,
    compute_xgmi: Option<Box<compute_xgmi::Root>>,
    compute_producer: Option<compute_peer::Producer>,
    frame_source: Option<peer_frame::Source>,
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
    scratch_byte_len: u64,
    sdma_leaf: Option<CooperativeSdmaLeafV1>,
    phase: CooperativeCopyPhaseV1,
    byte_cursor: usize,
}

enum CooperativeCopyProfileV1 {
    Scalar(Option<cooperative_directed::Root>),
    Segments(Arc<fe2o3_kfd::Gfx942ComputeXgmiSegmentsPlanV1>),
}

impl CooperativeCopySubmissionV1 {
    fn staging_byte_len(&self) -> u64 {
        if self.compute_xgmi.is_some() {
            0
        } else {
            self.source_region.byte_len
        }
    }

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
/// explicitly flush-driven state machine; poll and deadline wait only observe
/// stored state. The native-peer opt-in constructors enable bounded
/// native XGMI transfers of complete persistent allocations; other copies use
/// host staging. Mixed native/cooperative work is rejected
/// while either domain remains live on one logical stream.
#[must_use = "multi-device KFD backends must remain owned through quiescence"]
pub struct KfdMultiDeviceRuntimeBackendV1 {
    children: Vec<KfdRuntimeBackendV1>,
    device_children: HashMap<u64, usize>,
    request_policy: multi_admission::MultiRequestPolicyV1,
    compute_xgmi_routes: HashMap<(usize, usize), compute_xgmi::Route>,
    compute_xgmi_children: Vec<Option<u64>>,
    completed_compute_xgmi_copies: u64,
    terminal: bool,
    next_handle: u64,
    streams: HashMap<u64, RoutedHandleV1>,
    allocations: HashMap<u64, RoutedHandleV1>,
    generated_allocations: HashMap<u64, RoutedHandleV1>,
    generated_shells: HashMap<u64, multi_generated::MultiGeneratedShellPlanV1>,
    generated_submissions: HashMap<u64, multi_generated::MultiGeneratedSubmissionV1>,
    modules: HashMap<u64, RoutedHandleV1>,
    kernels: HashMap<u64, RoutedHandleV1>,
    kernel_modules: HashMap<u64, u64>,
    submissions: HashMap<u64, RoutedSubmissionV1>,
    producer_aware_native: HashMap<u64, Arc<RetainedComputeLaunchV1>>,
    events: HashMap<u64, RoutedEventV1>,
    cooperative_allocation_owners: HashMap<RoutedHandleV1, Vec<u64>>,
    cooperative_dependency_retain_counts: HashMap<u64, usize>,
    peer_launch_retains: PeerLaunchRetainsV1,
    deferred_compute_retains: deferred_compute::DeferredComputeRetainsV1,
    cooperative_stream_pending_counts: HashMap<u64, usize>,
    cooperative_stream_tails: HashMap<u64, u64>,
    native_stream_submission_counts: HashMap<u64, usize>,
    event_submission_retain_counts: HashMap<u64, usize>,
    cooperative_progress_generation: u64,
    // None preserves strict flush/drain; Some records a quantum's spent leaf.
    cooperative_progress_quantum: Option<bool>,
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
    ready_indexed: bool,
    ticket: Option<Gfx942SdmaCopyTicketV1>,
    sequence: Option<xgmi_segments::Sequence>,
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
    } else if ready > limit {
        XgmiFlushAdmissionV1::Capacity
    } else {
        XgmiFlushAdmissionV1::Publish { ready }
    }
}

fn publish_xgmi_flush_v1(
    ready: usize,
    in_flight: bool,
    publish: impl FnOnce() -> Result<
        XgmiBatchPublicationOutcomeV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    >,
) -> Result<XgmiBatchPublicationOutcomeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    match classify_xgmi_flush_v1(ready, in_flight, GFX942_SDMA_MAX_IN_FLIGHT_V1) {
        XgmiFlushAdmissionV1::NoReadyWork => Ok(XgmiBatchPublicationOutcomeV1::NoReadyWork),
        XgmiFlushAdmissionV1::InFlight => Err(KfdNativeXgmiRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::Busy,
            "native XGMI direction already has a published batch",
        )),
        XgmiFlushAdmissionV1::Capacity => Err(KfdNativeXgmiRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "native XGMI ready flush exceeds ring admission",
        )),
        XgmiFlushAdmissionV1::Publish { .. } => publish(),
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

fn index_xgmi_ready_id_v1(
    ids: &mut VecDeque<u64>,
    indexed: &mut bool,
    id: u64,
    front: bool,
) -> bool {
    if *indexed {
        return false;
    }
    if front {
        prepend_xgmi_ready_id_v1(ids, id);
    } else {
        enqueue_xgmi_ready_id_v1(ids, id);
    }
    *indexed = true;
    true
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
    ready_indexed: bool,
    in_flight: &mut Vec<u64>,
    id: u64,
) -> XgmiProgressIndexPhaseV1 {
    if remove_ordered_xgmi_id_v1(in_flight, id) {
        if ready_indexed {
            std::process::abort();
        }
        return XgmiProgressIndexPhaseV1::InFlight;
    }
    if ready_indexed {
        if !remove_xgmi_ready_id_v1(ready, id) {
            std::process::abort();
        }
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
            dependency_depth: 1,
            profile_dispatch_published: false,
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
    directed_roots: usize,
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
            && self.directed_roots == 0
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
///
/// The optional `RuntimePeerCopyBatchBackendV1` SPI accepts an explicit complete
/// ready or in-flight directional roster, retaining ordinary submission IDs.
/// Native aggregate preparation/execution unwinds are process-abort-only: they
/// cannot return resumable ownership. Terminal aggregate outcomes permanently
/// quarantine the queue, both sessions and the process-global KFD runtime gate.
#[must_use = "native XGMI backends must remain owned through quiescence"]
pub struct KfdNativeXgmiRuntimeBackendV1 {
    descriptions: [BackendDeviceDescriptionV1; 2],
    native: NativeXgmiCustodyV1,
    routes: [Gfx942XgmiRouteV1; 2],
    queue_creation_roots: [Gfx942NativeXgmiSdmaQueueCreationRootV1; 2],
    #[cfg(feature = "hardware-diagnostic")]
    xgmi_diagnostic: Option<xgmi_diagnostic::Recorder>,
    #[cfg(feature = "hardware-diagnostic")]
    xgmi_aggregate_diagnostic: Option<xgmi_batch_diagnostic::Recorder>,
    #[cfg(feature = "hardware-diagnostic")]
    xgmi_segments_diagnostic: Option<xgmi_segments_diagnostic::Recorder>,
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
    sequence_by_direction: [Option<u64>; 2],
    completion_reservations: usize,
    events: HashMap<u64, EventRecordV1>,
    event_submission_retain_counts: HashMap<u64, usize>,
    dependency_retain_counts: HashMap<u64, usize>,
    dependency_depths: HashMap<u64, usize>,
    dependency_waiters: HashMap<u64, Vec<u64>>,
    directed_roots: HashMap<u64, xgmi_directed::Root>,
    request_policy: xgmi_request::RequestPolicyV1,
}

fn settle_xgmi_queue_retirement<Q, E>(
    queues: &mut [Option<Q>; 2],
    terminal: &mut bool,
    direction: usize,
    retire: impl FnOnce(&mut Q) -> Result<(), E>,
) -> Result<(), E> {
    let Some(queue) = queues[direction].as_mut() else {
        return Ok(());
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| retire(queue))) {
        Ok(Ok(())) => {
            // Only a fully retired shell may leave the runtime's owner slot.
            queues[direction].take();
            Ok(())
        }
        Ok(Err(error)) => {
            *terminal = true;
            Err(error)
        }
        Err(payload) => {
            *terminal = true;
            std::panic::resume_unwind(payload)
        }
    }
}

fn settle_xgmi_queue_creation<R, Q, E>(
    roots: &mut [R; 2],
    queues: &mut [Option<Q>; 2],
    terminal: &mut bool,
    direction: usize,
    create: impl FnOnce(&mut R) -> Result<Q, E>,
) -> Result<(), E> {
    if queues[direction].is_some() {
        std::process::abort();
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        create(&mut roots[direction])
    })) {
        Ok(Ok(queue)) => {
            queues[direction] = Some(queue);
            Ok(())
        }
        Ok(Err(error)) => {
            // Latch before the caller formats or otherwise handles the error.
            // Preserve the runtime's conservative policy even for a lower-level
            // retryable preflight rejection with a still-vacant root.
            *terminal = true;
            Err(error)
        }
        Err(payload) => {
            *terminal = true;
            std::panic::resume_unwind(payload)
        }
    }
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
            .field("queue_creation_roots", &self.queue_creation_roots)
            .field("queues", &self.native.queue_count())
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
            .field("directed_roots", &self.directed_roots.len())
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
            .field("generated_allocations", &self.generated_allocations.len())
            .field("generated_shells", &self.generated_shells.len())
            .field("generated_submissions", &self.generated_submissions.len())
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
            .field("peer_launch_retains", &self.peer_launch_retains)
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
        Self::open_with_authorities_v1(devices, KfdRuntimeLaunchGateV1::Production, false)
    }

    /// Admits multiple devices with exact semantic launch authorities.
    pub fn open_default_with_semantic_authorities_v1(
        devices: Vec<(u64, Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_with_authorities_v1(devices, KfdRuntimeLaunchGateV1::Semantic, false)
    }

    fn open_default_with_gates_v1(
        devices: Vec<(u64, KfdRuntimeLaunchGateV1)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_default_with_gate_policy_v1(devices, false)
    }

    // Composition stays private so a caller cannot hide already-live child
    // handles behind newly empty routing tables.
    #[cfg(test)]
    fn from_backends(children: Vec<KfdRuntimeBackendV1>) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let device_children = multi_admission::reserve_device_index_v1(children.len())?;
        Self::from_backends_with_index_v1(children, device_children)
    }

    fn from_backends_with_index_v1(
        children: Vec<KfdRuntimeBackendV1>,
        mut device_children: HashMap<u64, usize>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let request_policy = multi_admission::classify_children_v1(&children)?;
        let mut compute_xgmi_children = Vec::new();
        compute_xgmi_children
            .try_reserve_exact(children.len())
            .map_err(|_| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "multi-device peer custody roster allocation failed",
                )
            })?;
        compute_xgmi_children.resize(children.len(), None);
        for (index, child) in children.iter().enumerate() {
            if child.description.backend_device == 0
                || device_children
                    .insert(child.description.backend_device, index)
                    .is_some()
            {
                return Err(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "multi-device child IDs must be nonzero and distinct",
                ));
            }
        }
        Ok(Self {
            children,
            device_children,
            request_policy,
            compute_xgmi_routes: HashMap::new(),
            compute_xgmi_children,
            completed_compute_xgmi_copies: 0,
            terminal: false,
            next_handle: 1,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            generated_allocations: HashMap::new(),
            generated_shells: HashMap::new(),
            generated_submissions: HashMap::new(),
            modules: HashMap::new(),
            kernels: HashMap::new(),
            kernel_modules: HashMap::new(),
            submissions: HashMap::new(),
            producer_aware_native: HashMap::new(),
            events: HashMap::new(),
            cooperative_allocation_owners: HashMap::new(),
            cooperative_dependency_retain_counts: HashMap::new(),
            peer_launch_retains: PeerLaunchRetainsV1::default(),
            deferred_compute_retains: deferred_compute::DeferredComputeRetainsV1::default(),
            cooperative_stream_pending_counts: HashMap::new(),
            cooperative_stream_tails: HashMap::new(),
            native_stream_submission_counts: HashMap::new(),
            event_submission_retain_counts: HashMap::new(),
            cooperative_progress_generation: 0,
            cooperative_progress_quantum: None,
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
            || !self.generated_allocations.is_empty()
            || !self.generated_shells.is_empty()
            || !self.generated_submissions.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.kernel_modules.is_empty()
            || !self.submissions.is_empty()
            || !self.producer_aware_native.is_empty()
            || !self.events.is_empty()
            || !self.cooperative_allocation_owners.is_empty()
            || !self.cooperative_dependency_retain_counts.is_empty()
            || !self.peer_launch_retains.is_empty()
            || !self.deferred_compute_retains.is_empty()
            || !self.cooperative_stream_pending_counts.is_empty()
            || !self.cooperative_stream_tails.is_empty()
            || !self.native_stream_submission_counts.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || self.compute_xgmi_children.iter().any(Option::is_some)
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
        if self.generated_allocations.contains_key(&id)
            || self.generated_shells.contains_key(&id)
            || self.generated_submissions.contains_key(&id)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device handle collides with generated custody",
            ));
        }
        self.next_handle = self.next_handle.checked_add(1).ok_or_else(|| {
            KfdRuntimeBackendV1::capacity("multi-device routing handle space exhausted")
        })?;
        Ok(id)
    }

    fn require_submission_capacity_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self
            .submissions
            .len()
            .checked_add(self.generated_submissions.len())
            .is_none_or(|count| count >= MAX_RUNTIME_SUBMISSIONS_V1)
        {
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
            RoutedEventV1::DeferredCompute {
                submission,
                child: event_child,
            } => {
                self.deferred_event_dependency_v1(submission, event_child, child)?;
                Ok(None)
            }
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
                    Some(
                        RoutedSubmissionV1::Native { .. } | RoutedSubmissionV1::DeferredCompute(_),
                    )
                    | None => {
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

    fn exact_launch_dependency_for_child(
        &self,
        dependency: BackendLaunchProducerV1,
        child: usize,
    ) -> Result<Option<BackendLaunchProducerV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let event = self.events.get(&dependency.event).copied().ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD event",
            )
        })?;
        let (event_route, event_submission) = match event {
            RoutedEventV1::Native { route, submission } => (route, submission),
            RoutedEventV1::DeferredCompute { .. } => {
                self.completed_deferred_dependency_depth_v1(dependency, child)?;
                return Ok(None);
            }
            RoutedEventV1::CooperativeCopy {
                submission,
                child: event_child,
            } => {
                if submission != dependency.producer_submission {
                    return Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "cooperative event does not name the expected launch producer",
                    ));
                }
                let Some(RoutedSubmissionV1::CooperativeCopy(copy)) =
                    self.submissions.get(&submission)
                else {
                    return Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "cooperative event does not retain its copy producer",
                    ));
                };
                if event_child != child || copy.destination.child != child {
                    return Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::WrongDevice,
                        "cooperative launch producer belongs to another destination device",
                    ));
                }
                return match copy.status() {
                    BackendPollV1::Succeeded => Ok(None),
                    BackendPollV1::Pending if copy.directed.is_some() => Ok(None),
                    BackendPollV1::Pending => Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "cooperative launch producer is pending",
                    )),
                    BackendPollV1::Failed { .. } => Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "cooperative launch producer did not succeed",
                    )),
                };
            }
        };
        if event_submission != dependency.producer_submission {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device KFD event does not name the expected producer",
            ));
        }
        if event_route.child != child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "producer-aware launch dependency belongs to another KFD device",
            ));
        }
        let producer_route = match self.submissions.get(&dependency.producer_submission) {
            Some(RoutedSubmissionV1::Native { route, .. }) => *route,
            Some(
                RoutedSubmissionV1::CooperativeCopy(_) | RoutedSubmissionV1::DeferredCompute(_),
            ) => {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "producer-aware launch requires a native KFD producer",
                ));
            }
            None => {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "multi-device KFD event refers to an unknown producer",
                ));
            }
        };
        if producer_route.child != child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "producer-aware launch producer belongs to another KFD device",
            ));
        }
        Ok(Some(BackendLaunchProducerV1 {
            event: event_route.local,
            producer_submission: producer_route.local,
        }))
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
            assert!(
                copy.sdma_leaf
                    .as_ref()
                    .is_none_or(|leaf| leaf.is_quiescent(&self.children[leaf.child()])),
                "cooperative SDMA custody remains live"
            );
            assert!(
                copy.compute_xgmi
                    .as_ref()
                    .is_none_or(|root| root.is_quiescent()),
                "compute-XGMI custody remains live"
            );
            assert!(
                [copy.source.child, copy.destination.child]
                    .into_iter()
                    .all(|child| self.compute_xgmi_children[child] != Some(submission)),
                "compute-XGMI child reservation remains live"
            );
            copy.phase = phase;
            copy.compute_producer = None;
            copy.frame_source = None;
            let staging = core::mem::take(&mut copy.staging);
            let released_staging_bytes = u64::try_from(staging.len())
                .expect("cooperative staging length was admitted as u64");
            debug_assert_eq!(released_staging_bytes, copy.staging_byte_len());
            let released_scratch = if copy.sdma_leaf.is_none() {
                core::mem::take(&mut copy.scratch_byte_len)
            } else {
                0
            };
            (
                copy.stream,
                copy.source,
                copy.destination,
                core::mem::take(&mut copy.dependencies),
                released_staging_bytes + released_scratch,
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
        if phase == CooperativeCopyPhaseV1::Cancelled {
            self.restore_cooperative_stream_tail_v1(submission);
        }
        self.note_cooperative_progress();
        status
    }

    fn note_cooperative_progress(&mut self) {
        self.cooperative_progress_generation = self.cooperative_progress_generation.wrapping_add(1);
    }

    #[cfg(test)]
    fn assert_cooperative_indexes_consistent(&self) {
        self.assert_deferred_compute_indexes_consistent_v1();
        let mut expected_allocation_owners = HashMap::<RoutedHandleV1, Vec<u64>>::new();
        let mut expected_dependency_counts = HashMap::<u64, usize>::new();
        let mut expected_stream_counts = HashMap::<u64, usize>::new();
        let mut expected_native_stream_counts = HashMap::<u64, usize>::new();
        let mut expected_staging_bytes = 0_u64;
        let mut expected_compute_xgmi_children = vec![None; self.children.len()];
        for (submission, record) in &self.submissions {
            let copy = match record {
                RoutedSubmissionV1::Native { stream, .. } => {
                    *expected_native_stream_counts.entry(*stream).or_insert(0) += 1;
                    continue;
                }
                RoutedSubmissionV1::DeferredCompute(root) => {
                    *expected_native_stream_counts
                        .entry(root.stream)
                        .or_insert(0) += 1;
                    continue;
                }
                RoutedSubmissionV1::CooperativeCopy(copy) => copy,
            };
            assert!(copy.dependency_depth <= MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1);
            if copy
                .compute_xgmi
                .as_ref()
                .is_some_and(|root| !root.is_quiescent())
            {
                assert!(!copy.is_quiescent());
                for child in [copy.source.child, copy.destination.child] {
                    assert!(
                        expected_compute_xgmi_children[child]
                            .replace(*submission)
                            .is_none()
                    );
                }
            }
            if copy.is_quiescent() {
                assert!(copy.dependencies.is_empty());
                assert!(copy.staging.is_empty());
                if let Some(leaf) = &copy.sdma_leaf {
                    assert!(leaf.is_quiescent(&self.children[leaf.child()]));
                } else {
                    assert_eq!(copy.scratch_byte_len, 0);
                }
                expected_staging_bytes += copy.scratch_byte_len;
                continue;
            }
            assert!(copy.dependency_cursor <= copy.dependencies.len());
            assert_eq!(
                u64::try_from(copy.staging.len()).unwrap(),
                copy.staging_byte_len()
            );
            expected_staging_bytes = expected_staging_bytes
                .checked_add(copy.staging_byte_len())
                .and_then(|total| total.checked_add(copy.scratch_byte_len))
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
        assert_eq!(self.compute_xgmi_children, expected_compute_xgmi_children);
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
                | RoutedEventV1::CooperativeCopy { submission, .. }
                | RoutedEventV1::DeferredCompute { submission, .. } => *submission,
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
            self.check_directed_if_present_v1(current)?;
            if matches!(
                self.submissions.get(&current),
                Some(RoutedSubmissionV1::CooperativeCopy(_))
            ) {
                self.validate_compute_peer_v1(current)?;
            }
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
            RoutedSubmissionV1::DeferredCompute(_) => {
                return self.observe_deferred_compute_v1(submission);
            }
        };
        match native_route {
            Some(route) => {
                if let Some(status) = self.compute_xgmi_stored_observation_v1(route) {
                    return self.observe_peer_launch_result_v1(submission, status, |status| {
                        *status != BackendPollV1::Pending
                    });
                }
                self.refresh_peer_launch_gate_v1(submission)?;
                self.service_native_peer_prefix_v1(route, false)?;
                let result = self.children[route.child].poll_v1(route.local);
                self.observe_peer_launch_result_v1(submission, result, |status| {
                    *status != BackendPollV1::Pending
                })
            }
            None => Ok(match &self.submissions[&submission] {
                RoutedSubmissionV1::CooperativeCopy(copy) => copy.status(),
                RoutedSubmissionV1::Native { .. } | RoutedSubmissionV1::DeferredCompute(_) => {
                    unreachable!()
                }
            }),
        }
    }

    fn fail_cooperative_copy(&mut self, submission: u64) -> BackendPollV1 {
        self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Failed)
    }

    /// Advances at most one cooperative staging or native peer transition.
    ///
    /// Submission and public observers never drive these leaves. Authoritative
    /// DeviceLocal backing uses private child SDMA copies in 64-KiB chunks;
    /// scratch and DMA custody survive every Pending observation. This ordinary
    /// entry point also propagates a failed ancestor through its selected path.
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
            let result = self.progress_selected_cooperative_copy_v1(oldest);
            if let Err(failure) = result {
                if matches!(failure, RuntimeBackendFailureV1::Quiescent(_)) {
                    // Settle the complete selected path, including intermediate
                    // copies that would otherwise be stranded behind a failed tail.
                    self.fail_cooperative_dependency_path_v1(submission, oldest)?;
                }
                return Err(failure);
            }
            return Ok(BackendPollV1::Pending);
        }
        self.progress_selected_cooperative_copy_v1(submission)
    }

    fn progress_selected_cooperative_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if matches!(self.submissions.get(&submission), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some())
        {
            return self.progress_retained_directed_peer_v1(submission);
        }
        // Native owners retain their own paired corruption/failure envelope.
        // Only a staged leg needs this additional resource-only handoff.
        if self.compute_xgmi_endpoints_v1(submission).is_none()
            && let Some(blocker) = self.directed_native_blocker_v1(submission)?
        {
            // Only an already-started owner is driven here. Its conclusive
            // result is not a success dependency of this ordinary copy.
            return match self.progress_retained_directed_peer_v1(blocker) {
                Ok(_) | Err(RuntimeBackendFailureV1::Quiescent(_)) => Ok(BackendPollV1::Pending),
                Err(error) => Err(error),
            };
        }
        self.progress_cooperative_copy_step_v1(submission)
    }

    fn progress_cooperative_copy_step_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.check_directed_if_present_v1(submission)?;
        let peer_endpoints = self.compute_xgmi_endpoints_v1(submission);
        let endpoint = match self.submissions.get(&submission) {
            Some(RoutedSubmissionV1::CooperativeCopy(copy)) => match copy.phase {
                CooperativeCopyPhaseV1::Read => Some(copy.source.child),
                CooperativeCopyPhaseV1::Write => Some(copy.destination.child),
                _ => None,
            },
            _ => None,
        };
        if endpoint.is_some() && !self.take_cooperative_progress_leaf_v1() {
            return Ok(BackendPollV1::Pending);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.progress_cooperative_copy_step_inner_v1(submission)
        }));
        match result {
            Ok(result @ Err(RuntimeBackendFailureV1::Terminal(_))) => {
                self.terminal = true;
                if let Some(endpoints) = peer_endpoints {
                    self.poison_compute_xgmi_children_v1(endpoints);
                }
                result
            }
            Ok(result) => result,
            Err(payload) => {
                self.terminal = true;
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    if let Some(endpoints) = peer_endpoints {
                        self.poison_compute_xgmi_children_v1(endpoints);
                    } else if let Some(child) = endpoint {
                        self.children[child].poison_terminal_v1();
                    }
                })
            }
        }
    }

    fn progress_cooperative_copy_step_inner_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let phase = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown cooperative copy submission",
            )
        })? {
            RoutedSubmissionV1::CooperativeCopy(copy) => copy.phase,
            RoutedSubmissionV1::Native { .. } | RoutedSubmissionV1::DeferredCompute(_) => {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "native submission routed through cooperative copy progress",
                ));
            }
        };

        self.validate_compute_peer_v1(submission)?;
        if phase == CooperativeCopyPhaseV1::Read
            && self.compute_xgmi_endpoints_v1(submission).is_some()
        {
            return self.progress_compute_xgmi_v1(submission);
        }

        if matches!(
            phase,
            CooperativeCopyPhaseV1::Read | CooperativeCopyPhaseV1::Write
        ) {
            let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission] else {
                unreachable!()
            };
            let child = if phase == CooperativeCopyPhaseV1::Read {
                copy.source.child
            } else {
                copy.destination.child
            };
            if self.compute_xgmi_child_occupied_v1(child) {
                return Ok(BackendPollV1::Pending);
            }
        }

        if matches!(
            phase,
            CooperativeCopyPhaseV1::Read | CooperativeCopyPhaseV1::Write
        ) && self.cooperative_sdma_leaf_is_selected_v1(submission)
        {
            return self.progress_cooperative_sdma_leaf_v1(submission);
        }

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
                    RoutedSubmissionV1::Native { .. } | RoutedSubmissionV1::DeferredCompute(_) => {
                        unreachable!()
                    }
                };
                if let Some(dependency) = dependency {
                    match self.progress_compute_peer_dependency_v1(submission, dependency) {
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
                        Err(error @ RuntimeBackendFailureV1::Quiescent(_)) if matches!(&self.submissions[&submission], RoutedSubmissionV1::CooperativeCopy(copy) if copy.is_quiescent()) =>
                        {
                            return Err(error);
                        }
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
                let origin = self.peer_copy_origin_v1(submission)?;
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
                    children[route.child].read_cooperative_host_range_v1(
                        route.local,
                        byte_offset,
                        &mut copy.staging[start..end],
                        origin,
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
                let origin = self.peer_copy_origin_v1(submission)?;
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
                    if origin.is_some() {
                        children[route.child].write_cooperative_host_range_with_peer_access_v1(
                            route.local,
                            byte_offset,
                            &copy.staging[start..end],
                            origin,
                        )
                    } else {
                        children[route.child].write_allocation_v1(
                            route.local,
                            byte_offset,
                            &copy.staging[start..end],
                        )
                    }
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
        self.submit_cooperative_copy_profile_v1(
            stream,
            source,
            destination,
            dependencies,
            require_distinct_devices,
            None,
        )
    }

    fn submit_cooperative_copy_profile_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
        require_distinct_devices: bool,
        directed: Option<cooperative_directed::Root>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.submit_cooperative_copy_transport_v1(
            stream,
            source,
            destination,
            dependencies,
            require_distinct_devices,
            CooperativeCopyProfileV1::Scalar(directed),
        )
    }

    fn submit_cooperative_copy_transport_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
        require_distinct_devices: bool,
        profile: CooperativeCopyProfileV1,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let (mut directed, segments) = match profile {
            CooperativeCopyProfileV1::Scalar(directed) => (directed, None),
            CooperativeCopyProfileV1::Segments(plan) => (None, Some(plan)),
        };
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
        self.require_no_deferred_stream_v1(stream)?;
        let distinct_devices = source_route.child != destination_route.child;
        if distinct_devices != require_distinct_devices
            || destination_route.child != stream_route.child
            || (segments.is_none() && source.byte_len != destination.byte_len)
            || source.byte_len == 0
            || destination.byte_len == 0
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
                "cooperative copy requires valid nonzero envelopes, scalar equal lengths, valid access, and a destination stream",
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
        let len = usize::try_from(source.byte_len)
            .map_err(|_| KfdRuntimeBackendV1::capacity("copy staging size overflow"))?;
        if directed.is_none() {
            self.admit_directed_owner_capacity_v1([source_route, destination_route], false)?;
        }
        let segment_predecessor = if segments.is_some() {
            self.prepare_segment_destination_predecessor_v1(stream, destination_route, dependencies)
        } else {
            None
        };
        let compute_producer = if directed.is_none() {
            self.prepare_compute_peer_v1(
                source_route,
                source,
                destination_route,
                destination,
                dependencies,
                (segments.as_ref(), segment_predecessor.as_ref()),
            )?
        } else {
            None
        };
        let frame_source = if directed.is_none() && compute_producer.is_none() {
            self.prepare_peer_frame_source_v1(
                [(source_route, source), (destination_route, destination)],
                dependencies,
                segments.as_ref(),
            )?
        } else {
            None
        };
        if compute_producer
            .as_ref()
            .is_some_and(|producer| !producer.orders_on_stream(stream))
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "ordered compute peer requires the predecessor on the same stream",
            ));
        }
        if (self.allocation_retained_by_deferred_compute_v1(source_route)
            && !compute_producer
                .as_ref()
                .is_some_and(|producer| producer.reserves_deferred_source(self, source_route)))
            || self.allocation_retained_by_deferred_compute_v1(destination_route)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "copy endpoint is retained by an unrelated deferred compute consumer",
            ));
        }
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
            let dependency = match &compute_producer {
                Some(producer) if producer.deferred_event(self, *event) => producer.id,
                _ => self.peer_dependency_submission(
                    *event,
                    source_route.child,
                    destination_route.child,
                )?,
            };
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
                && (!copy.is_quiescent() || directed.is_some())
            {
                dependency_depth = dependency_depth.max(
                    copy.dependency_depth.checked_add(1).ok_or_else(|| {
                        KfdRuntimeBackendV1::capacity("cooperative copy dependency depth overflow")
                    })?,
                );
            }
            // A whole-frame D2H is a provenance consumer, unlike an ordinary
            // transfer control. Keep its rank separate from legacy copy ordering.
            if self.children[destination_route.child].allocations[&destination_route.local].kind
                == RuntimeMemoryKindV1::HostVisible
                && let Some(frame) = self.compute_peer_segment_frame_v1(*dependency, source_route)
            {
                dependency_depth =
                    dependency_depth.max(frame.depth().checked_add(1).ok_or_else(|| {
                        KfdRuntimeBackendV1::capacity("segment frame readback depth overflow")
                    })?);
            }
        }
        if let Some(producer) = &compute_producer {
            dependency_depth =
                dependency_depth.max(producer.depth().checked_add(1).ok_or_else(|| {
                    KfdRuntimeBackendV1::capacity("compute peer dependency depth overflow")
                })?);
            for dependency in &dependency_submissions {
                let depth = self
                    .compute_peer_dependency_depth_v1(*dependency)
                    .ok_or_else(|| {
                        KfdRuntimeBackendV1::rejected(
                            KfdRuntimeBackendErrorKindV1::Unsupported,
                            "compute peer control dependency has no retained depth",
                        )
                    })?;
                dependency_depth = dependency_depth.max(depth.checked_add(1).ok_or_else(|| {
                    KfdRuntimeBackendV1::capacity("compute peer control dependency depth overflow")
                })?);
            }
        }
        if let Some(frame) = &frame_source {
            if !frame.controls_succeeded(self, &dependency_submissions) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "frame peer requires already successful unrelated controls",
                ));
            }
            for dependency in &dependency_submissions {
                let depth = if *dependency == frame.id() {
                    frame.depth()
                } else {
                    self.segment_frame_dependency_depth_v1(*dependency)
                        .ok_or_else(|| {
                            KfdRuntimeBackendV1::rejected(
                                KfdRuntimeBackendErrorKindV1::Unsupported,
                                "frame peer control has no retained dependency rank",
                            )
                        })?
                };
                dependency_depth = dependency_depth.max(depth.checked_add(1).ok_or_else(|| {
                    KfdRuntimeBackendV1::capacity("frame peer dependency depth overflow")
                })?);
            }
        }
        // A legacy settled list can retain completed controls whose historical
        // depth was not part of its transfer-only rank. Preserve that acceptance
        // if the stricter frame-consumer rank cannot be represented.
        let segment_frame_depth = if segments.is_some() {
            dependency_submissions
                .iter()
                .try_fold(dependency_depth, |depth, id| {
                    let parent = self.segment_frame_dependency_depth_v1(*id)?;
                    (parent > 0).then_some(())?;
                    Some(depth.max(parent.checked_add(1)?))
                })
                .filter(|depth| *depth <= MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1)
        } else {
            None
        };
        if dependency_depth > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "cooperative copy dependency depth exceeds its admitted bound",
            ));
        }
        if segments.is_some()
            && (compute_producer.is_some() || frame_source.is_some())
            && segment_frame_depth.is_none()
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "compute-backed segment frame dependency depth exceeds its admitted bound",
            ));
        }
        let readback_frame = self.compute_peer_readback_frame_v1(
            source_route,
            source,
            destination_route,
            destination,
            &dependency_submissions,
        );
        let segment_readback_frame =
            if self.children[destination_route.child].allocations[&destination_route.local].kind
                == RuntimeMemoryKindV1::HostVisible
            {
                dependency_submissions.iter().find_map(|id| {
                    self.compute_peer_segment_frame_v1(*id, source_route)
                        .filter(|frame| frame.covers(self, source))
                        .map(|frame| (*id, frame))
                })
            } else {
                None
            };
        let source_dependencies_complete = self
            .cooperative_allocation_owners
            .get(&source_route)
            .is_none_or(|owners| {
                owners.iter().all(|owner| {
                    dependency_set.contains(owner)
                        || frame_source.as_ref().is_some_and(|frame| {
                            frame.orders_owner(self, source_route, *owner)
                        })
                        || readback_frame.is_some_and(|producer| {
                            producer.orders_destination_owner(self, *owner)
                        })
                        || segment_readback_frame.as_ref().is_some_and(|(id, frame)| {
                            frame.orders_owner(self, *id, source_route, *owner)
                        })
                        || matches!(
                            self.submissions.get(owner),
                            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                                if copy.stream == stream || directed.as_ref().is_some_and(|root| root.shares_read_source(copy, source_route))
                        )
                })
            });
        let destination_dependencies_complete = self
            .cooperative_allocation_owners
            .get(&destination_route)
            .is_none_or(|owners| {
                owners.iter().all(|owner| {
                    dependency_set.contains(owner)
                        || compute_producer.as_ref().is_some_and(|producer| {
                            producer.orders_destination_owner(self, *owner)
                        })
                        || segment_predecessor.as_ref().is_some_and(|prior| {
                            prior.orders_owner(self, destination_route, *owner)
                        })
                        || matches!(
                            self.submissions.get(owner),
                            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                                if copy.stream == stream || directed.as_ref().is_some_and(|root| root.shares_read_source(copy, destination_route))
                        )
                })
            });
        if !source_dependencies_complete || !destination_dependencies_complete {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping cooperative copies require an explicit dependency",
            ));
        }

        for route in [source_route, destination_route] {
            if !compute_producer
                .as_ref()
                .is_some_and(|producer| producer.owns_source(self, route))
                && !self.cooperative_native_custody_is_ordered_v1(
                    route,
                    stream,
                    &dependency_set,
                    directed.as_ref(),
                )
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "cooperative copy allocation has unrelated native custody",
                ));
            }
        }

        let mut compute_xgmi = if let Some(plan) = segments {
            Some(self.prepare_compute_xgmi_segments_v1(
                source_route,
                source,
                destination_route,
                destination,
                plan,
                (
                    compute_producer.as_ref(),
                    segment_predecessor.as_ref(),
                    frame_source.as_ref(),
                ),
            )?)
        } else if let Some(producer) = &compute_producer {
            Some(self.prepare_compute_xgmi_plan_v1(
                source_route,
                destination_route,
                producer.window(),
            )?)
        } else if let Some(frame) = &frame_source {
            Some(self.prepare_compute_xgmi_plan_v1(
                source_route,
                destination_route,
                frame.window(),
            )?)
        } else {
            self.prepare_compute_xgmi_v1(
                source_route,
                source,
                destination_route,
                destination,
                directed.is_some(),
            )?
        };
        if let Some(root) = &mut compute_xgmi {
            let frame = if compute_producer.is_some() || segment_frame_depth.is_some() {
                self.prepare_segment_destination_frame_v1(
                    root,
                    (
                        stream,
                        dependency_depth,
                        segment_frame_depth.unwrap_or(dependency_depth),
                    ),
                    [(source_route, source), (destination_route, destination)],
                    compute_producer.as_ref(),
                    segment_predecessor,
                    frame_source.as_ref(),
                )
            } else {
                None
            };
            root.bind_segment_frame_v1(frame);
        }
        let staging_byte_len = if compute_xgmi.is_some() {
            0
        } else {
            source.byte_len
        };
        let scratch_byte_len = if compute_xgmi.is_none()
            && [source_route, destination_route].into_iter().any(|route| {
                let child = &self.children[route.child];
                child.native_available
            }) {
            source.byte_len.min(COOPERATIVE_COPY_CHUNK_BYTES_V1 as u64)
        } else {
            0
        };

        let next_cooperative_staging_bytes = self
            .cooperative_staging_bytes
            .checked_add(staging_byte_len)
            .and_then(|total| total.checked_add(scratch_byte_len))
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
        let staging = try_zeroed_staging_v1(if compute_xgmi.is_some() { 0 } else { len })?;
        let copy_shell = try_uninit_box_v1().map_err(|()| {
            KfdRuntimeBackendV1::capacity("cooperative copy owner allocation failed")
        })?;
        let id = self.next_id()?;

        if let Some(root) = &mut directed {
            root.submission = id;
            root.depth = dependency_depth;
            root.prior_stream_submission = stream_tail;
        }

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
            RoutedSubmissionV1::CooperativeCopy(Box::write(
                copy_shell,
                CooperativeCopySubmissionV1 {
                    directed,
                    compute_xgmi,
                    compute_producer,
                    frame_source,
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
                    scratch_byte_len,
                    sdma_leaf: None,
                    phase: CooperativeCopyPhaseV1::Dependencies,
                    byte_cursor: 0,
                },
            )),
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
        Self::open_default_with_backing_budgets_v1(
            first_unique_id,
            second_unique_id,
            [KfdNativeXgmiBackingBudgetV1::default(); 2],
        )
    }

    /// Opens two exact endpoints with independent immutable backing budgets.
    /// Array order is the argument order, never topology or numeric-ID order.
    pub fn open_default_with_backing_budgets_v1(
        first_unique_id: u64,
        second_unique_id: u64,
        budgets: [KfdNativeXgmiBackingBudgetV1; 2],
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
        Self::from_checked_pair_with_backing_budgets_v1(first, second, budgets)
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
        Self::from_checked_pair_with_backing_budgets_v1(
            first,
            second,
            [KfdNativeXgmiBackingBudgetV1::default(); 2],
        )
    }

    /// Builds the copy-only owner with budgets fixed before either endpoint's
    /// first allocation. Failure after acquiring the first VM is fail-stop.
    pub fn from_checked_pair_with_backing_budgets_v1(
        first: CheckedGfx942XnackMinusDevice,
        second: CheckedGfx942XnackMinusDevice,
        budgets: [KfdNativeXgmiBackingBudgetV1; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::from_checked_pair_with_admissions_v1(first, second, |_| {
            Ok(budgets.map(xgmi_budget::EndpointAdmissionV1::Local))
        })
    }

    /// Root admission for both ordered endpoints precedes either VM acquisition.
    pub fn open_default_with_native_backing_root_v1(
        first_unique_id: u64,
        second_unique_id: u64,
        root: &fe2o3_kfd::Gfx942NativeBackingRootV1,
        device_budgets: [fe2o3_kfd::Gfx942NativeBackingDeviceBudgetV1; 2],
        session_budgets: [fe2o3_kfd::Gfx942NativeBackingSessionBudgetV1; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if admit_xgmi_unique_id_pair_v1(first_unique_id, second_unique_id).is_err() {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI requires two distinct nonzero unique IDs",
            ));
        }
        let first = KfdRuntimeBackendV1::open_checked_device_v1(first_unique_id)?;
        let second = KfdRuntimeBackendV1::open_checked_device_v1(second_unique_id)?;
        Self::from_checked_pair_with_native_backing_root_v1(
            first,
            second,
            root,
            device_budgets,
            session_budgets,
        )
    }

    pub fn from_checked_pair_with_native_backing_root_v1(
        first: CheckedGfx942XnackMinusDevice,
        second: CheckedGfx942XnackMinusDevice,
        root: &fe2o3_kfd::Gfx942NativeBackingRootV1,
        device_budgets: [fe2o3_kfd::Gfx942NativeBackingDeviceBudgetV1; 2],
        session_budgets: [fe2o3_kfd::Gfx942NativeBackingSessionBudgetV1; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::from_checked_pair_with_admissions_v1(first, second, |devices| {
            xgmi_budget::admit_endpoints(
                devices,
                [
                    (device_budgets[0], session_budgets[0]),
                    (device_budgets[1], session_budgets[1]),
                ],
                |device, (parent, session)| {
                    root.admit_session_v1(device, parent, session)
                        .map(xgmi_budget::EndpointAdmissionV1::Native)
                        .map_err(native_budget::rooted_host_backing_admission_error_v1)
                },
            )
        })
    }

    fn from_checked_pair_with_admissions_v1(
        first: CheckedGfx942XnackMinusDevice,
        second: CheckedGfx942XnackMinusDevice,
        prepare: impl FnOnce(
            [&CheckedGfx942XnackMinusDevice; 2],
        )
            -> Result<[xgmi_budget::EndpointAdmissionV1; 2], KfdRuntimeBackendErrorV1>,
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
        let admissions = prepare([&first, &second])?;
        let (request_policy, sessions) = xgmi_budget::bind_before_acquire(
            [first, second],
            admissions,
            xgmi_request::RequestPolicyV1::from_admissions,
            |device, admission| {
                let result = match admission {
                    xgmi_budget::EndpointAdmissionV1::Local(budget) => device
                        .acquire_shared_gtt_memory_session_with_backing_budgets_v1(
                            budget.device,
                            budget.host_visible,
                        ),
                    xgmi_budget::EndpointAdmissionV1::Native(admission) => device
                        .acquire_shared_gtt_memory_session_with_rooted_native_backing_v1(admission),
                    xgmi_budget::EndpointAdmissionV1::Composed(admission) => {
                        device.acquire_shared_gtt_memory_session_with_composed_backing_v1(admission)
                    }
                };
                result.map_err(|error| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("first XGMI VM acquisition: {error}"),
                    )
                })
            },
        )?;
        Ok(Self {
            descriptions,
            native: NativeXgmiCustodyV1::new(sessions),
            routes: [forward, reverse],
            queue_creation_roots: [
                Gfx942NativeXgmiSdmaQueueCreationRootV1::new(),
                Gfx942NativeXgmiSdmaQueueCreationRootV1::new(),
            ],
            #[cfg(feature = "hardware-diagnostic")]
            xgmi_diagnostic: None,
            #[cfg(feature = "hardware-diagnostic")]
            xgmi_aggregate_diagnostic: None,
            #[cfg(feature = "hardware-diagnostic")]
            xgmi_segments_diagnostic: None,
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
            sequence_by_direction: [None, None],
            completion_reservations: 0,
            events: HashMap::new(),
            event_submission_retain_counts: HashMap::new(),
            dependency_retain_counts: HashMap::new(),
            dependency_depths: HashMap::new(),
            dependency_waiters: HashMap::new(),
            directed_roots: HashMap::new(),
            request_policy,
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

    fn require_healthy_xgmi_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.terminal
            || self
                .queue_creation_roots
                .iter()
                .any(|root| !root.is_vacant())
            || self.native.is_terminal()
        {
            return Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "native XGMI backend is terminal",
                ),
            ));
        }
        Ok(())
    }

    fn require_live(&self) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_healthy_xgmi_v1()?;
        self.native.full()?;
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
        self.require_live()?;
        if self.native.queues()?[direction].is_some() {
            return Ok(());
        }
        let route = self.routes[direction];
        let (sessions, queues) = self.native.parts_mut()?;
        let (source, destination) = Self::session_pair(sessions, direction);
        settle_xgmi_queue_creation(
            &mut self.queue_creation_roots,
            queues,
            &mut self.terminal,
            direction,
            |root| Gfx942NativeXgmiSdmaQueueV1::create(source, destination, route, root),
        )
        .map_err(|error| self.terminal_error(format!("XGMI queue creation: {error}")))
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
            let (first, second) = self.native.sessions_mut()?.split_at_mut(1);
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
            let (first, second) = self.native.sessions_mut()?.split_at_mut(1);
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
        if active.sequence.is_some()
            && self.sequence_by_direction[active.direction].take() != Some(active.id)
        {
            std::process::abort();
        }
        remove_xgmi_progress_index_v1(
            &mut self.ready_by_direction[active.direction],
            active.ready_indexed,
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
                self.index_ready_v1(active.direction, active.id, false);
            }
        }
    }

    fn index_ready_v1(&mut self, direction: usize, id: u64, front: bool) {
        index_xgmi_ready_id_v1(
            &mut self.ready_by_direction[direction],
            &mut self
                .active
                .get_mut(&id)
                .expect("indexed XGMI owner")
                .ready_indexed,
            id,
            front,
        );
    }

    fn take_ready_membership_v1(&mut self, id: u64) {
        if !core::mem::replace(
            &mut self
                .active
                .get_mut(&id)
                .expect("indexed XGMI owner")
                .ready_indexed,
            false,
        ) {
            std::process::abort();
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
        for (mut active, request) in active_batch.into_iter().zip(requests).rev() {
            let (source, destination) = request.into_mappings();
            self.restore_mapped_copy_pair(
                active.source,
                active.destination,
                active.direction,
                source,
                destination,
            )?;
            index_xgmi_ready_id_v1(
                &mut self.ready_by_direction[active.direction],
                &mut active.ready_indexed,
                active.id,
                true,
            );
            self.active.insert(active.id, active);
        }
        Ok(())
    }

    /// Publishes an allocation-disjoint FIFO prefix of at most 63. Pairwise
    /// selection is bounded by that window, independent of the active backlog.
    /// Exact flushes reject shared mappings before any native effect.
    fn publish_ready_peer_batch(
        &mut self,
        direction: usize,
        complete_ready_set: bool,
    ) -> Result<XgmiBatchPublicationOutcomeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        if self.sequence_by_direction[direction].is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "ordered XGMI copy requires sequence progress",
            ));
        }
        if !self.in_flight_by_direction[direction].is_empty() {
            return Ok(XgmiBatchPublicationOutcomeV1::AlreadyInFlight);
        }
        let batch_len = xgmi_progress::publication_len(
            direction,
            &self.ready_by_direction[direction],
            &self.active,
            &self.submissions,
            complete_ready_set,
            |left, right, allocation| xgmi_directed::shared_read(self, left, right, allocation),
        )
        .map_err(|error| match error {
            xgmi_progress::PrefixError::Corrupt => {
                self.terminal_error("native XGMI ready mapping custody is inconsistent")
            }
            xgmi_progress::PrefixError::Shared => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI ready flush requires disjoint allocation mappings",
            ),
        })?;
        if batch_len == 0 {
            return Ok(XgmiBatchPublicationOutcomeV1::NoReadyWork);
        }
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_aggregate_diagnostic.as_mut() {
            recorder.invalidate();
        }
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_segments_diagnostic.as_mut() {
            recorder.invalidate();
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
            self.take_ready_membership_v1(id);
            let mut active = self
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
                            index_xgmi_ready_id_v1(
                                &mut self.ready_by_direction[active.direction],
                                &mut active.ready_indexed,
                                active.id,
                                false,
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
                            index_xgmi_ready_id_v1(
                                &mut self.ready_by_direction[active.direction],
                                &mut active.ready_indexed,
                                active.id,
                                false,
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
        #[cfg(feature = "hardware-diagnostic")]
        let diagnostic_id = xgmi_diagnostic::CallIdentity {
            direction,
            submission: active_batch[0].id,
            submit: true,
        };
        #[cfg(feature = "hardware-diagnostic")]
        let profile = self
            .xgmi_diagnostic
            .as_mut()
            .is_some_and(|recorder| recorder.begin(diagnostic_id, batch_len));
        #[cfg(feature = "hardware-diagnostic")]
        let mut diagnostic = None;
        let result = {
            let (sessions, queues) = self.native.parts_mut()?;
            let (source_session, destination_session) = Self::session_pair(sessions, direction);
            let queue = queues[direction]
                .as_mut()
                .expect("directional XGMI queue was established");
            #[cfg(feature = "hardware-diagnostic")]
            let result = if profile {
                queue
                    .submit_batch_diagnostic_v1(source_session, destination_session, requests)
                    .map(|(tickets, timing)| {
                        diagnostic = Some(timing);
                        tickets
                    })
            } else {
                queue.submit_batch(source_session, destination_session, requests)
            };
            #[cfg(not(feature = "hardware-diagnostic"))]
            let result = queue.submit_batch(source_session, destination_session, requests);
            result
        };
        let outcome = (|| {
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
        })();
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_diagnostic.as_mut() {
            let observed = if matches!(outcome, Ok(XgmiBatchPublicationOutcomeV1::Published)) {
                diagnostic.map(|timing| (KfdRuntimeXgmiDiagnosticCallV1::Submit, timing))
            } else {
                None
            };
            recorder.finish(diagnostic_id, observed);
        }
        outcome
    }

    fn progress_peer_copy(
        &mut self,
        mut active: XgmiRuntimeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_segments_diagnostic.as_mut() {
            recorder.invalidate();
        }
        if let Some(ticket) = active.ticket.take() {
            #[cfg(feature = "hardware-diagnostic")]
            if let Some(recorder) = self.xgmi_aggregate_diagnostic.as_mut() {
                recorder.invalidate();
            }
            #[cfg(feature = "hardware-diagnostic")]
            let diagnostic_id = xgmi_diagnostic::CallIdentity {
                direction: active.direction,
                submission: active.id,
                submit: false,
            };
            #[cfg(feature = "hardware-diagnostic")]
            let profile = self
                .xgmi_diagnostic
                .as_mut()
                .is_some_and(|recorder| recorder.begin(diagnostic_id, 1));
            #[cfg(feature = "hardware-diagnostic")]
            let mut diagnostic = None;
            let result = {
                let (sessions, queues) = self.native.parts_mut()?;
                let (source_session, destination_session) =
                    Self::session_pair(sessions, active.direction);
                let queue = queues[active.direction]
                    .as_mut()
                    .expect("published XGMI copy retains queue");
                #[cfg(feature = "hardware-diagnostic")]
                let result = if profile {
                    queue
                        .poll_diagnostic_v1(source_session, destination_session, ticket)
                        .map(|(poll, timing)| {
                            diagnostic = Some(timing);
                            poll
                        })
                } else {
                    queue.poll(source_session, destination_session, ticket)
                };
                #[cfg(not(feature = "hardware-diagnostic"))]
                let result = queue.poll(source_session, destination_session, ticket);
                result
            };
            let outcome = (|| match result {
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
            })();
            #[cfg(feature = "hardware-diagnostic")]
            if let Some(recorder) = self.xgmi_diagnostic.as_mut() {
                let observed =
                    match outcome {
                        Ok(BackendPollV1::Pending) => diagnostic
                            .map(|timing| (KfdRuntimeXgmiDiagnosticCallV1::Pending, timing)),
                        Ok(BackendPollV1::Succeeded) => diagnostic
                            .map(|timing| (KfdRuntimeXgmiDiagnosticCallV1::Completed, timing)),
                        _ => None,
                    };
                recorder.finish(diagnostic_id, observed);
            }
            return outcome;
        }
        // Unpublished dependency progress belongs to the non-consuming scalar
        // driver. Neither observation nor retry may enqueue this owner again.
        self.active.insert(active.id, active);
        Ok(BackendPollV1::Pending)
    }

    fn logical_resource_counts(&self) -> XgmiLogicalResourceCountsV1 {
        XgmiLogicalResourceCountsV1 {
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
            directed_roots: self.directed_roots.len(),
        }
    }

    /// Destroys both directional queues after every logical handle is released.
    pub fn shutdown_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.sequence_by_direction.iter().any(Option::is_some)
            || !self.logical_resource_counts().permits_shutdown()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI logical resources remain live",
            ));
        }
        for direction in (0..2).rev() {
            let (sessions, queues) = self.native.parts_mut()?;
            settle_xgmi_queue_retirement(queues, &mut self.terminal, direction, |queue| {
                let (source, destination) = Self::session_pair(sessions, direction);
                queue.destroy_and_release(source, destination)
            })
            .map_err(|error| self.terminal_error(format!("XGMI queue teardown: {error}")))?;
        }
        self.shutdown = true;
        Ok(())
    }
}

impl RuntimeBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn observe_peer_copy_placement_v1(
        &self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> Option<crate::BackendPeerCopyPlacementV1> {
        self.observe_native_peer_placement_v1(stream, source, destination)
    }

    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<crate::RuntimeAllocationAdmissionProfileV1, RuntimeBackendFailureV1<Self::Error>>
    {
        // Policy remains observable after clean shutdown; it grants no native authority.
        self.require_healthy_xgmi_v1()?;
        self.request_policy.profile(
            self.descriptions
                .each_ref()
                .map(|entry| entry.backend_device),
        )
    }

    fn allocate_with_request_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
        witness: crate::RuntimeAllocationRequestWitnessV1<'_>,
    ) -> crate::RuntimeRequestAllocationResultV1<Self::Error> {
        crate::RuntimeRequestAllocationResultV1::Outcome(
            self.allocate_xgmi_request_v1(device, kind, byte_len, alignment, Some(witness))
                .map(RuntimeBackendAllocationOutcomeV1::Allocated),
        )
    }

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
        self.allocate_xgmi_request_v1(device, kind, byte_len, alignment, None)
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
        if let Err(error) = self.native.sessions_mut()?[device].release_gfx942_device_memory(lease)
        {
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
                Some(XgmiAllocationAuthorityV1::Unmapped(lease)) => self.native.sessions_mut()?
                    [device]
                    .read_gfx942_xgmi_device_memory(lease)
                    .map_err(|error| {
                        self.terminal_error(format!("XGMI write read-modify: {error}"))
                    })?,
                _ => {
                    return Err(self.terminal_error(
                        "XGMI allocation authority unavailable after successful unmap",
                    ));
                }
            }
        };
        full[byte_offset as usize..end as usize].copy_from_slice(bytes);
        let lease = match self.allocations[&allocation].authority.as_ref() {
            Some(XgmiAllocationAuthorityV1::Unmapped(lease)) => lease,
            _ => unreachable!("validated unmapped authority"),
        };
        self.native.sessions_mut()?[device]
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
            return Err(
                self.terminal_error("XGMI allocation authority unavailable after successful unmap")
            );
        };
        let bytes = self.native.sessions_mut()?[device]
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
        if active.sequence.is_some() {
            return self.progress_peer_segments(
                submission,
                Instant::now(),
                xgmi_segments::Progress::Poll,
            );
        }
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_segments_diagnostic.as_mut() {
            recorder.invalidate();
        }
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
        if self
            .active
            .get(&submission)
            .is_some_and(|active| active.sequence.is_some())
        {
            return wait_with_deadline_v1(deadline, || {
                self.progress_peer_segments(submission, deadline, xgmi_segments::Progress::Wait)
            });
        }
        wait_with_deadline_v1(deadline, || self.poll_v1(submission))
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        xgmi_directed::release_submission(self, submission)
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
        if self.sequence_by_direction[direction].is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "ordered XGMI copy retains the directional submission domain",
            ));
        }
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
        xgmi_directed::admit_scalar_owners(
            self,
            stream,
            direction,
            source,
            destination,
            dependencies,
            &dependency_submissions,
        )
        .map_err(|error| match error {
            xgmi_directed::OwnerError::Busy => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping XGMI copies require dependency",
            ),
            xgmi_directed::OwnerError::Capacity => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI allocation custody capacity exceeded",
            ),
            xgmi_directed::OwnerError::Corrupt => {
                self.terminal_error("native XGMI allocation-owner custody is inconsistent")
            }
        })?;
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
        let mut active = XgmiRuntimeSubmissionV1 {
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
            ready_indexed: false,
            ticket: None,
            sequence: None,
        };
        if xgmi_submission_is_ready_v1(&active, &self.submissions, direction) {
            index_xgmi_ready_id_v1(
                &mut self.ready_by_direction[direction],
                &mut active.ready_indexed,
                id,
                false,
            );
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
        if let Some(id) = self.sequence_by_direction[direction] {
            return match self.progress_peer_segments(
                id,
                Instant::now(),
                xgmi_segments::Progress::Flush,
            )? {
                BackendPollV1::Pending | BackendPollV1::Succeeded => Ok(()),
                BackendPollV1::Failed { .. } => Err(Self::quiescent_error(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "ordered XGMI flush completed with failure",
                )),
            };
        }
        #[cfg(feature = "hardware-diagnostic")]
        if self.active_by_direction[direction] != 0
            && let Some(recorder) = self.xgmi_segments_diagnostic.as_mut()
        {
            recorder.invalidate();
        }
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
        match publish_xgmi_flush_v1(
            ready_at_entry,
            !self.in_flight_by_direction[direction].is_empty(),
            || self.publish_ready_peer_batch(direction, true),
        )? {
            XgmiBatchPublicationOutcomeV1::NoReadyWork if ready_at_entry == 0 => Ok(()),
            XgmiBatchPublicationOutcomeV1::Published => Ok(()),
            XgmiBatchPublicationOutcomeV1::RecoveredPrepublicationFailure => {
                Err(Self::quiescent_error(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "native XGMI flush recovered a prepublication failure",
                ))
            }
            XgmiBatchPublicationOutcomeV1::NoReadyWork
            | XgmiBatchPublicationOutcomeV1::AlreadyInFlight => Err(self
                .terminal_error("native XGMI flush admission changed without concurrent access")),
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
            self.active.get(&submission).map(|active| {
                active.ticket.is_some()
                    || active
                        .sequence
                        .as_ref()
                        .is_some_and(xgmi_segments::Sequence::ever_published)
            }),
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
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_segments_diagnostic.as_mut() {
            recorder.invalidate();
        }
        let mut active = self
            .active
            .remove(&submission)
            .expect("prepublication XGMI submission remains active");
        if let Some(sequence) = active.sequence.as_mut() {
            sequence.cancel_before_publication();
        }
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
            || !self.native.is_full()
            || self
                .queue_creation_roots
                .iter()
                .any(|root| !root.is_vacant())
            || self.native.is_terminal()
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.submissions.is_empty()
            || !self.active.is_empty()
            || self.sequence_by_direction.iter().any(Option::is_some)
            || !self.active_stream_owners.is_empty()
            || !self.active_allocation_owners.is_empty()
            || !self.events.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || !self.dependency_retain_counts.is_empty()
            || !self.dependency_depths.is_empty()
            || !self.dependency_waiters.is_empty()
            || !self.directed_roots.is_empty()
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
            // Never let Drop unwind into field destruction: native storage keeps
            // sessions before queues, rooted until retirement or the abort.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (sessions, queues) = self
                    .native
                    .parts_mut()
                    .unwrap_or_else(|_| std::process::abort());
                settle_xgmi_queue_retirement(queues, &mut self.terminal, direction, |queue| {
                    let (source, destination) = Self::session_pair(sessions, direction);
                    queue.destroy_and_release(source, destination)
                })
            }));
            if !matches!(result, Ok(Ok(()))) {
                std::process::abort();
            }
        }
    }
}

impl RuntimeBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    type Error = KfdRuntimeBackendErrorV1;

    fn observe_peer_copy_placement_v1(
        &self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> Option<crate::BackendPeerCopyPlacementV1> {
        self.observe_multi_peer_placement_v1(stream, source, destination)
    }

    fn capture_coherent_host_range_v1(
        &mut self,
        request: crate::BackendHostCaptureV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<crate::RuntimeHostCaptureErrorV1>> {
        self.capture_coherent_host_range_impl_v1(request)
    }

    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<crate::RuntimeAllocationAdmissionProfileV1, RuntimeBackendFailureV1<Self::Error>>
    {
        self.request_profile_v1()
    }

    fn allocate_with_request_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
        witness: crate::RuntimeAllocationRequestWitnessV1<'_>,
    ) -> crate::RuntimeRequestAllocationResultV1<Self::Error> {
        self.allocate_requested_v1(device, kind, byte_len, alignment, witness)
    }

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
        self.require_no_deferred_stream_v1(stream)?;
        if self.cooperative_stream_pending_counts.contains_key(&stream) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream retains a pending cooperative copy",
            ));
        }
        let route = Self::route(&self.streams, stream, "unknown multi-device KFD stream")?;
        if let Some(tail) = self.cooperative_stream_tails.get(&stream).copied() {
            self.check_directed_if_present_v1(tail)?;
            let intact = match self.submissions.get(&tail) {
                Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.stream == stream => {
                    if !copy.is_quiescent() {
                        return Err(KfdRuntimeBackendV1::rejected(
                            KfdRuntimeBackendErrorKindV1::Busy,
                            "stream retains a pending cooperative copy",
                        ));
                    }
                    copy.dependencies.is_empty()
                        && copy.staging.is_empty()
                        && copy.sdma_leaf.as_ref().is_none_or(|leaf| {
                            self.children
                                .get(leaf.child())
                                .is_some_and(|child| leaf.is_quiescent(child))
                        })
                }
                _ => false,
            };
            if !intact {
                self.terminal = true;
                return Err(RuntimeBackendFailureV1::Terminal(
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Terminal,
                        "cooperative stream tail lost quiescent custody",
                    ),
                ));
            }
        }
        let result = self.children[route.child].destroy_stream_v1(route.local);
        self.latch(result)?;
        // Context destroys streams before releasing retained results and events.
        self.cooperative_stream_tails.remove(&stream);
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
        match self.allocate_with_outcome_v1(device, kind, byte_len, alignment)? {
            RuntimeBackendAllocationOutcomeV1::Allocated(handle) => Ok(handle),
            RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error) => {
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
        }
    }

    fn allocate_with_outcome_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<RuntimeBackendAllocationOutcomeV1<Self::Error>, RuntimeBackendFailureV1<Self::Error>>
    {
        self.require_live()?;
        let child = self.child_for_device(device)?;
        if self.request_policy == multi_admission::MultiRequestPolicyV1::Required {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "composed multi-device allocation requires a Context request witness",
            ));
        }
        match self.route_allocation_v1(child, |backend| {
            crate::RuntimeRequestAllocationResultV1::Outcome(
                backend.allocate_with_outcome_v1(device, kind, byte_len, alignment),
            )
        }) {
            crate::RuntimeRequestAllocationResultV1::Outcome(result) => result,
            crate::RuntimeRequestAllocationResultV1::Unsupported => {
                unreachable!("legacy outcome hook")
            }
        }
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
        if self.allocation_retained_by_router_v1(route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending cooperative copy",
            ));
        }
        self.require_compute_xgmi_child_available_v1(route.child)?;
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
        if self.allocation_retained_by_router_v1(route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending cooperative copy",
            ));
        }
        self.require_compute_xgmi_child_available_v1(route.child)?;
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
        if self.allocation_retained_by_router_v1(route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation is retained by a pending cooperative copy",
            ));
        }
        self.require_compute_xgmi_child_available_v1(route.child)?;
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
        if self.deferred_compute_retains.modules.contains_key(&route) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "module is retained by a deferred compute consumer",
            ));
        }
        self.require_compute_xgmi_child_available_v1(route.child)?;
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
        self.require_no_deferred_stream_v1(launch.stream)?;
        self.require_compute_xgmi_child_available_v1(stream.child)?;
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
            if self.allocation_retained_by_router_v1(allocation) {
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
        if self.generated_submissions.contains_key(&submission) {
            return self.poll_generated_submission_v1(submission);
        }
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
            RoutedSubmissionV1::DeferredCompute(_) => {
                return self.observe_deferred_compute_v1(submission);
            }
        };
        match native_route {
            Some(route) => {
                if let Some(status) = self.compute_xgmi_stored_observation_v1(route) {
                    return self.observe_peer_launch_result_v1(submission, status, |status| {
                        *status != BackendPollV1::Pending
                    });
                }
                self.refresh_peer_launch_gate_v1(submission)?;
                self.service_native_peer_prefix_v1(route, false)?;
                let result = self.children[route.child].poll_v1(route.local);
                self.observe_peer_launch_result_v1(submission, result, |status| {
                    *status != BackendPollV1::Pending
                })
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
        if self.generated_submissions.contains_key(&submission) {
            return wait_with_deadline_v1(deadline, || {
                self.poll_generated_submission_v1(submission)
            });
        }
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::CooperativeCopy(_) => None,
            RoutedSubmissionV1::DeferredCompute(_) => {
                return self.drain_deferred_compute_v1(submission, deadline, false);
            }
        };
        match native_route {
            Some(route) => {
                if let Some(status) = self.compute_xgmi_stored_observation_v1(route) {
                    return self.observe_peer_launch_result_v1(submission, status, |status| {
                        *status != BackendPollV1::Pending
                    });
                }
                self.refresh_peer_launch_gate_v1(submission)?;
                self.service_native_peer_prefix_v1(route, false)?;
                let result = self.children[route.child].wait_v1(route.local, deadline);
                self.observe_peer_launch_result_v1(submission, result, |status| {
                    *status != BackendPollV1::Pending
                })
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
        if self.generated_submissions.contains_key(&submission) {
            return self.release_generated_submission_v1(submission);
        }
        self.check_directed_if_present_v1(submission)?;
        if self.deferred_compute_v1(submission).is_some() {
            return self.release_deferred_compute_v1(submission);
        }
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
                RoutedSubmissionV1::DeferredCompute(_) => {
                    unreachable!("deferred release handled above")
                }
                RoutedSubmissionV1::CooperativeCopy(copy) => (
                    None,
                    None,
                    Some(copy.stream),
                    copy.is_quiescent()
                        && copy
                            .compute_xgmi
                            .as_ref()
                            .is_none_or(|root| root.is_quiescent())
                        && [copy.source.child, copy.destination.child]
                            .into_iter()
                            .all(|child| self.compute_xgmi_children[child] != Some(submission)),
                ),
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
        if self.submission_retained_as_dependency(submission)
            || self.peer_launch_retains.retains(submission)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "submission is retained by a dependent copy or launch",
            ));
        }
        if !cooperative_quiescent {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cooperative copy submission is pending",
            ));
        }
        if cooperative_stream.is_some() {
            self.release_cooperative_sdma_leaf_v1(submission)?;
        }
        if let Some(route) = native_route {
            if !self.peer_launch_retains.can_release(submission)
                || !self.peer_launch_retains.matches_native_route(
                    submission,
                    route,
                    native_stream.expect("native submission retains its stream"),
                )
            {
                return Err(self.directed_corruption_v1());
            }
            let result = self.children[route.child].release_submission_v1(route.local);
            self.latch(result)?;
            if !self.peer_launch_retains.release(submission) {
                return Err(self.directed_corruption_v1());
            }
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
        self.producer_aware_native.remove(&submission);
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
        if let RoutedSubmissionV1::DeferredCompute(root) = submission_route {
            if root.stream != stream {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::WrongDevice,
                    "deferred submission belongs to another stream",
                ));
            }
            return self.record_deferred_compute_event_v1(submission, stream_route.child);
        }
        let submission_route = match submission_route {
            RoutedSubmissionV1::Native { route, .. } => (Some(*route), None),
            RoutedSubmissionV1::CooperativeCopy(copy) => (None, Some(copy.stream)),
            RoutedSubmissionV1::DeferredCompute(_) => unreachable!("deferred event handled above"),
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
            | RoutedEventV1::CooperativeCopy { submission, .. }
            | RoutedEventV1::DeferredCompute { submission, .. } => submission,
        };
        self.events.remove(&event);
        Self::decrement_indexed_count(
            &mut self.event_submission_retain_counts,
            submission,
            "live multi-device event retain count is indexed",
        );
        Ok(())
    }

    fn supports_pending_compute_peer_copy_v1(&self) -> bool {
        true
    }

    fn supports_pending_segment_frame_peer_copy_v1(&self) -> bool {
        // The exact retained whole-list frame gates one native scalar window;
        // its envelope is never treated as scalar produced-byte coverage.
        true
    }

    fn supports_pending_segment_frame_peer_copy_segments_v1(&self) -> bool {
        true
    }

    fn supports_pending_compute_peer_copy_segments_v1(&self) -> bool {
        // One exact full-Write source producer gates the complete immutable list.
        // Native restoration preserves its initially settled destination frame;
        // consumers retain that list identity, never scalar envelope coverage.
        true
    }

    fn supports_peer_copy_segments_frame_v1(&self) -> bool {
        // Stable-source lists retain their original initialized destination frame
        // independently of compute provenance. Pending endpoint-writer profiles
        // keep legacy transfer behavior without this consumer authorization.
        true
    }

    fn supports_ordered_peer_copy_segments_v1(&self) -> bool {
        // Exact same-stream destination-list predecessors serialize whole owners.
        // The final frame retains list identity, never scalar envelope coverage.
        true
    }

    fn supports_ordered_compute_peer_copy_v1(&self) -> bool {
        // Exact destination predecessors serialize complete native owners and
        // preserve initialized bytes outside each checked window.
        true
    }

    fn supports_ordered_pending_compute_peer_copy_segments_v1(&self) -> bool {
        // Source-compute custody and exact destination-list ancestry are separate
        // success dependencies. Neither authorizes extraction before restoration.
        true
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

impl RuntimeProducerAwareLaunchBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_submission_capacity_v1()?;
        self.require_no_deferred_stream_v1(request.stream)?;
        if let Some(id) = self.try_submit_deferred_compute_v1(request)? {
            return Ok(id);
        }
        let stream = Self::route(
            &self.streams,
            request.stream,
            "unknown multi-device KFD stream",
        )?;
        self.require_compute_xgmi_child_available_v1(stream.child)?;
        let kernel = Self::route(
            &self.kernels,
            request.kernel,
            "unknown multi-device KFD kernel",
        )?;
        if stream.child != kernel.child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "kernel and stream belong to different KFD devices",
            ));
        }
        if request.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "KFD binding roster exceeds the host dispatch admission bound",
            ));
        }
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(request.bindings.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("multi-device binding translation failed")
            })?;
        for binding in request.bindings {
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
            if self.allocation_retained_by_deferred_compute_v1(allocation) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "kernel binding is retained by a deferred compute consumer",
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
        if request.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "KFD compute dependency capacity exceeded",
            ));
        }
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("multi-device dependency translation failed")
            })?;
        let mut peer_producers = Vec::new();
        let peer_count = request
            .dependencies
            .iter()
            .filter(|dependency| {
                matches!(
                    self.events.get(&dependency.event),
                    Some(RoutedEventV1::CooperativeCopy { .. })
                )
            })
            .count();
        peer_producers.try_reserve_exact(peer_count).map_err(|_| {
            KfdRuntimeBackendV1::capacity("peer launch dependency translation failed")
        })?;
        let mut completed_results = Vec::new();
        let completed_count = request
            .dependencies
            .iter()
            .filter(|dependency| {
                matches!(
                    self.events.get(&dependency.event),
                    Some(RoutedEventV1::DeferredCompute { .. })
                )
            })
            .count();
        completed_results
            .try_reserve_exact(completed_count)
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("completed launch result roster allocation failed")
            })?;
        let mut completed_depth = 1;
        for (index, dependency) in request.dependencies.iter().enumerate() {
            if request.dependencies[..index]
                .iter()
                .any(|prior| prior.producer_submission == dependency.producer_submission)
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute dependencies must name distinct submissions",
                ));
            }
            match self.exact_launch_dependency_for_child(*dependency, stream.child)? {
                Some(local) => dependencies.push(local),
                None if matches!(
                    self.events.get(&dependency.event),
                    Some(RoutedEventV1::CooperativeCopy { .. })
                ) =>
                {
                    peer_producers.push(dependency.producer_submission)
                }
                None => {
                    completed_depth = completed_depth.max(
                        self.completed_deferred_dependency_depth_v1(*dependency, stream.child)?,
                    );
                    completed_results.push(dependency.producer_submission);
                }
            }
        }
        let child_launch = BackendLaunchV1 {
            stream: stream.local,
            kernel: kernel.local,
            explicit_kernarg: request.explicit_kernarg,
            bindings: &bindings,
            dependencies: &[],
            geometry: request.geometry,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        };
        let child_preflight = self.children[stream.child].preflight_compute_v1(
            child_launch,
            ComputeDependencyRosterV1::Exact(&dependencies),
        );
        let mut collected = self.latch(child_preflight)?;
        collected.minimum_dependency_depth =
            collected.minimum_dependency_depth.max(completed_depth);
        let inherited = self.inherited_peer_launch_roots_v1(stream, &collected)?;
        self.reserve_native_stream_submission_v1(request.stream)?;
        Self::reserve_route(
            &mut self.submissions,
            "multi-device submission route allocation failed",
        )?;
        let id = self.next_id()?;
        let ancestry = if peer_producers.is_empty()
            && inherited.is_empty()
            && !self.cooperative_stream_tails.contains_key(&request.stream)
        {
            None
        } else {
            Some(self.capture_mixed_peer_launch_ancestry_v1(
                id,
                request.stream,
                &peer_producers,
                &inherited,
                peer_ancestry::MAX_PEER_LAUNCH_ANCESTORS_V1,
                peer_ancestry::MAX_PEER_LAUNCH_EDGES_V1,
            )?)
        };
        if let Some(ancestry) = &ancestry {
            let child_id = self.children[stream.child].next_handle;
            collected.minimum_dependency_depth =
                collected.minimum_dependency_depth.max(ancestry.depth());
            collected.peer_access = self.prepare_peer_compute_access_v1(
                RoutedHandleV1 {
                    child: stream.child,
                    local: child_id,
                },
                ancestry,
                &bindings,
            )?;
            let (result, ordered) = ancestry.state(self);
            collected.peer_gate = Some(
                PeerComputeGateV1::waiting(id, child_id, ordered)
                    .resolve(id, child_id, result, ordered)
                    .expect("captured peer state names the exact child consumer"),
            );
        } else if bindings.iter().any(|binding| {
            self.allocation_retained_by_router_v1(RoutedHandleV1 {
                child: stream.child,
                local: binding.region.allocation,
            })
        }) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "kernel binding is retained by an unrelated cooperative copy",
            ));
        }
        let retained = if self.children[stream.child].peer_visible_device_allocations {
            self.producer_aware_native.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("producer-aware native identity index growth failed")
            })?;
            Some(RetainedComputeLaunchV1::copy_from(
                child_launch,
                self.children[stream.child].launch_payload_account.as_ref(),
            )?)
        } else {
            None
        };
        let submit = |backend: &mut Self| {
            backend.children[stream.child].submit_collected_compute_with_payload_v1(
                child_launch,
                collected,
                retained.as_ref().map(Arc::clone),
            )
        };
        let local = if completed_results.is_empty() {
            self.with_peer_launch_ancestry_v1(id, ancestry, submit)
        } else {
            self.with_peer_launch_ancestry_and_results_v1(
                id,
                request.stream,
                ancestry,
                completed_results,
                submit,
            )
        }?;
        self.submissions.insert(
            id,
            RoutedSubmissionV1::Native {
                route: RoutedHandleV1 {
                    child: stream.child,
                    local,
                },
                stream: request.stream,
            },
        );
        self.retain_native_stream_submission_v1(request.stream);
        if let Some(retained) = retained {
            self.producer_aware_native.insert(id, retained);
        }
        Ok(id)
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
        self.copy_async_with_peer_access_v1(stream, source, destination, dependencies, None)
    }
}

impl KfdRuntimeBackendV1 {
    fn copy_async_with_peer_access_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
        peer_origin: Option<PeerCopyOriginV1>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if peer_origin.is_some_and(|origin| {
            !dependencies.is_empty()
                || !origin.matches_dma(self.description.backend_device, stream, source, destination)
        }) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "private peer DMA origin does not match its exact leaf",
            ));
        }
        self.require_no_generated_stream_v1(stream)?;
        self.allocations.reject_generated(source.allocation)?;
        self.allocations.reject_generated(destination.allocation)?;
        if [source.allocation, destination.allocation]
            .into_iter()
            .any(|allocation| self.native_reconciliation_holds_v1(allocation))
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native reconciliation retains a copy endpoint",
            ));
        }
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
        if peer_origin.is_some()
            && !self.peer_dma_endpoints_are_clean_v1(source.allocation, destination.allocation)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "private peer DMA requires already reconciled endpoints",
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
        let Some(_) = direct_sdma_copy_kind_v1(source_kind, destination_kind) else {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "direct KFD copy supports H2D, D2H, or same-device D2D",
            ));
        };
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
            .next_dependency_depth_v1(None, &dependency_submissions)
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
            if peer_origin.is_none() && custody.sole_stream == Some(stream) {
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
                if self.peer_access_authorizes_owner_v1(
                    allocation,
                    *owner,
                    peer_origin,
                    PeerAccessPurposeV1::Copy,
                ) {
                    continue;
                }
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
        self.reserve_published_sdma_index_v1()?;
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
            window_bytes: 0,
            window_requests: None,
            dependencies: dependency_submissions,
            dependency_cursor: 0,
            dependency_depth,
            peer_access: peer_origin
                .map(|origin| PeerCopyAccessV1::capture(origin, stream, source, destination)),
            phase: ActiveSdmaPhaseV1::Ready,
        };
        self.retain_active_sdma_stream_v1(stream, id, new_active_sdma_stream);
        self.stream_submission_tails.insert(stream, id);
        let all_ready = active.dependencies.iter().all(|submission| {
            self.submissions
                .get(submission)
                .is_some_and(|record| record.status == BackendPollV1::Succeeded)
        });
        // DMA uses retained backing, not its possibly stale host shadow. Only
        // separately materialized compute data requires prepublication reconciliation.
        let preparation_is_ready =
            [source.allocation, destination.allocation]
                .into_iter()
                .all(|allocation| {
                    self.allocations
                        .get(&allocation)
                        .is_some_and(|record| record.native_dirty.is_empty())
                });
        self.active_sdma.insert(id, active);
        if all_ready && preparation_is_ready {
            self.publish_sdma_copy_v1(id)?;
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
                    .is_some_and(|copy| matches!(copy.phase, ActiveSdmaPhaseV1::Ready))
            });
        let Some(submission) = compute.into_iter().chain(sdma).min() else {
            return Ok(());
        };
        if sdma == Some(submission) {
            let status = self.progress_unpublished_sdma_copy_v1(submission)?;
            if matches!(status, BackendPollV1::Failed { .. }) {
                return Err(Self::quiescent_error(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "KFD SDMA stream-head publication failed before publication",
                ));
            }
            if self.active_sdma.get(&submission).is_some_and(|copy| {
                matches!(copy.phase, ActiveSdmaPhaseV1::Ready)
                    && copy.dependency_cursor == copy.dependencies.len()
            }) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "dependency-ready KFD SDMA work was not published",
                ));
            }
            return Ok(());
        }
        let pending = self
            .pending_compute
            .get(&submission)
            .expect("compute stream FIFO head remains pending");
        if pending.peer_gate.is_some() {
            let pending = self
                .pending_compute
                .remove(&submission)
                .expect("peer-gated stream head remains pending");
            match self.observe_peer_compute_gate_v1(pending)? {
                PeerComputeStepV1::Continue(pending) => {
                    self.pending_compute.insert(submission, pending);
                }
                PeerComputeStepV1::Observed(BackendPollV1::Pending) => return Ok(()),
                PeerComputeStepV1::Observed(BackendPollV1::Failed { .. }) => {
                    return Err(Self::quiescent_error(
                        KfdRuntimeBackendErrorKindV1::Native,
                        "KFD peer-gated compute failed before publication",
                    ));
                }
                PeerComputeStepV1::Observed(BackendPollV1::Succeeded) => {
                    unreachable!("unpublished gate cannot succeed")
                }
            }
        }
        let pending = self
            .pending_compute
            .get(&submission)
            .expect("successful peer gate retains its real compute stream head");
        if pending.quiescence_complete_v1()
            && let Some(detail) = self.compute_stream_head_publication_blocker_v1(pending)
        {
            return Err(Self::rejected(KfdRuntimeBackendErrorKindV1::Busy, detail));
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
            .is_some_and(|pending| {
                pending.peer_gate_allows_native_checks_v1()
                    && pending.explicit_dependency_cursor
                        == pending.explicit_success_dependencies.len()
                    && pending.quiescence_complete_v1()
                    && pending.ordered_predecessor.is_none_or(|predecessor| {
                        self.submissions
                            .get(&predecessor)
                            .is_some_and(|record| record.status != BackendPollV1::Pending)
                    })
            })
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
        let persistent_prepared = self.active.as_ref().is_some_and(|active| {
            active.id == submission
                && active
                    .execution
                    .as_ref()
                    .is_some_and(|execution| match execution {
                        ActiveComputeExecutionV1::PersistentPrepared { .. }
                        | ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. } => true,
                        #[cfg(test)]
                        ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. }
                        | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                            ..
                        } => true,
                        _ => false,
                    })
        });
        if persistent_prepared {
            return self.cancel_persistent_prepared_v1(submission);
        }
        if let Some(lane) = self.active_compute_lane_v1(submission) {
            let prepared = self
                .active_compute_submission_v1(submission)
                .is_some_and(|active| {
                    matches!(
                        active.execution,
                        Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
                    )
                });
            if prepared {
                return self.with_compute_lane_state_v1(lane, |backend| {
                    backend.cancel_materialized_prepared_v1(submission)
                });
            }
        }
        if self.active_sdma.contains_key(&submission) {
            return self.cancel_sdma_copy_v1(submission);
        }
        if self.pending_compute.contains_key(&submission) {
            let stream = self.pending_compute[&submission].launch.stream;
            if !self.pending_compute_stream_membership_intact_v1(stream, submission) {
                return Err(
                    self.terminal_error("pending compute cancellation lost its FIFO membership")
                );
            }
            let is_stream_tail = self
                .pending_compute
                .get(&submission)
                .is_some_and(|pending| {
                    self.pending_compute_streams
                        .get(&pending.launch.stream)
                        .and_then(|queue| queue.back())
                        == Some(&submission)
                });
            if !is_stream_tail {
                // Removing an interior node would let its ordered successor
                // observe a terminal predecessor before the earlier stream
                // prefix has completed.
                return Ok(crate::BackendCancellationV1::TooLate);
            }
            let stream = self.pending_compute[&submission].launch.stream;
            self.settle_indexed_unpublished_compute_v1(submission, -2)?;
            self.restore_unfinished_stream_tail_v1(stream, submission);
            return Ok(crate::BackendCancellationV1::Cancelled);
        }
        if self.submissions.contains_key(&submission)
            || self.active_compute_lane_v1(submission).is_some()
        {
            // Published packets have no reviewed withdrawal primitive;
            // completed records are likewise conclusive.
            return Ok(crate::BackendCancellationV1::TooLate);
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
    fn supports_pending_peer_readback_v1(&self) -> bool {
        true
    }

    fn supports_pending_directed_peer_readback_v1(&self) -> bool {
        true
    }

    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.require_submission_capacity_v1()?;
        self.require_no_deferred_stream_v1(stream)?;
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
            if self.pending_native_peer_readback_v1(
                stream_route,
                source,
                source_route,
                destination,
                destination_route,
                dependencies,
            )? {
                return self.submit_cooperative_copy(
                    stream,
                    source,
                    destination,
                    dependencies,
                    false,
                );
            }
            self.require_compute_xgmi_child_available_v1(stream_route.child)?;
            if self.stream_has_pending_cooperative_copy_v1(stream) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "mixed cooperative/native stream ordering requires quiescing prior cooperative work",
                ));
            }
            if self.allocation_retained_by_router_v1(source_route)
                || self.allocation_retained_by_router_v1(destination_route)
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
    /// Attempts at most one cooperative Read/Write leaf, including nested native
    /// peer progress. Metadata traversal and child-native calls retain their
    /// existing bounds; this is not a wall-clock or complete-publication guarantee.
    fn progress_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.progress_stream_quantum_v1(stream)
    }

    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.drive_stream_v1(stream)
    }
}

impl RuntimeCancellationBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        self.check_directed_if_present_v1(submission)?;
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::DeferredCompute(_) => {
                return self.cancel_deferred_compute_v1(submission);
            }
            RoutedSubmissionV1::CooperativeCopy(copy) => {
                if copy
                    .compute_xgmi
                    .as_ref()
                    .is_some_and(|root| !root.is_quiescent())
                {
                    return Ok(crate::BackendCancellationV1::TooLate);
                }
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
            self.require_compute_xgmi_child_available_v1(route.child)?;
            let result = self.children[route.child].cancel_v1(route.local);
            return self.observe_peer_launch_result_v1(submission, result, |status| {
                *status == crate::BackendCancellationV1::Cancelled
            });
        }

        if !self.cancel_cooperative_sdma_leaf_v1(submission)? {
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Cancelled);
        Ok(crate::BackendCancellationV1::Cancelled)
    }

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.generated_submissions.contains_key(&submission) {
            return self.wait_v1(submission, deadline);
        }
        let native_route = match self.submissions.get(&submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown multi-device KFD submission",
            )
        })? {
            RoutedSubmissionV1::Native { route, .. } => Some(*route),
            RoutedSubmissionV1::DeferredCompute(_) => {
                return self.drain_deferred_compute_v1(submission, deadline, true);
            }
            RoutedSubmissionV1::CooperativeCopy(copy) => {
                if deadline <= Instant::now() || copy.is_quiescent() {
                    return Ok(copy.status());
                }
                None
            }
        };
        if let Some(route) = native_route {
            if (self.peer_launch_retains.is_empty() || deadline <= Instant::now())
                && let Some(status) = self.compute_xgmi_stored_observation_v1(route)
            {
                return self.observe_peer_launch_result_v1(submission, status, |status| {
                    *status != BackendPollV1::Pending
                });
            }
            if self.peer_launch_retains.is_empty() {
                self.service_native_peer_prefix_v1(route, false)?;
                let result = self.children[route.child].drain_v1(route.local, deadline);
                return self.latch(result);
            }
            let mut attempts = 0_u32;
            let mut sleep = WAIT_INITIAL_SLEEP_V1;
            loop {
                self.refresh_peer_launch_gate_v1(submission)?;
                if Instant::now() < deadline {
                    match self.service_native_peer_prefix_v1(route, true) {
                        Ok(()) => {}
                        Err(RuntimeBackendFailureV1::Quiescent(_))
                            if self.children[route.child]
                                .exact_submission_quiescent_v1(route.local) => {}
                        Err(error) => return Err(error),
                    }
                }
                let result = match self.compute_xgmi_stored_observation_v1(route) {
                    Some(status) => status,
                    None => self.children[route.child].poll_v1(route.local),
                };
                let status = self.observe_peer_launch_result_v1(submission, result, |status| {
                    *status != BackendPollV1::Pending
                })?;
                if status != BackendPollV1::Pending
                    || !apply_wait_backoff_v1(attempts, &mut sleep, deadline)
                {
                    return Ok(status);
                }
                attempts = attempts.saturating_add(1);
            }
        }

        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        loop {
            if Instant::now() >= deadline {
                return self.poll_v1(submission);
            }
            let status = if matches!(self.submissions.get(&submission), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some())
            {
                self.progress_retained_directed_peer_v1(submission)?
            } else {
                self.progress_cooperative_copy(submission)?
            };
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

impl crate::RuntimeOwnedShutdownBackendV1 for KfdRuntimeBackendV1 {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.shutdown_native_v1()
    }
}

impl crate::RuntimeOwnedShutdownBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.shutdown_native_v1()
    }
}

impl crate::RuntimeOwnedShutdownBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.shutdown_native_v1()
    }
}

impl Drop for KfdRuntimeBackendV1 {
    fn drop(&mut self) {
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if self.cpu_queue.as_ref().is_some_and(|queue| {
            self.require_cpu_provider_v1().is_err() || queue.fixture.ensure_clean().is_err()
        }) {
            std::process::abort();
        }
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
            || self.primary_teardown.is_some()
            || scripted_owner_live
            || !self.pending_compute.is_empty()
            || self.terminal_pending_compute.is_some()
            || !self.pending_compute_streams.is_empty()
            || !self.allocation_custody.is_empty()
            || !self.compute_module_retain_counts.is_empty()
            || !self.compute_dependency_retain_counts.is_empty()
            || self.any_compute_active_v1()
            || !self.active_sdma.is_empty()
            || !self.published_sdma_submissions.is_empty()
            || !self.active_sdma_streams.is_empty()
            || self.compute_completion_reservations != 0
            || self.sdma_completion_reservations != 0
            || self.terminal_memory.is_some()
            || self.has_live_generated_native_v1()
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
mod retained_release_tests;

#[cfg(test)]
mod tests {
    mod compute_pipeline;
    mod compute_sdma_coexistence;
    mod cooperative_admission;
    mod cooperative_custody;
    mod cooperative_progress;
    mod persistent_admission_xgmi;
    mod persistent_cancellation;
    mod persistent_completion;
    mod persistent_replay;
    mod producer_launch_admission;
    mod profiling;
    mod same_device_sdma;
    mod sdma_admission;
    mod sdma_progress;
    mod sdma_promotion;
    mod sdma_wait;
    mod semantic_pipeline;
    mod staging_launch;
    mod stream_ordering;
    mod three_binding_admission;
    mod timing_routes;
    mod wait_policy;

    mod bind_recovery_tests;
    mod compute_peer_gate_tests;
    #[path = "../compute_peer/tests.rs"]
    mod compute_peer_tests;
    mod compute_quiescence_tests;
    mod compute_settlement_custody_tests;
    mod cooperative_directed_tests;
    mod cooperative_sdma_tests;
    #[cfg(feature = "cpu-runtime-fixtures")]
    mod cpu_receipt_tests;
    #[cfg(feature = "hardware-diagnostic")]
    mod directional_wait_diagnostic_tests;
    mod initial_publication_tests;
    #[path = "initialized_storage_tests.rs"]
    mod initialized_storage_tests;
    mod materialized_cancellation_tests;
    mod materialized_publication_tests;
    #[cfg(feature = "cpu-runtime-fixtures")]
    mod multi_group_drain_tests;
    mod native_xgmi_creation_tests;
    mod native_xgmi_retirement_tests;
    mod prepared_cancellation_tests;
    mod prepared_publication_tests;
    mod queued_producer_context_tests;
    #[cfg(feature = "hardware-qualification")]
    mod r57_v2_tests;
    mod sdma_allocation_tests;
    mod sdma_cancellation_custody_tests;
    mod sdma_demotion_tests;
    mod sdma_host_read_tests;
    mod sdma_host_write_tests;
    mod sdma_observation_custody_tests;
    mod sdma_pending_allocation_tests;
    mod sdma_promotion_tests;
    mod sdma_publication_custody_tests;
    mod sdma_readiness_tests;
    mod sdma_recycle_tests;
    mod sdma_synchronous_tests;

    use super::kfd_backend_sdma_seam::{
        DirectionalSdmaOpsV1, DirectionalSdmaPairOwnerV1, ScriptedBufferKindV1,
        ScriptedExecutionOutcomeV1, ScriptedFailureModeV1, ScriptedRecycleOutcomeV1,
        ScriptedSameDeviceExecutionOutcomeV1, ScriptedSdmaStepV1, SdmaTerminalCustodyV1,
        SdmaTransitionFailureV1,
    };
    use super::*;

    use crate::synthetic_cov6;

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

    fn scripted_submit_window_step_v1(
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: impl IntoIterator<Item = DirectionalSdmaCopyRequestV1>,
        outcome: ScriptedFailureModeV1,
    ) -> ScriptedSdmaStepV1 {
        ScriptedSdmaStepV1::SubmitWindow {
            direction,
            requests: requests.into_iter().collect(),
            outcome,
        }
    }

    fn scripted_same_device_submit_window_step_v1(
        requests: impl IntoIterator<Item = SameDeviceSdmaCopyRequestV1>,
        outcome: ScriptedFailureModeV1,
    ) -> ScriptedSdmaStepV1 {
        ScriptedSdmaStepV1::SubmitSameDeviceWindow {
            requests: requests.into_iter().collect(),
            outcome,
        }
    }

    fn scripted_direct_backend_v1(
        byte_len: usize,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
    ) -> (KfdRuntimeBackendV1, u64, u64, u64) {
        scripted_direct_backend_configured_v1(byte_len, steps, |_| {})
    }

    fn scripted_direct_backend_configured_v1(
        byte_len: usize,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
        configure: impl FnOnce(&mut KfdRuntimeBackendV1),
    ) -> (KfdRuntimeBackendV1, u64, u64, u64) {
        let driver = ScriptedSdmaDriverV1::new(steps);
        let host_owner = driver.test_host_owner(byte_len);
        let device_owner = driver.test_device_owner(byte_len);
        let mut backend = KfdRuntimeBackendV1::mock();
        configure(&mut backend);
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
            persistent_storage_restore: None,
            #[cfg(test)]
            #[cfg(test)]
            #[cfg(test)]
            #[cfg(test)]
            #[cfg(test)]
            #[cfg(test)]
            #[cfg(test)]
            scripted_three_binding_replay: false,
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
        for (allocation, memory_kind) in [
            (host, KfdProfileMemoryKindV1::HostVisible),
            (device, KfdProfileMemoryKindV1::DeviceLocalHostStaged),
        ] {
            let profile_allocation =
                backend.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
            backend.observe_profile_v1(profile_allocation.map(|allocation| {
                KfdRuntimeProfileEventKindV1::AllocationCreated {
                    allocation,
                    memory_kind,
                    byte_len: u64::try_from(byte_len).unwrap(),
                    alignment: 8,
                }
            }));
        }
        backend.staged_context_bytes = (byte_len as u64) * 2;
        backend.native_available = true;
        backend.sdma_enabled = true;
        backend.scripted_sdma = Some(driver);
        (backend, stream, host, device)
    }

    fn add_scripted_direct_pair_v1(
        backend: &mut KfdRuntimeBackendV1,
        byte_len: usize,
    ) -> (u64, u64) {
        let (host_owner, device_owner) = {
            let driver = backend.scripted_sdma.as_ref().unwrap();
            (
                driver.test_host_owner(byte_len),
                driver.test_device_owner(byte_len),
            )
        };
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
            persistent_storage_restore: None,
            #[cfg(test)]
            scripted_three_binding_replay: false,
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
        backend.staged_context_bytes += u64::try_from(byte_len).unwrap() * 2;
        (host, device)
    }

    fn submit_scripted_read_v1(
        backend: &mut KfdRuntimeBackendV1,
        stream: u64,
        kernel: u64,
        allocation: u64,
        byte_len: u64,
        dependencies: &[u64],
    ) -> u64 {
        let mut explicit_kernarg = [0_u8; 16];
        explicit_kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
        backend
            .submit_v1(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &[BackendBindingV1 {
                    region: BackendMemoryRegionV1 {
                        allocation,
                        access: RuntimeAccessV1::Read,
                        byte_offset: 0,
                        byte_len,
                    },
                    kernarg_byte_offset: 0,
                }],
                dependencies,
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            })
            .unwrap()
    }

    fn submit_scripted_three_binding_v1(
        backend: &mut KfdRuntimeBackendV1,
        stream: u64,
        kernel: u64,
        allocations: [u64; 3],
        byte_len: u64,
    ) -> u64 {
        submit_scripted_three_binding_with_dependencies_v1(
            backend,
            stream,
            kernel,
            allocations,
            byte_len,
            &[],
        )
    }

    fn submit_scripted_three_binding_with_dependencies_v1(
        backend: &mut KfdRuntimeBackendV1,
        stream: u64,
        kernel: u64,
        allocations: [u64; 3],
        byte_len: u64,
        dependencies: &[u64],
    ) -> u64 {
        let mut explicit_kernarg = [0_u8; 32];
        explicit_kernarg[24..].copy_from_slice(&(byte_len / 4).to_le_bytes());
        let bindings = [
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocations[0],
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len,
                },
                kernarg_byte_offset: 0,
            },
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocations[1],
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len,
                },
                kernarg_byte_offset: 8,
            },
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocations[2],
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len,
                },
                kernarg_byte_offset: 16,
            },
        ];
        backend
            .submit_v1(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &bindings,
                dependencies,
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            })
            .unwrap()
    }

    struct ThreeBindingContextArgumentsV1 {
        allocations: [crate::RuntimeAllocationIdV1; 3],
        byte_len: u64,
    }

    struct GeneratedOnlyRejectedArgumentsV1;

    impl crate::RuntimeArgumentsV1 for GeneratedOnlyRejectedArgumentsV1 {
        const SIGNATURE_V1: [u8; 32] = [7; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            panic!("generated-only rejection must precede argument encoding")
        }

        fn bindings_v1(&self) -> Vec<crate::RuntimeBindingV1> {
            panic!("generated-only rejection must precede binding extraction")
        }
    }

    impl crate::RuntimeAtomicArgumentsV1 for GeneratedOnlyRejectedArgumentsV1 {
        const OPERATION_V1: RuntimeAtomicOperationV1 = RuntimeAtomicOperationV1::Add;
        const SCOPE_V1: RuntimeMemoryScopeV1 = RuntimeMemoryScopeV1::Workgroup;
        const ORDER_V1: RuntimeMemoryOrderV1 = RuntimeMemoryOrderV1::Relaxed;
    }

    impl crate::RuntimeCollectiveArgumentsV1 for GeneratedOnlyRejectedArgumentsV1 {
        const OPERATION_V1: crate::RuntimeCollectiveOperationV1 =
            crate::RuntimeCollectiveOperationV1::ReduceSum;
        const SCOPE_V1: RuntimeMemoryScopeV1 = RuntimeMemoryScopeV1::Workgroup;
        const ORDER_V1: RuntimeMemoryOrderV1 = RuntimeMemoryOrderV1::AcquireRelease;
    }

    impl crate::RuntimeArgumentsV1 for ThreeBindingContextArgumentsV1 {
        const SIGNATURE_V1: [u8; 32] = [7; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            let mut kernarg = vec![0_u8; 32];
            kernarg[24..].copy_from_slice(&(self.byte_len / 4).to_le_bytes());
            kernarg
        }

        fn bindings_v1(&self) -> Vec<crate::RuntimeBindingV1> {
            [
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Write,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, access)| crate::RuntimeBindingV1 {
                region: crate::RuntimeMemoryRegionV1 {
                    allocation: self.allocations[index],
                    access,
                    byte_offset: 0,
                    byte_len: self.byte_len,
                },
                kernarg_byte_offset: (index * 8) as u32,
            })
            .collect()
        }
    }

    struct ThreeBindingCandidateContextArgumentsV1 {
        allocations: [crate::RuntimeAllocationIdV1; 3],
        byte_offsets: [u64; 3],
        byte_lens: [u64; 3],
        accesses: [RuntimeAccessV1; 3],
    }

    impl crate::RuntimeArgumentsV1 for ThreeBindingCandidateContextArgumentsV1 {
        const SIGNATURE_V1: [u8; 32] = [7; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            let mut kernarg = vec![0_u8; 32];
            kernarg[24..].copy_from_slice(&(self.byte_lens[0] / 4).to_le_bytes());
            kernarg
        }

        fn bindings_v1(&self) -> Vec<crate::RuntimeBindingV1> {
            (0..3)
                .map(|index| crate::RuntimeBindingV1 {
                    region: crate::RuntimeMemoryRegionV1 {
                        allocation: self.allocations[index],
                        access: self.accesses[index],
                        byte_offset: self.byte_offsets[index],
                        byte_len: self.byte_lens[index],
                    },
                    kernarg_byte_offset: (index * 8) as u32,
                })
                .collect()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ThreeBindingPrelaunchStorageV1 {
        H2dReady {
            owner_id: u64,
            authenticated_sha256: [u8; 32],
            logical_bytes: u64,
            physical_bytes: u64,
            promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
        },
        Device {
            owner_id: u64,
        },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ThreeBindingLastFullHostWriteSnapshotV1 {
        bytes_identity: usize,
        byte_len: usize,
        declared_sha256: [u8; 32],
        observed_sha256: [u8; 32],
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ThreeBindingPrelaunchSnapshotV1 {
        bytes_identity: usize,
        content_sha256: Option<[u8; 32]>,
        last_full_host_write: Option<ThreeBindingLastFullHostWriteSnapshotV1>,
        native_dirty: Vec<NativeDirtyExtentV1>,
        sdma_backed: bool,
        sdma_initialized: bool,
        sdma_shadow_dirty: bool,
        storage_restore_pending: bool,
        scripted_three_binding_replay: bool,
        storage: ThreeBindingPrelaunchStorageV1,
    }

    fn three_binding_prelaunch_snapshot_v1(
        backend: &KfdRuntimeBackendV1,
        allocations: [u64; 3],
    ) -> [ThreeBindingPrelaunchSnapshotV1; 3] {
        allocations.map(|allocation| {
            let record = &backend.allocations[&allocation];
            let storage = match &record.sdma_storage {
                KfdRuntimeSdmaStorageV1::H2dReady(ready) => {
                    ThreeBindingPrelaunchStorageV1::H2dReady {
                        owner_id: ready
                            .owner
                            .scripted_owner_id()
                            .expect("fixture retains a scripted ready owner"),
                        authenticated_sha256: ready.owner.authenticated_sha256(),
                        logical_bytes: ready.owner.byte_len(),
                        physical_bytes: ready.owner.physical_byte_len(),
                        promotion: ready.promotion,
                    }
                }
                KfdRuntimeSdmaStorageV1::Device(device) => ThreeBindingPrelaunchStorageV1::Device {
                    owner_id: device
                        .scripted_owner_id()
                        .expect("fixture retains a scripted device owner"),
                },
                _ => panic!("fixture retained unexpected prelaunch storage"),
            };
            ThreeBindingPrelaunchSnapshotV1 {
                bytes_identity: record.bytes.as_ptr() as usize,
                content_sha256: record.content_sha256,
                last_full_host_write: record.last_full_host_write.as_ref().map(
                    |(bytes, declared_sha256)| ThreeBindingLastFullHostWriteSnapshotV1 {
                        bytes_identity: bytes.as_ptr() as usize,
                        byte_len: bytes.len(),
                        declared_sha256: *declared_sha256,
                        observed_sha256: Sha256::digest(bytes).into(),
                    },
                ),
                native_dirty: record.native_dirty.clone(),
                sdma_backed: record.sdma_backed,
                sdma_initialized: record.sdma_initialized,
                sdma_shadow_dirty: record.sdma_shadow_dirty,
                storage_restore_pending: record.persistent_storage_restore.is_some(),
                scripted_three_binding_replay: record.scripted_three_binding_replay,
                storage,
            }
        })
    }

    fn remove_three_binding_ready_witness_v1(
        record: &mut AllocationRecordV1,
    ) -> ThreeBindingPrelaunchStorageV1 {
        let storage =
            core::mem::replace(&mut record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic);
        let KfdRuntimeSdmaStorageV1::H2dReady(ready) = storage else {
            unreachable!("fixture allocation retains authenticated ready custody")
        };
        let PersistentComputeReadyStorageV1 { owner, promotion } = *ready;
        let witness = ThreeBindingPrelaunchStorageV1::H2dReady {
            owner_id: owner
                .scripted_owner_id()
                .expect("fixture retains a scripted ready owner"),
            authenticated_sha256: owner.authenticated_sha256(),
            logical_bytes: owner.byte_len(),
            physical_bytes: owner.physical_byte_len(),
            promotion,
        };
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(owner.normalize()));
        record.scripted_three_binding_replay = false;
        witness
    }

    fn restore_three_binding_ready_witness_v1(
        record: &mut AllocationRecordV1,
        witness: ThreeBindingPrelaunchStorageV1,
    ) {
        let ThreeBindingPrelaunchStorageV1::H2dReady {
            owner_id,
            authenticated_sha256,
            logical_bytes: _,
            physical_bytes: _,
            promotion,
        } = witness
        else {
            unreachable!("fixture restoration requires an H2D witness")
        };
        let storage =
            core::mem::replace(&mut record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic);
        let KfdRuntimeSdmaStorageV1::Device(device) = storage else {
            unreachable!("fixture rejection retains device custody")
        };
        assert_eq!(device.scripted_owner_id(), Some(owner_id));
        let DirectionalSdmaDeviceOwnerV1::Scripted(device) = *device else {
            unreachable!("fixture retains scripted device custody")
        };
        record.sdma_storage =
            KfdRuntimeSdmaStorageV1::H2dReady(Box::new(PersistentComputeReadyStorageV1 {
                owner: PersistentComputeReadyOwnerV1::Scripted {
                    device,
                    authenticated_sha256,
                },
                promotion,
            }));
        record.scripted_three_binding_replay = false;
    }

    fn scripted_three_binding_context_v1(
        byte_len: u64,
    ) -> (
        crate::RuntimeContextV1<KfdRuntimeBackendV1>,
        crate::RuntimeStreamIdV1,
        [crate::RuntimeAllocationIdV1; 3],
        [u64; 3],
    ) {
        scripted_three_binding_context_with_steps_v1(
            byte_len,
            (0..3).flat_map(|_| {
                [
                    ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                    ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                ]
            }),
        )
    }

    fn scripted_three_binding_context_with_steps_v1(
        byte_len: u64,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
    ) -> (
        crate::RuntimeContextV1<KfdRuntimeBackendV1>,
        crate::RuntimeStreamIdV1,
        [crate::RuntimeAllocationIdV1; 3],
        [u64; 3],
    ) {
        scripted_persistent_context_with_steps_v1::<3>(byte_len, steps, false)
    }

    fn scripted_persistent_context_with_steps_v1<const N: usize>(
        byte_len: u64,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
        journal: bool,
    ) -> (
        crate::RuntimeContextV1<KfdRuntimeBackendV1>,
        crate::RuntimeStreamIdV1,
        [crate::RuntimeAllocationIdV1; N],
        [u64; N],
    ) {
        let backend = KfdRuntimeBackendV1::mock();
        let mut context = if journal {
            crate::RuntimeContextV1::open_with_version_journal_members_v1(backend, N, 16, N * 4)
                .unwrap()
        } else {
            crate::RuntimeContextV1::open(backend).unwrap()
        };
        let device = context.devices()[0].id();
        if journal {
            context
                .configure_allocation_admission_v1(device, byte_len * N as u64, N)
                .unwrap();
        }
        let stream = context.create_stream(device).unwrap();
        let allocations = std::array::from_fn(|_| {
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, byte_len, 8)
                .unwrap()
        });
        let backend_allocations = {
            let backend = context.backend_mut_for_test_v1();
            let driver = ScriptedSdmaDriverV1::new(steps);
            let owners: [DirectionalSdmaDeviceOwnerV1; N] =
                std::array::from_fn(|_| driver.test_device_owner(byte_len as usize));
            let mut backend_allocations: Vec<_> = backend.allocations.keys().copied().collect();
            backend_allocations.sort_unstable();
            let backend_allocations: [u64; N] = backend_allocations.try_into().unwrap();
            for ((index, allocation), owner) in
                backend_allocations.into_iter().enumerate().zip(owners)
            {
                let record = backend.allocations.get_mut(&allocation).unwrap();
                let bytes: Arc<[u8]> = vec![0x71 + index as u8; byte_len as usize].into();
                let authenticated_sha256 = Sha256::digest(&bytes).into();
                let DirectionalSdmaDeviceOwnerV1::Scripted(device) = owner else {
                    unreachable!("scripted factory returned native device custody")
                };
                record.bytes = bytes;
                record.content_sha256 = Some(authenticated_sha256);
                record.sdma_storage =
                    KfdRuntimeSdmaStorageV1::H2dReady(Box::new(PersistentComputeReadyStorageV1 {
                        owner: PersistentComputeReadyOwnerV1::Scripted {
                            device,
                            authenticated_sha256,
                        },
                        promotion: Some(ready_promotion_observation_v1(index as u64)),
                    }));
                record.sdma_backed = true;
                record.sdma_initialized = true;
                record.sdma_shadow_dirty = false;
                record.scripted_three_binding_replay = false;
            }
            backend.native_available = true;
            backend.sdma_enabled = true;
            backend.scripted_sdma = Some(driver);
            backend_allocations
        };
        (context, stream, allocations, backend_allocations)
    }

    fn scripted_three_binding_backend_v1(byte_len: usize) -> (KfdRuntimeBackendV1, u64, [u64; 3]) {
        let release_steps = [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ];
        scripted_three_binding_backend_with_steps_v1(byte_len, release_steps)
    }

    fn scripted_three_binding_backend_with_steps_v1(
        byte_len: usize,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
    ) -> (KfdRuntimeBackendV1, u64, [u64; 3]) {
        scripted_persistent_backend_with_steps_v1::<3>(byte_len, steps)
    }

    fn scripted_persistent_backend_with_steps_v1<const N: usize>(
        byte_len: usize,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
    ) -> (KfdRuntimeBackendV1, u64, [u64; N]) {
        let driver = ScriptedSdmaDriverV1::new(steps);
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let mut insert = |index: usize, owner| {
            let allocation = backend.next_id().unwrap();
            let bytes: Arc<[u8]> = vec![0x31 + index as u8; byte_len].into();
            let authenticated_sha256 = Sha256::digest(&bytes).into();
            let DirectionalSdmaDeviceOwnerV1::Scripted(device) = owner else {
                unreachable!("scripted factory returned native device custody")
            };
            backend.allocations.insert(
                allocation,
                AllocationRecordV1 {
                    device: 7,
                    kind: RuntimeMemoryKindV1::DeviceLocal,
                    alignment: 8,
                    bytes,
                    content_sha256: Some(authenticated_sha256),
                    last_full_host_write: None,
                    native_dirty: Vec::new(),
                    sdma_storage: KfdRuntimeSdmaStorageV1::H2dReady(Box::new(
                        PersistentComputeReadyStorageV1 {
                            owner: PersistentComputeReadyOwnerV1::Scripted {
                                device,
                                authenticated_sha256,
                            },
                            promotion: None,
                        },
                    )),
                    sdma_backed: true,
                    sdma_initialized: true,
                    sdma_shadow_dirty: false,
                    persistent_storage_restore: None,
                    #[cfg(test)]
                    scripted_three_binding_replay: false,
                },
            );
            allocation
        };
        let allocations =
            std::array::from_fn(|index| insert(index, driver.test_device_owner(byte_len)));
        backend.staged_context_bytes = u64::try_from(byte_len * N).unwrap();
        backend.native_available = true;
        backend.sdma_enabled = true;
        backend.scripted_sdma = Some(driver);
        (backend, stream, allocations)
    }

    fn release_scripted_direct_pair_v1(backend: &mut KfdRuntimeBackendV1, host: u64, device: u64) {
        backend.release_allocation_v1(host).unwrap();
        backend.allocations.get_mut(&device).unwrap().sdma_backed = false;
        backend.release_allocation_v1(device).unwrap();
    }

    fn scripted_same_device_backend_v1(
        byte_len: usize,
        steps: impl IntoIterator<Item = ScriptedSdmaStepV1>,
    ) -> (KfdRuntimeBackendV1, u64, u64, u64) {
        let driver = ScriptedSdmaDriverV1::new(steps);
        let source_owner = driver.test_device_owner(byte_len);
        let destination_owner = driver.test_device_owner(byte_len);
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let source = backend.next_id().unwrap();
        let destination = backend.next_id().unwrap();
        let record = |storage| AllocationRecordV1 {
            device: 7,
            kind: RuntimeMemoryKindV1::DeviceLocal,
            alignment: 8,
            bytes: vec![0; byte_len].into(),
            content_sha256: None,
            last_full_host_write: None,
            native_dirty: Vec::new(),
            sdma_storage: KfdRuntimeSdmaStorageV1::Device(Box::new(storage)),
            sdma_backed: true,
            sdma_initialized: true,
            sdma_shadow_dirty: false,
            persistent_storage_restore: None,
            #[cfg(test)]
            scripted_three_binding_replay: false,
        };
        backend.allocations.insert(source, record(source_owner));
        backend
            .allocations
            .insert(destination, record(destination_owner));
        backend.staged_context_bytes = (byte_len as u64) * 2;
        backend.native_available = true;
        backend.sdma_enabled = true;
        backend.scripted_sdma = Some(driver);
        (backend, stream, source, destination)
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

    fn scripted_same_device_copy_regions_v1(
        source: u64,
        destination: u64,
        byte_len: u64,
    ) -> (BackendMemoryRegionV1, BackendMemoryRegionV1) {
        (
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
        )
    }

    fn scripted_release_steps_v1() -> [ScriptedSdmaStepV1; 3] {
        [
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    }

    fn scripted_same_device_release_steps_v1() -> [ScriptedSdmaStepV1; 4] {
        [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
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

    fn clean_scripted_same_device_backend_v1(
        backend: &mut KfdRuntimeBackendV1,
        stream: u64,
        source: u64,
        destination: u64,
        submission: Option<u64>,
    ) {
        if let Some(submission) = submission {
            backend.release_submission_v1(submission).unwrap();
        }
        for allocation in [source, destination] {
            backend
                .allocations
                .get_mut(&allocation)
                .expect("scripted same-device cleanup allocation remains indexed")
                .sdma_backed = false;
            backend.release_allocation_v1(allocation).unwrap();
        }
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

    fn ready_promotion_observation_v1(ordinal: u64) -> KfdRuntimeReadyPromotionPerformanceV1 {
        KfdRuntimeReadyPromotionPerformanceV1 {
            ordinal,
            content_ordinal: 0,
            authenticated_bytes: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
            authentication: Duration::from_nanos(ordinal + 1),
        }
    }

    pub(in crate::kfd_backend) fn host_visible_three_binding_launch_v1()
    -> (KfdRuntimeBackendV1, OwnedComputeLaunchV1) {
        host_visible_three_binding_launch_with_configuration_v1(|_| {})
    }

    pub(in crate::kfd_backend) fn host_visible_three_binding_launch_with_configuration_v1(
        configure: impl FnOnce(&mut KfdRuntimeBackendV1),
    ) -> (KfdRuntimeBackendV1, OwnedComputeLaunchV1) {
        let mut backend = KfdRuntimeBackendV1::mock();
        configure(&mut backend);
        let stream = backend.create_stream_v1(7).unwrap();
        let module = backend
            .load_module_v1(7, &synthetic_cov6::three_binding_module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let bindings = [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, access)| {
            let allocation = backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
                .unwrap();
            backend
                .write_allocation_v1(allocation, 0, &[0x31 + index as u8; 64])
                .unwrap();
            let record = backend.allocations.get_mut(&allocation).unwrap();
            record.sdma_backed = true;
            record.sdma_initialized = true;
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation,
                    access,
                    byte_offset: 0,
                    byte_len: 64,
                },
                kernarg_byte_offset: (index * 8) as u32,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
        let mut explicit_kernarg = vec![0; 32];
        explicit_kernarg[24..].copy_from_slice(&16_u64.to_le_bytes());
        let launch = OwnedComputeLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: explicit_kernarg.into_boxed_slice(),
            bindings,
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        (backend, launch)
    }

    struct ScriptedActiveProducerFixtureV1 {
        backend: KfdRuntimeBackendV1,
        producer_stream: u64,
        launch: OwnedComputeLaunchV1,
        allocations: [u64; 4],
        module: u64,
        producer: u64,
        event: u64,
    }

    impl ScriptedActiveProducerFixtureV1 {
        fn new(cross_stream: bool) -> Self {
            let byte_len = 64_usize;
            let steps = (0..4).flat_map(|_| {
                [
                    ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                    ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                ]
            });
            let (backend, producer_stream, allocations) =
                scripted_persistent_backend_with_steps_v1::<4>(byte_len, steps);
            Self::with_backend(backend, producer_stream, allocations, cross_stream)
        }

        fn with_backend(
            mut backend: KfdRuntimeBackendV1,
            producer_stream: u64,
            allocations: [u64; 4],
            cross_stream: bool,
        ) -> Self {
            let byte_len = 64_usize;
            let stream = if cross_stream {
                backend.create_stream_v1(7).unwrap()
            } else {
                producer_stream
            };
            let module = backend
                .load_module_v1(7, &synthetic_cov6::three_binding_module())
                .unwrap();
            let kernel = backend
                .resolve_kernel_v1(module, "vecadd", [7; 32])
                .unwrap();
            let [a, b, c, d] = allocations;
            let producer = submit_scripted_three_binding_v1(
                &mut backend,
                producer_stream,
                kernel,
                [a, b, c],
                byte_len as u64,
            );
            assert_eq!(backend.active.as_ref().unwrap().id, producer);
            for allocation in [a, b, c] {
                assert!(matches!(backend.allocations[&allocation].sdma_storage,
                    KfdRuntimeSdmaStorageV1::ComputeInFlight(owner) if owner == producer));
            }
            let event = backend.record_event_v1(producer_stream, producer).unwrap();
            let mut explicit_kernarg = [0_u8; 32];
            explicit_kernarg[24..].copy_from_slice(&(byte_len as u64 / 4).to_le_bytes());
            let bindings = [c, b, d]
                .into_iter()
                .enumerate()
                .map(|(index, allocation)| BackendBindingV1 {
                    region: BackendMemoryRegionV1 {
                        allocation,
                        access: if index == 2 {
                            RuntimeAccessV1::Write
                        } else {
                            RuntimeAccessV1::Read
                        },
                        byte_offset: 0,
                        byte_len: byte_len as u64,
                    },
                    kernarg_byte_offset: (index * 8) as u32,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let launch = OwnedComputeLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: explicit_kernarg.into(),
                bindings,
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            };
            Self {
                backend,
                producer_stream,
                launch,
                allocations,
                module,
                producer,
                event,
            }
        }

        fn published_owners(&self) -> [(u64, usize, [u8; 32]); 3] {
            let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. }) =
                self.backend.active.as_ref().unwrap().execution.as_ref()
            else {
                panic!("producer retains its published roster");
            };
            std::array::from_fn(|index| {
                let bytes = devices[index].scripted_bytes().unwrap();
                (
                    devices[index].scripted_owner_id().unwrap(),
                    bytes.as_ptr() as usize,
                    Sha256::digest(bytes).into(),
                )
            })
        }

        fn submit(&mut self) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
            self.backend
                .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                    stream: self.launch.stream,
                    kernel: self.launch.kernel,
                    explicit_kernarg: &self.launch.explicit_kernarg,
                    bindings: &self.launch.bindings,
                    dependencies: &[BackendLaunchProducerV1 {
                        event: self.event,
                        producer_submission: self.producer,
                    }],
                    geometry: self.launch.geometry,
                })
        }

        fn finish(mut self, consumer: Option<u64>) {
            if self.backend.events.contains_key(&self.event) {
                self.backend.release_event_v1(self.event).unwrap();
            }
            assert_eq!(
                self.backend
                    .wait_v1(self.producer, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
            if let Some(consumer) = consumer {
                self.backend.release_submission_v1(consumer).unwrap();
            }
            self.backend.release_submission_v1(self.producer).unwrap();
            assert!(self.backend.compute_dependency_retain_counts.is_empty());
            assert!(self.backend.compute_module_retain_counts.is_empty());
            assert!(self.backend.allocation_custody.is_empty());
            assert_eq!(self.backend.compute_completion_reservations, 0);
            assert_runtime_compute_pipeline_empty_v1(&self.backend);
            self.backend.unload_module_v1(self.module).unwrap();
            for allocation in self.allocations {
                self.backend
                    .allocations
                    .get_mut(&allocation)
                    .unwrap()
                    .sdma_backed = false;
                self.backend.release_allocation_v1(allocation).unwrap();
            }
            if self.launch.stream != self.producer_stream {
                self.backend.destroy_stream_v1(self.launch.stream).unwrap();
            }
            self.backend
                .destroy_stream_v1(self.producer_stream)
                .unwrap();
            let driver = self.backend.scripted_sdma.as_ref().unwrap();
            assert!(driver.is_exhausted());
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
            self.backend.shutdown_native_v1().unwrap();
        }
    }

    fn assert_scripted_persistent_transition_retry_is_terminal_v1(
        stage: ScriptedPersistentTransitionFailureV1,
        wait: bool,
    ) {
        let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
        let steps = [
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                u32::try_from(byte_len).unwrap(),
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
        ];
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
        backend
            .write_allocation_v1(host, 0, &vec![0xc7; byte_len])
            .unwrap();
        let (source, destination) =
            scripted_copy_regions_v1(host, device, u64::try_from(byte_len).unwrap());
        let copy = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();

        backend.scripted_persistent_transition_failure = Some(stage);
        let compute = submit_scripted_read_v1(
            &mut backend,
            stream,
            kernel,
            device,
            u64::try_from(byte_len).unwrap(),
            &[],
        );
        backend.flush_stream_v1(stream).unwrap();
        assert!(matches!(
            backend
                .active
                .as_ref()
                .and_then(|active| active.execution.as_ref()),
            Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
        ));

        let result = if wait {
            backend.wait_v1(compute, Instant::now() + Duration::from_secs(1))
        } else {
            backend.poll_v1(compute)
        };
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Terminal(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
        ));
        assert!(backend.terminal);
        assert_eq!(
            backend.active.as_ref().map(|active| active.id),
            Some(compute)
        );
        assert!(backend.last_launch_performance_v1().is_none());
        assert!(!backend.submissions.contains_key(&compute));
        assert!(matches!(
            backend.terminal_sdma_custody,
            Some(KfdRuntimeTerminalSdmaCustodyV1::Device(
                DirectionalSdmaDeviceOwnerV1::Scripted(_)
            ))
        ));
        assert!(matches!(
            backend.allocations[&device].sdma_storage,
            KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == compute
        ));
        assert_eq!(backend.scripted_persistent_transition_failure, None);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 2);
        assert_eq!(driver.unexpected_drops(), 0);
        let _ = (copy, module);
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }

    fn assert_disjoint_sdma_timeout_custody_during_persistent_compute_v1(
        direction: Gfx942PersistentSdmaDirectionV1,
    ) {
        let byte_len = usize::try_from(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1).unwrap();
        let mut steps = vec![
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                u32::try_from(byte_len).unwrap(),
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                direction,
                0,
                0,
                u32::try_from(byte_len).unwrap(),
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Pending),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
        ];
        if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
            steps.push(ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success));
        }
        steps.extend(scripted_release_steps_v1());
        steps.extend(scripted_release_steps_v1());
        let (mut backend, compute_stream, compute_host, compute_device) =
            scripted_direct_backend_v1(byte_len, steps);
        let copy_stream = backend.create_stream_v1(7).unwrap();
        let (copy_host, copy_device) = add_scripted_direct_pair_v1(&mut backend, byte_len);
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();

        backend
            .write_allocation_v1(compute_host, 0, &vec![0x51; byte_len])
            .unwrap();
        let (source, destination) = scripted_copy_regions_v1(
            compute_host,
            compute_device,
            u64::try_from(byte_len).unwrap(),
        );
        let initial_copy = backend
            .copy_async_v1(compute_stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.published_sdma_submissions, [initial_copy]);
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert_eq!(
            backend.poll_v1(initial_copy).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(backend.published_sdma_submissions.is_empty());
        assert!(backend.published_sdma_index_is_consistent_v1());
        backend
            .write_allocation_v1(copy_host, 0, &vec![0x62; byte_len])
            .unwrap();
        let compute = submit_scripted_read_v1(
            &mut backend,
            compute_stream,
            kernel,
            compute_device,
            u64::try_from(byte_len).unwrap(),
            &[],
        );
        backend.flush_stream_v1(compute_stream).unwrap();
        assert!(backend.persistent_compute_is_active_v1());

        let (shared_source, mut shared_destination) =
            scripted_copy_regions_v1(compute_host, compute_device, 8);
        shared_destination.byte_offset = 8;
        assert!(matches!(
            backend.copy_async_v1(copy_stream, shared_source, shared_destination, &[]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        let (mut source, mut destination) =
            scripted_copy_regions_v1(copy_host, copy_device, u64::try_from(byte_len).unwrap());
        if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
            source.allocation = copy_device;
            destination.allocation = copy_host;
        }
        let concurrent_copy = backend
            .copy_async_v1(copy_stream, source, destination, &[])
            .unwrap();
        assert!(matches!(
            backend.active_sdma[&concurrent_copy].phase,
            ActiveSdmaPhaseV1::DirectionalPublished(_)
        ));
        assert_eq!(backend.published_sdma_submissions, [concurrent_copy]);
        assert!(backend.published_sdma_index_is_consistent_v1());
        let retained_compute = backend.allocation_custody[&compute_device].owners.clone();
        let retained_host = backend.allocation_custody[&copy_host].owners.clone();
        let retained_device = backend.allocation_custody[&copy_device].owners.clone();
        assert_eq!(
            backend.poll_v1(concurrent_copy).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            backend.wait_v1(concurrent_copy, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            backend.allocation_custody[&compute_device].owners,
            retained_compute
        );
        assert_eq!(backend.allocation_custody[&copy_host].owners, retained_host);
        assert_eq!(
            backend.allocation_custody[&copy_device].owners,
            retained_device
        );
        assert!(backend.persistent_compute_is_active_v1());
        assert!(matches!(
            backend.active_sdma[&concurrent_copy].phase,
            ActiveSdmaPhaseV1::DirectionalPublished(_)
        ));
        assert!(matches!(
            backend.release_submission_v1(concurrent_copy),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        if direction == Gfx942PersistentSdmaDirectionV1::HostToDevice {
            assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
        }

        // Flush of an already published copy must not publish it a second time.
        backend.flush_stream_v1(copy_stream).unwrap();
        assert!(matches!(
            backend.active_sdma[&concurrent_copy].phase,
            ActiveSdmaPhaseV1::DirectionalPublished(_)
        ));
        assert_eq!(backend.published_sdma_submissions, [concurrent_copy]);
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert_eq!(
            backend.poll_v1(concurrent_copy).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(backend.published_sdma_submissions.is_empty());
        assert!(backend.published_sdma_index_is_consistent_v1());
        if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
            assert!(backend.persistent_compute_is_active_v1());
            assert!(backend.allocation_custody.contains_key(&compute_device));
            assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
        }
        for submission in [initial_copy, compute, concurrent_copy] {
            backend.release_submission_v1(submission).unwrap();
        }
        release_scripted_direct_pair_v1(&mut backend, compute_host, compute_device);
        release_scripted_direct_pair_v1(&mut backend, copy_host, copy_device);
        backend.unload_module_v1(module).unwrap();
        backend.destroy_stream_v1(compute_stream).unwrap();
        backend.destroy_stream_v1(copy_stream).unwrap();
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
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
            launch: Arc::new(RetainedComputeLaunchV1::unaccounted_for_test(
                OwnedComputeLaunchV1 {
                    stream,
                    kernel: 10,
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
            )),
            retained_allocations: vec![allocation].into_boxed_slice(),
            ordered_predecessor: None,
            explicit_success_dependencies: dependencies.into_boxed_slice(),
            explicit_dependency_cursor: 0,
            quiescence_dependencies: Box::new([]),
            quiescence_cursor: 0,
            dependency_depth,
            peer_gate: None,
            peer_access: PeerComputePermitsV1::default(),
        }
    }

    pub(super) fn pipelined_active_for_test_v1(id: u64) -> ActiveSubmissionV1 {
        ActiveSubmissionV1 {
            source_event: Default::default(),
            id,
            stream: 7,
            ordered_predecessor: id.checked_sub(1),
            deferred_ordered_predecessor_retain: true,
            kernel: 9,
            dependency_depth: 1,
            allocations: HashSet::new(),
            writebacks: Vec::new(),
            resident_descriptors: Vec::new(),
            ordinary_recipe: None,
            dispatch_shape_sha256: [0x5a; 32],
            published_at: Instant::now(),
            performance: KfdRuntimeLaunchPerformanceV1::default(),
            execution: None,
        }
    }

    fn index_pending_compute_custody_for_test_v1(
        backend: &mut KfdRuntimeBackendV1,
        submission: u64,
    ) {
        let pending = &backend.pending_compute[&submission];
        let stream = pending.launch.stream;
        let module = pending.module;
        let kernel = pending.launch.kernel;
        let allocations = pending.retained_allocations.to_vec();
        // Legacy private scheduling fixtures use fixed handles. Populate their
        // actual resource records as well as their retain indexes; public-path
        // fixtures already own these records and are left untouched.
        let mut resources = KfdRuntimeBackendV1::mock();
        if !backend.modules.contains_key(&module) || !backend.kernels.contains_key(&kernel) {
            let source_module = resources
                .load_module_v1(7, &synthetic_cov6::module())
                .unwrap();
            let source_kernel = resources
                .resolve_kernel_v1(source_module, "vecadd", [7; 32])
                .unwrap();
            backend
                .modules
                .entry(module)
                .or_insert_with(|| resources.modules.remove(&source_module).unwrap());
            backend.kernels.entry(kernel).or_insert_with(|| {
                let mut record = resources.kernels.remove(&source_kernel).unwrap();
                record.module = module;
                record
            });
        }
        for allocation in &allocations {
            if !backend.allocations.contains_key(allocation) {
                let source = resources
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap();
                backend
                    .allocations
                    .insert(*allocation, resources.allocations.remove(&source).unwrap());
                backend.staged_context_bytes += 8;
                resources.staged_context_bytes -= 8;
            }
        }
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

    fn release_pending_compute_test_resources_v1(backend: &mut KfdRuntimeBackendV1) {
        let allocations: Vec<_> = backend.allocations.keys().copied().collect();
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        let modules: Vec<_> = backend.modules.keys().copied().collect();
        for module in modules {
            backend.unload_module_v1(module).unwrap();
        }
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
        backend
            .quiescent_sdma_submissions
            .try_reserve(backend.sdma_completion_reservations)
            .unwrap();
        let new_stream_queue = backend.reserve_active_sdma_stream_v1(stream).unwrap();
        backend.retain_active_sdma_stream_v1(stream, submission, new_stream_queue);
        backend
            .submissions
            .try_reserve(
                backend.compute_completion_reservations + backend.sdma_completion_reservations,
            )
            .unwrap();
    }

    fn assert_runtime_compute_pipeline_empty_v1(backend: &KfdRuntimeBackendV1) {
        assert!(backend.compute_pipeline.is_empty());
        assert!(
            backend
                .auxiliary_compute_lanes
                .iter()
                .all(|lane| lane.pipeline.is_empty())
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
            ready_indexed: dependencies.is_empty(),
            dependencies,
            dependency_cursor: 0,
            ticket: None,
            sequence: None,
        }
    }
}
