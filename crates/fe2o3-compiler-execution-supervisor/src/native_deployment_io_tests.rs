use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    collections::VecDeque,
    fs,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{fs::MetadataExt, process::CommandExt},
    },
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Command, Stdio},
};

struct Script {
    start: Instant,
    clocks: usize,
    sends: usize,
    pauses: usize,
    results: VecDeque<std::result::Result<usize, Errno>>,
    fallback: std::result::Result<usize, Errno>,
    pause_result: std::result::Result<(), Errno>,
    expire_at_clock: usize,
    panic_send: bool,
}
impl Script {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            clocks: 0,
            sends: 0,
            pauses: 0,
            results: VecDeque::new(),
            fallback: Ok(READY_BYTES),
            pause_result: Ok(()),
            expire_at_clock: usize::MAX,
            panic_send: false,
        }
    }
}
impl ReadyIo for Script {
    fn now(&mut self) -> Instant {
        self.clocks += 1;
        if self.clocks >= self.expire_at_clock {
            self.start + SEND_TIMEOUT
        } else {
            self.start
        }
    }
    fn send(&mut self, _: BorrowedFd<'_>, bytes: &[u8]) -> std::result::Result<usize, Errno> {
        self.sends += 1;
        assert_eq!(bytes, &[7; READY_BYTES]);
        assert!(!self.panic_send, "scripted send unwind");
        self.results.pop_front().unwrap_or(self.fallback)
    }
    fn pause(&mut self) -> std::result::Result<(), Errno> {
        self.pauses += 1;
        self.pause_result
    }
}
fn boot() -> OwnedFd {
    File::open("/dev/null").unwrap().into()
}
const FLOOR: usize = FILE_STORAGE + READY_BYTES;

#[test]
fn readiness_exact_single_attempt_restores_storage_and_retains_original_work() {
    let boot = boot();
    let mut io = Script::new();
    let mut w = Work::new(19 + SEND_BASE_WORK + SEND_ATTEMPT_WORK);
    let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
    b.charge_work(19).unwrap();
    b.reserve_storage(FLOOR).unwrap();
    let ledger = b.work_ledger_identity_v1();
    send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io).unwrap();
    assert_eq!((io.clocks, io.sends, io.pauses), (3, 1, 0));
    assert_eq!(b.work(), 19 + SEND_BASE_WORK + SEND_ATTEMPT_WORK);
    assert_eq!(b.storage(), FLOOR);
    assert_eq!(b.peak_storage(), FLOOR + SEND_SCRATCH);
    assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
    assert!(ledger == b.work_ledger_identity_v1());
    assert!(rustix::io::fcntl_getfd(&boot).is_ok());
}

#[test]
fn readiness_retries_each_send_and_interrupted_pause_on_the_same_ledger() {
    let boot = boot();
    let mut io = Script::new();
    io.results = [Err(Errno::AGAIN), Err(Errno::INTR), Ok(READY_BYTES)].into();
    io.pause_result = Err(Errno::INTR);
    let required = SEND_BASE_WORK + 3 * SEND_ATTEMPT_WORK + 2 * SEND_PAUSE_WORK;
    let mut w = Work::new(required);
    let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io).unwrap();
    assert_eq!((io.clocks, io.sends, io.pauses), (11, 3, 2));
    assert_eq!(b.work(), required);
    assert_eq!(b.storage(), FLOOR);
}

#[test]
fn frozen_clock_and_repeated_eintr_exhaust_the_exact_finite_attempt_quota() {
    let boot = boot();
    let mut io = Script::new();
    io.fallback = Err(Errno::INTR);
    io.pause_result = Err(Errno::INTR);
    let expected = SEND_BASE_WORK + 30_001 * SEND_ATTEMPT_WORK + 30_000 * SEND_PAUSE_WORK;
    assert_eq!(SEND_WORK, expected);
    let mut w = Work::new(expected);
    let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io),
        Err(Error::ReadyAttempts)
    ));
    assert_eq!((io.sends, io.pauses), (30_001, 30_000));
    assert_eq!(io.clocks, 1 + 2 * 30_001 + 2 * 30_000);
    assert_eq!(b.work(), SEND_WORK);
    assert_eq!(b.storage(), FLOOR);
}

