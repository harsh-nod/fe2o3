use core::ffi::{c_char, c_int, c_long, c_void};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

use fe2o3_protected_service_profile::{
    PROTECTED_SERVICE_SECUREBITS_V1, ProtectedServiceCredentialProfileV1,
};

use crate::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
    PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1, ProtectedServiceDescriptorBindingV1,
};

const CLONE_PIDFD: u64 = 0x0000_1000;
const CLONE_CLEAR_SIGHAND: u64 = 0x0000_0001_0000_0000;
const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
const SIGCHLD: u64 = 17;
const SIGKILL: c_int = 9;
const SIGSTOP: c_int = 19;
const KERNEL_SIGNAL_COUNT: c_int = 64;
const KERNEL_SIGSET_BYTES: usize = 8;
const SIG_SETMASK: c_int = 2;
const PR_SET_PDEATHSIG: c_int = 1;
const PR_GET_PDEATHSIG: c_int = 2;
const PR_SET_DUMPABLE: c_int = 4;
const PR_GET_DUMPABLE: c_int = 3;
const PR_CAPBSET_READ: c_int = 23;
const PR_CAPBSET_DROP: c_int = 24;
const PR_GET_SECUREBITS: c_int = 27;
const PR_SET_SECUREBITS: c_int = 28;
const PR_SET_NO_NEW_PRIVS: c_int = 38;
const PR_GET_NO_NEW_PRIVS: c_int = 39;
const PR_CAP_AMBIENT: c_int = 47;
const PR_CAP_AMBIENT_IS_SET: c_int = 1;
const PR_CAP_AMBIENT_CLEAR_ALL: c_int = 4;
const RLIMIT_CORE: c_int = 4;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const FAILURE_BASE: u8 = 0xc0;

