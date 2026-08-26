use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};

use rustix::net::{AddressFamily, SocketType};

const DIRECTORY_PERMISSIONS: u32 = 0o700;
const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;
const MAX_PIDFD_FDINFO_BYTES: u64 = 4096;
const MAX_PROC_STAT_BYTES: u64 = 4096;
const PIDFD_INFO_PID_V0: u64 = 1 << 0;
const PIDFS_IOCTL_MAGIC: u32 = 0xff;
const PIDFD_GET_INFO_NUMBER: u32 = 11;

// Linux UAPI pidfd_info version 0 is exactly 64 bytes. Keeping a local layout also keeps the
// ioctl opcode at the v0 size if a later libc exposes a larger structure version.
#[repr(C)]
struct PidfdInfoV0 {
    mask: u64,
    cgroupid: u64,
    pid: u32,
    tgid: u32,
    ppid: u32,
    ruid: u32,
    rgid: u32,
    euid: u32,
    egid: u32,
    suid: u32,
    sgid: u32,
    fsuid: u32,
    fsgid: u32,
    exit_code: i32,
}

const _: () = assert!(std::mem::size_of::<PidfdInfoV0>() == 64);
const PIDFD_GET_INFO_V0: libc::Ioctl =
    libc::_IOWR::<PidfdInfoV0>(PIDFS_IOCTL_MAGIC, PIDFD_GET_INFO_NUMBER);

/// Stable classification for a failed protected-service admission or revalidation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AdmissionErrorKindV1 {
    ExpectedClientPid,
    InspectRoot,
    RootNotDirectory,
    RootOwner,
    RootMode,
    RootUnlinked,
    RootCloseOnExec,
    InspectPeer,
    PeerDomain,
    PeerSocketType,
    PeerNotConnected,
    PeerLocalAddress,
    PeerRemoteAddress,
    PeerCloseOnExec,
    InspectClientPidfd,
    InspectClientStartTime,
    ClientPidfdCloseOnExec,
    ClientPidfdThread,
    ClientPidfdTargetMismatch,
    ClientPidfdIdentityChanged,
    ClientStartTimeChanged,
    ClientAlreadyDead,
    SameUidClient,
    PeerCredentialsMismatch,
    DuplicateDescriptors,
    ServiceIdentityChanged,
    RootIdentityChanged,
    PeerIdentityChanged,
    PeerCredentialsChanged,
}

/// Failure from the inert protected-service admission boundary.
#[derive(Debug)]
pub struct BrokerAuthorityServiceAdmissionErrorV1 {
    kind: AdmissionErrorKindV1,
    message: String,
    source: Option<io::Error>,
}

impl BrokerAuthorityServiceAdmissionErrorV1 {
    fn new(kind: AdmissionErrorKindV1, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    fn io(kind: AdmissionErrorKindV1, message: impl Into<String>, source: io::Error) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source),
        }
    }

    pub const fn kind(&self) -> AdmissionErrorKindV1 {
        self.kind
    }
}

impl fmt::Display for BrokerAuthorityServiceAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for BrokerAuthorityServiceAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectIdentityV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}

impl ObjectIdentityV1 {
    fn inspect(
        descriptor: &OwnedFd,
        kind: AdmissionErrorKindV1,
        label: &'static str,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        let stat = rustix::fs::fstat(descriptor).map_err(|error| {
            BrokerAuthorityServiceAdmissionErrorV1::io(
                kind,
                format!("cannot inspect retained {label} descriptor"),
                io::Error::from(error),
            )
        })?;
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
        })
    }

    const fn object(self) -> (u64, u64) {
        (self.device, self.inode)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PeerCredentialsV1 {
    pid: u32,
    uid: u32,
    gid: u32,
}

impl PeerCredentialsV1 {
    fn inspect(peer: &OwnedFd) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        let credentials = rustix::net::sockopt::socket_peercred(peer).map_err(|error| {
            BrokerAuthorityServiceAdmissionErrorV1::io(
                AdmissionErrorKindV1::InspectPeer,
                "cannot inspect retained peer SO_PEERCRED",
                io::Error::from(error),
            )
        })?;
        let raw_pid = credentials.pid.as_raw_nonzero().get();
        let pid = u32::try_from(raw_pid).map_err(|_| {
            BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ExpectedClientPid,
                "broker peer SO_PEERCRED PID is not a positive u32",
            )
        })?;
        if pid == 0 {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ExpectedClientPid,
                "broker peer SO_PEERCRED PID is zero",
            ));
        }
        Ok(Self {
            pid,
            uid: credentials.uid.as_raw(),
            gid: credentials.gid.as_raw(),
        })
    }
}

/// Exact connection-time client credentials asserted by the protected supervisor.
///
/// Matching this value against `SO_PEERCRED` does not prove process liveness, prevent PID reuse,
/// or prove exclusive ownership of the connected endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedClientProcessIdentityV1 {
    pid: u32,
    uid: u32,
    gid: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PidfdIdentitySourceV1 {
    KernelIoctl,
    ProcfsFdinfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PidfdTargetObservationV1 {
    pid: u32,
    source: PidfdIdentitySourceV1,
}

/// Move-only, opaque evidence retaining one supervisor-supplied client pidfd.
///
/// Admission binds the pidfd's kernel-reported target to the exact expected client PID and
/// rejects an already exited target. Holding this value prevents that process identity from being
/// confused with a later reuse of its numeric PID. The target can still exit immediately after a
/// successful check, so callers must invoke [`Self::validate_liveness`] at each use boundary.
///
/// This token has no raw-descriptor, path, storage, serialization, signal, wait, or reap API. It
/// grants no authority and is not by itself bound to a broker peer; that binding occurs only when
/// it is consumed by [`ProtectedBrokerServiceAdmissionV1::admit`].
pub struct LiveClientPidfdIdentityV1 {
    pidfd: OwnedFd,
    expected_client: ExpectedClientProcessIdentityV1,
    descriptor_identity: ObjectIdentityV1,
    identity_source: PidfdIdentitySourceV1,
    start_time_ticks: u64,
}

impl fmt::Debug for LiveClientPidfdIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveClientPidfdIdentityV1")
            .field("authority", &"none")
            .field("expected_client", &self.expected_client)
            .field("identity_source", &self.identity_source)
            .finish_non_exhaustive()
    }
}

impl LiveClientPidfdIdentityV1 {
    /// Admits a supervisor-supplied pidfd for one exact, currently live expected PID.
    ///
    /// The exact `PIDFD_GET_INFO` v0 request is preferred. `ENOTTY`, or the legacy `EINVAL` caused
    /// by Linux 6.12 checking a nonzero ioctl argument before unknown commands, only dispatches to
    /// a bounded `/proc/self/fdinfo/<fd>` proof. The errno itself is never accepted. That kernel
    /// procfs record must contain exactly one `Pid:` and one octal `flags:` field and must not carry
    /// `PIDFD_THREAD`. The selected procfs mount must map `/proc/self` and `/proc/<getpid>` to the
    /// same entry; this is numeric-self consistency, not proof of the active PID namespace.
    pub fn admit(
        supervisor_pidfd: OwnedFd,
        expected_client: ExpectedClientProcessIdentityV1,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        require_close_on_exec(
            &supervisor_pidfd,
            AdmissionErrorKindV1::ClientPidfdCloseOnExec,
            "client pidfd",
        )?;
        let descriptor_identity = ObjectIdentityV1::inspect(
            &supervisor_pidfd,
            AdmissionErrorKindV1::InspectClientPidfd,
            "client pidfd",
        )?;
        let observation = inspect_pidfd_target(&supervisor_pidfd)?;
        require_process_pidfd_mode(&supervisor_pidfd)?;
        require_pidfd_target(observation.pid, expected_client.pid)?;
        let start_time_ticks = inspect_process_start_time_ticks(expected_client.pid)?;
        let identity = Self {
            pidfd: supervisor_pidfd,
            expected_client,
            descriptor_identity,
            identity_source: observation.source,
            start_time_ticks,
        };
        identity.validate_liveness()?;
        Ok(identity)
    }

    /// Revalidates the retained descriptor, exact target PID, and point-in-time liveness.
    ///
    /// The check polls the pidfd for terminal readiness and supplements that with
    /// `waitid(P_PIDFD, WEXITED | WNOHANG | WNOWAIT)` when the target is a waitable child. It never
    /// reaps. `ECHILD` is expected for a valid non-child pidfd and leaves poll as the liveness
    /// authority. A successful return cannot prevent a later process exit.
    pub fn validate_liveness(&self) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
        require_close_on_exec(
            &self.pidfd,
            AdmissionErrorKindV1::ClientPidfdCloseOnExec,
            "client pidfd",
        )?;
        let descriptor_identity = ObjectIdentityV1::inspect(
            &self.pidfd,
            AdmissionErrorKindV1::InspectClientPidfd,
            "client pidfd",
        )?;
        if descriptor_identity != self.descriptor_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd descriptor identity changed",
            ));
        }
        let observation = inspect_pidfd_target(&self.pidfd)?;
        require_process_pidfd_mode(&self.pidfd)?;
        if observation.source != self.identity_source {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd identity probe changed",
            ));
        }
        require_pidfd_target(observation.pid, self.expected_client.pid)?;
        require_client_start_time(
            inspect_process_start_time_ticks(self.expected_client.pid)?,
            self.start_time_ticks,
        )?;
        require_pidfd_live(&self.pidfd)?;
        let final_observation = inspect_pidfd_target(&self.pidfd)?;
        if final_observation != observation {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd target changed while checking liveness",
            ));
        }
        let final_identity = ObjectIdentityV1::inspect(
            &self.pidfd,
            AdmissionErrorKindV1::InspectClientPidfd,
            "client pidfd",
        )?;
        if final_identity != self.descriptor_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd descriptor changed while checking liveness",
            ));
        }
        require_client_start_time(
            inspect_process_start_time_ticks(self.expected_client.pid)?,
            self.start_time_ticks,
        )?;
        Ok(())
    }
}

