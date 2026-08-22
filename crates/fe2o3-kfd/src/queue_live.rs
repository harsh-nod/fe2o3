//! Safe, bounded Linux composition for one gfx942 compute-AQL queue.

use core::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

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
    Gfx942CompletedBatchV1, Gfx942CompletionBatchV1, Gfx942CompletionErrorV1,
    Gfx942CompletionPollV1, Gfx942CompletionRecycleObservationV1, NativeCompletionSignalBackendV1,
    initialize_pending_completion_signal_arena,
};
use super::dispatch_binding::{
    DeviceDataAllocationInputV1, DispatchGeometryV1, DispatchResourceOwnerV1,
    Gfx942CompletedDispatchBatchV1, Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1,
    Gfx942DispatchPollV1, ReturnedDispatchDataV1, TypedKernargImageV1, prepare_dispatch_resources,
    unwrap_completed, unwrap_published, validate_fixed_batch_ring, wrap_completed, wrap_poll,
    wrap_published,
};
use super::submit::{
    NativeAqlSubmissionBackendV1, NativeAqlSubmissionErrorV1, NativeAqlSubmissionOwnerV1,
    initialize_control_atomics, initialize_invalid_ring,
};
use super::*;
use crate::queue_linux::{
    LinuxCwsrShadowPagesV1, LinuxCwsrShadowsReadyForReleaseV1, LinuxDoorbellErrorV1,
    LinuxDoorbellSliceV1, LinuxKfdRuntimeEnabledV1, LinuxQueueExceptionEventV1,
    QueueExceptionWaitObservationV1,
};
use crate::shared_memory::{
    AqlCompletionSignalResourceRoleV1, AqlContextSaveResourceRoleV1, AqlControlResourceRoleV1,
    AqlEndOfPipeResourceRoleV1, AqlQueueGttV1, AqlRingResourceRoleV1, ExecutableGttV1,
    GttGpuAccessibleExecutableV1, GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1,
    SharedGttMemorySessionV1, SharedGttQueueResourceAuthorityV1,
};
use crate::{
    CheckedGfx942XnackMinusDevice, GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
    Gfx942AqlQueueResourcePlanV1, Gfx942QueueResourcePlanningError, MemorySessionError,
    SHARED_GTT_MEMORY_PROFILE_SHA256_V1, plan_gfx942_aql_queue_resources,
};
use fe2o3_aql::{
    AqlKernelDispatchPacketV1, AqlPreparedKernelDispatchBatchV2, AqlPreparedKernelDispatchV1,
};

