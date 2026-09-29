//! Inert compiler child-channel wire mechanics, not child or endpoint admission.
//! The root receiver must join kernel message/peer credentials to the original
//! clone pidfd, admitted profile and parent identity before releasing the gate.

use super::{ProtectedServiceSpawnErrorV2 as Error, Result, io};
use rustix::net::{SocketAddrAny, SocketAddrUnix, SocketType};
use std::os::fd::BorrowedFd;

/// Fixed compiler client descriptor, installed only after the exec gate opens.
pub const COMPILER_SERVICE_FD: i32 = 195;
/// Exact existing FE2CEC2 version-2 payload size, excluding SCM_RIGHTS.
pub const TRANSFER_BYTES: usize = 24;
const MAGIC: [u8; 8] = *b"FE2CEC2\0";
const VERSION: u32 = 2;

/// Untrusted scalar claims decoded from the transfer; never process authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transfer {
    /// Claimed child PID; compare with the retained clone owner and kernel sender.
    pub child_pid: u32,
    /// Claimed direct parent PID; compare with the actual receiving coordinator.
    pub parent_pid: u32,
}

/// Encodes only positive Linux PID values; establishes no identity or authority.
/// Fixed arrays keep this helper allocation-, panic- and destructor-free for the
/// raw child. Caller accounts for the fixed record work and storage.
pub fn encode(child_pid: u32, parent_pid: u32) -> Option<[u8; TRANSFER_BYTES]> {
    if child_pid == 0
        || child_pid > i32::MAX as u32
        || parent_pid == 0
        || parent_pid > i32::MAX as u32
    {
        return None;
    }
    let pid = child_pid.to_le_bytes();
    let parent = parent_pid.to_le_bytes();
    let version = VERSION.to_le_bytes();
    let fd = COMPILER_SERVICE_FD.to_le_bytes();
    Some([
        MAGIC[0], MAGIC[1], MAGIC[2], MAGIC[3], MAGIC[4], MAGIC[5], MAGIC[6], MAGIC[7], version[0],
        version[1], version[2], version[3], pid[0], pid[1], pid[2], pid[3], fd[0], fd[1], fd[2],
        fd[3], parent[0], parent[1], parent[2], parent[3],
    ])
}

/// Checks exact magic, version, fixed destination and positive Linux PID ranges.
/// Length, SCM_RIGHTS, SCM_CREDENTIALS, peer identity and original pidfd custody
/// remain separate receiver obligations. Caller funds this fixed 24-byte scan.
pub fn decode(bytes: &[u8; TRANSFER_BYTES]) -> Option<Transfer> {
    let child_pid = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let parent_pid = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if encode(child_pid, parent_pid)? != *bytes {
        return None;
    }
    Some(Transfer {
        child_pid,
        parent_pid,
    })
}

/// Compares the canonical wire with expected inert PIDs; admits no authority.
pub fn matches(bytes: &[u8; TRANSFER_BYTES], child_pid: u32, parent_pid: u32) -> bool {
    encode(child_pid, parent_pid).as_ref() == Some(bytes)
}

pub use encode as encode_transfer;
pub use matches as transfer_matches;

// Three shape observations, one high-FD duplicate and its eventual/partial close.
pub(crate) const STAGING_WORK: usize = 5 * (1024 + 64) + 256;

pub(crate) fn validate_transfer(fd: BorrowedFd<'_>) -> Result<()> {
    if rustix::net::sockopt::socket_type(fd)
        .map_err(|e| io("inspect compiler channel transfer type", e))?
        != SocketType::SEQPACKET
    {
        return Err(Error::State("compiler channel transfer is not seqpacket"));
    }
    let unnamed = SocketAddrAny::from(SocketAddrUnix::new_unnamed());
    let local = rustix::net::getsockname(fd)
        .map_err(|e| io("inspect compiler channel transfer address", e))?;
    let peer = rustix::net::getpeername(fd)
        .map_err(|e| io("inspect compiler channel transfer peer", e))?;
    if local != unnamed || peer.as_ref() != Some(&unnamed) {
        return Err(Error::State(
            "compiler channel transfer is not an unnamed pair",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_compiler_child_channel_tests.rs"]
mod tests;
