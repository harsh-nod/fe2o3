#![cfg(target_os = "linux")]

use std::fs::{self, File};
use std::io::{self, IoSlice, Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};

use fe2o3_hsaco_finalize::finalize_unfinalized;

#[allow(dead_code)]
#[path = "../src/worker_v2_artifact_container_test_fixture.rs"]
mod alpha_zeta_support;

include!("../../fe2o3-hsaco-finalize/tests/fixtures/worker_v2_hsaco_test_support.rs");

const WORKER_ID: &str = "cargo-fe2o3-fixture-worker-v1";
const CAPABILITY_BROKER_ENV: &str = "FE2O3_CAPABILITY_BROKER_V1";
const REQUEST_MAGIC: &[u8] = b"FE2O3-CARGO-CAPABILITY-BROKER-V1\0";
const BUILD_SESSION_BYTES: [u8; 16] = [0x11; 16];
static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

struct TestCapabilityBroker {
    endpoint: String,
    stop: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<thread::JoinHandle<io::Result<()>>>,
}

impl TestCapabilityBroker {
    fn start(artifact_dir: &Path) -> io::Result<Self> {
        let endpoint = random_broker_endpoint()?;
        let address =
            SocketAddr::from_abstract_name(format!("fe2o3-cap-v1-{endpoint}").as_bytes())?;
        let listener = UnixListener::bind_addr(&address)?;
        listener.set_nonblocking(true)?;
        let backend = sealed_test_backend()?;
        let artifact = File::from(
            rustix::fs::open(
                artifact_dir,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(io::Error::from)?,
        );
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                        let mut request = vec![0_u8; REQUEST_MAGIC.len() + 16];
                        stream.read_exact(&mut request)?;
                        let mut expected = REQUEST_MAGIC.to_vec();
                        expected.extend_from_slice(&BUILD_SESSION_BYTES);
                        if request != expected {
                            return Err(io::Error::new(
                                io::ErrorKind::PermissionDenied,
                                "test broker request did not match the build session",
                            ));
                        }
                        let descriptors = [backend.as_fd(), artifact.as_fd()];
                        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
                        let mut ancillary = SendAncillaryBuffer::new(&mut space);
                        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
                        let sent = sendmsg(
                            &stream,
                            &[IoSlice::new(&[1])],
                            &mut ancillary,
                            SendFlags::NOSIGNAL,
                        )
                        .map_err(io::Error::from)?;
                        if sent != 1 {
                            return Err(io::Error::new(
                                io::ErrorKind::WriteZero,
                                "test broker response was truncated",
                            ));
                        }
                        return Ok(());
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        });
        Ok(Self {
            endpoint,
            stop,
            worker: Some(worker),
        })
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl Drop for TestCapabilityBroker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("join test capability broker").unwrap();
        }
    }
}

fn random_broker_endpoint() -> io::Result<String> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex(&bytes))
}

