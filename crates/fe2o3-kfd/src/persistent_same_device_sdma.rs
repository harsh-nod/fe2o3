//! Two-allocation same-device persistent SDMA windows on one directional queue pair.

use std::fmt;

use crate::persistent_allocation::{
    Gfx942PersistentDependencyFrontierV1, Gfx942PersistentOperationV1, Gfx942PersistentPreparedV1,
    Gfx942PersistentPublishedV1, Gfx942PersistentQuarantineReasonV1, Gfx942PersistentUseLeaseV1,
    Gfx942PersistentUseRequestV1, cancel_prepared_local_sdma_pair_v1, complete_local_sdma_pair_v1,
    publish_local_sdma_pair_v1, quarantine_prepared_local_sdma_pair_v1,
    quarantine_published_local_sdma_pair_v1, restore_local_native_pair_from_sdma_v1,
    retire_settled_local_sdma_pair_v1, settle_completed_local_sdma_pair_v1,
};
use crate::persistent_directional_sdma::Gfx942DirectionalQueuePersistentAllocationV1;
use crate::queue::ComputeAqlQueueSessionErrorV1;
use crate::sdma::{
    CompletedPersistentSdmaWindowV1, Gfx942SdmaBufferKindV1, Gfx942SdmaBufferStorageV1,
    Gfx942SdmaBufferV1, Gfx942SdmaCopyRequestV1, Gfx942SdmaCopyTicketV1,
    planned_ticket_matches_queue_occurrence,
};

pub const GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1: usize =
    crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1;
pub const GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_BYTES_V1: u64 =
    GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1 as u64
        * crate::sdma::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64;

pub const GFX942_SAME_DEVICE_PERSISTENT_SDMA_WINDOW_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-same-device-persistent-sdma-r23-window-v1\n",
    "parent=fe2o3-gfx942-kfd-persistent-directional-local-sdma-r22-window-v1\n",
    "window=two-distinct-persistent-device-allocation-owners,source-read-and-destination-write,1..63-contiguous-linear-copy-packets\n",
    "binding=one-exact-parent-queue-vm-and-device-occurrence,one-current-directional-child-pair,distinct-storage-identities,fixed-h2d-child\n",
    "publication=all-native-capacity-range-alias-packet-completion-and-ticket-preflight-before-native-mutation,one-release-write-pointer-publication,one-final-release-doorbell\n",
    "completion=ordered-full-ticket-and-lower-record-authentication,pending-and-timeout-retain-both-owners,no-prefix-retirement,exact-completion-restores-and-settles-two-frontiers\n",
    "failure=clean-prepublication-restores-and-cancels-both-owners,retained-or-postpublication-ambiguity-quarantines-both-owners-and-poisons-session\n",
    "limits=same-device-local-d2d-only,no-striped-set,no-peer-or-xgmi,no-compute,no-overlapping-or-identical-storage\n",
    "evidence=native-neutral-host-custody-and-failure-injection-tests-only,no-native-hardware-execution-or-performance-evidence\n",
    "proof=abstract-model-separate,no-executable-rust-or-native-refinement\n",
);

pub const GFX942_SAME_DEVICE_PERSISTENT_SDMA_WINDOW_MANIFEST_SHA256_V1: &str =
    "93d1277fe7aa07e0773793a756f4a4797d25e1abd09b5cb7639188a08baaedc7";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
    source_offset: u64,
    destination_offset: u64,
    copy_bytes: u32,
    packet_count: usize,
}

impl Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
    pub const fn source_offset(self) -> u64 {
        self.source_offset
    }

    pub const fn destination_offset(self) -> u64 {
        self.destination_offset
    }

    pub const fn copy_bytes(self) -> u32 {
        self.copy_bytes
    }

    pub const fn packet_count(self) -> usize {
        self.packet_count
    }
}

#[must_use = "published same-device persistent SDMA custody must be observed"]
pub struct Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
    pub(crate) source: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) source_published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    pub(crate) destination: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) destination_published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    pub(crate) tickets: Vec<Gfx942SdmaCopyTicketV1>,
    pub(crate) descriptor: Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
}

impl Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
    pub const fn source_request(&self) -> Gfx942PersistentUseRequestV1 {
        self.source_published.request()
    }

    pub const fn destination_request(&self) -> Gfx942PersistentUseRequestV1 {
        self.destination_published.request()
    }

    pub const fn descriptor(&self) -> Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
        self.descriptor
    }

    pub const fn source_offset(&self) -> u64 {
        self.descriptor.source_offset
    }

    pub const fn destination_offset(&self) -> u64 {
        self.descriptor.destination_offset
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.descriptor.copy_bytes
    }

    pub const fn packet_count(&self) -> usize {
        self.descriptor.packet_count
    }
}

impl fmt::Debug for Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SameDevicePersistentSdmaWindowSubmissionV1")
            .field("source_request", &self.source_request())
            .field("destination_request", &self.destination_request())
            .field("descriptor", &self.descriptor)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942SameDevicePersistentSdmaWindowTerminalStageV1 {
    AdmissionRestored,
    PreparedRestored,
    PreparedUnrestored,
    PreparedQueueRetained,
    PublishedQueueRetained,
    CompletedUnrestored,
}

