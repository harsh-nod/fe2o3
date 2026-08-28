//! One-use transfer of exact rustc launch inputs to the protected supervisor.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
    CompilerExecutionClientProfileV1, CompilerExecutionExternalAnchorServiceIdentityV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionServiceLaunchManifestV1,
    CompilerExecutionServiceReadyErrorV1, CompilerExecutionServiceReadyV1,
    CompilerExecutionSupervisorHandoffErrorV1, CompilerExecutionSupervisorHandoffV1,
};
use rustix::fs::OFlags;
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrAny, SocketAddrUnix,
    SocketFlags, SocketType, connect, recv, recvmsg, sendmsg, socket_with,
};

use crate::{CompilerExecutionChildChannelErrorV1, CompilerExecutionServiceLaunchV1};

const INVALID_ID: u32 = u32::MAX;

/// Maximum connect-and-transfer bound accepted by the production supervisor client.
pub const MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1: Duration = Duration::from_secs(120);

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
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_client::PendingCompilerExecutionSupervisorV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<PendingCompilerExecutionSupervisorV1>();
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

    /// Consumes the control connection and admits one exact supervisor readiness acknowledgment.
    pub fn await_readiness(
        self,
        profile: &CompilerExecutionClientProfileV1,
        timeout: Duration,
    ) -> Result<CompilerExecutionServiceReadyV1, CompilerExecutionHandoffErrorV1> {
        validate_boundary_timeout(timeout)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionHandoffErrorV1::DeadlineOverflow)?;
        self.await_readiness_until(profile, deadline)
    }

    /// Consumes the control connection and admits readiness before an absolute deadline.
    pub fn await_readiness_until(
        self,
        profile: &CompilerExecutionClientProfileV1,
        deadline: Instant,
    ) -> Result<CompilerExecutionServiceReadyV1, CompilerExecutionHandoffErrorV1> {
        validate_boundary_deadline(deadline)?;
        let policy = profile.policy();
        if !self.handoff.launch_manifest().matches_policy(policy)
            || !self
                .handoff
                .launch_manifest()
                .matches_external_anchor_service(profile.external_anchor_service())
        {
            return Err(CompilerExecutionHandoffErrorV1::ReadinessMismatch);
        }
        let bytes = receive_readiness(&self.control, deadline)?;
        let readiness = CompilerExecutionServiceReadyV1::decode(&bytes)
            .map_err(CompilerExecutionHandoffErrorV1::ReadinessProtocol)?;
        if !readiness.matches_launch(
            readiness.issuer_pid(),
            self.handoff.launch_manifest(),
            policy,
        ) {
            return Err(CompilerExecutionHandoffErrorV1::ReadinessMismatch);
        }
        require_control_eof(&self.control, deadline)?;
        require_deadline(deadline)?;
        Ok(readiness)
    }
}

impl CompilerExecutionServiceLaunchV1 {
    /// Connects to and transfers this exact live rustc session to the production supervisor.
    ///
    /// The control endpoint is created internally as nonblocking Unix `SOCK_SEQPACKET`, connects
    /// only to [`COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1`], and must report the configured
    /// dedicated UID and GID through `SO_PEERCRED`. One absolute monotonic deadline covers both
    /// connect and transfer. The canonical packet binds the exact direct parent to the launch
    /// manifest and carries exactly two ordered descriptors: service peer, then rustc pidfd.
    ///
    /// Supplying an alternate path or preconnected descriptor is deliberately not part of the
    /// production API.
    ///
    /// ```compile_fail
    /// use std::os::fd::OwnedFd;
    /// use std::time::Duration;
    /// use fe2o3_compiler_execution_client::CompilerExecutionServiceLaunchV1;
    /// use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1;
    /// fn inject(
    ///     launch: CompilerExecutionServiceLaunchV1,
    ///     control: OwnedFd,
    ///     profile: &CompilerExecutionClientProfileV1,
    /// ) {
    ///     let _ = launch.transfer_to_supervisor(
    ///         control, profile, Duration::from_secs(1),
    ///     );
    /// }
    /// ```
    pub fn transfer_to_supervisor(
        self,
        profile: &CompilerExecutionClientProfileV1,
        timeout: Duration,
    ) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
        validate_boundary_timeout(timeout)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionHandoffErrorV1::DeadlineOverflow)?;
        self.transfer_to_supervisor_until(profile, deadline)
    }

    /// Transfers this exact client session to the supervisor before an absolute deadline.
    pub fn transfer_to_supervisor_until(
        self,
        profile: &CompilerExecutionClientProfileV1,
        deadline: Instant,
    ) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
        let expected_supervisor = CompilerExecutionSupervisorCredentialsV1::new(
            profile.supervisor_uid(),
            profile.supervisor_gid(),
        )?;
        transfer_to_supervisor_at_until_inner::<true>(
            self,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
            expected_supervisor,
            profile.external_anchor_service(),
            profile.policy(),
            deadline,
        )
    }
}

