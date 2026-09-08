use std::error::Error;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    AuthenticatedCompilerCapabilityCompletionErrorV5, AuthenticatedCompilerCapabilityCompletionV5,
    InertCompilerCapabilityVerifierResponseV5, MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5,
    PreparedCompilerCapabilityCompletionV5,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13;
use fe2o3_worker_v3_verification_protocol::{
    ExactIdentityCoordinateV5, MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
    WorkerV3VerificationCapabilityCarriageV5, WorkerV3VerificationCapabilityProtocolErrorV5,
    WorkerV3VerificationCapabilityRequestIdentityV5, WorkerV3VerificationCapabilityRequestV5,
    WorkerV3VerificationCapabilityResponseDispositionV5,
    WorkerV3VerificationCapabilityResponseIdentityV5, WorkerV3VerificationCapabilityResponseV5,
    WorkerV3VerificationEntryCoordinateV1, WorkerV3VerificationFdPayloadDescriptorV1,
    WorkerV3VerificationFreshChallengeV1, WorkerV3VerificationMeasurementIdentityV1,
    WorkerV3VerificationPolicyIdentityV1, WorkerV3VerificationProductionAttemptV5,
    WorkerV3VerificationProtectedEvidenceBindingIdentityV5, WorkerV3VerificationRequestV1,
    WorkerV3VerificationRosterIdentityV1, derive_worker_v3_protected_evidence_binding_v5,
};
use rustix::event::PollFlags;
use rustix::fs::{FileType, OFlags, SealFlags};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, Shutdown,
    SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
};
use sha2::{Digest as _, Sha256};

use crate::client_v2::{
    PeerCredentialsV2, canonical_filesystem_unix_address, enable_passcred, peer_credentials,
    require_deadline, require_peer_eof, send_begin, set_close_on_exec,
    validate_connected_path_peer, wait_for_peer,
};
use crate::{
    WorkerV3VerificationClientErrorV1, WorkerV3VerificationClientErrorV2,
    WorkerV3VerificationPayloadSnapshotsV1,
};

/// Sole production pathname for the issue #272 capability-completion service.
pub const PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5: &str = "/run/fe2o3/worker-v3-verifier.sock";

const VERIFIER_MEASUREMENT_DOMAIN_V5: &[u8] = b"FE2O3/WORKER-V3/FIXED-VERIFIER-MEASUREMENT/V5\0";

const PROTECTED_RESPONSE_MAGIC_V5: [u8; 8] = *b"F2WVPS05";
const PROTECTED_RESPONSE_VERSION_V5: u16 = 5;
const PROTECTED_RESPONSE_FIELDS_V5: u16 = 6;
const PROTECTED_RESPONSE_HEADER_BYTES_V5: usize = 24;
const PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5: usize = 8;
const PROTECTED_RESPONSE_TERMINAL_BYTES_V5: usize = 32;
const PROTECTED_RESPONSE_DOMAIN_V5: &[u8] = b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-RESPONSE/V5\0";
const PROTECTED_RESPONSE_IDENTITY_DOMAIN_V5: &[u8] =
    b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-RESPONSE-IDENTITY/V5\0";
const REQUIRED_PROTECTED_RESPONSE_SEALS_V5: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);
const TMPFS_MAGIC: u64 = 0x0102_1994;
const MAX_PROTECTED_RESPONSE_BYTES_V5: usize = PROTECTED_RESPONSE_HEADER_BYTES_V5
    + PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5 * PROTECTED_RESPONSE_FIELDS_V5 as usize
    + PROTECTED_RESPONSE_DOMAIN_V5.len()
    + 32
    + 32
    + MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5
    + MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5
    + 64
    + PROTECTED_RESPONSE_TERMINAL_BYTES_V5;

/// Derives the sole deployment measurement accepted by the fixed V5 service.
pub fn production_worker_v3_verifier_measurement_identity_v5(
    profile: &CompilerExecutionClientProfileV1,
) -> [u8; 32] {
    let executable = profile.policy().executable();
    let runtime = profile.policy().runtime();
    let mut digest = Sha256::new();
    digest.update(VERIFIER_MEASUREMENT_DOMAIN_V5);
    digest.update(profile.identity().as_bytes());
    digest.update(profile.policy().identity().as_bytes());
    digest.update(executable.sha256());
    digest.update(executable.byte_len().to_le_bytes());
    digest.update(runtime.sha256());
    digest.update(runtime.byte_len().to_le_bytes());
    digest.update(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5.as_bytes());
    digest.update(PROTECTED_RESPONSE_MAGIC_V5);
    digest.update(PROTECTED_RESPONSE_VERSION_V5.to_le_bytes());
    digest.finalize().into()
}

/// Caller-pinned endpoint and claim expectations for one V5 exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationCapabilityPeerPolicyV5 {
    uid: u32,
    gid: u32,
    policy: [u8; 32],
    measurement: [u8; 32],
}

impl WorkerV3VerificationCapabilityPeerPolicyV5 {
    pub fn new(
        uid: u32,
        gid: u32,
        policy: [u8; 32],
        measurement: [u8; 32],
    ) -> Result<Self, WorkerV3VerificationCapabilityClientErrorV5> {
        if policy == [0; 32] || measurement == [0; 32] {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::InvalidPeerPolicy);
        }
        Ok(Self {
            uid,
            gid,
            policy,
            measurement,
        })
    }

    pub const fn uid(self) -> u32 {
        self.uid
    }

    pub const fn gid(self) -> u32 {
        self.gid
    }

    pub const fn policy_identity(self) -> [u8; 32] {
        self.policy
    }

    pub const fn measurement_identity(self) -> [u8; 32] {
        self.measurement
    }
}

/// Move-only request custody derived from the already intake-authenticated V5 transaction.
///
/// ```compile_fail
/// use fe2o3_worker_v3_verification_client::IntakeAuthenticatedWorkerV3CapabilityCustodyV5;
/// fn duplicate(value: IntakeAuthenticatedWorkerV3CapabilityCustodyV5) {
///     let _again = value.clone();
/// }
/// ```
pub struct IntakeAuthenticatedWorkerV3CapabilityCustodyV5 {
    request: WorkerV3VerificationCapabilityRequestV5,
}

/// Request state whose evidence correlation can be computed before the evidence memfd is sealed.
pub struct WorkerV3VerificationCapabilityRequestPlanV5 {
    challenge: WorkerV3VerificationFreshChallengeV1,
    roster: WorkerV3VerificationRosterIdentityV1,
    policy: WorkerV3VerificationPolicyIdentityV1,
    measurement: WorkerV3VerificationMeasurementIdentityV1,
    object: WorkerV3VerificationFdPayloadDescriptorV1,
    entries: Vec<WorkerV3VerificationEntryCoordinateV1>,
    carriage: WorkerV3VerificationCapabilityCarriageV5,
    binding: WorkerV3VerificationProtectedEvidenceBindingIdentityV5,
}

impl WorkerV3VerificationCapabilityRequestPlanV5 {
    #[allow(clippy::too_many_arguments)]
    pub fn from_prepared_completion(
        challenge: WorkerV3VerificationFreshChallengeV1,
        roster: WorkerV3VerificationRosterIdentityV1,
        policy: WorkerV3VerificationPolicyIdentityV1,
        measurement: WorkerV3VerificationMeasurementIdentityV1,
        entries: Vec<WorkerV3VerificationEntryCoordinateV1>,
        prepared: &PreparedCompilerCapabilityCompletionV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let object = WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(
            u64::try_from(prepared.object_bytes().len())
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?,
            Sha256::digest(prepared.object_bytes()).into(),
        )
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::InvalidBaseRequest)?;
        let carriage = carriage_from_prepared_completion(prepared)?;
        let binding = derive_worker_v3_protected_evidence_binding_v5(
            challenge,
            roster,
            policy,
            measurement,
            object,
            &entries,
            &carriage,
        )?;
        Ok(Self {
            challenge,
            roster,
            policy,
            measurement,
            object,
            entries,
            carriage,
            binding,
        })
    }

    pub const fn protected_evidence_binding(
        &self,
    ) -> WorkerV3VerificationProtectedEvidenceBindingIdentityV5 {
        self.binding
    }

    pub fn complete(
        self,
        evidence: WorkerV3VerificationFdPayloadDescriptorV1,
    ) -> Result<
        IntakeAuthenticatedWorkerV3CapabilityCustodyV5,
        WorkerV3VerificationCapabilityProtocolErrorV5,
    > {
        let base = WorkerV3VerificationRequestV1::new(
            self.challenge,
            self.roster,
            self.policy,
            self.measurement,
            evidence,
            self.object,
            self.entries,
        )
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::InvalidBaseRequest)?;
        let request = WorkerV3VerificationCapabilityRequestV5::new(base, self.carriage)?;
        if request.protected_evidence_binding()? != self.binding {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::IdentityMismatch);
        }
        Ok(IntakeAuthenticatedWorkerV3CapabilityCustodyV5 { request })
    }
}

