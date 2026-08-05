//! Fail-closed object pinning for a future codegen-backend dynamic library.
//!
//! Linux is the only supported platform in this increment. The implementation opens the final
//! path component with `O_NOFOLLOW`, hashes through that opened descriptor, retains it, and
//! validates a `/proc/self/fd` reference to the same object. Other platforms return
//! [`PinCodegenBackendError::UnsupportedPlatform`]; they must not fall back to reopening the input
//! pathname.
//!
//! The descriptor remains `O_CLOEXEC`. [`BackendDescriptorReference::child_inheritance`] therefore
//! reports [`ChildDescriptorInheritance::BlockedByCloseOnExec`], and this module deliberately
//! provides no rustc command or dynamic-loading operation. A future increment must arrange and
//! verify race-free inheritance into the rustc child before compile execution can use the
//! descriptor-backed path.
//!
//! This primitive identifies bytes read from one regular-file object. Its digest is not an
//! authenticated trust or authority claim. Parent-directory resolution, in-place mutation by
//! another writer, dynamic-loader behavior, transitive shared dependencies, and the kernel/procfs
//! implementation remain outside this boundary.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Bounds hashing work for a selected codegen-backend object.
pub(crate) const MAX_CODEGEN_BACKEND_BYTES: u64 = 512 * 1024 * 1024;

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
            | Self::DescriptorStrategy { source, .. } => Some(source),
            Self::UnsupportedPlatform
            | Self::NotRegular { .. }
            | Self::Empty { .. }
            | Self::TooLarge { .. }
            | Self::UnexpectedEof { .. }
            | Self::GrewDuringRead { .. }
            | Self::ChangedDuringRead { .. }
            | Self::DescriptorObjectChanged { .. }
            | Self::DescriptorNotCloseOnExec { .. } => None,
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{
        ChildDescriptorInheritance, MAX_CODEGEN_BACKEND_BYTES, Path, PathBuf,
        PinCodegenBackendError,
    };
    use rustix::fs::{Mode, OFlags};
    use rustix::io::FdFlags;
    use sha2::{Digest, Sha256};
    use std::fs::{File, Metadata};
    use std::io::{self, Read, Seek, SeekFrom};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;

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
    }

    /// One opened codegen-backend object retained after validation.
    pub(crate) struct PinnedCodegenBackend {
        file: File,
        display_path: PathBuf,
        descriptor_path: PathBuf,
        snapshot: ObjectSnapshot,
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
            let mut file = File::from(fd);
            require_close_on_exec(&file, &display_path)?;

            let initial_metadata =
                file.metadata()
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

            let sha256 = hash_exact(&mut file, &display_path, initial.size)?;
            let final_metadata =
                file.metadata()
                    .map_err(|source| PinCodegenBackendError::Inspect {
                        path: display_path.clone(),
                        source,
                    })?;
            let snapshot = ObjectSnapshot::from_metadata(&final_metadata);
            if snapshot != initial {
                return Err(PinCodegenBackendError::ChangedDuringRead { path: display_path });
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|source| PinCodegenBackendError::Rewind {
                    path: display_path.clone(),
                    source,
                })?;

            let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
            validate_descriptor_path(&file, &descriptor_path, snapshot, &display_path)?;

            Ok(Self {
                file,
                display_path,
                descriptor_path,
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

        pub(crate) fn descriptor_reference(
            &self,
        ) -> Result<BackendDescriptorReference<'_>, PinCodegenBackendError> {
            require_close_on_exec(&self.file, &self.display_path)?;
            validate_descriptor_path(
                &self.file,
                &self.descriptor_path,
                self.snapshot,
                &self.display_path,
            )?;

            Ok(BackendDescriptorReference {
                _backend: self,
                path: &self.descriptor_path,
                child_inheritance: ChildDescriptorInheritance::BlockedByCloseOnExec,
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

    fn read_retry<R: Read>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match reader.read(buffer) {
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

    fn validate_descriptor_path(
        file: &File,
        descriptor_path: &Path,
        expected: ObjectSnapshot,
        display_path: &Path,
    ) -> Result<(), PinCodegenBackendError> {
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
        use std::fs::{self, FileTimes};
        use std::os::unix::fs::{MetadataExt, symlink};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{Duration, Instant};

        static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new() -> Self {
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
        fn exact_hash_policy_rejects_short_and_growing_streams() {
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
            let selected = root.path().join("backend.so");
            let replacement = root.path().join("replacement.so");
            let original = b"original backend bytes";
            let replacement_bytes = b"replacement backend bytes";
            fs::write(&selected, original).unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let original_digest = *pinned.sha256();

            fs::write(&replacement, replacement_bytes).unwrap();
            fs::rename(&replacement, &selected).unwrap();

            let descriptor = pinned.descriptor_reference().unwrap();
            assert_eq!(fs::read(descriptor.path()).unwrap(), original);
            assert_eq!(fs::read(&selected).unwrap(), replacement_bytes);
            assert_ne!(descriptor.path(), selected);
            assert!(descriptor.path().starts_with("/proc/self/fd"));
            assert_eq!(pinned.sha256(), &original_digest);
            let replacement_digest: [u8; 32] = Sha256::digest(replacement_bytes).into();
            assert_ne!(pinned.sha256(), &replacement_digest);
        }

        #[test]
        fn same_size_mutation_with_restored_mtime_is_rejected_by_ctime() {
            let root = TestDirectory::new();
            let selected = root.path().join("backend.so");
            fs::write(&selected, b"before").unwrap();
            let pinned = PinnedCodegenBackend::open(&selected).unwrap();
            let original_metadata = fs::metadata(&selected).unwrap();
            let original_modified = original_metadata.modified().unwrap();
            let original_ctime = (original_metadata.ctime(), original_metadata.ctime_nsec());

            std::thread::sleep(Duration::from_millis(10));
            fs::write(&selected, b"after!").unwrap();
            File::options()
                .write(true)
                .open(&selected)
                .unwrap()
                .set_times(FileTimes::new().set_modified(original_modified))
                .unwrap();

            let changed_metadata = fs::metadata(&selected).unwrap();
            assert_eq!(changed_metadata.len(), original_metadata.len());
            assert_eq!(changed_metadata.modified().unwrap(), original_modified);
            assert_ne!(
                (changed_metadata.ctime(), changed_metadata.ctime_nsec()),
                original_ctime,
                "fixture filesystem did not expose a ctime change"
            );
            assert!(matches!(
                pinned.descriptor_reference(),
                Err(PinCodegenBackendError::DescriptorObjectChanged { .. })
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
    }
}

#[cfg(target_os = "linux")]
#[allow(unused_imports)] // Used by the parent when compile execution is activated.
pub(crate) use platform::{BackendDescriptorReference, PinnedCodegenBackend};

#[cfg(not(target_os = "linux"))]
pub(crate) struct PinnedCodegenBackend;

#[cfg(not(target_os = "linux"))]
impl PinnedCodegenBackend {
    pub(crate) fn open(_path: &Path) -> Result<Self, PinCodegenBackendError> {
        Err(PinCodegenBackendError::UnsupportedPlatform)
    }
}
