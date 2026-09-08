use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{IoSlice, Write as _};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd as _, AsRawFd as _, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_worker_v3_verification_protocol::{
    MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
    WorkerV3VerificationCapabilityProtocolErrorV5, WorkerV3VerificationCapabilityRequestIdentityV5,
    WorkerV3VerificationCapabilityRequestV5, WorkerV3VerificationCapabilityResponseV5,
    WorkerV3VerificationFdPayloadKindV1, WorkerV3VerificationProductionAttemptV5,
};
use rustix::event::PollFlags;
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
use sha2::{Digest as _, Sha256};

use crate::protected_completion_v5::{
    MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5, ProtectedWorkerV3VerifierSigningKeyV5,
    WorkerV3ProtectedCompletionRequestV5, WorkerV3ProtectedCompletionResponseV5,
    issue_worker_v3_protected_completion_v5,
};
use crate::service::{
    RetainedWorkerV3VerificationPayloadV1, WorkerV3VerificationCallerV1,
    WorkerV3VerificationChallengeReplayGuardV1, WorkerV3VerificationMeasurementResolverV1,
    WorkerV3VerificationPolicyResolverV1, WorkerV3VerificationServiceErrorV1, caller_identity,
    capture_payload, object_key, receive_request_bounded, require_passcred, require_peer_write_eof,
    send_response, validate_control, wait_for,
};

const RESPONSE_MEMFD_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

/// Durable tombstone boundary for every rejected V5 submission.
pub trait WorkerV3VerificationCapabilityAttemptQuarantineV5 {
    type Error: Error + Send + Sync + 'static;

    fn quarantine_unidentified_submission(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        submission_sha256: Option<[u8; 32]>,
    ) -> Result<(), Self::Error>;

    fn quarantine_attempt_coordinates(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        attempt: WorkerV3VerificationProductionAttemptV5,
    ) -> Result<(), Self::Error>;

    fn quarantine_evidence(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        evidence_identity: Option<[u8; 32]>,
        reason: WorkerV3VerificationCapabilityRejectionReasonV5,
    ) -> Result<(), Self::Error>;
}

/// Restart-safe success boundary. The complete signed envelope must be fsynced before `Ok(())`.
pub trait WorkerV3VerificationCapabilityCompletionJournalV5 {
    type Error: Error + Send + Sync + 'static;

    fn record_issued(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        evidence_identity: [u8; 32],
        response: &WorkerV3ProtectedCompletionResponseV5,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationCapabilityRejectionReasonV5 {
    PolicyUnresolved,
    PolicyMismatch,
    MeasurementUnresolved,
    MeasurementMismatch,
    ChallengeReplay,
    PayloadDescriptor,
    PayloadAlias,
    PayloadCustody,
    EvidenceMalformed,
    EvidenceMismatch,
    ProtectedVerification,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationCapabilitySessionOutcomeV5 {
    Completed(Box<CompletedWorkerV3VerificationCapabilitySessionV5>),
    Rejected(Box<RejectedWorkerV3VerificationCapabilitySessionV5>),
}

/// Issued response plus immutable request payloads; no local transaction owner ever enters it.
pub struct CompletedWorkerV3VerificationCapabilitySessionV5 {
    caller: WorkerV3VerificationCallerV1,
    request: WorkerV3VerificationCapabilityRequestV5,
    response: WorkerV3ProtectedCompletionResponseV5,
    payloads: [RetainedWorkerV3VerificationPayloadV1; 2],
}

impl CompletedWorkerV3VerificationCapabilitySessionV5 {
    pub const fn caller(&self) -> WorkerV3VerificationCallerV1 {
        self.caller
    }

    pub const fn request(&self) -> &WorkerV3VerificationCapabilityRequestV5 {
        &self.request
    }

    pub const fn response(&self) -> &WorkerV3ProtectedCompletionResponseV5 {
        &self.response
    }

    pub fn payload(
        &self,
        kind: WorkerV3VerificationFdPayloadKindV1,
    ) -> &RetainedWorkerV3VerificationPayloadV1 {
        &self.payloads[kind.fd_ordinal() as usize]
    }

    pub fn into_response(self) -> WorkerV3ProtectedCompletionResponseV5 {
        self.response
    }
}

impl fmt::Debug for CompletedWorkerV3VerificationCapabilitySessionV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedWorkerV3VerificationCapabilitySessionV5")
            .field("caller", &self.caller)
            .field("request", &self.request.identity())
            .field("response", &self.response.identity())
            .field("local_owner_custody", &false)
            .finish_non_exhaustive()
    }
}

pub struct RejectedWorkerV3VerificationCapabilitySessionV5 {
    caller: WorkerV3VerificationCallerV1,
    request: WorkerV3VerificationCapabilityRequestV5,
    response: WorkerV3VerificationCapabilityResponseV5,
    reason: WorkerV3VerificationCapabilityRejectionReasonV5,
}

impl RejectedWorkerV3VerificationCapabilitySessionV5 {
    pub const fn caller(&self) -> WorkerV3VerificationCallerV1 {
        self.caller
    }

