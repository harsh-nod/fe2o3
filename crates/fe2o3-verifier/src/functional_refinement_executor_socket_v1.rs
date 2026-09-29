//! Bounded packet I/O, not endpoint provenance or runtime approval.

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl},
    io::{Errno, FdFlags, fcntl_getfd},
    net::{
        self, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendFlags, SocketAddrAny, SocketAddrUnix, SocketType, UCred, sockopt,
    },
};
use std::{
    io::{self, IoSliceMut},
    mem::MaybeUninit,
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

pub(super) const PACKET_BYTES: usize = 16 * 1024;

pub(super) struct CredentialSocketV1 {
    fd: OwnedFd,
    // Caller-supplied comparison data, never an approved peer identity.
    expected_sender: UCred,
}

// Rustix skips unknown ancillary types. Reserve exactly one aligned credential
// record: with PASSCRED, any additional record necessarily produces CTRUNC.
// No slack is available for unknown descriptor-bearing records such as PIDFD.
#[repr(C)]
struct CredentialControl {
    alignment: [usize; 0],
    bytes: [MaybeUninit<u8>; rustix::cmsg_aligned_space!(ScmCredentials(1))],
}

impl CredentialSocketV1 {
    pub(super) fn new(fd: OwnedFd, expected_sender: UCred) -> io::Result<Self> {
        let flags = fcntl_getfl(&fd)?;
        let unnamed = SocketAddrAny::from(SocketAddrUnix::new_unnamed());
        if sockopt::socket_domain(&fd)? != AddressFamily::UNIX
            || sockopt::socket_type(&fd)? != SocketType::SEQPACKET
            || sockopt::socket_acceptconn(&fd)?
            || fcntl_getfd(&fd)? != FdFlags::CLOEXEC
            || !flags.contains(OFlags::NONBLOCK)
            || flags & OFlags::ACCMODE != OFlags::RDWR
            || net::getsockname(&fd)? != unnamed
            || net::getpeername(&fd)? != Some(unnamed)
            || !sockopt::socket_passcred(&fd)?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "executor transport requires an unnamed connected nonblocking CLOEXEC Unix SEQPACKET with PASSCRED",
            ));
        }
        Ok(Self {
            fd,
            expected_sender,
        })
    }

    pub(super) fn send_packet(&self, bytes: &[u8], deadline: Instant) -> io::Result<()> {
        if bytes.len() > PACKET_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "executor packet exceeds the transport limit",
            ));
        }
        loop {
            remaining(deadline)?;
            match net::send(&self.fd, bytes, SendFlags::DONTWAIT | SendFlags::NOSIGNAL) {
                Ok(sent) => {
                    require_complete_packet(sent, bytes.len())?;
                    remaining(deadline)?;
                    return Ok(());
                }
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => self.wait_for(PollFlags::OUT, deadline)?,
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn receive_packet(
        &self,
        buffer: &mut [u8],
        deadline: Instant,
    ) -> io::Result<Option<usize>> {
        let capacity = buffer.len().min(PACKET_BYTES);
        loop {
            remaining(deadline)?;
            if !sockopt::socket_passcred(&self.fd)? {
                return Err(invalid_packet("executor socket lost PASSCRED"));
            }
            let mut storage = CredentialControl {
                alignment: [],
                bytes: [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmCredentials(1))],
            };
            let mut control = RecvAncillaryBuffer::new(&mut storage.bytes);
            let received = match net::recvmsg(
                &self.fd,
                &mut [IoSliceMut::new(&mut buffer[..capacity])],
                &mut control,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(received) => received,
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => {
                    self.wait_for(PollFlags::IN, deadline)?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let mut credentials = None;
            let mut unexpected = received
                .flags
                .intersects(ReturnFlags::CTRUNC | ReturnFlags::TRUNC);
            for message in control.drain() {
                match message {
                    RecvAncillaryMessage::ScmCredentials(value) if credentials.is_none() => {
                        credentials = Some(value);
                    }
                    // Dropping a rights iterator closes every installed descriptor.
                    // Linux also closes rights omitted by control-buffer truncation.
                    _ => unexpected = true,
                }
            }
            remaining(deadline)?;
            if unexpected || received.bytes > capacity {
                return Err(invalid_packet(
                    "truncated or unexpected executor packet control",
                ));
            }
            return match credentials {
                Some(actual) if actual == self.expected_sender => Ok(Some(received.bytes)),
                Some(_) => Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "executor packet sender differs from the supplied credentials",
                )),
                // PASSCRED supplies credentials even for an empty packet. Only
                // an unadorned zero-byte read is EOF, not a protocol terminator.
                None if received.bytes == 0 => Ok(None),
                None => Err(invalid_packet("executor packet has no sender credentials")),
            };
        }
    }

    fn wait_for(&self, events: PollFlags, deadline: Instant) -> io::Result<()> {
        loop {
            // Cap one poll interval only; the original absolute deadline is
            // checked again after timeout, signal interruption and readiness.
            let timeout =
                Timespec::try_from(remaining(deadline)?.min(Duration::from_secs(i32::MAX as u64)))
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid poll timeout")
                    })?;
            let mut descriptors = [PollFd::new(&self.fd, events)];
            let result = poll(&mut descriptors, Some(&timeout));
            remaining(deadline)?;
            match result {
                Err(Errno::INTR) | Ok(0) => continue,
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    let returned = descriptors[0].revents();
                    if returned.intersects(PollFlags::NVAL | PollFlags::ERR) {
                        return Err(io::Error::other("executor socket poll failed"));
                    }
                    if returned.intersects(events | PollFlags::HUP) {
                        return Ok(());
                    }
                    return Err(io::Error::other("unexpected executor socket readiness"));
                }
            }
        }
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    match deadline.checked_duration_since(Instant::now()) {
        Some(duration) if !duration.is_zero() => Ok(duration),
        _ => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "executor packet deadline expired",
        )),
    }
}

fn require_complete_packet(sent: usize, expected: usize) -> io::Result<()> {
    if sent == expected {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "executor packet was not sent atomically",
        ))
    }
}

fn invalid_packet(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "functional_refinement_executor_socket_v1_tests.rs"]
mod tests;
