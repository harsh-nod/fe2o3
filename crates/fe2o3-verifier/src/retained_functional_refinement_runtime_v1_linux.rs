use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

#[cfg(test)]
use std::sync::Mutex;

use rustix::fs::{
    AtFlags, FileType, MemfdFlags, Mode, OFlags, ResolveFlags, SealFlags, fchmod, fcntl_add_seals,
    fcntl_get_seals, fstat, inotify, memfd_create, open, openat, openat2, readlinkat, statat,
};
use sha2::{Digest, Sha256};

use crate::CanonicalGeneratedVerusProofInputV3;
use crate::authenticated_verus_execution_v2::{
    ADDRESS_SPACE_LIMIT_V2, CORE_LIMIT_V2, DATA_LIMIT_V2, FILE_LIMIT_V2,
};

#[cfg(test)]
use super::{DirectorySpecV2, FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME};
use super::{
    EntryKindV2, FileSpecV2, InterpreterSpecV2, MAX_TARGET_FILE_BYTES, ManifestV2,
    RetainedFunctionalRefinementRuntimeErrorKindV1, RetainedFunctionalRefinementRuntimeErrorV1,
    RetainedFunctionalRefinementRuntimeOutputV1,
};
#[path = "functional_refinement_process_tree_v1_linux.rs"]
mod functional_refinement_process_tree_v1;
const MAX_DIRECTORY_ENTRIES: usize = 256;
const MAX_TOTAL_RUNTIME_BYTES: u64 = 1024 * 1024 * 1024;

#[cfg(test)]
static RUNTIME_CLOSURE_PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

const RUST_VERIFY_FD: RawFd = 180;
const Z3_FD: RawFd = 181;
const DIST_DIRECTORY_FD: RawFd = 182;
const TOOLCHAIN_DIRECTORY_FD: RawFd = 183;
const TOOLCHAIN_LIB_DIRECTORY_FD: RawFd = 184;
const SYSTEM_LIB_DIRECTORY_FD: RawFd = 185;
const GENERATED_PROOF_SOURCE_FD: RawFd = 187;

const GENERATED_PROOF_SOURCE_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectSnapshotV2 {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    owner: u32,
    group: u32,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl ObjectSnapshotV2 {
    fn capture(
        file: &File,
        context: &str,
    ) -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        fstat(file)
            .map(|stat| Self {
                device: stat.st_dev,
                inode: stat.st_ino,
                mode: stat.st_mode,
                links: stat.st_nlink,
                owner: stat.st_uid,
                group: stat.st_gid,
                size: stat.st_size,
                modified_seconds: stat.st_mtime,
                modified_nanoseconds: stat.st_mtime_nsec as i64,
                changed_seconds: stat.st_ctime,
                changed_nanoseconds: stat.st_ctime_nsec as i64,
            })
            .map_err(|error| io_error(format!("inspect {context}"), error))
    }

    fn object_identity(self) -> ObjectIdentityV2 {
        ObjectIdentityV2 {
            device: self.device,
            inode: self.inode,
            mode: self.mode,
            owner: self.owner,
            group: self.group,
        }
    }

    fn file_type(self) -> FileType {
        FileType::from_raw_mode(self.mode)
    }

    fn permissions(self) -> u32 {
        self.mode & 0o7777
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectIdentityV2 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
    group: u32,
}

struct PathAnchorV2 {
    name: OsString,
    file: File,
    identity: ObjectIdentityV2,
}

struct RetainedDirectoryV2 {
    path: PathBuf,
    file: File,
    snapshot: ObjectSnapshotV2,
    expected_children: BTreeMap<PathBuf, EntryKindV2>,
}

struct RetainedFileV2 {
    path: PathBuf,
    file: File,
    snapshot: ObjectSnapshotV2,
}

struct RetainedSymlinkV2 {
    diagnostic_path: PathBuf,
    parent: File,
    name: OsString,
    file: File,
    snapshot: ObjectSnapshotV2,
    target: PathBuf,
}

struct RetainedInterpreterV2 {
    anchors: Vec<PathAnchorV2>,
    file: RetainedFileV2,
    links: Vec<RetainedSymlinkV2>,
}

struct ProtectionPolicyV2 {
    owner: u32,
    group: u32,
    protect_path_anchors: bool,
}

pub(super) struct RetainedRuntimeClosureV2 {
    path_anchors: Vec<PathAnchorV2>,
    directories: BTreeMap<PathBuf, RetainedDirectoryV2>,
    files: Vec<RetainedFileV2>,
    interpreter: Option<RetainedInterpreterV2>,
    journal: MutationJournalV2,
}

impl std::fmt::Debug for RetainedRuntimeClosureV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedRuntimeClosureV2")
            .field("path_anchor_count", &self.path_anchors.len())
            .field("directory_count", &self.directories.len())
            .field("file_count", &self.files.len())
            .field("has_interpreter", &self.interpreter.is_some())
            .finish_non_exhaustive()
    }
}

