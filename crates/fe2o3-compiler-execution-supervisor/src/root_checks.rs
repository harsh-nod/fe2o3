//! Policy-neutral root inspection, with fixed diagnostics and no retained authority.
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_STATE_ROOT_MODE_V1;
use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as IssuerServiceCredentialProfileV1;
use rustix::fs::{FileType, OFlags};
use rustix::io::{Errno, FdFlags};
use std::ffi::CStr;
use std::fs::File;

const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;
const FORBIDDEN_XATTRS: [&CStr; 3] = [
    c"security.capability",
    c"system.posix_acl_access",
    c"system.posix_acl_default",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RootSnapshot {
    pub(crate) device: u64,
    pub(crate) inode: u64,
    pub(crate) mode: u32,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) links: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RootCheckError {
    Invalid(&'static str),
    Io {
        operation: &'static str,
        errno: Errno,
    },
}

/// Six syscalls on success: descriptor flags, status flags, stat, and three
/// one-byte xattr probes. Preserve query and predicate order for legacy callers.
pub(crate) fn inspect(
    root: &File,
    credentials: IssuerServiceCredentialProfileV1,
) -> Result<RootSnapshot, RootCheckError> {
    let descriptor_flags = rustix::io::fcntl_getfd(root).map_err(|errno| RootCheckError::Io {
        operation: "inspect protected issuer root descriptor flags",
        errno,
    })?;
    let status = rustix::fs::fcntl_getfl(root).map_err(|errno| RootCheckError::Io {
        operation: "inspect protected issuer root status flags",
        errno,
    })?;
    let stat = rustix::fs::fstat(root).map_err(|errno| RootCheckError::Io {
        operation: "inspect protected issuer root",
        errno,
    })?;
    let snapshot = RootSnapshot {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
    };
    validate_snapshot(descriptor_flags, status, snapshot, credentials)?;
    for attribute in FORBIDDEN_XATTRS {
        require_absent_xattr(root, attribute)?;
    }
    Ok(snapshot)
}

fn validate_snapshot(
    descriptor_flags: FdFlags,
    status: OFlags,
    snapshot: RootSnapshot,
    credentials: IssuerServiceCredentialProfileV1,
) -> Result<(), RootCheckError> {
    if !descriptor_flags.contains(FdFlags::CLOEXEC) {
        return Err(RootCheckError::Invalid("descriptor is inheritable"));
    }
    if status & OFlags::ACCMODE != OFlags::RDONLY || status.contains(OFlags::PATH) {
        return Err(RootCheckError::Invalid(
            "descriptor is not read-only directory custody",
        ));
    }
    if FileType::from_raw_mode(snapshot.mode) != FileType::Directory {
        return Err(RootCheckError::Invalid("object is not a directory"));
    }
    if snapshot.uid != credentials.uid() || snapshot.gid != credentials.gid() {
        return Err(RootCheckError::Invalid(
            "owner does not match the service UID and GID",
        ));
    }
    if snapshot.mode & PERMISSION_AND_SPECIAL_BITS
        != COMPILER_EXECUTION_SUPERVISOR_STATE_ROOT_MODE_V1
    {
        return Err(RootCheckError::Invalid("mode is not exactly 0700"));
    }
    if snapshot.links == 0 {
        return Err(RootCheckError::Invalid("directory is unlinked"));
    }
    Ok(())
}

fn require_absent_xattr(root: &File, attribute: &CStr) -> Result<(), RootCheckError> {
    let mut byte = 0_u8;
    classify_xattr(rustix::fs::fgetxattr(
        root,
        attribute,
        std::slice::from_mut(&mut byte),
    ))
}

fn classify_xattr(result: rustix::io::Result<usize>) -> Result<(), RootCheckError> {
    match result {
        Err(Errno::NODATA | Errno::OPNOTSUPP) => Ok(()),
        Ok(_) | Err(Errno::RANGE) => Err(RootCheckError::Invalid(
            "directory has a forbidden capability or POSIX ACL",
        )),
        Err(errno) => Err(RootCheckError::Io {
            operation: "inspect protected issuer root extended attributes",
            errno,
        }),
    }
}

#[cfg(test)]
#[path = "root_checks_tests.rs"]
mod tests;