impl ExpectedClientProcessIdentityV1 {
    pub fn new(
        pid: u32,
        uid: u32,
        gid: u32,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        if pid == 0 {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ExpectedClientPid,
                "expected broker client PID must be nonzero",
            ));
        }
        Ok(Self { pid, uid, gid })
    }

    pub const fn pid(self) -> u32 {
        self.pid
    }

    pub const fn uid(self) -> u32 {
        self.uid
    }

    pub const fn gid(self) -> u32 {
        self.gid
    }

    const fn credentials(self) -> PeerCredentialsV1 {
        PeerCredentialsV1 {
            pid: self.pid,
            uid: self.uid,
            gid: self.gid,
        }
    }
}

/// Retained, move-only evidence that the protected service boundary was shaped correctly.
///
/// This value is not an execution capability. Its only operation revalidates the retained Linux
/// file descriptions, pidfd target and point-in-time liveness, and peer credentials against their
/// admission snapshots.
pub struct ProtectedBrokerServiceAdmissionV1 {
    root: OwnedFd,
    peer: OwnedFd,
    live_client: LiveClientPidfdIdentityV1,
    service_uid: u32,
    root_identity: ObjectIdentityV1,
    peer_identity: ObjectIdentityV1,
    #[cfg(test)]
    non_authoritative_same_uid_session_test: bool,
}

impl fmt::Debug for ProtectedBrokerServiceAdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedBrokerServiceAdmissionV1")
            .field("authority", &"none")
            .field("service_uid", &self.service_uid)
            .field("root_identity", &self.root_identity)
            .field("peer_identity", &self.peer_identity)
            .field("expected_client", &self.live_client.expected_client)
            .finish_non_exhaustive()
    }
}

impl ProtectedBrokerServiceAdmissionV1 {
    pub(crate) const fn matches_client_process(&self, pid: u32, start_time_ticks: u64) -> bool {
        self.live_client.expected_client.pid == pid
            && self.live_client.start_time_ticks == start_time_ticks
    }

    pub(crate) fn try_clone_service_root(
        &self,
    ) -> Result<OwnedFd, BrokerAuthorityServiceAdmissionErrorV1> {
        self.validate_session_continuity()?;
        rustix::io::fcntl_dupfd_cloexec(&self.root, 0).map_err(|error| {
            BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::InspectRoot,
                format!("cannot duplicate retained service root: {error}"),
            )
        })
    }

    pub(crate) fn into_service_root(self) -> OwnedFd {
        self.root
    }

    pub(crate) fn validate_session_continuity(
        &self,
    ) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
        #[cfg(test)]
        if self.non_authoritative_same_uid_session_test {
            return self.validate_continuity_inner::<false>();
        }
        self.validate_continuity()
    }

    #[cfg(test)]
    pub(crate) const fn non_authoritative_test_process_identity(&self) -> (u32, u64) {
        (
            self.live_client.expected_client.pid,
            self.live_client.start_time_ticks,
        )
    }

    /// Admits only supervisor-owned descriptors and an exact expected connection-time identity.
    ///
    /// The caller must run as the protected service UID. The retained directory must be owned by
    /// that UID with exact mode `0700`. The retained peer must be a connected Unix
    /// `SOCK_SEQPACKET` endpoint with unnamed local and peer addresses. Its kernel-reported PID,
    /// UID, and GID must exactly match the retained pidfd identity, whose UID must differ from the
    /// service. The pidfd is checked before and after the peer credential comparison.
    pub fn admit(
        supervisor_root: OwnedFd,
        retained_peer: OwnedFd,
        live_client: LiveClientPidfdIdentityV1,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        let service_uid = rustix::process::geteuid().as_raw();
        require_close_on_exec(
            &supervisor_root,
            AdmissionErrorKindV1::RootCloseOnExec,
            "supervisor root",
        )?;
        require_close_on_exec(
            &retained_peer,
            AdmissionErrorKindV1::PeerCloseOnExec,
            "broker peer",
        )?;
        let root_identity = validate_root(&supervisor_root, service_uid)?;
        let retained_peer_identity =
            ObjectIdentityV1::inspect(&retained_peer, AdmissionErrorKindV1::InspectPeer, "peer")?;
        require_distinct_descriptors(root_identity, retained_peer_identity)?;
        require_distinct_pidfd_descriptor(
            root_identity,
            retained_peer_identity,
            live_client.descriptor_identity,
        )?;
        let peer_identity = validate_peer_shape(&retained_peer)?;
        if peer_identity != retained_peer_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerIdentityChanged,
                "retained broker peer identity changed during admission",
            ));
        }
        if live_client.expected_client.uid == service_uid {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::SameUidClient,
                "expected broker client UID equals protected service effective UID",
            ));
        }
        let peer_credentials = PeerCredentialsV1::inspect(&retained_peer)?;
        if peer_credentials != live_client.expected_client.credentials() {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerCredentialsMismatch,
                "broker peer SO_PEERCRED does not match the retained pidfd client identity",
            ));
        }
        live_client.validate_liveness()?;
        Self::finish_admission(
            supervisor_root,
            retained_peer,
            live_client,
            service_uid,
            root_identity,
            peer_identity,
        )
    }

    #[cfg(test)]
    pub(crate) fn admit_non_authoritative_same_uid_session_test(
        root: OwnedFd,
        peer: OwnedFd,
        live_client: LiveClientPidfdIdentityV1,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        let service_uid = rustix::process::geteuid().as_raw();
        require_close_on_exec(
            &root,
            AdmissionErrorKindV1::RootCloseOnExec,
            "supervisor root",
        )?;
        require_close_on_exec(&peer, AdmissionErrorKindV1::PeerCloseOnExec, "broker peer")?;
        let root_identity = validate_root(&root, service_uid)?;
        let peer_identity = validate_peer_shape(&peer)?;
        require_distinct_descriptors(root_identity, peer_identity)?;
        require_distinct_pidfd_descriptor(
            root_identity,
            peer_identity,
            live_client.descriptor_identity,
        )?;
        let credentials = PeerCredentialsV1::inspect(&peer)?;
        if credentials != live_client.expected_client.credentials() {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerCredentialsMismatch,
                "test peer SO_PEERCRED does not match retained pidfd identity",
            ));
        }
        live_client.validate_liveness()?;
        let admission = Self {
            root,
            peer,
            live_client,
            service_uid,
            root_identity,
            peer_identity,
            non_authoritative_same_uid_session_test: true,
        };
        admission.validate_session_continuity()?;
        Ok(admission)
    }

    fn finish_admission(
        root: OwnedFd,
        peer: OwnedFd,
        live_client: LiveClientPidfdIdentityV1,
        service_uid: u32,
        root_identity: ObjectIdentityV1,
        peer_identity: ObjectIdentityV1,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        let admission = Self {
            root,
            peer,
            live_client,
            service_uid,
            root_identity,
            peer_identity,
            #[cfg(test)]
            non_authoritative_same_uid_session_test: false,
        };
        admission.validate_continuity()?;
        Ok(admission)
    }

    /// Revalidates service UID, root security metadata, socket shape, observed object identities,
    /// pidfd target and liveness, and `SO_PEERCRED`; any discrepancy fails closed.
    pub fn validate_continuity(&self) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
        self.validate_continuity_inner::<true>()
    }

    fn validate_continuity_inner<const REQUIRE_DISTINCT_UID: bool>(
        &self,
    ) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
        if rustix::process::geteuid().as_raw() != self.service_uid {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ServiceIdentityChanged,
                "protected service effective UID changed after admission",
            ));
        }

        require_close_on_exec(
            &self.root,
            AdmissionErrorKindV1::RootCloseOnExec,
            "supervisor root",
        )?;
        require_close_on_exec(
            &self.peer,
            AdmissionErrorKindV1::PeerCloseOnExec,
            "broker peer",
        )?;
        self.live_client.validate_liveness()?;

        let root_identity = validate_root(&self.root, self.service_uid)?;
        if root_identity != self.root_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::RootIdentityChanged,
                "retained supervisor directory identity or security metadata changed",
            ));
        }

        let peer_identity = validate_peer_shape(&self.peer)?;
        if peer_identity != self.peer_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerIdentityChanged,
                "retained broker peer descriptor identity changed",
            ));
        }
        require_distinct_descriptors(root_identity, peer_identity)?;
        require_distinct_pidfd_descriptor(
            root_identity,
            peer_identity,
            self.live_client.descriptor_identity,
        )?;

        if REQUIRE_DISTINCT_UID && self.live_client.expected_client.uid == self.service_uid {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::SameUidClient,
                "expected broker client UID equals protected service effective UID",
            ));
        }
        let credentials = PeerCredentialsV1::inspect(&self.peer)?;
        if credentials != self.live_client.expected_client.credentials() {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerCredentialsChanged,
                "retained broker peer SO_PEERCRED no longer matches expected client identity",
            ));
        }
        let final_peer_identity =
            ObjectIdentityV1::inspect(&self.peer, AdmissionErrorKindV1::InspectPeer, "peer")?;
        if final_peer_identity != self.peer_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerIdentityChanged,
                "retained broker peer identity changed while checking credentials",
            ));
        }
        self.live_client.validate_liveness()?;
        Ok(())
    }

    #[cfg(test)]
    fn validate_non_authoritative_test_continuity(
        &self,
    ) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
        self.validate_continuity_inner::<false>()
    }
}

