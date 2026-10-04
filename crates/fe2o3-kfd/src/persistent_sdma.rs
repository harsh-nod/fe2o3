//! Queue-bound persistent-allocation custody for targeted local SDMA.
//!
//! The public types in this module are addressless, move-only receipts. Native
//! orchestration remains in the queue session so the existing SDMA outstanding
//! buffer ledger and queue records remain the only resource accounting.

use std::fmt;

use fe2o3_runtime_model::QueueKeyV1;

use crate::persistent_allocation::{
    Gfx942PersistentDependencyFrontierV1, Gfx942PersistentDeviceAllocationV1,
    Gfx942PersistentPublishedV1, Gfx942PersistentUseLeaseV1, Gfx942PersistentUseRequestV1,
};
use crate::queue::ComputeAqlQueueSessionErrorV1;
use crate::sdma::{
    Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferV1, Gfx942SdmaCompletedCopyV1,
    Gfx942SdmaCopyTicketV1,
};

/// The executable R18 adapter is deliberately capped to the R17 model bound.
pub const GFX942_PERSISTENT_SDMA_MAX_ALLOCATION_BYTES_V1: u64 = 256 * 1024 * 1024;

/// Frozen claim boundary for the initial queue-integrated adapter.
pub const GFX942_PERSISTENT_LOCAL_SDMA_ADAPTER_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-persistent-local-sdma-r18-v1\n",
    "target=gfx942:xnack-,single-targeted-ordinary-sdma-engine,engine-1-h2d-or-engine-0-d2h\n",
    "admission=one-existing-queue-owned-full-page-multiple-device-buffer,1..268435456-bytes,one-ordinary-host-buffer\n",
    "binding=exact-parent-queue-occurrence,native-child-queue-id,engine-index,pool-generation,mapped-allocation-identity-and-extent,persistent-owner-incarnation,host-storage-identity-generation-and-extents,planned-ticket-slot-and-generation\n",
    "ledger=one-existing-sdma-outstanding-buffer-debit-preserved-across-promotion-submission-completion-and-demotion\n",
    "lifecycle=reserve-prepare-detach-submit,confirmed-only-publish,pending-or-timeout-retains-submission,exact-completion-restores-completes-settles,exact-quiescent-frontier-retirement-reclaims-settled-ledger\n",
    "failure=recoverable-prepublication-restores-and-cancels,retained-publication-quarantines-prepared,postpublication-uncertainty-is-opaque-process-teardown\n",
    "limits=single-flight,no-directional-set,no-striped-set,no-peer-or-xgmi,no-compute,no-concurrent-range-borrows\n",
    "evidence=host-custody-and-failure-injection-tests-only,no-native-hardware-execution-or-performance-evidence\n",
    "proof=abstract-model-separate,no-executable-rust-or-native-refinement\n",
);

pub const GFX942_PERSISTENT_LOCAL_SDMA_ADAPTER_MANIFEST_SHA256_V1: &str =
    "632334aac8cdf6084bbdaec96a0c89f24f73e4e7372c4ded05a40ea81e72b62c";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentSdmaDirectionV1 {
    HostToDevice,
    DeviceToHost,
}

