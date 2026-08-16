use crate::closure::InheritedDescriptorV1;
use crate::error::{HostLinkError, HostLinkErrorCodeV1, ResultContext};
use core::ffi::{c_char, c_int, c_long, c_uint, c_void};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::time::{Duration, Instant};

const AT_EMPTY_PATH: c_int = 0x1000;
const CLOSE_RANGE_CLOEXEC: c_uint = 1 << 2;
const CLONE_PIDFD: u64 = 0x0000_1000;
const CLONE_CLEAR_SIGHAND: u64 = 0x0000_0001_0000_0000;
const SIGCHLD: u64 = 17;
const SIGKILL: c_int = 9;
const SIGSTOP: c_int = 19;
const SYS_CLONE3: c_long = 435;
const SYS_RT_SIGACTION: c_long = 13;
const SYS_RT_SIGPROCMASK: c_long = 14;
const SIG_SETMASK: c_int = 2;
const KERNEL_SIGNAL_COUNT: c_int = 64;
const KERNEL_SIGSET_BYTES: usize = 8;
const PR_SET_NO_NEW_PRIVS: c_int = 38;
const PR_SET_SECCOMP: c_int = 22;
const SECCOMP_MODE_FILTER: c_long = 2;
const BPF_LD_W_ABS: u16 = 0x20;
const BPF_JMP_JEQ_K: u16 = 0x15;
const BPF_RET_K: u16 = 0x06;
const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const SECCOMP_RET_ERRNO_EPERM: u32 = 0x0005_0001;
const X86_64_SYS_CLONE: u32 = 56;
const X86_64_SYS_FORK: u32 = 57;
const X86_64_SYS_VFORK: u32 = 58;
const X86_64_SYS_CLONE3: u32 = 435;
const X32_SYSCALL_BIT: u32 = 0x4000_0000;
const REAPER_POLL_INTERVAL_V1: Duration = Duration::from_millis(10);
pub const MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1: usize = 64;
const REAP_SLOT_EMPTY: u8 = 0;
const REAP_SLOT_RESERVED: u8 = 1;
const REAP_SLOT_DEFERRED: u8 = 2;

unsafe extern "C" {
    fn close(descriptor: c_int) -> c_int;
    fn close_range(first: c_uint, last: c_uint, flags: c_uint) -> c_int;
    fn dup3(old_descriptor: c_int, new_descriptor: c_int, flags: c_int) -> c_int;
    fn execveat(
        descriptor: c_int,
        path: *const c_char,
        arguments: *const *const c_char,
        environment: *const *const c_char,
        flags: c_int,
    ) -> c_int;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn prctl(option: c_int, ...) -> c_int;
    fn syscall(number: c_long, ...) -> c_long;
    fn write(descriptor: c_int, bytes: *const c_void, length: usize) -> isize;
    fn _exit(status: c_int) -> !;
}

#[repr(C)]
struct CloneArgsV1 {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

#[repr(C)]
struct KernelSigactionV1 {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SockFilterV1 {
    code: u16,
    jump_true: u8,
    jump_false: u8,
    value: u32,
}

#[repr(C)]
struct SockFilterProgramV1 {
    length: u16,
    filters: *const SockFilterV1,
}

struct ExecVectorV1 {
    strings: Vec<CString>,
    pointers: Vec<*const c_char>,
}

// The pointers refer to allocations owned by `strings`; moving this record does not move those
// allocations. The record is immutable before `clone3` creates the child.
unsafe impl Send for ExecVectorV1 {}
unsafe impl Sync for ExecVectorV1 {}

impl ExecVectorV1 {
    fn new(arguments: &[Vec<u8>]) -> Result<Self, HostLinkError> {
        let strings = arguments
            .iter()
            .map(|argument| {
                CString::new(argument.as_slice()).map_err(|_| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidText,
                        "canonical worker argument contains NUL",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut pointers = strings
            .iter()
            .map(|argument| argument.as_ptr())
            .collect::<Vec<_>>();
        pointers.push(std::ptr::null());
        Ok(Self { strings, pointers })
    }

    fn pointers(&self) -> *const *const c_char {
        self.pointers.as_ptr()
    }

    fn keep_alive(&self) {
        debug_assert_eq!(self.pointers.len(), self.strings.len() + 1);
    }
}

struct ChildDescriptorV1 {
    source: OwnedFd,
    target: i32,
}

struct DeferredReapCellV1 {
    state: AtomicU8,
    pidfd: AtomicI32,
}

impl DeferredReapCellV1 {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(REAP_SLOT_EMPTY),
            pidfd: AtomicI32::new(-1),
        }
    }
}

struct DeferredReaperV1 {
    cells: [DeferredReapCellV1; MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1],
    thread_started: OnceLock<bool>,
}

impl DeferredReaperV1 {
    const fn new() -> Self {
        Self {
            cells: [const { DeferredReapCellV1::new() }; MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1],
            thread_started: OnceLock::new(),
        }
    }

