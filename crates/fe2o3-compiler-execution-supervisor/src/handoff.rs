//! Authenticated receipt of one exact rustc session from Cargo.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::os::fd::{AsFd, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1, ProtectedServiceAdmissionErrorV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1, CompilerExecutionClientProcessIdentityV1,
    CompilerExecutionServiceLaunchManifestV1, CompilerExecutionSupervisorHandoffErrorV1,
    CompilerExecutionSupervisorHandoffV1,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};

use crate::handoff_ancillary::Guard;
use crate::handoff_checks::{self as checks, Snapshot as DescriptorSnapshotV1};
use crate::{ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1};

mod application;
pub use application::{
    AcceptedApplicationHandoffV1, AcceptedCustodianApplicationHandoffV1,
    ProtectedApplicationHandoffErrorV1,
};

/// Move-only, fully admitted rustc descriptors and their authenticated control connection.
///
/// The value exposes no descriptor, signing, receipt, publication, loading, or execution API. It
/// can only be consumed by the protected issuer launch path in this package.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<AcceptedCompilerExecutionHandoffV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<AcceptedCompilerExecutionHandoffV1>();
/// ```
pub struct AcceptedCompilerExecutionHandoffV1 {
    pub(super) control: OwnedFd,
    pub(super) handoff: CompilerExecutionSupervisorHandoffV1,
    pub(super) service_peer: OwnedFd,
    pub(super) client_pidfd: OwnedFd,
    live_client: LiveClientPidfdIdentityV1,
    control_snapshot: DescriptorSnapshotV1,
    service_peer_snapshot: DescriptorSnapshotV1,
    client_pidfd_snapshot: DescriptorSnapshotV1,
}

impl fmt::Debug for AcceptedCompilerExecutionHandoffV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptedCompilerExecutionHandoffV1")
            .field("authority", &"none")
            .field("handoff", &self.handoff.identity())
            .finish_non_exhaustive()
    }
}

impl AcceptedCompilerExecutionHandoffV1 {
    /// Returns the exact canonical issuer launch manifest without exposing descriptor custody.
    pub const fn manifest(&self) -> &CompilerExecutionServiceLaunchManifestV1 {
        self.handoff.launch_manifest()
    }

    /// Returns the kernel-reported control submitter identity.
    pub const fn submitter(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.handoff.submitter()
    }

    /// Repeats supervisor, policy, control, service-peer, pidfd, and descriptor continuity checks.
    pub fn revalidate(
        &self,
        supervisor: &ProtectedIssuerSupervisorV1,
    ) -> Result<(), ProtectedIssuerHandoffErrorV1> {
        supervisor
            .revalidate()
            .map_err(ProtectedIssuerHandoffErrorV1::Supervisor)?;
        if !self
            .handoff
            .launch_manifest()
            .matches_policy(supervisor.policy())
        {
            return Err(ProtectedIssuerHandoffErrorV1::PolicyMismatch);
        }
        if !self
            .handoff
            .launch_manifest()
            .matches_external_anchor_service(supervisor.external_anchor_service())
        {
            return Err(ProtectedIssuerHandoffErrorV1::ExternalAnchorServiceMismatch);
        }
        if self.handoff.launch_manifest().client().pid()
            == supervisor.external_anchor_process().pid()
        {
            return Err(ProtectedIssuerHandoffErrorV1::ClientAndExternalAnchorProcessMatch);
        }
        if descriptor_snapshot(&self.control)? != self.control_snapshot
            || descriptor_snapshot(&self.service_peer)? != self.service_peer_snapshot
            || descriptor_snapshot(&self.client_pidfd)? != self.client_pidfd_snapshot
        {
            return Err(ProtectedIssuerHandoffErrorV1::DescriptorChanged);
        }
        require_distinct(
            self.control_snapshot,
            self.service_peer_snapshot,
            self.client_pidfd_snapshot,
        )?;
        let current_submitter = control_peer_identity(&self.control)?;
        if current_submitter != self.handoff.submitter() {
            return Err(ProtectedIssuerHandoffErrorV1::SubmitterCredentialsMismatch);
        }
        validate_service_peer(&self.service_peer, self.handoff.launch_manifest().client())?;
        self.live_client
            .validate_liveness()
            .map_err(ProtectedIssuerHandoffErrorV1::Pidfd)
    }

    pub(super) fn into_control(self) -> OwnedFd {
        self.control
    }
}

