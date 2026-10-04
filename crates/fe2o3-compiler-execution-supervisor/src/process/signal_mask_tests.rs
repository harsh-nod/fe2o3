use super::*;

std::thread_local! {
    static CHILD_PROBE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn prepare_child_probe() {
    extern "C" fn forbidden_handler(_: c_int) {
        // SAFETY: any inherited handler execution makes the private child fail immediately.
        unsafe { libc::_exit(99) }
    }
    // SAFETY: signal dispositions change only in the dedicated subprocess fixture.
    unsafe {
        assert_ne!(
            libc::signal(libc::SIGWINCH, forbidden_handler as *const () as usize),
            libc::SIG_ERR
        );
        assert_ne!(libc::signal(libc::SIGUSR2, libc::SIG_IGN), libc::SIG_ERR);
    }
    CHILD_PROBE.set(true);
}

pub(super) fn child_probe_enabled() -> bool {
    CHILD_PROBE.get()
}

pub(super) unsafe fn check_child_signals(before: bool) -> bool {
    let mut mask = 0_u64;
    let mut caught = KernelSigactionV1 {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let mut ignored = KernelSigactionV1 {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    // SAFETY: syscall-only post-clone probe over scalar stack state.
    unsafe {
        if libc::syscall(
            libc::SYS_rt_sigprocmask,
            SIG_SETMASK,
            std::ptr::null::<u64>(),
            &raw mut mask,
            KERNEL_SIGSET_BYTES,
        ) != 0
            || libc::syscall(
                libc::SYS_rt_sigaction,
                libc::SIGWINCH,
                std::ptr::null::<KernelSigactionV1>(),
                &raw mut caught,
                KERNEL_SIGSET_BYTES,
            ) != 0
            || libc::syscall(
                libc::SYS_rt_sigaction,
                libc::SIGUSR2,
                std::ptr::null::<KernelSigactionV1>(),
                &raw mut ignored,
                KERNEL_SIGSET_BYTES,
            ) != 0
        {
            return false;
        }
        if before {
            let all = !(1_u64 << (SIGKILL - 1)) & !(1_u64 << (SIGSTOP - 1));
            if mask != all || caught.handler <= 1 || ignored.handler != 1 {
                return false;
            }
            let pid = libc::syscall(libc::SYS_getpid);
            if libc::syscall(libc::SYS_tgkill, pid, pid, libc::SIGWINCH) != 0 {
                return false;
            }
            let mut pending = 0_u64;
            libc::syscall(
                libc::SYS_rt_sigpending,
                &raw mut pending,
                KERNEL_SIGSET_BYTES,
            ) == 0
                && pending & (1 << (libc::SIGWINCH - 1)) != 0
        } else {
            mask == 0 && caught.handler == 0 && ignored.handler == 0
        }
    }
}

std::thread_local! {
    static RESTORE_FAILURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WAIT_FAILURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn fail_waits(count: usize) {
    WAIT_FAILURES.set(count);
}

pub(crate) fn remaining_wait_failures() -> usize {
    WAIT_FAILURES.get()
}

pub(super) fn take_wait_failure() -> bool {
    WAIT_FAILURES.with(|remaining| {
        let count = remaining.get();
        remaining.set(count.saturating_sub(1));
        count != 0
    })
}

pub(crate) fn fail_restorations(count: usize) {
    RESTORE_FAILURES.set(count);
}

pub(super) fn take_restore_failure() -> bool {
    RESTORE_FAILURES.with(|remaining| {
        let count = remaining.get();
        remaining.set(count.saturating_sub(1));
        count != 0
    })
}

fn current_mask() -> u64 {
    let mut mask = 0_u64;
    // SAFETY: readback only, using the exact x86-64 kernel set width.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_rt_sigprocmask,
                SIG_SETMASK,
                std::ptr::null::<u64>(),
                &raw mut mask,
                KERNEL_SIGSET_BYTES,
            )
        },
        0
    );
    mask
}

#[test]
fn complete_kernel_mask_is_thread_local_and_exactly_restored() {
    let original = current_mask();
    let (check, request) = std::sync::mpsc::channel();
    let (reply, checked) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let sibling = current_mask();
            request
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            reply.send(current_mask() == sibling).unwrap();
        });
        let mut outer = SpawnSignalMaskV1::block().unwrap();
        let all = !(1_u64 << (SIGKILL - 1)) & !(1_u64 << (SIGSTOP - 1));
        assert_eq!(current_mask(), all);
        check.send(()).unwrap();
        assert!(
            checked
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
        );
        let partial = (1_u64 << (libc::SIGUSR1 - 1)) | (1_u64 << 31);
        // SAFETY: controlled test-thread state, restored by the outer owner.
        assert_eq!(
            unsafe {
                libc::syscall(
                    libc::SYS_rt_sigprocmask,
                    SIG_SETMASK,
                    &raw const partial,
                    std::ptr::null_mut::<u64>(),
                    KERNEL_SIGSET_BYTES,
                )
            },
            0
        );
        {
            let mut inner = SpawnSignalMaskV1::block().unwrap();
            assert_eq!(current_mask(), all);
            inner.restore().unwrap();
            inner.restore().unwrap();
            assert_eq!(current_mask(), partial);
        }
        outer.restore().unwrap();
    });
    assert_eq!(current_mask(), original);
}

#[test]
fn unwinding_restores_the_calling_threads_mask() {
    let original = current_mask();
    assert!(
        std::panic::catch_unwind(|| {
            let _guard = SpawnSignalMaskV1::block().unwrap();
            panic!("owned mask unwind fixture");
        })
        .is_err()
    );
    assert_eq!(current_mask(), original);
}
