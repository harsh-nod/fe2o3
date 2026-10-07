use std::{
    fs::File,
    io::{self, IoSlice, IoSliceMut, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        unix::fs::{FileExt, MetadataExt, PermissionsExt},
    },
    time::{Duration, Instant},
};

use rustix::{fs::OFlags, net};

use super::{Result, invalid, require};

pub(super) const DELEGATE: u8 = 1;
pub(super) const ACTIVATED: u8 = 2;
pub(super) const HELLO: u8 = 3;
pub(super) const HELLO_ACK: u8 = 4;
pub(super) const EXECUTE: u8 = 5;
pub(super) const EXECUTED: u8 = 6;
pub(super) const PROBE: u8 = 7;
pub(super) const PROBED: u8 = 8;
const HEADER: usize = 96;
pub(super) const MAX_OUTPUT: usize = 16 * 1024;
const MAX_BODY: usize = 144 + 2 * MAX_OUTPUT;
pub(super) const FRAME_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const MAX_PROOF_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Frame {
    pub(super) kind: u8,
    pub(super) session: [u8; 32],
    pub(super) sequence: u64,
    pub(super) challenge: [u8; 32],
    pub(super) body: Vec<u8>,
}

impl Frame {
    fn rights_count(&self) -> usize {
        match self.kind {
            DELEGATE => 2,
            EXECUTE => 1,
            _ => 0,
        }
    }

    fn encode(&self) -> Result<Vec<u8>> {
        require(
            (DELEGATE..=PROBED).contains(&self.kind)
                && self.session != [0; 32]
                && self.body.len() <= MAX_BODY
                && ((self.kind <= ACTIVATED && self.sequence == 0 && self.challenge == [0; 32])
                    || (self.kind > ACTIVATED && self.sequence > 0 && self.challenge != [0; 32])),
            "invalid compiler-proof frame fields",
        )?;
        let mut bytes = vec![0; HEADER + self.body.len()];
        bytes[..8].copy_from_slice(b"F3CPRV1\0");
        bytes[8] = self.kind;
        bytes[16..48].copy_from_slice(&self.session);
        bytes[48..56].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[56..88].copy_from_slice(&self.challenge);
        bytes[88..92].copy_from_slice(&(self.body.len() as u32).to_le_bytes());
        bytes[HEADER..].copy_from_slice(&self.body);
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        require(
            (HEADER..=HEADER + MAX_BODY).contains(&bytes.len()),
            "compiler-proof frame length",
        )?;
        require(
            &bytes[..8] == b"F3CPRV1\0"
                && bytes[9..16] == [0; 7]
                && bytes[88..92] == ((bytes.len() - HEADER) as u32).to_le_bytes()
                && bytes[92..96] == [0; 4],
            "compiler-proof frame header",
        )?;
        let frame = Self {
            kind: bytes[8],
            session: bytes[16..48].try_into().unwrap(),
            sequence: u64::from_le_bytes(bytes[48..56].try_into().unwrap()),
            challenge: bytes[56..88].try_into().unwrap(),
            body: bytes[HEADER..].to_vec(),
        };
        require(
            frame.encode()? == bytes,
            "noncanonical compiler-proof frame",
        )?;
        Ok(frame)
    }

    pub(super) fn reply(&self, kind: u8, body: Vec<u8>) -> Self {
        Self {
            kind,
            session: self.session,
            sequence: self.sequence,
            challenge: self.challenge,
            body,
        }
    }

    pub(super) fn require_reply(&self, request: &Self, kind: u8) -> Result<()> {
        require(
            self.kind == kind
                && self.session == request.session
                && self.sequence == request.sequence
                && self.challenge == request.challenge,
            "compiler-proof response operation mismatch",
        )
    }
}

pub(super) type Credentials = (u32, u32, u32);

