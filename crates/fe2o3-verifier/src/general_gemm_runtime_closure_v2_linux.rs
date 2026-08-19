use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rustix::fs::{
    AtFlags, FileType, MemfdFlags, Mode, OFlags, ResolveFlags, SealFlags, fcntl_add_seals,
    fcntl_get_seals, fstat, inotify, memfd_create, open, openat, openat2, readlinkat, statat,
};
use sha2::{Digest, Sha256};

use crate::authenticated_verus_execution_v2::{
    ADDRESS_SPACE_LIMIT_V2, BoundedProcessGroupFailureV2, CORE_LIMIT_V2, DATA_LIMIT_V2,
    FILE_LIMIT_V2, supervise_bounded_process_group_v2, validate_controller_security_v2,
};

use super::{
    EntryKindV2, FileSpecV2, GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME,
    GeneralGemmRuntimeClosureErrorKindV2, GeneralGemmRuntimeClosureErrorV2,
    GeneralGemmRuntimeProcessOutputV2, InterpreterSpecV2, MAX_TARGET_FILE_BYTES, ManifestV2,
};

const MAX_DIRECTORY_ENTRIES: usize = 256;
const MAX_TOTAL_RUNTIME_BYTES: u64 = 1024 * 1024 * 1024;
const PROOF_INPUT_BYTES_LIMIT: usize = 16 * 1024 * 1024;

const RUST_VERIFY_FD: RawFd = 180;
const Z3_FD: RawFd = 181;
const DIST_DIRECTORY_FD: RawFd = 182;
const TOOLCHAIN_DIRECTORY_FD: RawFd = 183;
const TOOLCHAIN_LIB_DIRECTORY_FD: RawFd = 184;
const SYSTEM_LIB_DIRECTORY_FD: RawFd = 185;
const WRAPPER_SOURCE_FD: RawFd = 187;
const MODEL_SOURCE_FD: RawFd = 188;
const PROOF_SOURCE_FD: RawFd = 189;

const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
const F_SETFD: i32 = 2;
const FD_CLOEXEC: i32 = 1;
const PR_SET_NO_NEW_PRIVS: i32 = 38;
const RLIMIT_FSIZE: i32 = 1;
const RLIMIT_DATA: i32 = 2;
const RLIMIT_CORE: i32 = 4;
const RLIMIT_AS: i32 = 9;

#[repr(C)]
struct ResourceLimitV2 {
    current: u64,
    maximum: u64,
}

unsafe extern "C" {
    fn close_range(first: u32, last: u32, flags: u32) -> i32;
    fn dup2(old_descriptor: i32, new_descriptor: i32) -> i32;
    fn fchdir(descriptor: i32) -> i32;
    fn fcntl(descriptor: i32, command: i32, ...) -> i32;
    fn prctl(option: i32, ...) -> i32;
    fn setrlimit(resource: i32, limit: *const ResourceLimitV2) -> i32;
    fn umask(mask: u32) -> u32;
}

pub(super) struct SealedProofInputV2 {
    wrapper: File,
    model: File,
    proof: File,
}

