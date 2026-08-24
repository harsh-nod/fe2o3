//! Fail-closed object pinning for a future codegen-backend dynamic library.
//!
//! Linux is the only supported platform in this increment. The implementation opens the final
//! path component with `O_NOFOLLOW`, copies and hashes its exact bounded contents into an anonymous
//! memfd, seals that image against mutation, and validates a `/proc/self/fd` reference to the
//! sealed object. Other platforms return
//! [`PinCodegenBackendError::UnsupportedPlatform`]; they must not fall back to reopening the input
//! pathname.
//!
//! The retained descriptor is read-only and remains `O_CLOEXEC` in the parent.
//! [`PinnedCodegenBackend::prepare_command`] validates its identity and seals again, appends the
//! exact descriptor-backed rustc option, and installs a child-only `pre_exec` step that verifies
//! the image and clears `FD_CLOEXEC`. The resulting command borrows the pin, so the descriptor
//! cannot be dropped before spawn. This module still provides no compile activation or
//! dynamic-loading operation.
//!
//! After successful exec, the descriptor is intentionally open in rustc and may remain visible to
//! descendants that rustc starts. A compile-activation design must define when rustc closes it or
//! restores `FD_CLOEXEC`; this primitive claims only that unrelated commands spawned by the parent
//! do not inherit it.
//!
//! The retained image is independent of later pathname replacement or source-inode mutation. Its
//! SHA-256 digest measures the origin bytes captured during `open`; it is unauthenticated and is
//! not a trust or authority claim. Parent-directory resolution, hostile mutation that defeats the
//! source metadata checks, dynamic-loader behavior, transitive shared dependencies, descendant
//! descriptor lifetime, and the kernel/procfs implementation remain outside this boundary.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::error::Error;
#[cfg(target_os = "linux")]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::{ExitStatus, Output};

/// Bounds hashing and sealed-image storage for a selected codegen-backend object.
///
/// Debug builds of the LLVM-backed compiler currently exceed 512 MiB. Keep this backend-specific
/// ceiling large enough to authenticate those builds while retaining a fixed pre-read resource
/// bound; copying and hashing still use fixed-size chunks.
pub(crate) const MAX_CODEGEN_BACKEND_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildDescriptorInheritance {
    /// The descriptor is intentionally close-on-exec and is not available to a rustc child.
    BlockedByCloseOnExec,
}

impl ChildDescriptorInheritance {
    pub(crate) const fn is_ready_for_rustc_child(self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) enum PinCodegenBackendError {
    #[allow(dead_code)] // Constructed by the non-Linux fail-closed implementation.
    UnsupportedPlatform,
    Open {
        path: PathBuf,
        source: io::Error,
    },
    Inspect {
        path: PathBuf,
        source: io::Error,
    },
    NotRegular {
        path: PathBuf,
    },
    Empty {
        path: PathBuf,
    },
    TooLarge {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
    Read {
        path: PathBuf,
        source: io::Error,
    },
    UnexpectedEof {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    GrewDuringRead {
        path: PathBuf,
        expected: u64,
    },
    ChangedDuringRead {
        path: PathBuf,
    },
    Rewind {
        path: PathBuf,
        source: io::Error,
    },
    CreateImage {
        path: PathBuf,
        source: io::Error,
    },
    WriteImage {
        path: PathBuf,
        source: io::Error,
    },
    ImageDigestMismatch {
        path: PathBuf,
    },
    SealImage {
        path: PathBuf,
        source: io::Error,
    },
    ImageSealsChanged {
        path: PathBuf,
    },
    DescriptorStrategy {
        path: PathBuf,
        source: io::Error,
    },
    DescriptorObjectChanged {
        path: PathBuf,
    },
    DescriptorNotCloseOnExec {
        path: PathBuf,
    },
    DescriptorNotReadOnly {
        path: PathBuf,
    },
    PreexistingCodegenBackendSelector {
        argument: OsString,
    },
    UninspectableResponseFile {
        argument: OsString,
    },
    OptionTerminator {
        argument: OsString,
    },
}

impl fmt::Display for PinCodegenBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str(
                "pinned codegen-backend preparation is supported only on Linux with procfs",
            ),
            Self::Open { path, source } => write!(
                formatter,
                "failed to open codegen backend {}: {source}",
                path.display()
            ),
            Self::Inspect { path, source } => write!(
                formatter,
                "failed to inspect opened codegen backend {}: {source}",
                path.display()
            ),
            Self::NotRegular { path } => write!(
                formatter,
                "codegen backend is not a regular file: {}",
                path.display()
            ),
            Self::Empty { path } => {
                write!(formatter, "codegen backend is empty: {}", path.display())
            }
            Self::TooLarge { path, size, limit } => write!(
                formatter,
                "codegen backend {} is {size} bytes, exceeding the {limit}-byte hashing limit",
                path.display()
            ),
            Self::Read { path, source } => write!(
                formatter,
                "failed while hashing codegen backend {}: {source}",
                path.display()
            ),
            Self::UnexpectedEof {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "codegen backend {} became shorter while hashing: expected {expected} bytes, read {actual}",
                path.display()
            ),
            Self::GrewDuringRead { path, expected } => write!(
                formatter,
                "codegen backend {} grew beyond its initial {expected}-byte size while hashing",
                path.display()
            ),
            Self::ChangedDuringRead { path } => write!(
                formatter,
                "codegen backend metadata changed while hashing: {}",
                path.display()
            ),
            Self::Rewind { path, source } => write!(
                formatter,
                "failed to rewind opened codegen backend {} after hashing: {source}",
                path.display()
            ),
            Self::CreateImage { path, source } => write!(
                formatter,
                "failed to create an anonymous image for codegen backend {}: {source}",
                path.display()
            ),
            Self::WriteImage { path, source } => write!(
                formatter,
                "failed to copy codegen backend {} into its anonymous image: {source}",
                path.display()
            ),
            Self::ImageDigestMismatch { path } => write!(
                formatter,
                "anonymous codegen-backend image does not match the captured bytes from {}",
                path.display()
            ),
            Self::SealImage { path, source } => write!(
                formatter,
                "failed to immutably seal the anonymous image for {}: {source}",
                path.display()
            ),
            Self::ImageSealsChanged { path } => write!(
                formatter,
                "anonymous codegen-backend image seals are missing or changed for {}",
                path.display()
            ),
            Self::DescriptorStrategy { path, source } => write!(
                formatter,
                "fd-backed codegen-backend access is unavailable for {}: {source}",
                path.display()
            ),
            Self::DescriptorObjectChanged { path } => write!(
                formatter,
                "pinned codegen-backend object changed after hashing: {}",
                path.display()
            ),
            Self::DescriptorNotCloseOnExec { path } => write!(
                formatter,
                "pinned codegen-backend descriptor unexpectedly became inheritable: {}",
                path.display()
            ),
            Self::DescriptorNotReadOnly { path } => write!(
                formatter,
                "pinned codegen-backend descriptor is unexpectedly writable: {}",
                path.display()
            ),
            Self::PreexistingCodegenBackendSelector { argument } => write!(
                formatter,
                "refusing command with a preexisting rustc codegen-backend selector: {}",
                argument.to_string_lossy()
            ),
            Self::UninspectableResponseFile { argument } => write!(
                formatter,
                "refusing rustc response-file argument while preparing the codegen backend: {}",
                argument.to_string_lossy()
            ),
            Self::OptionTerminator { argument } => write!(
                formatter,
                "refusing rustc option terminator before the managed codegen-backend selector: {}",
                argument.to_string_lossy()
            ),
        }
    }
}

