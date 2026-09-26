// Public fixed-slot admission fixtures only, not protected issuer/compiler evidence.
// Each reserved-slot operation runs in one exact child test, never the parallel parent.
// Fixture allocation/inspection is outside the logical admission resource account.
use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 as CHILD_FD, CompilerExecutionClientErrorV1 as Transport,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::{
    net::{self, AddressFamily, SocketFlags, SocketType},
    pipe::{PipeFlags, pipe_with},
};
use std::{
    fs, io,
    mem::size_of,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{
            fs::{FileTypeExt, MetadataExt},
            process::CommandExt,
        },
    },
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const CASE_ENV: &str = "FE2O3_NATIVE_INHERITED_CASE";
const CHILD_TEST: &str = "native_inherited_case_child";
const STEP: Duration = Duration::from_millis(5);
const CASE_TIMEOUT: Duration = Duration::from_secs(10);
const MATRIX_TIMEOUT: Duration = Duration::from_secs(120);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
const WORK: usize = 1_000_000;
const STORAGE: usize = 64 * 1024;
const ADMISSION_WORK: usize = 64 * 1024 + 8;
const EXTRA: usize = 23;
const PREFIX: usize = 19;
const CASES: &[&str] = &[
    "success",
    "work-exact",
    "missing",
    "cloexec",
    "pipe",
    "stream",
    "unconnected",
    "timeout-zero",
    "timeout-long",
    "missing-timeout",
    "cloexec-timeout",
    "pipe-timeout",
    "quota-entry",
    "quota-floor",
    "quota-work",
    "quota-scratch",
    "missing-quota",
];

struct ChildGuard(Child);
impl ChildGuard {
    fn wait(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        for _ in 0..2_000 {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(STEP);
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "inherited admission child",
        ))
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.0.kill();
        if let Err(error) = self.wait(CLEANUP_TIMEOUT) {
            eprintln!("inherited admission child cleanup failed: {error}");
        }
    }
}

fn socket_pair(kind: SocketType) -> (OwnedFd, OwnedFd) {
    net::socketpair(
        AddressFamily::UNIX,
        kind,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .expect("real unnamed Unix socket fixture required")
}

fn closed_pipe_reader() -> OwnedFd {
    let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
    drop(writer);
    reader
}

fn fixture(case: &str) -> (Option<OwnedFd>, OwnedFd) {
    match case {
        "success" | "work-exact" | "timeout-zero" | "timeout-long" => {
            let (peer, observer) = socket_pair(SocketType::SEQPACKET);
            (Some(peer), observer)
        }
        "stream" => {
            let (peer, observer) = socket_pair(SocketType::STREAM);
            (Some(peer), observer)
        }
        "unconnected" => (
            Some(
                net::socket(
                    AddressFamily::UNIX,
                    SocketType::SEQPACKET,
                    SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                    None,
                )
                .expect("real unconnected Unix socket fixture required"),
            ),
            closed_pipe_reader(),
        ),
        "missing" | "missing-timeout" | "missing-quota" => (None, closed_pipe_reader()),
        _ => {
            let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
            (Some(writer), reader)
        }
    }
}

#[test]
fn native_inherited_public_admission_matrix() {
    let deadline = Instant::now() + MATRIX_TIMEOUT;
    for case in CASES {
        assert!(
            Instant::now() < deadline,
            "inherited admission matrix deadline"
        );
        let (peer, observer) = fixture(case);
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                CHILD_TEST,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env(CASE_ENV, case)
            .stdin(Stdio::from(observer))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        // SAFETY: only async-signal-safe descriptor syscalls run after fork.
        // The captured source stays owned until exec; only this child's fixed slot changes.
        unsafe {
            command.pre_exec(move || {
                if let Some(peer) = &peer {
                    if libc::dup2(peer.as_raw_fd(), CHILD_FD) != CHILD_FD
                        || libc::fcntl(CHILD_FD, libc::F_SETFD, 0) < 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                } else if libc::close(CHILD_FD) != 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::EBADF) {
                        return Err(error);
                    }
                }
                Ok(())
            });
        }
        let mut child = ChildGuard(command.spawn().unwrap());
        // No parent alias of the admitted endpoint may keep the EOF witness alive.
        drop(command);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let status = child.wait(CASE_TIMEOUT.min(remaining)).unwrap();
        assert!(
            status.success(),
            "inherited admission case {case}: {status}"
        );
    }
}