#[allow(dead_code)]
pub(crate) enum Gfx942SameDevicePersistentSdmaWindowTerminalStateV1 {
    AdmissionRestored {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
    },
    PreparedRestored {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
    },
    PreparedUnrestored {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        request: Gfx942SdmaCopyRequestV1,
    },
    PreparedQueueRetained {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
    PublishedQueueRetained {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
    },
    CompletedUnrestored {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        completed: CompletedPersistentSdmaWindowV1,
    },
}

#[must_use = "terminal same-device persistent SDMA custody requires process teardown"]
pub struct Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
    pub(crate) source_sequence: Option<u64>,
    pub(crate) destination_sequence: Option<u64>,
    pub(crate) descriptor: Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
    pub(crate) state: Gfx942SameDevicePersistentSdmaWindowTerminalStateV1,
}

impl Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
    pub const fn source_sequence(&self) -> Option<u64> {
        self.source_sequence
    }

    pub const fn destination_sequence(&self) -> Option<u64> {
        self.destination_sequence
    }

    pub const fn descriptor(&self) -> Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
        self.descriptor
    }

    pub const fn stage(&self) -> Gfx942SameDevicePersistentSdmaWindowTerminalStageV1 {
        match self.state {
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::AdmissionRestored { .. } => {
                Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::AdmissionRestored
            }
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedRestored { .. } => {
                Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PreparedRestored
            }
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedUnrestored { .. } => {
                Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PreparedUnrestored
            }
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedQueueRetained {
                ..
            } => Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PreparedQueueRetained,
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                ..
            } => Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PublishedQueueRetained,
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::CompletedUnrestored { .. } => {
                Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::CompletedUnrestored
            }
        }
    }
}

impl fmt::Debug for Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1")
            .field("source_sequence", &self.source_sequence)
            .field("destination_sequence", &self.destination_sequence)
            .field("descriptor", &self.descriptor)
            .field("stage", &self.stage())
            .finish_non_exhaustive()
    }
}

#[must_use = "inspect retryable or process-teardown same-device custody"]
#[allow(clippy::large_enum_variant)]
pub enum Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1 {
    Retryable {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
    },
    ProcessTeardown(Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1),
}

#[must_use = "inspect the failure and retain both allocation owners"]
pub struct Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1,
}

impl Gfx942SameDevicePersistentSdmaWindowSubmissionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

#[must_use = "a same-device allocation pair must be retained or demoted"]
pub struct Gfx942SameDevicePersistentSdmaAllocationPairV1 {
    source: Gfx942DirectionalQueuePersistentAllocationV1,
    destination: Gfx942DirectionalQueuePersistentAllocationV1,
}

impl Gfx942SameDevicePersistentSdmaAllocationPairV1 {
    pub fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942DirectionalQueuePersistentAllocationV1,
    ) {
        (self.source, self.destination)
    }
}

#[must_use = "completed same-device persistent SDMA custody must be retained or retired"]
pub struct Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
    source: Gfx942DirectionalQueuePersistentAllocationV1,
    source_frontier: Gfx942PersistentDependencyFrontierV1,
    destination: Gfx942DirectionalQueuePersistentAllocationV1,
    destination_frontier: Gfx942PersistentDependencyFrontierV1,
    descriptor: Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
}

impl Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
    pub const fn descriptor(&self) -> Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
        self.descriptor
    }

    pub const fn source_offset(&self) -> u64 {
        self.descriptor.source_offset
    }

    pub const fn destination_offset(&self) -> u64 {
        self.descriptor.destination_offset
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.descriptor.copy_bytes
    }

    pub const fn packet_count(&self) -> usize {
        self.descriptor.packet_count
    }

    #[allow(clippy::result_large_err)]
    pub fn retire_settled_frontiers_v1(
        mut self,
    ) -> Result<
        Gfx942SameDevicePersistentSdmaAllocationPairV1,
        Gfx942SameDevicePersistentSdmaFrontierRetirementFailureV1,
    > {
        match retire_settled_local_sdma_pair_v1(
            &mut self.source.owner,
            self.source_frontier,
            &mut self.destination.owner,
            self.destination_frontier,
        ) {
            Ok(()) => Ok(Gfx942SameDevicePersistentSdmaAllocationPairV1 {
                source: self.source,
                destination: self.destination,
            }),
            Err((source_frontier, destination_frontier)) => {
                self.source_frontier = source_frontier;
                self.destination_frontier = destination_frontier;
                Err(Gfx942SameDevicePersistentSdmaFrontierRetirementFailureV1 { completed: self })
            }
        }
    }
}

#[must_use = "a rejected paired frontier retirement returns complete custody"]
pub struct Gfx942SameDevicePersistentSdmaFrontierRetirementFailureV1 {
    completed: Gfx942SameDevicePersistentSdmaWindowCompletedV1,
}

impl Gfx942SameDevicePersistentSdmaFrontierRetirementFailureV1 {
    pub fn into_completed(self) -> Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
        self.completed
    }
}

#[must_use = "pending same-device persistent SDMA custody must be polled again"]
pub enum Gfx942SameDevicePersistentSdmaWindowCopyPollV1 {
    Pending(Gfx942SameDevicePersistentSdmaWindowSubmissionV1),
    Completed(Gfx942SameDevicePersistentSdmaWindowCompletedV1),
}

#[must_use = "a timeout returns both owners; terminal custody requires teardown"]
pub enum Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1 {
    Pending(Gfx942SameDevicePersistentSdmaWindowSubmissionV1),
    ProcessTeardown(Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1),
}

