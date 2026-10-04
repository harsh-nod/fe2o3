use super::*;

struct OwnedChild(std::process::Child);

struct OwnedHelperArtifacts(PathBuf);

impl Drop for OwnedHelperArtifacts {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn filtered_atomic_pidfd_launch_cases() {
    for case in [
        "enosys",
        "handler",
        "eperm",
        "einval",
        "eagain",
        "legacy_eperm",
        "restore_once",
        "restore_controller",
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "tests::clone_compat::filtered_launch_helper",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FE2O3_CLONE_TEST_CASE", case);
        let mut child = OwnedChild(
            fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success(), "clone case {case}: {status}");
                break;
            }
            assert!(Instant::now() < deadline, "clone case {case} timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn raw_mask(mask: Option<&u64>) -> u64 {
    let mut previous = 0_u64;
    // SAFETY: exact x86-64 kernel sigset read/write in an isolated helper thread.
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_rt_sigprocmask,
                libc::SIG_SETMASK,
                mask.map_or(std::ptr::null(), |value| value as *const u64),
                &raw mut previous,
                8_usize,
            )
        },
        0
    );
    previous
}

fn deny_calls(calls: &[(libc::c_long, i32)]) {
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
    // SAFETY: the private subprocess permanently installs this bounded test filter.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program), 0);
    }
}

#[test]
#[ignore = "isolated syscall-policy helper; invoked with one exact filter case"]
fn filtered_launch_helper() {
    let case = std::env::var("FE2O3_CLONE_TEST_CASE").unwrap();
    if case == "restore_controller" {
        use std::os::unix::process::ExitStatusExt;
        let fixture = Fixture::new("restore-controller");
        let record = fixture.root.join("compiler-child");
        // SAFETY: only this dedicated subprocess becomes a subreaper; never the test runner.
        unsafe {
            assert_eq!(libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0), 0);
            let limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            assert_eq!(libc::setrlimit(libc::RLIMIT_CORE, &raw const limit), 0);
        }
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::clone_compat::filtered_launch_helper",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("FE2O3_CLONE_TEST_CASE", "restore_persistent")
                .env("FE2O3_CLONE_TEST_COMPILER_PID", &record)
                .spawn()
                .unwrap(),
        );
        let _artifacts = OwnedHelperArtifacts(std::env::temp_dir().join(format!(
            "fe2o3-supervisor-image-clone-compat-{}",
            child.0.id()
        )));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert_eq!(status.signal(), Some(libc::SIGABRT));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        // The compiler fixture is deliberately still live when the supervisor fail-stops.
        // Retire that known original descendant before checking for an abandoned issuer.
        let compiler =
            rustix::process::Pid::from_raw(fs::read_to_string(&record).unwrap().parse().unwrap())
                .unwrap();
        let _ = rustix::process::kill_process(compiler, rustix::process::Signal::KILL);
        rustix::process::waitpid(Some(compiler), rustix::process::WaitOptions::empty()).unwrap();
        assert!(
            matches!(
                rustix::process::waitpid(None, rustix::process::WaitOptions::NOHANG),
                Err(rustix::io::Errno::CHILD)
            ),
            "fail-stop left an unreaped descendant"
        );
        return;
    }
    // Initialize the shared deferred reaper before denying process creation for negative cases.
    clone3_pidfd_launch_admits_exact_readiness_and_reaps_once();
    let fixture = Fixture::with_code("clone-compat", &launched_probe_code(true));
    let supervisor = bound_supervisor(&fixture).expect("non-root launch fixture");
    let (_reserved, rustc_child, _control, accepted) = accepted_handoff(&supervisor);
    if case == "restore_persistent" {
        fs::write(
            std::env::var_os("FE2O3_CLONE_TEST_COMPILER_PID").unwrap(),
            rustc_child.id().to_string(),
        )
        .unwrap();
    }
    let _rustc_child = OwnedChild(rustc_child);
    let prepared = supervisor.prepare_launch_inner::<false>(accepted).unwrap();
    let clone3_error = match case.as_str() {
        "enosys" | "handler" | "legacy_eperm" | "restore_once" | "restore_persistent" => {
            libc::ENOSYS
        }
        "eperm" => libc::EPERM,
        "einval" => libc::EINVAL,
        "eagain" => libc::EAGAIN,
        _ => panic!("unknown clone case"),
    };
    let mut calls = vec![
        (libc::SYS_clone3, clone3_error),
        (libc::SYS_pidfd_open, libc::EPERM),
    ];
    if !matches!(
        case.as_str(),
        "enosys" | "handler" | "restore_once" | "restore_persistent"
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
    let children = fs::read_to_string("/proc/thread-self/children").unwrap();
    if case == "handler" {
        crate::process::signal_mask_tests::prepare_child_probe();
    }
    if case.starts_with("restore_") {
        if case == "restore_persistent" {
            // Piped core collectors ignore RLIMIT_CORE and can delay intentional aborts.
            // SAFETY: only this disposable post-exec helper loses dumpability.
            assert_eq!(unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) }, 0);
        }
        crate::process::signal_mask_tests::fail_waits(2);
        crate::process::signal_mask_tests::fail_restorations(if case == "restore_once" {
            1
        } else {
            usize::MAX
        });
    }
    let result = supervisor.launch_inner::<false>(prepared, Duration::from_secs(2));
    assert_eq!(raw_mask(None), partial);
    raw_mask(Some(&original));
    if case == "enosys" || case == "handler" {
        let launched = result.unwrap();
        let pid = rustix::process::Pid::from_raw(launched.pid() as i32).unwrap();
        read_exact_nonblocking(launched.stdout_reader_for_test(), b"LAUNCHED\n");
        launched.cancel().unwrap();
        assert_reaped(pid);
    } else {
        let error = result.unwrap_err();
        let ProtectedIssuerLaunchErrorV1::Io { source, .. } = error else {
            panic!("wrong error: {error:?}")
        };
        let expected = if case == "legacy_eperm" || case == "restore_once" {
            libc::EPERM
        } else {
            clone3_error
        };
        assert_eq!(source.raw_os_error(), Some(expected));
        assert_eq!(
            crate::process::signal_mask_tests::remaining_wait_failures(),
            0
        );
        assert_eq!(
            fs::read_to_string("/proc/thread-self/children").unwrap(),
            children
        );
    }
}
