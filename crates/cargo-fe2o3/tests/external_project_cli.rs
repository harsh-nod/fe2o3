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

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

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
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
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
    command.env_remove("FE2O3_BACKEND");

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
    let real_rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .args(["build", "-j", "4"])
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("RUSTC", env!("CARGO_BIN_EXE_cargo-fe2o3-rustc-fixture"))
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
        fs::read_dir(fixture.target.join("fe2o3"))
            .expect("read committed artifact directory")
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".hsaco"))
    );
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
    let real_rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("RUSTC", env!("CARGO_BIN_EXE_cargo-fe2o3-rustc-fixture"))
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
        stderr.contains("failed to receive brokered capabilities")
            || stderr.contains("Connection reset by peer"),
        "{stderr}"
    );
    assert!(
        !report.exists(),
        "unauthorized wrapper replay reached its attacker-selected compiler"
    );
}

#[cfg(target_os = "linux")]
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
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_CODEGEN_PIPELINE", "collected-row-softmax-v1")
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
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("FE2O3_BACKEND", &fixture.backend)
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_CODEGEN_PIPELINE", "collected-row-softmax-v1")
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
    let real_rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let real_cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .arg("build")
        .current_dir(&fixture.workspace)
        .env("CARGO", real_cargo)
        .env("RUSTC", env!("CARGO_BIN_EXE_cargo-fe2o3-rustc-fixture"))
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
    assert!(first.output().unwrap().status.success());

    let mut second = fixture.command(&[OsString::from("build")]);
    second.env(
        "FE2O3_TEST_BUILD_CONFIG_JSON",
        r#"{"rustflags":["--cfg","second"]}"#,
    );
    assert!(second.output().unwrap().status.success());

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
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
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
    assert_eq!(report["artifact_fd_open"], false);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["leaked_environment"], serde_json::json!([]));
    assert_eq!(report["inherited_fds"], serde_json::json!([]));
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
            let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
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
            assert_eq!(report["inherited_fds"], serde_json::json!([]));
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
        let thirty_third = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
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
    assert_eq!(report["inherited_fds"], serde_json::json!([]));
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
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
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
    assert_eq!(report["inherited_fds"], serde_json::json!([]));
    assert_eq!(report["slot_unlocks"], serde_json::json!([]));
    fs::remove_dir_all(root).expect("remove slot fixture");
}

#[cfg(unix)]
#[test]
fn hidden_supervisor_rejects_malformed_descriptors_without_abort() {
    use std::os::fd::IntoRawFd;
    use std::time::{Duration, Instant};

    fn invoke(channel: i32, slot: i32, inherited: &[i32]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
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
    let executable = Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    let hardlink_directory = SameFilesystemFixture::beside(executable);
    let hardlink = hardlink_directory.path().join("cargo-fe2o3-hardlink");
    fs::hard_link(executable, &hardlink).expect("create recursive runner hardlink");
    let source = fs::metadata(executable).expect("inspect runner executable");
    let alias = fs::metadata(&hardlink).expect("inspect runner hardlink");
    use std::os::unix::fs::MetadataExt;
    assert_eq!((source.dev(), source.ino()), (alias.dev(), alias.ino()));
    let runner = serde_json::json!([hardlink]);
    let mut command = fixture.command(&[
        OsString::from("run"),
        OsString::from("--target=x86_64-unknown-linux-gnu"),
    ]);
    command.env("FE2O3_TEST_CONFIG_RUNNER_JSON", runner.to_string());

    let output = command.output().expect("run hardlink runner rejection");
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
