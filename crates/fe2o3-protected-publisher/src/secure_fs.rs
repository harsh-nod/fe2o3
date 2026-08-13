use std::ffi::{CStr, CString, OsStr};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zeroize::Zeroizing;

use crate::PublisherError;

const MAX_LEDGER_INITIAL_HEADER_BYTES: usize = 4096;
const EMPTY_PATH: &CStr = c"";
const CURRENT_DIRECTORY: &CStr = c".";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) mode: u32,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) nlink: u64,
    pub(crate) size: i64,
    pub(crate) mtime: i64,
    pub(crate) mtime_nsec: i64,
    pub(crate) ctime: i64,
    pub(crate) ctime_nsec: i64,
}

impl FileIdentity {
    fn from_stat(stat: &libc::stat) -> Self {
        Self {
            dev: stat.st_dev,
            ino: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            nlink: stat.st_nlink,
            size: stat.st_size,
            mtime: stat.st_mtime,
            mtime_nsec: stat.st_mtime_nsec,
            ctime: stat.st_ctime,
            ctime_nsec: stat.st_ctime_nsec,
        }
    }

    pub(crate) fn is_regular(self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFREG
    }

    pub(crate) fn is_directory(self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFDIR
    }

    pub(crate) fn is_owner_only(self) -> bool {
        self.uid == unsafe { libc::geteuid() }
            && self.gid == unsafe { libc::getegid() }
            && self.mode & 0o077 == 0
    }

    fn same_stable_object(self, other: Self) -> bool {
        self.dev == other.dev
            && self.ino == other.ino
            && self.mode == other.mode
            && self.uid == other.uid
            && self.gid == other.gid
            && self.nlink == other.nlink
    }
}

pub(crate) struct SecureLocation {
    directory: File,
    directory_identity: FileIdentity,
    parent_path: PathBuf,
    name: CString,
}

impl SecureLocation {
    pub(crate) fn open(path: &Path) -> Result<Self, PublisherError> {
        if !path.is_absolute() {
            return Err(PublisherError::Config);
        }
        let parent = path.parent().ok_or(PublisherError::Config)?;
        let name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(PublisherError::Config)?;
        let name = component_name(name)?;
        let directory = open_directory_without_symlinks(parent)?;
        let directory_identity = fstat(directory.as_raw_fd())?;
        if !directory_identity.is_directory() || !directory_identity.is_owner_only() {
            return Err(PublisherError::Config);
        }
        Ok(Self {
            directory,
            directory_identity,
            parent_path: parent.to_owned(),
            name,
        })
    }

    pub(crate) fn open_existing_owner_only(
        &self,
        max_bytes: usize,
    ) -> Result<(File, FileIdentity), PublisherError> {
        self.open_existing_owner_only_after(max_bytes, || {})
    }

    fn open_existing_owner_only_after(
        &self,
        max_bytes: usize,
        after_lstat: impl FnOnce(),
    ) -> Result<(File, FileIdentity), PublisherError> {
        let before = self.entry_identity()?;
        validate_owner_file(before, max_bytes)?;
        after_lstat();
        let file = openat(
            self.directory.as_raw_fd(),
            &self.name,
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0,
        )?;
        let opened = fstat(file.as_raw_fd())?;
        let after = self.entry_identity()?;
        if before != opened || opened != after {
            return Err(PublisherError::Config);
        }
        Ok((file, opened))
    }

