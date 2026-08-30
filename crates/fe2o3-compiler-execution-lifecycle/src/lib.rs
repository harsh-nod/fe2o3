//! Crash-retained compiler-execution lifecycle lease custody.
//!
//! A protected service receives an independently opened, already shared-locked descriptor from
//! the root coordinator. Admission binds that descriptor to the canonical sibling of the retained
//! service state root and reacquires the shared lease on the same open file description. Drop only
//! closes this process's descriptor; it never calls `LOCK_UN`, which would release a lock still
//! carried by a fork or exec duplicate of the same open file description.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![deny(missing_docs, unsafe_code)]

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
};
use rustix::fs::{AtFlags, FileType, FlockOperation, Mode, OFlags, flock, fstat, openat, statat};

const ROOT_ID_V1: u32 = 0;
const LIFECYCLE_PARENT_MODE_V1: u32 = 0o755;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    byte_len: u64,
}

/// Move-only shared lifecycle custody retained by one protected service process.
///
/// This value deliberately does not implement `Clone`. Descriptor duplication is confined to the
/// root-controlled spawn path, and dropping this value closes rather than explicitly unlocks its
/// open file description.
pub struct CompilerExecutionServiceLifecycleLeaseV1 {
    file: File,
    parent: File,
    snapshot: ObjectSnapshotV1,
    expected_uid: u32,
    expected_gid: u32,
}

impl fmt::Debug for CompilerExecutionServiceLifecycleLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionServiceLifecycleLeaseV1")
            .field("authority", &"deployment-lifecycle-only")
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionServiceLifecycleLeaseV1 {
    /// Admits one root-owned lifecycle descriptor relative to a retained service state root.
    pub fn admit(file: File, state_root: &impl AsFd) -> Result<Self, LifecycleLeaseErrorV1> {
        Self::admit_for_owner(file, state_root, ROOT_ID_V1, ROOT_ID_V1)
    }

    fn admit_for_owner(
        file: File,
        state_root: &impl AsFd,
        expected_uid: u32,
        expected_gid: u32,
    ) -> Result<Self, LifecycleLeaseErrorV1> {
        let parent = File::from(
            openat(
                state_root,
                "..",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| io_error("derive lifecycle parent from service root", source))?,
        );
        validate_parent(&parent, expected_uid, expected_gid)?;
        let snapshot = validate_file(&file, expected_uid, expected_gid)?;
        validate_named_file(&parent, snapshot)?;
        acquire_shared(&file)?;
        let lease = Self {
            file,
            parent,
            snapshot,
            expected_uid,
            expected_gid,
        };
        lease.revalidate()?;
        Ok(lease)
    }

    /// Revalidates the retained descriptor, canonical pathname, parent, and shared lease.
    pub fn revalidate(&self) -> Result<(), LifecycleLeaseErrorV1> {
        validate_parent(&self.parent, self.expected_uid, self.expected_gid)?;
        if validate_file(&self.file, self.expected_uid, self.expected_gid)? != self.snapshot {
            return Err(LifecycleLeaseErrorV1::FileChanged);
        }
        validate_named_file(&self.parent, self.snapshot)?;
        acquire_shared(&self.file)?;
        if validate_file(&self.file, self.expected_uid, self.expected_gid)? != self.snapshot {
            return Err(LifecycleLeaseErrorV1::FileChanged);
        }
        validate_named_file(&self.parent, self.snapshot)
    }
}

impl AsFd for CompilerExecutionServiceLifecycleLeaseV1 {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

fn validate_parent(
    parent: &File,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), LifecycleLeaseErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(parent)
        .map_err(|source| io_error("inspect lifecycle parent descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(parent)
        .map_err(|source| io_error("inspect lifecycle parent status flags", source))?;
    let stat = fstat(parent).map_err(|source| io_error("inspect lifecycle parent", source))?;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != LIFECYCLE_PARENT_MODE_V1
        || stat.st_uid != expected_uid
        || stat.st_gid != expected_gid
        || stat.st_nlink == 0
    {
        return Err(LifecycleLeaseErrorV1::InvalidParent);
    }
    require_absent_attributes(parent, "inspect lifecycle parent extended attributes")
}

fn validate_file(
    file: &File,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<ObjectSnapshotV1, LifecycleLeaseErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(file)
        .map_err(|source| io_error("inspect lifecycle descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(file)
        .map_err(|source| io_error("inspect lifecycle status flags", source))?;
    let stat = fstat(file).map_err(|source| io_error("inspect lifecycle file", source))?;
    let snapshot = snapshot(&stat);
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(snapshot.mode) != FileType::RegularFile
        || snapshot.mode & 0o7777 != COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1
        || snapshot.uid != expected_uid
        || snapshot.gid != expected_gid
        || snapshot.links != 1
        || snapshot.byte_len != 0
    {
        return Err(LifecycleLeaseErrorV1::InvalidFile);
    }
    require_absent_attributes(file, "inspect lifecycle file extended attributes")?;
    Ok(snapshot)
}

fn validate_named_file(
    parent: &File,
    expected: ObjectSnapshotV1,
) -> Result<(), LifecycleLeaseErrorV1> {
    let name = Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
        .file_name()
        .ok_or(LifecycleLeaseErrorV1::InvalidCanonicalPath)?;
    let linked = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|source| io_error("inspect canonical lifecycle pathname", source))?;
    if snapshot(&linked) != expected {
        return Err(LifecycleLeaseErrorV1::PathChanged);
    }
    Ok(())
}

fn snapshot(stat: &rustix::fs::Stat) -> ObjectSnapshotV1 {
    ObjectSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
        byte_len: stat.st_size.try_into().unwrap_or(u64::MAX),
    }
}

fn require_absent_attributes(
    file: &File,
    operation: &'static str,
) -> Result<(), LifecycleLeaseErrorV1> {
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::fgetxattr(file, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(LifecycleLeaseErrorV1::ForbiddenAttributes);
            }
            Err(source) => return Err(io_error(operation, source)),
        }
    }
    Ok(())
}

