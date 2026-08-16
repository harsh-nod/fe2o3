use std::error::Error;
use std::fmt;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd};

use rustix::net::{AddressFamily, SocketType};

const DIRECTORY_PERMISSIONS: u32 = 0o700;
const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;

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
/// file descriptions and peer credentials against their admission snapshots.
pub struct ProtectedBrokerServiceAdmissionV1 {
    root: OwnedFd,
    peer: OwnedFd,
    service_uid: u32,
    root_identity: ObjectIdentityV1,
    peer_identity: ObjectIdentityV1,
    expected_client: ExpectedClientProcessIdentityV1,
}

impl fmt::Debug for ProtectedBrokerServiceAdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedBrokerServiceAdmissionV1")
            .field("authority", &"none")
            .field("service_uid", &self.service_uid)
            .field("root_identity", &self.root_identity)
            .field("peer_identity", &self.peer_identity)
            .field("expected_client", &self.expected_client)
            .finish_non_exhaustive()
    }
}

impl ProtectedBrokerServiceAdmissionV1 {
    /// Admits only supervisor-owned descriptors and an exact expected connection-time identity.
    ///
    /// The caller must run as the protected service UID. The retained directory must be owned by
    /// that UID with exact mode `0700`. The retained peer must be a connected Unix
    /// `SOCK_SEQPACKET` endpoint with unnamed local and peer addresses. Its kernel-reported PID,
    /// UID, and GID must exactly match `expected_client`, whose UID must differ from the service.
    pub fn admit(
        supervisor_root: OwnedFd,
        retained_peer: OwnedFd,
        expected_client: ExpectedClientProcessIdentityV1,
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
        let peer_identity = validate_peer_shape(&retained_peer)?;
        if peer_identity != retained_peer_identity {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerIdentityChanged,
                "retained broker peer identity changed during admission",
            ));
        }
        if expected_client.uid == service_uid {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::SameUidClient,
                "expected broker client UID equals protected service effective UID",
            ));
        }
        let peer_credentials = PeerCredentialsV1::inspect(&retained_peer)?;
        if peer_credentials != expected_client.credentials() {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::PeerCredentialsMismatch,
                "broker peer SO_PEERCRED does not match the supervisor-supplied client identity",
            ));
        }
        Self::finish_admission(
            supervisor_root,
            retained_peer,
            service_uid,
            root_identity,
            peer_identity,
            expected_client,
        )
    }

    fn finish_admission(
        root: OwnedFd,
        peer: OwnedFd,
        service_uid: u32,
        root_identity: ObjectIdentityV1,
        peer_identity: ObjectIdentityV1,
        expected_client: ExpectedClientProcessIdentityV1,
    ) -> Result<Self, BrokerAuthorityServiceAdmissionErrorV1> {
        let admission = Self {
            root,
            peer,
            service_uid,
            root_identity,
            peer_identity,
            expected_client,
        };
        admission.validate_continuity()?;
        Ok(admission)
    }

    /// Revalidates service UID, root security metadata, socket shape, object identities, and
    /// `SO_PEERCRED`. Any descriptor-table substitution or metadata drift fails closed.
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

        if REQUIRE_DISTINCT_UID && self.expected_client.uid == self.service_uid {
            return Err(BrokerAuthorityServiceAdmissionErrorV1::new(
                AdmissionErrorKindV1::SameUidClient,
                "expected broker client UID equals protected service effective UID",
            ));
        }
        let credentials = PeerCredentialsV1::inspect(&self.peer)?;
        if credentials != self.expected_client.credentials() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::mem;
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
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
        ExpectedClientProcessIdentityV1::new(
            std::process::id(),
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap()
    }

    // This bypasses only the distinct-UID policy so same-UID unit tests can exercise retained-FD
    // continuity. It is private test scaffolding and is not authority evidence.
    fn non_authoritative_test_admission(
        root: OwnedFd,
        peer: OwnedFd,
    ) -> ProtectedBrokerServiceAdmissionV1 {
        let service_uid = rustix::process::geteuid().as_raw();
        require_close_on_exec(
            &root,
            AdmissionErrorKindV1::RootCloseOnExec,
            "supervisor root",
        )
        .unwrap();
        require_close_on_exec(&peer, AdmissionErrorKindV1::PeerCloseOnExec, "broker peer").unwrap();
        let root_identity = validate_root(&root, service_uid).unwrap();
        let peer_identity = validate_peer_shape(&peer).unwrap();
        require_distinct_descriptors(root_identity, peer_identity).unwrap();
        let credentials = PeerCredentialsV1::inspect(&peer).unwrap();
        ProtectedBrokerServiceAdmissionV1 {
            root,
            peer,
            service_uid,
            root_identity,
            peer_identity,
            expected_client: ExpectedClientProcessIdentityV1::new(
                credentials.pid,
                credentials.uid,
                credentials.gid,
            )
            .unwrap(),
        }
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
    fn non_cloexec_root_public_input_fails_closed() {
        let (_directory, root) = protected_root();
        set_close_on_exec(&root, false);
        let (peer, _client) = seqpacket();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootCloseOnExec);
    }

    #[test]
    fn non_cloexec_peer_public_input_fails_closed() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        set_close_on_exec(&peer, false);
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerCloseOnExec);
    }

    #[test]
    fn same_uid_client_fails_closed() {
        let (_directory, root) = protected_root();
        let (peer, _client) = seqpacket();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::SameUidClient);
    }

    #[test]
    fn root_must_be_an_exact_directory() {
        let root: OwnedFd = tempfile::tempfile().unwrap().into();
        let (peer, _client) = seqpacket();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootNotDirectory);
    }

    #[test]
    fn root_must_have_exact_mode_0700() {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o710)).unwrap();
        let root: OwnedFd = File::open(directory.path()).unwrap().into();
        let (peer, _client) = seqpacket();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::RootMode);
    }

    #[test]
    fn ordinary_file_is_not_a_peer() {
        let (_directory, root) = protected_root();
        let peer: OwnedFd = tempfile::tempfile().unwrap().into();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
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
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
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
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
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
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerNotConnected);
    }

    #[test]
    fn pathname_local_socket_address_fails_closed() {
        let socket_directory = tempfile::tempdir().unwrap();
        let server = SocketAddrUnix::new(socket_directory.path().join("server.sock")).unwrap();
        let (peer, _client) = accepted_named_connection(&server, None);
        let (_directory, root) = protected_root();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerLocalAddress);
    }

    #[test]
    fn pathname_remote_socket_address_fails_closed() {
        let socket_directory = tempfile::tempdir().unwrap();
        let server = SocketAddrUnix::new(socket_directory.path().join("server.sock")).unwrap();
        let client = SocketAddrUnix::new(socket_directory.path().join("client.sock")).unwrap();
        let (peer, _client) = accepted_named_connection(&server, Some(&client));
        let (_directory, root) = protected_root();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerRemoteAddress);
    }

    #[test]
    fn abstract_local_socket_address_fails_closed() {
        let name = format!("fe2o3-broker-local-{}", std::process::id());
        let server = SocketAddrUnix::new_abstract_name(name.as_bytes()).unwrap();
        let (peer, _client) = accepted_named_connection(&server, None);
        let (_directory, root) = protected_root();
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
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
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, peer, current_process_identity())
                .unwrap_err();
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
        let error = ProtectedBrokerServiceAdmissionV1::admit(root, peer, expected).unwrap_err();
        assert_eq!(error.kind(), AdmissionErrorKindV1::PeerCredentialsMismatch);
    }

    #[test]
    fn duplicate_directory_description_fails_as_a_peer() {
        let (_directory, root) = protected_root();
        let duplicate = rustix::io::dup(&root).unwrap();
        set_close_on_exec(&duplicate, true);
        let error =
            ProtectedBrokerServiceAdmissionV1::admit(root, duplicate, current_process_identity())
                .unwrap_err();
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
        let root_target = fs::read_link(format!("/proc/self/fd/{root_fd}"))
            .unwrap()
            .into_os_string();
        let peer_target = fs::read_link(format!("/proc/self/fd/{peer_fd}"))
            .unwrap()
            .into_os_string();
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(
                r#"
root=$(readlink "/proc/self/fd/$ROOT_FD" 2>/dev/null || :)
peer=$(readlink "/proc/self/fd/$PEER_FD" 2>/dev/null || :)
[ "$root" != "$ROOT_TARGET" ] && [ "$peer" != "$PEER_TARGET" ]
"#,
            )
            .env("ROOT_FD", root_fd.to_string())
            .env("ROOT_TARGET", root_target)
            .env("PEER_FD", peer_fd.to_string())
            .env("PEER_TARGET", peer_target)
            .status()
            .unwrap();
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
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("linux::tests::privileged_distinct_uid_client_helper")
            .arg("--ignored")
            .arg("--nocapture")
            .env("FE2O3_BROKER_TEST_CONTROL_FD", child_control_fd.to_string())
            .spawn()
            .unwrap();
        drop(child_control);
        let retained_peer = receive_descriptor(control.as_raw_fd()).unwrap();
        let expected_client =
            ExpectedClientProcessIdentityV1::new(child.id(), 65_534, 65_534).unwrap();
        let (_directory, root) = protected_root();
        let admission =
            ProtectedBrokerServiceAdmissionV1::admit(root, retained_peer, expected_client).unwrap();
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
        let kind = admission.validate_continuity().unwrap_err().kind();
        std::mem::forget(admission);
        assert_eq!(kind, AdmissionErrorKindV1::PeerCloseOnExec);
    }
}
