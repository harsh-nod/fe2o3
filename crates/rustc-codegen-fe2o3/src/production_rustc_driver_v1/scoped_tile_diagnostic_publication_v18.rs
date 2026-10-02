//! Private bounded staging; final publication follows the exact diagnostic sentinel.
use super::super::publish_new_inert_output;
use sha2::{Digest, Sha256};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_PATH_BYTES: usize = 4096;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
}
impl Identity {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

pub(super) struct StagedOutput {
    output: PathBuf,
    directory: PathBuf,
    directory_identity: Identity,
    path: PathBuf,
    file_identity: Option<Identity>,
}
impl StagedOutput {
    #[cfg(test)]
    pub(super) fn test_paths(&self) -> (&Path, &Path) {
        (&self.directory, &self.path)
    }

    pub(super) fn new(output: &Path) -> Result<Self, String> {
        if output.as_os_str().len() > MAX_PATH_BYTES || output.file_name().is_none() {
            return Err("diagnostic V18 output must name a bounded fresh file".into());
        }
        match fs::symlink_metadata(output) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(
                    "diagnostic V18 output must not already exist, including a symlink".into(),
                );
            }
        }
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        for _ in 0..16 {
            let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
            let directory = parent.join(format!(".fe2o3-v18-{}-{sequence}", std::process::id()));
            if directory == output {
                continue;
            }
            match fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let metadata = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err("diagnostic V18 staging directory changed identity".into());
                    }
                    let path = directory.join("canonical-v18.bin");
                    return Ok(Self {
                        output: output.to_owned(),
                        directory,
                        directory_identity: Identity::of(&metadata),
                        path,
                        file_identity: None,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(format!("fresh diagnostic V18 staging directory: {error}"));
                }
            }
        }
        Err("diagnostic V18 staging-name retries exhausted".into())
    }

    fn directory_current(&self) -> bool {
        fs::symlink_metadata(&self.directory).is_ok_and(|metadata| {
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && Identity::of(&metadata) == self.directory_identity
        })
    }

    pub(super) fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.file_identity.is_some() || !self.directory_current() {
            return Err("diagnostic V18 staging is not fresh/current".into());
        }
        publish_new_inert_output(
            &self.path,
            bytes,
            fe2o3_kernel_ir::MAX_MODULE_BYTES_V1,
            "private diagnostic canonical KIR V18",
        )?;
        let metadata = fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() != bytes.len() as u64
        {
            return Err("diagnostic V18 staged file changed identity".into());
        }
        self.file_identity = Some(Identity::of(&metadata));
        Ok(())
    }

    pub(super) fn promote(&self, length: u64, digest: &[u8; 32]) -> Result<(), String> {
        if !self.directory_current()
            || length == 0
            || length > fe2o3_kernel_ir::MAX_MODULE_BYTES_V1 as u64
        {
            return Err("diagnostic V18 staged output is not bounded/current".into());
        }
        let expected = self
            .file_identity
            .ok_or("diagnostic V18 has no completed staged file")?;
        let metadata = fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || Identity::of(&metadata) != expected
            || metadata.len() != length
        {
            return Err("diagnostic V18 staged file identity mismatch".into());
        }
        let mut file = fs::File::open(&self.path).map_err(|e| e.to_string())?;
        let opened = file.metadata().map_err(|e| e.to_string())?;
        if !opened.is_file() || Identity::of(&opened) != expected || opened.len() != length {
            return Err("diagnostic V18 opened staging identity mismatch".into());
        }
        // Fixed scratch and an exact upper bound, independent of file growth.
        let mut hash = Sha256::new();
        let mut buffer = [0; 8192];
        let mut remaining = length;
        while remaining != 0 {
            let count = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
            file.read_exact(&mut buffer[..count])
                .map_err(|e| e.to_string())?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if file.read(&mut buffer[..1]).map_err(|e| e.to_string())? != 0
            || <[u8; 32]>::from(hash.finalize()) != *digest
        {
            return Err("diagnostic V18 staged bytes do not match the live candidate".into());
        }
        let current = fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?;
        if !self.directory_current()
            || current.file_type().is_symlink()
            || Identity::of(&current) != expected
            || current.len() != length
        {
            return Err("diagnostic V18 staging changed before promotion".into());
        }
        fs::hard_link(&self.path, &self.output)
            .map_err(|e| format!("fresh diagnostic V18 output promotion: {e}"))?;
        let published = fs::symlink_metadata(&self.output).map_err(|e| e.to_string())?;
        if !published.is_file()
            || published.file_type().is_symlink()
            || Identity::of(&published) != expected
            || published.len() != length
        {
            return Err("diagnostic V18 output identity changed during promotion".into());
        }
        Ok(())
    }
}
impl Drop for StagedOutput {
    fn drop(&mut self) {
        if !self.directory_current() {
            return;
        }
        if let Some(expected) = self.file_identity {
            if fs::symlink_metadata(&self.path).is_ok_and(|metadata| {
                metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && Identity::of(&metadata) == expected
            }) {
                let _ = fs::remove_file(&self.path);
            }
        }
        // Never recurse or remove the final destination. Unknown partial or
        // replaced entries remain retained rather than deleting foreign state.
        let _ = fs::remove_dir(&self.directory);
    }
}
