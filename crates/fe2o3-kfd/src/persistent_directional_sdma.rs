//! Queue-bound persistent-allocation custody for one directional local SDMA pair.
//!
//! This R19 surface extends, rather than changes, the frozen R18 targeted
//! adapter. Public values remain addressless, move-only custody receipts.

use std::fmt;

use fe2o3_kfd_uapi::{KFD_GFX942_SDMA_ENGINE_COUNT_V1, KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1};
use fe2o3_runtime_model::QueueKeyV1;

use crate::persistent_allocation::{
    Gfx942PersistentDependencyFrontierV1, Gfx942PersistentDeviceAllocationV1,
    Gfx942PersistentPreparedV1, Gfx942PersistentPublishedV1, Gfx942PersistentQuarantineReasonV1,
    Gfx942PersistentUseErrorV1, Gfx942PersistentUseLeaseV1, Gfx942PersistentUseRequestV1,
};
use crate::persistent_sdma::Gfx942PersistentSdmaDirectionV1;
use crate::queue::ComputeAqlQueueSessionErrorV1;
use crate::sdma::{
    CompletedPersistentSdmaWindowV1, GFX942_SDMA_D2H_ENGINE_INDEX_V1,
    GFX942_SDMA_H2D_ENGINE_INDEX_V1, GFX942_SDMA_MAX_IN_FLIGHT_V1,
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942DirectionalSdmaQueueObservationV1,
    Gfx942SdmaBufferKindV1, Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferStorageV1,
    Gfx942SdmaBufferV1, Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyRequestV1, Gfx942SdmaCopyTicketV1,
    planned_ticket_matches_queue_occurrence,
};

pub const GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_ALLOCATION_BYTES_V1: u64 = 256 * 1024 * 1024;
pub const GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1: usize =
    GFX942_SDMA_MAX_IN_FLIGHT_V1;
pub const GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_BYTES_V1: u64 =
    GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1 as u64
        * GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64;

pub const GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_WINDOW_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-persistent-directional-local-sdma-r22-window-v1\n",
    "parent=fe2o3-gfx942-kfd-persistent-directional-local-sdma-r19-v1\n",
    "window=one-homogeneous-direction,one-persistent-host-device-owner-pair,one-aggregate-ledger-use,1..63-contiguous-linear-copy-packets\n",
    "publication=all-fallible-capacity-range-packet-completion-and-ticket-preflight-before-native-mutation,all-records-before-mapped-writes,one-release-write-pointer-publication,one-final-release-doorbell\n",
    "completion=ordered-full-ticket-authentication,pending-and-timeout-retain-whole-window,no-independent-packet-retirement,exact-full-completion-restores-one-owner-pair-and-settles-one-frontier\n",
    "failure=clean-prepublication-restores-exact-pair,retained-or-postpublication-ambiguity-quarantines-whole-window-for-process-teardown\n",
    "limits=local-h2d-or-d2h-only,no-striped-set,no-peer-or-xgmi,no-compute,no-concurrent-range-borrows\n",
    "evidence=native-neutral-host-tests-only,no-native-hardware-execution-or-performance-evidence\n",
    "proof=abstract-model-separate,no-executable-rust-or-native-refinement\n",
);

pub const GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_WINDOW_MANIFEST_SHA256_V1: &str =
    "44821351a14664f9be3db9fc406ee9f4961d4f40a4346fdb085886ecfc84c2aa";

pub const GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_ADAPTER_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-persistent-directional-local-sdma-r19-v1\n",
    "target=gfx942:xnack-,one-directional-ordinary-sdma-queue-pair,engine-1-h2d-and-engine-0-d2h\n",
    "admission=one-existing-queue-owned-device-buffer,1..logical<=physical<=268435456-bytes,one-ordinary-host-buffer-per-use\n",
    "binding=exact-parent-queue-occurrence,distinct-h2d-and-d2h-child-queue-ids-and-engines,pool-generation,logical-and-physical-extents,mapped-storage-identity,persistent-owner-incarnation,host-storage-identity-and-extents,planned-full-ticket\n",
    "ledger=one-existing-sdma-outstanding-buffer-debit-preserved-across-promotion-submission-completion-and-demotion\n",
    "lifecycle=explicit-direction,reserve-prepare-detach-submit,confirmed-only-publish,pending-or-timeout-retains-submission,exact-completion-restores-completes-settles,exact-frontier-retirement-required-before-next-use\n",
    "failure=recoverable-prepublication-restores-and-cancels,retained-publication-quarantines-prepared,postpublication-uncertainty-is-opaque-process-teardown-and-session-poison\n",
    "limits=single-flight,no-striped-set,no-peer-or-xgmi,no-compute,no-concurrent-range-borrows\n",
    "evidence=native-neutral-host-custody-and-failure-injection-tests-only,no-native-hardware-execution-or-performance-evidence\n",
    "proof=abstract-model-separate,no-executable-rust-or-native-refinement\n",
);

pub const GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_ADAPTER_MANIFEST_SHA256_V1: &str =
    "c04f67240eecff85cffb092a228554c88a72cb89f1d49865c123db559cfae319";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942PersistentDirectionalSdmaPairV1 {
    pub(crate) host_to_device_queue_id: u32,
    pub(crate) device_to_host_queue_id: u32,
}

impl Gfx942PersistentDirectionalSdmaPairV1 {
    pub(crate) const fn queue_id(self, direction: Gfx942PersistentSdmaDirectionV1) -> u32 {
        match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => self.host_to_device_queue_id,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => self.device_to_host_queue_id,
        }
    }
}

