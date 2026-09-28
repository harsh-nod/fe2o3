//! Gated fork/exec: one controller owns the PID from birth, with no spawn helper
//! thread whose exec-error-pipe wait could outlive the controller's deadline.

use super::*;
use std::ffi::CString;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;

unsafe extern "C" {
    fn getppid() -> i32;
    fn pipe2(fds: *mut i32, flags: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn read(fd: i32, buffer: *mut c_void, count: usize) -> isize;
    fn write(fd: i32, buffer: *const c_void, count: usize) -> isize;
    fn chdir(path: *const std::ffi::c_char) -> i32;
    fn execve(
        path: *const std::ffi::c_char,
        args: *const *const std::ffi::c_char,
        env: *const *const std::ffi::c_char,
    ) -> i32;
    fn _exit(status: i32) -> !;
}

pub(super) struct SeizedChild {
    pid: i32,
    pub(super) stdout: Option<File>,
    pub(super) stderr: Option<File>,
}

impl SeizedChild {
    pub(super) fn id(&self) -> u32 {
        self.pid as u32
    }
}

fn pipe() -> Result<(OwnedFd, OwnedFd), RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut fds = [-1; 2];
    // SAFETY: pipe2 initializes both descriptor slots atomically with CLOEXEC.
    if unsafe { pipe2(fds.as_mut_ptr(), 0x80000) } != 0 {
        return Err(io_process_failure("create seized-child pipe"));
    }
    // SAFETY: successful pipe2 transferred exactly these two fresh descriptors.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

fn cstring(bytes: &[u8]) -> Result<CString, RetainedFunctionalRefinementRuntimeErrorV1> {
    CString::new(bytes).map_err(|_| process_failure("NUL in seized-child invocation"))
}

pub(super) fn spawn(
    command: Command,
    bindings: Vec<DescriptorBinding>,
    cpu_seconds: u64,
    deadline: Instant,
) -> Result<SeizedChild, RetainedFunctionalRefinementRuntimeErrorV1> {
    let executable = cstring(command.get_program().as_bytes())?;
    if !Path::new(command.get_program()).is_absolute() {
        return Err(process_failure("seized executable must be absolute"));
    }
    let arguments = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|s| cstring(s.as_bytes()))
        .collect::<Result<Vec<_>, _>>()?;
    // This private launcher deliberately never inherits the controller environment.
    let environment = command
        .get_envs()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .map(|(k, v)| {
            let mut bytes = k.as_bytes().to_vec();
            bytes.push(b'=');
            bytes.extend_from_slice(v.as_bytes());
            cstring(&bytes)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let argv = arguments
        .iter()
        .map(|s| s.as_ptr())
        .chain([std::ptr::null()])
        .collect::<Vec<_>>();
    let envp = environment
        .iter()
        .map(|s| s.as_ptr())
        .chain([std::ptr::null()])
        .collect::<Vec<_>>();
    let cwd = command
        .get_current_dir()
        .map(|p| cstring(p.as_os_str().as_bytes()))
        .transpose()?;
    let stdin = File::open("/dev/null").map_err(|_| io_process_failure("open proof stdin"))?;
    let (gate_read, gate_write) = pipe()?;
    let (stdout_read, stdout_write) = pipe()?;
    let (stderr_read, stderr_write) = pipe()?;
    let owner_pid = std::process::id() as i32;
    let child = fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| {
        if Instant::now() >= deadline {
            return Err(controller_error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut,
                "seized spawn deadline elapsed before fork",
            ));
        }
        // SAFETY: only the child branch below executes after fork, using prebuilt
        // pointers, raw syscalls and the syscall-only prepare_child; it never
        // allocates, unwinds, locks, returns to Rust callers or runs destructors.
        let fork_result: i64;
        // Bypass libc fork's registered atfork callbacks. No unrelated callback
        // may allocate or acquire an inherited lock in this pre-exec child.
        unsafe {
            std::arch::asm!("syscall", inlateout("rax") 57_i64 => fork_result,
                lateout("rcx") _, lateout("r11") _);
        }
        let pid = fork_result as i32;
        if pid < 0 {
            return Err(process_failure(format!(
                "fork gated proof child: {}",
                io::Error::from_raw_os_error(-pid)
            )));
        }
        if pid == 0 {
            // SAFETY: descriptors and C strings were constructed before fork.
            unsafe {
                // PR_SET_PDEATHSIG closes the pre-SEIZE orphan window. Check
                // again afterward for a parent lost before that prctl took effect.
                if prctl(1, SIGKILL as usize, 0_usize, 0_usize, 0_usize) != 0
                    || getppid() != owner_pid
                {
                    _exit(125);
                }
                close(gate_write.as_raw_fd());
                let mut token = 0_u8;
                if read(gate_read.as_raw_fd(), (&raw mut token).cast(), 1) != 1 || token != 1 {
                    _exit(125);
                }
                close(gate_read.as_raw_fd());
                if dup2(stdin.as_raw_fd(), 0) < 0
                    || dup2(stdout_write.as_raw_fd(), 1) < 0
                    || dup2(stderr_write.as_raw_fd(), 2) < 0
                {
                    _exit(125);
                }
                if let Some(cwd) = &cwd
                    && chdir(cwd.as_ptr()) != 0
                {
                    _exit(125);
                }
                if prepare_child(&bindings, cpu_seconds).is_err() {
                    _exit(125);
                }
                execve(executable.as_ptr(), argv.as_ptr(), envp.as_ptr());
                _exit(126);
            }
        }
        Ok(SeizedChild {
            pid,
            stdout: Some(stdout_read.into()),
            stderr: Some(stderr_read.into()),
        })
    })?;
    drop(gate_read);
    drop(stdout_write);
    drop(stderr_write);
    let mut seized = false;
    let mut current_stop = None;
    let mut terminal = None;
    let attachment = (|| {
        ptrace(PTRACE_SEIZE, child.pid, trace_options())?;
        seized = true;
        ptrace(PTRACE_INTERRUPT, child.pid, 0)?;
        let status = wait_for_specific(child.pid, deadline)?;
        current_stop = TraceeStop::observed(status);
        if current_stop.is_none() {
            terminal = Some(status);
        }
        if current_stop.is_none()
            || (status as u32) >> 16 != PTRACE_EVENT_STOP
            || stop_signal(status) != SIGTRAP
        {
            return Err(process_failure(
                "gated child missed its seized interrupt stop",
            ));
        }
        let token = 1_u8;
        // SAFETY: one byte goes to an empty retained gate with a stopped reader.
        if unsafe { write(gate_write.as_raw_fd(), (&raw const token).cast(), 1) } != 1 {
            return Err(io_process_failure("release seized-child gate"));
        }
        continue_tracee(child.pid, 0)?;
        current_stop = None;
        Ok(())
    })();
    drop(gate_write);
    if let Err(error) = attachment {
        if seized {
            let tree = BTreeMap::from([(
                child.pid,
                Tracee {
                    role: TraceeRole::Verifier,
                    thread_group: child.pid,
                    leader: true,
                    saw_exit_event: false,
                    current_stop,
                    queued_status: terminal,
                },
            )]);
            return Err(reject_and_reap(&tree, error));
        }
        kill_tracee(child.pid).map_err(process_failure)?;
        let terminal = wait_for_specific(child.pid, Instant::now() + CLEANUP_TIMEOUT)?;
        if stopped(terminal) {
            return Err(process_failure("unseized child failed to terminate"));
        }
        return Err(error);
    }
    Ok(child)
}
