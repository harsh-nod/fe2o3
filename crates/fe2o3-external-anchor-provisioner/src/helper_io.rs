//! Shared fixed-attempt helper descriptor, bootstrap and terminal exec mechanics.
use crate::{
    ExternalAnchorProvisioningReadyV1, entrypoint::ExternalAnchorProvisioningHelperErrorV1,
};
use core::ffi::{CStr, c_char, c_void};
use rustix::{
    fs::OFlags,
    net::{
        AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrAny,
        SocketAddrUnix, SocketType, sendmsg,
    },
};
use std::{
    io::{self, IoSlice},
    mem::MaybeUninit,
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd},
};

pub(crate) const STAGED_DESCRIPTOR_FLOOR_V1: RawFd = 300;
const CLOSE_RANGE_CLOEXEC_V1: u32 = 1 << 2;
const EXEC_FAILURE_STAGE_BASE_V1: u8 = 0xe0;

pub(crate) fn take_fixed(
    descriptor: RawFd,
    role: &'static str,
) -> Result<OwnedFd, ExternalAnchorProvisioningHelperErrorV1> {
    let retained = duplicate_fixed(descriptor, role)?;
    close_fixed(descriptor)?;
    Ok(retained)
}

pub(crate) fn duplicate_fixed(
    descriptor: RawFd,
    role: &'static str,
) -> Result<OwnedFd, ExternalAnchorProvisioningHelperErrorV1> {
    // SAFETY: F_GETFD observes only the scalar fixed descriptor and reports invalid values via errno.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_error("inspect inherited helper descriptor"));
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidDescriptor {
            role,
            reason: "fixed descriptor is unexpectedly close-on-exec",
        });
    }
    // SAFETY: F_DUPFD_CLOEXEC returns one new owned descriptor or reports errno.
    let retained = unsafe {
        libc::fcntl(
            descriptor,
            libc::F_DUPFD_CLOEXEC,
            STAGED_DESCRIPTOR_FLOOR_V1,
        )
    };
    if retained < 0 {
        return Err(descriptor_error("retain inherited helper descriptor"));
    }
    // SAFETY: the successful fcntl returned a new descriptor owned by this process.
    Ok(unsafe { OwnedFd::from_raw_fd(retained) })
}

pub(crate) fn close_fixed(
    descriptor: RawFd,
) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    // SAFETY: callers close each inherited fixed descriptor once after private retention.
    if unsafe { libc::close(descriptor) } != 0 {
        return Err(descriptor_error("close inherited helper descriptor"));
    }
    Ok(())
}

pub(crate) fn stage_above(
    source: &impl AsFd,
    next: &mut RawFd,
    role: &'static str,
) -> Result<OwnedFd, ExternalAnchorProvisioningHelperErrorV1> {
    let staged = rustix::io::fcntl_dupfd_cloexec(source, *next)
        .map_err(|source| io_error("stage helper descriptor", source.into()))?;
    *next = staged.as_raw_fd().checked_add(1).ok_or(
        ExternalAnchorProvisioningHelperErrorV1::InvalidDescriptor {
            role,
            reason: "staged descriptor range overflowed",
        },
    )?;
    Ok(staged)
}