impl IntakeAuthenticatedWorkerV3CapabilityCustodyV5 {
    /// Captures only exact identity coordinates; the prepared transaction remains with its owner.
    pub fn from_prepared_completion(
        base_request: WorkerV3VerificationRequestV1,
        prepared: &PreparedCompilerCapabilityCompletionV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let carriage = carriage_from_prepared_completion(prepared)?;
        Ok(Self {
            request: WorkerV3VerificationCapabilityRequestV5::new(base_request, carriage)?,
        })
    }

    pub const fn request(&self) -> &WorkerV3VerificationCapabilityRequestV5 {
        &self.request
    }
}

fn carriage_from_prepared_completion(
    prepared: &PreparedCompilerCapabilityCompletionV5,
) -> Result<WorkerV3VerificationCapabilityCarriageV5, WorkerV3VerificationCapabilityProtocolErrorV5>
{
    let attempt = prepared.attempt();
    let attempt = WorkerV3VerificationProductionAttemptV5::new(
        attempt.generation(),
        *attempt.session().as_bytes(),
        *attempt.invocation().as_bytes(),
    )?;
    let object = prepared.object_bytes();
    let handoff = prepared.handoff();
    let inputs = handoff.inputs();
    let subject = inputs.subject();
    let report = handoff.final_graph_report();
    let closure = handoff.target_closure();
    let executable_kir = VerifiedCanonicalKernelIrV13::from_canonical_bytes(
        handoff.executable_kir().canonical_preimage().to_vec(),
    )
    .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::SubjectMismatch)?;
    executable_kir
        .revalidate()
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::SubjectMismatch)?;
    if report.final_graph() != *executable_kir.identity().digest()
        || report.final_graph_bytes() != executable_kir.identity().canonical_length()
        || report.final_graph() != *subject.executable_kir().digest().as_bytes()
        || report.final_epoch() != subject.executable_kir_epoch()
        || closure.neutral_graph() != report.final_graph()
        || closure.neutral_graph_bytes() != report.final_graph_bytes()
        || closure.neutral_epoch() != report.final_epoch()
        || closure.target_model() != subject.target_model()
        || closure.launch_contract() != subject.launch_contract()
    {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::SubjectMismatch);
    }
    let handoff_identity = handoff.identity();
    let legacy_identity = handoff.legacy_handoff().identity();
    let source = handoff.source_refinement().identity();
    let executable_receipt = handoff.executable_kir().identity();
    WorkerV3VerificationCapabilityCarriageV5::new_exact(
        ExactIdentityCoordinateV5::new(handoff_identity.sha256(), handoff_identity.byte_len())?,
        ExactIdentityCoordinateV5::new(*legacy_identity.sha256(), legacy_identity.byte_len())?,
        *prepared.transaction_identity().as_bytes(),
        attempt,
        0,
        13,
        ExactIdentityCoordinateV5::new(
            *executable_kir.identity().digest(),
            executable_kir.identity().canonical_length(),
        )?,
        report.final_epoch(),
        *subject.kernel().digest().as_bytes(),
        *subject.root().digest().as_bytes(),
        *subject.target_model().digest().as_bytes(),
        *subject.launch_contract().digest().as_bytes(),
        closure.closure_identity(),
        ExactIdentityCoordinateV5::new(
            Sha256::digest(closure.canonical_bytes()).into(),
            u64::try_from(closure.canonical_bytes().len())
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?,
        )?,
        report.report_identity(),
        ExactIdentityCoordinateV5::new(source.sha256(), source.byte_len())?,
        ExactIdentityCoordinateV5::new(
            Sha256::digest(object).into(),
            u64::try_from(object.len())
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?,
        )?,
        inputs.semantic_mir_identity(),
        inputs.compiler_policy(),
        ExactIdentityCoordinateV5::new(executable_receipt.sha256(), executable_receipt.byte_len())?,
    )
}

/// Required durable response replay exclusion.
pub trait WorkerV3VerificationResponseReplayGuardV5 {
    /// Atomically consumes the exact response identity once for this request.
    fn admit_response_once(
        &mut self,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        response: WorkerV3VerificationCapabilityResponseIdentityV5,
    ) -> bool;
}

/// Required fail-closed client-side attempt quarantine.
pub trait WorkerV3VerificationClientFailureQuarantineV5 {
    type Error: Error + Send + Sync + 'static;

    /// Durably tombstones the exact attempt after any failed or rejected exchange.
    fn quarantine_failed_exchange(
        &mut self,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        attempt: WorkerV3VerificationProductionAttemptV5,
    ) -> Result<(), Self::Error>;
}

/// One owned, one-shot V5 connection to an exact pathname and credential-pinned service.
pub struct WorkerV3VerificationCapabilityClientV5 {
    peer: OwnedFd,
    deadline: Instant,
    peer_credentials: PeerCredentialsV2,
    policy: WorkerV3VerificationCapabilityPeerPolicyV5,
}

