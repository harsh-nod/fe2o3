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
    ComputeAqlTargetProfileV1, DeviceIdentityStateV1, IdentityDigestV1, MemoryAccessV1,
    MemoryCoherenceV1, MemoryKindV1, MemoryLifecycleStateV1, QueueConfigurationIdV1,
    QueueGenerationV1, QueueInstanceIdV1, QueueKeyV1, QueuePlanIdV1,
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
use super::dispatch_binding::{
    DeviceDataAllocationInputV1, DeviceDataEffectV1, DispatchGeometryV1, DispatchResourceOwnerV1,
    Gfx942CompletedDispatchBatchV1, Gfx942CompletedDispatchReadRequestV1,
    Gfx942CompletedDispatchReadbackV1, Gfx942CompletedDispatchSnapshotRequestV1,
    Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1, Gfx942DispatchPollV1,
    Gfx942DispatchPollWithProgressV1, Gfx942FixedDispatchDataV1, Gfx942FixedDispatchPacketV1,
    Gfx942FixedDispatchStorageIdentityV1, Gfx942RecycledDispatchWriteRequestV1,
    PersistentFixedDispatchControlIdentityV1, ReturnedDispatchDataV1, TypedKernargImageV1,
    persistent_fixed_dispatch_control_identity_v1, prepare_dispatch_resources,
    prepare_persistent_fixed_dispatch_resources_v1, prepare_public_fixed_dispatch_resources,
    prepare_public_fixed_dispatch_resources_after_detach,
    prepare_public_fixed_dispatch_resources_after_recycle, unwrap_completed, unwrap_published,
    validate_fixed_batch_ring, wrap_completed, wrap_poll_with_progress, wrap_published,
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
    Gfx942PersistentQuarantineReasonV1, Gfx942PersistentUseErrorV1, Gfx942PersistentUseLeaseV1,
    Gfx942PersistentUseRequestV1, cancel_prepared_local_sdma_pair_v1,
    detach_local_native_pair_for_sdma_v1, quarantine_published_local_sdma_pair_v1,
};
use crate::persistent_compute::{
    Gfx942CompletedPersistentComputeDispatchV1, Gfx942PersistentComputeBindFailureCustodyV1,
    Gfx942PersistentComputeBindFailureV1, Gfx942PersistentComputeBindTerminalCustodyV1,
    Gfx942PersistentComputeCancelFailureV1, Gfx942PersistentComputeCompletedV1,
    Gfx942PersistentComputeDetachFailureV1, Gfx942PersistentComputeDispatchV1,
    Gfx942PersistentComputeEffectV1, Gfx942PersistentComputeExecutionFailureV1,
    Gfx942PersistentComputeInputV1, Gfx942PersistentComputePollAndRecycleFailureV1,
    Gfx942PersistentComputePollAndRecycleV1, Gfx942PersistentComputePollFailureV1,
    Gfx942PersistentComputePollV1, Gfx942PersistentComputeReadyFailureCustodyV1,
    Gfx942PersistentComputeReadyFailureV1, Gfx942PersistentComputeReadyTerminalCustodyV1,
    Gfx942PersistentComputeReadyV1, Gfx942PersistentComputeRecycleFailureV1,
    Gfx942PreparedPersistentComputeDispatchV1, Gfx942RecycledPersistentComputeDispatchV1,
    PersistentComputeAttachmentV1, PersistentComputeBindingKeyV1,
    PersistentComputeTerminalNativeCustodyV1, PersistentComputeUseStateV1,
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
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942DirectionalSdmaQueueObservationV1,
    Gfx942SdmaBufferKindV1, Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferStorageV1,
    Gfx942SdmaBufferV1, Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyPollV1, Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1, Gfx942SdmaMemoryPoolObservationV1,
    Gfx942SdmaMultiQueuePlanV1, Gfx942SdmaMultiQueueShardTicketsV1,
    Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaQueueObservationV1,
    Gfx942SdmaQueueProgressObservationV1, Gfx942SdmaQueueSetV1, Gfx942SdmaUnpublishedCopyRequestV1,
    MultiQueueSdmaSubmitFailureV1, PersistentSdmaWindowPollV1,
    PreparedPersistentSdmaWindowPublicationFailureV1, PreparedPersistentSdmaWindowV1,
    PreparedSdmaPublicationFailureV1, PreparedSingleSdmaPublicationFailureV1, PreparedSingleSdmaV1,
    SingleSdmaWaitInCurrentScopeV1, allocate_device_buffer, allocate_host_buffer,
    exact_full_host_write_is_authenticatable, persistent_sdma_window_packet_count,
    planned_ticket_matches_queue_occurrence, read_host_buffer, release_buffer,
    striped_sdma_queue_count_is_admitted, write_full_host_buffer_authenticated, write_host_buffer,
};
use crate::shared_memory::{
    AqlCompletionSignalResourceRoleV1, AqlContextSaveResourceRoleV1, AqlControlResourceRoleV1,
    AqlEndOfPipeResourceRoleV1, AqlQueueGttV1, AqlRingResourceRoleV1, ExecutableAqlQueueProbeGttV1,
    ExecutableGttV1, Gfx942DeviceMemoryIdentityV1, Gfx942DeviceMemoryLeaseV1,
    Gfx942DeviceMemoryMappedV1, Gfx942InitializedDeviceMemoryV1,
    Gfx942InitializedHostVisibleMemoryV1, GttCpuWritableV1, GttGpuAccessibleExecutableV1,
    GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1, LiveQueueModelFoundationLoanV1,
    SharedGttAllocationV1, SharedGttMappedResourceFactsV1, SharedGttMemorySessionV1,
    SharedGttQueueResourceAuthorityV1, UserptrAqlControlGttV1, UserptrAqlQueueProbeGttV1,
};
use crate::{
    CheckedGfx942XnackMinusDevice, GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
    Gfx942AqlQueueResourcePlanV1, Gfx942QueueResourcePlanningError, KfdWithAdmittedUapi,
    MemorySessionError, SHARED_GTT_MEMORY_PROFILE_SHA256_V1, plan_gfx942_aql_queue_resources,
};
use fe2o3_aql::{
    AqlCompletionObservationV1, AqlPreparedBarrierAndV1, AqlPreparedKernelDispatchBatchV2,
    AqlPreparedKernelDispatchV1, classify_acquired_completion_value_v1,
};

#[allow(unsafe_code)]
#[path = "queue_dispatch_live.rs"]
mod dispatch;

pub use dispatch::{
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_V1, Gfx942KfdDebugTargetDispatchErrorV2,
    Gfx942KfdDebugTargetDispatchResultV2, Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchErrorV1,
    Gfx942KfdDispatchPointerFixupV1, Gfx942KfdDispatchRequestErrorV1, Gfx942KfdDispatchRequestV1,
    Gfx942KfdDispatchResultV1, Gfx942KfdQueueExceptionObservationV1,
    execute_gfx942_kfd_debug_target_dispatch_unchecked_v1,
    execute_gfx942_kfd_debug_target_dispatch_unchecked_v2,
    execute_gfx942_kfd_dispatch_unchecked_v1,
};

const CONTROL_BYTES: usize = 4_096;
pub(crate) const GFX942_DESTROYED_QUEUE_RELEASED_RESOURCE_COUNT_V1: u8 = 5;
static NEXT_QUEUE_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Canonical claim boundary for the live queue and fixed-batch foundation.
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-aql-session-r38-v1\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device\n",
    "memory_profile_sha256=bc7724673724d8cb9b370ac19c92342b17b760217370b977b76c7ae403ef8f38\n",
    "kfd_userptr_memory_schema_sha256=c1cee09bdf884d2c14a5dbb89c1f6f7885962c75b1457caf412821490919ee9e\n",
    "kfd_userptr_queue_control_schema_sha256=f1d75410d6bfacff2ea15ecfff226eb8aed7912ee324a36b8ed8550fa52bce02\n",
    "queue_resource_profile_sha256=37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02\n",
    "aql_dispatch_schema_sha256=82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0\n",
    "aql_barrier_and_schema_sha256=bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5\n",
    "aql_fixed_batch_schema_sha256=a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a\n",
    "aql_completion_schema_sha256=4b7e1090eccbae41ea09ce7d5147470eb665ee295cb0f4526f5584225c86369a\n",
    "dispatch_binding_schema_sha256=811fbd200ac0b72e5aff81494225b6ea37f517d62bad3779544653c2aae6d815\n",
    "event_schema_sha256=bdde2e2d9b03690d6a63dba3d91074da214d87ece9ae1894c4d7a160bced58b8\n",
    "runtime_enable_schema_sha256=fa47481b10ea4bd89438d10b82bd8197088906e55f5f0c827dc7aa5aba906288\n",
    "source.rocr.queues.c=b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b\n",
    "source.rocr.hsakamttypes.h=fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e\n",
    "source.kfd_events.c=295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a\n",
    "source.kfd_debug.c=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "source.kfd_chardev.c=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "source.kfd_process.c=d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409\n",
    "resources=linear-private-ring-control-eop-cwsr-completion-code-kernarg-and-exact-device-local-or-coherent-host-data-authorities,exact-one-existing-shared-vm-session,transferred-model-ownership\n",
    "gtt_policy=reusable-and-dispatch-ring:gfx942-host-visible-executable-single-span-without-gfx7-gfx8-double-map-workaround,one-shot-diagnostic-rings:plain-executable-gtt-one-span-or-userptr-writable-executable-coherent-uncached-no-substitute-one-span,control:exact-one-page-same-va-userptr-writable-coherent,completion-signals:host-visible-coherent-gtt,eop-and-cwsr:executable;ring-userptr-never-selectable-by-reusable-or-dispatch-queue-APIs\n",
    "userptr-diagnostic=smallest-selected-gpu-ring-backing-discriminator,no-full-rocr-allocation-or-map-order-parity-claim\n",
    "creation-boundary=planning-session-dispatch-and-ring-errors-before-userptr-control-registration-entry-retain-existing-classification,every-error-from-the-control-allocation-attempt-through-live-session-return-is-terminal-recovers-no-authority-permanently-poisons-the-process-global-runtime-gate-and-requires-process-termination\n",
    "runtime=one-process-global-fe2o3-context-with-refcounted-linear-queue-leases;first-lease-exact-enable-r_debug0-mode1-capabilities0-before-event-and-any-queue;last-fully-destroyed-lease-exact-disable;teardown-arm-permanent-poison-lifecycle-state-and-new-lease-admission-single-mutex-linearized;ordinary-queue-fd-or-consumed-debug-token-with-separate-same-process-admitted-control-fd;ttmp-save-excluded;foreign-kfd-clients-excluded\n",
    "initialization=every-logical-ring-slot-explicit-atomic-u32-invalid-1;control-amd-aql-v1-write-dispatch-id-at-0x38-read-dispatch-id-at-0x80-both-atomic-u64-zero-read-base-offset-u32-0x80-at-0x88;completion-arena-exact-8192-typed-64-byte-user-signals-pending-1-before-gpu-map;one-first-internal-auto-reset-signal-event-id-1-through-255-before-create;8-cwsr-bo-headers-and-24-control-stack-shadow-pages-at-0x1621000-stride,debug-offset-descending,debug-size-0x5f000,one-separate-private-aligned-error-reason-page-zero,exact-event-id\n",
    "submission=crate-private-non-clone-single-producer,aql-fixed-batch-v2-count-1-through-8192-and-ring-capacity-bounded,heap-owned-fixed-cardinality-state,no-mapped-slice-or-raw-pointer-escape,rptr-wptr-acquire,one-actual-wptr-acq-rel-fetch-add-by-count,all-invalid-bodies-before-per-packet-independent-0x1402-or-wait-for-prior-0x1502-ordered-u32-release-headers,exact-one-zero-setup-barrier-and-0x1403,conservative-service-default-wait-for-prior,release-fence-x86-sfence,one-final-volatile-u64-doorbell-store-of-last-packet-id\n",
    "completion=crate-private-non-clone-generation-bound-fixed-batches-and-one-signal-barrier-probe,fixed-batch-signal-code-kernarg-dispatch-and-queue-generations-retained,barrier-probe-queue-and-signal-generations-only,monotonic-deadline-or-legacy-bounded-atomic-acquire-poll-with-short-spin-yield-and-bounded-sleep-backoff-and-one-pre-post-currentness-envelope-and-same-scan-redacted-progress,pending-ready-fault-timeout-distinct,timeout-retains-private-linear-operation-through-sequential-pre-post-currentness-enveloped-addressless-write-read-counter-first-retained-packet-header-setup-first-retained-signal-kind-value-and-CWSR-reason-observation-before-poison,release-reset-only-after-all-retained-signals-zero\n",
    "liveness-probe=three-public-consuming-checked-device-entries-select-production-gfx942-executable-one-span-diagnostic-plain-executable-one-span-or-diagnostic-userptr-writable-executable-coherent-uncached-no-substitute-one-span-ring,selected-backing-and-exact-ring-span-bound-into-plan-and-configuration,selected-backing-bound-into-every-redacted-outcome,typed-nonzero-bounded-polls-validated-before-device-consumption,diagnostic-backings-not-selectable-by-reusable-or-dispatch-queue-APIs,exact-fresh-zero-history-no-dispatch-queue,one-zero-dependency-system-scope-barrier,queue-and-signal-generation-only,submission-retryable-only-by-explicit-before-side-effect-stage-classification,success-requires-currentness-packet-count1-write1-read0or1-timing-sensitive-header0x1403-or-device-consumed-invalid1-setup0-user-signal-completed-zero-exception-then-signal-reset-and-confirmed-explicit-queue-destroy,Creation-has-no-live-queue-and-precedes-userptr-control-registration-entry,TerminalCreation-covers-every-error-at-or-after-userptr-control-registration-entry-every-create-result-not-explicitly-failed-no-effect-and-every-post-create-failure-recovers-no-authority-permanently-poisons-process-global-runtime-gate-and-requires-process-termination,QuarantinedExecution-retains-opaque-custody-until-process-teardown,process-global-runtime-gate-poison-armed-before-destroy-and-cleared-only-after-confirmed-success,TerminalTeardown-and-panic-retain-permanent-gate-poison-and-recover-no-authority-native-resource-disposition-indeterminate-process-termination-required-no-retry-reopen-or-confirmed-cleanup\n",
    "dispatch=public-addressless-linear-fixed-batch,1-through-32-inspected-programs,1-through-8192-packets,validated-code-materialization,zero-pointer-kernarg-internal-injection,metadata-derived-COV6-geometry-and-dynamic-lds-implicit-subset-with-caller-zero-suffix,queue-pointer-and-runtime-address-fields-rejected,exact-mapped-data-set-retained-even-when-unreferenced-by-current-batch,referenced-subset-only-inspected-access-and-sealed-initialization-gates,ordinary-release-or-never-published-prepared-or-exact-recycle-gated-attached-or-detached-return-after-destroy\n",
    "readback=coherent-host-data-only,owned-bounded-copy-or-exact-caller-owned-destination-after-exact-acquire-observed-completion-and-signal-recycle,exact-dispatch-generation,ordinary-range-within-one-inspected-write-or-readwrite-binding-or-exact-admitted-initialized-enclosing-snapshot,no-native-address-or-mapped-borrow,no-whole-allocation-initialization-promotion\n",
    "rebinding=exact-completion-and-signal-recycle-before-detach,ordinary-detach-releases-code-and-kernarg,one-full-range-persistent-control-detach-retains-immutable-code-mapped-kernarg-packet-premise-and-recycled-generation-while-returning-only-the-exact-data-authority-for-directional-sdma,initial-persistent-control-open-and-explicit-release-use-full-currentness,its-exact-retained-control-replay-uses-operational-currentness-and-requires-exact-same-queue-vm-code-abi-packet-kernarg-role-layout-storage-and-predecessor-generation,live-rebind-retains-queue-ring-signal-event-doorbell-and-runtime,quiescent-rollover-confirms-old-native-destroy-before-new-queue-creation,exact-complete-detached-generation-cardinality-and-ordered-private-storage-identity-ledger,preflighted-device-or-host-insertion-at-exact-ordinal-and-release-gated-removal-or-replacement-while-unbound,exact-identity-kind-and-bounds-checked-in-place-initialized-coherent-overwrite-while-unbound-or-attached-and-recycled,attached-recycled-exact-shape-resubmission-advances-generation-without-code-kernarg-or-data-detach,replacement-owner-seeded-from-exact-predecessor-and-next-publication-strictly-advances-dispatch-generation-across-live-rebind-or-queue-rollover,all-mapped-data-retained-with-inspected-effects-only-for-currently-referenced-subset,new-ring-program-count-packet-count-geometry-kernarg-and-data-admitted-before-next-publication,fully-initialized-state-preserved-without-stale-current-content-digest,authoritative-model-foundation-restored-around-every-live-queue-allocation-lifecycle-mutation-and-reclaimed-before-return\n",
    "doorbell=complete-8192-byte-kfd-slice,exact-returned-offset,madv-dontfork,no-public-address-pointer-or-mmio-accessor\n",
    "lifecycle=runtime-enable,event-create,queue-create;all-completion-batches-observed-and-recycled;queue-destroy,event-destroy,immediate-payload-zero-protect-unmap,runtime-disable,doorbell-release,cwsr-queue-resource-and-completion-arena-release;debug-runtime-authority-leaves-token-before-event-and-create-lifecycle-mutation-with-no-post-handoff-restoration;published-owners-no-drop-ioctl-store-munmap-or-free;armed-unpublished-payload-guard-drop-zero-protect-unmap\n",
    "currentness=active-queue-opener-pid-before-non-draining-zero-timeout-reset-fifo-readiness-then-dedicated-wrapping-drm-vram-loss-counter-equality-then-closing-readiness-operational-fence-before-exact-persistent-replay,publication,after-bounded-preparation,and-before-mmio;readiness-means-nonempty-fifo-only-by-pinned-kfd-source-contract-not-loaded-kernel-authentication;packet-atomics-run-inside-those-owner-scopes;lifecycle-ioctls-and-persistent-control-open-close-retain-full-process-namespace-descriptor-uapi-xnack-drm-identity-vram-loss-topology-aperture-composite;operational-fence-excludes-those-lifecycle-identity-reobservations-and-cannot-exclude-reset-counter-wrap-or-observation-ABA;timeout-observation-confirms-device-runtime-event-and-CWSR-structure-before-and-after-its-sequential-racy-loads\n",
    "proof=queue-and-aql-model-obligations-only,cpu-gpu-atomic-coherence-mmio-driver-firmware-refinement-contracted\n",
    "event-lifecycle=linear-private-kfd-event,no-kfd-event-page-mmap,separate-private-payload-page-cleaned-on-unpublished-install-failure,armed-unpublished-payload-cleanup-through-all-pre-create-failures-until-immediately-before-native-create-queue-call,zeroized-protected-and-unmapped-immediately-after-event-destroy-before-runtime-disable-and-independent-of-later-resource-release,payload-cleanup-failure-after-event-destroy-aborts-process-before-owner-loss,queue-destroy-before-event-destroy-before-runtime-disable-before-cwsr-free-and-full-reservation-munmap,published-owners-no-drop-ioctl-or-unmap\n",
    "cwsr-address-semantics=bo-cpu-vma-is-create-address-except-exact-24-owned-fixed-private-anonymous-control-stack-pages,prot-none-then-dontfork-then-rw,whole-span-seal-then-exact-shadow-rw-restore;headers-and-control-stack-kfd-copy-targets,wave-state-remains-read-only-bo-mapped,event-payload-disjoint-from-all-control-stack-pages;ordinary-hardware-preemption-restore-contracted\n",
    "exception-observation=crate-private-one-shot-timeout-0-through-1000ms-wait-and-terminal-timeout-direct-volatile-CWSR-reason,wait-and-payload-must-agree,unknown-reason-rejected,zero-reason-is-racy-snapshot-not-absence-proof,no-atomic-or-lossless-delivery-claim\n",
    "failure=counter-divergence-regression-currentness-and-any-possible-side-effect-runtime-event-shadow-wait-publication-completion-observation-timeout-reset-or-teardown-error-terminally-poisons;timeout-snapshot-capture-failure-reports-currentness-or-observation-instead-of-unbound-evidence;no-in-process-recovery-rollback-or-cleanup-after-terminal-observation;only-explicitly-classified-pre-side-effect-full-or-insufficient-space-retryable\n",
    "excluded=kernel-dispatch-hardware-completion-fault-or-exception-delivery-refinement,kernel-effect-correctness-beyond-inspected-metadata,full-kernel-write-coverage,kernel-numerical-correctness,device-local-update,multi-producer,foreign-kfd-process-coordination,private-cwsr-wave-record-decoding\n",
);