impl Gfx942PersistentSdmaDirectionV1 {
    pub const fn engine_index(self) -> u32 {
        match self {
            Self::HostToDevice => crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1,
            Self::DeviceToHost => crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942PersistentSdmaAttachmentV1 {
    pub(crate) queue: QueueKeyV1,
    pub(crate) native_queue_id: u32,
    pub(crate) engine_index: u32,
    pub(crate) pool_generation: u64,
    pub(crate) logical_bytes: u64,
    pub(crate) physical_bytes: u64,
    pub(crate) storage_identity: Gfx942SdmaBufferStorageIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942PersistentSdmaHostBindingV1 {
    queue: QueueKeyV1,
    storage_identity: Gfx942SdmaBufferStorageIdentityV1,
    pool_generation: u64,
    logical_bytes: u64,
    physical_bytes: u64,
}

impl Gfx942PersistentSdmaHostBindingV1 {
    pub(crate) fn capture(host: &Gfx942SdmaBufferV1, queue: QueueKeyV1) -> Self {
        Self {
            queue,
            storage_identity: host.storage_identity(),
            pool_generation: host.pool_generation(),
            logical_bytes: host.requested_bytes(),
            physical_bytes: host.physical_bytes(),
        }
    }

    pub(crate) fn matches(&self, host: &Gfx942SdmaBufferV1) -> bool {
        host.belongs_to(self.queue)
            && host.storage_identity() == self.storage_identity
            && host.pool_generation() == self.pool_generation
            && host.requested_bytes() == self.logical_bytes
            && host.physical_bytes() == self.physical_bytes
    }
}

/// One device-local SDMA allocation promoted without changing the queue's
/// outstanding-buffer count.
///
/// The initial adapter is single-flight: submission consumes this owner and a
/// completion returns it. It supports only a single targeted local SDMA queue
/// on engine 1 for H2D or engine 0 for D2H.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942QueuePersistentAllocationV1;
/// fn cannot_clone(value: Gfx942QueuePersistentAllocationV1) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942QueuePersistentAllocationV1;
/// fn require_send<T: Send>(_: T) {}
/// fn cannot_send(value: Gfx942QueuePersistentAllocationV1) {
///     require_send(value);
/// }
/// ```
#[must_use = "persistent queue-bound allocation custody must be retained or demoted"]
pub struct Gfx942QueuePersistentAllocationV1 {
    pub(crate) owner: Gfx942PersistentDeviceAllocationV1,
    pub(crate) attachment: Gfx942PersistentSdmaAttachmentV1,
}

impl Gfx942QueuePersistentAllocationV1 {
    pub const fn byte_len(&self) -> u64 {
        self.attachment.logical_bytes
    }

    pub const fn engine_index(&self) -> u32 {
        self.attachment.engine_index
    }

    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        if self.attachment.engine_index == crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1 {
            Gfx942PersistentSdmaDirectionV1::HostToDevice
        } else {
            Gfx942PersistentSdmaDirectionV1::DeviceToHost
        }
    }

    /// Retires the exact latest settled frontier after this single-flight
    /// allocation has returned to quiescence. This is a host ledger transition
    /// only; it preserves the attached native allocation and queue buffer debit.
    #[allow(clippy::result_large_err)]
    pub fn retire_settled_frontier_v1(
        mut self,
        frontier: Gfx942PersistentDependencyFrontierV1,
    ) -> Result<Self, Gfx942PersistentSdmaFrontierRetirementFailureV1> {
        match self.owner.retire_settled_frontier(frontier) {
            Ok(()) => Ok(self),
            Err(frontier) => Err(Gfx942PersistentSdmaFrontierRetirementFailureV1 {
                allocation: self,
                frontier,
            }),
        }
    }
}

impl fmt::Debug for Gfx942QueuePersistentAllocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942QueuePersistentAllocationV1")
            .field("byte_len", &self.byte_len())
            .field("engine_index", &self.engine_index())
            .field("direction", &self.direction())
            .finish_non_exhaustive()
    }
}

/// A rejected retirement returns both move-only inputs unchanged.
#[must_use = "a rejected frontier retirement returns allocation and frontier custody"]
pub struct Gfx942PersistentSdmaFrontierRetirementFailureV1 {
    allocation: Gfx942QueuePersistentAllocationV1,
    frontier: Gfx942PersistentDependencyFrontierV1,
}

impl fmt::Debug for Gfx942PersistentSdmaFrontierRetirementFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentSdmaFrontierRetirementFailureV1")
            .field("allocation", &self.allocation)
            .field("frontier", &self.frontier)
            .finish()
    }
}

