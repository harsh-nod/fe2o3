//! Actual scalar syscall restrictions, not approved compiler/runtime evidence.
//! The static C diagnostic now also needs -pthread. Run isolated and serially.
use super::*;
use crate::native_spawn::{RootRetainedTaskTraceV2, RootTaskTraceEventV2};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

struct Backing {
    _image: File,
    _cwd: File,
    _terminal_witness: OwnedFd,
}

#[derive(Clone, Copy)]
enum Mode<'a> {
    Execute(&'a str),
    WorkShort,
    Unwind,
    InstallDenied,
}

fn pool() -> Drain {
    Drain(
        Cleanup::admit(Account::new(
            Work::new(
                LIMIT
                    + CLEANUP_TURNS
                        * Cleanup::pump_work(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2).unwrap(),
            ),
            LIMIT,
        ))
        .unwrap(),
    )
}

#[test]
#[ignore = "requires isolated root, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn compiler_restrictions_execute_real_mapping_controls_and_denials() {
    let mut pool = pool();
    for mode in [
        "ordinary",
        "anon-rx",
        "anon-rwx",
        "mprotect",
        "pkey-mprotect",
        "file-rwx",
        "restore-exec",
        "personality",
        "userfaultfd",
        "io-uring",
        "disable",
    ] {
        run(&mut pool.0, Mode::Execute(mode));
    }
}

#[test]
#[ignore = "requires isolated root, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn compiler_restrictions_survive_thread_fork_and_exec() {
    let mut pool = pool();
    for mode in ["thread", "fork", "fork-exec"] {
        run(&mut pool.0, Mode::Execute(mode));
    }
}

#[test]
#[ignore = "requires isolated root, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn compiler_restrictions_prepay_before_clone_and_keep_unwind_custody() {
    let mut pool = pool();
    run(&mut pool.0, Mode::WorkShort);
    run(&mut pool.0, Mode::Unwind);
    // A refused/exhausted request never resets the independent pool account or
    // poisons later funded requests. This is a distinct request, not a retry.
    run(&mut pool.0, Mode::Execute("ordinary"));
}