#[test]
fn readiness_deadlines_cover_before_send_success_and_both_pause_boundaries() {
    let boot = boot();
    for clock in 2..=5 {
        let mut io = Script::new();
        io.expire_at_clock = clock;
        if clock >= 4 {
            io.fallback = Err(Errno::AGAIN);
        }
        let mut w = Work::new(SEND_WORK);
        let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
        b.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io),
            Err(Error::ReadyTimeout)
        ));
        assert_eq!(io.sends, usize::from(clock != 2));
        assert_eq!(io.pauses, usize::from(clock == 5));
        assert_eq!(
            b.work(),
            SEND_BASE_WORK + SEND_ATTEMPT_WORK + if clock >= 4 { SEND_PAUSE_WORK } else { 0 }
        );
        assert_eq!(b.storage(), FLOOR);
    }
}

#[test]
fn readiness_partial_and_terminal_io_errors_never_retry() {
    let boot = boot();
    for sent in [
        Ok(0),
        Ok(READY_BYTES - 1),
        Ok(READY_BYTES + 1),
        Err(Errno::PIPE),
    ] {
        let mut io = Script::new();
        io.fallback = sent;
        let mut w = Work::new(SEND_WORK);
        let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
        b.reserve_storage(FLOOR).unwrap();
        let result = send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io);
        if sent.is_ok() {
            assert!(matches!(result, Err(Error::ReadyPartial)));
        } else {
            assert!(matches!(
                result,
                Err(Error::Io {
                    errno: Errno::PIPE,
                    ..
                })
            ));
        }
        assert_eq!((io.sends, io.pauses), (1, 0));
        assert_eq!(b.work(), SEND_BASE_WORK + SEND_ATTEMPT_WORK);
    }
    let mut io = Script::new();
    io.fallback = Err(Errno::AGAIN);
    io.pause_result = Err(Errno::IO);
    let mut w = Work::new(SEND_WORK);
    let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io),
        Err(Error::Io {
            errno: Errno::IO,
            ..
        })
    ));
    assert_eq!((io.sends, io.pauses), (1, 1));
    assert_eq!(
        b.work(),
        SEND_BASE_WORK + SEND_ATTEMPT_WORK + SEND_PAUSE_WORK
    );
}

#[test]
fn readiness_precharges_floor_frame_and_every_attempt_before_calling_out() {
    let boot = boot();
    for case in 0..7 {
        let work_limit = match case {
            0 => ENTRY - 1,
            2 => SEND_BASE_WORK - 1,
            4 => SEND_BASE_WORK + SEND_ATTEMPT_WORK - 1,
            5 => SEND_BASE_WORK + SEND_ATTEMPT_WORK + SEND_PAUSE_WORK - 1,
            6 => SEND_BASE_WORK + 2 * SEND_ATTEMPT_WORK + SEND_PAUSE_WORK - 1,
            _ => SEND_WORK,
        };
        let mut io = Script::new();
        io.fallback = Err(Errno::AGAIN);
        let mut w = Work::new(work_limit);
        let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH - usize::from(case == 3));
        let floor = FLOOR - usize::from(case == 1);
        b.reserve_storage(floor).unwrap();
        let result = send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io);
        match case {
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            3 => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(io.sends, usize::from(case >= 5));
        assert_eq!(io.pauses, usize::from(case == 6));
        assert_eq!(
            io.clocks,
            match case {
                4 => 1,
                5 => 3,
                6 => 5,
                _ => 0,
            }
        );
        assert_eq!(
            b.work(),
            match case {
                0 => 0,
                1 | 2 => ENTRY,
                3 | 4 => SEND_BASE_WORK,
                5 => SEND_BASE_WORK + SEND_ATTEMPT_WORK,
                _ => SEND_BASE_WORK + SEND_ATTEMPT_WORK + SEND_PAUSE_WORK,
            }
        );
    }
}

