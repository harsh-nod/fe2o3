use super::*;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) const CASE: &str = "FE2O3_CLONE_COMPAT_TEST_CASE";

fn run_case(case: &str) {
    run_named("clone_compat::tests::isolated_case", case);
}

pub(crate) fn run_named(name: &str, case: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env_clear()
        .env(CASE, case)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()
        .unwrap();
    let pid = rustix::process::Pid::from_raw(child.id() as i32).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let observed = loop {
        let status = rustix::process::waitid(
            rustix::process::WaitId::Pid(pid),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .unwrap();
        if status.is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    // Keep the leader waitable until the final group signal, including on timeout.
    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    let status = child.wait().unwrap();
    assert!(observed && status.success(), "{case}: {status}");
}

#[test]
fn seccomp_enosys_preserves_atomic_pidfd_and_exact_parent_mask() {
    run_case("success");
}
#[test]
fn fallback_blocks_inherited_handlers_before_child_signal_reset() {
    run_case("handler");
}
#[test]
fn other_clone3_errors_never_select_legacy_clone() {
    run_case("eperm");
    run_case("einval");
}
#[test]
fn failed_legacy_clone_restores_parent_mask_without_child() {
    run_case("clone-error");
}
#[test]
fn failed_parent_mask_restore_keeps_child_in_owned_cleanup() {
    run_case("restore-error");
}

#[test]
#[ignore = "requires the separately admitted root namespace with exact five service capabilities"]
fn root_service_namespace_filter_keeps_atomic_child_custody() {
    assert_eq!(rustix::process::getuid().as_raw(), 0);
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(rustix::process::getgid().as_raw(), 0);
    assert_eq!(rustix::process::getegid().as_raw(), 0);
    let expected = (1_u64 << 0) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8);
    let mut header = [0x20080522_u32, 0];
    let mut data = [0_u32; 6];
    // SAFETY: exact Linux v3 capget layout: one header and two three-u32 records.
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_capget, header.as_mut_ptr(), data.as_mut_ptr()) },
        0
    );
    assert_eq!(u64::from(data[0]) | u64::from(data[3]) << 32, expected);
    assert_eq!(u64::from(data[1]) | u64::from(data[4]) << 32, expected);
    assert_eq!((data[2], data[5]), (0, 0));
    for capability in 0..=40 {
        // SAFETY: read-only scalar bounding/ambient capability queries.
        assert_eq!(
            unsafe { libc::prctl(libc::PR_CAPBSET_READ, capability, 0, 0, 0) },
            i32::from(expected & (1 << capability) != 0)
        );
        assert_eq!(
            unsafe {
                libc::prctl(
                    libc::PR_CAP_AMBIENT,
                    libc::PR_CAP_AMBIENT_IS_SET,
                    capability,
                    0,
                    0,
                )
            },
            0
        );
    }
    for case in [
        "success",
        "handler",
        "eperm",
        "einval",
        "clone-error",
        "restore-error",
    ] {
        run_case(case);
    }
}

fn instruction(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

pub(crate) fn install_filter(clone3_error: i32, clone_error: bool, restore_error: bool) {
    const LD: u16 = 0x20;
    const JEQ: u16 = 0x15;
    const JSET: u16 = 0x45;
    const RET: u16 = 0x06;
    const ALLOW: u32 = 0x7fff0000;
    const ERRNO: u32 = 0x00050000;
    let rules = [
        instruction(LD, 0, 0, 4),
        instruction(JEQ, 1, 0, 0xc000003e),
        instruction(RET, 0, 0, 0x80000000),
        instruction(LD, 0, 0, 0),
        instruction(JEQ, 0, 1, libc::SYS_clone3 as u32),
        instruction(RET, 0, 0, ERRNO | clone3_error as u32),
        instruction(JEQ, 0, 1, libc::SYS_unshare as u32),
        instruction(RET, 0, 0, ERRNO | libc::EPERM as u32),
        instruction(JEQ, 0, 1, libc::SYS_setns as u32),
        instruction(RET, 0, 0, ERRNO | libc::EPERM as u32),
        instruction(JEQ, 0, 3, libc::SYS_clone as u32),
        instruction(LD, 0, 0, 16),
        instruction(JSET, 0, 1, if clone_error { u32::MAX } else { 0x7e020000 }),
        instruction(RET, 0, 0, ERRNO | libc::EPERM as u32),
        instruction(LD, 0, 0, 0),
        instruction(JEQ, 0, 3, libc::SYS_rt_sigprocmask as u32),
        instruction(LD, 0, 0, 16),
        instruction(
            JEQ,
            0,
            1,
            if restore_error {
                SIG_SETMASK as u32
            } else {
                u32::MAX
            },
        ),
        instruction(RET, 0, 0, ERRNO | libc::EPERM as u32),
        instruction(RET, 0, 0, ALLOW),
    ];
    let program = libc::sock_fprog {
        len: rules.len() as u16,
        filter: rules.as_ptr().cast_mut(),
    };
    // SAFETY: this isolated subprocess installs a closed x86-64 filter on itself.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program), 0);
    }
}

fn mask() -> u64 {
    let mut value = 0;
    // SAFETY: read-only query into one live kernel mask.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_rt_sigprocmask,
                SIG_BLOCK,
                std::ptr::null::<u64>(),
                &raw mut value,
                SIGSET_BYTES,
            )
        },
        0
    );
    value
}

