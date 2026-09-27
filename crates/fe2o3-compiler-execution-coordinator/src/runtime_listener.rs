//! Shared bounded filesystem mechanics, without policy-family or compiler authority.
use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use rustix::net::{
    AddressFamily, SocketAddrAny, SocketAddrUnix, SocketFlags, SocketType, bind, socket_with,
};
use rustix::process::{Gid, Uid};
use std::{
    os::fd::{AsFd, AsRawFd, OwnedFd},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) enum RuntimeListenerError {
    Invalid {
        role: &'static str,
        reason: &'static str,
    },
    Io {
        operation: &'static str,
        source: rustix::io::Errno,
    },
}

impl From<RuntimeListenerError> for crate::CompilerExecutionCoordinatorErrorV1 {
    fn from(error: RuntimeListenerError) -> Self {
        match error {
            RuntimeListenerError::Invalid { role, reason } => {
                Self::ProvisionedInput { role, reason }
            }
            RuntimeListenerError::Io { operation, source } => Self::Io {
                operation,
                source: source.into(),
            },
        }
    }
}

pub(crate) struct RuntimeRoot {
    pub(crate) descriptor: OwnedFd,
    expected_path: PathBuf,
    expected_owner: u32,
    expected_group: u32,
    expected_mode: u32,
    snapshot: RuntimeFileSnapshot,
}

impl RuntimeRoot {
    pub(crate) fn admit(
        descriptor: OwnedFd,
        expected_path: &Path,
        expected_owner: u32,
        expected_group: u32,
        expected_mode: u32,
    ) -> Result<Self, RuntimeListenerError> {
        if !expected_path.is_absolute() || expected_path.as_os_str().as_encoded_bytes().len() > 107
        {
            return Err(invalid_provisioned(
                "runtime root",
                "expected pathname is not bounded and absolute",
            ));
        }
        let snapshot = validate_runtime_root(
            &descriptor,
            expected_path,
            expected_owner,
            expected_group,
            expected_mode,
        )?;
        Ok(Self {
            descriptor,
            expected_path: expected_path.to_owned(),
            expected_owner,
            expected_group,
            expected_mode,
            snapshot,
        })
    }

    pub(crate) fn revalidate(&self) -> Result<(), RuntimeListenerError> {
        let current = validate_runtime_root(
            &self.descriptor,
            &self.expected_path,
            self.expected_owner,
            self.expected_group,
            self.expected_mode,
        )?;
        if current.device != self.snapshot.device
            || current.inode != self.snapshot.inode
            || current.mode != self.snapshot.mode
            || current.uid != self.snapshot.uid
            || current.gid != self.snapshot.gid
            || current.links != self.snapshot.links
        {
            return Err(invalid_provisioned(
                "runtime root",
                "directory identity changed after admission",
            ));
        }
        Ok(())
    }
}

pub(crate) struct RuntimeListener {
    pub(crate) runtime_root: RuntimeRoot,
    pub(crate) descriptor: Option<OwnedFd>,
    socket_path: PathBuf,
    socket_entry: &'static str,
    path_identity: Option<(u64, u64)>,
    cleanup_armed: bool,
}

