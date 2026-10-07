//! Fixed-attempt Unix transport; prepaid by the enclosing native handoff scope.
use super::{Credentials, Failure, READY_BYTES, Result, Transport};
use rustix::net::{
    self, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrAny, SocketAddrUnix,
    SocketFlags, SocketType, UCred,
};
use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
    time::Instant,
};

pub(super) struct System;
impl super::ReadinessIo for System {
    fn validate(&mut self, control: &OwnedFd, expected: Credentials) -> Result<()> {
        validate(control, expected)
    }
    fn readiness(&mut self, control: &OwnedFd, deadline: Instant) -> Result<[u8; READY_BYTES]> {
        readiness(control, deadline)
    }
    fn eof(&mut self, control: &OwnedFd, deadline: Instant) -> Result<()> {
        eof(control, deadline)
    }
}

pub(super) fn connect(expected: Credentials, deadline: Instant) -> Result<OwnedFd> {
    connect_using(expected, deadline, address()?)
}
pub(super) fn connect_application(expected: Credentials, deadline: Instant) -> Result<OwnedFd> {
    connect_using(expected, deadline, application_address()?)
}
fn connect_using(
    expected: Credentials,
    deadline: Instant,
    address: SocketAddrUnix,
) -> Result<OwnedFd> {
    super::super::require_deadline(deadline)?;
    let control = net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )?;
    match net::connect(&control, &address) {
        Ok(()) => {}
        Err(rustix::io::Errno::INPROGRESS) => {
            wait(&control, libc::POLLOUT, deadline)?;
        }
        Err(error) => return Err(error.into()),
    }
    validate_using(&control, expected, address)?;
    super::super::require_deadline(deadline)?;
    Ok(control)
}
fn address() -> Result<SocketAddrUnix> {
    Ok(SocketAddrUnix::new(
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
    )?)
}
fn application_address() -> Result<SocketAddrUnix> {
    Ok(SocketAddrUnix::new(
        fe2o3_compiler_execution_protocol::NATIVE_APPLICATION_SUPERVISOR_SOCKET_PATH_V3,
    )?)
}
pub(super) fn validate(control: &OwnedFd, expected: Credentials) -> Result<()> {
    validate_using(control, expected, address()?)
}
pub(super) fn validate_application(control: &OwnedFd, expected: Credentials) -> Result<()> {
    validate_using(control, expected, application_address()?)
}
fn validate_using(control: &OwnedFd, expected: Credentials, address: SocketAddrUnix) -> Result<()> {
    Ok(super::super::validate_production_control(
        control,
        expected,
        &SocketAddrAny::from(address),
    )?)
}
pub(super) fn send(
    control: &OwnedFd,
    bytes: &[u8],
    descriptors: &[BorrowedFd<'_>; 2],
    deadline: Instant,
) -> Result<()> {
    wait(control, libc::POLLOUT, deadline)?;
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
        return Err(Failure::Mismatch("handoff descriptor buffer is too small"));
    }
    let sent = net::sendmsg(
        control,
        &[IoSlice::new(bytes)],
        &mut ancillary,
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )?;
    if sent != bytes.len() {
        return Err(Transport::PartialSend.into());
    }
    Ok(super::super::require_deadline(deadline)?)
}

fn wait(control: &OwnedFd, events: i16, deadline: Instant) -> Result<()> {
    super::super::require_deadline(deadline)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let mut descriptor = libc::pollfd {
        fd: control.as_raw_fd(),
        events: events | libc::POLLERR | libc::POLLHUP,
        revents: 0,
    };
    // SAFETY: the owned descriptor and one writable pollfd live through this call.
    let count = unsafe {
        libc::poll(
            &mut descriptor,
            1,
            super::super::duration_to_poll_millis(remaining),
        )
    };
    if count < 0 {
        return Err(rustix::io::Errno::from_raw_os_error(
            std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        )
        .into());
    }
    super::super::require_deadline(deadline)?;
    if count != 1
        || descriptor.revents & (events | libc::POLLHUP) == 0
        || descriptor.revents & (libc::POLLNVAL | libc::POLLERR) != 0
    {
        return Err(Failure::Mismatch("supervisor control wait refused"));
    }
    Ok(())
}

pub(super) fn send_application(
    control: &OwnedFd,
    bytes: &[u8],
    descriptors: &[BorrowedFd<'_>],
    deadline: Instant,
) -> Result<()> {
    if !matches!(descriptors.len(), 0 | 1 | 4) {
        return Err(Failure::Mismatch("native application descriptor count"));
    }
    wait(control, libc::POLLOUT, deadline)?;
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !descriptors.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
        return Err(Failure::Mismatch("native application descriptor buffer"));
    }
    let sent = net::sendmsg(
        control,
        &[IoSlice::new(bytes)],
        &mut ancillary,
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )?;
    if sent != bytes.len() {
        return Err(Transport::PartialSend.into());
    }
    Ok(super::super::require_deadline(deadline)?)
}

fn receive<const N: usize>(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<([u8; N], usize, Option<UCred>)> {
    if !net::sockopt::socket_passcred(control)? {
        return Err(Failure::Mismatch(
            "supervisor control lost per-message credentials",
        ));
    }
    wait(control, libc::POLLIN, deadline)?;
    let mut bytes = [0; N];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let received = net::recvmsg(
        control,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )?;
    let mut credentials = None;
    let mut bad = received
        .flags
        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC);
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmCredentials(value) if credentials.is_none() => {
                credentials = Some(value)
            }
            // Dropping a rights iterator closes every received descriptor.
            _ => bad = true,
        }
    }
    if bad {
        return Err(Transport::MalformedReadiness.into());
    }
    super::super::require_deadline(deadline)?;
    Ok((bytes, received.bytes, credentials))
}
pub(super) fn readiness(control: &OwnedFd, deadline: Instant) -> Result<[u8; READY_BYTES]> {
    exact_record(control, deadline)
}

pub(super) fn exact_record<const N: usize>(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<[u8; N]> {
    let expected = net::sockopt::socket_peercred(control)?;
    let (bytes, count, credentials) = receive::<N>(control, deadline)?;
    if count != bytes.len() || credentials != Some(expected) {
        return Err(Transport::MalformedReadiness.into());
    }
    Ok(bytes)
}
pub(super) fn eof(control: &OwnedFd, deadline: Instant) -> Result<()> {
    // SO_PASSCRED marks even zero-length packets, whereas actual EOF has no cmsg.
    // Without this distinction an empty packet can hide later trailing data.
    let (_, count, credentials) = receive::<1>(control, deadline)?;
    if count != 0 || credentials.is_some() {
        return Err(Transport::TrailingReadiness.into());
    }
    Ok(())
}
