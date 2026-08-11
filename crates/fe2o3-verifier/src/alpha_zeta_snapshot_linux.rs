use super::*;

use std::ffi::{CString, c_int, c_long};
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

const AT_FDCWD: c_int = -100;
const SYS_OPENAT2: c_long = 437;
const O_RDONLY: u64 = 0;
const O_NONBLOCK: u64 = 0o4000;
const O_CLOEXEC: u64 = 0o2_000_000;
const O_NOFOLLOW: u64 = 0o400_000;
const O_DIRECTORY: u64 = 0o200_000;
const O_PATH: u64 = 0o10_000_000;
const RESOLVE_NO_XDEV: u64 = 0x01;
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;

const ROOT_RESOLVE: u64 = RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS;
const DESCENDANT_RESOLVE: u64 =
    RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV;
const SNAPSHOT_GENERATION_DOMAIN: &[u8; 8] = b"FE2AZSG\0";

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
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
            links: metadata.nlink(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.device.to_le_bytes());
        bytes.extend_from_slice(&self.inode.to_le_bytes());
        bytes.extend_from_slice(&self.mode.to_le_bytes());
        bytes.extend_from_slice(&self.links.to_le_bytes());
        bytes.extend_from_slice(&self.size.to_le_bytes());
        bytes.extend_from_slice(&self.modified_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.modified_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&self.changed_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.changed_nanoseconds.to_le_bytes());
    }
}

#[derive(Debug)]
struct RetainedObject {
    file: File,
    snapshot: ObjectSnapshot,
}

#[derive(Debug)]
struct RetainedFile {
    object: RetainedObject,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct SnapshotLease {
    root: RetainedObject,
    directories: BTreeMap<String, RetainedObject>,
    files: BTreeMap<String, RetainedFile>,
    generation_identity: Digest,
}

impl SnapshotLease {
    pub(super) fn generation_identity(&self) -> Digest {
        self.generation_identity
    }

