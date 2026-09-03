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
    COMPLETION_SIGNAL_ARENA_BYTES_V1, CompletionPacketTemplateV1, CompletionSignalArenaOwnerV1,
    Gfx942BarrierProbeRecycleObservationV1, Gfx942BarrierProbeV1, Gfx942BarrierProbeWaitFailureV1,
    Gfx942CompletedBarrierProbeV1, Gfx942CompletedBatchV1, Gfx942CompletionBatchV1,
    Gfx942CompletionErrorV1, Gfx942CompletionPollV1, Gfx942CompletionPollWithProgressV1,
    Gfx942CompletionRecycleObservationV1, Gfx942CompletionWaitFailureV1,
    Gfx942TimeoutExecutionObservationV1, Gfx942TimeoutSignalObservationV1,
    MAX_COMPLETION_POLL_ATTEMPTS_V1, NativeCompletionSignalBackendV1,
    initialize_pending_completion_signal_arena,
};
use super::dispatch_binding::{
    DeviceDataAllocationInputV1, DispatchGeometryV1, DispatchResourceOwnerV1,
    Gfx942CompletedDispatchBatchV1, Gfx942CompletedDispatchReadRequestV1,
    Gfx942CompletedDispatchReadbackV1, Gfx942CompletedDispatchSnapshotRequestV1,
    Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1, Gfx942DispatchPollV1,
    Gfx942DispatchPollWithProgressV1, Gfx942FixedDispatchDataV1, Gfx942FixedDispatchPacketV1,
    Gfx942FixedDispatchStorageIdentityV1, Gfx942RecycledDispatchWriteRequestV1,
    ReturnedDispatchDataV1, TypedKernargImageV1, prepare_dispatch_resources,
    prepare_public_fixed_dispatch_resources, prepare_public_fixed_dispatch_resources_after_recycle,
    unwrap_completed, unwrap_published, validate_fixed_batch_ring, wrap_completed,
    wrap_poll_with_progress, wrap_published,
};
use super::submit::{
    NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1, NativeAqlSubmissionOwnerV1,
    NativeBarrierAndSubmissionFailureV1, initialize_amd_aql_control, initialize_invalid_ring,
};
use super::*;
use crate::queue_linux::{
    LinuxCwsrShadowPagesV1, LinuxCwsrShadowsAfterEventDestroyedV1,
    LinuxCwsrShadowsReadyForReleaseV1, LinuxDoorbellErrorV1, LinuxDoorbellSliceV1,
    LinuxKfdRuntimeDisabledV1, LinuxKfdRuntimeEnabledV1, LinuxQueueExceptionEventV1,
    LinuxUnpublishedCwsrShadowPagesV1, QueueExceptionWaitObservationV1,
    arm_process_global_kfd_runtime_gate_for_teardown_v1,
    permanently_poison_process_global_kfd_runtime_gate_v1,
};
use crate::sdma::{
    Gfx942DirectionalSdmaQueueObservationV1, Gfx942SdmaBufferKindV1,
    Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1,
    Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyPollV1, Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1, Gfx942SdmaMemoryPoolObservationV1,
    Gfx942SdmaQueueObservationV1, Gfx942SdmaQueueProgressObservationV1, Gfx942SdmaQueueSetV1,
    allocate_device_buffer, allocate_host_buffer, read_host_buffer, release_buffer,
    striped_sdma_queue_count_is_admitted, write_host_buffer,
};
use crate::shared_memory::{
    AqlCompletionSignalResourceRoleV1, AqlContextSaveResourceRoleV1, AqlControlResourceRoleV1,
    AqlEndOfPipeResourceRoleV1, AqlQueueGttV1, AqlRingResourceRoleV1, ExecutableAqlQueueProbeGttV1,
    ExecutableGttV1, Gfx942InitializedHostVisibleMemoryV1, GttCpuWritableV1,
    GttGpuAccessibleExecutableV1, GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1,
    LiveQueueModelFoundationLoanV1, SharedGttAllocationV1, SharedGttMappedResourceFactsV1,
    SharedGttMemorySessionV1, SharedGttQueueResourceAuthorityV1, UserptrAqlControlGttV1,
    UserptrAqlQueueProbeGttV1,
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
static NEXT_QUEUE_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Canonical claim boundary for the live queue and fixed-batch foundation.
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-aql-session-r34-v1\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device\n",
    "memory_profile_sha256=fb01d099eedfb39a60a1763897691684b547c51610b5e62529f2a6ff0eb27f83\n",
    "kfd_userptr_memory_schema_sha256=c1cee09bdf884d2c14a5dbb89c1f6f7885962c75b1457caf412821490919ee9e\n",
    "kfd_userptr_queue_control_schema_sha256=f1d75410d6bfacff2ea15ecfff226eb8aed7912ee324a36b8ed8550fa52bce02\n",
    "queue_resource_profile_sha256=37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02\n",
    "aql_dispatch_schema_sha256=82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0\n",
    "aql_barrier_and_schema_sha256=bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5\n",
    "aql_fixed_batch_schema_sha256=a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a\n",
    "aql_completion_schema_sha256=4b7e1090eccbae41ea09ce7d5147470eb665ee295cb0f4526f5584225c86369a\n",
    "dispatch_binding_schema_sha256=0a8d45c4050b754bda7591889ee3ae5cf83ffde1d83ec9cce750f12576bac188\n",
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
    "dispatch=public-addressless-linear-fixed-batch,1-through-32-inspected-programs,1-through-8192-packets,validated-code-materialization,zero-pointer-kernarg-internal-injection,metadata-derived-COV6-geometry-and-dynamic-lds-implicit-subset-with-caller-zero-suffix,queue-pointer-and-runtime-address-fields-rejected,exact-mapped-data-set-retained-even-when-unreferenced-by-current-batch,referenced-subset-only-inspected-access-and-sealed-initialization-gates,ordinary-release-or-exact-recycle-gated-attached-or-detached-return-after-destroy\n",
    "readback=coherent-host-data-only,owned-bounded-copy-or-exact-caller-owned-destination-after-exact-acquire-observed-completion-and-signal-recycle,exact-dispatch-generation,ordinary-range-within-one-inspected-write-or-readwrite-binding-or-exact-admitted-initialized-enclosing-snapshot,no-native-address-or-mapped-borrow,no-whole-allocation-initialization-promotion\n",
    "rebinding=exact-completion-and-signal-recycle-before-detach,code-and-kernarg-released,live-rebind-retains-queue-ring-signal-event-doorbell-and-runtime,quiescent-rollover-confirms-old-native-destroy-before-new-queue-creation,exact-complete-detached-generation-cardinality-and-ordered-private-storage-identity-ledger,preflighted-device-or-host-insertion-at-exact-ordinal-and-release-gated-removal-or-replacement-while-unbound,exact-identity-kind-and-bounds-checked-in-place-initialized-coherent-overwrite-while-unbound-or-attached-and-recycled,attached-recycled-exact-shape-resubmission-advances-generation-without-code-kernarg-or-data-detach,replacement-owner-seeded-from-exact-predecessor-and-next-publication-strictly-advances-dispatch-generation-across-live-rebind-or-queue-rollover,all-mapped-data-retained-with-inspected-effects-only-for-currently-referenced-subset,new-ring-program-count-packet-count-geometry-kernarg-and-data-admitted-before-next-publication,fully-initialized-state-preserved-without-stale-current-content-digest,authoritative-model-foundation-restored-around-every-live-queue-allocation-lifecycle-mutation-and-reclaimed-before-return\n",
    "doorbell=complete-8192-byte-kfd-slice,exact-returned-offset,madv-dontfork,no-public-address-pointer-or-mmio-accessor\n",
    "lifecycle=runtime-enable,event-create,queue-create;all-completion-batches-observed-and-recycled;queue-destroy,event-destroy,immediate-payload-zero-protect-unmap,runtime-disable,doorbell-release,cwsr-queue-resource-and-completion-arena-release;debug-runtime-authority-leaves-token-before-event-and-create-lifecycle-mutation-with-no-post-handoff-restoration;published-owners-no-drop-ioctl-store-munmap-or-free;armed-unpublished-payload-guard-drop-zero-protect-unmap\n",
    "currentness=active-queue-process-reset-event-retained-descriptor-uapi-xnack-and-drm-vram-loss-operational-fence-before-publication,after-bounded-preparation,and-before-mmio;packet-atomics-run-inside-those-owner-scopes;lifecycle-ioctls-retain-full-device-topology-aperture-composite;timeout-observation-confirms-device-runtime-event-and-CWSR-structure-before-and-after-its-sequential-racy-loads\n",
    "proof=queue-and-aql-model-obligations-only,cpu-gpu-atomic-coherence-mmio-driver-firmware-refinement-contracted\n",
    "event-lifecycle=linear-private-kfd-event,no-kfd-event-page-mmap,separate-private-payload-page-cleaned-on-unpublished-install-failure,armed-unpublished-payload-cleanup-through-all-pre-create-failures-until-immediately-before-native-create-queue-call,zeroized-protected-and-unmapped-immediately-after-event-destroy-before-runtime-disable-and-independent-of-later-resource-release,payload-cleanup-failure-after-event-destroy-aborts-process-before-owner-loss,queue-destroy-before-event-destroy-before-runtime-disable-before-cwsr-free-and-full-reservation-munmap,published-owners-no-drop-ioctl-or-unmap\n",
    "cwsr-address-semantics=bo-cpu-vma-is-create-address-except-exact-24-owned-fixed-private-anonymous-control-stack-pages,prot-none-then-dontfork-then-rw,whole-span-seal-then-exact-shadow-rw-restore;headers-and-control-stack-kfd-copy-targets,wave-state-remains-read-only-bo-mapped,event-payload-disjoint-from-all-control-stack-pages;ordinary-hardware-preemption-restore-contracted\n",
    "exception-observation=crate-private-one-shot-timeout-0-through-1000ms-wait-and-terminal-timeout-direct-volatile-CWSR-reason,wait-and-payload-must-agree,unknown-reason-rejected,zero-reason-is-racy-snapshot-not-absence-proof,no-atomic-or-lossless-delivery-claim\n",
    "failure=counter-divergence-regression-currentness-and-any-possible-side-effect-runtime-event-shadow-wait-publication-completion-observation-timeout-reset-or-teardown-error-terminally-poisons;timeout-snapshot-capture-failure-reports-currentness-or-observation-instead-of-unbound-evidence;no-in-process-recovery-rollback-or-cleanup-after-terminal-observation;only-explicitly-classified-pre-side-effect-full-or-insufficient-space-retryable\n",
    "excluded=kernel-dispatch-hardware-completion-fault-or-exception-delivery-refinement,kernel-effect-correctness-beyond-inspected-metadata,full-kernel-write-coverage,kernel-numerical-correctness,device-local-update,multi-producer,foreign-kfd-process-coordination,private-cwsr-wave-record-decoding\n",
);