pub(crate) fn has_exact_root_identity() -> bool {
    let mut uids = [u32::MAX; 3];
    let mut gids = [u32::MAX; 3];
    // SAFETY: pointers name writable scalars; sentinel setfsid calls perform readback only.
    unsafe {
        libc::syscall(
            libc::SYS_getresuid,
            &raw mut uids[0],
            &raw mut uids[1],
            &raw mut uids[2],
        ) == 0
            && libc::syscall(
                libc::SYS_getresgid,
                &raw mut gids[0],
                &raw mut gids[1],
                &raw mut gids[2],
            ) == 0
            && uids == [0; 3]
            && gids == [0; 3]
            && libc::syscall(libc::SYS_setfsuid, u32::MAX) == 0
            && libc::syscall(libc::SYS_setfsgid, u32::MAX) == 0
    }
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
struct LinuxCapabilityHeaderV1 {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LinuxCapabilityDataV1 {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

#[repr(C)]
struct LinuxRlimit64V1 {
    current: u64,
    maximum: u64,
}

struct StagedBindingV1 {
    source: OwnedFd,
    destination: RawFd,
}

pub(crate) struct StagedProtectedServiceExecV1 {
    executable: OwnedFd,
    bindings: Vec<StagedBindingV1>,
    profile_ready_writer: OwnedFd,
    gate_reader: OwnedFd,
    exec_status_writer: OwnedFd,
}

impl StagedProtectedServiceExecV1 {
    pub(crate) fn new(
        executable: &File,
        bindings: &[ProtectedServiceDescriptorBindingV1<'_>],
        profile_ready_writer: BorrowedFd<'_>,
        gate_reader: BorrowedFd<'_>,
        exec_status_writer: BorrowedFd<'_>,
    ) -> io::Result<Self> {
        let mut next = PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1;
        let executable = duplicate_above(executable.as_fd(), &mut next)?;
        let mut staged_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            staged_bindings.push(StagedBindingV1 {
                source: duplicate_above(binding.source, &mut next)?,
                destination: binding.destination,
            });
        }
        Ok(Self {
            executable,
            bindings: staged_bindings,
            profile_ready_writer: duplicate_above(profile_ready_writer, &mut next)?,
            gate_reader: duplicate_above(gate_reader, &mut next)?,
            exec_status_writer: duplicate_above(exec_status_writer, &mut next)?,
        })
    }

    pub(crate) fn descriptor_count(&self) -> usize {
        self.bindings.len()
    }
}

fn duplicate_above(source: BorrowedFd<'_>, next: &mut RawFd) -> io::Result<OwnedFd> {
    let duplicate = rustix::io::fcntl_dupfd_cloexec(source, *next).map_err(io::Error::from)?;
    *next = duplicate
        .as_raw_fd()
        .checked_add(1)
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    Ok(duplicate)
}

pub(crate) fn spawn(
    staged: &StagedProtectedServiceExecV1,
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
    expected_parent: rustix::process::Pid,
) -> io::Result<RootOwnedProtectedServiceChildV1> {
    let mut pidfd_raw = -1_i32;
    let arguments = CloneArgsV1 {
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
    // SAFETY: clone3 receives the exact Linux ABI record without VM or file-table sharing. The
    // child executes direct syscalls only and cannot return into Rust.
    let result = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &raw const arguments,
            std::mem::size_of::<CloneArgsV1>(),
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    if result == 0 {
        // SAFETY: this is the direct post-clone child; child_exec always execs or exits.
        unsafe {
            child_exec(
                staged,
                credentials,
                cap_last_cap,
                expected_parent.as_raw_pid(),
            )
        }
    }
    let raw_pid =
        i32::try_from(result).map_err(|_| io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let pid = rustix::process::Pid::from_raw(raw_pid)
        .ok_or_else(|| io::Error::from_raw_os_error(libc::ESRCH))?;
    if pidfd_raw < 0 {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        reap_pid(pid);
        return Err(io::Error::from_raw_os_error(libc::EBADFD));
    }
    // SAFETY: successful CLONE_PIDFD installed one fresh descriptor in pidfd_raw.
    let pidfd = unsafe { OwnedFd::from_raw_fd(pidfd_raw) };
    let flags = match rustix::io::fcntl_getfd(&pidfd) {
        Ok(flags) => flags,
        Err(source) => {
            terminate_and_reap(pid, &pidfd);
            return Err(source.into());
        }
    };
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        terminate_and_reap(pid, &pidfd);
        return Err(io::Error::from_raw_os_error(libc::EPERM));
    }
    Ok(RootOwnedProtectedServiceChildV1 {
        pid,
        pidfd: Some(pidfd),
    })
}

pub(crate) struct RootOwnedProtectedServiceChildV1 {
    pid: rustix::process::Pid,
    pidfd: Option<OwnedFd>,
}

impl RootOwnedProtectedServiceChildV1 {
    #[cfg(feature = "test-support")]
    pub(crate) fn admit_non_authoritative_test(
        pid: rustix::process::Pid,
        pidfd: OwnedFd,
    ) -> io::Result<Self> {
        if !rustix::io::fcntl_getfd(&pidfd)
            .map_err(io::Error::from)?
            .contains(rustix::io::FdFlags::CLOEXEC)
        {
            return Err(io::Error::from_raw_os_error(libc::EPERM));
        }
        Ok(Self {
            pid,
            pidfd: Some(pidfd),
        })
    }

    pub(crate) const fn pid(&self) -> rustix::process::Pid {
        self.pid
    }

    fn pidfd(&self) -> &OwnedFd {
        self.pidfd.as_ref().expect("live child retains pidfd")
    }

    pub(crate) fn is_live(&self) -> io::Result<bool> {
        match rustix::process::waitid(
            rustix::process::WaitId::PidFd(self.pidfd().as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        ) {
            Ok(None) | Err(rustix::io::Errno::INTR) => Ok(true),
            Ok(Some(_)) => Ok(false),
            Err(source) => Err(source.into()),
        }
    }

    pub(crate) fn try_clone_pidfd(&self) -> io::Result<OwnedFd> {
        rustix::io::fcntl_dupfd_cloexec(self.pidfd(), 0).map_err(io::Error::from)
    }

    pub(crate) fn exit_description(&self, fallback: &'static str) -> String {
        rustix::process::waitid(
            rustix::process::WaitId::PidFd(self.pidfd().as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .ok()
        .flatten()
        .map_or_else(|| fallback.to_owned(), |status| format!("{status:?}"))
    }

    pub(crate) fn cancel_and_reap(&mut self) -> Result<(), ReapErrorV1> {
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Ok(());
        };
        let signal_error =
            match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => None,
                Err(pidfd_source) => {
                    match rustix::process::kill_process(self.pid, rustix::process::Signal::KILL) {
                        Ok(()) | Err(rustix::io::Errno::SRCH) => {
                            Some(io::Error::from(pidfd_source))
                        }
                        Err(_) => return Err(ReapErrorV1::Io(pidfd_source.into())),
                    }
                }
            };
        let wait_result = loop {
            match rustix::process::waitid(
                rustix::process::WaitId::PidFd(pidfd.as_fd()),
                rustix::process::WaitIdOptions::EXITED,
            ) {
                Ok(Some(_)) => break Ok(()),
                Ok(None) | Err(rustix::io::Errno::INTR) => {}
                Err(rustix::io::Errno::CHILD) => break Err(ReapErrorV1::OwnershipLost),
                Err(source) => break Err(ReapErrorV1::Io(source.into())),
            }
        };
        match wait_result {
            Ok(()) => {
                self.pidfd.take();
                signal_error.map_or(Ok(()), |source| Err(ReapErrorV1::Io(source)))
            }
            Err(ReapErrorV1::OwnershipLost) => {
                self.pidfd.take();
                Err(ReapErrorV1::OwnershipLost)
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for RootOwnedProtectedServiceChildV1 {
    fn drop(&mut self) {
        let _ = self.cancel_and_reap();
    }
}

pub(crate) enum ReapErrorV1 {
    OwnershipLost,
    Io(io::Error),
}

fn terminate_and_reap(pid: rustix::process::Pid, pidfd: &OwnedFd) {
    let signaled = match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => true,
        Err(_) => matches!(
            rustix::process::kill_process(pid, rustix::process::Signal::KILL),
            Ok(()) | Err(rustix::io::Errno::SRCH)
        ),
    };
    if signaled {
        loop {
            match rustix::process::waitid(
                rustix::process::WaitId::PidFd(pidfd.as_fd()),
                rustix::process::WaitIdOptions::EXITED,
            ) {
                Ok(Some(_)) | Err(rustix::io::Errno::CHILD) => break,
                Ok(None) | Err(rustix::io::Errno::INTR) => {}
                Err(_) => break,
            }
        }
    }
}

fn reap_pid(pid: rustix::process::Pid) {
    loop {
        match rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::empty()) {
            Ok(Some(_)) | Err(rustix::io::Errno::CHILD) => return,
            Ok(None) | Err(rustix::io::Errno::INTR) => {}
            Err(_) => return,
        }
    }
}

unsafe fn child_exec(
    staged: &StagedProtectedServiceExecV1,
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
    expected_parent: i32,
) -> ! {
    // SAFETY: every operation below is a direct scalar syscall over inherited storage.
    unsafe {
        if normalize_signal_state() != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 1);
        }
        if arm_parent_death(expected_parent) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 2);
        }
        if establish_profile(credentials, cap_last_cap) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 3);
        }
        let ready = PROTECTED_SERVICE_PROFILE_READY_V1;
        if libc::write(
            staged.profile_ready_writer.as_raw_fd(),
            (&raw const ready).cast::<c_void>(),
            1,
        ) != 1
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 4);
        }
        let mut release = 0_u8;
        loop {
            let count = libc::read(
                staged.gate_reader.as_raw_fd(),
                (&raw mut release).cast::<c_void>(),
                1,
            );
            if count == 1 {
                break;
            }
            if count < 0 && *libc::__errno_location() == libc::EINTR {
                continue;
            }
            child_fail(staged.exec_status_writer.as_raw_fd(), 5);
        }
        if release != PROTECTED_SERVICE_GATE_RELEASE_V1 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 6);
        }
        if libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, CLOSE_RANGE_CLOEXEC) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 7);
        }
        for binding in &staged.bindings {
            if libc::dup3(binding.source.as_raw_fd(), binding.destination, 0) != binding.destination
            {
                child_fail(staged.exec_status_writer.as_raw_fd(), 8);
            }
        }
        libc::close(0);
        libc::close(1);
        libc::close(2);
        let name = c"fe2o3-protected-service";
        let arguments = [name.as_ptr().cast_mut(), std::ptr::null_mut()];
        let environment = [std::ptr::null_mut::<c_char>()];
        libc::syscall(
            libc::SYS_execveat,
            staged.executable.as_raw_fd(),
            c"".as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
        child_fail(staged.exec_status_writer.as_raw_fd(), 9)
    }
}

