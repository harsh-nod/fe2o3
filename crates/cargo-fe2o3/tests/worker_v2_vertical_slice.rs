#![cfg(target_os = "linux")]

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};

use fe2o3_hsaco_finalize::finalize_unfinalized;
use fe2o3_worker_v2_bundle::WorkerV2LoadEnvelopeV1;

#[allow(dead_code)]
#[path = "../src/worker_v2_artifact_container_test_fixture.rs"]
mod alpha_zeta_support;

include!("../../fe2o3-hsaco-finalize/tests/fixtures/worker_v2_hsaco_test_support.rs");

const WORKER_ID: &str = "cargo-fe2o3-fixture-worker-v1";
static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf, Mutex<Option<OuterHarness>>);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-worker-v2-flow-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path, Mutex::new(None))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Some(harness) = self.1.get_mut().unwrap().take() {
            let output = harness.stop();
            if !output.status.success() && !thread::panicking() {
                panic!("outer cargo-fe2o3 failed: {}", stderr(&output));
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn worker_fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-worker-v2-fixture"))
}

fn envelope_input_fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-envelope-input-fixture"))
}

fn host_consumer_input_fixture() -> &'static Path {
    Path::new(env!(
        "CARGO_BIN_EXE_cargo-fe2o3-host-consumer-input-fixture"
    ))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn descriptor_identity(domain: &[u8], descriptor: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((descriptor.len() as u64).to_le_bytes());
    digest.update(descriptor);
    digest.finalize().into()
}

fn descriptor_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn descriptor_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn descriptor_text(bytes: &mut Vec<u8>, value: &str) {
    descriptor_u16(bytes, value.len() as u16);
    bytes.extend_from_slice(value.as_bytes());
}

fn cov6_descriptor_table() -> Vec<u8> {
    const RUST_TYPE_DOMAIN: &[u8] = b"FE2O3/RUST-TYPE/V1\0";
    const DEVICE_LAYOUT_DOMAIN: &[u8] = b"FE2O3/DEVICE-LAYOUT/V1\0";
    const SOURCE_DESCRIPTOR: &[u8] = &[2, 10, 0, 0];
    const LAYOUT_DESCRIPTOR: &[u8] = &[2, 10, 16, 0, 8, 0, 8, 8, 0, 0, 0, 0];

    let source_identity = descriptor_identity(RUST_TYPE_DOMAIN, SOURCE_DESCRIPTOR);
    let layout_identity = descriptor_identity(DEVICE_LAYOUT_DOMAIN, LAYOUT_DESCRIPTOR);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"FE2O3KD\0");
    descriptor_u16(&mut bytes, 1);
    descriptor_u16(&mut bytes, 0);
    descriptor_u32(&mut bytes, 0);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[6, 8, 1, 0]);
    descriptor_text(&mut bytes, "rustc");
    descriptor_text(&mut bytes, "fixture");
    bytes.extend_from_slice(&[0x66; 20]);
    descriptor_text(&mut bytes, "cargo-fe2o3-test");
    descriptor_text(&mut bytes, "fixture");
    descriptor_text(&mut bytes, "gfx942:xnack-");
    descriptor_u16(&mut bytes, 1);
    descriptor_u16(&mut bytes, 1);
    descriptor_u16(&mut bytes, 1);
    descriptor_u16(&mut bytes, 0);

    bytes.extend_from_slice(&source_identity);
    bytes.extend_from_slice(SOURCE_DESCRIPTOR);
    bytes.extend_from_slice(&layout_identity);
    bytes.extend_from_slice(LAYOUT_DESCRIPTOR);

    bytes.extend_from_slice(&[0x61; 32]);
    descriptor_text(&mut bytes, "workflow_kernel");
    descriptor_text(&mut bytes, "workflow_kernel");
    descriptor_text(&mut bytes, "workflow_kernel.kd");
    bytes.extend_from_slice(&[1, 1, 1, 0]);
    bytes.extend_from_slice(&[0x62; 32]);
    bytes.extend_from_slice(&[0x63; 32]);
    bytes.extend_from_slice(&[2, 1, 1, 0]);
    bytes.extend_from_slice(&[0x64; 32]);
    bytes.extend_from_slice(&[0x65; 32]);
    descriptor_u16(&mut bytes, 0);

    bytes.extend_from_slice(&[1, 1, 0, 0]);
    descriptor_u32(&mut bytes, 256);
    descriptor_u32(&mut bytes, 1);
    descriptor_u32(&mut bytes, 1);
    descriptor_u32(&mut bytes, u32::MAX);
    descriptor_u32(&mut bytes, 1);
    descriptor_u32(&mut bytes, 1);
    descriptor_u32(&mut bytes, 256);
    descriptor_u32(&mut bytes, 0);
    descriptor_u32(&mut bytes, 64 * 1024);

    descriptor_u16(&mut bytes, 1);
    descriptor_u16(&mut bytes, 2);
    descriptor_u32(&mut bytes, 16);
    descriptor_u32(&mut bytes, 272);
    descriptor_u32(&mut bytes, 8);

    descriptor_u16(&mut bytes, 0);
    descriptor_u16(&mut bytes, 0);
    descriptor_text(&mut bytes, "values");
    bytes.extend_from_slice(&source_identity);
    bytes.extend_from_slice(&layout_identity);
    bytes.extend_from_slice(&[2, 2, 2, 0]);
    descriptor_u16(&mut bytes, 2);
    descriptor_u16(&mut bytes, 0);

    bytes.extend_from_slice(&[2, 0, 2, 2]);
    descriptor_u32(&mut bytes, 0);
    descriptor_u16(&mut bytes, 8);
    descriptor_u16(&mut bytes, 8);
    descriptor_u16(&mut bytes, 0);
    descriptor_u16(&mut bytes, 0);
    bytes.extend_from_slice(&[3, 8, 1, 1]);
    descriptor_u32(&mut bytes, 8);
    descriptor_u16(&mut bytes, 8);
    descriptor_u16(&mut bytes, 8);
    descriptor_u16(&mut bytes, 0);
    descriptor_u16(&mut bytes, 0);

    let total_len = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    bytes
}

fn write_config(directory: &TestDirectory, selects_invocation: bool) -> PathBuf {
    write_config_with_output(directory, selects_invocation, None)
}