fn next_retained(
    trace: &mut RootRetainedTaskTraceV2<'_, Backing>,
    b: &mut Budget<'_>,
    deadline: Instant,
) -> RootTaskTraceEventV2 {
    loop {
        let event = trace.poll(b).unwrap();
        if !event.is_pending() {
            return event;
        }
        assert!(Instant::now() < deadline, "restriction trace deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn run(pool: &mut Cleanup, mode: Mode<'_>) {
    assert!(syscall::has_exact_root_identity());
    let baseline = pool.report().unwrap();
    let image = File::open(
        std::env::var_os("FE2O3_NATIVE_COMPILER_EXEC_FIXTURE").expect("static -pthread fixture"),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let cwd = File::open(directory.path()).unwrap();
    rustix::fs::fchown(
        &cwd,
        Some(rustix::process::Uid::from_raw(65534)),
        Some(rustix::process::Gid::from_raw(65534)),
    )
    .unwrap();
    let output = tempfile::tempfile().unwrap();
    let (witness, writer) = pipe();
    rustix::fs::fcntl_setfl(&witness, rustix::fs::OFlags::NONBLOCK).unwrap();
    let (ready, ready_writer) = pipe();
    rustix::fs::fcntl_setfl(&ready, rustix::fs::OFlags::NONBLOCK).unwrap();
    let (gate, gate_writer) = pipe();
    let (status, status_writer) = rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC | rustix::net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    let name = match mode {
        Mode::Execute(name) => name,
        _ => "ordinary",
    };
    let arguments = [
        CString::new("compiler-restriction").unwrap(),
        CString::new(name).unwrap(),
    ];
    // Whole fixture input backing, source owner headers and fixed path/IO frames.
    // These are inert test files, never a RetainedCompilerRuntime substitute.
    let source =
        usize::try_from(image.metadata().unwrap().len()).unwrap() + size_of::<Backing>() + 32768;
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(source).unwrap();
    // SAFETY: this isolated root test owns these actual sources, exact bytes,
    // channels and cleanup controller. No protected compiler admission is made.
    let (stage, charge) = unsafe {
        Stage::stage_compiler(
            &image,
            &arguments,
            &[],
            cwd.as_fd(),
            [None, Some(output.as_fd()), None],
            &[],
            ready_writer.as_fd(),
            gate.as_fd(),
            status_writer.as_fd(),
            source,
            &mut b,
        )
    }
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let original_storage = b.storage();
    let ceiling = observations::read_cap_last_cap().unwrap();
    let exact = stage
        .spawn_retaining_work::<Backing>(ceiling, source)
        .unwrap();
    if matches!(mode, Mode::WorkShort) {
        b.charge_work(LIMIT - b.work() - (exact - 1)).unwrap();
    }
    let before = b.work();
    let backing = Backing {
        _image: image,
        _cwd: cwd,
        _terminal_witness: writer,
    };
    let credentials = Credentials::new(65534, 65534).unwrap();
    // SAFETY: complete actual sources enter the original independently funded
    // slot before clone. Only this test thread consumes waits; its pool drains
    // all error/unwind paths before fixture inputs or the creator are retired.
    let spawned = unsafe { stage.spawn_retaining(credentials, backing, source, pool, &mut b) };
    if matches!(mode, Mode::WorkShort) {
        assert!(matches!(
            spawned,
            Err(Error::Resource(Resource::Work(_)))
                | Err(Error::Cleanup(
                    crate::ProtectedServiceCleanupErrorV2::Resource(Resource::Work(_))
                ))
        ));
        let denied = b.failed_work().expect("first work denial");
        assert_eq!(b.storage(), original_storage);
        assert_eq!(pool.report().unwrap().storage, baseline.storage);
        assert_eq!(rustix::io::read(&witness, &mut [0]).unwrap(), 0);
        assert_eq!(output.metadata().unwrap().len(), 0);
        assert_eq!(b.failed_work(), Some(denied));
        return;
    }
    let (mut child, charge) = spawned.unwrap();
    assert_eq!(b.work() - before, exact);
    b.reserve_storage(charge.additional_storage()).unwrap();
    let after_spawn = b.storage();
    drop(ready_writer);
    drop(status_writer);
    // Stage still owns high-FD writer duplicates, so retire it before EOF checks.
    drop(stage);
    drop(gate);
    let deadline = Instant::now() + Duration::from_secs(10);
    if matches!(mode, Mode::InstallDenied) {
        while child.is_live(&mut b).unwrap() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(rustix::io::read(&ready, &mut [0]).unwrap(), 0);
        let mut byte = [0];
        assert_eq!(rustix::io::read(&status, &mut byte).unwrap(), 1);
        assert_eq!(byte, [0xcd]);
        assert_eq!(output.metadata().unwrap().len(), 0);
        let _ = child.cancel();
        drop(child);
    } else {
        loop {
            let mut byte = [0];
            match rustix::io::read(&ready, &mut byte) {
                Ok(1) => {
                    assert_eq!(byte, [READY]);
                    break;
                }
                Err(Errno::AGAIN | Errno::INTR) => {
                    assert!(
                        Instant::now() < deadline,
                        "restriction profile-ready deadline"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                other => panic!("restriction profile-ready: {other:?}"),
            }
        }
        assert_eq!(rustix::io::read(&witness, &mut [0]), Err(Errno::AGAIN));
        observations::validate_process(credentials, child.pid()).unwrap();
        b.reserve_storage(Child::ROOT_TRACE_GROWTH).unwrap();
        let mut trace = child.into_root_trace(&mut b).unwrap();
        if matches!(mode, Mode::Unwind) {
            // Exhaust only the original request. Original pool funding still
            // owns cancellation, exact wait and backing destruction.
            assert!(b.charge_work(LIMIT).is_err());
            let denial = b.failed_work();
            let retained = b.storage();
            assert!(
                catch_unwind(AssertUnwindSafe(move || {
                    let _trace = trace;
                    panic!("restricted gated compiler unwind");
                }))
                .is_err()
            );
            assert_eq!(b.failed_work(), denial);
            assert_eq!(b.storage(), retained);
            assert_eq!(output.metadata().unwrap().len(), 0);
        } else {
            assert_eq!(rustix::io::write(&gate_writer, &[GO]).unwrap(), 1);
            assert!(next_retained(&mut trace, &mut b, deadline).is_exec());
            assert_eq!(rustix::io::read(&status, &mut [0]).unwrap(), 0);
            trace.resume(&mut b).unwrap();
            let terminal = loop {
                let event = next_retained(&mut trace, &mut b, deadline);
                if event.is_terminal() {
                    break event;
                }
                // Fork diagnostics wait their own child, so SIGCHLD delivery is
                // legitimate. Preserve it via the original trace's resume API.
                assert!(matches!(
                    event.signal_stop(),
                    Some(libc::SIGCHLD | libc::SIGSYS)
                ));
                trace.resume(&mut b).unwrap();
            };
            if matches!(name, "ordinary" | "fork" | "fork-exec") {
                assert_eq!(terminal.exit_code(), Some(7), "{name}");
            } else {
                assert_eq!(terminal.terminating_signal(), Some(libc::SIGSYS), "{name}");
            }
            assert_eq!(trace.poll(&mut b).unwrap(), terminal);
            assert_eq!(rustix::io::read(&witness, &mut [0]), Err(Errno::AGAIN));
            let _ = trace.cancel();
            drop(trace);
            let mut marker = [0; b"restriction probe\n".len()];
            assert_eq!(
                rustix::io::pread(&output, &mut marker, 0).unwrap(),
                marker.len()
            );
            assert_eq!(&marker, b"restriction probe\n");
        }
    }
    // No request quota is released while any source may remain in the pool.
    assert!(b.storage() >= after_spawn);
    loop {
        match rustix::io::read(&witness, &mut [0]) {
            Ok(0) => break,
            Err(Errno::AGAIN) => {
                assert!(pool.report().unwrap().storage > baseline.storage);
                assert!(
                    Instant::now() < deadline,
                    "restriction backing cleanup deadline"
                );
                pool.pump(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2)
                    .unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
            other => panic!("unexpected terminal witness: {other:?}"),
        }
    }
    assert_eq!(pool.report().unwrap().storage, baseline.storage);
    assert_eq!(pool.report().unwrap().work_limit, baseline.work_limit);
    assert_eq!(pool.report().unwrap().failed_work, baseline.failed_work);
}

#[test]
#[ignore = "requires isolated root, clone3, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_filter_install_failure_is_terminal_without_ready_or_exec() {
    use std::os::unix::process::CommandExt;
    const SENTINEL: &str = "FE2O3_COMPILER_FILTER_DENIED_CHILD";
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_filter_install_failure_is_terminal_without_ready_or_exec";
    if std::env::var_os(SENTINEL).is_some() {
        let mut pool = pool();
        run(&mut pool.0, Mode::InstallDenied);
        return;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SENTINEL, "1");
    // SAFETY: isolated exec child only. Fixed stack BPF rejects SET_SECCOMP with
    // EPERM, causing the actual compiler install syscall (not a test hook) to
    // fail. Parent test process and other service roles remain unchanged.
    unsafe {
        command.pre_exec(|| {
            let filter = [
                libc::sock_filter {
                    code: 0x20,
                    jt: 0,
                    jf: 0,
                    k: 0,
                },
                libc::sock_filter {
                    code: 0x15,
                    jt: 0,
                    jf: 3,
                    k: 157,
                },
                libc::sock_filter {
                    code: 0x20,
                    jt: 0,
                    jf: 0,
                    k: 16,
                },
                libc::sock_filter {
                    code: 0x15,
                    jt: 0,
                    jf: 1,
                    k: 22,
                },
                libc::sock_filter {
                    code: 0x06,
                    jt: 0,
                    jf: 0,
                    k: 0x0005_0001,
                },
                libc::sock_filter {
                    code: 0x06,
                    jt: 0,
                    jf: 0,
                    k: 0x7fff_0000,
                },
            ];
            let program = libc::sock_fprog {
                len: filter.len() as u16,
                filter: filter.as_ptr().cast_mut(),
            };
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                || libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "isolated install failure test: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "exact child test did not execute"
    );
}
