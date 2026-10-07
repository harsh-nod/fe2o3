//! Private root/controller transport. None of these packets are application authority.

use crate::{other, require};
use rustix::{
    fs::{OFlags, SealFlags},
    net,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, IoSliceMut, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        unix::fs::{FileExt, MetadataExt, PermissionsExt},
    },
    time::Instant,
};

const HEADER: usize = 56;
const MAX_BODY: usize = 2048;
pub(crate) const READY: u8 = 1;
pub(crate) const START: u8 = 2;
pub(crate) const PROVED: u8 = 3;
pub(crate) const PROBE: u8 = 4;
pub(crate) const RETAINED: u8 = 5;
pub(crate) const RELEASE: u8 = 6;
pub(crate) const RELEASED: u8 = 7;
pub(crate) const REJECTED: u8 = 8;
pub(crate) const ACTIVATE: u8 = 9;
pub(crate) const ACTIVATED: u8 = 10;
pub(crate) const QUARANTINED: u8 = 11;
pub(crate) const REQUEST_BYTES: usize = 168;

pub(crate) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub(crate) fn nonce() -> io::Result<[u8; 32]> {
    let mut bytes = [0; 32];
    let mut offset = 0;
    while offset < bytes.len() {
        match rustix::rand::getrandom(&mut bytes[offset..], rustix::rand::GetRandomFlags::empty()) {
            Ok(0) => return Err(io::Error::other("empty random read")),
            Ok(n) => offset += n,
            Err(rustix::io::Errno::INTR) => (),
            Err(e) => return Err(e.into()),
        }
    }
    require(bytes != [0; 32], "zero session nonce")?;
    Ok(bytes)
}

pub(crate) struct Request {
    pub(crate) nonce: [u8; 32],
    pub(crate) parent: i32,
    pub(crate) kernel: [u8; 32],
    pub(crate) envelope: ([u8; 32], u64),
    pub(crate) payload: ([u8; 32], u64),
}
impl Request {
    pub(crate) fn encode(&self) -> [u8; REQUEST_BYTES] {
        let mut bytes = [0; REQUEST_BYTES];
        bytes[..8].copy_from_slice(b"F3PCRQ1\0");
        bytes[8..12].copy_from_slice(&self.parent.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.nonce);
        bytes[48..80].copy_from_slice(&self.kernel);
        bytes[80..112].copy_from_slice(&self.envelope.0);
        bytes[112..120].copy_from_slice(&self.envelope.1.to_le_bytes());
        bytes[120..152].copy_from_slice(&self.payload.0);
        bytes[152..160].copy_from_slice(&self.payload.1.to_le_bytes());
        bytes
    }
    pub(crate) fn decode(bytes: &[u8]) -> io::Result<Self> {
        require(bytes.len() == REQUEST_BYTES, "request length")?;
        require(
            &bytes[..8] == b"F3PCRQ1\0" && bytes[12..16] == [0; 4] && bytes[160..] == [0; 8],
            "request header",
        )?;
        let value = Self {
            parent: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            nonce: bytes[16..48].try_into().unwrap(),
            kernel: bytes[48..80].try_into().unwrap(),
            envelope: (
                bytes[80..112].try_into().unwrap(),
                u64::from_le_bytes(bytes[112..120].try_into().unwrap()),
            ),
            payload: (
                bytes[120..152].try_into().unwrap(),
                u64::from_le_bytes(bytes[152..160].try_into().unwrap()),
            ),
        };
        require(
            value.parent > 0
                && value.nonce != [0; 32]
                && value.kernel != [0; 32]
                && value.envelope.1 > 0
                && value.envelope.1
                    <= fe2o3_runtime_protocol::MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 as u64
                && value.payload.1 > 0
                && value.payload.1
                    <= fe2o3_kernel_analysis::MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1 as u64,
            "request bounds or identity",
        )?;
        Ok(value)
    }
}

pub(crate) fn wait(
    fd: BorrowedFd<'_>,
    event: rustix::event::PollFlags,
    deadline: Instant,
) -> io::Result<()> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "proof-controller deadline"))?;
        let timeout = rustix::event::Timespec {
            tv_sec: remaining.as_secs().try_into().map_err(other)?,
            tv_nsec: remaining.subsec_nanos().into(),
        };
        let mut fds = [rustix::event::PollFd::new(&fd, event)];
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "proof-controller deadline",
                ));
            }
            Ok(_) => return Ok(()),
            Err(rustix::io::Errno::INTR) => (),
            Err(e) => return Err(e.into()),
        }
    }
}