fn acquire_shared(file: &File) -> Result<(), LifecycleLeaseErrorV1> {
    flock(file, FlockOperation::NonBlockingLockShared).map_err(|source| {
        if source == rustix::io::Errno::WOULDBLOCK {
            LifecycleLeaseErrorV1::Busy
        } else {
            io_error("acquire protected-service lifecycle lease", source)
        }
    })
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> LifecycleLeaseErrorV1 {
    LifecycleLeaseErrorV1::Io {
        operation,
        source: source.into(),
    }
}

/// Stable protected-service lifecycle lease admission failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum LifecycleLeaseErrorV1 {
    /// The canonical lifecycle path constant has no terminal file name.
    InvalidCanonicalPath,
    /// The canonical lifecycle parent has the wrong descriptor or filesystem policy.
    InvalidParent,
    /// The lifecycle file has the wrong descriptor or filesystem policy.
    InvalidFile,
    /// The retained lifecycle file identity changed.
    FileChanged,
    /// The canonical pathname no longer names the retained lifecycle file.
    PathChanged,
    /// The lifecycle parent or file carries a forbidden ACL or capability.
    ForbiddenAttributes,
    /// Provisioning currently owns the exclusive lifecycle lease.
    Busy,
    /// A bounded descriptor or filesystem operation failed.
    Io {
        /// Exact operation that failed.
        operation: &'static str,
        /// Kernel error reported by the operation.
        source: io::Error,
    },
}

impl fmt::Display for LifecycleLeaseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCanonicalPath => formatter.write_str("invalid canonical lifecycle path"),
            Self::InvalidParent => formatter.write_str("invalid lifecycle parent"),
            Self::InvalidFile => formatter.write_str("invalid lifecycle file"),
            Self::FileChanged => formatter.write_str("lifecycle file identity changed"),
            Self::PathChanged => formatter.write_str("lifecycle pathname identity changed"),
            Self::ForbiddenAttributes => {
                formatter.write_str("lifecycle object has forbidden attributes")
            }
            Self::Busy => formatter.write_str("compiler-execution lifecycle is busy"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for LifecycleLeaseErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    struct Fixture {
        root: tempfile::TempDir,
        state: File,
        lock_path: std::path::PathBuf,
        uid: u32,
        gid: u32,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            std::fs::set_permissions(
                root.path(),
                std::fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
            )
            .unwrap();
            let state_path = root.path().join("state");
            std::fs::create_dir(&state_path).unwrap();
            let lock_path = root.path().join(
                Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
                    .file_name()
                    .unwrap(),
            );
            std::fs::write(&lock_path, []).unwrap();
            std::fs::set_permissions(
                &lock_path,
                std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
            )
            .unwrap();
            Self {
                root,
                state: File::open(state_path).unwrap(),
                lock_path,
                uid: rustix::process::geteuid().as_raw(),
                gid: rustix::process::getegid().as_raw(),
            }
        }

        fn admit(&self) -> CompilerExecutionServiceLifecycleLeaseV1 {
            CompilerExecutionServiceLifecycleLeaseV1::admit_for_owner(
                File::open(&self.lock_path).unwrap(),
                &self.state,
                self.uid,
                self.gid,
            )
            .unwrap()
        }
    }

    #[test]
    fn independent_child_lease_survives_unrelated_owner_unlock() {
        let fixture = Fixture::new();
        let owner = File::open(&fixture.lock_path).unwrap();
        flock(&owner, FlockOperation::NonBlockingLockShared).unwrap();
        let owner_duplicate = owner.try_clone().unwrap();
        let child = fixture.admit();

        flock(&owner, FlockOperation::Unlock).unwrap();
        drop(owner);
        assert_eq!(
            flock(
                File::open(&fixture.lock_path).unwrap(),
                FlockOperation::NonBlockingLockExclusive,
            ),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
        drop(owner_duplicate);
        child.revalidate().unwrap();
        drop(child);
        flock(
            File::open(&fixture.lock_path).unwrap(),
            FlockOperation::NonBlockingLockExclusive,
        )
        .unwrap();
    }

    #[test]
    fn pathname_replacement_is_rejected_without_releasing_old_lease() {
        let fixture = Fixture::new();
        let child = fixture.admit();
        std::fs::rename(&fixture.lock_path, fixture.root.path().join("displaced")).unwrap();
        std::fs::write(&fixture.lock_path, []).unwrap();
        std::fs::set_permissions(
            &fixture.lock_path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        assert!(matches!(
            child.revalidate(),
            Err(LifecycleLeaseErrorV1::PathChanged)
        ));
    }

    #[test]
    fn exact_mode_and_empty_single_link_file_are_required() {
        let fixture = Fixture::new();
        std::fs::set_permissions(&fixture.lock_path, std::fs::Permissions::from_mode(0o600))
            .unwrap();
        std::fs::write(&fixture.lock_path, [1]).unwrap();
        std::fs::set_permissions(
            &fixture.lock_path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        assert!(matches!(
            CompilerExecutionServiceLifecycleLeaseV1::admit_for_owner(
                File::open(&fixture.lock_path).unwrap(),
                &fixture.state,
                fixture.uid,
                fixture.gid,
            ),
            Err(LifecycleLeaseErrorV1::InvalidFile)
        ));
    }
}