impl WorkerV3VerificationCapabilityClientV5 {
    /// Connects and admits the fixed production pathname under one caller-pinned profile.
    pub fn connect_production(
        policy: WorkerV3VerificationCapabilityPeerPolicyV5,
        timeout: Duration,
    ) -> Result<Self, WorkerV3VerificationCapabilityClientErrorV5> {
        if timeout.is_zero() {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::InvalidTimeout,
            ));
        }
        let deadline = Instant::now().checked_add(timeout).ok_or(
            WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::DeadlineOverflow,
            ),
        )?;
        let address =
            SocketAddrUnix::new(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5).map_err(|error| {
                WorkerV3VerificationCapabilityClientErrorV5::ServiceConnect(error.to_string())
            })?;
        let peer = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|error| {
            WorkerV3VerificationCapabilityClientErrorV5::ServiceConnect(error.to_string())
        })?;
        loop {
            require_deadline(deadline).map_err(WorkerV3VerificationCapabilityClientErrorV5::V2)?;
            match connect(&peer, &address) {
                Ok(()) | Err(rustix::io::Errno::ISCONN) => break,
                Err(rustix::io::Errno::INTR) => continue,
                Err(
                    rustix::io::Errno::INPROGRESS
                    | rustix::io::Errno::ALREADY
                    | rustix::io::Errno::AGAIN,
                ) => {
                    wait_for_peer(&peer, PollFlags::OUT, deadline)
                        .map_err(WorkerV3VerificationCapabilityClientErrorV5::V2)?;
                    rustix::net::sockopt::socket_error(&peer)
                        .map_err(|error| {
                            WorkerV3VerificationCapabilityClientErrorV5::ServiceConnect(
                                error.to_string(),
                            )
                        })?
                        .map_err(|error| {
                            WorkerV3VerificationCapabilityClientErrorV5::ServiceConnect(
                                error.to_string(),
                            )
                        })?;
                    break;
                }
                Err(error) => {
                    return Err(WorkerV3VerificationCapabilityClientErrorV5::ServiceConnect(
                        error.to_string(),
                    ));
                }
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        Self::admit_connected_path(
            peer,
            Path::new(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5),
            policy,
            remaining,
        )
        .map_err(|failure| failure.source)
    }

    /// Admits a caller-connected endpoint only when the pathname, peer credentials, policy, and
    /// verifier measurement are all caller-pinned. Production callers must pass
    /// [`PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5`].
    pub fn admit_connected_path(
        peer: OwnedFd,
        expected_service_path: &Path,
        policy: WorkerV3VerificationCapabilityPeerPolicyV5,
        timeout: Duration,
    ) -> Result<Self, WorkerV3VerificationCapabilityClientAdmissionFailureV5> {
        if timeout.is_zero() {
            return Err(WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
                peer,
                source: WorkerV3VerificationCapabilityClientErrorV5::V2(
                    WorkerV3VerificationClientErrorV2::InvalidTimeout,
                ),
            });
        }
        let Some(deadline) = Instant::now().checked_add(timeout) else {
            return Err(WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
                peer,
                source: WorkerV3VerificationCapabilityClientErrorV5::V2(
                    WorkerV3VerificationClientErrorV2::DeadlineOverflow,
                ),
            });
        };
        let admitted = (|| {
            require_deadline(deadline)?;
            let expected_address = canonical_filesystem_unix_address(expected_service_path)
                .ok_or(WorkerV3VerificationClientErrorV2::InvalidConnectedPathPeer)?;
            set_close_on_exec(&peer)?;
            validate_connected_path_peer(&peer, &expected_address)?;
            enable_passcred(&peer)?;
            validate_connected_path_peer(&peer, &expected_address)?;
            let credentials = peer_credentials(&peer)?;
            if credentials.uid != policy.uid || credentials.gid != policy.gid {
                return Err(WorkerV3VerificationClientErrorV2::ResponseCredentialsMismatch);
            }
            require_deadline(deadline)?;
            Ok(credentials)
        })();
        match admitted {
            Ok(peer_credentials) => Ok(Self {
                peer,
                deadline,
                peer_credentials,
                policy,
            }),
            Err(source) => Err(WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
                peer,
                source: WorkerV3VerificationCapabilityClientErrorV5::V2(source),
            }),
        }
    }

    /// Sends move-owned intake coordinates and receives one exact correlated response.
    pub fn exchange<R, Q>(
        self,
        custody: IntakeAuthenticatedWorkerV3CapabilityCustodyV5,
        snapshots: WorkerV3VerificationPayloadSnapshotsV1,
        response_replay: &mut R,
        quarantine: &mut Q,
    ) -> Result<
        WorkerV3VerificationCapabilityExchangeOutcomeV5,
        WorkerV3VerificationCapabilityClientErrorV5,
    >
    where
        R: WorkerV3VerificationResponseReplayGuardV5,
        Q: WorkerV3VerificationClientFailureQuarantineV5,
    {
        self.exchange_with_authenticator(
            custody,
            snapshots,
            response_replay,
            quarantine,
            authenticate_compiler_response_v5,
        )
    }

    fn exchange_with_authenticator<R, Q, A>(
        self,
        custody: IntakeAuthenticatedWorkerV3CapabilityCustodyV5,
        snapshots: WorkerV3VerificationPayloadSnapshotsV1,
        response_replay: &mut R,
        quarantine: &mut Q,
        authenticate: A,
    ) -> Result<
        WorkerV3VerificationCapabilityExchangeOutcomeV5,
        WorkerV3VerificationCapabilityClientErrorV5,
    >
    where
        R: WorkerV3VerificationResponseReplayGuardV5,
        Q: WorkerV3VerificationClientFailureQuarantineV5,
        A: FnOnce(&[u8], [u8; 64]) -> Result<[u8; 32], WorkerV3VerificationCapabilityClientErrorV5>,
    {
        let request = custody.request;
        let request_identity = request.identity();
        let attempt = request.carriage().attempt();
        let exchanged = (|| {
            if request.base_request().policy_identity().as_bytes() != &self.policy.policy
                || request.base_request().measurement_identity().as_bytes()
                    != &self.policy.measurement
            {
                return Err(WorkerV3VerificationCapabilityClientErrorV5::PeerPolicyMismatch);
            }
            snapshots
                .revalidate(request.base_request())
                .map_err(WorkerV3VerificationCapabilityClientErrorV5::V1)?;
            send_begin(
                &self.peer,
                request.encode_canonical(),
                snapshots.borrowed_fds(),
                self.deadline,
            )
            .map_err(WorkerV3VerificationCapabilityClientErrorV5::V2)?;
            rustix::net::shutdown(&self.peer, Shutdown::Write).map_err(|source| {
                WorkerV3VerificationCapabilityClientErrorV5::V2(
                    WorkerV3VerificationClientErrorV2::Shutdown(source.into()),
                )
            })?;
            snapshots
                .revalidate(request.base_request())
                .map_err(WorkerV3VerificationCapabilityClientErrorV5::V1)?;
            let received = receive_capability_response_packet_v5(
                &self.peer,
                self.deadline,
                self.peer_credentials,
            )?;
            let response =
                WorkerV3VerificationCapabilityResponseV5::decode_canonical(&received.bytes)?;
            if !response.matches_request(&request) {
                return Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch);
            }
            let protected = match response.disposition() {
                WorkerV3VerificationCapabilityResponseDispositionV5::Completed => {
                    if received.descriptors.len() != 1 {
                        return Err(
                            WorkerV3VerificationCapabilityClientErrorV5::ResponseDescriptorCount {
                                expected: 1,
                                actual: received.descriptors.len(),
                            },
                        );
                    }
                    let descriptor = received
                        .descriptors
                        .into_iter()
                        .next()
                        .expect("descriptor count was checked");
                    Some(read_and_authenticate_protected_response_v5(
                        descriptor,
                        &received.bytes,
                        &request,
                        authenticate,
                    )?)
                }
                WorkerV3VerificationCapabilityResponseDispositionV5::Rejected => {
                    if !received.descriptors.is_empty() {
                        return Err(
                            WorkerV3VerificationCapabilityClientErrorV5::ResponseDescriptorCount {
                                expected: 0,
                                actual: received.descriptors.len(),
                            },
                        );
                    }
                    None
                }
            };
            require_peer_eof(&self.peer, self.deadline)
                .map_err(WorkerV3VerificationCapabilityClientErrorV5::V2)?;
            if !response_replay.admit_response_once(request_identity, response.identity()) {
                return Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseReplay);
            }
            match response.disposition() {
                WorkerV3VerificationCapabilityResponseDispositionV5::Completed => {
                    let protected = protected.expect("completed response was authenticated");
                    Ok(WorkerV3VerificationCapabilityExchangeOutcomeV5::Completed(
                        WorkerV3VerificationCapabilityCompletedReceiptV5 {
                            request,
                            response,
                            protected_response: protected.canonical_bytes,
                            protected_response_identity: protected.identity,
                            authenticated_response_identity: protected.authenticated_identity,
                        },
                    ))
                }
                WorkerV3VerificationCapabilityResponseDispositionV5::Rejected => {
                    Ok(WorkerV3VerificationCapabilityExchangeOutcomeV5::Rejected(
                        WorkerV3VerificationCapabilityRejectedReceiptV5 { request, response },
                    ))
                }
            }
        })();
        if exchanged.is_err()
            || matches!(
                &exchanged,
                Ok(WorkerV3VerificationCapabilityExchangeOutcomeV5::Rejected(_))
            )
        {
            if let Err(source) = quarantine.quarantine_failed_exchange(request_identity, attempt) {
                return Err(WorkerV3VerificationCapabilityClientErrorV5::Quarantine(
                    source.to_string(),
                ));
            }
        }
        exchanged
    }
}

struct ReceivedCapabilityResponsePacketV5 {
    bytes: Vec<u8>,
    descriptors: Vec<OwnedFd>,
}

struct AuthenticatedProtectedResponseV5 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    authenticated_identity: [u8; 32],
}

fn receive_capability_response_packet_v5(
    peer: &OwnedFd,
    deadline: Instant,
    expected_credentials: PeerCredentialsV2,
) -> Result<ReceivedCapabilityResponsePacketV5, WorkerV3VerificationCapabilityClientErrorV5> {
    let mut bytes = vec![0_u8; MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5 + 1];
    loop {
        wait_for_peer(peer, PollFlags::IN, deadline)
            .map_err(WorkerV3VerificationCapabilityClientErrorV5::V2)?;
        let mut control_space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2), ScmCredentials(2))];
        let mut control = RecvAncillaryBuffer::new(&mut control_space);
        let received = {
            let mut vectors = [IoSliceMut::new(&mut bytes)];
            match rustix::net::recvmsg(
                peer,
                &mut vectors,
                &mut control,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC | RecvFlags::TRUNC,
            ) {
                Ok(received) => received,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(source) => {
                    return Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                        WorkerV3VerificationClientErrorV2::Receive(source.into()),
                    ));
                }
            }
        };
        let mut credentials = None;
        let mut descriptors = Vec::new();
        let mut invalid_control = false;
        for message in control.drain() {
            match message {
                RecvAncillaryMessage::ScmCredentials(received) => {
                    if credentials.replace(received).is_some() {
                        invalid_control = true;
                    }
                }
                RecvAncillaryMessage::ScmRights(received) => descriptors.extend(received),
                _ => invalid_control = true,
            }
        }
        if received.flags.contains(ReturnFlags::CTRUNC) || invalid_control {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::UnexpectedAncillaryData,
            ));
        }
        if received.flags.contains(ReturnFlags::TRUNC)
            || received.bytes > MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5
        {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::PacketOversize {
                    maximum: MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
                    actual: received.bytes,
                },
            ));
        }
        if received.bytes == 0 {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::PeerClosed,
            ));
        }
        if !credentials.is_some_and(|actual| {
            u32::try_from(actual.pid.as_raw_pid()).ok() == Some(expected_credentials.pid)
                && actual.uid.as_raw() == expected_credentials.uid
                && actual.gid.as_raw() == expected_credentials.gid
        }) {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::ResponseCredentialsMismatch,
            ));
        }
        bytes.truncate(received.bytes);
        return Ok(ReceivedCapabilityResponsePacketV5 { bytes, descriptors });
    }
}

