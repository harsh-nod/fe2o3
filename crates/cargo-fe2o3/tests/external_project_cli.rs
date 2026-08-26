use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::net::{UnixDatagram, UnixStream};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Stdio;
use std::process::{self, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use rustix::fs::{FlockOperation, flock};
#[cfg(unix)]
use rustix::io::Errno;
use sha2::{Digest, Sha256};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
#[cfg(unix)]
const ARTIFACT_CHILD_FD: i32 = 197;
const PROTECTED_RELEASE_CARGO_REPORT: &str = ".fe2o3-protected-release-cargo-report-v1.json";
const PROTECTED_RELEASE_CARGO_READY: &str = ".fe2o3-protected-release-cargo-ready-v1";
const PROTECTED_RELEASE_CARGO_HOLD: &str = ".fe2o3-protected-release-cargo-hold-v1";
const PROTECTED_RELEASE_CARGO_SURVIVED: &str = ".fe2o3-protected-release-cargo-survived-v1";
const PROTECTED_RELEASE_RUSTC_FD_REPORT: &str = ".fe2o3-protected-release-rustc-fd-report-v1.json";
const PROTECTED_RELEASE_RUSTC_FD_ATTACK: &str = ".fe2o3-protected-release-rustc-fd-attack-v1";

#[cfg(unix)]
struct ReapedChild(Option<std::process::Child>);

#[cfg(unix)]
impl ReapedChild {
    fn new(child: std::process::Child) -> Self {
        Self(Some(child))
    }

    fn child_mut(&mut self) -> &mut std::process::Child {
        self.0.as_mut().expect("child has not been reaped")
    }

    fn wait_with_output(mut self) -> std::io::Result<Output> {
        self.0
            .take()
            .expect("child has not been reaped")
            .wait_with_output()
    }
}

#[cfg(unix)]
impl Drop for ReapedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(unix)]
fn output_retrying_text_file_busy(command: &mut Command, context: &str) -> Output {
    let mut attempts = 0;
    loop {
        match command.output() {
            Ok(output) => return output,
            Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) && attempts < 7 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => panic!("{context}: {error}"),
        }
    }
}

#[cfg(target_os = "linux")]
fn close_nonstdio_descriptors_before_exec(command: &mut Command) {
    // SAFETY: close_range is one direct syscall in the post-fork child. Standard I/O has already
    // been installed at 0/1/2, and this fixture intentionally admits no other descriptor.
    unsafe {
        command.pre_exec(|| close_descriptor_range(3, u32::MAX));
    }
}

