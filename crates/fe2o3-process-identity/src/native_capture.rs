//! Inert descriptor capture for the existing native compiler launch adapter.
//! No paths are reopened and no process, namespace or execution authority is made.

use crate::{LinuxObjectIdentityV3, PinnedWorkingDirectoryV3};
use rustix::{fs::OFlags, io::FdFlags};
use std::{
    io,
    os::fd::{AsFd, BorrowedFd, FromRawFd, OwnedFd, RawFd},
};

impl PinnedWorkingDirectoryV3 {
    /// Borrows this owner's actual pinned directory for native descriptor staging.
    ///
    /// Checks the retained object and read-only/CLOEXEC directory flags without
    /// reopening its original pathname. Keep this owner alive through staging and
    /// final transfer validation. This establishes neither a binding to captured
    /// cwd text nor protection of directory contents or a namespace mapping.
    /// The caller must exclude descriptor/flag mutation through final staging.
    /// At most three descriptor syscalls; no Rust heap allocation or retry.
    pub fn native_source(&self) -> io::Result<BorrowedFd<'_>> {
        inspect_cwd(self.file.as_fd(), self.object)?;
        Ok(self.file.as_fd())
    }

    /// Checks the final staged duplicate against this still-retained cwd owner.
    ///
    /// The adapter must duplicate `native_source()` and retain exclusive custody
    /// of that transfer. Matching inode/flags alone cannot prove how a supplied
    /// descriptor was obtained or that it shares an open-file description.
    /// Both sources must remain read-only, directory-backed and CLOEXEC. A rename
    /// of the original pathname does not replace this object. At most six
    /// descriptor syscalls; no Rust heap allocation or retry. This grants no authority.
    pub fn validate_native_transfer(&self, transfer: BorrowedFd<'_>) -> io::Result<()> {
        let original = inspect_cwd(self.file.as_fd(), self.object)?;
        let staged = inspect_cwd(transfer, self.object)?;
        if original != staged {
            return Err(stale());
        }
        Ok(())
    }
}

fn inspect_cwd(fd: BorrowedFd<'_>, expected: LinuxObjectIdentityV3) -> io::Result<OFlags> {
    let stat = rustix::fs::fstat(fd)?;
    let observed = LinuxObjectIdentityV3::from_linux_stat(stat.st_dev, stat.st_ino, stat.st_mode);
    if observed != expected || stat.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Err(stale());
    }
    let status = rustix::fs::fcntl_getfl(fd)?;
    if status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || !status.contains(OFlags::DIRECTORY)
        || rustix::io::fcntl_getfd(fd)? != FdFlags::CLOEXEC
    {
        return Err(stale());
    }
    Ok(status)
}

/// One actual open standard descriptor, retained through a CLOEXEC duplicate.
///
/// The duplicate shares the original open-file description (including offset,
/// access mode and status flags). It is not a reopened procfs path. Original
/// descriptor flags are recorded separately because duplication sets CLOEXEC on
/// the retained copy. Borrowing this value supplies no process or I/O authority
/// beyond access to the already captured object.
pub struct CapturedStdioDescriptorV1 {
    source: OwnedFd,
    descriptor_flags: FdFlags,
    status_flags: OFlags,
}

impl CapturedStdioDescriptorV1 {
    /// Retained source for descriptor-only staging or an inert SCM_RIGHTS transfer.
    /// Do not mutate its shared status flags or offset while preparing the child.
    pub fn source(&self) -> BorrowedFd<'_> {
        self.source.as_fd()
    }

    /// Original F_GETFD result, to restore on the child's destination slot.
    /// In particular, original CLOEXEC means that slot must close on exec; the
    /// CLOEXEC flag on `source()` is only transfer hygiene, not this observation.
    pub const fn descriptor_flags(&self) -> FdFlags {
        self.descriptor_flags
    }

    /// Original F_GETFL result. These flags belong to the shared open-file
    /// description: staging must preserve them, never restore them using F_SETFL.
    /// This is an observation, not an immutable guarantee against other aliases.
    pub const fn status_flags(&self) -> OFlags {
        self.status_flags
    }

    fn revalidate(&self) -> io::Result<()> {
        if rustix::io::fcntl_getfd(&self.source)? != FdFlags::CLOEXEC
            || rustix::fs::fcntl_getfl(&self.source)? != self.status_flags
        {
            return Err(stale());
        }
        Ok(())
    }
}

