use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV1, RustcInvocationCapabilityV1,
};
use fe2o3_compiler_execution_client::PendingCompilerExecutionChildChannelV1;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationV1,
    CompilerExecutionServicePublishDispositionV1, CompilerExecutionServiceRequestKindV1,
    CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseV1,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
};
use fe2o3_process_identity::{
    CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2, EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
};
use sha2::{Digest as _, Sha256};

const ARTIFACT_FD: RawFd = fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1;
const BACKEND_FD: RawFd = fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_CHILD_FD_V1;
const BACKEND_PATH: &str = fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1;
const ARTIFACT_PATH: &str = fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_PATH_V1;
const TARGET: &str = "gfx942:xnack-";

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

pub struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    pub fn new(label: &str) -> Self {
        let target = workspace().join("target/issue272-qualification");
        fs::create_dir_all(&target).expect("create shared qualification root");
        let path = target.join(format!(
            "{label}-{}-{}",
            std::process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create ownership qualification directory");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("secure ownership qualification directory");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub struct ProtectedCompileResult {
    pub output: Output,
    pub host_object: PathBuf,
    pub artifact_directory: PathBuf,
    _scratch: ScratchDirectory,
}

pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .expect("test manifest must be inside the fe2o3 checkout")
        .to_path_buf()
}

pub fn fixture_source() -> PathBuf {
    workspace()
        .join("crates/rustc-codegen-fe2o3/tests/fixtures/single-codegen-ownership-v1/src/lib.rs")
        .canonicalize()
        .expect("canonical issue 272 fixture")
}

pub fn forged_source() -> PathBuf {
    workspace()
        .join("crates/rustc-codegen-fe2o3/tests/fixtures/single-codegen-ownership-v1/forged-prefix.rs")
        .canonicalize()
        .expect("canonical forged fixture")
}

pub fn loaded_backend() -> PathBuf {
    if let Some(path) = std::env::var_os("FE2O3_ISSUE272_BACKEND") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(Path::parent)
        .expect("Cargo profile directory");
    let backend = profile.join("librustc_codegen_fe2o3.so");
    assert!(
        backend.is_file(),
        "set FE2O3_ISSUE272_BACKEND to the built backend; inferred path is {}",
        backend.display(),
    );
    backend
}

pub fn rustc_path() -> PathBuf {
    let output = Command::new("rustup")
        .args(["which", "rustc"])
        .output()
        .expect("locate the pinned rustc");
    assert!(output.status.success(), "rustup which rustc failed");
    let path = String::from_utf8(output.stdout)
        .expect("rustc path is UTF-8")
        .trim()
        .to_owned();
    PathBuf::from(path)
        .canonicalize()
        .expect("canonical pinned rustc")
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).expect("read measured file")).into()
}

fn newest_dependency(prefix: &str) -> PathBuf {
    let dependencies = workspace().join("target/debug/deps");
    fs::read_dir(&dependencies)
        .expect("read shared dependency artifacts")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".rlib"))
        })
        .max_by_key(|entry| {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .map(|entry| entry.path())
        .unwrap_or_else(|| panic!("shared target has no dependency matching {prefix}*.rlib"))
}

fn compiler_closure(rustc: &Path, backend: &Path) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [0x31; 32],
        [0x32; 32],
        [0x33; 32],
        sha256(rustc),
        [0x35; 32],
        sha256(backend),
    )
    .expect("construct qualification compiler closure")
}

fn policy(signing_key: &SigningKey) -> CompilerExecutionIssuerPolicyV1 {
    CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([0x41; 32], 1).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([0x42; 32], 1).unwrap(),
        signing_key.verifying_key().to_bytes(),
        SigningKey::from_bytes(&[0x44; 32])
            .verifying_key()
            .to_bytes(),
    )
    .expect("construct qualification compiler-execution policy")
}

fn build_attempt() -> String {
    format!("1:{}:{}", "51".repeat(16), "52".repeat(32))
}

fn compile_environment(
    rustc: &Path,
    closure: CompilerClosureV2,
) -> (CompileEnvironmentV2, BTreeMap<OsString, OsString>) {
    let rustc_library = rustc
        .parent()
        .and_then(Path::parent)
        .expect("rustc sysroot")
        .join("lib");
    let values = BTreeMap::from([
        (
            OsString::from("FE2O3_BUILD_ATTEMPT_V1"),
            OsString::from(build_attempt()),
        ),
        (
            OsString::from(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2),
            OsString::from(hex(closure.codegen_backend_sha256())),
        ),
        (
            OsString::from(EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1),
            OsString::from(hex(closure.identity_sha256())),
        ),
        (
            OsString::from("FE2O3_HSACO_DIR"),
            OsString::from(ARTIFACT_PATH),
        ),
        (OsString::from("FE2O3_TARGET"), OsString::from(TARGET)),
        (
            OsString::from("FE2O3_VERIFY_KERNEL_IR"),
            OsString::from("1"),
        ),
        (
            OsString::from("LD_LIBRARY_PATH"),
            rustc_library.into_os_string(),
        ),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("TZ"), OsString::from("UTC")),
    ]);
    let environment = CompileEnvironmentV2::from_child_environment(values.clone())
        .expect("construct exact rustc environment");
    (environment, values)
}

