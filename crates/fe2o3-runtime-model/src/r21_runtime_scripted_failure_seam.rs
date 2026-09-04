//! Executable R21 model for a scripted directional-SDMA failure seam.
//!
//! The seam reuses R20 request and endpoint types, but independently models
//! facade classification and one move-only native authority. It performs no
//! I/O and is not a refinement of R20, concrete Rust, KFD, native execution,
//! hardware, liveness, HIP/HSA behavior, or performance. The independent
//! Verus artifact is not a refinement of this executable model.

use alloc::{boxed::Box, vec::Vec};

use crate::*;

pub const R21_RUNTIME_SCRIPTED_FAILURE_SEAM_SCHEMA_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21SeamBindingV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
    pub host_storage_id: u64,
    pub host_storage_generation: u64,
}

impl R21SeamBindingV1 {
    pub fn from_idle_r19_snapshot_model_only(
        snapshot: R19DirectionalSnapshotV1,
        host_storage_id: u64,
        host_storage_generation: u64,
    ) -> Result<Self, R21SeamErrorV1> {
        if snapshot.phase.is_some()
            || snapshot.location != R19DirectionalLocationV1::PersistentAllocation
            || snapshot.live_ticket.is_some()
            || snapshot.pending_frontier.is_some()
            || !snapshot.current
            || snapshot.attachment_generation == 0
            || snapshot.pool_generation == 0
            || snapshot.logical_byte_len == 0
            || snapshot.logical_byte_len > snapshot.physical_byte_len
            || host_storage_id == 0
            || host_storage_generation == 0
        {
            return Err(R21SeamErrorV1::InvalidBinding);
        }
        Ok(Self {
            allocation: snapshot.allocation,
            pair: snapshot.pair,
            attachment_generation: snapshot.attachment_generation,
            pool_generation: snapshot.pool_generation,
            logical_byte_len: snapshot.logical_byte_len,
            physical_byte_len: snapshot.physical_byte_len,
            host_storage_id,
            host_storage_generation,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21FacadePhaseV1 {
    HostReady,
    DeviceReady,
    Ready,
    Published,
    TerminalObserved,
    RecyclePending,
    Completed,
    QuiescentWithoutResult,
    DemotedDeviceCleanup,
    Released,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21CustodyKindV1 {
    Host,
    Device,
    Ready,
    Published,
    Terminal,
    Recycle,
    DemotedDevice,
    Opaque,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21FailurePointV1 {
    Promotion,
    Demotion,
    Submission,
    Poll,
    CompletionMetadata,
    Retirement,
    Recycle,
    HiddenCleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21OperationDispositionV1 {
    Succeeded,
    Retryable,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21DemotionDispositionV1 {
    Succeeded,
    RetryableBeforeDemotion,
    RecoveredDemotedNeedsCleanup,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21SubmitDispositionV1 {
    Published,
    DependenciesPending,
    Retryable,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21PollDispositionV1 {
    Pending,
    Retryable,
    TimedOut,
    Terminal(R18SdmaTerminalStatusV1),
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21CompletionMetadataV1 {
    pub transfer_id: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub packet_offset: u64,
    pub packet_byte_len: u64,
    pub ticket_generation: u32,
    pub slot_generation: u64,
    pub pool_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21RetirementKeyV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair_occurrence: u64,
    pub transfer_id: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub packet_offset: u64,
    pub packet_byte_len: u64,
    pub ticket_generation: u32,
    pub slot_generation: u64,
    pub pool_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21RecycleKeyV1 {
    pub pair_occurrence: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub ticket_generation: u32,
    pub slot_generation: u64,
    pub pool_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21TransferSnapshotV1 {
    pub transfer_id: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub source: R20CopyEndpointV1,
    pub destination: R20CopyEndpointV1,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub packet_offset: Option<u64>,
    pub packet_byte_len: Option<u64>,
    pub ticket_generation: Option<u32>,
    pub terminal_status: Option<R18SdmaTerminalStatusV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21CompletionRecordV1 {
    pub transfer_id: u64,
    pub succeeded: bool,
    pub failure_code: Option<i32>,
    pub completed_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R21QuiescentRecordV1 {
    pub transfer_id: u64,
    pub completed_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R21SeamSnapshotV1 {
    pub binding: R21SeamBindingV1,
    pub phase: R21FacadePhaseV1,
    pub custody: Option<R21CustodyKindV1>,
    pub authority_count: u8,
    pub opaque_failure: Option<R21FailurePointV1>,
    pub transfer: Option<R21TransferSnapshotV1>,
    pub completion: Option<R21CompletionRecordV1>,
    pub quiescent: Option<R21QuiescentRecordV1>,
    pub target_retained: bool,
    pub dirty_through: u64,
    pub host_dirty_through: u64,
    pub next_ticket_generation: u32,
    pub slot_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21FacadeClassificationV1 {
    Applied,
    DependencyPending,
    Pending,
    Retryable,
    TimedOut,
    Published(R21CompletionMetadataV1),
    FailedBeforeProgress(R21CompletionRecordV1),
    QuiescentWithoutResult(R21QuiescentRecordV1),
    TerminalObserved(R21RetirementKeyV1),
    RecyclePending(R21RecycleKeyV1),
    ReadyContinuation { completed_bytes: u64 },
    Completed(R21CompletionRecordV1),
    Released,
    ProcessTeardown { point: R21FailurePointV1 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21SeamErrorV1 {
    InvalidBinding,
    InvalidPhase,
    InvalidRequest,
    InvalidObservation,
    InvalidTransfer,
    TargetRetained,
    ProcessTeardown,
    CapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R21ScriptStepV1 {
    Promote(R21OperationDispositionV1),
    Begin(R20CopyRequestV1),
    Submit(R21SubmitDispositionV1),
    Poll {
        disposition: R21PollDispositionV1,
        metadata: Option<R21CompletionMetadataV1>,
    },
    Retire {
        key: R21RetirementKeyV1,
        disposition: R21OperationDispositionV1,
    },
    Recycle {
        key: R21RecycleKeyV1,
        disposition: R21OperationDispositionV1,
    },
    ReleaseSubmission {
        transfer_id: u64,
    },
    Demote(R21DemotionDispositionV1),
    HiddenCleanup(R21OperationDispositionV1),
    ReleaseAllocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R21AuthorityKeyV1 {
    allocation: R18NativeAllocationKeyV1,
    pair_occurrence: u64,
    attachment_generation: u64,
    pool_generation: u64,
    host_storage_id: u64,
    host_storage_generation: u64,
}

struct R21TransferV1 {
    request: R20CopyRequestV1,
    direction: R18LocalSdmaDirectionV1,
    completed_bytes: u64,
    packet_offset: Option<u64>,
    packet_byte_len: Option<u64>,
    ticket_generation: Option<u32>,
    terminal_status: Option<R18SdmaTerminalStatusV1>,
}

enum R21MoveOnlyCustodyV1 {
    Host(R21AuthorityKeyV1),
    Device(R21AuthorityKeyV1),
    Ready(R21AuthorityKeyV1),
    Published(R21AuthorityKeyV1),
    Terminal(R21AuthorityKeyV1),
    Recycle(R21AuthorityKeyV1),
    DemotedDevice(R21AuthorityKeyV1),
    Opaque {
        prior: Box<R21MoveOnlyCustodyV1>,
        point: R21FailurePointV1,
    },
}

impl R21MoveOnlyCustodyV1 {
    const fn kind(&self) -> R21CustodyKindV1 {
        match self {
            Self::Host(_) => R21CustodyKindV1::Host,
            Self::Device(_) => R21CustodyKindV1::Device,
            Self::Ready(_) => R21CustodyKindV1::Ready,
            Self::Published(_) => R21CustodyKindV1::Published,
            Self::Terminal(_) => R21CustodyKindV1::Terminal,
            Self::Recycle(_) => R21CustodyKindV1::Recycle,
            Self::DemotedDevice(_) => R21CustodyKindV1::DemotedDevice,
            Self::Opaque { .. } => R21CustodyKindV1::Opaque,
        }
    }

    const fn key(&self) -> R21AuthorityKeyV1 {
        match self {
            Self::Host(key)
            | Self::Device(key)
            | Self::Ready(key)
            | Self::Published(key)
            | Self::Terminal(key)
            | Self::Recycle(key)
            | Self::DemotedDevice(key) => *key,
            Self::Opaque { prior, .. } => prior.key(),
        }
    }

    const fn opaque_point(&self) -> Option<R21FailurePointV1> {
        match self {
            Self::Opaque { point, .. } => Some(*point),
            _ => None,
        }
    }
}

/// Single-allocation scripted model. The custody enum intentionally has no
/// `Clone` or `Copy` implementation, so transitions must move its sole value.
pub struct R21RuntimeScriptedFailureSeamV1 {
    binding: R21SeamBindingV1,
    phase: R21FacadePhaseV1,
    custody: Option<R21MoveOnlyCustodyV1>,
    transfer: Option<R21TransferV1>,
    completion: Option<R21CompletionRecordV1>,
    quiescent: Option<R21QuiescentRecordV1>,
    target_retained: bool,
    dirty_through: u64,
    host_dirty_through: u64,
    next_ticket_generation: u32,
    slot_generation: u64,
}

impl R21RuntimeScriptedFailureSeamV1 {
    pub fn new_model_only(binding: R21SeamBindingV1) -> Result<Self, R21SeamErrorV1> {
        validate_binding_v1(binding)?;
        let key = authority_key_v1(binding);
        Ok(Self {
            binding,
            phase: R21FacadePhaseV1::HostReady,
            custody: Some(R21MoveOnlyCustodyV1::Host(key)),
            transfer: None,
            completion: None,
            quiescent: None,
            target_retained: false,
            dirty_through: 0,
            host_dirty_through: 0,
            next_ticket_generation: 1,
            slot_generation: 1,
        })
    }

    pub fn snapshot(&self) -> R21SeamSnapshotV1 {
        R21SeamSnapshotV1 {
            binding: self.binding,
            phase: self.phase,
            custody: self.custody.as_ref().map(R21MoveOnlyCustodyV1::kind),
            authority_count: u8::from(self.custody.is_some()),
            opaque_failure: self
                .custody
                .as_ref()
                .and_then(R21MoveOnlyCustodyV1::opaque_point),
            transfer: self.transfer.as_ref().map(transfer_snapshot_v1),
            completion: self.completion,
            quiescent: self.quiescent,
            target_retained: self.target_retained,
            dirty_through: self.dirty_through,
            host_dirty_through: self.host_dirty_through,
            next_ticket_generation: self.next_ticket_generation,
            slot_generation: self.slot_generation,
        }
    }

    pub fn apply_script_step_model_only(
        &mut self,
        step: R21ScriptStepV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        match step {
            R21ScriptStepV1::Promote(disposition) => self.promote_model_only(disposition),
            R21ScriptStepV1::Begin(request) => self.begin_model_only(request),
            R21ScriptStepV1::Submit(disposition) => self.submit_model_only(disposition),
            R21ScriptStepV1::Poll {
                disposition,
                metadata,
            } => self.poll_model_only(disposition, metadata),
            R21ScriptStepV1::Retire { key, disposition } => {
                self.retire_model_only(key, disposition)
            }
            R21ScriptStepV1::Recycle { key, disposition } => {
                self.recycle_model_only(key, disposition)
            }
            R21ScriptStepV1::ReleaseSubmission { transfer_id } => {
                self.release_submission_model_only(transfer_id)
            }
            R21ScriptStepV1::Demote(disposition) => self.demote_model_only(disposition),
            R21ScriptStepV1::HiddenCleanup(disposition) => {
                self.hidden_cleanup_model_only(disposition)
            }
            R21ScriptStepV1::ReleaseAllocation => self.release_allocation_model_only(),
        }
    }

    pub fn run_script_model_only(
        &mut self,
        steps: &[R21ScriptStepV1],
    ) -> Vec<Result<R21FacadeClassificationV1, R21SeamErrorV1>> {
        steps
            .iter()
            .copied()
            .map(|step| self.apply_script_step_model_only(step))
            .collect()
    }

    pub fn promote_model_only(
        &mut self,
        disposition: R21OperationDispositionV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::HostReady)?;
        match disposition {
            R21OperationDispositionV1::Succeeded => {
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Host)?;
                self.custody = Some(R21MoveOnlyCustodyV1::Device(key));
                self.phase = R21FacadePhaseV1::DeviceReady;
                Ok(R21FacadeClassificationV1::Applied)
            }
            R21OperationDispositionV1::Retryable => Ok(R21FacadeClassificationV1::Retryable),
            R21OperationDispositionV1::ProcessTeardown => {
                Ok(self.enter_teardown_v1(R21FailurePointV1::Promotion)?)
            }
        }
    }

    pub fn begin_model_only(
        &mut self,
        request: R20CopyRequestV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::DeviceReady)?;
        if self.target_retained || request.transfer_id == 0 {
            return Err(R21SeamErrorV1::TargetRetained);
        }
        let direction = resolve_request_v1(self.binding, request)?;
        let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Device)?;
        self.custody = Some(R21MoveOnlyCustodyV1::Ready(key));
        self.phase = R21FacadePhaseV1::Ready;
        self.target_retained = true;
        self.dirty_through = 0;
        self.host_dirty_through = 0;
        self.transfer = Some(R21TransferV1 {
            request,
            direction,
            completed_bytes: 0,
            packet_offset: None,
            packet_byte_len: None,
            ticket_generation: None,
            terminal_status: None,
        });
        Ok(R21FacadeClassificationV1::Applied)
    }

    pub fn submit_model_only(
        &mut self,
        disposition: R21SubmitDispositionV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::Ready)?;
        match disposition {
            R21SubmitDispositionV1::DependenciesPending => {
                Ok(R21FacadeClassificationV1::DependencyPending)
            }
            R21SubmitDispositionV1::Retryable => self.settle_retryable_submission_v1(),
            R21SubmitDispositionV1::ProcessTeardown => {
                Ok(self.enter_teardown_v1(R21FailurePointV1::Submission)?)
            }
            R21SubmitDispositionV1::Published => {
                let ticket_generation = self.next_ticket_generation;
                let next = ticket_generation
                    .checked_add(1)
                    .ok_or(R21SeamErrorV1::CapacityExceeded)?;
                let transfer = self
                    .transfer
                    .as_mut()
                    .ok_or(R21SeamErrorV1::InvalidTransfer)?;
                let packet_offset = transfer.completed_bytes;
                let packet_byte_len = transfer
                    .request
                    .byte_len
                    .checked_sub(packet_offset)
                    .ok_or(R21SeamErrorV1::InvalidRequest)?
                    .min(R18_SDMA_MAX_LINEAR_COPY_BYTES_V1);
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Ready)?;
                transfer.packet_offset = Some(packet_offset);
                transfer.packet_byte_len = Some(packet_byte_len);
                transfer.ticket_generation = Some(ticket_generation);
                self.next_ticket_generation = next;
                self.custody = Some(R21MoveOnlyCustodyV1::Published(key));
                self.phase = R21FacadePhaseV1::Published;
                Ok(R21FacadeClassificationV1::Published(
                    completion_metadata_v1(
                        transfer,
                        ticket_generation,
                        self.slot_generation,
                        self.binding.pool_generation,
                    )?,
                ))
            }
        }
    }

    pub fn poll_model_only(
        &mut self,
        disposition: R21PollDispositionV1,
        metadata: Option<R21CompletionMetadataV1>,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::Published)?;
        match disposition {
            R21PollDispositionV1::Pending => {
                reject_spurious_metadata_v1(metadata)?;
                Ok(R21FacadeClassificationV1::Pending)
            }
            R21PollDispositionV1::Retryable => {
                reject_spurious_metadata_v1(metadata)?;
                Ok(R21FacadeClassificationV1::Retryable)
            }
            R21PollDispositionV1::TimedOut => {
                reject_spurious_metadata_v1(metadata)?;
                Ok(R21FacadeClassificationV1::TimedOut)
            }
            R21PollDispositionV1::ProcessTeardown => {
                reject_spurious_metadata_v1(metadata)?;
                Ok(self.enter_teardown_v1(R21FailurePointV1::Poll)?)
            }
            R21PollDispositionV1::Terminal(status) => {
                let observed = metadata.ok_or(R21SeamErrorV1::InvalidObservation)?;
                let transfer = self
                    .transfer
                    .as_ref()
                    .ok_or(R21SeamErrorV1::InvalidTransfer)?;
                let expected = completion_metadata_v1(
                    transfer,
                    transfer
                        .ticket_generation
                        .ok_or(R21SeamErrorV1::InvalidTransfer)?,
                    self.slot_generation,
                    self.binding.pool_generation,
                )?;
                if observed != expected {
                    return self.enter_teardown_v1(R21FailurePointV1::CompletionMetadata);
                }
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Published)?;
                let transfer = self
                    .transfer
                    .as_mut()
                    .ok_or(R21SeamErrorV1::InvalidTransfer)?;
                transfer.terminal_status = Some(status);
                self.custody = Some(R21MoveOnlyCustodyV1::Terminal(key));
                self.phase = R21FacadePhaseV1::TerminalObserved;
                let retirement = retirement_key_v1(self.binding, transfer, self.slot_generation)?;
                Ok(R21FacadeClassificationV1::TerminalObserved(retirement))
            }
        }
    }

    pub fn retire_model_only(
        &mut self,
        observed: R21RetirementKeyV1,
        disposition: R21OperationDispositionV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::TerminalObserved)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R21SeamErrorV1::InvalidTransfer)?;
        let expected = retirement_key_v1(self.binding, transfer, self.slot_generation)?;
        if observed != expected {
            return self.enter_teardown_v1(R21FailurePointV1::Retirement);
        }
        match disposition {
            R21OperationDispositionV1::Retryable => Ok(R21FacadeClassificationV1::Retryable),
            R21OperationDispositionV1::ProcessTeardown => {
                Ok(self.enter_teardown_v1(R21FailurePointV1::Retirement)?)
            }
            R21OperationDispositionV1::Succeeded => {
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Terminal)?;
                let transfer = self
                    .transfer
                    .as_mut()
                    .ok_or(R21SeamErrorV1::InvalidTransfer)?;
                if transfer.terminal_status == Some(R18SdmaTerminalStatusV1::Succeeded) {
                    transfer.completed_bytes = transfer
                        .completed_bytes
                        .checked_add(
                            transfer
                                .packet_byte_len
                                .ok_or(R21SeamErrorV1::InvalidTransfer)?,
                        )
                        .ok_or(R21SeamErrorV1::CapacityExceeded)?;
                    self.dirty_through = transfer.completed_bytes;
                    if transfer.direction == R18LocalSdmaDirectionV1::DeviceToHost {
                        self.host_dirty_through = transfer.completed_bytes;
                    }
                }
                self.custody = Some(R21MoveOnlyCustodyV1::Recycle(key));
                self.phase = R21FacadePhaseV1::RecyclePending;
                let recycle = recycle_key_v1(self.binding, transfer, self.slot_generation)?;
                Ok(R21FacadeClassificationV1::RecyclePending(recycle))
            }
        }
    }

    pub fn recycle_model_only(
        &mut self,
        observed: R21RecycleKeyV1,
        disposition: R21OperationDispositionV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::RecyclePending)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R21SeamErrorV1::InvalidTransfer)?;
        let expected = recycle_key_v1(self.binding, transfer, self.slot_generation)?;
        if observed != expected {
            return self.enter_teardown_v1(R21FailurePointV1::Recycle);
        }
        match disposition {
            R21OperationDispositionV1::Retryable => Ok(R21FacadeClassificationV1::Retryable),
            R21OperationDispositionV1::ProcessTeardown => {
                Ok(self.enter_teardown_v1(R21FailurePointV1::Recycle)?)
            }
            R21OperationDispositionV1::Succeeded => {
                let next_slot_generation = self
                    .slot_generation
                    .checked_add(1)
                    .ok_or(R21SeamErrorV1::CapacityExceeded)?;
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Recycle)?;
                self.slot_generation = next_slot_generation;
                let transfer = self
                    .transfer
                    .as_mut()
                    .ok_or(R21SeamErrorV1::InvalidTransfer)?;
                let status = transfer
                    .terminal_status
                    .ok_or(R21SeamErrorV1::InvalidTransfer)?;
                transfer.packet_offset = None;
                transfer.packet_byte_len = None;
                transfer.ticket_generation = None;
                transfer.terminal_status = None;
                if status == R18SdmaTerminalStatusV1::Succeeded
                    && transfer.completed_bytes < transfer.request.byte_len
                {
                    self.custody = Some(R21MoveOnlyCustodyV1::Ready(key));
                    self.phase = R21FacadePhaseV1::Ready;
                    return Ok(R21FacadeClassificationV1::ReadyContinuation {
                        completed_bytes: transfer.completed_bytes,
                    });
                }
                let completion = R21CompletionRecordV1 {
                    transfer_id: transfer.request.transfer_id,
                    succeeded: status == R18SdmaTerminalStatusV1::Succeeded,
                    failure_code: match status {
                        R18SdmaTerminalStatusV1::Succeeded => None,
                        R18SdmaTerminalStatusV1::Failed { code } => Some(code),
                    },
                    completed_bytes: transfer.completed_bytes,
                };
                self.completion = Some(completion);
                self.custody = Some(R21MoveOnlyCustodyV1::Device(key));
                self.phase = R21FacadePhaseV1::Completed;
                Ok(R21FacadeClassificationV1::Completed(completion))
            }
        }
    }

    pub fn release_submission_model_only(
        &mut self,
        transfer_id: u64,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        if !matches!(
            self.phase,
            R21FacadePhaseV1::Completed | R21FacadePhaseV1::QuiescentWithoutResult
        ) || !self.target_retained
            || self
                .transfer
                .as_ref()
                .is_none_or(|transfer| transfer.request.transfer_id != transfer_id)
        {
            return Err(R21SeamErrorV1::InvalidTransfer);
        }
        if self.custody.as_ref().map(R21MoveOnlyCustodyV1::kind) != Some(R21CustodyKindV1::Device) {
            return Err(R21SeamErrorV1::InvalidPhase);
        }
        self.transfer = None;
        self.completion = None;
        self.quiescent = None;
        self.target_retained = false;
        self.phase = R21FacadePhaseV1::DeviceReady;
        Ok(R21FacadeClassificationV1::Applied)
    }

    pub fn demote_model_only(
        &mut self,
        disposition: R21DemotionDispositionV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::DeviceReady)?;
        if self.target_retained {
            return Err(R21SeamErrorV1::TargetRetained);
        }
        match disposition {
            R21DemotionDispositionV1::Succeeded => {
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Device)?;
                self.custody = Some(R21MoveOnlyCustodyV1::Host(key));
                self.phase = R21FacadePhaseV1::HostReady;
                Ok(R21FacadeClassificationV1::Applied)
            }
            R21DemotionDispositionV1::RetryableBeforeDemotion => {
                Ok(R21FacadeClassificationV1::Retryable)
            }
            R21DemotionDispositionV1::RecoveredDemotedNeedsCleanup => {
                let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Device)?;
                self.custody = Some(R21MoveOnlyCustodyV1::DemotedDevice(key));
                self.phase = R21FacadePhaseV1::DemotedDeviceCleanup;
                Ok(R21FacadeClassificationV1::Retryable)
            }
            R21DemotionDispositionV1::ProcessTeardown => {
                Ok(self.enter_teardown_v1(R21FailurePointV1::Demotion)?)
            }
        }
    }

    pub fn hidden_cleanup_model_only(
        &mut self,
        disposition: R21OperationDispositionV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::DemotedDeviceCleanup)?;
        match disposition {
            R21OperationDispositionV1::Retryable => Ok(R21FacadeClassificationV1::Retryable),
            R21OperationDispositionV1::ProcessTeardown => {
                Ok(self.enter_teardown_v1(R21FailurePointV1::HiddenCleanup)?)
            }
            R21OperationDispositionV1::Succeeded => {
                let key =
                    take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::DemotedDevice)?;
                self.custody = Some(R21MoveOnlyCustodyV1::Host(key));
                self.phase = R21FacadePhaseV1::HostReady;
                Ok(R21FacadeClassificationV1::Applied)
            }
        }
    }

    pub fn release_allocation_model_only(
        &mut self,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        self.require_operational()?;
        self.require_phase(R21FacadePhaseV1::HostReady)?;
        if self.target_retained {
            return Err(R21SeamErrorV1::TargetRetained);
        }
        let _key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Host)?;
        self.phase = R21FacadePhaseV1::Released;
        Ok(R21FacadeClassificationV1::Released)
    }

    fn settle_retryable_submission_v1(
        &mut self,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        let key = take_matching_custody_v1(&mut self.custody, R21CustodyKindV1::Ready)?;
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(R21SeamErrorV1::InvalidTransfer)?;
        self.custody = Some(R21MoveOnlyCustodyV1::Device(key));
        if transfer.completed_bytes == 0 {
            let completion = R21CompletionRecordV1 {
                transfer_id: transfer.request.transfer_id,
                succeeded: false,
                failure_code: Some(-1),
                completed_bytes: 0,
            };
            self.completion = Some(completion);
            self.phase = R21FacadePhaseV1::Completed;
            Ok(R21FacadeClassificationV1::FailedBeforeProgress(completion))
        } else {
            let quiescent = R21QuiescentRecordV1 {
                transfer_id: transfer.request.transfer_id,
                completed_bytes: transfer.completed_bytes,
                total_bytes: transfer.request.byte_len,
            };
            self.quiescent = Some(quiescent);
            self.phase = R21FacadePhaseV1::QuiescentWithoutResult;
            Ok(R21FacadeClassificationV1::QuiescentWithoutResult(quiescent))
        }
    }

    fn enter_teardown_v1(
        &mut self,
        point: R21FailurePointV1,
    ) -> Result<R21FacadeClassificationV1, R21SeamErrorV1> {
        let prior = self.custody.take().ok_or(R21SeamErrorV1::InvalidPhase)?;
        self.custody = Some(R21MoveOnlyCustodyV1::Opaque {
            prior: Box::new(prior),
            point,
        });
        self.phase = R21FacadePhaseV1::ProcessTeardown;
        Ok(R21FacadeClassificationV1::ProcessTeardown { point })
    }

    fn require_operational(&self) -> Result<(), R21SeamErrorV1> {
        if self.phase == R21FacadePhaseV1::ProcessTeardown {
            Err(R21SeamErrorV1::ProcessTeardown)
        } else {
            Ok(())
        }
    }

    fn require_phase(&self, expected: R21FacadePhaseV1) -> Result<(), R21SeamErrorV1> {
        if self.phase == expected {
            Ok(())
        } else {
            Err(R21SeamErrorV1::InvalidPhase)
        }
    }
}

fn validate_binding_v1(binding: R21SeamBindingV1) -> Result<(), R21SeamErrorV1> {
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
        return Err(R21SeamErrorV1::InvalidBinding);
    }
    Ok(())
}