#[cfg(target_os = "linux")]
fn close_descriptor_range(first: u32, last: u32) -> std::io::Result<()> {
    // SAFETY: direct close_range has no userspace state and this helper runs only in pre_exec.
    if unsafe { libc::syscall(libc::SYS_close_range, first, last, 0_u32) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

struct ProjectFixture {
    root: PathBuf,
    workspace: PathBuf,
    cwd: PathBuf,
    target: PathBuf,
    backend: PathBuf,
    log: PathBuf,
}

#[cfg(unix)]
struct SameFilesystemFixture(PathBuf);

#[cfg(unix)]
impl SameFilesystemFixture {
    fn beside(source: &Path) -> Self {
        let parent = source.parent().expect("fixture executable has a parent");
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".cargo-fe2o3-hardlink-fixture-{}-{id}",
                process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create same-filesystem fixture: {error}"),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(unix)]
impl Drop for SameFilesystemFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl ProjectFixture {
    fn standalone() -> Self {
        let root = temp_root();
        let workspace = root.join("standalone");
        fs::create_dir_all(workspace.join("src/nested")).expect("create standalone project");
        fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname='external-standalone'\nversion='0.1.0'\nedition='2024'\n",
        )
        .expect("write manifest");
        fs::write(workspace.join("src/main.rs"), "fn main() {}\n").expect("write source");
        Self::from_paths(root, workspace.clone(), workspace.join("src/nested"))
    }

    fn virtual_workspace() -> Self {
        let root = temp_root();
        let workspace = root.join("workspace");
        let member = workspace.join("member");
        fs::create_dir_all(member.join("src/nested")).expect("create workspace member");
        fs::write(
            workspace.join("Cargo.toml"),
            "[workspace]\nmembers=['member']\nresolver='2'\n",
        )
        .expect("write workspace manifest");
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname='selected-member'\nversion='0.1.0'\nedition='2024'\n",
        )
        .expect("write member manifest");
        fs::write(member.join("src/main.rs"), "fn main() {}\n").expect("write member source");
        Self::from_paths(root, workspace, member.join("src/nested"))
    }

    fn from_paths(root: PathBuf, workspace: PathBuf, cwd: PathBuf) -> Self {
        let target = workspace.join("target");
        let backend = root.join("librustc_codegen_fe2o3.so");
        let log = root.join("cargo.log");
        fs::write(&backend, b"test backend").expect("write backend fixture");
        Self {
            root,
            workspace,
            cwd,
            target,
            backend,
            log,
        }
    }

    fn run(&self, args: &[OsString]) -> Output {
        self.command(args)
            .output()
            .expect("run cargo-fe2o3 external-project fixture")
    }

    fn command(&self, args: &[OsString]) -> Command {
        self.command_with_program(Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3")), args)
    }

    fn command_with_program(&self, program: &Path, args: &[OsString]) -> Command {
        let mut command = cargo_fe2o3_command_with_program(program);
        command
            .args(args)
            .current_dir(&self.cwd)
            .env("CARGO", env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture"))
            .env("FE2O3_BACKEND", &self.backend)
            .env("FE2O3_TARGET", "gfx942")
            .env("FE2O3_TEST_CARGO_LOG", &self.log)
            .env("FE2O3_TEST_WORKSPACE_ROOT", &self.workspace)
            .env("FE2O3_TEST_TARGET_DIRECTORY", &self.target)
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        command
    }

    fn authority_command(&self, args: &[OsString]) -> Command {
        let mut command = self.command(args);
        command
            .env("PATH", rustc_fixture_path(&self.root))
            .env(
                "FE2O3_AUTHORITY_RUSTC_PATH_V1",
                rustc_fixture_executable(&self.root),
            )
            .env(
                "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
                "1",
            );
        scrub_test_harness_dynamic_loader_environment(&mut command);
        command
    }

    fn protected_release_command(&self, action: &str) -> Command {
        let args = [
            OsString::from("authority"),
            OsString::from("release"),
            OsString::from(action),
        ];
        let trampoline = cargo_binding_trampoline(&self.root);
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command
            .env_clear()
            .args(args)
            .current_dir(&self.cwd)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .env("CARGO", env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture"))
            .env("FE2O3_BACKEND", &self.backend)
            .env("FE2O3_TARGET", "gfx942")
            .env(
                "FE2O3_AUTHORITY_RUSTC_PATH_V1",
                rustc_fixture_executable(&self.root),
            )
            .env(
                "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
                authority_rustc_sha256(&self.root),
            )
            .env(
                "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
                authority_rustc_runtime_sha256(&self.root),
            )
            .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
            .env(
                "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
                &trampoline,
            )
            .env(
                "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
                file_sha256(&trampoline),
            )
            .env(
                "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
                file_sha256(&self.backend),
            );
        command
    }

    #[cfg(target_os = "linux")]
    fn isolated_protected_release_command(&self, action: &str) -> Command {
        let mut command = self.protected_release_command(action);
        close_nonstdio_descriptors_before_exec(&mut command);
        command
    }

    fn protected_release_build_command(&self) -> Command {
        let cargo = Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-release-cargo-fixture"));
        let rustc = release_rustc_fixture_executable(&self.root);
        let build_config = self.inert_production_build_config();
        let mut command = self.isolated_protected_release_command("build");
        command
            .env("CARGO", cargo)
            .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", file_sha256(cargo))
            .env("FE2O3_AUTHORITY_RUSTC_PATH_V1", &rustc)
            .env("FE2O3_AUTHORITY_RUSTC_SHA256_V1", file_sha256(&rustc))
            .env("FE2O3_PRODUCTION_BUILD_CONFIG_V1", build_config)
            .env(
                "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
                runtime_tree_sha256(
                    &rustc
                        .parent()
                        .expect("release rustc bin directory")
                        .parent()
                        .expect("release rustc toolchain directory")
                        .join("lib"),
                ),
            );
        command
    }

    fn inert_production_build_config(&self) -> PathBuf {
        let worker = env::current_exe().expect("resolve inert production worker fixture");
        let worker_bytes = fs::read(&worker).expect("read inert production worker fixture");
        let worker_sha256 = file_sha256(&worker);
        let config = self.root.join("production-build-config.json");
        let value = serde_json::json!({
            "candidate_output_max_bytes": 4_194_304,
            "format": "fe2o3-production-build-config-v1",
            "limits": {
                "stderr_bytes": 65_536,
                "stdout_bytes": 8_388_608,
                "timeout_ms": 30_000
            },
            "link_options": [
                {"name": "code-object-version", "value": "5"},
                {"name": "opt-level", "value": "2"},
                {"name": "strip-debug", "value": "true"},
                {"name": "verify-each", "value": "true"}
            ],
            "providers": [],
            "units": [{
                "crate_name": "external_standalone",
                "source": self.workspace.join("src/main.rs"),
                "working_directory": self.cwd
            }],
            "worker": {
                "byte_len": worker_bytes.len(),
                "llvm_build_identity": "test-only-unreached-llvm",
                "path": worker,
                "sha256": worker_sha256,
                "worker_build_identity": "test-only-unreached-worker"
            }
        });
        fs::write(
            &config,
            serde_json::to_vec(&value).expect("encode canonical production worker config"),
        )
        .expect("write inert production worker config");
        config
    }

    fn invocations(&self) -> Vec<Invocation> {
        let bytes = fs::read(&self.log).expect("read fake Cargo log");
        Invocation::decode_all(&bytes)
    }
}

impl Drop for ProjectFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
struct Invocation {
    cwd: Vec<u8>,
    args: Vec<Vec<u8>>,
    cargo_target_dir: Vec<u8>,
    hsaco_dir: Vec<u8>,
    target: Vec<u8>,
    wrapper: Vec<u8>,
    rustflags: Vec<u8>,
    encoded_rustflags: Vec<u8>,
    managed_rustc_args: Vec<u8>,
}

impl Invocation {
    fn decode_all(mut bytes: &[u8]) -> Vec<Self> {
        let mut records = Vec::new();
        while !bytes.is_empty() {
            let cwd = take_field(&mut bytes);
            let count = take_u64(&mut bytes) as usize;
            let args = (0..count).map(|_| take_field(&mut bytes)).collect();
            records.push(Self {
                cwd,
                args,
                cargo_target_dir: take_field(&mut bytes),
                hsaco_dir: take_field(&mut bytes),
                target: take_field(&mut bytes),
                wrapper: take_field(&mut bytes),
                rustflags: take_field(&mut bytes),
                encoded_rustflags: take_field(&mut bytes),
                managed_rustc_args: take_field(&mut bytes),
            });
        }
        records
    }
}

fn temp_root() -> PathBuf {
    loop {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "cargo-fe2o3-external-project-{}-{id}",
            process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("failed to create temporary project: {error}"),
        }
    }
}

#[cfg(target_os = "linux")]
fn cargo_binding_trampoline(root: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = root.join("cargo-binding-trampoline");
    fs::create_dir_all(&directory).expect("create Cargo binding trampoline directory");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .expect("protect Cargo binding trampoline directory");
    let trampoline = directory.join("fe2o3-cargo-binding-trampoline");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("cargo-fe2o3 manifest is inside the repository");
    let status = Command::new(repository.join("scripts/fe2o3-rustc-trampoline-build.sh"))
        .arg("--cargo-binding")
        .arg(&trampoline)
        .status()
        .expect("build Cargo binding trampoline fixture");
    assert!(status.success(), "Cargo binding trampoline build failed");
    trampoline
}

fn cargo_fe2o3_command() -> Command {
    cargo_fe2o3_command_with_program(Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3")))
}

fn cargo_fe2o3_command_with_program(program: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .env_remove("RUSTUP_HOME")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTC")
        .env_remove("CARGO_BUILD_RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_TARGET_DIR");
    scrub_test_harness_rustup_environment(&mut command);
    scrub_test_harness_dynamic_loader_environment(&mut command);
    command
}

fn scrub_test_harness_rustup_environment(command: &mut Command) {
    for (name, _) in env::vars_os() {
        if os_bytes(&name).starts_with(b"RUSTUP_") {
            command.env_remove(name);
        }
    }
}

fn scrub_test_harness_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in env::vars_os() {
        let name_bytes = os_bytes(&name);
        if name_bytes.starts_with(b"LD_")
            || name_bytes.starts_with(b"DYLD_")
            || name_bytes == b"GLIBC_TUNABLES"
        {
            command.env_remove(name);
        }
    }
}

#[cfg(unix)]
fn rustc_fixture_path(root: &Path) -> OsString {
    let rustc = rustc_fixture_executable(root);
    let bin = rustc.parent().expect("rustc fixture bin").to_path_buf();
    let mut paths = vec![bin.clone()];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    env::join_paths(paths).expect("construct rustc fixture PATH")
}

#[cfg(unix)]
fn rustc_fixture_executable(root: &Path) -> PathBuf {
    let toolchain = root.join("pinned-rustc-fixture-toolchain");
    let bin = toolchain.join("bin");
    let library = toolchain.join("lib");
    fs::create_dir_all(&bin).expect("create pinned rustc fixture bin directory");
    fs::create_dir_all(&library).expect("create pinned rustc fixture library directory");
    let rustc = bin.join("rustc");
    if !rustc.exists() {
        fs::copy(env!("CARGO_BIN_EXE_cargo-fe2o3-rustc-fixture"), &rustc)
            .expect("install pinned rustc fixture");
        fs::write(library.join("runtime-marker"), b"fixture-runtime-v1")
            .expect("install pinned rustc runtime fixture");
    }
    rustc
}

#[cfg(unix)]
fn release_rustc_fixture_executable(root: &Path) -> PathBuf {
    let toolchain = root.join("pinned-release-rustc-fixture-toolchain");
    let bin = toolchain.join("bin");
    let library = toolchain.join("lib");
    fs::create_dir_all(&bin).expect("create pinned release rustc fixture bin directory");
    fs::create_dir_all(&library).expect("create pinned release rustc fixture library directory");
    let rustc = bin.join("rustc");
    if !rustc.exists() {
        fs::copy(
            env!("CARGO_BIN_EXE_cargo-fe2o3-release-rustc-fixture"),
            &rustc,
        )
        .expect("install pinned release rustc fixture");
        fs::write(
            library.join("runtime-marker"),
            b"release-fixture-runtime-v1",
        )
        .expect("install pinned release rustc runtime fixture");
    }
    rustc
}

fn resolved_real_rustc() -> OsString {
    if let Some(rustc) = env::var_os("RUSTC") {
        return rustc;
    }
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join("rustc");
        if candidate.is_file() {
            return candidate.into_os_string();
        }
    }
    panic!("test environment has no rustc executable")
}

fn authority_rustc_sha256(root: &Path) -> String {
    let _ = rustc_fixture_path(root);
    file_sha256(&root.join("pinned-rustc-fixture-toolchain/bin/rustc"))
}

fn authority_rustc_runtime_sha256(root: &Path) -> String {
    let _ = rustc_fixture_path(root);
    runtime_tree_sha256(&root.join("pinned-rustc-fixture-toolchain/lib"))
}

fn authority_cargo_sha256() -> String {
    file_sha256(Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture")))
}

fn write_authority_lockfile(workspace: &Path) {
    fs::write(
        workspace.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"external-standalone\"\nversion = \"0.1.0\"\n",
    )
    .expect("write protected release authority lockfile");
}

fn protected_release_has_published_hsaco(target: &Path) -> bool {
    fs::read_dir(target.join("fe2o3"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension() == Some(OsStr::new("hsaco")))
}

#[cfg(target_os = "linux")]
fn process_observation_for_test(pid: i32) -> Option<(u8, u64)> {
    let bytes = fs::read(format!("/proc/{pid}/stat")).ok()?;
    let close = bytes.iter().rposition(|byte| *byte == b')')?;
    let fields = std::str::from_utf8(bytes.get(close + 2..)?).ok()?;
    let mut fields = fields.split_ascii_whitespace();
    let state = *fields.next()?.as_bytes().first()?;
    let start_ticks = fields.nth(18)?.parse().ok()?;
    Some((state, start_ticks))
}

#[cfg(target_os = "linux")]
fn process_start_ticks_for_test(pid: i32) -> Option<u64> {
    process_observation_for_test(pid).map(|(_, start_ticks)| start_ticks)
}

#[cfg(target_os = "linux")]
fn wait_for_process_stop(pid: i32, start_ticks: u64, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match process_observation_for_test(pid) {
            None => return true,
            Some((b'Z', _)) => return true,
            Some((_, observed_start)) if observed_start != start_ticks => return true,
            Some(_) => {}
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn file_sha256(path: &Path) -> String {
    let bytes = fs::read(path).expect("read provisioned executable image");
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(unix)]
fn runtime_tree_sha256(root: &Path) -> String {
    fn hash_field(hash: &mut Sha256, value: &[u8]) {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }

    fn hash_directory(hash: &mut Sha256, directory: &Path) {
        let mut entries = fs::read_dir(directory)
            .expect("read rustc runtime fixture")
            .map(|entry| entry.expect("read rustc runtime entry"))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.file_name()
                .as_bytes()
                .cmp(right.file_name().as_bytes())
        });
        hash.update(b"directory\0");
        for entry in entries {
            hash_field(hash, entry.file_name().as_bytes());
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("inspect rustc runtime entry");
            if metadata.is_file() {
                let bytes = fs::read(&path).expect("read rustc runtime entry");
                hash.update(b"file\0");
                hash.update((metadata.mode() & 0o7777).to_le_bytes());
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            } else if metadata.is_dir() {
                hash.update(b"subdirectory\0");
                hash.update((metadata.mode() & 0o7777).to_le_bytes());
                hash_directory(hash, &path);
            } else {
                panic!("unsupported rustc runtime fixture entry {path:?}");
            }
        }
        hash.update(b"end-directory\0");
    }

    let mut hash = Sha256::new();
    hash.update(b"fe2o3-rustc-runtime-tree-v1\0");
    hash_directory(&mut hash, root);
    file_digest_hex(hash.finalize().into())
}

fn file_digest_hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn take_u64(bytes: &mut &[u8]) -> u64 {
    let raw: [u8; 8] = bytes[..8].try_into().expect("u64 field");
    *bytes = &bytes[8..];
    u64::from_le_bytes(raw)
}

fn take_field(bytes: &mut &[u8]) -> Vec<u8> {
    let len = take_u64(bytes) as usize;
    let field = bytes[..len].to_vec();
    *bytes = &bytes[len..];
    field
}

fn bytes(path: &Path) -> Vec<u8> {
    os_bytes(path.as_os_str()).to_vec()
}

fn strings(values: &[&str]) -> Vec<Vec<u8>> {
    values
        .iter()
        .map(|value| value.as_bytes().to_vec())
        .collect()
}

fn without_injected_runner(arguments: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut filtered = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == b"--config"
            && arguments.get(index + 1).is_some_and(|value| {
                value.starts_with(b"target.") && value.windows(8).any(|part| part == b".runner=")
            })
        {
            index += 2;
            continue;
        }
        filtered.push(arguments[index].clone());
        index += 1;
    }
    filtered
}

#[test]
fn standalone_build_preserves_manifest_package_and_target_selection() {
    let mut fixture = ProjectFixture::standalone();
    let custom_target = fixture.root.join("custom-target");
    fixture.target = custom_target.clone();
    let manifest = fixture.workspace.join("Cargo.toml");
    let args = vec![
        OsString::from("build"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--package=external-standalone"),
        OsString::from("--target-dir"),
        custom_target.as_os_str().to_owned(),
        OsString::from("--release"),
    ];

    let output = fixture.run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let records = fixture.invocations();
    assert_eq!(records.len(), 2, "{records:#?}");
    assert_eq!(records[0].cwd, bytes(&fixture.cwd));
    assert_eq!(
        records[0].args[1..],
        [
            strings(&[
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
            ]),
            vec![bytes(&manifest)],
        ]
        .concat()
    );
    assert_eq!(records[1].cwd, bytes(&fixture.cwd));
    assert_eq!(
        records[1].args[1..],
        args.iter()
            .map(|arg| os_bytes(arg).to_vec())
            .collect::<Vec<_>>()
    );
    assert!(records[1].hsaco_dir.is_empty());
    assert_eq!(records[1].target, b"gfx942");
    assert!(!records[1].wrapper.is_empty());
    assert!(records[1].rustflags.is_empty());
    assert!(records[1].encoded_rustflags.is_empty());
    assert!(!records[1].managed_rustc_args.is_empty());
    assert_eq!(records[1].cargo_target_dir, Vec::<u8>::new());
    assert_eq!(
        fs::read(custom_target.join("fe2o3/fixture.hsaco")).expect("read generated sidecar"),
        b"fixture-sidecar"
    );
}

#[test]
fn virtual_workspace_run_uses_member_cwd_and_workspace_target() {
    let fixture = ProjectFixture::virtual_workspace();
    let args = ["run", "-p", "selected-member", "--", "application-argument"].map(OsString::from);

    let output = fixture.run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let records = fixture.invocations();
    assert_eq!(records.len(), 2, "{records:#?}");
    assert_eq!(records[1].cwd, bytes(&fixture.cwd));
    assert_eq!(
        without_injected_runner(&records[1].args[1..]),
        strings(&["run", "-p", "selected-member", "--", "application-argument"])
    );
    assert!(records[1].hsaco_dir.is_empty());
}

#[test]
fn inherited_cargo_target_dir_controls_generated_output() {
    let mut fixture = ProjectFixture::standalone();
    fixture.target = fixture.root.join("environment-target");
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("CARGO_TARGET_DIR", &fixture.target);

    let output = command.output().expect("run with CARGO_TARGET_DIR");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records = fixture.invocations();
    assert_eq!(records[1].cargo_target_dir, bytes(&fixture.target));
    assert!(records[1].hsaco_dir.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn ordinary_build_accepts_caller_loader_state_but_scrubs_managed_children() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("LD_LIBRARY_PATH", "/ordinary/non-authoritative/runtime")
        .env("FE2O3_TEST_EXPECT_CALLER_LOADER_ENV_SCRUBBED_V1", "1");

    let output = command
        .output()
        .expect("run ordinary loader compatibility probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.target.join("fe2o3/fixture.hsaco").is_file());
}

#[test]
fn managed_selector_stays_child_only_after_backend_source_replacement() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("FE2O3_TEST_REPLACE_BACKEND", &fixture.backend);

    let output = command.output().expect("run backend replacement probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let invocations = fixture.invocations();
    assert!(
        invocations[1]
            .managed_rustc_args
            .windows(b"-Zcodegen-backend=/proc/./self/fd/198".len())
            .any(|window| window == b"-Zcodegen-backend=/proc/./self/fd/198")
    );
    assert!(invocations[1].hsaco_dir.is_empty());
    assert_eq!(
        fs::read(&fixture.backend).expect("read replaced source"),
        b"replacement backend"
    );
}

#[test]
fn source_backend_build_uses_an_isolated_external_target() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env_remove("FE2O3_BACKEND")
        .env("CARGO_PROFILE_DEV_DEBUG", "0");

    let output = command.output().expect("run isolated source backend build");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fixture
            .target
            .join(".fe2o3-backend-build-v1/debug/librustc_codegen_fe2o3.so")
            .is_file()
    );
    let records = fixture.invocations();
    assert_eq!(records.len(), 3, "{records:#?}");
    assert!(
        records[1]
            .args
            .windows(2)
            .any(|pair| { pair[0] == b"--target-dir" && pair[1] == b"/proc/self/fd/196" })
    );
    assert!(records[1].cargo_target_dir.is_empty());
    assert!(records[1].hsaco_dir.is_empty());
    assert!(records[1].rustflags.is_empty());
    assert!(records[1].encoded_rustflags.is_empty());
    assert!(records[1].managed_rustc_args.is_empty());
}

#[test]
fn caller_rustflags_are_preserved_for_cargo_and_managed_flags_are_separate() {
    let fixture = ProjectFixture::standalone();
    let encoded = OsString::from("-Copt-level=1\x1f--cfg\x1ffrom_encoded");
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("RUSTFLAGS", "--cfg from_raw")
        .env("CARGO_ENCODED_RUSTFLAGS", &encoded);

    let output = command.output().expect("run preserved rustflags probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records = fixture.invocations();
    assert_eq!(records[1].rustflags, b"--cfg from_raw");
    assert_eq!(records[1].encoded_rustflags, os_bytes(&encoded));
    assert!(!records[1].managed_rustc_args.is_empty());
}

#[test]
fn real_cargo_cooperatively_routes_capabilities_to_the_managed_rustc_child() {
    let fixture = ProjectFixture::standalone();
    let bin_dir = fixture.workspace.join("src/bin");
    fs::create_dir_all(&bin_dir).expect("create parallel rustc fixtures");
    for name in ["parallel_a", "parallel_b", "parallel_c", "parallel_d"] {
        fs::write(bin_dir.join(format!("{name}.rs")), "fn main() {}\n")
            .expect("write parallel rustc fixture");
    }
    let report = fixture.root.join("rustc-capabilities.log");
    let real_rustc = resolved_real_rustc();
    let fixture_path = rustc_fixture_path(&fixture.root);
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = cargo_fe2o3_command();
    command
        .args(["build", "-j", "4"])
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("PATH", fixture_path)
        .env_remove("RUSTC")
        .env("FE2O3_TEST_REAL_RUSTC", real_rustc)
        .env("FE2O3_TEST_RUSTC_CAPABILITY_REPORT", &report)
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR");

    let output = command.output().expect("run real Cargo capability probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(report).expect("read rustc capability report");
    assert!(report.contains("external_standalone:probe_"), "{report}");
    for name in ["parallel_a", "parallel_b", "parallel_c", "parallel_d"] {
        assert!(report.contains(&format!("{name}:probe_")), "{report}");
    }
    assert!(
        report
            .lines()
            .all(|line| line.ends_with(":fd199_open=false")),
        "ordinary managed rustc inherited the protected V3 descriptor:\n{report}"
    );
    assert!(
        fs::read_dir(fixture.target.join("fe2o3"))
            .expect("read committed artifact directory")
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".hsaco"))
    );
}

#[test]
fn real_cargo_routes_workspace_dependencies_through_builtin_llvm() {
    let fixture = ProjectFixture::standalone();
    let dependency = fixture.workspace.join("host-dependency");
    fs::create_dir_all(dependency.join("src")).expect("create host dependency");
    fs::write(
        fixture.workspace.join("Cargo.toml"),
        "[package]\nname='external-standalone'\nversion='0.1.0'\nedition='2024'\n\
         [dependencies]\nhost-dependency={path='host-dependency'}\n\
         [workspace]\nmembers=['host-dependency']\ndefault-members=['.']\nresolver='3'\n",
    )
    .expect("write workspace dependency manifest");
    fs::write(
        dependency.join("Cargo.toml"),
        "[package]\nname='host-dependency'\nversion='0.1.0'\nedition='2024'\n",
    )
    .expect("write host dependency manifest");
    fs::write(
        dependency.join("src/lib.rs"),
        "pub fn value() -> u32 { 7 }\n",
    )
    .expect("write host dependency source");
    fs::write(
        fixture.workspace.join("src/main.rs"),
        "fn main() { assert_eq!(host_dependency::value(), 7); }\n",
    )
    .expect("write dependency-using root source");

    let route_report = fixture.root.join("rustc-routes.log");
    let capability_report = fixture.root.join("rustc-capabilities.log");
    let mut command = cargo_fe2o3_command();
    command
        .args(["build", "-j", "1"])
        .current_dir(&fixture.workspace)
        .env(
            "CARGO",
            env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")),
        )
        .env("PATH", rustc_fixture_path(&fixture.root))
        .env_remove("RUSTC")
        .env("FE2O3_TEST_REAL_RUSTC", resolved_real_rustc())
        .env("FE2O3_TEST_RUSTC_ROUTE_REPORT", &route_report)
        .env("FE2O3_TEST_RUSTC_CAPABILITY_REPORT", capability_report)
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR");

    let output = command
        .output()
        .expect("run workspace dependency route probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(route_report).expect("read rustc route report");
    let dependency_route = report
        .lines()
        .find(|line| line.starts_with("host_dependency:"))
        .expect("host dependency route was recorded");
    assert_eq!(
        dependency_route,
        "host_dependency:managed_backend=false:managed_mir=false:qualification=false:backend_env=false:artifact=false:attempt=false"
    );
    let root_route = report
        .lines()
        .find(|line| line.starts_with("external_standalone:"))
        .expect("selected root route was recorded");
    assert!(root_route.contains("managed_backend=true"), "{root_route}");
    assert!(root_route.contains("managed_mir=true"), "{root_route}");
    assert!(root_route.contains("artifact=true"), "{root_route}");
    assert!(root_route.contains("attempt=true"), "{root_route}");
}

#[cfg(target_os = "linux")]
#[test]
fn ordinary_build_script_process_inherits_no_raw_capability_descriptors() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("ordinary-build-script-capabilities.log");
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("FE2O3_TEST_BUILD_SCRIPT_MODE", "ordinary")
        .env("FE2O3_TEST_BUILD_SCRIPT_REPORT", &report)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_FIXTURE",
            env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        );

    let output = command.output().expect("run ordinary build-script probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(report).expect("read ordinary build-script report");
    assert!(report.contains("backend_open=false"), "{report}");
    assert!(report.contains("artifact_open=false"), "{report}");
    assert!(report.contains("fd199_open=false"), "{report}");
}

#[cfg(target_os = "linux")]
#[test]
fn fake_cargo_build_script_cannot_replay_the_genuine_wrapper() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("exec-build-script-capabilities.log");
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("FE2O3_TEST_BUILD_SCRIPT_MODE", "exec-wrapper")
        .env("FE2O3_TEST_BUILD_SCRIPT_REPORT", &report)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_FIXTURE",
            env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        );

    let output = command.output().expect("run exec build-script probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !report.exists(),
        "unauthorized wrapper replay reached its attacker-selected compiler"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn build_script_execveat_cannot_replay_the_genuine_wrapper_and_live_broker() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("execveat-build-script-capabilities.log");
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("FE2O3_TEST_BUILD_SCRIPT_MODE", "execveat-wrapper")
        .env("FE2O3_TEST_BUILD_SCRIPT_REPORT", &report)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_FIXTURE",
            env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        );

    let output = command.output().expect("run execveat build-script probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !report.exists(),
        "execveat wrapper replay reached its attacker-selected compiler"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn substituted_multithreaded_image_cannot_replay_wrapper_from_non_leader_thread() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = ProjectFixture::standalone();
    let staged_wrapper = fixture.root.join("cargo-fe2o3-observed-wrapper");
    let genuine_wrapper = fixture.root.join("cargo-fe2o3-genuine-wrapper");
    let displaced_wrapper = fixture.root.join("cargo-fe2o3-displaced-wrapper");
    let substitute = fixture.root.join("cargo-fe2o3-hostile-substitute");
    let race_trace = fixture.root.join("wrapper-race.log");
    let thread_trace = fixture.root.join("multithreaded-image.log");
    let compiler_report = fixture.root.join("multithreaded-compiler.log");

    fs::copy(env!("CARGO_BIN_EXE_cargo-fe2o3"), &staged_wrapper)
        .expect("stage independently replaceable cargo-fe2o3 wrapper");
    fs::hard_link(&staged_wrapper, &genuine_wrapper).expect("retain genuine wrapper hard link");
    fs::copy(
        env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        &substitute,
    )
    .expect("stage hostile multithreaded image");
    let mut permissions = fs::metadata(&substitute).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&substitute, permissions).expect("make hostile substitute executable");

    let mut command = Command::new(&staged_wrapper);
    let harness_loader = env::var_os("LD_LIBRARY_PATH")
        .expect("Cargo test harness must provide LD_LIBRARY_PATH for this regression");
    command
        .arg("build")
        .current_dir(&fixture.cwd)
        .env("CARGO", env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture"))
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_TEST_CARGO_LOG", &fixture.log)
        .env("FE2O3_TEST_WORKSPACE_ROOT", &fixture.workspace)
        .env("FE2O3_TEST_TARGET_DIRECTORY", &fixture.target)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_MODE",
            "multithreaded-substitute-wrapper",
        )
        .env("FE2O3_TEST_BUILD_SCRIPT_REPORT", &compiler_report)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_FIXTURE",
            env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        )
        .env("FE2O3_TEST_GENUINE_WRAPPER", &genuine_wrapper)
        .env("FE2O3_TEST_WRAPPER_SUBSTITUTE", &substitute)
        .env("FE2O3_TEST_DISPLACED_WRAPPER", &displaced_wrapper)
        .env("FE2O3_TEST_WRAPPER_RACE_TRACE", &race_trace)
        .env("FE2O3_TEST_MULTITHREADED_TRACE", &thread_trace)
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .env("LD_LIBRARY_PATH", harness_loader)
        .env_remove("RUSTUP_HOME")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTC")
        .env_remove("CARGO_BUILD_RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");

    let output =
        output_retrying_text_file_busy(&mut command, "run multithreaded wrapper replay probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let race = fs::read_to_string(race_trace).expect("read wrapper substitution trace");
    assert!(race.contains("substituted=true"), "{race}");
    let thread = fs::read_to_string(thread_trace).expect("read hostile image trace");
    assert!(thread.contains("non_leader=true"), "{thread}");
    assert!(thread.contains("backend_open=false"), "{thread}");
    assert!(thread.contains("artifact_open=false"), "{thread}");
    assert!(
        !compiler_report.exists(),
        "non-leader replay reached its attacker-selected compiler or artifact authority"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn real_cargo_build_script_cannot_replay_the_genuine_wrapper_and_live_broker() {
    let fixture = ProjectFixture::standalone();
    fs::write(
        fixture.workspace.join("Cargo.toml"),
        "[package]\nname='external-standalone'\nversion='0.1.0'\nedition='2024'\nbuild='build.rs'\n",
    )
    .expect("write real build-script manifest");
    fs::write(
        fixture.workspace.join("build.rs"),
        r#"use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let wrapper = env::var_os("RUSTC_WORKSPACE_WRAPPER").unwrap();
    let compiler = env::var_os("FE2O3_TEST_BUILD_SCRIPT_FIXTURE").unwrap();
    let report = PathBuf::from(env::var_os("FE2O3_TEST_BUILD_SCRIPT_REPORT").unwrap());
    let source = report.with_extension("rs");
    fs::write(&source, "pub fn replayed() {}\n").unwrap();
    let error = Command::new(wrapper)
        .arg(compiler)
        .args([
            "--crate-name",
            "real_build_script_replay",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-Cmetadata=real-build-script-replay",
        ])
        .arg(source)
        .exec();
    panic!("could not exec inherited wrapper: {error}");
}
"#,
    )
    .expect("write real build script");

    let report = fixture.root.join("real-build-script-capabilities.log");
    let rustc_report = fixture
        .root
        .join("real-build-script-rustc-capabilities.log");
    let real_rustc = resolved_real_rustc();
    let fixture_path = rustc_fixture_path(&fixture.root);
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = cargo_fe2o3_command();
    command
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("PATH", fixture_path)
        .env_remove("RUSTC")
        .env("FE2O3_TEST_REAL_RUSTC", real_rustc)
        .env("FE2O3_TEST_RUSTC_CAPABILITY_REPORT", &rustc_report)
        .env("FE2O3_TEST_BUILD_SCRIPT_REPORT", &report)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_FIXTURE",
            env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        )
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR");

    let output = command
        .output()
        .expect("run real Cargo build-script replay");
    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not match the parent-pinned compiler")
            || stderr.contains("failed to receive brokered capabilities")
            || stderr.contains("Connection reset by peer"),
        "{stderr}"
    );
    assert!(
        !report.exists(),
        "unauthorized wrapper replay reached its attacker-selected compiler"
    );
}

#[cfg(target_os = "linux")]
#[cfg(debug_assertions)]
#[test]
fn authoritative_kernel_preflight_rejects_a_hostile_custom_build() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("preflight-build-script-ran");
    fs::write(
        fixture.workspace.join("Cargo.toml"),
        "[package]\nname='external-standalone'\nversion='0.1.0'\nedition='2024'\nbuild='build.rs'\n",
    )
    .expect("write hostile build manifest");
    fs::write(
        fixture.workspace.join("build.rs"),
        format!("fn main() {{ std::fs::write({report:?}, b\"ran\").unwrap(); }}\n"),
    )
    .expect("write hostile build script");
    fs::write(
        fixture.workspace.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"external-standalone\"\nversion = \"0.1.0\"\n",
    )
    .expect("write frozen hostile-build lockfile");
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo_sha256 = file_sha256(
        &fs::canonicalize(&real_cargo).expect("canonicalize authoritative Cargo executable"),
    );
    let output = cargo_fe2o3_command()
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("PATH", rustc_fixture_path(&fixture.root))
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TEST_REAL_RUSTC", resolved_real_rustc())
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_PATH_V1",
            rustc_fixture_executable(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", cargo_sha256)
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        )
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .output()
        .expect("run authoritative custom-build preflight");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("rejects unreviewed custom-build package \"external-standalone\""),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!report.exists(), "rejected custom build executed");
}

