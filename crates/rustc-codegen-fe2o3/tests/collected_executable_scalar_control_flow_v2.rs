use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{
    DirBuilderExt as _, FileExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use sha2::{Digest as _, Sha256};

const PIPELINE: &str = "collected-executable-scalar-control-flow-v2";
const FIXTURE: &str = include_str!("fixtures/executable-scalar-control-flow-v1.rs");
const SCALAR_GEMM_PIPELINE: &str = "collected-scalar-gemm-v1";
const SCALAR_GEMM_FIXTURE: &str = include_str!("../../../examples/scalar_gemm_v1/src/kernel.rs");
const TILED_GEMM_PIPELINE: &str = "collected-tiled-gemm-v1";
const TILED_GEMM_FIXTURE: &str = include_str!("fixtures/collected-tiled-gemm-v1/src/lib.rs");
const ROW_SOFTMAX_PIPELINE: &str = "collected-row-softmax-v1";
const ROW_SOFTMAX_FIXTURE: &str = include_str!("fixtures/collected-row-softmax-v1/src/lib.rs");
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
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();
static USER_MOUNT_NAMESPACE: OnceLock<Result<(), String>> = OnceLock::new();

const REQUIRED_MEMFD_SEALS: libc::c_int =
    libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;

struct PinnedBackend {
    file: File,
    len: usize,
    sha256: [u8; 32],
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
    build_frontend_dependencies(workspace).expect("build tiled GEMM frontend dependencies");
    backend
        .verify()
        .expect("sealed backend identity before tiled GEMM rustc");
    let source_path = output.0.join("tiled-gemm-v1.rs");
    std::fs::write(&source_path, source).expect("write tiled GEMM fixture");
    let device = workspace.join("target/debug/libfe2o3_device.rlib");
    let host = workspace.join("target/debug/libfe2o3_host.rlib");
    let manifest_directory =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-tiled-gemm-v1");
    assert!(device.is_file(), "missing {}", device.display());
    assert!(host.is_file(), "missing {}", host.display());
    let producer =
        ProducerIdentity::from_codegen("fe2o3_collected_tiled_gemm_v1_fixture", Some(&source_path))
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
        .args([
            "--edition=2024",
            "--crate-type",
            "lib",
            "--crate-name",
            "fe2o3_collected_tiled_gemm_v1_fixture",
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
            "-Cmetadata=4ceb166423714bdc",
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
    let invocation = BuildInvocation::from_bytes(digest.finalize().into());
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

    let mut rustflags = format!(
        "-Coverflow-checks=off -Cmetadata=fe2o3-row-softmax-v1-reviewed --remap-path-prefix={}=/fe2o3-reviewed-workspace/row-softmax-v1.rs",
        source.display()
    );
    for flag in spec.extra_rustflags {
        rustflags.push(' ');
        rustflags.push_str(flag);
    }
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "--manifest-path",
        ])
        .arg(&manifest)
        .env("CARGO_TARGET_DIR", cargo_target)
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("FE2O3_TARGET", spec.target)
        .env("FE2O3_CODEGEN_PIPELINE", ROW_SOFTMAX_PIPELINE)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"))
        .env("RUSTFLAGS", rustflags);
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
    let probe = unsafe { libc::kill(descendant, 0) };
    assert_eq!(
        probe, -1,
        "timed-out descendant {descendant} survived cleanup"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH),
        "timed-out descendant still exists"
    );
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
                .contains("48df32b608f5dafa300f35d18641b657f7758365791120d649495b1aea72dfe8")
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
            "const FRONTEND_CONTRACT: &[u8] = &[",
            "fn lookalike_fragment(bits: [u16; 4]) -> Bf16MfmaFragment {\n    Bf16MfmaFragment::from_bits(bits)\n}\n\nconst FRONTEND_CONTRACT: &[u8] = &[",
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

    let fabricated_output = TestOutputDir::new(&workspace);
    let fabricated = compile_row_softmax_with_device(
        &workspace,
        backend,
        &fabricated_output,
        ROW_SOFTMAX_FIXTURE,
        "gfx942:xnack-",
        "3a4d867f29d87610",
        &[],
        &device,
        &host,
        true,
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    let fabricated_stderr = stderr(&fabricated);
    assert!(
        !fabricated.status.success()
            && fabricated_stderr.contains("managed wrapper Cargo metadata transcript")
            && fabricated_stderr.contains("does not match rustc's ordered -Cmetadata values")
            && !fabricated_stderr.contains("selected canonical Kernel IR module"),
        "fabricated wrapper observation minted row-softmax authority:\n{fabricated_stderr}"
    );
    assert_row_softmax_published_nothing(&fabricated_output);

    let forged_attempt_output = TestOutputDir::new(&workspace);
    let forged_attempt = compile_row_softmax_with_device(
        &workspace,
        backend,
        &forged_attempt_output,
        ROW_SOFTMAX_FIXTURE,
        "gfx942:xnack-",
        "3a4d867f29d87610",
        &[],
        &device,
        &host,
        true,
        "a59650cf8d1bfc6168915cb817dbab3a0fa6a8839291231bbf4149a749913937",
    );
    let forged_attempt_stderr = stderr(&forged_attempt);
    assert!(
        !forged_attempt.status.success()
            && forged_attempt_stderr.contains("managed wrapper effective rustc argv")
            && !forged_attempt_stderr.contains("selected canonical Kernel IR module"),
        "direct rustc with a valid-looking attempt and exact metadata minted row-softmax authority:\n{forged_attempt_stderr}"
    );
    assert_row_softmax_published_nothing(&forged_attempt_output);
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
                && external_stderr
                    .contains("stopped at the fail-closed source-authenticated boundary")
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
            let managed_backend = pin_backend(
                &cargo_target.join(".fe2o3-backend-build-v1/debug/librustc_codegen_fe2o3.so"),
            )
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
                    && forged_stderr.contains(
                        "invocation-capability peer is not the cargo-fe2o3 executable pinned into this backend",
                    )
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
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(&workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "--locked",
            "--manifest-path",
        ])
        .arg(&manifest)
        .env_remove("CARGO_INCREMENTAL")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", TILED_GEMM_PIPELINE)
        .env("RUSTFLAGS", rustflags)
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
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