fn write_s09_config(directory: &TestDirectory) -> PathBuf {
    let path = write_config_with_output_and_cov(directory, true, None, 6, false);
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["source_debug_profile"] = JsonValue::String("s09-alpha-gfx942-o0-v1".to_owned());
    for option in value["link_options"].as_array_mut().unwrap() {
        match option["name"].as_str().unwrap() {
            "opt-level" => option["value"] = JsonValue::String("0".to_owned()),
            "strip-debug" => option["value"] = JsonValue::String("false".to_owned()),
            _ => {}
        }
    }
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

fn write_config_with_output(
    directory: &TestDirectory,
    selects_invocation: bool,
    worker_output: Option<&[u8]>,
) -> PathBuf {
    write_config_with_output_and_cov(directory, selects_invocation, worker_output, 5, false)
}

fn write_config_with_output_and_cov(
    directory: &TestDirectory,
    selects_invocation: bool,
    worker_output: Option<&[u8]>,
    code_object_version: u8,
    require_envelope: bool,
) -> PathBuf {
    let worker_bytes = fs::read(worker_fixture()).unwrap();
    let selected_source = if selects_invocation {
        directory.0.join("workflow_fixture.rs")
    } else {
        directory.0.join("different_device_unit.rs")
    };
    let providers = worker_output.map_or_else(Vec::new, |bytes| {
        let path = directory.0.join("worker-output.hsaco");
        fs::write(&path, bytes).unwrap();
        vec![json!({
            "byte_len": bytes.len(),
            "kind": "amdgpu-relocatable",
            "path": path,
            "sha256": hex(&Sha256::digest(bytes))
        })]
    });
    let mut value = json!({
        "candidate_output_max_bytes": 67108864,
        "format": "fe2o3-worker-v2-config-v2",
        "limits": {
            "stderr_bytes": 1024,
            "stdout_bytes": 16384,
            "timeout_ms": 2000
        },
        "link_options": [
            {"name": "code-object-version", "value": code_object_version.to_string()},
            {"name": "opt-level", "value": "2"},
            {"name": "strip-debug", "value": "true"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": providers,
        "units": [{
            "crate_name": "workflow_fixture",
            "source": selected_source,
            "working_directory": directory.0
        }],
        "worker": {
            "byte_len": worker_bytes.len(),
            "llvm_build_identity": "cargo-fe2o3-fixture-llvm-v1",
            "path": worker_fixture(),
            "sha256": hex(&Sha256::digest(&worker_bytes)),
            "worker_build_identity": WORKER_ID
        }
    });
    if require_envelope {
        value["load_envelope"] = JsonValue::String("required".to_owned());
    }
    let path = directory.0.join("worker-v2.json");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

struct OuterHarness {
    config_key: [u8; 32],
    child: Child,
    stdout: Receiver<Vec<u8>>,
    stderr: Receiver<Vec<u8>>,
    control: PathBuf,
    next_request: u64,
}

impl OuterHarness {
    fn start(directory: &TestDirectory, config: Option<&Path>) -> Result<Self, Output> {
        let control = directory.0.join("vertical-control");
        let _ = fs::remove_dir_all(&control);
        fs::create_dir(&control).unwrap();
        let mut command = outer_command(directory, config, &control);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn().unwrap();
        let stdout_receiver = capture_output(child.stdout.take().unwrap());
        let stderr_receiver = capture_output(child.stderr.take().unwrap());
        let mut harness = Self {
            config_key: config_key(config),
            child,
            stdout: stdout_receiver,
            stderr: stderr_receiver,
            control,
            next_request: 1,
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if harness.control.join("ready").exists() {
                return Ok(harness);
            }
            if let Some(status) = harness.child.try_wait().unwrap() {
                return Err(harness.collect_output(status));
            }
            thread::sleep(Duration::from_millis(2));
        }
        let output = harness.kill_and_collect();
        panic!(
            "outer cargo-fe2o3 did not start vertical Cargo fixture: {}",
            stderr(&output)
        );
    }

    fn submit(&mut self, options: VerticalRunOptions<'_>) -> VerticalRequest {
        let id = self.next_request;
        self.next_request += 1;
        for name in ["cov6", "fault", "strip", "restore", "handoff"] {
            let _ = fs::remove_file(self.control.join(name));
        }
        fs::write(self.control.join("mode"), options.rustc_mode).unwrap();
        if options.cov6 {
            fs::write(self.control.join("cov6"), []).unwrap();
        }
        if let Some(fault) = options.fault {
            fs::write(self.control.join("fault"), fault).unwrap();
        }
        if let Some(strip) = options.strip {
            fs::write(self.control.join("strip"), strip).unwrap();
        }
        if let Some(restore) = options.restore {
            fs::write(
                self.control.join("restore"),
                restore.as_os_str().as_encoded_bytes(),
            )
            .unwrap();
        }
        if let Some(handoff) = options.handoff {
            fs::write(
                self.control.join("handoff"),
                handoff.as_os_str().as_encoded_bytes(),
            )
            .unwrap();
        }
        let request = self.control.join("request");
        let request_temp = self.control.join("request.tmp");
        fs::write(&request_temp, id.to_string()).unwrap();
        fs::rename(request_temp, request).unwrap();
        VerticalRequest {
            control: self.control.clone(),
            id,
        }
    }

    fn stop(self) -> Output {
        self.stop_with_timeout(Duration::from_secs(10))
    }

    fn stop_with_timeout(mut self, graceful_timeout: Duration) -> Output {
        fs::write(self.control.join("stop"), []).unwrap();
        if let Some(status) = wait_for_exit(&mut self.child, graceful_timeout) {
            return self.collect_output(status);
        }
        self.kill_and_collect()
    }

    fn kill_and_collect(mut self) -> Output {
        let process_group = i32::try_from(self.child.id()).unwrap();
        let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
        let error = std::io::Error::last_os_error();
        assert!(
            result == 0 || error.raw_os_error() == Some(libc::ESRCH),
            "failed to kill stalled outer cargo-fe2o3 process group: {}",
            error
        );
        let status = wait_for_exit(&mut self.child, Duration::from_secs(5))
            .expect("outer cargo-fe2o3 survived bounded SIGKILL");
        self.collect_output(status)
    }

    fn collect_output(self, status: ExitStatus) -> Output {
        Output {
            status,
            stdout: self
                .stdout
                .recv_timeout(Duration::from_secs(5))
                .expect("outer cargo-fe2o3 stdout remained open after exit"),
            stderr: self
                .stderr
                .recv_timeout(Duration::from_secs(5))
                .expect("outer cargo-fe2o3 stderr remained open after exit"),
        }
    }
}

fn capture_output(mut pipe: impl Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (sender, receiver) = sync_channel(1);
    thread::spawn(move || {
        let mut output = Vec::new();
        pipe.read_to_end(&mut output).unwrap();
        let _ = sender.send(output);
    });
    receiver
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn outer_harness_stop_kills_an_unresponsive_process_group() {
    let directory = TestDirectory::new();
    let control = directory.0.join("unresponsive-control");
    fs::create_dir(&control).unwrap();
    let mut command = Command::new("bash");
    command
        .args(["-c", "trap '' TERM; while :; do sleep 60; done"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn().unwrap();
    let harness = OuterHarness {
        config_key: [0; 32],
        stdout: capture_output(child.stdout.take().unwrap()),
        stderr: capture_output(child.stderr.take().unwrap()),
        child,
        control,
        next_request: 1,
    };

    let started = Instant::now();
    let output = harness.stop_with_timeout(Duration::from_millis(100));
    assert!(!output.status.success());
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "unresponsive outer harness teardown exceeded its bound"
    );
}

struct VerticalRequest {
    control: PathBuf,
    id: u64,
}

impl VerticalRequest {
    fn result_path(&self, suffix: &str) -> PathBuf {
        self.control.join(format!("result-{}.{suffix}", self.id))
    }

    fn wrapper_pid(&self) -> Option<u32> {
        fs::read_to_string(self.result_path("pid"))
            .ok()
            .and_then(|pid| pid.parse().ok())
    }

    fn wait(self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if self.result_path("done").exists() {
                let raw: [u8; 4] = fs::read(self.result_path("status"))
                    .unwrap()
                    .try_into()
                    .unwrap();
                return Output {
                    status: ExitStatus::from_raw(i32::from_le_bytes(raw)),
                    stdout: fs::read(self.result_path("stdout")).unwrap(),
                    stderr: fs::read(self.result_path("stderr")).unwrap(),
                };
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("vertical wrapper request {} timed out", self.id);
    }
}

#[derive(Clone, Copy)]
struct VerticalRunOptions<'a> {
    rustc_mode: &'a str,
    cov6: bool,
    fault: Option<&'a str>,
    strip: Option<&'a str>,
    restore: Option<&'a Path>,
    handoff: Option<&'a Path>,
}

impl<'a> VerticalRunOptions<'a> {
    const fn new(rustc_mode: &'a str) -> Self {
        Self {
            rustc_mode,
            cov6: false,
            fault: None,
            strip: None,
            restore: None,
            handoff: None,
        }
    }
}

fn artifact_dir(directory: &TestDirectory) -> PathBuf {
    directory.0.join("target/fe2o3")
}

struct StaticApplicationFixtures {
    protocol: PathBuf,
    host_consumer: PathBuf,
    no_protocol: PathBuf,
}

fn static_application_fixtures() -> &'static StaticApplicationFixtures {
    static FIXTURES: OnceLock<StaticApplicationFixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = std::env::temp_dir().join(format!(
            "cargo-fe2o3-static-application-fixtures-{}",
            std::process::id()
        ));
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let built = Command::new(cargo)
            .current_dir(workspace)
            .env(
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
                "-C target-feature=+crt-static",
            )
            .env("FE2O3_HIP_SYS_DISABLE", "1")
            .args([
                "build",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--target-dir",
            ])
            .arg(&target)
            .args([
                "-p",
                "cargo-fe2o3",
                "--features",
                "host-consumer-fixture",
                "--bin",
                "cargo-fe2o3-runner-app-fixture",
                "--bin",
                "cargo-fe2o3-host-consumer-app-fixture",
                "--bin",
                "cargo-fe2o3-runner-chain-fixture",
            ])
            .output()
            .unwrap();
        assert!(built.status.success(), "{}", stderr(&built));
        let directory = target.join("x86_64-unknown-linux-gnu/debug");
        StaticApplicationFixtures {
            protocol: directory.join("cargo-fe2o3-runner-app-fixture"),
            host_consumer: directory.join("cargo-fe2o3-host-consumer-app-fixture"),
            no_protocol: directory.join("cargo-fe2o3-runner-chain-fixture"),
        }
    })
}

fn application_fixture() -> &'static Path {
    &static_application_fixtures().protocol
}

fn host_consumer_application_fixture() -> &'static Path {
    &static_application_fixtures().host_consumer
}

fn no_protocol_application_fixture() -> &'static Path {
    &static_application_fixtures().no_protocol
}

