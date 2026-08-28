//! One-use transfer of exact rustc launch inputs to the protected supervisor.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSlice};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV1, CompilerExecutionServiceLaunchManifestV1,
    CompilerExecutionSupervisorHandoffErrorV1, CompilerExecutionSupervisorHandoffV1,
};
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketType, sendmsg,
};

use crate::{CompilerExecutionChildChannelErrorV1, CompilerExecutionServiceLaunchV1};

const INVALID_ID: u32 = u32::MAX;

/// Exact expected credential identity of the dedicated protected-supervisor peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionSupervisorCredentialsV1 {
    uid: u32,
    gid: u32,
}

impl CompilerExecutionSupervisorCredentialsV1 {
    /// Constructs one dedicated non-root supervisor credential identity.
    pub const fn new(uid: u32, gid: u32) -> Result<Self, CompilerExecutionHandoffErrorV1> {
        if uid == 0 || uid == INVALID_ID {
            return Err(CompilerExecutionHandoffErrorV1::InvalidSupervisorUid);
        }
        if gid == 0 || gid == INVALID_ID {
            return Err(CompilerExecutionHandoffErrorV1::InvalidSupervisorGid);
        }
        Ok(Self { uid, gid })
    }

    /// Returns the expected protected-supervisor effective UID.
    pub const fn uid(self) -> u32 {
        self.uid
    }

    /// Returns the expected protected-supervisor effective GID.
    pub const fn gid(self) -> u32 {
        self.gid
    }
}

/// Opaque one-use control connection after the rustc descriptors have been transferred.
///
/// This value carries no signing, publication, loading, or execution authority. A subsequent
/// operation will consume it while authenticating issuer readiness on the same connection.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::PendingCompilerExecutionSupervisorV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PendingCompilerExecutionSupervisorV1>();
/// ```
pub struct PendingCompilerExecutionSupervisorV1 {
    control: OwnedFd,
    handoff: CompilerExecutionSupervisorHandoffV1,
}

impl fmt::Debug for PendingCompilerExecutionSupervisorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _retained_control = self.control.as_fd();
        formatter
            .debug_struct("PendingCompilerExecutionSupervisorV1")
            .field("authority", &"none")
            .field("handoff", &self.handoff.identity())
            .finish_non_exhaustive()
    }
}

impl PendingCompilerExecutionSupervisorV1 {
    /// Returns the exact launch manifest whose descriptors were transferred.
    pub const fn manifest(&self) -> &CompilerExecutionServiceLaunchManifestV1 {
        self.handoff.launch_manifest()
    }
}

impl CompilerExecutionServiceLaunchV1 {
    /// Transfers this exact live rustc session to one authenticated protected supervisor.
    ///
    /// `control` must be a connected Unix `SOCK_SEQPACKET` endpoint whose kernel-reported peer
    /// UID and GID match `expected_supervisor`. The single canonical packet binds the exact direct
    /// parent to the launch manifest and carries exactly two ordered descriptors: service peer,
    /// then rustc pidfd.
    pub fn transfer_to_supervisor(
        self,
        control: OwnedFd,
        expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        timeout: Duration,
    ) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
        transfer_to_supervisor_inner::<true>(self, control, expected_supervisor, policy, timeout)
    }
}

fn transfer_to_supervisor_inner<const REQUIRE_DISTINCT_UID: bool>(
    launch: CompilerExecutionServiceLaunchV1,
    control: OwnedFd,
    expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    timeout: Duration,
) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
    if timeout.is_zero() {
        return Err(CompilerExecutionHandoffErrorV1::InvalidTimeout);
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(CompilerExecutionHandoffErrorV1::DeadlineOverflow)?;
    if REQUIRE_DISTINCT_UID && launch.client().uid() == expected_supervisor.uid {
        return Err(CompilerExecutionHandoffErrorV1::ClientAndSupervisorUidMatch);
    }
    validate_control(&control, expected_supervisor)?;
    launch
        .revalidate_for_supervisor_handoff()
        .map_err(CompilerExecutionHandoffErrorV1::RustcLaunch)?;
    let manifest = CompilerExecutionServiceLaunchManifestV1::new(launch.client(), policy);
    let handoff = CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), manifest)
        .map_err(CompilerExecutionHandoffErrorV1::CanonicalHandoff)?;
    let (service_peer, client_pidfd) = launch.into_descriptors();
    let descriptors = [service_peer.as_fd(), client_pidfd.as_fd()];
    send_handoff(&control, handoff.canonical_bytes(), &descriptors, deadline)?;
    Ok(PendingCompilerExecutionSupervisorV1 { control, handoff })
}