pub(crate) fn validate_bootstrap<const REQUIRE_ROOT: bool>(
    bootstrap: &OwnedFd,
) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap descriptor", source.into()))?;
    let status = rustix::fs::fcntl_getfl(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap status", source.into()))?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDWR
        || !status.contains(OFlags::NONBLOCK)
        || status.intersects(forbidden)
        || rustix::net::sockopt::socket_domain(bootstrap)
            .map_err(|source| io_error("inspect helper bootstrap domain", source.into()))?
            != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(bootstrap)
            .map_err(|source| io_error("inspect helper bootstrap type", source.into()))?
            != SocketType::SEQPACKET
        || rustix::net::sockopt::socket_acceptconn(bootstrap)
            .map_err(|source| io_error("inspect helper bootstrap listener state", source.into()))?
    {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidBootstrap);
    }
    let unnamed = SocketAddrAny::from(SocketAddrUnix::new_unnamed());
    let local = rustix::net::getsockname(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap local address", source.into()))?;
    let remote = rustix::net::getpeername(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap remote address", source.into()))?;
    if local != unnamed || remote.as_ref() != Some(&unnamed) {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidBootstrap);
    }
    match rustix::net::sockopt::socket_error(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap socket error", source.into()))?
    {
        Ok(()) => {}
        Err(source) => {
            return Err(io_error(
                "helper bootstrap has a pending error",
                source.into(),
            ));
        }
    }
    let peer = rustix::net::sockopt::socket_peercred(bootstrap)
        .map_err(|source| io_error("inspect helper bootstrap peer credentials", source.into()))?;
    let expected_parent = rustix::process::getppid();
    if Some(peer.pid) != expected_parent
        || (REQUIRE_ROOT && (!peer.uid.is_root() || !peer.gid.is_root()))
    {
        return Err(ExternalAnchorProvisioningHelperErrorV1::InvalidBootstrap);
    }
    Ok(())
}

pub(crate) fn send_ready(
    bootstrap: &OwnedFd,
    supervisor_peer: &OwnedFd,
    ready: &ExternalAnchorProvisioningReadyV1,
) -> Result<(), ExternalAnchorProvisioningHelperErrorV1> {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    let descriptors = [supervisor_peer.as_fd()];
    if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
        return Err(ExternalAnchorProvisioningHelperErrorV1::ReadyTransfer);
    }
    let count = sendmsg(
        bootstrap,
        &[IoSlice::new(ready.canonical_bytes())],
        &mut ancillary,
        SendFlags::NOSIGNAL,
    )
    .map_err(|source| io_error("send helper-ready endpoint", source.into()))?;
    if count != ready.canonical_bytes().len() {
        return Err(ExternalAnchorProvisioningHelperErrorV1::ReadyTransfer);
    }
    Ok(())
}

unsafe fn exec_fail(bootstrap: RawFd, stage: u8) -> ! {
    let message = EXEC_FAILURE_STAGE_BASE_V1.saturating_add(stage);
    // SAFETY: bootstrap is a retained connected seqpacket and message points to one live byte.
    let _ = unsafe {
        libc::send(
            bootstrap,
            (&raw const message).cast::<c_void>(),
            1,
            libc::MSG_NOSIGNAL,
        )
    };
    // SAFETY: process-local fail-closed termination after an unrecoverable exec stage.
    unsafe { libc::_exit(127) }
}

fn descriptor_error(operation: &'static str) -> ExternalAnchorProvisioningHelperErrorV1 {
    io_error(operation, io::Error::last_os_error())
}

pub(crate) fn io_error(
    operation: &'static str,
    source: io::Error,
) -> ExternalAnchorProvisioningHelperErrorV1 {
    ExternalAnchorProvisioningHelperErrorV1::Io { operation, source }
}

// Call only at the terminal boundary of an isolated single-threaded helper.
// Sources must stay owned above every destination; failure terminates the process.
pub(crate) unsafe fn exec_inherited_daemon<const N: usize>(
    executable: RawFd,
    bootstrap: RawFd,
    transfers: &[(RawFd, RawFd, u8); N],
    name: &CStr,
    failure_stage: u8,
) -> ! {
    // SAFETY: the caller retains every source and authorizes terminal descriptor mutation.
    unsafe {
        if libc::syscall(
            libc::SYS_close_range,
            3_u32,
            u32::MAX,
            CLOSE_RANGE_CLOEXEC_V1,
        ) != 0
        {
            exec_fail(bootstrap, 1);
        }
        for &(source, target, stage) in transfers {
            if libc::dup3(source, target, 0) != target {
                exec_fail(bootstrap, stage);
            }
        }
        let arguments = [name.as_ptr().cast_mut(), std::ptr::null_mut()];
        let environment = [std::ptr::null_mut::<c_char>()];
        libc::syscall(
            libc::SYS_execveat,
            executable,
            c"".as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
        exec_fail(bootstrap, failure_stage)
    }
}
