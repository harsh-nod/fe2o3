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

    fn matches(self, host: &Gfx942SdmaBufferV1) -> bool {
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
}

#[allow(dead_code)]
pub(crate) enum Gfx942DirectionalPersistentSdmaTerminalStateV1 {
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
        match self.state {
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
    let attachment = allocation.attachment;
    if !device.belongs_to(attachment.queue)
        || !host_binding.matches(&host)
        || device.storage_identity() != attachment.storage_identity
        || device.pool_generation() != attachment.pool_generation
        || device.requested_bytes() != attachment.logical_bytes
        || device.physical_bytes() != attachment.physical_bytes
    {
        return Err((
            allocation,
            directional_persistent_sdma_request_v1(
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
        unreachable!("checked device-local storage")
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
            directional_persistent_sdma_request_v1(
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
    if outstanding_buffers == 0 {
        return Err(buffer);
    }
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
    Ok((
        Gfx942DirectionalQueuePersistentAllocationV1 {
            owner: Gfx942PersistentDeviceAllocationV1::from_local_mapping(lease),
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
mod tests {
    use super::*;
    use crate::persistent_allocation::Gfx942PersistentOperationV1;
    use crate::sdma::{
        GFX942_SDMA_MAX_IN_FLIGHT_V1, GFX942_SDMA_RING_BYTES_V1, Gfx942SdmaQueueObservationV1,
        persistent_sdma_buffers_for_test, persistent_sdma_ticket_coordinates_for_test,
    };
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        VmIdV1, VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    fn queue_key() -> QueueKeyV1 {
        queue_key_with_generation(1)
    }

    fn queue_key_with_generation(generation: u64) -> QueueKeyV1 {
        let device = DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(7),
            generation: DeviceGenerationV1(1),
        };
        QueueKeyV1 {
            vm: VmKeyV1 {
                device,
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(3),
            generation: QueueGenerationV1(generation),
        }
    }

    fn queue_observation(queue_id: u32, engine_index: u32) -> Gfx942SdmaQueueObservationV1 {
        Gfx942SdmaQueueObservationV1 {
            queue_id,
            ring_bytes: GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(engine_index),
        }
    }

    fn pair_observation(h2d_queue: u32, d2h_queue: u32) -> Gfx942DirectionalSdmaQueueObservationV1 {
        Gfx942DirectionalSdmaQueueObservationV1 {
            host_to_device: queue_observation(h2d_queue, GFX942_SDMA_H2D_ENGINE_INDEX_V1),
            device_to_host: queue_observation(d2h_queue, GFX942_SDMA_D2H_ENGINE_INDEX_V1),
            admitted_engine_count: 2,
            admitted_queues_per_engine: 8,
        }
    }

    fn promoted_fixture(
        id: u64,
        logical_bytes: u64,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
    ) {
        let (mut device, host) = persistent_sdma_buffers_for_test(queue_key(), id);
        device.set_logical_bytes(logical_bytes);
        let pair = admit_persistent_directional_sdma_pair_v1(pair_observation(17, 23)).unwrap();
        let (allocation, outstanding) =
            promote_directional_persistent_sdma_custody_v1(device, pair, 2).unwrap();
        assert_eq!(outstanding, 2);
        (allocation, host)
    }

    fn prepared_fixture(
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
        direction: Gfx942PersistentSdmaDirectionV1,
        ticket_generation: u32,
    ) -> (
        DirectionalPersistentSdmaPreparedCustodyV1,
        Gfx942SdmaCopyRequestV1,
        Gfx942SdmaCopyTicketV1,
    ) {
        let mut allocation = allocation;
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
        let host_binding =
            Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, queue_key());
        let request = directional_persistent_sdma_request_v1(direction, host, 8, device, 16, 32);
        let ticket = persistent_sdma_ticket_coordinates_for_test(
            queue_key(),
            allocation.attachment.pair.queue_id(direction),
            (ticket_generation as u16) % GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            ticket_generation,
        );
        (
            DirectionalPersistentSdmaPreparedCustodyV1 {
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

    fn published_fixture(
        id: u64,
        direction: Gfx942PersistentSdmaDirectionV1,
    ) -> (
        Gfx942DirectionalPersistentSdmaSubmissionV1,
        Gfx942SdmaCopyRequestV1,
    ) {
        let (allocation, host) = promoted_fixture(id, 2048);
        let (prepared, request, ticket) = prepared_fixture(allocation, host, None, direction, 1);
        let DirectionalPersistentSdmaPublicationTransitionV1::Published(submission) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Confirmed(ticket),
                true,
                true,
            )
        else {
            unreachable!()
        };
        (submission, request)
    }

    fn completed_request(request: Gfx942SdmaCopyRequestV1) -> Gfx942SdmaCompletedCopyV1 {
        Gfx942SdmaCompletedCopyV1 {
            source: request.source,
            destination: request.destination,
            copy_bytes: request.copy_bytes,
            source_offset: request.source_offset,
            destination_offset: request.destination_offset,
        }
    }

    fn prepared_window_fixture(
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        packet_count: usize,
    ) -> (
        DirectionalPersistentSdmaWindowPreparedCustodyV1,
        Gfx942SdmaCopyRequestV1,
        Vec<Gfx942SdmaCopyTicketV1>,
    ) {
        let mut allocation = allocation;
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
                None,
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
        let host_binding =
            Gfx942PersistentDirectionalSdmaHostBindingV1::capture(&host, queue_key());
        let request = directional_persistent_sdma_request_v1(direction, host, 8, device, 16, 32);
        let tickets = (0..packet_count)
            .map(|index| {
                persistent_sdma_ticket_coordinates_for_test(
                    queue_key(),
                    allocation.attachment.pair.queue_id(direction),
                    index as u16,
                    1,
                )
            })
            .collect::<Vec<_>>();
        (
            DirectionalPersistentSdmaWindowPreparedCustodyV1 {
                allocation,
                prepared,
                planned_tickets: tickets.clone(),
                host_binding,
                direction,
                host_offset: 8,
                device_offset: 16,
                copy_bytes: 32,
                packet_count,
            },
            request,
            tickets,
        )
    }

    fn published_window_fixture(
        id: u64,
        direction: Gfx942PersistentSdmaDirectionV1,
        packet_count: usize,
    ) -> (
        Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
        Gfx942SdmaCopyRequestV1,
    ) {
        let (allocation, host) = promoted_fixture(id, 2048);
        let (prepared, request, tickets) =
            prepared_window_fixture(allocation, host, direction, packet_count);
        let DirectionalPersistentSdmaWindowPublicationTransitionV1::Published(submission) =
            transition_directional_persistent_sdma_window_publication_v1(
                prepared,
                DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                true,
                true,
            )
        else {
            unreachable!()
        };
        (submission, request)
    }

    #[test]
    fn manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_ADAPTER_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_ADAPTER_MANIFEST_SHA256_V1
        );
    }

    #[test]
    fn window_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_WINDOW_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_WINDOW_MANIFEST_SHA256_V1
        );
    }

    #[test]
    fn window_clean_recovery_restores_the_exact_owner_pair() {
        for (id, direction) in [
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        ]
        .into_iter()
        .enumerate()
        {
            let (allocation, host) = promoted_fixture(100 + id as u64, 2048);
            let identity = allocation.attachment.storage_identity;
            let (prepared, request, _) = prepared_window_fixture(allocation, host, direction, 3);
            let DirectionalPersistentSdmaWindowPublicationTransitionV1::Retryable {
                allocation,
                host,
            } = transition_directional_persistent_sdma_window_publication_v1(
                prepared,
                DirectionalPersistentSdmaWindowPublicationObservationV1::Recoverable(request),
                true,
                true,
            )
            else {
                panic!("clean window rejection must restore exact custody")
            };
            assert_eq!(allocation.attachment.storage_identity, identity);
            assert_eq!(host.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
            assert!(allocation.owner.local_native_is_attached_for_sdma());
        }
    }

    #[test]
    fn window_retained_and_substituted_publications_quarantine_the_whole_roster() {
        let (allocation, host) = promoted_fixture(110, 2048);
        let (prepared, _, tickets) = prepared_window_fixture(
            allocation,
            host,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            3,
        );
        let DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_window_publication_v1(
                prepared,
                DirectionalPersistentSdmaWindowPublicationObservationV1::Retained(tickets),
                true,
                true,
            )
        else {
            panic!("retained window publication must be terminal")
        };
        assert_eq!(custody.packet_count(), 3);
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained
        );

        let (allocation, host) = promoted_fixture(111, 2048);
        let (prepared, _, mut tickets) = prepared_window_fixture(
            allocation,
            host,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            3,
        );
        tickets.swap(0, 1);
        let DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_window_publication_v1(
                prepared,
                DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                true,
                true,
            )
        else {
            panic!("reordered window tickets must be terminal")
        };
        assert_eq!(custody.packet_count(), 3);
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
        );
    }

    #[test]
    fn window_pending_timeout_and_exact_completion_are_aggregate() {
        let (submission, request) =
            published_window_fixture(120, Gfx942PersistentSdmaDirectionV1::DeviceToHost, 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(submission) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Pending,
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(submission.packet_count(), 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(submission) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Timeout,
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(submission.packet_count(), 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 3,
                    },
                ),
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(completed.packet_count(), 3);
        assert_eq!(completed.copy_bytes(), 32);
        assert_eq!(completed.host_offset(), 8);
        assert_eq!(completed.device_offset(), 16);
        assert_eq!(
            completed.direction(),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost
        );
        let (allocation, _, frontier) = completed.into_parts();
        let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
        assert!(allocation.owner.local_native_is_attached_for_sdma());
    }

    #[test]
    fn poisoned_queue_promotion_preflight_preserves_completed_h2d_frontier() {
        let (submission, request) =
            published_window_fixture(122, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 3,
                    },
                ),
                true,
            )
        else {
            unreachable!()
        };
        let failure = match crate::queue::preserve_persistent_compute_ready_preflight_custody_v1(
            completed,
            true,
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal queue session requires process teardown",
            )),
        ) {
            Ok(_) => panic!("a poisoned queue must reject H2D-ready promotion"),
            Err(failure) => failure,
        };
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(terminal) =
            custody
        else {
            panic!("terminal preflight must return opaque process-teardown custody")
        };
        let (allocation, _host, frontier) = terminal.completed.into_parts();
        assert_eq!(allocation.owner.retained_settled_use_count(), 1);
        let allocation = allocation
            .retire_settled_frontier_v1(frontier)
            .expect("preflight must not retire the completed H2D frontier");
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn currentness_hash_failure_returns_opaque_completed_h2d_custody() {
        let (submission, request) =
            published_window_fixture(124, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 3,
                    },
                ),
                true,
            )
        else {
            unreachable!()
        };
        let failure = crate::queue::terminal_persistent_compute_ready_hash_failure_v1(
            crate::MemorySessionError::ProcessChanged.into(),
            completed,
        );
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Memory(crate::MemorySessionError::ProcessChanged)
        ));
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(terminal) =
            custody
        else {
            panic!("currentness loss must seal exact completed-window custody")
        };
        let (allocation, _host, frontier) = terminal.completed.into_parts();
        assert_eq!(allocation.owner.retained_settled_use_count(), 1);
        let allocation = allocation
            .retire_settled_frontier_v1(frontier)
            .expect("hash failure must not retire the completed H2D frontier");
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn terminal_directional_admission_absorbs_self_owned_invalid_geometry() {
        let (allocation, host) = promoted_fixture(125, 2048);
        let allocation_identity = allocation.attachment.storage_identity;
        let host_identity = host.storage_identity();
        let failure = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
            queue_key(),
            true,
            allocation,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host,
            u64::MAX,
            u64::MAX,
            0,
        )
        .expect_err("terminal custody must dominate invalid single-copy geometry");
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(terminal) = custody
        else {
            panic!("self-owned terminal input must not return retryable custody")
        };
        assert_eq!(
            terminal.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored
        );
        let Gfx942DirectionalPersistentSdmaTerminalStateV1::AdmissionRestored { allocation, host } =
            terminal.state
        else {
            unreachable!()
        };
        assert_eq!(allocation.attachment.storage_identity, allocation_identity);
        assert_eq!(host.storage_identity(), host_identity);

        let (allocation, host) = promoted_fixture(126, 2048);
        let allocation_identity = allocation.attachment.storage_identity;
        let host_identity = host.storage_identity();
        let failure = crate::queue::admit_directional_persistent_sdma_window_input_v1(
            queue_key(),
            true,
            allocation,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            host,
            u64::MAX,
            u64::MAX,
            0,
        )
        .expect_err("terminal custody must dominate invalid window geometry");
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(terminal) =
            custody
        else {
            panic!("self-owned terminal window must not return retryable custody")
        };
        assert_eq!(terminal.packet_count(), 0);
        assert_eq!(
            terminal.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored
        );
        let Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::AdmissionRestored {
            allocation,
            host,
        } = terminal.state
        else {
            unreachable!()
        };
        assert_eq!(allocation.attachment.storage_identity, allocation_identity);
        assert_eq!(host.storage_identity(), host_identity);
    }