fn validate_control(
    control: &OwnedFd,
    expected: CompilerExecutionSupervisorCredentialsV1,
) -> Result<(), CompilerExecutionHandoffErrorV1> {
    let flags = rustix::io::fcntl_getfd(control).map_err(io::Error::from)?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
            "descriptor is inheritable",
        ));
    }
    if rustix::net::sockopt::socket_domain(control).map_err(io::Error::from)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(control).map_err(io::Error::from)?
            != SocketType::SEQPACKET
    {
        return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
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
        return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
            "endpoint is not connected within AF_UNIX",
        ));
    }
    let credentials = rustix::net::sockopt::socket_peercred(control).map_err(io::Error::from)?;
    if credentials.uid.as_raw() != expected.uid || credentials.gid.as_raw() != expected.gid {
        return Err(CompilerExecutionHandoffErrorV1::SupervisorCredentialsMismatch);
    }
    Ok(())
}

fn send_handoff(
    control: &OwnedFd,
    payload: &[u8],
    descriptors: &[std::os::fd::BorrowedFd<'_>; 2],
    deadline: Instant,
) -> Result<(), CompilerExecutionHandoffErrorV1> {
    loop {
        wait_writable(control, deadline)?;
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
            return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
                "SCM_RIGHTS buffer is too small",
            ));
        }
        match sendmsg(
            control,
            &[IoSlice::new(payload)],
            &mut ancillary,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        ) {
            Ok(sent) if sent == payload.len() => return Ok(()),
            Ok(_) => return Err(CompilerExecutionHandoffErrorV1::PartialSend),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(error) => return Err(CompilerExecutionHandoffErrorV1::Io(error.into())),
        }
    }
}

fn wait_writable(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<(), CompilerExecutionHandoffErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionHandoffErrorV1::Timeout);
        }
        let mut descriptor = libc::pollfd {
            fd: control.as_fd().as_raw_fd(),
            events: libc::POLLOUT | libc::POLLERR | libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: descriptor is a live one-element pollfd array for the complete call.
        let result = unsafe { libc::poll(&mut descriptor, 1, duration_to_poll_millis(remaining)) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(CompilerExecutionHandoffErrorV1::Io(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionHandoffErrorV1::Timeout);
        }
        if descriptor.revents & libc::POLLNVAL != 0 {
            return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
                "descriptor became invalid",
            ));
        }
        if descriptor.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Err(CompilerExecutionHandoffErrorV1::ControlClosed);
        }
        if descriptor.revents & libc::POLLOUT != 0 {
            return Ok(());
        }
    }
}

fn duration_to_poll_millis(duration: Duration) -> i32 {
    let millis = duration.as_millis();
    let rounded = if duration.subsec_nanos().is_multiple_of(1_000_000) {
        millis
    } else {
        millis.saturating_add(1)
    };
    rounded.clamp(1, i32::MAX as u128) as i32
}

/// Stable failure transferring one exact rustc session to the protected supervisor.
#[derive(Debug)]
pub enum CompilerExecutionHandoffErrorV1 {
    /// The configured protected-supervisor UID is privileged or invalid.
    InvalidSupervisorUid,
    /// The configured protected-supervisor GID is privileged or invalid.
    InvalidSupervisorGid,
    /// A production handoff attempted to use the rustc client's UID as the service UID.
    ClientAndSupervisorUidMatch,
    /// The complete handoff timeout is zero.
    InvalidTimeout,
    /// The monotonic deadline cannot be represented.
    DeadlineOverflow,
    /// The control endpoint has an invalid descriptor or socket property.
    InvalidControl(&'static str),
    /// `SO_PEERCRED` does not name the configured protected-supervisor UID and GID.
    SupervisorCredentialsMismatch,
    /// The retained rustc launch inputs failed repeat validation.
    RustcLaunch(CompilerExecutionChildChannelErrorV1),
    /// Canonical direct-parent handoff construction or validation failed.
    CanonicalHandoff(CompilerExecutionSupervisorHandoffErrorV1),
    /// The absolute handoff deadline expired.
    Timeout,
    /// The control endpoint closed or failed before the atomic transfer.
    ControlClosed,
    /// A `SOCK_SEQPACKET` send did not consume the complete canonical record.
    PartialSend,
    /// A bounded descriptor or socket operation failed.
    Io(io::Error),
}

impl fmt::Display for CompilerExecutionHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSupervisorUid => formatter.write_str("invalid protected-supervisor UID"),
            Self::InvalidSupervisorGid => formatter.write_str("invalid protected-supervisor GID"),
            Self::ClientAndSupervisorUidMatch => {
                formatter.write_str("rustc client and protected supervisor use the same UID")
            }
            Self::InvalidTimeout => formatter.write_str("supervisor handoff timeout is zero"),
            Self::DeadlineOverflow => formatter.write_str("supervisor handoff deadline overflowed"),
            Self::InvalidControl(reason) => {
                write!(formatter, "invalid supervisor control: {reason}")
            }
            Self::SupervisorCredentialsMismatch => {
                formatter.write_str("supervisor control peer credentials do not match policy")
            }
            Self::RustcLaunch(error) => write!(formatter, "rustc launch changed: {error}"),
            Self::CanonicalHandoff(error) => {
                write!(formatter, "invalid canonical supervisor handoff: {error}")
            }
            Self::Timeout => formatter.write_str("supervisor handoff deadline expired"),
            Self::ControlClosed => formatter.write_str("supervisor control endpoint closed"),
            Self::PartialSend => {
                formatter.write_str("supervisor handoff packet was partially sent")
            }
            Self::Io(error) => write!(formatter, "supervisor handoff operation failed: {error}"),
        }
    }
}

