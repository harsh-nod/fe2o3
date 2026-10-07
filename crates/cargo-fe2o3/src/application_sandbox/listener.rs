//! Own all received rights before rejecting their envelope or cardinality.

use super::{File, LISTENER_MESSAGE_MAGIC, MaybeUninit};
use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};
use std::io::IoSliceMut;
use std::os::fd::BorrowedFd;

pub(super) fn receive_listener(socket: BorrowedFd<'_>) -> Result<(File, u32), String> {
    let mut bytes = [0; 16];
    let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
    let message = recvmsg(
        socket,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
    )
    .map_err(|error| format!("failed to receive seccomp listener: {error}"))?;
    let mut descriptor = None;
    let mut rights_messages = 0;
    let mut unexpected = false;
    for item in ancillary.drain() {
        match item {
            RecvAncillaryMessage::ScmRights(rights) => {
                rights_messages += 1;
                for owned in rights {
                    if descriptor.is_none() {
                        descriptor = Some(owned);
                    } else {
                        unexpected = true;
                    }
                }
            }
            _ => unexpected = true,
        }
    }
    if message.bytes != bytes.len() || !(message.flags - ReturnFlags::CMSG_CLOEXEC).is_empty() {
        return Err("seccomp listener transfer was truncated".into());
    }
    let pid = u32::from_ne_bytes(bytes[8..12].try_into().unwrap());
    if bytes[..8] != LISTENER_MESSAGE_MAGIC || pid == 0 || bytes[12..] != [0; 4] {
        return Err("seccomp listener transfer header is invalid".into());
    }
    if unexpected || rights_messages != 1 {
        return Err("seccomp listener transfer descriptor is invalid".into());
    }
    descriptor
        .map(|owned| (File::from(owned), pid))
        .ok_or_else(|| "seccomp listener transfer descriptor is invalid".into())
}

#[cfg(test)]
#[path = "listener/tests.rs"]
mod tests;