    fn ensure_thread(&'static self) -> bool {
        *self.thread_started.get_or_init(|| {
            std::thread::Builder::new()
                .name("fe2o3-host-reaper-v1".to_owned())
                .spawn(move || self.run())
                .is_ok()
        })
    }

    fn reserve(&'static self) -> Result<ReapSlotV1, HostLinkError> {
        if !self.ensure_thread() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerCapacity,
                "could not start the bounded host-link pidfd reaper",
            ));
        }
        for cell in &self.cells {
            if cell
                .state
                .compare_exchange(
                    REAP_SLOT_EMPTY,
                    REAP_SLOT_RESERVED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Ok(ReapSlotV1 { cell, armed: true });
            }
        }
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::WorkerCapacity,
            "authenticated host-link execution/deferred-reap capacity is exhausted",
        ))
    }

    fn run(&'static self) -> ! {
        loop {
            for cell in &self.cells {
                if cell.state.load(Ordering::Acquire) != REAP_SLOT_DEFERRED {
                    continue;
                }
                let raw_pidfd = cell.pidfd.load(Ordering::Acquire);
                if raw_pidfd < 0 {
                    continue;
                }
                // SAFETY: the deferring owner transferred this live pidfd into the cell and no
                // other owner closes it until this event loop observes a successful waitid.
                let pidfd = unsafe { BorrowedFd::borrow_raw(raw_pidfd) };
                if matches!(try_reap_nonblocking(pidfd), ReapPollV1::Reaped(_)) {
                    // SAFETY: a successful waitid is the only transition that takes ownership
                    // back from a deferred cell. Dropping closes the retained pidfd exactly once.
                    drop(unsafe { OwnedFd::from_raw_fd(raw_pidfd) });
                    cell.pidfd.store(-1, Ordering::Release);
                    cell.state.store(REAP_SLOT_EMPTY, Ordering::Release);
                }
            }
            std::thread::sleep(REAPER_POLL_INTERVAL_V1);
        }
    }
}

struct ReapSlotV1 {
    cell: &'static DeferredReapCellV1,
    armed: bool,
}

impl ReapSlotV1 {
    fn complete(mut self) {
        self.cell.state.store(REAP_SLOT_EMPTY, Ordering::Release);
        self.armed = false;
    }

    fn defer(mut self, pidfd: OwnedFd) {
        let raw_pidfd = pidfd.into_raw_fd();
        self.cell.pidfd.store(raw_pidfd, Ordering::Release);
        self.cell.state.store(REAP_SLOT_DEFERRED, Ordering::Release);
        self.armed = false;
    }
}

impl Drop for ReapSlotV1 {
    fn drop(&mut self) {
        if self.armed {
            self.cell.state.store(REAP_SLOT_EMPTY, Ordering::Release);
        }
    }
}

enum ReapPollV1 {
    Pending,
    Reaped(rustix::process::WaitIdStatus),
}