#[test]
fn readiness_wrong_lengths_never_reach_clock_or_transport() {
    let boot = boot();
    for bytes in [
        &[][..],
        &[7; READY_BYTES - 1][..],
        &[7; READY_BYTES + 1][..],
    ] {
        let mut io = Script::new();
        let floor = FILE_STORAGE + bytes.len();
        let mut w = Work::new(SEND_BASE_WORK);
        let mut b = Budget::new(&mut w, floor + SEND_SCRATCH);
        b.reserve_storage(floor).unwrap();
        assert!(matches!(
            send_ready_with(boot.as_fd(), bytes, &mut b, &mut io),
            Err(Error::ReadyLength)
        ));
        assert_eq!((io.clocks, io.sends, io.pauses), (0, 0, 0));
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn readiness_storage_and_work_overflow_refuse_before_io() {
    let boot = boot();
    for storage in [false, true] {
        let mut io = Script::new();
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let floor = if storage {
            usize::MAX - SEND_SCRATCH + 1
        } else {
            FLOOR
        };
        b.reserve_storage(floor).unwrap();
        if !storage {
            b.charge_work(usize::MAX).unwrap();
        }
        let result = send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io);
        if storage {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(b.failed_storage(), Some(usize::MAX));
            assert_eq!(b.work(), SEND_BASE_WORK);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(b.failed_work(), Some(usize::MAX));
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor);
        assert_eq!(io.clocks, 0);
    }
}

#[test]
fn readiness_unwind_and_success_preserve_prior_denials_peak_and_borrowed_fd() {
    let boot = boot();
    for unwind in [false, true] {
        let mut io = Script::new();
        io.panic_send = unwind;
        let mut w = Work::new(SEND_WORK);
        let mut b = Budget::new(&mut w, FLOOR + 2 * SEND_SCRATCH);
        b.reserve_storage(FLOOR + 2 * SEND_SCRATCH).unwrap();
        b.release_storage(2 * SEND_SCRATCH).unwrap();
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
        let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
        let ledger = b.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            send_ready_with(boot.as_fd(), &[7; READY_BYTES], &mut b, &mut io)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            result.unwrap().unwrap();
        }
        assert_eq!(b.storage(), FLOOR);
        assert_eq!(b.work(), SEND_BASE_WORK + SEND_ATTEMPT_WORK);
        assert_eq!(
            (b.failed_work(), b.failed_storage(), b.peak_storage()),
            history
        );
        assert!(ledger == b.work_ledger_identity_v1());
        assert!(rustix::io::fcntl_getfd(&boot).is_ok());
    }
}

#[test]
fn actual_non_socket_refusal_preserves_borrowed_descriptor() {
    let boot = boot();
    let mut w = Work::new(SEND_BASE_WORK + SEND_ATTEMPT_WORK);
    let mut b = Budget::new(&mut w, FLOOR + SEND_SCRATCH);
    b.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        send_ready(&boot, &[7; READY_BYTES], &mut b),
        Err(Error::Io { .. })
    ));
    assert_eq!(b.storage(), FLOOR);
    assert!(rustix::io::fcntl_getfd(&boot).is_ok());
}

const CASE_ENV: &str = "FE2O3_NATIVE_SUPERVISOR_IO_CASE";
const REPORT_ENV: &str = "FE2O3_NATIVE_SUPERVISOR_IO_REPORT";

