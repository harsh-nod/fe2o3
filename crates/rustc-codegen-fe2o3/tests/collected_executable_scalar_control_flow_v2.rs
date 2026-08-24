use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{
    DirBuilderExt as _, FileExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, CompilerModuleHandoffErrorV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerModuleHandoffV2,
    CompilerModuleSymbolRoleV1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CapabilityV1, OwnershipSemantics,
    PhysicalAbiComponentKind, ScalarTypeV1,
};
use reserved_fe2o3_symbols::{
    MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1, host_kernel_symbol_v1,
};
use sha2::{Digest as _, Sha256};

const PIPELINE: &str = "collected-executable-scalar-control-flow-v2";
const FIXTURE: &str = include_str!("fixtures/executable-scalar-control-flow-v1.rs");
const SCALAR_GEMM_PIPELINE: &str = "collected-scalar-gemm-v1";
const SCALAR_GEMM_FIXTURE: &str = include_str!("../../../examples/scalar_gemm_v1/src/kernel.rs");
const TILED_GEMM_PIPELINE: &str = "collected-tiled-gemm-v1";
const TILED_GEMM_FIXTURE: &str = include_str!("fixtures/collected-tiled-gemm-v1/src/lib.rs");
const TILED_GEMM_LDS_SLICE1_FIXTURE: &str =
    include_str!("../../../examples/tiled_gemm_v1/src/kernel.rs");
const ROW_SOFTMAX_PIPELINE: &str = "collected-row-softmax-v1";
const ROW_SOFTMAX_FIXTURE: &str = include_str!("../../../examples/row_softmax_v1/src/kernel.rs");
// Reviewed independently from the handoff identity and section payloads. This
// binds every byte of the canonical LLVM lowering before compiler-owned data.
const EXPECTED_ROW_LLVM_BODY_SHA256: [u8; 32] = [
    0xd4, 0x8d, 0x33, 0x20, 0xc2, 0x86, 0xc6, 0xda, 0x22, 0x53, 0xa1, 0x04, 0x38, 0x60, 0x89, 0xe3,
    0x89, 0x64, 0x8f, 0x42, 0x60, 0xf2, 0xe7, 0xef, 0xda, 0x21, 0x26, 0x9f, 0xef, 0x95, 0x1c, 0x2c,
];
const EXPECTED_ROW_PORTABLE_MIR_COMMITMENT: [u8; 32] = [
    0x93, 0x7a, 0xe7, 0x1f, 0xa9, 0x7c, 0x7e, 0x4a, 0x78, 0x2e, 0x5b, 0x27, 0xec, 0x80, 0xa1, 0x00,
    0x8d, 0xf8, 0x96, 0x6c, 0xd0, 0xd1, 0x28, 0xe4, 0xfd, 0x03, 0xbe, 0xd6, 0x6a, 0x1d, 0xc6, 0xf0,
];
const EXPECTED_ROW_COMPILER_SEMANTICS_COMMITMENT: [u8; 32] = [
    0x31, 0x32, 0xd8, 0x6d, 0x22, 0x9a, 0x39, 0x77, 0xed, 0x9c, 0x52, 0x83, 0xc2, 0x41, 0xc4, 0xf6,
    0xc8, 0x5a, 0xff, 0x23, 0xc1, 0xd1, 0x77, 0xfb, 0x0d, 0x23, 0xc0, 0x74, 0x32, 0x79, 0xf0, 0xa4,
];
const EXPECTED_ROW_CANONICAL_MODULE_COMMITMENT: [u8; 32] = [
    0x1e, 0x1b, 0x14, 0xc6, 0x84, 0x2f, 0xfd, 0x09, 0x10, 0x3e, 0xb5, 0x5e, 0xb3, 0x9b, 0x1b, 0xca,
    0xe9, 0xc0, 0xda, 0x81, 0x59, 0x7f, 0xed, 0x61, 0x86, 0x76, 0x75, 0x62, 0x33, 0x72, 0x30, 0xe6,
];
const EXPECTED_ROW_FN_ABI_COMMITMENT: [u8; 32] = [
    0x1f, 0x97, 0x82, 0x38, 0x8c, 0x98, 0x28, 0x56, 0x4b, 0xd6, 0x34, 0xce, 0x21, 0x8a, 0x6f, 0xf1,
    0x18, 0x65, 0xdb, 0xba, 0x8a, 0x52, 0x83, 0xf5, 0xa0, 0x26, 0x7b, 0x2b, 0x7a, 0x97, 0xa4, 0xc6,
];
const REVIEWED_ROW_METADATA: &str = "fe2o3-row-softmax-v1-reviewed";
const ROW_AUTHORITY_DOMAIN: &[u8] = b"fe2o3.row-softmax.collected-authority.v1";
const ROW_METADATA_DOMAIN: &[u8] = b"fe2o3.row-softmax.cargo-metadata-observation.v1";
const ROW_PROVIDER_DOMAIN: &[u8] = b"FE2O3/ROW-SOFTMAX-PROVIDER-AUTHORITY/V1\0";
const ROW_PROVIDER_SOURCE_DOMAIN: &[u8] = b"FE2O3/ROW-SOFTMAX-PROVIDER-SOURCE-IDENTITY/V1\0";
const CARGO_METADATA_TRANSCRIPT_DOMAIN: &[u8] = b"FE2O3/CARGO-METADATA-BUILD-OBSERVATION/V2\0";
const RUSTC_RUNTIME_IDENTITY_DOMAIN: &[u8] = b"fe2o3-rustc-executable-runtime-identity-v1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v1\0";
const ROW_PROVIDER_PATHS: [&[u8]; 8] = [
    b"fe2o3_device::DisjointSlice",
    b"fe2o3_device::ThreadIndex",
    b"fe2o3_device::thread::index_1d",
    b"fe2o3_device::ThreadIndex::get",
    b"fe2o3_device::DisjointSlice::<T>::get_mut_at",
    b"fe2o3_device::DeviceMath",
    b"fe2o3_device::DeviceMath::current",
    b"fe2o3_device::DeviceMath::exp_f32",
];
const ROW_ABI_DOMAIN: &[u8] = b"fe2o3.row-softmax.abi-binding.v1";
const ROW_LAUNCH_DOMAIN: &[u8] = b"fe2o3.row-softmax.launch-binding.v1";
const ROW_CORRESPONDENCE_DOMAIN: &[u8] = b"fe2o3.row-softmax.reviewed-correspondence.v1";
const ROW_ABI: &[u8] = b"ptr64;size=32;align=8;input@0:16:8:slice-f32:shared-readonly;output@16:16:8:slice-f32:exclusive-readwrite;lengths=exactly-64-by-host-precondition";
const ROW_LAUNCH: &[u8] =
    b"rank=1;block=exact(64,1,1);grid=exact(1,1,1);static-shared=0;dynamic-shared=0;wave=64;cov=6";
const ROW_CORRESPONDENCE: &[u8] = b"exact reviewed Rust portable-MIR identity selects the private fe2o3::row_softmax_v1 canonical module;one lane performs three ordered 64-element loops;bounded reviewed correspondence only;not a compiler-refinement proof";
const ROW_FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];
const HANDOFF_OBSERVATION_MAGIC: &[u8] = b"FE2O3-CARGO-WRAPPER-HANDOFF-OBSERVATION-V1\0";
const HANDOFF_OBSERVATION_DOMAIN: &[u8] = b"FE2O3/CARGO-WRAPPER-HANDOFF-OBSERVATION/V1\0";
const HANDOFF_OBSERVATION_PRODUCER_DOMAIN: &[u8] = b"FE2O3/CARGO-WRAPPER-HANDOFF-PRODUCER/V1\0";
const HANDOFF_OBSERVATION_AUTHORITY_NONE: &[u8] =
    b"inert-one-shot-compiler-handoff-test-observation-no-authority";
const HANDOFF_OBSERVATION_ACK_MAGIC: &[u8] = b"FE2O3-CARGO-WRAPPER-HANDOFF-OBSERVATION-ACK-V1\0";
const HANDOFF_OBSERVATION_DIRECTORY_ENV: &str =
    "FE2O3_COMPILER_HANDOFF_OBSERVATION_DIRECTORY_TEST_ONLY_V1";
const HANDOFF_OBSERVATION_CRATE_ENV: &str = "FE2O3_COMPILER_HANDOFF_OBSERVATION_CRATE_TEST_ONLY_V1";
const CARGO_METADATA_MUTATION_TEST_ONLY_ENV: &str = "FE2O3_CARGO_METADATA_MUTATION_TEST_ONLY_V1";
const BROKER_PATH_SUBSTITUTION_MARKER: &str = "fe2o3-test-hostile-broker-path-substitution";
const MAX_HANDOFF_OBSERVATION_BYTES: usize = 32 * 1024;
const BUILD_HELPER_ENV: &str = "FE2O3_SCALAR_CF_ISOLATED_BUILD_HELPER";
const BUILD_HELPER_SOCKET_ENV: &str = "FE2O3_SCALAR_CF_BUILD_SOCKET_FD";
const BUILD_HELPER_WORKSPACE_ENV: &str = "FE2O3_SCALAR_CF_BUILD_WORKSPACE";
const BUILD_HELPER_MOUNT_ENV: &str = "FE2O3_SCALAR_CF_BUILD_MOUNT";
const SCALAR_GEMM_HANDOFF_OUTPUT_ENV: &str = "FE2O3_SCALAR_GEMM_V1_HANDOFF_OUTPUT";
const BACKEND_BUILD_TIMEOUT: Duration = Duration::from_secs(600);
const COMPILER_TIMEOUT: Duration = Duration::from_secs(120);
const TERMINATION_GRACE: Duration = Duration::from_secs(2);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);
static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static BACKEND: OnceLock<PinnedBackend> = OnceLock::new();
static COLLECTION_BACKEND: OnceLock<PinnedBackend> = OnceLock::new();
static AUTHORITY_TOOLCHAIN: OnceLock<AuthorityToolchain> = OnceLock::new();
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();
static USER_MOUNT_NAMESPACE: OnceLock<Result<(), String>> = OnceLock::new();

const REQUIRED_MEMFD_SEALS: libc::c_int =
    libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;

struct PinnedBackend {
    file: File,
    len: usize,
    sha256: [u8; 32],
}

struct PinnedBrokerExecutable {
    file: File,
    device: u64,
    inode: u64,
    mode: u32,
    len: u64,
    sha256: [u8; 32],
}

struct AuthorityToolchain {
    cargo: PathBuf,
    cargo_sha256: [u8; 32],
    rustc: PathBuf,
    rustc_sha256: [u8; 32],
    rustc_runtime_sha256: [u8; 32],
}

impl PinnedBrokerExecutable {
    fn open(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|error| {
                format!("open built cargo-fe2o3 without following symlinks: {error}")
            })?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("inspect pinned cargo-fe2o3: {error}"))?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o111 == 0
            || metadata.mode() & 0o022 != 0
            || metadata.len() == 0
        {
            return Err(
                "built cargo-fe2o3 is not an owned non-writable executable regular file".to_owned(),
            );
        }
        let len = metadata.len();
        let sha256 = sha256_file_description(&file, len)?;
        Ok(Self {
            file,
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            len,
            sha256,
        })
    }

    fn command(&self) -> Result<Command, String> {
        self.verify()?;
        Ok(Command::new(format!(
            "/proc/self/fd/./{}",
            self.file.as_raw_fd()
        )))
    }

    fn verify(&self) -> Result<(), String> {
        let metadata = self
            .file
            .metadata()
            .map_err(|error| format!("inspect retained cargo-fe2o3: {error}"))?;
        if metadata.dev() != self.device
            || metadata.ino() != self.inode
            || metadata.mode() != self.mode
            || metadata.len() != self.len
        {
            return Err("retained cargo-fe2o3 object identity changed".to_owned());
        }
        if sha256_file_description(&self.file, self.len)? != self.sha256 {
            return Err("retained cargo-fe2o3 bytes changed".to_owned());
        }
        Ok(())
    }

    fn sha256(&self) -> Result<[u8; 32], String> {
        self.verify()?;
        Ok(self.sha256)
    }
}

fn sha256_file_description(file: &File, len: u64) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < len {
        let remaining = usize::try_from((len - offset).min(buffer.len() as u64))
            .expect("bounded broker digest chunk fits usize");
        let count = file
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|error| format!("read pinned cargo-fe2o3: {error}"))?;
        if count == 0 {
            return Err("pinned cargo-fe2o3 was truncated while hashing".to_owned());
        }
        digest.update(&buffer[..count]);
        offset = offset
            .checked_add(count as u64)
            .ok_or_else(|| "pinned cargo-fe2o3 digest offset overflow".to_owned())?;
    }
    Ok(digest.finalize().into())
}

impl PinnedBackend {
    fn load_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/./{}", self.file.as_raw_fd()))
    }

    fn verify(&self) -> Result<(), String> {
        let descriptor_flags = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GETFD) };
        if descriptor_flags < 0 || descriptor_flags & libc::FD_CLOEXEC == 0 {
            return Err(format!(
                "backend descriptor is not close-on-exec in the parent: {descriptor_flags:#x}"
            ));
        }
        let seals = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 || seals & REQUIRED_MEMFD_SEALS != REQUIRED_MEMFD_SEALS {
            return Err(format!("backend memfd lost required seals: {seals:#x}"));
        }
        let actual_len = usize::try_from(
            self.file
                .metadata()
                .map_err(|error| error.to_string())?
                .len(),
        )
        .map_err(|_| "backend length does not fit usize".to_owned())?;
        if actual_len != self.len {
            return Err(format!(
                "sealed backend length changed: expected {}, found {actual_len}",
                self.len
            ));
        }
        let mut bytes = vec![0_u8; self.len];
        self.file
            .read_exact_at(&mut bytes, 0)
            .map_err(|error| format!("read sealed backend: {error}"))?;
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        if actual != self.sha256 {
            return Err("sealed backend SHA-256 changed".to_owned());
        }
        Ok(())
    }
}

struct PrivateBuildRoot(PathBuf);

impl PrivateBuildRoot {
    fn new(workspace: &Path) -> Self {
        let parent = workspace.join("target/rustc-codegen-fe2o3-private-builds");
        std::fs::create_dir_all(&parent).expect("create private-build parent");
        for attempt in 0..64_u64 {
            let path = parent.join(format!(
                "scalar-cf-{}-{}-{attempt}",
                std::process::id(),
                NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(&path) {
                Ok(()) => {
                    let mode = std::fs::metadata(&path)
                        .expect("stat private build root")
                        .permissions()
                        .mode()
                        & 0o777;
                    assert_eq!(mode, 0o700, "private build root must be owner-only");
                    return Self(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create private build root: {error}"),
            }
        }
        panic!("could not allocate a private backend build root")
    }
}

impl Drop for PrivateBuildRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct TestOutputDir(PathBuf);

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let parent = workspace.join("target/rustc-codegen-fe2o3-test-output");
        let mut parent_builder = std::fs::DirBuilder::new();
        parent_builder.recursive(true).mode(0o700);
        parent_builder
            .create(&parent)
            .expect("create owner-only scalar-control-flow output parent");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            .expect("secure scalar-control-flow output parent");
        let path = parent.join(format!(
            "collected-scalar-cf-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale scalar-control-flow output");
        }
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(&path)
            .expect("create owner-only scalar-control-flow output");
        std::fs::create_dir(path.join("artifacts"))
            .expect("create scalar-control-flow artifact output");
        Self(path)
    }
}

impl Drop for TestOutputDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

struct CapturedChild {
    child: Child,
    stdout: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
    stderr: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
    process_group: libc::pid_t,
    running: bool,
}

impl CapturedChild {
    fn spawn(command: &mut Command, context: &str) -> Result<Self, String> {
        let parent = unsafe { libc::getpid() };
        command
            .process_group(0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("spawn {context}: {error}"))?;
        let process_group = libc::pid_t::try_from(child.id())
            .map_err(|_| format!("{context} PID does not fit pid_t"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("capture {context} stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| format!("capture {context} stderr"))?;
        Ok(Self {
            child,
            stdout: Some(thread::spawn(move || {
                let mut bytes = Vec::new();
                stdout.read_to_end(&mut bytes)?;
                Ok(bytes)
            })),
            stderr: Some(thread::spawn(move || {
                let mut bytes = Vec::new();
                stderr.read_to_end(&mut bytes)?;
                Ok(bytes)
            })),
            process_group,
            running: true,
        })
    }

    fn try_wait(&mut self, context: &str) -> Result<Option<ExitStatus>, String> {
        let status = self
            .child
            .try_wait()
            .map_err(|error| format!("poll {context}: {error}"))?;
        if status.is_some() {
            self.running = false;
        }
        Ok(status)
    }

    fn terminate(&mut self) {
        if !self.running {
            return;
        }
        unsafe {
            libc::kill(-self.process_group, libc::SIGTERM);
        }
        let grace = Instant::now() + TERMINATION_GRACE;
        while Instant::now() < grace {
            if self.child.try_wait().ok().flatten().is_some() {
                self.running = false;
                return;
            }
            thread::sleep(PROCESS_POLL_INTERVAL);
        }
        unsafe {
            libc::kill(-self.process_group, libc::SIGKILL);
        }
        let _ = self.child.wait();
        self.running = false;
    }

    fn finish(mut self, status: ExitStatus, context: &str) -> Result<Output, String> {
        let stdout = self
            .stdout
            .take()
            .expect("stdout reader is present")
            .join()
            .map_err(|_| format!("{context} stdout reader panicked"))?
            .map_err(|error| format!("read {context} stdout: {error}"))?;
        let stderr = self
            .stderr
            .take()
            .expect("stderr reader is present")
            .join()
            .map_err(|_| format!("{context} stderr reader panicked"))?
            .map_err(|error| format!("read {context} stderr: {error}"))?;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }

    fn wait_until(mut self, deadline: Instant, context: &str) -> Result<Output, String> {
        loop {
            if let Some(status) = self.try_wait(context)? {
                return self.finish(status, context);
            }
            if Instant::now() >= deadline {
                self.terminate();
                return Err(format!("{context} exceeded its monotonic deadline"));
            }
            thread::sleep(PROCESS_POLL_INTERVAL);
        }
    }
}

impl Drop for CapturedChild {
    fn drop(&mut self) {
        if self.running {
            self.terminate();
        }
    }
}

fn run_bounded(command: &mut Command, timeout: Duration, context: &str) -> Result<Output, String> {
    CapturedChild::spawn(command, context)?.wait_until(Instant::now() + timeout, context)
}

fn isolated_backend_environment_is_unavailable() -> bool {
    let result = USER_MOUNT_NAMESPACE.get_or_init(|| {
        let output = run_bounded(
            Command::new("unshare")
                .args(["--user", "--map-root-user", "--mount", "--fork", "--"])
                .arg("true"),
            Duration::from_secs(10),
            "user/mount namespace capability probe",
        )?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if is_known_namespace_policy_denial(&stderr) {
            return Err(format!(
                "host policy disables the required user/mount namespace: {}",
                stderr.trim()
            ));
        }
        panic!(
            "user/mount namespace probe failed for an unexpected reason\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            stderr
        );
    });
    if let Err(reason) = result {
        eprintln!("SKIP isolated backend test: {reason}");
        true
    } else {
        false
    }
}

fn is_known_namespace_policy_denial(stderr: &str) -> bool {
    stderr.contains("/proc/self/uid_map: Operation not permitted")
        || stderr.contains("unshare failed: Operation not permitted")
        || stderr.contains("unshare: Operation not permitted")
}

fn receive_backend_from_child(
    mut child: CapturedChild,
    socket: RawFd,
    deadline: Instant,
    context: &str,
) -> Result<(File, Output), String> {
    loop {
        let mut pollfd = libc::pollfd {
            fd: socket,
            events: libc::POLLIN,
            revents: 0,
        };
        let polled = unsafe { libc::poll(&mut pollfd, 1, 20) };
        if polled < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(format!("poll {context} descriptor socket: {error}"));
            }
        } else if polled > 0 && pollfd.revents & libc::POLLIN != 0 {
            let file = receive_backend_descriptor(socket)?;
            let output = child.wait_until(deadline, context)?;
            return Ok((file, output));
        }

        if let Some(status) = child.try_wait(context)? {
            let output = child.finish(status, context)?;
            return Err(format!(
                "{context} exited before transferring a descriptor: {status}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if Instant::now() >= deadline {
            child.terminate();
            return Err(format!(
                "{context} exceeded its monotonic descriptor-transfer deadline"
            ));
        }
    }
}

fn build_backend(workspace: &Path) -> &'static PinnedBackend {
    BACKEND.get_or_init(|| {
        let build_root = PrivateBuildRoot::new(workspace);
        let (parent_socket, child_socket) = UnixDatagram::pair().expect("create backend FD socket");
        set_close_on_exec(child_socket.as_raw_fd(), false)
            .expect("make backend helper socket inheritable");
        let mut command = Command::new("unshare");
        command
            .args(["--user", "--map-root-user", "--mount", "--fork", "--"])
            .arg(std::env::current_exe().expect("current integration test executable"))
            .args(["--ignored", "--exact", "isolated_backend_build_helper"])
            .env(BUILD_HELPER_ENV, "1")
            .env(
                BUILD_HELPER_SOCKET_ENV,
                child_socket.as_raw_fd().to_string(),
            )
            .env(BUILD_HELPER_WORKSPACE_ENV, workspace)
            .env(BUILD_HELPER_MOUNT_ENV, &build_root.0);
        let child = CapturedChild::spawn(&mut command, "namespaced backend build helper")
            .expect("launch namespaced backend build helper");
        drop(child_socket);
        let hostile_path = build_root.0.join("target/debug/librustc_codegen_fe2o3.so");
        std::fs::create_dir_all(hostile_path.parent().unwrap())
            .expect("create same-UID hostile backend path");
        std::fs::write(&hostile_path, b"same-uid path substitution")
            .expect("substitute nominal backend path outside helper namespace");
        let (file, output) = receive_backend_from_child(
            child,
            parent_socket.as_raw_fd(),
            Instant::now() + BACKEND_BUILD_TIMEOUT,
            "namespaced backend build helper",
        )
        .expect("receive sealed backend from bounded private mount namespace");
        assert_eq!(
            std::fs::read(&hostile_path).expect("read hostile parent-namespace path"),
            b"same-uid path substitution",
            "namespaced build must never expose or consume its target through the parent path"
        );
        assert!(
            output.status.success(),
            "namespaced backend build failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        pinned_backend_from_file(file).expect("verify transferred sealed backend")
    })
}

fn build_collection_backend(workspace: &Path) -> &'static PinnedBackend {
    COLLECTION_BACKEND.get_or_init(|| {
        // Rebuilding the backend in the parent integration-test target can replace
        // its dylib after later test binaries have linked against it. Keep this
        // source-correspondence build private, then retain only its sealed memfd.
        let build_output = TestOutputDir::new(workspace);
        let target_dir = build_output.0.join("collection-backend-target");
        let mut command = Command::new(env!("CARGO"));
        command
            .current_dir(workspace)
            .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
            .arg("--target-dir")
            .arg(&target_dir)
            .env("CARGO_PROFILE_DEV_DEBUG", "1")
            .env_remove("CARGO_TARGET_DIR")
            .env("CARGO_INCREMENTAL", "0");
        let output = run_bounded(
            &mut command,
            BACKEND_BUILD_TIMEOUT,
            "source-correspondence backend cargo build",
        )
        .expect("build source-correspondence backend within deadline");
        assert!(
            output.status.success(),
            "source-correspondence backend build failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // This lane measures exact source correspondence. It deliberately makes
        // no protected-build or protected-authority claim.
        pin_backend(&target_dir.join("debug/librustc_codegen_fe2o3.so"))
            .expect("pin source-correspondence backend in a sealed memfd")
    })
}

fn pin_backend(path: &Path) -> Result<PinnedBackend, String> {
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("open built backend without following symlinks: {error}"))?;
    if !source
        .metadata()
        .map_err(|error| format!("stat built backend: {error}"))?
        .file_type()
        .is_file()
    {
        return Err("built backend is not a regular file".to_owned());
    }
    let mut bytes = Vec::new();
    source
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read built backend: {error}"))?;
    if bytes.is_empty() {
        return Err("built backend is empty".to_owned());
    }
    let sha256 = Sha256::digest(&bytes).into();
    let name = CString::new("fe2o3-scalar-cf-backend").expect("static memfd name");
    let raw_fd =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC) };
    if raw_fd < 0 {
        return Err(format!(
            "create backend memfd: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut file = unsafe { File::from_raw_fd(raw_fd) };
    file.write_all(&bytes)
        .map_err(|error| format!("populate backend memfd: {error}"))?;
    if unsafe { libc::fcntl(raw_fd, libc::F_ADD_SEALS, REQUIRED_MEMFD_SEALS) } != 0 {
        return Err(format!(
            "seal backend memfd: {}",
            std::io::Error::last_os_error()
        ));
    }
    let pinned = PinnedBackend {
        file,
        len: bytes.len(),
        sha256,
    };
    pinned.verify()?;
    Ok(pinned)
}

fn pinned_backend_from_file(file: File) -> Result<PinnedBackend, String> {
    let len = usize::try_from(
        file.metadata()
            .map_err(|error| format!("stat transferred backend memfd: {error}"))?
            .len(),
    )
    .map_err(|_| "transferred backend length does not fit usize".to_owned())?;
    if len == 0 {
        return Err("transferred backend memfd is empty".to_owned());
    }
    let mut bytes = vec![0_u8; len];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|error| format!("read transferred backend memfd: {error}"))?;
    let pinned = PinnedBackend {
        file,
        len,
        sha256: Sha256::digest(bytes).into(),
    };
    pinned.verify()?;
    Ok(pinned)
}

fn set_close_on_exec(descriptor: RawFd, enabled: bool) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let updated = if enabled {
        flags | libc::FD_CLOEXEC
    } else {
        flags & !libc::FD_CLOEXEC
    };
    if unsafe { libc::fcntl(descriptor, libc::F_SETFD, updated) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn send_backend_descriptor(socket: RawFd, descriptor: RawFd) -> std::io::Result<()> {
    const MESSAGE: u64 = 0x4645_324f_3342_454e;
    let mut io_vector = libc::iovec {
        iov_base: ptr::from_ref(&MESSAGE).cast_mut().cast(),
        iov_len: mem::size_of_val(&MESSAGE),
    };
    let mut control = [0_usize; 8];
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut io_vector;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&header);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
        ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), descriptor);
    }
    let sent = unsafe { libc::sendmsg(socket, &header, libc::MSG_NOSIGNAL) };
    if sent == mem::size_of_val(&MESSAGE) as isize {
        Ok(())
    } else if sent < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Err(std::io::Error::from_raw_os_error(libc::EIO))
    }
}