fn classify_reap_result(
    result: Result<Option<rustix::process::WaitIdStatus>, rustix::io::Errno>,
) -> ReapPollV1 {
    match result {
        Ok(Some(status)) => ReapPollV1::Reaped(status),
        Ok(None) | Err(_) => ReapPollV1::Pending,
    }
}

fn try_reap_nonblocking(pidfd: BorrowedFd<'_>) -> ReapPollV1 {
    classify_reap_result(rustix::process::waitid(
        rustix::process::WaitId::PidFd(pidfd),
        rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOHANG,
    ))
}

fn deferred_reaper() -> &'static DeferredReaperV1 {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    &REAPER
}

pub fn authenticated_host_link_available_capacity_v1() -> usize {
    deferred_reaper()
        .cells
        .iter()
        .filter(|cell| cell.state.load(Ordering::Acquire) == REAP_SLOT_EMPTY)
        .count()
}

pub(crate) struct AuthenticatedProcessV1 {
    pidfd: Option<OwnedFd>,
    reap_slot: Option<ReapSlotV1>,
    pid: rustix::process::Pid,
    successful_exit_observed: bool,
}

impl AuthenticatedProcessV1 {
    pub(crate) fn launch(
        tool: File,
        arguments: &[Vec<u8>],
        inherited: Vec<InheritedDescriptorV1>,
        deadline: Instant,
    ) -> Result<Self, HostLinkError> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::UnsupportedPlatform,
                "authenticated host-link clone3 launcher requires Linux x86_64",
            ));
        }
        let reap_slot = deferred_reaper().reserve()?;
        let highest_target = inherited
            .iter()
            .map(InheritedDescriptorV1::child_fd)
            .max()
            .unwrap_or(2);
        let source_floor = highest_target.checked_add(1).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "canonical child descriptor range overflowed",
            )
        })?;
        let tool = rustix::io::fcntl_dupfd_cloexec(&tool, source_floor)
            .context(HostLinkErrorCodeV1::WorkerLaunch, || {
                "duplicate sealed static LLD above the canonical child table".to_owned()
            })?;
        let null = open_validated_null()?;
        let null = rustix::io::fcntl_dupfd_cloexec(&null, tool.as_raw_fd() + 1)
            .context(HostLinkErrorCodeV1::WorkerLaunch, || {
                "duplicate null endpoint above the canonical child table".to_owned()
            })?;
        let mut child_descriptors = Vec::with_capacity(inherited.len());
        let mut next_source = null.as_raw_fd().checked_add(1).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "worker source descriptor range overflowed",
            )
        })?;
        for descriptor in inherited {
            let source = rustix::io::fcntl_dupfd_cloexec(descriptor.file(), next_source).context(
                HostLinkErrorCodeV1::WorkerLaunch,
                || {
                    format!(
                        "duplicate canonical child descriptor {} above its target table",
                        descriptor.child_fd()
                    )
                },
            )?;
            next_source = source.as_raw_fd().checked_add(1).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::FieldTooLarge,
                    "worker source descriptor range overflowed",
                )
            })?;
            child_descriptors.push(ChildDescriptorV1 {
                source,
                target: descriptor.child_fd(),
            });
        }

        let exec_vector = ExecVectorV1::new(arguments)?;
        let (exec_status_read, exec_status_write) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )
        .context(HostLinkErrorCodeV1::WorkerLaunch, || {
            "create authenticated exec-status pipe".to_owned()
        })?;
        let mut pidfd_raw = -1_i32;
        let clone_arguments = CloneArgsV1 {
            flags: CLONE_PIDFD | CLONE_CLEAR_SIGHAND,
            pidfd: (&raw mut pidfd_raw).addr() as u64,
            child_tid: 0,
            parent_tid: 0,
            exit_signal: SIGCHLD,
            stack: 0,
            stack_size: 0,
            tls: 0,
            set_tid: 0,
            set_tid_size: 0,
            cgroup: 0,
        };
        // SAFETY: clone3 receives the exact kernel ABI record. No VM/thread sharing flags are
        // used. The child calls only direct async-signal-safe syscalls over preallocated state and
        // then execs or exits; the kernel writes pidfd_raw atomically before returning to parent.
        let clone_result = unsafe {
            syscall(
                SYS_CLONE3,
                &raw const clone_arguments,
                std::mem::size_of::<CloneArgsV1>(),
            )
        };
        if clone_result < 0 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerLaunch,
                format!(
                    "clone3(CLONE_PIDFD) exact static LLD child: {}",
                    io::Error::last_os_error()
                ),
            ));
        }
        if clone_result == 0 {
            // SAFETY: this branch is the post-clone child and never returns into Rust cleanup.
            unsafe {
                child_exec(
                    tool.as_raw_fd(),
                    null.as_raw_fd(),
                    &child_descriptors,
                    &exec_vector,
                    exec_status_read.as_raw_fd(),
                    exec_status_write.as_raw_fd(),
                )
            }
        }
        drop(exec_status_write);
        let raw_pid = i32::try_from(clone_result).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::WorkerIdentity,
                "clone3 returned a static LLD PID outside Linux pid_t",
            )
        })?;
        let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::WorkerIdentity,
                "clone3 returned an invalid zero static LLD PID",
            )
        })?;
        if pidfd_raw < 0 {
            // SAFETY: clone3 returned this positive PID, and failure to return its requested pidfd
            // is a kernel contract failure. Kill is fail-closed cleanup only.
            unsafe { kill(raw_pid, SIGKILL) };
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerIdentity,
                "clone3 did not atomically return the requested pidfd",
            ));
        }
        // SAFETY: CLONE_PIDFD wrote one newly owned descriptor to pidfd_raw for this parent.
        let pidfd = unsafe { OwnedFd::from_raw_fd(pidfd_raw) };
        let mut process = Self {
            pidfd: Some(pidfd),
            reap_slot: Some(reap_slot),
            pid,
            successful_exit_observed: false,
        };
        if let Err(error) = await_exec_status(&exec_status_read, deadline) {
            let _ = process.kill_and_defer_reap();
            return Err(error);
        }
        Ok(process)
    }

    pub(crate) fn pid(&self) -> u32 {
        u32::try_from(self.pid.as_raw_pid()).expect("authenticated child PID is positive")
    }

    pub(crate) fn poll_successful_exit(&mut self) -> Result<bool, HostLinkError> {
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "static LLD process witness was already transferred for reaping",
            ));
        };
        if self.successful_exit_observed {
            return Ok(true);
        }
        let status = match rustix::process::waitid(
            rustix::process::WaitId::PidFd(pidfd.as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        ) {
            Ok(status) => status,
            Err(error) if error == rustix::io::Errno::INTR => return Ok(false),
            Err(error) => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerIdentity,
                    format!("poll exact clone3 pidfd static LLD child: {error}"),
                ));
            }
        };
        let Some(status) = status else {
            return Ok(false);
        };
        if status.exit_status() != Some(0) {
            let detail = if let Some(code) = status.exit_status() {
                format!("static LLD exited with status {code}")
            } else if let Some(signal) = status.terminating_signal() {
                format!("static LLD terminated by signal {signal}")
            } else {
                "static LLD ended without a successful exit status".to_owned()
            };
            self.finish_or_defer_reap();
            return Err(HostLinkError::new(HostLinkErrorCodeV1::WorkerExit, detail));
        }
        self.successful_exit_observed = true;
        Ok(true)
    }

    pub(crate) fn reap_success(&mut self) -> Result<(), HostLinkError> {
        if !self.successful_exit_observed || self.pidfd.is_none() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "static LLD cannot be reaped before one successful pidfd exit observation",
            ));
        }
        if let Some(status) = self.finish_or_defer_reap()
            && status.exit_status() != Some(0)
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerExit,
                "static LLD status changed between pidfd observation and reap",
            ));
        }
        Ok(())
    }

    pub(crate) fn terminate_for_timeout(&mut self) -> Result<(), HostLinkError> {
        self.kill_and_defer_reap()
    }

    pub(crate) fn terminate_after_admission_failure(&mut self) -> Result<(), HostLinkError> {
        self.kill_and_defer_reap()
    }

    fn kill_and_defer_reap(&mut self) -> Result<(), HostLinkError> {
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Ok(());
        };
        let signal_error =
            match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => None,
                Err(error) => Some(HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerIdentity,
                    format!("pidfd-kill exact static LLD child: {error}"),
                )),
            };
        self.finish_or_defer_reap();
        signal_error.map_or(Ok(()), Err)
    }

    fn finish_or_defer_reap(&mut self) -> Option<rustix::process::WaitIdStatus> {
        let pidfd = self.pidfd.take()?;
        let slot = self
            .reap_slot
            .take()
            .expect("live authenticated process retains one reap slot");
        match try_reap_nonblocking(pidfd.as_fd()) {
            ReapPollV1::Reaped(status) => {
                drop(pidfd);
                slot.complete();
                Some(status)
            }
            ReapPollV1::Pending => {
                slot.defer(pidfd);
                None
            }
        }
    }
}