/// Move-only, fixed-size capture of this process's actual standard descriptors.
///
/// `None` records an absent slot, not /dev/null or an instruction to inherit the
/// future launcher's slot. A present slot with original CLOEXEC remains distinct
/// from absence before exec. The compiler adapter must apply both observations.
/// Original slots can close or be reused after capture: the duplicates retain
/// their original open-file descriptions. No supplied FD/PID constructor exists.
/// This is inert transfer input, not compiler approval or invocation provenance.
///
/// ```compile_fail
/// use fe2o3_process_identity::CapturedStdioV1;
/// fn duplicate(v: CapturedStdioV1) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_process_identity::CapturedStdioV1;
/// let _ = CapturedStdioV1::from_files;
/// ```
pub struct CapturedStdioV1 {
    slots: [Option<CapturedStdioDescriptorV1>; 3],
}

impl CapturedStdioV1 {
    /// Captures only the actual slots 0, 1 and 2, without altering them.
    ///
    /// Every duplicate is allocated at FD >= 3, so capturing an open stream never
    /// fills an absent standard slot before that slot is inspected. Only EBADF
    /// from F_GETFD means absence; all other failures refuse. Partial captures
    /// close their duplicates. No opens, reads, writes, seeks, F_SETFL, Rust heap
    /// allocation or syscall retries occur. At most nine descriptor syscalls plus three
    /// closes on failure. Callers prepay this work, the fixed owner storage and
    /// the lifetime of up to three retained kernel descriptors on their account;
    /// no account credit or charge is manufactured here.
    ///
    /// # Safety
    /// The caller must exclusively control slots 0..=2 throughout capture: no
    /// thread, signal handler or foreign code may close, replace or change their
    /// descriptor flags. Exclude status-flag changes through ANY alias to their
    /// open-file descriptions. The caller must independently establish that these
    /// are the intended wrapper's streams. Capture is not an atomic snapshot and
    /// cannot enforce those exclusions. Retaining custody through the future
    /// native staging/exec and accounting for external aliases remain obligations
    /// of that adapter; this function does not authorize a process launch.
    pub unsafe fn capture_current() -> io::Result<Self> {
        let mut result = Self {
            slots: [None, None, None],
        };
        for (slot, destination) in result.slots.iter_mut().zip(0..=2) {
            let flags = match descriptor_flags(destination) {
                Ok(flags) => flags,
                Err(error) if error.raw_os_error() == Some(libc::EBADF) => continue,
                Err(error) => return Err(error),
            };
            // SAFETY: fcntl takes only scalar arguments. The caller excludes
            // source replacement, and the floor cannot occupy an absent stdio slot.
            let duplicate = unsafe { libc::fcntl(destination, libc::F_DUPFD_CLOEXEC, 3) };
            if duplicate < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: successful F_DUPFD_CLOEXEC returned a new, uniquely owned FD.
            let source = unsafe { OwnedFd::from_raw_fd(duplicate) };
            let status_flags = rustix::fs::fcntl_getfl(&source)?;
            *slot = Some(CapturedStdioDescriptorV1 {
                source,
                descriptor_flags: flags,
                status_flags,
            });
        }
        Ok(result)
    }

    /// Captured slot 0, or its observed absence.
    pub const fn stdin(&self) -> Option<&CapturedStdioDescriptorV1> {
        self.slots[0].as_ref()
    }

    /// Captured slot 1, or its observed absence.
    pub const fn stdout(&self) -> Option<&CapturedStdioDescriptorV1> {
        self.slots[1].as_ref()
    }

    /// Captured slot 2, or its observed absence.
    pub const fn stderr(&self) -> Option<&CapturedStdioDescriptorV1> {
        self.slots[2].as_ref()
    }

    /// Rechecks retained-copy CLOEXEC and captured shared status flags only.
    /// Does not inspect the original 0/1/2 slots, compare offsets, authenticate
    /// writers, or prove OFD identity for any later received duplicate. At most
    /// six descriptor syscalls; no mutation, Rust heap allocation or retry.
    pub fn revalidate(&self) -> io::Result<()> {
        for descriptor in self.slots.iter().flatten() {
            descriptor.revalidate()?;
        }
        Ok(())
    }
}

fn descriptor_flags(fd: RawFd) -> io::Result<FdFlags> {
    // SAFETY: a scalar probe is valid even when this raw standard slot is absent.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FdFlags::from_bits_retain(flags as _))
}

fn stale() -> io::Error {
    io::Error::from_raw_os_error(libc::ESTALE)
}