fn canonical_vacancy(observer: &OwnedFd) -> OwnedFd {
    // Atomic allocation proves the slot was free and owns its reuse immediately.
    // Never inspect a released descriptor number in a parallel test process.
    let replacement = rustix::io::fcntl_dupfd_cloexec(observer, CHILD_FD).unwrap();
    assert_eq!(
        replacement.as_raw_fd(),
        CHILD_FD,
        "canonical source was not consumed"
    );
    replacement
}

fn assert_eof(observer: &OwnedFd) {
    // The child can run before the parent drops its spawn-only source alias.
    // Observe this same pipe/socket until EOF; never inspect a released fd number.
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    for _ in 0..400 {
        match rustix::io::read(observer, &mut [0; 1]) {
            Ok(0) => return,
            Ok(_) => panic!("unexpected bytes on closure witness"),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
            Err(error) => panic!("closure witness: {error}"),
        }
        assert!(Instant::now() < deadline, "inherited input did not close");
        thread::sleep(STEP);
    }
    panic!("closure witness attempt limit");
}

fn matching_descriptors(identity: (u64, u64)) -> Vec<i32> {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = fs::metadata(entry.path()).ok()?;
            if (metadata.dev(), metadata.ino()) == identity {
                entry.file_name().to_str()?.parse().ok()
            } else {
                None
            }
        })
        .collect()
}

