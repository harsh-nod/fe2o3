#![deny(missing_docs, unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-protected-static-executable requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::PathBuf;

use fe2o3_runtime_protocol::{
    SealedStaticApplicationErrorV1, sealed_static_application_identity_v1,
};
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::process::{Gid, Uid};
use sha2::{Digest, Sha256};

const EXECUTABLE_MODE_V1: u32 = 0o555;
const REQUIRED_EXECUTABLE_SEALS_V1: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::EXEC)
    .union(SealFlags::SEAL);

/// Exact bounded SHA-256 measurement expected for one static executable image.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedStaticExecutableMeasurementV1 {
    sha256: [u8; 32],
    byte_len: u64,
    maximum_byte_len: u64,
}

impl ProtectedStaticExecutableMeasurementV1 {
    /// Constructs a nonzero measurement under one explicit trusted size ceiling.
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
        maximum_byte_len: u64,
    ) -> Result<Self, ProtectedStaticExecutableErrorV1> {
        if sha256 == [0; 32]
            || byte_len == 0
            || maximum_byte_len == 0
            || byte_len > maximum_byte_len
            || usize::try_from(byte_len).is_err()
        {
            return Err(ProtectedStaticExecutableErrorV1::InvalidMeasurement);
        }
        Ok(Self {
            sha256,
            byte_len,
            maximum_byte_len,
        })
    }

    /// Returns the complete expected SHA-256.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact expected image length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Returns the trusted maximum image length admitted at construction.
    pub const fn maximum_byte_len(self) -> u64 {
        self.maximum_byte_len
    }
}

/// Exact UID/GID ownership required for one sealed executable image.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedStaticExecutableOwnerV1 {
    uid: u32,
    gid: u32,
}

impl ProtectedStaticExecutableOwnerV1 {
    /// Constructs an exact kernel-representable owner identity.
    pub fn new(uid: u32, gid: u32) -> Result<Self, ProtectedStaticExecutableErrorV1> {
        if uid == u32::MAX || gid == u32::MAX {
            return Err(ProtectedStaticExecutableErrorV1::InvalidOwner);
        }
        Ok(Self { uid, gid })
    }

    /// Captures the current effective process identity.
    pub fn current() -> Self {
        Self {
            uid: rustix::process::geteuid().as_raw(),
            gid: rustix::process::getegid().as_raw(),
        }
    }

    /// Returns the exact owner UID.
    pub const fn uid(self) -> u32 {
        self.uid
    }

    /// Returns the exact owner GID.
    pub const fn gid(self) -> u32 {
        self.gid
    }
}

/// Stable kernel-visible object identity retained for a sealed executable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedStaticExecutableObjectIdentityV1 {
    device: u64,
    inode: u64,
    byte_len: u64,
    mode: u32,
    uid: u32,
    gid: u32,
}

impl ProtectedStaticExecutableObjectIdentityV1 {
    /// Returns the backing device number.
    pub const fn device(self) -> u64 {
        self.device
    }

    /// Returns the backing inode number.
    pub const fn inode(self) -> u64 {
        self.inode
    }

    /// Returns the exact image length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Returns the exact file type and permission bits.
    pub const fn mode(self) -> u32 {
        self.mode
    }

    /// Returns the exact owner UID.
    pub const fn uid(self) -> u32 {
        self.uid
    }