pub(crate) fn admit_persistent_directional_sdma_pair_v1(
    observation: Gfx942DirectionalSdmaQueueObservationV1,
) -> Result<Gfx942PersistentDirectionalSdmaPairV1, &'static str> {
    if observation.host_to_device.engine_index != Some(GFX942_SDMA_H2D_ENGINE_INDEX_V1)
        || observation.device_to_host.engine_index != Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1)
        || observation.admitted_engine_count != KFD_GFX942_SDMA_ENGINE_COUNT_V1
        || observation.admitted_queues_per_engine != KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1
    {
        return Err("persistent directional SDMA engine ordering");
    }
    if observation.host_to_device.queue_id == observation.device_to_host.queue_id {
        return Err("persistent directional SDMA child queue identity");
    }
    Ok(Gfx942PersistentDirectionalSdmaPairV1 {
        host_to_device_queue_id: observation.host_to_device.queue_id,
        device_to_host_queue_id: observation.device_to_host.queue_id,
    })
}

pub(crate) const fn directional_persistent_sdma_extents_are_admitted_v1(
    logical_bytes: u64,
    physical_bytes: u64,
    pool_generation: u64,
) -> bool {
    logical_bytes != 0
        && logical_bytes <= physical_bytes
        && physical_bytes <= GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_ALLOCATION_BYTES_V1
        && physical_bytes.is_multiple_of(crate::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
        && pool_generation != 0
}

pub(crate) const fn directional_persistent_sdma_queue_destroy_is_admitted_v1(
    outstanding_buffers: usize,
) -> bool {
    outstanding_buffers == 0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942PersistentDirectionalSdmaAttachmentV1 {
    pub(crate) queue: QueueKeyV1,
    pub(crate) pair: Gfx942PersistentDirectionalSdmaPairV1,
    pub(crate) pool_generation: u64,
    pub(crate) logical_bytes: u64,
    pub(crate) physical_bytes: u64,
    pub(crate) storage_identity: Gfx942SdmaBufferStorageIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942PersistentDirectionalSdmaHostBindingV1 {
    queue: QueueKeyV1,
    storage_identity: Gfx942SdmaBufferStorageIdentityV1,
    pool_generation: u64,
    logical_bytes: u64,
    physical_bytes: u64,
}

impl Gfx942PersistentDirectionalSdmaHostBindingV1 {
    pub(crate) fn capture(host: &Gfx942SdmaBufferV1, queue: QueueKeyV1) -> Self {
        Self {
            queue,
            storage_identity: host.storage_identity(),
            pool_generation: host.pool_generation(),
            logical_bytes: host.requested_bytes(),
            physical_bytes: host.physical_bytes(),
        }
    }

    pub(crate) fn matches(self, host: &Gfx942SdmaBufferV1) -> bool {
        host.belongs_to(self.queue)
            && host.storage_identity() == self.storage_identity
            && host.pool_generation() == self.pool_generation
            && host.requested_bytes() == self.logical_bytes
            && host.physical_bytes() == self.physical_bytes
    }
}

/// One device allocation bound to an exact directional child-queue pair.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DirectionalQueuePersistentAllocationV1;
/// fn cannot_clone(value: Gfx942DirectionalQueuePersistentAllocationV1) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DirectionalQueuePersistentAllocationV1;
/// fn require_send<T: Send>(_: T) {}
/// fn cannot_send(value: Gfx942DirectionalQueuePersistentAllocationV1) {
///     require_send(value);
/// }
/// ```
#[must_use = "directional persistent allocation custody must be retained or demoted"]
pub struct Gfx942DirectionalQueuePersistentAllocationV1 {
    pub(crate) owner: Gfx942PersistentDeviceAllocationV1,
    pub(crate) attachment: Gfx942PersistentDirectionalSdmaAttachmentV1,
}

impl Gfx942DirectionalQueuePersistentAllocationV1 {
    pub const fn byte_len(&self) -> u64 {
        self.attachment.logical_bytes
    }

    pub const fn physical_byte_len(&self) -> u64 {
        self.attachment.physical_bytes
    }

    #[allow(clippy::result_large_err)]
    pub fn retire_settled_frontier_v1(
        mut self,
        frontier: Gfx942PersistentDependencyFrontierV1,
    ) -> Result<Self, Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1> {
        match self.owner.retire_settled_frontier(frontier) {
            Ok(()) => Ok(self),
            Err(frontier) => Err(Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1 {
                allocation: self,
                frontier,
            }),
        }
    }
}

impl fmt::Debug for Gfx942DirectionalQueuePersistentAllocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DirectionalQueuePersistentAllocationV1")
            .field("byte_len", &self.byte_len())
            .field("physical_byte_len", &self.physical_byte_len())
            .finish_non_exhaustive()
    }
}

#[must_use = "a rejected retirement returns both custody inputs"]
pub struct Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1 {
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    frontier: Gfx942PersistentDependencyFrontierV1,
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1")
            .field("allocation", &self.allocation)
            .field("frontier", &self.frontier)
            .finish()
    }
}

impl Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1 {
    pub fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        (self.allocation, self.frontier)
    }
}

#[must_use = "terminal promotion custody must be retained until process teardown"]
pub struct Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1 {
    pub(crate) buffer: Gfx942SdmaBufferV1,
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = &self.buffer;
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1")
            .finish_non_exhaustive()
    }
}

#[must_use = "retain retryable or process-teardown promotion custody"]
pub enum Gfx942DirectionalPersistentSdmaPromotionCustodyV1 {
    Retryable(Gfx942SdmaBufferV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1),
}

#[must_use = "a promotion failure always returns explicit custody"]
pub struct Gfx942DirectionalPersistentSdmaPromotionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942DirectionalPersistentSdmaPromotionCustodyV1,
}

impl Gfx942DirectionalPersistentSdmaPromotionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942DirectionalPersistentSdmaPromotionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

