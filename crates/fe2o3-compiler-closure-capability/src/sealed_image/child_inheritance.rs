//! One reservation and async-signal-safe exec hook for sealed public records.
use super::*;
use crate::native_capability::{CompilerExecutionCapabilityErrorV2 as Error, Result};

impl SealedCapabilityImage {
    /// The caller prepays native work, this alias, and the Command's hook storage.
    /// There is no fallible operation after transferring the alias to Command.
    pub(crate) fn inherit_fixed(&self, command: &mut Command, child_fd: RawFd) -> Result<()> {
        self.revalidate_fixed()?;
        if child_fd < 3 {
            return Err(Error::Rejected("child descriptor overlaps stdio"));
        }
        // SAFETY: scalar inspection does not borrow or consume the target slot.
        let flags = unsafe { libc::fcntl(child_fd, libc::F_GETFD) };
        if flags >= 0 {
            return Err(Error::Rejected(
                "reserved child descriptor is already in use",
            ));
        }
        if std::io::Error::last_os_error().raw_os_error() != Some(libc::EBADF) {
            return Err(Error::last_os("inspect reserved child descriptor"));
        }
        // F_DUPFD_CLOEXEC never replaces an occupied descriptor, including a
        // concurrent claimant between the inspection and reservation.
        let reserved = rustix::io::fcntl_dupfd_cloexec(&self.image, child_fd)
            .map_err(|e| Error::io("reserve child descriptor", e))?;
        if reserved.as_raw_fd() != child_fd {
            return Err(Error::Rejected(
                "reserved child descriptor was concurrently claimed",
            ));
        }
        let device = self.device;
        let inode = self.inode;
        let length = self.length as i64;
        // SAFETY: Command owns the alias through spawn and drop. The hook uses
        // only async-signal-safe descriptor syscalls and initialized scalar data.
        unsafe {
            command.pre_exec(move || {
                if rustix::fs::fcntl_get_seals(&reserved).map_err(std::io::Error::from)?
                    != REQUIRED_SEALS
                    || !rustix::io::fcntl_getfd(&reserved)
                        .map_err(std::io::Error::from)?
                        .contains(rustix::io::FdFlags::CLOEXEC)
                {
                    return Err(std::io::Error::from_raw_os_error(libc::EPERM));
                }
                let stat = rustix::fs::fstat(&reserved).map_err(std::io::Error::from)?;
                if stat.st_mode != libc::S_IFREG | 0o400
                    || stat.st_size != length
                    || stat.st_dev != device
                    || stat.st_ino != inode
                {
                    return Err(std::io::Error::from_raw_os_error(libc::ESTALE));
                }
                rustix::io::fcntl_setfd(&reserved, rustix::io::FdFlags::empty())
                    .map_err(std::io::Error::from)
            });
        }
        Ok(())
    }
}