fn envelope_paths(directory: &TestDirectory) -> Vec<PathBuf> {
    artifact_entries(directory)
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(".fe2o3-worker-v2-load-envelope-v1-") && name.ends_with(".envelope")
        })
        .collect()
}

fn application_runner_command(
    directory: &TestDirectory,
    application: &Path,
    report: &Path,
) -> Command {
    application_runner_command_with_args(
        directory,
        application,
        [
            report.as_os_str(),
            OsStr::new("worker-v2-application-payload"),
        ],
    )
}

fn application_runner_command_with_args<I, S>(
    directory: &TestDirectory,
    application: &Path,
    arguments: I,
) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    use std::os::unix::fs::MetadataExt;

    let artifact = artifact_dir(directory);
    let metadata = fs::metadata(&artifact).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .arg("__fe2o3-runner-v1")
        .arg("3")
        .arg(hex(artifact.as_os_str().as_encoded_bytes()))
        .arg(metadata.dev().to_string())
        .arg(metadata.ino().to_string())
        .arg("required")
        .arg("0")
        .arg(application)
        .args(arguments);
    command
}

fn host_consumer_runner_command(directory: &TestDirectory, report: &Path) -> Command {
    let capsule = directory.0.join("host-consumer.compiler-transaction");
    let kernel = directory.0.join("host-consumer.kernel-id");
    let envelope = envelope_paths(directory).pop().unwrap();
    let generated = Command::new(host_consumer_input_fixture())
        .args([&envelope, &capsule, &kernel])
        .output()
        .unwrap();
    assert!(generated.status.success(), "{}", stderr(&generated));
    application_runner_command_with_args(
        directory,
        host_consumer_application_fixture(),
        [
            capsule.as_os_str(),
            kernel.as_os_str(),
            OsStr::new("gfx942:xnack-"),
            report.as_os_str(),
        ],
    )
}

fn run_application_fixture(directory: &TestDirectory, report: &Path) -> Output {
    application_runner_command(directory, application_fixture(), report)
        .output()
        .unwrap()
}

fn snapshot_artifacts(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            snapshot_artifacts(&entry.path(), &destination);
        } else {
            assert!(entry.file_type().unwrap().is_file());
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn config_key(config: Option<&Path>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"cargo-fe2o3-vertical-outer-config-v1\0");
    if let Some(config) = config {
        digest.update([1]);
        digest.update(fs::read(config).unwrap());
    } else {
        digest.update([0]);
    }
    digest.finalize().into()
}

fn start_wrapper(
    directory: &TestDirectory,
    config: Option<&Path>,
    options: VerticalRunOptions<'_>,
) -> Result<VerticalRequest, Output> {
    let key = config_key(config);
    let mut harness = directory.1.lock().unwrap();
    if harness
        .as_ref()
        .is_some_and(|harness| harness.config_key != key)
    {
        let output = harness.take().unwrap().stop();
        assert!(output.status.success(), "{}", stderr(&output));
    }
    if harness.is_none() {
        match OuterHarness::start(directory, config) {
            Ok(started) => *harness = Some(started),
            Err(output) => return Err(output),
        }
    }
    Ok(harness.as_mut().unwrap().submit(options))
}

fn run_wrapper_options(
    directory: &TestDirectory,
    config: Option<&Path>,
    options: VerticalRunOptions<'_>,
) -> Output {
    match start_wrapper(directory, config, options) {
        Ok(request) => request.wait(),
        Err(output) => output,
    }
}

fn run_wrapper(directory: &TestDirectory, config: Option<&Path>, rustc_mode: &str) -> Output {
    run_wrapper_options(directory, config, VerticalRunOptions::new(rustc_mode))
}

fn run_wrapper_with_options(
    directory: &TestDirectory,
    config: &Path,
    rustc_mode: &str,
    cov6: bool,
    fault: Option<&str>,
) -> Output {
    let mut options = VerticalRunOptions::new(rustc_mode);
    options.cov6 = cov6;
    options.fault = fault;
    run_wrapper_options(directory, Some(config), options)
}

fn stop_outer_harness(directory: &TestDirectory) {
    let harness = directory.1.lock().unwrap().take();
    if let Some(harness) = harness {
        let output = harness.stop();
        assert!(output.status.success(), "{}", stderr(&output));
    }
}