const CONTROL_BYTES: usize = 4_096;
static NEXT_QUEUE_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Canonical claim boundary for the live queue and private batch foundation.
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-aql-session-r11-v1\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device\n",
    "memory_profile_sha256=032e68de9b493deb70326fe8e65bb90248ff3a0d02d6a77f3e939df15262b33e\n",
    "queue_resource_profile_sha256=b8317e4288e14c6d7546b53887ec2a10e1938ffba9595271d174a2a652320f4f\n",
    "aql_dispatch_schema_sha256=b691e0df36e2c1f0695f49a19d49d3fbbe4380e8e9999b01368df02783952edf\n",
    "aql_fixed_batch_schema_sha256=3d8376174a564eaee500ad8849d8bf3a1a38d56f9e5bc50bf60aea408b25bf1d\n",
    "aql_completion_schema_sha256=406f1f2f3e93eb4704fba3b5ead0d0d05639991949baff4ad3a0360c343fb7a4\n",
    "dispatch_binding_schema_sha256=fe557859565195a4f24fb4e9689015c8845501afbdf2baddd3bd4415b1bda054\n",
    "event_schema_sha256=8d754af12ed2fcd0c238e1f9e38fbbdab053f44fc5d613b227fdcdd616fcc849\n",
    "runtime_enable_schema_sha256=4c762d1e35a5940f0972290151de51e6e19722f81874a6446c66ddc70a062ac1\n",
    "source.rocr.queues.c=b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b\n",
    "source.rocr.hsakamttypes.h=fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e\n",
    "source.kfd_events.c=295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a\n",
    "source.kfd_debug.c=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "source.kfd_chardev.c=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "source.kfd_process.c=d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409\n",
    "resources=linear-private-ring-control-eop-cwsr-completion-code-kernarg-and-exact-c3-device-lease-authorities,exact-one-vm,transferred-model-ownership\n",
    "gtt_policy=ring:aql-queue,control-and-completion-signals:host-visible-coherent,eop-and-cwsr:executable;fe2o3-policy-not-rocr-equivalence\n",
    "runtime=one-process-global-fe2o3-owner;exact-enable-r_debug0-mode1-capabilities0-before-event-and-any-queue;ttmp-save-excluded;foreign-kfd-clients-excluded\n",
    "initialization=every-logical-ring-slot-explicit-atomic-u32-invalid-1;control-explicit-two-atomic-u64-zero;completion-arena-exact-1024-typed-64-byte-user-signals-pending-1-before-gpu-map;one-first-internal-auto-reset-signal-event-id-1-through-255-before-create;8-cwsr-bo-and-shadow-headers-at-0x1621000-stride,debug-offset-descending,debug-size-0x5f000,one-first-shadow-aligned-error-reason-zero,exact-event-id\n",
    "submission=crate-private-non-clone-single-producer,aql-fixed-batch-v2-count-1-through-1024-and-ring-capacity-bounded,heap-owned-fixed-cardinality-state,no-mapped-slice-or-raw-pointer-escape,rptr-wptr-acquire,one-actual-wptr-acq-rel-fetch-add-by-count,all-invalid-bodies-before-any-ordered-u32-release-headers,release-fence-x86-sfence,one-final-volatile-u64-doorbell-store-of-last-packet-id\n",
    "completion=crate-private-non-clone-generation-bound-batches,unique-signal-per-packet,signal-code-kernarg-dispatch-and-queue-generations-retained,bounded-atomic-acquire-poll,pending-ready-fault-timeout-distinct,release-reset-only-after-all-signals-zero\n",
    "dispatch=private-only,validated-code-materialization-and-descriptor-resolution,typed-kernarg-device-pointer-injection,exact-c3-lease-set-and-data-premises,C2-publication,C4-completion,ordinary-release-or-exact-recycle-gated-c3-return-after-destroy\n",
    "doorbell=complete-8192-byte-kfd-slice,exact-returned-offset,madv-dontfork,no-public-address-pointer-or-mmio-accessor\n",
    "lifecycle=runtime-enable,event-create,queue-create;all-completion-batches-observed-and-recycled;queue-destroy,event-destroy,runtime-disable,doorbell-release,cwsr-queue-resource-and-completion-arena-release;no-drop-ioctl-store-munmap-or-free\n",
    "currentness=pid-and-device-before-publication,after-bounded-preparation,and-before-mmio\n",
    "proof=queue-and-aql-model-obligations-only,cpu-gpu-atomic-coherence-mmio-driver-firmware-refinement-contracted\n",
    "event-lifecycle=linear-private-kfd-event,no-event-page-mmap,queue-destroy-before-event-destroy-before-runtime-disable-before-cwsr-free-and-full-reservation-munmap,no-drop-ioctl-or-unmap\n",
    "cwsr-address-semantics=bo-cpu-vma-is-not-create-address;exact-8-owned-fixed-private-anonymous-pages,prot-none-then-dontfork-then-rw;headers-mirrored-and-read-back-in-bo-and-shadows;cpu-visible-debug-suspend-checkpoint-wave-state-copy-unsupported;ordinary-hardware-preemption-restore-contracted\n",
    "exception-observation=crate-private-one-shot-timeout-0-through-1000ms,wait-and-volatile-payload-must-agree,unknown-reason-rejected,timeout-is-terminal-racy-snapshot-not-absence-proof,no-atomic-or-lossless-delivery-claim\n",
    "failure=counter-divergence-regression-currentness-and-any-possible-side-effect-runtime-event-shadow-wait-publication-completion-observation-timeout-reset-or-teardown-error-terminally-poisons;no-in-process-recovery-rollback-or-cleanup-after-terminal-observation;only-pre-side-effect-full-or-insufficient-space-retryable\n",
    "excluded=public-submission,kernel-launch,live-batch-execution-evidence,actual-hardware-completion-fault-or-exception-delivery-evidence,production-copy-initialization-premise-mint,concrete-alias-or-effect-proof,update,multi-producer,foreign-kfd-process-coordination,cpu-visible-debug-suspend-checkpoint-wave-state-copy\n",
);