pub(crate) fn send(
    fd: BorrowedFd<'_>,
    nonce: [u8; 32],
    kind: u8,
    body: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    loop {
        wait(fd, rustix::event::PollFlags::OUT, deadline)?;
        if try_send(fd, nonce, kind, body)? {
            return Ok(());
        }
    }
}

/// One send attempt: backpressure and interruption preserve the complete frame.
pub(crate) fn try_send(
    fd: BorrowedFd<'_>,
    nonce: [u8; 32],
    kind: u8,
    body: &[u8],
) -> io::Result<bool> {
    require(
        (READY..=QUARANTINED).contains(&kind) && nonce != [0; 32] && body.len() <= MAX_BODY,
        "invalid outgoing controller frame",
    )?;
    let mut bytes = vec![0; HEADER + body.len()];
    bytes[..8].copy_from_slice(b"F3PCMS1\0");
    bytes[8] = kind;
    bytes[16..48].copy_from_slice(&nonce);
    bytes[48..52].copy_from_slice(&(body.len() as u32).to_le_bytes());
    bytes[HEADER..].copy_from_slice(body);
    match net::send(
        fd,
        &bytes,
        net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
    ) {
        Ok(n) => {
            require(n == bytes.len(), "partial controller frame")?;
            Ok(true)
        }
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn decode(bytes: &[u8], nonce: [u8; 32]) -> io::Result<(u8, Vec<u8>)> {
    require(
        (HEADER..=HEADER + MAX_BODY).contains(&bytes.len()),
        "controller frame length",
    )?;
    require(
        &bytes[..8] == b"F3PCMS1\0"
            && (READY..=QUARANTINED).contains(&bytes[8])
            && bytes[9..16] == [0; 7]
            && bytes[16..48] == nonce
            && bytes[48..52] == ((bytes.len() - HEADER) as u32).to_le_bytes()
            && bytes[52..56] == [0; 4],
        "controller frame header, nonce or length mismatch",
    )?;
    Ok((bytes[8], bytes[HEADER..].to_vec()))
}

pub(crate) fn receive(
    fd: BorrowedFd<'_>,
    sender: (i32, u32, u32),
    nonce: [u8; 32],
    deadline: Instant,
) -> io::Result<(u8, Vec<u8>)> {
    loop {
        wait(fd, rustix::event::PollFlags::IN, deadline)?;
        if let Some(packet) = try_receive(fd, sender, nonce)? {
            return Ok(packet);
        }
    }
}

/// At most one recvmsg, including on EINTR. No deadline or readiness wait is hidden here.
pub(crate) fn try_receive(
    fd: BorrowedFd<'_>,
    sender: (i32, u32, u32),
    nonce: [u8; 32],
) -> io::Result<Option<(u8, Vec<u8>)>> {
    try_receive_with::<{ HEADER + MAX_BODY }, _>(fd, sender, |bytes| decode(bytes, nonce))
}

/// Policy-neutral one-attempt credential/right checking; each caller owns its codec.
pub(crate) fn try_receive_with<const N: usize, T>(
    fd: BorrowedFd<'_>,
    sender: (i32, u32, u32),
    decode: impl FnOnce(&[u8]) -> io::Result<T>,
) -> io::Result<Option<T>> {
    let mut bytes = [0; N];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(1))];
    let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
    let message = match net::recvmsg(
        fd,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
    ) {
        Ok(m) => m,
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    require(
        message.bytes > 0
            && message.bytes <= bytes.len()
            && (message.flags - net::ReturnFlags::CMSG_CLOEXEC).is_empty(),
        "closed or truncated controller packet",
    )?;
    let mut credentials = None;
    for item in ancillary.drain() {
        match item {
            net::RecvAncillaryMessage::ScmCredentials(c) if credentials.is_none() => {
                credentials = Some(c)
            }
            _ => return Err(io::Error::other("unexpected controller ancillary data")),
        }
    }
    let c = credentials.ok_or_else(|| io::Error::other("missing controller sender credentials"))?;
    require(
        (c.pid.as_raw_pid(), c.uid.as_raw(), c.gid.as_raw()) == sender,
        "controller sender mismatch",
    )?;
    decode(&bytes[..message.bytes]).map(Some)
}

fn control_addresses(fd: BorrowedFd<'_>) -> io::Result<(net::SocketAddrUnix, net::SocketAddrUnix)> {
    require(
        net::sockopt::socket_type(fd)? == net::SocketType::SEQPACKET
            && net::sockopt::socket_domain(fd)? == net::AddressFamily::UNIX
            && !net::sockopt::socket_acceptconn(fd)?
            && rustix::fs::fcntl_getfl(fd)? == OFlags::RDWR | OFlags::NONBLOCK
            && rustix::io::fcntl_getfd(fd)? == rustix::io::FdFlags::CLOEXEC
            && net::sockopt::socket_passcred(fd)?,
        "invalid controller endpoint",
    )?;
    let local = net::SocketAddrUnix::try_from(net::getsockname(fd)?)?;
    let peer = net::SocketAddrUnix::try_from(
        net::getpeername(fd)?
            .ok_or_else(|| io::Error::other("disconnected controller endpoint"))?,
    )?;
    require(
        local.abstract_name().is_some() && peer.abstract_name().is_some() && local != peer,
        "controller endpoints lack distinct abstract addresses",
    )?;
    Ok((local, peer))
}

pub(crate) fn control_pair() -> io::Result<(OwnedFd, OwnedFd)> {
    let pair = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )?;
    for fd in [&pair.0, &pair.1] {
        net::sockopt::set_socket_passcred(fd, true)?;
        // Finish PASSCRED autobinding before either immutable endpoint snapshot is captured.
        net::bind(fd, &net::SocketAddrUnix::new_unnamed())?;
    }
    Ok(pair)
}

