#![cfg(test)]

use super::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScriptedFailureModeV1 {
    Success,
    Retryable,
    ProcessTeardown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScriptedExecutionOutcomeV1 {
    Pending,
    Completed {
        direction: Option<Gfx942PersistentSdmaDirectionV1>,
        copy_bytes: Option<u32>,
    },
    CompletedWindow {
        direction: Option<Gfx942PersistentSdmaDirectionV1>,
        copy_bytes: Option<u32>,
        requests: Option<Vec<DirectionalSdmaCopyRequestV1>>,
    },
    Retryable,
    ProcessTeardown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScriptedSameDeviceExecutionOutcomeV1 {
    Pending,
    Completed {
        copy_bytes: Option<u32>,
        requests: Option<Vec<SameDeviceSdmaCopyRequestV1>>,
        swap_allocations: bool,
    },
    Retryable,
    ProcessTeardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScriptedRecycleOutcomeV1 {
    Success,
    Recovered,
    Ambiguous,
    Panic,
}

#[derive(Debug)]
pub(crate) enum ScriptedSdmaStepV1 {
    Allocate {
        kind: ScriptedBufferKindV1,
        byte_len: usize,
    },
    AllocateHostFilled {
        byte_len: usize,
        fill: u8,
    },
    AllocateFailure {
        kind: ScriptedBufferKindV1,
        byte_len: usize,
        failure: SdmaAllocationFailureV1,
    },
    AllocatePanic {
        kind: ScriptedBufferKindV1,
        byte_len: usize,
    },
    Write {
        offset: u64,
        byte_len: usize,
    },
    WriteFault {
        offset: u64,
        byte_len: usize,
        written_prefix: usize,
        panic: bool,
    },
    Read {
        offset: u64,
        byte_len: u64,
    },
    ReadFault {
        offset: u64,
        byte_len: u64,
        panic: bool,
    },
    ReadPartialFault {
        offset: u64,
        byte_len: u64,
        copied_prefix: usize,
        panic: bool,
    },
    ReadLength {
        offset: u64,
        byte_len: u64,
        returned_len: usize,
    },
    Promote(ScriptedFailureModeV1),
    PromotePanic,
    PromoteInitializedStorage(ScriptedFailureModeV1),
    InitializedStorageNotEligible(Gfx942PersistentComputeStorageIneligibilityV1),
    InitializedStoragePanic,
    PromoteComputeReady(ScriptedFailureModeV1),
    PromoteComputeReadyForeignQueue,
    PromoteComputeReadyForeignQueueTerminal,
    Demote(ScriptedFailureModeV1),
    DemotePanic,
    Submit {
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        outcome: ScriptedFailureModeV1,
    },
    SubmitWindow {
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: Vec<DirectionalSdmaCopyRequestV1>,
        outcome: ScriptedFailureModeV1,
    },
    SubmitSameDeviceWindow {
        requests: Vec<SameDeviceSdmaCopyRequestV1>,
        outcome: ScriptedFailureModeV1,
    },
    SubmitPanic,
    SubmitSameDevicePanic,
    Poll(ScriptedExecutionOutcomeV1),
    Wait(ScriptedExecutionOutcomeV1),
    Retire(ScriptedFailureModeV1),
    RetirePanic,
    RetireCompletedRetry,
    PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1),
    WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1),
    RetireSameDevice(ScriptedFailureModeV1),
    Recycle(ScriptedRecycleOutcomeV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScriptedBufferKindV1 {
    Host,
    Device,
    PublicDevice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScriptedOwnerRoleV1 {
    Buffer(ScriptedBufferKindV1),
    Device,
}

#[derive(Debug, Default)]
struct ScriptedCustodyLedgerV1 {
    next_id: u64,
    owners: HashMap<u64, ScriptedOwnerRoleV1>,
    unexpected_drops: usize,
}

#[derive(Debug)]
struct ScriptedOwnerTokenV1 {
    id: u64,
    role: ScriptedOwnerRoleV1,
    ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>,
    armed: bool,
}

impl ScriptedOwnerTokenV1 {
    fn new(role: ScriptedOwnerRoleV1, ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>) -> Self {
        let id = {
            let mut ledger = ledger.borrow_mut();
            ledger.next_id += 1;
            let id = ledger.next_id;
            assert!(ledger.owners.insert(id, role).is_none());
            id
        };
        Self {
            id,
            role,
            ledger,
            armed: true,
        }
    }

    fn transition(mut self, role: ScriptedOwnerRoleV1) -> Self {
        let prior = self.ledger.borrow_mut().owners.insert(self.id, role);
        assert_eq!(prior, Some(self.role));
        self.role = role;
        self
    }

    fn release(mut self) {
        let prior = self.ledger.borrow_mut().owners.remove(&self.id);
        assert_eq!(prior, Some(self.role));
        self.armed = false;
    }
}

impl Drop for ScriptedOwnerTokenV1 {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.ledger.borrow_mut().owners.remove(&self.id);
            self.ledger.borrow_mut().unexpected_drops += 1;
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScriptedBufferOwnerV1 {
    token: ScriptedOwnerTokenV1,
    kind: ScriptedBufferKindV1,
    pub(super) bytes: Vec<u8>,
    full_content_certificate: Option<ScriptedHostContentCertificateV1>,
}

#[derive(Debug, Eq, PartialEq)]
struct ScriptedHostContentCertificateV1 {
    owner_id: u64,
    byte_len: usize,
    sha256: [u8; 32],
}

impl ScriptedBufferOwnerV1 {
    pub(crate) fn observation(&self) -> (u64, &[u8], Option<[u8; 32]>) {
        (
            self.token.id,
            &self.bytes,
            self.full_content_certificate
                .as_ref()
                .map(|certificate| certificate.sha256),
        )
    }
}

#[derive(Debug)]
pub(crate) struct ScriptedDeviceOwnerV1 {
    token: ScriptedOwnerTokenV1,
    pub(super) bytes: Vec<u8>,
}

impl ScriptedDeviceOwnerV1 {
    pub(super) const fn owner_id(&self) -> u64 {
        self.token.id
    }
}

#[derive(Debug)]
pub(crate) struct ScriptedSubmissionOwnerV1 {
    pair: DirectionalSdmaPairOwnerV1,
    direction: Gfx942PersistentSdmaDirectionV1,
    requests: DirectionalSdmaRequestPlanV1,
    copy_bytes: u32,
}

#[derive(Debug)]
pub(crate) struct ScriptedCompletedOwnerV1 {
    pair: DirectionalSdmaPairOwnerV1,
    pub(super) direction: Gfx942PersistentSdmaDirectionV1,
    pub(super) host_offset: u64,
    pub(super) device_offset: u64,
    pub(super) copy_bytes: u32,
    pub(super) packet_count: usize,
}

impl ScriptedSubmissionOwnerV1 {
    pub(crate) fn pair(&self) -> &DirectionalSdmaPairOwnerV1 {
        &self.pair
    }

    pub(crate) fn requests(&self) -> &[DirectionalSdmaCopyRequestV1] {
        self.requests.as_slice()
    }
}

impl ScriptedCompletedOwnerV1 {
    pub(crate) fn pair(&self) -> &DirectionalSdmaPairOwnerV1 {
        &self.pair
    }
}

pub(super) enum ScriptedPersistentComputeReadyFailureV1 {
    Recovered(DirectionalSdmaPairOwnerV1),
    ForeignQueue {
        completed: ScriptedCompletedOwnerV1,
        terminal_receiver: bool,
    },
    ProcessTeardown(ScriptedCompletedOwnerV1),
}

#[derive(Debug)]
pub(crate) struct ScriptedSameDeviceSubmissionOwnerV1 {
    pair: SameDeviceSdmaPairOwnerV1,
    requests: Box<[SameDeviceSdmaCopyRequestV1]>,
    copy_bytes: u32,
    source_owner_id: u64,
    destination_owner_id: u64,
}

impl ScriptedSameDeviceSubmissionOwnerV1 {
    pub(crate) fn pair(&self) -> &SameDeviceSdmaPairOwnerV1 {
        &self.pair
    }
}

#[derive(Debug)]
pub(crate) struct ScriptedSameDeviceCompletedOwnerV1 {
    pair: SameDeviceSdmaPairOwnerV1,
    pub(super) source_offset: u64,
    pub(super) destination_offset: u64,
    pub(super) copy_bytes: u32,
    pub(super) packet_count: usize,
}

#[allow(dead_code)]
pub(crate) enum ScriptedTerminalCustodyV1 {
    Buffer(SdmaBufferOwnerV1),
    Device(DirectionalSdmaDeviceOwnerV1),
    Pair(DirectionalSdmaPairOwnerV1),
    Submission(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
    SameDevicePair(SameDeviceSdmaPairOwnerV1),
    SameDeviceSubmission(SameDeviceSdmaSubmissionOwnerV1),
    SameDeviceCompleted(SameDeviceSdmaCompletedOwnerV1),
}

pub(crate) struct ScriptedSdmaDriverV1 {
    steps: VecDeque<ScriptedSdmaStepV1>,
    #[cfg(feature = "hardware-diagnostic")]
    pub(crate) wait_diagnostics: VecDeque<fe2o3_kfd::Gfx942SdmaPersistentWaitDiagnosticsV1>,
    ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>,
    promotion_custody: Option<ScriptedBufferOwnerV1>,
    demotion_custody: Option<ScriptedDeviceOwnerV1>,
    storage_conversion_custody: Option<ScriptedDeviceOwnerV1>,
    recycle_custody: Option<ScriptedBufferOwnerV1>,
    wait_custody: Option<ScriptedSubmissionOwnerV1>,
    retirement_custody: Option<ScriptedCompletedOwnerV1>,
    publication_custody: Option<ScriptedTerminalCustodyV1>,
    pub(crate) publication_byte_limit: Option<u32>,
}

impl core::fmt::Debug for ScriptedSdmaDriverV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ScriptedSdmaDriverV1")
            .field("steps", &self.steps)
            .field("owners", &self.ledger.borrow().owners)
            .field("unexpected_drops", &self.ledger.borrow().unexpected_drops)
            .finish()
    }
}

impl ScriptedSdmaDriverV1 {
    pub(crate) fn new(steps: impl IntoIterator<Item = ScriptedSdmaStepV1>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            #[cfg(feature = "hardware-diagnostic")]
            wait_diagnostics: VecDeque::new(),
            ledger: Rc::new(RefCell::new(ScriptedCustodyLedgerV1::default())),
            promotion_custody: None,
            demotion_custody: None,
            storage_conversion_custody: None,
            recycle_custody: None,
            wait_custody: None,
            retirement_custody: None,
            publication_custody: None,
            publication_byte_limit: None,
        }
    }

    fn pop(&mut self) -> Result<ScriptedSdmaStepV1, String> {
        self.steps
            .pop_front()
            .ok_or_else(|| "scripted directional SDMA operation was not expected".to_owned())
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.steps.is_empty()
    }

    pub(crate) fn remaining_steps(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn promotion_custody(&self) -> Option<&ScriptedBufferOwnerV1> {
        self.promotion_custody.as_ref()
    }

    pub(crate) fn storage_conversion_custody(&self) -> Option<u64> {
        self.storage_conversion_custody
            .as_ref()
            .map(ScriptedDeviceOwnerV1::owner_id)
    }

    pub(crate) fn recycle_custody(&self) -> Option<&ScriptedBufferOwnerV1> {
        self.recycle_custody.as_ref()
    }

    pub(crate) fn wait_custody(&self) -> Option<&ScriptedSubmissionOwnerV1> {
        self.wait_custody.as_ref()
    }

    pub(crate) fn retirement_custody(&self) -> Option<&ScriptedCompletedOwnerV1> {
        self.retirement_custody.as_ref()
    }

    pub(crate) fn publication_custody(&self) -> Option<&ScriptedTerminalCustodyV1> {
        self.publication_custody.as_ref()
    }

    fn retain_publication_and_panic(&mut self, custody: ScriptedTerminalCustodyV1) -> ! {
        if self.publication_custody.is_some() {
            std::process::abort();
        }
        self.publication_custody = Some(custody);
        std::panic::panic_any("scripted SDMA publication panic");
    }

    pub(crate) fn demotion_custody(&self) -> Option<(u64, &[u8])> {
        self.demotion_custody
            .as_ref()
            .map(|device| (device.token.id, device.bytes.as_slice()))
    }

    pub(crate) fn live_owner_count(&self) -> usize {
        self.ledger.borrow().owners.len()
    }

    pub(crate) fn unexpected_drops(&self) -> usize {
        self.ledger.borrow().unexpected_drops
    }

    pub(crate) fn test_host_owner(&self, byte_len: usize) -> SdmaBufferOwnerV1 {
        SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
            token: ScriptedOwnerTokenV1::new(
                ScriptedOwnerRoleV1::Buffer(ScriptedBufferKindV1::Host),
                Rc::clone(&self.ledger),
            ),
            kind: ScriptedBufferKindV1::Host,
            bytes: vec![0; byte_len],
            full_content_certificate: None,
        })
    }

    pub(crate) fn test_device_owner(&self, byte_len: usize) -> DirectionalSdmaDeviceOwnerV1 {
        DirectionalSdmaDeviceOwnerV1::Scripted(ScriptedDeviceOwnerV1 {
            token: ScriptedOwnerTokenV1::new(ScriptedOwnerRoleV1::Device, Rc::clone(&self.ledger)),
            bytes: vec![0; byte_len],
        })
    }

    fn owns_token(&self, token: &ScriptedOwnerTokenV1) -> bool {
        Rc::ptr_eq(&self.ledger, &token.ledger)
    }

    fn owns_buffer(&self, buffer: &ScriptedBufferOwnerV1) -> bool {
        self.owns_token(&buffer.token)
    }

    fn owns_device(&self, device: &ScriptedDeviceOwnerV1) -> bool {
        self.owns_token(&device.token)
    }

    fn owns_pair(&self, pair: &DirectionalSdmaPairOwnerV1) -> bool {
        matches!(
            (&pair.device, &pair.host),
            (
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                SdmaBufferOwnerV1::Scripted(host)
            ) if self.owns_device(device) && self.owns_buffer(host)
        )
    }

    fn owns_submission(&self, submission: &ScriptedSubmissionOwnerV1) -> bool {
        self.owns_pair(&submission.pair)
    }

    fn owns_completed(&self, completed: &ScriptedCompletedOwnerV1) -> bool {
        self.owns_pair(&completed.pair)
    }

    fn same_device_owner_ids(pair: &SameDeviceSdmaPairOwnerV1) -> Option<(u64, u64)> {
        match (&pair.source, &pair.destination) {
            (
                DirectionalSdmaDeviceOwnerV1::Scripted(source),
                DirectionalSdmaDeviceOwnerV1::Scripted(destination),
            ) => Some((source.token.id, destination.token.id)),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    fn owns_same_device_pair(&self, pair: &SameDeviceSdmaPairOwnerV1) -> bool {
        match (&pair.source, &pair.destination) {
            (
                DirectionalSdmaDeviceOwnerV1::Scripted(source),
                DirectionalSdmaDeviceOwnerV1::Scripted(destination),
            ) => self.owns_device(source) && self.owns_device(destination),
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }

    fn owns_same_device_submission(
        &self,
        submission: &ScriptedSameDeviceSubmissionOwnerV1,
    ) -> bool {
        self.owns_same_device_pair(&submission.pair)
            && Self::same_device_owner_ids(&submission.pair)
                == Some((submission.source_owner_id, submission.destination_owner_id))
    }

    fn owns_same_device_completed(&self, completed: &ScriptedSameDeviceCompletedOwnerV1) -> bool {
        self.owns_same_device_pair(&completed.pair)
    }
}

fn execute_outcome(
    submission: ScriptedSubmissionOwnerV1,
    outcome: ScriptedExecutionOutcomeV1,
    operation: &'static str,
) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
    execute_outcome_in_place(&mut Some(submission), outcome, operation)
}

fn execute_outcome_in_place(
    slot: &mut Option<ScriptedSubmissionOwnerV1>,
    outcome: ScriptedExecutionOutcomeV1,
    operation: &'static str,
) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
    match outcome {
        ScriptedExecutionOutcomeV1::Pending => Ok(DirectionalSdmaPollV1::Pending(
            DirectionalSdmaSubmissionOwnerV1::Scripted(
                slot.take().expect("retained scripted submission"),
            ),
        )),
        ScriptedExecutionOutcomeV1::Retryable => {
            Err(DirectionalSdmaExecutionFailureV1::Retryable {
                detail: format!("scripted {operation} retryable"),
                submission: DirectionalSdmaSubmissionOwnerV1::Scripted(
                    slot.take().expect("retained scripted submission"),
                ),
            })
        }
        ScriptedExecutionOutcomeV1::ProcessTeardown => {
            Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                detail: format!("scripted {operation} teardown"),
                custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(
                    DirectionalSdmaSubmissionOwnerV1::Scripted(
                        slot.take().expect("retained scripted submission"),
                    ),
                )),
            })
        }
        ScriptedExecutionOutcomeV1::Completed {
            direction,
            copy_bytes,
        } => complete_scripted_submission(slot, direction, copy_bytes, None),
        ScriptedExecutionOutcomeV1::CompletedWindow {
            direction,
            copy_bytes,
            requests,
        } => complete_scripted_submission(slot, direction, copy_bytes, requests),
    }
}