impl SealedProofInputV2 {
    pub(super) fn new(
        wrapper: &[u8],
        model: &[u8],
        proof: &[u8],
    ) -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
        let total = wrapper
            .len()
            .checked_add(model.len())
            .and_then(|value| value.checked_add(proof.len()))
            .filter(|value| *value <= PROOF_INPUT_BYTES_LIMIT)
            .ok_or_else(|| {
                error(
                    GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
                    "sealed proof inputs exceed their total byte bound",
                )
            })?;
        debug_assert_eq!(total, wrapper.len() + model.len() + proof.len());
        Ok(Self {
            wrapper: create_sealed_input("fe2o3-general-gemm-wrapper-v2", wrapper)?,
            model: create_sealed_input("fe2o3-general-gemm-model-v2", model)?,
            proof: create_sealed_input("fe2o3-general-gemm-proof-v2", proof)?,
        })
    }

    pub(super) fn revalidate(
        &self,
        expected_identity: [u8; 32],
    ) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
        let wrapper = read_sealed_input(&self.wrapper, "proof wrapper")?;
        let model = read_sealed_input(&self.model, "proof model")?;
        let proof = read_sealed_input(&self.proof, "proof body")?;
        if super::proof_input_identity(&wrapper, &model, &proof) != expected_identity {
            return Err(error(
                GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
                "sealed proof-input identity changed",
            ));
        }
        Ok(())
    }
}

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
    fn capture(file: &File, context: &str) -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
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
    ) -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
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
    ) -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
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
        let installed_manifest = open_regular_beneath(
            &root_directory.file,
            OsStr::new(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME),
        )?;
        let manifest_spec = FileSpecV2 {
            path: PathBuf::from(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME),
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
                GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
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
                        GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
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

    pub(super) fn revalidate(&self) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
                    GeneralGemmRuntimeClosureErrorKindV2::InventoryMismatch,
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

    fn required_file(&self, path: &Path) -> Result<&File, GeneralGemmRuntimeClosureErrorV2> {
        self.files
            .iter()
            .find(|retained| retained.path == path)
            .map(|retained| &retained.file)
            .ok_or_else(|| {
                error(
                    GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
                    format!("reviewed runtime manifest lacks {}", path.display()),
                )
            })
    }

    fn required_directory(&self, path: &Path) -> Result<&File, GeneralGemmRuntimeClosureErrorV2> {
        self.directories
            .get(path)
            .map(|retained| &retained.file)
            .ok_or_else(|| {
                error(
                    GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
                    format!("reviewed runtime manifest lacks {}", path.display()),
                )
            })
    }

    #[cfg(test)]
    fn open_for_test(
        root: &Path,
        manifest: &ManifestV2,
    ) -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
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

pub(super) fn execute_rust_verify(
    runtime: &RetainedRuntimeClosureV2,
    input: &SealedProofInputV2,
    deadline: Instant,
    output_limit: usize,
) -> Result<GeneralGemmRuntimeProcessOutputV2, GeneralGemmRuntimeClosureErrorV2> {
    validate_controller_security_v2().map_err(|failure| {
        process_error(
            format!("authenticated controller preflight failed: {failure}"),
            GeneralGemmRuntimeClosureErrorKindV2::Process,
        )
    })?;
    if output_limit == 0 {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge,
            "proof output bound is zero",
        ));
    }
    if Instant::now() >= deadline {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::TimedOut,
            "general GEMM proof deadline elapsed before spawn",
        ));
    }

    let rust_verify = runtime.required_file(Path::new("dist/rust_verify"))?;
    let z3 = runtime.required_file(Path::new("dist/z3"))?;
    let dist = runtime.required_directory(Path::new("dist"))?;
    let toolchain = runtime.required_directory(Path::new("toolchain"))?;
    let toolchain_lib = runtime.required_directory(Path::new("toolchain/lib"))?;
    let system_lib = runtime.required_directory(Path::new("system-lib"))?;
    let empty = runtime.required_directory(Path::new("empty"))?;

    // Normalize all source descriptors above the fixed child map. This prevents an ambient
    // descriptor allocation pattern from making one dup2 destination overwrite a later source.
    let sources = duplicate_child_sources([
        rust_verify,
        z3,
        dist,
        toolchain,
        toolchain_lib,
        system_lib,
        &input.wrapper,
        &input.model,
        &input.proof,
        empty,
    ])?;
    let inherited = [
        (sources[0].as_raw_fd(), RUST_VERIFY_FD, true),
        (sources[1].as_raw_fd(), Z3_FD, false),
        (sources[2].as_raw_fd(), DIST_DIRECTORY_FD, false),
        (sources[3].as_raw_fd(), TOOLCHAIN_DIRECTORY_FD, false),
        (sources[4].as_raw_fd(), TOOLCHAIN_LIB_DIRECTORY_FD, false),
        (sources[5].as_raw_fd(), SYSTEM_LIB_DIRECTORY_FD, false),
        (sources[6].as_raw_fd(), WRAPPER_SOURCE_FD, false),
        (sources[7].as_raw_fd(), MODEL_SOURCE_FD, false),
        (sources[8].as_raw_fd(), PROOF_SOURCE_FD, false),
    ];
    let empty_descriptor = sources[9].as_raw_fd();

    let mut command = Command::new(format!("/proc/self/fd/{RUST_VERIFY_FD}"));
    command
        .arg(format!("/proc/self/fd/{WRAPPER_SOURCE_FD}"))
        .args([
            "--crate-type",
            "lib",
            "--triggers-mode",
            "silent",
            "--no-cheating",
            "--num-threads",
            "1",
            "--sysroot",
        ])
        .arg(format!("/proc/self/fd/{TOOLCHAIN_DIRECTORY_FD}"))
        .env_clear()
        .env("VERUS_ROOT", format!("/proc/self/fd/{DIST_DIRECTORY_FD}"))
        .env("VERUS_Z3_PATH", format!("/proc/self/fd/{Z3_FD}"))
        .env(
            "LD_LIBRARY_PATH",
            format!(
                "/proc/self/fd/{TOOLCHAIN_LIB_DIRECTORY_FD}:/proc/self/fd/{SYSTEM_LIB_DIRECTORY_FD}"
            ),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);

    // SAFETY: the callback uses only async-signal-safe syscalls and raw descriptors captured
    // above. No allocation, lock, environment lookup, or path lookup occurs after fork.
    unsafe {
        command.pre_exec(move || prepare_proof_child(&inherited, empty_descriptor));
    }
    let mut child = command.spawn().map_err(|source| {
        process_error(
            format!("spawn retained rust_verify: {source}"),
            GeneralGemmRuntimeClosureErrorKindV2::Process,
        )
    })?;
    let output = supervise_bounded_process_group_v2(&mut child, deadline, output_limit).map_err(
        |failure| {
            let kind = match failure.kind() {
                BoundedProcessGroupFailureV2::TimedOut => {
                    GeneralGemmRuntimeClosureErrorKindV2::TimedOut
                }
                BoundedProcessGroupFailureV2::OutputTooLarge => {
                    GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge
                }
                BoundedProcessGroupFailureV2::Process => {
                    GeneralGemmRuntimeClosureErrorKindV2::Process
                }
            };
            process_error(failure.detail(), kind)
        },
    )?;
    Ok(GeneralGemmRuntimeProcessOutputV2 {
        exit_code: output.exit_code,
        signal: output.signal,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn duplicate_child_sources(
    files: [&File; 10],
) -> Result<[OwnedFd; 10], GeneralGemmRuntimeClosureErrorV2> {
    let mut next = 200;
    let mut descriptors = Vec::with_capacity(files.len());
    for file in files {
        let descriptor = rustix::io::fcntl_dupfd_cloexec(file, next)
            .map_err(|source| io_error("normalize proof child descriptor", source))?;
        next = descriptor.as_raw_fd().checked_add(1).ok_or_else(|| {
            error(
                GeneralGemmRuntimeClosureErrorKindV2::Process,
                "proof child descriptor space exhausted",
            )
        })?;
        descriptors.push(descriptor);
    }
    descriptors.try_into().map_err(|_| {
        error(
            GeneralGemmRuntimeClosureErrorKindV2::Process,
            "proof child descriptor normalization was incomplete",
        )
    })
}

fn create_sealed_input(name: &str, bytes: &[u8]) -> Result<File, GeneralGemmRuntimeClosureErrorV2> {
    let descriptor = memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
        .map_err(|source| io_error(format!("create sealed {name}"), source))?;
    let mut file = File::from(descriptor);
    file.write_all(bytes)
        .map_err(|source| io_std_error(format!("write sealed {name}"), source))?;
    let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    fcntl_add_seals(&file, required).map_err(|source| io_error(format!("seal {name}"), source))?;
    require_exact_input_seals(&file, name)?;
    Ok(file)
}

fn require_exact_input_seals(
    file: &File,
    context: &str,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    let actual = fcntl_get_seals(file)
        .map_err(|source| io_error(format!("inspect seals for {context}"), source))?;
    if actual != required {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
            format!("sealed proof input has unexpected seals: {context}"),
        ));
    }
    Ok(())
}