    /// Returns the exact owner GID.
    pub const fn gid(self) -> u32 {
        self.gid
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExecutableSnapshotV1 {
    object: ProtectedStaticExecutableObjectIdentityV1,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl ExecutableSnapshotV1 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            object: ProtectedStaticExecutableObjectIdentityV1 {
                device: metadata.dev(),
                inode: metadata.ino(),
                byte_len: metadata.len(),
                mode: metadata.mode(),
                uid: metadata.uid(),
                gid: metadata.gid(),
            },
            links: metadata.nlink(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

/// Move-only custody of one exact service-owned sealed static executable.
///
/// ```compile_fail
/// use fe2o3_protected_static_executable::ProtectedStaticExecutableV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProtectedStaticExecutableV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_protected_static_executable::ProtectedStaticExecutableV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ProtectedStaticExecutableV1>();
/// ```
pub struct ProtectedStaticExecutableV1 {
    image: File,
    snapshot: ExecutableSnapshotV1,
    measurement: ProtectedStaticExecutableMeasurementV1,
    owner: ProtectedStaticExecutableOwnerV1,
    static_identity: [u8; 32],
    role: &'static str,
}

impl fmt::Debug for ProtectedStaticExecutableV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedStaticExecutableV1")
            .field("role", &self.role)
            .field("measurement", &self.measurement)
            .field("owner", &self.owner)
            .field("object", &self.snapshot.object)
            .finish_non_exhaustive()
    }
}

impl ProtectedStaticExecutableV1 {
    /// Admits a provisioned source and seals its exact bytes for the requested owner.
    ///
    /// A caller may request another owner only while its effective UID is root. The returned memfd
    /// is already read-only, anonymous, mode 0555, fully sealed, and independently revalidated.
    pub fn seal_source_for_owner(
        source: File,
        measurement: ProtectedStaticExecutableMeasurementV1,
        owner: ProtectedStaticExecutableOwnerV1,
        role: &'static str,
    ) -> Result<Self, ProtectedStaticExecutableErrorV1> {
        require_owner_transition(owner, role)?;
        let before = validate_source(&source, measurement, role)?;
        let bytes = read_exact(&source, measurement, role)?;
        let after = snapshot(&source, "inspect static executable source after read")?;
        if before != after {
            return Err(ProtectedStaticExecutableErrorV1::SourceChanged(role));
        }
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != measurement.sha256 {
            return Err(ProtectedStaticExecutableErrorV1::MeasurementMismatch(role));
        }
        let static_identity = sealed_static_application_identity_v1(&bytes).map_err(|source| {
            ProtectedStaticExecutableErrorV1::InvalidStaticImage { role, source }
        })?;
        let image = create_sealed_image(&bytes, owner, role)?;
        let snapshot = validate_sealed_image(&image, measurement, owner, role)?;
        let admitted = Self {
            image,
            snapshot,
            measurement,
            owner,
            static_identity,
            role,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an existing read-only service-owned sealed executable descriptor.
    pub fn admit_sealed(
        image: File,
        measurement: ProtectedStaticExecutableMeasurementV1,
        owner: ProtectedStaticExecutableOwnerV1,
        role: &'static str,
    ) -> Result<Self, ProtectedStaticExecutableErrorV1> {
        let snapshot = validate_sealed_image(&image, measurement, owner, role)?;
        let bytes = read_exact(&image, measurement, role)?;
        let static_identity = sealed_static_application_identity_v1(&bytes).map_err(|source| {
            ProtectedStaticExecutableErrorV1::InvalidStaticImage { role, source }
        })?;
        let admitted = Self {
            image,
            snapshot,
            measurement,
            owner,
            static_identity,
            role,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits `/proc/self/exe` under the same sealed-image contract used by provisioning.
    pub fn admit_running(
        measurement: ProtectedStaticExecutableMeasurementV1,
        owner: ProtectedStaticExecutableOwnerV1,
        role: &'static str,
    ) -> Result<Self, ProtectedStaticExecutableErrorV1> {
        let image = File::open("/proc/self/exe").map_err(|source| {
            ProtectedStaticExecutableErrorV1::Io {
                operation: "open running protected static executable",
                source,
            }
        })?;
        Self::admit_sealed(image, measurement, owner, role)
    }

    /// Revalidates descriptor flags, object identity, ownership, seals, bytes, and static ELF form.
    pub fn revalidate(&self) -> Result<(), ProtectedStaticExecutableErrorV1> {
        let snapshot = validate_sealed_image(&self.image, self.measurement, self.owner, self.role)?;
        if snapshot != self.snapshot {
            return Err(ProtectedStaticExecutableErrorV1::Changed(self.role));
        }
        let bytes = read_exact(&self.image, self.measurement, self.role)?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.measurement.sha256
            || sealed_static_application_identity_v1(&bytes).map_err(|source| {
                ProtectedStaticExecutableErrorV1::InvalidStaticImage {
                    role: self.role,
                    source,
                }
            })? != self.static_identity
        {
            return Err(ProtectedStaticExecutableErrorV1::Changed(self.role));
        }
        Ok(())
    }

    /// Clones the exact same object as a close-on-exec descriptor for controlled `execveat` use.
    pub fn try_clone_for_exec(&self) -> Result<File, ProtectedStaticExecutableErrorV1> {
        self.revalidate()?;
        let image =
            self.image
                .try_clone()
                .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
                    operation: "clone protected static executable for exec",
                    source,
                })?;
        rustix::io::fcntl_setfd(&image, rustix::io::FdFlags::CLOEXEC).map_err(|source| {
            ProtectedStaticExecutableErrorV1::Io {
                operation: "protect cloned static executable descriptor",
                source: source.into(),
            }
        })?;
        if validate_sealed_image(&image, self.measurement, self.owner, self.role)? != self.snapshot
        {
            return Err(ProtectedStaticExecutableErrorV1::Changed(self.role));
        }
        Ok(image)
    }

    /// Revalidates an independently retained exec clone as the exact same sealed object.
    pub fn revalidate_exec_clone(
        &self,
        image: &File,
    ) -> Result<(), ProtectedStaticExecutableErrorV1> {
        self.revalidate()?;
        if validate_sealed_image(image, self.measurement, self.owner, self.role)? != self.snapshot {
            return Err(ProtectedStaticExecutableErrorV1::Changed(self.role));
        }
        Ok(())
    }

    /// Returns the trusted exact measurement.
    pub const fn measurement(&self) -> ProtectedStaticExecutableMeasurementV1 {
        self.measurement
    }

    /// Returns the loader-independent static application identity.
    pub const fn static_identity(&self) -> [u8; 32] {
        self.static_identity
    }

    /// Returns the retained kernel object identity without exposing descriptor custody.
    pub const fn object_identity(&self) -> ProtectedStaticExecutableObjectIdentityV1 {
        self.snapshot.object
    }
}

fn require_owner_transition(
    owner: ProtectedStaticExecutableOwnerV1,
    role: &'static str,
) -> Result<(), ProtectedStaticExecutableErrorV1> {
    let current = ProtectedStaticExecutableOwnerV1::current();
    if owner != current && current.uid != 0 {
        return Err(ProtectedStaticExecutableErrorV1::OwnerTransition(role));
    }
    Ok(())
}

fn validate_source(
    source: &File,
    measurement: ProtectedStaticExecutableMeasurementV1,
    role: &'static str,
) -> Result<ExecutableSnapshotV1, ProtectedStaticExecutableErrorV1> {
    let descriptor_flags =
        rustix::io::fcntl_getfd(source).map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "inspect static executable source descriptor flags",
            source: source.into(),
        })?;
    let status =
        rustix::fs::fcntl_getfl(source).map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "inspect static executable source status flags",
            source: source.into(),
        })?;
    let observed = snapshot(source, "inspect static executable source")?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || observed.object.mode & libc::S_IFMT != libc::S_IFREG
        || observed.object.mode & 0o111 == 0
        || observed.object.mode & (libc::S_ISUID | libc::S_ISGID) != 0
        || observed.object.byte_len != measurement.byte_len
    {
        return Err(ProtectedStaticExecutableErrorV1::InvalidSource(role));
    }
    require_no_file_capability(source, role, false)?;
    Ok(observed)
}

