//! Bounded gfx942 SDMA copy queue.
//!
//! Packet construction and ownership transitions are checked locally. Native
//! execution remains conditional on the pinned KFD, firmware, coherency, and
//! GPU memory-system contracts.

use core::fmt;
use std::time::{Duration, Instant};

use fe2o3_kfd_uapi::{
    KFD_GFX942_SDMA_ENGINE_COUNT_V1, KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1, KfdGfx942SdmaEngineId,
    KfdGfx942SdmaXgmiEngineId, KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs,
    KfdSdmaQueueBuffers, admit_kfd_aql_queue_ring_size, admit_kfd_gfx942_create_queue_outputs,
    admit_kfd_gfx942_sdma_engine_id, admit_kfd_gfx942_sdma_xgmi_engine_mask,
    admit_kfd_queue_percentage, admit_kfd_queue_priority,
};
use fe2o3_runtime_model::QueueKeyV1;

use crate::MemorySessionError;
use crate::queue::submit::initialize_amd_aql_control;
use crate::queue_linux::{
    LinuxDoorbellSliceV1, ProcessGlobalKfdRuntimeCreationArmV1,
    arm_process_global_kfd_runtime_gate_for_creation_v1, create_queue, destroy_queue,
    permanently_poison_process_global_kfd_runtime_gate_v1,
};
use crate::queue_resources::{
    AMD_AQL_READ_DISPATCH_ID_OFFSET_V1, AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1,
};
use crate::shared_memory::{
    AqlControlResourceRoleV1, AqlQueueGttV1, AqlRingResourceRoleV1, Gfx942DeviceMemoryIdentityV1,
    Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1, Gfx942XgmiMappedDeviceMemoryV1,
    GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1, SharedGttAllocationIdentityV1,
    SharedGttAllocationV1, SharedGttMemorySessionV1, SharedGttQueueResourceAuthorityV1,
    UserptrAqlControlGttV1,
};
use crate::wait::MonotonicWaitV1;

pub(crate) mod pool_policy;
pub(crate) use pool_policy::{
    DevicePoolDispositionV1, device_pool_recycle_decision_v1, device_pool_usage_v1,
};
pub use pool_policy::{Gfx942DevicePoolLimitsV1, Gfx942DevicePoolUsageV1};

pub(crate) mod host_pool_policy;
pub use host_pool_policy::{Gfx942HostPoolLimitsV1, Gfx942HostPoolUsageV1};
pub(crate) use host_pool_policy::{
    HostPoolDispositionV1, host_pool_recycle_decision_v1, host_pool_usage_v1,
};

mod multi_queue;
use multi_queue::next_striped_owner;
pub use multi_queue::{
    GFX942_SDMA_LOGICAL_MUX_MAX_REQUESTS_PER_NATIVE_QUEUE_V2,
    GFX942_SDMA_LOGICAL_MUX_MAX_REQUESTS_V2, GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2,
    Gfx942SdmaLogicalMuxCompletedV2, Gfx942SdmaLogicalMuxNativeShardObservationV2,
    Gfx942SdmaLogicalMuxObservationV2, Gfx942SdmaLogicalMuxPlanErrorV2, Gfx942SdmaLogicalMuxPlanV2,
    Gfx942SdmaLogicalMuxPollV2, Gfx942SdmaLogicalMuxSubmissionV2, Gfx942SdmaMultiQueueCompletedV1,
    Gfx942SdmaMultiQueuePlanErrorV1, Gfx942SdmaMultiQueuePlanV1, Gfx942SdmaMultiQueuePollV1,
    Gfx942SdmaMultiQueueShardTicketsV1, Gfx942SdmaMultiQueueSubmissionV1,
    Gfx942SdmaStripedDiagnosticSpinBudgetV1, Gfx942SdmaStripedWaitCpuMeasurementStatusV1,
    Gfx942SdmaStripedWaitDiagnosticsV1,
};
#[cfg(test)]
pub(crate) use multi_queue::{
    Gfx942SdmaMultiQueueIdentityForTestV1, striped_submission_for_unwind_test,
};
pub(crate) use multi_queue::{
    Gfx942SdmaStripedTailWaitOutcomeV1, LogicalMuxSdmaSubmitFailureV2,
    MultiQueueSdmaSubmitFailureV1, combined_striped_sdma_queue_count_is_admitted,
    gfx942_sdma_logical_mux_lane_count_is_admitted_v2, striped_sdma_queue_count_is_admitted,
};

pub const GFX942_SDMA_COPY_PACKET_BYTES_V1: usize = 7 * 4;
pub const GFX942_SDMA_FENCE_PACKET_BYTES_V1: usize = 4 * 4;
pub const GFX942_SDMA_SUBMISSION_BYTES_V1: usize = 64;
pub const GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1: u32 = 0x003f_ffe0;
pub const GFX942_SDMA_RING_BYTES_V1: u32 = 4_096;
const GFX942_SDMA_RING_SLOT_COUNT_V1: usize =
    GFX942_SDMA_RING_BYTES_V1 as usize / GFX942_SDMA_SUBMISSION_BYTES_V1;
pub const GFX942_SDMA_MAX_IN_FLIGHT_V1: usize = GFX942_SDMA_RING_SLOT_COUNT_V1 - 1;
pub const GFX942_SDMA_D2H_ENGINE_INDEX_V1: u32 = 0;
pub const GFX942_SDMA_H2D_ENGINE_INDEX_V1: u32 = 1;
pub const GFX942_SDMA_MAX_STRIPED_QUEUES_V1: usize =
    (KFD_GFX942_SDMA_ENGINE_COUNT_V1 * KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1) as usize;
pub const GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_PER_ENGINE_V1: usize =
    KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1 as usize - 1;
pub const GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_V1: usize = KFD_GFX942_SDMA_ENGINE_COUNT_V1
    as usize
    * GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_PER_ENGINE_V1;
pub const GFX942_SDMA_MAX_MULTI_QUEUE_SHARDS_V1: usize = GFX942_SDMA_MAX_STRIPED_QUEUES_V1;
pub const GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1: usize =
    GFX942_SDMA_MAX_STRIPED_QUEUES_V1 * GFX942_SDMA_MAX_IN_FLIGHT_V1;

/// Frozen claim boundary for the experimental two-native logical-lane mux.
pub const GFX942_SDMA_LOGICAL_MUX_MANIFEST_V2: &str = concat!(
    "profile=fe2o3-gfx942-kfd-sdma-logical-mux-r56-v2\n",
    "lower_sdma_manifest_sha256=5abb6d5fb9dcf321d82dfadf2ffcdd310d197b5ddb42ec3d88658ab26f1451e9\n",
    "topology=exact-gfx942-ordinary-sdma-two-engines-eight-queues-per-engine,two-persistent-native-queues,engine0-then-engine1,distinct-session-owned-queue-ids\n",
    "logical-lanes=closed-counts:2,4,8,14,16,cursor-domain-is-logical-lane,request-i-lane=(cursor+i)%lane-count,native=lane%2\n",
    "bounds=requests:2..126,exactly-two-nonempty-native-shards,at-most-63-per-native-shard\n",
    "ordering=original-request-order-is-logical-cursor-lane-major,each-native-shard-is-stable-filter-of-that-order,logical-lanes-sharing-one-native-queue-gain-cross-lane-order\n",
    "publication=prepare-both-shards-and-all-outcome-storage-before-first-publication,one-copy-plus-system-snoop-fence-per-request,one-write-pointer-publication-and-one-doorbell-per-native-queue,no-heap-allocation-after-first-publication\n",
    "completion=bind-two-exact-native-tail-fences,shared-monotonic-deadline,full-original-request-ordered-status-audit,currentness-check,and-all-or-nothing-custody-retirement\n",
    "cursor=unchanged-for-rejection-preparation-any-partial-indeterminate-closing-currentness-or-live-model-retake-failure,advance-only-after-both-native-publications-closing-currentness-successful-live-model-retake-and-restored-owner-commit\n",
    "failure=lower-layer-classified-no-native-effect-before-first-publication-recovers-original-ordered-inputs,ordinary-returned-terminal-errors-retain-audit-only-custody-and-poison-session-and-process-admission,facade-caught-rust-unwind-after-entering-live-owner-memory-operation-is-conservatively-post-effect-and-the-restoring-owner-helper-resumes-only-to-the-enclosing-facade-catch-which-poisons-session-and-process-admission-then-aborts-without-typed-custody-or-resumption-past-the-public-facade,nested-retirement-suffix-unwind-after-submission-ownership-move-causes-lower-immediate-abort-without-typed-custody-or-guaranteed-owner-restoration-or-explicit-poison,no-continued-execution-after-either-unwind-class,confirmed-one-optional-indeterminate-and-untouched-custody-remains-audit-only\n",
    "excluded=typed-panic-recovery,hip-stream-independence,independent-logical-lane-progress,priority,scheduling,event,capture,per-stream-synchronization,atomic-device-snapshot,formal-refinement,hardware-correctness,performance,hip-or-hsa-parity\n",
);

/// SHA-256 of [`GFX942_SDMA_LOGICAL_MUX_MANIFEST_V2`].
pub const GFX942_SDMA_LOGICAL_MUX_MANIFEST_SHA256_V2: &str =
    "51fe738b83cf2a306b8c62edeec9f270c78e94e57dc1b24eaf83df26fdbdfde0";
/// Ring, control, and completion allocation records retained by each SDMA queue.
pub const GFX942_SDMA_SHARED_ALLOCATION_RECORDS_PER_QUEUE_V1: usize = 3;
const GFX942_SDMA_D2H_OWNER_SLOT_V1: usize = 0;
const GFX942_SDMA_H2D_OWNER_SLOT_V1: usize = 1;
const GFX942_SDMA_SINGLE_OWNER_COUNT_V1: usize = 1;
const GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SdmaWaitProfileV1 {
    Default,
    PersistentElapsedSpinFloor(Duration),
}

impl SdmaWaitProfileV1 {
    fn cursor(self, deadline: Instant) -> MonotonicWaitV1 {
        match self {
            Self::Default => MonotonicWaitV1::until(deadline),
            Self::PersistentElapsedSpinFloor(floor) => {
                MonotonicWaitV1::until_with_active_spin_floor(deadline, floor)
            }
        }
    }
}

/// Frozen claim boundary for the bounded native gfx942 SDMA implementation.
pub const GFX942_SDMA_COPY_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-sdma-copy-r1-v14\n",
    "kfd_sdma_queue_schema_sha256=f489ae5735f8230e4ee788fe1fa9e62b307301c13cf88ee70889b0f455af0b5b\n",
    "sdma_topology_capability_sha256=51236bbd70ece3ee4e14cc1a3e7e7cfbbe0960e745130e1a3943f9e39bc36a26\n",
    "rocm_systems_commit=1b648038a0ac164cf2f06f2a581ced12cf5f7378\n",
    "rocr_amd_gpu_agent_sha256=50ee3dd832dcbd572a2c58e88fefd12697d396033d2c5b959dd866c54ea2a989\n",
    "rocr_engine_policy=projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_gpu_agent.cpp:991-993,1052-1055\n",
    "rocr_sdma_registers_sha256=0287a021439e49cd3075bd88c8f9f4558f20ad16e8f473f59732aa803c62df5b\n",
    "rocr_blit_sdma_source=projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_blit_sdma.cpp\n",
    "rocr_blit_sdma_sha256=f4d0be236a034cd9ad44b9dd196f4498bcf9dedb89a7812a217b988aef1ff359\n",
    "rocr_publication_policy=projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_blit_sdma.cpp:1954-1988,1998-2023,2049-2055\n",
    "packet=copy-linear-28-bytes,count-minus-one,source-u64,destination-u64;fence-16-bytes,mtype-3,sys-1,snp-1,u32-generation;zero-pad-to-64\n",
    "bounds=copy:1..4194272,ring:4096,submission:64,ring-slots:64,in-flight:63,one-slot-always-empty,nonoverlap\n",
    "engines=generic-compatible-or-topology-exact-ordinary:2,queues-per-engine:8,h2d-index:1,d2h-index:0,targeted-queue-type:4,standalone-balanced-striped-queues:even-2..16,combined-directional-plus-balanced-striped-queues:even-2..14-with-one-directional-queue-reserved-per-engine,round-robin-per-successful-batch\n",
    "queue-identity=distinct-among-the-session-owned-primary-and-live-auxiliary-compute-queues-and-the-created-directional-or-striped-sdma-sets-only,no-foreign-process-or-other-session-global-uniqueness-claim\n",
    "memory=move-only-host-coherent-or-device-local,logical-subrange-bounded,queue-retained-while-in-flight,exact-full-host-userspace-hash-while-copy-certificate-bound-to-queue-storage-identity-pool-generation-logical-and-physical-extents-and-range,certificate-non-clone-and-private\n",
    "submission=single-producer,all-fallible-preparation-and-allocation-retains-recoverable-requests-before-mutation,standalone-striped-multi-queue-bounds:2..16-queues-and-1..1008-requests,combined-striped-multi-queue-bounds:2..14-queues-and-1..882-requests,at-most-63-per-shard,all-striped-shards-and-outcome-storage-prepared-before-first-publication,no-heap-allocation-after-first-publication,write-complete-sdma-packet-images-and-retained-records-before-one-exact-release-visible-wptr-publication-and-one-final-release-doorbell-per-batch,queue-occurrence-and-generation-tagged-ticket\n",
    "completion=host-coherent-u32-fence-value-observed-through-i64-acquire,exact-owner-queue-slot-generation-and-request-index-binding,nonblocking-whole-submission-poll-observes-every-entry-before-pending,striped-blocking-wait-keeps-the-sole-full-submission-owner-outside-the-unwind-catching-live-memory-envelope-and-prebinds-one-exact-tail-per-active-shard-and-observes-only-those-tails-before-one-shared-monotonic-deadline,all-tail-ready-or-deadline-performs-one-full-ordered-status-and-retirement-preflight-audit,tail-ready-with-pending-prefix-fails-terminally,a-private-lifetime-bound-all-ready-witness-authorizes-only-the-immediate-abort-on-unwind-ordered-custody-move-without-reobservation-or-revalidation,completed-custody-in-original-request-order,timeout-retains-the-whole-submission-and-retry-starts-a-new-native-wait-epoch,queue-progress-at-host-monotonic-instant,no-atomic-device-snapshot-or-gpu-clock-calibration\n",
    "diagnostics=opt-in-success-only-striped-tail-host-decomposition,active-queue-and-request-counts,tail-round-and-observation-counts,spin-yield-sleep-pause-counts,requested-not-actual-sleep-duration,first-and-all-tail-host-monotonic-offsets,bind-opening-currentness-tail-scan-final-audit-closing-currentness-and-retirement-host-monotonic-durations,tail-scan-linux-clock-thread-cputime-id-nanoseconds-and-rusage-thread-voluntary-and-involuntary-context-switch-deltas-with-explicit-available-unavailable-invalid-status,syscall-failure-and-invalid-or-overflowing-observations-clear-all-three-cpu-cost-values-without-an-operational-error,ordinary-wait-uses-a-compile-time-disabled-profile-without-cpu-cost-syscalls-or-counters,closed-diagnostic-spin-budget-recorded-in-every-successful-profile,no-device-timestamps-or-engine-counters,no-admission-or-completion-authority,profiled-host-overhead-is-not-unprofiled-overhead\n",
    "striped-wait-policy=ordinary-and-profiled-current-first-observation-unconditional,64-spin-pauses,16-yield-pauses,subsequent-sleep-requests-capped-at-25000ns-and-clamped-to-one-shared-monotonic-deadline,actual-scheduler-wake-latency-unbounded,no-completion-authority-from-pause-schedule\n",
    "diagnostic-striped-spin-budget-experiment=profiled-only,closed-roster:current-or-250000ns-or-500000ns-or-1000000ns-or-1500000ns-or-3000000ns,nonzero-values-are-elapsed-active-spin-floors-checked-add-and-clamped-to-the-same-deadline,attempts-counted-during-floor,then-existing-adaptive-stage-with-25000ns-sleep-ceiling,ordinary-wrapper-always-selects-current,nonprofiled-noncurrent-rejected,budget-and-timing-and-profiling-observations-have-no-completion-authority,gpu-progresses-during-actual-host-sleeps,wall-minus-thread-cpu-minus-requested-sleep-is-not-avoidable-latency,spin-changes-host-observation-wakeup-cpu-and-context-switch-behavior-without-device-duration-causality,no-unbounded-input\n",
    "striped-tail-fence-premise=each-copy-submission-ends-in-the-exact-mtype-3-system-1-snoop-1-fence,each-bound-owner-engine-index-is-exactly-queue-ordinal-modulo-two,within-one-admitted-gfx942-sdma-engine-observing-the-exact-queue-slot-generation-bound-tail-fence-completion-implies-every-preceding-copy-and-fence-occurrence-on-that-shard-is-complete-and-system-visible,firmware-ordering-and-cpu-gpu-coherence-are-external-contracts\n",
    "persistent-sdma-wait-policy=elapsed-active-spin-floor:50000ns,checked-add-and-clamp-to-deadline,attempts-counted-during-floor,exact-floor-boundary-resumes-default-adaptive-stage,first-observation-unconditional;scope=directional-persistent-single,directional-persistent-window,same-device-persistent-window;excluded=ordinary-directional,generic-striped,fused-synchronous,xgmi,persistent-compute\n",
    "cancellation=published-packets-cannot-be-retracted,typed-rejection-retains-ticket,poll-or-explicit-drain-required\n",
    "pool=queue-branded,best-fit-by-kind-size-and-alignment,leased-and-in-flight-excluded,concrete-generation-advanced-on-recycle,explicit-trim-before-teardown,certificate-cleared-on-every-attempted-valid-range-cpu-write-device-destination-request-logical-resize-pool-generation-advance-or-private-reconstruction\n",
    "dispatch-data-bridge=exact-full-extent-host-content-or-completed-h2d-only,move-only-storage-identity-and-queue-and-pool-generation-binding,no-rematerialization,demotion-advances-pool-generation\n",
    "currentness=one-operational-pre-post-envelope-per-submit-batch-or-wait-batch-or-combined-submit-through-observed-completion,authenticated-full-host-write-retains-one-pre-post-envelope-per-max-linear-chunk,persistent-ready-certificate-validation-retains-one-pre-post-envelope,internal-atomics-and-mapped-writes-only-inside-envelope\n",
    "failure=structural-preflight-before-the-first-live-shared-memory-or-currentness-operation-recovers-inputs,retryable-no-native-effect-availability-detached-compute-foreign-buffer-and-recoverable-preparation-failures-preserve-inputs-and-do-not-poison,every-terminal-error-or-caught-unwind-at-or-after-that-boundary-permanently-poisons-process-global-kfd-admission-and-requires-process-teardown-independent-of-whether-a-confirmed-queue-roster-exists,prepared-live-and-terminal-creation-custody-are-distinct,validated-xgmi-route-scope-failure-quarantines-both-participating-sessions,currentness-counter-generation-and-post-preflight-uncertainty-terminally-poison-and-retain-native-custody,every-striped-submit-poll-or-wait-process-teardown-return-invokes-one-central-terminalizer-that-poisons-both-the-local-session-and-process-global-admission,striped-terminal-failure-exposes-audit-only-confirmed-and-at-most-one-indeterminate-and-untouched-observations-without-drain-or-resubmit-authority,striped-wait-timeout-retains-exact-pending-custody-and-does-not-invoke-the-terminalizer,striped-wait-panic-before-the-abort-only-retirement-suffix-preserves-the-exact-sealed-plan-shards-and-ordered-completion-roster-in-terminal-custody-and-invokes-the-same-terminalizer,striped-cursor-commits-only-after-complete-publication-and-closing-currentness\n",
    "teardown=combined-striped-before-directional-before-compute,standalone-sdma-before-compute,then-release-ring-control-completions-and-pooled-buffers-explicitly,terminal-creation-custody-has-no-in-process-cleanup-authority\n",
    "proof=abstract-pool-generation-retention-and-cross-device-coordinate-theorems-only,r46-model-is-not-an-executable-rust-refinement,host-thread-cpu-and-context-switch-measurements-are-not-proof\n",
    "contracted=ioctl-truth,doorbell-mapping,cpu-gpu-coherence,sha256-collision-resistance,userspace-certificate-is-not-kernel-attestation-or-loaded-kernel-proof,kernel-firmware-packet-consumption,completion,event-driven-completion,gpu-clock-calibration,progress,liveness\n",
    "measured=prior-host-diagnostic-motivates-closed-spin-budget-roster-only,no-r56-gpu-result,no-parity-or-striped-tail-wait-speedup-measured-for-this-revision,no-claim-that-spin-closes-the-observed-hip-gap\n",
);

/// SHA-256 of [`GFX942_SDMA_COPY_MANIFEST_V1`].
pub const GFX942_SDMA_COPY_MANIFEST_SHA256_V1: &str =
    "6ae8a129c33783ba8f0ff95c66f925221a8af127972e0f5093aa676fb311a59b";

const SDMA_OP_COPY: u32 = 1;
const SDMA_OP_FENCE: u32 = 5;
const SDMA_SUBOP_COPY_LINEAR: u32 = 0;
const SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1: u32 = (1 << 22) | (1 << 20) | (3 << 16) | SDMA_OP_FENCE;

type SdmaRingAuthorityV1 = SharedGttQueueResourceAuthorityV1<
    AqlRingResourceRoleV1,
    AqlQueueGttV1,
    GttGpuAccessibleMutableV1,
>;
type SdmaControlAuthorityV1 = SharedGttQueueResourceAuthorityV1<
    AqlControlResourceRoleV1,
    UserptrAqlControlGttV1,
    GttGpuAccessibleMutableV1,
>;
type MappedHostBufferV1 =
    SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaPacketErrorV1 {
    ZeroAddress,
    EmptyCopy,
    CopyTooLarge,
    AddressOverflow,
    ZeroCompletionValue,
}

impl fmt::Display for Gfx942SdmaPacketErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942SdmaPacketErrorV1 {}

/// One gfx942 linear-copy packet followed by a system-scope completion fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaCopySubmissionV1 {
    bytes: [u8; GFX942_SDMA_SUBMISSION_BYTES_V1],
    copy_bytes: u32,
}