pub(crate) struct ControlEndpoint {
    fd: OwnedFd,
    identity: (u64, u64, u32),
    creator: (i32, u32, u32),
    addresses: (net::SocketAddrUnix, net::SocketAddrUnix),
}
impl ControlEndpoint {
    pub(crate) fn fingerprint(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/PROOF-CONTROLLER-ENDPOINT/V1\0");
        hash.update(self.identity.0.to_le_bytes());
        hash.update(self.identity.1.to_le_bytes());
        hash.update(self.identity.2.to_le_bytes());
        hash.update(self.creator.0.to_le_bytes());
        hash.update(self.creator.1.to_le_bytes());
        hash.update(self.creator.2.to_le_bytes());
        for address in [&self.addresses.0, &self.addresses.1] {
            let bytes = address.abstract_name().expect("admitted abstract address");
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hash.finalize().into()
    }
    pub(crate) fn admit(fd: OwnedFd) -> io::Result<Self> {
        let stat = rustix::fs::fstat(&fd)?;
        let creator = net::sockopt::socket_peercred(&fd)?;
        let value = Self {
            addresses: control_addresses(fd.as_fd())?,
            identity: (stat.st_dev, stat.st_ino, stat.st_mode),
            creator: (
                creator.pid.as_raw_pid(),
                creator.uid.as_raw(),
                creator.gid.as_raw(),
            ),
            fd,
        };
        value.revalidate()?;
        Ok(value)
    }
    pub(crate) fn revalidate(&self) -> io::Result<()> {
        let stat = rustix::fs::fstat(&self.fd)?;
        let creator = net::sockopt::socket_peercred(&self.fd)?;
        require(
            (stat.st_dev, stat.st_ino, stat.st_mode) == self.identity
                && (
                    creator.pid.as_raw_pid(),
                    creator.uid.as_raw(),
                    creator.gid.as_raw(),
                ) == self.creator
                && control_addresses(self.as_fd())? == self.addresses,
            "controller endpoint continuity changed",
        )
    }
}
impl AsFd for ControlEndpoint {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

pub(crate) fn seal(bytes: &[u8]) -> io::Result<File> {
    let mut file = File::from(rustix::fs::memfd_create(
        c"fe2o3-proof-controller-input-v1",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?);
    file.write_all(bytes)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o400))?;
    rustix::fs::fcntl_add_seals(
        &file,
        SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE,
    )?;
    Ok(File::from(rustix::fs::open(
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        OFlags::RDONLY | OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?))
}

pub(crate) fn read_sealed(file: &File, maximum: usize) -> io::Result<Box<[u8]>> {
    read_sealed_for_owner(file, maximum, (0, 0))
}

pub(crate) fn read_application_sealed(
    file: &File,
    owner: (u32, u32),
    expected: ([u8; 32], u64),
    maximum: usize,
) -> io::Result<Box<[u8]>> {
    require(
        owner.0 > 0 && owner.1 > 0 && expected.1 > 0 && expected.1 <= maximum as u64,
        "application input owner or bounds",
    )?;
    require(
        rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC
            && file.metadata()?.len() == expected.1,
        "application input flags or declared length",
    )?;
    let bytes = read_sealed_for_owner(file, maximum, owner)?;
    require(
        digest(&bytes) == expected.0
            && rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC,
        "application input hash or flags changed",
    )?;
    Ok(bytes)
}

fn read_sealed_for_owner(file: &File, maximum: usize, owner: (u32, u32)) -> io::Result<Box<[u8]>> {
    let before = file.metadata()?;
    require(
        before.is_file()
            && before.nlink() == 0
            && (before.uid(), before.gid()) == owner
            && before.mode() & 0o7777 == 0o400
            && before.len() > 0
            && before.len() <= maximum as u64,
        "sealed controller input metadata",
    )?;
    let flags = rustix::fs::fcntl_getfl(file)?;
    require(
        flags & OFlags::ACCMODE == OFlags::RDONLY
            && !flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT),
        "sealed controller input flags",
    )?;
    let seals = SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE;
    require(
        rustix::fs::fcntl_get_seals(file)? == seals,
        "sealed controller input seals",
    )?;
    let link = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let link = link.as_os_str().as_encoded_bytes();
    require(
        (link.starts_with(b"/memfd:") || link.starts_with(b"memfd:"))
            && link.ends_with(b" (deleted)"),
        "controller input is not memfd",
    )?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(before.len() as usize)
        .map_err(other)?;
    bytes.resize(before.len() as usize, 0);
    file.read_exact_at(&mut bytes, 0)?;
    require(
        file.read_at(&mut [0], before.len())? == 0,
        "sealed input EOF changed",
    )?;
    let snapshot = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.uid(),
            m.gid(),
            m.nlink(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    require(
        snapshot(&before) == snapshot(&file.metadata()?)
            && rustix::fs::fcntl_getfl(file)? == flags
            && rustix::fs::fcntl_get_seals(file)? == seals,
        "sealed controller input changed",
    )?;
    Ok(bytes.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::IoSlice;
    use std::os::fd::AsFd;

    fn application_input(bytes: &[u8]) -> (File, (u32, u32)) {
        let file = seal(bytes).unwrap();
        let owner = if rustix::process::getuid().is_root() {
            (1000, 1000)
        } else {
            (
                rustix::process::getuid().as_raw(),
                rustix::process::getgid().as_raw(),
            )
        };
        if rustix::process::getuid().is_root() {
            rustix::fs::fchown(
                &file,
                Some(rustix::process::Uid::from_raw(owner.0)),
                Some(rustix::process::Gid::from_raw(owner.1)),
            )
            .unwrap();
        }
        (file, owner)
    }

    #[test]
    fn application_sealed_reader_preserves_original_and_separates_root_ownership() {
        let bytes = b"exact application-owned input";
        let (file, owner) = application_input(bytes);
        let expected = (digest(bytes), bytes.len() as u64);
        let duplicate = file.try_clone().unwrap();
        assert_eq!(
            read_application_sealed(&file, owner, expected, 128)
                .unwrap()
                .as_ref(),
            bytes
        );
        assert_eq!(
            read_application_sealed(&duplicate, owner, expected, 128)
                .unwrap()
                .as_ref(),
            bytes
        );
        assert!(read_sealed(&file, 128).is_err());
        for owner in [(0, 0), (owner.0 ^ 1, owner.1), (owner.0, owner.1 ^ 1)] {
            assert!(read_application_sealed(&file, owner, expected, 128).is_err());
        }
        for expected in [
            (digest(b"different"), expected.1),
            (expected.0, expected.1 + 1),
            (expected.0, 0),
        ] {
            assert!(read_application_sealed(&file, owner, expected, 128).is_err());
        }
        assert!(read_application_sealed(&file, owner, expected, bytes.len() - 1).is_err());
    }

    #[test]
    fn application_sealed_reader_rejects_mutated_flags_permissions_and_unsealed_inputs() {
        let bytes = b"input";
        let (file, owner) = application_input(bytes);
        let expected = (digest(bytes), bytes.len() as u64);
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        assert!(read_application_sealed(&file, owner, expected, 128).is_err());
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .unwrap();
        assert!(read_application_sealed(&file, owner, expected, 128).is_err());
        file.set_permissions(std::fs::Permissions::from_mode(0o400))
            .unwrap();
        let writable = File::from(
            rustix::fs::memfd_create(
                c"unsealed-application-test",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )
            .unwrap(),
        );
        writable.write_all_at(bytes, 0).unwrap();
        if rustix::process::getuid().is_root() {
            rustix::fs::fchown(
                &writable,
                Some(rustix::process::Uid::from_raw(owner.0)),
                Some(rustix::process::Gid::from_raw(owner.1)),
            )
            .unwrap();
        }
        writable
            .set_permissions(std::fs::Permissions::from_mode(0o400))
            .unwrap();
        assert!(read_application_sealed(&writable, owner, expected, 128).is_err());
        let readonly = File::from(
            rustix::fs::open(
                format!("/proc/self/fd/{}", writable.as_raw_fd()),
                OFlags::RDONLY | OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .unwrap(),
        );
        assert!(read_application_sealed(&readonly, owner, expected, 128).is_err());
        assert_eq!(
            read_application_sealed(&file, owner, expected, 128)
                .unwrap()
                .as_ref(),
            bytes
        );
    }

    #[test]
    fn endpoint_fingerprint_distinguishes_same_creator_pairs_and_endpoint_sides() {
        let (a, b) = control_pair().unwrap();
        let (c, _d) = control_pair().unwrap();
        let duplicate =
            ControlEndpoint::admit(rustix::io::fcntl_dupfd_cloexec(&a, 0).unwrap()).unwrap();
        let a = ControlEndpoint::admit(a).unwrap();
        let b = ControlEndpoint::admit(b).unwrap();
        let c = ControlEndpoint::admit(c).unwrap();
        assert_eq!(a.fingerprint(), duplicate.fingerprint());
        assert_ne!(a.fingerprint(), b.fingerprint());
        assert_ne!(a.fingerprint(), c.fingerprint());
    }
    #[test]
    fn exact_nonce_credentials_and_ancillary_are_required() {
        let (a, b) = control_pair().unwrap();
        let a = ControlEndpoint::admit(a).unwrap();
        let b = ControlEndpoint::admit(b).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let sender = (
            std::process::id() as i32,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        send(a.as_fd(), [1; 32], PROBE, &[], deadline).unwrap();
        assert_eq!(
            receive(b.as_fd(), sender, [1; 32], deadline).unwrap(),
            (PROBE, vec![])
        );
        a.revalidate().unwrap();
        b.revalidate().unwrap();
        send(a.as_fd(), [1; 32], PROBE, &[], deadline).unwrap();
        assert!(receive(b.as_fd(), sender, [2; 32], deadline).is_err());
        send(a.as_fd(), [1; 32], PROBE, &[], deadline).unwrap();
        assert!(
            receive(
                b.as_fd(),
                (sender.0, sender.1 ^ 1, sender.2),
                [1; 32],
                deadline
            )
            .is_err()
        );
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
        let rights = [a.as_fd()];
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(&rights)));
        net::sendmsg(
            &a,
            &[IoSlice::new(&[0; HEADER])],
            &mut ancillary,
            net::SendFlags::NOSIGNAL,
        )
        .unwrap();
        assert!(receive(b.as_fd(), sender, [1; 32], deadline).is_err());
    }

    #[test]
    fn revalidation_rejects_disabled_credentials_without_repair() {
        let (a, _b) = control_pair().unwrap();
        let a = ControlEndpoint::admit(a).unwrap();
        net::sockopt::set_socket_passcred(&a, false).unwrap();
        assert!(a.revalidate().is_err());
        assert!(!net::sockopt::socket_passcred(&a).unwrap());
    }

    #[test]
    fn polling_distinguishes_no_packet_from_eof_and_preserves_backpressured_frames() {
        let (a, b) = control_pair().unwrap();
        let sender = (
            std::process::id() as i32,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        assert!(try_receive(b.as_fd(), sender, [1; 32]).unwrap().is_none());
        net::sockopt::set_socket_send_buffer_size(&a, 4096).unwrap();
        let mut sent = 0_u32;
        while try_send(a.as_fd(), [1; 32], PROBE, &sent.to_le_bytes()).unwrap() {
            sent += 1;
            assert!(sent < 4096, "bounded socket never applied backpressure");
        }
        assert!(sent > 0);
        for expected in 0..sent {
            assert_eq!(
                try_receive(b.as_fd(), sender, [1; 32]).unwrap(),
                Some((PROBE, expected.to_le_bytes().to_vec()))
            );
        }
        assert!(try_receive(b.as_fd(), sender, [1; 32]).unwrap().is_none());
        assert!(try_send(a.as_fd(), [1; 32], PROBE, &sent.to_le_bytes()).unwrap());
        assert_eq!(
            try_receive(b.as_fd(), sender, [1; 32]).unwrap(),
            Some((PROBE, sent.to_le_bytes().to_vec()))
        );
        drop(a);
        assert!(try_receive(b.as_fd(), sender, [1; 32]).is_err());
    }
}