fn create_sealed_image(
    bytes: &[u8],
    owner: ProtectedStaticExecutableOwnerV1,
    role: &'static str,
) -> Result<File, ProtectedStaticExecutableErrorV1> {
    let descriptor = rustix::fs::memfd_create(
        c"fe2o3-protected-static-executable-v1",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::EXEC,
    )
    .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
        operation: "create protected static executable memfd",
        source: source.into(),
    })?;
    let mut writable = File::from(descriptor);
    writable
        .write_all(bytes)
        .and_then(|()| writable.flush())
        .and_then(|()| writable.sync_all())
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "populate protected static executable memfd",
            source,
        })?;
    if owner != ProtectedStaticExecutableOwnerV1::current() {
        rustix::fs::fchown(
            &writable,
            Some(Uid::from_raw(owner.uid)),
            Some(Gid::from_raw(owner.gid)),
        )
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "transfer protected static executable ownership",
            source: source.into(),
        })?;
    }
    rustix::fs::fchmod(
        &writable,
        Mode::RUSR | Mode::RGRP | Mode::ROTH | Mode::XUSR | Mode::XGRP | Mode::XOTH,
    )
    .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
        operation: "set protected static executable mode",
        source: source.into(),
    })?;
    let content_and_exec = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::EXEC;
    rustix::fs::fcntl_add_seals(&writable, content_and_exec)
        .and_then(|()| rustix::fs::fcntl_add_seals(&writable, SealFlags::SEAL))
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "seal protected static executable",
            source: source.into(),
        })?;
    let path = PathBuf::from(format!("/proc/self/fd/{}", writable.as_raw_fd()));
    let read_only = rustix::fs::open(&path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "bind read-only protected static executable",
            source: source.into(),
        })?;
    drop(writable);
    require_no_file_capability(&read_only, role, true)?;
    Ok(read_only)
}