#[must_use = "inspect the execution failure and retain its custody"]
pub struct Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1,
}

impl Gfx942SameDevicePersistentSdmaWindowExecutionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

pub(crate) struct SameDevicePersistentSdmaWindowPreparedCustodyV1 {
    pub(crate) source: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) source_prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    pub(crate) destination: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) destination_prepared: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    pub(crate) planned_tickets: Vec<Gfx942SdmaCopyTicketV1>,
    pub(crate) descriptor: Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum SameDevicePersistentSdmaWindowPublicationObservationV1 {
    Recoverable(Gfx942SdmaCopyRequestV1),
    Retained(Vec<Gfx942SdmaCopyTicketV1>),
    Confirmed(Vec<Gfx942SdmaCopyTicketV1>),
}

pub(crate) enum SameDevicePersistentSdmaWindowPublicationTransitionV1 {
    Retryable {
        source: Gfx942DirectionalQueuePersistentAllocationV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
    },
    Published(Gfx942SameDevicePersistentSdmaWindowSubmissionV1),
    ProcessTeardown(Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1),
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum SameDevicePersistentSdmaWindowCompletionObservationV1 {
    Pending,
    Timeout,
    QueueRetained,
    Completed(CompletedPersistentSdmaWindowV1),
}

pub(crate) enum SameDevicePersistentSdmaWindowCompletionTransitionV1 {
    Pending(Gfx942SameDevicePersistentSdmaWindowSubmissionV1),
    Timeout(Gfx942SameDevicePersistentSdmaWindowSubmissionV1),
    Completed(Gfx942SameDevicePersistentSdmaWindowCompletedV1),
    ProcessTeardown(Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1),
}

pub(crate) fn same_device_persistent_sdma_descriptor_v1(
    source_offset: u64,
    destination_offset: u64,
    copy_bytes: u32,
    packet_count: usize,
) -> Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
    Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
        source_offset,
        destination_offset,
        copy_bytes,
        packet_count,
    }
}

pub(crate) fn same_device_persistent_sdma_request_v1(
    source: Gfx942SdmaBufferV1,
    source_offset: u64,
    destination: Gfx942SdmaBufferV1,
    destination_offset: u64,
    copy_bytes: u32,
) -> Gfx942SdmaCopyRequestV1 {
    Gfx942SdmaCopyRequestV1::new(
        source,
        source_offset,
        destination,
        destination_offset,
        copy_bytes,
    )
}

#[allow(clippy::result_large_err)]
pub(crate) fn restore_same_device_persistent_sdma_request_v1(
    mut source_owner: Gfx942DirectionalQueuePersistentAllocationV1,
    mut destination_owner: Gfx942DirectionalQueuePersistentAllocationV1,
    descriptor: Gfx942SameDevicePersistentSdmaWindowDescriptorV1,
    request: Gfx942SdmaCopyRequestV1,
) -> Result<
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942DirectionalQueuePersistentAllocationV1,
    ),
    (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942SdmaCopyRequestV1,
    ),
> {
    let buffers_are_exact = request.source_offset == descriptor.source_offset
        && request.destination_offset == descriptor.destination_offset
        && request.copy_bytes == descriptor.copy_bytes
        && request.source.kind() == Gfx942SdmaBufferKindV1::DeviceLocal
        && request.destination.kind() == Gfx942SdmaBufferKindV1::DeviceLocal
        && request.source.belongs_to(source_owner.attachment.queue)
        && request
            .destination
            .belongs_to(destination_owner.attachment.queue)
        && request.source.storage_identity() == source_owner.attachment.storage_identity
        && request.destination.storage_identity() == destination_owner.attachment.storage_identity
        && request.source.storage_identity() != request.destination.storage_identity()
        && request.source.pool_generation() == source_owner.attachment.pool_generation
        && request.destination.pool_generation() == destination_owner.attachment.pool_generation
        && request.source.requested_bytes() == source_owner.attachment.logical_bytes
        && request.destination.requested_bytes() == destination_owner.attachment.logical_bytes
        && request.source.physical_bytes() == source_owner.attachment.physical_bytes
        && request.destination.physical_bytes() == destination_owner.attachment.physical_bytes
        && !source_owner.owner.local_native_is_attached_for_sdma()
        && !destination_owner.owner.local_native_is_attached_for_sdma();
    if !buffers_are_exact {
        return Err((source_owner, destination_owner, request));
    }
    let Gfx942SdmaCopyRequestV1 {
        source,
        destination,
        ..
    } = request;
    let (source_storage, source_queue, source_pool_generation, source_logical_bytes) =
        source.into_bridge_parts();
    let (
        destination_storage,
        destination_queue,
        destination_pool_generation,
        destination_logical_bytes,
    ) = destination.into_bridge_parts();
    let Gfx942SdmaBufferStorageV1::Device(source_lease) = source_storage else {
        unreachable!("checked same-device source storage")
    };
    let Gfx942SdmaBufferStorageV1::Device(destination_lease) = destination_storage else {
        unreachable!("checked same-device destination storage")
    };
    match restore_local_native_pair_from_sdma_v1(
        &mut source_owner.owner,
        source_lease,
        &mut destination_owner.owner,
        destination_lease,
    ) {
        Ok(()) => Ok((source_owner, destination_owner)),
        Err((_, source_lease, destination_lease)) => Err((
            source_owner,
            destination_owner,
            same_device_persistent_sdma_request_v1(
                Gfx942SdmaBufferV1::from_bridge_parts(
                    Gfx942SdmaBufferStorageV1::Device(source_lease),
                    source_queue,
                    source_pool_generation,
                    source_logical_bytes,
                ),
                descriptor.source_offset,
                Gfx942SdmaBufferV1::from_bridge_parts(
                    Gfx942SdmaBufferStorageV1::Device(destination_lease),
                    destination_queue,
                    destination_pool_generation,
                    destination_logical_bytes,
                ),
                descriptor.destination_offset,
                descriptor.copy_bytes,
            ),
        )),
    }
}

