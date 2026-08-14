//! Pre-Cargo host-code policy for authority-bearing kernel compilations.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(target_os = "linux")]
use std::ffi::{CStr, CString, OsStr};
use std::fs::{self, Metadata, OpenOptions};
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::pinned_executable::PinnedExecutable;
use crate::project::CargoProject;

const MAX_METADATA_BYTES: usize = 32 * 1024 * 1024;
const MAX_LOCK_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCE_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SOURCE_TREE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SOURCE_TREE_FILES: usize = 100_000;
const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
const TRUSTED_REGISTRY_BUILD_SCRIPTS: [(&str, &str, &str); 15] = [
    (
        "cap-primitives",
        "4.0.2",
        "9c5c6262db1c26d16dc7bc175a2785b15d8e5e0c02825ffd1be9e20a4bff50f1",
    ),
    (
        "io-extras",
        "0.19.0",
        "41ca4460f88bdeb51e5236f79540483047e935c6386db8ee5242cde7572075d4",
    ),
    (
        "io-lifetimes",
        "2.0.4",
        "41e3c9aefd8e587fce5aaa6b326b7f72babb8509fd97fec0b85923f46d244d7e",
    ),
    (
        "io-lifetimes",
        "3.0.1",
        "f992fc3ac492e187e4a5c5b4d6501e6fcfee4a8ab3ee1983fb1291cdea4d4108",
    ),
    (
        "libc",
        "0.2.189",
        "54f1477836437a81c25bfb0774a700a3043d80a3f20c395429f47f66db34ace3",
    ),
    (
        "num-traits",
        "0.2.19",
        "a70b98cd31b6d7ed51cb8c0d25cbe86b0d61bccd3fcb8775b5369aaeb5f18a7e",
    ),
    (
        "object",
        "0.39.1",
        "5007f85062b9881599bd080205a7bce0e2fffbcc024ea9d6cc2d35c5ab58dc4f",
    ),
    (
        "proc-macro2",
        "1.0.106",
        "09dc7aa3070a182d1a247cb6876af476bcc0db2390908facd93edbf3ede8a03b",
    ),
    (
        "proc-macro2",
        "1.0.107",
        "369ed937912de48187d2b6b9706b3c76128b3c9ca75b4404abf79306ac6db9f1",
    ),
    (
        "quote",
        "1.0.45",
        "8d6c868c0e133b6a257d426993ff53ba648a8fa4cfc805fe480b6c0c89e56638",
    ),
    (
        "quote",
        "1.0.47",
        "600f7d275e1f5f809dd2c6670acf7a4966733eb66983d8650c4d6b1d51184f29",
    ),
    (
        "rustix",
        "1.1.4",
        "62dd547337499a696f957e605c76c0419a2d34d915fcf06554827f955b93994a",
    ),
    (
        "serde_core",
        "1.0.229",
        "cfc95168c497e78cae0bf1dcb1c18302358c3b1831dcd5fbdda49fc51975def1",
    ),
    (
        "serde_json",
        "1.0.151",
        "9fb6b972f5ef5eaf17be5c50854f9a328c5c0c8f9a7b462f51119ec92001f682",
    ),
    (
        "zmij",
        "1.0.23",
        "ee4ed4bdafb98dc92c5a51095290212137f81ffc6cdfae77e9cb540373fb4c11",
    ),
];
const TRUSTED_FE2O3_MACROS_TREE: &str =
    "26c7df0dc0cc81890cd291c61c64d9d24a94adcbd3860d0f9a4fdc9ff2661fc3";
const TRUSTED_FE2O3_HIP_SYS_TREE: &str =
    "fc950a51041eeb74fd756624e3c981fe24d52a6e8b4868da613e5b9a8c499429";

pub(crate) struct AuthorizedKernelClosureV1 {
    snapshot: Vec<u8>,
    source_trees: Vec<ObservedSourceTree>,
    lockfile: ObservedFile,
    mutation_journal: MutationJournal,
}

struct ObservedSourceTree {
    root: PathBuf,
    excluded: Option<PathBuf>,
    digest: [u8; 32],
}

