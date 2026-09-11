//! Safe, bounded Linux composition for one gfx942 compute-AQL queue.

use core::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustix::fd::AsFd;

use fe2o3_kfd_uapi::{
    KfdAqlComputeQueueBuffers, admit_kfd_aql_queue_ring_size, admit_kfd_queue_percentage,
    admit_kfd_queue_priority,
};
use fe2o3_runtime_model::{
    ComputeAqlQueuePlanV1, ComputeAqlQueueResourcesV1, ComputeAqlResourceBindingV1,
    ComputeAqlTargetProfileV1, IdentityDigestV1, MemoryAccessV1, MemoryCoherenceV1, MemoryKindV1,
    QueueConfigurationIdV1, QueueGenerationV1, QueueInstanceIdV1, QueueKeyV1, QueuePlanIdV1,
};
use sha2::{Digest, Sha256};

use super::completion::{
    COMPLETION_SIGNAL_ARENA_BYTES_V1, CompletionCurrentnessHandoffV1, CompletionPacketTemplateV1,
    CompletionPollWithCurrentnessHandoffV1, CompletionSignalArenaOwnerV1,
    Gfx942BarrierProbeRecycleObservationV1, Gfx942BarrierProbeV1, Gfx942BarrierProbeWaitFailureV1,
    Gfx942CompletedBarrierProbeV1, Gfx942CompletedBatchV1, Gfx942CompletionBatchV1,
    Gfx942CompletionErrorV1, Gfx942CompletionPollV1, Gfx942CompletionPollWithProgressV1,
    Gfx942CompletionRecycleObservationV1, Gfx942CompletionWaitFailureV1,
    Gfx942TimeoutExecutionObservationV1, Gfx942TimeoutSignalObservationV1,
    MAX_COMPLETION_POLL_ATTEMPTS_V1, NativeCompletionSignalBackendV1,
    initialize_pending_completion_signal_arena,
};
use super::dependency::{
    CompletedComputeDependencyTargetUseV1, ComputeDependencyPublicationFailureV1,
    ComputeDependencyReaderBatchFailureV1, ComputeDependencySessionOwnerV1,
    ComputeDependencyTargetPollV1, ComputeDependencyTargetUseErrorV1,
    PreparedComputeDependencyTargetUseV1, PublishedComputeDependencyTargetUseV1,
    ReleasedComputeDependencyTargetUseV1, TerminalComputeDependencyTargetUseV1,
    retain_dependency_readers_for_target_v1, rollback_dependency_readers_before_publication_v1,
};
use super::dispatch_binding::{
    DeviceDataAllocationInputV1, DeviceDataEffectV1, DispatchEpochIdentityV1, DispatchGeometryV1,
    DispatchResourceOwnerV1, FixedDispatchPreparationCustodyV1, Gfx942CompletedDispatchBatchV1,
    Gfx942CompletedDispatchReadRequestV1, Gfx942CompletedDispatchReadbackV1,
    Gfx942CompletedDispatchSnapshotRequestV1, Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1,
    Gfx942DispatchPollV1, Gfx942DispatchPollWithProgressV1, Gfx942FixedDispatchDataV1,
    Gfx942FixedDispatchPacketV1, Gfx942FixedDispatchStorageIdentityV1,
    Gfx942RecycledDispatchWriteRequestV1, PersistentFixedDispatchControlIdentityV1,
    PristineDispatchAbortV1, PristineDispatchContinuationV1, ReturnedDispatchDataV1,
    TypedKernargImageV1, persistent_fixed_dispatch_control_identity_v1, prepare_dispatch_resources,
    prepare_persistent_fixed_dispatch_resources_v1,
    prepare_public_fixed_dispatch_resources_after_detach,
    prepare_public_fixed_dispatch_resources_after_pristine_abort_v1,
    prepare_public_fixed_dispatch_resources_after_recycle,
    prepare_three_binding_persistent_fixed_dispatch_resources_v1,
    three_binding_persistent_fixed_dispatch_control_identity_v1, unwrap_completed,
    unwrap_published, validate_fixed_batch_ring, wrap_completed, wrap_poll_with_progress,
    wrap_published,
};
use super::submit::{
    NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1, NativeAqlSubmissionFailureV1,
    NativeAqlSubmissionOwnerV1, NativeBarrierAndSubmissionFailureV1, initialize_amd_aql_control,
    initialize_invalid_ring,
};
use super::*;
use crate::persistent_allocation::{
    Gfx942PersistentCompletedV1, Gfx942PersistentDependencyFrontierV1,
    Gfx942PersistentDeviceAllocationV1, Gfx942PersistentOperationV1, Gfx942PersistentPreparedV1,
    Gfx942PersistentPublishedV1, Gfx942PersistentQuarantineReasonV1, Gfx942PersistentReservedV1,
    Gfx942PersistentUseErrorV1, Gfx942PersistentUseLeaseV1, Gfx942PersistentUseRequestV1,
    cancel_prepared_local_sdma_pair_v1, detach_local_native_pair_for_sdma_v1,
    quarantine_published_local_sdma_pair_v1,
};
use crate::persistent_compute::{
    BoundedPersistentComputeAttachmentV1, Gfx942CompletedPersistentComputeDispatchV1,
    Gfx942PersistentComputeBindFailureCustodyV1, Gfx942PersistentComputeBindFailureV1,
    Gfx942PersistentComputeBindTerminalCustodyV1, Gfx942PersistentComputeCancelFailureV1,
    Gfx942PersistentComputeCompletedV1, Gfx942PersistentComputeDetachFailureV1,
    Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeEffectV1,
    Gfx942PersistentComputeExecutionFailureV1, Gfx942PersistentComputeInputV1,
    Gfx942PersistentComputePollAndRecycleFailureV1, Gfx942PersistentComputePollAndRecycleV1,
    Gfx942PersistentComputePollFailureV1, Gfx942PersistentComputePollV1,
    Gfx942PersistentComputeReadyFailureCustodyV1, Gfx942PersistentComputeReadyFailureV1,
    Gfx942PersistentComputeReadyTerminalCustodyV1, Gfx942PersistentComputeReadyV1,
    Gfx942PersistentComputeRecycleFailureV1, Gfx942PersistentComputeWaitAndRecycleV1,
    Gfx942PreparedPersistentComputeDispatchV1,
    Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
    Gfx942RecycledPersistentComputeDispatchV1,
    Gfx942RecycledThreeBindingPersistentComputeDispatchV1,
    Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1,
    Gfx942ThreeBindingPersistentComputeBindFailureV1,
    Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1,
    Gfx942ThreeBindingPersistentComputeCancelFailureV1,
    Gfx942ThreeBindingPersistentComputeCompletedV1,
    Gfx942ThreeBindingPersistentComputeDetachFailureV1,
    Gfx942ThreeBindingPersistentComputeDispatchV1,
    Gfx942ThreeBindingPersistentComputeExecutionFailureV1,
    Gfx942ThreeBindingPersistentComputeInputsV1,
    Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
    Gfx942ThreeBindingPersistentComputeTransitionFailureV1,
    Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1, PersistentComputeAttachmentEntryV1,
    PersistentComputeAttachmentV1, PersistentComputeBindingKeyV1, PersistentComputeTerminalDataV1,
    PersistentComputeTerminalNativeCustodyV1, PersistentComputeUseStateV1,
    ThreeBindingPersistentComputeAttachmentV1,
};
use crate::persistent_directional_sdma::{
    DirectionalPersistentSdmaCompletionObservationV1,
    DirectionalPersistentSdmaCompletionTransitionV1, DirectionalPersistentSdmaPreparedCustodyV1,
    DirectionalPersistentSdmaPublicationObservationV1,
    DirectionalPersistentSdmaPublicationTransitionV1,
    DirectionalPersistentSdmaWindowCompletionObservationV1,
    DirectionalPersistentSdmaWindowCompletionTransitionV1,
    DirectionalPersistentSdmaWindowPreparedCustodyV1,
    DirectionalPersistentSdmaWindowPublicationObservationV1,
    DirectionalPersistentSdmaWindowPublicationTransitionV1,
    Gfx942DirectionalPersistentSdmaCompletedV1, Gfx942DirectionalPersistentSdmaCopyPollV1,
    Gfx942DirectionalPersistentSdmaDemotionFailureV1,
    Gfx942DirectionalPersistentSdmaExecutionCustodyV1,
    Gfx942DirectionalPersistentSdmaExecutionFailureV1,
    Gfx942DirectionalPersistentSdmaPromotionFailureV1,
    Gfx942DirectionalPersistentSdmaSubmissionCustodyV1,
    Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    Gfx942DirectionalPersistentSdmaSubmissionV1,
    Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
    Gfx942DirectionalPersistentSdmaTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaTerminalStateV1,
    Gfx942DirectionalPersistentSdmaWindowCompletedV1,
    Gfx942DirectionalPersistentSdmaWindowCopyPollV1,
    Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1,
    Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowTerminalStateV1,
    Gfx942DirectionalQueuePersistentAllocationV1, Gfx942PersistentDirectionalSdmaAttachmentV1,
    Gfx942PersistentDirectionalSdmaHostBindingV1, admit_persistent_directional_sdma_pair_v1,
    classify_directional_persistent_sdma_demotion_failure_v1,
    classify_directional_persistent_sdma_promotion_failure_v1,
    demote_directional_persistent_sdma_custody_v1,
    directional_persistent_sdma_extents_are_admitted_v1,
    directional_persistent_sdma_queue_destroy_is_admitted_v1,
    directional_persistent_sdma_request_v1, map_directional_persistent_sdma_use_error_v1,
    promote_directional_persistent_sdma_custody_v1, restore_directional_persistent_sdma_request_v1,
    transition_directional_persistent_sdma_completion_v1,
    transition_directional_persistent_sdma_publication_v1,
    transition_directional_persistent_sdma_window_completion_v1,
    transition_directional_persistent_sdma_window_publication_v1,
};
use crate::persistent_same_device_sdma::{
    Gfx942SameDevicePersistentSdmaWindowCompletedV1,
    Gfx942SameDevicePersistentSdmaWindowCopyPollV1,
    Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
    Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1,
    Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1,
    Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1,
    Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1,
    Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1,
    Gfx942SameDevicePersistentSdmaWindowTerminalStateV1,
    SameDevicePersistentSdmaWindowCompletionObservationV1,
    SameDevicePersistentSdmaWindowCompletionTransitionV1,
    SameDevicePersistentSdmaWindowPreparedCustodyV1,
    SameDevicePersistentSdmaWindowPublicationObservationV1,
    SameDevicePersistentSdmaWindowPublicationTransitionV1,
    restore_same_device_persistent_sdma_request_v1, same_device_destination_use_request_v1,
    same_device_persistent_sdma_descriptor_v1, same_device_persistent_sdma_request_v1,
    same_device_source_use_request_v1, transition_same_device_persistent_sdma_window_completion_v1,
    transition_same_device_persistent_sdma_window_publication_v1,
};
use crate::persistent_sdma::{
    GFX942_PERSISTENT_SDMA_MAX_ALLOCATION_BYTES_V1, Gfx942PersistentSdmaAttachmentV1,
    Gfx942PersistentSdmaCompletedV1, Gfx942PersistentSdmaCopyPollV1,
    Gfx942PersistentSdmaDemotionFailureV1, Gfx942PersistentSdmaDirectionV1,
    Gfx942PersistentSdmaExecutionCustodyV1, Gfx942PersistentSdmaExecutionFailureV1,
    Gfx942PersistentSdmaHostBindingV1, Gfx942PersistentSdmaPromotionFailureV1,
    Gfx942PersistentSdmaSubmissionCustodyV1, Gfx942PersistentSdmaSubmissionFailureV1,
    Gfx942PersistentSdmaSubmissionV1, Gfx942PersistentSdmaTerminalCustodyV1,
    Gfx942PersistentSdmaTerminalStateV1, Gfx942QueuePersistentAllocationV1,
};
use crate::queue_linux::{
    LinuxCwsrShadowPagesV1, LinuxCwsrShadowsAfterEventDestroyedV1,
    LinuxCwsrShadowsReadyForReleaseV1, LinuxDoorbellErrorV1, LinuxDoorbellSliceV1,
    LinuxKfdRuntimeDisabledV1, LinuxKfdRuntimeEnabledV1, LinuxQueueExceptionEventV1,
    LinuxUnpublishedCwsrShadowPagesV1, QueueExceptionWaitObservationV1,
    arm_process_global_kfd_runtime_gate_for_teardown_v1,
    permanently_poison_process_global_kfd_runtime_gate_v1,
};
use crate::sdma::{
    DevicePoolDispositionV1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942CombinedSdmaCapacityV1,
    Gfx942DevicePoolLimitsV1, Gfx942DevicePoolUsageV1, Gfx942DirectionalSdmaQueueObservationV1,
    Gfx942HostPoolLimitsV1, Gfx942HostPoolUsageV1, Gfx942SdmaBufferKindV1,
    Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1,
    Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyPollV1, Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1, Gfx942SdmaLogicalMuxObservationV2,
    Gfx942SdmaMemoryPoolObservationV1, Gfx942SdmaQueueObservationV1,
    Gfx942SdmaQueueProgressObservationV1, Gfx942SdmaQueueSetV1, HostPoolDispositionV1,
    PersistentSdmaWindowPollV1, PreparedPersistentSdmaWindowPublicationFailureV1,
    PreparedPersistentSdmaWindowV1, PreparedSdmaPublicationFailureV1,
    PreparedSingleSdmaPublicationFailureV1, PreparedSingleSdmaV1, SdmaWaitProfileV1,
    SingleSdmaWaitInCurrentScopeV1, allocate_device_buffer, allocate_host_buffer,
    combined_striped_sdma_queue_count_is_admitted, device_pool_recycle_decision_v1,
    device_pool_usage_v1, exact_full_host_write_is_authenticatable,
    gfx942_sdma_logical_mux_lane_count_is_admitted_v2, host_pool_recycle_decision_v1,
    host_pool_usage_v1, persistent_sdma_window_packet_count,
    planned_ticket_matches_queue_occurrence, read_host_buffer, release_buffer,
    striped_sdma_queue_count_is_admitted, write_full_host_buffer_authenticated, write_host_buffer,
};
use crate::shared_memory::{
    AqlCompletionSignalResourceRoleV1, AqlContextSaveResourceRoleV1, AqlControlResourceRoleV1,
    AqlEndOfPipeResourceRoleV1, AqlQueueGttV1, AqlRingResourceRoleV1, ExecutableAqlQueueProbeGttV1,
    ExecutableGttV1, Gfx942DeviceBackingBudgetV1, Gfx942DeviceBackingUsageV1,
    Gfx942DeviceMemoryIdentityV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
    Gfx942HostVisibleBackingBudgetV1, Gfx942HostVisibleBackingUsageV1,
    Gfx942InitializedDeviceMemoryV1, Gfx942InitializedHostVisibleMemoryV1, GttCpuWritableV1,
    GttGpuAccessibleExecutableV1, GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1,
    LiveQueueModelFoundationLoanV1, SharedGttAllocationV1, SharedGttMappedResourceFactsV1,
    SharedGttMemorySessionV1, SharedGttQueueResourceAuthorityV1, UserptrAqlControlGttV1,
    UserptrAqlQueueProbeGttV1,
};
use crate::wait::MonotonicWaitV1;
use crate::{
    CheckedGfx942XnackMinusDevice, GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
    Gfx942AqlQueueResourcePlanV1, Gfx942QueueResourcePlanningError, KfdWithAdmittedUapi,
    MemorySessionError, SHARED_GTT_MEMORY_PROFILE_SHA256_V1, plan_gfx942_aql_queue_resources,
};
use fe2o3_aql::{
    AqlCompletionObservationV1, AqlPreparedBarrierAndV1, AqlPreparedKernelDispatchBatchV2,
    AqlPreparedKernelDispatchV1, classify_acquired_completion_value_v1,
};

#[path = "queue_live/compute_sdma_coexistence.rs"]
mod compute_sdma_coexistence;
#[path = "queue_live/construction.rs"]
mod construction;
#[path = "queue_live/construction_auxiliary.rs"]
mod construction_auxiliary;
#[path = "queue_live/construction_primary.rs"]
mod construction_primary;
pub use compute_sdma_coexistence::Gfx942R66NativeObservationFailureV1;
use construction::{QueueResourcePrefixV1, RingConstructionV1};
use construction_auxiliary::QueueOwnerSlotV1;
#[cfg(test)]
pub(super) use construction_primary::settle_queue_constructor_fixture_v1;
use construction_primary::{
    PrimaryMemoryV1, PrimaryQueueConstructionV1, capture_returned_preparation_v1,
};
#[allow(unsafe_code)]
#[path = "queue_dispatch_live.rs"]
mod dispatch;
#[path = "queue_live/fixed_dispatch.rs"]
mod fixed_dispatch;
#[path = "queue_live/model_loan.rs"]
pub(in crate::queue) mod model_loan;
use model_loan::execute_live_model_custody_v1;
#[path = "queue_live/persistent_bind.rs"]
pub(in crate::queue) mod persistent_bind;
use persistent_bind::{settle_persistent_bind_preparation_v1, validate_persistent_bind_inputs_v1};
#[path = "queue_live/pristine_abort.rs"]
mod pristine_abort;
use pristine_abort::UnpublishedDispatchStateV1;
#[path = "queue_live/sdma_logical_mux.rs"]
mod sdma_logical_mux;
#[path = "queue_live/sdma_multi_queue.rs"]
mod sdma_multi_queue;

pub use dispatch::{
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_V1, Gfx942KfdDebugTargetDispatchErrorV2,
    Gfx942KfdDebugTargetDispatchResultV2, Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchErrorV1,
    Gfx942KfdDispatchPointerFixupV1, Gfx942KfdDispatchRequestErrorV1,
    Gfx942KfdDispatchRequestPartsV1, Gfx942KfdDispatchRequestV1, Gfx942KfdDispatchResultV1,
    Gfx942KfdQueueExceptionObservationV1, execute_gfx942_kfd_debug_target_dispatch_unchecked_v1,
    execute_gfx942_kfd_debug_target_dispatch_unchecked_v2,
    execute_gfx942_kfd_dispatch_unchecked_v1,
};

pub use sdma_logical_mux::{
    Gfx942SdmaLogicalMuxExecutionCustodyV2, Gfx942SdmaLogicalMuxExecutionFailureV2,
    Gfx942SdmaLogicalMuxFailureCustodyV2, Gfx942SdmaLogicalMuxFailureDispositionV2,
    Gfx942SdmaLogicalMuxSubmissionFailureV2, Gfx942SdmaLogicalMuxTerminalCustodyV2,
    Gfx942SdmaLogicalMuxTerminalNativeShardObservationV2,
};
pub use sdma_multi_queue::{
    Gfx942SdmaMultiQueueExecutionCustodyV1, Gfx942SdmaMultiQueueExecutionFailureV1,
    Gfx942SdmaMultiQueueFailureCustodyV1, Gfx942SdmaMultiQueueFailureDispositionV1,
    Gfx942SdmaMultiQueueSubmissionFailureV1, Gfx942SdmaMultiQueueTerminalCustodyV1,
    Gfx942SdmaTerminalShardObservationV1,
};

const CONTROL_BYTES: usize = 4_096;
const PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1: Duration = Duration::from_micros(50);
pub(crate) const GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1: u8 = 5;
/// Ring, control, completions, EOP, and context-save records for one compute queue.
pub const GFX942_COMPUTE_AQL_SHARED_ALLOCATION_RECORDS_V1: usize = 5;
static NEXT_QUEUE_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Canonical claim boundary for the live queue and fixed-batch foundation.
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-aql-session-r52-v1\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device\n",
    "memory_profile_sha256=026c8c05b6388149765ccb84a95739de6a6ddbbe89577217284b18bdcfdcbdd3\n",
    "kfd_userptr_memory_schema_sha256=c1cee09bdf884d2c14a5dbb89c1f6f7885962c75b1457caf412821490919ee9e\n",
    "kfd_userptr_queue_control_schema_sha256=f1d75410d6bfacff2ea15ecfff226eb8aed7912ee324a36b8ed8550fa52bce02\n",
    "queue_resource_profile_sha256=37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02\n",
    "aql_dispatch_schema_sha256=82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0\n",
    "aql_barrier_and_schema_sha256=bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5\n",
    "aql_fixed_batch_schema_sha256=a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a\n",
    "aql_completion_schema_sha256=485b21257623afce41573b27922350f125bcd7f5d339f8a89cf9a2c7c6ca77f1\n",
    "compute_event_custody_schema_sha256=3b235c35d117c198fcb21f65197431a47de78ed5081b95b459bbde61c6410e9a\n",
    "compute_dependency_publisher_schema_sha256=f988416cc136b8c09f3716af33a50207a929b3e53459e0a6de04abdb930008f7\n",
    "dispatch_binding_schema_sha256=854c96e2293317e3e70879b7af332ea953f6edfe00329f45b6f1b70dc743cb6d\n",
    "event_schema_sha256=bdde2e2d9b03690d6a63dba3d91074da214d87ece9ae1894c4d7a160bced58b8\n",
    "runtime_enable_schema_sha256=fa47481b10ea4bd89438d10b82bd8197088906e55f5f0c827dc7aa5aba906288\n",
    "source.rocr.queues.c=b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b\n",
    "source.rocr.hsakamttypes.h=fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e\n",
    "source.kfd_events.c=295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a\n",
    "source.kfd_debug.c=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "source.kfd_chardev.c=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "source.kfd_process.c=d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409\n",
    "resources=linear-private-ring-control-eop-cwsr-completion-code-kernarg-and-exact-device-local-or-coherent-host-data-authorities,exact-one-existing-shared-vm-session,transferred-model-ownership\n",
    "model-custody=identity-memory-and-private-nonclone-certificate-move-as-one-bundle,exact-session-domain-selected-device-vm-issuer-loan-generation-and-monotonic-revision-authentication,one-full-global-validation-at-mint-and-final-restore,local-constant-field-loan-and-retake-checks,all-shared-lifecycle-projections-advance-revision,exact-revision-capacity-preflight-before-native-memory-effects-plan-admission-and-queue-destroy,fresh-issuer-resets-generation-namespace-after-certificate-revocation\n",
    "shared-record-shape=primary-compute:ring1-control1-completions1-eop1-context-save1,sdma-per-queue:ring1-control1-completions1\n",
    "gtt_policy=reusable-and-dispatch-ring:gfx942-host-visible-executable-single-span-without-gfx7-gfx8-double-map-workaround,one-shot-diagnostic-rings:plain-executable-gtt-one-span-or-userptr-writable-executable-coherent-uncached-no-substitute-one-span,control:exact-one-page-same-va-userptr-writable-coherent,completion-signals:host-visible-coherent-gtt,eop-and-cwsr:executable;ring-userptr-never-selectable-by-reusable-or-dispatch-queue-APIs\n",
    "userptr-diagnostic=smallest-selected-gpu-ring-backing-discriminator,no-full-rocr-allocation-or-map-order-parity-claim\n",
    "creation-boundary=planning-session-dispatch-and-ring-errors-before-userptr-control-registration-entry-retain-existing-classification,every-error-from-the-control-allocation-attempt-through-live-session-return-is-terminal-recovers-no-authority-permanently-poisons-the-process-global-runtime-gate-and-requires-process-termination\n",
    "sdma-composition=gfx942-generic-or-directional-or-standalone-balanced-striped-2-through-16-or-co-resident-directional-plus-striped-2-through-14,one-directional-queue-reserved-per-engine,distinct-ids-among-this-session-primary-live-auxiliary-and-sdma-queues-only,auxiliary-created-after-sdma-is-checked-before-install,no-process-wide-or-foreign-session-id-uniqueness-claim\n",
    "sdma-creation=retryable-only-before-first-live-shared-memory-or-currentness-operation,raii-process-gate-poison-arm-from-opening-boundary-through-final-promotion,terminal-disposition-independent-of-optional-retained-queue-roster,prepared-live-terminal-custody-distinct,xgmi-route-failure-quarantines-both-sessions\n",
    "sdma-aggregate=striped-submit-with-prepared-all-shards,whole-submission-poll,one-shared-deadline-wait,observe-all-before-pending,full-retirement-preflight-before-infallible-normal-return-custody-moves,original-request-order,timeout-retains-whole-submission,no-atomic-device-snapshot\n",
    "runtime=one-process-global-fe2o3-context-with-refcounted-linear-queue-leases;first-lease-exact-enable-r_debug0-mode1-capabilities0-before-event-and-any-queue;last-fully-destroyed-lease-exact-disable;teardown-arm-permanent-poison-lifecycle-state-and-new-lease-admission-single-mutex-linearized;ordinary-queue-fd-or-consumed-debug-token-with-separate-same-process-admitted-control-fd;ttmp-save-excluded;foreign-kfd-clients-excluded\n",
    "initialization=every-logical-ring-slot-explicit-atomic-u32-invalid-1;control-amd-aql-v1-write-dispatch-id-at-0x38-read-dispatch-id-at-0x80-both-atomic-u64-zero-read-base-offset-u32-0x80-at-0x88;completion-arena-exact-8192-typed-64-byte-user-signals-pending-1-before-gpu-map;one-first-internal-auto-reset-signal-event-id-1-through-255-before-create;8-cwsr-bo-headers-and-24-control-stack-shadow-pages-at-0x1621000-stride,debug-offset-descending,debug-size-0x5f000,one-separate-private-aligned-error-reason-page-zero,exact-event-id\n",
    "submission=crate-private-non-clone-single-producer,aql-fixed-batch-v2-count-1-through-8192-and-ring-capacity-bounded,heap-owned-fixed-cardinality-state,no-mapped-slice-or-raw-pointer-escape,rptr-wptr-acquire,one-actual-wptr-acq-rel-fetch-add-by-count,all-invalid-bodies-before-per-packet-independent-0x1402-or-wait-for-prior-0x1502-ordered-u32-release-headers,exact-one-zero-setup-barrier-and-0x1403,conservative-service-default-wait-for-prior,release-fence-x86-sfence,one-final-volatile-u64-doorbell-store-of-last-packet-id\n",
    "completion=crate-private-non-clone-generation-bound-fixed-batches-and-one-signal-barrier-probe,fixed-batch-signal-code-kernarg-dispatch-and-queue-generations-retained-with-an-exact-sha256-occurrence-commitment-over-batch-queue-mapping-ordered-slots-and-generations-dispatch-roster-and-packet-interval,barrier-probe-queue-and-signal-generations-only,monotonic-deadline-or-legacy-bounded-atomic-acquire-poll-with-short-spin-yield-and-bounded-sleep-backoff-and-one-pre-post-currentness-envelope-and-same-scan-redacted-progress,pending-ready-fault-timeout-distinct,timeout-retains-private-linear-operation-through-sequential-pre-post-currentness-enveloped-addressless-write-read-counter-first-retained-packet-header-setup-first-retained-signal-kind-value-and-CWSR-reason-observation-before-poison,release-reset-only-after-all-retained-signals-zero-and-zero-event-reader-pins,dependency-signal-pinned-recycle-is-proven-no-effect-and-returns-exact-completed-custody-for-retry,other-recycle-failures-terminal\n",
    "compute-dependency=one-private-session-owner-seeded-from-nonzero-monotonic-session-queue-identity,nonzero-monotonic-burned-source-and-target-acceptance-epochs,128-preallocated-active-target-records-keyed-by-exact-epoch-with-capacity-preflight-before-event-reader-target-resource-or-native-mutation,one-addressless-source-event-per-actually-published-fixed-packet,any-two-distinct-live-session-lanes-with-exactly-one-source-arena-per-target,1-through-256-distinct-strictly-earlier-events,one-pass-preallocated-expected-linear-duplicate-preflight,B37-barrier-chain-and-final-target-publication-returns-stable-boxed-dispatch-plus-independent-event,pending-poll-reuses-box,ring-full-proven-no-effect-returns-exact-events,first-claim-or-later-error-and-native-callback-panic-terminally-process-gated-as-typed-failure,orchestration-unwind-preserves-payload,exact-dependent-completion-before-atomic-exactly-once-source-reader-event-release,target-event-explicit-release-or-downstream-consumption,source-and-target-recycle-and-all-lane-teardown-blocked-while-pinned-or-active\n",
    "liveness-probe=three-public-consuming-checked-device-entries-select-production-gfx942-executable-one-span-diagnostic-plain-executable-one-span-or-diagnostic-userptr-writable-executable-coherent-uncached-no-substitute-one-span-ring,selected-backing-and-exact-ring-span-bound-into-plan-and-configuration,selected-backing-bound-into-every-redacted-outcome,typed-nonzero-bounded-polls-validated-before-device-consumption,diagnostic-backings-not-selectable-by-reusable-or-dispatch-queue-APIs,exact-fresh-zero-history-no-dispatch-queue,one-zero-dependency-system-scope-barrier,queue-and-signal-generation-only,submission-retryable-only-by-explicit-before-side-effect-stage-classification,success-requires-currentness-packet-count1-write1-read0or1-timing-sensitive-header0x1403-or-device-consumed-invalid1-setup0-user-signal-completed-zero-exception-then-signal-reset-and-confirmed-explicit-queue-destroy,Creation-has-no-live-queue-and-precedes-userptr-control-registration-entry,TerminalCreation-covers-every-error-at-or-after-userptr-control-registration-entry-every-create-result-not-explicitly-failed-no-effect-and-every-post-create-failure-recovers-no-authority-permanently-poisons-process-global-runtime-gate-and-requires-process-termination,QuarantinedExecution-retains-opaque-custody-until-process-teardown,process-global-runtime-gate-poison-armed-before-destroy-and-cleared-only-after-confirmed-success,TerminalTeardown-and-panic-retain-permanent-gate-poison-and-recover-no-authority-native-resource-disposition-indeterminate-process-termination-required-no-retry-reopen-or-confirmed-cleanup\n",
    "dispatch=public-addressless-linear-fixed-batch,1-through-32-inspected-programs,1-through-8192-packets,validated-code-materialization,zero-pointer-kernarg-internal-injection,metadata-derived-COV6-geometry-and-dynamic-lds-implicit-subset-with-caller-zero-suffix,queue-pointer-and-runtime-address-fields-rejected,exact-mapped-data-set-retained-even-when-unreferenced-by-current-batch,referenced-subset-only-inspected-access-and-sealed-initialization-gates,one-immutable-physical-lane-recipe-requires-wait-for-prior-on-every-packet-and-admits-up-to-64-simultaneous-host-retained-publication-epochs-with-exact-recipe-slot-slot-generation-dispatch-completion-and-packet-occurrences,ordinary-release-readback-mutation-detach-rebind-effect-promotion-and-teardown-require-every-epoch-slot-vacant\n",
    "readback=coherent-host-data-only,owned-bounded-copy-or-exact-caller-owned-destination-after-exact-acquire-observed-completion-and-signal-recycle,exact-dispatch-generation,ordinary-range-within-one-inspected-write-or-readwrite-binding-or-exact-admitted-initialized-enclosing-snapshot,no-native-address-or-mapped-borrow,no-whole-allocation-initialization-promotion\n",
    "rebinding=all-epoch-slots-vacant-after-every-exact-completion-and-signal-recycle-before-detach,ordinary-detach-releases-code-and-kernarg,one-full-range-persistent-control-detach-retains-immutable-code-mapped-kernarg-packet-premise-and-maximum-recycled-generation-while-returning-only-the-exact-data-authority-for-directional-sdma,initial-persistent-control-open-and-explicit-release-use-full-currentness,its-exact-retained-control-replay-uses-operational-currentness-and-requires-exact-same-queue-vm-code-abi-packet-kernarg-role-layout-storage-and-predecessor-generation,live-rebind-retains-queue-ring-signal-event-doorbell-and-runtime,quiescent-rollover-confirms-old-native-destroy-before-new-queue-creation,exact-complete-detached-generation-cardinality-and-ordered-private-storage-identity-ledger,preflighted-device-or-host-insertion-at-exact-ordinal-and-release-gated-removal-or-replacement-while-unbound,exact-identity-kind-and-bounds-checked-in-place-initialized-coherent-overwrite-only-while-all-epochs-vacant,attached-recycled-exact-shape-resubmission-advances-generation-without-code-kernarg-or-data-detach,replacement-owner-seeded-from-exact-predecessor-and-next-publication-strictly-advances-dispatch-generation-across-live-rebind-or-queue-rollover,all-mapped-data-retained-with-inspected-effects-only-for-currently-referenced-subset,new-ring-program-count-packet-count-geometry-kernarg-and-data-admitted-before-next-publication,fully-initialized-state-preserved-without-stale-current-content-digest,all-live-shared-memory-lifecycle-model-mutation-including-public-prepare-data-and-three-persistent-manual-paths-use-one-central-certificate-custody-envelope\n",
    "doorbell=complete-8192-byte-kfd-slice,exact-returned-offset,madv-dontfork,no-public-address-pointer-or-mmio-accessor\n",
    "lifecycle=runtime-enable,event-create,queue-create;all-64-dispatch-epoch-slots-vacant-and-all-completion-batches-observed-and-recycled-and-event-reader-ledgers-empty;queue-destroy,event-destroy,immediate-payload-zero-protect-unmap,runtime-disable,doorbell-release,cwsr-queue-resource-and-completion-arena-release;debug-runtime-authority-leaves-token-before-event-and-create-lifecycle-mutation-with-no-post-handoff-restoration;published-owners-no-drop-ioctl-store-munmap-or-free;armed-unpublished-payload-guard-drop-zero-protect-unmap\n",
    "unwind=central-rust-catch-attempts-explicit-certificate-retake,retake-failure-on-normal-return-or-unwind-terminally-poisons-and-permanently-process-gates,public-lane-callback-unwind-restores-the-stable-lane-slot-then-terminally-process-gates-and-resumes-the-original-payload,persistent-consuming-submit-observe-and-recycle-unwind-preserves-the-returned-allocation-lease-phase-when-possible-claims-no-exact-consumed-native-stage-terminally-process-gates-and-resumes-the-original-payload,native-submission-callback-panic-is-erased-at-the-lower-owner-boundary-into-a-typed-terminal-error-and-does-not-preserve-a-rust-payload,no-foreign-unwind-or-drop-native-cleanup\n",
    "currentness=active-queue-opener-pid-before-non-draining-zero-timeout-reset-fifo-readiness-then-dedicated-wrapping-drm-vram-loss-counter-equality-then-closing-readiness-operational-fence-before-exact-persistent-replay,publication,after-bounded-preparation,and-before-mmio;readiness-means-nonempty-fifo-only-by-pinned-kfd-source-contract-not-loaded-kernel-authentication;packet-atomics-run-inside-those-owner-scopes;lifecycle-ioctls-and-persistent-control-open-close-retain-full-process-namespace-descriptor-uapi-xnack-drm-identity-vram-loss-topology-aperture-composite;operational-fence-excludes-those-lifecycle-identity-reobservations-and-cannot-exclude-reset-counter-wrap-or-observation-ABA;timeout-observation-confirms-device-runtime-event-and-CWSR-structure-before-and-after-its-sequential-racy-loads\n",
    "proof=queue-and-aql-model-obligations-and-hostile-rust-tests-only,no-runtime-model-refinement-of-ordered-shared-recipe-epochs-64-capacity-exact-occurrence-completion-commitment-or-out-of-order-host-observation,no-r42-or-r45-refinement-of-multiple-active-targets-one-source-arena-composition-or-dependent-completion-release,no-rust-verus-syscall-native-dependency-ordering-completion-truth-or-hardware-refinement,no-performance-or-parity-claim,cpu-gpu-atomic-coherence-mmio-driver-firmware-and-sha256-collision-resistance-refinement-contracted\n",
    "event-lifecycle=linear-private-kfd-event,no-kfd-event-page-mmap,separate-private-payload-page-cleaned-on-unpublished-install-failure,armed-unpublished-payload-cleanup-through-all-pre-create-failures-until-immediately-before-native-create-queue-call,zeroized-protected-and-unmapped-immediately-after-event-destroy-before-runtime-disable-and-independent-of-later-resource-release,payload-cleanup-failure-after-event-destroy-aborts-process-before-owner-loss,queue-destroy-before-event-destroy-before-runtime-disable-before-cwsr-free-and-full-reservation-munmap,published-owners-no-drop-ioctl-or-unmap\n",
    "cwsr-address-semantics=bo-cpu-vma-is-create-address-except-exact-24-owned-fixed-private-anonymous-control-stack-pages,prot-none-then-dontfork-then-rw,whole-span-seal-then-exact-shadow-rw-restore;headers-and-control-stack-kfd-copy-targets,wave-state-remains-read-only-bo-mapped,event-payload-disjoint-from-all-control-stack-pages;ordinary-hardware-preemption-restore-contracted\n",
    "exception-observation=crate-private-one-shot-timeout-0-through-1000ms-wait-and-terminal-timeout-direct-volatile-CWSR-reason,wait-and-payload-must-agree,unknown-reason-rejected,zero-reason-is-racy-snapshot-not-absence-proof,no-atomic-or-lossless-delivery-claim\n",
    "failure=counter-divergence-regression-currentness-and-any-possible-side-effect-runtime-event-shadow-wait-publication-completion-observation-timeout-reset-teardown-typed-native-terminal-or-orchestration-unwind-terminally-poisons-and-process-gates;65th-live-epoch-rejects-before-mutation;exact-completion-signal-arena-or-ring-capacity-failure-cancels-the-reserved-epoch-returns-public-custody-and-burns-identity-generations;certificate-revision-exhaustion-retains-plan-authority-inside-terminal-native-engine-but-initial-wrapper-recovers-no-rust-authority-or-quarantines-consumed-memory-token-custody-and-permanently-process-gates;timeout-snapshot-capture-failure-reports-currentness-or-observation-instead-of-unbound-evidence;no-in-process-recovery-rollback-or-cleanup-after-terminal-observation\n",
    "excluded=kernel-dispatch-hardware-completion-fault-or-exception-delivery-refinement,kernel-effect-correctness-beyond-inspected-metadata,full-kernel-write-coverage,kernel-numerical-correctness,device-local-update,multi-producer,foreign-kfd-process-coordination,private-cwsr-wave-record-decoding,general-multi-recipe-or-shared-buffer-dag,concurrent-kernel-execution,performance-gain-or-hip-hsa-parity\n",
);

/// SHA-256 of [`GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1`].
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1: &str =
    "421064a18734a53bfc41e627a2abeb1d402e2b3fcf4a98b1a456f9dc3c1b7be0";

type AqlSpecialRingAuthority = SharedGttQueueResourceAuthorityV1<
    AqlRingResourceRoleV1,
    AqlQueueGttV1,
    GttGpuAccessibleMutableV1,
>;
type ExecutableProbeRingAuthority = SharedGttQueueResourceAuthorityV1<
    AqlRingResourceRoleV1,
    ExecutableAqlQueueProbeGttV1,
    GttGpuAccessibleMutableV1,
>;
type UserptrProbeRingAuthority = SharedGttQueueResourceAuthorityV1<
    AqlRingResourceRoleV1,
    UserptrAqlQueueProbeGttV1,
    GttGpuAccessibleMutableV1,
>;
type AqlSpecialCpuRing = SharedGttAllocationV1<AqlQueueGttV1, GttCpuWritableV1>;
type ExecutableProbeCpuRing = SharedGttAllocationV1<ExecutableAqlQueueProbeGttV1, GttCpuWritableV1>;
type UserptrProbeCpuRing = SharedGttAllocationV1<UserptrAqlQueueProbeGttV1, GttCpuWritableV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueueRingBackingV1 {
    AqlSpecial,
    ExecutableProbe,
    UserptrProbe,
}

impl QueueRingBackingV1 {
    const fn observation(self) -> Gfx942BarrierProbeRingBackingV1 {
        match self {
            Self::AqlSpecial => Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX,
            Self::ExecutableProbe => Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX,
            Self::UserptrProbe => Gfx942BarrierProbeRingBackingV1::UserptrOneX,
        }
    }

    const fn digest_tag(self) -> u8 {
        match self {
            Self::AqlSpecial => 1,
            Self::ExecutableProbe => 2,
            Self::UserptrProbe => 3,
        }
    }

    const fn gpu_va_bytes(self, logical_bytes: u32) -> u64 {
        match self {
            Self::AqlSpecial | Self::ExecutableProbe | Self::UserptrProbe => logical_bytes as u64,
        }
    }
}

enum CpuRingAuthorityV1 {
    AqlSpecial(AqlSpecialCpuRing),
    ExecutableProbe(ExecutableProbeCpuRing),
    UserptrProbe(UserptrProbeCpuRing),
}

enum RingAuthority {
    AqlSpecial(AqlSpecialRingAuthority),
    ExecutableProbe(ExecutableProbeRingAuthority),
    UserptrProbe(UserptrProbeRingAuthority),
}

impl RingAuthority {
    const fn backing(&self) -> QueueRingBackingV1 {
        match self {
            Self::AqlSpecial(_) => QueueRingBackingV1::AqlSpecial,
            Self::ExecutableProbe(_) => QueueRingBackingV1::ExecutableProbe,
            Self::UserptrProbe(_) => QueueRingBackingV1::UserptrProbe,
        }
    }

    const fn facts(&self) -> &SharedGttMappedResourceFactsV1 {
        match self {
            Self::AqlSpecial(authority) => authority.facts(),
            Self::ExecutableProbe(authority) => authority.facts(),
            Self::UserptrProbe(authority) => authority.facts(),
        }
    }

    fn write_slot(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        slot: u32,
        packet: &[u8; fe2o3_aql::AQL_KERNEL_DISPATCH_PACKET_BYTES_V1],
    ) -> Result<(), MemorySessionError> {
        match self {
            Self::AqlSpecial(authority) => {
                memory.write_aql_ring_slot_in_current_scope(authority, slot, packet)
            }
            Self::ExecutableProbe(authority) => {
                memory.write_aql_ring_slot_in_current_scope(authority, slot, packet)
            }
            Self::UserptrProbe(authority) => {
                memory.write_aql_ring_slot_in_current_scope(authority, slot, packet)
            }
        }
    }

    fn publish_header(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        slot: u32,
        header: u16,
    ) -> Result<(), MemorySessionError> {
        match self {
            Self::AqlSpecial(authority) => {
                memory.publish_aql_ring_header_in_current_scope(authority, slot, header)
            }
            Self::ExecutableProbe(authority) => {
                memory.publish_aql_ring_header_in_current_scope(authority, slot, header)
            }
            Self::UserptrProbe(authority) => {
                memory.publish_aql_ring_header_in_current_scope(authority, slot, header)
            }
        }
    }

    fn observe_packet_header(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        packet_id: u64,
    ) -> Result<(u32, u16, u16), MemorySessionError> {
        match self {
            Self::AqlSpecial(authority) => {
                memory.observe_aql_ring_packet_header(authority, packet_id)
            }
            Self::ExecutableProbe(authority) => {
                memory.observe_aql_ring_packet_header(authority, packet_id)
            }
            Self::UserptrProbe(authority) => {
                memory.observe_aql_ring_packet_header(authority, packet_id)
            }
        }
    }

    fn unmap(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<CpuRingAuthorityV1, MemorySessionError> {
        match self {
            Self::AqlSpecial(authority) => {
                let ring = memory.unmap_from_gpu(authority.into_token())?;
                Ok(CpuRingAuthorityV1::AqlSpecial(ring))
            }
            Self::ExecutableProbe(authority) => {
                let ring = memory.unmap_from_gpu(authority.into_token())?;
                Ok(CpuRingAuthorityV1::ExecutableProbe(ring))
            }
            Self::UserptrProbe(authority) => {
                let ring = memory.unmap_from_gpu(authority.into_token())?;
                Ok(CpuRingAuthorityV1::UserptrProbe(ring))
            }
        }
    }
}

impl CpuRingAuthorityV1 {
    fn allocate(
        memory: &mut SharedGttMemorySessionV1,
        backing: QueueRingBackingV1,
        ring_bytes: usize,
    ) -> Result<Self, MemorySessionError> {
        match backing {
            QueueRingBackingV1::AqlSpecial => {
                memory.allocate_aql_queue(ring_bytes).map(Self::AqlSpecial)
            }
            QueueRingBackingV1::ExecutableProbe => memory
                .allocate_executable_aql_queue_probe(ring_bytes)
                .map(Self::ExecutableProbe),
            QueueRingBackingV1::UserptrProbe => memory
                .allocate_userptr_aql_queue_probe(ring_bytes)
                .map(Self::UserptrProbe),
        }
    }

    fn initialize_invalid(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<Result<(), NativeAqlSubmissionErrorV1>, MemorySessionError> {
        match self {
            Self::AqlSpecial(ring) => memory.with_bytes_mut(ring, initialize_invalid_ring),
            Self::ExecutableProbe(ring) => memory.with_bytes_mut(ring, initialize_invalid_ring),
            Self::UserptrProbe(ring) => memory.with_bytes_mut(ring, initialize_invalid_ring),
        }
    }

    fn release(self, memory: &mut SharedGttMemorySessionV1) -> Result<(), MemorySessionError> {
        match self {
            Self::AqlSpecial(ring) => memory.release(ring),
            Self::ExecutableProbe(ring) => memory.release(ring),
            Self::UserptrProbe(ring) => memory.release(ring),
        }
    }
}
type ControlAuthority = SharedGttQueueResourceAuthorityV1<
    AqlControlResourceRoleV1,
    UserptrAqlControlGttV1,
    GttGpuAccessibleMutableV1,
>;
type EopAuthority = SharedGttQueueResourceAuthorityV1<
    AqlEndOfPipeResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type ContextSaveAuthority = SharedGttQueueResourceAuthorityV1<
    AqlContextSaveResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type CompletionSignalAuthority = SharedGttQueueResourceAuthorityV1<
    AqlCompletionSignalResourceRoleV1,
    HostVisibleCoherentGttV1,
    GttGpuAccessibleMutableV1,
>;

struct QueueResourceAuthorityV1 {
    ring: RingAuthority,
    control: ControlAuthority,
    eop: EopAuthority,
    context_save: ContextSaveAuthority,
    view: NativeQueueResourceViewV1,
}

struct LinuxAqlSubmissionBackendV1<'a> {
    memory: &'a mut SharedGttMemorySessionV1,
    ring: &'a mut RingAuthority,
    control: &'a mut ControlAuthority,
    doorbell: &'a mut LinuxDoorbellSliceV1,
    exception: &'a QueueExceptionStateV1,
}

struct LinuxCompletionSignalBackendV1<'a> {
    memory: &'a mut SharedGttMemorySessionV1,
    signals: &'a mut CompletionSignalAuthority,
    exception: &'a QueueExceptionStateV1,
}

impl NativeCompletionSignalBackendV1 for LinuxCompletionSignalBackendV1<'_> {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        self.memory
            .check_queue_operational_currentness()
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
        self.exception
            .runtime
            .validate_queue_live_process(self.memory.opener_pid())
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
        self.exception
            .event
            .validate_live_with_shadows(
                self.memory.kfd_fd(),
                self.memory.opener_pid(),
                &self.exception.shadows,
            )
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        slot_index: u32,
    ) -> Result<fe2o3_aql::AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        self.memory
            .observe_one_aql_completion_signal_in_current_scope(self.signals, slot_index)
            .map_err(|_| Gfx942CompletionErrorV1::Observation)
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slot_indices: &[u32],
    ) -> Result<Vec<fe2o3_aql::AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        self.memory
            .observe_aql_completion_signals_in_current_scope(self.signals, slot_indices)
            .map_err(|_| Gfx942CompletionErrorV1::Observation)
    }

    fn reset_pending_release(&mut self, slot_index: u32) -> Result<(), Gfx942CompletionErrorV1> {
        self.memory
            .reset_aql_completion_signal_in_current_scope(self.signals, slot_index)
            .map_err(|_| Gfx942CompletionErrorV1::Recycle)
    }
}

impl NativeAqlSubmissionBackendV1 for LinuxAqlSubmissionBackendV1<'_> {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.memory
            .check_queue_operational_currentness()
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)?;
        self.exception
            .runtime
            .validate_queue_live_process(self.memory.opener_pid())
            .map_err(|_| NativeAqlSubmissionErrorV1::InvalidQueue("runtime exception gate"))?;
        self.exception
            .event
            .validate_live_with_shadows(
                self.memory.kfd_fd(),
                self.memory.opener_pid(),
                &self.exception.shadows,
            )
            .map_err(|_| NativeAqlSubmissionErrorV1::InvalidQueue("event/shadow exception gate"))
    }

    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        self.memory
            .observe_aql_control_counters_in_current_scope(self.control)
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)
    }

    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.memory
            .fetch_add_aql_control_write_in_current_scope(self.control, increment)
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)
    }

    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; fe2o3_aql::AQL_KERNEL_DISPATCH_PACKET_BYTES_V1],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.ring
            .write_slot(self.memory, slot, packet)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)
    }

    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.ring
            .publish_header(self.memory, slot, header)
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketHeader)
    }

    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.doorbell
            .store_packet_id_release(packet_id)
            .map_err(|_| NativeAqlSubmissionErrorV1::Doorbell)
    }
}

type LinuxNativeQueueBackendV1 = PrimaryQueueBackendV1<SharedGttMemorySessionV1>;

struct PrimaryQueueBackendV1<M> {
    session: M,
    foundation: Option<QueueModelFoundationV1>,
    foundation_in_engine: bool,
}

impl<M: PrimaryMemoryV1> NativeQueueBackendV1 for PrimaryQueueBackendV1<M> {
    type ResourceAuthority = QueueResourceAuthorityV1;

    fn opener_pid(&self) -> u32 {
        self.session.opener_pid()
    }

    fn take_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, NativeQueueAdapterErrorV1> {
        let foundation =
            self.foundation
                .take()
                .ok_or(NativeQueueAdapterErrorV1::InvalidResource(
                    "queue model ownership",
                ))?;
        self.foundation_in_engine = true;
        Ok(foundation)
    }

    fn authenticate_model_foundation(
        &self,
        foundation: &QueueModelFoundationV1,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        self.session
            .authenticate_queue_model_foundation(foundation)
            .map_err(|_| NativeQueueAdapterErrorV1::InvalidResource("queue foundation certificate"))
    }

    fn resource_view(
        &self,
        authority: &Self::ResourceAuthority,
    ) -> Result<NativeQueueResourceViewV1, NativeQueueAdapterErrorV1> {
        validate_resource_authority(authority)?;
        Ok(authority.view)
    }

    fn check_currentness(&mut self) -> Result<(), &'static str> {
        self.session
            .check_queue_currentness()
            .map_err(|_| "shared GTT/device currentness")
    }

    fn create(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs> {
        self.session.create_queue(args)
    }

    fn update(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs> {
        self.session.update_queue(args)
    }

    fn destroy(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs> {
        self.session.destroy_queue(args)
    }
}

/// Redacted observation of one confirmed live queue and mapped doorbell slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputeAqlQueueObservationV1 {
    queue_id: u32,
    ring_bytes: u32,
    doorbell_slice_bytes: usize,
    doorbell_byte_offset: u64,
    event_id: u32,
    cwsr_shadow_pages: u8,
}

impl ComputeAqlQueueObservationV1 {
    /// Process-local KFD observation, not queue authority.
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn ring_bytes(self) -> u32 {
        self.ring_bytes
    }
    pub const fn doorbell_slice_bytes(self) -> usize {
        self.doorbell_slice_bytes
    }
    /// Relative offset within the owned process slice, never a CPU/GPU address.
    pub const fn doorbell_byte_offset(self) -> u64 {
        self.doorbell_byte_offset
    }
    /// Process-local numeric observation, never event operation authority.
    pub const fn event_id(self) -> u32 {
        self.event_id
    }
    pub const fn cwsr_shadow_pages(self) -> u8 {
        self.cwsr_shadow_pages
    }

    #[cfg(test)]
    pub(crate) const fn from_parts_for_semantic_observation_tests(
        queue_id: u32,
        ring_bytes: u32,
        doorbell_slice_bytes: usize,
        doorbell_byte_offset: u64,
        event_id: u32,
        cwsr_shadow_pages: u8,
    ) -> Self {
        Self {
            queue_id,
            ring_bytes,
            doorbell_slice_bytes,
            doorbell_byte_offset,
            event_id,
            cwsr_shadow_pages,
        }
    }
}

/// Evidence returned only after confirmed DESTROY and explicit resource return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputeAqlQueueDestroyedV1 {
    queue_id: u32,
    released_resources: u8,
}

/// Addressless final state captured after one barrier completed and before recycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942BarrierProbeExecutionObservationV1 {
    inner: Gfx942TimeoutExecutionObservationV1,
}

impl Gfx942BarrierProbeExecutionObservationV1 {
    pub const fn packet_count(self) -> u16 {
        self.inner.packet_count()
    }

    pub const fn write_counter(self) -> u64 {
        self.inner.write_counter()
    }

    pub const fn read_counter(self) -> u64 {
        self.inner.read_counter()
    }

    pub const fn packet_header(self) -> u16 {
        self.inner.first_packet_header()
    }

    pub const fn packet_setup(self) -> u16 {
        self.inner.first_packet_setup()
    }

    pub const fn signal_kind(self) -> i64 {
        self.inner.first_signal_kind()
    }

    pub const fn signal(self) -> Gfx942TimeoutSignalObservationV1 {
        self.inner.first_signal()
    }

    pub const fn queue_exception_reason_mask(self) -> u64 {
        self.inner.queue_exception_reason_mask()
    }

    pub const fn currentness_confirmed(self) -> bool {
        self.inner.currentness_confirmed()
    }
}

/// Redacted success evidence returned only after signal recycle and queue destruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942BarrierProbeSuccessV1 {
    backing: Gfx942BarrierProbeRingBackingV1,
    poll_bound: u32,
    execution: Gfx942BarrierProbeExecutionObservationV1,
    recycled_signal_count: u16,
    destroyed: ComputeAqlQueueDestroyedV1,
}

/// Addressless identity of the ring allocation profile selected by a probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942BarrierProbeRingBackingV1 {
    /// Gfx942 executable GTT flags with one exact logical CPU/GPU span.
    Gfx942ExecutableOneX,
    /// Plain executable GTT flags with a one-times GPU VA span.
    ExecutableGttOneX,
    /// Writable executable coherent uncached no-substitute USERPTR, one CPU/GPU span.
    UserptrOneX,
}

/// Pre-consumption bounded poll count for the one-shot barrier probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942BarrierProbePollBoundV1(u32);

impl Gfx942BarrierProbePollBoundV1 {
    pub const fn new(polls: u32) -> Result<Self, Gfx942BarrierProbePollBoundErrorV1> {
        if polls == 0 {
            return Err(Gfx942BarrierProbePollBoundErrorV1::Zero);
        }
        if polls > MAX_COMPLETION_POLL_ATTEMPTS_V1 {
            return Err(Gfx942BarrierProbePollBoundErrorV1::ExceedsMaximum {
                requested: polls,
                maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
            });
        }
        Ok(Self(polls))
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub const fn maximum() -> u32 {
        MAX_COMPLETION_POLL_ATTEMPTS_V1
    }
}

/// Pure rejection from constructing a barrier-probe poll bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942BarrierProbePollBoundErrorV1 {
    /// A zero-attempt operation cannot establish completion.
    Zero,
    /// The requested count exceeds the frozen bounded-poll limit.
    ExceedsMaximum { requested: u32, maximum: u32 },
}

impl Gfx942BarrierProbeSuccessV1 {
    pub const fn backing(self) -> Gfx942BarrierProbeRingBackingV1 {
        self.backing
    }

    pub const fn poll_bound(self) -> u32 {
        self.poll_bound
    }

    pub const fn execution(self) -> Gfx942BarrierProbeExecutionObservationV1 {
        self.execution
    }

    pub const fn recycled_signal_count(self) -> u16 {
        self.recycled_signal_count
    }

    pub const fn destroyed(self) -> ComputeAqlQueueDestroyedV1 {
        self.destroyed
    }
}

/// Opaque queue custody after a terminal one-shot probe failure.
///
/// No queue operation or native authority accessor is exposed. The retained
/// process resources remain quarantined until process teardown.
#[must_use = "the terminal probe queue remains quarantined until process teardown"]
pub struct QuarantinedGfx942BarrierProbeV1 {
    backing: Gfx942BarrierProbeRingBackingV1,
    queue: ComputeAqlQueueSessionV1,
}

impl QuarantinedGfx942BarrierProbeV1 {
    pub const fn backing(&self) -> Gfx942BarrierProbeRingBackingV1 {
        self.backing
    }
}

impl fmt::Debug for QuarantinedGfx942BarrierProbeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuarantinedGfx942BarrierProbeV1")
            .field("backing", &self.backing)
            .field("queue", &self.queue.observation())
            .finish_non_exhaustive()
    }
}

/// Failure phase from the consuming fresh-queue barrier probe.
#[must_use = "inspect the failure and retain quarantined execution custody"]
pub enum Gfx942BarrierProbeFailureV1 {
    /// Fresh queue creation failed before a live queue was returned.
    Creation {
        error: ComputeAqlQueueSessionErrorV1,
        backing: Gfx942BarrierProbeRingBackingV1,
    },
    /// Queue creation may have taken effect; no authority is recovered.
    ///
    /// The process-global runtime gate remains poisoned and process
    /// termination is required. Retry, reopen, and cleanup claims are invalid.
    TerminalCreation {
        error: ComputeAqlQueueSessionErrorV1,
        backing: Gfx942BarrierProbeRingBackingV1,
    },
    /// Execution failed and exact queue custody is quarantined.
    QuarantinedExecution {
        error: ComputeAqlQueueSessionErrorV1,
        backing: Gfx942BarrierProbeRingBackingV1,
        retained: Box<QuarantinedGfx942BarrierProbeV1>,
    },
    /// Native teardown failed after probe completion and signal recycle.
    ///
    /// Native teardown and resource disposition are indeterminate. This
    /// variant recovers no authority and requires process termination; it
    /// does not permit retry, reopen, or any confirmed-cleanup claim.
    TerminalTeardown {
        error: ComputeAqlQueueSessionErrorV1,
        backing: Gfx942BarrierProbeRingBackingV1,
    },
}

impl Gfx942BarrierProbeFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        match self {
            Self::Creation { error, .. }
            | Self::TerminalCreation { error, .. }
            | Self::QuarantinedExecution { error, .. }
            | Self::TerminalTeardown { error, .. } => error,
        }
    }

    pub const fn backing(&self) -> Gfx942BarrierProbeRingBackingV1 {
        match self {
            Self::Creation { backing, .. }
            | Self::TerminalCreation { backing, .. }
            | Self::QuarantinedExecution { backing, .. }
            | Self::TerminalTeardown { backing, .. } => *backing,
        }
    }

    /// Returns the full addressless timeout snapshot when this was a timeout.
    pub fn timeout_observation(&self) -> Option<&Gfx942TimeoutExecutionObservationV1> {
        match self.error() {
            ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Timeout {
                observation,
                ..
            }) => Some(observation.as_ref()),
            _ => None,
        }
    }

    /// Returns opaque retained queue custody when failure preceded teardown.
    pub fn into_quarantined(self) -> Option<QuarantinedGfx942BarrierProbeV1> {
        match self {
            Self::QuarantinedExecution { retained, .. } => Some(*retained),
            Self::Creation { .. }
            | Self::TerminalCreation { .. }
            | Self::TerminalTeardown { .. } => None,
        }
    }
}

impl fmt::Debug for Gfx942BarrierProbeFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942BarrierProbeFailureV1")
            .field("error", self.error())
            .field("backing", &self.backing())
            .field(
                "phase",
                &match self {
                    Self::Creation { .. } => "creation",
                    Self::TerminalCreation { .. } => "terminal-creation",
                    Self::QuarantinedExecution { .. } => "quarantined-execution",
                    Self::TerminalTeardown { .. } => "terminal-teardown",
                },
            )
            .finish()
    }
}

impl ComputeAqlQueueDestroyedV1 {
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn released_resources(self) -> u8 {
        self.released_resources
    }

    #[cfg(test)]
    pub(crate) const fn from_parts_for_semantic_observation_tests(
        queue_id: u32,
        released_resources: u8,
    ) -> Self {
        Self {
            queue_id,
            released_resources,
        }
    }

    #[cfg(test)]
    pub(crate) const fn from_producer_for_semantic_observation_tests(queue_id: u32) -> Self {
        destroyed_queue_observation(queue_id)
    }
}

const fn destroyed_queue_observation(queue_id: u32) -> ComputeAqlQueueDestroyedV1 {
    ComputeAqlQueueDestroyedV1 {
        queue_id,
        // Ring, control, EOP, context-save, and completion-signal arena.
        released_resources: GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1,
    }
}

const fn destroyed_queue_observation_with_additional_resources(
    queue_id: u32,
    additional_resources: u8,
) -> ComputeAqlQueueDestroyedV1 {
    let mut destroyed = destroyed_queue_observation(queue_id);
    destroyed.released_resources += additional_resources;
    destroyed
}

/// Ownership returned by a prepared fixed-dispatch teardown path.
///
/// The value retains the active shared-memory session beside the actual mapped
/// device authorities. It exposes neither native identities nor device
/// addresses. Allocations that entered fully initialized retain that state, but
/// pre-publication content descriptors are not returned as current-content
/// evidence after a device dispatch. Storage admitted uninitialized remains
/// uninitialized after generic completion.
#[must_use = "returned mapped C3 leases require explicit unmap and release"]
pub struct Gfx942RecycledDispatchResourcesV1 {
    destroyed: ComputeAqlQueueDestroyedV1,
    memory: SharedGttMemorySessionV1,
    dispatch_generation: u64,
    data: Vec<Gfx942FixedDispatchDataV1>,
}

/// Data custody detached from a still-live queue after exact completion and recycle.
///
/// Queue ring, signal arena, event, doorbell, and native queue ownership remain
/// live in the session. The detached data can be supplied to a later fixed
/// batch on that same session; no native address is exposed.
#[must_use = "detached fixed-dispatch data must be rebound or explicitly released"]
pub struct Gfx942DetachedFixedDispatchV1 {
    generation: u64,
    data: Vec<Gfx942FixedDispatchDataV1>,
}

impl Gfx942DetachedFixedDispatchV1 {
    pub const fn dispatch_generation(&self) -> u64 {
        self.generation
    }

    pub fn data_lease_count(&self) -> usize {
        self.data.len()
    }

    pub fn into_data(self) -> Vec<Gfx942FixedDispatchDataV1> {
        self.data
    }
}

impl Gfx942RecycledDispatchResourcesV1 {
    pub const fn destroyed(&self) -> ComputeAqlQueueDestroyedV1 {
        self.destroyed
    }

    /// Returns zero when the destroyed batch was never published, or the exact
    /// latest recycled dispatch generation otherwise.
    pub const fn dispatch_generation(&self) -> u64 {
        self.dispatch_generation
    }

    pub fn data_lease_count(&self) -> usize {
        self.data.len()
    }

    /// Returns the exact owning KFD session and every retained mapped
    /// allocation without restoring stale exact-content authority after any
    /// publication.
    pub fn into_session_and_data(
        self,
    ) -> (SharedGttMemorySessionV1, Vec<Gfx942FixedDispatchDataV1>) {
        (self.memory, self.data)
    }

    /// Creates a replacement native queue while retaining the exact mapped
    /// data returned by the confirmed destruction of its predecessor.
    ///
    /// The replacement dispatch owner advances from the exact recycled
    /// predecessor generation. This transition does not restore stale content
    /// authority or expose native addresses. Any error consumes the returned
    /// resources because queue creation may have crossed a native side-effect
    /// boundary.
    pub fn recreate_compute_aql_queue_with_fixed_dispatch<const N: usize>(
        self,
        ring_bytes: u32,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        validate_fixed_batch_ring::<N>(ring_bytes)?;
        let Self {
            destroyed: _,
            memory,
            dispatch_generation,
            data,
        } = self;
        let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
        ComputeAqlQueueSessionV1::create_compute_aql_queue_inner(
            memory,
            geometry,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            move |memory| {
                prepare_public_fixed_dispatch_resources_after_recycle(
                    memory,
                    programs,
                    packets,
                    data,
                    dispatch_generation,
                )
                .map(Some)
                .map_err(ComputeAqlQueueSessionErrorV1::DispatchBinding)
            },
            None,
        )
    }
}

fn recover_fixed_dispatch_data(dispatch: ReturnedDispatchDataV1) -> Vec<Gfx942FixedDispatchDataV1> {
    dispatch
        .into_data()
        .into_iter()
        .map(|returned| returned.into_data())
        .collect()
}

fn fixed_dispatch_storage_identities(
    data: &[Gfx942FixedDispatchDataV1],
) -> Vec<Gfx942FixedDispatchStorageIdentityV1> {
    data.iter()
        .map(Gfx942FixedDispatchDataV1::storage_identity)
        .collect()
}

fn first_ordered_identity_mismatch<T: Eq>(expected: &[T], actual: &[T]) -> Option<usize> {
    expected
        .iter()
        .zip(actual)
        .position(|(expected, actual)| expected != actual)
        .or_else(|| (expected.len() != actual.len()).then(|| expected.len().min(actual.len())))
}

fn content_descriptor_matches_bytes(
    descriptor: Gfx942DeviceContentDescriptorV1,
    bytes: &[u8],
) -> bool {
    u64::try_from(bytes.len()) == Ok(descriptor.byte_len())
        && <[u8; 32]>::from(Sha256::digest(bytes)) == descriptor.sha256()
}

fn content_descriptor_matches_sha256(
    descriptor: Gfx942DeviceContentDescriptorV1,
    byte_len: u64,
    sha256: [u8; 32],
) -> bool {
    descriptor.byte_len() == byte_len && descriptor.sha256() == sha256
}

fn insert_detached_identity<T>(
    identities: &mut Vec<T>,
    next_insertion_index: &mut Option<usize>,
    identity: T,
) {
    let index = next_insertion_index.take().unwrap_or(identities.len());
    identities.insert(index, identity);
}

fn validate_new_detached_data_index(
    detached_data_count: usize,
    data_index: usize,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if data_index > detached_data_count {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: detached_data_count,
            detail: "detached insertion ordinal",
        });
    }
    Ok(())
}

fn insert_detached_identity_at<T>(
    identities: &mut Vec<T>,
    next_insertion_index: &mut Option<usize>,
    identity: T,
    data_index: usize,
) {
    identities.insert(data_index, identity);
    *next_insertion_index = None;
}

enum QueueDestroyOutcomeV1 {
    Released(ComputeAqlQueueDestroyedV1),
    Returned(Box<Gfx942RecycledDispatchResourcesV1>),
}

enum QueueDestroyModeV1 {
    Release,
    ReturnAttached,
    ReturnDetached(Vec<Gfx942FixedDispatchDataV1>),
}

#[derive(Debug)]
enum FixedDispatchSubmissionFailureV1 {
    RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    RetryableBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    Terminal(ComputeAqlQueueSessionErrorV1),
}

/// Classified result of publishing an already-bound ordinary fixed dispatch.
///
/// A rejected or retryable failure proves that no packet became visible. A
/// retryable failure additionally proves that the immutable binding was
/// restored after a transient reservation failure. A terminal failure means
/// the queue and process-global KFD gate were poisoned; callers must retain all
/// logical custody until process teardown.
#[derive(Debug)]
pub enum Gfx942FixedDispatchSubmissionFailureV1 {
    RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    RetryableBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    Terminal(ComputeAqlQueueSessionErrorV1),
}

impl Gfx942FixedDispatchSubmissionFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        match self {
            Self::RejectedBeforeSideEffect(error)
            | Self::RetryableBeforeSideEffect(error)
            | Self::Terminal(error) => error,
        }
    }

    pub fn into_error(self) -> ComputeAqlQueueSessionErrorV1 {
        match self {
            Self::RejectedBeforeSideEffect(error)
            | Self::RetryableBeforeSideEffect(error)
            | Self::Terminal(error) => error,
        }
    }
}

impl FixedDispatchSubmissionFailureV1 {
    fn into_error(self) -> ComputeAqlQueueSessionErrorV1 {
        match self {
            Self::RejectedBeforeSideEffect(error)
            | Self::RetryableBeforeSideEffect(error)
            | Self::Terminal(error) => error,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixedDispatchBindingModeV1 {
    Ordinary,
    ExactPersistentAttachment,
}

fn finish_fixed_dispatch_submission<const N: usize>(
    identity: DispatchEpochIdentityV1,
    completion: Result<Gfx942CompletionBatchV1<N>, FixedDispatchSubmissionFailureV1>,
    cancel_binding: impl FnOnce(DispatchEpochIdentityV1) -> Result<(), Gfx942DispatchBindingErrorV1>,
) -> Result<Gfx942DispatchBatchV1<N>, FixedDispatchSubmissionFailureV1> {
    match completion {
        Ok(completion) => Ok(wrap_published(completion, identity)),
        Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
            match cancel_binding(identity) {
                Ok(()) => Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                    error,
                )),
                Err(cancel_error) => Err(FixedDispatchSubmissionFailureV1::Terminal(
                    cancel_error.into(),
                )),
            }
        }
        Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error))
        | Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
            Err(FixedDispatchSubmissionFailureV1::Terminal(error))
        }
    }
}

#[derive(Debug)]
pub enum ComputeAqlQueueSessionErrorV1 {
    Planning(Gfx942QueueResourcePlanningError),
    Memory(MemorySessionError),
    Completion(Gfx942CompletionErrorV1),
    DispatchBinding(Gfx942DispatchBindingErrorV1),
    Contract(&'static str),
    Native(&'static str),
    Doorbell(String),
    Sdma(Gfx942SdmaErrorV1),
    /// USERPTR registration or `CREATE_QUEUE` may have taken effect and exact
    /// native custody cannot be returned. The process-global runtime gate is
    /// poisoned permanently.
    TerminalCreation {
        stage: &'static str,
        source: Box<ComputeAqlQueueSessionErrorV1>,
    },
}

impl ComputeAqlQueueSessionErrorV1 {
    pub const fn is_terminal_creation(&self) -> bool {
        matches!(self, Self::TerminalCreation { .. })
    }
}

impl fmt::Display for ComputeAqlQueueSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ComputeAqlQueueSessionErrorV1 {}

impl From<Gfx942QueueResourcePlanningError> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: Gfx942QueueResourcePlanningError) -> Self {
        Self::Planning(value)
    }
}

impl From<MemorySessionError> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: MemorySessionError) -> Self {
        Self::Memory(value)
    }
}

impl From<Gfx942CompletionErrorV1> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: Gfx942CompletionErrorV1) -> Self {
        Self::Completion(value)
    }
}

impl From<Gfx942DispatchBindingErrorV1> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: Gfx942DispatchBindingErrorV1) -> Self {
        Self::DispatchBinding(value)
    }
}

impl From<LinuxDoorbellErrorV1> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: LinuxDoorbellErrorV1) -> Self {
        Self::Doorbell(value.to_string())
    }
}

impl From<Gfx942SdmaErrorV1> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: Gfx942SdmaErrorV1) -> Self {
        Self::Sdma(value)
    }
}

#[must_use = "a recoverable failure returns both mapped buffer authorities"]
pub struct Gfx942SdmaSubmissionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovered: Option<(Gfx942SdmaBufferV1, Gfx942SdmaBufferV1)>,
}

impl Gfx942SdmaSubmissionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<(Gfx942SdmaBufferV1, Gfx942SdmaBufferV1)>,
    ) {
        (self.error, self.recovered)
    }
}

#[must_use = "a recoverable batch failure returns every mapped buffer authority"]
pub struct Gfx942SdmaBatchSubmissionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovered: Option<Vec<Gfx942SdmaCopyRequestV1>>,
}

impl Gfx942SdmaBatchSubmissionFailureV1 {
    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Vec<Gfx942SdmaCopyRequestV1>>,
    ) {
        (self.error, self.recovered)
    }
}

#[must_use = "a recoverable execution failure returns requests or pending tickets"]
pub enum Gfx942SdmaBatchExecutionRecoveryV1 {
    Requests(Vec<Gfx942SdmaCopyRequestV1>),
    PendingTickets(Vec<Gfx942SdmaCopyTicketV1>),
}

#[must_use = "inspect the error and recover pre-publication requests or timeout tickets"]
pub struct Gfx942SdmaBatchExecutionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovery: Option<Gfx942SdmaBatchExecutionRecoveryV1>,
}

impl Gfx942SdmaBatchExecutionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Gfx942SdmaBatchExecutionRecoveryV1>,
    ) {
        (self.error, self.recovery)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Gfx942SdmaBatchExecutionFinishV1 {
    Success,
    RecoverableTimeout,
    Terminal,
}

fn classify_sdma_batch_execution_finish(
    wait_error: Option<&ComputeAqlQueueSessionErrorV1>,
    closing_currentness_succeeded: bool,
) -> Gfx942SdmaBatchExecutionFinishV1 {
    if !closing_currentness_succeeded {
        return Gfx942SdmaBatchExecutionFinishV1::Terminal;
    }
    match wait_error {
        None => Gfx942SdmaBatchExecutionFinishV1::Success,
        Some(ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout)) => {
            Gfx942SdmaBatchExecutionFinishV1::RecoverableTimeout
        }
        Some(_) => Gfx942SdmaBatchExecutionFinishV1::Terminal,
    }
}

/// Move-only proof that successful directional preparation was closed by the
/// same operational observation that authorizes the immediately following
/// single-packet publication.
struct DirectionalPersistentSdmaSinglePreparedHandoffV1 {
    queue: QueueKeyV1,
    native_queue_id: u32,
    direction: Gfx942PersistentSdmaDirectionV1,
    planned_ticket: Gfx942SdmaCopyTicketV1,
    prepared: PreparedSingleSdmaV1,
}

struct DirectionalPersistentSdmaPreparedRequestV1 {
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    prepared_use: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    request: Gfx942SdmaCopyRequestV1,
}

struct DirectionalPersistentSdmaAdmittedRequestV1 {
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
}

enum DirectionalPersistentSdmaAsynchronousSingleOutcomeV1 {
    OpeningCurrentnessLost {
        admitted: DirectionalPersistentSdmaAdmittedRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
    },
    RequestPreparationRejected(Gfx942DirectionalPersistentSdmaSubmissionFailureV1),
    LowerPreparationRejected {
        prepared_request: DirectionalPersistentSdmaPreparedRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
        owner_healthy: bool,
        closing_currentness_succeeded: bool,
    },
    Publication {
        custody: DirectionalPersistentSdmaPreparedCustodyV1,
        observation: DirectionalPersistentSdmaPublicationObservationV1,
        error: ComputeAqlQueueSessionErrorV1,
        preparation_succeeded: bool,
        closing_currentness_succeeded: bool,
    },
}

const fn fused_async_single_prepublication_is_retryable_v1(
    loan_succeeded: bool,
    owner_healthy: bool,
    closing_currentness_succeeded: bool,
) -> bool {
    loan_succeeded && owner_healthy && closing_currentness_succeeded
}

impl DirectionalPersistentSdmaSinglePreparedHandoffV1 {
    fn publish(
        self,
        owner: &mut Gfx942SdmaQueueSetV1,
        memory: &mut SharedGttMemorySessionV1,
    ) -> (
        Gfx942PersistentSdmaDirectionV1,
        Gfx942SdmaCopyTicketV1,
        Result<Gfx942SdmaCopyTicketV1, PreparedSingleSdmaPublicationFailureV1>,
    ) {
        debug_assert!(planned_ticket_matches_queue_occurrence(
            self.planned_ticket,
            self.queue,
            self.native_queue_id,
        ));
        let publication = owner.submit_prepared_single_with_custody(memory, self.prepared);
        (self.direction, self.planned_ticket, publication)
    }
}

enum DirectionalPersistentSdmaSynchronousSingleOutcomeV1 {
    PreparationRejected {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        request: Gfx942SdmaCopyRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
        owner_healthy: bool,
        closing_currentness_succeeded: bool,
    },
    BeforePublication {
        custody: DirectionalPersistentSdmaPreparedCustodyV1,
        observation: DirectionalPersistentSdmaPublicationObservationV1,
        error: ComputeAqlQueueSessionErrorV1,
        preparation_succeeded: bool,
        closing_currentness_succeeded: bool,
    },
    Published {
        submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
        observation: DirectionalPersistentSdmaCompletionObservationV1,
        error: Option<ComputeAqlQueueSessionErrorV1>,
        final_currentness_succeeded: bool,
    },
}

/// Move-only counterpart for a bounded directional packet window. The ticket
/// roster allocation precedes the preparation envelope; this handoff only
/// moves the already-populated roster into the publication transition.
struct DirectionalPersistentSdmaWindowPreparedHandoffV1 {
    queue: QueueKeyV1,
    native_queue_id: u32,
    direction: Gfx942PersistentSdmaDirectionV1,
    packet_count: usize,
    planned_tickets: Vec<Gfx942SdmaCopyTicketV1>,
    prepared: PreparedPersistentSdmaWindowV1,
}

impl DirectionalPersistentSdmaWindowPreparedHandoffV1 {
    fn publish(
        self,
        owner: &mut Gfx942SdmaQueueSetV1,
        memory: &mut SharedGttMemorySessionV1,
    ) -> (
        Gfx942PersistentSdmaDirectionV1,
        usize,
        Vec<Gfx942SdmaCopyTicketV1>,
        Result<Vec<Gfx942SdmaCopyTicketV1>, PreparedPersistentSdmaWindowPublicationFailureV1>,
    ) {
        debug_assert_eq!(self.planned_tickets.len(), self.packet_count);
        debug_assert!(self.planned_tickets.iter().all(|ticket| {
            planned_ticket_matches_queue_occurrence(*ticket, self.queue, self.native_queue_id)
        }));
        let publication =
            owner.submit_prepared_persistent_window_with_custody(memory, self.prepared);
        (
            self.direction,
            self.packet_count,
            self.planned_tickets,
            publication,
        )
    }
}

/// Native-neutral custody captured immediately before one lower SDMA
/// publication attempt. This is deliberately crate-private: the public API
/// exposes only retryable, published, or process-teardown custody.
pub(crate) struct PersistentSdmaPreparedCustodyV1 {
    allocation: Gfx942QueuePersistentAllocationV1,
    prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    planned_ticket: Gfx942SdmaCopyTicketV1,
    host_binding: Gfx942PersistentSdmaHostBindingV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
}

// Keeping move-only request custody inline avoids allocation on a failure path.
#[allow(clippy::large_enum_variant)]
pub(crate) enum PersistentSdmaPublicationObservationV1 {
    Recoverable(Gfx942SdmaCopyRequestV1),
    Retained(Gfx942SdmaCopyTicketV1),
    Confirmed(Gfx942SdmaCopyTicketV1),
}

pub(crate) enum PersistentSdmaPublicationTransitionV1 {
    Retryable {
        allocation: Gfx942QueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    Published(Gfx942PersistentSdmaSubmissionV1),
    ProcessTeardown(Gfx942PersistentSdmaTerminalCustodyV1),
}

fn prepared_persistent_sdma_terminal_custody(
    custody: PersistentSdmaPreparedCustodyV1,
    request: Gfx942SdmaCopyRequestV1,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> Gfx942PersistentSdmaTerminalCustodyV1 {
    let PersistentSdmaPreparedCustodyV1 {
        allocation,
        prepared,
        planned_ticket: _,
        host_binding,
        direction,
        host_offset,
        device_offset,
        copy_bytes,
    } = custody;
    let sequence = prepared.sequence();
    let state = match restore_persistent_sdma_request(
        allocation,
        direction,
        host_offset,
        device_offset,
        copy_bytes,
        host_binding,
        request,
    ) {
        Ok((mut allocation, host)) => {
            allocation
                .owner
                .quarantine_prepared(prepared, reason)
                .expect("private prepared use must quarantine");
            Gfx942PersistentSdmaTerminalStateV1::PreparedRestored { allocation, host }
        }
        Err((mut allocation, request)) => {
            allocation
                .owner
                .quarantine_prepared(prepared, reason)
                .expect("private prepared use must quarantine");
            Gfx942PersistentSdmaTerminalStateV1::PreparedUnrestored {
                allocation,
                request,
            }
        }
    };
    Gfx942PersistentSdmaTerminalCustodyV1 {
        direction,
        sequence: Some(sequence),
        state,
    }
}

/// Applies the production ownership transition after a lower publication
/// observation. It performs no native I/O and can therefore be exercised on a
/// host with injected lower observations.
pub(crate) fn transition_persistent_sdma_publication_v1(
    custody: PersistentSdmaPreparedCustodyV1,
    observation: PersistentSdmaPublicationObservationV1,
    enclosing_operation_succeeded: bool,
    closing_currentness_succeeded: bool,
) -> PersistentSdmaPublicationTransitionV1 {
    match observation {
        PersistentSdmaPublicationObservationV1::Recoverable(request)
            if enclosing_operation_succeeded && closing_currentness_succeeded =>
        {
            let PersistentSdmaPreparedCustodyV1 {
                allocation,
                prepared,
                planned_ticket,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            } = custody;
            match restore_persistent_sdma_request(
                allocation,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                request,
            ) {
                Ok((mut allocation, host)) => {
                    allocation
                        .owner
                        .cancel_prepared(prepared)
                        .expect("private prepared use must cancel");
                    PersistentSdmaPublicationTransitionV1::Retryable { allocation, host }
                }
                Err((allocation, request)) => {
                    PersistentSdmaPublicationTransitionV1::ProcessTeardown(
                        prepared_persistent_sdma_terminal_custody(
                            PersistentSdmaPreparedCustodyV1 {
                                allocation,
                                prepared,
                                planned_ticket,
                                host_binding,
                                direction,
                                host_offset,
                                device_offset,
                                copy_bytes,
                            },
                            request,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                        ),
                    )
                }
            }
        }
        PersistentSdmaPublicationObservationV1::Recoverable(request) => {
            PersistentSdmaPublicationTransitionV1::ProcessTeardown(
                prepared_persistent_sdma_terminal_custody(
                    custody,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            )
        }
        PersistentSdmaPublicationObservationV1::Retained(ticket) => {
            let PersistentSdmaPreparedCustodyV1 {
                mut allocation,
                prepared,
                planned_ticket: _,
                host_binding: _,
                direction,
                host_offset: _,
                device_offset: _,
                copy_bytes: _,
            } = custody;
            let sequence = prepared.sequence();
            allocation
                .owner
                .quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                )
                .expect("private prepared use must quarantine");
            PersistentSdmaPublicationTransitionV1::ProcessTeardown(
                Gfx942PersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state: Gfx942PersistentSdmaTerminalStateV1::PreparedQueueRetained {
                        allocation,
                        ticket,
                    },
                },
            )
        }
        PersistentSdmaPublicationObservationV1::Confirmed(ticket) => {
            let PersistentSdmaPreparedCustodyV1 {
                mut allocation,
                prepared,
                planned_ticket,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            } = custody;
            let published = allocation
                .owner
                .publish(prepared)
                .expect("private prepared use must publish after confirmed publication");
            let ticket_identity_exact = ticket == planned_ticket;
            if enclosing_operation_succeeded
                && closing_currentness_succeeded
                && ticket_identity_exact
            {
                return PersistentSdmaPublicationTransitionV1::Published(
                    Gfx942PersistentSdmaSubmissionV1 {
                        allocation,
                        published,
                        ticket,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    },
                );
            }
            let sequence = published.sequence();
            allocation
                .owner
                .quarantine_published(
                    published,
                    if enclosing_operation_succeeded && closing_currentness_succeeded {
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
                    } else {
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
                    },
                )
                .expect("private published use must quarantine");
            PersistentSdmaPublicationTransitionV1::ProcessTeardown(
                Gfx942PersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state: Gfx942PersistentSdmaTerminalStateV1::PublishedQueueRetained {
                        allocation,
                        ticket,
                    },
                },
            )
        }
    }
}

// Keeping completed authority inline avoids allocation after device completion.
#[allow(clippy::large_enum_variant)]
pub(crate) enum PersistentSdmaCompletionObservationV1 {
    Pending,
    Timeout,
    QueueRetained,
    Completed(Gfx942SdmaCompletedCopyV1),
}

pub(crate) enum PersistentSdmaCompletionTransitionV1 {
    Pending(Gfx942PersistentSdmaSubmissionV1),
    Timeout(Gfx942PersistentSdmaSubmissionV1),
    Completed(Gfx942PersistentSdmaCompletedV1),
    ProcessTeardown(Gfx942PersistentSdmaTerminalCustodyV1),
}

/// Applies the production ownership transition after one lower completion
/// observation. `enclosing_operation_succeeded` closes the native currentness
/// envelope around the observation.
pub(crate) fn transition_persistent_sdma_completion_v1(
    mut submission: Gfx942PersistentSdmaSubmissionV1,
    observation: PersistentSdmaCompletionObservationV1,
    enclosing_operation_succeeded: bool,
) -> PersistentSdmaCompletionTransitionV1 {
    match observation {
        PersistentSdmaCompletionObservationV1::Pending if enclosing_operation_succeeded => {
            return PersistentSdmaCompletionTransitionV1::Pending(submission);
        }
        PersistentSdmaCompletionObservationV1::Timeout if enclosing_operation_succeeded => {
            let timeout = submission
                .allocation
                .owner
                .observe_timeout(submission.published)
                .expect("private published use must retain timeout custody");
            submission.published = timeout.into_published();
            return PersistentSdmaCompletionTransitionV1::Timeout(submission);
        }
        PersistentSdmaCompletionObservationV1::Completed(completed) => {
            let Gfx942PersistentSdmaSubmissionV1 {
                allocation,
                published,
                ticket: _,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            } = submission;
            let sequence = published.sequence();
            if !enclosing_operation_succeeded {
                let mut allocation = allocation;
                allocation
                    .owner
                    .quarantine_published(
                        published,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    )
                    .expect("private published use must quarantine");
                return PersistentSdmaCompletionTransitionV1::ProcessTeardown(
                    Gfx942PersistentSdmaTerminalCustodyV1 {
                        direction,
                        sequence: Some(sequence),
                        state: Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
                            allocation,
                            completed,
                        },
                    },
                );
            }
            return match restore_completed_persistent_sdma_copy(
                allocation,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                completed,
            ) {
                Ok((mut allocation, host)) => {
                    let completed_use = allocation
                        .owner
                        .complete(published)
                        .expect("private published use must complete");
                    let frontier = allocation
                        .owner
                        .settle(completed_use)
                        .expect("single-flight persistent use must settle in order");
                    PersistentSdmaCompletionTransitionV1::Completed(
                        Gfx942PersistentSdmaCompletedV1::new(
                            allocation, host, frontier, direction, copy_bytes,
                        ),
                    )
                }
                Err((mut allocation, completed)) => {
                    allocation
                        .owner
                        .quarantine_published(
                            published,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                        )
                        .expect("private published use must quarantine");
                    PersistentSdmaCompletionTransitionV1::ProcessTeardown(
                        Gfx942PersistentSdmaTerminalCustodyV1 {
                            direction,
                            sequence: Some(sequence),
                            state: Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
                                allocation,
                                completed,
                            },
                        },
                    )
                }
            };
        }
        PersistentSdmaCompletionObservationV1::Pending
        | PersistentSdmaCompletionObservationV1::Timeout
        | PersistentSdmaCompletionObservationV1::QueueRetained => {}
    }

    let Gfx942PersistentSdmaSubmissionV1 {
        mut allocation,
        published,
        ticket,
        host_binding: _,
        direction,
        host_offset: _,
        device_offset: _,
        copy_bytes: _,
    } = submission;
    let sequence = published.sequence();
    allocation
        .owner
        .quarantine_published(
            published,
            if enclosing_operation_succeeded {
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
            } else {
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
            },
        )
        .expect("private published use must quarantine");
    PersistentSdmaCompletionTransitionV1::ProcessTeardown(Gfx942PersistentSdmaTerminalCustodyV1 {
        direction,
        sequence: Some(sequence),
        state: Gfx942PersistentSdmaTerminalStateV1::PublishedQueueRetained { allocation, ticket },
    })
}

fn map_persistent_sdma_use_error(
    error: Gfx942PersistentUseErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    ComputeAqlQueueSessionErrorV1::Contract(match error {
        Gfx942PersistentUseErrorV1::InvalidRange => "persistent SDMA device range",
        Gfx942PersistentUseErrorV1::OperationRequiresPeerMapping => {
            "persistent SDMA local operation mapping"
        }
        Gfx942PersistentUseErrorV1::Capacity => "persistent SDMA use ledger full",
        Gfx942PersistentUseErrorV1::GenerationExhausted => {
            "persistent SDMA use generation exhausted"
        }
        Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration => {
            "persistent SDMA use owner or generation"
        }
        Gfx942PersistentUseErrorV1::WrongState => "persistent SDMA use state",
        Gfx942PersistentUseErrorV1::OverlappingWriterActive => {
            "persistent SDMA overlapping writer active"
        }
        Gfx942PersistentUseErrorV1::DependencyRequired => "persistent SDMA dependency required",
        Gfx942PersistentUseErrorV1::DependencyNotRequired => {
            "persistent SDMA dependency not required"
        }
        Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency => {
            "persistent SDMA stale or substituted dependency"
        }
        Gfx942PersistentUseErrorV1::EarlierUseNotSettled => {
            "persistent SDMA earlier use not settled"
        }
        Gfx942PersistentUseErrorV1::Quarantined => "persistent SDMA allocation quarantined",
        Gfx942PersistentUseErrorV1::OutstandingUses => {
            "persistent SDMA allocation has outstanding uses"
        }
    })
}

fn persistent_sdma_request(
    direction: Gfx942PersistentSdmaDirectionV1,
    host: Gfx942SdmaBufferV1,
    host_offset: u64,
    device: Gfx942SdmaBufferV1,
    device_offset: u64,
    copy_bytes: u32,
) -> Gfx942SdmaCopyRequestV1 {
    match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => {
            Gfx942SdmaCopyRequestV1::new(host, host_offset, device, device_offset, copy_bytes)
        }
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
            Gfx942SdmaCopyRequestV1::new(device, device_offset, host, host_offset, copy_bytes)
        }
    }
}

#[allow(clippy::result_large_err)]
fn restore_persistent_sdma_request(
    mut allocation: Gfx942QueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    host_binding: Gfx942PersistentSdmaHostBindingV1,
    request: Gfx942SdmaCopyRequestV1,
) -> Result<
    (Gfx942QueuePersistentAllocationV1, Gfx942SdmaBufferV1),
    (Gfx942QueuePersistentAllocationV1, Gfx942SdmaCopyRequestV1),
> {
    let matches_offsets = request.copy_bytes == copy_bytes
        && match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                request.source_offset == host_offset
                    && request.destination_offset == device_offset
                    && request.source.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
                    && request.destination.kind() == Gfx942SdmaBufferKindV1::DeviceLocal
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                request.source_offset == device_offset
                    && request.destination_offset == host_offset
                    && request.source.kind() == Gfx942SdmaBufferKindV1::DeviceLocal
                    && request.destination.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
            }
        };
    if !matches_offsets {
        return Err((allocation, request));
    }
    let Gfx942SdmaCopyRequestV1 {
        source,
        source_offset: _,
        destination,
        destination_offset: _,
        copy_bytes,
    } = request;
    let (device, host) = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => (destination, source),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => (source, destination),
    };
    let attachment = allocation.attachment;
    let exact = device.belongs_to(attachment.queue)
        && host_binding.matches(&host)
        && device.storage_identity() == attachment.storage_identity
        && device.pool_generation() == attachment.pool_generation
        && device.requested_bytes() == attachment.logical_bytes
        && device.physical_bytes() == attachment.physical_bytes;
    if !exact {
        return Err((
            allocation,
            persistent_sdma_request(
                direction,
                host,
                host_offset,
                device,
                device_offset,
                copy_bytes,
            ),
        ));
    }
    let (storage, owner, pool_generation, logical_bytes) = device.into_bridge_parts();
    let Gfx942SdmaBufferStorageV1::Device(lease) = storage else {
        unreachable!("checked device-local SDMA storage")
    };
    if let Err((_, lease)) = allocation.owner.restore_local_native_from_sdma(lease) {
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            owner,
            pool_generation,
            logical_bytes,
        );
        return Err((
            allocation,
            persistent_sdma_request(
                direction,
                host,
                host_offset,
                device,
                device_offset,
                copy_bytes,
            ),
        ));
    }
    Ok((allocation, host))
}

#[allow(clippy::result_large_err)]
fn restore_completed_persistent_sdma_copy(
    allocation: Gfx942QueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    host_binding: Gfx942PersistentSdmaHostBindingV1,
    completed: Gfx942SdmaCompletedCopyV1,
) -> Result<
    (Gfx942QueuePersistentAllocationV1, Gfx942SdmaBufferV1),
    (Gfx942QueuePersistentAllocationV1, Gfx942SdmaCompletedCopyV1),
> {
    if completed.copy_bytes != copy_bytes
        || completed.source_offset
            != match direction {
                Gfx942PersistentSdmaDirectionV1::HostToDevice => host_offset,
                Gfx942PersistentSdmaDirectionV1::DeviceToHost => device_offset,
            }
        || completed.destination_offset
            != match direction {
                Gfx942PersistentSdmaDirectionV1::HostToDevice => device_offset,
                Gfx942PersistentSdmaDirectionV1::DeviceToHost => host_offset,
            }
    {
        return Err((allocation, completed));
    }
    let Gfx942SdmaCompletedCopyV1 {
        source,
        destination,
        copy_bytes,
        source_offset,
        destination_offset,
    } = completed;
    let request = Gfx942SdmaCopyRequestV1 {
        source,
        source_offset,
        destination,
        destination_offset,
        copy_bytes,
    };
    match restore_persistent_sdma_request(
        allocation,
        direction,
        host_offset,
        device_offset,
        copy_bytes,
        host_binding,
        request,
    ) {
        Ok(restored) => Ok(restored),
        Err((allocation, request)) => {
            let Gfx942SdmaCopyRequestV1 {
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            } = request;
            Err((
                allocation,
                Gfx942SdmaCompletedCopyV1 {
                    source,
                    destination,
                    copy_bytes,
                    source_offset,
                    destination_offset,
                },
            ))
        }
    }
}

#[allow(clippy::result_large_err)]
fn demote_persistent_sdma_custody_v1(
    allocation: Gfx942QueuePersistentAllocationV1,
    outstanding_buffers: usize,
) -> Result<
    (Gfx942SdmaBufferV1, usize),
    (
        Gfx942PersistentUseErrorV1,
        Gfx942QueuePersistentAllocationV1,
    ),
> {
    let Some(next_generation) = allocation.attachment.pool_generation.checked_add(1) else {
        return Err((Gfx942PersistentUseErrorV1::GenerationExhausted, allocation));
    };
    let Gfx942QueuePersistentAllocationV1 { owner, attachment } = allocation;
    let native = match owner.try_into_native() {
        Ok(native) => native,
        Err((error, owner)) => {
            return Err((
                error,
                Gfx942QueuePersistentAllocationV1 { owner, attachment },
            ));
        }
    };
    let crate::persistent_allocation::Gfx942PersistentNativeAllocationV1::Local(lease) = native
    else {
        unreachable!("validated persistent SDMA custody is a local mapping")
    };
    Ok((
        Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            attachment.queue,
            next_generation,
            attachment.logical_bytes,
        ),
        outstanding_buffers,
    ))
}

#[allow(clippy::result_large_err)]
fn promote_persistent_sdma_custody_v1(
    buffer: Gfx942SdmaBufferV1,
    native_queue_id: u32,
    engine_index: u32,
) -> Result<Gfx942QueuePersistentAllocationV1, Gfx942SdmaBufferV1> {
    let storage_identity = buffer.storage_identity();
    let physical_bytes = buffer.physical_bytes();
    let (storage, queue, pool_generation, logical_bytes) = buffer.into_bridge_parts();
    let Gfx942SdmaBufferStorageV1::Device(lease) = storage else {
        return Err(Gfx942SdmaBufferV1::from_bridge_parts(
            storage,
            queue,
            pool_generation,
            logical_bytes,
        ));
    };
    Ok(Gfx942QueuePersistentAllocationV1 {
        owner: Gfx942PersistentDeviceAllocationV1::from_local_mapping(lease),
        attachment: Gfx942PersistentSdmaAttachmentV1 {
            queue,
            native_queue_id,
            engine_index,
            pool_generation,
            logical_bytes,
            physical_bytes,
            storage_identity,
        },
    })
}

#[must_use = "a recoverable buffer-transition failure returns the mapped buffer authority"]
pub struct Gfx942SdmaBufferTransitionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovered: Option<Gfx942SdmaBufferV1>,
}

impl Gfx942SdmaBufferTransitionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(self) -> (ComputeAqlQueueSessionErrorV1, Option<Gfx942SdmaBufferV1>) {
        (self.error, self.recovered)
    }
}

impl fmt::Debug for Gfx942SdmaBufferTransitionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SdmaBufferTransitionFailureV1")
            .field("error", &self.error)
            .field("recovered", &self.recovered.is_some())
            .finish()
    }
}

impl fmt::Display for Gfx942SdmaBufferTransitionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for Gfx942SdmaBufferTransitionFailureV1 {}

/// Move-only identity receipt for a zero-copy SDMA-buffer role transition.
#[must_use = "retain this receipt until the fixed-dispatch data returns"]
pub struct Gfx942SdmaDispatchDataBridgeV1 {
    owner: QueueKeyV1,
    pool_generation: u64,
    logical_bytes: u64,
    physical_bytes: u64,
    storage_identity: Gfx942SdmaBufferStorageIdentityV1,
}

/// Full H2D completion split into the retained upload and dispatch-ready destination.
#[must_use = "both allocation authorities and the return receipt must be retained"]
pub struct Gfx942PromotedSdmaDestinationV1 {
    source: Gfx942SdmaBufferV1,
    data: Gfx942FixedDispatchDataV1,
    bridge: Gfx942SdmaDispatchDataBridgeV1,
}

impl Gfx942PromotedSdmaDestinationV1 {
    pub fn into_parts(
        self,
    ) -> (
        Gfx942SdmaBufferV1,
        Gfx942FixedDispatchDataV1,
        Gfx942SdmaDispatchDataBridgeV1,
    ) {
        (self.source, self.data, self.bridge)
    }
}

#[must_use = "a recoverable promotion failure returns the completed copy custody"]
pub struct Gfx942SdmaCompletedPromotionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovered: Option<Gfx942SdmaCompletedCopyV1>,
}

impl Gfx942SdmaCompletedPromotionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Gfx942SdmaCompletedCopyV1>,
    ) {
        (self.error, self.recovered)
    }
}

impl fmt::Debug for Gfx942SdmaCompletedPromotionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SdmaCompletedPromotionFailureV1")
            .field("error", &self.error)
            .field("recovered", &self.recovered.is_some())
            .finish()
    }
}

impl fmt::Display for Gfx942SdmaCompletedPromotionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for Gfx942SdmaCompletedPromotionFailureV1 {}

#[must_use = "a recoverable demotion failure returns dispatch data and its bridge"]
pub struct Gfx942SdmaDispatchDataDemotionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    recovered: Option<(Gfx942FixedDispatchDataV1, Gfx942SdmaDispatchDataBridgeV1)>,
}

impl Gfx942SdmaDispatchDataDemotionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<(Gfx942FixedDispatchDataV1, Gfx942SdmaDispatchDataBridgeV1)>,
    ) {
        (self.error, self.recovered)
    }
}

impl fmt::Debug for Gfx942SdmaDispatchDataDemotionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SdmaDispatchDataDemotionFailureV1")
            .field("error", &self.error)
            .field("recovered", &self.recovered.is_some())
            .finish()
    }
}

impl fmt::Display for Gfx942SdmaDispatchDataDemotionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for Gfx942SdmaDispatchDataDemotionFailureV1 {}

#[derive(Clone, Copy)]
struct DetachedReturningDestroyPreflightV1 {
    dispatch_attached: bool,
    detached_data_count: usize,
    detached_dispatch_generation: Option<u64>,
    detached_identity_count: usize,
    returned_data_count: usize,
    identity_mismatch: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GenericRecycledDispatchAccessV1 {
    Read,
    ReadInto,
    Snapshot,
    Overwrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SdmaPublicationModeV1 {
    Persistent,
    #[cfg(test)]
    DirectionalCopy(Gfx942PersistentSdmaDirectionV1),
    #[cfg(test)]
    DirectionalWindow(Gfx942PersistentSdmaDirectionV1),
    SameDeviceWindow,
    Ordinary,
    OrdinaryBatch,
    StripedBatch,
    ExecuteBatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PersistentRetainedControlReplayCustodyStageV1 {
    Input,
    Storage,
    Data,
    Attached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PersistentRetainedControlReplayDispositionV1 {
    RetryableInput,
    TerminalInput,
    TerminalStorage,
    TerminalData,
    TerminalAttached,
}

const fn classify_persistent_retained_control_replay_failure_v1(
    stage: PersistentRetainedControlReplayCustodyStageV1,
    loan_succeeded: bool,
    cancellation_succeeded: bool,
    session_healthy: bool,
) -> PersistentRetainedControlReplayDispositionV1 {
    match stage {
        PersistentRetainedControlReplayCustodyStageV1::Input
            if loan_succeeded
                && persistent_bind_retryable_v1(session_healthy, cancellation_succeeded) =>
        {
            PersistentRetainedControlReplayDispositionV1::RetryableInput
        }
        PersistentRetainedControlReplayCustodyStageV1::Input if cancellation_succeeded => {
            PersistentRetainedControlReplayDispositionV1::TerminalInput
        }
        PersistentRetainedControlReplayCustodyStageV1::Input
        | PersistentRetainedControlReplayCustodyStageV1::Attached => {
            PersistentRetainedControlReplayDispositionV1::TerminalAttached
        }
        PersistentRetainedControlReplayCustodyStageV1::Storage => {
            PersistentRetainedControlReplayDispositionV1::TerminalStorage
        }
        PersistentRetainedControlReplayCustodyStageV1::Data => {
            PersistentRetainedControlReplayDispositionV1::TerminalData
        }
    }
}

const fn persistent_bind_retryable_v1(session_healthy: bool, cancellation_succeeded: bool) -> bool {
    session_healthy && cancellation_succeeded
}

fn persistent_retained_control_replay_input_failure_v1(
    error: ComputeAqlQueueSessionErrorV1,
    input: Gfx942PersistentComputeInputV1,
    retryable: bool,
) -> Gfx942PersistentComputeBindFailureV1 {
    Gfx942PersistentComputeBindFailureV1 {
        error,
        custody: if retryable {
            Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input)
        } else {
            Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                Gfx942PersistentComputeBindTerminalCustodyV1 { input: Some(input) },
            )
        },
    }
}

struct PersistentRetainedControlReplayRequestV1 {
    input: Gfx942PersistentComputeInputV1,
    prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    dispatch: DispatchResourceOwnerV1,
    initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    control_identity: PersistentFixedDispatchControlIdentityV1,
    predecessor_generation: u64,
}

struct PersistentRetainedControlReplayDetachedV1 {
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    dispatch: DispatchResourceOwnerV1,
    authenticated_sha256: Option<[u8; 32]>,
    fully_initialized: bool,
}

struct PersistentRetainedControlReplayStorageV1 {
    replay: PersistentRetainedControlReplayDetachedV1,
    lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    control_identity: PersistentFixedDispatchControlIdentityV1,
    predecessor_generation: u64,
}

struct PersistentRetainedControlReplayDataV1 {
    replay: PersistentRetainedControlReplayDetachedV1,
    data: Option<Gfx942FixedDispatchDataV1>,
    control_identity: PersistentFixedDispatchControlIdentityV1,
    predecessor_generation: u64,
}

#[derive(Clone, Copy)]
struct PersistentRetainedControlReplayCommitV1 {
    attachment_generation: u64,
    next_attachment_generation: u64,
    storage_identity: Gfx942DeviceMemoryIdentityV1,
    effect: Gfx942PersistentComputeEffectV1,
    predecessor_generation: u64,
}

#[allow(clippy::large_enum_variant)]
enum PersistentRetainedControlReplayOutcomeV1 {
    BeforeDetach {
        request: PersistentRetainedControlReplayRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
    },
    AfterDetach {
        replay: PersistentRetainedControlReplayDetachedV1,
        custody: PersistentComputeTerminalNativeCustodyV1,
        error: ComputeAqlQueueSessionErrorV1,
    },
    Ready(PersistentRetainedControlReplayDetachedV1),
}

enum PersistentRetainedControlReplayPipelineOutcomeV1<Request, Storage, Data, Attached, Error> {
    BeforeDetach { request: Request, error: Error },
    Storage { storage: Storage, error: Error },
    Data { data: Data, error: Error },
    Attached { attached: Attached, error: Error },
    Ready(Attached),
}

enum PersistentRetainedControlReplayPipelineCustodyV1<Request, Storage, Data, Attached> {
    Empty,
    Input(Request),
    Storage(Storage),
    Data(Data),
    Attached(Attached),
}

impl<Request, Storage, Data, Attached>
    PersistentRetainedControlReplayPipelineCustodyV1<Request, Storage, Data, Attached>
{
    fn into_outcome<E>(
        self,
        result: Result<(), E>,
    ) -> PersistentRetainedControlReplayPipelineOutcomeV1<Request, Storage, Data, Attached, E> {
        use PersistentRetainedControlReplayPipelineOutcomeV1 as Outcome;
        match self {
            Self::Empty => unreachable!("an executed replay retains one phase"),
            Self::Input(request) => Outcome::BeforeDetach {
                request,
                error: result.expect_err("input phase failed"),
            },
            Self::Storage(storage) => Outcome::Storage {
                storage,
                error: result.expect_err("storage phase failed"),
            },
            Self::Data(data) => Outcome::Data {
                data,
                error: result.expect_err("data phase failed"),
            },
            Self::Attached(attached) => match result {
                Ok(()) => Outcome::Ready(attached),
                Err(error) => Outcome::Attached { attached, error },
            },
        }
    }
}

type PersistentRetainedControlReplayCustodyV1 = PersistentRetainedControlReplayPipelineCustodyV1<
    PersistentRetainedControlReplayRequestV1,
    PersistentRetainedControlReplayStorageV1,
    PersistentRetainedControlReplayDataV1,
    PersistentRetainedControlReplayDetachedV1,
>;

#[allow(clippy::too_many_arguments)]
fn execute_persistent_retained_control_replay_pipeline_v1<Context, Custody, Error>(
    context: &mut Context,
    custody: &mut Custody,
    mapped_facts: impl FnOnce(&mut Context, &mut Custody) -> Result<(), Error>,
    detach: impl FnOnce(&mut Context, &mut Custody) -> Result<(), Error>,
    construct: impl FnOnce(&mut Context, &mut Custody) -> Result<(), Error>,
    retain: impl FnOnce(&mut Context, &mut Custody) -> Result<(), Error>,
    final_audit: impl FnOnce(&mut Context, &mut Custody) -> Result<(), Error>,
) -> Result<(), Error> {
    mapped_facts(context, custody)?;
    detach(context, custody)?;
    construct(context, custody)?;
    retain(context, custody)?;
    final_audit(context, custody)
}

enum PersistentRetainedControlReplayLoanResolutionV1<Request, Outcome, Error> {
    Unopened {
        request: Request,
        error: Error,
    },
    Executed {
        outcome: Outcome,
        retake_error: Option<Error>,
    },
}

enum PersistentComputeCompletionObservationV1<Completed> {
    Pending(Gfx942CompletionBatchV1<1>),
    Ready(Completed),
}

struct PersistentComputeCompletedTransitionV1<Completed> {
    binding: PersistentComputeBindingKeyV1,
    attachment: PersistentComputeAttachmentV1,
    completed_use: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    identity: DispatchEpochIdentityV1,
    completion_occurrence: super::completion::CompletionBatchOccurrenceV1,
    completed: Completed,
}

#[allow(clippy::large_enum_variant)]
enum PersistentComputePollTransitionV1<Pending, Ready> {
    Pending(Pending),
    Ready(Ready),
}

enum PersistentComputePollAndRecycleTransitionV1<Pending, Recycled, Midpoint> {
    Pending(Pending),
    Recycled {
        recycled: Recycled,
        completion_observed_at: Midpoint,
    },
}

enum PersistentComputePollAndRecycleTransitionFailureV1<PollFailure, RecycleFailure> {
    Poll(PollFailure),
    Recycle(RecycleFailure),
}

fn execute_persistent_compute_poll_and_recycle_v1<
    Context,
    Pending,
    Completed,
    Recycled,
    Midpoint,
    PollFailure,
    RecycleFailure,
>(
    context: &mut Context,
    poll: impl FnOnce(
        &mut Context,
    ) -> Result<PersistentComputePollTransitionV1<Pending, Completed>, PollFailure>,
    midpoint: impl FnOnce(&mut Context) -> Midpoint,
    recycle: impl FnOnce(&mut Context, Completed) -> Result<Recycled, RecycleFailure>,
) -> Result<
    PersistentComputePollAndRecycleTransitionV1<Pending, Recycled, Midpoint>,
    PersistentComputePollAndRecycleTransitionFailureV1<PollFailure, RecycleFailure>,
> {
    let completed =
        match poll(context).map_err(PersistentComputePollAndRecycleTransitionFailureV1::Poll)? {
            PersistentComputePollTransitionV1::Pending(pending) => {
                return Ok(PersistentComputePollAndRecycleTransitionV1::Pending(
                    pending,
                ));
            }
            PersistentComputePollTransitionV1::Ready(completed) => completed,
        };
    let completion_observed_at = midpoint(context);
    let recycled = recycle(context, completed)
        .map_err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle)?;
    Ok(PersistentComputePollAndRecycleTransitionV1::Recycled {
        recycled,
        completion_observed_at,
    })
}

enum PersistentComputeWaitAndRecycleTransitionV1<Pending, Recycled, Midpoint> {
    Timeout {
        pending: Pending,
        observations: u64,
    },
    Recycled {
        recycled: Recycled,
        completion_observed_at: Midpoint,
        observations: u64,
    },
}

fn execute_persistent_compute_wait_and_recycle_v1<Context, Pending, Recycled, Midpoint, Failure>(
    context: &mut Context,
    mut pending: Pending,
    mut poll_and_recycle: impl FnMut(
        &mut Context,
        Pending,
    ) -> Result<
        PersistentComputePollAndRecycleTransitionV1<Pending, Recycled, Midpoint>,
        Failure,
    >,
    mut timeout_after_pending: impl FnMut(&mut Context) -> bool,
) -> Result<PersistentComputeWaitAndRecycleTransitionV1<Pending, Recycled, Midpoint>, Failure> {
    let mut observations = 0_u64;
    loop {
        observations = observations.saturating_add(1);
        match poll_and_recycle(context, pending)? {
            PersistentComputePollAndRecycleTransitionV1::Pending(next) => {
                pending = next;
                if observations == u64::MAX || timeout_after_pending(context) {
                    return Ok(PersistentComputeWaitAndRecycleTransitionV1::Timeout {
                        pending,
                        observations,
                    });
                }
            }
            PersistentComputePollAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
            } => {
                return Ok(PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                    recycled,
                    completion_observed_at,
                    observations,
                });
            }
        }
    }
}

fn resolve_persistent_retained_control_replay_loan_v1<Request, Outcome, Error>(
    request: Option<Request>,
    outcome: Option<Outcome>,
    loan: Result<(), Error>,
    missing_error: impl FnOnce() -> Error,
) -> PersistentRetainedControlReplayLoanResolutionV1<Request, Outcome, Error> {
    match outcome {
        Some(outcome) => PersistentRetainedControlReplayLoanResolutionV1::Executed {
            outcome,
            retake_error: loan.err(),
        },
        None => PersistentRetainedControlReplayLoanResolutionV1::Unopened {
            request: request.expect("unopened replay loan retains its request"),
            error: loan.err().unwrap_or_else(missing_error),
        },
    }
}

fn persistent_compute_input_allocation_mut_v1(
    input: &mut Gfx942PersistentComputeInputV1,
) -> &mut Gfx942DirectionalQueuePersistentAllocationV1 {
    match input {
        Gfx942PersistentComputeInputV1::Uninitialized(allocation)
        | Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) => allocation,
        Gfx942PersistentComputeInputV1::Initialized(ready) => &mut ready.allocation,
    }
}

fn quarantine_persistent_retained_control_replay_prepared_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
) -> PersistentComputeUseStateV1 {
    match owner.quarantine_prepared(
        prepared,
        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
    ) {
        Ok(()) => PersistentComputeUseStateV1::Quarantined,
        Err(failure) => {
            let (_, prepared) = failure.into_parts();
            PersistentComputeUseStateV1::Prepared(prepared)
        }
    }
}

fn cancel_persistent_compute_reserved_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    reserved: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
) -> Result<(), Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>> {
    owner
        .cancel_reserved(reserved)
        .map_err(|failure| failure.into_parts().1)
}

fn cancel_persistent_compute_prepared_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
) -> Result<(), Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>> {
    owner
        .cancel_prepared(prepared)
        .map_err(|failure| failure.into_parts().1)
}

enum PersistentBindCancellationDispositionV1<Lease> {
    Retryable,
    Terminal(Lease),
}

fn classify_persistent_bind_cancellation_v1<Lease>(
    cancellation: Result<(), Lease>,
) -> PersistentBindCancellationDispositionV1<Lease> {
    match cancellation {
        Ok(()) => PersistentBindCancellationDispositionV1::Retryable,
        Err(lease) => PersistentBindCancellationDispositionV1::Terminal(lease),
    }
}

fn quarantine_persistent_compute_prepared_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> PersistentComputeUseStateV1 {
    match owner.quarantine_prepared(prepared, reason) {
        Ok(()) => PersistentComputeUseStateV1::Quarantined,
        Err(failure) => {
            let (_, prepared) = failure.into_parts();
            PersistentComputeUseStateV1::Prepared(prepared)
        }
    }
}

fn quarantine_persistent_compute_published_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> PersistentComputeUseStateV1 {
    match owner.quarantine_published(published, reason) {
        Ok(()) => PersistentComputeUseStateV1::Quarantined,
        Err(failure) => {
            let (_, published) = failure.into_parts();
            PersistentComputeUseStateV1::Published(published)
        }
    }
}

fn quarantine_persistent_compute_completed_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    completed: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> PersistentComputeUseStateV1 {
    match owner.quarantine_completed(completed, reason) {
        Ok(()) => PersistentComputeUseStateV1::Quarantined,
        Err(failure) => {
            let (_, completed) = failure.into_parts();
            PersistentComputeUseStateV1::Completed(completed)
        }
    }
}

fn quarantine_persistent_compute_recycled_v1(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    completed: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> PersistentComputeUseStateV1 {
    match owner.quarantine_completed(completed, reason) {
        Ok(()) => PersistentComputeUseStateV1::Quarantined,
        Err(failure) => {
            let (_, completed) = failure.into_parts();
            PersistentComputeUseStateV1::Recycled(completed)
        }
    }
}

trait PersistentComputeLedgerEntryV1 {
    fn owner_and_state_v1(
        &mut self,
    ) -> (
        &mut Gfx942PersistentDeviceAllocationV1,
        &mut PersistentComputeUseStateV1,
    );
}

impl PersistentComputeLedgerEntryV1 for PersistentComputeAttachmentV1 {
    fn owner_and_state_v1(
        &mut self,
    ) -> (
        &mut Gfx942PersistentDeviceAllocationV1,
        &mut PersistentComputeUseStateV1,
    ) {
        (&mut self.allocation.owner, &mut self.state)
    }
}

impl PersistentComputeLedgerEntryV1 for PersistentComputeAttachmentEntryV1 {
    fn owner_and_state_v1(
        &mut self,
    ) -> (
        &mut Gfx942PersistentDeviceAllocationV1,
        &mut PersistentComputeUseStateV1,
    ) {
        (&mut self.allocation.owner, &mut self.state)
    }
}

fn quarantine_persistent_compute_entries_v1<const B: usize, Entry>(
    entries: [&mut Entry; B],
    reason: Gfx942PersistentQuarantineReasonV1,
) where
    Entry: PersistentComputeLedgerEntryV1,
{
    for entry in entries {
        let (owner, state) = entry.owner_and_state_v1();
        let current = core::mem::replace(state, PersistentComputeUseStateV1::Quarantined);
        *state = match current {
            PersistentComputeUseStateV1::Reserved(reserved) => {
                match cancel_persistent_compute_reserved_v1(owner, reserved) {
                    Ok(()) => PersistentComputeUseStateV1::Quarantined,
                    Err(reserved) => PersistentComputeUseStateV1::Reserved(reserved),
                }
            }
            PersistentComputeUseStateV1::Prepared(prepared) => {
                quarantine_persistent_compute_prepared_v1(owner, prepared, reason)
            }
            PersistentComputeUseStateV1::Published(published) => {
                quarantine_persistent_compute_published_v1(owner, published, reason)
            }
            PersistentComputeUseStateV1::Completed(completed) => {
                quarantine_persistent_compute_completed_v1(owner, completed, reason)
            }
            PersistentComputeUseStateV1::Recycled(completed) => {
                quarantine_persistent_compute_recycled_v1(owner, completed, reason)
            }
            PersistentComputeUseStateV1::Quarantined => PersistentComputeUseStateV1::Quarantined,
        };
    }
}

fn cancel_persistent_compute_prepublication_entries_v1<const B: usize, Entry>(
    entries: [&mut Entry; B],
) -> bool
where
    Entry: PersistentComputeLedgerEntryV1,
{
    let mut exact = true;
    for entry in entries {
        let (owner, state) = entry.owner_and_state_v1();
        let current = core::mem::replace(state, PersistentComputeUseStateV1::Quarantined);
        *state = match current {
            PersistentComputeUseStateV1::Reserved(reserved) => {
                match cancel_persistent_compute_reserved_v1(owner, reserved) {
                    Ok(()) => PersistentComputeUseStateV1::Quarantined,
                    Err(reserved) => {
                        exact = false;
                        PersistentComputeUseStateV1::Reserved(reserved)
                    }
                }
            }
            PersistentComputeUseStateV1::Prepared(prepared) => {
                match cancel_persistent_compute_prepared_v1(owner, prepared) {
                    Ok(()) => PersistentComputeUseStateV1::Quarantined,
                    Err(prepared) => {
                        exact = false;
                        PersistentComputeUseStateV1::Prepared(prepared)
                    }
                }
            }
            PersistentComputeUseStateV1::Quarantined => PersistentComputeUseStateV1::Quarantined,
            other => {
                exact = false;
                other
            }
        };
    }
    exact
}

fn publish_persistent_compute_entries_v1<const B: usize, Entry>(entries: [&mut Entry; B]) -> bool
where
    Entry: PersistentComputeLedgerEntryV1,
{
    for entry in entries {
        let (owner, state) = entry.owner_and_state_v1();
        let current = core::mem::replace(state, PersistentComputeUseStateV1::Quarantined);
        let PersistentComputeUseStateV1::Prepared(prepared) = current else {
            *state = current;
            return false;
        };
        match owner.publish(prepared) {
            Ok(published) => *state = PersistentComputeUseStateV1::Published(published),
            Err(failure) => {
                let (_, prepared) = failure.into_parts();
                *state = PersistentComputeUseStateV1::Prepared(prepared);
                return false;
            }
        }
    }
    true
}

fn complete_persistent_compute_entries_v1<const B: usize, Entry>(entries: [&mut Entry; B]) -> bool
where
    Entry: PersistentComputeLedgerEntryV1,
{
    for entry in entries {
        let (owner, state) = entry.owner_and_state_v1();
        let current = core::mem::replace(state, PersistentComputeUseStateV1::Quarantined);
        let PersistentComputeUseStateV1::Published(published) = current else {
            *state = current;
            return false;
        };
        match owner.complete(published) {
            Ok(completed) => *state = PersistentComputeUseStateV1::Completed(completed),
            Err(failure) => {
                let (_, published) = failure.into_parts();
                *state = PersistentComputeUseStateV1::Published(published);
                return false;
            }
        }
    }
    true
}

fn recycle_persistent_compute_entries_v1<const B: usize, Entry>(entries: [&mut Entry; B]) -> bool
where
    Entry: PersistentComputeLedgerEntryV1,
{
    for entry in entries {
        let (_, state) = entry.owner_and_state_v1();
        let current = core::mem::replace(state, PersistentComputeUseStateV1::Quarantined);
        let PersistentComputeUseStateV1::Completed(completed) = current else {
            *state = current;
            return false;
        };
        *state = PersistentComputeUseStateV1::Recycled(completed);
    }
    true
}

fn three_binding_entries_into_inputs_v1(
    entries: [PersistentComputeAttachmentEntryV1; 3],
) -> Gfx942ThreeBindingPersistentComputeInputsV1 {
    Gfx942ThreeBindingPersistentComputeInputsV1::new(entries.map(|entry| {
        Gfx942PersistentComputeInputV1::from_parts(
            entry.allocation,
            entry.authenticated_sha256,
            entry.fully_initialized,
        )
    }))
}

#[cfg(test)]
thread_local! {
    static THREE_BINDING_BIND_VALIDATION_ONLY_V1: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
    static THREE_BINDING_BIND_VALIDATION_EFFECTS_V1:
        std::cell::Cell<Option<[DeviceDataEffectV1; 3]>> = const {
            std::cell::Cell::new(None)
        };
}

#[cfg(test)]
fn take_three_binding_bind_validation_only_v1() -> bool {
    THREE_BINDING_BIND_VALIDATION_ONLY_V1.replace(false)
}

#[cfg(not(test))]
const fn take_three_binding_bind_validation_only_v1() -> bool {
    false
}

#[cfg(test)]
fn record_three_binding_bind_validation_effects_v1(effects: [DeviceDataEffectV1; 3]) {
    THREE_BINDING_BIND_VALIDATION_EFFECTS_V1.set(Some(effects));
}

#[cfg(test)]
thread_local! {
    static PERSISTENT_UNWIND_PROCESS_GATE_RECORDED_V1: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
}

fn poison_process_global_after_persistent_unwind_v1() {
    #[cfg(not(test))]
    permanently_poison_process_global_kfd_runtime_gate_v1();
    #[cfg(test)]
    PERSISTENT_UNWIND_PROCESS_GATE_RECORDED_V1.set(true);
}

#[cfg(test)]
fn take_persistent_unwind_process_gate_record_v1() -> bool {
    PERSISTENT_UNWIND_PROCESS_GATE_RECORDED_V1.replace(false)
}

#[cfg(test)]
thread_local! {
    static DISPATCH_TERMINAL_PROCESS_GATE_RECORDED_V1: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
    static LANE_UNWIND_PROCESS_GATE_RECORDED_V1: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
}

fn poison_process_global_after_dispatch_terminal_v1() {
    #[cfg(not(test))]
    permanently_poison_process_global_kfd_runtime_gate_v1();
    #[cfg(test)]
    DISPATCH_TERMINAL_PROCESS_GATE_RECORDED_V1.set(true);
}

fn poison_process_global_after_lane_unwind_v1() {
    #[cfg(not(test))]
    permanently_poison_process_global_kfd_runtime_gate_v1();
    #[cfg(test)]
    LANE_UNWIND_PROCESS_GATE_RECORDED_V1.set(true);
}

#[cfg(test)]
fn take_dispatch_terminal_process_gate_record_v1() -> bool {
    DISPATCH_TERMINAL_PROCESS_GATE_RECORDED_V1.replace(false)
}

#[cfg(test)]
fn take_lane_unwind_process_gate_record_v1() -> bool {
    LANE_UNWIND_PROCESS_GATE_RECORDED_V1.replace(false)
}

fn admit_sdma_publication_while_compute_detached(
    terminal_poisoned: bool,
    persistent_compute_attached: bool,
    mode: SdmaPublicationModeV1,
) -> Result<SdmaPublicationModeV1, Gfx942DispatchBindingErrorV1> {
    if terminal_poisoned {
        Err(Gfx942DispatchBindingErrorV1::Poisoned)
    } else if persistent_compute_attached {
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    } else {
        Ok(mode)
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn preserve_ordinary_sdma_publication_custody_v1(
    persistent_compute_attached: bool,
    source: Gfx942SdmaBufferV1,
    destination: Gfx942SdmaBufferV1,
) -> Result<(Gfx942SdmaBufferV1, Gfx942SdmaBufferV1), Gfx942SdmaSubmissionFailureV1> {
    match admit_sdma_publication_while_compute_detached(
        false,
        persistent_compute_attached,
        SdmaPublicationModeV1::Ordinary,
    ) {
        Ok(_) => Ok((source, destination)),
        Err(error) => Err(Gfx942SdmaSubmissionFailureV1 {
            error: error.into(),
            recovered: Some((source, destination)),
        }),
    }
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn preserve_directional_window_sdma_publication_custody_v1(
    persistent_compute_attached: bool,
    direction: Gfx942PersistentSdmaDirectionV1,
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
    ),
    Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1,
> {
    match admit_sdma_publication_while_compute_detached(
        false,
        persistent_compute_attached,
        SdmaPublicationModeV1::DirectionalWindow(direction),
    ) {
        Ok(_) => Ok((allocation, host)),
        Err(error) => Err(Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
            error: error.into(),
            custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                allocation,
                host,
            },
        }),
    }
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn preserve_persistent_compute_bind_input_for_sdma_quiescence_v1(
    input: Gfx942PersistentComputeInputV1,
    directional_sdma_quiescent: bool,
) -> Result<Gfx942PersistentComputeInputV1, Gfx942PersistentComputeBindFailureV1> {
    if directional_sdma_quiescent {
        Ok(input)
    } else {
        Err(Gfx942PersistentComputeBindFailureV1 {
            error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
            custody: Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input),
        })
    }
}

fn admit_generic_recycled_dispatch_access(
    terminal_poisoned: bool,
    persistent_compute_attached: bool,
    operation: GenericRecycledDispatchAccessV1,
) -> Result<GenericRecycledDispatchAccessV1, Gfx942DispatchBindingErrorV1> {
    if terminal_poisoned {
        Err(Gfx942DispatchBindingErrorV1::Poisoned)
    } else if persistent_compute_attached {
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    } else {
        Ok(operation)
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn preserve_persistent_compute_ready_preflight_custody_v1(
    completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
    terminal_poisoned: bool,
    preflight: Result<(), ComputeAqlQueueSessionErrorV1>,
) -> Result<Gfx942DirectionalPersistentSdmaWindowCompletedV1, Gfx942PersistentComputeReadyFailureV1>
{
    match preflight {
        Ok(()) => Ok(completed),
        Err(error) if terminal_poisoned => Err(Gfx942PersistentComputeReadyFailureV1 {
            error,
            custody: Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(
                Gfx942PersistentComputeReadyTerminalCustodyV1 { completed },
            ),
        }),
        Err(error) => Err(Gfx942PersistentComputeReadyFailureV1 {
            error,
            custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable(
                completed.into_parts(),
            ),
        }),
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn preserve_persistent_compute_ready_affiliation_v1(
    completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
    queue: QueueKeyV1,
    terminal_poisoned: bool,
) -> Result<Gfx942DirectionalPersistentSdmaWindowCompletedV1, Gfx942PersistentComputeReadyFailureV1>
{
    if completed.belongs_to(queue) {
        Ok(completed)
    } else {
        Err(Gfx942PersistentComputeReadyFailureV1 {
            error: if terminal_poisoned {
                Gfx942DispatchBindingErrorV1::Poisoned.into()
            } else {
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute H2D completion owner substitution",
                )
            },
            custody: Gfx942PersistentComputeReadyFailureCustodyV1::ForeignQueue(completed),
        })
    }
}

pub(crate) fn terminal_persistent_compute_ready_hash_failure_v1(
    error: ComputeAqlQueueSessionErrorV1,
    completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
) -> Gfx942PersistentComputeReadyFailureV1 {
    Gfx942PersistentComputeReadyFailureV1 {
        error,
        custody: Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(
            Gfx942PersistentComputeReadyTerminalCustodyV1 { completed },
        ),
    }
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub(crate) fn admit_directional_persistent_sdma_copy_input_v1(
    queue: QueueKeyV1,
    terminal_poisoned: bool,
    mut allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host: Gfx942SdmaBufferV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
    ),
    Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
> {
    let retryable = |error, allocation, host| Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
        error,
        custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable { allocation, host },
    };
    if allocation.attachment.queue != queue || !host.belongs_to(queue) {
        return Err(retryable(
            ComputeAqlQueueSessionErrorV1::Contract(
                "directional persistent SDMA submission owner substitution",
            ),
            allocation,
            host,
        ));
    }
    if terminal_poisoned {
        allocation
            .owner
            .quarantine_for_caller_reported_currentness_loss();
        return Err(Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
            error: ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ),
            custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: None,
                    state: Gfx942DirectionalPersistentSdmaTerminalStateV1::AdmissionRestored {
                        allocation,
                        host,
                    },
                },
            ),
        });
    }
    if host.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
        || copy_bytes == 0
        || copy_bytes > crate::sdma::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
        || host_offset
            .checked_add(u64::from(copy_bytes))
            .is_none_or(|end| end > host.requested_bytes())
        || device_offset
            .checked_add(u64::from(copy_bytes))
            .is_none_or(|end| end > allocation.byte_len())
        || !allocation.owner.local_native_is_attached_for_sdma()
    {
        return Err(retryable(
            ComputeAqlQueueSessionErrorV1::Contract(
                "directional persistent SDMA submission owner, buffer, or range",
            ),
            allocation,
            host,
        ));
    }
    Ok((allocation, host))
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub(crate) fn admit_directional_persistent_sdma_window_input_v1(
    queue: QueueKeyV1,
    terminal_poisoned: bool,
    mut allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host: Gfx942SdmaBufferV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
        usize,
    ),
    Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1,
> {
    let retryable =
        |error, allocation, host| Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                allocation,
                host,
            },
        };
    if allocation.attachment.queue != queue || !host.belongs_to(queue) {
        return Err(retryable(
            ComputeAqlQueueSessionErrorV1::Contract(
                "directional persistent SDMA window owner substitution",
            ),
            allocation,
            host,
        ));
    }
    if terminal_poisoned {
        allocation
            .owner
            .quarantine_for_caller_reported_currentness_loss();
        return Err(Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
            error: ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ),
            custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                    direction,
                    sequence: None,
                    packet_count: persistent_sdma_window_packet_count(copy_bytes).unwrap_or(0),
                    state:
                        Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::AdmissionRestored {
                            allocation,
                            host,
                        },
                },
            ),
        });
    }
    let packet_count = match persistent_sdma_window_packet_count(copy_bytes) {
        Ok(packet_count) => packet_count,
        Err(error) => return Err(retryable(error.into(), allocation, host)),
    };
    if host.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
        || host_offset
            .checked_add(u64::from(copy_bytes))
            .is_none_or(|end| end > host.requested_bytes())
        || device_offset
            .checked_add(u64::from(copy_bytes))
            .is_none_or(|end| end > allocation.byte_len())
        || !allocation.owner.local_native_is_attached_for_sdma()
    {
        return Err(retryable(
            ComputeAqlQueueSessionErrorV1::Contract(
                "directional persistent SDMA window owner, buffer, or range",
            ),
            allocation,
            host,
        ));
    }
    Ok((allocation, host, packet_count))
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub(crate) fn admit_same_device_persistent_sdma_window_input_v1(
    queue: QueueKeyV1,
    terminal_poisoned: bool,
    mut source: Gfx942DirectionalQueuePersistentAllocationV1,
    source_offset: u64,
    mut destination: Gfx942DirectionalQueuePersistentAllocationV1,
    destination_offset: u64,
    copy_bytes: u32,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
    ),
    Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1,
> {
    let retryable =
        |error, source, destination| Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                source,
                destination,
            },
        };
    if source.attachment.queue != queue || destination.attachment.queue != queue {
        return Err(retryable(
            ComputeAqlQueueSessionErrorV1::Contract(
                "same-device persistent SDMA owner substitution",
            ),
            source,
            destination,
        ));
    }
    if terminal_poisoned {
        source
            .owner
            .quarantine_for_caller_reported_currentness_loss();
        destination
            .owner
            .quarantine_for_caller_reported_currentness_loss();
        let packet_count = persistent_sdma_window_packet_count(copy_bytes).unwrap_or(0);
        return Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
            error: ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ),
            custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                    source_sequence: None,
                    destination_sequence: None,
                    descriptor: same_device_persistent_sdma_descriptor_v1(
                        source_offset,
                        destination_offset,
                        copy_bytes,
                        packet_count,
                    ),
                    state: Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::AdmissionRestored {
                        source,
                        destination,
                    },
                },
            ),
        });
    }
    let packet_count = match persistent_sdma_window_packet_count(copy_bytes) {
        Ok(packet_count) => packet_count,
        Err(error) => return Err(retryable(error.into(), source, destination)),
    };
    let descriptor = same_device_persistent_sdma_descriptor_v1(
        source_offset,
        destination_offset,
        copy_bytes,
        packet_count,
    );
    if source.attachment.pair != destination.attachment.pair
        || source.attachment.storage_identity == destination.attachment.storage_identity
        || source_offset
            .checked_add(u64::from(copy_bytes))
            .is_none_or(|end| end > source.byte_len())
        || destination_offset
            .checked_add(u64::from(copy_bytes))
            .is_none_or(|end| end > destination.byte_len())
        || !source.owner.local_native_is_attached_for_sdma()
        || !destination.owner.local_native_is_attached_for_sdma()
    {
        return Err(retryable(
            ComputeAqlQueueSessionErrorV1::Contract(
                "same-device persistent SDMA owner, identity, or range",
            ),
            source,
            destination,
        ));
    }
    Ok((source, destination, descriptor))
}

fn admit_detached_returning_destroy(
    terminal_poisoned: &mut bool,
    preflight: DetachedReturningDestroyPreflightV1,
) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
    if *terminal_poisoned {
        return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
    }
    if preflight.dispatch_attached || preflight.detached_dispatch_generation.is_none() {
        *terminal_poisoned = true;
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
    }
    if preflight.detached_data_count > super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
        *terminal_poisoned = true;
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "detached dispatch-data ledger bound",
        ));
    }
    if preflight.detached_identity_count != preflight.detached_data_count {
        *terminal_poisoned = true;
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "detached dispatch-data identity ledger cardinality",
        ));
    }
    if preflight.returned_data_count != preflight.detached_data_count {
        *terminal_poisoned = true;
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: preflight
                .returned_data_count
                .min(preflight.detached_data_count),
            detail: "detached returning-destroy cardinality",
        }
        .into());
    }
    if let Some(index) = preflight.identity_mismatch {
        *terminal_poisoned = true;
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index,
            detail: "detached returning-destroy storage identity",
        }
        .into());
    }
    let generation = preflight
        .detached_dispatch_generation
        .expect("checked detached generation");
    Ok(generation)
}

#[derive(Default)]
struct SdmaDevicePoolConfigurationV1 {
    limits: Option<Gfx942DevicePoolLimitsV1>,
    activity_started: bool,
}

impl SdmaDevicePoolConfigurationV1 {
    fn configure(
        &mut self,
        limits: Gfx942DevicePoolLimitsV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.limits.is_some() || self.activity_started {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA device pool configuration is immutable after configuration or activity",
            ));
        }
        self.limits = Some(limits);
        Ok(())
    }

    fn begin_activity(&mut self) {
        self.activity_started = true;
    }
}

#[must_use = "queue destruction and resource return are explicit"]
pub struct ComputeAqlQueueSessionV1 {
    engine: Option<NativeQueueEngineV1<LinuxNativeQueueBackendV1>>,
    key: QueueKeyV1,
    compute_lane_session: QueueKeyV1,
    doorbell: Option<LinuxDoorbellSliceV1>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    completion_signals: Option<CompletionSignalAuthority>,
    completion_owner: QueueOwnerSlotV1<CompletionSignalArenaOwnerV1>,
    dependency_owner: QueueOwnerSlotV1<ComputeDependencySessionOwnerV1>,
    terminal_dependency: Option<Box<TerminalComputeDependencyTargetUseV1>>,
    dispatch: Option<DispatchResourceOwnerV1>,
    unpublished_dispatch: UnpublishedDispatchStateV1,
    detached_data_count: usize,
    detached_dispatch_generation: Option<u64>,
    detached_data_identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    detached_next_insertion_index: Option<usize>,
    persistent_compute: Option<BoundedPersistentComputeAttachmentV1>,
    #[cfg(test)]
    persistent_compute_test_release: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)>,
    next_persistent_compute_generation: u64,
    exception: Option<QueueExceptionStateV1>,
    sdma: Option<Gfx942SdmaQueueSetV1>,
    striped_sdma: Option<Gfx942SdmaQueueSetV1>,
    sdma_outstanding_buffers: usize,
    sdma_pool_free: Vec<Gfx942SdmaBufferV1>,
    sdma_pool_reuse_count: u64,
    sdma_device_pool: SdmaDevicePoolConfigurationV1,
    // Both policies share sdma_device_pool's irreversible activity latch.
    sdma_host_pool_limits: Option<Gfx942HostPoolLimitsV1>,
    terminal_poisoned: bool,
    observation: ComputeAqlQueueObservationV1,
    auxiliary_compute_lanes: Vec<AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1>>,
}

/// Fixed failures for direct coherent reads into caller-owned storage.
/// `NativeUncertain` requires terminal retention; no partial bytes are accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaHostReadIntoErrorV1 {
    InvalidRange,
    InvalidBuffer,
    Unavailable,
    NativeUncertain,
}

fn preflight_sdma_host_read_into_v1(
    buffer: &Gfx942SdmaBufferV1,
    queue: QueueKeyV1,
    offset: u64,
    destination_len: usize,
) -> Result<(), Gfx942SdmaHostReadIntoErrorV1> {
    if !buffer.belongs_to(queue)
        || buffer.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
        || buffer.pool_generation() == 0
        || buffer.requested_bytes() > buffer.physical_bytes()
    {
        return Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer);
    }
    let bytes =
        u64::try_from(destination_len).map_err(|_| Gfx942SdmaHostReadIntoErrorV1::InvalidRange)?;
    if bytes == 0
        || offset
            .checked_add(bytes)
            .is_none_or(|end| end > buffer.requested_bytes())
    {
        return Err(Gfx942SdmaHostReadIntoErrorV1::InvalidRange);
    }
    Ok(())
}

/// Stable queue-local lane selected inside one exact shared KFD VM session.
///
/// Lane zero is the original queue. Additional lanes own distinct KFD queue,
/// ring, doorbell, completion, exception-event, and dispatch authorities. The
/// private session occurrence and generation prevent cross-session and stale
/// slot substitution.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ComputeAqlQueueLaneV1 {
    session: QueueKeyV1,
    ordinal: usize,
    generation: u64,
}

impl ComputeAqlQueueLaneV1 {
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

impl fmt::Debug for ComputeAqlQueueLaneV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComputeAqlQueueLaneV1")
            .field("ordinal", &self.ordinal)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// Move-only addressless event for one actually published compute packet.
/// Its source queue, signal mapping, slot, generations, packet ID, and native
/// signal address remain private.
///
/// ```compile_fail
/// use fe2o3_kfd::{ComputeAqlQueueSessionV1, Gfx942ComputeDependencyEventV1};
/// fn release_twice(
///     queue: &mut ComputeAqlQueueSessionV1,
///     event: Gfx942ComputeDependencyEventV1,
/// ) {
///     let _first = queue.release_compute_dependency_event_v1(event);
///     let _second = queue.release_compute_dependency_event_v1(event);
/// }
/// ```
#[must_use = "a compute dependency event must be consumed or explicitly released"]
pub struct Gfx942ComputeDependencyEventV1 {
    lane: ComputeAqlQueueLaneV1,
    event: super::completion::Gfx942ComputeEventOccurrenceV1,
}

impl fmt::Debug for Gfx942ComputeDependencyEventV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencyEventV1")
            .field("binding_state", &self.event.binding_state())
            .finish_non_exhaustive()
    }
}

/// One published fixed batch and its exact per-packet dependency events.
#[must_use = "the published source batch and its events retain completion authority"]
pub struct Gfx942ComputeDependencySourceBatchV1<const N: usize> {
    batch: Gfx942DispatchBatchV1<N>,
    events: Vec<Gfx942ComputeDependencyEventV1>,
}

impl<const N: usize> fmt::Debug for Gfx942ComputeDependencySourceBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencySourceBatchV1")
            .field("packet_count", &N)
            .field("event_count", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl<const N: usize> Gfx942ComputeDependencySourceBatchV1<N> {
    pub fn into_parts(
        self,
    ) -> (
        Gfx942DispatchBatchV1<N>,
        Vec<Gfx942ComputeDependencyEventV1>,
    ) {
        (self.batch, self.events)
    }
}

/// Linear custody for one B37 dependency-ordered target dispatch.
///
/// ```compile_fail
/// use fe2o3_kfd::{ComputeAqlQueueSessionV1, Gfx942ComputeDependencyDispatchV1};
/// fn poll_twice(
///     queue: &mut ComputeAqlQueueSessionV1,
///     dispatch: Box<Gfx942ComputeDependencyDispatchV1>,
/// ) {
///     let _first = queue.poll_compute_dependency_dispatch_v1(dispatch);
///     let _second = queue.poll_compute_dependency_dispatch_v1(dispatch);
/// }
/// ```
#[must_use = "a dependent dispatch must be observed or retained for process teardown"]
pub struct Gfx942ComputeDependencyDispatchV1 {
    lane: ComputeAqlQueueLaneV1,
    source_lane: ComputeAqlQueueLaneV1,
    identity: DispatchEpochIdentityV1,
    published: Option<PublishedComputeDependencyTargetUseV1>,
}

impl fmt::Debug for Gfx942ComputeDependencyDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencyDispatchV1")
            .finish_non_exhaustive()
    }
}

/// Exact dependent completion after its source-reader roster was released.
#[must_use = "the completed dependent dispatch must be recycled"]
pub struct Gfx942CompletedComputeDependencyDispatchV1 {
    completed: Gfx942CompletedDispatchBatchV1<1>,
    dependency_count: u16,
}

impl Gfx942CompletedComputeDependencyDispatchV1 {
    pub const fn dependency_count(&self) -> u16 {
        self.dependency_count
    }

    pub fn into_batch(self) -> Gfx942CompletedDispatchBatchV1<1> {
        self.completed
    }
}

impl fmt::Debug for Gfx942CompletedComputeDependencyDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletedComputeDependencyDispatchV1")
            .field("dependency_count", &self.dependency_count)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Gfx942ComputeDependencyPollV1 {
    Pending(Box<Gfx942ComputeDependencyDispatchV1>),
    Ready(Box<Gfx942CompletedComputeDependencyDispatchV1>),
}

/// Fixed-dispatch recycle failure. Pure resource-phase preselection or an exact
/// dependency-pin rejection returns the unchanged completed dispatch for retry.
#[must_use = "a retryable completed dispatch must be recovered from this failure"]
pub struct Gfx942FixedDispatchRecycleFailureV1<const N: usize> {
    error: ComputeAqlQueueSessionErrorV1,
    retryable_completed: Option<Gfx942CompletedDispatchBatchV1<N>>,
}

impl<const N: usize> Gfx942FixedDispatchRecycleFailureV1<N> {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Gfx942CompletedDispatchBatchV1<N>>,
    ) {
        (self.error, self.retryable_completed)
    }
}

impl<const N: usize> fmt::Debug for Gfx942FixedDispatchRecycleFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942FixedDispatchRecycleFailureV1")
            .field("error", &self.error)
            .field("retryable", &self.retryable_completed.is_some())
            .finish()
    }
}

impl<const N: usize> fmt::Display for Gfx942FixedDispatchRecycleFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.error)
    }
}

impl<const N: usize> std::error::Error for Gfx942FixedDispatchRecycleFailureV1<N> {}

/// Poll failure. Rejection before selection of the owning lane returns the
/// exact dependent-dispatch custody; an accepted failure is terminal.
#[must_use = "a retryable dependent dispatch must be recovered from this failure"]
pub struct Gfx942ComputeDependencyPollFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    retryable_dispatch: Option<Box<Gfx942ComputeDependencyDispatchV1>>,
}

impl Gfx942ComputeDependencyPollFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Box<Gfx942ComputeDependencyDispatchV1>>,
    ) {
        (self.error, self.retryable_dispatch)
    }
}

impl fmt::Debug for Gfx942ComputeDependencyPollFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencyPollFailureV1")
            .field("error", &self.error)
            .field("retryable", &self.retryable_dispatch.is_some())
            .finish()
    }
}

/// Event-release failure. Rejection before selection of the owning lane
/// returns the exact event; an accepted release failure is terminal.
#[must_use = "a retryable dependency event must be recovered from this failure"]
pub struct Gfx942ComputeDependencyEventReleaseFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    retryable_event: Option<Box<Gfx942ComputeDependencyEventV1>>,
}

impl Gfx942ComputeDependencyEventReleaseFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Box<Gfx942ComputeDependencyEventV1>>,
    ) {
        (self.error, self.retryable_event)
    }
}

impl fmt::Debug for Gfx942ComputeDependencyEventReleaseFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencyEventReleaseFailureV1")
            .field("error", &self.error)
            .field("retryable", &self.retryable_event.is_some())
            .finish()
    }
}

/// Submission failure. Only a proven no-effect rejection returns the exact
/// dependency-event roster for retry.
pub struct Gfx942ComputeDependencySubmissionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    retryable_events: Option<Vec<Gfx942ComputeDependencyEventV1>>,
}

impl Gfx942ComputeDependencySubmissionFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Vec<Gfx942ComputeDependencyEventV1>>,
    ) {
        (self.error, self.retryable_events)
    }
}

impl fmt::Debug for Gfx942ComputeDependencySubmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ComputeDependencySubmissionFailureV1")
            .field("error", &self.error)
            .field("retryable", &self.retryable_events.is_some())
            .finish()
    }
}

struct AuxiliaryComputeLaneSlotV1<T> {
    generation: u64,
    state: Option<T>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedAuxiliaryComputeLaneSlotV1 {
    index: usize,
    generation: u64,
    append: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmittedComputeLaneV1 {
    Primary,
    Auxiliary(usize),
}

struct ComputeAqlQueueLaneStateV1<
    E: construction_primary::PrimaryEnvironmentV1 = construction_primary::LinuxPrimaryEnvironmentV1,
> {
    key: QueueKeyV1,
    doorbell: Option<E::Doorbell>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    completion_signals: Option<CompletionSignalAuthority>,
    completion_owner: QueueOwnerSlotV1<CompletionSignalArenaOwnerV1>,
    dispatch: Option<DispatchResourceOwnerV1>,
    unpublished_dispatch: UnpublishedDispatchStateV1,
    detached_data_count: usize,
    detached_dispatch_generation: Option<u64>,
    detached_data_identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    detached_next_insertion_index: Option<usize>,
    exception: Option<QueueExceptionStateV1<E>>,
    observation: ComputeAqlQueueObservationV1,
}

fn prepare_auxiliary_compute_lane_slot_v1<T>(
    slots: &[AuxiliaryComputeLaneSlotV1<T>],
) -> Result<PreparedAuxiliaryComputeLaneSlotV1, ComputeAqlQueueSessionErrorV1> {
    if slots.len() >= ComputeAqlQueueSessionV1::MAX_COMPUTE_LANES_V1 {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute queue lane roster exceeds profile",
        ));
    }
    if slots.iter().any(|slot| slot.generation == 0) {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "invalid compute queue lane generation",
        ));
    }
    if let Some((index, slot)) = slots
        .iter()
        .enumerate()
        .find(|(_, slot)| slot.state.is_none())
    {
        let generation =
            slot.generation
                .checked_add(1)
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "compute queue lane generation exhausted",
                ))?;
        return Ok(PreparedAuxiliaryComputeLaneSlotV1 {
            index,
            generation,
            append: false,
        });
    }
    if slots.len() + 1 >= ComputeAqlQueueSessionV1::MAX_COMPUTE_LANES_V1 {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute queue lane capacity exhausted",
        ));
    }
    Ok(PreparedAuxiliaryComputeLaneSlotV1 {
        index: slots.len(),
        generation: 1,
        append: true,
    })
}

enum CheckedAuxiliaryComputeLaneSlotV1<'a, T> {
    Append(&'a mut Vec<AuxiliaryComputeLaneSlotV1<T>>),
    Reuse(&'a mut AuxiliaryComputeLaneSlotV1<T>, u64),
}

fn check_auxiliary_compute_lane_slot_v1<T>(
    slots: &mut Vec<AuxiliaryComputeLaneSlotV1<T>>,
    prepared: PreparedAuxiliaryComputeLaneSlotV1,
) -> Result<CheckedAuxiliaryComputeLaneSlotV1<'_, T>, ComputeAqlQueueSessionErrorV1> {
    if prepare_auxiliary_compute_lane_slot_v1(slots)? != prepared {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute queue lane destination changed",
        ));
    }
    if prepared.append {
        if slots.capacity() <= slots.len() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute queue lane destination not reserved",
            ));
        }
        Ok(CheckedAuxiliaryComputeLaneSlotV1::Append(slots))
    } else {
        Ok(CheckedAuxiliaryComputeLaneSlotV1::Reuse(
            &mut slots[prepared.index],
            prepared.generation,
        ))
    }
}

fn install_auxiliary_compute_lane_slot_v1<T>(
    destination: CheckedAuxiliaryComputeLaneSlotV1<'_, T>,
    state: T,
) {
    // The exclusive vacancy holds the checked roster unchanged through these moves.
    match destination {
        CheckedAuxiliaryComputeLaneSlotV1::Append(slots) => {
            slots.push(AuxiliaryComputeLaneSlotV1 {
                generation: 1,
                state: Some(state),
            });
        }
        CheckedAuxiliaryComputeLaneSlotV1::Reuse(slot, generation) => {
            slot.generation = generation;
            slot.state = Some(state);
        }
    }
}

fn admit_compute_lane_v1<T>(
    session: QueueKeyV1,
    auxiliary: &[AuxiliaryComputeLaneSlotV1<T>],
    lane: ComputeAqlQueueLaneV1,
) -> Result<AdmittedComputeLaneV1, ComputeAqlQueueSessionErrorV1> {
    if lane.session != session {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "compute queue lane session substitution",
        ));
    }
    if lane.ordinal == 0 {
        if lane.generation != session.generation.0 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "stale primary compute queue lane",
            ));
        }
        return Ok(AdmittedComputeLaneV1::Primary);
    }
    let index = lane
        .ordinal
        .checked_sub(1)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "invalid compute queue lane",
        ))?;
    let slot = auxiliary
        .get(index)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "unknown compute queue lane",
        ))?;
    if lane.generation != slot.generation {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "stale compute queue lane",
        ));
    }
    if slot.state.is_none() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "unknown compute queue lane",
        ));
    }
    Ok(AdmittedComputeLaneV1::Auxiliary(index))
}

fn auxiliary_compute_lanes_are_quiescent_v1(
    lanes: &[AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1>],
) -> bool {
    lanes.iter().all(|slot| {
        slot.state.as_ref().is_none_or(|state| {
            if !state.unpublished_dispatch.is_clear() {
                return state.unpublished_dispatch.quiescent(
                    state.completion_owner.ensure_releasable().is_ok(),
                    state.dispatch.is_some(),
                    state.detached_dispatch_generation,
                    state.detached_data_count,
                    state.detached_data_identities.len(),
                    state.detached_next_insertion_index,
                );
            }
            auxiliary_compute_lane_quiescence_from_facts_v1(
                state.completion_owner.ensure_releasable().is_ok(),
                state
                    .dispatch
                    .as_ref()
                    .map(|dispatch| dispatch.ensure_releasable().is_ok()),
                state.detached_data_count,
                state.detached_dispatch_generation,
                state.detached_data_identities.len(),
                state.detached_next_insertion_index,
            )
        })
    })
}

fn auxiliary_compute_lane_quiescence_from_facts_v1(
    completion_releasable: bool,
    attached_dispatch_releasable: Option<bool>,
    detached_data_count: usize,
    detached_dispatch_generation: Option<u64>,
    detached_identity_count: usize,
    detached_next_insertion_index: Option<usize>,
) -> bool {
    completion_releasable
        && match attached_dispatch_releasable {
            Some(releasable) => {
                releasable
                    && detached_data_count == 0
                    && detached_dispatch_generation.is_none()
                    && detached_identity_count == 0
                    && detached_next_insertion_index.is_none()
            }
            None => {
                detached_dispatch_generation.is_some_and(|generation| {
                    generation != 0
                        || (detached_data_count == 0
                            && detached_identity_count == 0
                            && detached_next_insertion_index == Some(0))
                }) && detached_data_count <= super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1
                    && detached_identity_count == detached_data_count
                    && detached_next_insertion_index
                        .is_none_or(|index| index <= detached_identity_count)
            }
        }
}

fn take_after_auxiliary_destroy_preflight_v1<T>(
    state: &mut Option<T>,
    preflight: impl FnOnce(&T) -> Result<(), ComputeAqlQueueSessionErrorV1>,
) -> Result<T, ComputeAqlQueueSessionErrorV1> {
    let retained = state
        .as_ref()
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "unknown compute queue lane",
        ))?;
    preflight(retained)?;
    Ok(state.take().expect("preflight retained auxiliary lane"))
}

fn preflight_auxiliary_compute_lane_destroy_v1(
    state: &ComputeAqlQueueLaneStateV1,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    state.completion_owner.ensure_releasable()?;
    if !state.unpublished_dispatch.is_clear() {
        return if state.detached_data_count == 0
            && state.unpublished_dispatch.quiescent(
                true,
                state.dispatch.is_some(),
                state.detached_dispatch_generation,
                state.detached_data_count,
                state.detached_data_identities.len(),
                state.detached_next_insertion_index,
            ) {
            Ok(())
        } else {
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into())
        };
    }
    if let Some(dispatch) = state.dispatch.as_ref() {
        dispatch.ensure_releasable()?;
    }
    // Cache eviction releases code/kernarg before returning the data leases.
    // A detached lane is destroyable only after every lease has been released.
    if state.detached_data_count != 0
        || !auxiliary_compute_lane_quiescence_from_facts_v1(
            true,
            state.dispatch.as_ref().map(|_| true),
            state.detached_data_count,
            state.detached_dispatch_generation,
            state.detached_data_identities.len(),
            state.detached_next_insertion_index,
        )
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "auxiliary dispatch resources must be attached or fully released before destroy",
        ));
    }
    Ok(())
}

/// Narrow fixed-dispatch access to one admitted compute lane.
///
/// Session-global transitions such as SDMA creation are deliberately absent.
///
/// ```compile_fail
/// use fe2o3_kfd::ComputeAqlQueueLaneDispatchV1;
///
/// fn cannot_enable_session_global_sdma(lane: &mut ComputeAqlQueueLaneDispatchV1<'_>) {
///     lane.enable_gfx942_directional_sdma_copy_engines();
/// }
/// ```
pub struct ComputeAqlQueueLaneDispatchV1<'a> {
    session: &'a mut ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
}

impl ComputeAqlQueueLaneDispatchV1<'_> {
    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.session.observation()
    }

    pub fn detach_recycled_fixed_dispatch(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        self.session.detach_recycled_fixed_dispatch()
    }

    /// Returns complete data from a strictly pristine recipe, without a completion receipt.
    pub fn abort_unpublished_fixed_dispatch_v1(
        &mut self,
    ) -> Result<Vec<Gfx942FixedDispatchDataV1>, ComputeAqlQueueSessionErrorV1> {
        self.session.abort_unpublished_fixed_dispatch_v1()
    }

    /// Releases prepare-once code and kernarg control after persistent data
    /// was restored to its separate SDMA owner.
    pub fn release_retained_persistent_fixed_dispatch_control_v1(
        &mut self,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.session
            .release_retained_persistent_fixed_dispatch_control_v1()
    }

    pub fn bind_fixed_dispatch<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session.bind_fixed_dispatch(programs, packets, data)
    }

    pub fn preflight_fixed_dispatch_data_insertion(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session
            .preflight_fixed_dispatch_data_insertion(data_index)
    }

    pub fn insert_initialized_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.session
            .insert_initialized_fixed_dispatch_data(data_index, bytes, alignment, content)
    }

    pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        data: &mut Gfx942FixedDispatchDataV1,
        offset: u64,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session
            .overwrite_detached_initialized_host_visible_fixed_dispatch_data(
                data_index, data, offset, source,
            )
    }

    pub fn insert_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.session
            .insert_initialized_host_visible_fixed_dispatch_data(data_index, bytes)
    }

    pub fn insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
        &mut self,
        data_index: usize,
        bytes: &[u8],
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.session
            .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(data_index, bytes)
    }

    pub fn initialize_host_visible_fixed_dispatch_data_from_slice_v1(
        &mut self,
        bytes: &[u8],
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.session
            .initialize_host_visible_fixed_dispatch_data_from_slice_v1(bytes)
    }

    pub fn release_detached_fixed_dispatch_data(
        &mut self,
        data: Gfx942FixedDispatchDataV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session.release_detached_fixed_dispatch_data(data)
    }

    pub fn submit_fixed_dispatch<const N: usize>(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.session.submit_fixed_dispatch::<N>()
    }

    pub fn submit_fixed_dispatch_classified_v1<const N: usize>(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<N>, Gfx942FixedDispatchSubmissionFailureV1> {
        self.session.submit_fixed_dispatch_classified_v1::<N>()
    }

    /// Publishes a real fixed batch and records one addressless source event
    /// for each exact packet occurrence.
    pub fn submit_fixed_dispatch_with_dependency_events_v1<const N: usize>(
        &mut self,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.session
            .submit_fixed_dispatch_with_dependency_events_inner_v1::<N>(self.lane)
    }

    pub fn poll_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.session.poll_fixed_dispatch(batch)
    }

    #[allow(clippy::result_large_err)]
    pub fn recycle_fixed_dispatch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        self.session.recycle_fixed_dispatch(completed)
    }

    pub fn recycled_fixed_dispatch_generation(&self) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
        self.session.recycled_fixed_dispatch_generation()
    }

    pub fn read_recycled_fixed_dispatch_data(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        self.session.read_recycled_fixed_dispatch_data(request)
    }

    pub fn read_recycled_fixed_dispatch_data_into(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session
            .read_recycled_fixed_dispatch_data_into(request, destination)
    }

    pub fn overwrite_recycled_fixed_dispatch_host_data(
        &mut self,
        request: Gfx942RecycledDispatchWriteRequestV1,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.session
            .overwrite_recycled_fixed_dispatch_host_data(request, source)
    }
}

struct QueueExceptionStateV1<
    E: construction_primary::PrimaryEnvironmentV1 = construction_primary::LinuxPrimaryEnvironmentV1,
> {
    runtime: E::Runtime,
    runtime_control: Option<E::RuntimeControl>,
    event: E::Event,
    shadows: E::Published,
}

type ExternalRuntimeV1<'a> = (
    &'a mut Option<LinuxKfdRuntimeEnabledV1>,
    &'a mut Option<KfdWithAdmittedUapi>,
);

struct QueueAfterEventDestroyedV1 {
    runtime: LinuxKfdRuntimeEnabledV1,
    runtime_control: Option<KfdWithAdmittedUapi>,
    shadows: LinuxCwsrShadowsAfterEventDestroyedV1,
    return_attached: bool,
    detached_return: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)>,
}

#[must_use = "queue destruction and runtime-disable authority return are explicit"]
pub struct KfdTargetRuntimeDebugQueueV1 {
    session: Option<ComputeAqlQueueSessionV1>,
    thread_bound: PhantomData<Rc<()>>,
}

#[must_use = "finish disables the runtime and releases retained queue resources"]
pub struct KfdTargetRuntimeDebugQueueTeardownV1 {
    session: Option<ComputeAqlQueueSessionV1>,
    runtime: Option<LinuxKfdRuntimeEnabledV1>,
    runtime_control: Option<KfdWithAdmittedUapi>,
    shadows: Option<LinuxCwsrShadowsAfterEventDestroyedV1>,
    thread_bound: PhantomData<Rc<()>>,
}

impl fmt::Debug for KfdTargetRuntimeDebugQueueV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdTargetRuntimeDebugQueueV1")
            .field(
                "observation",
                &self
                    .session
                    .as_ref()
                    .map(ComputeAqlQueueSessionV1::observation),
            )
            .finish_non_exhaustive()
    }
}

impl KfdTargetRuntimeDebugQueueV1 {
    pub(crate) fn new(session: ComputeAqlQueueSessionV1) -> Self {
        Self {
            session: Some(session),
            thread_bound: PhantomData,
        }
    }

    pub fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.session
            .as_ref()
            .expect("linear debug queue remains owned")
            .observation()
    }

    pub fn queue_mut(&mut self) -> &mut ComputeAqlQueueSessionV1 {
        self.session
            .as_mut()
            .expect("linear debug queue remains owned")
    }

    pub fn destroy(
        mut self,
    ) -> Result<KfdTargetRuntimeDebugQueueTeardownV1, ComputeAqlQueueSessionErrorV1> {
        let mut session = self
            .session
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing debug queue session",
            ))?;
        let after_event = session.destroy_queue_and_event(QueueDestroyModeV1::Release)?;
        let runtime_control =
            after_event
                .runtime_control
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing debug runtime control descriptor",
                ))?;
        Ok(KfdTargetRuntimeDebugQueueTeardownV1 {
            session: Some(session),
            runtime: Some(after_event.runtime),
            runtime_control: Some(runtime_control),
            shadows: Some(after_event.shadows),
            thread_bound: PhantomData,
        })
    }
}

impl fmt::Debug for KfdTargetRuntimeDebugQueueTeardownV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdTargetRuntimeDebugQueueTeardownV1")
            .field("runtime_enabled", &self.runtime.is_some())
            .finish_non_exhaustive()
    }
}

impl KfdTargetRuntimeDebugQueueTeardownV1 {
    pub fn finish(mut self) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        self.finish_with(|_| Ok(()))
            .map(|(destroyed, ())| destroyed)
    }

    pub(crate) fn finish_with<T>(
        &mut self,
        after_queue_destroyed: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(ComputeAqlQueueDestroyedV1, T), ComputeAqlQueueSessionErrorV1> {
        let runtime = self
            .runtime
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing debug runtime authority",
            ))?;
        let control =
            self.runtime_control
                .take()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing debug runtime control descriptor",
                ))?;
        let disabled = runtime.disable(control.opened.fd.as_fd(), control.opened.opener_pid)?;
        let shadows = self
            .shadows
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing CWSR shadow authority",
            ))?;
        let session = self
            .session
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing destroyed debug queue session",
            ))?;
        let (outcome, callback_result) =
            session.complete_destroy(disabled, shadows, false, None, after_queue_destroyed)?;
        match outcome {
            QueueDestroyOutcomeV1::Released(destroyed) => Ok((destroyed, callback_result)),
            QueueDestroyOutcomeV1::Returned(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "debug queue teardown returned dispatch resources",
            )),
        }
    }
}

impl fmt::Debug for ComputeAqlQueueSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComputeAqlQueueSessionV1")
            .field("observation", &self.observation)
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Allocates exact fe2o3 GTT roles, creates one queue, and maps its complete
    /// doorbell slice. This API deliberately exposes no MMIO or packet store.
    pub fn create_compute_aql_queue(
        self,
        ring_bytes: u32,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with(ring_bytes, |_| Ok(()))
            .map(|(session, ())| session)
    }

    /// Creates one queue with optional immutable, session-local N2 admission.
    ///
    /// The budget is configured before preparation and queue certification.
    /// It charges padded device backing and records, not GTT queue resources
    /// or VM bootstrap. `None` preserves the ordinary unconfigured profile.
    pub fn create_compute_aql_queue_with_device_backing_budget_v1(
        self,
        ring_bytes: u32,
        budget: Option<Gfx942DeviceBackingBudgetV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_backing_budgets_v1(ring_bytes, budget, None)
    }

    /// Creates one queue with optional immutable N2 and ordinary coherent GTT budgets.
    ///
    /// Both accounts are installed before preparation or queue certification.
    /// The host account includes ordinary coherent completion/control allocations,
    /// not executable, userptr or doubled-VA AQL backing. These are session-local
    /// backing limits, not complete bootstrap or aggregate process accounting.
    pub fn create_compute_aql_queue_with_backing_budgets_v1(
        self,
        ring_bytes: u32,
        device_budget: Option<Gfx942DeviceBackingBudgetV1>,
        host_budget: Option<Gfx942HostVisibleBackingBudgetV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_runtime(
            ring_bytes,
            |_| Ok(()),
            None,
            device_budget,
            host_budget,
        )
        .map(|(session, ())| session)
    }

    pub(crate) fn create_compute_aql_queue_with<T>(
        self,
        ring_bytes: u32,
        prepare: impl FnOnce(&mut SharedGttMemorySessionV1) -> Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(ComputeAqlQueueSessionV1, T), ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_runtime(ring_bytes, prepare, None, None, None)
    }

    pub(crate) fn create_compute_aql_queue_for_debug_target(
        self,
        ring_bytes: u32,
        runtime: &mut Option<LinuxKfdRuntimeEnabledV1>,
        runtime_control: &mut Option<KfdWithAdmittedUapi>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_runtime(
            ring_bytes,
            |_| Ok(()),
            Some((runtime, runtime_control)),
            None,
            None,
        )
        .map(|(session, ())| session)
    }

    pub(crate) fn create_compute_aql_queue_for_debug_target_with<T>(
        self,
        ring_bytes: u32,
        prepare: impl FnOnce(&mut SharedGttMemorySessionV1) -> Result<T, ComputeAqlQueueSessionErrorV1>,
        runtime: &mut Option<LinuxKfdRuntimeEnabledV1>,
        runtime_control: &mut Option<KfdWithAdmittedUapi>,
    ) -> Result<(ComputeAqlQueueSessionV1, T), ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_runtime(
            ring_bytes,
            prepare,
            Some((runtime, runtime_control)),
            None,
            None,
        )
    }

    fn create_compute_aql_queue_with_runtime<T>(
        self,
        ring_bytes: u32,
        prepare: impl FnOnce(&mut SharedGttMemorySessionV1) -> Result<T, ComputeAqlQueueSessionErrorV1>,
        external_runtime: Option<(
            &mut Option<LinuxKfdRuntimeEnabledV1>,
            &mut Option<KfdWithAdmittedUapi>,
        )>,
        device_backing_budget: Option<Gfx942DeviceBackingBudgetV1>,
        host_visible_backing_budget: Option<Gfx942HostVisibleBackingBudgetV1>,
    ) -> Result<(ComputeAqlQueueSessionV1, T), ComputeAqlQueueSessionErrorV1> {
        let geometry = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )?;
        let memory = self.acquire_shared_gtt_memory_session_with_backing_budgets_v1(
            device_backing_budget,
            host_visible_backing_budget,
        )?;
        let root = PrimaryQueueConstructionV1::new(memory, None);
        let mut root = root.run(|root, entry| {
            capture_returned_preparation_v1(
                root.memory.as_mut().expect("construction memory"),
                &mut root.preparation,
                prepare,
            )?;
            root.construct(
                entry,
                geometry,
                ring_bytes,
                QueueRingBackingV1::AqlSpecial,
                external_runtime,
            )
        })?;
        Ok((
            root.completed
                .take()
                .expect("validated completed queue")
                .into_session(),
            root.preparation.take().expect("returned preparation"),
        ))
    }

    /// Runs one fresh-queue BARRIER_AND liveness probe through full teardown.
    ///
    /// Success is returned only after the completion signal was acquired as
    /// zero, reset to pending, and the native queue and all queue resources
    /// were explicitly destroyed and released. The typed poll bound is
    /// constructed before this consuming operation. An execution failure
    /// retains opaque queue custody until process teardown. A terminal
    /// creation or teardown failure recovers no authority because native
    /// resource disposition may be indeterminate and requires process
    /// termination.
    pub fn run_compute_aql_barrier_probe(
        self,
        ring_bytes: u32,
        poll_bound: Gfx942BarrierProbePollBoundV1,
    ) -> Result<Gfx942BarrierProbeSuccessV1, Gfx942BarrierProbeFailureV1> {
        self.run_compute_aql_barrier_probe_with_backing(
            ring_bytes,
            poll_bound,
            QueueRingBackingV1::AqlSpecial,
        )
    }

    /// Runs the one-shot barrier probe with a plain executable 1x GTT ring.
    ///
    /// This diagnostic backing changes only the ring allocation flags and GPU
    /// VA span. Reusable queues and every dispatch API retain the special AQL
    /// ring profile. Failure and teardown guarantees match the ordinary probe.
    pub fn run_compute_aql_executable_ring_barrier_probe(
        self,
        ring_bytes: u32,
        poll_bound: Gfx942BarrierProbePollBoundV1,
    ) -> Result<Gfx942BarrierProbeSuccessV1, Gfx942BarrierProbeFailureV1> {
        self.run_compute_aql_barrier_probe_with_backing(
            ring_bytes,
            poll_bound,
            QueueRingBackingV1::ExecutableProbe,
        )
    }

    /// Runs the one-shot barrier probe with an exact USERPTR 1x ring.
    ///
    /// Its writable, executable, coherent, uncached, no-substitute profile is
    /// the smallest selected-GPU ring-backing discriminator; it does not claim
    /// full ROCr allocation or map-order parity. The live CPU VMA is registered
    /// at the same GPU VA and remains private to the queue lifecycle. Reusable
    /// queues and every dispatch API retain the special AQL ring profile. Once
    /// inner creation begins, every failure is terminal because USERPTR
    /// registration may have retained native custody.
    pub fn run_compute_aql_userptr_ring_barrier_probe(
        self,
        ring_bytes: u32,
        poll_bound: Gfx942BarrierProbePollBoundV1,
    ) -> Result<Gfx942BarrierProbeSuccessV1, Gfx942BarrierProbeFailureV1> {
        self.run_compute_aql_barrier_probe_with_backing(
            ring_bytes,
            poll_bound,
            QueueRingBackingV1::UserptrProbe,
        )
    }

    fn run_compute_aql_barrier_probe_with_backing(
        self,
        ring_bytes: u32,
        poll_bound: Gfx942BarrierProbePollBoundV1,
        backing: QueueRingBackingV1,
    ) -> Result<Gfx942BarrierProbeSuccessV1, Gfx942BarrierProbeFailureV1> {
        let polls = poll_bound.get();
        let backing_observation = backing.observation();
        let geometry = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )
        .map_err(|error| Gfx942BarrierProbeFailureV1::Creation {
            error: error.into(),
            backing: backing_observation,
        })?;
        let memory = self.acquire_shared_gtt_memory_session().map_err(|error| {
            Gfx942BarrierProbeFailureV1::Creation {
                error: error.into(),
                backing: backing_observation,
            }
        })?;
        let mut queue = ComputeAqlQueueSessionV1::create_compute_aql_queue_inner(
            memory,
            geometry,
            ring_bytes,
            backing,
            |_| Ok(None),
            None,
        )
        .map_err(|error| barrier_probe_creation_failure(error, backing_observation))?;
        let probe = match queue.submit_barrier_probe() {
            Ok(probe) => probe,
            Err(error) => {
                return Err(quarantine_barrier_probe_failure(
                    queue,
                    error,
                    backing_observation,
                ));
            }
        };
        let completed = match queue.wait_barrier_probe(probe, polls) {
            Ok(completed) => completed,
            Err(error) => {
                return Err(quarantine_barrier_probe_failure(
                    queue,
                    error,
                    backing_observation,
                ));
            }
        };
        let execution = match queue.observe_completed_barrier_probe(&completed) {
            Ok(execution) => execution,
            Err(error) => {
                return Err(quarantine_barrier_probe_failure(
                    queue,
                    error.into(),
                    backing_observation,
                ));
            }
        };
        let recycle = match queue.recycle_barrier_probe(completed) {
            Ok(recycle) => recycle,
            Err(error) => {
                return Err(quarantine_barrier_probe_failure(
                    queue,
                    error,
                    backing_observation,
                ));
            }
        };
        let recycled_signal_count = recycle.packet_count();
        let teardown_arm = arm_process_global_kfd_runtime_gate_for_teardown_v1();
        let destroyed =
            queue
                .destroy()
                .map_err(|error| Gfx942BarrierProbeFailureV1::TerminalTeardown {
                    error,
                    backing: backing_observation,
                })?;
        teardown_arm.confirm_destroyed();
        Ok(Gfx942BarrierProbeSuccessV1 {
            backing: backing_observation,
            poll_bound: polls,
            execution: Gfx942BarrierProbeExecutionObservationV1 { inner: execution },
            recycled_signal_count,
            destroyed,
        })
    }
}

fn quarantine_barrier_probe_failure(
    queue: ComputeAqlQueueSessionV1,
    error: ComputeAqlQueueSessionErrorV1,
    backing: Gfx942BarrierProbeRingBackingV1,
) -> Gfx942BarrierProbeFailureV1 {
    Gfx942BarrierProbeFailureV1::QuarantinedExecution {
        error,
        backing,
        retained: Box::new(QuarantinedGfx942BarrierProbeV1 { backing, queue }),
    }
}

fn barrier_probe_creation_failure(
    error: ComputeAqlQueueSessionErrorV1,
    backing: Gfx942BarrierProbeRingBackingV1,
) -> Gfx942BarrierProbeFailureV1 {
    if backing == Gfx942BarrierProbeRingBackingV1::UserptrOneX {
        permanently_poison_process_global_kfd_runtime_gate_v1();
        let error = if error.is_terminal_creation() {
            error
        } else {
            terminal_creation("USERPTR queue resource creation", error)
        };
        Gfx942BarrierProbeFailureV1::TerminalCreation { error, backing }
    } else if error.is_terminal_creation() {
        Gfx942BarrierProbeFailureV1::TerminalCreation { error, backing }
    } else {
        Gfx942BarrierProbeFailureV1::Creation { error, backing }
    }
}

fn retained_device_queue_is_active_v1(
    terminal: bool,
    authority_poisoned: bool,
    phase: Option<ComputeAqlQueuePhaseV1>,
) -> bool {
    !terminal && !authority_poisoned && phase == Some(ComputeAqlQueuePhaseV1::Active)
}

impl crate::retained_device::RetainedDeviceScopeOwnerV1 for ComputeAqlQueueSessionV1 {
    type Subject = CheckedGfx942XnackMinusDevice;
    type Error = ComputeAqlQueueSessionErrorV1;

    fn check_scope(&mut self) -> Result<(), Self::Error> {
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing retained-device queue engine",
            ))?;
        if !retained_device_queue_is_active_v1(
            self.terminal_poisoned,
            engine.authority_poisoned,
            engine.phase(self.key),
        ) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "inactive retained-device queue",
            ));
        }
        engine
            .backend
            .session
            .validate_retained_device_domain_v1(self.key.vm)?;
        self.check_currentness()
    }

    fn subject(&self) -> &Self::Subject {
        self.engine
            .as_ref()
            .expect("checked retained-device engine")
            .backend
            .session
            .retained_device_v1()
    }

    fn poison_scope(&mut self) {
        self.poison_terminal();
        crate::queue_linux::permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Borrows the exact session-owned device inside full currentness checks.
    ///
    /// This neither selects a lane nor lends the queue's memory/model foundation.
    /// The result cannot borrow the device. Failure or panic poisons retained
    /// queue custody and the process runtime gate; it never authorizes retry.
    pub fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        crate::retained_device::with_retained_device_scope_v1(self, observe)
    }

    /// Maximum number of independently publishable compute queues retained by
    /// one checked process-VM session in the reviewed runtime profile.
    pub const MAX_COMPUTE_LANES_V1: usize = 2;

    fn swap_primary_compute_lane(&mut self, lane: &mut ComputeAqlQueueLaneStateV1) {
        core::mem::swap(&mut self.key, &mut lane.key);
        core::mem::swap(&mut self.doorbell, &mut lane.doorbell);
        core::mem::swap(&mut self.submission, &mut lane.submission);
        core::mem::swap(&mut self.completion_signals, &mut lane.completion_signals);
        core::mem::swap(&mut self.completion_owner, &mut lane.completion_owner);
        core::mem::swap(&mut self.dispatch, &mut lane.dispatch);
        core::mem::swap(
            &mut self.unpublished_dispatch,
            &mut lane.unpublished_dispatch,
        );
        core::mem::swap(&mut self.detached_data_count, &mut lane.detached_data_count);
        core::mem::swap(
            &mut self.detached_dispatch_generation,
            &mut lane.detached_dispatch_generation,
        );
        core::mem::swap(
            &mut self.detached_data_identities,
            &mut lane.detached_data_identities,
        );
        core::mem::swap(
            &mut self.detached_next_insertion_index,
            &mut lane.detached_next_insertion_index,
        );
        core::mem::swap(&mut self.exception, &mut lane.exception);
        core::mem::swap(&mut self.observation, &mut lane.observation);
    }

    /// Returns the session-bound handle for the original compute queue.
    pub const fn primary_compute_lane_v1(&self) -> ComputeAqlQueueLaneV1 {
        ComputeAqlQueueLaneV1 {
            session: self.compute_lane_session,
            ordinal: 0,
            generation: self.compute_lane_session.generation.0,
        }
    }

    /// Runs one fixed-dispatch transition against an exact queue-local lane.
    ///
    /// The callback receives only fixed-dispatch and queue-observation methods;
    /// session-global SDMA remains owned outside lane selection. Queue-local
    /// authorities are restored to their stable slots before this method
    /// returns, including on an ordinary callback error.
    pub fn with_compute_lane_v1<R>(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
        operation: impl FnOnce(&mut ComputeAqlQueueLaneDispatchV1<'_>) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let admitted = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            lane,
        )?;
        let AdmittedComputeLaneV1::Auxiliary(index) = admitted else {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                operation(&mut ComputeAqlQueueLaneDispatchV1 {
                    session: self,
                    lane,
                })
            }));
            return match result {
                Ok(result) => Ok(result),
                Err(payload) => {
                    self.poison_terminal();
                    poison_process_global_after_lane_unwind_v1();
                    std::panic::resume_unwind(payload)
                }
            };
        };
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let mut selected = self.auxiliary_compute_lanes[index]
            .state
            .take()
            .expect("admitted auxiliary compute lane retains state");
        self.swap_primary_compute_lane(&mut selected);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            operation(&mut ComputeAqlQueueLaneDispatchV1 {
                session: self,
                lane,
            })
        }));
        self.swap_primary_compute_lane(&mut selected);
        self.auxiliary_compute_lanes[index].state = Some(selected);
        match result {
            Ok(result) => Ok(result),
            Err(payload) => {
                self.poison_terminal();
                poison_process_global_after_lane_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn with_dependency_target_lane_v1<R>(
        &mut self,
        target_lane: ComputeAqlQueueLaneV1,
        source_lane: ComputeAqlQueueLaneV1,
        operation: impl FnOnce(&mut Self, &mut CompletionSignalArenaOwnerV1) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let target = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            target_lane,
        )?;
        let source = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            source_lane,
        )?;
        if target_lane == source_lane {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "dependent target and source lane must differ",
            ));
        }
        match (target, source) {
            (AdmittedComputeLaneV1::Primary, AdmittedComputeLaneV1::Auxiliary(source_index)) => {
                let mut source = self.auxiliary_compute_lanes[source_index]
                    .state
                    .take()
                    .expect("admitted dependency source retains state");
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(self, &mut source.completion_owner)
                }));
                self.auxiliary_compute_lanes[source_index].state = Some(source);
                match result {
                    Ok(result) => Ok(result),
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
            (AdmittedComputeLaneV1::Auxiliary(target_index), AdmittedComputeLaneV1::Primary) => {
                let mut displaced_primary = self.auxiliary_compute_lanes[target_index]
                    .state
                    .take()
                    .expect("admitted dependency target retains state");
                self.swap_primary_compute_lane(&mut displaced_primary);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(self, &mut displaced_primary.completion_owner)
                }));
                self.swap_primary_compute_lane(&mut displaced_primary);
                self.auxiliary_compute_lanes[target_index].state = Some(displaced_primary);
                match result {
                    Ok(result) => Ok(result),
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
            (
                AdmittedComputeLaneV1::Auxiliary(target_index),
                AdmittedComputeLaneV1::Auxiliary(source_index),
            ) => {
                let mut displaced_primary = self.auxiliary_compute_lanes[target_index]
                    .state
                    .take()
                    .expect("admitted dependency target retains state");
                let Some(mut source) = self.auxiliary_compute_lanes[source_index].state.take()
                else {
                    self.auxiliary_compute_lanes[target_index].state = Some(displaced_primary);
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "dependency source lane is not live",
                    ));
                };
                self.swap_primary_compute_lane(&mut displaced_primary);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(self, &mut source.completion_owner)
                }));
                self.swap_primary_compute_lane(&mut displaced_primary);
                self.auxiliary_compute_lanes[source_index].state = Some(source);
                self.auxiliary_compute_lanes[target_index].state = Some(displaced_primary);
                match result {
                    Ok(result) => Ok(result),
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
            _ => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "dependency lanes must be distinct live session lanes",
            )),
        }
    }

    /// Publishes one exact one-packet target after 1 through 256 addressless
    /// events from the session's other compute lane.
    #[allow(clippy::result_large_err)]
    pub fn submit_fixed_dispatch_with_dependencies_v1(
        &mut self,
        target_lane: ComputeAqlQueueLaneV1,
        events: Vec<Gfx942ComputeDependencyEventV1>,
    ) -> Result<
        (
            Box<Gfx942ComputeDependencyDispatchV1>,
            Gfx942ComputeDependencyEventV1,
        ),
        Gfx942ComputeDependencySubmissionFailureV1,
    > {
        let reject = |error, events| Gfx942ComputeDependencySubmissionFailureV1 {
            error,
            retryable_events: Some(events),
        };
        if self.terminal_poisoned {
            return Err(reject(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
                events,
            ));
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(reject(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                events,
            ));
        }
        if events.is_empty() || events.len() > fe2o3_aql::AQL_MAX_DEPENDENCY_SIGNALS_V1 {
            return Err(reject(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency roster must contain 1 through 256 events",
                ),
                events,
            ));
        }
        if let Err(error) = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            target_lane,
        ) {
            return Err(reject(error, events));
        }
        let source_lane = events[0].lane;
        if source_lane == target_lane
            || source_lane.session != self.compute_lane_session
            || events.iter().any(|event| event.lane != source_lane)
        {
            return Err(reject(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "dependencies must belong to the one other session lane",
                ),
                events,
            ));
        }
        if let Err(error) = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            source_lane,
        ) {
            return Err(reject(error, events));
        }
        if let Err(error) = self.dependency_owner.ensure_target_capacity() {
            return Err(reject(map_dependency_target_use_error_v1(error), events));
        }

        let mut retained_events = Some(events);
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_dependency_target_lane_v1(
                target_lane,
                source_lane,
                |session, source_owner| {
                    let events = retained_events
                        .take()
                        .expect("selected dependency target executes once");
                    session.submit_fixed_dispatch_with_dependencies_current_lane_v1(
                        target_lane,
                        source_lane,
                        source_owner,
                        events,
                    )
                },
            )
        }));
        let result = match operation {
            Ok(result) => result,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match result {
            Ok(result) => result,
            Err(error) => Err(Gfx942ComputeDependencySubmissionFailureV1 {
                error,
                retryable_events: retained_events,
            }),
        }
    }

    #[allow(clippy::result_large_err)]
    fn submit_fixed_dispatch_with_dependencies_current_lane_v1(
        &mut self,
        target_lane: ComputeAqlQueueLaneV1,
        source_lane: ComputeAqlQueueLaneV1,
        source_owner: &mut CompletionSignalArenaOwnerV1,
        events: Vec<Gfx942ComputeDependencyEventV1>,
    ) -> Result<
        (
            Box<Gfx942ComputeDependencyDispatchV1>,
            Gfx942ComputeDependencyEventV1,
        ),
        Gfx942ComputeDependencySubmissionFailureV1,
    > {
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.submit_fixed_dispatch_with_dependencies_operation_v1(
                target_lane,
                source_lane,
                source_owner,
                events,
            )
        }));
        match operation {
            Ok(result) => result,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn submit_fixed_dispatch_with_dependencies_operation_v1(
        &mut self,
        target_lane: ComputeAqlQueueLaneV1,
        source_lane: ComputeAqlQueueLaneV1,
        source_owner: &mut CompletionSignalArenaOwnerV1,
        events: Vec<Gfx942ComputeDependencyEventV1>,
    ) -> Result<
        (
            Box<Gfx942ComputeDependencyDispatchV1>,
            Gfx942ComputeDependencyEventV1,
        ),
        Gfx942ComputeDependencySubmissionFailureV1,
    > {
        let wrap_events = |events: Vec<super::completion::Gfx942ComputeEventOccurrenceV1>| {
            events
                .into_iter()
                .map(|event| Gfx942ComputeDependencyEventV1 {
                    lane: source_lane,
                    event,
                })
                .collect()
        };
        let retry = |error, events| Gfx942ComputeDependencySubmissionFailureV1 {
            error,
            retryable_events: Some(wrap_events(events)),
        };
        let terminal = |error| Gfx942ComputeDependencySubmissionFailureV1 {
            error,
            retryable_events: None,
        };
        let raw_events = events
            .into_iter()
            .map(|event| event.event)
            .collect::<Vec<_>>();
        let acceptance = match self.dependency_owner.reserve_acceptance_epoch() {
            Ok(acceptance) => acceptance,
            Err(ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency acceptance epoch exhausted",
                )));
            }
            Err(error) => {
                return Err(retry(map_dependency_target_use_error_v1(error), raw_events));
            }
        };
        let binding = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
            .and_then(|dispatch| dispatch.bind_templates::<1>(self.key));
        let (templates, identity) = match self
            .classify_fixed_dispatch_binding(FixedDispatchBindingModeV1::Ordinary, binding)
        {
            Ok(templates) => templates,
            Err(error) => return Err(retry(error.into_error(), raw_events)),
        };
        let bound = match self.completion_owner.bind_batch(templates) {
            Ok(bound) => bound,
            Err(Gfx942CompletionErrorV1::InsufficientSignals) => {
                if self
                    .cancel_dependency_dispatch_generation_v1(identity)
                    .is_ok()
                {
                    return Err(retry(
                        Gfx942CompletionErrorV1::InsufficientSignals.into(),
                        raw_events,
                    ));
                }
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(
                    Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                ));
            }
            Err(error) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(error.into()));
            }
        };
        let target = match self.completion_owner.prepare_dependency_target_v1(
            acceptance.session_occurrence(),
            acceptance.epoch(),
            bound,
        ) {
            Ok(target) => target,
            Err((error, bound)) => {
                let (_, retention) = bound.into_parts();
                let cancelled = self.completion_owner.cancel_bound(retention).is_ok()
                    && self
                        .cancel_dependency_dispatch_generation_v1(identity)
                        .is_ok();
                if cancelled {
                    return Err(retry(error.into(), raw_events));
                }
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(error.into()));
            }
        };
        if !raw_events
            .iter()
            .all(|event| source_owner.matches_dependency_event_v1(event))
        {
            let cancelled = self
                .completion_owner
                .cancel_prepared_dependency_target_v1(target)
                .is_ok()
                && self
                    .cancel_dependency_dispatch_generation_v1(identity)
                    .is_ok();
            if cancelled {
                return Err(retry(
                    ComputeAqlQueueSessionErrorV1::Contract("dependency source lane mismatch"),
                    raw_events,
                ));
            }
            self.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                "dependency source rollback",
            )));
        }
        let readers =
            retain_dependency_readers_for_target_v1(source_owner, raw_events, &acceptance);
        let readers = match readers {
            Ok(readers) => readers,
            Err(ComputeDependencyReaderBatchFailureV1::Rejected { error, events }) => {
                let cancelled = self
                    .completion_owner
                    .cancel_prepared_dependency_target_v1(target)
                    .is_ok()
                    && self
                        .cancel_dependency_dispatch_generation_v1(identity)
                        .is_ok();
                if cancelled {
                    return Err(retry(error.into(), events));
                }
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(error.into()));
            }
            Err(ComputeDependencyReaderBatchFailureV1::Terminal(_custody)) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency reader projection",
                )));
            }
        };
        let prepared = match self.dependency_owner.begin_target_use(
            &self.completion_owner,
            acceptance,
            target,
            readers,
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, _acceptance, target, readers) = failure.into_parts();
                let events =
                    rollback_dependency_readers_before_publication_v1(source_owner, readers);
                if let Ok(events) = events
                    && self
                        .completion_owner
                        .cancel_prepared_dependency_target_v1(target)
                        .is_ok()
                    && self
                        .cancel_dependency_dispatch_generation_v1(identity)
                        .is_ok()
                {
                    return Err(retry(map_dependency_target_use_error_v1(error), events));
                }
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(map_dependency_target_use_error_v1(error)));
            }
        };
        let native = self.publish_dependency_target_native_v1(prepared);
        let native = match native {
            Ok(native) => native,
            Err(ComputeDependencyPublicationFailureV1::Retryable(retryable)) => {
                let cancelled = {
                    let (dependency_owner, target_owner) =
                        (&mut self.dependency_owner, &mut self.completion_owner);
                    let mut source_owners = [source_owner];
                    dependency_owner.rollback_retryable_before_side_effect(
                        retryable,
                        &mut source_owners,
                        target_owner,
                    )
                };
                if let Ok(cancelled) = cancelled {
                    let (retention, events) = cancelled.into_parts();
                    if self.completion_owner.cancel_bound(retention).is_ok()
                        && self
                            .cancel_dependency_dispatch_generation_v1(identity)
                            .is_ok()
                    {
                        return Err(retry(
                            ComputeAqlQueueSessionErrorV1::Native(
                                "dependency submission ring occupancy",
                            ),
                            events,
                        ));
                    }
                }
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency no-effect rollback",
                )));
            }
            Err(ComputeDependencyPublicationFailureV1::Terminal(custody)) => {
                let error = map_submission_ref(&custody.error);
                self.terminal_dependency = Some(custody);
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(error));
            }
        };
        let published = match self
            .dependency_owner
            .bind_published_target(native, &mut self.completion_owner)
        {
            Ok(published) => published,
            Err(custody) => {
                self.terminal_dependency = Some(Box::new(custody));
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency publication binding",
                )));
            }
        };
        let (published, target_event) = published.into_parts();
        let completion_occurrence = match published.completion_occurrence_v1() {
            Ok(occurrence) => occurrence,
            Err(error) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(error.into()));
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("dependent dispatch owner remains retained")
            .mark_published_occurrence(identity, completion_occurrence)
            .is_err()
        {
            self.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(terminal(
                Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
            ));
        }
        Ok((
            Box::new(Gfx942ComputeDependencyDispatchV1 {
                lane: target_lane,
                source_lane,
                identity,
                published: Some(published),
            }),
            Gfx942ComputeDependencyEventV1 {
                lane: target_lane,
                event: target_event,
            },
        ))
    }

    fn cancel_dependency_dispatch_generation_v1(
        &mut self,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .cancel_binding(identity)
    }

    fn publish_dependency_target_native_v1(
        &mut self,
        prepared: PreparedComputeDependencyTargetUseV1,
    ) -> Result<
        super::dependency::NativePublishedComputeDependencyTargetUseV1,
        ComputeDependencyPublicationFailureV1,
    > {
        let structural_error =
            if self.terminal_poisoned {
                Some(NativeAqlSubmissionErrorV1::Poisoned)
            } else if self.exception.is_none() {
                Some(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing queue exception gate",
                ))
            } else if self.submission.is_none() {
                Some(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing submission owner",
                ))
            } else if self.engine.is_none() {
                Some(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing queue engine",
                ))
            } else if self.engine.as_ref().is_some_and(|engine| {
                engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active)
            }) {
                Some(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "queue is not active",
                ))
            } else if self.doorbell.is_none() {
                Some(NativeAqlSubmissionErrorV1::InvalidQueue("missing doorbell"))
            } else {
                None
            };
        if let Some(error) = structural_error {
            return Err(ComputeDependencyPublicationFailureV1::Terminal(
                self.dependency_owner
                    .terminal_before_native_publication(prepared, error),
            ));
        }
        let exception = self.exception.as_ref().expect("preflighted exception");
        let submission = self.submission.as_mut().expect("preflighted submission");
        let engine = self.engine.as_mut().expect("preflighted engine");
        let (backend, resources) = (&mut engine.backend, &mut engine.resources);
        let Some(resource) = resources
            .iter_mut()
            .find(|resource| resource.key == self.key)
        else {
            return Err(ComputeDependencyPublicationFailureV1::Terminal(
                self.dependency_owner.terminal_before_native_publication(
                    prepared,
                    NativeAqlSubmissionErrorV1::InvalidQueue("missing queue resources"),
                ),
            ));
        };
        let Some(authority) = resource.authority.as_mut() else {
            return Err(ComputeDependencyPublicationFailureV1::Terminal(
                self.dependency_owner.terminal_before_native_publication(
                    prepared,
                    NativeAqlSubmissionErrorV1::InvalidQueue("released queue resources"),
                ),
            ));
        };
        let doorbell = self.doorbell.as_mut().expect("preflighted doorbell");
        let mut native = LinuxAqlSubmissionBackendV1 {
            memory: &mut backend.session,
            ring: &mut authority.ring,
            control: &mut authority.control,
            doorbell,
            exception,
        };
        self.dependency_owner
            .publish_native(prepared, submission, &mut native)
    }

    /// Polls the exact dependent target once. A ready result has already
    /// released every consumed source reader and source event exactly once.
    pub fn poll_compute_dependency_dispatch_v1(
        &mut self,
        dispatch: Box<Gfx942ComputeDependencyDispatchV1>,
    ) -> Result<Gfx942ComputeDependencyPollV1, Gfx942ComputeDependencyPollFailureV1> {
        let rejected = |error, dispatch| Gfx942ComputeDependencyPollFailureV1 {
            error,
            retryable_dispatch: Some(dispatch),
        };
        if dispatch.lane.session != self.compute_lane_session {
            return Err(rejected(
                ComputeAqlQueueSessionErrorV1::Contract("cross-session dependent dispatch"),
                dispatch,
            ));
        }
        let lane = dispatch.lane;
        let source_lane = dispatch.source_lane;
        let mut retained = Some(dispatch);
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_dependency_target_lane_v1(lane, source_lane, |session, source_owner| {
                session.poll_compute_dependency_dispatch_current_lane_v1(
                    source_owner,
                    retained
                        .take()
                        .expect("selected dependent poll executes once"),
                )
            })
        }));
        let result = match operation {
            Ok(result) => result,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match result {
            Ok(result) => result,
            Err(error) => Err(Gfx942ComputeDependencyPollFailureV1 {
                error,
                retryable_dispatch: retained,
            }),
        }
    }

    fn poll_compute_dependency_dispatch_current_lane_v1(
        &mut self,
        source_owner: &mut CompletionSignalArenaOwnerV1,
        dispatch: Box<Gfx942ComputeDependencyDispatchV1>,
    ) -> Result<Gfx942ComputeDependencyPollV1, Gfx942ComputeDependencyPollFailureV1> {
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_compute_dependency_dispatch_operation_v1(source_owner, dispatch)
        }));
        match operation {
            Ok(Err(failure)) => {
                debug_assert!(failure.retryable_dispatch.is_none());
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                Err(failure)
            }
            Ok(Ok(result)) => Ok(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn poll_compute_dependency_dispatch_operation_v1(
        &mut self,
        source_owner: &mut CompletionSignalArenaOwnerV1,
        mut dispatch: Box<Gfx942ComputeDependencyDispatchV1>,
    ) -> Result<Gfx942ComputeDependencyPollV1, Gfx942ComputeDependencyPollFailureV1> {
        let terminal = |error| Gfx942ComputeDependencyPollFailureV1 {
            error,
            retryable_dispatch: None,
        };
        if self.terminal_poisoned {
            return Err(terminal(Gfx942DispatchBindingErrorV1::Poisoned.into()));
        }
        let identity = dispatch.identity;
        let published = dispatch
            .published
            .take()
            .expect("live dependent dispatch retains published custody");
        let completion_occurrence = published
            .completion_occurrence_v1()
            .map_err(|error| terminal(error.into()))?;
        self.dispatch
            .as_ref()
            .ok_or_else(|| terminal(Gfx942DispatchBindingErrorV1::ResourcePhase.into()))?
            .validate_published_occurrence(identity, completion_occurrence)
            .map_err(|error| terminal(error.into()))?;
        let poll = {
            let engine = self.engine.as_mut().ok_or_else(|| {
                terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))
            })?;
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependent target queue is not active",
                )));
            }
            let signals = self.completion_signals.as_mut().ok_or_else(|| {
                terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing completion signal arena",
                ))
            })?;
            let exception = self.exception.as_ref().ok_or_else(|| {
                terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue exception gate",
                ))
            })?;
            let mut backend = LinuxCompletionSignalBackendV1 {
                memory: &mut engine.backend.session,
                signals,
                exception,
            };
            self.dependency_owner.observe_published_target_once(
                published,
                &mut self.completion_owner,
                &mut backend,
            )
        };
        let poll = match poll {
            Ok(poll) => poll,
            Err(custody) => {
                self.terminal_dependency = Some(Box::new(custody));
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependent completion observation",
                )));
            }
        };
        match poll {
            ComputeDependencyTargetPollV1::Pending(published) => {
                dispatch.published = Some(published);
                Ok(Gfx942ComputeDependencyPollV1::Pending(dispatch))
            }
            ComputeDependencyTargetPollV1::Ready(completed) => {
                let completion_occurrence = match completed.completion_occurrence_v1() {
                    Ok(occurrence) => occurrence,
                    Err(_) => {
                        let custody = self.dependency_owner.terminal_after_dependent_completion(
                            completed,
                            NativeAqlSubmissionErrorV1::InvalidQueue(
                                "dependent completion identity",
                            ),
                        );
                        self.terminal_dependency = Some(Box::new(custody));
                        self.poison_terminal();
                        permanently_poison_process_global_kfd_runtime_gate_v1();
                        return Err(terminal(
                            Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                        ));
                    }
                };
                if self
                    .dispatch
                    .as_mut()
                    .expect("dependent dispatch owner remains retained")
                    .mark_completed_occurrence(identity, completion_occurrence)
                    .is_err()
                {
                    let custody = self.dependency_owner.terminal_after_dependent_completion(
                        completed,
                        NativeAqlSubmissionErrorV1::InvalidQueue("dependent dispatch generation"),
                    );
                    self.terminal_dependency = Some(Box::new(custody));
                    self.poison_terminal();
                    permanently_poison_process_global_kfd_runtime_gate_v1();
                    return Err(terminal(
                        Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                    ));
                }
                self.release_completed_dependency_target_v1(identity, source_owner, completed)
                    .map_err(terminal)
            }
        }
    }

    fn release_completed_dependency_target_v1(
        &mut self,
        identity: DispatchEpochIdentityV1,
        source_owner: &mut CompletionSignalArenaOwnerV1,
        completed: CompletedComputeDependencyTargetUseV1,
    ) -> Result<Gfx942ComputeDependencyPollV1, ComputeAqlQueueSessionErrorV1> {
        if !completed.matches_source_owner(source_owner) {
            let custody = self.dependency_owner.terminal_after_dependent_completion(
                completed,
                NativeAqlSubmissionErrorV1::InvalidQueue("dependent source owner"),
            );
            self.terminal_dependency = Some(Box::new(custody));
            self.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "dependent source owner",
            ));
        }
        let released = self.dependency_owner.release_after_dependent_completion(
            completed,
            &self.completion_owner,
            source_owner,
        );
        let ReleasedComputeDependencyTargetUseV1 {
            target_completion,
            dependency_count,
            ..
        } = match released {
            Ok(released) => released,
            Err(custody) => {
                self.terminal_dependency = Some(Box::new(custody));
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "dependent source release",
                ));
            }
        };
        Ok(Gfx942ComputeDependencyPollV1::Ready(Box::new(
            Gfx942CompletedComputeDependencyDispatchV1 {
                completed: wrap_completed(target_completion, identity),
                dependency_count,
            },
        )))
    }

    /// Releases one unused source or completed target event without exposing its
    /// packet, slot, signal, or native address.
    #[allow(clippy::result_large_err)]
    pub fn release_compute_dependency_event_v1(
        &mut self,
        event: Gfx942ComputeDependencyEventV1,
    ) -> Result<
        super::completion::Gfx942ComputeEventReleaseObservationV1,
        Gfx942ComputeDependencyEventReleaseFailureV1,
    > {
        if event.lane.session != self.compute_lane_session {
            return Err(Gfx942ComputeDependencyEventReleaseFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("cross-session dependency event"),
                retryable_event: Some(Box::new(event)),
            });
        }
        let lane = event.lane;
        let mut retained = Some(event.event);
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_compute_lane_v1(lane, |selected| {
                selected
                    .session
                    .completion_owner
                    .release_dependency_event_v1(
                        retained
                            .take()
                            .expect("selected event release executes once"),
                    )
            })
        }));
        let result = match operation {
            Ok(result) => result,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match result {
            Ok(Ok(observation)) => Ok(observation),
            Ok(Err((error, _event))) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                Err(Gfx942ComputeDependencyEventReleaseFailureV1 {
                    error: error.into(),
                    retryable_event: None,
                })
            }
            Err(error) => Err(Gfx942ComputeDependencyEventReleaseFailureV1 {
                error,
                retryable_event: retained
                    .map(|event| Box::new(Gfx942ComputeDependencyEventV1 { lane, event })),
            }),
        }
    }

    /// Creates one additional native compute queue under this session's exact
    /// VM/model owner and binds an initial fixed dispatch without publishing it.
    pub fn create_auxiliary_compute_lane_with_fixed_dispatch<const N: usize>(
        &mut self,
        ring_bytes: u32,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        prepare_data: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<
            Vec<Gfx942FixedDispatchDataV1>,
            ComputeAqlQueueSessionErrorV1,
        >,
    ) -> Result<ComputeAqlQueueLaneV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        validate_fixed_batch_ring::<N>(ring_bytes)?;
        let slot = prepare_auxiliary_compute_lane_slot_v1(&self.auxiliary_compute_lanes)?;
        if slot.append {
            self.auxiliary_compute_lanes
                .try_reserve_exact(1)
                .map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract("compute queue lane roster allocation")
                })?;
        }
        if self.engine.as_ref().is_some_and(|engine| {
            matches!(
                engine.preflight_operation(),
                Err(NativeQueueAdapterErrorV1::JournalCapacity)
            )
        }) {
            return Err(map_native(NativeQueueAdapterErrorV1::JournalCapacity));
        }

        construction_auxiliary::construct_auxiliary_compute_lane_v1(
            self,
            ring_bytes,
            programs,
            packets,
            slot,
            prepare_data,
        )
    }

    pub fn auxiliary_compute_lane_count_v1(&self) -> usize {
        self.auxiliary_compute_lanes
            .iter()
            .filter(|lane| lane.state.is_some())
            .count()
    }

    /// Destroys one quiescent auxiliary queue and releases all of its native
    /// resources while retaining the shared VM and primary queue.
    pub fn destroy_auxiliary_compute_lane_v1(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let admitted = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            lane,
        )?;
        let AdmittedComputeLaneV1::Auxiliary(index) = admitted else {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "the primary queue is destroyed with its session",
            ));
        };
        self.dependency_owner
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        let mut state = take_after_auxiliary_destroy_preflight_v1(
            &mut self.auxiliary_compute_lanes[index].state,
            preflight_auxiliary_compute_lane_destroy_v1,
        )?;

        let result =
            (|| {
                let engine =
                    self.engine
                        .as_mut()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue engine",
                        ))?;
                engine.destroy(state.key).map_err(map_native)?;
                let mut exception =
                    state
                        .exception
                        .take()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue exception state",
                        ))?;
                exception.runtime.mark_queue_destroyed()?;
                let destroyed_event = exception.event.destroy(
                    engine.backend.session.kfd_fd(),
                    engine.backend.session.opener_pid(),
                )?;
                let shadows = exception.shadows.after_event_destroy(destroyed_event)?;
                exception.runtime.mark_event_destroyed()?;
                let disabled_runtime = exception.runtime.disable(
                    engine.backend.session.kfd_fd(),
                    engine.backend.session.opener_pid(),
                )?;
                let shadow_release = shadows.after_runtime_destroy(disabled_runtime)?;
                state
                    .doorbell
                    .take()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing doorbell"))?
                    .release()?;
                self.check_currentness()?;
                let authority = self
                    .engine
                    .as_mut()
                    .expect("checked queue engine")
                    .release_destroyed_resources(state.key)
                    .map_err(map_native)?;
                let dispatch = state.dispatch.take();
                let completion_signals = state.completion_signals.take().ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                )?;
                self.with_live_queue_memory_model(move |memory| {
                    release_resource_authority(memory, authority, shadow_release)?;
                    if let Some(dispatch) = dispatch {
                        dispatch.release(memory)?;
                    }
                    let completion_signals =
                        memory.unmap_from_gpu(completion_signals.into_token())?;
                    memory.release(completion_signals)?;
                    Ok(())
                })
            })();
        if let Err(error) = result {
            self.poison_terminal();
            return Err(error);
        }
        Ok(())
    }

    fn create_compute_aql_queue_inner(
        memory: SharedGttMemorySessionV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        ring_bytes: u32,
        ring_backing: QueueRingBackingV1,
        prepare_dispatch: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<
            Option<DispatchResourceOwnerV1>,
            ComputeAqlQueueSessionErrorV1,
        >,
        external_runtime: Option<ExternalRuntimeV1<'_>>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        let root = PrimaryQueueConstructionV1::new(memory, ());
        let mut root = root.run(|root, entry| {
            root.dispatch = prepare_dispatch(root.memory.as_mut().expect("construction memory"))?;
            root.construct(entry, geometry, ring_bytes, ring_backing, external_runtime)
        })?;
        Ok(root
            .completed
            .take()
            .expect("validated completed queue")
            .into_session())
    }

    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.observation
    }

    /// Reports the retained session's optional N2 debit without native observation.
    ///
    /// This includes cached and uncertain backing while its session remains
    /// retained. It does not report GTT, queue or aggregate process residency.
    pub fn device_backing_usage_v1(&self) -> Option<Gfx942DeviceBackingUsageV1> {
        self.engine
            .as_ref()
            .and_then(|engine| engine.backend.session.device_backing_usage_v1())
    }

    /// Reports ordinary coherent GTT backing without native progress or cleanup.
    /// `None` means unavailable or unconfigured, not a zero-residency certificate.
    pub fn host_visible_backing_usage_v1(&self) -> Option<Gfx942HostVisibleBackingUsageV1> {
        self.engine
            .as_ref()
            .and_then(|engine| engine.backend.session.host_visible_backing_usage_v1())
    }

    /// Installs immutable device-cache limits before any SDMA resource attempt.
    /// Existing compute backing and queue certification do not close this configuration.
    /// These limits exclude checked-out backing, host buffers and queue resources.
    pub fn configure_sdma_device_pool_v1(
        &mut self,
        limits: Gfx942DevicePoolLimitsV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned
            || self.sdma.is_some()
            || self.striped_sdma.is_some()
            || self.sdma_outstanding_buffers != 0
            || !self.sdma_pool_free.is_empty()
            || self.sdma_pool_reuse_count != 0
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA device pool configuration requires a fresh SDMA resource history",
            ));
        }
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        engine
            .backend
            .session
            .validate_device_pool_domain_v1(self.key.vm)?;
        self.sdma_device_pool.configure(limits)
    }

    /// Reports padded backing retained by idle device buffers in this queue's cache.
    /// `None` means unconfigured, not zero usage. This does not observe native currentness.
    pub fn sdma_device_pool_usage_v1(
        &self,
    ) -> Result<Option<Gfx942DevicePoolUsageV1>, ComputeAqlQueueSessionErrorV1> {
        let Some(limits) = self.sdma_device_pool.limits else {
            return Ok(None);
        };
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        device_pool_usage_v1(
            &engine.backend.session,
            self.key,
            limits,
            &self.sdma_pool_free,
        )
        .map(Some)
        .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("invalid SDMA device pool roster"))
    }

    /// Installs ordinary coherent host-cache limits before any SDMA attempt.
    /// Compute queue certification alone does not close this configuration.
    pub fn configure_sdma_host_pool_v1(
        &mut self,
        limits: Gfx942HostPoolLimitsV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned
            || self.sdma_host_pool_limits.is_some()
            || self.sdma_device_pool.activity_started
            || self.sdma.is_some()
            || self.striped_sdma.is_some()
            || self.sdma_outstanding_buffers != 0
            || !self.sdma_pool_free.is_empty()
            || self.sdma_pool_reuse_count != 0
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA host pool configuration requires a fresh SDMA resource history",
            ));
        }
        self.engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session
            .validate_host_pool_domain_v1(self.key.vm)?;
        self.sdma_host_pool_limits = Some(limits);
        Ok(())
    }

    /// Observes padded idle Host backing, not checked-out bytes or currentness.
    /// `None` means unconfigured, not proof of empty native custody.
    pub fn sdma_host_pool_usage_v1(
        &self,
    ) -> Result<Option<Gfx942HostPoolUsageV1>, ComputeAqlQueueSessionErrorV1> {
        let Some(limits) = self.sdma_host_pool_limits else {
            return Ok(None);
        };
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        host_pool_usage_v1(
            &engine.backend.session,
            self.key,
            limits,
            &self.sdma_pool_free,
        )
        .map(Some)
        .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("invalid SDMA host pool roster"))
    }

    /// Adds one generic gfx942 SDMA queue to this session.
    ///
    /// Failures before the first live shared-memory/currentness operation are retryable.
    /// Every failure at or beyond that boundary returns terminal process-teardown custody.
    // Inline terminal custody avoids a fallible allocation after native state changes.
    #[allow(clippy::result_large_err)]
    pub fn enable_sdma_copy_engine(
        &mut self,
    ) -> Result<Gfx942SdmaQueueObservationV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        let reserved = self.active_compute_queue_ids_for_sdma_creation_v1()?;
        let key = self.key;
        let owner = self.with_sdma_queue_creation_custody_v1(
            "generic SDMA queue creation",
            |memory| Gfx942SdmaQueueSetV1::create_generic(memory, key, &reserved),
            |owner| Gfx942SdmaQueueSetV1::retain_created_for_terminal(owner, None),
        )?;
        let observation = owner
            .generic_observation()
            .expect("created generic SDMA queue set");
        self.sdma = Some(owner);
        Ok(observation)
    }

    /// Adds the exact gfx942 directional SDMA profile to this session.
    ///
    /// Admission requires exactly two ordinary SDMA engines with eight queues
    /// per engine. KFD engine index 1 handles H2D and index 0 handles D2H, as
    /// observed in the pinned ROCr gfx94x policy.
    // Inline terminal custody avoids a fallible allocation after native state changes.
    #[allow(clippy::result_large_err)]
    pub fn enable_gfx942_directional_sdma_copy_engines(
        &mut self,
    ) -> Result<Gfx942DirectionalSdmaQueueObservationV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        let reserved = self.active_compute_queue_ids_for_sdma_creation_v1()?;
        let key = self.key;
        let owner = self.with_sdma_queue_creation_custody_v1(
            "directional SDMA queue creation",
            |memory| Gfx942SdmaQueueSetV1::create_directional(memory, key, &reserved),
            |owner| Gfx942SdmaQueueSetV1::retain_created_for_terminal(owner, None),
        )?;
        let observation = owner
            .directional_observation()
            .expect("created directional SDMA queue set");
        self.sdma = Some(owner);
        Ok(observation)
    }

    /// Adds one exact gfx942 SDMA queue targeted by KFD engine index.
    ///
    /// This diagnostic control admits only index 0 or 1 after observing the
    /// exact two-engine/eight-queues-per-engine topology profile. The index is
    /// not the public HSA engine bit mask.
    // Inline terminal custody avoids a fallible allocation after native state changes.
    #[allow(clippy::result_large_err)]
    pub fn enable_gfx942_sdma_copy_engine_on_engine_index(
        &mut self,
        engine_index: u32,
    ) -> Result<Gfx942SdmaQueueObservationV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        let reserved = self.active_compute_queue_ids_for_sdma_creation_v1()?;
        let key = self.key;
        let owner = self.with_sdma_queue_creation_custody_v1(
            "targeted SDMA queue creation",
            |memory| Gfx942SdmaQueueSetV1::create_targeted(memory, key, engine_index, &reserved),
            |owner| Gfx942SdmaQueueSetV1::retain_created_for_terminal(owner, None),
        )?;
        let observation = owner
            .generic_observation()
            .expect("created targeted single SDMA queue set");
        self.sdma = Some(owner);
        Ok(observation)
    }

    /// Adds a balanced round-robin set of targeted gfx942 SDMA queues.
    ///
    /// `queue_count` must be even and in `2..=16`. Creation admits exactly two
    /// ordinary engines and eight queues per engine from the retained topology;
    /// each successive queue targets alternating engine indices 0 and 1.
    // Inline terminal custody avoids a fallible allocation after native state changes.
    #[allow(clippy::result_large_err)]
    pub fn enable_gfx942_striped_sdma_copy_engines(
        &mut self,
        queue_count: u32,
    ) -> Result<Vec<Gfx942SdmaQueueObservationV1>, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        if !striped_sdma_queue_count_is_admitted(queue_count) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "striped SDMA queue count must be even and in 2..=16",
            ));
        }
        let reserved = self.active_compute_queue_ids_for_sdma_creation_v1()?;
        let key = self.key;
        let (owner, observations) = self.with_sdma_queue_creation_custody_v1(
            "striped SDMA queue creation",
            |memory| Gfx942SdmaQueueSetV1::create_striped(memory, key, queue_count, &reserved),
            |(owner, _)| Gfx942SdmaQueueSetV1::retain_created_for_terminal(owner, None),
        )?;
        self.sdma = Some(owner);
        Ok(observations)
    }

    /// Adds the experimental V2 logical-lane mux over exactly two native queues.
    ///
    /// `logical_lane_count` is one of `2`, `4`, `8`, `14`, or `16`. The two
    /// persistent native queues target engine indices 0 and 1. Logical lanes
    /// sharing a native queue are ordered by the mux and therefore do not have
    /// HIP stream independence or independent scheduling semantics.
    #[allow(clippy::result_large_err)]
    pub fn enable_gfx942_two_native_sdma_logical_mux_v2(
        &mut self,
        logical_lane_count: u32,
    ) -> Result<Gfx942SdmaLogicalMuxObservationV2, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if self.sdma.is_some() || self.striped_sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        if !gfx942_sdma_logical_mux_lane_count_is_admitted_v2(logical_lane_count) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "logical-mux SDMA lane count must be one of 2,4,8,14,16",
            ));
        }
        let reserved = self.active_compute_queue_ids_for_sdma_creation_v1()?;
        let key = self.key;
        let (owner, observation) = self.with_sdma_queue_creation_custody_v1(
            "logical-mux SDMA queue creation",
            |memory| {
                Gfx942SdmaQueueSetV1::create_logical_mux_v2(
                    memory,
                    key,
                    logical_lane_count,
                    &reserved,
                )
            },
            |(owner, _)| Gfx942SdmaQueueSetV1::retain_created_for_terminal(owner, None),
        )?;
        self.sdma = Some(owner);
        Ok(observation)
    }

    /// Adds the directional pair and a co-resident balanced striped queue set.
    ///
    /// One queue per engine remains reserved for the directional pair, so the
    /// striped count is even and bounded to `2..=14` (seven per ordinary engine).
    // Inline terminal custody avoids a fallible allocation after native state changes.
    #[allow(clippy::result_large_err)]
    pub fn enable_gfx942_directional_and_striped_sdma_copy_engines_v1(
        &mut self,
        striped_queue_count: u32,
    ) -> Result<Gfx942CombinedSdmaCapacityV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if self.sdma.is_some() || self.striped_sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        if !combined_striped_sdma_queue_count_is_admitted(striped_queue_count) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "combined striped SDMA queue count must be even and in 2..=14",
            ));
        }
        let reserved_queue_ids = self.active_compute_queue_ids_for_sdma_creation_v1()?;
        let key = self.key;
        let (directional, striped, capacity) = self.with_sdma_queue_creation_custody_v1(
            "combined SDMA queue creation",
            |memory| {
                Gfx942SdmaQueueSetV1::create_combined_directional_and_striped(
                    memory,
                    key,
                    striped_queue_count,
                    &reserved_queue_ids,
                )
            },
            |(directional, striped, _)| {
                Gfx942SdmaQueueSetV1::retain_created_for_terminal(directional, Some(striped))
            },
        )?;
        self.sdma = Some(directional);
        self.striped_sdma = Some(striped);
        Ok(capacity)
    }

    pub fn allocate_sdma_host_buffer(
        &mut self,
        bytes: usize,
    ) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        self.require_sdma_enabled()?;
        let next_outstanding = self.sdma_outstanding_buffers.checked_add(1).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger exhausted"),
        )?;
        let owner = self.key;
        let buffer = self.with_live_queue_memory_model(|memory| {
            allocate_host_buffer(memory, owner, bytes).map_err(Into::into)
        })?;
        self.sdma_outstanding_buffers = next_outstanding;
        Ok(buffer)
    }

    pub fn allocate_sdma_device_buffer(
        &mut self,
        bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        self.require_sdma_enabled()?;
        let next_outstanding = self.sdma_outstanding_buffers.checked_add(1).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger exhausted"),
        )?;
        let owner = self.key;
        let buffer = self.with_live_queue_memory_model(|memory| {
            allocate_device_buffer(memory, owner, bytes, alignment).map_err(Into::into)
        })?;
        self.sdma_outstanding_buffers = next_outstanding;
        Ok(buffer)
    }

    pub fn allocate_sdma_pooled_host_buffer(
        &mut self,
        bytes: usize,
    ) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        let requested = u64::try_from(bytes).map_err(|_| {
            ComputeAqlQueueSessionErrorV1::Contract("pooled host-buffer size conversion")
        })?;
        if let Some(mut buffer) =
            self.checkout_sdma_pool(Gfx942SdmaBufferKindV1::HostVisibleCoherent, requested, 1)?
        {
            buffer.set_logical_bytes(requested);
            return Ok(buffer);
        }
        self.allocate_sdma_host_buffer(bytes)
    }

    pub fn allocate_sdma_pooled_device_buffer(
        &mut self,
        bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "pooled device-buffer alignment",
            ));
        }
        if let Some(mut buffer) =
            self.checkout_sdma_pool(Gfx942SdmaBufferKindV1::DeviceLocal, bytes, alignment)?
        {
            buffer.set_logical_bytes(bytes);
            return Ok(buffer);
        }
        self.allocate_sdma_device_buffer(bytes, alignment)
    }

    /// Promotes one exact queue-owned device buffer into the R18 persistent
    /// adapter without changing the queue's outstanding-buffer debit.
    ///
    /// Admission is intentionally narrow: one full physical extent of at most
    /// 256 MiB on a single targeted gfx942 engine. Engine 1 admits H2D and
    /// engine 0 admits D2H. Directional and striped queue sets are rejected.
    #[allow(clippy::result_large_err)]
    pub fn promote_sdma_device_buffer_to_persistent_allocation_v1(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
    ) -> Result<Gfx942QueuePersistentAllocationV1, Gfx942PersistentSdmaPromotionFailureV1> {
        let recover = |error, buffer| Gfx942PersistentSdmaPromotionFailureV1 {
            error,
            recovered: Some(buffer),
        };
        if !buffer.belongs_to(self.key) {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                buffer,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(recover(error, buffer));
        }
        let logical_bytes = buffer.requested_bytes();
        let physical_bytes = buffer.physical_bytes();
        if buffer.kind() != Gfx942SdmaBufferKindV1::DeviceLocal
            || logical_bytes != physical_bytes
            || logical_bytes == 0
            || logical_bytes > GFX942_PERSISTENT_SDMA_MAX_ALLOCATION_BYTES_V1
            || !logical_bytes.is_multiple_of(crate::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
            || buffer.pool_generation() == 0
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA promotion requires one full page-multiple device extent up to 256 MiB",
                ),
                buffer,
            ));
        }
        let observation = self.sdma.as_ref().and_then(|owner| {
            owner
                .exact_targeted_observation(crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1)
                .or_else(|| {
                    owner.exact_targeted_observation(crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1)
                })
        });
        let Some(observation) = observation else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA promotion requires one exact targeted engine 0 or 1",
                ),
                buffer,
            ));
        };
        let validation = self.with_live_queue_memory_model(|memory| {
            buffer
                .checked_gpu_subrange(memory, 0, physical_bytes)
                .map(|_| ())
                .map_err(Into::into)
        });
        if let Err(error) = validation {
            if self.terminal_poisoned {
                return Err(Gfx942PersistentSdmaPromotionFailureV1 {
                    error,
                    recovered: None,
                });
            }
            return Err(recover(error, buffer));
        }
        match promote_persistent_sdma_custody_v1(
            buffer,
            observation.queue_id,
            observation
                .engine_index
                .expect("exact targeted observation has an engine"),
        ) {
            Ok(allocation) => Ok(allocation),
            Err(_buffer) => {
                self.poison_terminal();
                Err(Gfx942PersistentSdmaPromotionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent SDMA promotion storage substitution",
                    ),
                    recovered: None,
                })
            }
        }
    }

    /// Demotes a quiescent, non-quarantined persistent owner back into the
    /// ordinary SDMA buffer API. The inherited outstanding-buffer debit is
    /// preserved and the pool generation advances exactly once.
    #[allow(clippy::result_large_err)]
    pub fn demote_persistent_allocation_to_sdma_device_buffer_v1(
        &mut self,
        allocation: Gfx942QueuePersistentAllocationV1,
    ) -> Result<Gfx942SdmaBufferV1, Gfx942PersistentSdmaDemotionFailureV1> {
        let recover = |error, allocation| Gfx942PersistentSdmaDemotionFailureV1 {
            error,
            recovered: Some(allocation),
        };
        if allocation.attachment.queue != self.key {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract("foreign persistent SDMA allocation owner"),
                allocation,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(recover(error, allocation));
        }
        if !self.persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA targeted queue attachment changed",
                ),
                allocation,
            ));
        }
        if allocation
            .attachment
            .pool_generation
            .checked_add(1)
            .is_none()
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA pool generation exhausted",
                ),
                allocation,
            ));
        }
        let Some(lease) = allocation.owner.local_native_for_sdma() else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA allocation is active or not local",
                ),
                allocation,
            ));
        };
        let validation = self.with_live_queue_memory_model(|memory| {
            memory
                .mapped_gfx942_device_memory_facts(lease)
                .map(|_| ())
                .map_err(Into::into)
        });
        if let Err(error) = validation {
            if self.terminal_poisoned {
                return Err(Gfx942PersistentSdmaDemotionFailureV1 {
                    error,
                    recovered: None,
                });
            }
            return Err(recover(error, allocation));
        }
        match demote_persistent_sdma_custody_v1(allocation, self.sdma_outstanding_buffers) {
            Ok((buffer, outstanding_buffers)) => {
                self.sdma_outstanding_buffers = outstanding_buffers;
                Ok(buffer)
            }
            Err((error, allocation)) => Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(match error {
                    Gfx942PersistentUseErrorV1::GenerationExhausted => {
                        "persistent SDMA pool generation exhausted"
                    }
                    Gfx942PersistentUseErrorV1::Quarantined => {
                        "persistent SDMA allocation is quarantined"
                    }
                    _ => "persistent SDMA allocation has outstanding uses",
                }),
                allocation,
            )),
        }
    }

    /// Publishes one targeted local H2D or D2H copy while preserving the R17
    /// persistent-use ledger and the existing queue buffer ledger.
    ///
    /// Exactly one ordinary host buffer accompanies the persistent device
    /// allocation. A clean pre-publication rejection returns both. Confirmed
    /// publication returns a move-only submission; indeterminate publication
    /// returns observation-only process-teardown custody.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn submit_persistent_sdma_copy_v1(
        &mut self,
        mut allocation: Gfx942QueuePersistentAllocationV1,
        dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942PersistentSdmaSubmissionV1, Gfx942PersistentSdmaSubmissionFailureV1> {
        let retryable = |error, allocation, host| Gfx942PersistentSdmaSubmissionFailureV1 {
            error,
            custody: Gfx942PersistentSdmaSubmissionCustodyV1::Retryable { allocation, host },
        };
        let direction = allocation.direction();
        if allocation.attachment.queue != self.key || !host.belongs_to(self.key) {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA submission owner substitution",
                ),
                allocation,
                host,
            ));
        }
        match admit_sdma_publication_while_compute_detached(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            SdmaPublicationModeV1::Persistent,
        ) {
            Ok(_) => {}
            Err(Gfx942DispatchBindingErrorV1::Poisoned) => {
                allocation
                    .owner
                    .quarantine_for_caller_reported_currentness_loss();
                return Err(Gfx942PersistentSdmaSubmissionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    custody: Gfx942PersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                        Gfx942PersistentSdmaTerminalCustodyV1 {
                            direction,
                            sequence: None,
                            state: Gfx942PersistentSdmaTerminalStateV1::AdmissionRestored {
                                allocation,
                                host,
                            },
                        },
                    ),
                });
            }
            Err(error) => return Err(retryable(error.into(), allocation, host)),
        }
        if host.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
            || copy_bytes == 0
            || u64::from(copy_bytes) > u64::from(crate::sdma::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
            || host_offset
                .checked_add(u64::from(copy_bytes))
                .is_none_or(|end| end > host.requested_bytes())
            || device_offset
                .checked_add(u64::from(copy_bytes))
                .is_none_or(|end| end > allocation.byte_len())
            || !allocation.owner.local_native_is_attached_for_sdma()
        {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA submission owner, buffer, or range",
                ),
                allocation,
                host,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(retryable(error, allocation, host));
        }
        if !self.persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA targeted queue attachment changed",
                ),
                allocation,
                host,
            ));
        }
        if let Err(error) = self.check_currentness() {
            allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            self.poison_terminal();
            return Err(Gfx942PersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942PersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                    Gfx942PersistentSdmaTerminalCustodyV1 {
                        direction,
                        sequence: None,
                        state: Gfx942PersistentSdmaTerminalStateV1::AdmissionRestored {
                            allocation,
                            host,
                        },
                    },
                ),
            });
        }
        let host_binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, self.key);
        let operation = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let use_request = match Gfx942PersistentUseRequestV1::new(
            operation,
            device_offset,
            u64::from(copy_bytes),
        ) {
            Ok(request) => request,
            Err(error) => {
                return Err(retryable(
                    map_persistent_sdma_use_error(error),
                    allocation,
                    host,
                ));
            }
        };
        let reserved = match allocation.owner.reserve(use_request, dependency) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(retryable(
                    map_persistent_sdma_use_error(failure.error()),
                    allocation,
                    host,
                ));
            }
        };
        let prepared_use = match allocation.owner.prepare(reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, reserved) = failure.into_parts();
                let _ = allocation.owner.cancel_reserved(reserved);
                return Err(retryable(
                    map_persistent_sdma_use_error(error),
                    allocation,
                    host,
                ));
            }
        };
        let lease = match allocation.owner.detach_local_native_for_sdma() {
            Ok(lease) => lease,
            Err(error) => {
                let _ = allocation.owner.cancel_prepared(prepared_use);
                return Err(retryable(
                    map_persistent_sdma_use_error(error),
                    allocation,
                    host,
                ));
            }
        };
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            allocation.attachment.queue,
            allocation.attachment.pool_generation,
            allocation.attachment.logical_bytes,
        );
        let request = persistent_sdma_request(
            direction,
            host,
            host_offset,
            device,
            device_offset,
            copy_bytes,
        );

        let mut requests = Some(vec![request]);
        let mut preparation = None;
        let prepare_operation = self.with_sdma_owner_memory(|owner, memory| {
            preparation = Some(owner.prepare_batch_recoverable(
                memory,
                requests.take().expect("persistent request consumed once"),
            ));
            Ok(())
        });
        let preparation = preparation.unwrap_or_else(|| {
            Err((
                Gfx942SdmaErrorV1::Contract("persistent SDMA preparation did not execute"),
                requests.expect("unexecuted preparation retains request"),
            ))
        });
        let closing_prepare = self.check_currentness();
        let owner_poisoned = self
            .sdma
            .as_ref()
            .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
        let preparation_terminal = prepare_operation.is_err()
            || closing_prepare.is_err()
            || (preparation.is_err() && owner_poisoned);
        let prepared_batch = match preparation {
            Ok(batch) if !preparation_terminal => batch,
            Ok(batch) => {
                let request = batch
                    .into_requests()
                    .pop()
                    .expect("one persistent SDMA request was prepared");
                return Err(self.terminal_prepared_persistent_sdma_failure(
                    prepare_operation
                        .err()
                        .or_else(|| closing_prepare.err())
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent SDMA preparation poisoned its queue",
                        )),
                    allocation,
                    prepared_use,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    host_binding,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ));
            }
            Err((error, mut recovered)) if !preparation_terminal => {
                let request = recovered
                    .pop()
                    .expect("one persistent SDMA request was rejected");
                let (mut allocation, host) = restore_persistent_sdma_request(
                    allocation,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    host_binding,
                    request,
                )
                .unwrap_or_else(|_| unreachable!("exact prepared request must restore"));
                allocation
                    .owner
                    .cancel_prepared(prepared_use)
                    .expect("private prepared use must cancel");
                return Err(retryable(error.into(), allocation, host));
            }
            Err((error, mut recovered)) => {
                let request = recovered
                    .pop()
                    .expect("one persistent SDMA request was rejected");
                return Err(self.terminal_prepared_persistent_sdma_failure(
                    prepare_operation
                        .err()
                        .or_else(|| closing_prepare.err())
                        .unwrap_or_else(|| error.into()),
                    allocation,
                    prepared_use,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    host_binding,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ));
            }
        };

        let planned_ticket = prepared_batch
            .exact_single_ticket()
            .expect("one persistent SDMA request prepares one exact ticket");
        let mut prepared_batch = Some(prepared_batch);
        let mut publication = None;
        let publication_operation = self.with_sdma_owner_memory(|owner, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            publication = Some(
                owner.submit_prepared_batch_with_custody(
                    memory,
                    prepared_batch
                        .take()
                        .expect("persistent prepared batch consumed once"),
                ),
            );
            Ok(())
        });
        if publication.is_none() {
            let request = prepared_batch
                .expect("unexecuted publication retains prepared batch")
                .into_requests()
                .pop()
                .expect("one persistent SDMA request was prepared");
            return Err(self.terminal_prepared_persistent_sdma_failure(
                publication_operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent SDMA publication did not execute",
                    )),
                allocation,
                prepared_use,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                request,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        match publication.expect("executed publication stores an outcome") {
            Err(PreparedSdmaPublicationFailureV1::Recoverable { error, prepared }) => {
                let request = prepared
                    .into_requests()
                    .pop()
                    .expect("one persistent SDMA request was recoverable");
                let closing = self.check_currentness();
                let operation_succeeded = publication_operation.is_ok();
                let closing_succeeded = closing.is_ok();
                let transition = transition_persistent_sdma_publication_v1(
                    PersistentSdmaPreparedCustodyV1 {
                        allocation,
                        prepared: prepared_use,
                        planned_ticket,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    },
                    PersistentSdmaPublicationObservationV1::Recoverable(request),
                    operation_succeeded,
                    closing_succeeded,
                );
                self.finish_persistent_sdma_publication_transition(
                    publication_operation
                        .err()
                        .or_else(|| closing.err())
                        .unwrap_or_else(|| error.into()),
                    transition,
                )
            }
            Err(PreparedSdmaPublicationFailureV1::Retained { error, tickets }) => {
                let ticket = *tickets
                    .first()
                    .expect("one persistent SDMA ticket was retained");
                let transition = transition_persistent_sdma_publication_v1(
                    PersistentSdmaPreparedCustodyV1 {
                        allocation,
                        prepared: prepared_use,
                        planned_ticket,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    },
                    PersistentSdmaPublicationObservationV1::Retained(ticket),
                    publication_operation.is_ok(),
                    false,
                );
                self.finish_persistent_sdma_publication_transition(
                    publication_operation.err().unwrap_or_else(|| error.into()),
                    transition,
                )
            }
            Ok(tickets) => {
                let [ticket] = tickets.as_slice() else {
                    unreachable!("one persistent SDMA request produces one ticket")
                };
                let ticket = *ticket;
                let closing = self.check_currentness();
                let operation_succeeded = publication_operation.is_ok();
                let closing_succeeded = closing.is_ok();
                let transition = transition_persistent_sdma_publication_v1(
                    PersistentSdmaPreparedCustodyV1 {
                        allocation,
                        prepared: prepared_use,
                        planned_ticket,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    },
                    PersistentSdmaPublicationObservationV1::Confirmed(ticket),
                    operation_succeeded,
                    closing_succeeded,
                );
                self.finish_persistent_sdma_publication_transition(
                    publication_operation
                        .err()
                        .or_else(|| closing.err())
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent SDMA ticket identity",
                        )),
                    transition,
                )
            }
        }
    }

    /// Nonblocking completion observation for one persistent SDMA submission.
    /// Pending returns the exact submission unchanged so an async progress
    /// loop can poll again without reconstructing ticket or allocation state.
    #[allow(clippy::result_large_err)]
    pub fn poll_persistent_sdma_copy_v1(
        &mut self,
        submission: Gfx942PersistentSdmaSubmissionV1,
    ) -> Result<Gfx942PersistentSdmaCopyPollV1, Gfx942PersistentSdmaExecutionFailureV1> {
        let pending_failure = |error, submission| Gfx942PersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942PersistentSdmaExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending_failure(
                ComputeAqlQueueSessionErrorV1::Contract("foreign persistent SDMA submission owner"),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(self.terminal_queued_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "terminal queue session requires process teardown",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending_failure(error, submission));
        }
        if !self.persistent_sdma_attachment_is_current(&submission.allocation.attachment) {
            return Err(self.terminal_queued_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA targeted queue attachment changed",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        if !crate::sdma::ticket_matches_queue_occurrence(
            submission.ticket,
            submission.allocation.attachment.queue,
            submission.allocation.attachment.native_queue_id,
        ) {
            return Err(self.terminal_queued_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract("persistent SDMA ticket identity"),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ));
        }
        let ticket = submission.ticket;
        let mut poll_result = None;
        let poll_operation = self.with_sdma_owner_memory(|owner, memory| {
            poll_result = Some(owner.poll(memory, ticket));
            Ok(())
        });
        let Some(poll_result) = poll_result else {
            return Err(self.terminal_queued_persistent_sdma_failure(
                poll_operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent SDMA poll did not execute",
                    )),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        };
        let operation_succeeded = poll_operation.is_ok();
        let (observation, lower_error) = match poll_result {
            Ok(Gfx942SdmaCopyPollV1::Pending) => {
                (PersistentSdmaCompletionObservationV1::Pending, None)
            }
            Err(error) => (
                PersistentSdmaCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(Gfx942SdmaCopyPollV1::Completed(completed)) => (
                PersistentSdmaCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        let transition =
            transition_persistent_sdma_completion_v1(submission, observation, operation_succeeded);
        match transition {
            PersistentSdmaCompletionTransitionV1::Pending(submission) => {
                Ok(Gfx942PersistentSdmaCopyPollV1::Pending(submission))
            }
            PersistentSdmaCompletionTransitionV1::Completed(completed) => {
                Ok(Gfx942PersistentSdmaCopyPollV1::Completed(completed))
            }
            PersistentSdmaCompletionTransitionV1::Timeout(_) => {
                unreachable!("a poll observation cannot produce timeout custody")
            }
            PersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) => Err(self
                .terminal_persistent_sdma_execution_transition(
                    poll_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                )),
        }
    }

    /// Waits for one confirmed persistent SDMA publication. A timeout returns
    /// the same move-only submission with its ticket and both native owners
    /// still retained. Any non-timeout uncertainty is observation-only and
    /// terminal for the queue session.
    #[allow(clippy::result_large_err)]
    pub fn wait_persistent_sdma_copy_for_v1(
        &mut self,
        submission: Gfx942PersistentSdmaSubmissionV1,
        timeout: Duration,
    ) -> Result<Gfx942PersistentSdmaCompletedV1, Gfx942PersistentSdmaExecutionFailureV1> {
        let pending = |error, submission| Gfx942PersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942PersistentSdmaExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract("foreign persistent SDMA submission owner"),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(self.terminal_queued_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "terminal queue session requires process teardown",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self.persistent_sdma_attachment_is_current(&submission.allocation.attachment) {
            return Err(self.terminal_queued_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent SDMA targeted queue attachment changed",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        if !crate::sdma::ticket_matches_queue_occurrence(
            submission.ticket,
            submission.allocation.attachment.queue,
            submission.allocation.attachment.native_queue_id,
        ) {
            return Err(self.terminal_queued_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract("persistent SDMA ticket identity"),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ));
        }

        let ticket = submission.ticket;
        let mut wait_result = None;
        let wait_operation = self.with_sdma_owner_memory(|owner, memory| {
            wait_result = Some(owner.wait_for(memory, ticket, timeout, SdmaWaitProfileV1::Default));
            Ok(())
        });
        let Some(wait_result) = wait_result else {
            return Err(self.terminal_queued_persistent_sdma_failure(
                wait_operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent SDMA wait did not execute",
                    )),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        };
        let operation_succeeded = wait_operation.is_ok();
        let (observation, lower_error) = match wait_result {
            Err(Gfx942SdmaErrorV1::Timeout) => {
                (PersistentSdmaCompletionObservationV1::Timeout, None)
            }
            Err(error) => (
                PersistentSdmaCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(completed) => (
                PersistentSdmaCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        let transition =
            transition_persistent_sdma_completion_v1(submission, observation, operation_succeeded);
        match transition {
            PersistentSdmaCompletionTransitionV1::Timeout(submission) => Err(pending(
                ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                submission,
            )),
            PersistentSdmaCompletionTransitionV1::Completed(completed) => Ok(completed),
            PersistentSdmaCompletionTransitionV1::Pending(_) => {
                unreachable!("a wait observation cannot produce pending custody")
            }
            PersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) => Err(self
                .terminal_persistent_sdma_execution_transition(
                    wait_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                )),
        }
    }

    /// Promotes one pooled or exact-size device buffer into the R19
    /// directional adapter without changing its outstanding-buffer debit.
    #[allow(clippy::result_large_err)]
    pub fn promote_sdma_device_buffer_to_directional_persistent_allocation_v1(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
    ) -> Result<
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942DirectionalPersistentSdmaPromotionFailureV1,
    > {
        let recover = |error, buffer| {
            classify_directional_persistent_sdma_promotion_failure_v1(error, buffer, false)
        };
        let terminal = |error, buffer| {
            classify_directional_persistent_sdma_promotion_failure_v1(error, buffer, true)
        };
        if !buffer.belongs_to(self.key) {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                buffer,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(if self.terminal_poisoned {
                terminal(error, buffer)
            } else {
                recover(error, buffer)
            });
        }
        let logical_bytes = buffer.requested_bytes();
        let physical_bytes = buffer.physical_bytes();
        if buffer.kind() != Gfx942SdmaBufferKindV1::DeviceLocal
            || !directional_persistent_sdma_extents_are_admitted_v1(
                logical_bytes,
                physical_bytes,
                buffer.pool_generation(),
            )
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA promotion requires 0 < logical <= page-rounded physical <= 256 MiB",
                ),
                buffer,
            ));
        }
        let Some(observation) = self
            .sdma
            .as_ref()
            .and_then(Gfx942SdmaQueueSetV1::directional_observation)
        else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA promotion requires one directional queue pair",
                ),
                buffer,
            ));
        };
        let pair = match admit_persistent_directional_sdma_pair_v1(observation) {
            Ok(pair) => pair,
            Err(detail) => {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(detail),
                    buffer,
                ));
            }
        };
        let validation = self.with_live_queue_memory_model(|memory| {
            buffer
                .validate_physical_device_mapping(memory)
                .map_err(Into::into)
        });
        if let Err(error) = validation {
            if self.terminal_poisoned {
                return Err(terminal(error, buffer));
            }
            return Err(recover(error, buffer));
        }
        match promote_directional_persistent_sdma_custody_v1(
            buffer,
            pair,
            self.sdma_outstanding_buffers,
        ) {
            Ok((allocation, outstanding_buffers)) => {
                self.sdma_outstanding_buffers = outstanding_buffers;
                Ok(allocation)
            }
            Err(buffer) => {
                self.poison_terminal();
                Err(terminal(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA promotion custody mismatch",
                    ),
                    buffer,
                ))
            }
        }
    }

    /// Demotes only quiescent, non-quarantined directional custody and advances
    /// the inherited pool generation exactly once.
    #[allow(clippy::result_large_err)]
    pub fn demote_directional_persistent_allocation_to_sdma_device_buffer_v1(
        &mut self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<Gfx942SdmaBufferV1, Gfx942DirectionalPersistentSdmaDemotionFailureV1> {
        let recover = |error, allocation| {
            classify_directional_persistent_sdma_demotion_failure_v1(error, allocation, false)
        };
        let terminal = |error, allocation| {
            classify_directional_persistent_sdma_demotion_failure_v1(error, allocation, true)
        };
        if allocation.attachment.queue != self.key {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA allocation owner",
                ),
                allocation,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(if self.terminal_poisoned {
                terminal(error, allocation)
            } else {
                recover(error, allocation)
            });
        }
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA queue-pair attachment changed",
                ),
                allocation,
            ));
        }
        if allocation
            .attachment
            .pool_generation
            .checked_add(1)
            .is_none()
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA pool generation exhausted",
                ),
                allocation,
            ));
        }
        let Some(lease) = allocation.owner.local_native_for_sdma() else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA allocation is active or not local",
                ),
                allocation,
            ));
        };
        let validation = self.with_live_queue_memory_model(|memory| {
            memory
                .mapped_gfx942_device_memory_facts(lease)
                .map(|_| ())
                .map_err(Into::into)
        });
        if let Err(error) = validation {
            if self.terminal_poisoned {
                return Err(terminal(error, allocation));
            }
            return Err(recover(error, allocation));
        }
        match demote_directional_persistent_sdma_custody_v1(
            allocation,
            self.sdma_outstanding_buffers,
        ) {
            Ok((buffer, outstanding_buffers)) => {
                self.sdma_outstanding_buffers = outstanding_buffers;
                Ok(buffer)
            }
            Err((error, allocation)) => Err(recover(
                map_directional_persistent_sdma_use_error_v1(error),
                allocation,
            )),
        }
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    fn admit_directional_persistent_sdma_request_v1(
        &self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        DirectionalPersistentSdmaAdmittedRequestV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        let retryable =
            |error, allocation, host| Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                },
            };
        let (allocation, host) = admit_directional_persistent_sdma_copy_input_v1(
            self.key,
            self.terminal_poisoned,
            allocation,
            direction,
            host,
            host_offset,
            device_offset,
            copy_bytes,
        )?;
        if !self.directional_sdma_coexists_with_persistent_compute_v1(&allocation, &host) {
            return Err(retryable(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                allocation,
                host,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(retryable(error, allocation, host));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA queue-pair attachment changed",
                ),
                allocation,
                host,
            ));
        }
        Ok(DirectionalPersistentSdmaAdmittedRequestV1 {
            allocation,
            host,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
        })
    }

    #[allow(clippy::result_large_err)]
    fn prepare_admitted_directional_persistent_sdma_request_v1(
        queue: QueueKeyV1,
        admitted: DirectionalPersistentSdmaAdmittedRequestV1,
    ) -> Result<
        DirectionalPersistentSdmaPreparedRequestV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        let DirectionalPersistentSdmaAdmittedRequestV1 {
            mut allocation,
            host,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
        } = admitted;
        let retryable =
            |error, allocation, host| Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                },
            };
        let host_binding = Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, queue);
        let operation = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let use_request = match Gfx942PersistentUseRequestV1::new(
            operation,
            device_offset,
            u64::from(copy_bytes),
        ) {
            Ok(request) => request,
            Err(error) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let reserved = match allocation.owner.reserve(use_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    allocation,
                    host,
                ));
            }
        };
        let prepared_use = match allocation.owner.prepare(reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, reserved) = failure.into_parts();
                let _ = allocation.owner.cancel_reserved(reserved);
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let lease = match allocation.owner.detach_local_native_for_sdma() {
            Ok(lease) => lease,
            Err(error) => {
                let _ = allocation.owner.cancel_prepared(prepared_use);
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            allocation.attachment.queue,
            allocation.attachment.pool_generation,
            allocation.attachment.logical_bytes,
        );
        let request = directional_persistent_sdma_request_v1(
            direction,
            host,
            host_offset,
            device,
            device_offset,
            copy_bytes,
        );
        Ok(DirectionalPersistentSdmaPreparedRequestV1 {
            allocation,
            prepared_use,
            host_binding,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            request,
        })
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    fn prepare_directional_persistent_sdma_request_v1(
        &mut self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        DirectionalPersistentSdmaPreparedRequestV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        let mut admitted = self.admit_directional_persistent_sdma_request_v1(
            allocation,
            direction,
            host,
            host_offset,
            device_offset,
            copy_bytes,
        )?;
        if let Err(error) = self.check_directional_persistent_sdma_operational_currentness() {
            admitted
                .allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            self.poison_terminal();
            return Err(Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                    Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                        direction,
                        sequence: None,
                        state: Gfx942DirectionalPersistentSdmaTerminalStateV1::AdmissionRestored {
                            allocation: admitted.allocation,
                            host: admitted.host,
                        },
                    },
                ),
            });
        }
        Self::prepare_admitted_directional_persistent_sdma_request_v1(self.key, admitted)
    }

    /// Publishes one copy on the explicitly selected member of the attached
    /// directional pair. Sequential uses may repeat or alternate direction.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn submit_directional_persistent_sdma_copy_v1(
        &mut self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaSubmissionV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        let admitted = self.admit_directional_persistent_sdma_request_v1(
            allocation,
            direction,
            host,
            host_offset,
            device_offset,
            copy_bytes,
        )?;
        let handoff_queue = admitted.allocation.attachment.queue;
        let handoff_native_queue_id = admitted.allocation.attachment.pair.queue_id(direction);
        let queue = self.key;
        let mut admitted = Some(admitted);
        let mut outcome = None;
        let fused_operation = self.with_sdma_owner_memory(|owner, memory| {
            let admitted = admitted
                .take()
                .expect("asynchronous directional admission consumed once");
            if let Err(error) = memory.check_queue_operational_currentness() {
                outcome = Some(
                    DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::OpeningCurrentnessLost {
                        admitted,
                        error: error.into(),
                    },
                );
                return Ok(());
            }
            let DirectionalPersistentSdmaPreparedRequestV1 {
                allocation,
                prepared_use,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                request,
            } = match Self::prepare_admitted_directional_persistent_sdma_request_v1(
                queue, admitted,
            ) {
                Ok(prepared) => prepared,
                Err(failure) => {
                    outcome = Some(
                        DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::RequestPreparationRejected(
                            failure,
                        ),
                    );
                    return Ok(());
                }
            };
            let prepared = match owner.prepare_directional_persistent_single_recoverable(memory, request) {
                Ok(prepared) => prepared,
                Err((error, request)) => {
                    let closing = memory.check_queue_operational_currentness();
                    let closing_currentness_succeeded = closing.is_ok();
                    let error = closing
                        .err()
                        .map(Into::into)
                        .unwrap_or_else(|| error.into());
                    outcome = Some(
                        DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::LowerPreparationRejected {
                            prepared_request: DirectionalPersistentSdmaPreparedRequestV1 {
                                allocation,
                                prepared_use,
                                host_binding,
                                direction,
                                host_offset,
                                device_offset,
                                copy_bytes,
                                request,
                            },
                            error,
                            owner_healthy: !owner.is_poisoned(),
                            closing_currentness_succeeded,
                        },
                    );
                    return Ok(());
                }
            };
            let planned_ticket = prepared.ticket();
            let custody = DirectionalPersistentSdmaPreparedCustodyV1 {
                allocation,
                prepared: prepared_use,
                planned_ticket,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            };
            if let Err(error) = memory.check_queue_operational_currentness() {
                let closing_currentness_succeeded =
                    memory.check_queue_operational_currentness().is_ok();
                outcome = Some(
                    DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::Publication {
                        custody,
                        observation: DirectionalPersistentSdmaPublicationObservationV1::Recoverable(
                            prepared.into_request(),
                        ),
                        error: error.into(),
                        preparation_succeeded: false,
                        closing_currentness_succeeded,
                    },
                );
                return Ok(());
            }
            let handoff = DirectionalPersistentSdmaSinglePreparedHandoffV1 {
                queue: handoff_queue,
                native_queue_id: handoff_native_queue_id,
                direction,
                planned_ticket,
                prepared,
            };
            let (_, _, publication) = handoff.publish(owner, memory);
            let (observation, lower_error) = match publication {
                Err(PreparedSingleSdmaPublicationFailureV1::Recoverable { error, prepared }) => (
                    DirectionalPersistentSdmaPublicationObservationV1::Recoverable(
                        prepared.into_request(),
                    ),
                    error,
                ),
                Err(PreparedSingleSdmaPublicationFailureV1::Retained { error, ticket }) => (
                    DirectionalPersistentSdmaPublicationObservationV1::Retained(ticket),
                    error,
                ),
                Ok(ticket) => (
                    DirectionalPersistentSdmaPublicationObservationV1::Confirmed(ticket),
                    Gfx942SdmaErrorV1::Contract(
                        "directional persistent SDMA post-publication currentness",
                    ),
                ),
            };
            let closing = memory.check_queue_operational_currentness();
            let closing_currentness_succeeded = closing.is_ok();
            let error = closing
                .err()
                .map(Into::into)
                .unwrap_or_else(|| lower_error.into());
            outcome = Some(
                DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::Publication {
                    custody,
                    observation,
                    error,
                    preparation_succeeded: true,
                    closing_currentness_succeeded,
                },
            );
            Ok(())
        });

        self.finish_asynchronous_directional_persistent_sdma_single_v1(
            direction,
            admitted,
            outcome,
            fused_operation.err(),
        )
    }

    /// Executes the runtime's bounded synchronous single-packet directional
    /// copy without reopening the standalone asynchronous observation path.
    ///
    /// This hidden composition preserves the public submit/poll/wait surface.
    /// One opening observation admits preparation; one owner-memory loan then
    /// contains preparation, the prepublication observation, publication,
    /// bounded completion observation, and the final observation before the
    /// lower completed record is removed.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn execute_synchronous_directional_persistent_sdma_copy_for_v1(
        &mut self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        timeout: Duration,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaCompletedV1,
        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
    > {
        let retryable =
            |error, allocation, host| Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                },
            };
        let prepared_request = match self.prepare_directional_persistent_sdma_request_v1(
            allocation,
            direction,
            host,
            host_offset,
            device_offset,
            copy_bytes,
        ) {
            Ok(prepared_request) => prepared_request,
            Err(failure) => {
                return Err(
                    Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
                        failure,
                    ),
                );
            }
        };
        let DirectionalPersistentSdmaPreparedRequestV1 {
            allocation,
            prepared_use,
            host_binding,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            request,
        } = prepared_request;

        let handoff_queue = allocation.attachment.queue;
        let handoff_native_queue_id = allocation.attachment.pair.queue_id(direction);
        let mut allocation = Some(allocation);
        let mut prepared_use = Some(prepared_use);
        let mut request = Some(request);
        let mut outcome = None;
        let fused_operation = self.with_sdma_owner_memory(|owner, memory| {
            let allocation = allocation
                .take()
                .expect("synchronous directional allocation consumed once");
            let prepared_use = prepared_use
                .take()
                .expect("synchronous directional prepared use consumed once");
            let request = request
                .take()
                .expect("synchronous directional request consumed once");
            let prepared = match owner.prepare_directional_persistent_single_recoverable(memory, request) {
                Ok(prepared) => prepared,
                Err((error, request)) => {
                    let closing = memory.check_queue_operational_currentness();
                    let closing_currentness_succeeded = closing.is_ok();
                    let error = closing
                        .err()
                        .map(Into::into)
                        .unwrap_or_else(|| error.into());
                    outcome = Some(
                        DirectionalPersistentSdmaSynchronousSingleOutcomeV1::PreparationRejected {
                            allocation,
                            prepared: prepared_use,
                            host_binding,
                            direction,
                            host_offset,
                            device_offset,
                            copy_bytes,
                            request,
                            error,
                            owner_healthy: !owner.is_poisoned(),
                            closing_currentness_succeeded,
                        },
                    );
                    return Ok(());
                }
            };
            let planned_ticket = prepared.ticket();
            let custody = DirectionalPersistentSdmaPreparedCustodyV1 {
                allocation,
                prepared: prepared_use,
                planned_ticket,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            };
            if let Err(error) = memory.check_queue_operational_currentness() {
                outcome = Some(
                    DirectionalPersistentSdmaSynchronousSingleOutcomeV1::BeforePublication {
                        custody,
                        observation: DirectionalPersistentSdmaPublicationObservationV1::Recoverable(
                            prepared.into_request(),
                        ),
                        error: error.into(),
                        preparation_succeeded: false,
                        closing_currentness_succeeded: false,
                    },
                );
                return Ok(());
            }
            let handoff = DirectionalPersistentSdmaSinglePreparedHandoffV1 {
                queue: handoff_queue,
                native_queue_id: handoff_native_queue_id,
                direction,
                planned_ticket,
                prepared,
            };
            let (direction, planned_ticket, publication) = handoff.publish(owner, memory);
            match publication {
                Err(PreparedSingleSdmaPublicationFailureV1::Recoverable { error, prepared }) => {
                    let closing = memory.check_queue_operational_currentness();
                    let closing_currentness_succeeded = closing.is_ok();
                    let error = closing
                        .err()
                        .map(Into::into)
                        .unwrap_or_else(|| error.into());
                    outcome = Some(
                        DirectionalPersistentSdmaSynchronousSingleOutcomeV1::BeforePublication {
                            custody,
                            observation:
                                DirectionalPersistentSdmaPublicationObservationV1::Recoverable(
                                    prepared.into_request(),
                                ),
                            error,
                            preparation_succeeded: !owner.is_poisoned(),
                            closing_currentness_succeeded,
                        },
                    );
                }
                Err(PreparedSingleSdmaPublicationFailureV1::Retained { error, ticket }) => {
                    let closing = memory.check_queue_operational_currentness();
                    let closing_currentness_succeeded = closing.is_ok();
                    let error = closing
                        .err()
                        .map(Into::into)
                        .unwrap_or_else(|| error.into());
                    outcome = Some(
                        DirectionalPersistentSdmaSynchronousSingleOutcomeV1::BeforePublication {
                            custody,
                            observation: DirectionalPersistentSdmaPublicationObservationV1::Retained(
                                ticket,
                            ),
                            error,
                            preparation_succeeded: true,
                            closing_currentness_succeeded,
                        },
                    );
                }
                Ok(ticket) => {
                    let DirectionalPersistentSdmaPreparedCustodyV1 {
                        mut allocation,
                        prepared,
                        host_binding,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        ..
                    } = custody;
                    let published = allocation
                        .owner
                        .publish(prepared)
                        .expect("confirmed synchronous publication advances prepared use");
                    let submission = Gfx942DirectionalPersistentSdmaSubmissionV1 {
                        allocation,
                        published,
                        ticket,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    };
                    if ticket != planned_ticket
                        || !planned_ticket_matches_queue_occurrence(
                            planned_ticket,
                            handoff_queue,
                            handoff_native_queue_id,
                        )
                    {
                        let final_currentness = memory.check_queue_operational_currentness();
                        let final_currentness_succeeded = final_currentness.is_ok();
                        let error = final_currentness.err().map(Into::into).unwrap_or(
                            ComputeAqlQueueSessionErrorV1::Contract(
                                "directional persistent SDMA synchronous publication ticket identity",
                            ),
                        );
                        outcome = Some(
                            DirectionalPersistentSdmaSynchronousSingleOutcomeV1::Published {
                                submission,
                                observation:
                                    DirectionalPersistentSdmaCompletionObservationV1::QueueRetained,
                                error: Some(error),
                                final_currentness_succeeded,
                            },
                        );
                        return Ok(());
                    }
                    let (observation, error, final_currentness_succeeded) =
                        match owner.wait_for_in_current_scope_with_final_currentness(
                            memory,
                            ticket,
                            timeout,
                        ) {
                            SingleSdmaWaitInCurrentScopeV1::Completed(completed) => (
                                DirectionalPersistentSdmaCompletionObservationV1::Completed(
                                    completed,
                                ),
                                None,
                                true,
                            ),
                            SingleSdmaWaitInCurrentScopeV1::Timeout => (
                                DirectionalPersistentSdmaCompletionObservationV1::Timeout,
                                Some(ComputeAqlQueueSessionErrorV1::Sdma(
                                    Gfx942SdmaErrorV1::Timeout,
                                )),
                                true,
                            ),
                            SingleSdmaWaitInCurrentScopeV1::QueueRetained(error) => (
                                DirectionalPersistentSdmaCompletionObservationV1::QueueRetained,
                                Some(error.into()),
                                true,
                            ),
                            SingleSdmaWaitInCurrentScopeV1::FinalCurrentnessLost(error) => (
                                DirectionalPersistentSdmaCompletionObservationV1::QueueRetained,
                                Some(error.into()),
                                false,
                            ),
                        };
                    outcome = Some(
                        DirectionalPersistentSdmaSynchronousSingleOutcomeV1::Published {
                            submission,
                            observation,
                            error,
                            final_currentness_succeeded,
                        },
                    );
                }
            }
            Ok(())
        });

        let loan_error = fused_operation.err();
        let Some(outcome) = outcome else {
            let failure = self.terminal_prepared_directional_persistent_sdma_failure(
                loan_error.unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "synchronous directional persistent SDMA operation did not execute",
                )),
                allocation.expect("unopened synchronous loan retains allocation"),
                prepared_use.expect("unopened synchronous loan retains prepared use"),
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                request.expect("unopened synchronous loan retains request"),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            return Err(
                Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(failure),
            );
        };
        match outcome {
            DirectionalPersistentSdmaSynchronousSingleOutcomeV1::PreparationRejected {
                allocation,
                prepared,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                request,
                error,
                owner_healthy,
                closing_currentness_succeeded,
            } => {
                if loan_error.is_none() && owner_healthy && closing_currentness_succeeded {
                    match restore_directional_persistent_sdma_request_v1(
                        allocation,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        host_binding,
                        request,
                    ) {
                        Ok((mut allocation, host)) => {
                            allocation
                                .owner
                                .cancel_prepared(prepared)
                                .expect("private synchronous prepared use must cancel");
                            return Err(
                                Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
                                    retryable(error, allocation, host),
                                ),
                            );
                        }
                        Err((allocation, request)) => {
                            let failure = self
                                .terminal_prepared_directional_persistent_sdma_failure(
                                    ComputeAqlQueueSessionErrorV1::Contract(
                                        "synchronous directional persistent SDMA preparation restoration",
                                    ),
                                    allocation,
                                    prepared,
                                    direction,
                                    host_offset,
                                    device_offset,
                                    copy_bytes,
                                    host_binding,
                                    request,
                                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                                );
                            return Err(
                                Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
                                    failure,
                                ),
                            );
                        }
                    }
                }
                let failure = self.terminal_prepared_directional_persistent_sdma_failure(
                    loan_error.unwrap_or(error),
                    allocation,
                    prepared,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    host_binding,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                Err(
                    Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
                        failure,
                    ),
                )
            }
            DirectionalPersistentSdmaSynchronousSingleOutcomeV1::BeforePublication {
                custody,
                observation,
                error,
                preparation_succeeded,
                closing_currentness_succeeded,
            } => {
                let enclosing_operation_succeeded = loan_error.is_none() && preparation_succeeded;
                let transition = transition_directional_persistent_sdma_publication_v1(
                    custody,
                    observation,
                    enclosing_operation_succeeded,
                    loan_error.is_none() && closing_currentness_succeeded,
                );
                let error = loan_error.unwrap_or(error);
                match self
                    .finish_directional_persistent_sdma_publication_transition(error, transition)
                {
                    Ok(_) => unreachable!("prepublication outcome cannot publish"),
                    Err(failure) => Err(
                        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
                            failure,
                        ),
                    ),
                }
            }
            DirectionalPersistentSdmaSynchronousSingleOutcomeV1::Published {
                submission,
                observation,
                error,
                final_currentness_succeeded,
            } => {
                let transition = transition_directional_persistent_sdma_completion_v1(
                    submission,
                    observation,
                    loan_error.is_none() && final_currentness_succeeded,
                );
                match transition {
                    DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) => {
                        Ok(completed)
                    }
                    DirectionalPersistentSdmaCompletionTransitionV1::Timeout(submission) => Err(
                        Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(
                            Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
                                error: error.unwrap_or(ComputeAqlQueueSessionErrorV1::Sdma(
                                    Gfx942SdmaErrorV1::Timeout,
                                )),
                                custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(
                                    submission,
                                ),
                            },
                        ),
                    ),
                    DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) => {
                        let failure = self
                            .terminal_directional_persistent_sdma_execution_transition(
                            loan_error.or(error).unwrap_or(
                                ComputeAqlQueueSessionErrorV1::Contract(
                                    "synchronous directional persistent SDMA completion identity",
                                ),
                            ),
                            custody,
                        );
                        Err(
                            Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(
                                failure,
                            ),
                        )
                    }
                    DirectionalPersistentSdmaCompletionTransitionV1::Pending(_) => {
                        unreachable!("bounded synchronous wait cannot return pending success")
                    }
                }
            }
        }
    }

    /// Observes one directional persistent copy without blocking.
    #[allow(clippy::result_large_err)]
    pub fn poll_directional_persistent_sdma_copy_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaCopyPollV1,
        Gfx942DirectionalPersistentSdmaExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA submission owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "terminal queue session requires process teardown",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self
            .directional_persistent_sdma_attachment_is_current(&submission.allocation.attachment)
        {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA queue-pair attachment changed",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        let expected_queue = submission
            .allocation
            .attachment
            .pair
            .queue_id(submission.direction);
        if !crate::sdma::ticket_matches_queue_occurrence(
            submission.ticket,
            submission.allocation.attachment.queue,
            expected_queue,
        ) {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA ticket identity",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ));
        }
        let ticket = submission.ticket;
        let mut poll_result = None;
        let poll_operation = self.with_sdma_owner_memory(|owner, memory| {
            poll_result = Some(owner.poll(memory, ticket));
            Ok(())
        });
        let Some(poll_result) = poll_result else {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                poll_operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA poll did not execute",
                    )),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        };
        let (observation, lower_error) = match poll_result {
            Ok(Gfx942SdmaCopyPollV1::Pending) => (
                DirectionalPersistentSdmaCompletionObservationV1::Pending,
                None,
            ),
            Err(error) => (
                DirectionalPersistentSdmaCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(Gfx942SdmaCopyPollV1::Completed(completed)) => (
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_directional_persistent_sdma_completion_v1(
            submission,
            observation,
            poll_operation.is_ok(),
        ) {
            DirectionalPersistentSdmaCompletionTransitionV1::Pending(submission) => Ok(
                Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(submission),
            ),
            DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) => Ok(
                Gfx942DirectionalPersistentSdmaCopyPollV1::Completed(completed),
            ),
            DirectionalPersistentSdmaCompletionTransitionV1::Timeout(_) => {
                unreachable!("poll cannot produce timeout custody")
            }
            DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) => Err(self
                .terminal_directional_persistent_sdma_execution_transition(
                    poll_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                )),
        }
    }

    /// Waits until completion or the supplied deadline. Timeout returns the
    /// exact published submission for a later wait or poll.
    #[allow(clippy::result_large_err)]
    pub fn wait_directional_persistent_sdma_copy_for_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
        timeout: Duration,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaCompletedV1,
        Gfx942DirectionalPersistentSdmaExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA submission owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "terminal queue session requires process teardown",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self
            .directional_persistent_sdma_attachment_is_current(&submission.allocation.attachment)
        {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA queue-pair attachment changed",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        }
        let expected_queue = submission
            .allocation
            .attachment
            .pair
            .queue_id(submission.direction);
        if !crate::sdma::ticket_matches_queue_occurrence(
            submission.ticket,
            submission.allocation.attachment.queue,
            expected_queue,
        ) {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA ticket identity",
                ),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ));
        }
        let ticket = submission.ticket;
        let mut wait_result = None;
        let wait_operation = self.with_sdma_owner_memory(|owner, memory| {
            wait_result = Some(owner.wait_for(
                memory,
                ticket,
                timeout,
                SdmaWaitProfileV1::PersistentElapsedSpinFloor(PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1),
            ));
            Ok(())
        });
        let Some(wait_result) = wait_result else {
            return Err(self.terminal_queued_directional_persistent_sdma_failure(
                wait_operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA wait did not execute",
                    )),
                submission,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            ));
        };
        let (observation, lower_error) = match wait_result {
            Err(Gfx942SdmaErrorV1::Timeout) => (
                DirectionalPersistentSdmaCompletionObservationV1::Timeout,
                None,
            ),
            Err(error) => (
                DirectionalPersistentSdmaCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(completed) => (
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_directional_persistent_sdma_completion_v1(
            submission,
            observation,
            wait_operation.is_ok(),
        ) {
            DirectionalPersistentSdmaCompletionTransitionV1::Timeout(submission) => Err(pending(
                ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                submission,
            )),
            DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) => Ok(completed),
            DirectionalPersistentSdmaCompletionTransitionV1::Pending(_) => {
                unreachable!("wait cannot produce pending custody")
            }
            DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) => Err(self
                .terminal_directional_persistent_sdma_execution_transition(
                    wait_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                )),
        }
    }

    /// Publishes one aggregate directional copy as a bounded packet window.
    ///
    /// The window owns one host/device pair and one persistent use lease. All
    /// packet slots are prepared before one write-pointer publication and one
    /// doorbell store make the complete window visible to the device.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn submit_directional_persistent_sdma_window_v1(
        &mut self,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: Gfx942SdmaBufferV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1,
    > {
        let retryable =
            |error, allocation, host| Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                },
            };
        let (allocation, host, packet_count) = admit_directional_persistent_sdma_window_input_v1(
            self.key,
            self.terminal_poisoned,
            allocation,
            direction,
            host,
            host_offset,
            device_offset,
            copy_bytes,
        )?;
        if !self.directional_sdma_coexists_with_persistent_compute_v1(&allocation, &host) {
            return Err(retryable(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                allocation,
                host,
            ));
        }
        let mut allocation = allocation;
        if let Err(error) = self.require_sdma_enabled() {
            return Err(retryable(error, allocation, host));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA window queue-pair attachment changed",
                ),
                allocation,
                host,
            ));
        }
        let mut planned_tickets = Vec::new();
        if planned_tickets.try_reserve_exact(packet_count).is_err() {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA planned ticket roster allocation",
                ),
                allocation,
                host,
            ));
        }
        if let Err(error) = self.check_directional_persistent_sdma_operational_currentness() {
            allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            self.poison_terminal();
            return Err(Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody:
                    Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                        Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                            direction,
                            sequence: None,
                            packet_count,
                            state: Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::AdmissionRestored {
                                allocation,
                                host,
                            },
                        },
                    ),
            });
        }

        let host_binding = Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, self.key);
        let operation = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let use_request = match Gfx942PersistentUseRequestV1::new(
            operation,
            device_offset,
            u64::from(copy_bytes),
        ) {
            Ok(request) => request,
            Err(error) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let reserved = match allocation.owner.reserve(use_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    allocation,
                    host,
                ));
            }
        };
        let prepared_use = match allocation.owner.prepare(reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, reserved) = failure.into_parts();
                let _ = allocation.owner.cancel_reserved(reserved);
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let lease = match allocation.owner.detach_local_native_for_sdma() {
            Ok(lease) => lease,
            Err(error) => {
                let _ = allocation.owner.cancel_prepared(prepared_use);
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    allocation,
                    host,
                ));
            }
        };
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            allocation.attachment.queue,
            allocation.attachment.pool_generation,
            allocation.attachment.logical_bytes,
        );
        let request = directional_persistent_sdma_request_v1(
            direction,
            host,
            host_offset,
            device,
            device_offset,
            copy_bytes,
        );

        let handoff_queue = allocation.attachment.queue;
        let handoff_native_queue_id = allocation.attachment.pair.queue_id(direction);
        let mut request = Some(request);
        let mut preparation_failure = None;
        let mut preparation_contract_failed = false;
        let mut prepared_without_handoff = None;
        let mut handoff_attempted = false;
        let mut publication = None;
        let prepare_and_publish_operation = self.with_sdma_owner_memory(|owner, memory| {
            match owner.prepare_persistent_window_recoverable(
                memory,
                request
                    .take()
                    .expect("persistent window request consumed once"),
            ) {
                Ok(prepared) => {
                    if prepared.tickets().len() != packet_count {
                        preparation_contract_failed = true;
                        preparation_failure = Some((
                            Gfx942SdmaErrorV1::Contract(
                                "directional persistent SDMA window prepared ticket count",
                            ),
                            prepared.into_request(),
                        ));
                    } else {
                        planned_tickets.extend_from_slice(prepared.tickets());
                        handoff_attempted = true;
                        if let Err(error) = memory.check_queue_operational_currentness() {
                            prepared_without_handoff = Some(prepared);
                            return Err(error.into());
                        }
                        let handoff = DirectionalPersistentSdmaWindowPreparedHandoffV1 {
                            queue: handoff_queue,
                            native_queue_id: handoff_native_queue_id,
                            direction,
                            packet_count,
                            planned_tickets: core::mem::take(&mut planned_tickets),
                            prepared,
                        };
                        publication = Some(handoff.publish(owner, memory));
                    }
                }
                Err(failure) => preparation_failure = Some(failure),
            }
            Ok(())
        });
        if !handoff_attempted {
            let (lower_error, request) = preparation_failure.unwrap_or_else(|| {
                (
                    Gfx942SdmaErrorV1::Contract(
                        "directional persistent SDMA window preparation did not execute",
                    ),
                    request.expect("unexecuted window preparation retains request"),
                )
            });
            let closing_prepare = self.check_directional_persistent_sdma_operational_currentness();
            let owner_poisoned = self
                .sdma
                .as_ref()
                .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
            if prepare_and_publish_operation.is_err()
                || closing_prepare.is_err()
                || owner_poisoned
                || preparation_contract_failed
            {
                return Err(
                    self.terminal_prepared_directional_persistent_sdma_window_failure(
                        prepare_and_publish_operation
                            .err()
                            .or_else(|| closing_prepare.err())
                            .unwrap_or_else(|| lower_error.into()),
                        allocation,
                        prepared_use,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        packet_count,
                        host_binding,
                        request,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    ),
                );
            }
            let (mut allocation, host) = restore_directional_persistent_sdma_request_v1(
                allocation,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                request,
            )
            .unwrap_or_else(|_| unreachable!("exact prepared window request must restore"));
            allocation
                .owner
                .cancel_prepared(prepared_use)
                .expect("private prepared window use must cancel");
            return Err(retryable(lower_error.into(), allocation, host));
        }
        let Some((handoff_direction, handoff_packet_count, planned_tickets, publication)) =
            publication
        else {
            return Err(
                self.terminal_prepared_directional_persistent_sdma_window_failure(
                    prepare_and_publish_operation.err().unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window handoff did not publish",
                        ),
                    ),
                    allocation,
                    prepared_use,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    packet_count,
                    host_binding,
                    prepared_without_handoff
                        .expect("failed handoff retains window preparation")
                        .into_request(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let direction = handoff_direction;
        let packet_count = handoff_packet_count;
        let (observation, lower_error) = match publication {
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable {
                error,
                prepared,
            }) => (
                DirectionalPersistentSdmaWindowPublicationObservationV1::Recoverable(
                    prepared.into_request(),
                ),
                error,
            ),
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Retained { error, tickets }) => (
                DirectionalPersistentSdmaWindowPublicationObservationV1::Retained(tickets),
                error,
            ),
            Ok(tickets) => (
                DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                Gfx942SdmaErrorV1::Contract(
                    "directional persistent SDMA window post-publication currentness",
                ),
            ),
        };
        let closing = self.check_directional_persistent_sdma_operational_currentness();
        let transition = transition_directional_persistent_sdma_window_publication_v1(
            DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                allocation,
                prepared: prepared_use,
                planned_tickets,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                packet_count,
            },
            observation,
            prepare_and_publish_operation.is_ok(),
            closing.is_ok(),
        );
        self.finish_directional_persistent_sdma_window_publication_transition(
            prepare_and_publish_operation
                .err()
                .or_else(|| closing.err())
                .unwrap_or_else(|| lower_error.into()),
            transition,
        )
    }

    /// Observes a complete persistent packet window without retiring a prefix.
    #[allow(clippy::result_large_err)]
    pub fn poll_directional_persistent_sdma_window_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowCopyPollV1,
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self
            .directional_persistent_sdma_attachment_is_current(&submission.allocation.attachment)
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission
            .allocation
            .attachment
            .pair
            .queue_id(submission.direction);
        if submission.tickets.len() != submission.packet_count
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.allocation.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut poll_result = None;
        let poll_operation = self.with_sdma_owner_memory(|owner, memory| {
            poll_result = Some(owner.poll_persistent_window(memory, &submission.tickets));
            Ok(())
        });
        let Some(poll_result) = poll_result else {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    poll_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window poll did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match poll_result {
            Ok(PersistentSdmaWindowPollV1::Pending) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Pending,
                None,
            ),
            Err(error) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(PersistentSdmaWindowPollV1::Completed(completed)) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_directional_persistent_sdma_window_completion_v1(
            submission,
            observation,
            poll_operation.is_ok(),
        ) {
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(submission) => Ok(
                Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Pending(submission),
            ),
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) => Ok(
                Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Completed(completed),
            ),
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(_) => {
                unreachable!("window poll cannot produce timeout custody")
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_directional_persistent_sdma_window_execution_transition(
                    poll_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }

    /// Waits for every packet in a persistent window. Timeout returns the
    /// unchanged aggregate submission for a later wait or poll.
    #[allow(clippy::result_large_err)]
    pub fn wait_directional_persistent_sdma_window_for_v1(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        timeout: Duration,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowCompletedV1,
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.allocation.attachment.queue != self.key {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign directional persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self
            .directional_persistent_sdma_attachment_is_current(&submission.allocation.attachment)
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission
            .allocation
            .attachment
            .pair
            .queue_id(submission.direction);
        if submission.tickets.len() != submission.packet_count
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.allocation.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "directional persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut wait_result = None;
        let wait_operation = self.with_sdma_owner_memory(|owner, memory| {
            wait_result = Some(owner.wait_persistent_window_for(
                memory,
                &submission.tickets,
                timeout,
                SdmaWaitProfileV1::PersistentElapsedSpinFloor(PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1),
            ));
            Ok(())
        });
        let Some(wait_result) = wait_result else {
            return Err(
                self.terminal_queued_directional_persistent_sdma_window_failure(
                    wait_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window wait did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match wait_result {
            Err(Gfx942SdmaErrorV1::Timeout) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Timeout,
                None,
            ),
            Err(error) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(completed) => (
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_directional_persistent_sdma_window_completion_v1(
            submission,
            observation,
            wait_operation.is_ok(),
        ) {
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(submission) => {
                Err(pending(
                    ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    submission,
                ))
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) => {
                Ok(completed)
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(_) => {
                unreachable!("window wait cannot produce pending custody")
            }
            DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_directional_persistent_sdma_window_execution_transition(
                    wait_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "directional persistent SDMA window completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }

    /// Publishes one same-device D2D copy from two distinct persistent owners.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub fn submit_same_device_persistent_sdma_window_v1(
        &mut self,
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        source_offset: u64,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1,
    > {
        let retryable =
            |error, source, destination| Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    source,
                    destination,
                },
            };
        let (mut source, mut destination, descriptor) =
            admit_same_device_persistent_sdma_window_input_v1(
                self.key,
                self.terminal_poisoned,
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            )?;
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.has_any_persistent_compute_attachment_v1(),
            SdmaPublicationModeV1::SameDeviceWindow,
        ) {
            return Err(retryable(error.into(), source, destination));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(retryable(error, source, destination));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&source.attachment)
            || !self.directional_persistent_sdma_attachment_is_current(&destination.attachment)
        {
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "same-device persistent SDMA queue-pair attachment changed",
                ),
                source,
                destination,
            ));
        }
        if let Err(error) = self.check_directional_persistent_sdma_operational_currentness() {
            source
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            destination
                .owner
                .quarantine_for_caller_reported_currentness_loss();
            self.poison_terminal();
            return Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                        source_sequence: None,
                        destination_sequence: None,
                        descriptor,
                        state:
                            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::AdmissionRestored {
                                source,
                                destination,
                            },
                    },
                ),
            });
        }

        let source_request = match same_device_source_use_request_v1(source_offset, copy_bytes) {
            Ok(request) => request,
            Err(error) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let destination_request =
            match same_device_destination_use_request_v1(destination_offset, copy_bytes) {
                Ok(request) => request,
                Err(error) => {
                    return Err(retryable(
                        map_directional_persistent_sdma_use_error_v1(error),
                        source,
                        destination,
                    ));
                }
            };
        let source_reserved = match source.owner.reserve(source_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    source,
                    destination,
                ));
            }
        };
        let destination_reserved = match destination.owner.reserve(destination_request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                source
                    .owner
                    .cancel_reserved(source_reserved)
                    .expect("private source reservation must cancel");
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    source,
                    destination,
                ));
            }
        };
        let source_prepared = match source.owner.prepare(source_reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, source_reserved) = failure.into_parts();
                source
                    .owner
                    .cancel_reserved(source_reserved)
                    .expect("private source reservation must cancel");
                destination
                    .owner
                    .cancel_reserved(destination_reserved)
                    .expect("private destination reservation must cancel");
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let destination_prepared = match destination.owner.prepare(destination_reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, destination_reserved) = failure.into_parts();
                source
                    .owner
                    .cancel_prepared(source_prepared)
                    .expect("private source preparation must cancel");
                destination
                    .owner
                    .cancel_reserved(destination_reserved)
                    .expect("private destination reservation must cancel");
                return Err(retryable(
                    map_directional_persistent_sdma_use_error_v1(error),
                    source,
                    destination,
                ));
            }
        };
        let (source_lease, destination_lease) =
            match detach_local_native_pair_for_sdma_v1(&mut source.owner, &mut destination.owner) {
                Ok(leases) => leases,
                Err(error) => {
                    cancel_prepared_local_sdma_pair_v1(
                        &mut source.owner,
                        source_prepared,
                        &mut destination.owner,
                        destination_prepared,
                    )
                    .unwrap_or_else(|failure| panic!("private prepared pair: {:?}", failure.error));
                    return Err(retryable(
                        map_directional_persistent_sdma_use_error_v1(error),
                        source,
                        destination,
                    ));
                }
            };
        let source_buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(source_lease),
            source.attachment.queue,
            source.attachment.pool_generation,
            source.attachment.logical_bytes,
        );
        let destination_buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(destination_lease),
            destination.attachment.queue,
            destination.attachment.pool_generation,
            destination.attachment.logical_bytes,
        );
        let request = same_device_persistent_sdma_request_v1(
            source_buffer,
            source_offset,
            destination_buffer,
            destination_offset,
            copy_bytes,
        );

        let mut request = Some(request);
        let mut preparation = None;
        let prepare_operation = self.with_sdma_owner_memory(|owner, memory| {
            preparation = Some(owner.prepare_same_device_persistent_window_recoverable(
                memory,
                request.take().expect("same-device request consumed once"),
            ));
            Ok(())
        });
        let preparation = preparation.unwrap_or_else(|| {
            Err((
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA preparation did not execute",
                ),
                request.expect("unexecuted same-device preparation retains request"),
            ))
        });
        let closing_prepare = self.check_directional_persistent_sdma_operational_currentness();
        let owner_poisoned = self
            .sdma
            .as_ref()
            .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
        let preparation_terminal = prepare_operation.is_err()
            || closing_prepare.is_err()
            || (preparation.is_err() && owner_poisoned);
        let prepared_lower = match preparation {
            Ok(prepared) if !preparation_terminal => prepared,
            Ok(prepared) => {
                return Err(
                    self.terminal_prepared_same_device_persistent_sdma_window_failure(
                        prepare_operation
                            .err()
                            .or_else(|| closing_prepare.err())
                            .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                                "same-device persistent SDMA preparation poisoned its queue",
                            )),
                        source,
                        source_prepared,
                        destination,
                        destination_prepared,
                        descriptor,
                        prepared.into_request(),
                    ),
                );
            }
            Err((error, request)) if !preparation_terminal => {
                let (mut source, mut destination) = restore_same_device_persistent_sdma_request_v1(
                    source,
                    destination,
                    descriptor,
                    request,
                )
                .unwrap_or_else(|_| unreachable!("exact same-device request must restore"));
                cancel_prepared_local_sdma_pair_v1(
                    &mut source.owner,
                    source_prepared,
                    &mut destination.owner,
                    destination_prepared,
                )
                .unwrap_or_else(|failure| panic!("private prepared pair: {:?}", failure.error));
                return Err(retryable(error.into(), source, destination));
            }
            Err((error, request)) => {
                return Err(
                    self.terminal_prepared_same_device_persistent_sdma_window_failure(
                        prepare_operation
                            .err()
                            .or_else(|| closing_prepare.err())
                            .unwrap_or_else(|| error.into()),
                        source,
                        source_prepared,
                        destination,
                        destination_prepared,
                        descriptor,
                        request,
                    ),
                );
            }
        };

        let mut planned_tickets = Vec::new();
        if planned_tickets
            .try_reserve_exact(prepared_lower.tickets().len())
            .is_err()
        {
            let request = prepared_lower.into_request();
            let (mut source, mut destination) = restore_same_device_persistent_sdma_request_v1(
                source,
                destination,
                descriptor,
                request,
            )
            .unwrap_or_else(|_| unreachable!("exact same-device request must restore"));
            cancel_prepared_local_sdma_pair_v1(
                &mut source.owner,
                source_prepared,
                &mut destination.owner,
                destination_prepared,
            )
            .unwrap_or_else(|failure| panic!("private prepared pair: {:?}", failure.error));
            return Err(retryable(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "same-device persistent SDMA planned ticket allocation",
                ),
                source,
                destination,
            ));
        }
        planned_tickets.extend_from_slice(prepared_lower.tickets());
        let mut prepared_lower = Some(prepared_lower);
        let mut publication = None;
        let publication_operation = self.with_sdma_owner_memory(|owner, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            publication = Some(
                owner.submit_prepared_persistent_window_with_custody(
                    memory,
                    prepared_lower
                        .take()
                        .expect("same-device preparation consumed once"),
                ),
            );
            Ok(())
        });
        if publication.is_none() {
            return Err(
                self.terminal_prepared_same_device_persistent_sdma_window_failure(
                    publication_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA publication did not execute",
                        )),
                    source,
                    source_prepared,
                    destination,
                    destination_prepared,
                    descriptor,
                    prepared_lower
                        .expect("unexecuted publication retains preparation")
                        .into_request(),
                ),
            );
        }
        let (observation, lower_error) = match publication.expect("publication outcome stored") {
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable {
                error,
                prepared,
            }) => (
                SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(
                    prepared.into_request(),
                ),
                error,
            ),
            Err(PreparedPersistentSdmaWindowPublicationFailureV1::Retained { error, tickets }) => (
                SameDevicePersistentSdmaWindowPublicationObservationV1::Retained(tickets),
                error,
            ),
            Ok(tickets) => (
                SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA post-publication currentness",
                ),
            ),
        };
        let closing = self.check_directional_persistent_sdma_operational_currentness();
        let transition = transition_same_device_persistent_sdma_window_publication_v1(
            SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                source,
                source_prepared,
                destination,
                destination_prepared,
                planned_tickets,
                descriptor,
            },
            observation,
            publication_operation.is_ok(),
            closing.is_ok(),
        );
        self.finish_same_device_persistent_sdma_window_publication_transition(
            publication_operation
                .err()
                .or_else(|| closing.err())
                .unwrap_or_else(|| lower_error.into()),
            transition,
        )
    }

    #[allow(clippy::result_large_err)]
    pub fn poll_same_device_persistent_sdma_window_v1(
        &mut self,
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowCopyPollV1,
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.source.attachment.queue != self.key
            || submission.destination.attachment.queue != self.key
        {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign same-device persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&submission.source.attachment)
            || !self.directional_persistent_sdma_attachment_is_current(
                &submission.destination.attachment,
            )
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission.source.attachment.pair.host_to_device_queue_id;
        if submission.source.attachment.pair != submission.destination.attachment.pair
            || submission.tickets.len() != submission.packet_count()
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.source.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut poll_result = None;
        let poll_operation = self.with_sdma_owner_memory(|owner, memory| {
            poll_result = Some(owner.poll_persistent_window(memory, &submission.tickets));
            Ok(())
        });
        let Some(poll_result) = poll_result else {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    poll_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA poll did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match poll_result {
            Ok(PersistentSdmaWindowPollV1::Pending) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Pending,
                None,
            ),
            Err(error) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(PersistentSdmaWindowPollV1::Completed(completed)) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            observation,
            poll_operation.is_ok(),
        ) {
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(submission) => Ok(
                Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Pending(submission),
            ),
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) => Ok(
                Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Completed(completed),
            ),
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(_) => {
                unreachable!("same-device poll cannot produce timeout custody")
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_same_device_persistent_sdma_window_execution_transition(
                    poll_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn wait_same_device_persistent_sdma_window_for_v1(
        &mut self,
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        timeout: Duration,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowCompletedV1,
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1,
    > {
        let pending = |error, submission| Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(submission),
        };
        if submission.source.attachment.queue != self.key
            || submission.destination.attachment.queue != self.key
        {
            return Err(pending(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "foreign same-device persistent SDMA window owner",
                ),
                submission,
            ));
        }
        if self.terminal_poisoned {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "terminal queue session requires process teardown",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(pending(error, submission));
        }
        if !self.directional_persistent_sdma_attachment_is_current(&submission.source.attachment)
            || !self.directional_persistent_sdma_attachment_is_current(
                &submission.destination.attachment,
            )
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA queue-pair attachment changed",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        }
        let expected_queue = submission.source.attachment.pair.host_to_device_queue_id;
        if submission.source.attachment.pair != submission.destination.attachment.pair
            || submission.tickets.len() != submission.packet_count()
            || submission.tickets.iter().any(|ticket| {
                !crate::sdma::ticket_matches_queue_occurrence(
                    *ticket,
                    submission.source.attachment.queue,
                    expected_queue,
                )
            })
        {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "same-device persistent SDMA window ticket identity",
                    ),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                ),
            );
        }
        let mut wait_result = None;
        let wait_operation = self.with_sdma_owner_memory(|owner, memory| {
            wait_result = Some(owner.wait_persistent_window_for(
                memory,
                &submission.tickets,
                timeout,
                SdmaWaitProfileV1::PersistentElapsedSpinFloor(PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1),
            ));
            Ok(())
        });
        let Some(wait_result) = wait_result else {
            return Err(
                self.terminal_queued_same_device_persistent_sdma_window_failure(
                    wait_operation
                        .err()
                        .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA wait did not execute",
                        )),
                    submission,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            );
        };
        let (observation, lower_error) = match wait_result {
            Err(Gfx942SdmaErrorV1::Timeout) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Timeout,
                None,
            ),
            Err(error) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::QueueRetained,
                Some(error.into()),
            ),
            Ok(completed) => (
                SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed),
                None,
            ),
        };
        match transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            observation,
            wait_operation.is_ok(),
        ) {
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(submission) => {
                Err(pending(
                    ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout),
                    submission,
                ))
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) => {
                Ok(completed)
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(_) => {
                unreachable!("same-device wait cannot produce pending custody")
            }
            SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) => Err(
                self.terminal_same_device_persistent_sdma_window_execution_transition(
                    wait_operation.err().or(lower_error).unwrap_or(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "same-device persistent SDMA completed resource identity",
                        ),
                    ),
                    custody,
                ),
            ),
        }
    }

    // Recoverable rejection returns the move-only allocation authority without
    // a fallible recovery allocation. Terminal bookkeeping failures retain it.
    #[allow(clippy::result_large_err)]
    pub fn recycle_sdma_buffer(
        &mut self,
        mut buffer: Gfx942SdmaBufferV1,
    ) -> Result<(), Gfx942SdmaBufferTransitionFailureV1> {
        self.sdma_device_pool.begin_activity();
        if !buffer.belongs_to(self.key) {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some(buffer),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error,
                recovered: None,
            });
        }
        if self.sdma_outstanding_buffers == 0 {
            self.poison_terminal();
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger underflow"),
                recovered: None,
            });
        }
        if let Some(limits) = self.sdma_device_pool.limits
            && buffer.kind() == Gfx942SdmaBufferKindV1::DeviceLocal
        {
            let decision = self
                .engine
                .as_ref()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))
                .and_then(|engine| {
                    device_pool_recycle_decision_v1(
                        &engine.backend.session,
                        self.key,
                        limits,
                        &self.sdma_pool_free,
                        &buffer,
                    )
                    .map_err(|_| {
                        ComputeAqlQueueSessionErrorV1::Contract("invalid SDMA device pool recycle")
                    })
                });
            match decision {
                Ok(DevicePoolDispositionV1::Cache) => {}
                // Cache pressure is successful disposal, not a recovered rejection.
                Ok(DevicePoolDispositionV1::Dispose) => return self.release_sdma_buffer(buffer),
                Err(error) => {
                    self.poison_terminal();
                    return Err(Gfx942SdmaBufferTransitionFailureV1 {
                        error,
                        recovered: None,
                    });
                }
            }
        }
        if let Some(limits) = self.sdma_host_pool_limits
            && buffer.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
        {
            let decision = self
                .engine
                .as_ref()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))
                .and_then(|engine| {
                    host_pool_recycle_decision_v1(
                        &engine.backend.session,
                        self.key,
                        limits,
                        &self.sdma_pool_free,
                        &buffer,
                    )
                    .map_err(|_| {
                        ComputeAqlQueueSessionErrorV1::Contract("invalid SDMA host pool recycle")
                    })
                });
            match decision {
                Ok(HostPoolDispositionV1::Cache) => {}
                Ok(HostPoolDispositionV1::Dispose) => return self.release_sdma_buffer(buffer),
                Err(error) => {
                    self.poison_terminal();
                    return Err(Gfx942SdmaBufferTransitionFailureV1 {
                        error,
                        recovered: None,
                    });
                }
            }
        }
        if self.sdma_pool_free.try_reserve(1).is_err() {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("SDMA pool allocation failed"),
                recovered: Some(buffer),
            });
        }
        if let Err(error) = buffer.advance_pool_generation() {
            self.poison_terminal();
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: error.into(),
                recovered: None,
            });
        }
        self.sdma_outstanding_buffers -= 1;
        self.sdma_pool_free.push(buffer);
        Ok(())
    }

    pub fn trim_sdma_memory_pool(&mut self) -> Result<usize, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        self.require_sdma_enabled()?;
        self.validate_configured_device_pool_v1()?;
        self.validate_configured_host_pool_v1()?;
        let mut released = 0_usize;
        while let Some(buffer) = self.sdma_pool_free.pop() {
            let result = self.with_live_queue_memory_model(|memory| {
                release_buffer(memory, buffer).map_err(Into::into)
            });
            if let Err(error) = result {
                self.poison_terminal();
                return Err(error);
            }
            released += 1;
        }
        Ok(released)
    }

    pub fn sdma_memory_pool_observation(
        &self,
    ) -> Result<Gfx942SdmaMemoryPoolObservationV1, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        let retained_free_bytes = self
            .sdma_pool_free
            .iter()
            .try_fold(0_u64, |total, buffer| {
                total.checked_add(buffer.physical_bytes()).ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("SDMA pool byte accounting overflow"),
                )
            })?;
        Ok(Gfx942SdmaMemoryPoolObservationV1 {
            checked_out_buffers: self.sdma_outstanding_buffers,
            retained_free_buffers: self.sdma_pool_free.len(),
            retained_free_bytes,
            reuse_count: self.sdma_pool_reuse_count,
        })
    }

    pub fn write_sdma_host_buffer(
        &mut self,
        buffer: &mut Gfx942SdmaBufferV1,
        offset: u64,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        self.with_live_queue_memory_model(|memory| {
            write_host_buffer(memory, buffer, offset, source).map_err(Into::into)
        })
    }

    /// Writes one exact full logical host-buffer extent.
    ///
    /// When the logical and physical extents match, the fe2o3 KFD adapter hashes
    /// `source` while copying it, seals the certificate inside `buffer`, and
    /// returns `Some(digest)`. The digest is only a scalar contracted-userspace
    /// observation, not certificate authority, kernel attestation, or
    /// loaded-kernel proof. When the physical extent includes padding, this
    /// preserves the ordinary chunked logical-write contract and returns `None`
    /// without minting a certificate. Every later CPU or device-write path
    /// invalidates a sealed certificate.
    pub fn write_full_sdma_host_buffer_authenticated_v1(
        &mut self,
        buffer: &mut Gfx942SdmaBufferV1,
        source: &[u8],
    ) -> Result<Option<[u8; 32]>, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        if exact_full_host_write_is_authenticatable(buffer, source.len())? {
            self.with_live_queue_memory_model(|memory| {
                write_full_host_buffer_authenticated(memory, buffer, source)
                    .map(Some)
                    .map_err(Into::into)
            })
        } else {
            for (index, chunk) in source
                .chunks(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
                .enumerate()
            {
                let offset = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA host write chunk offset overflow",
                    ))?;
                self.write_sdma_host_buffer(buffer, offset, chunk)?;
            }
            Ok(None)
        }
    }

    pub fn read_sdma_host_buffer(
        &mut self,
        buffer: &Gfx942SdmaBufferV1,
        offset: u64,
        byte_len: u64,
    ) -> Result<Box<[u8]>, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        self.with_live_queue_memory_model(|memory| {
            read_host_buffer(memory, buffer, offset, byte_len).map_err(Into::into)
        })
    }

    /// Reads one exact retained coherent buffer without output allocation or
    /// GPU work. The caller must establish completion before reading GPU writes.
    /// Operational reset/counter observations and the existing model loan remain
    /// mandatory; this method does not establish drain or completion authority.
    pub fn read_sdma_host_buffer_into_v1(
        &mut self,
        buffer: &Gfx942SdmaBufferV1,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), Gfx942SdmaHostReadIntoErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain);
        }
        preflight_sdma_host_read_into_v1(buffer, self.key, offset, destination.len())?;
        if self.require_sdma_enabled().is_err()
            || self.engine.is_none()
            || self.sdma_outstanding_buffers == 0
        {
            return Err(Gfx942SdmaHostReadIntoErrorV1::Unavailable);
        }
        // The immutable move-only buffer fixes queue, pool and native token
        // identity throughout the read; memory authenticates the exact record.
        let result = self.with_live_queue_memory_model(|memory| {
            crate::sdma::read_host_buffer_into_v1(memory, buffer, offset, destination)
                .map_err(Into::into)
        });
        if result.is_err() {
            self.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain);
        }
        Ok(())
    }

    /// Rebrands one fully initialized coherent SDMA buffer as dispatch data.
    ///
    /// The complete physical extent is copied to owned host bytes and hashed
    /// before the move. Pooled buffers whose logical extent is smaller than the
    /// physical allocation are rejected because dispatch would expose the full
    /// allocation. No allocation or device copy is performed.
    #[allow(clippy::result_large_err)]
    pub fn promote_sdma_host_buffer_to_fixed_dispatch_data(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (Gfx942FixedDispatchDataV1, Gfx942SdmaDispatchDataBridgeV1),
        Gfx942SdmaBufferTransitionFailureV1,
    > {
        if !buffer.belongs_to(self.key) {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some(buffer),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error,
                recovered: Some(buffer),
            });
        }
        if buffer.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
            || buffer.requested_bytes() != buffer.physical_bytes()
            || content.byte_len() != buffer.physical_bytes()
        {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "SDMA host promotion requires one exact full physical extent",
                ),
                recovered: Some(buffer),
            });
        }
        let observed = self.with_live_queue_memory_model(|memory| {
            read_host_buffer(memory, &buffer, 0, buffer.physical_bytes()).map_err(Into::into)
        });
        let observed = match observed {
            Ok(observed) => observed,
            Err(error) => {
                return Err(Gfx942SdmaBufferTransitionFailureV1 {
                    error,
                    recovered: Some(buffer),
                });
            }
        };
        if !content_descriptor_matches_bytes(content, &observed) {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "SDMA host promotion content descriptor mismatch",
                ),
                recovered: Some(buffer),
            });
        }
        if self.sdma_outstanding_buffers == 0 {
            self.poison_terminal();
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger underflow"),
                recovered: None,
            });
        }
        let physical_bytes = buffer.physical_bytes();
        let storage_identity = buffer.storage_identity();
        let (storage, owner, pool_generation, logical_bytes) = buffer.into_bridge_parts();
        let Gfx942SdmaBufferStorageV1::Host(token) = storage else {
            self.poison_terminal();
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "SDMA host promotion storage substitution",
                ),
                recovered: None,
            });
        };
        self.sdma_outstanding_buffers -= 1;
        Ok((
            Gfx942FixedDispatchDataV1::host_visible_initialized(
                Gfx942InitializedHostVisibleMemoryV1::from_completed_dispatch(token),
            ),
            Gfx942SdmaDispatchDataBridgeV1 {
                owner,
                pool_generation,
                logical_bytes,
                physical_bytes,
                storage_identity,
            },
        ))
    }

    /// Promotes the exact full device destination of one completed H2D copy.
    ///
    /// The source must be one fully initialized coherent buffer whose complete
    /// physical bytes match `content`; the destination must be one equal-sized
    /// device-local extent written from offset zero. The acquire-observed SDMA
    /// fence is the execution premise for rebranding the destination initialized.
    #[allow(clippy::result_large_err)]
    pub fn promote_completed_sdma_destination_to_fixed_dispatch_data(
        &mut self,
        completed: Gfx942SdmaCompletedCopyV1,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942PromotedSdmaDestinationV1, Gfx942SdmaCompletedPromotionFailureV1> {
        let invalid = !completed.source.belongs_to(self.key)
            || !completed.destination.belongs_to(self.key)
            || completed.source.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
            || completed.destination.kind() != Gfx942SdmaBufferKindV1::DeviceLocal
            || completed.source_offset != 0
            || completed.destination_offset != 0
            || u64::from(completed.copy_bytes) != completed.source.physical_bytes()
            || u64::from(completed.copy_bytes) != completed.destination.physical_bytes()
            || completed.source.requested_bytes() != completed.source.physical_bytes()
            || completed.destination.requested_bytes() != completed.destination.physical_bytes()
            || content.byte_len() != u64::from(completed.copy_bytes);
        if invalid {
            return Err(Gfx942SdmaCompletedPromotionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "SDMA destination promotion requires one exact full H2D completion",
                ),
                recovered: Some(completed),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaCompletedPromotionFailureV1 {
                error,
                recovered: Some(completed),
            });
        }
        let observed = self.with_live_queue_memory_model(|memory| {
            read_host_buffer(
                memory,
                &completed.source,
                0,
                completed.source.physical_bytes(),
            )
            .map_err(Into::into)
        });
        let observed = match observed {
            Ok(observed) => observed,
            Err(error) => {
                return Err(Gfx942SdmaCompletedPromotionFailureV1 {
                    error,
                    recovered: Some(completed),
                });
            }
        };
        if !content_descriptor_matches_bytes(content, &observed) {
            return Err(Gfx942SdmaCompletedPromotionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "SDMA destination promotion content descriptor mismatch",
                ),
                recovered: Some(completed),
            });
        }
        if self.sdma_outstanding_buffers < 2 {
            self.poison_terminal();
            return Err(Gfx942SdmaCompletedPromotionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger underflow"),
                recovered: None,
            });
        }
        let Gfx942SdmaCompletedCopyV1 {
            source,
            destination,
            copy_bytes: _,
            source_offset: _,
            destination_offset: _,
        } = completed;
        let physical_bytes = destination.physical_bytes();
        let storage_identity = destination.storage_identity();
        let (storage, owner, pool_generation, logical_bytes) = destination.into_bridge_parts();
        let Gfx942SdmaBufferStorageV1::Device(lease) = storage else {
            self.poison_terminal();
            return Err(Gfx942SdmaCompletedPromotionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "SDMA destination promotion storage substitution",
                ),
                recovered: None,
            });
        };
        self.sdma_outstanding_buffers -= 1;
        Ok(Gfx942PromotedSdmaDestinationV1 {
            source,
            data: Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease),
            bridge: Gfx942SdmaDispatchDataBridgeV1 {
                owner,
                pool_generation,
                logical_bytes,
                physical_bytes,
                storage_identity,
            },
        })
    }

    /// Restores one returned fixed-dispatch allocation to persistent SDMA custody.
    #[allow(clippy::result_large_err)]
    pub fn demote_fixed_dispatch_data_to_sdma_buffer(
        &mut self,
        data: Gfx942FixedDispatchDataV1,
        bridge: Gfx942SdmaDispatchDataBridgeV1,
    ) -> Result<Gfx942SdmaBufferV1, Gfx942SdmaDispatchDataDemotionFailureV1> {
        if !self.unpublished_dispatch.is_clear() {
            return Err(Gfx942SdmaDispatchDataDemotionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some((data, bridge)),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaDispatchDataDemotionFailureV1 {
                error,
                recovered: Some((data, bridge)),
            });
        }
        let layout = data.layout();
        if bridge.owner != self.key
            || data.sdma_storage_identity() != bridge.storage_identity
            || layout.requested_bytes() != bridge.physical_bytes
            || bridge.logical_bytes != bridge.physical_bytes
        {
            return Err(Gfx942SdmaDispatchDataDemotionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "fixed-dispatch to SDMA bridge substitution",
                ),
                recovered: Some((data, bridge)),
            });
        }
        let next_generation = match bridge.pool_generation.checked_add(1) {
            Some(generation) if generation != 0 => generation,
            _ => {
                self.poison_terminal();
                return Err(Gfx942SdmaDispatchDataDemotionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA bridge pool generation exhausted",
                    ),
                    recovered: None,
                });
            }
        };
        let next_outstanding = match self.sdma_outstanding_buffers.checked_add(1) {
            Some(count) => count,
            None => {
                self.poison_terminal();
                return Err(Gfx942SdmaDispatchDataDemotionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger exhausted"),
                    recovered: None,
                });
            }
        };
        let dispatch_identity = data.storage_identity();
        if self.detached_dispatch_generation.is_some() {
            let matching = self
                .detached_data_identities
                .iter()
                .position(|identity| *identity == dispatch_identity);
            let Some(index) = matching else {
                return Err(Gfx942SdmaDispatchDataDemotionFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "demoted dispatch data is absent from detached ledger",
                    ),
                    recovered: Some((data, bridge)),
                });
            };
            self.detached_data_identities.remove(index);
            self.detached_data_count =
                self.detached_data_count.checked_sub(1).ok_or_else(|| {
                    self.poison_terminal();
                    Gfx942SdmaDispatchDataDemotionFailureV1 {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "detached dispatch-data ledger underflow",
                        ),
                        recovered: None,
                    }
                })?;
            self.detached_next_insertion_index = Some(index);
        }
        let storage = data.into_sdma_storage();
        self.sdma_outstanding_buffers = next_outstanding;
        Ok(Gfx942SdmaBufferV1::from_bridge_parts(
            storage,
            bridge.owner,
            next_generation,
            bridge.logical_bytes,
        ))
    }

    // Recoverable rejection returns the move-only allocation authority before
    // native work. Terminal bookkeeping or native failures retain it.
    #[allow(clippy::result_large_err)]
    pub fn release_sdma_buffer(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
    ) -> Result<(), Gfx942SdmaBufferTransitionFailureV1> {
        self.sdma_device_pool.begin_activity();
        if !buffer.belongs_to(self.key) {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some(buffer),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error,
                recovered: None,
            });
        }
        if self.sdma_outstanding_buffers == 0 {
            self.poison_terminal();
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger underflow"),
                recovered: None,
            });
        }
        let result = self.with_live_queue_memory_model(|memory| {
            release_buffer(memory, buffer).map_err(Into::into)
        });
        match result {
            Ok(()) => {
                self.sdma_outstanding_buffers -= 1;
                Ok(())
            }
            Err(error) => {
                self.poison_terminal();
                Err(Gfx942SdmaBufferTransitionFailureV1 {
                    error,
                    recovered: None,
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    // The error returns both move-only allocation authorities without a
    // fallible recovery allocation.
    #[allow(clippy::result_large_err)]
    pub fn submit_sdma_copy(
        &mut self,
        source: Gfx942SdmaBufferV1,
        source_offset: u64,
        destination: Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942SdmaSubmissionFailureV1> {
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaSubmissionFailureV1 {
                error,
                recovered: None,
            });
        }
        let (source, destination) = preserve_ordinary_sdma_publication_custody_v1(
            self.has_any_persistent_compute_attachment_v1(),
            source,
            destination,
        )?;
        if !source.belongs_to(self.key) || !destination.belongs_to(self.key) {
            return Err(Gfx942SdmaSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some((source, destination)),
            });
        }
        if let Err(error) = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        }) {
            self.poison_terminal();
            return Err(Gfx942SdmaSubmissionFailureV1 {
                error,
                recovered: None,
            });
        }
        let preflight = self.with_sdma_owner_memory(|owner, memory| {
            owner
                .preflight_recoverable(
                    memory,
                    &source,
                    source_offset,
                    &destination,
                    destination_offset,
                    copy_bytes,
                )
                .map_err(Into::into)
        });
        if let Err(error) = preflight {
            let owner_poisoned = self
                .sdma
                .as_ref()
                .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
            let post = self.with_sdma_owner_memory(|_, memory| {
                memory
                    .check_queue_operational_currentness()
                    .map_err(Into::into)
            });
            if owner_poisoned || post.is_err() {
                self.poison_terminal();
                return Err(Gfx942SdmaSubmissionFailureV1 {
                    error: post.err().unwrap_or(error),
                    recovered: None,
                });
            }
            return Err(Gfx942SdmaSubmissionFailureV1 {
                error,
                recovered: Some((source, destination)),
            });
        }
        let result = self.with_sdma_owner_memory(|owner, memory| {
            owner
                .submit(
                    memory,
                    source,
                    source_offset,
                    destination,
                    destination_offset,
                    copy_bytes,
                )
                .map_err(Into::into)
        });
        let post = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        });
        match (result, post) {
            (Ok(ticket), Ok(())) => Ok(ticket),
            (Err(error), _) | (Ok(_), Err(error)) => {
                self.poison_terminal();
                Err(Gfx942SdmaSubmissionFailureV1 {
                    error,
                    recovered: None,
                })
            }
        }
    }

    pub fn submit_sdma_copy_batch(
        &mut self,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942SdmaBatchSubmissionFailureV1> {
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                error,
                recovered: None,
            });
        }
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.has_any_persistent_compute_attachment_v1(),
            SdmaPublicationModeV1::OrdinaryBatch,
        ) {
            return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                error: error.into(),
                recovered: Some(requests),
            });
        }
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some(requests),
            });
        }
        if let Err(error) = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        }) {
            self.poison_terminal();
            return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                error,
                recovered: None,
            });
        }
        let prepared = match self.with_sdma_owner_memory(|owner, memory| {
            Ok(owner.prepare_batch_recoverable(memory, requests))
        }) {
            Ok(Ok(prepared)) => prepared,
            Ok(Err((error, recovered))) => {
                let error = error.into();
                let owner_poisoned = self
                    .sdma
                    .as_ref()
                    .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
                let post = self.with_sdma_owner_memory(|_, memory| {
                    memory
                        .check_queue_operational_currentness()
                        .map_err(Into::into)
                });
                if owner_poisoned || post.is_err() {
                    self.poison_terminal();
                    return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                        error: post.err().unwrap_or(error),
                        recovered: None,
                    });
                }
                return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                    error,
                    recovered: Some(recovered),
                });
            }
            Err(error) => {
                self.poison_terminal();
                return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                    error,
                    recovered: None,
                });
            }
        };
        let result = self.with_sdma_owner_memory(|owner, memory| {
            owner
                .submit_prepared_batch(memory, prepared)
                .map_err(Into::into)
        });
        let post = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        });
        match (result, post) {
            (Ok(tickets), Ok(())) => Ok(tickets),
            (Err(error), _) | (Ok(_), Err(error)) => {
                self.poison_terminal();
                Err(Gfx942SdmaBatchSubmissionFailureV1 {
                    error,
                    recovered: None,
                })
            }
        }
    }

    /// Submits and completes one homogeneous batch inside one currentness envelope.
    ///
    /// This is the checked low-latency path: one operational-currentness check
    /// precedes every mapped read/write and packet publication, and one follows
    /// observed completion. A timeout returns the still-valid tickets after the
    /// closing check so the caller can continue waiting.
    pub fn execute_sdma_copy_batch_for(
        &mut self,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, Gfx942SdmaBatchExecutionFailureV1> {
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBatchExecutionFailureV1 {
                error,
                recovery: None,
            });
        }
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.has_any_persistent_compute_attachment_v1(),
            SdmaPublicationModeV1::ExecuteBatch,
        ) {
            return Err(Gfx942SdmaBatchExecutionFailureV1 {
                error: error.into(),
                recovery: Some(Gfx942SdmaBatchExecutionRecoveryV1::Requests(requests)),
            });
        }
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaBatchExecutionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovery: Some(Gfx942SdmaBatchExecutionRecoveryV1::Requests(requests)),
            });
        }
        if let Err(error) = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        }) {
            self.poison_terminal();
            return Err(Gfx942SdmaBatchExecutionFailureV1 {
                error,
                recovery: None,
            });
        }
        let prepared = match self.with_sdma_owner_memory(|owner, memory| {
            Ok(owner.prepare_batch_recoverable(memory, requests))
        }) {
            Ok(Ok(prepared)) => prepared,
            Ok(Err((error, recovered))) => {
                let error = error.into();
                let owner_poisoned = self
                    .sdma
                    .as_ref()
                    .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
                let post = self.with_sdma_owner_memory(|_, memory| {
                    memory
                        .check_queue_operational_currentness()
                        .map_err(Into::into)
                });
                if owner_poisoned || post.is_err() {
                    self.poison_terminal();
                    return Err(Gfx942SdmaBatchExecutionFailureV1 {
                        error: post.err().unwrap_or(error),
                        recovery: None,
                    });
                }
                return Err(Gfx942SdmaBatchExecutionFailureV1 {
                    error,
                    recovery: Some(Gfx942SdmaBatchExecutionRecoveryV1::Requests(recovered)),
                });
            }
            Err(error) => {
                self.poison_terminal();
                return Err(Gfx942SdmaBatchExecutionFailureV1 {
                    error,
                    recovery: None,
                });
            }
        };
        let tickets = match self.with_sdma_owner_memory(|owner, memory| {
            owner
                .submit_prepared_batch(memory, prepared)
                .map_err(Into::into)
        }) {
            Ok(tickets) => tickets,
            Err(error) => {
                let post = self.with_sdma_owner_memory(|_, memory| {
                    memory
                        .check_queue_operational_currentness()
                        .map_err(Into::into)
                });
                self.poison_terminal();
                return Err(Gfx942SdmaBatchExecutionFailureV1 {
                    error: post.err().unwrap_or(error),
                    recovery: None,
                });
            }
        };
        let result = self.with_sdma_owner_memory(|owner, memory| {
            owner
                .wait_many_for_in_current_scope(memory, &tickets, timeout)
                .map_err(Into::into)
        });
        let post = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        });
        match classify_sdma_batch_execution_finish(result.as_ref().err(), post.is_ok()) {
            Gfx942SdmaBatchExecutionFinishV1::Success => match result {
                Ok(completed) => Ok(completed),
                Err(_) => unreachable!("success classification requires a successful wait"),
            },
            Gfx942SdmaBatchExecutionFinishV1::RecoverableTimeout => {
                let Err(error) = result else {
                    unreachable!("timeout classification requires a timeout error")
                };
                Err(Gfx942SdmaBatchExecutionFailureV1 {
                    error,
                    recovery: Some(Gfx942SdmaBatchExecutionRecoveryV1::PendingTickets(tickets)),
                })
            }
            Gfx942SdmaBatchExecutionFinishV1::Terminal => {
                self.poison_terminal();
                let error = match post {
                    Err(error) => error,
                    Ok(()) => match result {
                        Err(error) => error,
                        Ok(_) => unreachable!("terminal classification requires a failure"),
                    },
                };
                Err(Gfx942SdmaBatchExecutionFailureV1 {
                    error,
                    recovery: None,
                })
            }
        }
    }

    pub fn poll_sdma_copy(
        &mut self,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<Gfx942SdmaCopyPollV1, ComputeAqlQueueSessionErrorV1> {
        let result = self
            .with_sdma_owner_memory(|owner, memory| owner.poll(memory, ticket).map_err(Into::into));
        if result.is_err() {
            self.poison_terminal();
        }
        result
    }

    /// Observes one queue's counters and ticket completions without consuming them.
    ///
    /// The timestamp is host-monotonic only and is not calibrated to a GPU clock.
    pub fn observe_sdma_copy_progress(
        &mut self,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<Gfx942SdmaQueueProgressObservationV1, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        self.check_currentness()?;
        let result = self.with_sdma_owner_memory(|owner, memory| {
            owner.observe_progress(memory, tickets).map_err(Into::into)
        });
        let post = self.check_currentness();
        match (result, post) {
            (Ok(observation), Ok(())) => Ok(observation),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Validates a published ticket and rejects cancellation without native mutation.
    ///
    /// KFD exposes no admitted operation that retracts one already-published SDMA
    /// packet. The returned ticket remains live and must be polled or drained.
    #[allow(clippy::result_large_err)]
    pub fn try_cancel_sdma_copy(
        &mut self,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<(), (ComputeAqlQueueSessionErrorV1, Gfx942SdmaCopyTicketV1)> {
        if let Err(error) = self.require_sdma_enabled() {
            return Err((error, ticket));
        }
        let validation = self.with_sdma_owner_memory(|owner, _| {
            owner.validate_published_ticket(ticket).map_err(Into::into)
        });
        match validation {
            Ok(()) => Err((
                ComputeAqlQueueSessionErrorV1::Sdma(
                    Gfx942SdmaErrorV1::PublishedCancellationUnsupported,
                ),
                ticket,
            )),
            Err(error) => Err((error, ticket)),
        }
    }

    pub fn wait_sdma_copy_for(
        &mut self,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
    ) -> Result<Gfx942SdmaCompletedCopyV1, ComputeAqlQueueSessionErrorV1> {
        let result = self.with_sdma_owner_memory(|owner, memory| {
            owner
                .wait_for(memory, ticket, timeout, SdmaWaitProfileV1::Default)
                .map_err(Into::into)
        });
        if result.as_ref().is_err_and(|error| {
            !matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout)
            )
        }) {
            self.poison_terminal();
        }
        result
    }

    pub fn wait_sdma_copy_batch_for(
        &mut self,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, ComputeAqlQueueSessionErrorV1> {
        let result = self.with_sdma_owner_memory(|owner, memory| {
            owner
                .wait_many_for(memory, tickets, timeout)
                .map_err(Into::into)
        });
        if result.as_ref().is_err_and(|error| {
            !matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout)
            )
        }) {
            self.poison_terminal();
        }
        result
    }

    /// Explicit drain spelling for a published ticket roster.
    pub fn drain_sdma_copy_batch_for(
        &mut self,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, ComputeAqlQueueSessionErrorV1> {
        self.wait_sdma_copy_batch_for(tickets, timeout)
    }

    /// Exact process-local queue occurrence for private debugger correlation.
    pub(crate) fn target_debug_queue_occurrence_v2(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3-kfd-target-debug-queue-occurrence-v2\0");
        digest.update(self.key.vm.device.physical.0.to_le_bytes());
        digest.update(self.key.vm.device.generation.0.to_le_bytes());
        digest.update(self.key.vm.id.0.to_le_bytes());
        digest.update(self.key.id.0.to_le_bytes());
        digest.update(self.key.generation.0.to_le_bytes());
        digest.update(self.observation.queue_id.to_le_bytes());
        digest.update(self.observation.ring_bytes.to_le_bytes());
        digest.update(self.observation.event_id.to_le_bytes());
        digest.update([self.observation.cwsr_shadow_pages]);
        digest.finalize().into()
    }

    /// Samples correlated KFD clock domains while this exact queue remains
    /// operational. The observation is bracketed by live runtime/event checks.
    /// It is a host publication/completion calibration input, not a GPU kernel
    /// start or end timestamp.
    pub fn observe_clock_correlation(
        &mut self,
    ) -> Result<crate::KfdClockCorrelationObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result = (|| {
            let engine = self
                .engine
                .as_mut()
                .ok_or(Gfx942CompletionErrorV1::Currentness)?;
            if engine.authority_poisoned
                || engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active)
            {
                return Err(Gfx942CompletionErrorV1::Currentness);
            }
            let exception = self
                .exception
                .as_ref()
                .ok_or(Gfx942CompletionErrorV1::Currentness)?;
            let validate_queue = |session: &SharedGttMemorySessionV1| {
                exception
                    .runtime
                    .validate_queue_live_process(session.opener_pid())
                    .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
                exception
                    .event
                    .validate_live_with_shadows(
                        session.kfd_fd(),
                        session.opener_pid(),
                        &exception.shadows,
                    )
                    .map_err(|_| Gfx942CompletionErrorV1::Currentness)
            };
            validate_queue(&engine.backend.session)?;
            let observation = engine
                .backend
                .session
                .observe_queue_clock_correlation()
                .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
            validate_queue(&engine.backend.session)?;
            Ok(observation)
        })();
        if result.is_err() {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Private bridge for the later dispatch composition. The public queue API
    /// cannot submit packets or access counters, slots, addresses, or MMIO.
    #[allow(dead_code)]
    pub(crate) fn submit_prepared(
        &mut self,
        packet: AqlPreparedKernelDispatchV1,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.submit_prepared_batch(AqlPreparedKernelDispatchBatchV2::one(packet))
    }

    /// Private arithmetic/publication bridge only. The prepared values carry
    /// no code, kernarg, allocation, dispatch-generation, or completion
    /// authority, so this is deliberately not a launch API.
    #[allow(dead_code)]
    pub(crate) fn submit_prepared_batch<const N: usize>(
        &mut self,
        batch: AqlPreparedKernelDispatchBatchV2<N>,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.submit_prepared_batch_classified(batch)
            .map_err(NativeAqlSubmissionFailureV1::into_error)
    }

    fn submit_prepared_batch_classified<const N: usize>(
        &mut self,
        batch: AqlPreparedKernelDispatchBatchV2<N>,
    ) -> Result<u64, NativeAqlSubmissionFailureV1> {
        let result = (|| {
            if self.terminal_poisoned {
                return Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Poisoned,
                ));
            }
            let exception = self.exception.as_ref().ok_or({
                NativeAqlSubmissionFailureV1::Terminal(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing queue exception gate",
                ))
            })?;
            let owner = self.submission.as_mut().ok_or({
                NativeAqlSubmissionFailureV1::Terminal(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing submission owner",
                ))
            })?;
            let engine = self.engine.as_mut().ok_or({
                NativeAqlSubmissionFailureV1::Terminal(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing queue engine",
                ))
            })?;
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::InvalidQueue("queue is not active"),
                ));
            }
            let (backend, resources) = (&mut engine.backend, &mut engine.resources);
            let resource = resources
                .iter_mut()
                .find(|resource| resource.key == self.key)
                .ok_or({
                    NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::InvalidQueue("missing queue resources"),
                    )
                })?;
            let authority = resource.authority.as_mut().ok_or({
                NativeAqlSubmissionFailureV1::Terminal(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "released queue resources",
                ))
            })?;
            let doorbell = self.doorbell.as_mut().ok_or({
                NativeAqlSubmissionFailureV1::Terminal(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "missing doorbell",
                ))
            })?;
            let mut native = LinuxAqlSubmissionBackendV1 {
                memory: &mut backend.session,
                ring: &mut authority.ring,
                control: &mut authority.control,
                doorbell,
                exception,
            };
            owner.submit_batch_classified(batch, &mut native)
        })();
        if matches!(&result, Err(NativeAqlSubmissionFailureV1::Terminal(_))) {
            self.poison_terminal();
        }
        result
    }

    fn submit_prepared_barrier(
        &mut self,
        packet: AqlPreparedBarrierAndV1,
    ) -> Result<u64, NativeBarrierAndSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Poisoned,
            ));
        }
        let exception = self
            .exception
            .as_ref()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing queue exception gate",
            ))
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        let owner = self
            .submission
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing submission owner",
            ))
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        let engine = self
            .engine
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing queue engine",
            ))
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::InvalidQueue("queue is not active"),
            ));
        }
        let (backend, resources) = (&mut engine.backend, &mut engine.resources);
        let resource = resources
            .iter_mut()
            .find(|resource| resource.key == self.key)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing queue resources",
            ))
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        let authority = resource
            .authority
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "released queue resources",
            ))
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        let doorbell = self
            .doorbell
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue("missing doorbell"))
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        let mut native = LinuxAqlSubmissionBackendV1 {
            memory: &mut backend.session,
            ring: &mut authority.ring,
            control: &mut authority.control,
            doorbell,
            exception,
        };
        let result = owner.submit_barrier_and(packet, &mut native);
        if matches!(
            &result,
            Err(NativeBarrierAndSubmissionFailureV1::Terminal(_))
        ) {
            self.terminal_poisoned = true;
        }
        result
    }

    /// Publishes one isolated zero-dependency BARRIER_AND queue probe.
    ///
    /// The probe leases one existing completion slot and binds only the exact
    /// queue and signal generations. It does not mint or retain code, kernarg,
    /// or dispatch-generation evidence.
    pub(crate) fn submit_barrier_probe(
        &mut self,
    ) -> Result<Gfx942BarrierProbeV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let bound = self.completion_owner.bind_barrier_probe()?;
        let (packet, retention) = bound.into_parts();
        match self.submit_prepared_barrier(packet) {
            Ok(packet_id) => match self
                .completion_owner
                .mark_barrier_probe_published(retention, packet_id)
            {
                Ok(probe) => Ok(probe),
                Err(error) => {
                    self.poison_terminal();
                    Err(error.into())
                }
            },
            Err(NativeBarrierAndSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                if let Err(cancel_error) =
                    self.completion_owner.cancel_bound_barrier_probe(retention)
                {
                    self.poison_terminal();
                    return Err(cancel_error.into());
                }
                Err(map_submission(error))
            }
            Err(NativeBarrierAndSubmissionFailureV1::Terminal(error)) => {
                self.poison_terminal();
                Err(map_submission(error))
            }
        }
    }

    /// Waits for one barrier completion with an exact bounded poll count.
    pub(crate) fn wait_barrier_probe(
        &mut self,
        probe: Gfx942BarrierProbeV1,
        polls: u32,
    ) -> Result<Gfx942CompletedBarrierProbeV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result =
            (|| -> Result<Gfx942CompletedBarrierProbeV1, Gfx942BarrierProbeWaitFailureV1> {
                let owner = &mut self.completion_owner;
                let engine =
                    self.engine
                        .as_mut()
                        .ok_or(Gfx942BarrierProbeWaitFailureV1::Terminal(
                            Gfx942CompletionErrorV1::Observation,
                        ))?;
                if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                    return Err(Gfx942BarrierProbeWaitFailureV1::Terminal(
                        Gfx942CompletionErrorV1::Observation,
                    ));
                }
                let signals = self.completion_signals.as_mut().ok_or(
                    Gfx942BarrierProbeWaitFailureV1::Terminal(Gfx942CompletionErrorV1::Observation),
                )?;
                let exception =
                    self.exception
                        .as_ref()
                        .ok_or(Gfx942BarrierProbeWaitFailureV1::Terminal(
                            Gfx942CompletionErrorV1::Observation,
                        ))?;
                let mut backend = LinuxCompletionSignalBackendV1 {
                    memory: &mut engine.backend.session,
                    signals,
                    exception,
                };
                owner.wait_barrier_probe_bounded(probe, polls, &mut backend)
            })();
        match result {
            Ok(completed) => Ok(completed),
            Err(Gfx942BarrierProbeWaitFailureV1::Terminal(error)) => {
                self.poison_terminal();
                Err(error.into())
            }
            Err(Gfx942BarrierProbeWaitFailureV1::Timeout { probe, polls }) => {
                let observation = observe_then_poison(
                    self,
                    |session| session.observe_barrier_probe_timeout(&probe),
                    Self::poison_terminal,
                );
                match observation {
                    Ok(observation) => Err(Gfx942CompletionErrorV1::Timeout {
                        polls,
                        observation: Box::new(observation),
                    }
                    .into()),
                    Err(error) => Err(error.into()),
                }
            }
        }
    }

    /// Resets the completed barrier signal and returns the queue to ready state.
    pub(crate) fn recycle_barrier_probe(
        &mut self,
        completed: Gfx942CompletedBarrierProbeV1,
    ) -> Result<Gfx942BarrierProbeRecycleObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result =
            (|| -> Result<Gfx942BarrierProbeRecycleObservationV1, Gfx942CompletionErrorV1> {
                let owner = &mut self.completion_owner;
                let engine = self
                    .engine
                    .as_mut()
                    .ok_or(Gfx942CompletionErrorV1::Observation)?;
                let signals = self
                    .completion_signals
                    .as_mut()
                    .ok_or(Gfx942CompletionErrorV1::Observation)?;
                let exception = self
                    .exception
                    .as_ref()
                    .ok_or(Gfx942CompletionErrorV1::Observation)?;
                let mut backend = LinuxCompletionSignalBackendV1 {
                    memory: &mut engine.backend.session,
                    signals,
                    exception,
                };
                owner.recycle_barrier_probe(completed, &mut backend)
            })();
        if result.is_err() {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Private dispatch-composition boundary. Each template is bound to one
    /// unique retained completion signal before the existing all-body/then-
    /// all-header batch publication. This remains unreachable from safe public
    /// API because code, kernarg, and data-allocation authorities are not yet
    /// available to mint the generation bindings.
    #[allow(dead_code)]
    pub(crate) fn submit_with_completions<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
    ) -> Result<Gfx942CompletionBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.submit_with_completions_classified(templates)
            .map_err(FixedDispatchSubmissionFailureV1::into_error)
    }

    fn submit_with_completions_classified<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
    ) -> Result<Gfx942CompletionBatchV1<N>, FixedDispatchSubmissionFailureV1> {
        self.submit_with_completions_classified_using(templates, |session, packets| {
            session.submit_prepared_batch_classified(packets)
        })
    }

    fn submit_with_completions_classified_using<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
        submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<N>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942CompletionBatchV1<N>, FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942CompletionErrorV1::Poisoned.into(),
            ));
        }
        let bound = match self.completion_owner.bind_batch(templates) {
            Ok(bound) => bound,
            Err(Gfx942CompletionErrorV1::InsufficientSignals) => {
                return Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                    Gfx942CompletionErrorV1::InsufficientSignals.into(),
                ));
            }
            Err(error) => {
                self.poison_terminal();
                return Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()));
            }
        };
        let (packets, retention) = bound.into_parts();
        if let Err(error) = self.completion_owner.validate_bound(&retention) {
            self.poison_terminal();
            return Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()));
        }
        match submit(self, packets) {
            Ok(last_packet_id) => {
                match self
                    .completion_owner
                    .mark_published(retention, last_packet_id)
                {
                    Ok(batch) => Ok(batch),
                    Err(error) => {
                        self.poison_terminal();
                        Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()))
                    }
                }
            }
            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                if let Err(cancel_error) = self.completion_owner.cancel_bound(retention) {
                    self.poison_terminal();
                    return Err(FixedDispatchSubmissionFailureV1::Terminal(
                        cancel_error.into(),
                    ));
                }
                Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                    map_submission(error),
                ))
            }
            Err(NativeAqlSubmissionFailureV1::Terminal(error)) => {
                self.poison_terminal();
                Err(FixedDispatchSubmissionFailureV1::Terminal(map_submission(
                    error,
                )))
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn poll_completion_batch<const N: usize>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
    ) -> Result<Gfx942CompletionPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        match self.poll_completion_batch_with_progress(batch)? {
            Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                Ok(Gfx942CompletionPollV1::Pending(batch))
            }
            Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                Ok(Gfx942CompletionPollV1::Ready(completed))
            }
        }
    }

    fn poll_completion_batch_with_progress<const N: usize>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
    ) -> Result<Gfx942CompletionPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result =
            {
                let owner = &mut self.completion_owner;
                let engine =
                    self.engine
                        .as_mut()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue engine",
                        ))?;
                if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "queue is not active",
                    ));
                }
                let signals = self.completion_signals.as_mut().ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                )?;
                let exception =
                    self.exception
                        .as_ref()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue exception gate",
                        ))?;
                let mut backend = LinuxCompletionSignalBackendV1 {
                    memory: &mut engine.backend.session,
                    signals,
                    exception,
                };
                owner.observe_once_with_progress(batch, &mut backend)
            };
        if result.is_err() {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    #[allow(clippy::result_large_err)]
    fn poll_completion_batch_with_progress_retaining<const N: usize>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
    ) -> Result<
        Gfx942CompletionPollWithProgressV1<N>,
        (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<N>),
    > {
        if self.terminal_poisoned {
            return Err((Gfx942CompletionErrorV1::Poisoned.into(), batch));
        }
        let result = {
            let owner = &mut self.completion_owner;
            let Some(engine) = self.engine.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue engine"),
                    batch,
                ));
            };
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("queue is not active"),
                    batch,
                ));
            }
            let Some(signals) = self.completion_signals.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                    batch,
                ));
            };
            let Some(exception) = self.exception.as_ref() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue exception gate"),
                    batch,
                ));
            };
            let mut backend = LinuxCompletionSignalBackendV1 {
                memory: &mut engine.backend.session,
                signals,
                exception,
            };
            owner
                .observe_once_with_progress_retaining(batch, &mut backend)
                .map_err(|(error, batch)| (error.into(), batch))
        };
        if result.is_err() {
            self.poison_terminal();
        }
        result
    }

    #[allow(clippy::result_large_err)]
    fn poll_completion_batch_with_current_handoff_retaining(
        &mut self,
        batch: Gfx942CompletionBatchV1<1>,
    ) -> Result<
        CompletionPollWithCurrentnessHandoffV1<1>,
        (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<1>),
    > {
        if self.terminal_poisoned {
            return Err((Gfx942CompletionErrorV1::Poisoned.into(), batch));
        }
        let result = {
            let owner = &mut self.completion_owner;
            let Some(engine) = self.engine.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue engine"),
                    batch,
                ));
            };
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("queue is not active"),
                    batch,
                ));
            }
            let Some(signals) = self.completion_signals.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                    batch,
                ));
            };
            let Some(exception) = self.exception.as_ref() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue exception gate"),
                    batch,
                ));
            };
            let mut backend = LinuxCompletionSignalBackendV1 {
                memory: &mut engine.backend.session,
                signals,
                exception,
            };
            owner
                .observe_one_with_progress_current_handoff_retaining(batch, &mut backend)
                .map_err(|(error, batch)| (error.into(), batch))
        };
        if result.as_ref().is_err_and(|(error, _)| {
            !matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Completion(
                    Gfx942CompletionErrorV1::SignalPinned { .. }
                )
            )
        }) {
            self.poison_terminal();
        }
        result
    }

    fn check_timeout_observation_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Currentness);
        }
        let engine = self
            .engine
            .as_mut()
            .ok_or(Gfx942CompletionErrorV1::Currentness)?;
        if engine.authority_poisoned
            || engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active)
        {
            return Err(Gfx942CompletionErrorV1::Currentness);
        }
        engine
            .backend
            .session
            .check_queue_currentness()
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
        let exception = self
            .exception
            .as_ref()
            .ok_or(Gfx942CompletionErrorV1::Currentness)?;
        exception
            .runtime
            .validate_queue_live_process(engine.backend.session.opener_pid())
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
        exception
            .event
            .validate_live_with_shadows_for_diagnostic(
                engine.backend.session.kfd_fd(),
                engine.backend.session.opener_pid(),
                &exception.shadows,
            )
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)
    }

    fn observe_completion_timeout<const N: usize>(
        &mut self,
        batch: &Gfx942CompletionBatchV1<N>,
    ) -> Result<Gfx942TimeoutExecutionObservationV1, Gfx942CompletionErrorV1> {
        let (first_packet_id, first_signal_slot) = batch.first_packet_and_signal_slot()?;
        let packet_count = u16::try_from(N).map_err(|_| Gfx942CompletionErrorV1::Observation)?;
        self.observe_timeout_packet_and_signal(packet_count, first_packet_id, first_signal_slot)
    }

    fn observe_barrier_probe_timeout(
        &mut self,
        probe: &Gfx942BarrierProbeV1,
    ) -> Result<Gfx942TimeoutExecutionObservationV1, Gfx942CompletionErrorV1> {
        let (packet_id, signal_slot) = probe.packet_and_signal_slot()?;
        self.observe_timeout_packet_and_signal(1, packet_id, signal_slot)
    }

    fn observe_completed_barrier_probe(
        &mut self,
        completed: &Gfx942CompletedBarrierProbeV1,
    ) -> Result<Gfx942TimeoutExecutionObservationV1, Gfx942CompletionErrorV1> {
        let (packet_id, signal_slot) = completed.packet_and_signal_slot()?;
        match self.observe_timeout_packet_and_signal(1, packet_id, signal_slot) {
            Ok(observation) => match validate_barrier_probe_success_snapshot(observation) {
                Ok(observation) => Ok(observation),
                Err(error) => {
                    self.poison_terminal();
                    Err(error)
                }
            },
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    fn observe_timeout_packet_and_signal(
        &mut self,
        packet_count: u16,
        first_packet_id: u64,
        first_signal_slot: u32,
    ) -> Result<Gfx942TimeoutExecutionObservationV1, Gfx942CompletionErrorV1> {
        self.check_timeout_observation_currentness()?;
        let (
            (write_counter, read_counter),
            (_, first_packet_header, first_packet_setup),
            (kind, value),
            reason,
        ) = {
            let engine = self
                .engine
                .as_mut()
                .ok_or(Gfx942CompletionErrorV1::Observation)?;
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err(Gfx942CompletionErrorV1::Observation);
            }
            let resource = engine
                .resources
                .iter_mut()
                .find(|resource| resource.key == self.key)
                .ok_or(Gfx942CompletionErrorV1::Observation)?;
            let authority = resource
                .authority
                .as_mut()
                .ok_or(Gfx942CompletionErrorV1::Observation)?;
            let memory = &mut engine.backend.session;
            let counters = memory
                .observe_aql_control_counters(&mut authority.control)
                .map_err(map_timeout_memory_observation_error)?;
            let packet = authority
                .ring
                .observe_packet_header(memory, first_packet_id)
                .map_err(map_timeout_memory_observation_error)?;
            let signals = self
                .completion_signals
                .as_mut()
                .ok_or(Gfx942CompletionErrorV1::Observation)?;
            let signal = memory
                .observe_aql_completion_signal_state(signals, first_signal_slot)
                .map_err(map_timeout_memory_observation_error)?;
            let reason = self
                .exception
                .as_ref()
                .ok_or(Gfx942CompletionErrorV1::Observation)?
                .shadows
                .observe_reason()
                .map_err(|_| Gfx942CompletionErrorV1::Observation)?;
            (counters, packet, signal, reason)
        };
        self.check_timeout_observation_currentness()?;
        let first_signal = match classify_acquired_completion_value_v1(value) {
            AqlCompletionObservationV1::Pending => Gfx942TimeoutSignalObservationV1::Pending,
            AqlCompletionObservationV1::Completed => Gfx942TimeoutSignalObservationV1::Completed,
            AqlCompletionObservationV1::Unexpected(value) => {
                Gfx942TimeoutSignalObservationV1::Fault(value)
            }
        };
        Ok(Gfx942TimeoutExecutionObservationV1::new(
            packet_count,
            write_counter,
            read_counter,
            first_packet_header,
            first_packet_setup,
            kind,
            first_signal,
            reason.get(),
        ))
    }

    #[allow(dead_code)]
    pub(crate) fn wait_completion_batch<const N: usize>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        polls: u32,
    ) -> Result<Gfx942CompletedBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result =
            {
                let owner = &mut self.completion_owner;
                let engine =
                    self.engine
                        .as_mut()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue engine",
                        ))?;
                if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "queue is not active",
                    ));
                }
                let signals = self.completion_signals.as_mut().ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                )?;
                let exception =
                    self.exception
                        .as_ref()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue exception gate",
                        ))?;
                let mut backend = LinuxCompletionSignalBackendV1 {
                    memory: &mut engine.backend.session,
                    signals,
                    exception,
                };
                owner.wait_bounded(batch, polls, &mut backend)
            };
        match result {
            Ok(completed) => Ok(completed),
            Err(Gfx942CompletionWaitFailureV1::Terminal(error)) => {
                self.poison_terminal();
                Err(error.into())
            }
            Err(Gfx942CompletionWaitFailureV1::Timeout { batch, polls }) => {
                let observation = observe_then_poison(
                    self,
                    |session| session.observe_completion_timeout(&batch),
                    Self::poison_terminal,
                );
                match observation {
                    Ok(observation) => Err(Gfx942CompletionErrorV1::Timeout {
                        polls,
                        observation: Box::new(observation),
                    }
                    .into()),
                    Err(error) => Err(error.into()),
                }
            }
        }
    }

    fn wait_completion_batch_until<const N: usize>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        deadline: Instant,
    ) -> Result<Gfx942CompletedBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result =
            {
                let owner = &mut self.completion_owner;
                let engine =
                    self.engine
                        .as_mut()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue engine",
                        ))?;
                if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "queue is not active",
                    ));
                }
                let signals = self.completion_signals.as_mut().ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                )?;
                let exception =
                    self.exception
                        .as_ref()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue exception gate",
                        ))?;
                let mut backend = LinuxCompletionSignalBackendV1 {
                    memory: &mut engine.backend.session,
                    signals,
                    exception,
                };
                owner.wait_until(batch, deadline, &mut backend)
            };
        match result {
            Ok(completed) => Ok(completed),
            Err(Gfx942CompletionWaitFailureV1::Terminal(error)) => {
                self.poison_terminal();
                Err(error.into())
            }
            Err(Gfx942CompletionWaitFailureV1::Timeout { batch, polls }) => {
                let observation = observe_then_poison(
                    self,
                    |session| session.observe_completion_timeout(&batch),
                    Self::poison_terminal,
                );
                match observation {
                    Ok(observation) => Err(Gfx942CompletionErrorV1::Timeout {
                        polls,
                        observation: Box::new(observation),
                    }
                    .into()),
                    Err(error) => Err(error.into()),
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn recycle_completion_batch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let result =
            {
                let owner = &mut self.completion_owner;
                let engine =
                    self.engine
                        .as_mut()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue engine",
                        ))?;
                if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "queue is not active",
                    ));
                }
                let signals = self.completion_signals.as_mut().ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                )?;
                let exception =
                    self.exception
                        .as_ref()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing queue exception gate",
                        ))?;
                let mut backend = LinuxCompletionSignalBackendV1 {
                    memory: &mut engine.backend.session,
                    signals,
                    exception,
                };
                owner.recycle(completed, &mut backend)
            };
        if result.is_err() {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    #[allow(clippy::result_large_err)]
    fn recycle_completion_batch_retaining<const N: usize>(
        &mut self,
        completed: Gfx942CompletedBatchV1<N>,
    ) -> Result<
        Gfx942CompletionRecycleObservationV1,
        (ComputeAqlQueueSessionErrorV1, Gfx942CompletedBatchV1<N>),
    > {
        if self.terminal_poisoned {
            return Err((Gfx942CompletionErrorV1::Poisoned.into(), completed));
        }
        let result = {
            let owner = &mut self.completion_owner;
            let Some(engine) = self.engine.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue engine"),
                    completed,
                ));
            };
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("queue is not active"),
                    completed,
                ));
            }
            let Some(signals) = self.completion_signals.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                    completed,
                ));
            };
            let Some(exception) = self.exception.as_ref() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue exception gate"),
                    completed,
                ));
            };
            let mut backend = LinuxCompletionSignalBackendV1 {
                memory: &mut engine.backend.session,
                signals,
                exception,
            };
            owner
                .recycle_retaining(completed, &mut backend)
                .map_err(|(error, completed)| (error.into(), completed))
        };
        if result.as_ref().is_err_and(|(error, _)| {
            !matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Completion(
                    Gfx942CompletionErrorV1::SignalPinned { .. }
                )
            )
        }) {
            self.poison_terminal();
        }
        result
    }

    #[allow(clippy::result_large_err)]
    fn recycle_completion_current_handoff_retaining<const N: usize>(
        &mut self,
        handoff: CompletionCurrentnessHandoffV1<N>,
    ) -> Result<
        Gfx942CompletionRecycleObservationV1,
        (
            ComputeAqlQueueSessionErrorV1,
            CompletionCurrentnessHandoffV1<N>,
        ),
    > {
        if self.terminal_poisoned {
            return Err((Gfx942CompletionErrorV1::Poisoned.into(), handoff));
        }
        let result = {
            let owner = &mut self.completion_owner;
            let Some(engine) = self.engine.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue engine"),
                    handoff,
                ));
            };
            if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("queue is not active"),
                    handoff,
                ));
            }
            let Some(signals) = self.completion_signals.as_mut() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                    handoff,
                ));
            };
            let Some(exception) = self.exception.as_ref() else {
                return Err((
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue exception gate"),
                    handoff,
                ));
            };
            let mut backend = LinuxCompletionSignalBackendV1 {
                memory: &mut engine.backend.session,
                signals,
                exception,
            };
            owner
                .recycle_current_handoff_retaining(handoff, &mut backend)
                .map_err(|(error, handoff)| (error.into(), handoff))
        };
        if result.is_err() {
            self.poison_terminal();
        }
        result
    }

    fn poison_terminal(&mut self) {
        self.terminal_poisoned = true;
        self.unpublished_dispatch.continuation = None;
        if let Some(owner) = self.dependency_owner.0.as_mut() {
            owner.poison();
        }
        if let Some(owner) = self.completion_owner.0.as_mut() {
            owner.poison_owner();
        }
        if let Some(dispatch) = self.dispatch.as_mut() {
            dispatch.poison();
        }
        if let Some(submission) = self.submission.as_mut() {
            submission.poison();
        }
    }

    #[cfg(feature = "live-validation")]
    pub fn verify_doorbell_dontfork(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.check_currentness()?;
        self.doorbell
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing doorbell"))?
            .verify_dontfork_child_negative()?;
        self.check_currentness()
    }

    #[cfg(feature = "live-validation")]
    pub fn verify_exception_shadows_dontfork(
        &mut self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.check_currentness()?;
        self.exception
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue exception state",
            ))?
            .shadows
            .verify_dontfork_child_negative()?;
        self.check_currentness()
    }

    #[allow(dead_code)]
    fn observe_queue_exception(
        &mut self,
        timeout_ms: u32,
    ) -> Result<QueueExceptionWaitObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "queue session terminally poisoned",
            ));
        }
        if let Err(error) = self.check_currentness() {
            self.poison_terminal();
            return Err(error);
        }
        if self.engine.is_none() || self.exception.is_none() {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue exception composition",
            ));
        }
        let result = {
            let engine = self.engine.as_mut().expect("checked queue engine");
            let exception = self.exception.as_mut().expect("checked exception state");
            exception.event.wait_and_observe(
                engine.backend.session.kfd_fd(),
                engine.backend.session.opener_pid(),
                &exception.shadows,
                timeout_ms,
            )
        };
        // A timeout/payload pair is a racy snapshot, not an absence proof. Any
        // observation attempt is terminal and forbids later publish/cleanup.
        self.poison_terminal();
        let observation = result?;
        self.check_currentness()?;
        Ok(observation)
    }

    pub fn destroy(self) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        match self.destroy_inner(QueueDestroyModeV1::Release)? {
            QueueDestroyOutcomeV1::Released(destroyed) => Ok(destroyed),
            QueueDestroyOutcomeV1::Returned(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "ordinary destroy returned dispatch resources",
            )),
        }
    }

    /// Destroys a queue and returns its actual mapped C3 authorities while the
    /// bound dispatch is prepared and no generation is in flight.
    ///
    /// A zero returned generation proves the batch was never published. A
    /// nonzero generation proves the latest publication reached exact C4
    /// completion and signal recycle. This grants no initialized-content or
    /// read authority.
    pub fn destroy_returning_fixed_dispatch_resources(
        self,
    ) -> Result<Gfx942RecycledDispatchResourcesV1, ComputeAqlQueueSessionErrorV1> {
        match self.destroy_inner(QueueDestroyModeV1::ReturnAttached)? {
            QueueDestroyOutcomeV1::Returned(resources) => Ok(*resources),
            QueueDestroyOutcomeV1::Released(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "returning destroy released dispatch resources",
            )),
        }
    }

    /// Destroys an unbound queue and returns the exact detached mapped data.
    ///
    /// The complete detached vector must be returned in one move. Its
    /// cardinality is checked against the private queue ledger, and the
    /// returned generation is the one recorded by exact recycle and detach.
    /// Any mismatch terminally poisons the consumed session before native
    /// teardown, so a caller cannot retry with substituted custody.
    pub fn destroy_returning_detached_fixed_dispatch_resources(
        self,
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<Gfx942RecycledDispatchResourcesV1, ComputeAqlQueueSessionErrorV1> {
        match self.destroy_inner(QueueDestroyModeV1::ReturnDetached(data))? {
            QueueDestroyOutcomeV1::Returned(resources) => Ok(*resources),
            QueueDestroyOutcomeV1::Released(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "returning destroy released dispatch resources",
            )),
        }
    }

    pub(crate) fn destroy_with<T>(
        mut self,
        after_queue_destroyed: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(ComputeAqlQueueDestroyedV1, T), ComputeAqlQueueSessionErrorV1> {
        let after_event = self.destroy_queue_and_event(QueueDestroyModeV1::Release)?;
        if after_event.runtime_control.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "debug runtime requires linear teardown owner",
            ));
        }
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing destroyed queue engine",
            ))?;
        let disabled_runtime = after_event.runtime.disable(
            engine.backend.session.kfd_fd(),
            engine.backend.session.opener_pid(),
        )?;
        let (outcome, callback_result) = self.complete_destroy(
            disabled_runtime,
            after_event.shadows,
            after_event.return_attached,
            after_event.detached_return,
            after_queue_destroyed,
        )?;
        match outcome {
            QueueDestroyOutcomeV1::Released(destroyed) => Ok((destroyed, callback_result)),
            QueueDestroyOutcomeV1::Returned(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "ordinary destroy callback returned dispatch resources",
            )),
        }
    }

    fn destroy_inner(
        mut self,
        mode: QueueDestroyModeV1,
    ) -> Result<QueueDestroyOutcomeV1, ComputeAqlQueueSessionErrorV1> {
        let after_event = self.destroy_queue_and_event(mode)?;
        if after_event.runtime_control.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "debug runtime requires linear teardown owner",
            ));
        }
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing destroyed queue engine",
            ))?;
        let disabled_runtime = after_event.runtime.disable(
            engine.backend.session.kfd_fd(),
            engine.backend.session.opener_pid(),
        )?;
        let (outcome, ()) = self.complete_destroy(
            disabled_runtime,
            after_event.shadows,
            after_event.return_attached,
            after_event.detached_return,
            |_| Ok(()),
        )?;
        Ok(outcome)
    }

    fn destroy_queue_and_event(
        &mut self,
        mode: QueueDestroyModeV1,
    ) -> Result<QueueAfterEventDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        if !self.unpublished_dispatch.is_clear()
            && (!matches!(mode, QueueDestroyModeV1::Release)
                || !self.unpublished_dispatch.quiescent(
                    true,
                    self.dispatch.is_some(),
                    self.detached_dispatch_generation,
                    self.detached_data_count,
                    self.detached_data_identities.len(),
                    self.detached_next_insertion_index,
                ))
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "persistent compute attachment must be restored before queue destruction",
            ));
        }
        if !directional_persistent_sdma_queue_destroy_is_admitted_v1(self.sdma_outstanding_buffers)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "all SDMA buffers must be released before queue destruction",
            ));
        }
        if self
            .auxiliary_compute_lanes
            .iter()
            .any(|lane| lane.state.is_some())
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "auxiliary compute queues must be destroyed before the primary queue",
            ));
        }
        if !self.sdma_pool_free.is_empty() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "the SDMA memory pool must be trimmed before queue destruction",
            ));
        }
        self.dependency_owner
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        self.completion_owner.ensure_releasable()?;
        let (return_attached, detached_return) = match mode {
            QueueDestroyModeV1::Release => {
                if self.detached_data_count != 0 {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "detached dispatch data must be rebound or released before destroy",
                    ));
                }
                if let Some(dispatch) = self.dispatch.as_ref() {
                    dispatch.ensure_releasable()?;
                }
                (false, None)
            }
            QueueDestroyModeV1::ReturnAttached => {
                if self.detached_data_count != 0 {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "detached dispatch data must be rebound or released before destroy",
                    ));
                }
                self.dispatch
                    .as_ref()
                    .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
                    .ensure_returnable_for_destroy()?;
                (true, None)
            }
            QueueDestroyModeV1::ReturnDetached(data) => {
                let returned_data_identities = fixed_dispatch_storage_identities(&data);
                let identity_mismatch = first_ordered_identity_mismatch(
                    &self.detached_data_identities,
                    &returned_data_identities,
                );
                let generation = admit_detached_returning_destroy(
                    &mut self.terminal_poisoned,
                    DetachedReturningDestroyPreflightV1 {
                        dispatch_attached: self.dispatch.is_some(),
                        detached_data_count: self.detached_data_count,
                        detached_dispatch_generation: self.detached_dispatch_generation,
                        detached_identity_count: self.detached_data_identities.len(),
                        returned_data_count: returned_data_identities.len(),
                        identity_mismatch,
                    },
                )?;
                (false, Some((generation, data)))
            }
        };
        if self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .foundation
            .preflight_memory_transition_revisions(1)
            .is_err()
        {
            self.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(map_native(NativeQueueAdapterErrorV1::ModelProjection));
        }
        if let Some(striped_sdma) = self.striped_sdma.as_mut() {
            let memory = &mut self
                .engine
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?
                .backend
                .session;
            if let Err(error) = striped_sdma.destroy_queue(memory) {
                self.terminal_poisoned = true;
                return Err(error.into());
            }
        }
        if let Some(sdma) = self.sdma.as_mut() {
            let memory = &mut self
                .engine
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?
                .backend
                .session;
            if let Err(error) = sdma.destroy_queue(memory) {
                self.terminal_poisoned = true;
                return Err(error.into());
            }
        }
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        engine.destroy(self.key).map_err(map_native)?;
        let mut exception =
            self.exception
                .take()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue exception state",
                ))?;
        exception.runtime.mark_queue_destroyed()?;
        let destroyed_event = exception.event.destroy(
            engine.backend.session.kfd_fd(),
            engine.backend.session.opener_pid(),
        )?;
        let shadows = exception.shadows.after_event_destroy(destroyed_event)?;
        exception.runtime.mark_event_destroyed()?;
        Ok(QueueAfterEventDestroyedV1 {
            runtime: exception.runtime,
            runtime_control: exception.runtime_control,
            shadows,
            return_attached,
            detached_return,
        })
    }

    fn complete_destroy<T>(
        mut self,
        disabled_runtime: LinuxKfdRuntimeDisabledV1,
        shadows: LinuxCwsrShadowsAfterEventDestroyedV1,
        return_attached: bool,
        detached_return: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)>,
        after_queue_destroyed: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(QueueDestroyOutcomeV1, T), ComputeAqlQueueSessionErrorV1> {
        let shadow_release = shadows.after_runtime_destroy(disabled_runtime)?;
        self.doorbell
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing doorbell"))?
            .release()?;
        self.check_currentness()?;
        let authority = self
            .engine
            .as_mut()
            .expect("session engine")
            .release_destroyed_resources(self.key)
            .map_err(map_native)?;
        self.restore_model_ownership()?;
        release_resource_authority(
            &mut self
                .engine
                .as_mut()
                .expect("session engine")
                .backend
                .session,
            authority,
            shadow_release,
        )?;
        let released_sdma_resources = self
            .sdma
            .as_ref()
            .map_or(0, Gfx942SdmaQueueSetV1::additional_resource_count)
            .saturating_add(
                self.striped_sdma
                    .as_ref()
                    .map_or(0, Gfx942SdmaQueueSetV1::additional_resource_count),
            );
        if let Some(striped_sdma) = self.striped_sdma.take() {
            striped_sdma.release_resources(
                &mut self
                    .engine
                    .as_mut()
                    .expect("session engine")
                    .backend
                    .session,
            )?;
        }
        if let Some(sdma) = self.sdma.take() {
            sdma.release_resources(
                &mut self
                    .engine
                    .as_mut()
                    .expect("session engine")
                    .backend
                    .session,
            )?;
        }
        let returned_dispatch = match self.dispatch.take() {
            Some(dispatch) if return_attached => {
                let returned = dispatch.release_non_data_for_returning_destroy(
                    &mut self
                        .engine
                        .as_mut()
                        .expect("session engine")
                        .backend
                        .session,
                )?;
                Some((returned.generation(), recover_fixed_dispatch_data(returned)))
            }
            Some(dispatch) => {
                dispatch.release(
                    &mut self
                        .engine
                        .as_mut()
                        .expect("session engine")
                        .backend
                        .session,
                )?;
                None
            }
            None => detached_return,
        };
        let completion_signals =
            self.completion_signals
                .take()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing completion signal arena",
                ))?;
        let memory = &mut self
            .engine
            .as_mut()
            .expect("session engine")
            .backend
            .session;
        let completion_signals = memory.unmap_from_gpu(completion_signals.into_token())?;
        memory.release(completion_signals)?;
        let callback_result = after_queue_destroyed(memory)?;
        let destroyed = destroyed_queue_observation_with_additional_resources(
            self.observation.queue_id,
            released_sdma_resources,
        );
        let Some((dispatch_generation, data)) = returned_dispatch else {
            return Ok((QueueDestroyOutcomeV1::Released(destroyed), callback_result));
        };
        let backend = self
            .engine
            .take()
            .expect("session engine")
            .into_backend()
            .map_err(map_native)?;
        Ok((
            QueueDestroyOutcomeV1::Returned(Box::new(Gfx942RecycledDispatchResourcesV1 {
                destroyed,
                memory: backend.session,
                dispatch_generation,
                data,
            })),
            callback_result,
        ))
    }

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        engine.prepare_operation().map_err(map_native)
    }

    fn require_sdma_enabled(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        if self.sdma.is_none() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is not enabled",
            ));
        }
        Ok(())
    }

    fn require_striped_sdma_enabled(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        if self.striped_sdma.is_none()
            && !self
                .sdma
                .as_ref()
                .is_some_and(Gfx942SdmaQueueSetV1::is_striped)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "striped SDMA copy engines are not enabled",
            ));
        }
        Ok(())
    }

    fn require_logical_mux_sdma_enabled_v2(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        if !self
            .sdma
            .as_ref()
            .is_some_and(Gfx942SdmaQueueSetV1::is_logical_mux_v2)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "two-native logical-mux SDMA engines are not enabled",
            ));
        }
        Ok(())
    }

    fn active_compute_queue_ids_for_sdma_creation_v1(
        &mut self,
    ) -> Result<Vec<u32>, ComputeAqlQueueSessionErrorV1> {
        let active_auxiliary_count = self
            .auxiliary_compute_lanes
            .iter()
            .filter(|slot| slot.state.is_some())
            .count();
        let mut queue_ids = Vec::new();
        queue_ids
            .try_reserve_exact(active_auxiliary_count + 1)
            .map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("compute queue ID roster allocation")
            })?;
        let inserted = push_unique_queue_id_v1(&mut queue_ids, self.observation.queue_id);
        debug_assert!(inserted, "empty queue ID roster accepts the primary queue");
        let duplicate = self
            .auxiliary_compute_lanes
            .iter()
            .filter_map(|slot| slot.state.as_ref())
            .find_map(|lane| {
                let queue_id = lane.observation.queue_id;
                (!push_unique_queue_id_v1(&mut queue_ids, queue_id)).then_some(queue_id)
            });
        if duplicate.is_some() {
            self.poison_terminal();
            permanently_poison_process_global_kfd_runtime_gate_v1();
            return Err(terminal_creation(
                "compute queue ID admission",
                ComputeAqlQueueSessionErrorV1::Contract(
                    "compute queue IDs are not session-wide unique",
                ),
            ));
        }
        Ok(queue_ids)
    }

    fn striped_sdma_is_poisoned(&self) -> bool {
        self.striped_sdma
            .as_ref()
            .or_else(|| self.sdma.as_ref().filter(|owner| owner.is_striped()))
            .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned)
    }

    fn logical_mux_sdma_is_poisoned_v2(&self) -> bool {
        self.sdma
            .as_ref()
            .filter(|owner| owner.is_logical_mux_v2())
            .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned)
    }

    fn persistent_sdma_attachment_is_current(
        &self,
        attachment: &Gfx942PersistentSdmaAttachmentV1,
    ) -> bool {
        if attachment.queue != self.key {
            return false;
        }
        self.sdma
            .as_ref()
            .and_then(|owner| owner.exact_targeted_observation(attachment.engine_index))
            .is_some_and(|observation| {
                observation.queue_id == attachment.native_queue_id
                    && observation.engine_index == Some(attachment.engine_index)
                    && matches!(
                        attachment.engine_index,
                        crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1
                            | crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1
                    )
            })
    }

    fn directional_persistent_sdma_attachment_is_current(
        &self,
        attachment: &Gfx942PersistentDirectionalSdmaAttachmentV1,
    ) -> bool {
        if attachment.queue != self.key {
            return false;
        }
        self.sdma
            .as_ref()
            .and_then(Gfx942SdmaQueueSetV1::directional_observation)
            .and_then(|observation| admit_persistent_directional_sdma_pair_v1(observation).ok())
            .is_some_and(|pair| pair == attachment.pair)
    }

    fn check_directional_persistent_sdma_operational_currentness(
        &mut self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        })
    }

    fn terminal_admitted_directional_persistent_sdma_failure_v1(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        mut allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        direction: Gfx942PersistentSdmaDirectionV1,
    ) -> Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
        allocation
            .owner
            .quarantine_for_caller_reported_currentness_loss();
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: None,
                    state: Gfx942DirectionalPersistentSdmaTerminalStateV1::AdmissionRestored {
                        allocation,
                        host,
                    },
                },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    fn finish_asynchronous_directional_persistent_sdma_single_v1(
        &mut self,
        direction: Gfx942PersistentSdmaDirectionV1,
        admitted: Option<DirectionalPersistentSdmaAdmittedRequestV1>,
        outcome: Option<DirectionalPersistentSdmaAsynchronousSingleOutcomeV1>,
        loan_error: Option<ComputeAqlQueueSessionErrorV1>,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaSubmissionV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        let Some(outcome) = outcome else {
            let admitted = admitted.expect("unopened asynchronous loan retains admission");
            return Err(
                self.terminal_admitted_directional_persistent_sdma_failure_v1(
                    loan_error.unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "asynchronous directional persistent SDMA operation did not execute",
                    )),
                    admitted.allocation,
                    admitted.host,
                    admitted.direction,
                ),
            );
        };
        match outcome {
            DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::OpeningCurrentnessLost {
                admitted,
                error,
            } => Err(
                self.terminal_admitted_directional_persistent_sdma_failure_v1(
                    loan_error.unwrap_or(error),
                    admitted.allocation,
                    admitted.host,
                    admitted.direction,
                ),
            ),
            DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::RequestPreparationRejected(
                failure,
            ) if fused_async_single_prepublication_is_retryable_v1(
                loan_error.is_none(),
                true,
                true,
            ) =>
            {
                Err(failure)
            }
            DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::RequestPreparationRejected(
                failure,
            ) => {
                let (_, custody) = failure.into_parts();
                let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                } = custody
                else {
                    unreachable!("admitted request preparation only returns retryable custody")
                };
                Err(
                    self.terminal_admitted_directional_persistent_sdma_failure_v1(
                        loan_error.expect("failed loan has an error"),
                        allocation,
                        host,
                        direction,
                    ),
                )
            }
            DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::LowerPreparationRejected {
                prepared_request:
                    DirectionalPersistentSdmaPreparedRequestV1 {
                        allocation,
                        prepared_use,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        request,
                    },
                error,
                owner_healthy,
                closing_currentness_succeeded,
            } => {
                if !fused_async_single_prepublication_is_retryable_v1(
                    loan_error.is_none(),
                    owner_healthy,
                    closing_currentness_succeeded,
                ) {
                    return Err(self.terminal_prepared_directional_persistent_sdma_failure(
                        loan_error.unwrap_or(error),
                        allocation,
                        prepared_use,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        host_binding,
                        request,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    ));
                }
                let (mut allocation, host) = restore_directional_persistent_sdma_request_v1(
                    allocation,
                    direction,
                    host_offset,
                    device_offset,
                    copy_bytes,
                    host_binding,
                    request,
                )
                .unwrap_or_else(|_| unreachable!("exact prepared request must restore"));
                allocation
                    .owner
                    .cancel_prepared(prepared_use)
                    .expect("private prepared use must cancel");
                Err(Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                    error,
                    custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                        allocation,
                        host,
                    },
                })
            }
            DirectionalPersistentSdmaAsynchronousSingleOutcomeV1::Publication {
                custody,
                observation,
                error,
                preparation_succeeded,
                closing_currentness_succeeded,
            } => {
                let transition = transition_directional_persistent_sdma_publication_v1(
                    custody,
                    observation,
                    loan_error.is_none() && preparation_succeeded,
                    loan_error.is_none() && closing_currentness_succeeded,
                );
                self.finish_directional_persistent_sdma_publication_transition(
                    loan_error.unwrap_or(error),
                    transition,
                )
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn finish_directional_persistent_sdma_publication_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        transition: DirectionalPersistentSdmaPublicationTransitionV1,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaSubmissionV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        match transition {
            DirectionalPersistentSdmaPublicationTransitionV1::Retryable { allocation, host } => {
                Err(Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                    error,
                    custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                        allocation,
                        host,
                    },
                })
            }
            DirectionalPersistentSdmaPublicationTransitionV1::Published(submission) => {
                Ok(submission)
            }
            DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) => {
                self.poison_terminal();
                Err(Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
                    error,
                    custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                        custody,
                    ),
                })
            }
        }
    }

    fn terminal_directional_persistent_sdma_execution_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        custody: Gfx942DirectionalPersistentSdmaTerminalCustodyV1,
    ) -> Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(custody),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn terminal_prepared_directional_persistent_sdma_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
        request: Gfx942SdmaCopyRequestV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
        let sequence = prepared.sequence();
        let state = match restore_directional_persistent_sdma_request_v1(
            allocation,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            host_binding,
            request,
        ) {
            Ok((mut allocation, host)) => {
                allocation
                    .owner
                    .quarantine_prepared(prepared, reason)
                    .expect("private prepared use must quarantine");
                Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedRestored {
                    allocation,
                    host,
                }
            }
            Err((mut allocation, request)) => {
                allocation
                    .owner
                    .quarantine_prepared(prepared, reason)
                    .expect("private prepared use must quarantine");
                Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedUnrestored {
                    allocation,
                    request,
                }
            }
        };
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state,
                },
            ),
        }
    }

    fn terminal_queued_directional_persistent_sdma_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
        let Gfx942DirectionalPersistentSdmaSubmissionV1 {
            mut allocation,
            published,
            ticket,
            direction,
            ..
        } = submission;
        let sequence = published.sequence();
        allocation
            .owner
            .quarantine_published(published, reason)
            .expect("private published use must quarantine");
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state: Gfx942DirectionalPersistentSdmaTerminalStateV1::PublishedQueueRetained {
                        allocation,
                        ticket,
                    },
                },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    fn finish_directional_persistent_sdma_window_publication_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        transition: DirectionalPersistentSdmaWindowPublicationTransitionV1,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1,
    > {
        match transition {
            DirectionalPersistentSdmaWindowPublicationTransitionV1::Retryable {
                allocation,
                host,
            } => Err(Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    allocation,
                    host,
                },
            }),
            DirectionalPersistentSdmaWindowPublicationTransitionV1::Published(submission) => {
                Ok(submission)
            }
            DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) => {
                self.poison_terminal();
                Err(Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
                    error,
                    custody:
                        Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                            custody,
                        ),
                })
            }
        }
    }

    fn terminal_directional_persistent_sdma_window_execution_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        custody: Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1,
    ) -> Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                custody,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn terminal_prepared_directional_persistent_sdma_window_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        packet_count: usize,
        host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
        request: Gfx942SdmaCopyRequestV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
        let sequence = prepared.sequence();
        let state = match restore_directional_persistent_sdma_request_v1(
            allocation,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            host_binding,
            request,
        ) {
            Ok((mut allocation, host)) => {
                allocation
                    .owner
                    .quarantine_prepared(prepared, reason)
                    .expect("private prepared window use must quarantine");
                Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PreparedRestored {
                    allocation,
                    host,
                }
            }
            Err((mut allocation, request)) => {
                allocation
                    .owner
                    .quarantine_prepared(prepared, reason)
                    .expect("private prepared window use must quarantine");
                Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PreparedUnrestored {
                    allocation,
                    request,
                }
            }
        };
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    packet_count,
                    state,
                },
            ),
        }
    }

    fn terminal_queued_directional_persistent_sdma_window_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
        let Gfx942DirectionalPersistentSdmaWindowSubmissionV1 {
            mut allocation,
            published,
            tickets,
            direction,
            packet_count,
            ..
        } = submission;
        let sequence = published.sequence();
        allocation
            .owner
            .quarantine_published(published, reason)
            .expect("private published window use must quarantine");
        self.poison_terminal();
        Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    packet_count,
                    state:
                        Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                            allocation,
                            tickets,
                        },
                },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    fn finish_same_device_persistent_sdma_window_publication_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        transition: SameDevicePersistentSdmaWindowPublicationTransitionV1,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1,
    > {
        match transition {
            SameDevicePersistentSdmaWindowPublicationTransitionV1::Retryable {
                source,
                destination,
            } => Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                error,
                custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                    source,
                    destination,
                },
            }),
            SameDevicePersistentSdmaWindowPublicationTransitionV1::Published(submission) => {
                Ok(submission)
            }
            SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) => {
                self.poison_terminal();
                Err(Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
                    error,
                    custody:
                        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                            custody,
                        ),
                })
            }
        }
    }

    fn terminal_same_device_persistent_sdma_window_execution_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        custody: Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1,
    ) -> Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
        self.poison_terminal();
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                custody,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn terminal_prepared_same_device_persistent_sdma_window_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        source_prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        destination_prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        descriptor: crate::persistent_same_device_sdma::Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
        let transition = transition_same_device_persistent_sdma_window_publication_v1(
            SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                source,
                source_prepared,
                destination,
                destination_prepared,
                planned_tickets: Vec::new(),
                descriptor,
            },
            SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request),
            false,
            false,
        );
        let SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
            transition
        else {
            unreachable!("failed enclosing operation must retain terminal same-device custody")
        };
        self.poison_terminal();
        Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                custody,
            ),
        }
    }

    fn terminal_queued_same_device_persistent_sdma_window_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
        let Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
            mut source,
            source_published,
            mut destination,
            destination_published,
            tickets,
            descriptor,
        } = submission;
        let source_sequence = source_published.sequence();
        let destination_sequence = destination_published.sequence();
        quarantine_published_local_sdma_pair_v1(
            &mut source.owner,
            source_published,
            &mut destination.owner,
            destination_published,
            reason,
        )
        .unwrap_or_else(|failure| panic!("private published pair: {:?}", failure.error));
        self.poison_terminal();
        Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
            error,
            custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                    source_sequence: Some(source_sequence),
                    destination_sequence: Some(destination_sequence),
                    descriptor,
                    state:
                        Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                            source,
                            destination,
                            tickets,
                        },
                },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    fn finish_persistent_sdma_publication_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        transition: PersistentSdmaPublicationTransitionV1,
    ) -> Result<Gfx942PersistentSdmaSubmissionV1, Gfx942PersistentSdmaSubmissionFailureV1> {
        match transition {
            PersistentSdmaPublicationTransitionV1::Retryable { allocation, host } => {
                Err(Gfx942PersistentSdmaSubmissionFailureV1 {
                    error,
                    custody: Gfx942PersistentSdmaSubmissionCustodyV1::Retryable {
                        allocation,
                        host,
                    },
                })
            }
            PersistentSdmaPublicationTransitionV1::Published(submission) => Ok(submission),
            PersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) => {
                self.poison_terminal();
                Err(Gfx942PersistentSdmaSubmissionFailureV1 {
                    error,
                    custody: Gfx942PersistentSdmaSubmissionCustodyV1::ProcessTeardown(custody),
                })
            }
        }
    }

    fn terminal_persistent_sdma_execution_transition(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        custody: Gfx942PersistentSdmaTerminalCustodyV1,
    ) -> Gfx942PersistentSdmaExecutionFailureV1 {
        self.poison_terminal();
        Gfx942PersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942PersistentSdmaExecutionCustodyV1::ProcessTeardown(custody),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn terminal_prepared_persistent_sdma_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        allocation: Gfx942QueuePersistentAllocationV1,
        prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        host_binding: Gfx942PersistentSdmaHostBindingV1,
        request: Gfx942SdmaCopyRequestV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942PersistentSdmaSubmissionFailureV1 {
        let sequence = prepared.sequence();
        let state = match restore_persistent_sdma_request(
            allocation,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            host_binding,
            request,
        ) {
            Ok((mut allocation, host)) => {
                allocation
                    .owner
                    .quarantine_prepared(prepared, reason)
                    .expect("private prepared use must quarantine");
                Gfx942PersistentSdmaTerminalStateV1::PreparedRestored { allocation, host }
            }
            Err((mut allocation, request)) => {
                allocation
                    .owner
                    .quarantine_prepared(prepared, reason)
                    .expect("private prepared use must quarantine");
                Gfx942PersistentSdmaTerminalStateV1::PreparedUnrestored {
                    allocation,
                    request,
                }
            }
        };
        self.poison_terminal();
        Gfx942PersistentSdmaSubmissionFailureV1 {
            error,
            custody: Gfx942PersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                Gfx942PersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state,
                },
            ),
        }
    }

    fn terminal_queued_persistent_sdma_failure(
        &mut self,
        error: ComputeAqlQueueSessionErrorV1,
        submission: Gfx942PersistentSdmaSubmissionV1,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Gfx942PersistentSdmaExecutionFailureV1 {
        let Gfx942PersistentSdmaSubmissionV1 {
            mut allocation,
            published,
            ticket,
            host_binding: _,
            direction,
            host_offset: _,
            device_offset: _,
            copy_bytes: _,
        } = submission;
        let sequence = published.sequence();
        allocation
            .owner
            .quarantine_published(published, reason)
            .expect("private published use must quarantine");
        self.poison_terminal();
        Gfx942PersistentSdmaExecutionFailureV1 {
            error,
            custody: Gfx942PersistentSdmaExecutionCustodyV1::ProcessTeardown(
                Gfx942PersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state: Gfx942PersistentSdmaTerminalStateV1::PublishedQueueRetained {
                        allocation,
                        ticket,
                    },
                },
            ),
        }
    }

    fn checkout_sdma_pool(
        &mut self,
        kind: Gfx942SdmaBufferKindV1,
        requested_bytes: u64,
        required_alignment: u64,
    ) -> Result<Option<Gfx942SdmaBufferV1>, ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        self.require_sdma_enabled()?;
        if requested_bytes == 0 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "pooled buffer length must be nonzero",
            ));
        }
        self.validate_configured_device_pool_v1()?;
        self.validate_configured_host_pool_v1()?;
        let best = self
            .sdma_pool_free
            .iter()
            .enumerate()
            .filter(|(_, buffer)| {
                buffer.kind() == kind
                    && buffer.physical_bytes() >= requested_bytes
                    && buffer.physical_alignment() >= required_alignment
            })
            .min_by_key(|(_, buffer)| (buffer.physical_bytes(), buffer.physical_alignment()))
            .map(|(index, _)| index);
        let Some(index) = best else {
            return Ok(None);
        };
        let next_outstanding = self.sdma_outstanding_buffers.checked_add(1).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("SDMA buffer ledger exhausted"),
        )?;
        let next_reuse = self.sdma_pool_reuse_count.checked_add(1).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("SDMA pool reuse counter exhausted"),
        )?;
        let buffer = self.sdma_pool_free.swap_remove(index);
        self.sdma_outstanding_buffers = next_outstanding;
        self.sdma_pool_reuse_count = next_reuse;
        Ok(Some(buffer))
    }

    fn validate_configured_device_pool_v1(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if let Err(error) = self.sdma_device_pool_usage_v1() {
            self.poison_terminal();
            return Err(error);
        }
        Ok(())
    }

    fn validate_configured_host_pool_v1(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if let Err(error) = self.sdma_host_pool_usage_v1() {
            self.poison_terminal();
            return Err(error);
        }
        Ok(())
    }

    fn with_sdma_owner_memory<R>(
        &mut self,
        operation: impl FnOnce(
            &mut Gfx942SdmaQueueSetV1,
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        let mut owner = self.sdma.take().expect("checked SDMA owner");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model(|memory| operation(&mut owner, memory))
        }));
        self.sdma = Some(owner);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn with_striped_sdma_owner_memory<R>(
        &mut self,
        operation: impl FnOnce(
            &mut Gfx942SdmaQueueSetV1,
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.require_striped_sdma_enabled()?;
        let separate = self.striped_sdma.is_some();
        let owner = if separate {
            self.striped_sdma.take()
        } else {
            self.sdma.take()
        };
        let Some(mut owner) = owner else {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "missing striped SDMA owner",
            ));
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model(|memory| operation(&mut owner, memory))
        }));
        if separate {
            self.striped_sdma = Some(owner);
        } else {
            self.sdma = Some(owner);
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn with_logical_mux_sdma_owner_memory_v2<R>(
        &mut self,
        operation: impl FnOnce(
            &mut Gfx942SdmaQueueSetV1,
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.require_logical_mux_sdma_enabled_v2()?;
        let Some(mut owner) = self.sdma.take() else {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "missing two-native logical-mux SDMA owner",
            ));
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model(|memory| operation(&mut owner, memory))
        }));
        self.sdma = Some(owner);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn with_live_queue_memory_model<R>(
        &mut self,
        operation: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        let (result, retake) = self.with_live_queue_memory_model_custody(operation)?;
        retake?;
        result
    }

    fn with_live_queue_memory_model_custody<R>(
        &mut self,
        operation: impl FnOnce(&mut SharedGttMemorySessionV1) -> R,
    ) -> Result<(R, Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1> {
        execute_live_model_custody_v1(
            self,
            |session| session.restore_model_ownership_for_live_mutation(),
            |session| {
                let engine = session
                    .engine
                    .as_mut()
                    .expect("model loan requires queue engine");
                operation(&mut engine.backend.session)
            },
            |session, loan| session.retake_model_ownership_after_live_mutation(loan),
            |session| {
                session.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
            },
        )
    }

    fn with_sdma_queue_creation_custody_v1<R>(
        &mut self,
        stage: &'static str,
        operation: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, crate::sdma::Gfx942SdmaQueueSetCreationFailureV1>,
        terminalize_success: impl FnOnce(R) -> Gfx942SdmaQueueSetV1,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        let mut created = None;
        let envelope = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model(|memory| {
                created = Some(operation(memory));
                Ok(())
            })
        }));
        let envelope = match envelope {
            Ok(envelope) => envelope,
            Err(payload) => {
                // A panicking creation callee may already have consumed native
                // owners. No recoverable owner custody is claimed in that case.
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match (envelope, created) {
            (Ok(()), Some(Ok(created))) => Ok(created),
            (Ok(()), Some(Err(failure))) => {
                let (error, disposition, retained) = failure.into_parts();
                if matches!(
                    disposition,
                    crate::sdma::Gfx942SdmaQueueSetCreationDispositionV1::Terminal
                ) {
                    if let Some(retained) = retained {
                        self.sdma = Some(retained);
                    }
                    self.poison_terminal();
                    permanently_poison_process_global_kfd_runtime_gate_v1();
                    Err(terminal_creation(stage, error.into()))
                } else {
                    Err(error.into())
                }
            }
            (Err(envelope_error), Some(Ok(created))) => {
                self.sdma = Some(terminalize_success(created));
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                Err(terminal_creation(stage, envelope_error))
            }
            (Err(envelope_error), Some(Err(failure))) => {
                let (_lower_error, _disposition, retained) = failure.into_parts();
                if let Some(retained) = retained {
                    self.sdma = Some(retained);
                }
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                Err(terminal_creation(stage, envelope_error))
            }
            (Ok(()), None) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                Err(terminal_creation(
                    stage,
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA creation operation did not execute",
                    ),
                ))
            }
            (Err(error), None) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                Err(terminal_creation(stage, error))
            }
        }
    }

    fn release_persistent_dispatch_data(
        &mut self,
        after_recycle: bool,
    ) -> Result<
        (u64, Vec<Gfx942FixedDispatchDataV1>),
        (
            ComputeAqlQueueSessionErrorV1,
            Vec<Gfx942FixedDispatchDataV1>,
        ),
    > {
        #[cfg(test)]
        if !after_recycle && let Some(returned) = self.persistent_compute_test_release.take() {
            return Ok(returned);
        }
        let Some(dispatch) = self.dispatch.take() else {
            return Err((
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                Vec::new(),
            ));
        };
        let mut dispatch = Some(dispatch);
        let envelope = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model_custody(|memory| {
                let dispatch = dispatch
                    .take()
                    .expect("custody operation executes at most once");
                if after_recycle {
                    dispatch.release_persistent_data_after_recycle(memory)
                } else {
                    dispatch.release_persistent_data_before_publication(memory)
                }
                .map_err(|(error, data)| (error.into(), data))
            })
        }));
        let envelope = match envelope {
            Ok(envelope) => envelope,
            Err(payload) => {
                // A consuming callee may already have dropped the owner while
                // unwinding. Restore it only when it remains in this frame;
                // otherwise only terminal/process-poison disposition is known;
                // no retained owner or native-resource custody is claimed.
                self.dispatch = dispatch;
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match envelope {
            Ok((result, Ok(()))) => result,
            Ok((result, Err(error))) => {
                let data = match result {
                    Ok((_, data)) | Err((_, data)) => data,
                };
                Err((error, data))
            }
            Err(error) => {
                self.dispatch = dispatch;
                Err((error, Vec::new()))
            }
        }
    }

    fn detach_persistent_dispatch_data_retaining_control_v1(
        &mut self,
    ) -> Result<
        (u64, Vec<Gfx942FixedDispatchDataV1>),
        (
            ComputeAqlQueueSessionErrorV1,
            Vec<Gfx942FixedDispatchDataV1>,
        ),
    > {
        let Some(dispatch) = self.dispatch.as_mut() else {
            return Err((
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                Vec::new(),
            ));
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch.detach_persistent_replay_data_after_recycle_v1()
        })) {
            Ok(result) => result.map_err(|error| (error.into(), Vec::new())),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn restore_model_ownership_for_live_mutation(
        &mut self,
    ) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        if !engine.backend.foundation_in_engine {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "live-queue model foundation was already restored",
            ));
        }
        let loan = engine
            .backend
            .session
            .loan_queue_model_foundation_for_live_mutation(&mut engine.foundation)?;
        engine.backend.foundation_in_engine = false;
        Ok(loan)
    }

    fn retake_model_ownership_after_live_mutation(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        if engine.backend.foundation_in_engine {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "live-queue model foundation was not restored",
            ));
        }
        engine
            .backend
            .session
            .retake_queue_model_foundation_after_live_mutation(&mut engine.foundation, loan)?;
        engine.backend.foundation_in_engine = true;
        Ok(())
    }

    fn restore_model_ownership(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        if !engine.backend.foundation_in_engine {
            return Ok(());
        }
        let domain = engine.foundation.identity().domain_id();
        let foundation = core::mem::replace(
            &mut engine.foundation,
            QueueModelFoundationV1::empty(domain),
        );
        engine
            .backend
            .session
            .restore_queue_model_foundation(foundation)?;
        engine.backend.foundation_in_engine = false;
        Ok(())
    }
}

fn validate_barrier_probe_success_snapshot(
    observation: Gfx942TimeoutExecutionObservationV1,
) -> Result<Gfx942TimeoutExecutionObservationV1, Gfx942CompletionErrorV1> {
    // The device may advance the read counter before or after this sequential
    // host snapshot and may invalidate a consumed packet before the host reads
    // its header. Both header states are valid only after signal completion.
    let packet_header = observation.first_packet_header();
    if observation.packet_count() != 1
        || observation.write_counter() != 1
        || observation.read_counter() > 1
        || (packet_header != fe2o3_aql::AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1
            && packet_header != fe2o3_aql::AQL_INVALID_PACKET_HEADER_V1)
        || observation.first_packet_setup() != 0
        || observation.first_signal_kind() != fe2o3_aql::AMD_SIGNAL_KIND_USER_V1
        || observation.first_signal() != Gfx942TimeoutSignalObservationV1::Completed
        || !observation.currentness_confirmed()
        || observation.queue_exception_reason_mask() != 0
    {
        return Err(Gfx942CompletionErrorV1::Observation);
    }
    Ok(observation)
}

impl Drop for ComputeAqlQueueSessionV1 {
    fn drop(&mut self) {
        // Model ownership can be restored without native effects. There is
        // deliberately no ioctl, MMIO store, munmap, GPU unmap, or FREE here.
        let _ = self.restore_model_ownership();
    }
}

fn build_resource_view(
    current_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    geometry: Gfx942AqlQueueResourcePlanV1,
    ring: &RingAuthority,
    control: &ControlAuthority,
    eop: &EopAuthority,
    context_save: &ContextSaveAuthority,
    next_queue: &AtomicU64,
) -> Result<NativeQueueResourceViewV1, ComputeAqlQueueSessionErrorV1> {
    let rf = ring.facts();
    let cf = control.facts();
    let ef = eop.facts();
    let sf = context_save.facts();
    let ring_backing = ring.backing();
    let vm = rf.mapping().allocation.vm;
    if [
        cf.mapping().allocation.vm,
        ef.mapping().allocation.vm,
        sf.mapping().allocation.vm,
    ]
    .iter()
    .any(|other| *other != vm)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "queue resource VM substitution",
        ));
    }
    let expected_ring_gpu_va_bytes = ring_backing.gpu_va_bytes(geometry.ring().mapping_bytes());
    let ring_base = rf
        .checked_gpu_subrange(0, expected_ring_gpu_va_bytes, 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("ring geometry"))?;
    if rf.logical_bytes() != geometry.ring().mapping_bytes() as usize
        || rf.gpu_va_bytes() != expected_ring_gpu_va_bytes
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract("ring size/profile"));
    }
    if cf.logical_bytes() != CONTROL_BYTES || cf.gpu_va_bytes() != CONTROL_BYTES as u64 {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "control size/profile",
        ));
    }
    // KFD truncates each pointer to its GPU page and requires that page to be
    // one exact PAGE_SIZE GPUVM mapping. The AMD AQL queue ABI places these
    // counters in distinct cache lines within that single reviewed page.
    let (write_pointer, read_pointer) = cf
        .checked_disjoint_gpu_subranges(
            (
                geometry.control().write_dispatch_id_offset_bytes(),
                geometry.control().counter_bytes(),
                geometry.control().counter_alignment_bytes(),
            ),
            (
                geometry.control().read_dispatch_id_offset_bytes(),
                geometry.control().counter_bytes(),
                geometry.control().counter_alignment_bytes(),
            ),
        )
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("control subranges"))?;
    let eop_base = ef
        .checked_gpu_subrange(0, geometry.end_of_pipe().mapping_bytes(), 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("EOP geometry"))?;
    if ef.logical_bytes() as u64 != geometry.end_of_pipe().mapping_bytes()
        || ef.gpu_va_bytes() != geometry.end_of_pipe().mapping_bytes()
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract("EOP size/profile"));
    }
    let context_base = sf
        .checked_gpu_subrange(0, geometry.context_save().mapping_bytes(), 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "context-save geometry",
        ))?;
    if sf.logical_bytes() as u64 != geometry.context_save().mapping_bytes()
        || sf.gpu_va_bytes() != geometry.context_save().mapping_bytes()
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "context-save size/profile",
        ));
    }
    // CREATE_QUEUE fields are per-XCC. The retained CWSR BO covers the
    // driver's independently checked aggregate across all XCCs.
    let ctl_stack_size = geometry.context_save().control_stack_bytes_per_xcc();
    let queue_number = next_queue
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("queue identity exhausted"))?;
    let queue = QueueKeyV1 {
        vm,
        id: QueueInstanceIdV1(queue_number),
        generation: QueueGenerationV1(1),
    };
    let plan_id = QueuePlanIdV1::from_untrusted_digest(digest_id(
        b"plan",
        queue,
        &[rf.mapping(), cf.mapping(), ef.mapping(), sf.mapping()],
        ring_backing,
        rf.logical_bytes(),
        rf.gpu_va_bytes(),
    ));
    let configuration = QueueConfigurationIdV1::from_untrusted_digest(digest_id(
        b"configuration",
        queue,
        &[rf.mapping(), cf.mapping(), ef.mapping(), sf.mapping()],
        ring_backing,
        rf.logical_bytes(),
        rf.gpu_va_bytes(),
    ));
    let binding = |facts: &crate::shared_memory::SharedGttMappedResourceFactsV1, kind| {
        ComputeAqlResourceBindingV1 {
            mapping: facts.mapping(),
            publication: facts.publication(),
            expected_kind: kind,
            expected_coherence: MemoryCoherenceV1::HostCoherent,
            expected_access: MemoryAccessV1::ReadWrite,
        }
    };
    let plan = ComputeAqlQueuePlanV1 {
        schema_version: fe2o3_runtime_model::QUEUE_LIFECYCLE_SCHEMA_VERSION_V1,
        target: ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        domain_id: current_device.domain_id(),
        plan_id,
        current_device,
        queue,
        initial_configuration: configuration,
        resources: ComputeAqlQueueResourcesV1 {
            ring: binding(rf, MemoryKindV1::QueueStorage),
            control: binding(cf, MemoryKindV1::HostVisibleCoherent),
            eop: binding(ef, MemoryKindV1::Executable),
            context_save: binding(sf, MemoryKindV1::Executable),
            private_scratch: None,
        },
    };
    let view = NativeQueueResourceViewV1 {
        plan,
        buffers: KfdAqlComputeQueueBuffers {
            ring_base_address: ring_base,
            write_pointer_address: write_pointer,
            read_pointer_address: read_pointer,
            eop_buffer_address: eop_base,
            eop_buffer_size: geometry.end_of_pipe().mapping_bytes(),
            ctx_save_restore_address: context_base,
            ctx_save_restore_size: geometry.context_save().context_save_bytes_per_xcc(),
            ctl_stack_size,
        },
        ring_size: admit_kfd_aql_queue_ring_size(geometry.ring().mapping_bytes())
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring UAPI size"))?,
        initial_percentage: admit_kfd_queue_percentage(100)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("queue percentage"))?,
        priority: admit_kfd_queue_priority(0)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("queue priority"))?,
    };
    validate_resource_view(view, [rf, cf, ef, sf]).map_err(map_native)?;
    Ok(view)
}

fn validate_resource_authority(
    authority: &QueueResourceAuthorityV1,
) -> Result<(), NativeQueueAdapterErrorV1> {
    validate_resource_view(
        authority.view,
        [
            authority.ring.facts(),
            authority.control.facts(),
            authority.eop.facts(),
            authority.context_save.facts(),
        ],
    )
}

fn validate_resource_view(
    view: NativeQueueResourceViewV1,
    facts: [&SharedGttMappedResourceFactsV1; 4],
) -> Result<(), NativeQueueAdapterErrorV1> {
    for (binding, facts) in view
        .plan
        .resources
        .ordered()
        .iter()
        .map(|(_, binding)| binding)
        .zip(facts)
    {
        if binding.mapping != facts.mapping() || binding.publication != facts.publication() {
            return Err(NativeQueueAdapterErrorV1::InvalidResource(
                "queue authority substitution",
            ));
        }
    }
    Ok(())
}

fn release_resource_authority(
    memory: &mut SharedGttMemorySessionV1,
    authority: QueueResourceAuthorityV1,
    shadow_release: LinuxCwsrShadowsReadyForReleaseV1,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    shadow_release.validate_for_release()?;
    let ring = authority.ring.unmap(memory)?;
    let control = memory.unmap_from_gpu(authority.control.into_token())?;
    let eop = memory.unmap_executable_from_gpu(authority.eop.into_token())?;
    let context_save = memory.unmap_executable_from_gpu(authority.context_save.into_token())?;
    ring.release(memory)?;
    memory.release(control)?;
    memory.release_executable(eop)?;
    memory.release_executable(context_save)?;
    shadow_release.complete()?;
    Ok(())
}

fn digest_id(
    tag: &[u8],
    queue: QueueKeyV1,
    mappings: &[fe2o3_runtime_model::MemoryMappingKeyV1; 4],
    ring_backing: QueueRingBackingV1,
    ring_logical_bytes: usize,
    ring_gpu_va_bytes: u64,
) -> IdentityDigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1.as_bytes());
    hasher.update(SHARED_GTT_MEMORY_PROFILE_SHA256_V1.as_bytes());
    hasher.update(tag);
    hasher.update([ring_backing.digest_tag()]);
    hasher.update((ring_logical_bytes as u64).to_le_bytes());
    hasher.update(ring_gpu_va_bytes.to_le_bytes());
    hasher.update(queue.id.0.to_le_bytes());
    hasher.update(queue.generation.0.to_le_bytes());
    for mapping in mappings {
        hasher.update(mapping.allocation.vm.id.0.to_le_bytes());
        hasher.update(mapping.allocation.id.0.to_le_bytes());
        hasher.update(mapping.allocation.generation.0.to_le_bytes());
        hasher.update(mapping.id.0.to_le_bytes());
    }
    IdentityDigestV1::from_untrusted_bytes(hasher.finalize().into())
}

fn map_timeout_memory_observation_error(error: MemorySessionError) -> Gfx942CompletionErrorV1 {
    match error {
        MemorySessionError::Device(_)
        | MemorySessionError::ProcessChanged
        | MemorySessionError::ProcessVmStatePoisoned
        | MemorySessionError::SharedSessionQuarantined => Gfx942CompletionErrorV1::Currentness,
        _ => Gfx942CompletionErrorV1::Observation,
    }
}

fn observe_then_poison<S, T, E>(
    state: &mut S,
    observe: impl FnOnce(&mut S) -> Result<T, E>,
    poison: impl FnOnce(&mut S),
) -> Result<T, E> {
    let observation = observe(state);
    poison(state);
    observation
}

fn push_unique_queue_id_v1(queue_ids: &mut Vec<u32>, queue_id: u32) -> bool {
    if queue_ids.contains(&queue_id) {
        false
    } else {
        queue_ids.push(queue_id);
        true
    }
}

const fn queue_id_collides_with_session_owned_roster_v1(
    candidate: u32,
    primary: u32,
    auxiliary_collision: bool,
    sdma_collision: bool,
) -> bool {
    candidate == primary || auxiliary_collision || sdma_collision
}

fn terminal_creation(
    stage: &'static str,
    source: ComputeAqlQueueSessionErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    ComputeAqlQueueSessionErrorV1::TerminalCreation {
        stage,
        source: Box::new(source),
    }
}

fn map_create(error: NativeQueueAdapterErrorV1) -> ComputeAqlQueueSessionErrorV1 {
    if matches!(
        error,
        NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Create)
    ) {
        map_native(error)
    } else {
        terminal_creation("CREATE_QUEUE result", map_native(error))
    }
}

fn map_native(error: NativeQueueAdapterErrorV1) -> ComputeAqlQueueSessionErrorV1 {
    let detail = match error {
        NativeQueueAdapterErrorV1::ProcessChanged => "queue process changed",
        NativeQueueAdapterErrorV1::Currentness(_) => "queue currentness lost",
        NativeQueueAdapterErrorV1::InvalidResource(_) => "invalid queue resource",
        NativeQueueAdapterErrorV1::InvalidPhase => "invalid queue phase",
        NativeQueueAdapterErrorV1::JournalCapacity => "queue journal capacity",
        NativeQueueAdapterErrorV1::BackendFailedNoEffect(_) => {
            "queue syscall failed with no effect"
        }
        NativeQueueAdapterErrorV1::BackendIndeterminate(_) => "queue syscall result indeterminate",
        NativeQueueAdapterErrorV1::MalformedKernelResult(_, _) => "malformed queue kernel result",
        NativeQueueAdapterErrorV1::ModelProjection => "queue model projection",
        NativeQueueAdapterErrorV1::AuthorityPoisoned => "queue authority poisoned",
    };
    ComputeAqlQueueSessionErrorV1::Native(detail)
}

fn map_submission(error: NativeAqlSubmissionErrorV1) -> ComputeAqlQueueSessionErrorV1 {
    map_submission_ref(&error)
}

fn map_submission_ref(error: &NativeAqlSubmissionErrorV1) -> ComputeAqlQueueSessionErrorV1 {
    let detail = match error {
        NativeAqlSubmissionErrorV1::InvalidQueue(_) => "invalid submission queue",
        NativeAqlSubmissionErrorV1::InvalidRing(_) => "invalid submission ring",
        NativeAqlSubmissionErrorV1::InvalidCwsr(_) => "invalid submission CWSR",
        NativeAqlSubmissionErrorV1::Poisoned => "submission owner poisoned",
        NativeAqlSubmissionErrorV1::Currentness => "submission currentness lost",
        NativeAqlSubmissionErrorV1::CounterObservation => "submission counter observation",
        NativeAqlSubmissionErrorV1::WriteCounterReplay { .. } => "submission write replay",
        NativeAqlSubmissionErrorV1::Ring(_) => "submission ring occupancy",
        NativeAqlSubmissionErrorV1::WriteCounterRace { .. } => "submission write race",
        NativeAqlSubmissionErrorV1::PacketBody => "submission packet body",
        NativeAqlSubmissionErrorV1::PacketHeader => "submission packet header",
        NativeAqlSubmissionErrorV1::Doorbell => "submission doorbell",
        NativeAqlSubmissionErrorV1::CallbackPanic => "submission callback panic",
    };
    ComputeAqlQueueSessionErrorV1::Native(detail)
}

fn map_dependency_target_use_error_v1(
    error: ComputeDependencyTargetUseErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    match error {
        ComputeDependencyTargetUseErrorV1::Completion(error) => error.into(),
        ComputeDependencyTargetUseErrorV1::Plan(_) => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency packet plan")
        }
        ComputeDependencyTargetUseErrorV1::Allocation => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency custody allocation")
        }
        ComputeDependencyTargetUseErrorV1::Poisoned => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency owner poisoned")
        }
        ComputeDependencyTargetUseErrorV1::ActiveTargetUse => {
            ComputeAqlQueueSessionErrorV1::Contract("dependent target still retained")
        }
        ComputeDependencyTargetUseErrorV1::ActiveTargetCapacity => {
            ComputeAqlQueueSessionErrorV1::Contract("active dependency target capacity exhausted")
        }
        ComputeDependencyTargetUseErrorV1::InvalidSessionOccurrence => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency session occurrence")
        }
        ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency acceptance epoch exhausted")
        }
        ComputeDependencyTargetUseErrorV1::EmptyDependencyRoster => {
            ComputeAqlQueueSessionErrorV1::Contract("empty dependency roster")
        }
        ComputeDependencyTargetUseErrorV1::TooManyDependencies => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency roster exceeds 256")
        }
        ComputeDependencyTargetUseErrorV1::InvalidTargetIdentity
        | ComputeDependencyTargetUseErrorV1::PublishedTargetMismatch => {
            ComputeAqlQueueSessionErrorV1::Contract("dependent target identity")
        }
        ComputeDependencyTargetUseErrorV1::CrossSessionDependency => {
            ComputeAqlQueueSessionErrorV1::Contract("cross-session dependency")
        }
        ComputeDependencyTargetUseErrorV1::SameQueueDependency => {
            ComputeAqlQueueSessionErrorV1::Contract("same-queue dependency")
        }
        ComputeDependencyTargetUseErrorV1::SelfDependency => {
            ComputeAqlQueueSessionErrorV1::Contract("self dependency")
        }
        ComputeDependencyTargetUseErrorV1::DependencyCycle => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency epoch cycle")
        }
        ComputeDependencyTargetUseErrorV1::DuplicateDependency => {
            ComputeAqlQueueSessionErrorV1::Contract("duplicate dependency")
        }
        ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch => {
            ComputeAqlQueueSessionErrorV1::Contract("dependency source lane")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn retained_device_scope_requires_exact_live_queue_phase_and_unpoisoned_owners() {
        use super::ComputeAqlQueuePhaseV1::*;
        for phase in [
            None,
            Some(Planned),
            Some(CreatePending),
            Some(Active),
            Some(UpdatePending),
            Some(DisablePending),
            Some(Disabled),
            Some(DestroyPending),
            Some(CancelledBeforeCreate),
            Some(Destroyed),
            Some(Ambiguous),
        ] {
            for terminal in [false, true] {
                for poisoned in [false, true] {
                    assert_eq!(
                        super::retained_device_queue_is_active_v1(terminal, poisoned, phase),
                        !terminal && !poisoned && phase == Some(Active)
                    );
                }
            }
        }
    }
    use super::*;

    #[test]
    fn device_pool_configuration_is_optional_immutable_and_history_is_irreversible() {
        let limits = Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap();
        let mut configuration = SdmaDevicePoolConfigurationV1::default();
        assert!(configuration.limits.is_none());
        configuration.configure(limits).unwrap();
        assert_eq!(configuration.limits, Some(limits));
        assert!(configuration.configure(limits).is_err());
        assert!(
            configuration
                .configure(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap())
                .is_err()
        );
        configuration.begin_activity();
        assert!(configuration.configure(limits).is_err());
        assert_eq!(configuration.limits, Some(limits));

        let mut configuration = SdmaDevicePoolConfigurationV1::default();
        configuration.begin_activity();
        configuration.begin_activity();
        assert!(configuration.configure(limits).is_err());
        assert!(configuration.limits.is_none());
    }

    #[test]
    fn device_pool_every_sdma_resource_attempt_closes_configuration_even_on_rejection() {
        let queue = test_queue_key(801, 1);
        for attempt in 0..14 {
            let mut session = persistent_compute_cancellation_test_session(queue, None, None);
            if attempt < 6 {
                // A deliberately unavailable native owner rejects enable before native entry.
                session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
            }
            match attempt {
                0 => assert!(session.enable_sdma_copy_engine().is_err()),
                1 => assert!(
                    session
                        .enable_gfx942_directional_sdma_copy_engines()
                        .is_err()
                ),
                2 => assert!(
                    session
                        .enable_gfx942_sdma_copy_engine_on_engine_index(0)
                        .is_err()
                ),
                3 => assert!(session.enable_gfx942_striped_sdma_copy_engines(2).is_err()),
                4 => assert!(
                    session
                        .enable_gfx942_two_native_sdma_logical_mux_v2(2)
                        .is_err()
                ),
                5 => assert!(
                    session
                        .enable_gfx942_directional_and_striped_sdma_copy_engines_v1(2)
                        .is_err()
                ),
                6 => assert!(session.allocate_sdma_host_buffer(16).is_err()),
                7 => assert!(session.allocate_sdma_device_buffer(16, 4).is_err()),
                8 => assert!(session.allocate_sdma_pooled_host_buffer(16).is_err()),
                9 => assert!(session.allocate_sdma_pooled_device_buffer(16, 0).is_err()),
                10 => assert!(
                    session
                        .checkout_sdma_pool(Gfx942SdmaBufferKindV1::DeviceLocal, 16, 4)
                        .is_err()
                ),
                11 => {
                    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 1);
                    assert!(session.recycle_sdma_buffer(device).is_err());
                }
                12 => assert!(session.trim_sdma_memory_pool().is_err()),
                13 => {
                    let (device, _) = crate::sdma::persistent_sdma_buffers_for_test(queue, 1);
                    assert!(session.release_sdma_buffer(device).is_err());
                }
                _ => unreachable!(),
            }
            assert!(
                session.sdma_device_pool.activity_started,
                "attempt {attempt}"
            );
            assert!(
                session
                    .sdma_device_pool
                    .configure(Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap())
                    .is_err()
            );
            assert!(session.sdma_device_pool.limits.is_none());
            assert!(
                session
                    .configure_sdma_host_pool_v1(Gfx942HostPoolLimitsV1::new(4096, 1).unwrap())
                    .is_err()
            );
            assert!(session.sdma_host_pool_limits.is_none());
        }
    }

    #[test]
    fn device_pool_default_preserves_recycle_checkout_and_host_cache_behavior() {
        let queue = test_queue_key(802, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        // No Linux owner is used: these legacy pool transitions require no native effect.
        session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
        let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 2);
        let identity = device.storage_identity();
        session.sdma_outstanding_buffers = 2;
        session.recycle_sdma_buffer(device).unwrap();
        session.recycle_sdma_buffer(host).unwrap();
        assert_eq!(session.sdma_device_pool_usage_v1().unwrap(), None);
        assert_eq!(session.sdma_host_pool_usage_v1().unwrap(), None);
        let recycled = session.allocate_sdma_pooled_device_buffer(16, 4).unwrap();
        assert_eq!(recycled.storage_identity(), identity);
        assert_eq!(recycled.pool_generation(), 2);
        assert_eq!(recycled.requested_bytes(), 16);
        assert_eq!(session.sdma_outstanding_buffers, 1);
        assert_eq!(session.sdma_pool_reuse_count, 1);
        assert_eq!(session.sdma_pool_free.len(), 1);
        session.sdma_device_pool.limits = Some(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap());
        let (_, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 3);
        session.sdma_outstanding_buffers += 1;
        session.recycle_sdma_buffer(host).unwrap();
        assert_eq!(session.sdma_pool_free.len(), 2);
        assert!(session.sdma_device_pool_usage_v1().is_err());
        // Missing native session cannot be mistaken for a configured zero usage result.
        assert!(!session.terminal_poisoned);
    }

    #[test]
    fn host_pool_configured_missing_owner_and_terminal_observation_are_not_empty_usage() {
        let queue = test_queue_key(803, 1);
        let limits = Gfx942HostPoolLimitsV1::new(4096, 1).unwrap();
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        assert_eq!(session.sdma_host_pool_usage_v1().unwrap(), None);
        assert!(session.configure_sdma_host_pool_v1(limits).is_err());
        assert!(session.sdma_host_pool_limits.is_none());
        session.sdma_host_pool_limits = Some(limits);
        assert!(session.sdma_host_pool_usage_v1().is_err());
        assert!(session.configure_sdma_host_pool_v1(limits).is_err());
        session.terminal_poisoned = true;
        assert!(session.sdma_host_pool_usage_v1().is_err());
        assert!(session.configure_sdma_host_pool_v1(limits).is_err());
        assert_eq!(session.sdma_host_pool_limits, Some(limits));
    }

    #[test]
    fn host_pool_invalid_native_roster_poison_precedes_checkout_or_trim_mutation() {
        for trim in [false, true] {
            let queue = test_queue_key(804, 1);
            let mut session = persistent_compute_cancellation_test_session(queue, None, None);
            session.sdma = Some(Gfx942SdmaQueueSetV1::Generic(Vec::new()));
            let (_, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 3);
            session.sdma_pool_free.push(host);
            session.sdma_host_pool_limits = Some(Gfx942HostPoolLimitsV1::new(4096, 1).unwrap());
            if trim {
                assert!(session.trim_sdma_memory_pool().is_err());
            } else {
                assert!(
                    session
                        .checkout_sdma_pool(Gfx942SdmaBufferKindV1::HostVisibleCoherent, 16, 1)
                        .is_err()
                );
            }
            assert!(session.terminal_poisoned);
            assert_eq!(session.sdma_pool_free.len(), 1);
            assert_eq!(session.sdma_outstanding_buffers, 0);
            assert_eq!(session.sdma_pool_reuse_count, 0);
        }
    }

    #[test]
    fn host_pool_linux_wiring_uses_exact_domain_before_recycle_checkout_trim() {
        // Static wiring complements the fake native record tests, not Linux acceptance.
        let source = include_str!("queue_live.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        let config = production
            .split_once("pub fn configure_sdma_host_pool_v1(")
            .unwrap()
            .1
            .split_once("pub fn sdma_host_pool_usage_v1(")
            .unwrap()
            .0;
        assert!(config.contains("self.sdma_device_pool.activity_started"));
        assert!(config.contains("validate_host_pool_domain_v1(self.key.vm)"));
        assert!(!config.contains("begin_activity()"));
        let recycle = production
            .split_once("pub fn recycle_sdma_buffer(")
            .unwrap()
            .1
            .split_once("pub fn trim_sdma_memory_pool(")
            .unwrap()
            .0;
        assert!(recycle.contains(
            "Ok(HostPoolDispositionV1::Dispose) => return self.release_sdma_buffer(buffer)"
        ));
        assert!(
            recycle.find("host_pool_recycle_decision_v1(").unwrap()
                < recycle.find("try_reserve(1)").unwrap()
        );
        for (function, mutation) in [
            ("fn checkout_sdma_pool(", "swap_remove("),
            ("pub fn trim_sdma_memory_pool(", ".pop()"),
        ] {
            let body = production.split_once(function).unwrap().1;
            assert!(
                body.find("self.validate_configured_host_pool_v1()?")
                    .unwrap()
                    < body.find(mutation).unwrap()
            );
        }
    }

    #[test]
    fn device_pool_linux_wiring_keeps_exact_domain_disposal_and_no_second_byte_ledger() {
        // Source wiring only; real records/account/loans/disposal use the fake backend tests.
        let source = include_str!("queue_live.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        let config = production
            .split("pub fn configure_sdma_device_pool_v1(")
            .nth(1)
            .unwrap()
            .split("/// Reports padded backing")
            .next()
            .unwrap();
        assert!(config.contains("validate_device_pool_domain_v1(self.key.vm)"));
        assert!(config.contains("self.sdma_device_pool.configure(limits)"));
        for forbidden in [
            "retained_device_memory",
            "take_queue_model",
            "check_currentness",
            "configure_device_backing_budget",
        ] {
            assert!(!config.contains(forbidden));
        }
        let recycle = production
            .split("pub fn recycle_sdma_buffer(")
            .nth(1)
            .unwrap()
            .split("pub fn trim_sdma_memory_pool(")
            .next()
            .unwrap();
        assert!(recycle.contains("device_pool_recycle_decision_v1("));
        assert!(recycle.contains(
            "Ok(DevicePoolDispositionV1::Dispose) => return self.release_sdma_buffer(buffer)"
        ));
        assert!(
            recycle.find("device_pool_recycle_decision_v1(").unwrap()
                < recycle.find("try_reserve(1)").unwrap()
        );
        for operation in ["fn checkout_sdma_pool(", "pub fn trim_sdma_memory_pool("] {
            let body = production.split(operation).nth(1).unwrap();
            let first_mutation = body
                .find(if operation.starts_with("fn checkout") {
                    "swap_remove("
                } else {
                    ".pop()"
                })
                .unwrap();
            assert!(
                body.find("self.validate_configured_device_pool_v1()?")
                    .unwrap()
                    < first_mutation
            );
        }
        let projection = include_str!("shared_memory.rs")
            .split("pub(crate) fn device_pool_backing_bytes_v1(")
            .nth(1)
            .unwrap()
            .split("pub fn retained_allocation_count")
            .next()
            .unwrap();
        assert!(!projection.contains("check_currentness"));
        assert!(!projection.contains("release_"));
    }

    #[test]
    fn backing_constructor_forwarding_precedes_prepare_and_keeps_process_vm_envelope() {
        // Linux acquisition cannot run in this CPU test; the shared-memory
        // tests execute the configuration and ownership helpers with a fake backend.
        let source = include_str!("queue_live.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        let constructor = production
            .split("fn create_compute_aql_queue_with_runtime<T>(")
            .nth(1)
            .unwrap()
            .split("/// Runs one fresh-queue")
            .next()
            .unwrap();
        let acquisition_call = ".acquire_shared_gtt_memory_session_with_backing_budgets_v1(";
        let acquire = constructor.find(acquisition_call).unwrap();
        let argument = constructor[acquire + acquisition_call.len()..]
            .split_once(')')
            .unwrap()
            .0
            .trim();
        let arguments: Vec<_> = argument
            .split(',')
            .map(str::trim)
            .filter(|argument| !argument.is_empty())
            .collect();
        assert_eq!(
            arguments,
            ["device_backing_budget", "host_visible_backing_budget"]
        );
        let root = constructor
            .find("PrimaryQueueConstructionV1::new(memory, None)")
            .unwrap();
        let prepare = constructor
            .find("capture_returned_preparation_v1(")
            .unwrap();
        let create = constructor.find("root.construct(").unwrap();
        assert!(acquire < root && root < prepare && prepare < create);
        assert!(production.contains(
            "self.create_compute_aql_queue_with_runtime(ring_bytes, prepare, None, None, None)"
        ));
        assert!(production.contains(
            "self.create_compute_aql_queue_with_backing_budgets_v1(ring_bytes, budget, None)"
        ));

        let shared = include_str!("shared_memory.rs");
        let acquisition = shared
            .split("pub fn acquire_shared_gtt_memory_session_with_backing_budgets_v1(")
            .nth(1)
            .unwrap()
            .split("impl SharedGttMemorySessionV1 {")
            .next()
            .unwrap();
        let begin = acquisition
            .find("begin_process_vm_attempt(pid, gpu_id)?")
            .unwrap();
        let bind = acquisition
            .find("engine.backend.bind_model_vm(VmIdV1(vm_id))?")
            .unwrap();
        let configure = acquisition
            .find(".configure_optional_device_backing_budget(")
            .unwrap();
        let configure_host = acquisition
            .find(".configure_optional_host_visible_backing_budget(")
            .unwrap();
        let finish = acquisition
            .find("finish_process_vm_attempt(result.is_ok(), pid, gpu_id)")
            .unwrap();
        assert!(
            begin < bind
                && bind < configure
                && configure < configure_host
                && configure_host < finish
        );
        assert!(shared.contains(
            "self.acquire_shared_gtt_memory_session_with_device_backing_budget_v1(None)"
        ));
        let usage = production
            .split("pub fn device_backing_usage_v1(")
            .nth(1)
            .unwrap()
            .split("/// Adds one generic")
            .next()
            .unwrap();
        assert!(usage.contains("engine.backend.session.device_backing_usage_v1()"));
        assert!(!usage.contains("check_currentness"));
        assert!(!usage.contains("with_live_queue_memory_model"));
    }

    #[test]
    fn production_dependency_owner_lane_envelope_and_teardown_shape_is_sealed() {
        let source = include_str!("queue_live.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        let fixed = include_str!("queue_live/fixed_dispatch.rs");
        let primary = include_str!("queue_live/construction_primary.rs");
        let environment = include_str!("queue_live/construction_primary/environment.rs");
        let auxiliary = include_str!("queue_live/construction_auxiliary.rs");
        assert!(!auxiliary.contains("ComputeDependencySessionOwnerV1::new("));
        assert!(!auxiliary.contains("create_dependency_owner("));
        assert_eq!(
            production
                .lines()
                .filter(|line| {
                    line.trim()
                        == "dependency_owner: QueueOwnerSlotV1<ComputeDependencySessionOwnerV1>,"
                })
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
                .count()
                + primary
                    .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
                    .count()
                + environment
                    .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
                    .count(),
            1
        );
        assert_eq!(
            primary.matches("E::create_dependency_owner(key)").count(),
            1
        );
        let dependency_constructor = environment
            .split("fn create_dependency_owner(")
            .nth(1)
            .unwrap()
            .split("fn ")
            .next()
            .unwrap();
        assert_eq!(
            dependency_constructor
                .matches("ComputeDependencySessionOwnerV1::new(key.id.0)")
                .count(),
            1
        );
        let lane_state = production
            .split("struct ComputeAqlQueueLaneStateV1")
            .nth(1)
            .unwrap()
            .split("fn prepare_auxiliary_compute_lane_slot_v1")
            .next()
            .unwrap();
        assert!(!lane_state.contains("ComputeDependencySessionOwnerV1"));

        let envelope = production
            .split("fn with_dependency_target_lane_v1<R>")
            .nth(1)
            .unwrap()
            .split("pub fn submit_fixed_dispatch_with_dependencies_v1")
            .next()
            .unwrap();
        assert_eq!(envelope.matches("std::panic::catch_unwind").count(), 3);
        assert_eq!(
            envelope
                .matches("std::panic::resume_unwind(payload)")
                .count(),
            3
        );
        assert!(envelope.contains("self.swap_primary_compute_lane(&mut displaced_primary)"));
        assert!(envelope.contains("source.completion_owner"));

        let submit = production
            .split("pub fn submit_fixed_dispatch_with_dependencies_v1")
            .nth(1)
            .unwrap()
            .split("fn cancel_dependency_dispatch_generation_v1")
            .next()
            .unwrap();
        assert!(submit.contains("with_dependency_target_lane_v1"));
        assert!(submit.contains("source_owner"));
        let poll = production
            .split("pub fn poll_compute_dependency_dispatch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn release_compute_dependency_event_v1")
            .next()
            .unwrap();
        assert!(poll.contains("with_dependency_target_lane_v1"));
        assert!(poll.contains("release_after_dependent_completion"));
        assert!(poll.contains("permanently_poison_process_global_kfd_runtime_gate_v1"));
        assert_eq!(production.matches(".ensure_idle()").count(), 2);

        let native_submit = include_str!("queue_submit.rs");
        let callback_catcher = native_submit
            .split("fn catch_dependency_callback<T>")
            .nth(1)
            .unwrap()
            .split("impl<B: NativeAqlSubmissionBackendV1>")
            .next()
            .unwrap();
        assert!(
            callback_catcher.contains("Err(_) => Err(NativeAqlSubmissionErrorV1::CallbackPanic)")
        );
        assert!(submit.contains("std::panic::resume_unwind(payload)"));
        assert!(submit.contains("ComputeDependencyPublicationFailureV1::Terminal(custody)"));
        assert!(submit.contains("Box::new(Gfx942ComputeDependencyDispatchV1"));
        assert!(poll.contains("dispatch: Box<Gfx942ComputeDependencyDispatchV1>"));
        assert!(
            poll.contains("dispatch") && poll.contains(".published") && poll.contains(".take()")
        );
        assert!(poll.contains("Gfx942ComputeDependencyPollV1::Pending(dispatch)"));
        assert!(!poll.contains("Box::new(Gfx942ComputeDependencyDispatchV1"));
        assert!(envelope.contains("AdmittedComputeLaneV1::Auxiliary(source_index)"));

        let source_submit = fixed
            .split("fn submit_fixed_dispatch_with_dependency_events_operation_v1")
            .nth(1)
            .unwrap()
            .split("fn submit_with_dependency_events_classified_v1")
            .next()
            .unwrap();
        assert!(source_submit.contains("GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1"));
        assert!(source_submit.contains("dependency source packet count must be 1 through 8192"));
        assert!(submit.contains("AQL_MAX_DEPENDENCY_SIGNALS_V1"));
        assert!(submit.contains("dependency roster must contain 1 through 256 events"));
        assert!(submit.contains("dependency_owner.ensure_target_capacity()"));
        assert_eq!(MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1, 128);

        let recycle = fixed
            .split("pub fn recycle_fixed_dispatch<const N: usize>")
            .nth(1)
            .unwrap()
            .split("pub fn recycled_fixed_dispatch_generation")
            .next()
            .unwrap();
        assert!(recycle.contains("Gfx942CompletionErrorV1::SignalPinned"));
        assert!(recycle.contains("retryable_completed: Some(wrap_completed"));
        assert!(recycle.contains("Gfx942DispatchBindingErrorV1::StaleDispatchGeneration"));
        assert!(recycle.contains("retryable_completed: None"));
    }

    #[test]
    fn nonpanic_retake_failure_requests_local_and_process_terminal_poison() {
        let mut poison = (false, false);
        let ((), retake) = execute_live_model_custody_v1(
            &mut poison,
            |_| Ok(()),
            |_| (),
            |_, ()| Err("injected retake failure"),
            |poison| *poison = (true, true),
        )
        .unwrap();
        assert_eq!(retake, Err("injected retake failure"));
        assert_eq!(poison, (true, true));
        let source = include_str!("queue_live.rs");
        let envelope = source
            .split("fn with_live_queue_memory_model_custody<R>")
            .nth(1)
            .unwrap()
            .split("fn with_sdma_queue_creation_custody_v1<R>")
            .next()
            .unwrap();
        assert!(envelope.contains("execute_live_model_custody_v1("));
        assert!(envelope.contains("session.poison_terminal()"));
        assert!(envelope.contains("permanently_poison_process_global_kfd_runtime_gate_v1()"));
    }

    #[test]
    fn destroy_revision_preflight_preserves_precondition_order_and_is_terminal() {
        let source = include_str!("queue_live.rs");
        let destroy = source
            .split("fn destroy_queue_and_event(")
            .nth(1)
            .unwrap()
            .split("fn complete_destroy<T>")
            .next()
            .unwrap();
        let releasable = destroy
            .find("self.completion_owner.ensure_releasable()")
            .unwrap();
        let mode = destroy
            .find("let (return_attached, detached_return) = match mode")
            .unwrap();
        let preflight = destroy
            .find("preflight_memory_transition_revisions(1)")
            .unwrap();
        let terminal = destroy[preflight..].find("self.poison_terminal()").unwrap() + preflight;
        let process = destroy[preflight..]
            .find("permanently_poison_process_global_kfd_runtime_gate_v1()")
            .unwrap()
            + preflight;
        let first_native = destroy.find("striped_sdma.destroy_queue(memory)").unwrap();
        assert!(releasable < mode);
        assert!(mode < preflight);
        assert!(preflight < terminal);
        assert!(terminal < process);
        assert!(process < first_native);
    }

    #[test]
    fn retake_failure_does_not_replace_the_original_panic_payload() {
        let mut poisoned = false;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute_live_model_custody_v1(
                &mut poisoned,
                |_| Ok(()),
                |_| -> () { std::panic::panic_any("original queue mutation panic") },
                |_, ()| Err("injected retake failure"),
                |poisoned| *poisoned = true,
            )
        }))
        .expect_err("panic resumption must escape the cleanup boundary");
        assert!(poisoned);
        assert_eq!(
            caught.downcast_ref::<&str>(),
            Some(&"original queue mutation panic")
        );
        let helper = include_str!("queue_live/model_loan.rs");
        let operation = helper
            .find("catch_unwind(AssertUnwindSafe(|| operation(context)))")
            .unwrap();
        let retake = helper
            .find("catch_unwind(AssertUnwindSafe(|| retake(context, loan)))")
            .unwrap();
        let poison = helper[retake..].find("poison(context)").unwrap() + retake;
        let retain_secondary = helper.find("core::mem::forget(closing)").unwrap();
        assert!(operation < retake && retake < poison && poison < retain_secondary);

        let bind = include_str!("queue_live/fixed_dispatch.rs")
            .split("pub fn bind_directional_persistent_fixed_dispatch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn submit_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        let panic_arm = bind
            .split("let prepared_dispatch = match prepared_dispatch")
            .nth(1)
            .unwrap()
            .split("let prepared_dispatch = match prepared_dispatch")
            .next()
            .unwrap();
        assert!(panic_arm.contains("quarantine_persistent_retained_control_replay_prepared_v1"));
        assert!(panic_arm.contains("state,"));
        assert!(!panic_arm.contains("state: PersistentComputeUseStateV1::Quarantined"));
    }

    #[test]
    fn every_live_foundation_mutation_and_moved_owner_uses_the_unwind_envelope() {
        let source = include_str!("queue_live.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        assert_eq!(
            production
                .matches("restore_model_ownership_for_live_mutation()")
                .count(),
            1
        );
        assert_eq!(
            production
                .matches("retake_model_ownership_after_live_mutation(loan)")
                .count(),
            1
        );

        let auxiliary = production
            .split("pub fn create_auxiliary_compute_lane_with_fixed_dispatch")
            .nth(1)
            .unwrap()
            .split("pub fn destroy_auxiliary_compute_lane_v1")
            .next()
            .unwrap();
        assert!(auxiliary.contains("construction_auxiliary::construct_auxiliary_compute_lane_v1("));
        let construction = include_str!("queue_live/construction_auxiliary.rs");
        let envelope = construction
            .find("with_preparation_custody(|memory|")
            .unwrap();
        assert!(
            include_str!("queue_live/construction_auxiliary/parent.rs")
                .contains("self.with_live_queue_memory_model_custody(work)")
        );
        let callback = construction.find("root.prepare_dispatch(").unwrap();
        assert!(envelope < callback);
        let preparation = construction
            .split("pub(super) fn prepare_dispatch(")
            .nth(1)
            .unwrap()
            .split("fn prepare(")
            .next()
            .unwrap();
        let capture = preparation
            .find("capture_returned_preparation_v1(memory, &mut self.data, prepare_data)")
            .unwrap();
        let retained = preparation.find("self.preparation = Some(").unwrap();
        assert!(capture < retained);

        for wrapper in [
            "fn with_sdma_owner_memory<R>",
            "fn with_striped_sdma_owner_memory<R>",
        ] {
            let body = production
                .split(wrapper)
                .nth(1)
                .unwrap()
                .split("\n    fn ")
                .next()
                .unwrap();
            assert!(body.contains("std::panic::catch_unwind"));
            assert!(body.contains("self.with_live_queue_memory_model"));
            assert!(body.contains("std::panic::resume_unwind(payload)"));
        }
    }

    #[test]
    fn destroyed_queue_observation_counts_base_and_optional_sdma_resources() {
        let without_sdma = destroyed_queue_observation(17);
        assert_eq!(without_sdma.queue_id(), 17);
        assert_eq!(
            without_sdma.released_resources(),
            GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1
        );

        let with_sdma = destroyed_queue_observation_with_additional_resources(17, 3);
        assert_eq!(with_sdma.queue_id(), 17);
        assert_eq!(
            with_sdma.released_resources(),
            GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1 + 3
        );
    }

    pub(super) fn test_queue_key(queue: u64, generation: u64) -> QueueKeyV1 {
        QueueKeyV1 {
            vm: fe2o3_runtime_model::VmKeyV1 {
                device: fe2o3_runtime_model::DeviceKeyV1 {
                    physical: fe2o3_runtime_model::PhysicalDeviceIdV1(7),
                    generation: fe2o3_runtime_model::DeviceGenerationV1(11),
                },
                id: fe2o3_runtime_model::VmIdV1(13),
            },
            id: QueueInstanceIdV1(queue),
            generation: QueueGenerationV1(generation),
        }
    }

    fn test_completion_mapping(
        queue: QueueKeyV1,
        id: u64,
    ) -> fe2o3_runtime_model::MemoryMappingKeyV1 {
        fe2o3_runtime_model::MemoryMappingKeyV1 {
            allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
                vm: queue.vm,
                id: fe2o3_runtime_model::AllocationIdV1(id),
                generation: fe2o3_runtime_model::AllocationGenerationV1(1),
            },
            id: fe2o3_runtime_model::MappingIdV1(id),
        }
    }

    fn test_completion_template(
        queue: QueueKeyV1,
        dispatch_generation: u64,
    ) -> CompletionPacketTemplateV1 {
        CompletionPacketTemplateV1::new(
            fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior,
            0,
            0,
            fe2o3_aql::ObservedGpuAddressV1::new(0x40_0000).unwrap(),
            fe2o3_aql::ObservedGpuAddressV1::new(0x50_0000).unwrap(),
            16,
            super::super::completion::CompletionDispatchGenerationBindingV1::new(
                queue,
                test_completion_mapping(queue, 30),
                test_completion_mapping(queue, 31),
                dispatch_generation,
            ),
        )
    }

    fn persistent_restore_fixture(
        direction: Gfx942PersistentSdmaDirectionV1,
        id: u64,
    ) -> (
        Gfx942QueuePersistentAllocationV1,
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        Gfx942SdmaCopyRequestV1,
        Gfx942PersistentSdmaHostBindingV1,
    ) {
        let queue = test_queue_key(21, 1);
        let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, id);
        let host_binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, queue);
        let storage_identity = device.storage_identity();
        let (storage, _, pool_generation, logical_bytes) = device.into_bridge_parts();
        let Gfx942SdmaBufferStorageV1::Device(lease) = storage else {
            unreachable!()
        };
        let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(lease);
        let operation = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let reserved = owner
            .reserve(
                Gfx942PersistentUseRequestV1::new(operation, 16, 32).unwrap(),
                None,
            )
            .unwrap();
        let prepared = owner.prepare(reserved).unwrap();
        let lease = owner.detach_local_native_for_sdma().unwrap();
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            queue,
            pool_generation,
            logical_bytes,
        );
        (
            Gfx942QueuePersistentAllocationV1 {
                owner,
                attachment: Gfx942PersistentSdmaAttachmentV1 {
                    queue,
                    native_queue_id: 17,
                    engine_index: direction.engine_index(),
                    pool_generation,
                    logical_bytes,
                    physical_bytes: logical_bytes,
                    storage_identity,
                },
            },
            prepared,
            persistent_sdma_request(direction, host, 8, device, 16, 32),
            host_binding,
        )
    }

    fn persistent_prepared_custody_fixture(
        direction: Gfx942PersistentSdmaDirectionV1,
        id: u64,
    ) -> (
        PersistentSdmaPreparedCustodyV1,
        Gfx942SdmaCopyRequestV1,
        Gfx942SdmaCopyTicketV1,
    ) {
        let (allocation, prepared, request, host_binding) =
            persistent_restore_fixture(direction, id);
        let ticket = crate::sdma::persistent_sdma_ticket_for_test(
            allocation.attachment.queue,
            allocation.attachment.native_queue_id,
        );
        (
            PersistentSdmaPreparedCustodyV1 {
                allocation,
                prepared,
                planned_ticket: ticket,
                host_binding,
                direction,
                host_offset: 8,
                device_offset: 16,
                copy_bytes: 32,
            },
            request,
            ticket,
        )
    }

    fn persistent_published_custody_fixture(
        direction: Gfx942PersistentSdmaDirectionV1,
        id: u64,
    ) -> (Gfx942PersistentSdmaSubmissionV1, Gfx942SdmaCopyRequestV1) {
        let (custody, request, ticket) = persistent_prepared_custody_fixture(direction, id);
        let transition = transition_persistent_sdma_publication_v1(
            custody,
            PersistentSdmaPublicationObservationV1::Confirmed(ticket),
            true,
            true,
        );
        let PersistentSdmaPublicationTransitionV1::Published(submission) = transition else {
            panic!("exact confirmed publication must publish")
        };
        (submission, request)
    }

    fn prepare_restored_persistent_custody(
        mut allocation: Gfx942QueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
    ) -> (
        PersistentSdmaPreparedCustodyV1,
        Gfx942SdmaCopyRequestV1,
        Gfx942SdmaCopyTicketV1,
    ) {
        let direction = allocation.direction();
        let operation = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentOperationV1::LocalSdmaDestination
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentOperationV1::LocalSdmaSource
            }
        };
        let reserved = allocation
            .owner
            .reserve(
                Gfx942PersistentUseRequestV1::new(operation, 16, 32).unwrap(),
                dependency,
            )
            .unwrap();
        let prepared = allocation.owner.prepare(reserved).unwrap();
        let lease = allocation.owner.detach_local_native_for_sdma().unwrap();
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            allocation.attachment.queue,
            allocation.attachment.pool_generation,
            allocation.attachment.logical_bytes,
        );
        let request = persistent_sdma_request(direction, host, 8, device, 16, 32);
        let host_binding = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                Gfx942PersistentSdmaHostBindingV1::capture(
                    &request.source,
                    allocation.attachment.queue,
                )
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                Gfx942PersistentSdmaHostBindingV1::capture(
                    &request.destination,
                    allocation.attachment.queue,
                )
            }
        };
        let ticket = crate::sdma::persistent_sdma_ticket_for_test(
            allocation.attachment.queue,
            allocation.attachment.native_queue_id,
        );
        (
            PersistentSdmaPreparedCustodyV1 {
                allocation,
                prepared,
                planned_ticket: ticket,
                host_binding,
                direction,
                host_offset: 8,
                device_offset: 16,
                copy_bytes: 32,
            },
            request,
            ticket,
        )
    }

    fn completed_persistent_request(request: Gfx942SdmaCopyRequestV1) -> Gfx942SdmaCompletedCopyV1 {
        let Gfx942SdmaCopyRequestV1 {
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
        } = request;
        Gfx942SdmaCompletedCopyV1 {
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
        }
    }

    #[test]
    fn persistent_sdma_request_restoration_is_exact_in_both_directions() {
        for (ordinal, direction) in [
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        ]
        .into_iter()
        .enumerate()
        {
            let (allocation, prepared, request, host_binding) =
                persistent_restore_fixture(direction, 80 + ordinal as u64);
            let (mut allocation, host) = match restore_persistent_sdma_request(
                allocation,
                direction,
                8,
                16,
                32,
                host_binding,
                request,
            ) {
                Ok(restored) => restored,
                Err(_) => panic!("exact persistent request must restore"),
            };
            assert!(allocation.owner.local_native_is_attached_for_sdma());
            assert_eq!(host.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
            allocation.owner.cancel_prepared(prepared).unwrap();
        }
    }

    #[test]
    fn persistent_sdma_request_restoration_rejects_generation_substitution() {
        let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
        let (mut allocation, prepared, request, host_binding) =
            persistent_restore_fixture(direction, 82);
        allocation.attachment.pool_generation += 1;
        let (mut allocation, request) = restore_persistent_sdma_request(
            allocation,
            direction,
            8,
            16,
            32,
            host_binding,
            request,
        )
        .unwrap_err();
        assert!(!allocation.owner.local_native_is_attached_for_sdma());
        let (source, destination) = request.into_buffers();
        assert_eq!(source.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
        assert_eq!(destination.kind(), Gfx942SdmaBufferKindV1::DeviceLocal);
        allocation.owner.cancel_prepared(prepared).unwrap();
    }

    #[test]
    fn recoverable_persistent_publication_restores_exact_owners_without_teardown() {
        let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
        let (custody, request, _ticket) = persistent_prepared_custody_fixture(direction, 83);
        let expected_request = custody.prepared.request();
        let expected_attachment = custody.allocation.attachment;
        let expected_host = request.source.storage_identity();
        let transition = transition_persistent_sdma_publication_v1(
            custody,
            PersistentSdmaPublicationObservationV1::Recoverable(request),
            true,
            true,
        );
        let PersistentSdmaPublicationTransitionV1::Retryable { allocation, host } = transition
        else {
            panic!("clean lower rejection must remain retryable without queue teardown")
        };
        assert_eq!(allocation.attachment, expected_attachment);
        assert_eq!(host.storage_identity(), expected_host);
        assert!(allocation.owner.local_native_is_attached_for_sdma());
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert_eq!(allocation.owner.quarantine_reason(), None);
        assert_eq!(
            expected_request,
            Gfx942PersistentUseRequestV1::new(
                Gfx942PersistentOperationV1::LocalSdmaDestination,
                16,
                32,
            )
            .unwrap()
        );
    }

    #[test]
    fn retained_persistent_publication_quarantines_directly_from_prepared() {
        let (custody, _request, ticket) =
            persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 84);
        let sequence = custody.prepared.sequence();
        let transition = transition_persistent_sdma_publication_v1(
            custody,
            PersistentSdmaPublicationObservationV1::Retained(ticket),
            true,
            false,
        );
        let PersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) = transition else {
            panic!("retained lower publication must require process teardown")
        };
        assert_eq!(custody.sequence(), Some(sequence));
        assert_eq!(
            custody.stage(),
            crate::Gfx942PersistentSdmaTerminalStageV1::PreparedQueueRetained
        );
        let Gfx942PersistentSdmaTerminalStateV1::PreparedQueueRetained {
            allocation,
            ticket: retained_ticket,
        } = custody.state
        else {
            unreachable!()
        };
        assert_eq!(retained_ticket, ticket);
        assert_eq!(
            allocation.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
        );
    }

    #[test]
    fn confirmed_persistent_publication_records_the_exact_ticket() {
        let (custody, _request, ticket) =
            persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 85);
        let sequence = custody.prepared.sequence();
        let expected_request = custody.prepared.request();
        let transition = transition_persistent_sdma_publication_v1(
            custody,
            PersistentSdmaPublicationObservationV1::Confirmed(ticket),
            true,
            true,
        );
        let PersistentSdmaPublicationTransitionV1::Published(submission) = transition else {
            panic!("confirmed exact publication must produce published custody")
        };
        assert_eq!(submission.ticket, ticket);
        assert_eq!(submission.published.sequence(), sequence);
        assert_eq!(submission.request(), expected_request);
        assert_eq!(submission.allocation.owner.live_use_count(), 1);
        assert_eq!(submission.allocation.owner.quarantine_reason(), None);
    }

    #[test]
    fn confirmed_persistent_publication_rejects_same_queue_ticket_substitution() {
        for (ordinal, slot, generation) in [(0_u64, 1_u16, 1_u32), (1, 0, 2)] {
            let (custody, _request, planned_ticket) = persistent_prepared_custody_fixture(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                95 + ordinal,
            );
            let substituted_ticket = crate::sdma::persistent_sdma_ticket_coordinates_for_test(
                custody.allocation.attachment.queue,
                custody.allocation.attachment.native_queue_id,
                slot,
                generation,
            );
            assert_ne!(substituted_ticket, planned_ticket);
            let transition = transition_persistent_sdma_publication_v1(
                custody,
                PersistentSdmaPublicationObservationV1::Confirmed(substituted_ticket),
                true,
                true,
            );
            let PersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) = transition else {
                panic!("same-queue ticket substitution must require process teardown")
            };
            assert_eq!(
                custody.stage(),
                crate::Gfx942PersistentSdmaTerminalStageV1::PublishedQueueRetained
            );
            let Gfx942PersistentSdmaTerminalStateV1::PublishedQueueRetained { allocation, ticket } =
                custody.state
            else {
                unreachable!()
            };
            assert_eq!(ticket, substituted_ticket);
            assert_eq!(
                allocation.owner.quarantine_reason(),
                Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
            );
        }
    }

    #[test]
    fn pending_and_timeout_keep_the_exact_published_submission() {
        let (pending, _request) =
            persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 86);
        let pending_ticket = pending.ticket;
        let pending_sequence = pending.published.sequence();
        let PersistentSdmaCompletionTransitionV1::Pending(pending) =
            transition_persistent_sdma_completion_v1(
                pending,
                PersistentSdmaCompletionObservationV1::Pending,
                true,
            )
        else {
            panic!("pending observation must preserve published custody")
        };
        assert_eq!(pending.ticket, pending_ticket);
        assert_eq!(pending.published.sequence(), pending_sequence);

        let (timeout, _request) =
            persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 87);
        let timeout_ticket = timeout.ticket;
        let timeout_sequence = timeout.published.sequence();
        let PersistentSdmaCompletionTransitionV1::Timeout(timeout) =
            transition_persistent_sdma_completion_v1(
                timeout,
                PersistentSdmaCompletionObservationV1::Timeout,
                true,
            )
        else {
            panic!("timeout observation must preserve published custody")
        };
        assert_eq!(timeout.ticket, timeout_ticket);
        assert_eq!(timeout.published.sequence(), timeout_sequence);
        assert_eq!(timeout.allocation.owner.live_use_count(), 1);
    }

    #[test]
    fn exact_persistent_completion_restores_both_owners_and_frontier() {
        let (submission, request) =
            persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::DeviceToHost, 88);
        let sequence = submission.published.sequence();
        let expected_attachment = submission.allocation.attachment;
        let expected_host = request.destination.storage_identity();
        let transition = transition_persistent_sdma_completion_v1(
            submission,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
            true,
        );
        let PersistentSdmaCompletionTransitionV1::Completed(completed) = transition else {
            panic!("exact completion must restore and settle custody")
        };
        assert_eq!(
            completed.direction(),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost
        );
        assert_eq!(completed.copy_bytes(), 32);
        let (allocation, host, frontier) = completed.into_parts();
        assert_eq!(allocation.attachment, expected_attachment);
        assert_eq!(host.storage_identity(), expected_host);
        assert!(allocation.owner.local_native_is_attached_for_sdma());
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert_eq!(allocation.owner.retained_settled_use_count(), 1);
        assert_eq!(frontier.through_sequence(), sequence);
    }

    #[test]
    fn exact_persistent_completion_rejects_same_queue_host_substitution() {
        let (submission, request) =
            persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 97);
        let queue = submission.allocation.attachment.queue;
        let Gfx942SdmaCopyRequestV1 {
            source: original_host,
            source_offset,
            destination: exact_device,
            destination_offset,
            copy_bytes,
        } = request;
        let (_unused_device, substituted_host) =
            crate::sdma::persistent_sdma_buffers_for_test(queue, 98);
        assert_ne!(
            original_host.storage_identity(),
            substituted_host.storage_identity()
        );
        let completed = Gfx942SdmaCompletedCopyV1 {
            source: substituted_host,
            source_offset,
            destination: exact_device,
            destination_offset,
            copy_bytes,
        };
        let transition = transition_persistent_sdma_completion_v1(
            submission,
            PersistentSdmaCompletionObservationV1::Completed(completed),
            true,
        );
        let PersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) = transition else {
            panic!("same-queue host substitution must require process teardown")
        };
        assert_eq!(
            custody.stage(),
            crate::Gfx942PersistentSdmaTerminalStageV1::CompletedUnrestored
        );
        let Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
            allocation,
            completed: _,
        } = custody.state
        else {
            unreachable!()
        };
        assert_eq!(
            allocation.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
        );
    }

    #[test]
    fn persistent_host_binding_rejects_same_storage_descriptor_substitution() {
        let queue = test_queue_key(21, 1);
        let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 99);
        drop(device);
        let binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, queue);
        let logical_bytes = host.requested_bytes();
        let (storage, owner, pool_generation, recovered_logical_bytes) = host.into_bridge_parts();
        assert_eq!(logical_bytes, recovered_logical_bytes);
        let substituted_generation = Gfx942SdmaBufferV1::from_bridge_parts(
            storage,
            owner,
            pool_generation + 1,
            logical_bytes,
        );
        assert!(!binding.matches(&substituted_generation));

        let (device, host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 100);
        drop(device);
        let binding = Gfx942PersistentSdmaHostBindingV1::capture(&host, queue);
        let (storage, owner, pool_generation, logical_bytes) = host.into_bridge_parts();
        let substituted_extent = Gfx942SdmaBufferV1::from_bridge_parts(
            storage,
            owner,
            pool_generation,
            logical_bytes / 2,
        );
        assert!(!binding.matches(&substituted_extent));
    }

    #[test]
    fn completed_observation_with_lost_currentness_is_terminal_and_unrestored() {
        let (submission, request) =
            persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 89);
        let sequence = submission.published.sequence();
        let transition = transition_persistent_sdma_completion_v1(
            submission,
            PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(request)),
            false,
        );
        let PersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) = transition else {
            panic!("completion outside the currentness envelope must be terminal")
        };
        assert_eq!(custody.sequence(), Some(sequence));
        assert_eq!(
            custody.stage(),
            crate::Gfx942PersistentSdmaTerminalStageV1::CompletedUnrestored
        );
        let Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
            allocation,
            completed: _,
        } = custody.state
        else {
            unreachable!()
        };
        assert_eq!(
            allocation.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
        );
    }

    #[test]
    fn persistent_demotion_advances_pool_generation_without_changing_buffer_debit() {
        let (custody, request, _ticket) =
            persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 90);
        let expected_identity = custody.allocation.attachment.storage_identity;
        let expected_generation = custody.allocation.attachment.pool_generation + 1;
        let transition = transition_persistent_sdma_publication_v1(
            custody,
            PersistentSdmaPublicationObservationV1::Recoverable(request),
            true,
            true,
        );
        let PersistentSdmaPublicationTransitionV1::Retryable {
            allocation,
            host: _,
        } = transition
        else {
            panic!("clean rejection must restore quiescent allocation custody")
        };
        let (buffer, outstanding_buffers) =
            demote_persistent_sdma_custody_v1(allocation, 2).unwrap();
        assert_eq!(outstanding_buffers, 2);
        assert_eq!(buffer.kind(), Gfx942SdmaBufferKindV1::DeviceLocal);
        assert_eq!(buffer.storage_identity(), expected_identity);
        assert_eq!(buffer.pool_generation(), expected_generation);
    }

    #[test]
    fn demote_repromote_rejects_frontier_aba_for_the_same_native_allocation() {
        let (submission, request) = persistent_published_custody_fixture(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            101,
        );
        let PersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_persistent_sdma_completion_v1(
                submission,
                PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                    request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (allocation, host, old_frontier) = completed.into_parts();
        let old_sequence = old_frontier.through_sequence();
        let old_identity = allocation.attachment.storage_identity;
        let old_pool_generation = allocation.attachment.pool_generation;
        let (device, outstanding_buffers) =
            demote_persistent_sdma_custody_v1(allocation, 2).unwrap();
        assert_eq!(outstanding_buffers, 2);
        assert_eq!(device.storage_identity(), old_identity);
        assert_eq!(device.pool_generation(), old_pool_generation + 1);

        let allocation = promote_persistent_sdma_custody_v1(
            device,
            17,
            Gfx942PersistentSdmaDirectionV1::HostToDevice.engine_index(),
        )
        .expect("the same device buffer must re-promote");
        assert_eq!(allocation.attachment.storage_identity, old_identity);
        let (prepared, request, ticket) =
            prepare_restored_persistent_custody(allocation, host, None);
        let PersistentSdmaPublicationTransitionV1::Published(submission) =
            transition_persistent_sdma_publication_v1(
                prepared,
                PersistentSdmaPublicationObservationV1::Confirmed(ticket),
                true,
                true,
            )
        else {
            unreachable!()
        };
        let PersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_persistent_sdma_completion_v1(
                submission,
                PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                    request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (mut allocation, _host, new_frontier) = completed.into_parts();
        assert_eq!(new_frontier.through_sequence(), old_sequence);

        let rejected = allocation
            .owner
            .reserve(
                Gfx942PersistentUseRequestV1::new(
                    Gfx942PersistentOperationV1::LocalSdmaDestination,
                    16,
                    32,
                )
                .unwrap(),
                Some(&old_frontier),
            )
            .expect_err("an old incarnation frontier cannot order new history");
        assert_eq!(
            rejected.error(),
            Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency
        );
        let failure = allocation
            .retire_settled_frontier_v1(old_frontier)
            .expect_err("an old incarnation frontier cannot retire new history");
        let (allocation, _returned_old_frontier) = failure.into_parts();
        let allocation = allocation
            .retire_settled_frontier_v1(new_frontier)
            .expect("the exact new incarnation frontier must retire new history");
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn frontier_retirement_supports_more_than_the_bounded_ledger_sequential_uses() {
        let (mut prepared, mut request, mut ticket) =
            persistent_prepared_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 91);
        let expected_attachment = prepared.allocation.attachment;
        for cycle in 0..(crate::GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 + 2) {
            let publication = transition_persistent_sdma_publication_v1(
                prepared,
                PersistentSdmaPublicationObservationV1::Confirmed(ticket),
                true,
                true,
            );
            let PersistentSdmaPublicationTransitionV1::Published(submission) = publication else {
                panic!("cycle {cycle} must publish")
            };
            let completion = transition_persistent_sdma_completion_v1(
                submission,
                PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                    request,
                )),
                true,
            );
            let PersistentSdmaCompletionTransitionV1::Completed(completed) = completion else {
                panic!("cycle {cycle} must complete")
            };
            let (allocation, host, frontier) = completed.into_parts();
            assert_eq!(allocation.owner.retained_settled_use_count(), 1);
            let allocation = allocation
                .retire_settled_frontier_v1(frontier)
                .unwrap_or_else(|_| panic!("cycle {cycle} exact frontier must retire"));
            assert_eq!(allocation.owner.retained_settled_use_count(), 0);
            assert_eq!(allocation.attachment, expected_attachment);
            if cycle + 1 == crate::GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 + 2 {
                break;
            }
            (prepared, request, ticket) =
                prepare_restored_persistent_custody(allocation, host, None);
        }
    }

    #[test]
    fn stale_frontier_retirement_returns_exact_custody_and_current_frontier_still_retires() {
        let (first, first_request) =
            persistent_published_custody_fixture(Gfx942PersistentSdmaDirectionV1::HostToDevice, 92);
        let PersistentSdmaCompletionTransitionV1::Completed(first) =
            transition_persistent_sdma_completion_v1(
                first,
                PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                    first_request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (allocation, host, stale_frontier) = first.into_parts();
        let stale_sequence = stale_frontier.through_sequence();
        let (second, second_request, second_ticket) =
            prepare_restored_persistent_custody(allocation, host, Some(&stale_frontier));
        let PersistentSdmaPublicationTransitionV1::Published(second) =
            transition_persistent_sdma_publication_v1(
                second,
                PersistentSdmaPublicationObservationV1::Confirmed(second_ticket),
                true,
                true,
            )
        else {
            unreachable!()
        };
        let PersistentSdmaCompletionTransitionV1::Completed(second) =
            transition_persistent_sdma_completion_v1(
                second,
                PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                    second_request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (allocation, _host, current_frontier) = second.into_parts();
        assert!(current_frontier.through_sequence() > stale_sequence);
        let failure = allocation
            .retire_settled_frontier_v1(stale_frontier)
            .expect_err("stale frontier must return both move-only inputs");
        let (allocation, returned_stale) = failure.into_parts();
        assert_eq!(returned_stale.through_sequence(), stale_sequence);
        assert_eq!(allocation.owner.retained_settled_use_count(), 2);
        let allocation = allocation
            .retire_settled_frontier_v1(current_frontier)
            .expect("exact latest frontier retires all settled history");
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn substituted_frontier_retirement_returns_both_allocation_and_frontier() {
        fn complete_one(
            direction: Gfx942PersistentSdmaDirectionV1,
            id: u64,
        ) -> (
            Gfx942QueuePersistentAllocationV1,
            Gfx942PersistentDependencyFrontierV1,
        ) {
            let (submission, request) = persistent_published_custody_fixture(direction, id);
            let PersistentSdmaCompletionTransitionV1::Completed(completed) =
                transition_persistent_sdma_completion_v1(
                    submission,
                    PersistentSdmaCompletionObservationV1::Completed(completed_persistent_request(
                        request,
                    )),
                    true,
                )
            else {
                unreachable!()
            };
            let (allocation, _host, frontier) = completed.into_parts();
            (allocation, frontier)
        }

        let (allocation_a, frontier_a) =
            complete_one(Gfx942PersistentSdmaDirectionV1::HostToDevice, 93);
        let (allocation_b, frontier_b) =
            complete_one(Gfx942PersistentSdmaDirectionV1::HostToDevice, 94);
        let frontier_b_sequence = frontier_b.through_sequence();
        let failure = allocation_a
            .retire_settled_frontier_v1(frontier_b)
            .expect_err("another allocation's frontier must be rejected");
        let (allocation_a, returned_frontier_b) = failure.into_parts();
        assert_eq!(returned_frontier_b.through_sequence(), frontier_b_sequence);
        let _allocation_a = allocation_a
            .retire_settled_frontier_v1(frontier_a)
            .expect("allocation A still accepts its exact frontier");
        let _allocation_b = allocation_b
            .retire_settled_frontier_v1(returned_frontier_b)
            .expect("allocation B accepts its returned exact frontier");
    }

    #[test]
    fn auxiliary_destroy_accepts_fully_released_detached_lane() {
        let queue = test_queue_key(201, 1);
        for insertion in [None, Some(0)] {
            let mut lane = compute_lane_state_for_multi_inflight_test(queue);
            lane.detached_dispatch_generation = Some(64);
            lane.detached_next_insertion_index = insertion;
            let mut state = Some(lane);
            let released = take_after_auxiliary_destroy_preflight_v1(
                &mut state,
                preflight_auxiliary_compute_lane_destroy_v1,
            )
            .expect("completed detached lane has no remaining data custody");
            assert!(state.is_none());
            assert!(released.dispatch.is_none());
            assert_eq!(released.detached_dispatch_generation, Some(64));
            assert_eq!(released.detached_next_insertion_index, insertion);
        }
    }

    #[test]
    fn auxiliary_destroy_rejects_live_or_malformed_detached_ledger_without_taking_custody() {
        let queue = test_queue_key(202, 1);
        let (device, _host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 0x6200);
        let Gfx942SdmaBufferStorageIdentityV1::Device(identity) = device.storage_identity() else {
            unreachable!("device fixture retains device storage")
        };
        let identity = Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(identity);
        for (generation, count, identities, insertion) in [
            (None, 0, vec![], None),
            (Some(64), 1, vec![identity], None),
            (Some(64), 1, vec![], None),
            (Some(64), 0, vec![identity], Some(0)),
            (Some(64), 0, vec![], Some(1)),
            (Some(0), 0, vec![], None),
        ] {
            let mut lane = compute_lane_state_for_multi_inflight_test(queue);
            lane.detached_dispatch_generation = generation;
            lane.detached_data_count = count;
            lane.detached_data_identities = identities.clone();
            lane.detached_next_insertion_index = insertion;
            let completion_before = lane.completion_owner.state_snapshot_for_test();
            let mut state = Some(lane);
            assert!(
                take_after_auxiliary_destroy_preflight_v1(
                    &mut state,
                    preflight_auxiliary_compute_lane_destroy_v1,
                )
                .is_err()
            );
            let retained = state.as_ref().expect("rejected preflight retains the lane");
            assert_eq!(retained.key, queue);
            assert_eq!(retained.detached_dispatch_generation, generation);
            assert_eq!(retained.detached_data_count, count);
            assert_eq!(retained.detached_data_identities, identities);
            assert_eq!(retained.detached_next_insertion_index, insertion);
            assert_eq!(
                retained.completion_owner.state_snapshot_for_test(),
                completion_before
            );
        }
    }

    #[test]
    fn auxiliary_destroy_rejects_live_completion_then_accepts_exact_cancellation() {
        let queue = test_queue_key(203, 1);
        let mut lane = compute_lane_state_for_multi_inflight_test(queue);
        lane.detached_dispatch_generation = Some(64);
        lane.detached_next_insertion_index = Some(0);
        let bound = lane
            .completion_owner
            .bind_batch([test_completion_template(queue, 65)])
            .unwrap();
        let (_, retention) = bound.into_parts();
        let before = lane.completion_owner.state_snapshot_for_test();
        let mut state = Some(lane);
        assert!(matches!(
            take_after_auxiliary_destroy_preflight_v1(
                &mut state,
                preflight_auxiliary_compute_lane_destroy_v1,
            ),
            Err(ComputeAqlQueueSessionErrorV1::Completion(_))
        ));
        let retained = state
            .as_mut()
            .expect("live completion retains lane custody");
        assert_eq!(retained.completion_owner.state_snapshot_for_test(), before);
        retained.completion_owner.cancel_bound(retention).unwrap();
        assert!(
            take_after_auxiliary_destroy_preflight_v1(
                &mut state,
                preflight_auxiliary_compute_lane_destroy_v1,
            )
            .is_ok()
        );
        assert!(state.is_none());
    }

    #[test]
    fn auxiliary_destroy_preflight_rejection_preserves_custody_for_retry() {
        struct TestLane {
            leased: bool,
        }

        let mut state = Some(TestLane { leased: true });
        let rejected = take_after_auxiliary_destroy_preflight_v1(&mut state, |lane| {
            if lane.leased {
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "injected live completion lease",
                ))
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            rejected,
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "injected live completion lease"
            ))
        ));
        assert!(state.as_ref().is_some_and(|lane| lane.leased));

        state.as_mut().unwrap().leased = false;
        let released = take_after_auxiliary_destroy_preflight_v1(&mut state, |_| Ok(())).unwrap();
        assert!(!released.leased);
        assert!(state.is_none());
    }

    #[test]
    fn combined_and_standalone_striped_owners_route_without_aliasing() {
        let live = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let combined = live
            .split("pub fn enable_gfx942_directional_and_striped_sdma_copy_engines_v1")
            .nth(1)
            .unwrap()
            .split("pub fn allocate_sdma_host_buffer")
            .next()
            .unwrap();
        let create = combined
            .find("with_sdma_queue_creation_custody_v1")
            .unwrap();
        assert!(create < combined.find("self.sdma = Some(directional)").unwrap());
        assert!(create < combined.find("self.striped_sdma = Some(striped)").unwrap());
        assert!(combined.contains("combined_striped_sdma_queue_count_is_admitted"));

        let routing = live
            .split("fn with_striped_sdma_owner_memory<R>")
            .nth(1)
            .unwrap()
            .split("fn with_device_buffer_pool")
            .next()
            .unwrap();
        assert!(routing.contains("let separate = self.striped_sdma.is_some()"));
        assert!(routing.contains("self.striped_sdma.take()"));
        assert!(routing.contains("self.sdma.take()"));
        assert!(routing.contains("self.striped_sdma = Some(owner)"));
        assert!(routing.contains("self.sdma = Some(owner)"));
    }

    #[test]
    fn session_owned_queue_id_roster_rejects_primary_auxiliary_and_sdma_collisions() {
        let mut roster = Vec::with_capacity(4);
        assert!(push_unique_queue_id_v1(&mut roster, 7));
        assert!(push_unique_queue_id_v1(&mut roster, 11));
        assert!(!push_unique_queue_id_v1(&mut roster, 7));
        assert_eq!(roster, [7, 11]);

        assert!(queue_id_collides_with_session_owned_roster_v1(
            7, 7, false, false
        ));
        assert!(queue_id_collides_with_session_owned_roster_v1(
            13, 7, true, false
        ));
        assert!(queue_id_collides_with_session_owned_roster_v1(
            13, 7, false, true
        ));
        assert!(!queue_id_collides_with_session_owned_roster_v1(
            13, 7, false, false
        ));

        let auxiliary = include_str!("queue_live/construction_auxiliary.rs");
        let auxiliary_finish = auxiliary
            .split("fn create_and_install(")
            .nth(1)
            .unwrap()
            .split("pub(super) fn construct_auxiliary_compute_lane_v1")
            .next()
            .unwrap();
        let recover_queue_id = auxiliary_finish
            .find("recover_native_queue_id(engine, key)")
            .unwrap();
        let collision = auxiliary_finish
            .find("session_owned_queue_id_is_retained_v1(queue_id)")
            .unwrap();
        let install = auxiliary_finish
            .find("install_auxiliary_compute_lane_slot_v1")
            .unwrap();
        assert!(recover_queue_id < collision && collision < install);
    }

    #[test]
    fn combined_teardown_destroys_and_releases_both_owner_sets_in_order() {
        let live = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let destroy = live
            .split("fn destroy_queue_and_event")
            .nth(1)
            .unwrap()
            .split("fn complete_destroy<T>")
            .next()
            .unwrap();
        let striped_destroy = destroy.find("striped_sdma.destroy_queue").unwrap();
        let directional_destroy = destroy.find("sdma.destroy_queue").unwrap();
        let compute_destroy = destroy.find("engine.destroy(self.key)").unwrap();
        assert!(striped_destroy < directional_destroy);
        assert!(directional_destroy < compute_destroy);

        let release = live
            .split("fn complete_destroy<T>")
            .nth(1)
            .unwrap()
            .split("fn require_sdma_enabled")
            .next()
            .unwrap();
        assert!(release.contains("saturating_add"));
        let striped_release = release.find("if let Some(striped_sdma)").unwrap();
        let directional_release = release.find("if let Some(sdma)").unwrap();
        assert!(striped_release < directional_release);
    }

    #[test]
    fn auxiliary_lane_reuse_advances_generation_and_rejects_substitution() {
        let session = test_queue_key(17, 3);
        let other_session = test_queue_key(19, 3);
        let mut slots = Vec::<AuxiliaryComputeLaneSlotV1<&'static str>>::new();
        let first = prepare_auxiliary_compute_lane_slot_v1(&slots).unwrap();
        assert_eq!(
            first,
            PreparedAuxiliaryComputeLaneSlotV1 {
                index: 0,
                generation: 1,
                append: true,
            }
        );
        slots.try_reserve_exact(1).unwrap();
        let destination = check_auxiliary_compute_lane_slot_v1(&mut slots, first).unwrap();
        install_auxiliary_compute_lane_slot_v1(destination, "first");
        let first_handle = ComputeAqlQueueLaneV1 {
            session,
            ordinal: 1,
            generation: first.generation,
        };
        assert!(matches!(
            admit_compute_lane_v1(session, &slots, first_handle),
            Ok(AdmittedComputeLaneV1::Auxiliary(0))
        ));
        assert!(matches!(
            admit_compute_lane_v1(other_session, &slots, first_handle),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "compute queue lane session substitution"
            ))
        ));

        assert_eq!(slots[0].state.take(), Some("first"));
        let replacement = prepare_auxiliary_compute_lane_slot_v1(&slots).unwrap();
        assert_eq!(replacement.index, first.index);
        assert_eq!(replacement.generation, first.generation + 1);
        assert!(!replacement.append);
        let destination = check_auxiliary_compute_lane_slot_v1(&mut slots, replacement).unwrap();
        install_auxiliary_compute_lane_slot_v1(destination, "replacement");

        assert!(matches!(
            admit_compute_lane_v1(session, &slots, first_handle),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "stale compute queue lane"
            ))
        ));
        let replacement_handle = ComputeAqlQueueLaneV1 {
            session,
            ordinal: 1,
            generation: replacement.generation,
        };
        assert!(matches!(
            admit_compute_lane_v1(session, &slots, replacement_handle),
            Ok(AdmittedComputeLaneV1::Auxiliary(0))
        ));
        assert_eq!(slots[0].state, Some("replacement"));
    }

    #[test]
    fn sdma_dispatch_content_check_rejects_length_and_digest_substitution() {
        let bytes = b"exact initialized bytes";
        let role = Gfx942DeviceContentRoleV1::new([0x5a; 32], 7).unwrap();
        let descriptor = Gfx942DeviceContentDescriptorV1::from_bytes(role, bytes).unwrap();
        let digest: [u8; 32] = Sha256::digest(bytes).into();

        assert!(content_descriptor_matches_bytes(descriptor, bytes));
        assert!(content_descriptor_matches_sha256(
            descriptor,
            bytes.len() as u64,
            digest
        ));
        assert!(!content_descriptor_matches_bytes(
            descriptor,
            b"exact initialized byte"
        ));

        let mut substituted = bytes.to_vec();
        substituted[0] ^= 1;
        assert!(!content_descriptor_matches_bytes(descriptor, &substituted));
        let substituted_digest: [u8; 32] = Sha256::digest(&substituted).into();
        assert!(!content_descriptor_matches_sha256(
            descriptor,
            bytes.len() as u64,
            substituted_digest
        ));
        assert!(!content_descriptor_matches_sha256(
            descriptor,
            bytes.len() as u64 - 1,
            digest
        ));
    }

    #[test]
    fn persistent_ready_promotion_uses_only_bound_certificate_under_currentness() {
        let source = include_str!("queue_live/fixed_dispatch.rs");
        let promotion = source
            .split("pub fn promote_full_h2d_to_persistent_compute_ready_v1")
            .nth(1)
            .unwrap()
            .split("fn terminal_persistent_retained_control_replay_after_detach_v1")
            .next()
            .unwrap();
        assert!(promotion.contains("certified_full_host_content_sha256"));
        assert_eq!(
            promotion
                .matches("check_queue_operational_currentness")
                .count(),
            2
        );
        assert!(!promotion.contains("sha256_host_buffer"));
        assert!(!promotion.contains("Sha256::digest"));
    }

    #[test]
    fn persistent_compute_auxiliary_quiescence_accepts_exact_detached_idle_states() {
        let max = super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1;
        assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
            true,
            None,
            2,
            Some(7),
            2,
            None,
        ));
        assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
            true,
            None,
            0,
            Some(7),
            0,
            Some(0),
        ));
        assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
            true,
            None,
            0,
            Some(0),
            0,
            Some(0),
        ));
        assert!(auxiliary_compute_lane_quiescence_from_facts_v1(
            true,
            Some(true),
            0,
            None,
            0,
            None,
        ));
        for facts in [
            (false, None, 0, Some(7), 0, Some(0)),
            (true, Some(false), 0, None, 0, None),
            (true, Some(true), 1, None, 1, None),
            (true, Some(true), 0, Some(7), 0, Some(0)),
            (true, None, 1, Some(0), 1, Some(0)),
            (true, None, 2, Some(7), 1, None),
            (true, None, max + 1, Some(7), max + 1, None),
            (true, None, 1, Some(7), 1, Some(2)),
        ] {
            assert!(!auxiliary_compute_lane_quiescence_from_facts_v1(
                facts.0, facts.1, facts.2, facts.3, facts.4, facts.5,
            ));
        }
    }

    #[test]
    fn persistent_compute_blocks_every_generic_recycled_dispatch_data_access() {
        for operation in [
            GenericRecycledDispatchAccessV1::Read,
            GenericRecycledDispatchAccessV1::ReadInto,
            GenericRecycledDispatchAccessV1::Snapshot,
            GenericRecycledDispatchAccessV1::Overwrite,
        ] {
            assert!(matches!(
                admit_generic_recycled_dispatch_access(false, true, operation),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            assert!(matches!(
                admit_generic_recycled_dispatch_access(false, false, operation),
                Ok(observed) if observed == operation
            ));
            for persistent_compute_attached in [false, true] {
                assert!(matches!(
                    admit_generic_recycled_dispatch_access(
                        true,
                        persistent_compute_attached,
                        operation,
                    ),
                    Err(Gfx942DispatchBindingErrorV1::Poisoned)
                ));
            }
        }
    }

    #[test]
    fn injected_persistent_bind_cancellation_failures_preserve_reserved_and_prepared_leases() {
        let mut reserved_owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(0x5201),
        );
        let mut prepared_owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(0x5202),
        );
        let mut foreign_owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(0x5203),
        );
        let request = |byte_len| {
            Gfx942PersistentUseRequestV1::new(
                Gfx942PersistentOperationV1::ComputeReadWrite,
                0,
                byte_len,
            )
            .unwrap()
        };

        let reserved = reserved_owner
            .reserve(request(reserved_owner.byte_len()), None)
            .unwrap();
        let reserved = cancel_persistent_compute_reserved_v1(&mut foreign_owner, reserved)
            .expect_err("foreign owner must return exact reserved lease");
        let reserved = match classify_persistent_bind_cancellation_v1(Err(reserved)) {
            PersistentBindCancellationDispositionV1::Terminal(reserved) => reserved,
            PersistentBindCancellationDispositionV1::Retryable => {
                panic!("failed reserved cancellation must never be retryable")
            }
        };
        assert_eq!(reserved_owner.live_use_count(), 1);
        cancel_persistent_compute_reserved_v1(&mut reserved_owner, reserved).unwrap();

        let reserved = prepared_owner
            .reserve(request(prepared_owner.byte_len()), None)
            .unwrap();
        let prepared = prepared_owner.prepare(reserved).unwrap();
        let prepared = cancel_persistent_compute_prepared_v1(&mut foreign_owner, prepared)
            .expect_err("foreign owner must return exact prepared lease");
        let prepared = match classify_persistent_bind_cancellation_v1(Err(prepared)) {
            PersistentBindCancellationDispositionV1::Terminal(prepared) => prepared,
            PersistentBindCancellationDispositionV1::Retryable => {
                panic!("failed prepared cancellation must never be retryable")
            }
        };
        assert_eq!(prepared_owner.live_use_count(), 1);
        cancel_persistent_compute_prepared_v1(&mut prepared_owner, prepared).unwrap();
        assert!(matches!(
            classify_persistent_bind_cancellation_v1::<()>(Ok(())),
            PersistentBindCancellationDispositionV1::Retryable
        ));

        let production = include_str!("queue_live/fixed_dispatch.rs");
        let reserved_terminal = production
            .split("match classify_persistent_bind_cancellation_v1")
            .nth(1)
            .unwrap()
            .split("if let Some(dispatch)")
            .next()
            .unwrap();
        assert!(
            reserved_terminal
                .contains("PersistentBindCancellationDispositionV1::Terminal(reserved)")
        );
        assert!(reserved_terminal.contains("PersistentComputeUseStateV1::Reserved(reserved)"));
        assert!(reserved_terminal.contains("ProcessTeardown"));
        let prepared_terminal = production
            .split("Err(error) => match classify_persistent_bind_cancellation_v1")
            .nth(1)
            .unwrap()
            .split("let (mut allocation")
            .next()
            .unwrap();
        assert!(
            prepared_terminal
                .contains("PersistentBindCancellationDispositionV1::Terminal(prepared)")
        );
        assert!(prepared_terminal.contains("PersistentComputeUseStateV1::Prepared(prepared)"));
        assert!(prepared_terminal.contains("ProcessTeardown"));
    }

    #[test]
    fn coherent_capture_preflight_binds_owner_kind_and_logical_extent() {
        let queue = test_queue_key(701, 1);
        let (device, mut host) = crate::sdma::persistent_sdma_buffers_for_test(queue, 91);
        host.set_logical_bytes(32);
        assert_eq!(preflight_sdma_host_read_into_v1(&host, queue, 8, 8), Ok(()));
        for foreign in [test_queue_key(702, 1), test_queue_key(701, 2)] {
            assert_eq!(
                preflight_sdma_host_read_into_v1(&host, foreign, 8, 8),
                Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer)
            );
        }
        assert_eq!(
            preflight_sdma_host_read_into_v1(&device, queue, 0, 8),
            Err(Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer)
        );
        for (offset, len) in [(0, 0), (25, 8), (u64::MAX, 8)] {
            assert_eq!(
                preflight_sdma_host_read_into_v1(&host, queue, offset, len),
                Err(Gfx942SdmaHostReadIntoErrorV1::InvalidRange)
            );
        }
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut destination = [0xa5; 8];
        assert_eq!(
            session.read_sdma_host_buffer_into_v1(&host, 8, &mut destination),
            Err(Gfx942SdmaHostReadIntoErrorV1::Unavailable)
        );
        assert_eq!(destination, [0xa5; 8]);
        assert!(!session.terminal_poisoned);
        session.terminal_poisoned = true;
        assert_eq!(
            session.read_sdma_host_buffer_into_v1(&host, 8, &mut destination),
            Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain)
        );
        assert_eq!(
            session.read_sdma_host_buffer_into_v1(&device, u64::MAX, &mut destination),
            Err(Gfx942SdmaHostReadIntoErrorV1::NativeUncertain)
        );
        assert_eq!(destination, [0xa5; 8]);
    }

    #[test]
    fn coherent_capture_queue_wiring_preserves_retake_and_terminal_boundary() {
        let source = include_str!("queue_live.rs");
        let body = source
            .split("pub fn read_sdma_host_buffer_into_v1(")
            .nth(1)
            .unwrap()
            .split("/// Rebrands one fully initialized")
            .next()
            .unwrap();
        assert!(body.contains("preflight_sdma_host_read_into_v1"));
        assert!(body.contains("self.with_live_queue_memory_model("));
        assert!(body.contains("crate::sdma::read_host_buffer_into_v1"));
        assert!(body.contains("permanently_poison_process_global_kfd_runtime_gate_v1()"));
        for forbidden in [
            ".poll(",
            ".flush(",
            "read_host_buffer(",
            "to_string(",
            "to_vec(",
            "allocate_",
        ] {
            assert!(
                !body.contains(forbidden),
                "unexpected capture path: {forbidden}"
            );
        }
    }

    pub(super) fn persistent_compute_cancellation_test_session(
        queue: QueueKeyV1,
        persistent_compute: Option<PersistentComputeAttachmentV1>,
        release: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)>,
    ) -> ComputeAqlQueueSessionV1 {
        ComputeAqlQueueSessionV1 {
            engine: None,
            key: queue,
            compute_lane_session: queue,
            doorbell: None,
            submission: None,
            completion_signals: None,
            completion_owner: QueueOwnerSlotV1(Some(
                CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
            )),
            dependency_owner: QueueOwnerSlotV1(Some(
                ComputeDependencySessionOwnerV1::new(queue.id.0).unwrap(),
            )),
            terminal_dependency: None,
            dispatch: None,
            unpublished_dispatch: UnpublishedDispatchStateV1::default(),
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            persistent_compute: persistent_compute
                .map(BoundedPersistentComputeAttachmentV1::from_single),
            persistent_compute_test_release: release,
            next_persistent_compute_generation: 2,
            exception: None,
            sdma: None,
            striped_sdma: None,
            sdma_outstanding_buffers: 0,
            sdma_pool_free: Vec::new(),
            sdma_pool_reuse_count: 0,
            sdma_device_pool: SdmaDevicePoolConfigurationV1::default(),
            sdma_host_pool_limits: None,
            terminal_poisoned: false,
            observation: ComputeAqlQueueObservationV1 {
                queue_id: 0,
                ring_bytes: 0,
                doorbell_slice_bytes: 0,
                doorbell_byte_offset: 0,
                event_id: 0,
                cwsr_shadow_pages: 0,
            },
            auxiliary_compute_lanes: Vec::new(),
        }
    }

    fn persistent_compute_gate_test_allocation_v1(
        queue: QueueKeyV1,
        id: u64,
    ) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        let (mut device, _host) = crate::sdma::persistent_sdma_buffers_for_test(queue, id);
        device.set_logical_bytes(device.physical_bytes());
        let pair =
            admit_persistent_directional_sdma_pair_v1(Gfx942DirectionalSdmaQueueObservationV1 {
                host_to_device: Gfx942SdmaQueueObservationV1 {
                    queue_id: 17,
                    ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
                    maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
                    engine_index: Some(crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1),
                },
                device_to_host: Gfx942SdmaQueueObservationV1 {
                    queue_id: 23,
                    ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
                    maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
                    engine_index: Some(crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1),
                },
                admitted_engine_count: 2,
                admitted_queues_per_engine: 8,
            })
            .unwrap();
        let (allocation, outstanding) =
            promote_directional_persistent_sdma_custody_v1(device, pair, 2).unwrap();
        assert_eq!(outstanding, 2);
        allocation
    }

    fn persistent_compute_gate_test_entry_v1(
        queue: QueueKeyV1,
        id: u64,
        effect: Gfx942PersistentComputeEffectV1,
    ) -> PersistentComputeAttachmentEntryV1 {
        let mut allocation = persistent_compute_gate_test_allocation_v1(queue, id);
        let storage_identity = allocation
            .owner
            .local_native_for_sdma()
            .expect("gate fixture retains native storage")
            .storage_identity();
        allocation
            .owner
            .quarantine_for_caller_reported_currentness_loss();
        PersistentComputeAttachmentEntryV1 {
            allocation,
            authenticated_sha256: Some([id as u8; 32]),
            fully_initialized: true,
            state: PersistentComputeUseStateV1::Quarantined,
            storage_identity: Some(storage_identity),
            effect,
        }
    }

    fn published_three_binding_test_entry_v1(
        queue: QueueKeyV1,
        id: u64,
        effect: Gfx942PersistentComputeEffectV1,
    ) -> PersistentComputeAttachmentEntryV1 {
        let mut allocation = persistent_compute_gate_test_allocation_v1(queue, id);
        let storage_identity = allocation
            .owner
            .local_native_for_sdma()
            .expect("published fixture retains native storage")
            .storage_identity();
        let operation = match effect {
            Gfx942PersistentComputeEffectV1::Read => Gfx942PersistentOperationV1::ComputeRead,
            Gfx942PersistentComputeEffectV1::Write => Gfx942PersistentOperationV1::ComputeWrite,
            Gfx942PersistentComputeEffectV1::ReadWrite => {
                Gfx942PersistentOperationV1::ComputeReadWrite
            }
        };
        let reserved = allocation
            .owner
            .reserve(
                Gfx942PersistentUseRequestV1::new(operation, 0, allocation.byte_len()).unwrap(),
                None,
            )
            .unwrap();
        let prepared = allocation.owner.prepare(reserved).unwrap();
        let published = allocation.owner.publish(prepared).unwrap();
        PersistentComputeAttachmentEntryV1 {
            allocation,
            authenticated_sha256: Some([id as u8; 32]),
            fully_initialized: true,
            state: PersistentComputeUseStateV1::Published(published),
            storage_identity: Some(storage_identity),
            effect,
        }
    }

    fn persistent_compute_gate_test_session_v1(
        queue: QueueKeyV1,
        binding_count: usize,
    ) -> ComputeAqlQueueSessionV1 {
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        let attachment = match binding_count {
            1 => {
                let entry = persistent_compute_gate_test_entry_v1(
                    queue,
                    0x5301,
                    Gfx942PersistentComputeEffectV1::ReadWrite,
                );
                BoundedPersistentComputeAttachmentV1::from_single(PersistentComputeAttachmentV1 {
                    allocation: entry.allocation,
                    authenticated_sha256: entry.authenticated_sha256,
                    fully_initialized: entry.fully_initialized,
                    state: entry.state,
                    binding,
                    storage_identity: entry
                        .storage_identity
                        .expect("single gate fixture retains storage identity"),
                    effect: entry.effect,
                    predecessor_dispatch_generation: None,
                    terminal_custody: None,
                })
            }
            3 => BoundedPersistentComputeAttachmentV1::from_three(
                ThreeBindingPersistentComputeAttachmentV1 {
                    entries: [
                        persistent_compute_gate_test_entry_v1(
                            queue,
                            0x5302,
                            Gfx942PersistentComputeEffectV1::Read,
                        ),
                        persistent_compute_gate_test_entry_v1(
                            queue,
                            0x5303,
                            Gfx942PersistentComputeEffectV1::Read,
                        ),
                        persistent_compute_gate_test_entry_v1(
                            queue,
                            0x5304,
                            Gfx942PersistentComputeEffectV1::Write,
                        ),
                    ],
                    binding,
                    predecessor_dispatch_generation: None,
                    terminal_custody: None,
                },
            ),
            _ => panic!("gate fixture supports exact N=1 or N=3"),
        };
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.persistent_compute = Some(attachment);
        session
    }

    #[test]
    fn either_persistent_roster_cardinality_blocks_generic_queue_transitions() {
        for (queue_id, binding_count) in [(190, 1), (191, 3)] {
            let queue = test_queue_key(queue_id, 1);
            let mut session = persistent_compute_gate_test_session_v1(queue, binding_count);
            let roster_snapshot = |session: &ComputeAqlQueueSessionV1| {
                let attachment = session.persistent_compute.as_ref().unwrap();
                (
                    attachment.binding,
                    attachment
                        .entries
                        .iter()
                        .map(|entry| {
                            (
                                entry.storage_identity,
                                entry.authenticated_sha256,
                                entry.fully_initialized,
                                entry.effect,
                                entry.allocation.byte_len(),
                                entry.allocation.owner.live_use_count(),
                                entry.allocation.owner.quarantine_reason(),
                            )
                        })
                        .collect::<Vec<_>>(),
                    session.next_persistent_compute_generation,
                )
            };
            let existing = roster_snapshot(&session);
            assert!(session.has_any_persistent_compute_attachment_v1());
            assert_eq!(
                session
                    .persistent_compute
                    .as_ref()
                    .expect("gate fixture retains attachment")
                    .entries
                    .len(),
                binding_count
            );

            assert!(matches!(
                session.submit_fixed_dispatch::<1>(),
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                ))
            ));
            assert!(matches!(
                session.submit_fixed_dispatch_classified_v1::<1>(),
                Err(
                    Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )
                )
            ));
            assert!(!session.terminal_poisoned);
            assert!(matches!(
                session.detach_recycled_fixed_dispatch(),
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                ))
            ));
            assert!(matches!(
                session.release_retained_persistent_fixed_dispatch_control_v1(),
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                ))
            ));
            assert!(matches!(
                session.recycled_fixed_dispatch_generation(),
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                ))
            ));
            assert!(matches!(
                session.destroy_queue_and_event(QueueDestroyModeV1::Release),
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute attachment must be restored before queue destruction"
                ))
            ));

            let allocation = persistent_compute_gate_test_allocation_v1(queue, 0x5400 + queue_id);
            let incoming = (
                allocation
                    .owner
                    .local_native_for_sdma()
                    .unwrap()
                    .storage_identity(),
                allocation.byte_len(),
            );
            let input = Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation);
            let packet = Gfx942FixedDispatchPacketV1::new(
                0,
                fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
                0,
                Vec::new().into_boxed_slice(),
                Vec::new().into_boxed_slice(),
            );
            let failure = match session.bind_directional_persistent_fixed_dispatch_v1(
                Vec::new(),
                [packet],
                input,
                Gfx942DeviceContentRoleV1::new([0x54; 32], 0).unwrap(),
            ) {
                Err(failure) => failure,
                Ok(_) => panic!("a live persistent roster must reject generic rebind"),
            };
            assert!(matches!(
                failure.error(),
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            ));
            let (_, custody) = failure.into_parts();
            let Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input) = custody else {
                panic!("occupied-slot rejection must return the exact incoming input")
            };
            let (allocation, digest, initialized) = input.into_parts();
            assert_eq!(digest, None);
            assert!(initialized);
            assert_eq!(allocation.attachment.queue, queue);
            assert_eq!(
                (
                    allocation
                        .owner
                        .local_native_for_sdma()
                        .unwrap()
                        .storage_identity(),
                    allocation.byte_len(),
                ),
                incoming
            );
            assert_eq!(roster_snapshot(&session), existing);
            assert!(session.has_any_persistent_compute_attachment_v1());
            assert!(!session.terminal_poisoned);
        }
    }

    #[test]
    fn every_generic_linear_transition_checks_the_unified_persistent_roster_gate() {
        let fixed = include_str!("queue_live/fixed_dispatch.rs");
        let production = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let guarded_spans = [
            (
                fixed,
                "pub fn detach_recycled_fixed_dispatch(\n        &mut self,",
                1,
                "pub fn release_retained_persistent_fixed_dispatch_control_v1",
            ),
            (
                fixed,
                "pub fn submit_fixed_dispatch<const N: usize>(\n        &mut self,",
                1,
                "fn submit_fixed_dispatch_with_dependency_events_inner_v1",
            ),
            (
                fixed,
                "pub fn poll_fixed_dispatch<const N: usize>(\n        &mut self,",
                1,
                "pub fn poll_fixed_dispatch_with_progress<const N: usize>",
            ),
            (
                fixed,
                "pub fn poll_fixed_dispatch_with_progress<const N: usize>(\n        &mut self,",
                1,
                "fn wait_fixed_dispatch_inner<const N: usize>",
            ),
            (
                fixed,
                "pub fn recycle_fixed_dispatch<const N: usize>(\n        &mut self,",
                1,
                "pub fn recycled_fixed_dispatch_generation",
            ),
            (
                production,
                "fn destroy_queue_and_event(\n        &mut self,",
                1,
                "fn complete_destroy<T>",
            ),
            (
                fixed,
                "pub fn bind_three_binding_directional_persistent_fixed_dispatch_v1",
                1,
                "pub fn bind_directional_persistent_fixed_dispatch_v1",
            ),
            (
                fixed,
                "pub fn bind_directional_persistent_fixed_dispatch_v1",
                1,
                "pub fn submit_directional_persistent_fixed_dispatch_v1",
            ),
        ];
        for (source, start, occurrence, end) in guarded_spans {
            let span = source
                .split(start)
                .nth(occurrence)
                .unwrap_or_else(|| panic!("missing audited transition: {start}"))
                .split(end)
                .next()
                .unwrap();
            assert!(
                span.contains("self.has_any_persistent_compute_attachment_v1()"),
                "transition bypasses unified persistent roster gate: {start}"
            );
        }
    }

    #[test]
    fn persistent_bind_terminal_ingress_retains_self_but_returns_foreign_input() {
        let receiver = test_queue_key(291, 1);
        for source in [receiver, test_queue_key(292, 1)] {
            let mut session = persistent_compute_cancellation_test_session(receiver, None, None);
            session.poison_terminal();
            let allocation = persistent_compute_gate_test_allocation_v1(source, 0x9100);
            let identity = allocation
                .owner
                .local_native_for_sdma()
                .unwrap()
                .storage_identity();
            let input = Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation);
            let packet = Gfx942FixedDispatchPacketV1::new(
                0,
                fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
                0,
                Vec::new().into_boxed_slice(),
                Vec::new().into_boxed_slice(),
            );
            let failure = match session.bind_directional_persistent_fixed_dispatch_v1(
                Vec::new(),
                [packet],
                input,
                Gfx942DeviceContentRoleV1::new([0x54; 32], 0).unwrap(),
            ) {
                Err(failure) => failure,
                Ok(_) => panic!("terminal ingress must reject"),
            };
            assert!(matches!(
                failure.error(),
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::Poisoned
                )
            ));
            let input = match failure.into_parts().1 {
                Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input) => {
                    assert_ne!(source, receiver);
                    input
                }
                Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(terminal) => {
                    assert_eq!(source, receiver);
                    terminal.input.unwrap()
                }
            };
            let (allocation, _, _) = input.into_parts();
            assert_eq!(allocation.attachment.queue, source);
            assert_eq!(
                allocation
                    .owner
                    .local_native_for_sdma()
                    .unwrap()
                    .storage_identity(),
                identity
            );
            assert!(session.persistent_compute.is_none());
            assert!(session.dispatch.is_none());
        }
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn three_binding_false_closing_currentness_quarantines_all_owners_for_any_observation() {
        use super::super::completion::TestOnlyAmbiguousCompletionObservationV1;

        for (queue_id, observation) in [
            (192, TestOnlyAmbiguousCompletionObservationV1::Pending),
            (193, TestOnlyAmbiguousCompletionObservationV1::Completed),
        ] {
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            let queue = test_queue_key(queue_id, 1);
            let mut session = persistent_compute_cancellation_test_session(queue, None, None);
            let mut dispatch_owner =
                super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
            let batch = session
                .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                    &mut dispatch_owner,
                    |generation| test_completion_template(queue, generation),
                    |_, packets| {
                        assert_eq!(packets.packet_count(), 1);
                        Ok(queue_id)
                    },
                )
                .unwrap();
            let binding = PersistentComputeBindingKeyV1 {
                queue,
                attachment_generation: 1,
            };
            session.persistent_compute = Some(BoundedPersistentComputeAttachmentV1::from_three(
                ThreeBindingPersistentComputeAttachmentV1 {
                    entries: [
                        published_three_binding_test_entry_v1(
                            queue,
                            0x5500 + queue_id,
                            Gfx942PersistentComputeEffectV1::Read,
                        ),
                        published_three_binding_test_entry_v1(
                            queue,
                            0x5600 + queue_id,
                            Gfx942PersistentComputeEffectV1::Read,
                        ),
                        published_three_binding_test_entry_v1(
                            queue,
                            0x5700 + queue_id,
                            Gfx942PersistentComputeEffectV1::Write,
                        ),
                    ],
                    binding,
                    predecessor_dispatch_generation: None,
                    terminal_custody: None,
                },
            ));
            let receipt = Gfx942ThreeBindingPersistentComputeDispatchV1 {
                binding,
                batch,
                thread_affinity: PhantomData,
            };
            let failure = match session
                .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
                    receipt,
                    |_, identity, completion| {
                        dispatch_owner
                            .validate_published(identity, completion)
                            .is_ok()
                    },
                    |session, completion| {
                        session
                            .completion_owner
                            .observe_one_with_false_closing_currentness_for_test(
                                completion,
                                observation,
                            )
                            .map_err(|(error, completion)| (error.into(), completion))
                    },
                ) {
                Err(failure) => failure,
                Ok(_) => panic!("false closing currentness cannot be retryable or complete"),
            };
            assert!(matches!(
                failure.error(),
                ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Currentness)
            ));
            let (_, recovered) = failure.into_parts();
            assert!(recovered.is_none());
            assert!(session.terminal_poisoned);
            assert!(take_dispatch_terminal_process_gate_record_v1());
            let attachment = session
                .three_binding_persistent_compute_attachment_v1()
                .expect("terminal queue retains all three owners");
            assert!(attachment.entries.iter().all(|entry| {
                matches!(entry.state, PersistentComputeUseStateV1::Quarantined)
                    && entry.allocation.owner.quarantine_reason()
                        == Some(
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                        )
            }));
            assert!(matches!(
                attachment.terminal_custody,
                Some(PersistentComputeTerminalNativeCustodyV1::Published(_))
            ));
        }
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn three_binding_bounded_wait_timeout_returns_exact_published_custody() {
        let queue = test_queue_key(197, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut dispatch_owner =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let batch = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch_owner,
                |generation| test_completion_template(queue, generation),
                |_, packets| {
                    assert_eq!(packets.packet_count(), 1);
                    Ok(197)
                },
            )
            .unwrap();
        let completion_before = session.completion_owner.state_snapshot_for_test();
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        session.persistent_compute = Some(BoundedPersistentComputeAttachmentV1::from_three(
            ThreeBindingPersistentComputeAttachmentV1 {
                entries: [
                    published_three_binding_test_entry_v1(
                        queue,
                        0x5b00,
                        Gfx942PersistentComputeEffectV1::Read,
                    ),
                    published_three_binding_test_entry_v1(
                        queue,
                        0x5b01,
                        Gfx942PersistentComputeEffectV1::Read,
                    ),
                    published_three_binding_test_entry_v1(
                        queue,
                        0x5b02,
                        Gfx942PersistentComputeEffectV1::Write,
                    ),
                ],
                binding,
                predecessor_dispatch_generation: None,
                terminal_custody: None,
            },
        ));
        let receipt = Gfx942ThreeBindingPersistentComputeDispatchV1 {
            binding,
            batch,
            thread_affinity: PhantomData,
        };
        let timeout = session
            .wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1_using(
                receipt,
                Instant::now(),
                |session, dispatch| {
                    session
                        .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
                            dispatch,
                            |_, identity, completion| {
                                dispatch_owner.validate_published(identity, completion).is_ok()
                            },
                            |session, completion| {
                                session
                                    .completion_owner
                                    .observe_one_pending_with_current_closing_for_test(completion)
                                    .map_err(|(error, completion)| (error.into(), completion))
                            },
                        )
                },
            )
            .expect("one conclusive Pending observation reaches the expired deadline");
        let Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Timeout {
            dispatch,
            observations,
        } = timeout
        else {
            panic!("pending completion must return timeout custody")
        };
        assert_eq!(observations, 1);
        assert_eq!(dispatch.binding, binding);
        assert_eq!(dispatch_owner.live_epoch_count(), 1);
        assert_eq!(
            session.completion_owner.state_snapshot_for_test(),
            completion_before
        );
        assert!(!session.terminal_poisoned);
        assert!(
            session
                .three_binding_persistent_compute_attachment_v1()
                .unwrap()
                .entries
                .iter()
                .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
        );
    }

    pub(super) fn compute_lane_state_for_multi_inflight_test(
        queue: QueueKeyV1,
    ) -> ComputeAqlQueueLaneStateV1 {
        ComputeAqlQueueLaneStateV1 {
            key: queue,
            doorbell: None,
            submission: None,
            completion_signals: None,
            completion_owner: QueueOwnerSlotV1(Some(
                CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
            )),
            dispatch: None,
            unpublished_dispatch: UnpublishedDispatchStateV1::default(),
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            exception: None,
            observation: ComputeAqlQueueObservationV1 {
                queue_id: 0,
                ring_bytes: 0,
                doorbell_slice_bytes: 0,
                doorbell_byte_offset: 0,
                event_id: 0,
                cwsr_shadow_pages: 0,
            },
        }
    }

    pub(super) fn prepared_persistent_compute_cancellation_fixture(
        queue: QueueKeyV1,
        id: u64,
        authenticated_sha256: Option<[u8; 32]>,
        predecessor_dispatch_generation: Option<u64>,
    ) -> (
        ComputeAqlQueueSessionV1,
        Gfx942PreparedPersistentComputeDispatchV1,
        crate::Gfx942DeviceMemoryIdentityV1,
    ) {
        prepared_persistent_compute_cancellation_fixture_with_initialization_v1(
            queue,
            id,
            authenticated_sha256,
            predecessor_dispatch_generation,
            true,
        )
    }

    fn prepared_persistent_compute_cancellation_fixture_with_initialization_v1(
        queue: QueueKeyV1,
        id: u64,
        authenticated_sha256: Option<[u8; 32]>,
        predecessor_dispatch_generation: Option<u64>,
        fully_initialized: bool,
    ) -> (
        ComputeAqlQueueSessionV1,
        Gfx942PreparedPersistentComputeDispatchV1,
        crate::Gfx942DeviceMemoryIdentityV1,
    ) {
        let (mut device, _host) = crate::sdma::persistent_sdma_buffers_for_test(queue, id);
        device.set_logical_bytes(device.physical_bytes());
        let pair =
            admit_persistent_directional_sdma_pair_v1(Gfx942DirectionalSdmaQueueObservationV1 {
                host_to_device: Gfx942SdmaQueueObservationV1 {
                    queue_id: 17,
                    ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
                    maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
                    engine_index: Some(crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1),
                },
                device_to_host: Gfx942SdmaQueueObservationV1 {
                    queue_id: 23,
                    ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
                    maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
                    engine_index: Some(crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1),
                },
                admitted_engine_count: 2,
                admitted_queues_per_engine: 8,
            })
            .unwrap();
        let (mut allocation, outstanding) =
            promote_directional_persistent_sdma_custody_v1(device, pair, 2).unwrap();
        assert_eq!(outstanding, 2);
        let storage_identity = allocation
            .owner
            .local_native_for_sdma()
            .expect("promoted allocation retains local native custody")
            .storage_identity();
        let effect = if fully_initialized {
            Gfx942PersistentComputeEffectV1::ReadWrite
        } else {
            Gfx942PersistentComputeEffectV1::Write
        };
        let request = Gfx942PersistentUseRequestV1::new(
            if fully_initialized {
                Gfx942PersistentOperationV1::ComputeReadWrite
            } else {
                Gfx942PersistentOperationV1::ComputeWrite
            },
            0,
            allocation.byte_len(),
        )
        .unwrap();
        let reserved = allocation.owner.reserve(request, None).unwrap();
        let prepared = allocation.owner.prepare(reserved).unwrap();
        let lease = allocation
            .owner
            .detach_local_native_for_compute(&prepared)
            .unwrap();
        let data = if fully_initialized {
            Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease)
        } else {
            Gfx942FixedDispatchDataV1::uninitialized(lease)
        };
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        let attachment = PersistentComputeAttachmentV1 {
            allocation,
            authenticated_sha256,
            fully_initialized,
            state: PersistentComputeUseStateV1::Prepared(prepared),
            binding,
            storage_identity,
            effect,
            predecessor_dispatch_generation,
            terminal_custody: None,
        };
        let session = persistent_compute_cancellation_test_session(
            queue,
            Some(attachment),
            Some((predecessor_dispatch_generation.unwrap_or(0), vec![data])),
        );
        (
            session,
            Gfx942PreparedPersistentComputeDispatchV1 {
                binding,
                thread_affinity: PhantomData,
            },
            storage_identity,
        )
    }

    fn prepared_three_binding_persistent_compute_cancellation_fixture_v1(
        queue: QueueKeyV1,
    ) -> (
        ComputeAqlQueueSessionV1,
        Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
        [crate::Gfx942DeviceMemoryIdentityV1; 3],
        [[u8; 32]; 3],
    ) {
        let digests = [[0xa1; 32], [0xb2; 32], [0xc3; 32]];
        let effects = [
            Gfx942PersistentComputeEffectV1::Read,
            Gfx942PersistentComputeEffectV1::Read,
            Gfx942PersistentComputeEffectV1::Write,
        ];
        let make = |index: usize| {
            let mut allocation = persistent_compute_gate_test_allocation_v1(
                queue,
                0x5900 + u64::try_from(index).unwrap(),
            );
            let storage_identity = allocation
                .owner
                .local_native_for_sdma()
                .expect("prepared fixture retains native storage")
                .storage_identity();
            let operation = match effects[index] {
                Gfx942PersistentComputeEffectV1::Read => Gfx942PersistentOperationV1::ComputeRead,
                Gfx942PersistentComputeEffectV1::Write => Gfx942PersistentOperationV1::ComputeWrite,
                Gfx942PersistentComputeEffectV1::ReadWrite => {
                    Gfx942PersistentOperationV1::ComputeReadWrite
                }
            };
            let request =
                Gfx942PersistentUseRequestV1::new(operation, 0, allocation.byte_len()).unwrap();
            let reserved = allocation.owner.reserve(request, None).unwrap();
            let prepared = allocation.owner.prepare(reserved).unwrap();
            let lease = allocation
                .owner
                .detach_local_native_for_compute(&prepared)
                .unwrap();
            (
                PersistentComputeAttachmentEntryV1 {
                    allocation,
                    authenticated_sha256: Some(digests[index]),
                    fully_initialized: true,
                    state: PersistentComputeUseStateV1::Prepared(prepared),
                    storage_identity: Some(storage_identity),
                    effect: effects[index],
                },
                Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease),
                storage_identity,
            )
        };
        let [
            (entry_a, data_a, identity_a),
            (entry_b, data_b, identity_b),
            (entry_c, data_c, identity_c),
        ] = [0, 1, 2].map(make);
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        let mut session = persistent_compute_cancellation_test_session(
            queue,
            None,
            Some((7, vec![data_a, data_b, data_c])),
        );
        session.set_three_binding_persistent_compute_attachment_v1(
            ThreeBindingPersistentComputeAttachmentV1 {
                entries: [entry_a, entry_b, entry_c],
                binding,
                predecessor_dispatch_generation: Some(7),
                terminal_custody: None,
            },
        );
        (
            session,
            Gfx942PreparedThreeBindingPersistentComputeDispatchV1 {
                binding,
                thread_affinity: PhantomData,
            },
            [identity_a, identity_b, identity_c],
            digests,
        )
    }

    #[test]
    fn three_binding_prepublication_cancel_restores_all_exact_initialized_inputs() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(194, 1);
        let (mut session, prepared, identities, digests) =
            prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue);
        let inputs = session
            .cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("clean prepublication cancellation restores all three inputs")
            .into_inputs();
        assert!(session.persistent_compute.is_none());
        assert_eq!(session.detached_dispatch_generation, Some(7));
        assert_eq!(session.detached_next_insertion_index, Some(0));
        assert!(!session.terminal_poisoned);
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        for (index, input) in inputs.into_iter().enumerate() {
            let (allocation, digest, initialized) = input.into_parts();
            assert_eq!(digest, Some(digests[index]));
            assert!(initialized);
            assert_eq!(
                allocation
                    .owner
                    .local_native_for_sdma()
                    .expect("cancelled input regains native storage")
                    .storage_identity(),
                identities[index]
            );
            assert_eq!(allocation.owner.live_use_count(), 0);
            assert_eq!(allocation.owner.retained_settled_use_count(), 0);
            assert_eq!(allocation.owner.quarantine_reason(), None);
        }
    }

    #[test]
    fn three_binding_cancel_identity_preflight_prevents_partial_restoration() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(195, 1);
        let (mut session, prepared, identities, _) =
            prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue);
        session
            .persistent_compute
            .as_mut()
            .expect("fixture retains three entries")
            .entries[1]
            .storage_identity = Some(identities[0]);
        let failure = session
            .cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("substituted identity must fail before any owner restoration");
        assert!(failure.into_parts().1.is_none());
        assert!(session.terminal_poisoned);
        assert!(take_dispatch_terminal_process_gate_record_v1());
        let attachment = session
            .three_binding_persistent_compute_attachment_v1()
            .expect("terminal cancellation retains all owners");
        assert!(attachment.entries.iter().all(|entry| {
            entry.allocation.owner.local_native_for_sdma().is_none()
                && matches!(entry.state, PersistentComputeUseStateV1::Quarantined)
                && entry.allocation.owner.quarantine_reason()
                    == Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
        }));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Data(_))
        ));
    }

    #[test]
    fn production_three_binding_bind_validates_real_vecadd_metadata_and_exact_storage() {
        use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
        use fe2o3_hsaco::ArgumentAccess;

        let queue = test_queue_key(196, 1);
        let signature = [0x57; 32];
        let image =
            include_bytes!("../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
        let program = validate(image, AdmittedProfile::Gfx942XnackOffCov6)
            .unwrap()
            .bind_kernel("vecadd")
            .unwrap()
            .reconcile_dispatch_abi(
                signature,
                &[
                    KernelGlobalBufferAbiV1::new(0, "arg0.data", 0, 1, ArgumentAccess::ReadOnly),
                    KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 1, ArgumentAccess::ReadOnly),
                    KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 1, ArgumentAccess::WriteOnly),
                ],
            )
            .unwrap();
        let byte_len = 4096_u64;
        let mut kernarg = [0_u8; 48];
        for offset in [8, 24, 40] {
            kernarg[offset..offset + 8].copy_from_slice(&(byte_len / 4).to_le_bytes());
        }
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            fe2o3_aql::AqlDispatchGeometryV1::new([1024, 1, 1], [256, 1, 1]).unwrap(),
            0,
            kernarg.into(),
            [
                super::super::dispatch_binding::Gfx942DispatchBufferBindingV1::new(
                    0, 0, 0, byte_len,
                ),
                super::super::dispatch_binding::Gfx942DispatchBufferBindingV1::new(
                    2, 1, 0, byte_len,
                ),
                super::super::dispatch_binding::Gfx942DispatchBufferBindingV1::new(
                    4, 2, 0, byte_len,
                ),
            ]
            .into(),
        );
        assert_eq!(
            packet.ordering(),
            fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior
        );
        let make = |index: u64| {
            let allocation = persistent_compute_gate_test_allocation_v1(
                queue,
                0x5a00_u64.checked_add(index).unwrap(),
            );
            let identity = allocation
                .owner
                .local_native_for_sdma()
                .unwrap()
                .storage_identity();
            (
                Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation),
                identity,
            )
        };
        let [
            (input_a, identity_a),
            (input_b, identity_b),
            (input_c, identity_c),
        ] = [0, 1, 2].map(make);
        let identities = [identity_a, identity_b, identity_c];
        assert!(identity_a != identity_b && identity_a != identity_c && identity_b != identity_c);
        THREE_BINDING_BIND_VALIDATION_EFFECTS_V1.set(None);
        THREE_BINDING_BIND_VALIDATION_ONLY_V1.set(true);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let failure = session
            .bind_three_binding_directional_persistent_fixed_dispatch_v1(
                vec![program],
                [packet],
                Gfx942ThreeBindingPersistentComputeInputsV1::new([input_a, input_b, input_c]),
                [0, 1, 2]
                    .map(|ordinal| Gfx942DeviceContentRoleV1::new(signature, ordinal).unwrap()),
            )
            .expect_err("validation-only boundary returns exact unmodified input custody");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Contract("test-only three-binding validation boundary")
        ));
        assert_eq!(
            THREE_BINDING_BIND_VALIDATION_EFFECTS_V1.replace(None),
            Some([
                DeviceDataEffectV1::ReadOnly,
                DeviceDataEffectV1::ReadOnly,
                DeviceDataEffectV1::WriteOnly,
            ])
        );
        let (_, custody) = failure.into_parts();
        let Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::Retryable(inputs) = custody
        else {
            panic!("validation-only exit must return all three inputs")
        };
        for (input, identity) in inputs.into_inputs().into_iter().zip(identities) {
            let (allocation, _, initialized) = input.into_parts();
            assert!(initialized);
            assert_eq!(
                allocation
                    .owner
                    .local_native_for_sdma()
                    .unwrap()
                    .storage_identity(),
                identity
            );
            assert_eq!(allocation.owner.live_use_count(), 0);
        }
        assert!(!session.has_any_persistent_compute_attachment_v1());
        assert!(!session.terminal_poisoned);
    }

    #[test]
    fn ordinary_template_validation_rejection_is_clean_and_next_binding_can_publish() {
        let queue = test_queue_key(170, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        for error in [
            Gfx942DispatchBindingErrorV1::ZeroPacketCount,
            Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum {
                requested: GFX942_MAX_FIXED_DISPATCH_PACKETS_V1 + 1,
                maximum: GFX942_MAX_FIXED_DISPATCH_PACKETS_V1,
            },
            Gfx942DispatchBindingErrorV1::WrongQueueGeneration,
        ] {
            let failure = session
                .classify_fixed_dispatch_binding::<[CompletionPacketTemplateV1; 1]>(
                    FixedDispatchBindingModeV1::Ordinary,
                    Err(error),
                )
                .expect_err("ordinary pre-mutation validation must reject");
            assert!(matches!(
                failure,
                FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(_)
            ));
            assert!(!session.terminal_poisoned);
        }

        let templates = session
            .classify_fixed_dispatch_binding(
                FixedDispatchBindingModeV1::Ordinary,
                Ok([test_completion_template(queue, 8)]),
            )
            .expect("a corrected batch remains admissible");
        let _published = session
            .submit_with_completions_classified_using(templates, |_, packets| {
                assert_eq!(packets.packet_count(), 1);
                Ok(41)
            })
            .expect("clean validation rejection leaves completion submission usable");
        assert!(!session.terminal_poisoned);
    }

    #[test]
    fn persistent_exact_occupancy_retry_restores_every_layer_then_succeeds() {
        for occupancy in [
            fe2o3_aql::AqlRingReservationError::Full,
            fe2o3_aql::AqlRingReservationError::InsufficientSpace {
                requested: 1,
                available: 0,
            },
        ] {
            let queue = test_queue_key(171, 1);
            let (mut session, prepared, _) =
                prepared_persistent_compute_cancellation_fixture(queue, 6969, None, Some(7));
            let expected_binding = prepared.binding;
            let mut dispatch =
                super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(
                    7,
                )
                .expect("fixture admits a production-reachable recycled predecessor");

            let failure = session
                .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
                    session.submit_fixed_dispatch_inner_classified_with_test_owner(
                        &mut dispatch,
                        |generation| test_completion_template(queue, generation),
                        |_, packets| {
                            assert_eq!(packets.packet_count(), 1);
                            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                                NativeAqlSubmissionErrorV1::Ring(occupancy),
                            ))
                        },
                    )
                })
                .expect_err("exact pre-side-effect occupancy must restore retry custody");
            assert!(matches!(
                failure.error(),
                ComputeAqlQueueSessionErrorV1::Native("submission ring occupancy")
            ));
            let (_, retryable) = failure.into_parts();
            let retryable = retryable.expect("outer persistent receipt is restored");
            assert_eq!(retryable.binding, expected_binding);
            assert_eq!(dispatch.predecessor_generation(), 7);
            assert_eq!(dispatch.last_cancelled_generation(), Some(8));
            assert!(matches!(
                dispatch.active_generation(),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            let attachment = session
                .single_persistent_compute_attachment_v1()
                .expect("persistent attachment is restored");
            assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
            assert!(matches!(
                attachment.single_entry().unwrap().state,
                PersistentComputeUseStateV1::Prepared(_)
            ));
            assert!(!session.terminal_poisoned);

            let _published = session
                .submit_directional_persistent_fixed_dispatch_v1_using(retryable, |session| {
                    session.submit_fixed_dispatch_inner_classified_with_test_owner(
                        &mut dispatch,
                        |generation| test_completion_template(queue, generation),
                        |_, packets| {
                            assert_eq!(packets.packet_count(), 1);
                            Ok(64)
                        },
                    )
                })
                .expect("the unchanged persistent receipt succeeds exactly once on retry");
            assert!(matches!(dispatch.active_generation(), Ok(9)));
            let attachment = session
                .single_persistent_compute_attachment_v1()
                .expect("successful retry retains published attachment custody");
            assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
            assert!(matches!(
                attachment.single_entry().unwrap().state,
                PersistentComputeUseStateV1::Published(_)
            ));
            assert!(!session.terminal_poisoned);
        }
    }

    #[test]
    fn persistent_submit_panic_resumes_payload_and_retains_terminal_phase() {
        assert!(!take_persistent_unwind_process_gate_record_v1());
        let queue = test_queue_key(174, 1);
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 0x5210, None, Some(7));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = session.submit_directional_persistent_fixed_dispatch_v1_using(
                prepared,
                |_| -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
                    std::panic::panic_any("r52-persistent-submit-panic")
                },
            );
        }))
        .expect_err("persistent submit envelope must resume the original payload");
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"r52-persistent-submit-panic")
        );
        assert!(take_persistent_unwind_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        let attachment = session
            .single_persistent_compute_attachment_v1()
            .expect("panic retains the attachment");
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(attachment.terminal_custody.is_none());
    }

    #[test]
    fn persistent_typed_native_callback_panic_has_no_claimed_native_stage() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(178, 1);
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 0x5211, None, Some(7));
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(7)
                .unwrap();
        let failure = session
            .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
                session.submit_fixed_dispatch_inner_classified_with_test_owner(
                    &mut dispatch,
                    |generation| test_completion_template(queue, generation),
                    |_, _| {
                        Err(NativeAqlSubmissionFailureV1::Terminal(
                            NativeAqlSubmissionErrorV1::CallbackPanic,
                        ))
                    },
                )
            })
            .expect_err("typed native callback panic is a terminal failure");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Native("submission callback panic")
        ));
        let (_, retryable) = failure.into_parts();
        assert!(retryable.is_none());
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        let attachment = session.single_persistent_compute_attachment_v1().unwrap();
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(attachment.terminal_custody.is_none());
    }

    #[test]
    fn persistent_normal_poll_identity_failure_process_gates_after_custody_consumption() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(182, 1);
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 0x5212, None, Some(7));
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(7)
                .unwrap();
        let published = session
            .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
                session.submit_fixed_dispatch_inner_classified_with_test_owner(
                    &mut dispatch,
                    |generation| test_completion_template(queue, generation),
                    |_, _| Ok(65),
                )
            })
            .unwrap();

        let failure = match session.poll_directional_persistent_fixed_dispatch_v1(published) {
            Err(failure) => failure,
            Ok(_) => panic!("the foreign dispatch owner identity must fail after consumption"),
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
            )
        ));
        assert_eq!(failure.retained_stage(), None);
        let (_, custody) = failure.into_parts();
        assert!(matches!(
            custody,
            crate::Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(_)
        ));
        assert!(session.terminal_poisoned);
        assert!(take_dispatch_terminal_process_gate_record_v1());
        let attachment = session.single_persistent_compute_attachment_v1().unwrap();
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Published(_))
        ));
    }

    #[test]
    fn persistent_normal_recycle_identity_failure_process_gates_after_custody_consumption() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(183, 1);
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 0x5213, None, Some(7));
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(7)
                .unwrap();
        let published = session
            .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
                session.submit_fixed_dispatch_inner_classified_with_test_owner(
                    &mut dispatch,
                    |generation| test_completion_template(queue, generation),
                    |_, _| Ok(66),
                )
            })
            .unwrap();
        let Gfx942PersistentComputeDispatchV1 { binding, batch, .. } = published;
        let (completion, identity) = unwrap_published(batch);
        let completion = session
            .completion_owner
            .complete_one_without_native_for_test(completion);
        let completed = wrap_completed(completion, identity);
        let attachment = session
            .single_persistent_compute_attachment_mut_v1()
            .unwrap();
        let entry = attachment.single_entry_mut().unwrap();
        let state = core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined);
        let PersistentComputeUseStateV1::Published(published_use) = state else {
            panic!("successful submit retains a published allocation lease")
        };
        entry.state = PersistentComputeUseStateV1::Completed(
            entry.allocation.owner.complete(published_use).unwrap(),
        );

        let failure = match session.recycle_directional_persistent_fixed_dispatch_v1(
            Gfx942CompletedPersistentComputeDispatchV1 {
                binding,
                completed,
                thread_affinity: PhantomData,
            },
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("the foreign dispatch owner identity must fail after consumption"),
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
            )
        ));
        assert_eq!(failure.retained_stage(), None);
        let (_, custody) = failure.into_parts();
        assert!(matches!(
            custody,
            crate::Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(_)
        ));
        assert!(session.terminal_poisoned);
        assert!(take_dispatch_terminal_process_gate_record_v1());
        let attachment = session.single_persistent_compute_attachment_v1().unwrap();
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Completed(_))
        ));
    }

    #[test]
    fn fixed_dispatch_observation_error_process_gates_with_a_live_sibling_epoch() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(179, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let first = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| Ok(61),
            )
            .unwrap();
        let sibling = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| Ok(62),
            )
            .unwrap();
        assert_eq!(dispatch.live_epoch_count(), 2);

        drop(first);
        dispatch.poison();
        let error = session
            .terminalize_fixed_dispatch_observation_result_v1::<()>(Err(
                Gfx942CompletionErrorV1::Observation.into(),
            ))
            .expect_err("an accepted observation failure is terminal");
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Observation)
        ));
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        assert_eq!(dispatch.live_epoch_count(), 2);
        assert!(matches!(
            dispatch.reserve_one(queue, test_completion_template(queue, 3)),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        assert!(matches!(
            dispatch.ensure_releasable(),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        drop(sibling);
    }

    #[test]
    fn fixed_dispatch_recycle_only_preserves_the_signal_pinned_retry_path() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(180, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let published = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| Ok(63),
            )
            .unwrap();
        let completed = session
            .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, published)
            .unwrap();
        let retryable = session
            .terminalize_fixed_dispatch_recycle_result_v1::<1>(Err(
                Gfx942FixedDispatchRecycleFailureV1 {
                    error: Gfx942CompletionErrorV1::SignalPinned {
                        slot: 0,
                        event_pins: 1,
                        native_reader_pins: 0,
                    }
                    .into(),
                    retryable_completed: Some(completed),
                },
            ))
            .expect_err("a pinned signal retains exact retry custody");
        assert!(retryable.retryable_completed.is_some());
        assert!(!session.terminal_poisoned);
        assert!(!take_dispatch_terminal_process_gate_record_v1());

        let mut terminal_session =
            persistent_compute_cancellation_test_session(test_queue_key(181, 1), None, None);
        let terminal = terminal_session
            .terminalize_fixed_dispatch_recycle_result_v1::<1>(Err(
                Gfx942FixedDispatchRecycleFailureV1 {
                    error: Gfx942CompletionErrorV1::Recycle.into(),
                    retryable_completed: None,
                },
            ))
            .expect_err("a consumed reset failure is terminal");
        assert!(terminal.retryable_completed.is_none());
        assert!(terminal_session.terminal_poisoned);
        assert!(take_dispatch_terminal_process_gate_record_v1());
    }

    #[test]
    fn hostile_quarantine_failure_preserves_each_exact_persistent_phase() {
        let owner = |id| {
            Gfx942PersistentDeviceAllocationV1::from_local_mapping(
                crate::shared_memory::local_mapping_for_persistent_sdma_test(id),
            )
        };
        let request = |byte_len| {
            Gfx942PersistentUseRequestV1::new(
                Gfx942PersistentOperationV1::ComputeReadWrite,
                0,
                byte_len,
            )
            .unwrap()
        };
        let mut foreign = owner(0x5220);

        let mut prepared_owner = owner(0x5221);
        let reserved = prepared_owner
            .reserve(request(prepared_owner.byte_len()), None)
            .unwrap();
        let prepared = prepared_owner.prepare(reserved).unwrap();
        assert!(matches!(
            quarantine_persistent_compute_prepared_v1(
                &mut foreign,
                prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
            ),
            PersistentComputeUseStateV1::Prepared(_)
        ));

        let mut published_owner = owner(0x5222);
        let reserved = published_owner
            .reserve(request(published_owner.byte_len()), None)
            .unwrap();
        let prepared = published_owner.prepare(reserved).unwrap();
        let published = published_owner.publish(prepared).unwrap();
        assert!(matches!(
            quarantine_persistent_compute_published_v1(
                &mut foreign,
                published,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ),
            PersistentComputeUseStateV1::Published(_)
        ));

        let mut completed_owner = owner(0x5223);
        let reserved = completed_owner
            .reserve(request(completed_owner.byte_len()), None)
            .unwrap();
        let prepared = completed_owner.prepare(reserved).unwrap();
        let published = completed_owner.publish(prepared).unwrap();
        let completed = completed_owner.complete(published).unwrap();
        assert!(matches!(
            quarantine_persistent_compute_completed_v1(
                &mut foreign,
                completed,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ),
            PersistentComputeUseStateV1::Completed(_)
        ));

        let mut recycled_owner = owner(0x5224);
        let reserved = recycled_owner
            .reserve(request(recycled_owner.byte_len()), None)
            .unwrap();
        let prepared = recycled_owner.prepare(reserved).unwrap();
        let published = recycled_owner.publish(prepared).unwrap();
        let completed = recycled_owner.complete(published).unwrap();
        assert!(matches!(
            quarantine_persistent_compute_recycled_v1(
                &mut foreign,
                completed,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            ),
            PersistentComputeUseStateV1::Recycled(_)
        ));
    }

    struct TestPersistentLedgerEntryV1 {
        owner: Gfx942PersistentDeviceAllocationV1,
        state: PersistentComputeUseStateV1,
    }

    impl PersistentComputeLedgerEntryV1 for TestPersistentLedgerEntryV1 {
        fn owner_and_state_v1(
            &mut self,
        ) -> (
            &mut Gfx942PersistentDeviceAllocationV1,
            &mut PersistentComputeUseStateV1,
        ) {
            (&mut self.owner, &mut self.state)
        }
    }

    fn prepared_test_persistent_ledger_entry_v1(id: u64) -> TestPersistentLedgerEntryV1 {
        let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(id),
        );
        let request = Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::ComputeReadWrite,
            0,
            owner.byte_len(),
        )
        .unwrap();
        let reserved = owner.reserve(request, None).unwrap();
        let prepared = owner.prepare(reserved).unwrap();
        TestPersistentLedgerEntryV1 {
            owner,
            state: PersistentComputeUseStateV1::Prepared(prepared),
        }
    }

    #[test]
    fn shared_persistent_ledger_core_is_equivalent_for_one_and_three_entries() {
        let mut single = prepared_test_persistent_ledger_entry_v1(0x5310);
        let [mut a, mut b, mut c] =
            [0x5311, 0x5312, 0x5313].map(prepared_test_persistent_ledger_entry_v1);

        assert!(publish_persistent_compute_entries_v1([&mut single]));
        assert!(publish_persistent_compute_entries_v1([
            &mut a, &mut b, &mut c
        ]));
        assert!(matches!(
            single.state,
            PersistentComputeUseStateV1::Published(_)
        ));
        assert!(
            [&a, &b, &c]
                .into_iter()
                .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
        );

        assert!(complete_persistent_compute_entries_v1([&mut single]));
        assert!(complete_persistent_compute_entries_v1([
            &mut a, &mut b, &mut c
        ]));
        assert!(recycle_persistent_compute_entries_v1([&mut single]));
        assert!(recycle_persistent_compute_entries_v1([
            &mut a, &mut b, &mut c
        ]));
        quarantine_persistent_compute_entries_v1(
            [&mut single],
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        quarantine_persistent_compute_entries_v1(
            [&mut a, &mut b, &mut c],
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        assert!(matches!(
            single.state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(
            [&a, &b, &c]
                .into_iter()
                .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Quarantined))
        );
        let quarantined_live_uses = single.owner.live_use_count();
        assert!(
            [&a, &b, &c]
                .into_iter()
                .all(|entry| entry.owner.live_use_count() == quarantined_live_uses)
        );

        let mut single_cancel = prepared_test_persistent_ledger_entry_v1(0x5314);
        let [mut cancel_a, mut cancel_b, mut cancel_c] =
            [0x5315, 0x5316, 0x5317].map(prepared_test_persistent_ledger_entry_v1);
        assert!(cancel_persistent_compute_prepublication_entries_v1([
            &mut single_cancel
        ]));
        assert!(cancel_persistent_compute_prepublication_entries_v1([
            &mut cancel_a,
            &mut cancel_b,
            &mut cancel_c,
        ]));
        assert_eq!(single_cancel.owner.live_use_count(), 0);
        assert!(
            [&cancel_a, &cancel_b, &cancel_c]
                .into_iter()
                .all(|entry| entry.owner.live_use_count() == 0)
        );
    }

    #[test]
    fn mocked_facade_retains_and_recycles_a_b_c_on_one_immutable_wait_for_prior_recipe() {
        let queue = test_queue_key(172, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let (a, b, c) = {
            let mut submit = |packet_id| {
                session
                    .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                        &mut dispatch,
                        |generation| {
                            let template = test_completion_template(queue, generation);
                            assert_eq!(
                                template.ordering_for_test(),
                                fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior
                            );
                            template
                        },
                        |_, packets| {
                            assert_eq!(packets.packet_count(), 1);
                            Ok(packet_id)
                        },
                    )
                    .unwrap()
            };
            (submit(41), submit(42), submit(43))
        };
        assert_eq!(dispatch.live_epoch_count(), 3);
        let b = session
            .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, b)
            .unwrap();
        session
            .recycle_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, b)
            .unwrap();
        assert_eq!(dispatch.live_epoch_count(), 2);
        let c = session
            .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, c)
            .unwrap();
        session
            .recycle_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, c)
            .unwrap();
        let a = session
            .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, a)
            .unwrap();
        session
            .recycle_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, a)
            .unwrap();
        assert_eq!(dispatch.live_epoch_count(), 0);
        assert!(!session.terminal_poisoned);
    }

    #[test]
    fn public_auxiliary_lane_unwind_restores_lane_then_terminalizes_session() {
        assert!(!take_lane_unwind_process_gate_record_v1());
        let primary = test_queue_key(175, 1);
        let auxiliary = test_queue_key(176, 1);
        let mut session = persistent_compute_cancellation_test_session(primary, None, None);
        session
            .auxiliary_compute_lanes
            .push(AuxiliaryComputeLaneSlotV1 {
                generation: 7,
                state: Some(compute_lane_state_for_multi_inflight_test(auxiliary)),
            });
        let lane = ComputeAqlQueueLaneV1 {
            session: primary,
            ordinal: 1,
            generation: 7,
        };
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = session.with_compute_lane_v1(lane, |lane_dispatch| {
                let _published = lane_dispatch
                    .session
                    .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                        &mut dispatch,
                        |generation| test_completion_template(auxiliary, generation),
                        |_, _| Ok(51),
                    )
                    .unwrap();
                std::panic::panic_any("r52-lane-after-publication")
            });
        }))
        .expect_err("public lane callback panic must be resumed");
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"r52-lane-after-publication")
        );
        assert!(take_lane_unwind_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        assert_eq!(session.key, primary);
        assert_eq!(
            session.auxiliary_compute_lanes[0]
                .state
                .as_ref()
                .expect("auxiliary lane is restored before unwind")
                .key,
            auxiliary
        );
        assert_eq!(dispatch.live_epoch_count(), 1);
        assert!(matches!(
            session.with_compute_lane_v1(lane, |_| ()),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            ))
        ));
        assert!(session.destroy().is_err());
    }

    #[test]
    fn typed_native_callback_panic_terminalizes_without_rust_unwind_payload() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(177, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let failure = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| {
                    Err(NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::CallbackPanic,
                    ))
                },
            )
            .expect_err("typed lower callback panic is terminal, not a Rust unwind");
        assert!(matches!(
            failure,
            FixedDispatchSubmissionFailureV1::Terminal(ComputeAqlQueueSessionErrorV1::Native(
                "submission callback panic"
            ))
        ));
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        assert_eq!(dispatch.live_epoch_count(), 1);
        assert!(matches!(
            session.submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| panic!("terminal session must not call native submission"),
            ),
            Err(FixedDispatchSubmissionFailureV1::Terminal(
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::Poisoned
                )
            ))
        ));
    }

    #[test]
    fn completion_signal_full_is_retryable_without_native_effect_and_retries_after_recycle() {
        let queue = test_queue_key(173, 1);
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        let full = session
            .completion_owner
            .fill_all_signals_for_test(test_completion_template(queue, 9));
        let full_snapshot = session.completion_owner.state_snapshot_for_test();
        let mut dispatch =
            super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
        let native_called = std::cell::Cell::new(false);

        let failure = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| {
                    native_called.set(true);
                    Ok(9000)
                },
            )
            .expect_err("a full completion arena must reject before native submission");
        assert!(matches!(
            failure,
            FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Completion(
                    Gfx942CompletionErrorV1::InsufficientSignals
                )
            )
        ));
        assert!(!native_called.get());
        assert_eq!(
            session.completion_owner.state_snapshot_for_test(),
            full_snapshot
        );
        assert_eq!(dispatch.next_generation(), 2);
        assert_eq!(dispatch.live_epoch_count(), 0);
        assert!(!session.terminal_poisoned);

        let source_failure = session
            .submit_with_dependency_events_classified_v1([test_completion_template(queue, 2)], 1, 1)
            .expect_err("dependency source capacity failure is retryable");
        assert!(matches!(
            source_failure,
            FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Completion(
                    Gfx942CompletionErrorV1::InsufficientSignals
                )
            )
        ));
        assert_eq!(
            session.completion_owner.state_snapshot_for_test(),
            full_snapshot
        );
        assert!(!session.terminal_poisoned);

        session
            .completion_owner
            .complete_and_recycle_all_for_test(full);
        let _retry = session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, packets| {
                    assert_eq!(packets.packet_count(), 1);
                    Ok(9001)
                },
            )
            .expect("recycled completion capacity accepts the burned successor identity");
        assert_eq!(dispatch.live_epoch_count(), 1);
        assert_eq!(dispatch.next_generation(), 3);

        let production = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let target_capacity = production
            .split("fn submit_fixed_dispatch_with_dependencies_operation_v1")
            .nth(1)
            .unwrap()
            .split("let target = match")
            .next()
            .unwrap();
        assert!(target_capacity.contains("Err(Gfx942CompletionErrorV1::InsufficientSignals)"));
        assert!(target_capacity.contains("cancel_dependency_dispatch_generation_v1(identity)"));
        assert!(target_capacity.contains("return Err(retry("));
    }

    #[test]
    fn prepared_persistent_compute_cancellation_restores_initialized_rebind_input() {
        let queue = test_queue_key(161, 1);
        let digest = [0xa5; 32];
        let (mut session, prepared, storage_identity) =
            prepared_persistent_compute_cancellation_fixture(queue, 6161, Some(digest), Some(7));

        let input = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("exact prepared cancellation must restore persistent input");
        let Gfx942PersistentComputeInputV1::Initialized(ready) = input else {
            panic!("initialized attachment must cancel back to initialized input")
        };
        assert_eq!(ready.authenticated_sha256(), digest);
        let allocation = ready.into_allocation();
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .expect("cancellation restored local native custody")
                .storage_identity(),
            storage_identity
        );
        assert!(allocation.owner.local_native_is_attached_for_sdma());
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
        assert!(session.persistent_compute.is_none());
        assert!(session.dispatch.is_none());
        assert_eq!(session.detached_dispatch_generation, Some(7));
        assert_eq!(session.detached_next_insertion_index, Some(0));
        assert_eq!(session.detached_data_count, 0);
        assert!(session.detached_data_identities.is_empty());
        let input = Gfx942PersistentComputeInputV1::from_parts(allocation, Some(digest), true);
        assert!(preserve_persistent_compute_bind_input_for_sdma_quiescence_v1(input, true).is_ok());
    }

    #[test]
    fn single_uninitialized_write_bind_cancellation_preserves_uninitialized_custody() {
        let queue = test_queue_key(189, 1);
        let (mut session, prepared, storage_identity) =
            prepared_persistent_compute_cancellation_fixture_with_initialization_v1(
                queue, 8989, None, None, false,
            );
        let attachment = session
            .single_persistent_compute_attachment_v1()
            .expect("fixture retains one prepared attachment");
        assert!(!attachment.single_entry().unwrap().fully_initialized);
        assert_eq!(
            attachment.single_entry().unwrap().effect,
            Gfx942PersistentComputeEffectV1::Write
        );

        let input = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("prepublication cancellation restores exact write-only input");
        let Gfx942PersistentComputeInputV1::Uninitialized(allocation) = input else {
            panic!("uninitialized write custody must not be promoted during cancellation")
        };
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .expect("cancellation reattached native storage")
                .storage_identity(),
            storage_identity
        );
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
        assert!(session.persistent_compute.is_none());
        assert!(session.dispatch.is_none());
        assert_eq!(session.detached_dispatch_generation, Some(0));
        assert_eq!(session.detached_data_count, 0);
    }

    #[test]
    fn prepared_replay_cancellation_preserves_initialized_after_dispatch_without_digest() {
        let queue = test_queue_key(165, 1);
        let (mut session, prepared, storage_identity) =
            prepared_persistent_compute_cancellation_fixture(queue, 6565, None, Some(7));

        let input = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("exact replay cancellation must restore initialized input");
        let Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) = input else {
            panic!("digest-free initialized replay must remain fully initialized")
        };
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .expect("cancellation restored local native custody")
                .storage_identity(),
            storage_identity
        );
        assert_eq!(session.detached_dispatch_generation, Some(7));
        assert_eq!(session.detached_next_insertion_index, Some(0));
    }

    #[test]
    fn prepared_initial_cancellation_records_never_published_detached_generation() {
        let queue = test_queue_key(166, 1);
        let digest = [0xd8; 32];
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 6666, Some(digest), None);

        let input = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("initial prepared cancellation must restore its input");
        assert!(matches!(
            input,
            Gfx942PersistentComputeInputV1::Initialized(_)
        ));
        assert_eq!(session.detached_dispatch_generation, Some(0));
        assert_eq!(session.detached_next_insertion_index, Some(0));
        assert_eq!(session.detached_data_count, 0);
        assert!(session.detached_data_identities.is_empty());
    }

    #[test]
    fn exact_prepared_persistent_submit_structural_failure_is_terminal_and_opaque() {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(167, 1);
        let (mut session, prepared, _) = prepared_persistent_compute_cancellation_fixture(
            queue,
            6767,
            Some([0xe9; 32]),
            Some(7),
        );

        let failure = session
            .submit_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("missing retained dispatch owner is a terminal structural failure");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, retryable) = failure.into_parts();
        assert!(retryable.is_none());
        assert!(session.terminal_poisoned);
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert_eq!(session.persistent_compute_terminal_stage_v1(), None);
        let attachment = session
            .single_persistent_compute_attachment_v1()
            .expect("terminal submit retains the exact attachment");
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(attachment.terminal_custody.is_none());
        assert!(matches!(
            session.release_retained_persistent_fixed_dispatch_control_v1(),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            ))
        ));
    }

    #[test]
    fn foreign_prepared_persistent_submit_receipt_remains_recoverable() {
        let producer_key = test_queue_key(168, 1);
        let receiver_key = test_queue_key(169, 1);
        let (mut producer, prepared, _) = prepared_persistent_compute_cancellation_fixture(
            producer_key,
            6868,
            Some([0xfa; 32]),
            Some(7),
        );
        let mut receiver = persistent_compute_cancellation_test_session(receiver_key, None, None);

        let failure = receiver
            .submit_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("foreign receipt must be rejected before local attachment consumption");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, retryable) = failure.into_parts();
        let prepared = retryable.expect("foreign receipt remains owned by its producer");
        assert!(!receiver.terminal_poisoned);
        assert!(receiver.persistent_compute.is_none());
        receiver.terminal_poisoned = true;
        let failure = receiver
            .submit_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("terminal foreign receiver cannot absorb another queue's receipt");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        let (_, retryable) = failure.into_parts();
        let prepared = retryable.expect("terminal foreign receiver returns exact custody");
        assert!(
            producer
                .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
                .is_ok()
        );
    }

    #[test]
    fn foreign_prepared_cancellation_receipt_retries_on_its_producer() {
        let producer_key = test_queue_key(162, 1);
        let receiver_key = test_queue_key(163, 1);
        let digest = [0xb6; 32];
        let (mut producer, prepared, storage_identity) =
            prepared_persistent_compute_cancellation_fixture(
                producer_key,
                6262,
                Some(digest),
                Some(7),
            );
        let mut receiver = persistent_compute_cancellation_test_session(receiver_key, None, None);

        let failure = receiver
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("a live foreign queue must reject the exact prepared receipt");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(prepared) = custody
        else {
            panic!("live foreign rejection must return the exact prepared receipt")
        };
        receiver.terminal_poisoned = true;
        let failure = receiver
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("a terminal foreign queue must still return the exact receipt");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(prepared) = custody
        else {
            panic!("terminal foreign rejection must return the exact prepared receipt")
        };
        let input = producer
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("the producer must accept the unchanged prepared receipt");
        let (allocation, observed_digest, initialized) = input.into_parts();
        assert!(initialized);
        assert_eq!(observed_digest, Some(digest));
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .expect("producer cancellation restored local native custody")
                .storage_identity(),
            storage_identity
        );
        assert!(allocation.owner.local_native_is_attached_for_sdma());
    }

    #[test]
    fn terminal_self_owned_prepared_cancellation_is_absorbed_opaquely() {
        let queue = test_queue_key(164, 1);
        let (mut session, prepared, _) = prepared_persistent_compute_cancellation_fixture(
            queue,
            6464,
            Some([0xc7; 32]),
            Some(7),
        );
        session.terminal_poisoned = true;

        let failure = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect_err("terminal producer must absorb its prepared receipt");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
        assert_eq!(
            session.persistent_compute_terminal_stage_v1(),
            Some(crate::Gfx942PersistentComputeTerminalStageV1::Attached)
        );
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(terminal) =
            custody
        else {
            panic!("self-owned terminal receipt must not return retryable authority")
        };
        assert_eq!(terminal.stage(), None);
        let attachment = session
            .single_persistent_compute_attachment_v1()
            .expect("terminal queue retains the exact attachment");
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Attached)
        ));
    }

    #[test]
    fn legacy_detached_only_gate_blocks_every_sdma_publication_and_both_directions() {
        let modes = [
            SdmaPublicationModeV1::Persistent,
            SdmaPublicationModeV1::DirectionalCopy(Gfx942PersistentSdmaDirectionV1::HostToDevice),
            SdmaPublicationModeV1::DirectionalCopy(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
            SdmaPublicationModeV1::DirectionalWindow(Gfx942PersistentSdmaDirectionV1::HostToDevice),
            SdmaPublicationModeV1::DirectionalWindow(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
            SdmaPublicationModeV1::SameDeviceWindow,
            SdmaPublicationModeV1::Ordinary,
            SdmaPublicationModeV1::OrdinaryBatch,
            SdmaPublicationModeV1::StripedBatch,
            SdmaPublicationModeV1::ExecuteBatch,
        ];
        for mode in modes {
            assert!(matches!(
                admit_sdma_publication_while_compute_detached(false, true, mode),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            assert!(matches!(
                admit_sdma_publication_while_compute_detached(false, false, mode),
                Ok(observed) if observed == mode
            ));
            assert!(matches!(
                admit_sdma_publication_while_compute_detached(true, true, mode),
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
        }
    }

    #[test]
    fn ordinary_sdma_publication_gate_returns_exact_buffer_custody() {
        let queue = test_queue_key(151, 1);
        let (source, destination) = crate::sdma::persistent_sdma_buffers_for_test(queue, 5151);
        let source_identity = source.storage_identity();
        let destination_identity = destination.storage_identity();
        let failure = preserve_ordinary_sdma_publication_custody_v1(true, source, destination)
            .expect_err("persistent compute must block ordinary SDMA publication");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, recovered) = failure.into_parts();
        let (source, destination) = recovered.expect("preflight returns both exact buffers");
        assert_eq!(source.storage_identity(), source_identity);
        assert_eq!(destination.storage_identity(), destination_identity);
    }

    #[test]
    fn combined_sdma_finish_recovers_only_timeout_after_closing_currentness() {
        let timeout = ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout);
        let terminal = ComputeAqlQueueSessionErrorV1::Contract("SDMA wait failure");

        assert_eq!(
            classify_sdma_batch_execution_finish(None, true),
            Gfx942SdmaBatchExecutionFinishV1::Success
        );
        assert_eq!(
            classify_sdma_batch_execution_finish(Some(&timeout), true),
            Gfx942SdmaBatchExecutionFinishV1::RecoverableTimeout
        );
        assert_eq!(
            classify_sdma_batch_execution_finish(Some(&timeout), false),
            Gfx942SdmaBatchExecutionFinishV1::Terminal
        );
        assert_eq!(
            classify_sdma_batch_execution_finish(None, false),
            Gfx942SdmaBatchExecutionFinishV1::Terminal
        );
        assert_eq!(
            classify_sdma_batch_execution_finish(Some(&terminal), true),
            Gfx942SdmaBatchExecutionFinishV1::Terminal
        );
    }

    #[test]
    fn barrier_probe_backings_have_stable_distinct_contracts() {
        let logical_bytes = 64 * 1024;
        assert_eq!(
            QueueRingBackingV1::AqlSpecial.observation(),
            Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX
        );
        assert_eq!(
            QueueRingBackingV1::ExecutableProbe.observation(),
            Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX
        );
        assert_eq!(
            QueueRingBackingV1::UserptrProbe.observation(),
            Gfx942BarrierProbeRingBackingV1::UserptrOneX
        );
        assert_eq!(QueueRingBackingV1::AqlSpecial.digest_tag(), 1);
        assert_eq!(QueueRingBackingV1::ExecutableProbe.digest_tag(), 2);
        assert_eq!(QueueRingBackingV1::UserptrProbe.digest_tag(), 3);
        assert_eq!(
            QueueRingBackingV1::AqlSpecial.gpu_va_bytes(logical_bytes),
            u64::from(logical_bytes)
        );
        assert_eq!(
            QueueRingBackingV1::ExecutableProbe.gpu_va_bytes(logical_bytes),
            u64::from(logical_bytes)
        );
        assert_eq!(
            QueueRingBackingV1::UserptrProbe.gpu_va_bytes(logical_bytes),
            u64::from(logical_bytes)
        );
    }

    #[test]
    fn queue_identity_digest_binds_backing_and_exact_ring_span() {
        let vm = fe2o3_runtime_model::VmKeyV1 {
            device: fe2o3_runtime_model::DeviceKeyV1 {
                physical: fe2o3_runtime_model::PhysicalDeviceIdV1(7),
                generation: fe2o3_runtime_model::DeviceGenerationV1(11),
            },
            id: fe2o3_runtime_model::VmIdV1(13),
        };
        let queue = QueueKeyV1 {
            vm,
            id: QueueInstanceIdV1(17),
            generation: QueueGenerationV1(19),
        };
        let mappings = core::array::from_fn(|index| fe2o3_runtime_model::MemoryMappingKeyV1 {
            allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
                vm,
                id: fe2o3_runtime_model::AllocationIdV1(index as u64 + 1),
                generation: fe2o3_runtime_model::AllocationGenerationV1(1),
            },
            id: fe2o3_runtime_model::MappingIdV1(index as u64 + 21),
        });
        let special = digest_id(
            b"plan",
            queue,
            &mappings,
            QueueRingBackingV1::AqlSpecial,
            65_536,
            131_072,
        );
        let executable = digest_id(
            b"plan",
            queue,
            &mappings,
            QueueRingBackingV1::ExecutableProbe,
            65_536,
            65_536,
        );
        let hostile_span = digest_id(
            b"plan",
            queue,
            &mappings,
            QueueRingBackingV1::ExecutableProbe,
            65_536,
            131_072,
        );
        let userptr = digest_id(
            b"plan",
            queue,
            &mappings,
            QueueRingBackingV1::UserptrProbe,
            65_536,
            65_536,
        );
        assert_ne!(special, executable);
        assert_ne!(special, userptr);
        assert_ne!(executable, userptr);
        assert_ne!(executable, hostile_span);
    }

    #[test]
    fn create_result_is_nonterminal_only_when_no_effect_is_explicit() {
        let no_effect = map_create(NativeQueueAdapterErrorV1::BackendFailedNoEffect(
            NativeQueueOperationV1::Create,
        ));
        assert!(!no_effect.is_terminal_creation());

        for error in [
            NativeQueueAdapterErrorV1::ProcessChanged,
            NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Create),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "hostile output",
            ),
            NativeQueueAdapterErrorV1::ModelProjection,
        ] {
            assert!(map_create(error).is_terminal_creation());
        }
    }

    #[test]
    fn probe_creation_phase_preserves_backing_and_terminal_classification() {
        for backing in [
            Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX,
            Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX,
        ] {
            let ordinary = barrier_probe_creation_failure(
                ComputeAqlQueueSessionErrorV1::Contract("pre-create"),
                backing,
            );
            assert!(matches!(
                ordinary,
                Gfx942BarrierProbeFailureV1::Creation { .. }
            ));
            assert_eq!(ordinary.backing(), backing);

            let terminal = barrier_probe_creation_failure(
                terminal_creation(
                    "post-create",
                    ComputeAqlQueueSessionErrorV1::Contract("fault injection"),
                ),
                backing,
            );
            assert!(matches!(
                terminal,
                Gfx942BarrierProbeFailureV1::TerminalCreation { .. }
            ));
            assert_eq!(terminal.backing(), backing);
        }
    }

    #[test]
    fn userptr_inner_creation_failure_is_always_terminal() {
        const CHILD_ENV: &str = "FE2O3_TEST_USERPTR_CREATION_GATE_POISON";
        if std::env::var_os(CHILD_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("userptr_inner_creation_failure_is_always_terminal")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let teardown_arm = arm_process_global_kfd_runtime_gate_for_teardown_v1();
        let failure = barrier_probe_creation_failure(
            ComputeAqlQueueSessionErrorV1::Contract("pre-create USERPTR failure"),
            Gfx942BarrierProbeRingBackingV1::UserptrOneX,
        );
        assert!(matches!(
            failure,
            Gfx942BarrierProbeFailureV1::TerminalCreation { .. }
        ));
        assert_eq!(
            failure.backing(),
            Gfx942BarrierProbeRingBackingV1::UserptrOneX
        );
        teardown_arm.confirm_destroyed();

        use std::os::fd::AsFd;
        let file = std::fs::File::open("/dev/null").unwrap();
        assert!(matches!(
            LinuxKfdRuntimeEnabledV1::enable(file.as_fd(), std::process::id()),
            Err(LinuxDoorbellErrorV1::Runtime(
                "process-global gate poisoned"
            ))
        ));
    }

    #[test]
    fn userptr_control_entry_failure_is_terminal_for_every_queue_backing() {
        const CHILD_ENV: &str = "FE2O3_TEST_USERPTR_CONTROL_GATE_POISON";
        if std::env::var_os(CHILD_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("userptr_control_entry_failure_is_terminal_for_every_queue_backing")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let teardown_arm = arm_process_global_kfd_runtime_gate_for_teardown_v1();
        for backing in [
            Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX,
            Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX,
            Gfx942BarrierProbeRingBackingV1::UserptrOneX,
        ] {
            let error = construction_primary::terminal_control_failure_for_test(
                ComputeAqlQueueSessionErrorV1::Contract("control fault injection"),
            );
            assert!(error.is_terminal_creation());
            let failure = barrier_probe_creation_failure(error, backing);
            assert!(matches!(
                failure,
                Gfx942BarrierProbeFailureV1::TerminalCreation { .. }
            ));
            assert_eq!(failure.backing(), backing);
        }
        teardown_arm.confirm_destroyed();

        use std::os::fd::AsFd;
        let file = std::fs::File::open("/dev/null").unwrap();
        assert!(matches!(
            LinuxKfdRuntimeEnabledV1::enable(file.as_fd(), std::process::id()),
            Err(LinuxDoorbellErrorV1::Runtime(
                "process-global gate poisoned"
            ))
        ));
    }

    #[test]
    fn barrier_probe_poll_bound_rejects_before_device_consumption() {
        assert_eq!(
            Gfx942BarrierProbePollBoundV1::new(0),
            Err(Gfx942BarrierProbePollBoundErrorV1::Zero)
        );
        assert_eq!(Gfx942BarrierProbePollBoundV1::new(1).unwrap().get(), 1);
        assert_eq!(
            Gfx942BarrierProbePollBoundV1::new(Gfx942BarrierProbePollBoundV1::maximum())
                .unwrap()
                .get(),
            Gfx942BarrierProbePollBoundV1::maximum()
        );
        assert_eq!(
            Gfx942BarrierProbePollBoundV1::new(Gfx942BarrierProbePollBoundV1::maximum() + 1),
            Err(Gfx942BarrierProbePollBoundErrorV1::ExceedsMaximum {
                requested: Gfx942BarrierProbePollBoundV1::maximum() + 1,
                maximum: Gfx942BarrierProbePollBoundV1::maximum(),
            })
        );
    }

    #[test]
    fn timeout_capture_always_precedes_terminal_poison() {
        #[derive(Default)]
        struct State {
            poisoned: bool,
            observed_before_poison: bool,
        }

        for observation in [Ok(7_u8), Err(11_u8)] {
            let mut state = State::default();
            let result = observe_then_poison(
                &mut state,
                |state| {
                    state.observed_before_poison = !state.poisoned;
                    observation
                },
                |state| state.poisoned = true,
            );
            assert_eq!(result, observation);
            assert!(state.observed_before_poison);
            assert!(state.poisoned);
        }
    }

    #[test]
    fn timeout_memory_errors_preserve_currentness_classification() {
        for error in [
            MemorySessionError::ProcessChanged,
            MemorySessionError::ProcessVmStatePoisoned,
            MemorySessionError::SharedSessionQuarantined,
        ] {
            assert_eq!(
                map_timeout_memory_observation_error(error),
                Gfx942CompletionErrorV1::Currentness
            );
        }
        assert_eq!(
            map_timeout_memory_observation_error(MemorySessionError::InvalidAllocationAuthority),
            Gfx942CompletionErrorV1::Observation
        );
    }

    #[derive(Clone, Copy)]
    struct BarrierSnapshotInput {
        packet_count: u16,
        write: u64,
        read: u64,
        header: u16,
        setup: u16,
        kind: i64,
        signal: Gfx942TimeoutSignalObservationV1,
        reason: u64,
    }

    impl BarrierSnapshotInput {
        fn valid(read: u64) -> Self {
            Self {
                packet_count: 1,
                write: 1,
                read,
                header: fe2o3_aql::AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
                setup: 0,
                kind: fe2o3_aql::AMD_SIGNAL_KIND_USER_V1,
                signal: Gfx942TimeoutSignalObservationV1::Completed,
                reason: 0,
            }
        }

        fn observation(self) -> Gfx942TimeoutExecutionObservationV1 {
            Gfx942TimeoutExecutionObservationV1::new(
                self.packet_count,
                self.write,
                self.read,
                self.header,
                self.setup,
                self.kind,
                self.signal,
                self.reason,
            )
        }
    }

    #[test]
    fn barrier_success_snapshot_accepts_only_the_exact_redacted_contract() {
        for read in [0, 1] {
            for header in [
                fe2o3_aql::AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
                fe2o3_aql::AQL_INVALID_PACKET_HEADER_V1,
            ] {
                let observation = BarrierSnapshotInput {
                    header,
                    ..BarrierSnapshotInput::valid(read)
                }
                .observation();
                assert_eq!(
                    validate_barrier_probe_success_snapshot(observation),
                    Ok(observation)
                );
            }
        }

        let valid = BarrierSnapshotInput::valid(1);
        let hostile = [
            BarrierSnapshotInput {
                packet_count: 2,
                ..valid
            }
            .observation(),
            BarrierSnapshotInput { write: 2, ..valid }.observation(),
            BarrierSnapshotInput { read: 2, ..valid }.observation(),
            BarrierSnapshotInput {
                header: 0x1402,
                ..valid
            }
            .observation(),
            BarrierSnapshotInput { setup: 1, ..valid }.observation(),
            BarrierSnapshotInput { kind: 0, ..valid }.observation(),
            BarrierSnapshotInput {
                signal: Gfx942TimeoutSignalObservationV1::Pending,
                ..valid
            }
            .observation(),
            BarrierSnapshotInput { reason: 1, ..valid }.observation(),
        ];
        for observation in hostile {
            assert_eq!(
                validate_barrier_probe_success_snapshot(observation),
                Err(Gfx942CompletionErrorV1::Observation)
            );
        }
    }

    fn detached_preflight(
        dispatch_attached: bool,
        data_count: usize,
        generation: Option<u64>,
        identity_count: usize,
        returned_count: usize,
        identity_mismatch: Option<usize>,
    ) -> DetachedReturningDestroyPreflightV1 {
        DetachedReturningDestroyPreflightV1 {
            dispatch_attached,
            detached_data_count: data_count,
            detached_dispatch_generation: generation,
            detached_identity_count: identity_count,
            returned_data_count: returned_count,
            identity_mismatch,
        }
    }

    #[test]
    fn detached_returning_destroy_observes_exact_private_generation() {
        let mut poisoned = false;
        let generation = admit_detached_returning_destroy(
            &mut poisoned,
            detached_preflight(false, 3, Some(17), 3, 3, None),
        )
        .unwrap();
        assert_eq!(generation, 17);
        assert!(!poisoned);
    }

    #[test]
    fn detached_returning_destroy_preserves_never_published_generation_zero() {
        let mut poisoned = false;
        let generation = admit_detached_returning_destroy(
            &mut poisoned,
            detached_preflight(false, 0, Some(0), 0, 0, None),
        )
        .unwrap();
        assert_eq!(generation, 0);
        assert!(!poisoned);
    }

    #[test]
    fn detached_returning_destroy_rejects_fabricated_unbound_phase() {
        for (dispatch_attached, generation) in [(true, None), (false, None)] {
            let mut poisoned = false;
            let error = admit_detached_returning_destroy(
                &mut poisoned,
                detached_preflight(dispatch_attached, 0, generation, 0, 0, None),
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            ));
            assert!(poisoned);
        }
    }

    #[test]
    fn detached_returning_destroy_cardinality_mismatch_is_terminal() {
        let mut poisoned = false;
        let error = admit_detached_returning_destroy(
            &mut poisoned,
            detached_preflight(false, 4, Some(29), 4, 3, None),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::InvalidData {
                    index: 3,
                    detail: "detached returning-destroy cardinality",
                }
            )
        ));
        assert!(poisoned);
        assert!(matches!(
            admit_detached_returning_destroy(
                &mut poisoned,
                detached_preflight(false, 4, Some(29), 4, 4, None),
            ),
            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            ))
        ));
    }

    #[test]
    fn detached_storage_identity_rejects_cross_queue_substitution_and_reordering() {
        let first_queue = [(1_u64, 11_u64), (1, 12), (1, 13)];
        let second_queue = [(2_u64, 11_u64), (2, 12), (2, 13)];
        assert_eq!(
            first_ordered_identity_mismatch(&first_queue, &second_queue),
            Some(0)
        );

        let reordered = [first_queue[1], first_queue[0], first_queue[2]];
        assert_eq!(
            first_ordered_identity_mismatch(&first_queue, &reordered),
            Some(0)
        );

        let substituted = [first_queue[0], second_queue[1], first_queue[2]];
        assert_eq!(
            first_ordered_identity_mismatch(&first_queue, &substituted),
            Some(1)
        );

        let mut poisoned = false;
        let error = admit_detached_returning_destroy(
            &mut poisoned,
            detached_preflight(false, 3, Some(11), 3, 3, Some(1)),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::InvalidData {
                    index: 1,
                    detail: "detached returning-destroy storage identity",
                }
            )
        ));
        assert!(poisoned);
    }

    #[test]
    fn detached_storage_identity_replacement_preserves_the_released_ordinal() {
        let mut identities = vec![11_u64, 12, 13];
        let removed_index = 1;
        assert_eq!(identities.remove(removed_index), 12);
        let mut next_insertion_index = Some(removed_index);

        insert_detached_identity(&mut identities, &mut next_insertion_index, 22);

        assert_eq!(identities, [11, 22, 13]);
        assert_eq!(next_insertion_index, None);
        assert_eq!(
            first_ordered_identity_mismatch(&identities, &[11, 22, 13]),
            None
        );
    }

    #[test]
    fn borrowed_initialization_queue_rejects_before_native_model_loan() {
        for replacement in [false, true] {
            for mode in 0..7 {
                let mut session = persistent_compute_cancellation_test_session(
                    test_queue_key(771, 1),
                    None,
                    None,
                );
                session.detached_dispatch_generation = Some(7);
                session.detached_next_insertion_index = Some(0);
                match mode {
                    0 => session.terminal_poisoned = true,
                    1 => session.detached_dispatch_generation = None,
                    2 => session.detached_data_count = 1,
                    3 => {
                        session.detached_data_count =
                            super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 + 1
                    }
                    4 => {
                        session.detached_data_count =
                            super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1;
                        let identity = Gfx942FixedDispatchDataV1::host_visible_uninitialized(
                            crate::shared_memory::mapped_host_for_persistent_sdma_test(1, 4096),
                        )
                        .storage_identity();
                        session.detached_data_identities =
                            vec![identity; session.detached_data_count];
                    }
                    5 => session.detached_next_insertion_index = Some(1),
                    _ => session.detached_next_insertion_index = None,
                }
                let before = (
                    session.detached_data_count,
                    session.detached_dispatch_generation,
                    session.detached_next_insertion_index,
                    session.detached_data_identities.clone(),
                );
                let source = [0x5a; 9];
                let error = if replacement {
                    session.initialize_host_visible_fixed_dispatch_data_from_slice_v1(&source)
                } else {
                    session.insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
                        1, &source,
                    )
                }
                .unwrap_err();
                match mode {
                    0 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::Poisoned
                        )
                    )),
                    1 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )),
                    2 | 3 | 5 => {
                        assert!(matches!(error, ComputeAqlQueueSessionErrorV1::Contract(_)))
                    }
                    4 => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::DataLeaseCount { .. }
                        )
                    )),
                    _ if replacement => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )),
                    _ => assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::InvalidData { .. }
                        )
                    )),
                }
                assert_eq!(
                    before,
                    (
                        session.detached_data_count,
                        session.detached_dispatch_generation,
                        session.detached_next_insertion_index,
                        session.detached_data_identities.clone()
                    )
                );
                assert!(session.engine.is_none());
                assert_eq!(source, [0x5a; 9]);
            }
        }
    }

    #[test]
    fn borrowed_initialization_queue_wiring_keeps_guards_retake_and_identity_order() {
        let source = include_str!("queue_live/fixed_dispatch.rs");
        for (name, next, guard, record) in [
            (
                "insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1",
                "initialize_host_visible_fixed_dispatch_data",
                "require_new_detached_data_index",
                "record_new_detached_data_at",
            ),
            (
                "initialize_host_visible_fixed_dispatch_data_from_slice_v1",
                "insert_host_visible_fixed_dispatch_data",
                "detached_next_insertion_index.is_none()",
                "record_new_detached_data",
            ),
        ] {
            let start = format!("pub fn {name}(");
            let end = format!("pub fn {next}(");
            let body = source
                .split(&start)
                .nth(1)
                .unwrap()
                .split(&end)
                .next()
                .unwrap();
            let loan = body.find("self.with_live_queue_memory_model(").unwrap();
            for preflight in [
                "require_unbound_fixed_dispatch",
                "require_detached_allocation_capacity",
                guard,
            ] {
                assert!(body.find(preflight).unwrap() < loan);
            }
            assert!(body.contains("initialize_host_visible_coherent_from_slice_v1(bytes)"));
            assert!(body.find(record).unwrap() > body.find("Ok(memory)").unwrap());
            assert!(body.contains("self.poison_terminal()"));
            for forbidden in ["submit_", "to_vec(", "to_owned(", "Box::from("] {
                assert!(!body.contains(forbidden));
            }
        }
    }

    #[test]
    fn exact_detached_insertion_clears_legacy_replacement_state_at_any_valid_ordinal() {
        let mut same_ordinal = vec![11_u64, 13];
        let mut pending = Some(1);
        validate_new_detached_data_index(same_ordinal.len(), 1).unwrap();
        insert_detached_identity_at(&mut same_ordinal, &mut pending, 12, 1);
        assert_eq!(same_ordinal, [11, 12, 13]);
        assert_eq!(pending, None);

        let mut different_ordinal = vec![21_u64, 23];
        let mut pending = Some(1);
        validate_new_detached_data_index(different_ordinal.len(), 0).unwrap();
        insert_detached_identity_at(&mut different_ordinal, &mut pending, 20, 0);
        assert_eq!(different_ordinal, [20, 21, 23]);
        assert_eq!(pending, None);
    }

    #[test]
    fn exact_detached_insertion_rejects_out_of_range_without_mutation() {
        let identities = vec![31_u64, 32];
        let pending = Some(0);
        assert!(matches!(
            validate_new_detached_data_index(identities.len(), 3),
            Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: 2,
                detail: "detached insertion ordinal",
            })
        ));
        assert_eq!(identities, [31, 32]);
        assert_eq!(pending, Some(0));
    }

    #[test]
    fn fused_async_single_prepublication_failure_script_is_fail_closed() {
        for loan_succeeded in [false, true] {
            for owner_healthy in [false, true] {
                for closing_currentness_succeeded in [false, true] {
                    assert_eq!(
                        fused_async_single_prepublication_is_retryable_v1(
                            loan_succeeded,
                            owner_healthy,
                            closing_currentness_succeeded,
                        ),
                        loan_succeeded && owner_healthy && closing_currentness_succeeded,
                        "loan={loan_succeeded} owner={owner_healthy} close={closing_currentness_succeeded}",
                    );
                }
            }
        }
    }

    #[test]
    fn persistent_sdma_active_spin_floor_is_bound_to_the_frozen_manifest() {
        assert_eq!(
            PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1,
            Duration::from_nanos(50_000)
        );
        assert!(crate::sdma::GFX942_SDMA_COPY_MANIFEST_V1.lines().any(|line| {
            line == "persistent-sdma-wait-policy=elapsed-active-spin-floor:50000ns,checked-add-and-clamp-to-deadline,attempts-counted-during-floor,exact-floor-boundary-resumes-default-adaptive-stage,first-observation-unconditional;scope=directional-persistent-single,directional-persistent-window,same-device-persistent-window;excluded=ordinary-directional,generic-striped,fused-synchronous,xgmi,persistent-compute"
        }));
    }

    #[test]
    fn session_manifest_digest_is_frozen() {
        assert_eq!(
            GFX942_COMPUTE_AQL_SHARED_ALLOCATION_RECORDS_V1,
            1 + 1 + 1 + 1 + 1
        );
        assert_eq!(
            fe2o3_aql::AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1,
            "82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0"
        );
        assert_eq!(
            fe2o3_aql::AQL_BARRIER_AND_ABI_SCHEMA_MANIFEST_SHA256_V1,
            "bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5"
        );
        assert_eq!(
            fe2o3_aql::AQL_FIXED_BATCH_MODEL_MANIFEST_SHA256_V2,
            "a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a"
        );
        assert_eq!(
            super::super::completion::GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1,
            "485b21257623afce41573b27922350f125bcd7f5d339f8a89cf9a2c7c6ca77f1"
        );
        assert!(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1.contains(&format!(
            "compute_dependency_publisher_schema_sha256={}\n",
            super::super::dependency::GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_SHA256_V1
        )));
        assert_eq!(
            super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1,
            "854c96e2293317e3e70879b7af332ea953f6edfe00329f45b6f1b70dc743cb6d"
        );
        assert!(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1.contains(&format!(
            "dispatch_binding_schema_sha256={}\n",
            super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1
        )));
        assert_eq!(
            SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
            "026c8c05b6388149765ccb84a95739de6a6ddbbe89577217284b18bdcfdcbdd3"
        );
        assert_eq!(
            GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
            "37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02"
        );
        assert_eq!(
            fe2o3_kfd_uapi::KFD_USERPTR_MEMORY_SCHEMA_MANIFEST_SHA256,
            "c1cee09bdf884d2c14a5dbb89c1f6f7885962c75b1457caf412821490919ee9e"
        );
        assert_eq!(
            fe2o3_kfd_uapi::KFD_USERPTR_QUEUE_CONTROL_SCHEMA_MANIFEST_SHA256,
            "f1d75410d6bfacff2ea15ecfff226eb8aed7912ee324a36b8ed8550fa52bce02"
        );
        assert_eq!(
            fe2o3_kfd_uapi::KFD_RUNTIME_ENABLE_SCHEMA_SHA256,
            "fa47481b10ea4bd89438d10b82bd8197088906e55f5f0c827dc7aa5aba906288"
        );
        let digest = Sha256::digest(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1);
    }

    #[derive(Clone, Copy, Debug)]
    enum InjectedPostHandoffFailureV1 {
        EventCreation,
        ShadowInstallation,
        ShadowInitialization,
        ResourceSealing,
        ResourceMapping,
        ModelTransfer,
        EngineAdmission,
        SubmissionModel,
        QueueCreateNoEffect,
        QueueCreateIndeterminate,
        RuntimeQueueTransition,
        CreateOutputs,
        NativeQueueId,
        SessionComposition,
        PreDoorbellCurrentness,
        DoorbellMapping,
        PostDoorbellCurrentness,
    }

    #[test]
    fn every_post_handoff_failure_keeps_original_debug_token_empty() {
        for injected in [
            InjectedPostHandoffFailureV1::EventCreation,
            InjectedPostHandoffFailureV1::ShadowInstallation,
            InjectedPostHandoffFailureV1::ShadowInitialization,
            InjectedPostHandoffFailureV1::ResourceSealing,
            InjectedPostHandoffFailureV1::ResourceMapping,
            InjectedPostHandoffFailureV1::ModelTransfer,
            InjectedPostHandoffFailureV1::EngineAdmission,
            InjectedPostHandoffFailureV1::SubmissionModel,
            InjectedPostHandoffFailureV1::QueueCreateNoEffect,
            InjectedPostHandoffFailureV1::QueueCreateIndeterminate,
            InjectedPostHandoffFailureV1::RuntimeQueueTransition,
            InjectedPostHandoffFailureV1::CreateOutputs,
            InjectedPostHandoffFailureV1::NativeQueueId,
            InjectedPostHandoffFailureV1::SessionComposition,
            InjectedPostHandoffFailureV1::PreDoorbellCurrentness,
            InjectedPostHandoffFailureV1::DoorbellMapping,
            InjectedPostHandoffFailureV1::PostDoorbellCurrentness,
        ] {
            let mut token_runtime = Some("runtime");
            let mut token_control = Some("control");
            let terminal_runtime = token_runtime.take();
            let terminal_control = token_control.take();

            assert_eq!(terminal_runtime, Some("runtime"), "{injected:?}");
            assert_eq!(terminal_control, Some("control"), "{injected:?}");
            assert!(token_runtime.is_none(), "{injected:?}");
            assert!(token_control.is_none(), "{injected:?}");
        }
    }

    #[test]
    fn production_handoff_precedes_event_create_and_all_fallible_queue_steps() {
        let source = include_str!("queue_live/construction_primary.rs");
        let runtime = source
            .split("fn prepare_runtime_and_shadows(")
            .nth(1)
            .unwrap()
            .split("fn map_and_retain_resources(")
            .next()
            .unwrap();
        let enable = runtime.find("E::enable_runtime").unwrap();
        let handoff = runtime
            .find("runtime.take().expect(\"validated debug runtime authority\")")
            .unwrap();
        let arm = runtime.find("E::arm_creation(memory)").unwrap();
        let event = runtime.find("E::create_event(memory)").unwrap();
        let shadows = runtime.find("E::install_shadows").unwrap();
        assert!(enable < handoff && handoff < arm && arm < event && event < shadows);
        assert!(!runtime.contains("publish_for_native_queue_creation"));

        let construct = source
            .split("pub(super) fn construct(")
            .nth(1)
            .unwrap()
            .split("fn prepare_runtime_and_shadows(")
            .next()
            .unwrap();
        let control_entry = construct
            .find("entry.enter(\"USERPTR queue-control creation\")")
            .unwrap();
        let control = construct
            .find("memory.allocate_userptr_aql_control()")
            .unwrap();
        let runtime = construct.find("self.prepare_runtime_and_shadows(").unwrap();
        let mapping = construct.find("self.map_and_retain_resources(").unwrap();
        let create = construct.find("self.create_and_assemble(").unwrap();
        let doorbell = construct.find("self.finish_doorbell()?").unwrap();
        let finish = construct.find("E::finish_creation(").unwrap();
        assert!(
            control_entry < control
                && control < runtime
                && runtime < mapping
                && mapping < create
                && create < doorbell
                && doorbell < finish
        );

        let create = source
            .split("fn create_and_assemble(")
            .nth(1)
            .unwrap()
            .split("fn finish_doorbell(")
            .next()
            .unwrap();
        let native = create.find(".create_at_native_boundary(key").unwrap();
        let publish = create.find("E::publish_shadows(").unwrap();
        let dependency = create.find("E::create_dependency_owner(key)").unwrap();
        let assembled = create
            .find("self.completed = Some(CompletedPrimaryV1")
            .unwrap();
        assert!(native < publish && publish < dependency && dependency < assembled);
        assert_eq!(create.matches("E::publish_shadows").count(), 1);
        let commit = &create[assembled..];
        assert!(!commit.contains('?'));

        let doorbell = source
            .split("fn finish_doorbell(")
            .nth(1)
            .unwrap()
            .split("fn run_rooted_construction_v1")
            .next()
            .unwrap();
        assert!(doorbell.contains("self.completed.as_mut()"));
        assert_eq!(doorbell.matches(".prepare_operation()").count(), 2);
        let map = doorbell.find("E::map_doorbell").unwrap();
        let observation = doorbell.find("E::doorbell_observation").unwrap();
        assert!(doorbell.find("session.doorbell = Some(").unwrap() < map && map < observation);
        let platform = include_str!("queue_live/construction_primary/environment.rs");
        for primitive in [
            "LinuxKfdRuntimeEnabledV1::enable",
            "LinuxQueueExceptionEventV1::create",
            "LinuxCwsrShadowPagesV1::install",
            "LinuxDoorbellSliceV1::map",
            "arm.finish_checked(pid)",
            "ComputeDependencySessionOwnerV1::new(key.id.0)",
        ] {
            assert!(platform.contains(primitive));
        }
        let conversion = platform.split("fn into_session(self)").nth(1).unwrap();
        for forbidden in [
            "?",
            "check_currentness",
            "prepare_operation",
            "::map(",
            "::allocate",
            "::enable(",
        ] {
            assert!(
                !conversion.contains(forbidden),
                "fallible post-gate conversion: {forbidden}"
            );
        }
    }

    #[test]
    fn auxiliary_queue_publishes_cwsr_payload_only_at_native_create_boundary() {
        let source = include_str!("queue_live/construction_auxiliary.rs");
        let prepared_state = source
            .split("struct AuxiliaryConstructionV1")
            .nth(1)
            .unwrap()
            .split("struct AuxiliaryConstructionScopeV1")
            .next()
            .unwrap();
        assert!(prepared_state.contains("unpublished: Option<E::Unpublished>"));
        assert!(!prepared_state.contains("exception: QueueExceptionStateV1"));

        let body = source
            .split("fn prepare(")
            .nth(1)
            .unwrap()
            .split("pub(super) fn construct_auxiliary_compute_lane_v1")
            .next()
            .unwrap();
        let install = body
            .find("self.unpublished = Some(E::install_shadows")
            .unwrap();
        let restore = body.find("E::restore_shadow_write").unwrap();
        let native_boundary = body.find("create_at_native_boundary(key").unwrap();
        let publish = body
            .find("self.published = Some(E::publish_shadows(")
            .unwrap();
        let published_exception = body.find("exception: Some(QueueExceptionStateV1").unwrap();
        assert!(install < restore);
        assert!(restore < native_boundary);
        assert!(native_boundary < publish);
        assert!(publish < published_exception);
        assert_eq!(body.matches("E::publish_shadows(").count(), 1);
        assert!(!body.contains("engine.create(key)"));
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RetainedReplayInjectedStageV1 {
        MappedFacts,
        Detach,
        AuthenticatedConstruction,
        Retain,
        FinalAudit,
    }

    struct RetainedReplayScriptV1 {
        fail: Option<RetainedReplayInjectedStageV1>,
        panic: bool,
        trace: Vec<RetainedReplayInjectedStageV1>,
    }

    impl RetainedReplayScriptV1 {
        fn observe(
            &mut self,
            stage: RetainedReplayInjectedStageV1,
        ) -> Result<(), RetainedReplayInjectedStageV1> {
            self.trace.push(stage);
            if self.fail == Some(stage) {
                if self.panic {
                    std::panic::panic_any(stage);
                }
                return Err(stage);
            }
            Ok(())
        }
    }

    struct RetainedReplayScriptRequestV1(u64);
    struct RetainedReplayScriptStorageV1(u64);
    struct RetainedReplayScriptDataV1(u64);
    struct RetainedReplayScriptAttachedV1(u64);

    type RetainedReplayScriptOutcomeV1 = PersistentRetainedControlReplayPipelineOutcomeV1<
        RetainedReplayScriptRequestV1,
        RetainedReplayScriptStorageV1,
        RetainedReplayScriptDataV1,
        RetainedReplayScriptAttachedV1,
        RetainedReplayInjectedStageV1,
    >;
    type RetainedReplayScriptCustodyV1 = PersistentRetainedControlReplayPipelineCustodyV1<
        RetainedReplayScriptRequestV1,
        RetainedReplayScriptStorageV1,
        RetainedReplayScriptDataV1,
        RetainedReplayScriptAttachedV1,
    >;

    fn execute_retained_replay_script_v1(
        script: &mut RetainedReplayScriptV1,
        phases: &mut RetainedReplayScriptCustodyV1,
    ) -> Result<(), RetainedReplayInjectedStageV1> {
        use PersistentRetainedControlReplayPipelineCustodyV1 as Phase;
        execute_persistent_retained_control_replay_pipeline_v1(
            script,
            phases,
            |script, phases| {
                let Phase::Input(request) = phases else {
                    panic!("input phase")
                };
                assert_eq!(request.0, 0x35);
                script.observe(RetainedReplayInjectedStageV1::MappedFacts)
            },
            |script, phases| {
                let Phase::Input(request) = phases else {
                    panic!("input phase")
                };
                assert_eq!(request.0, 0x35);
                script.observe(RetainedReplayInjectedStageV1::Detach)?;
                let Phase::Input(request) = core::mem::replace(phases, Phase::Empty) else {
                    unreachable!()
                };
                *phases = Phase::Storage(RetainedReplayScriptStorageV1(request.0));
                Ok(())
            },
            |script, phases| {
                let Phase::Storage(storage) = phases else {
                    panic!("storage phase")
                };
                assert_eq!(storage.0, 0x35);
                script.observe(RetainedReplayInjectedStageV1::AuthenticatedConstruction)?;
                let Phase::Storage(storage) = core::mem::replace(phases, Phase::Empty) else {
                    unreachable!()
                };
                *phases = Phase::Data(RetainedReplayScriptDataV1(storage.0));
                Ok(())
            },
            |script, phases| {
                let Phase::Data(data) = phases else {
                    panic!("data phase")
                };
                assert_eq!(data.0, 0x35);
                script.observe(RetainedReplayInjectedStageV1::Retain)?;
                let Phase::Data(data) = core::mem::replace(phases, Phase::Empty) else {
                    unreachable!()
                };
                *phases = Phase::Attached(RetainedReplayScriptAttachedV1(data.0));
                Ok(())
            },
            |script, phases| {
                let Phase::Attached(attached) = phases else {
                    panic!("attached phase")
                };
                assert_eq!(attached.0, 0x35);
                script.observe(RetainedReplayInjectedStageV1::FinalAudit)
            },
        )
    }

    fn run_retained_replay_script_v1(
        fail: Option<RetainedReplayInjectedStageV1>,
    ) -> (
        RetainedReplayScriptOutcomeV1,
        Vec<RetainedReplayInjectedStageV1>,
    ) {
        let mut script = RetainedReplayScriptV1 {
            fail,
            panic: false,
            trace: Vec::new(),
        };
        let mut phases = RetainedReplayScriptCustodyV1::Input(RetainedReplayScriptRequestV1(0x35));
        let result = execute_retained_replay_script_v1(&mut script, &mut phases);
        (phases.into_outcome(result), script.trace)
    }

    #[test]
    fn retained_control_replay_panics_keep_the_current_phase_outside_the_loan() {
        let stages = [
            RetainedReplayInjectedStageV1::MappedFacts,
            RetainedReplayInjectedStageV1::Detach,
            RetainedReplayInjectedStageV1::AuthenticatedConstruction,
            RetainedReplayInjectedStageV1::Retain,
            RetainedReplayInjectedStageV1::FinalAudit,
        ];
        for (index, stage) in stages.into_iter().enumerate() {
            let mut script = RetainedReplayScriptV1 {
                fail: Some(stage),
                panic: true,
                trace: Vec::new(),
            };
            let mut phases =
                RetainedReplayScriptCustodyV1::Input(RetainedReplayScriptRequestV1(0x35));
            let mut retakes = 0;
            let mut poisoned = false;
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute_live_model_custody_v1(
                    &mut script,
                    |_| Ok::<_, ()>(()),
                    |script| {
                        let _ = execute_retained_replay_script_v1(script, &mut phases);
                    },
                    |_, ()| {
                        retakes += 1;
                        std::panic::panic_any("secondary retake");
                    },
                    |_| poisoned = true,
                )
            }));
            assert_eq!(
                caught
                    .unwrap_err()
                    .downcast_ref::<RetainedReplayInjectedStageV1>(),
                Some(&stage)
            );
            assert_eq!(retakes, 1);
            assert!(poisoned);
            assert_eq!(script.trace, stages[..=index]);
            let outcome = phases.into_outcome(Err(stage));
            use PersistentRetainedControlReplayPipelineOutcomeV1 as Outcome;
            match (index, outcome) {
                (0 | 1, Outcome::BeforeDetach { request, .. }) => assert_eq!(request.0, 0x35),
                (2, Outcome::Storage { storage, .. }) => assert_eq!(storage.0, 0x35),
                (3, Outcome::Data { data, .. }) => assert_eq!(data.0, 0x35),
                (4, Outcome::Attached { attached, .. }) => assert_eq!(attached.0, 0x35),
                _ => panic!("panic lost the active replay phase"),
            }
        }
    }

    fn retained_replay_prepared_owner_fixture_v1(
        queue: QueueKeyV1,
        id: u64,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) {
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, id, None, Some(7));
        let input = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("fixture cancellation restores attached replay input");
        let (mut allocation, _, _) = input.into_parts();
        let request = Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::ComputeReadWrite,
            0,
            allocation.byte_len(),
        )
        .unwrap();
        let reserved = allocation.owner.reserve(request, None).unwrap();
        let prepared = allocation.owner.prepare(reserved).unwrap();
        (allocation, prepared)
    }

    fn persistent_bind_prepared_entries_for_test(
        count: usize,
    ) -> Vec<PersistentComputeAttachmentEntryV1> {
        (0..count)
            .map(|index| {
                let (allocation, prepared) = retained_replay_prepared_owner_fixture_v1(
                    test_queue_key(293, 1),
                    0x9200 + index as u64,
                );
                let storage_identity = Some(
                    allocation
                        .owner
                        .local_native_for_sdma()
                        .unwrap()
                        .storage_identity(),
                );
                PersistentComputeAttachmentEntryV1 {
                    allocation,
                    authenticated_sha256: None,
                    fully_initialized: true,
                    state: PersistentComputeUseStateV1::Prepared(prepared),
                    storage_identity,
                    effect: Gfx942PersistentComputeEffectV1::ReadWrite,
                }
            })
            .collect()
    }

    #[test]
    fn persistent_bind_early_validation_retains_each_prepared_entry_on_error_and_panic() {
        for count in [1, 3] {
            for failed in 0..count {
                for panic in [false, true] {
                    let entries = persistent_bind_prepared_entries_for_test(count);
                    let expected: Vec<_> = entries
                        .iter()
                        .map(|entry| entry.storage_identity.unwrap())
                        .collect();
                    let mut visited = 0;
                    let result = validate_persistent_bind_inputs_v1(
                        &mut visited,
                        &entries,
                        |visited, entry| {
                            let index = *visited;
                            *visited += 1;
                            assert_eq!(
                                entry
                                    .allocation
                                    .owner
                                    .local_native_for_sdma()
                                    .unwrap()
                                    .storage_identity(),
                                expected[index]
                            );
                            if index == failed {
                                if panic {
                                    std::panic::panic_any(index);
                                }
                                return Err(index);
                            }
                            Ok(())
                        },
                    );
                    assert_eq!(visited, failed + 1);
                    if panic {
                        assert_eq!(result.unwrap_err().downcast_ref::<usize>(), Some(&failed));
                    } else {
                        assert_eq!(result.unwrap(), Err(failed));
                    }
                    for (entry, identity) in entries.iter().zip(expected) {
                        assert!(matches!(
                            entry.state,
                            PersistentComputeUseStateV1::Prepared(_)
                        ));
                        assert_eq!(entry.allocation.owner.live_use_count(), 1);
                        assert_eq!(
                            entry
                                .allocation
                                .owner
                                .local_native_for_sdma()
                                .unwrap()
                                .storage_identity(),
                            identity
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn persistent_bind_terminal_retake_dominates_successful_roster_cancellation() {
        for count in [1, 3] {
            for terminal in [false, true] {
                let mut entries = persistent_bind_prepared_entries_for_test(count);
                let expected: Vec<_> = entries
                    .iter()
                    .map(|entry| entry.storage_identity.unwrap())
                    .collect();
                let mut session = persistent_compute_cancellation_test_session(
                    test_queue_key(293, 1),
                    None,
                    None,
                );
                let (operation, retake) = execute_live_model_custody_v1(
                    &mut session,
                    |_| Ok(()),
                    |_| Err::<(), _>("early validation"),
                    |_, ()| {
                        if terminal {
                            Err("closing retake")
                        } else {
                            Ok(())
                        }
                    },
                    |session| session.poison_terminal(),
                )
                .unwrap();
                assert_eq!(operation, Err("early validation"));
                assert_eq!(retake.is_err(), terminal);
                let cancelled = if count == 1 {
                    cancel_persistent_compute_prepublication_entries_v1([&mut entries[0]])
                } else {
                    let [a, b, c] = entries.as_mut_slice() else {
                        unreachable!()
                    };
                    cancel_persistent_compute_prepublication_entries_v1([a, b, c])
                };
                assert!(cancelled);
                assert_eq!(
                    persistent_bind_retryable_v1(!session.terminal_poisoned, cancelled),
                    !terminal
                );
                for (entry, identity) in entries.iter().zip(expected) {
                    assert_eq!(entry.allocation.owner.live_use_count(), 0);
                    assert_eq!(
                        entry
                            .allocation
                            .owner
                            .local_native_for_sdma()
                            .unwrap()
                            .storage_identity(),
                        identity
                    );
                }
                assert!(session.dispatch.is_none());
            }
        }
    }

    #[test]
    fn retained_control_replay_script_executes_the_production_pipeline_and_preserves_stage_custody()
    {
        let stages = [
            RetainedReplayInjectedStageV1::MappedFacts,
            RetainedReplayInjectedStageV1::Detach,
            RetainedReplayInjectedStageV1::AuthenticatedConstruction,
            RetainedReplayInjectedStageV1::Retain,
            RetainedReplayInjectedStageV1::FinalAudit,
        ];
        for (index, failed_stage) in stages.into_iter().enumerate() {
            let (outcome, trace) = run_retained_replay_script_v1(Some(failed_stage));
            assert_eq!(trace, stages[..=index]);
            match (failed_stage, outcome) {
                (
                    RetainedReplayInjectedStageV1::MappedFacts
                    | RetainedReplayInjectedStageV1::Detach,
                    PersistentRetainedControlReplayPipelineOutcomeV1::BeforeDetach {
                        request,
                        error,
                    },
                ) => {
                    assert_eq!(request.0, 0x35);
                    assert_eq!(error, failed_stage);
                }
                (
                    RetainedReplayInjectedStageV1::AuthenticatedConstruction,
                    PersistentRetainedControlReplayPipelineOutcomeV1::Storage { storage, error },
                ) => {
                    assert_eq!(storage.0, 0x35);
                    assert_eq!(error, failed_stage);
                }
                (
                    RetainedReplayInjectedStageV1::Retain,
                    PersistentRetainedControlReplayPipelineOutcomeV1::Data { data, error },
                ) => {
                    assert_eq!(data.0, 0x35);
                    assert_eq!(error, failed_stage);
                }
                (
                    RetainedReplayInjectedStageV1::FinalAudit,
                    PersistentRetainedControlReplayPipelineOutcomeV1::Attached { attached, error },
                ) => {
                    assert_eq!(attached.0, 0x35);
                    assert_eq!(error, failed_stage);
                }
                _ => panic!("injected replay stage returned the wrong custody"),
            }
        }

        let (outcome, trace) = run_retained_replay_script_v1(None);
        assert_eq!(trace, stages);
        let PersistentRetainedControlReplayPipelineOutcomeV1::Ready(attached) = outcome else {
            panic!("clean replay pipeline must reach Ready")
        };
        assert_eq!(attached.0, 0x35);
    }

    #[test]
    fn retained_control_replay_loan_resolution_distinguishes_open_retake_and_ready() {
        let unopened: PersistentRetainedControlReplayLoanResolutionV1<
            RetainedReplayScriptRequestV1,
            RetainedReplayScriptAttachedV1,
            &'static str,
        > = resolve_persistent_retained_control_replay_loan_v1(
            Some(RetainedReplayScriptRequestV1(0x41)),
            None,
            Err("loan open"),
            || "missing",
        );
        let PersistentRetainedControlReplayLoanResolutionV1::Unopened { request, error } = unopened
        else {
            panic!("unopened loan must retain input custody")
        };
        assert_eq!(request.0, 0x41);
        assert_eq!(error, "loan open");

        for loan in [Ok(()), Err("loan retake")] {
            let resolution: PersistentRetainedControlReplayLoanResolutionV1<
                RetainedReplayScriptRequestV1,
                RetainedReplayScriptAttachedV1,
                &'static str,
            > = resolve_persistent_retained_control_replay_loan_v1(
                None,
                Some(RetainedReplayScriptAttachedV1(0x42)),
                loan,
                || "missing",
            );
            let PersistentRetainedControlReplayLoanResolutionV1::Executed {
                outcome,
                retake_error,
            } = resolution
            else {
                panic!("executed loan must preserve its move-only outcome")
            };
            assert_eq!(outcome.0, 0x42);
            assert_eq!(retake_error, loan.err());
        }
    }

    #[test]
    fn retained_control_replay_terminal_custody_variants_preserve_exact_native_stage() {
        let queue = test_queue_key(181, 1);
        let (mut storage_allocation, storage_prepared) =
            retained_replay_prepared_owner_fixture_v1(queue, 8181);
        let storage_identity = storage_allocation
            .owner
            .local_native_for_sdma()
            .unwrap()
            .storage_identity();
        let storage = storage_allocation
            .owner
            .detach_local_native_for_compute(&storage_prepared)
            .unwrap();
        let custody = PersistentComputeTerminalNativeCustodyV1::Storage(
            Gfx942SdmaBufferStorageV1::Device(storage),
        );
        assert_eq!(
            custody.stage(),
            crate::Gfx942PersistentComputeTerminalStageV1::StorageDetached
        );
        let PersistentComputeTerminalNativeCustodyV1::Storage(Gfx942SdmaBufferStorageV1::Device(
            storage,
        )) = custody
        else {
            unreachable!()
        };
        assert_eq!(storage.storage_identity(), storage_identity);

        let (mut data_allocation, data_prepared) =
            retained_replay_prepared_owner_fixture_v1(queue, 8282);
        let data_identity = data_allocation
            .owner
            .local_native_for_sdma()
            .unwrap()
            .storage_identity();
        let data = Gfx942FixedDispatchDataV1::uninitialized(
            data_allocation
                .owner
                .detach_local_native_for_compute(&data_prepared)
                .unwrap(),
        );
        let custody = PersistentComputeTerminalNativeCustodyV1::Data(
            PersistentComputeTerminalDataV1::from_one(data),
        );
        assert_eq!(
            custody.stage(),
            crate::Gfx942PersistentComputeTerminalStageV1::DataDetached
        );
        let PersistentComputeTerminalNativeCustodyV1::Data(data) = custody else {
            unreachable!()
        };
        assert_eq!(data.len(), 1);
        assert_eq!(
            data[0].sdma_storage_identity(),
            Gfx942SdmaBufferStorageIdentityV1::Device(data_identity)
        );

        assert_eq!(
            PersistentComputeTerminalNativeCustodyV1::Attached.stage(),
            crate::Gfx942PersistentComputeTerminalStageV1::Attached
        );
    }

    #[test]
    fn retained_control_replay_cancellation_and_quarantine_preserve_prepared_authority() {
        let queue = test_queue_key(182, 1);
        let (mut exact, exact_prepared) = retained_replay_prepared_owner_fixture_v1(queue, 8383);
        let exact_sequence = exact_prepared.sequence();
        let (mut substituted, substituted_prepared) =
            retained_replay_prepared_owner_fixture_v1(queue, 8484);

        let cancellation = substituted
            .owner
            .cancel_prepared(exact_prepared)
            .expect_err("a substituted owner cannot cancel the exact replay use");
        let (_, exact_prepared) = cancellation.into_parts();
        assert_eq!(exact_prepared.sequence(), exact_sequence);
        let state = quarantine_persistent_retained_control_replay_prepared_v1(
            &mut substituted.owner,
            exact_prepared,
        );
        let PersistentComputeUseStateV1::Prepared(exact_prepared) = state else {
            panic!("failed quarantine must preserve Prepared authority")
        };
        assert_eq!(exact_prepared.sequence(), exact_sequence);
        assert_eq!(substituted.owner.quarantine_reason(), None);
        exact.owner.cancel_prepared(exact_prepared).unwrap();
        substituted
            .owner
            .cancel_prepared(substituted_prepared)
            .unwrap();

        let (mut exact, exact_prepared) = retained_replay_prepared_owner_fixture_v1(queue, 8585);
        let state = quarantine_persistent_retained_control_replay_prepared_v1(
            &mut exact.owner,
            exact_prepared,
        );
        assert!(matches!(state, PersistentComputeUseStateV1::Quarantined));
        assert_eq!(
            exact.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
        );
    }

    #[test]
    fn retained_control_replay_public_input_failure_is_retryable_only_for_clean_round_trip() {
        for (id, retryable) in [(8686, true), (8787, false)] {
            let queue = test_queue_key(183, 1);
            let (mut allocation, prepared) = retained_replay_prepared_owner_fixture_v1(queue, id);
            let expected_identity = allocation.attachment.storage_identity;
            allocation.owner.cancel_prepared(prepared).unwrap();
            let failure = persistent_retained_control_replay_input_failure_v1(
                ComputeAqlQueueSessionErrorV1::Contract("replay fault injection"),
                Gfx942PersistentComputeInputV1::Uninitialized(allocation),
                retryable,
            );
            let (_, custody) = failure.into_parts();
            let input = match (retryable, custody) {
                (true, Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input)) => input,
                (false, Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(terminal)) => {
                    assert!(terminal.retains_prebinding_input());
                    terminal
                        .input
                        .expect("terminal input custody remains exact")
                }
                _ => panic!("public replay failure returned the wrong custody class"),
            };
            let (allocation, _, _) = input.into_parts();
            assert_eq!(allocation.attachment.storage_identity, expected_identity);
        }
    }

    #[test]
    fn persistent_control_replay_uses_the_active_queue_currentness_policy() {
        let production = include_str!("queue_live/fixed_dispatch.rs");
        let replay = production
            .split("fn bind_retained_persistent_fixed_dispatch_control_replay_v1")
            .nth(1)
            .unwrap()
            .split("pub fn bind_three_binding_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        assert_eq!(replay.matches("with_live_queue_memory_model").count(), 1);
        let mapped = replay.find("phases.mapped_facts(memory)").unwrap();
        let detach = replay.find("phases.detach()").unwrap();
        let construct = replay.find("phases.construct()").unwrap();
        let retain = replay.find("phases.retain(memory)").unwrap();
        let replay_validation = replay.find("phases.audit(memory)").unwrap();
        let phases = include_str!("queue_live/persistent_bind.rs");
        for operation in [
            "mapped_gfx942_device_memory_facts",
            "detach_local_native_for_compute",
            "from_authenticated_full_transfer",
            "retain_persistent_replay_data_in_place_v1",
            "validate_persistent_replay_dispatch_memory",
        ] {
            assert!(phases.contains(operation));
        }
        assert!(!phases.contains("validate_live_queue_dispatch_memory"));
        let loan_close = replay
            .find("resolve_persistent_retained_control_replay_loan_v1")
            .unwrap();
        let commit = replay
            .find("state: PersistentComputeUseStateV1::Prepared(replay.prepared)")
            .unwrap();
        assert!(mapped < detach);
        assert!(detach < construct);
        assert!(construct < retain);
        assert!(retain < replay_validation);
        assert!(replay_validation < loan_close);
        assert!(loan_close < commit);
        assert_eq!(
            replay
                .matches("restore_model_ownership_for_live_mutation")
                .count(),
            0
        );
        assert_eq!(
            replay
                .matches("retake_model_ownership_after_live_mutation")
                .count(),
            0
        );
        assert!(replay.contains("let mut request = Some(request)"));
        assert!(
            replay.contains("let mut phases = PersistentRetainedControlReplayCustodyV1::Empty")
        );
        assert!(replay.contains("let mut pipeline_result = None"));
        assert!(replay.find("let mut phases").unwrap() < replay.find("catch_unwind").unwrap());
        assert!(replay.contains("PersistentRetainedControlReplayOutcomeV1::Ready(replay)"));
        assert!(phases.contains("PersistentComputeTerminalNativeCustodyV1::Storage"));
        assert!(phases.contains("PersistentComputeTerminalNativeCustodyV1::Data"));
        assert!(replay.contains("PersistentComputeTerminalNativeCustodyV1::Attached"));
        assert!(
            replay.contains("predecessor_dispatch_generation: Some(commit.predecessor_generation)")
        );
        assert!(replay.contains("queue: self.key"));
        assert!(replay.contains("attachment_generation: commit.attachment_generation"));
        assert!(replay.contains("self.dispatch = Some(replay.dispatch)"));
        assert!(replay.contains("self.detached_data_count = 0"));
        assert!(replay.contains("self.detached_dispatch_generation = None"));
        assert!(replay.contains("self.detached_data_identities.clear()"));
        assert!(replay.contains("self.detached_next_insertion_index = None"));
        assert!(replay.contains("storage_identity: commit.storage_identity"));
        assert!(replay.contains("effect: commit.effect"));
        assert!(replay.contains("terminal_custody: None"));
        assert!(replay.contains("self.next_persistent_compute_generation ="));
        assert!(!replay.contains("prepare_persistent_fixed_dispatch_resources_v1"));

        let bind = production
            .split("pub fn bind_directional_persistent_fixed_dispatch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn submit_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        let retained = bind
            .find("if let Some(dispatch) = self.dispatch.take()")
            .unwrap();
        let replay_call = bind
            .find("bind_retained_persistent_fixed_dispatch_control_replay_v1")
            .unwrap();
        let initial_start = bind.find("validate_persistent_bind_inputs_v1(").unwrap();
        assert!(retained < replay_call);
        assert!(replay_call < initial_start);
        let initial = &bind[initial_start..];
        assert_eq!(
            initial
                .matches("with_live_queue_memory_model(|memory|")
                .count(),
            2
        );
        assert_eq!(
            initial
                .matches("with_live_queue_memory_model_custody")
                .count(),
            0
        );
        assert_eq!(
            initial
                .matches("restore_model_ownership_for_live_mutation")
                .count(),
            0
        );
        assert_eq!(
            initial
                .matches("retake_model_ownership_after_live_mutation")
                .count(),
            0
        );
        assert_eq!(
            initial
                .matches("Self::validate_persistent_bind_preparation_v1")
                .count(),
            1
        );
        assert!(initial.contains("prepare_persistent_fixed_dispatch_resources_v1"));
        assert!(!initial.contains("validate_persistent_replay_dispatch_memory"));
        assert!(!initial.contains("retain_persistent_replay_data_in_place_v1"));

        let release = production
            .split("pub fn release_retained_persistent_fixed_dispatch_control_v1")
            .last()
            .unwrap()
            .split("fn detach_recycled_fixed_dispatch_inner")
            .next()
            .unwrap();
        let close_audit = release.find(".check_queue_currentness()").unwrap();
        let consume_control = release.find("let mut dispatch = Some(").unwrap();
        assert!(close_audit < consume_control);
        assert!(release.contains("std::panic::catch_unwind"));
        assert!(release.contains("with_live_queue_memory_model_custody"));

        let shared_memory = include_str!("shared_memory.rs");
        let operational = shared_memory
            .split("fn validate_persistent_replay_dispatch_memory")
            .nth(1)
            .unwrap()
            .split("fn map_device_memory")
            .next()
            .unwrap();
        assert!(operational.contains("self.check_operational_currentness()?"));
        assert!(operational.contains("validate_dispatch_device_memory_authorities"));
        assert!(!operational.contains("validate_complete_dispatch_device_memory_set"));
        assert!(!operational.contains("self.check_currentness()?"));

        let ordinary_rebind = production
            .split("pub fn bind_fixed_dispatch<const N: usize>")
            .last()
            .unwrap()
            .split("pub fn allocate_uninitialized_fixed_dispatch_data")
            .next()
            .unwrap();
        assert!(ordinary_rebind.contains("validate_live_queue_dispatch_memory"));
        assert!(!ordinary_rebind.contains("validate_persistent_replay_dispatch_memory"));
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CompletionRecycleScriptV1 {
        Pending,
        Ready,
        PublishedStateFailure,
        DispatchGenerationFailure,
        CompletionObservationFailure,
        DispatchCompletionFailure,
        AllocationCompletionFailure,
        SignalGenerationFailure,
        SignalResetFailure,
        ClosingCurrentnessFailure,
        RecycleCurrentnessFailure,
        RecycleInfrastructureFailure,
        DispatchRecycleFailure,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CompletionRecycleScriptCustodyV1 {
        Published(u64),
        Completed(u64),
        Recycled(u64),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct CompletionRecycleScriptFailureV1 {
        point: CompletionRecycleScriptV1,
        custody: CompletionRecycleScriptCustodyV1,
    }

    struct CompletionRecycleScriptStateV1 {
        script: CompletionRecycleScriptV1,
        trace: Vec<&'static str>,
    }

    type CompletionRecycleScriptResultV1 = Result<
        PersistentComputePollAndRecycleTransitionV1<u64, u64, u64>,
        PersistentComputePollAndRecycleTransitionFailureV1<
            CompletionRecycleScriptFailureV1,
            CompletionRecycleScriptFailureV1,
        >,
    >;

    fn execute_completion_recycle_script_v1(
        script: CompletionRecycleScriptV1,
    ) -> (CompletionRecycleScriptResultV1, Vec<&'static str>) {
        const CUSTODY_ID: u64 = 73;
        let mut state = CompletionRecycleScriptStateV1 {
            script,
            trace: Vec::new(),
        };
        let result = execute_persistent_compute_poll_and_recycle_v1(
            &mut state,
            |state| match state.script {
                CompletionRecycleScriptV1::PublishedStateFailure => {
                    state.trace.push("published-state-failure");
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Published(CUSTODY_ID),
                    })
                }
                CompletionRecycleScriptV1::DispatchGenerationFailure => {
                    state.trace.push("dispatch-generation-failure");
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Published(CUSTODY_ID),
                    })
                }
                CompletionRecycleScriptV1::CompletionObservationFailure => {
                    state
                        .trace
                        .extend(["check-a", "acquire", "observation-failure"]);
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Published(CUSTODY_ID),
                    })
                }
                CompletionRecycleScriptV1::DispatchCompletionFailure => {
                    state.trace.extend([
                        "check-a",
                        "acquire",
                        "check-b",
                        "dispatch-completion-failure",
                    ]);
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(CUSTODY_ID),
                    })
                }
                CompletionRecycleScriptV1::AllocationCompletionFailure => {
                    state.trace.extend([
                        "check-a",
                        "acquire",
                        "check-b",
                        "dispatch-completed",
                        "allocation-completion-failure",
                    ]);
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(CUSTODY_ID),
                    })
                }
                CompletionRecycleScriptV1::Pending => {
                    state.trace.extend(["check-a", "acquire", "check-b"]);
                    Ok(PersistentComputePollTransitionV1::Pending(CUSTODY_ID))
                }
                _ => {
                    state.trace.extend([
                        "check-a",
                        "acquire",
                        "check-b",
                        "dispatch-completed",
                        "allocation-completed",
                    ]);
                    Ok(PersistentComputePollTransitionV1::Ready(CUSTODY_ID))
                }
            },
            |state| {
                state.trace.push("midpoint");
                101
            },
            |state, completed| match state.script {
                CompletionRecycleScriptV1::Ready => {
                    state.trace.extend([
                        "reset",
                        "check-c",
                        "dispatch-recycled",
                        "attachment-recycled",
                    ]);
                    Ok(completed)
                }
                CompletionRecycleScriptV1::SignalGenerationFailure => {
                    state.trace.push("signal-generation-failure");
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                    })
                }
                CompletionRecycleScriptV1::SignalResetFailure => {
                    state.trace.push("reset-failure");
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                    })
                }
                CompletionRecycleScriptV1::ClosingCurrentnessFailure => {
                    state.trace.extend(["reset", "check-c-failure"]);
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                    })
                }
                CompletionRecycleScriptV1::RecycleCurrentnessFailure => {
                    state.trace.push("recycle-currentness-failure");
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                    })
                }
                CompletionRecycleScriptV1::RecycleInfrastructureFailure => {
                    state.trace.push("recycle-infrastructure-failure");
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Completed(completed),
                    })
                }
                CompletionRecycleScriptV1::DispatchRecycleFailure => {
                    state
                        .trace
                        .extend(["reset", "check-c", "dispatch-recycle-failure"]);
                    Err(CompletionRecycleScriptFailureV1 {
                        point: state.script,
                        custody: CompletionRecycleScriptCustodyV1::Recycled(completed),
                    })
                }
                _ => unreachable!("poll-stage scripts never reach recycle"),
            },
        );
        (result, state.trace)
    }

    #[test]
    fn persistent_completion_recycle_driver_executes_required_paths() {
        let (pending, trace) =
            execute_completion_recycle_script_v1(CompletionRecycleScriptV1::Pending);
        assert!(matches!(
            pending,
            Ok(PersistentComputePollAndRecycleTransitionV1::Pending(73))
        ));
        assert_eq!(trace, ["check-a", "acquire", "check-b"]);

        let (ready, trace) = execute_completion_recycle_script_v1(CompletionRecycleScriptV1::Ready);
        assert!(matches!(
            ready,
            Ok(PersistentComputePollAndRecycleTransitionV1::Recycled {
                recycled: 73,
                completion_observed_at: 101,
            })
        ));
        assert_eq!(
            trace,
            [
                "check-a",
                "acquire",
                "check-b",
                "dispatch-completed",
                "allocation-completed",
                "midpoint",
                "reset",
                "check-c",
                "dispatch-recycled",
                "attachment-recycled",
            ]
        );

        for (script, expected_trace, expected_custody) in [
            (
                CompletionRecycleScriptV1::CompletionObservationFailure,
                &["check-a", "acquire", "observation-failure"][..],
                CompletionRecycleScriptCustodyV1::Published(73),
            ),
            (
                CompletionRecycleScriptV1::SignalResetFailure,
                &[
                    "check-a",
                    "acquire",
                    "check-b",
                    "dispatch-completed",
                    "allocation-completed",
                    "midpoint",
                    "reset-failure",
                ][..],
                CompletionRecycleScriptCustodyV1::Completed(73),
            ),
            (
                CompletionRecycleScriptV1::ClosingCurrentnessFailure,
                &[
                    "check-a",
                    "acquire",
                    "check-b",
                    "dispatch-completed",
                    "allocation-completed",
                    "midpoint",
                    "reset",
                    "check-c-failure",
                ][..],
                CompletionRecycleScriptCustodyV1::Completed(73),
            ),
            (
                CompletionRecycleScriptV1::DispatchRecycleFailure,
                &[
                    "check-a",
                    "acquire",
                    "check-b",
                    "dispatch-completed",
                    "allocation-completed",
                    "midpoint",
                    "reset",
                    "check-c",
                    "dispatch-recycle-failure",
                ][..],
                CompletionRecycleScriptCustodyV1::Recycled(73),
            ),
        ] {
            let (result, trace) = execute_completion_recycle_script_v1(script);
            let failure = match result {
                Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure))
                    if script == CompletionRecycleScriptV1::CompletionObservationFailure =>
                {
                    failure
                }
                Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                    failure
                }
                _ => panic!("script returned the wrong transition"),
            };
            assert_eq!(failure.point, script);
            assert_eq!(failure.custody, expected_custody);
            assert_eq!(trace, expected_trace);
        }
    }

    #[test]
    fn persistent_completion_recycle_driver_exhausts_failure_custody_matrix() {
        for (script, expected_custody, poll_failure) in [
            (
                CompletionRecycleScriptV1::PublishedStateFailure,
                CompletionRecycleScriptCustodyV1::Published(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::DispatchGenerationFailure,
                CompletionRecycleScriptCustodyV1::Published(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::CompletionObservationFailure,
                CompletionRecycleScriptCustodyV1::Published(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::DispatchCompletionFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::AllocationCompletionFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::SignalGenerationFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::SignalResetFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::ClosingCurrentnessFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::RecycleCurrentnessFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::RecycleInfrastructureFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::DispatchRecycleFailure,
                CompletionRecycleScriptCustodyV1::Recycled(73),
                false,
            ),
        ] {
            let (result, _) = execute_completion_recycle_script_v1(script);
            let (failure, actual_poll_failure) = match result {
                Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => {
                    (failure, true)
                }
                Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                    (failure, false)
                }
                Ok(_) => panic!("failure script unexpectedly succeeded"),
            };
            assert_eq!(failure.point, script);
            assert_eq!(failure.custody, expected_custody);
            assert_eq!(actual_poll_failure, poll_failure);
        }
    }

    struct CompletionWaitRecycleScriptStateV1 {
        scripts: std::collections::VecDeque<CompletionRecycleScriptV1>,
        trace: Vec<&'static str>,
        pending_boundaries: u64,
        timeout_after_pending_boundaries: u64,
    }

    struct CompletionWaitPendingV1(u64);

    type CompletionWaitRecycleScriptResultV1 = Result<
        PersistentComputeWaitAndRecycleTransitionV1<CompletionWaitPendingV1, u64, u64>,
        PersistentComputePollAndRecycleTransitionFailureV1<
            CompletionRecycleScriptFailureV1,
            CompletionRecycleScriptFailureV1,
        >,
    >;

    fn execute_completion_wait_recycle_scripts_v1(
        pending: CompletionWaitPendingV1,
        scripts: impl IntoIterator<Item = CompletionRecycleScriptV1>,
        timeout_after_pending_boundaries: u64,
    ) -> (CompletionWaitRecycleScriptResultV1, Vec<&'static str>) {
        let mut state = CompletionWaitRecycleScriptStateV1 {
            scripts: scripts.into_iter().collect(),
            trace: Vec::new(),
            pending_boundaries: 0,
            timeout_after_pending_boundaries,
        };
        let result = execute_persistent_compute_wait_and_recycle_v1(
            &mut state,
            pending,
            |state, pending| {
                assert_eq!(pending.0, 73);
                let script = state
                    .scripts
                    .pop_front()
                    .expect("wait script retains one outcome per observation");
                let (result, trace) = execute_completion_recycle_script_v1(script);
                state.trace.extend(trace);
                result.map(|transition| match transition {
                    PersistentComputePollAndRecycleTransitionV1::Pending(pending) => {
                        PersistentComputePollAndRecycleTransitionV1::Pending(
                            CompletionWaitPendingV1(pending),
                        )
                    }
                    PersistentComputePollAndRecycleTransitionV1::Recycled {
                        recycled,
                        completion_observed_at,
                    } => PersistentComputePollAndRecycleTransitionV1::Recycled {
                        recycled,
                        completion_observed_at,
                    },
                })
            },
            |state| {
                state.pending_boundaries += 1;
                state.pending_boundaries >= state.timeout_after_pending_boundaries
            },
        );
        (result, state.trace)
    }

    #[test]
    fn persistent_completion_wait_driver_counts_zero_deadline_late_and_retry_observations() {
        let (timeout, trace) = execute_completion_wait_recycle_scripts_v1(
            CompletionWaitPendingV1(73),
            [CompletionRecycleScriptV1::Pending],
            1,
        );
        let retry_pending = match timeout {
            Ok(PersistentComputeWaitAndRecycleTransitionV1::Timeout {
                pending,
                observations: 1,
            }) => pending,
            _ => panic!("first pending observation must return exact timeout custody"),
        };
        assert_eq!(trace, ["check-a", "acquire", "check-b"]);
        assert!(!trace.contains(&"midpoint"));
        assert!(!trace.contains(&"reset"));

        for (scripts, expected_observations) in [
            (vec![CompletionRecycleScriptV1::Ready], 1),
            (
                vec![
                    CompletionRecycleScriptV1::Pending,
                    CompletionRecycleScriptV1::Ready,
                ],
                2,
            ),
            (
                vec![
                    CompletionRecycleScriptV1::Pending,
                    CompletionRecycleScriptV1::Pending,
                    CompletionRecycleScriptV1::Ready,
                ],
                3,
            ),
        ] {
            let (ready, trace) = execute_completion_wait_recycle_scripts_v1(
                CompletionWaitPendingV1(73),
                scripts,
                u64::MAX,
            );
            assert!(matches!(
                ready,
                Ok(PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                    recycled: 73,
                    completion_observed_at: 101,
                    observations,
                }) if observations == expected_observations
            ));
            assert_eq!(
                trace.iter().filter(|event| **event == "acquire").count(),
                expected_observations as usize
            );
            assert_eq!(trace.iter().filter(|event| **event == "reset").count(), 1);
        }

        let (retry, _) = execute_completion_wait_recycle_scripts_v1(
            retry_pending,
            [CompletionRecycleScriptV1::Ready],
            u64::MAX,
        );
        assert!(matches!(
            retry,
            Ok(PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                recycled: 73,
                observations: 1,
                ..
            })
        ));
    }

    #[test]
    fn persistent_completion_wait_driver_preserves_every_failure_route_and_custody() {
        for (script, expected_custody, poll_failure) in [
            (
                CompletionRecycleScriptV1::PublishedStateFailure,
                CompletionRecycleScriptCustodyV1::Published(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::DispatchGenerationFailure,
                CompletionRecycleScriptCustodyV1::Published(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::CompletionObservationFailure,
                CompletionRecycleScriptCustodyV1::Published(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::DispatchCompletionFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::AllocationCompletionFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                true,
            ),
            (
                CompletionRecycleScriptV1::SignalGenerationFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::SignalResetFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::ClosingCurrentnessFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::RecycleCurrentnessFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::RecycleInfrastructureFailure,
                CompletionRecycleScriptCustodyV1::Completed(73),
                false,
            ),
            (
                CompletionRecycleScriptV1::DispatchRecycleFailure,
                CompletionRecycleScriptCustodyV1::Recycled(73),
                false,
            ),
        ] {
            let (result, _) = execute_completion_wait_recycle_scripts_v1(
                CompletionWaitPendingV1(73),
                [script],
                u64::MAX,
            );
            let (failure, actual_poll_failure) = match result {
                Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => {
                    (failure, true)
                }
                Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                    (failure, false)
                }
                Ok(_) => panic!("failure wait script unexpectedly succeeded"),
            };
            assert_eq!(failure.point, script);
            assert_eq!(failure.custody, expected_custody);
            assert_eq!(actual_poll_failure, poll_failure);
        }
    }

    #[test]
    fn persistent_completion_wait_route_reuses_fused_poll_without_split_recycle() {
        let fixed = include_str!("queue_live/fixed_dispatch.rs");
        let production = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let wait = fixed
            .split("pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1")
            .nth(1)
            .unwrap()
            .split(
                "pub fn wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1",
            )
            .next()
            .unwrap();
        assert_eq!(
            wait.matches("poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch)")
                .count(),
            1
        );
        assert_eq!(
            wait.matches("execute_persistent_compute_wait_and_recycle_v1")
                .count(),
            1
        );
        assert!(!wait.contains("poll_directional_persistent_fixed_dispatch_v1"));
        assert!(!wait.contains("session.recycle_directional_persistent_fixed_dispatch_v1("));

        let driver = production
            .split("fn execute_persistent_compute_wait_and_recycle_v1")
            .nth(1)
            .unwrap()
            .split("fn resolve_persistent_retained_control_replay_loan_v1")
            .next()
            .unwrap();
        assert!(driver.contains("observations.saturating_add(1)"));
        assert!(driver.contains("observations == u64::MAX"));
    }

    #[test]
    fn persistent_completion_recycle_fused_route_has_one_ordered_handoff() {
        let fixed = include_str!("queue_live/fixed_dispatch.rs");
        let production = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let all_production = format!("{production}\n{fixed}");
        let fused = fixed
            .split("pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn recycle_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        assert_eq!(
            fused
                .matches("poll_completion_batch_with_current_handoff_retaining")
                .count(),
            1
        );
        assert_eq!(
            fused
                .matches("finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
                .count(),
            1
        );
        assert_eq!(
            fused
                .matches("Self::recycle_completion_current_handoff_retaining")
                .count(),
            1
        );
        assert_eq!(
            fused
                .matches("execute_persistent_compute_poll_and_recycle_v1")
                .count(),
            1
        );
        assert!(!fused.contains("recycle_completion_batch_retaining"));
        assert!(!fused.contains("Vec<"));
        assert!(!fused.contains("Box<"));
        assert!(!fused.contains("dyn "));

        let shared_poll = fixed
            .split("fn poll_directional_persistent_fixed_dispatch_inner_v1")
            .nth(1)
            .unwrap()
            .split("/// Polls and immediately recycles the exact three-binding dispatch.")
            .next()
            .unwrap();
        let preflight = shared_poll.find("let generation_is_current").unwrap();
        let observe = shared_poll.find("let observed =").unwrap();
        assert!(
            shared_poll[observe..]
                .starts_with("let observed =\n            std::panic::catch_unwind")
        );
        let dispatch_completed = shared_poll.find(".mark_completed_occurrence(").unwrap();
        let allocation_completed = shared_poll
            .find("complete_persistent_compute_entries_v1([&mut attachment])")
            .unwrap();
        assert!(preflight < observe);
        assert!(observe < dispatch_completed);
        assert!(dispatch_completed < allocation_completed);

        let driver = production
            .split("fn execute_persistent_compute_poll_and_recycle_v1")
            .nth(1)
            .unwrap()
            .split("fn resolve_persistent_retained_control_replay_loan_v1")
            .next()
            .unwrap();
        let driver_poll = driver.find("let completed").unwrap();
        let midpoint = driver.find("let completion_observed_at").unwrap();
        let driver_recycle = driver.find("let recycled = recycle").unwrap();
        assert!(driver_poll < midpoint);
        assert!(midpoint < driver_recycle);

        let shared_recycle = fixed
            .split("fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        let native_recycle = shared_recycle.find("let recycled =").unwrap();
        assert!(
            shared_recycle[native_recycle..]
                .starts_with("let recycled =\n            std::panic::catch_unwind")
        );
        let dispatch_recycled = shared_recycle.find(".mark_recycled_occurrence(").unwrap();
        let attachment_recycled = shared_recycle
            .find("recycle_persistent_compute_entries_v1([&mut attachment])")
            .unwrap();
        assert!(native_recycle < dispatch_recycled);
        assert!(dispatch_recycled < attachment_recycled);

        assert!(!all_production.contains("PersistentCompletionRecycleFailurePointV1"));
        assert!(!all_production.contains("persistent_completion_recycle_failure_stage_v1"));
        assert!(!all_production.contains("persistent_completion_recycle_terminal_custody_v1"));
        assert_eq!(
            shared_poll
                .matches("PersistentComputeTerminalNativeCustodyV1::Published")
                .count(),
            3
        );
        assert_eq!(
            shared_poll
                .matches("PersistentComputeTerminalNativeCustodyV1::Completed")
                .count(),
            2
        );
        assert_eq!(
            shared_recycle
                .matches("PersistentComputeTerminalNativeCustodyV1::Completed")
                .count(),
            1
        );
        assert_eq!(
            shared_recycle
                .matches("PersistentComputeTerminalNativeCustodyV1::Recycled")
                .count(),
            2
        );

        let split_poll = fixed
            .split("pub fn poll_directional_persistent_fixed_dispatch_v1")
            .nth(1)
            .unwrap()
            .split("fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
            .next()
            .unwrap();
        assert_eq!(
            split_poll
                .matches("poll_directional_persistent_fixed_dispatch_inner_v1")
                .count(),
            1
        );
        let split_recycle = fixed
            .split("pub fn recycle_directional_persistent_fixed_dispatch_v1")
            .nth(1)
            .unwrap()
            .split("pub fn detach_recycled_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        assert_eq!(
            split_recycle
                .matches("finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
                .count(),
            1
        );
    }

    #[test]
    fn retained_control_replay_failure_matrix_requires_an_exact_clean_round_trip() {
        for loan_succeeded in [false, true] {
            for cancellation_succeeded in [false, true] {
                for session_healthy in [false, true] {
                    let observed = classify_persistent_retained_control_replay_failure_v1(
                        PersistentRetainedControlReplayCustodyStageV1::Input,
                        loan_succeeded,
                        cancellation_succeeded,
                        session_healthy,
                    );
                    let expected = if loan_succeeded && cancellation_succeeded && session_healthy {
                        PersistentRetainedControlReplayDispositionV1::RetryableInput
                    } else if cancellation_succeeded {
                        PersistentRetainedControlReplayDispositionV1::TerminalInput
                    } else {
                        PersistentRetainedControlReplayDispositionV1::TerminalAttached
                    };
                    assert_eq!(observed, expected);
                }
            }
        }

        for (stage, expected) in [
            (
                PersistentRetainedControlReplayCustodyStageV1::Storage,
                PersistentRetainedControlReplayDispositionV1::TerminalStorage,
            ),
            (
                PersistentRetainedControlReplayCustodyStageV1::Data,
                PersistentRetainedControlReplayDispositionV1::TerminalData,
            ),
            (
                PersistentRetainedControlReplayCustodyStageV1::Attached,
                PersistentRetainedControlReplayDispositionV1::TerminalAttached,
            ),
        ] {
            for loan_succeeded in [false, true] {
                for cancellation_succeeded in [false, true] {
                    for session_healthy in [false, true] {
                        assert_eq!(
                            classify_persistent_retained_control_replay_failure_v1(
                                stage,
                                loan_succeeded,
                                cancellation_succeeded,
                                session_healthy,
                            ),
                            expected
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn retained_control_replay_authority_carriers_are_move_only_and_private() {
        let source = include_str!("queue_live.rs");
        for carrier in [
            "PersistentRetainedControlReplayRequestV1",
            "PersistentRetainedControlReplayDetachedV1",
            "PersistentRetainedControlReplayOutcomeV1",
        ] {
            assert!(
                source.contains(&format!("struct {carrier}"))
                    || source.contains(&format!("enum {carrier}"))
            );
            assert!(!source.contains(&format!("pub struct {carrier}")));
            assert!(!source.contains(&format!("pub enum {carrier}")));
            assert!(!source.contains(&format!("#[derive(Clone)]\nstruct {carrier}")));
            assert!(!source.contains(&format!("#[derive(Clone)]\nenum {carrier}")));
            assert!(!source.contains(&format!("#[derive(Clone, Copy)]\nstruct {carrier}")));
            assert!(!source.contains(&format!("#[derive(Clone, Copy)]\nenum {carrier}")));
        }
    }
}