fn require_close_on_exec(
    descriptor: &OwnedFd,
    kind: AdmissionErrorKindV1,
    label: &'static str,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    let flags = rustix::io::fcntl_getfd(descriptor).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            kind,
            format!("cannot inspect {label} descriptor flags"),
            io::Error::from(error),
        )
    })?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            kind,
            format!("retained {label} descriptor does not have FD_CLOEXEC"),
        ));
    }
    Ok(())
}

fn require_process_pidfd_mode(
    pidfd: &OwnedFd,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    let flags = rustix::fs::fcntl_getfl(pidfd).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot inspect client pidfd file status flags",
            io::Error::from(error),
        )
    })?;
    // Linux v6.12 UAPI defines PIDFD_THREAD as exactly O_EXCL. This contract rejects only that
    // identified process-vs-thread selector and does not require unrelated flag bits to be zero.
    if flags.contains(rustix::fs::OFlags::EXCL) {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientPidfdThread,
            "client pidfd has Linux PIDFD_THREAD (O_EXCL) semantics",
        ));
    }
    Ok(())
}

fn inspect_pidfd_target(
    pidfd: &OwnedFd,
) -> Result<PidfdTargetObservationV1, BrokerAuthorityServiceAdmissionErrorV1> {
    // SAFETY: PidfdInfoV0 contains only integer fields, so all-zero is a valid request. The ioctl
    // reads the request mask and initializes fields indicated by the returned mask.
    let mut info = unsafe { MaybeUninit::<PidfdInfoV0>::zeroed().assume_init() };
    info.mask = PIDFD_INFO_PID_V0;
    // SAFETY: `info` is writable for the exact 64-byte v0 type encoded by PIDFD_GET_INFO_V0 and the
    // descriptor remains borrowed for the call.
    let result = unsafe { libc::ioctl(pidfd.as_raw_fd(), PIDFD_GET_INFO_V0, &mut info) };
    if result == 0 {
        if info.mask & PIDFD_INFO_PID_V0 == 0 || info.pid == 0 || info.tgid != info.pid {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::InspectClientPidfd,
                "PIDFD_GET_INFO omitted a usable process-leader target PID",
            ));
        }
        return Ok(PidfdTargetObservationV1 {
            pid: info.pid,
            source: PidfdIdentitySourceV1::KernelIoctl,
        });
    }

    dispatch_pidfd_get_info_error(pidfd, io::Error::last_os_error())
}

fn dispatch_pidfd_get_info_error(
    pidfd: &OwnedFd,
    error: io::Error,
) -> Result<PidfdTargetObservationV1, BrokerAuthorityServiceAdmissionErrorV1> {
    match error.raw_os_error() {
        // Linux v6.12 checks for a nonzero pidfd ioctl argument before its command switch, so the
        // pointer-bearing v0 info request returns EINVAL. Linux v6.13 dispatches PIDFD_GET_INFO
        // before that check. Neither errno proves descriptor type: only strict kernel procfs
        // inspection below can make this fallback succeed.
        Some(libc::ENOTTY) | Some(libc::EINVAL) => inspect_pidfd_target_from_procfs(pidfd),
        Some(libc::ESRCH) => Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientAlreadyDead,
            "client pidfd target exited before identity inspection",
        )),
        _ => Err(BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot inspect client pidfd with PIDFD_GET_INFO",
            error,
        )),
    }
}

fn inspect_pidfd_target_from_procfs(
    pidfd: &OwnedFd,
) -> Result<PidfdTargetObservationV1, BrokerAuthorityServiceAdmissionErrorV1> {
    let self_entry = open_validated_procfs_self()?;
    let directory_flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC;
    let fdinfo: File = rustix::fs::openat(
        &self_entry,
        "fdinfo",
        directory_flags,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot open the retained procfs fdinfo directory",
            io::Error::from(error),
        )
    })?;
    require_procfs(&fdinfo, "retained /proc/self/fdinfo directory")?;
    let record_flags =
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC;
    let mut record: File = rustix::fs::openat(
        &fdinfo,
        pidfd.as_raw_fd().to_string(),
        record_flags,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot open the bounded procfs client pidfd identity record",
            io::Error::from(error),
        )
    })?;
    require_procfs(&record, "client pidfd identity record")?;

    let mut contents = String::new();
    record
        .by_ref()
        .take(MAX_PIDFD_FDINFO_BYTES + 1)
        .read_to_string(&mut contents)
        .map_err(|error| {
            BrokerAuthorityServiceAdmissionErrorV1::io(
                AdmissionErrorKindV1::InspectClientPidfd,
                "cannot read the bounded procfs client pidfd identity record",
                error,
            )
        })?;
    if contents.len() as u64 > MAX_PIDFD_FDINFO_BYTES {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "procfs client pidfd identity record exceeds 4096 bytes",
        ));
    }

    let pid = parse_pidfd_fdinfo(&contents)?;
    Ok(PidfdTargetObservationV1 {
        pid,
        source: PidfdIdentitySourceV1::ProcfsFdinfo,
    })
}

fn parse_pidfd_fdinfo(contents: &str) -> Result<u32, BrokerAuthorityServiceAdmissionErrorV1> {
    let mut pid_value = None;
    let mut flags_value = None;
    for line in contents.lines() {
        if let Some(field) = line.strip_prefix("Pid:") {
            let value = field.strip_prefix('\t').ok_or_else(|| {
                BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed Pid field",
                )
            })?;
            if pid_value.is_some() {
                return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has duplicate Pid fields",
                ));
            }
            let canonical_positive_or_zero = !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value.len() == 1 || !value.starts_with('0'));
            if value != "-1" && !canonical_positive_or_zero {
                return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has a non-canonical decimal Pid field",
                ));
            }
            pid_value = Some(value.parse::<i64>().map_err(|_| {
                BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed Pid field",
                )
            })?);
        }
        if let Some(field) = line.strip_prefix("flags:") {
            let value = field.strip_prefix('\t').ok_or_else(|| {
                BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed flags field",
                )
            })?;
            if flags_value.is_some() {
                return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has duplicate flags fields",
                ));
            }
            if value.len() < 2
                || !value.starts_with('0')
                || !value.bytes().all(|byte| matches!(byte, b'0'..=b'7'))
            {
                return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed octal flags field",
                ));
            }
            flags_value = Some(u32::from_str_radix(value, 8).map_err(|_| {
                BrokerAuthorityServiceAdmissionErrorV1::new(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "procfs client pidfd identity record has an out-of-range octal flags field",
                )
            })?);
        }
    }
    let pid_value = pid_value.ok_or_else(|| {
        BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "descriptor is not a pidfd with a procfs Pid identity field",
        )
    })?;
    let flags_value = flags_value.ok_or_else(|| {
        BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "descriptor has no exact procfs octal flags identity field",
        )
    })?;
    // Linux v6.12 fs/proc/fd.c emits file->f_flags in octal, and pidfd.h defines PIDFD_THREAD as
    // O_EXCL. Reject only that exact bit so unrelated current or future flags remain admissible.
    if flags_value & libc::PIDFD_THREAD != 0 {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientPidfdThread,
            "procfs client pidfd flags contain Linux PIDFD_THREAD (O_EXCL)",
        ));
    }
    if pid_value == -1 {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientAlreadyDead,
            "client pidfd target was already reaped",
        ));
    }
    let pid = u32::try_from(pid_value).map_err(|_| {
        BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "procfs client pidfd identity is not positive in the selected procfs namespace view",
        )
    })?;
    if pid == 0 {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "procfs client pidfd identity is not positive in the selected procfs namespace view",
        ));
    }
    Ok(pid)
}

fn open_validated_procfs_self() -> Result<File, BrokerAuthorityServiceAdmissionErrorV1> {
    let self_entry = File::open("/proc/self").map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot open /proc/self for pidfd fallback validation",
            error,
        )
    })?;
    let numeric_entry = File::open(format!("/proc/{}", std::process::id())).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "selected procfs mount has no numeric entry for the service getpid value",
            error,
        )
    })?;
    require_procfs(&self_entry, "/proc/self")?;
    require_procfs(&numeric_entry, "numeric /proc self entry")?;
    let self_stat = rustix::fs::fstat(&self_entry).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot inspect /proc/self",
            io::Error::from(error),
        )
    })?;
    let numeric_stat = rustix::fs::fstat(&numeric_entry).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot inspect numeric /proc self entry",
            io::Error::from(error),
        )
    })?;
    if (self_stat.st_dev, self_stat.st_ino) != (numeric_stat.st_dev, numeric_stat.st_ino) {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "/proc/self and /proc/<getpid> do not name the same process in the selected procfs mount",
        ));
    }
    Ok(self_entry)
}

