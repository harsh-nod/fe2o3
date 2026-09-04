use super::*;

#[derive(Serialize)]
struct Manifest<'a> {
    schema: &'static str,
    namespace: &'static str,
    authority: &'static str,
    artifact: &'static str,
    crate_name: &'a str,
    target: &'static str,
    code_object_version: u8,
    compiler_handoff: Identity,
    tools: Tools<'a>,
    providers: Vec<Provider>,
    options: FixedOptions,
    execution: Execution,
    hsaco: Hsaco<'a>,
    grants: Grants,
}

#[derive(Serialize)]
struct Identity {
    sha256: String,
    byte_len: u64,
}

#[derive(Serialize)]
struct Tools<'a> {
    cargo: Identity,
    rustc: Identity,
    rustc_lib_tree_sha256: String,
    host_linker: Identity,
    host_lld: Identity,
    host_lld_proxy: Identity,
    cargo_vendor: Option<CargoVendor<'a>>,
    extractor: Identity,
    extractor_backend: Identity,
    worker: Worker<'a>,
}

#[derive(Serialize)]
struct CargoVendor<'a> {
    tree_sha256: String,
    git_sources: &'a [CargoGitSource],
}

#[derive(Serialize)]
struct Worker<'a> {
    executable: Identity,
    worker_build_identity: &'a str,
    llvm_build_identity: &'a str,
}

#[derive(Serialize)]
struct Provider {
    kind: &'static str,
    identity: Identity,
}

#[derive(Serialize)]
struct FixedOptions {
    optimization: &'static str,
    strip_debug: bool,
    verify_each: bool,
    timeout_seconds: u64,
    maximum_output_bytes: u64,
}

#[derive(Serialize)]
struct Execution {
    bootstrap_request: Identity,
    bootstrap_response: Identity,
    replay_request: Identity,
    replay_response: Identity,
    exact_output_replay: bool,
}

#[derive(Serialize)]
struct Hsaco<'a> {
    identity: Identity,
    canonical_descriptor_sha256: String,
    kernel_names: &'a [String],
}

#[derive(Serialize)]
struct Grants {
    publication: bool,
    load: bool,
    launch: bool,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the canonical manifest binds every independently measured tool and payload explicitly"
)]
pub(super) fn canonical_manifest(
    options: &Options,
    observation: &EngineeringHsacoObservationV1,
    cargo: ContentIdentityV1,
    rustc: ContentIdentityV1,
    host_linker: ContentIdentityV1,
    host_lld: ContentIdentityV1,
    host_lld_proxy: ContentIdentityV1,
    rustc_lib_tree_sha256: [u8; 32],
    cargo_vendor_sha256: Option<[u8; 32]>,
    extractor: &[u8],
    extractor_backend: &[u8],
) -> Result<Vec<u8>, String> {
    let providers = observation
        .providers()
        .iter()
        .map(|provider| Provider {
            kind: provider_kind(provider.kind()),
            identity: identity(provider.identity()),
        })
        .collect();
    let worker = observation.worker_measurement();
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        namespace: NAMESPACE,
        authority: observation.authority(),
        artifact: "observation.hsaco",
        crate_name: &options.crate_name,
        target: TARGET,
        code_object_version: CODE_OBJECT_VERSION,
        compiler_handoff: identity(observation.handoff_identity()),
        tools: Tools {
            cargo: identity(cargo),
            rustc: identity(rustc),
            rustc_lib_tree_sha256: hex(&rustc_lib_tree_sha256),
            host_linker: identity(host_linker),
            host_lld: identity(host_lld),
            host_lld_proxy: identity(host_lld_proxy),
            cargo_vendor: cargo_vendor_sha256.map(|tree| CargoVendor {
                tree_sha256: hex(&tree),
                git_sources: &options.cargo_git_sources,
            }),
            extractor: identity(ContentIdentityV1::calculate(extractor)),
            extractor_backend: identity(ContentIdentityV1::calculate(extractor_backend)),
            worker: Worker {
                executable: identity(worker.executable()),
                worker_build_identity: worker.worker_build_identity(),
                llvm_build_identity: worker.llvm_build_identity(),
            },
        },
        providers,
        options: FixedOptions {
            optimization: "O2",
            strip_debug: true,
            verify_each: true,
            timeout_seconds: options.timeout.as_secs(),
            maximum_output_bytes: options.max_output_bytes,
        },
        execution: Execution {
            bootstrap_request: identity(observation.bootstrap_request_identity()),
            bootstrap_response: identity(observation.bootstrap_response_identity()),
            replay_request: identity(observation.replay_request_identity()),
            replay_response: identity(observation.replay_response_identity()),
            exact_output_replay: true,
        },
        hsaco: Hsaco {
            identity: identity(observation.finalized_hsaco_identity()),
            canonical_descriptor_sha256: hex(observation.canonical_descriptor_digest()),
            kernel_names: observation.kernel_names(),
        },
        grants: Grants {
            publication: observation.grants_publication_authority(),
            load: observation.grants_load_authority(),
            launch: observation.grants_launch_authority(),
        },
    };
    let mut bytes = serde_json::to_vec(&manifest)
        .map_err(|error| format!("cannot encode engineering observation: {error}"))?;
    if bytes.len() >= MAX_MANIFEST_BYTES {
        return Err(format!(
            "engineering observation manifest exceeds {MAX_MANIFEST_BYTES} bytes"
        ));
    }
    bytes.push(b'\n');
    Ok(bytes)
}

