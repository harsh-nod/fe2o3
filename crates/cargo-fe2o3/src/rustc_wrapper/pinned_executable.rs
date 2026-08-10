//! Fail-closed pinning for the native `rustc` executable.
//!
//! Linux is the only supported platform in this increment. The implementation opens the final
//! path component with `O_NOFOLLOW`, hashes through that opened descriptor, and executes through a
//! validated `/proc/self/fd` reference while retaining the descriptor. Other platforms return
//! [`PinExecutableError::UnsupportedPlatform`]; they must grow an equivalent descriptor-based
//! execution primitive rather than falling back to reopening the input pathname.
//!
//! This is an inert object/byte consistency primitive, not a trust-chain or execution-history
//! authenticator. Parent-directory symlinks, mutations of the opened inode by another writer, the
//! ELF interpreter, shared libraries, the codegen backend dynamic library, and the kernel/`procfs`
//! implementation remain outside this boundary. A protected supervisor is required to grant
//! authority over pre-backend loader history.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// A deliberately bounded read prevents a selected tool path from causing unbounded hashing work.
pub(crate) const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum PinExecutableError {
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
    NotExecutable {
        path: PathBuf,
        mode: u32,
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
    ExecutionStrategy {
        path: PathBuf,
        source: io::Error,
    },
    ExecutionObjectChanged {
        path: PathBuf,
    },
}

impl fmt::Display for PinExecutableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter
                .write_str("pinned executable preparation is supported only on Linux with procfs"),
            Self::Open { path, source } => {
                write!(
                    formatter,
                    "failed to open executable {}: {source}",
                    path.display()
                )
            }
            Self::Inspect { path, source } => write!(
                formatter,
                "failed to inspect opened executable {}: {source}",
                path.display()
            ),
            Self::NotRegular { path } => {
                write!(
                    formatter,
                    "executable is not a regular file: {}",
                    path.display()
                )
            }
            Self::NotExecutable { path, mode } => write!(
                formatter,
                "executable has no execute permission bits: {} (mode {mode:#o})",
                path.display()
            ),
            Self::Empty { path } => {
                write!(formatter, "executable is empty: {}", path.display())
            }
            Self::TooLarge { path, size, limit } => write!(
                formatter,
                "executable {} is {size} bytes, exceeding the {limit}-byte hashing limit",
                path.display()
            ),
            Self::Read { path, source } => write!(
                formatter,
                "failed while hashing executable {}: {source}",
                path.display()
            ),
            Self::UnexpectedEof {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "executable {} became shorter while hashing: expected {expected} bytes, read {actual}",
                path.display()
            ),
            Self::GrewDuringRead { path, expected } => write!(
                formatter,
                "executable {} grew beyond its initial {expected}-byte size while hashing",
                path.display()
            ),
            Self::ChangedDuringRead { path } => write!(
                formatter,
                "executable metadata changed while hashing: {}",
                path.display()
            ),
            Self::Rewind { path, source } => write!(
                formatter,
                "failed to rewind opened executable {} after hashing: {source}",
                path.display()
            ),
            Self::ExecutionStrategy { path, source } => write!(
                formatter,
                "fd-backed execution is unavailable for {}: {source}",
                path.display()
            ),
            Self::ExecutionObjectChanged { path } => write!(
                formatter,
                "pinned executable changed after hashing: {}",
                path.display()
            ),
        }
    }
}

