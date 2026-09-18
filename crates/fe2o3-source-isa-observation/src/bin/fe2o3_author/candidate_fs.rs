//! Linux-only, retained-descriptor reads and atomic no-replace candidate publication.
//! Requires O_TMPFILE and procfs fd links. No named temporary file is exposed.

use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

use fe2o3_source_isa_observation::source_edit_v1::{
    MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1, MAX_SOURCE_EDIT_OUTPUT_BYTES_V1,
    validate_source_edit_path_v1,
};
use rustix::fs::{AtFlags, CWD, Mode, OFlags, fchmod, linkat, open, openat};

const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const SOURCE_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

pub(super) struct RetainedSource {
    parent: File,
    name: String,
    file: File,
    observed: Metadata,
    original: Vec<u8>,
}

pub(super) struct Published {
    pub device: u64,
    pub inode: u64,
}

impl RetainedSource {
    pub(super) fn open(path: &str) -> Result<Self, String> {
        let (parent, name) = parent(path)?;
        let mut file = source_file(&parent, &name)?;
        let observed = regular_metadata(&file)?;
        let original = read_bounded(&mut file)?;
        let after = regular_metadata(&file)?;
        if !same_snapshot(&observed, &after) || after.len() != original.len() as u64 {
            return Err("source changed while reading retained file".into());
        }
        Ok(Self {
            parent,
            name,
            file,
            observed,
            original,
        })
    }

    pub(super) fn original(&self) -> &[u8] {
        &self.original
    }

    fn verify_unchanged(&mut self) -> Result<(), String> {
        if !same_snapshot(&self.observed, &regular_metadata(&self.file)?) {
            return Err("source changed before candidate publication".into());
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        let current = read_bounded(&mut self.file)?;
        if current != self.original
            || !same_snapshot(&self.observed, &regular_metadata(&self.file)?)
        {
            return Err("source changed before candidate publication".into());
        }
        let named = source_file(&self.parent, &self.name)?;
        if !same_snapshot(&self.observed, &regular_metadata(&named)?) {
            return Err("source path changed before candidate publication".into());
        }
        // The basename is resolved in the retained parent, not by re-walking
        // textual ancestors. An ancestor can be renamed after retention. This
        // is a point-in-time descriptor/name observation, not pathname or CAS
        // protection against other writers or directory changes.
        Ok(())
    }
}

pub(super) fn publish(
    source: &mut RetainedSource,
    candidate: &str,
    bytes: &[u8],
) -> Result<Published, String> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_EDIT_OUTPUT_BYTES_V1 {
        return Err("candidate exceeds the bounded source-output profile".into());
    }
    let (directory, name) = parent(candidate)?;
    let temporary = openat(
        &directory,
        ".",
        OFlags::RDWR | OFlags::TMPFILE | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|error| {
        format!("anonymous candidate staging unavailable (O_TMPFILE required): {error}")
    })?;
    let mut temporary = File::from(temporary);
    fchmod(&temporary, Mode::RUSR | Mode::WUSR)
        .map_err(|error| format!("cannot restrict candidate permissions: {error}"))?;
    temporary
        .write_all(bytes)
        .map_err(|error| format!("cannot write candidate: {error}"))?;
    temporary
        .sync_all()
        .map_err(|error| format!("cannot sync candidate: {error}"))?;
    let metadata = temporary.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.nlink() != 0 || metadata.len() != bytes.len() as u64 {
        return Err("anonymous candidate file identity changed".into());
    }
    source.verify_unchanged()?;
    // linkat follows ONLY our retained anonymous fd's procfs link. The explicit
    // destination is descriptor-relative and linkat never replaces an entry.
    // Existing files, directories, hard links, and symlinks all reject atomically.
    let descriptor_path = format!("/proc/self/fd/{}", temporary.as_raw_fd());
    linkat(
        CWD,
        &descriptor_path,
        &directory,
        &name,
        AtFlags::SYMLINK_FOLLOW,
    )
    .map_err(|error| {
        format!("candidate publication failed without replacing an existing entry: {error}")
    })?;
    directory.sync_all().map_err(|error| {
        format!("candidate {candidate:?} was created but directory durability is unconfirmed; candidate was retained: {error}")
    })?;
    Ok(Published {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn parent(path: &str) -> Result<(File, String), String> {
    validate_source_edit_path_v1(path).map_err(|error| error.to_string())?;
    let mut directory = File::from(
        open(".", DIRECTORY_FLAGS, Mode::empty())
            .map_err(|error| format!("cannot retain working directory: {error}"))?,
    );
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            return Ok((directory, component.to_owned()));
        }
        directory = File::from(
            openat(&directory, component, DIRECTORY_FLAGS, Mode::empty()).map_err(|error| {
                format!("path parent must be an existing ordinary no-symlink directory: {error}")
            })?,
        );
    }
    Err("source or candidate path has no file name".into())
}

fn source_file(directory: &File, name: &str) -> Result<File, String> {
    openat(directory, name, SOURCE_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|error| format!("cannot open ordinary no-symlink source file: {error}"))
}

fn regular_metadata(file: &File) -> Result<Metadata, String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("source must be an ordinary regular file".into());
    }
    if metadata.len() > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 as u64 {
        return Err("source exceeds the 1 MiB candidate limit".into());
    }
    Ok(metadata)
}

fn read_bounded(file: &mut File) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read retained source: {error}"))?;
    if bytes.len() > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 {
        return Err("source exceeds the 1 MiB candidate limit".into());
    }
    Ok(bytes)
}

fn same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(test)]
#[path = "candidate_fs_tests.rs"]
mod tests;