impl RuntimeListener {
    pub(crate) fn construct(
        runtime_root: RuntimeRoot,
        socket_path: &Path,
        socket_entry: &'static str,
        socket_owner: u32,
        socket_group: u32,
        socket_mode: u32,
    ) -> Result<Self, RuntimeListenerError> {
        if socket_path.as_os_str().as_encoded_bytes().len() > 107
            || socket_entry.len() > 107
            || socket_path.parent() != Some(runtime_root.expected_path.as_path())
            || socket_path.file_name() != Some(std::ffi::OsStr::new(socket_entry))
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "socket path is not the fixed runtime-root entry",
            ));
        }
        runtime_root.revalidate()?;
        require_runtime_entry_absent(&runtime_root.descriptor, socket_entry)?;

        let descriptor = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|source| runtime_io("create compiler-execution listener", source))?;
        let address = SocketAddrUnix::new(socket_path)
            .map_err(|source| runtime_io("encode compiler-execution listener pathname", source))?;
        bind(&descriptor, &address)
            .map_err(|source| runtime_io("bind compiler-execution listener", source))?;

        let mut constructed = Self {
            runtime_root,
            descriptor: Some(descriptor),
            socket_path: socket_path.to_owned(),
            socket_entry,
            path_identity: None,
            cleanup_armed: true,
        };
        let created = runtime_entry_snapshot(
            &constructed.runtime_root.descriptor,
            constructed.socket_entry,
            "inspect newly bound compiler-execution listener",
        )?;
        constructed.path_identity = Some((created.device, created.inode));

        let owner = (created.uid != socket_owner).then(|| Uid::from_raw(socket_owner));
        let group = (created.gid != socket_group).then(|| Gid::from_raw(socket_group));
        if owner.is_some() || group.is_some() {
            rustix::fs::chownat(
                &constructed.runtime_root.descriptor,
                constructed.socket_entry,
                owner,
                group,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|source| runtime_io("set compiler-execution listener ownership", source))?;
        }
        rustix::fs::chmodat(
            &constructed.runtime_root.descriptor,
            constructed.socket_entry,
            Mode::from_raw_mode(socket_mode),
            AtFlags::empty(),
        )
        .map_err(|source| runtime_io("set compiler-execution listener mode", source))?;

        constructed.revalidate(socket_owner, socket_group, socket_mode)?;
        Ok(constructed)
    }

    pub(crate) fn take_descriptor(&mut self) -> Result<OwnedFd, RuntimeListenerError> {
        self.descriptor
            .take()
            .ok_or_else(|| invalid_provisioned("listener", "descriptor already transferred"))
    }

    pub(crate) fn disarm_cleanup(mut self) {
        self.cleanup_armed = false;
    }

    pub(crate) fn revalidate(
        &self,
        socket_owner: u32,
        socket_group: u32,
        socket_mode: u32,
    ) -> Result<(), RuntimeListenerError> {
        self.runtime_root.revalidate()?;
        let descriptor = self
            .descriptor
            .as_ref()
            .ok_or_else(|| invalid_provisioned("listener", "descriptor already transferred"))?;
        let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
            .map_err(|source| runtime_io("inspect listener descriptor flags", source))?;
        let status = rustix::fs::fcntl_getfl(descriptor)
            .map_err(|source| runtime_io("inspect listener status flags", source))?;
        let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
        if descriptor_flags != rustix::io::FdFlags::CLOEXEC
            || status & OFlags::ACCMODE != OFlags::RDWR
            || !status.contains(OFlags::NONBLOCK)
            || status.intersects(forbidden)
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "descriptor flags are not exact nonblocking close-on-exec custody",
            ));
        }
        if rustix::net::sockopt::socket_domain(descriptor)
            .map_err(|source| runtime_io("inspect listener domain", source))?
            != AddressFamily::UNIX
            || rustix::net::sockopt::socket_type(descriptor)
                .map_err(|source| runtime_io("inspect listener type", source))?
                != SocketType::SEQPACKET
            || rustix::net::sockopt::socket_protocol(descriptor)
                .map_err(|source| runtime_io("inspect listener protocol", source))?
                .is_some()
            || rustix::net::sockopt::socket_acceptconn(descriptor)
                .map_err(|source| runtime_io("inspect listener state", source))?
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "endpoint is not a bound non-listening Unix SOCK_SEQPACKET socket",
            ));
        }
        let expected_address = SocketAddrAny::from(
            SocketAddrUnix::new(&self.socket_path)
                .map_err(|source| runtime_io("encode fixed listener pathname", source))?,
        );
        if rustix::net::getsockname(descriptor)
            .map_err(|source| runtime_io("inspect listener pathname", source))?
            != expected_address
            || socket_has_peer(descriptor)?
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "endpoint is not unconnected at the fixed pathname",
            ));
        }
        if rustix::net::sockopt::socket_error(descriptor)
            .map_err(|source| runtime_io("inspect listener socket error", source))?
            .is_err()
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "endpoint has a pending socket error",
            ));
        }
        let descriptor_stat = rustix::fs::fstat(descriptor)
            .map_err(|source| runtime_io("inspect listener descriptor", source))?;
        if FileType::from_raw_mode(descriptor_stat.st_mode) != FileType::Socket
            || descriptor_stat.st_nlink == 0
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "descriptor is not a live socket",
            ));
        }
        let path = runtime_entry_snapshot(
            &self.runtime_root.descriptor,
            self.socket_entry,
            "inspect compiler-execution listener pathname",
        )?;
        if FileType::from_raw_mode(path.mode) != FileType::Socket
            || path.mode & 0o7777 != socket_mode
            || path.uid != socket_owner
            || path.gid != socket_group
            || path.links != 1
            || path.byte_len != 0
            || self.path_identity != Some((path.device, path.inode))
        {
            return Err(invalid_provisioned(
                "compiler-execution listener",
                "pathname type, owner, group, mode, links, length, or identity is not exact",
            ));
        }
        require_no_runtime_entry_xattrs(
            &self.runtime_root.descriptor,
            self.socket_entry,
            "compiler-execution listener",
        )?;
        self.runtime_root.revalidate()
    }
}