fn complete_scripted_submission(
    slot: &mut Option<ScriptedSubmissionOwnerV1>,
    direction: Option<Gfx942PersistentSdmaDirectionV1>,
    copy_bytes: Option<u32>,
    reported_requests: Option<Vec<DirectionalSdmaCopyRequestV1>>,
) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
    let submission = slot.as_mut().expect("retained scripted submission");
    for request in submission.requests.as_slice() {
        let len = usize::try_from(request.copy_bytes).expect("u32 fits usize");
        let host_start =
            usize::try_from(request.host_offset).expect("admitted scripted host offset fits usize");
        let device_start = usize::try_from(request.device_offset)
            .expect("admitted scripted device offset fits usize");
        let (device, host) = match (&mut submission.pair.device, &mut submission.pair.host) {
            (DirectionalSdmaDeviceOwnerV1::Scripted(device), SdmaBufferOwnerV1::Scripted(host)) => {
                (device, host)
            }
            _ => unreachable!("scripted submission retains scripted pair"),
        };
        match submission.direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => device.bytes
                [device_start..device_start + len]
                .copy_from_slice(&host.bytes[host_start..host_start + len]),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => host.bytes
                [host_start..host_start + len]
                .copy_from_slice(&device.bytes[device_start..device_start + len]),
        }
    }
    let packet_count = reported_requests
        .as_ref()
        .map_or_else(|| submission.requests.packet_count(), Vec::len);
    let (host_offset, device_offset) = reported_requests
        .as_deref()
        .unwrap_or_else(|| submission.requests.as_slice())
        .first()
        .map(|request| (request.host_offset, request.device_offset))
        .unwrap_or((0, 0));
    let submission = slot
        .take()
        .expect("completed copy retains original submission");
    Ok(DirectionalSdmaPollV1::Completed(
        DirectionalSdmaCompletedOwnerV1::Scripted(ScriptedCompletedOwnerV1 {
            pair: submission.pair,
            direction: direction.unwrap_or(submission.direction),
            host_offset,
            device_offset,
            copy_bytes: copy_bytes.unwrap_or(submission.copy_bytes),
            packet_count,
        }),
    ))
}

