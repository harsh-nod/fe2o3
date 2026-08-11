use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::io::{Read, Write};
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
fn trusted_build_script_exec_replay_documents_broker_scope() {
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
    let report = fs::read_to_string(report).expect("read exec build-script report");
    assert!(report.contains("backend_open=true"), "{report}");
    assert!(report.contains("artifact_open=true"), "{report}");
}

#[cfg(target_os = "linux")]
#[test]
fn real_cargo_build_script_exec_replay_documents_broker_scope() {
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
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("build completed without an authorized device backend"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(report).expect("read real build-script replay report");
    assert!(report.contains("backend_open=true"), "{report}");
    assert!(report.contains("artifact_open=true"), "{report}");
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
    let report = std::env::var_os("FE2O3_TEST_PROC_MACRO_REPORT").unwrap();
    std::fs::write(report, format!("backend_open={backend}\nartifact_open={artifact}\n")).unwrap();
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
        .args([
            OsString::from("__fe2o3-runner-v1"),
            OsString::from("0"),
            OsString::from(env!("CARGO_BIN_EXE_cargo-fe2o3-runner-app-fixture")),
            report.as_os_str().to_os_string(),
            payload,
        ])
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
    assert_eq!(report["payload_hex"], "6170706c69636174696f6e2dff");
    fs::remove_dir_all(root).expect("remove runner fixture");
}

#[cfg(unix)]
#[test]
fn configured_multi_argument_runner_is_chained_after_capability_scrub() {
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
    assert_runner_chain_report(&runner_report, "qemu", "707265736572766564");
    assert_application_report(
        &application_report,
        "71656d752d7061796c6f6164",
        "707265736572766564",
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_environment_runner_is_chained_losslessly() {
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
    assert_runner_chain_report(&runner_report, "ssh", "72756e6e65722d656e762dfe");
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
    assert_application_report(
        &application_report,
        "7373682d7061796c6f61642dfd",
        "72756e6e65722d656e762dfe",
    );
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
    let hardlink = fixture.root.join("cargo-fe2o3-hardlink");
    fs::hard_link(env!("CARGO_BIN_EXE_cargo-fe2o3"), &hardlink)
        .expect("create recursive runner hardlink");
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
fn assert_runner_chain_report(path: &Path, mode: &str, preserved_environment: &str) {
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("read runner report"))
            .expect("decode runner report");
    assert_eq!(report["artifact_fd_open"], false);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["leaked_environment"], serde_json::json!([]));
    assert_eq!(report["preserved_environment_hex"], preserved_environment);
    assert!(
        report["prefix_hex"]
            .as_array()
            .expect("prefix array")
            .iter()
            .any(|value| value == &hex(mode.as_bytes()))
    );
}

#[cfg(unix)]
fn assert_application_report(path: &Path, payload: &str, preserved_environment: &str) {
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("read application report"))
            .expect("decode application report");
    assert_eq!(report["artifact_fd_open"], false);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["leaked_environment"], serde_json::json!([]));
    assert_eq!(report["payload_hex"], payload);
    assert_eq!(report["preserved_environment_hex"], preserved_environment);
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