pub(crate) fn classify_directional_persistent_sdma_promotion_failure_v1(
    error: ComputeAqlQueueSessionErrorV1,
    buffer: Gfx942SdmaBufferV1,
    process_teardown: bool,
) -> Gfx942DirectionalPersistentSdmaPromotionFailureV1 {
    Gfx942DirectionalPersistentSdmaPromotionFailureV1 {
        error,
        custody: if process_teardown {
            Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1 { buffer },
            )
        } else {
            Gfx942DirectionalPersistentSdmaPromotionCustodyV1::Retryable(buffer)
        },
    }
}

#[must_use = "terminal demotion custody must be retained until process teardown"]
pub struct Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = &self.allocation;
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1")
            .finish_non_exhaustive()
    }
}

#[must_use = "retain retryable or process-teardown demotion custody"]
pub enum Gfx942DirectionalPersistentSdmaDemotionCustodyV1 {
    Retryable(Gfx942DirectionalQueuePersistentAllocationV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1),
}

#[must_use = "a demotion failure always returns explicit custody"]
pub struct Gfx942DirectionalPersistentSdmaDemotionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942DirectionalPersistentSdmaDemotionCustodyV1,
}

impl Gfx942DirectionalPersistentSdmaDemotionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942DirectionalPersistentSdmaDemotionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

pub(crate) fn classify_directional_persistent_sdma_demotion_failure_v1(
    error: ComputeAqlQueueSessionErrorV1,
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    process_teardown: bool,
) -> Gfx942DirectionalPersistentSdmaDemotionFailureV1 {
    Gfx942DirectionalPersistentSdmaDemotionFailureV1 {
        error,
        custody: if process_teardown {
            Gfx942DirectionalPersistentSdmaDemotionCustodyV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1 { allocation },
            )
        } else {
            Gfx942DirectionalPersistentSdmaDemotionCustodyV1::Retryable(allocation)
        },
    }
}

#[must_use = "published directional persistent SDMA custody must be observed"]
pub struct Gfx942DirectionalPersistentSdmaSubmissionV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    pub(crate) ticket: Gfx942SdmaCopyTicketV1,
    pub(crate) host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) host_offset: u64,
    pub(crate) device_offset: u64,
    pub(crate) copy_bytes: u32,
}

impl Gfx942DirectionalPersistentSdmaSubmissionV1 {
    pub const fn request(&self) -> Gfx942PersistentUseRequestV1 {
        self.published.request()
    }

    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub const fn host_offset(&self) -> u64 {
        self.host_offset
    }

    pub const fn device_offset(&self) -> u64 {
        self.device_offset
    }
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaSubmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaSubmissionV1")
            .field("request", &self.request())
            .field("direction", &self.direction)
            .field("copy_bytes", &self.copy_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942DirectionalPersistentSdmaTerminalStageV1 {
    AdmissionRestored,
    PreparedRestored,
    PreparedUnrestored,
    PreparedQueueRetained,
    PublishedQueueRetained,
    CompletedUnrestored,
    /// A synchronous transition could not settle its retained use lease.
    SynchronousUnsettled,
}

#[allow(dead_code)]
#[allow(
    clippy::large_enum_variant,
    reason = "terminal custody must not allocate after native effects"
)]
pub(crate) enum Gfx942DirectionalPersistentSdmaTerminalStateV1 {
    Synchronous(crate::queue::SdmaSynchronousCustodyV1),
    AdmissionRestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    PreparedRestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    PreparedUnrestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        request: Gfx942SdmaCopyRequestV1,
    },
    PreparedQueueRetained {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
    PublishedQueueRetained {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
    CompletedUnrestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        completed: Gfx942SdmaCompletedCopyV1,
    },
}

#[must_use = "terminal native custody must be retained until process teardown"]
pub struct Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) sequence: Option<u64>,
    pub(crate) state: Gfx942DirectionalPersistentSdmaTerminalStateV1,
}

impl Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn sequence(&self) -> Option<u64> {
        self.sequence
    }

    pub const fn stage(&self) -> Gfx942DirectionalPersistentSdmaTerminalStageV1 {
        match &self.state {
            Gfx942DirectionalPersistentSdmaTerminalStateV1::Synchronous(root) => root.stage,
            Gfx942DirectionalPersistentSdmaTerminalStateV1::AdmissionRestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored
            }
            Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedRestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedRestored
            }
            Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedUnrestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
            }
            Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedQueueRetained { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained
            }
            Gfx942DirectionalPersistentSdmaTerminalStateV1::PublishedQueueRetained { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
            }
            Gfx942DirectionalPersistentSdmaTerminalStateV1::CompletedUnrestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
            }
        }
    }
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaTerminalCustodyV1")
            .field("direction", &self.direction)
            .field("sequence", &self.sequence)
            .field("stage", &self.stage())
            .finish_non_exhaustive()
    }
}