    pub(crate) fn read_existing_owner_only(
        &self,
        max_bytes: usize,
    ) -> Result<Vec<u8>, PublisherError> {
        let (mut file, opened) = self.open_existing_owner_only(max_bytes)?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| PublisherError::Config)?;
        if bytes.len() > max_bytes
            || fstat(file.as_raw_fd())? != opened
            || self.entry_identity()? != opened
            || !self
                .directory_identity()?
                .same_stable_object(self.directory_identity)
            || !self
                .path_directory_identity()?
                .same_stable_object(self.directory_identity)
        {
            return Err(PublisherError::Config);
        }
        Ok(bytes)
    }

    pub(crate) fn read_existing_owner_only_secret(
        &self,
        max_bytes: usize,
    ) -> Result<Zeroizing<Vec<u8>>, PublisherError> {
        let (mut file, opened) = self.open_existing_owner_only(max_bytes)?;
        let mut bytes = Zeroizing::new(Vec::with_capacity(max_bytes.saturating_add(1)));
        std::io::Read::by_ref(&mut file)
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| PublisherError::Config)?;
        if bytes.len() > max_bytes
            || fstat(file.as_raw_fd())? != opened
            || self.entry_identity()? != opened
            || !self
                .directory_identity()?
                .same_stable_object(self.directory_identity)
            || !self
                .path_directory_identity()?
                .same_stable_object(self.directory_identity)
        {
            return Err(PublisherError::Config);
        }
        Ok(bytes)
    }

    pub(crate) fn open_or_create_ledger(
        &self,
        header: &[u8],
    ) -> Result<(File, FileIdentity), PublisherError> {
        self.open_or_create_ledger_controlled(header, &LedgerInitControl::default())
    }

    fn open_or_create_ledger_controlled(
        &self,
        header: &[u8],
        control: &LedgerInitControl,
    ) -> Result<(File, FileIdentity), PublisherError> {
        if header.is_empty() || header.len() > MAX_LEDGER_INITIAL_HEADER_BYTES {
            return Err(PublisherError::Store);
        }
        if let Some(existing) = self.open_existing_ledger()? {
            self.sync()?;
            return Ok(existing);
        }
        self.verify_directory_for_ledger()?;
        let mut temporary = openat_raw(
            self.directory.as_raw_fd(),
            CURRENT_DIRECTORY,
            libc::O_RDWR | libc::O_CLOEXEC | libc::O_TMPFILE,
            0o600,
        )
        .map_err(|_| PublisherError::Store)?;
        let initial = fstat(temporary.as_raw_fd())?;
        if !initial.is_regular()
            || !initial.is_owner_only()
            || initial.mode & 0o777 != 0o600
            || initial.nlink != 0
            || initial.size != 0
        {
            return Err(PublisherError::Store);
        }
        write_initial_header(&mut temporary, header, control)?;
        control.stop(InitStage::BeforeTemporarySync)?;
        temporary.sync_data().map_err(|_| PublisherError::Store)?;
        control.stop(InitStage::AfterTemporarySync)?;
        let synced = fstat(temporary.as_raw_fd())?;
        if !synced.is_regular()
            || !synced.is_owner_only()
            || synced.mode & 0o777 != 0o600
            || synced.nlink != 0
            || synced.size != header.len() as i64
        {
            return Err(PublisherError::Store);
        }
        self.verify_directory_for_ledger()?;
        control.stop(InitStage::BeforePublish)?;
        if let Err(error) = publish_anonymous(
            temporary.as_raw_fd(),
            self.directory.as_raw_fd(),
            &self.name,
        ) {
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(PublisherError::Store);
            }
            self.sync()?;
            return self.open_existing_ledger()?.ok_or(PublisherError::Store);
        }
        control.stop(InitStage::AfterPublish)?;
        let published = fstat(temporary.as_raw_fd())?;
        let entry = self.entry_identity()?;
        if published != entry
            || published.nlink != 1
            || published.size != header.len() as i64
            || !published.is_regular()
            || !published.is_owner_only()
        {
            return Err(PublisherError::Store);
        }
        control.stop(InitStage::BeforeParentSync)?;
        self.sync()?;
        control.stop(InitStage::AfterParentSync)?;
        drop(temporary);
        self.open_existing_ledger()?.ok_or(PublisherError::Store)
    }

    fn open_existing_ledger(&self) -> Result<Option<(File, FileIdentity)>, PublisherError> {
        let before = match fstatat_raw(self.directory.as_raw_fd(), &self.name) {
            Ok(identity) => identity,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(PublisherError::Config),
        };
        validate_ledger_file(before)?;
        let file = openat(
            self.directory.as_raw_fd(),
            &self.name,
            libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0,
        )?;
        let opened = fstat(file.as_raw_fd())?;
        let after = self.entry_identity()?;
        if before != opened
            || opened != after
            || validate_ledger_file(opened).is_err()
            || !self
                .directory_identity()?
                .same_stable_object(self.directory_identity)
            || !self
                .path_directory_identity()?
                .same_stable_object(self.directory_identity)
        {
            return Err(PublisherError::Config);
        }
        Ok(Some((file, opened)))
    }

    pub(crate) fn verify_ledger_entry(&self, expected: FileIdentity) -> Result<(), PublisherError> {
        let current = self.entry_identity()?;
        if current.dev != expected.dev
            || current.ino != expected.ino
            || current.mode != expected.mode
            || current.uid != expected.uid
            || current.gid != expected.gid
            || current.nlink != expected.nlink
            || !current.is_regular()
            || !current.is_owner_only()
            || !self
                .directory_identity()?
                .same_stable_object(self.directory_identity)
            || !self
                .path_directory_identity()?
                .same_stable_object(self.directory_identity)
        {
            return Err(PublisherError::Store);
        }
        Ok(())
    }

    pub(crate) fn sync(&self) -> Result<(), PublisherError> {
        self.directory.sync_all().map_err(|_| PublisherError::Store)
    }

    fn verify_directory_for_ledger(&self) -> Result<(), PublisherError> {
        if !self
            .directory_identity()?
            .same_stable_object(self.directory_identity)
            || !self
                .path_directory_identity()?
                .same_stable_object(self.directory_identity)
        {
            return Err(PublisherError::Store);
        }
        Ok(())
    }

    fn entry_identity(&self) -> Result<FileIdentity, PublisherError> {
        fstatat(self.directory.as_raw_fd(), &self.name)
    }

    fn directory_identity(&self) -> Result<FileIdentity, PublisherError> {
        fstat(self.directory.as_raw_fd())
    }

    fn path_directory_identity(&self) -> Result<FileIdentity, PublisherError> {
        let directory = open_directory_without_symlinks(&self.parent_path)?;
        fstat(directory.as_raw_fd())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitStage {
    BeforeTemporarySync,
    AfterTemporarySync,
    BeforePublish,
    AfterPublish,
    BeforeParentSync,
    AfterParentSync,
}

struct LedgerInitControl {
    maximum_write_chunk: usize,
    fail_write_after: Option<usize>,
    stop_at: Option<InitStage>,
}

impl Default for LedgerInitControl {
    fn default() -> Self {
        Self {
            maximum_write_chunk: usize::MAX,
            fail_write_after: None,
            stop_at: None,
        }
    }
}

impl LedgerInitControl {
    fn stop(&self, stage: InitStage) -> Result<(), PublisherError> {
        if self.stop_at == Some(stage) {
            Err(PublisherError::Store)
        } else {
            Ok(())
        }
    }
}

fn write_initial_header(
    file: &mut File,
    header: &[u8],
    control: &LedgerInitControl,
) -> Result<(), PublisherError> {
    if control.maximum_write_chunk == 0 {
        return Err(PublisherError::Store);
    }
    let mut written = 0usize;
    while written < header.len() {
        if control
            .fail_write_after
            .is_some_and(|threshold| written >= threshold)
        {
            return Err(PublisherError::Store);
        }
        let end = header
            .len()
            .min(written.saturating_add(control.maximum_write_chunk));
        let count = match file.write(&header[written..end]) {
            Ok(0) => return Err(PublisherError::Store),
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(PublisherError::Store),
        };
        written = written.checked_add(count).ok_or(PublisherError::Store)?;
    }
    Ok(())
}

fn publish_anonymous(temporary: RawFd, directory: RawFd, name: &CStr) -> std::io::Result<()> {
    if unsafe {
        libc::linkat(
            temporary,
            EMPTY_PATH.as_ptr(),
            directory,
            name.as_ptr(),
            libc::AT_EMPTY_PATH,
        )
    } == 0
    {
        return Ok(());
    }
    let direct_error = std::io::Error::last_os_error();
    if direct_error.kind() == std::io::ErrorKind::AlreadyExists {
        return Err(direct_error);
    }
    if !matches!(
        direct_error.raw_os_error(),
        Some(libc::ENOENT | libc::EPERM | libc::EINVAL)
    ) {
        return Err(direct_error);
    }

    let descriptor_path = CString::new(format!("/proc/self/fd/{temporary}"))
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?;
    if unsafe {
        libc::linkat(
            libc::AT_FDCWD,
            descriptor_path.as_ptr(),
            directory,
            name.as_ptr(),
            libc::AT_SYMLINK_FOLLOW,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) fn read_owner_only(path: &Path, max_bytes: usize) -> Result<Vec<u8>, PublisherError> {
    SecureLocation::open(path)?.read_existing_owner_only(max_bytes)
}

pub(crate) fn read_owner_only_secret(
    path: &Path,
    max_bytes: usize,
) -> Result<Zeroizing<Vec<u8>>, PublisherError> {
    SecureLocation::open(path)?.read_existing_owner_only_secret(max_bytes)
}

pub(crate) fn write_new_owner_only(
    path: &Path,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), PublisherError> {
    static NONCE: AtomicU64 = AtomicU64::new(0);

    if bytes.len() > max_bytes {
        return Err(PublisherError::Config);
    }
    let location = SecureLocation::open(path)?;
    for _ in 0..64 {
        let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
        let temporary = CString::new(format!(
            ".fe2o3-publisher-{}-{nonce}.tmp",
            std::process::id()
        ))
        .map_err(|_| PublisherError::Config)?;
        let mut file = match openat_raw(
            location.directory.as_raw_fd(),
            &temporary,
            libc::O_WRONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_CREAT | libc::O_EXCL,
            0o600,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(PublisherError::Config),
        };
        let result = (|| {
            file.write_all(bytes).map_err(|_| PublisherError::Config)?;
            file.sync_all().map_err(|_| PublisherError::Config)?;
            if unsafe {
                libc::renameat2(
                    location.directory.as_raw_fd(),
                    temporary.as_ptr(),
                    location.directory.as_raw_fd(),
                    location.name.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            } != 0
            {
                return Err(PublisherError::Config);
            }
            location.sync().map_err(|_| PublisherError::Config)?;
            if location.read_existing_owner_only(max_bytes)? != bytes {
                return Err(PublisherError::Config);
            }
            Ok(())
        })();
        if result.is_err() {
            unsafe {
                libc::unlinkat(location.directory.as_raw_fd(), temporary.as_ptr(), 0);
            }
        }
        return result;
    }
    Err(PublisherError::Config)
}

fn validate_owner_file(identity: FileIdentity, max_bytes: usize) -> Result<(), PublisherError> {
    if !identity.is_regular()
        || !identity.is_owner_only()
        || identity.nlink != 1
        || identity.size < 0
        || identity.size as u64 > max_bytes as u64
    {
        return Err(PublisherError::Config);
    }
    Ok(())
}

fn validate_ledger_file(identity: FileIdentity) -> Result<(), PublisherError> {
    if !identity.is_regular()
        || !identity.is_owner_only()
        || identity.mode & 0o777 != 0o600
        || identity.nlink != 1
    {
        return Err(PublisherError::Config);
    }
    Ok(())
}

fn open_directory_without_symlinks(path: &Path) -> Result<File, PublisherError> {
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open("/")
        .map_err(|_| PublisherError::Config)?;
    for component in path.components() {
        let Component::Normal(component) = component else {
            if matches!(component, Component::RootDir) {
                continue;
            }
            return Err(PublisherError::Config);
        };
        let name = component_name(component)?;
        directory = openat(
            directory.as_raw_fd(),
            &name,
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            0,
        )?;
    }
    Ok(directory)
}

fn component_name(name: &OsStr) -> Result<CString, PublisherError> {
    let name = CString::new(name.as_bytes()).map_err(|_| PublisherError::Config)?;
    if name.as_bytes().is_empty() || name.as_bytes() == b"." || name.as_bytes() == b".." {
        return Err(PublisherError::Config);
    }
    Ok(name)
}

fn openat(
    directory: RawFd,
    name: &CStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> Result<File, PublisherError> {
    openat_raw(directory, name, flags, mode).map_err(|_| PublisherError::Config)
}

fn openat_raw(
    directory: RawFd,
    name: &CStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> std::io::Result<File> {
    let fd = unsafe { libc::openat(directory, name.as_ptr(), flags, mode) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn fstat(fd: RawFd) -> Result<FileIdentity, PublisherError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(PublisherError::Config);
    }
    Ok(FileIdentity::from_stat(&unsafe { stat.assume_init() }))
}

fn fstatat(directory: RawFd, name: &CStr) -> Result<FileIdentity, PublisherError> {
    fstatat_raw(directory, name).map_err(|_| PublisherError::Config)
}

fn fstatat_raw(directory: RawFd, name: &CStr) -> std::io::Result<FileIdentity> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            directory,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(FileIdentity::from_stat(&unsafe { stat.assume_init() }))
}

#[cfg(test)]
mod tests {
    use std::fs::{Permissions, hard_link};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::test_support::secure_tempdir;

    const TEST_LEDGER_HEADER: &[u8] = b"fe2o3-ledger-init-test-v1\n";

    #[test]
    fn owner_file_rejects_parent_symlinks_and_entry_links() {
        let temp = secure_tempdir();
        let directory = temp.path().join("secure");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("value");
        std::fs::write(&path, b"value").unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_owner_only(&path, 16).unwrap(), b"value");

        let parent_link = temp.path().join("parent-link");
        symlink(&directory, &parent_link).unwrap();
        assert!(read_owner_only(&parent_link.join("value"), 16).is_err());
        let entry_link = directory.join("entry-link");
        symlink(&path, &entry_link).unwrap();
        assert!(read_owner_only(&entry_link, 16).is_err());
        let hard = directory.join("hard");
        hard_link(&path, &hard).unwrap();
        assert!(read_owner_only(&path, 16).is_err());
    }

    #[test]
    fn ledger_entry_substitution_is_detected() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let location = SecureLocation::open(&path).unwrap();
        let (_file, identity) = location.open_or_create_ledger(TEST_LEDGER_HEADER).unwrap();
        let moved = temp.path().join("moved.db");
        std::fs::rename(&path, &moved).unwrap();
        std::fs::write(&path, b"replacement").unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        assert!(location.verify_ledger_entry(identity).is_err());
    }

    #[test]
    fn every_partial_initial_header_fails_without_a_named_inode() {
        let temp = secure_tempdir();
        for prefix in 0..TEST_LEDGER_HEADER.len() {
            let path = temp.path().join(format!("partial-{prefix}.ledger"));
            let location = SecureLocation::open(&path).unwrap();
            let control = LedgerInitControl {
                maximum_write_chunk: 1,
                fail_write_after: Some(prefix),
                stop_at: None,
            };
            assert!(
                location
                    .open_or_create_ledger_controlled(TEST_LEDGER_HEADER, &control)
                    .is_err()
            );
            assert!(!path.exists());
        }

        let path = temp.path().join("short-write-complete.ledger");
        let location = SecureLocation::open(&path).unwrap();
        let control = LedgerInitControl {
            maximum_write_chunk: 1,
            fail_write_after: None,
            stop_at: None,
        };
        location
            .open_or_create_ledger_controlled(TEST_LEDGER_HEADER, &control)
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), TEST_LEDGER_HEADER);
        assert!(std::fs::read_dir(temp.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".fe2o3-publisher")
        }));
    }

    #[test]
    fn initialization_crash_boundaries_leave_absent_or_complete_final() {
        let stages = [
            InitStage::BeforeTemporarySync,
            InitStage::AfterTemporarySync,
            InitStage::BeforePublish,
            InitStage::AfterPublish,
            InitStage::BeforeParentSync,
            InitStage::AfterParentSync,
        ];
        for stage in stages {
            let temp = secure_tempdir();
            let path = temp.path().join("publisher.ledger");
            let location = SecureLocation::open(&path).unwrap();
            let control = LedgerInitControl {
                maximum_write_chunk: 3,
                fail_write_after: None,
                stop_at: Some(stage),
            };
            assert!(
                location
                    .open_or_create_ledger_controlled(TEST_LEDGER_HEADER, &control)
                    .is_err()
            );
            let was_published = matches!(
                stage,
                InitStage::AfterPublish | InitStage::BeforeParentSync | InitStage::AfterParentSync
            );
            assert_eq!(path.exists(), was_published, "stage {stage:?}");
            if was_published {
                assert_eq!(std::fs::read(&path).unwrap(), TEST_LEDGER_HEADER);
            }
            location.open_or_create_ledger(TEST_LEDGER_HEADER).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), TEST_LEDGER_HEADER);
            assert!(std::fs::read_dir(temp.path()).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".fe2o3-publisher")
            }));
        }
    }

    #[test]
    fn initial_ledger_symlink_and_hardlink_are_rejected() {
        let temp = secure_tempdir();
        let target = temp.path().join("target");
        std::fs::write(&target, TEST_LEDGER_HEADER).unwrap();
        std::fs::set_permissions(&target, Permissions::from_mode(0o600)).unwrap();
        let path = temp.path().join("publisher.ledger");
        symlink(&target, &path).unwrap();
        assert!(
            SecureLocation::open(&path)
                .unwrap()
                .open_or_create_ledger(TEST_LEDGER_HEADER)
                .is_err()
        );
        std::fs::remove_file(&path).unwrap();
        hard_link(&target, &path).unwrap();
        assert!(
            SecureLocation::open(&path)
                .unwrap()
                .open_or_create_ledger(TEST_LEDGER_HEADER)
                .is_err()
        );
    }

    #[test]
    #[ignore = "subprocess helper invoked by concurrent_process_initializers_publish_one_header"]
    fn ledger_initializer_process_child() {
        let path = PathBuf::from(std::env::var_os("FE2O3_LEDGER_INIT_PATH").unwrap());
        let start = PathBuf::from(std::env::var_os("FE2O3_LEDGER_INIT_START").unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !start.exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let location = SecureLocation::open(&path).unwrap();
        let (mut file, identity) = location.open_or_create_ledger(TEST_LEDGER_HEADER).unwrap();
        assert_eq!(identity.size, TEST_LEDGER_HEADER.len() as i64);
        let mut observed = Vec::new();
        file.read_to_end(&mut observed).unwrap();
        assert_eq!(observed, TEST_LEDGER_HEADER);
    }

    #[test]
    fn concurrent_process_initializers_publish_one_header() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.ledger");
        let start = temp.path().join("start");
        let executable = std::env::current_exe().unwrap();
        let mut children = Vec::new();
        for _ in 0..24 {
            children.push(
                Command::new(&executable)
                    .args([
                        "secure_fs::tests::ledger_initializer_process_child",
                        "--exact",
                        "--ignored",
                    ])
                    .env("FE2O3_LEDGER_INIT_PATH", &path)
                    .env("FE2O3_LEDGER_INIT_START", &start)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
        }
        std::fs::write(&start, b"start").unwrap();
        for child in children {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "initializer failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert_eq!(std::fs::read(&path).unwrap(), TEST_LEDGER_HEADER);
        std::fs::remove_file(&start).unwrap();
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn substitution_between_lstat_and_open_is_detected() {
        let temp = secure_tempdir();
        let path = temp.path().join("authority");
        std::fs::write(&path, b"trusted").unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        let replacement = temp.path().join("replacement");
        std::fs::write(&replacement, b"attacker").unwrap();
        std::fs::set_permissions(&replacement, Permissions::from_mode(0o600)).unwrap();
        let location = SecureLocation::open(&path).unwrap();
        assert!(
            location
                .open_existing_owner_only_after(16, || {
                    std::fs::rename(&replacement, &path).unwrap();
                })
                .is_err()
        );
    }

    #[test]
    fn atomic_owner_file_creation_never_replaces_an_existing_artifact() {
        let temp = secure_tempdir();
        let path = temp.path().join("enrollment.json");
        write_new_owner_only(&path, b"first", 16).unwrap();
        assert_eq!(read_owner_only(&path, 16).unwrap(), b"first");
        assert!(write_new_owner_only(&path, b"second", 16).is_err());
        assert_eq!(read_owner_only(&path, 16).unwrap(), b"first");
    }
}