fn validate_sealed_image(
    image: &File,
    measurement: ProtectedStaticExecutableMeasurementV1,
    owner: ProtectedStaticExecutableOwnerV1,
    role: &'static str,
) -> Result<ExecutableSnapshotV1, ProtectedStaticExecutableErrorV1> {
    let descriptor_flags =
        rustix::io::fcntl_getfd(image).map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "inspect sealed static executable descriptor flags",
            source: source.into(),
        })?;
    let status =
        rustix::fs::fcntl_getfl(image).map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "inspect sealed static executable status flags",
            source: source.into(),
        })?;
    let seals = rustix::fs::fcntl_get_seals(image).map_err(|source| {
        ProtectedStaticExecutableErrorV1::Io {
            operation: "inspect sealed static executable seals",
            source: source.into(),
        }
    })?;
    let observed = snapshot(image, "inspect sealed static executable object")?;
    let accepted_seals = seals == REQUIRED_EXECUTABLE_SEALS_V1
        || seals == REQUIRED_EXECUTABLE_SEALS_V1 | SealFlags::FUTURE_WRITE;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || !accepted_seals
        || observed.object.mode & libc::S_IFMT != libc::S_IFREG
        || observed.object.mode & 0o7777 != EXECUTABLE_MODE_V1
        || observed.object.uid != owner.uid
        || observed.object.gid != owner.gid
        || observed.links != 0
        || observed.object.byte_len != measurement.byte_len
    {
        return Err(ProtectedStaticExecutableErrorV1::InvalidSealedImage(role));
    }
    require_no_file_capability(image, role, true)?;
    let bytes = read_exact(image, measurement, role)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != measurement.sha256 {
        return Err(ProtectedStaticExecutableErrorV1::MeasurementMismatch(role));
    }
    sealed_static_application_identity_v1(&bytes)
        .map_err(|source| ProtectedStaticExecutableErrorV1::InvalidStaticImage { role, source })?;
    Ok(observed)
}