fn rustc_argv(source: &Path, host_object: &Path, features: &[&str]) -> Vec<String> {
    let rustc = rustc_path();
    let dependencies = workspace().join("target/debug/deps");
    let device = newest_dependency("libfe2o3_device-");
    let helper = newest_dependency("libissue272_device_helper-");
    let mut argv = vec![
        rustc.to_string_lossy().into_owned(),
        source.to_string_lossy().into_owned(),
        "--crate-name=fe2o3_issue272_attributed_fixture".to_owned(),
        "--crate-type=lib".to_owned(),
        "--edition=2024".to_owned(),
        "-Cpanic=abort".to_owned(),
        "-Copt-level=2".to_owned(),
        "--emit=obj".to_owned(),
        "-o".to_owned(),
        host_object.to_string_lossy().into_owned(),
        format!("-Ldependency={}", dependencies.display()),
        "--extern".to_owned(),
        format!("fe2o3_device={}", device.display()),
        "--extern".to_owned(),
        format!("issue272_device_helper={}", helper.display()),
    ];
    for feature in features {
        argv.push("--cfg".to_owned());
        argv.push(format!("feature={feature:?}"));
    }
    argv.push(format!("-Zcodegen-backend={BACKEND_PATH}"));
    argv
}

fn inherit_file_at(command: &mut Command, file: &File, child_fd: RawFd) {
    let source_fd = file.as_raw_fd();
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(source_fd, child_fd) != child_fd {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn receive_packet(peer: RawFd) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0_u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1];
    let received = unsafe { libc::recv(peer, bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    if received < 0 {
        return Err(io::Error::last_os_error());
    }
    bytes.truncate(received as usize);
    Ok(bytes)
}

fn send_packet(peer: RawFd, bytes: &[u8]) -> io::Result<()> {
    let sent = unsafe { libc::send(peer, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) };
    if sent == bytes.len() as isize {
        Ok(())
    } else if sent < 0 {
        Err(io::Error::last_os_error())
    } else {
        Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "partial seqpacket send",
        ))
    }
}

fn run_service(peer: File, policy: CompilerExecutionIssuerPolicyV1, key: SigningKey) -> bool {
    let mut subject = None;
    let mut publication = None;
    loop {
        let packet = match receive_packet(peer.as_raw_fd()) {
            Ok(packet) if !packet.is_empty() => packet,
            Ok(_) => return false,
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => return false,
            Err(error) => panic!("qualification service receive failed: {error}"),
        };
        let request = CompilerExecutionServiceRequestV1::decode(&packet)
            .expect("qualification service received canonical request");
        assert_eq!(request.policy_identity(), policy.identity());
        let response = match request.kind() {
            CompilerExecutionServiceRequestKindV1::Recover => {
                subject = request.subject().cloned();
                CompilerExecutionServiceResponseV1::receipt_absent(
                    request.identity(),
                    &policy,
                    1,
                    [0; 32],
                )
                .unwrap()
            }
            CompilerExecutionServiceRequestKindV1::Inspect => {
                CompilerExecutionServiceResponseV1::ready(request.identity(), &policy, 1, [0; 32])
                    .unwrap()
            }
            CompilerExecutionServiceRequestKindV1::Prepare => {
                let challenge = CompilerExecutionAttestationChallengeV1::new(
                    &policy,
                    subject.as_ref().expect("recover precedes prepare"),
                    [0x61; 32],
                    1,
                    [0; 32],
                )
                .unwrap();
                CompilerExecutionServiceResponseV1::prepared(request.identity(), &policy, challenge)
                    .unwrap()
            }
            CompilerExecutionServiceRequestKindV1::Issue => {
                let receipt = CompilerExecutionAttestationReceiptV1::issue(
                    &policy,
                    request.request().expect("issue carries request"),
                    &key,
                )
                .unwrap();
                let issued =
                    CompilerExecutionReceiptPublicationV1::new([0x62; 32], [0x63; 32], receipt)
                        .unwrap();
                publication = Some(issued.clone());
                CompilerExecutionServiceResponseV1::issued(request.identity(), &policy, issued)
                    .unwrap()
            }
            CompilerExecutionServiceRequestKindV1::Publish => {
                let issued = publication.as_ref().expect("issue precedes publish");
                assert_eq!(request.publication(), Some(issued));
                let acknowledgment =
                    CompilerExecutionReceiptPublicationAckV1::new(issued, [0x64; 32]).unwrap();
                let response = CompilerExecutionServiceResponseV1::published(
                    request.identity(),
                    &policy,
                    acknowledgment,
                    CompilerExecutionServicePublishDispositionV1::Advanced,
                )
                .unwrap();
                send_packet(peer.as_raw_fd(), response.canonical_bytes()).unwrap();
                return true;
            }
            unexpected => panic!("qualification service received unexpected {unexpected:?}"),
        };
        send_packet(peer.as_raw_fd(), response.canonical_bytes()).unwrap();
    }
}