fn open_validated_null() -> Result<File, HostLinkError> {
    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .context(HostLinkErrorCodeV1::WorkerLaunch, || {
            "open read/write null standard-I/O endpoint".to_owned()
        })?;
    let status = rustix::fs::fstat(&null).context(HostLinkErrorCodeV1::WorkerLaunch, || {
        "inspect null standard-I/O endpoint".to_owned()
    })?;
    if rustix::fs::FileType::from_raw_mode(status.st_mode) != rustix::fs::FileType::CharacterDevice
        || rustix::fs::major(status.st_rdev) != 1
        || rustix::fs::minor(status.st_rdev) != 3
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::WorkerLaunch,
            "null standard-I/O endpoint is not character device 1:3",
        ));
    }
    let flags = rustix::fs::fcntl_getfl(&null)
        .context(HostLinkErrorCodeV1::WorkerLaunch, || {
            "inspect null standard-I/O access mode".to_owned()
        })?;
    if !flags.contains(rustix::fs::OFlags::RDWR) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::WorkerLaunch,
            "null standard-I/O endpoint is not open read/write",
        ));
    }
    Ok(null)
}

impl Drop for AuthenticatedProcessV1 {
    fn drop(&mut self) {
        let _ = self.kill_and_defer_reap();
    }
}

