//! Actual scalar syscall restrictions, not approved compiler/runtime evidence.
//! The static C diagnostic now also needs -pthread. Run isolated and serially.
//! Inherited personality refusal precedes exec. Dynamic loading and personality
//! established by static or dynamic exec are outside this diagnostic's claim.
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
    NamespaceService,
    NamespaceInstallDenied,
    NamespaceWorkShort,
    NamespaceStorageShort,
    WorkShort,
    Unwind,
    InstallDenied,
    QueryDenied,
    InheritedReadImpliesExec,
}

const READ_IMPLIES_EXEC: libc::c_long = 0x0040_0000;

fn current_personality() -> libc::c_long {
    // SAFETY: the query sentinel reads only the calling test thread's state.
    unsafe { libc::syscall(libc::SYS_personality, u32::MAX as libc::c_ulong) }
}

struct CreatorPersonality(libc::c_ulong);
impl CreatorPersonality {
    fn set_read_implies_exec() -> Self {
        let original = current_personality();
        assert!(original >= 0, "requires an allowed personality query");
        assert_eq!(
            original & READ_IMPLIES_EXEC,
            0,
            "requires clean creator state"
        );
        // Arm restoration before any mutation or fallible readback. Setting the
        // bit before re-executing this test harness would not test inheritance.
        let restore = Self(original as libc::c_ulong);
        let expected = original | READ_IMPLIES_EXEC;
        // SAFETY: isolated test thread only; the live guard restores the exact
        // original state after clone and on every error/unwind path.
        let previous = unsafe { libc::syscall(libc::SYS_personality, expected as libc::c_ulong) };
        assert_eq!(previous, original, "requires an allowed personality setter");
        assert_eq!(current_personality(), expected);
        restore
    }
}
impl Drop for CreatorPersonality {
    fn drop(&mut self) {
        if current_personality() == self.0 as libc::c_long {
            return;
        }
        // SAFETY: same isolated clone-originating thread and exact saved state.
        let previous = unsafe { libc::syscall(libc::SYS_personality, self.0) };
        assert!(previous >= 0, "creator personality restoration failed");
        assert_eq!(current_personality(), self.0 as libc::c_long);
    }
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
#[ignore = "requires isolated root/outside custodian, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn every_native_child_confines_namespaces_and_preserves_thread_fork_exec() {
    let mut pool = pool();
    run(&mut pool.0, Mode::NamespaceWorkShort);
    run(&mut pool.0, Mode::NamespaceStorageShort);
    run(&mut pool.0, Mode::NamespaceService);
    run(&mut pool.0, Mode::Execute("namespace"));
    // Stacking the shared namespace floor must not relax compiler memory denial.
    run(&mut pool.0, Mode::Execute("anon-rx"));
    // A later creator clone still succeeds; the filter did not reach the parent.
    run(&mut pool.0, Mode::NamespaceService);
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
    let service = matches!(
        mode,
        Mode::NamespaceService
            | Mode::NamespaceInstallDenied
            | Mode::NamespaceWorkShort
            | Mode::NamespaceStorageShort
    );
    let namespace = service || matches!(mode, Mode::Execute("namespace"));
    if namespace {
        // SAFETY: zero unshare flags and invalid setns/clone3 arguments neither
        // create tasks nor change namespaces. Refuse a host filter that could
        // supply the expected child errno before our filter ever installed.
        unsafe {
            assert_eq!(libc::syscall(libc::SYS_unshare, 0), 0);
            assert_eq!(libc::syscall(libc::SYS_setns, -1, 0), -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EBADF)
            );
            assert_eq!(libc::syscall(libc::SYS_clone3, 0, 0), -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EINVAL)
            );
        }
    }
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
        Mode::NamespaceService
        | Mode::NamespaceInstallDenied
        | Mode::NamespaceWorkShort
        | Mode::NamespaceStorageShort => "namespace",
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
        if service {
            Stage::stage(
                &image,
                &[Binding::new(output.as_fd(), 198).unwrap()],
                ready_writer.as_fd(),
                gate.as_fd(),
                status_writer.as_fd(),
                source,
                &mut b,
            )
        } else {
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
    }
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    if matches!(mode, Mode::NamespaceStorageShort) {
        // Exactly one byte short of the shared pre-clone ABI/filter frame.
        b.reserve_storage(LIMIT - b.storage() - (Stage::SPAWN_SCRATCH - 1))
            .unwrap();
    }
    let original_storage = b.storage();
    let ceiling = observations::read_cap_last_cap().unwrap();
    let exact = stage
        .spawn_retaining_work::<Backing>(ceiling, source)
        .unwrap();
    if matches!(mode, Mode::WorkShort | Mode::NamespaceWorkShort) {
        b.charge_work(LIMIT - b.work() - (exact - 1)).unwrap();
    }
    let before = b.work();
    let backing = Backing {
        _image: image,
        _cwd: cwd,
        _terminal_witness: writer,
    };
    let credentials = Credentials::new(65534, 65534).unwrap();
    let personality = matches!(mode, Mode::InheritedReadImpliesExec)
        .then(CreatorPersonality::set_read_implies_exec);
    // SAFETY: complete actual sources enter the original independently funded
    // slot before clone. Only this test thread consumes waits; its pool drains
    // all error/unwind paths before fixture inputs or the creator are retired.
    let spawned = unsafe { stage.spawn_retaining(credentials, backing, source, pool, &mut b) };
    drop(personality);
    if matches!(
        mode,
        Mode::WorkShort | Mode::NamespaceWorkShort | Mode::NamespaceStorageShort
    ) {
        if matches!(mode, Mode::NamespaceStorageShort) {
            assert!(matches!(
                spawned,
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            assert!(b.failed_work().is_none());
        } else {
            assert!(matches!(
                spawned,
                Err(Error::Resource(Resource::Work(_)))
                    | Err(Error::Cleanup(
                        crate::ProtectedServiceCleanupErrorV2::Resource(Resource::Work(_))
                    ))
            ));
            assert!(b.failed_work().is_some());
            assert!(b.failed_storage().is_none());
        }
        let denied = (b.failed_work(), b.failed_storage());
        assert_eq!(b.storage(), original_storage);
        assert_eq!(pool.report().unwrap().storage, baseline.storage);
        assert_eq!(rustix::io::read(&witness, &mut [0]).unwrap(), 0);
        assert_eq!(output.metadata().unwrap().len(), 0);
        assert_eq!((b.failed_work(), b.failed_storage()), denied);
        assert_eq!(pool.report().unwrap().failed_work, baseline.failed_work);
        assert_eq!(pool.report().unwrap().work_limit, baseline.work_limit);
        drop(stage);
        drop(ready_writer);
        drop(status_writer);
        assert_eq!(rustix::io::read(&ready, &mut [0]).unwrap(), 0);
        assert_eq!(rustix::io::read(&status, &mut [0]).unwrap(), 0);
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
    if matches!(
        mode,
        Mode::InstallDenied
            | Mode::NamespaceInstallDenied
            | Mode::QueryDenied
            | Mode::InheritedReadImpliesExec
    ) {
        while child.is_live(&mut b).unwrap() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(rustix::io::read(&ready, &mut [0]).unwrap(), 0);
        let mut byte = [0];
        assert_eq!(rustix::io::read(&status, &mut byte).unwrap(), 1);
        assert_eq!(
            byte,
            [if matches!(mode, Mode::NamespaceInstallDenied) {
                0xce
            } else {
                0xcd
            }]
        );
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
            if matches!(name, "ordinary" | "fork" | "fork-exec" | "namespace") {
                assert_eq!(terminal.exit_code(), Some(7), "{name}");
            } else {
                assert_eq!(terminal.terminating_signal(), Some(libc::SIGSYS), "{name}");
            }
            assert_eq!(trace.poll(&mut b).unwrap(), terminal);
            assert_eq!(rustix::io::read(&witness, &mut [0]), Err(Errno::AGAIN));
            let _ = trace.cancel();
            drop(trace);
            let expected: &[u8] = if namespace {
                b"namespace probe\nnamespace thread fork exec complete\n"
            } else {
                b"restriction probe\n"
            };
            let mut marker = vec![0; expected.len()];
            assert_eq!(
                rustix::io::pread(&output, &mut marker, 0).unwrap(),
                marker.len()
            );
            assert_eq!(marker.as_slice(), expected);
            if namespace {
                assert_eq!(output.metadata().unwrap().len(), expected.len() as u64);
            }
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
    if namespace {
        assert!(b.failed_work().is_none());
        assert!(b.failed_storage().is_none());
    }
}

#[test]
#[ignore = "requires isolated root, clone3, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_filter_install_failure_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_filter_install_failure_is_terminal_without_ready_or_exec";
    isolated_refusal(TEST, Mode::InstallDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, clone3, seccomp and static -pthread native_compiler_exec.c"]
fn namespace_filter_install_failure_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::namespace_filter_install_failure_is_terminal_without_ready_or_exec";
    isolated_refusal(TEST, Mode::NamespaceInstallDenied);
}

#[test]
#[ignore = "requires isolated root, clone3, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_personality_query_denial_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_personality_query_denial_is_terminal_without_ready_or_exec";
    isolated_refusal(TEST, Mode::QueryDenied);
}

#[test]
#[ignore = "requires isolated root, personality setter/query, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn compiler_inherited_read_implies_exec_is_rejected_and_creator_restored() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_inherited_read_implies_exec_is_rejected_and_creator_restored";
    isolated_refusal(TEST, Mode::InheritedReadImpliesExec);
}

fn isolated_refusal(test: &str, mode: Mode<'_>) {
    const SENTINEL: &str = "FE2O3_COMPILER_RESTRICTION_REFUSAL_CHILD";
    if std::env::var_os(SENTINEL).is_some() {
        let mut pool = pool();
        if matches!(
            mode,
            Mode::InstallDenied | Mode::NamespaceInstallDenied | Mode::QueryDenied
        ) {
            install_denial(mode);
        }
        if matches!(mode, Mode::InstallDenied) {
            let observed = current_personality();
            assert!(
                observed >= 0,
                "installation refusal requires an allowed query"
            );
            assert_eq!(observed & READ_IMPLIES_EXEC, 0);
        }
        if matches!(mode, Mode::QueryDenied) {
            let observed = current_personality();
            let error = std::io::Error::last_os_error();
            assert_eq!(observed, -1);
            assert_eq!(error.raw_os_error(), Some(libc::EPERM));
        }
        run(&mut pool.0, mode);
        if matches!(mode, Mode::InheritedReadImpliesExec) {
            let original = current_personality();
            let unwind = catch_unwind(|| {
                let _restore = CreatorPersonality::set_read_implies_exec();
                panic!("creator personality unwind");
            });
            let panic = unwind.err().expect("expected the explicit creator unwind");
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&"creator personality unwind")
            );
            assert_eq!(current_personality(), original);
            // Separate funded request on the same original pool, not a retry or
            // account reset. The creator no longer passes a poisoned personality.
            run(&mut pool.0, Mode::Execute("ordinary"));
        }
        return;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            test,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SENTINEL, "1");
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "isolated restriction refusal test: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "exact child test did not execute"
    );
}

fn install_denial(mode: Mode<'_>) {
    let (nr, argument) = match mode {
        Mode::InstallDenied | Mode::NamespaceInstallDenied => (157, 22), // prctl(PR_SET_SECCOMP)
        Mode::QueryDenied => (135, u32::MAX), // personality query sentinel
        _ => panic!("invalid denial fixture mode"),
    };
    // SAFETY: this is the isolated clone-originating test thread after harness
    // startup. The kernel copies fixed stack BPF synchronously. Its EPERM denial
    // is inherited by the actual service/compiler child, not a test hook.
    // The surrounding exact-test subprocess owns this irreversible restriction.
    unsafe {
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
                k: nr,
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
                k: argument,
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
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(
            libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0),
            0
        );
    }
}