fn require_procfs(
    file: &File,
    label: &'static str,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    let filesystem = rustix::fs::fstatfs(file).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientPidfd,
            format!("cannot inspect filesystem type for {label}"),
            io::Error::from(error),
        )
    })?;
    if filesystem.f_type != rustix::fs::PROC_SUPER_MAGIC {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            format!("{label} is not backed by procfs"),
        ));
    }
    Ok(())
}

fn inspect_process_start_time_ticks(
    pid: u32,
) -> Result<u64, BrokerAuthorityServiceAdmissionErrorV1> {
    // Validate that the selected procfs mount maps the service's numeric getpid consistently
    // before trusting a numeric client entry. This remains a trusted compatible-procfs
    // precondition; the check does not prove mount-namespace provenance.
    let _validated_self = open_validated_procfs_self()?;
    let mut record = File::open(format!("/proc/{pid}/stat")).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::InspectClientStartTime,
            "cannot open bounded client procfs stat identity",
            error,
        )
    })?;
    require_procfs(&record, "client process stat identity")?;
    let mut contents = Vec::new();
    record
        .by_ref()
        .take(MAX_PROC_STAT_BYTES + 1)
        .read_to_end(&mut contents)
        .map_err(|error| {
            BrokerAuthorityServiceAdmissionErrorV1::io(
                AdmissionErrorKindV1::InspectClientStartTime,
                "cannot read bounded client procfs stat identity",
                error,
            )
        })?;
    if contents.is_empty() || contents.len() as u64 > MAX_PROC_STAT_BYTES {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientStartTime,
            "client procfs stat identity is empty or exceeds 4096 bytes",
        ));
    }
    parse_process_start_time_ticks(&contents, pid)
}

fn parse_process_start_time_ticks(
    contents: &[u8],
    expected_pid: u32,
) -> Result<u64, BrokerAuthorityServiceAdmissionErrorV1> {
    let close = contents
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| {
            BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::InspectClientStartTime,
                "client procfs stat identity has no command terminator",
            )
        })?;
    let first_space = contents
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or_else(|| {
            BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::InspectClientStartTime,
                "client procfs stat identity has no PID terminator",
            )
        })?;
    if contents.get(first_space + 1) != Some(&b'(') || close <= first_space + 1 {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientStartTime,
            "client procfs stat identity has a malformed command field",
        ));
    }
    let pid_bytes = &contents[..first_space];
    if pid_bytes.is_empty()
        || (pid_bytes.len() > 1 && pid_bytes.starts_with(b"0"))
        || !pid_bytes.iter().all(u8::is_ascii_digit)
    {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientStartTime,
            "client procfs stat identity has a noncanonical PID",
        ));
    }
    let recorded_pid = std::str::from_utf8(pid_bytes)
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    if recorded_pid != Some(expected_pid) {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientStartTime,
            "client procfs stat PID does not match the retained pidfd target",
        ));
    }
    let mut fields = contents
        .get(close + 1..)
        .ok_or_else(|| {
            BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::InspectClientStartTime,
                "client procfs stat identity ended at its command field",
            )
        })?
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty());
    let start_time = fields.nth(19).ok_or_else(|| {
        BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientStartTime,
            "client procfs stat identity has no start-time field",
        )
    })?;
    if start_time.is_empty()
        || (start_time.len() > 1 && start_time.starts_with(b"0"))
        || !start_time.iter().all(u8::is_ascii_digit)
    {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::InspectClientStartTime,
            "client procfs stat identity has a noncanonical start time",
        ));
    }
    let start_time = std::str::from_utf8(start_time)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::InspectClientStartTime,
                "client procfs stat identity has an invalid start time",
            )
        })?;
    Ok(start_time)
}

fn require_client_start_time(
    actual: u64,
    expected: u64,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    if actual == expected {
        Ok(())
    } else {
        Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientStartTimeChanged,
            "retained client process start time changed",
        ))
    }
}

fn require_pidfd_target(
    actual_pid: u32,
    expected_pid: u32,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    if actual_pid != expected_pid {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientPidfdTargetMismatch,
            format!("client pidfd targets PID {actual_pid}, expected exact PID {expected_pid}"),
        ));
    }
    Ok(())
}

fn require_pidfd_live(pidfd: &OwnedFd) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    require_pidfd_not_pollable(pidfd)?;
    let options = rustix::process::WaitIdOptions::EXITED
        | rustix::process::WaitIdOptions::NOHANG
        | rustix::process::WaitIdOptions::NOWAIT;
    match rustix::process::waitid(rustix::process::WaitId::PidFd(pidfd.as_fd()), options) {
        Ok(Some(_)) => {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::ClientAlreadyDead,
                "client pidfd identifies an exited waitable child",
            ));
        }
        Ok(None) | Err(rustix::io::Errno::CHILD) => {}
        Err(error) => {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::io(
                AdmissionErrorKindV1::InspectClientPidfd,
                "cannot perform non-reaping waitid liveness probe on client pidfd",
                io::Error::from(error),
            ));
        }
    }
    require_pidfd_not_pollable(pidfd)
}

fn require_pidfd_not_pollable(
    pidfd: &OwnedFd,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    let mut poll_descriptor = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = loop {
        // SAFETY: `poll_descriptor` is writable for one declared pollfd and timeout zero cannot
        // block. The retained descriptor remains borrowed for the call.
        let result = unsafe { libc::poll(&mut poll_descriptor, 1, 0) };
        if result >= 0 {
            break result;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::io(
                AdmissionErrorKindV1::InspectClientPidfd,
                "cannot poll client pidfd for liveness",
                error,
            ));
        }
    };
    if ready == 0 {
        return Ok(());
    }
    if poll_descriptor.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::ClientAlreadyDead,
            "client pidfd reports process exit",
        ));
    }
    Err(BrokerAuthorityServiceAdmissionErrorV1::new(
        AdmissionErrorKindV1::InspectClientPidfd,
        format!(
            "client pidfd returned unexpected poll events 0x{:x}",
            poll_descriptor.revents
        ),
    ))
}

fn validate_root(
    root: &OwnedFd,
    service_uid: u32,
) -> Result<ObjectIdentityV1, BrokerAuthorityServiceAdmissionErrorV1> {
    let identity = ObjectIdentityV1::inspect(root, AdmissionErrorKindV1::InspectRoot, "root")?;
    if !rustix::fs::FileType::from_raw_mode(identity.mode).is_dir() {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::RootNotDirectory,
            "supervisor-supplied root descriptor is not a directory",
        ));
    }
    if identity.uid != service_uid {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::RootOwner,
            format!(
                "supervisor directory owner UID {} differs from service UID {service_uid}",
                identity.uid
            ),
        ));
    }
    if identity.mode & PERMISSION_AND_SPECIAL_BITS != DIRECTORY_PERMISSIONS {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::RootMode,
            format!(
                "supervisor directory mode is {:04o}, expected exactly 0700",
                identity.mode & PERMISSION_AND_SPECIAL_BITS
            ),
        ));
    }
    if identity.links == 0 {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::RootUnlinked,
            "supervisor directory has st_nlink zero and is no longer linked",
        ));
    }
    Ok(identity)
}

fn validate_peer_shape(
    peer: &OwnedFd,
) -> Result<ObjectIdentityV1, BrokerAuthorityServiceAdmissionErrorV1> {
    let identity = ObjectIdentityV1::inspect(peer, AdmissionErrorKindV1::InspectPeer, "peer")?;
    let domain = rustix::net::sockopt::socket_domain(peer).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::PeerDomain,
            "retained broker peer is not a socket with an inspectable domain",
            io::Error::from(error),
        )
    })?;
    if domain != AddressFamily::UNIX {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::PeerDomain,
            "retained broker peer is not an AF_UNIX socket",
        ));
    }

    let socket_type = rustix::net::sockopt::socket_type(peer).map_err(|error| {
        BrokerAuthorityServiceAdmissionErrorV1::io(
            AdmissionErrorKindV1::PeerSocketType,
            "cannot inspect retained broker peer socket type",
            io::Error::from(error),
        )
    })?;
    if socket_type != SocketType::SEQPACKET {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::PeerSocketType,
            "retained broker peer is not SOCK_SEQPACKET",
        ));
    }

    require_unnamed_unix_address(
        peer,
        UnixAddressSideV1::Remote,
        AdmissionErrorKindV1::PeerRemoteAddress,
        "remote",
    )?;
    require_unnamed_unix_address(
        peer,
        UnixAddressSideV1::Local,
        AdmissionErrorKindV1::PeerLocalAddress,
        "local",
    )?;
    let final_identity =
        ObjectIdentityV1::inspect(peer, AdmissionErrorKindV1::InspectPeer, "peer")?;
    if final_identity != identity {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::PeerIdentityChanged,
            "retained broker peer identity changed while checking socket shape",
        ));
    }
    Ok(final_identity)
}

#[derive(Clone, Copy)]
enum UnixAddressSideV1 {
    Local,
    Remote,
}

