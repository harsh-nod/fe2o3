use std::ffi::{CStr, CString, OsStr};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::PublisherError;

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
        {
            return Err(PublisherError::Config);
        }
        Ok(bytes)
    }

    pub(crate) fn open_or_create_ledger(
        &self,
    ) -> Result<(File, FileIdentity, bool), PublisherError> {
        let mut created = false;
        let file = match openat_raw(
            self.directory.as_raw_fd(),
            &self.name,
            libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_CREAT | libc::O_EXCL,
            0o600,
        ) {
            Ok(file) => {
                created = true;
                file
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => openat(
                self.directory.as_raw_fd(),
                &self.name,
                libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0,
            )?,
            Err(_) => return Err(PublisherError::Config),
        };
        let opened = fstat(file.as_raw_fd())?;
        let after = self.entry_identity()?;
        if opened != after
            || !opened.is_regular()
            || !opened.is_owner_only()
            || opened.mode & 0o777 != 0o600
            || opened.nlink != 1
            || !self
                .directory_identity()?
                .same_stable_object(self.directory_identity)
        {
            return Err(PublisherError::Config);
        }
        Ok((file, opened, created))
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
        {
            return Err(PublisherError::Store);
        }
        Ok(())
    }

    pub(crate) fn sync(&self) -> Result<(), PublisherError> {
        self.directory.sync_all().map_err(|_| PublisherError::Store)
    }

    fn entry_identity(&self) -> Result<FileIdentity, PublisherError> {
        fstatat(self.directory.as_raw_fd(), &self.name)
    }

    fn directory_identity(&self) -> Result<FileIdentity, PublisherError> {
        fstat(self.directory.as_raw_fd())
    }
}

pub(crate) fn read_owner_only(path: &Path, max_bytes: usize) -> Result<Vec<u8>, PublisherError> {
    SecureLocation::open(path)?.read_existing_owner_only(max_bytes)
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
        return Err(PublisherError::Config);
    }
    Ok(FileIdentity::from_stat(&unsafe { stat.assume_init() }))
}

#[cfg(test)]
mod tests {
    use std::fs::{Permissions, hard_link};
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;
    use crate::test_support::secure_tempdir;

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
    fn database_entry_substitution_is_detected() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.db");
        let location = SecureLocation::open(&path).unwrap();
        let (_file, identity, _) = location.open_or_create_ledger().unwrap();
        let moved = temp.path().join("moved.db");
        std::fs::rename(&path, &moved).unwrap();
        std::fs::write(&path, b"replacement").unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        assert!(location.verify_ledger_entry(identity).is_err());
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
