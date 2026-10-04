//! Policy-neutral launch-object predicates; callers own admission and resource accounting.

use std::os::fd::{AsFd, OwnedFd};

use fe2o3_compiler_execution_issuer::{
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1, COMPILER_EXECUTION_ISSUER_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1, COMPILER_EXECUTION_ISSUER_READY_FD_V1,
    COMPILER_EXECUTION_ISSUER_ROOT_FD_V1, COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1,
};
use fe2o3_static_preexec_manifest::{
    PREEXEC_MANIFEST_BYTES_V1, PREEXEC_SOURCE_FD_BASE, StaticPreexecDescriptorV1 as Descriptor,
    StaticPreexecObjectIdentityV1 as Object,
};
use rustix::fs::{FileType, OFlags, SealFlags};
use rustix::io::{Errno, FdFlags};
use rustix::pipe::{PipeFlags, pipe_with};

pub(super) const SOURCE_COUNT_V1: usize = 12;
pub(super) const STDIN_SOURCE_INDEX: usize = 0;
pub(super) const STDOUT_SOURCE_INDEX: usize = 1;
pub(super) const STDERR_SOURCE_INDEX: usize = 2;
pub(super) const ROOT_SOURCE_INDEX: usize = 3;
pub(super) const SERVICE_PEER_SOURCE_INDEX: usize = 4;
pub(super) const CLIENT_PIDFD_SOURCE_INDEX: usize = 5;
pub(super) const POLICY_SOURCE_INDEX: usize = 6;
pub(super) const SIGNING_KEY_SOURCE_INDEX: usize = 7;
pub(super) const LAUNCH_MANIFEST_SOURCE_INDEX: usize = 8;
pub(super) const READINESS_SOURCE_INDEX: usize = 9;
pub(super) const EXTERNAL_ANCHOR_PEER_SOURCE_INDEX: usize = 10;
pub(super) const EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX: usize = 11;
pub(super) const MANIFEST_MODE_V1: u32 = 0o400;
pub(super) const REQUIRED_MANIFEST_SEALS_V1: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

pub(super) const DESTINATION_FDS_V1: [i32; SOURCE_COUNT_V1] = [
    libc::STDIN_FILENO,
    libc::STDOUT_FILENO,
    libc::STDERR_FILENO,
    COMPILER_EXECUTION_ISSUER_ROOT_FD_V1,
    COMPILER_EXECUTION_ISSUER_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1,
    COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1,
    COMPILER_EXECUTION_ISSUER_READY_FD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1,
];