impl ProtectedIssuerSupervisorV1 {
    /// Receives and authenticates one exact Cargo-owned rustc launch handoff.
    ///
    /// The one canonical packet must carry exactly two ordered `SCM_RIGHTS` descriptors: the
    /// rustc service peer followed by its pidfd. The control submitter must share the rustc UID and
    /// GID while both remain distinct from this protected service UID.
    pub fn accept_handoff(
        &self,
        control: OwnedFd,
        timeout: Duration,
    ) -> Result<AcceptedCompilerExecutionHandoffV1, ProtectedIssuerHandoffErrorV1> {
        self.accept_handoff_inner::<true>(control, timeout)
    }

    pub(crate) fn accept_handoff_inner<const REQUIRE_DISTINCT_UID: bool>(
        &self,
        control: OwnedFd,
        timeout: Duration,
    ) -> Result<AcceptedCompilerExecutionHandoffV1, ProtectedIssuerHandoffErrorV1> {
        if timeout.is_zero() {
            return Err(ProtectedIssuerHandoffErrorV1::InvalidTimeout);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ProtectedIssuerHandoffErrorV1::DeadlineOverflow)?;
        self.revalidate()
            .map_err(ProtectedIssuerHandoffErrorV1::Supervisor)?;
        validate_control_shape(&control)?;
        let submitter = control_peer_identity(&control)?;
        if REQUIRE_DISTINCT_UID && submitter.uid() == self.credentials().uid() {
            return Err(ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch);
        }
        let control_snapshot = descriptor_snapshot(&control)?;
        let (payload, service_peer, client_pidfd) = receive_handoff(&control, deadline)?;
        let handoff = CompilerExecutionSupervisorHandoffV1::decode(&payload)
            .map_err(ProtectedIssuerHandoffErrorV1::CanonicalHandoff)?;
        self.admit_received_handoff::<REQUIRE_DISTINCT_UID>(
            control,
            handoff,
            service_peer,
            client_pidfd,
            control_snapshot,
        )
    }

    fn admit_received_handoff<const REQUIRE_DISTINCT_UID: bool>(
        &self,
        control: OwnedFd,
        handoff: CompilerExecutionSupervisorHandoffV1,
        service_peer: OwnedFd,
        client_pidfd: OwnedFd,
        control_snapshot: DescriptorSnapshotV1,
    ) -> Result<AcceptedCompilerExecutionHandoffV1, ProtectedIssuerHandoffErrorV1> {
        self.revalidate()
            .map_err(ProtectedIssuerHandoffErrorV1::Supervisor)?;
        validate_control_shape(&control)?;
        let submitter = control_peer_identity(&control)?;
        if REQUIRE_DISTINCT_UID && submitter.uid() == self.credentials().uid() {
            return Err(ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch);
        }
        if !handoff.launch_manifest().matches_policy(self.policy()) {
            return Err(ProtectedIssuerHandoffErrorV1::PolicyMismatch);
        }
        if !handoff
            .launch_manifest()
            .matches_external_anchor_service(self.external_anchor_service())
        {
            return Err(ProtectedIssuerHandoffErrorV1::ExternalAnchorServiceMismatch);
        }
        if submitter != handoff.submitter() {
            return Err(ProtectedIssuerHandoffErrorV1::SubmitterCredentialsMismatch);
        }
        let client = handoff.launch_manifest().client();
        if client.pid() == self.external_anchor_process().pid() {
            return Err(ProtectedIssuerHandoffErrorV1::ClientAndExternalAnchorProcessMatch);
        }
        let service_peer_snapshot = descriptor_snapshot(&service_peer)?;
        let client_pidfd_snapshot = descriptor_snapshot(&client_pidfd)?;
        require_distinct(
            control_snapshot,
            service_peer_snapshot,
            client_pidfd_snapshot,
        )?;
        validate_service_peer(&service_peer, client)?;
        let pidfd_for_validation = rustix::io::fcntl_dupfd_cloexec(&client_pidfd, 0)
            .map_err(|error| ProtectedIssuerHandoffErrorV1::Io(error.into()))?;
        let expected =
            ExpectedClientProcessIdentityV1::new(client.pid(), client.uid(), client.gid())
                .map_err(ProtectedIssuerHandoffErrorV1::Pidfd)?;
        let live_client = LiveClientPidfdIdentityV1::admit(pidfd_for_validation, expected)
            .map_err(ProtectedIssuerHandoffErrorV1::Pidfd)?;
        self.revalidate()
            .map_err(ProtectedIssuerHandoffErrorV1::Supervisor)?;
        let accepted = AcceptedCompilerExecutionHandoffV1 {
            control,
            handoff,
            service_peer,
            client_pidfd,
            live_client,
            control_snapshot,
            service_peer_snapshot,
            client_pidfd_snapshot,
        };
        accepted.revalidate(self)?;
        Ok(accepted)
    }
}

