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
use crate::queue_linux::{LinuxDoorbellSliceV1, create_queue, destroy_queue};
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
pub const GFX942_SDMA_MAX_MULTI_QUEUE_SHARDS_V1: usize = GFX942_SDMA_MAX_STRIPED_QUEUES_V1;
pub const GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1: usize =
    GFX942_SDMA_MAX_STRIPED_QUEUES_V1 * GFX942_SDMA_MAX_IN_FLIGHT_V1;
const GFX942_SDMA_D2H_OWNER_SLOT_V1: usize = 0;
const GFX942_SDMA_H2D_OWNER_SLOT_V1: usize = 1;
const GFX942_SDMA_SINGLE_OWNER_COUNT_V1: usize = 1;
const GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1: usize = 2;

/// Frozen claim boundary for the bounded native gfx942 SDMA implementation.
pub const GFX942_SDMA_COPY_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-sdma-copy-r1-v4\n",
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
    "engines=generic-compatible-or-topology-exact-ordinary:2,queues-per-engine:8,h2d-index:1,d2h-index:0,targeted-queue-type:4,balanced-striped-queues:even-2..16,round-robin-per-successful-batch\n",
    "queue-identity=all-directional-or-striped-native-queue-ids-must-be-distinct-before-publication\n",
    "memory=move-only-host-coherent-or-device-local,logical-subrange-bounded,queue-retained-while-in-flight\n",
    "submission=single-producer,all-fallible-preparation-and-allocation-retains-recoverable-requests-before-mutation,striped-multi-queue-bounds:2..16-queues-and-1..1008-requests-and-at-most-63-per-shard,all-striped-shards-and-outcome-storage-prepared-before-first-publication,no-heap-allocation-after-first-publication,write-complete-sdma-packet-images-and-retained-records-before-one-exact-release-visible-wptr-publication-and-one-final-release-doorbell-per-batch,queue-occurrence-and-generation-tagged-ticket\n",
    "completion=host-coherent-u32-fence-value-observed-through-i64-acquire,exact-generation,nonblocking-poll,queue-progress-at-host-monotonic-instant,adaptive-deadline-wait,custody-returned-only-after-observation,no-gpu-clock-calibration\n",
    "cancellation=published-packets-cannot-be-retracted,typed-rejection-retains-ticket,poll-or-explicit-drain-required\n",
    "pool=queue-branded,best-fit-by-kind-size-and-alignment,leased-and-in-flight-excluded,concrete-generation-advanced-on-recycle,explicit-trim-before-teardown\n",
    "dispatch-data-bridge=exact-full-extent-host-content-or-completed-h2d-only,move-only-storage-identity-and-queue-and-pool-generation-binding,no-rematerialization,demotion-advances-pool-generation\n",
    "currentness=one-operational-pre-post-envelope-per-submit-batch-or-wait-batch-or-combined-submit-through-observed-completion,internal-atomics-and-mapped-writes-only-inside-envelope\n",
    "failure=structural-preflight-and-ordinary-capacity-rejection-recover-inputs,currentness-counter-generation-and-post-preflight-uncertainty-terminally-poison-and-retain-native-custody,striped-terminal-failure-exposes-audit-only-confirmed-and-at-most-one-indeterminate-and-untouched-observations-without-drain-or-resubmit-authority,striped-cursor-commits-only-after-complete-publication-and-closing-currentness,partial-directional-or-striped-create-or-destroy-has-process-only-terminal-custody\n",
    "teardown=destroy-sdma-before-compute,then-release-ring-control-completions-and-pooled-buffers-explicitly\n",
    "proof=abstract-pool-generation-retention-and-cross-device-coordinate-theorems-only,no-executable-rust-refinement\n",
    "contracted=ioctl-truth,doorbell-mapping,cpu-gpu-coherence,kernel-firmware-packet-consumption,completion,event-driven-completion,gpu-clock-calibration,progress,liveness\n",
    "measured=hardware-correctness-and-performance-on-identified-host-only\n",
);

/// SHA-256 of [`GFX942_SDMA_COPY_MANIFEST_V1`].
pub const GFX942_SDMA_COPY_MANIFEST_SHA256_V1: &str =
    "8543f344b4fba5fff152b718ea547e620e931ca01101f32f65828fe1eb9a303b";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942SdmaMultiQueuePlanErrorV1 {
    QueueCount { actual: usize },
    DuplicateQueueId { queue_id: u32 },
    RequestCount { actual: usize, maximum: usize },
    InvalidCursor { actual: usize, queue_count: usize },
    Allocation,
}

impl fmt::Display for Gfx942SdmaMultiQueuePlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 SDMA multi-queue plan: {self:?}")
    }
}

impl std::error::Error for Gfx942SdmaMultiQueuePlanErrorV1 {}

/// Deterministic bounded request-to-queue assignment for one striped submission.
///
/// This is a structural plan. It neither publishes packets nor proves queue currentness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaMultiQueuePlanV1 {
    queue_ids: Vec<u32>,
    first_queue: u16,
    request_count: u16,
    assignments: Vec<u16>,
    shard_counts: Vec<u16>,
}