fn read_exact(
    image: &File,
    measurement: ProtectedStaticExecutableMeasurementV1,
    role: &'static str,
) -> Result<Vec<u8>, ProtectedStaticExecutableErrorV1> {
    let length = usize::try_from(measurement.byte_len)
        .map_err(|_| ProtectedStaticExecutableErrorV1::InvalidMeasurement)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ProtectedStaticExecutableErrorV1::InvalidMeasurement)?;
    bytes.resize(length, 0);
    image
        .read_exact_at(&mut bytes, 0)
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "read exact protected static executable",
            source,
        })?;
    let mut trailing = [0_u8; 1];
    if image
        .read_at(&mut trailing, measurement.byte_len)
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io {
            operation: "check protected static executable boundary",
            source,
        })?
        != 0
    {
        return Err(ProtectedStaticExecutableErrorV1::SourceChanged(role));
    }
    Ok(bytes)
}

fn snapshot(
    image: &File,
    operation: &'static str,
) -> Result<ExecutableSnapshotV1, ProtectedStaticExecutableErrorV1> {
    image
        .metadata()
        .map(|metadata| ExecutableSnapshotV1::from_metadata(&metadata))
        .map_err(|source| ProtectedStaticExecutableErrorV1::Io { operation, source })
}

fn require_no_file_capability(
    image: &File,
    role: &'static str,
    sealed: bool,
) -> Result<(), ProtectedStaticExecutableErrorV1> {
    let mut byte = 0_u8;
    match rustix::fs::fgetxattr(
        image,
        "security.capability",
        std::slice::from_mut(&mut byte),
    ) {
        Ok(_) | Err(rustix::io::Errno::RANGE) => {
            if sealed {
                Err(ProtectedStaticExecutableErrorV1::SealedFileCapability(role))
            } else {
                Err(ProtectedStaticExecutableErrorV1::SourceFileCapability(role))
            }
        }
        Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => Ok(()),
        Err(source) => Err(ProtectedStaticExecutableErrorV1::Io {
            operation: "inspect protected static executable file capability",
            source: source.into(),
        }),
    }
}