fn execute_same_device_outcome(
    submission: ScriptedSameDeviceSubmissionOwnerV1,
    outcome: ScriptedSameDeviceExecutionOutcomeV1,
    operation: &'static str,
) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
    match outcome {
        ScriptedSameDeviceExecutionOutcomeV1::Pending => Ok(SameDeviceSdmaPollV1::Pending(
            SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
        )),
        ScriptedSameDeviceExecutionOutcomeV1::Retryable => {
            Err(SameDeviceSdmaExecutionFailureV1::Retryable {
                detail: format!("scripted same-device {operation} retryable"),
                submission: SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
            })
        }
        ScriptedSameDeviceExecutionOutcomeV1::ProcessTeardown => {
            Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                detail: format!("scripted same-device {operation} teardown"),
                custody: SdmaTerminalCustodyV1::Scripted(
                    ScriptedTerminalCustodyV1::SameDeviceSubmission(
                        SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
                    ),
                ),
            })
        }
        ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes,
            requests,
            swap_allocations,
        } => complete_scripted_same_device_submission(
            submission,
            copy_bytes,
            requests,
            swap_allocations,
        ),
    }
}

fn complete_scripted_same_device_submission(
    mut submission: ScriptedSameDeviceSubmissionOwnerV1,
    copy_bytes: Option<u32>,
    reported_requests: Option<Vec<SameDeviceSdmaCopyRequestV1>>,
    swap_allocations: bool,
) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
    for request in submission.requests.iter() {
        let len = usize::try_from(request.copy_bytes).expect("u32 fits usize");
        let source_start = usize::try_from(request.source_offset)
            .expect("admitted scripted source offset fits usize");
        let destination_start = usize::try_from(request.destination_offset)
            .expect("admitted scripted destination offset fits usize");
        let (
            DirectionalSdmaDeviceOwnerV1::Scripted(source),
            DirectionalSdmaDeviceOwnerV1::Scripted(destination),
        ) = (&submission.pair.source, &mut submission.pair.destination)
        else {
            unreachable!("scripted same-device submission retains scripted owners")
        };
        destination.bytes[destination_start..destination_start + len]
            .copy_from_slice(&source.bytes[source_start..source_start + len]);
    }
    if swap_allocations {
        core::mem::swap(
            &mut submission.pair.source,
            &mut submission.pair.destination,
        );
    }
    let reported_requests = reported_requests
        .map(Vec::into_boxed_slice)
        .unwrap_or_else(|| submission.requests.clone());
    let pair_ids = ScriptedSdmaDriverV1::same_device_owner_ids(&submission.pair);
    if pair_ids != Some((submission.source_owner_id, submission.destination_owner_id))
        || reported_requests.as_ref() != submission.requests.as_ref()
    {
        return Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
            detail: "scripted same-device completion identity or request roster changed".to_owned(),
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDevicePair(
                submission.pair,
            )),
        });
    }
    let packet_count = reported_requests.len();
    let (source_offset, destination_offset) = reported_requests
        .first()
        .map(|request| (request.source_offset, request.destination_offset))
        .unwrap_or((0, 0));
    Ok(SameDeviceSdmaPollV1::Completed(
        SameDeviceSdmaCompletedOwnerV1::Scripted(ScriptedSameDeviceCompletedOwnerV1 {
            pair: submission.pair,
            source_offset,
            destination_offset,
            copy_bytes: copy_bytes.unwrap_or(submission.copy_bytes),
            packet_count,
        }),
    ))
}