pub(super) fn current_credentials() -> Credentials {
    (
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
}

pub(super) fn nonce() -> Result<[u8; 32]> {
    let mut bytes = [0; 32];
    let mut offset = 0;
    while offset < bytes.len() {
        match rustix::rand::getrandom(&mut bytes[offset..], rustix::rand::GetRandomFlags::empty()) {
            Ok(0) => return Err(invalid("empty compiler-proof nonce read")),
            Ok(count) => offset += count,
            Err(rustix::io::Errno::INTR) => (),
            Err(error) => return Err(error.into()),
        }
    }
    require(bytes != [0; 32], "zero compiler-proof nonce")?;
    Ok(bytes)
}

pub(super) fn check_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "compiler-proof deadline",
        ));
    }
    Ok(())
}

fn monotonic_nanos() -> Result<u64> {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let seconds = u64::try_from(time.tv_sec).map_err(io::Error::other)?;
    let nanos = u64::try_from(time.tv_nsec).map_err(io::Error::other)?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(nanos))
        .ok_or_else(|| invalid("compiler-proof monotonic clock overflow"))
}

pub(super) fn encode_deadline(deadline: Instant) -> Result<u64> {
    let clock = monotonic_nanos()?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    require(
        !remaining.is_zero() && remaining <= MAX_PROOF_TIMEOUT,
        "compiler-proof execution deadline bounds",
    )?;
    clock
        .checked_add(u64::try_from(remaining.as_nanos()).map_err(io::Error::other)?)
        .ok_or_else(|| invalid("compiler-proof execution deadline overflow"))
}

pub(super) fn decode_deadline(encoded: u64) -> Result<Instant> {
    // Sampling Instant first conservatively excludes clock-query time from the remote budget.
    let instant = Instant::now();
    let remaining = encoded
        .checked_sub(monotonic_nanos()?)
        .filter(|nanos| *nanos > 0 && u128::from(*nanos) <= MAX_PROOF_TIMEOUT.as_nanos())
        .ok_or_else(|| invalid("expired or excessive compiler-proof execution deadline"))?;
    instant
        .checked_add(Duration::from_nanos(remaining))
        .ok_or_else(|| invalid("compiler-proof local deadline overflow"))
}

fn addresses(fd: BorrowedFd<'_>) -> Result<(net::SocketAddrUnix, net::SocketAddrUnix)> {
    require(
        net::sockopt::socket_type(fd)? == net::SocketType::SEQPACKET
            && net::sockopt::socket_domain(fd)? == net::AddressFamily::UNIX
            && !net::sockopt::socket_acceptconn(fd)?
            && net::sockopt::socket_passcred(fd)?
            && rustix::fs::fcntl_getfl(fd)? == OFlags::RDWR | OFlags::NONBLOCK
            && rustix::io::fcntl_getfd(fd)? == rustix::io::FdFlags::CLOEXEC,
        "invalid compiler-proof endpoint",
    )?;
    let local = net::SocketAddrUnix::try_from(net::getsockname(fd)?)?;
    let peer = net::SocketAddrUnix::try_from(
        net::getpeername(fd)?.ok_or_else(|| invalid("unconnected compiler-proof endpoint"))?,
    )?;
    require(
        local.abstract_name().is_some() && peer.abstract_name().is_some() && local != peer,
        "compiler-proof endpoint addresses",
    )?;
    Ok((local, peer))
}

pub(super) fn pair() -> Result<(OwnedFd, OwnedFd)> {
    let (server, client) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )?;
    for fd in [&server, &client] {
        net::sockopt::set_socket_passcred(fd, true)?;
        net::bind(fd, &net::SocketAddrUnix::new_unnamed())?;
    }
    Ok((server, client))
}

pub(super) struct Endpoint {
    pub(super) fd: OwnedFd,
    object: (u64, u64, u32),
    pub(super) creator: Credentials,
    addresses: (net::SocketAddrUnix, net::SocketAddrUnix),
}