const fn authority_key_v1(binding: R21SeamBindingV1) -> R21AuthorityKeyV1 {
    R21AuthorityKeyV1 {
        allocation: binding.allocation,
        pair_occurrence: binding.pair.pair_occurrence,
        attachment_generation: binding.attachment_generation,
        pool_generation: binding.pool_generation,
        host_storage_id: binding.host_storage_id,
        host_storage_generation: binding.host_storage_generation,
    }
}

fn take_matching_custody_v1(
    custody: &mut Option<R21MoveOnlyCustodyV1>,
    expected: R21CustodyKindV1,
) -> Result<R21AuthorityKeyV1, R21SeamErrorV1> {
    let owner = custody.take().ok_or(R21SeamErrorV1::InvalidPhase)?;
    if owner.kind() != expected {
        *custody = Some(owner);
        return Err(R21SeamErrorV1::InvalidPhase);
    }
    Ok(owner.key())
}

fn resolve_request_v1(
    binding: R21SeamBindingV1,
    request: R20CopyRequestV1,
) -> Result<R18LocalSdmaDirectionV1, R21SeamErrorV1> {
    if request.byte_len == 0 || request.byte_len > binding.logical_byte_len {
        return Err(R21SeamErrorV1::InvalidRequest);
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
            _ => return Err(R21SeamErrorV1::InvalidRequest),
        };
    if allocation != binding.allocation
        || host_offset
            .checked_add(request.byte_len)
            .is_none_or(|end| end > host.byte_len)
        || device_offset
            .checked_add(request.byte_len)
            .is_none_or(|end| end > binding.logical_byte_len)
    {
        return Err(R21SeamErrorV1::InvalidRequest);
    }
    Ok(direction)
}