fn outer_command(directory: &TestDirectory, config: Option<&Path>, control: &Path) -> Command {
    let source = directory.0.join("workflow_fixture.rs");
    fs::write(&source, "fn main() {}\n").unwrap();
    let project_source = directory.0.join("src/main.rs");
    fs::create_dir_all(project_source.parent().unwrap()).unwrap();
    fs::write(&project_source, "fn main() {}\n").unwrap();
    fs::write(
        directory.0.join("Cargo.toml"),
        "[package]\nname='worker-v2-vertical'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    let backend = directory.0.join("librustc_codegen_fe2o3.so");
    fs::write(&backend, b"worker-v2 vertical backend").unwrap();
    let target = directory.0.join("target");
    let cargo_log = directory.0.join("cargo.log");

    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .arg("build")
        .env_clear()
        .current_dir(&directory.0)
        .env("CARGO", env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture"))
        .env("FE2O3_BACKEND", backend)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_FIXTURE_RUSTC_MARKER", directory.0.join("spawned"))
        .env("FE2O3_FIXTURE_SOURCE", &source)
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_TEST_CARGO_LOG", cargo_log)
        .env("FE2O3_TEST_TARGET_DIRECTORY", target)
        .env("FE2O3_TEST_VERTICAL_CONTROL_DIR", control)
        .env("FE2O3_TEST_VERTICAL_RUSTC", worker_fixture())
        .env("FE2O3_TEST_WORKSPACE_ROOT", &directory.0);
    if let Some(config) = config {
        command.env("FE2O3_WORKER_V2_CONFIG_V2", config);
    }
    if directory.0.join("alpha-zeta-profile").exists() {
        command.env("FE2O3_TEST_WORKER_V2_ALPHA_ZETA", "1");
    }
    command
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stage_ready_restart(directory: &TestDirectory) -> (PathBuf, PathBuf) {
    let config = write_config(directory, true);
    let handoff_marker = directory.0.join("handoff-ready");
    let mut options = VerticalRunOptions::new("stop-after-handoff");
    options.handoff = Some(&handoff_marker);
    let request = start_wrapper(directory, Some(&config), options).unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut wrapper_pid = None;
    while Instant::now() < deadline {
        if let Some(pid) = request.wrapper_pid() {
            let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
            if handoff_marker.exists() && status.lines().any(|line| line.starts_with("State:\tT")) {
                wrapper_pid = Some(pid);
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    let wrapper_pid = wrapper_pid.expect("vertical wrapper did not stop after handoff");
    let status = fs::read_to_string(format!("/proc/{wrapper_pid}/status")).unwrap();
    assert!(status.lines().any(|line| line.starts_with("State:\tT")));
    assert_eq!(unsafe { libc::kill(wrapper_pid as i32, libc::SIGKILL) }, 0);
    assert!(!request.wait().status.success());

    let attempt = fs::read_to_string(directory.0.join("spawned")).unwrap();
    let source = directory.0.join("workflow_fixture.rs");
    let artifact_dir = artifact_dir(directory);
    let staged = Command::new(worker_fixture())
        .arg("--stage-restart")
        .arg(&artifact_dir)
        .arg(&source)
        .arg(attempt.trim())
        .output()
        .unwrap();
    assert!(staged.status.success(), "{}", stderr(&staged));
    fs::remove_file(directory.0.join("spawned")).unwrap();
    (config, artifact_dir)
}

struct PublicationFixture {
    config: PathBuf,
    raw_worker_output: Vec<u8>,
    expected_publication: Vec<u8>,
    cov6: bool,
}

fn publication_fixture(directory: &TestDirectory, cov6: bool) -> PublicationFixture {
    publication_fixture_with_envelope(directory, cov6, false)
}

fn publication_fixture_with_envelope(
    directory: &TestDirectory,
    cov6: bool,
    require_envelope: bool,
) -> PublicationFixture {
    let table = cov6.then(cov6_descriptor_table);
    let built = fixture_with_descriptor_table(
        FixtureOptions {
            target: "gfx942:xnack-",
            code_object_version: if cov6 { 4 } else { 3 },
            entry: "workflow_kernel",
            descriptor: "workflow_kernel.kd",
            ..FixtureOptions::valid()
        },
        table.as_deref(),
    );
    let provider = built.bytes;
    let mut raw_worker_output = provider.clone();
    raw_worker_output[built.text_offset] ^= 1;
    let expected_publication = if cov6 {
        finalize_unfinalized(&raw_worker_output)
            .unwrap()
            .as_bytes()
            .to_vec()
    } else {
        raw_worker_output.clone()
    };
    let config = write_config_with_output_and_cov(
        directory,
        true,
        Some(&provider),
        if cov6 { 6 } else { 5 },
        require_envelope,
    );
    PublicationFixture {
        config,
        raw_worker_output,
        expected_publication,
        cov6,
    }
}

fn required_alpha_zeta_publication_fixture(directory: &TestDirectory) -> PublicationFixture {
    required_alpha_zeta_publication_fixture_with_seed(directory, 0)
}

fn required_alpha_zeta_publication_fixture_with_seed(
    directory: &TestDirectory,
    identity_seed: u8,
) -> PublicationFixture {
    let provider = alpha_zeta_support::canonical_alpha_zeta_unfinalized_fixture();
    let mut raw_worker_output = provider.clone();
    let text = raw_worker_output
        .windows(16)
        .position(|window| window == [0xbf; 16])
        .unwrap();
    raw_worker_output[text] ^= 1;
    let expected_publication = finalize_unfinalized(&raw_worker_output)
        .unwrap()
        .as_bytes()
        .to_vec();
    let raw_path = directory.0.join("expected-worker-output.hsaco");
    let finalized_path = directory.0.join("expected-finalized-output.hsaco");
    let capsule = directory.0.join("envelope-inputs.capsule");
    fs::write(&raw_path, &raw_worker_output).unwrap();
    fs::write(&finalized_path, &expected_publication).unwrap();
    let _ = fs::remove_file(&capsule);
    let generated = Command::new(envelope_input_fixture())
        .args([&raw_path, &finalized_path, &capsule])
        .arg(identity_seed.to_string())
        .output()
        .unwrap();
    assert!(generated.status.success(), "{}", stderr(&generated));

    let config = write_config_with_output_and_cov(directory, true, Some(&provider), 6, false);
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    let bytes = fs::read(&capsule).unwrap();
    value["load_envelope"] = JsonValue::String("required".to_owned());
    value["load_envelope_inputs"] = json!({
        "byte_len": bytes.len(),
        "path": capsule,
        "sha256": hex(&Sha256::digest(&bytes))
    });
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    fs::write(directory.0.join("alpha-zeta-profile"), []).unwrap();
    PublicationFixture {
        config,
        raw_worker_output,
        expected_publication,
        cov6: true,
    }
}

fn artifact_entries(directory: &TestDirectory) -> Vec<PathBuf> {
    fs::read_dir(artifact_dir(directory))
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default()
}

fn published_artifacts(directory: &TestDirectory) -> Vec<Vec<u8>> {
    artifact_entries(directory)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fe2o3-link-artifact-v1-")
        })
        .map(|path| fs::read(path).unwrap())
        .collect()
}

fn restart_records(directory: &TestDirectory) -> Vec<PathBuf> {
    artifact_entries(directory)
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.ends_with(".record")
                && (name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                    || name.starts_with(".fe2o3-cargo-worker-v2-resume-v1-"))
        })
        .collect()
}

fn envelope_input_residue(directory: &TestDirectory) -> Vec<PathBuf> {
    artifact_entries(directory)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fe2o3-worker-v2-envelope-inputs-v1-")
        })
        .collect()
}

#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn envelope_publication_temp_residue(directory: &TestDirectory) -> Vec<PathBuf> {
    artifact_entries(directory)
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(".fe2o3-worker-v2-load-envelope-v1-")
                && name.contains(".envelope.tmp-")
        })
        .collect()
}

#[test]
fn valid_worker_output_persists_before_publication_and_cleans_exact_restart_state() {
    let directory = TestDirectory::new();
    let built = fixture(FixtureOptions {
        target: "gfx942:xnack-",
        code_object_version: 3,
        entry: "workflow_kernel",
        descriptor: "workflow_kernel.kd",
        ..FixtureOptions::valid()
    });
    assert!(built.text_offset < built.bytes.len());
    let provider = built.bytes;
    let mut output = provider.clone();
    output[built.text_offset] ^= 1;
    let config = write_config_with_output(&directory, true, Some(&provider));

    let result = run_wrapper(&directory, Some(&config), "publish-valid");
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(directory.0.join("spawned").exists());

    let artifact_dir = artifact_dir(&directory);
    let entries = fs::read_dir(&artifact_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|path| {
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".fe2o3-link-artifact-v1-")
            && fs::read(path).unwrap() == output
    }));
    assert!(entries.iter().all(|path| {
        let name = path.file_name().unwrap().to_string_lossy();
        !name.ends_with(".record")
            || (!name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                && !name.starts_with(".fe2o3-cargo-worker-v2-resume-v1-"))
    }));
}