impl Gfx942SdmaMultiQueuePlanV1 {
    pub fn new(
        queue_ids: &[u32],
        request_count: usize,
        first_queue: usize,
    ) -> Result<Self, Gfx942SdmaMultiQueuePlanErrorV1> {
        if queue_ids.len() < KFD_GFX942_SDMA_ENGINE_COUNT_V1 as usize
            || !queue_ids
                .len()
                .is_multiple_of(KFD_GFX942_SDMA_ENGINE_COUNT_V1 as usize)
            || queue_ids.len() > GFX942_SDMA_MAX_STRIPED_QUEUES_V1
        {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount {
                actual: queue_ids.len(),
            });
        }
        for (index, queue_id) in queue_ids.iter().copied().enumerate() {
            if queue_ids[..index].contains(&queue_id) {
                return Err(Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { queue_id });
            }
        }
        let maximum = queue_ids
            .len()
            .checked_mul(GFX942_SDMA_MAX_IN_FLIGHT_V1)
            .ok_or(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount {
                actual: request_count,
                maximum: GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1,
            })?;
        if request_count == 0
            || request_count > maximum
            || request_count > GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1
        {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount {
                actual: request_count,
                maximum,
            });
        }
        if first_queue >= queue_ids.len() {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor {
                actual: first_queue,
                queue_count: queue_ids.len(),
            });
        }
        let mut retained_queue_ids = Vec::new();
        retained_queue_ids
            .try_reserve_exact(queue_ids.len())
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        retained_queue_ids.extend_from_slice(queue_ids);
        let mut assignments = Vec::new();
        assignments
            .try_reserve_exact(request_count)
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        let mut shard_counts = Vec::new();
        shard_counts
            .try_reserve_exact(queue_ids.len())
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        shard_counts.resize(queue_ids.len(), 0_u16);
        for request_index in 0..request_count {
            let queue = (first_queue + request_index) % queue_ids.len();
            assignments.push(queue as u16);
            shard_counts[queue] += 1;
        }
        debug_assert!(
            shard_counts
                .iter()
                .all(|count| usize::from(*count) <= GFX942_SDMA_MAX_IN_FLIGHT_V1)
        );
        Ok(Self {
            queue_ids: retained_queue_ids,
            first_queue: first_queue as u16,
            request_count: request_count as u16,
            assignments,
            shard_counts,
        })
    }

    pub fn queue_ids(&self) -> &[u32] {
        &self.queue_ids
    }

    pub const fn first_queue(&self) -> usize {
        self.first_queue as usize
    }

    pub const fn request_count(&self) -> usize {
        self.request_count as usize
    }

    pub fn queue_for_request(&self, request_index: usize) -> Option<usize> {
        self.assignments
            .get(request_index)
            .map(|queue| *queue as usize)
    }

    pub fn shard_count(&self, queue: usize) -> Option<usize> {
        self.shard_counts.get(queue).map(|count| *count as usize)
    }

    pub fn active_shard_count(&self) -> usize {
        self.shard_counts
            .iter()
            .filter(|count| **count != 0)
            .count()
    }

    pub fn next_queue_after_success(&self) -> usize {
        (self.first_queue() + self.request_count()) % self.queue_ids.len()
    }

    pub fn is_current_for(&self, queue_ids: &[u32], first_queue: usize) -> bool {
        self.queue_ids.as_slice() == queue_ids && self.first_queue() == first_queue
    }

    pub fn is_balanced(&self) -> bool {
        let minimum = self.shard_counts.iter().copied().min().unwrap_or(0);
        let maximum = self.shard_counts.iter().copied().max().unwrap_or(0);
        maximum - minimum <= 1
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Gfx942SdmaBufferStorageIdentityV1 {
    Host(SharedGttAllocationIdentityV1),
    Device(Gfx942DeviceMemoryIdentityV1),
}

/// Move-only allocation accepted by the bounded SDMA queue.
#[must_use = "the buffer owns a mapped allocation and requires explicit release"]
pub struct Gfx942SdmaBufferV1 {
    storage: Gfx942SdmaBufferStorageV1,
    owner: QueueKeyV1,
    pool_generation: u64,
    logical_bytes: u64,
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
        self.logical_bytes = logical_bytes;
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

/// Exact custody record for one queue shard after successful or indeterminate publication.
/// A success result means the shard publication returned success; the `indeterminate` field of a
/// failure means mapped publication began but its final device-visible state is not known.
#[must_use = "published tickets retain queue-owned buffer custody until completion"]
pub struct Gfx942SdmaMultiQueueShardTicketsV1 {
    queue_ordinal: u16,
    queue_id: u32,
    request_indices: Vec<u16>,
    tickets: Vec<Gfx942SdmaCopyTicketV1>,
}

impl Gfx942SdmaMultiQueueShardTicketsV1 {
    pub const fn queue_ordinal(&self) -> usize {
        self.queue_ordinal as usize
    }

    pub const fn queue_id(&self) -> u32 {
        self.queue_id
    }

    pub fn request_indices(&self) -> &[u16] {
        &self.request_indices
    }

    pub fn tickets(&self) -> &[Gfx942SdmaCopyTicketV1] {
        &self.tickets
    }

    pub fn into_tickets(self) -> Vec<Gfx942SdmaCopyTicketV1> {
        self.tickets
    }
}

impl fmt::Debug for Gfx942SdmaMultiQueueShardTicketsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SdmaMultiQueueShardTicketsV1")
            .field("queue_ordinal", &self.queue_ordinal)
            .field("queue_id", &self.queue_id)
            .field("request_indices", &self.request_indices)
            .field("ticket_count", &self.tickets.len())
            .finish()
    }
}

/// Successful publication across every non-empty shard in one bounded plan.
#[must_use = "every shard contains live tickets that must be completed"]
pub struct Gfx942SdmaMultiQueueSubmissionV1 {
    plan: Gfx942SdmaMultiQueuePlanV1,
    shards: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
}

impl Gfx942SdmaMultiQueueSubmissionV1 {
    pub const fn plan(&self) -> &Gfx942SdmaMultiQueuePlanV1 {
        &self.plan
    }

    pub fn shards(&self) -> &[Gfx942SdmaMultiQueueShardTicketsV1] {
        &self.shards
    }

