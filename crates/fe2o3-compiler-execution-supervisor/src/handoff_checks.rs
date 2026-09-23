//! Policy-neutral socket/descriptor predicates with fixed, allocation-free failures.
use fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1 as Client;
use rustix::net::{AddressFamily, SocketAddrUnix, SocketType};
use std::os::fd::AsFd;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Snapshot(pub(super) u64, pub(super) u64, pub(super) u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    InvalidControl(&'static str),
    SubmitterCredentialsMismatch,
    InvalidServicePeer,
    ServicePeerCredentialsMismatch,
    DescriptorChanged,
    DescriptorAlias,
    Io(rustix::io::Errno),
}
impl From<rustix::io::Errno> for Failure {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Io(error)
    }
}
type Result<T> = std::result::Result<T, Failure>;

pub(super) fn control_shape(control: &impl AsFd) -> Result<()> {
    if !has_cloexec(control)? {
        return Err(Failure::InvalidControl("control descriptor is inheritable"));
    }
    if rustix::net::sockopt::socket_domain(control)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(control)? != SocketType::SEQPACKET
    {
        return Err(Failure::InvalidControl(
            "endpoint is not Unix SOCK_SEQPACKET",
        ));
    }
    let local = rustix::net::getsockname(control)?;
    let remote = rustix::net::getpeername(control)?;
    if local.address_family() != AddressFamily::UNIX
        || remote
            .as_ref()
            .is_none_or(|address| address.address_family() != AddressFamily::UNIX)
    {
        return Err(Failure::InvalidControl(
            "endpoint is not connected within AF_UNIX",
        ));
    }
    Ok(())
}

pub(super) fn control_peer(control: &impl AsFd) -> Result<Client> {
    let credentials = rustix::net::sockopt::socket_peercred(control)?;
    let pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
        .map_err(|_| Failure::SubmitterCredentialsMismatch)?;
    Client::new(pid, credentials.uid.as_raw(), credentials.gid.as_raw())
        .map_err(|_| Failure::SubmitterCredentialsMismatch)
}

pub(super) fn service_peer(peer: &impl AsFd, expected: Client) -> Result<()> {
    if !has_cloexec(peer)? {
        return Err(Failure::InvalidServicePeer);
    }
    let before = snapshot(peer)?;
    if before.2 & libc::S_IFMT != libc::S_IFSOCK
        || rustix::net::sockopt::socket_domain(peer)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(peer)? != SocketType::SEQPACKET
    {
        return Err(Failure::InvalidServicePeer);
    }
    let local = rustix::net::getsockname(peer)?;
    let remote = rustix::net::getpeername(peer)?;
    let unnamed = rustix::net::SocketAddrAny::from(SocketAddrUnix::new_unnamed());
    if local != unnamed || remote.as_ref() != Some(&unnamed) {
        return Err(Failure::InvalidServicePeer);
    }
    let credentials = rustix::net::sockopt::socket_peercred(peer)?;
    let pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
        .map_err(|_| Failure::ServicePeerCredentialsMismatch)?;
    if (pid, credentials.uid.as_raw(), credentials.gid.as_raw())
        != (expected.pid(), expected.uid(), expected.gid())
    {
        return Err(Failure::ServicePeerCredentialsMismatch);
    }
    if snapshot(peer)? != before {
        return Err(Failure::DescriptorChanged);
    }
    Ok(())
}

fn has_cloexec(descriptor: &impl AsFd) -> Result<bool> {
    Ok(rustix::io::fcntl_getfd(descriptor)?.contains(rustix::io::FdFlags::CLOEXEC))
}

pub(super) fn snapshot(descriptor: &impl AsFd) -> Result<Snapshot> {
    let stat = rustix::fs::fstat(descriptor)?;
    Ok(Snapshot(stat.st_dev, stat.st_ino, stat.st_mode))
}

pub(super) fn distinct(control: Snapshot, service: Snapshot, pidfd: Snapshot) -> Result<()> {
    let control = (control.0, control.1);
    let service = (service.0, service.1);
    let pidfd = (pidfd.0, pidfd.1);
    if control == service || control == pidfd || service == pidfd {
        return Err(Failure::DescriptorAlias);
    }
    Ok(())
}