#[must_use = "inspect retryable or process-teardown custody"]
#[allow(clippy::large_enum_variant)]
pub enum Gfx942DirectionalPersistentSdmaSubmissionCustodyV1 {
    Retryable {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    ProcessTeardown(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
}

#[must_use = "inspect the failure and retain the returned custody"]
pub struct Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942DirectionalPersistentSdmaSubmissionCustodyV1,
}

impl Gfx942DirectionalPersistentSdmaSubmissionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942DirectionalPersistentSdmaSubmissionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

#[must_use = "completed directional persistent custody must be retained or demoted"]
pub struct Gfx942DirectionalPersistentSdmaCompletedV1 {
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    frontier: Gfx942PersistentDependencyFrontierV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
}

impl Gfx942DirectionalPersistentSdmaCompletedV1 {
    pub(crate) fn from_settled_v1(
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        frontier: Gfx942PersistentDependencyFrontierV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Self {
        Self {
            allocation,
            host,
            frontier,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
        }
    }

    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub(crate) fn into_single_packet_window_v1(
        self,
    ) -> Gfx942DirectionalPersistentSdmaWindowCompletedV1 {
        Gfx942DirectionalPersistentSdmaWindowCompletedV1 {
            allocation: self.allocation,
            host: self.host,
            frontier: self.frontier,
            direction: self.direction,
            host_offset: self.host_offset,
            device_offset: self.device_offset,
            copy_bytes: self.copy_bytes,
            packet_count: 1,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        (self.allocation, self.host, self.frontier)
    }
}

#[must_use = "pending directional persistent custody must be polled again"]
pub enum Gfx942DirectionalPersistentSdmaCopyPollV1 {
    Pending(Gfx942DirectionalPersistentSdmaSubmissionV1),
    Completed(Gfx942DirectionalPersistentSdmaCompletedV1),
}

#[must_use = "a timeout returns the submission; terminal custody requires teardown"]
#[allow(
    clippy::large_enum_variant,
    reason = "terminal custody must not allocate after native effects"
)]
pub enum Gfx942DirectionalPersistentSdmaExecutionCustodyV1 {
    Pending(Gfx942DirectionalPersistentSdmaSubmissionV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
}

#[must_use = "inspect the execution failure and retain its custody"]
pub struct Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942DirectionalPersistentSdmaExecutionCustodyV1,
}

impl Gfx942DirectionalPersistentSdmaExecutionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942DirectionalPersistentSdmaExecutionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

/// Internal composition result used by the runtime's bounded synchronous
/// single-packet path. Standalone asynchronous submission and observation keep
/// their existing APIs and custody transitions.
#[doc(hidden)]
#[must_use = "inspect the submission or execution failure and retain its custody"]
#[allow(clippy::large_enum_variant)]
pub enum Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1 {
    Submission(Gfx942DirectionalPersistentSdmaSubmissionFailureV1),
    Execution(Gfx942DirectionalPersistentSdmaExecutionFailureV1),
}

#[must_use = "published directional persistent SDMA window custody must be observed"]
pub struct Gfx942DirectionalPersistentSdmaWindowSubmissionV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    pub(crate) tickets: Vec<Gfx942SdmaCopyTicketV1>,
    pub(crate) host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) host_offset: u64,
    pub(crate) device_offset: u64,
    pub(crate) copy_bytes: u32,
    pub(crate) packet_count: usize,
}

impl Gfx942DirectionalPersistentSdmaWindowSubmissionV1 {
    pub const fn request(&self) -> Gfx942PersistentUseRequestV1 {
        self.published.request()
    }

    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub const fn host_offset(&self) -> u64 {
        self.host_offset
    }

    pub const fn device_offset(&self) -> u64 {
        self.device_offset
    }

    pub const fn packet_count(&self) -> usize {
        self.packet_count
    }
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaWindowSubmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaWindowSubmissionV1")
            .field("request", &self.request())
            .field("direction", &self.direction)
            .field("host_offset", &self.host_offset)
            .field("device_offset", &self.device_offset)
            .field("copy_bytes", &self.copy_bytes)
            .field("packet_count", &self.packet_count)
            .finish_non_exhaustive()
    }
}

#[allow(dead_code)]
pub(crate) enum Gfx942DirectionalPersistentSdmaWindowTerminalStateV1 {
    AdmissionRestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    PreparedRestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    PreparedUnrestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        request: Gfx942SdmaCopyRequestV1,
    },
    PreparedQueueRetained {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
    PublishedQueueRetained {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
    CompletedUnrestored {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        completed: CompletedPersistentSdmaWindowV1,
    },
}

#[must_use = "terminal native window custody must be retained until process teardown"]
pub struct Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) sequence: Option<u64>,
    pub(crate) packet_count: usize,
    pub(crate) state: Gfx942DirectionalPersistentSdmaWindowTerminalStateV1,
}

impl Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn sequence(&self) -> Option<u64> {
        self.sequence
    }

    pub const fn packet_count(&self) -> usize {
        self.packet_count
    }

    pub const fn stage(&self) -> Gfx942DirectionalPersistentSdmaTerminalStageV1 {
        match self.state {
            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::AdmissionRestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored
            }
            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PreparedRestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedRestored
            }
            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PreparedUnrestored { .. } => {
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
            }
            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PreparedQueueRetained {
                ..
            } => Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained,
            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                ..
            } => Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained,
            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::CompletedUnrestored {
                ..
            } => Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored,
        }
    }
}

impl fmt::Debug for Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1")
            .field("direction", &self.direction)
            .field("sequence", &self.sequence)
            .field("packet_count", &self.packet_count)
            .field("stage", &self.stage())
            .finish_non_exhaustive()
    }
}

#[must_use = "inspect retryable or process-teardown window custody"]
#[allow(clippy::large_enum_variant)]
pub enum Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1 {
    Retryable {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    ProcessTeardown(Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1),
}

#[must_use = "inspect the window failure and retain the returned custody"]
pub struct Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1,
}

impl Gfx942DirectionalPersistentSdmaWindowSubmissionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

#[must_use = "completed directional persistent SDMA window custody must be retained or demoted"]
pub struct Gfx942DirectionalPersistentSdmaWindowCompletedV1 {
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    frontier: Gfx942PersistentDependencyFrontierV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    packet_count: usize,
}

impl Gfx942DirectionalPersistentSdmaWindowCompletedV1 {
    pub(crate) fn belongs_to(&self, queue: QueueKeyV1) -> bool {
        self.allocation.attachment.queue == queue && self.host.belongs_to(queue)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts_for_terminal(
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        frontier: Gfx942PersistentDependencyFrontierV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        packet_count: usize,
    ) -> Self {
        Self {
            allocation,
            host,
            frontier,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            packet_count,
        }
    }

    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub const fn host_offset(&self) -> u64 {
        self.host_offset
    }

    pub const fn device_offset(&self) -> u64 {
        self.device_offset
    }

    pub const fn packet_count(&self) -> usize {
        self.packet_count
    }

    pub fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        (self.allocation, self.host, self.frontier)
    }
}

