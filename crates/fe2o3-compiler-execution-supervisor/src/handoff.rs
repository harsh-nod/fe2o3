//! Authenticated receipt of one exact rustc session from Cargo.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
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
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SocketAddrUnix, SocketType, recvmsg,
};

use crate::{ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1};

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
    loop {
        wait_readable(control, deadline)?;
        let mut payload = [0_u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1];
        let result = {
            let mut vectors = [IoSliceMut::new(&mut payload)];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut space);
            match recvmsg(
                control,
                &mut vectors,
                &mut ancillary,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(received) => {
                    let invalid_packet = received.bytes != payload.len()
                        || received
                            .flags
                            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC);
                    let mut descriptors = Vec::with_capacity(2);
                    let mut invalid_ancillary = false;
                    for message in ancillary.drain() {
                        match message {
                            RecvAncillaryMessage::ScmRights(received) => {
                                descriptors.extend(received);
                            }
                            _ => invalid_ancillary = true,
                        }
                    }
                    if invalid_packet || invalid_ancillary || descriptors.len() != 2 {
                        Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer)
                    } else {
                        let client_pidfd = descriptors.pop().expect("length checked");
                        let service_peer = descriptors.pop().expect("length checked");
                        Ok((service_peer, client_pidfd))
                    }
                }
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
                Err(error) => return Err(ProtectedIssuerHandoffErrorV1::Io(error.into())),
            }
        }?;
        return Ok((payload, result.0, result.1));
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
    if !has_cloexec(control)? {
        return Err(ProtectedIssuerHandoffErrorV1::InvalidControl(
            "control descriptor is inheritable",
        ));
    }
    if rustix::net::sockopt::socket_domain(control).map_err(io::Error::from)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(control).map_err(io::Error::from)?
            != SocketType::SEQPACKET
    {
        return Err(ProtectedIssuerHandoffErrorV1::InvalidControl(
            "endpoint is not Unix SOCK_SEQPACKET",
        ));
    }
    let local = rustix::net::getsockname(control).map_err(io::Error::from)?;
    let remote = rustix::net::getpeername(control).map_err(io::Error::from)?;
    if local.address_family() != AddressFamily::UNIX
        || remote
            .as_ref()
            .is_none_or(|address| address.address_family() != AddressFamily::UNIX)
    {
        return Err(ProtectedIssuerHandoffErrorV1::InvalidControl(
            "endpoint is not connected within AF_UNIX",
        ));
    }
    Ok(())
}

fn control_peer_identity(
    control: &OwnedFd,
) -> Result<CompilerExecutionClientProcessIdentityV1, ProtectedIssuerHandoffErrorV1> {
    let credentials = rustix::net::sockopt::socket_peercred(control).map_err(io::Error::from)?;
    let pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
        .map_err(|_| ProtectedIssuerHandoffErrorV1::SubmitterCredentialsMismatch)?;
    CompilerExecutionClientProcessIdentityV1::new(
        pid,
        credentials.uid.as_raw(),
        credentials.gid.as_raw(),
    )
    .map_err(|_| ProtectedIssuerHandoffErrorV1::SubmitterCredentialsMismatch)
}

pub(super) fn validate_service_peer(
    peer: &impl AsFd,
    expected: CompilerExecutionClientProcessIdentityV1,
) -> Result<(), ProtectedIssuerHandoffErrorV1> {
    if !has_cloexec(peer)? {
        return Err(ProtectedIssuerHandoffErrorV1::InvalidServicePeer);
    }
    let before = descriptor_snapshot(peer)?;
    if before.mode & libc::S_IFMT != libc::S_IFSOCK
        || rustix::net::sockopt::socket_domain(peer).map_err(io::Error::from)?
            != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(peer).map_err(io::Error::from)?
            != SocketType::SEQPACKET
    {
        return Err(ProtectedIssuerHandoffErrorV1::InvalidServicePeer);
    }
    let local = rustix::net::getsockname(peer).map_err(io::Error::from)?;
    let remote = rustix::net::getpeername(peer).map_err(io::Error::from)?;
    let unnamed = rustix::net::SocketAddrAny::from(SocketAddrUnix::new_unnamed());
    if local != unnamed || remote.as_ref() != Some(&unnamed) {
        return Err(ProtectedIssuerHandoffErrorV1::InvalidServicePeer);
    }
    let credentials = rustix::net::sockopt::socket_peercred(peer).map_err(io::Error::from)?;
    let pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
        .map_err(|_| ProtectedIssuerHandoffErrorV1::ServicePeerCredentialsMismatch)?;
    if (pid, credentials.uid.as_raw(), credentials.gid.as_raw())
        != (expected.pid(), expected.uid(), expected.gid())
    {
        return Err(ProtectedIssuerHandoffErrorV1::ServicePeerCredentialsMismatch);
    }
    if descriptor_snapshot(peer)? != before {
        return Err(ProtectedIssuerHandoffErrorV1::DescriptorChanged);
    }
    Ok(())
}

fn has_cloexec(descriptor: &impl AsFd) -> Result<bool, ProtectedIssuerHandoffErrorV1> {
    let flags = rustix::io::fcntl_getfd(descriptor).map_err(io::Error::from)?;
    Ok(flags.contains(rustix::io::FdFlags::CLOEXEC))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DescriptorSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
}

fn descriptor_snapshot(
    descriptor: &impl AsFd,
) -> Result<DescriptorSnapshotV1, ProtectedIssuerHandoffErrorV1> {
    let stat = rustix::fs::fstat(descriptor).map_err(io::Error::from)?;
    Ok(DescriptorSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
    })
}

fn require_distinct(
    control: DescriptorSnapshotV1,
    service_peer: DescriptorSnapshotV1,
    client_pidfd: DescriptorSnapshotV1,
) -> Result<(), ProtectedIssuerHandoffErrorV1> {
    let control_key = (control.device, control.inode);
    let service_key = (service_peer.device, service_peer.inode);
    let pidfd_key = (client_pidfd.device, client_pidfd.inode);
    if control_key == service_key || control_key == pidfd_key || service_key == pidfd_key {
        return Err(ProtectedIssuerHandoffErrorV1::DescriptorAlias);
    }
    Ok(())
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