fn scripted_buffer_mismatch(
    buffer: ScriptedBufferOwnerV1,
    detail: String,
) -> SdmaTransitionFailureV1<SdmaBufferOwnerV1> {
    SdmaTransitionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
            SdmaBufferOwnerV1::Scripted(buffer),
        )),
    }
}

fn scripted_same_device_pair_mismatch(
    pair: SameDeviceSdmaPairOwnerV1,
    detail: String,
) -> SdmaTransitionFailureV1<SameDeviceSdmaPairOwnerV1> {
    SdmaTransitionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDevicePair(pair)),
    }
}

fn scripted_same_device_submission_mismatch(
    submission: ScriptedSameDeviceSubmissionOwnerV1,
    detail: String,
) -> SameDeviceSdmaExecutionFailureV1 {
    SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDeviceSubmission(
            SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
        )),
    }
}

fn scripted_same_device_completed_mismatch(
    completed: ScriptedSameDeviceCompletedOwnerV1,
    detail: String,
) -> SdmaTransitionFailureV1<SameDeviceSdmaCompletedOwnerV1> {
    SdmaTransitionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDeviceCompleted(
            SameDeviceSdmaCompletedOwnerV1::Scripted(completed),
        )),
    }
}

fn scripted_device_mismatch(
    device: ScriptedDeviceOwnerV1,
    detail: String,
) -> SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1> {
    SdmaTransitionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(
            DirectionalSdmaDeviceOwnerV1::Scripted(device),
        )),
    }
}