impl Error for PinCodegenBackendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::Inspect { source, .. }
            | Self::Read { source, .. }
            | Self::Rewind { source, .. }
            | Self::CreateImage { source, .. }
            | Self::WriteImage { source, .. }
            | Self::SealImage { source, .. }
            | Self::DescriptorStrategy { source, .. } => Some(source),
            Self::UnsupportedPlatform
            | Self::NotRegular { .. }
            | Self::Empty { .. }
            | Self::TooLarge { .. }
            | Self::UnexpectedEof { .. }
            | Self::GrewDuringRead { .. }
            | Self::ChangedDuringRead { .. }
            | Self::ImageDigestMismatch { .. }
            | Self::ImageSealsChanged { .. }
            | Self::DescriptorObjectChanged { .. }
            | Self::DescriptorNotCloseOnExec { .. }
            | Self::DescriptorNotReadOnly { .. }
            | Self::PreexistingCodegenBackendSelector { .. }
            | Self::UninspectableResponseFile { .. } => None,
            Self::OptionTerminator { .. } => None,
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{
        ChildDescriptorInheritance, MAX_CODEGEN_BACKEND_BYTES, Path, PathBuf,
        PinCodegenBackendError,
    };
    use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
    use rustix::io::FdFlags;
    use sha2::{Digest, Sha256};
    use std::fs::{File, Metadata};
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd, RawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{FileExt, MetadataExt};
    use std::os::unix::process::CommandExt;

    use super::{Command, ExitStatus, OsStr, OsString, Output};
    use fe2o3_rustc_invocation::{
        is_rustc_codegen_backend_selector_v2, is_rustc_option_terminator_v2,
    };

    const HASH_CHUNK_BYTES: usize = 64 * 1024;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ObjectSnapshot {
        device: u64,
        inode: u64,
        mode: u32,
        size: u64,
        modified_seconds: i64,
        modified_nanoseconds: i64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
    }

    impl ObjectSnapshot {
        fn from_metadata(metadata: &Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                size: metadata.len(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            }
        }

        fn matches_stat(self, stat: &rustix::fs::Stat) -> bool {
            self.device == stat.st_dev
                && self.inode == stat.st_ino
                && self.mode == stat.st_mode
                && u64::try_from(stat.st_size).ok() == Some(self.size)
                && stat.st_mtime == self.modified_seconds
                && i64::try_from(stat.st_mtime_nsec).ok() == Some(self.modified_nanoseconds)
                && stat.st_ctime == self.changed_seconds
                && i64::try_from(stat.st_ctime_nsec).ok() == Some(self.changed_nanoseconds)
        }
    }

    fn rustc_backend_descriptor_path(descriptor: RawFd) -> PathBuf {
        // This pinned rustc treats a backend selector as a dylib path only when the value
        // contains a dot. The dotted procfs alias resolves to the same retained descriptor.
        PathBuf::from(format!("/proc/./self/fd/{descriptor}"))
    }

    /// One immutable anonymous image retained after validating its origin bytes.
    pub(crate) struct PinnedCodegenBackend {
        file: File,
        display_path: PathBuf,
        descriptor_path: PathBuf,
        snapshot: ObjectSnapshot,
        seals: SealFlags,
        /// An unauthenticated measurement of the captured source bytes.
        sha256: [u8; 32],
    }

    impl PinnedCodegenBackend {
        pub(crate) fn open(path: &Path) -> Result<Self, PinCodegenBackendError> {
            let display_path = path.to_path_buf();
            let fd = rustix::fs::open(
                path,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| PinCodegenBackendError::Open {
                path: display_path.clone(),
                source: source.into(),
            })?;
            let mut source_file = File::from(fd);
            require_close_on_exec(&source_file, &display_path)?;

            let initial_metadata =
                source_file
                    .metadata()
                    .map_err(|source| PinCodegenBackendError::Inspect {
                        path: display_path.clone(),
                        source,
                    })?;
            if !initial_metadata.is_file() {
                return Err(PinCodegenBackendError::NotRegular { path: display_path });
            }

            let initial = ObjectSnapshot::from_metadata(&initial_metadata);
            if initial.size == 0 {
                return Err(PinCodegenBackendError::Empty { path: display_path });
            }
            if initial.size > MAX_CODEGEN_BACKEND_BYTES {
                return Err(PinCodegenBackendError::TooLarge {
                    path: display_path,
                    size: initial.size,
                    limit: MAX_CODEGEN_BACKEND_BYTES,
                });
            }

            let (file, snapshot, seals, sha256) =
                capture_source(&mut source_file, &display_path, initial)?;
            require_read_only(&file, &display_path)?;

            let descriptor_path = rustc_backend_descriptor_path(file.as_raw_fd());
            validate_descriptor_path(&file, &descriptor_path, snapshot, seals, &display_path)?;
            Ok(Self {
                file,
                display_path,
                descriptor_path,
                snapshot,
                seals,
                sha256,
            })
        }

        #[allow(dead_code)] // Used by the Cargo orchestration binary, not the standalone wrapper.
        pub(crate) fn from_transferred_file(file: File) -> Result<Self, PinCodegenBackendError> {
            let display_path = PathBuf::from("<cargo-fe2o3 capability broker backend>");
            require_close_on_exec(&file, &display_path)?;
            require_read_only(&file, &display_path)?;
            let metadata = file
                .metadata()
                .map_err(|source| PinCodegenBackendError::Inspect {
                    path: display_path.clone(),
                    source,
                })?;
            if !metadata.is_file() {
                return Err(PinCodegenBackendError::NotRegular { path: display_path });
            }
            let snapshot = ObjectSnapshot::from_metadata(&metadata);
            if snapshot.size == 0 {
                return Err(PinCodegenBackendError::Empty { path: display_path });
            }
            if snapshot.size > MAX_CODEGEN_BACKEND_BYTES {
                return Err(PinCodegenBackendError::TooLarge {
                    path: display_path,
                    size: snapshot.size,
                    limit: MAX_CODEGEN_BACKEND_BYTES,
                });
            }
            let seals = rustix::fs::fcntl_get_seals(&file).map_err(|source| {
                PinCodegenBackendError::SealImage {
                    path: display_path.clone(),
                    source: source.into(),
                }
            })?;
            let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
            if seals != required && seals != required | SealFlags::FUTURE_WRITE {
                return Err(PinCodegenBackendError::ImageSealsChanged { path: display_path });
            }
            let sha256 = hash_exact_at(&file, &display_path, snapshot.size)?;
            let descriptor_path = rustc_backend_descriptor_path(file.as_raw_fd());
            validate_descriptor_path(&file, &descriptor_path, snapshot, seals, &display_path)?;
            Ok(Self {
                file,
                display_path,
                descriptor_path,
                snapshot,
                seals,
                sha256,
            })
        }

        #[allow(dead_code)] // Used by the Cargo orchestration binary, not the standalone wrapper.
        pub(crate) fn try_clone_for_transfer(&self) -> Result<File, PinCodegenBackendError> {
            self.descriptor_reference()?;
            self.file
                .try_clone()
                .map_err(|source| PinCodegenBackendError::DescriptorStrategy {
                    path: self.display_path.clone(),
                    source,
                })
        }

        /// Returns an unauthenticated SHA-256 measurement of the captured source bytes.
        pub(crate) const fn sha256(&self) -> &[u8; 32] {
            &self.sha256
        }

        pub(crate) const fn size(&self) -> u64 {
            self.snapshot.size
        }

        pub(crate) fn descriptor_reference(
            &self,
        ) -> Result<BackendDescriptorReference<'_>, PinCodegenBackendError> {
            require_close_on_exec(&self.file, &self.display_path)?;
            require_read_only(&self.file, &self.display_path)?;
            validate_descriptor_path(
                &self.file,
                &self.descriptor_path,
                self.snapshot,
                self.seals,
                &self.display_path,
            )?;

            Ok(BackendDescriptorReference {
                _backend: self,
                path: &self.descriptor_path,
                child_inheritance: ChildDescriptorInheritance::BlockedByCloseOnExec,
            })
        }

        /// Returns the stable procfs path used after the sealed image is installed at `target_fd`
        /// in one child. The target must be unused in the parent so the child setup cannot replace
        /// an unrelated capability.
        #[allow(dead_code)] // Used by the Cargo orchestration binary, not the standalone wrapper.
        pub(crate) fn fixed_child_descriptor_path(
            &self,
            target_fd: RawFd,
        ) -> Result<PathBuf, PinCodegenBackendError> {
            require_close_on_exec(&self.file, &self.display_path)?;
            require_read_only(&self.file, &self.display_path)?;
            require_unused_descriptor(target_fd, &self.display_path)?;
            Ok(rustc_backend_descriptor_path(target_fd))
        }

        /// Installs the exact sealed image at a stable descriptor in one child process.
        #[allow(dead_code)] // Used by the Cargo orchestration binary, not the standalone wrapper.
        pub(crate) fn inherit_for_child_at(
            &self,
            command: &mut Command,
            target_fd: RawFd,
        ) -> Result<(), PinCodegenBackendError> {
            require_close_on_exec(&self.file, &self.display_path)?;
            require_read_only(&self.file, &self.display_path)?;
            require_unused_descriptor(target_fd, &self.display_path)?;
            let source_fd = self.file.as_raw_fd();
            let snapshot = self.snapshot;
            let seals = self.seals;
            // SAFETY: the retained File remains alive through spawn. `dup2` and descriptor-only
            // validation are async-signal-safe operations in the child callback.
            unsafe {
                command.pre_exec(move || {
                    prepare_fixed_descriptor_in_child(source_fd, target_fd, snapshot, seals)
                });
            }
            Ok(())
        }

        /// Installs the sealed image at a stable descriptor in one child, replacing only the
        /// child's inherited entry at that number when Cargo kept it occupied.
        #[allow(dead_code)] // Used by the Cargo orchestration binary, not the standalone wrapper.
        pub(crate) fn replace_for_child_at(
            &self,
            command: &mut Command,
            target_fd: RawFd,
        ) -> Result<(), PinCodegenBackendError> {
            require_close_on_exec(&self.file, &self.display_path)?;
            require_read_only(&self.file, &self.display_path)?;
            if target_fd < 3 {
                return Err(PinCodegenBackendError::DescriptorStrategy {
                    path: self.display_path.clone(),
                    source: io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "fixed backend descriptor would replace a standard stream",
                    ),
                });
            }
            let source_fd = self.file.as_raw_fd();
            let snapshot = self.snapshot;
            let seals = self.seals;
            // SAFETY: the retained File remains alive through spawn. dup3 runs only in the child
            // and may replace a descriptor inherited through Cargo without changing the wrapper.
            unsafe {
                command.pre_exec(move || {
                    prepare_replacing_fixed_descriptor_in_child(
                        source_fd, target_fd, snapshot, seals,
                    )
                });
            }
            Ok(())
        }

        /// Prepare one command to pass this exact opened object to rustc.
        pub(crate) fn prepare_command(
            &self,
            mut command: Command,
        ) -> Result<PreparedCodegenBackendCommand<'_>, PinCodegenBackendError> {
            reject_preexisting_backend_selector(&command)?;
            require_close_on_exec(&self.file, &self.display_path)?;
            require_read_only(&self.file, &self.display_path)?;
            validate_descriptor_path(
                &self.file,
                &self.descriptor_path,
                self.snapshot,
                self.seals,
                &self.display_path,
            )?;

            let descriptor = self.file.as_raw_fd();
            let snapshot = self.snapshot;
            let seals = self.seals;
            // SAFETY: the callback uses only async-signal-safe descriptor syscalls. The retained
            // File and the returned command's borrow keep `descriptor` valid until spawn.
            unsafe {
                command.pre_exec(move || prepare_descriptor_in_child(descriptor, snapshot, seals));
            }

            let mut argument = OsString::from("-Zcodegen-backend=");
            argument.push(&self.descriptor_path);
            command.arg(&argument);

            Ok(PreparedCodegenBackendCommand {
                _backend: self,
                command,
                argument,
            })
        }
    }

    /// A descriptor path whose borrow cannot outlive the retained backend object.
    pub(crate) struct BackendDescriptorReference<'backend> {
        _backend: &'backend PinnedCodegenBackend,
        path: &'backend Path,
        child_inheritance: ChildDescriptorInheritance,
    }

    impl BackendDescriptorReference<'_> {
        pub(crate) const fn path(&self) -> &Path {
            self.path
        }

        pub(crate) const fn child_inheritance(&self) -> ChildDescriptorInheritance {
            self.child_inheritance
        }
    }

    /// A rustc command that cannot outlive its pinned codegen-backend descriptor.
    ///
    /// Configure all ordinary arguments before preparation. This type intentionally exposes no
    /// argument mutation, so its final and only codegen-backend selector cannot be overridden.
    pub(crate) struct PreparedCodegenBackendCommand<'backend> {
        _backend: &'backend PinnedCodegenBackend,
        command: Command,
        argument: OsString,
    }

    impl PreparedCodegenBackendCommand<'_> {
        pub(crate) fn codegen_backend_argument(&self) -> &OsStr {
            &self.argument
        }

        pub(crate) fn status(&mut self) -> io::Result<ExitStatus> {
            self.command.status()
        }

        pub(crate) fn output(&mut self) -> io::Result<Output> {
            self.command.output()
        }

        #[cfg(test)]
        fn command(&self) -> &Command {
            &self.command
        }
    }

    fn prepare_descriptor_in_child(
        descriptor: RawFd,
        expected: ObjectSnapshot,
        expected_seals: SealFlags,
    ) -> io::Result<()> {
        // SAFETY: the prepared command borrows the File that owns this descriptor through spawn.
        let descriptor = unsafe { BorrowedFd::borrow_raw(descriptor) };
        let flags = rustix::io::fcntl_getfd(descriptor).map_err(io::Error::from)?;
        if !flags.contains(FdFlags::CLOEXEC) {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::PERM.raw_os_error(),
            ));
        }

        let stat = rustix::fs::fstat(descriptor).map_err(io::Error::from)?;
        if !expected.matches_stat(&stat) {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::STALE.raw_os_error(),
            ));
        }

        let status_flags = rustix::fs::fcntl_getfl(descriptor).map_err(io::Error::from)?;
        if status_flags & OFlags::ACCMODE != OFlags::RDONLY {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::PERM.raw_os_error(),
            ));
        }

        let seals = rustix::fs::fcntl_get_seals(descriptor).map_err(io::Error::from)?;
        if seals != expected_seals {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::STALE.raw_os_error(),
            ));
        }

        let mut inherited_flags = flags;
        inherited_flags.remove(FdFlags::CLOEXEC);
        rustix::io::fcntl_setfd(descriptor, inherited_flags).map_err(io::Error::from)
    }

    fn require_unused_descriptor(
        descriptor: RawFd,
        display_path: &Path,
    ) -> Result<(), PinCodegenBackendError> {
        if descriptor < 3 {
            return Err(PinCodegenBackendError::DescriptorStrategy {
                path: display_path.to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "fixed backend descriptor would replace a standard stream",
                ),
            });
        }
        let path = PathBuf::from(format!("/proc/self/fd/{descriptor}"));
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(PinCodegenBackendError::DescriptorStrategy {
                path: display_path.to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("fixed backend descriptor {descriptor} is already in use"),
                ),
            }),
            Err(source) => Err(PinCodegenBackendError::DescriptorStrategy {
                path: display_path.to_path_buf(),
                source,
            }),
        }
    }

    fn prepare_fixed_descriptor_in_child(
        source_fd: RawFd,
        target_fd: RawFd,
        expected: ObjectSnapshot,
        expected_seals: SealFlags,
    ) -> io::Result<()> {
        // SAFETY: the prepared command borrows the File that owns this descriptor through spawn.
        let source = unsafe { BorrowedFd::borrow_raw(source_fd) };
        let installed =
            rustix::io::fcntl_dupfd_cloexec(source, target_fd).map_err(io::Error::from)?;
        if installed.as_raw_fd() != target_fd {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::BUSY.raw_os_error(),
            ));
        }
        let stat = rustix::fs::fstat(&installed).map_err(io::Error::from)?;
        let status_flags = rustix::fs::fcntl_getfl(&installed).map_err(io::Error::from)?;
        let seals = rustix::fs::fcntl_get_seals(&installed).map_err(io::Error::from)?;
        if !expected.matches_stat(&stat)
            || status_flags & OFlags::ACCMODE != OFlags::RDONLY
            || seals != expected_seals
        {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::STALE.raw_os_error(),
            ));
        }
        rustix::io::fcntl_setfd(&installed, FdFlags::empty()).map_err(io::Error::from)?;
        let _ = installed.into_raw_fd();
        Ok(())
    }

    fn prepare_replacing_fixed_descriptor_in_child(
        source_fd: RawFd,
        target_fd: RawFd,
        expected: ObjectSnapshot,
        expected_seals: SealFlags,
    ) -> io::Result<()> {
        if source_fd != target_fd
            && unsafe { libc::dup3(source_fd, target_fd, libc::O_CLOEXEC) } != target_fd
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: dup3 succeeded or the retained source already occupies the requested number.
        let installed = unsafe { BorrowedFd::borrow_raw(target_fd) };
        let stat = rustix::fs::fstat(installed).map_err(io::Error::from)?;
        let status_flags = rustix::fs::fcntl_getfl(installed).map_err(io::Error::from)?;
        let seals = rustix::fs::fcntl_get_seals(installed).map_err(io::Error::from)?;
        if !expected.matches_stat(&stat)
            || status_flags & OFlags::ACCMODE != OFlags::RDONLY
            || seals != expected_seals
        {
            return Err(io::Error::from_raw_os_error(
                rustix::io::Errno::STALE.raw_os_error(),
            ));
        }
        rustix::io::fcntl_setfd(installed, FdFlags::empty()).map_err(io::Error::from)
    }

    fn reject_preexisting_backend_selector(
        command: &Command,
    ) -> Result<(), PinCodegenBackendError> {
        let arguments = command.get_args().collect::<Vec<_>>();
        for (index, argument) in arguments.iter().enumerate() {
            let bytes = argument.as_bytes();
            if bytes.starts_with(b"@") {
                return Err(PinCodegenBackendError::UninspectableResponseFile {
                    argument: (*argument).to_os_string(),
                });
            }
            if is_rustc_option_terminator_v2(argument) {
                return Err(PinCodegenBackendError::OptionTerminator {
                    argument: (*argument).to_os_string(),
                });
            }
            if is_rustc_codegen_backend_selector_v2(argument, arguments.get(index + 1).copied()) {
                return Err(PinCodegenBackendError::PreexistingCodegenBackendSelector {
                    argument: (*argument).to_os_string(),
                });
            }
        }
        Ok(())
    }

    fn capture_source(
        source: &mut File,
        display_path: &Path,
        initial: ObjectSnapshot,
    ) -> Result<(File, ObjectSnapshot, SealFlags, [u8; 32]), PinCodegenBackendError> {
        let image_fd = rustix::fs::memfd_create(
            "fe2o3-codegen-backend",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|source| PinCodegenBackendError::CreateImage {
            path: display_path.to_path_buf(),
            source: source.into(),
        })?;
        let mut image = File::from(image_fd);
        require_close_on_exec(&image, display_path)?;

        let source_digest = copy_exact(source, &mut image, display_path, initial.size)?;
        let final_source = source
            .metadata()
            .map(|metadata| ObjectSnapshot::from_metadata(&metadata))
            .map_err(|source| PinCodegenBackendError::Inspect {
                path: display_path.to_path_buf(),
                source,
            })?;
        if final_source != initial {
            return Err(PinCodegenBackendError::ChangedDuringRead {
                path: display_path.to_path_buf(),
            });
        }

        image
            .seek(SeekFrom::Start(0))
            .map_err(|source| PinCodegenBackendError::Rewind {
                path: display_path.to_path_buf(),
                source,
            })?;
        let image_digest = hash_exact(&mut image, display_path, initial.size)?;
        if image_digest != source_digest {
            return Err(PinCodegenBackendError::ImageDigestMismatch {
                path: display_path.to_path_buf(),
            });
        }
        image
            .seek(SeekFrom::Start(0))
            .map_err(|source| PinCodegenBackendError::Rewind {
                path: display_path.to_path_buf(),
                source,
            })?;

        // `image` is the sole memfd descriptor and no mapping or duplicate has been created, so
        // applying `F_SEAL_WRITE` cannot be blocked by a writable alias. Retention uses a fresh
        // read-only descriptor after all seals have been verified.
        let seals = seal_image(&image, display_path)?;
        let snapshot = image
            .metadata()
            .map(|metadata| ObjectSnapshot::from_metadata(&metadata))
            .map_err(|source| PinCodegenBackendError::Inspect {
                path: display_path.to_path_buf(),
                source,
            })?;
        if snapshot.size != initial.size {
            return Err(PinCodegenBackendError::DescriptorObjectChanged {
                path: display_path.to_path_buf(),
            });
        }

        let writable_descriptor_path =
            PathBuf::from(format!("/proc/self/fd/{}", image.as_raw_fd()));
        validate_descriptor_path(
            &image,
            &writable_descriptor_path,
            snapshot,
            seals,
            display_path,
        )?;
        let read_only_fd = rustix::fs::open(
            &writable_descriptor_path,
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| PinCodegenBackendError::DescriptorStrategy {
            path: display_path.to_path_buf(),
            source: source.into(),
        })?;
        let read_only_image = File::from(read_only_fd);
        require_close_on_exec(&read_only_image, display_path)?;
        require_read_only(&read_only_image, display_path)?;
        let read_only_snapshot = read_only_image
            .metadata()
            .map(|metadata| ObjectSnapshot::from_metadata(&metadata))
            .map_err(|source| PinCodegenBackendError::Inspect {
                path: display_path.to_path_buf(),
                source,
            })?;
        if read_only_snapshot != snapshot {
            return Err(PinCodegenBackendError::DescriptorObjectChanged {
                path: display_path.to_path_buf(),
            });
        }
        require_exact_seals(&read_only_image, seals, display_path)?;
        drop(image);

        Ok((read_only_image, snapshot, seals, source_digest))
    }

    fn seal_image(image: &File, display_path: &Path) -> Result<SealFlags, PinCodegenBackendError> {
        let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK;
        let with_future_write = required | SealFlags::FUTURE_WRITE;
        let data_seals = match rustix::fs::fcntl_add_seals(image, with_future_write) {
            Ok(()) => with_future_write,
            Err(source) if source == rustix::io::Errno::INVAL => {
                let existing = rustix::fs::fcntl_get_seals(image).map_err(|source| {
                    PinCodegenBackendError::SealImage {
                        path: display_path.to_path_buf(),
                        source: source.into(),
                    }
                })?;
                if !existing.is_empty() {
                    return Err(PinCodegenBackendError::ImageSealsChanged {
                        path: display_path.to_path_buf(),
                    });
                }
                rustix::fs::fcntl_add_seals(image, required).map_err(|source| {
                    PinCodegenBackendError::SealImage {
                        path: display_path.to_path_buf(),
                        source: source.into(),
                    }
                })?;
                required
            }
            Err(source) => {
                return Err(PinCodegenBackendError::SealImage {
                    path: display_path.to_path_buf(),
                    source: source.into(),
                });
            }
        };

        rustix::fs::fcntl_add_seals(image, SealFlags::SEAL).map_err(|source| {
            PinCodegenBackendError::SealImage {
                path: display_path.to_path_buf(),
                source: source.into(),
            }
        })?;
        let expected = data_seals | SealFlags::SEAL;
        require_exact_seals(image, expected, display_path)?;
        Ok(expected)
    }

    fn require_exact_seals(
        image: &File,
        expected: SealFlags,
        display_path: &Path,
    ) -> Result<(), PinCodegenBackendError> {
        let actual = rustix::fs::fcntl_get_seals(image).map_err(|source| {
            PinCodegenBackendError::SealImage {
                path: display_path.to_path_buf(),
                source: source.into(),
            }
        })?;
        if actual != expected {
            return Err(PinCodegenBackendError::ImageSealsChanged {
                path: display_path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn copy_exact<R: Read, W: Write>(
        reader: &mut R,
        writer: &mut W,
        display_path: &Path,
        expected_size: u64,
    ) -> Result<[u8; 32], PinCodegenBackendError> {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; HASH_CHUNK_BYTES];
        let mut total = 0_u64;

        while total < expected_size {
            let remaining = expected_size - total;
            let requested = usize::try_from(remaining.min(HASH_CHUNK_BYTES as u64))
                .expect("copy chunk length fits usize");
            let read = read_retry(reader, &mut buffer[..requested]).map_err(|source| {
                PinCodegenBackendError::Read {
                    path: display_path.to_path_buf(),
                    source,
                }
            })?;
            if read == 0 {
                return Err(PinCodegenBackendError::UnexpectedEof {
                    path: display_path.to_path_buf(),
                    expected: expected_size,
                    actual: total,
                });
            }
            writer.write_all(&buffer[..read]).map_err(|source| {
                PinCodegenBackendError::WriteImage {
                    path: display_path.to_path_buf(),
                    source,
                }
            })?;
            hasher.update(&buffer[..read]);
            total += read as u64;
        }

        if read_retry(reader, &mut buffer[..1]).map_err(|source| PinCodegenBackendError::Read {
            path: display_path.to_path_buf(),
            source,
        })? != 0
        {
            return Err(PinCodegenBackendError::GrewDuringRead {
                path: display_path.to_path_buf(),
                expected: expected_size,
            });
        }

        Ok(hasher.finalize().into())
    }

    fn hash_exact<R: Read>(
        reader: &mut R,
        display_path: &Path,
        expected_size: u64,
    ) -> Result<[u8; 32], PinCodegenBackendError> {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; HASH_CHUNK_BYTES];
        let mut total = 0_u64;

        while total < expected_size {
            let remaining = expected_size - total;
            let requested = usize::try_from(remaining.min(HASH_CHUNK_BYTES as u64))
                .expect("hash chunk length fits usize");
            let read = read_retry(reader, &mut buffer[..requested]).map_err(|source| {
                PinCodegenBackendError::Read {
                    path: display_path.to_path_buf(),
                    source,
                }
            })?;
            if read == 0 {
                return Err(PinCodegenBackendError::UnexpectedEof {
                    path: display_path.to_path_buf(),
                    expected: expected_size,
                    actual: total,
                });
            }
            hasher.update(&buffer[..read]);
            total += read as u64;
        }

        if read_retry(reader, &mut buffer[..1]).map_err(|source| PinCodegenBackendError::Read {
            path: display_path.to_path_buf(),
            source,
        })? != 0
        {
            return Err(PinCodegenBackendError::GrewDuringRead {
                path: display_path.to_path_buf(),
                expected: expected_size,
            });
        }

        Ok(hasher.finalize().into())
    }

    fn hash_exact_at(
        file: &File,
        display_path: &Path,
        expected_size: u64,
    ) -> Result<[u8; 32], PinCodegenBackendError> {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; HASH_CHUNK_BYTES];
        let mut total = 0_u64;

        while total < expected_size {
            let remaining = expected_size - total;
            let requested = usize::try_from(remaining.min(HASH_CHUNK_BYTES as u64))
                .expect("hash chunk length fits usize");
            let read = read_at_retry(file, &mut buffer[..requested], total).map_err(|source| {
                PinCodegenBackendError::Read {
                    path: display_path.to_path_buf(),
                    source,
                }
            })?;
            if read == 0 {
                return Err(PinCodegenBackendError::UnexpectedEof {
                    path: display_path.to_path_buf(),
                    expected: expected_size,
                    actual: total,
                });
            }
            hasher.update(&buffer[..read]);
            total += read as u64;
        }

        if read_at_retry(file, &mut buffer[..1], expected_size).map_err(|source| {
            PinCodegenBackendError::Read {
                path: display_path.to_path_buf(),
                source,
            }
        })? != 0
        {
            return Err(PinCodegenBackendError::GrewDuringRead {
                path: display_path.to_path_buf(),
                expected: expected_size,
            });
        }

        Ok(hasher.finalize().into())
    }

    fn read_retry<R: Read>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match reader.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }

    fn read_at_retry(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
        loop {
            match file.read_at(buffer, offset) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }

    fn require_close_on_exec(
        file: &File,
        display_path: &Path,
    ) -> Result<(), PinCodegenBackendError> {
        let flags = rustix::io::fcntl_getfd(file).map_err(|source| {
            PinCodegenBackendError::DescriptorStrategy {
                path: display_path.to_path_buf(),
                source: source.into(),
            }
        })?;
        if !flags.contains(FdFlags::CLOEXEC) {
            return Err(PinCodegenBackendError::DescriptorNotCloseOnExec {
                path: display_path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn require_read_only(file: &File, display_path: &Path) -> Result<(), PinCodegenBackendError> {
        let flags = rustix::fs::fcntl_getfl(file).map_err(|source| {
            PinCodegenBackendError::DescriptorStrategy {
                path: display_path.to_path_buf(),
                source: source.into(),
            }
        })?;
        if flags & OFlags::ACCMODE != OFlags::RDONLY {
            return Err(PinCodegenBackendError::DescriptorNotReadOnly {
                path: display_path.to_path_buf(),
            });
        }
        Ok(())
    }

    fn validate_descriptor_path(
        file: &File,
        descriptor_path: &Path,
        expected: ObjectSnapshot,
        expected_seals: SealFlags,
        display_path: &Path,
    ) -> Result<(), PinCodegenBackendError> {
        require_exact_seals(file, expected_seals, display_path)?;
        let descriptor_metadata =
            file.metadata()
                .map_err(|source| PinCodegenBackendError::Inspect {
                    path: display_path.to_path_buf(),
                    source,
                })?;
        let descriptor = ObjectSnapshot::from_metadata(&descriptor_metadata);
        if descriptor != expected {
            return Err(PinCodegenBackendError::DescriptorObjectChanged {
                path: display_path.to_path_buf(),
            });
        }

        let procfs_metadata = std::fs::metadata(descriptor_path).map_err(|source| {
            PinCodegenBackendError::DescriptorStrategy {
                path: display_path.to_path_buf(),
                source,
            }
        })?;
        let procfs = ObjectSnapshot::from_metadata(&procfs_metadata);
        if procfs != descriptor {
            return Err(PinCodegenBackendError::DescriptorObjectChanged {
                path: display_path.to_path_buf(),
            });
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs::{self, OpenOptions};
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{Duration, Instant};

        static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new() -> Self {
                fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
                let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "cargo-fe2o3-pinned-codegen-backend-{}-{id}",
                    std::process::id()
                ));
                fs::create_dir(&path).expect("create pinned backend test directory");
                Self(path)
            }

            fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn digest_is_computed_from_exact_opened_bytes() {
            let root = TestDirectory::new();
            let path = root.path().join("backend.so");
            fs::write(&path, b"abc").unwrap();

            let pinned = PinnedCodegenBackend::open(&path).unwrap();

            assert_eq!(pinned.size(), 3);
            assert_eq!(
                pinned.sha256(),
                &[
                    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d,
                    0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10,
                    0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
                ]
            );
        }

        #[test]
        fn exact_copy_and_hash_policies_reject_short_and_growing_streams() {
            let path = Path::new("fixture");
            let mut short = &b"abc"[..];
            assert!(matches!(
                hash_exact(&mut short, path, 4),
                Err(PinCodegenBackendError::UnexpectedEof {
                    expected: 4,
                    actual: 3,
                    ..
                })
            ));

            let mut growing = &b"abc"[..];
            assert!(matches!(
                hash_exact(&mut growing, path, 2),
                Err(PinCodegenBackendError::GrewDuringRead { expected: 2, .. })
            ));

            let mut short = &b"abc"[..];
            let mut short_image = Vec::new();
            assert!(matches!(
                copy_exact(&mut short, &mut short_image, path, 4),
                Err(PinCodegenBackendError::UnexpectedEof {
                    expected: 4,
                    actual: 3,
                    ..
                })
            ));
            assert_eq!(short_image, b"abc");

            let mut growing = &b"abc"[..];
            let mut growing_image = Vec::new();
            assert!(matches!(
                copy_exact(&mut growing, &mut growing_image, path, 2),
                Err(PinCodegenBackendError::GrewDuringRead { expected: 2, .. })
            ));
            assert_eq!(growing_image, b"ab");
        }

        #[test]
        fn final_symlink_is_rejected_without_following_it() {
            let root = TestDirectory::new();
            let target = root.path().join("target.so");
            let link = root.path().join("backend.so");
            fs::write(&target, b"backend").unwrap();
            symlink(&target, &link).unwrap();

            assert!(matches!(
                PinnedCodegenBackend::open(&link),
                Err(PinCodegenBackendError::Open { .. })
            ));
        }

        #[test]
        fn non_regular_object_is_rejected() {
            let root = TestDirectory::new();

            assert!(matches!(
                PinnedCodegenBackend::open(root.path()),
                Err(PinCodegenBackendError::NotRegular { .. })
            ));
        }

        #[test]
        fn fifo_is_rejected_without_waiting_for_a_writer() {
            let root = TestDirectory::new();
            let fifo = root.path().join("backend.fifo");
            rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, Mode::RUSR | Mode::WUSR).unwrap();

            let writer_path = fifo.clone();
            let delayed_writer = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(400));
                let _ = rustix::fs::open(
                    &writer_path,
                    OFlags::WRONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                );
            });
            let started = Instant::now();
            let result = PinnedCodegenBackend::open(&fifo);
            let elapsed = started.elapsed();
            delayed_writer.join().unwrap();

            assert!(matches!(
                result,
                Err(PinCodegenBackendError::NotRegular { .. })
            ));
            assert!(
                elapsed < Duration::from_millis(250),
                "opening a FIFO blocked for {elapsed:?}"
            );
        }

        #[test]
        fn empty_backend_is_rejected() {
            let root = TestDirectory::new();
            let path = root.path().join("backend.so");
            fs::write(&path, b"").unwrap();

            assert!(matches!(
                PinnedCodegenBackend::open(&path),
                Err(PinCodegenBackendError::Empty { .. })
            ));
        }

        #[test]
        fn oversized_backend_is_rejected_before_it_is_read() {
            let root = TestDirectory::new();
            let path = root.path().join("backend.so");
            let file = File::create(&path).unwrap();
            file.set_len(MAX_CODEGEN_BACKEND_BYTES + 1).unwrap();
            drop(file);

            assert!(matches!(
                PinnedCodegenBackend::open(&path),
                Err(PinCodegenBackendError::TooLarge { .. })
            ));
        }

        #[test]
        fn pathname_substitution_does_not_change_the_descriptor_object() {
            let root = TestDirectory::new();
            let selected_parent = root.path().join("selected");
            let retained_parent = root.path().join("retained");
            fs::create_dir(&selected_parent).unwrap();
            let selected = selected_parent.join("backend.so");
            let original = b"original backend bytes";
            let replacement_bytes = b"replacement backend bytes";
            fs::write(&selected, original).unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let original_digest = *pinned.sha256();

            fs::rename(&selected_parent, &retained_parent).unwrap();
            fs::create_dir(&selected_parent).unwrap();
            fs::write(&selected, replacement_bytes).unwrap();

            let descriptor = pinned.descriptor_reference().unwrap();
            assert_eq!(fs::read(descriptor.path()).unwrap(), original);
            assert_eq!(fs::read(&selected).unwrap(), replacement_bytes);
            assert_ne!(descriptor.path(), selected);
            assert!(descriptor.path().starts_with("/proc/./self/fd"));
            assert_eq!(pinned.sha256(), &original_digest);
            let replacement_digest: [u8; 32] = Sha256::digest(replacement_bytes).into();
            assert_ne!(pinned.sha256(), &replacement_digest);
        }

        #[test]
        fn source_mutation_after_capture_cannot_change_the_sealed_image() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"before").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            fs::write(&selected, b"after!").unwrap();

            let descriptor = pinned.descriptor_reference().unwrap();
            assert_eq!(fs::read(descriptor.path()).unwrap(), b"before");
            assert_eq!(fs::read(&selected).unwrap(), b"after!");
        }

        #[test]
        fn source_mutation_after_initial_snapshot_fails_capture() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"before").unwrap();
            fs::set_permissions(&selected, fs::Permissions::from_mode(0o640)).unwrap();
            let fd = rustix::fs::open(
                &selected,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap();
            let mut source = File::from(fd);
            let initial = ObjectSnapshot::from_metadata(&source.metadata().unwrap());

            fs::write(&selected, b"after!").unwrap();
            fs::set_permissions(&selected, fs::Permissions::from_mode(0o600)).unwrap();

            assert!(matches!(
                capture_source(&mut source, &selected, initial),
                Err(PinCodegenBackendError::ChangedDuringRead { .. })
            ));
        }

        #[test]
        fn descriptor_lives_with_the_pin_and_closes_when_the_pin_drops() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let descriptor = pinned.file.as_raw_fd();
            let descriptor_path = pinned.descriptor_reference().unwrap().path().to_path_buf();
            let pinned_metadata = fs::metadata(&descriptor_path).unwrap();

            assert_eq!(fs::read(&descriptor_path).unwrap(), b"backend bytes");
            drop(pinned);

            if let Ok(reused) = fs::metadata(format!("/proc/self/fd/{descriptor}")) {
                assert!(
                    reused.dev() != pinned_metadata.dev() || reused.ino() != pinned_metadata.ino(),
                    "dropping the pin left its backend descriptor open"
                );
            }
        }

        #[test]
        fn procfs_reference_has_the_open_descriptor_identity() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let descriptor = pinned.descriptor_reference().unwrap();

            let opened = pinned.file.metadata().unwrap();
            let procfs = fs::metadata(descriptor.path()).unwrap();
            assert_eq!(procfs.dev(), opened.dev());
            assert_eq!(procfs.ino(), opened.ino());
            assert_eq!(procfs.len(), opened.len());
            assert_eq!(procfs.mtime(), opened.mtime());
            assert_eq!(procfs.mtime_nsec(), opened.mtime_nsec());
            assert_eq!(procfs.ctime(), opened.ctime());
            assert_eq!(procfs.ctime_nsec(), opened.ctime_nsec());
        }

        #[test]
        fn retained_image_has_exact_immutable_seals() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            let seals = rustix::fs::fcntl_get_seals(&pinned.file).unwrap();
            assert_eq!(seals, pinned.seals);
            assert!(seals.contains(SealFlags::WRITE));
            assert!(seals.contains(SealFlags::GROW));
            assert!(seals.contains(SealFlags::SHRINK));
            assert!(seals.contains(SealFlags::SEAL));
            assert!(
                seals == (SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL)
                    || seals
                        == (SealFlags::WRITE
                            | SealFlags::GROW
                            | SealFlags::SHRINK
                            | SealFlags::FUTURE_WRITE
                            | SealFlags::SEAL)
            );
        }

        #[test]
        fn retained_image_rejects_write_resize_and_new_seals() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            let original = b"backend bytes";
            fs::write(&selected, original).unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            let writable_alias = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&pinned.descriptor_path)
                .unwrap();
            assert!(rustix::io::write(&writable_alias, b"x").is_err());
            assert!(rustix::fs::ftruncate(&writable_alias, 1).is_err());
            assert!(rustix::fs::ftruncate(&writable_alias, original.len() as u64 + 1).is_err());
            assert!(rustix::fs::fcntl_add_seals(&writable_alias, SealFlags::EXEC).is_err());
            assert_eq!(fs::read(&pinned.descriptor_path).unwrap(), original);
            assert_eq!(
                rustix::fs::fcntl_get_seals(&pinned.file).unwrap(),
                pinned.seals
            );
        }

        #[test]
        fn retained_descriptor_is_read_only() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            require_read_only(&pinned.file, &selected).unwrap();
            let flags = rustix::fs::fcntl_getfl(&pinned.file).unwrap();
            assert_eq!(flags & OFlags::ACCMODE, OFlags::RDONLY);
        }

        #[test]
        fn procfs_cannot_create_a_write_capability_for_the_sealed_image() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            let original = b"backend bytes";
            fs::write(&selected, original).unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            if let Ok(mut alias) = OpenOptions::new().write(true).open(&pinned.descriptor_path) {
                assert!(alias.write_all(b"attacker").is_err());
            }
            assert_eq!(fs::read(&pinned.descriptor_path).unwrap(), original);
        }

        #[test]
        fn close_on_exec_is_explicitly_not_ready_for_child_inheritance() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let descriptor = pinned.descriptor_reference().unwrap();

            let flags = rustix::io::fcntl_getfd(&pinned.file).unwrap();
            assert!(flags.contains(FdFlags::CLOEXEC));
            assert_eq!(
                descriptor.child_inheritance(),
                ChildDescriptorInheritance::BlockedByCloseOnExec
            );
            assert!(!descriptor.child_inheritance().is_ready_for_rustc_child());
        }

        #[test]
        fn unexpectedly_inheritable_descriptor_fails_closed() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            rustix::io::fcntl_setfd(&pinned.file, FdFlags::empty()).unwrap();

            assert!(matches!(
                pinned.descriptor_reference(),
                Err(PinCodegenBackendError::DescriptorNotCloseOnExec { .. })
            ));
        }

        #[test]
        fn prepared_command_appends_the_exact_descriptor_argument() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            let prepared = pinned.prepare_command(Command::new("/bin/true")).unwrap();
            let expected = OsString::from(format!(
                "-Zcodegen-backend=/proc/./self/fd/{}",
                pinned.file.as_raw_fd()
            ));

            assert_eq!(prepared.codegen_backend_argument(), expected);
            assert_eq!(
                prepared.command().get_args().collect::<Vec<_>>(),
                vec![expected.as_os_str()]
            );
            assert_ne!(prepared.codegen_backend_argument(), selected.as_os_str());
        }

        #[test]
        fn preexisting_joined_backend_selectors_are_rejected() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            for selector in [
                OsString::from("-Zcodegen-backend=/tmp/attacker.so"),
                OsString::from("-Zcodegen-backend"),
                OsString::from("-Z=codegen-backend=/tmp/attacker.so"),
                OsString::from("-Z=codegen-backend"),
                OsString::from("-Zcodegen_backend=/tmp/attacker.so"),
                OsString::from("-Z=codegen_backend=/tmp/attacker.so"),
            ] {
                let mut command = Command::new("/bin/true");
                command.arg(&selector);
                assert!(matches!(
                    pinned.prepare_command(command),
                    Err(PinCodegenBackendError::PreexistingCodegenBackendSelector { .. })
                ));
            }

            use std::os::unix::ffi::OsStringExt;
            let mut command = Command::new("/bin/true");
            command.arg(OsString::from_vec(
                b"-Zcodegen-backend=/tmp/non-utf8-\xff.so".to_vec(),
            ));
            assert!(matches!(
                pinned.prepare_command(command),
                Err(PinCodegenBackendError::PreexistingCodegenBackendSelector { .. })
            ));
        }

        #[test]
        fn preexisting_split_backend_selectors_are_rejected() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            for value in [
                OsString::from("codegen-backend=/tmp/attacker.so"),
                OsString::from("codegen-backend"),
                OsString::from("codegen_backend=/tmp/attacker.so"),
                OsString::from("codegen_backend"),
            ] {
                let mut command = Command::new("/bin/true");
                command.args([OsStr::new("-Z"), value.as_os_str()]);
                assert!(matches!(
                    pinned.prepare_command(command),
                    Err(PinCodegenBackendError::PreexistingCodegenBackendSelector { .. })
                ));
            }
        }

        #[test]
        fn response_files_cannot_hide_a_second_backend_selector() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let mut command = Command::new("/bin/true");
            command.arg("@/tmp/uninspected-rustc-arguments");

            assert!(matches!(
                pinned.prepare_command(command),
                Err(PinCodegenBackendError::UninspectableResponseFile { .. })
            ));
        }

        #[test]
        fn option_terminator_cannot_hide_the_managed_backend_selector() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let mut command = Command::new("/bin/true");
            command.args(["--crate-name", "kernel", "--"]);

            assert!(matches!(
                pinned.prepare_command(command),
                Err(PinCodegenBackendError::OptionTerminator { .. })
            ));
        }

        #[test]
        fn non_backend_arguments_are_preserved_before_the_sole_selector() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let mut command = Command::new("/bin/true");
            command.args(["--crate-name", "kernel", "-Zunstable-options"]);

            let prepared = pinned.prepare_command(command).unwrap();
            let arguments = prepared.command().get_args().collect::<Vec<_>>();

            assert_eq!(
                &arguments[..3],
                ["--crate-name", "kernel", "-Zunstable-options"]
            );
            assert_eq!(
                arguments.last().copied(),
                Some(prepared.codegen_backend_argument())
            );
            assert_eq!(
                arguments
                    .iter()
                    .filter(|argument| argument.as_bytes().starts_with(b"-Zcodegen-backend"))
                    .count(),
                1
            );
        }

        #[test]
        fn child_reads_the_pinned_object_after_pathname_substitution() {
            let root = TestDirectory::new();
            let selected_parent = root.path().join("selected");
            let retained_parent = root.path().join("retained");
            fs::create_dir(&selected_parent).unwrap();
            let selected = selected_parent.join("backend.so");
            let original = b"original backend object";
            let replacement = b"replacement pathname object";
            fs::write(&selected, original).unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            let mut command = Command::new("/bin/sh");
            command.args([
                "-c",
                concat!(
                    "case \"$1\" in -Zcodegen-backend=/proc/./self/fd/*) ;; ",
                    "*) exit 91 ;; esac; ",
                    "path=${1#-Zcodegen-backend=}; cat \"$path\""
                ),
                "backend-probe",
            ]);
            let mut prepared = pinned.prepare_command(command).unwrap();

            fs::rename(&selected_parent, &retained_parent).unwrap();
            fs::create_dir(&selected_parent).unwrap();
            fs::write(&selected, replacement).unwrap();

            let output = prepared.output().unwrap();
            assert!(output.status.success(), "child stderr: {:?}", output.stderr);
            assert_eq!(output.stdout, original);
            assert_eq!(fs::read(&selected).unwrap(), replacement);
        }

        #[test]
        fn descriptor_stays_close_on_exec_in_parent_before_and_after_spawn() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            assert!(
                rustix::io::fcntl_getfd(&pinned.file)
                    .unwrap()
                    .contains(FdFlags::CLOEXEC)
            );
            let mut prepared = pinned.prepare_command(Command::new("/bin/true")).unwrap();
            assert!(
                rustix::io::fcntl_getfd(&pinned.file)
                    .unwrap()
                    .contains(FdFlags::CLOEXEC)
            );
            assert!(prepared.status().unwrap().success());
            assert!(
                rustix::io::fcntl_getfd(&pinned.file)
                    .unwrap()
                    .contains(FdFlags::CLOEXEC)
            );
        }

        #[test]
        fn in_place_source_mutation_after_preparation_cannot_affect_child() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            let original = b"backend bytes";
            fs::write(&selected, original).unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let mut command = Command::new("/bin/sh");
            command.args([
                "-c",
                "path=${1#-Zcodegen-backend=}; cat \"$path\"",
                "backend-probe",
            ]);
            let mut prepared = pinned.prepare_command(command).unwrap();

            fs::write(&selected, b"changed bytes").unwrap();

            let output = prepared.output().unwrap();
            assert!(output.status.success(), "child stderr: {:?}", output.stderr);
            assert_eq!(output.stdout, original);
        }

        #[test]
        fn source_mutation_before_preparation_cannot_affect_image() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();

            fs::write(&selected, b"changed bytes").unwrap();

            let mut prepared = pinned.prepare_command(Command::new("/bin/true")).unwrap();
            assert!(prepared.status().unwrap().success());
            assert_eq!(fs::read(&pinned.descriptor_path).unwrap(), b"backend bytes");
        }

        #[test]
        fn deliberate_child_descriptor_close_aborts_spawn() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let descriptor = pinned.file.as_raw_fd();
            let mut command = Command::new("/bin/true");
            // SAFETY: this callback runs only in the forked child. Closing its copy deliberately
            // exercises the prepared callback's fail-closed `EBADF` path.
            unsafe {
                command.pre_exec(move || {
                    rustix::io::close(descriptor);
                    Ok(())
                });
            }
            let mut prepared = pinned.prepare_command(command).unwrap();

            let error = prepared.status().unwrap_err();
            assert_eq!(
                error.raw_os_error(),
                Some(rustix::io::Errno::BADF.raw_os_error())
            );
            assert!(
                rustix::io::fcntl_getfd(&pinned.file)
                    .unwrap()
                    .contains(FdFlags::CLOEXEC)
            );
        }

        #[test]
        fn unrelated_children_do_not_inherit_the_backend_descriptor() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"backend bytes").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let descriptor_path = pinned.descriptor_path.clone();
            let _prepared = pinned.prepare_command(Command::new("/bin/true")).unwrap();

            let status = Command::new("/bin/sh")
                .args([
                    OsStr::new("-c"),
                    OsStr::new("test ! -e \"$1\""),
                    OsStr::new("backend-leak-probe"),
                    descriptor_path.as_os_str(),
                ])
                .status()
                .unwrap();
            assert!(status.success());
            assert!(
                rustix::io::fcntl_getfd(&pinned.file)
                    .unwrap()
                    .contains(FdFlags::CLOEXEC)
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[allow(unused_imports)] // Used by the parent when compile execution is activated.
pub(crate) use platform::{
    BackendDescriptorReference, PinnedCodegenBackend, PreparedCodegenBackendCommand,
};

#[cfg(not(target_os = "linux"))]
pub(crate) struct PinnedCodegenBackend;

#[cfg(not(target_os = "linux"))]
pub(crate) struct BackendDescriptorReference<'backend> {
    _backend: &'backend PinnedCodegenBackend,
}