/// SHA-256 of [`GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1`].
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1: &str =
    "382dae772d3b7d99094ee5ddde74a4d3ffa8fef558923d6c8b4f0e3f6d9ce06d";

type RingAuthority = SharedGttQueueResourceAuthorityV1<
    AqlRingResourceRoleV1,
    AqlQueueGttV1,
    GttGpuAccessibleMutableV1,
>;
type ControlAuthority = SharedGttQueueResourceAuthorityV1<
    AqlControlResourceRoleV1,
    HostVisibleCoherentGttV1,
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
            .check_queue_currentness()
            .map_err(|_| Gfx942CompletionErrorV1::Currentness)?;
        self.exception
            .runtime
            .validate_queue_live(self.memory.kfd_fd(), self.memory.opener_pid())
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

    fn observe_acquire(
        &mut self,
        slot_index: u32,
    ) -> Result<fe2o3_aql::AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        self.memory
            .observe_aql_completion_signal(self.signals, slot_index)
            .map_err(|_| Gfx942CompletionErrorV1::Observation)
    }

    fn reset_pending_release(&mut self, slot_index: u32) -> Result<(), Gfx942CompletionErrorV1> {
        self.memory
            .reset_aql_completion_signal(self.signals, slot_index)
            .map_err(|_| Gfx942CompletionErrorV1::Recycle)
    }
}

impl NativeAqlSubmissionBackendV1 for LinuxAqlSubmissionBackendV1<'_> {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.memory
            .check_queue_currentness()
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)?;
        self.exception
            .runtime
            .validate_queue_live(self.memory.kfd_fd(), self.memory.opener_pid())
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
            .observe_aql_control_counters(self.control)
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)
    }

    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.memory
            .fetch_add_aql_control_write(self.control, increment)
            .map_err(|_| NativeAqlSubmissionErrorV1::Currentness)
    }

    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.memory
            .write_aql_ring_slot(self.ring, slot, &packet.encode_unpublished_le())
            .map_err(|_| NativeAqlSubmissionErrorV1::PacketBody)
    }

    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.memory
            .publish_aql_ring_header(self.ring, slot, header)
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
}

/// Evidence returned only after confirmed DESTROY and explicit resource return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputeAqlQueueDestroyedV1 {
    queue_id: u32,
    released_resources: u8,
}

impl ComputeAqlQueueDestroyedV1 {
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn released_resources(self) -> u8 {
        self.released_resources
    }
}

/// Private ownership returned by the exact recycled-dispatch teardown path.
///
/// The value retains the active shared-memory session beside the actual mapped
/// C3 authorities. It exposes neither native identities nor device addresses
/// and does not claim that any returned extent contains initialized content.
#[allow(dead_code)]
#[must_use = "returned mapped C3 leases require explicit unmap and release"]
pub(crate) struct Gfx942RecycledDispatchResourcesV1 {
    destroyed: ComputeAqlQueueDestroyedV1,
    memory: SharedGttMemorySessionV1,
    dispatch: ReturnedDispatchDataV1,
}

#[allow(dead_code)]
impl Gfx942RecycledDispatchResourcesV1 {
    pub(crate) const fn destroyed(&self) -> ComputeAqlQueueDestroyedV1 {
        self.destroyed
    }

    pub(crate) const fn dispatch_generation(&self) -> u64 {
        self.dispatch.generation()
    }

    pub(crate) fn data_lease_count(&self) -> usize {
        self.dispatch.data().len()
    }

    pub(super) fn into_parts(self) -> (SharedGttMemorySessionV1, ReturnedDispatchDataV1) {
        (self.memory, self.dispatch)
    }
}