impl Error for CompilerExecutionHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RustcLaunch(error) => Some(error),
            Self::CanonicalHandoff(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for CompilerExecutionHandoffErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use std::io::IoSliceMut;
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;
    use std::process::Command;

    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SocketFlags,
        socketpair,
    };

    use super::*;
    use crate::PendingCompilerExecutionChildChannelV1;

    fn policy() -> CompilerExecutionIssuerPolicyV1 {
        use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
        CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 1).unwrap(),
            [3; 32],
        )
        .unwrap()
    }

    fn control_pair() -> (OwnedFd, OwnedFd) {
        socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap()
    }

    #[test]
    fn exact_manifest_and_two_ordered_descriptors_transfer_once() {
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let pending_child = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
        let mut child = command.spawn().unwrap();
        let launch = pending_child
            .finish(child.id(), Duration::from_secs(2))
            .unwrap();
        let client = launch.client();
        let submitter = launch.submitter();
        let (sender, receiver) = control_pair();
        let credentials = CompilerExecutionSupervisorCredentialsV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let expected_policy = policy();
        let transferred = transfer_to_supervisor_inner::<false>(
            launch,
            sender,
            credentials,
            &expected_policy,
            Duration::from_secs(2),
        )
        .unwrap();

        let mut payload = [0_u8;
            fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1];
        let mut vectors = [IoSliceMut::new(&mut payload)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = rustix::net::recvmsg(
            &receiver,
            &mut vectors,
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC,
        )
        .unwrap();
        assert_eq!(received.bytes, payload.len());
        let descriptors: Vec<_> = ancillary
            .drain()
            .flat_map(|message| match message {
                RecvAncillaryMessage::ScmRights(descriptors) => descriptors.collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        assert_eq!(descriptors.len(), 2);
        let handoff = CompilerExecutionSupervisorHandoffV1::decode(&payload).unwrap();
        assert_eq!(handoff.submitter(), submitter);
        assert_eq!(handoff.launch_manifest().client(), client);
        assert_eq!(
            handoff.launch_manifest().policy_identity(),
            expected_policy.identity()
        );
        assert_eq!(transferred.manifest(), handoff.launch_manifest());
        assert_eq!(
            rustix::net::sockopt::socket_type(&descriptors[0]).unwrap(),
            SocketType::SEQPACKET
        );
        assert!(rustix::fs::fstat(&descriptors[1]).unwrap().st_ino != 0);
        assert!(descriptors.iter().all(|descriptor| {
            rustix::io::fcntl_getfd(descriptor)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        }));

        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn production_transfer_rejects_same_uid_and_wrong_control_shapes() {
        let current_uid = rustix::process::geteuid().as_raw();
        let current_gid = rustix::process::getegid().as_raw();
        if current_uid == 0 || current_gid == 0 {
            return;
        }
        let credentials =
            CompilerExecutionSupervisorCredentialsV1::new(current_uid, current_gid).unwrap();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let pending_child = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
        let mut child = command.spawn().unwrap();
        let launch = pending_child
            .finish(child.id(), Duration::from_secs(2))
            .unwrap();
        let (sender, _receiver) = control_pair();
        assert!(matches!(
            launch.transfer_to_supervisor(sender, credentials, &policy(), Duration::from_secs(2)),
            Err(CompilerExecutionHandoffErrorV1::ClientAndSupervisorUidMatch)
        ));
        child.kill().unwrap();
        child.wait().unwrap();

        let stream = socketpair(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        assert!(matches!(
            validate_control(&stream.0, credentials),
            Err(CompilerExecutionHandoffErrorV1::InvalidControl(_))
        ));
        rustix::io::fcntl_setfd(&stream.1, rustix::io::FdFlags::empty()).unwrap();
        assert!(matches!(
            validate_control(&stream.1, credentials),
            Err(CompilerExecutionHandoffErrorV1::InvalidControl(
                "descriptor is inheritable"
            ))
        ));
        let wrong_uid = if current_uid == 1 { 2 } else { 1 };
        let wrong_credentials =
            CompilerExecutionSupervisorCredentialsV1::new(wrong_uid, current_gid).unwrap();
        let (sender, _receiver) = control_pair();
        assert!(matches!(
            validate_control(&sender, wrong_credentials),
            Err(CompilerExecutionHandoffErrorV1::SupervisorCredentialsMismatch)
        ));
        let _ = stream.0.as_raw_fd();
    }
}