    pub const fn request(&self) -> &WorkerV3VerificationCapabilityRequestV5 {
        &self.request
    }

    pub const fn response(&self) -> &WorkerV3VerificationCapabilityResponseV5 {
        &self.response
    }

    pub const fn reason(&self) -> WorkerV3VerificationCapabilityRejectionReasonV5 {
        self.reason
    }
}

impl fmt::Debug for RejectedWorkerV3VerificationCapabilitySessionV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RejectedWorkerV3VerificationCapabilitySessionV5")
            .field("caller", &self.caller)
            .field("request", &self.request.identity())
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Receives one immutable request, reconstructs #213/#214 evidence, journals, then responds.
#[allow(clippy::too_many_arguments)]
pub fn serve_worker_v3_verification_capability_session_v5<P, M, R, J, Q>(
    control: OwnedFd,
    timeout: Duration,
    policy_resolver: &mut P,
    measurement_resolver: &mut M,
    replay_guard: &mut R,
    signer: &ProtectedWorkerV3VerifierSigningKeyV5,
    journal: &mut J,
    quarantine: &mut Q,
) -> Result<
    WorkerV3VerificationCapabilitySessionOutcomeV5,
    WorkerV3VerificationCapabilityServiceErrorV5,
>
where
    P: WorkerV3VerificationPolicyResolverV1,
    M: WorkerV3VerificationMeasurementResolverV1,
    R: WorkerV3VerificationChallengeReplayGuardV1,
    J: WorkerV3VerificationCapabilityCompletionJournalV5,
    Q: WorkerV3VerificationCapabilityAttemptQuarantineV5,
{
    if timeout.is_zero() {
        return Err(WorkerV3VerificationCapabilityServiceErrorV5::InvalidTimeout);
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(WorkerV3VerificationCapabilityServiceErrorV5::DeadlineOverflow)?;
    validate_control(&control).map_err(WorkerV3VerificationCapabilityServiceErrorV5::V1)?;
    require_passcred(&control).map_err(WorkerV3VerificationCapabilityServiceErrorV5::V1)?;
    let caller =
        caller_identity(&control).map_err(WorkerV3VerificationCapabilityServiceErrorV5::V1)?;
    let (request_bytes, descriptors) = match receive_request_bounded(
        &control,
        caller,
        deadline,
        MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
    ) {
        Ok(received) => received,
        Err(source) => {
            persist_unidentified(quarantine, caller, None)?;
            return Err(WorkerV3VerificationCapabilityServiceErrorV5::V1(source));
        }
    };
    let request = match WorkerV3VerificationCapabilityRequestV5::decode_canonical(&request_bytes) {
        Ok(request) => request,
        Err(source) => {
            persist_unidentified(
                quarantine,
                caller,
                Some(Sha256::digest(&request_bytes).into()),
            )?;
            return Err(WorkerV3VerificationCapabilityServiceErrorV5::Protocol(
                source,
            ));
        }
    };
    if let Err(source) = require_peer_write_eof(&control, deadline) {
        persist_attempt(quarantine, caller, &request)?;
        return Err(WorkerV3VerificationCapabilityServiceErrorV5::V1(source));
    }

    let base = request.base_request();
    let Some(expected_policy) = policy_resolver.resolve_expected_policy(caller, base) else {
        return reject_coordinates(
            control,
            caller,
            deadline,
            request,
            WorkerV3VerificationCapabilityRejectionReasonV5::PolicyUnresolved,
            quarantine,
        );
    };
    if base.policy_identity() != expected_policy {
        return reject_coordinates(
            control,
            caller,
            deadline,
            request,
            WorkerV3VerificationCapabilityRejectionReasonV5::PolicyMismatch,
            quarantine,
        );
    }
    let Some(expected_measurement) =
        measurement_resolver.resolve_expected_measurement(caller, expected_policy, base)
    else {
        return reject_coordinates(
            control,
            caller,
            deadline,
            request,
            WorkerV3VerificationCapabilityRejectionReasonV5::MeasurementUnresolved,
            quarantine,
        );
    };
    if base.measurement_identity() != expected_measurement {
        return reject_coordinates(
            control,
            caller,
            deadline,
            request,
            WorkerV3VerificationCapabilityRejectionReasonV5::MeasurementMismatch,
            quarantine,
        );
    }
    if !replay_guard.admit_fresh_challenge(caller, expected_policy, base.challenge()) {
        return reject_coordinates(
            control,
            caller,
            deadline,
            request,
            WorkerV3VerificationCapabilityRejectionReasonV5::ChallengeReplay,
            quarantine,
        );
    }

    let [evidence_source, object_source] = descriptors;
    let evidence_key = object_key(&evidence_source);
    let object_key_value = object_key(&object_source);
    if evidence_key.is_err() || object_key_value.is_err() {
        return reject_evidence(
            control,
            caller,
            deadline,
            request,
            None,
            WorkerV3VerificationCapabilityRejectionReasonV5::PayloadDescriptor,
            quarantine,
        );
    }
    if matches!((evidence_key, object_key_value), (Ok(first), Ok(second)) if first == second) {
        return reject_evidence(
            control,
            caller,
            deadline,
            request,
            None,
            WorkerV3VerificationCapabilityRejectionReasonV5::PayloadAlias,
            quarantine,
        );
    }
    let evidence_payload = match capture_payload(evidence_source, base.payloads()[0], caller) {
        Ok(payload) => payload,
        Err(_) => {
            return reject_evidence(
                control,
                caller,
                deadline,
                request,
                None,
                WorkerV3VerificationCapabilityRejectionReasonV5::PayloadCustody,
                quarantine,
            );
        }
    };
    let object_payload = match capture_payload(object_source, base.payloads()[1], caller) {
        Ok(payload) => payload,
        Err(_) => {
            return reject_evidence(
                control,
                caller,
                deadline,
                request,
                None,
                WorkerV3VerificationCapabilityRejectionReasonV5::PayloadCustody,
                quarantine,
            );
        }
    };
    let evidence_bytes = read_payload(
        &evidence_payload,
        MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
    )?;
    let object_bytes = read_payload(&object_payload, usize::MAX)?;
    let evidence = match WorkerV3ProtectedCompletionRequestV5::decode(&evidence_bytes) {
        Ok(evidence) => evidence,
        Err(_) => {
            return reject_evidence(
                control,
                caller,
                deadline,
                request,
                Some(Sha256::digest(&evidence_bytes).into()),
                WorkerV3VerificationCapabilityRejectionReasonV5::EvidenceMalformed,
                quarantine,
            );
        }
    };
    let evidence_identity = evidence.identity();
    if evidence.request_binding_identity()
        != request
            .protected_evidence_binding()
            .map_err(WorkerV3VerificationCapabilityServiceErrorV5::Protocol)?
            .as_bytes()
    {
        return reject_evidence(
            control,
            caller,
            deadline,
            request,
            Some(evidence_identity),
            WorkerV3VerificationCapabilityRejectionReasonV5::EvidenceMismatch,
            quarantine,
        );
    }
    let response =
        match issue_worker_v3_protected_completion_v5(&request, evidence, &object_bytes, signer) {
            Ok(response) => response,
            Err(_) => {
                return reject_evidence(
                    control,
                    caller,
                    deadline,
                    request,
                    Some(evidence_identity),
                    WorkerV3VerificationCapabilityRejectionReasonV5::ProtectedVerification,
                    quarantine,
                );
            }
        };

    journal
        .record_issued(caller, request.identity(), evidence_identity, &response)
        .map_err(|source| {
            WorkerV3VerificationCapabilityServiceErrorV5::Persistence(source.to_string())
        })?;
    send_completed_response(&control, &response, deadline)
        .map_err(WorkerV3VerificationCapabilityServiceErrorV5::V1)?;
    drop(control);
    Ok(WorkerV3VerificationCapabilitySessionOutcomeV5::Completed(
        Box::new(CompletedWorkerV3VerificationCapabilitySessionV5 {
            caller,
            request,
            response,
            payloads: [evidence_payload, object_payload],
        }),
    ))
}

fn persist_unidentified<Q: WorkerV3VerificationCapabilityAttemptQuarantineV5>(
    quarantine: &mut Q,
    caller: WorkerV3VerificationCallerV1,
    digest: Option<[u8; 32]>,
) -> Result<(), WorkerV3VerificationCapabilityServiceErrorV5> {
    quarantine
        .quarantine_unidentified_submission(caller, digest)
        .map_err(|source| {
            WorkerV3VerificationCapabilityServiceErrorV5::Persistence(source.to_string())
        })
}

fn persist_attempt<Q: WorkerV3VerificationCapabilityAttemptQuarantineV5>(
    quarantine: &mut Q,
    caller: WorkerV3VerificationCallerV1,
    request: &WorkerV3VerificationCapabilityRequestV5,
) -> Result<(), WorkerV3VerificationCapabilityServiceErrorV5> {
    quarantine
        .quarantine_attempt_coordinates(caller, request.identity(), request.carriage().attempt())
        .map_err(|source| {
            WorkerV3VerificationCapabilityServiceErrorV5::Persistence(source.to_string())
        })
}

fn reject_coordinates<Q: WorkerV3VerificationCapabilityAttemptQuarantineV5>(
    control: OwnedFd,
    caller: WorkerV3VerificationCallerV1,
    deadline: Instant,
    request: WorkerV3VerificationCapabilityRequestV5,
    reason: WorkerV3VerificationCapabilityRejectionReasonV5,
    quarantine: &mut Q,
) -> Result<
    WorkerV3VerificationCapabilitySessionOutcomeV5,
    WorkerV3VerificationCapabilityServiceErrorV5,
> {
    persist_attempt(quarantine, caller, &request)?;
    reject_response(control, caller, deadline, request, reason)
}

fn reject_evidence<Q: WorkerV3VerificationCapabilityAttemptQuarantineV5>(
    control: OwnedFd,
    caller: WorkerV3VerificationCallerV1,
    deadline: Instant,
    request: WorkerV3VerificationCapabilityRequestV5,
    evidence_identity: Option<[u8; 32]>,
    reason: WorkerV3VerificationCapabilityRejectionReasonV5,
    quarantine: &mut Q,
) -> Result<
    WorkerV3VerificationCapabilitySessionOutcomeV5,
    WorkerV3VerificationCapabilityServiceErrorV5,
> {
    quarantine
        .quarantine_evidence(caller, request.identity(), evidence_identity, reason)
        .map_err(|source| {
            WorkerV3VerificationCapabilityServiceErrorV5::Persistence(source.to_string())
        })?;
    reject_response(control, caller, deadline, request, reason)
}

fn reject_response(
    control: OwnedFd,
    caller: WorkerV3VerificationCallerV1,
    deadline: Instant,
    request: WorkerV3VerificationCapabilityRequestV5,
    reason: WorkerV3VerificationCapabilityRejectionReasonV5,
) -> Result<
    WorkerV3VerificationCapabilitySessionOutcomeV5,
    WorkerV3VerificationCapabilityServiceErrorV5,
> {
    let response = WorkerV3VerificationCapabilityResponseV5::rejected(&request)?;
    send_response(&control, response.encode_canonical(), deadline)
        .map_err(WorkerV3VerificationCapabilityServiceErrorV5::V1)?;
    drop(control);
    Ok(WorkerV3VerificationCapabilitySessionOutcomeV5::Rejected(
        Box::new(RejectedWorkerV3VerificationCapabilitySessionV5 {
            caller,
            request,
            response,
            reason,
        }),
    ))
}

fn read_payload(
    payload: &RetainedWorkerV3VerificationPayloadV1,
    maximum: usize,
) -> Result<Vec<u8>, WorkerV3VerificationCapabilityServiceErrorV5> {
    let length = usize::try_from(payload.byte_len())
        .map_err(|_| WorkerV3VerificationCapabilityServiceErrorV5::PayloadTooLarge)?;
    if length > maximum {
        return Err(WorkerV3VerificationCapabilityServiceErrorV5::PayloadTooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| WorkerV3VerificationCapabilityServiceErrorV5::PayloadTooLarge)?;
    bytes.resize(length, 0);
    if rustix::io::pread(payload, &mut bytes, 0)
        .map_err(|_| WorkerV3VerificationCapabilityServiceErrorV5::PayloadRead)?
        != length
    {
        return Err(WorkerV3VerificationCapabilityServiceErrorV5::PayloadRead);
    }
    Ok(bytes)
}

fn send_completed_response(
    control: &OwnedFd,
    response: &WorkerV3ProtectedCompletionResponseV5,
    deadline: Instant,
) -> Result<(), WorkerV3VerificationServiceErrorV1> {
    let descriptor = rustix::fs::memfd_create(
        "fe2o3-worker-v3-protected-completion-v5",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(|source| WorkerV3VerificationServiceErrorV1::Io {
        operation: "create protected completion response",
        source: source.into(),
    })?;
    let mut writer = File::from(descriptor);
    rustix::fs::fchmod(&writer, Mode::RUSR).map_err(|source| {
        WorkerV3VerificationServiceErrorV1::Io {
            operation: "restrict protected completion response",
            source: source.into(),
        }
    })?;
    writer
        .write_all(response.canonical_bytes())
        .map_err(|source| WorkerV3VerificationServiceErrorV1::Io {
            operation: "write protected completion response",
            source,
        })?;
    writer
        .flush()
        .map_err(|source| WorkerV3VerificationServiceErrorV1::Io {
            operation: "flush protected completion response",
            source,
        })?;
    rustix::fs::fcntl_add_seals(&writer, RESPONSE_MEMFD_SEALS).map_err(|source| {
        WorkerV3VerificationServiceErrorV1::Io {
            operation: "seal protected completion response",
            source: source.into(),
        }
    })?;
    let reader = rustix::fs::open(
        format!("/proc/self/fd/{}", writer.as_raw_fd()),
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| WorkerV3VerificationServiceErrorV1::Io {
        operation: "reopen protected completion response",
        source: source.into(),
    })?;
    drop(writer);

    let packet = response.transport_response().encode_canonical();
    loop {
        wait_for(control, PollFlags::OUT, deadline)?;
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut storage);
        let descriptors = [reader.as_fd()];
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(WorkerV3VerificationServiceErrorV1::InvalidControl(
                "protected response ancillary storage is undersized",
            ));
        }
        match sendmsg(
            control,
            &[IoSlice::new(packet)],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(sent) if sent == packet.len() => return Ok(()),
            Ok(_) => return Err(WorkerV3VerificationServiceErrorV1::PartialSend),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(source) => {
                return Err(WorkerV3VerificationServiceErrorV1::Io {
                    operation: "send protected completion response",
                    source: source.into(),
                });
            }
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationCapabilityServiceErrorV5 {
    InvalidTimeout,
    DeadlineOverflow,
    PayloadTooLarge,
    PayloadRead,
    V1(WorkerV3VerificationServiceErrorV1),
    Protocol(WorkerV3VerificationCapabilityProtocolErrorV5),
    Persistence(String),
}

impl fmt::Display for WorkerV3VerificationCapabilityServiceErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Worker V3 V5 capability service failed: {self:?}"
        )
    }
}

impl Error for WorkerV3VerificationCapabilityServiceErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::V1(source) => Some(source),
            Self::Protocol(source) => Some(source),
            _ => None,
        }
    }
}

impl From<WorkerV3VerificationCapabilityProtocolErrorV5>
    for WorkerV3VerificationCapabilityServiceErrorV5
{
    fn from(source: WorkerV3VerificationCapabilityProtocolErrorV5) -> Self {
        Self::Protocol(source)
    }
}