/// SHA-256 of [`GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1`].
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1: &str =
    "0dc31c8db1e395f0290ac607cbe9610e455238cd5b2ba95a77dfe47494b2a8dc";

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

    fn map_and_retain(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<RingAuthority, MemorySessionError> {
        match self {
            Self::AqlSpecial(ring) => {
                let ring = memory.map_to_gpu(ring)?;
                memory
                    .retain_aql_ring_resource(ring)
                    .map(RingAuthority::AqlSpecial)
            }
            Self::ExecutableProbe(ring) => {
                let ring = memory.map_to_gpu(ring)?;
                memory
                    .retain_executable_aql_probe_ring_resource(ring)
                    .map(RingAuthority::ExecutableProbe)
            }
            Self::UserptrProbe(ring) => {
                let ring = memory.map_to_gpu(ring)?;
                memory
                    .retain_userptr_aql_probe_ring_resource(ring)
                    .map(RingAuthority::UserptrProbe)
            }
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

struct LinuxNativeQueueBackendV1 {
    session: SharedGttMemorySessionV1,
    foundation: Option<QueueModelFoundationV1>,
    foundation_in_engine: bool,
}

impl NativeQueueBackendV1 for LinuxNativeQueueBackendV1 {
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
        mut args: fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs> {
        let status = match crate::queue_linux::create_queue(self.session.kfd_fd(), &mut args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }

    fn update(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs> {
        let status = match crate::queue_linux::update_queue(self.session.kfd_fd(), &args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }

    fn destroy(
        &mut self,
        mut args: fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs> {
        let status = match crate::queue_linux::destroy_queue(self.session.kfd_fd(), &mut args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
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
    generation: u64,
    completion: Result<Gfx942CompletionBatchV1<N>, FixedDispatchSubmissionFailureV1>,
    cancel_binding: impl FnOnce(u64) -> Result<(), Gfx942DispatchBindingErrorV1>,
) -> Result<Gfx942DispatchBatchV1<N>, FixedDispatchSubmissionFailureV1> {
    match completion {
        Ok(completion) => Ok(wrap_published(completion, generation)),
        Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
            match cancel_binding(generation) {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaMultiQueueFailureDispositionV1 {
    RetryablePreflight,
    TerminalPrePublication,
    TerminalPartialPublication,
    TerminalPostPublication,
}

/// Addressless observation of queue-retained terminal custody.
///
/// Ticket values are intentionally not exposed: after any multi-queue terminal failure the
/// session is poisoned, so these records cannot be polled, drained, or resubmitted safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaTerminalShardObservationV1<'a> {
    queue_ordinal: usize,
    queue_id: u32,
    request_indices: &'a [u16],
    retained_ticket_count: usize,
}

impl<'a> Gfx942SdmaTerminalShardObservationV1<'a> {
    pub const fn queue_ordinal(self) -> usize {
        self.queue_ordinal
    }

    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }

    pub const fn request_indices(self) -> &'a [u16] {
        self.request_indices
    }

    pub const fn retained_ticket_count(self) -> usize {
        self.retained_ticket_count
    }
}

enum Gfx942SdmaTerminalCustodyStateV1 {
    BeforePublication(Vec<Gfx942SdmaCopyRequestV1>),
    Publication {
        plan: Gfx942SdmaMultiQueuePlanV1,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
        untouched: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
    },
    CompletePublication {
        plan: Gfx942SdmaMultiQueuePlanV1,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
    },
}

/// Audit-only ownership retained after a terminal multi-queue failure.
///
/// The contained buffers remain either queue-retained or tied to the poisoned queue occurrence.
/// This type deliberately provides observations only and has no ticket/request extraction or
/// drain API. It must remain owned until process teardown.
#[must_use = "terminal SDMA custody must remain retained until process teardown"]
pub struct Gfx942SdmaMultiQueueTerminalCustodyV1 {
    state: Gfx942SdmaTerminalCustodyStateV1,
}

impl Gfx942SdmaMultiQueueTerminalCustodyV1 {
    fn before_publication(requests: Vec<Gfx942SdmaCopyRequestV1>) -> Self {
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::BeforePublication(requests),
        }
    }

    fn publication(
        plan: Gfx942SdmaMultiQueuePlanV1,
        confirmed: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
        untouched: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
    ) -> Self {
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::Publication {
                plan,
                confirmed,
                indeterminate,
                untouched,
            },
        }
    }

    fn complete_publication(submission: Gfx942SdmaMultiQueueSubmissionV1) -> Self {
        let (plan, confirmed) = submission.into_parts();
        Self {
            state: Gfx942SdmaTerminalCustodyStateV1::CompletePublication { plan, confirmed },
        }
    }

    pub const fn plan(&self) -> Option<&Gfx942SdmaMultiQueuePlanV1> {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => None,
            Gfx942SdmaTerminalCustodyStateV1::Publication { plan, .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletePublication { plan, .. } => Some(plan),
        }
    }

    pub fn confirmed_shard_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => 0,
            Gfx942SdmaTerminalCustodyStateV1::Publication { confirmed, .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletePublication { confirmed, .. } => {
                confirmed.len()
            }
        }
    }

    pub fn confirmed_shard(
        &self,
        index: usize,
    ) -> Option<Gfx942SdmaTerminalShardObservationV1<'_>> {
        let shard = match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(_) => None,
            Gfx942SdmaTerminalCustodyStateV1::Publication { confirmed, .. }
            | Gfx942SdmaTerminalCustodyStateV1::CompletePublication { confirmed, .. } => {
                confirmed.get(index)
            }
        }?;
        Some(Gfx942SdmaTerminalShardObservationV1 {
            queue_ordinal: shard.queue_ordinal(),
            queue_id: shard.queue_id(),
            request_indices: shard.request_indices(),
            retained_ticket_count: shard.tickets().len(),
        })
    }

    pub fn indeterminate_shard(&self) -> Option<Gfx942SdmaTerminalShardObservationV1<'_>> {
        let Gfx942SdmaTerminalCustodyStateV1::Publication { indeterminate, .. } = &self.state
        else {
            return None;
        };
        let shard = indeterminate.as_ref()?;
        Some(Gfx942SdmaTerminalShardObservationV1 {
            queue_ordinal: shard.queue_ordinal(),
            queue_id: shard.queue_id(),
            request_indices: shard.request_indices(),
            retained_ticket_count: shard.tickets().len(),
        })
    }

    pub fn untouched_request_count(&self) -> usize {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(requests) => requests.len(),
            Gfx942SdmaTerminalCustodyStateV1::Publication { untouched, .. } => untouched.len(),
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication { .. } => 0,
        }
    }

    pub fn untouched_request_index(&self, index: usize) -> Option<usize> {
        match &self.state {
            Gfx942SdmaTerminalCustodyStateV1::BeforePublication(requests) => {
                (index < requests.len()).then_some(index)
            }
            Gfx942SdmaTerminalCustodyStateV1::Publication { untouched, .. } => {
                untouched.get(index).map(|request| request.request_index())
            }
            Gfx942SdmaTerminalCustodyStateV1::CompletePublication { .. } => None,
        }
    }
}

#[must_use = "inspect retryable requests or retain terminal custody through process teardown"]
pub enum Gfx942SdmaMultiQueueFailureCustodyV1 {
    /// No native side effect occurred and the requests may be submitted again.
    RetryableRequests(Vec<Gfx942SdmaCopyRequestV1>),
    /// The queue occurrence is terminal. This is audit-only/process-teardown custody.
    ProcessTeardown(Gfx942SdmaMultiQueueTerminalCustodyV1),
}

#[must_use = "failure preserves exact multi-queue custody and publication progress"]
pub struct Gfx942SdmaMultiQueueSubmissionFailureV1 {
    error: ComputeAqlQueueSessionErrorV1,
    disposition: Gfx942SdmaMultiQueueFailureDispositionV1,
    custody: Gfx942SdmaMultiQueueFailureCustodyV1,
}

impl Gfx942SdmaMultiQueueSubmissionFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub const fn disposition(&self) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
        self.disposition
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.disposition,
            Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
        )
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SdmaMultiQueueFailureDispositionV1,
        Gfx942SdmaMultiQueueFailureCustodyV1,
    ) {
        (self.error, self.disposition, self.custody)
    }
}

const fn classify_multi_queue_preparation_failure(
    owner_poisoned: bool,
    closing_currentness_failed: bool,
) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
    if owner_poisoned || closing_currentness_failed {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    } else {
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
    }
}

const fn classify_multi_queue_availability_failure(
    session_terminal: bool,
) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
    if session_terminal {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    } else {
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
    }
}

