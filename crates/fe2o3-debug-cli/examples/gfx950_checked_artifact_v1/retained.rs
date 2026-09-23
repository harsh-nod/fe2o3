//! Bounded read-only file snapshot; no claim of atomic filesystem authentication.
use super::Failure;
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Eq, PartialEq)]
struct Stamp {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Stamp {
    fn from(value: Metadata) -> Result<Self, Failure> {
        if !value.is_file() || value.nlink() == 0 {
            return Err(Failure::new(
                "artifact_file",
                "regular_linked_file_required",
            ));
        }
        Ok(Self {
            device: value.dev(),
            inode: value.ino(),
            length: value.len(),
            mode: value.mode(),
            uid: value.uid(),
            gid: value.gid(),
            links: value.nlink(),
            modified: (value.mtime(), value.mtime_nsec()),
            changed: (value.ctime(), value.ctime_nsec()),
        })
    }
}
pub(super) struct Retained {
    file: File,
    path: PathBuf,
    stamp: Stamp,
    bytes: Vec<u8>,
}
impl Retained {
    pub(super) fn open(
        path: &Path,
        expected_bytes: usize,
        expected_sha: [u8; 32],
    ) -> Result<Self, Failure> {
        if expected_bytes == 0 || expected_bytes > fe2o3_hsaco::MAX_HSACO_BYTES {
            return Err(Failure::new("artifact_file", "size_bound"));
        }
        if !path.is_absolute() || std::fs::canonicalize(path).ok().as_deref() != Some(path) {
            return Err(Failure::new(
                "artifact_file",
                "canonical_absolute_path_required",
            ));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Failure::new("artifact_file", "open_nofollow"))?;
        let stamp = Stamp::from(
            file.metadata()
                .map_err(|_| Failure::new("artifact_file", "fstat"))?,
        )?;
        if stamp.length != expected_bytes as u64 {
            return Err(Failure::new("artifact_file", "expected_size_mismatch"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(expected_bytes)
            .map_err(|_| Failure::new("artifact_file", "allocation_failed"))?;
        bytes.resize(expected_bytes, 0);
        file.read_exact(&mut bytes)
            .map_err(|_| Failure::new("artifact_file", "short_read"))?;
        let mut extra = [0u8; 1];
        if file
            .read(&mut extra)
            .map_err(|_| Failure::new("artifact_file", "extra_read"))?
            != 0
        {
            return Err(Failure::new("artifact_file", "grew_during_read"));
        }
        let value = Self {
            file,
            path: path.to_path_buf(),
            stamp,
            bytes,
        };
        value.metadata_recheck()?;
        let digest: [u8; 32] = Sha256::digest(&value.bytes).into();
        if digest != expected_sha {
            return Err(Failure::new("artifact_pin", "sha256_mismatch"));
        }
        Ok(value)
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    fn metadata_recheck(&self) -> Result<(), Failure> {
        let fail = || Failure::new("retained_currentness", "file_identity_or_metadata_changed");
        let fd = Stamp::from(self.file.metadata().map_err(|_| fail())?).map_err(|_| fail())?;
        let path = Stamp::from(std::fs::symlink_metadata(&self.path).map_err(|_| fail())?)
            .map_err(|_| fail())?;
        if fd != self.stamp
            || path != self.stamp
            || std::fs::canonicalize(&self.path).ok().as_deref() != Some(self.path.as_path())
        {
            return Err(fail());
        }
        Ok(())
    }
    pub(super) fn recheck(&self) -> Result<(), Failure> {
        self.metadata_recheck()?;
        let mut scratch = [0u8; 65536];
        for (index, expected) in self.bytes.chunks(scratch.len()).enumerate() {
            let buffer = &mut scratch[..expected.len()];
            self.file
                .read_exact_at(buffer, (index * 65536) as u64)
                .map_err(|_| Failure::new("retained_currentness", "reread_failed"))?;
            if buffer != expected {
                return Err(Failure::new(
                    "retained_currentness",
                    "artifact_bytes_changed",
                ));
            }
        }
        let mut extra = [0u8; 1];
        if self
            .file
            .read_at(&mut extra, self.bytes.len() as u64)
            .map_err(|_| Failure::new("retained_currentness", "extra_reread"))?
            != 0
        {
            return Err(Failure::new("retained_currentness", "artifact_grew"));
        }
        self.metadata_recheck()
    }
}