    pub fn into_shards(self) -> Vec<Gfx942SdmaMultiQueueShardTicketsV1> {
        self.shards
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942SdmaMultiQueuePlanV1,
        Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
    ) {
        (self.plan, self.shards)
    }
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
        destination: Gfx942SdmaBufferV1,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx942SdmaMemoryPoolObservationV1 {
    pub checked_out_buffers: usize,
    pub retained_free_buffers: usize,
    pub retained_free_bytes: u64,
    pub reuse_count: u64,
}

struct SdmaCopyRecordV1 {
    generation: u32,
    completion_value: u32,
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

struct IndexedSdmaRequestV1<R = Gfx942SdmaCopyRequestV1> {
    index: u16,
    request: R,
}

struct PreparedMultiQueueShardV1<P = PreparedSdmaBatchV1> {
    queue_ordinal: usize,
    request_indices: Vec<u16>,
    batch: P,
}

pub(crate) struct PreparedMultiQueueSdmaBatchV1<
    P = PreparedSdmaBatchV1,
    S = Gfx942SdmaMultiQueueShardTicketsV1,
    U = Gfx942SdmaUnpublishedCopyRequestV1,
> {
    plan: Gfx942SdmaMultiQueuePlanV1,
    shards: Vec<PreparedMultiQueueShardV1<P>>,
    preflight: MultiQueuePreflightStateV1,
    published_capacity: Vec<S>,
    unpublished_capacity: Vec<U>,
}

pub(crate) struct MultiQueueSdmaPreparationFailureV1<R = Gfx942SdmaCopyRequestV1> {
    pub(crate) error: Gfx942SdmaErrorV1,
    pub(crate) requests: Vec<R>,
}

pub(crate) struct MultiQueueSdmaPublicationFailureV1<
    S = Gfx942SdmaMultiQueueShardTicketsV1,
    U = Gfx942SdmaUnpublishedCopyRequestV1,
> {
    pub(crate) error: Gfx942SdmaErrorV1,
    pub(crate) plan: Gfx942SdmaMultiQueuePlanV1,
    pub(crate) published: Vec<S>,
    pub(crate) indeterminate: Option<S>,
    pub(crate) unpublished: Vec<U>,
}

pub(crate) enum MultiQueueSdmaSubmitFailureV1 {
    Preparation(MultiQueueSdmaPreparationFailureV1),
    Publication(MultiQueueSdmaPublicationFailureV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MultiQueueCursorOutcomeV1 {
    CompleteSuccess,
    #[cfg(test)]
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MultiQueuePreflightStateV1 {
    expected_queue_mask: u16,
    prepared_queue_mask: u16,
    confirmed_queue_mask: u16,
    indeterminate_queue_mask: u16,
    publication_authorized: bool,
}

impl MultiQueuePreflightStateV1 {
    fn new(plan: &Gfx942SdmaMultiQueuePlanV1) -> Self {
        let mut expected_queue_mask = 0_u16;
        for queue in 0..plan.queue_ids().len() {
            if plan.shard_count(queue).is_some_and(|count| count != 0) {
                expected_queue_mask |= 1_u16 << queue;
            }
        }
        Self {
            expected_queue_mask,
            prepared_queue_mask: 0,
            confirmed_queue_mask: 0,
            indeterminate_queue_mask: 0,
            publication_authorized: false,
        }
    }

    fn record_prepared_queue(&mut self, queue: usize) -> Result<(), Gfx942SdmaErrorV1> {
        let Some(bit) = 1_u16.checked_shl(queue as u32) else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue preflight queue bound",
            ));
        };
        if self.publication_authorized
            || self.expected_queue_mask & bit == 0
            || self.prepared_queue_mask & bit != 0
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue duplicate or unexpected preflight",
            ));
        }
        self.prepared_queue_mask |= bit;
        Ok(())
    }

    fn authorize_publication(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        if self.publication_authorized
            || self.expected_queue_mask == 0
            || self.prepared_queue_mask != self.expected_queue_mask
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue publication before complete preflight",
            ));
        }
        self.publication_authorized = true;
        Ok(())
    }

    fn record_publication_observation(
        &mut self,
        queue: usize,
        observation: MultiQueuePublicationObservationV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Some(bit) = 1_u16.checked_shl(queue as u32) else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue publication queue bound",
            ));
        };
        if !self.publication_authorized
            || self.expected_queue_mask & bit == 0
            || (self.confirmed_queue_mask | self.indeterminate_queue_mask) & bit != 0
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue duplicate or unexpected publication observation",
            ));
        }
        match observation {
            MultiQueuePublicationObservationV1::Confirmed => {
                self.confirmed_queue_mask |= bit;
            }
            MultiQueuePublicationObservationV1::RecoverableNoEffect => {}
            MultiQueuePublicationObservationV1::Indeterminate => {
                if self.indeterminate_queue_mask != 0 {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "multi-queue duplicate indeterminate publication observation",
                    ));
                }
                self.indeterminate_queue_mask = bit;
            }
        }
        Ok(())
    }

    const fn publication_is_complete(&self) -> bool {
        self.publication_authorized
            && self.confirmed_queue_mask == self.expected_queue_mask
            && self.indeterminate_queue_mask == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MultiQueuePublicationObservationV1 {
    Confirmed,
    RecoverableNoEffect,
    Indeterminate,
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
    pub(crate) fn create(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        Self::create_with_engine(memory, owner, None)
    }

    fn create_on_engine(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine: KfdGfx942SdmaEngineId,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        Self::create_with_engine(
            memory,
            owner,
            Some(Gfx942SdmaEngineProfileV1::Ordinary(engine)),
        )
    }

    fn create_on_xgmi_engine(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine: KfdGfx942SdmaXgmiEngineId,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        Self::create_with_engine(memory, owner, Some(Gfx942SdmaEngineProfileV1::Xgmi(engine)))
    }

    fn create_with_engine(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine: Option<Gfx942SdmaEngineProfileV1>,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        let mut records = Vec::new();
        records
            .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA record roster allocation"))?;
        records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
        let mut xgmi_records = Vec::new();
        xgmi_records
            .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI SDMA record roster allocation"))?;
        xgmi_records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
        let mut persistent_window_slots = Vec::new();
        persistent_window_slots
            .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("persistent SDMA window slot roster"))?;
        persistent_window_slots.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
        let mut persistent_window_records = Vec::new();
        persistent_window_records
            .try_reserve_exact(GFX942_SDMA_RING_SLOT_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("persistent SDMA window owner roster"))?;
        persistent_window_records.resize_with(GFX942_SDMA_RING_SLOT_COUNT_V1, || None);
        memory.check_queue_currentness()?;
        let mut ring = memory.allocate_aql_queue(GFX942_SDMA_RING_BYTES_V1 as usize)?;
        memory.with_bytes_mut(&mut ring, |bytes| bytes.fill(0))?;
        let mut control = memory.allocate_userptr_aql_control()?;
        memory
            .with_bytes_mut(&mut control, initialize_amd_aql_control)?
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA control initialization"))?;
        let mut completions =
            memory.allocate_host_visible_coherent(GFX942_SDMA_RING_BYTES_V1 as usize)?;
        memory.with_bytes_mut(&mut completions, |bytes| bytes.fill(0))?;

        let ring = memory.map_to_gpu(ring)?;
        let control = memory.map_to_gpu(control)?;
        let completions = memory.map_to_gpu(completions)?;
        let ring_facts = memory.mapped_resource_facts(&ring)?;
        let control_facts = memory.mapped_resource_facts(&control)?;
        let ring = memory.retain_aql_ring_resource(ring)?;
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
        let mut actual = expected;
        let doorbell_failure = preallocate_doorbell_failure_message()?;
        create_queue(memory.kfd_fd(), &mut actual)
            .map_err(|_| Gfx942SdmaErrorV1::QueueCreationIndeterminate)?;
        let output_queue_id = actual.queue_id;
        let output_doorbell = actual.doorbell_offset;
        actual.queue_id = u32::MAX;
        actual.doorbell_offset = u64::MAX;
        if actual != expected {
            return Err(Gfx942SdmaErrorV1::Contract(
                "kernel changed immutable SDMA CREATE_QUEUE inputs",
            ));
        }
        let outputs = admit_kfd_gfx942_create_queue_outputs(
            output_queue_id,
            output_doorbell,
            memory.gpu_id(),
        )
        .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA CREATE_QUEUE outputs"))?;
        let queue_id = outputs.queue_id().value();
        let doorbell = LinuxDoorbellSliceV1::map(memory.kfd_fd(), outputs, memory.opener_pid())
            .map_err(|_| Gfx942SdmaErrorV1::Doorbell(doorbell_failure))?;
        memory.check_queue_currentness()?;

        Ok(Self {
            owner,
            queue_id,
            engine_index: engine.map(Gfx942SdmaEngineProfileV1::value),
            ring: Some(ring),
            control: Some(control),
            completions: Some(completions),
            doorbell: Some(doorbell),
            records,
            xgmi_records,
            persistent_window_slots,
            persistent_window_records,
            uncertain_xgmi_ticket: None,
            generations: [0; GFX942_SDMA_RING_SLOT_COUNT_V1],
            destroyed: false,
            poisoned: false,
        })
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
            generation,
            completion_value,
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
            write,
            write_end,
            copy,
            ticket,
            request,
            ..
        } = prepared;
        self.generations[copy.slot] = copy.generation;
        self.records[copy.slot] = Some(SdmaCopyRecordV1 {
            generation: copy.generation,
            completion_value: copy.completion_value,
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
    ) -> Result<CompletedPersistentSdmaWindowV1, Gfx942SdmaErrorV1> {
        self.require_live()?;
        self.validate_persistent_window_tickets(tickets)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "persistent SDMA window wait deadline",
            ))?;
        let mut wait = MonotonicWaitV1::until(deadline);
        memory.check_queue_operational_currentness()?;
        loop {
            if self.observe_persistent_window_completion(memory, tickets)? {
                break;
            }
            if wait.expired() {
                memory.check_queue_operational_currentness()?;
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            wait.pause();
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
                generation: item.generation,
                completion_value: item.completion_value,
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
        let mut wait = MonotonicWaitV1::until(deadline);
        memory.check_queue_operational_currentness()?;
        loop {
            let observed = memory.observe_mapped_host_visible_i64_at_in_current_scope(
                self.completions
                    .as_mut()
                    .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?,
                (slot * 8) as u64,
            )?;
            if observed == i64::from(expected) {
                break;
            }
            if observed != 0 {
                self.poisoned = true;
                return Err(Gfx942SdmaErrorV1::Contract(
                    "unexpected SDMA completion value",
                ));
            }
            if wait.expired() {
                memory.check_queue_operational_currentness()?;
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            wait.pause();
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
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        source.validate_gfx942_xgmi_route_with_peer(destination, route)?;
        if source.gpu_id() != route.source_gpu_id() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "XGMI queue executing GPU does not match directional route",
            ));
        }
        let engine =
            admit_kfd_gfx942_sdma_xgmi_engine_mask(route.link().recommended_sdma_engine_id_mask())
                .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI SDMA route engine mask"))?;
        if engine.value() != route.recommended_engine_id() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "XGMI SDMA route engine identity",
            ));
        }
        let owner_key = source.next_xgmi_sdma_queue_key()?;
        let owner = Gfx942SdmaQueueOwnerV1::create_on_xgmi_engine(source, owner_key, engine)?;
        source.validate_gfx942_xgmi_route_with_peer(destination, route)?;
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