fn read_and_authenticate_protected_response_v5<A>(
    descriptor: OwnedFd,
    packet: &[u8],
    request: &WorkerV3VerificationCapabilityRequestV5,
    authenticate: A,
) -> Result<AuthenticatedProtectedResponseV5, WorkerV3VerificationCapabilityClientErrorV5>
where
    A: FnOnce(&[u8], [u8; 64]) -> Result<[u8; 32], WorkerV3VerificationCapabilityClientErrorV5>,
{
    validate_protected_response_descriptor_v5(&descriptor)?;
    let stat = rustix::fs::fstat(&descriptor)
        .map_err(|source| response_descriptor_io("stat protected response", source.into()))?;
    let length = usize::try_from(stat.st_size).map_err(|_| {
        WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseTooLarge {
            maximum: MAX_PROTECTED_RESPONSE_BYTES_V5,
            actual: usize::MAX,
        }
    })?;
    if length > MAX_PROTECTED_RESPONSE_BYTES_V5 {
        return Err(
            WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseTooLarge {
                maximum: MAX_PROTECTED_RESPONSE_BYTES_V5,
                actual: length,
            },
        );
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| {
        WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseTooLarge {
            maximum: MAX_PROTECTED_RESPONSE_BYTES_V5,
            actual: length,
        }
    })?;
    bytes.resize(length, 0);
    if rustix::io::pread(&descriptor, &mut bytes, 0)
        .map_err(|source| response_descriptor_io("read protected response", source.into()))?
        != length
    {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseRead);
    }
    let mut trailing = [0_u8; 1];
    if rustix::io::pread(&descriptor, &mut trailing, length as u64).map_err(|source| {
        response_descriptor_io("check protected response boundary", source.into())
    })? != 0
    {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseRead);
    }
    let decoded = decode_protected_response_record_v5(&bytes)?;
    if decoded.fields[0] != PROTECTED_RESPONSE_DOMAIN_V5
        || decoded.fields[1] != request.identity().as_bytes()
        || decoded.fields[2] != request.base_request().challenge().as_bytes()
        || decoded.fields[3] != packet
    {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch);
    }
    let transport = WorkerV3VerificationCapabilityResponseV5::decode_canonical(decoded.fields[3])?;
    if !transport.matches_request(request)
        || transport.disposition() != WorkerV3VerificationCapabilityResponseDispositionV5::Completed
    {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch);
    }
    let completion = transport
        .completion()
        .ok_or(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch)?;
    let verifier_response = decoded.fields[4];
    let verifier_identity = ExactIdentityCoordinateV5::new(
        Sha256::digest(verifier_response).into(),
        u64::try_from(verifier_response.len())
            .map_err(|_| WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)?,
    )?;
    if completion.result_identity() != verifier_identity {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch);
    }
    let signature: [u8; 64] = decoded.fields[5]
        .try_into()
        .map_err(|_| WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)?;
    let authenticated_identity = authenticate(verifier_response, signature)?;
    let identity = decoded.identity;
    drop(decoded);
    Ok(AuthenticatedProtectedResponseV5 {
        canonical_bytes: bytes.into_boxed_slice(),
        identity,
        authenticated_identity,
    })
}

fn validate_protected_response_descriptor_v5(
    descriptor: &OwnedFd,
) -> Result<(), WorkerV3VerificationCapabilityClientErrorV5> {
    let stat = rustix::fs::fstat(descriptor)
        .map_err(|source| response_descriptor_io("stat protected response", source.into()))?;
    let descriptor_flags = rustix::io::fcntl_getfd(descriptor).map_err(|source| {
        response_descriptor_io("read protected response descriptor flags", source.into())
    })?;
    let status = rustix::fs::fcntl_getfl(descriptor).map_err(|source| {
        response_descriptor_io("read protected response status flags", source.into())
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != 0o400
        || stat.st_nlink != 0
        || stat.st_size <= 0
        || !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
    {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::InvalidResponseDescriptor);
    }
    let seals = rustix::fs::fcntl_get_seals(descriptor)
        .map_err(|source| response_descriptor_io("read protected response seals", source.into()))?;
    let filesystem = rustix::fs::fstatfs(descriptor).map_err(|source| {
        response_descriptor_io("stat protected response filesystem", source.into())
    })?;
    if seals != REQUIRED_PROTECTED_RESPONSE_SEALS_V5 || filesystem.f_type as u64 != TMPFS_MAGIC {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::InvalidResponseDescriptor);
    }
    Ok(())
}

fn authenticate_compiler_response_v5(
    response: &[u8],
    signature: [u8; 64],
) -> Result<[u8; 32], WorkerV3VerificationCapabilityClientErrorV5> {
    let response = InertCompilerCapabilityVerifierResponseV5::decode(response)
        .map_err(WorkerV3VerificationCapabilityClientErrorV5::Artifact)?;
    let authenticated = AuthenticatedCompilerCapabilityCompletionV5::from_signed_verifier_response(
        response, signature,
    )
    .map_err(WorkerV3VerificationCapabilityClientErrorV5::Artifact)?;
    Ok(authenticated.signed_response_identity())
}

struct DecodedProtectedResponseRecordV5<'a> {
    fields: Vec<&'a [u8]>,
    identity: [u8; 32],
}

fn decode_protected_response_record_v5(
    bytes: &[u8],
) -> Result<DecodedProtectedResponseRecordV5<'_>, WorkerV3VerificationCapabilityClientErrorV5> {
    if bytes.len() > MAX_PROTECTED_RESPONSE_BYTES_V5 {
        return Err(
            WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseTooLarge {
                maximum: MAX_PROTECTED_RESPONSE_BYTES_V5,
                actual: bytes.len(),
            },
        );
    }
    if bytes.len() < PROTECTED_RESPONSE_HEADER_BYTES_V5 + PROTECTED_RESPONSE_TERMINAL_BYTES_V5
        || bytes[..8] != PROTECTED_RESPONSE_MAGIC_V5
        || read_u16_v5(bytes, 8)? != PROTECTED_RESPONSE_VERSION_V5
        || read_u16_v5(bytes, 10)? != PROTECTED_RESPONSE_FIELDS_V5
        || read_u32_v5(bytes, 12)? != 0
        || read_u64_v5(bytes, 16)? != bytes.len() as u64
    {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed);
    }
    let payload_end = bytes.len() - PROTECTED_RESPONSE_TERMINAL_BYTES_V5;
    let identity: [u8; 32] = bytes[payload_end..]
        .try_into()
        .map_err(|_| WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)?;
    let mut digest = Sha256::new();
    digest.update(PROTECTED_RESPONSE_IDENTITY_DOMAIN_V5);
    digest.update(&bytes[..payload_end]);
    if <[u8; 32]>::from(digest.finalize()) != identity {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed);
    }
    let mut fields = Vec::with_capacity(PROTECTED_RESPONSE_FIELDS_V5 as usize);
    let mut offset = PROTECTED_RESPONSE_HEADER_BYTES_V5;
    for expected in 1..=PROTECTED_RESPONSE_FIELDS_V5 {
        if read_u16_v5(bytes, offset)? != expected || read_u16_v5(bytes, offset + 2)? != 0 {
            return Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed);
        }
        let length = read_u32_v5(bytes, offset + 4)? as usize;
        offset = offset
            .checked_add(PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5)
            .ok_or(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= payload_end)
            .ok_or(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)?;
        fields.push(&bytes[offset..end]);
        offset = end;
    }
    if offset != payload_end {
        return Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed);
    }
    Ok(DecodedProtectedResponseRecordV5 { fields, identity })
}

fn read_u16_v5(
    bytes: &[u8],
    offset: usize,
) -> Result<u16, WorkerV3VerificationCapabilityClientErrorV5> {
    Ok(u16::from_le_bytes(array_at_v5(bytes, offset)?))
}

fn read_u32_v5(
    bytes: &[u8],
    offset: usize,
) -> Result<u32, WorkerV3VerificationCapabilityClientErrorV5> {
    Ok(u32::from_le_bytes(array_at_v5(bytes, offset)?))
}

fn read_u64_v5(
    bytes: &[u8],
    offset: usize,
) -> Result<u64, WorkerV3VerificationCapabilityClientErrorV5> {
    Ok(u64::from_le_bytes(array_at_v5(bytes, offset)?))
}

fn array_at_v5<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], WorkerV3VerificationCapabilityClientErrorV5> {
    bytes
        .get(offset..offset.saturating_add(N))
        .and_then(|value| value.try_into().ok())
        .ok_or(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)
}

fn response_descriptor_io(
    operation: &'static str,
    source: io::Error,
) -> WorkerV3VerificationCapabilityClientErrorV5 {
    WorkerV3VerificationCapabilityClientErrorV5::ResponseDescriptorIo { operation, source }
}

/// Failed connected-path admission retaining the endpoint for caller quarantine.
pub struct WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
    peer: OwnedFd,
    source: WorkerV3VerificationCapabilityClientErrorV5,
}

impl WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
    pub const fn source_error(&self) -> &WorkerV3VerificationCapabilityClientErrorV5 {
        &self.source
    }

    pub fn into_peer(self) -> OwnedFd {
        self.peer
    }
}