impl RetainedRuntimeClosureV2 {
    pub(super) fn open_protected(
        root: &Path,
        manifest: &ManifestV2,
    ) -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        Self::open_with_policy(
            root,
            manifest,
            ProtectionPolicyV2 {
                owner: 0,
                group: 0,
                protect_path_anchors: true,
            },
        )
    }

    fn open_with_policy(
        root: &Path,
        manifest: &ManifestV2,
        policy: ProtectionPolicyV2,
    ) -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        let path_anchors = open_directory_anchors(root, &policy)?;
        let retained_root = duplicate_directory(
            &path_anchors
                .last()
                .expect("absolute non-root path has an anchor")
                .file,
            "runtime root",
        )?;
        let root_snapshot =
            validate_directory(&retained_root, manifest.root_mode, &policy, "runtime root")?;
        let journal = MutationJournalV2::new()?;
        journal.watch(&retained_root, Path::new(""))?;

        let mut directories = BTreeMap::new();
        directories.insert(
            PathBuf::new(),
            RetainedDirectoryV2 {
                path: PathBuf::new(),
                file: retained_root,
                snapshot: root_snapshot,
                expected_children: manifest
                    .children
                    .get(Path::new(""))
                    .cloned()
                    .expect("reviewed manifest has root children"),
            },
        );
        for specification in &manifest.directories {
            let parent_path = specification.path.parent().unwrap_or_else(|| Path::new(""));
            let parent = directories
                .get(parent_path)
                .expect("reviewed manifest orders parents before children");
            let name = specification
                .path
                .file_name()
                .expect("reviewed directory path has a name");
            let file = open_directory_beneath(&parent.file, name, true)?;
            let snapshot = validate_directory(
                &file,
                specification.mode,
                &policy,
                &format!("runtime directory {}", specification.path.display()),
            )?;
            journal.watch(&file, &specification.path)?;
            directories.insert(
                specification.path.clone(),
                RetainedDirectoryV2 {
                    path: specification.path.clone(),
                    file,
                    snapshot,
                    expected_children: manifest
                        .children
                        .get(&specification.path)
                        .cloned()
                        .expect("reviewed manifest has every directory inventory"),
                },
            );
        }
        validate_all_inventories(&directories)?;

        let mut files = Vec::with_capacity(manifest.files.len() + 1);
        let root_directory = directories
            .get(Path::new(""))
            .expect("retained runtime root exists");
        let installed_manifest =
            open_regular_beneath(&root_directory.file, OsStr::new(manifest.manifest_name))?;
        let manifest_spec = FileSpecV2 {
            path: PathBuf::from(manifest.manifest_name),
            mode: manifest.manifest_mode,
            size: Some(manifest.manifest_bytes.len() as u64),
            sha256: Sha256::digest(manifest.manifest_bytes).into(),
        };
        let retained_manifest = retain_file(installed_manifest, &manifest_spec, &policy)?;
        if read_exact_file(
            &retained_manifest.file,
            retained_manifest.snapshot.size as u64,
            &retained_manifest.path,
        )? != manifest.manifest_bytes
        {
            return Err(error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch,
                "installed runtime manifest bytes differ from the reviewed manifest",
            ));
        }
        files.push(retained_manifest);

        let mut total_bytes = manifest.manifest_bytes.len() as u64;
        for specification in &manifest.files {
            let parent = specification.path.parent().unwrap_or_else(|| Path::new(""));
            let directory = directories
                .get(parent)
                .expect("reviewed file parent is retained");
            let name = specification
                .path
                .file_name()
                .expect("reviewed file path has a name");
            let retained = retain_file(
                open_regular_beneath(&directory.file, name)?,
                specification,
                &policy,
            )?;
            total_bytes = total_bytes
                .checked_add(retained.snapshot.size as u64)
                .filter(|total| *total <= MAX_TOTAL_RUNTIME_BYTES)
                .ok_or_else(|| {
                    error(
                        RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch,
                        "runtime closure exceeds its total byte bound",
                    )
                })?;
            files.push(retained);
        }

        let interpreter = manifest
            .interpreter
            .as_ref()
            .map(retain_interpreter)
            .transpose()?;
        let retained = Self {
            path_anchors,
            directories,
            files,
            interpreter,
            journal,
        };
        retained.revalidate()?;
        Ok(retained)
    }

    pub(super) fn revalidate(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        self.journal.ensure_clean()?;
        revalidate_anchor_edges(&self.path_anchors, "runtime root")?;
        for directory in self.directories.values() {
            let current = ObjectSnapshotV2::capture(
                &directory.file,
                &format!("retained directory {}", directory.path.display()),
            )?;
            if current != directory.snapshot {
                return Err(changed(format!(
                    "retained directory changed: {}",
                    directory.path.display()
                )));
            }
            let parent_path = directory.path.parent().unwrap_or_else(|| Path::new(""));
            if !directory.path.as_os_str().is_empty() {
                let parent = self
                    .directories
                    .get(parent_path)
                    .expect("retained directory parent exists");
                let reopened = open_directory_beneath(
                    &parent.file,
                    directory
                        .path
                        .file_name()
                        .expect("retained directory has a name"),
                    true,
                )?;
                if ObjectSnapshotV2::capture(&reopened, "reopened runtime directory")?
                    .object_identity()
                    != directory.snapshot.object_identity()
                {
                    return Err(changed(format!(
                        "runtime directory path was substituted: {}",
                        directory.path.display()
                    )));
                }
            }
            let actual = scan_inventory(&directory.file, &directory.path)?;
            if actual != directory.expected_children {
                return Err(error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::InventoryMismatch,
                    format!(
                        "runtime directory inventory changed: {}",
                        display_relative(&directory.path)
                    ),
                ));
            }
        }
        for retained in &self.files {
            let current = ObjectSnapshotV2::capture(
                &retained.file,
                &format!("retained file {}", retained.path.display()),
            )?;
            if current != retained.snapshot {
                return Err(changed(format!(
                    "retained runtime file changed: {}",
                    retained.path.display()
                )));
            }
            let parent = self
                .directories
                .get(retained.path.parent().unwrap_or_else(|| Path::new("")))
                .expect("retained file parent exists");
            let reopened = open_regular_beneath(
                &parent.file,
                retained.path.file_name().expect("retained file has a name"),
            )?;
            if ObjectSnapshotV2::capture(&reopened, "reopened runtime file")?.object_identity()
                != retained.snapshot.object_identity()
            {
                return Err(changed(format!(
                    "runtime file path was substituted: {}",
                    retained.path.display()
                )));
            }
        }
        if let Some(interpreter) = &self.interpreter {
            interpreter.revalidate()?;
        }
        self.journal.ensure_clean()
    }

    fn required_file(
        &self,
        path: &Path,
    ) -> Result<&File, RetainedFunctionalRefinementRuntimeErrorV1> {
        self.files
            .iter()
            .find(|retained| retained.path == path)
            .map(|retained| &retained.file)
            .ok_or_else(|| {
                error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
                    format!("reviewed runtime manifest lacks {}", path.display()),
                )
            })
    }

    fn required_directory(
        &self,
        path: &Path,
    ) -> Result<&File, RetainedFunctionalRefinementRuntimeErrorV1> {
        self.directories
            .get(path)
            .map(|retained| &retained.file)
            .ok_or_else(|| {
                error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
                    format!("reviewed runtime manifest lacks {}", path.display()),
                )
            })
    }

    fn allowed_runtime_object_identities(
        &self,
    ) -> Result<
        Vec<functional_refinement_process_tree_v1::AllowedRuntimeExecutableV1>,
        RetainedFunctionalRefinementRuntimeErrorV1,
    > {
        let mut executables = Vec::new();
        for file in &self.files {
            if let Some(executable) =
                functional_refinement_process_tree_v1::allowed_runtime_executable(
                    &file.file,
                    file.snapshot.object_identity(),
                    &file.path,
                )?
            {
                executables.push(executable);
            }
        }
        if let Some(interpreter) = &self.interpreter {
            let executable = functional_refinement_process_tree_v1::allowed_runtime_executable(
                &interpreter.file.file,
                interpreter.file.snapshot.object_identity(),
                &interpreter.file.path,
            )?
            .ok_or_else(|| {
                error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch,
                    "retained interpreter is not one x86-64 ELF executable image",
                )
            })?;
            executables.push(executable);
        }
        Ok(executables)
    }

    #[cfg(test)]
    fn open_for_test(
        root: &Path,
        manifest: &ManifestV2,
    ) -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        let root_file = open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| io_error("open synthetic root", error))?;
        let snapshot = ObjectSnapshotV2::capture(&root_file, "synthetic root")?;
        Self::open_with_policy(
            root,
            manifest,
            ProtectionPolicyV2 {
                owner: snapshot.owner,
                group: snapshot.group,
                protect_path_anchors: false,
            },
        )
    }
}