fn prepared_terminal(
    mut custody: SameDevicePersistentSdmaWindowPreparedCustodyV1,
    request: Gfx942SdmaCopyRequestV1,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
    let source_sequence = custody.source_prepared.sequence();
    let destination_sequence = custody.destination_prepared.sequence();
    let descriptor = custody.descriptor;
    let state = match restore_same_device_persistent_sdma_request_v1(
        custody.source,
        custody.destination,
        descriptor,
        request,
    ) {
        Ok((source, destination)) => {
            custody.source = source;
            custody.destination = destination;
            quarantine_prepared_local_sdma_pair_v1(
                &mut custody.source.owner,
                custody.source_prepared,
                &mut custody.destination.owner,
                custody.destination_prepared,
                reason,
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "private prepared source/destination leases: {:?}",
                    failure.error
                )
            });
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedRestored {
                source: custody.source,
                destination: custody.destination,
            }
        }
        Err((mut source, mut destination, request)) => {
            quarantine_prepared_local_sdma_pair_v1(
                &mut source.owner,
                custody.source_prepared,
                &mut destination.owner,
                custody.destination_prepared,
                reason,
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "private prepared source/destination leases: {:?}",
                    failure.error
                )
            });
            Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedUnrestored {
                source,
                destination,
                request,
            }
        }
    };
    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
        source_sequence: Some(source_sequence),
        destination_sequence: Some(destination_sequence),
        descriptor,
        state,
    }
}

pub(crate) fn transition_same_device_persistent_sdma_window_publication_v1(
    custody: SameDevicePersistentSdmaWindowPreparedCustodyV1,
    observation: SameDevicePersistentSdmaWindowPublicationObservationV1,
    enclosing_operation_succeeded: bool,
    closing_currentness_succeeded: bool,
) -> SameDevicePersistentSdmaWindowPublicationTransitionV1 {
    match observation {
        SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request)
            if enclosing_operation_succeeded && closing_currentness_succeeded =>
        {
            let SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                mut source,
                source_prepared,
                mut destination,
                destination_prepared,
                descriptor,
                ..
            } = custody;
            match restore_same_device_persistent_sdma_request_v1(
                source,
                destination,
                descriptor,
                request,
            ) {
                Ok((restored_source, restored_destination)) => {
                    source = restored_source;
                    destination = restored_destination;
                    cancel_prepared_local_sdma_pair_v1(
                        &mut source.owner,
                        source_prepared,
                        &mut destination.owner,
                        destination_prepared,
                    )
                    .unwrap_or_else(|failure| panic!("private prepared source/destination leases: {:?}", failure.error));
                    SameDevicePersistentSdmaWindowPublicationTransitionV1::Retryable {
                        source,
                        destination,
                    }
                }
                Err((source, destination, request)) => {
                    SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                        prepared_terminal(
                            SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                                source,
                                source_prepared,
                                destination,
                                destination_prepared,
                                planned_tickets: Vec::new(),
                                descriptor,
                            },
                            request,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                        ),
                    )
                }
            }
        }
        SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request) => {
            SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                prepared_terminal(
                    custody,
                    request,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                ),
            )
        }
        SameDevicePersistentSdmaWindowPublicationObservationV1::Retained(tickets) => {
            let SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                mut source,
                source_prepared,
                mut destination,
                destination_prepared,
                descriptor,
                ..
            } = custody;
            let source_sequence = source_prepared.sequence();
            let destination_sequence = destination_prepared.sequence();
            quarantine_prepared_local_sdma_pair_v1(
                &mut source.owner,
                source_prepared,
                &mut destination.owner,
                destination_prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "private prepared source/destination leases: {:?}",
                    failure.error
                )
            });
            SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                    source_sequence: Some(source_sequence),
                    destination_sequence: Some(destination_sequence),
                    descriptor,
                    state:
                        Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedQueueRetained {
                            source,
                            destination,
                            tickets,
                        },
                },
            )
        }
        SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets) => {
            let SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                mut source,
                source_prepared,
                mut destination,
                destination_prepared,
                planned_tickets,
                descriptor,
            } = custody;
            let expected_queue = source.attachment.pair.host_to_device_queue_id;
            let roster_exact = planned_tickets.len() == descriptor.packet_count
                && planned_tickets.iter().all(|ticket| {
                    planned_ticket_matches_queue_occurrence(
                        *ticket,
                        source.attachment.queue,
                        expected_queue,
                    )
                })
                && tickets == planned_tickets;
            let (source_published, destination_published) = publish_local_sdma_pair_v1(
                &mut source.owner,
                source_prepared,
                &mut destination.owner,
                destination_prepared,
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "private prepared source/destination leases: {:?}",
                    failure.error
                )
            });
            if enclosing_operation_succeeded && closing_currentness_succeeded && roster_exact {
                return SameDevicePersistentSdmaWindowPublicationTransitionV1::Published(
                    Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
                        source,
                        source_published,
                        destination,
                        destination_published,
                        tickets,
                        descriptor,
                    },
                );
            }
            let source_sequence = source_published.sequence();
            let destination_sequence = destination_published.sequence();
            quarantine_published_local_sdma_pair_v1(
                &mut source.owner,
                source_published,
                &mut destination.owner,
                destination_published,
                if enclosing_operation_succeeded && closing_currentness_succeeded {
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
                } else {
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
                },
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "private published source/destination leases: {:?}",
                    failure.error
                )
            });
            SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(
                Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                    source_sequence: Some(source_sequence),
                    destination_sequence: Some(destination_sequence),
                    descriptor,
                    state: Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                        source,
                        destination,
                        tickets,
                    },
                },
            )
        }
    }
}