enum QueueDestroyOutcomeV1 {
    Released(ComputeAqlQueueDestroyedV1),
    Returned(Box<Gfx942RecycledDispatchResourcesV1>),
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

#[must_use = "queue destruction and resource return are explicit"]
pub struct ComputeAqlQueueSessionV1 {
    engine: Option<NativeQueueEngineV1<LinuxNativeQueueBackendV1>>,
    key: QueueKeyV1,
    doorbell: Option<LinuxDoorbellSliceV1>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    completion_signals: Option<CompletionSignalAuthority>,
    completion_owner: CompletionSignalArenaOwnerV1,
    dispatch: Option<DispatchResourceOwnerV1>,
    exception: Option<QueueExceptionStateV1>,
    terminal_poisoned: bool,
    observation: ComputeAqlQueueObservationV1,
}

struct QueueExceptionStateV1 {
    runtime: LinuxKfdRuntimeEnabledV1,
    event: LinuxQueueExceptionEventV1,
    shadows: LinuxCwsrShadowPagesV1,
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
        self.create_compute_aql_queue_inner(ring_bytes, |_| Ok(None))
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
        self.create_compute_aql_queue_inner(ring_bytes, move |memory| {
            prepare_dispatch_resources(memory, kernel, geometry, kernargs, data)
                .map(Some)
                .map_err(ComputeAqlQueueSessionErrorV1::DispatchBinding)
        })
    }

    fn create_compute_aql_queue_inner(
        self,
        ring_bytes: u32,
        prepare_dispatch: impl FnOnce(
            &mut SharedGttMemorySessionV1,
        ) -> Result<
            Option<DispatchResourceOwnerV1>,
            ComputeAqlQueueSessionErrorV1,
        >,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        let geometry = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )?;
        let mut memory = self.acquire_shared_gtt_memory_session()?;
        let dispatch = prepare_dispatch(&mut memory)?;

        let mut ring =
            memory
                .allocate_aql_queue(usize::try_from(ring_bytes).map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract("ring size conversion")
                })?)?;
        let mut control = memory.allocate_host_visible_coherent(CONTROL_BYTES)?;
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
        let ring_initialization = memory.with_bytes_mut(&mut ring, initialize_invalid_ring)?;
        ring_initialization
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("INVALID ring initialization"))?;
        let control_initialization =
            memory.with_bytes_mut(&mut control, initialize_control_atomics)?;
        control_initialization.map_err(|_| {
            ComputeAqlQueueSessionErrorV1::Contract("AQL control atomic initialization")
        })?;
        let completion_initialization = memory.with_bytes_mut(
            &mut completion_signals,
            initialize_pending_completion_signal_arena,
        )?;
        completion_initialization?;
        memory.with_bytes_mut(&mut eop, |bytes| bytes.fill(0))?;
        memory.with_bytes_mut(&mut context_save, |bytes| bytes.fill(0))?;
        memory.check_queue_currentness()?;
        let mut runtime =
            match LinuxKfdRuntimeEnabledV1::enable(memory.kfd_fd(), memory.opener_pid()) {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = memory
                        .quarantine_queue_composition("RUNTIME_ENABLE enable ambiguous failure");
                    return Err(error.into());
                }
            };
        runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?;
        memory.check_queue_currentness()?;
        let event = match LinuxQueueExceptionEventV1::create(memory.kfd_fd(), memory.opener_pid()) {
            Ok(event) => event,
            Err(error) => {
                let _ = memory.quarantine_queue_composition("CREATE_EVENT ambiguous failure");
                return Err(error.into());
            }
        };
        memory.check_queue_currentness()?;
        let shadow_plan = memory.cwsr_shadow_plan(&context_save)?;
        let shadows = match LinuxCwsrShadowPagesV1::install(shadow_plan, &event) {
            Ok(shadows) => shadows,
            Err(error) => {
                let _ = memory.quarantine_queue_composition("CWSR shadow setup failure");
                return Err(error.into());
            }
        };
        let cwsr_initialization = match memory.with_bytes_mut(&mut context_save, |bytes| {
            shadows.initialize_and_validate_bo_headers(bytes)
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
        runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?;
        event.validate_live_with_shadows(memory.kfd_fd(), memory.opener_pid(), &shadows)?;
        memory.check_queue_currentness()?;
        let eop = memory.seal_executable(eop)?;
        let context_save = memory.seal_executable(context_save)?;
        let ring = memory.map_to_gpu(ring)?;
        let control = memory.map_to_gpu(control)?;
        let completion_signals = memory.map_to_gpu(completion_signals)?;
        let eop = memory.map_executable_to_gpu(eop)?;
        let context_save = memory.map_executable_to_gpu(context_save)?;
        let ring = memory.retain_aql_ring_resource(ring)?;
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
        let (identity, model) = match dispatch.as_ref() {
            Some(dispatch) => memory
                .take_queue_model_foundation_with_dispatch_memory(dispatch.device_authorities())?,
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
        engine.create(key).map_err(map_native)?;
        runtime.mark_queue_created()?;
        let outputs = engine
            .create_outputs(key)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing CREATE outputs",
            ))?;
        let queue_id = engine
            .native_queue_id(key)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing queue id"))?;
        let mut session = ComputeAqlQueueSessionV1 {
            engine: Some(engine),
            key,
            doorbell: None,
            submission: Some(NativeAqlSubmissionOwnerV1::new(ring_bytes).map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("AQL ring submission model")
            })?),
            completion_signals: Some(completion_signals),
            completion_owner,
            dispatch,
            exception: Some(QueueExceptionStateV1 {
                runtime,
                event,
                shadows,
            }),
            terminal_poisoned: false,
            observation: ComputeAqlQueueObservationV1 {
                queue_id,
                ring_bytes,
                doorbell_slice_bytes: 0,
                doorbell_byte_offset: 0,
                event_id: 0,
                cwsr_shadow_pages: 0,
            },
        };
        let exception = session.exception.as_ref().expect("queue exception state");
        session.observation.event_id = exception.event.event_id_observation();
        session.observation.cwsr_shadow_pages = 8;
        session.check_currentness()?;
        let doorbell = {
            let engine = session.engine.as_ref().expect("session engine");
            LinuxDoorbellSliceV1::map(engine.backend.session.kfd_fd(), outputs, engine.opener_pid)?
        };
        session.observation.doorbell_slice_bytes = doorbell.slice_bytes();
        session.observation.doorbell_byte_offset = doorbell.queue_byte_offset();
        session.doorbell = Some(doorbell);
        session.check_currentness()?;
        Ok(session)
    }
}