#[test]
fn ordinary_cov6_production_publishes_exact_non_authoritative_finalized_bytes() {
    let directory = TestDirectory::new();
    let fixture = publication_fixture(&directory, true);
    let result = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert_ne!(fixture.raw_worker_output, fixture.expected_publication);
    assert_eq!(
        published_artifacts(&directory),
        [fixture.expected_publication]
    );
    assert!(restart_records(&directory).is_empty());
}

#[test]
fn required_cov6_rejects_missing_configured_capsule_before_attempt() {
    let directory = TestDirectory::new();
    let fixture = publication_fixture_with_envelope(&directory, true, true);
    let result = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(!result.status.success());
    assert!(
        stderr(&result).contains("requires load_envelope_inputs"),
        "{}",
        stderr(&result)
    );
    assert!(published_artifacts(&directory).is_empty());
    assert!(restart_records(&directory).is_empty());
    assert!(!directory.0.join("spawned").exists());
}

#[test]
fn required_cov6_production_wrapper_publishes_a_canonical_envelope() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let result = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert_eq!(
        published_artifacts(&directory),
        [fixture.expected_publication]
    );
    assert!(restart_records(&directory).is_empty());
    let envelope_count = fs::read_dir(artifact_dir(&directory))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| {
            let name = name.to_string_lossy();
            name.starts_with(".fe2o3-worker-v2-load-envelope-v1-") && name.ends_with(".envelope")
        })
        .count();
    assert_eq!(envelope_count, 1);
    assert!(envelope_input_residue(&directory).is_empty());
}

#[test]
fn canonical_envelope_is_consumed_through_descriptor_protocol_completion() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));

    let report = directory.0.join("application-handoff.json");
    let application = run_application_fixture(&directory, &report);
    assert!(application.status.success(), "{}", stderr(&application));
    let report: JsonValue = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["artifact_fd_open"], false);
    assert_eq!(report["backend_fd_open"], false);
    assert_eq!(report["leaked_environment"], json!([]));
    assert_eq!(report["handoff"]["acknowledged"], true);
    assert_eq!(report["handoff"]["artifact_directory_read_only"], true);
    assert_eq!(report["handoff"]["read_only"], true);
    assert_eq!(report["handoff"]["commitment"].as_str().unwrap().len(), 64);
    assert_eq!(report["payload_hex"], hex(b"worker-v2-application-payload"));
}

#[test]
fn real_host_consumer_reaches_exact_prerequisite_admission() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));

    let report = directory.0.join("host-consumer-report.json");
    let consumed = host_consumer_runner_command(&directory, &report)
        .env("LD_PRELOAD", "")
        .env("LD_LIBRARY_PATH", "/untrusted/runtime")
        .env("LD_AUDIT", "")
        .env("LD_PROFILE", "untrusted.profile")
        .env("GLIBC_TUNABLES", "glibc.malloc.trim_threshold=1")
        .env("GCONV_PATH", "/untrusted/gconv")
        .env("HOSTALIASES", "/untrusted/aliases")
        .env("LANG", "C.UTF-8")
        .env("LANGUAGE", "untrusted")
        .env("LC_ALL", "C.UTF-8")
        .env("LOCALDOMAIN", "untrusted.invalid")
        .env("LOCPATH", "/untrusted/locale")
        .env("MALLOC_ARENA_MAX", "1")
        .env("MALLOC_PERTURB_", "7")
        .env("NSS_DISABLE_AUDIT", "1")
        .env("RES_OPTIONS", "attempts:1")
        .env("RESOLV_HOST_CONF", "/untrusted/host.conf")
        .output()
        .unwrap();
    assert!(!consumed.status.success());
    assert!(
        stderr(&consumed).contains("AmdWave"),
        "{}",
        stderr(&consumed)
    );
    assert!(
        !stderr(&consumed).contains("loader-sensitive environment survived"),
        "{}",
        stderr(&consumed)
    );
    let report: JsonValue = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["host_consumer"], true);
    assert_eq!(report["loader_environment_clear"], true);
    assert_eq!(report["admitted"], false);

    let rejected_report = directory.0.join("host-consumer-substitution.json");
    let rejected = host_consumer_runner_command(&directory, &rejected_report)
        .env("RUNNER_HOST_FIXTURE_SUBSTITUTE_COMMITMENT", "1")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("commitment"),
        "{}",
        stderr(&rejected)
    );
    assert!(
        !stderr(&rejected).contains("AmdWave"),
        "{}",
        stderr(&rejected)
    );
}

#[test]
fn application_handoff_close_range_blocks_unrelated_inheritable_descriptors() {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    const PROBE_FD: i32 = 199;
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));

    let report = directory.0.join("close-range-report.json");
    let source = File::open("/dev/null").unwrap();
    let source_fd = source.as_raw_fd();
    let mut command = application_runner_command(&directory, application_fixture(), &report);
    command.env("RUNNER_FIXTURE_PROBE_FD", PROBE_FD.to_string());
    // SAFETY: this callback creates one intentionally inheritable descriptor in the runner child
    // before exec. The application pre-exec close_range must quarantine it atomically.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(source_fd, PROBE_FD) != PROBE_FD
                || libc::fcntl(PROBE_FD, libc::F_SETFD, 0) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let application = command.output().unwrap();
    assert!(application.status.success(), "{}", stderr(&application));
    let report: JsonValue = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["probe_fd_open"], false);
    assert_eq!(report["handoff"]["acknowledged"], true);
}

#[test]
fn public_ack_completion_does_not_replace_private_host_currentness_authority() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));

    let report = directory.0.join("public-ack-report.json");
    let completed = application_runner_command(&directory, application_fixture(), &report)
        .env("RUNNER_FIXTURE_PUBLIC_ACK_WITHOUT_REACQUIRE", "1")
        .output()
        .unwrap();
    assert!(completed.status.success(), "{}", stderr(&completed));
    let report: JsonValue = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["handoff"]["acknowledged"], true);
    assert_eq!(report["handoff"]["child_reacquired_currentness"], false);
    // Success remains grounded in the runner's private retained lease and its post-ACK
    // currentness check; the reproducible child bytes supply only protocol completion.
}

#[test]
fn application_seccomp_rejects_process_and_double_fork_setsid_escape() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));

    let report = directory.0.join("seccomp-process-report.json");
    let escape_marker = directory.0.join("double-fork-setsid-escaped");
    let application = application_fixture();
    let completed = application_runner_command(&directory, application, &report)
        .env("RUNNER_FIXTURE_SECCOMP_PROCESS_PROBE", "1")
        .env("RUNNER_FIXTURE_DESCENDANT_PID_FILE", &escape_marker)
        .output()
        .unwrap();
    assert!(completed.status.success(), "{}", stderr(&completed));
    let report: JsonValue = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["handoff"]["acknowledged"], true);
    for probe in [
        "fork",
        "vfork",
        "clone",
        "clone3",
        "unshare",
        "setns",
        "setsid",
        "io_uring",
        "double_fork_setsid",
    ] {
        assert_eq!(report["handoff"]["process_creation"][probe], "EPERM");
    }
    assert!(
        !escape_marker.exists(),
        "double-fork+setsid descendant escaped the seccomp profile"
    );
}