pub(super) fn execute_functional_refinement_generated_rust_verify(
    runtime: &RetainedRuntimeClosureV2,
    source: &CanonicalGeneratedVerusProofInputV3,
    deadline: Instant,
    output_limit: usize,
) -> Result<RetainedFunctionalRefinementRuntimeOutputV1, RetainedFunctionalRefinementRuntimeErrorV1>
{
    functional_refinement_process_tree_v1::execute(runtime, source, deadline, output_limit)
}

struct SealedGeneratedProofSourceV3 {
    file: File,
    snapshot: ObjectSnapshotV2,
}

impl SealedGeneratedProofSourceV3 {
    fn create(
        source: &CanonicalGeneratedVerusProofInputV3,
    ) -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        let mut writable = memfd_create(
            "fe2o3-generated-verus-proof-v3",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .map_err(|error| io_error("create generated proof memfd", error))?;
        fchmod(&writable, Mode::RUSR)
            .map_err(|error| io_error("protect generated proof memfd", error))?;
        writable
            .write_all(source.source())
            .and_then(|()| writable.flush())
            .and_then(|()| writable.sync_all())
            .map_err(|error| io_std_error("write generated proof memfd", error))?;
        fcntl_add_seals(
            &writable,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
        )
        .and_then(|()| fcntl_add_seals(&writable, SealFlags::SEAL))
        .map_err(|error| io_error("seal generated proof memfd", error))?;
        let snapshot = ObjectSnapshotV2::capture(&writable, "generated proof memfd")?;
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", writable.as_raw_fd()));
        let file = open(
            &descriptor_path,
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| io_error("retain generated proof memfd read-only", error))?;
        if ObjectSnapshotV2::capture(&file, "read-only generated proof memfd")? != snapshot {
            return Err(changed(
                "read-only generated proof memfd identity changed during retention",
            ));
        }
        drop(writable);
        let sealed = Self { file, snapshot };
        sealed.revalidate(source)?;
        Ok(sealed)
    }

    fn revalidate(
        &self,
        source: &CanonicalGeneratedVerusProofInputV3,
    ) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        let current = ObjectSnapshotV2::capture(&self.file, "generated proof memfd")?;
        let descriptor_flags = rustix::io::fcntl_getfd(&self.file)
            .map_err(|error| io_error("inspect generated proof memfd descriptor flags", error))?;
        let status_flags = rustix::fs::fcntl_getfl(&self.file)
            .map_err(|error| io_error("inspect generated proof memfd status flags", error))?;
        if current != self.snapshot
            || current.file_type() != FileType::RegularFile
            || current.permissions() != 0o400
            || current.size < 0
            || current.size as u64 != source.byte_len()
            || !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
            || status_flags & OFlags::ACCMODE != OFlags::RDONLY
        {
            return Err(changed("generated proof memfd metadata changed"));
        }
        let seals = fcntl_get_seals(&self.file)
            .map_err(|error| io_error("inspect generated proof memfd seals", error))?;
        if seals != GENERATED_PROOF_SOURCE_SEALS {
            return Err(changed(
                "generated proof memfd lost its exact immutable seals",
            ));
        }
        if read_exact_file(
            &self.file,
            source.byte_len(),
            Path::new("generated-proof-v3.rs"),
        )? != source.source()
        {
            return Err(changed("generated proof memfd content changed"));
        }
        Ok(())
    }
}