#[must_use = "pending directional persistent SDMA window custody must be polled again"]
pub enum Gfx942DirectionalPersistentSdmaWindowCopyPollV1 {
    Pending(Gfx942DirectionalPersistentSdmaWindowSubmissionV1),
    Completed(Gfx942DirectionalPersistentSdmaWindowCompletedV1),
}

#[must_use = "a window timeout returns the submission; terminal custody requires teardown"]
pub enum Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1 {
    Pending(Gfx942DirectionalPersistentSdmaWindowSubmissionV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1),
}

#[must_use = "inspect the window execution failure and retain its custody"]
pub struct Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1,
}

impl Gfx942DirectionalPersistentSdmaWindowExecutionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

pub(crate) struct DirectionalPersistentSdmaWindowPreparedCustodyV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    pub(crate) planned_tickets: Vec<Gfx942SdmaCopyTicketV1>,
    pub(crate) host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) host_offset: u64,
    pub(crate) device_offset: u64,
    pub(crate) copy_bytes: u32,
    pub(crate) packet_count: usize,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum DirectionalPersistentSdmaWindowPublicationObservationV1 {
    Recoverable(Gfx942SdmaCopyRequestV1),
    Retained(Vec<Gfx942SdmaCopyTicketV1>),
    Confirmed(Vec<Gfx942SdmaCopyTicketV1>),
}

pub(crate) enum DirectionalPersistentSdmaWindowPublicationTransitionV1 {
    Retryable {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    Published(Gfx942DirectionalPersistentSdmaWindowSubmissionV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1),
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum DirectionalPersistentSdmaWindowCompletionObservationV1 {
    Pending,
    Timeout,
    QueueRetained,
    Completed(CompletedPersistentSdmaWindowV1),
}

pub(crate) enum DirectionalPersistentSdmaWindowCompletionTransitionV1 {
    Pending(Gfx942DirectionalPersistentSdmaWindowSubmissionV1),
    Timeout(Gfx942DirectionalPersistentSdmaWindowSubmissionV1),
    Completed(Gfx942DirectionalPersistentSdmaWindowCompletedV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1),
}

pub(crate) struct DirectionalPersistentSdmaPreparedCustodyV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    pub(crate) planned_ticket: Gfx942SdmaCopyTicketV1,
    pub(crate) host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) host_offset: u64,
    pub(crate) device_offset: u64,
    pub(crate) copy_bytes: u32,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum DirectionalPersistentSdmaPublicationObservationV1 {
    Recoverable(Gfx942SdmaCopyRequestV1),
    Retained(Gfx942SdmaCopyTicketV1),
    Confirmed(Gfx942SdmaCopyTicketV1),
}

#[allow(
    clippy::large_enum_variant,
    reason = "terminal custody must not allocate after native effects"
)]
pub(crate) enum DirectionalPersistentSdmaPublicationTransitionV1 {
    Retryable {
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    Published(Gfx942DirectionalPersistentSdmaSubmissionV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum DirectionalPersistentSdmaCompletionObservationV1 {
    Pending,
    Timeout,
    QueueRetained,
    Completed(Gfx942SdmaCompletedCopyV1),
}

#[allow(
    clippy::large_enum_variant,
    reason = "terminal custody must not allocate after native effects"
)]
pub(crate) enum DirectionalPersistentSdmaCompletionTransitionV1 {
    Pending(Gfx942DirectionalPersistentSdmaSubmissionV1),
    Timeout(Gfx942DirectionalPersistentSdmaSubmissionV1),
    Completed(Gfx942DirectionalPersistentSdmaCompletedV1),
    ProcessTeardown(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
}

pub(crate) fn transition_directional_persistent_sdma_publication_v1(
    custody: DirectionalPersistentSdmaPreparedCustodyV1,
    observation: DirectionalPersistentSdmaPublicationObservationV1,
    enclosing_operation_succeeded: bool,
    closing_currentness_succeeded: bool,
) -> DirectionalPersistentSdmaPublicationTransitionV1 {
    match observation {
        DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request)
            if enclosing_operation_succeeded && closing_currentness_succeeded =>
        {
            let DirectionalPersistentSdmaPreparedCustodyV1 {
                allocation,
                prepared,
                planned_ticket,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            } = custody;
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
                        .expect("private prepared use must cancel");
                    DirectionalPersistentSdmaPublicationTransitionV1::Retryable {
                        allocation,
                        host,
                    }
                }
                Err((allocation, request)) => {
                    DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(
                        prepared_terminal_custody(
                            DirectionalPersistentSdmaPreparedCustodyV1 {
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
        DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request) => {
            DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(
                prepared_terminal_custody(
                    custody,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            )
        }
        DirectionalPersistentSdmaPublicationObservationV1::Retained(ticket) => {
            let DirectionalPersistentSdmaPreparedCustodyV1 {
                mut allocation,
                prepared,
                direction,
                ..
            } = custody;
            let sequence = prepared.sequence();
            allocation
                .owner
                .quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                )
                .expect("private prepared use must quarantine");
            DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state: Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedQueueRetained {
                        allocation,
                        ticket,
                    },
                },
            )
        }
        DirectionalPersistentSdmaPublicationObservationV1::Confirmed(ticket) => {
            let DirectionalPersistentSdmaPreparedCustodyV1 {
                mut allocation,
                prepared,
                planned_ticket,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            } = custody;
            let planned_ticket_is_exact = planned_ticket_matches_queue_occurrence(
                planned_ticket,
                allocation.attachment.queue,
                allocation.attachment.pair.queue_id(direction),
            );
            let published = allocation
                .owner
                .publish(prepared)
                .expect("private prepared use must publish only after confirmation");
            if enclosing_operation_succeeded
                && closing_currentness_succeeded
                && planned_ticket_is_exact
                && ticket == planned_ticket
            {
                return DirectionalPersistentSdmaPublicationTransitionV1::Published(
                    Gfx942DirectionalPersistentSdmaSubmissionV1 {
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
            DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    state: Gfx942DirectionalPersistentSdmaTerminalStateV1::PublishedQueueRetained {
                        allocation,
                        ticket,
                    },
                },
            )
        }
    }
}

fn prepared_terminal_custody(
    custody: DirectionalPersistentSdmaPreparedCustodyV1,
    request: Gfx942SdmaCopyRequestV1,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
    let DirectionalPersistentSdmaPreparedCustodyV1 {
        allocation,
        prepared,
        host_binding,
        direction,
        host_offset,
        device_offset,
        copy_bytes,
        ..
    } = custody;
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
            Gfx942DirectionalPersistentSdmaTerminalStateV1::PreparedRestored { allocation, host }
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
    Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
        direction,
        sequence: Some(sequence),
        state,
    }
}

pub(crate) fn transition_directional_persistent_sdma_completion_v1(
    mut submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
    observation: DirectionalPersistentSdmaCompletionObservationV1,
    enclosing_operation_succeeded: bool,
) -> DirectionalPersistentSdmaCompletionTransitionV1 {
    match observation {
        DirectionalPersistentSdmaCompletionObservationV1::Pending
            if enclosing_operation_succeeded =>
        {
            return DirectionalPersistentSdmaCompletionTransitionV1::Pending(submission);
        }
        DirectionalPersistentSdmaCompletionObservationV1::Timeout
            if enclosing_operation_succeeded =>
        {
            let timeout = submission
                .allocation
                .owner
                .observe_timeout(submission.published)
                .expect("private published use must retain timeout custody");
            submission.published = timeout.into_published();
            return DirectionalPersistentSdmaCompletionTransitionV1::Timeout(submission);
        }
        DirectionalPersistentSdmaCompletionObservationV1::Completed(completed) => {
            let Gfx942DirectionalPersistentSdmaSubmissionV1 {
                allocation,
                published,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                ..
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
                return DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(
                    Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                        direction,
                        sequence: Some(sequence),
                        state:
                            Gfx942DirectionalPersistentSdmaTerminalStateV1::CompletedUnrestored {
                                allocation,
                                completed,
                            },
                    },
                );
            }
            return match restore_directional_completed_sdma_copy_v1(
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
                        .expect("single-flight use must settle in order");
                    DirectionalPersistentSdmaCompletionTransitionV1::Completed(
                        Gfx942DirectionalPersistentSdmaCompletedV1 {
                            allocation,
                            host,
                            frontier,
                            direction,
                            host_offset,
                            device_offset,
                            copy_bytes,
                        },
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
                    DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(
                        Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
                            direction,
                            sequence: Some(sequence),
                            state:
                                Gfx942DirectionalPersistentSdmaTerminalStateV1::CompletedUnrestored {
                                    allocation,
                                    completed,
                                },
                        },
                    )
                }
            };
        }
        DirectionalPersistentSdmaCompletionObservationV1::Pending
        | DirectionalPersistentSdmaCompletionObservationV1::Timeout
        | DirectionalPersistentSdmaCompletionObservationV1::QueueRetained => {}
    }

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
        .quarantine_published(
            published,
            if enclosing_operation_succeeded {
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
            } else {
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
            },
        )
        .expect("private published use must quarantine");
    DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(
        Gfx942DirectionalPersistentSdmaTerminalCustodyV1 {
            direction,
            sequence: Some(sequence),
            state: Gfx942DirectionalPersistentSdmaTerminalStateV1::PublishedQueueRetained {
                allocation,
                ticket,
            },
        },
    )
}