fn receive_handoff(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<
    (
        [u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1],
        OwnedFd,
        OwnedFd,
    ),
    ProtectedIssuerHandoffErrorV1,
> {
    match receive_profile(control, deadline)? {
        ReceivedHandoffV1::Compiler(bytes, [peer, pidfd]) => Ok((bytes, peer, pidfd)),
        ReceivedHandoffV1::Application(..) | ReceivedHandoffV1::CustodianApplication(..) => {
            Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer)
        }
    }
}

enum ReceivedHandoffV1 {
    CustodianApplication(
        Box<[u8; fe2o3_runtime_protocol::WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1]>,
        [OwnedFd; 4],
    ),
    Compiler(
        [u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1],
        [OwnedFd; 2],
    ),
    Application(
        Box<[u8; fe2o3_runtime_protocol::WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1]>,
        [OwnedFd; 4],
    ),
}

pub(crate) enum AcceptedHandoffV1 {
    CustodianApplication(Box<AcceptedCustodianApplicationHandoffV1>),
    Compiler(Box<AcceptedCompilerExecutionHandoffV1>),
    Application(Box<AcceptedApplicationHandoffV1>),
}

impl ProtectedIssuerSupervisorV1 {
    pub(crate) fn accept_profile_until<const REQUIRE_DISTINCT_UID: bool>(
        &self,
        control: OwnedFd,
        deadline: Instant,
    ) -> Result<AcceptedHandoffV1, crate::ProtectedIssuerSessionErrorV1> {
        use crate::ProtectedIssuerSessionErrorV1 as E;
        let receive = || -> Result<_, ProtectedIssuerHandoffErrorV1> {
            self.revalidate()
                .map_err(ProtectedIssuerHandoffErrorV1::Supervisor)?;
            validate_control_shape(&control)?;
            if REQUIRE_DISTINCT_UID
                && control_peer_identity(&control)?.uid() == self.credentials().uid()
            {
                return Err(ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch);
            }
            let snapshot = descriptor_snapshot(&control)?;
            let packet = receive_profile(&control, deadline)?;
            Ok((snapshot, packet))
        };
        let (snapshot, packet) = receive().map_err(E::Handoff)?;
        match packet {
            ReceivedHandoffV1::CustodianApplication(bytes, rights) => {
                let handoff =
                    fe2o3_runtime_protocol::WorkerV3ApplicationCustodianHandoffV1::decode(&*bytes)
                        .map_err(|e| {
                            E::ApplicationHandoff(
                                ProtectedApplicationHandoffErrorV1::CustodianRoute(e),
                            )
                        })?;
                self.admit_received_custodian_application::<REQUIRE_DISTINCT_UID>(
                    control, handoff, rights, snapshot, deadline,
                )
                .map(|accepted| AcceptedHandoffV1::CustodianApplication(Box::new(accepted)))
                .map_err(E::ApplicationHandoff)
            }
            ReceivedHandoffV1::Compiler(bytes, [peer, pidfd]) => {
                let handoff = CompilerExecutionSupervisorHandoffV1::decode(&bytes)
                    .map_err(|e| E::Handoff(ProtectedIssuerHandoffErrorV1::CanonicalHandoff(e)))?;
                self.admit_received_handoff::<REQUIRE_DISTINCT_UID>(
                    control, handoff, peer, pidfd, snapshot,
                )
                .map(|accepted| AcceptedHandoffV1::Compiler(Box::new(accepted)))
                .map_err(E::Handoff)
            }
            ReceivedHandoffV1::Application(bytes, rights) => {
                let binding =
                    fe2o3_runtime_protocol::WorkerV3ApplicationRegistrationBindingV1::decode(
                        &*bytes,
                    )
                    .map_err(|e| {
                        E::ApplicationHandoff(ProtectedApplicationHandoffErrorV1::Binding(e))
                    })?;
                self.admit_received_application::<REQUIRE_DISTINCT_UID>(
                    control, binding, rights, snapshot, deadline,
                )
                .map(|accepted| AcceptedHandoffV1::Application(Box::new(accepted)))
                .map_err(E::ApplicationHandoff)
            }
        }
    }
}

