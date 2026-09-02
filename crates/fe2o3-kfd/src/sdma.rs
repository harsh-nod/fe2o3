//! Bounded gfx942 SDMA copy queue.
//!
//! Packet construction and ownership transitions are checked locally. Native
//! execution remains conditional on the pinned KFD, firmware, coherency, and
//! GPU memory-system contracts.

use core::fmt;
use std::time::{Duration, Instant};

use fe2o3_kfd_uapi::{
    KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs, KfdSdmaQueueBuffers,
    admit_kfd_aql_queue_ring_size, admit_kfd_gfx942_create_queue_outputs,
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
    AqlControlResourceRoleV1, AqlQueueGttV1, AqlRingResourceRoleV1, Gfx942DeviceMemoryLeaseV1,
    Gfx942DeviceMemoryMappedV1, GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1,
    SharedGttAllocationV1, SharedGttMemorySessionV1, SharedGttQueueResourceAuthorityV1,
    UserptrAqlControlGttV1,
};

pub const GFX942_SDMA_COPY_PACKET_BYTES_V1: usize = 7 * 4;
pub const GFX942_SDMA_FENCE_PACKET_BYTES_V1: usize = 4 * 4;
pub const GFX942_SDMA_SUBMISSION_BYTES_V1: usize = 64;
pub const GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1: u32 = 0x003f_ffe0;
pub const GFX942_SDMA_RING_BYTES_V1: u32 = 4_096;
pub const GFX942_SDMA_MAX_IN_FLIGHT_V1: usize =
    GFX942_SDMA_RING_BYTES_V1 as usize / GFX942_SDMA_SUBMISSION_BYTES_V1;

/// Frozen claim boundary for the bounded native gfx942 SDMA implementation.
pub const GFX942_SDMA_COPY_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-sdma-copy-r1-v1\n",
    "kfd_sdma_queue_schema_sha256=bd862d85fcf5c4c3ae972e109777079fd22ade9f00dcc779415031605c998baf\n",
    "rocm_systems_commit=1b648038a0ac164cf2f06f2a581ced12cf5f7378\n",
    "rocr_sdma_registers_sha256=0287a021439e49cd3075bd88c8f9f4558f20ad16e8f473f59732aa803c62df5b\n",
    "rocr_blit_sdma_sha256=f4d0be236a034cd9ad44b9dd196f4498bcf9dedb89a7812a217b988aef1ff359\n",
    "packet=copy-linear-28-bytes,count-minus-one,source-u64,destination-u64;fence-16-bytes,mtype-3,sys-1,snp-1,u32-generation;zero-pad-to-64\n",
    "bounds=copy:1..4194272,ring:4096,submission:64,in-flight:64,nonoverlap\n",
    "memory=move-only-host-coherent-or-device-local,logical-subrange-bounded,queue-retained-while-in-flight\n",
    "submission=single-producer,write-reservation-before-ring-publication,release-doorbell-per-submission,queue-occurrence-and-generation-tagged-ticket\n",
    "completion=host-coherent-u32-fence-value-observed-through-i64-acquire,exact-generation,deadline-wait,custody-returned-only-after-observation\n",
    "pool=queue-branded,best-fit-by-kind-size-and-alignment,leased-and-in-flight-excluded,concrete-generation-advanced-on-recycle,explicit-trim-before-teardown\n",
    "currentness=one-operational-pre-post-envelope-per-submit-batch-or-wait-batch,internal-atomics-and-mapped-writes-only-inside-envelope\n",
    "failure=structural-preflight-and-ordinary-capacity-rejection-recover-inputs,currentness-counter-generation-and-post-preflight-uncertainty-terminally-poison-and-retain-native-custody\n",
    "teardown=destroy-sdma-before-compute,then-release-ring-control-completions-and-pooled-buffers-explicitly\n",
    "proof=abstract-pool-generation-retention-and-cross-device-coordinate-theorems-only,no-executable-rust-refinement\n",
    "contracted=ioctl-truth,doorbell-mapping,cpu-gpu-coherence,kernel-firmware-packet-consumption,completion,progress,liveness\n",
    "measured=hardware-correctness-and-performance-on-identified-host-only\n",
);

/// SHA-256 of [`GFX942_SDMA_COPY_MANIFEST_V1`].
pub const GFX942_SDMA_COPY_MANIFEST_SHA256_V1: &str =
    "a1a2f3cb07b67e8f66d89578d278853d5750b1a0ad862f0edd27c2fb1ef7b4ec";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SdmaBufferKindV1 {
    HostVisibleCoherent,
    DeviceLocal,
}