fn transfer_snapshot_v1(transfer: &R21TransferV1) -> R21TransferSnapshotV1 {
    R21TransferSnapshotV1 {
        transfer_id: transfer.request.transfer_id,
        direction: transfer.direction,
        source: transfer.request.source,
        destination: transfer.request.destination,
        total_bytes: transfer.request.byte_len,
        completed_bytes: transfer.completed_bytes,
        packet_offset: transfer.packet_offset,
        packet_byte_len: transfer.packet_byte_len,
        ticket_generation: transfer.ticket_generation,
        terminal_status: transfer.terminal_status,
    }
}

fn completion_metadata_v1(
    transfer: &R21TransferV1,
    ticket_generation: u32,
    slot_generation: u64,
    pool_generation: u64,
) -> Result<R21CompletionMetadataV1, R21SeamErrorV1> {
    Ok(R21CompletionMetadataV1 {
        transfer_id: transfer.request.transfer_id,
        direction: transfer.direction,
        packet_offset: transfer
            .packet_offset
            .ok_or(R21SeamErrorV1::InvalidTransfer)?,
        packet_byte_len: transfer
            .packet_byte_len
            .ok_or(R21SeamErrorV1::InvalidTransfer)?,
        ticket_generation,
        slot_generation,
        pool_generation,
    })
}