impl Gfx942SdmaCopySubmissionV1 {
    pub fn new(
        source: u64,
        destination: u64,
        copy_bytes: u32,
        completion_address: u64,
        completion_value: u32,
    ) -> Result<Self, Gfx942SdmaPacketErrorV1> {
        if source == 0 || destination == 0 || completion_address == 0 {
            return Err(Gfx942SdmaPacketErrorV1::ZeroAddress);
        }
        if copy_bytes == 0 {
            return Err(Gfx942SdmaPacketErrorV1::EmptyCopy);
        }
        if copy_bytes > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 {
            return Err(Gfx942SdmaPacketErrorV1::CopyTooLarge);
        }
        if completion_value == 0 {
            return Err(Gfx942SdmaPacketErrorV1::ZeroCompletionValue);
        }
        source
            .checked_add(u64::from(copy_bytes) - 1)
            .ok_or(Gfx942SdmaPacketErrorV1::AddressOverflow)?;
        destination
            .checked_add(u64::from(copy_bytes) - 1)
            .ok_or(Gfx942SdmaPacketErrorV1::AddressOverflow)?;
        completion_address
            .checked_add(3)
            .ok_or(Gfx942SdmaPacketErrorV1::AddressOverflow)?;

        let copy_words = [
            SDMA_OP_COPY | (SDMA_SUBOP_COPY_LINEAR << 8),
            copy_bytes - 1,
            0,
            source as u32,
            (source >> 32) as u32,
            destination as u32,
            (destination >> 32) as u32,
        ];
        let fence_words = [
            SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1,
            completion_address as u32,
            (completion_address >> 32) as u32,
            completion_value,
        ];
        let mut bytes = [0_u8; GFX942_SDMA_SUBMISSION_BYTES_V1];
        for (index, word) in copy_words.into_iter().chain(fence_words).enumerate() {
            let offset = index * 4;
            bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        Ok(Self { bytes, copy_bytes })
    }

    pub const fn bytes(&self) -> &[u8; GFX942_SDMA_SUBMISSION_BYTES_V1] {
        &self.bytes
    }

    const fn fence_header(&self) -> u32 {
        u32::from_le_bytes([
            self.bytes[28],
            self.bytes[29],
            self.bytes[30],
            self.bytes[31],
        ])
    }

    pub const fn copy_bytes(self) -> u32 {
        self.copy_bytes
    }
}

#[derive(Debug)]
pub enum Gfx942SdmaErrorV1 {
    Memory(MemorySessionError),
    Packet(Gfx942SdmaPacketErrorV1),
    Contract(&'static str),
    QueueCreationIndeterminate,
    QueueDestroyIndeterminate,
    Doorbell(String),
    QueueFull,
    Pending,
    Timeout,
    PublishedCancellationUnsupported,
}

impl fmt::Display for Gfx942SdmaErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942SdmaErrorV1 {}

impl From<MemorySessionError> for Gfx942SdmaErrorV1 {
    fn from(value: MemorySessionError) -> Self {
        Self::Memory(value)
    }
}

impl From<Gfx942SdmaPacketErrorV1> for Gfx942SdmaErrorV1 {
    fn from(value: Gfx942SdmaPacketErrorV1) -> Self {
        Self::Packet(value)
    }
}

fn map_multi_queue_plan_error(error: Gfx942SdmaMultiQueuePlanErrorV1) -> Gfx942SdmaErrorV1 {
    match error {
        Gfx942SdmaMultiQueuePlanErrorV1::Allocation => {
            Gfx942SdmaErrorV1::Contract("multi-queue SDMA plan allocation")
        }
        Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { .. }
        | Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { .. }
        | Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { .. }
        | Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor { .. } => {
            Gfx942SdmaErrorV1::Contract("invalid multi-queue SDMA plan")
        }
    }
}

fn preallocate_doorbell_failure_message() -> Result<String, Gfx942SdmaErrorV1> {
    const MESSAGE: &str = "SDMA doorbell operation failed";
    let mut message = String::new();
    message
        .try_reserve_exact(MESSAGE.len())
        .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA doorbell error allocation"))?;
    message.push_str(MESSAGE);
    Ok(message)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaBufferKindV1 {
    HostVisibleCoherent,
    DeviceLocal,
}

pub(crate) enum Gfx942SdmaBufferStorageV1 {
    Host(MappedHostBufferV1),
    Device(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Gfx942SdmaBufferStorageIdentityV1 {
    Host(SharedGttAllocationIdentityV1),
    Device(Gfx942DeviceMemoryIdentityV1),
}

#[derive(Debug, Eq, PartialEq)]
struct Gfx942SdmaHostContentCertificateV1 {
    owner: QueueKeyV1,
    storage_identity: Gfx942SdmaBufferStorageIdentityV1,
    pool_generation: u64,
    logical_bytes: u64,
    physical_bytes: u64,
    byte_offset: u64,
    byte_len: u64,
    sha256: [u8; 32],
}

/// Move-only allocation accepted by the bounded SDMA queue.
#[must_use = "the buffer owns a mapped allocation and requires explicit release"]
pub struct Gfx942SdmaBufferV1 {
    storage: Gfx942SdmaBufferStorageV1,
    owner: QueueKeyV1,
    pool_generation: u64,
    logical_bytes: u64,
    host_content_certificate: Option<Box<Gfx942SdmaHostContentCertificateV1>>,
}

impl fmt::Debug for Gfx942SdmaBufferV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SdmaBufferV1")
            .field("kind", &self.kind())
            .field("requested_bytes", &self.requested_bytes())
            .field("pool_generation", &self.pool_generation())
            .finish_non_exhaustive()
    }
}

impl Gfx942SdmaBufferV1 {
    pub const fn kind(&self) -> Gfx942SdmaBufferKindV1 {
        match self.storage {
            Gfx942SdmaBufferStorageV1::Host(_) => Gfx942SdmaBufferKindV1::HostVisibleCoherent,
            Gfx942SdmaBufferStorageV1::Device(_) => Gfx942SdmaBufferKindV1::DeviceLocal,
        }
    }

    pub const fn requested_bytes(&self) -> u64 {
        self.logical_bytes
    }

    pub const fn pool_generation(&self) -> u64 {
        self.pool_generation
    }

    pub(crate) fn belongs_to(&self, owner: QueueKeyV1) -> bool {
        exact_queue_owner(self.owner, owner)
    }

    pub(crate) fn advance_pool_generation(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        self.host_content_certificate = None;
        self.pool_generation = next_pool_generation(self.pool_generation)?;
        Ok(())
    }

    pub(crate) const fn physical_bytes(&self) -> u64 {
        match &self.storage {
            Gfx942SdmaBufferStorageV1::Host(token) => token.layout().requested_bytes() as u64,
            Gfx942SdmaBufferStorageV1::Device(lease) => lease.layout().requested_bytes(),
        }
    }

    pub(crate) const fn physical_alignment(&self) -> u64 {
        match &self.storage {
            Gfx942SdmaBufferStorageV1::Host(_) => crate::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
            Gfx942SdmaBufferStorageV1::Device(lease) => lease.layout().alignment(),
        }
    }

    pub(crate) fn set_logical_bytes(&mut self, logical_bytes: u64) {
        debug_assert!(logical_bytes != 0 && logical_bytes <= self.physical_bytes());
        self.host_content_certificate = None;
        self.logical_bytes = logical_bytes;
    }

    fn clear_host_content_certificate(&mut self) {
        self.host_content_certificate = None;
    }

    fn certify_full_host_content(&mut self, sha256: [u8; 32]) {
        debug_assert_eq!(self.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
        debug_assert_eq!(self.logical_bytes, self.physical_bytes());
        self.host_content_certificate = Some(Box::new(Gfx942SdmaHostContentCertificateV1 {
            owner: self.owner,
            storage_identity: self.storage_identity(),
            pool_generation: self.pool_generation,
            logical_bytes: self.logical_bytes,
            physical_bytes: self.physical_bytes(),
            byte_offset: 0,
            byte_len: self.physical_bytes(),
            sha256,
        }));
    }

    fn replace_full_host_content_certificate(
        &mut self,
        write: impl FnOnce(&mut Gfx942SdmaBufferStorageV1) -> Result<[u8; 32], Gfx942SdmaErrorV1>,
    ) -> Result<[u8; 32], Gfx942SdmaErrorV1> {
        self.clear_host_content_certificate();
        let digest = write(&mut self.storage)?;
        self.certify_full_host_content(digest);
        Ok(digest)
    }

    pub(crate) fn certified_full_host_content_sha256(&self, byte_len: u64) -> Option<[u8; 32]> {
        let certificate = self.host_content_certificate.as_ref()?;
        (self.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
            && certificate.owner == self.owner
            && certificate.storage_identity == self.storage_identity()
            && certificate.pool_generation == self.pool_generation
            && certificate.logical_bytes == self.logical_bytes
            && certificate.physical_bytes == self.physical_bytes()
            && certificate.byte_offset == 0
            && certificate.byte_len == byte_len
            && byte_len == self.logical_bytes
            && byte_len == self.physical_bytes())
        .then_some(certificate.sha256)
    }

    pub(crate) const fn storage_identity(&self) -> Gfx942SdmaBufferStorageIdentityV1 {
        match &self.storage {
            Gfx942SdmaBufferStorageV1::Host(token) => {
                Gfx942SdmaBufferStorageIdentityV1::Host(token.storage_identity())
            }
            Gfx942SdmaBufferStorageV1::Device(lease) => {
                Gfx942SdmaBufferStorageIdentityV1::Device(lease.storage_identity())
            }
        }
    }

    pub(crate) fn into_bridge_parts(self) -> (Gfx942SdmaBufferStorageV1, QueueKeyV1, u64, u64) {
        (
            self.storage,
            self.owner,
            self.pool_generation,
            self.logical_bytes,
        )
    }

    pub(crate) fn from_bridge_parts(
        storage: Gfx942SdmaBufferStorageV1,
        owner: QueueKeyV1,
        pool_generation: u64,
        logical_bytes: u64,
    ) -> Self {
        Self {
            storage,
            owner,
            pool_generation,
            logical_bytes,
            host_content_certificate: None,
        }
    }

    pub(crate) fn checked_gpu_subrange(
        &self,
        memory: &SharedGttMemorySessionV1,
        offset: u64,
        byte_len: u64,
    ) -> Result<u64, Gfx942SdmaErrorV1> {
        if byte_len == 0
            || offset
                .checked_add(byte_len)
                .is_none_or(|end| end > self.logical_bytes)
        {
            return Err(Gfx942SdmaErrorV1::Contract("logical buffer copy range"));
        }
        match &self.storage {
            Gfx942SdmaBufferStorageV1::Host(token) => memory
                .mapped_resource_facts(token)?
                .checked_gpu_subrange(offset, byte_len, 1)
                .ok_or(Gfx942SdmaErrorV1::Contract("host buffer copy range")),
            Gfx942SdmaBufferStorageV1::Device(lease) => memory
                .mapped_gfx942_device_memory_facts(lease)?
                .checked_gpu_subrange(offset, byte_len, 1)
                .ok_or(Gfx942SdmaErrorV1::Contract("device buffer copy range")),
        }
    }

    /// Validates the exact mapped device backing against its physical extent.
    /// Copy paths continue to use `checked_gpu_subrange` and remain bounded by
    /// the logical extent.
    pub(crate) fn validate_physical_device_mapping(
        &self,
        memory: &SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Gfx942SdmaBufferStorageV1::Device(lease) = &self.storage else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "physical device mapping requires device-local storage",
            ));
        };
        memory
            .mapped_gfx942_device_memory_facts(lease)?
            .checked_gpu_subrange(0, self.physical_bytes(), 1)
            .map(|_| ())
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "physical device mapping extent",
            ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaCopyTicketV1 {
    owner: QueueKeyV1,
    queue_id: u32,
    slot: u16,
    generation: u32,
}

#[must_use = "the request owns both mapped buffers until submission"]
pub struct Gfx942SdmaCopyRequestV1 {
    pub(crate) source: Gfx942SdmaBufferV1,
    pub(crate) source_offset: u64,
    pub(crate) destination: Gfx942SdmaBufferV1,
    pub(crate) destination_offset: u64,
    pub(crate) copy_bytes: u32,
}

/// One request that was not handed to a native queue after partial publication.
#[must_use = "the unpublished request still owns both mapped buffers"]
pub struct Gfx942SdmaUnpublishedCopyRequestV1 {
    request_index: u16,
    request: Gfx942SdmaCopyRequestV1,
}

impl Gfx942SdmaUnpublishedCopyRequestV1 {
    pub const fn request_index(&self) -> usize {
        self.request_index as usize
    }

    pub fn into_request(self) -> Gfx942SdmaCopyRequestV1 {
        self.request
    }
}

/// One move-only XGMI copy request prepared for a directional route.
#[must_use = "the request owns both peer-mapped allocations until submission"]
pub struct Gfx942XgmiSdmaCopyRequestV1 {
    source: Gfx942XgmiMappedDeviceMemoryV1,
    source_offset: u64,
    destination: Gfx942XgmiMappedDeviceMemoryV1,
    destination_offset: u64,
    copy_bytes: u32,
}

impl Gfx942XgmiSdmaCopyRequestV1 {
    pub fn new(
        source: Gfx942XgmiMappedDeviceMemoryV1,
        source_offset: u64,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Self {
        Self {
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
        }
    }

    pub fn into_mappings(
        self,
    ) -> (
        Gfx942XgmiMappedDeviceMemoryV1,
        Gfx942XgmiMappedDeviceMemoryV1,
    ) {
        (self.source, self.destination)
    }
}

impl Gfx942SdmaCopyRequestV1 {
    pub fn new(
        source: Gfx942SdmaBufferV1,
        source_offset: u64,
        mut destination: Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Self {
        destination.clear_host_content_certificate();
        Self {
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
        }
    }

    pub fn into_buffers(self) -> (Gfx942SdmaBufferV1, Gfx942SdmaBufferV1) {
        (self.source, self.destination)
    }
}

#[must_use = "completed buffers retain mapped allocation authority"]
pub struct Gfx942SdmaCompletedCopyV1 {
    pub source: Gfx942SdmaBufferV1,
    pub destination: Gfx942SdmaBufferV1,
    pub(crate) copy_bytes: u32,
    pub(crate) source_offset: u64,
    pub(crate) destination_offset: u64,
}

impl Gfx942SdmaCompletedCopyV1 {
    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub fn into_buffers(self) -> (Gfx942SdmaBufferV1, Gfx942SdmaBufferV1) {
        (self.source, self.destination)
    }
}

// Keeping the completed authority inline avoids a new allocation after the
// device-visible operation has completed.
#[allow(clippy::large_enum_variant)]
pub enum Gfx942SdmaCopyPollV1 {
    Pending,
    Completed(Gfx942SdmaCompletedCopyV1),
}

/// Non-consuming host observation of one submitted ticket roster.
///
/// `host_observed_at` is a process-local monotonic timestamp. It is neither a
/// GPU timestamp nor calibrated against a device clock.
#[derive(Clone, Copy, Debug)]
pub struct Gfx942SdmaQueueProgressObservationV1 {
    queue_id: u32,
    submitted_count: u16,
    completed_count: u16,
    queue_write_bytes: u64,
    queue_read_bytes: u64,
    host_observed_at: Instant,
}

impl Gfx942SdmaQueueProgressObservationV1 {
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }

    pub const fn submitted_count(self) -> u16 {
        self.submitted_count
    }

    pub const fn completed_count(self) -> u16 {
        self.completed_count
    }

    pub const fn pending_count(self) -> u16 {
        self.submitted_count - self.completed_count
    }

    pub const fn queue_write_bytes(self) -> u64 {
        self.queue_write_bytes
    }

    pub const fn queue_read_bytes(self) -> u64 {
        self.queue_read_bytes
    }

    pub const fn host_observed_at(self) -> Instant {
        self.host_observed_at
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaQueueObservationV1 {
    pub queue_id: u32,
    pub ring_bytes: u32,
    pub maximum_in_flight: u16,
    /// KFD engine index for a targeted queue; `None` for a generic queue.
    pub engine_index: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DirectionalSdmaQueueObservationV1 {
    pub host_to_device: Gfx942SdmaQueueObservationV1,
    pub device_to_host: Gfx942SdmaQueueObservationV1,
    pub admitted_engine_count: u32,
    pub admitted_queues_per_engine: u32,
}

/// Exact ordinary-engine capacity retained after creating directional and striped queues.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942CombinedSdmaCapacityV1 {
    directional: Gfx942DirectionalSdmaQueueObservationV1,
    striped: Vec<Gfx942SdmaQueueObservationV1>,
    maximum_striped_queue_count: u32,
}

impl Gfx942CombinedSdmaCapacityV1 {
    pub const fn directional(&self) -> Gfx942DirectionalSdmaQueueObservationV1 {
        self.directional
    }

    pub fn striped(&self) -> &[Gfx942SdmaQueueObservationV1] {
        &self.striped
    }

    pub const fn striped_queue_count(&self) -> usize {
        self.striped.len()
    }

    pub const fn maximum_striped_queue_count(&self) -> u32 {
        self.maximum_striped_queue_count
    }

    pub const fn admitted_engine_count(&self) -> u32 {
        self.directional.admitted_engine_count
    }

    pub const fn admitted_queues_per_engine(&self) -> u32 {
        self.directional.admitted_queues_per_engine
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx942SdmaMemoryPoolObservationV1 {
    pub checked_out_buffers: usize,
    pub retained_free_buffers: usize,
    pub retained_free_bytes: u64,
    pub reuse_count: u64,
}

struct SdmaCopyRecordV1 {
    directional_persistent: bool,
    generation: u32,
    completion_value: u32,
    fence_header: u32,
    completion_observed: bool,
    source: Gfx942SdmaBufferV1,
    destination: Gfx942SdmaBufferV1,
    copy_bytes: u32,
    source_offset: u64,
    destination_offset: u64,
}

struct XgmiSdmaCopyRecordV1 {
    generation: u32,
    completion_value: u32,
    source: Gfx942XgmiMappedDeviceMemoryV1,
    destination: Gfx942XgmiMappedDeviceMemoryV1,
    copy_bytes: u32,
}

#[derive(Clone, Copy)]
struct PersistentSdmaWindowSlotV1 {
    anchor_slot: usize,
    generation: u32,
    completion_value: u32,
}

struct PersistentSdmaWindowRecordV1 {
    request: Gfx942SdmaCopyRequestV1,
    packet_count: usize,
}

pub(crate) struct RetainedDirectionalSdmaObservationV1<'a> {
    pub(crate) identity: [u8; 32],
    pub(crate) source: &'a Gfx942SdmaBufferV1,
    pub(crate) destination: &'a Gfx942SdmaBufferV1,
    pub(crate) source_offset: u64,
    pub(crate) destination_offset: u64,
    pub(crate) copy_bytes: u32,
}

pub struct Gfx942XgmiCompletedCopyV1 {
    pub source: Gfx942XgmiMappedDeviceMemoryV1,
    pub destination: Gfx942XgmiMappedDeviceMemoryV1,
    copy_bytes: u32,
}

#[allow(clippy::large_enum_variant)]
pub enum Gfx942XgmiCopyPollV1 {
    Pending(Gfx942SdmaCopyTicketV1),
    Completed(Gfx942XgmiCompletedCopyV1),
}

impl Gfx942XgmiCompletedCopyV1 {
    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub fn into_mappings(
        self,
    ) -> (
        Gfx942XgmiMappedDeviceMemoryV1,
        Gfx942XgmiMappedDeviceMemoryV1,
    ) {
        (self.source, self.destination)
    }
}

#[derive(Clone, Copy)]
struct PreparedSdmaCopyV1 {
    packet: Gfx942SdmaCopySubmissionV1,
    slot: usize,
    generation: u32,
    completion_value: u32,
}

#[derive(Clone, Copy)]
struct PreparedXgmiSdmaCopyV1 {
    packet: Gfx942SdmaCopySubmissionV1,
    slot: usize,
    generation: u32,
    completion_value: u32,
}

pub(crate) struct PreparedSdmaBatchV1 {
    queue_id: u32,
    write: u64,
    write_end: u64,
    copies: Vec<PreparedSdmaCopyV1>,
    tickets: Vec<Gfx942SdmaCopyTicketV1>,
    requests: Vec<Gfx942SdmaCopyRequestV1>,
    doorbell_failure: String,
}

/// Stack-sized preparation custody for latency-sensitive single-copy paths.
pub(crate) struct PreparedSingleSdmaV1 {
    directional_persistent: bool,
    queue_id: u32,
    write: u64,
    write_end: u64,
    copy: PreparedSdmaCopyV1,
    ticket: Gfx942SdmaCopyTicketV1,
    request: Gfx942SdmaCopyRequestV1,
}

/// One persistent host/device owner pair prepared as a bounded packet window.
pub(crate) struct PreparedPersistentSdmaWindowV1 {
    queue_id: u32,
    write: u64,
    write_end: u64,
    copies: Vec<PreparedSdmaCopyV1>,
    tickets: Vec<Gfx942SdmaCopyTicketV1>,
    request: Gfx942SdmaCopyRequestV1,
    doorbell_failure: String,
}

impl PreparedPersistentSdmaWindowV1 {
    pub(crate) fn tickets(&self) -> &[Gfx942SdmaCopyTicketV1] {
        &self.tickets
    }

    pub(crate) fn into_request(self) -> Gfx942SdmaCopyRequestV1 {
        self.request
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum PreparedPersistentSdmaWindowPublicationFailureV1 {
    Recoverable {
        error: Gfx942SdmaErrorV1,
        prepared: PreparedPersistentSdmaWindowV1,
    },
    Retained {
        error: Gfx942SdmaErrorV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
}

pub(crate) struct CompletedPersistentSdmaWindowV1 {
    pub(crate) request: Gfx942SdmaCopyRequestV1,
    pub(crate) packet_count: usize,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum PersistentSdmaWindowPollV1 {
    Pending,
    Completed(CompletedPersistentSdmaWindowV1),
}

impl PreparedSingleSdmaV1 {
    pub(crate) const fn ticket(&self) -> Gfx942SdmaCopyTicketV1 {
        self.ticket
    }

    pub(crate) fn into_request(self) -> Gfx942SdmaCopyRequestV1 {
        self.request
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum PreparedSingleSdmaPublicationFailureV1 {
    Recoverable {
        error: Gfx942SdmaErrorV1,
        prepared: PreparedSingleSdmaV1,
    },
    Retained {
        error: Gfx942SdmaErrorV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
}

/// Result of waiting after publication while the caller retains one admitted
/// operational-currentness scope. The completed record is removed only after
/// the final currentness observation succeeds.
#[allow(clippy::large_enum_variant)]
pub(crate) enum SingleSdmaWaitInCurrentScopeV1 {
    Completed(Gfx942SdmaCompletedCopyV1),
    Timeout,
    QueueRetained(Gfx942SdmaErrorV1),
    FinalCurrentnessLost(Gfx942SdmaErrorV1),
}

fn close_single_sdma_wait_failure_currentness(
    memory: &mut SharedGttMemorySessionV1,
    error: Gfx942SdmaErrorV1,
) -> SingleSdmaWaitInCurrentScopeV1 {
    match memory.check_queue_operational_currentness() {
        Ok(()) => SingleSdmaWaitInCurrentScopeV1::QueueRetained(error),
        Err(error) => SingleSdmaWaitInCurrentScopeV1::FinalCurrentnessLost(error.into()),
    }
}

impl PreparedSdmaBatchV1 {
    pub(crate) fn exact_single_ticket(&self) -> Option<Gfx942SdmaCopyTicketV1> {
        let [ticket] = self.tickets.as_slice() else {
            return None;
        };
        Some(*ticket)
    }

    pub(crate) fn into_requests(self) -> Vec<Gfx942SdmaCopyRequestV1> {
        self.requests
    }
}

pub(crate) enum PreparedSdmaPublicationFailureV1<
    P = PreparedSdmaBatchV1,
    T = Gfx942SdmaCopyTicketV1,
> {
    Recoverable {
        error: Gfx942SdmaErrorV1,
        prepared: P,
    },
    Retained {
        error: Gfx942SdmaErrorV1,
        tickets: Vec<T>,
    },
}

struct PreparedXgmiSdmaBatchV1 {
    queue_id: u32,
    write: u64,
    write_end: u64,
    copies: Vec<PreparedXgmiSdmaCopyV1>,
    tickets: Vec<Gfx942SdmaCopyTicketV1>,
    requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    doorbell_failure: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SdmaBatchPublicationPlanV1 {
    write: u64,
    write_end: u64,
    packet_count: usize,
}

fn admit_sdma_batch_publication_plan(
    write: u64,
    write_end: u64,
    packet_count: usize,
) -> Result<SdmaBatchPublicationPlanV1, Gfx942SdmaErrorV1> {
    validate_sdma_write_counter_alignment(write)?;
    let expected_end = write
        .checked_add(submission_batch_bytes(packet_count)?)
        .ok_or(Gfx942SdmaErrorV1::Contract(
            "SDMA batch publication overflow",
        ))?;
    if write_end != expected_end {
        return Err(Gfx942SdmaErrorV1::Contract("SDMA batch publication extent"));
    }
    Ok(SdmaBatchPublicationPlanV1 {
        write,
        write_end,
        packet_count,
    })
}

pub(crate) struct Gfx942SdmaQueueOwnerV1 {
    owner: QueueKeyV1,
    queue_id: u32,
    engine_index: Option<u32>,
    ring: Option<SdmaRingAuthorityV1>,
    control: Option<SdmaControlAuthorityV1>,
    completions: Option<MappedHostBufferV1>,
    doorbell: Option<LinuxDoorbellSliceV1>,
    records: Vec<Option<SdmaCopyRecordV1>>,
    xgmi_records: Vec<Option<XgmiSdmaCopyRecordV1>>,
    persistent_window_slots: Vec<Option<PersistentSdmaWindowSlotV1>>,
    persistent_window_records: Vec<Option<PersistentSdmaWindowRecordV1>>,
    uncertain_xgmi_ticket: Option<Gfx942SdmaCopyTicketV1>,
    generations: [u32; GFX942_SDMA_RING_SLOT_COUNT_V1],
    destroyed: bool,
    poisoned: bool,
}

pub(crate) struct PreparedGfx942SdmaQueueV1 {
    owner: QueueKeyV1,
    engine_index: Option<u32>,
    ring: SdmaRingAuthorityV1,
    control: SdmaControlAuthorityV1,
    completions: MappedHostBufferV1,
    records: Vec<Option<SdmaCopyRecordV1>>,
    xgmi_records: Vec<Option<XgmiSdmaCopyRecordV1>>,
    persistent_window_slots: Vec<Option<PersistentSdmaWindowSlotV1>>,
    persistent_window_records: Vec<Option<PersistentSdmaWindowRecordV1>>,
}

struct PreparedGfx942SdmaQueueHostResourcesV1 {
    records: Vec<Option<SdmaCopyRecordV1>>,
    xgmi_records: Vec<Option<XgmiSdmaCopyRecordV1>>,
    persistent_window_slots: Vec<Option<PersistentSdmaWindowSlotV1>>,
    persistent_window_records: Vec<Option<PersistentSdmaWindowRecordV1>>,
    doorbell_failure: String,
}

impl PreparedGfx942SdmaQueueV1 {
    fn into_live(self, queue_id: u32, doorbell: LinuxDoorbellSliceV1) -> Gfx942SdmaQueueOwnerV1 {
        Gfx942SdmaQueueOwnerV1 {
            owner: self.owner,
            queue_id,
            engine_index: self.engine_index,
            ring: Some(self.ring),
            control: Some(self.control),
            completions: Some(self.completions),
            doorbell: Some(doorbell),
            records: self.records,
            xgmi_records: self.xgmi_records,
            persistent_window_slots: self.persistent_window_slots,
            persistent_window_records: self.persistent_window_records,
            uncertain_xgmi_ticket: None,
            generations: [0; GFX942_SDMA_RING_SLOT_COUNT_V1],
            destroyed: false,
            poisoned: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Gfx942SdmaQueueCreationNativeObservationV1 {
    UntrustedIoctlOutputs { queue_id: u32, doorbell_offset: u64 },
    ValidatedQueueId(u32),
}

// Terminal native custody must remain inline: adding indirection here would
// allocate after live memory or CREATE_QUEUE side effects.
#[allow(clippy::large_enum_variant)]
pub(crate) enum TerminalGfx942SdmaQueueCreationV1 {
    /// A live shared-memory operation may have consumed or quarantined native
    /// authority. No cleanup claim is made before process termination.
    OpaqueAfterFirstMemoryOperation,
    QueueAttempt {
        prepared: PreparedGfx942SdmaQueueV1,
        native: Gfx942SdmaQueueCreationNativeObservationV1,
        doorbell: Option<LinuxDoorbellSliceV1>,
    },
}

impl TerminalGfx942SdmaQueueCreationV1 {
    fn retained_resource_count(&self) -> usize {
        match self {
            Self::OpaqueAfterFirstMemoryOperation => 0,
            Self::QueueAttempt {
                prepared,
                native,
                doorbell,
            } => {
                let _ = (prepared, native, doorbell);
                3
            }
        }
    }
}

// The terminal variant carries exact move-only custody without a fallible box.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Gfx942SdmaQueueOwnerCreationFailureV1 {
    RetryableBeforeCreate(Gfx942SdmaErrorV1),
    TerminalAfterMemoryOperation {
        error: Gfx942SdmaErrorV1,
        retained: TerminalGfx942SdmaQueueCreationV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Gfx942SdmaQueueSetCreationDispositionV1 {
    Retryable,
    Terminal,
}

pub(crate) struct Gfx942SdmaQueueSetCreationFailureV1 {
    error: Gfx942SdmaErrorV1,
    disposition: Gfx942SdmaQueueSetCreationDispositionV1,
    retained: Option<Gfx942SdmaQueueSetV1>,
}

impl Gfx942SdmaQueueSetCreationFailureV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942SdmaErrorV1,
        Gfx942SdmaQueueSetCreationDispositionV1,
        Option<Gfx942SdmaQueueSetV1>,
    ) {
        (self.error, self.disposition, self.retained)
    }
}

impl Gfx942SdmaQueueOwnerCreationFailureV1 {
    fn retryable(error: Gfx942SdmaErrorV1) -> Self {
        Self::RetryableBeforeCreate(error)
    }

    fn terminal(error: Gfx942SdmaErrorV1, retained: TerminalGfx942SdmaQueueCreationV1) -> Self {
        permanently_poison_process_global_kfd_runtime_gate_v1();
        Self::TerminalAfterMemoryOperation { error, retained }
    }
}

#[allow(clippy::result_large_err)]
fn prepare_sdma_queue_host_resources()
-> Result<PreparedGfx942SdmaQueueHostResourcesV1, Gfx942SdmaQueueOwnerCreationFailureV1> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
        .map_err(|_| {
            Gfx942SdmaQueueOwnerCreationFailureV1::retryable(Gfx942SdmaErrorV1::Contract(
                "SDMA record roster allocation",
            ))
        })?;
    records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    let mut xgmi_records = Vec::new();
    xgmi_records
        .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
        .map_err(|_| {
            Gfx942SdmaQueueOwnerCreationFailureV1::retryable(Gfx942SdmaErrorV1::Contract(
                "XGMI SDMA record roster allocation",
            ))
        })?;
    xgmi_records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    let mut persistent_window_slots = Vec::new();
    persistent_window_slots
        .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
        .map_err(|_| {
            Gfx942SdmaQueueOwnerCreationFailureV1::retryable(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window slot roster",
            ))
        })?;
    persistent_window_slots.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    let mut persistent_window_records = Vec::new();
    persistent_window_records
        .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
        .map_err(|_| {
            Gfx942SdmaQueueOwnerCreationFailureV1::retryable(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window owner roster",
            ))
        })?;
    persistent_window_records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
    let doorbell_failure = preallocate_doorbell_failure_message()
        .map_err(Gfx942SdmaQueueOwnerCreationFailureV1::retryable)?;
    Ok(PreparedGfx942SdmaQueueHostResourcesV1 {
        records,
        xgmi_records,
        persistent_window_slots,
        persistent_window_records,
        doorbell_failure,
    })
}

#[allow(clippy::result_large_err)]
fn prepare_sdma_queue_host_resource_roster(
    queue_count: usize,
) -> Result<Vec<PreparedGfx942SdmaQueueHostResourcesV1>, Gfx942SdmaQueueOwnerCreationFailureV1> {
    let mut roster = Vec::new();
    roster.try_reserve_exact(queue_count).map_err(|_| {
        Gfx942SdmaQueueOwnerCreationFailureV1::retryable(Gfx942SdmaErrorV1::Contract(
            "SDMA prepared host-resource roster allocation",
        ))
    })?;
    for _ in 0..queue_count {
        roster.push(prepare_sdma_queue_host_resources()?);
    }
    Ok(roster)
}

fn recover_sdma_owner_preflight_error(
    failure: Gfx942SdmaQueueOwnerCreationFailureV1,
) -> Gfx942SdmaErrorV1 {
    match failure {
        Gfx942SdmaQueueOwnerCreationFailureV1::RetryableBeforeCreate(error) => error,
        Gfx942SdmaQueueOwnerCreationFailureV1::TerminalAfterMemoryOperation { .. } => {
            std::process::abort()
        }
    }
}

#[derive(Clone, Copy)]
enum Gfx942SdmaEngineProfileV1 {
    Ordinary(KfdGfx942SdmaEngineId),
    Xgmi(KfdGfx942SdmaXgmiEngineId),
}

impl Gfx942SdmaEngineProfileV1 {
    const fn value(self) -> u32 {
        match self {
            Self::Ordinary(engine) => engine.value(),
            Self::Xgmi(engine) => engine.value(),
        }
    }
}

impl Gfx942SdmaQueueOwnerV1 {
    fn collect_compute_coexistence_endpoints_v1(
        &self,
        session: SharedGttAllocationIdentityV1,
        endpoints: &mut arrayvec::ArrayVec<fe2o3_runtime_model::R66DeviceStorageV1, 258>,
    ) -> Option<()> {
        let slots = GFX942_SDMA_RING_SLOT_COUNT_V1;
        if self.records.len() != slots
            || self.xgmi_records.len() != slots
            || self.persistent_window_slots.len() != slots
            || self.persistent_window_records.len() != slots
            || self.xgmi_records.iter().any(Option::is_some)
        {
            return None;
        }
        let mut occupied = 0;
        for slot in 0..slots {
            if let Some(record) = &self.records[slot] {
                if !record.directional_persistent
                    || record.copy_bytes > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
                    || record.generation == 0
                    || record.generation != self.generations[slot]
                    || record.completion_value != record.generation
                    || self.persistent_window_slots[slot].is_some()
                    || self.persistent_window_records[slot].is_some()
                {
                    return None;
                }
                self.collect_compute_coexistence_pair_v1(
                    session,
                    &record.source,
                    &record.destination,
                    record.source_offset,
                    record.destination_offset,
                    record.copy_bytes,
                    endpoints,
                )?;
                occupied += 1;
            }
            if let Some(window_slot) = self.persistent_window_slots[slot] {
                let record = self
                    .persistent_window_records
                    .get(window_slot.anchor_slot)?
                    .as_ref()?;
                if !fe2o3_runtime_model::r66_window_slot_matches_v1(
                    slot,
                    window_slot.anchor_slot,
                    record.packet_count,
                    window_slot.generation,
                    self.generations[slot],
                    window_slot.completion_value,
                ) {
                    return None;
                }
                occupied += 1;
            }
            if let Some(record) = &self.persistent_window_records[slot] {
                if persistent_sdma_window_packet_count(record.request.copy_bytes).ok()
                    != Some(record.packet_count)
                {
                    return None;
                }
                for offset in 0..record.packet_count {
                    let linked = self.persistent_window_slots[(slot + offset) % slots]?;
                    if linked.anchor_slot != slot {
                        return None;
                    }
                }
                let request = &record.request;
                self.collect_compute_coexistence_pair_v1(
                    session,
                    &request.source,
                    &request.destination,
                    request.source_offset,
                    request.destination_offset,
                    request.copy_bytes,
                    endpoints,
                )?;
            }
        }
        (occupied <= GFX942_SDMA_MAX_IN_FLIGHT_V1).then_some(())
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_compute_coexistence_pair_v1(
        &self,
        session: SharedGttAllocationIdentityV1,
        source: &Gfx942SdmaBufferV1,
        destination: &Gfx942SdmaBufferV1,
        source_offset: u64,
        destination_offset: u64,
        bytes: u32,
        endpoints: &mut arrayvec::ArrayVec<fe2o3_runtime_model::R66DeviceStorageV1, 258>,
    ) -> Option<()> {
        let (host, device) = match self.engine_index {
            Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1) => (destination, source),
            Some(GFX942_SDMA_H2D_ENGINE_INDEX_V1) => (source, destination),
            _ => return None,
        };
        let Gfx942SdmaBufferStorageIdentityV1::Host(host_identity) = host.storage_identity() else {
            return None;
        };
        let Gfx942SdmaBufferStorageIdentityV1::Device(device_identity) = device.storage_identity()
        else {
            return None;
        };
        if bytes == 0 || !host_identity.same_retained_session_v1(session) {
            return None;
        }
        for (buffer, offset) in [(source, source_offset), (destination, destination_offset)] {
            if !buffer.belongs_to(self.owner)
                || buffer.pool_generation == 0
                || buffer.logical_bytes == 0
                || buffer.logical_bytes > buffer.physical_bytes()
                || offset.checked_add(u64::from(bytes))? > buffer.logical_bytes
            {
                return None;
            }
        }
        endpoints
            .try_push(device_identity.coexistence_facts_v1()?)
            .ok()?;
        Some(())
    }

    #[allow(clippy::result_large_err)]
    fn create_on_engine_in_armed_scope(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine: KfdGfx942SdmaEngineId,
        host: PreparedGfx942SdmaQueueHostResourcesV1,
        creation_arm: &ProcessGlobalKfdRuntimeCreationArmV1,
    ) -> Result<Self, Gfx942SdmaQueueOwnerCreationFailureV1> {
        Self::create_with_engine_in_armed_scope(
            memory,
            owner,
            Some(Gfx942SdmaEngineProfileV1::Ordinary(engine)),
            host,
            creation_arm,
        )
    }

    #[allow(clippy::result_large_err)]
    fn create_on_xgmi_engine_in_armed_scope(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine: KfdGfx942SdmaXgmiEngineId,
        host: PreparedGfx942SdmaQueueHostResourcesV1,
        creation_arm: &ProcessGlobalKfdRuntimeCreationArmV1,
    ) -> Result<Self, Gfx942SdmaQueueOwnerCreationFailureV1> {
        Self::create_with_engine_in_armed_scope(
            memory,
            owner,
            Some(Gfx942SdmaEngineProfileV1::Xgmi(engine)),
            host,
            creation_arm,
        )
    }

    #[allow(clippy::result_large_err)]
    fn create_with_engine_in_armed_scope(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine: Option<Gfx942SdmaEngineProfileV1>,
        host: PreparedGfx942SdmaQueueHostResourcesV1,
        _creation_arm: &ProcessGlobalKfdRuntimeCreationArmV1,
    ) -> Result<Self, Gfx942SdmaQueueOwnerCreationFailureV1> {
        let PreparedGfx942SdmaQueueHostResourcesV1 {
            records,
            xgmi_records,
            persistent_window_slots,
            persistent_window_records,
            doorbell_failure,
        } = host;
        let prepared = (|| -> Result<_, Gfx942SdmaErrorV1> {
            // The first live shared-memory/currentness operation is the
            // terminal creation boundary. These paths may consume or
            // quarantine authority even when no native queue exists yet.
            memory.check_queue_currentness()?;
            let mut ring = memory.allocate_aql_queue(GFX942_SDMA_RING_BYTES_V1 as usize)?;
            memory.with_bytes_mut(&mut ring, |bytes| bytes.fill(0))?;
            let ring = memory.map_to_gpu(ring)?;
            let ring_facts = memory.mapped_resource_facts(&ring)?;
            let ring = memory.retain_aql_ring_resource(ring)?;
            let mut control = memory.allocate_userptr_aql_control()?;
            memory
                .with_bytes_mut(&mut control, initialize_amd_aql_control)?
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA control initialization"))?;
            let mut completions =
                memory.allocate_host_visible_coherent(GFX942_SDMA_RING_BYTES_V1 as usize)?;
            memory.with_bytes_mut(&mut completions, |bytes| bytes.fill(0))?;
            let control = memory.map_to_gpu(control)?;
            let completions = memory.map_to_gpu(completions)?;
            let control_facts = memory.mapped_resource_facts(&control)?;
            let control = memory.retain_aql_control_resource(control)?;
            let buffers = KfdSdmaQueueBuffers {
                ring_base_address: ring_facts.gpu_va(),
                write_pointer_address: control_facts
                    .gpu_va()
                    .checked_add(AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1 as u64)
                    .ok_or(Gfx942SdmaErrorV1::Contract("SDMA write pointer address"))?,
                read_pointer_address: control_facts
                    .gpu_va()
                    .checked_add(AMD_AQL_READ_DISPATCH_ID_OFFSET_V1 as u64)
                    .ok_or(Gfx942SdmaErrorV1::Contract("SDMA read pointer address"))?,
            };
            let ring_size = admit_kfd_aql_queue_ring_size(GFX942_SDMA_RING_BYTES_V1)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA ring size"))?;
            let queue_percentage = admit_kfd_queue_percentage(100)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA queue percentage"))?;
            let queue_priority = admit_kfd_queue_priority(0)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA queue priority"))?;
            let expected = match engine {
                Some(Gfx942SdmaEngineProfileV1::Ordinary(engine)) => {
                    KfdIoctlCreateQueueArgs::new_sdma_on_engine(
                        buffers,
                        ring_size,
                        memory.gpu_id(),
                        queue_percentage,
                        queue_priority,
                        engine,
                    )
                }
                Some(Gfx942SdmaEngineProfileV1::Xgmi(engine)) => {
                    KfdIoctlCreateQueueArgs::new_sdma_xgmi_on_engine(
                        buffers,
                        ring_size,
                        memory.gpu_id(),
                        queue_percentage,
                        queue_priority,
                        engine,
                    )
                }
                None => KfdIoctlCreateQueueArgs::new_sdma(
                    buffers,
                    ring_size,
                    memory.gpu_id(),
                    queue_percentage,
                    queue_priority,
                ),
            };
            Ok((
                PreparedGfx942SdmaQueueV1 {
                    owner,
                    engine_index: engine.map(Gfx942SdmaEngineProfileV1::value),
                    ring,
                    control,
                    completions,
                    records,
                    xgmi_records,
                    persistent_window_slots,
                    persistent_window_records,
                },
                expected,
            ))
        })();
        let (prepared, expected) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(Gfx942SdmaQueueOwnerCreationFailureV1::terminal(
                    error,
                    TerminalGfx942SdmaQueueCreationV1::OpaqueAfterFirstMemoryOperation,
                ));
            }
        };
        let mut actual = expected;
        if create_queue(memory.kfd_fd(), &mut actual).is_err() {
            return Err(Gfx942SdmaQueueOwnerCreationFailureV1::terminal(
                Gfx942SdmaErrorV1::QueueCreationIndeterminate,
                TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                    prepared,
                    native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs {
                        queue_id: actual.queue_id,
                        doorbell_offset: actual.doorbell_offset,
                    },
                    doorbell: None,
                },
            ));
        }
        let output_queue_id = actual.queue_id;
        let output_doorbell = actual.doorbell_offset;
        actual.queue_id = u32::MAX;
        actual.doorbell_offset = u64::MAX;
        if actual != expected {
            return Err(Gfx942SdmaQueueOwnerCreationFailureV1::terminal(
                Gfx942SdmaErrorV1::Contract("kernel changed immutable SDMA CREATE_QUEUE inputs"),
                TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                    prepared,
                    native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs {
                        queue_id: output_queue_id,
                        doorbell_offset: output_doorbell,
                    },
                    doorbell: None,
                },
            ));
        }
        let outputs = match admit_kfd_gfx942_create_queue_outputs(
            output_queue_id,
            output_doorbell,
            memory.gpu_id(),
        ) {
            Ok(outputs) => outputs,
            Err(_) => {
                return Err(Gfx942SdmaQueueOwnerCreationFailureV1::terminal(
                    Gfx942SdmaErrorV1::Contract("SDMA CREATE_QUEUE outputs"),
                    TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                        prepared,
                        native: Gfx942SdmaQueueCreationNativeObservationV1::UntrustedIoctlOutputs {
                            queue_id: output_queue_id,
                            doorbell_offset: output_doorbell,
                        },
                        doorbell: None,
                    },
                ));
            }
        };
        let queue_id = outputs.queue_id().value();
        let doorbell =
            match LinuxDoorbellSliceV1::map(memory.kfd_fd(), outputs, memory.opener_pid()) {
                Ok(doorbell) => doorbell,
                Err(_) => {
                    return Err(Gfx942SdmaQueueOwnerCreationFailureV1::terminal(
                        Gfx942SdmaErrorV1::Doorbell(doorbell_failure),
                        TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                            prepared,
                            native: Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(
                                queue_id,
                            ),
                            doorbell: None,
                        },
                    ));
                }
            };
        if let Err(error) = memory.check_queue_currentness() {
            return Err(Gfx942SdmaQueueOwnerCreationFailureV1::terminal(
                error.into(),
                TerminalGfx942SdmaQueueCreationV1::QueueAttempt {
                    prepared,
                    native: Gfx942SdmaQueueCreationNativeObservationV1::ValidatedQueueId(queue_id),
                    doorbell: Some(doorbell),
                },
            ));
        }

        let owner = prepared.into_live(queue_id, doorbell);
        Ok(owner)
    }

    pub(crate) const fn observation(&self) -> Gfx942SdmaQueueObservationV1 {
        Gfx942SdmaQueueObservationV1 {
            queue_id: self.queue_id,
            ring_bytes: GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: self.engine_index,
        }
    }

    pub(crate) fn preflight_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        source: &Gfx942SdmaBufferV1,
        source_offset: u64,
        destination: &Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_live()?;
        Self::checked_copy_addresses(
            memory,
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
        )?;
        self.observe_batch_start(memory, 1)?;
        Ok(())
    }

    fn observe_batch_start(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        count: usize,
    ) -> Result<u64, Gfx942SdmaErrorV1> {
        if count == 0 || count > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let control = self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing SDMA control authority",
        ))?;
        let (write, read) = memory.observe_aql_control_counters_in_current_scope(control)?;
        validate_sdma_write_counter_or_poison(write, &mut self.poisoned)?;
        let requested = (count as u64)
            .checked_mul(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA batch byte count"))?;
        if !sdma_ring_delta_is_below_capacity(write, read) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        let end = checked_sdma_write_end(write, requested, &mut self.poisoned)?;
        if !sdma_ring_delta_is_below_capacity(end, read) {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        for index in 0..count {
            let slot = batch_ring_slot(write, index)?;
            if self.records[slot].is_some()
                || self.xgmi_records[slot].is_some()
                || self.persistent_window_slots[slot].is_some()
            {
                return Err(Gfx942SdmaErrorV1::QueueFull);
            }
        }
        Ok(write)
    }

    pub(crate) fn submit(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        source: Gfx942SdmaBufferV1,
        source_offset: u64,
        destination: Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let source_address =
            source.checked_gpu_subrange(memory, source_offset, u64::from(copy_bytes))?;
        let destination_address =
            destination.checked_gpu_subrange(memory, destination_offset, u64::from(copy_bytes))?;
        if ranges_overlap(
            source_address,
            u64::from(copy_bytes),
            destination_address,
            u64::from(copy_bytes),
        ) {
            return Err(Gfx942SdmaErrorV1::Contract("overlapping SDMA copy ranges"));
        }

        let control = self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing SDMA control authority",
        ))?;
        let (write, read) = memory.observe_aql_control_counters_in_current_scope(control)?;
        validate_sdma_write_counter_or_poison(write, &mut self.poisoned)?;
        if !sdma_ring_delta_is_below_capacity(write, read) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        let write_end = checked_sdma_write_end(
            write,
            GFX942_SDMA_SUBMISSION_BYTES_V1 as u64,
            &mut self.poisoned,
        )?;
        if !sdma_ring_delta_is_below_capacity(write_end, read) {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let ring_slot = ((write % u64::from(GFX942_SDMA_RING_BYTES_V1))
            / GFX942_SDMA_SUBMISSION_BYTES_V1 as u64) as usize;
        if self.records[ring_slot].is_some()
            || self.xgmi_records[ring_slot].is_some()
            || self.persistent_window_slots[ring_slot].is_some()
        {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let generation =
            next_sdma_ticket_generation(self.generations[ring_slot], &mut self.poisoned)?;
        let completion_value = generation;
        let completion_offset = (ring_slot * 8) as u64;
        let completion_address = memory
            .mapped_resource_facts(
                self.completions
                    .as_ref()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
            )?
            .checked_gpu_subrange(completion_offset, 4, 4)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA completion address"))?;
        let packet = Gfx942SdmaCopySubmissionV1::new(
            source_address,
            destination_address,
            copy_bytes,
            completion_address,
            completion_value,
        )?;
        if self.ring.is_none() || self.control.is_none() || self.doorbell.is_none() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "missing SDMA publication authority",
            ));
        }

        let doorbell_failure = preallocate_doorbell_failure_message()?;
        self.poisoned = true;
        let completions = self.completions.as_mut().expect("checked completion arena");
        memory.overwrite_mapped_host_visible_subrange_in_current_scope(
            completions,
            completion_offset,
            &[0; 8],
        )?;
        memory.write_sdma_ring_slot_in_current_scope(
            self.ring.as_mut().expect("checked SDMA ring"),
            ring_slot as u32,
            packet.bytes(),
        )?;
        self.generations[ring_slot] = generation;
        self.records[ring_slot] = Some(SdmaCopyRecordV1 {
            directional_persistent: false,
            generation,
            completion_value,
            fence_header: packet.fence_header(),
            completion_observed: false,
            source,
            destination,
            copy_bytes,
            source_offset,
            destination_offset,
        });
        memory.publish_sdma_control_write_release_in_current_scope(
            self.control.as_mut().expect("checked SDMA control"),
            write,
            write_end,
        )?;
        self.doorbell
            .as_mut()
            .expect("checked SDMA doorbell")
            .store_packet_id_release(write_end)
            .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))?;
        self.poisoned = false;
        Ok(Gfx942SdmaCopyTicketV1 {
            owner: self.owner,
            queue_id: self.queue_id,
            slot: ring_slot as u16,
            generation,
        })
    }

    fn submit_xgmi(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        source: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        source_address: u64,
        destination: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        destination_address: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let source_mapping = source
            .as_ref()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI source mapping"))?;
        let destination_mapping = destination.as_ref().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing XGMI destination mapping",
        ))?;
        if !source_mapping.is_fully_mapped()
            || !destination_mapping.is_fully_mapped()
            || source_mapping.gpu_ids() != destination_mapping.gpu_ids()
            || copy_bytes == 0
            || copy_bytes > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
            || ranges_overlap(
                source_address,
                u64::from(copy_bytes),
                destination_address,
                u64::from(copy_bytes),
            )
        {
            return Err(Gfx942SdmaErrorV1::Contract("XGMI SDMA copy binding"));
        }
        let control = self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing SDMA control authority",
        ))?;
        let (write, read) = memory.observe_aql_control_counters_in_current_scope(control)?;
        validate_sdma_write_counter_or_poison(write, &mut self.poisoned)?;
        if !sdma_ring_delta_is_below_capacity(write, read) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        let write_end = checked_sdma_write_end(
            write,
            GFX942_SDMA_SUBMISSION_BYTES_V1 as u64,
            &mut self.poisoned,
        )?;
        if !sdma_ring_delta_is_below_capacity(write_end, read) {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let ring_slot = ((write % u64::from(GFX942_SDMA_RING_BYTES_V1))
            / GFX942_SDMA_SUBMISSION_BYTES_V1 as u64) as usize;
        if self.records[ring_slot].is_some()
            || self.xgmi_records[ring_slot].is_some()
            || self.persistent_window_slots[ring_slot].is_some()
        {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let generation =
            next_sdma_ticket_generation(self.generations[ring_slot], &mut self.poisoned)?;
        let completion_value = generation;
        let completion_offset = (ring_slot * 8) as u64;
        let completion_address = memory
            .mapped_resource_facts(
                self.completions
                    .as_ref()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
            )?
            .checked_gpu_subrange(completion_offset, 4, 4)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA completion address"))?;
        let packet = Gfx942SdmaCopySubmissionV1::new(
            source_address,
            destination_address,
            copy_bytes,
            completion_address,
            completion_value,
        )?;
        if self.ring.is_none() || self.control.is_none() || self.doorbell.is_none() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "missing SDMA publication authority",
            ));
        }

        let doorbell_failure = preallocate_doorbell_failure_message()?;
        let source = source.take().expect("checked XGMI source mapping");
        let destination = destination
            .take()
            .expect("checked XGMI destination mapping");
        self.generations[ring_slot] = generation;
        self.xgmi_records[ring_slot] = Some(XgmiSdmaCopyRecordV1 {
            generation,
            completion_value,
            source,
            destination,
            copy_bytes,
        });
        let ticket = Gfx942SdmaCopyTicketV1 {
            owner: self.owner,
            queue_id: self.queue_id,
            slot: ring_slot as u16,
            generation,
        };
        self.uncertain_xgmi_ticket = Some(ticket);
        self.poisoned = true;
        let completions = self.completions.as_mut().expect("checked completion arena");
        memory.overwrite_mapped_host_visible_subrange_in_current_scope(
            completions,
            completion_offset,
            &[0; 8],
        )?;
        memory.write_sdma_ring_slot_in_current_scope(
            self.ring.as_mut().expect("checked SDMA ring"),
            ring_slot as u32,
            packet.bytes(),
        )?;
        memory.publish_sdma_control_write_release_in_current_scope(
            self.control.as_mut().expect("checked SDMA control"),
            write,
            write_end,
        )?;
        self.doorbell
            .as_mut()
            .expect("checked SDMA doorbell")
            .store_packet_id_release(write_end)
            .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))?;
        self.poisoned = false;
        self.uncertain_xgmi_ticket = None;
        Ok(ticket)
    }

    fn prepare_xgmi_batch_recoverable(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        route: crate::topology::Gfx942XgmiRouteV1,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<PreparedXgmiSdmaBatchV1, (Gfx942SdmaErrorV1, Vec<Gfx942XgmiSdmaCopyRequestV1>)>
    {
        match self.prepare_xgmi_batch(source_session, destination_session, route, &requests) {
            Ok((write, write_end, copies, tickets)) => {
                let doorbell_failure = match preallocate_doorbell_failure_message() {
                    Ok(message) => message,
                    Err(error) => return Err((error, requests)),
                };
                Ok(PreparedXgmiSdmaBatchV1 {
                    queue_id: self.queue_id,
                    write,
                    write_end,
                    copies,
                    tickets,
                    requests,
                    doorbell_failure,
                })
            }
            Err(error) => Err((error, requests)),
        }
    }

    #[allow(clippy::type_complexity)]
    fn prepare_xgmi_batch(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        route: crate::topology::Gfx942XgmiRouteV1,
        requests: &[Gfx942XgmiSdmaCopyRequestV1],
    ) -> Result<
        (
            u64,
            u64,
            Vec<PreparedXgmiSdmaCopyV1>,
            Vec<Gfx942SdmaCopyTicketV1>,
        ),
        Gfx942SdmaErrorV1,
    > {
        self.require_live()?;
        let write = self.observe_batch_start(source_session, requests.len())?;
        let write_end = checked_sdma_write_end(
            write,
            submission_batch_bytes(requests.len())?,
            &mut self.poisoned,
        )?;
        let completion_base = source_session
            .mapped_resource_facts(
                self.completions
                    .as_ref()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
            )?
            .gpu_va();
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(requests.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI SDMA packet roster allocation"))?;
        let mut tickets = Vec::new();
        tickets
            .try_reserve_exact(requests.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI SDMA ticket roster allocation"))?;
        for (index, request) in requests.iter().enumerate() {
            if !request.source.is_fully_mapped()
                || !request.destination.is_fully_mapped()
                || request.source.gpu_ids() != route.canonical_mapping_gpu_ids()
                || request.destination.gpu_ids() != route.canonical_mapping_gpu_ids()
            {
                return Err(Gfx942SdmaErrorV1::Contract("XGMI mapping route roster"));
            }
            let source_address = source_session
                .mapped_xgmi_device_memory_facts(&request.source)?
                .checked_gpu_subrange(request.source_offset, u64::from(request.copy_bytes), 1)
                .ok_or(Gfx942SdmaErrorV1::Contract("XGMI source copy range"))?;
            let destination_address = destination_session
                .mapped_xgmi_device_memory_facts(&request.destination)?
                .checked_gpu_subrange(request.destination_offset, u64::from(request.copy_bytes), 1)
                .ok_or(Gfx942SdmaErrorV1::Contract("XGMI destination copy range"))?;
            if request.copy_bytes == 0
                || request.copy_bytes > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
                || ranges_overlap(
                    source_address,
                    u64::from(request.copy_bytes),
                    destination_address,
                    u64::from(request.copy_bytes),
                )
            {
                return Err(Gfx942SdmaErrorV1::Contract("XGMI SDMA copy binding"));
            }
            let slot = batch_ring_slot(write, index)?;
            let generation =
                next_sdma_ticket_generation(self.generations[slot], &mut self.poisoned)?;
            let completion_address = completion_base
                .checked_add((slot * 8) as u64)
                .ok_or(Gfx942SdmaErrorV1::Contract("XGMI SDMA completion address"))?;
            prepared.push(PreparedXgmiSdmaCopyV1 {
                packet: Gfx942SdmaCopySubmissionV1::new(
                    source_address,
                    destination_address,
                    request.copy_bytes,
                    completion_address,
                    generation,
                )?,
                slot,
                generation,
                completion_value: generation,
            });
            tickets.push(Gfx942SdmaCopyTicketV1 {
                owner: self.owner,
                queue_id: self.queue_id,
                slot: slot as u16,
                generation,
            });
        }
        Ok((write, write_end, prepared, tickets))
    }

    fn submit_prepared_xgmi_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedXgmiSdmaBatchV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, (Gfx942SdmaErrorV1, Vec<Gfx942SdmaCopyTicketV1>)> {
        if prepared.queue_id != self.queue_id
            || prepared.requests.len() != prepared.copies.len()
            || prepared.requests.len() != prepared.tickets.len()
        {
            self.poisoned = true;
            return Err((
                Gfx942SdmaErrorV1::Contract("XGMI SDMA prepared batch queue or roster"),
                prepared.tickets,
            ));
        }
        let publication_plan = match admit_sdma_batch_publication_plan(
            prepared.write,
            prepared.write_end,
            prepared.copies.len(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.poisoned = true;
                return Err((error, prepared.tickets));
            }
        };
        let PreparedXgmiSdmaBatchV1 {
            queue_id: _,
            write: _,
            write_end: _,
            copies,
            tickets,
            requests,
            doorbell_failure,
        } = prepared;
        // Retain every move-only mapping before the first fallible mapped write.
        // Thereafter any error returns only tickets and leaves native custody here.
        for (request, item) in requests.into_iter().zip(&copies) {
            self.generations[item.slot] = item.generation;
            self.xgmi_records[item.slot] = Some(XgmiSdmaCopyRecordV1 {
                generation: item.generation,
                completion_value: item.completion_value,
                source: request.source,
                destination: request.destination,
                copy_bytes: request.copy_bytes,
            });
        }
        self.poisoned = true;
        let publication = (|| {
            let completions = self
                .completions
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing XGMI SDMA completion arena",
                ))?;
            for item in &copies {
                memory.overwrite_mapped_host_visible_subrange_in_current_scope(
                    completions,
                    (item.slot * 8) as u64,
                    &[0; 8],
                )?;
            }
            let ring = self.ring.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
                "missing XGMI SDMA ring authority",
            ))?;
            for item in &copies {
                memory.write_sdma_ring_slot_in_current_scope(
                    ring,
                    item.slot as u32,
                    item.packet.bytes(),
                )?;
            }
            memory.publish_sdma_control_write_release_in_current_scope(
                self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing XGMI SDMA control authority",
                ))?,
                publication_plan.write,
                publication_plan.write_end,
            )?;
            self.doorbell
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA doorbell"))?
                .store_packet_id_release(publication_plan.write_end)
                .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))
        })();
        match publication {
            Ok(()) => {
                self.poisoned = false;
                Ok(tickets)
            }
            Err(error) => Err((error, tickets)),
        }
    }

    fn prepare_batch_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<PreparedSdmaBatchV1, (Gfx942SdmaErrorV1, Vec<Gfx942SdmaCopyRequestV1>)> {
        match self.prepare_batch(memory, &requests) {
            Ok((write, write_end, copies, tickets)) => {
                let doorbell_failure = match preallocate_doorbell_failure_message() {
                    Ok(message) => message,
                    Err(error) => return Err((error, requests)),
                };
                Ok(PreparedSdmaBatchV1 {
                    queue_id: self.queue_id,
                    write,
                    write_end,
                    copies,
                    tickets,
                    requests,
                    doorbell_failure,
                })
            }
            Err(error) => Err((error, requests)),
        }
    }

    #[allow(clippy::result_large_err)]
    fn prepare_single_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Result<PreparedSingleSdmaV1, (Gfx942SdmaErrorV1, Gfx942SdmaCopyRequestV1)> {
        let prepared = (|| {
            self.require_live()?;
            let write = self.observe_batch_start(memory, 1)?;
            let write_end = checked_sdma_write_end(
                write,
                GFX942_SDMA_SUBMISSION_BYTES_V1 as u64,
                &mut self.poisoned,
            )?;
            let (source_address, destination_address) = Self::checked_copy_addresses(
                memory,
                &request.source,
                request.source_offset,
                &request.destination,
                request.destination_offset,
                request.copy_bytes,
            )?;
            let slot = batch_ring_slot(write, 0)?;
            let generation =
                next_sdma_ticket_generation(self.generations[slot], &mut self.poisoned)?;
            let completion_address = memory
                .mapped_resource_facts(
                    self.completions
                        .as_ref()
                        .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
                )?
                .gpu_va()
                .checked_add((slot * 8) as u64)
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA completion address"))?;
            let copy = PreparedSdmaCopyV1 {
                packet: Gfx942SdmaCopySubmissionV1::new(
                    source_address,
                    destination_address,
                    request.copy_bytes,
                    completion_address,
                    generation,
                )?,
                slot,
                generation,
                completion_value: generation,
            };
            Ok((write, write_end, copy, slot, generation))
        })();
        match prepared {
            Ok((write, write_end, copy, slot, generation)) => Ok(PreparedSingleSdmaV1 {
                directional_persistent: false,
                queue_id: self.queue_id,
                write,
                write_end,
                copy,
                ticket: Gfx942SdmaCopyTicketV1 {
                    owner: self.owner,
                    queue_id: self.queue_id,
                    slot: slot as u16,
                    generation,
                },
                request,
            }),
            Err(error) => Err((error, request)),
        }
    }

    #[allow(clippy::result_large_err)]
    fn submit_prepared_single_with_custody(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedSingleSdmaV1,
    ) -> Result<Gfx942SdmaCopyTicketV1, PreparedSingleSdmaPublicationFailureV1> {
        if prepared.queue_id != self.queue_id {
            return Err(PreparedSingleSdmaPublicationFailureV1::Recoverable {
                error: Gfx942SdmaErrorV1::Contract("SDMA prepared single queue"),
                prepared,
            });
        }
        if let Err(error) = admit_sdma_batch_publication_plan(prepared.write, prepared.write_end, 1)
        {
            return Err(PreparedSingleSdmaPublicationFailureV1::Recoverable { error, prepared });
        }
        if self.completions.is_none()
            || self.ring.is_none()
            || self.control.is_none()
            || self.doorbell.is_none()
        {
            return Err(PreparedSingleSdmaPublicationFailureV1::Recoverable {
                error: Gfx942SdmaErrorV1::Contract("missing SDMA publication authority"),
                prepared,
            });
        }
        let prepared_slot_is_free = self.records[prepared.copy.slot].is_none()
            && self.xgmi_records[prepared.copy.slot].is_none()
            && self.persistent_window_slots[prepared.copy.slot].is_none()
            && self.generations[prepared.copy.slot]
                .checked_add(1)
                .filter(|generation| *generation != 0)
                == Some(prepared.copy.generation);
        if !prepared_slot_is_free {
            return Err(PreparedSingleSdmaPublicationFailureV1::Recoverable {
                error: Gfx942SdmaErrorV1::Contract("SDMA prepared single slot occupancy"),
                prepared,
            });
        }
        let PreparedSingleSdmaV1 {
            directional_persistent,
            write,
            write_end,
            copy,
            ticket,
            request,
            ..
        } = prepared;
        self.generations[copy.slot] = copy.generation;
        self.records[copy.slot] = Some(SdmaCopyRecordV1 {
            directional_persistent,
            generation: copy.generation,
            completion_value: copy.completion_value,
            fence_header: copy.packet.fence_header(),
            completion_observed: false,
            source: request.source,
            destination: request.destination,
            copy_bytes: request.copy_bytes,
            source_offset: request.source_offset,
            destination_offset: request.destination_offset,
        });
        self.poisoned = true;
        let publication = (|| {
            memory.overwrite_mapped_host_visible_subrange_in_current_scope(
                self.completions.as_mut().expect("checked completion arena"),
                (copy.slot * 8) as u64,
                &[0; 8],
            )?;
            memory.write_sdma_ring_slot_in_current_scope(
                self.ring.as_mut().expect("checked SDMA ring"),
                copy.slot as u32,
                copy.packet.bytes(),
            )?;
            memory.publish_sdma_control_write_release_in_current_scope(
                self.control.as_mut().expect("checked SDMA control"),
                write,
                write_end,
            )?;
            self.doorbell
                .as_mut()
                .expect("checked SDMA doorbell")
                .store_packet_id_release(write_end)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA doorbell operation failed"))
        })();
        match publication {
            Ok(()) => {
                self.poisoned = false;
                Ok(ticket)
            }
            Err(error) => Err(PreparedSingleSdmaPublicationFailureV1::Retained { error, ticket }),
        }
    }

    #[allow(clippy::result_large_err)]
    fn prepare_persistent_window_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Result<PreparedPersistentSdmaWindowV1, (Gfx942SdmaErrorV1, Gfx942SdmaCopyRequestV1)> {
        let prepared = (|| {
            self.require_live()?;
            let packet_count = persistent_sdma_window_packet_count(request.copy_bytes)?;
            let write = self.observe_batch_start(memory, packet_count)?;
            let write_end = checked_sdma_write_end(
                write,
                submission_batch_bytes(packet_count)?,
                &mut self.poisoned,
            )?;
            let (source_address, destination_address) = Self::checked_copy_addresses(
                memory,
                &request.source,
                request.source_offset,
                &request.destination,
                request.destination_offset,
                request.copy_bytes,
            )?;
            let completion_base = memory
                .mapped_resource_facts(self.completions.as_ref().ok_or(
                    Gfx942SdmaErrorV1::Contract("missing persistent SDMA window completion arena"),
                )?)?
                .gpu_va();
            let mut copies = Vec::new();
            copies
                .try_reserve_exact(packet_count)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("persistent SDMA window packets"))?;
            let mut tickets = Vec::new();
            tickets
                .try_reserve_exact(packet_count)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("persistent SDMA window tickets"))?;
            for index in 0..packet_count {
                let packet_offset = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or(Gfx942SdmaErrorV1::Contract(
                        "persistent SDMA window packet offset",
                    ))?;
                let remaining = u64::from(request.copy_bytes)
                    .checked_sub(packet_offset)
                    .ok_or(Gfx942SdmaErrorV1::Contract(
                        "persistent SDMA window packet extent",
                    ))?;
                let packet_bytes =
                    u32::try_from(remaining.min(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)))
                        .map_err(|_| {
                        Gfx942SdmaErrorV1::Contract("persistent SDMA window packet bytes")
                    })?;
                let slot = batch_ring_slot(write, index)?;
                let generation =
                    next_sdma_ticket_generation(self.generations[slot], &mut self.poisoned)?;
                let completion_address = completion_base.checked_add((slot * 8) as u64).ok_or(
                    Gfx942SdmaErrorV1::Contract("persistent SDMA window completion address"),
                )?;
                let packet_source = source_address.checked_add(packet_offset).ok_or(
                    Gfx942SdmaErrorV1::Contract("persistent SDMA window source address"),
                )?;
                let packet_destination = destination_address.checked_add(packet_offset).ok_or(
                    Gfx942SdmaErrorV1::Contract("persistent SDMA window destination address"),
                )?;
                copies.push(PreparedSdmaCopyV1 {
                    packet: Gfx942SdmaCopySubmissionV1::new(
                        packet_source,
                        packet_destination,
                        packet_bytes,
                        completion_address,
                        generation,
                    )?,
                    slot,
                    generation,
                    completion_value: generation,
                });
                tickets.push(Gfx942SdmaCopyTicketV1 {
                    owner: self.owner,
                    queue_id: self.queue_id,
                    slot: slot as u16,
                    generation,
                });
            }
            Ok((
                write,
                write_end,
                copies,
                tickets,
                preallocate_doorbell_failure_message()?,
            ))
        })();
        match prepared {
            Ok((write, write_end, copies, tickets, doorbell_failure)) => {
                Ok(PreparedPersistentSdmaWindowV1 {
                    queue_id: self.queue_id,
                    write,
                    write_end,
                    copies,
                    tickets,
                    request,
                    doorbell_failure,
                })
            }
            Err(error) => Err((error, request)),
        }
    }

    #[allow(clippy::result_large_err)]
    fn submit_prepared_persistent_window_with_custody(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedPersistentSdmaWindowV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, PreparedPersistentSdmaWindowPublicationFailureV1> {
        let recover = |error, prepared| {
            PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable { error, prepared }
        };
        if prepared.queue_id != self.queue_id
            || prepared.copies.len() != prepared.tickets.len()
            || prepared.copies.is_empty()
        {
            return Err(recover(
                Gfx942SdmaErrorV1::Contract("persistent SDMA window queue or roster"),
                prepared,
            ));
        }
        let publication_plan = match admit_sdma_batch_publication_plan(
            prepared.write,
            prepared.write_end,
            prepared.copies.len(),
        ) {
            Ok(plan) => plan,
            Err(error) => return Err(recover(error, prepared)),
        };
        if self.completions.is_none()
            || self.ring.is_none()
            || self.control.is_none()
            || self.doorbell.is_none()
        {
            return Err(recover(
                Gfx942SdmaErrorV1::Contract("missing persistent SDMA window authority"),
                prepared,
            ));
        }
        for (index, (copy, ticket)) in prepared
            .copies
            .iter()
            .zip(prepared.tickets.iter())
            .enumerate()
        {
            let expected_slot = match batch_ring_slot(prepared.write, index) {
                Ok(slot) => slot,
                Err(error) => return Err(recover(error, prepared)),
            };
            if copy.slot != expected_slot
                || usize::from(ticket.slot) != expected_slot
                || ticket.owner != self.owner
                || ticket.queue_id != self.queue_id
                || ticket.generation != copy.generation
                || copy.completion_value != copy.generation
                || self.generations[copy.slot]
                    .checked_add(1)
                    .filter(|generation| *generation != 0)
                    != Some(copy.generation)
                || self.records[copy.slot].is_some()
                || self.xgmi_records[copy.slot].is_some()
                || self.persistent_window_slots[copy.slot].is_some()
                || self.persistent_window_records[copy.slot].is_some()
            {
                return Err(recover(
                    Gfx942SdmaErrorV1::Contract("persistent SDMA window prepared identity"),
                    prepared,
                ));
            }
        }

        let PreparedPersistentSdmaWindowV1 {
            copies,
            tickets,
            request,
            doorbell_failure,
            ..
        } = prepared;
        let anchor_slot = copies[0].slot;
        let packet_count = copies.len();
        self.persistent_window_records[anchor_slot] = Some(PersistentSdmaWindowRecordV1 {
            request,
            packet_count,
        });
        for copy in &copies {
            self.generations[copy.slot] = copy.generation;
            self.persistent_window_slots[copy.slot] = Some(PersistentSdmaWindowSlotV1 {
                anchor_slot,
                generation: copy.generation,
                completion_value: copy.completion_value,
            });
        }
        self.poisoned = true;
        let publication = (|| {
            let completions = self
                .completions
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing persistent SDMA window completion arena",
                ))?;
            for copy in &copies {
                memory.overwrite_mapped_host_visible_subrange_in_current_scope(
                    completions,
                    (copy.slot * 8) as u64,
                    &[0; 8],
                )?;
            }
            let ring = self.ring.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
                "missing persistent SDMA window ring",
            ))?;
            for copy in &copies {
                memory.write_sdma_ring_slot_in_current_scope(
                    ring,
                    copy.slot as u32,
                    copy.packet.bytes(),
                )?;
            }
            memory.publish_sdma_control_write_release_in_current_scope(
                self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing persistent SDMA window control",
                ))?,
                publication_plan.write,
                publication_plan.write_end,
            )?;
            self.doorbell
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing persistent SDMA window doorbell",
                ))?
                .store_packet_id_release(publication_plan.write_end)
                .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))
        })();
        match publication {
            Ok(()) => {
                self.poisoned = false;
                Ok(tickets)
            }
            Err(error) => {
                Err(PreparedPersistentSdmaWindowPublicationFailureV1::Retained { error, tickets })
            }
        }
    }

    fn validate_persistent_window_tickets(
        &self,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<usize, Gfx942SdmaErrorV1> {
        if tickets.is_empty() || tickets.len() > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            return Err(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window ticket count",
            ));
        }
        let anchor_slot = usize::from(tickets[0].slot);
        let record = self
            .persistent_window_records
            .get(anchor_slot)
            .and_then(Option::as_ref)
            .ok_or(Gfx942SdmaErrorV1::Contract("stale persistent SDMA window"))?;
        if record.packet_count != tickets.len() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window ticket roster",
            ));
        }
        for (index, ticket) in tickets.iter().copied().enumerate() {
            if !ticket_matches_queue_occurrence(ticket, self.owner, self.queue_id) {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "persistent SDMA window ticket queue occurrence",
                ));
            }
            let expected_slot = (anchor_slot + index) % GFX942_SDMA_RING_SLOT_COUNT_V1;
            let slot = self
                .persistent_window_slots
                .get(expected_slot)
                .and_then(Option::as_ref)
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "stale persistent SDMA window ticket",
                ))?;
            if usize::from(ticket.slot) != expected_slot
                || slot.anchor_slot != anchor_slot
                || slot.generation != ticket.generation
            {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "persistent SDMA window ticket generation or order",
                ));
            }
        }
        Ok(anchor_slot)
    }

    fn observe_persistent_window_completion(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        let anchor_slot = self.validate_persistent_window_tickets(tickets)?;
        let mut all_ready = true;
        for ticket in tickets {
            let slot_index = usize::from(ticket.slot);
            let expected = self.persistent_window_slots[slot_index]
                .as_ref()
                .expect("validated persistent window slot")
                .completion_value;
            let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                self.completions
                    .as_mut()
                    .ok_or(Gfx942SdmaErrorV1::Contract(
                        "missing persistent SDMA window completion arena",
                    ))?,
                (slot_index * 8) as u64,
            )?;
            if observed == 0 {
                all_ready = false;
            } else if observed != i64::from(expected) {
                self.poisoned = true;
                return Err(Gfx942SdmaErrorV1::Contract(
                    "unexpected persistent SDMA window completion value",
                ));
            }
        }
        debug_assert!(self.persistent_window_records[anchor_slot].is_some());
        Ok(all_ready)
    }

    fn complete_persistent_window(
        &mut self,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> CompletedPersistentSdmaWindowV1 {
        let anchor_slot = usize::from(tickets[0].slot);
        for ticket in tickets {
            self.persistent_window_slots[usize::from(ticket.slot)] = None;
        }
        let record = self.persistent_window_records[anchor_slot]
            .take()
            .expect("validated persistent SDMA window owner");
        CompletedPersistentSdmaWindowV1 {
            request: record.request,
            packet_count: record.packet_count,
        }
    }

    fn poll_persistent_window(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<PersistentSdmaWindowPollV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        memory.check_queue_operational_currentness()?;
        if !self.observe_persistent_window_completion(memory, tickets)? {
            memory.check_queue_operational_currentness()?;
            return Ok(PersistentSdmaWindowPollV1::Pending);
        }
        memory.check_queue_operational_currentness()?;
        Ok(PersistentSdmaWindowPollV1::Completed(
            self.complete_persistent_window(tickets),
        ))
    }

    fn wait_persistent_window_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
        wait_profile: SdmaWaitProfileV1,
    ) -> Result<CompletedPersistentSdmaWindowV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        self.validate_persistent_window_tickets(tickets)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window wait deadline",
            ))?;
        memory.check_queue_operational_currentness()?;
        let mut wait = wait_profile.cursor(deadline);
        if !wait
            .observe_until_ready(|| self.observe_persistent_window_completion(memory, tickets))?
        {
            memory.check_queue_operational_currentness()?;
            return Err(Gfx942SdmaErrorV1::Timeout);
        }
        memory.check_queue_operational_currentness()?;
        Ok(self.complete_persistent_window(tickets))
    }

    #[allow(clippy::type_complexity)]
    fn prepare_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: &[Gfx942SdmaCopyRequestV1],
    ) -> Result<
        (
            u64,
            u64,
            Vec<PreparedSdmaCopyV1>,
            Vec<Gfx942SdmaCopyTicketV1>,
        ),
        Gfx942SdmaErrorV1,
    > {
        self.require_live()?;
        let mut tickets = Vec::new();
        tickets
            .try_reserve_exact(requests.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA ticket roster allocation"))?;
        let write = self.observe_batch_start(memory, requests.len())?;
        let requested = submission_batch_bytes(requests.len())?;
        let write_end = checked_sdma_write_end(write, requested, &mut self.poisoned)?;
        let completion_base = memory
            .mapped_resource_facts(
                self.completions
                    .as_ref()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
            )?
            .gpu_va();
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(requests.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA packet roster allocation"))?;
        for (index, request) in requests.iter().enumerate() {
            let (source_address, destination_address) = Self::checked_copy_addresses(
                memory,
                &request.source,
                request.source_offset,
                &request.destination,
                request.destination_offset,
                request.copy_bytes,
            )?;
            let slot = batch_ring_slot(write, index)?;
            let generation =
                next_sdma_ticket_generation(self.generations[slot], &mut self.poisoned)?;
            let completion_offset = (slot * 8) as u64;
            let completion_address = completion_base
                .checked_add(completion_offset)
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA completion address"))?;
            let packet = Gfx942SdmaCopySubmissionV1::new(
                source_address,
                destination_address,
                request.copy_bytes,
                completion_address,
                generation,
            )?;
            prepared.push(PreparedSdmaCopyV1 {
                packet,
                slot,
                generation,
                completion_value: generation,
            });
            tickets.push(Gfx942SdmaCopyTicketV1 {
                owner: self.owner,
                queue_id: self.queue_id,
                slot: slot as u16,
                generation,
            });
        }
        Ok((write, write_end, prepared, tickets))
    }

    fn submit_prepared_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared_batch: PreparedSdmaBatchV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942SdmaErrorV1> {
        self.submit_prepared_batch_with_custody(memory, prepared_batch)
            .map_err(|failure| match failure {
                PreparedSdmaPublicationFailureV1::Recoverable { error, .. }
                | PreparedSdmaPublicationFailureV1::Retained { error, .. } => error,
            })
    }

    // Inline failure custody avoids allocating after native publication has begun.
    #[allow(clippy::result_large_err)]
    fn submit_prepared_batch_with_custody(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared_batch: PreparedSdmaBatchV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, PreparedSdmaPublicationFailureV1> {
        if prepared_batch.queue_id != self.queue_id
            || prepared_batch.requests.len() != prepared_batch.copies.len()
            || prepared_batch.requests.len() != prepared_batch.tickets.len()
        {
            return Err(PreparedSdmaPublicationFailureV1::Recoverable {
                error: Gfx942SdmaErrorV1::Contract("SDMA prepared batch queue or roster"),
                prepared: prepared_batch,
            });
        }
        let publication_plan = match admit_sdma_batch_publication_plan(
            prepared_batch.write,
            prepared_batch.write_end,
            prepared_batch.copies.len(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(PreparedSdmaPublicationFailureV1::Recoverable {
                    error,
                    prepared: prepared_batch,
                });
            }
        };
        let prepared_slots_are_free = prepared_batch.copies.iter().all(|copy| {
            self.records[copy.slot].is_none()
                && self.xgmi_records[copy.slot].is_none()
                && self.persistent_window_slots[copy.slot].is_none()
                && self.generations[copy.slot]
                    .checked_add(1)
                    .filter(|generation| *generation != 0)
                    == Some(copy.generation)
        });
        if !prepared_slots_are_free {
            return Err(PreparedSdmaPublicationFailureV1::Recoverable {
                error: Gfx942SdmaErrorV1::Contract("SDMA prepared batch slot occupancy"),
                prepared: prepared_batch,
            });
        }
        let PreparedSdmaBatchV1 {
            queue_id: _,
            write: _,
            write_end: _,
            copies,
            tickets,
            requests,
            doorbell_failure,
        } = prepared_batch;
        // Every fallible structural check and allocation precedes this point.
        // Retain all buffers before the first mapped write so a later error
        // leaves exact native custody in this poisoned owner.
        for (request, item) in requests.into_iter().zip(&copies) {
            self.generations[item.slot] = item.generation;
            self.records[item.slot] = Some(SdmaCopyRecordV1 {
                directional_persistent: false,
                generation: item.generation,
                completion_value: item.completion_value,
                fence_header: item.packet.fence_header(),
                completion_observed: false,
                source: request.source,
                destination: request.destination,
                copy_bytes: request.copy_bytes,
                source_offset: request.source_offset,
                destination_offset: request.destination_offset,
            });
        }
        self.poisoned = true;
        let publication = (|| {
            let completions = self
                .completions
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?;
            for item in &copies {
                memory.overwrite_mapped_host_visible_subrange_in_current_scope(
                    completions,
                    (item.slot * 8) as u64,
                    &[0; 8],
                )?;
            }
            let ring = self
                .ring
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA ring authority"))?;
            for item in &copies {
                memory.write_sdma_ring_slot_in_current_scope(
                    ring,
                    item.slot as u32,
                    item.packet.bytes(),
                )?;
            }
            memory.publish_sdma_control_write_release_in_current_scope(
                self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing SDMA control authority",
                ))?,
                publication_plan.write,
                publication_plan.write_end,
            )?;
            self.doorbell
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA doorbell"))?
                .store_packet_id_release(publication_plan.write_end)
                .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))
        })();
        match publication {
            Ok(()) => {
                self.poisoned = false;
                Ok(tickets)
            }
            Err(error) => Err(PreparedSdmaPublicationFailureV1::Retained { error, tickets }),
        }
    }

    fn checked_copy_addresses(
        memory: &SharedGttMemorySessionV1,
        source: &Gfx942SdmaBufferV1,
        source_offset: u64,
        destination: &Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<(u64, u64), Gfx942SdmaErrorV1> {
        let source_address =
            source.checked_gpu_subrange(memory, source_offset, u64::from(copy_bytes))?;
        let destination_address =
            destination.checked_gpu_subrange(memory, destination_offset, u64::from(copy_bytes))?;
        if ranges_overlap(
            source_address,
            u64::from(copy_bytes),
            destination_address,
            u64::from(copy_bytes),
        ) {
            return Err(Gfx942SdmaErrorV1::Contract("overlapping SDMA copy ranges"));
        }
        Ok((source_address, destination_address))
    }

    pub(crate) fn poll(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<Gfx942SdmaCopyPollV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let slot = self.validate_ticket(ticket)?;
        memory.check_queue_operational_currentness()?;
        let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
            self.completions
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
            (slot * 8) as u64,
        )?;
        let expected = self.records[slot]
            .as_ref()
            .expect("validated SDMA record")
            .completion_value;
        if observed == 0 {
            memory.check_queue_operational_currentness()?;
            return Ok(Gfx942SdmaCopyPollV1::Pending);
        }
        if observed != i64::from(expected) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract(
                "unexpected SDMA completion value",
            ));
        }
        self.records[slot]
            .as_mut()
            .expect("validated SDMA record")
            .completion_observed = true;
        memory.check_queue_operational_currentness()?;
        let record = self.records[slot].take().expect("observed SDMA record");
        Ok(Gfx942SdmaCopyPollV1::Completed(Gfx942SdmaCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
            source_offset: record.source_offset,
            destination_offset: record.destination_offset,
        }))
    }

    pub(crate) fn wait_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
        wait_profile: SdmaWaitProfileV1,
    ) -> Result<Gfx942SdmaCompletedCopyV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let slot = self.validate_ticket(ticket)?;
        let expected = self.records[slot]
            .as_ref()
            .expect("validated SDMA record")
            .completion_value;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA wait deadline"))?;
        memory.check_queue_operational_currentness()?;
        let mut wait = wait_profile.cursor(deadline);
        if !wait.observe_until_ready(|| {
            let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                self.completions
                    .as_mut()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
                (slot * 8) as u64,
            )?;
            if observed == i64::from(expected) {
                return Ok(true);
            }
            if observed != 0 {
                self.poisoned = true;
                return Err(Gfx942SdmaErrorV1::Contract(
                    "unexpected SDMA completion value",
                ));
            }
            Ok(false)
        })? {
            memory.check_queue_operational_currentness()?;
            return Err(Gfx942SdmaErrorV1::Timeout);
        }
        memory.check_queue_operational_currentness()?;
        let record = self.records[slot].take().expect("completed SDMA record");
        Ok(Gfx942SdmaCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
            source_offset: record.source_offset,
            destination_offset: record.destination_offset,
        })
    }

    fn wait_for_in_current_scope_with_final_currentness(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
    ) -> SingleSdmaWaitInCurrentScopeV1 {
        if let Err(error) = self.require_live() {
            return close_single_sdma_wait_failure_currentness(memory, error);
        }
        let slot = match self.validate_ticket(ticket) {
            Ok(slot) => slot,
            Err(error) => return close_single_sdma_wait_failure_currentness(memory, error),
        };
        let expected = self.records[slot]
            .as_ref()
            .expect("validated SDMA record")
            .completion_value;
        let deadline = match Instant::now().checked_add(timeout) {
            Some(deadline) => deadline,
            None => {
                return close_single_sdma_wait_failure_currentness(
                    memory,
                    Gfx942SdmaErrorV1::Contract("SDMA wait deadline"),
                );
            }
        };
        let mut wait = MonotonicWaitV1::until(deadline);
        loop {
            let observed = match memory.observe_mapped_host_visible_i64_at_in_current_scope(
                self.completions
                    .as_mut()
                    .expect("validated SDMA ticket retains completion arena"),
                (slot * 8) as u64,
            ) {
                Ok(observed) => observed,
                Err(error) => {
                    return close_single_sdma_wait_failure_currentness(memory, error.into());
                }
            };
            if observed == i64::from(expected) {
                break;
            }
            if observed != 0 {
                self.poisoned = true;
                return close_single_sdma_wait_failure_currentness(
                    memory,
                    Gfx942SdmaErrorV1::Contract("unexpected SDMA completion value"),
                );
            }
            if wait.expired() {
                return match memory.check_queue_operational_currentness() {
                    Ok(()) => SingleSdmaWaitInCurrentScopeV1::Timeout,
                    Err(error) => {
                        SingleSdmaWaitInCurrentScopeV1::FinalCurrentnessLost(error.into())
                    }
                };
            }
            wait.pause();
        }
        if let Err(error) = memory.check_queue_operational_currentness() {
            return SingleSdmaWaitInCurrentScopeV1::FinalCurrentnessLost(error.into());
        }
        let record = self.records[slot]
            .take()
            .expect("final-current completed SDMA record");
        SingleSdmaWaitInCurrentScopeV1::Completed(Gfx942SdmaCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
            source_offset: record.source_offset,
            destination_offset: record.destination_offset,
        })
    }

    pub(crate) fn wait_many_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, Gfx942SdmaErrorV1> {
        memory.check_queue_operational_currentness()?;
        let result = self.wait_many_for_in_current_scope(memory, tickets, timeout);
        let post = memory.check_queue_operational_currentness();
        match (result, post) {
            (Ok(completed), Ok(())) => Ok(completed),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error.into()),
        }
    }

    fn poll_xgmi_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<Gfx942XgmiCopyPollV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let slot = self.validate_xgmi_ticket(ticket)?;
        let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
            self.completions
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing XGMI SDMA completion arena",
                ))?,
            (slot * 8) as u64,
        )?;
        let expected = self.xgmi_records[slot]
            .as_ref()
            .expect("validated XGMI SDMA record")
            .completion_value;
        if observed == 0 {
            return Ok(Gfx942XgmiCopyPollV1::Pending(ticket));
        }
        if observed != i64::from(expected) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract(
                "unexpected XGMI SDMA completion value",
            ));
        }
        let record = self.xgmi_records[slot]
            .take()
            .expect("completed XGMI SDMA record");
        Ok(Gfx942XgmiCopyPollV1::Completed(Gfx942XgmiCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
        }))
    }

    fn wait_xgmi_for_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
    ) -> Result<Gfx942XgmiCompletedCopyV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let slot = self.validate_xgmi_ticket(ticket)?;
        let expected = self.xgmi_records[slot]
            .as_ref()
            .expect("validated XGMI SDMA record")
            .completion_value;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract("XGMI SDMA wait deadline"))?;
        let mut wait = MonotonicWaitV1::until(deadline);
        loop {
            let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                self.completions
                    .as_mut()
                    .ok_or(Gfx942SdmaErrorV1::Contract(
                        "missing XGMI SDMA completion arena",
                    ))?,
                (slot * 8) as u64,
            )?;
            if observed == i64::from(expected) {
                break;
            }
            if observed != 0 {
                self.poisoned = true;
                return Err(Gfx942SdmaErrorV1::Contract(
                    "unexpected XGMI SDMA completion value",
                ));
            }
            if wait.expired() {
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            wait.pause();
        }
        let record = self.xgmi_records[slot]
            .take()
            .expect("completed XGMI SDMA record");
        Ok(Gfx942XgmiCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
        })
    }

    fn wait_many_xgmi_for_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942XgmiCompletedCopyV1>, Gfx942SdmaErrorV1> {
        self.require_live()?;
        if tickets.is_empty() || tickets.len() > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            return Err(Gfx942SdmaErrorV1::Contract("XGMI SDMA wait batch size"));
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(tickets.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI SDMA wait roster allocation"))?;
        for ticket in tickets {
            let slot = self.validate_xgmi_ticket(*ticket)?;
            if slots.contains(&slot) {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "duplicate XGMI SDMA wait ticket",
                ));
            }
            slots.push(slot);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract("XGMI SDMA batch wait deadline"))?;
        let mut wait = MonotonicWaitV1::until(deadline);
        let mut ready = vec![false; slots.len()];
        loop {
            let mut all_ready = true;
            for (index, slot) in slots.iter().copied().enumerate() {
                if ready[index] {
                    continue;
                }
                let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                    self.completions
                        .as_mut()
                        .ok_or(Gfx942SdmaErrorV1::Contract(
                            "missing XGMI SDMA completion arena",
                        ))?,
                    (slot * 8) as u64,
                )?;
                let expected = self.xgmi_records[slot]
                    .as_ref()
                    .expect("validated XGMI SDMA batch record")
                    .completion_value;
                if observed == i64::from(expected) {
                    ready[index] = true;
                } else if observed == 0 {
                    all_ready = false;
                } else {
                    self.poisoned = true;
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "unexpected XGMI SDMA batch completion value",
                    ));
                }
            }
            if all_ready && ready.iter().all(|value| *value) {
                break;
            }
            if wait.expired() {
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            wait.pause();
        }
        let mut completed = Vec::new();
        completed
            .try_reserve_exact(slots.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI completion roster allocation"))?;
        for slot in slots {
            let record = self.xgmi_records[slot]
                .take()
                .expect("completed XGMI SDMA batch record");
            completed.push(Gfx942XgmiCompletedCopyV1 {
                source: record.source,
                destination: record.destination,
                copy_bytes: record.copy_bytes,
            });
        }
        Ok(completed)
    }

    fn observe_progress_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        xgmi: bool,
    ) -> Result<Gfx942SdmaQueueProgressObservationV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        if tickets.is_empty() || tickets.len() > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA progress ticket roster"));
        }
        let mut completed_count = 0_u16;
        let mut seen_slots = Vec::new();
        seen_slots
            .try_reserve_exact(tickets.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA progress roster allocation"))?;
        for ticket in tickets {
            let slot = if xgmi {
                self.validate_xgmi_ticket(*ticket)?
            } else {
                self.validate_ticket(*ticket)?
            };
            if seen_slots.contains(&slot) {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "duplicate SDMA progress ticket",
                ));
            }
            seen_slots.push(slot);
            let expected = if xgmi {
                self.xgmi_records[slot]
                    .as_ref()
                    .expect("validated XGMI SDMA progress record")
                    .completion_value
            } else {
                self.records[slot]
                    .as_ref()
                    .expect("validated SDMA progress record")
                    .completion_value
            };
            let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                self.completions
                    .as_mut()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
                (slot * 8) as u64,
            )?;
            if observed == i64::from(expected) {
                completed_count += 1;
            } else if observed != 0 {
                self.poisoned = true;
                return Err(Gfx942SdmaErrorV1::Contract(
                    "unexpected SDMA progress completion value",
                ));
            }
        }
        let (queue_write_bytes, queue_read_bytes) = memory
            .observe_aql_control_counters_in_current_scope(self.control.as_mut().ok_or(
                Gfx942SdmaErrorV1::Contract("missing SDMA control authority"),
            )?)?;
        validate_sdma_write_counter_or_poison(queue_write_bytes, &mut self.poisoned)?;
        if !sdma_ring_delta_is_below_capacity(queue_write_bytes, queue_read_bytes) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        Ok(Gfx942SdmaQueueProgressObservationV1 {
            queue_id: self.queue_id,
            submitted_count: tickets.len() as u16,
            completed_count,
            queue_write_bytes,
            queue_read_bytes,
            host_observed_at: Instant::now(),
        })
    }

    pub(crate) fn wait_many_for_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, Gfx942SdmaErrorV1> {
        self.require_live()?;
        if tickets.is_empty() || tickets.len() > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA wait batch size"));
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(tickets.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA wait roster allocation"))?;
        for ticket in tickets {
            let slot = self.validate_ticket(*ticket)?;
            if slots.contains(&slot) {
                return Err(Gfx942SdmaErrorV1::Contract("duplicate SDMA wait ticket"));
            }
            slots.push(slot);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA batch wait deadline"))?;
        let mut wait = MonotonicWaitV1::until(deadline);
        let mut ready = Vec::new();
        ready
            .try_reserve_exact(slots.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA ready roster allocation"))?;
        ready.resize(slots.len(), false);
        loop {
            let mut all_ready = true;
            for (index, slot) in slots.iter().copied().enumerate() {
                if ready[index] {
                    continue;
                }
                let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                    self.completions
                        .as_mut()
                        .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
                    (slot * 8) as u64,
                )?;
                let expected = self.records[slot]
                    .as_ref()
                    .expect("validated SDMA batch record")
                    .completion_value;
                if observed == i64::from(expected) {
                    ready[index] = true;
                } else if observed == 0 {
                    all_ready = false;
                } else {
                    self.poisoned = true;
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "unexpected SDMA batch completion value",
                    ));
                }
            }
            if all_ready && ready.iter().all(|value| *value) {
                break;
            }
            if wait.expired() {
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            wait.pause();
        }
        let mut completed = Vec::new();
        completed
            .try_reserve_exact(slots.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA completion roster allocation"))?;
        for slot in slots {
            let record = self.records[slot]
                .take()
                .expect("completed SDMA batch record");
            completed.push(Gfx942SdmaCompletedCopyV1 {
                source: record.source,
                destination: record.destination,
                copy_bytes: record.copy_bytes,
                source_offset: record.source_offset,
                destination_offset: record.destination_offset,
            });
        }
        Ok(completed)
    }

    pub(crate) fn destroy_queue(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_live()?;
        if self.records.iter().any(Option::is_some)
            || self.xgmi_records.iter().any(Option::is_some)
            || self.persistent_window_slots.iter().any(Option::is_some)
            || self.persistent_window_records.iter().any(Option::is_some)
        {
            return Err(Gfx942SdmaErrorV1::Pending);
        }
        memory.check_queue_currentness()?;
        let mut args = KfdIoctlDestroyQueueArgs::new(self.queue_id);
        let doorbell_failure = preallocate_doorbell_failure_message()?;
        // Once DESTROY_QUEUE reaches the kernel, failure cannot establish
        // whether the queue still exists. Keep every retained resource under
        // terminal custody instead of permitting a second mutation attempt.
        self.poisoned = true;
        destroy_queue(memory.kfd_fd(), &mut args)
            .map_err(|_| Gfx942SdmaErrorV1::QueueDestroyIndeterminate)?;
        if args != KfdIoctlDestroyQueueArgs::new(self.queue_id) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "kernel changed immutable SDMA DESTROY_QUEUE inputs",
            ));
        }
        self.doorbell
            .take()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA doorbell"))?
            .release()
            .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))?;
        self.destroyed = true;
        memory.check_queue_currentness()?;
        self.poisoned = false;
        Ok(())
    }

    pub(crate) fn release_resources(
        mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        if !self.destroyed
            || self.poisoned
            || self.records.iter().any(Option::is_some)
            || self.xgmi_records.iter().any(Option::is_some)
            || self.persistent_window_slots.iter().any(Option::is_some)
            || self.persistent_window_records.iter().any(Option::is_some)
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "SDMA resources are not releasable",
            ));
        }
        let completions = memory.unmap_from_gpu(
            self.completions
                .take()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
        )?;
        let control = memory.unmap_from_gpu(
            self.control
                .take()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA control"))?
                .into_token(),
        )?;
        let ring = memory.unmap_from_gpu(
            self.ring
                .take()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA ring"))?
                .into_token(),
        )?;
        memory.release(completions)?;
        memory.release(control)?;
        memory.release(ring)?;
        Ok(())
    }

    fn validate_ticket(&self, ticket: Gfx942SdmaCopyTicketV1) -> Result<usize, Gfx942SdmaErrorV1> {
        if !ticket_matches_queue_occurrence(ticket, self.owner, self.queue_id) {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA ticket queue occurrence"));
        }
        let slot = usize::from(ticket.slot);
        let Some(record) = self.records.get(slot).and_then(Option::as_ref) else {
            return Err(Gfx942SdmaErrorV1::Contract("stale SDMA ticket"));
        };
        if record.generation != ticket.generation || record.completion_observed {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA ticket generation"));
        }
        Ok(slot)
    }

    fn observe_validated_slot_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        slot: usize,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        self.require_live()?;
        let expected = self
            .records
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "validated striped SDMA record disappeared",
            ))?
            .completion_value;
        let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
            self.completions
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "missing striped SDMA completion arena",
                ))?,
            (slot * 8) as u64,
        )?;
        if observed == i64::from(expected) {
            return Ok(true);
        }
        if observed == 0 {
            return Ok(false);
        }
        self.poisoned = true;
        Err(Gfx942SdmaErrorV1::Contract(
            "unexpected striped SDMA completion value",
        ))
    }

    fn retire_validated_slot(&mut self, slot: usize) -> Gfx942SdmaCompletedCopyV1 {
        let Some(record) = self.records.get_mut(slot).and_then(Option::take) else {
            std::process::abort();
        };
        Gfx942SdmaCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
            source_offset: record.source_offset,
            destination_offset: record.destination_offset,
        }
    }

    fn validated_slot_remains_present(&self, slot: usize) -> bool {
        !self.destroyed && !self.poisoned && self.records.get(slot).is_some_and(Option::is_some)
    }

    fn validate_xgmi_ticket(
        &self,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<usize, Gfx942SdmaErrorV1> {
        if !ticket_matches_queue_occurrence(ticket, self.owner, self.queue_id) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "XGMI SDMA ticket queue occurrence",
            ));
        }
        let slot = usize::from(ticket.slot);
        let Some(record) = self.xgmi_records.get(slot).and_then(Option::as_ref) else {
            return Err(Gfx942SdmaErrorV1::Contract("stale XGMI SDMA ticket"));
        };
        if record.generation != ticket.generation {
            return Err(Gfx942SdmaErrorV1::Contract("XGMI SDMA ticket generation"));
        }
        Ok(slot)
    }

    fn require_live(&self) -> Result<(), Gfx942SdmaErrorV1> {
        if self.destroyed || self.poisoned {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA queue is not live"));
        }
        Ok(())
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn uncertain_xgmi_ticket(&self) -> Option<Gfx942SdmaCopyTicketV1> {
        self.uncertain_xgmi_ticket
    }
}