#[cfg(test)]
fn transfer_to_supervisor_at_inner<const REQUIRE_DISTINCT_UID: bool>(
    launch: CompilerExecutionServiceLaunchV1,
    path: &Path,
    expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    timeout: Duration,
) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
    validate_boundary_timeout(timeout)?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(CompilerExecutionHandoffErrorV1::DeadlineOverflow)?;
    transfer_to_supervisor_at_until_inner::<REQUIRE_DISTINCT_UID>(
        launch,
        path,
        expected_supervisor,
        external_anchor_service,
        policy,
        deadline,
    )
}

fn transfer_to_supervisor_at_until_inner<const REQUIRE_DISTINCT_UID: bool>(
    launch: CompilerExecutionServiceLaunchV1,
    path: &Path,
    expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    deadline: Instant,
) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
    validate_boundary_deadline(deadline)?;
    if REQUIRE_DISTINCT_UID && launch.client().uid() == expected_supervisor.uid {
        return Err(CompilerExecutionHandoffErrorV1::ClientAndSupervisorUidMatch);
    }
    let control = connect_to_supervisor(path, expected_supervisor, deadline)?;
    transfer_to_supervisor_inner::<REQUIRE_DISTINCT_UID>(
        launch,
        control,
        expected_supervisor,
        external_anchor_service,
        policy,
        deadline,
    )
}

fn transfer_to_supervisor_inner<const REQUIRE_DISTINCT_UID: bool>(
    launch: CompilerExecutionServiceLaunchV1,
    control: OwnedFd,
    expected_supervisor: CompilerExecutionSupervisorCredentialsV1,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    deadline: Instant,
) -> Result<PendingCompilerExecutionSupervisorV1, CompilerExecutionHandoffErrorV1> {
    validate_control(&control, expected_supervisor)?;
    require_deadline(deadline)?;
    launch
        .revalidate_for_supervisor_handoff()
        .map_err(CompilerExecutionHandoffErrorV1::RustcLaunch)?;
    let manifest = CompilerExecutionServiceLaunchManifestV1::new(
        launch.client(),
        external_anchor_service,
        policy,
    );
    let handoff = CompilerExecutionSupervisorHandoffV1::new(launch.submitter(), manifest)
        .map_err(CompilerExecutionHandoffErrorV1::CanonicalHandoff)?;
    let (service_peer, client_pidfd) = launch.into_descriptors();
    let descriptors = [service_peer.as_fd(), client_pidfd.as_fd()];
    send_handoff(&control, handoff.canonical_bytes(), &descriptors, deadline)?;
    require_deadline(deadline)?;
    Ok(PendingCompilerExecutionSupervisorV1 { control, handoff })
}

fn validate_boundary_timeout(timeout: Duration) -> Result<(), CompilerExecutionHandoffErrorV1> {
    if timeout.is_zero() || timeout > MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1 {
        return Err(CompilerExecutionHandoffErrorV1::InvalidTimeout);
    }
    Ok(())
}

fn validate_boundary_deadline(deadline: Instant) -> Result<(), CompilerExecutionHandoffErrorV1> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(CompilerExecutionHandoffErrorV1::Timeout);
    }
    if remaining > MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1 {
        return Err(CompilerExecutionHandoffErrorV1::InvalidTimeout);
    }
    Ok(())
}

fn connect_to_supervisor(
    path: &Path,
    expected: CompilerExecutionSupervisorCredentialsV1,
    deadline: Instant,
) -> Result<OwnedFd, CompilerExecutionHandoffErrorV1> {
    require_deadline(deadline)?;
    let address = SocketAddrUnix::new(path).map_err(io::Error::from)?;
    let expected_address = SocketAddrAny::from(address.clone());
    let control = socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .map_err(io::Error::from)?;
    connect_with_deadline(&control, &address, deadline)?;
    validate_production_control(&control, expected, &expected_address)?;
    require_deadline(deadline)?;
    Ok(control)
}