    pub(super) fn revalidate(&self) -> Result<(), AlphaZetaProofErrorV1> {
        validate_retained_object(".", &self.root, true)?;
        for (path, directory) in &self.directories {
            validate_retained_object(path, directory, true)?;
        }
        for (path, file) in &self.files {
            validate_retained_object(path, &file.object, false)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct SnapshotFilesystem {
    root: RetainedObject,
    directories: BTreeMap<String, RetainedObject>,
    files: BTreeMap<String, RetainedFile>,
}

impl SnapshotFilesystem {
    pub(super) fn open(workspace_root: &Path) -> Result<Self, AlphaZetaProofErrorV1> {
        let components = lexical_absolute_components(workspace_root)?;
        let slash = CString::new("/").expect("fixed path has no NUL");
        let mut current = File::from(
            openat2(
                AT_FDCWD,
                &slash,
                O_PATH | O_CLOEXEC | O_DIRECTORY,
                0,
                ROOT_RESOLVE,
            )
            .map_err(|error| manifest_open_error("open workspace root", "/", error))?,
        );
        for component in components {
            let name = cstring_component(&component, "workspace root")?;
            current = File::from(
                openat2(
                    current.as_raw_fd(),
                    &name,
                    O_PATH | O_CLOEXEC | O_NOFOLLOW | O_DIRECTORY,
                    0,
                    RESOLVE_BENEATH | ROOT_RESOLVE,
                )
                .map_err(|error| {
                    manifest_open_error(
                        "open workspace component",
                        &component.to_string_lossy(),
                        error,
                    )
                })?,
            );
        }
        let snapshot = directory_snapshot(&current, ".")?;
        Ok(Self {
            root: RetainedObject {
                file: current,
                snapshot,
            },
            directories: BTreeMap::new(),
            files: BTreeMap::new(),
        })
    }

    pub(super) fn read_file(&mut self, path: &str) -> Result<Vec<u8>, AlphaZetaProofErrorV1> {
        self.read_file_internal(path, false)?
            .ok_or_else(|| manifest_io("open", path.to_owned()))
    }

    pub(super) fn regular_file_exists(
        &mut self,
        path: &str,
    ) -> Result<bool, AlphaZetaProofErrorV1> {
        Ok(self.read_file_internal(path, true)?.is_some())
    }

    pub(super) fn finish(self) -> Result<SnapshotLease, AlphaZetaProofErrorV1> {
        pause_before_finish_revalidation();
        let lease = SnapshotLease {
            generation_identity: snapshot_generation_identity(
                &self.root,
                &self.directories,
                &self.files,
            ),
            root: self.root,
            directories: self.directories,
            files: self.files,
        };
        lease.revalidate()?;
        Ok(lease)
    }

    fn read_file_internal(
        &mut self,
        path: &str,
        missing_ok: bool,
    ) -> Result<Option<Vec<u8>>, AlphaZetaProofErrorV1> {
        if let Some(retained) = self.files.get(path) {
            return Ok(Some(retained.bytes.clone()));
        }
        let relative = normalize_relative(path)?;
        let mut components = relative.split('/').collect::<Vec<_>>();
        let leaf = components
            .pop()
            .ok_or_else(|| manifest_structure(path, "source path has no file name"))?;
        let Some(parent) = self.open_parent_directories(path, &components, missing_ok)? else {
            return Ok(None);
        };
        let leaf = CString::new(leaf)
            .map_err(|_| manifest_structure(path, "source file name contains NUL"))?;
        let file = match openat2(
            parent.as_raw_fd(),
            &leaf,
            O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK,
            0,
            DESCENDANT_RESOLVE,
        ) {
            Ok(file) => File::from(file),
            Err(error) if missing_ok && error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(manifest_open_error("open source file", path, error)),
        };
        let before = regular_file_snapshot(&file, path)?;
        if before.size == 0 || before.size > MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1 {
            return Err(AlphaZetaProofErrorV1::SourceLengthOutOfRange {
                max: MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1,
            });
        }
        let mut file = file;
        let mut bytes = Vec::with_capacity(before.size as usize);
        Read::by_ref(&mut file)
            .take(MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| manifest_open_error("read source file", path, error))?;
        let after = regular_file_snapshot(&file, path)?;
        if before != after || bytes.len() as u64 != after.size {
            return Err(AlphaZetaProofErrorV1::SourceSnapshotGenerationChanged);
        }
        self.files.insert(
            relative,
            RetainedFile {
                object: RetainedObject {
                    file,
                    snapshot: after,
                },
                bytes: bytes.clone(),
            },
        );
        Ok(Some(bytes))
    }

    fn open_parent_directories(
        &mut self,
        full_path: &str,
        components: &[&str],
        missing_ok: bool,
    ) -> Result<Option<File>, AlphaZetaProofErrorV1> {
        let mut parent = self
            .root
            .file
            .try_clone()
            .map_err(|error| manifest_open_error("clone workspace root", full_path, error))?;
        let mut key = String::new();
        for component in components {
            if !key.is_empty() {
                key.push('/');
            }
            key.push_str(component);
            if let Some(retained) = self.directories.get(&key) {
                parent = retained.file.try_clone().map_err(|error| {
                    manifest_open_error("clone source parent", full_path, error)
                })?;
                continue;
            }
            let name = CString::new(*component)
                .map_err(|_| manifest_structure(full_path, "source component contains NUL"))?;
            let opened = match openat2(
                parent.as_raw_fd(),
                &name,
                O_PATH | O_CLOEXEC | O_NOFOLLOW | O_DIRECTORY,
                0,
                DESCENDANT_RESOLVE,
            ) {
                Ok(file) => File::from(file),
                Err(error) if missing_ok && error.kind() == io::ErrorKind::NotFound => {
                    return Ok(None);
                }
                Err(error) => {
                    return Err(manifest_open_error("open source parent", full_path, error));
                }
            };
            let snapshot = directory_snapshot(&opened, &key)?;
            parent = opened
                .try_clone()
                .map_err(|error| manifest_open_error("clone source parent", full_path, error))?;
            self.directories.insert(
                key.clone(),
                RetainedObject {
                    file: opened,
                    snapshot,
                },
            );
        }
        Ok(Some(parent))
    }
}

fn lexical_absolute_components(
    path: &Path,
) -> Result<Vec<std::ffi::OsString>, AlphaZetaProofErrorV1> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| manifest_open_error("resolve current directory", ".", error))?
            .join(path)
    };
    let mut components = Vec::new();
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) => components.push(value.to_owned()),
            Component::ParentDir if components.pop().is_some() => {}
            Component::ParentDir | Component::Prefix(_) => {
                return Err(manifest_structure(
                    &path.display().to_string(),
                    "workspace root escapes the filesystem root",
                ));
            }
        }
    }
    Ok(components)
}

fn cstring_component(
    component: &std::ffi::OsStr,
    path: &str,
) -> Result<CString, AlphaZetaProofErrorV1> {
    CString::new(component.as_bytes())
        .map_err(|_| manifest_structure(path, "path component contains NUL"))
}

