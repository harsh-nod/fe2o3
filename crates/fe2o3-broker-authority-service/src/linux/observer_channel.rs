//! Private root-observer protocol. Returned bytes never construct signing authority alone.

use super::*;
use std::io::{IoSlice, IoSliceMut};
use std::os::fd::BorrowedFd;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    CompilerModuleHandoffLockRetentionV3, INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1,
    InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionServiceLaunchManifestV1,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1, ProtectedServiceNamespaceSetV1,
    validate_current_protected_service_profile_v1, validate_protected_service_process_v1,
};
use fe2o3_protected_service_spawn::require_exact_root_identity_v1;
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, SocketFlags, recvmsg, sendmsg, socketpair,
};

use crate::{ProtectedCompilerExecutionOccurrenceErrorV1, RetainedCompilerExecutionOccurrenceV1};

mod application;
mod issuer;
pub(crate) mod registry;
mod root;
pub use application::{
    PendingApplicationObservationGateV1, PublishedApplicationCustodianHandoffV1,
};
pub use issuer::ProtectedCompilerExecutionObserverV1;
pub(crate) use issuer::RemoteCompilerExecutionOccurrenceGuardV1;
#[cfg(test)]
pub(crate) use issuer::tests;
pub use registry::{
    ObservedApplicationRegistrationV1, ObservedCustodianApplicationRegistrationV1,
    PreparedRootCompilerObserverRegistryV1, RegisteredApplicationObserverV1,
    RegisteredCompilerObserverV1, RegisteredCustodianApplicationObserverV1,
    RootCompilerObserverRegistryV1, SupervisorCompilerObserverRegistryV1,
};
pub use root::{
    PreparedRootCompilerExecutionObserverV1, RootCompilerExecutionObserverProgressV1,
    RootCompilerExecutionObserverV1,
};

const TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_REQUESTS: u64 = 1024;
const HEADER: usize = 128;
const HELLO_BODY: usize = COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1 + 48;
const OCCURRENCE_BODY: usize = INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 + 32;
const MAX_PACKET: usize = HEADER
    + if HELLO_BODY > OCCURRENCE_BODY {
        HELLO_BODY
    } else {
        OCCURRENCE_BODY
    };
const MAGIC: &[u8; 8] = b"F2O3OBS1";
// Registry and application messages share bounded transport, not the compiler packet codec.
const MAX_TRANSPORT_PACKET: usize =
    fe2o3_runtime_protocol::WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1;

/// Fail-closed private observer admission or protocol failure.
#[derive(Debug)]
pub enum CompilerExecutionObserverErrorV1 {
    /// A retained process or descriptor failed its original admission checks.
    Admission(ProtectedServiceAdmissionErrorV1),
    /// Live compiler/publication observation failed.
    Occurrence(Box<ProtectedCompilerExecutionOccurrenceErrorV1>),
    /// Independent pre-ACK application observation failed.
    ApplicationObservation(Box<crate::WorkerV3ApplicationObservationErrorV1>),
    /// The peer, message, or operation is outside the exact private protocol.
    Protocol(&'static str),
    /// The admitted process profile changed or was not established.
    Profile(String),
    /// A bounded transport operation failed.
    Io(io::Error),
    /// The absolute transport or operation deadline elapsed.
    Timeout,
    /// This channel has already failed and cannot authorize another operation.
    Poisoned,
    /// The peer closed its channel; this carries no packet or authority.
    Closed,
}

impl fmt::Display for CompilerExecutionObserverErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(e) => write!(f, "observer admission: {e}"),
            Self::Occurrence(e) => write!(f, "observer occurrence: {e}"),
            Self::ApplicationObservation(e) => write!(f, "observer application: {e}"),
            Self::Protocol(e) => write!(f, "observer protocol: {e}"),
            Self::Profile(e) => write!(f, "observer process profile: {e}"),
            Self::Io(e) => write!(f, "observer transport: {e}"),
            Self::Timeout => f.write_str("observer deadline elapsed"),
            Self::Poisoned => f.write_str("observer channel is terminal"),
            Self::Closed => f.write_str("observer channel closed"),
        }
    }
}