fn connect_with_deadline(
    control: &OwnedFd,
    address: &SocketAddrUnix,
    deadline: Instant,
) -> Result<(), CompilerExecutionHandoffErrorV1> {
    loop {
        require_deadline(deadline)?;
        match connect(control, address) {
            Ok(()) | Err(rustix::io::Errno::ISCONN) => return Ok(()),
            Err(rustix::io::Errno::INTR) => {}
            Err(
                rustix::io::Errno::INPROGRESS
                | rustix::io::Errno::ALREADY
                | rustix::io::Errno::AGAIN,
            ) => {
                wait_for_connect_event(control, deadline)?;
                match rustix::net::sockopt::socket_error(control).map_err(io::Error::from)? {
                    Ok(()) => {}
                    Err(source) => {
                        return Err(CompilerExecutionHandoffErrorV1::Io(source.into()));
                    }
                }
                match rustix::net::getpeername(control) {
                    Ok(Some(_)) => return Ok(()),
                    Ok(None) | Err(rustix::io::Errno::NOTCONN) => {}
                    Err(source) => {
                        return Err(CompilerExecutionHandoffErrorV1::Io(source.into()));
                    }
                }
            }
            Err(source) => return Err(CompilerExecutionHandoffErrorV1::Io(source.into())),
        }
    }
}

fn wait_for_connect_event(
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
                "descriptor became invalid while connecting",
            ));
        }
        if descriptor.revents & (libc::POLLOUT | libc::POLLERR | libc::POLLHUP) != 0 {
            return Ok(());
        }
    }
}

fn validate_production_control(
    control: &OwnedFd,
    expected_credentials: CompilerExecutionSupervisorCredentialsV1,
    expected_address: &SocketAddrAny,
) -> Result<(), CompilerExecutionHandoffErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(control).map_err(io::Error::from)?;
    let status = rustix::fs::fcntl_getfl(control).map_err(io::Error::from)?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDWR
        || !status.contains(OFlags::NONBLOCK)
        || status.intersects(forbidden)
    {
        return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
            "descriptor flags are not exact nonblocking close-on-exec custody",
        ));
    }
    validate_control(control, expected_credentials)?;
    if rustix::net::sockopt::socket_acceptconn(control).map_err(io::Error::from)? {
        return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
            "control endpoint is a listener",
        ));
    }
    let unnamed = SocketAddrAny::from(SocketAddrUnix::new_unnamed());
    let local = rustix::net::getsockname(control).map_err(io::Error::from)?;
    let remote = rustix::net::getpeername(control).map_err(io::Error::from)?;
    if local != unnamed || remote.as_ref() != Some(expected_address) {
        return Err(CompilerExecutionHandoffErrorV1::InvalidControl(
            "control endpoint is not connected from unnamed local custody to the fixed pathname",
        ));
    }
    match rustix::net::sockopt::socket_error(control).map_err(io::Error::from)? {
        Ok(()) => Ok(()),
        Err(source) => Err(CompilerExecutionHandoffErrorV1::Io(source.into())),
    }
}

