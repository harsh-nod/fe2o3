//! Linux-only, retained-descriptor reads and atomic no-replace candidate publication.
//! Requires O_TMPFILE and procfs fd links. No named temporary file is exposed.
//!
//! Paths are explicit bounded relative Rust-file paths, never source-map display
//! labels. This is inert file I/O: the bytes need not be valid UTF-8 or Rust, and
//! neither retained descriptors nor publication observations authenticate source,
//! establish semantic equivalence, or grant proof, compiler-resume or GPU authority.
//! Callers must independently validate their byte commitments and source edits.

use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

use crate::source_edit_v1::{
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

/// Move-only original bytes and private descriptors for the opened file and parent.
///
/// The parent descriptor survives ancestor renames. Currentness therefore means
/// exact bytes and the retained parent/name at a point in time, not that rewalking
/// the original textual path reaches this file. No lock or filesystem CAS is claimed.
///
/// ```compile_fail
/// use fe2o3_source_isa_observation::source_candidate_io_v1::RetainedSource;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<RetainedSource>();
/// ```
///
/// ```compile_fail
/// use fe2o3_source_isa_observation::source_candidate_io_v1::RetainedSource;
/// let _: RetainedSource = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use fe2o3_source_isa_observation::source_candidate_io_v1::RetainedSource;
/// fn replace_bytes(source: &mut RetainedSource) {
///     source.original = b"substituted".to_vec();
/// }
/// ```
pub struct RetainedSource {
    parent: File,
    name: String,
    file: File,
    observed: Metadata,
    original: Vec<u8>,
}

/// Inert device/inode observation of the new candidate, not source authenticity
/// or an owner granting access to the published file. Other writers can still
/// modify or replace its name after publication; no continuing currentness is claimed.
pub struct Published {
    /// Device observed on the retained anonymous staging descriptor.
    pub device: u64,
    /// Inode observed on the retained anonymous staging descriptor.
    pub inode: u64,
}

impl RetainedSource {
    /// Retain a bounded regular source and its no-symlink parent traversal.
    /// Reads at most the existing 1 MiB source limit plus one rejection byte.
    /// UTF-8, Rust syntax, compiler identity and caller hashes are not checked here.
    pub fn open(path: &str) -> Result<Self, String> {
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

    /// Exact bytes observed during `open`; this immutable view carries no authority.
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// Recheck exact bytes, inode/metadata and the basename in the retained parent.
    /// This point-in-time observation is also mandatory inside [`publish`], after
    /// candidate staging and before its atomic no-replace link. Ancestor renames
    /// do not retarget the retained parent. Concurrent future changes remain possible.
    pub fn recheck(&mut self) -> Result<(), String> {
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

/// Create a new bounded candidate without replacing any existing destination.
///
/// Staging is anonymous and mode 0600; bytes are synced and read back before
/// rechecking the retained source and atomically linking the new name. A failure
/// before linking exposes no temporary name. A directory-sync failure after
/// linking reports that the candidate was retained; it is never removed as an
/// implicit rollback. No compilation, semantic check or source authentication occurs.
pub fn publish(
    source: &mut RetainedSource,
    candidate: &str,
    bytes: &[u8],
) -> Result<Published, String> {
    publish_inner(
        source,
        candidate,
        bytes,
        #[cfg(test)]
        None,
    )
}

// The test-only callback has no global state and cannot be supplied by a
// dependent crate. Ordinary builds have neither this type nor a hook parameter.
#[cfg(test)]
type DirectorySyncHook<'a> = &'a mut dyn FnMut(&File, &File) -> std::io::Result<()>;

fn publish_inner(
    source: &mut RetainedSource,
    candidate: &str,
    bytes: &[u8],
    #[cfg(test)] directory_sync: Option<DirectorySyncHook<'_>>,
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
    verify_staged_bytes(&mut temporary, &metadata, bytes)?;
    source.recheck()?;
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
    // Inject only the final directory-sync result in crate-local unit tests.
    // All staging, readback, source checks and the no-replace link above are real.
    #[cfg(not(test))]
    let directory_sync_result = directory.sync_all();
    #[cfg(test)]
    let directory_sync_result = match directory_sync {
        Some(sync) => sync(&directory, &temporary),
        None => directory.sync_all(),
    };
    directory_sync_result.map_err(|error| {
        format!("candidate {candidate:?} was created but directory durability is unconfirmed; candidate was retained: {error}")
    })?;
    Ok(Published {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn verify_staged_bytes(
    temporary: &mut File,
    observed: &Metadata,
    expected: &[u8],
) -> Result<(), String> {
    if expected.is_empty() || expected.len() > MAX_SOURCE_EDIT_OUTPUT_BYTES_V1 {
        return Err("candidate exceeds the bounded source-output profile".into());
    }
    temporary
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind staged candidate for readback: {error}"))?;
    let mut readback = Vec::with_capacity(expected.len() + 1);
    (&mut *temporary)
        .take(expected.len() as u64 + 1)
        .read_to_end(&mut readback)
        .map_err(|error| format!("cannot read back staged candidate: {error}"))?;
    if readback != expected {
        return Err("anonymous candidate bytes changed before publication".into());
    }
    let after = temporary
        .metadata()
        .map_err(|error| format!("cannot inspect staged candidate after readback: {error}"))?;
    if !after.is_file() || after.nlink() != 0 || !same_snapshot(observed, &after) {
        return Err("anonymous candidate file identity changed during readback".into());
    }
    Ok(())
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
#[path = "source_candidate_io_v1_tests.rs"]
mod tests;