enum Gfx942SdmaBufferStorageV1 {
    Host(MappedHostBufferV1),
    Device(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
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

    fn checked_gpu_subrange(
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
    copy_bytes: u32,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaQueueObservationV1 {
    pub queue_id: u32,
    pub ring_bytes: u32,
    pub maximum_in_flight: u16,
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
}

pub(crate) struct Gfx942SdmaQueueOwnerV1 {
    owner: QueueKeyV1,
    queue_id: u32,
    ring: Option<SdmaRingAuthorityV1>,
    control: Option<SdmaControlAuthorityV1>,
    completions: Option<MappedHostBufferV1>,
    doorbell: Option<LinuxDoorbellSliceV1>,
    records: Vec<Option<SdmaCopyRecordV1>>,
    generations: [u32; GFX942_SDMA_MAX_IN_FLIGHT_V1],
    destroyed: bool,
    poisoned: bool,
}

impl Gfx942SdmaQueueOwnerV1 {
    pub(crate) fn create(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        let mut records = Vec::new();
        records
            .try_reserve_exact(GFX942_SDMA_MAX_IN_FLIGHT_V1)
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA record roster allocation"))?;
        records.resize_with(GFX942_SDMA_MAX_IN_FLIGHT_V1, || None);
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
        let expected = KfdIoctlCreateQueueArgs::new_sdma(
            buffers,
            admit_kfd_aql_queue_ring_size(GFX942_SDMA_RING_BYTES_V1)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA ring size"))?,
            memory.gpu_id(),
            admit_kfd_queue_percentage(100)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA queue percentage"))?,
            admit_kfd_queue_priority(0)
                .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA queue priority"))?,
        );
        let mut actual = expected;
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
            .map_err(|error| Gfx942SdmaErrorV1::Doorbell(error.to_string()))?;
        memory.check_queue_currentness()?;

        Ok(Self {
            owner,
            queue_id,
            ring: Some(ring),
            control: Some(control),
            completions: Some(completions),
            doorbell: Some(doorbell),
            records,
            generations: [0; GFX942_SDMA_MAX_IN_FLIGHT_V1],
            destroyed: false,
            poisoned: false,
        })
    }

    pub(crate) const fn observation(&self) -> Gfx942SdmaQueueObservationV1 {
        Gfx942SdmaQueueObservationV1 {
            queue_id: self.queue_id,
            ring_bytes: GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
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
        if read > write || write - read > u64::from(GFX942_SDMA_RING_BYTES_V1) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        if write - read + GFX942_SDMA_SUBMISSION_BYTES_V1 as u64
            > u64::from(GFX942_SDMA_RING_BYTES_V1)
        {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let ring_slot = ((write % u64::from(GFX942_SDMA_RING_BYTES_V1))
            / GFX942_SDMA_SUBMISSION_BYTES_V1 as u64) as usize;
        if self.records[ring_slot].is_some() {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        Ok(())
    }

    pub(crate) fn ensure_batch_capacity(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        count: usize,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_live()?;
        if count == 0 || count > GFX942_SDMA_MAX_IN_FLIGHT_V1 {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let control = self.control.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing SDMA control authority",
        ))?;
        let (write, read) = memory.observe_aql_control_counters_in_current_scope(control)?;
        let requested = (count as u64)
            .checked_mul(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA batch byte count"))?;
        if read > write || write - read > u64::from(GFX942_SDMA_RING_BYTES_V1) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        if write - read + requested > u64::from(GFX942_SDMA_RING_BYTES_V1) {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        for index in 0..count {
            let offset = (index as u64)
                .checked_mul(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
                .and_then(|offset| write.checked_add(offset))
                .ok_or(Gfx942SdmaErrorV1::Contract("SDMA batch slot offset"))?;
            let slot = ((offset % u64::from(GFX942_SDMA_RING_BYTES_V1))
                / GFX942_SDMA_SUBMISSION_BYTES_V1 as u64) as usize;
            if self.records[slot].is_some() {
                return Err(Gfx942SdmaErrorV1::QueueFull);
            }
        }
        Ok(())
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
        if read > write || write - read > u64::from(GFX942_SDMA_RING_BYTES_V1) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract("invalid SDMA queue counters"));
        }
        if write - read + GFX942_SDMA_SUBMISSION_BYTES_V1 as u64
            > u64::from(GFX942_SDMA_RING_BYTES_V1)
        {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let ring_slot = ((write % u64::from(GFX942_SDMA_RING_BYTES_V1))
            / GFX942_SDMA_SUBMISSION_BYTES_V1 as u64) as usize;
        if self.records[ring_slot].is_some() {
            return Err(Gfx942SdmaErrorV1::QueueFull);
        }
        let generation = self.generations[ring_slot]
            .checked_add(1)
            .filter(|value| *value != 0)
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "SDMA ticket generation exhausted",
            ))?;
        let completion_value = generation;
        let completion_offset = (ring_slot * 8) as u64;
        let completions = self
            .completions
            .as_mut()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA completion arena"))?;
        memory.overwrite_mapped_host_visible_subrange_in_current_scope(
            completions,
            completion_offset,
            &[0; 8],
        )?;
        let completion_address = memory
            .mapped_resource_facts(completions)?
            .checked_gpu_subrange(completion_offset, 4, 4)
            .ok_or(Gfx942SdmaErrorV1::Contract("SDMA completion address"))?;
        let packet = Gfx942SdmaCopySubmissionV1::new(
            source_address,
            destination_address,
            copy_bytes,
            completion_address,
            completion_value,
        )?;
        let reserved = memory.fetch_add_aql_control_write_in_current_scope(
            control,
            GFX942_SDMA_SUBMISSION_BYTES_V1 as u64,
        )?;
        if reserved != write {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract(
                "SDMA single-producer counter changed",
            ));
        }
        memory.write_aql_ring_slot_in_current_scope(
            self.ring
                .as_mut()
                .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA ring authority"))?,
            ring_slot as u32,
            packet.bytes(),
        )?;
        self.doorbell
            .as_mut()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA doorbell"))?
            .store_packet_id_release(
                write
                    .checked_add(GFX942_SDMA_SUBMISSION_BYTES_V1 as u64)
                    .ok_or(Gfx942SdmaErrorV1::Contract("SDMA doorbell value"))?,
            )
            .map_err(|error| Gfx942SdmaErrorV1::Doorbell(error.to_string()))?;
        self.generations[ring_slot] = generation;
        self.records[ring_slot] = Some(SdmaCopyRecordV1 {
            generation,
            completion_value,
            completion_observed: false,
            source,
            destination,
            copy_bytes,
        });
        Ok(Gfx942SdmaCopyTicketV1 {
            owner: self.owner,
            queue_id: self.queue_id,
            slot: ring_slot as u16,
            generation,
        })
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
            if Instant::now() >= deadline {
                memory.check_queue_operational_currentness()?;
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            core::hint::spin_loop();
        }
        memory.check_queue_operational_currentness()?;
        let record = self.records[slot].take().expect("completed SDMA record");
        Ok(Gfx942SdmaCompletedCopyV1 {
            source: record.source,
            destination: record.destination,
            copy_bytes: record.copy_bytes,
        })
    }

    pub(crate) fn wait_many_for(
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
        let mut ready = Vec::new();
        ready
            .try_reserve_exact(slots.len())
            .map_err(|_| Gfx942SdmaErrorV1::Contract("SDMA ready roster allocation"))?;
        ready.resize(slots.len(), false);
        memory.check_queue_operational_currentness()?;
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
            if Instant::now() >= deadline {
                memory.check_queue_operational_currentness()?;
                return Err(Gfx942SdmaErrorV1::Timeout);
            }
            core::hint::spin_loop();
        }
        memory.check_queue_operational_currentness()?;
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
            });
        }
        Ok(completed)
    }