fn await_exec_status(status: &OwnedFd, deadline: Instant) -> Result<(), HostLinkError> {
    let mut record = [0_u8; 2];
    loop {
        match rustix::io::read(status, &mut record) {
            Ok(0) => return Ok(()),
            Ok(1) => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerLaunch,
                    format!("static LLD child rejected pre-exec stage {}", record[0]),
                ));
            }
            Ok(_) => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerLaunch,
                    "static LLD child sent a noncanonical exec-status record",
                ));
            }
            Err(error) if error == rustix::io::Errno::AGAIN => {
                if Instant::now() >= deadline {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::WorkerTimeout,
                        "static LLD did not complete authenticated exec before its wall deadline",
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(error) => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerLaunch,
                    format!("read authenticated static LLD exec status: {error}"),
                ));
            }
        }
    }
}

unsafe fn child_exec(
    tool: c_int,
    null: c_int,
    descriptors: &[ChildDescriptorV1],
    vector: &ExecVectorV1,
    status_read: c_int,
    status_write: c_int,
) -> ! {
    // SAFETY: this function runs only in the post-clone child. Every operation is a direct Linux
    // syscall over preallocated memory; failure reports one bounded stage byte and exits.
    unsafe {
        if normalize_signal_state() != 0 {
            child_fail(status_write, 1);
        }
        close(status_read);
        if close_range(3_u32, u32::MAX, CLOSE_RANGE_CLOEXEC) != 0 {
            child_fail(status_write, 2);
        }
        for target in 0..=2 {
            if dup3(null, target, 0) < 0 {
                child_fail(status_write, 3);
            }
        }
        for descriptor in descriptors {
            if dup3(descriptor.source.as_raw_fd(), descriptor.target, 0) < 0 {
                child_fail(status_write, 4);
            }
        }
        if install_no_descendants_filter() != 0 {
            child_fail(status_write, 5);
        }
        let empty_path = c"";
        let environment = [std::ptr::null::<c_char>()];
        execveat(
            tool,
            empty_path.as_ptr(),
            vector.pointers(),
            environment.as_ptr(),
            AT_EMPTY_PATH,
        );
        vector.keep_alive();
        child_fail(status_write, 6);
    }
}