impl Gfx942PersistentSdmaFrontierRetirementFailureV1 {
    pub fn into_parts(
        self,
    ) -> (
        Gfx942QueuePersistentAllocationV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        (self.allocation, self.frontier)
    }
}

#[must_use = "a recoverable promotion failure returns the original SDMA buffer"]
pub struct Gfx942PersistentSdmaPromotionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) recovered: Option<Gfx942SdmaBufferV1>,
}

impl Gfx942PersistentSdmaPromotionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(self) -> (ComputeAqlQueueSessionErrorV1, Option<Gfx942SdmaBufferV1>) {
        (self.error, self.recovered)
    }
}

#[must_use = "a recoverable demotion failure returns the persistent allocation"]
pub struct Gfx942PersistentSdmaDemotionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) recovered: Option<Gfx942QueuePersistentAllocationV1>,
}

impl Gfx942PersistentSdmaDemotionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Gfx942QueuePersistentAllocationV1>,
    ) {
        (self.error, self.recovered)
    }
}

/// Published single-flight custody. The device allocation is held by the
/// queue record; this receipt retains its R17 owner and exact ticket.
#[must_use = "published persistent SDMA custody must be observed to completion"]
pub struct Gfx942PersistentSdmaSubmissionV1 {
    pub(crate) allocation: Gfx942QueuePersistentAllocationV1,
    pub(crate) published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    pub(crate) ticket: Gfx942SdmaCopyTicketV1,
    pub(crate) host_binding: Gfx942PersistentSdmaHostBindingV1,
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) host_offset: u64,
    pub(crate) device_offset: u64,
    pub(crate) copy_bytes: u32,
}