impl Endpoint {
    pub(super) fn admit(fd: OwnedFd) -> Result<Self> {
        let stat = rustix::fs::fstat(&fd)?;
        let peer = net::sockopt::socket_peercred(&fd)?;
        let value = Self {
            object: (stat.st_dev, stat.st_ino, stat.st_mode),
            creator: (
                peer.pid.as_raw_pid() as u32,
                peer.uid.as_raw(),
                peer.gid.as_raw(),
            ),
            addresses: addresses(fd.as_fd())?,
            fd,
        };
        value.revalidate()?;
        Ok(value)
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        let stat = rustix::fs::fstat(&self.fd)?;
        let peer = net::sockopt::socket_peercred(&self.fd)?;
        require(
            (stat.st_dev, stat.st_ino, stat.st_mode) == self.object
                && (
                    peer.pid.as_raw_pid() as u32,
                    peer.uid.as_raw(),
                    peer.gid.as_raw(),
                ) == self.creator
                && addresses(self.fd.as_fd())? == self.addresses,
            "compiler-proof endpoint changed",
        )?;
        self.poll(rustix::event::PollFlags::empty(), Duration::ZERO)
    }

    fn poll(&self, events: rustix::event::PollFlags, duration: Duration) -> Result<()> {
        let timeout = rustix::event::Timespec {
            tv_sec: duration.as_secs().try_into().map_err(io::Error::other)?,
            tv_nsec: duration.subsec_nanos().into(),
        };
        let mut fds = [rustix::event::PollFd::new(&self.fd, events)];
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(_) => require(
                !fds[0].revents().intersects(
                    rustix::event::PollFlags::ERR
                        | rustix::event::PollFlags::HUP
                        | rustix::event::PollFlags::NVAL,
                ),
                "compiler-proof endpoint closed or failed",
            ),
            Err(rustix::io::Errno::INTR) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn wait(&self, events: rustix::event::PollFlags, deadline: Instant) -> Result<()> {
        check_deadline(deadline)?;
        self.poll(
            events,
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(20)),
        )?;
        check_deadline(deadline)
    }

