//! Native application proof transport. Each operation makes one syscall attempt.
use crate::{other, require, wire};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime_protocol::{
    NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1, NativeApplicationProofMessageV1 as Message,
    NativeApplicationProofSessionV1 as Session,
};
use rustix::net;
use std::{
    io::{self, IoSliceMut},
    mem::MaybeUninit,
    os::fd::OwnedFd,
};

const WORK: usize = 64 * 1024;
const SCRATCH: usize = 64 * 1024;
pub(crate) struct Received {
    pub(crate) message: Message,
    pub(crate) rights: [Option<OwnedFd>; 2],
}
impl Received {
    pub(crate) fn retained_storage(&self) -> usize {
        size_of::<Self>() - size_of::<Message>()
            + self.message.retained_storage()
            + size_of::<usize>()
    }
}

pub(crate) fn try_send(
    peer: &wire::ControlEndpoint,
    message: &Message,
    budget: &mut Budget<'_>,
) -> io::Result<bool> {
    budget.charge_work(WORK).map_err(other)?;
    require(
        message.required_rights() == 0,
        "native controller cannot send input rights",
    )?;
    require(
        budget.storage() >= size_of::<wire::ControlEndpoint>() + message.retained_storage(),
        "native outgoing proof owners not prepaid",
    )?;
    budget.reserve_storage(SCRATCH).map_err(other)?;
    peer.revalidate()?;
    let result = match net::send(
        peer,
        message.canonical_bytes(),
        net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
    ) {
        Ok(n) => {
            require(
                n == message.canonical_bytes().len(),
                "short native proof send",
            )?;
            true
        }
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => false,
        Err(error) => return Err(error.into()),
    };
    peer.revalidate()?;
    budget.release_storage(SCRATCH).map_err(other)?;
    Ok(result)
}

/// Original endpoint/Session must be paid. Returns the full unreserved output
/// charge; excess/unknown rights and credentials fail closed and are dropped.
pub(crate) fn try_receive(
    peer: &wire::ControlEndpoint,
    sender: (i32, u32, u32),
    session: &Session,
    budget: &mut Budget<'_>,
) -> io::Result<Option<(Received, usize)>> {
    budget.charge_work(WORK).map_err(other)?;
    require(
        budget.storage() >= size_of::<wire::ControlEndpoint>() + session.retained_storage(),
        "native incoming proof owners not prepaid",
    )?;
    let floor = budget.storage();
    budget.reserve_storage(SCRATCH).map_err(other)?;
    peer.revalidate()?;
    let mut bytes = [0; NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(2))];
    let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
    let received = match net::recvmsg(
        peer,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
    ) {
        Ok(value) => value,
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
            budget.release_storage(SCRATCH).map_err(other)?;
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    require(
        received.bytes > 0
            && received.bytes <= bytes.len()
            && (received.flags - net::ReturnFlags::CMSG_CLOEXEC).is_empty(),
        "closed or truncated native proof frame",
    )?;
    let mut credentials = None;
    let mut rights = [None, None];
    let mut right_count = 0;
    let mut saw_rights = false;
    for item in ancillary.drain() {
        match item {
            net::RecvAncillaryMessage::ScmCredentials(c) if credentials.is_none() => {
                credentials = Some(c)
            }
            net::RecvAncillaryMessage::ScmRights(fds) if !saw_rights => {
                saw_rights = true;
                for fd in fds {
                    require(right_count < rights.len(), "excess native input rights")?;
                    require(
                        rustix::io::fcntl_getfd(&fd)? == rustix::io::FdFlags::CLOEXEC,
                        "native input right lacks CLOEXEC",
                    )?;
                    rights[right_count] = Some(fd);
                    right_count += 1;
                }
            }
            _ => return Err(io::Error::other("extra native proof ancillary message")),
        }
    }
    let c = credentials.ok_or_else(|| io::Error::other("native proof omitted sender"))?;
    require(
        (c.pid.as_raw_pid(), c.uid.as_raw(), c.gid.as_raw()) == sender,
        "native proof sender differs",
    )?;
    // Stack framing/ancillary and original Session are covered before native decode.
    let (message, charge) =
        Message::decode(&bytes[..received.bytes], session, budget).map_err(other)?;
    budget
        .reserve_storage(charge.retained_storage())
        .map_err(other)?;
    require(
        right_count == message.required_rights() && saw_rights == (right_count != 0),
        "native proof rights roster differs",
    )?;
    peer.revalidate()?;
    let value = Received { message, rights };
    let retained = value.retained_storage();
    budget
        .release_storage(budget.storage() - floor)
        .map_err(other)?;
    Ok(Some((value, retained)))
}

#[cfg(test)]
mod tests;