#[test]
fn repeated_required_builds_do_not_accumulate_capsules_or_temps() {
    let directory = TestDirectory::new();
    for seed in [0, 1, 2] {
        let fixture = required_alpha_zeta_publication_fixture_with_seed(&directory, seed);
        let _ = fs::remove_file(directory.0.join("spawned"));
        let result = run_wrapper_with_options(
            &directory,
            &fixture.config,
            "publish-valid",
            fixture.cov6,
            None,
        );
        assert!(result.status.success(), "seed {seed}: {}", stderr(&result));
        assert!(restart_records(&directory).is_empty(), "seed {seed}");
        assert!(envelope_input_residue(&directory).is_empty(), "seed {seed}");
    }
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn required_pending_marker_crash_fails_closed_without_capsule_residue() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let interrupted = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        Some("pending-marker"),
    );
    assert_eq!(
        interrupted.status.code(),
        Some(86),
        "{}",
        stderr(&interrupted)
    );
    assert_eq!(envelope_input_residue(&directory).len(), 1);
    fs::remove_file(directory.0.join("spawned")).unwrap();

    let recovered = run_wrapper_with_options(&directory, &fixture.config, "fail", true, None);
    assert!(!recovered.status.success());
    assert!(!directory.0.join("spawned").exists());
    assert!(restart_records(&directory).is_empty());
    assert!(envelope_input_residue(&directory).is_empty());
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn required_finished_crash_recovery_removes_capsule_residue() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let interrupted = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        Some("finished"),
    );
    assert_eq!(
        interrupted.status.code(),
        Some(86),
        "{}",
        stderr(&interrupted)
    );
    assert_eq!(envelope_input_residue(&directory).len(), 1);
    fs::remove_file(directory.0.join("spawned")).unwrap();
    fs::remove_file(directory.0.join("envelope-inputs.capsule")).unwrap();

    let recovered = run_wrapper_with_options(&directory, &fixture.config, "fail", true, None);
    assert!(recovered.status.success(), "{}", stderr(&recovered));
    assert!(!directory.0.join("spawned").exists());
    assert!(restart_records(&directory).is_empty());
    assert!(envelope_input_residue(&directory).is_empty());
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn required_cov6_fault_matrix_recovers_every_committed_boundary() {
    for point in [
        "pending-intent",
        "ready",
        "published",
        "envelope-published",
        "completed",
        "intent-cleared",
        "finished",
    ] {
        let directory = TestDirectory::new();
        let fixture = required_alpha_zeta_publication_fixture(&directory);
        let interrupted = run_wrapper_with_options(
            &directory,
            &fixture.config,
            "publish-valid",
            fixture.cov6,
            Some(point),
        );
        assert_eq!(
            interrupted.status.code(),
            Some(86),
            "{point}: {}",
            stderr(&interrupted)
        );
        fs::remove_file(directory.0.join("spawned")).unwrap();
        fs::remove_file(directory.0.join("envelope-inputs.capsule")).unwrap();

        let recovered =
            run_wrapper_with_options(&directory, &fixture.config, "fail", fixture.cov6, None);
        assert!(
            recovered.status.success(),
            "{point}: {}",
            stderr(&recovered)
        );
        assert!(
            !directory.0.join("spawned").exists(),
            "{point} unexpectedly spawned rustc"
        );
        assert_eq!(
            published_artifacts(&directory),
            [fixture.expected_publication],
            "{point}"
        );
        assert!(restart_records(&directory).is_empty(), "{point}");
        assert!(envelope_input_residue(&directory).is_empty(), "{point}");
        assert!(
            envelope_publication_temp_residue(&directory).is_empty(),
            "{point}"
        );
        let envelope_count = fs::read_dir(artifact_dir(&directory))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| {
                let name = name.to_string_lossy();
                name.starts_with(".fe2o3-worker-v2-load-envelope-v1-")
                    && name.ends_with(".envelope")
            })
            .count();
        assert_eq!(envelope_count, 1, "{point}");
    }
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn required_cov6_production_wrapper_recovers_after_published_crash() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let interrupted = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        Some("published"),
    );
    assert_eq!(
        interrupted.status.code(),
        Some(86),
        "{}",
        stderr(&interrupted)
    );
    fs::remove_file(directory.0.join("spawned")).unwrap();
    fs::remove_file(directory.0.join("envelope-inputs.capsule")).unwrap();

    let recovered =
        run_wrapper_with_options(&directory, &fixture.config, "fail", fixture.cov6, None);
    assert!(recovered.status.success(), "{}", stderr(&recovered));
    assert!(!directory.0.join("spawned").exists());
    assert_eq!(
        published_artifacts(&directory),
        [fixture.expected_publication]
    );
    assert!(restart_records(&directory).is_empty());
    let report = directory.0.join("recovered-application-handoff.json");
    let application = run_application_fixture(&directory, &report);
    assert!(application.status.success(), "{}", stderr(&application));
    let report: JsonValue = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["handoff"]["acknowledged"], true);
    assert_eq!(report["handoff"]["child_reacquired_currentness"], true);
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn repeated_required_envelope_temp_crashes_are_bounded_and_recover() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);

    for cycle in 1..=3 {
        let interrupted = run_wrapper_with_options(
            &directory,
            &fixture.config,
            "publish-valid",
            fixture.cov6,
            Some("envelope-temp-synced"),
        );
        assert_eq!(
            interrupted.status.code(),
            Some(86),
            "crash cycle {cycle}: {}",
            stderr(&interrupted)
        );
        assert_eq!(
            envelope_publication_temp_residue(&directory).len(),
            1,
            "crash cycle {cycle} accumulated publication temps"
        );
        if cycle == 1 {
            fs::remove_file(directory.0.join("spawned")).unwrap();
            fs::remove_file(directory.0.join("envelope-inputs.capsule")).unwrap();
        } else {
            assert!(
                !directory.0.join("spawned").exists(),
                "crash cycle {cycle} unexpectedly spawned rustc"
            );
        }
    }

    let recovered =
        run_wrapper_with_options(&directory, &fixture.config, "fail", fixture.cov6, None);
    assert!(recovered.status.success(), "{}", stderr(&recovered));
    assert!(!directory.0.join("spawned").exists());
    assert!(envelope_publication_temp_residue(&directory).is_empty());
    assert!(envelope_input_residue(&directory).is_empty());
    assert!(restart_records(&directory).is_empty());
    assert_eq!(
        published_artifacts(&directory),
        [fixture.expected_publication]
    );
}

#[test]
fn application_handoff_rejects_child_protocol_substitution_and_omission() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));

    for (probe, value) in [
        ("RUNNER_FIXTURE_REUSE_HANDOFF_FD", "1"),
        ("RUNNER_FIXTURE_REUSE_ARTIFACT_DIR_FD", "1"),
        ("RUNNER_FIXTURE_SUBSTITUTE_COMMITMENT", "1"),
        ("RUNNER_FIXTURE_IGNORE_HANDOFF", "1"),
        ("RUNNER_FIXTURE_PREMATURE_CLOSE_ACK", "1"),
        ("RUNNER_FIXTURE_EXTRA_ACK_BYTE", "1"),
    ] {
        let report = directory.0.join(format!("rejected-{probe}.json"));
        let rejected = application_runner_command(&directory, application_fixture(), &report)
            .env(probe, value)
            .output()
            .unwrap();
        assert!(
            !rejected.status.success(),
            "{probe} unexpectedly passed: {}",
            stderr(&rejected)
        );
    }

    let absent_report = directory.0.join("absent-child-protocol.json");
    let absent = application_runner_command(
        &directory,
        no_protocol_application_fixture(),
        &absent_report,
    )
    .output()
    .unwrap();
    assert!(
        !absent.status.success(),
        "child without protocol support passed"
    );
    assert!(
        stderr(&absent).contains("acknowledgment"),
        "{}",
        stderr(&absent)
    );
}

