//! One directory lock shared by both issuer wire families.
use rustix::fs::{FlockOperation, Mode, OFlags, flock};
use std::{
    io,
    os::fd::{AsFd, OwnedFd},
};

pub(crate) struct SingletonLock {
    descriptor: OwnedFd,
}
impl SingletonLock {
    #[cfg(test)]
    pub(crate) fn descriptor(&self) -> std::os::fd::BorrowedFd<'_> {
        self.descriptor.as_fd()
    }

    pub fn acquire(root: &impl AsFd) -> io::Result<Self> {
        let descriptor = rustix::fs::openat(
            root,
            ".",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?;
        let before = rustix::fs::fstat(root)?;
        let actual = rustix::fs::fstat(&descriptor)?;
        if (before.st_dev, before.st_ino) != (actual.st_dev, actual.st_ino) {
            return Err(io::Error::other(
                "issuer lock descriptor does not name the retained service root",
            ));
        }
        if !rustix::io::fcntl_getfd(&descriptor)?.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(io::Error::other("issuer lock descriptor lacks FD_CLOEXEC"));
        }
        flock(&descriptor, FlockOperation::NonBlockingLockExclusive)?;
        Ok(Self { descriptor })
    }
}
impl Drop for SingletonLock {
    fn drop(&mut self) {
        // Release the shared open-file description, including transient fork inheritance.
        let _ = flock(&self.descriptor, FlockOperation::Unlock);
    }
}