    pub(super) fn send(
        &self,
        frame: &Frame,
        rights: &[BorrowedFd<'_>],
        deadline: Instant,
        validate: impl Fn() -> Result<()>,
    ) -> Result<()> {
        require(
            rights.len() == frame.rights_count(),
            "outgoing compiler-proof rights roster",
        )?;
        let bytes = frame.encode()?;
        loop {
            check_deadline(deadline)?;
            self.revalidate()?;
            validate()?;
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
            let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
            if !rights.is_empty() {
                require(
                    ancillary.push(net::SendAncillaryMessage::ScmRights(rights)),
                    "compiler-proof send rights",
                )?;
            }
            match net::sendmsg(
                &self.fd,
                &[IoSlice::new(&bytes)],
                &mut ancillary,
                net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
            ) {
                Ok(count) if count == bytes.len() => {
                    self.revalidate()?;
                    validate()?;
                    return check_deadline(deadline);
                }
                Ok(_) => return Err(invalid("partial compiler-proof send")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    self.wait(rustix::event::PollFlags::OUT, deadline)?
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn receive(
        &self,
        sender: Credentials,
        deadline: Instant,
        validate: impl Fn() -> Result<()>,
    ) -> Result<(Frame, Vec<OwnedFd>)> {
        loop {
            check_deadline(deadline)?;
            self.revalidate()?;
            validate()?;
            let mut bytes = vec![0; HEADER + MAX_BODY];
            let mut space =
                [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(2))];
            let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
            let received = match net::recvmsg(
                &self.fd,
                &mut [IoSliceMut::new(&mut bytes)],
                &mut ancillary,
                net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(received) => received,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    self.wait(rustix::event::PollFlags::IN, deadline)?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let mut credentials = None;
            let mut rights = Vec::with_capacity(2);
            let mut rights_messages = 0;
            let mut malformed = false;
            for item in ancillary.drain() {
                match item {
                    net::RecvAncillaryMessage::ScmCredentials(value) => {
                        malformed |= credentials.replace(value).is_some();
                    }
                    net::RecvAncillaryMessage::ScmRights(value) => {
                        rights_messages += 1;
                        rights.extend(value);
                    }
                    _ => malformed = true,
                }
            }
            require(
                received.bytes > 0
                    && received.bytes <= bytes.len()
                    && (received.flags - net::ReturnFlags::CMSG_CLOEXEC).is_empty()
                    && !malformed
                    && rights.len() <= 2
                    && rights_messages == usize::from(!rights.is_empty()),
                "malformed compiler-proof frame or ancillary roster",
            )?;
            let c =
                credentials.ok_or_else(|| invalid("missing compiler-proof sender credentials"))?;
            require(
                (c.pid.as_raw_pid() as u32, c.uid.as_raw(), c.gid.as_raw()) == sender,
                "compiler-proof sender mismatch",
            )?;
            for fd in &rights {
                require(
                    rustix::io::fcntl_getfd(fd)? == rustix::io::FdFlags::CLOEXEC,
                    "compiler-proof received rights flags",
                )?;
            }
            let frame = Frame::decode(&bytes[..received.bytes])?;
            require(
                rights.len() == frame.rights_count(),
                "received compiler-proof rights roster",
            )?;
            self.revalidate()?;
            validate()?;
            check_deadline(deadline)?;
            return Ok((frame, rights));
        }
    }
}

const SEALS: rustix::fs::SealFlags = rustix::fs::SealFlags::SEAL
    .union(rustix::fs::SealFlags::WRITE)
    .union(rustix::fs::SealFlags::SHRINK)
    .union(rustix::fs::SealFlags::GROW);

pub(super) fn seal_source(bytes: &[u8]) -> Result<File> {
    require(
        !bytes.is_empty() && bytes.len() <= crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3,
        "compiler-proof source bounds",
    )?;
    let mut file = File::from(rustix::fs::memfd_create(
        c"fe2o3-compiler-proof-source-v1",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?);
    file.write_all(bytes)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o400))?;
    rustix::fs::fcntl_add_seals(&file, SEALS)?;
    Ok(File::from(rustix::fs::open(
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        OFlags::RDONLY | OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?))
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct FileSnapshot([u64; 11]);

impl FileSnapshot {
    pub(super) fn capture(file: &File) -> Result<Self> {
        let m = file.metadata()?;
        Ok(Self([
            m.dev(),
            m.ino(),
            u64::from(m.mode()),
            u64::from(m.uid()),
            u64::from(m.gid()),
            m.nlink(),
            m.len(),
            m.mtime() as u64,
            m.mtime_nsec() as u64,
            m.ctime() as u64,
            m.ctime_nsec() as u64,
        ]))
    }
}

pub(super) fn read_source(
    file: File,
    owner: Credentials,
    length: u64,
) -> Result<crate::CanonicalGeneratedVerusProofInputV3> {
    require(
        length > 0 && length <= crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 as u64,
        "compiler-proof source declared bounds",
    )?;
    let snapshot = FileSnapshot::capture(&file)?;
    let m = file.metadata()?;
    require(
        m.is_file()
            && m.nlink() == 0
            && m.len() == length
            && m.mode() & 0o7777 == 0o400
            && (m.uid(), m.gid()) == (owner.1, owner.2),
        "compiler-proof source metadata",
    )?;
    let flags = rustix::fs::fcntl_getfl(&file)?;
    require(
        flags & OFlags::ACCMODE == OFlags::RDONLY
            && !flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            && rustix::io::fcntl_getfd(&file)? == rustix::io::FdFlags::CLOEXEC
            && rustix::fs::fcntl_get_seals(&file)? == SEALS,
        "compiler-proof source flags or seals",
    )?;
    let mut bytes = vec![0; length as usize];
    file.read_exact_at(&mut bytes, 0)?;
    require(
        file.read_at(&mut [0], length)? == 0
            && FileSnapshot::capture(&file)? == snapshot
            && rustix::fs::fcntl_getfl(&file)? == flags
            && rustix::fs::fcntl_get_seals(&file)? == SEALS,
        "compiler-proof source changed",
    )?;
    crate::CanonicalGeneratedVerusProofInputV3::new(bytes).map_err(io::Error::other)
}

#[cfg(test)]
mod tests;