unsafe fn normalize_signal_state() -> c_int {
    let default_action = KernelSigactionV1 {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    for signal in 1..=KERNEL_SIGNAL_COUNT {
        if signal == SIGKILL || signal == SIGSTOP {
            continue;
        }
        // SAFETY: x86-64 rt_sigaction consumes this exact kernel-layout action and an 8-byte
        // kernel signal set. CLONE_CLEAR_SIGHAND has already atomically removed caught handlers;
        // this loop also normalizes inherited ignored dispositions before unblocking signals.
        if unsafe {
            syscall(
                SYS_RT_SIGACTION,
                signal,
                &raw const default_action,
                std::ptr::null_mut::<KernelSigactionV1>(),
                KERNEL_SIGSET_BYTES,
            )
        } != 0
        {
            return -1;
        }
    }
    let empty_mask = 0_u64;
    // SAFETY: the x86-64 kernel signal set is one u64. All resettable dispositions are default
    // before the inherited mask is cleared, so unblocking cannot invoke ambient user handlers.
    if unsafe {
        syscall(
            SYS_RT_SIGPROCMASK,
            SIG_SETMASK,
            &raw const empty_mask,
            std::ptr::null_mut::<u64>(),
            KERNEL_SIGSET_BYTES,
        )
    } != 0
    {
        return -1;
    }
    0
}

unsafe fn install_no_descendants_filter() -> c_int {
    let filters = [
        SockFilterV1 {
            code: BPF_LD_W_ABS,
            jump_true: 0,
            jump_false: 0,
            value: 4,
        },
        SockFilterV1 {
            code: BPF_JMP_JEQ_K,
            jump_true: 1,
            jump_false: 0,
            value: AUDIT_ARCH_X86_64,
        },
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_KILL_PROCESS,
        },
        SockFilterV1 {
            code: BPF_LD_W_ABS,
            jump_true: 0,
            jump_false: 0,
            value: 0,
        },
        deny_syscall(X86_64_SYS_CLONE),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X86_64_SYS_FORK),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X86_64_SYS_VFORK),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X86_64_SYS_CLONE3),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X32_SYSCALL_BIT | X86_64_SYS_CLONE),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X32_SYSCALL_BIT | X86_64_SYS_FORK),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X32_SYSCALL_BIT | X86_64_SYS_VFORK),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        deny_syscall(X32_SYSCALL_BIT | X86_64_SYS_CLONE3),
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ERRNO_EPERM,
        },
        SockFilterV1 {
            code: BPF_RET_K,
            jump_true: 0,
            jump_false: 0,
            value: SECCOMP_RET_ALLOW,
        },
    ];
    let program = SockFilterProgramV1 {
        length: filters.len() as u16,
        filters: filters.as_ptr(),
    };
    // SAFETY: post-clone child passes exact prctl scalar/filter ABI values over stack data.
    unsafe {
        if prctl(
            PR_SET_NO_NEW_PRIVS,
            1 as c_long,
            0 as c_long,
            0 as c_long,
            0 as c_long,
        ) != 0
        {
            return -1;
        }
        prctl(
            PR_SET_SECCOMP,
            SECCOMP_MODE_FILTER,
            &raw const program,
            0 as c_long,
            0 as c_long,
        )
    }
}