fn scripted_pair_mismatch(
    pair: DirectionalSdmaPairOwnerV1,
    detail: String,
) -> SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1> {
    SdmaTransitionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(pair)),
    }
}

fn scripted_submission_mismatch(
    submission: ScriptedSubmissionOwnerV1,
    detail: String,
) -> DirectionalSdmaExecutionFailureV1 {
    DirectionalSdmaExecutionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(
            DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
        )),
    }
}

fn scripted_completed_mismatch(
    completed: ScriptedCompletedOwnerV1,
    detail: String,
) -> SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1> {
    SdmaTransitionFailureV1::ProcessTeardown {
        detail,
        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(
            DirectionalSdmaCompletedOwnerV1::Scripted(completed),
        )),
    }
}

#[test]
fn scripted_cpu_write_invalidates_authenticated_full_content() {
    let mut driver = ScriptedSdmaDriverV1::new([
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: 8,
        },
        ScriptedSdmaStepV1::Write {
            offset: 3,
            byte_len: 1,
        },
    ]);
    let SdmaBufferOwnerV1::Scripted(mut host) = driver.test_host_owner(8) else {
        unreachable!()
    };
    let digest = driver
        .write_full_host_authenticated(&mut host, &[0x5a; 8])
        .unwrap();
    assert_eq!(
        host.full_content_certificate
            .as_ref()
            .map(|certificate| certificate.sha256),
        Some(digest)
    );
    driver.write_host(&mut host, 3, &[0xa5]).unwrap();
    assert!(host.full_content_certificate.is_none());
}

