//! Executable transition model for one persistent local-device SDMA adapter.
//!
//! Admission validates one local-device mapping and builds a private instance
//! of R17's allocation-form-neutral use ledger. It binds the immutable
//! allocation identity to one logical/native queue occurrence. A transfer has
//! exactly one persistent device allocation and one ordinary host buffer.
//! Engine zero is device-to-host and reads the persistent source; engine one is
//! host-to-device and writes the persistent destination.
//!
//! This model performs no native operation and grants no KFD, HSA, HIP, Rust
//! ownership, or hardware authority. Its observations are caller-constructible
//! and non-authoritative. In particular, this is not a refinement proof between
//! these Rust transitions and a concrete KFD adapter or the Verus R18 model.

// Transition failures deliberately return move-only custody without boxing.
#![allow(clippy::result_large_err)]

use alloc::rc::Rc;
use core::marker::PhantomData;

use crate::*;

pub const R18_PERSISTENT_LOCAL_SDMA_ADAPTER_SCHEMA_VERSION_V1: u16 = 1;
pub const R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1: u32 = 0;
pub const R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1: u32 = 1;
pub const R18_SDMA_RING_SLOT_COUNT_V1: u16 = 64;
pub const R18_SDMA_MAX_LINEAR_COPY_BYTES_V1: u64 = 0x003f_ffe0;
pub const R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1: u32 = 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R18NativeAllocationKeyV1 {
    pub owner: R17PersistentAllocationOwnerIdV1,
    pub allocation: MemoryAllocationKeyV1,
    pub mapping: MemoryMappingKeyV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R18LocalPersistentAllocationAdmissionV1 {
    pub owner: R17PersistentAllocationOwnerIdV1,
    pub allocation: MemoryAllocationRecordV1,
    pub mapping: MemoryMappingRecordV1,
    pub device: DeviceKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R18LocalSdmaQueueOccurrenceV1 {
    pub logical_queue: QueueKeyV1,
    /// Zero is a valid native queue index and must not be treated as absent.
    pub native_queue_id: u32,
    pub occurrence: u64,
    pub engine_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact non-authoritative identity of one ordinary coherent host allocation.
pub struct R18HostBufferKeyV1 {
    pub session_id: u64,
    pub id: u64,
    pub generation: u64,
    pub byte_len: u64,
    pub coherence: MemoryCoherenceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18ByteRangeV1 {
    pub byte_offset: u64,
    pub byte_len: u64,
}

impl R18ByteRangeV1 {
    pub const fn checked_end(self) -> Option<u64> {
        self.byte_offset.checked_add(self.byte_len)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18LocalSdmaDirectionV1 {
    DeviceToHost,
    HostToDevice,
}

impl R18LocalSdmaDirectionV1 {
    pub const fn required_engine(self) -> u32 {
        match self {
            Self::DeviceToHost => R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1,
            Self::HostToDevice => R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1,
        }
    }

    pub const fn persistent_access(self) -> R17PersistentAccessModeV1 {
        match self {
            Self::DeviceToHost => R17PersistentAccessModeV1::Read,
            Self::HostToDevice => R17PersistentAccessModeV1::Write,
        }
    }

    pub const fn persistent_endpoint(self) -> R18PersistentEndpointV1 {
        match self {
            Self::DeviceToHost => R18PersistentEndpointV1::Source,
            Self::HostToDevice => R18PersistentEndpointV1::Destination,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18PersistentEndpointV1 {
    Source,
    Destination,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Lower-layer planned ticket observation; authority remains in move-only leases.
pub struct R18PlannedSdmaTicketV1 {
    pub owner: QueueKeyV1,
    pub queue_id: u32,
    pub slot: u16,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18PersistentLocalSdmaBindingV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub queue: R18LocalSdmaQueueOccurrenceV1,
    pub attachment_generation: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub persistent_access: R17PersistentAccessModeV1,
    pub persistent_endpoint: R18PersistentEndpointV1,
    pub device_range: R18ByteRangeV1,
    pub host: R18HostBufferKeyV1,
    pub host_range: R18ByteRangeV1,
    pub persistent_use: R17PersistentUseBindingV1,
    pub ticket: R18PlannedSdmaTicketV1,
}

impl R18PersistentLocalSdmaBindingV1 {
    pub const fn persistent_descriptor(self) -> R17PersistentUseDescriptorV1 {
        R17PersistentUseDescriptorV1 {
            class: R17PersistentUseClassV1::LocalSdma {
                device: self.allocation.allocation.vm.device,
                queue: self.queue.logical_queue,
                engine_id: self.queue.engine_id,
            },
            access: self.persistent_access,
            range: R17PersistentUseRangeV1 {
                byte_offset: self.device_range.byte_offset,
                byte_len: self.device_range.byte_len,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18PersistentLocalSdmaPhaseV1 {
    Prepared,
    Published,
    TimedOut,
    Completed,
    Restored,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18PersistentNativeLocationV1 {
    PersistentAllocation,
    PreparedBatch,
    NativeQueue,
    CompletionBatch,
    Quarantine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18SdmaTerminalStatusV1 {
    Succeeded,
    Failed { code: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18PrepublicationFailurePointV1 {
    BeforeQueueCustody,
    CompletionReset,
    RingReservation,
    PacketWrite,
    WritePointer,
    Doorbell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18QuarantineReasonV1 {
    PublicationIndeterminate(R18PrepublicationFailurePointV1),
    QueueCurrentnessAmbiguous,
    CompletionCurrentnessAmbiguous,
    RestoreCurrentnessAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18PublicationResolutionV1 {
    Confirmed,
    RecoverableFailure {
        point: R18PrepublicationFailurePointV1,
    },
    IndeterminateRetention {
        point: R18PrepublicationFailurePointV1,
    },
    CurrentnessAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18PublicationObservationV1 {
    pub binding: R18PersistentLocalSdmaBindingV1,
    pub resolution: R18PublicationResolutionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18CompletionResolutionV1 {
    Pending,
    TimedOut,
    Terminal(R18SdmaTerminalStatusV1),
    CurrentnessAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18CompletionObservationV1 {
    pub binding: R18PersistentLocalSdmaBindingV1,
    pub resolution: R18CompletionResolutionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18RestoreObservationV1 {
    pub binding: R18PersistentLocalSdmaBindingV1,
    pub status: R18SdmaTerminalStatusV1,
    pub queue_current: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18SettlementObservationV1 {
    pub binding: R18PersistentLocalSdmaBindingV1,
    pub status: R18SdmaTerminalStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R18PersistentLocalSdmaErrorV1 {
    InvalidAllocation,
    InvalidQueue,
    InvalidHostBuffer,
    InvalidRange,
    WrongDirection,
    Busy,
    WrongAdapter,
    StaleBinding,
    ObservationMismatch,
    IllegalFailureClassification,
    IllegalState,
    NotCurrent,
    Quarantined,
    CapacityExceeded,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18PersistentLocalSdmaSnapshotV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub queue: R18LocalSdmaQueueOccurrenceV1,
    pub attachment_generation: u64,
    pub active_phase: Option<R18PersistentLocalSdmaPhaseV1>,
    pub native_location: R18PersistentNativeLocationV1,
    pub current: bool,
    pub settled_transfer_count: usize,
    pub pending_frontier: Option<R18SettledFrontierKeyV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18PrepublicationRestorationReceiptV1 {
    pub binding: R18PersistentLocalSdmaBindingV1,
    pub point: R18PrepublicationFailurePointV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18SettledFrontierKeyV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub queue: R18LocalSdmaQueueOccurrenceV1,
    pub attachment_generation: u64,
    pub persistent_frontier: R17SettledFrontierKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18FrontierRetirementReceiptV1 {
    pub frontier: R18SettledFrontierKeyV1,
    pub retired_use_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18QueueRebindReceiptV1 {
    pub previous: R18LocalSdmaQueueOccurrenceV1,
    pub current: R18LocalSdmaQueueOccurrenceV1,
    pub attachment_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R18LocalAllocationReleaseReceiptV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub device: DeviceKeyV1,
    pub completed_lease_count: usize,
    pub settled_transfer_count: usize,
}

struct R18ActiveTransferRecordV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    phase: R18PersistentLocalSdmaPhaseV1,
    location: R18PersistentNativeLocationV1,
    terminal_status: Option<R18SdmaTerminalStatusV1>,
    quarantine_reason: Option<R18QuarantineReasonV1>,
    persistent_lease: Option<R18UnderlyingPersistentLeaseV1>,
}

struct R18PendingFrontierRecordV1 {
    key: R18SettledFrontierKeyV1,
    persistent_frontier: R17SettledPersistentFrontierV1,
}

#[derive(Debug)]
enum R18UnderlyingPersistentLeaseV1 {
    Reserved(R17ReservedPersistentUseLeaseV1),
    Published(R17PublishedPersistentUseLeaseV1),
    TimedOut(R17TimedOutPersistentUseLeaseV1),
    Terminal(R17TerminalPersistentUseLeaseV1),
    Quarantined(R17QuarantinedPersistentUseLeaseV1),
}

impl R18UnderlyingPersistentLeaseV1 {
    const fn binding(&self) -> R17PersistentUseBindingV1 {
        match self {
            Self::Reserved(lease) => lease.binding(),
            Self::Published(lease) => lease.binding(),
            Self::TimedOut(lease) => lease.binding(),
            Self::Terminal(lease) => lease.binding(),
            Self::Quarantined(lease) => lease.binding(),
        }
    }
}

/// Sole model owner for an R17 registry attached to one local SDMA queue.
///
/// The `Rc` incarnation makes state-changing leases specific to this exact
/// adapter reconstruction. Numeric bindings and observations can coincide and
/// are never sufficient transition authority.
pub struct R18PersistentLocalSdmaAdapterV1 {
    registry: R17PersistentNativeAllocationRegistryV1,
    allocation: R18NativeAllocationKeyV1,
    queue: R18LocalSdmaQueueOccurrenceV1,
    attachment_generation: u64,
    settled_transfer_count: usize,
    current: bool,
    active: Option<R18ActiveTransferRecordV1>,
    pending_frontier: Option<R18PendingFrontierRecordV1>,
    registry_incarnation: Rc<()>,
    thread_affine: PhantomData<Rc<()>>,
}

impl R18PersistentLocalSdmaAdapterV1 {
    pub fn new_local_model_only(
        admission: R18LocalPersistentAllocationAdmissionV1,
        queue: R18LocalSdmaQueueOccurrenceV1,
    ) -> Result<Self, R18AdapterAdmissionFailureV1> {
        let allocation = R18NativeAllocationKeyV1 {
            owner: admission.owner,
            allocation: admission.allocation.key,
            mapping: admission.mapping.key,
        };
        if !valid_queue_for_allocation_v1(queue, allocation) {
            return Err(R18AdapterAdmissionFailureV1 {
                error: R18PersistentLocalSdmaErrorV1::InvalidQueue,
                retained: admission,
            });
        }
        let registry = match R17PersistentNativeAllocationRegistryV1::new_local_model_only(
            admission.owner,
            admission.allocation,
            admission.mapping.clone(),
            admission.device,
        ) {
            Ok(registry) => registry,
            Err(_) => {
                return Err(R18AdapterAdmissionFailureV1 {
                    error: R18PersistentLocalSdmaErrorV1::InvalidAllocation,
                    retained: admission,
                });
            }
        };
        let adapter = Self {
            registry,
            allocation,
            queue,
            attachment_generation: 1,
            settled_transfer_count: 0,
            current: true,
            active: None,
            pending_frontier: None,
            registry_incarnation: Rc::new(()),
            thread_affine: PhantomData,
        };
        if let Err(error) = adapter.validate_global_invariants() {
            return Err(R18AdapterAdmissionFailureV1 {
                error,
                retained: admission,
            });
        }
        Ok(adapter)
    }

    pub const fn allocation(&self) -> R18NativeAllocationKeyV1 {
        self.allocation
    }

    pub const fn queue(&self) -> R18LocalSdmaQueueOccurrenceV1 {
        self.queue
    }

    pub fn snapshot(&self) -> R18PersistentLocalSdmaSnapshotV1 {
        R18PersistentLocalSdmaSnapshotV1 {
            allocation: self.allocation,
            queue: self.queue,
            attachment_generation: self.attachment_generation,
            active_phase: self.active.as_ref().map(|record| record.phase),
            native_location: self.active.as_ref().map_or(
                R18PersistentNativeLocationV1::PersistentAllocation,
                |record| record.location,
            ),
            current: self.current,
            settled_transfer_count: self.settled_transfer_count,
            pending_frontier: self.pending_frontier.as_ref().map(|frontier| frontier.key),
        }
    }

    /// Non-authoritative view of the exact composed R17 use record.
    pub fn active_persistent_use_record(&self) -> Option<R17PersistentUseRecordObservationV1> {
        let lease = self
            .active
            .as_ref()
            .map(|record| record.binding.persistent_use.lease)
            .or_else(|| {
                self.pending_frontier
                    .as_ref()
                    .map(|frontier| frontier.key.persistent_frontier.through_use)
            })?;
        self.registry.record(lease)
    }

    pub fn prepare_model_only(
        &mut self,
        direction: R18LocalSdmaDirectionV1,
        device_range: R18ByteRangeV1,
        host: R18HostBufferKeyV1,
        host_range: R18ByteRangeV1,
        ticket: R18PlannedSdmaTicketV1,
    ) -> Result<R18PreparedPersistentLocalSdmaLeaseV1, R18PersistentLocalSdmaErrorV1> {
        if !self.current {
            return Err(R18PersistentLocalSdmaErrorV1::NotCurrent);
        }
        if self.active.is_some() || self.pending_frontier.is_some() {
            return Err(R18PersistentLocalSdmaErrorV1::Busy);
        }
        if direction.required_engine() != self.queue.engine_id {
            return Err(R18PersistentLocalSdmaErrorV1::WrongDirection);
        }
        if host.session_id == 0
            || host.id == 0
            || host.generation == 0
            || host.byte_len == 0
            || host.coherence != MemoryCoherenceV1::HostCoherent
        {
            return Err(R18PersistentLocalSdmaErrorV1::InvalidHostBuffer);
        }
        validate_equal_ranges_v1(
            device_range,
            self.registry.byte_len(),
            host_range,
            host.byte_len,
        )?;
        if ticket.owner != self.queue.logical_queue
            || ticket.queue_id != self.queue.native_queue_id
            || ticket.slot >= R18_SDMA_RING_SLOT_COUNT_V1
            || ticket.generation == 0
        {
            return Err(R18PersistentLocalSdmaErrorV1::StaleBinding);
        }
        let descriptor = R17PersistentUseDescriptorV1 {
            class: R17PersistentUseClassV1::LocalSdma {
                device: self.allocation.allocation.vm.device,
                queue: self.queue.logical_queue,
                engine_id: self.queue.engine_id,
            },
            access: direction.persistent_access(),
            range: R17PersistentUseRangeV1 {
                byte_offset: device_range.byte_offset,
                byte_len: device_range.byte_len,
            },
        };
        let persistent_lease = self
            .registry
            .reserve_model_only(descriptor, alloc::vec![])
            .map_err(|_| R18PersistentLocalSdmaErrorV1::InvariantViolation)?;
        let persistent_use = persistent_lease.binding();
        let binding = R18PersistentLocalSdmaBindingV1 {
            allocation: self.allocation,
            queue: self.queue,
            attachment_generation: self.attachment_generation,
            direction,
            persistent_access: direction.persistent_access(),
            persistent_endpoint: direction.persistent_endpoint(),
            device_range,
            host,
            host_range,
            persistent_use,
            ticket,
        };
        self.active = Some(R18ActiveTransferRecordV1 {
            binding,
            phase: R18PersistentLocalSdmaPhaseV1::Prepared,
            location: R18PersistentNativeLocationV1::PreparedBatch,
            terminal_status: None,
            quarantine_reason: None,
            persistent_lease: Some(R18UnderlyingPersistentLeaseV1::Reserved(persistent_lease)),
        });
        self.validate_global_invariants()?;
        Ok(R18PreparedPersistentLocalSdmaLeaseV1 {
            binding,
            registry_incarnation: Rc::clone(&self.registry_incarnation),
        })
    }

    pub fn rebind_queue_model_only(
        &mut self,
        queue: R18LocalSdmaQueueOccurrenceV1,
    ) -> Result<R18QueueRebindReceiptV1, R18PersistentLocalSdmaErrorV1> {
        if !self.current {
            return Err(R18PersistentLocalSdmaErrorV1::NotCurrent);
        }
        if self.active.is_some() || self.pending_frontier.is_some() {
            return Err(R18PersistentLocalSdmaErrorV1::Busy);
        }
        if !valid_queue_for_allocation_v1(queue, self.allocation) {
            return Err(R18PersistentLocalSdmaErrorV1::InvalidQueue);
        }
        let attachment_generation = self
            .attachment_generation
            .checked_add(1)
            .ok_or(R18PersistentLocalSdmaErrorV1::CapacityExceeded)?;
        let previous = self.queue;
        self.queue = queue;
        self.attachment_generation = attachment_generation;
        self.validate_global_invariants()?;
        Ok(R18QueueRebindReceiptV1 {
            previous,
            current: queue,
            attachment_generation,
        })
    }

    pub fn release_model_only(
        self,
    ) -> Result<R18LocalAllocationReleaseReceiptV1, R18AdapterReleaseFailureV1> {
        let error = if !self.current {
            Some(R18PersistentLocalSdmaErrorV1::Quarantined)
        } else if self.active.is_some() || self.pending_frontier.is_some() {
            Some(R18PersistentLocalSdmaErrorV1::Busy)
        } else {
            self.validate_global_invariants().err()
        };
        if let Some(error) = error {
            return Err(R18AdapterReleaseFailureV1 {
                error,
                retained: self,
            });
        }
        let allocation = self.allocation;
        let device = allocation.allocation.vm.device;
        let settled_transfer_count = self.settled_transfer_count;
        let release = match self.registry.release_allocation_model_only() {
            Ok(release) => release,
            Err(_) => unreachable!("validated idle R17 ledger must release"),
        };
        Ok(R18LocalAllocationReleaseReceiptV1 {
            allocation,
            device,
            completed_lease_count: release.completed_lease_count,
            settled_transfer_count,
        })
    }

    pub fn validate_global_invariants(&self) -> Result<(), R18PersistentLocalSdmaErrorV1> {
        if self.allocation.owner != self.registry.owner()
            || self.allocation.allocation != self.registry.allocation()
            || self.allocation.mapping != self.registry.mapping()
            || self.allocation.mapping.allocation != self.allocation.allocation
            || self.attachment_generation == 0
            || self.current != self.registry.is_current()
            || !valid_queue_for_allocation_v1(self.queue, self.allocation)
        {
            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
        }
        if self.active.is_some() && self.pending_frontier.is_some() {
            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
        }
        if let Some(record) = &self.active {
            self.validate_binding(record.binding)?;
            let exact_location = match record.phase {
                R18PersistentLocalSdmaPhaseV1::Prepared => {
                    R18PersistentNativeLocationV1::PreparedBatch
                }
                R18PersistentLocalSdmaPhaseV1::Published
                | R18PersistentLocalSdmaPhaseV1::TimedOut => {
                    R18PersistentNativeLocationV1::NativeQueue
                }
                R18PersistentLocalSdmaPhaseV1::Completed => {
                    R18PersistentNativeLocationV1::CompletionBatch
                }
                R18PersistentLocalSdmaPhaseV1::Restored => {
                    R18PersistentNativeLocationV1::PersistentAllocation
                }
                R18PersistentLocalSdmaPhaseV1::Quarantined => {
                    R18PersistentNativeLocationV1::Quarantine
                }
            };
            let Some(persistent_lease) = record.persistent_lease.as_ref() else {
                return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
            };
            let expected_r17_phase = match record.phase {
                R18PersistentLocalSdmaPhaseV1::Prepared => R17PersistentUsePhaseV1::Reserved,
                R18PersistentLocalSdmaPhaseV1::Published => R17PersistentUsePhaseV1::Published,
                R18PersistentLocalSdmaPhaseV1::TimedOut => R17PersistentUsePhaseV1::TimedOut,
                R18PersistentLocalSdmaPhaseV1::Completed
                | R18PersistentLocalSdmaPhaseV1::Restored => R17PersistentUsePhaseV1::Terminal,
                R18PersistentLocalSdmaPhaseV1::Quarantined => R17PersistentUsePhaseV1::Quarantined,
            };
            let lease_r17_phase = match persistent_lease {
                R18UnderlyingPersistentLeaseV1::Reserved(_) => R17PersistentUsePhaseV1::Reserved,
                R18UnderlyingPersistentLeaseV1::Published(_) => R17PersistentUsePhaseV1::Published,
                R18UnderlyingPersistentLeaseV1::TimedOut(_) => R17PersistentUsePhaseV1::TimedOut,
                R18UnderlyingPersistentLeaseV1::Terminal(_) => R17PersistentUsePhaseV1::Terminal,
                R18UnderlyingPersistentLeaseV1::Quarantined(_) => {
                    R17PersistentUsePhaseV1::Quarantined
                }
            };
            if record.location != exact_location
                || lease_r17_phase != expected_r17_phase
                || persistent_lease.binding() != record.binding.persistent_use
                || self
                    .registry
                    .record(record.binding.persistent_use.lease)
                    .is_none_or(|observation| {
                        observation.binding != record.binding.persistent_use
                            || observation.phase != expected_r17_phase
                    })
            {
                return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
            }
            let terminal_valid = matches!(
                record.phase,
                R18PersistentLocalSdmaPhaseV1::Completed | R18PersistentLocalSdmaPhaseV1::Restored
            ) == record.terminal_status.is_some();
            let quarantine_valid = (record.phase == R18PersistentLocalSdmaPhaseV1::Quarantined)
                == record.quarantine_reason.is_some();
            if !terminal_valid || !quarantine_valid {
                return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
            }
        }
        if let Some(frontier) = &self.pending_frontier {
            let snapshot = self.registry.snapshot();
            if frontier.key.allocation != self.allocation
                || frontier.key.queue != self.queue
                || frontier.key.attachment_generation != self.attachment_generation
                || frontier.persistent_frontier.key() != frontier.key.persistent_frontier
                || snapshot.lease_count != 1
                || snapshot.settled_count != 1
                || snapshot.frontier_generation != frontier.key.persistent_frontier.generation
                || snapshot.frontier_use != Some(frontier.key.persistent_frontier.through_use)
            {
                return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
            }
        }
        let expected_lease_count =
            usize::from(self.active.is_some() || self.pending_frontier.is_some());
        if self.registry.snapshot().lease_count != expected_lease_count {
            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
        }
        if self.current
            == self
                .active
                .as_ref()
                .is_some_and(|record| record.phase == R18PersistentLocalSdmaPhaseV1::Quarantined)
        {
            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn validate_binding(
        &self,
        binding: R18PersistentLocalSdmaBindingV1,
    ) -> Result<(), R18PersistentLocalSdmaErrorV1> {
        if binding.allocation != self.allocation
            || binding.queue != self.queue
            || binding.attachment_generation != self.attachment_generation
            || binding.ticket.owner != binding.queue.logical_queue
            || binding.ticket.queue_id != binding.queue.native_queue_id
            || binding.ticket.slot >= R18_SDMA_RING_SLOT_COUNT_V1
            || binding.ticket.generation == 0
            || binding.persistent_use.allocation != binding.allocation.allocation
            || binding.persistent_use.mapping != binding.allocation.mapping
            || binding.persistent_use.lease.owner != binding.allocation.owner
        {
            return Err(R18PersistentLocalSdmaErrorV1::StaleBinding);
        }
        if binding.direction.required_engine() != binding.queue.engine_id
            || binding.persistent_access != binding.direction.persistent_access()
            || binding.persistent_endpoint != binding.direction.persistent_endpoint()
        {
            return Err(R18PersistentLocalSdmaErrorV1::WrongDirection);
        }
        if binding.persistent_use.descriptor != binding.persistent_descriptor() {
            return Err(R18PersistentLocalSdmaErrorV1::StaleBinding);
        }
        if binding.host.session_id == 0
            || binding.host.id == 0
            || binding.host.generation == 0
            || binding.host.byte_len == 0
            || binding.host.coherence != MemoryCoherenceV1::HostCoherent
        {
            return Err(R18PersistentLocalSdmaErrorV1::InvalidHostBuffer);
        }
        validate_equal_ranges_v1(
            binding.device_range,
            self.registry.byte_len(),
            binding.host_range,
            binding.host.byte_len,
        )
    }

    fn require_active(
        &self,
        binding: R18PersistentLocalSdmaBindingV1,
        incarnation: &Rc<()>,
        phases: &[R18PersistentLocalSdmaPhaseV1],
    ) -> Result<(), R18PersistentLocalSdmaErrorV1> {
        if !Rc::ptr_eq(incarnation, &self.registry_incarnation) {
            return Err(R18PersistentLocalSdmaErrorV1::WrongAdapter);
        }
        self.validate_binding(binding)?;
        let record = self
            .active
            .as_ref()
            .ok_or(R18PersistentLocalSdmaErrorV1::StaleBinding)?;
        if record.binding != binding {
            return Err(R18PersistentLocalSdmaErrorV1::StaleBinding);
        }
        if !phases.contains(&record.phase) {
            return Err(R18PersistentLocalSdmaErrorV1::IllegalState);
        }
        Ok(())
    }

    fn set_phase(
        &mut self,
        phase: R18PersistentLocalSdmaPhaseV1,
        location: R18PersistentNativeLocationV1,
        terminal_status: Option<R18SdmaTerminalStatusV1>,
        quarantine_reason: Option<R18QuarantineReasonV1>,
    ) {
        let record = self.active.as_mut().expect("validated active transfer");
        record.phase = phase;
        record.location = location;
        record.terminal_status = terminal_status;
        record.quarantine_reason = quarantine_reason;
        if phase == R18PersistentLocalSdmaPhaseV1::Quarantined {
            self.current = false;
        }
    }

    fn take_persistent_lease(
        &mut self,
    ) -> Result<R18UnderlyingPersistentLeaseV1, R18PersistentLocalSdmaErrorV1> {
        self.active
            .as_mut()
            .and_then(|record| record.persistent_lease.take())
            .ok_or(R18PersistentLocalSdmaErrorV1::InvariantViolation)
    }

    fn restore_persistent_lease(&mut self, lease: R18UnderlyingPersistentLeaseV1) {
        self.active
            .as_mut()
            .expect("validated active transfer")
            .persistent_lease = Some(lease);
    }
}

fn valid_queue_for_allocation_v1(
    queue: R18LocalSdmaQueueOccurrenceV1,
    allocation: R18NativeAllocationKeyV1,
) -> bool {
    queue.logical_queue.vm == allocation.allocation.vm
        && queue.logical_queue.id.0 != 0
        && queue.logical_queue.generation.0 != 0
        && queue.native_queue_id < R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1
        && queue.occurrence != 0
        && matches!(
            queue.engine_id,
            R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1 | R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1
        )
}

fn validate_equal_ranges_v1(
    device: R18ByteRangeV1,
    device_len: u64,
    host: R18ByteRangeV1,
    host_len: u64,
) -> Result<(), R18PersistentLocalSdmaErrorV1> {
    let Some(device_end) = device.checked_end() else {
        return Err(R18PersistentLocalSdmaErrorV1::InvalidRange);
    };
    let Some(host_end) = host.checked_end() else {
        return Err(R18PersistentLocalSdmaErrorV1::InvalidRange);
    };
    if device.byte_len == 0
        || device.byte_len > R18_SDMA_MAX_LINEAR_COPY_BYTES_V1
        || device.byte_len != host.byte_len
        || device_end > device_len
        || host_end > host_len
    {
        return Err(R18PersistentLocalSdmaErrorV1::InvalidRange);
    }
    Ok(())
}

fn exact_observation_v1(
    expected: R18PersistentLocalSdmaBindingV1,
    observed: R18PersistentLocalSdmaBindingV1,
) -> Result<(), R18PersistentLocalSdmaErrorV1> {
    if expected == observed {
        Ok(())
    } else {
        Err(R18PersistentLocalSdmaErrorV1::ObservationMismatch)
    }
}

const fn r17_status_v1(status: R18SdmaTerminalStatusV1) -> R17PersistentTerminalStatusV1 {
    match status {
        R18SdmaTerminalStatusV1::Succeeded => R17PersistentTerminalStatusV1::Succeeded,
        R18SdmaTerminalStatusV1::Failed { code } => R17PersistentTerminalStatusV1::Failed { code },
    }
}

#[derive(Debug)]
pub struct R18AdapterTransitionFailureV1<T> {
    error: R18PersistentLocalSdmaErrorV1,
    retained: T,
}

impl<T> R18AdapterTransitionFailureV1<T> {
    pub const fn error(&self) -> R18PersistentLocalSdmaErrorV1 {
        self.error
    }

    pub fn into_parts(self) -> (R18PersistentLocalSdmaErrorV1, T) {
        (self.error, self.retained)
    }
}

#[derive(Debug)]
#[must_use = "prepared SDMA custody must be resolved"]
pub struct R18PreparedPersistentLocalSdmaLeaseV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    registry_incarnation: Rc<()>,
}

impl R18PreparedPersistentLocalSdmaLeaseV1 {
    pub const fn binding(&self) -> R18PersistentLocalSdmaBindingV1 {
        self.binding
    }

    pub fn resolve_publication_model_only(
        self,
        adapter: &mut R18PersistentLocalSdmaAdapterV1,
        observation: R18PublicationObservationV1,
    ) -> Result<R18PublicationOutcomeV1, R18AdapterTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_active(
                self.binding,
                &self.registry_incarnation,
                &[R18PersistentLocalSdmaPhaseV1::Prepared],
            )?;
            exact_observation_v1(self.binding, observation.binding)?;
            match observation.resolution {
                R18PublicationResolutionV1::Confirmed => {
                    let R18UnderlyingPersistentLeaseV1::Reserved(lease) =
                        adapter.take_persistent_lease()?
                    else {
                        return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                    };
                    let lease = match lease.publish_model_only(&mut adapter.registry) {
                        Ok(lease) => lease,
                        Err(failure) => {
                            adapter.restore_persistent_lease(
                                R18UnderlyingPersistentLeaseV1::Reserved(failure.into_parts().1),
                            );
                            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                        }
                    };
                    adapter
                        .restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Published(lease));
                    adapter.set_phase(
                        R18PersistentLocalSdmaPhaseV1::Published,
                        R18PersistentNativeLocationV1::NativeQueue,
                        None,
                        None,
                    );
                    Ok(R18PublicationOutcomeV1::Published(
                        R18PublishedPersistentLocalSdmaLeaseV1 {
                            binding: self.binding,
                            registry_incarnation: Rc::clone(&self.registry_incarnation),
                        },
                    ))
                }
                R18PublicationResolutionV1::RecoverableFailure { point } => {
                    if point != R18PrepublicationFailurePointV1::BeforeQueueCustody {
                        return Err(R18PersistentLocalSdmaErrorV1::IllegalFailureClassification);
                    }
                    let R18UnderlyingPersistentLeaseV1::Reserved(lease) =
                        adapter.take_persistent_lease()?
                    else {
                        return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                    };
                    if let Err(failure) =
                        lease.cancel_before_publication_model_only(&mut adapter.registry)
                    {
                        adapter.restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Reserved(
                            failure.into_parts().1,
                        ));
                        return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                    }
                    adapter.active = None;
                    Ok(R18PublicationOutcomeV1::Restored(
                        R18PrepublicationRestorationReceiptV1 {
                            binding: self.binding,
                            point,
                        },
                    ))
                }
                R18PublicationResolutionV1::IndeterminateRetention { point } => {
                    if point == R18PrepublicationFailurePointV1::BeforeQueueCustody {
                        return Err(R18PersistentLocalSdmaErrorV1::IllegalFailureClassification);
                    }
                    let R18UnderlyingPersistentLeaseV1::Reserved(lease) =
                        adapter.take_persistent_lease()?
                    else {
                        return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                    };
                    let lease = match lease
                        .quarantine_indeterminate_prepublication_model_only(&mut adapter.registry)
                    {
                        Ok(lease) => lease,
                        Err(failure) => {
                            adapter.restore_persistent_lease(
                                R18UnderlyingPersistentLeaseV1::Reserved(failure.into_parts().1),
                            );
                            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                        }
                    };
                    adapter.restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Quarantined(
                        lease,
                    ));
                    let reason = R18QuarantineReasonV1::PublicationIndeterminate(point);
                    adapter.set_phase(
                        R18PersistentLocalSdmaPhaseV1::Quarantined,
                        R18PersistentNativeLocationV1::Quarantine,
                        None,
                        Some(reason),
                    );
                    Ok(R18PublicationOutcomeV1::Quarantined(
                        R18QuarantinedPersistentLocalSdmaLeaseV1 {
                            binding: self.binding,
                            reason,
                            _registry_incarnation: Rc::clone(&self.registry_incarnation),
                        },
                    ))
                }
                R18PublicationResolutionV1::CurrentnessAmbiguous => {
                    let R18UnderlyingPersistentLeaseV1::Reserved(lease) =
                        adapter.take_persistent_lease()?
                    else {
                        return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                    };
                    let lease = match lease
                        .quarantine_indeterminate_prepublication_model_only(&mut adapter.registry)
                    {
                        Ok(lease) => lease,
                        Err(failure) => {
                            adapter.restore_persistent_lease(
                                R18UnderlyingPersistentLeaseV1::Reserved(failure.into_parts().1),
                            );
                            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                        }
                    };
                    adapter.restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Quarantined(
                        lease,
                    ));
                    let reason = R18QuarantineReasonV1::QueueCurrentnessAmbiguous;
                    adapter.set_phase(
                        R18PersistentLocalSdmaPhaseV1::Quarantined,
                        R18PersistentNativeLocationV1::Quarantine,
                        None,
                        Some(reason),
                    );
                    Ok(R18PublicationOutcomeV1::Quarantined(
                        R18QuarantinedPersistentLocalSdmaLeaseV1 {
                            binding: self.binding,
                            reason,
                            _registry_incarnation: Rc::clone(&self.registry_incarnation),
                        },
                    ))
                }
            }
        })();
        match transition {
            Ok(outcome) => {
                if let Err(error) = adapter.validate_global_invariants() {
                    Err(R18AdapterTransitionFailureV1 {
                        error,
                        retained: self,
                    })
                } else {
                    Ok(outcome)
                }
            }
            Err(error) => Err(R18AdapterTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
pub enum R18PublicationOutcomeV1 {
    Restored(R18PrepublicationRestorationReceiptV1),
    Published(R18PublishedPersistentLocalSdmaLeaseV1),
    Quarantined(R18QuarantinedPersistentLocalSdmaLeaseV1),
}

#[derive(Debug)]
#[must_use = "published SDMA custody must reach a terminal or quarantine state"]
pub struct R18PublishedPersistentLocalSdmaLeaseV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    registry_incarnation: Rc<()>,
}

impl R18PublishedPersistentLocalSdmaLeaseV1 {
    pub const fn binding(&self) -> R18PersistentLocalSdmaBindingV1 {
        self.binding
    }

    pub fn observe_model_only(
        self,
        adapter: &mut R18PersistentLocalSdmaAdapterV1,
        observation: R18CompletionObservationV1,
    ) -> Result<R18PublishedPollV1, R18AdapterTransitionFailureV1<Self>> {
        match observe_completion_v1(
            adapter,
            self.binding,
            &self.registry_incarnation,
            observation,
            false,
        ) {
            Ok(R18ObservedCompletionV1::Published) => Ok(R18PublishedPollV1::Pending(self)),
            Ok(R18ObservedCompletionV1::TimedOut) => Ok(R18PublishedPollV1::TimedOut(
                R18TimedOutPersistentLocalSdmaLeaseV1 {
                    binding: self.binding,
                    registry_incarnation: self.registry_incarnation,
                },
            )),
            Ok(R18ObservedCompletionV1::Completed(status)) => Ok(R18PublishedPollV1::Completed(
                R18CompletedPersistentLocalSdmaLeaseV1 {
                    binding: self.binding,
                    status,
                    registry_incarnation: self.registry_incarnation,
                },
            )),
            Ok(R18ObservedCompletionV1::Quarantined(reason)) => Ok(
                R18PublishedPollV1::Quarantined(R18QuarantinedPersistentLocalSdmaLeaseV1 {
                    binding: self.binding,
                    reason,
                    _registry_incarnation: self.registry_incarnation,
                }),
            ),
            Err(error) => Err(R18AdapterTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
pub enum R18PublishedPollV1 {
    Pending(R18PublishedPersistentLocalSdmaLeaseV1),
    TimedOut(R18TimedOutPersistentLocalSdmaLeaseV1),
    Completed(R18CompletedPersistentLocalSdmaLeaseV1),
    Quarantined(R18QuarantinedPersistentLocalSdmaLeaseV1),
}

#[derive(Debug)]
#[must_use = "timed-out SDMA custody remains in the native queue"]
pub struct R18TimedOutPersistentLocalSdmaLeaseV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    registry_incarnation: Rc<()>,
}

impl R18TimedOutPersistentLocalSdmaLeaseV1 {
    pub const fn binding(&self) -> R18PersistentLocalSdmaBindingV1 {
        self.binding
    }

    pub fn observe_model_only(
        self,
        adapter: &mut R18PersistentLocalSdmaAdapterV1,
        observation: R18CompletionObservationV1,
    ) -> Result<R18TimedOutPollV1, R18AdapterTransitionFailureV1<Self>> {
        match observe_completion_v1(
            adapter,
            self.binding,
            &self.registry_incarnation,
            observation,
            true,
        ) {
            Ok(R18ObservedCompletionV1::Published | R18ObservedCompletionV1::TimedOut) => {
                Ok(R18TimedOutPollV1::TimedOut(self))
            }
            Ok(R18ObservedCompletionV1::Completed(status)) => Ok(R18TimedOutPollV1::Completed(
                R18CompletedPersistentLocalSdmaLeaseV1 {
                    binding: self.binding,
                    status,
                    registry_incarnation: self.registry_incarnation,
                },
            )),
            Ok(R18ObservedCompletionV1::Quarantined(reason)) => Ok(R18TimedOutPollV1::Quarantined(
                R18QuarantinedPersistentLocalSdmaLeaseV1 {
                    binding: self.binding,
                    reason,
                    _registry_incarnation: self.registry_incarnation,
                },
            )),
            Err(error) => Err(R18AdapterTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
pub enum R18TimedOutPollV1 {
    TimedOut(R18TimedOutPersistentLocalSdmaLeaseV1),
    Completed(R18CompletedPersistentLocalSdmaLeaseV1),
    Quarantined(R18QuarantinedPersistentLocalSdmaLeaseV1),
}

fn observe_completion_v1(
    adapter: &mut R18PersistentLocalSdmaAdapterV1,
    binding: R18PersistentLocalSdmaBindingV1,
    incarnation: &Rc<()>,
    observation: R18CompletionObservationV1,
    already_timed_out: bool,
) -> Result<R18ObservedCompletionV1, R18PersistentLocalSdmaErrorV1> {
    let expected_phase = if already_timed_out {
        R18PersistentLocalSdmaPhaseV1::TimedOut
    } else {
        R18PersistentLocalSdmaPhaseV1::Published
    };
    adapter.require_active(binding, incarnation, &[expected_phase])?;
    exact_observation_v1(binding, observation.binding)?;
    let r17_observation = match observation.resolution {
        R18CompletionResolutionV1::Pending => R17PersistentUseObservationV1::Pending,
        R18CompletionResolutionV1::TimedOut => R17PersistentUseObservationV1::TimedOut,
        R18CompletionResolutionV1::Terminal(status) => {
            R17PersistentUseObservationV1::Terminal(r17_status_v1(status))
        }
        R18CompletionResolutionV1::CurrentnessAmbiguous => {
            R17PersistentUseObservationV1::Indeterminate(
                R17PersistentQuarantineReasonV1::CompletionObservationUnavailable,
            )
        }
    };
    let persistent_lease = adapter.take_persistent_lease()?;
    let persistent_lease = match (persistent_lease, already_timed_out) {
        (R18UnderlyingPersistentLeaseV1::Published(lease), false) => {
            match lease.observe_model_only(&mut adapter.registry, r17_observation) {
                Ok(R17PersistentUsePollV1::Published(lease)) => {
                    R18UnderlyingPersistentLeaseV1::Published(lease)
                }
                Ok(R17PersistentUsePollV1::TimedOut(lease)) => {
                    R18UnderlyingPersistentLeaseV1::TimedOut(lease)
                }
                Ok(R17PersistentUsePollV1::Terminal(lease)) => {
                    R18UnderlyingPersistentLeaseV1::Terminal(lease)
                }
                Ok(R17PersistentUsePollV1::Quarantined(lease)) => {
                    R18UnderlyingPersistentLeaseV1::Quarantined(lease)
                }
                Err(failure) => {
                    adapter.restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Published(
                        failure.into_parts().1,
                    ));
                    return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                }
            }
        }
        (R18UnderlyingPersistentLeaseV1::TimedOut(lease), true) => {
            match lease.observe_model_only(&mut adapter.registry, r17_observation) {
                Ok(R17TimedOutUsePollV1::TimedOut(lease)) => {
                    R18UnderlyingPersistentLeaseV1::TimedOut(lease)
                }
                Ok(R17TimedOutUsePollV1::Terminal(lease)) => {
                    R18UnderlyingPersistentLeaseV1::Terminal(lease)
                }
                Ok(R17TimedOutUsePollV1::Quarantined(lease)) => {
                    R18UnderlyingPersistentLeaseV1::Quarantined(lease)
                }
                Err(failure) => {
                    adapter.restore_persistent_lease(R18UnderlyingPersistentLeaseV1::TimedOut(
                        failure.into_parts().1,
                    ));
                    return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                }
            }
        }
        (lease, _) => {
            adapter.restore_persistent_lease(lease);
            return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
        }
    };
    adapter.restore_persistent_lease(persistent_lease);
    let observed = match observation.resolution {
        R18CompletionResolutionV1::Pending if already_timed_out => {
            R18ObservedCompletionV1::TimedOut
        }
        R18CompletionResolutionV1::Pending => R18ObservedCompletionV1::Published,
        R18CompletionResolutionV1::TimedOut => R18ObservedCompletionV1::TimedOut,
        R18CompletionResolutionV1::Terminal(status) => R18ObservedCompletionV1::Completed(status),
        R18CompletionResolutionV1::CurrentnessAmbiguous => R18ObservedCompletionV1::Quarantined(
            R18QuarantineReasonV1::CompletionCurrentnessAmbiguous,
        ),
    };
    match observed {
        R18ObservedCompletionV1::Published => adapter.set_phase(
            R18PersistentLocalSdmaPhaseV1::Published,
            R18PersistentNativeLocationV1::NativeQueue,
            None,
            None,
        ),
        R18ObservedCompletionV1::TimedOut => adapter.set_phase(
            R18PersistentLocalSdmaPhaseV1::TimedOut,
            R18PersistentNativeLocationV1::NativeQueue,
            None,
            None,
        ),
        R18ObservedCompletionV1::Completed(status) => adapter.set_phase(
            R18PersistentLocalSdmaPhaseV1::Completed,
            R18PersistentNativeLocationV1::CompletionBatch,
            Some(status),
            None,
        ),
        R18ObservedCompletionV1::Quarantined(reason) => adapter.set_phase(
            R18PersistentLocalSdmaPhaseV1::Quarantined,
            R18PersistentNativeLocationV1::Quarantine,
            None,
            Some(reason),
        ),
    };
    adapter.validate_global_invariants()?;
    Ok(observed)
}

#[derive(Clone, Copy)]
enum R18ObservedCompletionV1 {
    Published,
    TimedOut,
    Completed(R18SdmaTerminalStatusV1),
    Quarantined(R18QuarantineReasonV1),
}

#[derive(Debug)]
#[must_use = "completed SDMA custody must be restored or quarantined"]
pub struct R18CompletedPersistentLocalSdmaLeaseV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    status: R18SdmaTerminalStatusV1,
    registry_incarnation: Rc<()>,
}

impl R18CompletedPersistentLocalSdmaLeaseV1 {
    pub const fn binding(&self) -> R18PersistentLocalSdmaBindingV1 {
        self.binding
    }

    pub const fn status(&self) -> R18SdmaTerminalStatusV1 {
        self.status
    }

    pub fn restore_model_only(
        self,
        adapter: &mut R18PersistentLocalSdmaAdapterV1,
        observation: R18RestoreObservationV1,
    ) -> Result<R18RestoreOutcomeV1, R18AdapterTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_active(
                self.binding,
                &self.registry_incarnation,
                &[R18PersistentLocalSdmaPhaseV1::Completed],
            )?;
            exact_observation_v1(self.binding, observation.binding)?;
            if observation.status != self.status {
                return Err(R18PersistentLocalSdmaErrorV1::ObservationMismatch);
            }
            if observation.queue_current {
                adapter.set_phase(
                    R18PersistentLocalSdmaPhaseV1::Restored,
                    R18PersistentNativeLocationV1::PersistentAllocation,
                    Some(self.status),
                    None,
                );
                Ok(R18RestoreOutcomeV1::Restored(
                    R18RestoredPersistentLocalSdmaLeaseV1 {
                        binding: self.binding,
                        status: self.status,
                        registry_incarnation: Rc::clone(&self.registry_incarnation),
                    },
                ))
            } else {
                let R18UnderlyingPersistentLeaseV1::Terminal(lease) =
                    adapter.take_persistent_lease()?
                else {
                    return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                };
                adapter
                    .registry
                    .lose_currentness_model_only(
                        R17PersistentQuarantineReasonV1::DeviceCurrentnessLost,
                    )
                    .map_err(|_| R18PersistentLocalSdmaErrorV1::InvariantViolation)?;
                let lease = lease
                    .reconcile_after_currentness_loss_model_only(&adapter.registry)
                    .map_err(|_| R18PersistentLocalSdmaErrorV1::InvariantViolation)?;
                adapter
                    .restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Quarantined(lease));
                let reason = R18QuarantineReasonV1::RestoreCurrentnessAmbiguous;
                adapter.set_phase(
                    R18PersistentLocalSdmaPhaseV1::Quarantined,
                    R18PersistentNativeLocationV1::Quarantine,
                    None,
                    Some(reason),
                );
                Ok(R18RestoreOutcomeV1::Quarantined(
                    R18QuarantinedPersistentLocalSdmaLeaseV1 {
                        binding: self.binding,
                        reason,
                        _registry_incarnation: Rc::clone(&self.registry_incarnation),
                    },
                ))
            }
        })();
        match transition {
            Ok(outcome) => {
                if let Err(error) = adapter.validate_global_invariants() {
                    Err(R18AdapterTransitionFailureV1 {
                        error,
                        retained: self,
                    })
                } else {
                    Ok(outcome)
                }
            }
            Err(error) => Err(R18AdapterTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
pub enum R18RestoreOutcomeV1 {
    Restored(R18RestoredPersistentLocalSdmaLeaseV1),
    Quarantined(R18QuarantinedPersistentLocalSdmaLeaseV1),
}

#[derive(Debug)]
#[must_use = "restored SDMA custody must be settled before reuse or release"]
pub struct R18RestoredPersistentLocalSdmaLeaseV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    status: R18SdmaTerminalStatusV1,
    registry_incarnation: Rc<()>,
}

impl R18RestoredPersistentLocalSdmaLeaseV1 {
    pub const fn binding(&self) -> R18PersistentLocalSdmaBindingV1 {
        self.binding
    }

    pub fn settle_model_only(
        self,
        adapter: &mut R18PersistentLocalSdmaAdapterV1,
        observation: R18SettlementObservationV1,
    ) -> Result<R18SettledFrontierV1, R18AdapterTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_active(
                self.binding,
                &self.registry_incarnation,
                &[R18PersistentLocalSdmaPhaseV1::Restored],
            )?;
            exact_observation_v1(self.binding, observation.binding)?;
            if observation.status != self.status {
                return Err(R18PersistentLocalSdmaErrorV1::ObservationMismatch);
            }
            let settled_transfer_count = adapter
                .settled_transfer_count
                .checked_add(1)
                .ok_or(R18PersistentLocalSdmaErrorV1::CapacityExceeded)?;
            let R18UnderlyingPersistentLeaseV1::Terminal(lease) =
                adapter.take_persistent_lease()?
            else {
                return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
            };
            let persistent_frontier =
                match lease.settle_for_frontier_model_only(&mut adapter.registry) {
                    Ok(frontier) => frontier,
                    Err(failure) => {
                        adapter.restore_persistent_lease(R18UnderlyingPersistentLeaseV1::Terminal(
                            failure.into_parts().1,
                        ));
                        return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                    }
                };
            let key = R18SettledFrontierKeyV1 {
                allocation: adapter.allocation,
                queue: adapter.queue,
                attachment_generation: adapter.attachment_generation,
                persistent_frontier: persistent_frontier.key(),
            };
            adapter.active = None;
            adapter.pending_frontier = Some(R18PendingFrontierRecordV1 {
                key,
                persistent_frontier,
            });
            adapter.settled_transfer_count = settled_transfer_count;
            adapter.validate_global_invariants()?;
            Ok(R18SettledFrontierV1 {
                key,
                registry_incarnation: Rc::clone(&self.registry_incarnation),
            })
        })();
        match transition {
            Ok(receipt) => Ok(receipt),
            Err(error) => Err(R18AdapterTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "settled frontier must be retired before persistent reuse"]
pub struct R18SettledFrontierV1 {
    key: R18SettledFrontierKeyV1,
    registry_incarnation: Rc<()>,
}

impl R18SettledFrontierV1 {
    pub const fn key(&self) -> R18SettledFrontierKeyV1 {
        self.key
    }

    pub fn retire_model_only(
        self,
        adapter: &mut R18PersistentLocalSdmaAdapterV1,
        observed: R18SettledFrontierKeyV1,
    ) -> Result<R18FrontierRetirementReceiptV1, R18AdapterTransitionFailureV1<Self>> {
        let transition = (|| {
            if !Rc::ptr_eq(&self.registry_incarnation, &adapter.registry_incarnation) {
                return Err(R18PersistentLocalSdmaErrorV1::WrongAdapter);
            }
            if observed != self.key {
                return Err(R18PersistentLocalSdmaErrorV1::ObservationMismatch);
            }
            let Some(frontier) = adapter.pending_frontier.take() else {
                return Err(R18PersistentLocalSdmaErrorV1::StaleBinding);
            };
            if frontier.key != self.key {
                adapter.pending_frontier = Some(frontier);
                return Err(R18PersistentLocalSdmaErrorV1::StaleBinding);
            }
            let receipt = match frontier
                .persistent_frontier
                .retire_model_only(&mut adapter.registry)
            {
                Ok(receipt) => receipt,
                Err(failure) => {
                    adapter.pending_frontier = Some(R18PendingFrontierRecordV1 {
                        key: frontier.key,
                        persistent_frontier: failure.into_parts().1,
                    });
                    return Err(R18PersistentLocalSdmaErrorV1::InvariantViolation);
                }
            };
            adapter.validate_global_invariants()?;
            Ok(R18FrontierRetirementReceiptV1 {
                frontier: self.key,
                retired_use_count: receipt.retired_use_count,
            })
        })();
        match transition {
            Ok(receipt) => Ok(receipt),
            Err(error) => Err(R18AdapterTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "quarantined custody is retained until process teardown"]
pub struct R18QuarantinedPersistentLocalSdmaLeaseV1 {
    binding: R18PersistentLocalSdmaBindingV1,
    reason: R18QuarantineReasonV1,
    _registry_incarnation: Rc<()>,
}

impl R18QuarantinedPersistentLocalSdmaLeaseV1 {
    pub const fn binding(&self) -> R18PersistentLocalSdmaBindingV1 {
        self.binding
    }

    pub const fn reason(&self) -> R18QuarantineReasonV1 {
        self.reason
    }
}

pub struct R18AdapterAdmissionFailureV1 {
    error: R18PersistentLocalSdmaErrorV1,
    retained: R18LocalPersistentAllocationAdmissionV1,
}

impl core::fmt::Debug for R18AdapterAdmissionFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("R18AdapterAdmissionFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl R18AdapterAdmissionFailureV1 {
    pub const fn error(&self) -> R18PersistentLocalSdmaErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        R18PersistentLocalSdmaErrorV1,
        R18LocalPersistentAllocationAdmissionV1,
    ) {
        (self.error, self.retained)
    }
}

pub struct R18AdapterReleaseFailureV1 {
    error: R18PersistentLocalSdmaErrorV1,
    retained: R18PersistentLocalSdmaAdapterV1,
}

impl core::fmt::Debug for R18AdapterReleaseFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("R18AdapterReleaseFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl R18AdapterReleaseFailureV1 {
    pub const fn error(&self) -> R18PersistentLocalSdmaErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        R18PersistentLocalSdmaErrorV1,
        R18PersistentLocalSdmaAdapterV1,
    ) {
        (self.error, self.retained)
    }
}