const _: () = {
    let mut index = 0;
    while index < SOURCE_COUNT_V1 {
        assert!(DESTINATION_FDS_V1[index] == index as i32);
        index += 1;
    }
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    InvalidDescriptor {
        role: &'static str,
        reason: &'static str,
    },
    DescriptorChanged(&'static str),
    DescriptorAlias(&'static str),
    Io {
        operation: &'static str,
        source: Errno,
    },
}

type Result<T> = std::result::Result<T, Failure>;

pub(super) fn source_role(index: usize) -> &'static str {
    const ROLES: [&str; SOURCE_COUNT_V1] = [
        "stdin",
        "stdout",
        "stderr",
        "issuer root",
        "rustc service peer",
        "rustc pidfd",
        "issuer policy",
        "issuer signing key",
        "service launch manifest",
        "readiness writer",
        "external-anchor peer",
        "external-anchor pidfd",
    ];
    ROLES[index]
}

pub(super) fn object_identity(descriptor: &impl AsFd, role: &'static str) -> Result<Object> {
    let stat = rustix::fs::fstat(descriptor).map_err(|source| Failure::Io {
        operation: "inspect protected launch object",
        source,
    })?;
    let size = u64::try_from(stat.st_size).map_err(|_| Failure::InvalidDescriptor {
        role,
        reason: "object size is negative",
    })?;
    Ok(Object::new(stat.st_dev, stat.st_ino, size, stat.st_mode))
}

pub(super) fn source_identities(
    sources: &[impl AsFd; SOURCE_COUNT_V1],
) -> Result<[Object; SOURCE_COUNT_V1]> {
    let mut identities = [Object::new(0, 0, 0, 0); SOURCE_COUNT_V1];
    for (index, source) in sources.iter().enumerate() {
        let object = object_identity(source, source_role(index))?;
        identities[index] = if matches!(
            index,
            CLIENT_PIDFD_SOURCE_INDEX | EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX
        ) {
            Object::new_process_pidfd(
                object.device(),
                object.inode(),
                object.size(),
                object.mode(),
            )
        } else {
            object
        };
    }
    Ok(identities)
}

/// Compares inert table facts only; canonical encoding and authority are caller concerns.
pub(super) fn validate_static_manifest_sources(
    executable: &Object,
    descriptors: &[Descriptor],
    issuer: &impl AsFd,
    sources: &[Object; SOURCE_COUNT_V1],
) -> Result<()> {
    if descriptors.len() != SOURCE_COUNT_V1
        || executable != &object_identity(issuer, "compiler issuer")?
        || descriptors
            .iter()
            .zip(DESTINATION_FDS_V1)
            .zip(sources)
            .enumerate()
            .any(|(index, ((entry, destination), source))| {
                entry.source_fd() != PREEXEC_SOURCE_FD_BASE + index as i32
                    || entry.destination_fd() != destination
                    || entry.object() != source
            })
    {
        return Err(Failure::DescriptorChanged("static pre-exec source table"));
    }
    Ok(())
}

pub(super) fn validate_manifest_metadata(file: &impl AsFd, expected_object: Object) -> Result<()> {
    let descriptor_flags = rustix::io::fcntl_getfd(file).map_err(|source| Failure::Io {
        operation: "inspect static manifest descriptor flags",
        source,
    })?;
    let status = rustix::fs::fcntl_getfl(file).map_err(|source| Failure::Io {
        operation: "inspect static manifest status flags",
        source,
    })?;
    let seals = rustix::fs::fcntl_get_seals(file).map_err(|source| Failure::Io {
        operation: "inspect static manifest seals",
        source,
    })?;
    let stat = rustix::fs::fstat(file).map_err(|source| Failure::Io {
        operation: "inspect static manifest object",
        source,
    })?;
    if !descriptor_flags.contains(FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || seals != REQUIRED_MANIFEST_SEALS_V1
        || FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != MANIFEST_MODE_V1
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_gid != rustix::process::getegid().as_raw()
        || stat.st_nlink != 0
        || stat.st_size != PREEXEC_MANIFEST_BYTES_V1 as i64
        || object_identity(file, "static pre-exec manifest")? != expected_object
    {
        return Err(Failure::InvalidDescriptor {
            role: "static pre-exec manifest",
            reason: "descriptor, object, access, mode, length, or seals changed",
        });
    }
    Ok(())
}

pub(super) fn protected_pipe(role: &'static str) -> Result<(OwnedFd, OwnedFd)> {
    let pair =
        pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(|source| Failure::Io {
            operation: "create protected issuer pipe",
            source,
        })?;
    validate_pipe_end(&pair.0, OFlags::RDONLY, role)?;
    validate_pipe_end(&pair.1, OFlags::WRONLY, role)?;
    Ok(pair)
}

pub(super) fn validate_pipe_end(
    descriptor: &impl AsFd,
    access: OFlags,
    role: &'static str,
) -> Result<()> {
    let flags = rustix::io::fcntl_getfd(descriptor).map_err(|source| Failure::Io {
        operation: "inspect protected pipe descriptor flags",
        source,
    })?;
    let status = rustix::fs::fcntl_getfl(descriptor).map_err(|source| Failure::Io {
        operation: "inspect protected pipe status flags",
        source,
    })?;
    let stat = rustix::fs::fstat(descriptor).map_err(|source| Failure::Io {
        operation: "inspect protected pipe object",
        source,
    })?;
    pipe_shape(flags, status, stat.st_mode, access, role)
}

fn pipe_shape(
    flags: FdFlags,
    status: OFlags,
    mode: u32,
    access: OFlags,
    role: &'static str,
) -> Result<()> {
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if !flags.contains(FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != access
        || !status.contains(OFlags::NONBLOCK)
        || status.intersects(forbidden)
        || FileType::from_raw_mode(mode) != FileType::Fifo
    {
        return Err(Failure::InvalidDescriptor {
            role,
            reason: "pipe type, access, status, or descriptor flags changed",
        });
    }
    Ok(())
}

pub(super) fn validate_pipe_pair(
    writer: &impl AsFd,
    reader: &impl AsFd,
    role: &'static str,
) -> Result<()> {
    validate_pipe_end(writer, OFlags::WRONLY, role)?;
    validate_pipe_end(reader, OFlags::RDONLY, role)?;
    if !same_object(
        &object_identity(writer, role)?,
        &object_identity(reader, role)?,
    ) {
        return Err(Failure::DescriptorChanged(role));
    }
    Ok(())
}

pub(super) fn validate_readiness_capacity(descriptor: &impl AsFd) -> Result<()> {
    if rustix::pipe::fcntl_getpipe_size(descriptor).map_err(|source| Failure::Io {
        operation: "inspect readiness pipe capacity",
        source,
    })? < fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_READY_BYTES_V1
    {
        return Err(Failure::InvalidDescriptor {
            role: "readiness writer",
            reason: "pipe capacity is smaller than one atomic readiness record",
        });
    }
    Ok(())
}

pub(super) fn require_launcher_non_aliasing(
    launcher: &impl AsFd,
    issuer: &impl AsFd,
    manifest: &impl AsFd,
    sources: &[impl AsFd; SOURCE_COUNT_V1],
) -> Result<()> {
    let launcher = object_identity(launcher, "static launcher")?;
    if same_object(&launcher, &object_identity(issuer, "compiler issuer")?)
        || same_object(
            &launcher,
            &object_identity(manifest, "static pre-exec manifest")?,
        )
        // Inspect all sources before reporting a source alias, preserving I/O-error precedence.
        || source_identities(sources)?
            .iter()
            .any(|source| same_object(&launcher, source))
    {
        return Err(Failure::DescriptorAlias(
            "static launcher aliases another launch role",
        ));
    }
    Ok(())
}

pub(super) const fn same_object(left: &Object, right: &Object) -> bool {
    left.device() == right.device() && left.inode() == right.inode()
}

#[cfg(test)]
#[path = "launch_checks_tests.rs"]
mod tests;