#[test]
fn scripted_authenticated_write_preserves_max_linear_chunk_schedule() {
    let first_len = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    let byte_len = first_len + 1;
    let mut driver = ScriptedSdmaDriverV1::new([
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: first_len,
        },
        ScriptedSdmaStepV1::Write {
            offset: u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
            byte_len: 1,
        },
    ]);
    let SdmaBufferOwnerV1::Scripted(mut host) = driver.test_host_owner(byte_len) else {
        unreachable!()
    };
    let bytes = vec![0x6b; byte_len];
    let observed = driver
        .write_full_host_authenticated(&mut host, &bytes)
        .unwrap();
    assert_eq!(observed, <[u8; 32]>::from(Sha256::digest(&bytes)));
    assert!(driver.is_exhausted());
    assert_eq!(host.bytes, bytes);
    assert!(host.full_content_certificate.is_some());
}

#[test]
fn scripted_request_direction_preserves_h2d_source_and_invalidates_d2h_destination() {
    for (direction, expect_certificate) in [
        (Gfx942PersistentSdmaDirectionV1::HostToDevice, true),
        (Gfx942PersistentSdmaDirectionV1::DeviceToHost, false),
    ] {
        let mut driver = ScriptedSdmaDriverV1::new([
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Submit {
                direction,
                host_offset: 0,
                device_offset: 0,
                copy_bytes: 8,
                outcome: ScriptedFailureModeV1::Retryable,
            },
        ]);
        let SdmaBufferOwnerV1::Scripted(mut host) = driver.test_host_owner(8) else {
            unreachable!()
        };
        driver
            .write_full_host_authenticated(&mut host, &[0x5a; 8])
            .unwrap();
        let device = match driver.test_device_owner(8) {
            DirectionalSdmaDeviceOwnerV1::Scripted(device) => device,
            DirectionalSdmaDeviceOwnerV1::Native(_) => unreachable!(),
        };
        let request = DirectionalSdmaCopyRequestV1 {
            host_offset: 0,
            device_offset: 0,
            copy_bytes: 8,
        };
        let Err(SdmaTransitionFailureV1::Retryable { custody: pair, .. }) = driver.submit(
            device,
            host,
            direction,
            DirectionalSdmaRequestPlanV1::Single(request),
        ) else {
            panic!("scripted request must return recoverable prepublication custody")
        };
        let SdmaBufferOwnerV1::Scripted(host) = pair.host else {
            unreachable!()
        };
        assert_eq!(host.full_content_certificate.is_some(), expect_certificate);
    }
}

mod transitions;