impl RetainedInterpreterV2 {
    fn revalidate(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        revalidate_anchor_edges(&self.anchors, "system interpreter")?;
        if ObjectSnapshotV2::capture(&self.file.file, "retained system interpreter")?
            != self.file.snapshot
        {
            return Err(changed("retained system interpreter changed"));
        }
        for link in &self.links {
            link.revalidate()?;
        }
        Ok(())
    }
}

impl RetainedSymlinkV2 {
    fn revalidate(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        let reopened = open_symlink_beneath(&self.parent, &self.name)?;
        let snapshot = ObjectSnapshotV2::capture(&reopened, "reopened interpreter symlink")?;
        if snapshot != self.snapshot
            || read_link(&self.parent, &self.name)? != self.target
            || ObjectSnapshotV2::capture(&self.file, "retained interpreter symlink")?
                != self.snapshot
        {
            return Err(changed(format!(
                "system interpreter link changed: {}",
                self.diagnostic_path.display()
            )));
        }
        Ok(())
    }
}

fn retain_interpreter(
    specification: &InterpreterSpecV2,
) -> Result<RetainedInterpreterV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    let policy = ProtectionPolicyV2 {
        owner: 0,
        group: 0,
        protect_path_anchors: true,
    };
    let anchors = open_file_anchors(&specification.canonical, &policy)?;
    let file = anchors
        .last()
        .expect("absolute interpreter path has an anchor")
        .file
        .try_clone()
        .map_err(|error| io_std_error("duplicate retained interpreter", error))?;
    let retained = retain_file(
        file,
        &FileSpecV2 {
            path: specification.canonical.clone(),
            mode: 0o755,
            size: Some(specification.size),
            sha256: specification.sha256,
        },
        &policy,
    )?;
    let mut links = Vec::new();
    for (path, target) in &specification.links {
        links.push(retain_symlink(path, target)?);
    }
    if std::fs::canonicalize(&specification.requested)
        .map_err(|error| io_std_error("resolve system PT_INTERP chain", error))?
        != specification.canonical
    {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::SymlinkOrTraversal,
            "system PT_INTERP chain resolves to a different object",
        ));
    }
    Ok(RetainedInterpreterV2 {
        anchors,
        file: retained,
        links,
    })
}

fn retain_symlink(
    path: &Path,
    expected_target: &Path,
) -> Result<RetainedSymlinkV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    let parent_path = path.parent().ok_or_else(|| {
        error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::SymlinkOrTraversal,
            "interpreter link has no parent",
        )
    })?;
    let parent_anchors = open_directory_anchors(
        parent_path,
        &ProtectionPolicyV2 {
            owner: 0,
            group: 0,
            protect_path_anchors: true,
        },
    )?;
    let parent = parent_anchors
        .last()
        .expect("absolute interpreter-link parent has an anchor")
        .file
        .try_clone()
        .map_err(|error| io_std_error("duplicate interpreter-link parent", error))?;
    let name = path
        .file_name()
        .expect("interpreter link has a name")
        .to_os_string();
    let file = open_symlink_beneath(&parent, &name)?;
    let snapshot = ObjectSnapshotV2::capture(&file, "system interpreter link")?;
    if snapshot.file_type() != FileType::Symlink
        || snapshot.owner != 0
        || snapshot.group != 0
        || read_link(&parent, &name)? != expected_target
    {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::SymlinkOrTraversal,
            format!("system interpreter link differs: {}", path.display()),
        ));
    }
    Ok(RetainedSymlinkV2 {
        diagnostic_path: path.to_path_buf(),
        parent,
        name,
        file,
        snapshot,
        target: expected_target.to_path_buf(),
    })
}

fn open_directory_anchors(
    path: &Path,
    policy: &ProtectionPolicyV2,
) -> Result<Vec<PathAnchorV2>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let root = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| io_error("open filesystem root", error))?;
    let root_snapshot = ObjectSnapshotV2::capture(&root, "filesystem root")?;
    if policy.protect_path_anchors {
        validate_anchor_protection(root_snapshot, "filesystem root")?;
    }
    let mut anchors = vec![PathAnchorV2 {
        name: OsString::from("/"),
        file: root,
        identity: root_snapshot.object_identity(),
    }];
    for component in path.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let parent = &anchors.last().expect("filesystem root anchor exists").file;
        let file = open_directory_beneath(parent, name, false)?;
        let snapshot = ObjectSnapshotV2::capture(&file, "runtime path anchor")?;
        if policy.protect_path_anchors {
            validate_anchor_protection(snapshot, &format!("path anchor {name:?}"))?;
        }
        anchors.push(PathAnchorV2 {
            name: name.to_os_string(),
            file,
            identity: snapshot.object_identity(),
        });
    }
    Ok(anchors)
}

fn open_file_anchors(
    path: &Path,
    policy: &ProtectionPolicyV2,
) -> Result<Vec<PathAnchorV2>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let parent = path
        .parent()
        .expect("validated absolute file path has a parent");
    let mut anchors = open_directory_anchors(parent, policy)?;
    let name = path
        .file_name()
        .expect("validated absolute file path has a name");
    let file = open_regular_beneath(
        &anchors
            .last()
            .expect("absolute file parent has an anchor")
            .file,
        name,
    )?;
    let snapshot = ObjectSnapshotV2::capture(&file, "absolute retained file")?;
    anchors.push(PathAnchorV2 {
        name: name.to_os_string(),
        file,
        identity: snapshot.object_identity(),
    });
    Ok(anchors)
}