/// SHA-256 of [`GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1`].
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1: &str =
    "2df22ab1f0bf49e270d4dc332e490a9ce760bec04fccc0676658dd455ec4e47a";

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
}

/// Ownership returned by the exact recycled fixed-dispatch teardown path.
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

    pub const fn dispatch_generation(&self) -> u64 {
        self.dispatch_generation
    }

    pub fn data_lease_count(&self) -> usize {
        self.data.len()
    }

    /// Returns the exact owning KFD session and every executed mapped
    /// allocation without restoring stale exact-content authority.
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
    if generation == 0 {
        *terminal_poisoned = true;
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "detached dispatch generation was zero",
        ));
    }
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
        let admitted = admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            lane,
        )?;
        let AdmittedComputeLaneV1::Auxiliary(index) = admitted else {
            let mut lane = ComputeAqlQueueLaneDispatchV1 { session: self };
            return Ok(operation(&mut lane));
        };
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
            Ok(owner) => {
                let observations = owner
                    .striped_observations()
                    .expect("created striped SDMA queue set");
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
        if !source.belongs_to(self.key) || !destination.belongs_to(self.key) {
            return Err(Gfx942SdmaSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some((source, destination)),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaSubmissionFailureV1 {
                error,
                recovered: None,
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
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovered: Some(requests),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBatchSubmissionFailureV1 {
                error,
                recovered: None,
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
        if requests.iter().any(|request| {
            !request.source.belongs_to(self.key) || !request.destination.belongs_to(self.key)
        }) {
            return Err(Gfx942SdmaBatchExecutionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
                recovery: Some(Gfx942SdmaBatchExecutionRecoveryV1::Requests(requests)),
            });
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(Gfx942SdmaBatchExecutionFailureV1 {
                error,
                recovery: None,
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

    /// Detaches one exactly completed and recycled fixed batch while keeping
    /// the native queue and all queue resources live.
    pub fn detach_recycled_fixed_dispatch(
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
        if self
            .detached_dispatch_generation
            .is_none_or(|generation| generation == 0)
        {
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
            prepare_public_fixed_dispatch_resources_after_recycle(
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
        if self
            .detached_dispatch_generation
            .is_none_or(|generation| generation == 0)
        {
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
        if self.terminal_poisoned {
            return Err(NativeAqlSubmissionErrorV1::Poisoned);
        }
        let exception = self
            .exception
            .as_ref()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing queue exception gate",
            ))?;
        let owner = self
            .submission
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing submission owner",
            ))?;
        let engine = self
            .engine
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing queue engine",
            ))?;
        if engine.phase(self.key) != Some(ComputeAqlQueuePhaseV1::Active) {
            return Err(NativeAqlSubmissionErrorV1::InvalidQueue(
                "queue is not active",
            ));
        }
        let (backend, resources) = (&mut engine.backend, &mut engine.resources);
        let resource = resources
            .iter_mut()
            .find(|resource| resource.key == self.key)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                "missing queue resources",
            ))?;
        let authority =
            resource
                .authority
                .as_mut()
                .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue(
                    "released queue resources",
                ))?;
        let doorbell = self
            .doorbell
            .as_mut()
            .ok_or(NativeAqlSubmissionErrorV1::InvalidQueue("missing doorbell"))?;
        let mut native = LinuxAqlSubmissionBackendV1 {
            memory: &mut backend.session,
            ring: &mut authority.ring,
            control: &mut authority.control,
            doorbell,
            exception,
        };
        let result = owner.submit_batch(batch, &mut native);
        if let Err(error) = &result {
            let ordinary_occupancy = matches!(
                error,
                NativeAqlSubmissionErrorV1::Ring(
                    fe2o3_aql::AqlRingReservationError::Full
                        | fe2o3_aql::AqlRingReservationError::InsufficientSpace { .. }
                )
            );
            if !ordinary_occupancy {
                self.terminal_poisoned = true;
            }
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
        if self.terminal_poisoned {
            return Err(Gfx942CompletionErrorV1::Poisoned.into());
        }
        let bound = self.completion_owner.bind_batch(templates)?;
        let (packets, retention) = bound.into_parts();
        self.completion_owner.validate_bound(&retention)?;
        match self.submit_prepared_batch(packets) {
            Ok(last_packet_id) => {
                match self
                    .completion_owner
                    .mark_published(retention, last_packet_id)
                {
                    Ok(batch) => Ok(batch),
                    Err(error) => {
                        self.poison_terminal();
                        Err(error.into())
                    }
                }
            }
            Err(error) => {
                let ordinary_occupancy = matches!(
                    error,
                    NativeAqlSubmissionErrorV1::Ring(
                        fe2o3_aql::AqlRingReservationError::Full
                            | fe2o3_aql::AqlRingReservationError::InsufficientSpace { .. }
                    )
                );
                if ordinary_occupancy {
                    if let Err(cancel_error) = self.completion_owner.cancel_bound(retention) {
                        self.poison_terminal();
                        return Err(cancel_error.into());
                    }
                } else {
                    self.completion_owner.poison_owner();
                }
                Err(map_submission(error))
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
        let templates = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .bind_templates::<N>(self.key)?;
        let generation = self
            .dispatch
            .as_ref()
            .expect("dispatch owner was just bound")
            .active_generation()?;
        match self.submit_with_completions(templates) {
            Ok(completion) => Ok(wrap_published(completion, generation)),
            Err(error) => {
                let dispatch = self.dispatch.as_mut().expect("dispatch owner retained");
                if self.terminal_poisoned {
                    dispatch.poison();
                } else if dispatch.cancel_binding(generation).is_err() {
                    self.poison_terminal();
                }
                Err(error)
            }
        }
    }

    /// Polls every packet signal once and returns linear pending or completed custody.
    pub fn poll_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        match self.poll_fixed_dispatch_with_progress(batch)? {
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

    /// Destroys a queue and returns its actual mapped C3 authorities only when
    /// the bound dispatch reached exact C4 completion and signal recycle.
    ///
    /// This is crate-private prerequisite plumbing for a future authenticated
    /// copy-kernel bridge. It grants no initialized-content or read authority.
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
        if self.sdma_outstanding_buffers != 0 {
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
                    .ensure_returnable()?;
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
                let returned = dispatch.release_non_data_after_recycle(
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
        let destroyed = ComputeAqlQueueDestroyedV1 {
            queue_id: self.observation.queue_id,
            released_resources: 5 + released_sdma_resources,
        };
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

        assert!(content_descriptor_matches_bytes(descriptor, bytes));
        assert!(!content_descriptor_matches_bytes(
            descriptor,
            b"exact initialized byte"
        ));

        let mut substituted = bytes.to_vec();
        substituted[0] ^= 1;
        assert!(!content_descriptor_matches_bytes(descriptor, &substituted));
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
            "0a8d45c4050b754bda7591889ee3ae5cf83ffde1d83ec9cce750f12576bac188"
        );
        assert!(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1.contains(&format!(
            "dispatch_binding_schema_sha256={}\n",
            super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1
        )));
        assert_eq!(
            SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
            "fb01d099eedfb39a60a1763897691684b547c51610b5e62529f2a6ff0eb27f83"
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
}