unsafe fn normalize_signal_state() -> c_int {
    let action = KernelSigactionV1 {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    for signal in 1..=KERNEL_SIGNAL_COUNT {
        if signal == SIGKILL || signal == SIGSTOP {
            continue;
        }
        // SAFETY: x86-64 rt_sigaction consumes this exact kernel layout and sigset width.
        if unsafe {
            libc::syscall(
                libc::SYS_rt_sigaction,
                signal,
                &raw const action,
                std::ptr::null_mut::<KernelSigactionV1>(),
                KERNEL_SIGSET_BYTES,
            )
        } != 0
        {
            return -1;
        }
    }
    let empty = 0_u64;
    // SAFETY: the x86-64 kernel sigset is one u64.
    if unsafe {
        libc::syscall(
            libc::SYS_rt_sigprocmask,
            SIG_SETMASK,
            &raw const empty,
            std::ptr::null_mut::<u64>(),
            KERNEL_SIGSET_BYTES,
        )
    } != 0
    {
        return -1;
    }
    0
}

unsafe fn arm_parent_death(expected_parent: i32) -> c_int {
    let mut observed = 0_i32;
    // SAFETY: scalar getppid/prctl operations run in the direct child.
    if unsafe { libc::syscall(libc::SYS_getppid) } != c_long::from(expected_parent)
        || unsafe { libc::prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_getppid) } != c_long::from(expected_parent)
        || unsafe { libc::prctl(PR_GET_PDEATHSIG, &raw mut observed, 0, 0, 0) } != 0
        || observed != SIGKILL
    {
        return -1;
    }
    0
}