// A packet is consumed exactly once. Profile selection never peeks or retries another receiver.
fn receive_profile(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<ReceivedHandoffV1, ProtectedIssuerHandoffErrorV1> {
    use fe2o3_runtime_protocol::WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1 as CUSTODIAN_BYTES;
    use fe2o3_runtime_protocol::WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1 as APP_BYTES;
    loop {
        wait_readable(control, deadline)?;
        let mut payload = [0; CUSTODIAN_BYTES];
        let mut vectors = [IoSliceMut::new(&mut payload)];
        let mut guard = Guard::new();
        let (space, armed) = guard.parts();
        let mut ancillary = RecvAncillaryBuffer::new(space);
        let received = recvmsg(
            control,
            &mut vectors,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        );
        *armed = received.is_ok();
        let received = match received {
            Ok(value) => value,
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(error) => return Err(ProtectedIssuerHandoffErrorV1::Io(error.into())),
        };
        let mut rights = Vec::with_capacity(5);
        let mut invalid = false;
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(received) => rights.extend(received),
                _ => invalid = true,
            }
        }
        drop(ancillary);
        invalid |= guard.finish();
        if received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || invalid
        {
            return Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer);
        }
        if Instant::now() >= deadline {
            return Err(ProtectedIssuerHandoffErrorV1::Timeout);
        }
        return match (received.bytes, rights.len()) {
            (COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1, 2) => Ok(ReceivedHandoffV1::Compiler(
                payload[..COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1]
                    .try_into()
                    .unwrap(),
                rights.try_into().unwrap(),
            )),
            (APP_BYTES, 4) => Ok(ReceivedHandoffV1::Application(
                Box::new(payload[..APP_BYTES].try_into().unwrap()),
                rights.try_into().unwrap(),
            )),
            (CUSTODIAN_BYTES, 4) => Ok(ReceivedHandoffV1::CustodianApplication(
                Box::new(payload),
                rights.try_into().unwrap(),
            )),
            _ => Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer),
        };
    }
}

fn wait_readable(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<(), ProtectedIssuerHandoffErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProtectedIssuerHandoffErrorV1::Timeout);
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(remaining.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(remaining.subsec_nanos()),
        };
        let mut descriptors = [PollFd::new(
            control,
            PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
        )];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) => return Err(ProtectedIssuerHandoffErrorV1::Timeout),
            Ok(_) => {
                let events = descriptors[0].revents();
                if events.contains(PollFlags::NVAL) {
                    return Err(ProtectedIssuerHandoffErrorV1::InvalidControl(
                        "descriptor became invalid",
                    ));
                }
                if events.intersects(PollFlags::ERR | PollFlags::HUP)
                    && !events.contains(PollFlags::IN)
                {
                    return Err(ProtectedIssuerHandoffErrorV1::ControlClosed);
                }
                if events.contains(PollFlags::IN) {
                    return Ok(());
                }
            }
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(ProtectedIssuerHandoffErrorV1::Io(error.into())),
        }
    }
}

fn validate_control_shape(control: &OwnedFd) -> Result<(), ProtectedIssuerHandoffErrorV1> {
    checks::control_shape(control).map_err(Into::into)
}

fn control_peer_identity(
    control: &OwnedFd,
) -> Result<CompilerExecutionClientProcessIdentityV1, ProtectedIssuerHandoffErrorV1> {
    checks::control_peer(control).map_err(Into::into)
}

pub(super) fn validate_service_peer(
    peer: &impl AsFd,
    expected: CompilerExecutionClientProcessIdentityV1,
) -> Result<(), ProtectedIssuerHandoffErrorV1> {
    checks::service_peer(peer, expected).map_err(Into::into)
}

fn descriptor_snapshot(
    descriptor: &impl AsFd,
) -> Result<DescriptorSnapshotV1, ProtectedIssuerHandoffErrorV1> {
    checks::snapshot(descriptor).map_err(Into::into)
}

fn require_distinct(
    control: DescriptorSnapshotV1,
    service_peer: DescriptorSnapshotV1,
    client_pidfd: DescriptorSnapshotV1,
) -> Result<(), ProtectedIssuerHandoffErrorV1> {
    checks::distinct(control, service_peer, client_pidfd).map_err(Into::into)
}

impl From<checks::Failure> for ProtectedIssuerHandoffErrorV1 {
    fn from(error: checks::Failure) -> Self {
        match error {
            checks::Failure::InvalidControl(reason) => Self::InvalidControl(reason),
            checks::Failure::SubmitterCredentialsMismatch => Self::SubmitterCredentialsMismatch,
            checks::Failure::InvalidServicePeer => Self::InvalidServicePeer,
            checks::Failure::ServicePeerCredentialsMismatch => Self::ServicePeerCredentialsMismatch,
            checks::Failure::DescriptorChanged => Self::DescriptorChanged,
            checks::Failure::DescriptorAlias => Self::DescriptorAlias,
            checks::Failure::Io(errno) => Self::Io(errno.into()),
        }
    }
}

