//! Shared trusted-tree predicates. Sealed descriptors use different rules.
use crate::native_capability::CompilerExecutionCapabilityErrorV2;
use rustix::fs::{FileType, OFlags};
use std::{
    fs::File,
    os::{fd::AsFd, unix::fs::MetadataExt},
};

pub(crate) const COMPONENTS: [&str; 3] = ["etc", "fe2o3", "compiler-execution"];
pub(crate) const TRUSTED_FILE_MODE: u32 = 0o444;
pub(crate) const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
pub(crate) const FILE_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TrustedFileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

pub(crate) enum TrustedProfileError {
    Inspect(i32),
    Invalid,
    ForbiddenAttribute(&'static str),
    InspectAttribute { attribute: &'static str, errno: i32 },
}
impl TrustedProfileError {
    pub(crate) fn legacy(self, label: &str, directory: bool) -> String {
        let object = if directory {
            "trusted client-profile directory"
        } else {
            "trusted client-profile file"
        };
        match self {
            Self::Inspect(errno) => {
                let kind = if directory {
                    "trusted directory"
                } else {
                    "trusted client profile"
                };
                format!(
                    "cannot inspect {kind} {label:?}: {}",
                    std::io::Error::from_raw_os_error(errno)
                )
            }
            Self::Invalid if directory => format!(
                "trusted client-profile directory {label:?} has invalid descriptor, type, owner, mode, or link state"
            ),
            Self::Invalid => format!(
                "trusted client profile {label:?} has invalid descriptor, type, owner, mode, link count, or length"
            ),
            Self::ForbiddenAttribute(attribute) => {
                format!("{object} has forbidden capability or POSIX ACL attribute {attribute:?}")
            }
            Self::InspectAttribute { attribute, errno } => format!(
                "cannot inspect {object} extended attribute {attribute:?}: {}",
                std::io::Error::from_raw_os_error(errno)
            ),
        }
    }
}
impl From<TrustedProfileError> for CompilerExecutionCapabilityErrorV2 {
    fn from(value: TrustedProfileError) -> Self {
        match value {
            TrustedProfileError::Inspect(errno) => Self::Io {
                operation: "inspect trusted profile object",
                errno,
            },
            TrustedProfileError::Invalid => Self::Rejected(
                "invalid trusted profile descriptor, type, owner, mode, links, or length",
            ),
            TrustedProfileError::ForbiddenAttribute(_) => {
                Self::Rejected("trusted profile object has a forbidden attribute")
            }
            TrustedProfileError::InspectAttribute { errno, .. } => Self::Io {
                operation: "inspect trusted profile attribute",
                errno,
            },
        }
    }
}
fn inspect(error: rustix::io::Errno) -> TrustedProfileError {
    TrustedProfileError::Inspect(error.raw_os_error())
}

pub(crate) fn validate_directory(
    directory: &File,
    uid: u32,
    gid: u32,
) -> Result<(), TrustedProfileError> {
    let flags = rustix::io::fcntl_getfd(directory).map_err(inspect)?;
    let status = rustix::fs::fcntl_getfl(directory).map_err(inspect)?;
    let stat = rustix::fs::fstat(directory).map_err(inspect)?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != uid
        || stat.st_gid != gid
        || stat.st_nlink == 0
        || stat.st_mode & 0o022 != 0
        || stat.st_mode & 0o100 == 0
    {
        return Err(TrustedProfileError::Invalid);
    }
    require_absent_xattrs(directory)
}

pub(crate) fn validate_file(
    profile: &File,
    uid: u32,
    gid: u32,
    bytes: usize,
) -> Result<TrustedFileSnapshot, TrustedProfileError> {
    let flags = rustix::io::fcntl_getfd(profile).map_err(inspect)?;
    let status = rustix::fs::fcntl_getfl(profile).map_err(inspect)?;
    let metadata = profile
        .metadata()
        .map_err(|e| TrustedProfileError::Inspect(e.raw_os_error().unwrap_or(libc::EIO)))?;
    let snapshot = TrustedFileSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    };
    if !flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(snapshot.mode) != FileType::RegularFile
        || snapshot.uid != uid
        || snapshot.gid != gid
        || snapshot.links != 1
        || snapshot.mode & 0o7777 != TRUSTED_FILE_MODE
        || snapshot.length != bytes as u64
    {
        return Err(TrustedProfileError::Invalid);
    }
    require_absent_xattrs(profile)?;
    Ok(snapshot)
}

const ATTRIBUTES: [&str; 3] = [
    "security.capability",
    "system.posix_acl_access",
    "system.posix_acl_default",
];
fn require_absent_xattrs(object: &impl AsFd) -> Result<(), TrustedProfileError> {
    for attribute in ATTRIBUTES {
        let mut byte = 0;
        admit_attribute(
            attribute,
            rustix::fs::fgetxattr(object, attribute, std::slice::from_mut(&mut byte)),
        )?;
    }
    Ok(())
}
fn admit_attribute(
    attribute: &'static str,
    result: rustix::io::Result<usize>,
) -> Result<(), TrustedProfileError> {
    match result {
        Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => {
            Err(TrustedProfileError::ForbiddenAttribute(attribute))
        }
        Err(error) => Err(TrustedProfileError::InspectAttribute {
            attribute,
            errno: error.raw_os_error(),
        }),
    }
}

const _: () = {
    use crate::native_capability::envelope_overhead;
    use std::mem::size_of;
    assert!(
        4 * size_of::<rustix::fs::Stat>()
            + 8 * size_of::<TrustedProfileError>()
            + 64 * size_of::<usize>()
            + 2 * size_of::<std::array::IntoIter<&'static str, 3>>()
            + 512
            + size_of::<Result<(), TrustedProfileError>>()
            + envelope_overhead::<TrustedFileSnapshot, TrustedProfileError>()
            + envelope_overhead::<usize, rustix::io::Errno>()
            <= 4096
    );
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_attribute_result_is_classified_without_allocating_a_value_buffer() {
        use rustix::io::Errno;
        for name in ATTRIBUTES {
            for value in [Ok(0), Ok(1), Err(Errno::RANGE)] {
                assert!(
                    matches!(admit_attribute(name, value), Err(TrustedProfileError::ForbiddenAttribute(actual)) if actual == name)
                );
            }
            for error in [Errno::NODATA, Errno::OPNOTSUPP] {
                assert!(admit_attribute(name, Err(error)).is_ok());
            }
            assert!(
                matches!(admit_attribute(name, Err(Errno::INTR)), Err(TrustedProfileError::InspectAttribute { attribute, errno }) if attribute == name && errno == libc::EINTR)
            );
        }
    }

    #[test]
    fn legacy_diagnostics_preserve_labels_and_attribute_context() {
        let label = "a\"b";
        assert_eq!(
            TrustedProfileError::Invalid.legacy(label, true),
            "trusted client-profile directory \"a\\\"b\" has invalid descriptor, type, owner, mode, or link state"
        );
        assert_eq!(
            TrustedProfileError::Invalid.legacy(label, false),
            "trusted client profile \"a\\\"b\" has invalid descriptor, type, owner, mode, link count, or length"
        );
        for (directory, object, inspect_kind) in [
            (
                true,
                "trusted client-profile directory",
                "trusted directory",
            ),
            (
                false,
                "trusted client-profile file",
                "trusted client profile",
            ),
        ] {
            assert_eq!(
                TrustedProfileError::Inspect(libc::EACCES).legacy(label, directory),
                format!(
                    "cannot inspect {inspect_kind} {label:?}: {}",
                    std::io::Error::from_raw_os_error(libc::EACCES)
                )
            );
            for attribute in ATTRIBUTES {
                assert_eq!(
                    TrustedProfileError::ForbiddenAttribute(attribute).legacy(label, directory),
                    format!(
                        "{object} has forbidden capability or POSIX ACL attribute {attribute:?}"
                    )
                );
                assert_eq!(
                    TrustedProfileError::InspectAttribute {
                        attribute,
                        errno: libc::EACCES
                    }
                    .legacy(label, directory),
                    format!(
                        "cannot inspect {object} extended attribute {attribute:?}: {}",
                        std::io::Error::from_raw_os_error(libc::EACCES)
                    )
                );
            }
        }
    }
}