pub(crate) fn transition_same_device_persistent_sdma_window_completion_v1(
    mut submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    observation: SameDevicePersistentSdmaWindowCompletionObservationV1,
    enclosing_operation_succeeded: bool,
) -> SameDevicePersistentSdmaWindowCompletionTransitionV1 {
    match observation {
        SameDevicePersistentSdmaWindowCompletionObservationV1::Pending
            if enclosing_operation_succeeded =>
        {
            return SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(submission);
        }
        SameDevicePersistentSdmaWindowCompletionObservationV1::Timeout
            if enclosing_operation_succeeded =>
        {
            submission.source_published = submission
                .source
                .owner
                .observe_timeout(submission.source_published)
                .expect("private published source lease retains timeout custody")
                .into_published();
            submission.destination_published = submission
                .destination
                .owner
                .observe_timeout(submission.destination_published)
                .expect("private published destination lease retains timeout custody")
                .into_published();
            return SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(submission);
        }
        SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed) => {
            let Gfx942SameDevicePersistentSdmaWindowSubmissionV1 {
                source,
                source_published,
                destination,
                destination_published,
                descriptor,
                ..
            } = submission;
            let source_sequence = source_published.sequence();
            let destination_sequence = destination_published.sequence();
            let completed_descriptor = Gfx942SameDevicePersistentSdmaWindowDescriptorV1 {
                source_offset: completed.request.source_offset,
                destination_offset: completed.request.destination_offset,
                copy_bytes: completed.request.copy_bytes,
                packet_count: completed.packet_count,
            };
            if !enclosing_operation_succeeded || completed_descriptor != descriptor {
                let mut source = source;
                let mut destination = destination;
                quarantine_published_local_sdma_pair_v1(
                    &mut source.owner,
                    source_published,
                    &mut destination.owner,
                    destination_published,
                    if enclosing_operation_succeeded {
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
                    } else {
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
                    },
                )
                .unwrap_or_else(|failure| {
                    panic!(
                        "private published source/destination leases: {:?}",
                        failure.error
                    )
                });
                return SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(
                    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                        source_sequence: Some(source_sequence),
                        destination_sequence: Some(destination_sequence),
                        descriptor,
                        state: Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::CompletedUnrestored {
                            source,
                            destination,
                            completed,
                        },
                    },
                );
            }
            return match restore_same_device_persistent_sdma_request_v1(
                source,
                destination,
                descriptor,
                completed.request,
            ) {
                Ok((mut source, mut destination)) => {
                    let (source_completed, destination_completed) = complete_local_sdma_pair_v1(
                        &mut source.owner,
                        source_published,
                        &mut destination.owner,
                        destination_published,
                    )
                    .unwrap_or_else(|failure| {
                        panic!(
                            "private published source/destination leases: {:?}",
                            failure.error
                        )
                    });
                    let (source_frontier, destination_frontier) =
                        settle_completed_local_sdma_pair_v1(
                            &mut source.owner,
                            source_completed,
                            &mut destination.owner,
                            destination_completed,
                        )
                        .unwrap_or_else(|failure| {
                            panic!(
                                "private completed source/destination leases: {:?}",
                                failure.error
                            )
                        });
                    SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(
                        Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
                            source,
                            source_frontier,
                            destination,
                            destination_frontier,
                            descriptor: completed_descriptor,
                        },
                    )
                }
                Err((mut source, mut destination, request)) => {
                    quarantine_published_local_sdma_pair_v1(
                        &mut source.owner,
                        source_published,
                        &mut destination.owner,
                        destination_published,
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                    )
                    .unwrap_or_else(|failure| {
                        panic!(
                            "private published source/destination leases: {:?}",
                            failure.error
                        )
                    });
                    SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(
                        Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
                            source_sequence: Some(source_sequence),
                            destination_sequence: Some(destination_sequence),
                            descriptor,
                            state: Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::CompletedUnrestored {
                                source,
                                destination,
                                completed: CompletedPersistentSdmaWindowV1 {
                                    request,
                                    packet_count: completed.packet_count,
                                },
                            },
                        },
                    )
                }
            };
        }
        SameDevicePersistentSdmaWindowCompletionObservationV1::Pending
        | SameDevicePersistentSdmaWindowCompletionObservationV1::Timeout
        | SameDevicePersistentSdmaWindowCompletionObservationV1::QueueRetained => {}
    }

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
        if enclosing_operation_succeeded {
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate
        } else {
            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss
        },
    )
    .unwrap_or_else(|failure| {
        panic!(
            "private published source/destination leases: {:?}",
            failure.error
        )
    });
    SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(
        Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1 {
            source_sequence: Some(source_sequence),
            destination_sequence: Some(destination_sequence),
            descriptor,
            state: Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
                source,
                destination,
                tickets,
            },
        },
    )
}