    #[test]
    fn terminal_receiver_returns_foreign_directional_inputs_exactly() {
        let (allocation, host) = promoted_fixture(127, 2048);
        let allocation_identity = allocation.attachment.storage_identity;
        let host_identity = host.storage_identity();
        let failure = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
            queue_key_with_generation(2),
            true,
            allocation,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host,
            0,
            0,
            1,
        )
        .expect_err("foreign input must be returned before terminal absorption");
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable { allocation, host } =
            custody
        else {
            panic!("foreign input must remain retryable on its producing queue")
        };
        assert_eq!(allocation.attachment.storage_identity, allocation_identity);
        assert_eq!(host.storage_identity(), host_identity);
        let retry = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
            queue_key(),
            false,
            allocation,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host,
            0,
            0,
            1,
        );
        assert!(retry.is_ok());

        let (allocation, host) = promoted_fixture(128, 2048);
        let allocation_identity = allocation.attachment.storage_identity;
        let host_identity = host.storage_identity();
        let failure = crate::queue::admit_directional_persistent_sdma_window_input_v1(
            queue_key_with_generation(2),
            true,
            allocation,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            host,
            0,
            0,
            1,
        )
        .expect_err("foreign window input must be returned before terminal absorption");
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
            allocation,
            host,
        } = custody
        else {
            panic!("foreign window input must remain retryable on its producing queue")
        };
        assert_eq!(allocation.attachment.storage_identity, allocation_identity);
        assert_eq!(host.storage_identity(), host_identity);
        let retry = crate::queue::admit_directional_persistent_sdma_window_input_v1(
            queue_key(),
            false,
            allocation,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            host,
            0,
            0,
            1,
        );
        assert!(retry.is_ok());
    }

    #[test]
    fn foreign_queue_ready_promotion_returns_exact_retryable_completed_receipt() {
        let (submission, request) =
            published_window_fixture(123, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 3,
                    },
                ),
                true,
            )
        else {
            unreachable!()
        };
        let failure = match crate::queue::preserve_persistent_compute_ready_affiliation_v1(
            completed,
            queue_key_with_generation(2),
            true,
        ) {
            Ok(_) => panic!("foreign queue must not consume completed H2D custody"),
            Err(failure) => failure,
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                crate::Gfx942DispatchBindingErrorV1::Poisoned
            )
        ));
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeReadyFailureCustodyV1::ForeignQueue(completed) = custody
        else {
            panic!("foreign queue must return the original completed receipt")
        };
        let completed = crate::queue::preserve_persistent_compute_ready_affiliation_v1(
            completed,
            queue_key(),
            false,
        )
        .expect("the producing queue must accept the unchanged receipt");
        assert_eq!(completed.packet_count(), 3);
        let (allocation, _host, frontier) = completed.into_parts();
        let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn persistent_compute_window_gate_returns_exact_custody_in_both_directions() {
        for (ordinal, direction) in [
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        ]
        .into_iter()
        .enumerate()
        {
            let (allocation, host) = promoted_fixture(130 + ordinal as u64, 4096);
            let allocation_identity = allocation.attachment.storage_identity;
            let host_identity = host.storage_identity();
            let failure =
                match crate::queue::preserve_directional_window_sdma_publication_custody_v1(
                    true, direction, allocation, host,
                ) {
                    Ok(_) => panic!("persistent compute must block directional window publication"),
                    Err(failure) => failure,
                };
            assert!(matches!(
                failure.error(),
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    crate::Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            ));
            let (_, custody) = failure.into_parts();
            let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                allocation,
                host,
            } = custody
            else {
                panic!("pure publication rejection must be retryable")
            };
            assert_eq!(allocation.attachment.storage_identity, allocation_identity);
            assert_eq!(host.storage_identity(), host_identity);
            assert_eq!(allocation.owner.live_use_count(), 0);
        }
    }

    #[test]
    fn active_directional_sdma_blocks_bind_and_returns_exact_compute_input() {
        for (ordinal, direction) in [
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        ]
        .into_iter()
        .enumerate()
        {
            let (active, _request) = published_window_fixture(140 + ordinal as u64, direction, 3);
            let (allocation, _host) = promoted_fixture(150 + ordinal as u64, 4096);
            let identity = allocation.attachment.storage_identity;
            let input = crate::Gfx942PersistentComputeInputV1::Uninitialized(allocation);
            let failure =
                match crate::queue::preserve_persistent_compute_bind_input_for_sdma_quiescence_v1(
                    input,
                    active.packet_count() == 0,
                ) {
                    Ok(_) => panic!("active directional SDMA must block persistent compute bind"),
                    Err(failure) => failure,
                };
            assert_eq!(active.packet_count(), 3);
            assert!(matches!(
                failure.error(),
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    crate::Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            ));
            let (_, custody) = failure.into_parts();
            let crate::Gfx942PersistentComputeBindFailureCustodyV1::Retryable(recovered) = custody
            else {
                panic!("quiescence rejection returns exact retryable compute input")
            };
            let (allocation, digest, initialized) = recovered.into_parts();
            assert_eq!(allocation.attachment.storage_identity, identity);
            assert_eq!(digest, None);
            assert!(!initialized);
            assert_eq!(allocation.owner.live_use_count(), 0);
        }
    }

    #[test]
    fn window_completion_metadata_mismatch_is_terminal() {
        let (submission, request) =
            published_window_fixture(121, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 2,
                    },
                ),
                true,
            )
        else {
            panic!("partial window completion must retain terminal custody")
        };
        assert_eq!(custody.packet_count(), 3);
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
        );
    }

    #[test]
    fn window_completion_offset_substitution_is_terminal() {
        let (submission, mut request) =
            published_window_fixture(122, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
        request.source_offset += 1;
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 3,
                    },
                ),
                true,
            )
        else {
            panic!("host-offset substitution must retain terminal custody")
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
        );

        let (submission, mut request) =
            published_window_fixture(123, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
        request.destination_offset += 1;
        let DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_window_completion_v1(
                submission,
                DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                    CompletedPersistentSdmaWindowV1 {
                        request,
                        packet_count: 3,
                    },
                ),
                true,
            )
        else {
            panic!("device-offset substitution must retain terminal custody")
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
        );
    }

    #[test]
    fn pair_admission_rejects_swapped_engines_and_duplicate_children() {
        let pair = admit_persistent_directional_sdma_pair_v1(pair_observation(17, 23)).unwrap();
        assert_eq!(pair.host_to_device_queue_id, 17);
        assert_eq!(pair.device_to_host_queue_id, 23);

        let mut swapped = pair_observation(17, 23);
        swapped.host_to_device.engine_index = Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1);
        swapped.device_to_host.engine_index = Some(GFX942_SDMA_H2D_ENGINE_INDEX_V1);
        assert!(admit_persistent_directional_sdma_pair_v1(swapped).is_err());
        assert!(admit_persistent_directional_sdma_pair_v1(pair_observation(17, 17)).is_err());
        let mut wrong_inventory = pair_observation(17, 23);
        wrong_inventory.admitted_queues_per_engine = 7;
        assert!(admit_persistent_directional_sdma_pair_v1(wrong_inventory).is_err());
    }

    #[test]
    fn pooled_logical_extent_preserves_physical_owner_and_buffer_debit() {
        let (allocation, _host) = promoted_fixture(10, 2048);
        assert_eq!(allocation.byte_len(), 2048);
        assert_eq!(allocation.physical_byte_len(), 4096);
        let original_generation = allocation.attachment.pool_generation;
        let (device, outstanding) =
            demote_directional_persistent_sdma_custody_v1(allocation, 2).unwrap();
        assert_eq!(outstanding, 2);
        assert_eq!(device.requested_bytes(), 2048);
        assert_eq!(device.physical_bytes(), 4096);
        assert_eq!(device.pool_generation(), original_generation + 1);
        assert!(!directional_persistent_sdma_queue_destroy_is_admitted_v1(
            outstanding
        ));
        assert!(directional_persistent_sdma_queue_destroy_is_admitted_v1(0));
    }

    #[test]
    fn extent_admission_is_bounded_and_page_rounded_only_physically() {
        assert!(directional_persistent_sdma_extents_are_admitted_v1(
            1, 4096, 1
        ));
        assert!(directional_persistent_sdma_extents_are_admitted_v1(
            2048, 4096, 1
        ));
        assert!(!directional_persistent_sdma_extents_are_admitted_v1(
            4097, 4096, 1
        ));
        assert!(!directional_persistent_sdma_extents_are_admitted_v1(
            1, 4095, 1
        ));
        assert!(!directional_persistent_sdma_extents_are_admitted_v1(
            1,
            GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_ALLOCATION_BYTES_V1 + 4096,
            1,
        ));
    }

    #[test]
    fn promotion_and_demotion_failures_preserve_explicit_owner_custody() {
        let (device, _) = persistent_sdma_buffers_for_test(queue_key(), 80);
        let identity = device.storage_identity();
        let failure = classify_directional_persistent_sdma_promotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("retryable promotion"),
            device,
            false,
        );
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaPromotionCustodyV1::Retryable(device) = custody else {
            panic!("recoverable promotion must return retryable device custody")
        };
        assert_eq!(device.storage_identity(), identity);

        let (device, _) = persistent_sdma_buffers_for_test(queue_key(), 81);
        let identity = device.storage_identity();
        let failure = classify_directional_persistent_sdma_promotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("terminal promotion"),
            device,
            true,
        );
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(terminal) = custody
        else {
            panic!("terminal promotion must retain opaque device custody")
        };
        assert_eq!(terminal.buffer.storage_identity(), identity);

        let (allocation, _) = promoted_fixture(82, 2048);
        let identity = allocation.attachment.storage_identity;
        let failure = classify_directional_persistent_sdma_demotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("retryable demotion"),
            allocation,
            false,
        );
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaDemotionCustodyV1::Retryable(allocation) = custody
        else {
            panic!("recoverable demotion must return retryable allocation custody")
        };
        assert_eq!(allocation.attachment.storage_identity, identity);

        let (allocation, _) = promoted_fixture(83, 2048);
        let identity = allocation.attachment.storage_identity;
        let failure = classify_directional_persistent_sdma_demotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("terminal demotion"),
            allocation,
            true,
        );
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaDemotionCustodyV1::ProcessTeardown(terminal) = custody
        else {
            panic!("terminal demotion must retain opaque allocation custody")
        };
        assert_eq!(terminal.allocation.attachment.storage_identity, identity);
    }

    #[test]
    fn live_r19_path_uses_physical_promotion_and_operational_hot_path_checks() {
        let live = include_str!("queue_live.rs");
        let promotion = live
            .split("pub fn promote_sdma_device_buffer_to_directional_persistent_allocation_v1")
            .nth(1)
            .unwrap()
            .split("pub fn demote_directional_persistent_allocation_to_sdma_device_buffer_v1")
            .next()
            .unwrap();
        assert!(promotion.contains("validate_physical_device_mapping"));
        assert!(!promotion.contains("checked_gpu_subrange"));

        let submission = live
            .split("pub fn submit_directional_persistent_sdma_copy_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_directional_persistent_sdma_copy_v1")
            .next()
            .unwrap();
        assert_eq!(
            submission
                .matches("check_directional_persistent_sdma_operational_currentness")
                .count(),
            3
        );
        assert_eq!(
            submission
                .matches("check_queue_operational_currentness")
                .count(),
            1
        );
        let shared_close_open = submission
            .find("if let Err(error) = memory.check_queue_operational_currentness()")
            .unwrap();
        let handoff = submission
            .find("DirectionalPersistentSdmaSinglePreparedHandoffV1")
            .unwrap();
        let publication = submission.find("handoff.publish(owner, memory)").unwrap();
        let failed_prepare = submission.find("if !handoff_attempted").unwrap();
        let handoff_failure = submission.find("let Some((handoff_direction").unwrap();
        let missing_publication = submission
            .find("directional persistent SDMA handoff did not publish")
            .unwrap();
        let final_close = submission
            .rfind("check_directional_persistent_sdma_operational_currentness")
            .unwrap();
        assert!(shared_close_open < handoff);
        assert!(handoff < publication);
        let handoff_to_publication = &submission[handoff..publication];
        assert!(!handoff_to_publication.contains("return Err"));
        assert!(!handoff_to_publication.contains('?'));
        assert!(!handoff_to_publication.contains("check_"));
        assert!(publication < failed_prepare);
        assert!(failed_prepare < handoff_failure);
        assert!(handoff_failure < missing_publication);
        assert!(missing_publication < final_close);
        let failed_prepare_path = &submission[failed_prepare..handoff_failure];
        assert_eq!(
            failed_prepare_path
                .matches("check_directional_persistent_sdma_operational_currentness")
                .count(),
            1
        );
        let handoff_failure_path = &submission[handoff_failure..final_close];
        assert!(
            handoff_failure_path.contains("terminal_prepared_directional_persistent_sdma_failure")
        );
        assert!(handoff_failure_path.contains("prepared_without_handoff"));
        assert!(!submission.contains("self.check_currentness()"));
        assert!(submission.contains("prepare_single_recoverable"));
        assert!(!submission.contains("vec![request]"));

        let window = live
            .split("pub fn submit_directional_persistent_sdma_window_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_directional_persistent_sdma_window_v1")
            .next()
            .unwrap();
        assert_eq!(
            window
                .matches("check_directional_persistent_sdma_operational_currentness")
                .count(),
            3
        );
        assert_eq!(
            window
                .matches("check_queue_operational_currentness")
                .count(),
            1
        );
        let roster_allocation = window.find("try_reserve_exact(packet_count)").unwrap();
        let preparation_open = window
            .find("check_directional_persistent_sdma_operational_currentness")
            .unwrap();
        let roster_count_guard = window
            .find("if prepared.tickets().len() != packet_count")
            .unwrap();
        let roster_population = window
            .find("planned_tickets.extend_from_slice(prepared.tickets())")
            .unwrap();
        let shared_close_open = window
            .find("if let Err(error) = memory.check_queue_operational_currentness()")
            .unwrap();
        let handoff = window
            .find("DirectionalPersistentSdmaWindowPreparedHandoffV1")
            .unwrap();
        let publication = window.find("handoff.publish(owner, memory)").unwrap();
        let failed_prepare = window.find("if !handoff_attempted").unwrap();
        let handoff_failure = window.find("let Some((handoff_direction").unwrap();
        let missing_publication = window
            .find("directional persistent SDMA window handoff did not publish")
            .unwrap();
        let final_close = window
            .rfind("check_directional_persistent_sdma_operational_currentness")
            .unwrap();
        assert!(roster_allocation < preparation_open);
        assert!(preparation_open < roster_count_guard);
        assert!(roster_count_guard < roster_population);
        assert!(roster_population < shared_close_open);
        assert!(shared_close_open < handoff);
        assert!(handoff < publication);
        let handoff_to_publication = &window[handoff..publication];
        assert!(!handoff_to_publication.contains("return Err"));
        assert!(!handoff_to_publication.contains('?'));
        assert!(!handoff_to_publication.contains("check_"));
        assert!(publication < failed_prepare);
        assert!(failed_prepare < handoff_failure);
        assert!(handoff_failure < missing_publication);
        assert!(missing_publication < final_close);
        let failed_prepare_path = &window[failed_prepare..handoff_failure];
        assert_eq!(
            failed_prepare_path
                .matches("check_directional_persistent_sdma_operational_currentness")
                .count(),
            1
        );
        assert!(failed_prepare_path.contains("preparation_contract_failed"));
        let handoff_failure_path = &window[handoff_failure..final_close];
        assert!(
            handoff_failure_path
                .contains("terminal_prepared_directional_persistent_sdma_window_failure")
        );
        assert!(handoff_failure_path.contains("prepared_without_handoff"));

        let same_device = live
            .split("pub fn submit_same_device_persistent_sdma_window_v1")
            .nth(1)
            .unwrap()
            .split("pub fn poll_same_device_persistent_sdma_window_v1")
            .next()
            .unwrap();
        assert!(!same_device.contains("PreparedHandoffV1"));

        let lower = include_str!("sdma.rs")
            .split("pub(crate) fn poll(")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn wait_for(")
            .next()
            .unwrap();
        assert_eq!(
            lower.matches("check_queue_operational_currentness").count(),
            3
        );
        assert!(lower.contains("if observed == 0"));
        assert!(lower.contains("return Ok(Gfx942SdmaCopyPollV1::Pending)"));

        let pair = admit_persistent_directional_sdma_pair_v1(pair_observation(17, 23)).unwrap();
        assert_eq!(
            pair.queue_id(Gfx942PersistentSdmaDirectionV1::HostToDevice),
            17
        );
        assert_eq!(
            pair.queue_id(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
            23
        );
    }

    #[test]
    fn recoverable_publication_restores_exact_owners_in_both_directions() {
        for (index, direction) in [
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        ]
        .into_iter()
        .enumerate()
        {
            let (allocation, host) = promoted_fixture(20 + index as u64, 2048);
            let identity = allocation.attachment.storage_identity;
            let (prepared, request, _) = prepared_fixture(allocation, host, None, direction, 1);
            let DirectionalPersistentSdmaPublicationTransitionV1::Retryable { allocation, host } =
                transition_directional_persistent_sdma_publication_v1(
                    prepared,
                    DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request),
                    true,
                    true,
                )
            else {
                panic!("clean lower rejection must be retryable")
            };
            assert_eq!(allocation.attachment.storage_identity, identity);
            assert_eq!(host.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
            assert!(allocation.owner.local_native_is_attached_for_sdma());
        }
    }

    #[test]
    fn retained_and_substituted_ticket_publications_are_terminal() {
        let (allocation, host) = promoted_fixture(30, 2048);
        let (prepared, _request, ticket) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            1,
        );
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Retained(ticket),
                true,
                true,
            )
        else {
            panic!("retained lower custody must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained
        );

        let (allocation, host) = promoted_fixture(31, 2048);
        let (prepared, _request, _ticket) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            1,
        );
        let substituted = persistent_sdma_ticket_coordinates_for_test(queue_key(), 24, 1, 1);
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Confirmed(substituted),
                true,
                true,
            )
        else {
            panic!("substituted full ticket must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
        );
    }

    #[test]
    fn every_full_ticket_coordinate_is_authenticated() {
        for case in 0..4 {
            let (allocation, host) = promoted_fixture(32 + case, 2048);
            let (prepared, _request, _ticket) = prepared_fixture(
                allocation,
                host,
                None,
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                1,
            );
            let substituted = match case {
                0 => persistent_sdma_ticket_coordinates_for_test(
                    queue_key_with_generation(2),
                    23,
                    1,
                    1,
                ),
                1 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 24, 1, 1),
                2 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 23, 2, 1),
                3 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 23, 1, 2),
                _ => unreachable!(),
            };
            let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
                transition_directional_persistent_sdma_publication_v1(
                    prepared,
                    DirectionalPersistentSdmaPublicationObservationV1::Confirmed(substituted),
                    true,
                    true,
                )
            else {
                panic!("ticket substitution case {case} must be terminal")
            };
            assert_eq!(
                custody.stage(),
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
            );
        }
    }

    #[test]
    fn prepared_ticket_must_name_the_selected_child_and_valid_slot_generation() {
        for case in 0..3 {
            let (allocation, host) = promoted_fixture(36 + case, 2048);
            let (mut prepared, _request, _ticket) = prepared_fixture(
                allocation,
                host,
                None,
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                1,
            );
            prepared.planned_ticket = match case {
                0 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 23, 1, 1),
                1 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 17, 64, 1),
                2 => persistent_sdma_ticket_coordinates_for_test(queue_key(), 17, 1, 0),
                _ => unreachable!(),
            };
            let substituted = prepared.planned_ticket;
            let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
                transition_directional_persistent_sdma_publication_v1(
                    prepared,
                    DirectionalPersistentSdmaPublicationObservationV1::Confirmed(substituted),
                    true,
                    true,
                )
            else {
                panic!("invalid prepared ticket case {case} must be terminal")
            };
            assert_eq!(
                custody.stage(),
                Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
            );
        }
    }

    #[test]
    fn pending_timeout_and_exact_completion_preserve_direction_and_size() {
        let (submission, request) =
            published_fixture(40, Gfx942PersistentSdmaDirectionV1::DeviceToHost);
        let DirectionalPersistentSdmaCompletionTransitionV1::Pending(submission) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Pending,
                true,
            )
        else {
            unreachable!()
        };
        let DirectionalPersistentSdmaCompletionTransitionV1::Timeout(submission) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Timeout,
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            submission.direction(),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost
        );
        assert_eq!(submission.copy_bytes(), 32);
        let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(
                    request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            completed.direction(),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost
        );
        assert_eq!(completed.copy_bytes(), 32);
        let completed = completed.into_single_packet_window_v1();
        assert_eq!(completed.packet_count(), 1);
        assert_eq!(completed.host_offset(), 8);
        assert_eq!(completed.device_offset(), 16);
        assert_eq!(completed.copy_bytes(), 32);
    }

    #[test]
    fn next_use_requires_exact_frontier_retirement_not_dependency_chaining() {
        let (submission, request) =
            published_fixture(45, Gfx942PersistentSdmaDirectionV1::HostToDevice);
        let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(
                    request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (mut allocation, host, frontier) = completed.into_parts();
        let error = allocation
            .owner
            .reserve(
                Gfx942PersistentUseRequestV1::new(
                    Gfx942PersistentOperationV1::LocalSdmaDestination,
                    16,
                    32,
                )
                .unwrap(),
                None,
            )
            .expect_err("an unretired overlapping frontier must block the next use");
        assert_eq!(
            error.error(),
            Gfx942PersistentUseErrorV1::DependencyRequired
        );
        let allocation = allocation
            .retire_settled_frontier_v1(frontier)
            .expect("the exact frontier must retire");
        let (_prepared, _request, _ticket) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            2,
        );
    }

    #[test]
    fn demotion_requires_exact_frontier_retirement() {
        let (submission, request) =
            published_fixture(46, Gfx942PersistentSdmaDirectionV1::DeviceToHost);
        let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(
                    request,
                )),
                true,
            )
        else {
            unreachable!()
        };
        let (allocation, _host, frontier) = completed.into_parts();
        let (error, allocation) = demote_directional_persistent_sdma_custody_v1(allocation, 2)
            .expect_err("an unretired frontier must block demotion");
        assert_eq!(error, Gfx942PersistentUseErrorV1::OutstandingUses);
        let allocation = allocation
            .retire_settled_frontier_v1(frontier)
            .expect("the exact frontier must retire");
        let (buffer, outstanding) = demote_directional_persistent_sdma_custody_v1(allocation, 2)
            .expect("retired custody must demote");
        assert_eq!(buffer.kind(), Gfx942SdmaBufferKindV1::DeviceLocal);
        assert_eq!(outstanding, 2);
    }

    #[test]
    fn closing_currentness_loss_is_terminal_before_and_after_completion() {
        let (allocation, host) = promoted_fixture(50, 2048);
        let (prepared, request, _) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            1,
        );
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request),
                true,
                false,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedRestored
        );

        let (submission, request) =
            published_fixture(51, Gfx942PersistentSdmaDirectionV1::HostToDevice);
        let DirectionalPersistentSdmaCompletionTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_completion_v1(
                submission,
                DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(
                    request,
                )),
                false,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
        );
    }

    #[test]
    fn host_and_range_substitution_are_terminal() {
        let (allocation, host) = promoted_fixture(60, 2048);
        let (prepared, mut request, _) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            1,
        );
        request.source_offset += 1;
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Recoverable(request),
                true,
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
        );

        let (allocation, host) = promoted_fixture(61, 2048);
        let (prepared, request, _) = prepared_fixture(
            allocation,
            host,
            None,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            1,
        );
        let (_unused, foreign_host) = persistent_sdma_buffers_for_test(queue_key(), 999);
        let Gfx942SdmaCopyRequestV1 {
            source: device,
            source_offset,
            destination: _,
            destination_offset,
            copy_bytes,
        } = request;
        let substituted = Gfx942SdmaCopyRequestV1::new(
            device,
            source_offset,
            foreign_host,
            destination_offset,
            copy_bytes,
        );
        let DirectionalPersistentSdmaPublicationTransitionV1::ProcessTeardown(custody) =
            transition_directional_persistent_sdma_publication_v1(
                prepared,
                DirectionalPersistentSdmaPublicationObservationV1::Recoverable(substituted),
                true,
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            custody.stage(),
            Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedUnrestored
        );
    }

    fn exercise_sequential_directions(
        id: u64,
        direction_for_cycle: impl Fn(usize) -> Gfx942PersistentSdmaDirectionV1,
    ) {
        let (mut allocation, mut host) = promoted_fixture(id, 2048);
        for cycle in 0..(crate::GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 + 2) {
            let direction = direction_for_cycle(cycle);
            let generation = u32::try_from(cycle + 1).unwrap();
            let (prepared, request, ticket) =
                prepared_fixture(allocation, host, None, direction, generation);
            let DirectionalPersistentSdmaPublicationTransitionV1::Published(submission) =
                transition_directional_persistent_sdma_publication_v1(
                    prepared,
                    DirectionalPersistentSdmaPublicationObservationV1::Confirmed(ticket),
                    true,
                    true,
                )
            else {
                panic!("cycle {cycle} must publish")
            };
            assert_eq!(submission.direction(), direction);
            let DirectionalPersistentSdmaCompletionTransitionV1::Completed(completed) =
                transition_directional_persistent_sdma_completion_v1(
                    submission,
                    DirectionalPersistentSdmaCompletionObservationV1::Completed(completed_request(
                        request,
                    )),
                    true,
                )
            else {
                panic!("cycle {cycle} must complete")
            };
            let (next_allocation, next_host, frontier) = completed.into_parts();
            allocation = next_allocation
                .retire_settled_frontier_v1(frontier)
                .unwrap_or_else(|_| panic!("cycle {cycle} frontier must retire"));
            host = next_host;
        }
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn more_than_64_repeated_same_direction_uses_are_admitted() {
        exercise_sequential_directions(70, |_| Gfx942PersistentSdmaDirectionV1::HostToDevice);
        exercise_sequential_directions(71, |_| Gfx942PersistentSdmaDirectionV1::DeviceToHost);
    }

    #[test]
    fn more_than_64_arbitrarily_alternating_directions_are_admitted() {
        exercise_sequential_directions(72, |cycle| {
            if cycle.is_multiple_of(3) {
                Gfx942PersistentSdmaDirectionV1::DeviceToHost
            } else {
                Gfx942PersistentSdmaDirectionV1::HostToDevice
            }
        });
    }
}