/// One directional native XGMI SDMA queue bound to an exact retained route.
#[must_use = "the native XGMI queue must be explicitly destroyed"]
pub struct Gfx942NativeXgmiSdmaQueueV1 {
    route: crate::topology::Gfx942XgmiRouteV1,
    owner: Option<Gfx942SdmaQueueOwnerV1>,
}

// Both variants retain already-live native authority and cannot be boxed safely.
#[allow(clippy::large_enum_variant)]
enum Gfx942NativeXgmiSdmaQueueCreationTerminalV1 {
    Confirmed(Gfx942SdmaQueueOwnerV1),
    Attempted(TerminalGfx942SdmaQueueCreationV1),
}

#[must_use = "terminal XGMI creation custody requires process teardown"]
pub struct Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
    error: Gfx942SdmaErrorV1,
    disposition: Gfx942SdmaQueueSetCreationDispositionV1,
    retained: Option<Gfx942NativeXgmiSdmaQueueCreationTerminalV1>,
}

impl Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        &self.error
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.disposition,
            Gfx942SdmaQueueSetCreationDispositionV1::Terminal
        )
    }

    fn terminal_stage(&self) -> Option<&'static str> {
        match self.retained.as_ref() {
            None if self.is_terminal() => Some("memory-terminal-no-queue-custody"),
            None => None,
            Some(Gfx942NativeXgmiSdmaQueueCreationTerminalV1::Confirmed(owner)) => {
                let _ = owner;
                Some("confirmed-queue-retained")
            }
            Some(Gfx942NativeXgmiSdmaQueueCreationTerminalV1::Attempted(attempted)) => {
                let _ = attempted;
                Some("create-attempt-retained")
            }
        }
    }
}