#[test]
fn isolated_raw_source_cases() {
    for case in [
        "fixed",
        "missing",
        "flags",
        "swapped",
        "refusal",
        "unwind",
        "close-error",
    ] {
        // Named paths pin inode identities through all child closure assertions.
        let files: [_; 11] = std::array::from_fn(|_| tempfile::NamedTempFile::new().unwrap());
        let retained: [_; 11] = std::array::from_fn(|i| {
            rustix::io::fcntl_dupfd_cloexec(files[i].as_file(), 400).unwrap()
        });
        let report = tempfile::NamedTempFile::new().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "native_deployment_io::tests::raw_source_subprocess",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CASE_ENV, case)
            .env(REPORT_ENV, report.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: the post-fork closure only performs descriptor syscalls. All
        // source owners are above every fixed destination and CLOEXEC at exec.
        unsafe {
            command.pre_exec(move || {
                for (source, target) in retained.iter().zip(INPUT_FDS) {
                    if libc::dup2(source.as_raw_fd(), target) != target {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                for target in [13, 219, 221, 333] {
                    if libc::dup2(retained[0].as_raw_fd(), target) != target {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        drop(command);
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("raw source subprocess timed out: {case}");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "raw source subprocess failed: {case} {status}"
        );
        assert_eq!(fs::read(report.path()).unwrap(), b"complete", "{case}");
    }
}

fn raw_open(fd: RawFd) -> bool {
    // SAFETY: scalar descriptor observation, including intentionally closed slots.
    unsafe { libc::fcntl(fd, libc::F_GETFD) >= 0 }
}
fn raw_identity(fd: RawFd) -> (u64, u64) {
    // SAFETY: helper only calls this for a live exclusively installed descriptor.
    let fd = unsafe { BorrowedFd::borrow_raw(fd) };
    let m = rustix::fs::fstat(fd).unwrap();
    (m.st_dev, m.st_ino)
}
fn assert_closed(objects: [(u64, u64); 11]) {
    for entry in fs::read_dir("/proc/self/fd").unwrap() {
        match fs::metadata(entry.unwrap().path()) {
            Ok(m) => assert!(!objects.contains(&(m.dev(), m.ino()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => panic!("inspect source closure: {e}"),
        }
    }
}

#[test]
fn raw_source_subprocess() {
    let Ok(case) = std::env::var(CASE_ENV) else {
        assert_eq!(INPUT_FDS, [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 220]);
        return;
    };
    let report = std::env::var_os(REPORT_ENV).unwrap();
    let objects = INPUT_FDS.map(raw_identity);
    // SAFETY: isolated exact-filter helper; inherited slots are raw, not Rust
    // owners. No admitted capabilities, files or child cleanup exist here.
    unsafe {
        close_unrelated().unwrap();
    }
    for fd in [0, 1, 2, 13, 219, 221, 333] {
        assert!(!raw_open(fd));
    }
    for fd in INPUT_FDS {
        assert!(raw_open(fd));
    }
    match case.as_str() {
        "refusal" | "unwind" => {
            let mut w = Work::new(if case == "refusal" { 0 } else { SOURCE_WORK });
            let floor = INPUT_FDS.len() * FILE_STORAGE;
            let mut b = Budget::new(&mut w, floor + SOURCE_SCRATCH);
            b.reserve_storage(floor).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
                // SAFETY: the child exclusively transfers the eleven raw slots.
                let mut sources = unsafe { Sources::new() };
                b.with_prepaid_scope(floor, 8, SOURCE_WORK, SOURCE_SCRATCH, |_| {
                    sources.validate()?;
                    let _owned = sources.take(LAUNCHER)?;
                    panic!("after guarded source duplication");
                })
            }));
            if case == "refusal" {
                assert!(matches!(
                    result,
                    Ok(Err(Error::Resource(Resource::Work(_))))
                ));
            } else {
                assert!(result.is_err());
            }
            assert_eq!(b.storage(), floor);
            assert_eq!(b.work(), if case == "refusal" { 0 } else { SOURCE_WORK });
        }
        _ => {
            let mut expected = objects;
            match case.as_str() {
                "missing" => deployment::close_inherited(INPUT_FDS[DEPLOYMENT]).unwrap(),
                "flags" => {
                    // SAFETY: install hostile flags before transferring raw custody.
                    assert_eq!(
                        unsafe { libc::fcntl(INPUT_FDS[KEY], libc::F_SETFD, libc::FD_CLOEXEC) },
                        0
                    );
                }
                "swapped" => {
                    let saved = deployment::duplicate_inherited(INPUT_FDS[LISTENER]).unwrap();
                    // SAFETY: arrange the raw fixture table before guard construction.
                    assert_eq!(
                        unsafe { libc::dup2(INPUT_FDS[LAUNCHER], INPUT_FDS[LISTENER]) },
                        INPUT_FDS[LISTENER]
                    );
                    assert_eq!(
                        unsafe { libc::dup2(saved.as_raw_fd(), INPUT_FDS[LAUNCHER]) },
                        INPUT_FDS[LAUNCHER]
                    );
                    drop(saved);
                    expected.swap(LISTENER, LAUNCHER);
                }
                _ => {}
            }
            // SAFETY: the child exclusively transfers the eleven raw slots.
            let mut sources = unsafe { Sources::new() };
            assert!(matches!(sources.take(LISTENER), Err(Error::SourceState)));
            match case.as_str() {
                "missing" => {
                    assert!(matches!(sources.validate(), Err(Error::Io { .. })));
                    assert!(matches!(sources.take(LISTENER), Err(Error::SourceState)));
                }
                "flags" => {
                    assert!(
                        matches!(sources.validate(), Err(Error::SourceFlags(fd)) if fd == INPUT_FDS[KEY])
                    );
                    assert!(matches!(sources.take(LISTENER), Err(Error::SourceState)));
                }
                "close-error" => {
                    sources.validate().unwrap();
                    let marker = File::open("/dev/null").unwrap();
                    let mut replacement = None;
                    let result = sources.take_with(LISTENER, |fd| {
                        deployment::close_inherited(fd)?;
                        // SAFETY: emulate Linux's released-on-error slot being
                        // reused before the error reaches the Sources guard.
                        assert_eq!(unsafe { libc::dup2(marker.as_raw_fd(), fd) }, fd);
                        replacement = Some(unsafe { OwnedFd::from_raw_fd(fd) });
                        Err(MechanicalError::Descriptor {
                            operation: "injected close failure after slot release",
                            source: Errno::INTR.into(),
                        })
                    });
                    assert!(matches!(
                        result,
                        Err(Error::Io {
                            errno: Errno::INTR,
                            ..
                        })
                    ));
                    assert!(matches!(sources.close(LISTENER), Err(Error::SourceState)));
                    drop(sources);
                    assert!(rustix::io::fcntl_getfd(replacement.as_ref().unwrap()).is_ok());
                    assert_closed(objects);
                    drop((replacement, marker));
                    fs::write(report, b"complete").unwrap();
                    // SAFETY: terminal helper exit; never resume the test harness after raw cleanup.
                    unsafe {
                        libc::_exit(0);
                    }
                }
                "fixed" | "swapped" => {
                    sources.validate().unwrap();
                    // Flags do not establish object roles: actual native admission
                    // remains responsible for refusing a substituted capability/image.
                    for role in 0..INPUT_FDS.len() {
                        let file = sources.take(role).unwrap();
                        assert!(file.as_raw_fd() >= 256);
                        assert_eq!(raw_identity(file.as_raw_fd()), expected[role]);
                        assert_eq!(
                            rustix::io::fcntl_getfd(&file).unwrap(),
                            rustix::io::FdFlags::CLOEXEC
                        );
                        assert!(!raw_open(INPUT_FDS[role]));
                        assert!(matches!(sources.take(role), Err(Error::SourceState)));
                        drop(file);
                    }
                    assert!(matches!(
                        sources.take(INPUT_FDS.len()),
                        Err(Error::SourceState)
                    ));
                }
                _ => panic!("unexpected source case"),
            }
            drop(sources);
        }
    }
    for fd in INPUT_FDS {
        assert!(!raw_open(fd));
    }
    assert_closed(objects);
    fs::write(report, b"complete").unwrap();
    // SAFETY: terminal helper exit; no protected startup or authority is claimed.
    unsafe {
        libc::_exit(0);
    }
}