fn window_prepared_terminal_custody(
    custody: DirectionalPersistentSdmaWindowPreparedCustodyV1,
    request: Gfx942SdmaCopyRequestV1,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
    let DirectionalPersistentSdmaWindowPreparedCustodyV1 {
        allocation,
        prepared,
        host_binding,
        direction,
        host_offset,
        device_offset,
        copy_bytes,
        packet_count,
        ..
    } = custody;
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
    Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
        direction,
        sequence: Some(sequence),
        packet_count,
        state,
    }
}

pub(crate) fn transition_directional_persistent_sdma_window_publication_v1(
    custody: DirectionalPersistentSdmaWindowPreparedCustodyV1,
    observation: DirectionalPersistentSdmaWindowPublicationObservationV1,
    enclosing_operation_succeeded: bool,
    closing_currentness_succeeded: bool,
) -> DirectionalPersistentSdmaWindowPublicationTransitionV1 {
    match observation {
        DirectionalPersistentSdmaWindowPublicationObservationV1::Recoverable(request)
            if enclosing_operation_succeeded && closing_currentness_succeeded =>
        {
            let DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                allocation,
                prepared,
                planned_tickets,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                packet_count,
            } = custody;
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
                        .expect("private prepared window use must cancel");
                    DirectionalPersistentSdmaWindowPublicationTransitionV1::Retryable {
                        allocation,
                        host,
                    }
                }
                Err((allocation, request)) => {
                    DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                        window_prepared_terminal_custody(
                            DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                                allocation,
                                prepared,
                                planned_tickets,
                                host_binding,
                                direction,
                                host_offset,
                                device_offset,
                                copy_bytes,
                                packet_count,
                            },
                            request,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                        ),
                    )
                }
            }
        }
        DirectionalPersistentSdmaWindowPublicationObservationV1::Recoverable(request) => {
            DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                window_prepared_terminal_custody(
                    custody,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            )
        }
        DirectionalPersistentSdmaWindowPublicationObservationV1::Retained(tickets) => {
            let DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                mut allocation,
                prepared,
                direction,
                packet_count,
                ..
            } = custody;
            let sequence = prepared.sequence();
            allocation
                .owner
                .quarantine_prepared(
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                )
                .expect("private prepared window use must quarantine");
            DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                    direction,
                    sequence: Some(sequence),
                    packet_count,
                    state:
                        Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PreparedQueueRetained {
                            allocation,
                            tickets,
                        },
                },
            )
        }
        DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets) => {
            let DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                mut allocation,
                prepared,
                planned_tickets,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                packet_count,
            } = custody;
            let expected_queue = allocation.attachment.pair.queue_id(direction);
            let planned_roster_is_exact = planned_tickets.len() == packet_count
                && planned_tickets.iter().all(|ticket| {
                    planned_ticket_matches_queue_occurrence(
                        *ticket,
                        allocation.attachment.queue,
                        expected_queue,
                    )
                });
            let published = allocation
                .owner
                .publish(prepared)
                .expect("private prepared window use publishes only after confirmation");
            if enclosing_operation_succeeded
                && closing_currentness_succeeded
                && planned_roster_is_exact
                && tickets == planned_tickets
            {
                return DirectionalPersistentSdmaWindowPublicationTransitionV1::Published(
                    Gfx942DirectionalPersistentSdmaWindowSubmissionV1 {
                        allocation,
                        published,
                        tickets,
                        host_binding,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                        packet_count,
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
                .expect("private published window use must quarantine");
            DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
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
            )
        }
    }
}