impl fmt::Debug for Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942NativeXgmiSdmaQueueCreationFailureV1")
            .field("error", &self.error)
            .field("terminal_stage", &self.terminal_stage())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for Gfx942NativeXgmiSdmaQueueCreationFailureV1 {}

impl Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
    fn retryable(error: Gfx942SdmaErrorV1) -> Self {
        Self {
            error,
            disposition: Gfx942SdmaQueueSetCreationDispositionV1::Retryable,
            retained: None,
        }
    }

    fn terminal(
        error: Gfx942SdmaErrorV1,
        retained: Option<Gfx942NativeXgmiSdmaQueueCreationTerminalV1>,
    ) -> Self {
        permanently_poison_process_global_kfd_runtime_gate_v1();
        Self {
            error,
            disposition: Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
            retained,
        }
    }
}

fn quarantine_xgmi_creation_sessions(
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    detail: &'static str,
) {
    let _ = source.quarantine_queue_composition(detail);
    let _ = destination.quarantine_queue_composition(detail);
}

#[derive(Clone, Copy)]
enum XgmiRouteCurrentnessV1 {
    Full,
    BatchScoped,
}

/// A bounded native-XGMI submission scope with one full route check at each edge.
///
/// The scope exclusively borrows both device sessions, so completed mappings
/// cannot be unmapped or released before [`Self::finish`] performs the closing
/// full topology check. Dropping a scope without finishing it fail-closes the
/// queue and both sessions.
#[must_use = "the native XGMI batch must be explicitly finished"]
pub struct Gfx942NativeXgmiSdmaBatchV1<'a> {
    queue: &'a mut Gfx942NativeXgmiSdmaQueueV1,
    source: &'a mut SharedGttMemorySessionV1,
    destination: &'a mut SharedGttMemorySessionV1,
    finished: bool,
}

pub enum Gfx942XgmiCopyFailureV1 {
    Recoverable {
        error: Gfx942SdmaErrorV1,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
    },
    Retained {
        error: Gfx942SdmaErrorV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
    CompletedCurrentnessIndeterminate {
        error: Gfx942SdmaErrorV1,
        completed: Gfx942XgmiCompletedCopyV1,
    },
}

impl Gfx942XgmiCopyFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        match self {
            Self::Recoverable { error, .. }
            | Self::Retained { error, .. }
            | Self::CompletedCurrentnessIndeterminate { error, .. } => error,
        }
    }

    pub const fn retained_ticket(&self) -> Option<Gfx942SdmaCopyTicketV1> {
        match self {
            Self::Retained { ticket, .. } => Some(*ticket),
            Self::Recoverable { .. } | Self::CompletedCurrentnessIndeterminate { .. } => None,
        }
    }

    pub fn into_recoverable_mappings(
        self,
    ) -> Option<(
        Gfx942XgmiMappedDeviceMemoryV1,
        Gfx942XgmiMappedDeviceMemoryV1,
    )> {
        match self {
            Self::Recoverable {
                source,
                destination,
                ..
            } => Some((source, destination)),
            Self::Retained { .. } | Self::CompletedCurrentnessIndeterminate { .. } => None,
        }
    }

    pub fn into_indeterminate_completion(self) -> Option<Gfx942XgmiCompletedCopyV1> {
        match self {
            Self::CompletedCurrentnessIndeterminate { completed, .. } => Some(completed),
            Self::Recoverable { .. } | Self::Retained { .. } => None,
        }
    }
}