impl fmt::Debug for WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationCapabilityClientAdmissionFailureV5")
            .field("source", &self.source)
            .field("endpoint_retained", &true)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "V5 connected-path admission failed: {}",
            self.source
        )
    }
}

impl Error for WorkerV3VerificationCapabilityClientAdmissionFailureV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationCapabilityExchangeOutcomeV5 {
    Completed(WorkerV3VerificationCapabilityCompletedReceiptV5),
    Rejected(WorkerV3VerificationCapabilityRejectedReceiptV5),
}

/// Move-only, inert transport receipt for a correlated completed response.
///
/// ```compile_fail
/// use fe2o3_worker_v3_verification_client::WorkerV3VerificationCapabilityCompletedReceiptV5;
/// fn duplicate(value: WorkerV3VerificationCapabilityCompletedReceiptV5) {
///     let _again = value.clone();
/// }
/// ```
pub struct WorkerV3VerificationCapabilityCompletedReceiptV5 {
    request: WorkerV3VerificationCapabilityRequestV5,
    response: WorkerV3VerificationCapabilityResponseV5,
    protected_response: Box<[u8]>,
    protected_response_identity: [u8; 32],
    authenticated_response_identity: [u8; 32],
}

impl WorkerV3VerificationCapabilityCompletedReceiptV5 {
    pub const fn request(&self) -> &WorkerV3VerificationCapabilityRequestV5 {
        &self.request
    }
    pub const fn response(&self) -> &WorkerV3VerificationCapabilityResponseV5 {
        &self.response
    }

    /// Identity of the exact canonical protected envelope received from the sealed descriptor.
    pub const fn protected_response_identity(&self) -> [u8; 32] {
        self.protected_response_identity
    }

    /// Identity rederived by the fixed-key verifier from the signed inner response.
    pub const fn authenticated_response_identity(&self) -> [u8; 32] {
        self.authenticated_response_identity
    }

    /// Exact bounded envelope bytes retained for the later authority-owning join boundary.
    pub fn protected_response_bytes(&self) -> &[u8] {
        &self.protected_response
    }

    /// Consumes the inert transport receipt without projecting artifact authority.
    pub fn into_parts(
        self,
    ) -> (
        WorkerV3VerificationCapabilityRequestV5,
        WorkerV3VerificationCapabilityResponseV5,
    ) {
        (self.request, self.response)
    }

    pub const fn grants_artifact_completion_authority(&self) -> bool {
        debug_assert!(!self.response.grants_artifact_completion_authority());
        false
    }
}

impl fmt::Debug for WorkerV3VerificationCapabilityCompletedReceiptV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationCapabilityCompletedReceiptV5")
            .field("request", &self.request.identity())
            .field("response", &self.response.identity())
            .field("protected_response", &self.protected_response_identity)
            .field(
                "authenticated_response",
                &self.authenticated_response_identity,
            )
            .finish_non_exhaustive()
    }
}

/// Move-only, inert transport receipt for a correlated generic rejection.
pub struct WorkerV3VerificationCapabilityRejectedReceiptV5 {
    request: WorkerV3VerificationCapabilityRequestV5,
    response: WorkerV3VerificationCapabilityResponseV5,
}

impl WorkerV3VerificationCapabilityRejectedReceiptV5 {
    pub const fn request(&self) -> &WorkerV3VerificationCapabilityRequestV5 {
        &self.request
    }
    pub const fn response(&self) -> &WorkerV3VerificationCapabilityResponseV5 {
        &self.response
    }

    pub fn into_parts(
        self,
    ) -> (
        WorkerV3VerificationCapabilityRequestV5,
        WorkerV3VerificationCapabilityResponseV5,
    ) {
        (self.request, self.response)
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for WorkerV3VerificationCapabilityRejectedReceiptV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationCapabilityRejectedReceiptV5")
            .field("request", &self.request.identity())
            .field("response", &self.response.identity())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationCapabilityClientErrorV5 {
    V1(WorkerV3VerificationClientErrorV1),
    V2(WorkerV3VerificationClientErrorV2),
    Protocol(WorkerV3VerificationCapabilityProtocolErrorV5),
    InvalidPeerPolicy,
    ServiceConnect(String),
    PeerPolicyMismatch,
    ResponseDescriptorCount {
        expected: usize,
        actual: usize,
    },
    InvalidResponseDescriptor,
    ResponseDescriptorIo {
        operation: &'static str,
        source: io::Error,
    },
    ProtectedResponseTooLarge {
        maximum: usize,
        actual: usize,
    },
    ProtectedResponseRead,
    ProtectedResponseMalformed,
    ResponseMismatch,
    ResponseReplay,
    Artifact(AuthenticatedCompilerCapabilityCompletionErrorV5),
    Quarantine(String),
}

impl fmt::Display for WorkerV3VerificationCapabilityClientErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Worker V3 V5 capability exchange failed: {self:?}"
        )
    }
}

impl Error for WorkerV3VerificationCapabilityClientErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::V1(source) => Some(source),
            Self::V2(source) => Some(source),
            Self::Protocol(source) => Some(source),
            Self::ResponseDescriptorIo { source, .. } => Some(source),
            Self::Artifact(source) => Some(source),
            _ => None,
        }
    }
}