fn receive_backend_descriptor(socket: RawFd) -> Result<File, String> {
    const MESSAGE: u64 = 0x4645_324f_3342_454e;
    let mut message = MaybeUninit::<u64>::zeroed();
    let mut io_vector = libc::iovec {
        iov_base: message.as_mut_ptr().cast(),
        iov_len: mem::size_of::<u64>(),
    };
    let mut control = [0_usize; 8];
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut io_vector;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
    let received = unsafe { libc::recvmsg(socket, &mut header, libc::MSG_CMSG_CLOEXEC) };
    if received < 0 {
        return Err(format!(
            "receive backend descriptor: {}",
            std::io::Error::last_os_error()
        ));
    }
    if received as usize != mem::size_of::<u64>()
        || header.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
    {
        return Err("backend descriptor transfer was truncated".to_owned());
    }
    let message = unsafe { message.assume_init() };
    if message != MESSAGE {
        return Err("backend descriptor transfer header is invalid".to_owned());
    }
    let descriptor = unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&header);
        if cmsg.is_null()
            || (*cmsg).cmsg_level != libc::SOL_SOCKET
            || (*cmsg).cmsg_type != libc::SCM_RIGHTS
            || (*cmsg).cmsg_len != libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize
            || !libc::CMSG_NXTHDR(&header, cmsg).is_null()
        {
            return Err("backend descriptor transfer control data is invalid".to_owned());
        }
        ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>())
    };
    if descriptor < 0 {
        return Err("backend descriptor transfer returned an invalid descriptor".to_owned());
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn mount_private_build_tmpfs(path: &Path) -> Result<(), String> {
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0 {
        return Err(format!(
            "make isolated build helper non-dumpable: {}",
            std::io::Error::last_os_error()
        ));
    }
    let source = CString::new("fe2o3-scalar-cf-build").unwrap();
    let target = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "private build mount path contains NUL".to_owned())?;
    let filesystem = CString::new("tmpfs").unwrap();
    let options = CString::new("mode=0700").unwrap();
    let result = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            filesystem.as_ptr(),
            libc::MS_NODEV | libc::MS_NOSUID,
            options.as_ptr().cast(),
        )
    };
    if result != 0 {
        return Err(format!(
            "mount isolated backend build tmpfs: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn compile(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source: &str,
    target: &str,
    pipeline: &str,
) -> Output {
    let source_path = output.0.join("fixture.rs");
    std::fs::write(&source_path, source).expect("write scalar-control-flow fixture");
    compile_path(
        workspace,
        backend,
        output,
        &source_path,
        target,
        pipeline,
        &[],
    )
}

fn compile_path(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source_path: &Path,
    target: &str,
    pipeline: &str,
    extra_args: &[&str],
) -> Output {
    backend
        .verify()
        .expect("sealed backend identity before rustc");
    let canonical_source = if source_path.is_absolute() {
        source_path.to_path_buf()
    } else {
        workspace.join(source_path)
    }
    .canonicalize()
    .expect("canonical scalar-control-flow fixture path");
    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(&canonical_source)
        .arg(format!(
            "--remap-path-prefix={}=/fe2o3-reviewed-workspace/scalar-control-flow-v1.rs",
            canonical_source.display()
        ))
        .args([
            "--edition=2024",
            "--crate-name",
            "fe2o3_scalar_control_flow_v1_fixture",
            "-C",
            "overflow-checks=off",
            "-Cmetadata=fe2o3-scalar-control-flow-v2-reviewed",
            "-Zmir-enable-passes=-JumpThreading",
            "-Zremap-cwd-prefix=/fe2o3-reviewed-workspace",
        ])
        .args(extra_args)
        .arg(format!(
            "-Zcodegen-backend={}",
            backend.load_path().display()
        ))
        .arg("-o")
        .arg(output.0.join("fixture"))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_DUMP_LLVM", "1")
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", pipeline)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"));
    let backend_descriptor = backend.file.as_raw_fd();
    unsafe {
        command.pre_exec(move || set_close_on_exec(backend_descriptor, false));
    }
    run_bounded(&mut command, COMPILER_TIMEOUT, "scalar-control-flow rustc")
        .expect("compile scalar-control-flow fixture within deadline")
}

fn build_frontend_dependencies(workspace: &Path) -> Result<(), String> {
    FRONTEND_DEPENDENCIES
        .get_or_init(|| {
            let mut command = Command::new(env!("CARGO"));
            command
                .current_dir(workspace)
                .args([
                    "build",
                    "--locked",
                    "-p",
                    "fe2o3-device",
                    "-p",
                    "fe2o3-host",
                ])
                .env("CARGO_INCREMENTAL", "0");
            let output = run_bounded(
                &mut command,
                BACKEND_BUILD_TIMEOUT,
                "scalar GEMM frontend dependency build",
            )?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "scalar GEMM frontend dependency build failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        })
        .clone()
}

fn compile_scalar_gemm(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source: &str,
    target: &str,
    extra_args: &[&str],
) -> Output {
    build_frontend_dependencies(workspace).expect("build scalar GEMM frontend dependencies");
    backend
        .verify()
        .expect("sealed backend identity before scalar GEMM rustc");
    let source_path = output.0.join("scalar-gemm-v1.rs");
    std::fs::write(&source_path, source).expect("write scalar GEMM fixture");
    let device = workspace.join("target/debug/libfe2o3_device.rlib");
    let host = workspace.join("target/debug/libfe2o3_host.rlib");
    let manifest_directory =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/scalar-gemm-v1");
    assert!(device.is_file(), "missing {}", device.display());
    assert!(host.is_file(), "missing {}", host.display());
    let producer =
        ProducerIdentity::from_codegen("fe2o3_scalar_gemm_v1_fixture", Some(&source_path))
            .expect("scalar GEMM fixture producer");
    let attempt = begin_build_attempt(
        &output.0.join("artifacts"),
        &producer,
        BuildInvocation::from_bytes(Sha256::digest(source.as_bytes()).into()),
        BuildSession::from_bytes([
            0x53, 0x47, 0x56, 0x31, 0x53, 0x47, 0x56, 0x31, 0x53, 0x47, 0x56, 0x31, 0x53, 0x47,
            0x56, 0x31,
        ]),
    )
    .expect("begin scalar GEMM managed fixture attempt");

    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}=/fe2o3-reviewed-workspace/scalar-gemm-v1.rs",
            source_path.display()
        ))
        .args([
            "--edition=2024",
            "--crate-type",
            "lib",
            "--crate-name",
            "fe2o3_scalar_gemm_v1_fixture",
            "--extern",
        ])
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            workspace.join("target/debug/deps").display()
        ))
        .args([
            "-C",
            "overflow-checks=off",
            "-Cmetadata=fe2o3-scalar-gemm-v1-reviewed",
            "-Zmir-enable-passes=-JumpThreading",
            "-Zremap-cwd-prefix=/fe2o3-reviewed-workspace",
        ])
        .args(extra_args)
        .arg(format!(
            "-Zcodegen-backend={}",
            backend.load_path().display()
        ))
        .arg("-o")
        .arg(output.0.join("scalar-gemm-v1"))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_DUMP_LLVM", "1")
        .env("CARGO_MANIFEST_DIR", manifest_directory)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "7cfd53537e4e74e68c2800f807b8d8a4b04507b5653d07b38ab3e99ace8d2740",
        )
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", SCALAR_GEMM_PIPELINE)
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value())
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"));
    let backend_descriptor = backend.file.as_raw_fd();
    unsafe {
        command.pre_exec(move || set_close_on_exec(backend_descriptor, false));
    }
    let result = run_bounded(&mut command, COMPILER_TIMEOUT, "scalar GEMM rustc")
        .expect("compile scalar GEMM fixture within deadline");
    if result.status.success()
        && let Some(destination) = std::env::var_os(SCALAR_GEMM_HANDOFF_OUTPUT_ENV)
    {
        let consumed =
            consume_compiler_module_handoff_v1(&output.0.join("artifacts"), &producer, attempt)
                .expect(
                    "consume exact scalar GEMM frontend handoff for configured integration output",
                );
        let decoded = CompilerModuleHandoffV2::decode(consumed.bytes())
            .expect("frontend published one canonical Worker V2 handoff");
        assert_eq!(decoded.canonical_bytes(), consumed.bytes());
        assert!(
            std::str::from_utf8(decoded.module_bytes())
                .expect("scalar compiler module is textual LLVM")
                .contains(".fe2o3.scalar-auth.v1")
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(destination)
            .expect("create fresh scalar GEMM frontend handoff output");
        file.write_all(consumed.bytes())
            .expect("write exact scalar GEMM frontend handoff");
        file.sync_all()
            .expect("sync exact scalar GEMM frontend handoff");
    }
    result
}

fn compile_tiled_gemm(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source: &str,
    target: &str,
    extra_args: &[&str],
) -> Output {
    let is_lds_slice1 = source.contains("pub fn tiled_gemm_lds_slice1");
    build_frontend_dependencies(workspace).expect("build tiled GEMM frontend dependencies");
    backend
        .verify()
        .expect("sealed backend identity before tiled GEMM rustc");
    let source_path = output.0.join("tiled-gemm-v1.rs");
    std::fs::write(&source_path, source).expect("write tiled GEMM fixture");
    let cargo_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let device = cargo_target.join("debug/libfe2o3_device.rlib");
    let host = cargo_target.join("debug/libfe2o3_host.rlib");
    let manifest_directory = if is_lds_slice1 {
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-lds-slice1")
    } else {
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-v1")
    };
    assert!(device.is_file(), "missing {}", device.display());
    assert!(host.is_file(), "missing {}", host.display());
    let crate_name = if is_lds_slice1 {
        "fe2o3_collected_tiled_gemm_lds_slice1_fixture"
    } else {
        "fe2o3_collected_tiled_gemm_v1_fixture"
    };
    let producer = ProducerIdentity::from_codegen(crate_name, Some(&source_path))
        .expect("tiled GEMM fixture producer");
    let attempt = begin_build_attempt(
        &output.0.join("artifacts"),
        &producer,
        BuildInvocation::from_bytes(Sha256::digest(source.as_bytes()).into()),
        BuildSession::from_bytes([
            0x54, 0x47, 0x56, 0x31, 0x54, 0x47, 0x56, 0x31, 0x54, 0x47, 0x56, 0x31, 0x54, 0x47,
            0x56, 0x31,
        ]),
    )
    .expect("begin tiled GEMM managed fixture attempt");

    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}=/fe2o3-reviewed-workspace/tiled-gemm-v1.rs",
            source_path.display()
        ))
        .args(["--edition=2024", "--crate-type", "lib", "--crate-name"])
        .arg(crate_name)
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            cargo_target.join("debug/deps").display()
        ))
        .args(["-C", "overflow-checks=off"]);
    if is_lds_slice1 {
        command.arg("-Cmetadata=e1f4d566b68639ae");
    } else {
        command.arg("-Cmetadata=4ceb166423714bdc");
    }
    command
        .args([
            "-Cmetadata=fe2o3-tiled-gemm-v1-reviewed",
            "-Zmir-enable-passes=-JumpThreading",
        ])
        .args(extra_args)
        .arg(format!(
            "-Zcodegen-backend={}",
            backend.load_path().display()
        ))
        .arg("-o")
        .arg(output.0.join("tiled-gemm-v1"))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_DUMP_LLVM", "1")
        .env("CARGO_MANIFEST_DIR", manifest_directory)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "c1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0",
        )
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", TILED_GEMM_PIPELINE)
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value())
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"));
    let backend_descriptor = backend.file.as_raw_fd();
    unsafe {
        command.pre_exec(move || set_close_on_exec(backend_descriptor, false));
    }
    run_bounded(&mut command, COMPILER_TIMEOUT, "tiled GEMM rustc")
        .expect("compile tiled GEMM fixture within deadline")
}

fn compile_row_softmax_direct(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source: &str,
) -> Output {
    build_frontend_dependencies(workspace).expect("build row-softmax frontend dependencies");
    let cargo_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    compile_row_softmax_with_device(
        workspace,
        backend,
        output,
        source,
        "gfx942:xnack-",
        "3a4d867f29d87610",
        &[],
        &cargo_target.join("debug/libfe2o3_device.rlib"),
        &cargo_target.join("debug/libfe2o3_host.rlib"),
        false,
        "a59650cf8d1bfc6168915cb817dbab3a0fa6a8839291231bbf4149a749913937",
    )
}

fn decode_compiler_owned_module_section(
    module: &str,
    section_name: &str,
) -> Result<Vec<u8>, String> {
    let lines = module.lines().collect::<Vec<_>>();
    let declarations = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            module_asm_section_name(line)
                .filter(|name| *name == section_name)
                .map(|_| index)
        })
        .collect::<Vec<_>>();
    let [header_index] = declarations.as_slice() else {
        return Err(format!(
            "expected exactly one {section_name} module-assembly header, found {}",
            declarations.len()
        ));
    };

    let mut target_bytes = Vec::new();
    let mut seen_sections = Vec::new();
    let mut index = *header_index;
    loop {
        let line = lines[index];
        let current_name = canonical_module_asm_section_name(line).ok_or_else(|| {
            format!("{section_name} has a noncanonical module-assembly section header {line:?}")
        })?;
        if seen_sections.contains(&current_name) {
            return Err(format!(
                "{section_name} suffix repeats module-assembly section {current_name}"
            ));
        }
        seen_sections.push(current_name);
        if lines.get(index + 1) != Some(&"module asm \".balign 8\"") {
            return Err(format!(
                "module-assembly section {current_name} does not have exact alignment 8"
            ));
        }
        index += 2;

        let first_byte_line = index;
        while let Some(line) = lines.get(index) {
            let Some(values) = line
                .strip_prefix("module asm \".byte ")
                .and_then(|line| line.strip_suffix('"'))
            else {
                break;
            };
            for value in values.split(", ") {
                let digits = value.strip_prefix("0x").ok_or_else(|| {
                    format!("module-assembly section {current_name} byte lacks 0x prefix")
                })?;
                if digits.len() != 2
                    || !digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(format!(
                        "module-assembly section {current_name} has noncanonical byte {value:?}"
                    ));
                }
                if current_name == section_name {
                    target_bytes.push(u8::from_str_radix(digits, 16).map_err(|error| {
                        format!("decode module-assembly section {current_name} byte: {error}")
                    })?);
                }
            }
            index += 1;
        }
        if index == first_byte_line {
            return Err(format!(
                "module-assembly section {current_name} has no retained bytes"
            ));
        }
        let Some(line) = lines.get(index) else {
            break;
        };
        if canonical_module_asm_section_name(line).is_none() {
            return Err(format!(
                "module-assembly section {current_name} has unexpected trailing line {line:?}"
            ));
        }
    }
    if target_bytes.is_empty() {
        return Err(format!("{section_name} has no retained bytes"));
    }
    Ok(target_bytes)
}

fn module_asm_section_name(line: &str) -> Option<&str> {
    line.strip_prefix("module asm \".section ")?
        .split_once(',')
        .map(|(name, _)| name)
}