impl Gfx942PersistentSdmaSubmissionV1 {
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

impl fmt::Debug for Gfx942PersistentSdmaSubmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentSdmaSubmissionV1")
            .field("request", &self.request())
            .field("direction", &self.direction)
            .field("copy_bytes", &self.copy_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentSdmaTerminalStageV1 {
    AdmissionRestored,
    PreparedRestored,
    PreparedUnrestored,
    PreparedQueueRetained,
    PublishedQueueRetained,
    CompletedUnrestored,
}

#[allow(dead_code)]
pub(crate) enum Gfx942PersistentSdmaTerminalStateV1 {
    AdmissionRestored {
        allocation: Gfx942QueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    PreparedRestored {
        allocation: Gfx942QueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    PreparedUnrestored {
        allocation: Gfx942QueuePersistentAllocationV1,
        request: crate::sdma::Gfx942SdmaCopyRequestV1,
    },
    PreparedQueueRetained {
        allocation: Gfx942QueuePersistentAllocationV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
    PublishedQueueRetained {
        allocation: Gfx942QueuePersistentAllocationV1,
        ticket: Gfx942SdmaCopyTicketV1,
    },
    CompletedUnrestored {
        allocation: Gfx942QueuePersistentAllocationV1,
        completed: Gfx942SdmaCompletedCopyV1,
    },
}

/// Opaque terminal custody. It exposes observations only; cleanup requires
/// process teardown because queue currentness or publication is indeterminate.
#[must_use = "terminal native custody must be retained until process teardown"]
pub struct Gfx942PersistentSdmaTerminalCustodyV1 {
    pub(crate) direction: Gfx942PersistentSdmaDirectionV1,
    pub(crate) sequence: Option<u64>,
    pub(crate) state: Gfx942PersistentSdmaTerminalStateV1,
}

impl Gfx942PersistentSdmaTerminalCustodyV1 {
    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn sequence(&self) -> Option<u64> {
        self.sequence
    }

    pub const fn stage(&self) -> Gfx942PersistentSdmaTerminalStageV1 {
        match self.state {
            Gfx942PersistentSdmaTerminalStateV1::AdmissionRestored { .. } => {
                Gfx942PersistentSdmaTerminalStageV1::AdmissionRestored
            }
            Gfx942PersistentSdmaTerminalStateV1::PreparedRestored { .. } => {
                Gfx942PersistentSdmaTerminalStageV1::PreparedRestored
            }
            Gfx942PersistentSdmaTerminalStateV1::PreparedUnrestored { .. } => {
                Gfx942PersistentSdmaTerminalStageV1::PreparedUnrestored
            }
            Gfx942PersistentSdmaTerminalStateV1::PreparedQueueRetained { .. } => {
                Gfx942PersistentSdmaTerminalStageV1::PreparedQueueRetained
            }
            Gfx942PersistentSdmaTerminalStateV1::PublishedQueueRetained { .. } => {
                Gfx942PersistentSdmaTerminalStageV1::PublishedQueueRetained
            }
            Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored { .. } => {
                Gfx942PersistentSdmaTerminalStageV1::CompletedUnrestored
            }
        }
    }
}

impl fmt::Debug for Gfx942PersistentSdmaTerminalCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentSdmaTerminalCustodyV1")
            .field("direction", &self.direction)
            .field("sequence", &self.sequence)
            .field("stage", &self.stage())
            .finish_non_exhaustive()
    }
}

#[must_use = "inspect retryable or process-teardown custody"]
#[allow(clippy::large_enum_variant)]
pub enum Gfx942PersistentSdmaSubmissionCustodyV1 {
    Retryable {
        allocation: Gfx942QueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
    },
    ProcessTeardown(Gfx942PersistentSdmaTerminalCustodyV1),
}

#[must_use = "inspect the failure and retain the returned custody"]
pub struct Gfx942PersistentSdmaSubmissionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942PersistentSdmaSubmissionCustodyV1,
}

impl Gfx942PersistentSdmaSubmissionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942PersistentSdmaSubmissionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

#[must_use = "completed persistent custody must be retained or demoted"]
pub struct Gfx942PersistentSdmaCompletedV1 {
    allocation: Gfx942QueuePersistentAllocationV1,
    host: Gfx942SdmaBufferV1,
    frontier: Gfx942PersistentDependencyFrontierV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    copy_bytes: u32,
}

/// Nonblocking observation of one confirmed persistent SDMA publication.
/// Pending returns the exact move-only submission unchanged.
#[must_use = "pending persistent SDMA custody must be retained and polled again"]
pub enum Gfx942PersistentSdmaCopyPollV1 {
    Pending(Gfx942PersistentSdmaSubmissionV1),
    Completed(Gfx942PersistentSdmaCompletedV1),
}

impl Gfx942PersistentSdmaCompletedV1 {
    pub const fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        self.direction
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.copy_bytes
    }

    pub fn into_parts(
        self,
    ) -> (
        Gfx942QueuePersistentAllocationV1,
        Gfx942SdmaBufferV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        (self.allocation, self.host, self.frontier)
    }

    pub(crate) fn new(
        allocation: Gfx942QueuePersistentAllocationV1,
        host: Gfx942SdmaBufferV1,
        frontier: Gfx942PersistentDependencyFrontierV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        copy_bytes: u32,
    ) -> Self {
        Self {
            allocation,
            host,
            frontier,
            direction,
            copy_bytes,
        }
    }
}

#[must_use = "a timeout returns the published submission; terminal custody requires teardown"]
pub enum Gfx942PersistentSdmaExecutionCustodyV1 {
    Pending(Gfx942PersistentSdmaSubmissionV1),
    ProcessTeardown(Gfx942PersistentSdmaTerminalCustodyV1),
}

#[must_use = "inspect the execution failure and retain its custody"]
pub struct Gfx942PersistentSdmaExecutionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942PersistentSdmaExecutionCustodyV1,
}

