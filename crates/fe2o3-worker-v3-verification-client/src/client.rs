use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use fe2o3_worker_v3_verification_protocol::{
    WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1, WorkerV3VerificationRequestV1,
    WorkerV3VerificationResponseV1,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, Shutdown, SocketType,
};

use crate::{WorkerV3VerificationClientErrorV1, WorkerV3VerificationPayloadSnapshotsV1};

const LINUX_SA_FAMILY_BYTES: u32 = 2;

/// One owned, one-shot connection to a Worker V3 verification framing service.
///
/// Admission checks transport shape and descriptor lifetime only. It does not authenticate the
/// service peer. `exchange` consumes this value and half-closes the request direction after the
/// sole canonical request packet.
pub struct WorkerV3VerificationClientV1 {
    peer: OwnedFd,
    deadline: Instant,
}

impl std::fmt::Debug for WorkerV3VerificationClientV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationClientV1")
            .field("deadline", &self.deadline)
            .field("peer_authority", &"none")
            .finish_non_exhaustive()
    }
}

impl WorkerV3VerificationClientV1 {
    /// Admits one connected unnamed Unix `SOCK_SEQPACKET` peer under one absolute deadline.
    pub fn admit(
        peer: OwnedFd,
        timeout: Duration,
    ) -> Result<Self, WorkerV3VerificationClientErrorV1> {
        if timeout.is_zero() {
            return Err(WorkerV3VerificationClientErrorV1::InvalidTimeout);
        }
        set_close_on_exec(&peer)?;
        validate_peer(&peer)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(WorkerV3VerificationClientErrorV1::DeadlineOverflow)?;
        Ok(Self { peer, deadline })
    }

    /// Sends exactly one request with exactly two ordered payload descriptors and receives one
    /// exact framing-only response.
    pub fn exchange(
        self,
        request: WorkerV3VerificationRequestV1,
        snapshots: WorkerV3VerificationPayloadSnapshotsV1,
    ) -> Result<WorkerV3VerificationFramingReceiptV1, WorkerV3VerificationClientErrorV1> {
        snapshots.revalidate(&request)?;
        send_request(
            &self.peer,
            request.encode_canonical(),
            snapshots.borrowed_fds(),
            self.deadline,
        )?;
        rustix::net::shutdown(&self.peer, Shutdown::Write)
            .map_err(|source| WorkerV3VerificationClientErrorV1::Shutdown(source.into()))?;
        snapshots.revalidate(&request)?;
        let response_bytes = receive_response(&self.peer, self.deadline)?;
        let response = WorkerV3VerificationResponseV1::decode_canonical(&response_bytes)?;
        if !response.matches_request(&request) {
            return Err(WorkerV3VerificationClientErrorV1::ResponseRequestMismatch);
        }
        Ok(WorkerV3VerificationFramingReceiptV1 { response })
    }

    /// Reports that socket-shape admission does not authenticate the peer.
    pub const fn authenticates_peer(&self) -> bool {
        false
    }
}

/// Authority-free receipt for one correlated framing response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationFramingReceiptV1 {
    response: WorkerV3VerificationResponseV1,
}

impl WorkerV3VerificationFramingReceiptV1 {
    /// Returns the exact correlated framing response.
    pub const fn response(&self) -> &WorkerV3VerificationResponseV1 {
        &self.response
    }

    /// Reports that the transport did not authenticate the service peer.
    pub const fn authenticates_peer(&self) -> bool {
        false
    }

    /// Reports that framing cannot establish a compiler or machine theorem.
    pub const fn grants_theorem_authority(&self) -> bool {
        false
    }

    /// Reports that framing cannot authorize native code loading.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that framing cannot authorize GPU launch.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_peer(peer: &OwnedFd) -> Result<(), WorkerV3VerificationClientErrorV1> {
    let socket_type = rustix::net::sockopt::socket_type(peer)
        .map_err(|source| descriptor_error("inspect peer socket type", source.into()))?;
    if socket_type != SocketType::SEQPACKET {
        return Err(WorkerV3VerificationClientErrorV1::NotSeqpacket);
    }
    let domain = rustix::net::sockopt::socket_domain(peer)
        .map_err(|source| descriptor_error("inspect peer socket domain", source.into()))?;
    if domain != AddressFamily::UNIX {
        return Err(WorkerV3VerificationClientErrorV1::NamedOrNonUnixPeer);
    }
    let local = rustix::net::getsockname(peer)
        .map_err(|source| descriptor_error("inspect local peer address", source.into()))?;
    let remote = rustix::net::getpeername(peer)
        .map_err(|source| descriptor_error("inspect remote peer address", source.into()))?
        .ok_or(WorkerV3VerificationClientErrorV1::NamedOrNonUnixPeer)?;
    if local.address_family() != AddressFamily::UNIX
        || remote.address_family() != AddressFamily::UNIX
        || local.addr_len() != LINUX_SA_FAMILY_BYTES
        || remote.addr_len() != LINUX_SA_FAMILY_BYTES
    {
        return Err(WorkerV3VerificationClientErrorV1::NamedOrNonUnixPeer);
    }
    Ok(())
}