fn canonical_module_asm_section_name(line: &str) -> Option<&str> {
    let suffix = line.strip_prefix("module asm \".section ")?;
    let name = suffix.strip_suffix(",\\22\\22,@progbits\"")?;
    (!name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
    .then_some(name)
}

struct ExactRowCompilerModule<'a> {
    llvm_body: &'a str,
    descriptor: Vec<u8>,
    authority_transcript: Vec<u8>,
    authority: Vec<u8>,
    exponential: Vec<u8>,
}

fn decode_exact_row_compiler_module(module: &str) -> Result<ExactRowCompilerModule<'_>, String> {
    const DESCRIPTOR: &str = ".fe2o3.kd.v1";
    const AUTHORITY_TRANSCRIPT: &str = ".fe2o3.row-softmax-authority-transcript.v1";
    const AUTHORITY: &str = ".fe2o3.row-softmax-auth.v1";
    const EXPONENTIAL: &str = ".fe2o3.row-exp.v1";

    if !module.ends_with('\n') {
        return Err("row-softmax LLVM module lacks its exact final newline".to_owned());
    }
    let descriptor_header = canonical_section_header(DESCRIPTOR);
    let boundary = format!("\n{descriptor_header}");
    let boundary_index = module
        .find(&boundary)
        .ok_or_else(|| "row-softmax LLVM module lacks its exact descriptor boundary".to_owned())?;
    if module.rfind(&boundary) != Some(boundary_index) {
        return Err("row-softmax LLVM module repeats its descriptor boundary".to_owned());
    }
    let llvm_body = &module[..boundary_index];
    if <[u8; 32]>::from(Sha256::digest(llvm_body.as_bytes())) != EXPECTED_ROW_LLVM_BODY_SHA256 {
        return Err(
            "row-softmax complete LLVM instruction body differs from its reviewed digest"
                .to_owned(),
        );
    }
    let sections = decode_exact_compiler_owned_suffix(
        &module[boundary_index + 1..],
        &[DESCRIPTOR, AUTHORITY_TRANSCRIPT, AUTHORITY, EXPONENTIAL],
    )?;
    let [descriptor, authority_transcript, authority, exponential] =
        sections.try_into().map_err(|_| {
            "row-softmax compiler-owned section cardinality differs from exactly four".to_owned()
        })?;
    Ok(ExactRowCompilerModule {
        llvm_body,
        descriptor,
        authority_transcript,
        authority,
        exponential,
    })
}

fn decode_exact_compiler_owned_suffix(
    suffix: &str,
    expected_sections: &[&str],
) -> Result<Vec<Vec<u8>>, String> {
    if suffix.is_empty() || !suffix.ends_with('\n') {
        return Err("compiler-owned section suffix lacks its exact final newline".to_owned());
    }
    let mut offset = 0;
    let mut sections = Vec::with_capacity(expected_sections.len());
    for expected_name in expected_sections {
        consume_exact_text(
            suffix,
            &mut offset,
            &canonical_section_header(expected_name),
        )?;
        consume_exact_text(suffix, &mut offset, "module asm \".balign 8\"\n")?;

        let mut bytes = Vec::new();
        let mut chunk_lengths = Vec::new();
        while offset < suffix.len() && !suffix[offset..].starts_with("module asm \".section ") {
            let relative_end = suffix[offset..].find('\n').ok_or_else(|| {
                format!("compiler-owned section {expected_name} has an unterminated byte line")
            })?;
            let line_end = offset + relative_end;
            let line = &suffix[offset..line_end];
            offset = line_end + 1;
            let values = line
                .strip_prefix("module asm \".byte ")
                .and_then(|line| line.strip_suffix('"'))
                .ok_or_else(|| {
                    format!("compiler-owned section {expected_name} has unexpected line {line:?}")
                })?;
            let mut chunk_length = 0;
            for value in values.split(", ") {
                let digits = value.strip_prefix("0x").ok_or_else(|| {
                    format!("compiler-owned section {expected_name} byte lacks 0x prefix")
                })?;
                if digits.len() != 2
                    || !digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(format!(
                        "compiler-owned section {expected_name} has noncanonical byte {value:?}"
                    ));
                }
                bytes.push(u8::from_str_radix(digits, 16).map_err(|error| {
                    format!("decode compiler-owned section {expected_name} byte: {error}")
                })?);
                chunk_length += 1;
            }
            if chunk_length == 0 || chunk_length > 16 {
                return Err(format!(
                    "compiler-owned section {expected_name} has a noncanonical byte chunk"
                ));
            }
            chunk_lengths.push(chunk_length);
        }
        let Some((last, preceding)) = chunk_lengths.split_last() else {
            return Err(format!(
                "compiler-owned section {expected_name} has no retained bytes"
            ));
        };
        if preceding.iter().any(|length| *length != 16) || !(1..=16).contains(last) {
            return Err(format!(
                "compiler-owned section {expected_name} does not use exact 16-byte chunking"
            ));
        }
        sections.push(bytes);
    }
    if offset != suffix.len() {
        return Err(format!(
            "compiler-owned section suffix has unreviewed trailing bytes at offset {offset}"
        ));
    }
    Ok(sections)
}

fn canonical_section_header(section: &str) -> String {
    format!("module asm \".section {section},\\22\\22,@progbits\"\n")
}

fn consume_exact_text(input: &str, offset: &mut usize, expected: &str) -> Result<(), String> {
    if !input[*offset..].starts_with(expected) {
        return Err(format!(
            "compiler-owned section suffix differs at offset {offset}; expected {expected:?}"
        ));
    }
    *offset += expected.len();
    Ok(())
}

fn require_same_length_instruction_substitution_rejected(
    handoff: &CompilerModuleHandoffV2,
    exact: &ExactRowCompilerModule<'_>,
) -> Result<(), String> {
    const ORIGINAL: &[u8] = b"fdiv float";
    const SUBSTITUTE: &[u8] = b"fmul float";
    let positions = exact
        .llvm_body
        .as_bytes()
        .windows(ORIGINAL.len())
        .enumerate()
        .filter_map(|(index, window)| (window == ORIGINAL).then_some(index))
        .collect::<Vec<_>>();
    let [position] = positions.as_slice() else {
        return Err(format!(
            "reviewed row LLVM contains {} fdiv instruction sites, expected one",
            positions.len()
        ));
    };
    let mut substituted_module = handoff.module_bytes().to_vec();
    substituted_module[*position..*position + ORIGINAL.len()].copy_from_slice(SUBSTITUTE);
    let rebuilt = CompilerModuleHandoffV2::new(
        handoff.kind(),
        handoff.target(),
        handoff.code_object_version(),
        handoff.envelope().clone(),
        handoff.symbol_manifest().clone(),
        &substituted_module,
    )
    .map_err(|error| format!("rebuild same-length instruction adversary: {error}"))?;
    let self_consistent = CompilerModuleHandoffV2::decode(rebuilt.canonical_bytes())
        .map_err(|error| format!("decode self-consistent instruction adversary: {error}"))?;
    let substituted_text = std::str::from_utf8(self_consistent.module_bytes())
        .map_err(|_| "same-length instruction adversary is not textual LLVM".to_owned())?;
    if decode_exact_row_compiler_module(substituted_text).is_ok() {
        return Err(
            "self-consistent same-length fdiv-to-fmul instruction substitution was accepted"
                .to_owned(),
        );
    }
    Ok(())
}

fn encode_test_compiler_owned_section(section: &str, bytes: &[u8], chunk_width: usize) -> String {
    let mut encoded = canonical_section_header(section);
    encoded.push_str("module asm \".balign 8\"\n");
    for chunk in bytes.chunks(chunk_width) {
        encoded.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                encoded.push_str(", ");
            }
            encoded.push_str(&format!("0x{byte:02x}"));
        }
        encoded.push_str("\"\n");
    }
    encoded
}

fn independently_expected_row_exp_boundary() -> [u8; 32] {
    const DOMAIN: &[u8] = b"fe2o3.row-softmax.exponential-boundary.v1";
    const REVIEWED_BOUNDARY: &[u8] = b"canonical Kernel IR names its abstract f32 exp operation;no authenticated implementation, approximation/error contract, OCML bitcode, link request, LLVM lowering, or real-number softmax equivalence";
    let mut digest = Sha256::new();
    for field in [DOMAIN, REVIEWED_BOUNDARY] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn independently_expected_cargo_row_authority(
    crate_name: &str,
    ordered_metadata: &[String],
    descriptor_bytes: &[u8],
    transcript: &[u8],
    attempt: &BuildAttempt,
    workspace: &Path,
    cargo_target: &Path,
    broker: &PinnedBrokerExecutable,
) -> Result<[u8; 32], String> {
    let [generated_metadata, reviewed_metadata] = ordered_metadata else {
        return Err("wrapper did not observe exactly two ordered Cargo metadata values".to_owned());
    };
    if generated_metadata.len() != 16
        || !generated_metadata
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || reviewed_metadata != REVIEWED_ROW_METADATA
    {
        return Err("wrapper-observed Cargo metadata does not match the reviewed shape".to_owned());
    }
    let crate_binding =
        derive_crate_binding_id_v1(crate_name, ordered_metadata.iter().map(String::as_str));
    let kernel_binding = derive_kernel_binding_id_v1(
        crate_binding,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        "row_softmax_v1",
        "row_softmax_v1",
    );
    let root_instance = host_kernel_symbol_v1(kernel_binding);
    let descriptor = CompilerDescriptorSourceV1::decode(descriptor_bytes)
        .map_err(|error| format!("decode descriptor for independent authority: {error}"))?;

    let mut metadata_digest = Sha256::new();
    hash_expected_field(&mut metadata_digest, ROW_METADATA_DOMAIN);
    for value in ordered_metadata {
        hash_expected_field(&mut metadata_digest, value.as_bytes());
    }
    let metadata_commitment: [u8; 32] = metadata_digest.finalize().into();

    let expected_metadata_transcript = cargo_metadata_transcript(ordered_metadata);
    let mut decoder = AuthorityTranscriptDecoder::new(transcript)?;
    for expected in [
        ROW_AUTHORITY_DOMAIN,
        &EXPECTED_ROW_PORTABLE_MIR_COMMITMENT,
        &EXPECTED_ROW_COMPILER_SEMANTICS_COMMITMENT,
        &EXPECTED_ROW_CANONICAL_MODULE_COMMITMENT,
        descriptor.identity().sha256(),
        root_instance.as_bytes(),
        b"row_softmax_v1",
        b"gfx942:xnack-",
        &6_u16.to_le_bytes(),
        &32_u64.to_le_bytes(),
        &288_u64.to_le_bytes(),
        &64_u32.to_le_bytes(),
        &expected_domain_commitment(ROW_ABI_DOMAIN, ROW_ABI),
        &EXPECTED_ROW_FN_ABI_COMMITMENT,
        &expected_domain_commitment(ROW_LAUNCH_DOMAIN, ROW_LAUNCH),
        &expected_domain_commitment(ROW_CORRESPONDENCE_DOMAIN, ROW_CORRESPONDENCE),
        &independently_expected_row_exp_boundary(),
        &Sha256::digest(ROW_FRONTEND_CONTRACT),
        generated_metadata.as_bytes(),
        reviewed_metadata.as_bytes(),
        &metadata_commitment,
    ] {
        decoder.expect(expected)?;
    }

    decoder.expect(b"fe2o3_device")?;
    let stable_crate_id = decoder.array::<8>()?;
    let crate_hash = decoder.array::<16>()?;
    if u64::from_le_bytes(stable_crate_id) == 0 || crate_hash == [0; 16] {
        return Err("authority transcript has an empty provider crate identity".to_owned());
    }
    decoder.expect(&expected_metadata_transcript)?;
    let provider_source = decoder.array::<32>()?;
    let definitions = (0..ROW_PROVIDER_PATHS.len())
        .map(|_| decoder.array::<16>())
        .collect::<Result<Vec<_>, _>>()?;
    let sources = (0..ROW_PROVIDER_PATHS.len())
        .map(|_| decoder.array::<32>())
        .collect::<Result<Vec<_>, _>>()?;
    if definitions.iter().any(|identity| identity == &[0; 16])
        || sources.iter().any(|identity| identity == &[0; 32])
        || sources.first() != Some(&provider_source)
    {
        return Err("authority transcript has incomplete provider identities".to_owned());
    }
    let observed_source_identities = independently_observed_provider_sources(workspace)?;
    if sources
        .iter()
        .any(|identity| !observed_source_identities.contains(identity))
    {
        return Err(
            "authority transcript names a provider source outside reviewed files".to_owned(),
        );
    }
    let provider_commitment = decoder.array::<32>()?;
    let mut expected_provider = Sha256::new();
    hash_expected_field(&mut expected_provider, ROW_PROVIDER_DOMAIN);
    hash_expected_field(&mut expected_provider, b"fe2o3_device");
    hash_expected_field(&mut expected_provider, &stable_crate_id);
    hash_expected_field(&mut expected_provider, &crate_hash);
    hash_expected_field(&mut expected_provider, &expected_metadata_transcript);
    for ((path, definition), source) in ROW_PROVIDER_PATHS.iter().zip(&definitions).zip(&sources) {
        hash_expected_field(&mut expected_provider, path);
        hash_expected_field(&mut expected_provider, definition);
        hash_expected_field(&mut expected_provider, source);
    }
    if provider_commitment != <[u8; 32]>::from(expected_provider.finalize()) {
        return Err("authority transcript provider commitment differs".to_owned());
    }

    decoder.expect(&attempt.generation().to_le_bytes())?;
    decoder.expect(attempt.session().as_bytes())?;
    decoder.expect(attempt.invocation().as_bytes())?;
    decoder.expect(&expected_metadata_transcript)?;
    decoder.expect(&independently_expected_compiler_closure(cargo_target)?)?;
    decoder.expect(&broker.sha256()?)?;
    if !decoder.finished() {
        return Err("authority transcript has trailing fields".to_owned());
    }
    Ok(Sha256::digest(transcript).into())
}

fn independently_expected_compiler_closure(cargo_target: &Path) -> Result<[u8; 32], String> {
    let backend = cargo_target.join("debug/librustc_codegen_fe2o3.so");
    if !backend.is_file() {
        return Err("test-owned authority backend is absent".to_owned());
    }
    independently_expected_compiler_closure_for_backend(sha256_path(&backend))
}

fn independently_expected_compiler_closure_for_backend(
    backend_sha256: [u8; 32],
) -> Result<[u8; 32], String> {
    let toolchain = authority_toolchain();
    let mut rustc_identity = Sha256::new();
    rustc_identity.update(RUSTC_RUNTIME_IDENTITY_DOMAIN);
    rustc_identity.update(toolchain.rustc_sha256);
    rustc_identity.update(toolchain.rustc_runtime_sha256);

    let mut closure = Sha256::new();
    closure.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
    closure.update(toolchain.cargo_sha256);
    closure.update(rustc_identity.finalize());
    closure.update(backend_sha256);
    Ok(closure.finalize().into())
}

struct AuthorityTranscriptDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> AuthorityTranscriptDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 4096 {
            return Err("authority transcript length is invalid".to_owned());
        }
        Ok(Self { bytes, offset: 0 })
    }

    fn field(&mut self) -> Result<&'a [u8], String> {
        let length_end = self
            .offset
            .checked_add(8)
            .ok_or_else(|| "authority transcript length overflow".to_owned())?;
        let encoded: [u8; 8] = self
            .bytes
            .get(self.offset..length_end)
            .ok_or_else(|| "authority transcript field length is truncated".to_owned())?
            .try_into()
            .expect("field length has exact width");
        let length = usize::try_from(u64::from_le_bytes(encoded))
            .map_err(|_| "authority transcript field exceeds usize".to_owned())?;
        let end = length_end
            .checked_add(length)
            .ok_or_else(|| "authority transcript field length overflow".to_owned())?;
        let field = self
            .bytes
            .get(length_end..end)
            .ok_or_else(|| "authority transcript field is truncated".to_owned())?;
        self.offset = end;
        Ok(field)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), String> {
        if self.field()? != expected {
            return Err("authority transcript fixed field differs".to_owned());
        }
        Ok(())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.field()?
            .try_into()
            .map_err(|_| format!("authority transcript field is not {N} bytes"))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn cargo_metadata_transcript(ordered_metadata: &[String]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CARGO_METADATA_TRANSCRIPT_DOMAIN);
    digest.update((ordered_metadata.len() as u64).to_le_bytes());
    for token in ordered_metadata {
        digest.update((token.len() as u64).to_le_bytes());
        digest.update(token.as_bytes());
    }
    digest.finalize().into()
}

fn independently_observed_provider_sources(workspace: &Path) -> Result<Vec<[u8; 32]>, String> {
    let root = workspace.join("crates/fe2o3-device/src");
    let mut files = Vec::new();
    collect_regular_files(&root, &mut files)?;
    files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&root)
                .map_err(|_| "provider source escaped its reviewed root".to_owned())?;
            let relative = relative.to_string_lossy();
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("read provider source {}: {error}", path.display()))?;
            let mut digest = Sha256::new();
            digest.update(ROW_PROVIDER_SOURCE_DOMAIN);
            digest.update((relative.len() as u64).to_le_bytes());
            digest.update(relative.as_bytes());
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
            Ok(digest.finalize().into())
        })
        .collect()
}

fn collect_regular_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(directory)
        .map_err(|error| format!("read provider source directory: {error}"))?
    {
        let entry = entry.map_err(|error| format!("read provider source entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect provider source entry: {error}"))?;
        if file_type.is_dir() {
            collect_regular_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        } else {
            return Err("provider source closure contains a non-file entry".to_owned());
        }
    }
    Ok(())
}

fn expected_domain_commitment(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_expected_field(&mut digest, domain);
    hash_expected_field(&mut digest, value);
    digest.finalize().into()
}

fn hash_expected_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn require_exact_row_descriptor_source(bytes: &[u8]) -> Result<(), String> {
    let source = CompilerDescriptorSourceV1::decode(bytes)
        .map_err(|error| format!("decode row descriptor source: {error}"))?;
    let table = source.table();
    if table.device_target().to_string() != "gfx942:xnack-"
        || table.code_object_version() != CodeObjectVersion::V6
        || table.kernels().len() != 1
    {
        return Err("row descriptor target, COV, or kernel cardinality differs".to_owned());
    }
    let kernel = &table.kernels()[0];
    if kernel.logical_name().as_str() != "row_softmax_v1"
        || kernel.entry_name().as_str() != "row_softmax_v1"
        || kernel.descriptor_symbol().as_str() != "row_softmax_v1.kd"
    {
        return Err("row descriptor symbol closure differs".to_owned());
    }
    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != 32
        || abi.kernarg_segment_size() != 288
        || abi.kernarg_segment_alignment() != 8
    {
        return Err("row descriptor kernarg sizes or alignment differ".to_owned());
    }
    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err("row descriptor does not require an exact block".to_owned());
    };
    let grid = launch.max_grid();
    if launch.rank() != 1
        || (block.x(), block.y(), block.z()) != (64, 1, 1)
        || (grid.x(), grid.y(), grid.z()) != (1, 1, 1)
        || launch.max_flat_workgroup_size() != 64
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err("row descriptor launch geometry differs from WG64/grid1".to_owned());
    }
    let [input, output] = kernel.arguments() else {
        return Err("row descriptor does not contain exactly two slices".to_owned());
    };
    if input.source_index() != 0
        || input.name().as_str() != "arg0"
        || input.ownership() != OwnershipSemantics::SharedBorrow
        || input.access() != AccessMode::ReadOnly
        || input.alias() != AliasSemantics::SharedReadOnly
        || output.source_index() != 1
        || output.name().as_str() != "arg1"
        || output.ownership() != OwnershipSemantics::UniqueBorrow
        || output.access() != AccessMode::ReadWrite
        || output.alias() != AliasSemantics::Exclusive
    {
        return Err(format!(
            "row descriptor slice roles or effects differ: input=({}, {}, {:?}, {:?}, {:?}), output=({}, {}, {:?}, {:?}, {:?})",
            input.source_index(),
            input.name().as_str(),
            input.ownership(),
            input.access(),
            input.alias(),
            output.source_index(),
            output.name().as_str(),
            output.ownership(),
            output.access(),
            output.alias(),
        ));
    }
    let input_type = table
        .type_records()
        .iter()
        .find(|record| record.identity() == input.source_type())
        .ok_or_else(|| "row input type record is missing".to_owned())?;
    let output_type = table
        .type_records()
        .iter()
        .find(|record| record.identity() == output.source_type())
        .ok_or_else(|| "row output type record is missing".to_owned())?;
    if !input_type.descriptor().is_shared_slice()
        || input_type.descriptor().scalar_type() != ScalarTypeV1::F32
        || !output_type.descriptor().is_disjoint_slice()
        || output_type.descriptor().scalar_type() != ScalarTypeV1::F32
    {
        return Err("row descriptor arguments are not the exact two f32 slice types".to_owned());
    }
    let expected_input = [
        (PhysicalAbiComponentKind::GlobalPointer, 0, 8, 8),
        (PhysicalAbiComponentKind::SliceLengthU64, 8, 8, 8),
    ];
    let expected_output = [
        (PhysicalAbiComponentKind::GlobalPointer, 16, 8, 8),
        (PhysicalAbiComponentKind::SliceLengthU64, 24, 8, 8),
    ];
    if input.physical_components().collect::<Vec<_>>() != expected_input
        || output.physical_components().collect::<Vec<_>>() != expected_output
    {
        return Err("row descriptor physical slice layout differs".to_owned());
    }
    Ok(())
}