unsafe fn establish_profile(
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
) -> c_int {
    let core = LinuxRlimit64V1 {
        current: 0,
        maximum: 0,
    };
    // SAFETY: fixed scalar arguments and local Linux ABI records are supplied throughout.
    if unsafe { libc::syscall(libc::SYS_umask, 0o077_u32) } < 0
        || unsafe {
            libc::syscall(
                libc::SYS_prlimit64,
                0,
                RLIMIT_CORE,
                &raw const core,
                std::ptr::null_mut::<LinuxRlimit64V1>(),
            )
        } != 0
        || unsafe { libc::prctl(PR_SET_SECUREBITS, PROTECTED_SERVICE_SECUREBITS_V1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 0, 0, 0) } != 0
    {
        return -1;
    }
    for capability in 0..=cap_last_cap {
        if unsafe { libc::prctl(PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0 {
            return -1;
        }
    }
    if unsafe { libc::syscall(libc::SYS_setgroups, 0_usize, std::ptr::null::<u32>()) } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_setresgid,
                credentials.gid(),
                credentials.gid(),
                credentials.gid(),
            )
        } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_setresuid,
                credentials.uid(),
                credentials.uid(),
                credentials.uid(),
            )
        } != 0
    {
        return -1;
    }
    let mut header = LinuxCapabilityHeaderV1 {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let empty = [LinuxCapabilityDataV1 {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    if unsafe { libc::syscall(libc::SYS_capset, &raw mut header, empty.as_ptr()) } != 0
        || unsafe { libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0
    {
        return -1;
    }
    // SAFETY: direct readback validates complete child-local profile state.
    unsafe { validate_profile(credentials, cap_last_cap) }
}

unsafe fn validate_profile(
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
) -> c_int {
    let mut uids = [u32::MAX; 3];
    let mut gids = [u32::MAX; 3];
    let mut header = LinuxCapabilityHeaderV1 {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut data = [LinuxCapabilityDataV1 {
        effective: u32::MAX,
        permitted: u32::MAX,
        inheritable: u32::MAX,
    }; 2];
    let mut core = LinuxRlimit64V1 {
        current: u64::MAX,
        maximum: u64::MAX,
    };
    // SAFETY: pointers identify writable direct-child stack storage.
    if unsafe {
        libc::syscall(
            libc::SYS_getresuid,
            &raw mut uids[0],
            &raw mut uids[1],
            &raw mut uids[2],
        )
    } != 0
        || uids != [credentials.uid(); 3]
        || unsafe {
            libc::syscall(
                libc::SYS_getresgid,
                &raw mut gids[0],
                &raw mut gids[1],
                &raw mut gids[2],
            )
        } != 0
        || gids != [credentials.gid(); 3]
        || unsafe { libc::syscall(libc::SYS_setfsuid, u32::MAX) } != c_long::from(credentials.uid())
        || unsafe { libc::syscall(libc::SYS_setfsgid, u32::MAX) } != c_long::from(credentials.gid())
        || unsafe { libc::syscall(libc::SYS_getgroups, 0_usize, std::ptr::null_mut::<u32>()) } != 0
        || unsafe { libc::syscall(libc::SYS_capget, &raw mut header, data.as_mut_ptr()) } != 0
        || data
            .iter()
            .any(|value| value.effective != 0 || value.permitted != 0 || value.inheritable != 0)
    {
        return -1;
    }
    for capability in 0..=cap_last_cap {
        if unsafe { libc::prctl(PR_CAPBSET_READ, capability, 0, 0, 0) } != 0
            || unsafe { libc::prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, capability, 0, 0) } != 0
        {
            return -1;
        }
    }
    if unsafe { libc::prctl(PR_GET_SECUREBITS, 0, 0, 0, 0) }
        != PROTECTED_SERVICE_SECUREBITS_V1 as c_int
        || unsafe { libc::prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        || unsafe { libc::prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_prlimit64,
                0,
                RLIMIT_CORE,
                std::ptr::null::<LinuxRlimit64V1>(),
                &raw mut core,
            )
        } != 0
        || core.current != 0
        || core.maximum != 0
        || unsafe { libc::syscall(libc::SYS_umask, 0o077_u32) } != 0o077
    {
        return -1;
    }
    0
}

unsafe fn child_fail(exec_status: RawFd, stage: u8) -> ! {
    let message = FAILURE_BASE.saturating_add(stage);
    // SAFETY: exec_status is the staged seqpacket and message names one live byte.
    unsafe {
        let _ = libc::send(
            exec_status,
            (&raw const message).cast::<c_void>(),
            1,
            libc::MSG_NOSIGNAL,
        );
        libc::_exit(126)
    }
}