#[test]
#[ignore = "private fixed-FD child role; run only through native_inherited_public_admission_matrix"]
fn native_inherited_case_child() {
    let case = std::env::var(CASE_ENV).expect("explicit inherited admission child case");
    assert!(CASES.contains(&case.as_str()));
    let observer = rustix::io::fcntl_dupfd_cloexec(std::io::stdin(), 3).unwrap();
    let missing = case.starts_with("missing");
    let source = fs::metadata(format!("/proc/self/fd/{CHILD_FD}"));
    if missing {
        assert_eq!(source.as_ref().unwrap_err().kind(), io::ErrorKind::NotFound);
    } else {
        assert!(source.is_ok());
        // SAFETY: the fixture inherited this exact reserved descriptor through exec.
        assert_eq!(unsafe { libc::fcntl(CHILD_FD, libc::F_GETFD) }, 0);
        if case.starts_with("cloexec") || case.starts_with("quota-") {
            // SAFETY: inject one flag fault only in this child's exclusively owned slot.
            assert_eq!(
                unsafe { libc::fcntl(CHILD_FD, libc::F_SETFD, libc::FD_CLOEXEC) },
                0
            );
        }
    }
    let socket_identity = source
        .ok()
        .filter(|s| s.file_type().is_socket())
        .map(|s| (s.dev(), s.ino()));
    let extra = if case == "quota-floor" { 0 } else { EXTRA };
    let prepaid = Client::PEER_STORAGE - usize::from(case == "quota-floor");
    let floor = extra + prepaid;
    let work_limit = match case.as_str() {
        "quota-entry" | "missing-quota" => PREFIX + 7,
        "quota-work" => PREFIX + ADMISSION_WORK - 1,
        "work-exact" => PREFIX + ADMISSION_WORK,
        _ => WORK,
    };
    let storage_limit = if case == "quota-scratch" {
        floor + 8191
    } else {
        STORAGE
    };
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(floor).unwrap();
    let original = budget.work_ledger_identity_v1();
    let timeout = match case.as_str() {
        "timeout-zero" | "missing-timeout" | "cloexec-timeout" | "pipe-timeout" | "quota-entry"
        | "quota-floor" | "quota-work" | "quota-scratch" | "missing-quota" => Duration::ZERO,
        "timeout-long" => Duration::from_secs(301),
        _ => Duration::from_secs(1),
    };
    let result = Client::admit_inherited_child(timeout, &mut budget);
    let replacement = canonical_vacancy(&observer);
    if matches!(case.as_str(), "success" | "work-exact") {
        let client =
            result.unwrap_or_else(|error| panic!("public native inherited admission: {error}"));
        let descriptors = matching_descriptors(socket_identity.unwrap());
        assert_eq!(descriptors.len(), 1, "exactly one retained input duplicate");
        let private = descriptors[0];
        assert!(private >= 3 && private != CHILD_FD);
        // SAFETY: the admitted client is still alive and this isolated child has
        // no other fixture thread that can close or replace its retained descriptor.
        let flags = unsafe { libc::fcntl(private, libc::F_GETFD) };
        assert!(flags >= 0 && flags & libc::FD_CLOEXEC != 0);
        let client = if case == "success" {
            let (client, ()) = client
                .prepare::<_, Error>(|b| {
                    assert!(b.work_ledger_identity_v1() == original);
                    assert_eq!(b.work(), PREFIX + ADMISSION_WORK + 8);
                    assert_eq!(b.storage(), EXTRA + size_of::<Client<'_, '_>>());
                    Ok(())
                })
                .unwrap();
            client
        } else {
            client
        };
        drop(client);
        assert_eq!(budget.storage(), EXTRA);
    } else {
        let error = match result {
            Ok(client) => {
                drop(client);
                panic!("unexpected inherited admission for {case}");
            }
            Err(error) => error,
        };
        match case.as_str() {
            "quota-entry" | "missing-quota" | "quota-work" => {
                assert!(matches!(error, Error::Resource(Resource::Work(_))));
            }
            "quota-floor" => assert!(matches!(error, Error::Resource(Resource::Accounting))),
            "quota-scratch" => assert!(matches!(error, Error::Resource(Resource::Storage(_)))),
            "missing" | "missing-timeout" => assert!(matches!(
                error,
                Error::Transport(Transport::MissingInheritedPeer)
            )),
            "cloexec" | "cloexec-timeout" => assert!(matches!(
                error,
                Error::Transport(Transport::InheritedPeerCloseOnExec)
            )),
            "pipe" => assert!(
                matches!(error, Error::Transport(Transport::Descriptor(ref e)) if e.raw_os_error() == Some(libc::ENOTSOCK))
            ),
            "stream" => assert!(matches!(error, Error::Transport(Transport::NotSeqpacket))),
            "unconnected" => assert!(matches!(
                error,
                Error::Transport(Transport::NamedOrNonUnixPeer)
            )),
            "timeout-zero" | "timeout-long" | "pipe-timeout" => {
                assert!(matches!(error, Error::Transport(Transport::InvalidTimeout)))
            }
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), floor);
    }
    assert_eof(&observer);
    if let Some(identity) = socket_identity {
        assert!(
            matching_descriptors(identity).is_empty(),
            "retained input leaked"
        );
    }
    // The client's drop must not close the independently reoccupied canonical slot.
    assert_eof(&replacement);
    drop(replacement);
    assert!(budget.work_ledger_identity_v1() == original);
    let expected_work = match case.as_str() {
        "quota-entry" | "missing-quota" => PREFIX,
        "quota-floor" | "quota-work" => PREFIX + 8,
        "success" => PREFIX + ADMISSION_WORK + 8,
        _ => PREFIX + ADMISSION_WORK,
    };
    assert_eq!(budget.work(), expected_work);
    let failed_work = match case.as_str() {
        "quota-entry" | "missing-quota" => Some(PREFIX + 8),
        "quota-work" => Some(PREFIX + ADMISSION_WORK),
        _ => None,
    };
    assert_eq!(budget.failed_work(), failed_work);
    assert_eq!(
        budget.failed_storage(),
        if case == "quota-scratch" {
            Some(floor + 8192)
        } else {
            None
        }
    );
    let expected_peak = match case.as_str() {
        "quota-entry" | "missing-quota" | "quota-floor" | "quota-work" | "quota-scratch" => floor,
        _ => floor + 8192,
    };
    assert_eq!(budget.peak_storage(), expected_peak);
    let remaining = budget.storage();
    budget.release_storage(remaining).unwrap();
    assert_eq!(budget.storage(), 0);
}