impl Gfx942PersistentSdmaExecutionFailureV1 {
    pub fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942PersistentSdmaExecutionCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistent_allocation::{
        Gfx942PersistentOperationV1, Gfx942PersistentQuarantineReasonV1,
    };
    use crate::sdma::{
        Gfx942SdmaBufferStorageV1, persistent_sdma_buffers_for_test,
        persistent_sdma_ticket_for_test,
    };
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        VmIdV1, VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    fn queue_key() -> QueueKeyV1 {
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
            generation: QueueGenerationV1(1),
        }
    }

    fn submission_fixture(
        id: u64,
        direction: Gfx942PersistentSdmaDirectionV1,
    ) -> (
        Gfx942PersistentSdmaSubmissionV1,
        Gfx942SdmaBufferV1,
        Gfx942SdmaBufferV1,
    ) {
        let queue = queue_key();
        let (device, host) = persistent_sdma_buffers_for_test(queue, id);
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
        let raw_device = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            queue,
            pool_generation,
            logical_bytes,
        );
        let published = owner.publish(prepared).unwrap();
        let engine_index = direction.engine_index();
        (
            Gfx942PersistentSdmaSubmissionV1 {
                allocation: Gfx942QueuePersistentAllocationV1 {
                    owner,
                    attachment: Gfx942PersistentSdmaAttachmentV1 {
                        queue,
                        native_queue_id: 17,
                        engine_index,
                        pool_generation,
                        logical_bytes,
                        physical_bytes: logical_bytes,
                        storage_identity,
                    },
                },
                published,
                ticket: persistent_sdma_ticket_for_test(queue, 17),
                host_binding,
                direction,
                host_offset: 8,
                device_offset: 16,
                copy_bytes: 32,
            },
            raw_device,
            host,
        )
    }

    #[test]
    fn exact_direction_is_derived_from_the_targeted_engine() {
        assert_eq!(
            Gfx942PersistentSdmaDirectionV1::HostToDevice.engine_index(),
            crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1
        );
        assert_eq!(
            Gfx942PersistentSdmaDirectionV1::DeviceToHost.engine_index(),
            crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1
        );
    }

    #[test]
    fn adapter_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_PERSISTENT_LOCAL_SDMA_ADAPTER_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_PERSISTENT_LOCAL_SDMA_ADAPTER_MANIFEST_SHA256_V1
        );
    }

    #[test]
    fn pending_poll_retains_the_exact_move_only_submission() {
        let (submission, _device, _host) =
            submission_fixture(31, Gfx942PersistentSdmaDirectionV1::HostToDevice);
        let sequence = submission.published.sequence();
        let poll = Gfx942PersistentSdmaCopyPollV1::Pending(submission);
        let Gfx942PersistentSdmaCopyPollV1::Pending(submission) = poll else {
            panic!("fixture is pending")
        };
        assert_eq!(submission.published.sequence(), sequence);
        assert_eq!(submission.copy_bytes(), 32);
        assert_eq!(
            submission.direction(),
            Gfx942PersistentSdmaDirectionV1::HostToDevice
        );
    }

    #[test]
    fn terminal_completion_is_opaque_and_reports_exact_stage() {
        let (submission, device, host) =
            submission_fixture(32, Gfx942PersistentSdmaDirectionV1::DeviceToHost);
        let sequence = submission.published.sequence();
        let Gfx942PersistentSdmaSubmissionV1 {
            mut allocation,
            published,
            ticket: _,
            host_binding: _,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
        } = submission;
        allocation
            .owner
            .quarantine_published(
                published,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            )
            .unwrap();
        let custody = Gfx942PersistentSdmaTerminalCustodyV1 {
            direction,
            sequence: Some(sequence),
            state: Gfx942PersistentSdmaTerminalStateV1::CompletedUnrestored {
                allocation,
                completed: Gfx942SdmaCompletedCopyV1 {
                    source: device,
                    source_offset: device_offset,
                    destination: host,
                    destination_offset: host_offset,
                    copy_bytes,
                },
            },
        };
        assert_eq!(
            custody.stage(),
            Gfx942PersistentSdmaTerminalStageV1::CompletedUnrestored
        );
        assert_eq!(custody.sequence(), Some(sequence));
    }
}