pub enum Gfx942XgmiWaitFailureV1 {
    Retained {
        error: Gfx942SdmaErrorV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
    CompletedCurrentnessIndeterminate {
        error: Gfx942SdmaErrorV1,
        completed: Gfx942XgmiCompletedCopyV1,
    },
}

#[must_use = "inspect the error and recover requests or the retained pending tickets"]
pub enum Gfx942XgmiBatchSubmissionFailureV1 {
    Recoverable {
        error: Gfx942SdmaErrorV1,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    },
    Retained {
        error: Gfx942SdmaErrorV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
}

impl Gfx942XgmiBatchSubmissionFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        match self {
            Self::Recoverable { error, .. } | Self::Retained { error, .. } => error,
        }
    }

    pub fn into_recoverable_requests(self) -> Option<Vec<Gfx942XgmiSdmaCopyRequestV1>> {
        match self {
            Self::Recoverable { requests, .. } => Some(requests),
            Self::Retained { .. } => None,
        }
    }

    pub fn into_retained_tickets(self) -> Option<Vec<Gfx942SdmaCopyTicketV1>> {
        match self {
            Self::Retained { tickets, .. } => Some(tickets),
            Self::Recoverable { .. } => None,
        }
    }
}

impl fmt::Display for Gfx942XgmiBatchSubmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl fmt::Debug for Gfx942XgmiBatchSubmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942XgmiBatchSubmissionFailureV1")
            .field("error", self.error())
            .field(
                "recovery",
                &match self {
                    Self::Recoverable { requests, .. } => ("requests", requests.len()),
                    Self::Retained { tickets, .. } => ("tickets", tickets.len()),
                },
            )
            .finish()
    }
}

impl std::error::Error for Gfx942XgmiBatchSubmissionFailureV1 {}

#[must_use = "inspect the error and recover pending tickets or completed mappings"]
pub enum Gfx942XgmiBatchWaitFailureV1 {
    Retained {
        error: Gfx942SdmaErrorV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
    CompletedCurrentnessIndeterminate {
        error: Gfx942SdmaErrorV1,
        completed: Vec<Gfx942XgmiCompletedCopyV1>,
    },
}

impl Gfx942XgmiBatchWaitFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        match self {
            Self::Retained { error, .. }
            | Self::CompletedCurrentnessIndeterminate { error, .. } => error,
        }
    }

    pub fn into_retained_tickets(self) -> Option<Vec<Gfx942SdmaCopyTicketV1>> {
        match self {
            Self::Retained { tickets, .. } => Some(tickets),
            Self::CompletedCurrentnessIndeterminate { .. } => None,
        }
    }

    pub fn into_indeterminate_completions(self) -> Option<Vec<Gfx942XgmiCompletedCopyV1>> {
        match self {
            Self::CompletedCurrentnessIndeterminate { completed, .. } => Some(completed),
            Self::Retained { .. } => None,
        }
    }
}

impl fmt::Display for Gfx942XgmiBatchWaitFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl fmt::Debug for Gfx942XgmiBatchWaitFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942XgmiBatchWaitFailureV1")
            .field("error", self.error())
            .field(
                "recovery",
                &match self {
                    Self::Retained { tickets, .. } => ("tickets", tickets.len()),
                    Self::CompletedCurrentnessIndeterminate { completed, .. } => {
                        ("completed", completed.len())
                    }
                },
            )
            .finish()
    }
}

impl std::error::Error for Gfx942XgmiBatchWaitFailureV1 {}

impl Gfx942XgmiWaitFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        match self {
            Self::Retained { error, .. }
            | Self::CompletedCurrentnessIndeterminate { error, .. } => error,
        }
    }

    pub const fn retained_ticket(&self) -> Option<Gfx942SdmaCopyTicketV1> {
        match self {
            Self::Retained { ticket, .. } => Some(*ticket),
            Self::CompletedCurrentnessIndeterminate { .. } => None,
        }
    }

    pub fn into_indeterminate_completion(self) -> Option<Gfx942XgmiCompletedCopyV1> {
        match self {
            Self::CompletedCurrentnessIndeterminate { completed, .. } => Some(completed),
            Self::Retained { .. } => None,
        }
    }
}

impl fmt::Display for Gfx942XgmiWaitFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl fmt::Debug for Gfx942XgmiWaitFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942XgmiWaitFailureV1")
            .field("error", self.error())
            .field("retained_ticket", &self.retained_ticket())
            .finish_non_exhaustive()
    }
}

impl std::error::Error for Gfx942XgmiWaitFailureV1 {}

// These failures retain move-only native custody inline; boxing here could
// allocate after publication or completion and is therefore inappropriate.
#[allow(clippy::result_large_err)]
fn classify_xgmi_wait_result(
    result: Result<Gfx942XgmiCompletedCopyV1, Gfx942SdmaErrorV1>,
    post: Result<(), Gfx942SdmaErrorV1>,
    ticket: Gfx942SdmaCopyTicketV1,
) -> Result<Gfx942XgmiCompletedCopyV1, Gfx942XgmiWaitFailureV1> {
    match (result, post) {
        (Ok(completed), Ok(())) => Ok(completed),
        (Err(error), Ok(())) => Err(Gfx942XgmiWaitFailureV1::Retained { error, ticket }),
        (Err(_), Err(error)) => Err(Gfx942XgmiWaitFailureV1::Retained { error, ticket }),
        (Ok(completed), Err(error)) => {
            Err(Gfx942XgmiWaitFailureV1::CompletedCurrentnessIndeterminate { error, completed })
        }
    }
}