fn require_exact_lds_slice1_descriptor_source(bytes: &[u8]) -> Result<(), String> {
    let source = CompilerDescriptorSourceV1::decode(bytes)
        .map_err(|error| format!("decode LDS Slice 1 descriptor source: {error}"))?;
    let table = source.table();
    if table.device_target().to_string() != "gfx942:xnack-"
        || table.code_object_version() != CodeObjectVersion::V6
        || table.kernels().len() != 1
    {
        return Err("LDS descriptor target, COV, or kernel cardinality differs".to_owned());
    }
    let kernel = &table.kernels()[0];
    if kernel.logical_name().as_str() != "tiled_gemm_lds_slice1"
        || kernel.entry_name().as_str() != "tiled_gemm_lds_v1"
        || kernel.descriptor_symbol().as_str() != "tiled_gemm_lds_v1.kd"
    {
        return Err("LDS descriptor source/canonical symbol join differs".to_owned());
    }
    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != 48
        || abi.kernarg_segment_size() != 304
        || abi.kernarg_segment_alignment() != 8
    {
        return Err("LDS descriptor 48/304-byte COV6 ABI differs".to_owned());
    }
    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err("LDS descriptor block is not exact".to_owned());
    };
    let grid = launch.max_grid();
    if launch.rank() != 1
        || (block.x(), block.y(), block.z()) != (64, 1, 1)
        || (grid.x(), grid.y(), grid.z()) != (1, 1, 1)
        || launch.max_flat_workgroup_size() != 64
        || launch.static_shared_memory_bytes() != 1024
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err("LDS descriptor is not exact WG64/grid1/static-LDS1024".to_owned());
    }
    for capability in [
        CapabilityV1::Subgroup,
        CapabilityV1::WorkgroupMemory,
        CapabilityV1::MatrixMultiply,
        CapabilityV1::AmdWave,
        CapabilityV1::AmdMfma,
    ] {
        if !kernel.capabilities().contains(&capability) {
            return Err(format!("LDS descriptor lacks {capability:?}"));
        }
    }
    let [a, b, c] = kernel.arguments() else {
        return Err("LDS descriptor does not contain exactly A/B/C".to_owned());
    };
    if a.access() != AccessMode::ReadOnly
        || b.access() != AccessMode::ReadOnly
        || c.access() != AccessMode::ReadWrite
        || a.ownership() != OwnershipSemantics::SharedBorrow
        || b.ownership() != OwnershipSemantics::SharedBorrow
        || c.ownership() != OwnershipSemantics::UniqueBorrow
        || c.alias() != AliasSemantics::Exclusive
    {
        return Err("LDS descriptor argument roles differ".to_owned());
    }
    Ok(())
}

fn decode_framed_fields(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    let mut fields = Vec::new();
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let length_bytes: [u8; 8] = remaining
            .get(..8)
            .ok_or_else(|| "resource transcript has a truncated field length".to_owned())?
            .try_into()
            .unwrap();
        remaining = &remaining[8..];
        let length = usize::try_from(u64::from_le_bytes(length_bytes))
            .map_err(|_| "resource transcript field length exceeds usize".to_owned())?;
        let field = remaining
            .get(..length)
            .ok_or_else(|| "resource transcript has a truncated field".to_owned())?;
        fields.push(field);
        remaining = &remaining[length..];
    }
    Ok(fields)
}

fn validate_exact_row_softmax_handoff<F>(bytes: &[u8], expected_authority: F) -> Result<(), String>
where
    F: FnOnce(&[u8], &[u8]) -> Result<[u8; 32], String>,
{
    let handoff = CompilerModuleHandoffV2::decode(bytes)
        .map_err(|error| format!("decode exact row-softmax Worker V2 handoff: {error}"))?;
    if handoff.canonical_bytes() != bytes {
        return Err("row-softmax handoff is not its exact canonical encoding".to_owned());
    }
    if handoff.target().to_string() != "gfx942:xnack-"
        || handoff.code_object_version() != CodeObjectVersion::V6
    {
        return Err("row-softmax handoff target or COV differs".to_owned());
    }
    let expected_manifest = [
        (CompilerModuleSymbolRoleV1::KernelEntry, "row_softmax_v1"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "row_softmax_v1.kd",
        ),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            "__ocml_exp_f32",
        ),
    ];
    if handoff.symbol_manifest().entries().collect::<Vec<_>>() != expected_manifest {
        return Err(
            "row-softmax handoff symbol manifest is not the exact three-symbol closure".to_owned(),
        );
    }
    if handoff.authenticates_compiler_origin()
        || handoff.grants_compiler_authority()
        || handoff.grants_worker_authority()
        || handoff.grants_link_authority()
        || handoff.grants_load_authority()
        || handoff.grants_launch_authority()
    {
        return Err("inert row-softmax handoff unexpectedly grants authority".to_owned());
    }

    let module = std::str::from_utf8(handoff.module_bytes())
        .map_err(|_| "row-softmax compiler module is not textual LLVM".to_owned())?;
    let exact_module = decode_exact_row_compiler_module(module)?;
    if exact_module
        .llvm_body
        .matches("declare float @__ocml_exp_f32(float)")
        .count()
        != 1
        || exact_module
            .llvm_body
            .matches("call float @__ocml_exp_f32(float ")
            .count()
            != 2
    {
        return Err(
            "row-softmax LLVM does not contain its exact one-declaration/two-call OCML closure"
                .to_owned(),
        );
    }
    require_same_length_instruction_substitution_rejected(&handoff, &exact_module)?;

    let descriptor_bytes = exact_module.descriptor;
    require_exact_row_descriptor_source(&descriptor_bytes)?;
    if require_exact_row_descriptor_source(
        descriptor_bytes
            .get(..descriptor_bytes.len() - 1)
            .ok_or_else(|| "row descriptor source is unexpectedly empty".to_owned())?,
    )
    .is_ok()
    {
        return Err("truncated row descriptor source was accepted".to_owned());
    }
    let mut symbol_substitution = descriptor_bytes.clone();
    let symbol_offset = symbol_substitution
        .windows(b"row_softmax_v1".len())
        .position(|window| window == b"row_softmax_v1")
        .ok_or_else(|| "row descriptor omits its kernel symbol".to_owned())?;
    symbol_substitution[symbol_offset] = b's';
    if require_exact_row_descriptor_source(&symbol_substitution).is_ok() {
        return Err("same-length row descriptor symbol substitution was accepted".to_owned());
    }

    let authority_transcript = exact_module.authority_transcript;
    let authority = exact_module.authority;
    let exponential = exact_module.exponential;
    if authority.len() != 32
        || authority.as_slice()
            != <[u8; 32]>::from(Sha256::digest(&authority_transcript)).as_slice()
        || authority.as_slice() != expected_authority(&descriptor_bytes, &authority_transcript)?
    {
        return Err(format!(
            "row-softmax authority commitment differs from the independent expectation: actual {}",
            encode_lower_hex(&authority)
        ));
    }
    if exponential.as_slice() != independently_expected_row_exp_boundary() {
        return Err(
            "row-softmax exponential-boundary commitment differs from the independent derivation"
                .to_owned(),
        );
    }

    let renamed_section = module.replacen(".fe2o3.row-softmax-auth.v1", ".fe2o3.row-auth.v1", 1);
    if decode_compiler_owned_module_section(&renamed_section, ".fe2o3.row-softmax-auth.v1").is_ok()
    {
        return Err("legacy row authority section name was accepted".to_owned());
    }
    Ok(())
}

#[test]
fn compiler_owned_module_section_decoder_rejects_malformed_and_substituted_sections() {
    let exact = concat!(
        "module asm \".section .fe2o3.test.v1,\\22\\22,@progbits\"\n",
        "module asm \".balign 8\"\n",
        "module asm \".byte 0x00, 0x7f, 0xff\"\n",
    );
    assert_eq!(
        decode_compiler_owned_module_section(exact, ".fe2o3.test.v1").unwrap(),
        [0x00, 0x7f, 0xff]
    );
    let malformed = exact.replace("0x7f", "0x7F");
    assert!(decode_compiler_owned_module_section(&malformed, ".fe2o3.test.v1").is_err());
    let misaligned = exact.replace(".balign 8", ".balign 4");
    assert!(decode_compiler_owned_module_section(&misaligned, ".fe2o3.test.v1").is_err());
    let substituted = exact.replace(".fe2o3.test.v1", ".fe2o3.test.v2");
    assert!(decode_compiler_owned_module_section(&substituted, ".fe2o3.test.v1").is_err());
    let duplicate = format!("{exact}{exact}");
    assert!(decode_compiler_owned_module_section(&duplicate, ".fe2o3.test.v1").is_err());

    for directive in [".zero 1", ".long 0", ".p2align 3"] {
        let trailing_directive = format!("{exact}module asm \"{directive}\"\n");
        assert!(
            decode_compiler_owned_module_section(&trailing_directive, ".fe2o3.test.v1").is_err(),
            "accepted trailing assembler directive {directive}"
        );
    }
    let ordinary_line = format!("{exact}define void @trailing() {{ ret void }}\n");
    assert!(decode_compiler_owned_module_section(&ordinary_line, ".fe2o3.test.v1").is_err());
    let empty_ambiguity = format!("{exact}\n");
    assert!(decode_compiler_owned_module_section(&empty_ambiguity, ".fe2o3.test.v1").is_err());

    let next_header = concat!(
        "module asm \".section .fe2o3.next.v1,\\22\\22,@progbits\"\n",
        "module asm \".balign 8\"\n",
        "module asm \".byte 0x42\"\n",
    );
    let exact_with_next_section = format!("{exact}{next_header}");
    assert_eq!(
        decode_compiler_owned_module_section(&exact_with_next_section, ".fe2o3.test.v1").unwrap(),
        [0x00, 0x7f, 0xff]
    );
    let extra_byte_after_boundary = format!(
        "{exact}module asm \".section .fe2o3.next.v1,\\22\\22,@progbits\"\nmodule asm \".byte 0x42\"\n"
    );
    assert!(
        decode_compiler_owned_module_section(&extra_byte_after_boundary, ".fe2o3.test.v1").is_err()
    );
    let reordered = exact.replace(
        "module asm \".balign 8\"\nmodule asm \".byte 0x00, 0x7f, 0xff\"",
        "module asm \".byte 0x00, 0x7f, 0xff\"\nmodule asm \".balign 8\"",
    );
    assert!(decode_compiler_owned_module_section(&reordered, ".fe2o3.test.v1").is_err());
    let split_target = format!("{exact}{next_header}{exact}");
    assert!(decode_compiler_owned_module_section(&split_target, ".fe2o3.test.v1").is_err());
    let extra_byte_after_ordinary_boundary =
        format!("{exact}define void @trailing() {{ ret void }}\nmodule asm \".byte 0x42\"\n");
    assert!(
        decode_compiler_owned_module_section(&extra_byte_after_ordinary_boundary, ".fe2o3.test.v1")
            .is_err()
    );

    let descriptor = (0_u8..17).collect::<Vec<_>>();
    let authority = [0xa5; 32];
    let exponential = [0x5a; 32];
    let descriptor_section = encode_test_compiler_owned_section(".fe2o3.kd.v1", &descriptor, 16);
    let authority_section =
        encode_test_compiler_owned_section(".fe2o3.row-softmax-auth.v1", &authority, 16);
    let exponential_section =
        encode_test_compiler_owned_section(".fe2o3.row-exp.v1", &exponential, 16);
    let exact_suffix = format!("{descriptor_section}{authority_section}{exponential_section}");
    assert_eq!(
        decode_exact_compiler_owned_suffix(
            &exact_suffix,
            &[
                ".fe2o3.kd.v1",
                ".fe2o3.row-softmax-auth.v1",
                ".fe2o3.row-exp.v1",
            ],
        )
        .unwrap(),
        [descriptor.clone(), authority.to_vec(), exponential.to_vec()]
    );

    let unreviewed = encode_test_compiler_owned_section(".fe2o3.unreviewed.v1", &[0x42], 16);
    assert!(
        decode_exact_compiler_owned_suffix(
            &format!("{exact_suffix}{unreviewed}"),
            &[
                ".fe2o3.kd.v1",
                ".fe2o3.row-softmax-auth.v1",
                ".fe2o3.row-exp.v1",
            ],
        )
        .is_err(),
        "accepted an appended canonical unreviewed section"
    );
    for malformed_suffix in [
        format!("{authority_section}{descriptor_section}{exponential_section}"),
        format!("{descriptor_section}{descriptor_section}{authority_section}{exponential_section}"),
        format!("{descriptor_section}{authority_section}"),
        format!(
            "{}{}{}",
            encode_test_compiler_owned_section(".fe2o3.kd.v1", &descriptor, 8),
            authority_section,
            exponential_section,
        ),
        exact_suffix.trim_end_matches('\n').to_owned(),
    ] {
        assert!(
            decode_exact_compiler_owned_suffix(
                &malformed_suffix,
                &[
                    ".fe2o3.kd.v1",
                    ".fe2o3.row-softmax-auth.v1",
                    ".fe2o3.row-exp.v1",
                ],
            )
            .is_err(),
            "accepted malformed exact compiler-owned suffix"
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_row_softmax_with_device(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source: &str,
    target: &str,
    cargo_metadata: &str,
    extra_args: &[&str],
    device: &Path,
    host: &Path,
    managed_attempt: bool,
    cargo_metadata_observation: &str,
) -> Output {
    backend
        .verify()
        .expect("sealed backend identity before row-softmax rustc");
    let source_path = output.0.join("row-softmax-v1.rs");
    std::fs::write(&source_path, source).expect("write row-softmax fixture");
    let cargo_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let manifest_directory =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1");
    assert!(device.is_file(), "missing {}", device.display());
    assert!(host.is_file(), "missing {}", host.display());
    let attempt = managed_attempt.then(|| {
        let producer = ProducerIdentity::from_codegen(
            "fe2o3_collected_row_softmax_v1_fixture",
            Some(&source_path),
        )
        .expect("row-softmax fixture producer");
        begin_build_attempt(
            &output.0.join("artifacts"),
            &producer,
            BuildInvocation::from_bytes(Sha256::digest(source.as_bytes()).into()),
            BuildSession::from_bytes([0x52; 16]),
        )
        .expect("begin row-softmax managed fixture attempt")
    });

    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}=/fe2o3-reviewed-workspace/row-softmax-v1.rs",
            source_path.display()
        ))
        .args([
            "--edition=2024",
            "--crate-type",
            "lib",
            "--crate-name",
            "fe2o3_collected_row_softmax_v1_fixture",
            "--extern",
        ])
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            cargo_target.join("debug/deps").display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            device
                .parent()
                .expect("device rlib has a target profile directory")
                .join("deps")
                .display()
        ))
        .args(["-C", "overflow-checks=off"])
        .arg(format!("-Cmetadata={cargo_metadata}"))
        .args([
            "-Cmetadata=fe2o3-row-softmax-v1-reviewed",
            "-Zmir-enable-passes=-JumpThreading",
        ])
        .args(extra_args)
        .arg(format!(
            "-Zcodegen-backend={}",
            backend.load_path().display()
        ))
        .arg("-o")
        .arg(output.0.join("row-softmax-v1"))
        .env("FE2O3_VERBOSE", "1")
        .env("CARGO_MANIFEST_DIR", manifest_directory)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            cargo_metadata_observation,
        )
        .env(
            "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1",
            encode_lower_hex(
                &independently_expected_compiler_closure_for_backend(backend.sha256)
                    .expect("derive the row-softmax command's exact compiler closure"),
            ),
        )
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", ROW_SOFTMAX_PIPELINE)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"));
    if let Some(attempt) = attempt {
        command.env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value());
    } else {
        command.env_remove("FE2O3_BUILD_ATTEMPT_V1");
    }
    let backend_descriptor = backend.file.as_raw_fd();
    unsafe {
        command.pre_exec(move || set_close_on_exec(backend_descriptor, false));
    }
    run_bounded(&mut command, COMPILER_TIMEOUT, "row-softmax rustc")
        .expect("compile row-softmax fixture within deadline")
}

fn compile_row_softmax_with_forged_exact_argv_and_fixed_descriptors(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
) -> Output {
    const EFFECTIVE_ARGV_DOMAIN: &[u8] = b"FE2O3/ROW-SOFTMAX/EFFECTIVE-RUSTC-ARGV/V1\0";
    const ARTIFACT_FD: RawFd = fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1;
    const BACKEND_FD: RawFd = fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_CHILD_FD_V1;
    const INVOCATION_AUTHORITY_FD: RawFd =
        fe2o3_artifact_transaction::BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1;

    backend
        .verify()
        .expect("sealed production backend before forged row-softmax rustc");
    build_frontend_dependencies(workspace).expect("build row-softmax frontend dependencies");
    let cargo_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let source_path = output.0.join("row-softmax-v1.rs");
    std::fs::write(&source_path, ROW_SOFTMAX_FIXTURE).expect("write forged row-softmax source");
    let device = cargo_target.join("debug/libfe2o3_device.rlib");
    let host = cargo_target.join("debug/libfe2o3_host.rlib");
    let manifest_directory =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1");

    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}=/fe2o3-reviewed-workspace/row-softmax-v1.rs",
            source_path.display()
        ))
        .args([
            "--edition=2024",
            "--crate-type",
            "lib",
            "--crate-name",
            "fe2o3_collected_row_softmax_v1_fixture",
            "--extern",
        ])
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            cargo_target.join("debug/deps").display()
        ))
        .args([
            "-C",
            "overflow-checks=off",
            "-Cmetadata=3a4d867f29d87610",
            "-Cmetadata=fe2o3-row-softmax-v1-reviewed",
            "-o",
        ])
        .arg(output.0.join("row-softmax-v1"))
        .args([
            "-Zmir-enable-passes=-JumpThreading",
            "--cfg",
            "fe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
        ])
        .arg(format!(
            "-Zcodegen-backend={}",
            fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1
        ));

    let argv = std::iter::once(command.get_program())
        .chain(command.get_args())
        .collect::<Vec<_>>();
    let mut digest = Sha256::new();
    digest.update(EFFECTIVE_ARGV_DOMAIN);
    digest.update((argv.len() as u64).to_le_bytes());
    for argument in argv {
        let bytes = argument.as_bytes();
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    let compiler_closure = independently_expected_compiler_closure_for_backend(backend.sha256)
        .expect("derive the forged command's exact compiler closure");
    let invocation = BuildInvocation::from_bytes(digest.finalize().into())
        .bind_compiler_closure_v1(compiler_closure);
    let producer = ProducerIdentity::from_codegen(
        "fe2o3_collected_row_softmax_v1_fixture",
        Some(&source_path),
    )
    .expect("forged row-softmax producer");
    let attempt = begin_build_attempt(
        &output.0.join("artifacts"),
        &producer,
        invocation,
        BuildSession::from_bytes([0x52; 16]),
    )
    .expect("begin forged exact-argv attempt");

    command
        .env("FE2O3_VERBOSE", "1")
        .env("CARGO_MANIFEST_DIR", manifest_directory)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "a59650cf8d1bfc6168915cb817dbab3a0fa6a8839291231bbf4149a749913937",
        )
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", ROW_SOFTMAX_PIPELINE)
        .env(
            "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1",
            encode_lower_hex(&compiler_closure),
        )
        .env(
            "FE2O3_HSACO_DIR",
            fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_PATH_V1,
        )
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value());

    let artifact = File::open(output.0.join("artifacts"))
        .expect("open attacker-controlled artifact directory");
    let (attacker_child, _attacker_peer) =
        UnixStream::pair().expect("create attacker invocation-authority socket");
    let artifact_source = artifact.as_raw_fd();
    let backend_source = backend.file.as_raw_fd();
    let authority_source = attacker_child.as_raw_fd();
    assert!(
        [artifact_source, backend_source, authority_source]
            .into_iter()
            .all(|source| ![ARTIFACT_FD, BACKEND_FD, INVOCATION_AUTHORITY_FD].contains(&source)),
        "test source descriptors unexpectedly occupied a reserved child FD"
    );
    unsafe {
        command.pre_exec(move || {
            for (source, target) in [
                (artifact_source, ARTIFACT_FD),
                (backend_source, BACKEND_FD),
                (authority_source, INVOCATION_AUTHORITY_FD),
            ] {
                if libc::dup2(source, target) != target {
                    return Err(std::io::Error::last_os_error());
                }
                set_close_on_exec(target, false)?;
            }
            Ok(())
        });
    }
    run_bounded(
        &mut command,
        COMPILER_TIMEOUT,
        "forged exact-argv row-softmax rustc",
    )
    .expect("run forged exact-argv row-softmax rustc within deadline")
}

