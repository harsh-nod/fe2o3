use super::*;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

// Test-only admission of an ordinary child, not a production launcher authority.
fn ordinary_child() -> (std::process::Child, RootOwnedProtectedServiceChildV1) {
    let mut process = std::process::Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid = rustix::process::Pid::from_raw(process.id() as i32).unwrap();
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            panic!("test pidfd: {error}");
        }
    };
    (
        process,
        RootOwnedProtectedServiceChildV1 {
            pid,
            pidfd: Some(pidfd),
            reaping_ownership_lost: false,
        },
    )
}

#[test]
fn polling_cancellation_reaps_original_child_and_is_idempotent() {
    let (_process, mut child) = ordinary_child();
    let original = child.pidfd().as_raw_fd();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.poll_cancel_and_reap() {
            Ok(true) => break,
            Ok(false) => assert_eq!(child.pidfd().as_raw_fd(), original),
            Err(_) => panic!("polling cancellation failed"),
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(child.pidfd.is_none());
    assert!(matches!(child.poll_cancel_and_reap(), Ok(true)));
    assert!(matches!(
        rustix::process::waitpid(Some(child.pid), rustix::process::WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn polling_reaping_ownership_loss_is_sticky() {
    let (mut process, mut child) = ordinary_child();
    process.kill().unwrap();
    process.wait().unwrap();
    for _ in 0..2 {
        assert!(matches!(
            child.poll_cancel_and_reap(),
            Err(ReapErrorV1::OwnershipLost)
        ));
        assert!(child.pidfd.is_none());
    }
    assert!(matches!(
        child.cancel_and_reap(),
        Err(ReapErrorV1::OwnershipLost)
    ));
}

fn static_exit_image() -> File {
    // One RX load segment and an x86-64 exit(0), with no interpreter or runtime startup.
    let code = [0x31, 0xff, 0xb8, 60, 0, 0, 0, 0x0f, 0x05];
    static_image(&code)
}

pub(super) fn static_image(code: &[u8]) -> File {
    let mut bytes = vec![0_u8; 4096 + code.len()];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
    bytes[64..68].copy_from_slice(&1_u32.to_le_bytes());
    bytes[68..72].copy_from_slice(&5_u32.to_le_bytes());
    bytes[80..88].copy_from_slice(&0x400000_u64.to_le_bytes());
    let length = bytes.len() as u64;
    bytes[96..104].copy_from_slice(&length.to_le_bytes());
    bytes[104..112].copy_from_slice(&length.to_le_bytes());
    bytes[112..120].copy_from_slice(&4096_u64.to_le_bytes());
    bytes[4096..].copy_from_slice(code);
    sealed_executable(&bytes)
}

pub(super) fn sealed_executable(bytes: &[u8]) -> File {
    // SAFETY: memfd_create returns a fresh owned descriptor or a negative errno.
    let raw = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            c"spawn-exit-test".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    assert!(raw >= 0);
    // SAFETY: this is the sole owner of the successful memfd result.
    let mut file = unsafe { File::from_raw_fd(raw as i32) };
    file.write_all(bytes).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o555))
        .unwrap();
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::SEAL
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE,
    )
    .unwrap();
    file
}

pub(super) fn deny_calls(calls: &[(c_long, i32)]) {
    let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
    let mut filter = vec![
        instruction(0x20, 0, 0, 4),
        instruction(0x15, 1, 0, 0xc000_003e),
        instruction(0x06, 0, 0, 0x8000_0000),
        instruction(0x20, 0, 0, 0),
    ];
    for &(number, errno) in calls {
        filter.push(instruction(0x15, 0, 1, number as u32));
        filter.push(instruction(0x06, 0, 0, 0x0005_0000 | errno as u32));
    }
    filter.push(instruction(0x06, 0, 0, 0x7fff_0000));
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: irreversible test policy is confined to an explicitly invoked private root helper.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program), 0);
    }
}

fn raw_mask(mask: Option<&u64>) -> u64 {
    let mut previous = 0_u64;
    // SAFETY: exact kernel mask, in one isolated helper thread.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_rt_sigprocmask,
                SIG_SETMASK,
                mask.map_or(std::ptr::null(), |value| value as *const u64),
                &raw mut previous,
                KERNEL_SIGSET_BYTES,
            )
        },
        0
    );
    previous
}

#[test]
#[ignore = "requires private real-root namespace; subreaper checks containment before fail-stop"]
fn root_restore_failure_is_contained() {
    use std::os::unix::process::ExitStatusExt;
    assert!(has_exact_root_identity());
    // SAFETY: this explicitly invoked helper owns its entire private subprocess tree.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0), 0);
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        assert_eq!(libc::setrlimit(libc::RLIMIT_CORE, &raw const limit), 0);
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "syscall::tests::root_atomic_pidfd_launch",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_ROOT_CLONE_CASE", "restore_persistent")
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.signal(), Some(libc::SIGABRT));
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("restoration helper timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        matches!(
            rustix::process::waitpid(None, rustix::process::WaitOptions::NOHANG),
            Err(rustix::io::Errno::CHILD)
        ),
        "fail-stop abandoned its child"
    );
}