pub(crate) enum Gfx942SdmaQueueSetV1 {
    Generic(Vec<Gfx942SdmaQueueOwnerV1>),
    Directional(Vec<Gfx942SdmaQueueOwnerV1>),
    Striped {
        owners: Vec<Gfx942SdmaQueueOwnerV1>,
        next_owner: usize,
    },
}

impl Gfx942SdmaQueueSetV1 {
    pub(crate) fn create_generic(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(GFX942_SDMA_SINGLE_OWNER_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("generic SDMA owner roster allocation"))?;
        owners.push(Gfx942SdmaQueueOwnerV1::create(memory, owner)?);
        Ok(Self::Generic(owners))
    }

    pub(crate) fn create_directional(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        memory.check_gfx942_sdma_topology_capability_currentness()?;
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "directional SDMA engine inventory is not the exact gfx942 profile",
            ));
        }
        let mut directional = Vec::new();
        directional
            .try_reserve_exact(GFX942_SDMA_DIRECTIONAL_OWNER_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("directional SDMA owner roster allocation"))?;
        let device_to_host = Gfx942SdmaQueueOwnerV1::create_on_engine(
            memory,
            owner,
            admit_kfd_gfx942_sdma_engine_id(GFX942_SDMA_D2H_ENGINE_INDEX_V1)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("D2H SDMA engine index"))?,
        )?;
        let host_to_device = Gfx942SdmaQueueOwnerV1::create_on_engine(
            memory,
            owner,
            admit_kfd_gfx942_sdma_engine_id(GFX942_SDMA_H2D_ENGINE_INDEX_V1)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("H2D SDMA engine index"))?,
        )?;
        if !directional_queue_ids_are_distinct(device_to_host.queue_id, host_to_device.queue_id) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "directional SDMA queues returned duplicate native queue IDs",
            ));
        }
        directional.push(device_to_host);
        directional.push(host_to_device);
        memory.check_gfx942_sdma_topology_capability_currentness()?;
        Ok(Self::Directional(directional))
    }

    pub(crate) fn create_targeted(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        engine_index: u32,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        memory.check_gfx942_sdma_topology_capability_currentness()?;
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "targeted SDMA engine inventory is not the exact gfx942 profile",
            ));
        }
        let engine = admit_kfd_gfx942_sdma_engine_id(engine_index)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("targeted SDMA engine index"))?;
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(GFX942_SDMA_SINGLE_OWNER_COUNT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("targeted SDMA owner roster allocation"))?;
        owners.push(Gfx942SdmaQueueOwnerV1::create_on_engine(
            memory, owner, engine,
        )?);
        memory.check_gfx942_sdma_topology_capability_currentness()?;
        Ok(Self::Generic(owners))
    }

    pub(crate) fn create_striped(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        queue_count: u32,
    ) -> Result<(Self, Vec<Gfx942SdmaQueueObservationV1>), Gfx942SdmaErrorV1> {
        memory.check_gfx942_sdma_topology_capability_currentness()?;
        let (engine_count, queues_per_engine) = memory.gfx942_sdma_engine_inventory();
        if engine_count != Some(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
            || queues_per_engine != Some(KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
            || !striped_sdma_queue_count_is_admitted(queue_count)
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped SDMA queue topology or count",
            ));
        }
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(queue_count as usize)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("striped SDMA owner roster allocation"))?;
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(queue_count as usize)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("striped SDMA observation allocation"))?;
        for queue_index in 0..queue_count {
            let engine_index = queue_index % KFD_GFX942_SDMA_ENGINE_COUNT_V1;
            let engine = admit_kfd_gfx942_sdma_engine_id(engine_index)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("striped SDMA engine index"))?;
            let created = Gfx942SdmaQueueOwnerV1::create_on_engine(memory, owner, engine)?;
            if owners
                .iter()
                .any(|existing: &Gfx942SdmaQueueOwnerV1| existing.queue_id == created.queue_id)
            {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped SDMA duplicate native queue ID",
                ));
            }
            observations.push(created.observation());
            owners.push(created);
        }
        memory.check_gfx942_sdma_topology_capability_currentness()?;
        Ok((
            Self::Striped {
                owners,
                next_owner: 0,
            },
            observations,
        ))
    }

    pub(crate) fn generic_observation(&self) -> Option<Gfx942SdmaQueueObservationV1> {
        match self {
            Self::Generic(owners) => owners.first().map(Gfx942SdmaQueueOwnerV1::observation),
            Self::Directional(_) | Self::Striped { .. } => None,
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

    // Inline partial-publication custody is preallocated before the first publication.
    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_striped_multi_queue_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaMultiQueueSubmissionV1, MultiQueueSdmaSubmitFailureV1> {
        let (owners, next_owner) = match self {
            Self::Striped { owners, next_owner } => (owners, next_owner),
            Self::Generic(_) | Self::Directional(_) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                    MultiQueueSdmaPreparationFailureV1 {
                        error: Gfx942SdmaErrorV1::Contract(
                            "multi-queue submission requires a striped SDMA queue set",
                        ),
                        requests,
                    },
                ));
            }
        };
        let mut queue_ids = Vec::new();
        if queue_ids.try_reserve_exact(owners.len()).is_err() {
            return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                MultiQueueSdmaPreparationFailureV1 {
                    error: Gfx942SdmaErrorV1::Contract(
                        "multi-queue SDMA queue identity allocation",
                    ),
                    requests,
                },
            ));
        }
        queue_ids.extend(owners.iter().map(|owner| owner.queue_id));
        let plan = match Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, requests.len(), *next_owner) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                    MultiQueueSdmaPreparationFailureV1 {
                        error: map_multi_queue_plan_error(error),
                        requests,
                    },
                ));
            }
        };
        let prepared = match prepare_multi_queue_batch(
            owners.len(),
            plan,
            requests,
            |queue, requests| owners[queue].prepare_batch_recoverable(memory, requests),
            PreparedSdmaBatchV1::into_requests,
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(failure));
            }
        };
        match publish_multi_queue_batch(
            prepared,
            |queue, batch| {
                let queue_id = batch.queue_id;
                match owners[queue].submit_prepared_batch_with_custody(memory, batch) {
                    Ok(tickets) => {
                        debug_assert!(tickets.iter().all(|ticket| ticket.queue_id == queue_id));
                        Ok((queue_id, tickets))
                    }
                    Err(failure) => Err((queue_id, failure)),
                }
            },
            PreparedSdmaBatchV1::into_requests,
            |queue_ordinal, queue_id, request_indices, tickets| {
                Gfx942SdmaMultiQueueShardTicketsV1 {
                    queue_ordinal: queue_ordinal as u16,
                    queue_id,
                    request_indices,
                    tickets,
                }
            },
            |request_index, request| Gfx942SdmaUnpublishedCopyRequestV1 {
                request_index,
                request,
            },
            |request| request.request_index,
            |plan, shards| Gfx942SdmaMultiQueueSubmissionV1 { plan, shards },
        ) {
            Ok(submission) => Ok(submission),
            // Failure deliberately performs no cursor write.
            Err(failure) => Err(MultiQueueSdmaSubmitFailureV1::Publication(failure)),
        }
    }

    pub(crate) fn commit_striped_multi_queue_success(
        &mut self,
        plan: &Gfx942SdmaMultiQueuePlanV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Self::Striped { owners, next_owner } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue cursor commit requires striped SDMA queues",
            ));
        };
        if plan.first_queue() != *next_owner
            || plan.queue_ids().len() != owners.len()
            || !plan
                .queue_ids()
                .iter()
                .copied()
                .eq(owners.iter().map(|owner| owner.queue_id))
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "stale multi-queue cursor commit",
            ));
        }
        *next_owner = cursor_after_multi_queue_outcome(
            *next_owner,
            plan,
            MultiQueueCursorOutcomeV1::CompleteSuccess,
        )?;
        Ok(())
    }

    pub(crate) fn directional_observation(
        &self,
    ) -> Option<Gfx942DirectionalSdmaQueueObservationV1> {
        match self {
            Self::Generic(_) | Self::Striped { .. } => None,
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
    pub(crate) fn prepare_single_recoverable(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942SdmaCopyRequestV1,
    ) -> Result<PreparedSingleSdmaV1, (Gfx942SdmaErrorV1, Gfx942SdmaCopyRequestV1)> {
        let owner = match self.owner_for_copy(request.source.kind(), request.destination.kind()) {
            Ok(owner) => owner,
            Err(error) => return Err((error, request)),
        };
        owner.prepare_single_recoverable(memory, request)
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
    ) -> Result<CompletedPersistentSdmaWindowV1, Gfx942SdmaErrorV1> {
        self.owner_for_tickets(tickets)?
            .wait_persistent_window_for(memory, tickets, timeout)
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
    ) -> Result<Gfx942SdmaCompletedCopyV1, Gfx942SdmaErrorV1> {
        self.owner_for_ticket(ticket)?
            .wait_for(memory, ticket, timeout)
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
            Self::Striped { .. } => true,
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
        }
    }

    pub(crate) fn additional_resource_count(&self) -> u8 {
        match self {
            Self::Generic(_) => 3,
            Self::Directional(_) => 6,
            Self::Striped { owners, .. } => {
                u8::try_from(owners.len().saturating_mul(3)).unwrap_or(u8::MAX)
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

struct MultiQueueShardBuildV1<R = Gfx942SdmaCopyRequestV1> {
    request_indices: Vec<u16>,
    requests: Vec<R>,
}

fn prepare_multi_queue_batch<R, P, S, U>(
    queue_count: usize,
    plan: Gfx942SdmaMultiQueuePlanV1,
    requests: Vec<R>,
    mut prepare_shard: impl FnMut(usize, Vec<R>) -> Result<P, (Gfx942SdmaErrorV1, Vec<R>)>,
    mut prepared_into_requests: impl FnMut(P) -> Vec<R>,
) -> Result<PreparedMultiQueueSdmaBatchV1<P, S, U>, MultiQueueSdmaPreparationFailureV1<R>> {
    let allocation_error = || Gfx942SdmaErrorV1::Contract("multi-queue SDMA custody allocation");
    let mut shards = Vec::new();
    if shards.try_reserve_exact(queue_count).is_err() {
        return Err(MultiQueueSdmaPreparationFailureV1 {
            error: allocation_error(),
            requests,
        });
    }
    for queue in 0..queue_count {
        let count = plan.shard_count(queue).unwrap_or(0);
        let mut request_indices = Vec::new();
        let mut queue_requests = Vec::new();
        if request_indices.try_reserve_exact(count).is_err()
            || queue_requests.try_reserve_exact(count).is_err()
        {
            return Err(MultiQueueSdmaPreparationFailureV1 {
                error: allocation_error(),
                requests,
            });
        }
        shards.push(MultiQueueShardBuildV1 {
            request_indices,
            requests: queue_requests,
        });
    }
    let mut prepared_shards = Vec::new();
    let mut recovery_pairs = Vec::new();
    let mut recovered_requests = Vec::new();
    let mut published_capacity = Vec::new();
    let mut unpublished_capacity = Vec::new();
    let mut preflight = MultiQueuePreflightStateV1::new(&plan);
    if prepared_shards
        .try_reserve_exact(plan.active_shard_count())
        .is_err()
        || recovery_pairs
            .try_reserve_exact(plan.request_count())
            .is_err()
        || recovered_requests
            .try_reserve_exact(plan.request_count())
            .is_err()
        || published_capacity
            .try_reserve_exact(plan.active_shard_count())
            .is_err()
        || unpublished_capacity
            .try_reserve_exact(plan.request_count())
            .is_err()
    {
        return Err(MultiQueueSdmaPreparationFailureV1 {
            error: allocation_error(),
            requests,
        });
    }
    for (index, request) in requests.into_iter().enumerate() {
        let queue = plan
            .queue_for_request(index)
            .expect("plan covers every bounded request");
        shards[queue].request_indices.push(index as u16);
        shards[queue].requests.push(request);
    }
    for step in 0..queue_count {
        let queue = (plan.first_queue() + step) % queue_count;
        if shards[queue].requests.is_empty() {
            continue;
        }
        let queue_requests = std::mem::take(&mut shards[queue].requests);
        match prepare_shard(queue, queue_requests) {
            Ok(batch) => {
                let prepared = PreparedMultiQueueShardV1 {
                    queue_ordinal: queue,
                    request_indices: std::mem::take(&mut shards[queue].request_indices),
                    batch,
                };
                if let Err(error) = preflight.record_prepared_queue(queue) {
                    prepared_shards.push(prepared);
                    append_prepared_requests(
                        &mut recovery_pairs,
                        prepared_shards,
                        &mut prepared_into_requests,
                    );
                    for shard in shards {
                        append_indexed_requests(
                            &mut recovery_pairs,
                            shard.request_indices,
                            shard.requests,
                        );
                    }
                    recovery_pairs.sort_unstable_by_key(|request| request.index);
                    recovered_requests
                        .extend(recovery_pairs.into_iter().map(|request| request.request));
                    return Err(MultiQueueSdmaPreparationFailureV1 {
                        error,
                        requests: recovered_requests,
                    });
                }
                prepared_shards.push(prepared);
            }
            Err((error, queue_requests)) => {
                append_prepared_requests(
                    &mut recovery_pairs,
                    prepared_shards,
                    &mut prepared_into_requests,
                );
                append_indexed_requests(
                    &mut recovery_pairs,
                    std::mem::take(&mut shards[queue].request_indices),
                    queue_requests,
                );
                for shard in shards {
                    append_indexed_requests(
                        &mut recovery_pairs,
                        shard.request_indices,
                        shard.requests,
                    );
                }
                recovery_pairs.sort_unstable_by_key(|request| request.index);
                recovered_requests
                    .extend(recovery_pairs.into_iter().map(|request| request.request));
                return Err(MultiQueueSdmaPreparationFailureV1 {
                    error,
                    requests: recovered_requests,
                });
            }
        }
    }
    Ok(PreparedMultiQueueSdmaBatchV1 {
        plan,
        shards: prepared_shards,
        preflight,
        published_capacity,
        unpublished_capacity,
    })
}

// Boxing this error would add an allocation after a prior shard may be device-visible.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
fn publish_multi_queue_batch<R, P, T, S, U, O>(
    prepared: PreparedMultiQueueSdmaBatchV1<P, S, U>,
    mut publish_shard: impl FnMut(
        usize,
        P,
    ) -> Result<
        (u32, Vec<T>),
        (u32, PreparedSdmaPublicationFailureV1<P, T>),
    >,
    mut prepared_into_requests: impl FnMut(P) -> Vec<R>,
    mut make_published: impl FnMut(usize, u32, Vec<u16>, Vec<T>) -> S,
    mut make_unpublished: impl FnMut(u16, R) -> U,
    unpublished_index: impl Fn(&U) -> u16,
    make_submission: impl FnOnce(Gfx942SdmaMultiQueuePlanV1, Vec<S>) -> O,
) -> Result<O, MultiQueueSdmaPublicationFailureV1<S, U>> {
    let PreparedMultiQueueSdmaBatchV1 {
        plan,
        shards,
        mut preflight,
        mut published_capacity,
        mut unpublished_capacity,
    } = prepared;
    if let Err(error) = preflight.authorize_publication() {
        for shard in shards {
            append_unpublished_requests(
                &mut unpublished_capacity,
                shard.request_indices,
                prepared_into_requests(shard.batch),
                &mut make_unpublished,
            );
        }
        unpublished_capacity.sort_unstable_by_key(&unpublished_index);
        return Err(MultiQueueSdmaPublicationFailureV1 {
            error,
            plan,
            published: published_capacity,
            indeterminate: None,
            unpublished: unpublished_capacity,
        });
    }
    let mut pending = shards.into_iter();
    while let Some(shard) = pending.next() {
        let queue_ordinal = shard.queue_ordinal;
        match publish_shard(queue_ordinal, shard.batch) {
            Ok((queue_id, tickets)) => {
                debug_assert_eq!(tickets.len(), shard.request_indices.len());
                if preflight
                    .record_publication_observation(
                        queue_ordinal,
                        MultiQueuePublicationObservationV1::Confirmed,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                published_capacity.push(make_published(
                    queue_ordinal,
                    queue_id,
                    shard.request_indices,
                    tickets,
                ));
            }
            Err((_, PreparedSdmaPublicationFailureV1::Recoverable { error, prepared })) => {
                if preflight
                    .record_publication_observation(
                        queue_ordinal,
                        MultiQueuePublicationObservationV1::RecoverableNoEffect,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                append_unpublished_requests(
                    &mut unpublished_capacity,
                    shard.request_indices,
                    prepared_into_requests(prepared),
                    &mut make_unpublished,
                );
                for pending_shard in pending {
                    append_unpublished_requests(
                        &mut unpublished_capacity,
                        pending_shard.request_indices,
                        prepared_into_requests(pending_shard.batch),
                        &mut make_unpublished,
                    );
                }
                unpublished_capacity.sort_unstable_by_key(&unpublished_index);
                return Err(MultiQueueSdmaPublicationFailureV1 {
                    error,
                    plan,
                    published: published_capacity,
                    indeterminate: None,
                    unpublished: unpublished_capacity,
                });
            }
            Err((queue_id, PreparedSdmaPublicationFailureV1::Retained { error, tickets })) => {
                debug_assert_eq!(tickets.len(), shard.request_indices.len());
                if preflight
                    .record_publication_observation(
                        queue_ordinal,
                        MultiQueuePublicationObservationV1::Indeterminate,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                let indeterminate = Some(make_published(
                    queue_ordinal,
                    queue_id,
                    shard.request_indices,
                    tickets,
                ));
                for pending_shard in pending {
                    append_unpublished_requests(
                        &mut unpublished_capacity,
                        pending_shard.request_indices,
                        prepared_into_requests(pending_shard.batch),
                        &mut make_unpublished,
                    );
                }
                unpublished_capacity.sort_unstable_by_key(&unpublished_index);
                return Err(MultiQueueSdmaPublicationFailureV1 {
                    error,
                    plan,
                    published: published_capacity,
                    indeterminate,
                    unpublished: unpublished_capacity,
                });
            }
        }
    }
    if !preflight.publication_is_complete() {
        std::process::abort();
    }
    Ok(make_submission(plan, published_capacity))
}

fn append_prepared_requests<R, P>(
    output: &mut Vec<IndexedSdmaRequestV1<R>>,
    shards: Vec<PreparedMultiQueueShardV1<P>>,
    prepared_into_requests: &mut impl FnMut(P) -> Vec<R>,
) {
    for shard in shards {
        append_indexed_requests(
            output,
            shard.request_indices,
            prepared_into_requests(shard.batch),
        );
    }
}

fn append_indexed_requests<R>(
    output: &mut Vec<IndexedSdmaRequestV1<R>>,
    indices: Vec<u16>,
    requests: Vec<R>,
) {
    debug_assert_eq!(indices.len(), requests.len());
    output.extend(
        indices
            .into_iter()
            .zip(requests)
            .map(|(index, request)| IndexedSdmaRequestV1 { index, request }),
    );
}

fn append_unpublished_requests<R, U>(
    output: &mut Vec<U>,
    indices: Vec<u16>,
    requests: Vec<R>,
    make_unpublished: &mut impl FnMut(u16, R) -> U,
) {
    debug_assert_eq!(indices.len(), requests.len());
    output.extend(
        indices
            .into_iter()
            .zip(requests)
            .map(|(request_index, request)| make_unpublished(request_index, request)),
    );
}

#[cfg(test)]
fn multi_queue_custody_is_exact<'a>(
    plan: &Gfx942SdmaMultiQueuePlanV1,
    shards: impl IntoIterator<Item = (usize, &'a [u16])>,
    unpublished: impl IntoIterator<Item = usize>,
) -> bool {
    let mut seen = [false; GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1];
    let mut observed = 0usize;
    for (queue, indices) in shards {
        for index in indices.iter().map(|index| usize::from(*index)) {
            if index >= plan.request_count()
                || seen[index]
                || plan.queue_for_request(index) != Some(queue)
            {
                return false;
            }
            seen[index] = true;
            observed += 1;
        }
    }
    for index in unpublished {
        if index >= plan.request_count() || seen[index] {
            return false;
        }
        seen[index] = true;
        observed += 1;
    }
    observed == plan.request_count() && seen[..plan.request_count()].iter().all(|value| *value)
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
    })
}

pub(crate) fn allocate_device_buffer(
    memory: &mut SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    bytes: u64,
    alignment: u64,
) -> Result<Gfx942SdmaBufferV1, Gfx942SdmaErrorV1> {
    let lease = memory.allocate_gfx942_device_memory(bytes, alignment)?;
    let lease = memory.map_gfx942_device_memory(lease)?;
    Ok(Gfx942SdmaBufferV1 {
        storage: Gfx942SdmaBufferStorageV1::Device(lease),
        owner,
        pool_generation: 1,
        logical_bytes: bytes,
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
    };
    let host = Gfx942SdmaBufferV1 {
        storage: Gfx942SdmaBufferStorageV1::Host(
            crate::shared_memory::mapped_host_for_persistent_sdma_test(id + 1000, 4096),
        ),
        owner,
        pool_generation: 1,
        logical_bytes: 4096,
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

pub(crate) const fn striped_sdma_queue_count_is_admitted(queue_count: u32) -> bool {
    queue_count >= KFD_GFX942_SDMA_ENGINE_COUNT_V1
        && queue_count.is_multiple_of(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
        && queue_count <= KFD_GFX942_SDMA_ENGINE_COUNT_V1 * KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1
}

fn next_striped_owner(current: usize, owner_count: usize) -> Result<usize, Gfx942SdmaErrorV1> {
    if owner_count == 0 || current >= owner_count {
        return Err(Gfx942SdmaErrorV1::Contract("striped SDMA owner cursor"));
    }
    Ok((current + 1) % owner_count)
}

fn cursor_after_multi_queue_outcome(
    current: usize,
    plan: &Gfx942SdmaMultiQueuePlanV1,
    outcome: MultiQueueCursorOutcomeV1,
) -> Result<usize, Gfx942SdmaErrorV1> {
    if plan.first_queue() != current {
        return Err(Gfx942SdmaErrorV1::Contract(
            "multi-queue cursor plan is stale",
        ));
    }
    Ok(match outcome {
        MultiQueueCursorOutcomeV1::CompleteSuccess => plan.next_queue_after_success(),
        #[cfg(test)]
        MultiQueueCursorOutcomeV1::Failure => current,
    })
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

    fn word(packet: &Gfx942SdmaCopySubmissionV1, index: usize) -> u32 {
        let offset = index * 4;
        u32::from_le_bytes(packet.bytes[offset..offset + 4].try_into().unwrap())
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
    fn striped_queue_cursor_is_deterministic_and_wraps() {
        assert_eq!(next_striped_owner(0, 4).unwrap(), 1);
        assert_eq!(next_striped_owner(2, 4).unwrap(), 3);
        assert_eq!(next_striped_owner(3, 4).unwrap(), 0);
        assert!(next_striped_owner(0, 0).is_err());
        assert!(next_striped_owner(4, 4).is_err());
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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum InjectedMultiQueueFaultV1 {
        Preparation { call: usize },
        RecoverablePublication { call: usize },
        IndeterminatePublication { call: usize },
        ClosingCurrentness,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct InjectedMultiQueueOutcomeV1 {
        succeeded: bool,
        confirmed_queues: Vec<usize>,
        confirmed_requests: Vec<usize>,
        indeterminate_queue: Option<usize>,
        indeterminate_requests: Vec<usize>,
        untouched_requests: Vec<usize>,
        cursor: usize,
    }

    struct InjectedShardV1 {
        queue_ordinal: usize,
        queue_id: u32,
        request_indices: Vec<u16>,
        tickets: Vec<usize>,
    }

    struct InjectedSubmissionV1 {
        plan: Gfx942SdmaMultiQueuePlanV1,
        shards: Vec<InjectedShardV1>,
    }

    fn injected_multi_queue_outcome(
        plan: &Gfx942SdmaMultiQueuePlanV1,
        cursor: usize,
        fault: Option<InjectedMultiQueueFaultV1>,
    ) -> InjectedMultiQueueOutcomeV1 {
        fn shard_observations(shards: &[InjectedShardV1]) -> (Vec<usize>, Vec<usize>) {
            let queues = shards
                .iter()
                .map(|shard| {
                    assert_eq!(shard.queue_id, 10 + shard.queue_ordinal as u32);
                    assert_eq!(shard.request_indices.len(), shard.tickets.len());
                    assert_eq!(
                        shard
                            .request_indices
                            .iter()
                            .map(|index| usize::from(*index))
                            .collect::<Vec<_>>(),
                        shard.tickets
                    );
                    shard.queue_ordinal
                })
                .collect();
            let mut requests = shards
                .iter()
                .flat_map(|shard| shard.request_indices.iter().copied())
                .map(usize::from)
                .collect::<Vec<_>>();
            requests.sort_unstable();
            (queues, requests)
        }

        let requests = (0..plan.request_count()).collect::<Vec<_>>();
        let mut preparation_call = 0;
        let prepared: PreparedMultiQueueSdmaBatchV1<Vec<usize>, InjectedShardV1, (u16, usize)> =
            match prepare_multi_queue_batch(
                plan.queue_ids().len(),
                plan.clone(),
                requests,
                |_, requests| {
                    let this_call = preparation_call;
                    preparation_call += 1;
                    if fault == Some(InjectedMultiQueueFaultV1::Preparation { call: this_call }) {
                        Err((
                            Gfx942SdmaErrorV1::Contract("injected preparation failure"),
                            requests,
                        ))
                    } else {
                        Ok(requests)
                    }
                },
                |prepared| prepared,
            ) {
                Ok(prepared) => prepared,
                Err(failure) => {
                    assert!(matches!(
                        fault,
                        Some(InjectedMultiQueueFaultV1::Preparation { .. })
                    ));
                    return InjectedMultiQueueOutcomeV1 {
                        succeeded: false,
                        confirmed_queues: Vec::new(),
                        confirmed_requests: Vec::new(),
                        indeterminate_queue: None,
                        indeterminate_requests: Vec::new(),
                        untouched_requests: failure.requests,
                        cursor,
                    };
                }
            };
        assert_eq!(prepared.published_capacity.len(), 0);
        assert!(prepared.published_capacity.capacity() >= plan.active_shard_count());
        assert_eq!(prepared.unpublished_capacity.len(), 0);
        assert!(prepared.unpublished_capacity.capacity() >= plan.request_count());

        let mut publication_call = 0;
        let published = publish_multi_queue_batch(
            prepared,
            |queue, prepared| {
                let this_call = publication_call;
                publication_call += 1;
                let queue_id = plan.queue_ids()[queue];
                match fault {
                    Some(InjectedMultiQueueFaultV1::RecoverablePublication { call })
                        if call == this_call =>
                    {
                        Err((
                            queue_id,
                            PreparedSdmaPublicationFailureV1::Recoverable {
                                error: Gfx942SdmaErrorV1::Contract(
                                    "injected recoverable publication failure",
                                ),
                                prepared,
                            },
                        ))
                    }
                    Some(InjectedMultiQueueFaultV1::IndeterminatePublication { call })
                        if call == this_call =>
                    {
                        Err((
                            queue_id,
                            PreparedSdmaPublicationFailureV1::Retained {
                                error: Gfx942SdmaErrorV1::Contract(
                                    "injected indeterminate publication failure",
                                ),
                                tickets: prepared,
                            },
                        ))
                    }
                    _ => Ok((queue_id, prepared)),
                }
            },
            |prepared| prepared,
            |queue_ordinal, queue_id, request_indices, tickets| InjectedShardV1 {
                queue_ordinal,
                queue_id,
                request_indices,
                tickets,
            },
            |request_index, request| (request_index, request),
            |request| request.0,
            |plan, shards| InjectedSubmissionV1 { plan, shards },
        );
        match published {
            Ok(submission) => {
                assert_eq!(submission.plan, *plan);
                assert!(submission.shards.capacity() >= plan.active_shard_count());
                let (confirmed_queues, confirmed_requests) = shard_observations(&submission.shards);
                let succeeded = fault != Some(InjectedMultiQueueFaultV1::ClosingCurrentness);
                InjectedMultiQueueOutcomeV1 {
                    succeeded,
                    confirmed_queues,
                    confirmed_requests,
                    indeterminate_queue: None,
                    indeterminate_requests: Vec::new(),
                    untouched_requests: Vec::new(),
                    cursor: if succeeded {
                        cursor_after_multi_queue_outcome(
                            cursor,
                            plan,
                            MultiQueueCursorOutcomeV1::CompleteSuccess,
                        )
                        .unwrap()
                    } else {
                        cursor
                    },
                }
            }
            Err(failure) => {
                assert!(failure.published.capacity() >= plan.active_shard_count());
                assert!(failure.unpublished.capacity() >= plan.request_count());
                let (confirmed_queues, confirmed_requests) = shard_observations(&failure.published);
                let (indeterminate_queue, indeterminate_requests) = failure
                    .indeterminate
                    .as_ref()
                    .map(|shard| {
                        let (_, requests) = shard_observations(std::slice::from_ref(shard));
                        (Some(shard.queue_ordinal), requests)
                    })
                    .unwrap_or((None, Vec::new()));
                let untouched_requests = failure
                    .unpublished
                    .into_iter()
                    .map(|(index, request)| {
                        assert_eq!(usize::from(index), request);
                        request
                    })
                    .collect();
                InjectedMultiQueueOutcomeV1 {
                    succeeded: false,
                    confirmed_queues,
                    confirmed_requests,
                    indeterminate_queue,
                    indeterminate_requests,
                    untouched_requests,
                    cursor,
                }
            }
        }
    }

    #[test]
    fn multi_queue_plan_rejects_invalid_duplicate_and_overcapacity_inputs() {
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[], 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { actual: 0 })
        );
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[1, 2, 3], 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { actual: 3 })
        );
        let too_many_queues = (0..=GFX942_SDMA_MAX_STRIPED_QUEUES_V1 as u32).collect::<Vec<_>>();
        assert!(matches!(
            Gfx942SdmaMultiQueuePlanV1::new(&too_many_queues, 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { .. })
        ));
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8, 7, 9], 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { queue_id: 7 })
        );
        assert!(matches!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 0, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { actual: 0, .. })
        ));
        assert!(matches!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 2 * GFX942_SDMA_MAX_IN_FLIGHT_V1 + 1, 0,),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { .. })
        ));
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 1, 2),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor {
                actual: 2,
                queue_count: 2,
            })
        );
    }

    #[test]
    fn multi_queue_plan_is_balanced_deterministic_fair_and_current() {
        let queue_ids = [10, 11, 12, 13];
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 10, 2).unwrap();
        assert_eq!(
            (0..10)
                .map(|index| plan.queue_for_request(index).unwrap())
                .collect::<Vec<_>>(),
            [2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
        );
        assert_eq!(
            (0..4)
                .map(|queue| plan.shard_count(queue).unwrap())
                .collect::<Vec<_>>(),
            [2, 2, 3, 3]
        );
        assert!(plan.is_balanced());
        assert_eq!(plan.active_shard_count(), 4);
        assert_eq!(plan.next_queue_after_success(), 0);
        assert!(plan.is_current_for(&queue_ids, 2));
        assert!(!plan.is_current_for(&[10, 12, 11, 13], 2));
        assert!(!plan.is_current_for(&queue_ids, 1));

        let mut cursor = 0;
        let mut selected = Vec::new();
        for _ in 0..8 {
            let single = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 1, cursor).unwrap();
            selected.push(single.queue_for_request(0).unwrap());
            cursor = single.next_queue_after_success();
        }
        assert_eq!(selected, [0, 1, 2, 3, 0, 1, 2, 3]);
    }

    #[test]
    fn multi_queue_cursor_advances_only_after_complete_success() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 3, 1).unwrap();
        assert_eq!(
            cursor_after_multi_queue_outcome(1, &plan, MultiQueueCursorOutcomeV1::CompleteSuccess,)
                .unwrap(),
            0
        );
        for failure_stage in ["preparation", "partial-publication", "terminal"] {
            assert_eq!(
                cursor_after_multi_queue_outcome(1, &plan, MultiQueueCursorOutcomeV1::Failure,)
                    .unwrap(),
                1,
                "{failure_stage} failure advanced the cursor",
            );
        }
        assert!(
            cursor_after_multi_queue_outcome(0, &plan, MultiQueueCursorOutcomeV1::CompleteSuccess,)
                .is_err()
        );
    }

    #[test]
    fn multi_queue_partial_progress_accounting_is_exact() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 10, 2).unwrap();
        let published = [(2, [0_u16, 4, 8].as_slice())];
        let indeterminate = [(3, [1_u16, 5, 9].as_slice())];
        let unpublished = [2_usize, 3, 6, 7];
        assert!(multi_queue_custody_is_exact(
            &plan,
            published.into_iter().chain(indeterminate),
            unpublished,
        ));
        assert!(!multi_queue_custody_is_exact(
            &plan,
            [(2, [0_u16, 4, 8].as_slice()), (3, [1_u16, 5, 9].as_slice())],
            [2_usize, 3, 6, 6],
        ));
        assert!(!multi_queue_custody_is_exact(
            &plan,
            [(1, [0_u16, 4, 8].as_slice()), (3, [1_u16, 5, 9].as_slice())],
            unpublished,
        ));
    }

    #[test]
    fn multi_queue_preflight_gate_rejects_hostile_ordering_without_publication() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 4, 2).unwrap();
        let mut preflight = MultiQueuePreflightStateV1::new(&plan);
        assert!(!preflight.publication_authorized);
        assert!(
            preflight
                .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
                .is_err()
        );
        assert!(preflight.record_prepared_queue(4).is_err());
        assert!(preflight.record_prepared_queue(2).is_ok());
        assert!(preflight.record_prepared_queue(2).is_err());
        assert!(preflight.authorize_publication().is_err());
        assert!(!preflight.publication_authorized);
        assert!(preflight.record_prepared_queue(3).is_ok());
        assert!(preflight.record_prepared_queue(0).is_ok());
        assert!(preflight.record_prepared_queue(1).is_ok());
        assert!(preflight.authorize_publication().is_ok());
        assert!(preflight.publication_authorized);
        assert!(preflight.record_prepared_queue(0).is_err());
        assert!(preflight.authorize_publication().is_err());
        assert!(
            preflight
                .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
                .is_ok()
        );
        assert!(
            preflight
                .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
                .is_err()
        );
        assert!(!preflight.publication_is_complete());
    }

    #[test]
    fn multi_queue_injected_coordinator_reports_exact_custody_and_cursor_outcomes() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 10, 2).unwrap();

        assert_eq!(
            injected_multi_queue_outcome(&plan, 2, None),
            InjectedMultiQueueOutcomeV1 {
                succeeded: true,
                confirmed_queues: vec![2, 3, 0, 1],
                confirmed_requests: (0..10).collect(),
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: vec![],
                cursor: 0,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::Preparation { call: 1 }),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![],
                confirmed_requests: vec![],
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: (0..10).collect(),
                cursor: 2,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::RecoverablePublication { call: 1 }),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![2],
                confirmed_requests: vec![0, 4, 8],
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: vec![1, 2, 3, 5, 6, 7, 9],
                cursor: 2,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::IndeterminatePublication { call: 1 }),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![2],
                confirmed_requests: vec![0, 4, 8],
                indeterminate_queue: Some(3),
                indeterminate_requests: vec![1, 5, 9],
                untouched_requests: vec![2, 3, 6, 7],
                cursor: 2,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::ClosingCurrentness),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![2, 3, 0, 1],
                confirmed_requests: (0..10).collect(),
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: vec![],
                cursor: 2,
            }
        );
    }

    #[test]
    fn multi_queue_publication_requires_fully_prepared_private_custody() {
        let source = include_str!("sdma.rs");
        let coordinator = source
            .split("pub(crate) fn submit_striped_multi_queue_batch")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn directional_observation")
            .next()
            .unwrap();
        let prepare = coordinator.find("prepare_multi_queue_batch").unwrap();
        let publish = coordinator.find("publish_multi_queue_batch").unwrap();
        assert!(prepare < publish);

        let prepare_body = source
            .split("fn prepare_multi_queue_batch<")
            .nth(1)
            .unwrap()
            .split("fn publish_multi_queue_batch<")
            .next()
            .unwrap();
        assert!(coordinator.contains("prepare_batch_recoverable"));
        assert!(coordinator.contains("submit_prepared_batch_with_custody"));
        assert!(prepare_body.contains("prepare_shard(queue, queue_requests)"));
        assert!(!prepare_body.contains("publish_shard(queue_ordinal"));

        let publish_body = source
            .split("fn publish_multi_queue_batch<")
            .nth(1)
            .unwrap()
            .split("fn append_prepared_requests")
            .next()
            .unwrap();
        assert!(publish_body.contains("publish_shard(queue_ordinal, shard.batch)"));
        assert!(!publish_body.contains("try_reserve"));
        assert!(!publish_body.contains("Vec::new"));
        assert!(!publish_body.contains(".collect"));
        assert!(!publish_body.contains("to_string"));
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
        let digest = Sha256::digest(GFX942_SDMA_COPY_MANIFEST_V1);
        let mut rendered = String::with_capacity(64);
        for byte in digest {
            use core::fmt::Write;
            write!(&mut rendered, "{byte:02x}").unwrap();
        }
        assert_eq!(rendered, GFX942_SDMA_COPY_MANIFEST_SHA256_V1);
    }
}