fn assert_row_softmax_published_nothing(output: &TestOutputDir) {
    assert!(
        !output.0.join("row-softmax-v1").exists(),
        "row-softmax boundary emitted a linked output"
    );
    let artifact_directory = output.0.join("artifacts");
    if !artifact_directory.exists() {
        return;
    }
    let artifacts = std::fs::read_dir(artifact_directory)
        .expect("read row-softmax artifact directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("enumerate row-softmax artifacts");
    let published = artifacts
        .iter()
        .filter(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some(".fe2o3-attempts-v1" | ".fe2o3-artifacts.lock")
            )
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert!(
        published.is_empty(),
        "row-softmax boundary published artifacts: {published:?}"
    );
}

struct WrapperHandoffObservation {
    attempt: BuildAttempt,
    crate_name: String,
    source_path: PathBuf,
    output_dir: PathBuf,
    ordered_metadata: Vec<String>,
    observation_sha256: [u8; 32],
}

impl WrapperHandoffObservation {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_HANDOFF_OBSERVATION_BYTES
            || bytes.len() < HANDOFF_OBSERVATION_MAGIC.len() + 2 + 32 + 32
        {
            return Err("wrapper handoff observation length is invalid".to_owned());
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if digest_observation_parts(HANDOFF_OBSERVATION_DOMAIN, &[body]).as_slice() != checksum {
            return Err("wrapper handoff observation checksum differs".to_owned());
        }
        let mut decoder = HandoffObservationDecoder::new(body);
        if decoder.take(HANDOFF_OBSERVATION_MAGIC.len())? != HANDOFF_OBSERVATION_MAGIC
            || decoder.u16()? != 1
            || decoder.take(HANDOFF_OBSERVATION_AUTHORITY_NONE.len())?
                != HANDOFF_OBSERVATION_AUTHORITY_NONE
        {
            return Err("wrapper handoff observation header differs".to_owned());
        }
        let attempt_text = decoder.text()?;
        let attempt = BuildAttempt::from_env_value(attempt_text)
            .map_err(|_| "wrapper handoff observation attempt is invalid".to_owned())?;
        if attempt.session() == BuildSession::DIRECT || attempt.to_env_value() != attempt_text {
            return Err(
                "wrapper handoff observation attempt is not canonical managed evidence".to_owned(),
            );
        }
        let crate_name = decoder.text()?.to_owned();
        let source_path = PathBuf::from(decoder.text()?);
        let output_dir = PathBuf::from(decoder.text()?);
        let metadata_count = usize::from(decoder.u16()?);
        if metadata_count == 0 || metadata_count > 32 {
            return Err("wrapper handoff observation metadata count is invalid".to_owned());
        }
        let mut ordered_metadata = Vec::with_capacity(metadata_count);
        for _ in 0..metadata_count {
            ordered_metadata.push(decoder.text()?.to_owned());
        }
        let producer_binding = decoder.array::<32>()?;
        if !decoder.finished()
            || producer_binding
                != digest_observation_parts(
                    HANDOFF_OBSERVATION_PRODUCER_DOMAIN,
                    &[crate_name.as_bytes(), source_path.as_os_str().as_bytes()],
                )
        {
            return Err("wrapper handoff observation producer binding differs".to_owned());
        }
        Ok(Self {
            attempt,
            crate_name,
            source_path,
            output_dir,
            ordered_metadata,
            observation_sha256: Sha256::digest(bytes).into(),
        })
    }
}

struct HandoffObservationDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> HandoffObservationDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| "wrapper handoff observation length overflow".to_owned())?;
        let field = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "wrapper handoff observation is truncated".to_owned())?;
        self.offset = end;
        Ok(field)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?
            .try_into()
            .map_err(|_| "wrapper handoff observation array is truncated".to_owned())
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn text(&mut self) -> Result<&'a str, String> {
        let length = usize::try_from(u32::from_le_bytes(self.array()?))
            .map_err(|_| "wrapper handoff observation field length overflow".to_owned())?;
        if length == 0 || length > 4096 {
            return Err("wrapper handoff observation field length is invalid".to_owned());
        }
        std::str::from_utf8(self.take(length)?)
            .map_err(|_| "wrapper handoff observation field is not UTF-8".to_owned())
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn digest_observation_parts(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn observe_and_validate_external_row_handoff(
    observation_directory: &Path,
    expected_crate_name: &str,
    crate_root: &Path,
    workspace: &Path,
    cargo_target: &Path,
    broker: &PinnedBrokerExecutable,
    cancelled: &std::sync::mpsc::Receiver<()>,
) -> Result<(), String> {
    let observation_path = observation_directory.join("observation");
    let deadline = Instant::now() + BACKEND_BUILD_TIMEOUT;
    let bytes = loop {
        match read_private_observation(&observation_path)? {
            Some(bytes) => break bytes,
            None if cancelled.try_recv().is_ok() => {
                return Err(
                    "cargo-fe2o3 exited before publishing wrapper handoff evidence".to_owned(),
                );
            }
            None if Instant::now() < deadline => thread::sleep(PROCESS_POLL_INTERVAL),
            None => return Err("wrapper handoff observation exceeded its deadline".to_owned()),
        }
    };
    let observation = WrapperHandoffObservation::decode(&bytes)?;
    if observation.crate_name != expected_crate_name {
        return Err(format!(
            "wrapper observed crate {:?}, expected {expected_crate_name:?}",
            observation.crate_name
        ));
    }
    let resolved_source = if observation.source_path.is_absolute() {
        observation.source_path.clone()
    } else {
        crate_root.join(&observation.source_path)
    };
    if resolved_source.canonicalize().ok()
        != Some(
            crate_root
                .join("src/lib.rs")
                .canonicalize()
                .map_err(|error| format!("canonicalize external row source: {error}"))?,
        )
    {
        return Err(
            "wrapper handoff producer source does not name the external row crate".to_owned(),
        );
    }
    let output_dir = observation
        .output_dir
        .canonicalize()
        .map_err(|error| format!("canonicalize observed compiler-handoff output: {error}"))?;
    let cargo_target = cargo_target
        .canonicalize()
        .map_err(|error| format!("canonicalize external Cargo target: {error}"))?;
    if output_dir != observation.output_dir
        || !output_dir.starts_with(&cargo_target)
        || output_dir.file_name() != Some(std::ffi::OsStr::new("fe2o3"))
    {
        return Err(
            "wrapper handoff output directory is not the brokered Cargo generation".to_owned(),
        );
    }

    let producer =
        ProducerIdentity::from_codegen(&observation.crate_name, Some(&observation.source_path))
            .map_err(|error| format!("decode wrapper-produced producer evidence: {error}"))?;
    let consumed = consume_compiler_module_handoff_v1(&output_dir, &producer, observation.attempt)
        .map_err(|error| format!("consume wrapper-observed row-softmax handoff: {error}"))?;
    let consumed_sha256: [u8; 32] = Sha256::digest(consumed.bytes()).into();
    if consumed.attempt() != observation.attempt
        || consumed.identity().as_bytes() != &consumed_sha256
        || consumed.grants_publication_authority()
        || consumed.grants_compiler_authority()
        || consumed.grants_link_authority()
        || consumed.grants_load_authority()
        || consumed.grants_launch_authority()
    {
        return Err("consumed wrapper handoff evidence or authority surface differs".to_owned());
    }
    let structural_validation =
        validate_exact_row_softmax_handoff(consumed.bytes(), |descriptor, transcript| {
            independently_expected_cargo_row_authority(
                &observation.crate_name,
                &observation.ordered_metadata,
                descriptor,
                transcript,
                &observation.attempt,
                workspace,
                &cargo_target,
                broker,
            )
        });
    if !matches!(
        consume_compiler_module_handoff_v1(&output_dir, &producer, observation.attempt),
        Err(CompilerModuleHandoffErrorV1::AlreadyConsumed)
    ) {
        return Err("wrapper-observed compiler handoff was not one-shot".to_owned());
    }

    let mut ack = Vec::with_capacity(HANDOFF_OBSERVATION_ACK_MAGIC.len() + 64);
    ack.extend_from_slice(HANDOFF_OBSERVATION_ACK_MAGIC);
    ack.extend_from_slice(&observation.observation_sha256);
    ack.extend_from_slice(consumed.identity().as_bytes());
    write_private_observation_ack(observation_directory, &ack)?;
    structural_validation
}

fn read_private_observation(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("open wrapper handoff observation: {error}")),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect wrapper handoff observation: {error}"))?;
    if !metadata.is_file()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err("wrapper handoff observation is not a private regular file".to_owned());
    }
    let length = usize::try_from(metadata.len())
        .map_err(|_| "wrapper handoff observation length overflow".to_owned())?;
    if length == 0 {
        return Ok(None);
    }
    if length > MAX_HANDOFF_OBSERVATION_BYTES {
        return Err("wrapper handoff observation exceeds its byte bound".to_owned());
    }
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("read wrapper handoff observation: {error}"))?;
    Ok(Some(bytes))
}

fn write_private_observation_ack(directory: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = directory.join(".ack.tmp");
    let ack = directory.join("ack");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temp)
        .map_err(|error| format!("create wrapper handoff observation ack: {error}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("persist wrapper handoff observation ack: {error}"))?;
    std::fs::rename(&temp, &ack)
        .map_err(|error| format!("publish wrapper handoff observation ack: {error}"))?;
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("sync wrapper handoff observation directory: {error}"))
}

struct ExternalRowSoftmaxSpec<'a> {
    package_name: &'a str,
    source: &'a str,
    target: &'a str,
    extra_rustflags: &'a [&'a str],
    device_root: &'a Path,
    host_root: &'a Path,
}

fn compile_external_row_softmax_crate(
    workspace: &Path,
    cargo_target: &Path,
    spec: ExternalRowSoftmaxSpec<'_>,
) -> (Output, TestOutputDir) {
    let broker = build_and_pin_broker(workspace, cargo_target, false);
    compile_external_row_softmax_crate_with_broker(workspace, cargo_target, spec, &broker, None)
}

fn compile_external_row_softmax_crate_with_broker(
    workspace: &Path,
    cargo_target: &Path,
    spec: ExternalRowSoftmaxSpec<'_>,
    broker: &PinnedBrokerExecutable,
    metadata_mutation: Option<&str>,
) -> (Output, TestOutputDir) {
    let output = TestOutputDir::new(workspace);
    let crate_root = output.0.join(spec.package_name);
    let source_directory = crate_root.join("src");
    std::fs::create_dir_all(&source_directory).expect("create external row-softmax source root");
    let source = source_directory.join("lib.rs");
    std::fs::write(&source, spec.source).expect("write external row-softmax source");
    let manifest = crate_root.join("Cargo.toml");
    std::fs::write(
        &manifest,
        format!(
            "[package]\nname = {:?}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\nfe2o3-host = {{ path = {:?} }}\n\n[workspace]\n",
            spec.package_name, spec.device_root, spec.host_root,
        ),
    )
    .expect("write external row-softmax manifest");
    generate_external_lockfile(&manifest);

    let mut rustflags = format!(
        "-Coverflow-checks=off -Cmetadata=fe2o3-row-softmax-v1-reviewed --remap-path-prefix={}=/fe2o3-reviewed-workspace/row-softmax-v1.rs",
        source.display()
    );
    for flag in spec.extra_rustflags {
        rustflags.push(' ');
        rustflags.push_str(flag);
    }
    let mut command = broker
        .command()
        .expect("verify test-owned cargo-fe2o3 before launch");
    command
        .current_dir(workspace)
        .args(["build", "--manifest-path"])
        .arg(&manifest)
        .args(["--target-dir"])
        .arg(cargo_target)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("FE2O3_TARGET", spec.target)
        .env("FE2O3_CODEGEN_PIPELINE", ROW_SOFTMAX_PIPELINE)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"))
        .env("RUSTFLAGS", rustflags);
    match metadata_mutation {
        Some(mutation) => {
            command.env(CARGO_METADATA_MUTATION_TEST_ONLY_ENV, mutation);
        }
        None => {
            command.env_remove(CARGO_METADATA_MUTATION_TEST_ONLY_ENV);
        }
    }
    configure_unprotected_row_authority_validation(&mut command, cargo_target);
    scrub_test_dynamic_loader_environment(&mut command);
    let compiled = run_bounded(
        &mut command,
        BACKEND_BUILD_TIMEOUT,
        "clean external cargo-fe2o3 row-softmax crate",
    )
    .expect("run clean external row-softmax crate within deadline");
    (compiled, output)
}

fn compile_clean_external_row_softmax_crate(
    workspace: &Path,
    cargo_target: &Path,
    package_name: &str,
) -> (Output, TestOutputDir) {
    compile_external_row_softmax_crate(
        workspace,
        cargo_target,
        ExternalRowSoftmaxSpec {
            package_name,
            source: ROW_SOFTMAX_FIXTURE,
            target: "gfx942:xnack-",
            extra_rustflags: &[],
            device_root: &workspace.join("crates/fe2o3-device"),
            host_root: &workspace.join("crates/fe2o3-host"),
        },
    )
}

fn build_and_pin_broker(
    workspace: &Path,
    cargo_target: &Path,
    handoff_observation: bool,
) -> Arc<PinnedBrokerExecutable> {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "cargo-fe2o3"]);
    if handoff_observation {
        command.args(["--features", "compiler-handoff-observation-test-only"]);
    }
    command
        .args(["--bin", "cargo-fe2o3"])
        .env("CARGO_TARGET_DIR", cargo_target)
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS");
    let built = run_bounded(
        &mut command,
        BACKEND_BUILD_TIMEOUT,
        "build cargo-fe2o3 before pinning its test oracle",
    )
    .expect("build cargo-fe2o3 before pinning within deadline");
    assert!(
        built.status.success(),
        "failed to build cargo-fe2o3 before pinning:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr),
    );
    let broker = cargo_target.join("debug/cargo-fe2o3");
    std::fs::set_permissions(&broker, std::fs::Permissions::from_mode(0o500))
        .expect("make the test-owned cargo-fe2o3 object non-writable before pinning");
    let broker = Arc::new(
        PinnedBrokerExecutable::open(&broker).expect("pin the exact built cargo-fe2o3 object"),
    );

    let mut backend_command = Command::new(env!("CARGO"));
    backend_command
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"]);
    if handoff_observation {
        backend_command.args(["--features", "row-softmax-metadata-mutation-test-only"]);
    }
    backend_command
        .env("CARGO_TARGET_DIR", cargo_target)
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .env(
            "FE2O3_BUILD_CARGO_FE2O3_EXECUTABLE_SHA256_V1",
            encode_lower_hex(
                &broker
                    .sha256()
                    .expect("verify broker before binding the backend build"),
            ),
        )
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS");
    let backend = run_bounded(
        &mut backend_command,
        BACKEND_BUILD_TIMEOUT,
        "build test backend bound to the pinned cargo-fe2o3 object",
    )
    .expect("build broker-bound test backend within deadline");
    assert!(
        backend.status.success(),
        "failed to build broker-bound test backend:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&backend.stdout),
        String::from_utf8_lossy(&backend.stderr),
    );
    broker
}

fn build_and_pin_handoff_broker(
    workspace: &Path,
    cargo_target: &Path,
) -> Arc<PinnedBrokerExecutable> {
    build_and_pin_broker(workspace, cargo_target, true)
}

fn scrub_test_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        let bytes = name.as_bytes();
        if bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_") || bytes == b"GLIBC_TUNABLES" {
            command.env_remove(name);
        }
    }
}

fn generate_external_lockfile(manifest: &Path) {
    let generated = run_bounded(
        Command::new(env!("CARGO"))
            .args(["generate-lockfile", "--offline", "--manifest-path"])
            .arg(manifest),
        BACKEND_BUILD_TIMEOUT,
        "generate external source-validation lockfile",
    )
    .expect("generate external source-validation lockfile within deadline");
    assert!(
        generated.status.success(),
        "failed to generate external source-validation lockfile:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr),
    );
}

fn authority_toolchain() -> &'static AuthorityToolchain {
    AUTHORITY_TOOLCHAIN.get_or_init(|| {
        let cargo = std::fs::canonicalize(env!("CARGO")).expect("canonical Cargo executable");
        let rustc = std::fs::canonicalize(
            cargo
                .parent()
                .expect("Cargo executable has a bin directory")
                .join("rustc"),
        )
        .expect("canonical rustc executable beside Cargo");
        let rustc_runtime = rustc
            .parent()
            .and_then(Path::parent)
            .expect("rustc executable has a toolchain root")
            .join("lib");
        AuthorityToolchain {
            cargo_sha256: sha256_path(&cargo),
            rustc_sha256: sha256_path(&rustc),
            rustc_runtime_sha256: runtime_tree_sha256(&rustc_runtime),
            cargo,
            rustc,
        }
    })
}

fn configure_unprotected_row_authority_validation(command: &mut Command, cargo_target: &Path) {
    let toolchain = authority_toolchain();
    let backend = cargo_target.join("debug/librustc_codegen_fe2o3.so");
    assert!(backend.is_file(), "test-owned authority backend is absent");
    command
        .env("CARGO", &toolchain.cargo)
        .env("FE2O3_BACKEND", &backend)
        .env(
            "FE2O3_AUTHORITY_CARGO_SHA256_V1",
            encode_lower_hex(&toolchain.cargo_sha256),
        )
        .env("FE2O3_AUTHORITY_RUSTC_PATH_V1", &toolchain.rustc)
        .env(
            "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
            encode_lower_hex(&toolchain.rustc_sha256),
        )
        .env(
            "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
            encode_lower_hex(&toolchain.rustc_runtime_sha256),
        )
        .env(
            "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
            encode_lower_hex(&sha256_path(&backend)),
        )
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        );
    for (name, _) in std::env::vars_os() {
        if name.as_bytes().starts_with(b"RUSTUP_") {
            command.env_remove(name);
        }
    }
    for name in [
        "RUSTC",
        "CARGO_BUILD_RUSTC",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(name);
    }
}

fn sha256_path(path: &Path) -> [u8; 32] {
    Sha256::digest(std::fs::read(path).expect("read authority-validation tool")).into()
}

fn runtime_tree_sha256(root: &Path) -> [u8; 32] {
    fn hash_field(hash: &mut Sha256, value: &[u8]) {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }

    fn hash_directory(hash: &mut Sha256, directory: &Path) {
        let mut entries = std::fs::read_dir(directory)
            .expect("read rustc runtime tree")
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
            let metadata = std::fs::symlink_metadata(&path).expect("inspect rustc runtime entry");
            if metadata.is_file() {
                let bytes = std::fs::read(&path).expect("read rustc runtime entry");
                hash.update(b"file\0");
                hash.update((metadata.mode() & 0o7777).to_le_bytes());
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            } else if metadata.is_dir() {
                hash.update(b"subdirectory\0");
                hash.update((metadata.mode() & 0o7777).to_le_bytes());
                hash_directory(hash, &path);
            } else {
                panic!("unsupported rustc runtime entry {path:?}");
            }
        }
        hash.update(b"end-directory\0");
    }

    let mut hash = Sha256::new();
    hash.update(b"fe2o3-rustc-runtime-tree-v1\0");
    hash_directory(&mut hash, root);
    hash.finalize().into()
}

fn substitute_handoff_broker_path(cargo_target: &Path, broker: &PinnedBrokerExecutable) -> PathBuf {
    broker
        .verify()
        .expect("verify broker immediately before pathname substitution");
    let broker_path = cargo_target.join("debug/cargo-fe2o3");
    let retained_path = cargo_target.join("debug/.cargo-fe2o3-retained-test-object");
    std::fs::rename(&broker_path, &retained_path)
        .expect("retain the pinned broker object under a different pathname");

    let mut replacement = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&broker_path)
        .expect("install same-UID broker pathname substitution");
    writeln!(
        replacement,
        "#!/bin/sh\nprintf '%s\\n' '{BROKER_PATH_SUBSTITUTION_MARKER}' >&2\nexit 97"
    )
    .expect("write hostile broker pathname substitution");
    replacement
        .sync_all()
        .expect("persist hostile broker pathname substitution");
    File::open(broker_path.parent().expect("broker has a parent directory"))
        .and_then(|directory| directory.sync_all())
        .expect("persist broker pathname substitution directory entry");

    let replacement_metadata = replacement
        .metadata()
        .expect("inspect hostile broker pathname substitution");
    assert_eq!(replacement_metadata.uid(), unsafe { libc::geteuid() });
    drop(replacement);
    let replacement_sha256: [u8; 32] = Sha256::digest(
        std::fs::read(&broker_path).expect("read hostile broker pathname substitution"),
    )
    .into();
    assert_ne!(
        replacement_sha256,
        broker
            .sha256()
            .expect("hash retained broker after pathname substitution")
    );
    let replaced = run_bounded(
        &mut Command::new(&broker_path),
        Duration::from_secs(10),
        "hostile broker pathname substitution probe",
    )
    .expect("run hostile broker pathname substitution probe");
    assert_eq!(replaced.status.code(), Some(97));
    assert!(stderr(&replaced).contains(BROKER_PATH_SUBSTITUTION_MARKER));
    broker
        .verify()
        .expect("pathname substitution changed the retained broker object");
    retained_path
}

