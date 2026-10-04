//! Executable R20 model for runtime-facade directional chunking.
//!
//! The facade owns and directly composes one R19 adapter. Every successful
//! packet passes through the R19 prepare, publish, complete, restore, settle,
//! and exact-frontier-retire transitions before another packet becomes ready.
//! This model performs no I/O and is not a Rust, KFD, hardware, liveness, or
//! performance refinement. The independent Verus artifact is not a refinement
//! of this executable model.

use alloc::{boxed::Box, vec::Vec};

use crate::*;

pub const R20_RUNTIME_FACADE_DIRECTIONAL_CHUNKING_SCHEMA_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20CopyEndpointV1 {
    Host {
        buffer: R18HostBufferKeyV1,
        offset: u64,
    },
    Device {
        allocation: R18NativeAllocationKeyV1,
        offset: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20CopyRequestV1 {
    pub transfer_id: u64,
    pub source: R20CopyEndpointV1,
    pub destination: R20CopyEndpointV1,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20DependencyV1 {
    pub event_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20DependencyObservationV1 {
    pub dependency: R20DependencyV1,
    pub status: R20DependencyStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20DependencyStatusV1 {
    Pending,
    Satisfied,
    Failed,
    QuiescentWithoutResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20FacadePacketPhaseV1 {
    Idle,
    Ready,
    Published,
    Completed,
    QuiescentWithoutResult,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20ActiveTransferSnapshotV1 {
    pub transfer_id: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub source: R20CopyEndpointV1,
    pub destination: R20CopyEndpointV1,
    pub byte_len: u64,
    pub completed_bytes: u64,
    pub packet_offset: Option<u64>,
    pub packet_byte_len: Option<u64>,
    pub ticket: Option<R18PlannedSdmaTicketV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R20RuntimeFacadeSnapshotV1 {
    pub phase: R20FacadePacketPhaseV1,
    pub active: Option<R20ActiveTransferSnapshotV1>,
    pub destination_dirty: Vec<R20DestinationDirtyV1>,
    pub completions: Vec<R20CompletionRecordV1>,
    pub quiescent_without_result: Option<R20QuiescentWithoutResultV1>,
    pub retained_targets: Vec<u64>,
    pub next_ticket_generation: u32,
    pub adapter: Option<R19DirectionalSnapshotV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20QuiescentWithoutResultV1 {
    pub transfer_id: u64,
    pub completed_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20DestinationDirtyV1 {
    pub transfer_id: u64,
    pub destination: R20CopyEndpointV1,
    pub byte_offset: u64,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R20CompletionRecordV1 {
    pub transfer_id: u64,
    pub succeeded: bool,
    pub failure_code: Option<i32>,
    pub completed_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20PublicationDispositionV1 {
    Confirmed,
    RetryableBeforeQueueCustody,
    OpaqueAfterPacketWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20PollObservationV1 {
    Pending,
    TimedOut,
    Succeeded,
    Failed { code: i32 },
    CurrentnessAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20FlushOutcomeV1 {
    Published { transfer_id: u64, byte_len: u64 },
    DependencyPending,
    Quiescent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20PollOutcomeV1 {
    Pending,
    TimedOut,
    ReadyContinuation { completed_bytes: u64 },
    Completed(R20CompletionRecordV1),
    QuiescentWithoutResult(R20QuiescentWithoutResultV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R20FacadeErrorV1 {
    Busy,
    UnsupportedCopy,
    InvalidEndpoint,
    InvalidRange,
    InvalidTransfer,
    DependencyMismatch,
    NotPublished,
    TooLate,
    CapacityExceeded,
    ProcessTeardown,
    LowerInvariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R20TransferRecordV1 {
    request: R20CopyRequestV1,
    direction: R18LocalSdmaDirectionV1,
    host: R18HostBufferKeyV1,
    host_base: u64,
    device_base: u64,
    completed_bytes: u64,
}

struct R20PublishedPacketV1 {
    transfer: R20TransferRecordV1,
    packet_offset: u64,
    packet_byte_len: u64,
    binding: R19DirectionalTransferBindingV1,
    lease: R19DirectionalTransferLeaseV1,
}

enum R20ActiveV1 {
    Ready(Box<R20TransferRecordV1>),
    Published(Box<R20PublishedPacketV1>),
}

#[derive(Clone, Copy)]
struct R20PacketMetadataV1 {
    transfer: R20TransferRecordV1,
    packet_offset: u64,
    packet_byte_len: u64,
    binding: R19DirectionalTransferBindingV1,
}

enum R20OpaqueCustodyV1 {
    Quarantined {
        adapter: R19DirectionalPersistentLocalSdmaAdapterV1,
        lease: R19DirectionalQuarantinedLeaseV1,
    },
    LowerTransition {
        adapter: R19DirectionalPersistentLocalSdmaAdapterV1,
        lease: R19DirectionalTransferLeaseV1,
    },
    Frontier {
        adapter: R19DirectionalPersistentLocalSdmaAdapterV1,
        frontier: R19DirectionalSettledFrontierV1,
    },
}

/// Single-allocation, single-flight R20 facade model.
pub struct R20RuntimeFacadeDirectionalChunkingV1 {
    adapter: Option<R19DirectionalPersistentLocalSdmaAdapterV1>,
    active: Option<R20ActiveV1>,
    dependencies: Vec<R20DependencyV1>,
    destination_dirty: Vec<R20DestinationDirtyV1>,
    completions: Vec<R20CompletionRecordV1>,
    quiescent_without_result: Option<R20QuiescentWithoutResultV1>,
    next_ticket_generation: u32,
    opaque: Option<R20OpaqueCustodyV1>,
    retained_targets: Vec<u64>,
}

impl R20RuntimeFacadeDirectionalChunkingV1 {
    pub fn new_model_only(adapter: R19DirectionalPersistentLocalSdmaAdapterV1) -> Self {
        Self {
            adapter: Some(adapter),
            active: None,
            dependencies: Vec::new(),
            destination_dirty: Vec::new(),
            completions: Vec::new(),
            quiescent_without_result: None,
            next_ticket_generation: 1,
            opaque: None,
            retained_targets: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> R20RuntimeFacadeSnapshotV1 {
        let (phase, active) = match self.active.as_ref() {
            None => (
                if self.opaque.is_some() {
                    R20FacadePacketPhaseV1::ProcessTeardown
                } else if self.quiescent_without_result.is_some() {
                    R20FacadePacketPhaseV1::QuiescentWithoutResult
                } else if !self.completions.is_empty() {
                    R20FacadePacketPhaseV1::Completed
                } else {
                    R20FacadePacketPhaseV1::Idle
                },
                None,
            ),
            Some(R20ActiveV1::Ready(record)) => (
                R20FacadePacketPhaseV1::Ready,
                Some(snapshot_record(**record, None)),
            ),
            Some(R20ActiveV1::Published(packet)) => (
                R20FacadePacketPhaseV1::Published,
                Some(snapshot_record(
                    packet.transfer,
                    Some((
                        packet.packet_offset,
                        packet.packet_byte_len,
                        packet.binding.ticket,
                    )),
                )),
            ),
        };
        R20RuntimeFacadeSnapshotV1 {
            phase,
            active,
            destination_dirty: self.destination_dirty.clone(),
            completions: self.completions.clone(),
            quiescent_without_result: self.quiescent_without_result,
            retained_targets: self.retained_targets.clone(),
            next_ticket_generation: self.next_ticket_generation,
            adapter: self.adapter.as_ref().map(|adapter| adapter.snapshot()),
        }
    }

    pub fn enqueue_model_only(
        &mut self,
        request: R20CopyRequestV1,
        dependencies: Vec<R20DependencyV1>,
    ) -> Result<(), R20FacadeErrorV1> {
        self.require_operational()?;
        if self.active.is_some() || self.quiescent_without_result.is_some() {
            return Err(R20FacadeErrorV1::Busy);
        }
        // Direction is deliberately resolved before any facade mutation.
        let (direction, host, host_base, device_allocation, device_base) =
            resolve_direction_v1(request)?;
        let adapter = self
            .adapter
            .as_ref()
            .ok_or(R20FacadeErrorV1::ProcessTeardown)?;
        let snapshot = adapter.snapshot();
        if device_allocation != snapshot.allocation {
            return Err(R20FacadeErrorV1::InvalidEndpoint);
        }
        validate_range_v1(device_base, request.byte_len, snapshot.logical_byte_len)?;
        validate_range_v1(host_base, request.byte_len, host.byte_len)?;
        if request.transfer_id == 0
            || self.retained_targets.contains(&request.transfer_id)
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
            return Err(R20FacadeErrorV1::InvalidTransfer);
        }
        self.dependencies = dependencies;
        self.retained_targets.push(request.transfer_id);
        self.active = Some(R20ActiveV1::Ready(Box::new(R20TransferRecordV1 {
            request,
            direction,
            host,
            host_base,
            device_base,
            completed_bytes: 0,
        })));
        Ok(())
    }

    pub fn flush_model_only(
        &mut self,
        observed_dependencies: &[R20DependencyObservationV1],
        disposition: R20PublicationDispositionV1,
    ) -> Result<R20FlushOutcomeV1, R20FacadeErrorV1> {
        self.require_operational()?;
        let record = match self.active.take() {
            Some(R20ActiveV1::Ready(record)) => *record,
            Some(active) => {
                self.active = Some(active);
                return Err(R20FacadeErrorV1::Busy);
            }
            None => return Err(R20FacadeErrorV1::InvalidTransfer),
        };
        if !dependencies_match_v1(&self.dependencies, observed_dependencies) {
            self.active = Some(R20ActiveV1::Ready(Box::new(record)));
            return Err(R20FacadeErrorV1::DependencyMismatch);
        }
        if observed_dependencies
            .iter()
            .any(|entry| entry.status == R20DependencyStatusV1::Pending)
        {
            self.active = Some(R20ActiveV1::Ready(Box::new(record)));
            return Ok(R20FlushOutcomeV1::DependencyPending);
        }
        if observed_dependencies
            .iter()
            .any(|entry| entry.status == R20DependencyStatusV1::QuiescentWithoutResult)
        {
            self.dependencies.clear();
            self.quiescent_without_result = Some(R20QuiescentWithoutResultV1 {
                transfer_id: record.request.transfer_id,
                completed_bytes: record.completed_bytes,
                total_bytes: record.request.byte_len,
            });
            return Ok(R20FlushOutcomeV1::Quiescent);
        }
        if observed_dependencies
            .iter()
            .any(|entry| entry.status == R20DependencyStatusV1::Failed)
        {
            self.dependencies.clear();
            self.completions.push(R20CompletionRecordV1 {
                transfer_id: record.request.transfer_id,
                succeeded: false,
                failure_code: Some(-2),
                completed_bytes: record.completed_bytes,
            });
            return Ok(R20FlushOutcomeV1::Quiescent);
        }
        let packet_offset = record.completed_bytes;
        let remaining = record
            .request
            .byte_len
            .checked_sub(packet_offset)
            .ok_or(R20FacadeErrorV1::LowerInvariant)?;
        let packet_byte_len = remaining.min(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        let generation = self.next_ticket_generation;
        let next_generation = generation
            .checked_add(1)
            .ok_or(R20FacadeErrorV1::CapacityExceeded)?;
        let mut adapter = self
            .adapter
            .take()
            .ok_or(R20FacadeErrorV1::ProcessTeardown)?;
        let child = adapter.pair().child(record.direction);
        let ticket = R18PlannedSdmaTicketV1 {
            owner: adapter.pair().parent_queue,
            queue_id: child.native_queue_id,
            slot: (generation % u32::from(R18_SDMA_RING_SLOT_COUNT_V1)) as u16,
            generation,
        };
        let device_offset = record
            .device_base
            .checked_add(packet_offset)
            .ok_or(R20FacadeErrorV1::InvalidRange)?;
        let host_offset = record
            .host_base
            .checked_add(packet_offset)
            .ok_or(R20FacadeErrorV1::InvalidRange)?;
        let prepared = match adapter.prepare_model_only(
            record.direction,
            R18ByteRangeV1 {
                byte_offset: device_offset,
                byte_len: packet_byte_len,
            },
            record.host,
            R18ByteRangeV1 {
                byte_offset: host_offset,
                byte_len: packet_byte_len,
            },
            ticket,
        ) {
            Ok(lease) => lease,
            Err(_) => {
                self.adapter = Some(adapter);
                self.active = Some(R20ActiveV1::Ready(Box::new(record)));
                return Err(R20FacadeErrorV1::LowerInvariant);
            }
        };
        let binding = prepared.binding();
        let resolution = match disposition {
            R20PublicationDispositionV1::Confirmed => R18PublicationResolutionV1::Confirmed,
            R20PublicationDispositionV1::RetryableBeforeQueueCustody => {
                R18PublicationResolutionV1::RecoverableFailure {
                    point: R18PrepublicationFailurePointV1::BeforeQueueCustody,
                }
            }
            R20PublicationDispositionV1::OpaqueAfterPacketWrite => {
                R18PublicationResolutionV1::IndeterminateRetention {
                    point: R18PrepublicationFailurePointV1::PacketWrite,
                }
            }
        };
        match prepared.resolve_publication_model_only(
            &mut adapter,
            R19DirectionalPublicationObservationV1 {
                binding,
                resolution,
            },
        ) {
            Ok(R19DirectionalPublicationOutcomeV1::Published(lease)) => {
                self.next_ticket_generation = next_generation;
                self.adapter = Some(adapter);
                self.active = Some(R20ActiveV1::Published(Box::new(R20PublishedPacketV1 {
                    transfer: record,
                    packet_offset,
                    packet_byte_len,
                    binding,
                    lease,
                })));
                Ok(R20FlushOutcomeV1::Published {
                    transfer_id: record.request.transfer_id,
                    byte_len: packet_byte_len,
                })
            }
            Ok(R19DirectionalPublicationOutcomeV1::Recovered(_)) => {
                let partial = record.completed_bytes != 0;
                let restored = record;
                self.next_ticket_generation = next_generation;
                self.adapter = Some(adapter);
                if partial {
                    self.dependencies.clear();
                    self.quiescent_without_result = Some(R20QuiescentWithoutResultV1 {
                        transfer_id: restored.request.transfer_id,
                        completed_bytes: restored.completed_bytes,
                        total_bytes: restored.request.byte_len,
                    });
                    Ok(R20FlushOutcomeV1::Quiescent)
                } else {
                    self.dependencies.clear();
                    self.completions.push(R20CompletionRecordV1 {
                        transfer_id: restored.request.transfer_id,
                        succeeded: false,
                        failure_code: Some(-1),
                        completed_bytes: 0,
                    });
                    Ok(R20FlushOutcomeV1::Quiescent)
                }
            }
            Ok(R19DirectionalPublicationOutcomeV1::Quarantined(lease)) => {
                self.opaque = Some(R20OpaqueCustodyV1::Quarantined { adapter, lease });
                Err(R20FacadeErrorV1::ProcessTeardown)
            }
            Err(failure) => {
                let (_, lease) = failure.into_parts();
                self.opaque = Some(R20OpaqueCustodyV1::LowerTransition { adapter, lease });
                Err(R20FacadeErrorV1::ProcessTeardown)
            }
        }
    }

    pub fn poll_model_only(
        &mut self,
        observation: R20PollObservationV1,
    ) -> Result<R20PollOutcomeV1, R20FacadeErrorV1> {
        self.require_operational()?;
        let packet = match self.active.take() {
            Some(R20ActiveV1::Published(packet)) => packet,
            Some(active) => {
                self.active = Some(active);
                return Err(R20FacadeErrorV1::NotPublished);
            }
            None => return Err(R20FacadeErrorV1::NotPublished),
        };
        let mut adapter = self
            .adapter
            .take()
            .ok_or(R20FacadeErrorV1::ProcessTeardown)?;
        let R20PublishedPacketV1 {
            transfer,
            packet_offset,
            packet_byte_len,
            binding,
            lease,
        } = *packet;
        let metadata = R20PacketMetadataV1 {
            transfer,
            packet_offset,
            packet_byte_len,
            binding,
        };
        let resolution = match observation {
            R20PollObservationV1::Pending => R18CompletionResolutionV1::Pending,
            R20PollObservationV1::TimedOut => R18CompletionResolutionV1::TimedOut,
            R20PollObservationV1::Succeeded => {
                R18CompletionResolutionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded)
            }
            R20PollObservationV1::Failed { code } => {
                R18CompletionResolutionV1::Terminal(R18SdmaTerminalStatusV1::Failed { code })
            }
            R20PollObservationV1::CurrentnessAmbiguous => {
                R18CompletionResolutionV1::CurrentnessAmbiguous
            }
        };
        let observed = R19DirectionalCompletionObservationV1 {
            binding,
            resolution,
        };
        match lease.observe_model_only(&mut adapter, observed) {
            Ok(R19DirectionalPollV1::Pending(lease)) => {
                self.adapter = Some(adapter);
                self.active = Some(R20ActiveV1::Published(Box::new(R20PublishedPacketV1 {
                    transfer,
                    packet_offset,
                    packet_byte_len,
                    binding,
                    lease,
                })));
                Ok(R20PollOutcomeV1::Pending)
            }
            Ok(R19DirectionalPollV1::TimedOut(lease)) => {
                self.adapter = Some(adapter);
                self.active = Some(R20ActiveV1::Published(Box::new(R20PublishedPacketV1 {
                    transfer,
                    packet_offset,
                    packet_byte_len,
                    binding,
                    lease,
                })));
                Ok(R20PollOutcomeV1::TimedOut)
            }
            Ok(R19DirectionalPollV1::Quarantined(lease)) => {
                self.opaque = Some(R20OpaqueCustodyV1::Quarantined { adapter, lease });
                Err(R20FacadeErrorV1::ProcessTeardown)
            }
            Ok(R19DirectionalPollV1::Completed(lease)) => {
                self.finish_packet_model_only(adapter, metadata, lease, observation)
            }
            Err(failure) => {
                let (_, lease) = failure.into_parts();
                self.opaque = Some(R20OpaqueCustodyV1::LowerTransition { adapter, lease });
                Err(R20FacadeErrorV1::ProcessTeardown)
            }
        }
    }

    pub fn cancel_model_only(&mut self) -> Result<u64, R20FacadeErrorV1> {
        self.require_operational()?;
        match self.active.take() {
            Some(R20ActiveV1::Ready(record)) if record.completed_bytes == 0 => {
                self.dependencies.clear();
                self.retained_targets
                    .retain(|target| *target != record.request.transfer_id);
                Ok(record.request.transfer_id)
            }
            Some(active) => {
                self.active = Some(active);
                Err(R20FacadeErrorV1::TooLate)
            }
            None => Err(R20FacadeErrorV1::InvalidTransfer),
        }
    }

    pub fn release_quiescent_model_only(
        &mut self,
        transfer_id: u64,
    ) -> Result<R20QuiescentWithoutResultV1, R20FacadeErrorV1> {
        self.require_operational()?;
        let record = self
            .quiescent_without_result
            .ok_or(R20FacadeErrorV1::InvalidTransfer)?;
        if record.transfer_id != transfer_id {
            return Err(R20FacadeErrorV1::InvalidTransfer);
        }
        self.release_submission_model_only(transfer_id)?;
        Ok(record)
    }

    pub fn poll_submission_model_only(
        &self,
        transfer_id: u64,
    ) -> Result<R20PollOutcomeV1, R20FacadeErrorV1> {
        self.require_operational()?;
        if let Some(record) = self
            .completions
            .iter()
            .find(|record| record.transfer_id == transfer_id)
        {
            return Ok(R20PollOutcomeV1::Completed(*record));
        }
        match self.quiescent_without_result {
            Some(record) if record.transfer_id == transfer_id => {
                Ok(R20PollOutcomeV1::QuiescentWithoutResult(record))
            }
            _ => Err(R20FacadeErrorV1::InvalidTransfer),
        }
    }

    pub fn release_submission_model_only(
        &mut self,
        transfer_id: u64,
    ) -> Result<(), R20FacadeErrorV1> {
        self.require_operational()?;
        if self.active.as_ref().is_some_and(|active| match active {
            R20ActiveV1::Ready(record) => record.request.transfer_id == transfer_id,
            R20ActiveV1::Published(packet) => packet.transfer.request.transfer_id == transfer_id,
        }) {
            return Err(R20FacadeErrorV1::TooLate);
        }
        let retained_before = self.retained_targets.len();
        self.retained_targets
            .retain(|target| *target != transfer_id);
        self.completions
            .retain(|completion| completion.transfer_id != transfer_id);
        if self
            .quiescent_without_result
            .is_some_and(|record| record.transfer_id == transfer_id)
        {
            self.quiescent_without_result = None;
        }
        if retained_before == self.retained_targets.len() {
            return Err(R20FacadeErrorV1::InvalidTransfer);
        }
        Ok(())
    }

    fn finish_packet_model_only(
        &mut self,
        mut adapter: R19DirectionalPersistentLocalSdmaAdapterV1,
        metadata: R20PacketMetadataV1,
        completed: R19DirectionalTransferLeaseV1,
        observation: R20PollObservationV1,
    ) -> Result<R20PollOutcomeV1, R20FacadeErrorV1> {
        let R20PacketMetadataV1 {
            transfer,
            packet_offset,
            packet_byte_len,
            binding,
        } = metadata;
        let status = match observation {
            R20PollObservationV1::Succeeded => R18SdmaTerminalStatusV1::Succeeded,
            R20PollObservationV1::Failed { code } => R18SdmaTerminalStatusV1::Failed { code },
            _ => return Err(R20FacadeErrorV1::LowerInvariant),
        };
        let restored = match completed.restore_model_only(
            &mut adapter,
            R19DirectionalRestoreObservationV1 {
                binding,
                status,
                child_current: true,
            },
        ) {
            Ok(R19DirectionalRestoreOutcomeV1::Restored(lease)) => lease,
            Ok(R19DirectionalRestoreOutcomeV1::Quarantined(lease)) => {
                self.opaque = Some(R20OpaqueCustodyV1::Quarantined { adapter, lease });
                return Err(R20FacadeErrorV1::ProcessTeardown);
            }
            Err(failure) => {
                let (_, lease) = failure.into_parts();
                self.opaque = Some(R20OpaqueCustodyV1::LowerTransition { adapter, lease });
                return Err(R20FacadeErrorV1::ProcessTeardown);
            }
        };
        let frontier = match restored.settle_model_only(
            &mut adapter,
            R19DirectionalSettlementObservationV1 { binding, status },
        ) {
            Ok(frontier) => frontier,
            Err(failure) => {
                let (_, lease) = failure.into_parts();
                self.opaque = Some(R20OpaqueCustodyV1::LowerTransition { adapter, lease });
                return Err(R20FacadeErrorV1::ProcessTeardown);
            }
        };
        let frontier_key = frontier.key();
        if frontier_key.direction != transfer.direction
            || frontier_key.persistent_frontier.through_use != binding.persistent_use.lease
        {
            self.opaque = Some(R20OpaqueCustodyV1::Frontier { adapter, frontier });
            return Err(R20FacadeErrorV1::ProcessTeardown);
        }
        if let Err(failure) = frontier.retire_model_only(&mut adapter, frontier_key) {
            let (_, frontier) = failure.into_parts();
            self.opaque = Some(R20OpaqueCustodyV1::Frontier { adapter, frontier });
            return Err(R20FacadeErrorV1::ProcessTeardown);
        }
        let mut record = transfer;
        if status == R18SdmaTerminalStatusV1::Succeeded {
            record.completed_bytes = record
                .completed_bytes
                .checked_add(packet_byte_len)
                .ok_or(R20FacadeErrorV1::CapacityExceeded)?;
            let destination_base = endpoint_offset_v1(record.request.destination);
            self.destination_dirty.push(R20DestinationDirtyV1 {
                transfer_id: record.request.transfer_id,
                destination: record.request.destination,
                byte_offset: destination_base
                    .checked_add(packet_offset)
                    .ok_or(R20FacadeErrorV1::CapacityExceeded)?,
                byte_len: packet_byte_len,
            });
        }
        self.adapter = Some(adapter);
        if status == R18SdmaTerminalStatusV1::Succeeded
            && record.completed_bytes < record.request.byte_len
        {
            self.active = Some(R20ActiveV1::Ready(Box::new(record)));
            return Ok(R20PollOutcomeV1::ReadyContinuation {
                completed_bytes: record.completed_bytes,
            });
        }
        let completion = R20CompletionRecordV1 {
            transfer_id: record.request.transfer_id,
            succeeded: status == R18SdmaTerminalStatusV1::Succeeded,
            failure_code: match status {
                R18SdmaTerminalStatusV1::Succeeded => None,
                R18SdmaTerminalStatusV1::Failed { code } => Some(code),
            },
            completed_bytes: record.completed_bytes,
        };
        self.dependencies.clear();
        self.completions.push(completion);
        Ok(R20PollOutcomeV1::Completed(completion))
    }

    fn require_operational(&self) -> Result<(), R20FacadeErrorV1> {
        if self.opaque.is_some() || self.adapter.is_none() {
            Err(R20FacadeErrorV1::ProcessTeardown)
        } else {
            Ok(())
        }
    }

    /// Confirms that terminal lower custody is still owned by this facade.
    pub fn terminal_custody_kind(&self) -> Option<&'static str> {
        match self.opaque.as_ref()? {
            R20OpaqueCustodyV1::Quarantined { adapter, lease } => {
                let _ = (adapter.snapshot(), lease.binding());
                Some("quarantined")
            }
            R20OpaqueCustodyV1::LowerTransition { adapter, lease } => {
                let _ = (adapter.snapshot(), lease.binding());
                Some("lower-transition")
            }
            R20OpaqueCustodyV1::Frontier { adapter, frontier } => {
                let _ = (adapter.snapshot(), frontier.key());
                Some("frontier")
            }
        }
    }
}

fn snapshot_record(
    record: R20TransferRecordV1,
    packet: Option<(u64, u64, R18PlannedSdmaTicketV1)>,
) -> R20ActiveTransferSnapshotV1 {
    R20ActiveTransferSnapshotV1 {
        transfer_id: record.request.transfer_id,
        direction: record.direction,
        source: record.request.source,
        destination: record.request.destination,
        byte_len: record.request.byte_len,
        completed_bytes: record.completed_bytes,
        packet_offset: packet.map(|entry| entry.0),
        packet_byte_len: packet.map(|entry| entry.1),
        ticket: packet.map(|entry| entry.2),
    }
}

fn resolve_direction_v1(
    request: R20CopyRequestV1,
) -> Result<
    (
        R18LocalSdmaDirectionV1,
        R18HostBufferKeyV1,
        u64,
        R18NativeAllocationKeyV1,
        u64,
    ),
    R20FacadeErrorV1,
> {
    match (request.source, request.destination) {
        (
            R20CopyEndpointV1::Host {
                buffer,
                offset: host_offset,
            },
            R20CopyEndpointV1::Device {
                allocation,
                offset: device_offset,
            },
        ) => Ok((
            R18LocalSdmaDirectionV1::HostToDevice,
            buffer,
            host_offset,
            allocation,
            device_offset,
        )),
        (
            R20CopyEndpointV1::Device {
                allocation,
                offset: device_offset,
            },
            R20CopyEndpointV1::Host {
                buffer,
                offset: host_offset,
            },
        ) => Ok((
            R18LocalSdmaDirectionV1::DeviceToHost,
            buffer,
            host_offset,
            allocation,
            device_offset,
        )),
        _ => Err(R20FacadeErrorV1::UnsupportedCopy),
    }
}

fn validate_range_v1(offset: u64, byte_len: u64, extent: u64) -> Result<(), R20FacadeErrorV1> {
    if byte_len == 0 || offset.checked_add(byte_len).is_none_or(|end| end > extent) {
        Err(R20FacadeErrorV1::InvalidRange)
    } else {
        Ok(())
    }
}

fn endpoint_offset_v1(endpoint: R20CopyEndpointV1) -> u64 {
    match endpoint {
        R20CopyEndpointV1::Host { offset, .. } | R20CopyEndpointV1::Device { offset, .. } => offset,
    }
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