fn revalidate_anchor_edges(
    anchors: &[PathAnchorV2],
    label: &str,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    for (index, anchor) in anchors.iter().enumerate() {
        if ObjectSnapshotV2::capture(&anchor.file, label)?.object_identity() != anchor.identity {
            return Err(changed(format!(
                "retained {label} anchor changed at index {index}"
            )));
        }
        if index == 0 {
            continue;
        }
        let parent = &anchors[index - 1].file;
        let reopened = if FileType::from_raw_mode(anchor.identity.mode) == FileType::RegularFile {
            open_regular_beneath(parent, &anchor.name)?
        } else {
            open_directory_beneath(parent, &anchor.name, false)?
        };
        if ObjectSnapshotV2::capture(&reopened, label)?.object_identity() != anchor.identity {
            return Err(changed(format!(
                "{label} path edge was substituted at index {index}"
            )));
        }
    }
    Ok(())
}

fn validate_anchor_protection(
    snapshot: ObjectSnapshotV2,
    context: &str,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if snapshot.owner != 0 || snapshot.group != 0 || snapshot.permissions() & 0o022 != 0 {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::Protection,
            format!("{context} is not root-owned and protected"),
        ));
    }
    Ok(())
}

fn validate_directory(
    file: &File,
    expected_mode: u32,
    policy: &ProtectionPolicyV2,
    context: &str,
) -> Result<ObjectSnapshotV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    let snapshot = ObjectSnapshotV2::capture(file, context)?;
    if snapshot.file_type() != FileType::Directory {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::ObjectType,
            format!("{context} is not a directory"),
        ));
    }
    if snapshot.owner != policy.owner
        || snapshot.group != policy.group
        || snapshot.permissions() != expected_mode
    {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::Protection,
            format!("{context} ownership or mode differs"),
        ));
    }
    Ok(snapshot)
}

fn retain_file(
    file: File,
    specification: &FileSpecV2,
    policy: &ProtectionPolicyV2,
) -> Result<RetainedFileV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    let context = format!("runtime file {}", specification.path.display());
    let before = ObjectSnapshotV2::capture(&file, &context)?;
    if before.file_type() != FileType::RegularFile {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::ObjectType,
            format!("{context} is not a regular file"),
        ));
    }
    if before.owner != policy.owner
        || before.group != policy.group
        || before.links != 1
        || before.permissions() != specification.mode
        || before.size < 0
    {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::Protection,
            format!("{context} ownership, links, mode, or size differs"),
        ));
    }
    let size = before.size as u64;
    if specification.size.is_some_and(|expected| expected != size)
        || specification.size.is_none() && size > MAX_TARGET_FILE_BYTES
    {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch,
            format!("{context} size differs"),
        ));
    }
    let digest = hash_exact_file(&file, size, &specification.path)?;
    let after = ObjectSnapshotV2::capture(&file, &context)?;
    if before != after || digest != specification.sha256 {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch,
            format!("{context} changed or has a different SHA-256"),
        ));
    }
    Ok(RetainedFileV2 {
        path: specification.path.clone(),
        file,
        snapshot: before,
    })
}

fn validate_all_inventories(
    directories: &BTreeMap<PathBuf, RetainedDirectoryV2>,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    for directory in directories.values() {
        let actual = scan_inventory(&directory.file, &directory.path)?;
        if actual != directory.expected_children {
            return Err(error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::InventoryMismatch,
                format!(
                    "runtime directory inventory differs: {}",
                    display_relative(&directory.path)
                ),
            ));
        }
    }
    Ok(())
}

fn scan_inventory(
    directory: &File,
    path: &Path,
) -> Result<BTreeMap<PathBuf, EntryKindV2>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        io_error(
            format!("duplicate directory scan {}", path.display()),
            error,
        )
    })?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| io_error(format!("scan directory {}", path.display()), error))?;
    let mut actual = BTreeMap::new();
    for entry in &mut entries {
        let entry =
            entry.map_err(|error| io_error(format!("read directory {}", path.display()), error))?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        if actual.len() >= MAX_DIRECTORY_ENTRIES {
            return Err(error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::InventoryMismatch,
                format!("directory has too many entries: {}", path.display()),
            ));
        }
        let name = OsString::from_vec(bytes.to_vec());
        let stat = statat(directory, Path::new(&name), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| io_error(format!("inspect directory entry {name:?}"), error))?;
        let kind = match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => EntryKindV2::Directory,
            FileType::RegularFile => EntryKindV2::File,
            _ => {
                return Err(error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::ObjectType,
                    format!("runtime inventory contains a non-file entry: {name:?}"),
                ));
            }
        };
        if actual.insert(PathBuf::from(name), kind).is_some() {
            return Err(error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::InventoryMismatch,
                "runtime directory returned a duplicate name",
            ));
        }
    }
    Ok(actual)
}