#[allow(clippy::result_large_err)]
impl Gfx942NativeXgmiSdmaQueueV1 {
    pub fn create(
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        route: crate::topology::Gfx942XgmiRouteV1,
    ) -> Result<Self, Gfx942NativeXgmiSdmaQueueCreationFailureV1> {
        if source.gpu_id() != route.source_gpu_id() {
            return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable(
                Gfx942SdmaErrorV1::Contract(
                    "XGMI queue executing GPU does not match directional route",
                ),
            ));
        }
        let engine =
            admit_kfd_gfx942_sdma_xgmi_engine_mask(route.link().recommended_sdma_engine_id_mask())
                .map_err(|_| {
                    Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable(
                        Gfx942SdmaErrorV1::Contract("XGMI SDMA route engine mask"),
                    )
                })?;
        if engine.value() != route.recommended_engine_id() {
            return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable(
                Gfx942SdmaErrorV1::Contract("XGMI SDMA route engine identity"),
            ));
        }
        let host = prepare_sdma_queue_host_resources()
            .map_err(recover_sdma_owner_preflight_error)
            .map_err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable)?;
        let creation_arm = arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable(Gfx942SdmaErrorV1::Contract(
                "process-global KFD creation gate unavailable",
            ))
        })?;
        if let Err(error) = source.validate_gfx942_xgmi_route_with_peer(destination, route) {
            quarantine_xgmi_creation_sessions(
                source,
                destination,
                "XGMI SDMA creation opening route validation",
            );
            return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
                error.into(),
                None,
            ));
        }
        let owner_key = match source.next_xgmi_sdma_queue_key() {
            Ok(owner_key) => owner_key,
            Err(error) => {
                quarantine_xgmi_creation_sessions(
                    source,
                    destination,
                    "XGMI SDMA creation queue identity failure",
                );
                return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
                    error.into(),
                    None,
                ));
            }
        };
        let owner = match Gfx942SdmaQueueOwnerV1::create_on_xgmi_engine_in_armed_scope(
            source,
            owner_key,
            engine,
            host,
            &creation_arm,
        ) {
            Ok(owner) => owner,
            Err(Gfx942SdmaQueueOwnerCreationFailureV1::RetryableBeforeCreate(error)) => {
                quarantine_xgmi_creation_sessions(
                    source,
                    destination,
                    "XGMI SDMA creation failed after route validation",
                );
                return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
                    error, None,
                ));
            }
            Err(Gfx942SdmaQueueOwnerCreationFailureV1::TerminalAfterMemoryOperation {
                error,
                retained,
            }) => {
                quarantine_xgmi_creation_sessions(
                    source,
                    destination,
                    "XGMI SDMA native queue creation failure",
                );
                return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
                    error,
                    Some(Gfx942NativeXgmiSdmaQueueCreationTerminalV1::Attempted(
                        retained,
                    )),
                ));
            }
        };
        if let Err(error) = source.validate_gfx942_xgmi_route_with_peer(destination, route) {
            quarantine_xgmi_creation_sessions(
                source,
                destination,
                "XGMI SDMA creation closing route validation",
            );
            return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
                error.into(),
                Some(Gfx942NativeXgmiSdmaQueueCreationTerminalV1::Confirmed(
                    owner,
                )),
            ));
        }
        creation_arm.disarm();
        Ok(Self {
            route,
            owner: Some(owner),
        })
    }

    pub const fn route(&self) -> crate::topology::Gfx942XgmiRouteV1 {
        self.route
    }

    pub fn observation(&self) -> Option<Gfx942SdmaQueueObservationV1> {
        self.owner.as_ref().map(Gfx942SdmaQueueOwnerV1::observation)
    }

    /// Begins a bounded measurement/submission scope.
    ///
    /// Exact directional topology is freshly rediscovered here and again by
    /// [`Gfx942NativeXgmiSdmaBatchV1::finish`]. Individual operations inside
    /// that envelope retain exact local route, mapping, ticket, queue-capacity,
    /// and range checks plus a prospective reset fence before and after each
    /// publication/completion, without rediscovering sysfs topology per packet.
    pub fn begin_batch<'a>(
        &'a mut self,
        source: &'a mut SharedGttMemorySessionV1,
        destination: &'a mut SharedGttMemorySessionV1,
    ) -> Result<Gfx942NativeXgmiSdmaBatchV1<'a>, Gfx942SdmaErrorV1> {
        source.validate_gfx942_xgmi_route_with_peer(destination, self.route)?;
        self.owner
            .as_ref()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?
            .require_live()?;
        Ok(Gfx942NativeXgmiSdmaBatchV1 {
            queue: self,
            source,
            destination,
            finished: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        source_offset: u64,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942XgmiCopyFailureV1> {
        self.submit_with_currentness(
            source_session,
            destination_session,
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
            XgmiRouteCurrentnessV1::Full,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_with_currentness(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        source_offset: u64,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        destination_offset: u64,
        copy_bytes: u32,
        currentness: XgmiRouteCurrentnessV1,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942XgmiCopyFailureV1> {
        let mut source = Some(source);
        let mut destination = Some(destination);
        let preflight = (|| {
            Self::validate_route_currentness(
                source_session,
                destination_session,
                self.route,
                currentness,
            )?;
            let source_mapping = source
                .as_ref()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI source mapping"))?;
            let destination_mapping = destination.as_ref().ok_or(Gfx942SdmaErrorV1::Contract(
                "missing XGMI destination mapping",
            ))?;
            if source_mapping.gpu_ids() != self.route.canonical_mapping_gpu_ids()
                || destination_mapping.gpu_ids() != self.route.canonical_mapping_gpu_ids()
            {
                return Err(Gfx942SdmaErrorV1::Contract("XGMI mapping route roster"));
            }
            let source_address = source_session
                .mapped_xgmi_device_memory_facts(source_mapping)?
                .checked_gpu_subrange(source_offset, u64::from(copy_bytes), 1)
                .ok_or(Gfx942SdmaErrorV1::Contract("XGMI source copy range"))?;
            let destination_address = destination_session
                .mapped_xgmi_device_memory_facts(destination_mapping)?
                .checked_gpu_subrange(destination_offset, u64::from(copy_bytes), 1)
                .ok_or(Gfx942SdmaErrorV1::Contract("XGMI destination copy range"))?;
            Ok((source_address, destination_address))
        })();
        let (source_address, destination_address) = match preflight {
            Ok(addresses) => addresses,
            Err(error) => {
                return Err(Gfx942XgmiCopyFailureV1::Recoverable {
                    error,
                    source: source.take().expect("retained XGMI source"),
                    destination: destination.take().expect("retained XGMI destination"),
                });
            }
        };
        let owner = self
            .owner
            .as_mut()
            .ok_or_else(|| Gfx942XgmiCopyFailureV1::Recoverable {
                error: Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"),
                source: source.take().expect("retained XGMI source"),
                destination: destination.take().expect("retained XGMI destination"),
            })?;
        match owner.submit_xgmi(
            source_session,
            &mut source,
            source_address,
            &mut destination,
            destination_address,
            copy_bytes,
        ) {
            Ok(ticket) => match Self::validate_route_currentness(
                source_session,
                destination_session,
                self.route,
                currentness,
            ) {
                Ok(()) => Ok(ticket),
                Err(error) => {
                    owner.poisoned = true;
                    Err(Gfx942XgmiCopyFailureV1::Retained { error, ticket })
                }
            },
            Err(error) => Err(match (source.take(), destination.take()) {
                (Some(source), Some(destination)) => Gfx942XgmiCopyFailureV1::Recoverable {
                    error,
                    source,
                    destination,
                },
                _ => Gfx942XgmiCopyFailureV1::Retained {
                    error,
                    ticket: owner
                        .uncertain_xgmi_ticket()
                        .expect("XGMI queue retained mappings only after assigning a ticket"),
                },
            }),
        }
    }

    /// Prepares all packet images, retains every peer mapping, and publishes
    /// the bounded batch with one write-pointer update and one doorbell store.
    pub fn submit_batch(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942XgmiBatchSubmissionFailureV1> {
        self.submit_batch_with_currentness(
            source_session,
            destination_session,
            requests,
            XgmiRouteCurrentnessV1::Full,
        )
    }

    fn submit_batch_with_currentness(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        currentness: XgmiRouteCurrentnessV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942XgmiBatchSubmissionFailureV1> {
        if let Err(error) = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            currentness,
        ) {
            return Err(Gfx942XgmiBatchSubmissionFailureV1::Recoverable { error, requests });
        }
        let owner = match self.owner.as_mut() {
            Some(owner) => owner,
            None => {
                return Err(Gfx942XgmiBatchSubmissionFailureV1::Recoverable {
                    error: Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"),
                    requests,
                });
            }
        };
        let prepared = match owner.prepare_xgmi_batch_recoverable(
            source_session,
            destination_session,
            self.route,
            requests,
        ) {
            Ok(prepared) => prepared,
            Err((error, requests)) => {
                return Err(Gfx942XgmiBatchSubmissionFailureV1::Recoverable { error, requests });
            }
        };
        let tickets = match owner.submit_prepared_xgmi_batch(source_session, prepared) {
            Ok(tickets) => tickets,
            Err((error, tickets)) => {
                return Err(Gfx942XgmiBatchSubmissionFailureV1::Retained { error, tickets });
            }
        };
        if let Err(error) = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            currentness,
        ) {
            owner.poisoned = true;
            return Err(Gfx942XgmiBatchSubmissionFailureV1::Retained { error, tickets });
        }
        Ok(tickets)
    }

    /// Observes one XGMI completion without blocking or releasing custody early.
    pub fn poll(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<Gfx942XgmiCopyPollV1, Gfx942XgmiCopyFailureV1> {
        if let Err(error) = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        ) {
            self.poison_for_abandoned_batch();
            return Err(Gfx942XgmiCopyFailureV1::Retained { error, ticket });
        }
        let result = match self.owner.as_mut() {
            Some(owner) => owner.poll_xgmi_in_current_scope(source_session, ticket),
            None => Err(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner")),
        };
        let post = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        );
        if let Err(error) = post {
            self.poison_for_abandoned_batch();
            return Err(match result {
                Ok(Gfx942XgmiCopyPollV1::Completed(completed)) => {
                    Gfx942XgmiCopyFailureV1::CompletedCurrentnessIndeterminate { error, completed }
                }
                Ok(Gfx942XgmiCopyPollV1::Pending(pending)) => Gfx942XgmiCopyFailureV1::Retained {
                    error,
                    ticket: pending,
                },
                Err(_) => Gfx942XgmiCopyFailureV1::Retained { error, ticket },
            });
        }
        result.map_err(|error| Gfx942XgmiCopyFailureV1::Retained { error, ticket })
    }

    /// Reports queue counters and per-ticket fence progress at one host instant.
    pub fn observe_progress(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<Gfx942SdmaQueueProgressObservationV1, Gfx942SdmaErrorV1> {
        Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        )?;
        let result = self
            .owner
            .as_mut()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?
            .observe_progress_in_current_scope(source_session, tickets, true);
        let post = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        );
        if post.is_err() {
            self.poison_for_abandoned_batch();
        }
        match (result, post) {
            (Ok(observation), Ok(())) => Ok(observation),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }

    /// Validates the ticket and rejects cancellation without mutating native state.
    ///
    /// Published SDMA packets cannot be retracted safely with the admitted KFD
    /// queue interface. The returned ticket remains valid and must be drained.
    pub fn try_cancel(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<(), (Gfx942SdmaErrorV1, Gfx942SdmaCopyTicketV1)> {
        match self.observe_progress(source_session, destination_session, &[ticket]) {
            Ok(_) => Err((Gfx942SdmaErrorV1::PublishedCancellationUnsupported, ticket)),
            Err(error) => Err((error, ticket)),
        }
    }

    pub fn wait_for(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
    ) -> Result<Gfx942XgmiCompletedCopyV1, Gfx942XgmiWaitFailureV1> {
        self.wait_for_with_currentness(
            source_session,
            destination_session,
            ticket,
            timeout,
            XgmiRouteCurrentnessV1::Full,
        )
    }

    /// Drains every ticket in one bounded batch under an exact route envelope.
    pub fn wait_batch_for(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        timeout: Duration,
    ) -> Result<Vec<Gfx942XgmiCompletedCopyV1>, Gfx942XgmiBatchWaitFailureV1> {
        self.wait_batch_for_with_currentness(
            source_session,
            destination_session,
            tickets,
            timeout,
            XgmiRouteCurrentnessV1::Full,
        )
    }

    fn wait_batch_for_with_currentness(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        timeout: Duration,
        currentness: XgmiRouteCurrentnessV1,
    ) -> Result<Vec<Gfx942XgmiCompletedCopyV1>, Gfx942XgmiBatchWaitFailureV1> {
        if let Err(error) = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            currentness,
        ) {
            self.poison_for_abandoned_batch();
            return Err(Gfx942XgmiBatchWaitFailureV1::Retained { error, tickets });
        }
        let result = match self.owner.as_mut() {
            Some(owner) => {
                owner.wait_many_xgmi_for_in_current_scope(source_session, &tickets, timeout)
            }
            None => Err(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner")),
        };
        let post = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            currentness,
        );
        if post.is_err() {
            self.poison_for_abandoned_batch();
        }
        match (result, post) {
            (Ok(completed), Ok(())) => Ok(completed),
            (Err(error), Ok(())) => Err(Gfx942XgmiBatchWaitFailureV1::Retained { error, tickets }),
            (Err(_), Err(error)) => Err(Gfx942XgmiBatchWaitFailureV1::Retained { error, tickets }),
            (Ok(completed), Err(error)) => Err(
                Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
                    error,
                    completed,
                },
            ),
        }
    }

    fn wait_for_with_currentness(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
        currentness: XgmiRouteCurrentnessV1,
    ) -> Result<Gfx942XgmiCompletedCopyV1, Gfx942XgmiWaitFailureV1> {
        if let Err(error) = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            currentness,
        ) {
            self.poison_for_abandoned_batch();
            return Err(Gfx942XgmiWaitFailureV1::Retained { error, ticket });
        }
        let result = match self.owner.as_mut() {
            Some(owner) => owner.wait_xgmi_for_in_current_scope(source_session, ticket, timeout),
            None => Err(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner")),
        };
        let post = Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            currentness,
        );
        if post.is_err() {
            self.poison_for_abandoned_batch();
        }
        classify_xgmi_wait_result(result, post, ticket)
    }

    fn validate_route_currentness(
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        route: crate::topology::Gfx942XgmiRouteV1,
        currentness: XgmiRouteCurrentnessV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        match currentness {
            XgmiRouteCurrentnessV1::Full => {
                source.validate_gfx942_xgmi_route_with_peer(destination, route)?
            }
            XgmiRouteCurrentnessV1::BatchScoped => {
                source.validate_gfx942_xgmi_publication_with_peer(destination, route)?
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn copy_for(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        source_offset: u64,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        destination_offset: u64,
        copy_bytes: u32,
        timeout: Duration,
    ) -> Result<Gfx942XgmiCompletedCopyV1, Gfx942XgmiCopyFailureV1> {
        let ticket = self.submit(
            source_session,
            destination_session,
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
        )?;
        self.wait_for(source_session, destination_session, ticket, timeout)
            .map_err(|failure| match failure {
                Gfx942XgmiWaitFailureV1::Retained { error, ticket } => {
                    Gfx942XgmiCopyFailureV1::Retained { error, ticket }
                }
                Gfx942XgmiWaitFailureV1::CompletedCurrentnessIndeterminate { error, completed } => {
                    Gfx942XgmiCopyFailureV1::CompletedCurrentnessIndeterminate { error, completed }
                }
            })
    }

    /// Destroys the native queue and releases its retained resources.
    ///
    /// Pre-mutation currentness and pending-work failures leave this queue
    /// intact for inspection or retry. Any failure after `DESTROY_QUEUE` is
    /// issued is terminal and retains the remaining resources.
    pub fn destroy_and_release(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        source.validate_gfx942_xgmi_route_with_peer(destination, self.route)?;
        self.owner
            .as_mut()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?
            .destroy_queue(source)?;
        let owner = self
            .owner
            .take()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?;
        owner.release_resources(source)?;
        source
            .validate_gfx942_xgmi_route_with_peer(destination, self.route)
            .map_err(Into::into)
    }
}

#[allow(clippy::result_large_err)]
impl Gfx942NativeXgmiSdmaBatchV1<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &mut self,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        source_offset: u64,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942XgmiCopyFailureV1> {
        self.queue.submit_with_currentness(
            self.source,
            self.destination,
            source,
            source_offset,
            destination,
            destination_offset,
            copy_bytes,
            XgmiRouteCurrentnessV1::BatchScoped,
        )
    }

    /// Publishes a prepared multi-packet batch with one final doorbell store.
    pub fn submit_batch(
        &mut self,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942XgmiBatchSubmissionFailureV1> {
        self.queue.submit_batch_with_currentness(
            self.source,
            self.destination,
            requests,
            XgmiRouteCurrentnessV1::BatchScoped,
        )
    }

    pub fn wait_for(
        &mut self,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
    ) -> Result<Gfx942XgmiCompletedCopyV1, Gfx942XgmiWaitFailureV1> {
        self.queue.wait_for_with_currentness(
            self.source,
            self.destination,
            ticket,
            timeout,
            XgmiRouteCurrentnessV1::BatchScoped,
        )
    }

    pub fn wait_batch_for(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        timeout: Duration,
    ) -> Result<Vec<Gfx942XgmiCompletedCopyV1>, Gfx942XgmiBatchWaitFailureV1> {
        self.queue.wait_batch_for_with_currentness(
            self.source,
            self.destination,
            tickets,
            timeout,
            XgmiRouteCurrentnessV1::BatchScoped,
        )
    }

    /// Closes the scope with a fresh full directional-topology observation.
    pub fn finish(mut self) -> Result<(), Gfx942SdmaErrorV1> {
        let result = self
            .source
            .validate_gfx942_xgmi_route_with_peer(self.destination, self.queue.route)
            .map_err(Into::into);
        if result.is_err() {
            self.queue.poison_for_abandoned_batch();
        }
        self.finished = true;
        result
    }
}

impl Drop for Gfx942NativeXgmiSdmaBatchV1<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.queue.poison_for_abandoned_batch();
        let _ = self
            .source
            .quarantine_queue_composition("native XGMI batch was not finished");
        let _ = self
            .destination
            .quarantine_queue_composition("native XGMI batch was not finished");
    }
}

impl Gfx942NativeXgmiSdmaQueueV1 {
    fn poison_for_abandoned_batch(&mut self) {
        if let Some(owner) = self.owner.as_mut() {
            owner.poisoned = true;
        }
    }
}

// Creation-terminal custody is deliberately inline and allocation-free.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Gfx942SdmaQueueSetV1 {
    Generic(Vec<Gfx942SdmaQueueOwnerV1>),
    Directional(Vec<Gfx942SdmaQueueOwnerV1>),
    Striped {
        owners: Vec<Gfx942SdmaQueueOwnerV1>,
        next_owner: usize,
    },
    LogicalMuxV2 {
        owners: Vec<Gfx942SdmaQueueOwnerV1>,
        logical_lane_count: u8,
        next_logical_lane: u8,
    },
    /// All known and indeterminate native queues retained after creation failure.
    TerminalRetained {
        primary: Vec<Gfx942SdmaQueueOwnerV1>,
        secondary: Vec<Gfx942SdmaQueueOwnerV1>,
        attempted: Option<TerminalGfx942SdmaQueueCreationV1>,
    },
}

fn append_confirmed_sdma_owners(
    retained: &mut Vec<Gfx942SdmaQueueOwnerV1>,
    mut owners: Vec<Gfx942SdmaQueueOwnerV1>,
) {
    retained.append(&mut owners);
}

fn finish_sdma_owner_creation_failure(
    retained: Vec<Gfx942SdmaQueueOwnerV1>,
    failure: Gfx942SdmaQueueOwnerCreationFailureV1,
    earlier_memory_boundary_crossed: bool,
) -> Gfx942SdmaQueueSetCreationFailureV1 {
    let (error, owner_failure_terminal, attempted) = match failure {
        Gfx942SdmaQueueOwnerCreationFailureV1::RetryableBeforeCreate(error) => (error, false, None),
        Gfx942SdmaQueueOwnerCreationFailureV1::TerminalAfterMemoryOperation { error, retained } => {
            (error, true, Some(retained))
        }
    };
    let disposition = classify_sdma_queue_set_creation_failure(
        earlier_memory_boundary_crossed,
        !retained.is_empty(),
        owner_failure_terminal,
    );
    let retained = if retained.is_empty() && attempted.is_none() {
        None
    } else {
        Some(Gfx942SdmaQueueSetV1::TerminalRetained {
            primary: retained,
            secondary: Vec::new(),
            attempted,
        })
    };
    if disposition == Gfx942SdmaQueueSetCreationDispositionV1::Terminal {
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
    Gfx942SdmaQueueSetCreationFailureV1 {
        error,
        disposition,
        retained,
    }
}

const fn classify_sdma_queue_set_creation_failure(
    earlier_memory_boundary_crossed: bool,
    confirmed_owner_retained: bool,
    owner_failure_terminal: bool,
) -> Gfx942SdmaQueueSetCreationDispositionV1 {
    if earlier_memory_boundary_crossed || confirmed_owner_retained || owner_failure_terminal {
        Gfx942SdmaQueueSetCreationDispositionV1::Terminal
    } else {
        Gfx942SdmaQueueSetCreationDispositionV1::Retryable
    }
}

fn finish_known_terminal_sdma_creation_failure(
    error: Gfx942SdmaErrorV1,
    retained: Vec<Gfx942SdmaQueueOwnerV1>,
) -> Gfx942SdmaQueueSetCreationFailureV1 {
    permanently_poison_process_global_kfd_runtime_gate_v1();
    Gfx942SdmaQueueSetCreationFailureV1 {
        error,
        disposition: Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
        retained: Some(Gfx942SdmaQueueSetV1::TerminalRetained {
            primary: retained,
            secondary: Vec::new(),
            attempted: None,
        }),
    }
}

fn retryable_sdma_queue_set_creation_failure(
    error: Gfx942SdmaErrorV1,
) -> Gfx942SdmaQueueSetCreationFailureV1 {
    Gfx942SdmaQueueSetCreationFailureV1 {
        error,
        disposition: Gfx942SdmaQueueSetCreationDispositionV1::Retryable,
        retained: None,
    }
}

fn terminal_sdma_queue_set_creation_failure(
    error: Gfx942SdmaErrorV1,
) -> Gfx942SdmaQueueSetCreationFailureV1 {
    permanently_poison_process_global_kfd_runtime_gate_v1();
    Gfx942SdmaQueueSetCreationFailureV1 {
        error,
        disposition: Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
        retained: None,
    }
}

impl Gfx942SdmaQueueSetV1 {
    pub(crate) fn observe_directional_retained_request_v1(
        &self,
        expected_owner: QueueKeyV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Option<RetainedDirectionalSdmaObservationV1<'_>> {
        use sha2::{Digest, Sha256};
        use std::hash::{Hash, Hasher};
        struct ObserverHash(Sha256);
        impl Hasher for ObserverHash {
            fn finish(&self) -> u64 {
                unreachable!("full digest only")
            }
            fn write(&mut self, bytes: &[u8]) {
                self.0.update(bytes);
            }
        }
        self.compute_coexistence_endpoints_v1(expected_owner)?;
        let Self::Directional(owners) = self else {
            return None;
        };
        let first = *tickets.first()?;
        let owner = owners
            .iter()
            .find(|owner| owner.queue_id == first.queue_id)?;
        let (source, destination, source_offset, destination_offset, copy_bytes) =
            if tickets.len() == 1 && owner.records.get(usize::from(first.slot))?.is_some() {
                let slot = owner.validate_ticket(first).ok()?;
                let record = owner.records[slot].as_ref()?;
                (
                    &record.source,
                    &record.destination,
                    record.source_offset,
                    record.destination_offset,
                    record.copy_bytes,
                )
            } else {
                let anchor = owner.validate_persistent_window_tickets(tickets).ok()?;
                let request = &owner.persistent_window_records[anchor].as_ref()?.request;
                (
                    &request.source,
                    &request.destination,
                    request.source_offset,
                    request.destination_offset,
                    request.copy_bytes,
                )
            };
        let mut hash = ObserverHash(Sha256::new());
        hash.write(b"fe2o3.r66.retained-directional-sdma.v1\0");
        tickets.len().hash(&mut hash);
        for ticket in tickets {
            ticket.owner.hash(&mut hash);
            ticket.queue_id.hash(&mut hash);
            ticket.slot.hash(&mut hash);
            ticket.generation.hash(&mut hash);
        }
        for buffer in [source, destination] {
            buffer.storage_identity().hash(&mut hash);
            buffer.pool_generation.hash(&mut hash);
            buffer.logical_bytes.hash(&mut hash);
            buffer.physical_bytes().hash(&mut hash);
        }
        (source_offset, destination_offset, copy_bytes).hash(&mut hash);
        Some(RetainedDirectionalSdmaObservationV1 {
            identity: hash.0.finalize().into(),
            source,
            destination,
            source_offset,
            destination_offset,
            copy_bytes,
        })
    }

    /// Visits every retained endpoint only after validating both complete slot
    /// ledgers. This is a borrowed custody observation, not a transferable permit.
    pub(crate) fn compute_coexistence_endpoints_v1(
        &self,
        expected_owner: QueueKeyV1,
    ) -> Option<arrayvec::ArrayVec<fe2o3_runtime_model::R66DeviceStorageV1, 258>> {
        let Self::Directional(owners) = self else {
            return None;
        };
        let [d2h, h2d] = owners.as_slice() else {
            return None;
        };
        if d2h.queue_id == h2d.queue_id
            || d2h.engine_index != Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1)
            || h2d.engine_index != Some(GFX942_SDMA_H2D_ENGINE_INDEX_V1)
        {
            return None;
        }
        let mut endpoints = arrayvec::ArrayVec::new();
        for owner in owners {
            if owner.owner != expected_owner
                || owner.destroyed
                || owner.poisoned
                || owner.ring.is_none()
                || owner.control.is_none()
                || owner.doorbell.is_none()
                || owner.uncertain_xgmi_ticket.is_some()
            {
                return None;
            }
            let session = owner.completions.as_ref()?.storage_identity();
            if !session.same_retained_session_v1(d2h.completions.as_ref()?.storage_identity()) {
                return None;
            }
            owner.collect_compute_coexistence_endpoints_v1(session, &mut endpoints)?;
        }
        Some(endpoints)
    }

    pub(crate) fn compute_coexistence_host_is_current_v1(
        &self,
        owner: QueueKeyV1,
        host: &Gfx942SdmaBufferV1,
    ) -> bool {
        let Self::Directional(owners) = self else {
            return false;
        };
        let Some(session) = owners.first().and_then(|owner| owner.completions.as_ref()) else {
            return false;
        };
        let Gfx942SdmaBufferStorageIdentityV1::Host(identity) = host.storage_identity() else {
            return false;
        };
        host.belongs_to(owner)
            && host.pool_generation != 0
            && host.logical_bytes != 0
            && host.logical_bytes <= host.physical_bytes()
            && identity.same_retained_session_v1(session.storage_identity())
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn create_generic(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        reserved_queue_ids: &[u32],
    ) -> Result<Self, Gfx942SdmaQueueSetCreationFailureV1> {
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(GFX942_SDMA_SINGLE_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "generic SDMA owner roster allocation",
                ))
            })?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(GFX942_SDMA_SINGLE_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "generic SDMA terminal roster allocation",
                ))
            })?;
        let host = prepare_sdma_queue_host_resources()
            .map_err(recover_sdma_owner_preflight_error)
            .map_err(retryable_sdma_queue_set_creation_failure)?;
        let creation_arm = arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                "process-global KFD creation gate unavailable",
            ))
        })?;
        let created = match Gfx942SdmaQueueOwnerV1::create_with_engine_in_armed_scope(
            memory,
            owner,
            None,
            host,
            &creation_arm,
        ) {
            Ok(created) => created,
            Err(failure) => {
                return Err(finish_sdma_owner_creation_failure(retained, failure, true));
            }
        };
        let duplicate = reserved_queue_ids.contains(&created.queue_id);
        owners.push(created);
        if duplicate {
            append_confirmed_sdma_owners(&mut retained, owners);
            return Err(finish_known_terminal_sdma_creation_failure(
                Gfx942SdmaErrorV1::Contract("generic SDMA queue ID collides with a compute queue"),
                retained,
            ));
        }
        creation_arm.disarm();
        Ok(Self::Generic(owners))
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn create_directional(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        reserved_queue_ids: &[u32],
    ) -> Result<Self, Gfx942SdmaQueueSetCreationFailureV1> {
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
        {
            return Err(retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract(
                    "directional SDMA engine inventory is not the exact gfx942 profile",
                ),
            ));
        }
        let mut directional = Vec::new();
        directional
            .try_reserve_exact(GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "directional SDMA owner roster allocation",
                ))
            })?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "directional SDMA terminal roster allocation",
                ))
            })?;
        let d2h_engine =
            admit_kfd_gfx942_sdma_engine_id(GFX942_SDMA_D2H_ENGINE_INDEX_V1).map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "D2H SDMA engine index",
                ))
            })?;
        let h2d_engine =
            admit_kfd_gfx942_sdma_engine_id(GFX942_SDMA_H2D_ENGINE_INDEX_V1).map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "H2D SDMA engine index",
                ))
            })?;
        let mut host_resources =
            prepare_sdma_queue_host_resource_roster(GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1)
                .map_err(recover_sdma_owner_preflight_error)
                .map_err(retryable_sdma_queue_set_creation_failure)?;
        let h2d_host = host_resources
            .pop()
            .unwrap_or_else(|| std::process::abort());
        let d2h_host = host_resources
            .pop()
            .unwrap_or_else(|| std::process::abort());
        let creation_arm = arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                "process-global KFD creation gate unavailable",
            ))
        })?;
        memory
            .check_gfx942_sdma_topology_capability_currentness()
            .map_err(|error| terminal_sdma_queue_set_creation_failure(error.into()))?;
        match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
            memory,
            owner,
            d2h_engine,
            d2h_host,
            &creation_arm,
        ) {
            Ok(created) => directional.push(created),
            Err(failure) => {
                return Err(finish_sdma_owner_creation_failure(retained, failure, true));
            }
        }
        if reserved_queue_ids.contains(&directional[0].queue_id) {
            append_confirmed_sdma_owners(&mut retained, directional);
            return Err(finish_known_terminal_sdma_creation_failure(
                Gfx942SdmaErrorV1::Contract(
                    "directional SDMA queue ID collides with a compute queue",
                ),
                retained,
            ));
        }
        match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
            memory,
            owner,
            h2d_engine,
            h2d_host,
            &creation_arm,
        ) {
            Ok(created) => directional.push(created),
            Err(failure) => {
                append_confirmed_sdma_owners(&mut retained, directional);
                return Err(finish_sdma_owner_creation_failure(retained, failure, true));
            }
        }
        if !directional_queue_ids_are_distinct(directional[0].queue_id, directional[1].queue_id)
            || reserved_queue_ids.contains(&directional[1].queue_id)
        {
            append_confirmed_sdma_owners(&mut retained, directional);
            return Err(finish_known_terminal_sdma_creation_failure(
                Gfx942SdmaErrorV1::Contract("directional SDMA queue ID is not session-wide unique"),
                retained,
            ));
        }
        if let Err(error) = memory.check_gfx942_sdma_topology_capability_currentness() {
            append_confirmed_sdma_owners(&mut retained, directional);
            return Err(finish_known_terminal_sdma_creation_failure(
                error.into(),
                retained,
            ));
        }
        creation_arm.disarm();
        Ok(Self::Directional(directional))
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn create_targeted(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine_index: u32,
        reserved_queue_ids: &[u32],
    ) -> Result<Self, Gfx942SdmaQueueSetCreationFailureV1> {
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
        {
            return Err(retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract(
                    "targeted SDMA engine inventory is not the exact gfx942 profile",
                ),
            ));
        }
        let engine = admit_kfd_gfx942_sdma_engine_id(engine_index).map_err(|_| {
            retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                "targeted SDMA engine index",
            ))
        })?;
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(GFX942_SDMA_SINGLE_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "targeted SDMA owner roster allocation",
                ))
            })?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(GFX942_SDMA_SINGLE_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "targeted SDMA terminal roster allocation",
                ))
            })?;
        let host = prepare_sdma_queue_host_resources()
            .map_err(recover_sdma_owner_preflight_error)
            .map_err(retryable_sdma_queue_set_creation_failure)?;
        let creation_arm = arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                "process-global KFD creation gate unavailable",
            ))
        })?;
        memory
            .check_gfx942_sdma_topology_capability_currentness()
            .map_err(|error| terminal_sdma_queue_set_creation_failure(error.into()))?;
        let created = match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
            memory,
            owner,
            engine,
            host,
            &creation_arm,
        ) {
            Ok(created) => created,
            Err(failure) => {
                return Err(finish_sdma_owner_creation_failure(retained, failure, true));
            }
        };
        let duplicate = reserved_queue_ids.contains(&created.queue_id);
        owners.push(created);
        if duplicate {
            append_confirmed_sdma_owners(&mut retained, owners);
            return Err(finish_known_terminal_sdma_creation_failure(
                Gfx942SdmaErrorV1::Contract("targeted SDMA queue ID collides with a compute queue"),
                retained,
            ));
        }
        if let Err(error) = memory.check_gfx942_sdma_topology_capability_currentness() {
            append_confirmed_sdma_owners(&mut retained, owners);
            return Err(finish_known_terminal_sdma_creation_failure(
                error.into(),
                retained,
            ));
        }
        creation_arm.disarm();
        Ok(Self::Generic(owners))
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn create_striped(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        queue_count: u32,
        reserved_queue_ids: &[u32],
    ) -> Result<(Self, Vec<Gfx942SdmaQueueObservationV1>), Gfx942SdmaQueueSetCreationFailureV1>
    {
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
            || !striped_sdma_queue_count_is_admitted(queue_count)
        {
            return Err(retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract("striped SDMA queue topology or count"),
            ));
        }
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(queue_count as usize)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "striped SDMA owner roster allocation",
                ))
            })?;
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(queue_count as usize)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "striped SDMA observation allocation",
                ))
            })?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(queue_count as usize)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "striped SDMA terminal roster allocation",
                ))
            })?;
        let mut host_resources = prepare_sdma_queue_host_resource_roster(queue_count as usize)
            .map_err(recover_sdma_owner_preflight_error)
            .map_err(retryable_sdma_queue_set_creation_failure)?
            .into_iter();
        let creation_arm = arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                "process-global KFD creation gate unavailable",
            ))
        })?;
        memory
            .check_gfx942_sdma_topology_capability_currentness()
            .map_err(|error| terminal_sdma_queue_set_creation_failure(error.into()))?;
        for queue_index in 0..queue_count {
            let engine_index = queue_index % KFD_GFX942_SDMA_ENGINE_COUNT_V1;
            let engine = match admit_kfd_gfx942_sdma_engine_id(engine_index) {
                Ok(engine) => engine,
                Err(_) => {
                    append_confirmed_sdma_owners(&mut retained, owners);
                    return Err(finish_known_terminal_sdma_creation_failure(
                        Gfx942SdmaErrorV1::Contract("striped SDMA engine index"),
                        retained,
                    ));
                }
            };
            let host = host_resources
                .next()
                .unwrap_or_else(|| std::process::abort());
            let created = match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
                memory,
                owner,
                engine,
                host,
                &creation_arm,
            ) {
                Ok(created) => created,
                Err(failure) => {
                    append_confirmed_sdma_owners(&mut retained, owners);
                    return Err(finish_sdma_owner_creation_failure(retained, failure, true));
                }
            };
            let duplicate = reserved_queue_ids.contains(&created.queue_id)
                || owners
                    .iter()
                    .any(|owner| owner.queue_id == created.queue_id);
            observations.push(created.observation());
            owners.push(created);
            if duplicate {
                append_confirmed_sdma_owners(&mut retained, owners);
                return Err(finish_known_terminal_sdma_creation_failure(
                    Gfx942SdmaErrorV1::Contract("striped SDMA queue ID is not session-wide unique"),
                    retained,
                ));
            }
        }
        if let Err(error) = memory.check_gfx942_sdma_topology_capability_currentness() {
            append_confirmed_sdma_owners(&mut retained, owners);
            return Err(finish_known_terminal_sdma_creation_failure(
                error.into(),
                retained,
            ));
        }
        creation_arm.disarm();
        Ok((
            Self::Striped {
                owners,
                next_owner: 0,
            },
            observations,
        ))
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn create_combined_directional_and_striped(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        striped_queue_count: u32,
        reserved_queue_ids: &[u32],
    ) -> Result<(Self, Self, Gfx942CombinedSdmaCapacityV1), Gfx942SdmaQueueSetCreationFailureV1>
    {
        if !combined_striped_sdma_queue_count_is_admitted(striped_queue_count)
            || queue_ids_have_duplicates(reserved_queue_ids)
        {
            return Err(retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract("combined SDMA striped queue count or reserved IDs"),
            ));
        }
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
        {
            return Err(retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract(
                    "combined SDMA engine inventory is not the exact gfx942 profile",
                ),
            ));
        }

        let mut directional = Vec::new();
        directional
            .try_reserve_exact(GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "combined directional owner roster",
                ))
            })?;
        let mut striped = Vec::new();
        striped
            .try_reserve_exact(striped_queue_count as usize)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "combined striped owner roster",
                ))
            })?;
        let mut striped_observations = Vec::new();
        striped_observations
            .try_reserve_exact(striped_queue_count as usize)
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "combined striped observation roster",
                ))
            })?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(
                GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1 + striped_queue_count as usize,
            )
            .map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "combined SDMA terminal roster",
                ))
            })?;
        let d2h_engine =
            admit_kfd_gfx942_sdma_engine_id(GFX942_SDMA_D2H_ENGINE_INDEX_V1).map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "D2H SDMA engine index",
                ))
            })?;
        let h2d_engine =
            admit_kfd_gfx942_sdma_engine_id(GFX942_SDMA_H2D_ENGINE_INDEX_V1).map_err(|_| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "H2D SDMA engine index",
                ))
            })?;
        let creation_queue_count = GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1
            .checked_add(striped_queue_count as usize)
            .ok_or_else(|| {
                retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                    "combined SDMA host-resource roster count",
                ))
            })?;
        let mut host_resources = prepare_sdma_queue_host_resource_roster(creation_queue_count)
            .map_err(recover_sdma_owner_preflight_error)
            .map_err(retryable_sdma_queue_set_creation_failure)?
            .into_iter();
        let d2h_host = host_resources
            .next()
            .unwrap_or_else(|| std::process::abort());
        let h2d_host = host_resources
            .next()
            .unwrap_or_else(|| std::process::abort());
        let creation_arm = arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            retryable_sdma_queue_set_creation_failure(Gfx942SdmaErrorV1::Contract(
                "process-global KFD creation gate unavailable",
            ))
        })?;
        memory
            .check_gfx942_sdma_topology_capability_currentness()
            .map_err(|error| terminal_sdma_queue_set_creation_failure(error.into()))?;
        match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
            memory,
            owner,
            d2h_engine,
            d2h_host,
            &creation_arm,
        ) {
            Ok(created) => directional.push(created),
            Err(failure) => {
                return Err(finish_sdma_owner_creation_failure(retained, failure, true));
            }
        }
        if reserved_queue_ids.contains(&directional[0].queue_id) {
            append_confirmed_sdma_owners(&mut retained, directional);
            return Err(finish_known_terminal_sdma_creation_failure(
                Gfx942SdmaErrorV1::Contract("combined SDMA queue ID collides with a compute queue"),
                retained,
            ));
        }
        match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
            memory,
            owner,
            h2d_engine,
            h2d_host,
            &creation_arm,
        ) {
            Ok(created) => directional.push(created),
            Err(failure) => {
                append_confirmed_sdma_owners(&mut retained, directional);
                return Err(finish_sdma_owner_creation_failure(retained, failure, true));
            }
        }
        if !directional_queue_ids_are_distinct(
            directional[GFX942_SDMA_D2H_OWNER_SLOT_V1].queue_id,
            directional[GFX942_SDMA_H2D_OWNER_SLOT_V1].queue_id,
        ) || reserved_queue_ids.contains(&directional[GFX942_SDMA_H2D_OWNER_SLOT_V1].queue_id)
        {
            append_confirmed_sdma_owners(&mut retained, directional);
            return Err(finish_known_terminal_sdma_creation_failure(
                Gfx942SdmaErrorV1::Contract(
                    "combined directional SDMA queue ID is not session-wide unique",
                ),
                retained,
            ));
        }
        let directional_observation = Gfx942DirectionalSdmaQueueObservationV1 {
            host_to_device: directional[GFX942_SDMA_H2D_OWNER_SLOT_V1].observation(),
            device_to_host: directional[GFX942_SDMA_D2H_OWNER_SLOT_V1].observation(),
            admitted_engine_count: KFD_GFX942_SDMA_ENGINE_COUNT_V1,
            admitted_queues_per_engine: KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1,
        };
        let directional_ids = [
            directional_observation.host_to_device.queue_id,
            directional_observation.device_to_host.queue_id,
        ];
        for queue_index in 0..striped_queue_count {
            let engine_index = queue_index % KFD_GFX942_SDMA_ENGINE_COUNT_V1;
            let engine = match admit_kfd_gfx942_sdma_engine_id(engine_index) {
                Ok(engine) => engine,
                Err(_) => {
                    append_confirmed_sdma_owners(&mut retained, directional);
                    append_confirmed_sdma_owners(&mut retained, striped);
                    return Err(finish_known_terminal_sdma_creation_failure(
                        Gfx942SdmaErrorV1::Contract("striped SDMA engine index"),
                        retained,
                    ));
                }
            };
            let host = host_resources
                .next()
                .unwrap_or_else(|| std::process::abort());
            let created = match Gfx942SdmaQueueOwnerV1::create_on_engine_in_armed_scope(
                memory,
                owner,
                engine,
                host,
                &creation_arm,
            ) {
                Ok(created) => created,
                Err(failure) => {
                    append_confirmed_sdma_owners(&mut retained, directional);
                    append_confirmed_sdma_owners(&mut retained, striped);
                    return Err(finish_sdma_owner_creation_failure(retained, failure, true));
                }
            };
            let duplicate = reserved_queue_ids.contains(&created.queue_id)
                || directional_ids.contains(&created.queue_id)
                || striped
                    .iter()
                    .any(|existing: &Gfx942SdmaQueueOwnerV1| existing.queue_id == created.queue_id);
            striped_observations.push(created.observation());
            striped.push(created);
            if duplicate {
                append_confirmed_sdma_owners(&mut retained, directional);
                append_confirmed_sdma_owners(&mut retained, striped);
                return Err(finish_known_terminal_sdma_creation_failure(
                    Gfx942SdmaErrorV1::Contract(
                        "combined SDMA queue ID is not session-wide unique",
                    ),
                    retained,
                ));
            }
        }
        if let Err(error) = memory.check_gfx942_sdma_topology_capability_currentness() {
            append_confirmed_sdma_owners(&mut retained, directional);
            append_confirmed_sdma_owners(&mut retained, striped);
            return Err(finish_known_terminal_sdma_creation_failure(
                error.into(),
                retained,
            ));
        }
        creation_arm.disarm();
        Ok((
            Self::Directional(directional),
            Self::Striped {
                owners: striped,
                next_owner: 0,
            },
            Gfx942CombinedSdmaCapacityV1 {
                directional: directional_observation,
                striped: striped_observations,
                maximum_striped_queue_count: GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_V1 as u32,
            },
        ))
    }

    pub(crate) fn generic_observation(&self) -> Option<Gfx942SdmaQueueObservationV1> {
        match self {
            Self::Generic(owners) => owners.first().map(Gfx942SdmaQueueOwnerV1::observation),
            Self::Directional(_)
            | Self::Striped { .. }
            | Self::LogicalMuxV2 { .. }
            | Self::TerminalRetained { .. } => None,
        }
    }

    pub(crate) fn exact_targeted_observation(
        &self,
        engine_index: u32,
    ) -> Option<Gfx942SdmaQueueObservationV1> {
        let Self::Generic(owners) = self else {
            return None;
        };
        let [owner] = owners.as_slice() else {
            return None;
        };
        (owner.engine_index == Some(engine_index)).then(|| owner.observation())
    }

    pub(crate) const fn is_striped(&self) -> bool {
        matches!(self, Self::Striped { .. })
    }

    pub(crate) const fn is_logical_mux_v2(&self) -> bool {
        matches!(self, Self::LogicalMuxV2 { .. })
    }

    pub(crate) fn multi_queue_owners_v1(
        &self,
    ) -> Result<&[Gfx942SdmaQueueOwnerV1], Gfx942SdmaErrorV1> {
        match self {
            Self::Striped { owners, .. } | Self::LogicalMuxV2 { owners, .. } => Ok(owners),
            Self::Generic(_) | Self::Directional(_) | Self::TerminalRetained { .. } => Err(
                Gfx942SdmaErrorV1::Contract("multi-queue operation requires a queue roster"),
            ),
        }
    }

    pub(crate) fn multi_queue_owners_mut_v1(
        &mut self,
    ) -> Result<&mut [Gfx942SdmaQueueOwnerV1], Gfx942SdmaErrorV1> {
        match self {
            Self::Striped { owners, .. } | Self::LogicalMuxV2 { owners, .. } => Ok(owners),
            Self::Generic(_) | Self::Directional(_) | Self::TerminalRetained { .. } => Err(
                Gfx942SdmaErrorV1::Contract("multi-queue operation requires a queue roster"),
            ),
        }
    }

    pub(crate) fn contains_confirmed_queue_id(&self, queue_id: u32) -> bool {
        match self {
            Self::Generic(owners)
            | Self::Directional(owners)
            | Self::Striped { owners, .. }
            | Self::LogicalMuxV2 { owners, .. } => {
                owners.iter().any(|owner| owner.queue_id == queue_id)
            }
            Self::TerminalRetained {
                primary, secondary, ..
            } => primary
                .iter()
                .chain(secondary)
                .any(|owner| owner.queue_id == queue_id),
        }
    }

    pub(crate) fn retain_created_for_terminal(primary: Self, secondary: Option<Self>) -> Self {
        fn confirmed_owners(owner: Gfx942SdmaQueueSetV1) -> Vec<Gfx942SdmaQueueOwnerV1> {
            match owner {
                Gfx942SdmaQueueSetV1::Generic(owners)
                | Gfx942SdmaQueueSetV1::Directional(owners)
                | Gfx942SdmaQueueSetV1::Striped { owners, .. }
                | Gfx942SdmaQueueSetV1::LogicalMuxV2 { owners, .. } => owners,
                Gfx942SdmaQueueSetV1::TerminalRetained { .. } => std::process::abort(),
            }
        }

        let primary = confirmed_owners(primary);
        let secondary = secondary.map_or_else(Vec::new, confirmed_owners);
        permanently_poison_process_global_kfd_runtime_gate_v1();
        Self::TerminalRetained {
            primary,
            secondary,
            attempted: None,
        }
    }

    pub(crate) fn directional_observation(
        &self,
    ) -> Option<Gfx942DirectionalSdmaQueueObservationV1> {
        match self {
            Self::Generic(_)
            | Self::Striped { .. }
            | Self::LogicalMuxV2 { .. }
            | Self::TerminalRetained { .. } => None,
            Self::Directional(owners) => Some(Gfx942DirectionalSdmaQueueObservationV1 {
                host_to_device: owners.get(GFX942_SDMA_H2D_OWNER_SLOT_V1)?.observation(),
                device_to_host: owners.get(GFX942_SDMA_D2H_OWNER_SLOT_V1)?.observation(),
                admitted_engine_count: KFD_GFX942_SDMA_ENGINE_COUNT_V1,
                admitted_queues_per_engine: KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1,
            }),
        }
    }

    pub(crate) fn preflight_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        source: &Gfx942SdmaBufferV1,
        source_offset: u64,
        destination: &Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.owner_for_copy(source.kind(), destination.kind())?
            .preflight_recoverable(
                memory,
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            )
    }

    pub(crate) fn prepare_batch_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<PreparedSdmaBatchV1, (Gfx942SdmaErrorV1, Vec<Gfx942SdmaCopyRequestV1>)> {
        let owner = match self.owner_for_requests(&requests) {
            Ok(owner) => owner,
            Err(error) => return Err((error, requests)),
        };
        owner.prepare_batch_recoverable(memory, requests)
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn prepare_directional_persistent_single_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Result<PreparedSingleSdmaV1, (Gfx942SdmaErrorV1, Gfx942SdmaCopyRequestV1)> {
        if !matches!(self, Self::Directional(_)) {
            return Err((
                Gfx942SdmaErrorV1::Contract("directional persistent single profile"),
                request,
            ));
        }
        let owner = match self.owner_for_copy(request.source.kind(), request.destination.kind()) {
            Ok(owner) => owner,
            Err(error) => return Err((error, request)),
        };
        let mut prepared = owner.prepare_single_recoverable(memory, request)?;
        prepared.directional_persistent = true;
        Ok(prepared)
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn prepare_persistent_window_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Result<PreparedPersistentSdmaWindowV1, (Gfx942SdmaErrorV1, Gfx942SdmaCopyRequestV1)> {
        let owner = match self.owner_for_copy(request.source.kind(), request.destination.kind()) {
            Ok(owner) => owner,
            Err(error) => return Err((error, request)),
        };
        owner.prepare_persistent_window_recoverable(memory, request)
    }

    /// Prepares a local D2D window on the fixed H2D child without widening the
    /// directional H2D/D2H selector used by the ordinary and R22 surfaces.
    #[allow(clippy::result_large_err)]
    pub(crate) fn prepare_same_device_persistent_window_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Result<PreparedPersistentSdmaWindowV1, (Gfx942SdmaErrorV1, Gfx942SdmaCopyRequestV1)> {
        if request.source.kind() != Gfx942SdmaBufferKindV1::DeviceLocal
            || request.destination.kind() != Gfx942SdmaBufferKindV1::DeviceLocal
        {
            return Err((
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA window requires two device buffers",
                ),
                request,
            ));
        }
        let Self::Directional(owners) = self else {
            return Err((
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA window requires a directional queue pair",
                ),
                request,
            ));
        };
        let Some(owner) = owners.get_mut(GFX942_SDMA_H2D_OWNER_SLOT_V1) else {
            return Err((
                Gfx942SdmaErrorV1::Contract(
                    "same-device persistent SDMA window missing fixed child queue",
                ),
                request,
            ));
        };
        owner.prepare_persistent_window_recoverable(memory, request)
    }

    pub(crate) fn submit(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        source: Gfx942SdmaBufferV1,
        source_offset: u64,
        destination: Gfx942SdmaBufferV1,
        destination_offset: u64,
        copy_bytes: u32,
    ) -> Result<Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1> {
        let striped = matches!(self, Self::Striped { .. });
        let result = self
            .owner_for_copy(source.kind(), destination.kind())?
            .submit(
                memory,
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            );
        if striped && result.is_ok() {
            self.advance_striped_owner()?;
        }
        result
    }

    pub(crate) fn submit_prepared_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedSdmaBatchV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942SdmaErrorV1> {
        let ticket = prepared
            .tickets
            .first()
            .copied()
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "SDMA prepared batch ticket roster",
            ))?;
        let striped = matches!(self, Self::Striped { .. });
        let result = self
            .owner_for_ticket(ticket)?
            .submit_prepared_batch(memory, prepared);
        if striped && result.is_ok() {
            self.advance_striped_owner()?;
        }
        result
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_prepared_batch_with_custody(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedSdmaBatchV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, PreparedSdmaPublicationFailureV1> {
        let ticket = match prepared.tickets.first().copied() {
            Some(ticket) => ticket,
            None => {
                return Err(PreparedSdmaPublicationFailureV1::Recoverable {
                    error: Gfx942SdmaErrorV1::Contract("SDMA prepared batch ticket roster"),
                    prepared,
                });
            }
        };
        let owner = match self.owner_for_ticket(ticket) {
            Ok(owner) => owner,
            Err(error) => {
                return Err(PreparedSdmaPublicationFailureV1::Recoverable { error, prepared });
            }
        };
        owner.submit_prepared_batch_with_custody(memory, prepared)
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_prepared_single_with_custody(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedSingleSdmaV1,
    ) -> Result<Gfx942SdmaCopyTicketV1, PreparedSingleSdmaPublicationFailureV1> {
        let ticket = prepared.ticket();
        let owner = match self.owner_for_ticket(ticket) {
            Ok(owner) => owner,
            Err(error) => {
                return Err(PreparedSingleSdmaPublicationFailureV1::Recoverable {
                    error,
                    prepared,
                });
            }
        };
        owner.submit_prepared_single_with_custody(memory, prepared)
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_prepared_persistent_window_with_custody(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: PreparedPersistentSdmaWindowV1,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, PreparedPersistentSdmaWindowPublicationFailureV1> {
        let Some(ticket) = prepared.tickets().first().copied() else {
            return Err(
                PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable {
                    error: Gfx942SdmaErrorV1::Contract(
                        "persistent SDMA window prepared ticket roster",
                    ),
                    prepared,
                },
            );
        };
        let owner = match self.owner_for_ticket(ticket) {
            Ok(owner) => owner,
            Err(error) => {
                return Err(
                    PreparedPersistentSdmaWindowPublicationFailureV1::Recoverable {
                        error,
                        prepared,
                    },
                );
            }
        };
        owner.submit_prepared_persistent_window_with_custody(memory, prepared)
    }

    pub(crate) fn poll_persistent_window(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<PersistentSdmaWindowPollV1, Gfx942SdmaErrorV1> {
        self.owner_for_tickets(tickets)?
            .poll_persistent_window(memory, tickets)
    }

    pub(crate) fn wait_persistent_window_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
        wait_profile: SdmaWaitProfileV1,
    ) -> Result<CompletedPersistentSdmaWindowV1, Gfx942SdmaErrorV1> {
        self.owner_for_tickets(tickets)?.wait_persistent_window_for(
            memory,
            tickets,
            timeout,
            wait_profile,
        )
    }

    pub(crate) fn poll(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<Gfx942SdmaCopyPollV1, Gfx942SdmaErrorV1> {
        self.owner_for_ticket(ticket)?.poll(memory, ticket)
    }

    pub(crate) fn observe_progress(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<Gfx942SdmaQueueProgressObservationV1, Gfx942SdmaErrorV1> {
        self.owner_for_tickets(tickets)?
            .observe_progress_in_current_scope(memory, tickets, false)
    }

    pub(crate) fn validate_published_ticket(
        &mut self,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.owner_for_ticket(ticket)?.validate_ticket(ticket)?;
        Ok(())
    }

    pub(crate) fn wait_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
        wait_profile: SdmaWaitProfileV1,
    ) -> Result<Gfx942SdmaCompletedCopyV1, Gfx942SdmaErrorV1> {
        self.owner_for_ticket(ticket)?
            .wait_for(memory, ticket, timeout, wait_profile)
    }

    pub(crate) fn wait_for_in_current_scope_with_final_currentness(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        ticket: Gfx942SdmaCopyTicketV1,
        timeout: Duration,
    ) -> SingleSdmaWaitInCurrentScopeV1 {
        match self.owner_for_ticket(ticket) {
            Ok(owner) => {
                owner.wait_for_in_current_scope_with_final_currentness(memory, ticket, timeout)
            }
            Err(error) => close_single_sdma_wait_failure_currentness(memory, error),
        }
    }

    pub(crate) fn wait_many_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, Gfx942SdmaErrorV1> {
        self.owner_for_tickets(tickets)?
            .wait_many_for(memory, tickets, timeout)
    }

    pub(crate) fn wait_many_for_in_current_scope(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tickets: &[Gfx942SdmaCopyTicketV1],
        timeout: Duration,
    ) -> Result<Vec<Gfx942SdmaCompletedCopyV1>, Gfx942SdmaErrorV1> {
        self.owner_for_tickets(tickets)?
            .wait_many_for_in_current_scope(memory, tickets, timeout)
    }

    fn owner_for_tickets(
        &mut self,
        tickets: &[Gfx942SdmaCopyTicketV1],
    ) -> Result<&mut Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        let ticket = tickets
            .first()
            .copied()
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA wait batch size"))?;
        let owner = self.owner_for_ticket(ticket)?;
        if tickets
            .iter()
            .any(|ticket| ticket.queue_id != owner.queue_id)
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "mixed directional SDMA wait batch",
            ));
        }
        Ok(owner)
    }

    pub(crate) fn destroy_queue(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let targeted = match self {
            Self::Generic(owners) => owners
                .first()
                .is_some_and(|owner| owner.engine_index.is_some()),
            Self::Directional(_) => true,
            Self::Striped { .. } | Self::LogicalMuxV2 { .. } => true,
            Self::TerminalRetained { .. } => true,
        };
        if targeted {
            memory.check_gfx942_sdma_topology_capability_currentness()?;
        }
        match self {
            Self::Generic(owners) => owners
                .first_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing generic SDMA owner"))?
                .destroy_queue(memory),
            Self::Directional(owners) => {
                if owners.len() != GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1 {
                    return Err(Gfx942SdmaErrorV1::Contract("directional SDMA owner roster"));
                }
                owners[GFX942_SDMA_H2D_OWNER_SLOT_V1].destroy_queue(memory)?;
                owners[GFX942_SDMA_D2H_OWNER_SLOT_V1].destroy_queue(memory)
            }
            Self::Striped { owners, .. } => {
                for owner in owners {
                    owner.destroy_queue(memory)?;
                }
                Ok(())
            }
            Self::LogicalMuxV2 { owners, .. } => {
                if owners.len() != GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2 {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "logical-mux native queue roster",
                    ));
                }
                for owner in owners {
                    owner.destroy_queue(memory)?;
                }
                Ok(())
            }
            Self::TerminalRetained { .. } => Err(Gfx942SdmaErrorV1::Contract(
                "terminal retained SDMA queues require process teardown",
            )),
        }?;
        if targeted {
            memory.check_gfx942_sdma_topology_capability_currentness()?;
        }
        Ok(())
    }

    pub(crate) fn release_resources(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        match self {
            Self::Generic(mut owners) => owners
                .pop()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing generic SDMA owner"))?
                .release_resources(memory),
            Self::Directional(mut owners) => {
                if owners.len() != GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1 {
                    return Err(Gfx942SdmaErrorV1::Contract("directional SDMA owner roster"));
                }
                owners
                    .pop()
                    .expect("checked H2D SDMA owner")
                    .release_resources(memory)?;
                owners
                    .pop()
                    .expect("checked D2H SDMA owner")
                    .release_resources(memory)
            }
            Self::Striped { mut owners, .. } => {
                while let Some(owner) = owners.pop() {
                    owner.release_resources(memory)?;
                }
                Ok(())
            }
            Self::LogicalMuxV2 { mut owners, .. } => {
                while let Some(owner) = owners.pop() {
                    owner.release_resources(memory)?;
                }
                Ok(())
            }
            Self::TerminalRetained { .. } => Err(Gfx942SdmaErrorV1::Contract(
                "terminal retained SDMA resources require process teardown",
            )),
        }
    }

    pub(crate) fn additional_resource_count(&self) -> u8 {
        match self {
            Self::Generic(_) => 3,
            Self::Directional(_) => 6,
            Self::Striped { owners, .. } => {
                u8::try_from(owners.len().saturating_mul(3)).unwrap_or(u8::MAX)
            }
            Self::LogicalMuxV2 { owners, .. } => {
                u8::try_from(owners.len().saturating_mul(3)).unwrap_or(u8::MAX)
            }
            Self::TerminalRetained {
                primary,
                secondary,
                attempted,
            } => {
                let attempted = attempted.as_ref().map_or(
                    0,
                    TerminalGfx942SdmaQueueCreationV1::retained_resource_count,
                );
                u8::try_from(
                    primary
                        .len()
                        .saturating_add(secondary.len())
                        .saturating_mul(3)
                        .saturating_add(attempted),
                )
                .unwrap_or(u8::MAX)
            }
        }
    }

    pub(crate) fn is_poisoned(&self) -> bool {
        match self {
            Self::Generic(owners) => {
                owners.len() != GFX942_SDMA_SINGLE_OWNER_COUNT_V1 || owners[0].is_poisoned()
            }
            Self::Directional(owners) => {
                owners.len() != GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1
                    || owners.iter().any(Gfx942SdmaQueueOwnerV1::is_poisoned)
            }
            Self::Striped { owners, next_owner } => {
                owners.len() < 2
                    || !owners.len().is_multiple_of(2)
                    || *next_owner >= owners.len()
                    || owners.iter().any(Gfx942SdmaQueueOwnerV1::is_poisoned)
            }
            Self::LogicalMuxV2 {
                owners,
                logical_lane_count,
                next_logical_lane,
            } => {
                owners.len() != GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2
                    || !gfx942_sdma_logical_mux_lane_count_is_admitted_v2(u32::from(
                        *logical_lane_count,
                    ))
                    || usize::from(*next_logical_lane) >= usize::from(*logical_lane_count)
                    || owners.iter().any(Gfx942SdmaQueueOwnerV1::is_poisoned)
            }
            Self::TerminalRetained { .. } => true,
        }
    }

    fn owner_for_copy(
        &mut self,
        source: Gfx942SdmaBufferKindV1,
        destination: Gfx942SdmaBufferKindV1,
    ) -> Result<&mut Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        match self {
            Self::Generic(owners) => owners
                .first_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing generic SDMA owner")),
            Self::Directional(owners) => match (source, destination) {
                (
                    Gfx942SdmaBufferKindV1::HostVisibleCoherent,
                    Gfx942SdmaBufferKindV1::DeviceLocal,
                ) => owners
                    .get_mut(GFX942_SDMA_H2D_OWNER_SLOT_V1)
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing H2D SDMA owner")),
                (
                    Gfx942SdmaBufferKindV1::DeviceLocal,
                    Gfx942SdmaBufferKindV1::HostVisibleCoherent,
                ) => owners
                    .get_mut(GFX942_SDMA_D2H_OWNER_SLOT_V1)
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing D2H SDMA owner")),
                _ => Err(Gfx942SdmaErrorV1::Contract(
                    "directional SDMA profile admits only H2D or D2H copies",
                )),
            },
            Self::Striped { owners, next_owner } => owners
                .get_mut(*next_owner)
                .ok_or(Gfx942SdmaErrorV1::Contract("striped SDMA owner cursor")),
            Self::LogicalMuxV2 { .. } => Err(Gfx942SdmaErrorV1::Contract(
                "logical-mux queues require the V2 batch API",
            )),
            Self::TerminalRetained { .. } => Err(Gfx942SdmaErrorV1::Contract(
                "terminal retained SDMA queues require process teardown",
            )),
        }
    }

    fn owner_for_requests(
        &mut self,
        requests: &[Gfx942SdmaCopyRequestV1],
    ) -> Result<&mut Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        if requests.is_empty() {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        self.owner_for_request_kinds(
            requests
                .iter()
                .map(|request| (request.source.kind(), request.destination.kind())),
        )
    }

    fn owner_for_request_kinds(
        &mut self,
        mut kinds: impl Iterator<Item = (Gfx942SdmaBufferKindV1, Gfx942SdmaBufferKindV1)>,
    ) -> Result<&mut Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        let first = kinds.next().ok_or(Gfx942SdmaErrorV1::QueueFull)?;
        if kinds.any(|kinds| kinds != first) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "mixed directional SDMA submission batch",
            ));
        }
        self.owner_for_copy(first.0, first.1)
    }

    fn owner_for_ticket(
        &mut self,
        ticket: Gfx942SdmaCopyTicketV1,
    ) -> Result<&mut Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        match self {
            Self::Generic(owners) => owners
                .iter_mut()
                .find(|owner| owner.queue_id == ticket.queue_id)
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA ticket queue occurrence")),
            Self::Directional(owners) => owners
                .iter_mut()
                .find(|owner| owner.queue_id == ticket.queue_id)
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA ticket queue occurrence")),
            Self::Striped { owners, .. } => owners
                .iter_mut()
                .find(|owner| owner.queue_id == ticket.queue_id)
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA ticket queue occurrence")),
            Self::LogicalMuxV2 { owners, .. } => owners
                .iter_mut()
                .find(|owner| owner.queue_id == ticket.queue_id)
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA ticket queue occurrence")),
            Self::TerminalRetained { .. } => Err(Gfx942SdmaErrorV1::Contract(
                "terminal retained SDMA queues require process teardown",
            )),
        }
    }

    fn advance_striped_owner(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        let Self::Striped { owners, next_owner } = self else {
            return Ok(());
        };
        *next_owner = next_striped_owner(*next_owner, owners.len())?;
        Ok(())
    }
}