#[test]
fn application_handoff_rejects_symlink_and_generation_path_replacement() {
    use std::os::unix::fs::symlink;

    let symlink_directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&symlink_directory);
    let published = run_wrapper_with_options(
        &symlink_directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));
    stop_outer_harness(&symlink_directory);
    let envelope = envelope_paths(&symlink_directory).pop().unwrap();
    let saved = envelope.with_extension("saved");
    fs::rename(&envelope, &saved).unwrap();
    symlink(saved.file_name().unwrap(), &envelope).unwrap();
    let symlinked = run_application_fixture(
        &symlink_directory,
        &symlink_directory.0.join("symlinked-report.json"),
    );
    assert!(!symlinked.status.success(), "symlinked envelope passed");

    let replaced_directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&replaced_directory);
    let published = run_wrapper_with_options(
        &replaced_directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));
    stop_outer_harness(&replaced_directory);
    let report = replaced_directory.0.join("replaced-generation-report.json");
    let mut command =
        application_runner_command(&replaced_directory, application_fixture(), &report);
    let original = artifact_dir(&replaced_directory);
    let moved = replaced_directory.0.join("original-fe2o3");
    fs::rename(&original, &moved).unwrap();
    fs::create_dir(&original).unwrap();
    let replaced = command.output().unwrap();
    assert!(
        !replaced.status.success(),
        "replaced generation directory passed"
    );
    assert!(
        stderr(&replaced).contains("identity was substituted"),
        "{}",
        stderr(&replaced)
    );
}

#[test]
fn application_handoff_rejects_truncated_and_extended_envelopes() {
    let directory = TestDirectory::new();
    let fixture = required_alpha_zeta_publication_fixture(&directory);
    let published = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));
    let envelope = envelope_paths(&directory).pop().unwrap();
    let exact = fs::read(&envelope).unwrap();

    fs::write(&envelope, &exact[..exact.len() - 1]).unwrap();
    let truncated = run_application_fixture(&directory, &directory.0.join("truncated.json"));
    assert!(!truncated.status.success(), "truncated envelope passed");

    let mut extended = exact;
    extended.push(0);
    fs::write(&envelope, extended).unwrap();
    let extra = run_application_fixture(&directory, &directory.0.join("extended.json"));
    assert!(!extra.status.success(), "extended envelope passed");
}

#[test]
fn application_handoff_rejects_stale_envelope_after_publication_turnover() {
    let directory = TestDirectory::new();
    let first = required_alpha_zeta_publication_fixture_with_seed(&directory, 0);
    let published =
        run_wrapper_with_options(&directory, &first.config, "publish-valid", first.cov6, None);
    assert!(published.status.success(), "{}", stderr(&published));
    let old = envelope_paths(&directory);
    assert_eq!(old.len(), 1);
    let old_bytes = fs::read(&old[0]).unwrap();

    fs::remove_file(directory.0.join("spawned")).unwrap();
    let second = required_alpha_zeta_publication_fixture_with_seed(&directory, 0);
    let mut second_config: JsonValue =
        serde_json::from_slice(&fs::read(&second.config).unwrap()).unwrap();
    second_config["limits"]["timeout_ms"] = json!(2001);
    fs::write(&second.config, serde_json::to_vec(&second_config).unwrap()).unwrap();
    let published = run_wrapper_with_options(
        &directory,
        &second.config,
        "publish-valid",
        second.cov6,
        None,
    );
    assert!(published.status.success(), "{}", stderr(&published));
    let all = envelope_paths(&directory);
    assert!(
        all.iter().any(|candidate| !old.contains(candidate)),
        "second publication must have a distinct envelope"
    );
    let current_report = directory.0.join("current-after-turnover.json");
    let current_run = run_application_fixture(&directory, &current_report);
    assert!(current_run.status.success(), "{}", stderr(&current_run));
    let report: JsonValue = serde_json::from_slice(&fs::read(current_report).unwrap()).unwrap();
    let admitted_identity = report["handoff"]["envelope_identity"].as_str().unwrap();
    let current = all
        .iter()
        .find(|candidate| {
            let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&fs::read(candidate).unwrap())
                .expect("decode test envelope");
            hex(&envelope.identity().as_bytes()) == admitted_identity
        })
        .expect("locate the envelope admitted after turnover");
    stop_outer_harness(&directory);
    fs::remove_file(current).unwrap();
    fs::write(&old[0], old_bytes).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&old[0], fs::Permissions::from_mode(0o600)).unwrap();

    let stale_report = directory.0.join("stale.json");
    let stale = run_application_fixture(&directory, &stale_report);
    assert!(!stale.status.success(), "stale envelope passed");
    assert!(
        stderr(&stale).contains("none is current"),
        "{}",
        stderr(&stale)
    );
}

#[test]
#[cfg(not(feature = "worker-v2-fault-injection-test-only"))]
fn production_build_does_not_expose_the_fault_injection_environment_switch() {
    let directory = TestDirectory::new();
    let fixture = publication_fixture(&directory, true);
    let result = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        Some("completed"),
    );
    assert!(result.status.success(), "{}", stderr(&result));
    assert_eq!(
        published_artifacts(&directory),
        [fixture.expected_publication]
    );
    assert!(restart_records(&directory).is_empty());
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn raw_and_ordinary_finalized_fault_matrix_recovers_every_durable_boundary() {
    const RECOVERABLE: &[&str] = &[
        "pending-intent",
        "ready",
        "published",
        "completed",
        "intent-cleared",
        "finished",
    ];

    for cov6 in [false, true] {
        for point in std::iter::once("pending-marker").chain(RECOVERABLE.iter().copied()) {
            let directory = TestDirectory::new();
            let fixture = publication_fixture(&directory, cov6);
            let interrupted = run_wrapper_with_options(
                &directory,
                &fixture.config,
                "publish-valid",
                fixture.cov6,
                Some(point),
            );
            assert_eq!(
                interrupted.status.code(),
                Some(86),
                "{point} cov6={cov6}: {}",
                stderr(&interrupted)
            );
            fs::remove_file(directory.0.join("spawned")).unwrap();

            let recovered =
                run_wrapper_with_options(&directory, &fixture.config, "fail", fixture.cov6, None);
            assert!(
                !directory.0.join("spawned").exists(),
                "{point} cov6={cov6} unexpectedly spawned rustc"
            );
            if point == "pending-marker" {
                assert!(
                    !recovered.status.success(),
                    "marker-only state must fail closed for cov6={cov6}"
                );
                assert!(published_artifacts(&directory).is_empty());
            } else {
                assert!(
                    recovered.status.success(),
                    "{point} cov6={cov6}: {}",
                    stderr(&recovered)
                );
                assert_eq!(
                    published_artifacts(&directory),
                    [fixture.expected_publication],
                    "{point} cov6={cov6}"
                );
            }
            assert!(
                restart_records(&directory).is_empty(),
                "{point} cov6={cov6} left restart records"
            );
        }
    }
}