fn hash_exact_file(
    file: &File,
    size: u64,
    path: &Path,
) -> Result<[u8; 32], RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut digest = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < size {
        let limit = usize::try_from((size - offset).min(buffer.len() as u64))
            .expect("bounded read length fits usize");
        let count = rustix::io::pread(file, &mut buffer[..limit], offset)
            .map_err(|error| io_error(format!("hash runtime file {}", path.display()), error))?;
        if count == 0 {
            return Err(error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged,
                format!("runtime file shortened while hashing: {}", path.display()),
            ));
        }
        digest.update(&buffer[..count]);
        offset += count as u64;
    }
    let mut extra = [0_u8; 1];
    if rustix::io::pread(file, &mut extra, size)
        .map_err(|error| io_error(format!("bound runtime file {}", path.display()), error))?
        != 0
    {
        return Err(error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged,
            format!("runtime file grew while hashing: {}", path.display()),
        ));
    }
    Ok(digest.finalize().into())
}

fn read_exact_file(
    file: &File,
    size: u64,
    path: &Path,
) -> Result<Vec<u8>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let length = usize::try_from(size).map_err(|_| {
        error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch,
            format!("runtime file is too large to read: {}", path.display()),
        )
    })?;
    let mut bytes = vec![0_u8; length];
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let count = rustix::io::pread(file, &mut bytes[offset..], offset as u64)
            .map_err(|error| io_error(format!("read runtime file {}", path.display()), error))?;
        if count == 0 {
            return Err(changed(format!(
                "runtime file shortened while reading: {}",
                path.display()
            )));
        }
        offset += count;
    }
    Ok(bytes)
}

fn duplicate_directory(
    directory: &File,
    context: &str,
) -> Result<File, RetainedFunctionalRefinementRuntimeErrorV1> {
    openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| io_error(format!("duplicate {context}"), error))
}

fn open_directory_beneath(
    parent: &File,
    name: &OsStr,
    no_cross_device: bool,
) -> Result<File, RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut resolve =
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS;
    if no_cross_device {
        resolve |= ResolveFlags::NO_XDEV;
    }
    openat2(
        parent,
        Path::new(name),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        resolve,
    )
    .map(File::from)
    .map_err(|error| {
        open_error(
            format!("open directory beneath retained parent: {name:?}"),
            error,
        )
    })
}

fn open_regular_beneath(
    parent: &File,
    name: &OsStr,
) -> Result<File, RetainedFunctionalRefinementRuntimeErrorV1> {
    openat2(
        parent,
        Path::new(name),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|error| {
        open_error(
            format!("open file beneath retained parent: {name:?}"),
            error,
        )
    })
}

fn open_symlink_beneath(
    parent: &File,
    name: &OsStr,
) -> Result<File, RetainedFunctionalRefinementRuntimeErrorV1> {
    openat2(
        parent,
        Path::new(name),
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|error| open_error(format!("retain interpreter symlink: {name:?}"), error))
}

fn read_link(
    parent: &File,
    name: &OsStr,
) -> Result<PathBuf, RetainedFunctionalRefinementRuntimeErrorV1> {
    let target = readlinkat(parent, Path::new(name), Vec::new())
        .map_err(|error| io_error(format!("read interpreter symlink: {name:?}"), error))?;
    Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
}

struct MutationJournalV2 {
    descriptor: OwnedFd,
}

impl MutationJournalV2 {
    fn new() -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)
            .map(|descriptor| Self { descriptor })
            .map_err(|error| io_error("create runtime mutation journal", error))
    }

    fn watch(
        &self,
        directory: &File,
        path: &Path,
    ) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let flags = inotify::WatchFlags::ATTRIB
            | inotify::WatchFlags::CLOSE_WRITE
            | inotify::WatchFlags::CREATE
            | inotify::WatchFlags::DELETE
            | inotify::WatchFlags::DELETE_SELF
            | inotify::WatchFlags::MODIFY
            | inotify::WatchFlags::MOVE_SELF
            | inotify::WatchFlags::MOVED_FROM
            | inotify::WatchFlags::MOVED_TO
            | inotify::WatchFlags::EXCL_UNLINK
            | inotify::WatchFlags::ONLYDIR;
        inotify::add_watch(&self.descriptor, &descriptor_path, flags)
            .map(|_| ())
            .map_err(|error| {
                io_error(
                    format!("watch runtime directory {}", display_relative(path)),
                    error,
                )
            })
    }

    fn ensure_clean(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        let mut storage = [std::mem::MaybeUninit::uninit(); 16 * 1024];
        let mut reader = inotify::Reader::new(&self.descriptor, &mut storage);
        match reader.next() {
            Err(rustix::io::Errno::AGAIN) => Ok(()),
            Err(error_value) => Err(io_error("read runtime mutation journal", error_value)),
            Ok(event) => Err(changed(format!(
                "runtime mutation journal recorded watch {} flags {:?}",
                event.wd(),
                event.events()
            ))),
        }
    }
}

fn open_error(
    context: impl Into<String>,
    value: rustix::io::Errno,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    let kind = if matches!(value, rustix::io::Errno::LOOP | rustix::io::Errno::XDEV) {
        RetainedFunctionalRefinementRuntimeErrorKindV1::SymlinkOrTraversal
    } else if matches!(value, rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) {
        RetainedFunctionalRefinementRuntimeErrorKindV1::ObjectType
    } else {
        RetainedFunctionalRefinementRuntimeErrorKindV1::Io
    };
    error(kind, format!("{}: {value}", context.into()))
}

fn io_error(
    context: impl Into<String>,
    value: rustix::io::Errno,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::Io,
        format!("{}: {value}", context.into()),
    )
}

fn io_std_error(
    context: impl Into<String>,
    value: std::io::Error,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::Io,
        format!("{}: {value}", context.into()),
    )
}

fn changed(detail: impl Into<String>) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged,
        detail,
    )
}

fn error(
    kind: RetainedFunctionalRefinementRuntimeErrorKindV1,
    detail: impl Into<String>,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    RetainedFunctionalRefinementRuntimeErrorV1::new(kind, detail)
}

