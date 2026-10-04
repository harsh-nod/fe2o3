//! Close-only custody of the complete descriptor-root observer lock set.

use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::fs::{FileType, OFlags, fcntl_getfl, fstat};
use rustix::io::{FdFlags, fcntl_dupfd_cloexec, fcntl_getfd};

/// Inert, move-only retention of the two original observer lock descriptions.
///
/// The order is the named artifact OFD lock followed by the output-directory flock. This
/// deliberately supports only descriptor-root, nonrepairing observations, which have no third
/// pathname guard. Duplicating or transferring these descriptions preserves their locks when
/// the original observer exits, including abrupt process death. No lock is unlocked on drop;
/// the kernel releases it when the last description reference closes.
///
/// This is NOT a currentness token or proof that received descriptions actually hold locks.
/// Authentication, session binding, current publication checks, and retention through durable
/// commit belong to the private observer transport. Never expose these scoped filesystem
/// capabilities to the application or an untrusted process. A recipient must not unlock them,
/// change their flags, or write the lock file. Locks coordinate cooperating writers only.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffLockRetentionV3;
/// fn cloneable<T: Clone>() {}
/// cloneable::<CompilerModuleHandoffLockRetentionV3>();
/// ```
/// ```compile_fail
/// use fe2o3_artifact_transaction::{
///     CompilerModuleHandoffConsumptionTokenV3, CompilerModuleHandoffLockRetentionV3,
/// };
/// fn token(locks: CompilerModuleHandoffLockRetentionV3) -> CompilerModuleHandoffConsumptionTokenV3 {
///     locks.into()
/// }
/// ```
pub struct CompilerModuleHandoffLockRetentionV3 {
    descriptors: Option<[OwnedFd; 2]>,
}

impl fmt::Debug for CompilerModuleHandoffLockRetentionV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffLockRetentionV3")
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl CompilerModuleHandoffLockRetentionV3 {
    pub(crate) fn retain_observer_lock(lock: &crate::OutputLock) -> io::Result<Self> {
        if lock.path_guard.is_some() {
            return Err(invalid(
                "observer lock retention does not support a pathname guard",
            ));
        }
        let named = lock
            .fd
            .as_ref()
            .ok_or_else(|| invalid("missing named lock"))?;
        let root = lock
            .root_guard
            .as_ref()
            .ok_or_else(|| invalid("missing output-directory lock"))?;
        Self::from_received_descriptors([
            fcntl_dupfd_cloexec(named, 0)?,
            fcntl_dupfd_cloexec(root, 0)?,
        ])
    }

    /// Retains exactly two received descriptors after checking their shape and close-on-exec.
    ///
    /// Call only after validating the enclosing private transport packet, its kernel sender
    /// credentials, operation identity, and exact ancillary roster. These checks are intentionally
    /// inert: a reopened regular file and directory can pass them without retaining any lock.
    /// The caller must establish that a trusted observer sent the original locked descriptions.
    /// This function neither acquires locks nor observes publication contents.
    pub fn from_received_descriptors(descriptors: [OwnedFd; 2]) -> io::Result<Self> {
        let retained = Self {
            descriptors: Some(descriptors),
        };
        retained.validate_shape()?;
        Ok(retained)
    }

    /// Borrows the ordered original descriptions for one private `SCM_RIGHTS` transfer.
    ///
    /// Borrowing cannot move custody out of this owner. The receiver must retain its aliases
    /// through durable commit even if the sending process disappears. Do not reopen these
    /// descriptors via `/proc/self/fd`: that would create independent, unlocked descriptions.
    pub fn transfer_descriptors(&self) -> [BorrowedFd<'_>; 2] {
        let descriptors = self
            .descriptors
            .as_ref()
            .expect("live retention owns descriptors");
        [descriptors[0].as_fd(), descriptors[1].as_fd()]
    }

    fn validate_shape(&self) -> io::Result<()> {
        for (index, descriptor) in self.transfer_descriptors().into_iter().enumerate() {
            if fcntl_getfd(descriptor)? != FdFlags::CLOEXEC {
                return Err(invalid("observer lock descriptor must be close-on-exec"));
            }
            let status = fcntl_getfl(descriptor)?;
            let access = if index == 0 {
                OFlags::RDWR
            } else {
                OFlags::RDONLY
            };
            if status & OFlags::ACCMODE != access
                || status.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            {
                return Err(invalid(
                    "observer lock descriptor has unexpected access flags",
                ));
            }
            let stat = fstat(descriptor)?;
            if index == 0 {
                if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
                    || stat.st_nlink != 1
                    || stat.st_mode & 0o077 != 0
                {
                    return Err(invalid(
                        "named observer lock must be a private single-link file",
                    ));
                }
            } else if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
                || stat.st_nlink == 0
            {
                return Err(invalid("observer root lock must be a linked directory"));
            }
        }
        Ok(())
    }
}

impl Drop for CompilerModuleHandoffLockRetentionV3 {
    fn drop(&mut self) {
        crate::ArtifactProcessSpawnCoordinatorV1::global().release_lock_descriptors(|| {
            drop(self.descriptors.take());
        });
    }
}

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, reason)
}