fn identity(identity: ContentIdentityV1) -> Identity {
    Identity {
        sha256: hex(identity.sha256()),
        byte_len: identity.byte_len(),
    }
}

const fn provider_kind(kind: WorkerInputKindV1) -> &'static str {
    match kind {
        WorkerInputKindV1::LlvmBitcode => "llvm-bitcode",
        WorkerInputKindV1::AmdGpuRelocatable => "amdgpu-relocatable",
        WorkerInputKindV1::LlvmTextIr => "llvm-ir",
    }
}

pub(super) fn publish_observation(
    root: &Path,
    manifest: &[u8],
    hsaco: &[u8],
) -> Result<PathBuf, String> {
    publish_observation_inner(root, manifest, hsaco, false)
}

pub(super) fn publish_observation_inner(
    root: &Path,
    manifest: &[u8],
    hsaco: &[u8],
    fail_after_first_write: bool,
) -> Result<PathBuf, String> {
    validate_fresh_output_root(root)?;
    let content_id = observation_content_id(manifest, hsaco);
    let content_dir = root.join(&content_id);
    let parent_path = root
        .parent()
        .ok_or_else(|| "engineering namespace has no parent directory".to_owned())?;
    let root_name = root
        .file_name()
        .ok_or_else(|| "engineering namespace has no basename".to_owned())?;
    let parent = crate::project::PinnedDirectory::open_existing(
        parent_path.to_path_buf(),
        "engineering output parent",
    )?;
    rustix::fs::mkdirat(
        parent.file(),
        root_name,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
    )
    .map_err(|error| format!("cannot create engineering namespace: {error}"))?;
    let namespace = parent
        .open_child(NAMESPACE, "engineering namespace")?
        .ok_or_else(|| "created engineering namespace disappeared".to_owned())?;
    namespace.validate_path("engineering namespace")?;
    rustix::fs::mkdirat(
        namespace.file(),
        content_id.as_str(),
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
    )
    .map_err(|error| format!("cannot create engineering content directory: {error}"))?;
    let content = namespace
        .open_child(&content_id, "engineering content directory")?
        .ok_or_else(|| "created engineering content directory disappeared".to_owned())?;
    content.validate_path("engineering content directory")?;
    let retained_hsaco = write_new_file_at(content.file(), "observation.hsaco", hsaco, 0o600)?;
    if fail_after_first_write {
        return Err(
            "injected engineering publication failure; partial output was retained".to_owned(),
        );
    }
    let retained_manifest = write_new_file_at(content.file(), "observation.json", manifest, 0o600)?;
    sync_directory(content.file())
        .map_err(|error| format!("cannot sync engineering content directory: {error}"))?;
    sync_directory(namespace.file())
        .map_err(|error| format!("cannot sync engineering namespace: {error}"))?;
    namespace.validate_path("engineering namespace")?;
    content.validate_path("engineering content directory")?;
    retained_hsaco.validate_name(content.file(), "observation.hsaco")?;
    retained_manifest.validate_name(content.file(), "observation.json")?;
    Ok(content_dir)
}

fn sync_directory(directory: &File) -> std::io::Result<()> {
    let descriptor = rustix::fs::openat(
        directory,
        ".",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    File::from(descriptor).sync_all()
}

fn unlink_matching_directory(
    parent: &crate::project::PinnedDirectory,
    name: &str,
    child: &crate::project::PinnedDirectory,
) -> Result<(), String> {
    let stat = rustix::fs::statat(parent.file(), name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("cannot inspect cleanup entry {name}: {error}"))?;
    if !child.matches_identity(stat.st_dev, stat.st_ino) {
        return Err(format!("cleanup entry {name} was substituted"));
    }
    rustix::fs::unlinkat(parent.file(), name, rustix::fs::AtFlags::REMOVEDIR)
        .map_err(|error| format!("cannot remove cleanup directory {name}: {error}"))
}

pub(super) fn observation_content_id(manifest: &[u8], hsaco: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CONTENT_ID_DOMAIN);
    hasher.update((manifest.len() as u64).to_le_bytes());
    hasher.update(manifest);
    hasher.update((hsaco.len() as u64).to_le_bytes());
    hasher.update(hsaco);
    hex(&hasher.finalize())
}