#[cfg(not(target_os = "linux"))]
impl BackendDescriptorReference<'_> {
    pub(crate) fn path(&self) -> &Path {
        unreachable!("unsupported platforms cannot construct a backend descriptor reference")
    }
}

#[cfg(not(target_os = "linux"))]
impl PinnedCodegenBackend {
    pub(crate) fn open(_path: &Path) -> Result<Self, PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }

    pub(crate) fn prepare_command(
        &self,
        _command: Command,
    ) -> Result<PreparedCodegenBackendCommand<'_>, PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }

    pub(crate) fn from_transferred_file(
        _file: std::fs::File,
    ) -> Result<Self, PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }

    pub(crate) fn sha256(&self) -> &[u8; 32] {
        unreachable!("unsupported platforms cannot construct a pinned backend")
    }

    pub(crate) fn fixed_child_descriptor_path(
        &self,
        _target_fd: std::os::fd::RawFd,
    ) -> Result<PathBuf, PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }

    pub(crate) fn inherit_for_child_at(
        &self,
        _command: &mut Command,
        _target_fd: std::os::fd::RawFd,
    ) -> Result<(), PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }

    pub(crate) fn replace_for_child_at(
        &self,
        _command: &mut Command,
        _target_fd: std::os::fd::RawFd,
    ) -> Result<(), PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) struct PreparedCodegenBackendCommand<'backend> {
    _backend: &'backend PinnedCodegenBackend,
}
