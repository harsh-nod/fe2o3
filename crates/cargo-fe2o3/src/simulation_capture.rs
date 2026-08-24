use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use fe2o3_artifact_transaction::BuildAttempt;
use rustix::fs::{AtFlags, FileType, Mode, OFlags, fstat, fsync, openat, unlinkat};

use crate::project::PinnedDirectory;

const CAPTURE_PREFIX: &str = ".fe2o3-simulation-kir-v1-";
const CAPTURE_SUFFIX: &str = ".kir6";
const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CAPTURE_ENTRIES: usize = 2;

pub(crate) fn publish(
    output_dir: &Path,
    attempt: BuildAttempt,
    bytes: &[u8],
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_CAPTURE_BYTES {
        return Err(format!(
            "simulation KIR capture is {} bytes; expected 1..={MAX_CAPTURE_BYTES}",
            bytes.len()
        ));
    }
    let name = format!("{CAPTURE_PREFIX}{}{CAPTURE_SUFFIX}", attempt.to_env_value());
    let path = output_dir.join(&name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("cannot create exact simulation capture {name:?}: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write exact simulation capture {name:?}: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync exact simulation capture {name:?}: {error}"))?;
    let directory = File::open(output_dir)
        .map_err(|error| format!("cannot reopen simulation capture directory: {error}"))?;
    directory
        .sync_all()
        .map_err(|error| format!("cannot sync simulation capture directory: {error}"))
}

pub(crate) fn consume_exactly_one(directory: &PinnedDirectory) -> Result<Vec<u8>, String> {
    let scan = openat(
        directory.file(),
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("cannot pin simulation capture directory scan: {error}"))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("cannot enumerate simulation captures: {error}"))?;
    let mut captures = Vec::new();
    for entry in &mut entries {
        let entry =
            entry.map_err(|error| format!("cannot enumerate a simulation capture: {error}"))?;
        let name = entry.file_name();
        let bytes = name.to_bytes();
        if bytes == b"." || bytes == b".." || !is_capture_name(bytes) {
            continue;
        }
        captures
            .try_reserve(1)
            .map_err(|_| "cannot reserve simulation capture scan".to_owned())?;
        captures.push(copy_name(name)?);
        if captures.len() >= MAX_CAPTURE_ENTRIES {
            break;
        }
    }
    captures.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let [name] = captures.as_slice() else {
        let found = if captures.is_empty() {
            "none"
        } else {
            "multiple"
        };
        return Err(format!(
            "cargo fe2o3 simulate requires exactly one compiled kernel module; found {found}"
        ));
    };
    consume_capture(&scan, name)
}

fn is_capture_name(name: &[u8]) -> bool {
    name.starts_with(CAPTURE_PREFIX.as_bytes())
        && name.ends_with(CAPTURE_SUFFIX.as_bytes())
        && name.len() > CAPTURE_PREFIX.len() + CAPTURE_SUFFIX.len()
}

fn copy_name(name: &std::ffi::CStr) -> Result<OsString, String> {
    let bytes = name.to_bytes();
    let mut copy = OsString::new();
    copy.try_reserve(bytes.len())
        .map_err(|_| "cannot reserve simulation capture name".to_owned())?;
    copy.push(OsStr::from_bytes(bytes));
    Ok(copy)
}

fn consume_capture(directory: &std::os::fd::OwnedFd, name: &OsStr) -> Result<Vec<u8>, String> {
    let descriptor = openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|error| format!("cannot open exact simulation capture {name:?}: {error}"))?;
    let before = fstat(&descriptor)
        .map_err(|error| format!("cannot inspect exact simulation capture {name:?}: {error}"))?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 1
        || before.st_mode & 0o077 != 0
        || before.st_size <= 0
        || before.st_size as usize > MAX_CAPTURE_BYTES
    {
        return Err(format!(
            "exact simulation capture {name:?} is not one private bounded regular file"
        ));
    }
    let expected = before.st_size as usize;
    let mut file = File::from(descriptor);
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| "cannot reserve exact simulation capture bytes".to_owned())?;
    Read::by_ref(&mut file)
        .take((MAX_CAPTURE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read exact simulation capture {name:?}: {error}"))?;
    let after = fstat(&file)
        .map_err(|error| format!("cannot re-inspect exact simulation capture {name:?}: {error}"))?;
    if bytes.len() != expected
        || before.st_dev != after.st_dev
        || before.st_ino != after.st_ino
        || before.st_size != after.st_size
        || before.st_mtime != after.st_mtime
        || before.st_mtime_nsec != after.st_mtime_nsec
        || before.st_ctime != after.st_ctime
        || before.st_ctime_nsec != after.st_ctime_nsec
    {
        return Err(format!(
            "exact simulation capture {name:?} changed while it was read"
        ));
    }
    unlinkat(directory, name, AtFlags::empty())
        .map_err(|error| format!("cannot consume exact simulation capture {name:?}: {error}"))?;
    let unlinked = fstat(&file).map_err(|error| {
        format!("cannot re-inspect consumed simulation capture {name:?}: {error}")
    })?;
    if unlinked.st_dev != after.st_dev
        || unlinked.st_ino != after.st_ino
        || unlinked.st_nlink != 0
        || unlinked.st_size != after.st_size
    {
        return Err(format!(
            "exact simulation capture {name:?} was substituted before consumption"
        ));
    }
    fsync(directory)
        .map_err(|error| format!("cannot sync consumed simulation capture directory: {error}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{CAPTURE_PREFIX, CAPTURE_SUFFIX, consume_exactly_one};
    use crate::project::PinnedDirectory;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-simulation-capture-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn pin(&self) -> PinnedDirectory {
            PinnedDirectory::open_existing(self.0.clone(), "simulation capture test").unwrap()
        }

        fn capture(&self, id: &str) -> PathBuf {
            self.0.join(format!("{CAPTURE_PREFIX}{id}{CAPTURE_SUFFIX}"))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_private(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn capture_selection_requires_exactly_one_private_regular_file() {
        let directory = TestDirectory::new();
        assert!(
            consume_exactly_one(&directory.pin())
                .unwrap_err()
                .contains("found none")
        );

        let first = directory.capture("first");
        write_private(&first, b"canonical");
        assert_eq!(consume_exactly_one(&directory.pin()).unwrap(), b"canonical");

        write_private(&first, b"canonical");
        write_private(&directory.capture("second"), b"other");
        assert!(
            consume_exactly_one(&directory.pin())
                .unwrap_err()
                .contains("found multiple")
        );
    }

    #[test]
    fn capture_selection_rejects_symlink_and_permissive_substitution() {
        let directory = TestDirectory::new();
        let outside = directory.0.join("outside");
        write_private(&outside, b"outside");
        let capture = directory.capture("link");
        symlink(&outside, &capture).unwrap();
        assert!(consume_exactly_one(&directory.pin()).is_err());

        fs::remove_file(&capture).unwrap();
        write_private(&capture, b"canonical");
        fs::set_permissions(&capture, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(consume_exactly_one(&directory.pin()).is_err());
    }
}