#[test]
#[ignore = "requires an isolated real-root namespace; exact spawn, profile and syscall-policy witness"]
fn root_atomic_pidfd_launch() {
    assert!(has_exact_root_identity());
    let case = std::env::var("FE2O3_ROOT_CLONE_CASE").unwrap_or_else(|_| "native".into());
    let image = static_exit_image();
    let (profile_reader, profile_writer) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (gate_reader, gate_writer) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (status_reader, status_writer) =
        rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let source = File::open("/dev/null").unwrap();
    let staged = crate::StagedProtectedServiceExecV1::new(
        &image,
        &[ProtectedServiceDescriptorBindingV1::new(source.as_fd(), 3).unwrap()],
        profile_writer.as_fd(),
        gate_reader.as_fd(),
        status_writer.as_fd(),
    )
    .unwrap();
    let credentials = ProtectedServiceCredentialProfileV1::new(61000, 61000).unwrap();
    let expected = match case.as_str() {
        "native" | "enosys" | "handler" | "drop_retry" => None,
        "eperm" | "legacy_eperm" | "restore_once" | "restore_persistent" => Some(libc::EPERM),
        "einval" => Some(libc::EINVAL),
        "eagain" => Some(libc::EAGAIN),
        _ => panic!("unknown root clone case"),
    };
    let mut calls = vec![(libc::SYS_pidfd_open, libc::EPERM)];
    if case != "native" {
        calls.push((
            libc::SYS_clone3,
            if matches!(
                case.as_str(),
                "enosys"
                    | "handler"
                    | "drop_retry"
                    | "legacy_eperm"
                    | "restore_once"
                    | "restore_persistent"
            ) {
                libc::ENOSYS
            } else {
                expected.unwrap()
            },
        ));
    }
    if matches!(
        case.as_str(),
        "eperm" | "einval" | "eagain" | "legacy_eperm"
    ) {
        calls.push((
            libc::SYS_clone,
            if case == "legacy_eperm" {
                libc::EPERM
            } else {
                libc::EUCLEAN
            },
        ));
    }
    let partial = (1_u64 << (libc::SIGUSR1 - 1)) | (1_u64 << 31);
    let original = raw_mask(Some(&partial));
    deny_calls(&calls);
    let children = std::fs::read_to_string("/proc/thread-self/children").unwrap();
    if case == "handler" {
        signal_mask_tests::prepare_child_probe();
    }
    if case.starts_with("restore_") {
        if case == "restore_persistent" {
            // Piped core collectors ignore RLIMIT_CORE and can delay intentional aborts.
            // SAFETY: only this disposable post-exec helper loses dumpability.
            assert_eq!(unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) }, 0);
        }
        signal_mask_tests::fail_waits(2);
        signal_mask_tests::fail_restorations(if case == "restore_once" {
            1
        } else {
            usize::MAX
        });
    }
    let result = staged.spawn(credentials);
    assert_eq!(raw_mask(None), partial);
    raw_mask(Some(&original));
    if let Some(expected) = expected {
        let error = result.unwrap_err();
        let crate::ProtectedServiceSpawnErrorV1::Io { source, .. } = error else {
            panic!("wrong error: {error:?}")
        };
        assert_eq!(source.raw_os_error(), Some(expected));
        assert_eq!(signal_mask_tests::remaining_wait_failures(), 0);
        assert_eq!(
            std::fs::read_to_string("/proc/thread-self/children").unwrap(),
            children
        );
        return;
    }
    let mut child = result.unwrap();
    let pid = child.pid();
    assert!(
        rustix::io::fcntl_getfd(child.inner.pidfd())
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    drop((staged, profile_writer, gate_reader, status_writer));
    let mut ready = [0_u8];
    assert_eq!(rustix::io::read(&profile_reader, &mut ready).unwrap(), 1);
    assert_eq!(ready[0], PROTECTED_SERVICE_PROFILE_READY_V1);
    fe2o3_protected_service_profile::validate_protected_service_process_v1(credentials, pid)
        .unwrap();
    if case == "drop_retry" {
        signal_mask_tests::fail_waits(2);
        drop(child);
        assert_eq!(signal_mask_tests::remaining_wait_failures(), 0);
        assert!(matches!(
            rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG),
            Err(rustix::io::Errno::CHILD)
        ));
        return;
    }
    assert_eq!(
        rustix::io::write(&gate_writer, &[PROTECTED_SERVICE_GATE_RELEASE_V1]).unwrap(),
        1
    );
    let mut status = [0_u8];
    assert_eq!(rustix::io::read(&status_reader, &mut status).unwrap(), 0);
    let exit = rustix::process::waitid(
        rustix::process::WaitId::PidFd(child.inner.pidfd().as_fd()),
        rustix::process::WaitIdOptions::EXITED,
    )
    .unwrap()
    .unwrap();
    assert_eq!(exit.exit_status(), Some(0));
    child.inner.pidfd.take();
    assert!(matches!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}