impl ComputeAqlQueueSessionV1 {
    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.observation
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
    #[allow(dead_code)]
    pub(crate) fn submit_bound_dispatch<const N: usize>(
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

    #[allow(dead_code)]
    pub(crate) fn poll_bound_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
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
        match self.poll_completion_batch(completion) {
            Ok(poll) => {
                if matches!(poll, Gfx942CompletionPollV1::Ready(_))
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
                Ok(wrap_poll(poll, generation))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn wait_bound_dispatch<const N: usize>(
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

    #[allow(dead_code)]
    pub(crate) fn recycle_bound_dispatch<const N: usize>(
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

    #[allow(dead_code)]
    pub(crate) fn poll_completion_batch<const N: usize>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
    ) -> Result<Gfx942CompletionPollV1<N>, ComputeAqlQueueSessionErrorV1> {
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
                owner.observe_once(batch, &mut backend)
            };
        if result.is_err() {
            self.poison_terminal();
        }
        result.map_err(Into::into)
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
        if result.is_err() {
            self.poison_terminal();
        }
        result.map_err(Into::into)
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
        match self.destroy_inner(false)? {
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
    #[allow(dead_code)]
    pub(crate) fn destroy_returning_recycled_dispatch_resources(
        self,
    ) -> Result<Gfx942RecycledDispatchResourcesV1, ComputeAqlQueueSessionErrorV1> {
        match self.destroy_inner(true)? {
            QueueDestroyOutcomeV1::Returned(resources) => Ok(*resources),
            QueueDestroyOutcomeV1::Released(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "returning destroy released dispatch resources",
            )),
        }
    }

    fn destroy_inner(
        mut self,
        return_dispatch_data: bool,
    ) -> Result<QueueDestroyOutcomeV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            ));
        }
        self.completion_owner.ensure_releasable()?;
        if let Some(dispatch) = self.dispatch.as_ref() {
            if return_dispatch_data {
                dispatch.ensure_returnable()?;
            } else {
                dispatch.ensure_releasable()?;
            }
        } else if return_dispatch_data {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
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
        exception.runtime.mark_event_destroyed()?;
        let disabled_runtime = exception.runtime.disable(
            engine.backend.session.kfd_fd(),
            engine.backend.session.opener_pid(),
        )?;
        let shadow_release = exception
            .shadows
            .after_event_and_runtime_destroy(destroyed_event, disabled_runtime)?;
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
        let returned_dispatch = match self.dispatch.take() {
            Some(dispatch) if return_dispatch_data => Some(
                dispatch.release_non_data_after_recycle(
                    &mut self
                        .engine
                        .as_mut()
                        .expect("session engine")
                        .backend
                        .session,
                )?,
            ),
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
            None => None,
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
        let destroyed = ComputeAqlQueueDestroyedV1 {
            queue_id: self.observation.queue_id,
            released_resources: 5,
        };
        let Some(dispatch) = returned_dispatch else {
            return Ok(QueueDestroyOutcomeV1::Released(destroyed));
        };
        let backend = self
            .engine
            .take()
            .expect("session engine")
            .into_backend()
            .map_err(map_native)?;
        Ok(QueueDestroyOutcomeV1::Returned(Box::new(
            Gfx942RecycledDispatchResourcesV1 {
                destroyed,
                memory: backend.session,
                dispatch,
            },
        )))
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
        let memory = core::mem::replace(&mut engine.memory, MemoryLifecycleStateV1::new(domain));
        engine
            .backend
            .session
            .restore_queue_model_foundation(identity, memory)?;
        engine.backend.foundation_in_engine = false;
        Ok(())
    }
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
    let ring_base = rf
        .checked_gpu_subrange(0, u64::from(geometry.ring().mapping_bytes()) * 2, 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("ring geometry"))?;
    if rf.logical_bytes() != geometry.ring().mapping_bytes() as usize
        || rf.gpu_va_bytes() != u64::from(geometry.ring().mapping_bytes()) * 2
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract("ring size/profile"));
    }
    if cf.logical_bytes() != CONTROL_BYTES || cf.gpu_va_bytes() != CONTROL_BYTES as u64 {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "control size/profile",
        ));
    }
    // KFD truncates each pointer to its GPU page and requires that page to be
    // one exact PAGE_SIZE GPUVM mapping. Both distinct counters live within
    // that single reviewed page.
    let (write_pointer, read_pointer) = cf
        .checked_disjoint_gpu_subranges((0, 8, 8), (8, 8, 8))
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
    ));
    let configuration = QueueConfigurationIdV1::from_untrusted_digest(digest_id(
        b"configuration",
        queue,
        &[rf.mapping(), cf.mapping(), ef.mapping(), sf.mapping()],
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
    let ring = memory.unmap_from_gpu(authority.ring.into_token())?;
    let control = memory.unmap_from_gpu(authority.control.into_token())?;
    let eop = memory.unmap_executable_from_gpu(authority.eop.into_token())?;
    let context_save = memory.unmap_executable_from_gpu(authority.context_save.into_token())?;
    memory.release(ring)?;
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
) -> IdentityDigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1.as_bytes());
    hasher.update(SHARED_GTT_MEMORY_PROFILE_SHA256_V1.as_bytes());
    hasher.update(tag);
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
    fn session_manifest_digest_is_frozen() {
        assert_eq!(
            fe2o3_aql::AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1,
            "b691e0df36e2c1f0695f49a19d49d3fbbe4380e8e9999b01368df02783952edf"
        );
        assert_eq!(
            fe2o3_aql::AQL_FIXED_BATCH_MODEL_MANIFEST_SHA256_V2,
            "3d8376174a564eaee500ad8849d8bf3a1a38d56f9e5bc50bf60aea408b25bf1d"
        );
        assert_eq!(
            super::super::completion::GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1,
            "406f1f2f3e93eb4704fba3b5ead0d0d05639991949baff4ad3a0360c343fb7a4"
        );
        assert_eq!(
            SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
            "032e68de9b493deb70326fe8e65bb90248ff3a0d02d6a77f3e939df15262b33e"
        );
        assert_eq!(
            fe2o3_kfd_uapi::KFD_RUNTIME_ENABLE_SCHEMA_SHA256,
            "4c762d1e35a5940f0972290151de51e6e19722f81874a6446c66ddc70a062ac1"
        );
        let digest = Sha256::digest(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1);
    }
}