fn display_relative(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsFd;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::process::Command;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    use super::ManifestV2;
    use super::*;

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);
    static SYNTHETIC_MANIFEST: &[u8] = b"synthetic-functional-refinement-runtime-v1\n";

    struct TestClosure {
        root: PathBuf,
        outside: Vec<PathBuf>,
        manifest: ManifestV2,
    }

    impl TestClosure {
        fn new() -> Self {
            // A concurrent fork would duplicate a fixture writer and its later
            // exec-close would look like a post-retention CLOSE_WRITE event.
            let _process_guard = RUNTIME_CLOSURE_PROCESS_TEST_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let root = std::env::temp_dir().join(format!(
                "fe2o3-functional-refinement-runtime-v1-{}-{}",
                std::process::id(),
                NEXT_TEST.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            for directory in ["bin", "empty", "lib", "lib/nested"] {
                fs::create_dir(root.join(directory)).unwrap();
            }
            let inputs = [
                ("bin/rust_verify", b"rust-verify-v2".as_slice()),
                ("lib/data", b"vstd-data-v2".as_slice()),
                ("lib/nested/target", b"rust-target-v2".as_slice()),
            ];
            let files = inputs
                .iter()
                .map(|(path, bytes)| {
                    fs::write(root.join(path), bytes).unwrap();
                    fs::set_permissions(root.join(path), fs::Permissions::from_mode(0o444))
                        .unwrap();
                    FileSpecV2 {
                        path: PathBuf::from(path),
                        mode: 0o444,
                        size: Some(bytes.len() as u64),
                        sha256: Sha256::digest(bytes).into(),
                    }
                })
                .collect();
            fs::write(
                root.join(FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME),
                SYNTHETIC_MANIFEST,
            )
            .unwrap();
            fs::set_permissions(
                root.join(FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME),
                fs::Permissions::from_mode(0o444),
            )
            .unwrap();
            for directory in ["bin", "empty", "lib/nested", "lib"] {
                fs::set_permissions(root.join(directory), fs::Permissions::from_mode(0o555))
                    .unwrap();
            }
            fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
            let directories = ["bin", "empty", "lib", "lib/nested"]
                .into_iter()
                .map(|path| DirectorySpecV2 {
                    path: PathBuf::from(path),
                    mode: 0o555,
                })
                .collect();
            Self {
                root,
                outside: Vec::new(),
                manifest: ManifestV2::synthetic(SYNTHETIC_MANIFEST, directories, files),
            }
        }

        fn open(
            &self,
        ) -> Result<RetainedRuntimeClosureV2, RetainedFunctionalRefinementRuntimeErrorV1> {
            RetainedRuntimeClosureV2::open_for_test(&self.root, &self.manifest)
        }

        fn make_parent_writable(&self, relative: &str) {
            fs::set_permissions(self.root.join(relative), fs::Permissions::from_mode(0o755))
                .unwrap();
        }

        fn seal_directory(&self, relative: &str) {
            fs::set_permissions(self.root.join(relative), fs::Permissions::from_mode(0o555))
                .unwrap();
        }
    }

    impl Drop for TestClosure {
        fn drop(&mut self) {
            make_test_closure_removable(&self.root);
            let _ = fs::remove_dir_all(&self.root);
            for path in &self.outside {
                if fs::remove_file(path).is_err() {
                    make_test_closure_removable(path);
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
    }

    fn make_test_closure_removable(root: &Path) {
        for directory in ["", "bin", "empty", "lib", "lib/nested"] {
            let path = root.join(directory);
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_dir()) {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
            }
        }
    }

    #[test]
    fn exact_synthetic_closure_is_retained_and_revalidated() {
        let tree = TestClosure::new();
        let retained = tree.open().unwrap();
        retained.revalidate().unwrap();

        let assert_close_on_exec = |descriptor: &dyn AsFd| {
            assert!(
                rustix::io::fcntl_getfd(descriptor)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
        };
        for anchor in &retained.path_anchors {
            assert_close_on_exec(&anchor.file);
        }
        for directory in retained.directories.values() {
            assert_close_on_exec(&directory.file);
        }
        for file in &retained.files {
            assert_close_on_exec(&file.file);
        }
        assert_close_on_exec(&retained.journal.descriptor);
    }

    #[test]
    fn root_symlink_is_rejected_without_following_it() {
        let mut tree = TestClosure::new();
        let link = tree.root.with_extension("link");
        symlink(&tree.root, &link).unwrap();
        tree.outside.push(link.clone());
        let error = RetainedRuntimeClosureV2::open_for_test(&link, &tree.manifest).unwrap_err();
        assert!(matches!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ObjectType
                | RetainedFunctionalRefinementRuntimeErrorKindV1::SymlinkOrTraversal
        ));
    }

    #[test]
    fn expected_leaf_symlink_is_rejected() {
        let mut tree = TestClosure::new();
        let outside = tree.root.with_extension("outside");
        fs::write(&outside, b"rust-verify-v2").unwrap();
        tree.outside.push(outside.clone());
        tree.make_parent_writable("bin");
        fs::remove_file(tree.root.join("bin/rust_verify")).unwrap();
        symlink(&outside, tree.root.join("bin/rust_verify")).unwrap();
        tree.seal_directory("bin");
        let error = tree.open().unwrap_err();
        assert!(matches!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ObjectType
                | RetainedFunctionalRefinementRuntimeErrorKindV1::SymlinkOrTraversal
        ));
    }

    #[test]
    fn extra_and_missing_entries_are_rejected() {
        let tree = TestClosure::new();
        tree.make_parent_writable("empty");
        fs::write(tree.root.join("empty/extra"), b"extra").unwrap();
        tree.seal_directory("empty");
        assert_eq!(
            tree.open().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::InventoryMismatch
        );

        let tree = TestClosure::new();
        tree.make_parent_writable("lib/nested");
        fs::remove_file(tree.root.join("lib/nested/target")).unwrap();
        tree.seal_directory("lib/nested");
        assert_eq!(
            tree.open().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::InventoryMismatch
        );
    }

    #[test]
    fn digest_and_hardlink_substitutions_are_rejected() {
        let tree = TestClosure::new();
        tree.make_parent_writable("lib");
        fs::set_permissions(
            tree.root.join("lib/data"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(tree.root.join("lib/data"), b"attacker-data").unwrap();
        fs::set_permissions(
            tree.root.join("lib/data"),
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        tree.seal_directory("lib");
        assert_eq!(
            tree.open().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ContentMismatch
        );

        let mut tree = TestClosure::new();
        let outside = tree.root.with_extension("hardlink");
        fs::hard_link(tree.root.join("lib/data"), &outside).unwrap();
        tree.outside.push(outside);
        assert_eq!(
            tree.open().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Protection
        );
    }

    #[test]
    fn persistent_and_transient_content_mutation_revoke_the_lease() {
        let tree = TestClosure::new();
        let retained = tree.open().unwrap();
        fs::set_permissions(
            tree.root.join("lib/data"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(tree.root.join("lib/data"), b"persistent-evil").unwrap();
        assert_eq!(
            retained.revalidate().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged
        );

        let tree = TestClosure::new();
        let retained = tree.open().unwrap();
        let path = tree.root.join("lib/data");
        let original = fs::read(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        fs::write(&path, b"transient-evil").unwrap();
        fs::write(&path, original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            retained.revalidate().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged
        );
    }

    #[test]
    fn same_bytes_leaf_replacement_and_root_replacement_revoke_the_lease() {
        let tree = TestClosure::new();
        let retained = tree.open().unwrap();
        tree.make_parent_writable("bin");
        let path = tree.root.join("bin/rust_verify");
        let displaced = tree.root.join("bin/displaced");
        fs::rename(&path, &displaced).unwrap();
        fs::write(&path, b"rust-verify-v2").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            retained.revalidate().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged
        );

        let mut tree = TestClosure::new();
        let retained = tree.open().unwrap();
        let displaced_root = tree.root.with_extension("displaced");
        fs::rename(&tree.root, &displaced_root).unwrap();
        tree.outside.push(displaced_root);
        assert_eq!(
            retained.revalidate().unwrap_err().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged
        );
    }

    #[test]
    fn repeated_parallel_setup_is_clean_and_post_lease_same_byte_writes_are_recorded() {
        const REPETITIONS: usize = 64;

        let start = Barrier::new(2);
        thread::scope(|scope| {
            scope.spawn(|| {
                start.wait();
                for _ in 0..REPETITIONS {
                    let _process_guard = RUNTIME_CLOSURE_PROCESS_TEST_LOCK
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let mut command = Command::new("/bin/true");
                    assert!(
                        crate::executor::status_artifact_coordinated_child(&mut command)
                            .unwrap()
                            .success()
                    );
                }
            });

            start.wait();
            for _ in 0..REPETITIONS {
                let tree = TestClosure::new();
                let retained = tree.open().unwrap();
                retained.journal.ensure_clean().unwrap();

                let path = tree.root.join("lib/data");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                fs::write(&path, b"vstd-data-v2").unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
                let error = retained.journal.ensure_clean().unwrap_err();
                assert_eq!(
                    error.kind(),
                    RetainedFunctionalRefinementRuntimeErrorKindV1::ClosureChanged
                );
                assert!(
                    error
                        .to_string()
                        .contains("runtime mutation journal recorded")
                );
            }
        });
    }

    #[test]
    fn object_snapshots_cover_owner_group_links_and_nanosecond_times() {
        let tree = TestClosure::new();
        let metadata = fs::metadata(tree.root.join("lib/data")).unwrap();
        let retained = tree.open().unwrap();
        let snapshot = retained
            .files
            .iter()
            .find(|file| file.path == Path::new("lib/data"))
            .unwrap()
            .snapshot;
        assert_eq!(snapshot.owner, metadata.uid());
        assert_eq!(snapshot.group, metadata.gid());
        assert_eq!(snapshot.links, metadata.nlink());
        assert_eq!(snapshot.modified_nanoseconds, metadata.mtime_nsec());
        assert_eq!(snapshot.changed_nanoseconds, metadata.ctime_nsec());
    }

    #[test]
    fn generated_proof_source_is_exactly_sealed_and_immutable() {
        let source = CanonicalGeneratedVerusProofInputV3::new(
            b"verus! { proof fn generated() {} }\n".to_vec(),
        )
        .unwrap();
        let sealed = SealedGeneratedProofSourceV3::create(&source).unwrap();
        sealed.revalidate(&source).unwrap();
        assert_eq!(
            fcntl_get_seals(&sealed.file).unwrap(),
            GENERATED_PROOF_SOURCE_SEALS
        );
        assert_eq!(
            rustix::fs::fcntl_getfl(&sealed.file).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        assert_eq!(
            read_exact_file(
                &sealed.file,
                source.byte_len(),
                Path::new("generated-proof-v3.rs")
            )
            .unwrap(),
            source.source()
        );
        assert!(rustix::io::pwrite(&sealed.file, b"x", 0).is_err());
        sealed.revalidate(&source).unwrap();
    }
}