pub(super) fn write_new_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot create fresh `{}`: {error}", path.display()))?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    if let Err(error) = result {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("cannot publish `{}`: {error}", path.display()));
    }
    Ok(())
}

struct RetainedOutputFile {
    file: File,
    device: u64,
    inode: u64,
}

impl RetainedOutputFile {
    fn from_file(file: File) -> Result<Self, String> {
        let metadata = file
            .metadata()
            .map_err(|error| format!("cannot inspect retained output file: {error}"))?;
        Ok(Self {
            file,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn validate_name(&self, parent: &File, name: &str) -> Result<(), String> {
        let stat = rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| format!("cannot inspect retained output `{name}`: {error}"))?;
        if stat.st_dev != self.device || stat.st_ino != self.inode {
            return Err(format!("retained output `{name}` was substituted"));
        }
        Ok(())
    }
}

fn write_new_file_at(
    parent: &File,
    name: &str,
    bytes: &[u8],
    mode: u32,
) -> Result<RetainedOutputFile, String> {
    let descriptor = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_bits_retain(mode),
    )
    .map_err(|error| format!("cannot create fresh `{name}`: {error}"))?;
    let mut retained = RetainedOutputFile::from_file(File::from(descriptor))?;
    if let Err(error) = retained
        .file
        .write_all(bytes)
        .and_then(|()| retained.file.sync_all())
    {
        return Err(format!(
            "cannot publish `{name}`: {error}; partial output was retained"
        ));
    }
    Ok(retained)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(super) fn absolute_path(current_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

pub(super) struct ScratchDirectory {
    pub(super) path: PathBuf,
    parent: crate::project::PinnedDirectory,
    directory: crate::project::PinnedDirectory,
    name: String,
}

impl ScratchDirectory {
    pub(super) fn new() -> Result<Self, String> {
        let base = fs::canonicalize(env::temp_dir())
            .map_err(|error| format!("cannot canonicalize temporary directory: {error}"))?;
        let parent = crate::project::PinnedDirectory::open_existing(
            base.clone(),
            "engineering scratch parent",
        )?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is before the Unix epoch".to_owned())?
            .as_nanos();
        for attempt in 0_u32..32 {
            let name = format!(
                "fe2o3-engineering-hsaco-{}-{nonce}-{attempt}",
                std::process::id()
            );
            let path = base.join(&name);
            match rustix::fs::mkdirat(
                parent.file(),
                name.as_str(),
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
            ) {
                Ok(()) => {
                    let directory = parent
                        .open_child(&name, "engineering scratch")?
                        .ok_or_else(|| "created engineering scratch disappeared".to_owned())?;
                    return Ok(Self {
                        path,
                        parent,
                        directory,
                        name,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(error) => {
                    return Err(format!(
                        "cannot create private engineering scratch: {error}"
                    ));
                }
            }
        }
        Err("cannot allocate a fresh engineering scratch directory".to_owned())
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = remove_retained_directory_contents(self.directory.file());
        let _ = unlink_matching_directory(&self.parent, &self.name, &self.directory);
    }
}

fn remove_retained_directory_contents(directory: &File) -> Result<(), String> {
    let scan = rustix::fs::openat(
        directory,
        ".",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| format!("cannot open retained cleanup directory: {error}"))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("cannot enumerate retained cleanup directory: {error}"))?;
    let mut names = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(|error| format!("cannot enumerate cleanup entry: {error}"))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        names.push(std::ffi::OsString::from_vec(bytes.to_vec()));
    }
    for name in names {
        let stat = rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| format!("cannot inspect cleanup entry {name:?}: {error}"))?;
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::Directory {
            let child = rustix::fs::openat(
                directory,
                &name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(|error| format!("cannot retain cleanup directory {name:?}: {error}"))?;
            let opened = rustix::fs::fstat(&child)
                .map_err(|error| format!("cannot inspect retained cleanup entry: {error}"))?;
            if (stat.st_dev, stat.st_ino, stat.st_mode)
                != (opened.st_dev, opened.st_ino, opened.st_mode)
            {
                return Err(format!(
                    "cleanup directory {name:?} changed before retention"
                ));
            }
            remove_retained_directory_contents(&child)?;
            let current =
                rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|error| {
                        format!("cannot re-inspect cleanup directory {name:?}: {error}")
                    })?;
            if (current.st_dev, current.st_ino, current.st_mode)
                != (opened.st_dev, opened.st_ino, opened.st_mode)
            {
                return Err(format!("cleanup directory {name:?} was substituted"));
            }
            rustix::fs::unlinkat(directory, &name, rustix::fs::AtFlags::REMOVEDIR)
                .map_err(|error| format!("cannot remove cleanup directory {name:?}: {error}"))?;
        } else {
            rustix::fs::unlinkat(directory, &name, rustix::fs::AtFlags::empty())
                .map_err(|error| format!("cannot remove cleanup entry {name:?}: {error}"))?;
        }
    }
    Ok(())
}