fn set_close_on_exec(peer: &OwnedFd) -> Result<(), WorkerV3VerificationClientErrorV1> {
    rustix::io::fcntl_setfd(peer, rustix::io::FdFlags::CLOEXEC)
        .map_err(|source| descriptor_error("set peer close-on-exec", source.into()))?;
    let actual = rustix::io::fcntl_getfd(peer)
        .map_err(|source| descriptor_error("inspect peer descriptor flags", source.into()))?;
    if actual != rustix::io::FdFlags::CLOEXEC {
        return Err(descriptor_error(
            "retain exact peer close-on-exec flags",
            std::io::Error::other(format!(
                "unexpected descriptor flags 0x{:08x}",
                actual.bits()
            )),
        ));
    }
    Ok(())
}

fn send_request(
    peer: &OwnedFd,
    bytes: &[u8],
    descriptors: [std::os::fd::BorrowedFd<'_>; 2],
    deadline: Instant,
) -> Result<(), WorkerV3VerificationClientErrorV1> {
    loop {
        wait_for_peer(peer, PollFlags::OUT, deadline)?;
        let mut control_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut control = SendAncillaryBuffer::new(&mut control_space);
        if !control.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(WorkerV3VerificationClientErrorV1::PartialSend {
                expected: bytes.len(),
                actual: 0,
            });
        }
        match rustix::net::sendmsg(
            peer,
            &[IoSlice::new(bytes)],
            &mut control,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(actual) if actual == bytes.len() => return Ok(()),
            Ok(actual) => {
                return Err(WorkerV3VerificationClientErrorV1::PartialSend {
                    expected: bytes.len(),
                    actual,
                });
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
            Err(source) => {
                return Err(WorkerV3VerificationClientErrorV1::Send(source.into()));
            }
        }
    }
}

fn receive_response(
    peer: &OwnedFd,
    deadline: Instant,
) -> Result<[u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1], WorkerV3VerificationClientErrorV1> {
    let mut bytes = [0_u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 + 1];
    loop {
        wait_for_peer(peer, PollFlags::IN, deadline)?;
        let mut control_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
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
                    return Err(WorkerV3VerificationClientErrorV1::Receive(source.into()));
                }
            }
        };
        if received.flags.contains(ReturnFlags::CTRUNC) || control.drain().next().is_some() {
            return Err(WorkerV3VerificationClientErrorV1::ResponseAncillaryData);
        }
        if received.flags.contains(ReturnFlags::TRUNC)
            || received.bytes > WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1
        {
            return Err(WorkerV3VerificationClientErrorV1::ResponseOversize {
                maximum: WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1,
                actual: received.bytes,
            });
        }
        if received.bytes == 0 {
            return Err(WorkerV3VerificationClientErrorV1::PeerClosed);
        }
        if received.bytes < WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 {
            return Err(WorkerV3VerificationClientErrorV1::ResponseTruncated {
                expected: WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1,
                actual: received.bytes,
            });
        }
        return Ok(bytes[..WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1]
            .try_into()
            .expect("exact response length checked"));
    }
}

fn wait_for_peer(
    peer: &OwnedFd,
    wanted: PollFlags,
    deadline: Instant,
) -> Result<(), WorkerV3VerificationClientErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(WorkerV3VerificationClientErrorV1::Timeout);
        }
        let timeout = Timespec::try_from(remaining)
            .map_err(|_| WorkerV3VerificationClientErrorV1::DeadlineOverflow)?;
        let mut descriptors = [PollFd::new(peer, wanted | PollFlags::ERR | PollFlags::HUP)];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) => return Err(WorkerV3VerificationClientErrorV1::Timeout),
            Ok(_) => {
                let ready = descriptors[0].revents();
                if ready.contains(PollFlags::NVAL) {
                    return Err(WorkerV3VerificationClientErrorV1::InvalidPeer);
                }
                if ready.contains(wanted) {
                    return Ok(());
                }
                if ready.contains(PollFlags::ERR) {
                    return Err(WorkerV3VerificationClientErrorV1::PeerFailed);
                }
                if ready.contains(PollFlags::HUP) {
                    return Err(WorkerV3VerificationClientErrorV1::PeerClosed);
                }
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => return Err(WorkerV3VerificationClientErrorV1::Poll(source.into())),
        }
    }
}

fn descriptor_error(
    operation: &'static str,
    source: std::io::Error,
) -> WorkerV3VerificationClientErrorV1 {
    WorkerV3VerificationClientErrorV1::Descriptor { operation, source }
}