fn read_sealed_input(
    file: &File,
    context: &str,
) -> Result<Vec<u8>, GeneralGemmRuntimeClosureErrorV2> {
    require_exact_input_seals(file, context)?;
    let metadata = file
        .metadata()
        .map_err(|source| io_std_error(format!("inspect {context}"), source))?;
    let size = usize::try_from(metadata.len())
        .ok()
        .filter(|size| *size <= PROOF_INPUT_BYTES_LIMIT)
        .ok_or_else(|| {
            error(
                GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
                format!("sealed proof input is too large: {context}"),
            )
        })?;
    let mut bytes = vec![0_u8; size];
    let mut offset = 0;
    while offset < bytes.len() {
        let read = rustix::io::pread(file, &mut bytes[offset..], offset as u64)
            .map_err(|source| io_error(format!("read {context}"), source))?;
        if read == 0 {
            return Err(error(
                GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
                format!("sealed proof input shortened while reading: {context}"),
            ));
        }
        offset += read;
    }
    let mut extra = [0_u8; 1];
    if rustix::io::pread(file, &mut extra, size as u64)
        .map_err(|source| io_error(format!("bound {context}"), source))?
        != 0
    {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
            format!("sealed proof input grew while reading: {context}"),
        ));
    }
    require_exact_input_seals(file, context)?;
    Ok(bytes)
}

