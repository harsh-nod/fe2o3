use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{
    DirBuilderExt as _, FileExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::os::unix::net::UnixDatagram;
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
