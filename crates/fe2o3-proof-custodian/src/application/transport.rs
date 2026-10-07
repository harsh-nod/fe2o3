use crate::{other, require, wire};
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1, WorkerV3ApplicationProofMessageV1 as Message,
};
use rustix::net;
use std::{
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::fd::{BorrowedFd, OwnedFd},
    time::Instant,
};

pub(crate) fn try_send(
    fd: BorrowedFd<'_>,
    message: &Message,
    rights: &[BorrowedFd<'_>],
) -> io::Result<bool> {
    require(
        rights.len() == message.required_rights(),
        "application outgoing rights roster",
    )?;
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        require(
            ancillary.push(net::SendAncillaryMessage::ScmRights(rights)),
            "application rights buffer",
        )?;
    }
    match net::sendmsg(
        fd,
        &[IoSlice::new(message.canonical_bytes())],
        &mut ancillary,
        net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
    ) {
        Ok(n) => {
            require(
                n == message.canonical_bytes().len(),
                "partial application proof frame",
            )?;
            Ok(true)
        }
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn try_receive(
    fd: BorrowedFd<'_>,
    sender: (i32, u32, u32),
    session: [u8; 32],
) -> io::Result<Option<(Message, Vec<OwnedFd>)>> {
    let mut bytes = [0; WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(2))];
    let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
    let received = match net::recvmsg(
        fd,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
    ) {
        Ok(value) => value,
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    require(
        received.bytes > 0
            && received.bytes <= bytes.len()
            && (received.flags - net::ReturnFlags::CMSG_CLOEXEC).is_empty(),
        "closed or truncated application proof frame",
    )?;
    let mut credentials = None;
    let mut rights = None;
    for item in ancillary.drain() {
        match item {
            net::RecvAncillaryMessage::ScmCredentials(c) if credentials.is_none() => {
                credentials = Some(c)
            }
            net::RecvAncillaryMessage::ScmRights(fds) if rights.is_none() => {
                rights = Some(fds.collect::<Vec<_>>())
            }
            _ => return Err(io::Error::other("extra application ancillary message")),
        }
    }
    let c = credentials.ok_or_else(|| io::Error::other("application omitted credentials"))?;
    require(
        (c.pid.as_raw_pid(), c.uid.as_raw(), c.gid.as_raw()) == sender,
        "application proof sender mismatch",
    )?;
    let message = Message::decode(&bytes[..received.bytes]).map_err(other)?;
    require(
        message.session() == session,
        "application proof session mismatch",
    )?;
    let rights = rights.unwrap_or_default();
    require(
        rights.len() == message.required_rights(),
        "application proof rights roster mismatch",
    )?;
    Ok(Some((message, rights)))
}

pub(crate) fn send(fd: BorrowedFd<'_>, message: &Message, deadline: Instant) -> io::Result<()> {
    loop {
        wire::wait(fd, rustix::event::PollFlags::OUT, deadline)?;
        if try_send(fd, message, &[])? {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests;