pub(crate) fn transition_directional_persistent_sdma_window_completion_v1(
    mut submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    observation: DirectionalPersistentSdmaWindowCompletionObservationV1,
    enclosing_operation_succeeded: bool,
) -> DirectionalPersistentSdmaWindowCompletionTransitionV1 {
    match observation {
        DirectionalPersistentSdmaWindowCompletionObservationV1::Pending
            if enclosing_operation_succeeded =>
        {
            return DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(submission);
        }
        DirectionalPersistentSdmaWindowCompletionObservationV1::Timeout
            if enclosing_operation_succeeded =>
        {
            let timeout = submission
                .allocation
                .owner
                .observe_timeout(submission.published)
                .expect("private published window use retains timeout custody");
            submission.published = timeout.into_published();
            return DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(submission);
        }
        DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(completed) => {
            let Gfx942DirectionalPersistentSdmaWindowSubmissionV1 {
                allocation,
                published,
                host_binding,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                packet_count,
                ..
            } = submission;
            let sequence = published.sequence();
            if !enclosing_operation_succeeded || completed.packet_count != packet_count {
                let mut allocation = allocation;
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
                    .expect("private published window use must quarantine");
                return DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(
                    Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                        direction,
                        sequence: Some(sequence),
                        packet_count,
                        state:
                            Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::CompletedUnrestored {
                                allocation,
                                completed,
                            },
                    },
                );
            }
            return match restore_directional_persistent_sdma_request_v1(
                allocation,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                host_binding,
                completed.request,
            ) {
                Ok((mut allocation, host)) => {
                    let completed_use = allocation
                        .owner
                        .complete(published)
                        .expect("private published window use must complete");
                    let frontier = allocation
                        .owner
                        .settle(completed_use)
                        .expect("single aggregate window use must settle in order");
                    DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(
                        Gfx942DirectionalPersistentSdmaWindowCompletedV1 {
                            allocation,
                            host,
                            frontier,
                            direction,
                            host_offset,
                            device_offset,
                            copy_bytes,
                            packet_count,
                        },
                    )
                }
                Err((mut allocation, request)) => {
                    allocation
                        .owner
                        .quarantine_published(
                            published,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                        )
                        .expect("private published window use must quarantine");
                    DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(
                        Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
                            direction,
                            sequence: Some(sequence),
                            packet_count,
                            state:
                                Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::CompletedUnrestored {
                                    allocation,
                                    completed: CompletedPersistentSdmaWindowV1 {
                                        request,
                                        packet_count,
                                    },
                                },
                        },
                    )
                }
            };
        }
        DirectionalPersistentSdmaWindowCompletionObservationV1::Pending
        | DirectionalPersistentSdmaWindowCompletionObservationV1::Timeout
        | DirectionalPersistentSdmaWindowCompletionObservationV1::QueueRetained => {}
    }

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
        .quarantine_published(
            published,
            if enclosing_operation_succeeded {
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
            } else {
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
            },
        )
        .expect("private published window use must quarantine");
    DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(
        Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1 {
            direction,
            sequence: Some(sequence),
            packet_count,
            state: Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                allocation,
                tickets,
            },
        },
    )
}

pub(crate) fn directional_persistent_sdma_request_v1(
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

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub(crate) fn restore_directional_persistent_sdma_request_v1(
    mut allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    request: Gfx942SdmaCopyRequestV1,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
    ),
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaCopyRequestV1,
    ),