#[cfg(test)]
const fn directional_sdma_pair_quiescence_is_admitted(
    device_to_host_quiescent: bool,
    host_to_device_quiescent: bool,
) -> bool {
    device_to_host_quiescent && host_to_device_quiescent
}

pub(crate) fn allocate_host_buffer(
    memory: &mut SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    bytes: usize,
) -> Result<Gfx942SdmaBufferV1, Gfx942SdmaErrorV1> {
    let token = memory.allocate_host_visible_coherent(bytes)?;
    let token = memory.map_to_gpu(token)?;
    Ok(Gfx942SdmaBufferV1 {
        storage: Gfx942SdmaBufferStorageV1::Host(token),
        owner,
        pool_generation: 1,
        logical_bytes: bytes as u64,
        host_content_certificate: None,
    })
}

pub(crate) fn device_buffer_allocation_extents_v1(
    logical_bytes: u64,
    alignment: u64,
) -> Result<(u64, u64), Gfx942SdmaErrorV1> {
    let layout = crate::shared_memory::device_memory_layout(
        logical_bytes,
        alignment,
        fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL,
    )?;
    Ok((logical_bytes, layout.backing_bytes()))
}

pub(crate) fn allocate_device_buffer(
    memory: &mut SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    bytes: u64,
    alignment: u64,
) -> Result<Gfx942SdmaBufferV1, Gfx942SdmaErrorV1> {
    // Promotion needs exact mapped authority for the rounded backing. Copies
    // still use the original logical extent, never the allocation's padding.
    let (logical_bytes, physical_bytes) = device_buffer_allocation_extents_v1(bytes, alignment)?;
    let lease = memory.allocate_gfx942_device_memory(physical_bytes, alignment)?;
    let lease = memory.map_gfx942_device_memory(lease)?;
    Ok(Gfx942SdmaBufferV1 {
        storage: Gfx942SdmaBufferStorageV1::Device(lease),
        owner,
        pool_generation: 1,
        logical_bytes,
        host_content_certificate: None,
    })
}

#[cfg(test)]
pub(crate) fn persistent_sdma_buffers_for_test(
    owner: QueueKeyV1,
    id: u64,
) -> (Gfx942SdmaBufferV1, Gfx942SdmaBufferV1) {
    let device = Gfx942SdmaBufferV1 {
        storage: Gfx942SdmaBufferStorageV1::Device(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(id),
        ),
        owner,
        pool_generation: 1,
        logical_bytes: 4096,
        host_content_certificate: None,
    };
    let host = Gfx942SdmaBufferV1 {
        storage: Gfx942SdmaBufferStorageV1::Host(
            crate::shared_memory::mapped_host_for_persistent_sdma_test(id + 1000, 4096),
        ),
        owner,
        pool_generation: 1,
        logical_bytes: 4096,
        host_content_certificate: None,
    };
    (device, host)
}

#[cfg(test)]
pub(crate) fn persistent_sdma_ticket_for_test(
    owner: QueueKeyV1,
    queue_id: u32,
) -> Gfx942SdmaCopyTicketV1 {
    persistent_sdma_ticket_coordinates_for_test(owner, queue_id, 0, 1)
}

#[cfg(test)]
pub(crate) fn persistent_sdma_ticket_coordinates_for_test(
    owner: QueueKeyV1,
    queue_id: u32,
    slot: u16,
    generation: u32,
) -> Gfx942SdmaCopyTicketV1 {
    Gfx942SdmaCopyTicketV1 {
        owner,
        queue_id,
        slot,
        generation,
    }
}

pub(crate) fn release_buffer(
    memory: &mut SharedGttMemorySessionV1,
    buffer: Gfx942SdmaBufferV1,
) -> Result<(), Gfx942SdmaErrorV1> {
    match buffer.storage {
        Gfx942SdmaBufferStorageV1::Host(token) => {
            let token = memory.unmap_from_gpu(token)?;
            memory.release(token)?;
        }
        Gfx942SdmaBufferStorageV1::Device(lease) => {
            let lease = memory.unmap_gfx942_device_memory(lease)?;
            memory.release_gfx942_device_memory(lease)?;
        }
    }
    Ok(())
}

pub(crate) fn write_host_buffer(
    memory: &mut SharedGttMemorySessionV1,
    buffer: &mut Gfx942SdmaBufferV1,
    offset: u64,
    source: &[u8],
) -> Result<(), Gfx942SdmaErrorV1> {
    if source.is_empty()
        || offset
            .checked_add(source.len() as u64)
            .is_none_or(|end| end > buffer.logical_bytes)
    {
        return Err(Gfx942SdmaErrorV1::Contract("logical host write range"));
    }
    buffer.clear_host_content_certificate();
    match &mut buffer.storage {
        Gfx942SdmaBufferStorageV1::Host(token) => {
            memory.overwrite_mapped_host_visible_subrange(token, offset, source)?;
            Ok(())
        }
        Gfx942SdmaBufferStorageV1::Device(_) => Err(Gfx942SdmaErrorV1::Contract(
            "device-local buffer is not CPU writable",
        )),
    }
}

pub(crate) fn exact_full_host_write_is_authenticatable(
    buffer: &Gfx942SdmaBufferV1,
    source_len: usize,
) -> Result<bool, Gfx942SdmaErrorV1> {
    if source_len == 0 || u64::try_from(source_len).ok() != Some(buffer.requested_bytes()) {
        return Err(Gfx942SdmaErrorV1::Contract(
            "authenticated host write requires one exact full logical extent",
        ));
    }
    Ok(buffer.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
        && buffer.requested_bytes() == buffer.physical_bytes())
}

pub(crate) fn write_full_host_buffer_authenticated(
    memory: &mut SharedGttMemorySessionV1,
    buffer: &mut Gfx942SdmaBufferV1,
    source: &[u8],
) -> Result<[u8; 32], Gfx942SdmaErrorV1> {
    if source.is_empty()
        || u64::try_from(source.len()).ok() != Some(buffer.logical_bytes)
        || buffer.logical_bytes != buffer.physical_bytes()
    {
        return Err(Gfx942SdmaErrorV1::Contract(
            "authenticated host write requires one exact full physical extent",
        ));
    }
    buffer.replace_full_host_content_certificate(|storage| match storage {
        Gfx942SdmaBufferStorageV1::Host(token) => memory
            .overwrite_full_mapped_host_visible_and_sha256(
                token,
                source,
                GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize,
            )
            .map_err(Into::into),
        Gfx942SdmaBufferStorageV1::Device(_) => Err(Gfx942SdmaErrorV1::Contract(
            "device-local buffer is not CPU writable",
        )),
    })
}

pub(crate) fn read_host_buffer(
    memory: &mut SharedGttMemorySessionV1,
    buffer: &Gfx942SdmaBufferV1,
    offset: u64,
    byte_len: u64,
) -> Result<Box<[u8]>, Gfx942SdmaErrorV1> {
    if byte_len == 0
        || offset
            .checked_add(byte_len)
            .is_none_or(|end| end > buffer.logical_bytes)
    {
        return Err(Gfx942SdmaErrorV1::Contract("logical host read range"));
    }
    match &buffer.storage {
        Gfx942SdmaBufferStorageV1::Host(token) => {
            Ok(memory.copy_mapped_host_visible_subrange(token, offset, byte_len)?)
        }
        Gfx942SdmaBufferStorageV1::Device(_) => Err(Gfx942SdmaErrorV1::Contract(
            "device-local buffer is not CPU readable",
        )),
    }
}

pub(crate) fn read_host_buffer_into_v1(
    memory: &mut SharedGttMemorySessionV1,
    buffer: &Gfx942SdmaBufferV1,
    offset: u64,
    destination: &mut [u8],
) -> Result<(), Gfx942SdmaErrorV1> {
    let byte_len = u64::try_from(destination.len())
        .map_err(|_| Gfx942SdmaErrorV1::Contract("logical host read length"))?;
    if byte_len == 0
        || offset
            .checked_add(byte_len)
            .is_none_or(|end| end > buffer.logical_bytes)
    {
        return Err(Gfx942SdmaErrorV1::Contract("logical host read range"));
    }
    match &buffer.storage {
        Gfx942SdmaBufferStorageV1::Host(token) => memory
            .copy_mapped_host_visible_subrange_into(token, offset, destination)
            .map_err(Into::into),
        Gfx942SdmaBufferStorageV1::Device(_) => Err(Gfx942SdmaErrorV1::Contract(
            "device-local buffer is not CPU readable",
        )),
    }
}

fn ranges_overlap(left: u64, left_bytes: u64, right: u64, right_bytes: u64) -> bool {
    let Some(left_end) = left.checked_add(left_bytes) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_bytes) else {
        return true;
    };
    left < right_end && right < left_end
}

fn submission_batch_bytes(count: usize) -> Result<u64, Gfx942SdmaErrorV1> {
    if count == 0 || count > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
        return Err(Gfx942SdmaErrorV1::QueueFull);
    }
    (count as u64)
        .checked_mul(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
        .ok_or(Gfx942SdmaErrorV1::Contract("SDMA batch byte count"))
}

pub(crate) fn persistent_sdma_window_packet_count(
    copy_bytes: u32,
) -> Result<usize, Gfx942SdmaErrorV1> {
    if copy_bytes == 0 {
        return Err(Gfx942SdmaErrorV1::Contract("empty persistent SDMA window"));
    }
    let maximum = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let count = u64::from(copy_bytes).div_ceil(maximum);
    let count = usize::try_from(count)
        .map_err(|_| Gfx942SdmaErrorV1::Contract("persistent SDMA window packet count"))?;
    if count > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
        return Err(Gfx942SdmaErrorV1::QueueFull);
    }
    Ok(count)
}

fn sdma_ring_delta_is_below_capacity(later: u64, earlier: u64) -> bool {
    later
        .checked_sub(earlier)
        .is_some_and(|delta| delta < u64::from(GFX942_SDMA_RING_BYTES_V1))
}