impl ObservedSourceTree {
    fn capture(
        root: PathBuf,
        excluded: Option<PathBuf>,
        mutation_journal: Option<&MutationJournal>,
    ) -> Result<Self, String> {
        let digest = canonical_tree_digest_monitored(&root, excluded.as_deref(), mutation_journal)?;
        Ok(Self {
            root,
            excluded,
            digest,
        })
    }

    fn revalidate(&self) -> Result<(), String> {
        let current = canonical_tree_digest(&self.root, self.excluded.as_deref())?;
        if current != self.digest {
            return Err(format!(
                "authoritative source closure changed after preflight under {}",
                self.root.display()
            ));
        }
        Ok(())
    }
}

struct ObservedFile {
    path: PathBuf,
    digest: [u8; 32],
    size: u64,
}

#[cfg(target_os = "linux")]
struct MutationJournal {
    descriptor: OwnedFd,
    watch_by_path: std::sync::Mutex<BTreeMap<PathBuf, i32>>,
    path_by_watch: std::sync::Mutex<BTreeMap<i32, PathBuf>>,
    excluded: Vec<PathBuf>,
}

#[cfg(target_os = "linux")]
impl MutationJournal {
    fn new(excluded: Vec<PathBuf>) -> Result<Self, String> {
        let descriptor = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if descriptor < 0 {
            return Err(format!(
                "cannot start authoritative source mutation journal: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self {
            descriptor: unsafe { OwnedFd::from_raw_fd(descriptor) },
            watch_by_path: std::sync::Mutex::new(BTreeMap::new()),
            path_by_watch: std::sync::Mutex::new(BTreeMap::new()),
            excluded,
        })
    }

    fn watch_directory(&self, directory: &Path) -> Result<(), String> {
        let mut watch_by_path = self
            .watch_by_path
            .lock()
            .map_err(|_| "authoritative mutation journal path map was poisoned".to_owned())?;
        if watch_by_path.contains_key(directory) {
            return Ok(());
        }
        let path = CString::new(directory.as_os_str().as_bytes()).map_err(|_| {
            format!(
                "authoritative source directory contains a NUL byte: {}",
                directory.display()
            )
        })?;
        let mask = libc::IN_ATTRIB
            | libc::IN_CLOSE_WRITE
            | libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_MODIFY
            | libc::IN_MOVE_SELF
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO
            | libc::IN_DONT_FOLLOW
            | libc::IN_EXCL_UNLINK
            | libc::IN_ONLYDIR;
        let watch =
            unsafe { libc::inotify_add_watch(self.descriptor.as_raw_fd(), path.as_ptr(), mask) };
        if watch < 0 {
            return Err(format!(
                "cannot journal authoritative source directory {}: {}",
                directory.display(),
                std::io::Error::last_os_error()
            ));
        }
        self.path_by_watch
            .lock()
            .map_err(|_| "authoritative mutation journal watch map was poisoned".to_owned())?
            .insert(watch, directory.to_path_buf());
        watch_by_path.insert(directory.to_path_buf(), watch);
        Ok(())
    }

    fn ensure_quiet(&self) -> Result<(), String> {
        let mut storage = [0_u64; 1024];
        loop {
            let bytes = unsafe {
                libc::read(
                    self.descriptor.as_raw_fd(),
                    storage.as_mut_ptr().cast(),
                    std::mem::size_of_val(&storage),
                )
            };
            if bytes < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(format!(
                    "cannot read authoritative source mutation journal: {error}"
                ));
            }
            if bytes == 0 {
                return Err("authoritative source mutation journal closed unexpectedly".to_owned());
            }
            self.reject_events(storage.as_ptr().cast(), bytes as usize)?;
        }
    }