impl Error for CompilerExecutionObserverErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Occurrence(e) => Some(e),
            Self::ApplicationObservation(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ProtectedServiceAdmissionErrorV1> for CompilerExecutionObserverErrorV1 {
    fn from(e: ProtectedServiceAdmissionErrorV1) -> Self {
        Self::Admission(e)
    }
}
impl From<ProtectedCompilerExecutionOccurrenceErrorV1> for CompilerExecutionObserverErrorV1 {
    fn from(e: ProtectedCompilerExecutionOccurrenceErrorV1) -> Self {
        Self::Occurrence(Box::new(e))
    }
}
impl From<io::Error> for CompilerExecutionObserverErrorV1 {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<rustix::io::Errno> for CompilerExecutionObserverErrorV1 {
    fn from(e: rustix::io::Errno) -> Self {
        Self::Io(e.into())
    }
}
type Result<T> = std::result::Result<T, CompilerExecutionObserverErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum Kind {
    Hello = 1,
    Begin,
    Revalidate,
    Finish,
    Begun,
    Revalidated,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Packet {
    kind: Kind,
    session: [u8; 32],
    launch: [u8; 32],
    sequence: u64,
    operation: u64,
    nonce: [u8; 32],
    body: Vec<u8>,
}

impl Packet {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = vec![0; HEADER + self.body.len()];
        if bytes.len() > MAX_PACKET {
            return Err(invalid("oversized packet"));
        }
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8] = self.kind as u8;
        bytes[16..48].copy_from_slice(&self.session);
        bytes[48..80].copy_from_slice(&self.launch);
        bytes[80..88].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[88..96].copy_from_slice(&self.operation.to_le_bytes());
        bytes[96..128].copy_from_slice(&self.nonce);
        bytes[HEADER..].copy_from_slice(&self.body);
        Self::decode(&bytes)?;
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER
            || bytes.len() > MAX_PACKET
            || &bytes[..8] != MAGIC
            || bytes[9..16] != [0; 7]
        {
            return Err(invalid("noncanonical packet header"));
        }
        let kind = match bytes[8] {
            1 => Kind::Hello,
            2 => Kind::Begin,
            3 => Kind::Revalidate,
            4 => Kind::Finish,
            5 => Kind::Begun,
            6 => Kind::Revalidated,
            7 => Kind::Finished,
            _ => return Err(invalid("unknown packet kind")),
        };
        let body_len = match kind {
            Kind::Hello => HELLO_BODY,
            Kind::Begun => OCCURRENCE_BODY,
            _ => 0,
        };
        if bytes.len() != HEADER + body_len {
            return Err(invalid("noncanonical packet length"));
        }
        let packet = Self {
            kind,
            session: bytes[16..48].try_into().unwrap(),
            launch: bytes[48..80].try_into().unwrap(),
            sequence: u64::from_le_bytes(bytes[80..88].try_into().unwrap()),
            operation: u64::from_le_bytes(bytes[88..96].try_into().unwrap()),
            nonce: bytes[96..128].try_into().unwrap(),
            body: bytes[HEADER..].to_vec(),
        };
        if packet.session == [0; 32] || packet.launch == [0; 32] {
            return Err(invalid("empty session or launch binding"));
        }
        if kind == Kind::Hello {
            if packet.sequence != 0 || packet.operation != 0 || packet.nonce != [0; 32] {
                return Err(invalid("noncanonical hello identity"));
            }
        } else if packet.sequence == 0
            || packet.sequence > MAX_REQUESTS
            || packet.operation == 0
            || packet.operation > packet.sequence
            || packet.nonce == [0; 32]
        {
            return Err(invalid("invalid operation identity"));
        }
        Ok(packet)
    }

    fn response(&self, kind: Kind, body: Vec<u8>) -> Self {
        Self {
            kind,
            body,
            ..self.clone()
        }
    }

    fn matches_response(&self, response: &Self, kind: Kind) -> bool {
        response.kind == kind
            && response.session == self.session
            && response.launch == self.launch
            && response.sequence == self.sequence
            && response.operation == self.operation
            && response.nonce == self.nonce
    }
}

struct Endpoint {
    peer: OwnedFd,
    identity: ObjectIdentityV1,
    creator: PeerCredentialsV1,
    addresses: (rustix::net::SocketAddrUnix, rustix::net::SocketAddrUnix),
}

impl Endpoint {
    fn admit(peer: OwnedFd) -> Result<Self> {
        let endpoint = Self {
            identity: ObjectIdentityV1::inspect(
                &peer,
                AdmissionErrorKindV1::InspectPeer,
                "observer peer",
            )?,
            creator: PeerCredentialsV1::inspect(&peer)?,
            addresses: observer_addresses(&peer)?,
            peer,
        };
        endpoint.revalidate()?;
        Ok(endpoint)
    }

    fn revalidate(&self) -> Result<()> {
        if ObjectIdentityV1::inspect(
            &self.peer,
            AdmissionErrorKindV1::InspectPeer,
            "observer peer",
        )? != self.identity
            || observer_addresses(&self.peer)? != self.addresses
            || PeerCredentialsV1::inspect(&self.peer)? != self.creator
            || rustix::io::fcntl_getfd(&self.peer)? != rustix::io::FdFlags::CLOEXEC
            || !rustix::net::sockopt::socket_passcred(&self.peer)?
        {
            return Err(invalid("private endpoint continuity changed"));
        }
        let flags = rustix::fs::fcntl_getfl(&self.peer)?;
        if flags & OFlags::ACCMODE != OFlags::RDWR
            || !flags.contains(OFlags::NONBLOCK)
            || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
        {
            return Err(invalid("private endpoint status changed"));
        }
        Ok(())
    }

    fn receive(
        &self,
        sender: &LiveClientPidfdIdentityV1,
    ) -> Result<Option<(Packet, Vec<OwnedFd>)>> {
        let Some((bytes, rights)) = self.receive_bytes(sender)? else {
            return Ok(None);
        };
        let packet = Packet::decode(&bytes)?;
        if rights.len() != if packet.kind == Kind::Begun { 2 } else { 0 } {
            return Err(invalid("unexpected packet descriptors"));
        }
        Ok(Some((packet, rights)))
    }

    fn receive_bytes(
        &self,
        sender: &LiveClientPidfdIdentityV1,
    ) -> Result<Option<(Vec<u8>, Vec<OwnedFd>)>> {
        self.revalidate()?;
        sender.validate_liveness()?;
        let mut bytes = [0; MAX_TRANSPORT_PACKET];
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(2))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = match recvmsg(
            &self.peer,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(received) => received,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut credentials = None;
        let mut rights = Vec::with_capacity(2);
        let mut rights_messages = 0;
        let mut malformed = false;
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmCredentials(value) => {
                    if credentials.replace(value).is_some() {
                        malformed = true;
                    }
                }
                RecvAncillaryMessage::ScmRights(value) => {
                    rights_messages += 1;
                    rights.extend(value);
                }
                _ => malformed = true,
            }
        }
        if received.bytes == 0
            && (received.flags - ReturnFlags::CMSG_CLOEXEC).is_empty()
            && credentials.is_none()
            && rights.is_empty()
            && !malformed
            && self.closed()?
        {
            return Err(CompilerExecutionObserverErrorV1::Closed);
        }
        let expected = sender.expected_client;
        if malformed
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || !credentials.is_some_and(|c| {
                c.pid.as_raw_pid() as u32 == expected.pid
                    && c.uid.as_raw() == expected.uid
                    && c.gid.as_raw() == expected.gid
            })
        {
            return Err(invalid("packet sender or ancillary roster mismatch"));
        }
        if rights.len() > 2 || rights_messages != usize::from(!rights.is_empty()) {
            return Err(invalid("unexpected packet descriptors"));
        }
        sender.validate_liveness()?;
        self.revalidate()?;
        Ok(Some((bytes[..received.bytes].to_vec(), rights)))
    }

    fn send(&self, packet: &Packet, rights: &[BorrowedFd<'_>]) -> Result<bool> {
        self.revalidate()?;
        if rights.len() != if packet.kind == Kind::Begun { 2 } else { 0 } {
            return Err(invalid("outgoing descriptor roster mismatch"));
        }
        let bytes = packet.encode()?;
        self.send_bytes(&bytes, rights)
    }

    fn send_bytes(&self, bytes: &[u8], rights: &[BorrowedFd<'_>]) -> Result<bool> {
        self.revalidate()?;
        if bytes.is_empty() || bytes.len() > MAX_TRANSPORT_PACKET || rights.len() > 2 {
            return Err(invalid(
                "outgoing datagram exceeds private transport bounds",
            ));
        }
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !rights.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(rights)) {
            return Err(invalid("outgoing ancillary capacity"));
        }
        match sendmsg(
            &self.peer,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(length) if length == bytes.len() => Ok(true),
            Ok(_) => Err(invalid("partial seqpacket send")),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn close(&self) {
        let _ = rustix::net::shutdown(&self.peer, rustix::net::Shutdown::Both);
    }

    fn closed(&self) -> Result<bool> {
        let mut event = libc::pollfd {
            fd: self.peer.as_raw_fd(),
            events: libc::POLLRDHUP,
            revents: 0,
        };
        // SAFETY: poll borrows one initialized descriptor record for this call.
        if unsafe { libc::poll(&mut event, 1, 0) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if event.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(invalid("observer socket poll failed"));
        }
        Ok(event.revents & (libc::POLLRDHUP | libc::POLLHUP) != 0)
    }
}

fn observer_addresses(
    peer: &OwnedFd,
) -> Result<(rustix::net::SocketAddrUnix, rustix::net::SocketAddrUnix)> {
    if rustix::net::sockopt::socket_domain(peer)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(peer)? != SocketType::SEQPACKET
        || rustix::net::sockopt::socket_acceptconn(peer)?
    {
        return Err(invalid(
            "observer endpoint is not a connected Unix seqpacket",
        ));
    }
    let local = rustix::net::SocketAddrUnix::try_from(rustix::net::getsockname(peer)?)?;
    let remote = rustix::net::SocketAddrUnix::try_from(
        rustix::net::getpeername(peer)?
            .ok_or_else(|| invalid("observer endpoint is disconnected"))?,
    )?;
    if local.abstract_name().is_none() || remote.abstract_name().is_none() || local == remote {
        return Err(invalid(
            "observer endpoints lack distinct retained abstract addresses",
        ));
    }
    Ok((local, remote))
}

fn observer_pair() -> Result<(OwnedFd, OwnedFd)> {
    let pair = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )?;
    for peer in [&pair.0, &pair.1] {
        rustix::net::sockopt::set_socket_passcred(peer, true)?;
        // Linux SO_PASSCRED autobinds on first send. Bind now, before either endpoint is
        // exposed, so the exact kernel-selected addresses can be retained immutably.
        rustix::net::bind(peer, &rustix::net::SocketAddrUnix::new_unnamed())?;
    }
    Ok(pair)
}

fn invalid(reason: &'static str) -> CompilerExecutionObserverErrorV1 {
    CompilerExecutionObserverErrorV1::Protocol(reason)
}

fn nonce() -> Result<[u8; 32]> {
    let mut nonce = [0; 32];
    let mut filled = 0;
    while filled < nonce.len() {
        match rustix::rand::getrandom(&mut nonce[filled..], rustix::rand::GetRandomFlags::empty()) {
            Ok(0) => return Err(invalid("empty entropy read")),
            Ok(count) => filled += count,
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    if nonce == [0; 32] {
        return Err(invalid("zero random nonce"));
    }
    Ok(nonce)
}

fn require_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(CompilerExecutionObserverErrorV1::Timeout)
    } else {
        Ok(())
    }
}

fn pidfd_exited(pidfd: &OwnedFd) -> Result<bool> {
    let mut event = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one live, exclusively borrowed pollfd is passed for a nonblocking observation.
    let result = unsafe { libc::poll(&mut event, 1, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if event.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
        return Err(invalid("invalid retained pidfd"));
    }
    Ok(event.revents & (libc::POLLIN | libc::POLLHUP) != 0)
}

fn current_identity() -> Result<LiveClientPidfdIdentityV1> {
    let expected = ExpectedClientProcessIdentityV1::new(
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )?;
    let pidfd = rustix::process::pidfd_open(
        rustix::process::getpid(),
        rustix::process::PidfdFlags::empty(),
    )?;
    Ok(LiveClientPidfdIdentityV1::admit(pidfd, expected)?)
}

fn encode_identity(identity: &LiveClientPidfdIdentityV1, body: &mut Vec<u8>) {
    for value in [
        identity.expected_client.pid,
        identity.expected_client.uid,
        identity.expected_client.gid,
    ] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(&identity.start_time_ticks.to_le_bytes());
}

fn decode_identity(bytes: &[u8]) -> Result<(ExpectedClientProcessIdentityV1, u64)> {
    if bytes.len() != 20 {
        return Err(invalid("process identity length"));
    }
    Ok((
        ExpectedClientProcessIdentityV1::new(
            u32::from_le_bytes(bytes[..4].try_into().unwrap()),
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        )?,
        u64::from_le_bytes(bytes[12..].try_into().unwrap()),
    ))
}