pub fn protected_compile(source: &Path, features: &[&str], label: &str) -> ProtectedCompileResult {
    let scratch = ScratchDirectory::new(label);
    let artifact_directory = scratch.path().join("artifacts");
    fs::create_dir(&artifact_directory).expect("create managed artifact directory");
    fs::set_permissions(&artifact_directory, fs::Permissions::from_mode(0o700))
        .expect("secure managed artifact directory");
    let host_object = scratch.path().join("host.o");
    let backend = loaded_backend().canonicalize().expect("canonical backend");
    let rustc = rustc_path();
    let closure = compiler_closure(&rustc, &backend);
    let signing_key = SigningKey::from_bytes(&[0x43; 32]);
    let policy = policy(&signing_key);
    let policy_capability = CompilerExecutionPolicyCapabilityV1::create(policy.clone())
        .expect("seal compiler-execution policy");
    let argv = rustc_argv(source, &host_object, features);
    let (environment, environment_values) = compile_environment(&rustc, closure);
    let unit = RustcUnitV2::new(workspace().to_string_lossy().into_owned(), argv.clone())
        .expect("construct exact rustc unit");
    let descriptor =
        RustcInvocationDescriptorV2::new(sha256(&rustc), sha256(&backend), unit, environment)
            .and_then(|v2| RustcInvocationDescriptorV3::new(v2, closure))
            .expect("construct exact V3 rustc invocation");
    let invocation_capability =
        RustcInvocationCapabilityV1::create(descriptor).expect("seal exact rustc invocation");

    let artifact = File::open(&artifact_directory).expect("open artifact directory capability");
    let backend_file = File::options()
        .read(true)
        .custom_flags(libc::O_CLOEXEC)
        .open(&backend)
        .expect("open backend capability");
    let mut command = Command::new(&rustc);
    command
        .current_dir(workspace())
        .args(&argv[1..])
        .env_clear()
        .envs(environment_values)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    inherit_file_at(&mut command, &artifact, ARTIFACT_FD);
    inherit_file_at(&mut command, &backend_file, BACKEND_FD);
    invocation_capability
        .inherit_for_child(&mut command)
        .expect("inherit invocation capability at fd 199");
    policy_capability
        .inherit_for_child(&mut command)
        .expect("inherit policy capability at fd 202");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command)
        .expect("prepare child-created compiler service channel");
    let child = command.spawn().expect("spawn attributed rustc");
    let launch = pending
        .finish(child.id(), Duration::from_secs(10))
        .expect("receive exact rustc service endpoint");
    let (service_peer, client_pidfd) = launch.into_test_descriptors();
    let service_peer = File::from(service_peer);
    let service = thread::spawn(move || {
        let _client_pidfd = client_pidfd;
        run_service(service_peer, policy, signing_key)
    });
    let output = child
        .wait_with_output()
        .expect("wait for attributed rustc fixture");
    let published = service.join().expect("qualification service thread");
    if output.status.success() {
        assert!(
            published,
            "successful backend run did not publish a signed receipt"
        );
    }

    ProtectedCompileResult {
        output,
        host_object,
        artifact_directory,
        _scratch: scratch,
    }
}

pub fn require_failure_with(output: &Output, diagnostic: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "rustc unexpectedly succeeded");
    assert!(
        stderr.contains(diagnostic),
        "rustc omitted {diagnostic:?}:\n{stderr}"
    );
}

pub fn recursively_named(root: &Path, name: &OsStr) -> Vec<PathBuf> {
    fn visit(path: &Path, name: &OsStr, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).expect("read artifact directory") {
            let entry = entry.expect("read artifact entry");
            let path = entry.path();
            if path.is_dir() {
                visit(&path, name, found);
            } else if path.file_name() == Some(name) {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    visit(root, name, &mut found);
    found
}