    pub(crate) fn destroy_queue(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_live()?;
        if self.records.iter().any(Option::is_some) {
            return Err(Gfx942SdmaErrorV1::Pending);
        }
        memory.check_queue_currentness()?;
        let mut args = KfdIoctlDestroyQueueArgs::new(self.queue_id);
        destroy_queue(memory.kfd_fd(), &mut args)
            .map_err(|_| Gfx942SdmaErrorV1::QueueDestroyIndeterminate)?;
        if args != KfdIoctlDestroyQueueArgs::new(self.queue_id) {
            self.poisoned = true;
            return Err(Gfx942SdmaErrorV1::Contract(
                "kernel changed immutable SDMA DESTROY_QUEUE inputs",
            ));
        }
        self.doorbell
            .take()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing SDMA doorbell"))?
            .release()
            .map_err(|error| Gfx942SdmaErrorV1::Doorbell(error.to_string()))?;
        self.destroyed = true;
        memory.check_queue_currentness()?;
        Ok(())
    }

    pub(crate) fn release_resources(
        mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        if !self.destroyed || self.poisoned || self.records.iter().any(Option::is_some) {
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

    fn require_live(&self) -> Result<(), Gfx942SdmaErrorV1> {
        if self.destroyed || self.poisoned {
            return Err(Gfx942SdmaErrorV1::Contract("SDMA queue is not live"));
        }
        Ok(())
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }
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

fn exact_queue_owner(left: QueueKeyV1, right: QueueKeyV1) -> bool {
    left == right
}

fn ticket_matches_queue_occurrence(
    ticket: Gfx942SdmaCopyTicketV1,
    owner: QueueKeyV1,
    queue_id: u32,
) -> bool {
    ticket.owner == owner && ticket.queue_id == queue_id
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
    fn sdma_copy_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_SDMA_COPY_MANIFEST_V1);
        let mut rendered = String::with_capacity(64);
        for byte in digest {
            use core::fmt::Write;
            write!(&mut rendered, "{byte:02x}").unwrap();
        }
        assert_eq!(rendered, GFX942_SDMA_COPY_MANIFEST_SHA256_V1);
    }
}
