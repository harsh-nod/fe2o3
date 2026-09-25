//! Independent executable R23 model for same-device D2D persistent-SDMA windows.
//!
//! The model owns two abstract allocation authorities and pairs one source-read
//! lease with one destination-write lease. It performs no I/O and is not a
//! refinement of R17-R22, executable runtime/KFD code, native ordering,
//! hardware completion, liveness, HIP/HSA behavior, or performance. Public
//! keys and snapshots are observations only; authority remains in the private
//! move-only custody enum.

use alloc::vec::Vec;

use crate::*;

pub const R23_SAME_DEVICE_D2D_PERSISTENT_SDMA_WINDOWS_SCHEMA_VERSION_V1: u16 = 1;
pub const R23_D2D_WINDOW_MAX_PACKETS_V1: usize = R22_SDMA_WINDOW_MAX_PACKETS_V1;
pub const R23_D2D_WINDOW_MAX_BYTES_V1: u64 = R22_SDMA_WINDOW_MAX_BYTES_V1;
pub const R23_D2D_NATIVE_H2D_ENGINE_ID_V1: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dAllocationBindingV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub backing_identity: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
    pub mapped_gpu_va: GpuVaRangeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dBindingV1 {
    pub source: R23D2dAllocationBindingV1,
    pub destination: R23D2dAllocationBindingV1,
    pub queue: R18LocalSdmaQueueOccurrenceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dCopyRequestV1 {
    pub transfer_id: u64,
    pub source_range: R18ByteRangeV1,
    pub destination_range: R18ByteRangeV1,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dLeaseRoleV1 {
    SourceRead,
    DestinationWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dLeaseKeyV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub backing_identity: u64,
    pub role: R23D2dLeaseRoleV1,
    pub range: R18ByteRangeV1,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dLeasePairObservationV1 {
    pub source_read: R23D2dLeaseKeyV1,
    pub destination_write: R23D2dLeaseKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dWindowPacketV1 {
    pub packet_index: u16,
    pub transfer_offset: u64,
    pub source_range: R18ByteRangeV1,
    pub destination_range: R18ByteRangeV1,
    pub ticket: R18PlannedSdmaTicketV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R23D2dWindowPlanV1 {
    pub transfer_id: u64,
    pub window_ordinal: u64,
    pub transfer_offset: u64,
    pub byte_len: u64,
    pub source: R23D2dAllocationBindingV1,
    pub destination: R23D2dAllocationBindingV1,
    pub queue: R18LocalSdmaQueueOccurrenceV1,
    pub leases: R23D2dLeasePairObservationV1,
    pub packets: Vec<R23D2dWindowPacketV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dTicketCompletionV1 {
    pub ticket: R18PlannedSdmaTicketV1,
    pub completion_value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R23D2dAggregateCompletionMetadataV1 {
    pub plan: R23D2dWindowPlanV1,
    pub completions: Vec<R23D2dTicketCompletionV1>,
    pub aggregate_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R23D2dFrontierKeyV1 {
    pub completion: R23D2dAggregateCompletionMetadataV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dQuarantineReasonV1 {
    PublicationRetained,
    PublicationIdentityMismatch,
    CompletionIndeterminate,
    CompletionMetadataMismatch,
    FrontierMismatch,
    CurrentnessLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dPhaseV1 {
    DevicePairReady,
    Ready,
    Prepared,
    Published,
    TimedOut,
    FrontierPending,
    Completed,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dCustodyKindV1 {
    Device,
    Ready,
    Prepared,
    Published,
    Frontier,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dPublicationDispositionV1 {
    Confirmed,
    RetryableBeforeQueueCustody,
    RetainedAfterPacketWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dReservationDispositionV1 {
    Paired,
    DestinationRejectedAfterSourceReserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dPollDispositionV1 {
    Pending,
    TimedOut,
    Incomplete { completed_packets: usize },
    Completed,
    Indeterminate(R23D2dQuarantineReasonV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dCompletionRecordV1 {
    pub transfer_id: u64,
    pub succeeded: bool,
    pub failure_code: Option<i32>,
    pub completed_bytes: u64,
    pub destination_dirty_through: u64,
    pub destination_possibly_mutated_through: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dQuarantineRecordV1 {
    pub transfer_id: u64,
    pub reason: R23D2dQuarantineReasonV1,
    pub completed_bytes: u64,
    pub destination_dirty_through: u64,
    pub destination_possibly_mutated_through: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R23D2dTransferSnapshotV1 {
    pub request: R23D2dCopyRequestV1,
    pub completed_bytes: u64,
    pub window_ordinal: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R23D2dSnapshotV1 {
    pub binding: R23D2dBindingV1,
    pub phase: R23D2dPhaseV1,
    pub custody: Option<R23D2dCustodyKindV1>,
    pub source_authority_count: u8,
    pub destination_authority_count: u8,
    pub source_read_lease_count: u8,
    pub destination_write_lease_count: u8,
    pub leases: Option<R23D2dLeasePairObservationV1>,
    pub transfer: Option<R23D2dTransferSnapshotV1>,
    pub window: Option<R23D2dWindowPlanV1>,
    pub observed_completed_packets: usize,
    pub completion: Option<R23D2dCompletionRecordV1>,
    pub quarantine: Option<R23D2dQuarantineRecordV1>,
    pub target_retained: bool,
    pub current: bool,
    pub destination_dirty_through: u64,
    pub destination_possibly_mutated_through: u64,
    pub next_ring_slot: u16,
    pub slot_generations: [u32; R18_SDMA_RING_SLOT_COUNT_V1 as usize],
    pub source_next_use_generation: u64,
    pub destination_next_use_generation: u64,
    pub published_windows: u64,
    pub published_packets: u64,
    pub write_pointer_publications: u64,
    pub doorbell_publications: u64,
    pub retired_windows: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum R23D2dClassificationV1 {
    Applied,
    DependencyPending,
    Prepared(R23D2dWindowPlanV1),
    Retryable,
    Published(R23D2dWindowPlanV1),
    Pending,
    TimedOut,
    Incomplete { completed_packets: usize },
    FrontierPending(R23D2dFrontierKeyV1),
    ReadyContinuation { completed_bytes: u64 },
    Completed(R23D2dCompletionRecordV1),
    Quarantined(R23D2dQuarantineRecordV1),
    Cancelled,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R23D2dErrorV1 {
    InvalidBinding,
    InvalidPhase,
    InvalidRequest,
    InvalidTransfer,
    InvalidObservation,
    DependencyMismatch,
    CapacityExceeded,
    TargetRetained,
    Quarantined,
}

struct R23AllocationAuthorityV1 {
    binding: R23D2dAllocationBindingV1,
}

struct R23MoveOnlyLeasePairV1 {
    observation: R23D2dLeasePairObservationV1,
}

enum R23MoveOnlyD2dCustodyV1 {
    Device(R23AllocationAuthorityV1, R23AllocationAuthorityV1),
    Ready(R23AllocationAuthorityV1, R23AllocationAuthorityV1),
    Prepared {
        source: R23AllocationAuthorityV1,
        destination: R23AllocationAuthorityV1,
        leases: R23MoveOnlyLeasePairV1,
    },
    Published {
        source: R23AllocationAuthorityV1,
        destination: R23AllocationAuthorityV1,
        leases: R23MoveOnlyLeasePairV1,
    },
    Frontier {
        source: R23AllocationAuthorityV1,
        destination: R23AllocationAuthorityV1,
        leases: R23MoveOnlyLeasePairV1,
    },
    Quarantined {
        source: R23AllocationAuthorityV1,
        destination: R23AllocationAuthorityV1,
        leases: Option<R23MoveOnlyLeasePairV1>,
    },
}

impl R23MoveOnlyD2dCustodyV1 {
    const fn kind(&self) -> R23D2dCustodyKindV1 {
        match self {
            Self::Device(..) => R23D2dCustodyKindV1::Device,
            Self::Ready(..) => R23D2dCustodyKindV1::Ready,
            Self::Prepared { .. } => R23D2dCustodyKindV1::Prepared,
            Self::Published { .. } => R23D2dCustodyKindV1::Published,
            Self::Frontier { .. } => R23D2dCustodyKindV1::Frontier,
            Self::Quarantined { .. } => R23D2dCustodyKindV1::Quarantined,
        }
    }

    const fn authorities(&self) -> (&R23AllocationAuthorityV1, &R23AllocationAuthorityV1) {
        match self {
            Self::Device(source, destination) | Self::Ready(source, destination) => {
                (source, destination)
            }
            Self::Prepared {
                source,
                destination,
                ..
            }
            | Self::Published {
                source,
                destination,
                ..
            }
            | Self::Frontier {
                source,
                destination,
                ..
            }
            | Self::Quarantined {
                source,
                destination,
                ..
            } => (source, destination),
        }
    }

    const fn leases(&self) -> Option<R23D2dLeasePairObservationV1> {
        match self {
            Self::Prepared { leases, .. }
            | Self::Published { leases, .. }
            | Self::Frontier { leases, .. } => Some(leases.observation),
            Self::Quarantined { leases, .. } => match leases {
                Some(leases) => Some(leases.observation),
                None => None,
            },
            Self::Device(..) | Self::Ready(..) => None,
        }
    }
}

struct R23D2dTransferV1 {
    request: R23D2dCopyRequestV1,
    completed_bytes: u64,
    window_ordinal: u64,
}

struct R23ActiveD2dWindowV1 {
    plan: R23D2dWindowPlanV1,
    observed_completed_packets: usize,
    completion: Option<R23D2dAggregateCompletionMetadataV1>,
}

/// Sole executable owner of the independent R23 pair machine.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R23SameDeviceD2dPersistentSdmaWindowsV1;
/// fn cannot_clone(model: R23SameDeviceD2dPersistentSdmaWindowsV1) {
///     let _duplicate = model.clone();
/// }
/// ```
pub struct R23SameDeviceD2dPersistentSdmaWindowsV1 {
    binding: R23D2dBindingV1,
    phase: R23D2dPhaseV1,
    custody: Option<R23MoveOnlyD2dCustodyV1>,
    transfer: Option<R23D2dTransferV1>,
    dependencies: Vec<R20DependencyV1>,
    window: Option<R23ActiveD2dWindowV1>,
    completion: Option<R23D2dCompletionRecordV1>,
    quarantine: Option<R23D2dQuarantineRecordV1>,
    target_retained: bool,
    current: bool,
    destination_dirty_through: u64,
    destination_possibly_mutated_through: u64,
    next_ring_slot: u16,
    slot_generations: [u32; R18_SDMA_RING_SLOT_COUNT_V1 as usize],
    source_next_use_generation: u64,
    destination_next_use_generation: u64,
    published_windows: u64,
    published_packets: u64,
    write_pointer_publications: u64,
    doorbell_publications: u64,
    retired_windows: u64,
}

impl R23SameDeviceD2dPersistentSdmaWindowsV1 {
    pub fn new_model_only(binding: R23D2dBindingV1) -> Result<Self, R23D2dErrorV1> {
        validate_d2d_binding_v1(binding)?;
        Ok(Self {
            binding,
            phase: R23D2dPhaseV1::DevicePairReady,
            custody: Some(R23MoveOnlyD2dCustodyV1::Device(
                R23AllocationAuthorityV1 {
                    binding: binding.source,
                },
                R23AllocationAuthorityV1 {
                    binding: binding.destination,
                },
            )),
            transfer: None,
            dependencies: Vec::new(),
            window: None,
            completion: None,
            quarantine: None,
            target_retained: false,
            current: true,
            destination_dirty_through: 0,
            destination_possibly_mutated_through: 0,
            next_ring_slot: 0,
            slot_generations: [0; R18_SDMA_RING_SLOT_COUNT_V1 as usize],
            source_next_use_generation: 1,
            destination_next_use_generation: 1,
            published_windows: 0,
            published_packets: 0,
            write_pointer_publications: 0,
            doorbell_publications: 0,
            retired_windows: 0,
        })
    }

    pub fn snapshot(&self) -> R23D2dSnapshotV1 {
        let (source_authority_count, destination_authority_count, leases) =
            self.custody.as_ref().map_or((0, 0, None), |custody| {
                let (source, destination) = custody.authorities();
                debug_assert_eq!(source.binding, self.binding.source);
                debug_assert_eq!(destination.binding, self.binding.destination);
                (1, 1, custody.leases())
            });
        R23D2dSnapshotV1 {
            binding: self.binding,
            phase: self.phase,
            custody: self.custody.as_ref().map(R23MoveOnlyD2dCustodyV1::kind),
            source_authority_count,
            destination_authority_count,
            source_read_lease_count: u8::from(leases.is_some()),
            destination_write_lease_count: u8::from(leases.is_some()),
            leases,
            transfer: self
                .transfer
                .as_ref()
                .map(|transfer| R23D2dTransferSnapshotV1 {
                    request: transfer.request,
                    completed_bytes: transfer.completed_bytes,
                    window_ordinal: transfer.window_ordinal,
                }),
            window: self.window.as_ref().map(|window| window.plan.clone()),
            observed_completed_packets: self
                .window
                .as_ref()
                .map_or(0, |window| window.observed_completed_packets),
            completion: self.completion,
            quarantine: self.quarantine,
            target_retained: self.target_retained,
            current: self.current,
            destination_dirty_through: self.destination_dirty_through,
            destination_possibly_mutated_through: self.destination_possibly_mutated_through,
            next_ring_slot: self.next_ring_slot,
            slot_generations: self.slot_generations,
            source_next_use_generation: self.source_next_use_generation,
            destination_next_use_generation: self.destination_next_use_generation,
            published_windows: self.published_windows,
            published_packets: self.published_packets,
            write_pointer_publications: self.write_pointer_publications,
            doorbell_publications: self.doorbell_publications,
            retired_windows: self.retired_windows,
        }
    }

    pub fn begin_model_only(
        &mut self,
        request: R23D2dCopyRequestV1,
        dependencies: Vec<R20DependencyV1>,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.require_phase(R23D2dPhaseV1::DevicePairReady)?;
        if self.target_retained {
            return Err(R23D2dErrorV1::TargetRetained);
        }
        validate_d2d_request_v1(self.binding, request)?;
        if request.transfer_id == 0
            || dependencies.iter().enumerate().any(|(index, dependency)| {
                dependency.event_id == 0
                    || dependency.generation == 0
                    || dependencies[..index].contains(dependency)
            })
        {
            return Err(R23D2dErrorV1::InvalidTransfer);
        }
        let (source, destination) = self.take_unleased_pair(R23D2dCustodyKindV1::Device)?;
        self.custody = Some(R23MoveOnlyD2dCustodyV1::Ready(source, destination));
        self.transfer = Some(R23D2dTransferV1 {
            request,
            completed_bytes: 0,
            window_ordinal: 0,
        });
        self.dependencies = dependencies;
        self.window = None;
        self.completion = None;
        self.quarantine = None;
        self.target_retained = true;
        self.destination_dirty_through = 0;
        self.destination_possibly_mutated_through = 0;
        self.phase = R23D2dPhaseV1::Ready;
        Ok(R23D2dClassificationV1::Applied)
    }

    pub fn prepare_window_model_only(
        &mut self,
        observations: &[R20DependencyObservationV1],
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.prepare_window_with_reservation_model_only(
            observations,
            R23D2dReservationDispositionV1::Paired,
        )
    }

    pub fn prepare_window_with_reservation_model_only(
        &mut self,
        observations: &[R20DependencyObservationV1],
        reservation: R23D2dReservationDispositionV1,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.require_phase(R23D2dPhaseV1::Ready)?;
        if !r23_dependencies_match_v1(&self.dependencies, observations) {
            return Err(R23D2dErrorV1::DependencyMismatch);
        }
        if observations
            .iter()
            .any(|observation| observation.status == R20DependencyStatusV1::Pending)
        {
            return Ok(R23D2dClassificationV1::DependencyPending);
        }
        if observations
            .iter()
            .any(|observation| observation.status == R20DependencyStatusV1::QuiescentWithoutResult)
        {
            return self.enter_quarantine(R23D2dQuarantineReasonV1::CompletionIndeterminate);
        }
        if observations
            .iter()
            .any(|observation| observation.status == R20DependencyStatusV1::Failed)
        {
            return self.complete_before_publication(-2);
        }

        let plan = self.plan_window()?;
        self.source_next_use_generation = self
            .source_next_use_generation
            .checked_add(1)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        if reservation == R23D2dReservationDispositionV1::DestinationRejectedAfterSourceReserved {
            return Ok(R23D2dClassificationV1::Retryable);
        }
        self.destination_next_use_generation = self
            .destination_next_use_generation
            .checked_add(1)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        let (source, destination) = self.take_unleased_pair(R23D2dCustodyKindV1::Ready)?;
        self.custody = Some(R23MoveOnlyD2dCustodyV1::Prepared {
            source,
            destination,
            leases: R23MoveOnlyLeasePairV1 {
                observation: plan.leases,
            },
        });
        self.window = Some(R23ActiveD2dWindowV1 {
            plan: plan.clone(),
            observed_completed_packets: 0,
            completion: None,
        });
        self.phase = R23D2dPhaseV1::Prepared;
        Ok(R23D2dClassificationV1::Prepared(plan))
    }

    pub fn resolve_publication_model_only(
        &mut self,
        observed_plan: &R23D2dWindowPlanV1,
        disposition: R23D2dPublicationDispositionV1,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.require_phase(R23D2dPhaseV1::Prepared)?;
        let expected = self
            .window
            .as_ref()
            .ok_or(R23D2dErrorV1::InvalidTransfer)?
            .plan
            .clone();
        if observed_plan != &expected {
            return self.enter_quarantine(R23D2dQuarantineReasonV1::PublicationIdentityMismatch);
        }
        match disposition {
            R23D2dPublicationDispositionV1::RetryableBeforeQueueCustody => {
                let (source, destination, leases) =
                    self.take_leased_pair(R23D2dCustodyKindV1::Prepared)?;
                if leases.observation != expected.leases {
                    self.custody = Some(R23MoveOnlyD2dCustodyV1::Prepared {
                        source,
                        destination,
                        leases,
                    });
                    return self
                        .enter_quarantine(R23D2dQuarantineReasonV1::PublicationIdentityMismatch);
                }
                self.window = None;
                self.custody = Some(R23MoveOnlyD2dCustodyV1::Ready(source, destination));
                self.phase = R23D2dPhaseV1::Ready;
                Ok(R23D2dClassificationV1::Retryable)
            }
            R23D2dPublicationDispositionV1::RetainedAfterPacketWrite => {
                self.destination_possibly_mutated_through = expected
                    .transfer_offset
                    .checked_add(expected.byte_len)
                    .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                self.enter_quarantine(R23D2dQuarantineReasonV1::PublicationRetained)
            }
            R23D2dPublicationDispositionV1::Confirmed => {
                let packet_count = expected.packets.len();
                let packet_count_u64 =
                    u64::try_from(packet_count).map_err(|_| R23D2dErrorV1::CapacityExceeded)?;
                let next_ring_slot = ((usize::from(self.next_ring_slot) + packet_count)
                    % usize::from(R18_SDMA_RING_SLOT_COUNT_V1))
                    as u16;
                let published_windows = self
                    .published_windows
                    .checked_add(1)
                    .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                let published_packets = self
                    .published_packets
                    .checked_add(packet_count_u64)
                    .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                let write_pointer_publications = self
                    .write_pointer_publications
                    .checked_add(1)
                    .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                let doorbell_publications = self
                    .doorbell_publications
                    .checked_add(1)
                    .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                let destination_possibly_mutated_through = expected
                    .transfer_offset
                    .checked_add(expected.byte_len)
                    .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                let mut generations = self.slot_generations;
                for packet in &expected.packets {
                    let slot = usize::from(packet.ticket.slot);
                    let generation = generations[slot]
                        .checked_add(1)
                        .filter(|generation| *generation != 0)
                        .ok_or(R23D2dErrorV1::CapacityExceeded)?;
                    if generation != packet.ticket.generation {
                        return self.enter_quarantine(
                            R23D2dQuarantineReasonV1::PublicationIdentityMismatch,
                        );
                    }
                    generations[slot] = generation;
                }
                let (source, destination, leases) =
                    self.take_leased_pair(R23D2dCustodyKindV1::Prepared)?;
                if leases.observation != expected.leases {
                    self.custody = Some(R23MoveOnlyD2dCustodyV1::Prepared {
                        source,
                        destination,
                        leases,
                    });
                    return self
                        .enter_quarantine(R23D2dQuarantineReasonV1::PublicationIdentityMismatch);
                }
                self.next_ring_slot = next_ring_slot;
                self.slot_generations = generations;
                self.published_windows = published_windows;
                self.published_packets = published_packets;
                self.write_pointer_publications = write_pointer_publications;
                self.doorbell_publications = doorbell_publications;
                self.destination_possibly_mutated_through = destination_possibly_mutated_through;
                self.custody = Some(R23MoveOnlyD2dCustodyV1::Published {
                    source,
                    destination,
                    leases,
                });
                self.phase = R23D2dPhaseV1::Published;
                Ok(R23D2dClassificationV1::Published(expected))
            }
        }
    }

    pub fn poll_window_model_only(
        &mut self,
        disposition: R23D2dPollDispositionV1,
        metadata: Option<&R23D2dAggregateCompletionMetadataV1>,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        if !matches!(
            self.phase,
            R23D2dPhaseV1::Published | R23D2dPhaseV1::TimedOut
        ) {
            return Err(R23D2dErrorV1::InvalidPhase);
        }
        match disposition {
            R23D2dPollDispositionV1::Pending => {
                reject_r23_spurious_metadata_v1(metadata)?;
                Ok(R23D2dClassificationV1::Pending)
            }
            R23D2dPollDispositionV1::TimedOut => {
                reject_r23_spurious_metadata_v1(metadata)?;
                self.phase = R23D2dPhaseV1::TimedOut;
                Ok(R23D2dClassificationV1::TimedOut)
            }
            R23D2dPollDispositionV1::Incomplete { completed_packets } => {
                reject_r23_spurious_metadata_v1(metadata)?;
                let window = self.window.as_mut().ok_or(R23D2dErrorV1::InvalidTransfer)?;
                if completed_packets == 0
                    || completed_packets >= window.plan.packets.len()
                    || completed_packets < window.observed_completed_packets
                {
                    return Err(R23D2dErrorV1::InvalidObservation);
                }
                window.observed_completed_packets = completed_packets;
                Ok(R23D2dClassificationV1::Incomplete { completed_packets })
            }
            R23D2dPollDispositionV1::Indeterminate(reason) => {
                reject_r23_spurious_metadata_v1(metadata)?;
                self.enter_quarantine(reason)
            }
            R23D2dPollDispositionV1::Completed => {
                let observed = metadata.ok_or(R23D2dErrorV1::InvalidObservation)?;
                let expected = {
                    let window = self.window.as_ref().ok_or(R23D2dErrorV1::InvalidTransfer)?;
                    r23_exact_completion_metadata_v1(&window.plan)
                };
                if observed != &expected {
                    return self
                        .enter_quarantine(R23D2dQuarantineReasonV1::CompletionMetadataMismatch);
                }
                let (source, destination, leases) = self.take_leased_pair(match self.phase {
                    R23D2dPhaseV1::Published => R23D2dCustodyKindV1::Published,
                    R23D2dPhaseV1::TimedOut => R23D2dCustodyKindV1::Published,
                    _ => return Err(R23D2dErrorV1::InvalidPhase),
                })?;
                if leases.observation != expected.plan.leases {
                    self.custody = Some(R23MoveOnlyD2dCustodyV1::Published {
                        source,
                        destination,
                        leases,
                    });
                    return self
                        .enter_quarantine(R23D2dQuarantineReasonV1::CompletionMetadataMismatch);
                }
                let window = self.window.as_mut().ok_or(R23D2dErrorV1::InvalidTransfer)?;
                window.observed_completed_packets = window.plan.packets.len();
                window.completion = Some(expected.clone());
                let frontier = R23D2dFrontierKeyV1 {
                    completion: expected,
                };
                self.custody = Some(R23MoveOnlyD2dCustodyV1::Frontier {
                    source,
                    destination,
                    leases,
                });
                self.phase = R23D2dPhaseV1::FrontierPending;
                Ok(R23D2dClassificationV1::FrontierPending(frontier))
            }
        }
    }

    pub fn retire_window_model_only(
        &mut self,
        observed: &R23D2dFrontierKeyV1,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.require_phase(R23D2dPhaseV1::FrontierPending)?;
        let expected = {
            let window = self.window.as_ref().ok_or(R23D2dErrorV1::InvalidTransfer)?;
            R23D2dFrontierKeyV1 {
                completion: window
                    .completion
                    .clone()
                    .ok_or(R23D2dErrorV1::InvalidTransfer)?,
            }
        };
        if observed != &expected {
            return self.enter_quarantine(R23D2dQuarantineReasonV1::FrontierMismatch);
        }
        let retired_windows = self
            .retired_windows
            .checked_add(1)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        let (source, destination, leases) = self.take_leased_pair(R23D2dCustodyKindV1::Frontier)?;
        if leases.observation != expected.completion.plan.leases {
            self.custody = Some(R23MoveOnlyD2dCustodyV1::Frontier {
                source,
                destination,
                leases,
            });
            return self.enter_quarantine(R23D2dQuarantineReasonV1::FrontierMismatch);
        }
        let transfer = self
            .transfer
            .as_mut()
            .ok_or(R23D2dErrorV1::InvalidTransfer)?;
        transfer.completed_bytes = transfer
            .completed_bytes
            .checked_add(expected.completion.aggregate_bytes)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        transfer.window_ordinal = transfer
            .window_ordinal
            .checked_add(1)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        self.destination_dirty_through = transfer.completed_bytes;
        let classification = if transfer.completed_bytes == transfer.request.byte_len {
            let completion = R23D2dCompletionRecordV1 {
                transfer_id: transfer.request.transfer_id,
                succeeded: true,
                failure_code: None,
                completed_bytes: transfer.completed_bytes,
                destination_dirty_through: self.destination_dirty_through,
                destination_possibly_mutated_through: self.destination_possibly_mutated_through,
            };
            self.completion = Some(completion);
            self.phase = R23D2dPhaseV1::Completed;
            R23D2dClassificationV1::Completed(completion)
        } else {
            self.phase = R23D2dPhaseV1::Ready;
            R23D2dClassificationV1::ReadyContinuation {
                completed_bytes: transfer.completed_bytes,
            }
        };
        self.retired_windows = retired_windows;
        self.window = None;
        self.custody = if self.phase == R23D2dPhaseV1::Ready {
            Some(R23MoveOnlyD2dCustodyV1::Ready(source, destination))
        } else {
            Some(R23MoveOnlyD2dCustodyV1::Device(source, destination))
        };
        Ok(classification)
    }

    pub fn cancel_model_only(&mut self) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.require_phase(R23D2dPhaseV1::Ready)?;
        if self
            .transfer
            .as_ref()
            .is_none_or(|transfer| transfer.completed_bytes != 0)
        {
            return Err(R23D2dErrorV1::InvalidPhase);
        }
        let (source, destination) = self.take_unleased_pair(R23D2dCustodyKindV1::Ready)?;
        self.custody = Some(R23MoveOnlyD2dCustodyV1::Device(source, destination));
        self.transfer = None;
        self.dependencies.clear();
        self.target_retained = false;
        self.phase = R23D2dPhaseV1::DevicePairReady;
        Ok(R23D2dClassificationV1::Cancelled)
    }

    pub fn release_terminal_model_only(
        &mut self,
        transfer_id: u64,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.require_phase(R23D2dPhaseV1::Completed)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R23D2dErrorV1::InvalidTransfer)?;
        if transfer.request.transfer_id != transfer_id || !self.target_retained {
            return Err(R23D2dErrorV1::InvalidTransfer);
        }
        let (source, destination) = self.take_unleased_pair(R23D2dCustodyKindV1::Device)?;
        self.custody = Some(R23MoveOnlyD2dCustodyV1::Device(source, destination));
        self.transfer = None;
        self.dependencies.clear();
        self.completion = None;
        self.target_retained = false;
        self.phase = R23D2dPhaseV1::DevicePairReady;
        Ok(R23D2dClassificationV1::Released)
    }

    pub fn lose_currentness_model_only(&mut self) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        self.require_operational()?;
        self.enter_quarantine(R23D2dQuarantineReasonV1::CurrentnessLost)
    }

    fn plan_window(&self) -> Result<R23D2dWindowPlanV1, R23D2dErrorV1> {
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R23D2dErrorV1::InvalidTransfer)?;
        let remaining = transfer
            .request
            .byte_len
            .checked_sub(transfer.completed_bytes)
            .ok_or(R23D2dErrorV1::InvalidTransfer)?;
        let byte_len = remaining.min(R23_D2D_WINDOW_MAX_BYTES_V1);
        if byte_len == 0 {
            return Err(R23D2dErrorV1::InvalidTransfer);
        }
        let source_offset = transfer
            .request
            .source_range
            .byte_offset
            .checked_add(transfer.completed_bytes)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        let destination_offset = transfer
            .request
            .destination_range
            .byte_offset
            .checked_add(transfer.completed_bytes)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?;
        let leases = R23D2dLeasePairObservationV1 {
            source_read: lease_key_v1(
                self.binding.source,
                R23D2dLeaseRoleV1::SourceRead,
                R18ByteRangeV1 {
                    byte_offset: source_offset,
                    byte_len,
                },
                self.source_next_use_generation,
            ),
            destination_write: lease_key_v1(
                self.binding.destination,
                R23D2dLeaseRoleV1::DestinationWrite,
                R18ByteRangeV1 {
                    byte_offset: destination_offset,
                    byte_len,
                },
                self.destination_next_use_generation,
            ),
        };
        let packet_count_u64 = byte_len
            .checked_add(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 - 1)
            .ok_or(R23D2dErrorV1::CapacityExceeded)?
            / R18_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        let packet_count =
            usize::try_from(packet_count_u64).map_err(|_| R23D2dErrorV1::CapacityExceeded)?;
        if packet_count == 0 || packet_count > R23_D2D_WINDOW_MAX_PACKETS_V1 {
            return Err(R23D2dErrorV1::CapacityExceeded);
        }
        let mut packets = Vec::new();
        packets
            .try_reserve_exact(packet_count)
            .map_err(|_| R23D2dErrorV1::CapacityExceeded)?;
        for index in 0..packet_count {
            let transfer_offset = (index as u64)
                .checked_mul(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1)
                .ok_or(R23D2dErrorV1::CapacityExceeded)?;
            let packet_bytes = byte_len
                .checked_sub(transfer_offset)
                .ok_or(R23D2dErrorV1::CapacityExceeded)?
                .min(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1);
            let slot = ((usize::from(self.next_ring_slot) + index)
                % usize::from(R18_SDMA_RING_SLOT_COUNT_V1)) as u16;
            let generation = self.slot_generations[usize::from(slot)]
                .checked_add(1)
                .filter(|generation| *generation != 0)
                .ok_or(R23D2dErrorV1::CapacityExceeded)?;
            packets.push(R23D2dWindowPacketV1 {
                packet_index: u16::try_from(index).map_err(|_| R23D2dErrorV1::CapacityExceeded)?,
                transfer_offset,
                source_range: R18ByteRangeV1 {
                    byte_offset: source_offset
                        .checked_add(transfer_offset)
                        .ok_or(R23D2dErrorV1::CapacityExceeded)?,
                    byte_len: packet_bytes,
                },
                destination_range: R18ByteRangeV1 {
                    byte_offset: destination_offset
                        .checked_add(transfer_offset)
                        .ok_or(R23D2dErrorV1::CapacityExceeded)?,
                    byte_len: packet_bytes,
                },
                ticket: R18PlannedSdmaTicketV1 {
                    owner: self.binding.queue.logical_queue,
                    queue_id: self.binding.queue.native_queue_id,
                    slot,
                    generation,
                },
            });
        }
        Ok(R23D2dWindowPlanV1 {
            transfer_id: transfer.request.transfer_id,
            window_ordinal: transfer.window_ordinal,
            transfer_offset: transfer.completed_bytes,
            byte_len,
            source: self.binding.source,
            destination: self.binding.destination,
            queue: self.binding.queue,
            leases,
            packets,
        })
    }

    fn complete_before_publication(
        &mut self,
        code: i32,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        let (source, destination) = self.take_unleased_pair(R23D2dCustodyKindV1::Ready)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R23D2dErrorV1::InvalidTransfer)?;
        let completion = R23D2dCompletionRecordV1 {
            transfer_id: transfer.request.transfer_id,
            succeeded: false,
            failure_code: Some(code),
            completed_bytes: transfer.completed_bytes,
            destination_dirty_through: self.destination_dirty_through,
            destination_possibly_mutated_through: self.destination_possibly_mutated_through,
        };
        self.custody = Some(R23MoveOnlyD2dCustodyV1::Device(source, destination));
        self.completion = Some(completion);
        self.phase = R23D2dPhaseV1::Completed;
        Ok(R23D2dClassificationV1::Completed(completion))
    }

    fn enter_quarantine(
        &mut self,
        reason: R23D2dQuarantineReasonV1,
    ) -> Result<R23D2dClassificationV1, R23D2dErrorV1> {
        let custody = self.custody.take().ok_or(R23D2dErrorV1::InvalidPhase)?;
        let (source, destination, leases) = into_quarantined_parts(custody);
        let transfer_id = self
            .transfer
            .as_ref()
            .map_or(0, |transfer| transfer.request.transfer_id);
        let completed_bytes = self
            .transfer
            .as_ref()
            .map_or(0, |transfer| transfer.completed_bytes);
        let record = R23D2dQuarantineRecordV1 {
            transfer_id,
            reason,
            completed_bytes,
            destination_dirty_through: self.destination_dirty_through,
            destination_possibly_mutated_through: self.destination_possibly_mutated_through,
        };
        self.custody = Some(R23MoveOnlyD2dCustodyV1::Quarantined {
            source,
            destination,
            leases,
        });
        self.quarantine = Some(record);
        self.current = false;
        self.target_retained = true;
        self.phase = R23D2dPhaseV1::Quarantined;
        Ok(R23D2dClassificationV1::Quarantined(record))
    }

    fn take_unleased_pair(
        &mut self,
        expected: R23D2dCustodyKindV1,
    ) -> Result<(R23AllocationAuthorityV1, R23AllocationAuthorityV1), R23D2dErrorV1> {
        let custody = self.custody.take().ok_or(R23D2dErrorV1::InvalidPhase)?;
        if custody.kind() != expected || custody.leases().is_some() {
            self.custody = Some(custody);
            return Err(R23D2dErrorV1::InvalidPhase);
        }
        match custody {
            R23MoveOnlyD2dCustodyV1::Device(source, destination)
            | R23MoveOnlyD2dCustodyV1::Ready(source, destination) => Ok((source, destination)),
            _ => unreachable!("unleased custody kind was checked"),
        }
    }

    fn take_leased_pair(
        &mut self,
        expected: R23D2dCustodyKindV1,
    ) -> Result<
        (
            R23AllocationAuthorityV1,
            R23AllocationAuthorityV1,
            R23MoveOnlyLeasePairV1,
        ),
        R23D2dErrorV1,
    > {
        let custody = self.custody.take().ok_or(R23D2dErrorV1::InvalidPhase)?;
        if custody.kind() != expected || custody.leases().is_none() {
            self.custody = Some(custody);
            return Err(R23D2dErrorV1::InvalidPhase);
        }
        match custody {
            R23MoveOnlyD2dCustodyV1::Prepared {
                source,
                destination,
                leases,
            }
            | R23MoveOnlyD2dCustodyV1::Published {
                source,
                destination,
                leases,
            }
            | R23MoveOnlyD2dCustodyV1::Frontier {
                source,
                destination,
                leases,
            } => Ok((source, destination, leases)),
            _ => unreachable!("leased custody kind was checked"),
        }
    }

    fn require_operational(&self) -> Result<(), R23D2dErrorV1> {
        if self.phase == R23D2dPhaseV1::Quarantined {
            Err(R23D2dErrorV1::Quarantined)
        } else {
            Ok(())
        }
    }

    fn require_phase(&self, expected: R23D2dPhaseV1) -> Result<(), R23D2dErrorV1> {
        if self.phase == expected {
            Ok(())
        } else {
            Err(R23D2dErrorV1::InvalidPhase)
        }
    }
}

pub fn r23_exact_completion_metadata_v1(
    plan: &R23D2dWindowPlanV1,
) -> R23D2dAggregateCompletionMetadataV1 {
    R23D2dAggregateCompletionMetadataV1 {
        plan: plan.clone(),
        completions: plan
            .packets
            .iter()
            .map(|packet| R23D2dTicketCompletionV1 {
                ticket: packet.ticket,
                completion_value: packet.ticket.generation,
            })
            .collect(),
        aggregate_bytes: plan.byte_len,
    }
}

fn validate_d2d_binding_v1(binding: R23D2dBindingV1) -> Result<(), R23D2dErrorV1> {
    validate_d2d_allocation_v1(binding.source)?;
    validate_d2d_allocation_v1(binding.destination)?;
    let source = binding.source.allocation;
    let destination = binding.destination.allocation;
    if source.owner == destination.owner
        || source.allocation == destination.allocation
        || source.mapping == destination.mapping
        || binding.source.backing_identity == binding.destination.backing_identity
        || source.allocation.vm != destination.allocation.vm
        || binding.queue.logical_queue.vm != source.allocation.vm
        || binding.queue.logical_queue.generation.0 == 0
        || binding.queue.occurrence == 0
        || binding.queue.native_queue_id >= R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1
        || binding.queue.engine_id != R23_D2D_NATIVE_H2D_ENGINE_ID_V1
        || gpu_va_ranges_overlap_v1(
            binding.source.mapped_gpu_va,
            binding.destination.mapped_gpu_va,
        )
    {
        return Err(R23D2dErrorV1::InvalidBinding);
    }
    Ok(())
}

fn validate_d2d_allocation_v1(binding: R23D2dAllocationBindingV1) -> Result<(), R23D2dErrorV1> {
    if binding.allocation.owner.0 == 0
        || binding.allocation.allocation.vm.device.generation.0 == 0
        || binding.allocation.allocation.vm.id.0 == 0
        || binding.allocation.allocation.id.0 == 0
        || binding.allocation.allocation.generation.0 == 0
        || binding.allocation.mapping.allocation != binding.allocation.allocation
        || binding.allocation.mapping.id.0 == 0
        || binding.attachment_generation == 0
        || binding.pool_generation == 0
        || binding.backing_identity == 0
        || binding.logical_byte_len == 0
        || binding.logical_byte_len > binding.physical_byte_len
        || binding.physical_byte_len > R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1
        || binding.mapped_gpu_va.base == 0
        || binding.mapped_gpu_va.byte_len != binding.physical_byte_len
        || binding.mapped_gpu_va.checked_end().is_none()
    {
        return Err(R23D2dErrorV1::InvalidBinding);
    }
    Ok(())
}

fn validate_d2d_request_v1(
    binding: R23D2dBindingV1,
    request: R23D2dCopyRequestV1,
) -> Result<(), R23D2dErrorV1> {
    if request.byte_len == 0
        || request.source_range.byte_len != request.byte_len
        || request.destination_range.byte_len != request.byte_len
        || request.source_range.checked_end().is_none_or(|end| {
            end > binding.source.logical_byte_len || end > binding.source.physical_byte_len
        })
        || request.destination_range.checked_end().is_none_or(|end| {
            end > binding.destination.logical_byte_len
                || end > binding.destination.physical_byte_len
        })
    {
        return Err(R23D2dErrorV1::InvalidRequest);
    }
    Ok(())
}

const fn gpu_va_ranges_overlap_v1(left: GpuVaRangeV1, right: GpuVaRangeV1) -> bool {
    match (left.checked_end(), right.checked_end()) {
        (Some(left_end), Some(right_end)) => left.base < right_end && right.base < left_end,
        _ => true,
    }
}

const fn lease_key_v1(
    binding: R23D2dAllocationBindingV1,
    role: R23D2dLeaseRoleV1,
    range: R18ByteRangeV1,
    generation: u64,
) -> R23D2dLeaseKeyV1 {
    R23D2dLeaseKeyV1 {
        allocation: binding.allocation,
        attachment_generation: binding.attachment_generation,
        pool_generation: binding.pool_generation,
        backing_identity: binding.backing_identity,
        role,
        range,
        generation,
    }
}

fn r23_dependencies_match_v1(
    expected: &[R20DependencyV1],
    observed: &[R20DependencyObservationV1],
) -> bool {
    expected.len() == observed.len()
        && expected.iter().all(|dependency| {
            observed
                .iter()
                .filter(|observation| observation.dependency == *dependency)
                .count()
                == 1
        })
}

fn reject_r23_spurious_metadata_v1(
    metadata: Option<&R23D2dAggregateCompletionMetadataV1>,
) -> Result<(), R23D2dErrorV1> {
    if metadata.is_some() {
        Err(R23D2dErrorV1::InvalidObservation)
    } else {
        Ok(())
    }
}

fn into_quarantined_parts(
    custody: R23MoveOnlyD2dCustodyV1,
) -> (
    R23AllocationAuthorityV1,
    R23AllocationAuthorityV1,
    Option<R23MoveOnlyLeasePairV1>,
) {
    match custody {
        R23MoveOnlyD2dCustodyV1::Device(source, destination)
        | R23MoveOnlyD2dCustodyV1::Ready(source, destination) => (source, destination, None),
        R23MoveOnlyD2dCustodyV1::Prepared {
            source,
            destination,
            leases,
        }
        | R23MoveOnlyD2dCustodyV1::Published {
            source,
            destination,
            leases,
        }
        | R23MoveOnlyD2dCustodyV1::Frontier {
            source,
            destination,
            leases,
        } => (source, destination, Some(leases)),
        R23MoveOnlyD2dCustodyV1::Quarantined {
            source,
            destination,
            leases,
        } => (source, destination, leases),
    }
}
