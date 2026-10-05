//! Atomic pidfd creation for an unmapped child under namespace-denying seccomp.

use core::ffi::{c_int, c_long};

const CLONE_PIDFD: u64 = 0x1000;
const CLONE_CLEAR_SIGHAND: u64 = 1 << 32;
const SIGCHLD: u64 = 17;
const SIG_BLOCK: c_int = 0;
const SIG_SETMASK: c_int = 2;
const SIGSET_BYTES: usize = 8;

/// Additional worst-case parent work: failed clone3, mask block, clone and restore.
pub const WORK: usize = 4 * (1024 + 64);
/// Signal-mask scalars and the returned pending-restoration record.
pub const SCRATCH: usize = 2 * size_of::<u64>() + size_of::<ParentSignalMask>();

#[repr(C)]
struct CloneArgs {
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

/// A blocked calling-thread mask; restore only after adopting all child custody.
///
/// The pending restoration cannot move to another thread:
/// ```compile_fail
/// use fe2o3_protected_service_spawn::clone_compat::ParentSignalMask;
/// let mask = ParentSignalMask::unchanged();
/// std::thread::spawn(move || mask.restore());
/// ```
/// Nor can its state be shared between threads:
/// ```compile_fail
/// use fe2o3_protected_service_spawn::clone_compat::ParentSignalMask;
/// fn require_sync<T: Sync>() {}
/// require_sync::<ParentSignalMask>();
/// ```
#[must_use]
pub struct ParentSignalMask {
    previous: Option<u64>,
    thread_affine: core::marker::PhantomData<*mut ()>,
}

impl ParentSignalMask {
    /// Represents a clone3 path that never changed the calling thread's mask.
    pub const fn unchanged() -> Self {
        Self {
            previous: None,
            thread_affine: core::marker::PhantomData,
        }
    }

    /// Restores the caller's exact prior mask after its child is cleanup-owned.
    ///
    /// Refusal is a launch error, so the already-adopted cleanup owner must not
    /// release the child's execution gate or retire unresolved cleanup custody.
    pub fn restore(self) -> rustix::io::Result<()> {
        if let Some(previous) = self.previous {
            // SAFETY: this is a live scalar kernel signal set for this thread.
            if unsafe { restore_mask(&previous) } != 0 {
                // SAFETY: failed syscall set this thread's errno.
                return Err(rustix::io::Errno::from_raw_os_error(unsafe {
                    *libc::__errno_location()
                }));
            }
        }
        Ok(())
    }
}

unsafe fn restore_mask(previous: &u64) -> c_long {
    // SAFETY: both the ABI size and live input pointer are exact for Linux x86-64.
    unsafe {
        libc::syscall(
            libc::SYS_rt_sigprocmask,
            SIG_SETMASK,
            previous as *const u64,
            std::ptr::null_mut::<u64>(),
            SIGSET_BYTES,
        )
    }
}

/// Creates only an ordinary direct child with an atomic CLOEXEC pidfd.
///
/// No namespace, cgroup, VM, thread, descriptor-table or signal-handler sharing
/// argument is exposed. Only clone3 ENOSYS selects legacy clone(CLONE_PIDFD).
///
/// # Safety
/// On zero return the caller must immediately enter its direct-syscall-only child
/// entry, reset every signal disposition before clearing the inherited blocked
/// mask, and exec or exit without Rust cleanup. On positive return it must adopt
/// the pid/pidfd and every pre-clone lease before restoring ParentSignalMask.
/// The caller must own SIGCHLD/wait custody and keep `pidfd` alive throughout.
pub unsafe fn clone_unmapped(pidfd: &mut c_int) -> rustix::io::Result<(c_long, ParentSignalMask)> {
    let args = CloneArgs {
        flags: CLONE_PIDFD | CLONE_CLEAR_SIGHAND,
        pidfd: (pidfd as *mut c_int).addr() as u64,
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
    // SAFETY: a closed ABI record, no sharing flags, and the caller's child contract.
    let result =
        unsafe { libc::syscall(libc::SYS_clone3, &raw const args, size_of::<CloneArgs>()) };
    if result >= 0 {
        return Ok((result, ParentSignalMask::unchanged()));
    }
    // SAFETY: failed syscall set this thread's errno.
    let error = unsafe { *libc::__errno_location() };
    if error != libc::ENOSYS {
        return Err(rustix::io::Errno::from_raw_os_error(error));
    }

    // Legacy clone cannot carry CLONE_CLEAR_SIGHAND. Block all catchable signals
    // in this thread before cloning; the existing child entry resets dispositions
    // before unmasking. Other parent threads and their masks are untouched.
    let all = u64::MAX;
    let mut previous = 0_u64;
    // SAFETY: exact live kernel mask storage; this operation creates no child.
    if unsafe {
        libc::syscall(
            libc::SYS_rt_sigprocmask,
            SIG_BLOCK,
            &raw const all,
            &raw mut previous,
            SIGSET_BYTES,
        )
    } != 0
    {
        // SAFETY: failed syscall set this thread's errno.
        return Err(rustix::io::Errno::from_raw_os_error(unsafe {
            *libc::__errno_location()
        }));
    }
    // SAFETY: raw x86-64 clone uses parent_tid as the atomic pidfd destination.
    // No CLONE_PARENT_SETTID, namespace, shared state or alternate stack is used.
    let result = unsafe {
        libc::syscall(
            libc::SYS_clone,
            CLONE_PIDFD | SIGCHLD,
            0_usize,
            pidfd as *mut c_int,
            0_usize,
            0_usize,
        )
    };
    if result < 0 {
        // SAFETY: preserve the clone error before restoring this thread's mask.
        let error = unsafe { *libc::__errno_location() };
        ParentSignalMask {
            previous: Some(previous),
            thread_affine: core::marker::PhantomData,
        }
        .restore()?;
        return Err(rustix::io::Errno::from_raw_os_error(error));
    }
    Ok((
        result,
        ParentSignalMask {
            previous: if result == 0 { None } else { Some(previous) },
            thread_affine: core::marker::PhantomData,
        },
    ))
}

#[cfg(test)]
#[path = "clone_compat_tests.rs"]
pub(crate) mod tests;