fn directory_snapshot(file: &File, path: &str) -> Result<ObjectSnapshot, AlphaZetaProofErrorV1> {
    let metadata = file
        .metadata()
        .map_err(|error| manifest_open_error("fstat source directory", path, error))?;
    if !metadata.is_dir() {
        return Err(manifest_structure(path, "source parent is not a directory"));
    }
    Ok(ObjectSnapshot::from_metadata(&metadata))
}

fn regular_file_snapshot(file: &File, path: &str) -> Result<ObjectSnapshot, AlphaZetaProofErrorV1> {
    let metadata = file
        .metadata()
        .map_err(|error| manifest_open_error("fstat source file", path, error))?;
    if !metadata.is_file() {
        return Err(manifest_structure(path, "source is not a regular file"));
    }
    if metadata.nlink() != 1 {
        return Err(manifest_structure(path, "source file must have one link"));
    }
    Ok(ObjectSnapshot::from_metadata(&metadata))
}

fn validate_retained_object(
    path: &str,
    object: &RetainedObject,
    directory: bool,
) -> Result<(), AlphaZetaProofErrorV1> {
    let actual = if directory {
        directory_snapshot(&object.file, path)?
    } else {
        regular_file_snapshot(&object.file, path)?
    };
    if actual == object.snapshot {
        Ok(())
    } else {
        Err(AlphaZetaProofErrorV1::SourceSnapshotGenerationChanged)
    }
}

fn snapshot_generation_identity(
    root: &RetainedObject,
    directories: &BTreeMap<String, RetainedObject>,
    files: &BTreeMap<String, RetainedFile>,
) -> Digest {
    let mut bytes = Vec::with_capacity(128 + (directories.len() + files.len()) * 128);
    bytes.extend_from_slice(SNAPSHOT_GENERATION_DOMAIN);
    root.snapshot.encode(&mut bytes);
    bytes.extend_from_slice(&(directories.len() as u16).to_le_bytes());
    for (path, object) in directories {
        put_text(&mut bytes, path);
        object.snapshot.encode(&mut bytes);
    }
    bytes.extend_from_slice(&(files.len() as u16).to_le_bytes());
    for (path, file) in files {
        put_text(&mut bytes, path);
        file.object.snapshot.encode(&mut bytes);
    }
    sha256(&bytes)
}

fn manifest_open_error(
    operation: &'static str,
    path: &str,
    _error: io::Error,
) -> AlphaZetaProofErrorV1 {
    manifest_io(operation, path.to_owned())
}

fn openat2(
    directory: RawFd,
    path: &CString,
    flags: u64,
    mode: u64,
    resolve: u64,
) -> io::Result<OwnedFd> {
    let how = OpenHow {
        flags,
        mode,
        resolve,
    };
    // SAFETY: `path` and `how` remain live for the exact syscall duration, and
    // a successful return is a newly owned descriptor.
    let result = unsafe {
        linux_syscall(
            SYS_OPENAT2,
            directory,
            path.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: successful `openat2` returns a new owned descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(result as RawFd) })
    }
}

#[cfg(test)]
#[derive(Clone)]
struct FinishPause {
    thread: std::thread::ThreadId,
    reached: std::sync::Arc<std::sync::Barrier>,
    resume: std::sync::Arc<std::sync::Barrier>,
}

#[cfg(test)]
static FINISH_PAUSE: std::sync::Mutex<Option<FinishPause>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(super) fn install_finish_pause(
    reached: std::sync::Arc<std::sync::Barrier>,
    resume: std::sync::Arc<std::sync::Barrier>,
) {
    *FINISH_PAUSE.lock().unwrap() = Some(FinishPause {
        thread: std::thread::current().id(),
        reached,
        resume,
    });
}

#[cfg(test)]
pub(super) fn clear_finish_pause() {
    *FINISH_PAUSE.lock().unwrap() = None;
}

#[cfg(test)]
fn pause_before_finish_revalidation() {
    let pause = FINISH_PAUSE
        .lock()
        .unwrap()
        .as_ref()
        .filter(|pause| pause.thread == std::thread::current().id())
        .cloned();
    if let Some(pause) = pause {
        pause.reached.wait();
        pause.resume.wait();
    }
}

#[cfg(not(test))]
fn pause_before_finish_revalidation() {}

unsafe extern "C" {
    #[link_name = "syscall"]
    fn linux_syscall(number: c_long, ...) -> c_long;
}