impl Drop for RuntimeListener {
    fn drop(&mut self) {
        if !self.cleanup_armed {
            return;
        }
        let Ok(current) = rustix::fs::statat(
            &self.runtime_root.descriptor,
            self.socket_entry,
            AtFlags::SYMLINK_NOFOLLOW,
        ) else {
            return;
        };
        if owns_socket(self.path_identity, &current) {
            let _ = rustix::fs::unlinkat(
                &self.runtime_root.descriptor,
                self.socket_entry,
                AtFlags::empty(),
            );
        }
    }
}

fn owns_socket(identity: Option<(u64, u64)>, current: &rustix::fs::Stat) -> bool {
    identity == Some((current.st_dev, current.st_ino))
        && FileType::from_raw_mode(current.st_mode) == FileType::Socket
}

fn validate_runtime_root(
    descriptor: &OwnedFd,
    expected_path: &Path,
    expected_owner: u32,
    expected_group: u32,
    expected_mode: u32,
) -> Result<RuntimeFileSnapshot, RuntimeListenerError> {
    let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
        .map_err(|source| runtime_io("inspect runtime-root descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|source| runtime_io("inspect runtime-root status flags", source))?;
    let descriptor_snapshot = RuntimeFileSnapshot::from_stat(
        rustix::fs::fstat(descriptor)
            .map_err(|source| runtime_io("inspect runtime-root descriptor", source))?,
    );
    let path_snapshot = RuntimeFileSnapshot::from_stat(
        rustix::fs::lstat(expected_path)
            .map_err(|source| runtime_io("inspect fixed runtime-root pathname", source))?,
    );
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(descriptor_snapshot.mode) != FileType::Directory
        || descriptor_snapshot.mode & 0o7777 != expected_mode
        || descriptor_snapshot.uid != expected_owner
        || descriptor_snapshot.gid != expected_group
        || descriptor_snapshot.links == 0
        || descriptor_snapshot != path_snapshot
    {
        return Err(invalid_provisioned(
            "runtime root",
            "descriptor/path identity, access, owner, group, mode, or links is not exact",
        ));
    }
    require_no_descriptor_xattrs(descriptor, "runtime root")?;
    Ok(descriptor_snapshot)
}

fn require_runtime_entry_absent(
    runtime_root: &OwnedFd,
    entry: &str,
) -> Result<(), RuntimeListenerError> {
    match rustix::fs::statat(runtime_root, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(invalid_provisioned(
            "compiler-execution listener",
            "fixed pathname already exists",
        )),
        Err(source) => Err(runtime_io("inspect fixed listener pathname", source)),
    }
}

fn runtime_entry_snapshot(
    runtime_root: &OwnedFd,
    entry: &str,
    operation: &'static str,
) -> Result<RuntimeFileSnapshot, RuntimeListenerError> {
    rustix::fs::statat(runtime_root, entry, AtFlags::SYMLINK_NOFOLLOW)
        .map(RuntimeFileSnapshot::from_stat)
        .map_err(|source| runtime_io(operation, source))
}

fn require_no_descriptor_xattrs(
    descriptor: &impl AsFd,
    role: &'static str,
) -> Result<(), RuntimeListenerError> {
    let mut attributes = [0_u8; 1];
    match rustix::fs::flistxattr(descriptor, &mut attributes) {
        Ok(0) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => Err(invalid_provisioned(
            role,
            "object carries an extended attribute",
        )),
        Err(source) => Err(runtime_io(
            "inspect runtime-root extended attributes",
            source,
        )),
    }
}

fn require_no_runtime_entry_xattrs(
    runtime_root: &OwnedFd,
    entry: &str,
    role: &'static str,
) -> Result<(), RuntimeListenerError> {
    let path = format!("/proc/self/fd/{}/{entry}", runtime_root.as_raw_fd());
    let mut attributes = [0_u8; 1];
    match rustix::fs::llistxattr(path, &mut attributes) {
        Ok(0) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => Err(invalid_provisioned(
            role,
            "pathname carries an extended attribute",
        )),
        Err(source) => Err(runtime_io("inspect listener extended attributes", source)),
    }
}

fn socket_has_peer(descriptor: &OwnedFd) -> Result<bool, RuntimeListenerError> {
    match rustix::net::getpeername(descriptor) {
        Ok(peer) => Ok(peer.is_some()),
        Err(rustix::io::Errno::NOTCONN) => Ok(false),
        Err(source) => Err(runtime_io("inspect listener peer", source)),
    }
}

fn invalid_provisioned(role: &'static str, reason: &'static str) -> RuntimeListenerError {
    RuntimeListenerError::Invalid { role, reason }
}

fn runtime_io(operation: &'static str, source: rustix::io::Errno) -> RuntimeListenerError {
    RuntimeListenerError::Io { operation, source }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeFileSnapshot {
    pub(crate) device: u64,
    pub(crate) inode: u64,
    pub(crate) mode: u32,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) links: u64,
    pub(crate) byte_len: u64,
}

impl RuntimeFileSnapshot {
    pub(crate) fn from_stat(stat: rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
            byte_len: stat.st_size.try_into().unwrap_or(u64::MAX),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_requires_a_known_matching_socket_inode() {
        let file = tempfile::tempfile().unwrap();
        let mut stat = rustix::fs::fstat(&file).unwrap();
        let identity = (stat.st_dev, stat.st_ino);
        assert!(!owns_socket(Some(identity), &stat));
        // Exercise the cleanup predicate, not a fabricated admitted listener.
        stat.st_mode = FileType::Socket.as_raw_mode();
        assert!(owns_socket(Some(identity), &stat));
        assert!(!owns_socket(None, &stat));
        assert!(!owns_socket(Some((identity.0 ^ 1, identity.1)), &stat));
        assert!(!owns_socket(Some((identity.0, identity.1 ^ 1)), &stat));
    }

    #[test]
    fn runtime_paths_are_bounded_before_copying_or_filesystem_access() {
        let file = tempfile::tempfile().unwrap();
        let long_path = format!("/{}", "x".repeat(107));
        let error = RuntimeRoot::admit(file.into(), Path::new(&long_path), 0, 0, 0o755)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RuntimeListenerError::Invalid {
                role: "runtime root",
                ..
            }
        ));
    }
}