fn compile_clean_external_row_softmax_crate_with_handoff(
    workspace: &Path,
    cargo_target: &Path,
    broker: &Arc<PinnedBrokerExecutable>,
    package_name: &str,
) -> (Output, TestOutputDir) {
    let output = TestOutputDir::new(workspace);
    let crate_root = output.0.join(package_name);
    let source_directory = crate_root.join("src");
    std::fs::create_dir_all(&source_directory).expect("create external row-softmax source root");
    let source = source_directory.join("lib.rs");
    std::fs::write(&source, ROW_SOFTMAX_FIXTURE).expect("write external row-softmax source");
    let manifest = crate_root.join("Cargo.toml");
    std::fs::write(
        &manifest,
        format!(
            "[package]\nname = {package_name:?}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\nfe2o3-host = {{ path = {:?} }}\n\n[workspace]\n",
            workspace.join("crates/fe2o3-device"),
            workspace.join("crates/fe2o3-host"),
        ),
    )
    .expect("write external row-softmax manifest");
    generate_external_lockfile(&manifest);
    let observation_directory = output.0.join("handoff-observation");
    let mut observation_builder = std::fs::DirBuilder::new();
    observation_builder.mode(0o700);
    observation_builder
        .create(&observation_directory)
        .expect("create private wrapper handoff observation directory");
    let expected_crate_name = package_name.replace('-', "_");
    let (cancel_sender, cancel_receiver) = std::sync::mpsc::channel();
    let observer_directory = observation_directory.clone();
    let observer_crate = expected_crate_name.clone();
    let observer_root = crate_root.clone();
    let observer_workspace = workspace.to_path_buf();
    let observer_target = cargo_target.to_path_buf();
    let observer_broker = Arc::clone(broker);
    let observer = thread::spawn(move || {
        observe_and_validate_external_row_handoff(
            &observer_directory,
            &observer_crate,
            &observer_root,
            &observer_workspace,
            &observer_target,
            &observer_broker,
            &cancel_receiver,
        )
    });

    let rustflags = format!(
        "-Coverflow-checks=off -Cmetadata=fe2o3-row-softmax-v1-reviewed --remap-path-prefix={}=/fe2o3-reviewed-workspace/row-softmax-v1.rs",
        source.display()
    );
    let mut command = broker
        .command()
        .expect("verify pinned cargo-fe2o3 before launch");
    command
        .current_dir(workspace)
        .args(["build", "--manifest-path"])
        .arg(&manifest)
        .args(["--target-dir"])
        .arg(cargo_target)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", ROW_SOFTMAX_PIPELINE)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"))
        .env(HANDOFF_OBSERVATION_DIRECTORY_ENV, &observation_directory)
        .env(HANDOFF_OBSERVATION_CRATE_ENV, &expected_crate_name)
        .env("RUSTFLAGS", rustflags);
    configure_unprotected_row_authority_validation(&mut command, cargo_target);
    scrub_test_dynamic_loader_environment(&mut command);
    let compiled = run_bounded(
        &mut command,
        BACKEND_BUILD_TIMEOUT,
        "clean external cargo-fe2o3 row-softmax crate",
    );
    let _ = cancel_sender.send(());
    let observation = observer
        .join()
        .expect("wrapper handoff observer thread did not panic");
    let compiled = compiled.expect("run clean external row-softmax crate within deadline");
    if let Err(error) = observation {
        panic!(
            "wrapper handoff observation failed: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&compiled.stdout),
            String::from_utf8_lossy(&compiled.stderr),
        );
    }
    (compiled, output)
}

fn admitted_row_softmax_root(stderr: &str) -> Option<&str> {
    let marker = "exact collected KernelEntry `";
    let tail = stderr.rsplit_once(marker)?.1;
    tail.split_once('`').map(|(root, _)| root)
}

fn copy_source_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create copied source directory");
    for entry in std::fs::read_dir(source).expect("read source directory") {
        let entry = entry.expect("read source entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("read source entry type").is_dir() {
            copy_source_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy source file");
        }
    }
}

fn prepare_hostile_same_name_device_provider(
    workspace: &Path,
    output: &TestOutputDir,
) -> (PathBuf, PathBuf) {
    let crate_root = output.0.join("hostile-fe2o3-device");
    copy_source_tree(
        &workspace.join("crates/fe2o3-device/src"),
        &crate_root.join("src"),
    );
    std::fs::write(
        crate_root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-device\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nfe2o3-macros = {{ path = {:?} }}\n\n[lib]\nname = \"fe2o3_device\"\n",
            workspace.join("crates/fe2o3-macros")
        ),
    )
    .expect("write hostile provider manifest");
    let host_root = output.0.join("hostile-fe2o3-host");
    std::fs::create_dir_all(host_root.join("src")).expect("create hostile host source root");
    std::fs::write(
        host_root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-host\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\n\n[lib]\nname = \"fe2o3_host\"\n",
            crate_root
        ),
    )
    .expect("write hostile host manifest");
    std::fs::write(
        host_root.join("src/lib.rs"),
        r"#![no_std]

pub mod __generated {
    use core::marker::PhantomData;

    pub struct GeneratedReadDeviceSlice<'allocation, T>(PhantomData<&'allocation T>);
    pub struct GeneratedReadWriteDeviceSlice<'allocation, T>(PhantomData<&'allocation mut T>);

    #[derive(Clone, Copy)]
    pub enum CompilerGeneratedKernelProfileV1 {
        ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: [u8; 32],
        },
    }

    pub struct ValidatedCompilerGeneratedSemanticWitnessV1;
    pub struct CompilerGeneratedSemanticWitnessErrorV1;

    pub unsafe trait CompilerGeneratedKernelExpectationV1:
        fe2o3_device::KernelMarkerV1
    {
        const PROFILE: CompilerGeneratedKernelProfileV1;
        const KERNEL_BINDING_ID_V1: [u8; 32];
        fn semantic_witness_v1() -> Result<
            ValidatedCompilerGeneratedSemanticWitnessV1,
            CompilerGeneratedSemanticWitnessErrorV1,
        >;
    }

    pub unsafe fn semantic_witness_from_backend_v1(
        _pointer: *const u8,
        _length: usize,
        _binding: [u8; 32],
        _contract: [u8; 32],
    ) -> Result<
        ValidatedCompilerGeneratedSemanticWitnessV1,
        CompilerGeneratedSemanticWitnessErrorV1,
    > {
        Err(CompilerGeneratedSemanticWitnessErrorV1)
    }
}
",
    )
    .expect("write hostile host source");
    std::fs::write(
        output.0.join("Cargo.toml"),
        "[workspace]\nmembers = [\"hostile-fe2o3-device\", \"hostile-fe2o3-host\"]\nresolver = \"3\"\n",
    )
    .expect("write hostile provider workspace manifest");
    (crate_root, host_root)
}

fn assert_tiled_gemm_published_no_handoff(output: &TestOutputDir) {
    let artifacts = std::fs::read_dir(output.0.join("artifacts"))
        .expect("read tiled GEMM artifact directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("enumerate tiled GEMM artifacts");
    assert!(
        artifacts.iter().all(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .contains("compiler-module")
        }),
        "rejected tiled GEMM published a compiler handoff: {:?}",
        artifacts
            .iter()
            .map(|entry| entry.path())
            .collect::<Vec<_>>()
    );
}

fn consume_tiled_gemm_handoff(
    output: &TestOutputDir,
    source: &str,
    crate_name: &str,
) -> CompilerModuleHandoffV2 {
    let source_path = output.0.join("tiled-gemm-v1.rs");
    let producer = ProducerIdentity::from_codegen(crate_name, Some(&source_path))
        .expect("tiled GEMM handoff producer");
    let attempt = begin_build_attempt(
        &output.0.join("artifacts"),
        &producer,
        BuildInvocation::from_bytes(Sha256::digest(source.as_bytes()).into()),
        BuildSession::from_bytes([
            0x54, 0x47, 0x56, 0x31, 0x54, 0x47, 0x56, 0x31, 0x54, 0x47, 0x56, 0x31, 0x54, 0x47,
            0x56, 0x31,
        ]),
    )
    .expect("resume tiled GEMM handoff attempt");
    let consumed =
        consume_compiler_module_handoff_v1(&output.0.join("artifacts"), &producer, attempt)
            .expect("consume exact tiled GEMM handoff");
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .expect("decode exact tiled GEMM Worker V2 handoff");
    assert_eq!(handoff.canonical_bytes(), consumed.bytes());
    handoff
}

fn assert_scalar_gemm_published_no_handoff(output: &TestOutputDir) {
    assert!(
        !output.0.join("scalar-gemm-v1").exists(),
        "rejected scalar GEMM emitted a linked output"
    );
    let artifacts = std::fs::read_dir(output.0.join("artifacts"))
        .expect("read scalar GEMM artifact directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("enumerate scalar GEMM artifacts");
    assert!(
        artifacts.iter().all(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .contains("compiler-module")
        }),
        "rejected scalar GEMM published a compiler handoff: {:?}",
        artifacts
            .iter()
            .map(|entry| entry.path())
            .collect::<Vec<_>>()
    );
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn hex_after(text: &str, marker: &str) -> String {
    text.split_once(marker)
        .unwrap_or_else(|| panic!("missing digest marker {marker:?}\n{text}"))
        .1
        .chars()
        .take_while(|character| character.is_ascii_hexdigit())
        .take(64)
        .collect()
}

fn assert_rejected_without_fallback(output: &Output, expected: &str) {
    let stderr = stderr(output);
    assert!(!output.status.success(), "unexpected success\n{stderr}");
    assert!(
        stderr.contains(expected),
        "missing `{expected}` diagnostic\n{stderr}"
    );
    assert!(
        !stderr.contains("unsupported kernel shape for AMDGPU LLVM IR MVP")
            && !stderr.contains("selected legacy-v1")
            && !stderr.contains("emitted scalar_control_flow_v1"),
        "rejection entered a legacy/artifact fallback\n{stderr}"
    );
}

#[test]
#[ignore = "invoked only through the private user/mount namespace build protocol"]
fn isolated_backend_build_helper() {
    if std::env::var_os(BUILD_HELPER_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    let workspace = PathBuf::from(
        std::env::var_os(BUILD_HELPER_WORKSPACE_ENV).expect("isolated build workspace"),
    );
    let mount = PathBuf::from(
        std::env::var_os(BUILD_HELPER_MOUNT_ENV).expect("isolated build mount point"),
    );
    let socket_descriptor: RawFd = std::env::var(BUILD_HELPER_SOCKET_ENV)
        .expect("isolated build socket descriptor")
        .parse()
        .expect("numeric isolated build socket descriptor");
    mount_private_build_tmpfs(&mount).expect("isolate backend build output in a private tmpfs");
    let target_dir = mount.join("target");
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(&workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .env("CARGO_INCREMENTAL", "0");
    let output = run_bounded(
        &mut command,
        BACKEND_BUILD_TIMEOUT,
        "private namespace backend cargo build",
    )
    .expect("build rustc backend in private mount namespace within deadline");
    assert!(
        output.status.success(),
        "isolated backend build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pinned = pin_backend(&target_dir.join("debug/librustc_codegen_fe2o3.so"))
        .expect("pin namespaced backend in a sealed memfd");
    send_backend_descriptor(socket_descriptor, pinned.file.as_raw_fd())
        .expect("transfer sealed backend descriptor");
}

#[test]
fn backend_helper_early_exit_fails_within_deadline() {
    let (parent_socket, child_socket) = UnixDatagram::pair().expect("create regression FD socket");
    set_close_on_exec(child_socket.as_raw_fd(), false)
        .expect("make regression child socket inheritable");
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exit 23"]);
    let child = CapturedChild::spawn(&mut command, "early-exit regression helper")
        .expect("spawn early-exit regression helper");
    drop(child_socket);
    let started = Instant::now();
    let error = receive_backend_from_child(
        child,
        parent_socket.as_raw_fd(),
        started + Duration::from_secs(2),
        "early-exit regression helper",
    )
    .expect_err("helper exit before descriptor transfer must fail closed");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "early helper exit was not detected before the deadline"
    );
    assert!(
        error.contains("exited before transferring a descriptor") && error.contains("status: 23"),
        "unexpected early-exit diagnostic: {error}"
    );
}

#[test]
fn namespace_probe_skips_only_known_host_policy_denials() {
    assert!(is_known_namespace_policy_denial(
        "unshare: write failed /proc/self/uid_map: Operation not permitted"
    ));
    assert!(is_known_namespace_policy_denial(
        "unshare: unshare failed: Operation not permitted"
    ));
    for unexpected in [
        "unshare: command not found",
        "unshare: write failed /proc/self/gid_map: Invalid argument",
        "backend build failed",
        "",
    ] {
        assert!(!is_known_namespace_policy_denial(unexpected));
    }
}

#[test]
fn subprocess_timeout_reaps_its_descendant_group() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    let pid_file = output.0.join("timed-out-descendant.pid");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "sleep 30 & echo $! > \"$PID_FILE\"; wait"])
        .env("PID_FILE", &pid_file);
    let error = run_bounded(
        &mut command,
        Duration::from_millis(200),
        "descendant cleanup regression",
    )
    .expect_err("long-running subprocess must hit its deadline");
    assert!(
        error.contains("exceeded its monotonic deadline"),
        "unexpected timeout diagnostic: {error}"
    );
    let descendant: libc::pid_t = std::fs::read_to_string(&pid_file)
        .expect("read timed-out descendant PID")
        .trim()
        .parse()
        .expect("numeric timed-out descendant PID");
    let reap_deadline = Instant::now() + TERMINATION_GRACE;
    loop {
        let probe = unsafe { libc::kill(descendant, 0) };
        let error = std::io::Error::last_os_error();
        if probe == -1 && error.raw_os_error() == Some(libc::ESRCH) {
            break;
        }
        assert_eq!(probe, 0, "probe timed-out descendant {descendant}: {error}");
        assert!(
            Instant::now() < reap_deadline,
            "timed-out descendant {descendant} survived bounded cleanup"
        );
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

#[test]
fn authenticated_fixture_seals_semantics_then_stops_before_executable_authority() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let backend = build_backend(&workspace);
    let output = TestOutputDir::new(&workspace);
    let fixture =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/executable-scalar-control-flow-v1.rs");
    let compiled = compile_path(
        &workspace,
        backend,
        &output,
        fixture,
        "gfx942:xnack-",
        PIPELINE,
        &[],
    );
    let baseline_stderr = stderr(&compiled);
    assert!(
        !compiled.status.success(),
        "unexpected success\n{baseline_stderr}"
    );
    assert!(
        baseline_stderr.contains("[kernel] scalar_control_flow_v1"),
        "{baseline_stderr}"
    );
    assert!(
        baseline_stderr.contains("[internal-helper]"),
        "{baseline_stderr}"
    );
    assert!(
        baseline_stderr.contains(&format!("{PIPELINE} authenticated collected KernelEntry")),
        "missing authenticated export diagnostic\n{baseline_stderr}"
    );
    assert!(baseline_stderr.contains("exact reachable InternalHelper"));
    assert!(baseline_stderr.contains("path-independent portable MIR semantics"));
    assert!(baseline_stderr.contains("compiler semantics"));
    assert!(baseline_stderr.contains("sealed collected authority"));
    assert!(baseline_stderr.contains("executable-MIR capture/import"));
    assert!(baseline_stderr.contains(
        "no executable authority, Kernel IR, LLVM, LLD, HSACO, or legacy fallback was entered"
    ));
    eprintln!(
        "V2_IDENTITIES root={} helper={} portable={} compiler={} authority={}",
        hex_after(&baseline_stderr, "exact reviewed root MIR "),
        hex_after(
            &baseline_stderr,
            "exact reachable InternalHelper `nested_match_helper` MIR ",
        ),
        hex_after(&baseline_stderr, "path-independent portable MIR semantics "),
        hex_after(&baseline_stderr, "compiler semantics "),
        hex_after(&baseline_stderr, "sealed collected authority "),
    );
    assert!(!baseline_stderr.contains("define amdgpu_kernel"));
    assert_eq!(
        std::fs::read_dir(output.0.join("artifacts"))
            .expect("read empty artifact directory")
            .count(),
        0,
        "admission-only slice must not claim an artifact"
    );

    let repeated_output = TestOutputDir::new(&workspace);
    let repeated = compile_path(
        &workspace,
        backend,
        &repeated_output,
        fixture,
        "gfx942:xnack-",
        PIPELINE,
        &[],
    );
    let repeated_stderr = stderr(&repeated);
    assert!(
        !repeated.status.success(),
        "unexpected success\n{repeated_stderr}"
    );
    assert_eq!(
        hex_after(&baseline_stderr, "path-independent portable MIR semantics "),
        hex_after(&repeated_stderr, "path-independent portable MIR semantics ")
    );
    assert_eq!(
        hex_after(&baseline_stderr, "sealed collected authority "),
        hex_after(&repeated_stderr, "sealed collected authority ")
    );
    assert_eq!(
        hex_after(&baseline_stderr, "exact reviewed root MIR "),
        hex_after(&repeated_stderr, "exact reviewed root MIR "),
        "canonical source remapping must make full rustc MIR identity path independent"
    );
}

#[test]
fn target_pipeline_identity_abi_and_collection_substitutions_reject_without_fallback() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let backend = build_backend(&workspace);

    let wrong_target = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_target,
            FIXTURE,
            "gfx942:xnack+",
            PIPELINE,
        ),
        "requires exact target `gfx942:xnack-`, found `gfx942:xnack+`",
    );

    let custom_pipeline = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &custom_pipeline,
            FIXTURE,
            "gfx942:xnack-",
            "collected-executable-scalar-control-flow-v2-custom",
        ),
        "FE2O3_CODEGEN_PIPELINE must be unset or exactly",
    );

    let custom_llvm = TestOutputDir::new(&workspace);
    let fixture =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/executable-scalar-control-flow-v1.rs");
    assert_rejected_without_fallback(
        &compile_path(
            &workspace,
            backend,
            &custom_llvm,
            fixture,
            "gfx942:xnack-",
            PIPELINE,
            &["-Cpasses=default<O1>"],
        ),
        "rejects custom LLVM pipeline selection",
    );

    for (extra_arg, expected) in [
        (
            "-Cpanic=abort",
            "compiler semantics mismatch: panic strategy must be Unwind, found Abort",
        ),
        (
            "-Copt-level=1",
            "compiler semantics mismatch: rustc optimization must be No/0",
        ),
        (
            "-Zmir-opt-level=2",
            "compiler semantics mismatch: effective MIR optimization level must be 1",
        ),
        (
            "-Ctarget-cpu=native",
            "compiler semantics mismatch: rustc target CPU/features must be unset",
        ),
        (
            "-Coverflow-checks=on",
            "unsupported executable MIR edge `assert(",
        ),
        (
            "-Cdebug-assertions=no",
            "compiler semantics mismatch: debug assertions must be enabled",
        ),
        (
            "--remap-path-prefix=/tmp=/attacker",
            "compiler semantics mismatch: source remapping must contain exactly one canonical fixture destination",
        ),
        (
            "-Cmetadata=attacker",
            "compiler semantics mismatch: crate metadata must be exactly",
        ),
    ] {
        let output = TestOutputDir::new(&workspace);
        assert_rejected_without_fallback(
            &compile_path(
                &workspace,
                backend,
                &output,
                fixture,
                "gfx942:xnack-",
                PIPELINE,
                &[extra_arg],
            ),
            expected,
        );
    }

    let wrong_abi_source = FIXTURE
        .replace(
            "pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u32)",
            "pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u64)",
        )
        .replace(
            "nested_match_helper(limit);",
            "nested_match_helper(limit as u32);",
        )
        .replace("    fn(u32),", "    fn(u64),");
    let wrong_abi = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_abi,
            &wrong_abi_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "root ABI mismatch",
    );

    let wrong_helper_source = FIXTURE.replace("_ => sum += inner,", "_ => sum += inner + 1,");
    let wrong_helper = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_helper,
            &wrong_helper_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "MIR identity mismatch",
    );

    let wrong_helper_type_source = FIXTURE
        .replace(
            "fn nested_match_helper(limit: u32) -> u32",
            "fn nested_match_helper(limit: u64) -> u64",
        )
        .replace("0_u32", "0_u64")
        .replace(
            "nested_match_helper(limit);",
            "nested_match_helper(limit as u64);",
        );
    let wrong_helper_type = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_helper_type,
            &wrong_helper_type_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "helper ABI mismatch",
    );

    for changed_helper in [
        FIXTURE.replace("_ => sum += inner,", "_ => sum *= inner,"),
        FIXTURE.replace(
            "_ => sum += inner,",
            "_ => { if inner == 7 { sum += 1; } sum += inner },",
        ),
    ] {
        let output = TestOutputDir::new(&workspace);
        assert_rejected_without_fallback(
            &compile(
                &workspace,
                backend,
                &output,
                &changed_helper,
                "gfx942:xnack-",
                PIPELINE,
            ),
            "MIR identity mismatch",
        );
    }

    let additional_root_source = FIXTURE.replace(
        "fn main() {}",
        r#"
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_scalar_control_flow_extra(_: u32) {}

#[used]
#[allow(non_upper_case_globals)]
static __fe2o3_kernel_registration_scalar_control_flow_extra: (
    u64, u16, u16, &'static str, &'static str, fn(u32),
) = (
    0x4e52_4b33_4f32_4546, 1, 1,
    "scalar_control_flow_extra", "scalar_control_flow_extra",
    fe2o3_kernel_scalar_control_flow_extra,
);

fn main() {}
"#,
    );
    let additional_root = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &additional_root,
            &additional_root_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "requires exactly two collected functions, found 3",
    );
}