impl Error for PinExecutableError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::Inspect { source, .. }
            | Self::Read { source, .. }
            | Self::Rewind { source, .. }
            | Self::ExecutionStrategy { source, .. } => Some(source),
            Self::UnsupportedPlatform
            | Self::NotRegular { .. }
            | Self::NotExecutable { .. }
            | Self::Empty { .. }
            | Self::TooLarge { .. }
            | Self::UnexpectedEof { .. }
            | Self::GrewDuringRead { .. }
            | Self::ChangedDuringRead { .. }
            | Self::ExecutionObjectChanged { .. } => None,
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{MAX_EXECUTABLE_BYTES, Path, PathBuf, PinExecutableError};
    use fe2o3_process_identity::LinuxObjectIdentityV3;
    use rustix::fs::{Access, Mode, OFlags};
    use sha2::{Digest, Sha256};
    use std::ffi::OsStr;
    use std::fs::{File, Metadata};
    use std::io::{self, Read, Seek, SeekFrom};
    use std::os::fd::AsRawFd;
    #[cfg(test)]
    use std::os::fd::RawFd;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, ExitStatus};

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

        fn same_execution_object(self, current: Self) -> bool {
            self.device == current.device
                && self.inode == current.inode
                && self.mode == current.mode
                && self.size == current.size
                && self.modified_seconds == current.modified_seconds
                && self.modified_nanoseconds == current.modified_nanoseconds
                && self.changed_seconds == current.changed_seconds
                && self.changed_nanoseconds == current.changed_nanoseconds
        }
    }

    /// A native executable held open from validation through command execution.
    pub(crate) struct PinnedExecutable {
        file: File,
        display_path: PathBuf,
        execution_path: PathBuf,
        snapshot: ObjectSnapshot,
        sha256: [u8; 32],
    }

    impl PinnedExecutable {
        pub(crate) fn open(path: &Path) -> Result<Self, PinExecutableError> {
            let display_path = path.to_path_buf();
            let fd = rustix::fs::open(
                path,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| PinExecutableError::Open {
                path: display_path.clone(),
                source: source.into(),
            })?;
            Self::from_open_file(File::from(fd), display_path)
        }

        pub(crate) fn from_transferred_file(
            file: File,
            display_path: PathBuf,
        ) -> Result<Self, PinExecutableError> {
            let flags = rustix::io::fcntl_getfd(&file).map_err(|source| {
                PinExecutableError::ExecutionStrategy {
                    path: display_path.clone(),
                    source: source.into(),
                }
            })?;
            if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
                return Err(PinExecutableError::ExecutionStrategy {
                    path: display_path,
                    source: io::Error::new(
                        io::ErrorKind::InvalidData,
                        "transferred executable observation is not close-on-exec",
                    ),
                });
            }
            Self::from_open_file(file, display_path)
        }

        pub(crate) fn try_clone_for_transfer(&self) -> Result<File, PinExecutableError> {
            self.file
                .try_clone()
                .map_err(|source| PinExecutableError::Inspect {
                    path: self.display_path.clone(),
                    source,
                })
        }

        fn from_open_file(
            mut file: File,
            display_path: PathBuf,
        ) -> Result<Self, PinExecutableError> {
            let initial_metadata =
                file.metadata()
                    .map_err(|source| PinExecutableError::Inspect {
                        path: display_path.clone(),
                        source,
                    })?;
            if !initial_metadata.is_file() {
                return Err(PinExecutableError::NotRegular { path: display_path });
            }

            let initial = ObjectSnapshot::from_metadata(&initial_metadata);
            if initial.mode & 0o111 == 0 {
                return Err(PinExecutableError::NotExecutable {
                    path: display_path,
                    mode: initial.mode,
                });
            }
            if initial.size == 0 {
                return Err(PinExecutableError::Empty { path: display_path });
            }
            if initial.size > MAX_EXECUTABLE_BYTES {
                return Err(PinExecutableError::TooLarge {
                    path: display_path,
                    size: initial.size,
                    limit: MAX_EXECUTABLE_BYTES,
                });
            }

            let sha256 = hash_exact(&mut file, &display_path, initial.size)?;
            let final_metadata = file
                .metadata()
                .map_err(|source| PinExecutableError::Inspect {
                    path: display_path.clone(),
                    source,
                })?;
            let snapshot = ObjectSnapshot::from_metadata(&final_metadata);
            if snapshot != initial {
                return Err(PinExecutableError::ChangedDuringRead { path: display_path });
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|source| PinExecutableError::Rewind {
                    path: display_path.clone(),
                    source,
                })?;

            let execution_path = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
            validate_execution_path(&file, &execution_path, snapshot, &display_path)?;

            Ok(Self {
                file,
                display_path,
                execution_path,
                snapshot,
                sha256,
            })
        }

        pub(crate) const fn sha256(&self) -> &[u8; 32] {
            &self.sha256
        }

        pub(crate) const fn size(&self) -> u64 {
            self.snapshot.size
        }

        pub(crate) const fn object_identity(&self) -> LinuxObjectIdentityV3 {
            LinuxObjectIdentityV3::from_linux_stat(
                self.snapshot.device,
                self.snapshot.inode,
                self.snapshot.mode,
            )
        }

        pub(crate) fn command(&self) -> Result<PinnedCommand<'_>, PinExecutableError> {
            let current = self
                .file
                .metadata()
                .map_err(|source| PinExecutableError::Inspect {
                    path: self.display_path.clone(),
                    source,
                })?;
            if !self
                .snapshot
                .same_execution_object(ObjectSnapshot::from_metadata(&current))
            {
                return Err(PinExecutableError::ExecutionObjectChanged {
                    path: self.display_path.clone(),
                });
            }
            validate_execution_path(
                &self.file,
                &self.execution_path,
                self.snapshot,
                &self.display_path,
            )?;

            let mut command = Command::new(&self.execution_path);
            command.arg0(&self.display_path);
            Ok(PinnedCommand {
                _executable: self,
                command,
                configured_argv0: self.display_path.as_os_str(),
            })
        }

        #[cfg(test)]
        pub(crate) fn raw_fd(&self) -> RawFd {
            self.file.as_raw_fd()
        }

        #[cfg(test)]
        fn execution_path(&self) -> &Path {
            &self.execution_path
        }
    }

    /// A command that cannot outlive the descriptor used by its executable pathname.
    pub(crate) struct PinnedCommand<'executable> {
        _executable: &'executable PinnedExecutable,
        command: Command,
        configured_argv0: &'executable OsStr,
    }

    impl PinnedCommand<'_> {
        pub(crate) const fn as_command(&self) -> &Command {
            &self.command
        }

        pub(crate) fn as_command_mut(&mut self) -> &mut Command {
            &mut self.command
        }

        pub(crate) const fn configured_argv0(&self) -> &OsStr {
            self.configured_argv0
        }

        pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<OsStr>,
        {
            self.command.args(args);
            self
        }

        pub(crate) fn status(&mut self) -> io::Result<ExitStatus> {
            self.command.status()
        }
    }

    fn hash_exact<R: Read>(
        reader: &mut R,
        display_path: &Path,
        expected_size: u64,
    ) -> Result<[u8; 32], PinExecutableError> {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; HASH_CHUNK_BYTES];
        let mut total = 0_u64;

        while total < expected_size {
            let remaining = expected_size - total;
            let requested = usize::try_from(remaining.min(HASH_CHUNK_BYTES as u64))
                .expect("hash chunk length fits usize");
            let read = read_retry(reader, &mut buffer[..requested]).map_err(|source| {
                PinExecutableError::Read {
                    path: display_path.to_path_buf(),
                    source,
                }
            })?;
            if read == 0 {
                return Err(PinExecutableError::UnexpectedEof {
                    path: display_path.to_path_buf(),
                    expected: expected_size,
                    actual: total,
                });
            }
            hasher.update(&buffer[..read]);
            total += read as u64;
        }

        if read_retry(reader, &mut buffer[..1]).map_err(|source| PinExecutableError::Read {
            path: display_path.to_path_buf(),
            source,
        })? != 0
        {
            return Err(PinExecutableError::GrewDuringRead {
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

    fn validate_execution_path(
        file: &File,
        execution_path: &Path,
        expected: ObjectSnapshot,
        display_path: &Path,
    ) -> Result<(), PinExecutableError> {
        let descriptor_metadata =
            file.metadata()
                .map_err(|source| PinExecutableError::Inspect {
                    path: display_path.to_path_buf(),
                    source,
                })?;
        let descriptor = ObjectSnapshot::from_metadata(&descriptor_metadata);
        if !expected.same_execution_object(descriptor) {
            return Err(PinExecutableError::ExecutionObjectChanged {
                path: display_path.to_path_buf(),
            });
        }

        let execution_metadata = std::fs::metadata(execution_path).map_err(|source| {
            PinExecutableError::ExecutionStrategy {
                path: display_path.to_path_buf(),
                source,
            }
        })?;
        let execution = ObjectSnapshot::from_metadata(&execution_metadata);
        if descriptor.device != execution.device || descriptor.inode != execution.inode {
            return Err(PinExecutableError::ExecutionObjectChanged {
                path: display_path.to_path_buf(),
            });
        }
        rustix::fs::access(execution_path, Access::EXEC_OK).map_err(|source| {
            PinExecutableError::ExecutionStrategy {
                path: display_path.to_path_buf(),
                source: source.into(),
            }
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::pinned_executable_test_directory::TestDirectory;
        use std::fs::{self, FileTimes};
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        use std::process::Stdio;
        use std::time::{Duration, Instant};

        fn write_executable(path: &Path, contents: &[u8]) {
            fs::write(path, contents).expect("write executable fixture");
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(path, permissions).expect("make fixture executable");
        }

        fn copy_executable(source: &Path, destination: &Path) {
            fs::copy(source, destination).expect("copy executable fixture");
            let mut permissions = fs::metadata(destination).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(destination, permissions).expect("make copy executable");
        }

        #[test]
        fn digest_is_computed_from_the_opened_object() {
            let root = TestDirectory::new();
            let path = root.path().join("tool");
            write_executable(&path, b"abc");

            let pinned = PinnedExecutable::open(&path).unwrap();

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
        fn exact_hash_policy_rejects_short_and_growing_streams() {
            let path = Path::new("fixture");
            let mut short = &b"abc"[..];
            assert!(matches!(
                hash_exact(&mut short, path, 4),
                Err(PinExecutableError::UnexpectedEof {
                    expected: 4,
                    actual: 3,
                    ..
                })
            ));

            let mut growing = &b"abc"[..];
            assert!(matches!(
                hash_exact(&mut growing, path, 2),
                Err(PinExecutableError::GrewDuringRead { expected: 2, .. })
            ));
        }

        #[test]
        fn final_symlink_is_rejected_without_following_it() {
            let root = TestDirectory::new();
            let target = root.path().join("target");
            let link = root.path().join("tool");
            write_executable(&target, b"abc");
            symlink(&target, &link).unwrap();

            assert!(matches!(
                PinnedExecutable::open(&link),
                Err(PinExecutableError::Open { .. })
            ));
        }

        #[test]
        fn non_regular_object_is_rejected() {
            let root = TestDirectory::new();

            assert!(matches!(
                PinnedExecutable::open(root.path()),
                Err(PinExecutableError::NotRegular { .. })
            ));
        }

        #[test]
        fn fifo_is_rejected_without_waiting_for_a_writer() {
            let root = TestDirectory::new();
            let fifo = root.path().join("fifo");
            rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, Mode::RUSR | Mode::WUSR | Mode::XUSR)
                .unwrap();

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
            let result = PinnedExecutable::open(&fifo);
            let elapsed = started.elapsed();
            delayed_writer.join().unwrap();

            assert!(matches!(result, Err(PinExecutableError::NotRegular { .. })));
            assert!(
                elapsed < Duration::from_millis(250),
                "opening a FIFO blocked for {elapsed:?}"
            );
        }

        #[test]
        fn regular_file_without_execute_bits_is_rejected() {
            let root = TestDirectory::new();
            let path = root.path().join("tool");
            fs::write(&path, b"abc").unwrap();
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(&path, permissions).unwrap();

            assert!(matches!(
                PinnedExecutable::open(&path),
                Err(PinExecutableError::NotExecutable { .. })
            ));
        }

        #[test]
        fn empty_executable_is_rejected() {
            let root = TestDirectory::new();
            let path = root.path().join("tool");
            write_executable(&path, b"");

            assert!(matches!(
                PinnedExecutable::open(&path),
                Err(PinExecutableError::Empty { .. })
            ));
        }

        #[test]
        fn oversized_file_is_rejected_before_it_is_read() {
            let root = TestDirectory::new();
            let path = root.path().join("tool");
            let file = File::create(&path).unwrap();
            file.set_len(MAX_EXECUTABLE_BYTES + 1).unwrap();
            drop(file);
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&path, permissions).unwrap();

            assert!(matches!(
                PinnedExecutable::open(&path),
                Err(PinExecutableError::TooLarge { .. })
            ));
        }

        #[test]
        fn retained_descriptor_prevents_path_trampoline() {
            let root = TestDirectory::new();
            let selected = root.path().join("rustc");
            let replacement = root.path().join("replacement");
            copy_executable(Path::new("/bin/true"), &selected);
            let pinned = PinnedExecutable::open(&selected).unwrap();
            let selected_digest = *pinned.sha256();

            copy_executable(Path::new("/bin/false"), &replacement);
            fs::rename(&replacement, &selected).expect("atomically replace selected pathname");

            let pinned_result = pinned.command();
            match pinned_result {
                Ok(mut command) => {
                    assert_eq!(command.as_command().get_program(), pinned.execution_path());
                    assert_eq!(command.configured_argv0(), selected.as_os_str());
                    assert!(
                        command.status().unwrap().success(),
                        "pinned command reopened replacement"
                    );
                }
                Err(PinExecutableError::ExecutionObjectChanged { .. }) => {}
                Err(error) => panic!("unexpected substitution result: {error}"),
            }
            assert_eq!(pinned.sha256(), &selected_digest);
            let replacement_bytes = fs::read(&selected).unwrap();
            assert_eq!(replacement_bytes, fs::read("/bin/false").unwrap());
            let replacement_digest: [u8; 32] = Sha256::digest(replacement_bytes).into();
            assert_ne!(pinned.sha256(), &replacement_digest);
        }

        #[test]
        fn same_size_mutation_with_restored_mtime_is_rejected_by_ctime() {
            let root = TestDirectory::new();
            let selected = root.path().join("rustc");
            copy_executable(Path::new("/bin/true"), &selected);
            let pinned = PinnedExecutable::open(&selected).unwrap();
            let original_metadata = fs::metadata(&selected).unwrap();
            let original_modified = original_metadata.modified().unwrap();
            let original_ctime = (original_metadata.ctime(), original_metadata.ctime_nsec());

            let mut bytes = fs::read(&selected).unwrap();
            bytes[0] ^= 0xff;
            let deadline = Instant::now() + Duration::from_secs(2);
            let changed_metadata = loop {
                fs::write(&selected, &bytes).unwrap();
                File::options()
                    .write(true)
                    .open(&selected)
                    .unwrap()
                    .set_times(FileTimes::new().set_modified(original_modified))
                    .unwrap();
                let metadata = fs::metadata(&selected).unwrap();
                if (metadata.ctime(), metadata.ctime_nsec()) != original_ctime {
                    break metadata;
                }
                assert!(
                    Instant::now() < deadline,
                    "fixture filesystem did not expose a ctime change"
                );
                std::thread::sleep(Duration::from_millis(20));
            };
            assert_eq!(changed_metadata.len(), original_metadata.len());
            assert_eq!(changed_metadata.mode(), original_metadata.mode());
            assert_eq!(changed_metadata.modified().unwrap(), original_modified);

            assert!(matches!(
                pinned.command(),
                Err(PinExecutableError::ExecutionObjectChanged { .. })
            ));
        }

        #[test]
        fn descriptor_lives_through_command_and_closes_with_pin() {
            let root = TestDirectory::new();
            let selected = root.path().join("rustc");
            copy_executable(Path::new("/bin/true"), &selected);
            let pinned = PinnedExecutable::open(&selected).unwrap();
            let descriptor_path = pinned.execution_path().to_path_buf();
            let descriptor = pinned.raw_fd();
            let pinned_metadata = fs::metadata(&descriptor_path).unwrap();

            {
                let mut command = pinned.command().unwrap();
                command
                    .args(std::iter::empty::<&str>())
                    .as_command_mut()
                    .stdin(Stdio::null());
                assert!(command.status().unwrap().success());
            }
            let after_command = fs::metadata(&descriptor_path).unwrap();
            assert_eq!(pinned_metadata.dev(), after_command.dev());
            assert_eq!(pinned_metadata.ino(), after_command.ino());

            drop(pinned);
            if let Ok(reused) = fs::metadata(format!("/proc/self/fd/{descriptor}")) {
                assert!(
                    reused.dev() != pinned_metadata.dev() || reused.ino() != pinned_metadata.ino(),
                    "dropping the pin left its executable descriptor open"
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[allow(unused_imports)] // Used by the parent when compile execution is activated.
pub(crate) use platform::PinnedExecutable;

#[cfg(not(target_os = "linux"))]
pub(crate) struct PinnedExecutable;

#[cfg(not(target_os = "linux"))]
impl PinnedExecutable {
    pub(crate) fn open(_path: &Path) -> Result<Self, PinExecutableError> {
        Err(PinExecutableError::UnsupportedPlatform)
    }
}