/// Stable failure admitting one exact cross-process rustc launch handoff.
#[derive(Debug)]
pub enum ProtectedIssuerHandoffErrorV1 {
    /// Bound supervisor authority changed or is no longer valid.
    Supervisor(ProtectedIssuerSupervisorErrorV1),
    /// The complete handoff timeout is zero.
    InvalidTimeout,
    /// The monotonic deadline cannot be represented.
    DeadlineOverflow,
    /// The control endpoint has an invalid descriptor or socket property.
    InvalidControl(&'static str),
    /// Production rustc and supervisor processes use the same UID.
    ClientAndSupervisorUidMatch,
    /// The control submitter does not match the rustc identity or is rustc itself.
    SubmitterCredentialsMismatch,
    /// The canonical transfer packet or ancillary descriptor set is malformed.
    MalformedTransfer,
    /// The canonical direct-parent handoff is invalid.
    CanonicalHandoff(CompilerExecutionSupervisorHandoffErrorV1),
    /// The manifest does not name the supervisor's caller-pinned policy.
    PolicyMismatch,
    /// The manifest does not name the supervisor-provisioned external-anchor service.
    ExternalAnchorServiceMismatch,
    /// The rustc client and external-anchor endpoint name the same process.
    ClientAndExternalAnchorProcessMatch,
    /// The transferred service endpoint has the wrong socket shape.
    InvalidServicePeer,
    /// The transferred service endpoint does not name the exact rustc process.
    ServicePeerCredentialsMismatch,
    /// The transferred pidfd does not name the exact live rustc process.
    Pidfd(ProtectedServiceAdmissionErrorV1),
    /// A control, service-peer, or pidfd descriptor aliases another role.
    DescriptorAlias,
    /// A retained descriptor identity changed after admission.
    DescriptorChanged,
    /// The absolute handoff deadline expired.
    Timeout,
    /// The control connection closed before one complete packet arrived.
    ControlClosed,
    /// A bounded descriptor or socket operation failed.
    Io(io::Error),
}

impl fmt::Display for ProtectedIssuerHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supervisor(error) => write!(formatter, "protected supervisor changed: {error}"),
            Self::InvalidTimeout => formatter.write_str("supervisor handoff timeout is zero"),
            Self::DeadlineOverflow => formatter.write_str("supervisor handoff deadline overflowed"),
            Self::InvalidControl(reason) => write!(formatter, "invalid handoff control: {reason}"),
            Self::ClientAndSupervisorUidMatch => {
                formatter.write_str("rustc client and protected supervisor use the same UID")
            }
            Self::SubmitterCredentialsMismatch => {
                formatter.write_str("handoff submitter credentials do not match the rustc client")
            }
            Self::MalformedTransfer => formatter.write_str("rustc handoff packet is malformed"),
            Self::CanonicalHandoff(error) => {
                write!(formatter, "invalid canonical rustc handoff: {error}")
            }
            Self::PolicyMismatch => {
                formatter.write_str("rustc launch manifest names another issuer policy")
            }
            Self::ExternalAnchorServiceMismatch => {
                formatter.write_str("rustc launch manifest names another external-anchor service")
            }
            Self::ClientAndExternalAnchorProcessMatch => formatter
                .write_str("rustc client and external-anchor service name the same process"),
            Self::InvalidServicePeer => {
                formatter.write_str("rustc service endpoint has the wrong socket shape")
            }
            Self::ServicePeerCredentialsMismatch => {
                formatter.write_str("rustc service endpoint names another process")
            }
            Self::Pidfd(error) => write!(formatter, "rustc pidfd admission failed: {error}"),
            Self::DescriptorAlias => formatter.write_str("handoff descriptors alias roles"),
            Self::DescriptorChanged => formatter.write_str("a retained handoff descriptor changed"),
            Self::Timeout => formatter.write_str("supervisor handoff deadline expired"),
            Self::ControlClosed => formatter.write_str("supervisor handoff control closed"),
            Self::Io(error) => write!(formatter, "supervisor handoff operation failed: {error}"),
        }
    }
}

impl Error for ProtectedIssuerHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Supervisor(error) => Some(error),
            Self::CanonicalHandoff(error) => Some(error),
            Self::Pidfd(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProtectedIssuerHandoffErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