    fn reject_events(&self, bytes: *const u8, length: usize) -> Result<(), String> {
        let paths = self
            .path_by_watch
            .lock()
            .map_err(|_| "authoritative mutation journal watch map was poisoned".to_owned())?;
        let mut offset = 0_usize;
        while offset < length {
            if length - offset < std::mem::size_of::<libc::inotify_event>() {
                return Err(
                    "authoritative source mutation journal returned a partial event".to_owned(),
                );
            }
            let event = unsafe {
                std::ptr::read_unaligned(bytes.add(offset).cast::<libc::inotify_event>())
            };
            let record_length = std::mem::size_of::<libc::inotify_event>()
                .checked_add(event.len as usize)
                .ok_or_else(|| {
                    "authoritative source mutation journal event length overflowed".to_owned()
                })?;
            if record_length > length - offset {
                return Err(
                    "authoritative source mutation journal returned a truncated event".to_owned(),
                );
            }
            if event.mask & libc::IN_Q_OVERFLOW != 0 {
                return Err(
                    "authoritative source mutation journal overflowed; closure authority is denied"
                        .to_owned(),
                );
            }
            let base = paths.get(&event.wd).ok_or_else(|| {
                format!(
                    "authoritative source mutation journal reported unknown watch {}",
                    event.wd
                )
            })?;
            let path = if event.len == 0 {
                base.clone()
            } else {
                let name = unsafe {
                    CStr::from_ptr(
                        bytes
                            .add(offset + std::mem::size_of::<libc::inotify_event>())
                            .cast(),
                    )
                };
                base.join(OsStr::from_bytes(name.to_bytes()))
            };
            if !self
                .excluded
                .iter()
                .any(|excluded| path == *excluded || path.starts_with(excluded))
            {
                return Err(format!(
                    "authoritative source closure mutated after preflight: {} (inotify mask 0x{:x})",
                    path.display(),
                    event.mask
                ));
            }
            offset += record_length;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
struct MutationJournal;

#[cfg(not(target_os = "linux"))]
impl MutationJournal {
    fn new(_excluded: Vec<PathBuf>) -> Result<Self, String> {
        Err("authoritative source mutation journaling requires Linux".to_owned())
    }

    fn watch_directory(&self, _directory: &Path) -> Result<(), String> {
        Err("authoritative source mutation journaling requires Linux".to_owned())
    }

    fn ensure_quiet(&self) -> Result<(), String> {
        Err("authoritative source mutation journaling requires Linux".to_owned())
    }
}

impl ObservedFile {
    fn capture(path: PathBuf, limit: u64) -> Result<Self, String> {
        let (digest, size) = hash_regular_file(&path, limit)?;
        Ok(Self { path, digest, size })
    }

    fn revalidate(&self, limit: u64) -> Result<(), String> {
        let (digest, size) = hash_regular_file(&self.path, limit)?;
        if digest != self.digest || size != self.size {
            return Err(format!(
                "authoritative input changed after preflight: {}",
                self.path.display()
            ));
        }
        Ok(())
    }
}

impl AuthorizedKernelClosureV1 {
    pub(crate) fn observe(
        project: &CargoProject,
        args: &[std::ffi::OsString],
        cargo: &PinnedExecutable,
    ) -> Result<Self, String> {
        let mut command = cargo
            .command()
            .map_err(|error| format!("failed to prepare pinned Cargo metadata: {error}"))?;
        command
            .as_command_mut()
            .args(["metadata", "--format-version", "1"])
            .args(project.authority_metadata_args(args)?)
            .current_dir(project.invocation_dir().child_path());
        let output = command
            .output()
            .map_err(|error| format!("failed to run pinned Cargo metadata: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "could not resolve authoritative Cargo closure: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if output.stdout.is_empty() || output.stdout.len() > MAX_METADATA_BYTES {
            return Err(format!(
                "authoritative Cargo metadata must contain 1 through {MAX_METADATA_BYTES} bytes"
            ));
        }
        let metadata: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("failed to parse authoritative Cargo metadata: {error}"))?;
        Self::from_metadata(&metadata, args, cargo.sha256())
    }

    pub(crate) fn snapshot(&self) -> &[u8] {
        &self.snapshot
    }

    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.mutation_journal.ensure_quiet()?;
        for tree in &self.source_trees {
            tree.revalidate()?;
        }
        self.lockfile.revalidate(MAX_LOCK_BYTES as u64)?;
        self.mutation_journal.ensure_quiet()
    }

    fn from_metadata(
        metadata: &Value,
        args: &[std::ffi::OsString],
        cargo_digest: &[u8; 32],
    ) -> Result<Self, String> {
        let packages = metadata
            .get("packages")
            .and_then(Value::as_array)
            .ok_or_else(|| "authoritative Cargo metadata has no package array".to_owned())?;
        let package_by_id = packages
            .iter()
            .map(|package| Ok((required_string(package, "id")?.to_owned(), package)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let resolve = metadata
            .get("resolve")
            .and_then(Value::as_object)
            .ok_or_else(|| "authoritative Cargo metadata has no resolved graph".to_owned())?;
        let nodes = resolve
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| "authoritative Cargo metadata has no resolved nodes".to_owned())?;
        let dependencies = nodes
            .iter()
            .map(|node| {
                let id = required_string(node, "id")?.to_owned();
                let values = node
                    .get("dependencies")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("resolved package {id:?} has no dependency array"))?;
                let values = values
                    .iter()
                    .map(|value| {
                        value.as_str().map(str::to_owned).ok_or_else(|| {
                            format!("resolved package {id:?} has a non-string dependency")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((id, values))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let roots = selected_roots(metadata, resolve, args, &package_by_id)?;
        let mut pending = roots;
        let mut closure = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !closure.insert(id.clone()) {
                continue;
            }
            let next = dependencies
                .get(&id)
                .ok_or_else(|| format!("selected package {id:?} has no resolved node"))?;
            pending.extend(next.iter().cloned());
        }

        let target_directory = metadata
            .get("target_directory")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let mutation_journal = MutationJournal::new(target_directory.iter().cloned().collect())?;
        let mut snapshot = b"fe2o3-authorized-kernel-closure-content-v2\0".to_vec();
        append_field(&mut snapshot, cargo_digest);
        let mut source_trees = Vec::with_capacity(closure.len());
        for id in &closure {
            let package = package_by_id
                .get(id)
                .ok_or_else(|| format!("resolved package {id:?} has no metadata record"))?;
            let manifest = PathBuf::from(required_string(package, "manifest_path")?);
            let root = manifest.parent().ok_or_else(|| {
                format!(
                    "package {id:?} manifest has no parent: {}",
                    manifest.display()
                )
            })?;
            let observed_tree = ObservedSourceTree::capture(
                root.to_path_buf(),
                target_directory
                    .as_ref()
                    .filter(|target| target.starts_with(root))
                    .cloned(),
                Some(&mutation_journal),
            )?;
            let tree_digest = observed_tree.digest;
            validate_host_code_package(package, &tree_digest)?;
            append_field(&mut snapshot, id.as_bytes());
            for field in ["name", "version", "source", "checksum", "manifest_path"] {
                append_field(
                    &mut snapshot,
                    package
                        .get(field)
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .as_bytes(),
                );
            }
            let mut next = dependencies
                .get(id)
                .cloned()
                .ok_or_else(|| format!("resolved package {id:?} has no dependency record"))?;
            next.sort();
            for dependency in next {
                append_field(&mut snapshot, dependency.as_bytes());
            }
            append_field(&mut snapshot, &tree_digest);
            source_trees.push(observed_tree);
        }

        let workspace_root = metadata
            .get("workspace_root")
            .and_then(Value::as_str)
            .ok_or_else(|| "authoritative Cargo metadata has no workspace root".to_owned())?;
        let lock_path = Path::new(workspace_root).join("Cargo.lock");
        mutation_journal.watch_directory(Path::new(workspace_root))?;
        let lockfile = ObservedFile::capture(lock_path, MAX_LOCK_BYTES as u64)?;
        if lockfile.size == 0 {
            return Err(format!(
                "authoritative lockfile must contain 1 through {MAX_LOCK_BYTES} bytes"
            ));
        }
        append_field(&mut snapshot, &lockfile.digest);
        mutation_journal.ensure_quiet()?;
        Ok(Self {
            snapshot,
            source_trees,
            lockfile,
            mutation_journal,
        })
    }
}

fn selected_roots(
    metadata: &Value,
    resolve: &serde_json::Map<String, Value>,
    args: &[std::ffi::OsString],
    packages: &BTreeMap<String, &Value>,
) -> Result<Vec<String>, String> {
    let selected = selected_package_names(args)?;
    if !selected.is_empty() {
        let workspace_members = string_array(metadata, "workspace_members")?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut roots = Vec::new();
        for selected in selected {
            let matches = packages
                .iter()
                .filter(|(id, package)| {
                    workspace_members.contains(*id)
                        && package.get("name").and_then(Value::as_str) == Some(&selected)
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(format!(
                    "authoritative package selection {selected:?} matched {} workspace packages",
                    matches.len()
                ));
            }
            roots.push(matches[0].clone());
        }
        return Ok(roots);
    }
    if args.iter().any(|argument| argument == "--workspace") {
        return string_array(metadata, "workspace_members");
    }
    if let Some(root) = resolve.get("root").and_then(Value::as_str) {
        return Ok(vec![root.to_owned()]);
    }
    string_array(metadata, "workspace_default_members")
}

fn selected_package_names(args: &[std::ffi::OsString]) -> Result<Vec<String>, String> {
    let mut selected = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let value = args[index].to_string_lossy();
        if value == "-p" || value == "--package" {
            index += 1;
            let package = args
                .get(index)
                .and_then(|value| value.to_str())
                .ok_or_else(|| "authoritative --package requires one UTF-8 name".to_owned())?;
            selected.push(package.to_owned());
        } else if let Some(package) = value.strip_prefix("--package=") {
            if package.is_empty() {
                return Err("authoritative --package requires a non-empty name".to_owned());
            }
            selected.push(package.to_owned());
        }
        index += 1;
    }
    selected.sort();
    selected.dedup();
    Ok(selected)
}

fn validate_host_code_package(package: &Value, tree_digest: &[u8; 32]) -> Result<(), String> {
    let name = required_string(package, "name")?;
    let reviewed_hip_sys = is_reviewed_fe2o3_hip_sys(package, tree_digest)?;
    if package.get("links").is_some_and(|value| !value.is_null()) && !reviewed_hip_sys {
        return Err(format!(
            "authoritative kernel closure rejects native links package {name:?}"
        ));
    }
    let targets = package
        .get("targets")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("package {name:?} has no target array"))?;
    let kinds = targets
        .iter()
        .flat_map(|target| {
            target
                .get("kind")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    if kinds.contains("custom-build")
        && !is_reviewed_registry_build_script(package, tree_digest)?
        && !reviewed_hip_sys
    {
        return Err(format!(
            "authoritative kernel closure rejects unreviewed custom-build package {name:?}"
        ));
    }
    if kinds.contains("proc-macro") {
        validate_reviewed_fe2o3_macros(package, tree_digest)?;
    }
    Ok(())
}

fn is_reviewed_fe2o3_hip_sys(package: &Value, tree_digest: &[u8; 32]) -> Result<bool, String> {
    if package.get("name").and_then(Value::as_str) != Some("fe2o3-hip-sys") {
        return Ok(false);
    }
    if package.get("version").and_then(Value::as_str) != Some("0.1.0")
        || package.get("source").is_some_and(|value| !value.is_null())
        || package.get("links").and_then(Value::as_str) != Some("amdhip64")
    {
        return Err(
            "authoritative kernel closure rejects an unreviewed fe2o3-hip-sys package".to_owned(),
        );
    }
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-fe2o3 has a workspace crates directory")
        .join("fe2o3-hip-sys");
    let manifest = PathBuf::from(required_string(package, "manifest_path")?);
    if manifest != expected.join("Cargo.toml") {
        return Err(format!(
            "authoritative kernel closure rejects fe2o3-hip-sys from {}",
            manifest.display()
        ));
    }
    validate_expected_tree(
        tree_digest,
        TRUSTED_FE2O3_HIP_SYS_TREE,
        &expected,
        "native build",
    )?;
    if std::env::var_os("FE2O3_HIP_SYS_DISABLE").is_none() {
        return Err(
            "authoritative native build requires FE2O3_HIP_SYS_DISABLE so unpinned C tools and ROCm headers cannot enter the trusted closure"
                .to_owned(),
        );
    }
    Ok(true)
}

fn is_reviewed_registry_build_script(
    package: &Value,
    tree_digest: &[u8; 32],
) -> Result<bool, String> {
    let Some(name) = package.get("name").and_then(Value::as_str) else {
        return Ok(false);
    };
    let Some(version) = package.get("version").and_then(Value::as_str) else {
        return Ok(false);
    };
    if package.get("source").and_then(Value::as_str) != Some(CRATES_IO_SOURCE) {
        return Ok(false);
    }
    let Some((_, _, expected)) =
        TRUSTED_REGISTRY_BUILD_SCRIPTS
            .iter()
            .find(|(trusted_name, trusted_version, _)| {
                name == *trusted_name && version == *trusted_version
            })
    else {
        return Ok(false);
    };
    validate_expected_tree(
        tree_digest,
        expected,
        Path::new(required_string(package, "manifest_path")?)
            .parent()
            .expect("manifest path was already required"),
        "registry build-script",
    )?;
    Ok(true)
}

fn validate_reviewed_fe2o3_macros(package: &Value, tree_digest: &[u8; 32]) -> Result<(), String> {
    if package.get("name").and_then(Value::as_str) != Some("fe2o3-macros")
        || package.get("source").is_some_and(|value| !value.is_null())
    {
        return Err(
            "authoritative kernel closure rejects an unreviewed procedural macro".to_owned(),
        );
    }
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-fe2o3 has a workspace crates directory")
        .join("fe2o3-macros");
    let manifest = PathBuf::from(required_string(package, "manifest_path")?);
    if manifest != expected.join("Cargo.toml") {
        return Err(format!(
            "authoritative kernel closure rejects fe2o3-macros from {}",
            manifest.display()
        ));
    }
    validate_expected_tree(
        tree_digest,
        TRUSTED_FE2O3_MACROS_TREE,
        &expected,
        "proc-macro",
    )
}

fn validate_expected_tree(
    observed: &[u8; 32],
    expected: &str,
    root: &Path,
    kind: &str,
) -> Result<(), String> {
    if hex(observed) != expected {
        return Err(format!(
            "reviewed {kind} closure content changed under {}",
            root.display()
        ));
    }
    Ok(())
}

fn canonical_tree_digest(root: &Path, excluded: Option<&Path>) -> Result<[u8; 32], String> {
    canonical_tree_digest_monitored(root, excluded, None)
}

fn canonical_tree_digest_monitored(
    root: &Path,
    excluded: Option<&Path>,
    mutation_journal: Option<&MutationJournal>,
) -> Result<[u8; 32], String> {
    let mut files = Vec::new();
    collect_tree_files(root, root, excluded, mutation_journal, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.len() > MAX_SOURCE_TREE_FILES {
        return Err(format!(
            "authoritative source tree {} contains more than {MAX_SOURCE_TREE_FILES} files",
            root.display()
        ));
    }

    let mut total = 0_u64;
    let mut tree = Sha256::new();
    tree.update(b"fe2o3-canonical-source-tree-v1\0");
    for (relative, path) in files {
        let (digest, size) = hash_regular_file(&path, MAX_SOURCE_FILE_BYTES)?;
        total = total
            .checked_add(size)
            .ok_or_else(|| format!("authoritative source tree {} is too large", root.display()))?;
        if total > MAX_SOURCE_TREE_BYTES {
            return Err(format!(
                "authoritative source tree {} exceeds {MAX_SOURCE_TREE_BYTES} bytes",
                root.display()
            ));
        }
        tree.update(hex(&digest).as_bytes());
        tree.update(b"  ");
        tree.update(relative.as_bytes());
        tree.update(b"\n");
    }
    Ok(tree.finalize().into())
}

fn collect_tree_files(
    root: &Path,
    directory: &Path,
    excluded: Option<&Path>,
    mutation_journal: Option<&MutationJournal>,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    if excluded.is_some_and(|excluded| directory == excluded || directory.starts_with(excluded)) {
        return Ok(());
    }
    let initial = fs::symlink_metadata(directory).map_err(|error| {
        format!(
            "cannot inspect authoritative source directory {}: {error}",
            directory.display()
        )
    })?;
    if !initial.is_dir() || initial.file_type().is_symlink() {
        return Err(format!(
            "authoritative source directory must be a real directory: {}",
            directory.display()
        ));
    }
    if let Some(mutation_journal) = mutation_journal {
        mutation_journal.watch_directory(directory)?;
    }
    let entries = fs::read_dir(directory).map_err(|error| {
        format!(
            "cannot enumerate authoritative source directory {}: {error}",
            directory.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot enumerate authoritative source directory {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        if excluded.is_some_and(|excluded| path == excluded || path.starts_with(excluded)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "cannot inspect authoritative source {}: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "authoritative source closure rejects symbolic link {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_tree_files(root, &path, excluded, mutation_journal, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("recursive source path remains under its root")
                .to_str()
                .ok_or_else(|| {
                    format!("authoritative source path is not UTF-8: {}", path.display())
                })?;
            if relative.contains(['\n', '\r']) {
                return Err(format!(
                    "authoritative source path contains a line break: {}",
                    path.display()
                ));
            }
            files.push((relative.replace(std::path::MAIN_SEPARATOR, "/"), path));
        } else {
            return Err(format!(
                "authoritative source closure rejects special file {}",
                path.display()
            ));
        }
    }
    let final_metadata = fs::symlink_metadata(directory).map_err(|error| {
        format!(
            "cannot re-inspect authoritative source directory {}: {error}",
            directory.display()
        )
    })?;
    if !same_observed_object(&initial, &final_metadata) {
        return Err(format!(
            "authoritative source directory changed while observed: {}",
            directory.display()
        ));
    }
    Ok(())
}

fn hash_regular_file(path: &Path, limit: u64) -> Result<([u8; 32], u64), String> {
    let initial = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "cannot inspect authoritative input {}: {error}",
            path.display()
        )
    })?;
    if !initial.is_file() || initial.file_type().is_symlink() {
        return Err(format!(
            "authoritative input must be a regular file: {}",
            path.display()
        ));
    }
    if initial.len() > limit {
        return Err(format!(
            "authoritative input {} must contain at most {limit} bytes",
            path.display()
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let mut file = options.open(path).map_err(|error| {
        format!(
            "cannot open authoritative input {}: {error}",
            path.display()
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        format!(
            "cannot inspect authoritative input {}: {error}",
            path.display()
        )
    })?;
    if !same_observed_object(&initial, &opened) {
        return Err(format!(
            "authoritative input changed before it was opened: {}",
            path.display()
        ));
    }
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            format!(
                "cannot read authoritative input {}: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| format!("authoritative input is too large: {}", path.display()))?;
        if size > limit {
            return Err(format!(
                "authoritative input {} exceeds {limit} bytes",
                path.display()
            ));
        }
        digest.update(&buffer[..read]);
    }
    let final_metadata = file.metadata().map_err(|error| {
        format!(
            "cannot re-inspect authoritative input {}: {error}",
            path.display()
        )
    })?;
    if size != initial.len() || !same_observed_object(&initial, &final_metadata) {
        return Err(format!(
            "authoritative input changed while it was read: {}",
            path.display()
        ));
    }
    Ok((digest.finalize().into(), size))
}

#[cfg(unix)]
fn same_observed_object(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_observed_object(left: &Metadata, right: &Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Cargo metadata record has no string {field:?}"))
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Cargo metadata has no {field:?} array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Cargo metadata {field:?} contains a non-string"))
        })
        .collect()
}

fn append_field(snapshot: &mut Vec<u8>, field: &[u8]) {
    snapshot.extend_from_slice(&(field.len() as u64).to_le_bytes());
    snapshot.extend_from_slice(field);
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pinned_executable_test_directory::TestDirectory;

    fn metadata(target_kind: &str, package_name: &str, links: Value) -> Value {
        serde_json::json!({
            "packages": [{
                "id": "path+file:///fixture#0.1.0",
                "name": package_name,
                "version": "0.1.0",
                "source": null,
                "checksum": null,
                "manifest_path": "/fixture/Cargo.toml",
                "links": links,
                "targets": [{"kind": [target_kind]}]
            }],
            "resolve": {
                "root": "path+file:///fixture#0.1.0",
                "nodes": [{"id": "path+file:///fixture#0.1.0", "dependencies": []}]
            },
            "workspace_members": ["path+file:///fixture#0.1.0"],
            "workspace_default_members": ["path+file:///fixture#0.1.0"],
            "workspace_root": "/fixture"
        })
    }

    #[test]
    fn host_code_policy_rejects_custom_build_proc_macro_and_native_links() {
        for (kind, name, links, expected) in [
            ("custom-build", "hostile-build", Value::Null, "custom-build"),
            (
                "proc-macro",
                "hostile-macro",
                Value::Null,
                "procedural macro",
            ),
            (
                "lib",
                "hostile-links",
                Value::String("native".into()),
                "native links",
            ),
        ] {
            let record = metadata(kind, name, links);
            let package = &record["packages"][0];
            assert!(
                validate_host_code_package(package, &[0_u8; 32])
                    .unwrap_err()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn tampered_registry_build_script_changes_the_reviewed_content_identity() {
        let directory = TestDirectory::new();
        fs::write(
            directory.path().join("Cargo.toml"),
            b"[package]\nname='reviewed-registry-crate'\nversion='1.0.0'\n",
        )
        .unwrap();
        let build_script = directory.path().join("build.rs");
        fs::write(&build_script, b"fn main() {}\n").unwrap();
        let reviewed = canonical_tree_digest(directory.path(), None).unwrap();
        let reviewed_hex = hex(&reviewed);
        validate_expected_tree(
            &reviewed,
            &reviewed_hex,
            directory.path(),
            "registry build-script",
        )
        .unwrap();

        fs::write(&build_script, b"fn main() { panic!(\"tampered\") }\n").unwrap();
        let tampered = canonical_tree_digest(directory.path(), None).unwrap();
        let error = validate_expected_tree(
            &tampered,
            &reviewed_hex,
            directory.path(),
            "registry build-script",
        )
        .unwrap_err();
        assert!(error.contains("registry build-script closure content changed"));
    }

    #[test]
    fn canonical_closure_identity_covers_manifests_and_nested_sources() {
        let directory = TestDirectory::new();
        let source = directory.path().join("src");
        fs::create_dir(&source).unwrap();
        let manifest = directory.path().join("Cargo.toml");
        let library = source.join("lib.rs");
        fs::write(&manifest, b"[package]\nname='fixture'\nversion='0.1.0'\n").unwrap();
        fs::write(&library, b"pub fn reviewed() {}\n").unwrap();
        let reviewed = canonical_tree_digest(directory.path(), None).unwrap();

        fs::write(&manifest, b"[package]\nname='fixture'\nversion='0.1.1'\n").unwrap();
        let manifest_drift = canonical_tree_digest(directory.path(), None).unwrap();
        assert_ne!(manifest_drift, reviewed);

        fs::write(&manifest, b"[package]\nname='fixture'\nversion='0.1.0'\n").unwrap();
        fs::write(&library, b"pub fn substituted() {}\n").unwrap();
        let source_drift = canonical_tree_digest(directory.path(), None).unwrap();
        assert_ne!(source_drift, reviewed);
    }

    #[test]
    fn reviewed_workspace_host_code_trees_match_their_complete_pins() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cargo-fe2o3 has a workspace crates directory");
        for (name, expected) in [
            ("fe2o3-macros", TRUSTED_FE2O3_MACROS_TREE),
            ("fe2o3-hip-sys", TRUSTED_FE2O3_HIP_SYS_TREE),
        ] {
            let root = crates.join(name);
            assert_eq!(hex(&canonical_tree_digest(&root, None).unwrap()), expected);
        }
    }

    #[test]
    fn proc_macro_mutation_after_preflight_fails_revalidation() {
        let directory = TestDirectory::new();
        let source = directory.path().join("src");
        fs::create_dir(&source).unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            b"[lib]\nproc-macro=true\n",
        )
        .unwrap();
        let library = source.join("lib.rs");
        fs::write(&library, b"pub fn reviewed() {}\n").unwrap();
        let observed =
            ObservedSourceTree::capture(directory.path().to_path_buf(), None, None).unwrap();

        fs::write(&library, b"pub fn injected_after_preflight() {}\n").unwrap();
        let error = observed.revalidate().unwrap_err();
        assert!(error.contains("source closure changed after preflight"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn restored_proc_macro_mutation_remains_in_the_journal() {
        let directory = TestDirectory::new();
        let source = directory.path().join("src");
        fs::create_dir(&source).unwrap();
        let library = source.join("lib.rs");
        let reviewed = b"pub fn reviewed() {}\n";
        fs::write(&library, reviewed).unwrap();
        let journal = MutationJournal::new(Vec::new()).unwrap();
        let observed =
            ObservedSourceTree::capture(directory.path().to_path_buf(), None, Some(&journal))
                .unwrap();
        journal.ensure_quiet().unwrap();

        fs::write(&library, b"pub fn injected() {}\n").unwrap();
        fs::write(&library, reviewed).unwrap();
        observed.revalidate().unwrap();
        let error = journal.ensure_quiet().unwrap_err();
        assert!(error.contains("source closure mutated after preflight"));
    }
}