pub(crate) fn same_device_source_use_request_v1(
    offset: u64,
    copy_bytes: u32,
) -> Result<Gfx942PersistentUseRequestV1, crate::persistent_allocation::Gfx942PersistentUseErrorV1>
{
    Gfx942PersistentUseRequestV1::new(
        Gfx942PersistentOperationV1::LocalSdmaSource,
        offset,
        u64::from(copy_bytes),
    )
}

pub(crate) fn same_device_destination_use_request_v1(
    offset: u64,
    copy_bytes: u32,
) -> Result<Gfx942PersistentUseRequestV1, crate::persistent_allocation::Gfx942PersistentUseErrorV1>
{
    Gfx942PersistentUseRequestV1::new(
        Gfx942PersistentOperationV1::LocalSdmaDestination,
        offset,
        u64::from(copy_bytes),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistent_allocation::{Gfx942PersistentAccessV1, Gfx942PersistentUseErrorV1};
    use crate::persistent_directional_sdma::{
        admit_persistent_directional_sdma_pair_v1, promote_directional_persistent_sdma_custody_v1,
    };
    use crate::sdma::{
        GFX942_SDMA_D2H_ENGINE_INDEX_V1, GFX942_SDMA_H2D_ENGINE_INDEX_V1,
        GFX942_SDMA_MAX_IN_FLIGHT_V1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
        GFX942_SDMA_RING_BYTES_V1, Gfx942DirectionalSdmaQueueObservationV1,
        Gfx942SdmaQueueObservationV1, persistent_sdma_buffers_for_test,
        persistent_sdma_ticket_coordinates_for_test, persistent_sdma_window_packet_count,
    };
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        QueueKeyV1, VmIdV1, VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    fn queue_key() -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(7),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(3),
            generation: QueueGenerationV1(1),
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

    fn pair() -> crate::persistent_directional_sdma::Gfx942PersistentDirectionalSdmaPairV1 {
        admit_persistent_directional_sdma_pair_v1(Gfx942DirectionalSdmaQueueObservationV1 {
            host_to_device: queue_observation(17, GFX942_SDMA_H2D_ENGINE_INDEX_V1),
            device_to_host: queue_observation(23, GFX942_SDMA_D2H_ENGINE_INDEX_V1),
            admitted_engine_count: 2,
            admitted_queues_per_engine: 8,
        })
        .unwrap()
    }

    fn allocation(id: u64) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        let (mut device, _) = persistent_sdma_buffers_for_test(queue_key(), id);
        device.set_logical_bytes(2048);
        let (allocation, outstanding) =
            promote_directional_persistent_sdma_custody_v1(device, pair(), 2).unwrap();
        assert_eq!(outstanding, 2);
        allocation
    }

    fn prepared_fixture(
        packet_count: usize,
    ) -> (
        SameDevicePersistentSdmaWindowPreparedCustodyV1,
        Gfx942SdmaCopyRequestV1,
        Vec<Gfx942SdmaCopyTicketV1>,
    ) {
        let mut source = allocation(10);
        let mut destination = allocation(20);
        let source_reserved = source
            .owner
            .reserve(same_device_source_use_request_v1(8, 32).unwrap(), None)
            .unwrap();
        let destination_reserved = destination
            .owner
            .reserve(
                same_device_destination_use_request_v1(16, 32).unwrap(),
                None,
            )
            .unwrap();
        let source_prepared = source.owner.prepare(source_reserved).unwrap();
        let destination_prepared = destination.owner.prepare(destination_reserved).unwrap();
        let (source_lease, destination_lease) =
            crate::persistent_allocation::detach_local_native_pair_for_sdma_v1(
                &mut source.owner,
                &mut destination.owner,
            )
            .unwrap();
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
        let descriptor = same_device_persistent_sdma_descriptor_v1(8, 16, 32, packet_count);
        let request =
            same_device_persistent_sdma_request_v1(source_buffer, 8, destination_buffer, 16, 32);
        let tickets = (0..packet_count)
            .map(|index| {
                persistent_sdma_ticket_coordinates_for_test(
                    queue_key(),
                    pair().host_to_device_queue_id,
                    index as u16,
                    1,
                )
            })
            .collect::<Vec<_>>();
        (
            SameDevicePersistentSdmaWindowPreparedCustodyV1 {
                source,
                source_prepared,
                destination,
                destination_prepared,
                planned_tickets: tickets.clone(),
                descriptor,
            },
            request,
            tickets,
        )
    }

    fn published_fixture(
        packet_count: usize,
    ) -> (
        Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
        Gfx942SdmaCopyRequestV1,
    ) {
        let (prepared, request, tickets) = prepared_fixture(packet_count);
        let SameDevicePersistentSdmaWindowPublicationTransitionV1::Published(submission) =
            transition_same_device_persistent_sdma_window_publication_v1(
                prepared,
                SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
                true,
                true,
            )
        else {
            unreachable!()
        };
        (submission, request)
    }

    #[test]
    fn same_device_window_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_SAME_DEVICE_PERSISTENT_SDMA_WINDOW_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_SAME_DEVICE_PERSISTENT_SDMA_WINDOW_MANIFEST_SHA256_V1
        );
    }

    #[test]
    fn same_device_window_packet_bound_is_exact() {
        let maximum = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        assert!(persistent_sdma_window_packet_count(0).is_err());
        assert_eq!(persistent_sdma_window_packet_count(1).unwrap(), 1);
        assert_eq!(
            persistent_sdma_window_packet_count(maximum * 63).unwrap(),
            63
        );
        assert!(persistent_sdma_window_packet_count(maximum * 63 + 1).is_err());
    }

    #[test]
    fn terminal_same_device_admission_absorbs_invalid_geometry_after_affiliation() {
        let source = allocation(31);
        let destination = allocation(32);
        let source_identity = source.attachment.storage_identity;
        let destination_identity = destination.attachment.storage_identity;
        let failure = crate::queue::admit_same_device_persistent_sdma_window_input_v1(
            queue_key(),
            true,
            source,
            u64::MAX,
            destination,
            u64::MAX,
            0,
        )
        .expect_err("terminal custody must dominate invalid same-device geometry");
        let (_, custody) = failure.into_parts();
        let Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(terminal) =
            custody
        else {
            panic!("self-owned terminal inputs must not return retryable custody")
        };
        assert_eq!(
            terminal.stage(),
            Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::AdmissionRestored
        );
        assert_eq!(terminal.descriptor().copy_bytes(), 0);
        assert_eq!(terminal.descriptor().packet_count(), 0);
        let Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::AdmissionRestored {
            source,
            destination,
        } = terminal.state
        else {
            unreachable!()
        };
        assert_eq!(source.attachment.storage_identity, source_identity);
        assert_eq!(
            destination.attachment.storage_identity,
            destination_identity
        );
    }

    #[test]
    fn terminal_receiver_returns_foreign_same_device_inputs_exactly() {
        let source = allocation(33);
        let destination = allocation(34);
        let source_identity = source.attachment.storage_identity;
        let destination_identity = destination.attachment.storage_identity;
        let receiver = QueueKeyV1 {
            generation: QueueGenerationV1(2),
            ..queue_key()
        };
        let failure = crate::queue::admit_same_device_persistent_sdma_window_input_v1(
            receiver,
            true,
            source,
            0,
            destination,
            0,
            1,
        )
        .expect_err("foreign inputs must be returned before terminal absorption");
        let (_, custody) = failure.into_parts();
        let Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
            source,
            destination,
        } = custody
        else {
            panic!("foreign inputs must remain retryable on their producing queue")
        };
        assert_eq!(source.attachment.storage_identity, source_identity);
        assert_eq!(
            destination.attachment.storage_identity,
            destination_identity
        );
        let retry = crate::queue::admit_same_device_persistent_sdma_window_input_v1(
            queue_key(),
            false,
            source,
            0,
            destination,
            0,
            1,
        );
        assert!(retry.is_ok());
    }

    #[test]
    fn same_device_leases_have_distinct_read_and_write_roles() {
        let (prepared, request, _) = prepared_fixture(3);
        assert_eq!(
            prepared.source_prepared.request().operation(),
            Gfx942PersistentOperationV1::LocalSdmaSource
        );
        assert_eq!(
            prepared.source_prepared.request().access(),
            Gfx942PersistentAccessV1::Read
        );
        assert_eq!(
            prepared.destination_prepared.request().operation(),
            Gfx942PersistentOperationV1::LocalSdmaDestination
        );
        assert_eq!(
            prepared.destination_prepared.request().access(),
            Gfx942PersistentAccessV1::Write
        );
        let SameDevicePersistentSdmaWindowPublicationTransitionV1::Retryable {
            source,
            destination,
        } = transition_same_device_persistent_sdma_window_publication_v1(
            prepared,
            SameDevicePersistentSdmaWindowPublicationObservationV1::Recoverable(request),
            true,
            true,
        )
        else {
            unreachable!()
        };
        assert_eq!(source.owner.live_use_count(), 0);
        assert_eq!(destination.owner.live_use_count(), 0);
    }

    #[test]
    fn same_device_pending_timeout_completion_and_retirement_are_paired() {
        let (submission, request) = published_fixture(3);
        let SameDevicePersistentSdmaWindowCompletionTransitionV1::Pending(submission) =
            transition_same_device_persistent_sdma_window_completion_v1(
                submission,
                SameDevicePersistentSdmaWindowCompletionObservationV1::Pending,
                true,
            )
        else {
            unreachable!()
        };
        let SameDevicePersistentSdmaWindowCompletionTransitionV1::Timeout(submission) =
            transition_same_device_persistent_sdma_window_completion_v1(
                submission,
                SameDevicePersistentSdmaWindowCompletionObservationV1::Timeout,
                true,
            )
        else {
            unreachable!()
        };
        let completed_lower = CompletedPersistentSdmaWindowV1 {
            request,
            packet_count: 3,
        };
        let SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
            transition_same_device_persistent_sdma_window_completion_v1(
                submission,
                SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(completed_lower),
                true,
            )
        else {
            unreachable!()
        };
        assert_eq!(
            completed.descriptor(),
            same_device_persistent_sdma_descriptor_v1(8, 16, 32, 3)
        );
        let pair = match completed.retire_settled_frontiers_v1() {
            Ok(pair) => pair,
            Err(_) => panic!("exact paired frontier retirement must succeed"),
        };
        let (source, destination) = pair.into_parts();
        assert_eq!(source.owner.retained_settled_use_count(), 0);
        assert_eq!(destination.owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn same_device_completion_coordinate_substitution_is_terminal() {
        let (submission, mut request) = published_fixture(3);
        request.destination_offset += 1;
        let transition = transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        );
        let SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
            transition
        else {
            panic!("coordinate substitution must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::CompletedUnrestored
        );
    }

    #[test]
    fn same_device_completion_source_destination_substitution_is_terminal() {
        let (submission, mut request) = published_fixture(2);
        std::mem::swap(&mut request.source, &mut request.destination);
        std::mem::swap(&mut request.source_offset, &mut request.destination_offset);
        let transition = transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 2,
                },
            ),
            true,
        );
        assert!(matches!(
            transition,
            SameDevicePersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(_)
        ));
    }

    #[test]
    fn same_device_swapped_frontiers_do_not_partially_retire() {
        let (submission, request) = published_fixture(2);
        let transition = transition_same_device_persistent_sdma_window_completion_v1(
            submission,
            SameDevicePersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 2,
                },
            ),
            true,
        );
        let SameDevicePersistentSdmaWindowCompletionTransitionV1::Completed(completed) = transition
        else {
            unreachable!()
        };
        let Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
            source,
            source_frontier,
            destination,
            destination_frontier,
            descriptor,
        } = completed;
        let substituted = Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
            source,
            source_frontier: destination_frontier,
            destination,
            destination_frontier: source_frontier,
            descriptor,
        };
        let failure = match substituted.retire_settled_frontiers_v1() {
            Ok(_) => panic!("swapped frontiers must not retire"),
            Err(failure) => failure,
        };
        let Gfx942SameDevicePersistentSdmaWindowCompletedV1 {
            source,
            destination,
            ..
        } = failure.into_completed();
        assert_eq!(source.owner.retained_settled_use_count(), 1);
        assert_eq!(destination.owner.retained_settled_use_count(), 1);
    }

    #[test]
    fn same_device_retained_and_ticket_substitution_quarantine_both_owners() {
        let (prepared, _, tickets) = prepared_fixture(3);
        let transition = transition_same_device_persistent_sdma_window_publication_v1(
            prepared,
            SameDevicePersistentSdmaWindowPublicationObservationV1::Retained(tickets),
            true,
            true,
        );
        let SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
            transition
        else {
            panic!("retained publication must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PreparedQueueRetained
        );
        let Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PreparedQueueRetained {
            source,
            destination,
            ..
        } = custody.state
        else {
            unreachable!()
        };
        assert_eq!(
            source.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
        );
        assert_eq!(
            destination.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
        );

        let (prepared, _, mut tickets) = prepared_fixture(3);
        tickets[1] = persistent_sdma_ticket_coordinates_for_test(queue_key(), 17, 1, 2);
        let transition = transition_same_device_persistent_sdma_window_publication_v1(
            prepared,
            SameDevicePersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
            true,
            true,
        );
        let SameDevicePersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
            transition
        else {
            panic!("ticket substitution must be terminal")
        };
        assert_eq!(
            custody.stage(),
            Gfx942SameDevicePersistentSdmaWindowTerminalStageV1::PublishedQueueRetained
        );
        let Gfx942SameDevicePersistentSdmaWindowTerminalStateV1::PublishedQueueRetained {
            source,
            destination,
            ..
        } = custody.state
        else {
            unreachable!()
        };
        assert_eq!(
            source.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
        );
        assert_eq!(
            destination.owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate)
        );
    }

    #[test]
    fn same_device_identical_storage_is_rejected_before_pair_publication() {
        let mut source = allocation(40);
        let mut destination = allocation(40);
        let source_reserved = source
            .owner
            .reserve(same_device_source_use_request_v1(0, 32).unwrap(), None)
            .unwrap();
        let destination_reserved = destination
            .owner
            .reserve(
                same_device_destination_use_request_v1(64, 32).unwrap(),
                None,
            )
            .unwrap();
        let source_prepared = source.owner.prepare(source_reserved).unwrap();
        let destination_prepared = destination.owner.prepare(destination_reserved).unwrap();
        let failure = publish_local_sdma_pair_v1(
            &mut source.owner,
            source_prepared,
            &mut destination.owner,
            destination_prepared,
        )
        .expect_err("identical storage identity must be rejected");
        assert_eq!(
            failure.error,
            Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration
        );
        assert_eq!(failure.source.request().range().offset(), 0);
        assert_eq!(failure.destination.request().range().offset(), 64);
    }

    #[test]
    fn same_device_lower_selector_stays_additive() {
        let source = include_str!("sdma.rs");
        let selector = source
            .split("fn prepare_same_device_persistent_window_recoverable")
            .nth(1)
            .expect("same-device selector")
            .split("pub(crate) fn submit")
            .next()
            .unwrap();
        assert!(selector.contains("GFX942_SDMA_H2D_OWNER_SLOT_V1"));
        assert!(selector.contains("prepare_persistent_window_recoverable"));
        let directional = source
            .split("fn owner_for_copy")
            .nth(1)
            .unwrap()
            .split("fn owner_for_requests")
            .next()
            .unwrap();
        assert!(directional.contains("admits only H2D or D2H copies"));
    }
}