fn sealed_test_backend() -> io::Result<File> {
    let descriptor = rustix::fs::memfd_create(
        "cargo-fe2o3-worker-v2-test-backend",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(io::Error::from)?;
    let mut writable = File::from(descriptor);
    writable.write_all(b"worker-v2 test backend")?;
    rustix::fs::fcntl_add_seals(
        &writable,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
    )
    .map_err(io::Error::from)?;
    rustix::fs::fcntl_add_seals(&writable, SealFlags::SEAL).map_err(io::Error::from)?;
    let path = PathBuf::from(format!("/proc/self/fd/{}", writable.as_raw_fd()));
    let read_only = File::from(
        rustix::fs::open(&path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
            .map_err(io::Error::from)?,
    );
    drop(writable);
    Ok(read_only)
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-worker-v2-flow-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn worker_fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-worker-v2-fixture"))
}

fn envelope_input_fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cargo-fe2o3-envelope-input-fixture"))
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

fn run_wrapper(directory: &TestDirectory, config: Option<&Path>, rustc_mode: &str) -> Output {
    let (mut command, _broker) = wrapper_command(directory, config, rustc_mode);
    command.output().unwrap()
}

fn run_wrapper_with_options(
    directory: &TestDirectory,
    config: &Path,
    rustc_mode: &str,
    cov6: bool,
    fault: Option<&str>,
) -> Output {
    let (mut command, _broker) =
        wrapper_command_with_options(directory, Some(config), rustc_mode, cov6, fault);
    command.output().unwrap()
}

fn wrapper_command(
    directory: &TestDirectory,
    config: Option<&Path>,
    rustc_mode: &str,
) -> (Command, TestCapabilityBroker) {
    wrapper_command_with_options(directory, config, rustc_mode, false, None)
}

fn wrapper_command_with_options(
    directory: &TestDirectory,
    config: Option<&Path>,
    rustc_mode: &str,
    cov6: bool,
    fault: Option<&str>,
) -> (Command, TestCapabilityBroker) {
    let source = directory.0.join("workflow_fixture.rs");
    fs::write(&source, "fn main() {}\n").unwrap();
    let artifact_dir = directory.0.join("artifacts");
    fs::create_dir_all(&artifact_dir).unwrap();
    let broker = TestCapabilityBroker::start(&artifact_dir).unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .env_clear()
        .current_dir(&directory.0)
        .env("FE2O3_BINDING_WRAPPER_MODE_V1", "1")
        .env(
            "FE2O3_MANAGED_RUSTC_ARGS_V1",
            "-Zcodegen-backend=/proc/./self/fd/198\x1f-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"11111111111111111111111111111111\"",
        )
        .env("FE2O3_BUILD_SESSION_V1", "11".repeat(16))
        .env(CAPABILITY_BROKER_ENV, broker.endpoint())
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_FIXTURE_RUSTC_MARKER", directory.0.join("spawned"))
        .env("FE2O3_FIXTURE_RUSTC_MODE", rustc_mode)
        .env("FE2O3_FIXTURE_SOURCE", &source)
        .env("FE2O3_HSACO_DIR", artifact_dir)
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .arg(worker_fixture())
        .args(["--crate-name", "workflow_fixture"])
        .arg(&source)
        .arg("-Cmetadata=worker-v2-test");
    if let Some(config) = config {
        command.env("FE2O3_WORKER_V2_CONFIG_V2", config);
    }
    if cov6 {
        command.env("FE2O3_TEST_WORKER_V2_COV6", "1");
    }
    if let Some(fault) = fault {
        command.env("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1", fault);
    }
    if directory.0.join("alpha-zeta-profile").exists() {
        command.env("FE2O3_TEST_WORKER_V2_ALPHA_ZETA", "1");
    }
    (command, broker)
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stage_ready_restart(directory: &TestDirectory) -> (PathBuf, PathBuf) {
    let config = write_config(directory, true);
    let handoff_marker = directory.0.join("handoff-ready");
    let (mut first_command, _broker) =
        wrapper_command(directory, Some(&config), "stop-after-handoff");
    first_command.env("FE2O3_FIXTURE_HANDOFF_MARKER", &handoff_marker);
    let mut first = first_command.spawn().unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let status = fs::read_to_string(format!("/proc/{}/status", first.id())).unwrap_or_default();
        if handoff_marker.exists() && status.lines().any(|line| line.starts_with("State:\tT")) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let status = fs::read_to_string(format!("/proc/{}/status", first.id())).unwrap();
    assert!(status.lines().any(|line| line.starts_with("State:\tT")));
    first.kill().unwrap();
    first.wait().unwrap();

    let attempt = fs::read_to_string(directory.0.join("spawned")).unwrap();
    let source = directory.0.join("workflow_fixture.rs");
    let artifact_dir = directory.0.join("artifacts");
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

fn published_artifacts(directory: &TestDirectory) -> Vec<Vec<u8>> {
    fs::read_dir(directory.0.join("artifacts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
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
    fs::read_dir(directory.0.join("artifacts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.ends_with(".record")
                && (name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                    || name.starts_with(".fe2o3-cargo-worker-v2-resume-v1-"))
        })
        .collect()
}

fn envelope_input_residue(directory: &TestDirectory) -> Vec<PathBuf> {
    fs::read_dir(directory.0.join("artifacts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fe2o3-worker-v2-envelope-inputs-v1-")
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

    let artifact_dir = directory.0.join("artifacts");
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
    let envelope_count = fs::read_dir(directory.0.join("artifacts"))
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
    let mut value: JsonValue = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["candidate_output_max_bytes"] = JsonValue::from(4096);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let rejected = run_wrapper(&directory, Some(&config), "fail");
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
    let artifact_dir = directory.0.join("artifacts");
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