#[test]
#[cfg(feature = "worker-v2-fault-injection-test-only")]
fn required_fault_injection_cannot_bypass_missing_capsule_configuration() {
    let directory = TestDirectory::new();
    let fixture = publication_fixture_with_envelope(&directory, true, true);
    let rejected = run_wrapper_with_options(
        &directory,
        &fixture.config,
        "publish-valid",
        fixture.cov6,
        Some("published"),
    );
    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("requires load_envelope_inputs"));
    assert!(published_artifacts(&directory).is_empty());
    assert!(restart_records(&directory).is_empty());
}

#[test]
fn ready_intent_resumes_in_a_new_process_without_spawning_rustc() {
    let directory = TestDirectory::new();
    let (config, artifact_dir) = stage_ready_restart(&directory);
    let recovered = run_wrapper(&directory, Some(&config), "fail");
    assert!(recovered.status.success(), "{}", stderr(&recovered));
    assert!(
        !directory.0.join("spawned").exists(),
        "recovery unexpectedly spawned rustc"
    );
    let entries = fs::read_dir(&artifact_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|path| {
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".fe2o3-link-artifact-v1-")
            && fs::read(path).unwrap() == b"restart-recovered-inert-worker-v2-output"
    }));
    assert!(entries.iter().all(|path| {
        let name = path.file_name().unwrap().to_string_lossy();
        !name.ends_with(".record")
            || (!name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                && !name.starts_with(".fe2o3-cargo-worker-v2-resume-v1-"))
    }));
}

#[test]
fn changed_worker_configuration_rejects_ready_intent_without_spawning_rustc() {
    let directory = TestDirectory::new();
    let (config, artifact_dir) = stage_ready_restart(&directory);
    let staged_artifacts = directory.0.join("staged-artifacts");
    snapshot_artifacts(&artifact_dir, &staged_artifacts);
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["candidate_output_max_bytes"] = JsonValue::from(4096);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let mut options = VerticalRunOptions::new("fail");
    options.restore = Some(&staged_artifacts);
    let rejected = run_wrapper_options(&directory, Some(&config), options);
    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("different build session or invocation"),
        "{}",
        stderr(&rejected)
    );
    assert!(!directory.0.join("spawned").exists());
    assert!(fs::read_dir(&artifact_dir).unwrap().any(|entry| {
        let name = entry.unwrap().file_name();
        name.to_string_lossy()
            .starts_with(".fe2o3-cargo-worker-v2-resume-v1-")
            && name.to_string_lossy().ends_with(".record")
    }));
}

#[test]
fn invalid_worker_output_fails_independent_hsaco_inspection_without_publication() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, true);
    let output = run_wrapper(&directory, Some(&config), "publish");

    assert!(!output.status.success());
    assert!(directory.0.join("spawned").exists());
    let stderr = stderr(&output);
    assert!(
        stderr.contains("independent Worker V2 HSACO inspection failed")
            && stderr.contains("invalid ELF"),
        "{stderr}"
    );
    assert!(!stderr.contains("without an authorized device backend"));
    let artifact_dir = artifact_dir(&directory);
    assert!(fs::read_dir(&artifact_dir).unwrap().all(|entry| {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy();
        !name.starts_with(".fe2o3-link-artifact-v1-")
            && !name.starts_with(".fe2o3-link-publication-v1-")
    }));

    fs::remove_file(directory.0.join("spawned")).unwrap();
    let retry = run_wrapper(&directory, Some(&config), "publish");
    assert!(!retry.status.success());
    assert!(
        !directory.0.join("spawned").exists(),
        "an admission-rejected attempt must remain terminal"
    );
}

#[test]
fn missing_handoff_fails_and_makes_the_attempt_terminal() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, true);
    let first = run_wrapper(&directory, Some(&config), "no-handoff");
    assert!(!first.status.success());
    assert!(
        stderr(&first).contains("compiler-module handoff consumption failed"),
        "{}",
        stderr(&first)
    );

    fs::remove_file(directory.0.join("spawned")).unwrap();
    let retry = run_wrapper(&directory, Some(&config), "publish");
    assert!(!retry.status.success());
    assert!(
        !directory.0.join("spawned").exists(),
        "a failed exact attempt must not respawn rustc"
    );
}

#[test]
fn worker_mismatch_invalidates_the_attempt_before_completion() {
    let directory = TestDirectory::new();
    let config = write_config(&directory, true);
    let output = run_wrapper(&directory, Some(&config), "publish-mismatch");

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("Worker V2 output bytes differ"), "{stderr}");
    assert!(!stderr.contains("invalidation also failed"), "{stderr}");
}

#[test]
fn missing_or_mismeasured_configuration_prevents_rustc_spawn() {
    let missing_directory = TestDirectory::new();
    let missing = run_wrapper(&missing_directory, None, "publish");
    assert!(!missing.status.success());
    assert!(!missing_directory.0.join("spawned").exists());
    assert!(
        stderr(&missing).contains("requires FE2O3_WORKER_V2_CONFIG_V2"),
        "{}",
        stderr(&missing)
    );

    let mismatched_directory = TestDirectory::new();
    let config = write_config(&mismatched_directory, true);
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["worker"]["sha256"] = JsonValue::String("00".repeat(32));
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    let mismatched = run_wrapper(&mismatched_directory, Some(&config), "publish");
    assert!(!mismatched.status.success());
    assert!(!mismatched_directory.0.join("spawned").exists());
    assert!(
        stderr(&mismatched).contains("Worker V2 setup failed"),
        "{}",
        stderr(&mismatched)
    );
}

#[test]
fn s09_environment_stripping_cannot_downgrade_the_prepared_broker() {
    let directory = TestDirectory::new();
    let config = write_s09_config(&directory);
    let mut options = VerticalRunOptions::new("publish");
    options.strip =
        Some("FE2O3_CODEGEN_PIPELINE,FE2O3_WORKER_V2_CONFIG_V2,FE2O3_WORKER_V2_EXPECTED_ID_V1");
    let output = run_wrapper_options(&directory, Some(&config), options);
    assert!(!output.status.success());
    assert!(!directory.0.join("spawned").exists());
    assert!(
        stderr(&output).contains("does not match the prepared profile/config identity"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn s09_requires_the_outer_prepared_worker_identity() {
    let directory = TestDirectory::new();
    let config = write_s09_config(&directory);
    let mut options = VerticalRunOptions::new("publish");
    options.strip = Some("FE2O3_WORKER_V2_EXPECTED_ID_V1");
    let output = run_wrapper_options(&directory, Some(&config), options);
    assert!(!output.status.success());
    assert!(!directory.0.join("spawned").exists());
    assert!(
        stderr(&output).contains("S09 Worker V2 configuration requires"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn unselected_host_units_run_without_attempts_but_device_production_still_fails_closed() {
    let host_directory = TestDirectory::new();
    let host_config = write_config(&host_directory, false);
    let host = run_wrapper(&host_directory, Some(&host_config), "no-handoff");
    assert!(host.status.success(), "{}", stderr(&host));
    assert_eq!(
        fs::read_to_string(host_directory.0.join("spawned")).unwrap(),
        "no-attempt"
    );

    let device_directory = TestDirectory::new();
    let device_config = write_config(&device_directory, false);
    let device = run_wrapper(
        &device_directory,
        Some(&device_config),
        "device-requires-attempt",
    );
    assert_eq!(device.status.code(), Some(42));
    assert!(
        stderr(&device).contains("rejected a missing managed attempt"),
        "{}",
        stderr(&device)
    );
}