#[cfg(target_os = "linux")]
#[cfg(debug_assertions)]
#[test]
fn authoritative_kernel_preflight_rejects_a_hostile_proc_macro() {
    let fixture = ProjectFixture::standalone();
    let macro_root = fixture.workspace.join("hostile-macro");
    fs::create_dir_all(macro_root.join("src")).expect("create hostile proc-macro fixture");
    fs::write(
        fixture.workspace.join("Cargo.toml"),
        "[package]\nname='external-standalone'\nversion='0.1.0'\nedition='2024'\n\
         [dependencies]\nhostile-macro={path='hostile-macro'}\n\
         [workspace]\nmembers=['hostile-macro']\nresolver='2'\n",
    )
    .expect("write hostile proc-macro host manifest");
    fs::write(
        macro_root.join("Cargo.toml"),
        "[package]\nname='hostile-macro'\nversion='0.1.0'\nedition='2024'\n\
         [lib]\nproc-macro=true\n",
    )
    .expect("write hostile proc-macro manifest");
    fs::write(
        macro_root.join("src/lib.rs"),
        "extern crate proc_macro;\n#[proc_macro]\npub fn hostile(_: proc_macro::TokenStream) -> proc_macro::TokenStream { panic!(\"must not run\") }\n",
    )
    .expect("write hostile proc-macro source");
    fs::write(
        fixture.workspace.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"external-standalone\"\nversion = \"0.1.0\"\ndependencies = [\n \"hostile-macro\",\n]\n\n[[package]]\nname = \"hostile-macro\"\nversion = \"0.1.0\"\n",
    )
    .expect("write frozen proc-macro lockfile");
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo_sha256 = file_sha256(
        &fs::canonicalize(&real_cargo).expect("canonicalize authoritative Cargo executable"),
    );
    let output = cargo_fe2o3_command()
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("PATH", rustc_fixture_path(&fixture.root))
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TEST_REAL_RUSTC", resolved_real_rustc())
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_PATH_V1",
            rustc_fixture_executable(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", cargo_sha256)
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        )
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .output()
        .expect("run authoritative proc-macro preflight");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("rejects an unreviewed procedural macro"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn trusted_procedural_macro_documents_in_process_descriptor_visibility() {
    let fixture = ProjectFixture::standalone();
    let macro_root = fixture.workspace.join("fd-probe-macro");
    fs::create_dir_all(macro_root.join("src")).expect("create procedural macro fixture");
    fs::write(
        fixture.workspace.join("Cargo.toml"),
        "[package]\nname='external-standalone'\nversion='0.1.0'\nedition='2024'\n\
         [dependencies]\nfd-probe-macro={path='fd-probe-macro'}\n\
         [workspace]\nmembers=['fd-probe-macro']\nresolver='2'\n",
    )
    .expect("write procedural macro host manifest");
    fs::write(
        macro_root.join("Cargo.toml"),
        "[package]\nname='fd-probe-macro'\nversion='0.1.0'\nedition='2024'\n\
         [lib]\nproc-macro=true\n",
    )
    .expect("write procedural macro manifest");
    fs::write(
        macro_root.join("src/lib.rs"),
        r#"extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn probe(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    let backend = std::fs::symlink_metadata("/proc/self/fd/198").is_ok();
    let artifact = std::fs::symlink_metadata("/proc/self/fd/197").is_ok();
    let wrapper = std::env::var_os("RUSTC_WORKSPACE_WRAPPER").unwrap();
    let compiler = std::env::var_os("FE2O3_TEST_BUILD_SCRIPT_FIXTURE").unwrap();
    let replay_report = std::path::PathBuf::from(
        std::env::var_os("FE2O3_TEST_BUILD_SCRIPT_REPORT").unwrap(),
    );
    let source = replay_report.with_extension("rs");
    std::fs::write(&source, "pub fn replayed() {}\n").unwrap();
    let replay_succeeded = std::process::Command::new(wrapper)
        .arg(compiler)
        .args([
            "--crate-name",
            "proc_macro_replay",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-Cmetadata=proc-macro-replay",
        ])
        .arg(source)
        .status()
        .is_ok_and(|status| status.success());
    let report = std::env::var_os("FE2O3_TEST_PROC_MACRO_REPORT").unwrap();
    std::fs::write(
        report,
        format!(
            "backend_open={backend}\nartifact_open={artifact}\nreplay_succeeded={replay_succeeded}\n"
        ),
    )
    .unwrap();
    item
}
"#,
    )
    .expect("write procedural macro source");
    fs::write(
        fixture.workspace.join("src/main.rs"),
        "#[fd_probe_macro::probe]\nfn main() {}\n",
    )
    .expect("write procedural macro host source");

    let proc_macro_report = fixture.root.join("proc-macro-capabilities.log");
    let replay_report = fixture.root.join("proc-macro-replay-capabilities.log");
    let rustc_report = fixture.root.join("proc-macro-rustc-capabilities.log");
    let real_rustc = resolved_real_rustc();
    let fixture_path = rustc_fixture_path(&fixture.root);
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = cargo_fe2o3_command();
    command
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("PATH", fixture_path)
        .env_remove("RUSTC")
        .env("FE2O3_TEST_REAL_RUSTC", real_rustc)
        .env("FE2O3_TEST_RUSTC_CAPABILITY_REPORT", &rustc_report)
        .env("FE2O3_TEST_PROC_MACRO_REPORT", &proc_macro_report)
        .env("FE2O3_TEST_BUILD_SCRIPT_REPORT", &replay_report)
        .env(
            "FE2O3_TEST_BUILD_SCRIPT_FIXTURE",
            env!("CARGO_BIN_EXE_cargo-fe2o3-build-script-fixture"),
        )
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR");

    let output = command
        .output()
        .expect("run procedural macro capability probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(proc_macro_report).expect("read procedural macro report");
    assert!(report.contains("backend_open=true"), "{report}");
    assert!(report.contains("artifact_open=true"), "{report}");
    assert!(report.contains("replay_succeeded=false"), "{report}");
    assert!(
        !replay_report.exists(),
        "procedural macro wrapper replay reached its attacker-selected compiler"
    );
}

#[test]
fn effective_cargo_configuration_changes_generation_identity() {
    let fixture = ProjectFixture::standalone();
    let mut first = fixture.command(&[OsString::from("build")]);
    first.env(
        "FE2O3_TEST_BUILD_CONFIG_JSON",
        r#"{"rustflags":["--cfg","first"]}"#,
    );
    let first_output = first.output().expect("run first configured build");
    assert!(
        first_output.status.success(),
        "first configured build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first_output.stdout),
        String::from_utf8_lossy(&first_output.stderr),
    );

    let mut second = fixture.command(&[OsString::from("build")]);
    second.env(
        "FE2O3_TEST_BUILD_CONFIG_JSON",
        r#"{"rustflags":["--cfg","second"]}"#,
    );
    let second_output = second.output().expect("run second configured build");
    assert!(
        second_output.status.success(),
        "second configured build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second_output.stdout),
        String::from_utf8_lossy(&second_output.stderr),
    );

    let records = fixture.invocations();
    assert_ne!(records[1].managed_rustc_args, records[3].managed_rustc_args);
}

#[test]
fn configured_rustc_wrappers_are_rejected_before_build() {
    for (variable, configured) in [
        ("FE2O3_TEST_RUSTC_WRAPPER_JSON", r#""/tmp/outer-wrapper""#),
        (
            "FE2O3_TEST_RUSTC_WORKSPACE_WRAPPER_JSON",
            r#""/tmp/workspace-wrapper""#,
        ),
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.command(&[OsString::from("build")]);
        command.env(variable, configured);
        let output = command.output().expect("run wrapper rejection probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("cannot compose"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join("fe2o3").exists());
    }
}

#[test]
fn arbitrary_rustc_environment_is_rejected_before_artifact_authority() {
    for variable in ["RUSTC", "CARGO_BUILD_RUSTC"] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.command(&[OsString::from("build")]);
        command.env(variable, "/tmp/attacker-rustc");
        let output = command
            .output()
            .expect("run rustc override rejection probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("rejects preexisting compiler selection"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join("fe2o3").exists());
        assert!(
            !fixture.log.exists(),
            "Cargo ran before rejecting {variable}"
        );
    }
}

#[test]
fn configured_arbitrary_rustc_is_rejected_before_artifact_authority() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("FE2O3_TEST_RUSTC_JSON", r#""/tmp/attacker-rustc""#);
    let output = command
        .output()
        .expect("run configured rustc rejection probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("rejects configured compiler selection build.rustc"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.target.join("fe2o3").exists());
    assert!(
        fixture
            .invocations()
            .iter()
            .all(|invocation| invocation.args.get(1).is_none_or(|arg| arg != b"build")),
        "Cargo build ran after config rejection"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn loader_injection_environment_is_rejected_before_artifact_authority() {
    for variable in [
        "LD_PRELOAD",
        "LD_AUDIT",
        "LD_DEBUG",
        "DYLD_INSERT_LIBRARIES",
        "GLIBC_TUNABLES",
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env(variable, "/definitely/not/a/fe2o3-loader-object.so");
        let output = command.output().expect("run loader rejection probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("rejects dynamic-loader injection variable"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join("fe2o3").exists());
        assert!(
            !fixture.log.exists(),
            "Cargo ran before rejecting {variable}"
        );
    }
}

#[test]
fn configured_loader_environment_is_rejected_before_artifact_authority() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env(
        "FE2O3_TEST_ENV_CONFIG_JSON",
        r#"{"LD_PRELOAD":{"value":"/tmp/attacker.so","force":true}}"#,
    );
    let output = command
        .output()
        .expect("run configured loader rejection probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("rejects configured dynamic-loader environment env.LD_PRELOAD"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.target.join("fe2o3").exists());
    assert!(
        fixture
            .invocations()
            .iter()
            .all(|invocation| invocation.args.get(1).is_none_or(|arg| arg != b"build")),
        "Cargo build ran after config rejection"
    );
}

#[cfg(debug_assertions)]
#[test]
fn authority_build_requires_an_independent_exact_rustc_digest() {
    let missing = ProjectFixture::standalone();
    let mut command = missing.authority_command(&[OsString::from("build")]);
    command.env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1");
    let output = command.output().expect("run missing rustc pin probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires FE2O3_AUTHORITY_RUSTC_SHA256_V1"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!missing.target.join(".fe2o3-backend-build-v1").exists());

    for digest in ["01".repeat(32), "00".repeat(32), "A1".repeat(32)] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env("FE2O3_AUTHORITY_RUSTC_SHA256_V1", digest)
            .env(
                "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
                authority_rustc_runtime_sha256(&fixture.root),
            )
            .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
            .env(
                "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
                file_sha256(&fixture.backend),
            );
        let output = command.output().expect("run invalid rustc pin probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("FE2O3_AUTHORITY_RUSTC_SHA256_V1"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join(".fe2o3-backend-build-v1").exists());
    }
}

#[test]
fn authority_release_fails_before_executing_cargo_without_a_protected_launcher() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.authority_command(&[OsString::from("build")]);
    command
        .env_remove("FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1");
    let output = command.output().expect("run protected-launch gate probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires a protected pre-exec launcher/image contract"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.log.exists(), "authority gate executed Cargo");
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_probe_mints_only_a_real_launcher_handoff() {
    let fixture = ProjectFixture::standalone();
    let output = fixture
        .isolated_protected_release_command("probe")
        .output()
        .expect("run protected release handoff probe");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("probe output is UTF-8");
    assert!(
        stdout.contains("FE2O3_PROTECTED_AUTHORITY_RELEASE_V1_OK"),
        "{stdout}"
    );
    assert!(stdout.contains("runtime_authority=none"), "{stdout}");
    assert!(stdout.contains("gpu_authority=none"), "{stdout}");
    assert!(!fixture.log.exists(), "release probe executed Cargo");
    assert!(
        !fixture.target.join("fe2o3").exists(),
        "release probe created an artifact generation"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_obsolete_and_qualification_selectors_before_launch() {
    let fixture = ProjectFixture::standalone();
    for environment in ["FE2O3_CODEGEN_PIPELINE", "FE2O3_QUALIFICATION_ORACLE_V1"] {
        for selector in [
            "production-v1",
            "collected-row-softmax-v1",
            "kernel-ir-worker-v2",
        ] {
            let output = fixture
                .isolated_protected_release_command("probe")
                .env(environment, selector)
                .output()
                .expect("run protected release selector rejection");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "selector {selector:?}: {stderr}");
            assert!(
                stderr.contains("rejects unexpected inherited environment")
                    && stderr.contains(environment),
                "selector {selector:?}: {stderr}",
            );
        }
    }
    assert!(!fixture.log.exists(), "selector rejection executed Cargo");
    assert!(
        !fixture.target.join("fe2o3").exists(),
        "selector rejection created an artifact generation"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_preserves_rustc_custody_and_requires_a_real_compiler_handoff() {
    let fixture = ProjectFixture::standalone();
    write_authority_lockfile(&fixture.workspace);
    let output = fixture
        .protected_release_build_command()
        .output()
        .expect("run protected release build fixture");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("V3 compiler module handoff is not published"),
        "{stderr}"
    );

    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.target.join(PROTECTED_RELEASE_CARGO_REPORT))
            .expect("read protected release Cargo report"),
    )
    .expect("decode protected release Cargo report");
    assert_eq!(report["parent_death_signal"], libc::SIGKILL);
    assert_eq!(report["target"], "gfx942:xnack-");
    let args = report["args"]
        .as_array()
        .expect("protected release Cargo args are an array");
    for required in ["build", "--offline", "--frozen"] {
        assert!(args.iter().any(|value| value == required), "{report}");
    }
    assert!(
        args.windows(2).any(|pair| {
            pair[0] == "--target" && pair[1] == fe2o3_amd_target::PRODUCTION_GFX942_RUSTC_TARGET_V1
        }),
        "{report}"
    );
    assert_eq!(report["wrapper"], "/proc/self/fd/192", "{report}");
    assert!(report["trampoline_path_input"].is_null(), "{report}");
    assert!(report["trampoline_digest_input"].is_null(), "{report}");
    assert!(
        report["managed_rustc_args"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "{report}"
    );
    assert!(
        !protected_release_has_published_hsaco(&fixture.target),
        "a custody-only rustc fixture published production output"
    );

    let fd_report: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.target.join(PROTECTED_RELEASE_RUSTC_FD_REPORT))
            .expect("read protected release rustc fd199 report"),
    )
    .expect("decode protected release rustc fd199 report");
    assert_eq!(fd_report["fd"], 199, "{fd_report}");
    assert_eq!(fd_report["invocation_authority_fd"], 195, "{fd_report}");
    assert_ne!(fd_report["fd"], fd_report["invocation_authority_fd"]);
    assert_eq!(fd_report["magic_hex"], "4645324f33524900", "{fd_report}");
    assert_eq!(fd_report["version"], 3, "{fd_report}");
    assert_eq!(fd_report["canonical_v3"], true, "{fd_report}");
    assert_eq!(fd_report["raw_compiler_closure"], false, "{fd_report}");
    assert_eq!(
        fd_report["mode"],
        serde_json::json!(libc::S_IFREG | 0o400),
        "{fd_report}"
    );
    let required_seals =
        libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
    assert_eq!(fd_report["seals"], required_seals, "{fd_report}");
    assert_eq!(fd_report["required_seals"], required_seals, "{fd_report}");
    assert_eq!(fd_report["close_on_exec"], false, "{fd_report}");
    assert_eq!(fd_report["fd195_open"], false, "{fd_report}");
    assert_eq!(fd_report["same_object_as_fd195"], false, "{fd_report}");

    let attack_fixture = ProjectFixture::standalone();
    write_authority_lockfile(&attack_fixture.workspace);
    for (attack, expected) in [
        (
            "setup-substitute",
            "reserved rustc-invocation capability descriptor 199 is already in use",
        ),
        (
            "rustc-substitute",
            "rustc-invocation capability is not an exact regular mode-0400 file",
        ),
        (
            "rustc-close",
            "cannot inspect inherited rustc-invocation capability descriptor 199",
        ),
        ("rustc-truncate", "sealed fd199 truncation was denied"),
    ] {
        let _ = fs::remove_dir_all(&attack_fixture.target);
        fs::create_dir_all(&attack_fixture.target).expect("reset fd199 attack target");
        fs::write(
            attack_fixture
                .target
                .join(PROTECTED_RELEASE_RUSTC_FD_ATTACK),
            attack,
        )
        .expect("select protected release fd199 attack");
        let output = attack_fixture
            .protected_release_build_command()
            .output()
            .expect("run protected release fd199 attack");
        assert!(
            !output.status.success(),
            "fd199 attack {attack} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "attack={attack}\nstderr:\n{stderr}"
        );
        assert!(
            !attack_fixture
                .target
                .join(PROTECTED_RELEASE_RUSTC_FD_REPORT)
                .exists(),
            "fd199 attack {attack} reached successful descriptor admission"
        );
        assert!(
            !protected_release_has_published_hsaco(&attack_fixture.target),
            "fd199 attack {attack} published authorized output"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_substituted_or_nonstatic_cargo_binding_trampoline() {
    for (path, digest, expected) in [
        (None, Some("01".repeat(32)), "does not match"),
        (
            Some(PathBuf::from("/usr/bin/env")),
            Some(file_sha256(Path::new("/usr/bin/env"))),
            "has a runtime interpreter",
        ),
    ] {
        let fixture = ProjectFixture::standalone();
        write_authority_lockfile(&fixture.workspace);
        let mut command = fixture.protected_release_build_command();
        if let Some(path) = path {
            command.env("FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1", path);
        }
        if let Some(digest) = digest {
            command.env("FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1", digest);
        }
        let output = command.output().expect("run trampoline rejection probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !fixture.target.join(PROTECTED_RELEASE_CARGO_REPORT).exists(),
            "substituted trampoline reached Cargo"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_launcher_death_kills_admitted_child_and_cargo() {
    use std::os::unix::process::ExitStatusExt;
    use std::thread;
    use std::time::{Duration, Instant};

    struct KillProcesses(Vec<i32>);
    impl Drop for KillProcesses {
        fn drop(&mut self) {
            for pid in self.0.drain(..) {
                // SAFETY: these exact PIDs came from the protected Cargo process report.
                let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
    }

    let fixture = ProjectFixture::standalone();
    write_authority_lockfile(&fixture.workspace);
    fs::create_dir_all(&fixture.target).expect("create protected release target");
    fs::write(fixture.target.join(PROTECTED_RELEASE_CARGO_HOLD), [])
        .expect("install protected release Cargo hold");
    let mut command = fixture.protected_release_build_command();
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut launcher = ReapedChild::new(command.spawn().expect("spawn protected release launcher"));

    let ready = fixture.target.join(PROTECTED_RELEASE_CARGO_READY);
    let deadline = Instant::now() + Duration::from_secs(120);
    while !ready.is_file() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        ready.is_file(),
        "protected Cargo did not reach its hold point"
    );
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.target.join(PROTECTED_RELEASE_CARGO_REPORT))
            .expect("read held protected release Cargo report"),
    )
    .expect("decode held protected release Cargo report");
    assert_eq!(report["parent_death_signal"], libc::SIGKILL);
    let cargo_pid = report["pid"].as_i64().expect("Cargo PID") as i32;
    let child_pid = report["parent_pid"].as_i64().expect("release child PID") as i32;
    let cargo_start = process_start_ticks_for_test(cargo_pid).expect("observe protected Cargo");
    let child_start = process_start_ticks_for_test(child_pid).expect("observe release child");
    let mut cleanup = KillProcesses(vec![cargo_pid, child_pid]);

    let launcher_pid = launcher.child_mut().id() as i32;
    // SAFETY: this is the exact live launcher PID returned by spawn.
    assert_eq!(unsafe { libc::kill(launcher_pid, libc::SIGKILL) }, 0);
    let status = launcher
        .wait_with_output()
        .expect("reap killed protected release launcher")
        .status;
    assert_eq!(status.signal(), Some(libc::SIGKILL));
    assert!(
        wait_for_process_stop(child_pid, child_start, Duration::from_secs(10)),
        "admitted release child survived launcher death"
    );
    assert!(
        wait_for_process_stop(cargo_pid, cargo_start, Duration::from_secs(10)),
        "protected Cargo survived launcher death"
    );
    cleanup.0.clear();
    assert!(
        !fixture
            .target
            .join(PROTECTED_RELEASE_CARGO_SURVIVED)
            .exists(),
        "protected Cargo continued after launcher death"
    );
    assert!(
        !fixture
            .target
            .join(".fe2o3-protected-release-rustc-report-v1")
            .exists(),
        "launcher death reached the protected rustc/backend fixture"
    );
    let published_hsaco = fs::read_dir(fixture.target.join("fe2o3"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension() == Some(OsStr::new("hsaco")));
    assert!(
        !published_hsaco,
        "launcher death published protected Cargo output"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_child_rejects_release_without_launcher_descriptors() {
    let fixture = ProjectFixture::standalone();
    let output = fixture
        .command(&[
            OsString::from("__fe2o3-authority-release-child-v1"),
            OsString::from("probe"),
        ])
        .output()
        .expect("run launcher-free release child");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("release contract descriptor"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.log.exists(), "launcher-free child executed Cargo");
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_unexpected_inherited_descriptor() {
    let fixture = ProjectFixture::standalone();
    let inherited = OpenOptions::new()
        .read(true)
        .open("/dev/null")
        .expect("open hostile inherited descriptor");
    let source = inherited.as_raw_fd();
    let mut command = fixture.protected_release_command("probe");
    // SAFETY: the retained source descriptor outlives spawn; fcntl, dup3, and close_range are
    // async-signal-safe direct system calls in the post-fork child.
    unsafe {
        command.pre_exec(move || {
            if source == 42 {
                let flags = libc::fcntl(source, libc::F_GETFD);
                if flags < 0 || libc::fcntl(source, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            } else if libc::dup3(source, 42, 0) != 42 {
                return Err(std::io::Error::last_os_error());
            }
            close_descriptor_range(3, 41)?;
            close_descriptor_range(43, u32::MAX)?;
            Ok(())
        });
    }
    let output = command.output().expect("run inherited descriptor probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("rejects unexpected inherited descriptors [42]"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.log.exists(), "descriptor rejection executed Cargo");
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_inherited_descriptor_enumeration_directory() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.protected_release_command("probe");
    // SAFETY: open, dup3, and close are async-signal-safe.
    unsafe {
        command.pre_exec(|| {
            close_descriptor_range(3, u32::MAX)?;
            let source = libc::open(
                c"/proc/self/fd".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            );
            if source < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::dup3(source, 42, 0) != 42 {
                libc::close(source);
                return Err(std::io::Error::last_os_error());
            }
            libc::close(source);
            Ok(())
        });
    }
    let output = command
        .output()
        .expect("run inherited descriptor-enumeration directory probe");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires exactly one descriptor-enumeration directory")
            && stderr.contains("42"),
        "{stderr}"
    );
    assert!(!fixture.log.exists(), "descriptor rejection executed Cargo");
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_preloads_and_selector_injection() {
    for (name, value, expected) in [
        (
            "LD_PRELOAD",
            "/definitely/not/a/fe2o3-loader-object.so",
            "rejects dynamic-loader injection variable",
        ),
        ("RUSTC", "/tmp/hostile-rustc", "compiler selection RUSTC"),
        (
            "CARGO_TARGET_GFX942_LINKER",
            "/tmp/hostile-linker",
            "rejects tool override",
        ),
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.isolated_protected_release_command("probe");
        command.env(name, value);
        let output = command.output().expect("run release injection probe");
        assert!(!output.status.success(), "{name}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.log.exists(), "{name} rejection executed Cargo");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_unexpected_inherited_environment() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.isolated_protected_release_command("probe");
    command.env("UNEXPECTED_RELEASE_STATE", "hostile");
    let output = command.output().expect("run inherited environment probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("rejects unexpected inherited environment"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !fixture.log.exists(),
        "environment rejection executed Cargo"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn protected_release_rejects_path_aliases() {
    use std::os::unix::fs::symlink;

    let path_alias = ProjectFixture::standalone();
    let alias = path_alias.root.join("cargo-alias");
    symlink(env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture"), &alias)
        .expect("create Cargo path alias");
    let mut command = path_alias.isolated_protected_release_command("probe");
    command.env("CARGO", alias);
    let output = command.output().expect("run path alias probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("rejects aliased CARGO path"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!path_alias.log.exists(), "alias rejection executed Cargo");
}

#[cfg(debug_assertions)]
#[test]
fn authority_requires_an_explicit_rustc_path_without_path_selection() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.authority_command(&[OsString::from("build")]);
    command
        .env_remove("FE2O3_AUTHORITY_RUSTC_PATH_V1")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        );
    let output = command.output().expect("run explicit rustc path probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("FE2O3_AUTHORITY_RUSTC_PATH_V1"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.log.exists(), "rustc path rejection executed Cargo");
}

#[cfg(debug_assertions)]
#[test]
fn authority_metadata_is_frozen_offline_and_has_no_host_helper_environment() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("authority-preflight-report");
    let mut command = fixture.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        )
        .env("FE2O3_TEST_AUTHORITY_PREFLIGHT_REPORT", &report)
        .env("HOME", "/tmp/attacker-home")
        .env("CARGO_HOME", "/tmp/attacker-cargo-home")
        .env("GIT_CONFIG_GLOBAL", "/tmp/attacker-git-config")
        .env("SSH_AUTH_SOCK", "/tmp/attacker-agent");
    let output = command
        .output()
        .expect("run authority preflight environment probe");
    assert!(!output.status.success());
    let report = fs::read_to_string(report).expect("read authority preflight report");
    for required in [
        "frozen=true",
        "offline=true",
        "rustc=/proc/self/fd/194",
        "PATH=None",
        "HOME=None",
        "CARGO_HOME=None",
        "GIT_CONFIG_GLOBAL=None",
        "SSH_AUTH_SOCK=None",
        "CARGO_REGISTRIES_CRATES_IO_TOKEN=None",
        "rustc_fd=true",
        "lib_tree_fd=true",
    ] {
        assert!(
            report.contains(required),
            "missing {required:?} in {report:?}"
        );
    }
}

#[cfg(debug_assertions)]
#[test]
fn authority_rejects_credential_and_registry_helper_channels_before_cargo() {
    for name in [
        "CARGO_REGISTRIES_CRATES_IO_TOKEN",
        "CARGO_CREDENTIAL_ALIAS_ATTACKER",
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env(name, "attacker-helper");
        let output = command.output().expect("run Cargo helper-channel probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("rejects pre-admission helper/configuration channel"),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.log.exists(), "helper rejection executed Cargo");
    }
}

#[cfg(debug_assertions)]
#[test]
fn authority_build_requires_independent_runtime_and_cargo_digests() {
    let missing_runtime = ProjectFixture::standalone();
    let mut command = missing_runtime.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&missing_runtime.root),
        );
    let output = command.output().expect("run missing runtime pin probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let missing_cargo = ProjectFixture::standalone();
    let mut command = missing_cargo.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&missing_cargo.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&missing_cargo.root),
        );
    let output = command.output().expect("run missing Cargo pin probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires FE2O3_AUTHORITY_CARGO_SHA256_V1"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (name, value, expected) in [
        (
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            "01".repeat(32),
            "authority rustc toolchain lib tree does not match",
        ),
        (
            "FE2O3_AUTHORITY_CARGO_SHA256_V1",
            "01".repeat(32),
            "authority Cargo does not match",
        ),
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env(
                "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
                authority_rustc_sha256(&fixture.root),
            )
            .env(
                "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
                authority_rustc_runtime_sha256(&fixture.root),
            )
            .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
            .env(
                "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
                file_sha256(&fixture.backend),
            )
            .env(name, value);
        let output = command.output().expect("run wrong closure pin probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(debug_assertions)]
#[test]
fn authority_build_requires_an_independent_exact_backend_digest() {
    let missing = ProjectFixture::standalone();
    let mut command = missing.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&missing.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&missing.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256());
    let output = command.output().expect("run missing backend pin probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires FE2O3_AUTHORITY_BACKEND_SHA256_V1"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let missing_backend = ProjectFixture::standalone();
    let mut command = missing_backend.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&missing_backend.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&missing_backend.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&missing_backend.backend),
        )
        .env_remove("FE2O3_BACKEND");
    let output = command
        .output()
        .expect("run missing prebuilt backend probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires FE2O3_BACKEND to name an explicit prebuilt codegen backend"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !missing_backend.log.exists(),
        "backend admission executed Cargo"
    );
    assert!(
        !missing_backend
            .target
            .join(".fe2o3-backend-build-v1")
            .exists()
    );

    let wrong = ProjectFixture::standalone();
    let substituted_backend = wrong.root.join("substituted-codegen-backend.so");
    fs::write(&substituted_backend, b"substituted backend").expect("write backend substitute");
    let mut command = wrong.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&wrong.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&wrong.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&wrong.backend),
        )
        .env("FE2O3_BACKEND", substituted_backend);
    let output = command.output().expect("run wrong backend pin probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("authority backend does not match"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!wrong.log.exists(), "backend substitution executed Cargo");
    assert!(!wrong.target.join("fe2o3").exists());
}

#[cfg(debug_assertions)]
#[test]
fn authority_build_rejects_rustup_selection_substitution() {
    for (variable, value) in [
        ("RUSTUP_TOOLCHAIN", "attacker-toolchain"),
        ("RUSTUP_HOME", "/tmp/attacker-rustup-home"),
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env(
                "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
                authority_rustc_sha256(&fixture.root),
            )
            .env(variable, value);
        let output = command.output().expect("run rustup substitution probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("rejects rustup selection channel"),
            "{variable}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join(".fe2o3-backend-build-v1").exists());
    }
}

#[cfg(unix)]
#[cfg(debug_assertions)]
#[test]
fn authority_build_never_executes_a_rustup_proxy_during_rustc_resolution() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let fixture = ProjectFixture::standalone();
    let hostile = fixture.root.join("hostile-rustup");
    let bin = hostile.join("bin");
    fs::create_dir_all(&bin).expect("create hostile rustup directory");
    let marker = hostile.join("executed");
    let rustup = hostile.join("rustup");
    fs::write(
        &rustup,
        format!(
            "#!/bin/sh\nprintf executed > {}\nexit 0\n",
            marker.display()
        ),
    )
    .expect("write hostile rustup proxy");
    fs::set_permissions(&rustup, fs::Permissions::from_mode(0o700))
        .expect("make hostile rustup executable");
    symlink(&rustup, bin.join("rustc")).expect("install hostile rustc proxy");

    let mut paths = vec![bin.clone()];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    let mut command = fixture.authority_command(&[OsString::from("build")]);
    command
        .env(
            "PATH",
            env::join_paths(paths).expect("construct hostile rustup PATH"),
        )
        .env("FE2O3_AUTHORITY_RUSTC_PATH_V1", bin.join("rustc"))
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        );
    let output = command.output().expect("run hostile rustup proxy probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("resolves to a rustup proxy"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "authority discovery executed rustup");
}

#[cfg(unix)]
#[cfg(debug_assertions)]
#[test]
fn authority_build_rejects_unpinned_cargo_before_executing_it() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = ProjectFixture::standalone();
    let marker = fixture.root.join("hostile-cargo-executed");
    let hostile = fixture.root.join("hostile-cargo");
    fs::write(
        &hostile,
        format!(
            "#!/bin/sh\nprintf executed > {}\nexit 0\n",
            marker.display()
        ),
    )
    .expect("write hostile Cargo executable");
    fs::set_permissions(&hostile, fs::Permissions::from_mode(0o700))
        .expect("make hostile Cargo executable");
    let mut command = fixture.authority_command(&[OsString::from("build")]);
    command
        .env("CARGO", &hostile)
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        );
    let output = command.output().expect("run hostile Cargo probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("authority Cargo does not match"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !marker.exists(),
        "authority discovery executed unpinned Cargo"
    );
}

#[cfg(debug_assertions)]
#[test]
fn authority_build_rejects_linker_and_runner_environment() {
    for variable in [
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env(variable, "attacker-selected-tool");
        let output = command.output().expect("run authority override probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("authority build rejects tool override"),
            "{variable}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join("fe2o3").exists());
    }
}

#[cfg(debug_assertions)]
#[test]
fn authority_build_rejects_configured_linker_and_runner() {
    for (variable, value, diagnostic) in [
        (
            "FE2O3_TEST_TARGET_TABLE_JSON",
            r#"{"x86_64-unknown-linux-gnu":{"linker":"/tmp/attacker"}}"#,
            "configured target.x86_64-unknown-linux-gnu.linker",
        ),
        (
            "FE2O3_TEST_TARGET_TABLE_JSON",
            r#"{"cfg(unix)":{"runner":["/tmp/attacker"]}}"#,
            "configured target.cfg(unix).runner",
        ),
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.authority_command(&[OsString::from("build")]);
        command
            .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
            .env(
                "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
                authority_rustc_sha256(&fixture.root),
            )
            .env(
                "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
                authority_rustc_runtime_sha256(&fixture.root),
            )
            .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
            .env(
                "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
                file_sha256(&fixture.backend),
            )
            .env(variable, value);
        let output = command
            .output()
            .expect("run configured authority override probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{diagnostic}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.target.join("fe2o3").exists());
    }
}

#[cfg(unix)]
#[test]
fn cargo_cannot_substitute_the_parent_pinned_rustc_before_artifact_authority() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = ProjectFixture::standalone();
    let marker = fixture.root.join("attacker-rustc-executed");
    let attacker = fixture.root.join("attacker-rustc");
    fs::write(
        &attacker,
        format!("#!/bin/sh\nprintf reached > '{}'\n", marker.display()),
    )
    .expect("write attacker rustc");
    fs::set_permissions(&attacker, fs::Permissions::from_mode(0o700))
        .expect("make attacker rustc executable");
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("FE2O3_TEST_SUBSTITUTE_RUSTC", &attacker);

    let output = command.output().expect("run rustc substitution probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("does not match the parent-pinned compiler"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "attacker-selected rustc executed");
    assert!(
        !fixture.target.join("fe2o3").exists(),
        "failed rustc substitution committed an artifact generation"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cargo_cannot_substitute_inherited_rustc_lib_tree_fd_193() {
    let fixture = ProjectFixture::standalone();
    let substitute = fixture.root.join("attacker-lib-tree");
    fs::create_dir(&substitute).expect("create attacker lib-tree directory");
    fs::write(substitute.join("rustc_driver.so"), b"attacker runtime")
        .expect("write attacker runtime object");
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("PATH", rustc_fixture_path(&fixture.root))
        .env("FE2O3_TEST_SUBSTITUTE_RUSTC_LIB_TREE", &substitute);
    let output = command.output().expect("run fd 193 substitution probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("does not match the broker-authenticated retained object"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.target.join("fe2o3").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn cargo_cannot_inject_compiler_closure_into_ordinary_root_rustc() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("compiler-closure-rustc-report");
    let capability_report = fixture.root.join("compiler-closure-capability-report");
    let real_rustc = resolved_real_rustc();
    let mut command = fixture.command(&[OsString::from("build")]);
    command
        .env("PATH", rustc_fixture_path(&fixture.root))
        .env("FE2O3_TEST_REAL_RUSTC", real_rustc)
        .env("FE2O3_TEST_RUSTC_CAPABILITY_REPORT", capability_report)
        .env("FE2O3_TEST_COMPILER_CLOSURE_RUSTC_REPORT", &report);
    let output = command
        .output()
        .expect("run compiler closure substitution probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(report).expect("read rustc compiler closure report"),
        "absent"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cargo_cannot_replace_the_inherited_rustc_descriptor() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = ProjectFixture::standalone();
    let marker = fixture.root.join("descriptor-attacker-executed");
    let attacker = fixture.root.join("descriptor-attacker-rustc");
    fs::write(
        &attacker,
        format!("#!/bin/sh\nprintf reached > '{}'\n", marker.display()),
    )
    .expect("write descriptor attacker rustc");
    fs::set_permissions(&attacker, fs::Permissions::from_mode(0o700))
        .expect("make descriptor attacker rustc executable");
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("FE2O3_TEST_SUBSTITUTE_RUSTC_DESCRIPTOR", &attacker);

    let output = command
        .output()
        .expect("run rustc descriptor substitution probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("failed to pin rustc executable"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "descriptor-substituted rustc executed");
    assert!(!fixture.target.join("fe2o3").exists());
}

#[test]
fn pinned_real_rustc_image_executes_without_loader_environment() {
    let fixture = ProjectFixture::standalone();
    let report = fixture.root.join("pinned-rustc-version");
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("FE2O3_TEST_PINNED_RUSTC_REPORT", &report);

    let output = command.output().expect("run pinned real rustc probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fs::read(report)
            .expect("read pinned rustc version")
            .starts_with(b"rustc ")
    );
}

#[cfg(unix)]
#[test]
fn rustc_image_digest_changes_generation_identity() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = ProjectFixture::standalone();
    let mut managed_arguments = Vec::new();
    for (name, body) in [
        ("first", "#!/bin/sh\nexit 0\n"),
        ("second", "#!/bin/sh\n# distinct exact image\nexit 0\n"),
    ] {
        let directory = fixture.root.join(format!("rustc-{name}"));
        fs::create_dir(&directory).expect("create rustc image directory");
        let rustc = directory.join("rustc");
        fs::write(&rustc, body).expect("write rustc image");
        fs::set_permissions(&rustc, fs::Permissions::from_mode(0o700))
            .expect("make rustc image executable");
        let mut paths = vec![directory];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
        let mut command = fixture.command(&[OsString::from("build")]);
        command.env(
            "PATH",
            env::join_paths(paths).expect("construct rustc image PATH"),
        );
        let output = command.output().expect("run rustc identity build");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        managed_arguments.push(
            fixture
                .invocations()
                .last()
                .expect("record rustc identity build")
                .managed_rustc_args
                .clone(),
        );
    }
    assert_ne!(managed_arguments[0], managed_arguments[1]);
}

#[cfg(target_os = "linux")]
#[test]
fn cargo_child_loader_injection_fails_before_artifact_authority() {
    for variable in [
        "LD_PRELOAD",
        "LD_AUDIT",
        "LD_DEBUG",
        "DYLD_INSERT_LIBRARIES",
        "GLIBC_TUNABLES",
    ] {
        let fixture = ProjectFixture::standalone();
        let mut command = fixture.command(&[OsString::from("build")]);
        command.env("FE2O3_TEST_WRAPPER_LOADER_NAME", variable).env(
            "FE2O3_TEST_WRAPPER_LOADER_VALUE",
            "/definitely/not/a/fe2o3-loader-object.so",
        );

        let output = command.output().expect("run child loader injection probe");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("binding wrapper rejects dynamic-loader injection variable"),
            "{variable}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !fixture.target.join("fe2o3").exists(),
            "loader-injected wrapper committed an artifact generation"
        );
    }
}

#[cfg(unix)]
#[test]
fn run_forwards_non_utf8_application_argv_losslessly() {
    use std::os::unix::ffi::OsStringExt;

    let fixture = ProjectFixture::standalone();
    let non_utf8 = OsString::from_vec(b"argument-\xff".to_vec());
    let args = [
        OsString::from("run"),
        OsString::from("--"),
        non_utf8.clone(),
    ];

    let output = fixture.run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records = fixture.invocations();
    assert_eq!(
        without_injected_runner(&records[1].args[1..]),
        strings(&["run", "--"])
            .into_iter()
            .chain([b"argument-\xff".to_vec()])
            .collect::<Vec<_>>()
    );
}

#[test]
fn successful_generation_is_reused_without_deleting_host_outputs() {
    let fixture = ProjectFixture::standalone();
    let unrelated = fixture.target.join("debug/host-output");
    fs::create_dir_all(unrelated.parent().expect("host output parent"))
        .expect("create host output directory");
    fs::write(&unrelated, b"keep").expect("write host output");

    for _ in 0..2 {
        let output = fixture.run(&[OsString::from("build")]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let records = fixture.invocations();
    assert_eq!(records.len(), 4, "{records:#?}");
    assert_eq!(records[1].managed_rustc_args, records[3].managed_rustc_args);
    assert_eq!(fs::read(unrelated).expect("read host output"), b"keep");
    assert!(
        fixture
            .target
            .join("fe2o3/.codegen-generation-v1")
            .is_file()
    );
}

#[test]
#[cfg(unix)]
fn cleaning_fe2o3_created_external_target_preserves_parent_and_host_outputs() {
    let mut fixture = ProjectFixture::standalone();
    fixture.target = fixture.root.join("dedicated-target");
    let target = fixture.target.clone();
    let target_argument = target.as_os_str().to_os_string();
    let build = fixture.run(&[
        OsString::from("build"),
        OsString::from("--target-dir"),
        target_argument.clone(),
    ]);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(!target.join(".fe2o3-target-root-owned-v1").exists());
    fs::create_dir_all(target.join("debug")).expect("create host output directory");
    fs::write(target.join("debug/host"), b"host").expect("write host output");

    let clean = fixture.run(&[
        OsString::from("clean"),
        OsString::from("--target-dir"),
        target_argument,
    ]);
    assert!(
        clean.status.success(),
        "{}",
        String::from_utf8_lossy(&clean.stderr)
    );
    assert!(target.is_dir());
    assert_eq!(fs::read(target.join("debug/host")).unwrap(), b"host");
    assert!(!target.join("fe2o3").exists());
}

#[test]
#[cfg(unix)]
fn preexisting_external_target_never_gains_parent_deletion_authority() {
    let mut fixture = ProjectFixture::standalone();
    fixture.target = fixture.root.join("preexisting-target");
    let target = fixture.target.clone();
    fs::create_dir(&target).expect("create preexisting target");
    fs::write(target.join("keep"), b"keep").expect("write preexisting sentinel");
    let target_argument = target.as_os_str().to_os_string();

    let build = fixture.run(&[
        OsString::from("build"),
        OsString::from("--target-dir"),
        target_argument.clone(),
    ]);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(!target.join(".fe2o3-target-root-owned-v1").exists());
    let mut forged = b"fe2o3-target-root-owned-v1\0".to_vec();
    forged.extend_from_slice(&[0x5a; 16]);
    fs::write(target.join(".fe2o3-target-root-owned-v1"), forged)
        .expect("write forged stale root guard");

    let clean = fixture.run(&[
        OsString::from("clean"),
        OsString::from("--target-dir"),
        target_argument,
    ]);
    assert!(
        clean.status.success(),
        "{}",
        String::from_utf8_lossy(&clean.stderr)
    );
    assert!(target.is_dir());
    assert_eq!(
        fs::read(target.join("keep")).expect("read sentinel"),
        b"keep"
    );
    assert!(target.join(".fe2o3-target-root-owned-v1").is_file());
    assert!(!target.join("fe2o3").exists());
}

#[test]
fn missing_generated_sidecar_allocates_a_new_cargo_generation() {
    let fixture = ProjectFixture::standalone();
    let first = fixture.run(&[OsString::from("build")]);
    assert!(first.status.success());
    fs::remove_file(fixture.target.join("fe2o3/fixture.hsaco")).expect("remove generated sidecar");

    let second = fixture.run(&[OsString::from("build")]);
    assert!(second.status.success());

    let records = fixture.invocations();
    assert_ne!(records[1].managed_rustc_args, records[3].managed_rustc_args);
    assert!(fixture.target.join("fe2o3/fixture.hsaco").is_file());
}

#[test]
fn unowned_interrupted_generation_fails_closed_and_is_preserved() {
    let fixture = ProjectFixture::standalone();
    fs::create_dir_all(fixture.target.join("fe2o3")).expect("create interrupted generation");
    fs::write(fixture.target.join("fe2o3/stale"), b"stale").expect("write interrupted output");

    let output = fixture.run(&[OsString::from("build")]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unowned"));
    assert_eq!(
        fs::read(fixture.target.join("fe2o3/stale")).expect("stale output remains"),
        b"stale"
    );
}

#[test]
fn failed_owned_generation_is_cleaned_before_retry() {
    let fixture = ProjectFixture::standalone();
    let failure_marker = fixture.root.join("fail-once");
    let mut first = fixture.command(&[OsString::from("build")]);
    first.env("FE2O3_TEST_FAIL_ONCE", &failure_marker);

    let output = first.output().expect("run failing generation");
    assert!(!output.status.success());
    assert!(!fixture.target.join("fe2o3").exists());

    let mut retry = fixture.command(&[OsString::from("build")]);
    retry.env("FE2O3_TEST_FAIL_ONCE", &failure_marker);
    let output = retry.output().expect("retry generation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.target.join("fe2o3/fixture.hsaco").is_file());
}

#[cfg(target_os = "linux")]
#[cfg(debug_assertions)]
#[test]
fn cargo_failure_aggregates_runtime_and_authority_closure_revalidation() {
    let fixture = ProjectFixture::standalone();
    fs::write(fixture.workspace.join("Cargo.lock"), "version = 4\n")
        .expect("write authority lockfile");
    let runtime = fixture
        .root
        .join("pinned-rustc-fixture-toolchain/lib/runtime-marker");
    let source = fixture.workspace.join("src/main.rs");
    let mut command = fixture.authority_command(&[OsString::from("build")]);
    command
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "collected-row-softmax-v1")
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            authority_rustc_sha256(&fixture.root),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            authority_rustc_runtime_sha256(&fixture.root),
        )
        .env("FE2O3_AUTHORITY_CARGO_SHA256_V1", authority_cargo_sha256())
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            file_sha256(&fixture.backend),
        )
        .env("FE2O3_TEST_AUTHORITY_METADATA_V1", "1")
        .env("FE2O3_TEST_MUTATE_RUSTC_RUNTIME_V1", runtime)
        .env("FE2O3_TEST_MUTATE_AUTHORITY_SOURCE_V1", source);

    let output = command.output().expect("run post-spawn aggregation probe");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let cargo = stderr
        .find("cargo fe2o3 device phase (build) failed with status exit status: 23")
        .unwrap_or_else(|| panic!("missing Cargo primary failure in {stderr}"));
    let runtime = stderr
        .find("rustc runtime-tree revalidation also failed")
        .unwrap_or_else(|| panic!("missing runtime revalidation failure in {stderr}"));
    let closure = stderr
        .find("authorized kernel-closure revalidation also failed")
        .unwrap_or_else(|| panic!("missing closure revalidation failure in {stderr}"));
    assert!(cargo < runtime && runtime < closure, "{stderr}");
    assert!(!fixture.target.join("fe2o3").exists());
}

#[cfg(unix)]
#[test]
fn artifact_path_substitution_is_rejected_without_redirecting_writes() {
    let fixture = ProjectFixture::standalone();
    let artifact = fixture.target.join("fe2o3");
    let relocated = artifact.with_extension("relocated");
    let outside = artifact.with_extension("outside");
    let mut command = fixture.command(&[OsString::from("build")]);
    command.env("FE2O3_TEST_SUBSTITUTE_ARTIFACT", &artifact);

    let output = command.output().expect("run substitution fixture");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("path was substituted") || stderr.contains("refusing directory path"),
        "{stderr}"
    );
    assert!(!relocated.exists(), "pending opened output must be cleaned");
    assert_eq!(
        fs::read(outside.join("keep")).expect("outside sentinel"),
        b"outside"
    );
    assert!(!outside.join("fixture.hsaco").exists());
}

#[cfg(unix)]
#[test]
fn concurrent_generations_are_serialized_by_the_target_lock() {
    let fixture = ProjectFixture::standalone();
    let active = fixture.root.join("active-generation");
    let mut first = fixture.command(&[OsString::from("build")]);
    first
        .env("FE2O3_TEST_EXCLUSIVE_ACTIVE", &active)
        .env("FE2O3_TEST_GENERATION_CONTROL", "stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut first = ReapedChild::new(first.spawn().expect("spawn first generation"));

    let mut ready = [0_u8; 5];
    first
        .child_mut()
        .stdout
        .as_mut()
        .expect("first generation stdout")
        .read_exact(&mut ready)
        .expect("read generation readiness");
    assert_eq!(ready, *b"ready");
    assert!(active.is_file(), "first generation did not become active");

    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.target.join(".fe2o3-generation.lock-v1"))
        .expect("open generation lock");
    assert_eq!(
        flock(&lock, FlockOperation::NonBlockingLockExclusive),
        Err(Errno::WOULDBLOCK),
        "generation worker did not retain the target lock"
    );

    let mut second = fixture.command(&[OsString::from("build")]);
    second
        .env("FE2O3_TEST_EXCLUSIVE_ACTIVE", &active)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let second = ReapedChild::new(second.spawn().expect("spawn second generation"));
    first
        .child_mut()
        .stdin
        .take()
        .expect("first generation stdin")
        .write_all(b"release")
        .expect("release first generation");

    let first = first.wait_with_output().expect("run first generation");
    let second = second.wait_with_output().expect("run second generation");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(!active.exists());
}

#[test]
fn successful_incremental_build_republishes_the_generation_snapshot() {
    let fixture = ProjectFixture::standalone();
    let counter = fixture.root.join("mutation-counter");
    for _ in 0..3 {
        let mut command = fixture.command(&[OsString::from("build")]);
        command.env("FE2O3_TEST_MUTATING_COUNTER", &counter);
        let output = command.output().expect("run mutating generation");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let records = fixture.invocations();
    let managed_rustc_args = records
        .iter()
        .filter(|record| record.args.get(1) == Some(&b"build".to_vec()))
        .map(|record| record.managed_rustc_args.clone())
        .collect::<Vec<_>>();
    assert_eq!(managed_rustc_args.len(), 3);
    assert_eq!(managed_rustc_args[0], managed_rustc_args[1]);
    assert_eq!(managed_rustc_args[1], managed_rustc_args[2]);
    assert_eq!(
        fs::read(fixture.target.join("fe2o3/fixture.hsaco")).expect("read sidecar"),
        b"fixture-sidecar-3"
    );
}

#[cfg(unix)]
#[test]
fn application_runner_scrubs_build_environment_and_preserves_non_utf8_argv() {
    use std::os::unix::ffi::OsStringExt;

    let root = temp_root();
    let report = root.join("runner-report.json");
    let payload = OsString::from_vec(b"application-\xff".to_vec());
    let mut command = cargo_fe2o3_command();
    command
        .args(internal_runner_args(
            &root,
            Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture")),
            &report,
            payload,
        ))
        .env("FE2O3_PRIVATE_BUILD_CAPABILITY", "must-not-leak")
        .env("FE2O3_BINDING_WRAPPER_MODE_V1", "1")
        .env("CARGO_ENCODED_RUSTFLAGS", "must-not-leak")
        .env("RUSTC_WORKSPACE_WRAPPER", "must-not-leak")
        .env(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
            "must-not-leak",
        );

    let output = command.output().expect("run isolated application fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read runner report"))
            .expect("decode runner report");
    assert_eq!(report["artifact_fd_open"], true);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["leaked_environment"], serde_json::json!([]));
    assert_eq!(report["inherited_fds"].as_array().unwrap().len(), 1);
    assert_eq!(report["inherited_fds"][0]["fd"], 197);
    assert_eq!(
        report["inherited_fds"][0]["target"],
        root.join("runner-artifact").to_str().unwrap()
    );
    assert_eq!(report["runtime_artifact_directory"], "/proc/self/fd/197");
    assert_eq!(report["slot_unlocks"], serde_json::json!([]));
    assert_eq!(report["payload_hex"], "6170706c69636174696f6e2dff");
    fs::remove_dir_all(root).expect("remove runner fixture");
}

#[cfg(unix)]
#[test]
fn hostile_orphan_descendants_cannot_retain_supervisor_slots() {
    struct KillOrphans(Vec<i32>);
    impl Drop for KillOrphans {
        fn drop(&mut self) {
            for child in self.0.drain(..) {
                // SAFETY: these are exact PIDs written by the hostile fork fixture.
                let _ = unsafe { libc::kill(child, libc::SIGKILL) };
            }
        }
    }

    for repetition in 0..2 {
        let mut roots = Vec::new();
        let mut orphans = KillOrphans(Vec::new());
        for launch in 0..32 {
            let root = temp_root();
            let report = root.join("report.json");
            let marker = root.join("holder.pid");
            let mut arguments = internal_runner_args(
                &root,
                Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture")),
                &report,
                OsString::from("probe"),
            );
            arguments.push(OsString::from("--fe2o3-test-fork-fd-holder"));
            arguments.push(marker.as_os_str().to_owned());
            let output = cargo_fe2o3_command()
                .args(arguments)
                .output()
                .expect("launch hostile no-envelope application");
            assert!(
                output.status.success(),
                "repetition {repetition}, launch {launch}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report: serde_json::Value =
                serde_json::from_slice(&fs::read(&report).expect("read holder report"))
                    .expect("decode holder report");
            assert_only_runtime_artifact_descriptor(&report, &root);
            let child = fs::read_to_string(&marker)
                .expect("read holder PID")
                .parse::<i32>()
                .expect("parse holder PID");
            // SAFETY: signal zero only checks the exact holder PID remains concurrent.
            assert_eq!(unsafe { libc::kill(child, 0) }, 0);
            orphans.0.push(child);
            roots.push(root);
        }

        let root = temp_root();
        let report = root.join("report.json");
        let thirty_third = cargo_fe2o3_command()
            .args(internal_runner_args(
                &root,
                Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture")),
                &report,
                OsString::from("thirty-third"),
            ))
            .output()
            .expect("launch application after 32 concurrent orphans");
        assert!(
            thirty_third.status.success(),
            "orphan descendants saturated launch 33: {}",
            String::from_utf8_lossy(&thirty_third.stderr)
        );
        roots.push(root);
        drop(orphans);
        for root in roots {
            fs::remove_dir_all(root).expect("remove hostile holder fixture");
        }
    }
}

#[cfg(unix)]
#[test]
fn hostile_application_cannot_forge_pending_supervisor_success() {
    use std::time::{Duration, Instant};

    let root = temp_root();
    let report = root.join("report.json");
    let marker = root.join("application.pid");
    let mut arguments = internal_runner_args(
        &root,
        Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture")),
        &report,
        OsString::from("probe"),
    );
    arguments.push(OsString::from("--fe2o3-test-forge-supervisor-result"));
    arguments.push(marker.as_os_str().to_owned());
    let started = Instant::now();
    let output = cargo_fe2o3_command()
        .args(arguments)
        .output()
        .expect("launch supervisor forgery probe");
    let elapsed = started.elapsed();
    assert!(
        output.status.success(),
        "secure forgery probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        elapsed >= Duration::from_millis(1_500) && elapsed < Duration::from_secs(10),
        "frontend accepted a forged pending result or exceeded its broad bound: {elapsed:?}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read forgery report"))
            .expect("decode forgery report");
    assert_eq!(report["forged_supervisor_result"], false);
    assert_only_runtime_artifact_descriptor(&report, &root);
    let application = fs::read_to_string(&marker)
        .expect("read application PID")
        .parse::<i32>()
        .expect("parse application PID");
    // SAFETY: the completed supervisor must already have reaped this exact application PID.
    assert_eq!(unsafe { libc::kill(application, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    fs::remove_dir_all(root).expect("remove forgery fixture");
}

#[cfg(unix)]
#[test]
fn hostile_application_cannot_unlock_supervisor_admission() {
    let root = temp_root();
    let report = root.join("report.json");
    let mut arguments = internal_runner_args(
        &root,
        Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture")),
        &report,
        OsString::from("probe"),
    );
    arguments.push(OsString::from("--fe2o3-test-unlock-supervisor-slot"));
    let output = cargo_fe2o3_command()
        .args(arguments)
        .output()
        .expect("launch slot unlock probe");
    assert!(
        output.status.success(),
        "slot unlock probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read slot report"))
            .expect("decode slot report");
    assert_only_runtime_artifact_descriptor(&report, &root);
    assert_eq!(report["slot_unlocks"], serde_json::json!([]));
    fs::remove_dir_all(root).expect("remove slot fixture");
}

#[cfg(unix)]
#[test]
fn hidden_supervisor_rejects_malformed_descriptors_without_abort() {
    use std::os::fd::IntoRawFd;
    use std::time::{Duration, Instant};

    fn invoke(channel: i32, slot: i32, inherited: &[i32]) -> Output {
        let mut command = cargo_fe2o3_command();
        command.args([
            OsString::from("__fe2o3-application-supervisor-v1"),
            OsString::from(channel.to_string()),
            OsString::from(slot.to_string()),
            OsString::from("00".repeat(32)),
            OsString::from("runner"),
        ]);
        let inherited = inherited.to_vec();
        // SAFETY: the callback changes only FD_CLOEXEC on test-owned descriptors so the hidden
        // CLI receives the exact malformed descriptor configuration under review.
        unsafe {
            command.pre_exec(move || {
                for descriptor in &inherited {
                    let flags = libc::fcntl(*descriptor, libc::F_GETFD);
                    if flags < 0
                        || libc::fcntl(*descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC) != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        command
            .output()
            .expect("invoke malformed hidden supervisor")
    }

    fn reject(label: &str, started: Instant, output: Output) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{label} exceeded bounded rejection"
        );
        assert!(!output.status.success(), "{label} unexpectedly succeeded");
        assert!(output.status.code().is_some(), "{label} aborted by signal");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cargo-fe2o3 application supervisor:"),
            "{label} did not return a normal diagnostic: {stderr}"
        );
        assert!(!stderr.contains("fatal runtime error"), "{label}: {stderr}");
    }

    for (label, channel, slot) in [
        ("exact absent descriptors", 999_999, 999_998),
        ("negative descriptors", -1, -2),
        ("stdio descriptors", 1, 2),
    ] {
        let started = Instant::now();
        reject(label, started, invoke(channel, slot, &[]));
    }

    let root = temp_root();
    let regular_path = root.join("regular");
    let regular = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&regular_path)
        .expect("create malformed regular descriptor");
    let directory = fs::File::open(&root).expect("open malformed directory descriptor");
    let (stream, stream_peer) = UnixStream::pair().expect("create hidden CLI stream");
    let (datagram, datagram_peer) = UnixDatagram::pair().expect("create hidden CLI datagram");

    let started = Instant::now();
    reject(
        "aliased numeric descriptor",
        started,
        invoke(
            stream.as_raw_fd(),
            stream.as_raw_fd(),
            &[stream.as_raw_fd()],
        ),
    );
    let stream_alias = unsafe { libc::dup(stream.as_raw_fd()) };
    assert!(stream_alias >= 0);
    let started = Instant::now();
    reject(
        "aliased socket descriptions",
        started,
        invoke(
            stream.as_raw_fd(),
            stream_alias,
            &[stream.as_raw_fd(), stream_alias],
        ),
    );
    // SAFETY: dup returned this test-owned descriptor and no Rust owner wraps it.
    unsafe { libc::close(stream_alias) };

    for (label, channel, slot, inherited) in [
        (
            "regular channel",
            regular.as_raw_fd(),
            directory.as_raw_fd(),
            vec![regular.as_raw_fd(), directory.as_raw_fd()],
        ),
        (
            "directory channel",
            directory.as_raw_fd(),
            regular.as_raw_fd(),
            vec![directory.as_raw_fd(), regular.as_raw_fd()],
        ),
        (
            "datagram channel",
            datagram.as_raw_fd(),
            regular.as_raw_fd(),
            vec![datagram.as_raw_fd(), regular.as_raw_fd()],
        ),
        (
            "noncanonical regular slot",
            stream.as_raw_fd(),
            regular.as_raw_fd(),
            vec![stream.as_raw_fd(), regular.as_raw_fd()],
        ),
    ] {
        let started = Instant::now();
        reject(label, started, invoke(channel, slot, &inherited));
    }

    let reused_number = {
        let file = OpenOptions::new()
            .read(true)
            .open(&regular_path)
            .expect("open descriptor for reuse probe");
        file.into_raw_fd()
    };
    // SAFETY: into_raw_fd transferred ownership of this exact descriptor to the test.
    unsafe { libc::close(reused_number) };
    let started = Instant::now();
    reject(
        "closed descriptor reuse",
        started,
        invoke(reused_number, 999_998, &[]),
    );

    drop((stream_peer, datagram_peer));
    fs::remove_dir_all(root).expect("remove malformed descriptor fixture");
}

#[cfg(unix)]
fn internal_runner_args(
    root: &Path,
    application: &Path,
    report: &Path,
    payload: OsString,
) -> Vec<OsString> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let artifact = root.join("runner-artifact");
    fs::create_dir(&artifact).expect("create runner artifact directory");
    fs::set_permissions(&artifact, fs::Permissions::from_mode(0o700))
        .expect("make runner artifact directory private");
    let owner = artifact.join(".fe2o3-owned-v1");
    let mut owner_bytes = b"fe2o3-owned-v1\0".to_vec();
    owner_bytes.extend_from_slice(&[1; 16]);
    fs::write(&owner, owner_bytes).expect("write runner artifact owner record");
    fs::set_permissions(&owner, fs::Permissions::from_mode(0o600))
        .expect("make runner owner record private");
    let metadata = fs::metadata(&artifact).expect("inspect runner artifact directory");
    vec![
        OsString::from("__fe2o3-runner-v1"),
        OsString::from("3"),
        OsString::from(hex(os_bytes(artifact.as_os_str()))),
        OsString::from(metadata.dev().to_string()),
        OsString::from(metadata.ino().to_string()),
        OsString::from("none"),
        OsString::from("0"),
        application.as_os_str().to_os_string(),
        report.as_os_str().to_os_string(),
        payload,
    ]
}

#[cfg(unix)]
fn assert_only_runtime_artifact_descriptor(report: &serde_json::Value, root: &Path) {
    let inherited = report["inherited_fds"]
        .as_array()
        .expect("inherited descriptor report is an array");
    assert_eq!(inherited.len(), 1, "{report}");
    assert_eq!(inherited[0]["fd"], ARTIFACT_CHILD_FD, "{report}");
    assert_eq!(
        inherited[0]["target"],
        root.join("runner-artifact").to_str().unwrap(),
        "{report}"
    );
}

#[cfg(unix)]
#[test]
fn configured_multi_argument_runner_is_chained_after_empty_environment_reset() {
    let fixture = ProjectFixture::standalone();
    let runner_report = fixture.root.join("qemu-runner.json");
    let application_report = fixture.root.join("qemu-application.json");
    let original_runner = serde_json::json!([
        env!("CARGO_BIN_EXE_cargo-fe2o3-runner-chain-fixture"),
        "--mode",
        "qemu",
        "--cpu",
        "max",
        "--report",
        runner_report,
        "--runner-end"
    ]);
    let mut command = fixture.command(&[
        OsString::from("run"),
        OsString::from("--target"),
        OsString::from("x86_64-unknown-linux-gnu"),
    ]);
    command
        .env("FE2O3_TEST_CONFIG_RUNNER_JSON", original_runner.to_string())
        .env(
            "FE2O3_TEST_RUN_APPLICATION",
            env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture"),
        )
        .env("FE2O3_TEST_RUN_APPLICATION_REPORT", &application_report)
        .env("FE2O3_TEST_RUN_APPLICATION_PAYLOAD", "qemu-payload")
        .env("FE2O3_PRIVATE_BUILD_CAPABILITY", "must-not-leak")
        .env("RUNNER_CHAIN_ENV", "preserved");

    let output = command.output().expect("run configured runner chain");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_runner_chain_report(&runner_report, "qemu", None);
    assert_application_report(&application_report, "71656d752d7061796c6f6164", None);
}

#[cfg(unix)]
#[test]
fn non_utf8_runner_arguments_survive_empty_environment_reset() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let fixture = ProjectFixture::standalone();
    let runner_report = fixture.root.join("ssh-runner.json");
    let application_report = fixture.root.join("ssh-application.json");
    let runner_parts = [
        os_bytes(OsStr::new(env!(
            "CARGO_BIN_EXE_cargo-fe2o3-runner-chain-fixture"
        ))),
        b"--mode",
        b"ssh",
        b"--option",
        b"host-\xff",
        b"--report",
        runner_report.as_os_str().as_bytes(),
        b"--runner-end",
    ];
    let runner = OsString::from_vec(runner_parts.join(&b' '));
    let preserved = OsString::from_vec(b"runner-env-\xfe".to_vec());
    let payload = OsString::from_vec(b"ssh-payload-\xfd".to_vec());
    let mut command = fixture.command(&[
        OsString::from("run"),
        OsString::from("--target=x86_64-unknown-linux-gnu"),
    ]);
    command
        .env("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER", runner)
        .env(
            "FE2O3_TEST_RUN_APPLICATION",
            env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture"),
        )
        .env("FE2O3_TEST_RUN_APPLICATION_REPORT", &application_report)
        .env("FE2O3_TEST_RUN_APPLICATION_PAYLOAD", payload)
        .env("FE2O3_PRIVATE_BUILD_CAPABILITY", "must-not-leak")
        .env("RUNNER_CHAIN_ENV", preserved);

    let output = command.output().expect("run environment runner chain");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_runner_chain_report(&runner_report, "ssh", None);
    let runner: serde_json::Value =
        serde_json::from_slice(&fs::read(&runner_report).expect("read runner report"))
            .expect("decode runner report");
    assert!(
        runner["prefix_hex"]
            .as_array()
            .expect("prefix array")
            .iter()
            .any(|value| value == "686f73742dff")
    );
    assert_application_report(&application_report, "7373682d7061796c6f61642dfd", None);
}

#[test]
fn cfg_selected_runner_fails_closed_instead_of_being_bypassed() {
    let fixture = ProjectFixture::standalone();
    let mut command = fixture.command(&[
        OsString::from("run"),
        OsString::from("--target=x86_64-unknown-linux-gnu"),
    ]);
    command.env(
        "FE2O3_TEST_TARGET_TABLE_JSON",
        r#"{"cfg(unix)":{"runner":["qemu","--cpu","max"]}}"#,
    );

    let output = command.output().expect("run cfg runner rejection");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot safely resolve cfg-selected"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn recursive_runner_configuration_fails_closed() {
    let fixture = ProjectFixture::standalone();
    let runner = serde_json::json!([env!("CARGO_BIN_EXE_cargo-fe2o3")]);
    let mut command = fixture.command(&[
        OsString::from("run"),
        OsString::from("--target=x86_64-unknown-linux-gnu"),
    ]);
    command.env("FE2O3_TEST_CONFIG_RUNNER_JSON", runner.to_string());

    let output = command.output().expect("run recursive runner rejection");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("recursive cargo-fe2o3"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn hardlink_to_cargo_fe2o3_is_rejected_as_a_recursive_runner() {
    let fixture = ProjectFixture::standalone();
    let shared_executable = Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    let hardlink_directory = SameFilesystemFixture::beside(shared_executable);
    let executable = hardlink_directory.path().join("cargo-fe2o3-private");
    let staging = hardlink_directory
        .path()
        .join("cargo-fe2o3-private.staging");
    fs::copy(shared_executable, &staging).expect("stage private recursive runner executable");
    let staged = OpenOptions::new()
        .write(true)
        .open(&staging)
        .expect("open staged recursive runner executable");
    staged
        .sync_all()
        .expect("sync staged recursive runner executable");
    drop(staged);
    fs::rename(&staging, &executable).expect("publish private recursive runner executable");
    let hardlink = hardlink_directory.path().join("cargo-fe2o3-hardlink");
    fs::hard_link(&executable, &hardlink).expect("create recursive runner hardlink");
    let source = fs::metadata(&executable).expect("inspect runner executable");
    let alias = fs::metadata(&hardlink).expect("inspect runner hardlink");
    use std::os::unix::fs::MetadataExt;
    assert_eq!((source.dev(), source.ino()), (alias.dev(), alias.ino()));
    let runner = serde_json::json!([hardlink]);
    let mut command = fixture.command_with_program(
        &executable,
        &[
            OsString::from("run"),
            OsString::from("--target=x86_64-unknown-linux-gnu"),
        ],
    );
    command.env("FE2O3_TEST_CONFIG_RUNNER_JSON", runner.to_string());

    let output = output_retrying_text_file_busy(&mut command, "run hardlink runner rejection");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("recursive cargo-fe2o3"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn assert_runner_chain_report(path: &Path, mode: &str, preserved_environment: Option<&str>) {
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("read runner report"))
            .expect("decode runner report");
    assert_eq!(report["artifact_fd_open"], false);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["inherited_fds"], serde_json::json!([]));
    assert_eq!(report["leaked_environment"], serde_json::json!([]));
    assert_eq!(
        report["preserved_environment_hex"],
        preserved_environment
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null)
    );
    assert!(
        report["prefix_hex"]
            .as_array()
            .expect("prefix array")
            .iter()
            .any(|value| value == &hex(mode.as_bytes()))
    );
}

#[cfg(unix)]
fn assert_application_report(path: &Path, payload: &str, preserved_environment: Option<&str>) {
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("read application report"))
            .expect("decode application report");
    assert_eq!(report["artifact_fd_open"], false);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["leaked_environment"], serde_json::json!([]));
    assert_eq!(report["unexpected_environment"], serde_json::json!([]));
    assert_eq!(report["payload_hex"], payload);
    assert_eq!(
        report["preserved_environment_hex"],
        preserved_environment
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null)
    );
}

#[cfg(unix)]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value.to_str().expect("UTF-8 fixture argument").as_bytes()
}