const fn classify_multi_queue_publication_failure(
    published_shards: usize,
    has_indeterminate_shard: bool,
) -> Gfx942SdmaMultiQueueFailureDispositionV1 {
    if published_shards == 0 && !has_indeterminate_shard {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    } else {
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
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
    DirectionalCopy(Gfx942PersistentSdmaDirectionV1),
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
            if loan_succeeded && cancellation_succeeded && session_healthy =>
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
    data: Gfx942FixedDispatchDataV1,
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

#[allow(clippy::too_many_arguments)]
fn execute_persistent_retained_control_replay_pipeline_v1<
    Context,
    Request,
    Storage,
    Data,
    Attached,
    Error,
>(
    context: &mut Context,
    mut request: Request,
    mapped_facts: impl FnOnce(&mut Context, &mut Request) -> Result<(), Error>,
    detach: impl FnOnce(&mut Context, Request) -> Result<Storage, (Error, Request)>,
    construct: impl FnOnce(&mut Context, Storage) -> Result<Data, (Error, Storage)>,
    retain: impl FnOnce(&mut Context, Data) -> Result<Attached, (Error, Data)>,
    final_audit: impl FnOnce(&mut Context, &Attached) -> Result<(), Error>,
) -> PersistentRetainedControlReplayPipelineOutcomeV1<Request, Storage, Data, Attached, Error> {
    if let Err(error) = mapped_facts(context, &mut request) {
        return PersistentRetainedControlReplayPipelineOutcomeV1::BeforeDetach { request, error };
    }
    let storage = match detach(context, request) {
        Ok(storage) => storage,
        Err((error, request)) => {
            return PersistentRetainedControlReplayPipelineOutcomeV1::BeforeDetach {
                request,
                error,
            };
        }
    };
    let data = match construct(context, storage) {
        Ok(data) => data,
        Err((error, storage)) => {
            return PersistentRetainedControlReplayPipelineOutcomeV1::Storage { storage, error };
        }
    };
    let attached = match retain(context, data) {
        Ok(attached) => attached,
        Err((error, data)) => {
            return PersistentRetainedControlReplayPipelineOutcomeV1::Data { data, error };
        }
    };
    if let Err(error) = final_audit(context, &attached) {
        return PersistentRetainedControlReplayPipelineOutcomeV1::Attached { attached, error };
    }
    PersistentRetainedControlReplayPipelineOutcomeV1::Ready(attached)
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
    generation: u64,
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

#[must_use = "queue destruction and resource return are explicit"]
pub struct ComputeAqlQueueSessionV1 {
    engine: Option<NativeQueueEngineV1<LinuxNativeQueueBackendV1>>,
    key: QueueKeyV1,
    compute_lane_session: QueueKeyV1,
    doorbell: Option<LinuxDoorbellSliceV1>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    completion_signals: Option<CompletionSignalAuthority>,
    completion_owner: CompletionSignalArenaOwnerV1,
    dispatch: Option<DispatchResourceOwnerV1>,
    detached_data_count: usize,
    detached_dispatch_generation: Option<u64>,
    detached_data_identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    detached_next_insertion_index: Option<usize>,
    persistent_compute: Option<PersistentComputeAttachmentV1>,
    #[cfg(test)]
    persistent_compute_test_release: Option<(u64, Vec<Gfx942FixedDispatchDataV1>)>,
    next_persistent_compute_generation: u64,
    exception: Option<QueueExceptionStateV1>,
    sdma: Option<Gfx942SdmaQueueSetV1>,
    sdma_outstanding_buffers: usize,
    sdma_pool_free: Vec<Gfx942SdmaBufferV1>,
    sdma_pool_reuse_count: u64,
    terminal_poisoned: bool,
    observation: ComputeAqlQueueObservationV1,
    auxiliary_compute_lanes: Vec<AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1>>,
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

struct ComputeAqlQueueLaneStateV1 {
    key: QueueKeyV1,
    doorbell: Option<LinuxDoorbellSliceV1>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    completion_signals: Option<CompletionSignalAuthority>,
    completion_owner: CompletionSignalArenaOwnerV1,
    dispatch: Option<DispatchResourceOwnerV1>,
    detached_data_count: usize,
    detached_dispatch_generation: Option<u64>,
    detached_data_identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    detached_next_insertion_index: Option<usize>,
    exception: Option<QueueExceptionStateV1>,
    observation: ComputeAqlQueueObservationV1,
}

struct PreparedAuxiliaryComputeLaneV1 {
    authority: QueueResourceAuthorityV1,
    completion_signals: CompletionSignalAuthority,
    completion_owner: CompletionSignalArenaOwnerV1,
    submission: NativeAqlSubmissionOwnerV1,
    dispatch: DispatchResourceOwnerV1,
    runtime: LinuxKfdRuntimeEnabledV1,
    event: LinuxQueueExceptionEventV1,
    unpublished_shadows: LinuxUnpublishedCwsrShadowPagesV1,
    ring_bytes: u32,
}

fn prepare_auxiliary_compute_lane_slot_v1<T>(
    slots: &[AuxiliaryComputeLaneSlotV1<T>],
) -> Result<PreparedAuxiliaryComputeLaneSlotV1, ComputeAqlQueueSessionErrorV1> {
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

fn install_auxiliary_compute_lane_slot_v1<T>(
    slots: &mut Vec<AuxiliaryComputeLaneSlotV1<T>>,
    prepared: PreparedAuxiliaryComputeLaneSlotV1,
    state: T,
) {
    if prepared.append {
        slots.push(AuxiliaryComputeLaneSlotV1 {
            generation: prepared.generation,
            state: Some(state),
        });
    } else {
        let slot = &mut slots[prepared.index];
        slot.generation = prepared.generation;
        slot.state = Some(state);
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

    pub fn poll_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.session.poll_fixed_dispatch(batch)
    }

    pub fn recycle_fixed_dispatch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, ComputeAqlQueueSessionErrorV1> {
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

struct QueueExceptionStateV1 {
    runtime: LinuxKfdRuntimeEnabledV1,
    runtime_control: Option<KfdWithAdmittedUapi>,
    event: LinuxQueueExceptionEventV1,
    shadows: LinuxCwsrShadowPagesV1,
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

    pub(crate) fn create_compute_aql_queue_with<T>(
        self,
        ring_bytes: u32,
        prepare: impl FnOnce(&mut SharedGttMemorySessionV1) -> Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(ComputeAqlQueueSessionV1, T), ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_runtime(ring_bytes, prepare, None)
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
    ) -> Result<(ComputeAqlQueueSessionV1, T), ComputeAqlQueueSessionErrorV1> {
        let geometry = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )?;
        let mut memory = self.acquire_shared_gtt_memory_session()?;
        let prepared = prepare(&mut memory)?;
        let session = ComputeAqlQueueSessionV1::create_compute_aql_queue_inner(
            memory,
            geometry,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            |_| Ok(None),
            external_runtime,
        )?;
        Ok((session, prepared))
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

    /// Private source-complete preparation path. There is intentionally no
    /// safe public producer for its data premises or typed kernarg images.
    #[allow(dead_code)]
    pub(crate) fn create_compute_aql_queue_with_dispatch<const N: usize>(
        self,
        ring_bytes: u32,
        kernel: fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>,
        geometry: [DispatchGeometryV1; N],
        kernargs: [TypedKernargImageV1; N],
        data: Vec<DeviceDataAllocationInputV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        validate_fixed_batch_ring::<N>(ring_bytes)?;
        let geometry_plan = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )?;
        let memory = self.acquire_shared_gtt_memory_session()?;
        ComputeAqlQueueSessionV1::create_compute_aql_queue_inner(
            memory,
            geometry_plan,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            move |memory| {
                prepare_dispatch_resources(memory, kernel, geometry, kernargs, data)
                    .map(Some)
                    .map_err(ComputeAqlQueueSessionErrorV1::DispatchBinding)
            },
            None,
        )
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

impl SharedGttMemorySessionV1 {
    /// Creates one long-lived compute-AQL queue from this exact KFD VM session
    /// and consumes all fixed-batch executable, kernarg, and device-storage
    /// authority into it.
    ///
    /// The operation does not expose native addresses. Inspected global-buffer
    /// access determines whether each referenced move-only storage input must
    /// carry sealed initialization authority. Every mapped storage input and
    /// inspected program is retained even when no packet in this batch selects
    /// it. Queue creation does not establish
    /// kernel numerical correctness, memory-effect refinement, or hardware
    /// execution.
    pub fn create_compute_aql_queue_with_fixed_dispatch<const N: usize>(
        self,
        ring_bytes: u32,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        validate_fixed_batch_ring::<N>(ring_bytes)?;
        let geometry = self.plan_aql_queue_resources(ring_bytes)?;
        ComputeAqlQueueSessionV1::create_compute_aql_queue_inner(
            self,
            geometry,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            move |memory| {
                prepare_public_fixed_dispatch_resources(memory, programs, packets, data)
                    .map(Some)
                    .map_err(ComputeAqlQueueSessionErrorV1::DispatchBinding)
            },
            None,
        )
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Reports only the terminal persistent-compute custody stage; native
    /// identities and authorities remain retained inside the queue.
    pub fn persistent_compute_terminal_stage_v1(
        &self,
    ) -> Option<crate::persistent_compute::Gfx942PersistentComputeTerminalStageV1> {
        self.persistent_compute
            .as_ref()?
            .terminal_custody
            .as_ref()
            .map(PersistentComputeTerminalNativeCustodyV1::stage)
    }

    fn absorb_terminal_prepared_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
    ) -> bool {
        let Some(mut attachment) = self.persistent_compute.take() else {
            return false;
        };
        if attachment.binding != binding {
            self.persistent_compute = Some(attachment);
            return false;
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Prepared(prepared) = state else {
            attachment.state = state;
            self.persistent_compute = Some(attachment);
            return false;
        };
        let _ = attachment.allocation.owner.quarantine_prepared(
            prepared,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
        );
        attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
        self.persistent_compute = Some(attachment);
        true
    }

    #[allow(clippy::result_large_err)]
    fn absorb_terminal_published_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<(), Gfx942DispatchBatchV1<1>> {
        let Some(mut attachment) = self.persistent_compute.take() else {
            return Err(batch);
        };
        if attachment.binding != binding {
            self.persistent_compute = Some(attachment);
            return Err(batch);
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Published(published) = state else {
            attachment.state = state;
            self.persistent_compute = Some(attachment);
            return Err(batch);
        };
        let _ = attachment.allocation.owner.quarantine_published(
            published,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        attachment.terminal_custody =
            Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
        self.persistent_compute = Some(attachment);
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    fn absorb_terminal_completed_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<(), Gfx942CompletedDispatchBatchV1<1>> {
        let Some(mut attachment) = self.persistent_compute.take() else {
            return Err(completed);
        };
        if attachment.binding != binding {
            self.persistent_compute = Some(attachment);
            return Err(completed);
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            attachment.state = state;
            self.persistent_compute = Some(attachment);
            return Err(completed);
        };
        let _ = attachment.allocation.owner.quarantine_completed(
            completed_use,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Completed(
            completed,
        ));
        self.persistent_compute = Some(attachment);
        Ok(())
    }

    fn absorb_terminal_recycled_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        recycle: Gfx942CompletionRecycleObservationV1,
    ) -> Result<(), Gfx942CompletionRecycleObservationV1> {
        let Some(mut attachment) = self.persistent_compute.take() else {
            return Err(recycle);
        };
        if attachment.binding != binding {
            self.persistent_compute = Some(attachment);
            return Err(recycle);
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Recycled(completed_use) = state else {
            attachment.state = state;
            self.persistent_compute = Some(attachment);
            return Err(recycle);
        };
        let _ = attachment.allocation.owner.quarantine_completed(
            completed_use,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        attachment.terminal_custody =
            Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
        self.persistent_compute = Some(attachment);
        Ok(())
    }

    /// Authenticates one exact full-allocation H2D window and retires its
    /// settled frontier into an initialized persistent-compute receipt.
    #[allow(clippy::result_large_err)]
    pub fn promote_full_h2d_to_persistent_compute_ready_v1(
        &mut self,
        completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (Gfx942PersistentComputeReadyV1, Gfx942SdmaBufferV1),
        Gfx942PersistentComputeReadyFailureV1,
    > {
        let completed = preserve_persistent_compute_ready_affiliation_v1(
            completed,
            self.key,
            self.terminal_poisoned,
        )?;
        let preflight = self.require_sdma_enabled();
        let completed = preserve_persistent_compute_ready_preflight_custody_v1(
            completed,
            self.terminal_poisoned,
            preflight,
        )?;
        let direction = completed.direction();
        let host_offset = completed.host_offset();
        let device_offset = completed.device_offset();
        let copy_bytes = u64::from(completed.copy_bytes());
        let packet_count = completed.packet_count();
        let (allocation, host, frontier) = completed.into_parts();
        let valid = direction == Gfx942PersistentSdmaDirectionV1::HostToDevice
            && host_offset == 0
            && device_offset == 0
            && allocation.byte_len() == allocation.physical_byte_len()
            && copy_bytes == allocation.physical_byte_len()
            && content.byte_len() == copy_bytes
            && host.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
            && host.requested_bytes() == copy_bytes
            && host.physical_bytes() == copy_bytes
            && allocation.attachment.queue == self.compute_lane_session
            && self.directional_persistent_sdma_attachment_is_current(&allocation.attachment);
        if !valid {
            return Err(Gfx942PersistentComputeReadyFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute initialization requires one exact full H2D window",
                ),
                custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable((
                    allocation, host, frontier,
                )),
            });
        }
        let observed = self.with_live_queue_memory_model(|memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            let observed = host.certified_full_host_content_sha256(copy_bytes);
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            Ok(observed)
        });
        let observed = match observed {
            Ok(observed) => observed,
            Err(error) => {
                self.poison_terminal();
                let completed =
                    Gfx942DirectionalPersistentSdmaWindowCompletedV1::from_parts_for_terminal(
                        allocation,
                        host,
                        frontier,
                        direction,
                        host_offset,
                        device_offset,
                        u32::try_from(copy_bytes).expect("copy bytes came from u32"),
                        packet_count,
                    );
                return Err(terminal_persistent_compute_ready_hash_failure_v1(
                    error, completed,
                ));
            }
        };
        if observed.is_none_or(|observed| {
            !content_descriptor_matches_sha256(content, copy_bytes, observed)
        }) {
            return Err(Gfx942PersistentComputeReadyFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute H2D content descriptor mismatch",
                ),
                custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable((
                    allocation, host, frontier,
                )),
            });
        }
        let allocation = match allocation.retire_settled_frontier_v1(frontier) {
            Ok(allocation) => allocation,
            Err(failure) => {
                let (allocation, frontier) = failure.into_parts();
                return Err(Gfx942PersistentComputeReadyFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute H2D frontier retirement",
                    ),
                    custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable((
                        allocation, host, frontier,
                    )),
                });
            }
        };
        Ok((
            Gfx942PersistentComputeReadyV1 {
                allocation,
                authenticated_sha256: content.sha256(),
            },
            host,
        ))
    }

    /// Authenticates one exact full-allocation, single-packet H2D copy and
    /// retires its settled frontier into an initialized persistent-compute
    /// receipt. The consumed completion is normalized to the same sealed
    /// one-packet window representation used by the common ready transition.
    #[allow(clippy::result_large_err)]
    pub fn promote_full_single_h2d_to_persistent_compute_ready_v1(
        &mut self,
        completed: Gfx942DirectionalPersistentSdmaCompletedV1,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (Gfx942PersistentComputeReadyV1, Gfx942SdmaBufferV1),
        Gfx942PersistentComputeReadyFailureV1,
    > {
        self.promote_full_h2d_to_persistent_compute_ready_v1(
            completed.into_single_packet_window_v1(),
            content,
        )
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
            let mut lane = ComputeAqlQueueLaneDispatchV1 { session: self };
            return Ok(operation(&mut lane));
        };
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let mut selected = self.auxiliary_compute_lanes[index]
            .state
            .take()
            .expect("admitted auxiliary compute lane retains state");
        self.swap_primary_compute_lane(&mut selected);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            operation(&mut ComputeAqlQueueLaneDispatchV1 { session: self })
        }));
        self.swap_primary_compute_lane(&mut selected);
        self.auxiliary_compute_lanes[index].state = Some(selected);
        match result {
            Ok(result) => Ok(result),
            Err(payload) => std::panic::resume_unwind(payload),
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
        if self.persistent_compute.is_some() {
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
        self.check_currentness()?;

        let prepared = self.with_live_queue_memory_model(move |memory| {
            let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
            let data = prepare_data(memory)?;
            let dispatch = prepare_public_fixed_dispatch_resources(memory, programs, packets, data)
                .map_err(ComputeAqlQueueSessionErrorV1::DispatchBinding)?;
            let mut ring = CpuRingAuthorityV1::allocate(
                memory,
                QueueRingBackingV1::AqlSpecial,
                usize::try_from(ring_bytes)
                    .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring size conversion"))?,
            )?;
            let mut control = memory.allocate_userptr_aql_control()?;
            let mut completion_signals =
                memory.allocate_host_visible_coherent(COMPLETION_SIGNAL_ARENA_BYTES_V1)?;
            let mut eop = memory.allocate_executable(
                usize::try_from(geometry.end_of_pipe().mapping_bytes())
                    .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("EOP size conversion"))?,
            )?;
            let mut context_save = memory.allocate_executable(
                usize::try_from(geometry.context_save().mapping_bytes()).map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract("context-save size conversion")
                })?,
            )?;
            ring.initialize_invalid(memory)?.map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("INVALID ring initialization")
            })?;
            memory
                .with_bytes_mut(&mut control, initialize_amd_aql_control)?
                .map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract("AMD AQL control initialization")
                })?;
            memory.with_bytes_mut(
                &mut completion_signals,
                initialize_pending_completion_signal_arena,
            )??;
            memory.with_bytes_mut(&mut eop, |bytes| bytes.fill(0))?;
            memory.with_bytes_mut(&mut context_save, |bytes| bytes.fill(0))?;
            memory.check_queue_currentness()?;

            let runtime = LinuxKfdRuntimeEnabledV1::enable(memory.kfd_fd(), memory.opener_pid())?;
            runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?;
            let event = LinuxQueueExceptionEventV1::create(memory.kfd_fd(), memory.opener_pid())?;
            memory.check_queue_currentness()?;
            let shadow_plan = memory.cwsr_shadow_plan(&context_save)?;
            let unpublished_shadows = LinuxCwsrShadowPagesV1::install(shadow_plan, &event)?;
            let cwsr_initialization = memory.with_bytes_mut(&mut context_save, |bytes| {
                unpublished_shadows
                    .shadows()
                    .initialize_and_validate_bo_headers(bytes)
            })?;
            cwsr_initialization.map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("CWSR header initialization")
            })?;
            runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?;
            event.validate_live_with_shadows(
                memory.kfd_fd(),
                memory.opener_pid(),
                unpublished_shadows.shadows(),
            )?;
            memory.check_queue_currentness()?;

            let eop = memory.seal_executable(eop)?;
            let context_save = memory.seal_executable(context_save)?;
            unpublished_shadows
                .shadows()
                .restore_kernel_write_access_after_bo_seal()?;
            let ring = ring.map_and_retain(memory)?;
            let control = memory.map_to_gpu(control)?;
            let completion_signals = memory.map_to_gpu(completion_signals)?;
            let eop = memory.map_executable_to_gpu(eop)?;
            let context_save = memory.map_executable_to_gpu(context_save)?;
            let control = memory.retain_aql_control_resource(control)?;
            let completion_signals =
                memory.retain_aql_completion_signal_resource(completion_signals)?;
            let eop = memory.retain_aql_eop_resource(eop)?;
            let context_save = memory.retain_aql_context_save_resource(context_save)?;
            let authority = build_resource_authority(
                memory.queue_model_device(),
                geometry,
                ring,
                control,
                eop,
                context_save,
            )?;
            let completion_owner = CompletionSignalArenaOwnerV1::new(
                authority.view.plan.queue,
                completion_signals.facts(),
            )?;
            let submission = NativeAqlSubmissionOwnerV1::new(ring_bytes).map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("AQL ring submission model")
            })?;
            Ok(PreparedAuxiliaryComputeLaneV1 {
                authority,
                completion_signals,
                completion_owner,
                submission,
                dispatch,
                runtime,
                event,
                unpublished_shadows,
                ring_bytes,
            })
        });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.poison_terminal();
                return Err(error);
            }
        };
        let result = self.finish_auxiliary_compute_lane_creation_v1(prepared, slot);
        if result.is_err() {
            self.poison_terminal();
        }
        result
    }

    fn finish_auxiliary_compute_lane_creation_v1(
        &mut self,
        prepared: PreparedAuxiliaryComputeLaneV1,
        slot: PreparedAuxiliaryComputeLaneSlotV1,
    ) -> Result<ComputeAqlQueueLaneV1, ComputeAqlQueueSessionErrorV1> {
        let PreparedAuxiliaryComputeLaneV1 {
            authority,
            completion_signals,
            completion_owner,
            submission,
            dispatch,
            mut runtime,
            event,
            unpublished_shadows,
            ring_bytes,
        } = prepared;
        let (key, outputs, queue_id, shadows) = {
            let engine = self
                .engine
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?;
            let key = engine.admit(authority).map_err(map_native)?;
            let mut shadows = None;
            engine
                .create_at_native_boundary(key, || {
                    shadows = Some(unpublished_shadows.publish_for_native_queue_creation());
                })
                .map_err(map_create)?;
            let shadows = shadows.expect("native CREATE_QUEUE boundary published CWSR shadows");
            runtime.mark_queue_created().map_err(|error| {
                terminal_creation("runtime queue-live transition", error.into())
            })?;
            let outputs = engine.create_outputs(key).ok_or_else(|| {
                terminal_creation(
                    "CREATE_QUEUE output recovery",
                    ComputeAqlQueueSessionErrorV1::Contract("missing CREATE outputs"),
                )
            })?;
            let queue_id = engine.native_queue_id(key).ok_or_else(|| {
                terminal_creation(
                    "CREATE_QUEUE identity recovery",
                    ComputeAqlQueueSessionErrorV1::Contract("missing queue id"),
                )
            })?;
            (key, outputs, queue_id, shadows)
        };
        let mut observation = ComputeAqlQueueObservationV1 {
            queue_id,
            ring_bytes,
            doorbell_slice_bytes: 0,
            doorbell_byte_offset: 0,
            event_id: event.event_id_observation(),
            cwsr_shadow_pages: 8,
        };
        self.check_currentness()?;
        let doorbell = {
            let engine = self.engine.as_ref().expect("checked queue engine");
            LinuxDoorbellSliceV1::map(engine.backend.session.kfd_fd(), outputs, engine.opener_pid)
        }
        .map_err(|error| terminal_creation("doorbell mapping", error.into()))?;
        observation.doorbell_slice_bytes = doorbell.slice_bytes();
        observation.doorbell_byte_offset = doorbell.queue_byte_offset();
        self.check_currentness()?;
        install_auxiliary_compute_lane_slot_v1(
            &mut self.auxiliary_compute_lanes,
            slot,
            ComputeAqlQueueLaneStateV1 {
                key,
                doorbell: Some(doorbell),
                submission: Some(submission),
                completion_signals: Some(completion_signals),
                completion_owner,
                dispatch: Some(dispatch),
                detached_data_count: 0,
                detached_dispatch_generation: None,
                detached_data_identities: Vec::new(),
                detached_next_insertion_index: None,
                exception: Some(QueueExceptionStateV1 {
                    runtime,
                    runtime_control: None,
                    event,
                    shadows,
                }),
                observation,
            },
        );
        Ok(ComputeAqlQueueLaneV1 {
            session: self.compute_lane_session,
            ordinal: slot.index + 1,
            generation: slot.generation,
        })
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
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let mut state = take_after_auxiliary_destroy_preflight_v1(
            &mut self.auxiliary_compute_lanes[index].state,
            |state| {
                state.completion_owner.ensure_releasable()?;
                state
                    .dispatch
                    .as_ref()
                    .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
                    .ensure_releasable()?;
                Ok(())
            },
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
                let dispatch = state
                    .dispatch
                    .take()
                    .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
                let completion_signals = state.completion_signals.take().ok_or(
                    ComputeAqlQueueSessionErrorV1::Contract("missing completion signal arena"),
                )?;
                self.with_live_queue_memory_model(move |memory| {
                    release_resource_authority(memory, authority, shadow_release)?;
                    dispatch.release(memory)?;
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
        mut memory: SharedGttMemorySessionV1,
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
        let dispatch = prepare_dispatch(&mut memory)?;

        let ring = CpuRingAuthorityV1::allocate(
            &mut memory,
            ring_backing,
            usize::try_from(ring_bytes)
                .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring size conversion"))?,
        )?;
        Self::create_compute_aql_queue_after_userptr_control_entry(
            memory,
            geometry,
            ring_bytes,
            ring,
            dispatch,
            external_runtime,
        )
        .map_err(terminal_userptr_control_creation)
    }

    fn create_compute_aql_queue_after_userptr_control_entry(
        mut memory: SharedGttMemorySessionV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        ring_bytes: u32,
        mut ring: CpuRingAuthorityV1,
        dispatch: Option<DispatchResourceOwnerV1>,
        mut external_runtime: Option<ExternalRuntimeV1<'_>>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        let mut control = memory.allocate_userptr_aql_control()?;
        let mut completion_signals =
            memory.allocate_host_visible_coherent(COMPLETION_SIGNAL_ARENA_BYTES_V1)?;
        let mut eop = memory.allocate_executable(
            usize::try_from(geometry.end_of_pipe().mapping_bytes())
                .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("EOP size conversion"))?,
        )?;
        let mut context_save = memory.allocate_executable(
            usize::try_from(geometry.context_save().mapping_bytes()).map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("context-save size conversion")
            })?,
        )?;
        let ring_initialization = ring.initialize_invalid(&mut memory)?;
        ring_initialization
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("INVALID ring initialization"))?;
        let control_initialization =
            memory.with_bytes_mut(&mut control, initialize_amd_aql_control)?;
        control_initialization.map_err(|_| {
            ComputeAqlQueueSessionErrorV1::Contract("AMD AQL control initialization")
        })?;
        let completion_initialization = memory.with_bytes_mut(
            &mut completion_signals,
            initialize_pending_completion_signal_arena,
        )?;
        completion_initialization?;
        memory.with_bytes_mut(&mut eop, |bytes| bytes.fill(0))?;
        memory.with_bytes_mut(&mut context_save, |bytes| bytes.fill(0))?;
        memory.check_queue_currentness()?;
        let mut owned_runtime = None;
        match external_runtime.as_mut() {
            Some((runtime, control)) => {
                let runtime = runtime
                    .as_ref()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing debug runtime authority",
                    ))?;
                let control = control
                    .as_ref()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing debug runtime control descriptor",
                    ))?;
                runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?;
            }
            None => {
                let runtime =
                    match LinuxKfdRuntimeEnabledV1::enable(memory.kfd_fd(), memory.opener_pid()) {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = memory.quarantine_queue_composition(
                                "RUNTIME_ENABLE enable ambiguous failure",
                            );
                            return Err(error.into());
                        }
                    };
                owned_runtime = Some(runtime);
            }
        }
        if let Some((runtime, control)) = external_runtime.as_ref() {
            let runtime = runtime.as_ref().expect("validated debug runtime authority");
            let control = control
                .as_ref()
                .expect("validated debug runtime control descriptor");
            runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?;
        } else {
            owned_runtime
                .as_ref()
                .expect("enabled queue runtime")
                .validate_active(memory.kfd_fd(), memory.opener_pid())?;
        }
        memory.check_queue_currentness()?;
        let (mut runtime, runtime_control) = match external_runtime.as_mut() {
            Some((runtime, control)) => (
                runtime.take().expect("validated debug runtime authority"),
                Some(
                    control
                        .take()
                        .expect("validated debug runtime control descriptor"),
                ),
            ),
            None => (owned_runtime.take().expect("enabled queue runtime"), None),
        };
        // Event creation is the first queue-lifecycle mutation. Debug runtime
        // authority has left the original token before this boundary, so no
        // later failure can issue a stale no-queue disable transition.
        let event = match LinuxQueueExceptionEventV1::create(memory.kfd_fd(), memory.opener_pid()) {
            Ok(event) => event,
            Err(error) => {
                let _ = memory.quarantine_queue_composition("CREATE_EVENT ambiguous failure");
                return Err(error.into());
            }
        };
        memory.check_queue_currentness()?;
        let shadow_plan = memory.cwsr_shadow_plan(&context_save)?;
        let unpublished_shadows = match LinuxCwsrShadowPagesV1::install(shadow_plan, &event) {
            Ok(shadows) => shadows,
            Err(error) => {
                let _ = memory.quarantine_queue_composition("CWSR shadow setup failure");
                return Err(error.into());
            }
        };
        let cwsr_initialization = match memory.with_bytes_mut(&mut context_save, |bytes| {
            unpublished_shadows
                .shadows()
                .initialize_and_validate_bo_headers(bytes)
        }) {
            Ok(initialization) => initialization,
            Err(error) => {
                let _ = memory.quarantine_queue_composition("CWSR BO initialization failure");
                return Err(error.into());
            }
        };
        if cwsr_initialization.is_err() {
            let _ = memory.quarantine_queue_composition("CWSR header readback failure");
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "gfx942 CWSR header initialization",
            ));
        }
        if let Some(control) = runtime_control.as_ref() {
            runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?;
        } else {
            runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?;
        }
        event.validate_live_with_shadows(
            memory.kfd_fd(),
            memory.opener_pid(),
            unpublished_shadows.shadows(),
        )?;
        memory.check_queue_currentness()?;
        let eop = memory.seal_executable(eop)?;
        let context_save = memory.seal_executable(context_save)?;
        unpublished_shadows
            .shadows()
            .restore_kernel_write_access_after_bo_seal()?;
        let ring = ring.map_and_retain(&mut memory)?;
        let control = memory.map_to_gpu(control)?;
        let completion_signals = memory.map_to_gpu(completion_signals)?;
        let eop = memory.map_executable_to_gpu(eop)?;
        let context_save = memory.map_executable_to_gpu(context_save)?;
        let control = memory.retain_aql_control_resource(control)?;
        let completion_signals =
            memory.retain_aql_completion_signal_resource(completion_signals)?;
        let eop = memory.retain_aql_eop_resource(eop)?;
        let context_save = memory.retain_aql_context_save_resource(context_save)?;
        let authority = build_resource_authority(
            memory.queue_model_device(),
            geometry,
            ring,
            control,
            eop,
            context_save,
        )?;
        let completion_owner = CompletionSignalArenaOwnerV1::new(
            authority.view.plan.queue,
            completion_signals.facts(),
        )?;
        let submission = NativeAqlSubmissionOwnerV1::new(ring_bytes)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("AQL ring submission model"))?;
        let (identity, model) = match dispatch.as_ref() {
            Some(dispatch) => {
                let device_authorities = dispatch.device_authorities();
                memory.take_queue_model_foundation_with_dispatch_memory(&device_authorities)?
            }
            None => memory.take_queue_model_foundation()?,
        };
        let backend = LinuxNativeQueueBackendV1 {
            session: memory,
            foundation: Some(QueueModelFoundationV1 {
                identity,
                memory: model,
            }),
            foundation_in_engine: false,
        };
        let mut engine = NativeQueueEngineV1::new(backend).map_err(map_native)?;
        let key = engine.admit(authority).map_err(map_native)?;
        let mut shadows = None;
        engine
            .create_at_native_boundary(key, || {
                // The backend call is the first boundary where KFD may retain
                // the header's payload pointer. Ambiguous native effects from
                // this point require process teardown.
                shadows = Some(unpublished_shadows.publish_for_native_queue_creation());
            })
            .map_err(map_create)?;
        let shadows = shadows.expect("native CREATE_QUEUE boundary published CWSR shadows");
        runtime
            .mark_queue_created()
            .map_err(|error| terminal_creation("runtime queue-live transition", error.into()))?;
        let outputs = engine.create_outputs(key).ok_or_else(|| {
            terminal_creation(
                "CREATE_QUEUE output recovery",
                ComputeAqlQueueSessionErrorV1::Contract("missing CREATE outputs"),
            )
        })?;
        let queue_id = engine.native_queue_id(key).ok_or_else(|| {
            terminal_creation(
                "CREATE_QUEUE identity recovery",
                ComputeAqlQueueSessionErrorV1::Contract("missing queue id"),
            )
        })?;
        let mut session = ComputeAqlQueueSessionV1 {
            engine: Some(engine),
            key,
            compute_lane_session: key,
            doorbell: None,
            submission: Some(submission),
            completion_signals: Some(completion_signals),
            completion_owner,
            dispatch,
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            persistent_compute: None,
            #[cfg(test)]
            persistent_compute_test_release: None,
            next_persistent_compute_generation: 1,
            exception: Some(QueueExceptionStateV1 {
                runtime,
                runtime_control,
                event,
                shadows,
            }),
            sdma: None,
            sdma_outstanding_buffers: 0,
            sdma_pool_free: Vec::new(),
            sdma_pool_reuse_count: 0,
            terminal_poisoned: false,
            observation: ComputeAqlQueueObservationV1 {
                queue_id,
                ring_bytes,
                doorbell_slice_bytes: 0,
                doorbell_byte_offset: 0,
                event_id: 0,
                cwsr_shadow_pages: 0,
            },
            auxiliary_compute_lanes: Vec::new(),
        };
        let exception = session.exception.as_ref().expect("queue exception state");
        session.observation.event_id = exception.event.event_id_observation();
        session.observation.cwsr_shadow_pages =
            u8::try_from(crate::queue_linux::GFX942_CWSR_SHADOW_PAGES_V1).map_err(|_| {
                terminal_creation(
                    "CWSR shadow page count",
                    ComputeAqlQueueSessionErrorV1::Contract("CWSR shadow page count"),
                )
            })?;
        session
            .check_currentness()
            .map_err(|error| terminal_creation("post-create currentness before doorbell", error))?;
        let doorbell = {
            let engine = session.engine.as_ref().expect("session engine");
            LinuxDoorbellSliceV1::map(engine.backend.session.kfd_fd(), outputs, engine.opener_pid)
        }
        .map_err(|error| terminal_creation("doorbell mapping", error.into()))?;
        session.observation.doorbell_slice_bytes = doorbell.slice_bytes();
        session.observation.doorbell_byte_offset = doorbell.queue_byte_offset();
        session.doorbell = Some(doorbell);
        session
            .check_currentness()
            .map_err(|error| terminal_creation("post-doorbell currentness", error))?;
        Ok(session)
    }

    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.observation
    }

    /// Adds one generic gfx942 SDMA queue to this session.
    ///
    /// Any failure is terminal because USERPTR registration or CREATE_QUEUE may
    /// already have changed native state without returning exact custody.
    pub fn enable_sdma_copy_engine(
        &mut self,
    ) -> Result<Gfx942SdmaQueueObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        let key = self.key;
        let created = self.with_live_queue_memory_model(|memory| {
            Gfx942SdmaQueueSetV1::create_generic(memory, key).map_err(Into::into)
        });
        match created {
            Ok(owner) => {
                let observation = owner
                    .generic_observation()
                    .expect("created generic SDMA queue set");
                self.sdma = Some(owner);
                Ok(observation)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Adds the exact gfx942 directional SDMA profile to this session.
    ///
    /// Admission requires exactly two ordinary SDMA engines with eight queues
    /// per engine. KFD engine index 1 handles H2D and index 0 handles D2H, as
    /// observed in the pinned ROCr gfx94x policy.
    pub fn enable_gfx942_directional_sdma_copy_engines(
        &mut self,
    ) -> Result<Gfx942DirectionalSdmaQueueObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        let key = self.key;
        let created = self.with_live_queue_memory_model(|memory| {
            Gfx942SdmaQueueSetV1::create_directional(memory, key).map_err(Into::into)
        });
        match created {
            Ok(owner) => {
                let observation = owner
                    .directional_observation()
                    .expect("created directional SDMA queue set");
                self.sdma = Some(owner);
                Ok(observation)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Adds one exact gfx942 SDMA queue targeted by KFD engine index.
    ///
    /// This diagnostic control admits only index 0 or 1 after observing the
    /// exact two-engine/eight-queues-per-engine topology profile. The index is
    /// not the public HSA engine bit mask.
    pub fn enable_gfx942_sdma_copy_engine_on_engine_index(
        &mut self,
        engine_index: u32,
    ) -> Result<Gfx942SdmaQueueObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA copy engine is already enabled",
            ));
        }
        let key = self.key;
        let created = self.with_live_queue_memory_model(|memory| {
            Gfx942SdmaQueueSetV1::create_targeted(memory, key, engine_index).map_err(Into::into)
        });
        match created {
            Ok(owner) => {
                let observation = owner
                    .generic_observation()
                    .expect("created targeted single SDMA queue set");
                self.sdma = Some(owner);
                Ok(observation)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Adds a balanced round-robin set of targeted gfx942 SDMA queues.
    ///
    /// `queue_count` must be even and in `2..=16`. Creation admits exactly two
    /// ordinary engines and eight queues per engine from the retained topology;
    /// each successive queue targets alternating engine indices 0 and 1.
    pub fn enable_gfx942_striped_sdma_copy_engines(
        &mut self,
        queue_count: u32,
    ) -> Result<Vec<Gfx942SdmaQueueObservationV1>, ComputeAqlQueueSessionErrorV1> {
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
        let key = self.key;
        let created = self.with_live_queue_memory_model(|memory| {
            Gfx942SdmaQueueSetV1::create_striped(memory, key, queue_count).map_err(Into::into)
        });
        match created {
            Ok((owner, observations)) => {
                self.sdma = Some(owner);
                Ok(observations)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    pub fn allocate_sdma_host_buffer(
        &mut self,
        bytes: usize,
    ) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
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
            self.persistent_compute.is_some(),
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
            wait_result = Some(owner.wait_for(memory, ticket, timeout));
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
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.persistent_compute.is_some(),
            SdmaPublicationModeV1::DirectionalCopy(direction),
        ) {
            return Err(retryable(error.into(), allocation, host));
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
            let prepared = match owner.prepare_single_recoverable(memory, request) {
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
            let prepared = match owner.prepare_single_recoverable(memory, request) {
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
            wait_result = Some(owner.wait_for(memory, ticket, timeout));
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
        let (mut allocation, host) = preserve_directional_window_sdma_publication_custody_v1(
            self.persistent_compute.is_some(),
            direction,
            allocation,
            host,
        )?;
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
            wait_result =
                Some(owner.wait_persistent_window_for(memory, &submission.tickets, timeout));
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
            self.persistent_compute.is_some(),
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
            wait_result =
                Some(owner.wait_persistent_window_for(memory, &submission.tickets, timeout));
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
        self.require_sdma_enabled()?;
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
            self.persistent_compute.is_some(),
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
            self.persistent_compute.is_some(),
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

    /// Preflights and then publishes one balanced batch across every striped SDMA queue.
    ///
    /// All shards are prepared before the first queue write-pointer publication. Native queues
    /// cannot be rolled back as a group, so a later shard failure reports confirmed earlier
    /// shards, one optional indeterminate retained shard, and every untouched request separately.
    /// Terminal custody is observation-only and must remain retained until process teardown.
    // Inline custody avoids allocating an error after native publication has begun.
    #[allow(clippy::result_large_err)]
    pub fn submit_gfx942_striped_sdma_copy_batch_v1(
        &mut self,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaMultiQueueSubmissionFailureV1> {
        if let Err(error) = self.require_sdma_enabled() {
            let disposition = classify_multi_queue_availability_failure(self.terminal_poisoned);
            let custody = if self.terminal_poisoned {
                Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                    Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(requests),
                )
            } else {
                Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests)
            };
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error,
                disposition,
                custody,
            });
        }
        if let Err(error) = admit_sdma_publication_while_compute_detached(
            false,
            self.persistent_compute.is_some(),
            SdmaPublicationModeV1::StripedBatch,
        ) {
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: error.into(),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests),
            });
        }
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests),
            });
        }
        if !self
            .sdma
            .as_ref()
            .is_some_and(Gfx942SdmaQueueSetV1::is_striped)
        {
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "multi-queue submission requires striped SDMA queues",
                ),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(requests),
            });
        }
        if let Err(error) = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        }) {
            self.poison_terminal();
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error,
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                    Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(requests),
                ),
            });
        }

        let mut pending = Some(requests);
        let mut submitted = None;
        let operation = self.with_sdma_owner_memory(|owner, memory| {
            let requests = pending.take().expect("multi-queue requests consumed once");
            submitted = Some(owner.submit_striped_multi_queue_batch(memory, requests));
            Ok(())
        });
        if submitted.is_none() {
            self.poison_terminal();
            return Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                error: operation
                    .err()
                    .unwrap_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "multi-queue operation did not execute",
                    )),
                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication,
                custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                    Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(
                        pending.expect("unexecuted operation retains requests"),
                    ),
                ),
            });
        }
        let post = self.with_sdma_owner_memory(|_, memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        });
        let closing_error = operation.err().or_else(|| post.err());
        match submitted.expect("executed multi-queue operation stores result") {
            Ok(submission) => match closing_error {
                None => {
                    let committed = self
                        .sdma
                        .as_mut()
                        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                            "missing striped SDMA cursor owner",
                        ))
                        .and_then(|owner| {
                            owner
                                .commit_striped_multi_queue_success(submission.plan())
                                .map_err(Into::into)
                        });
                    match committed {
                        Ok(()) => Ok(submission),
                        Err(error) => {
                            self.poison_terminal();
                            Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                                error,
                                disposition: Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPostPublication,
                                custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                                    Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(
                                        submission,
                                    ),
                                ),
                            })
                        }
                    }
                }
                Some(error) => {
                    self.poison_terminal();
                    Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                        error,
                        disposition:
                            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPostPublication,
                        custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                            Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(submission),
                        ),
                    })
                }
            },
            Err(MultiQueueSdmaSubmitFailureV1::Preparation(failure)) => {
                let poisoned = self
                    .sdma
                    .as_ref()
                    .is_none_or(Gfx942SdmaQueueSetV1::is_poisoned);
                let disposition =
                    classify_multi_queue_preparation_failure(poisoned, closing_error.is_some());
                let terminal =
                    disposition == Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication;
                let error = closing_error.unwrap_or_else(|| failure.error.into());
                if terminal {
                    self.poison_terminal();
                }
                let custody = if terminal {
                    Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::before_publication(failure.requests),
                    )
                } else {
                    Gfx942SdmaMultiQueueFailureCustodyV1::RetryableRequests(failure.requests)
                };
                Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                    error,
                    disposition,
                    custody,
                })
            }
            Err(MultiQueueSdmaSubmitFailureV1::Publication(failure)) => {
                self.poison_terminal();
                let disposition = classify_multi_queue_publication_failure(
                    failure.published.len(),
                    failure.indeterminate.is_some(),
                );
                Err(Gfx942SdmaMultiQueueSubmissionFailureV1 {
                    error: closing_error.unwrap_or_else(|| failure.error.into()),
                    disposition,
                    custody: Gfx942SdmaMultiQueueFailureCustodyV1::ProcessTeardown(
                        Gfx942SdmaMultiQueueTerminalCustodyV1::publication(
                            failure.plan,
                            failure.published,
                            failure.indeterminate,
                            failure.unpublished,
                        ),
                    ),
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
            self.persistent_compute.is_some(),
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
            owner.wait_for(memory, ticket, timeout).map_err(Into::into)
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

    #[allow(clippy::result_large_err)]
    fn terminal_persistent_retained_control_replay_after_detach_v1(
        &mut self,
        mut replay: PersistentRetainedControlReplayDetachedV1,
        custody: PersistentComputeTerminalNativeCustodyV1,
        error: ComputeAqlQueueSessionErrorV1,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Gfx942PersistentComputeBindFailureV1 {
        let disposition = match &custody {
            PersistentComputeTerminalNativeCustodyV1::Storage(_) => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Storage,
                    false,
                    false,
                    false,
                )
            }
            PersistentComputeTerminalNativeCustodyV1::Data(_) => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Data,
                    false,
                    false,
                    false,
                )
            }
            PersistentComputeTerminalNativeCustodyV1::Attached => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Attached,
                    false,
                    false,
                    false,
                )
            }
            _ => unreachable!("replay bind admits only pre-publication custody"),
        };
        debug_assert!(matches!(
            (&custody, disposition),
            (
                PersistentComputeTerminalNativeCustodyV1::Storage(_),
                PersistentRetainedControlReplayDispositionV1::TerminalStorage,
            ) | (
                PersistentComputeTerminalNativeCustodyV1::Data(_),
                PersistentRetainedControlReplayDispositionV1::TerminalData,
            ) | (
                PersistentComputeTerminalNativeCustodyV1::Attached,
                PersistentRetainedControlReplayDispositionV1::TerminalAttached,
            )
        ));
        let state = quarantine_persistent_retained_control_replay_prepared_v1(
            &mut replay.allocation.owner,
            replay.prepared,
        );
        self.dispatch = Some(replay.dispatch);
        self.persistent_compute = Some(PersistentComputeAttachmentV1 {
            allocation: replay.allocation,
            authenticated_sha256: replay.authenticated_sha256,
            state,
            binding: PersistentComputeBindingKeyV1 {
                queue: self.key,
                attachment_generation: commit.attachment_generation,
            },
            storage_identity: commit.storage_identity,
            effect: commit.effect,
            predecessor_dispatch_generation: Some(commit.predecessor_generation),
            terminal_custody: Some(custody),
        });
        self.next_persistent_compute_generation = commit.next_attachment_generation;
        self.poison_terminal();
        Gfx942PersistentComputeBindFailureV1 {
            error,
            custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    fn finish_persistent_retained_control_replay_before_detach_v1(
        &mut self,
        request: PersistentRetainedControlReplayRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
        loan_succeeded: bool,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Gfx942PersistentComputeBindFailureV1 {
        let PersistentRetainedControlReplayRequestV1 {
            mut input,
            prepared,
            dispatch,
            initialized_content: _,
            control_identity: _,
            predecessor_generation: _,
        } = request;
        self.dispatch = Some(dispatch);
        let cancellation = persistent_compute_input_allocation_mut_v1(&mut input)
            .owner
            .cancel_prepared(prepared);
        let cancellation_succeeded = cancellation.is_ok();
        let disposition = classify_persistent_retained_control_replay_failure_v1(
            PersistentRetainedControlReplayCustodyStageV1::Input,
            loan_succeeded,
            cancellation_succeeded,
            !self.terminal_poisoned,
        );
        match (disposition, cancellation) {
            (PersistentRetainedControlReplayDispositionV1::RetryableInput, Ok(())) => {
                persistent_retained_control_replay_input_failure_v1(error, input, true)
            }
            (PersistentRetainedControlReplayDispositionV1::TerminalInput, Ok(())) => {
                self.poison_terminal();
                persistent_retained_control_replay_input_failure_v1(error, input, false)
            }
            (PersistentRetainedControlReplayDispositionV1::TerminalAttached, Err(failure)) => {
                let (_, prepared) = failure.into_parts();
                let (mut allocation, authenticated_sha256, _) = input.into_parts();
                let state = quarantine_persistent_retained_control_replay_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                );
                self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    state,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation: commit.attachment_generation,
                    },
                    storage_identity: commit.storage_identity,
                    effect: commit.effect,
                    predecessor_dispatch_generation: Some(commit.predecessor_generation),
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                });
                self.next_persistent_compute_generation = commit.next_attachment_generation;
                self.poison_terminal();
                Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                    ),
                }
            }
            _ => unreachable!("replay failure disposition matches exact cancellation custody"),
        }
    }

    #[allow(clippy::result_large_err)]
    fn bind_retained_persistent_fixed_dispatch_control_replay_v1(
        &mut self,
        request: PersistentRetainedControlReplayRequestV1,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Result<Gfx942PreparedPersistentComputeDispatchV1, Gfx942PersistentComputeBindFailureV1>
    {
        let mut request = Some(request);
        let mut outcome = None;
        let fused_loan = self.with_live_queue_memory_model(|memory| {
            let pipeline = execute_persistent_retained_control_replay_pipeline_v1(
                memory,
                request
                    .take()
                    .expect("opened replay loan consumes its request exactly once"),
                |memory, replay_request| {
                    let allocation =
                        persistent_compute_input_allocation_mut_v1(&mut replay_request.input);
                    let lease = allocation
                        .owner
                        .local_native_for_sdma()
                        .expect("replay preflight validated attached local native custody");
                    memory
                        .mapped_gfx942_device_memory_facts(lease)
                        .map(|_| ())
                        .map_err(ComputeAqlQueueSessionErrorV1::from)
                },
                |_, mut replay_request| {
                    let detached = {
                        let allocation =
                            persistent_compute_input_allocation_mut_v1(&mut replay_request.input);
                        allocation
                            .owner
                            .detach_local_native_for_compute(&replay_request.prepared)
                    };
                    let lease = match detached {
                        Ok(lease) => lease,
                        Err(error) => {
                            return Err((
                                map_directional_persistent_sdma_use_error_v1(error),
                                replay_request,
                            ));
                        }
                    };
                    let PersistentRetainedControlReplayRequestV1 {
                        input,
                        prepared,
                        dispatch,
                        initialized_content,
                        control_identity,
                        predecessor_generation,
                    } = replay_request;
                    let (allocation, authenticated_sha256, _) = input.into_parts();
                    Ok(PersistentRetainedControlReplayStorageV1 {
                        replay: PersistentRetainedControlReplayDetachedV1 {
                            allocation,
                            prepared,
                            dispatch,
                            authenticated_sha256,
                        },
                        lease,
                        initialized_content,
                        control_identity,
                        predecessor_generation,
                    })
                },
                |_, storage| {
                    let PersistentRetainedControlReplayStorageV1 {
                        replay,
                        lease,
                        initialized_content,
                        control_identity,
                        predecessor_generation,
                    } = storage;
                    let data = match initialized_content {
                        Some(content) => {
                            match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                                lease, content,
                            ) {
                                Ok(initialized) => Gfx942FixedDispatchDataV1::initialized(initialized),
                                Err(lease) => {
                                    return Err((
                                        ComputeAqlQueueSessionErrorV1::Contract(
                                            "persistent compute authenticated extent changed after preflight",
                                        ),
                                        PersistentRetainedControlReplayStorageV1 {
                                            replay,
                                            lease,
                                            initialized_content,
                                            control_identity,
                                            predecessor_generation,
                                        },
                                    ));
                                }
                            }
                        }
                        None => Gfx942FixedDispatchDataV1::uninitialized(lease),
                    };
                    Ok(PersistentRetainedControlReplayDataV1 {
                        replay,
                        data,
                        control_identity,
                        predecessor_generation,
                    })
                },
                |memory, data| {
                    let PersistentRetainedControlReplayDataV1 {
                        mut replay,
                        data,
                        control_identity,
                        predecessor_generation,
                    } = data;
                    if let Err((error, data)) = replay.dispatch.retain_persistent_replay_data_v1(
                        memory,
                        control_identity,
                        data,
                        predecessor_generation,
                    ) {
                        return Err((
                            error.into(),
                            PersistentRetainedControlReplayDataV1 {
                                replay,
                                data,
                                control_identity,
                                predecessor_generation,
                            },
                        ));
                    }
                    Ok(replay)
                },
                |memory, replay| {
                    let device_authorities = replay.dispatch.device_authorities();
                    memory
                        .validate_persistent_replay_dispatch_memory(&device_authorities)
                        .map_err(Into::into)
                },
            );
            outcome = Some(match pipeline {
                PersistentRetainedControlReplayPipelineOutcomeV1::BeforeDetach {
                    request,
                    error,
                } => PersistentRetainedControlReplayOutcomeV1::BeforeDetach { request, error },
                PersistentRetainedControlReplayPipelineOutcomeV1::Storage { storage, error } => {
                    PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                        replay: storage.replay,
                        custody: PersistentComputeTerminalNativeCustodyV1::Storage(
                            Gfx942SdmaBufferStorageV1::Device(storage.lease),
                        ),
                        error,
                    }
                }
                PersistentRetainedControlReplayPipelineOutcomeV1::Data { data, error } => {
                    PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                        replay: data.replay,
                        custody: PersistentComputeTerminalNativeCustodyV1::Data(vec![data.data]),
                        error,
                    }
                }
                PersistentRetainedControlReplayPipelineOutcomeV1::Attached {
                    attached,
                    error,
                } => PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                    replay: attached,
                    custody: PersistentComputeTerminalNativeCustodyV1::Attached,
                    error,
                },
                PersistentRetainedControlReplayPipelineOutcomeV1::Ready(replay) => {
                    PersistentRetainedControlReplayOutcomeV1::Ready(replay)
                }
            });
            Ok(())
        });

        let (outcome, loan_error) = match resolve_persistent_retained_control_replay_loan_v1(
            request,
            outcome,
            fused_loan,
            || {
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent replay foundation loan did not execute",
                )
            },
        ) {
            PersistentRetainedControlReplayLoanResolutionV1::Unopened { request, error } => {
                return Err(
                    self.finish_persistent_retained_control_replay_before_detach_v1(
                        request, error, false, commit,
                    ),
                );
            }
            PersistentRetainedControlReplayLoanResolutionV1::Executed {
                outcome,
                retake_error,
            } => (outcome, retake_error),
        };
        let loan_succeeded = loan_error.is_none();
        match outcome {
            PersistentRetainedControlReplayOutcomeV1::BeforeDetach { request, error } => Err(self
                .finish_persistent_retained_control_replay_before_detach_v1(
                    request,
                    loan_error.unwrap_or(error),
                    loan_succeeded,
                    commit,
                )),
            PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                replay,
                custody,
                error,
            } => Err(
                self.terminal_persistent_retained_control_replay_after_detach_v1(
                    replay,
                    custody,
                    loan_error.unwrap_or(error),
                    commit,
                ),
            ),
            PersistentRetainedControlReplayOutcomeV1::Ready(replay) => {
                if let Some(error) = loan_error {
                    return Err(
                        self.terminal_persistent_retained_control_replay_after_detach_v1(
                            replay,
                            PersistentComputeTerminalNativeCustodyV1::Attached,
                            error,
                            commit,
                        ),
                    );
                }
                let binding = PersistentComputeBindingKeyV1 {
                    queue: self.key,
                    attachment_generation: commit.attachment_generation,
                };
                self.dispatch = Some(replay.dispatch);
                self.detached_data_count = 0;
                self.detached_dispatch_generation = None;
                self.detached_data_identities.clear();
                self.detached_next_insertion_index = None;
                self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                    allocation: replay.allocation,
                    authenticated_sha256: replay.authenticated_sha256,
                    state: PersistentComputeUseStateV1::Prepared(replay.prepared),
                    binding,
                    storage_identity: commit.storage_identity,
                    effect: commit.effect,
                    predecessor_dispatch_generation: Some(commit.predecessor_generation),
                    terminal_custody: None,
                });
                self.next_persistent_compute_generation = commit.next_attachment_generation;
                Ok(Gfx942PreparedPersistentComputeDispatchV1 {
                    binding,
                    thread_affinity: PhantomData,
                })
            }
        }
    }

    /// Detaches one exactly completed and recycled fixed batch while keeping
    /// the native queue and all queue resources live.
    #[allow(clippy::result_large_err)]
    pub fn bind_directional_persistent_fixed_dispatch_v1(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; 1],
        mut input: Gfx942PersistentComputeInputV1,
        content_role: Gfx942DeviceContentRoleV1,
    ) -> Result<Gfx942PreparedPersistentComputeDispatchV1, Gfx942PersistentComputeBindFailureV1>
    {
        let recover = |error, input| Gfx942PersistentComputeBindFailureV1 {
            error,
            custody: Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input),
        };
        if !input.belongs_to(self.compute_lane_session) {
            return Err(recover(
                if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned.into()
                } else {
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute input owner substitution",
                    )
                },
                input,
            ));
        }
        if self.terminal_poisoned {
            return Err(Gfx942PersistentComputeBindFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                    Gfx942PersistentComputeBindTerminalCustodyV1 { input: Some(input) },
                ),
            });
        }
        if self.persistent_compute.is_some() {
            return Err(recover(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                input,
            ));
        }
        if self.key != self.compute_lane_session {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute requires the primary compute lane",
                ),
                input,
            ));
        }
        let detached_generation = self.detached_dispatch_generation;
        let detached_is_empty = self.detached_data_count == 0
            && self.detached_data_identities.is_empty()
            && match detached_generation {
                None => self.detached_next_insertion_index.is_none(),
                Some(_) => self.detached_next_insertion_index == Some(0),
            };
        if !detached_is_empty {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute requires an empty initial or recycled dispatch roster",
                ),
                input,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(recover(error, input));
        }
        let competing_queues_quiescent = self
            .sdma
            .as_ref()
            .is_some_and(Gfx942SdmaQueueSetV1::persistent_compute_is_quiescent)
            && auxiliary_compute_lanes_are_quiescent_v1(&self.auxiliary_compute_lanes);
        input = preserve_persistent_compute_bind_input_for_sdma_quiescence_v1(
            input,
            competing_queues_quiescent,
        )?;

        let authenticated_sha256 = match &input {
            Gfx942PersistentComputeInputV1::Uninitialized(_) => None,
            Gfx942PersistentComputeInputV1::Initialized(ready) => Some(ready.authenticated_sha256),
            Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_) => None,
        };
        let initialized = !matches!(&input, Gfx942PersistentComputeInputV1::Uninitialized(_));
        let allocation = match &mut input {
            Gfx942PersistentComputeInputV1::Uninitialized(allocation)
            | Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) => allocation,
            Gfx942PersistentComputeInputV1::Initialized(ready) => &mut ready.allocation,
        };
        if allocation.attachment.queue != self.compute_lane_session
            || !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment)
            || allocation.byte_len() != allocation.physical_byte_len()
            || allocation.owner.live_use_count() != 0
            || allocation.owner.retained_settled_use_count() != 0
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute requires exact quiescent full-extent directional custody",
                ),
                input,
            ));
        }
        let (layout, storage_identity) = {
            let Some(lease) = allocation.owner.local_native_for_sdma() else {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute requires attached local native custody",
                    ),
                    input,
                ));
            };
            (
                super::dispatch_binding::Gfx942FixedDispatchDataLayoutV1::device_local(
                    lease.layout().requested_bytes(),
                    lease.layout().alignment(),
                ),
                lease.storage_identity(),
            )
        };
        // Exact replay may recover initialization from the queue-retained
        // predecessor premise after identity/currentness validation. Initial
        // read admission still requires an authenticated input.
        let control_initialized = initialized || self.dispatch.is_some();
        let control_identity = match persistent_fixed_dispatch_control_identity_v1(
            self.key,
            &programs,
            &packets,
            layout,
            control_initialized,
            content_role,
            Gfx942SdmaBufferStorageIdentityV1::Device(storage_identity),
        ) {
            Ok(identity) => identity,
            Err(error) => return Err(recover(error.into(), input)),
        };
        if let Some(dispatch) = self.dispatch.as_ref() {
            let Some(predecessor_generation) = detached_generation else {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "retained persistent control requires a recycled predecessor generation",
                    ),
                    input,
                ));
            };
            if let Err(error) =
                dispatch.validate_persistent_replay_v1(control_identity, predecessor_generation)
            {
                return Err(recover(error.into(), input));
            }
        }
        let native_effect = control_identity.effect();
        let (operation, effect) = match native_effect {
            DeviceDataEffectV1::ReadOnly => (
                Gfx942PersistentOperationV1::ComputeRead,
                Gfx942PersistentComputeEffectV1::Read,
            ),
            DeviceDataEffectV1::WriteOnly => (
                Gfx942PersistentOperationV1::ComputeWrite,
                Gfx942PersistentComputeEffectV1::Write,
            ),
            DeviceDataEffectV1::ReadWrite => (
                Gfx942PersistentOperationV1::ComputeReadWrite,
                Gfx942PersistentComputeEffectV1::ReadWrite,
            ),
        };
        let initialized_content = match authenticated_sha256 {
            Some(sha256) => match Gfx942DeviceContentDescriptorV1::new(
                content_role,
                allocation.byte_len(),
                sha256,
            ) {
                Ok(content) => Some(content),
                Err(_) => {
                    return Err(recover(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent compute initialized-content relabeling",
                        ),
                        input,
                    ));
                }
            },
            None => None,
        };
        let attachment_generation = self.next_persistent_compute_generation;
        let Some(next_attachment_generation) = attachment_generation.checked_add(1) else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute attachment generation exhausted",
                ),
                input,
            ));
        };
        let request = Gfx942PersistentUseRequestV1::new(operation, 0, allocation.byte_len())
            .expect("nonzero admitted persistent allocation extent");
        let reserved = match allocation.owner.reserve(request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(recover(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    input,
                ));
            }
        };
        let prepared = match allocation.owner.prepare(reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, reserved) = failure.into_parts();
                let _ = allocation.owner.cancel_reserved(reserved);
                return Err(recover(
                    map_directional_persistent_sdma_use_error_v1(error),
                    input,
                ));
            }
        };
        if let Some(dispatch) = self.dispatch.take() {
            let predecessor_generation = detached_generation
                .expect("persistent replay was preflighted with a recycled predecessor");
            return self.bind_retained_persistent_fixed_dispatch_control_replay_v1(
                PersistentRetainedControlReplayRequestV1 {
                    input,
                    prepared,
                    dispatch,
                    initialized_content,
                    control_identity,
                    predecessor_generation,
                },
                PersistentRetainedControlReplayCommitV1 {
                    attachment_generation,
                    next_attachment_generation,
                    storage_identity,
                    effect,
                    predecessor_generation,
                },
            );
        }
        let validation = {
            let lease = allocation
                .owner
                .local_native_for_sdma()
                .expect("validated local native custody");
            self.with_live_queue_memory_model(|memory| {
                memory
                    .mapped_gfx942_device_memory_facts(lease)
                    .map(|_| ())
                    .map_err(Into::into)
            })
        };
        if let Err(error) = validation {
            match allocation.owner.cancel_prepared(prepared) {
                Ok(()) if !self.terminal_poisoned => return Err(recover(error, input)),
                Ok(()) => {
                    return Err(Gfx942PersistentComputeBindFailureV1 {
                        error,
                        custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                            Gfx942PersistentComputeBindTerminalCustodyV1 { input: Some(input) },
                        ),
                    });
                }
                Err(failure) => {
                    let (_, prepared) = failure.into_parts();
                    let (mut allocation, authenticated_sha256, _) = input.into_parts();
                    let _ = allocation.owner.quarantine_prepared(
                        prepared,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    );
                    self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                        allocation,
                        authenticated_sha256,
                        state: PersistentComputeUseStateV1::Quarantined,
                        binding: PersistentComputeBindingKeyV1 {
                            queue: self.key,
                            attachment_generation,
                        },
                        storage_identity,
                        effect,
                        predecessor_dispatch_generation: detached_generation,
                        terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                    });
                    self.next_persistent_compute_generation = next_attachment_generation;
                    self.poison_terminal();
                    return Err(Gfx942PersistentComputeBindFailureV1 {
                        error,
                        custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                            Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                        ),
                    });
                }
            }
        }
        let lease = match allocation.owner.detach_local_native_for_compute(&prepared) {
            Ok(lease) => lease,
            Err(error) => {
                let _ = allocation.owner.cancel_prepared(prepared);
                return Err(recover(
                    map_directional_persistent_sdma_use_error_v1(error),
                    input,
                ));
            }
        };
        let (mut allocation, authenticated_sha256, initialized) = input.into_parts();
        let data = match initialized_content {
            Some(content) => {
                match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                    lease, content,
                ) {
                    Ok(initialized) => Gfx942FixedDispatchDataV1::initialized(initialized),
                    Err(_lease) => {
                        allocation
                            .owner
                            .quarantine_prepared(
                                prepared,
                                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                            )
                            .expect("private prepared use remains current");
                        self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                            allocation,
                            authenticated_sha256,
                            state: PersistentComputeUseStateV1::Quarantined,
                            binding: PersistentComputeBindingKeyV1 {
                                queue: self.key,
                                attachment_generation,
                            },
                            storage_identity,
                            effect,
                            predecessor_dispatch_generation: detached_generation,
                            terminal_custody: Some(
                                PersistentComputeTerminalNativeCustodyV1::Storage(
                                    Gfx942SdmaBufferStorageV1::Device(_lease),
                                ),
                            ),
                        });
                        self.next_persistent_compute_generation = next_attachment_generation;
                        self.poison_terminal();
                        return Err(Gfx942PersistentComputeBindFailureV1 {
                            error: ComputeAqlQueueSessionErrorV1::Contract(
                                "persistent compute authenticated extent changed after preflight",
                            ),
                            custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                            ),
                        });
                    }
                }
            }
            None => Gfx942FixedDispatchDataV1::uninitialized(lease),
        };
        debug_assert_eq!(initialized, authenticated_sha256.is_some());
        let loan = match self.restore_model_ownership_for_live_mutation() {
            Ok(loan) => loan,
            Err(error) => {
                allocation
                    .owner
                    .quarantine_prepared(
                        prepared,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    )
                    .expect("private prepared use remains current");
                self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    state: PersistentComputeUseStateV1::Quarantined,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation,
                    },
                    storage_identity,
                    effect,
                    predecessor_dispatch_generation: detached_generation,
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Data(vec![
                        data,
                    ])),
                });
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                return Err(Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                    ),
                });
            }
        };
        let prepared_dispatch = {
            let memory = &mut self
                .engine
                .as_mut()
                .expect("model loan requires queue engine")
                .backend
                .session;
            prepare_persistent_fixed_dispatch_resources_v1(
                memory,
                programs,
                packets,
                data,
                detached_generation,
                control_identity,
            )
        };
        let retake = self.retake_model_ownership_after_live_mutation(loan);
        let prepared_dispatch = match (prepared_dispatch, retake) {
            (Ok(dispatch), Ok(())) => Ok(dispatch),
            (Err(failure), Ok(())) => Err((failure.error.into(), failure.data)),
            (Ok(dispatch), Err(error)) => {
                self.dispatch = Some(dispatch);
                Err((error, Vec::new()))
            }
            (Err(failure), Err(error)) => {
                let _ = failure.error;
                Err((error, failure.data))
            }
        };
        let prepared_dispatch = match prepared_dispatch {
            Ok(dispatch) => dispatch,
            Err((error, data)) => {
                allocation
                    .owner
                    .quarantine_prepared(
                        prepared,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    )
                    .expect("private prepared use remains current");
                self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    state: PersistentComputeUseStateV1::Quarantined,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation,
                    },
                    storage_identity,
                    effect,
                    predecessor_dispatch_generation: detached_generation,
                    terminal_custody: Some(if data.is_empty() {
                        PersistentComputeTerminalNativeCustodyV1::Attached
                    } else {
                        PersistentComputeTerminalNativeCustodyV1::Data(data)
                    }),
                });
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                return Err(Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                    ),
                });
            }
        };
        let device_authorities = prepared_dispatch.device_authorities();
        let memory = &mut self
            .engine
            .as_mut()
            .expect("checked queue engine")
            .backend
            .session;
        let validation = memory.validate_live_queue_dispatch_memory(&device_authorities);
        if let Err(error) = validation {
            allocation
                .owner
                .quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                )
                .expect("private prepared use remains current");
            self.persistent_compute = Some(PersistentComputeAttachmentV1 {
                allocation,
                authenticated_sha256,
                state: PersistentComputeUseStateV1::Quarantined,
                binding: PersistentComputeBindingKeyV1 {
                    queue: self.key,
                    attachment_generation,
                },
                storage_identity,
                effect,
                predecessor_dispatch_generation: detached_generation,
                terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
            });
            self.next_persistent_compute_generation = next_attachment_generation;
            self.dispatch = Some(prepared_dispatch);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeBindFailureV1 {
                error: error.into(),
                custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                    Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                ),
            });
        }
        let binding = PersistentComputeBindingKeyV1 {
            queue: self.key,
            attachment_generation,
        };
        self.dispatch = Some(prepared_dispatch);
        self.detached_data_count = 0;
        self.detached_dispatch_generation = None;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = None;
        self.persistent_compute = Some(PersistentComputeAttachmentV1 {
            allocation,
            authenticated_sha256,
            state: PersistentComputeUseStateV1::Prepared(prepared),
            binding,
            storage_identity,
            effect,
            predecessor_dispatch_generation: detached_generation,
            terminal_custody: None,
        });
        self.next_persistent_compute_generation = next_attachment_generation;
        Ok(Gfx942PreparedPersistentComputeDispatchV1 {
            binding,
            thread_affinity: PhantomData,
        })
    }

    /// Publishes the exact prepared persistent-compute attachment.
    #[allow(clippy::result_large_err)]
    pub fn submit_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeExecutionFailureV1> {
        self.submit_directional_persistent_fixed_dispatch_v1_using(prepared_receipt, |session| {
            session.submit_fixed_dispatch_inner_classified::<1>(
                FixedDispatchBindingModeV1::ExactPersistentAttachment,
            )
        })
    }

    #[allow(clippy::result_large_err)]
    fn submit_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
        submit: impl FnOnce(
            &mut Self,
        )
            -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1>,
    ) -> Result<Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeExecutionFailureV1> {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        if self.terminal_poisoned {
            let retryable = (!self.absorb_terminal_prepared_persistent_compute_v1(binding))
                .then_some(prepared_receipt);
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable,
            });
        }
        let valid = self
            .persistent_compute
            .as_ref()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: Some(prepared_receipt),
            });
        }
        let mut attachment = self
            .persistent_compute
            .take()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Prepared(prepared) = state else {
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: None,
            });
        };
        match submit(self) {
            Ok(batch) => match attachment.allocation.owner.publish(prepared) {
                Ok(published) => {
                    attachment.state = PersistentComputeUseStateV1::Published(published);
                    self.persistent_compute = Some(attachment);
                    Ok(Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch,
                        thread_affinity: PhantomData,
                    })
                }
                Err(failure) => {
                    let (_, prepared) = failure.into_parts();
                    let _ = attachment.allocation.owner.quarantine_prepared(
                        prepared,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                    );
                    attachment.terminal_custody =
                        Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                    self.persistent_compute = Some(attachment);
                    self.poison_terminal();
                    Err(Gfx942PersistentComputeExecutionFailureV1 {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent compute publication ledger transition",
                        ),
                        retryable: None,
                    })
                }
            },
            Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => Err(self
                .restore_retryable_persistent_compute_submission(
                    attachment, prepared, binding, error,
                )),
            Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                let _ = attachment.allocation.owner.quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                let _ = attachment.allocation.owner.quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
        }
    }

    fn restore_retryable_persistent_compute_submission(
        &mut self,
        mut attachment: PersistentComputeAttachmentV1,
        prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        binding: PersistentComputeBindingKeyV1,
        error: ComputeAqlQueueSessionErrorV1,
    ) -> Gfx942PersistentComputeExecutionFailureV1 {
        attachment.state = PersistentComputeUseStateV1::Prepared(prepared);
        self.persistent_compute = Some(attachment);
        Gfx942PersistentComputeExecutionFailureV1 {
            error,
            retryable: Some(Gfx942PreparedPersistentComputeDispatchV1 {
                binding,
                thread_affinity: PhantomData,
            }),
        }
    }

    /// Cancels an exact prepared attachment before publication and restores
    /// the original initialized or uninitialized persistent input.
    #[allow(clippy::result_large_err)]
    pub fn cancel_prepared_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeInputV1, Gfx942PersistentComputeCancelFailureV1> {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            let recovered = (!self.absorb_terminal_prepared_persistent_compute_v1(binding))
                .then_some(prepared_receipt);
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                recovered,
                retained: None,
            });
        }
        let valid = self
            .persistent_compute
            .as_ref()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(prepared_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .persistent_compute
            .take()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Prepared(prepared) = state else {
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: None,
            });
        };
        let returned = self.release_persistent_dispatch_data(false);
        let (generation, mut data) = match returned {
            Ok(returned) => returned,
            Err((error, data)) => {
                let _ = attachment.allocation.owner.quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody = Some(if data.is_empty() && self.dispatch.is_some() {
                    PersistentComputeTerminalNativeCustodyV1::Attached
                } else {
                    PersistentComputeTerminalNativeCustodyV1::Data(data)
                });
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeCancelFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        let expected_generation = attachment.predecessor_dispatch_generation.unwrap_or(0);
        let exact = generation == expected_generation
            && data.len() == 1
            && data[0].sdma_storage_identity()
                == Gfx942SdmaBufferStorageIdentityV1::Device(attachment.storage_identity);
        if !exact {
            let _ = attachment.allocation.owner.quarantine_prepared(
                prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Data(data));
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute cancellation returned substituted storage",
                ),
                recovered: None,
                retained: None,
            });
        }
        let data = data.pop().expect("validated one returned data authority");
        let fully_initialized = data.is_fully_initialized();
        let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
            unreachable!("validated device storage identity")
        };
        if let Err((_error, lease)) = attachment
            .allocation
            .owner
            .restore_local_native_from_cancelled_compute(&prepared, lease)
        {
            let _ = attachment.allocation.owner.quarantine_prepared(
                prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Storage(
                Gfx942SdmaBufferStorageV1::Device(lease),
            ));
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute cancellation native restore",
                ),
                recovered: None,
                retained: None,
            });
        }
        if attachment
            .allocation
            .owner
            .cancel_prepared(prepared)
            .is_err()
        {
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Restored);
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute cancellation ledger transition",
                ),
                recovered: None,
                retained: None,
            });
        }
        self.detached_dispatch_generation = Some(expected_generation);
        self.detached_next_insertion_index = Some(0);
        self.detached_data_count = 0;
        self.detached_data_identities.clear();
        Ok(Gfx942PersistentComputeInputV1::from_parts(
            attachment.allocation,
            attachment.authenticated_sha256,
            fully_initialized,
        ))
    }

    #[allow(clippy::result_large_err)]
    fn poll_directional_persistent_fixed_dispatch_inner_v1<Completed>(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
        observe: impl FnOnce(
            &mut Self,
            Gfx942CompletionBatchV1<1>,
        ) -> Result<
            PersistentComputeCompletionObservationV1<Completed>,
            (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<1>),
        >,
        into_completed: impl FnOnce(Completed) -> Gfx942CompletedBatchV1<1>,
    ) -> Result<
        PersistentComputePollTransitionV1<
            Gfx942PersistentComputeDispatchV1,
            PersistentComputeCompletedTransitionV1<Completed>,
        >,
        Gfx942PersistentComputePollFailureV1,
    > {
        let binding = dispatch_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self
                .absorb_terminal_published_persistent_compute_v1(binding, dispatch_receipt.batch)
            {
                Ok(()) => Err(Gfx942PersistentComputePollFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(batch) => Err(Gfx942PersistentComputePollFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .persistent_compute
            .as_ref()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(dispatch_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .persistent_compute
            .take()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Published(published) = state else {
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: Some(PersistentComputeTerminalNativeCustodyV1::Published(
                    dispatch_receipt.batch,
                )),
            });
        };
        let (completion, generation) = unwrap_published(dispatch_receipt.batch);
        let generation_is_current = self
            .dispatch
            .as_ref()
            .and_then(|dispatch| dispatch.active_generation().ok())
            == Some(generation);
        if !generation_is_current {
            let batch = wrap_published(completion, generation);
            let _ = attachment.allocation.owner.quarantine_published(
                published,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        let completed = match observe(self, completion) {
            Ok(PersistentComputeCompletionObservationV1::Pending(batch)) => {
                attachment.state = PersistentComputeUseStateV1::Published(published);
                self.persistent_compute = Some(attachment);
                return Ok(PersistentComputePollTransitionV1::Pending(
                    Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch: wrap_published(batch, generation),
                        thread_affinity: PhantomData,
                    },
                ));
            }
            Ok(PersistentComputeCompletionObservationV1::Ready(completed)) => completed,
            Err((error, completion)) => {
                let batch = wrap_published(completion, generation);
                let _ = attachment.allocation.owner.quarantine_published(
                    published,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputePollFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("persistent dispatch retained")
            .mark_completed(generation)
            .is_err()
        {
            let completed = wrap_completed(into_completed(completed), generation);
            let _ = attachment.allocation.owner.quarantine_published(
                published,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        let completed_use = match attachment.allocation.owner.complete(published) {
            Ok(completed_use) => completed_use,
            Err(failure) => {
                let (_, published) = failure.into_parts();
                let completed = wrap_completed(into_completed(completed), generation);
                let _ = attachment.allocation.owner.quarantine_published(
                    published,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = Some(
                    PersistentComputeTerminalNativeCustodyV1::Completed(completed),
                );
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputePollFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute completion ledger transition",
                    ),
                    recovered: None,
                    retained: None,
                });
            }
        };
        Ok(PersistentComputePollTransitionV1::Ready(
            PersistentComputeCompletedTransitionV1 {
                binding,
                attachment,
                completed_use,
                generation,
                completed,
            },
        ))
    }

    /// Polls one published persistent-compute dispatch, retaining all custody
    /// in either returned typestate.
    #[allow(clippy::result_large_err)]
    pub fn poll_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputePollV1, Gfx942PersistentComputePollFailureV1> {
        let transition = self.poll_directional_persistent_fixed_dispatch_inner_v1(
            dispatch_receipt,
            |session, completion| {
                session
                    .poll_completion_batch_with_progress_retaining(completion)
                    .map(|poll| match poll {
                        Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                            PersistentComputeCompletionObservationV1::Pending(batch)
                        }
                        Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                            PersistentComputeCompletionObservationV1::Ready(completed)
                        }
                    })
            },
            |completed| completed,
        )?;
        match transition {
            PersistentComputePollTransitionV1::Pending(dispatch) => {
                Ok(Gfx942PersistentComputePollV1::Pending(dispatch))
            }
            PersistentComputePollTransitionV1::Ready(PersistentComputeCompletedTransitionV1 {
                binding,
                mut attachment,
                completed_use,
                generation,
                completed,
            }) => {
                attachment.state = PersistentComputeUseStateV1::Completed(completed_use);
                self.persistent_compute = Some(attachment);
                Ok(Gfx942PersistentComputePollV1::Ready(
                    Gfx942CompletedPersistentComputeDispatchV1 {
                        binding,
                        completed: wrap_completed(completed, generation),
                        thread_affinity: PhantomData,
                    },
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1<Completed>(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        mut attachment: PersistentComputeAttachmentV1,
        completed_use: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        generation: u64,
        completed: Completed,
        recycle: impl FnOnce(
            &mut Self,
            Completed,
        ) -> Result<
            Gfx942CompletionRecycleObservationV1,
            (ComputeAqlQueueSessionErrorV1, Completed),
        >,
        into_completed: impl FnOnce(Completed) -> Gfx942CompletedBatchV1<1>,
    ) -> Result<Gfx942RecycledPersistentComputeDispatchV1, Gfx942PersistentComputeRecycleFailureV1>
    {
        let recycle = match recycle(self, completed) {
            Ok(recycle) => recycle,
            Err((error, completed)) => {
                let completed = wrap_completed(into_completed(completed), generation);
                let _ = attachment.allocation.owner.quarantine_completed(
                    completed_use,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = Some(
                    PersistentComputeTerminalNativeCustodyV1::Completed(completed),
                );
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("persistent dispatch retained")
            .mark_recycled(generation)
            .is_err()
        {
            let _ = attachment.allocation.owner.quarantine_completed(
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        attachment.state = PersistentComputeUseStateV1::Recycled(completed_use);
        self.persistent_compute = Some(attachment);
        Ok(Gfx942RecycledPersistentComputeDispatchV1 {
            binding,
            recycle,
            thread_affinity: PhantomData,
        })
    }

    /// Polls one published persistent-compute dispatch and, on Ready, recycles
    /// its exact completion signal without reopening the just-closed
    /// currentness envelope. Pending preserves the ordinary two-check poll.
    #[allow(clippy::result_large_err)]
    pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
    ) -> Result<
        Gfx942PersistentComputePollAndRecycleV1,
        Gfx942PersistentComputePollAndRecycleFailureV1,
    > {
        let transition = execute_persistent_compute_poll_and_recycle_v1(
            self,
            |session| {
                session.poll_directional_persistent_fixed_dispatch_inner_v1(
                    dispatch_receipt,
                    |session, completion| {
                        session
                            .poll_completion_batch_with_current_handoff_retaining(completion)
                            .map(|poll| match poll {
                                CompletionPollWithCurrentnessHandoffV1::Pending {
                                    batch, ..
                                } => PersistentComputeCompletionObservationV1::Pending(batch),
                                CompletionPollWithCurrentnessHandoffV1::Ready {
                                    handoff, ..
                                } => PersistentComputeCompletionObservationV1::Ready(handoff),
                            })
                    },
                    CompletionCurrentnessHandoffV1::into_completed,
                )
            },
            |_| Instant::now(),
            |session, completed| {
                let PersistentComputeCompletedTransitionV1 {
                    binding,
                    attachment,
                    completed_use,
                    generation,
                    completed: handoff,
                } = completed;
                session.finish_directional_persistent_fixed_dispatch_recycle_inner_v1(
                    binding,
                    attachment,
                    completed_use,
                    generation,
                    handoff,
                    Self::recycle_completion_current_handoff_retaining,
                    CompletionCurrentnessHandoffV1::into_completed,
                )
            },
        );
        match transition {
            Ok(PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)) => {
                Ok(Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch))
            }
            Ok(PersistentComputePollAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
            }) => Ok(Gfx942PersistentComputePollAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
            }),
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => Err(
                Gfx942PersistentComputePollAndRecycleFailureV1::Poll(failure),
            ),
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => Err(
                Gfx942PersistentComputePollAndRecycleFailureV1::Recycle(failure),
            ),
        }
    }

    /// Recycles the exact completion signal after device completion.
    #[allow(clippy::result_large_err)]
    pub fn recycle_directional_persistent_fixed_dispatch_v1(
        &mut self,
        completed_receipt: Gfx942CompletedPersistentComputeDispatchV1,
    ) -> Result<Gfx942RecycledPersistentComputeDispatchV1, Gfx942PersistentComputeRecycleFailureV1>
    {
        let binding = completed_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(completed_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self.absorb_terminal_completed_persistent_compute_v1(
                binding,
                completed_receipt.completed,
            ) {
                Ok(()) => Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(completed) => Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942CompletedPersistentComputeDispatchV1 {
                        binding,
                        completed,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .persistent_compute
            .as_ref()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(completed_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .persistent_compute
            .take()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: Some(PersistentComputeTerminalNativeCustodyV1::Completed(
                    completed_receipt.completed,
                )),
            });
        };
        let (completion, generation) = unwrap_completed(completed_receipt.completed);
        let generation_is_current = self
            .dispatch
            .as_ref()
            .and_then(|dispatch| dispatch.active_generation().ok())
            == Some(generation);
        if !generation_is_current {
            let completed = wrap_completed(completion, generation);
            let _ = attachment.allocation.owner.quarantine_completed(
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        self.finish_directional_persistent_fixed_dispatch_recycle_inner_v1(
            binding,
            attachment,
            completed_use,
            generation,
            completion,
            Self::recycle_completion_batch_retaining,
            |completed| completed,
        )
    }

    /// Detaches the recycled batch, restores the exact mapped HBM authority to
    /// its persistent owner, and settles the compute ledger use.
    #[allow(clippy::result_large_err)]
    pub fn detach_recycled_directional_persistent_fixed_dispatch_v1(
        &mut self,
        recycled_receipt: Gfx942RecycledPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeCompletedV1, Gfx942PersistentComputeDetachFailureV1> {
        let binding = recycled_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(recycled_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self
                .absorb_terminal_recycled_persistent_compute_v1(binding, recycled_receipt.recycle)
            {
                Ok(()) => Err(Gfx942PersistentComputeDetachFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(recycle) => Err(Gfx942PersistentComputeDetachFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942RecycledPersistentComputeDispatchV1 {
                        binding,
                        recycle,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .persistent_compute
            .as_ref()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(recycled_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .persistent_compute
            .take()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Recycled(completed_use) = state else {
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: Some(PersistentComputeTerminalNativeCustodyV1::Recycled(
                    recycled_receipt.recycle,
                )),
            });
        };
        let release_preflight = self
            .completion_owner
            .ensure_releasable()
            .map_err(ComputeAqlQueueSessionErrorV1::from)
            .and_then(|()| {
                (self.detached_data_count == 0
                    && self.detached_dispatch_generation.is_none()
                    && self.detached_data_identities.is_empty()
                    && self.detached_next_insertion_index.is_none())
                .then_some(())
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute detach requires an empty detached-data ledger",
                ))
            });
        if let Err(error) = release_preflight {
            let _ = attachment.allocation.owner.quarantine_completed(
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error,
                recovered: None,
                retained: None,
            });
        }
        let (generation, mut data) = match self
            .detach_persistent_dispatch_data_retaining_control_v1()
        {
            Ok(returned) => returned,
            Err((error, data)) => {
                let _ = attachment.allocation.owner.quarantine_completed(
                    completed_use,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody = Some(if data.is_empty() && self.dispatch.is_some() {
                    PersistentComputeTerminalNativeCustodyV1::Attached
                } else {
                    PersistentComputeTerminalNativeCustodyV1::Data(data)
                });
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeDetachFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        let exact = generation != 0
            && recycled_receipt.recycle.packet_count() == 1
            && data.len() == 1
            && data[0].sdma_storage_identity()
                == Gfx942SdmaBufferStorageIdentityV1::Device(attachment.storage_identity);
        if !exact {
            let _ = attachment.allocation.owner.quarantine_completed(
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Data(data));
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute detach returned substituted storage",
                ),
                recovered: None,
                retained: None,
            });
        }
        let data = data.pop().expect("validated one detached data authority");
        let fully_initialized = data.is_fully_initialized();
        let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
            unreachable!("validated device storage identity")
        };
        if let Err((_error, lease)) = attachment
            .allocation
            .owner
            .restore_local_native_from_compute(&completed_use, lease)
        {
            let _ = attachment.allocation.owner.quarantine_completed(
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Storage(
                Gfx942SdmaBufferStorageV1::Device(lease),
            ));
            self.persistent_compute = Some(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("persistent compute native restore"),
                recovered: None,
                retained: None,
            });
        }
        let frontier = match attachment.allocation.owner.settle(completed_use) {
            Ok(frontier) => frontier,
            Err(failure) => {
                let (_, completed_use) = failure.into_parts();
                let _ = attachment.allocation.owner.quarantine_completed(
                    completed_use,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Restored);
                self.persistent_compute = Some(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeDetachFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract("persistent compute settlement"),
                    recovered: None,
                    retained: None,
                });
            }
        };
        self.detached_dispatch_generation = Some(generation);
        self.detached_data_count = 0;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = Some(0);
        Ok(Gfx942PersistentComputeCompletedV1 {
            allocation: attachment.allocation,
            frontier,
            effect: attachment.effect,
            authenticated_sha256: (!attachment.effect.writes())
                .then_some(attachment.authenticated_sha256)
                .flatten(),
            fully_initialized,
        })
    }

    pub fn detach_recycled_fixed_dispatch(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.detach_recycled_fixed_dispatch_inner()
    }

    /// Consumes a detached persistent dispatch's immutable code/kernarg
    /// control without changing the queue's detached-generation ledger.
    pub fn release_retained_persistent_fixed_dispatch_control_v1(
        &mut self,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let Some(dispatch) = self.dispatch.as_ref() else {
            return Ok(false);
        };
        if !dispatch.persistent_data_is_detached_v1() {
            return Ok(false);
        }
        let Some(generation) = self.detached_dispatch_generation else {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "retained persistent control lost its detached generation",
            ));
        };
        if let Err(error) = dispatch.validate_detached_persistent_control_release_v1(generation) {
            self.poison_terminal();
            return Err(error.into());
        }
        let full_currentness = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))
            .and_then(|engine| {
                engine
                    .backend
                    .session
                    .check_queue_currentness()
                    .map_err(Into::into)
            });
        if let Err(error) = full_currentness {
            self.poison_terminal();
            return Err(error);
        }
        let loan = self.restore_model_ownership_for_live_mutation()?;
        let dispatch = self
            .dispatch
            .take()
            .expect("validated detached persistent control remains retained");
        let release = dispatch.release_detached_persistent_control_v1(
            &mut self
                .engine
                .as_mut()
                .expect("model loan requires queue engine")
                .backend
                .session,
            generation,
        );
        let retake = self.retake_model_ownership_after_live_mutation(loan);
        match (release, retake) {
            (Ok(()), Ok(())) => Ok(true),
            (Err(error), Ok(())) => {
                self.poison_terminal();
                Err(error.into())
            }
            (_, Err(error)) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    fn detach_recycled_fixed_dispatch_inner(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.detached_data_count != 0
            || self.detached_dispatch_generation.is_some()
            || !self.detached_data_identities.is_empty()
            || self.detached_next_insertion_index.is_some()
        {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger was not empty",
            ));
        }
        self.completion_owner.ensure_releasable()?;
        let dispatch = self
            .dispatch
            .take()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let returned = self.with_live_queue_memory_model(|memory| {
            dispatch
                .release_non_data_after_recycle(memory)
                .map_err(Into::into)
        });
        let returned = match returned {
            Ok(returned) => returned,
            Err(error) => {
                self.poison_terminal();
                return Err(error);
            }
        };
        let generation = returned.generation();
        let data = recover_fixed_dispatch_data(returned);
        if generation == 0 {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch generation was zero",
            ));
        }
        self.detached_data_count = data.len();
        self.detached_dispatch_generation = Some(generation);
        self.detached_data_identities = fixed_dispatch_storage_identities(&data);
        self.detached_next_insertion_index = None;
        Ok(Gfx942DetachedFixedDispatchV1 { generation, data })
    }

    /// Binds a new fixed batch to the same live native queue.
    ///
    /// The queue must have no attached batch. The complete detached data set is
    /// rebound, and its device-local subset is revalidated against the retained
    /// KFD session before the new owner is installed. Every mapped storage input
    /// and inspected program is retained even when no packet in this batch
    /// selects it. This does not publish.
    pub fn bind_fixed_dispatch<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.dispatch.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.detached_dispatch_generation.is_none() {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let data_identities = fixed_dispatch_storage_identities(&data);
        if self.detached_data_identities.len() != self.detached_data_count {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger cardinality",
            ));
        }
        if data.len() != self.detached_data_count {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data.len().min(self.detached_data_count),
                detail: "detached dispatch-data cardinality",
            }
            .into());
        }
        if let Some(index) =
            first_ordered_identity_mismatch(&self.detached_data_identities, &data_identities)
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "detached rebind storage identity",
            }
            .into());
        }
        self.completion_owner.ensure_releasable()?;
        validate_fixed_batch_ring::<N>(self.observation.ring_bytes)?;
        let predecessor_generation = self
            .detached_dispatch_generation
            .expect("checked detached dispatch generation");
        let prepared = self.with_live_queue_memory_model(|memory| {
            prepare_public_fixed_dispatch_resources_after_detach(
                memory,
                programs,
                packets,
                data,
                predecessor_generation,
            )
            .map_err(Into::into)
        });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.poison_terminal();
                return Err(error);
            }
        };
        let device_authorities = prepared.device_authorities();
        let validation = self
            .engine
            .as_mut()
            .expect("checked queue engine")
            .backend
            .session
            .validate_live_queue_dispatch_memory(&device_authorities);
        if let Err(error) = validation {
            self.poison_terminal();
            return Err(error.into());
        }
        self.dispatch = Some(prepared);
        self.detached_data_count = 0;
        self.detached_dispatch_generation = None;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = None;
        Ok(())
    }

    /// Allocates and maps one uninitialized device-local extent while no fixed
    /// batch is attached.
    pub fn allocate_uninitialized_fixed_dispatch_data(
        &mut self,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .allocate_gfx942_device_memory(requested_bytes, alignment)
                .and_then(|lease| memory.map_gfx942_device_memory(lease))
                .map_err(Into::into)
        });
        match result {
            Ok(lease) => {
                let data = Gfx942FixedDispatchDataV1::uninitialized(lease);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates, writes, verifies, CPU-unmaps, and GPU-maps one fully
    /// initialized device-local extent while no fixed batch is attached.
    pub fn initialize_fixed_dispatch_data(
        &mut self,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_gfx942_device_memory(bytes, alignment, content)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::initialized(memory);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates and inserts one initialized device-local extent at an exact
    /// detached data ordinal without replacing an existing allocation.
    ///
    /// The insertion ordinal is validated before allocation. It is intended
    /// for service ledgers that keep device-local entries before coherent host
    /// entries while changing the detached allocation cardinality.
    pub fn insert_initialized_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_gfx942_device_memory(bytes, alignment, content)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::initialized(memory);
                self.record_new_detached_data_at(&data, data_index);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Validates an exact detached-data insertion without allocating, mapping,
    /// or changing queue state.
    ///
    /// Service layers can use this before mutating their own allocation ledger,
    /// so a full lower data roster or an invalid ordinal remains a retry-safe
    /// rejection with unchanged custody.
    pub fn preflight_fixed_dispatch_data_insertion(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)
    }

    /// Overwrites one initialized coherent extent retained from the immediately
    /// preceding completed dispatch while the queue is unbound.
    ///
    /// The exact detached storage identity and bounds are checked before the
    /// mapped bytes are changed. Native handles and GPU addresses remain private.
    pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        data: &mut Gfx942FixedDispatchDataV1,
        offset: u64,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        let expected_identity = *self.detached_data_identities.get(data_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite ordinal",
            },
        )?;
        if data.storage_identity() != expected_identity {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite storage identity",
            }
            .into());
        }
        let end = offset
            .checked_add(u64::try_from(source.len()).map_err(|_| {
                Gfx942DispatchBindingErrorV1::InvalidData {
                    index: data_index,
                    detail: "detached overwrite source length",
                }
            })?)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite range overflow",
            })?;
        if source.is_empty() || end > data.layout().requested_bytes() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite range",
            }
            .into());
        }
        let token = data.initialized_host_visible_token_mut().ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite requires initialized coherent storage",
            },
        )?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .overwrite_mapped_host_visible_subrange(token, offset, source)
                .map_err(Into::into)
        });
        if let Err(error) = result {
            self.poison_terminal();
            return Err(error);
        }
        Ok(())
    }

    /// Allocates, initializes, and inserts one coherent host-visible extent at
    /// an exact detached data ordinal.
    pub fn insert_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_host_visible_coherent(bytes)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_initialized(memory);
                self.record_new_detached_data_at(&data, data_index);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates and initializes one coherent host-visible extent at the exact
    /// ordinal vacated by the immediately preceding detached release.
    pub fn initialize_host_visible_fixed_dispatch_data(
        &mut self,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        if self.detached_next_insertion_index.is_none() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_host_visible_coherent(bytes)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_initialized(memory);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates, maps, and inserts one uninitialized coherent host-visible
    /// extent at an exact detached data ordinal.
    pub fn insert_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        requested_bytes: usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)?;
        let result = self.with_live_queue_memory_model(|memory| {
            let allocation = memory.allocate_host_visible_coherent(requested_bytes)?;
            memory.map_to_gpu(allocation).map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory);
                self.record_new_detached_data_at(&data, data_index);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates and maps one uninitialized coherent host-visible extent at the
    /// exact ordinal vacated by the immediately preceding detached release.
    pub fn allocate_host_visible_fixed_dispatch_data(
        &mut self,
        requested_bytes: usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        if self.detached_next_insertion_index.is_none() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let result = self.with_live_queue_memory_model(|memory| {
            let allocation = memory.allocate_host_visible_coherent(requested_bytes)?;
            memory.map_to_gpu(allocation).map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Unmaps and releases detached fixed-dispatch storage exactly once.
    pub fn release_detached_fixed_dispatch_data(
        &mut self,
        data: Gfx942FixedDispatchDataV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        let identity = data.storage_identity();
        let mut matching = self
            .detached_data_identities
            .iter()
            .enumerate()
            .filter(|(_, retained)| **retained == identity);
        let Some((identity_index, _)) = matching.next() else {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.detached_data_count,
                detail: "detached release storage identity",
            }
            .into());
        };
        if matching.next().is_some() {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "duplicate detached storage identity",
            ));
        }
        let result = self.with_live_queue_memory_model(|memory| {
            memory.release_fixed_dispatch_data(data).map_err(Into::into)
        });
        if let Err(error) = result {
            self.poison_terminal();
            return Err(error);
        }
        self.detached_data_count = self.detached_data_count.checked_sub(1).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("detached dispatch-data ledger underflow"),
        )?;
        self.detached_data_identities.remove(identity_index);
        self.detached_next_insertion_index = Some(identity_index);
        Ok(())
    }

    fn require_unbound_fixed_dispatch(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.dispatch.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.detached_dispatch_generation.is_none() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.detached_data_count > super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger bound",
            ));
        }
        if self.detached_data_identities.len() != self.detached_data_count
            || self
                .detached_next_insertion_index
                .is_some_and(|index| index > self.detached_data_identities.len())
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger",
            ));
        }
        self.completion_owner.ensure_releasable()?;
        Ok(())
    }

    fn require_new_detached_data_index(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        validate_new_detached_data_index(self.detached_data_count, data_index).map_err(Into::into)
    }

    fn record_new_detached_data(&mut self, data: &Gfx942FixedDispatchDataV1) {
        insert_detached_identity(
            &mut self.detached_data_identities,
            &mut self.detached_next_insertion_index,
            data.storage_identity(),
        );
        self.detached_data_count += 1;
    }

    fn record_new_detached_data_at(&mut self, data: &Gfx942FixedDispatchDataV1, data_index: usize) {
        insert_detached_identity_at(
            &mut self.detached_data_identities,
            &mut self.detached_next_insertion_index,
            data.storage_identity(),
            data_index,
        );
        self.detached_data_count += 1;
    }

    fn require_detached_allocation_capacity(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.detached_data_count >= super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
                requested: self.detached_data_count + 1,
                maximum: super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1,
            }
            .into());
        }
        Ok(())
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

    /// Private end-to-end binding of real retained dispatch resources to C2
    /// publication and C4 per-packet completion. No public caller can construct
    /// the required resource owner inputs.
    /// Publishes the entire prepared fixed batch with one ring reservation and
    /// one final doorbell store, retaining one completion signal per packet.
    pub fn submit_fixed_dispatch<const N: usize>(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.submit_fixed_dispatch_inner::<N>()
    }

    fn submit_fixed_dispatch_inner<const N: usize>(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.submit_fixed_dispatch_inner_classified::<N>(FixedDispatchBindingModeV1::Ordinary)
            .map_err(FixedDispatchSubmissionFailureV1::into_error)
    }

    fn submit_fixed_dispatch_inner_classified<const N: usize>(
        &mut self,
        mode: FixedDispatchBindingModeV1,
    ) -> Result<Gfx942DispatchBatchV1<N>, FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
            ));
        }
        let binding = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
            .and_then(|dispatch| dispatch.bind_templates::<N>(self.key));
        let templates = self.classify_fixed_dispatch_binding(mode, binding)?;
        let generation = match self
            .dispatch
            .as_ref()
            .expect("dispatch owner was just bound")
            .active_generation()
        {
            Ok(generation) => generation,
            Err(error) => {
                self.poison_terminal();
                return Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()));
            }
        };
        let completion = self.submit_with_completions_classified(templates);
        let result = finish_fixed_dispatch_submission(generation, completion, |generation| {
            self.dispatch
                .as_mut()
                .expect("dispatch owner retained")
                .cancel_binding(generation)
        });
        if matches!(&result, Err(FixedDispatchSubmissionFailureV1::Terminal(_))) {
            self.poison_terminal();
        }
        result
    }

    #[cfg(test)]
    fn submit_fixed_dispatch_inner_classified_with_test_owner(
        &mut self,
        owner: &mut super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1,
        template: impl FnOnce(u64) -> CompletionPacketTemplateV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<1>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
        let generation = owner
            .bind_one()
            .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))?;
        let completion =
            self.submit_with_completions_classified_using([template(generation)], native_submit);
        let result = finish_fixed_dispatch_submission(generation, completion, |generation| {
            owner.cancel_binding(generation)
        });
        if matches!(&result, Err(FixedDispatchSubmissionFailureV1::Terminal(_))) {
            self.poison_terminal();
        }
        result
    }

    fn classify_fixed_dispatch_binding<T>(
        &mut self,
        mode: FixedDispatchBindingModeV1,
        binding: Result<T, Gfx942DispatchBindingErrorV1>,
    ) -> Result<T, FixedDispatchSubmissionFailureV1> {
        binding.map_err(|error| {
            let error = error.into();
            match mode {
                FixedDispatchBindingModeV1::Ordinary => {
                    FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)
                }
                FixedDispatchBindingModeV1::ExactPersistentAttachment => {
                    self.poison_terminal();
                    FixedDispatchSubmissionFailureV1::Terminal(error)
                }
            }
        })
    }

    /// Polls every packet signal once and returns linear pending or completed custody.
    pub fn poll_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        match self.poll_fixed_dispatch_with_progress_inner(batch)? {
            Gfx942DispatchPollWithProgressV1::Pending { batch, .. } => {
                Ok(Gfx942DispatchPollV1::Pending(batch))
            }
            Gfx942DispatchPollWithProgressV1::Ready { completed, .. } => {
                Ok(Gfx942DispatchPollV1::Ready(completed))
            }
        }
    }

    /// Polls every packet signal once and returns custody plus same-scan progress.
    pub fn poll_fixed_dispatch_with_progress<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.poll_fixed_dispatch_with_progress_inner(batch)
    }

    fn poll_fixed_dispatch_with_progress_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, generation) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .active_generation()?
            != generation
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        match self.poll_completion_batch_with_progress(completion) {
            Ok(poll) => {
                if matches!(poll, Gfx942CompletionPollWithProgressV1::Ready { .. })
                    && self
                        .dispatch
                        .as_mut()
                        .expect("dispatch owner retained")
                        .mark_completed(generation)
                        .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_poll_with_progress(poll, generation))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Performs a bounded wait for every signal in the exact published batch.
    pub fn wait_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        polls: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let (completion, generation) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .active_generation()?
            != generation
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        match self.wait_completion_batch(completion, polls) {
            Ok(completion) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_completed(generation)
                    .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_completed(completion, generation))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Waits for the exact published batch until a monotonic relative deadline.
    ///
    /// This is the preferred blocking API. It performs a short latency spin,
    /// then yields and sleeps with bounded backoff. The poll-count method is
    /// retained for compatibility with callers that require an observation
    /// budget rather than a time budget.
    pub fn wait_fixed_dispatch_for<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        timeout_milliseconds: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let (completion, generation) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .active_generation()?
            != generation
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_milliseconds));
        match self.wait_completion_batch_until(completion, deadline) {
            Ok(completion) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_completed(generation)
                    .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_completed(completion, generation))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Recycles all completed signal slots and returns the queue to prepared state.
    pub fn recycle_fixed_dispatch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.recycle_fixed_dispatch_inner(completed)
    }

    fn recycle_fixed_dispatch_inner<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, ComputeAqlQueueSessionErrorV1> {
        let (completion, generation) = unwrap_completed(completed);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .active_generation()?
            != generation
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        let observation = match self.recycle_completion_batch(completion) {
            Ok(observation) => observation,
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                return Err(error);
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("dispatch owner retained")
            .mark_recycled(generation)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        Ok(observation)
    }

    /// Returns the exact dispatch generation only while its completion signals
    /// have been observed and recycled and the same batch remains attached.
    pub fn recycled_fixed_dispatch_generation(&self) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.persistent_compute.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .ensure_returnable()
            .map_err(Into::into)
    }

    /// Copies one inspected writable subrange from coherent host-visible data.
    ///
    /// The exact attached dispatch must have completed and recycled. Device-local
    /// storage, read-only or unwritten bytes, stale generations, invalid bounds,
    /// and requests intersecting more than one admitted writable range fail
    /// before any mapped bytes are exposed.
    pub fn read_recycled_fixed_dispatch_data(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.persistent_compute.is_some(),
            GenericRecycledDispatchAccessV1::Read,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible(memory, request);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies one inspected writable coherent subrange into caller-owned bytes.
    ///
    /// This has the same generation, effect, kind, and bounds checks as
    /// [`Self::read_recycled_fixed_dispatch_data`] but avoids an intermediate
    /// owned readback allocation when the caller already owns the destination.
    pub fn read_recycled_fixed_dispatch_data_into(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.persistent_compute.is_some(),
            GenericRecycledDispatchAccessV1::ReadInto,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible_into(memory, request, destination);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies one exact admitted enclosing snapshot from coherent host-visible data.
    ///
    /// The exact attached dispatch must have completed and recycled. Admission
    /// requires a retained fully initialized range strictly enclosing one
    /// isolated inspected writable binding. Subranges, stale generations,
    /// device-local storage, and undeclared ranges fail before bytes are exposed.
    pub fn read_recycled_fixed_dispatch_snapshot(
        &mut self,
        request: Gfx942CompletedDispatchSnapshotRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.persistent_compute.is_some(),
            GenericRecycledDispatchAccessV1::Snapshot,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible_snapshot(memory, request);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Overwrites one initialized coherent range while the attached dispatch
    /// is exactly completed, recycled, and ready for another generation.
    pub fn overwrite_recycled_fixed_dispatch_host_data(
        &mut self,
        request: Gfx942RecycledDispatchWriteRequestV1,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.persistent_compute.is_some(),
            GenericRecycledDispatchAccessV1::Overwrite,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.overwrite_recycled_host_visible(memory, request, source);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
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
        if result.is_err() {
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
        if result.is_err() {
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
        self.completion_owner.poison_owner();
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
        if self.persistent_compute.is_some() {
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
            .map_or(0, Gfx942SdmaQueueSetV1::additional_resource_count);
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
        self.require_sdma_enabled()?;
        if requested_bytes == 0 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "pooled buffer length must be nonzero",
            ));
        }
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

    fn with_sdma_owner_memory<R>(
        &mut self,
        operation: impl FnOnce(
            &mut Gfx942SdmaQueueSetV1,
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        let mut owner = self.sdma.take().expect("checked SDMA owner");
        let result = self.with_live_queue_memory_model(|memory| operation(&mut owner, memory));
        self.sdma = Some(owner);
        result
    }

    fn with_live_queue_memory_model<R>(
        &mut self,
        operation: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<R, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        let loan = self.restore_model_ownership_for_live_mutation()?;
        let result = {
            let engine = self
                .engine
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?;
            operation(&mut engine.backend.session)
        };
        if let Err(error) = self.retake_model_ownership_after_live_mutation(loan) {
            self.poison_terminal();
            return Err(error);
        }
        result
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
        let loan = self
            .restore_model_ownership_for_live_mutation()
            .map_err(|error| (error, Vec::new()))?;
        let Some(dispatch) = self.dispatch.take() else {
            let retake = self.retake_model_ownership_after_live_mutation(loan);
            return Err((
                retake
                    .err()
                    .unwrap_or_else(|| Gfx942DispatchBindingErrorV1::ResourcePhase.into()),
                Vec::new(),
            ));
        };
        let result = {
            let memory = &mut self
                .engine
                .as_mut()
                .expect("model loan requires queue engine")
                .backend
                .session;
            if after_recycle {
                dispatch.release_persistent_data_after_recycle(memory)
            } else {
                dispatch.release_persistent_data_before_publication(memory)
            }
            .map_err(|(error, data)| (error.into(), data))
        };
        if let Err(error) = self.retake_model_ownership_after_live_mutation(loan) {
            self.poison_terminal();
            let data = match result {
                Ok((_, data)) | Err((_, data)) => data,
            };
            return Err((error, data));
        }
        result
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
        dispatch
            .detach_persistent_replay_data_after_recycle_v1()
            .map_err(|error| (error.into(), Vec::new()))
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
            .loan_queue_model_foundation_for_live_mutation(
                &mut engine.identity,
                &mut engine.memory,
            )?;
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
            .retake_queue_model_foundation_after_live_mutation(
                &mut engine.identity,
                &mut engine.memory,
                loan,
            )?;
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
        let domain = engine.identity.domain_id();
        let identity = core::mem::replace(&mut engine.identity, DeviceIdentityStateV1::new(domain));
        let memory = core::mem::replace(
            &mut engine.memory,
            MemoryLifecycleStateV1::new_monotonic_non_reusable(domain),
        );
        engine
            .backend
            .session
            .restore_queue_model_foundation(identity, memory)?;
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

fn build_resource_authority(
    current_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    geometry: Gfx942AqlQueueResourcePlanV1,
    ring: RingAuthority,
    control: ControlAuthority,
    eop: EopAuthority,
    context_save: ContextSaveAuthority,
) -> Result<QueueResourceAuthorityV1, ComputeAqlQueueSessionErrorV1> {
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
    let queue_number = NEXT_QUEUE_INSTANCE
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
    let authority = QueueResourceAuthorityV1 {
        ring,
        control,
        eop,
        context_save,
        view,
    };
    validate_resource_authority(&authority).map_err(map_native)?;
    Ok(authority)
}

fn validate_resource_authority(
    authority: &QueueResourceAuthorityV1,
) -> Result<(), NativeQueueAdapterErrorV1> {
    let view = authority.view;
    let facts = [
        authority.ring.facts(),
        authority.control.facts(),
        authority.eop.facts(),
        authority.context_save.facts(),
    ];
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

fn terminal_creation(
    stage: &'static str,
    source: ComputeAqlQueueSessionErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    ComputeAqlQueueSessionErrorV1::TerminalCreation {
        stage,
        source: Box::new(source),
    }
}

fn terminal_userptr_control_creation(
    error: ComputeAqlQueueSessionErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    permanently_poison_process_global_kfd_runtime_gate_v1();
    if error.is_terminal_creation() {
        error
    } else {
        terminal_creation("USERPTR queue-control creation", error)
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
    };
    ComputeAqlQueueSessionErrorV1::Native(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn test_queue_key(queue: u64, generation: u64) -> QueueKeyV1 {
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
    fn multi_queue_failures_distinguish_retryable_and_terminal_truth() {
        assert_eq!(
            classify_multi_queue_preparation_failure(false, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
        );
        assert_eq!(
            classify_multi_queue_preparation_failure(true, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
        assert_eq!(
            classify_multi_queue_preparation_failure(false, true),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
        assert_eq!(
            classify_multi_queue_publication_failure(0, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
        assert_eq!(
            classify_multi_queue_publication_failure(0, true),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
        );
        assert_eq!(
            classify_multi_queue_publication_failure(1, false),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
        );
    }

    #[test]
    fn already_terminal_multi_queue_session_never_advertises_retryable_custody() {
        assert_eq!(
            classify_multi_queue_availability_failure(false),
            Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
        );
        assert_eq!(
            classify_multi_queue_availability_failure(true),
            Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
        );
    }

    #[test]
    fn terminal_shard_observation_returns_the_source_slice_lifetime() {
        fn request_indices<'a>(observation: Gfx942SdmaTerminalShardObservationV1<'a>) -> &'a [u16] {
            observation.request_indices()
        }

        let indices = [1_u16, 5, 9];
        let observation = Gfx942SdmaTerminalShardObservationV1 {
            queue_ordinal: 3,
            queue_id: 17,
            request_indices: &indices,
            retained_ticket_count: indices.len(),
        };
        let retained = request_indices(observation);
        assert_eq!(retained, indices);
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
        install_auxiliary_compute_lane_slot_v1(&mut slots, first, "first");
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
        install_auxiliary_compute_lane_slot_v1(&mut slots, replacement, "replacement");

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
        let source = include_str!("queue_live.rs");
        let promotion = source
            .split("pub fn promote_full_h2d_to_persistent_compute_ready_v1")
            .nth(1)
            .unwrap()
            .split("pub const MAX_COMPUTE_LANES_V1")
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

    fn persistent_compute_cancellation_test_session(
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
            completion_owner:
                CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue),
            dispatch: None,
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            persistent_compute,
            persistent_compute_test_release: release,
            next_persistent_compute_generation: 2,
            exception: None,
            sdma: None,
            sdma_outstanding_buffers: 0,
            sdma_pool_free: Vec::new(),
            sdma_pool_reuse_count: 0,
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

    fn prepared_persistent_compute_cancellation_fixture(
        queue: QueueKeyV1,
        id: u64,
        authenticated_sha256: Option<[u8; 32]>,
        predecessor_dispatch_generation: Option<u64>,
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
        let request = Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::ComputeReadWrite,
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
        let data = Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease);
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        let attachment = PersistentComputeAttachmentV1 {
            allocation,
            authenticated_sha256,
            state: PersistentComputeUseStateV1::Prepared(prepared),
            binding,
            storage_identity,
            effect: Gfx942PersistentComputeEffectV1::ReadWrite,
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
                .persistent_compute
                .as_ref()
                .expect("persistent attachment is restored");
            assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
            assert!(matches!(
                attachment.state,
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
                .persistent_compute
                .as_ref()
                .expect("successful retry retains published attachment custody");
            assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
            assert!(matches!(
                attachment.state,
                PersistentComputeUseStateV1::Published(_)
            ));
            assert!(!session.terminal_poisoned);
        }
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
        assert_eq!(
            session.persistent_compute_terminal_stage_v1(),
            Some(crate::Gfx942PersistentComputeTerminalStageV1::Attached)
        );
        let attachment = session
            .persistent_compute
            .as_ref()
            .expect("terminal submit retains the exact attachment");
        assert!(matches!(
            attachment.state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Attached)
        ));
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
            .persistent_compute
            .as_ref()
            .expect("terminal queue retains the exact attachment");
        assert!(matches!(
            attachment.state,
            PersistentComputeUseStateV1::Quarantined
        ));
        assert!(matches!(
            attachment.terminal_custody,
            Some(PersistentComputeTerminalNativeCustodyV1::Attached)
        ));
    }

    #[test]
    fn persistent_compute_blocks_every_sdma_publication_and_both_directions() {
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
            let error = terminal_userptr_control_creation(ComputeAqlQueueSessionErrorV1::Contract(
                "control fault injection",
            ));
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
    fn session_manifest_digest_is_frozen() {
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
            "4b7e1090eccbae41ea09ce7d5147470eb665ee295cb0f4526f5584225c86369a"
        );
        assert_eq!(
            super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1,
            "811fbd200ac0b72e5aff81494225b6ea37f517d62bad3779544653c2aae6d815"
        );
        assert!(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1.contains(&format!(
            "dispatch_binding_schema_sha256={}\n",
            super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1
        )));
        assert_eq!(
            SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
            "bc7724673724d8cb9b370ac19c92342b17b760217370b977b76c7ae403ef8f38"
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
        let source = include_str!("queue_live.rs");
        let body = source
            .split("fn create_compute_aql_queue_after_userptr_control_entry(")
            .nth(1)
            .unwrap()
            .split("pub const fn observation")
            .next()
            .unwrap();
        let handoff = body
            .find("runtime.take().expect(\"validated debug runtime authority\")")
            .unwrap();
        let event_create = body.find("LinuxQueueExceptionEventV1::create").unwrap();
        let native_boundary = body
            .find("engine\n            .create_at_native_boundary")
            .unwrap();
        let unpublished_payload_disarm = body
            .find("unpublished_shadows.publish_for_native_queue_creation()")
            .unwrap();
        assert!(handoff < event_create);
        assert!(event_create < native_boundary);
        assert!(native_boundary < unpublished_payload_disarm);
        for post_handoff_step in [
            "LinuxCwsrShadowPagesV1::install",
            "initialize_and_validate_bo_headers",
            "memory.seal_executable(eop)",
            "ring.map_and_retain(&mut memory)",
            "memory.take_queue_model_foundation()?",
            "NativeQueueEngineV1::new(backend)",
            "NativeAqlSubmissionOwnerV1::new(ring_bytes)",
            "create_at_native_boundary(key",
            "mark_queue_created()",
            ".create_outputs(key)",
            ".native_queue_id(key)",
            "let mut session = ComputeAqlQueueSessionV1",
            "session\n            .check_currentness()",
            "LinuxDoorbellSliceV1::map",
            "session.doorbell = Some(doorbell)",
        ] {
            assert!(
                event_create < body.find(post_handoff_step).unwrap(),
                "{post_handoff_step}"
            );
        }
    }

    #[test]
    fn auxiliary_queue_publishes_cwsr_payload_only_at_native_create_boundary() {
        let source = include_str!("queue_live.rs");
        let prepared_state = source
            .split("struct PreparedAuxiliaryComputeLaneV1")
            .nth(1)
            .unwrap()
            .split("fn prepare_auxiliary_compute_lane_slot_v1")
            .next()
            .unwrap();
        assert!(prepared_state.contains("unpublished_shadows: LinuxUnpublishedCwsrShadowPagesV1"));
        assert!(!prepared_state.contains("exception: QueueExceptionStateV1"));

        let body = source
            .split("pub fn create_auxiliary_compute_lane_with_fixed_dispatch")
            .nth(1)
            .unwrap()
            .split("pub fn auxiliary_compute_lane_count_v1")
            .next()
            .unwrap();
        let install = body
            .find("let unpublished_shadows = LinuxCwsrShadowPagesV1::install")
            .unwrap();
        let restore = body
            .find("restore_kernel_write_access_after_bo_seal")
            .unwrap();
        let native_boundary = body.find("create_at_native_boundary(key").unwrap();
        let publish = body
            .find("unpublished_shadows.publish_for_native_queue_creation()")
            .unwrap();
        let published_exception = body.find("exception: Some(QueueExceptionStateV1").unwrap();
        assert!(install < restore);
        assert!(restore < native_boundary);
        assert!(native_boundary < publish);
        assert!(publish < published_exception);
        assert_eq!(
            body.matches("publish_for_native_queue_creation()").count(),
            1
        );
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
        trace: Vec<RetainedReplayInjectedStageV1>,
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

    fn run_retained_replay_script_v1(
        fail: Option<RetainedReplayInjectedStageV1>,
    ) -> (
        RetainedReplayScriptOutcomeV1,
        Vec<RetainedReplayInjectedStageV1>,
    ) {
        let mut script = RetainedReplayScriptV1 {
            fail,
            trace: Vec::new(),
        };
        let outcome = execute_persistent_retained_control_replay_pipeline_v1(
            &mut script,
            RetainedReplayScriptRequestV1(0x35),
            |script, request| {
                assert_eq!(request.0, 0x35);
                script
                    .trace
                    .push(RetainedReplayInjectedStageV1::MappedFacts);
                (script.fail != Some(RetainedReplayInjectedStageV1::MappedFacts))
                    .then_some(())
                    .ok_or(RetainedReplayInjectedStageV1::MappedFacts)
            },
            |script, request| {
                assert_eq!(request.0, 0x35);
                script.trace.push(RetainedReplayInjectedStageV1::Detach);
                if script.fail == Some(RetainedReplayInjectedStageV1::Detach) {
                    Err((RetainedReplayInjectedStageV1::Detach, request))
                } else {
                    Ok(RetainedReplayScriptStorageV1(request.0))
                }
            },
            |script, storage| {
                assert_eq!(storage.0, 0x35);
                script
                    .trace
                    .push(RetainedReplayInjectedStageV1::AuthenticatedConstruction);
                if script.fail == Some(RetainedReplayInjectedStageV1::AuthenticatedConstruction) {
                    Err((
                        RetainedReplayInjectedStageV1::AuthenticatedConstruction,
                        storage,
                    ))
                } else {
                    Ok(RetainedReplayScriptDataV1(storage.0))
                }
            },
            |script, data| {
                assert_eq!(data.0, 0x35);
                script.trace.push(RetainedReplayInjectedStageV1::Retain);
                if script.fail == Some(RetainedReplayInjectedStageV1::Retain) {
                    Err((RetainedReplayInjectedStageV1::Retain, data))
                } else {
                    Ok(RetainedReplayScriptAttachedV1(data.0))
                }
            },
            |script, attached| {
                assert_eq!(attached.0, 0x35);
                script.trace.push(RetainedReplayInjectedStageV1::FinalAudit);
                (script.fail != Some(RetainedReplayInjectedStageV1::FinalAudit))
                    .then_some(())
                    .ok_or(RetainedReplayInjectedStageV1::FinalAudit)
            },
        );
        (outcome, script.trace)
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
        let custody = PersistentComputeTerminalNativeCustodyV1::Data(vec![data]);
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
        let source = include_str!("queue_live.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().unwrap();
        let replay = production
            .split("fn bind_retained_persistent_fixed_dispatch_control_replay_v1")
            .nth(1)
            .unwrap()
            .split("/// Detaches one exactly completed and recycled fixed batch")
            .next()
            .unwrap();
        assert_eq!(replay.matches("with_live_queue_memory_model").count(), 1);
        let mapped = replay.find("mapped_gfx942_device_memory_facts").unwrap();
        let detach = replay.find("detach_local_native_for_compute").unwrap();
        let construct = replay
            .find("Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer")
            .unwrap();
        let retain = replay.find("retain_persistent_replay_data_v1").unwrap();
        let replay_validation = replay
            .find("validate_persistent_replay_dispatch_memory")
            .unwrap();
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
        assert!(replay.contains("let mut outcome = None"));
        assert!(replay.contains("PersistentRetainedControlReplayOutcomeV1::Ready(replay)"));
        assert!(replay.contains("PersistentComputeTerminalNativeCustodyV1::Storage"));
        assert!(replay.contains("PersistentComputeTerminalNativeCustodyV1::Data"));
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
        let initial_start = bind.find("let validation = {").unwrap();
        assert!(retained < replay_call);
        assert!(replay_call < initial_start);
        let initial = &bind[initial_start..];
        assert_eq!(initial.matches("with_live_queue_memory_model").count(), 1);
        assert_eq!(
            initial
                .matches("restore_model_ownership_for_live_mutation")
                .count(),
            1
        );
        assert_eq!(
            initial
                .matches("retake_model_ownership_after_live_mutation")
                .count(),
            1
        );
        assert_eq!(
            initial
                .matches("validate_live_queue_dispatch_memory")
                .count(),
            1
        );
        assert!(initial.contains("prepare_persistent_fixed_dispatch_resources_v1"));
        assert!(!initial.contains("validate_persistent_replay_dispatch_memory"));
        assert!(!initial.contains("retain_persistent_replay_data_v1"));

        let release = production
            .split("pub fn release_retained_persistent_fixed_dispatch_control_v1")
            .last()
            .unwrap()
            .split("fn detach_recycled_fixed_dispatch_inner")
            .next()
            .unwrap();
        let close_audit = release.find(".check_queue_currentness()").unwrap();
        let consume_control = release.find(".dispatch\n            .take()").unwrap();
        assert!(close_audit < consume_control);

        let shared_memory = include_str!("shared_memory.rs");
        let operational = shared_memory
            .split("fn validate_persistent_replay_dispatch_memory")
            .nth(1)
            .unwrap()
            .split("fn map_device_memory")
            .next()
            .unwrap();
        assert!(operational.contains("self.check_operational_currentness()?"));
        assert!(operational.contains("validate_dispatch_device_memory_set"));
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

    #[test]
    fn persistent_completion_recycle_fused_route_has_one_ordered_handoff() {
        let production = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let fused = production
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

        let shared_poll = production
            .split("fn poll_directional_persistent_fixed_dispatch_inner_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        let preflight = shared_poll.find("let generation_is_current").unwrap();
        let observe = shared_poll
            .find("let completed = match observe(self")
            .unwrap();
        let dispatch_completed = shared_poll.find(".mark_completed(generation)").unwrap();
        let allocation_completed = shared_poll
            .find("attachment.allocation.owner.complete(published)")
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

        let shared_recycle = production
            .split("fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1")
            .next()
            .unwrap();
        let native_recycle = shared_recycle
            .find("let recycle = match recycle(self")
            .unwrap();
        let dispatch_recycled = shared_recycle.find(".mark_recycled(generation)").unwrap();
        let attachment_recycled = shared_recycle
            .find("attachment.state = PersistentComputeUseStateV1::Recycled")
            .unwrap();
        assert!(native_recycle < dispatch_recycled);
        assert!(dispatch_recycled < attachment_recycled);

        assert!(!production.contains("PersistentCompletionRecycleFailurePointV1"));
        assert!(!production.contains("persistent_completion_recycle_failure_stage_v1"));
        assert!(!production.contains("persistent_completion_recycle_terminal_custody_v1"));
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
            1
        );

        let split_poll = production
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
        let split_recycle = production
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