fn require_unnamed_unix_address(
    peer: &OwnedFd,
    side: UnixAddressSideV1,
    kind: AdmissionErrorKindV1,
    label: &'static str,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    let mut address = MaybeUninit::<libc::sockaddr_un>::zeroed();
    let mut length = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_un>())
        .expect("sockaddr_un length fits socklen_t");
    // SAFETY: the address buffer is writable for its declared size, `length` is initialized to
    // that size, and the retained descriptor remains borrowed for the syscall.
    let result = unsafe {
        match side {
            UnixAddressSideV1::Local => libc::getsockname(
                peer.as_raw_fd(),
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
            UnixAddressSideV1::Remote => libc::getpeername(
                peer.as_raw_fd(),
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
        }
    };
    if result != 0 {
        let error_kind = match side {
            UnixAddressSideV1::Local => kind,
            UnixAddressSideV1::Remote => AdmissionErrorKindV1::PeerNotConnected,
        };
        return Err(BrokerAuthorityServiceAdmissionErrorV1::io(
            error_kind,
            format!("cannot inspect retained broker peer {label} address"),
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: a successful getsockname/getpeername initialized at least the family field, and the
    // buffer began fully zeroed for any bytes the kernel did not write.
    let address = unsafe { address.assume_init() };
    if i32::from(address.sun_family) != libc::AF_UNIX {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            kind,
            format!("retained broker peer {label} address is not AF_UNIX"),
        ));
    }
    let unnamed_length = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    if usize::try_from(length).ok() != Some(unnamed_length) {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            kind,
            format!("retained broker peer {label} address is named"),
        ));
    }
    Ok(())
}

fn require_distinct_descriptors(
    root: ObjectIdentityV1,
    peer: ObjectIdentityV1,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    if root.object() == peer.object() {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::DuplicateDescriptors,
            "supervisor root and broker peer resolve to the same object",
        ));
    }
    Ok(())
}

