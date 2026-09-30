//! Actual scalar syscall restrictions, not approved compiler/runtime evidence.
//! The static C diagnostic now also needs -pthread. Run isolated and serially.
//! Inherited personality refusal precedes exec. Dynamic loading and personality
//! established by static or dynamic exec are outside this diagnostic's claim.
use super::*;
use crate::native_spawn::{RootRetainedTaskTraceV2, RootTaskTraceEventV2};
use std::{
    mem::size_of,
    os::unix::fs::MetadataExt,
    panic::{AssertUnwindSafe, catch_unwind},
};

struct Backing {
    _image: File,
    _cwd: File,
    _selector: File,
    _terminal_witness: OwnedFd,
}

#[derive(Clone, Copy)]
enum Mode<'a> {
    Execute(&'a str),
    UnmappedCreator,
    MappedService,
    NamespaceInstallDenied,
    NamespaceWorkShort,
    NamespaceStorageShort,
    WorkShort,
    Unwind,
    InstallDenied,
    ObservationReadDenied,
    MappedObservationReadDenied,
    ObservationEofDenied,
    MappedObservationEofDenied,
    ObservationStorageShort,
    LockPreserved,
    MappedLockPreserved,
    DirtyLockPreserved,
    MappedDirtyLockPreserved,
    MappedAcquisitionDenied,
    MappedProfileAbort,
    InheritedReadImpliesExec,
}

impl Mode<'_> {
    fn mapped(self) -> bool {
        matches!(
            self,
            Self::MappedService
                | Self::MappedLockPreserved
                | Self::MappedObservationReadDenied
                | Self::MappedObservationEofDenied
                | Self::MappedDirtyLockPreserved
                | Self::MappedAcquisitionDenied
                | Self::MappedProfileAbort
                | Self::NamespaceInstallDenied
                | Self::NamespaceWorkShort
                | Self::NamespaceStorageShort
        )
    }

    fn service(self) -> bool {
        matches!(
            self,
            Self::UnmappedCreator
                | Self::MappedService
                | Self::NamespaceInstallDenied
                | Self::NamespaceWorkShort
                | Self::NamespaceStorageShort
        )
    }

    fn read_denied(self) -> bool {
        matches!(
            self,
            Self::ObservationReadDenied | Self::MappedObservationReadDenied
        )
    }

    fn eof_denied(self) -> bool {
        matches!(
            self,
            Self::ObservationEofDenied | Self::MappedObservationEofDenied
        )
    }

    fn dirty_locked(self) -> bool {
        matches!(
            self,
            Self::DirtyLockPreserved | Self::MappedDirtyLockPreserved
        )
    }

    fn early_fault(self) -> bool {
        matches!(
            self,
            Self::MappedAcquisitionDenied | Self::MappedProfileAbort
        )
    }

    fn failure_stage(self) -> Option<u8> {
        match self {
            Self::MappedProfileAbort => Some(3),
            Self::InstallDenied => Some(13),
            Self::NamespaceInstallDenied => Some(14),
            Self::MappedAcquisitionDenied => Some(15),
            Self::ObservationReadDenied
            | Self::MappedObservationReadDenied
            | Self::ObservationEofDenied
            | Self::MappedObservationEofDenied
            | Self::DirtyLockPreserved
            | Self::MappedDirtyLockPreserved
            | Self::InheritedReadImpliesExec => Some(16),
            _ => None,
        }
    }
}