fn batch_ring_slot(write: u64, index: usize) -> Result<usize, Gfx942SdmaErrorV1> {
    validate_sdma_write_counter_alignment(write)?;
    let offset = (index as u64)
        .checked_mul(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
        .and_then(|offset| write.checked_add(offset))
        .ok_or(Gfx942SdmaErrorV1::Contract("SDMA batch slot offset"))?;
    Ok(
        ((offset % u64::from(GFX942_SDMA_RING_BYTES_V1)) / GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
            as usize,
    )
}

fn validate_sdma_write_counter_alignment(write: u64) -> Result<(), Gfx942SdmaErrorV1> {
    if !write.is_multiple_of(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64) {
        return Err(Gfx942SdmaErrorV1::Contract("unaligned SDMA write counter"));
    }
    Ok(())
}

fn validate_sdma_write_counter_or_poison(
    write: u64,
    poisoned: &mut bool,
) -> Result<(), Gfx942SdmaErrorV1> {
    if let Err(error) = validate_sdma_write_counter_alignment(write) {
        *poisoned = true;
        return Err(error);
    }
    Ok(())
}

fn next_sdma_ticket_generation(
    current: u32,
    poisoned: &mut bool,
) -> Result<u32, Gfx942SdmaErrorV1> {
    let Some(next) = current.checked_add(1).filter(|value| *value != 0) else {
        *poisoned = true;
        return Err(Gfx942SdmaErrorV1::Contract(
            "SDMA ticket generation exhausted",
        ));
    };
    Ok(next)
}

fn checked_sdma_write_end(
    write: u64,
    requested: u64,
    poisoned: &mut bool,
) -> Result<u64, Gfx942SdmaErrorV1> {
    let Some(end) = write.checked_add(requested) else {
        *poisoned = true;
        return Err(Gfx942SdmaErrorV1::Contract("SDMA write counter exhausted"));
    };
    Ok(end)
}

const fn directional_queue_ids_are_distinct(
    device_to_host_queue_id: u32,
    host_to_device_queue_id: u32,
) -> bool {
    device_to_host_queue_id != host_to_device_queue_id
}

fn queue_ids_have_duplicates(queue_ids: &[u32]) -> bool {
    queue_ids
        .iter()
        .enumerate()
        .any(|(index, queue_id)| queue_ids[..index].contains(queue_id))
}

fn exact_queue_owner(left: QueueKeyV1, right: QueueKeyV1) -> bool {
    left == right
}

pub(crate) fn ticket_matches_queue_occurrence(
    ticket: Gfx942SdmaCopyTicketV1,
    owner: QueueKeyV1,
    queue_id: u32,
) -> bool {
    ticket.owner == owner && ticket.queue_id == queue_id
}

pub(crate) fn planned_ticket_matches_queue_occurrence(
    ticket: Gfx942SdmaCopyTicketV1,
    owner: QueueKeyV1,
    queue_id: u32,
) -> bool {
    ticket_matches_queue_occurrence(ticket, owner, queue_id)
        && usize::from(ticket.slot) < GFX942_SDMA_RING_SLOT_COUNT_V1
        && ticket.generation != 0
}

fn next_pool_generation(current: u64) -> Result<u64, Gfx942SdmaErrorV1> {
    current.checked_add(1).ok_or(Gfx942SdmaErrorV1::Contract(
        "SDMA buffer pool generation exhausted",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        VmIdV1, VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    #[test]
    fn device_buffer_allocation_extents_preserve_logical_bounds() {
        for (logical, physical) in [
            (1, 4096),
            (4095, 4096),
            (4096, 4096),
            (4097, 8192),
            (1_048_832, 1_052_672),
            (256 << 20, 256 << 20),
        ] {
            assert_eq!(
                device_buffer_allocation_extents_v1(logical, 4096).unwrap(),
                (logical, physical)
            );
            assert!(crate::persistent_directional_sdma::directional_persistent_sdma_extents_are_admitted_v1(logical, physical, 1));
        }
        let (logical, physical) =
            device_buffer_allocation_extents_v1((256 << 20) + 1, 4096).unwrap();
        assert!(!crate::persistent_directional_sdma::directional_persistent_sdma_extents_are_admitted_v1(logical, physical, 1));
        for bytes in [
            0,
            crate::shared_memory::MAX_GFX942_DEVICE_MEMORY_BYTES_V1 + 1,
            u64::MAX,
        ] {
            assert!(device_buffer_allocation_extents_v1(bytes, 4096).is_err());
        }
        for alignment in [0, 3, 8192, u64::MAX] {
            assert!(device_buffer_allocation_extents_v1(4097, alignment).is_err());
        }
    }

    #[test]
    fn device_buffer_allocation_wires_rounded_backing_and_original_logical_extent() {
        let body = include_str!("sdma.rs")
            .split("pub(crate) fn allocate_device_buffer(")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(body.contains("device_buffer_allocation_extents_v1(bytes, alignment)?"));
        assert!(body.contains("allocate_gfx942_device_memory(physical_bytes, alignment)?"));
        assert!(body.contains("logical_bytes,"));
        assert!(!body.contains("logical_bytes: physical_bytes"));
    }

    fn word(packet: &Gfx942SdmaCopySubmissionV1, index: usize) -> u32 {
        let offset = index * 4;
        u32::from_le_bytes(packet.bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn shared_allocation_record_shape_covers_ring_control_and_completions() {
        assert_eq!(
            GFX942_SDMA_SHARED_ALLOCATION_RECORDS_PER_QUEUE_V1,
            1 + 1 + 1
        );
    }

    fn queue_key(physical: u64, queue: u64, generation: u64) -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(physical),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(queue),
            generation: QueueGenerationV1(generation),
        }
    }

    fn compute_coexistence_owner_for_test(engine: u32) -> Gfx942SdmaQueueOwnerV1 {
        Gfx942SdmaQueueOwnerV1 {
            owner: queue_key(7, 11, 13),
            queue_id: engine + 20,
            engine_index: Some(engine),
            ring: None,
            control: None,
            completions: Some(crate::shared_memory::mapped_host_for_persistent_sdma_test(
                900, 4096,
            )),
            doorbell: None,
            records: (0..64).map(|_| None).collect(),
            xgmi_records: (0..64).map(|_| None).collect(),
            persistent_window_slots: (0..64).map(|_| None).collect(),
            persistent_window_records: (0..64).map(|_| None).collect(),
            uncertain_xgmi_ticket: None,
            generations: [0; 64],
            destroyed: false,
            poisoned: false,
        }
    }

    fn compute_coexistence_add_single(owner: &mut Gfx942SdmaQueueOwnerV1, id: u64) {
        let (device, host) = persistent_sdma_buffers_for_test(owner.owner, id);
        let (source, destination) = if owner.engine_index == Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1) {
            (device, host)
        } else {
            (host, device)
        };
        owner.generations[7] = 3;
        owner.records[7] = Some(SdmaCopyRecordV1 {
            directional_persistent: true,
            generation: 3,
            completion_value: 3,
            fence_header: 0,
            completion_observed: false,
            source,
            destination,
            copy_bytes: 4096,
            source_offset: 0,
            destination_offset: 0,
        });
    }

    fn compute_coexistence_add_window(owner: &mut Gfx942SdmaQueueOwnerV1, id: u64) {
        let bytes = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1;
        let device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(
                crate::shared_memory::local_mapping_with_extent_for_persistent_sdma_test(
                    id,
                    u64::from(bytes),
                ),
            ),
            owner.owner,
            1,
            u64::from(bytes),
        );
        let host = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Host(
                crate::shared_memory::mapped_host_for_persistent_sdma_test(
                    id + 1000,
                    bytes as usize,
                ),
            ),
            owner.owner,
            1,
            u64::from(bytes),
        );
        let request = if owner.engine_index == Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1) {
            Gfx942SdmaCopyRequestV1::new(device, 0, host, 0, bytes)
        } else {
            Gfx942SdmaCopyRequestV1::new(host, 0, device, 0, bytes)
        };
        owner.persistent_window_records[63] = Some(PersistentSdmaWindowRecordV1 {
            request,
            packet_count: 2,
        });
        for (slot, generation) in [(63, 4), (0, 5)] {
            owner.generations[slot] = generation;
            owner.persistent_window_slots[slot] = Some(PersistentSdmaWindowSlotV1 {
                anchor_slot: 63,
                generation,
                completion_value: generation,
            });
        }
    }

    fn compute_coexistence_collect_for_test(
        owner: &Gfx942SdmaQueueOwnerV1,
    ) -> Option<arrayvec::ArrayVec<fe2o3_runtime_model::R66DeviceStorageV1, 258>> {
        let mut endpoints = arrayvec::ArrayVec::new();
        owner.collect_compute_coexistence_endpoints_v1(
            owner.completions.as_ref()?.storage_identity(),
            &mut endpoints,
        )?;
        Some(endpoints)
    }

    #[test]
    fn compute_coexistence_scans_both_directions_and_settled_retained_records() {
        for engine in [
            GFX942_SDMA_D2H_ENGINE_INDEX_V1,
            GFX942_SDMA_H2D_ENGINE_INDEX_V1,
        ] {
            let mut owner = compute_coexistence_owner_for_test(engine);
            compute_coexistence_add_single(&mut owner, 41);
            compute_coexistence_add_window(&mut owner, 42);
            owner.records[7].as_mut().unwrap().completion_observed = true;
            let endpoints = compute_coexistence_collect_for_test(&owner).unwrap();
            assert_eq!(endpoints.len(), 2);
            assert!(endpoints.iter().any(|entry| entry.allocation_id == 41));
            assert!(endpoints.iter().any(|entry| entry.allocation_id == 42));
            let domain = fe2o3_runtime_model::R66DeviceDomainV1 {
                physical_device: 7,
                device_generation: 1,
                vm_id: 1,
            };
            let mut compute = endpoints[0];
            compute.allocation_id = 43;
            assert!(fe2o3_runtime_model::r66_device_storage_rosters_disjoint_v1(
                domain,
                &[compute],
                &endpoints
            ));
            compute.allocation_id = 41;
            compute.generation += 1;
            assert!(
                !fe2o3_runtime_model::r66_device_storage_rosters_disjoint_v1(
                    domain,
                    &[compute],
                    &endpoints
                )
            );
            assert!(owner.records[7].is_some());
            assert!(owner.persistent_window_records[63].is_some());
        }
    }

    #[test]
    fn compute_coexistence_rejects_incomplete_stale_and_ordinary_ledgers() {
        for mutation in 0..13 {
            let mut owner = compute_coexistence_owner_for_test(GFX942_SDMA_H2D_ENGINE_INDEX_V1);
            compute_coexistence_add_single(&mut owner, 41);
            compute_coexistence_add_window(&mut owner, 42);
            match mutation {
                0 => {
                    owner.records.pop();
                }
                1 => {
                    owner.persistent_window_slots[0] = None;
                }
                2 => {
                    owner.persistent_window_records[63] = None;
                }
                3 => {
                    owner.persistent_window_slots[0]
                        .as_mut()
                        .unwrap()
                        .generation += 1;
                }
                4 => {
                    owner.persistent_window_slots[0]
                        .as_mut()
                        .unwrap()
                        .anchor_slot = 64;
                }
                5 => {
                    owner.persistent_window_slots[0]
                        .as_mut()
                        .unwrap()
                        .completion_value = 0;
                }
                6 => {
                    owner.persistent_window_records[63]
                        .as_mut()
                        .unwrap()
                        .packet_count = 1;
                }
                7 => {
                    owner.records[7].as_mut().unwrap().directional_persistent = false;
                }
                8 => {
                    owner.records[7].as_mut().unwrap().generation += 1;
                }
                9 => {
                    owner.records[7]
                        .as_mut()
                        .unwrap()
                        .destination
                        .owner
                        .generation
                        .0 += 1;
                }
                10 => {
                    owner.records[7].as_mut().unwrap().source.pool_generation = 0;
                }
                11 => {
                    owner.records[7].as_mut().unwrap().destination_offset = u64::MAX;
                }
                12 => {
                    owner.persistent_window_slots[7] = owner.persistent_window_slots[0];
                }
                _ => unreachable!(),
            }
            assert!(
                compute_coexistence_collect_for_test(&owner).is_none(),
                "mutation {mutation}"
            );
            assert!(
                owner.records[7].is_some(),
                "rejection must retain exact custody"
            );
        }
    }

    #[test]
    fn compute_coexistence_missing_native_authority_never_admits() {
        let owner = queue_key(7, 11, 13);
        let queues = Gfx942SdmaQueueSetV1::Directional(vec![
            compute_coexistence_owner_for_test(GFX942_SDMA_D2H_ENGINE_INDEX_V1),
            compute_coexistence_owner_for_test(GFX942_SDMA_H2D_ENGINE_INDEX_V1),
        ]);
        assert!(queues.compute_coexistence_endpoints_v1(owner).is_none());
        assert!(
            Gfx942SdmaQueueSetV1::Generic(Vec::new())
                .compute_coexistence_endpoints_v1(owner)
                .is_none()
        );
        assert!(
            Gfx942SdmaQueueSetV1::Directional(Vec::new())
                .compute_coexistence_endpoints_v1(owner)
                .is_none()
        );
    }

    #[test]
    fn full_host_content_certificate_is_bound_preserved_and_invalidated() {
        let owner = queue_key(7, 11, 13);
        let digest = [0x5a; 32];
        let (device, mut host) = persistent_sdma_buffers_for_test(owner, 100);
        assert_eq!(host.certified_full_host_content_sha256(4096), None);
        host.certify_full_host_content(digest);
        assert_eq!(host.certified_full_host_content_sha256(4096), Some(digest));
        assert_eq!(host.certified_full_host_content_sha256(4095), None);

        let request = Gfx942SdmaCopyRequestV1::new(host, 0, device, 0, 4096);
        let (mut host, _device) = request.into_buffers();
        assert_eq!(
            host.certified_full_host_content_sha256(4096),
            Some(digest),
            "H2D source construction must preserve exact host evidence"
        );
        host.set_logical_bytes(2048);
        assert_eq!(host.certified_full_host_content_sha256(2048), None);

        let (device, mut host) = persistent_sdma_buffers_for_test(owner, 200);
        host.certify_full_host_content(digest);
        let request = Gfx942SdmaCopyRequestV1::new(device, 0, host, 0, 4096);
        let (_device, host) = request.into_buffers();
        assert_eq!(
            host.certified_full_host_content_sha256(4096),
            None,
            "D2H destination construction must invalidate before publication"
        );

        let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 300);
        host.certify_full_host_content(digest);
        host.advance_pool_generation().unwrap();
        assert_eq!(host.certified_full_host_content_sha256(4096), None);

        let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 400);
        host.certify_full_host_content(digest);
        let (storage, owner, generation, logical_bytes) = host.into_bridge_parts();
        let host = Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, generation, logical_bytes);
        assert_eq!(host.certified_full_host_content_sha256(4096), None);
    }

    #[test]
    fn host_content_certificate_rejects_owner_and_storage_substitution() {
        let owner = queue_key(7, 21, 23);
        let digest = [0xa5; 32];
        let (_device, mut first) = persistent_sdma_buffers_for_test(owner, 500);
        let (_device, mut second) = persistent_sdma_buffers_for_test(owner, 600);
        first.certify_full_host_content(digest);
        core::mem::swap(&mut first.storage, &mut second.storage);
        assert_eq!(first.certified_full_host_content_sha256(4096), None);

        second.certify_full_host_content(digest);
        second.owner = queue_key(7, 22, 23);
        assert_eq!(second.certified_full_host_content_sha256(4096), None);
    }

    #[test]
    fn attempted_full_write_clears_certificate_before_lower_failure() {
        let owner = queue_key(7, 31, 33);
        let digest = [0x3c; 32];
        let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 700);
        host.certify_full_host_content(digest);
        let result = host.replace_full_host_content_certificate(|_| {
            Err(Gfx942SdmaErrorV1::Contract(
                "injected opening or closing currentness failure",
            ))
        });
        assert!(result.is_err());
        assert_eq!(host.certified_full_host_content_sha256(4096), None);
    }

    #[test]
    fn padded_full_logical_write_is_valid_but_not_authenticatable() {
        let owner = queue_key(7, 41, 43);
        let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 800);
        assert!(matches!(
            exact_full_host_write_is_authenticatable(&host, 4096),
            Ok(true)
        ));
        host.set_logical_bytes(2048);
        assert!(matches!(
            exact_full_host_write_is_authenticatable(&host, 2048),
            Ok(false)
        ));
        assert!(exact_full_host_write_is_authenticatable(&host, 2049).is_err());
        assert!(exact_full_host_write_is_authenticatable(&host, 0).is_err());
    }

    #[test]
    fn single_copy_prepare_and_publication_are_stack_sized() {
        let source = include_str!("sdma.rs");
        let prepare = source
            .split("fn prepare_single_recoverable")
            .nth(1)
            .unwrap()
            .split("fn submit_prepared_single_with_custody")
            .next()
            .unwrap();
        let publish = source
            .split("fn submit_prepared_single_with_custody")
            .nth(1)
            .unwrap()
            .split("fn prepare_persistent_window_recoverable")
            .next()
            .unwrap();
        assert!(!prepare.contains("Vec<"));
        assert!(!prepare.contains("vec!["));
        assert!(!prepare.contains("preallocate_doorbell_failure_message"));
        assert!(!publish.contains("Vec<"));
        assert!(!publish.contains("preallocate_doorbell_failure_message"));
        assert!(publish.contains("PreparedSingleSdmaPublicationFailureV1::Retained"));
    }

    #[test]
    fn persistent_window_packet_limits_and_ring_wrap_are_exact() {
        let maximum = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        assert!(persistent_sdma_window_packet_count(0).is_err());
        assert_eq!(persistent_sdma_window_packet_count(1).unwrap(), 1);
        assert_eq!(persistent_sdma_window_packet_count(maximum).unwrap(), 1);
        assert_eq!(persistent_sdma_window_packet_count(maximum + 1).unwrap(), 2);
        let sixty_three =
            u32::try_from(u64::from(maximum) * GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64).unwrap();
        assert_eq!(
            persistent_sdma_window_packet_count(sixty_three).unwrap(),
            GFX942_SDMA_MAX_IN_FLIGHT_V1
        );
        assert!(persistent_sdma_window_packet_count(sixty_three + 1).is_err());

        let write =
            u64::from(GFX942_SDMA_RING_BYTES_V1) - 2 * GFX942_SDMA_SUBMISSION_BYTES_V1 as u64;
        assert_eq!(batch_ring_slot(write, 0).unwrap(), 62);
        assert_eq!(batch_ring_slot(write, 1).unwrap(), 63);
        assert_eq!(batch_ring_slot(write, 2).unwrap(), 0);
    }

    #[test]
    fn persistent_window_publication_is_one_pointer_and_one_doorbell() {
        let source = include_str!("sdma.rs");
        let publication = source
            .split("fn submit_prepared_persistent_window_with_custody")
            .nth(1)
            .unwrap()
            .split("fn validate_persistent_window_tickets")
            .next()
            .unwrap();
        assert_eq!(
            publication
                .matches("publish_sdma_control_write_release_in_current_scope")
                .count(),
            1
        );
        assert_eq!(publication.matches("store_packet_id_release").count(), 1);
        let records = publication
            .find("persistent_window_records[anchor_slot]")
            .unwrap();
        let first_mapped_write = publication
            .find("overwrite_mapped_host_visible_subrange_in_current_scope")
            .unwrap();
        assert!(records < first_mapped_write);
        assert!(publication.contains("for copy in &copies"));
        assert!(publication.contains("PreparedPersistentSdmaWindowPublicationFailureV1::Retained"));
    }

    #[test]
    fn persistent_window_has_exclusive_occupancy_and_whole_window_retirement() {
        let source = include_str!("sdma.rs");
        let owner = source
            .split("pub(crate) struct Gfx942SdmaQueueOwnerV1")
            .nth(1)
            .unwrap()
            .split("impl Gfx942SdmaQueueOwnerV1")
            .next()
            .unwrap();
        assert!(owner.contains("persistent_window_slots"));
        assert!(owner.contains("persistent_window_records"));

        let batch_start = source
            .split("fn observe_batch_start")
            .nth(1)
            .unwrap()
            .split("fn prepare_xgmi_batch")
            .next()
            .unwrap();
        assert!(batch_start.contains("persistent_window_slots"));
        let destroy = source.split("pub(crate) fn destroy_queue").nth(1).unwrap();
        assert!(destroy.contains("persistent_window_slots"));
        assert!(destroy.contains("persistent_window_records"));

        let generic_validation = source
            .split("fn validate_ticket")
            .nth(1)
            .unwrap()
            .split("fn validate_xgmi_ticket")
            .next()
            .unwrap();
        assert!(generic_validation.contains("self.records"));
        assert!(!generic_validation.contains("persistent_window_slots"));

        let completion = source
            .split("fn complete_persistent_window")
            .nth(1)
            .unwrap()
            .split("fn poll_persistent_window")
            .next()
            .unwrap();
        assert!(completion.contains("for ticket in tickets"));
        assert!(completion.contains("persistent_window_records[anchor_slot]"));
    }

    #[test]
    fn pool_owner_and_generation_coordinates_are_exact() {
        let owner = queue_key(7, 3, 1);
        assert!(exact_queue_owner(owner, owner));
        assert!(!exact_queue_owner(owner, queue_key(8, 3, 1)));
        assert!(!exact_queue_owner(owner, queue_key(7, 4, 1)));
        assert!(!exact_queue_owner(owner, queue_key(7, 3, 2)));
        assert_eq!(next_pool_generation(1).unwrap(), 2);
        assert!(next_pool_generation(u64::MAX).is_err());
    }

    #[test]
    fn persistent_compute_requires_both_directional_sdma_ledgers_quiescent() {
        assert!(directional_sdma_pair_quiescence_is_admitted(true, true));
        assert!(!directional_sdma_pair_quiescence_is_admitted(false, true));
        assert!(!directional_sdma_pair_quiescence_is_admitted(true, false));
        assert!(!directional_sdma_pair_quiescence_is_admitted(false, false));
    }

    #[test]
    fn ticket_rejects_native_queue_id_reuse_across_queue_occurrences() {
        let owner = queue_key(7, 3, 1);
        let ticket = Gfx942SdmaCopyTicketV1 {
            owner,
            queue_id: 11,
            slot: 0,
            generation: 1,
        };
        assert!(ticket_matches_queue_occurrence(ticket, owner, 11));
        assert!(!ticket_matches_queue_occurrence(
            ticket,
            queue_key(7, 3, 2),
            11
        ));
        assert!(!ticket_matches_queue_occurrence(ticket, owner, 12));
    }

    #[test]
    fn xgmi_timeout_failure_retains_the_exact_ticket() {
        let ticket = Gfx942SdmaCopyTicketV1 {
            owner: queue_key(7, 4, 1),
            queue_id: 17,
            slot: 3,
            generation: 9,
        };
        let failure = classify_xgmi_wait_result(Err(Gfx942SdmaErrorV1::Timeout), Ok(()), ticket)
            .err()
            .unwrap();
        assert!(matches!(failure.error(), Gfx942SdmaErrorV1::Timeout));
        assert_eq!(failure.retained_ticket(), Some(ticket));
        assert!(failure.into_indeterminate_completion().is_none());
    }

    #[test]
    fn xgmi_post_completion_currentness_failure_retains_both_mappings() {
        let ticket = Gfx942SdmaCopyTicketV1 {
            owner: queue_key(7, 4, 1),
            queue_id: 17,
            slot: 3,
            generation: 9,
        };
        let completed = Gfx942XgmiCompletedCopyV1 {
            source: crate::shared_memory::xgmi_mapping_for_sdma_test(11),
            destination: crate::shared_memory::xgmi_mapping_for_sdma_test(12),
            copy_bytes: 4096,
        };
        let failure = classify_xgmi_wait_result(
            Ok(completed),
            Err(Gfx942SdmaErrorV1::Contract("injected post currentness")),
            ticket,
        )
        .err()
        .unwrap();
        assert!(failure.retained_ticket().is_none());
        let completed = failure.into_indeterminate_completion().unwrap();
        assert_eq!(completed.copy_bytes(), 4096);
        let (source, destination) = completed.into_mappings();
        assert_eq!(source.gpu_ids(), [7, 9]);
        assert_eq!(destination.gpu_ids(), [7, 9]);
        assert!(source.is_fully_mapped());
        assert!(destination.is_fully_mapped());
    }

    #[test]
    fn gfx942_linear_copy_and_fence_match_the_pinned_packet_layout() {
        let packet = Gfx942SdmaCopySubmissionV1::new(
            0x1234_5678_9abc_def0,
            0xfedc_ba98_7654_3210,
            4096,
            0x1111_2222_3333_4448,
            7,
        )
        .unwrap();
        assert_eq!(word(&packet, 0), 1);
        assert_eq!(word(&packet, 1), 4095);
        assert_eq!(word(&packet, 2), 0);
        assert_eq!(word(&packet, 3), 0x9abc_def0);
        assert_eq!(word(&packet, 4), 0x1234_5678);
        assert_eq!(word(&packet, 5), 0x7654_3210);
        assert_eq!(word(&packet, 6), 0xfedc_ba98);
        assert_eq!(word(&packet, 7), 0x0053_0005);
        assert_eq!(word(&packet, 8), 0x3333_4448);
        assert_eq!(word(&packet, 9), 0x1111_2222);
        assert_eq!(word(&packet, 10), 7);
        assert!(packet.bytes[44..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn invalid_sizes_addresses_and_completion_values_fail_closed() {
        assert_eq!(
            Gfx942SdmaCopySubmissionV1::new(0, 1, 1, 1, 1),
            Err(Gfx942SdmaPacketErrorV1::ZeroAddress)
        );
        assert_eq!(
            Gfx942SdmaCopySubmissionV1::new(1, 1, 0, 1, 1),
            Err(Gfx942SdmaPacketErrorV1::EmptyCopy)
        );
        assert_eq!(
            Gfx942SdmaCopySubmissionV1::new(1, 1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1, 1, 1,),
            Err(Gfx942SdmaPacketErrorV1::CopyTooLarge)
        );
        assert_eq!(
            Gfx942SdmaCopySubmissionV1::new(1, 1, 1, 1, 0),
            Err(Gfx942SdmaPacketErrorV1::ZeroCompletionValue)
        );
        assert_eq!(
            Gfx942SdmaCopySubmissionV1::new(u64::MAX, 1, 2, 1, 1),
            Err(Gfx942SdmaPacketErrorV1::AddressOverflow)
        );
    }

    #[test]
    fn overlap_check_is_half_open_and_overflow_fail_closed() {
        assert!(!ranges_overlap(0x1000, 16, 0x1010, 16));
        assert!(ranges_overlap(0x1000, 17, 0x1010, 16));
        assert!(ranges_overlap(u64::MAX, 2, 0, 1));
    }

    #[test]
    fn fixed_batch_geometry_has_unique_slots_across_wrap() {
        assert_eq!(submission_batch_bytes(1).unwrap(), 64);
        assert_eq!(submission_batch_bytes(63).unwrap(), 4032);
        assert!(submission_batch_bytes(0).is_err());
        assert!(submission_batch_bytes(64).is_err());
        assert!(batch_ring_slot(1, 0).is_err());

        let slots = (0..GFX942_SDMA_RING_SLOT_COUNT_V1)
            .map(|index| batch_ring_slot(4032, index).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(slots[0], 63);
        assert_eq!(slots[1], 0);
        let mut unique = slots.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), GFX942_SDMA_RING_SLOT_COUNT_V1);

        assert!(sdma_ring_delta_is_below_capacity(4032, 0));
        assert!(!sdma_ring_delta_is_below_capacity(4096, 0));
        assert!(!sdma_ring_delta_is_below_capacity(63, 64));
    }

    #[test]
    fn batch_publication_plan_has_one_exact_tail_for_fake_mmio() {
        #[derive(Default)]
        struct FakePublication {
            packet_writes: usize,
            write_publications: Vec<(u64, u64)>,
            doorbells: Vec<u64>,
        }

        let plan = admit_sdma_batch_publication_plan(4032, 4032 + 4 * 64, 4).unwrap();
        let mut fake = FakePublication::default();
        fake.packet_writes += plan.packet_count;
        fake.write_publications.push((plan.write, plan.write_end));
        fake.doorbells.push(plan.write_end);
        assert_eq!(fake.packet_writes, 4);
        assert_eq!(fake.write_publications, [(4032, 4288)]);
        assert_eq!(fake.doorbells, [4288]);

        assert!(admit_sdma_batch_publication_plan(1, 65, 1).is_err());
        assert!(admit_sdma_batch_publication_plan(0, 64, 0).is_err());
        assert!(admit_sdma_batch_publication_plan(0, 192, 2).is_err());
        assert!(
            admit_sdma_batch_publication_plan(0, 64 * 64, GFX942_SDMA_RING_SLOT_COUNT_V1).is_err()
        );
    }

    #[test]
    fn striped_queue_count_is_closed_to_balanced_gfx942_inventory() {
        for admitted in [2, 4, 6, 8, 10, 12, 14, 16] {
            assert!(striped_sdma_queue_count_is_admitted(admitted));
        }
        for rejected in [0, 1, 3, 15, 17, u32::MAX] {
            assert!(!striped_sdma_queue_count_is_admitted(rejected));
        }
    }

    #[test]
    fn combined_queue_count_reserves_one_directional_queue_per_engine() {
        assert_eq!(GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_PER_ENGINE_V1, 7);
        assert_eq!(GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_V1, 14);
        for admitted in [2, 4, 6, 8, 10, 12, 14] {
            assert!(combined_striped_sdma_queue_count_is_admitted(admitted));
        }
        for rejected in [0, 1, 3, 15, 16, 17, u32::MAX] {
            assert!(!combined_striped_sdma_queue_count_is_admitted(rejected));
        }
        assert!(
            striped_sdma_queue_count_is_admitted(16),
            "standalone striping retains the full eight queues per engine"
        );
    }

    #[test]
    fn queue_set_creation_disposition_is_independent_of_retained_roster() {
        for earlier_boundary_crossed in [false, true] {
            for confirmed_owner_retained in [false, true] {
                for owner_failure_terminal in [false, true] {
                    let expected = if earlier_boundary_crossed
                        || confirmed_owner_retained
                        || owner_failure_terminal
                    {
                        Gfx942SdmaQueueSetCreationDispositionV1::Terminal
                    } else {
                        Gfx942SdmaQueueSetCreationDispositionV1::Retryable
                    };
                    assert_eq!(
                        classify_sdma_queue_set_creation_failure(
                            earlier_boundary_crossed,
                            confirmed_owner_retained,
                            owner_failure_terminal,
                        ),
                        expected,
                    );
                }
            }
        }

        assert_eq!(
            classify_sdma_queue_set_creation_failure(true, false, false),
            Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
            "a lower preflight failure is terminal after an earlier memory boundary",
        );
    }

    #[test]
    fn xgmi_creation_failure_marks_terminal_without_requiring_queue_roster() {
        let retryable = Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
            error: Gfx942SdmaErrorV1::Contract("injected pure preflight"),
            disposition: Gfx942SdmaQueueSetCreationDispositionV1::Retryable,
            retained: None,
        };
        assert!(!retryable.is_terminal());

        let terminal_without_queue = Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
            error: Gfx942SdmaErrorV1::Contract("injected post-route failure"),
            disposition: Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
            retained: None,
        };
        assert!(terminal_without_queue.is_terminal());
        assert_eq!(
            terminal_without_queue.terminal_stage(),
            Some("memory-terminal-no-queue-custody")
        );
    }

    #[test]
    fn creation_guards_cover_the_first_memory_operation_and_xgmi_route_scope() {
        let source = include_str!("sdma.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let host_preparation = source
            .split("fn prepare_sdma_queue_host_resources()")
            .nth(1)
            .unwrap()
            .split("fn prepare_sdma_queue_host_resource_roster")
            .next()
            .unwrap();
        assert!(host_preparation.contains("preallocate_doorbell_failure_message"));
        let owner_create = source
            .split("fn create_with_engine_in_armed_scope(")
            .nth(1)
            .unwrap()
            .split("pub(crate) const fn observation")
            .next()
            .unwrap();
        assert!(owner_create.contains("&ProcessGlobalKfdRuntimeCreationArmV1"));
        assert!(!owner_create.contains("arm_process_global_kfd_runtime_gate_for_creation_v1"));
        assert!(!owner_create.contains("creation_arm.disarm()"));
        let opening_currentness = owner_create
            .find("memory.check_queue_currentness()")
            .unwrap();
        let promotion = owner_create.find("prepared.into_live").unwrap();
        assert!(opening_currentness < promotion);

        let generic_create = source
            .split("pub(crate) fn create_generic(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn create_directional")
            .next()
            .unwrap();
        let host = generic_create
            .find("prepare_sdma_queue_host_resources")
            .unwrap();
        let arm = generic_create
            .find("arm_process_global_kfd_runtime_gate_for_creation_v1")
            .unwrap();
        let scoped_owner = generic_create
            .find("create_with_engine_in_armed_scope")
            .unwrap();
        let disarm = generic_create.find("creation_arm.disarm()").unwrap();
        assert!(host < arm && arm < scoped_owner && scoped_owner < disarm);

        let xgmi_create = source
            .split("impl Gfx942NativeXgmiSdmaQueueV1 {")
            .nth(1)
            .unwrap()
            .split("pub const fn route")
            .next()
            .unwrap();
        let xgmi_arm = xgmi_create
            .find("arm_process_global_kfd_runtime_gate_for_creation_v1")
            .unwrap();
        let xgmi_host = xgmi_create
            .find("prepare_sdma_queue_host_resources")
            .unwrap();
        let opening_route = xgmi_create
            .find("validate_gfx942_xgmi_route_with_peer")
            .unwrap();
        let scoped_owner = xgmi_create
            .find("create_on_xgmi_engine_in_armed_scope")
            .unwrap();
        let closing_route = xgmi_create
            .rfind("validate_gfx942_xgmi_route_with_peer")
            .unwrap();
        let xgmi_disarm = xgmi_create.find("creation_arm.disarm()").unwrap();
        assert!(
            xgmi_host < xgmi_arm
                && xgmi_arm < opening_route
                && opening_route < scoped_owner
                && scoped_owner < closing_route
                && closing_route < xgmi_disarm
        );
        assert!(
            xgmi_create
                .matches("quarantine_xgmi_creation_sessions")
                .count()
                >= 4
        );
        let inherited_preflight_failure = xgmi_create
            .split("RetryableBeforeCreate(error)")
            .nth(1)
            .unwrap()
            .split("TerminalAfterMemoryOperation")
            .next()
            .unwrap();
        assert!(
            inherited_preflight_failure
                .contains("Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal")
        );
    }

    #[test]
    fn progress_counts_pending_without_device_clock_claim() {
        let observed_at = Instant::now();
        let progress = Gfx942SdmaQueueProgressObservationV1 {
            queue_id: 17,
            submitted_count: 7,
            completed_count: 3,
            queue_write_bytes: 448,
            queue_read_bytes: 192,
            host_observed_at: observed_at,
        };
        assert_eq!(progress.queue_id(), 17);
        assert_eq!(progress.submitted_count(), 7);
        assert_eq!(progress.completed_count(), 3);
        assert_eq!(progress.pending_count(), 4);
        assert_eq!(progress.queue_write_bytes(), 448);
        assert_eq!(progress.queue_read_bytes(), 192);
        assert_eq!(progress.host_observed_at(), observed_at);
    }

    #[test]
    fn directional_queue_ids_must_be_distinct() {
        assert!(directional_queue_ids_are_distinct(7, 8));
        assert!(!directional_queue_ids_are_distinct(7, 7));
    }

    #[test]
    fn xgmi_destroy_borrows_queue_until_native_destroy_succeeds() {
        type DestroyXgmiQueueV1 = fn(
            &mut Gfx942NativeXgmiSdmaQueueV1,
            &mut SharedGttMemorySessionV1,
            &mut SharedGttMemorySessionV1,
        ) -> Result<(), Gfx942SdmaErrorV1>;

        let _: DestroyXgmiQueueV1 = Gfx942NativeXgmiSdmaQueueV1::destroy_and_release;
    }

    #[test]
    fn counter_and_generation_invariant_failures_are_terminal() {
        let mut poisoned = false;
        assert!(validate_sdma_write_counter_or_poison(1, &mut poisoned).is_err());
        assert!(poisoned);

        let mut poisoned = false;
        assert_eq!(next_sdma_ticket_generation(7, &mut poisoned).unwrap(), 8);
        assert!(!poisoned);
        assert!(next_sdma_ticket_generation(u32::MAX, &mut poisoned).is_err());
        assert!(poisoned);

        let mut poisoned = false;
        assert!(checked_sdma_write_end(u64::MAX - 63, 64, &mut poisoned).is_err());
        assert!(poisoned);
    }

    #[test]
    fn persistent_elapsed_spin_profile_is_confined_to_three_public_wait_routes() {
        let live = include_str!("queue_live.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let fixed = include_str!("queue_live/fixed_dispatch.rs");
        fn method<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
            source
                .split(start)
                .nth(1)
                .unwrap()
                .split(end)
                .next()
                .unwrap()
        }
        let elapsed_profile = "SdmaWaitProfileV1::PersistentElapsedSpinFloor";

        for body in [
            method(
                live,
                "pub fn wait_directional_persistent_sdma_copy_for_v1(",
                "pub fn submit_directional_persistent_sdma_window_v1(",
            ),
            method(
                live,
                "pub fn wait_directional_persistent_sdma_window_for_v1(",
                "pub fn submit_same_device_persistent_sdma_window_v1(",
            ),
            method(
                live,
                "pub fn wait_same_device_persistent_sdma_window_for_v1(",
                "pub fn recycle_sdma_buffer(",
            ),
        ] {
            assert_eq!(body.matches(elapsed_profile).count(), 1);
        }
        assert_eq!(live.matches(elapsed_profile).count(), 3);

        let generic_persistent = method(
            live,
            "pub fn wait_persistent_sdma_copy_for_v1(",
            "pub fn promote_sdma_device_buffer_to_directional_persistent_allocation_v1(",
        );
        let ordinary = method(
            live,
            "pub fn wait_sdma_copy_for(",
            "pub fn wait_sdma_copy_batch_for(",
        );
        assert!(generic_persistent.contains("SdmaWaitProfileV1::Default"));
        assert!(ordinary.contains("SdmaWaitProfileV1::Default"));

        for body in [
            generic_persistent,
            ordinary,
            method(
                live,
                "pub fn execute_synchronous_directional_persistent_sdma_copy_for_v1(",
                "pub fn poll_directional_persistent_sdma_copy_v1(",
            ),
            method(
                fixed,
                "pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(",
                "pub fn recycle_directional_persistent_fixed_dispatch_v1(",
            ),
        ] {
            assert!(!body.contains(elapsed_profile));
        }

        let lower = include_str!("sdma.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let fused_synchronous = lower
            .split("fn wait_for_in_current_scope_with_final_currentness(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn wait_many_for(")
            .next()
            .unwrap();
        let xgmi_single = lower
            .split("fn wait_xgmi_for_in_current_scope(")
            .nth(1)
            .unwrap()
            .split("fn wait_many_xgmi_for_in_current_scope(")
            .next()
            .unwrap();
        let xgmi_batch = lower
            .split("fn wait_many_xgmi_for_in_current_scope(")
            .nth(1)
            .unwrap()
            .split("fn observe_progress_in_current_scope(")
            .next()
            .unwrap();
        let ordinary_batch_or_striped = lower
            .split("pub(crate) fn wait_many_for_in_current_scope(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn destroy_queue(")
            .next()
            .unwrap();
        for body in [
            fused_synchronous,
            xgmi_single,
            xgmi_batch,
            ordinary_batch_or_striped,
        ] {
            assert!(!body.contains("SdmaWaitProfileV1"));
            assert!(body.contains("MonotonicWaitV1::until(deadline)"));
        }

        let persistent_compute = fixed
            .split("pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(")
            .nth(1)
            .unwrap()
            .split("pub fn recycle_directional_persistent_fixed_dispatch_v1(")
            .next()
            .unwrap();
        assert!(!persistent_compute.contains(elapsed_profile));
        assert!(persistent_compute.contains("MonotonicWaitV1::until(deadline)"));
    }

    #[test]
    fn profiled_sdma_waits_observe_before_testing_the_deadline() {
        let source = include_str!("sdma.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let window = source
            .split("fn wait_persistent_window_for(")
            .nth(1)
            .unwrap()
            .split("#[allow(clippy::type_complexity)]")
            .next()
            .unwrap();
        assert!(window.contains(".observe_until_ready"));
        assert!(window.contains("self.observe_persistent_window_completion"));

        let single = source
            .split("pub(crate) fn wait_for(")
            .nth(1)
            .unwrap()
            .split("fn wait_for_in_current_scope_with_final_currentness(")
            .next()
            .unwrap();
        assert!(single.contains(".observe_until_ready"));
        assert!(single.contains("let observed ="));

        let wait_helper = include_str!("wait.rs")
            .split("pub(crate) fn observe_until_ready")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(
            wait_helper.find("if observe_ready()?").unwrap()
                < wait_helper.find("if self.expired()").unwrap()
        );

        let profile = source
            .split("impl SdmaWaitProfileV1")
            .nth(1)
            .unwrap()
            .split("/// Frozen claim boundary")
            .next()
            .unwrap();
        assert!(profile.contains("Self::Default => MonotonicWaitV1::until(deadline)"));
        assert!(profile.contains("MonotonicWaitV1::until_with_active_spin_floor"));
    }

    #[test]
    fn persistent_profile_drives_one_zero_deadline_observation_before_timeout() {
        let profile = SdmaWaitProfileV1::PersistentElapsedSpinFloor(Duration::from_nanos(50_000));

        let mut pending = profile.cursor(Instant::now());
        let mut pending_observations = 0;
        assert_eq!(
            pending.observe_until_ready(|| {
                pending_observations += 1;
                Ok::<_, ()>(false)
            }),
            Ok(false)
        );
        assert_eq!(pending_observations, 1);

        let mut ready = profile.cursor(Instant::now());
        let mut ready_observations = 0;
        assert_eq!(
            ready.observe_until_ready(|| {
                ready_observations += 1;
                Ok::<_, ()>(true)
            }),
            Ok(true)
        );
        assert_eq!(ready_observations, 1);
    }

    #[test]
    fn sdma_copy_manifest_digest_is_frozen() {
        assert!(
            GFX942_SDMA_COPY_MANIFEST_V1
                .contains(fe2o3_kfd_uapi::KFD_SDMA_QUEUE_SCHEMA_MANIFEST_SHA256)
        );
        assert!(
            GFX942_SDMA_COPY_MANIFEST_V1
                .contains(crate::topology::GFX942_SDMA_TOPOLOGY_CAPABILITY_MANIFEST_SHA256_V1)
        );
        for required in [
            "nonblocking-whole-submission-poll-observes-every-entry-before-pending",
            "prebinds-one-exact-tail-per-active-shard",
            "tail-ready-with-pending-prefix-fails-terminally",
            "private-lifetime-bound-all-ready-witness",
            "immediate-abort-on-unwind-ordered-custody-move-without-reobservation-or-revalidation",
            "timeout-retains-the-whole-submission-and-retry-starts-a-new-native-wait-epoch",
            "ordinary-wait-uses-a-compile-time-disabled-profile",
            "without-cpu-cost-syscalls-or-counters",
            "explicit-available-unavailable-invalid-status",
            "clear-all-three-cpu-cost-values-without-an-operational-error",
            "closed-diagnostic-spin-budget-recorded-in-every-successful-profile",
            "host-thread-cpu-and-context-switch-measurements-are-not-proof",
            "profiled-host-overhead-is-not-unprofiled-overhead",
            "ordinary-wrapper-always-selects-current",
            "nonprofiled-noncurrent-rejected",
            "closed-roster:current-or-250000ns-or-500000ns-or-1000000ns-or-1500000ns-or-3000000ns",
            "wall-minus-thread-cpu-minus-requested-sleep-is-not-avoidable-latency",
            "spin-changes-host-observation-wakeup-cpu-and-context-switch-behavior-without-device-duration-causality",
            "no-unbounded-input",
            "subsequent-sleep-requests-capped-at-25000ns",
            "actual-scheduler-wake-latency-unbounded",
            "no-completion-authority-from-pause-schedule",
            "striped-tail-fence-premise=each-copy-submission-ends-in-the-exact-mtype-3-system-1-snoop-1-fence",
            "each-bound-owner-engine-index-is-exactly-queue-ordinal-modulo-two",
            "preserves-the-exact-sealed-plan-shards-and-ordered-completion-roster-in-terminal-custody",
            "retryable-no-native-effect-availability-detached-compute-foreign-buffer-and-recoverable-preparation-failures-preserve-inputs-and-do-not-poison",
            "every-striped-submit-poll-or-wait-process-teardown-return-invokes-one-central-terminalizer",
            "striped-wait-timeout-retains-exact-pending-custody-and-does-not-invoke-the-terminalizer",
            "and-invokes-the-same-terminalizer",
            "r46-model-is-not-an-executable-rust-refinement",
            "no-parity-or-striped-tail-wait-speedup-measured-for-this-revision",
            "no-claim-that-spin-closes-the-observed-hip-gap",
        ] {
            assert!(GFX942_SDMA_COPY_MANIFEST_V1.contains(required));
        }
        let digest = Sha256::digest(GFX942_SDMA_COPY_MANIFEST_V1);
        let mut rendered = String::with_capacity(64);
        for byte in digest {
            use core::fmt::Write;
            write!(&mut rendered, "{byte:02x}").unwrap();
        }
        assert_eq!(rendered, GFX942_SDMA_COPY_MANIFEST_SHA256_V1);
    }

    #[test]
    fn logical_mux_manifest_digest_and_exclusions_are_frozen() {
        for required in [
            "two-persistent-native-queues",
            "logical-lanes=closed-counts:2,4,8,14,16",
            "requests:2..126",
            "at-most-63-per-native-shard",
            "each-native-shard-is-stable-filter-of-that-order",
            "one-write-pointer-publication-and-one-doorbell-per-native-queue",
            "bind-two-exact-native-tail-fences",
            "advance-only-after-both-native-publications-closing-currentness-successful-live-model-retake-and-restored-owner-commit",
            "lower-layer-classified-no-native-effect-before-first-publication",
            "facade-caught-rust-unwind-after-entering-live-owner-memory-operation-is-conservatively-post-effect",
            "restoring-owner-helper-resumes-only-to-the-enclosing-facade-catch",
            "nested-retirement-suffix-unwind-after-submission-ownership-move-causes-lower-immediate-abort",
            "without-typed-custody-or-guaranteed-owner-restoration-or-explicit-poison",
            "no-continued-execution-after-either-unwind-class",
            "typed-panic-recovery",
            "hip-stream-independence",
            "formal-refinement",
            "performance",
        ] {
            assert!(GFX942_SDMA_LOGICAL_MUX_MANIFEST_V2.contains(required));
        }
        let digest = Sha256::digest(GFX942_SDMA_LOGICAL_MUX_MANIFEST_V2);
        let mut rendered = String::with_capacity(64);
        for byte in digest {
            use core::fmt::Write;
            write!(&mut rendered, "{byte:02x}").unwrap();
        }
        assert_eq!(rendered, GFX942_SDMA_LOGICAL_MUX_MANIFEST_SHA256_V2);
    }
}