> {
    if !directional_persistent_sdma_request_matches_v1(
        &allocation,
        direction,
        host_offset,
        device_offset,
        copy_bytes,
        host_binding,
        &request,
    ) {
        return Err((allocation, request));
    }
    let Gfx942SdmaCopyRequestV1 {
        source,
        destination,
        copy_bytes,
        ..
    } = request;
    let (device, host) = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => (destination, source),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => (source, destination),
    };
    if let Err(device) = allocation.owner.restore_sdma_buffer(device) {
        let (source, source_offset, destination, destination_offset) = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                (host, host_offset, device, device_offset)
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                (device, device_offset, host, host_offset)
            }
        };
        return Err((
            allocation,
            Gfx942SdmaCopyRequestV1 {
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            },
        ));
    }
    Ok((allocation, host))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn directional_persistent_sdma_request_matches_v1(
    allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    request: &Gfx942SdmaCopyRequestV1,
) -> bool {
    let offsets_exact = request.copy_bytes == copy_bytes
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
    if !offsets_exact {
        return false;
    }
    let (device, host) = match direction {
        Gfx942PersistentSdmaDirectionV1::HostToDevice => (&request.destination, &request.source),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost => (&request.source, &request.destination),
    };
    let attachment = allocation.attachment;
    device.belongs_to(attachment.queue)
        && host_binding.matches(host)
        && device.storage_identity() == attachment.storage_identity
        && device.pool_generation() == attachment.pool_generation
        && device.requested_bytes() == attachment.logical_bytes
        && device.physical_bytes() == attachment.physical_bytes
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn restore_directional_completed_sdma_copy_v1(
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    host_offset: u64,
    device_offset: u64,
    copy_bytes: u32,
    host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
    completed: Gfx942SdmaCompletedCopyV1,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
    ),
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaCompletedCopyV1,
    ),
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
        destination,
        copy_bytes,
        source_offset,
        destination_offset,
    };
    match restore_directional_persistent_sdma_request_v1(
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
                destination,
                copy_bytes,
                source_offset,
                destination_offset,
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
pub(crate) fn promote_directional_persistent_sdma_custody_v1(
    buffer: Gfx942SdmaBufferV1,
    pair: Gfx942PersistentDirectionalSdmaPairV1,
    outstanding_buffers: usize,
) -> Result<(Gfx942DirectionalQueuePersistentAllocationV1, usize), Gfx942SdmaBufferV1> {
    if outstanding_buffers == 0 || buffer.kind() != Gfx942SdmaBufferKindV1::DeviceLocal {
        return Err(buffer);
    }
    let storage_identity = buffer.storage_identity();
    let physical_bytes = buffer.physical_bytes();
    let queue = buffer.queue_owner();
    let pool_generation = buffer.pool_generation();
    let logical_bytes = buffer.requested_bytes();
    Ok((
        Gfx942DirectionalQueuePersistentAllocationV1 {
            owner: Gfx942PersistentDeviceAllocationV1::from_sdma_buffer(buffer)?,
            attachment: Gfx942PersistentDirectionalSdmaAttachmentV1 {
                queue,
                pair,
                pool_generation,
                logical_bytes,
                physical_bytes,
                storage_identity,
            },
        },
        outstanding_buffers,
    ))
}

#[allow(clippy::result_large_err)]
pub(crate) fn demote_directional_persistent_sdma_custody_v1(
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    outstanding_buffers: usize,
) -> Result<
    (Gfx942SdmaBufferV1, usize),
    (
        Gfx942PersistentUseErrorV1,
        Gfx942DirectionalQueuePersistentAllocationV1,
    ),
> {
    let Some(next_generation) = allocation.attachment.pool_generation.checked_add(1) else {
        return Err((Gfx942PersistentUseErrorV1::GenerationExhausted, allocation));
    };
    if outstanding_buffers == 0 {
        return Err((Gfx942PersistentUseErrorV1::WrongState, allocation));
    }
    if allocation.owner.retained_settled_use_count() != 0 {
        return Err((Gfx942PersistentUseErrorV1::OutstandingUses, allocation));
    }
    let Gfx942DirectionalQueuePersistentAllocationV1 { owner, attachment } = allocation;
    let native = match owner.try_into_native() {
        Ok(native) => native,
        Err((error, owner)) => {
            return Err((
                error,
                Gfx942DirectionalQueuePersistentAllocationV1 { owner, attachment },
            ));
        }
    };
    let crate::persistent_allocation::Gfx942PersistentNativeAllocationV1::Local(lease) = native
    else {
        unreachable!("validated directional custody is local")
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

pub(crate) fn map_directional_persistent_sdma_use_error_v1(
    error: Gfx942PersistentUseErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    ComputeAqlQueueSessionErrorV1::Contract(match error {
        Gfx942PersistentUseErrorV1::InvalidRange => "directional persistent SDMA device range",
        Gfx942PersistentUseErrorV1::OperationRequiresPeerMapping => {
            "directional persistent SDMA local operation mapping"
        }
        Gfx942PersistentUseErrorV1::Capacity => "directional persistent SDMA use ledger full",
        Gfx942PersistentUseErrorV1::GenerationExhausted => {
            "directional persistent SDMA use generation exhausted"
        }
        Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration => {
            "directional persistent SDMA use owner or generation"
        }
        Gfx942PersistentUseErrorV1::WrongState => "directional persistent SDMA use state",
        Gfx942PersistentUseErrorV1::OverlappingWriterActive => {
            "directional persistent SDMA overlapping writer active"
        }
        Gfx942PersistentUseErrorV1::DependencyRequired => {
            "directional persistent SDMA dependency required"
        }
        Gfx942PersistentUseErrorV1::DependencyNotRequired => {
            "directional persistent SDMA dependency not required"
        }
        Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency => {
            "directional persistent SDMA stale or substituted dependency"
        }
        Gfx942PersistentUseErrorV1::EarlierUseNotSettled => {
            "directional persistent SDMA earlier use not settled"
        }
        Gfx942PersistentUseErrorV1::Quarantined => {
            "directional persistent SDMA allocation quarantined"
        }
        Gfx942PersistentUseErrorV1::OutstandingUses => {
            "directional persistent SDMA allocation has outstanding uses"
        }
    })
}

#[cfg(test)]
#[path = "persistent_directional_sdma/tests.rs"]
mod tests;