const fn deny_syscall(number: u32) -> SockFilterV1 {
    SockFilterV1 {
        code: BPF_JMP_JEQ_K,
        jump_true: 0,
        jump_false: 1,
        value: number,
    }
}

unsafe fn child_fail(status: c_int, stage: u8) -> ! {
    // SAFETY: post-clone failure reporting is one best-effort async-signal-safe write followed by
    // immediate _exit; no Rust destructor or allocator is entered.
    unsafe {
        write(status, (&raw const stage).cast(), 1);
        _exit(127)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_vector_owns_stable_nul_terminated_arguments() {
        let vector = ExecVectorV1::new(&[b"fe2o3-host-lld".to_vec()]).unwrap();
        assert_eq!(vector.strings[0].as_bytes(), b"fe2o3-host-lld");
        assert!(vector.pointers.last().unwrap().is_null());
    }

    #[test]
    fn descendant_filter_denies_all_x86_64_process_creation_syscalls() {
        let denied = [
            X86_64_SYS_CLONE,
            X86_64_SYS_FORK,
            X86_64_SYS_VFORK,
            X86_64_SYS_CLONE3,
            X32_SYSCALL_BIT | X86_64_SYS_CLONE,
            X32_SYSCALL_BIT | X86_64_SYS_FORK,
            X32_SYSCALL_BIT | X86_64_SYS_VFORK,
            X32_SYSCALL_BIT | X86_64_SYS_CLONE3,
        ];
        assert_eq!(
            denied,
            [
                56,
                57,
                58,
                435,
                0x4000_0038,
                0x4000_0039,
                0x4000_003a,
                0x4000_01b3
            ]
        );
        assert_eq!(SECCOMP_RET_ERRNO_EPERM & 0xffff, 1);
    }

    #[test]
    fn clone_boundary_atomically_clears_caught_signal_handlers() {
        assert_eq!(CLONE_CLEAR_SIGHAND, 0x0000_0001_0000_0000);
        assert_eq!(CLONE_PIDFD | CLONE_CLEAR_SIGHAND, 0x0000_0001_0000_1000);
    }

    #[test]
    fn validated_null_is_character_device_one_three_and_read_write() {
        let null = open_validated_null().unwrap();
        let status = rustix::fs::fstat(&null).unwrap();
        assert_eq!(rustix::fs::major(status.st_rdev), 1);
        assert_eq!(rustix::fs::minor(status.st_rdev), 3);
        assert!(
            rustix::fs::fcntl_getfl(&null)
                .unwrap()
                .contains(rustix::fs::OFlags::RDWR)
        );
    }

    #[test]
    fn reap_slots_are_bounded_and_release_only_when_owned() {
        let mut slots = Vec::new();
        for _ in 0..MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1 {
            slots.push(deferred_reaper().reserve().unwrap());
        }
        assert_eq!(
            deferred_reaper()
                .reserve()
                .err()
                .expect("the bounded reaper must reject its sixty-fifth reservation")
                .code(),
            HostLinkErrorCodeV1::WorkerCapacity
        );
        drop(slots);
        assert_eq!(
            authenticated_host_link_available_capacity_v1(),
            MAX_AUTHENTICATED_HOST_LINK_EXECUTIONS_V1
        );
    }

    #[test]
    fn eintr_and_wait_errors_never_claim_a_successful_reap() {
        assert!(matches!(
            classify_reap_result(Err(rustix::io::Errno::INTR)),
            ReapPollV1::Pending
        ));
        assert!(matches!(
            classify_reap_result(Err(rustix::io::Errno::IO)),
            ReapPollV1::Pending
        ));
        assert!(matches!(
            classify_reap_result(Ok(None)),
            ReapPollV1::Pending
        ));
    }
}