fn prepare_proof_child(
    inherited: &[(RawFd, RawFd, bool)],
    empty_descriptor: RawFd,
) -> io::Result<()> {
    // SAFETY: close_range only changes close-on-exec flags for descriptors in this process.
    if unsafe { close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for &(source, destination, close_on_exec) in inherited {
        // SAFETY: both values are live integer descriptors captured before fork.
        if unsafe { dup2(source, destination) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fcntl operates on the just-duplicated descriptor and takes an integer flag.
        if unsafe {
            fcntl(
                destination,
                F_SETFD,
                if close_on_exec { FD_CLOEXEC } else { 0 },
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: the retained empty directory descriptor remains open until exec.
    if unsafe { fchdir(empty_descriptor) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: umask has no failure mode and accepts the supplied permission bits.
    unsafe { umask(0o077) };
    // SAFETY: PR_SET_NO_NEW_PRIVS with argument 1 requires no pointer arguments.
    if unsafe { prctl(PR_SET_NO_NEW_PRIVS, 1_i32, 0_i32, 0_i32, 0_i32) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for (resource, value) in [
        (RLIMIT_AS, ADDRESS_SPACE_LIMIT_V2),
        (RLIMIT_DATA, DATA_LIMIT_V2),
        (RLIMIT_FSIZE, FILE_LIMIT_V2),
        (RLIMIT_CORE, CORE_LIMIT_V2),
    ] {
        let limit = ResourceLimitV2 {
            current: value,
            maximum: value,
        };
        // SAFETY: setrlimit reads one initialized fixed-layout value during pre-exec setup.
        if unsafe { setrlimit(resource, &limit) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn process_error(
    detail: impl Into<String>,
    kind: GeneralGemmRuntimeClosureErrorKindV2,
) -> GeneralGemmRuntimeClosureErrorV2 {
    error(kind, detail)
}

impl RetainedInterpreterV2 {
    fn revalidate(&self) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
    fn revalidate(&self) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<RetainedInterpreterV2, GeneralGemmRuntimeClosureErrorV2> {
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
            GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal,
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
) -> Result<RetainedSymlinkV2, GeneralGemmRuntimeClosureErrorV2> {
    let parent_path = path.parent().ok_or_else(|| {
        error(
            GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal,
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
            GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal,
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
) -> Result<Vec<PathAnchorV2>, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<Vec<PathAnchorV2>, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    if snapshot.owner != 0 || snapshot.group != 0 || snapshot.permissions() & 0o022 != 0 {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::Protection,
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
) -> Result<ObjectSnapshotV2, GeneralGemmRuntimeClosureErrorV2> {
    let snapshot = ObjectSnapshotV2::capture(file, context)?;
    if snapshot.file_type() != FileType::Directory {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::ObjectType,
            format!("{context} is not a directory"),
        ));
    }
    if snapshot.owner != policy.owner
        || snapshot.group != policy.group
        || snapshot.permissions() != expected_mode
    {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::Protection,
            format!("{context} ownership or mode differs"),
        ));
    }
    Ok(snapshot)
}

fn retain_file(
    file: File,
    specification: &FileSpecV2,
    policy: &ProtectionPolicyV2,
) -> Result<RetainedFileV2, GeneralGemmRuntimeClosureErrorV2> {
    let context = format!("runtime file {}", specification.path.display());
    let before = ObjectSnapshotV2::capture(&file, &context)?;
    if before.file_type() != FileType::RegularFile {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::ObjectType,
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
            GeneralGemmRuntimeClosureErrorKindV2::Protection,
            format!("{context} ownership, links, mode, or size differs"),
        ));
    }
    let size = before.size as u64;
    if specification.size.is_some_and(|expected| expected != size)
        || specification.size.is_none() && size > MAX_TARGET_FILE_BYTES
    {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
            format!("{context} size differs"),
        ));
    }
    let digest = hash_exact_file(&file, size, &specification.path)?;
    let after = ObjectSnapshotV2::capture(&file, &context)?;
    if before != after || digest != specification.sha256 {
        return Err(error(
            GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    for directory in directories.values() {
        let actual = scan_inventory(&directory.file, &directory.path)?;
        if actual != directory.expected_children {
            return Err(error(
                GeneralGemmRuntimeClosureErrorKindV2::InventoryMismatch,
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
) -> Result<BTreeMap<PathBuf, EntryKindV2>, GeneralGemmRuntimeClosureErrorV2> {
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
                GeneralGemmRuntimeClosureErrorKindV2::InventoryMismatch,
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
                    GeneralGemmRuntimeClosureErrorKindV2::ObjectType,
                    format!("runtime inventory contains a non-file entry: {name:?}"),
                ));
            }
        };
        if actual.insert(PathBuf::from(name), kind).is_some() {
            return Err(error(
                GeneralGemmRuntimeClosureErrorKindV2::InventoryMismatch,
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
) -> Result<[u8; 32], GeneralGemmRuntimeClosureErrorV2> {
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
                GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged,
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
            GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged,
            format!("runtime file grew while hashing: {}", path.display()),
        ));
    }
    Ok(digest.finalize().into())
}

fn read_exact_file(
    file: &File,
    size: u64,
    path: &Path,
) -> Result<Vec<u8>, GeneralGemmRuntimeClosureErrorV2> {
    let length = usize::try_from(size).map_err(|_| {
        error(
            GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch,
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
) -> Result<File, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<File, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<File, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<File, GeneralGemmRuntimeClosureErrorV2> {
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

fn read_link(parent: &File, name: &OsStr) -> Result<PathBuf, GeneralGemmRuntimeClosureErrorV2> {
    let target = readlinkat(parent, Path::new(name), Vec::new())
        .map_err(|error| io_error(format!("read interpreter symlink: {name:?}"), error))?;
    Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
}

struct MutationJournalV2 {
    descriptor: OwnedFd,
}

impl MutationJournalV2 {
    fn new() -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
        inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)
            .map(|descriptor| Self { descriptor })
            .map_err(|error| io_error("create runtime mutation journal", error))
    }

    fn watch(&self, directory: &File, path: &Path) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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

    fn ensure_clean(&self) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
) -> GeneralGemmRuntimeClosureErrorV2 {
    let kind = if matches!(value, rustix::io::Errno::LOOP | rustix::io::Errno::XDEV) {
        GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal
    } else if matches!(value, rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) {
        GeneralGemmRuntimeClosureErrorKindV2::ObjectType
    } else {
        GeneralGemmRuntimeClosureErrorKindV2::Io
    };
    error(kind, format!("{}: {value}", context.into()))
}

fn io_error(
    context: impl Into<String>,
    value: rustix::io::Errno,
) -> GeneralGemmRuntimeClosureErrorV2 {
    error(
        GeneralGemmRuntimeClosureErrorKindV2::Io,
        format!("{}: {value}", context.into()),
    )
}

fn io_std_error(
    context: impl Into<String>,
    value: std::io::Error,
) -> GeneralGemmRuntimeClosureErrorV2 {
    error(
        GeneralGemmRuntimeClosureErrorKindV2::Io,
        format!("{}: {value}", context.into()),
    )
}

fn changed(detail: impl Into<String>) -> GeneralGemmRuntimeClosureErrorV2 {
    error(GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged, detail)
}

fn error(
    kind: GeneralGemmRuntimeClosureErrorKindV2,
    detail: impl Into<String>,
) -> GeneralGemmRuntimeClosureErrorV2 {
    GeneralGemmRuntimeClosureErrorV2::new(kind, detail)
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
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::general_gemm_runtime_closure_v2::{DirectorySpecV2, ManifestV2};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);
    static SYNTHETIC_MANIFEST: &[u8] = b"synthetic-general-gemm-runtime-v2\n";

    struct TestClosure {
        root: PathBuf,
        outside: Vec<PathBuf>,
        manifest: ManifestV2,
    }

    impl TestClosure {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "fe2o3-general-gemm-runtime-v2-{}-{}",
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
                root.join(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME),
                SYNTHETIC_MANIFEST,
            )
            .unwrap();
            fs::set_permissions(
                root.join(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME),
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

        fn open(&self) -> Result<RetainedRuntimeClosureV2, GeneralGemmRuntimeClosureErrorV2> {
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
            for directory in ["", "bin", "empty", "lib", "lib/nested"] {
                let _ = fs::set_permissions(
                    self.root.join(directory),
                    fs::Permissions::from_mode(0o755),
                );
            }
            let _ = fs::remove_dir_all(&self.root);
            for path in &self.outside {
                let _ = fs::remove_file(path);
                let _ = fs::remove_dir_all(path);
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
            GeneralGemmRuntimeClosureErrorKindV2::ObjectType
                | GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal
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
            GeneralGemmRuntimeClosureErrorKindV2::ObjectType
                | GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal
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
            GeneralGemmRuntimeClosureErrorKindV2::InventoryMismatch
        );

        let tree = TestClosure::new();
        tree.make_parent_writable("lib/nested");
        fs::remove_file(tree.root.join("lib/nested/target")).unwrap();
        tree.seal_directory("lib/nested");
        assert_eq!(
            tree.open().unwrap_err().kind(),
            GeneralGemmRuntimeClosureErrorKindV2::InventoryMismatch
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
            GeneralGemmRuntimeClosureErrorKindV2::ContentMismatch
        );

        let mut tree = TestClosure::new();
        let outside = tree.root.with_extension("hardlink");
        fs::hard_link(tree.root.join("lib/data"), &outside).unwrap();
        tree.outside.push(outside);
        assert_eq!(
            tree.open().unwrap_err().kind(),
            GeneralGemmRuntimeClosureErrorKindV2::Protection
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
            GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged
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
            GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged
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
            GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged
        );

        let mut tree = TestClosure::new();
        let retained = tree.open().unwrap();
        let displaced_root = tree.root.with_extension("displaced");
        fs::rename(&tree.root, &displaced_root).unwrap();
        tree.outside.push(displaced_root);
        assert_eq!(
            retained.revalidate().unwrap_err().kind(),
            GeneralGemmRuntimeClosureErrorKindV2::ClosureChanged
        );
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
    fn sealed_proof_inputs_are_exact_immutable_close_on_exec_objects() {
        let wrapper = b"wrapper-v2\n";
        let model = b"model-v2\n";
        let proof = b"proof-v2\n";
        let expected = super::super::proof_input_identity(wrapper, model, proof);
        let sealed = SealedProofInputV2::new(wrapper, model, proof).unwrap();
        sealed.revalidate(expected).unwrap();
        for (file, bytes) in [
            (&sealed.wrapper, wrapper.as_slice()),
            (&sealed.model, model.as_slice()),
            (&sealed.proof, proof.as_slice()),
        ] {
            assert_eq!(read_sealed_input(file, "test input").unwrap(), bytes);
            assert!(
                rustix::io::fcntl_getfd(file)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
            assert!(rustix::io::pwrite(file, b"x", 0).is_err());
        }
        assert!(sealed.revalidate([0; 32]).is_err());
    }

    #[test]
    fn proof_child_boundary_clears_environment_and_installs_only_explicit_inputs() {
        let tree = TestClosure::new();
        let empty = File::open(tree.root.join("empty")).unwrap();
        let source = create_sealed_input("fe2o3-proof-child-test", b"sealed-input\n").unwrap();
        let normalized = rustix::io::fcntl_dupfd_cloexec(&source, 200).unwrap();
        let source_descriptor = normalized.as_raw_fd();
        let empty_descriptor = empty.as_raw_fd();
        let empty_path = tree.root.join("empty");
        let inherited = [(source_descriptor, WRAPPER_SOURCE_FD, false)];
        let script = format!(
            "test \"$ONLY_EXACT_ENV\" = retained && \
             test -z \"${{HOME+x}}\" && \
             test \"$(pwd -P)\" = \"{}\" && \
             test \"$(/usr/bin/cat /proc/self/fd/{WRAPPER_SOURCE_FD})\" = sealed-input && \
             test ! -e /proc/self/fd/{source_descriptor} && \
             test \"$(umask)\" = 0077 && \
             /usr/bin/grep -q '^NoNewPrivs:[[:space:]]*1$' /proc/self/status && \
             printf prepared",
            empty_path.display()
        );
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", &script])
            .env_clear()
            .env("ONLY_EXACT_ENV", "retained")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: this is the production async-signal-safe child preparation callback over
        // descriptors retained for the entire spawn operation.
        unsafe {
            command.pre_exec(move || prepare_proof_child(&inherited, empty_descriptor));
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "child preparation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"prepared");
    }
}
