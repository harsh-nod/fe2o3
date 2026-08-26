//! Descriptor policy applied immediately before an application-boundary exec.

use std::io;
use std::os::fd::RawFd;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
use std::os::unix::process::CommandExt;
#[cfg(any(test, feature = "qualification-oracles-test-only"))]
use std::process::Command;

/// Marks every non-stdio descriptor close-on-exec without closing descriptors that trusted
/// pre-exec setup still needs. Callers may subsequently expose only their exact application ABI.
pub(crate) fn protect_all_nonstdio_descriptors() -> io::Result<()> {
    // SAFETY: close_range with CLOEXEC changes descriptor flags only; it neither dereferences
    // userspace memory nor closes descriptors before the final exec.
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            3_u32,
            u32::MAX,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Exposes one already-validated descriptor as an intentional child ABI descriptor.
pub(crate) fn expose_descriptor(fd: RawFd) -> io::Result<()> {
    let flags = get_descriptor_flags(fd)?;
    set_descriptor_flags(fd, flags & !libc::FD_CLOEXEC)?;
    if get_descriptor_flags(fd)? & libc::FD_CLOEXEC != 0 {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    }
    Ok(())
}

#[cfg(any(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn configure_closed_descriptor_baseline(command: &mut Command) {
    // SAFETY: the callback invokes one async-signal-safe raw syscall and retains no borrowed
    // process state. No descriptor above stderr is part of this application ABI.
    unsafe {
        command.pre_exec(protect_all_nonstdio_descriptors);
    }
}

fn get_descriptor_flags(fd: RawFd) -> io::Result<i32> {
    loop {
        // SAFETY: F_GETFD reads flags for only the supplied raw descriptor.
        let result = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if result >= 0 {
            return Ok(result);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn set_descriptor_flags(fd: RawFd, flags: i32) -> io::Result<()> {
    loop {
        // SAFETY: F_SETFD updates flags for only the supplied raw descriptor.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, flags) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