/// Stable failure admitting, sealing, or revalidating a protected static executable.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedStaticExecutableErrorV1 {
    /// The digest, length, or explicit trusted size bound is invalid.
    InvalidMeasurement,
    /// A UID or GID is the kernel sentinel value rather than an identity.
    InvalidOwner,
    /// A non-root caller requested an executable owned by another identity.
    OwnerTransition(&'static str),
    /// A provisioned source has invalid type, mode, access, descriptor flags, or size.
    InvalidSource(&'static str),
    /// A provisioned source carries a file capability.
    SourceFileCapability(&'static str),
    /// A provisioned source changed during exact admission.
    SourceChanged(&'static str),
    /// Exact bytes do not match trusted provisioning.
    MeasurementMismatch(&'static str),
    /// The image is outside the loader-independent static ELF profile.
    InvalidStaticImage {
        /// Image role being admitted.
        role: &'static str,
        /// Exact static-image validation failure.
        source: SealedStaticApplicationErrorV1,
    },
    /// A sealed image has invalid flags, type, mode, owner, links, seals, or size.
    InvalidSealedImage(&'static str),
    /// A sealed executable carries a file capability.
    SealedFileCapability(&'static str),
    /// The retained executable object or its exact bytes changed.
    Changed(&'static str),
    /// A bounded operating-system operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem error.
        source: io::Error,
    },
}

impl fmt::Display for ProtectedStaticExecutableErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMeasurement => {
                formatter.write_str("invalid static executable measurement")
            }
            Self::InvalidOwner => formatter.write_str("invalid static executable owner"),
            Self::OwnerTransition(role) => {
                write!(
                    formatter,
                    "cannot transfer {role} ownership from this identity"
                )
            }
            Self::InvalidSource(role) => write!(formatter, "invalid {role} source descriptor"),
            Self::SourceFileCapability(role) => {
                write!(formatter, "{role} source has a file capability")
            }
            Self::SourceChanged(role) => {
                write!(formatter, "{role} source changed during admission")
            }
            Self::MeasurementMismatch(role) => {
                write!(formatter, "{role} differs from its trusted measurement")
            }
            Self::InvalidStaticImage { role, source } => {
                write!(
                    formatter,
                    "{role} is not loader-independent static ELF: {source}"
                )
            }
            Self::InvalidSealedImage(role) => write!(formatter, "invalid sealed {role} image"),
            Self::SealedFileCapability(role) => {
                write!(formatter, "sealed {role} has a file capability")
            }
            Self::Changed(role) => write!(formatter, "retained sealed {role} changed"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedStaticExecutableErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidStaticImage { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    use super::*;

    const MAX_BYTES: u64 = 1024 * 1024;

    struct Fixture {
        _root: tempfile::TempDir,
        path: PathBuf,
        bytes: Vec<u8>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("static-entry");
            let bytes = static_elf();
            fs::write(&path, &bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
            Self {
                _root: root,
                path,
                bytes,
            }
        }

        fn measurement(&self) -> ProtectedStaticExecutableMeasurementV1 {
            ProtectedStaticExecutableMeasurementV1::new(
                Sha256::digest(&self.bytes).into(),
                self.bytes.len() as u64,
                MAX_BYTES,
            )
            .unwrap()
        }

        fn open(&self) -> File {
            File::open(&self.path).unwrap()
        }
    }

    #[test]
    fn exact_source_becomes_move_only_owned_sealed_static_image() {
        let fixture = Fixture::new();
        let admitted = ProtectedStaticExecutableV1::seal_source_for_owner(
            fixture.open(),
            fixture.measurement(),
            ProtectedStaticExecutableOwnerV1::current(),
            "test executable",
        )
        .unwrap();
        admitted.revalidate().unwrap();
        assert_eq!(admitted.measurement(), fixture.measurement());
        assert_ne!(admitted.static_identity(), [0; 32]);
        assert_eq!(admitted.object_identity().mode() & 0o7777, 0o555);
        assert_eq!(
            admitted.object_identity().uid(),
            rustix::process::geteuid().as_raw()
        );
        let cloned = admitted.try_clone_for_exec().unwrap();
        assert_eq!(
            rustix::fs::fcntl_getfl(&cloned).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        let seals = rustix::fs::fcntl_get_seals(&cloned).unwrap();
        assert!(
            seals == REQUIRED_EXECUTABLE_SEALS_V1
                || seals == REQUIRED_EXECUTABLE_SEALS_V1 | SealFlags::FUTURE_WRITE
        );
    }

    #[test]
    fn wrong_measurement_dynamic_image_and_hostile_source_flags_reject() {
        let fixture = Fixture::new();
        let mut digest = fixture.measurement().sha256();
        digest[0] ^= 1;
        let wrong = ProtectedStaticExecutableMeasurementV1::new(
            digest,
            fixture.bytes.len() as u64,
            MAX_BYTES,
        )
        .unwrap();
        assert!(matches!(
            ProtectedStaticExecutableV1::seal_source_for_owner(
                fixture.open(),
                wrong,
                ProtectedStaticExecutableOwnerV1::current(),
                "test executable",
            ),
            Err(ProtectedStaticExecutableErrorV1::MeasurementMismatch(
                "test executable"
            ))
        ));

        fs::set_permissions(&fixture.path, fs::Permissions::from_mode(0o755)).unwrap();
        let writable = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&fixture.path)
            .unwrap();
        assert!(matches!(
            ProtectedStaticExecutableV1::seal_source_for_owner(
                writable,
                fixture.measurement(),
                ProtectedStaticExecutableOwnerV1::current(),
                "test executable",
            ),
            Err(ProtectedStaticExecutableErrorV1::InvalidSource(
                "test executable"
            ))
        ));

        let inheritable = fs::OpenOptions::new()
            .read(true)
            .custom_flags(0)
            .open(&fixture.path)
            .unwrap();
        rustix::io::fcntl_setfd(&inheritable, rustix::io::FdFlags::empty()).unwrap();
        assert!(matches!(
            ProtectedStaticExecutableV1::seal_source_for_owner(
                inheritable,
                fixture.measurement(),
                ProtectedStaticExecutableOwnerV1::current(),
                "test executable",
            ),
            Err(ProtectedStaticExecutableErrorV1::InvalidSource(
                "test executable"
            ))
        ));

        let current = fs::read(std::env::current_exe().unwrap()).unwrap();
        let dynamic_path = fixture._root.path().join("dynamic-entry");
        fs::write(&dynamic_path, &current).unwrap();
        fs::set_permissions(&dynamic_path, fs::Permissions::from_mode(0o555)).unwrap();
        let measurement = ProtectedStaticExecutableMeasurementV1::new(
            Sha256::digest(&current).into(),
            current.len() as u64,
            current.len() as u64,
        )
        .unwrap();
        assert!(matches!(
            ProtectedStaticExecutableV1::seal_source_for_owner(
                File::open(dynamic_path).unwrap(),
                measurement,
                ProtectedStaticExecutableOwnerV1::current(),
                "dynamic executable",
            ),
            Err(ProtectedStaticExecutableErrorV1::InvalidStaticImage { .. })
        ));
    }

    #[test]
    fn invalid_bounds_and_owner_transition_reject() {
        assert!(matches!(
            ProtectedStaticExecutableMeasurementV1::new([0; 32], 1, 1),
            Err(ProtectedStaticExecutableErrorV1::InvalidMeasurement)
        ));
        assert!(matches!(
            ProtectedStaticExecutableOwnerV1::new(u32::MAX, 1),
            Err(ProtectedStaticExecutableErrorV1::InvalidOwner)
        ));
        if !rustix::process::geteuid().is_root() {
            let fixture = Fixture::new();
            let another = ProtectedStaticExecutableOwnerV1::new(
                rustix::process::geteuid().as_raw().wrapping_add(1),
                rustix::process::getegid().as_raw(),
            )
            .unwrap();
            assert!(matches!(
                ProtectedStaticExecutableV1::seal_source_for_owner(
                    fixture.open(),
                    fixture.measurement(),
                    another,
                    "test executable",
                ),
                Err(ProtectedStaticExecutableErrorV1::OwnerTransition(
                    "test executable"
                ))
            ));
        }
    }

    fn static_elf() -> Vec<u8> {
        const HEADER: usize = 64;
        const PROGRAM: usize = 56;
        const PROGRAMS: usize = 4;
        const CODE_OFFSET: usize = 0x1000;
        let mut bytes = vec![0_u8; CODE_OFFSET + 1];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
        bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
        bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
        bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
        bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());
        let table_size = (PROGRAM * PROGRAMS) as u64;
        write_program(
            &mut bytes,
            0,
            6,
            4,
            HEADER as u64,
            0x400040,
            table_size,
            table_size,
            8,
        );
        write_program(
            &mut bytes,
            1,
            1,
            4,
            0,
            0x400000,
            HEADER as u64 + table_size,
            HEADER as u64 + table_size,
            0x1000,
        );
        write_program(
            &mut bytes,
            2,
            1,
            5,
            CODE_OFFSET as u64,
            0x401000,
            1,
            1,
            0x1000,
        );
        write_program(&mut bytes, 3, 0x6474_e551, 6, 0, 0, 0, 0, 16);
        bytes[CODE_OFFSET] = 0xc3;
        bytes
    }

    #[allow(clippy::too_many_arguments)]
    fn write_program(
        bytes: &mut [u8],
        index: usize,
        kind: u32,
        flags: u32,
        offset: u64,
        virtual_address: u64,
        file_size: u64,
        memory_size: u64,
        alignment: u64,
    ) {
        const HEADER: usize = 64;
        const PROGRAM: usize = 56;
        let start = HEADER + index * PROGRAM;
        bytes[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[start + 16..start + 24].copy_from_slice(&virtual_address.to_le_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&file_size.to_le_bytes());
        bytes[start + 40..start + 48].copy_from_slice(&memory_size.to_le_bytes());
        bytes[start + 48..start + 56].copy_from_slice(&alignment.to_le_bytes());
    }
}