#[test]
fn mapped_personality_controls_select_compiler_staging_and_exact_failure_stages() {
    for mode in [
        Mode::MappedObservationReadDenied,
        Mode::MappedObservationEofDenied,
        Mode::MappedDirtyLockPreserved,
        Mode::MappedAcquisitionDenied,
        Mode::MappedProfileAbort,
    ] {
        assert!(mode.mapped());
        assert!(!mode.service());
        let stage = if matches!(mode, Mode::MappedAcquisitionDenied) {
            15
        } else if matches!(mode, Mode::MappedProfileAbort) {
            3
        } else {
            16
        };
        assert_eq!(mode.failure_stage(), Some(stage));
    }
    assert!(Mode::MappedLockPreserved.mapped());
    assert!(!Mode::MappedLockPreserved.service());
    assert_eq!(Mode::MappedLockPreserved.failure_stage(), None);
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
fn unmapped_service_preserves_nested_clone3_without_qualifying_a_helper() {
    let mut pool = pool();
    run(&mut pool.0, Mode::UnmappedCreator);
    run(&mut pool.0, Mode::Execute("namespace"));
    // Neither the compiler's filter nor its retirement may affect the creator.
    run(&mut pool.0, Mode::UnmappedCreator);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn mapped_service_and_compiler_confine_namespaces_and_preserve_thread_fork_exec() {
    let mut pool = pool();
    run(&mut pool.0, Mode::NamespaceWorkShort);
    run(&mut pool.0, Mode::NamespaceStorageShort);
    run(&mut pool.0, Mode::MappedService);
    run(&mut pool.0, Mode::Execute("namespace"));
    // Stacking the shared namespace floor must not relax compiler memory denial.
    run(&mut pool.0, Mode::Execute("anon-rx"));
    // A later creator clone still succeeds; the filter did not reach the parent.
    run(&mut pool.0, Mode::MappedService);
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
    let mapped = mode.mapped();
    let creator = matches!(mode, Mode::UnmappedCreator);
    let service = mode.service();
    let namespace = (mapped && service) || matches!(mode, Mode::Execute("namespace"));
    if namespace || creator {
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
    let selector = tempfile::tempfile().unwrap();
    // Fixture behavior only: this byte is never consulted by staging or spawn.
    assert_eq!(
        rustix::io::pwrite(&selector, if creator { b"C" } else { b"N" }, 0).unwrap(),
        1
    );
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
        Mode::UnmappedCreator => "creator",
        Mode::MappedService
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
    let ledger = b.work_ledger_identity_v1();
    // SAFETY: this isolated root test owns these actual sources, exact bytes,
    // channels and cleanup controller. No protected compiler admission is made.
    let (stage, charge) = unsafe {
        if service {
            Stage::stage(
                &image,
                &[
                    Binding::new(output.as_fd(), 198).unwrap(),
                    Binding::new(selector.as_fd(), 199).unwrap(),
                ],
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
    assert_eq!(stage.compiler_arguments().is_some(), !service);
    b.reserve_storage(charge.additional_storage()).unwrap();
    if matches!(
        mode,
        Mode::NamespaceStorageShort | Mode::ObservationStorageShort
    ) {
        // Exactly one byte short of the shared pre-clone ABI/filter frame.
        b.reserve_storage(LIMIT - b.storage() - (Stage::SPAWN_SCRATCH - 1))
            .unwrap();
    }
    let original_storage = b.storage();
    let ceiling = observations::read_cap_last_cap().unwrap();
    let exact = stage
        .spawn_retaining_work::<Backing>(ceiling, source)
        .unwrap()
        + if mapped {
            Stage::FRESH_NAMESPACE_WORK
        } else {
            0
        };
    if matches!(mode, Mode::WorkShort | Mode::NamespaceWorkShort) {
        b.charge_work(LIMIT - b.work() - (exact - 1)).unwrap();
    }
    let before = b.work();
    let backing = Backing {
        _image: image,
        _cwd: cwd,
        _selector: selector,
        _terminal_witness: writer,
    };
    let credentials = Credentials::new(65534, 65534).unwrap();
    let personality = matches!(mode, Mode::InheritedReadImpliesExec)
        .then(CreatorPersonality::set_read_implies_exec);
    // SAFETY: complete actual sources enter the original independently funded
    // slot before clone. Only this test thread consumes waits; its pool drains
    // all error/unwind paths before fixture inputs or the creator are retired.
    // Mapped controls use the actual typed placement/map gate, not a generic
    // service mislabeled as a helper. Admission of a proof helper is not tested.
    let spawned = unsafe {
        if mapped {
            stage.spawn_retaining_in_fresh_user_namespace(
                credentials,
                Credentials::new(65533, 65533).unwrap(),
                backing,
                source,
                pool,
                &mut b,
            )
        } else {
            stage.spawn_retaining(credentials, backing, source, pool, &mut b)
        }
    };
    drop(personality);
    if matches!(
        mode,
        Mode::WorkShort
            | Mode::NamespaceWorkShort
            | Mode::NamespaceStorageShort
            | Mode::ObservationStorageShort
    ) {
        if matches!(
            mode,
            Mode::NamespaceStorageShort | Mode::ObservationStorageShort
        ) {
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
        assert!(b.work_ledger_identity_v1() == ledger);
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
    let domain;
    if let Some(failure_stage) = mode.failure_stage() {
        while child.is_live(&mut b).unwrap() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(rustix::io::read(&ready, &mut [0]).unwrap(), 0);
        let mut byte = [0];
        assert_eq!(rustix::io::read(&status, &mut byte).unwrap(), 1);
        // Stages distinguish acquisition (15), profile abort (3), post-drop
        // observation (16), and filter installation (13/14). No earlier error
        // or refused parent mapping may count as the requested child boundary.
        assert_eq!(byte, [0xc0 + failure_stage]);
        assert_eq!(rustix::io::read(&status, &mut [0]).unwrap(), 0);
        assert_eq!(output.metadata().unwrap().len(), 0);
        assert_eq!(rustix::io::read(&witness, &mut [0]), Err(Errno::AGAIN));
        // is_live is non-consuming; the exact terminal child still supplies its
        // proc membership. Only original cancellation/cleanup may remove it.
        domain = mapped.then(|| actual_domain(child.pid().as_raw_pid()));
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
        domain = mapped.then(|| actual_domain(child.pid().as_raw_pid()));
        observations::validate_process(credentials, child.pid()).unwrap();
        if !service {
            // The actual cap-free/nondumpable child is still held before exec.
            // Its private proc observation must already be closed, not CLOEXEC-only.
            for entry in
                std::fs::read_dir(format!("/proc/{}/fd", child.pid().as_raw_pid())).unwrap()
            {
                let target = std::fs::read_link(entry.unwrap().path()).unwrap();
                assert!(
                    !(target.starts_with("/proc") && target.ends_with("personality")),
                    "personality observation survived READY: {target:?}"
                );
            }
            let parent_pidns = std::fs::metadata("/proc/thread-self/ns/pid").unwrap();
            let child_pidns =
                std::fs::metadata(format!("/proc/{}/ns/pid", child.pid().as_raw_pid())).unwrap();
            assert_eq!(
                (child_pidns.dev(), child_pidns.ino()),
                (parent_pidns.dev(), parent_pidns.ino())
            );
        }
        if mapped {
            let parent = std::fs::metadata("/proc/thread-self/ns/user").unwrap();
            let actual =
                std::fs::metadata(format!("/proc/{}/ns/user", child.pid().as_raw_pid())).unwrap();
            assert_ne!((actual.dev(), actual.ino()), (parent.dev(), parent.ino()));
            for map in ["uid_map", "gid_map"] {
                let text =
                    std::fs::read_to_string(format!("/proc/{}/{map}", child.pid().as_raw_pid()))
                        .unwrap();
                let rows: Vec<Vec<u32>> = text
                    .lines()
                    .map(|line| {
                        line.split_whitespace()
                            .map(|field| field.parse().unwrap())
                            .collect()
                    })
                    .collect();
                assert_eq!(
                    rows,
                    [[0, 0, 1], [65533, 65533, 1], [65534, 65534, 1]],
                    "{map}"
                );
            }
        }
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
            if matches!(
                name,
                "ordinary" | "fork" | "fork-exec" | "namespace" | "creator"
            ) {
                assert_eq!(terminal.exit_code(), Some(7), "{name}");
            } else {
                assert_eq!(terminal.terminating_signal(), Some(libc::SIGSYS), "{name}");
            }
            assert_eq!(trace.poll(&mut b).unwrap(), terminal);
            assert_eq!(rustix::io::read(&witness, &mut [0]), Err(Errno::AGAIN));
            let _ = trace.cancel();
            drop(trace);
            let expected: &[u8] = if creator {
                b"creator probe\ncreator clone3 pidfd child complete\n"
            } else if namespace {
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
            if namespace || creator {
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
    if let Some(domain) = domain {
        assert_eq!(
            std::fs::metadata(domain).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
    assert_eq!(pool.report().unwrap().storage, baseline.storage);
    assert_eq!(pool.report().unwrap().work_limit, baseline.work_limit);
    assert_eq!(pool.report().unwrap().failed_work, baseline.failed_work);
    assert!(b.work_ledger_identity_v1() == ledger);
    if !matches!(mode, Mode::Unwind) {
        assert!(b.failed_work().is_none());
        assert!(b.failed_storage().is_none());
    }
}

fn actual_domain(pid: i32) -> std::path::PathBuf {
    fn membership(pid: i32) -> std::path::PathBuf {
        let file = File::open(format!("/proc/{pid}/cgroup")).unwrap();
        let mut text = String::new();
        file.take(4097).read_to_string(&mut text).unwrap();
        assert!(text.len() < 4096 && text.ends_with('\n'));
        let mut unified = text.lines().filter_map(|line| line.strip_prefix("0::"));
        let path = unified.next().unwrap();
        assert!(unified.next().is_none());
        let path = std::path::Path::new(path).strip_prefix("/").unwrap();
        assert!(
            path.components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        );
        std::path::Path::new("/sys/fs/cgroup").join(path)
    }
    let parent = membership(rustix::process::getpid().as_raw_pid());
    assert_ne!(
        parent,
        std::path::Path::new("/sys/fs/cgroup"),
        "isolated non-root parent required"
    );
    assert_eq!(
        std::fs::read(parent.join("cgroup.type")).unwrap(),
        b"domain\n"
    );
    let domain = membership(pid);
    assert_eq!(domain.parent(), Some(parent.as_path()));
    let name = domain.file_name().unwrap().to_str().unwrap();
    let suffix = name.strip_prefix("fe2o3-native-").unwrap();
    assert_eq!(suffix.len(), 32);
    assert!(
        suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let stat = std::fs::metadata(&domain).unwrap();
    assert!(stat.is_dir());
    assert_eq!((stat.uid(), stat.gid(), stat.mode() & 0o077), (0, 0, 0));
    assert_eq!(stat.dev(), std::fs::metadata(parent).unwrap().dev());
    // Observation only: the test never removes this path or writes controls.
    // The original production domain owner must retire it before backing EOF.
    domain
}

#[test]
#[ignore = "requires isolated root, clone3, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_filter_install_failure_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_filter_install_failure_is_terminal_without_ready_or_exec";
    isolated_control(TEST, Mode::InstallDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, clone3, seccomp and static -pthread native_compiler_exec.c"]
fn namespace_filter_install_failure_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::namespace_filter_install_failure_is_terminal_without_ready_or_exec";
    isolated_control(TEST, Mode::NamespaceInstallDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, clone3, procfs, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_personality_proc_read_denial_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_personality_proc_read_denial_is_terminal_without_ready_or_exec";
    isolated_control(TEST, Mode::ObservationReadDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, clone3, procfs, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_personality_proc_eof_denial_is_terminal_without_ready_or_exec() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_personality_proc_eof_denial_is_terminal_without_ready_or_exec";
    isolated_control(TEST, Mode::ObservationEofDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, clone3, pidfd_getfd, procfs, CAP_SYS_PTRACE, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_exact_child_observation_preserves_inherited_unit_personality_lock() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_exact_child_observation_preserves_inherited_unit_personality_lock";
    isolated_control(TEST, Mode::LockPreserved);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, clone3, procfs, CAP_SYS_PTRACE, seccomp and static -pthread native_compiler_exec.c"]
fn mapped_compiler_acquires_its_own_observation_after_actual_mapping() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::mapped_compiler_acquires_its_own_observation_after_actual_mapping";
    isolated_control(TEST, Mode::MappedLockPreserved);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, clone3, procfs, seccomp and static -pthread native_compiler_exec.c"]
fn mapped_compiler_personality_proc_read_denial_reaches_stage16_and_retires_domain() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::mapped_compiler_personality_proc_read_denial_reaches_stage16_and_retires_domain";
    isolated_control(TEST, Mode::MappedObservationReadDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, clone3, procfs, seccomp and static -pthread native_compiler_exec.c"]
fn mapped_compiler_personality_proc_eof_denial_reaches_stage16_and_retires_domain() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::mapped_compiler_personality_proc_eof_denial_reaches_stage16_and_retires_domain";
    isolated_control(TEST, Mode::MappedObservationEofDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, initial personality setter/query, clone3, procfs, seccomp and static -pthread native_compiler_exec.c"]
fn mapped_compiler_dirty_inheritance_reaches_stage16_and_retires_domain() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::mapped_compiler_dirty_inheritance_reaches_stage16_and_retires_domain";
    isolated_control(TEST, Mode::MappedDirtyLockPreserved);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, clone3, procfs, CAP_SYS_PTRACE, seccomp and static -pthread native_compiler_exec.c"]
fn mapped_compiler_personality_acquisition_refusal_reaches_stage15_and_retires_domain() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::mapped_compiler_personality_acquisition_refusal_reaches_stage15_and_retires_domain";
    isolated_control(TEST, Mode::MappedAcquisitionDenied);
}

#[test]
#[ignore = "requires isolated root/outside custodian, protected non-root ordinary-domain writable cgroup2, userns/CAP_SETFCAP, allowed PR_SET_DUMPABLE, clone3, procfs, CAP_SYS_PTRACE, seccomp and static -pthread native_compiler_exec.c"]
fn mapped_compiler_profile_abort_after_acquisition_reaches_stage3_and_retires_domain() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::mapped_compiler_profile_abort_after_acquisition_reaches_stage3_and_retires_domain";
    isolated_control(TEST, Mode::MappedProfileAbort);
}

#[test]
#[ignore = "requires isolated root/outside custodian, initial personality setter/query, clone3, procfs, seccomp and static -pthread native_compiler_exec.c"]
fn compiler_dirty_inheritance_under_unit_lock_refuses_without_clearing() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_dirty_inheritance_under_unit_lock_refuses_without_clearing";
    isolated_control(TEST, Mode::DirtyLockPreserved);
}

#[test]
#[ignore = "requires isolated root, personality setter/query, clone3, CAP_SYS_PTRACE and static -pthread native_compiler_exec.c"]
fn compiler_inherited_read_implies_exec_is_rejected_and_creator_restored() {
    const TEST: &str = "native_spawn::compiler_spawn::exec_tests::restrictions::compiler_inherited_read_implies_exec_is_rejected_and_creator_restored";
    isolated_control(TEST, Mode::InheritedReadImpliesExec);
}

fn isolated_control(test: &str, mode: Mode<'_>) {
    const SENTINEL: &str = "FE2O3_COMPILER_RESTRICTION_CONTROL_CHILD";
    if std::env::var_os(SENTINEL).is_some() {
        let mut pool = pool();
        if mode.read_denied() || mode.eof_denied() {
            check_read_boundary(mode, false);
        }
        if matches!(mode, Mode::InstallDenied | Mode::NamespaceInstallDenied)
            || mode.read_denied()
            || mode.eof_denied()
        {
            install_denial(mode);
        }
        if mode.read_denied() || mode.eof_denied() {
            check_read_boundary(mode, true);
        }
        if matches!(mode, Mode::InstallDenied) {
            let observed = current_personality();
            assert!(
                observed >= 0,
                "installation refusal requires an allowed query"
            );
            assert_eq!(observed & READ_IMPLIES_EXEC, 0);
        }
        let locked = matches!(mode, Mode::LockPreserved | Mode::MappedLockPreserved)
            || mode.dirty_locked()
            || mode.read_denied()
            || mode.eof_denied()
            || mode.early_fault();
        if locked {
            assert_eq!(
                current_personality(),
                0,
                "requires a clean native creator personality"
            );
            if mode.dirty_locked() {
                // SAFETY: isolated exact-test thread only, before the inherited
                // filter; no exec occurs between this mutation and production clone.
                assert_eq!(
                    unsafe { libc::syscall(libc::SYS_personality, READ_IMPLIES_EXEC) },
                    0
                );
                assert_eq!(current_personality(), READ_IMPLIES_EXEC);
            }
            install_unit_personality_lock();
            let observed = current_personality();
            let error = std::io::Error::last_os_error();
            assert_eq!(observed, -1);
            assert_eq!(error.raw_os_error(), Some(libc::EPERM));
        }
        if matches!(mode, Mode::LockPreserved) {
            run(&mut pool.0, Mode::WorkShort);
            run(&mut pool.0, Mode::ObservationStorageShort);
        }
        if mode.early_fault() {
            // First prove the exact mapped/compiler path reaches READY and exec
            // under the inherited lock, before introducing the sole new denial.
            run(&mut pool.0, Mode::MappedLockPreserved);
            check_early_boundary(mode, false);
            install_denial(mode);
            check_early_boundary(mode, true);
        }
        run(&mut pool.0, mode);
        if matches!(mode, Mode::LockPreserved) {
            run(&mut pool.0, Mode::Execute("personality"));
            run(&mut pool.0, Mode::Execute("fork-exec"));
            let image = File::open(std::env::var_os("FE2O3_NATIVE_COMPILER_EXEC_FIXTURE").unwrap())
                .unwrap();
            let baseline = pool.0.report().unwrap();
            // Reuse the genuine production child-channel success/failure path.
            // Its socket closes run while the private observation is retained.
            super::run(&image, true, super::Channel::Enabled, &mut pool.0);
            super::run(&image, true, super::Channel::Backpressure, &mut pool.0);
            let after = pool.0.report().unwrap();
            assert_eq!(after.storage, baseline.storage);
            assert_eq!(after.work_limit, baseline.work_limit);
            assert_eq!(after.failed_work, baseline.failed_work);
        }
        if mode.dirty_locked() {
            assert_eq!(
                std::fs::read("/proc/thread-self/personality").unwrap(),
                b"00400000\n"
            );
            // No restoration/clearing: this isolated process terminates with the
            // same dirty state. Its parent never changed state or installed this filter.
        }
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
        drop(pool);
        println!("PERSONALITY-CONTROL:{test}:reached");
        return;
    }
    // Regular output storage cannot deadlock on a full pipe or a surviving
    // descendant writer. The outside custodian still owns whole-tree cleanup.
    let mut log = tempfile::tempfile().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            test,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SENTINEL, "1")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log.try_clone().unwrap());
    let mut process = ControlProcess(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = process.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "isolated restriction control timed out: {test}"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    log.rewind().unwrap();
    let mut bytes = Vec::new();
    log.take(65537).read_to_end(&mut bytes).unwrap();
    assert!(
        bytes.len() <= 65536,
        "isolated restriction control log overflow"
    );
    let output = String::from_utf8_lossy(&bytes);
    assert!(
        status.success(),
        "isolated restriction control {test}: {output}"
    );
    assert!(
        output.contains("1 passed"),
        "exact child test did not execute"
    );
    assert!(
        output.contains(&format!("PERSONALITY-CONTROL:{test}:reached")),
        "exact child did not reach its observation control"
    );
}

struct ControlProcess(std::process::Child);
impl Drop for ControlProcess {
    fn drop(&mut self) {
        // Command owns this direct subprocess. This is not a descendant/cgroup
        // custodian, and cannot qualify the deployment's external cleanup duty.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn check_read_boundary(mode: Mode<'_>, denied: bool) {
    // Preflight only: prove the kernel filter targets the intended record or EOF
    // operation, not acquisition. The production child must separately report
    // its exact post-drop refusal stage and no READY/exec.
    let file = File::open("/proc/thread-self/personality").unwrap();
    let mut record = [0_u8; 10];
    let read = rustix::io::pread(&file, &mut record, 0);
    let eof = rustix::io::pread(&file, &mut [0_u8; 1], 9);
    if denied && mode.read_denied() {
        assert_eq!(read, Err(Errno::PERM));
    } else {
        assert_eq!(read, Ok(9));
        assert_eq!(&record[..9], b"00000000\n");
    }
    assert_eq!(
        eof,
        if denied && mode.eof_denied() {
            Err(Errno::PERM)
        } else {
            Ok(0)
        }
    );
}

fn check_early_boundary(mode: Mode<'_>, denied: bool) {
    // These preflights establish actual kernel/filter behavior in the isolated
    // caller, not child admission. The subsequent production clone must reach
    // its distinct stage and preserve original cleanup/accounting custody.
    match mode {
        Mode::MappedAcquisitionDenied => {
            let mut path = [0_u8; 32];
            // SAFETY: readonly kernel link, fixed initialized output buffer.
            let count = unsafe {
                libc::syscall(
                    libc::SYS_readlinkat,
                    libc::AT_FDCWD,
                    c"/proc/thread-self".as_ptr(),
                    path.as_mut_ptr(),
                    path.len(),
                )
            };
            let error = std::io::Error::last_os_error().raw_os_error();
            if denied {
                assert_eq!(count, -1);
                assert_eq!(error, Some(libc::EPERM));
            } else {
                assert!(count > 0 && count < path.len() as libc::c_long);
                // SAFETY: scalar query of the exact preflight calling thread.
                let tid = unsafe { libc::syscall(libc::SYS_gettid) };
                assert_eq!(
                    &path[..count as usize],
                    format!("{}/task/{tid}", std::process::id()).as_bytes()
                );
            }
        }
        Mode::MappedProfileAbort => {
            // SAFETY: only this isolated exact-test subprocess's mm is changed.
            // Before installing the denial, restore the exact original value
            // before asserting results, including a failed setter. Afterward
            // the denied setter cannot modify that value.
            unsafe {
                let original = libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0);
                assert!(matches!(original, 0 | 1), "requires restorable dumpability");
                let result = libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
                let error = std::io::Error::last_os_error().raw_os_error();
                if denied {
                    assert_eq!(result, -1);
                    assert_eq!(error, Some(libc::EPERM));
                } else {
                    let restored = libc::prctl(libc::PR_SET_DUMPABLE, original, 0, 0, 0);
                    assert_eq!(result, 0);
                    assert_eq!(restored, 0);
                }
                assert_eq!(libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0), original);
            }
        }
        _ => panic!("not an early personality fault control"),
    }
}

fn install_denial(mode: Mode<'_>) {
    if mode.read_denied() || mode.eof_denied() {
        let (count, offset) = if mode.read_denied() { (10, 0) } else { (1, 9) };
        install_test_filter(&[
            instruction(0x20, 0, 0, 0),
            instruction(0x15, libc::SYS_pread64 as u32, 0, 5),
            instruction(0x20, 32, 0, 0),
            instruction(0x15, count, 0, 3),
            instruction(0x20, 40, 0, 0),
            instruction(0x15, offset, 0, 1),
            instruction(0x06, 0x0005_0001, 0, 0),
            instruction(0x06, 0x7fff_0000, 0, 0),
        ]);
        return;
    }
    if matches!(mode, Mode::MappedAcquisitionDenied) {
        // The child-local acquisition uses exactly this bounded link buffer.
        // Parent mapping/metadata operations do not use readlinkat with count 32.
        install_test_filter(&[
            instruction(0x20, 0, 0, 0),
            instruction(0x15, libc::SYS_readlinkat as u32, 0, 3),
            instruction(0x20, 40, 0, 0),
            instruction(0x15, 32, 0, 1),
            instruction(0x06, 0x0005_0001, 0, 0),
            instruction(0x06, 0x7fff_0000, 0, 0),
        ]);
        return;
    }
    if matches!(mode, Mode::MappedProfileAbort) {
        // Profile transition's final nondumpability setter, after acquisition,
        // UID/GID transition and capset. Do not deny earlier parent mapping.
        install_test_filter(&[
            instruction(0x20, 0, 0, 0),
            instruction(0x15, libc::SYS_prctl as u32, 0, 5),
            instruction(0x20, 16, 0, 0),
            instruction(0x15, libc::PR_SET_DUMPABLE as u32, 0, 3),
            instruction(0x20, 24, 0, 0),
            instruction(0x15, 0, 0, 1),
            instruction(0x06, 0x0005_0001, 0, 0),
            instruction(0x06, 0x7fff_0000, 0, 0),
        ]);
        return;
    }
    let (nr, argument) = match mode {
        Mode::InstallDenied | Mode::NamespaceInstallDenied => (157, 22), // prctl(PR_SET_SECCOMP)
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

fn instruction(code: u16, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

fn install_unit_personality_lock() {
    // Native-ABI equivalent of systemd v255 LockPersonality=yes/PER_LINUX:
    // permit exactly argument 0, deny every other full-width argument, query too.
    install_test_filter(&[
        instruction(0x20, 0, 0, 0),
        instruction(0x15, libc::SYS_personality as u32, 0, 5),
        instruction(0x20, 16, 0, 0),
        instruction(0x15, 0, 0, 2),
        instruction(0x20, 20, 0, 0),
        instruction(0x15, 0, 1, 0),
        instruction(0x06, 0x0005_0001, 0, 0),
        instruction(0x06, 0x7fff_0000, 0, 0),
    ]);
}

fn install_test_filter(filter: &[libc::sock_filter]) {
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_ptr().cast_mut(),
    };
    // SAFETY: isolated exact-test subprocess only. Kernel copies the live BPF
    // synchronously; descendants inherit this kernel restriction, not a mock.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(
            libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0),
            0
        );
    }
}