impl From<WorkerV3VerificationCapabilityProtocolErrorV5>
    for WorkerV3VerificationCapabilityClientErrorV5
{
    fn from(source: WorkerV3VerificationCapabilityProtocolErrorV5) -> Self {
        Self::Protocol(source)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{IoSlice, IoSliceMut, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::{AsFd, AsRawFd, OwnedFd};
    use std::path::PathBuf;
    use std::thread;

    use fe2o3_worker_v3_verification_protocol::{
        MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
        WorkerV3VerificationEntryCoordinateV1, WorkerV3VerificationFdPayloadDescriptorV1,
        WorkerV3VerificationFreshChallengeV1, WorkerV3VerificationMeasurementIdentityV1,
        WorkerV3VerificationPolicyIdentityV1, WorkerV3VerificationRosterIdentityV1,
    };
    use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrUnix, SocketFlags,
        SocketType, accept_with, bind, connect, listen, recv, recvmsg, send, sendmsg, socket_with,
        socketpair,
    };

    use super::*;

    const LOAD_BYTES: &[u8] = b"canonical-v2-load-envelope-v5";
    const OBJECT_BYTES: &[u8] = b"canonical-finalized-object-v5";
    const VERIFIER_BYTES: &[u8] = b"fixture-signed-verifier-response-v5";
    const SIGNATURE: [u8; 64] = [91; 64];
    const AUTHENTICATED_IDENTITY: [u8; 32] = [92; 32];
    const POLICY: [u8; 32] = [82; 32];
    const MEASUREMENT: [u8; 32] = [83; 32];
    const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
        .union(SealFlags::GROW)
        .union(SealFlags::SHRINK)
        .union(SealFlags::SEAL);

    #[derive(Clone, Copy)]
    enum ResponseSubject {
        Exact,
        StaleChallenge,
        CrossKernel,
        CrossTarget,
        CrossLaunch,
    }

    struct ReplayGuard {
        admit: bool,
        calls: usize,
    }

    impl WorkerV3VerificationResponseReplayGuardV5 for ReplayGuard {
        fn admit_response_once(
            &mut self,
            _request: WorkerV3VerificationCapabilityRequestIdentityV5,
            _response: WorkerV3VerificationCapabilityResponseIdentityV5,
        ) -> bool {
            self.calls += 1;
            self.admit
        }
    }

    #[derive(Default)]
    struct Quarantine {
        calls: usize,
    }

    impl WorkerV3VerificationClientFailureQuarantineV5 for Quarantine {
        type Error = std::io::Error;

        fn quarantine_failed_exchange(
            &mut self,
            _request: WorkerV3VerificationCapabilityRequestIdentityV5,
            _attempt: WorkerV3VerificationProductionAttemptV5,
        ) -> Result<(), Self::Error> {
            self.calls += 1;
            Ok(())
        }
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn coordinate(seed: u8) -> ExactIdentityCoordinateV5 {
        ExactIdentityCoordinateV5::new([seed; 32], u64::from(seed) + 1).unwrap()
    }

    fn request(
        challenge: u8,
        kernel: u8,
        target: u8,
        launch: u8,
    ) -> WorkerV3VerificationCapabilityRequestV5 {
        let base = WorkerV3VerificationRequestV1::new(
            WorkerV3VerificationFreshChallengeV1::new([challenge; 32]).unwrap(),
            WorkerV3VerificationRosterIdentityV1::new([81; 32]).unwrap(),
            WorkerV3VerificationPolicyIdentityV1::new(POLICY).unwrap(),
            WorkerV3VerificationMeasurementIdentityV1::new(MEASUREMENT).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::protected_completion_evidence_v5(
                LOAD_BYTES.len() as u64,
                sha256(LOAD_BYTES),
            )
            .unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(
                OBJECT_BYTES.len() as u64,
                sha256(OBJECT_BYTES),
            )
            .unwrap(),
            vec![
                WorkerV3VerificationEntryCoordinateV1::new(
                    0,
                    "kernel",
                    "kernel_export",
                    [kernel; 32],
                    [86; 32],
                    [87; 32],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let carriage = WorkerV3VerificationCapabilityCarriageV5::new_exact(
            coordinate(1),
            coordinate(2),
            [3; 32],
            WorkerV3VerificationProductionAttemptV5::new(7, [4; 16], [5; 32]).unwrap(),
            0,
            13,
            coordinate(6),
            9,
            [kernel; 32],
            [8; 32],
            [target; 32],
            [launch; 32],
            [11; 32],
            coordinate(12),
            [13; 32],
            coordinate(14),
            ExactIdentityCoordinateV5::new(sha256(OBJECT_BYTES), OBJECT_BYTES.len() as u64)
                .unwrap(),
            [15; 32],
            POLICY,
            coordinate(16),
        )
        .unwrap();
        WorkerV3VerificationCapabilityRequestV5::new(base, carriage).unwrap()
    }

    fn completed_response(
        request: &WorkerV3VerificationCapabilityRequestV5,
    ) -> WorkerV3VerificationCapabilityResponseV5 {
        let completion = fe2o3_worker_v3_verification_protocol::WorkerV3VerificationCapabilityCompletionV5::new_exact(
            request,
            ExactIdentityCoordinateV5::new(
                sha256(VERIFIER_BYTES),
                VERIFIER_BYTES.len() as u64,
            )
            .unwrap(),
            coordinate(52),
            coordinate(53),
            request.carriage().object_identity(),
            coordinate(54),
            coordinate(55),
        )
        .unwrap();
        WorkerV3VerificationCapabilityResponseV5::completed(request, completion).unwrap()
    }

    fn protected_response(
        request: &WorkerV3VerificationCapabilityRequestV5,
        response: &WorkerV3VerificationCapabilityResponseV5,
        verifier: &[u8],
        signature: [u8; 64],
    ) -> Vec<u8> {
        let request_identity = request.identity();
        let challenge = request.base_request().challenge();
        let fields: [&[u8]; PROTECTED_RESPONSE_FIELDS_V5 as usize] = [
            PROTECTED_RESPONSE_DOMAIN_V5,
            request_identity.as_bytes(),
            challenge.as_bytes(),
            response.encode_canonical(),
            verifier,
            &signature,
        ];
        let body = fields
            .iter()
            .map(|field| PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5 + field.len())
            .sum::<usize>();
        let total =
            PROTECTED_RESPONSE_HEADER_BYTES_V5 + body + PROTECTED_RESPONSE_TERMINAL_BYTES_V5;
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(&PROTECTED_RESPONSE_MAGIC_V5);
        bytes.extend_from_slice(&PROTECTED_RESPONSE_VERSION_V5.to_le_bytes());
        bytes.extend_from_slice(&PROTECTED_RESPONSE_FIELDS_V5.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(total as u64).to_le_bytes());
        for (index, field) in fields.iter().enumerate() {
            bytes.extend_from_slice(&((index + 1) as u16).to_le_bytes());
            bytes.extend_from_slice(&0_u16.to_le_bytes());
            bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
            bytes.extend_from_slice(field);
        }
        let mut digest = Sha256::new();
        digest.update(PROTECTED_RESPONSE_IDENTITY_DOMAIN_V5);
        digest.update(&bytes);
        bytes.extend_from_slice(&digest.finalize());
        bytes
    }

    fn protected_descriptor(bytes: &[u8], seals: SealFlags, read_only: bool) -> OwnedFd {
        named_protected_descriptor(
            "fe2o3-worker-v3-protected-response-client-test",
            bytes,
            seals,
            read_only,
        )
    }

    fn named_protected_descriptor(
        name: &str,
        bytes: &[u8],
        seals: SealFlags,
        read_only: bool,
    ) -> OwnedFd {
        let descriptor =
            rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
                .unwrap();
        let mut writer = File::from(descriptor);
        rustix::fs::fchmod(&writer, Mode::RUSR).unwrap();
        writer.write_all(bytes).unwrap();
        if !seals.is_empty() {
            rustix::fs::fcntl_add_seals(&writer, seals).unwrap();
        }
        if !read_only {
            return writer.into();
        }
        let reader = rustix::fs::open(
            format!("/proc/self/fd/{}", writer.as_raw_fd()),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        drop(writer);
        reader
    }

    fn send_response_with_descriptors(peer: &OwnedFd, packet: &[u8], descriptors: &[&OwnedFd]) {
        if descriptors.is_empty() {
            assert_eq!(
                send(peer, packet, SendFlags::NOSIGNAL).unwrap(),
                packet.len()
            );
            return;
        }
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut ancillary = SendAncillaryBuffer::new(&mut storage);
        let borrowed: Vec<_> = descriptors
            .iter()
            .map(|descriptor| descriptor.as_fd())
            .collect();
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&borrowed)));
        assert_eq!(
            sendmsg(
                peer,
                &[IoSlice::new(packet)],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap(),
            packet.len()
        );
    }

    fn sealed(bytes: &[u8]) -> OwnedFd {
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-worker-v3-client-v5-test",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .unwrap();
        let mut file = File::from(descriptor);
        file.write_all(bytes).unwrap();
        rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS).unwrap();
        file.into()
    }

    fn snapshots(
        request: &WorkerV3VerificationCapabilityRequestV5,
    ) -> WorkerV3VerificationPayloadSnapshotsV1 {
        WorkerV3VerificationPayloadSnapshotsV1::admit(
            request.base_request(),
            vec![sealed(LOAD_BYTES), sealed(OBJECT_BYTES)],
        )
        .unwrap()
    }

    fn connected_path_pair() -> (tempfile::TempDir, PathBuf, OwnedFd, OwnedFd) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("verifier.sock");
        let listener = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        bind(&listener, &SocketAddrUnix::new(&path).unwrap()).unwrap();
        listen(&listener, 1).unwrap();
        let client = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        connect(&client, &SocketAddrUnix::new(&path).unwrap()).unwrap();
        let service = accept_with(&listener, SocketFlags::CLOEXEC).unwrap();
        (root, path, client, service)
    }

    fn receive_request(peer: &OwnedFd) -> WorkerV3VerificationCapabilityRequestV5 {
        let mut bytes = [0_u8; MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5 + 1];
        let mut control_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut control = RecvAncillaryBuffer::new(&mut control_space);
        let received = {
            let mut vectors = [IoSliceMut::new(&mut bytes)];
            recvmsg(
                peer,
                &mut vectors,
                &mut control,
                RecvFlags::CMSG_CLOEXEC | RecvFlags::TRUNC,
            )
            .unwrap()
        };
        assert!(
            !received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        );
        let mut descriptors = Vec::new();
        for message in control.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => panic!("unexpected request ancillary message"),
            }
        }
        assert_eq!(descriptors.len(), 2);
        let decoded =
            WorkerV3VerificationCapabilityRequestV5::decode_canonical(&bytes[..received.bytes])
                .unwrap();
        let mut eof = [0_u8; 1];
        assert_eq!(recv(peer, &mut eof, RecvFlags::empty()).unwrap().0, 0);
        decoded
    }

    fn peer_policy() -> WorkerV3VerificationCapabilityPeerPolicyV5 {
        WorkerV3VerificationCapabilityPeerPolicyV5::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
            POLICY,
            MEASUREMENT,
        )
        .unwrap()
    }

    fn run_response(
        subject: ResponseSubject,
        replay_admit: bool,
    ) -> (
        Result<
            WorkerV3VerificationCapabilityExchangeOutcomeV5,
            WorkerV3VerificationCapabilityClientErrorV5,
        >,
        ReplayGuard,
        Quarantine,
    ) {
        let (_root, path, client_peer, service_peer) = connected_path_pair();
        let client_request = request(80, 85, 89, 90);
        let payloads = snapshots(&client_request);
        let service = thread::spawn(move || {
            let received = receive_request(&service_peer);
            let response_request = match subject {
                ResponseSubject::Exact => received,
                ResponseSubject::StaleChallenge => request(79, 85, 89, 90),
                ResponseSubject::CrossKernel => request(80, 84, 89, 90),
                ResponseSubject::CrossTarget => request(80, 85, 88, 90),
                ResponseSubject::CrossLaunch => request(80, 85, 89, 91),
            };
            let response = completed_response(&response_request);
            let protected =
                protected_response(&response_request, &response, VERIFIER_BYTES, SIGNATURE);
            let descriptor =
                protected_descriptor(&protected, REQUIRED_PROTECTED_RESPONSE_SEALS_V5, true);
            send_response_with_descriptors(
                &service_peer,
                response.encode_canonical(),
                &[&descriptor],
            );
        });
        let client = WorkerV3VerificationCapabilityClientV5::admit_connected_path(
            client_peer,
            &path,
            peer_policy(),
            Duration::from_secs(2),
        )
        .unwrap();
        let custody = IntakeAuthenticatedWorkerV3CapabilityCustodyV5 {
            request: client_request,
        };
        let mut replay = ReplayGuard {
            admit: replay_admit,
            calls: 0,
        };
        let mut quarantine = Quarantine::default();
        let result = client.exchange_with_authenticator(
            custody,
            payloads,
            &mut replay,
            &mut quarantine,
            |response, signature| {
                assert_eq!(response, VERIFIER_BYTES);
                assert_eq!(signature, SIGNATURE);
                Ok(AUTHENTICATED_IDENTITY)
            },
        );
        service.join().unwrap();
        (result, replay, quarantine)
    }

    #[derive(Clone, Copy)]
    enum HostileTransfer {
        ZeroDescriptors,
        MultipleDescriptors,
        UnsealedDescriptor,
        WritableDescriptor,
        WrongDescriptorType,
        MismatchedEnvelope,
        TrailingControl,
    }

    fn run_hostile_transfer(
        transfer: HostileTransfer,
    ) -> (
        Result<
            WorkerV3VerificationCapabilityExchangeOutcomeV5,
            WorkerV3VerificationCapabilityClientErrorV5,
        >,
        ReplayGuard,
        Quarantine,
    ) {
        let (_root, path, client_peer, service_peer) = connected_path_pair();
        let client_request = request(80, 85, 89, 90);
        let payloads = snapshots(&client_request);
        let service = thread::spawn(move || {
            let received = receive_request(&service_peer);
            let response = completed_response(&received);
            let envelope_request = if matches!(transfer, HostileTransfer::MismatchedEnvelope) {
                request(79, 85, 89, 90)
            } else {
                request(80, 85, 89, 90)
            };
            let protected =
                protected_response(&envelope_request, &response, VERIFIER_BYTES, SIGNATURE);
            match transfer {
                HostileTransfer::ZeroDescriptors => {
                    send_response_with_descriptors(&service_peer, response.encode_canonical(), &[])
                }
                HostileTransfer::MultipleDescriptors => {
                    let first = named_protected_descriptor(
                        "fe2o3-client-v5-multiple-first",
                        &protected,
                        REQUIRED_PROTECTED_RESPONSE_SEALS_V5,
                        true,
                    );
                    let second = named_protected_descriptor(
                        "fe2o3-client-v5-multiple-second",
                        &protected,
                        REQUIRED_PROTECTED_RESPONSE_SEALS_V5,
                        true,
                    );
                    send_response_with_descriptors(
                        &service_peer,
                        response.encode_canonical(),
                        &[&first, &second],
                    );
                }
                HostileTransfer::UnsealedDescriptor => {
                    let descriptor = named_protected_descriptor(
                        "fe2o3-client-v5-unsealed",
                        &protected,
                        SealFlags::empty(),
                        true,
                    );
                    send_response_with_descriptors(
                        &service_peer,
                        response.encode_canonical(),
                        &[&descriptor],
                    );
                }
                HostileTransfer::WritableDescriptor => {
                    let descriptor = named_protected_descriptor(
                        "fe2o3-client-v5-writable",
                        &protected,
                        REQUIRED_PROTECTED_RESPONSE_SEALS_V5,
                        false,
                    );
                    send_response_with_descriptors(
                        &service_peer,
                        response.encode_canonical(),
                        &[&descriptor],
                    );
                }
                HostileTransfer::WrongDescriptorType => {
                    let (descriptor, _other) = socketpair(
                        AddressFamily::UNIX,
                        SocketType::STREAM,
                        SocketFlags::CLOEXEC,
                        None,
                    )
                    .unwrap();
                    send_response_with_descriptors(
                        &service_peer,
                        response.encode_canonical(),
                        &[&descriptor],
                    );
                }
                HostileTransfer::MismatchedEnvelope => {
                    let descriptor = named_protected_descriptor(
                        "fe2o3-client-v5-mismatch",
                        &protected,
                        REQUIRED_PROTECTED_RESPONSE_SEALS_V5,
                        true,
                    );
                    send_response_with_descriptors(
                        &service_peer,
                        response.encode_canonical(),
                        &[&descriptor],
                    );
                }
                HostileTransfer::TrailingControl => {
                    let descriptor = named_protected_descriptor(
                        "fe2o3-client-v5-trailing-main",
                        &protected,
                        REQUIRED_PROTECTED_RESPONSE_SEALS_V5,
                        true,
                    );
                    send_response_with_descriptors(
                        &service_peer,
                        response.encode_canonical(),
                        &[&descriptor],
                    );
                    let trailing = named_protected_descriptor(
                        "fe2o3-client-v5-trailing-extra",
                        b"trailing",
                        REQUIRED_PROTECTED_RESPONSE_SEALS_V5,
                        true,
                    );
                    send_response_with_descriptors(&service_peer, b"x", &[&trailing]);
                }
            }
        });
        let client = WorkerV3VerificationCapabilityClientV5::admit_connected_path(
            client_peer,
            &path,
            peer_policy(),
            Duration::from_secs(2),
        )
        .unwrap();
        let mut replay = ReplayGuard {
            admit: true,
            calls: 0,
        };
        let mut quarantine = Quarantine::default();
        let result = client.exchange_with_authenticator(
            IntakeAuthenticatedWorkerV3CapabilityCustodyV5 {
                request: client_request,
            },
            payloads,
            &mut replay,
            &mut quarantine,
            |response, signature| {
                assert_eq!(response, VERIFIER_BYTES);
                assert_eq!(signature, SIGNATURE);
                Ok(AUTHENTICATED_IDENTITY)
            },
        );
        service.join().unwrap();
        (result, replay, quarantine)
    }

    fn assert_no_received_memfd_leak(names: &[&str]) {
        let open_targets: Vec<_> = std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read_link(entry.path()).ok())
            .map(|target| target.to_string_lossy().into_owned())
            .collect();
        for name in names {
            assert!(
                open_targets.iter().all(|target| !target.contains(name)),
                "received descriptor leaked for {name}: {open_targets:?}"
            );
        }
    }

    #[test]
    fn connected_path_accepts_exact_completed_response_only_as_inert_transport() {
        let (result, replay, quarantine) = run_response(ResponseSubject::Exact, true);
        let WorkerV3VerificationCapabilityExchangeOutcomeV5::Completed(receipt) = result.unwrap()
        else {
            panic!("exact response did not complete the transport exchange");
        };
        assert!(!receipt.grants_artifact_completion_authority());
        assert_eq!(
            receipt.authenticated_response_identity(),
            AUTHENTICATED_IDENTITY
        );
        assert_eq!(
            decode_protected_response_record_v5(receipt.protected_response_bytes())
                .unwrap()
                .identity,
            receipt.protected_response_identity()
        );
        assert_eq!(replay.calls, 1);
        assert_eq!(quarantine.calls, 0);
    }

    #[test]
    fn completed_response_requires_exactly_one_descriptor() {
        for (transfer, expected, actual) in [
            (HostileTransfer::ZeroDescriptors, 1, 0),
            (HostileTransfer::MultipleDescriptors, 1, 2),
        ] {
            let (result, replay, quarantine) = run_hostile_transfer(transfer);
            assert!(matches!(
                result,
                Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseDescriptorCount {
                    expected: observed_expected,
                    actual: observed_actual,
                }) if observed_expected == expected && observed_actual == actual
            ));
            assert_eq!(replay.calls, 0);
            assert_eq!(quarantine.calls, 1);
        }
        assert_no_received_memfd_leak(&[
            "fe2o3-client-v5-multiple-first",
            "fe2o3-client-v5-multiple-second",
        ]);
    }

    #[test]
    fn completed_response_rejects_unsealed_writable_and_wrong_type_descriptors() {
        for transfer in [
            HostileTransfer::UnsealedDescriptor,
            HostileTransfer::WritableDescriptor,
            HostileTransfer::WrongDescriptorType,
        ] {
            let (result, replay, quarantine) = run_hostile_transfer(transfer);
            assert!(matches!(
                result,
                Err(WorkerV3VerificationCapabilityClientErrorV5::InvalidResponseDescriptor)
            ));
            assert_eq!(replay.calls, 0);
            assert_eq!(quarantine.calls, 1);
        }
        assert_no_received_memfd_leak(&["fe2o3-client-v5-unsealed", "fe2o3-client-v5-writable"]);
    }

    #[test]
    fn completed_response_rejects_mismatched_envelope_and_trailing_control() {
        let (mismatch, replay, quarantine) =
            run_hostile_transfer(HostileTransfer::MismatchedEnvelope);
        assert!(matches!(
            mismatch,
            Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch)
        ));
        assert_eq!(replay.calls, 0);
        assert_eq!(quarantine.calls, 1);

        let (trailing, replay, quarantine) = run_hostile_transfer(HostileTransfer::TrailingControl);
        assert!(matches!(
            trailing,
            Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::UnexpectedAncillaryData
            ))
        ));
        assert_eq!(replay.calls, 0);
        assert_eq!(quarantine.calls, 1);
        assert_no_received_memfd_leak(&[
            "fe2o3-client-v5-mismatch",
            "fe2o3-client-v5-trailing-main",
            "fe2o3-client-v5-trailing-extra",
        ]);
    }

    #[test]
    fn production_authenticator_rejects_malformed_signed_response() {
        assert!(matches!(
            authenticate_compiler_response_v5(b"not-a-verifier-response", SIGNATURE),
            Err(WorkerV3VerificationCapabilityClientErrorV5::Artifact(_))
        ));
    }

    #[test]
    fn protected_response_requires_exact_terminal_and_inner_response_identities() {
        let request = request(80, 85, 89, 90);
        let response = completed_response(&request);
        let mut mutated = protected_response(&request, &response, VERIFIER_BYTES, SIGNATURE);
        let terminal = mutated.len() - PROTECTED_RESPONSE_TERMINAL_BYTES_V5;
        mutated[terminal] ^= 1;
        assert!(matches!(
            decode_protected_response_record_v5(&mutated),
            Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseMalformed)
        ));

        let mismatched = protected_response(&request, &response, b"different", SIGNATURE);
        let descriptor =
            protected_descriptor(&mismatched, REQUIRED_PROTECTED_RESPONSE_SEALS_V5, true);
        assert!(matches!(
            read_and_authenticate_protected_response_v5(
                descriptor,
                response.encode_canonical(),
                &request,
                |_, _| Ok(AUTHENTICATED_IDENTITY),
            ),
            Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch)
        ));
    }

    #[test]
    fn protected_response_size_is_checked_before_allocation_or_read() {
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-client-v5-oversized",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .unwrap();
        let writer = File::from(descriptor);
        rustix::fs::fchmod(&writer, Mode::RUSR).unwrap();
        rustix::fs::ftruncate(&writer, (MAX_PROTECTED_RESPONSE_BYTES_V5 + 1) as u64).unwrap();
        rustix::fs::fcntl_add_seals(&writer, REQUIRED_PROTECTED_RESPONSE_SEALS_V5).unwrap();
        let reader = rustix::fs::open(
            format!("/proc/self/fd/{}", writer.as_raw_fd()),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        drop(writer);
        let request = request(80, 85, 89, 90);
        let response = completed_response(&request);
        assert!(matches!(
            read_and_authenticate_protected_response_v5(
                reader,
                response.encode_canonical(),
                &request,
                |_, _| panic!("oversized descriptor must fail before authentication"),
            ),
            Err(WorkerV3VerificationCapabilityClientErrorV5::ProtectedResponseTooLarge {
                maximum: MAX_PROTECTED_RESPONSE_BYTES_V5,
                actual,
            }) if actual == MAX_PROTECTED_RESPONSE_BYTES_V5 + 1
        ));
        assert_no_received_memfd_leak(&["fe2o3-client-v5-oversized"]);
    }

    #[test]
    fn stale_and_cross_subject_responses_fail_before_replay_admission() {
        for subject in [
            ResponseSubject::StaleChallenge,
            ResponseSubject::CrossKernel,
            ResponseSubject::CrossTarget,
            ResponseSubject::CrossLaunch,
        ] {
            let (result, replay, quarantine) = run_response(subject, true);
            assert!(matches!(
                result,
                Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseMismatch)
            ));
            assert_eq!(replay.calls, 0);
            assert_eq!(quarantine.calls, 1);
        }
    }

    #[test]
    fn replayed_response_is_quarantined_after_endpoint_and_correlation_checks() {
        let (result, replay, quarantine) = run_response(ResponseSubject::Exact, false);
        assert!(matches!(
            result,
            Err(WorkerV3VerificationCapabilityClientErrorV5::ResponseReplay)
        ));
        assert_eq!(replay.calls, 1);
        assert_eq!(quarantine.calls, 1);
    }

    #[test]
    fn generic_rejection_remains_descriptor_free_and_is_quarantined() {
        let (_root, path, client_peer, service_peer) = connected_path_pair();
        let request = request(80, 85, 89, 90);
        let payloads = snapshots(&request);
        let service = thread::spawn(move || {
            let received = receive_request(&service_peer);
            let response = WorkerV3VerificationCapabilityResponseV5::rejected(&received).unwrap();
            send_response_with_descriptors(&service_peer, response.encode_canonical(), &[]);
        });
        let client = WorkerV3VerificationCapabilityClientV5::admit_connected_path(
            client_peer,
            &path,
            peer_policy(),
            Duration::from_secs(2),
        )
        .unwrap();
        let mut replay = ReplayGuard {
            admit: true,
            calls: 0,
        };
        let mut quarantine = Quarantine::default();
        let result = client.exchange_with_authenticator(
            IntakeAuthenticatedWorkerV3CapabilityCustodyV5 { request },
            payloads,
            &mut replay,
            &mut quarantine,
            |_, _| panic!("rejection must not invoke protected-response authentication"),
        );
        service.join().unwrap();
        assert!(matches!(
            result,
            Ok(WorkerV3VerificationCapabilityExchangeOutcomeV5::Rejected(_))
        ));
        assert_eq!(replay.calls, 1);
        assert_eq!(quarantine.calls, 1);
    }

    #[test]
    fn daemon_crash_before_response_is_quarantined() {
        let (_root, path, client_peer, service_peer) = connected_path_pair();
        let request = request(80, 85, 89, 90);
        let payloads = snapshots(&request);
        let service = thread::spawn(move || drop(receive_request(&service_peer)));
        let client = WorkerV3VerificationCapabilityClientV5::admit_connected_path(
            client_peer,
            &path,
            peer_policy(),
            Duration::from_secs(2),
        )
        .unwrap();
        let mut replay = ReplayGuard {
            admit: true,
            calls: 0,
        };
        let mut quarantine = Quarantine::default();
        let result = client.exchange(
            IntakeAuthenticatedWorkerV3CapabilityCustodyV5 { request },
            payloads,
            &mut replay,
            &mut quarantine,
        );
        service.join().unwrap();
        assert!(matches!(
            result,
            Err(WorkerV3VerificationCapabilityClientErrorV5::V2(
                WorkerV3VerificationClientErrorV2::PeerClosed
            ))
        ));
        assert_eq!(replay.calls, 0);
        assert_eq!(quarantine.calls, 1);
    }

    #[test]
    fn response_delivered_before_daemon_exit_remains_correlated_but_not_authoritative() {
        let (result, replay, quarantine) = run_response(ResponseSubject::Exact, true);
        let WorkerV3VerificationCapabilityExchangeOutcomeV5::Completed(receipt) = result.unwrap()
        else {
            panic!("delivered response did not complete transport");
        };
        let (request, response) = receipt.into_parts();
        assert!(response.matches_request(&request));
        assert_eq!(replay.calls, 1);
        assert_eq!(quarantine.calls, 0);
    }

    #[test]
    fn pinned_policy_and_measurement_mismatch_fail_before_transfer_and_quarantine() {
        for (policy, measurement) in [([81; 32], MEASUREMENT), (POLICY, [84; 32])] {
            let (_root, path, client_peer, service_peer) = connected_path_pair();
            let request = request(80, 85, 89, 90);
            let payloads = snapshots(&request);
            let service = thread::spawn(move || {
                let mut byte = [0_u8; 1];
                assert_eq!(
                    recv(&service_peer, &mut byte, RecvFlags::empty())
                        .unwrap()
                        .0,
                    0
                );
            });
            let peer_policy = WorkerV3VerificationCapabilityPeerPolicyV5::new(
                rustix::process::geteuid().as_raw(),
                rustix::process::getegid().as_raw(),
                policy,
                measurement,
            )
            .unwrap();
            let client = WorkerV3VerificationCapabilityClientV5::admit_connected_path(
                client_peer,
                &path,
                peer_policy,
                Duration::from_secs(2),
            )
            .unwrap();
            let mut replay = ReplayGuard {
                admit: true,
                calls: 0,
            };
            let mut quarantine = Quarantine::default();
            let result = client.exchange(
                IntakeAuthenticatedWorkerV3CapabilityCustodyV5 { request },
                payloads,
                &mut replay,
                &mut quarantine,
            );
            service.join().unwrap();
            assert!(matches!(
                result,
                Err(WorkerV3VerificationCapabilityClientErrorV5::PeerPolicyMismatch)
            ));
            assert_eq!(replay.calls, 0);
            assert_eq!(quarantine.calls, 1);
        }
    }
}