fn retirement_key_v1(
    binding: R21SeamBindingV1,
    transfer: &R21TransferV1,
    slot_generation: u64,
) -> Result<R21RetirementKeyV1, R21SeamErrorV1> {
    let metadata = completion_metadata_v1(
        transfer,
        transfer
            .ticket_generation
            .ok_or(R21SeamErrorV1::InvalidTransfer)?,
        slot_generation,
        binding.pool_generation,
    )?;
    Ok(R21RetirementKeyV1 {
        allocation: binding.allocation,
        pair_occurrence: binding.pair.pair_occurrence,
        transfer_id: metadata.transfer_id,
        direction: metadata.direction,
        packet_offset: metadata.packet_offset,
        packet_byte_len: metadata.packet_byte_len,
        ticket_generation: metadata.ticket_generation,
        slot_generation: metadata.slot_generation,
        pool_generation: metadata.pool_generation,
    })
}

fn recycle_key_v1(
    binding: R21SeamBindingV1,
    transfer: &R21TransferV1,
    slot_generation: u64,
) -> Result<R21RecycleKeyV1, R21SeamErrorV1> {
    Ok(R21RecycleKeyV1 {
        pair_occurrence: binding.pair.pair_occurrence,
        direction: transfer.direction,
        ticket_generation: transfer
            .ticket_generation
            .ok_or(R21SeamErrorV1::InvalidTransfer)?,
        slot_generation,
        pool_generation: binding.pool_generation,
    })
}

fn reject_spurious_metadata_v1(
    metadata: Option<R21CompletionMetadataV1>,
) -> Result<(), R21SeamErrorV1> {
    if metadata.is_none() {
        Ok(())
    } else {
        Err(R21SeamErrorV1::InvalidObservation)
    }
}