fn require_distinct_pidfd_descriptor(
    root: ObjectIdentityV1,
    peer: ObjectIdentityV1,
    pidfd: ObjectIdentityV1,
) -> Result<(), BrokerAuthorityServiceAdmissionErrorV1> {
    if pidfd.object() == root.object() || pidfd.object() == peer.object() {
        return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
            AdmissionErrorKindV1::DuplicateDescriptors,
            "client pidfd resolves to the same object as another retained service descriptor",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::mem;
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Child, Command};
    use std::ptr;

    use rustix::net::{
        SocketAddrUnix, SocketFlags, accept_with, bind, connect, listen, socket_with, socketpair,
    };
    use tempfile::TempDir;

    fn protected_root() -> (TempDir, OwnedFd) {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let file = File::open(directory.path()).unwrap();
        (directory, file.into())
    }

    fn seqpacket() -> (OwnedFd, OwnedFd) {
        socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap()
    }

    fn set_close_on_exec(descriptor: &OwnedFd, enabled: bool) {
        let mut flags = rustix::io::fcntl_getfd(descriptor).unwrap();
        flags.set(rustix::io::FdFlags::CLOEXEC, enabled);
        rustix::io::fcntl_setfd(descriptor, flags).unwrap();
    }

    fn accepted_named_connection(
        server: &SocketAddrUnix,
        client_address: Option<&SocketAddrUnix>,
    ) -> (OwnedFd, OwnedFd) {
        let listener = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        bind(&listener, server).unwrap();
        listen(&listener, 1).unwrap();
        let client = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        if let Some(address) = client_address {
            bind(&client, address).unwrap();
        }
        connect(&client, server).unwrap();
        let accepted = accept_with(&listener, SocketFlags::CLOEXEC).unwrap();
        (accepted, client)
    }

    fn send_descriptor(socket: RawFd, descriptor: RawFd) -> io::Result<()> {
        let mut byte = 0x46_u8;
        let mut io_vector = libc::iovec {
            iov_base: ptr::from_mut(&mut byte).cast(),
            iov_len: 1,
        };
        let mut control = [0_usize; 8];
        // SAFETY: zero is the documented empty initialization for msghdr.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut io_vector;
        header.msg_iovlen = 1;
        header.msg_control = control.as_mut_ptr().cast();
        header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
        // SAFETY: control has enough aligned storage for one SCM_RIGHTS descriptor.
        unsafe {
            let message = libc::CMSG_FIRSTHDR(&header);
            (*message).cmsg_level = libc::SOL_SOCKET;
            (*message).cmsg_type = libc::SCM_RIGHTS;
            (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
            ptr::write_unaligned(libc::CMSG_DATA(message).cast::<RawFd>(), descriptor);
        }
        let sent = unsafe { libc::sendmsg(socket, &header, libc::MSG_NOSIGNAL) };
        if sent == 1 {
            Ok(())
        } else if sent < 0 {
            Err(io::Error::last_os_error())
        } else {
            Err(io::Error::from_raw_os_error(libc::EIO))
        }
    }

    fn receive_descriptor(socket: RawFd) -> io::Result<OwnedFd> {
        let mut byte = 0_u8;
        let mut io_vector = libc::iovec {
            iov_base: ptr::from_mut(&mut byte).cast(),
            iov_len: 1,
        };
        let mut control = [0_usize; 8];
        // SAFETY: zero is the documented empty initialization for msghdr.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut io_vector;
        header.msg_iovlen = 1;
        header.msg_control = control.as_mut_ptr().cast();
        header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
        let received = unsafe { libc::recvmsg(socket, &mut header, libc::MSG_CMSG_CLOEXEC) };
        if received < 0 {
            return Err(io::Error::last_os_error());
        }
        if received != 1
            || byte != 0x46
            || header.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
        {
            return Err(io::Error::from_raw_os_error(libc::EBADMSG));
        }
        // SAFETY: recvmsg succeeded and control storage is aligned and bounded by msg_controllen.
        let descriptor = unsafe {
            let message = libc::CMSG_FIRSTHDR(&header);
            if message.is_null()
                || (*message).cmsg_level != libc::SOL_SOCKET
                || (*message).cmsg_type != libc::SCM_RIGHTS
                || (*message).cmsg_len != libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize
                || !libc::CMSG_NXTHDR(&header, message).is_null()
            {
                return Err(io::Error::from_raw_os_error(libc::EBADMSG));
            }
            ptr::read_unaligned(libc::CMSG_DATA(message).cast::<RawFd>())
        };
        if descriptor < 0 {
            return Err(io::Error::from_raw_os_error(libc::EBADF));
        }
        // SAFETY: SCM_RIGHTS returned a fresh owned descriptor and MSG_CMSG_CLOEXEC marked it.
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }

    fn current_process_identity() -> ExpectedClientProcessIdentityV1 {
        process_identity(std::process::id())
    }

    fn process_identity(pid: u32) -> ExpectedClientProcessIdentityV1 {
        ExpectedClientProcessIdentityV1::new(
            pid,
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap()
    }

    fn try_pidfd_for(pid: u32, flags: libc::c_uint) -> io::Result<OwnedFd> {
        let pid = libc::pid_t::try_from(pid).unwrap();
        // SAFETY: pidfd_open takes one positive scalar PID and scalar UAPI flags. A nonnegative
        // return is a new close-on-exec descriptor owned by the caller.
        let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, flags) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful pidfd_open returned a fresh owned descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor as RawFd) })
    }

    fn pidfd_for(pid: u32) -> OwnedFd {
        try_pidfd_for(pid, 0).unwrap()
    }

    fn live_identity(expected: ExpectedClientProcessIdentityV1) -> LiveClientPidfdIdentityV1 {
        LiveClientPidfdIdentityV1::admit(pidfd_for(expected.pid()), expected).unwrap()
    }

    fn public_test_admit(
        root: OwnedFd,
        peer: OwnedFd,
        expected: ExpectedClientProcessIdentityV1,
    ) -> Result<ProtectedBrokerServiceAdmissionV1, BrokerAuthorityServiceAdmissionErrorV1> {
        ProtectedBrokerServiceAdmissionV1::admit(root, peer, live_identity(expected))
    }

    fn sleeping_child() -> Child {
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        crate::test_process_execution::spawn(&mut command).unwrap()
    }

    fn terminate_child(child: &mut Child) {
        let _ = child.kill();
        child.wait().unwrap();
    }

    fn wait_for_pidfd_exit(pidfd: &OwnedFd) {
        let mut descriptor = libc::pollfd {
            fd: pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one writable pollfd is supplied and the bounded timeout prevents a hung test.
        let ready = unsafe { libc::poll(&mut descriptor, 1, 5000) };
        assert_eq!(ready, 1, "pidfd did not report process exit");
        assert_ne!(descriptor.revents & (libc::POLLIN | libc::POLLHUP), 0);
    }

    // This bypasses only the distinct-UID policy so same-UID unit tests can exercise retained-FD
    // continuity. It is private test scaffolding and is not authority evidence.
    fn non_authoritative_test_admission(
        root: OwnedFd,
        peer: OwnedFd,
    ) -> ProtectedBrokerServiceAdmissionV1 {
        let credentials = PeerCredentialsV1::inspect(&peer).unwrap();
        let expected_client =
            ExpectedClientProcessIdentityV1::new(credentials.pid, credentials.uid, credentials.gid)
                .unwrap();
        non_authoritative_test_admission_with_live(root, peer, live_identity(expected_client))
    }

    fn non_authoritative_test_admission_with_live(
        root: OwnedFd,
        peer: OwnedFd,
        live_client: LiveClientPidfdIdentityV1,
    ) -> ProtectedBrokerServiceAdmissionV1 {
        try_non_authoritative_test_admission_with_live(root, peer, live_client).unwrap()
    }

    fn try_non_authoritative_test_admission_with_live(
        root: OwnedFd,
        peer: OwnedFd,
        live_client: LiveClientPidfdIdentityV1,
    ) -> Result<ProtectedBrokerServiceAdmissionV1, BrokerAuthorityServiceAdmissionErrorV1> {
        ProtectedBrokerServiceAdmissionV1::admit_non_authoritative_same_uid_session_test(
            root,
            peer,
            live_client,
        )
    }

    #[test]
    fn public_authority_marker_is_none() {
        assert_eq!(crate::BROKER_AUTHORITY_SERVICE_AUTHORITY_V1, "none");
    }

    #[test]
    fn expected_client_pid_must_be_nonzero() {
        let error = ExpectedClientProcessIdentityV1::new(0, 1, 1).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::ExpectedClientPid);
    }

    #[test]
    fn live_current_process_pidfd_is_admitted() {
        let identity = live_identity(current_process_identity());
        assert_ne!(identity.start_time_ticks, 0);
        identity.validate_liveness().unwrap();
        let debug = format!("{identity:?}");
        assert!(debug.contains("authority: \"none\""));
        assert!(!debug.contains("raw_fd"));
        assert!(!debug.contains("descriptor_identity"));
    }

    fn proc_stat_fixture(pid: u32, start_time_ticks: &str) -> Vec<u8> {
        let mut fields = vec!["R"; 19];
        fields.push(start_time_ticks);
        format!("{pid} (command with ) delimiters) {}\n", fields.join(" ")).into_bytes()
    }

    #[test]
    fn proc_stat_parser_binds_exact_pid_and_start_time() {
        let pid = std::process::id();
        let bytes = proc_stat_fixture(pid, "987654321");
        assert_eq!(
            parse_process_start_time_ticks(&bytes, pid).unwrap(),
            987_654_321
        );

        for wrong_pid in [pid.saturating_add(1), pid.saturating_sub(1)] {
            if wrong_pid != 0 && wrong_pid != pid {
                assert_eq!(
                    parse_process_start_time_ticks(&bytes, wrong_pid)
                        .unwrap_err()
                        .kind(),
                    AdmissionErrorKindV1::InspectClientStartTime
                );
            }
        }
    }

    #[test]
    fn proc_stat_parser_rejects_noncanonical_or_missing_start_time() {
        let pid = std::process::id();
        for start_time in ["0", "01", "-1", "+1", "x", "18446744073709551616"] {
            assert_eq!(
                parse_process_start_time_ticks(&proc_stat_fixture(pid, start_time), pid)
                    .unwrap_err()
                    .kind(),
                AdmissionErrorKindV1::InspectClientStartTime,
                "start time {start_time}"
            );
        }
        for malformed in [
            format!("{pid} command R 1 2 3"),
            format!("{pid} (command) R 1 2 3"),
            format!("0{pid} (command) {}", vec!["1"; 20].join(" ")),
            format!(
                "{} (command) {}",
                pid.saturating_add(1),
                vec!["1"; 20].join(" ")
            ),
        ] {
            assert_eq!(
                parse_process_start_time_ticks(malformed.as_bytes(), pid)
                    .unwrap_err()
                    .kind(),
                AdmissionErrorKindV1::InspectClientStartTime
            );
        }
    }

    #[test]
    fn protected_admission_matches_both_pid_and_captured_start_time() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        let pid = admission.live_client.expected_client.pid;
        let start_time = admission.live_client.start_time_ticks;
        assert!(admission.matches_client_process(pid, start_time));
        assert!(!admission.matches_client_process(pid, start_time.saturating_add(1)));
        assert!(!admission.matches_client_process(pid.saturating_add(1), start_time));
        admission
            .validate_non_authoritative_test_continuity()
            .unwrap();
    }

    #[test]
    fn ordinary_descriptor_runtime_ioctl_is_not_a_pidfd() {
        let descriptor: OwnedFd = File::open("/dev/null").unwrap().into();
        let error =
            LiveClientPidfdIdentityV1::admit(descriptor, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::InspectClientPidfd);
    }

    #[test]
    fn injected_legacy_ioctl_errnos_route_to_strict_procfs_fallback() {
        let pidfd = pidfd_for(std::process::id());
        for errno in [libc::ENOTTY, libc::EINVAL] {
            assert_eq!(
                dispatch_pidfd_get_info_error(&pidfd, io::Error::from_raw_os_error(errno)).unwrap(),
                PidfdTargetObservationV1 {
                    pid: std::process::id(),
                    source: PidfdIdentitySourceV1::ProcfsFdinfo,
                }
            );
        }
    }

    #[test]
    fn injected_legacy_ioctl_errnos_reject_an_ordinary_descriptor() {
        let descriptor: OwnedFd = File::open("/dev/null").unwrap().into();
        for errno in [libc::ENOTTY, libc::EINVAL] {
            assert_eq!(
                dispatch_pidfd_get_info_error(&descriptor, io::Error::from_raw_os_error(errno),)
                    .unwrap_err()
                    .kind(),
                AdmissionErrorKindV1::InspectClientPidfd
            );
        }
    }

    #[test]
    fn injected_nonfallback_ioctl_errnos_remain_errors() {
        let pidfd = pidfd_for(std::process::id());
        for errno in [libc::EACCES, libc::EFAULT, libc::EIO, libc::EPERM] {
            assert_eq!(
                dispatch_pidfd_get_info_error(&pidfd, io::Error::from_raw_os_error(errno))
                    .unwrap_err()
                    .kind(),
                AdmissionErrorKindV1::InspectClientPidfd
            );
        }
        assert_eq!(
            dispatch_pidfd_get_info_error(&pidfd, io::Error::from_raw_os_error(libc::ESRCH))
                .unwrap_err()
                .kind(),
            AdmissionErrorKindV1::ClientAlreadyDead
        );
    }

    #[test]
    fn pidfd_fdinfo_parser_accepts_one_exact_positive_pid() {
        let base_flags = u32::try_from(libc::O_RDWR | libc::O_CLOEXEC).unwrap();
        let nonblocking = base_flags | u32::try_from(libc::O_NONBLOCK).unwrap();
        let future_non_thread = base_flags | (1 << 30);
        for flags in [base_flags, nonblocking, future_non_thread] {
            let record = format!("pos:\t0\nflags:\t0{flags:o}\nPid:\t1234\nNSpid:\t1234\n");
            assert_eq!(parse_pidfd_fdinfo(&record).unwrap(), 1234);
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_rejects_thread_flag() {
        let flags = u32::try_from(libc::O_RDWR | libc::O_CLOEXEC).unwrap() | libc::PIDFD_THREAD;
        for flags in [flags, flags | (1 << 30)] {
            let record = format!("flags:\t0{flags:o}\nPid:\t1234\n");
            assert_eq!(
                parse_pidfd_fdinfo(&record).unwrap_err().kind(),
                AdmissionErrorKindV1::ClientPidfdThread
            );
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_rejects_missing_duplicate_and_malformed_flags() {
        for record in [
            "Pid:\t1\n",
            "flags:\t02000002\n",
            "flags:\t02000002\nflags:\t02000002\nPid:\t1\n",
            "flags:\t02000002\nflags: 02000002\nPid:\t1\n",
            "flags: 02000002\nPid:\t1\n",
            "flags:\t\nPid:\t1\n",
            "flags:\t0\nPid:\t1\n",
            "flags:\t2000002\nPid:\t1\n",
            "flags:\t02000008\nPid:\t1\n",
            "flags:\t02000002 \nPid:\t1\n",
            "flags:\t077777777777\nPid:\t1\n",
        ] {
            assert_eq!(
                parse_pidfd_fdinfo(record).unwrap_err().kind(),
                AdmissionErrorKindV1::InspectClientPidfd
            );
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_rejects_missing_duplicate_and_malformed_pid() {
        for record in [
            "flags:\t02000002\n",
            "flags:\t02000002\nPid:\t1\nPid:\t1\n",
            "flags:\t02000002\nPid:\t1\nPid: 1\n",
            "flags:\t02000002\nPid: 1\n",
            "flags:\t02000002\nPid:\t 1\n",
            "flags:\t02000002\nPid:\t1 \n",
            "flags:\t02000002\nPid:\t+1\n",
            "flags:\t02000002\nPid:\t01\n",
            "flags:\t02000002\nPid:\t-2\n",
            "flags:\t02000002\nPid:\t0\n",
            "flags:\t02000002\nPid:\t4294967296\n",
        ] {
            assert_eq!(
                parse_pidfd_fdinfo(record).unwrap_err().kind(),
                AdmissionErrorKindV1::InspectClientPidfd
            );
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_classifies_reaped_target_as_dead() {
        assert_eq!(
            parse_pidfd_fdinfo("flags:\t02000002\nPid:\t-1\n")
                .unwrap_err()
                .kind(),
            AdmissionErrorKindV1::ClientAlreadyDead
        );
    }

    #[test]
    fn pidfd_thread_runtime_rejection_or_explicit_capability_skip() {
        const ABI_PIN: &str = "Linux v6.12 include/uapi/linux/pidfd.h: PIDFD_THREAD=O_EXCL; fs/proc/fd.c: flags is 0%o file->f_flags";
        assert_eq!(
            libc::PIDFD_THREAD,
            libc::O_EXCL as libc::c_uint,
            "{ABI_PIN}"
        );
        let pidfd = match try_pidfd_for(std::process::id(), libc::PIDFD_THREAD) {
            Ok(pidfd) => pidfd,
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(libc::EINVAL) | Some(libc::ENOSYS)
                ) =>
            {
                eprintln!("PIDFD_THREAD_RUNTIME=skipped; kernel lacks {ABI_PIN}");
                return;
            }
            Err(error) => panic!("PIDFD_THREAD capability probe failed: {error}; {ABI_PIN}"),
        };
        let record =
            fs::read_to_string(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd())).unwrap();
        assert_eq!(
            parse_pidfd_fdinfo(&record).unwrap_err().kind(),
            AdmissionErrorKindV1::ClientPidfdThread
        );
        let error =
            LiveClientPidfdIdentityV1::admit(pidfd, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::ClientPidfdThread);
    }

    #[test]
    fn non_cloexec_pidfd_fails_closed() {
        let pidfd = pidfd_for(std::process::id());
        set_close_on_exec(&pidfd, false);
        let error =
            LiveClientPidfdIdentityV1::admit(pidfd, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::ClientPidfdCloseOnExec);
    }

    #[test]
    fn pidfd_for_another_live_process_fails_exact_pid_binding() {
        let mut first = sleeping_child();
        let mut second = sleeping_child();
        let error =
            LiveClientPidfdIdentityV1::admit(pidfd_for(first.id()), process_identity(second.id()))
                .unwrap_err();
        assert_eq!(
            error.kind(),
            AdmissionErrorKindV1::ClientPidfdTargetMismatch
        );
        terminate_child(&mut first);
        terminate_child(&mut second);
    }

    #[test]
    fn already_exited_pidfd_fails_without_reaping_child() {
        let mut command = Command::new("/bin/true");
        let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
        let pidfd = pidfd_for(child.id());
        wait_for_pidfd_exit(&pidfd);
        let error =
            LiveClientPidfdIdentityV1::admit(pidfd, process_identity(child.id())).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::ClientAlreadyDead);
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn liveness_revalidation_detects_exit_without_reaping_child() {
        let mut child = sleeping_child();
        let identity =
            LiveClientPidfdIdentityV1::admit(pidfd_for(child.id()), process_identity(child.id()))
                .unwrap();
        child.kill().unwrap();
        wait_for_pidfd_exit(&identity.pidfd);
        let error = identity.validate_liveness().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::ClientAlreadyDead);
        assert!(!child.wait().unwrap().success());
    }

    #[test]
    fn cleared_pidfd_cloexec_fails_liveness_revalidation() {
        let identity = live_identity(current_process_identity());
        set_close_on_exec(&identity.pidfd, false);
        let error = identity.validate_liveness().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::ClientPidfdCloseOnExec);
    }

    #[test]
    fn pidfd_descriptor_table_substitution_fails_closed() {
        let mut first = sleeping_child();
        let mut second = sleeping_child();
        let identity =
            LiveClientPidfdIdentityV1::admit(pidfd_for(first.id()), process_identity(first.id()))
                .unwrap();
        let replacement = pidfd_for(second.id());
        let target = identity.pidfd.as_raw_fd();
        // SAFETY: dup2 deliberately replaces the token-owned descriptor-table entry. The token
        // remains the unique Rust owner of that entry and the replacement remains separately owned.
        assert_eq!(
            unsafe { libc::dup2(replacement.as_raw_fd(), target) },
            target
        );
        set_close_on_exec(&identity.pidfd, true);
        let error = identity.validate_liveness().unwrap_err();
        assert!(matches!(
            error.kind(),
            AdmissionErrorKindV1::ClientPidfdIdentityChanged
                | AdmissionErrorKindV1::ClientPidfdTargetMismatch
        ));
        terminate_child(&mut first);
        terminate_child(&mut second);
    }

    #[test]
    fn non_cloexec_root_public_input_fails_closed() {
        let (_directory, root) = protected_root();
        set_close_on_exec(&root, false);
        let (peer, _client) = seqpacket();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootCloseOnExec);
    }

    #[test]
    fn non_cloexec_peer_public_input_fails_closed() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        set_close_on_exec(&peer, false);
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerCloseOnExec);
    }

    #[test]
    fn same_uid_client_fails_closed() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::SameUidClient);
    }

    #[test]
    fn root_must_be_an_exact_directory() {
        let root: OwnedFd = tempfile::tempfile().unwrap().into();
        let (peer, _client) = seqpacket();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootNotDirectory);
    }

    #[test]
    fn root_must_have_exact_mode_0700() {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o710)).unwrap();
        let root: OwnedFd = File::open(directory.path()).unwrap().into();
        let (peer, _client) = seqpacket();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootMode);
    }

    #[test]
    fn ordinary_file_is_not_a_peer() {
        let (_directory, root) = protected_root();
        let peer: OwnedFd = tempfile::tempfile().unwrap().into();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerDomain);
    }

    #[test]
    fn stream_socket_is_not_a_peer() {
        let (_directory, root) = protected_root();
        let (peer, _client) = socketpair(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerSocketType);
    }

    #[test]
    fn non_unix_socket_is_not_a_peer() {
        let (_directory, root) = protected_root();
        let peer = socket_with(
            AddressFamily::INET,
            SocketType::STREAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerDomain);
    }

    #[test]
    fn unconnected_seqpacket_is_not_a_peer() {
        let (_directory, root) = protected_root();
        let peer = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerNotConnected);
    }

    #[test]
    fn pathname_local_socket_address_fails_closed() {
        let socket_directory = tempfile::tempdir().unwrap();
        let server = SocketAddrUnix::new(socket_directory.path().join("server.sock")).unwrap();
        let (peer, _client) = accepted_named_connection(&server, None);
        let (_directory, root) = protected_root();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerLocalAddress);
    }

    #[test]
    fn pathname_remote_socket_address_fails_closed() {
        let socket_directory = tempfile::tempdir().unwrap();
        let server = SocketAddrUnix::new(socket_directory.path().join("server.sock")).unwrap();
        let client = SocketAddrUnix::new(socket_directory.path().join("client.sock")).unwrap();
        let (peer, _client) = accepted_named_connection(&server, Some(&client));
        let (_directory, root) = protected_root();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerRemoteAddress);
    }

    #[test]
    fn abstract_local_socket_address_fails_closed() {
        let name = format!("fe2o3-broker-local-{}", std::process::id());
        let server = SocketAddrUnix::new_abstract_name(name.as_bytes()).unwrap();
        let (peer, _client) = accepted_named_connection(&server, None);
        let (_directory, root) = protected_root();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerLocalAddress);
    }

    #[test]
    fn abstract_remote_socket_address_fails_closed() {
        let unique = std::process::id();
        let server =
            SocketAddrUnix::new_abstract_name(format!("fe2o3-broker-server-{unique}").as_bytes())
                .unwrap();
        let client =
            SocketAddrUnix::new_abstract_name(format!("fe2o3-broker-client-{unique}").as_bytes())
                .unwrap();
        let (peer, _client) = accepted_named_connection(&server, Some(&client));
        let (_directory, root) = protected_root();
        let error = public_test_admit(root, peer, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerRemoteAddress);
    }

    #[test]
    fn exact_expected_credentials_are_required() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let current = current_process_identity();
        let distinct_uid = if current.uid() == u32::MAX {
            current.uid() - 1
        } else {
            current.uid() + 1
        };
        let expected =
            ExpectedClientProcessIdentityV1::new(current.pid(), distinct_uid, current.gid())
                .unwrap();
        let error = public_test_admit(root, peer, expected).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerCredentialsMismatch);
    }

    #[test]
    fn duplicate_directory_description_fails_as_a_peer() {
        let (_directory, root) = protected_root();
        let duplicate = rustix::io::dup(&root).unwrap();
        set_close_on_exec(&duplicate, true);
        let error = public_test_admit(root, duplicate, current_process_identity()).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::DuplicateDescriptors);
    }

    #[test]
    fn retained_descriptors_validate_without_drift() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        non_authoritative_test_admission(root, peer)
            .validate_non_authoritative_test_continuity()
            .unwrap();
    }

    #[test]
    fn clearing_root_cloexec_fails_continuity() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        set_close_on_exec(&admission.root, false);
        let error = admission.validate_continuity().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootCloseOnExec);
    }

    #[test]
    fn clearing_peer_cloexec_fails_continuity() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        set_close_on_exec(&admission.peer, false);
        let error = admission.validate_continuity().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerCloseOnExec);
    }

    #[test]
    fn retained_descriptors_do_not_leak_across_exec() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        admission
            .validate_non_authoritative_test_continuity()
            .unwrap();
        let root_fd = admission.root.as_raw_fd();
        let peer_fd = admission.peer.as_raw_fd();
        let pidfd = admission.live_client.pidfd.as_raw_fd();
        let root_target = fs::read_link(format!("/proc/self/fd/{root_fd}"))
            .unwrap()
            .into_os_string();
        let peer_target = fs::read_link(format!("/proc/self/fd/{peer_fd}"))
            .unwrap()
            .into_os_string();
        let pidfd_target = fs::read_link(format!("/proc/self/fd/{pidfd}"))
            .unwrap()
            .into_os_string();
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(
                r#"
root=$(readlink "/proc/self/fd/$ROOT_FD" 2>/dev/null || :)
peer=$(readlink "/proc/self/fd/$PEER_FD" 2>/dev/null || :)
pidfd=$(readlink "/proc/self/fd/$PIDFD" 2>/dev/null || :)
[ "$root" != "$ROOT_TARGET" ] && [ "$peer" != "$PEER_TARGET" ] && [ "$pidfd" != "$PIDFD_TARGET" ]
"#,
            )
            .env("ROOT_FD", root_fd.to_string())
            .env("ROOT_TARGET", root_target)
            .env("PEER_FD", peer_fd.to_string())
            .env("PEER_TARGET", peer_target)
            .env("PIDFD", pidfd.to_string())
            .env("PIDFD_TARGET", pidfd_target);
        let status = crate::test_process_execution::status(&mut command).unwrap();
        assert!(status.success());
    }

    #[test]
    fn root_metadata_change_fails_continuity() {
        let (directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let error = admission.validate_continuity().unwrap_err();
        assert!(matches!(
            error.kind(),
            AdmissionErrorKindV1::RootMode | AdmissionErrorKindV1::RootIdentityChanged
        ));
    }

    #[test]
    fn unlinked_root_fails_continuity() {
        let (directory, root) = protected_root();
        let path = directory.path().to_owned();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        fs::remove_dir(&path).unwrap();
        let error = admission.validate_continuity().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootUnlinked);
    }

    #[test]
    fn root_link_count_change_fails_continuity() {
        let (directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        fs::create_dir(directory.path().join("child")).unwrap();
        let error = admission.validate_continuity().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootIdentityChanged);
    }

    #[test]
    fn root_descriptor_table_substitution_fails_continuity() {
        let (_directory, root) = protected_root();
        let root_raw = root.as_raw_fd();
        let (peer, _client) = seqpacket();
        let admission = non_authoritative_test_admission(root, peer);
        let (replacement_directory, replacement) = protected_root();
        assert_ne!(
            admission.root_identity,
            validate_root(&replacement, admission.service_uid).unwrap()
        );
        // SAFETY: dup2 atomically replaces the admission-owned descriptor-table entry for this
        // adversarial test. `admission` remains the unique Rust owner of the resulting entry.
        assert_eq!(
            unsafe { libc::dup2(replacement.as_raw_fd(), root_raw) },
            root_raw
        );
        set_close_on_exec(&admission.root, true);
        let error = admission.validate_continuity().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootIdentityChanged);
        drop(replacement_directory);
    }

    #[test]
    fn peer_descriptor_table_substitution_fails_continuity() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let peer_raw = peer.as_raw_fd();
        let admission = non_authoritative_test_admission(root, peer);
        let (replacement, _replacement_client) = socketpair(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        // SAFETY: as above, this replaces the admission-owned descriptor-table entry while
        // preserving unique Rust ownership of the descriptor number.
        assert_eq!(
            unsafe { libc::dup2(replacement.as_raw_fd(), peer_raw) },
            peer_raw
        );
        set_close_on_exec(&admission.peer, true);
        let error = admission.validate_continuity().unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerSocketType);
    }

    #[test]
    #[ignore = "helper process for same-UID pidfd/SO_PEERCRED binding tests"]
    fn same_uid_live_client_helper() {
        let Some(control) = std::env::var_os("FE2O3_BROKER_SAME_UID_CONTROL_FD") else {
            return;
        };
        let control: RawFd = control.to_string_lossy().parse().unwrap();
        // SAFETY: the parent transferred this inherited descriptor by clearing CLOEXEC before
        // spawn, and this helper takes its sole Rust ownership.
        let control = unsafe { OwnedFd::from_raw_fd(control) };
        let (transferred, held_peer) = seqpacket();
        send_descriptor(control.as_raw_fd(), transferred.as_raw_fd()).unwrap();
        let mut acknowledgment = [0_u8; 1];
        assert_eq!(rustix::io::read(&control, &mut acknowledgment).unwrap(), 1);
        assert_eq!(acknowledgment, [0x51]);
        drop(held_peer);
    }

    #[test]
    fn child_pidfd_is_bound_to_child_socket_peer_credentials() {
        let (control, child_control) = seqpacket();
        set_close_on_exec(&child_control, false);
        let child_control_fd = child_control.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("linux::tests::same_uid_live_client_helper")
            .arg("--ignored")
            .arg("--nocapture")
            .env(
                "FE2O3_BROKER_SAME_UID_CONTROL_FD",
                child_control_fd.to_string(),
            );
        let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
        drop(child_control);
        let retained_peer = receive_descriptor(control.as_raw_fd()).unwrap();
        let expected = process_identity(child.id());
        let live_client =
            LiveClientPidfdIdentityV1::admit(pidfd_for(child.id()), expected).unwrap();
        let (_directory, root) = protected_root();
        let admission =
            non_authoritative_test_admission_with_live(root, retained_peer, live_client);
        admission
            .validate_non_authoritative_test_continuity()
            .unwrap();
        assert_eq!(rustix::io::write(&control, &[0x51]).unwrap(), 1);
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn pidfd_for_service_process_does_not_bind_to_child_socket_peer() {
        let (control, child_control) = seqpacket();
        set_close_on_exec(&child_control, false);
        let child_control_fd = child_control.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("linux::tests::same_uid_live_client_helper")
            .arg("--ignored")
            .env(
                "FE2O3_BROKER_SAME_UID_CONTROL_FD",
                child_control_fd.to_string(),
            );
        let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
        drop(child_control);
        let retained_peer = receive_descriptor(control.as_raw_fd()).unwrap();
        let live_client = live_identity(current_process_identity());
        let (_directory, root) = protected_root();
        let error =
            try_non_authoritative_test_admission_with_live(root, retained_peer, live_client)
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerCredentialsMismatch);
        assert_eq!(rustix::io::write(&control, &[0x51]).unwrap(), 1);
        assert!(child.wait().unwrap().success());
    }

    #[test]
    #[ignore = "helper for the opt-in root distinct-UID admission fixture"]
    fn privileged_distinct_uid_client_helper() {
        let Some(control) = std::env::var_os("FE2O3_BROKER_TEST_CONTROL_FD") else {
            return;
        };
        let control: RawFd = control.to_string_lossy().parse().unwrap();
        // SAFETY: the opt-in parent transferred unique ownership of this inherited descriptor to
        // the helper process by clearing CLOEXEC immediately before spawn.
        let control = unsafe { OwnedFd::from_raw_fd(control) };
        const CLIENT_UID: libc::uid_t = 65_534;
        const CLIENT_GID: libc::gid_t = 65_534;
        // SAFETY: this ignored fixture requires a root/capability-bearing process and permanently
        // drops all supplementary, real, effective, and saved credentials before socketpair.
        unsafe {
            assert_eq!(libc::setgroups(0, ptr::null()), 0);
            assert_eq!(libc::setresgid(CLIENT_GID, CLIENT_GID, CLIENT_GID), 0);
            assert_eq!(libc::setresuid(CLIENT_UID, CLIENT_UID, CLIENT_UID), 0);
        }
        let (transferred, held_peer) = seqpacket();
        send_descriptor(control.as_raw_fd(), transferred.as_raw_fd()).unwrap();
        let mut acknowledgment = [0_u8; 1];
        assert_eq!(rustix::io::read(&control, &mut acknowledgment).unwrap(), 1);
        assert_eq!(acknowledgment, [0x41]);
        drop(held_peer);
    }

    #[test]
    #[ignore = "set FE2O3_RUN_PRIVILEGED_BROKER_TEST=1 and run as root"]
    fn privileged_distinct_uid_public_admission_fixture() {
        if std::env::var_os("FE2O3_RUN_PRIVILEGED_BROKER_TEST").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            eprintln!("privileged broker admission fixture not requested");
            return;
        }
        assert_eq!(rustix::process::geteuid().as_raw(), 0);
        let (control, child_control) = seqpacket();
        set_close_on_exec(&child_control, false);
        let child_control_fd = child_control.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("linux::tests::privileged_distinct_uid_client_helper")
            .arg("--ignored")
            .arg("--nocapture")
            .env("FE2O3_BROKER_TEST_CONTROL_FD", child_control_fd.to_string());
        let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
        drop(child_control);
        let retained_peer = receive_descriptor(control.as_raw_fd()).unwrap();
        let expected_client =
            ExpectedClientProcessIdentityV1::new(child.id(), 65_534, 65_534).unwrap();
        let (_directory, root) = protected_root();
        let admission = public_test_admit(root, retained_peer, expected_client).unwrap();
        admission.validate_continuity().unwrap();
        assert_eq!(rustix::io::write(&control, &[0x41]).unwrap(), 1);
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn closed_descriptor_fails_continuity() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let peer_raw = peer.as_raw_fd();
        let admission = non_authoritative_test_admission(root, peer);
        // SAFETY: this deliberately invalidates the admission-owned entry. `forget` prevents the
        // later destructor from closing a descriptor number that the process might have reused.
        assert_eq!(unsafe { libc::close(peer_raw) }, 0);
        let failed_closed = admission.validate_continuity().is_err();
        std::mem::forget(admission);
        // Parallel tests may reuse the deliberately closed descriptor number before validation;
        // either an invalid entry or any substituted object must fail closed.
        assert!(failed_closed);
    }
}