extern "C" fn inherited_handler(_: c_int) {
    // SAFETY: a test sentinel; executing this handler in a cloned child is a failure.
    unsafe { libc::_exit(99) }
}

#[test]
fn isolated_case() {
    let Ok(case) = std::env::var(CASE) else {
        return;
    };
    let added = 1_u64 << (libc::SIGUSR2 - 1);
    // SAFETY: only this subprocess's main thread is modified.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_rt_sigprocmask,
                SIG_BLOCK,
                &raw const added,
                std::ptr::null_mut::<u64>(),
                SIGSET_BYTES,
            )
        },
        0
    );
    let before = mask();
    if case == "handler" {
        // SAFETY: installs a sentinel in this subprocess, inherited but blocked in the child.
        unsafe {
            libc::signal(
                libc::SIGUSR1,
                inherited_handler as *const () as libc::sighandler_t,
            );
        }
    }
    let clone3_error = match case.as_str() {
        "eperm" => libc::EPERM,
        "einval" => libc::EINVAL,
        _ => libc::ENOSYS,
    };
    install_filter(clone3_error, case == "clone-error", case == "restore-error");
    // SAFETY: these probes cannot create a namespace: the actual filter denies both operations.
    unsafe {
        assert_eq!(libc::syscall(libc::SYS_unshare, libc::CLONE_NEWUSER), -1);
        assert_eq!(*libc::__errno_location(), libc::EPERM);
        assert_eq!(
            libc::syscall(
                libc::SYS_clone,
                libc::CLONE_NEWUSER as u64 | SIGCHLD,
                0_usize,
                0_usize,
                0_usize,
                0_usize
            ),
            -1
        );
        assert_eq!(*libc::__errno_location(), libc::EPERM);
    }
    let mut raw_fd = -1;
    // SAFETY: child branch below uses only direct syscalls then _exit; parent adopts pidfd immediately.
    let result = unsafe { clone_unmapped(&mut raw_fd) };
    if matches!(case.as_str(), "eperm" | "einval" | "clone-error") {
        assert!(
            matches!(result, Err(e) if e.raw_os_error() == if case == "einval" { libc::EINVAL } else { libc::EPERM })
        );
        assert_eq!(raw_fd, -1);
        assert_eq!(mask(), before);
        return;
    }
    let (pid, pending_mask) = result.unwrap();
    if pid == 0 {
        // SAFETY: the fork child neither allocates nor runs Rust destructors.
        unsafe {
            if case == "handler" {
                libc::syscall(
                    libc::SYS_kill,
                    libc::syscall(libc::SYS_getpid),
                    libc::SIGUSR1,
                );
                // The sentinel must not execute before dispositions can be reset.
                let action = [0_u64; 4];
                libc::syscall(
                    libc::SYS_rt_sigaction,
                    libc::SIGUSR1,
                    action.as_ptr(),
                    0_usize,
                    8_usize,
                );
                let empty = 0_u64;
                libc::syscall(
                    libc::SYS_rt_sigprocmask,
                    SIG_SETMASK,
                    &raw const empty,
                    0_usize,
                    8_usize,
                );
                libc::_exit(98);
            }
            if case == "restore-error" {
                loop {
                    libc::syscall(libc::SYS_pause);
                }
            }
            libc::_exit(7);
        }
    }
    assert!(pid > 0 && raw_fd >= 0);
    // SAFETY: CLONE_PIDFD installed this fresh owned descriptor atomically.
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    assert!(
        rustix::io::fcntl_getfd(&fd)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    let restored = pending_mask.restore();
    if case == "restore-error" {
        assert_eq!(restored, Err(rustix::io::Errno::PERM));
        let _ = rustix::process::pidfd_send_signal(&fd, rustix::process::Signal::KILL);
    } else {
        restored.unwrap();
        assert_eq!(mask(), before);
    }
    // SAFETY: waitid initializes the live siginfo record for this exact owned pidfd.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe {
            libc::waitid(
                libc::P_PIDFD,
                fd.as_raw_fd() as u32,
                &raw mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        },
        0
    );
    assert_eq!(unsafe { info.si_pid() }, pid as i32);
    let observed = rustix::process::waitid(
        rustix::process::WaitId::PidFd(fd.as_fd()),
        rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOWAIT,
    )
    .unwrap()
    .unwrap();
    if case == "handler" {
        assert_eq!(observed.terminating_signal(), Some(libc::SIGUSR1));
    } else if case == "success" {
        assert_eq!(observed.exit_status(), Some(7));
    }
    let terminal = rustix::process::waitid(
        rustix::process::WaitId::PidFd(fd.as_fd()),
        rustix::process::WaitIdOptions::EXITED,
    )
    .unwrap()
    .unwrap();
    assert_eq!(terminal.raw_code(), observed.raw_code());
    assert_eq!(terminal.exit_status(), observed.exit_status());
    assert_eq!(terminal.terminating_signal(), observed.terminating_signal());
    assert!(matches!(
        rustix::process::waitid(
            rustix::process::WaitId::PidFd(fd.as_fd()),
            rustix::process::WaitIdOptions::EXITED
        ),
        Err(rustix::io::Errno::CHILD)
    ));
}