#[test]
fn scalar_gemm_v1_frontend_receipt_selects_only_the_reviewed_full_portable_mir() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let backend = build_backend(&workspace);
    let output = TestOutputDir::new(&workspace);
    let compiled = compile_scalar_gemm(
        &workspace,
        backend,
        &output,
        SCALAR_GEMM_FIXTURE,
        "gfx942:xnack-",
        &[],
    );
    let admission_stderr = stderr(&compiled);
    assert!(
        compiled.status.success()
            && admission_stderr.contains("consumed its single-use frontend receipt")
            && admission_stderr.contains("af4ca76c4517b779bca4b7a63bcae09a23cad947e740b2e51f872d7cc0d6d002")
            && admission_stderr.contains("published exact inert Worker V2 compiler-module handoff")
            && admission_stderr.contains("compiler descriptor and frontend-authority sections")
            && admission_stderr.contains("measured Worker execution, raw-HSACO inspection, finalization, durable HSACO publication, load, launch, and COMGR were not entered by the backend"),
        "reviewed scalar GEMM did not publish its authenticated handoff:\n{admission_stderr}"
    );
    assert!(output.0.join("scalar-gemm-v1").is_file());
    assert!(
        std::fs::read_dir(output.0.join("artifacts"))
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("compiler-module"))
    );

    let mutated_source = SCALAR_GEMM_FIXTURE.replace(
        "let product = a[a_index] * b[b_index];",
        "let product = a[a_index] + b[b_index];",
    );
    assert_ne!(mutated_source, SCALAR_GEMM_FIXTURE);
    let mutated_output = TestOutputDir::new(&workspace);
    let mutated = compile_scalar_gemm(
        &workspace,
        backend,
        &mutated_output,
        &mutated_source,
        "gfx942:xnack-",
        &[],
    );
    let mutated_stderr = stderr(&mutated);
    assert!(
        !mutated.status.success()
            && mutated_stderr.contains("portable MIR identity mismatch")
            && !mutated_stderr.contains("published exact inert Worker V2 compiler-module handoff"),
        "same-shape arithmetic mutation was not rejected by full portable MIR identity:\n{mutated_stderr}"
    );
    assert_scalar_gemm_published_no_handoff(&mutated_output);

    let copied_digest_source = mutated_source.replacen(
        "use fe2o3_device",
        "const CLAIMED_PORTABLE_MIR: &str = \"af4ca76c4517b779bca4b7a63bcae09a23cad947e740b2e51f872d7cc0d6d002\";\nuse fe2o3_device",
        1,
    );
    assert_ne!(copied_digest_source, mutated_source);
    let copied_digest_output = TestOutputDir::new(&workspace);
    let copied_digest = compile_scalar_gemm(
        &workspace,
        backend,
        &copied_digest_output,
        &copied_digest_source,
        "gfx942:xnack-",
        &[],
    );
    let copied_digest_stderr = stderr(&copied_digest);
    assert!(
        !copied_digest.status.success()
            && copied_digest_stderr.contains("portable MIR identity mismatch")
            && !copied_digest_stderr
                .contains("published exact inert Worker V2 compiler-module handoff"),
        "a copied digest claim minted frontend authority:\n{copied_digest_stderr}"
    );
    assert_scalar_gemm_published_no_handoff(&copied_digest_output);

    let wrong_target_output = TestOutputDir::new(&workspace);
    let wrong_target = compile_scalar_gemm(
        &workspace,
        backend,
        &wrong_target_output,
        SCALAR_GEMM_FIXTURE,
        "gfx942:xnack+",
        &[],
    );
    let wrong_target_stderr = stderr(&wrong_target);
    assert!(
        !wrong_target.status.success()
            && wrong_target_stderr.contains("requires exact target `gfx942:xnack-`")
            && !wrong_target_stderr
                .contains("published exact inert Worker V2 compiler-module handoff"),
        "wrong target minted frontend authority:\n{wrong_target_stderr}"
    );
    assert_scalar_gemm_published_no_handoff(&wrong_target_output);
}

