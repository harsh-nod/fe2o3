//! Executable R22 model for batched directional persistent-SDMA windows.
//!
//! R22 is an additive successor that retains R19 identities and directional
//! roles while independently modeling one aggregate move-only pair and lease.
//! One window contains at most 63 exact contiguous packets and has one abstract
//! write-pointer publication and one abstract doorbell action. This module
//! performs no I/O and is not a refinement of executable R19, concrete Rust,
//! KFD, hardware, liveness, HIP/HSA behavior, or performance. Its independent
//! Verus artifact is not a refinement of this executable model.

use alloc::{boxed::Box, vec::Vec};

use crate::*;

pub const R22_BATCHED_DIRECTIONAL_PERSISTENT_SDMA_WINDOWS_SCHEMA_VERSION_V1: u16 = 1;
pub const R22_SDMA_WINDOW_MAX_PACKETS_V1: usize = R18_SDMA_RING_SLOT_COUNT_V1 as usize - 1;
pub const R22_SDMA_WINDOW_MAX_BYTES_V1: u64 =
    R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 * R22_SDMA_WINDOW_MAX_PACKETS_V1 as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R22WindowPhaseV1 {
    DeviceReady,
    Ready,
    Prepared,
    Published,
    FrontierPending,
    Completed,
    QuiescentWithoutResult,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R22WindowCustodyKindV1 {
    Device,
    Ready,
    PreparedWindow,
    PublishedWindow,
    FrontierPending,
    Opaque,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R22WindowFailurePointV1 {
    Publication,
    Completion,
    CompletionMetadata,
    Retirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R22WindowPublicationDispositionV1 {
    Confirmed,
    RetryableBeforeQueueCustody,
    RetainedAfterPacketWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R22WindowPollDispositionV1 {
    Pending,
    TimedOut,
    Partial { completed_packets: usize },
    RecoveredWithoutTerminal,
    Terminal(R18SdmaTerminalStatusV1),
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R22AggregateLeaseKeyV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub host: R18HostBufferKeyV1,
    pub direction: R18LocalSdmaDirectionV1,
    pub device_range: R18ByteRangeV1,
    pub host_range: R18ByteRangeV1,
    pub use_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R22WindowPacketV1 {
    pub packet_index: u16,
    pub transfer_offset: u64,
    pub device_range: R18ByteRangeV1,
    pub host_range: R18ByteRangeV1,
    pub ticket: R18PlannedSdmaTicketV1,
}

/// Cloneable observation only; authority stays in the private custody enum.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R22WindowPlanV1 {
    pub transfer_id: u64,
    pub window_ordinal: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub transfer_offset: u64,
    pub byte_len: u64,
    pub lease: R22AggregateLeaseKeyV1,
    pub packets: Vec<R22WindowPacketV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R22WindowCompletionMetadataV1 {
    pub plan: R22WindowPlanV1,
    pub completed_packets: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R22WindowFrontierKeyV1 {
    pub plan: R22WindowPlanV1,
    pub status: R18SdmaTerminalStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R22WindowTransferSnapshotV1 {
    pub transfer_id: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub source: R20CopyEndpointV1,
    pub destination: R20CopyEndpointV1,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub window_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R22WindowCompletionRecordV1 {
    pub transfer_id: u64,
    pub succeeded: bool,
    pub failure_code: Option<i32>,
    pub completed_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R22WindowQuiescentRecordV1 {
    pub transfer_id: u64,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub possibly_mutated_through: u64,
    pub host_possibly_mutated_through: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R22WindowSnapshotV1 {
    pub binding: R21SeamBindingV1,
    pub phase: R22WindowPhaseV1,
    pub custody: Option<R22WindowCustodyKindV1>,
    pub authority_count: u8,
    pub aggregate_lease_count: u8,
    pub opaque_failure: Option<R22WindowFailurePointV1>,
    pub transfer: Option<R22WindowTransferSnapshotV1>,
    pub window: Option<R22WindowPlanV1>,
    pub observed_completed_packets: usize,
    pub completion: Option<R22WindowCompletionRecordV1>,
    pub quiescent: Option<R22WindowQuiescentRecordV1>,
    pub target_retained: bool,
    pub destination_dirty_through: u64,
    pub host_dirty_through: u64,
    pub possibly_mutated_through: u64,
    pub host_possibly_mutated_through: u64,
    pub next_ring_slot: u16,
    pub slot_generations: [u32; R18_SDMA_RING_SLOT_COUNT_V1 as usize],
    pub next_use_generation: u64,
    pub published_windows: u64,
    pub published_packets: u64,
    pub write_pointer_publications: u64,
    pub doorbell_publications: u64,
    pub retired_windows: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum R22WindowClassificationV1 {
    Applied,
    DependencyPending,
    Prepared(R22WindowPlanV1),
    Retryable,
    Published(R22WindowCompletionMetadataV1),
    Pending,
    TimedOut,
    Partial { completed_packets: usize },
    FrontierPending(R22WindowFrontierKeyV1),
    ReadyContinuation { completed_bytes: u64 },
    Completed(R22WindowCompletionRecordV1),
    QuiescentWithoutResult(R22WindowQuiescentRecordV1),
    Released,
    ProcessTeardown { point: R22WindowFailurePointV1 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R22WindowErrorV1 {
    InvalidBinding,
    InvalidPhase,
    InvalidRequest,
    InvalidObservation,
    InvalidTransfer,
    DependencyMismatch,
    TargetRetained,
    CapacityExceeded,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R22AuthorityKeyV1 {
    allocation: R18NativeAllocationKeyV1,
    pair_occurrence: u64,
    attachment_generation: u64,
    pool_generation: u64,
    host_storage_id: u64,
    host_storage_generation: u64,
}

struct R22TransferV1 {
    request: R20CopyRequestV1,
    direction: R18LocalSdmaDirectionV1,
    host: R18HostBufferKeyV1,
    host_base: u64,
    device_base: u64,
    completed_bytes: u64,
    window_ordinal: u64,
}

struct R22ActiveWindowV1 {
    plan: R22WindowPlanV1,
    observed_completed_packets: usize,
    terminal_status: Option<R18SdmaTerminalStatusV1>,
}

enum R22MoveOnlyCustodyV1 {
    Device(R22AuthorityKeyV1),
    Ready(R22AuthorityKeyV1),
    PreparedWindow {
        authority: R22AuthorityKeyV1,
        lease: R22AggregateLeaseKeyV1,
    },
    PublishedWindow {
        authority: R22AuthorityKeyV1,
        lease: R22AggregateLeaseKeyV1,
    },
    FrontierPending {
        authority: R22AuthorityKeyV1,
        lease: R22AggregateLeaseKeyV1,
    },
    Opaque {
        prior: Box<R22MoveOnlyCustodyV1>,
        point: R22WindowFailurePointV1,
    },
}

impl R22MoveOnlyCustodyV1 {
    const fn kind(&self) -> R22WindowCustodyKindV1 {
        match self {
            Self::Device(_) => R22WindowCustodyKindV1::Device,
            Self::Ready(_) => R22WindowCustodyKindV1::Ready,
            Self::PreparedWindow { .. } => R22WindowCustodyKindV1::PreparedWindow,
            Self::PublishedWindow { .. } => R22WindowCustodyKindV1::PublishedWindow,
            Self::FrontierPending { .. } => R22WindowCustodyKindV1::FrontierPending,
            Self::Opaque { .. } => R22WindowCustodyKindV1::Opaque,
        }
    }

    const fn authority(&self) -> R22AuthorityKeyV1 {
        match self {
            Self::Device(authority)
            | Self::Ready(authority)
            | Self::PreparedWindow { authority, .. }
            | Self::PublishedWindow { authority, .. }
            | Self::FrontierPending { authority, .. } => *authority,
            Self::Opaque { prior, .. } => prior.authority(),
        }
    }

    const fn lease(&self) -> Option<R22AggregateLeaseKeyV1> {
        match self {
            Self::PreparedWindow { lease, .. }
            | Self::PublishedWindow { lease, .. }
            | Self::FrontierPending { lease, .. } => Some(*lease),
            Self::Opaque { prior, .. } => prior.lease(),
            Self::Device(_) | Self::Ready(_) => None,
        }
    }

    const fn opaque_point(&self) -> Option<R22WindowFailurePointV1> {
        match self {
            Self::Opaque { point, .. } => Some(*point),
            _ => None,
        }
    }
}

/// Single-pair R22 model. The custody enum intentionally implements neither
/// `Clone` nor `Copy`; every transition moves its sole aggregate authority.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R22BatchedDirectionalPersistentSdmaWindowsV1;
/// fn cannot_clone(model: R22BatchedDirectionalPersistentSdmaWindowsV1) {
///     let _copy = model.clone();
/// }
/// ```
pub struct R22BatchedDirectionalPersistentSdmaWindowsV1 {
    binding: R21SeamBindingV1,
    phase: R22WindowPhaseV1,
    custody: Option<R22MoveOnlyCustodyV1>,
    transfer: Option<R22TransferV1>,
    dependencies: Vec<R20DependencyV1>,
    window: Option<R22ActiveWindowV1>,
    completion: Option<R22WindowCompletionRecordV1>,
    quiescent: Option<R22WindowQuiescentRecordV1>,
    target_retained: bool,
    destination_dirty_through: u64,
    host_dirty_through: u64,
    possibly_mutated_through: u64,
    host_possibly_mutated_through: u64,
    next_ring_slot: u16,
    slot_generations: [u32; R18_SDMA_RING_SLOT_COUNT_V1 as usize],
    next_use_generation: u64,
    published_windows: u64,
    published_packets: u64,
    write_pointer_publications: u64,
    doorbell_publications: u64,
    retired_windows: u64,
}

impl R22BatchedDirectionalPersistentSdmaWindowsV1 {
    pub fn new_model_only(binding: R21SeamBindingV1) -> Result<Self, R22WindowErrorV1> {
        validate_binding_v1(binding)?;
        let authority = authority_key_v1(binding);
        Ok(Self {
            binding,
            phase: R22WindowPhaseV1::DeviceReady,
            custody: Some(R22MoveOnlyCustodyV1::Device(authority)),
            transfer: None,
            dependencies: Vec::new(),
            window: None,
            completion: None,
            quiescent: None,
            target_retained: false,
            destination_dirty_through: 0,
            host_dirty_through: 0,
            possibly_mutated_through: 0,
            host_possibly_mutated_through: 0,
            next_ring_slot: 0,
            slot_generations: [0; R18_SDMA_RING_SLOT_COUNT_V1 as usize],
            next_use_generation: 1,
            published_windows: 0,
            published_packets: 0,
            write_pointer_publications: 0,
            doorbell_publications: 0,
            retired_windows: 0,
        })
    }

    pub fn from_idle_r19_snapshot_model_only(
        snapshot: R19DirectionalSnapshotV1,
        host_storage_id: u64,
        host_storage_generation: u64,
    ) -> Result<Self, R22WindowErrorV1> {
        let binding = R21SeamBindingV1::from_idle_r19_snapshot_model_only(
            snapshot,
            host_storage_id,
            host_storage_generation,
        )
        .map_err(|_| R22WindowErrorV1::InvalidBinding)?;
        Self::new_model_only(binding)
    }

    pub fn snapshot(&self) -> R22WindowSnapshotV1 {
        R22WindowSnapshotV1 {
            binding: self.binding,
            phase: self.phase,
            custody: self.custody.as_ref().map(R22MoveOnlyCustodyV1::kind),
            authority_count: u8::from(self.custody.is_some()),
            aggregate_lease_count: u8::from(
                self.custody
                    .as_ref()
                    .is_some_and(|custody| custody.lease().is_some()),
            ),
            opaque_failure: self
                .custody
                .as_ref()
                .and_then(R22MoveOnlyCustodyV1::opaque_point),
            transfer: self.transfer.as_ref().map(transfer_snapshot_v1),
            window: self.window.as_ref().map(|window| window.plan.clone()),
            observed_completed_packets: self
                .window
                .as_ref()
                .map_or(0, |window| window.observed_completed_packets),
            completion: self.completion,
            quiescent: self.quiescent,
            target_retained: self.target_retained,
            destination_dirty_through: self.destination_dirty_through,
            host_dirty_through: self.host_dirty_through,
            possibly_mutated_through: self.possibly_mutated_through,
            host_possibly_mutated_through: self.host_possibly_mutated_through,
            next_ring_slot: self.next_ring_slot,
            slot_generations: self.slot_generations,
            next_use_generation: self.next_use_generation,
            published_windows: self.published_windows,
            published_packets: self.published_packets,
            write_pointer_publications: self.write_pointer_publications,
            doorbell_publications: self.doorbell_publications,
            retired_windows: self.retired_windows,
        }
    }

    pub fn begin_model_only(
        &mut self,
        request: R20CopyRequestV1,
        dependencies: Vec<R20DependencyV1>,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        self.require_phase(R22WindowPhaseV1::DeviceReady)?;
        if self.target_retained {
            return Err(R22WindowErrorV1::TargetRetained);
        }
        if request.transfer_id == 0
            || dependencies.iter().any(|dependency| {
                dependency.event_id == 0
                    || dependency.generation == 0
                    || dependencies
                        .iter()
                        .filter(|other| *other == dependency)
                        .count()
                        != 1
            })
        {
            return Err(R22WindowErrorV1::InvalidTransfer);
        }
        let (direction, host, host_base, device_base) = resolve_request_v1(self.binding, request)?;
        let authority = self.take_authority_v1(R22WindowCustodyKindV1::Device)?;
        self.custody = Some(R22MoveOnlyCustodyV1::Ready(authority));
        self.transfer = Some(R22TransferV1 {
            request,
            direction,
            host,
            host_base,
            device_base,
            completed_bytes: 0,
            window_ordinal: 0,
        });
        self.dependencies = dependencies;
        self.window = None;
        self.completion = None;
        self.quiescent = None;
        self.target_retained = true;
        self.destination_dirty_through = 0;
        self.host_dirty_through = 0;
        self.possibly_mutated_through = 0;
        self.host_possibly_mutated_through = 0;
        self.phase = R22WindowPhaseV1::Ready;
        Ok(R22WindowClassificationV1::Applied)
    }

    pub fn prepare_window_model_only(
        &mut self,
        observations: &[R20DependencyObservationV1],
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        self.require_phase(R22WindowPhaseV1::Ready)?;
        if !dependencies_match_v1(&self.dependencies, observations) {
            return Err(R22WindowErrorV1::DependencyMismatch);
        }
        if observations
            .iter()
            .any(|observation| observation.status == R20DependencyStatusV1::Pending)
        {
            return Ok(R22WindowClassificationV1::DependencyPending);
        }
        if observations
            .iter()
            .any(|observation| observation.status == R20DependencyStatusV1::QuiescentWithoutResult)
        {
            let record = self.quiesce_without_window_v1()?;
            return Ok(R22WindowClassificationV1::QuiescentWithoutResult(record));
        }
        if observations
            .iter()
            .any(|observation| observation.status == R20DependencyStatusV1::Failed)
        {
            let record = self.fail_before_window_v1(-2)?;
            return Ok(R22WindowClassificationV1::Completed(record));
        }

        let plan = self.plan_window_v1()?;
        let authority = self.take_authority_v1(R22WindowCustodyKindV1::Ready)?;
        self.custody = Some(R22MoveOnlyCustodyV1::PreparedWindow {
            authority,
            lease: plan.lease,
        });
        self.window = Some(R22ActiveWindowV1 {
            plan: plan.clone(),
            observed_completed_packets: 0,
            terminal_status: None,
        });
        self.phase = R22WindowPhaseV1::Prepared;
        Ok(R22WindowClassificationV1::Prepared(plan))
    }

    pub fn resolve_publication_model_only(
        &mut self,
        observed_plan: &R22WindowPlanV1,
        disposition: R22WindowPublicationDispositionV1,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        self.require_phase(R22WindowPhaseV1::Prepared)?;
        let expected = self
            .window
            .as_ref()
            .ok_or(R22WindowErrorV1::InvalidTransfer)?
            .plan
            .clone();
        if observed_plan != &expected {
            return self.enter_teardown_v1(R22WindowFailurePointV1::Publication);
        }
        match disposition {
            R22WindowPublicationDispositionV1::RetryableBeforeQueueCustody => {
                let (authority, lease) =
                    self.take_window_custody_v1(R22WindowCustodyKindV1::PreparedWindow)?;
                if lease != expected.lease {
                    self.custody = Some(R22MoveOnlyCustodyV1::PreparedWindow { authority, lease });
                    return self.enter_teardown_v1(R22WindowFailurePointV1::Publication);
                }
                self.window = None;
                self.custody = Some(R22MoveOnlyCustodyV1::Ready(authority));
                self.phase = R22WindowPhaseV1::Ready;
                Ok(R22WindowClassificationV1::Retryable)
            }
            R22WindowPublicationDispositionV1::RetainedAfterPacketWrite => {
                self.enter_teardown_v1(R22WindowFailurePointV1::Publication)
            }
            R22WindowPublicationDispositionV1::Confirmed => {
                let packet_count = expected.packets.len();
                let packet_count_u64 =
                    u64::try_from(packet_count).map_err(|_| R22WindowErrorV1::CapacityExceeded)?;
                let next_use_generation = self
                    .next_use_generation
                    .checked_add(1)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                let next_ring_slot = ((usize::from(self.next_ring_slot) + packet_count)
                    % usize::from(R18_SDMA_RING_SLOT_COUNT_V1))
                    as u16;
                let published_packets = self
                    .published_packets
                    .checked_add(packet_count_u64)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                let published_windows = self
                    .published_windows
                    .checked_add(1)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                let write_pointer_publications = self
                    .write_pointer_publications
                    .checked_add(1)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                let doorbell_publications = self
                    .doorbell_publications
                    .checked_add(1)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                let mut slot_generations = self.slot_generations;
                for packet in &expected.packets {
                    let slot = usize::from(packet.ticket.slot);
                    let generation = slot_generations[slot]
                        .checked_add(1)
                        .filter(|generation| *generation != 0)
                        .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                    if generation != packet.ticket.generation {
                        return self.enter_teardown_v1(R22WindowFailurePointV1::Publication);
                    }
                    slot_generations[slot] = generation;
                }
                let window_end = expected
                    .transfer_offset
                    .checked_add(expected.byte_len)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                let (authority, lease) =
                    self.take_window_custody_v1(R22WindowCustodyKindV1::PreparedWindow)?;
                if lease != expected.lease {
                    self.custody = Some(R22MoveOnlyCustodyV1::PreparedWindow { authority, lease });
                    return self.enter_teardown_v1(R22WindowFailurePointV1::Publication);
                }
                self.next_ring_slot = next_ring_slot;
                self.slot_generations = slot_generations;
                self.next_use_generation = next_use_generation;
                self.published_packets = published_packets;
                self.published_windows = published_windows;
                self.write_pointer_publications = write_pointer_publications;
                self.doorbell_publications = doorbell_publications;
                self.possibly_mutated_through = self.possibly_mutated_through.max(window_end);
                if expected.direction == R18LocalSdmaDirectionV1::DeviceToHost {
                    self.host_possibly_mutated_through =
                        self.host_possibly_mutated_through.max(window_end);
                }
                self.custody = Some(R22MoveOnlyCustodyV1::PublishedWindow { authority, lease });
                self.phase = R22WindowPhaseV1::Published;
                Ok(R22WindowClassificationV1::Published(
                    completion_metadata_v1(&expected),
                ))
            }
        }
    }

    pub fn poll_window_model_only(
        &mut self,
        disposition: R22WindowPollDispositionV1,
        metadata: Option<&R22WindowCompletionMetadataV1>,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        self.require_phase(R22WindowPhaseV1::Published)?;
        match disposition {
            R22WindowPollDispositionV1::Pending => {
                reject_spurious_metadata_v1(metadata)?;
                Ok(R22WindowClassificationV1::Pending)
            }
            R22WindowPollDispositionV1::TimedOut => {
                reject_spurious_metadata_v1(metadata)?;
                Ok(R22WindowClassificationV1::TimedOut)
            }
            R22WindowPollDispositionV1::Partial { completed_packets } => {
                reject_spurious_metadata_v1(metadata)?;
                let window = self
                    .window
                    .as_mut()
                    .ok_or(R22WindowErrorV1::InvalidTransfer)?;
                if completed_packets == 0
                    || completed_packets >= window.plan.packets.len()
                    || completed_packets < window.observed_completed_packets
                {
                    return Err(R22WindowErrorV1::InvalidObservation);
                }
                window.observed_completed_packets = completed_packets;
                Ok(R22WindowClassificationV1::Partial { completed_packets })
            }
            R22WindowPollDispositionV1::RecoveredWithoutTerminal => {
                reject_spurious_metadata_v1(metadata)?;
                let (authority, _) =
                    self.take_window_custody_v1(R22WindowCustodyKindV1::PublishedWindow)?;
                self.custody = Some(R22MoveOnlyCustodyV1::Device(authority));
                self.window = None;
                self.dependencies.clear();
                let transfer = self
                    .transfer
                    .as_ref()
                    .ok_or(R22WindowErrorV1::InvalidTransfer)?;
                let quiescent = R22WindowQuiescentRecordV1 {
                    transfer_id: transfer.request.transfer_id,
                    completed_bytes: transfer.completed_bytes,
                    total_bytes: transfer.request.byte_len,
                    possibly_mutated_through: self.possibly_mutated_through,
                    host_possibly_mutated_through: self.host_possibly_mutated_through,
                };
                self.quiescent = Some(quiescent);
                self.phase = R22WindowPhaseV1::QuiescentWithoutResult;
                Ok(R22WindowClassificationV1::QuiescentWithoutResult(quiescent))
            }
            R22WindowPollDispositionV1::ProcessTeardown => {
                reject_spurious_metadata_v1(metadata)?;
                self.enter_teardown_v1(R22WindowFailurePointV1::Completion)
            }
            R22WindowPollDispositionV1::Terminal(status) => {
                let observed = metadata.ok_or(R22WindowErrorV1::InvalidObservation)?;
                let expected = {
                    let window = self
                        .window
                        .as_ref()
                        .ok_or(R22WindowErrorV1::InvalidTransfer)?;
                    completion_metadata_v1(&window.plan)
                };
                if observed != &expected {
                    return self.enter_teardown_v1(R22WindowFailurePointV1::CompletionMetadata);
                }
                let (authority, lease) =
                    self.take_window_custody_v1(R22WindowCustodyKindV1::PublishedWindow)?;
                if lease != expected.plan.lease {
                    self.custody = Some(R22MoveOnlyCustodyV1::PublishedWindow { authority, lease });
                    return self.enter_teardown_v1(R22WindowFailurePointV1::CompletionMetadata);
                }
                let window = self
                    .window
                    .as_mut()
                    .ok_or(R22WindowErrorV1::InvalidTransfer)?;
                window.observed_completed_packets = window.plan.packets.len();
                window.terminal_status = Some(status);
                let frontier = R22WindowFrontierKeyV1 {
                    plan: window.plan.clone(),
                    status,
                };
                self.custody = Some(R22MoveOnlyCustodyV1::FrontierPending { authority, lease });
                self.phase = R22WindowPhaseV1::FrontierPending;
                Ok(R22WindowClassificationV1::FrontierPending(frontier))
            }
        }
    }

    pub fn retire_window_model_only(
        &mut self,
        observed: &R22WindowFrontierKeyV1,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        self.require_phase(R22WindowPhaseV1::FrontierPending)?;
        let expected = {
            let window = self
                .window
                .as_ref()
                .ok_or(R22WindowErrorV1::InvalidTransfer)?;
            R22WindowFrontierKeyV1 {
                plan: window.plan.clone(),
                status: window
                    .terminal_status
                    .ok_or(R22WindowErrorV1::InvalidTransfer)?,
            }
        };
        if observed != &expected {
            return self.enter_teardown_v1(R22WindowFailurePointV1::Retirement);
        }
        let (authority, lease) =
            self.take_window_custody_v1(R22WindowCustodyKindV1::FrontierPending)?;
        if lease != expected.plan.lease {
            self.custody = Some(R22MoveOnlyCustodyV1::FrontierPending { authority, lease });
            return self.enter_teardown_v1(R22WindowFailurePointV1::Retirement);
        }
        let retired_windows = self
            .retired_windows
            .checked_add(1)
            .ok_or(R22WindowErrorV1::CapacityExceeded)?;
        let transfer = self
            .transfer
            .as_mut()
            .ok_or(R22WindowErrorV1::InvalidTransfer)?;
        let classification = match expected.status {
            R18SdmaTerminalStatusV1::Succeeded => {
                transfer.completed_bytes = transfer
                    .completed_bytes
                    .checked_add(expected.plan.byte_len)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                self.destination_dirty_through = transfer.completed_bytes;
                if transfer.direction == R18LocalSdmaDirectionV1::DeviceToHost {
                    self.host_dirty_through = transfer.completed_bytes;
                }
                transfer.window_ordinal = transfer
                    .window_ordinal
                    .checked_add(1)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?;
                if transfer.completed_bytes == transfer.request.byte_len {
                    let completion = R22WindowCompletionRecordV1 {
                        transfer_id: transfer.request.transfer_id,
                        succeeded: true,
                        failure_code: None,
                        completed_bytes: transfer.completed_bytes,
                    };
                    self.completion = Some(completion);
                    self.phase = R22WindowPhaseV1::Completed;
                    R22WindowClassificationV1::Completed(completion)
                } else {
                    self.phase = R22WindowPhaseV1::Ready;
                    R22WindowClassificationV1::ReadyContinuation {
                        completed_bytes: transfer.completed_bytes,
                    }
                }
            }
            R18SdmaTerminalStatusV1::Failed { code } => {
                let completion = R22WindowCompletionRecordV1 {
                    transfer_id: transfer.request.transfer_id,
                    succeeded: false,
                    failure_code: Some(code),
                    completed_bytes: transfer.completed_bytes,
                };
                self.completion = Some(completion);
                self.phase = R22WindowPhaseV1::Completed;
                R22WindowClassificationV1::Completed(completion)
            }
        };
        self.retired_windows = retired_windows;
        self.window = None;
        self.dependencies.clear();
        self.custody = Some(if self.phase == R22WindowPhaseV1::Ready {
            R22MoveOnlyCustodyV1::Ready(authority)
        } else {
            R22MoveOnlyCustodyV1::Device(authority)
        });
        Ok(classification)
    }

    pub fn cancel_model_only(
        &mut self,
        transfer_id: u64,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        self.require_phase(R22WindowPhaseV1::Ready)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R22WindowErrorV1::InvalidTransfer)?;
        if transfer.request.transfer_id != transfer_id || transfer.completed_bytes != 0 {
            return Err(R22WindowErrorV1::InvalidTransfer);
        }
        let authority = self.take_authority_v1(R22WindowCustodyKindV1::Ready)?;
        self.custody = Some(R22MoveOnlyCustodyV1::Device(authority));
        self.transfer = None;
        self.dependencies.clear();
        self.target_retained = false;
        self.phase = R22WindowPhaseV1::DeviceReady;
        Ok(R22WindowClassificationV1::Released)
    }

    pub fn poll_submission_model_only(
        &self,
        transfer_id: u64,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        if self
            .transfer
            .as_ref()
            .is_none_or(|transfer| transfer.request.transfer_id != transfer_id)
        {
            return Err(R22WindowErrorV1::InvalidTransfer);
        }
        match self.phase {
            R22WindowPhaseV1::Ready
            | R22WindowPhaseV1::Prepared
            | R22WindowPhaseV1::Published
            | R22WindowPhaseV1::FrontierPending => Ok(R22WindowClassificationV1::Pending),
            R22WindowPhaseV1::Completed => Ok(R22WindowClassificationV1::Completed(
                self.completion.ok_or(R22WindowErrorV1::InvalidTransfer)?,
            )),
            R22WindowPhaseV1::QuiescentWithoutResult => {
                Ok(R22WindowClassificationV1::QuiescentWithoutResult(
                    self.quiescent.ok_or(R22WindowErrorV1::InvalidTransfer)?,
                ))
            }
            _ => Err(R22WindowErrorV1::InvalidPhase),
        }
    }

    pub fn release_submission_model_only(
        &mut self,
        transfer_id: u64,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        self.require_operational()?;
        if !matches!(
            self.phase,
            R22WindowPhaseV1::Completed | R22WindowPhaseV1::QuiescentWithoutResult
        ) || !self.target_retained
            || self
                .transfer
                .as_ref()
                .is_none_or(|transfer| transfer.request.transfer_id != transfer_id)
        {
            return Err(R22WindowErrorV1::InvalidTransfer);
        }
        if self.custody.as_ref().map(R22MoveOnlyCustodyV1::kind)
            != Some(R22WindowCustodyKindV1::Device)
        {
            return Err(R22WindowErrorV1::InvalidPhase);
        }
        self.transfer = None;
        self.dependencies.clear();
        self.window = None;
        self.completion = None;
        self.quiescent = None;
        self.target_retained = false;
        self.phase = R22WindowPhaseV1::DeviceReady;
        Ok(R22WindowClassificationV1::Released)
    }

    fn plan_window_v1(&self) -> Result<R22WindowPlanV1, R22WindowErrorV1> {
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R22WindowErrorV1::InvalidTransfer)?;
        let remaining = transfer
            .request
            .byte_len
            .checked_sub(transfer.completed_bytes)
            .ok_or(R22WindowErrorV1::InvalidRequest)?;
        if remaining == 0 {
            return Err(R22WindowErrorV1::InvalidPhase);
        }
        let window_byte_len = remaining.min(R22_SDMA_WINDOW_MAX_BYTES_V1);
        let packet_count_u64 = window_byte_len
            .checked_add(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1 - 1)
            .ok_or(R22WindowErrorV1::CapacityExceeded)?
            / R18_SDMA_MAX_LINEAR_COPY_BYTES_V1;
        let packet_count =
            usize::try_from(packet_count_u64).map_err(|_| R22WindowErrorV1::CapacityExceeded)?;
        if packet_count == 0 || packet_count > R22_SDMA_WINDOW_MAX_PACKETS_V1 {
            return Err(R22WindowErrorV1::CapacityExceeded);
        }
        self.next_use_generation
            .checked_add(1)
            .ok_or(R22WindowErrorV1::CapacityExceeded)?;

        let device_window_offset = transfer
            .device_base
            .checked_add(transfer.completed_bytes)
            .ok_or(R22WindowErrorV1::CapacityExceeded)?;
        let host_window_offset = transfer
            .host_base
            .checked_add(transfer.completed_bytes)
            .ok_or(R22WindowErrorV1::CapacityExceeded)?;
        let lease = R22AggregateLeaseKeyV1 {
            allocation: self.binding.allocation,
            pair: self.binding.pair,
            attachment_generation: self.binding.attachment_generation,
            pool_generation: self.binding.pool_generation,
            host: transfer.host,
            direction: transfer.direction,
            device_range: R18ByteRangeV1 {
                byte_offset: device_window_offset,
                byte_len: window_byte_len,
            },
            host_range: R18ByteRangeV1 {
                byte_offset: host_window_offset,
                byte_len: window_byte_len,
            },
            use_generation: self.next_use_generation,
        };
        let child = self.binding.pair.child(transfer.direction);
        let mut packets = Vec::with_capacity(packet_count);
        let mut relative_offset = 0_u64;
        for index in 0..packet_count {
            let packet_byte_len =
                (window_byte_len - relative_offset).min(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1);
            let packet_index =
                u16::try_from(index).map_err(|_| R22WindowErrorV1::CapacityExceeded)?;
            let slot = ((usize::from(self.next_ring_slot) + index)
                % usize::from(R18_SDMA_RING_SLOT_COUNT_V1)) as u16;
            let generation = self.slot_generations[usize::from(slot)]
                .checked_add(1)
                .filter(|generation| *generation != 0)
                .ok_or(R22WindowErrorV1::CapacityExceeded)?;
            packets.push(R22WindowPacketV1 {
                packet_index,
                transfer_offset: transfer
                    .completed_bytes
                    .checked_add(relative_offset)
                    .ok_or(R22WindowErrorV1::CapacityExceeded)?,
                device_range: R18ByteRangeV1 {
                    byte_offset: device_window_offset
                        .checked_add(relative_offset)
                        .ok_or(R22WindowErrorV1::CapacityExceeded)?,
                    byte_len: packet_byte_len,
                },
                host_range: R18ByteRangeV1 {
                    byte_offset: host_window_offset
                        .checked_add(relative_offset)
                        .ok_or(R22WindowErrorV1::CapacityExceeded)?,
                    byte_len: packet_byte_len,
                },
                ticket: R18PlannedSdmaTicketV1 {
                    owner: self.binding.pair.parent_queue,
                    queue_id: child.native_queue_id,
                    slot,
                    generation,
                },
            });
            relative_offset = relative_offset
                .checked_add(packet_byte_len)
                .ok_or(R22WindowErrorV1::CapacityExceeded)?;
        }
        if relative_offset != window_byte_len {
            return Err(R22WindowErrorV1::CapacityExceeded);
        }
        Ok(R22WindowPlanV1 {
            transfer_id: transfer.request.transfer_id,
            window_ordinal: transfer.window_ordinal,
            direction: transfer.direction,
            transfer_offset: transfer.completed_bytes,
            byte_len: window_byte_len,
            lease,
            packets,
        })
    }

    fn fail_before_window_v1(
        &mut self,
        code: i32,
    ) -> Result<R22WindowCompletionRecordV1, R22WindowErrorV1> {
        let authority = self.take_authority_v1(R22WindowCustodyKindV1::Ready)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R22WindowErrorV1::InvalidTransfer)?;
        let completion = R22WindowCompletionRecordV1 {
            transfer_id: transfer.request.transfer_id,
            succeeded: false,
            failure_code: Some(code),
            completed_bytes: transfer.completed_bytes,
        };
        self.custody = Some(R22MoveOnlyCustodyV1::Device(authority));
        self.dependencies.clear();
        self.completion = Some(completion);
        self.phase = R22WindowPhaseV1::Completed;
        Ok(completion)
    }

    fn quiesce_without_window_v1(
        &mut self,
    ) -> Result<R22WindowQuiescentRecordV1, R22WindowErrorV1> {
        let authority = self.take_authority_v1(R22WindowCustodyKindV1::Ready)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R22WindowErrorV1::InvalidTransfer)?;
        let quiescent = R22WindowQuiescentRecordV1 {
            transfer_id: transfer.request.transfer_id,
            completed_bytes: transfer.completed_bytes,
            total_bytes: transfer.request.byte_len,
            possibly_mutated_through: self.possibly_mutated_through,
            host_possibly_mutated_through: self.host_possibly_mutated_through,
        };
        self.custody = Some(R22MoveOnlyCustodyV1::Device(authority));
        self.dependencies.clear();
        self.quiescent = Some(quiescent);
        self.phase = R22WindowPhaseV1::QuiescentWithoutResult;
        Ok(quiescent)
    }

    fn enter_teardown_v1(
        &mut self,
        point: R22WindowFailurePointV1,
    ) -> Result<R22WindowClassificationV1, R22WindowErrorV1> {
        let prior = self.custody.take().ok_or(R22WindowErrorV1::InvalidPhase)?;
        self.custody = Some(R22MoveOnlyCustodyV1::Opaque {
            prior: Box::new(prior),
            point,
        });
        self.phase = R22WindowPhaseV1::ProcessTeardown;
        Ok(R22WindowClassificationV1::ProcessTeardown { point })
    }

    fn take_authority_v1(
        &mut self,
        expected: R22WindowCustodyKindV1,
    ) -> Result<R22AuthorityKeyV1, R22WindowErrorV1> {
        let custody = self.custody.take().ok_or(R22WindowErrorV1::InvalidPhase)?;
        if custody.kind() != expected || custody.lease().is_some() {
            self.custody = Some(custody);
            return Err(R22WindowErrorV1::InvalidPhase);
        }
        Ok(custody.authority())
    }

    fn take_window_custody_v1(
        &mut self,
        expected: R22WindowCustodyKindV1,
    ) -> Result<(R22AuthorityKeyV1, R22AggregateLeaseKeyV1), R22WindowErrorV1> {
        let custody = self.custody.take().ok_or(R22WindowErrorV1::InvalidPhase)?;
        if custody.kind() != expected {
            self.custody = Some(custody);
            return Err(R22WindowErrorV1::InvalidPhase);
        }
        let authority = custody.authority();
        let lease = match custody.lease() {
            Some(lease) => lease,
            None => {
                self.custody = Some(custody);
                return Err(R22WindowErrorV1::InvalidPhase);
            }
        };
        Ok((authority, lease))
    }

    fn require_operational(&self) -> Result<(), R22WindowErrorV1> {
        if self.phase == R22WindowPhaseV1::ProcessTeardown {
            Err(R22WindowErrorV1::ProcessTeardown)
        } else {
            Ok(())
        }
    }

    fn require_phase(&self, expected: R22WindowPhaseV1) -> Result<(), R22WindowErrorV1> {
        if self.phase == expected {
            Ok(())
        } else {
            Err(R22WindowErrorV1::InvalidPhase)
        }
    }
}

fn validate_binding_v1(binding: R21SeamBindingV1) -> Result<(), R22WindowErrorV1> {
    if binding.attachment_generation == 0
        || binding.pool_generation == 0
        || binding.logical_byte_len == 0
        || binding.logical_byte_len > binding.physical_byte_len
        || binding.host_storage_id == 0
        || binding.host_storage_generation == 0
        || binding.pair.pair_occurrence == 0
        || binding.pair.device_to_host.native_queue_id
            == binding.pair.host_to_device.native_queue_id
        || binding.pair.device_to_host.engine_id != R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1
        || binding.pair.host_to_device.engine_id != R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1
    {
        return Err(R22WindowErrorV1::InvalidBinding);
    }
    Ok(())
}

const fn authority_key_v1(binding: R21SeamBindingV1) -> R22AuthorityKeyV1 {
    R22AuthorityKeyV1 {
        allocation: binding.allocation,
        pair_occurrence: binding.pair.pair_occurrence,
        attachment_generation: binding.attachment_generation,
        pool_generation: binding.pool_generation,
        host_storage_id: binding.host_storage_id,
        host_storage_generation: binding.host_storage_generation,
    }
}

fn resolve_request_v1(
    binding: R21SeamBindingV1,
    request: R20CopyRequestV1,
) -> Result<(R18LocalSdmaDirectionV1, R18HostBufferKeyV1, u64, u64), R22WindowErrorV1> {
    if request.byte_len == 0 || request.byte_len > binding.logical_byte_len {
        return Err(R22WindowErrorV1::InvalidRequest);
    }
    let (direction, host, host_offset, allocation, device_offset) =
        match (request.source, request.destination) {
            (
                R20CopyEndpointV1::Host { buffer, offset },
                R20CopyEndpointV1::Device {
                    allocation,
                    offset: device_offset,
                },
            ) => (
                R18LocalSdmaDirectionV1::HostToDevice,
                buffer,
                offset,
                allocation,
                device_offset,
            ),
            (
                R20CopyEndpointV1::Device {
                    allocation,
                    offset: device_offset,
                },
                R20CopyEndpointV1::Host { buffer, offset },
            ) => (
                R18LocalSdmaDirectionV1::DeviceToHost,
                buffer,
                offset,
                allocation,
                device_offset,
            ),
            _ => return Err(R22WindowErrorV1::InvalidRequest),
        };
    if allocation != binding.allocation
        || host.id != binding.host_storage_id
        || host.generation != binding.host_storage_generation
        || host.coherence != MemoryCoherenceV1::HostCoherent
        || host_offset
            .checked_add(request.byte_len)
            .is_none_or(|end| end > host.byte_len)
        || device_offset
            .checked_add(request.byte_len)
            .is_none_or(|end| end > binding.logical_byte_len)
    {
        return Err(R22WindowErrorV1::InvalidRequest);
    }
    Ok((direction, host, host_offset, device_offset))
}

fn dependencies_match_v1(
    expected: &[R20DependencyV1],
    observed: &[R20DependencyObservationV1],
) -> bool {
    expected.len() == observed.len()
        && expected
            .iter()
            .zip(observed)
            .all(|(expected, observed)| *expected == observed.dependency)
}

fn completion_metadata_v1(plan: &R22WindowPlanV1) -> R22WindowCompletionMetadataV1 {
    R22WindowCompletionMetadataV1 {
        plan: plan.clone(),
        completed_packets: plan.packets.len(),
    }
}

fn reject_spurious_metadata_v1(
    metadata: Option<&R22WindowCompletionMetadataV1>,
) -> Result<(), R22WindowErrorV1> {
    if metadata.is_some() {
        Err(R22WindowErrorV1::InvalidObservation)
    } else {
        Ok(())
    }
}

fn transfer_snapshot_v1(transfer: &R22TransferV1) -> R22WindowTransferSnapshotV1 {
    R22WindowTransferSnapshotV1 {
        transfer_id: transfer.request.transfer_id,
        direction: transfer.direction,
        source: transfer.request.source,
        destination: transfer.request.destination,
        total_bytes: transfer.request.byte_len,
        completed_bytes: transfer.completed_bytes,
        window_ordinal: transfer.window_ordinal,
    }
}