fn require_deadline(deadline: Instant) -> Result<(), CompilerExecutionHandoffErrorV1> {
    if deadline.saturating_duration_since(Instant::now()).is_zero() {
        return Err(CompilerExecutionHandoffErrorV1::Timeout);
    }
    Ok(())
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
    if credentials.pid.as_raw_pid() <= 0 {
        return Err(CompilerExecutionHandoffErrorV1::InvalidSupervisorPid);
    }
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

fn receive_readiness(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<[u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1], CompilerExecutionHandoffErrorV1> {
    loop {
        wait_readable(control, deadline)?;
        let mut bytes = [0_u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1];
        let received = {
            let mut vectors = [IoSliceMut::new(&mut bytes)];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut space);
            match recvmsg(
                control,
                &mut vectors,
                &mut ancillary,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(received) => {
                    let mut has_ancillary = false;
                    for message in ancillary.drain() {
                        match message {
                            RecvAncillaryMessage::ScmRights(descriptors) => {
                                has_ancillary |= descriptors.count() != 0;
                            }
                            _ => has_ancillary = true,
                        }
                    }
                    Some((received.bytes, received.flags, has_ancillary))
                }
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => None,
                Err(error) => return Err(CompilerExecutionHandoffErrorV1::Io(error.into())),
            }
        };
        let Some((count, flags, has_ancillary)) = received else {
            continue;
        };
        if count == 0 {
            return Err(CompilerExecutionHandoffErrorV1::ControlClosed);
        }
        if count != bytes.len()
            || flags.intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || has_ancillary
        {
            return Err(CompilerExecutionHandoffErrorV1::MalformedReadiness);
        }
        return Ok(bytes);
    }
}

fn require_control_eof(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<(), CompilerExecutionHandoffErrorV1> {
    loop {
        wait_readable(control, deadline)?;
        let mut trailing = [0_u8; 1];
        match recv(control, &mut trailing, RecvFlags::DONTWAIT) {
            Ok((0, _)) => return Ok(()),
            Ok(_) => return Err(CompilerExecutionHandoffErrorV1::TrailingReadiness),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
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

fn wait_readable(
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
            events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
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
        if descriptor.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            return Ok(());
        }
        if descriptor.revents & libc::POLLERR != 0 {
            return Err(CompilerExecutionHandoffErrorV1::ControlClosed);
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
    /// `SO_PEERCRED` did not report a positive protected-supervisor PID.
    InvalidSupervisorPid,
    /// A production handoff attempted to use the rustc client's UID as the service UID.
    ClientAndSupervisorUidMatch,
    /// A connect, handoff, or readiness timeout is zero or exceeds two minutes.
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
    /// Supervisor readiness ended before one exact canonical packet.
    MalformedReadiness,
    /// Supervisor readiness contained a second packet or trailing bytes.
    TrailingReadiness,
    /// The canonical supervisor readiness packet failed strict decoding.
    ReadinessProtocol(CompilerExecutionServiceReadyErrorV1),
    /// Supervisor readiness names another launch manifest or issuer policy.
    ReadinessMismatch,
    /// A bounded descriptor or socket operation failed.
    Io(io::Error),
}

impl fmt::Display for CompilerExecutionHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSupervisorUid => formatter.write_str("invalid protected-supervisor UID"),
            Self::InvalidSupervisorGid => formatter.write_str("invalid protected-supervisor GID"),
            Self::InvalidSupervisorPid => formatter.write_str("invalid protected-supervisor PID"),
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
            Self::MalformedReadiness => {
                formatter.write_str("supervisor readiness packet is malformed")
            }
            Self::TrailingReadiness => {
                formatter.write_str("supervisor readiness contains trailing data")
            }
            Self::ReadinessProtocol(error) => {
                write!(formatter, "supervisor readiness is invalid: {error}")
            }
            Self::ReadinessMismatch => {
                formatter.write_str("supervisor readiness names another launch or policy")
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
            Self::ReadinessProtocol(error) => Some(error),
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
    use std::fs;
    use std::io::IoSliceMut;
    use std::mem::MaybeUninit;
    use std::os::fd::{AsFd, AsRawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SocketFlags,
        accept_with, bind, listen, socketpair,
    };

    use super::*;
    use crate::PendingCompilerExecutionChildChannelV1;

    static LISTENER_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    static RESERVED_CHILD_FD_LOCK: Mutex<()> = Mutex::new(());

    struct NamedListener {
        root: PathBuf,
        path: PathBuf,
        descriptor: OwnedFd,
    }

    impl NamedListener {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "fe2o3-client-supervisor-{name}-{}-{}",
                std::process::id(),
                LISTENER_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let path = root.join("supervisor.sock");
            let descriptor = socket_with(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                None,
            )
            .unwrap();
            bind(&descriptor, &SocketAddrUnix::new(&path).unwrap()).unwrap();
            listen(&descriptor, 8).unwrap();
            Self {
                root,
                path,
                descriptor,
            }
        }
    }

    impl Drop for NamedListener {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn policy() -> CompilerExecutionIssuerPolicyV1 {
        use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;
        CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 1).unwrap(),
            [3; 32],
            ed25519_dalek::SigningKey::from_bytes(&[4; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn external_anchor_service() -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        CompilerExecutionExternalAnchorServiceIdentityV1::new(6_000, 7_000).unwrap()
    }

    fn client_profile() -> CompilerExecutionClientProfileV1 {
        CompilerExecutionClientProfileV1::new(5_000, 5_001, external_anchor_service(), policy())
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

    fn pending_readiness(
        expected_profile: &CompilerExecutionClientProfileV1,
    ) -> (
        PendingCompilerExecutionSupervisorV1,
        OwnedFd,
        CompilerExecutionServiceReadyV1,
    ) {
        let submitter =
            fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1::new(
                100, 1000, 1001,
            )
            .unwrap();
        let client =
            fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1::new(
                101, 1000, 1001,
            )
            .unwrap();
        let manifest = CompilerExecutionServiceLaunchManifestV1::new(
            client,
            expected_profile.external_anchor_service(),
            expected_profile.policy(),
        );
        let handoff = CompilerExecutionSupervisorHandoffV1::new(submitter, manifest).unwrap();
        let readiness = CompilerExecutionServiceReadyV1::new(
            200,
            handoff.launch_manifest(),
            expected_profile.policy(),
        )
        .unwrap();
        let (control, supervisor) = control_pair();
        (
            PendingCompilerExecutionSupervisorV1 { control, handoff },
            supervisor,
            readiness,
        )
    }

    #[test]
    fn exact_supervisor_readiness_is_admitted_once_after_eof() {
        let expected_profile = client_profile();
        let (pending, supervisor, readiness) = pending_readiness(&expected_profile);
        assert_eq!(
            rustix::net::send(
                &supervisor,
                readiness.canonical_bytes(),
                SendFlags::NOSIGNAL,
            )
            .unwrap(),
            readiness.canonical_bytes().len()
        );
        drop(supervisor);
        assert_eq!(
            pending
                .await_readiness(&expected_profile, Duration::from_secs(1))
                .unwrap(),
            readiness
        );
    }

    #[test]
    fn malformed_mismatched_trailing_and_timed_out_readiness_fail_closed() {
        let expected_profile = client_profile();
        let expected_policy = expected_profile.policy().clone();

        let (pending, supervisor, _) = pending_readiness(&expected_profile);
        rustix::net::send(&supervisor, b"short", SendFlags::NOSIGNAL).unwrap();
        drop(supervisor);
        assert!(matches!(
            pending.await_readiness(&expected_profile, Duration::from_secs(1)),
            Err(CompilerExecutionHandoffErrorV1::MalformedReadiness)
        ));

        let (pending, supervisor, readiness) = pending_readiness(&expected_profile);
        let mut corrupted = *readiness.canonical_bytes();
        corrupted[0] ^= 0xff;
        rustix::net::send(&supervisor, &corrupted, SendFlags::NOSIGNAL).unwrap();
        drop(supervisor);
        assert!(matches!(
            pending.await_readiness(&expected_profile, Duration::from_secs(1)),
            Err(CompilerExecutionHandoffErrorV1::ReadinessProtocol(_))
        ));

        let (pending, supervisor, readiness) = pending_readiness(&expected_profile);
        let extra = std::fs::File::open("/dev/null").unwrap();
        let descriptors = [extra.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
        assert_eq!(
            sendmsg(
                &supervisor,
                &[IoSlice::new(readiness.canonical_bytes())],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap(),
            readiness.canonical_bytes().len()
        );
        drop(supervisor);
        assert!(matches!(
            pending.await_readiness(&expected_profile, Duration::from_secs(1)),
            Err(CompilerExecutionHandoffErrorV1::MalformedReadiness)
        ));

        let (pending, supervisor, _readiness) = pending_readiness(&expected_profile);
        let other_client =
            fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1::new(
                102, 1000, 1001,
            )
            .unwrap();
        let other_manifest = CompilerExecutionServiceLaunchManifestV1::new(
            other_client,
            external_anchor_service(),
            &expected_policy,
        );
        let substituted =
            CompilerExecutionServiceReadyV1::new(200, &other_manifest, &expected_policy).unwrap();
        rustix::net::send(
            &supervisor,
            substituted.canonical_bytes(),
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        drop(supervisor);
        assert!(matches!(
            pending.await_readiness(&expected_profile, Duration::from_secs(1)),
            Err(CompilerExecutionHandoffErrorV1::ReadinessMismatch)
        ));

        let (pending, supervisor, readiness) = pending_readiness(&expected_profile);
        rustix::net::send(
            &supervisor,
            readiness.canonical_bytes(),
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        rustix::net::send(&supervisor, b"trailing", SendFlags::NOSIGNAL).unwrap();
        drop(supervisor);
        assert!(matches!(
            pending.await_readiness(&expected_profile, Duration::from_secs(1)),
            Err(CompilerExecutionHandoffErrorV1::TrailingReadiness)
        ));

        let (pending, _supervisor, _) = pending_readiness(&expected_profile);
        assert!(matches!(
            pending.await_readiness(&expected_profile, Duration::from_millis(20)),
            Err(CompilerExecutionHandoffErrorV1::Timeout)
        ));

        let (pending, _supervisor, _) = pending_readiness(&expected_profile);
        let substituted_profile = CompilerExecutionClientProfileV1::new(
            expected_profile.supervisor_uid(),
            expected_profile.supervisor_gid(),
            CompilerExecutionExternalAnchorServiceIdentityV1::new(6_001, 7_001).unwrap(),
            expected_policy,
        )
        .unwrap();
        assert!(matches!(
            pending.await_readiness(&substituted_profile, Duration::from_secs(1)),
            Err(CompilerExecutionHandoffErrorV1::ReadinessMismatch)
        ));
    }

    #[test]
    fn exact_manifest_and_two_ordered_descriptors_transfer_once() {
        let _reserved_child_fd = RESERVED_CHILD_FD_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current_uid = rustix::process::geteuid().as_raw();
        let current_gid = rustix::process::getegid().as_raw();
        if current_uid == 0 || current_gid == 0 {
            return;
        }
        let listener = NamedListener::new("exact-transfer");
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let pending_child = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
        let mut child = command.spawn().unwrap();
        let launch = pending_child
            .finish(child.id(), Duration::from_secs(2))
            .unwrap();
        let client = launch.client();
        let submitter = launch.submitter();
        let credentials =
            CompilerExecutionSupervisorCredentialsV1::new(current_uid, current_gid).unwrap();
        let expected_policy = policy();
        let transferred = transfer_to_supervisor_at_inner::<false>(
            launch,
            &listener.path,
            credentials,
            external_anchor_service(),
            &expected_policy,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            rustix::io::fcntl_getfd(&transferred.control).unwrap(),
            rustix::io::FdFlags::CLOEXEC
        );
        assert!(
            rustix::fs::fcntl_getfl(&transferred.control)
                .unwrap()
                .contains(OFlags::NONBLOCK)
        );
        assert_eq!(
            rustix::net::getsockname(&transferred.control).unwrap(),
            SocketAddrAny::from(SocketAddrUnix::new_unnamed())
        );
        assert_eq!(
            rustix::net::getpeername(&transferred.control).unwrap(),
            Some(SocketAddrAny::from(
                SocketAddrUnix::new(&listener.path).unwrap()
            ))
        );
        let receiver = accept_with(
            &listener.descriptor,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
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
        let _reserved_child_fd = RESERVED_CHILD_FD_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let same_uid_profile = CompilerExecutionClientProfileV1::new(
            current_uid,
            current_gid,
            external_anchor_service(),
            policy(),
        )
        .unwrap();
        assert!(matches!(
            launch.transfer_to_supervisor(&same_uid_profile, Duration::from_secs(2)),
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

    #[test]
    fn fixed_connector_rejects_missing_service_wrong_credentials_and_unbounded_timeouts() {
        let current_uid = rustix::process::geteuid().as_raw();
        let current_gid = rustix::process::getegid().as_raw();
        if current_uid == 0 || current_gid == 0 {
            return;
        }
        let credentials =
            CompilerExecutionSupervisorCredentialsV1::new(current_uid, current_gid).unwrap();
        assert!(matches!(
            validate_boundary_timeout(Duration::ZERO),
            Err(CompilerExecutionHandoffErrorV1::InvalidTimeout)
        ));
        assert!(matches!(
            validate_boundary_timeout(
                MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1 + Duration::from_nanos(1)
            ),
            Err(CompilerExecutionHandoffErrorV1::InvalidTimeout)
        ));
        assert!(matches!(
            validate_boundary_deadline(Instant::now()),
            Err(CompilerExecutionHandoffErrorV1::Timeout)
        ));
        assert!(matches!(
            validate_boundary_deadline(
                Instant::now()
                    + MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1
                    + Duration::from_secs(1)
            ),
            Err(CompilerExecutionHandoffErrorV1::InvalidTimeout)
        ));

        let missing = std::env::temp_dir().join(format!(
            "fe2o3-client-missing-supervisor-{}",
            std::process::id()
        ));
        assert!(matches!(
            connect_to_supervisor(
                &missing,
                credentials,
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionHandoffErrorV1::Io(_))
        ));

        let listener = NamedListener::new("wrong-credentials");
        let wrong_uid = if current_uid == 1 { 2 } else { 1 };
        let wrong = CompilerExecutionSupervisorCredentialsV1::new(wrong_uid, current_gid).unwrap();
        assert!(matches!(
            connect_to_supervisor(
                &listener.path,
                wrong,
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionHandoffErrorV1::SupervisorCredentialsMismatch)
        ));
    }
}