#[test]
fn tiled_gemm_v1_source_authentication_and_adversaries_fail_closed() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let backend = build_backend(&workspace);
    assert!(TILED_GEMM_FIXTURE.contains("launch(required = [64, 1, 1], max = [64, 1, 1])"));
    assert!(
        !TILED_GEMM_FIXTURE.contains("static __fe2o3_kernel_frontend_contract_v1_tiled_gemm_v1")
    );

    let exact_output = TestOutputDir::new(&workspace);
    let exact = compile_tiled_gemm(
        &workspace,
        backend,
        &exact_output,
        TILED_GEMM_FIXTURE,
        "gfx942:xnack-",
        &[],
    );
    let exact_stderr = stderr(&exact);
    assert!(
        exact.status.success()
            && exact_stderr.contains("consumed its single-use frontend receipt")
            && exact_stderr
                .contains("ac26009cf04bdefb1c95dbd2266ea8a23cf2679e90316576ec6fdd6aeefa21a6")
            && exact_stderr.contains("explicit kernarg 64 bytes, complete COV6 kernarg 320 bytes")
            && exact_stderr.contains("exact one-wave 64x1x1 one-tile launch with no LDS")
            && exact_stderr.contains("selected canonical fe2o3::tiled_gemm_v1")
            && exact_stderr
                .contains("bounded reviewed correspondence, not a compiler-refinement proof")
            && exact_stderr.contains("COMGR")
            && exact_output.0.join("tiled-gemm-v1").is_file(),
        "reviewed tiled GEMM did not publish its authenticated handoff:\n{exact_stderr}"
    );

    let same_name_source = TILED_GEMM_FIXTURE.replace(
        "let lane_column = lane % 16;",
        "let lane_column = lane % 8;",
    );
    assert_ne!(same_name_source, TILED_GEMM_FIXTURE);
    let same_name_output = TestOutputDir::new(&workspace);
    let same_name = compile_tiled_gemm(
        &workspace,
        backend,
        &same_name_output,
        &same_name_source,
        "gfx942:xnack-",
        &[],
    );
    let same_name_stderr = stderr(&same_name);
    assert!(
        !same_name.status.success()
            && same_name_stderr.contains("portable MIR identity mismatch")
            && !same_name_stderr.contains("published tiled GEMM Worker V2 handoff"),
        "same-name source mutation minted tiled authority:\n{same_name_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&same_name_output);

    let lookalike_source = TILED_GEMM_FIXTURE
        .replacen(
            "#[kernel(",
            "fn lookalike_fragment(bits: [u16; 4]) -> Bf16MfmaFragment {\n    Bf16MfmaFragment::from_bits(bits)\n}\n\n#[kernel(",
            1,
        )
        .replacen("Bf16MfmaFragment::from_bits([", "lookalike_fragment([", 1);
    assert_ne!(lookalike_source, TILED_GEMM_FIXTURE);
    let lookalike_output = TestOutputDir::new(&workspace);
    let lookalike = compile_tiled_gemm(
        &workspace,
        backend,
        &lookalike_output,
        &lookalike_source,
        "gfx942:xnack-",
        &[],
    );
    let lookalike_stderr = stderr(&lookalike);
    assert!(
        !lookalike.status.success()
            && lookalike_stderr.contains("requires exactly one collected function and no helpers")
            && !lookalike_stderr.contains("published tiled GEMM Worker V2 handoff"),
        "lookalike helper entered the reviewed closure:\n{lookalike_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&lookalike_output);

    let abi_source = TILED_GEMM_FIXTURE
        .replace("c: &[f32]", "c: &mut [f32]")
        .replace(
            "fn(&[u16], &[u16], &[f32], DisjointSlice<f32>)",
            "fn(&[u16], &[u16], &mut [f32], DisjointSlice<f32>)",
        );
    assert_ne!(abi_source, TILED_GEMM_FIXTURE);
    let abi_output = TestOutputDir::new(&workspace);
    let abi = compile_tiled_gemm(
        &workspace,
        backend,
        &abi_output,
        &abi_source,
        "gfx942:xnack-",
        &[],
    );
    let abi_stderr = stderr(&abi);
    assert!(
        !abi.status.success()
            && (abi_stderr.contains("ABI mismatch")
                || abi_stderr.contains("argument kinds")
                || abi_stderr.contains("generated host-contract identity")
                || abi_stderr.contains("#[kernel(typed)] requires"))
            && !abi_stderr.contains("published tiled GEMM Worker V2 handoff"),
        "ABI adversary minted tiled authority:\n{abi_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&abi_output);

    let target_output = TestOutputDir::new(&workspace);
    let wrong_target = compile_tiled_gemm(
        &workspace,
        backend,
        &target_output,
        TILED_GEMM_FIXTURE,
        "gfx942:xnack+",
        &[],
    );
    let target_stderr = stderr(&wrong_target);
    assert!(
        !wrong_target.status.success()
            && target_stderr.contains("requires exact target `gfx942:xnack-`")
            && !target_stderr.contains("published tiled GEMM Worker V2 handoff"),
        "wrong target minted tiled authority:\n{target_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&target_output);

    let semantics_output = TestOutputDir::new(&workspace);
    let wrong_semantics = compile_tiled_gemm(
        &workspace,
        backend,
        &semantics_output,
        TILED_GEMM_FIXTURE,
        "gfx942:xnack-",
        &["-Copt-level=1"],
    );
    let semantics_stderr = stderr(&wrong_semantics);
    assert!(
        !wrong_semantics.status.success()
            && semantics_stderr.contains("compiler semantics mismatch")
            && semantics_stderr.contains("rustc optimization must be No/0")
            && !semantics_stderr.contains("published tiled GEMM Worker V2 handoff"),
        "compiler-semantics adversary minted tiled authority:\n{semantics_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&semantics_output);
}

#[test]
fn tiled_gemm_lds_slice1_attributed_source_publishes_only_the_bound_worker_v2_handoff() {
    let workspace = workspace();
    let backend = build_collection_backend(&workspace);
    assert!(TILED_GEMM_LDS_SLICE1_FIXTURE.contains("pub fn tiled_gemm_lds_slice1"));
    assert!(TILED_GEMM_LDS_SLICE1_FIXTURE.contains("gfx942_lds_bf16_tile_pair_m16x16_v1"));
    assert!(TILED_GEMM_LDS_SLICE1_FIXTURE.contains("gfx942_publish_lds_bf16_tile_pair_m16x16_v1"));
    assert!(!TILED_GEMM_LDS_SLICE1_FIXTURE.contains("\nmacro_rules!"));

    let exact_output = TestOutputDir::new(&workspace);
    let exact = compile_tiled_gemm(
        &workspace,
        backend,
        &exact_output,
        TILED_GEMM_LDS_SLICE1_FIXTURE,
        "gfx942:xnack-",
        &[],
    );
    let exact_stderr = stderr(&exact);
    assert!(
        exact.status.success()
            && exact_stderr
                .contains("selected canonical Kernel IR `fe2o3::tiled_gemm_lds_v1` identity")
            && exact_stderr.contains("constructed compiler descriptor")
            && exact_stderr.contains("0 user-supplied static shared-memory bytes")
            && exact_stderr.contains("1024 static LDS bytes")
            && exact_stderr.contains("completed protected attempt-scoped Worker V2 publication")
            && exact_stderr.contains("not compiler refinement")
            && exact_stderr.contains("COMGR were not entered"),
        "attributed LDS source missed the protected handoff boundary:\n{exact_stderr}"
    );
    assert!(exact_output.0.join("tiled-gemm-v1").is_file());

    let handoff = consume_tiled_gemm_handoff(
        &exact_output,
        TILED_GEMM_LDS_SLICE1_FIXTURE,
        "fe2o3_collected_tiled_gemm_lds_slice1_fixture",
    );
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        handoff
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .collect::<Vec<_>>(),
        ["tiled_gemm_lds_v1"]
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
            .collect::<Vec<_>>(),
        ["tiled_gemm_lds_v1.kd"]
    );
    let module = std::str::from_utf8(handoff.module_bytes()).unwrap();
    assert_eq!(
        module
            .matches("internal addrspace(3) global [256 x i16]")
            .count(),
        2
    );
    assert_eq!(module.matches("s_barrier").count(), 1);
    assert_eq!(
        module
            .matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
            .count(),
        1
    );
    let descriptor = decode_compiler_owned_module_section(module, ".fe2o3.kd.v1").unwrap();
    require_exact_lds_slice1_descriptor_source(&descriptor).unwrap();
    let authority =
        decode_compiler_owned_module_section(module, ".fe2o3.tiled-lds-slice1-auth.v1").unwrap();
    let resources =
        decode_compiler_owned_module_section(module, ".fe2o3.tiled-lds-slice1-resources.v1")
            .unwrap();
    assert_eq!(authority.len(), 32);
    let fields = decode_framed_fields(&resources).unwrap();
    assert!(fields.contains(&b"gfx942:xnack-".as_slice()));
    assert!(fields.contains(&0_u32.to_le_bytes().as_slice()));
    assert!(fields.contains(&1024_u32.to_le_bytes().as_slice()));
    assert!(fields.contains(&512_u32.to_le_bytes().as_slice()));
    assert!(!handoff.authenticates_compiler_origin());
    assert!(!handoff.grants_worker_authority());
    assert!(!handoff.grants_link_authority());
    assert!(!handoff.grants_load_authority());
    assert!(!handoff.grants_launch_authority());
}

#[test]
fn tiled_gemm_lds_slice1_source_mutations_cannot_select_canonical_ir() {
    let workspace = workspace();
    let backend = build_collection_backend(&workspace);
    let mutations = [
        (
            "partial-tile-publish",
            TILED_GEMM_LDS_SLICE1_FIXTURE.replace(
                "    b_lds.write_mfma_fragment(&lane, b_global);\n",
                "",
            ),
        ),
        (
            "cross-pair-publish",
            TILED_GEMM_LDS_SLICE1_FIXTURE.replace(
                "    let (mut a_lds, mut b_lds) = gfx942_lds_bf16_tile_pair_m16x16_v1();",
                "    let (mut a_lds, _first_b_lds) = gfx942_lds_bf16_tile_pair_m16x16_v1();\n    let (_second_a_lds, mut b_lds) = gfx942_lds_bf16_tile_pair_m16x16_v1();",
            ),
        ),
        (
            "index-drift",
            TILED_GEMM_LDS_SLICE1_FIXTURE.replace(
                "a[a_row_base + depth_base + 3]",
                "a[a_row_base + depth_base + 2]",
            ),
        ),
    ];
    for (name, source) in mutations {
        assert_ne!(source, TILED_GEMM_LDS_SLICE1_FIXTURE);
        let output = TestOutputDir::new(&workspace);
        let compilation =
            compile_tiled_gemm(&workspace, backend, &output, &source, "gfx942:xnack-", &[]);
        let stderr = stderr(&compilation);
        assert!(
            !compilation.status.success()
                && stderr.contains("portable MIR identity mismatch")
                && !stderr.contains("selected canonical Kernel IR")
                && !stderr.contains("published attributed LDS Slice 1 Worker V2 handoff"),
            "{name} mutation selected LDS authority:\n{stderr}"
        );
        assert_tiled_gemm_published_no_handoff(&output);
    }

    let lookalike = TILED_GEMM_LDS_SLICE1_FIXTURE.replace(
        "gfx942_lds_bf16_tile_pair_m16x16_v1()",
        "lookalike_lds_pair_v1()",
    ) + r#"
fn lookalike_lds_pair_v1<'workgroup>() -> (
    fe2o3_device::LdsTile16x16<'workgroup, fe2o3_device::Bf16>,
    fe2o3_device::LdsTile16x16<'workgroup, fe2o3_device::Bf16>,
) {
    gfx942_lds_bf16_tile_pair_m16x16_v1()
}
"#;
    let output = TestOutputDir::new(&workspace);
    let compilation = compile_tiled_gemm(
        &workspace,
        backend,
        &output,
        &lookalike,
        "gfx942:xnack-",
        &[],
    );
    let lookalike_stderr = stderr(&compilation);
    assert!(
        !compilation.status.success()
            && lookalike_stderr.contains("requires exactly one collected function and no helpers")
            && !lookalike_stderr.contains("selected canonical Kernel IR")
            && !lookalike_stderr.contains("published attributed LDS Slice 1 Worker V2 handoff"),
        "lookalike helper selected LDS authority:\n{lookalike_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&output);

    let wrong_target_output = TestOutputDir::new(&workspace);
    let wrong_target = compile_tiled_gemm(
        &workspace,
        backend,
        &wrong_target_output,
        TILED_GEMM_LDS_SLICE1_FIXTURE,
        "gfx942:xnack+",
        &[],
    );
    let wrong_target_stderr = stderr(&wrong_target);
    assert!(
        !wrong_target.status.success()
            && wrong_target_stderr.contains("requires exact target `gfx942:xnack-`")
            && !wrong_target_stderr.contains("published attributed LDS Slice 1 Worker V2 handoff"),
        "wrong target published LDS Slice 1 authority:\n{wrong_target_stderr}"
    );
    assert_tiled_gemm_published_no_handoff(&wrong_target_output);
}

#[test]
fn row_softmax_v1_source_authentication_and_adversaries_stop_at_canonical_ir() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let backend = build_backend(&workspace);

    let exact_output = TestOutputDir::new(&workspace);
    let exact = compile_row_softmax_direct(&workspace, backend, &exact_output, ROW_SOFTMAX_FIXTURE);
    let exact_stderr = stderr(&exact);
    assert!(
        !exact.status.success()
            && exact_stderr.contains("requires a managed FE2O3_BUILD_ATTEMPT_V1")
            && !exact_stderr.contains("consumed its private single-use frontend receipt")
            && !exact_stderr.contains("selected canonical Kernel IR module"),
        "direct rustc minted row-softmax authority:\n{exact_stderr}"
    );
    assert_row_softmax_published_nothing(&exact_output);

    let cargo_output = TestOutputDir::new(&workspace);
    let cargo_target = cargo_output.0.join("cargo-target");
    let device_root = workspace.join("crates/fe2o3-device");
    let host_root = workspace.join("crates/fe2o3-host");
    let compile_managed = |package_name, source, target, extra_rustflags| {
        compile_external_row_softmax_crate(
            &workspace,
            &cargo_target,
            ExternalRowSoftmaxSpec {
                package_name,
                source,
                target,
                extra_rustflags,
                device_root: &device_root,
                host_root: &host_root,
            },
        )
    };

    let arithmetic_source = ROW_SOFTMAX_FIXTURE.replace("value > maximum", "value >= maximum");
    assert_ne!(arithmetic_source, ROW_SOFTMAX_FIXTURE);
    let (arithmetic, arithmetic_output) = compile_managed(
        "fe2o3-row-softmax-arithmetic-adversary",
        &arithmetic_source,
        "gfx942:xnack-",
        &[],
    );
    let arithmetic_stderr = stderr(&arithmetic);
    assert!(
        !arithmetic.status.success()
            && arithmetic_stderr.contains("portable MIR identity mismatch")
            && !arithmetic_stderr.contains("selected canonical Kernel IR module"),
        "same-name arithmetic mutation minted row-softmax authority:\n{arithmetic_stderr}"
    );
    assert_row_softmax_published_nothing(&arithmetic_output);

    let extent_source = ROW_SOFTMAX_FIXTURE.replace(
        "const ROW_ELEMENTS: usize = 64;",
        "const ROW_ELEMENTS: usize = 63;",
    );
    assert_ne!(extent_source, ROW_SOFTMAX_FIXTURE);
    let (extent, extent_output) = compile_managed(
        "fe2o3-row-softmax-extent-adversary",
        &extent_source,
        "gfx942:xnack-",
        &[],
    );
    let extent_stderr = stderr(&extent);
    assert!(
        !extent.status.success()
            && extent_stderr.contains("portable MIR identity mismatch")
            && !extent_stderr.contains("selected canonical Kernel IR module"),
        "63-element source mutation minted row-softmax authority:\n{extent_stderr}"
    );
    assert_row_softmax_published_nothing(&extent_output);

    let helper_source = ROW_SOFTMAX_FIXTURE
        .replace("math.exp_f32(", "exp_lookalike(&math, ")
        .replacen(
            "const ROW_ELEMENTS: usize = 64;",
            "const ROW_ELEMENTS: usize = 64;\n\nfn exp_lookalike(math: &DeviceMath, value: f32) -> f32 {\n    math.exp_f32(value)\n}",
            1,
        );
    assert_ne!(helper_source, ROW_SOFTMAX_FIXTURE);
    let (helper, helper_output) = compile_managed(
        "fe2o3-row-softmax-helper-adversary",
        &helper_source,
        "gfx942:xnack-",
        &[],
    );
    let helper_stderr = stderr(&helper);
    assert!(
        !helper.status.success()
            && helper_stderr.contains("requires exactly one collected function and no helpers")
            && !helper_stderr.contains("selected canonical Kernel IR module"),
        "lookalike exp helper entered the reviewed closure:\n{helper_stderr}"
    );
    assert_row_softmax_published_nothing(&helper_output);

    let abi_source = ROW_SOFTMAX_FIXTURE
        .replacen("input: &[f32]", "input: &mut [f32]", 1)
        .replace(
            "fn(&[f32], DisjointSlice<f32>)",
            "fn(&mut [f32], DisjointSlice<f32>)",
        );
    assert_ne!(abi_source, ROW_SOFTMAX_FIXTURE);
    let (abi, abi_output) = compile_managed(
        "fe2o3-row-softmax-abi-adversary",
        &abi_source,
        "gfx942:xnack-",
        &[],
    );
    let abi_stderr = stderr(&abi);
    assert!(
        !abi.status.success()
            && (abi_stderr.contains("ABI mismatch")
                || abi_stderr.contains("argument kinds")
                || abi_stderr.contains("generated host-contract identity")
                || abi_stderr.contains("#[kernel(typed)] requires"))
            && !abi_stderr.contains("selected canonical Kernel IR module"),
        "ABI adversary minted row-softmax authority:\n{abi_stderr}"
    );
    assert_row_softmax_published_nothing(&abi_output);

    let contract_source = ROW_SOFTMAX_FIXTURE.replacen(
        "3, 0, 0, 0, 64, 0, 0,\n    0, 1",
        "3, 0, 0, 0, 32, 0, 0,\n    0, 1",
        1,
    );
    assert_ne!(contract_source, ROW_SOFTMAX_FIXTURE);
    let (contract, contract_output) = compile_managed(
        "fe2o3-row-softmax-contract-adversary",
        &contract_source,
        "gfx942:xnack-",
        &[],
    );
    let contract_stderr = stderr(&contract);
    assert!(
        !contract.status.success()
            && contract_stderr.contains("frontend contract bytes do not match")
            && !contract_stderr.contains("selected canonical Kernel IR module"),
        "frontend-contract adversary minted row-softmax authority:\n{contract_stderr}"
    );
    assert_row_softmax_published_nothing(&contract_output);

    let (target, target_output) = compile_managed(
        "fe2o3-row-softmax-target-adversary",
        ROW_SOFTMAX_FIXTURE,
        "gfx942:xnack+",
        &[],
    );
    let target_stderr = stderr(&target);
    assert!(
        !target.status.success()
            && target_stderr.contains("requires exact target `gfx942:xnack-`")
            && !target_stderr.contains("selected canonical Kernel IR module"),
        "wrong target minted row-softmax authority:\n{target_stderr}"
    );
    assert_row_softmax_published_nothing(&target_output);

    let (semantics, semantics_output) = compile_managed(
        "fe2o3-row-softmax-semantics-adversary",
        ROW_SOFTMAX_FIXTURE,
        "gfx942:xnack-",
        &["-Copt-level=1"],
    );
    let semantics_stderr = stderr(&semantics);
    assert!(
        !semantics.status.success()
            && semantics_stderr.contains("compiler semantics mismatch")
            && semantics_stderr.contains("rustc optimization must be No/0")
            && !semantics_stderr.contains("selected canonical Kernel IR module"),
        "compiler-semantics adversary minted row-softmax authority:\n{semantics_stderr}"
    );
    assert_row_softmax_published_nothing(&semantics_output);
}

#[test]
fn row_softmax_rejects_a_hostile_same_name_device_provider() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let provider_output = TestOutputDir::new(&workspace);
    let (hostile_device, hostile_host) =
        prepare_hostile_same_name_device_provider(&workspace, &provider_output);
    let cargo_output = TestOutputDir::new(&workspace);
    let (compiled, output) = compile_external_row_softmax_crate(
        &workspace,
        &cargo_output.0.join("cargo-target"),
        ExternalRowSoftmaxSpec {
            package_name: "fe2o3-row-softmax-hostile-provider",
            source: ROW_SOFTMAX_FIXTURE,
            target: "gfx942:xnack-",
            extra_rustflags: &[],
            device_root: &hostile_device,
            host_root: &hostile_host,
        },
    );
    let compiler_stderr = stderr(&compiled);
    assert!(
        !compiled.status.success()
            && !compiler_stderr.contains("selected canonical Kernel IR module")
            && (compiler_stderr.contains("trusted-provider marker")
                || compiler_stderr.contains("genuine external `fe2o3_device::DisjointSlice` type")
                || compiler_stderr.contains("expected exact argument order")
                || compiler_stderr
                    .contains("diagnostic item does not resolve to the trusted function")),
        "hostile same-name provider minted row-softmax authority:\n{compiler_stderr}"
    );
    assert_row_softmax_published_nothing(&output);
}

#[test]
fn row_softmax_requires_managed_wrapper_argv_and_exact_metadata_transcript() {
    if isolated_backend_environment_is_unavailable() {
        return;
    }
    let workspace = workspace();
    let backend = build_backend(&workspace);
    build_frontend_dependencies(&workspace).expect("build reviewed frontend dependencies");
    let cargo_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let device = cargo_target.join("debug/libfe2o3_device.rlib");
    let host = cargo_target.join("debug/libfe2o3_host.rlib");

    let direct_output = TestOutputDir::new(&workspace);
    let direct = compile_row_softmax_with_device(
        &workspace,
        backend,
        &direct_output,
        ROW_SOFTMAX_FIXTURE,
        "gfx942:xnack-",
        "3a4d867f29d87610",
        &[],
        &device,
        &host,
        false,
        "a59650cf8d1bfc6168915cb817dbab3a0fa6a8839291231bbf4149a749913937",
    );
    let direct_stderr = stderr(&direct);
    assert!(
        !direct.status.success()
            && direct_stderr.contains("requires a managed FE2O3_BUILD_ATTEMPT_V1")
            && !direct_stderr.contains("selected canonical Kernel IR module"),
        "direct rustc minted row-softmax authority:\n{direct_stderr}"
    );

    let missing_tail_output = TestOutputDir::new(&workspace);
    let missing_tail = compile_row_softmax_with_device(
        &workspace,
        backend,
        &missing_tail_output,
        ROW_SOFTMAX_FIXTURE,
        "gfx942:xnack-",
        "3a4d867f29d87610",
        &[],
        &device,
        &host,
        true,
        "a59650cf8d1bfc6168915cb817dbab3a0fa6a8839291231bbf4149a749913937",
    );
    let missing_tail_stderr = stderr(&missing_tail);
    assert!(
        !missing_tail.status.success()
            && missing_tail_stderr.contains(
                "managed wrapper effective rustc argv omitted or changed its exact managed tail"
            )
            && !missing_tail_stderr.contains("selected canonical Kernel IR module"),
        "direct rustc without the managed tail minted row-softmax authority:\n{missing_tail_stderr}"
    );
    assert_row_softmax_published_nothing(&missing_tail_output);

    let managed_output = TestOutputDir::new(&workspace);
    let managed_target = managed_output.0.join("cargo-target");
    let broker = build_and_pin_handoff_broker(&workspace, &managed_target);
    let device_root = workspace.join("crates/fe2o3-device");
    let host_root = workspace.join("crates/fe2o3-host");
    for (name, mutation, expected) in [
        (
            "fe2o3-row-softmax-metadata-omitted",
            Some("omit"),
            "managed wrapper omitted FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
        ),
        (
            "fe2o3-row-softmax-metadata-substituted",
            Some("substitute"),
            "managed wrapper Cargo metadata transcript does not match rustc's ordered -Cmetadata values",
        ),
        (
            "fe2o3-row-softmax-managed-exact",
            None,
            "selected canonical Kernel IR module `fe2o3::row_softmax_v1`",
        ),
    ] {
        let (managed, output) = compile_external_row_softmax_crate_with_broker(
            &workspace,
            &managed_target,
            ExternalRowSoftmaxSpec {
                package_name: name,
                source: ROW_SOFTMAX_FIXTURE,
                target: "gfx942:xnack-",
                extra_rustflags: &[],
                device_root: &device_root,
                host_root: &host_root,
            },
            &broker,
            mutation,
        );
        let managed_stderr = stderr(&managed);
        assert!(
            !managed.status.success() && managed_stderr.contains(expected),
            "managed wrapper case {name} missed {expected:?}:\n{managed_stderr}"
        );
        if mutation.is_some() {
            assert!(
                !managed_stderr.contains("selected canonical Kernel IR module"),
                "metadata mutation reached canonical IR selection:\n{managed_stderr}"
            );
        } else {
            assert!(
                managed_stderr.contains("published an inert Worker V2 compiler-module handoff")
                    && managed_stderr
                        .contains("build completed without an authorized device backend"),
                "exact managed fixture missed its positive checkpoints:\n{managed_stderr}"
            );
        }
        assert_row_softmax_published_nothing(&output);
    }
}

#[test]
fn clean_external_cargo_fe2o3_produces_row_softmax_worker_v2_handoffs() {
    let workspace = workspace();
    let cargo_output = TestOutputDir::new(&workspace);
    let cargo_target = cargo_output.0.join("cargo-target");
    let broker = build_and_pin_handoff_broker(&workspace, &cargo_target);
    let retained_broker_path = substitute_handoff_broker_path(&cargo_target, &broker);
    assert!(retained_broker_path.is_file());
    let mut roots = Vec::new();
    for package_name in [
        "fe2o3-row-softmax-external-a",
        "fe2o3-row-softmax-external-b",
    ] {
        let (external, output) = compile_clean_external_row_softmax_crate_with_handoff(
            &workspace,
            &cargo_target,
            &broker,
            package_name,
        );
        let external_stderr = stderr(&external);
        assert!(
            !external.status.success()
                && external_stderr.contains("consumed its private single-use frontend receipt")
                && external_stderr
                    .contains("selected canonical Kernel IR module `fe2o3::row_softmax_v1`")
                && external_stderr.contains("published an inert Worker V2 compiler-module handoff")
                && external_stderr.contains("published row-softmax Worker V2 handoff")
                && external_stderr.contains(
                    "explicit kernarg 32 bytes and required COV6 complete kernarg 288 bytes"
                )
                && external_stderr.contains("`__ocml_exp_f32` unresolved-import bindings")
                && external_stderr.contains("build completed without an authorized device backend")
                && !external_stderr.contains("root instance must have")
                && !external_stderr.contains("portable MIR identity mismatch")
                && !external_stderr.contains("rustc FnAbi identity mismatch")
                && !external_stderr.contains(BROKER_PATH_SUBSTITUTION_MARKER),
            "clean external cargo-fe2o3 crate missed row-softmax handoff production:\n{external_stderr}"
        );
        let root = admitted_row_softmax_root(&external_stderr).unwrap_or_else(|| {
            panic!("external admission omitted its generated root:\n{external_stderr}")
        });
        let suffix = root
            .strip_prefix("__fe2o3_host_kernel_v1_")
            .unwrap_or_else(|| panic!("external admission reported a malformed root: {root:?}"));
        assert_eq!(suffix.len(), 64);
        assert!(
            suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "external admission reported a noncanonical root: {root:?}"
        );
        roots.push(root.to_owned());
        assert!(
            !output.0.join("row-softmax-v1").exists(),
            "external Cargo path must not fabricate a linked row-softmax output"
        );
    }
    assert_ne!(
        roots[0], roots[1],
        "distinct Cargo crate identities must exercise variable generated roots"
    );
}

#[test]
fn row_softmax_managed_wrapper_accepts_variable_generated_roots() {
    let workspace = workspace();
    let cargo_output = TestOutputDir::new(&workspace);
    let cargo_target = cargo_output.0.join("cargo-target");
    let mut roots = Vec::new();
    for package_name in [
        "fe2o3-row-softmax-external-a",
        "fe2o3-row-softmax-external-b",
    ] {
        let (external, output) =
            compile_clean_external_row_softmax_crate(&workspace, &cargo_target, package_name);
        let external_stderr = stderr(&external);
        assert!(
            !external.status.success()
                && external_stderr.contains("consumed its private single-use frontend receipt")
                && external_stderr
                    .contains("selected canonical Kernel IR module `fe2o3::row_softmax_v1`")
                && external_stderr.contains("published an inert Worker V2 compiler-module handoff")
                && external_stderr.contains("build completed without an authorized device backend")
                && !external_stderr.contains("root instance must have")
                && !external_stderr.contains("portable MIR identity mismatch")
                && !external_stderr.contains("rustc FnAbi identity mismatch"),
            "clean external cargo-fe2o3 crate missed the row-softmax boundary:\n{external_stderr}"
        );
        let root = admitted_row_softmax_root(&external_stderr).unwrap_or_else(|| {
            panic!("external admission omitted its generated root:\n{external_stderr}")
        });
        let suffix = root
            .strip_prefix("__fe2o3_host_kernel_v1_")
            .unwrap_or_else(|| panic!("external admission reported a malformed root: {root:?}"));
        assert_eq!(suffix.len(), 64);
        assert!(
            suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "external admission reported a noncanonical root: {root:?}"
        );
        roots.push(root.to_owned());
        assert_row_softmax_published_nothing(&output);

        if roots.len() == 1 {
            let managed_backend =
                pin_backend(&cargo_target.join("debug/librustc_codegen_fe2o3.so"))
                    .expect("pin the backend built with cargo-fe2o3 broker identity");
            let forged_output = TestOutputDir::new(&workspace);
            let forged = compile_row_softmax_with_forged_exact_argv_and_fixed_descriptors(
                &workspace,
                &managed_backend,
                &forged_output,
            );
            let forged_stderr = stderr(&forged);
            assert!(
                !forged.status.success()
                    && forged_stderr
                        .contains("protected rustc invocation admission failed without fallback")
                    && forged_stderr
                        .contains("cannot admit canonical fd 199 as a sealed V3 capability")
                    && !forged_stderr.contains("selected canonical Kernel IR module"),
                "exact-argv direct rustc with attacker-populated reserved FDs minted row-softmax authority:\n{forged_stderr}"
            );
            assert_row_softmax_published_nothing(&forged_output);
        }
    }
    assert_ne!(
        roots[0], roots[1],
        "distinct Cargo crate identities must exercise variable generated roots"
    );
}

#[test]
fn external_cargo_fe2o3_reaches_the_typed_tiled_handoff_boundary() {
    let workspace = workspace();
    let manifest = workspace
        .join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-v1/Cargo.toml");
    let source = manifest.parent().unwrap().join("src/lib.rs");
    let fixture_target = manifest.parent().unwrap().join("target");
    if fixture_target.exists() {
        std::fs::remove_dir_all(&fixture_target).expect("clear external fixture target");
    }
    let rustflags = format!(
        "-Coverflow-checks=off -Cmetadata=fe2o3-tiled-gemm-v1-reviewed --remap-path-prefix={}=/fe2o3-reviewed-workspace/tiled-gemm-v1.rs",
        source.display()
    );
    let cargo_output = TestOutputDir::new(&workspace);
    let broker = build_and_pin_broker(&workspace, &cargo_output.0.join("cargo-target"), false);
    let mut command = broker
        .command()
        .expect("verify test-owned cargo-fe2o3 before tiled launch");
    command
        .current_dir(&workspace)
        .args(["build", "--locked", "--manifest-path"])
        .arg(&manifest)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_INCREMENTAL")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", TILED_GEMM_PIPELINE)
        .env("RUSTFLAGS", rustflags)
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    scrub_test_dynamic_loader_environment(&mut command);
    let external = run_bounded(
        &mut command,
        BACKEND_BUILD_TIMEOUT,
        "external cargo-fe2o3 tiled GEMM fixture",
    )
    .expect("run external cargo-fe2o3 fixture within deadline");
    let external_stderr = stderr(&external);
    assert!(
        !external.status.success()
            && external_stderr.contains("published tiled GEMM Worker V2 handoff")
            && external_stderr
                .contains("explicit kernarg 64 bytes, complete COV6 kernarg 320 bytes")
            && external_stderr.contains("selected canonical fe2o3::tiled_gemm_v1")
            && external_stderr.contains("build completed without an authorized device backend")
            && !external_stderr.contains("portable MIR identity mismatch")
            && !external_stderr.contains("rustc FnAbi identity mismatch"),
        "external cargo-fe2o3 fixture missed the typed downstream boundary:\n{external_stderr}"
    );
}

#[test]
fn managed_cargo_fe2o3_collects_the_exact_attributed_lds_slice1_source() {
    let workspace = workspace();
    let manifest = workspace.join(
        "crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-lds-slice1/Cargo.toml",
    );
    let source = workspace.join("examples/tiled_gemm_v1/src/kernel.rs");
    let fixture_target = manifest.parent().unwrap().join("target");
    if fixture_target.exists() {
        std::fs::remove_dir_all(&fixture_target).expect("clear external LDS fixture target");
    }
    let rustflags = format!(
        "-Coverflow-checks=off -Cmetadata=fe2o3-tiled-gemm-v1-reviewed --remap-path-prefix={}=/fe2o3-reviewed-workspace/tiled-gemm-v1.rs",
        source.display()
    );
    let cargo_output = TestOutputDir::new(&workspace);
    let broker = build_and_pin_broker(&workspace, &cargo_output.0.join("cargo-target"), false);
    let mut command = broker
        .command()
        .expect("verify test-owned cargo-fe2o3 before LDS launch");
    command
        .current_dir(&workspace)
        .args(["build", "--locked", "--manifest-path"])
        .arg(&manifest)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_INCREMENTAL")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", TILED_GEMM_PIPELINE)
        .env("RUSTFLAGS", rustflags)
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    scrub_test_dynamic_loader_environment(&mut command);
    let external = run_bounded(
        &mut command,
        BACKEND_BUILD_TIMEOUT,
        "managed cargo-fe2o3 LDS Slice 1 fixture",
    )
    .expect("run managed LDS Slice 1 fixture within deadline");
    let external_stderr = stderr(&external);
    assert!(
        !external.status.success()
            && external_stderr
                .contains("selected canonical Kernel IR `fe2o3::tiled_gemm_lds_v1` identity")
            && external_stderr.contains("constructed compiler descriptor")
            && external_stderr.contains("0 user-supplied static shared-memory bytes")
            && external_stderr.contains("1024 static LDS bytes")
            && external_stderr.contains(
                "published attributed LDS Slice 1 Worker V2 handoff bound to source authority"
            )
            && external_stderr.contains("completed protected attempt-scoped Worker V2 publication")
            && external_stderr.contains("build completed without an authorized device backend")
            && external_stderr.contains("COMGR were not entered"),
        "managed cargo-fe2o3 missed the attributed LDS protected handoff boundary:\n{external_stderr}"
    );
}

#[test]
fn pinned_backend_descriptor_survives_same_uid_path_replacement() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    assert_eq!(
        std::fs::metadata(&output.0).unwrap().permissions().mode() & 0o777,
        0o700
    );

    let replaceable = output.0.join("replaceable-backend.so");
    let original = b"original backend bytes";
    std::fs::write(&replaceable, original).unwrap();
    let pinned = pin_backend(&replaceable).unwrap();
    std::fs::write(&replaceable, b"same-uid substituted path contents").unwrap();

    pinned.verify().unwrap();
    let expected_sha256: [u8; 32] = Sha256::digest(original).into();
    assert_eq!(pinned.sha256, expected_sha256);
    assert_eq!(
        pinned.load_path(),
        PathBuf::from(format!("/proc/self/fd/./{}", pinned.file.as_raw_fd()))
    );
    let replacement = [0_u8];
    let written = unsafe {
        libc::pwrite(
            pinned.file.as_raw_fd(),
            replacement.as_ptr().cast(),
            replacement.len(),
            0,
        )
    };
    assert_eq!(written, -1, "F_SEAL_WRITE must reject descriptor writes");
    pinned.verify().unwrap();
}
