use super::*;
use std::{
    os::fd::IntoRawFd,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MARKER: &str = "FE2O3_PRIVATE_PROFILE_TRANSITION_TEST";

#[test]
fn seccomp_enosys_never_falls_back_for_cgroup_or_user_namespace() {
    crate::clone_compat::tests::run_named(
        "syscall::profile_tests::strict_clone3_subprocess",
        "strict-clone3",
    );
}

#[test]
fn strict_clone3_subprocess() {
    if std::env::var(crate::clone_compat::tests::CASE).as_deref() != Ok("strict-clone3") {
        return;
    }
    let dummy = File::open("/dev/null").unwrap();
    let staged =
        StagedProtectedServiceExecV1::new(&dummy, &[], dummy.as_fd(), dummy.as_fd(), dummy.as_fd())
            .unwrap();
    let credentials = ProtectedServiceCredentialProfileV1::new(1000, 1000).unwrap();
    // Different errors distinguish strict clone3 refusal from any legacy retry.
    crate::clone_compat::tests::install_filter(libc::ENOSYS, true, false);
    for mapping in [None, Some((dummy.as_fd(), dummy.as_fd()))] {
        let result = clone_child_with_cgroup(
            &staged,
            credentials,
            40,
            rustix::process::getpid(),
            Some(dummy.as_fd()),
            mapping,
        );
        assert!(matches!(result, Err(rustix::io::Errno::NOSYS)));
    }
    assert!(matches!(
        rustix::process::waitpid(None, rustix::process::WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn fresh_domain_clone_preserves_atomic_pidfd_and_signal_contract() {
    let domain = File::open("/dev/null").unwrap();
    let mut output = -1;
    for (fd, userns) in [
        (None, false),
        (Some(domain.as_fd()), false),
        (Some(domain.as_fd()), true),
    ] {
        let args = clone_arguments(&mut output, fd, userns);
        assert_eq!(
            args.flags,
            CLONE_PIDFD
                | CLONE_CLEAR_SIGHAND
                | if fd.is_some() { CLONE_INTO_CGROUP } else { 0 }
                | if userns { CLONE_NEWUSER } else { 0 }
        );
        assert_eq!(args.pidfd, (&raw mut output).addr() as u64);
        assert_eq!(args.cgroup, fd.map_or(0, |fd| fd.as_raw_fd() as u64));
        assert_eq!(args.exit_signal, SIGCHLD);
        assert_eq!(
            [
                args.child_tid,
                args.parent_tid,
                args.stack,
                args.stack_size,
                args.tls,
                args.set_tid,
                args.set_tid_size
            ],
            [0; 7]
        );
    }
}

#[test]
fn profile_guard_survives_a_transition_that_clears_parent_death() {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "syscall::profile_tests::profile_subprocess",
            "--nocapture",
        ])
        .env_clear()
        .env(MARKER, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("profile transition subprocess exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn profile_subprocess() {
    if std::env::var_os(MARKER).is_none() {
        return;
    }
    let parent = rustix::process::getppid().unwrap().as_raw_pid();
    let mut called = false;
    // SAFETY: only this isolated subprocess changes its own parent-death setting.
    let result = unsafe {
        establish_guarded_profile(parent, || {
            called = true;
            assert_eq!(parent_death_signal(), SIGKILL);
            // Model the kernel's credential-transition effect without requiring root.
            assert_eq!(libc::prctl(PR_SET_PDEATHSIG, 0, 0, 0, 0), 0);
            assert_eq!(parent_death_signal(), 0);
            0
        })
    };
    assert_eq!(result, Ok(()));
    assert!(called);
    assert_eq!(parent_death_signal(), SIGKILL);

    // A wrong parent cannot run the profile transition or reach readiness.
    called = false;
    let result = unsafe {
        establish_guarded_profile(rustix::process::getpid().as_raw_pid(), || {
            called = true;
            0
        })
    };
    assert_eq!(result, Err(2));
    assert!(!called);

    // A failed transition preserves its failure category and cannot publish readiness.
    let result = unsafe {
        establish_guarded_profile(parent, || {
            assert_eq!(libc::prctl(PR_SET_PDEATHSIG, 0, 0, 0, 0), 0);
            -1
        })
    };
    assert_eq!(result, Err(3));
    assert_eq!(parent_death_signal(), 0);

    for payload in [Some(PROTECTED_SERVICE_GATE_RELEASE_V1), Some(0), None] {
        let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        if let Some(byte) = payload {
            assert_eq!(rustix::io::write(&writer, &[byte]).unwrap(), 1);
        }
        let (reader, writer) = (reader.into_raw_fd(), writer.into_raw_fd());
        let mut installed = false;
        // SAFETY: this deadline-bounded subprocess transfers both private pipe
        // ends to the production mapping gate. Failure leaves only the reader.
        let result = unsafe {
            let result = establish_guarded_profile(parent, || {
                if await_mapping_gate(reader, writer) != 0 {
                    return -1;
                }
                installed = true;
                0
            });
            if result.is_err() {
                libc::syscall(libc::SYS_close, reader);
            }
            result
        };
        let accepted = payload == Some(PROTECTED_SERVICE_GATE_RELEASE_V1);
        assert_eq!(result, if accepted { Ok(()) } else { Err(3) });
        assert_eq!(installed, accepted);
    }
}

fn parent_death_signal() -> c_int {
    let mut signal = -1;
    // SAFETY: syscall writes one local scalar in the isolated subprocess.
    assert_eq!(
        unsafe { libc::prctl(PR_GET_PDEATHSIG, &raw mut signal, 0, 0, 0) },
        0
    );
    signal
}
